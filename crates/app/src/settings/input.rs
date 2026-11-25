use otto_core::{
    ClipboardPayload, FocusSettingsTab, FocusTextField, SettingsHit, SettingsPage,
    WindowControlAction, scale,
};
use otto_platform::{
    ClipboardRead, apply_settings_min_track, capture_mouse, client_to_screen, current_process_id,
    handle_settings_ncactivate, handle_settings_nccalcsize, handle_settings_nchittest,
    invalidate_settings_window, is_settings_window_maximized, is_settings_window_visible,
    is_virtual_key_down, kill_window_timer, minimize_settings_window, place_no_zorder,
    read_clipboard_payload, release_capture_if_owned, set_window_timer, settings_client_size,
    suggested_dpi_rect, toggle_settings_window_maximize, track_mouse_leave,
};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_BACK, VK_DOWN, VK_ESCAPE, VK_F4, VK_LEFT, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_ACTIVATE, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DPICHANGED,
    WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST,
    WM_PAINT, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCOLORCHANGE, WM_SYSKEYDOWN,
    WM_THEMECHANGED, WM_TIMER,
};

use crate::host::RuntimeContext;
use crate::win_msg::{
    AUDIT_TIMER_ID, GREEN_HOVER_TIMER, UPDATE_TIMER_ID, WM_MOUSELEAVE, WM_UNICHAR,
};
use crate::window_controls::{GREEN_HOVER_MS, is_alt_pressed, open_tiling_menu};

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
    let ctrl_down = is_virtual_key_down(0x11);
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
            } else if focused_field == FocusTextField::CategoryName {
                super::actions::submit_focus_category(runtime);
            } else if focused_field == FocusTextField::CategoryRename {
                super::actions::submit_rename_focus_category(runtime);
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
                FocusSettingsTab::Modes => FocusSettingsTab::Insights,
                FocusSettingsTab::Insights => FocusSettingsTab::Modes,
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

fn cancel_green_hover(window: &mut super::SettingsWindow) {
    if window.green_hover_armed {
        window.green_hover_armed = false;
        kill_window_timer(window.hwnd, GREEN_HOVER_TIMER);
    }
}

fn sync_green_hover(window: &mut super::SettingsWindow, hit: Option<SettingsHit>) {
    let should_arm =
        hit == Some(SettingsHit::WindowControl(
            WindowControlAction::ToggleMaximize,
        )) && window.pressed.is_none()
            && window.active;
    if should_arm {
        if !window.green_hover_armed {
            window.green_hover_armed =
                set_window_timer(window.hwnd, GREEN_HOVER_TIMER, GREEN_HOVER_MS);
        }
    } else {
        cancel_green_hover(window);
    }
}

fn open_settings_tiling_menu(runtime: &RuntimeContext, hwnd: isize) {
    let Some(zoom_rect) = with_window(runtime, |window| window.layout.window_controls[2].rect)
    else {
        return;
    };
    if zoom_rect.is_empty() {
        return;
    }
    let (anchor_x, anchor_y) = client_to_screen(hwnd, zoom_rect.x, zoom_rect.bottom() + 4);
    open_tiling_menu(runtime, hwnd, current_process_id(), anchor_x, anchor_y);
}

#[allow(clippy::too_many_lines)]
pub(crate) fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if message == WM_NCCALCSIZE {
        return handle_settings_nccalcsize(hwnd, wparam.0, lparam.0).map(LRESULT);
    }
    if message == WM_NCACTIVATE {
        return handle_settings_ncactivate(hwnd).map(LRESULT);
    }
    let known = with_window(runtime, |window| window.hwnd == hwnd)?;
    if !known {
        return None;
    }
    match message {
        WM_SYSKEYDOWN if (wparam.0 & 0xFFFF) as u16 == VK_F4.0 && is_alt_pressed() => {
            close(runtime);
            Some(LRESULT(0))
        }
        WM_NCHITTEST => handle_settings_nchittest(hwnd, lparam.0, |x, y| {
            with_window(runtime, |window| window.layout.is_titlebar_drag(x, y)).unwrap_or(false)
        })
        .map(LRESULT),
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
                window.maximized = is_settings_window_maximized(hwnd);
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
            Some(LRESULT(0))
        }
        WM_DPICHANGED => {
            if let Some(rect) = suggested_dpi_rect(lparam) {
                place_no_zorder(hwnd, rect);
            }
            with_window(runtime, |window| {
                window.maximized = is_settings_window_maximized(hwnd);
                window.relayout();
                window.renderer = None;
                window.icons_attempted.clear();
            });
            invalidate_settings_window(hwnd);
            Some(LRESULT(0))
        }
        WM_ACTIVATE => {
            let active = (wparam.0 & 0xFFFF) != 0;
            with_window(runtime, |window| {
                window.active = active;
                if !active {
                    cancel_green_hover(window);
                    window.alt_down = false;
                }
            });
            invalidate_settings_window(hwnd);
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
                    let _ = track_mouse_leave(hwnd);
                }
                let hit = window.hit(x, y);
                let over_control = matches!(hit, Some(SettingsHit::WindowControl(_)));
                let next_alt = over_control && is_alt_pressed();
                let alt_changed = next_alt != window.alt_down;
                window.alt_down = next_alt;
                sync_green_hover(window, hit);
                let changed = hit != window.hovered || alt_changed;
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
                window.alt_down = false;
                cancel_green_hover(window);
            });
            invalidate_settings_window(hwnd);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            let pressed_control = with_window(runtime, |window| {
                cancel_green_hover(window);
                let hit = window.hit(x, y);
                window.pressed = hit;
                window.hovered = hit;
                matches!(hit, Some(SettingsHit::WindowControl(_)))
            })
            .unwrap_or(false);
            capture_mouse(hwnd);
            if pressed_control {
                invalidate_settings_window(hwnd);
            }
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            let (pressed_was_control, hit) = with_window(runtime, |window| {
                let pressed = window.pressed.take();
                let released = window.hit(x, y);
                window.hovered = released;
                sync_green_hover(window, released);
                let matched = (pressed == released).then_some(released).flatten();
                (
                    matches!(pressed, Some(SettingsHit::WindowControl(_))),
                    matched,
                )
            })
            .unwrap_or((false, None));
            let _ = release_capture_if_owned(hwnd);
            if pressed_was_control {
                invalidate_settings_window(hwnd);
            }
            if let Some(hit) = hit {
                match hit {
                    SettingsHit::WindowControl(WindowControlAction::Close) => close(runtime),
                    SettingsHit::WindowControl(WindowControlAction::Minimize) => {
                        if runtime.genie_enabled.get() {
                            let _ = crate::genie::minimize_window(runtime, hwnd);
                        } else {
                            minimize_settings_window(hwnd);
                        }
                    }
                    SettingsHit::WindowControl(WindowControlAction::ToggleMaximize) => {
                        toggle_settings_window_maximize(hwnd);
                    }
                    other => activate(runtime, other),
                }
            }
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            let had_press =
                with_window(runtime, |window| window.pressed.take().is_some()).unwrap_or(false);
            if had_press {
                invalidate_settings_window(hwnd);
            }
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            let over_controls = with_window(runtime, |window| {
                let hit = window.hit(x, y);
                if matches!(hit, Some(SettingsHit::WindowControl(_))) {
                    cancel_green_hover(window);
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
            over_controls.then(|| {
                open_settings_tiling_menu(runtime, hwnd);
                LRESULT(0)
            })
        }
        WM_TIMER if wparam.0 == GREEN_HOVER_TIMER => {
            let ready = with_window(runtime, |window| {
                cancel_green_hover(window);
                window.hovered
                    == Some(SettingsHit::WindowControl(
                        WindowControlAction::ToggleMaximize,
                    ))
                    && window.pressed.is_none()
                    && window.active
            })
            .unwrap_or(false);
            if ready {
                open_settings_tiling_menu(runtime, hwnd);
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

#[cfg(test)]
mod tests {
    use otto_core::{SettingsHit, SettingsPage, compute_settings_layout};
    use otto_platform::{create_settings_window, destroy_settings_window};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_CAPTURECHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP,
    };

    use super::handle_message;
    use crate::host::{RuntimeContext, SETTINGS_WND_PROC};
    use crate::settings_window::SettingsWindow;

    struct WindowGuard(isize);

    impl Drop for WindowGuard {
        fn drop(&mut self) {
            destroy_settings_window(self.0);
        }
    }

    const fn pack_point(x: i32, y: i32) -> LPARAM {
        LPARAM(((y as u16 as isize) << 16) | (x as u16 as isize))
    }

    #[test]
    fn sidebar_click_navigates_and_capture_loss_cancels_press() {
        let hwnd = create_settings_window(SETTINGS_WND_PROC).expect("hidden settings window");
        let _guard = WindowGuard(hwnd);
        let runtime = RuntimeContext::new(false);
        let mut window = SettingsWindow::new(hwnd);
        window.layout = compute_settings_layout(920, 620, 96);
        let modules_rect = window
            .layout
            .page_rect(SettingsPage::Modules)
            .expect("modules page slot");
        let appearance_rect = window
            .layout
            .page_rect(SettingsPage::Appearance)
            .expect("appearance page slot");
        *runtime.settings_window.borrow_mut() = Some(window);

        let pt = pack_point(
            modules_rect.x + modules_rect.width / 2,
            modules_rect.y + modules_rect.height / 2,
        );
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONDOWN, WPARAM(0), pt).is_some());
        assert_eq!(
            runtime
                .settings_window
                .borrow()
                .as_ref()
                .and_then(|w| w.pressed),
            Some(SettingsHit::Page(SettingsPage::Modules))
        );
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONUP, WPARAM(0), pt).is_some());
        assert_eq!(
            runtime.settings_window.borrow().as_ref().map(|w| w.page),
            Some(SettingsPage::Modules)
        );

        let pt2 = pack_point(
            appearance_rect.x + appearance_rect.width / 2,
            appearance_rect.y + appearance_rect.height / 2,
        );
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONDOWN, WPARAM(0), pt2).is_some());
        assert!(handle_message(&runtime, hwnd, WM_CAPTURECHANGED, WPARAM(0), LPARAM(0)).is_some());
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONUP, WPARAM(0), pt2).is_some());
        assert_eq!(
            runtime.settings_window.borrow().as_ref().map(|w| w.page),
            Some(SettingsPage::Modules)
        );
    }
}
