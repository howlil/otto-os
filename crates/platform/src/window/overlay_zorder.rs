use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::{
    BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, EnumWindows, GW_HWNDNEXT, GW_HWNDPREV,
    GWL_EXSTYLE, GetWindow, GetWindowLongW, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IsIconic,
    IsWindow, IsWindowVisible, IsZoomed, SET_WINDOW_POS_FLAGS, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, WS_EX_TOPMOST,
};
use windows::core::BOOL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastTrackFrame {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub maximized: bool,
}

enum OverlayPlacement {
    Invalid,
    AlreadyVisible,
    NeedsShow,
    NeedsInsert(HWND),
}

fn plan_overlay_placement(overlay: HWND, target: HWND) -> OverlayPlacement {
    unsafe {
        if !IsWindow(Some(overlay)).as_bool() || !IsWindow(Some(target)).as_bool() {
            return OverlayPlacement::Invalid;
        }
        let target_ex = GetWindowLongW(target, GWL_EXSTYLE) as u32;
        let overlay_ex = GetWindowLongW(overlay, GWL_EXSTYLE) as u32;
        let target_topmost = target_ex & WS_EX_TOPMOST.0 != 0;
        let overlay_topmost = overlay_ex & WS_EX_TOPMOST.0 != 0;
        if overlay_topmost != target_topmost {
            let band = if target_topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };
            let _ = SetWindowPos(
                overlay,
                Some(band),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let prev = GetWindow(target, GW_HWNDPREV)
            .ok()
            .filter(|hwnd| !hwnd.0.is_null());
        if prev == Some(overlay) && overlay_topmost == target_topmost {
            return if IsWindowVisible(overlay).as_bool() {
                OverlayPlacement::AlreadyVisible
            } else {
                OverlayPlacement::NeedsShow
            };
        }
        let insert_after = if target_topmost {
            prev.unwrap_or(HWND_TOPMOST)
        } else {
            match prev {
                Some(hwnd) if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOPMOST.0 != 0 => {
                    HWND_TOP
                }
                Some(hwnd) => hwnd,
                None => HWND_TOP,
            }
        };
        OverlayPlacement::NeedsInsert(insert_after)
    }
}

#[must_use]
pub fn place_overlay_above(overlay_hwnd: isize, target_hwnd: isize) -> bool {
    let overlay = HWND(overlay_hwnd as *mut _);
    let target = HWND(target_hwnd as *mut _);
    let (insert_after, extra) = match plan_overlay_placement(overlay, target) {
        OverlayPlacement::Invalid => return false,
        OverlayPlacement::AlreadyVisible => return true,
        OverlayPlacement::NeedsShow => (None, SWP_NOZORDER),
        OverlayPlacement::NeedsInsert(after) => (Some(after), SET_WINDOW_POS_FLAGS::default()),
    };
    unsafe {
        let _ = SetWindowPos(
            overlay,
            insert_after,
            0,
            0,
            0,
            0,
            extra | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    true
}

#[must_use]
pub fn place_overlays_batch(pairs: &[(isize, isize)]) -> Vec<isize> {
    if pairs.is_empty() {
        return Vec::new();
    }
    let mut failed = Vec::new();
    let mut pending = Vec::with_capacity(pairs.len());
    for &(overlay_raw, target_raw) in pairs {
        let overlay = HWND(overlay_raw as *mut _);
        let target = HWND(target_raw as *mut _);
        match plan_overlay_placement(overlay, target) {
            OverlayPlacement::Invalid => failed.push(overlay_raw),
            OverlayPlacement::AlreadyVisible => {}
            OverlayPlacement::NeedsShow => pending.push((overlay, None, SWP_NOZORDER)),
            OverlayPlacement::NeedsInsert(after) => {
                pending.push((overlay, Some(after), SET_WINDOW_POS_FLAGS::default()));
            }
        }
    }
    if pending.is_empty() {
        return failed;
    }
    unsafe {
        let Ok(mut hdwp) = BeginDeferWindowPos(pending.len() as i32) else {
            for (overlay, insert_after, extra) in pending {
                let _ = SetWindowPos(
                    overlay,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    extra | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            return failed;
        };
        for &(overlay, insert_after, extra) in &pending {
            if let Ok(next) = DeferWindowPos(
                hdwp,
                overlay,
                insert_after,
                0,
                0,
                0,
                0,
                extra | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            ) {
                hdwp = next;
            } else {
                for (fb_overlay, fb_after, fb_extra) in pending {
                    let _ = SetWindowPos(
                        fb_overlay,
                        fb_after,
                        0,
                        0,
                        0,
                        0,
                        fb_extra | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
                return failed;
            }
        }
        let _ = EndDeferWindowPos(hdwp);
    }
    failed
}

#[must_use]
pub fn reposition_overlay_for_target(
    overlay_hwnd: isize,
    target_hwnd: isize,
    cover_width: i32,
) -> Option<FastTrackFrame> {
    let overlay = HWND(overlay_hwnd as *mut _);
    let target = HWND(target_hwnd as *mut _);
    let placement = plan_overlay_placement(overlay, target);
    if matches!(placement, OverlayPlacement::Invalid) {
        return None;
    }
    let mut frame = RECT::default();
    let (after, flags) = match placement {
        OverlayPlacement::NeedsInsert(insert_after) => (
            Some(insert_after),
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        ),
        _ => (
            None,
            SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        ),
    };
    unsafe {
        if !IsWindowVisible(target).as_bool() || IsIconic(target).as_bool() {
            return None;
        }
        DwmGetWindowAttribute(
            target,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut frame).cast(),
            size_of::<RECT>() as u32,
        )
        .ok()?;
        let maximized = IsZoomed(target).as_bool();
        let _ = SetWindowPos(
            overlay,
            after,
            frame.right - cover_width,
            frame.top,
            0,
            0,
            flags,
        );
        Some(FastTrackFrame {
            left: frame.left,
            top: frame.top,
            right: frame.right,
            bottom: frame.bottom,
            maximized,
        })
    }
}

fn lowest_anchor_hwnd(overlay: HWND, anchors: &[isize]) -> Option<HWND> {
    struct Probe<'a> {
        overlay: HWND,
        anchors: &'a [isize],
        lowest: Option<HWND>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let probe = &mut *(lparam.0 as *mut Probe<'_>);
            let raw = hwnd.0 as isize;
            if hwnd != probe.overlay
                && probe.anchors.contains(&raw)
                && IsWindowVisible(hwnd).as_bool()
                && (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0) != 0
            {
                probe.lowest = Some(hwnd);
            }
        }
        BOOL(1)
    }
    if anchors.is_empty() {
        return None;
    }
    let mut probe = Probe {
        overlay,
        anchors,
        lowest: None,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.lowest
}

#[must_use]
pub fn place_overlay_below(overlay_hwnd: isize, anchors: &[isize]) -> bool {
    let overlay = HWND(overlay_hwnd as *mut _);
    unsafe {
        if !IsWindow(Some(overlay)).as_bool() {
            return false;
        }
        let Some(anchor) = lowest_anchor_hwnd(overlay, anchors) else {
            return SetWindowPos(
                overlay,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .is_ok();
        };
        let overlay_topmost = (GetWindowLongW(overlay, GWL_EXSTYLE) as u32) & WS_EX_TOPMOST.0 != 0;
        if !overlay_topmost {
            let _ = SetWindowPos(
                overlay,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let already_below = GetWindow(anchor, GW_HWNDNEXT)
            .ok()
            .is_some_and(|next| next == overlay);
        if already_below {
            return true;
        }
        SetWindowPos(
            overlay,
            Some(anchor),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .is_ok()
    }
}
