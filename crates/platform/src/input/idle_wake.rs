use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use otto_core::{MonitorEdgeWatch, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, PostMessageW};

struct EdgeWatches {
    top: Vec<MonitorEdgeWatch>,
    bottom: Vec<MonitorEdgeWatch>,
}

impl EdgeWatches {
    const fn new() -> Self {
        Self {
            top: Vec::new(),
            bottom: Vec::new(),
        }
    }

    fn is_watching(&self) -> bool {
        self.top.iter().any(MonitorEdgeWatch::is_watching)
            || self.bottom.iter().any(MonitorEdgeWatch::is_watching)
    }

    fn touches(&self, x: i32, y: i32) -> bool {
        self.top.iter().any(|watch| watch.touches(x, y))
            || self.bottom.iter().any(|watch| watch.touches(x, y))
    }
}

static WAKE_TARGET: AtomicIsize = AtomicIsize::new(0);
static WAKE_MESSAGE: AtomicU32 = AtomicU32::new(0);
static WAKE_PENDING: AtomicBool = AtomicBool::new(false);
static ANY_EDGE_WATCHED: AtomicBool = AtomicBool::new(false);
static EDGE_WATCHES: Mutex<EdgeWatches> = Mutex::new(EdgeWatches::new());

pub fn register_idle_wake(target_hwnd: isize, message: u32) {
    WAKE_PENDING.store(false, Ordering::SeqCst);
    WAKE_MESSAGE.store(message, Ordering::SeqCst);
    WAKE_TARGET.store(target_hwnd, Ordering::SeqCst);
}

pub fn unregister_idle_wake() {
    WAKE_TARGET.store(0, Ordering::SeqCst);
    WAKE_MESSAGE.store(0, Ordering::SeqCst);
    WAKE_PENDING.store(false, Ordering::SeqCst);
    ANY_EDGE_WATCHED.store(false, Ordering::SeqCst);
    if let Ok(mut watches) = EDGE_WATCHES.lock() {
        watches.top.clear();
        watches.bottom.clear();
    }
}

pub fn acknowledge_idle_wake() {
    WAKE_PENDING.store(false, Ordering::Release);
}

pub fn watch_top_edge(enabled: bool, screen_width: i32, screen_height: i32) {
    if enabled {
        watch_top_edges(&[Rect {
            x: 0,
            y: 0,
            width: screen_width,
            height: screen_height,
        }]);
    } else {
        watch_top_edges(&[]);
    }
}

pub fn watch_top_edges(monitors: &[Rect]) {
    if let Ok(mut watches) = EDGE_WATCHES.lock() {
        watches.top.clear();
        for &bounds in monitors {
            if bounds.width > 0 && bounds.height > 0 {
                watches.top.push(MonitorEdgeWatch {
                    bounds,
                    top: true,
                    bottom_band: None,
                });
            }
        }
        ANY_EDGE_WATCHED.store(watches.is_watching(), Ordering::Release);
    }
}

pub fn watch_bottom_edge(band: Option<i32>, screen_width: i32, screen_height: i32) {
    match band.filter(|&value| value > 0) {
        Some(value) => watch_bottom_edges(&[(
            Rect {
                x: 0,
                y: 0,
                width: screen_width,
                height: screen_height,
            },
            value,
        )]),
        None => watch_bottom_edges(&[]),
    }
}

pub fn watch_bottom_edges(monitors: &[(Rect, i32)]) {
    if let Ok(mut watches) = EDGE_WATCHES.lock() {
        watches.bottom.clear();
        for &(bounds, band) in monitors {
            if band > 0 && bounds.width > 0 && bounds.height > 0 {
                watches.bottom.push(MonitorEdgeWatch {
                    bounds,
                    top: false,
                    bottom_band: Some(band),
                });
            }
        }
        ANY_EDGE_WATCHED.store(watches.is_watching(), Ordering::Release);
    }
}

pub(crate) fn pointer_moved() {
    if !ANY_EDGE_WATCHED.load(Ordering::Acquire) {
        return;
    }
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return;
    }
    let touched = EDGE_WATCHES
        .try_lock()
        .is_ok_and(|watches| watches.touches(point.x, point.y));
    if touched {
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
    use otto_core::{MonitorEdgeWatch, PointerEdgeWatch, Rect};

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

    #[test]
    fn secondary_monitor_watches_edges_in_virtual_coordinates() {
        let right = MonitorEdgeWatch {
            bounds: Rect {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
            top: true,
            bottom_band: Some(6),
        };
        assert!(right.touches(2500, 0));
        assert!(right.touches(2500, 1439));
        assert!(!right.touches(960, 0));
    }
}
