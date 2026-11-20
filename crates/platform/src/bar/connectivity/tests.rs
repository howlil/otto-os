use super::bluetooth::*;
use super::types::*;
use super::wifi::*;
use std::time::Instant;

use otto_core::{BluetoothDeviceKind, WifiNetwork, WifiState};

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
        super::worker::resume_connectivity_task();
    }
}

#[test]
fn worker_exits_on_shutdown_flag_even_when_bounded_queue_is_full() {
    use std::sync::{PoisonError, mpsc::sync_channel};

    let _serial = SHUTDOWN_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let _resume = ResumeGuard;
    super::worker::resume_connectivity_task();

    let (sender, receiver) = sync_channel(super::types::QUEUE_DEPTH);
    for _ in 0..super::types::QUEUE_DEPTH {
        sender
            .try_send(super::types::ConnectivityTask::RefreshWifi)
            .unwrap();
    }
    assert!(
        sender
            .try_send(super::types::ConnectivityTask::Shutdown)
            .is_err()
    );

    super::types::SHUTTING_DOWN.store(true, std::sync::atomic::Ordering::Release);
    super::worker::worker_loop(&receiver);
    assert!(receiver.try_recv().ok().is_some());
}

#[test]
fn shutdown_prevents_connectivity_enqueue_until_resumed() {
    use std::sync::PoisonError;

    let _serial = SHUTDOWN_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let _resume = ResumeGuard;
    super::api::shutdown_connectivity_task();
    assert!(!super::worker::enqueue(
        super::types::ConnectivityTask::RefreshWifi
    ));
    assert!(!super::api::read_wifi_state().supported);
    assert!(!super::api::read_bluetooth_state().supported);
    assert!(super::types::WORKER.lock().unwrap().is_none());
}

#[test]
fn normalizes_bluetooth_mac_addresses_and_rejects_invalid_strings() {
    assert_eq!(
        super::bluetooth::normalize_bluetooth_address("d710d8da6c59").as_deref(),
        Some("D710D8DA6C59")
    );
    assert_eq!(
        super::bluetooth::normalize_bluetooth_address("a4:c7:88:65:19:a8").as_deref(),
        Some("A4C7886519A8")
    );
    assert_eq!(
        super::bluetooth::normalize_bluetooth_address("A4-C7-88"),
        None
    );
    assert_eq!(
        super::bluetooth::normalize_bluetooth_address("G710D8DA6C59"),
        None
    );
}

#[test]
fn parses_pnp_battery_bytes_and_attaches_only_to_connected_devices() {
    use std::collections::BTreeMap;
    use windows::Win32::Devices::Properties::{DEVPROP_TYPE_BYTE, DEVPROP_TYPE_STRING};

    assert_eq!(
        super::bluetooth::parse_pnp_battery_byte(DEVPROP_TYPE_BYTE, &[80]),
        Some(80)
    );
    assert_eq!(
        super::bluetooth::parse_pnp_battery_byte(DEVPROP_TYPE_BYTE, &[120]),
        Some(100)
    );
    assert_eq!(
        super::bluetooth::parse_pnp_battery_byte(DEVPROP_TYPE_STRING, &[80]),
        None
    );
    assert_eq!(
        super::bluetooth::parse_pnp_battery_byte(DEVPROP_TYPE_BYTE, &[]),
        None
    );

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
    super::bluetooth::apply_battery_levels(&mut devices, &batteries);
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
