use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
    MMDeviceEnumerator, eConsole, eRender,
};
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
static LIVE_UPDATES: AtomicBool = AtomicBool::new(false);
static RUNTIME: Mutex<Option<AudioRuntime>> = Mutex::new(None);

enum AudioCommand {
    Refresh,
    DefaultDeviceChanged,
    SetVolume(u8),
    SetMute(bool),
    Shutdown,
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

fn decode_notification(data: &AUDIO_VOLUME_NOTIFICATION_DATA) -> (u8, bool) {
    (
        percent_from_scalar(data.fMasterVolume),
        data.bMuted.as_bool(),
    )
}

fn tracks_console_render(flow: EDataFlow, role: ERole) -> bool {
    flow == eRender && role == eConsole
}

fn store_state(percent: u8, muted: bool) {
    VOLUME_PERCENT.store(percent, Ordering::Relaxed);
    MUTED.store(muted, Ordering::Relaxed);
}

#[allow(clippy::inline_always, clippy::ref_as_ptr)]
mod listeners {
    use std::sync::Mutex;
    use std::sync::mpsc::Sender;

    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::Endpoints::{
        IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
    };
    use windows::Win32::Media::Audio::{
        AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole, IMMNotificationClient,
        IMMNotificationClient_Impl,
    };
    use windows::core::{PCWSTR, Result, implement};

    use super::{AudioCommand, decode_notification, store_state, tracks_console_render};

    #[implement(IAudioEndpointVolumeCallback)]
    pub(super) struct VolumeListener;

    impl IAudioEndpointVolumeCallback_Impl for VolumeListener_Impl {
        fn OnNotify(&self, notify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
            if let Some(data) = unsafe { notify.as_ref() } {
                let (percent, muted) = decode_notification(data);
                store_state(percent, muted);
                crate::idle_wake::request_idle_wake();
            }
            Ok(())
        }
    }

    #[implement(IMMNotificationClient)]
    pub(super) struct DeviceListener {
        pub(super) worker: Mutex<Sender<AudioCommand>>,
    }

    impl DeviceListener {
        fn notify_worker(&self, command: AudioCommand) {
            if let Ok(worker) = self.worker.lock() {
                let _ = worker.send(command);
            }
        }
    }

    impl IMMNotificationClient_Impl for DeviceListener_Impl {
        fn OnDeviceStateChanged(&self, _device: &PCWSTR, _state: DEVICE_STATE) -> Result<()> {
            Ok(())
        }

        fn OnDeviceAdded(&self, _device: &PCWSTR) -> Result<()> {
            Ok(())
        }

        fn OnDeviceRemoved(&self, _device: &PCWSTR) -> Result<()> {
            Ok(())
        }

        fn OnDefaultDeviceChanged(
            &self,
            flow: EDataFlow,
            role: ERole,
            _device: &PCWSTR,
        ) -> Result<()> {
            if tracks_console_render(flow, role) {
                self.notify_worker(AudioCommand::DefaultDeviceChanged);
            }
            Ok(())
        }

        fn OnPropertyValueChanged(&self, _device: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
            Ok(())
        }
    }
}

use listeners::{DeviceListener, VolumeListener};

struct DeviceSubscription {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

impl DeviceSubscription {
    fn register(worker: Sender<AudioCommand>) -> Option<Self> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }.ok()?;
        let client: IMMNotificationClient = DeviceListener {
            worker: Mutex::new(worker),
        }
        .into();
        unsafe { enumerator.RegisterEndpointNotificationCallback(&client) }.ok()?;
        Some(Self { enumerator, client })
    }
}

impl Drop for DeviceSubscription {
    fn drop(&mut self) {
        let _ = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.client)
        };
    }
}

struct EndpointSlot {
    endpoint: Option<IAudioEndpointVolume>,
    listener: IAudioEndpointVolumeCallback,
    listening: bool,
}

impl EndpointSlot {
    fn new() -> Self {
        Self {
            endpoint: None,
            listener: VolumeListener.into(),
            listening: false,
        }
    }

    fn current(&mut self) -> Option<&IAudioEndpointVolume> {
        if self.endpoint.is_none() {
            let endpoint = acquire_endpoint().ok()?;
            self.listening =
                unsafe { endpoint.RegisterControlChangeNotify(&self.listener) }.is_ok();
            self.endpoint = Some(endpoint);
        }
        self.endpoint.as_ref()
    }

    fn listening(&self) -> bool {
        self.endpoint.is_some() && self.listening
    }

    fn invalidate(&mut self) {
        if let Some(endpoint) = self.endpoint.take()
            && self.listening
        {
            let _ = unsafe { endpoint.UnregisterControlChangeNotify(&self.listener) };
        }
        self.listening = false;
    }
}

impl Drop for EndpointSlot {
    fn drop(&mut self) {
        self.invalidate();
    }
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

pub fn shutdown_audio_task() {
    let _ = stop_runtime();
}

fn stop_runtime() -> bool {
    let Some(runtime) = RUNTIME.lock().ok().and_then(|mut guard| guard.take()) else {
        return true;
    };
    let _ = runtime.sender.send(AudioCommand::Shutdown);
    drop(runtime.sender);
    let joined = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    AVAILABLE.store(false, Ordering::Release);
    REFRESH_PENDING.store(false, Ordering::Release);
    LIVE_UPDATES.store(false, Ordering::Release);
    joined
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
    slot: &mut EndpointSlot,
    operation: impl Fn(&IAudioEndpointVolume) -> Result<T>,
) -> Option<T> {
    for _ in 0..2 {
        let endpoint = slot.current()?;
        match operation(endpoint) {
            Ok(value) => return Some(value),
            Err(_) => slot.invalidate(),
        }
    }
    None
}

fn refresh(slot: &mut EndpointSlot) {
    let state = with_endpoint(slot, |endpoint| unsafe {
        Ok((
            endpoint.GetMasterVolumeLevelScalar()?,
            endpoint.GetMute()?.as_bool(),
        ))
    });
    match state {
        Some((scalar, muted)) => {
            store_state(percent_from_scalar(scalar), muted);
            AVAILABLE.store(true, Ordering::Release);
        }
        None => AVAILABLE.store(false, Ordering::Release),
    }
}

fn apply_volume(slot: &mut EndpointSlot, percent: u8) {
    let applied = with_endpoint(slot, |endpoint| unsafe {
        endpoint.SetMasterVolumeLevelScalar(f32::from(percent) / 100.0, std::ptr::null())?;
        endpoint.SetMute(percent == 0, std::ptr::null())
    });
    if applied.is_some() {
        store_state(percent, percent == 0);
    }
}

fn apply_mute(slot: &mut EndpointSlot, muted: bool) {
    let applied = with_endpoint(slot, |endpoint| unsafe {
        endpoint.SetMute(muted, std::ptr::null())
    });
    if applied.is_some() {
        MUTED.store(muted, Ordering::Relaxed);
    }
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
    let devices = DeviceSubscription::register(worker);
    let mut endpoint = EndpointSlot::new();
    refresh(&mut endpoint);
    publish_live_updates(devices.as_ref(), &endpoint);
    let _ = ready.send(());
    for command in receiver {
        match command {
            AudioCommand::Refresh => {
                REFRESH_PENDING.store(false, Ordering::Release);
                refresh(&mut endpoint);
            }
            AudioCommand::DefaultDeviceChanged => {
                endpoint.invalidate();
                refresh(&mut endpoint);
            }
            AudioCommand::SetVolume(percent) => apply_volume(&mut endpoint, percent),
            AudioCommand::SetMute(muted) => apply_mute(&mut endpoint, muted),
            AudioCommand::Shutdown => break,
        }
        publish_live_updates(devices.as_ref(), &endpoint);
    }
    LIVE_UPDATES.store(false, Ordering::Release);
    drop(endpoint);
    drop(devices);
}

#[cfg(test)]
mod tests {
    use windows::Win32::Media::Audio::{
        AUDIO_VOLUME_NOTIFICATION_DATA, eAll, eCapture, eCommunications, eConsole, eMultimedia,
        eRender,
    };
    use windows::core::BOOL;

    use super::{
        LIVE_UPDATES, RUNTIME, decode_notification, is_available, percent_from_scalar,
        read_volume_and_mute, stepped_percent, stop_runtime, tracks_console_render, volume_step,
    };
    use std::sync::atomic::Ordering;

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
    fn endpoint_notifications_decode_to_percent_and_mute() {
        let data = AUDIO_VOLUME_NOTIFICATION_DATA {
            bMuted: BOOL::from(true),
            fMasterVolume: 0.376,
            nChannels: 1,
            ..Default::default()
        };
        assert_eq!(decode_notification(&data), (38, true));
    }

    #[test]
    fn only_the_console_render_default_is_tracked() {
        assert!(tracks_console_render(eRender, eConsole));
        assert!(!tracks_console_render(eRender, eMultimedia));
        assert!(!tracks_console_render(eRender, eCommunications));
        assert!(!tracks_console_render(eCapture, eConsole));
        assert!(!tracks_console_render(eAll, eConsole));
    }

    #[test]
    fn runtime_starts_on_first_read_and_its_worker_exits_on_shutdown() {
        let _ = read_volume_and_mute();
        let handle_alive = RUNTIME
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|runtime| !runtime.handle.is_finished());
        assert!(handle_alive);
        assert!(stop_runtime());
        assert!(RUNTIME.lock().unwrap().is_none());
        assert!(!is_available());
        assert!(!LIVE_UPDATES.load(Ordering::Acquire));
    }

    #[test]
    #[ignore = "needs a default audio output device"]
    fn live_updates_register_on_the_default_output_device() {
        let _ = read_volume_and_mute();
        assert!(is_available());
        assert!(LIVE_UPDATES.load(Ordering::Acquire));
        assert!(stop_runtime());
        assert!(!LIVE_UPDATES.load(Ordering::Acquire));
    }
}
