#![allow(clippy::inline_always, clippy::ref_as_ptr)]

use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::{
    AUDIO_VOLUME_NOTIFICATION_DATA, AudioSessionDisconnectReason, AudioSessionState, DEVICE_STATE,
    EDataFlow, ERole, IAudioSessionControl, IAudioSessionEvents, IAudioSessionEvents_Impl,
    IAudioSessionNotification, IAudioSessionNotification_Impl, IMMDeviceEnumerator,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
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
    fn OnSessionCreated(&self, _newsession: Ref<'_, IAudioSessionControl>) -> Result<()> {
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

    fn OnIconPathChanged(&self, _newiconpath: &PCWSTR, _eventcontext: *const GUID) -> Result<()> {
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

    fn OnSessionDisconnected(&self, _disconnectreason: AudioSessionDisconnectReason) -> Result<()> {
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
        crate::bar::connectivity::notify_bluetooth_hardware_change();
        crate::idle_wake::request_idle_wake();
        Ok(())
    }

    fn OnDeviceAdded(&self, _device: &PCWSTR) -> Result<()> {
        self.notify_worker(AudioCommand::CaptureDeviceChanged);
        crate::bar::connectivity::notify_bluetooth_hardware_change();
        crate::idle_wake::request_idle_wake();
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device: &PCWSTR) -> Result<()> {
        self.notify_worker(AudioCommand::CaptureDeviceChanged);
        crate::idle_wake::request_idle_wake();
        Ok(())
    }

    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _device: &PCWSTR) -> Result<()> {
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

pub(super) struct DeviceSubscription {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

impl DeviceSubscription {
    pub(super) fn register(worker: Sender<AudioCommand>) -> Option<Self> {
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
