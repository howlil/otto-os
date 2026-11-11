use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::Result;

use crate::com::ComGuard;
use crate::worker::join_within;

const READY_TIMEOUT: Duration = Duration::from_millis(500);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const STEP_PERCENT: i32 = 5;
const DEFAULT_VOLUME: u8 = 50;

static VOLUME_PERCENT: AtomicU8 = AtomicU8::new(DEFAULT_VOLUME);
static MUTED: AtomicBool = AtomicBool::new(false);
static AVAILABLE: AtomicBool = AtomicBool::new(false);
static REFRESH_PENDING: AtomicBool = AtomicBool::new(false);
static RUNTIME: Mutex<Option<AudioRuntime>> = Mutex::new(None);

enum AudioCommand {
    Refresh,
    SetVolume(u8),
    SetMute(bool),
}

struct AudioRuntime {
    sender: Sender<AudioCommand>,
    handle: JoinHandle<()>,
}

fn volume_step(wheel_delta: i16) -> Option<i32> {
    match wheel_delta.cmp(&0) {
        std::cmp::Ordering::Greater => Some(STEP_PERCENT),
        std::cmp::Ordering::Less => Some(-STEP_PERCENT),
        std::cmp::Ordering::Equal => None,
    }
}

fn stepped_percent(current: u8, step: i32) -> u8 {
    (i32::from(current) + step).clamp(0, 100) as u8
}

fn percent_from_scalar(scalar: f32) -> u8 {
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn running_sender() -> Option<Sender<AudioCommand>> {
    RUNTIME
        .try_lock()
        .ok()?
        .as_ref()
        .map(|runtime| runtime.sender.clone())
}

fn start_runtime() -> Option<Sender<AudioCommand>> {
    let (ready_sender, ready_receiver) = sync_channel(1);
    let sender = {
        let mut guard = RUNTIME.lock().ok()?;
        if let Some(runtime) = guard.as_ref() {
            return Some(runtime.sender.clone());
        }
        let (sender, receiver) = mpsc::channel();
        let handle = thread::Builder::new()
            .name(String::from("otto-audio"))
            .spawn(move || audio_worker(&receiver, &ready_sender))
            .ok()?;
        *guard = Some(AudioRuntime {
            sender: sender.clone(),
            handle,
        });
        sender
    };
    let _ = ready_receiver.recv_timeout(READY_TIMEOUT);
    Some(sender)
}

pub(crate) fn read_volume_and_mute() -> (u8, bool) {
    request_refresh();
    (
        VOLUME_PERCENT.load(Ordering::Relaxed),
        MUTED.load(Ordering::Relaxed),
    )
}

pub(crate) fn is_available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}

fn request_refresh() {
    let Some(sender) = running_sender().or_else(start_runtime) else {
        return;
    };
    if !REFRESH_PENDING.swap(true, Ordering::AcqRel) && sender.send(AudioCommand::Refresh).is_err()
    {
        REFRESH_PENDING.store(false, Ordering::Release);
    }
}

#[must_use]
pub fn set_system_volume(percent: u8) -> Option<(u8, bool)> {
    if !is_available() {
        return None;
    }
    let sender = running_sender()?;
    let target = percent.min(100);
    let muted = target == 0;
    sender.send(AudioCommand::SetVolume(target)).ok()?;
    VOLUME_PERCENT.store(target, Ordering::Relaxed);
    MUTED.store(muted, Ordering::Relaxed);
    Some((target, muted))
}

#[must_use]
pub fn step_system_volume(wheel_delta: i16) -> Option<(u8, bool)> {
    let step = volume_step(wheel_delta)?;
    set_system_volume(stepped_percent(
        VOLUME_PERCENT.load(Ordering::Relaxed),
        step,
    ))
}

#[must_use]
pub fn toggle_system_mute() -> Option<(u8, bool)> {
    if !is_available() {
        return None;
    }
    let sender = running_sender()?;
    let next = !MUTED.fetch_not(Ordering::AcqRel);
    sender.send(AudioCommand::SetMute(next)).ok()?;
    Some((VOLUME_PERCENT.load(Ordering::Relaxed), next))
}

pub fn shutdown_audio_task() {
    let Some(runtime) = RUNTIME.lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    drop(runtime.sender);
    let _ = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    AVAILABLE.store(false, Ordering::Release);
    REFRESH_PENDING.store(false, Ordering::Release);
}

fn acquire_endpoint() -> Result<IAudioEndpointVolume> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        device.Activate(CLSCTX_INPROC_SERVER, None)
    }
}

fn with_endpoint<T>(
    slot: &mut Option<IAudioEndpointVolume>,
    operation: impl Fn(&IAudioEndpointVolume) -> Result<T>,
) -> Option<T> {
    for _ in 0..2 {
        if slot.is_none() {
            *slot = acquire_endpoint().ok();
        }
        let endpoint = slot.as_ref()?;
        match operation(endpoint) {
            Ok(value) => return Some(value),
            Err(_) => *slot = None,
        }
    }
    None
}

fn refresh(slot: &mut Option<IAudioEndpointVolume>) {
    let state = with_endpoint(slot, |endpoint| unsafe {
        Ok((
            endpoint.GetMasterVolumeLevelScalar()?,
            endpoint.GetMute()?.as_bool(),
        ))
    });
    match state {
        Some((scalar, muted)) => {
            VOLUME_PERCENT.store(percent_from_scalar(scalar), Ordering::Relaxed);
            MUTED.store(muted, Ordering::Relaxed);
            AVAILABLE.store(true, Ordering::Release);
        }
        None => AVAILABLE.store(false, Ordering::Release),
    }
}

fn apply_volume(slot: &mut Option<IAudioEndpointVolume>, percent: u8) {
    let applied = with_endpoint(slot, |endpoint| unsafe {
        endpoint.SetMasterVolumeLevelScalar(f32::from(percent) / 100.0, std::ptr::null())?;
        endpoint.SetMute(percent == 0, std::ptr::null())
    });
    if applied.is_some() {
        VOLUME_PERCENT.store(percent, Ordering::Relaxed);
        MUTED.store(percent == 0, Ordering::Relaxed);
    }
}

fn apply_mute(slot: &mut Option<IAudioEndpointVolume>, muted: bool) {
    let applied = with_endpoint(slot, |endpoint| unsafe {
        endpoint.SetMute(muted, std::ptr::null())
    });
    if applied.is_some() {
        MUTED.store(muted, Ordering::Relaxed);
    }
}

fn audio_worker(receiver: &Receiver<AudioCommand>, ready: &SyncSender<()>) {
    let _com = ComGuard::multithreaded();
    let mut endpoint: Option<IAudioEndpointVolume> = None;
    refresh(&mut endpoint);
    let _ = ready.send(());
    for command in receiver {
        match command {
            AudioCommand::Refresh => {
                REFRESH_PENDING.store(false, Ordering::Release);
                refresh(&mut endpoint);
            }
            AudioCommand::SetVolume(percent) => apply_volume(&mut endpoint, percent),
            AudioCommand::SetMute(muted) => apply_mute(&mut endpoint, muted),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RUNTIME, is_available, percent_from_scalar, read_volume_and_mute, shutdown_audio_task,
        stepped_percent, volume_step,
    };

    #[test]
    fn wheel_direction_maps_to_a_fixed_step() {
        assert_eq!(volume_step(120), Some(5));
        assert_eq!(volume_step(-120), Some(-5));
        assert_eq!(volume_step(0), None);
    }

    #[test]
    fn stepping_clamps_to_the_percent_range() {
        assert_eq!(stepped_percent(98, 5), 100);
        assert_eq!(stepped_percent(3, -5), 0);
        assert_eq!(stepped_percent(50, 5), 55);
    }

    #[test]
    fn scalar_conversion_rounds_and_clamps() {
        assert_eq!(percent_from_scalar(0.554), 55);
        assert_eq!(percent_from_scalar(1.4), 100);
        assert_eq!(percent_from_scalar(-0.2), 0);
    }

    #[test]
    fn runtime_starts_on_first_read_and_releases_on_shutdown() {
        let _ = read_volume_and_mute();
        assert!(RUNTIME.lock().unwrap().is_some());
        shutdown_audio_task();
        assert!(RUNTIME.lock().unwrap().is_none());
        assert!(!is_available());
    }
}
