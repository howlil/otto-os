use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicU32, AtomicU64, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_REORDER, EVENT_OBJECT_SHOW,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_CAPTURESTART, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MOVESIZEEND,
    EVENT_SYSTEM_MOVESIZESTART, FindWindowW, GA_ROOT, GetAncestor, GetForegroundWindow, IsWindow,
    OBJID_CURSOR, OBJID_WINDOW, PostMessageW, SW_HIDE, ShowWindow, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};
use windows::core::{PCWSTR, w};

const HOOKED_EVENT_RANGES: [(u32, u32); 9] = [
    (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
    (EVENT_SYSTEM_CAPTURESTART, EVENT_SYSTEM_CAPTURESTART),
    (EVENT_SYSTEM_MOVESIZESTART, EVENT_SYSTEM_MOVESIZEEND),
    (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
    (EVENT_OBJECT_REORDER, EVENT_OBJECT_REORDER),
    (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
    (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
    (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
    (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
];

const SYNC_FLAG_TOPOLOGY: u32 = 1 << 0;
const SYNC_FLAG_GEOMETRY: u32 = 1 << 1;
const SYNC_FLAG_MULTI_MOVE: u32 = 1 << 2;
const SYNC_FLAG_SETTLE: u32 = 1 << 3;
const SYNC_FLAG_CAPTURE: u32 = 1 << 4;
const SYNC_FLAG_MOVESIZE_START: u32 = 1 << 5;
const SYNC_FLAG_MOVESIZE_END: u32 = 1 << 6;

static HOOKS: [AtomicPtr<c_void>; HOOKED_EVENT_RANGES.len()] =
    [const { AtomicPtr::new(std::ptr::null_mut()) }; HOOKED_EVENT_RANGES.len()];
static HOOKS_LIVE: AtomicBool = AtomicBool::new(false);
static TOPOLOGY_GENERATION: AtomicU64 = AtomicU64::new(0);
static GEOMETRY_GENERATION: AtomicU64 = AtomicU64::new(0);
static TITLE_GENERATION: AtomicU64 = AtomicU64::new(0);
static WIN_EVENT_TARGET: AtomicIsize = AtomicIsize::new(0);
static WIN_EVENT_MSG: AtomicU32 = AtomicU32::new(0);
static SYNC_PENDING: AtomicBool = AtomicBool::new(false);
static PENDING_SYNC_FLAGS: AtomicU32 = AtomicU32::new(0);
static LAST_MOVED_HWND: AtomicIsize = AtomicIsize::new(0);
static LAST_CAPTURE_HWND: AtomicIsize = AtomicIsize::new(0);
static LAST_MOVESIZE_START_HWND: AtomicIsize = AtomicIsize::new(0);
static LAST_MOVESIZE_END_HWND: AtomicIsize = AtomicIsize::new(0);
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static LAST_POLLED_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowChangeGenerations {
    pub topology: u64,
    pub geometry: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowSyncEvent {
    pub topology: bool,
    pub settle: bool,
    pub moved_hwnd: isize,
    pub capture_hwnd: isize,
    pub move_size_start: isize,
    pub move_size_end: isize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowChange {
    Topology,
    Geometry,
    OverlaySync,
}

fn classify_window_event(event: u32, top_level: bool, is_foreground: bool) -> Option<WindowChange> {
    if !top_level {
        return None;
    }
    match event {
        EVENT_SYSTEM_FOREGROUND
        | EVENT_SYSTEM_MINIMIZESTART
        | EVENT_SYSTEM_MINIMIZEEND
        | EVENT_SYSTEM_MOVESIZEEND
        | EVENT_OBJECT_DESTROY
        | EVENT_OBJECT_SHOW
        | EVENT_OBJECT_HIDE
        | EVENT_OBJECT_CLOAKED
        | EVENT_OBJECT_UNCLOAKED => Some(WindowChange::Topology),
        EVENT_OBJECT_LOCATIONCHANGE => Some(WindowChange::Geometry),
        EVENT_OBJECT_REORDER | EVENT_SYSTEM_MOVESIZESTART => Some(WindowChange::OverlaySync),
        EVENT_OBJECT_NAMECHANGE if is_foreground => Some(WindowChange::OverlaySync),
        _ => None,
    }
}

fn record_window_change(change: WindowChange) {
    match change {
        WindowChange::Topology => {
            TOPOLOGY_GENERATION.fetch_add(1, Ordering::AcqRel);
            GEOMETRY_GENERATION.fetch_add(1, Ordering::AcqRel);
        }
        WindowChange::Geometry => {
            GEOMETRY_GENERATION.fetch_add(1, Ordering::AcqRel);
        }
        WindowChange::OverlaySync => {}
    }
}

#[must_use]
pub const fn is_name_change_sync(event: u32) -> bool {
    event == EVENT_OBJECT_NAMECHANGE
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
pub fn foreground_title_generation() -> Option<u64> {
    HOOKS_LIVE
        .load(Ordering::Acquire)
        .then(|| TITLE_GENERATION.load(Ordering::Acquire))
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
    unsafe {
        if !announced.0.is_null() && IsWindow(Some(announced)).as_bool() {
            return announced;
        }
        GetForegroundWindow()
    }
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
    if slot
        .compare_exchange(
            std::ptr::null_mut(),
            hook.0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_err()
    {
        unsafe {
            let _ = UnhookWinEvent(hook);
        }
    }
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

fn clear_pending_sync_state() {
    SYNC_PENDING.store(false, Ordering::SeqCst);
    PENDING_SYNC_FLAGS.store(0, Ordering::SeqCst);
    LAST_MOVED_HWND.store(0, Ordering::SeqCst);
    LAST_CAPTURE_HWND.store(0, Ordering::SeqCst);
    LAST_MOVESIZE_START_HWND.store(0, Ordering::SeqCst);
    LAST_MOVESIZE_END_HWND.store(0, Ordering::SeqCst);
}

pub fn install_window_tracking_hook(target_hwnd: isize, message: u32) -> bool {
    WIN_EVENT_TARGET.store(target_hwnd, Ordering::SeqCst);
    WIN_EVENT_MSG.store(message, Ordering::SeqCst);
    clear_pending_sync_state();
    let installed = install_window_event_hooks();
    if !installed {
        uninstall_window_tracking_hook();
    }
    installed
}

pub fn uninstall_window_tracking_hook() {
    WIN_EVENT_TARGET.store(0, Ordering::SeqCst);
    WIN_EVENT_MSG.store(0, Ordering::SeqCst);
    clear_pending_sync_state();
    LAST_POLLED_FOREGROUND.store(0, Ordering::SeqCst);
}

#[must_use]
pub fn take_window_sync_event() -> WindowSyncEvent {
    let flags = PENDING_SYNC_FLAGS.swap(0, Ordering::AcqRel);
    let moved_raw = LAST_MOVED_HWND.swap(0, Ordering::AcqRel);
    let capture_raw = LAST_CAPTURE_HWND.swap(0, Ordering::AcqRel);
    let start_raw = LAST_MOVESIZE_START_HWND.swap(0, Ordering::AcqRel);
    let end_raw = LAST_MOVESIZE_END_HWND.swap(0, Ordering::AcqRel);
    SYNC_PENDING.store(false, Ordering::Release);
    let topology = flags & (SYNC_FLAG_TOPOLOGY | SYNC_FLAG_MULTI_MOVE) != 0;
    WindowSyncEvent {
        topology,
        settle: flags & SYNC_FLAG_SETTLE != 0,
        moved_hwnd: if flags & SYNC_FLAG_GEOMETRY != 0 {
            moved_raw
        } else {
            0
        },
        capture_hwnd: if flags & SYNC_FLAG_CAPTURE != 0 {
            capture_raw
        } else {
            0
        },
        move_size_start: if flags & SYNC_FLAG_MOVESIZE_START != 0 {
            start_raw
        } else {
            0
        },
        move_size_end: if flags & SYNC_FLAG_MOVESIZE_END != 0 {
            end_raw
        } else {
            0
        },
    }
}

#[must_use]
pub fn acknowledge_window_sync() -> bool {
    take_window_sync_event().settle
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

pub(crate) fn notify_caption_probe_ready() {
    PENDING_SYNC_FLAGS.fetch_or(SYNC_FLAG_TOPOLOGY | SYNC_FLAG_SETTLE, Ordering::AcqRel);
    post_window_sync(HWND::default(), EVENT_OBJECT_REORDER);
    crate::idle_wake::request_idle_wake();
}

fn root_of(hwnd: HWND) -> HWND {
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root.0.is_null() { hwnd } else { root }
}

fn is_top_level_or_gone(hwnd: HWND) -> bool {
    root_of(hwnd) == hwnd
}

fn record_sync_signal(event: u32, hwnd: HWND, is_foreground: bool) {
    let raw = hwnd.0 as isize;
    match event {
        EVENT_OBJECT_LOCATIONCHANGE => {
            let previous = LAST_MOVED_HWND.swap(raw, Ordering::AcqRel);
            let mut bits = SYNC_FLAG_GEOMETRY;
            if previous != 0 && previous != raw {
                bits |= SYNC_FLAG_MULTI_MOVE;
            }
            PENDING_SYNC_FLAGS.fetch_or(bits, Ordering::AcqRel);
        }
        EVENT_SYSTEM_MOVESIZESTART => {
            LAST_MOVESIZE_START_HWND.store(raw, Ordering::Release);
            PENDING_SYNC_FLAGS.fetch_or(SYNC_FLAG_MOVESIZE_START, Ordering::AcqRel);
        }
        EVENT_SYSTEM_MOVESIZEEND => {
            LAST_MOVESIZE_END_HWND.store(raw, Ordering::Release);
            PENDING_SYNC_FLAGS.fetch_or(
                SYNC_FLAG_MOVESIZE_END | SYNC_FLAG_TOPOLOGY | SYNC_FLAG_SETTLE,
                Ordering::AcqRel,
            );
        }
        EVENT_SYSTEM_FOREGROUND => {
            PENDING_SYNC_FLAGS.fetch_or(SYNC_FLAG_TOPOLOGY | SYNC_FLAG_SETTLE, Ordering::AcqRel);
        }
        EVENT_OBJECT_NAMECHANGE if is_foreground => {
            PENDING_SYNC_FLAGS.fetch_or(SYNC_FLAG_TOPOLOGY | SYNC_FLAG_SETTLE, Ordering::AcqRel);
        }
        _ => {
            PENDING_SYNC_FLAGS.fetch_or(SYNC_FLAG_TOPOLOGY, Ordering::AcqRel);
        }
    }
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
        if event == EVENT_OBJECT_LOCATIONCHANGE && id_object == OBJID_CURSOR.0 {
            crate::idle_wake::pointer_moved();
            return;
        }
        if event == EVENT_SYSTEM_CAPTURESTART && !hwnd.0.is_null() {
            let root = root_of(hwnd);
            if !root.0.is_null() {
                LAST_CAPTURE_HWND.store(root.0 as isize, Ordering::Release);
                PENDING_SYNC_FLAGS.fetch_or(SYNC_FLAG_CAPTURE, Ordering::AcqRel);
                post_window_sync(root, event);
            }
            return;
        }
        if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 || hwnd.0.is_null() {
            return;
        }
        let is_foreground = if event == EVENT_OBJECT_NAMECHANGE {
            if hwnd != current_foreground() {
                return;
            }
            true
        } else {
            false
        };
        let change = classify_window_event(event, is_top_level_or_gone(hwnd), is_foreground);
        if let Some(change) = change {
            record_window_change(change);
        }
        if event == EVENT_SYSTEM_FOREGROUND {
            LAST_FOREGROUND.store(hwnd.0 as isize, Ordering::Release);
            TITLE_GENERATION.fetch_add(1, Ordering::AcqRel);
        } else if event == EVENT_OBJECT_NAMECHANGE && is_foreground {
            TITLE_GENERATION.fetch_add(1, Ordering::AcqRel);
        }
        let relevant = event == EVENT_SYSTEM_FOREGROUND || change.is_some();
        if relevant {
            record_sync_signal(event, hwnd, is_foreground);
            post_window_sync(hwnd, event);
            crate::idle_wake::request_idle_wake();
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
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
        EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_REORDER, EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED,
        EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART,
        EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART,
    };

    use super::{
        WindowChange, classify_window_event, clear_pending_sync_state, record_sync_signal,
        take_window_sync_event,
    };

    #[test]
    fn top_level_lifecycle_events_change_the_window_topology() {
        for event in [
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_MINIMIZESTART,
            EVENT_SYSTEM_MINIMIZEEND,
            EVENT_SYSTEM_MOVESIZEEND,
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_HIDE,
            EVENT_OBJECT_CLOAKED,
            EVENT_OBJECT_UNCLOAKED,
        ] {
            assert_eq!(
                classify_window_event(event, true, false),
                Some(WindowChange::Topology)
            );
        }
    }

    #[test]
    fn top_level_moves_change_only_geometry() {
        assert_eq!(
            classify_window_event(EVENT_OBJECT_LOCATIONCHANGE, true, false),
            Some(WindowChange::Geometry)
        );
    }

    #[test]
    fn zorder_movesize_start_and_foreground_name_changes_sync_overlays_without_bumping_topology() {
        assert_eq!(
            classify_window_event(EVENT_OBJECT_REORDER, true, false),
            Some(WindowChange::OverlaySync)
        );
        assert_eq!(
            classify_window_event(EVENT_SYSTEM_MOVESIZESTART, true, false),
            Some(WindowChange::OverlaySync)
        );
        assert_eq!(
            classify_window_event(EVENT_OBJECT_NAMECHANGE, true, true),
            Some(WindowChange::OverlaySync)
        );
    }

    #[test]
    fn child_windows_and_unrelated_events_are_ignored() {
        assert_eq!(classify_window_event(EVENT_OBJECT_SHOW, false, false), None);
        assert_eq!(
            classify_window_event(EVENT_OBJECT_LOCATIONCHANGE, false, false),
            None
        );
        assert_eq!(
            classify_window_event(EVENT_OBJECT_REORDER, false, false),
            None
        );
        assert_eq!(
            classify_window_event(EVENT_OBJECT_NAMECHANGE, true, false),
            None
        );
        assert_eq!(
            classify_window_event(EVENT_OBJECT_NAMECHANGE, false, true),
            None
        );
    }

    #[test]
    fn single_window_location_change_stays_fast_path_while_multi_move_promotes_to_topology() {
        clear_pending_sync_state();
        record_sync_signal(EVENT_OBJECT_LOCATIONCHANGE, HWND(42 as *mut _), false);
        let single = take_window_sync_event();
        assert!(!single.topology);
        assert_eq!(single.moved_hwnd, 42);

        record_sync_signal(EVENT_OBJECT_LOCATIONCHANGE, HWND(42 as *mut _), false);
        record_sync_signal(EVENT_OBJECT_LOCATIONCHANGE, HWND(99 as *mut _), false);
        let multi = take_window_sync_event();
        assert!(multi.topology);
    }
}
