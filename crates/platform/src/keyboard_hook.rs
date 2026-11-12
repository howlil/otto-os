use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::BarKey;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CAPITAL, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_LWIN, VK_MENU, VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_VOLUME_DOWN,
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
const KEY_SLOTS: usize = 256;

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);
static ROUTE: Mutex<Route> = Mutex::new(Route::IDLE);
static SWALLOWED_KEY_DOWNS: Mutex<SwallowedKeys> = Mutex::new(SwallowedKeys::EMPTY);

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Modifiers {
    shift: bool,
    ctrl_alt_or_win: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Route {
    target: isize,
    message: u32,
    deadline_ms: u64,
}

impl Route {
    const IDLE: Self = Self {
        target: 0,
        message: 0,
        deadline_ms: 0,
    };

    const fn armed(target: isize, message: u32, now_ms: u64) -> Self {
        Self {
            target,
            message,
            deadline_ms: now_ms + ROUTE_IDLE_LIMIT_MS,
        }
    }

    const fn targets(self, target: isize) -> bool {
        target != 0 && self.target == target
    }

    const fn is_live(self, now_ms: u64) -> bool {
        self.target != 0 && route_live(self.deadline_ms, now_ms)
    }

    fn renewed(self, now_ms: u64) -> Option<Self> {
        self.is_live(now_ms).then_some(Self {
            deadline_ms: now_ms + ROUTE_IDLE_LIMIT_MS,
            ..self
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SwallowedKeys([bool; KEY_SLOTS]);

impl SwallowedKeys {
    const EMPTY: Self = Self([false; KEY_SLOTS]);

    fn record(&mut self, virtual_key: u32, swallowed: bool) {
        if let Some(slot) = self.slot(virtual_key) {
            *slot = swallowed;
        }
    }

    fn take(&mut self, virtual_key: u32) -> bool {
        self.slot(virtual_key).is_some_and(std::mem::take)
    }

    fn slot(&mut self, virtual_key: u32) -> Option<&mut bool> {
        usize::try_from(virtual_key)
            .ok()
            .and_then(|index| self.0.get_mut(index))
    }
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

fn bar_key(virtual_key: u32, flags: u32, modifiers: Modifiers) -> Option<BarKey> {
    if flags & INJECTED_FLAG != 0 || modifiers.ctrl_alt_or_win {
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
        key if key == VK_TAB.0 && modifiers.shift => Some(BarKey::PreviousTab),
        key if key == VK_TAB.0 => Some(BarKey::NextTab),
        _ => None,
    }
}

fn key_is_down(virtual_key: u16) -> bool {
    unsafe { GetAsyncKeyState(i32::from(virtual_key)) < 0 }
}

fn current_modifiers() -> Modifiers {
    Modifiers {
        shift: key_is_down(VK_SHIFT.0),
        ctrl_alt_or_win: [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(|key| key_is_down(key.0)),
    }
}

fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}

const fn route_live(deadline_ms: u64, now_ms: u64) -> bool {
    deadline_ms != 0 && now_ms < deadline_ms
}

fn locked<T, U>(state: &Mutex<T>, update: impl FnOnce(&mut T) -> U) -> U {
    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
    update(&mut guard)
}

fn renew_route(now_ms: u64) -> Option<Route> {
    locked(&ROUTE, |route| {
        let renewed = route.renewed(now_ms);
        *route = renewed.unwrap_or(Route::IDLE);
        renewed
    })
}

#[must_use]
pub fn route_bar_keys(target_hwnd: isize, message: u32) -> bool {
    let installed = HOOK_THREAD.lock().is_ok_and(|guard| guard.is_some());
    if !installed {
        return false;
    }
    let now = now_ms();
    locked(&ROUTE, |route| {
        *route = Route::armed(target_hwnd, message, now);
    });
    true
}

pub fn stop_routing_bar_keys() {
    locked(&ROUTE, |route| *route = Route::IDLE);
}

#[must_use]
pub fn routes_keys_to(target_hwnd: isize) -> bool {
    let now = now_ms();
    locked(&ROUTE, |route| {
        route.targets(target_hwnd) && route.is_live(now)
    })
}

pub fn stop_routing_if_targets(target_hwnd: isize) {
    locked(&ROUTE, |route| {
        if route.targets(target_hwnd) {
            *route = Route::IDLE;
        }
    });
}

#[must_use]
pub fn is_routing_bar_keys() -> bool {
    let now = now_ms();
    locked(&ROUTE, |route| route.is_live(now))
}

fn post_bar_key(key: BarKey) -> bool {
    let Some(route) = renew_route(now_ms()) else {
        return false;
    };
    let posted = unsafe {
        PostMessageW(
            Some(HWND(route.target as *mut _)),
            route.message,
            WPARAM(key.code()),
            LPARAM(0),
        )
    };
    if posted.is_err() {
        stop_routing_if_targets(route.target);
        return false;
    }
    true
}

fn is_key_down(message: usize) -> bool {
    message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize
}

fn record_key_down(virtual_key: u32, swallowed: bool) {
    locked(&SWALLOWED_KEY_DOWNS, |keys| {
        keys.record(virtual_key, swallowed);
    });
}

fn take_key_down(virtual_key: u32) -> bool {
    locked(&SWALLOWED_KEY_DOWNS, |keys| keys.take(virtual_key))
}

fn apply_volume_key(key: VolumeKey) -> bool {
    match key {
        VolumeKey::Up => step_system_volume(WHEEL_UP).is_some(),
        VolumeKey::Down => step_system_volume(WHEEL_DOWN).is_some(),
        VolumeKey::Mute => toggle_system_mute().is_some(),
    }
}

fn swallow_key_down(virtual_key: u32, flags: u32) -> bool {
    let routed = bar_key(virtual_key, flags, current_modifiers()).is_some_and(post_bar_key);
    record_key_down(virtual_key, routed);
    routed || volume_key(virtual_key, flags).is_some_and(apply_volume_key)
}

fn swallow_key_up(virtual_key: u32, flags: u32) -> bool {
    take_key_down(virtual_key) || (volume_key(virtual_key, flags).is_some() && is_available())
}

fn swallow(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 {
        return false;
    }
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if event.vkCode == u32::from(VK_CAPITAL.0) {
        crate::idle_wake::request_idle_wake();
    }
    if is_key_down(wparam.0) {
        swallow_key_down(event.vkCode, event.flags.0)
    } else {
        swallow_key_up(event.vkCode, event.flags.0)
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
        HOOK_THREAD, INJECTED_FLAG, Modifiers, Route, SwallowedKeys, VolumeKey, bar_key,
        install_keyboard_hook, is_key_down, is_routing_bar_keys, route_bar_keys, route_live,
        uninstall_keyboard_hook, volume_key,
    };
    use otto_core::BarKey;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_A, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB,
        VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
    };
    use windows::Win32::UI::WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN};

    const PLAIN: Modifiers = Modifiers {
        shift: false,
        ctrl_alt_or_win: false,
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ctrl_alt_or_win: false,
    };
    const CHORD: Modifiers = Modifiers {
        shift: false,
        ctrl_alt_or_win: true,
    };

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
        let key = |vk: u16| bar_key(u32::from(vk), 0, PLAIN);
        assert_eq!(key(VK_LEFT.0), Some(BarKey::Previous));
        assert_eq!(key(VK_RIGHT.0), Some(BarKey::Next));
        assert_eq!(key(VK_HOME.0), Some(BarKey::First));
        assert_eq!(key(VK_END.0), Some(BarKey::Last));
        assert_eq!(key(VK_RETURN.0), Some(BarKey::Activate));
        assert_eq!(key(VK_SPACE.0), Some(BarKey::Activate));
        assert_eq!(key(VK_DOWN.0), Some(BarKey::Activate));
        assert_eq!(key(VK_ESCAPE.0), Some(BarKey::Exit));
        assert_eq!(bar_key(u32::from(VK_LEFT.0), INJECTED_FLAG, PLAIN), None);
    }

    #[test]
    fn tab_moves_forward_and_shift_tab_moves_back() {
        assert_eq!(
            bar_key(u32::from(VK_TAB.0), 0, PLAIN),
            Some(BarKey::NextTab)
        );
        assert_eq!(
            bar_key(u32::from(VK_TAB.0), 0, SHIFT),
            Some(BarKey::PreviousTab)
        );
        assert_eq!(bar_key(u32::from(VK_TAB.0), INJECTED_FLAG, PLAIN), None);
    }

    #[test]
    fn ctrl_alt_and_win_chords_are_left_to_the_system() {
        for vk in [VK_TAB, VK_LEFT, VK_ESCAPE] {
            assert_eq!(bar_key(u32::from(vk.0), 0, CHORD), None);
        }
    }

    #[test]
    fn routing_expires_at_its_deadline() {
        assert!(!route_live(0, 10));
        assert!(route_live(100, 99));
        assert!(!route_live(100, 100));
    }

    #[test]
    fn a_route_renews_only_while_it_is_live() {
        let route = Route::armed(7, 0x8001, 1_000);
        assert_eq!(
            route.renewed(15_999).map(|renewed| renewed.deadline_ms),
            Some(30_999)
        );
        assert_eq!(route.renewed(16_000), None);
    }

    #[test]
    fn a_route_is_owned_only_by_its_live_target() {
        let route = Route::armed(7, 0x8001, 1_000);
        assert!(route.targets(7));
        assert!(!route.targets(8));
        assert!(!route.targets(0));
        assert!(route.is_live(15_999));
        assert!(!route.is_live(16_000));
        assert!(!Route::IDLE.targets(0));
        assert!(!Route::IDLE.is_live(0));
    }

    #[test]
    fn a_key_up_is_swallowed_only_when_its_key_down_was() {
        let mut keys = SwallowedKeys::EMPTY;
        let left = u32::from(VK_LEFT.0);
        let right = u32::from(VK_RIGHT.0);
        assert!(!keys.take(left));
        keys.record(left, true);
        keys.record(right, false);
        assert!(!keys.take(right));
        assert!(keys.take(left));
        assert!(!keys.take(left));
    }

    #[test]
    fn a_passed_key_down_clears_an_earlier_swallow() {
        let mut keys = SwallowedKeys::EMPTY;
        let left = u32::from(VK_LEFT.0);
        keys.record(left, true);
        keys.record(left, false);
        assert!(!keys.take(left));
    }

    #[test]
    fn out_of_range_virtual_keys_are_never_swallowed() {
        let mut keys = SwallowedKeys::EMPTY;
        keys.record(u32::MAX, true);
        assert!(!keys.take(u32::MAX));
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
