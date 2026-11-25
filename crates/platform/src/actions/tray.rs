use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowRect, HWND_TOPMOST, IsWindowVisible, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow,
};
use windows::core::{PCWSTR, w};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayOverflowOutcome {
    Opened,
    Closed,
    Unavailable,
}

pub(super) fn tray_overflow_size(rect: &RECT) -> Option<(i32, i32)> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    (width > 0 && height > 0).then_some((width, height))
}

pub(super) fn find_tray_overflow_window() -> Option<HWND> {
    [
        w!("NotifyIconOverflowWindow"),
        w!("TopLevelWindowForOverflowXamlIsland"),
    ]
    .into_iter()
    .find_map(|class| {
        let window = unsafe { FindWindowW(class, PCWSTR::null()) }.ok()?;
        (!window.0.is_null()).then_some(window)
    })
}

#[must_use]
pub fn close_tray_overflow() -> TrayOverflowOutcome {
    unsafe {
        let Some(overflow) = find_tray_overflow_window() else {
            return TrayOverflowOutcome::Unavailable;
        };
        if IsWindowVisible(overflow).as_bool() {
            let _ = ShowWindow(overflow, SW_HIDE);
        }
        if IsWindowVisible(overflow).as_bool() {
            TrayOverflowOutcome::Unavailable
        } else {
            TrayOverflowOutcome::Closed
        }
    }
}

#[must_use]
pub fn toggle_tray_overflow(
    anchor_center_x: i32,
    bar_height: i32,
    screen_width: i32,
) -> TrayOverflowOutcome {
    toggle_tray_overflow_in_bounds(
        anchor_center_x,
        bar_height,
        otto_core::Rect {
            x: 0,
            y: 0,
            width: screen_width,
            height: 0,
        },
    )
}

#[must_use]
pub fn toggle_tray_overflow_in_bounds(
    anchor_center_x: i32,
    bar_bottom_y: i32,
    bounds: otto_core::Rect,
) -> TrayOverflowOutcome {
    unsafe {
        let Some(overflow) = find_tray_overflow_window() else {
            return TrayOverflowOutcome::Unavailable;
        };
        if IsWindowVisible(overflow).as_bool() {
            return close_tray_overflow();
        }
        let _ = ShowWindow(overflow, SW_SHOWNOACTIVATE);
        let mut rect = RECT::default();
        if GetWindowRect(overflow, &mut rect).is_err() {
            let _ = ShowWindow(overflow, SW_HIDE);
            return TrayOverflowOutcome::Unavailable;
        }
        let Some((width, _)) = tray_overflow_size(&rect) else {
            let _ = ShowWindow(overflow, SW_HIDE);
            return TrayOverflowOutcome::Unavailable;
        };
        let min_x = bounds.x + 8;
        let max_x = (bounds.right() - width - 8).max(min_x);
        let x = (anchor_center_x - width / 2).clamp(min_x, max_x);
        if SetWindowPos(
            overflow,
            Some(HWND_TOPMOST),
            x,
            bar_bottom_y + 6,
            0,
            0,
            SWP_NOSIZE | SWP_SHOWWINDOW,
        )
        .is_err()
        {
            let _ = ShowWindow(overflow, SW_HIDE);
            return TrayOverflowOutcome::Unavailable;
        }
        let _ = SetForegroundWindow(overflow);
        if IsWindowVisible(overflow).as_bool() {
            TrayOverflowOutcome::Opened
        } else {
            TrayOverflowOutcome::Unavailable
        }
    }
}
