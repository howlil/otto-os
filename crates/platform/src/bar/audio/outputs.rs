use otto_core::AudioOutputDevice;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IMMDevice, eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::core::{PCWSTR, PWSTR, Result};

use super::slots::{EndpointSlot, refresh};

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

pub(super) fn enumerate_audio_outputs(slot: &mut EndpointSlot) -> Vec<AudioOutputDevice> {
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

pub(super) fn apply_audio_output(slot: &mut EndpointSlot, id: &str) -> bool {
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

    pub(super) const CLSID_POLICY_CONFIG_CLIENT: GUID =
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
