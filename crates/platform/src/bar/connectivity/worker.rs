use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, sync_channel};
use std::thread::{self};
use std::time::Instant;

use otto_core::NotchHudKind;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, PostMessageW,
};
use windows::core::{PCWSTR, w};

use crate::com::ComGuard;

use super::bluetooth::query_bluetooth_state;
use super::bluetooth::set_bluetooth_enabled_hw;
use super::bluetooth::toggle_bluetooth_device_hw;
use super::types::BT_DEBOUNCE_SLICE;
use super::types::BT_DEBOUNCE_STEPS;
use super::types::BT_HARDWARE_DIRTY;
use super::types::BT_HUD_MSG;
use super::types::BT_HUD_TARGET;
use super::types::CACHE;
use super::types::ConnectivityCache;
use super::types::ConnectivityRuntime;
use super::types::ConnectivityTask;
use super::types::QUEUE_DEPTH;
use super::types::SHUTTING_DOWN;
use super::types::WM_APP_CONTROL_CENTER_REFRESH;
use super::types::WORKER;
use super::wifi::connect_wifi_hw;
use super::wifi::query_wifi_state;
use super::wifi::set_wifi_enabled_hw;

pub(super) fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::Acquire)
}

pub(crate) fn resume_connectivity_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

pub(super) fn with_cache<T>(action: impl FnOnce(&mut ConnectivityCache) -> T) -> Option<T> {
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(ConnectivityCache::default);
    Some(action(cache))
}

pub(super) fn enqueue(task: ConnectivityTask) -> bool {
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

pub(super) fn worker_loop(receiver: &Receiver<ConnectivityTask>) {
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

pub(super) fn wait_for_bluetooth_debounce() -> bool {
    for _ in 0..BT_DEBOUNCE_STEPS {
        if is_shutting_down() {
            return false;
        }
        thread::sleep(BT_DEBOUNCE_SLICE);
    }
    !is_shutting_down()
}

pub(super) fn post_bluetooth_hud_event() {
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
