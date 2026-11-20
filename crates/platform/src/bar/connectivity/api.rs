use std::sync::atomic::Ordering;
use std::time::Instant;

use otto_core::{BluetoothState, WifiState};

use crate::worker::join_within;

use super::types::ConnectivityTask;
use super::types::SHUTDOWN_TIMEOUT;
use super::types::SHUTTING_DOWN;
use super::types::WORKER;
use super::worker::enqueue;
use super::worker::is_shutting_down;
use super::worker::with_cache;

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
