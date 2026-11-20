mod listeners;
mod outputs;
mod slots;
#[cfg(test)]
mod tests;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::{AudioOutputDevice, MicState};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, EDataFlow, ERole, eCapture, eConsole, eRender,
};

use crate::com::ComGuard;
use crate::worker::join_within;

use self::listeners::DeviceSubscription;
use self::outputs::{apply_audio_output, enumerate_audio_outputs};
use self::slots::{CaptureSlot, EndpointSlot, apply_mute, apply_volume, refresh};

const READY_TIMEOUT: Duration = Duration::from_millis(500);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const STEP_PERCENT: i32 = 5;
const DEFAULT_VOLUME: u8 = 50;

pub(super) static VOLUME_PERCENT: AtomicU8 = AtomicU8::new(DEFAULT_VOLUME);
pub(super) static MUTED: AtomicBool = AtomicBool::new(false);
pub(super) static AVAILABLE: AtomicBool = AtomicBool::new(false);
static REFRESH_PENDING: AtomicBool = AtomicBool::new(false);
pub(super) static LIVE_UPDATES: AtomicBool = AtomicBool::new(false);
pub(super) static MIC_AVAILABLE: AtomicBool = AtomicBool::new(false);
pub(super) static MIC_MUTED: AtomicBool = AtomicBool::new(false);
pub(super) static MIC_SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
pub(super) static CAPTURE_LOADED: AtomicBool = AtomicBool::new(false);
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
pub(super) static RUNTIME: Mutex<Option<AudioRuntime>> = Mutex::new(None);

pub(super) enum AudioCommand {
    Refresh,
    DefaultDeviceChanged,
    CaptureDeviceChanged,
    CaptureSessionsChanged,
    SetVolume(u8),
    SetMute(bool),
    SetMicMute(bool),
    ReadOutputs(SyncSender<Vec<AudioOutputDevice>>),
    SetOutput(String, SyncSender<bool>),
    Shutdown,
}

pub(super) struct AudioRuntime {
    pub(super) sender: Sender<AudioCommand>,
    pub(super) handle: JoinHandle<()>,
}

pub(super) fn volume_step(wheel_delta: i16) -> Option<i32> {
    match wheel_delta.cmp(&0) {
        std::cmp::Ordering::Greater => Some(STEP_PERCENT),
        std::cmp::Ordering::Less => Some(-STEP_PERCENT),
        std::cmp::Ordering::Equal => None,
    }
}

pub(super) fn stepped_percent(current: u8, step: i32) -> u8 {
    (i32::from(current) + step).clamp(0, 100) as u8
}

pub(super) fn percent_from_scalar(scalar: f32) -> u8 {
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

pub(super) fn decode_notification(data: &AUDIO_VOLUME_NOTIFICATION_DATA) -> (u8, bool) {
    (
        percent_from_scalar(data.fMasterVolume),
        data.bMuted.as_bool(),
    )
}

pub(super) fn tracks_console_render(flow: EDataFlow, role: ERole) -> bool {
    flow == eRender && role == eConsole
}

pub(super) fn tracks_console_capture(flow: EDataFlow, role: ERole) -> bool {
    flow == eCapture && role == eConsole
}

pub(super) fn store_state(percent: u8, muted: bool) {
    VOLUME_PERCENT.store(percent, Ordering::Relaxed);
    MUTED.store(muted, Ordering::Relaxed);
}

pub(crate) fn resume_audio_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

fn running_sender() -> Option<Sender<AudioCommand>> {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return None;
    }
    RUNTIME
        .try_lock()
        .ok()?
        .as_ref()
        .map(|runtime| runtime.sender.clone())
}

fn start_runtime() -> Option<Sender<AudioCommand>> {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return None;
    }
    let (ready_sender, ready_receiver) = sync_channel(1);
    let sender = {
        let mut guard = RUNTIME.lock().ok()?;
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            return None;
        }
        if let Some(runtime) = guard.as_ref() {
            return Some(runtime.sender.clone());
        }
        let (sender, receiver) = mpsc::channel();
        let worker = sender.clone();
        let handle = thread::Builder::new()
            .name(String::from("otto-audio"))
            .spawn(move || audio_worker(&receiver, worker, &ready_sender))
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
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return (
            VOLUME_PERCENT.load(Ordering::Relaxed),
            MUTED.load(Ordering::Relaxed),
        );
    }
    let live = running_sender().is_some() && LIVE_UPDATES.load(Ordering::Acquire);
    if !live {
        request_refresh();
    }
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
    store_state(target, muted);
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

#[must_use]
pub fn set_system_mute(muted: bool) -> Option<(u8, bool)> {
    if !is_available() {
        return None;
    }
    let sender = running_sender()?;
    sender.send(AudioCommand::SetMute(muted)).ok()?;
    MUTED.store(muted, Ordering::Release);
    Some((VOLUME_PERCENT.load(Ordering::Relaxed), muted))
}

#[must_use]
pub(crate) fn read_mic_state() -> MicState {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return MicState::Unavailable;
    }
    if !CAPTURE_LOADED.load(Ordering::Acquire) {
        let _ = running_sender().or_else(start_runtime);
    }
    if !MIC_AVAILABLE.load(Ordering::Acquire) {
        return MicState::Unavailable;
    }
    let muted = MIC_MUTED.load(Ordering::Relaxed);
    if MIC_SESSION_ACTIVE.load(Ordering::Acquire) {
        MicState::Active { muted }
    } else {
        MicState::Idle { muted }
    }
}

#[must_use]
pub(crate) fn toggle_mic_mute() -> Option<bool> {
    let sender = running_sender().or_else(start_runtime)?;
    if !MIC_AVAILABLE.load(Ordering::Acquire) {
        return None;
    }
    let next = !MIC_MUTED.fetch_not(Ordering::AcqRel);
    sender.send(AudioCommand::SetMicMute(next)).ok()?;
    Some(next)
}

pub fn shutdown_audio_task() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    let _ = stop_runtime();
}

#[must_use]
pub(crate) fn read_audio_outputs() -> Vec<AudioOutputDevice> {
    let Some(sender) = running_sender().or_else(start_runtime) else {
        return Vec::new();
    };
    let (reply_tx, reply_rx) = sync_channel(1);
    if sender.send(AudioCommand::ReadOutputs(reply_tx)).is_err() {
        return Vec::new();
    }
    reply_rx.recv_timeout(READY_TIMEOUT).unwrap_or_default()
}

#[must_use]
pub(crate) fn set_audio_output(device_id: &str) -> bool {
    if device_id.trim().is_empty() {
        return false;
    }
    let Some(sender) = running_sender().or_else(start_runtime) else {
        return false;
    };
    let (reply_tx, reply_rx) = sync_channel(1);
    if sender
        .send(AudioCommand::SetOutput(device_id.to_owned(), reply_tx))
        .is_err()
    {
        return false;
    }
    reply_rx.recv_timeout(READY_TIMEOUT).unwrap_or(false)
}

pub(super) fn stop_runtime() -> bool {
    let Some(runtime) = RUNTIME.lock().ok().and_then(|mut guard| guard.take()) else {
        return true;
    };
    let _ = runtime.sender.send(AudioCommand::Shutdown);
    drop(runtime.sender);
    let joined = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    AVAILABLE.store(false, Ordering::Release);
    REFRESH_PENDING.store(false, Ordering::Release);
    LIVE_UPDATES.store(false, Ordering::Release);
    MIC_AVAILABLE.store(false, Ordering::Release);
    MIC_MUTED.store(false, Ordering::Release);
    MIC_SESSION_ACTIVE.store(false, Ordering::Release);
    CAPTURE_LOADED.store(false, Ordering::Release);
    joined
}

fn publish_live_updates(devices: Option<&DeviceSubscription>, slot: &EndpointSlot) {
    LIVE_UPDATES.store(devices.is_some() && slot.listening(), Ordering::Release);
}

fn audio_worker(
    receiver: &Receiver<AudioCommand>,
    worker: Sender<AudioCommand>,
    ready: &SyncSender<()>,
) {
    let _com = ComGuard::multithreaded();
    let mut capture = CaptureSlot::new(worker.clone());
    let devices = DeviceSubscription::register(worker);
    let mut endpoint = EndpointSlot::new();
    refresh(&mut endpoint);
    capture.refresh();
    publish_live_updates(devices.as_ref(), &endpoint);
    let _ = ready.send(());
    for command in receiver {
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            break;
        }
        match command {
            AudioCommand::Refresh => {
                REFRESH_PENDING.store(false, Ordering::Release);
                refresh(&mut endpoint);
                capture.refresh();
            }
            AudioCommand::DefaultDeviceChanged => {
                endpoint.invalidate();
                refresh(&mut endpoint);
            }
            AudioCommand::CaptureDeviceChanged => {
                capture.invalidate();
                capture.refresh();
            }
            AudioCommand::CaptureSessionsChanged => {
                capture.refresh_sessions();
            }
            AudioCommand::SetVolume(percent) => apply_volume(&mut endpoint, percent),
            AudioCommand::SetMute(muted) => apply_mute(&mut endpoint, muted),
            AudioCommand::SetMicMute(muted) => capture.apply_mute(muted),
            AudioCommand::ReadOutputs(reply) => {
                let _ = reply.send(enumerate_audio_outputs(&mut endpoint));
            }
            AudioCommand::SetOutput(device_id, reply) => {
                let ok = apply_audio_output(&mut endpoint, &device_id);
                let _ = reply.send(ok);
            }
            AudioCommand::Shutdown => break,
        }
        publish_live_updates(devices.as_ref(), &endpoint);
    }
    LIVE_UPDATES.store(false, Ordering::Release);
    drop(capture);
    drop(endpoint);
    drop(devices);
}
