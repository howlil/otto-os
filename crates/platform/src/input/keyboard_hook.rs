use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use otto_core::BarKey;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_DOWN, VK_E, VK_END, VK_ESCAPE, VK_HOME, VK_KANA,
    VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NUMLOCK, VK_RCONTROL,
    VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
    VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PeekMessageW,
    PostMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::bar::audio::{is_available, step_system_volume, toggle_system_mute};
use crate::worker::{HookStartup, HookThread, install_hook_thread, uninstall_hook_thread};

const READY_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
const INJECTED_FLAG: u32 = 0x10;
const WIN_MENU_MASK_VK: u16 = 0x00FF;
const WHEEL_UP: i16 = 120;
const WHEEL_DOWN: i16 = -120;
const ROUTE_IDLE_LIMIT_MS: u64 = 15_000;
const KEY_SLOTS: usize = 256;
const SWALLOWED_WORDS: usize = KEY_SLOTS / 64;

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);
static ROUTE: Mutex<Route> = Mutex::new(Route::IDLE);
static ROUTE_ACTIVE: AtomicBool = AtomicBool::new(false);
static SWALLOWED_KEY_DOWNS: SwallowedKeys = SwallowedKeys::empty();
static VOLUME_HUD_HOOK_ENABLED: AtomicBool = AtomicBool::new(true);
static FINDER_HOOK_TARGET: AtomicIsize = AtomicIsize::new(0);
static FINDER_HOOK_MESSAGE: AtomicU32 = AtomicU32::new(0);
static KEYSTROKE_COUNT: AtomicU64 = AtomicU64::new(0);

#[must_use]
pub fn take_keystroke_count() -> u64 {
    KEYSTROKE_COUNT.swap(0, Ordering::AcqRel)
}

const fn is_typing_key(virtual_key: u32, flags: u32) -> bool {
    if flags & INJECTED_FLAG != 0 || virtual_key > u16::MAX as u32 {
        return false;
    }
    let vk = virtual_key as u16;
    !matches!(
        vk,
        x if x == VK_SHIFT.0
            || x == VK_LSHIFT.0
            || x == VK_RSHIFT.0
            || x == VK_CONTROL.0
            || x == VK_LCONTROL.0
            || x == VK_RCONTROL.0
            || x == VK_MENU.0
            || x == VK_LMENU.0
            || x == VK_RMENU.0
            || x == VK_LWIN.0
            || x == VK_RWIN.0
            || x == VK_VOLUME_UP.0
            || x == VK_VOLUME_DOWN.0
            || x == VK_VOLUME_MUTE.0
    )
}

pub fn set_volume_hud_hook_enabled(enabled: bool) {
    VOLUME_HUD_HOOK_ENABLED.store(enabled, Ordering::Release);
}

#[must_use]
pub fn is_volume_hud_hook_enabled() -> bool {
    VOLUME_HUD_HOOK_ENABLED.load(Ordering::Acquire)
}

pub fn set_finder_shortcut_hook(target_hwnd: isize, message: u32) {
    if target_hwnd == 0 || message == 0 {
        FINDER_HOOK_TARGET.store(0, Ordering::Release);
        FINDER_HOOK_MESSAGE.store(0, Ordering::Release);
    } else {
        FINDER_HOOK_MESSAGE.store(message, Ordering::Release);
        FINDER_HOOK_TARGET.store(target_hwnd, Ordering::Release);
    }
}

#[must_use]
pub fn is_finder_shortcut_hook_enabled() -> bool {
    FINDER_HOOK_TARGET.load(Ordering::Acquire) != 0
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FinderShortcutModifiers {
    win: bool,
    ctrl_alt_or_shift: bool,
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

struct SwallowedKeys([AtomicU64; SWALLOWED_WORDS]);

impl SwallowedKeys {
    const fn empty() -> Self {
        Self([const { AtomicU64::new(0) }; SWALLOWED_WORDS])
    }

    fn word_and_mask(virtual_key: u32) -> Option<(usize, u64)> {
        let index = usize::try_from(virtual_key).ok()?;
        if index >= KEY_SLOTS {
            return None;
        }
        Some((index / 64, 1u64 << (index % 64)))
    }

    fn record(&self, virtual_key: u32, swallowed: bool) {
        if let Some((word, mask)) = Self::word_and_mask(virtual_key) {
            if swallowed {
                self.0[word].fetch_or(mask, Ordering::AcqRel);
            } else if self.0[word].load(Ordering::Relaxed) & mask != 0 {
                self.0[word].fetch_and(!mask, Ordering::AcqRel);
            }
        }
    }

    fn is_swallowed(&self, virtual_key: u32) -> bool {
        Self::word_and_mask(virtual_key)
            .is_some_and(|(word, mask)| self.0[word].load(Ordering::Acquire) & mask != 0)
    }

    fn take(&self, virtual_key: u32) -> bool {
        let Some((word, mask)) = Self::word_and_mask(virtual_key) else {
            return false;
        };
        if self.0[word].load(Ordering::Acquire) & mask == 0 {
            return false;
        }
        self.0[word].fetch_and(!mask, Ordering::AcqRel) & mask != 0
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

const fn is_finder_shortcut_key(
    virtual_key: u32,
    flags: u32,
    modifiers: FinderShortcutModifiers,
) -> bool {
    flags & INJECTED_FLAG == 0
        && virtual_key == VK_E.0 as u32
        && modifiers.win
        && !modifiers.ctrl_alt_or_shift
}

fn bar_key(virtual_key: u32, flags: u32, modifiers: Modifiers) -> Option<BarKey> {
    if flags & INJECTED_FLAG != 0 || modifiers.ctrl_alt_or_win {
        return None;
    }
    let key = u16::try_from(virtual_key).ok()?;
    match key {
        key if key == VK_UP.0 => Some(BarKey::Up),
        key if key == VK_DOWN.0 => Some(BarKey::Down),
        key if key == VK_LEFT.0 => Some(BarKey::Previous),
        key if key == VK_RIGHT.0 => Some(BarKey::Next),
        key if key == VK_HOME.0 => Some(BarKey::First),
        key if key == VK_END.0 => Some(BarKey::Last),
        key if key == VK_RETURN.0 || key == VK_SPACE.0 => Some(BarKey::Activate),
        key if key == VK_ESCAPE.0 => Some(BarKey::Exit),
        key if key == VK_TAB.0 && modifiers.shift => Some(BarKey::PreviousTab),
        key if key == VK_TAB.0 => Some(BarKey::NextTab),
        0x30..=0x39 if !modifiers.shift => Some(BarKey::Digit((key - 0x30) as u8)),
        0x60..=0x69 => Some(BarKey::Digit((key - 0x60) as u8)),
        0x08 => Some(BarKey::Backspace),
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

fn current_finder_modifiers() -> FinderShortcutModifiers {
    FinderShortcutModifiers {
        win: key_is_down(VK_LWIN.0) || key_is_down(VK_RWIN.0),
        ctrl_alt_or_shift: [VK_CONTROL, VK_MENU, VK_SHIFT]
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
        if renewed.is_none() {
            ROUTE_ACTIVE.store(false, Ordering::Release);
        }
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
        ROUTE_ACTIVE.store(target_hwnd != 0, Ordering::Release);
    });
    true
}

pub fn stop_routing_bar_keys() {
    locked(&ROUTE, |route| {
        *route = Route::IDLE;
        ROUTE_ACTIVE.store(false, Ordering::Release);
    });
}

#[must_use]
pub fn routes_keys_to(target_hwnd: isize) -> bool {
    if !ROUTE_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let now = now_ms();
    locked(&ROUTE, |route| {
        let live = route.is_live(now);
        if !live {
            *route = Route::IDLE;
            ROUTE_ACTIVE.store(false, Ordering::Release);
            return false;
        }
        route.targets(target_hwnd)
    })
}

pub fn stop_routing_if_targets(target_hwnd: isize) {
    if !ROUTE_ACTIVE.load(Ordering::Acquire) {
        return;
    }
    locked(&ROUTE, |route| {
        if route.targets(target_hwnd) {
            *route = Route::IDLE;
            ROUTE_ACTIVE.store(false, Ordering::Release);
        }
    });
}

#[must_use]
pub fn is_routing_bar_keys() -> bool {
    if !ROUTE_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let now = now_ms();
    locked(&ROUTE, |route| {
        let live = route.is_live(now);
        if !live {
            *route = Route::IDLE;
            ROUTE_ACTIVE.store(false, Ordering::Release);
        }
        live
    })
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

fn post_finder_shortcut(target_hwnd: isize, message: u32) -> bool {
    let mask = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(WIN_MENU_MASK_VK),
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let posted = unsafe {
        let _ = SendInput(&[mask], size_of::<INPUT>() as i32);
        PostMessageW(
            Some(HWND(target_hwnd as *mut _)),
            message,
            WPARAM(0),
            LPARAM(0),
        )
    };
    posted.is_ok()
}

fn try_swallow_finder_shortcut(virtual_key: u32, flags: u32) -> bool {
    if flags & INJECTED_FLAG != 0 || virtual_key != u32::from(VK_E.0) {
        return false;
    }
    let target = FINDER_HOOK_TARGET.load(Ordering::Acquire);
    if target == 0 {
        return false;
    }
    let message = FINDER_HOOK_MESSAGE.load(Ordering::Acquire);
    if message == 0 || !is_finder_shortcut_key(virtual_key, flags, current_finder_modifiers()) {
        return false;
    }
    if SWALLOWED_KEY_DOWNS.is_swallowed(virtual_key) {
        return true;
    }
    if post_finder_shortcut(target, message) {
        record_key_down(virtual_key, true);
        return true;
    }
    false
}

fn is_key_down(message: usize) -> bool {
    message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize
}

fn record_key_down(virtual_key: u32, swallowed: bool) {
    SWALLOWED_KEY_DOWNS.record(virtual_key, swallowed);
}

fn take_key_down(virtual_key: u32) -> bool {
    SWALLOWED_KEY_DOWNS.take(virtual_key)
}

fn apply_volume_key(key: VolumeKey) -> bool {
    match key {
        VolumeKey::Up => step_system_volume(WHEEL_UP).is_some(),
        VolumeKey::Down => step_system_volume(WHEEL_DOWN).is_some(),
        VolumeKey::Mute => toggle_system_mute().is_some(),
    }
}

fn swallow_key_down(virtual_key: u32, flags: u32) -> bool {
    let routed = ROUTE_ACTIVE.load(Ordering::Acquire)
        && bar_key(virtual_key, flags, current_modifiers()).is_some_and(post_bar_key);
    if routed {
        record_key_down(virtual_key, true);
        return true;
    }
    if try_swallow_finder_shortcut(virtual_key, flags) {
        return true;
    }
    record_key_down(virtual_key, false);
    is_volume_hud_hook_enabled() && volume_key(virtual_key, flags).is_some_and(apply_volume_key)
}

fn swallow_key_up(virtual_key: u32, flags: u32) -> bool {
    take_key_down(virtual_key)
        || (is_volume_hud_hook_enabled()
            && volume_key(virtual_key, flags).is_some()
            && is_available())
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct WakeModifiers {
    win: bool,
    shift: bool,
    alt_or_ctrl: bool,
}

fn should_wake_status_lazy(
    virtual_key: u32,
    win: impl FnOnce() -> bool,
    shift: impl FnOnce() -> bool,
    alt_or_ctrl: impl FnOnce() -> bool,
) -> bool {
    let Ok(vk) = u16::try_from(virtual_key) else {
        return false;
    };
    match vk {
        x if x == VK_CAPITAL.0 || x == VK_NUMLOCK.0 || x == VK_KANA.0 => true,
        0x70..=0x7B | 0xAD..=0xB3 => true,
        x if x == VK_SPACE.0 => win(),
        x if x == VK_SHIFT.0 || x == VK_LSHIFT.0 || x == VK_RSHIFT.0 => alt_or_ctrl(),
        x if x == VK_MENU.0 || x == VK_LMENU.0 || x == VK_RMENU.0 => shift(),
        _ => false,
    }
}

#[cfg(test)]
fn should_wake_status_with(virtual_key: u32, mods: WakeModifiers) -> bool {
    should_wake_status_lazy(virtual_key, || mods.win, || mods.shift, || mods.alt_or_ctrl)
}

fn should_wake_status(virtual_key: u32) -> bool {
    should_wake_status_lazy(
        virtual_key,
        || key_is_down(VK_LWIN.0) || key_is_down(VK_RWIN.0),
        || key_is_down(VK_SHIFT.0) || key_is_down(VK_LSHIFT.0) || key_is_down(VK_RSHIFT.0),
        || {
            key_is_down(VK_MENU.0)
                || key_is_down(VK_LMENU.0)
                || key_is_down(VK_RMENU.0)
                || key_is_down(VK_CONTROL.0)
        },
    )
}

fn swallow(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 || lparam.0 == 0 {
        return false;
    }
    let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    if should_wake_status(event.vkCode) {
        crate::idle_wake::request_idle_wake();
    }
    if is_key_down(wparam.0) {
        if is_typing_key(event.vkCode, event.flags.0) {
            KEYSTROKE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
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

fn hook_thread_main(startup: &HookStartup) {
    unsafe {
        let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0)
        else {
            let _ = startup.publish(None);
            return;
        };
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        if !startup.publish(Some(GetCurrentThreadId())) {
            let _ = UnhookWindowsHookEx(hook);
            return;
        }
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

#[must_use]
pub fn install_keyboard_hook() -> bool {
    install_hook_thread(
        &HOOK_THREAD,
        "otto-keyboard-hook",
        READY_TIMEOUT,
        SHUTDOWN_TIMEOUT,
        |startup: Arc<HookStartup>| hook_thread_main(&startup),
    )
}

pub fn uninstall_keyboard_hook() {
    stop_routing_bar_keys();
    set_finder_shortcut_hook(0, 0);
    uninstall_hook_thread(&HOOK_THREAD, SHUTDOWN_TIMEOUT);
}

#[cfg(test)]
mod tests {
    use super::{
        FinderShortcutModifiers, HOOK_THREAD, INJECTED_FLAG, Modifiers, Route, SwallowedKeys,
        VolumeKey, WakeModifiers, bar_key, install_keyboard_hook, is_finder_shortcut_hook_enabled,
        is_finder_shortcut_key, is_key_down, is_routing_bar_keys, route_bar_keys, route_live,
        set_finder_shortcut_hook, should_wake_status_with, uninstall_keyboard_hook, volume_key,
    };
    use otto_core::BarKey;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_A, VK_CAPITAL, VK_DOWN, VK_E, VK_END, VK_ESCAPE, VK_F1, VK_F2, VK_HOME, VK_LEFT,
        VK_NUMLOCK, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP, VK_VOLUME_DOWN,
        VK_VOLUME_MUTE, VK_VOLUME_UP,
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
        assert_eq!(key(VK_UP.0), Some(BarKey::Up));
        assert_eq!(key(VK_DOWN.0), Some(BarKey::Down));
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
        let keys = SwallowedKeys::empty();
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
        let keys = SwallowedKeys::empty();
        let left = u32::from(VK_LEFT.0);
        keys.record(left, true);
        keys.record(left, false);
        assert!(!keys.take(left));
    }

    #[test]
    fn out_of_range_virtual_keys_are_never_swallowed() {
        let keys = SwallowedKeys::empty();
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

    #[test]
    fn status_wake_matches_lock_keys_and_layout_chords() {
        let none = WakeModifiers::default();
        assert!(should_wake_status_with(u32::from(VK_CAPITAL.0), none));
        assert!(should_wake_status_with(u32::from(VK_NUMLOCK.0), none));
        assert!(should_wake_status_with(u32::from(VK_F1.0), none));
        assert!(should_wake_status_with(u32::from(VK_F2.0), none));
        assert!(should_wake_status_with(u32::from(VK_VOLUME_UP.0), none));
        assert!(!should_wake_status_with(u32::from(VK_SPACE.0), none));
        assert!(should_wake_status_with(
            u32::from(VK_SPACE.0),
            WakeModifiers { win: true, ..none }
        ));
        assert!(should_wake_status_with(
            u32::from(VK_SHIFT.0),
            WakeModifiers {
                alt_or_ctrl: true,
                ..none
            }
        ));
    }

    #[test]
    fn volume_hud_hook_flag_toggles_cleanly() {
        let initial = super::is_volume_hud_hook_enabled();
        super::set_volume_hud_hook_enabled(false);
        assert!(!super::is_volume_hud_hook_enabled());
        super::set_volume_hud_hook_enabled(true);
        assert!(super::is_volume_hud_hook_enabled());
        super::set_volume_hud_hook_enabled(initial);
    }

    #[test]
    fn null_lparam_is_safely_ignored_by_hook_filter() {
        assert!(!super::swallow(
            0,
            super::WPARAM(WM_KEYDOWN as usize),
            super::LPARAM(0)
        ));
        assert!(!super::swallow(
            -1,
            super::WPARAM(WM_KEYDOWN as usize),
            super::LPARAM(0)
        ));
    }

    #[test]
    fn win_e_matches_finder_shortcut_only_without_extra_modifiers() {
        let win_only = FinderShortcutModifiers {
            win: true,
            ctrl_alt_or_shift: false,
        };
        let win_with_extra = FinderShortcutModifiers {
            win: true,
            ctrl_alt_or_shift: true,
        };
        let no_win = FinderShortcutModifiers::default();

        assert!(is_finder_shortcut_key(u32::from(VK_E.0), 0, win_only));
        assert!(!is_finder_shortcut_key(
            u32::from(VK_E.0),
            INJECTED_FLAG,
            win_only
        ));
        assert!(!is_finder_shortcut_key(
            u32::from(VK_E.0),
            0,
            win_with_extra
        ));
        assert!(!is_finder_shortcut_key(u32::from(VK_E.0), 0, no_win));
        assert!(!is_finder_shortcut_key(u32::from(VK_A.0), 0, win_only));
    }

    #[test]
    fn finder_shortcut_hook_toggles_and_clears_cleanly() {
        set_finder_shortcut_hook(0, 0);
        assert!(!is_finder_shortcut_hook_enabled());
        set_finder_shortcut_hook(42, 0x803F);
        assert!(is_finder_shortcut_hook_enabled());
        set_finder_shortcut_hook(42, 0);
        assert!(!is_finder_shortcut_hook_enabled());
        set_finder_shortcut_hook(0, 0x803F);
        assert!(!is_finder_shortcut_hook_enabled());
    }
}
