use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

use otto_core::{WifiNetwork, WifiState};
use windows::Devices::Radios::RadioKind;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_AVAILABLE_NETWORK_CONNECTED, WLAN_AVAILABLE_NETWORK_HAS_PROFILE,
    WLAN_AVAILABLE_NETWORK_LIST, WLAN_CONNECTION_PARAMETERS, WLAN_INTERFACE_INFO_LIST,
    WLAN_PHY_RADIO_STATE, WLAN_RADIO_STATE, WlanCloseHandle, WlanConnect, WlanEnumInterfaces,
    WlanFreeMemory, WlanGetAvailableNetworkList, WlanOpenHandle, WlanQueryInterface, WlanScan,
    WlanSetInterface, dot11_radio_state_off, dot11_radio_state_on, dot11_radio_state_unknown,
    wlan_connection_mode_profile, wlan_intf_opcode_radio_state,
};
use windows::core::{GUID, PCWSTR};

use super::bluetooth::read_radio_state;
use super::bluetooth::set_radio_state;
use super::types::MAX_WIFI_NETWORKS;

pub(super) struct WlanClient(HANDLE);

impl Drop for WlanClient {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { WlanCloseHandle(self.0, None) };
        }
    }
}

pub(super) struct WlanMemory(*const c_void);

impl Drop for WlanMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WlanFreeMemory(self.0) };
        }
    }
}

pub(super) fn open_wlan_client() -> Option<WlanClient> {
    let mut negotiated = 0u32;
    let mut handle = HANDLE::default();
    let status = unsafe { WlanOpenHandle(2, None, &raw mut negotiated, &raw mut handle) };
    (status == 0 && !handle.is_invalid()).then_some(WlanClient(handle))
}

pub(super) fn wlan_interfaces(client: &WlanClient) -> Option<(WlanMemory, Vec<GUID>)> {
    let mut ptr: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
    let status = unsafe { WlanEnumInterfaces(client.0, None, &raw mut ptr) };
    if status != 0 || ptr.is_null() {
        return None;
    }
    let guard = WlanMemory(ptr.cast());
    let count = unsafe { (*ptr).dwNumberOfItems as usize };
    let items = unsafe { std::slice::from_raw_parts((*ptr).InterfaceInfo.as_ptr(), count) };
    let guids = items.iter().map(|info| info.InterfaceGuid).collect();
    Some((guard, guids))
}

pub(super) fn query_wlan_radio(client: &WlanClient, guid: &GUID) -> Option<(bool, usize)> {
    let mut size = 0u32;
    let mut data: *mut c_void = ptr::null_mut();
    let status = unsafe {
        WlanQueryInterface(
            client.0,
            guid,
            wlan_intf_opcode_radio_state,
            None,
            &raw mut size,
            &raw mut data,
            None,
        )
    };
    if status != 0 || data.is_null() || (size as usize) < size_of::<WLAN_RADIO_STATE>() {
        if !data.is_null() {
            unsafe { WlanFreeMemory(data) };
        }
        return None;
    }
    let _guard = WlanMemory(data);
    let state = unsafe { &*data.cast::<WLAN_RADIO_STATE>() };
    let phys_len = (state.dwNumberOfPhys as usize).min(state.PhyRadioState.len());
    let phys = &state.PhyRadioState[..phys_len];
    let software_on = phys
        .iter()
        .any(|phy| phy.dot11SoftwareRadioState == dot11_radio_state_on);
    let hardware_on = phys
        .iter()
        .all(|phy| phy.dot11HardwareRadioState != dot11_radio_state_off);
    Some((software_on && hardware_on, phys_len.max(1)))
}

pub(super) fn query_wifi_state() -> WifiState {
    let Some(client) = open_wlan_client() else {
        return fallback_winrt_wifi_state();
    };
    let Some((_mem, guids)) = wlan_interfaces(&client) else {
        return WifiState::default();
    };
    let Some(first_guid) = guids.first() else {
        return WifiState::default();
    };
    let (enabled, _) = query_wlan_radio(&client, first_guid)
        .unwrap_or_else(|| (read_radio_state(RadioKind::WiFi).unwrap_or(false), 1));
    if !enabled {
        return WifiState {
            supported: true,
            enabled: false,
            busy: false,
            connected_ssid: None,
            networks: Vec::new(),
        };
    }

    let mut raw_networks = Vec::new();
    for guid in &guids {
        collect_interface_networks(&client, guid, &mut raw_networks);
    }
    let networks = normalize_wifi_networks(raw_networks);
    let connected_ssid = networks
        .iter()
        .find(|net| net.connected)
        .map(|net| net.ssid.clone());

    WifiState {
        supported: true,
        enabled: true,
        busy: false,
        connected_ssid,
        networks,
    }
}

pub(super) fn fallback_winrt_wifi_state() -> WifiState {
    match read_radio_state(RadioKind::WiFi) {
        Some(enabled) => WifiState {
            supported: true,
            enabled,
            busy: false,
            connected_ssid: None,
            networks: Vec::new(),
        },
        None => WifiState::default(),
    }
}

pub(super) fn collect_interface_networks(
    client: &WlanClient,
    guid: &GUID,
    out: &mut Vec<WifiNetwork>,
) {
    let mut ptr: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
    let status = unsafe { WlanGetAvailableNetworkList(client.0, guid, 0, None, &raw mut ptr) };
    if status != 0 || ptr.is_null() {
        return;
    }
    let _guard = WlanMemory(ptr.cast());
    let count = unsafe { (*ptr).dwNumberOfItems as usize };
    let items = unsafe { std::slice::from_raw_parts((*ptr).Network.as_ptr(), count) };
    for net in items {
        let len = (net.dot11Ssid.uSSIDLength as usize).min(net.dot11Ssid.ucSSID.len());
        if len == 0 {
            continue;
        }
        let ssid = String::from_utf8_lossy(&net.dot11Ssid.ucSSID[..len]).into_owned();
        if ssid.trim().is_empty() {
            continue;
        }
        out.push(WifiNetwork {
            ssid,
            signal_percent: net.wlanSignalQuality.min(100) as u8,
            secured: net.bSecurityEnabled.as_bool(),
            connected: (net.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED) != 0,
            saved: (net.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE) != 0,
        });
    }
}

pub(super) fn set_wifi_enabled_hw(enabled: bool) -> bool {
    if let Some(client) = open_wlan_client()
        && let Some((_mem, guids)) = wlan_interfaces(&client)
    {
        let mut any_ok = false;
        let state_val = if enabled {
            dot11_radio_state_on
        } else {
            dot11_radio_state_off
        };
        for guid in &guids {
            let phy_count = query_wlan_radio(&client, guid).map_or(1, |(_, n)| n);
            for phy_index in 0..phy_count {
                let phy_state = WLAN_PHY_RADIO_STATE {
                    dwPhyIndex: phy_index as u32,
                    dot11SoftwareRadioState: state_val,
                    dot11HardwareRadioState: dot11_radio_state_unknown,
                };
                let status = unsafe {
                    WlanSetInterface(
                        client.0,
                        guid,
                        wlan_intf_opcode_radio_state,
                        size_of::<WLAN_PHY_RADIO_STATE>() as u32,
                        (&raw const phy_state).cast(),
                        None,
                    )
                };
                if status == 0 {
                    any_ok = true;
                }
            }
            if enabled {
                let _ = unsafe { WlanScan(client.0, guid, None, None, None) };
            }
        }
        if any_ok {
            return true;
        }
    }
    set_radio_state(RadioKind::WiFi, enabled)
}

pub(super) fn connect_wifi_hw(target_ssid: &str) -> bool {
    let Some(client) = open_wlan_client() else {
        return false;
    };
    let Some((_mem, guids)) = wlan_interfaces(&client) else {
        return false;
    };
    for guid in &guids {
        let mut ptr: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
        let status = unsafe { WlanGetAvailableNetworkList(client.0, guid, 0, None, &raw mut ptr) };
        if status != 0 || ptr.is_null() {
            continue;
        }
        let _guard = WlanMemory(ptr.cast());
        let count = unsafe { (*ptr).dwNumberOfItems as usize };
        let items = unsafe { std::slice::from_raw_parts((*ptr).Network.as_ptr(), count) };
        for net in items {
            let len = (net.dot11Ssid.uSSIDLength as usize).min(net.dot11Ssid.ucSSID.len());
            if len == 0 || (net.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE) == 0 {
                continue;
            }
            let ssid = String::from_utf8_lossy(&net.dot11Ssid.ucSSID[..len]);
            if ssid != target_ssid {
                continue;
            }
            let end = net
                .strProfileName
                .iter()
                .position(|&ch| ch == 0)
                .unwrap_or(net.strProfileName.len());
            if end == 0 {
                continue;
            }
            let mut profile_w = net.strProfileName[..end].to_vec();
            profile_w.push(0);
            let params = WLAN_CONNECTION_PARAMETERS {
                wlanConnectionMode: wlan_connection_mode_profile,
                strProfile: PCWSTR(profile_w.as_ptr()),
                pDot11Ssid: ptr::null_mut(),
                pDesiredBssidList: ptr::null_mut(),
                dot11BssType: net.dot11BssType,
                dwFlags: 0,
            };
            let res = unsafe { WlanConnect(client.0, guid, &raw const params, None) };
            if res == 0 {
                return true;
            }
        }
    }
    false
}

pub(crate) fn normalize_wifi_networks(raw: Vec<WifiNetwork>) -> Vec<WifiNetwork> {
    let mut merged: Vec<WifiNetwork> = Vec::new();
    for item in raw {
        if let Some(existing) = merged.iter_mut().find(|net| net.ssid == item.ssid) {
            existing.signal_percent = existing.signal_percent.max(item.signal_percent);
            existing.secured |= item.secured;
            existing.connected |= item.connected;
            existing.saved |= item.saved;
        } else {
            merged.push(item);
        }
    }
    merged.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.saved.cmp(&a.saved))
            .then_with(|| b.signal_percent.cmp(&a.signal_percent))
            .then_with(|| a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase()))
    });
    merged.truncate(MAX_WIFI_NETWORKS);
    merged
}
