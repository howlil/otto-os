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
    edge_watch_from(
        EDGE_SCREEN_WIDTH.load(Ordering::Acquire),
        EDGE_SCREEN_HEIGHT.load(Ordering::Acquire),
        EDGE_TOP.load(Ordering::Acquire),
        EDGE_BOTTOM_BAND.load(Ordering::Acquire),
    )
}

const fn edge_watch_from(
    screen_width: i32,
    screen_height: i32,
    top: bool,
    bottom_band: i32,
) -> PointerEdgeWatch {
    PointerEdgeWatch {
        screen_width,
        screen_height,
        top,
        bottom_band: if bottom_band > 0 {
            Some(bottom_band)
        } else {
            None
        },
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

#[cfg(test)]
mod tests {
    use super::edge_watch_from;

    #[test]
    fn a_zero_band_means_no_bottom_watch() {
        let edges = edge_watch_from(1920, 1080, false, 0);
        assert_eq!(edges.bottom_band, None);
        assert!(!edges.is_watching());
    }

    #[test]
    fn a_positive_band_watches_the_bottom_rows() {
        let edges = edge_watch_from(1920, 1080, false, 6);
        assert_eq!(edges.bottom_band, Some(6));
        assert!(edges.touches(960, 1079));
        assert!(!edges.touches(960, 500));
    }

    #[test]
    fn the_top_flag_is_carried_through() {
        let edges = edge_watch_from(1920, 1080, true, 0);
        assert!(edges.touches(960, 0));
        assert!(edges.is_watching());
    }
}
