use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};

use otto_core::PointerEdgeWatch;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW};

static WAKE_TARGET: AtomicIsize = AtomicIsize::new(0);
static WAKE_MESSAGE: AtomicU32 = AtomicU32::new(0);
static WAKE_PENDING: AtomicBool = AtomicBool::new(false);
static EDGE_SCREEN_WIDTH: AtomicI32 = AtomicI32::new(0);
static EDGE_SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(0);
static EDGE_TOP: AtomicBool = AtomicBool::new(false);
static EDGE_BOTTOM_BAND: AtomicI32 = AtomicI32::new(0);

pub fn register_idle_wake(target_hwnd: isize, message: u32) {
    WAKE_PENDING.store(false, Ordering::SeqCst);
    WAKE_MESSAGE.store(message, Ordering::SeqCst);
    WAKE_TARGET.store(target_hwnd, Ordering::SeqCst);
}

pub fn unregister_idle_wake() {
    WAKE_TARGET.store(0, Ordering::SeqCst);
    WAKE_MESSAGE.store(0, Ordering::SeqCst);
    WAKE_PENDING.store(false, Ordering::SeqCst);
    EDGE_TOP.store(false, Ordering::SeqCst);
    EDGE_BOTTOM_BAND.store(0, Ordering::SeqCst);
}

pub fn acknowledge_idle_wake() {
    WAKE_PENDING.store(false, Ordering::Release);
}

pub fn watch_top_edge(enabled: bool, screen_width: i32, screen_height: i32) {
    store_edge_screen(screen_width, screen_height);
    EDGE_TOP.store(enabled, Ordering::Release);
}

pub fn watch_bottom_edge(band: Option<i32>, screen_width: i32, screen_height: i32) {
    store_edge_screen(screen_width, screen_height);
    EDGE_BOTTOM_BAND.store(band.unwrap_or(0).max(0), Ordering::Release);
}

fn store_edge_screen(screen_width: i32, screen_height: i32) {
    EDGE_SCREEN_WIDTH.store(screen_width, Ordering::Release);
    EDGE_SCREEN_HEIGHT.store(screen_height, Ordering::Release);
}

fn edge_watch() -> PointerEdgeWatch {
    let band = EDGE_BOTTOM_BAND.load(Ordering::Acquire);
    PointerEdgeWatch {
        screen_width: EDGE_SCREEN_WIDTH.load(Ordering::Acquire),
        screen_height: EDGE_SCREEN_HEIGHT.load(Ordering::Acquire),
        top: EDGE_TOP.load(Ordering::Acquire),
        bottom_band: (band > 0).then_some(band),
    }
}

pub(crate) fn pointer_moved() {
    let edges = edge_watch();
    if !edges.is_watching() {
        return;
    }
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_ok() && edges.touches(point.x, point.y) {
        request_idle_wake();
    }
}

pub(crate) fn request_idle_wake() {
    let target = WAKE_TARGET.load(Ordering::Acquire);
    let message = WAKE_MESSAGE.load(Ordering::Acquire);
    if target == 0 || message == 0 || WAKE_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let posted =
        unsafe { PostMessageW(Some(HWND(target as *mut _)), message, WPARAM(0), LPARAM(0)) };
    if posted.is_err() {
        WAKE_PENDING.store(false, Ordering::Release);
    }
}
