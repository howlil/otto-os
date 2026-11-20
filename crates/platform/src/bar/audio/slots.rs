use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback};
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, IAudioSessionControl, IAudioSessionEvents, IAudioSessionManager2,
    IAudioSessionNotification, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture,
    eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::Result;

use super::listeners::{
    CaptureSessionEvents, CaptureSessionNotification, CaptureVolumeListener, VolumeListener,
};
use super::{
    AVAILABLE, AudioCommand, CAPTURE_LOADED, MIC_AVAILABLE, MIC_MUTED, MIC_SESSION_ACTIVE, MUTED,
    percent_from_scalar, store_state,
};

pub(super) struct EndpointSlot {
    enumerator: Option<IMMDeviceEnumerator>,
    endpoint: Option<IAudioEndpointVolume>,
    listener: IAudioEndpointVolumeCallback,
    listening: bool,
}

impl EndpointSlot {
    pub(super) fn new() -> Self {
        Self {
            enumerator: None,
            endpoint: None,
            listener: VolumeListener.into(),
            listening: false,
        }
    }

    pub(super) fn enumerator(&mut self) -> Option<&IMMDeviceEnumerator> {
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

    pub(super) fn listening(&self) -> bool {
        self.endpoint.is_some() && self.listening
    }

    pub(super) fn invalidate(&mut self) {
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

pub(super) struct CaptureSlot {
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
    pub(super) fn new(worker: Sender<AudioCommand>) -> Self {
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

    pub(super) fn refresh(&mut self) {
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

    pub(super) fn refresh_sessions(&mut self) {
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

    pub(super) fn apply_mute(&mut self, muted: bool) {
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

    pub(super) fn invalidate(&mut self) {
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

pub(super) fn refresh(slot: &mut EndpointSlot) {
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

pub(super) fn apply_volume(slot: &mut EndpointSlot, percent: u8) {
    let applied = with_endpoint(slot, |endpoint| unsafe {
        endpoint.SetMasterVolumeLevelScalar(f32::from(percent) / 100.0, std::ptr::null())?;
        endpoint.SetMute(percent == 0, std::ptr::null())
    });
    if applied.is_some() {
        store_state(percent, percent == 0);
    }
}

pub(super) fn apply_mute(slot: &mut EndpointSlot, muted: bool) {
    let applied = with_endpoint(slot, |endpoint| unsafe {
        endpoint.SetMute(muted, std::ptr::null())
    });
    if applied.is_some() {
        MUTED.store(muted, Ordering::Relaxed);
    }
}
