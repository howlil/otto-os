use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicU32, AtomicU64, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED,
    EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, FindWindowW,
    GA_ROOT, GetAncestor, GetForegroundWindow, IsWindow, OBJID_WINDOW, PostMessageW, SW_HIDE,
    ShowWindow, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};
use windows::core::{PCWSTR, w};

const HOOKED_EVENT_RANGES: [(u32, u32); 5] = [
    (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
    (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
    (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
    (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
    (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
];

static HOOKS: [AtomicPtr<c_void>; HOOKED_EVENT_RANGES.len()] =
    [const { AtomicPtr::new(std::ptr::null_mut()) }; HOOKED_EVENT_RANGES.len()];
static HOOKS_LIVE: AtomicBool = AtomicBool::new(false);
static TOPOLOGY_GENERATION: AtomicU64 = AtomicU64::new(0);
static GEOMETRY_GENERATION: AtomicU64 = AtomicU64::new(0);
static WIN_EVENT_TARGET: AtomicIsize = AtomicIsize::new(0);
static WIN_EVENT_MSG: AtomicU32 = AtomicU32::new(0);
static SYNC_PENDING: AtomicBool = AtomicBool::new(false);
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static LAST_POLLED_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowChangeGenerations {
    pub topology: u64,
    pub geometry: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowChange {
    Topology,
    Geometry,
}

fn classify_window_event(event: u32, top_level: bool) -> Option<WindowChange> {
    if !top_level {
        return None;
    }
    match event {
        EVENT_SYSTEM_FOREGROUND
        | EVENT_SYSTEM_MINIMIZESTART
        | EVENT_SYSTEM_MINIMIZEEND
        | EVENT_OBJECT_DESTROY
        | EVENT_OBJECT_SHOW
        | EVENT_OBJECT_HIDE
        | EVENT_OBJECT_CLOAKED
        | EVENT_OBJECT_UNCLOAKED => Some(WindowChange::Topology),
        EVENT_OBJECT_LOCATIONCHANGE => Some(WindowChange::Geometry),
        _ => None,
    }
}

fn record_window_change(change: WindowChange) {
    if change == WindowChange::Topology {
        TOPOLOGY_GENERATION.fetch_add(1, Ordering::AcqRel);
    }
    GEOMETRY_GENERATION.fetch_add(1, Ordering::AcqRel);
}

#[must_use]
pub fn window_change_generations() -> Option<WindowChangeGenerations> {
    HOOKS_LIVE
        .load(Ordering::Acquire)
        .then(|| WindowChangeGenerations {
            topology: TOPOLOGY_GENERATION.load(Ordering::Acquire),
            geometry: GEOMETRY_GENERATION.load(Ordering::Acquire),
        })
}

#[must_use]
pub fn poll_foreground_change() -> bool {
    let current = unsafe { GetForegroundWindow() }.0 as isize;
    if current == 0 || LAST_POLLED_FOREGROUND.swap(current, Ordering::AcqRel) == current {
        return false;
    }
    LAST_FOREGROUND.store(current, Ordering::Release);
    true
}

pub(crate) fn current_foreground() -> HWND {
    let announced = HWND(LAST_FOREGROUND.load(Ordering::Acquire) as *mut _);
    if !announced.0.is_null() && unsafe { IsWindow(Some(announced)) }.as_bool() {
        return announced;
    }
    unsafe { GetForegroundWindow() }
}

fn install_hook(slot: &AtomicPtr<c_void>, (first, last): (u32, u32)) -> bool {
    if !slot.load(Ordering::SeqCst).is_null() {
        return true;
    }
    let hook = unsafe {
        SetWinEventHook(
            first,
            last,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    if hook.0.is_null() {
        return false;
    }
    slot.store(hook.0, Ordering::SeqCst);
    true
}

fn remove_hook(slot: &AtomicPtr<c_void>) {
    let previous = slot.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !previous.is_null() {
        unsafe {
            let _ = UnhookWinEvent(HWINEVENTHOOK(previous));
        }
    }
}

pub fn install_window_event_hooks() -> bool {
    if HOOKS_LIVE.load(Ordering::Acquire) {
        return true;
    }
    let installed = HOOKS
        .iter()
        .zip(HOOKED_EVENT_RANGES)
        .all(|(slot, range)| install_hook(slot, range));
    if !installed {
        uninstall_window_event_hooks();
        return false;
    }
    HOOKS_LIVE.store(true, Ordering::Release);
    true
}

pub fn uninstall_window_event_hooks() {
    HOOKS_LIVE.store(false, Ordering::Release);
    for slot in &HOOKS {
        remove_hook(slot);
    }
    LAST_FOREGROUND.store(0, Ordering::SeqCst);
    LAST_POLLED_FOREGROUND.store(0, Ordering::SeqCst);
}

pub fn install_window_tracking_hook(target_hwnd: isize, message: u32) -> bool {
    WIN_EVENT_TARGET.store(target_hwnd, Ordering::SeqCst);
    WIN_EVENT_MSG.store(message, Ordering::SeqCst);
    SYNC_PENDING.store(false, Ordering::SeqCst);
    let installed = install_window_event_hooks();
    if !installed {
        uninstall_window_tracking_hook();
    }
    installed
}

pub fn uninstall_window_tracking_hook() {
    WIN_EVENT_TARGET.store(0, Ordering::SeqCst);
    WIN_EVENT_MSG.store(0, Ordering::SeqCst);
    SYNC_PENDING.store(false, Ordering::SeqCst);
    LAST_POLLED_FOREGROUND.store(0, Ordering::SeqCst);
}

pub fn acknowledge_window_sync() {
    SYNC_PENDING.store(false, Ordering::Release);
}

fn post_window_sync(source: HWND, event: u32) {
    let target = WIN_EVENT_TARGET.load(Ordering::Relaxed);
    let message = WIN_EVENT_MSG.load(Ordering::Relaxed);
    if target == 0 || message == 0 || SYNC_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let posted = unsafe {
        PostMessageW(
            Some(HWND(target as *mut _)),
            message,
            WPARAM(source.0 as usize),
            LPARAM(event as isize),
        )
    };
    if posted.is_err() {
        SYNC_PENDING.store(false, Ordering::Release);
    }
}

fn is_top_level_or_gone(hwnd: HWND) -> bool {
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    root.0.is_null() || root == hwnd
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _event_time: u32,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 || hwnd.0.is_null() {
            return;
        }
        if let Some(change) = classify_window_event(event, is_top_level_or_gone(hwnd)) {
            record_window_change(change);
        }
        if event == EVENT_SYSTEM_FOREGROUND {
            LAST_FOREGROUND.store(hwnd.0 as isize, Ordering::Release);
        }
        let relevant = event == EVENT_SYSTEM_FOREGROUND
            || (event == EVENT_OBJECT_SHOW
                && hwnd.0 as isize == LAST_FOREGROUND.load(Ordering::Acquire))
            || (event == EVENT_OBJECT_LOCATIONCHANGE && current_foreground() == hwnd);
        if relevant {
            post_window_sync(hwnd, event);
        }
    }));
}

pub fn suppress_native_osd() {
    unsafe {
        if let Ok(hud) = FindWindowW(w!("NativeHUD"), PCWSTR::null())
            && !hud.0.is_null()
        {
            let _ = ShowWindow(hud, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
        EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED,
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART,
    };

    use super::{WindowChange, classify_window_event};

    #[test]
    fn top_level_lifecycle_events_change_the_window_topology() {
        for event in [
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_MINIMIZESTART,
            EVENT_SYSTEM_MINIMIZEEND,
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_HIDE,
            EVENT_OBJECT_CLOAKED,
            EVENT_OBJECT_UNCLOAKED,
        ] {
            assert_eq!(
                classify_window_event(event, true),
                Some(WindowChange::Topology)
            );
        }
    }

    #[test]
    fn top_level_moves_change_only_geometry() {
        assert_eq!(
            classify_window_event(EVENT_OBJECT_LOCATIONCHANGE, true),
            Some(WindowChange::Geometry)
        );
    }

    #[test]
    fn child_windows_and_unrelated_events_are_ignored() {
        assert_eq!(classify_window_event(EVENT_OBJECT_SHOW, false), None);
        assert_eq!(
            classify_window_event(EVENT_OBJECT_LOCATIONCHANGE, false),
            None
        );
        assert_eq!(classify_window_event(EVENT_OBJECT_NAMECHANGE, true), None);
    }
}
