use std::collections::BTreeMap;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{
    BluetoothConnectTracker, BluetoothDevice, BluetoothDeviceKind, BluetoothState, NotchHudKind,
    WifiNetwork, WifiState,
};
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
use windows::Win32::Foundation::{
    CloseHandle, DEVPROPKEY, ERROR_MORE_DATA, HANDLE, HWND, LPARAM, WPARAM,
};
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_AVAILABLE_NETWORK_CONNECTED, WLAN_AVAILABLE_NETWORK_HAS_PROFILE,
    WLAN_AVAILABLE_NETWORK_LIST, WLAN_CONNECTION_PARAMETERS, WLAN_INTERFACE_INFO_LIST,
    WLAN_PHY_RADIO_STATE, WLAN_RADIO_STATE, WlanCloseHandle, WlanConnect, WlanEnumInterfaces,
    WlanFreeMemory, WlanGetAvailableNetworkList, WlanOpenHandle, WlanQueryInterface, WlanScan,
    WlanSetInterface, dot11_radio_state_off, dot11_radio_state_on, dot11_radio_state_unknown,
    wlan_connection_mode_profile, wlan_intf_opcode_radio_state,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, PostMessageW,
};
use windows::core::{GUID, PCWSTR, w};

use crate::com::ComGuard;
use crate::winrt_io::await_until;
use crate::worker::join_within;

pub const WM_APP_CONTROL_CENTER_REFRESH: u32 = 0x8000 + 25;

const CACHE_TTL: Duration = Duration::from_secs(2);
const PENDING_STALE: Duration = Duration::from_secs(4);
const BUSY_HOLD: Duration = Duration::from_millis(2_500);
const OP_TIMEOUT: Duration = Duration::from_millis(1_500);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const QUEUE_DEPTH: usize = 16;
const MAX_WIFI_NETWORKS: usize = 16;
const MAX_BLUETOOTH_DEVICES: usize = 16;
const BT_DEBOUNCE_STEPS: usize = 14;
const BT_DEBOUNCE_SLICE: Duration = Duration::from_millis(25);
static DEVPKEY_BLUETOOTH_BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104E_A319_6EE2_4701_BD47_8DDB_F425_BBE5),
    pid: 2,
};
static DEVPKEY_BLUETOOTH_DEVICE_ADDRESS: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x2BD6_7D8B_8BEB_48D5_87E0_6CDA_3428_040A),
    pid: 1,
};

enum ConnectivityTask {
    RefreshWifi,
    RefreshBluetooth,
    SeedBluetoothBaseline,
    RefreshBluetoothDebounced,
    SetWifiEnabled(bool),
    ConnectWifi(String),
    SetBluetoothEnabled(bool),
    ToggleBluetoothDevice { id: String, connect: bool },
    Shutdown,
}

const fn bluetooth_base_uuid(uuid16: u32) -> GUID {
    GUID::from_values(
        uuid16,
        0x0000,
        0x1000,
        [0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB],
    )
}

#[derive(Default)]
struct ConnectivityCache {
    wifi: WifiState,
    bluetooth: BluetoothState,
    bt_tracker: BluetoothConnectTracker,
    pending_bluetooth_hud: Option<NotchHudKind>,
    wifi_refreshed_at: Option<Instant>,
    wifi_requested_at: Option<Instant>,
    wifi_busy_until: Option<Instant>,
    bt_refreshed_at: Option<Instant>,
    bt_requested_at: Option<Instant>,
    bt_busy_until: Option<Instant>,
}

impl ConnectivityCache {
    fn snapshot_wifi(&mut self, now: Instant) -> (WifiState, bool) {
        if self
            .wifi_requested_at
            .is_some_and(|at| now.saturating_duration_since(at) >= PENDING_STALE)
        {
            self.wifi_requested_at = None;
        }
        let fresh = self
            .wifi_refreshed_at
            .is_some_and(|at| now.saturating_duration_since(at) < CACHE_TTL);
        let busy_active = self.wifi_busy_until.is_some_and(|until| now < until);
        let request = !fresh && self.wifi_requested_at.is_none();
        if request {
            self.wifi_requested_at = Some(now);
        }
        let mut out = self.wifi.clone();
        out.busy = out.busy || busy_active;
        (out, request)
    }

    fn snapshot_bluetooth(&mut self, now: Instant) -> (BluetoothState, bool) {
        if self
            .bt_requested_at
            .is_some_and(|at| now.saturating_duration_since(at) >= PENDING_STALE)
        {
            self.bt_requested_at = None;
        }
        let fresh = self
            .bt_refreshed_at
            .is_some_and(|at| now.saturating_duration_since(at) < CACHE_TTL);
        let busy_active = self.bt_busy_until.is_some_and(|until| now < until);
        let request = !fresh && self.bt_requested_at.is_none();
        if request {
            self.bt_requested_at = Some(now);
        }
        let mut out = self.bluetooth.clone();
        out.busy = out.busy || busy_active;
        (out, request)
    }

    fn store_wifi(&mut self, mut state: WifiState, now: Instant) {
        state.busy = false;
        self.wifi = state;
        self.wifi_refreshed_at = Some(now);
        self.wifi_requested_at = None;
        self.wifi_busy_until = None;
    }

    fn store_bluetooth(&mut self, mut state: BluetoothState, now: Instant) -> bool {
        state.busy = false;
        let hud = self.bt_tracker.observe(&state);
        if let Some(event) = hud {
            self.pending_bluetooth_hud = Some(event);
        }
        self.bluetooth = state;
        self.bt_refreshed_at = Some(now);
        self.bt_requested_at = None;
        self.bt_busy_until = None;
        hud.is_some()
    }

    fn take_pending_bluetooth_hud(&mut self) -> Option<NotchHudKind> {
        self.pending_bluetooth_hud.take()
    }

    fn mark_wifi_busy(&mut self, enabled: Option<bool>, now: Instant) {
        if let Some(val) = enabled {
            self.wifi.enabled = val;
        }
        self.wifi.busy = true;
        self.wifi_busy_until = Some(now + BUSY_HOLD);
        self.wifi_requested_at = Some(now);
    }

    fn mark_bt_busy(&mut self, enabled: Option<bool>, now: Instant) {
        if let Some(val) = enabled {
            self.bluetooth.enabled = val;
        }
        self.bluetooth.busy = true;
        self.bt_busy_until = Some(now + BUSY_HOLD);
        self.bt_requested_at = Some(now);
    }
}

struct ConnectivityRuntime {
    sender: SyncSender<ConnectivityTask>,
    handle: JoinHandle<()>,
}

static CACHE: Mutex<Option<ConnectivityCache>> = Mutex::new(None);
static WORKER: Mutex<Option<ConnectivityRuntime>> = Mutex::new(None);
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static BT_HUD_TARGET: AtomicIsize = AtomicIsize::new(0);
static BT_HUD_MSG: AtomicU32 = AtomicU32::new(0);
static BT_HARDWARE_DIRTY: AtomicBool = AtomicBool::new(false);

fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::Acquire)
}

pub(crate) fn resume_connectivity_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

fn with_cache<T>(action: impl FnOnce(&mut ConnectivityCache) -> T) -> Option<T> {
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(ConnectivityCache::default);
    Some(action(cache))
}

fn enqueue(task: ConnectivityTask) -> bool {
    if is_shutting_down() {
        return false;
    }
    let Ok(mut guard) = WORKER.lock() else {
        return false;
    };
    if is_shutting_down() {
        return false;
    }
    if guard.is_none() {
        let (sender, receiver) = sync_channel(QUEUE_DEPTH);
        let Ok(handle) = thread::Builder::new()
            .name(String::from("otto-connectivity"))
            .spawn(move || worker_loop(&receiver))
        else {
            return false;
        };
        *guard = Some(ConnectivityRuntime { sender, handle });
    }
    let Some(runtime) = guard.as_ref() else {
        return false;
    };
    match runtime.sender.try_send(task) {
        Ok(()) => true,
        Err(mpsc::TrySendError::Full(_)) => false,
        Err(mpsc::TrySendError::Disconnected(_)) => {
            *guard = None;
            false
        }
    }
}

fn worker_loop(receiver: &Receiver<ConnectivityTask>) {
    let _com = ComGuard::multithreaded();
    while let Ok(task) = receiver.recv() {
        if is_shutting_down() {
            break;
        }
        match task {
            ConnectivityTask::Shutdown => break,
            ConnectivityTask::RefreshWifi => {
                let state = query_wifi_state();
                let _ = with_cache(|cache| cache.store_wifi(state, Instant::now()));
            }
            ConnectivityTask::RefreshBluetooth => {
                let state = query_bluetooth_state();
                if with_cache(|cache| cache.store_bluetooth(state, Instant::now())) == Some(true)
                    && !is_shutting_down()
                {
                    post_bluetooth_hud_event();
                }
            }
            ConnectivityTask::SeedBluetoothBaseline => {
                let state = query_bluetooth_state();
                let _ = with_cache(|cache| cache.store_bluetooth(state, Instant::now()));
                continue;
            }
            ConnectivityTask::RefreshBluetoothDebounced => {
                if !wait_for_bluetooth_debounce() {
                    break;
                }
                BT_HARDWARE_DIRTY.store(false, Ordering::Release);
                let state = query_bluetooth_state();
                if with_cache(|cache| cache.store_bluetooth(state, Instant::now())) == Some(true)
                    && !is_shutting_down()
                {
                    post_bluetooth_hud_event();
                }
            }
            ConnectivityTask::SetWifiEnabled(enabled) => {
                let _ = set_wifi_enabled_hw(enabled);
                let state = query_wifi_state();
                let _ = with_cache(|cache| cache.store_wifi(state, Instant::now()));
            }
            ConnectivityTask::ConnectWifi(ssid) => {
                let _ = connect_wifi_hw(&ssid);
                let state = query_wifi_state();
                let _ = with_cache(|cache| cache.store_wifi(state, Instant::now()));
            }
            ConnectivityTask::SetBluetoothEnabled(enabled) => {
                let _ = set_bluetooth_enabled_hw(enabled);
                let state = query_bluetooth_state();
                if with_cache(|cache| cache.store_bluetooth(state, Instant::now())) == Some(true)
                    && !is_shutting_down()
                {
                    post_bluetooth_hud_event();
                }
            }
            ConnectivityTask::ToggleBluetoothDevice { id, connect } => {
                let _ = toggle_bluetooth_device_hw(&id, connect);
                let state = query_bluetooth_state();
                if with_cache(|cache| cache.store_bluetooth(state, Instant::now())) == Some(true)
                    && !is_shutting_down()
                {
                    post_bluetooth_hud_event();
                }
            }
        }
        if !is_shutting_down() {
            post_control_center_refresh();
        }
    }
}

pub(crate) fn post_control_center_refresh() {
    let Ok(hwnd) = (unsafe { FindWindowW(w!("OttoControlCenter"), PCWSTR::null()) }) else {
        return;
    };
    if hwnd.is_invalid() {
        return;
    }
    let mut window_pid = 0u32;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut window_pid)) };
    if window_pid == unsafe { GetCurrentProcessId() } {
        let _ = unsafe {
            PostMessageW(
                Some(hwnd),
                WM_APP_CONTROL_CENTER_REFRESH,
                WPARAM(0),
                LPARAM(0),
            )
        };
    }
}

fn wait_for_bluetooth_debounce() -> bool {
    for _ in 0..BT_DEBOUNCE_STEPS {
        if is_shutting_down() {
            return false;
        }
        thread::sleep(BT_DEBOUNCE_SLICE);
    }
    !is_shutting_down()
}

fn post_bluetooth_hud_event() {
    let target = BT_HUD_TARGET.load(Ordering::Acquire);
    let message = BT_HUD_MSG.load(Ordering::Acquire);
    if target == 0 || message == 0 {
        return;
    }
    let _ = unsafe {
        PostMessageW(
            Some(HWND(target as *mut c_void)),
            message,
            WPARAM(0),
            LPARAM(0),
        )
    };
}

pub fn register_bluetooth_hud_target(target_hwnd: isize, message: u32) {
    BT_HARDWARE_DIRTY.store(false, Ordering::Release);
    BT_HUD_MSG.store(message, Ordering::Release);
    BT_HUD_TARGET.store(target_hwnd, Ordering::Release);
    if target_hwnd != 0 && message != 0 {
        let _ = enqueue(ConnectivityTask::SeedBluetoothBaseline);
    }
}

pub fn unregister_bluetooth_hud_target() {
    BT_HUD_TARGET.store(0, Ordering::Release);
    BT_HUD_MSG.store(0, Ordering::Release);
    BT_HARDWARE_DIRTY.store(false, Ordering::Release);
}

pub fn notify_bluetooth_hardware_change() {
    if is_shutting_down() || BT_HUD_TARGET.load(Ordering::Acquire) == 0 {
        return;
    }
    if !BT_HARDWARE_DIRTY.swap(true, Ordering::AcqRel)
        && !enqueue(ConnectivityTask::RefreshBluetoothDebounced)
    {
        BT_HARDWARE_DIRTY.store(false, Ordering::Release);
    }
}

#[must_use]
pub fn take_bluetooth_hud_event() -> Option<NotchHudKind> {
    with_cache(ConnectivityCache::take_pending_bluetooth_hud).flatten()
}

#[must_use]
pub fn read_wifi_state() -> WifiState {
    if is_shutting_down() {
        return WifiState::default();
    }
    let now = Instant::now();
    let Some((state, request)) = with_cache(|cache| cache.snapshot_wifi(now)) else {
        return WifiState::default();
    };
    if request {
        let _ = enqueue(ConnectivityTask::RefreshWifi);
    }
    state
}

#[must_use]
pub fn set_wifi_enabled(enabled: bool) -> bool {
    let supported = with_cache(|cache| cache.wifi.supported).unwrap_or(false);
    if !supported {
        return false;
    }
    let now = Instant::now();
    let _ = with_cache(|cache| cache.mark_wifi_busy(Some(enabled), now));
    enqueue(ConnectivityTask::SetWifiEnabled(enabled))
}

#[must_use]
pub fn connect_wifi(ssid: &str) -> bool {
    if ssid.is_empty() {
        return false;
    }
    let saved = with_cache(|cache| {
        cache
            .wifi
            .networks
            .iter()
            .find(|net| net.ssid == ssid)
            .is_some_and(|net| net.saved)
    })
    .unwrap_or(false);
    if !saved {
        return false;
    }
    let now = Instant::now();
    let _ = with_cache(|cache| cache.mark_wifi_busy(None, now));
    enqueue(ConnectivityTask::ConnectWifi(String::from(ssid)))
}

#[must_use]
pub fn read_bluetooth_state() -> BluetoothState {
    if is_shutting_down() {
        return BluetoothState::default();
    }
    let now = Instant::now();
    let Some((state, request)) = with_cache(|cache| cache.snapshot_bluetooth(now)) else {
        return BluetoothState::default();
    };
    if request {
        let _ = enqueue(ConnectivityTask::RefreshBluetooth);
    }
    state
}

#[must_use]
pub fn set_bluetooth_enabled(enabled: bool) -> bool {
    let supported = with_cache(|cache| cache.bluetooth.supported).unwrap_or(false);
    if !supported {
        return false;
    }
    let now = Instant::now();
    let _ = with_cache(|cache| cache.mark_bt_busy(Some(enabled), now));
    enqueue(ConnectivityTask::SetBluetoothEnabled(enabled))
}

#[must_use]
pub fn toggle_bluetooth_device(id: &str, connect: bool) -> bool {
    if id.is_empty() {
        return false;
    }
    let supported =
        with_cache(|cache| cache.bluetooth.supported && cache.bluetooth.enabled).unwrap_or(false);
    if !supported {
        return false;
    }
    let now = Instant::now();
    let _ = with_cache(|cache| cache.mark_bt_busy(None, now));
    enqueue(ConnectivityTask::ToggleBluetoothDevice {
        id: String::from(id),
        connect,
    })
}

pub fn shutdown_connectivity_task() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    let runtime = WORKER.lock().ok().and_then(|mut guard| guard.take());
    if let Some(runtime) = runtime {
        let _ = runtime.sender.try_send(ConnectivityTask::Shutdown);
        drop(runtime.sender);
        let _ = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    }
}

struct WlanClient(HANDLE);

impl Drop for WlanClient {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { WlanCloseHandle(self.0, None) };
        }
    }
}

struct WlanMemory(*const c_void);

impl Drop for WlanMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WlanFreeMemory(self.0) };
        }
    }
}

fn open_wlan_client() -> Option<WlanClient> {
    let mut negotiated = 0u32;
    let mut handle = HANDLE::default();
    let status = unsafe { WlanOpenHandle(2, None, &raw mut negotiated, &raw mut handle) };
    (status == 0 && !handle.is_invalid()).then_some(WlanClient(handle))
}

fn wlan_interfaces(client: &WlanClient) -> Option<(WlanMemory, Vec<GUID>)> {
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

fn query_wlan_radio(client: &WlanClient, guid: &GUID) -> Option<(bool, usize)> {
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

fn query_wifi_state() -> WifiState {
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

fn fallback_winrt_wifi_state() -> WifiState {
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

fn collect_interface_networks(client: &WlanClient, guid: &GUID, out: &mut Vec<WifiNetwork>) {
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

fn set_wifi_enabled_hw(enabled: bool) -> bool {
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

fn connect_wifi_hw(target_ssid: &str) -> bool {
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

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

struct RadioFindGuard(HBLUETOOTH_RADIO_FIND);

impl Drop for RadioFindGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { BluetoothFindRadioClose(self.0) };
        }
    }
}

struct DeviceFindGuard(HBLUETOOTH_DEVICE_FIND);

impl Drop for DeviceFindGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { BluetoothFindDeviceClose(self.0) };
        }
    }
}

fn find_first_bluetooth_radio() -> Option<OwnedHandle> {
    let params = BLUETOOTH_FIND_RADIO_PARAMS {
        dwSize: size_of::<BLUETOOTH_FIND_RADIO_PARAMS>() as u32,
    };
    let mut radio = HANDLE::default();
    let find = unsafe { BluetoothFindFirstRadio(&raw const params, &raw mut radio) }.ok()?;
    let _find_guard = RadioFindGuard(find);
    (!radio.is_invalid()).then_some(OwnedHandle(radio))
}

fn query_bluetooth_state() -> BluetoothState {
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

fn enumerate_bluetooth_devices() -> Vec<BluetoothDevice> {
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

struct DevInfoGuard(HDEVINFO);

impl Drop for DevInfoGuard {
    fn drop(&mut self) {
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

fn query_bluetooth_battery_levels() -> BTreeMap<String, u8> {
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

fn read_dev_property(
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

fn read_pnp_battery_percent(dev_set: HDEVINFO, dev_info: &SP_DEVINFO_DATA) -> Option<u8> {
    let mut buf = [0u8; 4];
    let (prop_type, len) =
        read_dev_property(dev_set, dev_info, DEVPKEY_BLUETOOTH_BATTERY, &mut buf)?;
    parse_pnp_battery_byte(prop_type, &buf[..len])
}

fn read_pnp_bluetooth_address(dev_set: HDEVINFO, dev_info: &SP_DEVINFO_DATA) -> Option<String> {
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

fn decode_utf16_le_bytes(bytes: &[u8]) -> String {
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

fn set_bluetooth_enabled_hw(enabled: bool) -> bool {
    set_radio_state(RadioKind::Bluetooth, enabled)
}

fn toggle_bluetooth_device_hw(id: &str, connect: bool) -> bool {
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

fn installed_bluetooth_services(radio: HANDLE, info: &BLUETOOTH_DEVICE_INFO) -> Vec<GUID> {
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

fn read_radio_state(kind: RadioKind) -> Option<bool> {
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

fn set_radio_state(kind: RadioKind, enabled: bool) -> bool {
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

fn utf16_slice_to_string(slice: &[u16]) -> String {
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use otto_core::{BluetoothDeviceKind, WifiNetwork, WifiState};

    use super::{ConnectivityCache, classify_bluetooth_cod, normalize_wifi_networks};

    #[test]
    fn classifies_bluetooth_device_classes() {
        assert_eq!(
            classify_bluetooth_cod(0x0024_0404),
            BluetoothDeviceKind::Headphones
        );
        assert_eq!(
            classify_bluetooth_cod(0x0024_0414),
            BluetoothDeviceKind::Speaker
        );
        assert_eq!(
            classify_bluetooth_cod(0x0000_2580),
            BluetoothDeviceKind::Mouse
        );
        assert_eq!(
            classify_bluetooth_cod(0x0000_2540),
            BluetoothDeviceKind::Keyboard
        );
        assert_eq!(
            classify_bluetooth_cod(0x005A_020C),
            BluetoothDeviceKind::Phone
        );
    }

    #[test]
    fn normalizes_and_sorts_wifi_networks() {
        let sorted = normalize_wifi_networks(vec![
            WifiNetwork {
                ssid: String::from("Guest"),
                signal_percent: 90,
                secured: false,
                connected: false,
                saved: false,
            },
            WifiNetwork {
                ssid: String::from("Office"),
                signal_percent: 40,
                secured: true,
                connected: false,
                saved: true,
            },
            WifiNetwork {
                ssid: String::from("Office"),
                signal_percent: 75,
                secured: true,
                connected: true,
                saved: false,
            },
        ]);
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].ssid, "Office");
        assert!(sorted[0].connected);
        assert!(sorted[0].saved);
        assert_eq!(sorted[0].signal_percent, 75);
        assert_eq!(sorted[1].ssid, "Guest");
    }

    #[test]
    fn connectivity_cache_requests_refresh_once_per_ttl() {
        let mut cache = ConnectivityCache::default();
        let t0 = Instant::now();
        let (_, req1) = cache.snapshot_wifi(t0);
        let (_, req2) = cache.snapshot_wifi(t0);
        assert!(req1);
        assert!(!req2);

        cache.store_wifi(
            WifiState {
                supported: true,
                enabled: true,
                busy: false,
                connected_ssid: Some(String::from("Home")),
                networks: Vec::new(),
            },
            t0,
        );
        let (state, req3) = cache.snapshot_wifi(t0);
        assert!(state.enabled);
        assert!(!req3);
    }

    static SHUTDOWN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct ResumeGuard;
    impl Drop for ResumeGuard {
        fn drop(&mut self) {
            super::resume_connectivity_task();
        }
    }

    #[test]
    fn worker_exits_on_shutdown_flag_even_when_bounded_queue_is_full() {
        use std::sync::{PoisonError, mpsc::sync_channel};

        let _serial = SHUTDOWN_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = ResumeGuard;
        super::resume_connectivity_task();

        let (sender, receiver) = sync_channel(super::QUEUE_DEPTH);
        for _ in 0..super::QUEUE_DEPTH {
            sender
                .try_send(super::ConnectivityTask::RefreshWifi)
                .unwrap();
        }
        assert!(sender.try_send(super::ConnectivityTask::Shutdown).is_err());

        super::SHUTTING_DOWN.store(true, std::sync::atomic::Ordering::Release);
        super::worker_loop(&receiver);
        assert!(receiver.try_recv().ok().is_some());
    }

    #[test]
    fn shutdown_prevents_connectivity_enqueue_until_resumed() {
        use std::sync::PoisonError;

        let _serial = SHUTDOWN_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = ResumeGuard;
        super::shutdown_connectivity_task();
        assert!(!super::enqueue(super::ConnectivityTask::RefreshWifi));
        assert!(!super::read_wifi_state().supported);
        assert!(!super::read_bluetooth_state().supported);
        assert!(super::WORKER.lock().unwrap().is_none());
    }

    #[test]
    fn normalizes_bluetooth_mac_addresses_and_rejects_invalid_strings() {
        assert_eq!(
            super::normalize_bluetooth_address("d710d8da6c59").as_deref(),
            Some("D710D8DA6C59")
        );
        assert_eq!(
            super::normalize_bluetooth_address("a4:c7:88:65:19:a8").as_deref(),
            Some("A4C7886519A8")
        );
        assert_eq!(super::normalize_bluetooth_address("A4-C7-88"), None);
        assert_eq!(super::normalize_bluetooth_address("G710D8DA6C59"), None);
    }

    #[test]
    fn parses_pnp_battery_bytes_and_attaches_only_to_connected_devices() {
        use std::collections::BTreeMap;
        use windows::Win32::Devices::Properties::{DEVPROP_TYPE_BYTE, DEVPROP_TYPE_STRING};

        assert_eq!(
            super::parse_pnp_battery_byte(DEVPROP_TYPE_BYTE, &[80]),
            Some(80)
        );
        assert_eq!(
            super::parse_pnp_battery_byte(DEVPROP_TYPE_BYTE, &[120]),
            Some(100)
        );
        assert_eq!(
            super::parse_pnp_battery_byte(DEVPROP_TYPE_STRING, &[80]),
            None
        );
        assert_eq!(super::parse_pnp_battery_byte(DEVPROP_TYPE_BYTE, &[]), None);

        let mut devices = vec![
            otto_core::BluetoothDevice {
                id: String::from("A4C7886519A8"),
                name: String::from("Redmi 15"),
                kind: BluetoothDeviceKind::Phone,
                connected: true,
                battery_percent: None,
            },
            otto_core::BluetoothDevice {
                id: String::from("D710D8DA6C59"),
                name: String::from("Pebble K380s"),
                kind: BluetoothDeviceKind::Keyboard,
                connected: false,
                battery_percent: None,
            },
        ];
        let mut batteries = BTreeMap::new();
        batteries.insert(String::from("A4C7886519A8"), 80);
        batteries.insert(String::from("D710D8DA6C59"), 100);
        super::apply_battery_levels(&mut devices, &batteries);
        assert_eq!(devices[0].battery_percent, Some(80));
        assert_eq!(devices[1].battery_percent, None);
    }

    #[test]
    fn connectivity_cache_records_bluetooth_connect_hud_after_baseline_seed() {
        let mut cache = ConnectivityCache::default();
        let t0 = Instant::now();
        let baseline = otto_core::BluetoothState {
            supported: true,
            enabled: true,
            busy: false,
            devices: vec![otto_core::BluetoothDevice {
                id: String::from("D710D8DA6C59"),
                name: String::from("Pebble K380s"),
                kind: BluetoothDeviceKind::Keyboard,
                connected: true,
                battery_percent: Some(100),
            }],
        };
        assert!(!cache.store_bluetooth(baseline, t0));
        assert_eq!(cache.take_pending_bluetooth_hud(), None);

        let connected = otto_core::BluetoothState {
            supported: true,
            enabled: true,
            busy: false,
            devices: vec![
                otto_core::BluetoothDevice {
                    id: String::from("D710D8DA6C59"),
                    name: String::from("Pebble K380s"),
                    kind: BluetoothDeviceKind::Keyboard,
                    connected: true,
                    battery_percent: Some(100),
                },
                otto_core::BluetoothDevice {
                    id: String::from("A4C7886519A8"),
                    name: String::from("WH-1000XM5"),
                    kind: BluetoothDeviceKind::Headphones,
                    connected: true,
                    battery_percent: Some(80),
                },
            ],
        };
        assert!(cache.store_bluetooth(connected, t0));
        assert_eq!(
            cache.take_pending_bluetooth_hud(),
            Some(otto_core::NotchHudKind::BluetoothConnected {
                name: otto_core::HudDeviceLabel::from_name("WH-1000XM5"),
                kind: BluetoothDeviceKind::Headphones,
                battery_percent: Some(80),
            })
        );
        assert_eq!(cache.take_pending_bluetooth_hud(), None);
    }
}
