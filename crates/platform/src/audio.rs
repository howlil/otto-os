use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::{AudioOutputDevice, MicState};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, AudioSessionStateActive, DEVICE_STATE_ACTIVE, EDataFlow, ERole,
    IAudioSessionControl, IAudioSessionEvents, IAudioSessionManager2, IAudioSessionNotification,
    IMMDevice, IMMDeviceEnumerator, IMMNotificationClient, MMDeviceEnumerator, eCapture,
    eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, STGM_READ,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::core::{PCWSTR, PWSTR, Result};

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
static MIC_AVAILABLE: AtomicBool = AtomicBool::new(false);
static MIC_MUTED: AtomicBool = AtomicBool::new(false);
static MIC_SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAPTURE_LOADED: AtomicBool = AtomicBool::new(false);
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static RUNTIME: Mutex<Option<AudioRuntime>> = Mutex::new(None);

enum AudioCommand {
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

fn tracks_console_capture(flow: EDataFlow, role: ERole) -> bool {
    flow == eCapture && role == eConsole
}

fn store_state(percent: u8, muted: bool) {
    VOLUME_PERCENT.store(percent, Ordering::Relaxed);
    MUTED.store(muted, Ordering::Relaxed);
}

#[allow(clippy::inline_always, clippy::ref_as_ptr)]
mod listeners {
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::Sender;

    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::Media::Audio::Endpoints::{
        IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
    };
    use windows::Win32::Media::Audio::{
        AUDIO_VOLUME_NOTIFICATION_DATA, AudioSessionDisconnectReason, AudioSessionState,
        DEVICE_STATE, EDataFlow, ERole, IAudioSessionControl, IAudioSessionEvents,
        IAudioSessionEvents_Impl, IAudioSessionNotification, IAudioSessionNotification_Impl,
        IMMNotificationClient, IMMNotificationClient_Impl,
    };
    use windows::core::{BOOL, GUID, PCWSTR, Ref, Result, implement};

    use super::{
        AudioCommand, MIC_MUTED, decode_notification, store_state, tracks_console_capture,
        tracks_console_render,
    };

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

    #[implement(IAudioEndpointVolumeCallback)]
    pub(super) struct CaptureVolumeListener;

    impl IAudioEndpointVolumeCallback_Impl for CaptureVolumeListener_Impl {
        fn OnNotify(&self, notify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
            if let Some(data) = unsafe { notify.as_ref() } {
                MIC_MUTED.store(data.bMuted.as_bool(), Ordering::Relaxed);
                crate::idle_wake::request_idle_wake();
            }
            Ok(())
        }
    }

    #[implement(IAudioSessionNotification)]
    pub(super) struct CaptureSessionNotification {
        pub(super) worker: Mutex<Sender<AudioCommand>>,
    }

    impl IAudioSessionNotification_Impl for CaptureSessionNotification_Impl {
        fn OnSessionCreated(&self, _newsession: Ref<IAudioSessionControl>) -> Result<()> {
            if let Ok(worker) = self.worker.lock() {
                let _ = worker.send(AudioCommand::CaptureSessionsChanged);
            }
            crate::idle_wake::request_idle_wake();
            Ok(())
        }
    }

    #[implement(IAudioSessionEvents)]
    pub(super) struct CaptureSessionEvents {
        pub(super) worker: Mutex<Sender<AudioCommand>>,
    }

    impl CaptureSessionEvents_Impl {
        fn notify(&self) {
            if let Ok(worker) = self.worker.lock() {
                let _ = worker.send(AudioCommand::CaptureSessionsChanged);
            }
            crate::idle_wake::request_idle_wake();
        }
    }

    impl IAudioSessionEvents_Impl for CaptureSessionEvents_Impl {
        fn OnDisplayNameChanged(
            &self,
            _newdisplayname: &PCWSTR,
            _eventcontext: *const GUID,
        ) -> Result<()> {
            Ok(())
        }

        fn OnIconPathChanged(
            &self,
            _newiconpath: &PCWSTR,
            _eventcontext: *const GUID,
        ) -> Result<()> {
            Ok(())
        }

        fn OnSimpleVolumeChanged(
            &self,
            _newvolume: f32,
            _newmute: BOOL,
            _eventcontext: *const GUID,
        ) -> Result<()> {
            Ok(())
        }

        fn OnChannelVolumeChanged(
            &self,
            _channelcount: u32,
            _newchannelvolumearray: *const f32,
            _changedchannel: u32,
            _eventcontext: *const GUID,
        ) -> Result<()> {
            Ok(())
        }

        fn OnGroupingParamChanged(
            &self,
            _newgroupingparam: *const GUID,
            _eventcontext: *const GUID,
        ) -> Result<()> {
            Ok(())
        }

        fn OnStateChanged(&self, _newstate: AudioSessionState) -> Result<()> {
            self.notify();
            Ok(())
        }

        fn OnSessionDisconnected(
            &self,
            _disconnectreason: AudioSessionDisconnectReason,
        ) -> Result<()> {
            self.notify();
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
            self.notify_worker(AudioCommand::CaptureDeviceChanged);
            crate::connectivity::notify_bluetooth_hardware_change();
            crate::idle_wake::request_idle_wake();
            Ok(())
        }

        fn OnDeviceAdded(&self, _device: &PCWSTR) -> Result<()> {
            self.notify_worker(AudioCommand::CaptureDeviceChanged);
            crate::connectivity::notify_bluetooth_hardware_change();
            crate::idle_wake::request_idle_wake();
            Ok(())
        }

        fn OnDeviceRemoved(&self, _device: &PCWSTR) -> Result<()> {
            self.notify_worker(AudioCommand::CaptureDeviceChanged);
            crate::idle_wake::request_idle_wake();
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
                crate::idle_wake::request_idle_wake();
            } else if tracks_console_capture(flow, role) {
                self.notify_worker(AudioCommand::CaptureDeviceChanged);
                crate::idle_wake::request_idle_wake();
            }
            Ok(())
        }

        fn OnPropertyValueChanged(&self, _device: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
            Ok(())
        }
    }
}

use listeners::{
    CaptureSessionEvents, CaptureSessionNotification, CaptureVolumeListener, DeviceListener,
    VolumeListener,
};

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
    enumerator: Option<IMMDeviceEnumerator>,
    endpoint: Option<IAudioEndpointVolume>,
    listener: IAudioEndpointVolumeCallback,
    listening: bool,
}

impl EndpointSlot {
    fn new() -> Self {
        Self {
            enumerator: None,
            endpoint: None,
            listener: VolumeListener.into(),
            listening: false,
        }
    }

    fn enumerator(&mut self) -> Option<&IMMDeviceEnumerator> {
        if self.enumerator.is_none() {
            self.enumerator =
                unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }.ok();
        }
        self.enumerator.as_ref()
    }

    fn current(&mut self) -> Option<&IAudioEndpointVolume> {
        if self.endpoint.is_none() {
            let endpoint = acquire_endpoint(self.enumerator()?).ok()?;
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
        self.enumerator = None;
    }
}

struct CaptureSlot {
    enumerator: Option<IMMDeviceEnumerator>,
    endpoint: Option<IAudioEndpointVolume>,
    volume_listener: IAudioEndpointVolumeCallback,
    volume_listening: bool,
    session_manager: Option<IAudioSessionManager2>,
    session_listener: IAudioSessionNotification,
    session_listening: bool,
    session_events: Vec<(IAudioSessionControl, IAudioSessionEvents)>,
    worker: Sender<AudioCommand>,
}

impl CaptureSlot {
    fn new(worker: Sender<AudioCommand>) -> Self {
        let session_listener: IAudioSessionNotification = CaptureSessionNotification {
            worker: Mutex::new(worker.clone()),
        }
        .into();
        Self {
            enumerator: None,
            endpoint: None,
            volume_listener: CaptureVolumeListener.into(),
            volume_listening: false,
            session_manager: None,
            session_listener,
            session_listening: false,
            session_events: Vec::new(),
            worker,
        }
    }

    fn enumerator(&mut self) -> Option<&IMMDeviceEnumerator> {
        if self.enumerator.is_none() {
            self.enumerator =
                unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }.ok();
        }
        self.enumerator.as_ref()
    }

    fn default_capture_device(&mut self) -> Option<IMMDevice> {
        let enumerator = self.enumerator()?;
        unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) }.ok()
    }

    fn ensure_interfaces(&mut self, device: &IMMDevice) {
        if self.endpoint.is_none()
            && let Ok(endpoint) =
                unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_INPROC_SERVER, None) }
        {
            self.volume_listening =
                unsafe { endpoint.RegisterControlChangeNotify(&self.volume_listener) }.is_ok();
            self.endpoint = Some(endpoint);
        }
        if self.session_manager.is_none()
            && let Ok(manager) =
                unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_INPROC_SERVER, None) }
        {
            self.session_listening =
                unsafe { manager.RegisterSessionNotification(&self.session_listener) }.is_ok();
            self.session_manager = Some(manager);
        }
    }

    fn refresh(&mut self) {
        let Some(device) = self.default_capture_device() else {
            self.invalidate();
            MIC_AVAILABLE.store(false, Ordering::Release);
            MIC_MUTED.store(false, Ordering::Release);
            MIC_SESSION_ACTIVE.store(false, Ordering::Release);
            CAPTURE_LOADED.store(true, Ordering::Release);
            return;
        };
        self.ensure_interfaces(&device);
        let muted = self
            .endpoint
            .as_ref()
            .and_then(|endpoint| unsafe { endpoint.GetMute() }.ok())
            .map(windows::core::BOOL::as_bool);
        match muted {
            Some(is_muted) => {
                MIC_MUTED.store(is_muted, Ordering::Relaxed);
                MIC_AVAILABLE.store(true, Ordering::Release);
            }
            None => MIC_AVAILABLE.store(false, Ordering::Release),
        }
        self.refresh_sessions();
        CAPTURE_LOADED.store(true, Ordering::Release);
    }

    fn refresh_sessions(&mut self) {
        self.clear_session_events();
        let Some(manager) = self.session_manager.as_ref() else {
            MIC_SESSION_ACTIVE.store(false, Ordering::Release);
            return;
        };
        let Ok(enumerator) = (unsafe { manager.GetSessionEnumerator() }) else {
            MIC_SESSION_ACTIVE.store(false, Ordering::Release);
            return;
        };
        let count = unsafe { enumerator.GetCount() }.unwrap_or(0);
        let mut any_active = false;
        for index in 0..count {
            let Ok(control) = (unsafe { enumerator.GetSession(index) }) else {
                continue;
            };
            if unsafe { control.GetState() }.ok() == Some(AudioSessionStateActive) {
                any_active = true;
            }
            let listener: IAudioSessionEvents = CaptureSessionEvents {
                worker: Mutex::new(self.worker.clone()),
            }
            .into();
            if unsafe { control.RegisterAudioSessionNotification(&listener) }.is_ok() {
                self.session_events.push((control, listener));
            }
        }
        MIC_SESSION_ACTIVE.store(any_active, Ordering::Release);
    }

    fn apply_mute(&mut self, muted: bool) {
        if self.endpoint.is_none() {
            self.refresh();
        }
        let applied = self
            .endpoint
            .as_ref()
            .is_some_and(|ep| unsafe { ep.SetMute(muted, std::ptr::null()) }.is_ok());
        if applied {
            MIC_MUTED.store(muted, Ordering::Relaxed);
        } else {
            self.invalidate();
            self.refresh();
            if let Some(ep) = self.endpoint.as_ref()
                && unsafe { ep.SetMute(muted, std::ptr::null()) }.is_ok()
            {
                MIC_MUTED.store(muted, Ordering::Relaxed);
            }
        }
    }

    fn clear_session_events(&mut self) {
        for (control, listener) in self.session_events.drain(..) {
            let _ = unsafe { control.UnregisterAudioSessionNotification(&listener) };
        }
    }

    fn invalidate(&mut self) {
        self.clear_session_events();
        if let Some(manager) = self.session_manager.take()
            && self.session_listening
        {
            let _ = unsafe { manager.UnregisterSessionNotification(&self.session_listener) };
        }
        if let Some(endpoint) = self.endpoint.take()
            && self.volume_listening
        {
            let _ = unsafe { endpoint.UnregisterControlChangeNotify(&self.volume_listener) };
        }
        self.session_listening = false;
        self.volume_listening = false;
    }
}

impl Drop for CaptureSlot {
    fn drop(&mut self) {
        self.invalidate();
        self.enumerator = None;
    }
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
    MIC_AVAILABLE.store(false, Ordering::Release);
    MIC_MUTED.store(false, Ordering::Release);
    MIC_SESSION_ACTIVE.store(false, Ordering::Release);
    CAPTURE_LOADED.store(false, Ordering::Release);
    joined
}

fn acquire_endpoint(enumerator: &IMMDeviceEnumerator) -> Result<IAudioEndpointVolume> {
    unsafe {
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

struct PropVariantGuard(PROPVARIANT);

impl Drop for PropVariantGuard {
    fn drop(&mut self) {
        let _ = unsafe { PropVariantClear(&raw mut self.0) };
    }
}

fn take_cotask_pwstr(raw: PWSTR) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    let text = unsafe { raw.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    text.filter(|s| !s.trim().is_empty())
}

fn device_id(device: &IMMDevice) -> Option<String> {
    let raw = unsafe { device.GetId() }.ok()?;
    take_cotask_pwstr(raw)
}

fn device_friendly_name(device: &IMMDevice) -> Option<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.ok()?;
    let value = PropVariantGuard(unsafe { store.GetValue(&PKEY_Device_FriendlyName) }.ok()?);
    unsafe {
        if value.0.Anonymous.Anonymous.vt != VT_LPWSTR {
            return None;
        }
        let ptr = value.0.Anonymous.Anonymous.Anonymous.pwszVal;
        (!ptr.is_null())
            .then(|| ptr.to_string().ok())
            .flatten()
            .filter(|s| !s.trim().is_empty())
    }
}

fn enumerate_audio_outputs(slot: &mut EndpointSlot) -> Vec<AudioOutputDevice> {
    let Some(enumerator) = slot.enumerator() else {
        return Vec::new();
    };
    let default_id = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .ok()
        .and_then(|dev| device_id(&dev));
    let Ok(collection) = (unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) })
    else {
        return Vec::new();
    };
    let Ok(count) = (unsafe { collection.GetCount() }) else {
        return Vec::new();
    };
    let mut outputs = Vec::with_capacity(count as usize);
    for index in 0..count {
        let Ok(device) = (unsafe { collection.Item(index) }) else {
            continue;
        };
        let Some(id) = device_id(&device) else {
            continue;
        };
        let name = device_friendly_name(&device).unwrap_or_else(|| id.clone());
        let is_default = default_id.as_deref() == Some(id.as_str());
        outputs.push(AudioOutputDevice {
            id,
            name,
            is_default,
        });
    }
    outputs
}

fn apply_audio_output(slot: &mut EndpointSlot, id: &str) -> bool {
    if !set_default_endpoint_policy(id) {
        return false;
    }
    slot.invalidate();
    refresh(slot);
    true
}

#[allow(
    clippy::inline_always,
    clippy::missing_safety_doc,
    clippy::ref_as_ptr,
    clippy::transmute_ptr_to_ptr
)]
mod policy_com {
    use std::ffi::c_void;

    use windows::Win32::Media::Audio::ERole;
    use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

    pub const CLSID_POLICY_CONFIG_CLIENT: GUID =
        GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    pub unsafe trait IPolicyConfig: IUnknown {
        pub unsafe fn get_mix_format(
            &self,
            device_name: PCWSTR,
            format: *mut *mut c_void,
        ) -> HRESULT;
        pub unsafe fn get_device_format(
            &self,
            device_name: PCWSTR,
            default: i32,
            format: *mut *mut c_void,
        ) -> HRESULT;
        pub unsafe fn reset_device_format(&self, device_name: PCWSTR) -> HRESULT;
        pub unsafe fn set_device_format(
            &self,
            device_name: PCWSTR,
            endpoint_format: *mut c_void,
            mix_format: *mut c_void,
        ) -> HRESULT;
        pub unsafe fn get_processing_period(
            &self,
            device_name: PCWSTR,
            default: i32,
            default_period: *mut i64,
            min_period: *mut i64,
        ) -> HRESULT;
        pub unsafe fn set_processing_period(
            &self,
            device_name: PCWSTR,
            period: *mut i64,
        ) -> HRESULT;
        pub unsafe fn get_share_mode(&self, device_name: PCWSTR, mode: *mut c_void) -> HRESULT;
        pub unsafe fn set_share_mode(&self, device_name: PCWSTR, mode: *mut c_void) -> HRESULT;
        pub unsafe fn get_property_value(
            &self,
            device_name: PCWSTR,
            fx_store: i32,
            key: *const c_void,
            value: *mut c_void,
        ) -> HRESULT;
        pub unsafe fn set_property_value(
            &self,
            device_name: PCWSTR,
            fx_store: i32,
            key: *const c_void,
            value: *mut c_void,
        ) -> HRESULT;
        pub unsafe fn set_default_endpoint(&self, device_id: PCWSTR, role: ERole) -> HRESULT;
        pub unsafe fn set_endpoint_visibility(&self, device_id: PCWSTR, visible: i32) -> HRESULT;
    }
}

fn set_default_endpoint_policy(id: &str) -> bool {
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let policy: Result<policy_com::IPolicyConfig> =
        unsafe { CoCreateInstance(&policy_com::CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL) };
    let Ok(policy) = policy else {
        return false;
    };
    let ptr = PCWSTR::from_raw(wide.as_ptr());
    unsafe {
        policy.set_default_endpoint(ptr, eConsole).is_ok()
            && policy.set_default_endpoint(ptr, eMultimedia).is_ok()
            && policy.set_default_endpoint(ptr, eCommunications).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Media::Audio::{
        AUDIO_VOLUME_NOTIFICATION_DATA, eAll, eCapture, eCommunications, eConsole, eMultimedia,
        eRender,
    };
    use windows::core::BOOL;

    use std::sync::atomic::Ordering;
    use std::sync::{Mutex, PoisonError};

    use super::{
        LIVE_UPDATES, RUNTIME, decode_notification, is_available, percent_from_scalar,
        read_mic_state, read_volume_and_mute, resume_audio_task, shutdown_audio_task,
        stepped_percent, stop_runtime, tracks_console_capture, tracks_console_render, volume_step,
    };

    static RUNTIME_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct AudioResumeGuard;

    impl Drop for AudioResumeGuard {
        fn drop(&mut self) {
            let _ = stop_runtime();
            resume_audio_task();
        }
    }

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
    fn only_the_console_capture_default_is_tracked_for_mic() {
        assert!(tracks_console_capture(eCapture, eConsole));
        assert!(!tracks_console_capture(eCapture, eMultimedia));
        assert!(!tracks_console_capture(eCapture, eCommunications));
        assert!(!tracks_console_capture(eRender, eConsole));
        assert!(!tracks_console_capture(eAll, eConsole));
    }

    #[test]
    fn runtime_starts_on_first_read_and_its_worker_exits_on_shutdown() {
        let _serial = RUNTIME_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = AudioResumeGuard;
        resume_audio_task();
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
    fn shutdown_prevents_audio_worker_respawn_until_resumed() {
        let _serial = RUNTIME_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = AudioResumeGuard;
        resume_audio_task();
        let _ = read_volume_and_mute();
        shutdown_audio_task();
        assert!(RUNTIME.lock().unwrap().is_none());

        let _ = read_volume_and_mute();
        let _ = read_mic_state();
        let _ = super::read_audio_outputs();
        assert!(RUNTIME.lock().unwrap().is_none());
    }

    #[test]
    #[ignore = "needs a default audio output device"]
    fn live_updates_register_on_the_default_output_device() {
        let _serial = RUNTIME_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = AudioResumeGuard;
        resume_audio_task();
        let _ = read_volume_and_mute();
        assert!(is_available());
        assert!(LIVE_UPDATES.load(Ordering::Acquire));
        assert!(stop_runtime());
        assert!(!LIVE_UPDATES.load(Ordering::Acquire));
    }

    #[test]
    fn audio_output_queries_are_safe_and_blank_id_is_rejected() {
        let _serial = RUNTIME_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = AudioResumeGuard;
        resume_audio_task();
        let outputs = super::read_audio_outputs();
        if let Some(default_dev) = outputs.iter().find(|dev| dev.is_default) {
            assert!(!default_dev.id.trim().is_empty());
            assert!(!default_dev.name.trim().is_empty());
        }
        assert!(!super::set_audio_output(""));
        assert!(!super::set_audio_output("   "));
        assert!(stop_runtime());
    }
}
