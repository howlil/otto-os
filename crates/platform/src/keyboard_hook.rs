use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::BarKey;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_VOLUME_DOWN,
    VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW,
    PostMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
};

use crate::audio::{is_available, step_system_volume, toggle_system_mute};
use crate::worker::join_within;

const READY_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
const INJECTED_FLAG: u32 = 0x10;
const WHEEL_UP: i16 = 120;
const WHEEL_DOWN: i16 = -120;
const ROUTE_IDLE_LIMIT_MS: u64 = 15_000;

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);
static ROUTE_TARGET: AtomicIsize = AtomicIsize::new(0);
static ROUTE_MESSAGE: AtomicU32 = AtomicU32::new(0);
static ROUTE_DEADLINE_MS: AtomicU64 = AtomicU64::new(0);

struct HookThread {
    thread_id: u32,
    handle: JoinHandle<()>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VolumeKey {
    Up,
    Down,
    Mute,
}

fn volume_key(virtual_key: u32, flags: u32) -> Option<VolumeKey> {
    if flags & INJECTED_FLAG != 0 {
        return None;
    }
    match virtual_key {
        key if key == u32::from(VK_VOLUME_UP.0) => Some(VolumeKey::Up),
        key if key == u32::from(VK_VOLUME_DOWN.0) => Some(VolumeKey::Down),
        key if key == u32::from(VK_VOLUME_MUTE.0) => Some(VolumeKey::Mute),
        _ => None,
    }
}

fn bar_key(virtual_key: u32, flags: u32) -> Option<BarKey> {
    if flags & INJECTED_FLAG != 0 {
        return None;
    }
    let key = u16::try_from(virtual_key).ok()?;
    match key {
        key if key == VK_LEFT.0 => Some(BarKey::Previous),
        key if key == VK_RIGHT.0 => Some(BarKey::Next),
        key if key == VK_HOME.0 => Some(BarKey::First),
        key if key == VK_END.0 => Some(BarKey::Last),
        key if key == VK_RETURN.0 || key == VK_SPACE.0 || key == VK_DOWN.0 => {
            Some(BarKey::Activate)
        }
        key if key == VK_ESCAPE.0 => Some(BarKey::Exit),
        _ => None,
    }
}

fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}

const fn route_live(deadline_ms: u64, now_ms: u64) -> bool {
    deadline_ms != 0 && now_ms < deadline_ms
}

#[must_use]
pub fn route_bar_keys(target_hwnd: isize, message: u32) -> bool {
    let installed = HOOK_THREAD.lock().is_ok_and(|guard| guard.is_some());
    if !installed {
        return false;
    }
    ROUTE_MESSAGE.store(message, Ordering::Release);
    ROUTE_TARGET.store(target_hwnd, Ordering::Release);
    ROUTE_DEADLINE_MS.store(now_ms() + ROUTE_IDLE_LIMIT_MS, Ordering::Release);
    true
}

pub fn stop_routing_bar_keys() {
    ROUTE_TARGET.store(0, Ordering::Release);
    ROUTE_DEADLINE_MS.store(0, Ordering::Release);
}

#[must_use]
pub fn is_routing_bar_keys() -> bool {
    ROUTE_TARGET.load(Ordering::Acquire) != 0
        && route_live(ROUTE_DEADLINE_MS.load(Ordering::Acquire), now_ms())
}

fn route_bar_key(key: BarKey, key_down: bool) -> bool {
    let target = ROUTE_TARGET.load(Ordering::Acquire);
    if target == 0 {
        return false;
    }
    let now = now_ms();
    if !route_live(ROUTE_DEADLINE_MS.load(Ordering::Acquire), now) {
        stop_routing_bar_keys();
        return false;
    }
    if !key_down {
        return true;
    }
    ROUTE_DEADLINE_MS.store(now + ROUTE_IDLE_LIMIT_MS, Ordering::Release);
    let message = ROUTE_MESSAGE.load(Ordering::Acquire);
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            message,
            WPARAM(key.code()),
            LPARAM(0),
        )
    };
    if posted.is_err() {
        stop_routing_bar_keys();
        return false;
    }
    true
}

fn is_key_down(message: usize) -> bool {
    message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize
}

fn apply_volume_key(key: VolumeKey) -> bool {
    match key {
        VolumeKey::Up => step_system_volume(WHEEL_UP).is_some(),
        VolumeKey::Down => step_system_volume(WHEEL_DOWN).is_some(),
        VolumeKey::Mute => toggle_system_mute().is_some(),
    }
}

fn swallow(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 {
        return false;
    }
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if let Some(key) = bar_key(event.vkCode, event.flags.0) {
        return route_bar_key(key, is_key_down(wparam.0));
    }
    let Some(key) = volume_key(event.vkCode, event.flags.0) else {
        return false;
    };
    if is_key_down(wparam.0) {
        apply_volume_key(key)
    } else {
        is_available()
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled = catch_unwind(AssertUnwindSafe(|| swallow(code, wparam, lparam)));
    if matches!(handled, Ok(true)) {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn hook_thread_main(ready: &SyncSender<Option<u32>>) {
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0) };
    let Ok(hook) = hook else {
        let _ = ready.send(None);
        return;
    };
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        let _ = ready.send(Some(GetCurrentThreadId()));
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

pub fn install_keyboard_hook() -> bool {
    let Ok(mut guard) = HOOK_THREAD.lock() else {
        return false;
    };
    if guard.is_some() {
        return true;
    }
    let (ready_sender, ready_receiver) = sync_channel(1);
    let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-keyboard-hook"))
        .spawn(move || hook_thread_main(&ready_sender))
    else {
        return false;
    };
    match ready_receiver.recv_timeout(READY_TIMEOUT) {
        Ok(Some(thread_id)) => {
            *guard = Some(HookThread { thread_id, handle });
            true
        }
        _ => false,
    }
}

pub fn uninstall_keyboard_hook() {
    stop_routing_bar_keys();
    let Some(thread) = HOOK_THREAD.lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    unsafe {
        let _ = PostThreadMessageW(thread.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
    let _ = join_within(thread.handle, SHUTDOWN_TIMEOUT);
}

#[cfg(test)]
mod tests {
    use super::{
        HOOK_THREAD, INJECTED_FLAG, VolumeKey, bar_key, install_keyboard_hook, is_key_down,
        is_routing_bar_keys, route_bar_keys, route_live, uninstall_keyboard_hook, volume_key,
    };
    use otto_core::BarKey;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_A, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB,
        VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
    };
    use windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN};

    #[test]
    fn maps_hardware_volume_keys() {
        assert_eq!(
            volume_key(u32::from(VK_VOLUME_UP.0), 0),
            Some(VolumeKey::Up)
        );
        assert_eq!(
            volume_key(u32::from(VK_VOLUME_DOWN.0), 0),
            Some(VolumeKey::Down)
        );
        assert_eq!(
            volume_key(u32::from(VK_VOLUME_MUTE.0), 0),
            Some(VolumeKey::Mute)
        );
    }

    #[test]
    fn ignores_injected_and_unrelated_keys() {
        assert_eq!(volume_key(u32::from(VK_VOLUME_UP.0), INJECTED_FLAG), None);
        assert_eq!(volume_key(u32::from(VK_A.0), 0), None);
    }

    #[test]
    fn distinguishes_key_down_messages() {
        assert!(is_key_down(WM_KEYDOWN as usize));
        assert!(is_key_down(WM_SYSKEYDOWN as usize));
        assert!(!is_key_down(WM_KEYUP as usize));
    }

    #[test]
    fn maps_navigation_keys_to_bar_keys() {
        let key = |vk: u16| bar_key(u32::from(vk), 0);
        assert_eq!(key(VK_LEFT.0), Some(BarKey::Previous));
        assert_eq!(key(VK_RIGHT.0), Some(BarKey::Next));
        assert_eq!(key(VK_HOME.0), Some(BarKey::First));
        assert_eq!(key(VK_END.0), Some(BarKey::Last));
        assert_eq!(key(VK_RETURN.0), Some(BarKey::Activate));
        assert_eq!(key(VK_SPACE.0), Some(BarKey::Activate));
        assert_eq!(key(VK_DOWN.0), Some(BarKey::Activate));
        assert_eq!(key(VK_ESCAPE.0), Some(BarKey::Exit));
        assert_eq!(key(VK_TAB.0), None);
        assert_eq!(bar_key(u32::from(VK_LEFT.0), INJECTED_FLAG), None);
    }

    #[test]
    fn routing_expires_at_its_deadline() {
        assert!(!route_live(0, 10));
        assert!(route_live(100, 99));
        assert!(!route_live(100, 100));
    }

    #[test]
    fn hook_and_key_routing_are_released_after_uninstall() {
        uninstall_keyboard_hook();
        assert!(!route_bar_keys(1, 0x8001));
        assert!(!is_routing_bar_keys());
        let installed = install_keyboard_hook();
        assert_eq!(HOOK_THREAD.lock().unwrap().is_some(), installed);
        if installed {
            assert!(route_bar_keys(1, 0x8001));
            assert!(is_routing_bar_keys());
        }
        uninstall_keyboard_hook();
        assert!(HOOK_THREAD.lock().unwrap().is_none());
        assert!(!is_routing_bar_keys());
    }
}
