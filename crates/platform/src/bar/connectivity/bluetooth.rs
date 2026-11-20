use std::collections::BTreeMap;
use std::mem::size_of;
use std::time::Instant;

use otto_core::{BluetoothDevice, BluetoothDeviceKind, BluetoothState};
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::Devices::Bluetooth::{
    AVRemoteControlTargetServiceClassID_UUID16, AudioSinkServiceClassID_UUID16,
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BLUETOOTH_FIND_RADIO_PARAMS,
    BLUETOOTH_SERVICE_DISABLE, BLUETOOTH_SERVICE_ENABLE, BluetoothEnumerateInstalledServices,
    BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindFirstRadio,
    BluetoothFindNextDevice, BluetoothFindRadioClose, BluetoothGetDeviceInfo,
    BluetoothIsConnectable, BluetoothSetServiceState, HBLUETOOTH_DEVICE_FIND,
    HBLUETOOTH_RADIO_FIND, HandsfreeServiceClassID_UUID16,
};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA, SetupDiDestroyDeviceInfoList,
    SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDevicePropertyW,
};
use windows::Win32::Devices::Properties::{DEVPROP_TYPE_BYTE, DEVPROP_TYPE_STRING, DEVPROPTYPE};
use windows::Win32::Foundation::{CloseHandle, DEVPROPKEY, ERROR_MORE_DATA, HANDLE};
use windows::core::{GUID, w};

use crate::winrt_io::await_until;

use super::types::DEVPKEY_BLUETOOTH_BATTERY;
use super::types::DEVPKEY_BLUETOOTH_DEVICE_ADDRESS;
use super::types::MAX_BLUETOOTH_DEVICES;
use super::types::OP_TIMEOUT;
use super::types::bluetooth_base_uuid;

pub(super) struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

pub(super) struct RadioFindGuard(HBLUETOOTH_RADIO_FIND);

impl Drop for RadioFindGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { BluetoothFindRadioClose(self.0) };
        }
    }
}

pub(super) struct DeviceFindGuard(HBLUETOOTH_DEVICE_FIND);

impl Drop for DeviceFindGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { BluetoothFindDeviceClose(self.0) };
        }
    }
}

pub(super) fn find_first_bluetooth_radio() -> Option<OwnedHandle> {
    let params = BLUETOOTH_FIND_RADIO_PARAMS {
        dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
    };
    let mut radio = HANDLE::default();
    let find = unsafe { BluetoothFindFirstRadio(&raw const params, &raw mut radio) }.ok()?;
    let _find_guard = RadioFindGuard(find);
    (!radio.is_invalid()).then_some(OwnedHandle(radio))
}

pub(super) fn query_bluetooth_state() -> BluetoothState {
    let winrt_state = read_radio_state(RadioKind::Bluetooth);
    let win32_radio = find_first_bluetooth_radio();
    let (supported, enabled) = match (winrt_state, win32_radio.as_ref()) {
        (Some(on), _) => (true, on),
        (None, Some(radio)) => (
            true,
            unsafe { BluetoothIsConnectable(Some(radio.0)) }.as_bool(),
        ),
        (None, None) => return BluetoothState::default(),
    };
    if !enabled {
        return BluetoothState {
            supported,
            enabled: false,
            busy: false,
            devices: Vec::new(),
        };
    }
    let devices = enumerate_bluetooth_devices();
    BluetoothState {
        supported: true,
        enabled: true,
        busy: false,
        devices,
    }
}

pub(super) fn enumerate_bluetooth_devices() -> Vec<BluetoothDevice> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: true.into(),
        fReturnRemembered: true.into(),
        fReturnUnknown: false.into(),
        fReturnConnected: true.into(),
        fIssueInquiry: false.into(),
        cTimeoutMultiplier: 0,
        hRadio: HANDLE::default(),
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let Ok(find) = (unsafe { BluetoothFindFirstDevice(&raw const params, &raw mut info) }) else {
        return Vec::new();
    };
    let _guard = DeviceFindGuard(find);

    let mut devices = Vec::new();
    loop {
        let addr = unsafe { info.Address.Anonymous.ullLong };
        let name = utf16_slice_to_string(&info.szName);
        if addr != 0 && !name.trim().is_empty() {
            devices.push(BluetoothDevice {
                id: format!("{addr:012X}"),
                name,
                kind: classify_bluetooth_cod(info.ulClassofDevice),
                connected: info.fConnected.as_bool(),
                battery_percent: None,
            });
        }
        info = BLUETOOTH_DEVICE_INFO {
            dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
            ..Default::default()
        };
        if unsafe { BluetoothFindNextDevice(find, &raw mut info) }.is_err() {
            break;
        }
    }
    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    devices.truncate(MAX_BLUETOOTH_DEVICES);
    if devices.iter().any(|device| device.connected) {
        let batteries = query_bluetooth_battery_levels();
        apply_battery_levels(&mut devices, &batteries);
    }
    devices
}

pub(super) struct DevInfoGuard(HDEVINFO);

impl Drop for DevInfoGuard {
    fn drop(&mut self) {
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

pub(super) fn query_bluetooth_battery_levels() -> BTreeMap<String, u8> {
    let mut batteries = BTreeMap::new();
    for enumerator in [w!("BTHENUM"), w!("BTHLE")] {
        let Ok(handle) = (unsafe {
            SetupDiGetClassDevsW(None, enumerator, None, DIGCF_ALLCLASSES | DIGCF_PRESENT)
        }) else {
            continue;
        };
        let guard = DevInfoGuard(handle);
        let mut index = 0u32;
        loop {
            let mut dev_info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            if unsafe { SetupDiEnumDeviceInfo(guard.0, index, &raw mut dev_info) }.is_err() {
                break;
            }
            index += 1;
            let Some(battery) = read_pnp_battery_percent(guard.0, &dev_info) else {
                continue;
            };
            if let Some(addr) = read_pnp_bluetooth_address(guard.0, &dev_info) {
                batteries.entry(addr).or_insert(battery);
            }
        }
    }
    batteries
}

pub(super) fn read_dev_property(
    dev_set: HDEVINFO,
    dev_info: &SP_DEVINFO_DATA,
    key: DEVPROPKEY,
    buf: &mut [u8],
) -> Option<(DEVPROPTYPE, usize)> {
    let mut prop_type = DEVPROPTYPE::default();
    let mut required = 0u32;
    unsafe {
        SetupDiGetDevicePropertyW(
            dev_set,
            &raw const *dev_info,
            &raw const key,
            &raw mut prop_type,
            Some(buf),
            Some(&raw mut required),
            0,
        )
    }
    .ok()?;
    let len = usize::try_from(required).ok()?.min(buf.len());
    Some((prop_type, len))
}

pub(super) fn read_pnp_battery_percent(
    dev_set: HDEVINFO,
    dev_info: &SP_DEVINFO_DATA,
) -> Option<u8> {
    let mut buf = [0u8; 4];
    let (prop_type, len) =
        read_dev_property(dev_set, dev_info, DEVPKEY_BLUETOOTH_BATTERY, &mut buf)?;
    parse_pnp_battery_byte(prop_type, &buf[..len])
}

pub(super) fn read_pnp_bluetooth_address(
    dev_set: HDEVINFO,
    dev_info: &SP_DEVINFO_DATA,
) -> Option<String> {
    let mut buf = [0u8; 128];
    let (prop_type, len) = read_dev_property(
        dev_set,
        dev_info,
        DEVPKEY_BLUETOOTH_DEVICE_ADDRESS,
        &mut buf,
    )?;
    if prop_type != DEVPROP_TYPE_STRING {
        return None;
    }
    normalize_bluetooth_address(&decode_utf16_le_bytes(&buf[..len]))
}

pub(super) fn decode_utf16_le_bytes(bytes: &[u8]) -> String {
    let mut words = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let code = u16::from_le_bytes([pair[0], pair[1]]);
        if code == 0 {
            break;
        }
        words.push(code);
    }
    String::from_utf16_lossy(&words)
}

pub(crate) fn normalize_bluetooth_address(raw: &str) -> Option<String> {
    let mut hex = String::with_capacity(12);
    for ch in raw.chars() {
        if ch.is_ascii_hexdigit() {
            hex.push(ch.to_ascii_uppercase());
        } else if !matches!(ch, ':' | '-' | ' ' | '\0') {
            return None;
        }
    }
    (hex.len() == 12).then_some(hex)
}

pub(crate) fn parse_pnp_battery_byte(prop_type: DEVPROPTYPE, buf: &[u8]) -> Option<u8> {
    (prop_type == DEVPROP_TYPE_BYTE && !buf.is_empty()).then(|| buf[0].min(100))
}

pub(crate) fn apply_battery_levels(
    devices: &mut [BluetoothDevice],
    batteries: &BTreeMap<String, u8>,
) {
    for device in devices {
        if device.connected {
            device.battery_percent = batteries.get(&device.id).copied();
        }
    }
}

pub(super) fn set_bluetooth_enabled_hw(enabled: bool) -> bool {
    set_radio_state(RadioKind::Bluetooth, enabled)
}

pub(super) fn toggle_bluetooth_device_hw(id: &str, connect: bool) -> bool {
    let Ok(addr) = u64::from_str_radix(id.trim(), 16) else {
        return false;
    };
    let Some(radio) = find_first_bluetooth_radio() else {
        return false;
    };
    let mut info = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    info.Address.Anonymous.ullLong = addr;
    if unsafe { BluetoothGetDeviceInfo(Some(radio.0), &raw mut info) } != 0 {
        return false;
    }

    let mut guids = installed_bluetooth_services(radio.0, &info);
    if guids.is_empty() {
        guids = vec![
            bluetooth_base_uuid(AudioSinkServiceClassID_UUID16),
            bluetooth_base_uuid(HandsfreeServiceClassID_UUID16),
            bluetooth_base_uuid(AVRemoteControlTargetServiceClassID_UUID16),
        ];
    }
    let flag = if connect {
        BLUETOOTH_SERVICE_ENABLE
    } else {
        BLUETOOTH_SERVICE_DISABLE
    };
    let mut any_ok = false;
    for guid in guids {
        let status = unsafe {
            BluetoothSetServiceState(Some(radio.0), &raw const info, &raw const guid, flag)
        };
        if status == 0 {
            any_ok = true;
        }
    }
    any_ok
}

pub(super) fn installed_bluetooth_services(
    radio: HANDLE,
    info: &BLUETOOTH_DEVICE_INFO,
) -> Vec<GUID> {
    let mut count = 16u32;
    let mut guids = vec![GUID::zeroed(); count as usize];
    let status = unsafe {
        BluetoothEnumerateInstalledServices(
            Some(radio),
            &raw const *info,
            &raw mut count,
            Some(guids.as_mut_ptr()),
        )
    };
    if status == ERROR_MORE_DATA.0 && count > 0 {
        guids.resize(count as usize, GUID::zeroed());
        let retry = unsafe {
            BluetoothEnumerateInstalledServices(
                Some(radio),
                &raw const *info,
                &raw mut count,
                Some(guids.as_mut_ptr()),
            )
        };
        if retry == 0 {
            guids.truncate(count as usize);
            return guids;
        }
        return Vec::new();
    }
    if status == 0 {
        guids.truncate(count as usize);
        return guids;
    }
    Vec::new()
}

pub(super) fn read_radio_state(kind: RadioKind) -> Option<bool> {
    let deadline = Instant::now() + OP_TIMEOUT;
    let radios = await_until!(Radio::GetRadiosAsync(), deadline)?;
    let count = radios.Size().ok()?;
    for idx in 0..count {
        let Ok(radio) = radios.GetAt(idx) else {
            continue;
        };
        if radio.Kind().ok() == Some(kind) {
            return Some(radio.State().ok() == Some(RadioState::On));
        }
    }
    None
}

pub(super) fn set_radio_state(kind: RadioKind, enabled: bool) -> bool {
    let deadline = Instant::now() + OP_TIMEOUT;
    let access = await_until!(Radio::RequestAccessAsync(), deadline);
    if !matches!(access, Some(RadioAccessStatus::Allowed)) {
        return false;
    }
    let Some(radios) = await_until!(Radio::GetRadiosAsync(), deadline) else {
        return false;
    };
    let Ok(count) = radios.Size() else {
        return false;
    };
    let target = if enabled {
        RadioState::On
    } else {
        RadioState::Off
    };
    let mut any_ok = false;
    for idx in 0..count {
        let Ok(radio) = radios.GetAt(idx) else {
            continue;
        };
        if radio.Kind().ok() == Some(kind)
            && let Some(status) = await_until!(radio.SetStateAsync(target), deadline)
            && status == RadioAccessStatus::Allowed
        {
            any_ok = true;
        }
    }
    any_ok
}

pub(super) fn utf16_slice_to_string(slice: &[u16]) -> String {
    let end = slice.iter().position(|&ch| ch == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end])
}

#[must_use]
pub(crate) fn classify_bluetooth_cod(cod: u32) -> BluetoothDeviceKind {
    let major = (cod >> 8) & 0x1F;
    match major {
        0x02 => BluetoothDeviceKind::Phone,
        0x04 => {
            let minor = (cod >> 2) & 0x3F;
            match minor {
                0x04 | 0x05 | 0x08 | 0x0A => BluetoothDeviceKind::Speaker,
                _ => BluetoothDeviceKind::Headphones,
            }
        }
        0x05 => {
            let sub = (cod >> 6) & 0x03;
            match sub {
                0x02 => BluetoothDeviceKind::Mouse,
                0x01 | 0x03 => BluetoothDeviceKind::Keyboard,
                _ => BluetoothDeviceKind::Other,
            }
        }
        _ => BluetoothDeviceKind::Other,
    }
}
