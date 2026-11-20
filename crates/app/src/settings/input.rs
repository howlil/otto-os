use std::mem::size_of;

use otto_core::{SettingsHit, scale};
use otto_platform::{
    apply_settings_min_track, invalidate_settings_window, settings_client_size, suggested_dpi_rect,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_DOWN, VK_ESCAPE,
    VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_CLOSE, WM_DPICHANGED, WM_ERASEBKGND,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_PAINT, WM_SIZE, WM_TIMER,
};

use crate::host::RuntimeContext;
use crate::win_msg::{UPDATE_TIMER_ID, WM_MOUSELEAVE};

use super::actions::{activate, poll_update_check};
use super::lifecycle::close;
use super::{paint, with_window};

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let known = with_window(runtime, |window| window.hwnd == hwnd)?;
    if !known {
        return None;
    }
    match message {
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_PAINT => {
            paint(runtime, hwnd);
            Some(LRESULT(0))
        }
        WM_GETMINMAXINFO => {
            apply_settings_min_track(hwnd, lparam);
            Some(LRESULT(0))
        }
        WM_SIZE => {
            with_window(runtime, |window| {
                window.relayout();
                let (width, height) = settings_client_size(hwnd);
                let resized = window
                    .renderer
                    .as_ref()
                    .is_some_and(|renderer| renderer.resize(width, height).is_ok());
                if !resized {
                    window.renderer = None;
                }
            });
            invalidate_settings_window(hwnd);
            Some(LRESULT(0))
        }
        WM_DPICHANGED => {
            if let Some(rect) = suggested_dpi_rect(lparam) {
                unsafe {
                    let _ = SetWindowPos(
                        HWND(hwnd as *mut _),
                        None,
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            with_window(runtime, |window| {
                window.relayout();
                window.renderer = None;
            });
            invalidate_settings_window(hwnd);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            let changed = with_window(runtime, |window| {
                if !window.tracking_leave {
                    window.tracking_leave = true;
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: HWND(hwnd as *mut _),
                        dwHoverTime: 0,
                    };
                    unsafe {
                        let _ = TrackMouseEvent(&mut track);
                    }
                }
                let hit = window.hit(x, y);
                let changed = hit != window.hovered;
                window.hovered = hit;
                changed
            })
            .unwrap_or(false);
            if changed {
                invalidate_settings_window(hwnd);
            }
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_window(runtime, |window| {
                window.tracking_leave = false;
                window.hovered = None;
            });
            invalidate_settings_window(hwnd);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_window(runtime, |window| window.pressed = window.hit(x, y));
            unsafe {
                let _ = SetCapture(HWND(hwnd as *mut _));
            }
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            unsafe {
                let _ = ReleaseCapture();
            }
            let hit = with_window(runtime, |window| {
                let pressed = window.pressed.take();
                let released = window.hit(x, y);
                (pressed == released).then_some(released).flatten()
            })
            .flatten();
            if let Some(hit) = hit {
                activate(runtime, hit);
            }
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = i32::from(((wparam.0 >> 16) & 0xFFFF) as i16);
            let moved = with_window(runtime, |window| {
                let before = window.scroll;
                window.scroll -= delta * scale(48, window.dpi) / 120;
                window.recompute();
                window.hovered = None;
                window.scroll != before
            })
            .unwrap_or(false);
            if moved {
                invalidate_settings_window(hwnd);
            }
            Some(LRESULT(0))
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u16;
            if key == VK_ESCAPE.0 {
                close(runtime);
                return Some(LRESULT(0));
            }
            let forward = key == VK_DOWN.0;
            if forward || key == VK_UP.0 {
                let next = with_window(runtime, |window| window.page.neighbor(forward)).flatten();
                if let Some(page) = next {
                    activate(runtime, SettingsHit::Page(page));
                }
                return Some(LRESULT(0));
            }
            None
        }
        WM_TIMER if wparam.0 == UPDATE_TIMER_ID => {
            poll_update_check(runtime, hwnd);
            Some(LRESULT(0))
        }
        WM_CLOSE => {
            close(runtime);
            Some(LRESULT(0))
        }
        _ => None,
    }
}
