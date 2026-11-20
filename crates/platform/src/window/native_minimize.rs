use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EnumWindows, GA_ROOT, GWL_STYLE, GetAncestor, GetMessageW,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible, KillTimer,
    MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, PostMessageW, PostThreadMessageW, SC_MINIMIZE,
    SetTimer, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_MOUSE_LL, WM_APP,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_SYSCOMMAND, WM_TIMER, WS_CAPTION, WS_MINIMIZEBOX, WS_SYSMENU,
    WindowFromPoint,
};
use windows::core::BOOL;

use otto_core::scale;

use crate::window::caption::{covering_caption_of, dwm_rect};
use crate::worker::{HookStartup, HookThread, install_hook_thread, uninstall_hook_thread};

const READY_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
const ACK_DEADLINE_MS: u64 = 400;
const WATCHDOG_TICK_MS: u32 = 100;
const WM_APP_ARM_WATCHDOG: u32 = WM_APP + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pending {
    window: isize,
    deadline_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchdogAction {
    Wait,
    MinimizeNatively(isize),
}

static HOOK_THREAD: Mutex<Option<HookThread>> = Mutex::new(None);
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static TARGET: AtomicIsize = AtomicIsize::new(0);
static MESSAGE: AtomicU32 = AtomicU32::new(0);
static SWALLOW_BUTTON_UP: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}

fn locked<T, U>(state: &Mutex<T>, update: impl FnOnce(&mut T) -> U) -> U {
    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
    update(&mut guard)
}

fn watchdog_action(pending: Option<Pending>, now_ms: u64) -> WatchdogAction {
    match pending {
        Some(pending) if now_ms >= pending.deadline_ms => {
            WatchdogAction::MinimizeNatively(pending.window)
        }
        _ => WatchdogAction::Wait,
    }
}

fn is_cloaked(window: HWND) -> bool {
    let mut cloaked = 0_u32;
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            std::ptr::from_mut(&mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0
}

struct HitSearch {
    x: i32,
    y: i32,
    found: isize,
}

const CAPTION_BAND_MAX_DIP: i32 = 96;
const WINDOW_RECT_PREFILTER_MARGIN_PX: i32 = 16;

const fn rect_contains(rect: RECT, x: i32, y: i32, margin: i32) -> bool {
    x >= rect.left - margin
        && x < rect.right + margin
        && y >= rect.top - margin
        && y < rect.bottom + margin
}

const fn has_minimize_caption_style(style: u32) -> bool {
    style & WS_CAPTION.0 == WS_CAPTION.0
        && style & WS_SYSMENU.0 != 0
        && style & WS_MINIMIZEBOX.0 != 0
}

fn is_within_top_caption_band(x: i32, y: i32, frame: RECT, dpi: u32) -> bool {
    rect_contains(frame, x, y, 0) && y < frame.top + scale(CAPTION_BAND_MAX_DIP, dpi.max(96))
}

unsafe extern "system" fn visit_window(window: HWND, lparam: LPARAM) -> BOOL {
    let search = unsafe { &mut *(lparam.0 as *mut HitSearch) };
    if !unsafe { IsWindowVisible(window) }.as_bool() || unsafe { IsIconic(window) }.as_bool() {
        return BOOL(1);
    }
    let mut bounds = RECT::default();
    if unsafe { GetWindowRect(window, &raw mut bounds) }.is_ok()
        && !rect_contains(bounds, search.x, search.y, WINDOW_RECT_PREFILTER_MARGIN_PX)
    {
        return BOOL(1);
    }
    let Some(frame) = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS) else {
        return BOOL(1);
    };
    if rect_contains(frame, search.x, search.y, 0) && !is_cloaked(window) {
        search.found = window.0 as isize;
        return BOOL(0);
    }
    BOOL(1)
}

enum FastHit {
    Window(HWND),
    OwnProcess,
    Fallback,
}

fn fast_top_level_window_at(x: i32, y: i32) -> FastHit {
    let hit = unsafe { WindowFromPoint(POINT { x, y }) };
    if hit.is_invalid() {
        return FastHit::Fallback;
    }
    let ancestor = unsafe { GetAncestor(hit, GA_ROOT) };
    let root = if ancestor.is_invalid() { hit } else { ancestor };
    if belongs_to_this_process(root) {
        return FastHit::OwnProcess;
    }
    if !unsafe { IsWindowVisible(root) }.as_bool()
        || unsafe { IsIconic(root) }.as_bool()
        || is_cloaked(root)
    {
        return FastHit::Fallback;
    }
    let Some(frame) = dwm_rect(root, DWMWA_EXTENDED_FRAME_BOUNDS) else {
        return FastHit::Fallback;
    };
    if rect_contains(frame, x, y, 0) {
        FastHit::Window(root)
    } else {
        FastHit::Fallback
    }
}

fn top_level_window_at(x: i32, y: i32) -> Option<HWND> {
    match fast_top_level_window_at(x, y) {
        FastHit::Window(window) => return Some(window),
        FastHit::OwnProcess => return None,
        FastHit::Fallback => {}
    }
    let mut search = HitSearch { x, y, found: 0 };
    let _ = unsafe {
        EnumWindows(
            Some(visit_window),
            LPARAM(std::ptr::from_mut(&mut search) as isize),
        )
    };
    (search.found != 0).then_some(HWND(search.found as *mut _))
}

fn belongs_to_this_process(window: HWND) -> bool {
    let mut process = 0_u32;
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut process)) };
    process == unsafe { GetCurrentProcessId() }
}

fn minimize_target_under(x: i32, y: i32) -> Option<isize> {
    let window = top_level_window_at(x, y)?;
    if belongs_to_this_process(window) {
        return None;
    }
    let style = unsafe { GetWindowLongW(window, GWL_STYLE) } as u32;
    if !has_minimize_caption_style(style) {
        return None;
    }
    let frame = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS)?;
    let dpi = unsafe { GetDpiForWindow(window) };
    if !is_within_top_caption_band(x, y, frame, dpi) {
        return None;
    }
    let hit = covering_caption_of(window)?
        .minimize_button()?
        .contains(x, y);
    if !hit || crate::window::model::is_window_uipi_blocked(window) {
        return None;
    }
    Some(window.0 as isize)
}

fn request_minimize(x: i32, y: i32) -> bool {
    let target = TARGET.load(Ordering::Acquire);
    let message = MESSAGE.load(Ordering::Acquire);
    if target == 0 || message == 0 {
        return false;
    }
    let Some(window) = minimize_target_under(x, y) else {
        return false;
    };
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            message,
            WPARAM(window as usize),
            LPARAM(0),
        )
    };
    if posted.is_err() {
        return false;
    }
    let deadline_ms = now_ms() + ACK_DEADLINE_MS;
    locked(&PENDING, |pending| {
        *pending = Some(Pending {
            window,
            deadline_ms,
        });
    });
    let _ = unsafe {
        PostThreadMessageW(
            GetCurrentThreadId(),
            WM_APP_ARM_WATCHDOG,
            WPARAM(0),
            LPARAM(0),
        )
    };
    SWALLOW_BUTTON_UP.store(true, Ordering::Release);
    true
}

fn swallow(code: i32, wparam: WPARAM, lparam: LPARAM) -> bool {
    if code < 0 {
        return false;
    }
    let event = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    match wparam.0 as u32 {
        WM_LBUTTONDOWN => request_minimize(event.pt.x, event.pt.y),
        WM_LBUTTONUP => SWALLOW_BUTTON_UP.swap(false, Ordering::AcqRel),
        _ => false,
    }
}

unsafe extern "system" fn low_level_mouse_proc(
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

fn run_watchdog() {
    let now = now_ms();
    let action = locked(&PENDING, |pending| {
        let action = watchdog_action(*pending, now);
        if action != WatchdogAction::Wait {
            *pending = None;
        }
        action
    });
    if let WatchdogAction::MinimizeNatively(window) = action {
        let _ = unsafe {
            PostMessageW(
                Some(HWND(window as *mut _)),
                WM_SYSCOMMAND,
                WPARAM(SC_MINIMIZE as usize),
                LPARAM(0),
            )
        };
    }
}

fn should_keep_watchdog_timer(pending: Option<Pending>) -> bool {
    pending.is_some()
}

fn hook_thread_main(startup: &HookStartup) {
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), None, 0) };
    let Ok(hook) = hook else {
        let _ = startup.publish(None);
        return;
    };
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        let mut watchdog_timer: usize = 0;
        if !startup.publish(Some(GetCurrentThreadId())) {
            let _ = UnhookWindowsHookEx(hook);
            return;
        }
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            if message.message == WM_APP_ARM_WATCHDOG {
                if watchdog_timer == 0
                    && locked(&PENDING, |pending| should_keep_watchdog_timer(*pending))
                {
                    watchdog_timer = SetTimer(None, 0, WATCHDOG_TICK_MS, None);
                }
                continue;
            }
            if message.message == WM_TIMER {
                run_watchdog();
                if watchdog_timer != 0
                    && !locked(&PENDING, |pending| should_keep_watchdog_timer(*pending))
                {
                    let _ = KillTimer(None, watchdog_timer);
                    watchdog_timer = 0;
                }
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if watchdog_timer != 0 {
            let _ = KillTimer(None, watchdog_timer);
        }
        let _ = UnhookWindowsHookEx(hook);
    }
}

pub fn install_native_minimize_hook(target_hwnd: isize, message: u32) -> bool {
    MESSAGE.store(message, Ordering::Release);
    TARGET.store(target_hwnd, Ordering::Release);
    install_hook_thread(
        &HOOK_THREAD,
        "otto-native-minimize-hook",
        READY_TIMEOUT,
        SHUTDOWN_TIMEOUT,
        |startup: Arc<HookStartup>| hook_thread_main(&startup),
    )
}

pub fn uninstall_native_minimize_hook() {
    TARGET.store(0, Ordering::Release);
    MESSAGE.store(0, Ordering::Release);
    SWALLOW_BUTTON_UP.store(false, Ordering::Release);
    locked(&PENDING, |pending| *pending = None);
    uninstall_hook_thread(&HOOK_THREAD, SHUTDOWN_TIMEOUT);
}

pub fn acknowledge_native_minimize(window: isize) {
    locked(&PENDING, |pending| {
        if pending.is_some_and(|pending| pending.window == window) {
            *pending = None;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        HOOK_THREAD, Pending, RECT, WS_CAPTION, WS_MINIMIZEBOX, WS_SYSMENU, WatchdogAction,
        acknowledge_native_minimize, has_minimize_caption_style, install_native_minimize_hook,
        is_within_top_caption_band, should_keep_watchdog_timer, uninstall_native_minimize_hook,
        watchdog_action,
    };

    #[test]
    fn the_watchdog_waits_for_the_deadline_and_then_minimizes_natively() {
        let pending = Some(Pending {
            window: 77,
            deadline_ms: 1_000,
        });
        assert_eq!(watchdog_action(pending, 999), WatchdogAction::Wait);
        assert_eq!(
            watchdog_action(pending, 1_000),
            WatchdogAction::MinimizeNatively(77)
        );
        assert_eq!(watchdog_action(None, 5_000), WatchdogAction::Wait);
    }

    #[test]
    fn an_acknowledgement_only_clears_its_own_window() {
        use super::{PENDING, locked};
        locked(&PENDING, |pending| {
            *pending = Some(Pending {
                window: 5,
                deadline_ms: 10,
            });
        });
        acknowledge_native_minimize(6);
        assert!(locked(&PENDING, |pending| pending.is_some()));
        acknowledge_native_minimize(5);
        assert!(locked(&PENDING, |pending| pending.is_none()));
    }

    #[test]
    fn the_watchdog_timer_only_runs_while_a_minimize_is_pending() {
        assert!(!should_keep_watchdog_timer(None));
        assert!(should_keep_watchdog_timer(Some(Pending {
            window: 77,
            deadline_ms: 1_000,
        })));
    }

    #[test]
    fn the_hook_thread_is_released_after_uninstall() {
        assert!(install_native_minimize_hook(1, 0x8000 + 24));
        assert!(HOOK_THREAD.lock().is_ok_and(|guard| guard.is_some()));
        uninstall_native_minimize_hook();
        assert!(HOOK_THREAD.lock().is_ok_and(|guard| guard.is_none()));
    }

    #[test]
    fn caption_band_and_style_prefilter_rejects_client_area_and_tool_windows() {
        let style_ok = WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0;
        assert!(has_minimize_caption_style(style_ok));
        assert!(!has_minimize_caption_style(WS_CAPTION.0 | WS_SYSMENU.0));
        assert!(!has_minimize_caption_style(WS_SYSMENU.0 | WS_MINIMIZEBOX.0));

        let frame = RECT {
            left: 100,
            top: 40,
            right: 900,
            bottom: 700,
        };
        assert!(is_within_top_caption_band(800, 60, frame, 96));
        assert!(!is_within_top_caption_band(800, 300, frame, 96));
        assert!(!is_within_top_caption_band(50, 60, frame, 96));
        assert!(is_within_top_caption_band(800, 160, frame, 144));
        assert!(!is_within_top_caption_band(800, 200, frame, 144));
    }
}
