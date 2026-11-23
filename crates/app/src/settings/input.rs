use std::mem::size_of;

use otto_core::{
    ClipboardPayload, FocusSettingsTab, FocusTextField, SettingsHit, SettingsPage, scale,
};
use otto_platform::{
    ClipboardRead, apply_settings_min_track, invalidate_settings_window,
    is_settings_window_visible, read_clipboard_payload, settings_client_size, suggested_dpi_rect,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_BACK, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_ACTIVATE, WM_CHAR, WM_CLOSE, WM_DPICHANGED,
    WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_MOVE, WM_PAINT, WM_SETTINGCHANGE, WM_SIZE,
    WM_SYSCOLORCHANGE, WM_THEMECHANGED, WM_TIMER, WM_WINDOWPOSCHANGED,
};

use crate::host::RuntimeContext;
use crate::win_msg::{AUDIT_TIMER_ID, UPDATE_TIMER_ID, WM_MOUSELEAVE, WM_UNICHAR};

use super::actions::{activate, poll_update_check, submit_blocked_website};
use super::lifecycle::{close, refresh_appearance, repaint};
use super::{paint, with_window};

const UNICODE_NOCHAR: usize = 0xFFFF;

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn handle_char_unit(runtime: &RuntimeContext, unit: u16) -> bool {
    let changed = with_window(runtime, |window| {
        if window.page != SettingsPage::Focus
            || window.focus_state.focused_field == FocusTextField::None
        {
            window.pending_high_surrogate = None;
            return false;
        }
        if (0xD800..=0xDBFF).contains(&unit) {
            window.pending_high_surrogate = Some(unit);
            return false;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            if let Some(high) = window.pending_high_surrogate.take() {
                let high = u32::from(high - 0xD800);
                let low = u32::from(unit - 0xDC00);
                if let Some(ch) = char::from_u32(0x1_0000 + (high << 10) + low) {
                    return window.focus_state.insert_char(ch);
                }
            }
            return false;
        }
        window.pending_high_surrogate = None;
        char::from_u32(u32::from(unit)).is_some_and(|ch| window.focus_state.insert_char(ch))
    })
    .unwrap_or(false);
    if changed {
        repaint(runtime);
    }
    changed
}

fn handle_focused_input_keydown(
    runtime: &RuntimeContext,
    focused_field: FocusTextField,
    key: u16,
) -> LRESULT {
    let ctrl_down = (unsafe { GetAsyncKeyState(0x11) } as u16 & 0x8000) != 0;
    if ctrl_down && key == 0x56 {
        if let ClipboardRead::Captured {
            payload: ClipboardPayload::Text(text) | ClipboardPayload::Link(text),
            ..
        } = read_clipboard_payload()
        {
            let changed = with_window(runtime, |window| window.focus_state.insert_text(&text))
                .unwrap_or(false);
            if changed {
                repaint(runtime);
            }
        }
        return LRESULT(0);
    }
    if ctrl_down && key == VK_BACK.0 {
        let changed = with_window(runtime, |window| {
            window.focus_state.clear_field(focused_field)
        })
        .unwrap_or(false);
        if changed {
            repaint(runtime);
        }
        return LRESULT(0);
    }
    match key {
        value if value == VK_ESCAPE.0 => {
            with_window(runtime, |window| {
                if !window.focus_state.clear_field(focused_field) {
                    window.focus_state.focused_field = FocusTextField::None;
                }
            });
            repaint(runtime);
        }
        value if value == VK_BACK.0 => {
            let changed =
                with_window(runtime, |window| window.focus_state.backspace()).unwrap_or(false);
            if changed {
                repaint(runtime);
            }
        }
        value if value == VK_RETURN.0 => {
            if focused_field == FocusTextField::WebsiteDomain {
                submit_blocked_website(runtime);
            } else {
                with_window(runtime, |window| {
                    window.focus_state.focused_field = FocusTextField::None;
                });
            }
            repaint(runtime);
        }
        _ => {}
    }
    LRESULT(0)
}

fn handle_keydown(runtime: &RuntimeContext, key: u16) -> Option<LRESULT> {
    let focused_field = with_window(runtime, |window| {
        if window.page == SettingsPage::Focus {
            window.focus_state.focused_field
        } else {
            FocusTextField::None
        }
    })
    .unwrap_or(FocusTextField::None);

    if focused_field != FocusTextField::None {
        return Some(handle_focused_input_keydown(runtime, focused_field, key));
    }

    if key == VK_ESCAPE.0 {
        close(runtime);
        return Some(LRESULT(0));
    }
    if key == VK_LEFT.0 || key == VK_RIGHT.0 {
        let delta = if key == VK_LEFT.0 { -1 } else { 1 };
        let stepped = with_window(runtime, |window| {
            window.page == SettingsPage::Focus && window.focus_state.step_chart_day(delta)
        })
        .unwrap_or(false);
        if stepped {
            repaint(runtime);
            return Some(LRESULT(0));
        }
    }
    if key == VK_TAB.0 {
        let toggled = with_window(runtime, |window| {
            if window.page != SettingsPage::Focus {
                return false;
            }
            let next = match window.focus_state.tab {
                FocusSettingsTab::BlockApps => FocusSettingsTab::Activity,
                FocusSettingsTab::Activity => FocusSettingsTab::BlockApps,
            };
            window.focus_state.select_tab(next);
            window.scroll = 0;
            true
        })
        .unwrap_or(false);
        if toggled {
            repaint(runtime);
            return Some(LRESULT(0));
        }
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
                    window.icons_attempted.clear();
                }
            });
            invalidate_settings_window(hwnd);
            crate::window_controls::notify_own_window_change(runtime);
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
                window.icons_attempted.clear();
            });
            invalidate_settings_window(hwnd);
            crate::window_controls::notify_own_window_change(runtime);
            Some(LRESULT(0))
        }
        WM_MOVE | WM_ACTIVATE | WM_WINDOWPOSCHANGED => {
            crate::window_controls::notify_own_window_change(runtime);
            None
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_SYSCOLORCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
            crate::theme_state::refresh_system_appearance(runtime);
            refresh_appearance(runtime);
            None
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
        WM_CHAR => {
            handle_char_unit(runtime, wparam.0 as u16);
            Some(LRESULT(0))
        }
        WM_UNICHAR => {
            if wparam.0 == UNICODE_NOCHAR {
                return Some(LRESULT(1));
            }
            let changed = with_window(runtime, |window| {
                window.page == SettingsPage::Focus
                    && window.focus_state.focused_field != FocusTextField::None
                    && char::from_u32(wparam.0 as u32)
                        .is_some_and(|ch| window.focus_state.insert_char(ch))
            })
            .unwrap_or(false);
            if changed {
                repaint(runtime);
            }
            Some(LRESULT(0))
        }
        WM_KEYDOWN => handle_keydown(runtime, wparam.0 as u16),
        WM_TIMER if wparam.0 == UPDATE_TIMER_ID => {
            poll_update_check(runtime, hwnd);
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == AUDIT_TIMER_ID => {
            let should_refresh = with_window(runtime, |window| {
                if window.page != SettingsPage::General
                    || !is_settings_window_visible(window.hwnd)
                    || window.pressed.is_some()
                    || matches!(
                        window.hovered,
                        Some(SettingsHit::Row { .. } | SettingsHit::Part { .. })
                    )
                {
                    return false;
                }
                window.refresh_process_audit();
                true
            })
            .unwrap_or(false);
            if should_refresh {
                repaint(runtime);
            }
            Some(LRESULT(0))
        }
        WM_CLOSE => {
            close(runtime);
            Some(LRESULT(0))
        }
        _ => None,
    }
}
