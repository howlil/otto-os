use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32};
use std::sync::mpsc::SyncSender;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use otto_core::{BluetoothConnectTracker, BluetoothState, NotchHudKind, WifiState};
use windows::Win32::Foundation::DEVPROPKEY;
use windows::core::GUID;

pub const WM_APP_CONTROL_CENTER_REFRESH: u32 = 0x8000 + 25;

pub(super) const CACHE_TTL: Duration = Duration::from_secs(2);
pub(super) const PENDING_STALE: Duration = Duration::from_secs(4);
pub(super) const BUSY_HOLD: Duration = Duration::from_millis(2_500);
pub(super) const OP_TIMEOUT: Duration = Duration::from_millis(1_500);
pub(super) const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
pub(super) const QUEUE_DEPTH: usize = 16;
pub(super) const MAX_WIFI_NETWORKS: usize = 16;
pub(super) const MAX_BLUETOOTH_DEVICES: usize = 16;
pub(super) const BT_DEBOUNCE_STEPS: usize = 14;
pub(super) const BT_DEBOUNCE_SLICE: Duration = Duration::from_millis(25);
pub(super) static DEVPKEY_BLUETOOTH_BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104E_A319_6EE2_4701_BD47_8DDB_F425_BBE5),
    pid: 2,
};
pub(super) static DEVPKEY_BLUETOOTH_DEVICE_ADDRESS: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x2BD6_7D8B_8BEB_48D5_87E0_6CDA_3428_040A),
    pid: 1,
};

pub(super) enum ConnectivityTask {
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

pub(super) const fn bluetooth_base_uuid(uuid16: u32) -> GUID {
    GUID::from_values(
        uuid16,
        0x0000,
        0x1000,
        [0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB],
    )
}

#[derive(Default)]
pub(super) struct ConnectivityCache {
    pub(super) wifi: WifiState,
    pub(super) bluetooth: BluetoothState,
    pub(super) bt_tracker: BluetoothConnectTracker,
    pub(super) pending_bluetooth_hud: Option<NotchHudKind>,
    pub(super) wifi_refreshed_at: Option<Instant>,
    pub(super) wifi_requested_at: Option<Instant>,
    pub(super) wifi_busy_until: Option<Instant>,
    pub(super) bt_refreshed_at: Option<Instant>,
    pub(super) bt_requested_at: Option<Instant>,
    pub(super) bt_busy_until: Option<Instant>,
}

impl ConnectivityCache {
    pub(super) fn snapshot_wifi(&mut self, now: Instant) -> (WifiState, bool) {
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

    pub(super) fn snapshot_bluetooth(&mut self, now: Instant) -> (BluetoothState, bool) {
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

    pub(super) fn store_wifi(&mut self, mut state: WifiState, now: Instant) {
        state.busy = false;
        self.wifi = state;
        self.wifi_refreshed_at = Some(now);
        self.wifi_requested_at = None;
        self.wifi_busy_until = None;
    }

    pub(super) fn store_bluetooth(&mut self, mut state: BluetoothState, now: Instant) -> bool {
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

    pub(super) fn take_pending_bluetooth_hud(&mut self) -> Option<NotchHudKind> {
        self.pending_bluetooth_hud.take()
    }

    pub(super) fn mark_wifi_busy(&mut self, enabled: Option<bool>, now: Instant) {
        if let Some(val) = enabled {
            self.wifi.enabled = val;
        }
        self.wifi.busy = true;
        self.wifi_busy_until = Some(now + BUSY_HOLD);
        self.wifi_requested_at = Some(now);
    }

    pub(super) fn mark_bt_busy(&mut self, enabled: Option<bool>, now: Instant) {
        if let Some(val) = enabled {
            self.bluetooth.enabled = val;
        }
        self.bluetooth.busy = true;
        self.bt_busy_until = Some(now + BUSY_HOLD);
        self.bt_requested_at = Some(now);
    }
}

pub(super) struct ConnectivityRuntime {
    pub(super) sender: SyncSender<ConnectivityTask>,
    pub(super) handle: JoinHandle<()>,
}

pub(super) static CACHE: Mutex<Option<ConnectivityCache>> = Mutex::new(None);
pub(super) static WORKER: Mutex<Option<ConnectivityRuntime>> = Mutex::new(None);
pub(super) static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
pub(super) static BT_HUD_TARGET: AtomicIsize = AtomicIsize::new(0);
pub(super) static BT_HUD_MSG: AtomicU32 = AtomicU32::new(0);
pub(super) static BT_HARDWARE_DIRTY: AtomicBool = AtomicBool::new(false);
