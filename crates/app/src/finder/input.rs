use std::time::{SystemTime, UNIX_EPOCH};

use otto_core::{
    FinderFlags, FinderKey, FinderKeyMods, FinderKeyOutcome, FinderPort, FinderViewMode,
    handle_finder_key, scale,
};
use otto_platform::{
    WM_APP_FINDER_FS_CHANGE, WindowsFinderPort, apply_finder_min_track, client_to_screen,
    current_process_id, extract_dropped_files, handle_settings_ncactivate,
    handle_settings_nccalcsize, handle_settings_nchittest, invalidate_settings_window,
    is_settings_window_maximized, is_virtual_key_down, kill_window_timer, place_no_zorder,
    settings_client_size, suggested_dpi_rect, track_mouse_leave,
};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F2, VK_F4, VK_HOME, VK_LEFT, VK_OEM_4,
    VK_OEM_6, VK_OEM_PERIOD, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_ACTIVATE, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DPICHANGED,
    WM_DWMCOLORIZATIONCOLORCHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST,
    WM_PAINT, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCOLORCHANGE, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WM_THEMECHANGED, WM_TIMER, WM_XBUTTONDOWN,
};

use crate::host::RuntimeContext;
use crate::win_msg::{
    FINDER_SPRING_TIMER, GREEN_HOVER_TIMER, WM_DROPFILES, WM_MOUSELEAVE, WM_UNICHAR,
};
use crate::window_controls::{is_alt_pressed, open_tiling_menu};

use super::lifecycle::{close, refresh_appearance};
use super::pointer::{on_left_down, on_left_up, on_mouse_move, on_right_up, on_spring_timer};
use super::{
    UI_ACTIVE, UI_ALT_DOWN, UI_DRAGGING_SIDEBAR, UI_DRAGGING_SLIDER, UI_GREEN_HOVER_ARMED,
    UI_MAXIMIZED, UI_TRACKING_LEAVE, paint, with_window,
};

const UNICODE_NOCHAR: usize = 0xFFFF;
const XBUTTON1_MASK: usize = 0x0001;
const XBUTTON2_MASK: usize = 0x0002;

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn current_mods() -> FinderKeyMods {
    FinderKeyMods::new(
        is_virtual_key_down(0x11),
        is_virtual_key_down(0x10),
        is_alt_pressed(),
    )
}

fn map_virtual_key(vk: u16, mods: FinderKeyMods) -> Option<FinderKey> {
    match vk {
        v if v == VK_RETURN.0 => Some(FinderKey::Enter),
        v if v == VK_ESCAPE.0 => Some(FinderKey::Escape),
        v if v == VK_SPACE.0 => Some(FinderKey::Space),
        v if v == VK_BACK.0 => Some(FinderKey::Backspace),
        v if v == VK_DELETE.0 => Some(FinderKey::Delete),
        v if v == VK_TAB.0 => Some(FinderKey::Tab),
        v if v == VK_F2.0 => Some(FinderKey::F2),
        v if v == VK_UP.0 => Some(FinderKey::Up),
        v if v == VK_DOWN.0 => Some(FinderKey::Down),
        v if v == VK_LEFT.0 => Some(FinderKey::Left),
        v if v == VK_RIGHT.0 => Some(FinderKey::Right),
        v if v == VK_HOME.0 => Some(FinderKey::Home),
        v if v == VK_END.0 => Some(FinderKey::End),
        v if v == VK_OEM_4.0 && mods.ctrl() => Some(FinderKey::Char('[')),
        v if v == VK_OEM_6.0 && mods.ctrl() => Some(FinderKey::Char(']')),
        v if v == VK_OEM_PERIOD.0 && mods.ctrl() => Some(FinderKey::Char('.')),
        0x31..=0x34 if mods.ctrl() => Some(FinderKey::Char(char::from(vk as u8))),
        0x41..=0x5A if mods.ctrl() => {
            Some(FinderKey::Char(char::from((vk as u8).to_ascii_lowercase())))
        }
        _ => None,
    }
}

fn dispatch_key(runtime: &RuntimeContext, key: FinderKey, mods: FinderKeyMods) -> bool {
    let port = WindowsFinderPort;
    let outcome = with_window(runtime, |window| {
        let out = handle_finder_key(&mut window.state, &port, key, mods, now_millis());
        if out == FinderKeyOutcome::Handled {
            window.relayout();
            window.sync_watcher();
            invalidate_settings_window(window.hwnd);
        }
        out
    })
    .unwrap_or(FinderKeyOutcome::Ignored);
    if outcome == FinderKeyOutcome::CloseWindow {
        close(runtime);
        return true;
    }
    outcome == FinderKeyOutcome::Handled
}

fn handle_char_unit(runtime: &RuntimeContext, unit: u16) {
    let ch = with_window(runtime, |window| {
        if (0xD800..=0xDBFF).contains(&unit) {
            window.pending_high_surrogate = Some(unit);
            return None;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            if let Some(high) = window.pending_high_surrogate.take() {
                let h = u32::from(high - 0xD800);
                let l = u32::from(unit - 0xDC00);
                return char::from_u32(0x1_0000 + (h << 10) + l);
            }
            return None;
        }
        window.pending_high_surrogate = None;
        char::from_u32(u32::from(unit))
    })
    .flatten();
    let Some(ch) = ch else {
        return;
    };
    if ch.is_control() || is_virtual_key_down(0x11) {
        return;
    }
    let text_input_active = with_window(runtime, |window| {
        window.state.rename.is_some()
            || window.state.flags.has(FinderFlags::SEARCH_FOCUSED)
            || window.state.flags.has(FinderFlags::GO_TO_FOLDER)
    })
    .unwrap_or(false);
    if text_input_active || ch.is_alphanumeric() {
        let _ = dispatch_key(runtime, FinderKey::Char(ch), FinderKeyMods::default());
    }
}

fn handle_mouse_wheel(runtime: &RuntimeContext, wparam: WPARAM, lparam: LPARAM) {
    let delta = i32::from(((wparam.0 >> 16) & 0xFFFF) as i16);
    let shift = is_virtual_key_down(0x10);
    let _ = lparam;
    with_window(runtime, |window| {
        let step = scale(48, window.dpi) * delta / 120;
        let view = window.state.tab().view_mode;
        if shift || matches!(view, FinderViewMode::Column | FinderViewMode::Gallery) {
            let max_x =
                (window.layout.content_scroll_width - window.layout.content_stage.width).max(0);
            let next = (window.state.tab().scroll_x - step).clamp(0, max_x);
            window.state.tab_mut().scroll_x = next;
        } else {
            let max_y =
                (window.layout.content_scroll_height - window.layout.content_stage.height).max(0);
            let next = (window.state.tab().scroll_y - step).clamp(0, max_y);
            window.state.tab_mut().scroll_y = next;
        }
        window.relayout();
        invalidate_settings_window(window.hwnd);
    });
}

fn handle_drop_files(runtime: &RuntimeContext, wparam: WPARAM) {
    let (_, files) = extract_dropped_files(wparam.0);
    if files.is_empty() {
        return;
    }
    let port = WindowsFinderPort;
    let move_mode = is_virtual_key_down(0x10);
    with_window(runtime, |window| {
        if let Some(dest) = window.state.active_dir_path() {
            let done = port.paste_paths(&files, &dest, move_mode);
            if !done.is_empty() {
                window.state.reload_active_tab(&port);
                window.relayout();
                invalidate_settings_window(window.hwnd);
            }
        }
    });
}

fn handle_mouse_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEMOVE => {
            let should_track = with_window(runtime, |window| {
                if window.flag(UI_TRACKING_LEAVE) {
                    false
                } else {
                    window.set_flag(UI_TRACKING_LEAVE, true);
                    true
                }
            })
            .unwrap_or(false);
            if should_track {
                let _ = track_mouse_leave(hwnd);
            }
            let (x, y) = unpack_point(lparam);
            on_mouse_move(runtime, x, y);
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_window(runtime, |window| {
                window.set_flag(UI_TRACKING_LEAVE, false);
                window.hovered = None;
                if window.flag(UI_GREEN_HOVER_ARMED) {
                    window.set_flag(UI_GREEN_HOVER_ARMED, false);
                    kill_window_timer(window.hwnd, GREEN_HOVER_TIMER);
                }
                invalidate_settings_window(window.hwnd);
            });
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            on_left_down(runtime, x, y);
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            on_left_up(runtime, x, y);
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            on_right_up(runtime, x, y);
            Some(LRESULT(0))
        }
        WM_XBUTTONDOWN => {
            let xbtn = (wparam.0 >> 16) & 0xFFFF;
            let port = WindowsFinderPort;
            with_window(runtime, |window| {
                let moved = if (xbtn & XBUTTON1_MASK) != 0 {
                    window.state.go_back(&port)
                } else if (xbtn & XBUTTON2_MASK) != 0 {
                    window.state.go_forward(&port)
                } else {
                    false
                };
                if moved {
                    window.relayout();
                    window.sync_watcher();
                    invalidate_settings_window(window.hwnd);
                }
            });
            Some(LRESULT(1))
        }
        WM_MOUSEWHEEL => {
            handle_mouse_wheel(runtime, wparam, lparam);
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_window(runtime, |window| {
                window.pressed = None;
                window.marquee_start = None;
                window.marquee_rect = None;
                window.drag_paths = None;
                window.set_flag(UI_DRAGGING_SIDEBAR, false);
                window.set_flag(UI_DRAGGING_SLIDER, false);
                invalidate_settings_window(window.hwnd);
            });
            Some(LRESULT(0))
        }
        _ => None,
    }
}

fn handle_keyboard_message(
    runtime: &RuntimeContext,
    message: u32,
    wparam: WPARAM,
) -> Option<LRESULT> {
    match message {
        WM_KEYDOWN => {
            let mods = current_mods();
            let handled = map_virtual_key((wparam.0 & 0xFFFF) as u16, mods)
                .is_some_and(|key| dispatch_key(runtime, key, mods));
            handled.then_some(LRESULT(0))
        }
        WM_SYSKEYDOWN | WM_SYSKEYUP => {
            let alt = is_alt_pressed();
            with_window(runtime, |window| {
                if window.flag(UI_ALT_DOWN) != alt {
                    window.set_flag(UI_ALT_DOWN, alt);
                    invalidate_settings_window(window.hwnd);
                }
            });
            if message == WM_SYSKEYDOWN {
                let vk = (wparam.0 & 0xFFFF) as u16;
                if vk == VK_F4.0 && alt {
                    close(runtime);
                    return Some(LRESULT(0));
                }
                let mods = current_mods();
                if let Some(key) = map_virtual_key(vk, mods)
                    && dispatch_key(runtime, key, mods)
                {
                    return Some(LRESULT(0));
                }
            }
            None
        }
        WM_CHAR => {
            handle_char_unit(runtime, (wparam.0 & 0xFFFF) as u16);
            Some(LRESULT(0))
        }
        WM_UNICHAR => {
            if wparam.0 == UNICODE_NOCHAR {
                return Some(LRESULT(1));
            }
            if let Some(ch) = char::from_u32(wparam.0 as u32)
                && !ch.is_control()
            {
                let _ = dispatch_key(runtime, FinderKey::Char(ch), FinderKeyMods::default());
            }
            Some(LRESULT(0))
        }
        _ => None,
    }
}

fn handle_timer_message(runtime: &RuntimeContext, wparam: WPARAM) -> Option<LRESULT> {
    if wparam.0 == GREEN_HOVER_TIMER {
        let anchor = with_window(runtime, |window| {
            window.set_flag(UI_GREEN_HOVER_ARMED, false);
            kill_window_timer(window.hwnd, GREEN_HOVER_TIMER);
            window
                .layout
                .window_controls
                .get(2)
                .map(|slot| (window.hwnd, slot.rect.x, slot.rect.bottom()))
        })
        .flatten();
        if let Some((h, x, y)) = anchor {
            let (sx, sy) = client_to_screen(h, x, y);
            open_tiling_menu(runtime, h, current_process_id(), sx, sy);
        }
        return Some(LRESULT(0));
    }
    if wparam.0 == FINDER_SPRING_TIMER {
        on_spring_timer(runtime);
        return Some(LRESULT(0));
    }
    None
}

pub(crate) fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if let Some(res) = handle_mouse_message(runtime, hwnd, message, wparam, lparam) {
        return Some(res);
    }
    if let Some(res) = handle_keyboard_message(runtime, message, wparam) {
        return Some(res);
    }
    match message {
        WM_NCCALCSIZE => handle_settings_nccalcsize(hwnd, wparam.0, lparam.0).map(LRESULT),
        WM_NCACTIVATE => handle_settings_ncactivate(hwnd).map(LRESULT),
        WM_NCHITTEST => handle_settings_nchittest(hwnd, lparam.0, |x, y| {
            with_window(runtime, |window| window.layout.is_caption_drag_point(x, y))
                .unwrap_or(false)
        })
        .map(LRESULT),
        WM_GETMINMAXINFO => {
            apply_finder_min_track(hwnd, lparam);
            Some(LRESULT(0))
        }
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_PAINT => {
            paint(runtime);
            Some(LRESULT(0))
        }
        WM_SIZE => {
            let (w, h) = settings_client_size(hwnd);
            with_window(runtime, |window| {
                window.set_flag(UI_MAXIMIZED, is_settings_window_maximized(hwnd));
                if let Some(renderer) = window.renderer.as_ref()
                    && renderer.resize(w, h).is_err()
                {
                    window.renderer = None;
                }
                window.relayout();
                invalidate_settings_window(window.hwnd);
            });
            Some(LRESULT(0))
        }
        WM_DPICHANGED => {
            if let Some(rect) = suggested_dpi_rect(lparam) {
                place_no_zorder(hwnd, rect);
            }
            let dpi = ((wparam.0 & 0xFFFF) as u32).max(96);
            with_window(runtime, |window| {
                if let Some(renderer) = window.renderer.as_mut()
                    && renderer.set_dpi(dpi).is_err()
                {
                    window.renderer = None;
                }
                window.relayout();
                invalidate_settings_window(window.hwnd);
            });
            Some(LRESULT(0))
        }
        WM_ACTIVATE => {
            let active = (wparam.0 & 0xFFFF) != 0;
            with_window(runtime, |window| {
                window.set_flag(UI_ACTIVE, active);
                if !active {
                    window.set_flag(UI_ALT_DOWN, false);
                    if window.flag(UI_GREEN_HOVER_ARMED) {
                        window.set_flag(UI_GREEN_HOVER_ARMED, false);
                        kill_window_timer(window.hwnd, GREEN_HOVER_TIMER);
                    }
                }
                invalidate_settings_window(window.hwnd);
            });
            None
        }
        WM_THEMECHANGED | WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
            refresh_appearance(runtime);
            None
        }
        WM_DROPFILES => {
            handle_drop_files(runtime, wparam);
            Some(LRESULT(0))
        }
        WM_TIMER => handle_timer_message(runtime, wparam),
        WM_APP_FINDER_FS_CHANGE => {
            let port = WindowsFinderPort;
            with_window(runtime, |window| {
                window.state.reload_active_tab(&port);
                window.relayout();
                invalidate_settings_window(window.hwnd);
            });
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
    use otto_core::{
        FinderLayoutInput, FinderLocation, FinderToolbarButton, FinderViewMode,
        compute_finder_layout,
    };
    use otto_platform::{create_finder_window, destroy_settings_window};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_ACTIVATE, WM_CAPTURECHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP,
    };

    use super::handle_message;
    use crate::finder::{FinderWindow, UI_ACTIVE};
    use crate::host::{RuntimeContext, SETTINGS_WND_PROC};

    struct WindowGuard(isize);

    impl Drop for WindowGuard {
        fn drop(&mut self) {
            destroy_settings_window(self.0);
        }
    }

    const fn pack_point(x: i32, y: i32) -> LPARAM {
        LPARAM(((y as u16 as isize) << 16) | (x as u16 as isize))
    }

    fn set_fixed_layout(window: &mut FinderWindow) {
        window.dpi = 96;
        window.list_rows = window.state.visible_list_rows();
        window.layout = compute_finder_layout(&FinderLayoutInput {
            width: 1080,
            height: 700,
            dpi: 96,
            metrics: window.state.metrics,
            flags: window.state.flags,
            tabs: &window.state.tabs,
            active_tab: window.state.active_tab,
            sidebar_items: &window.state.sidebar_items,
            list_rows: &window.list_rows,
            visible_entries_count: window.state.tab().entries.len(),
            active_columns: &window.state.tab().miller_columns,
        });
    }

    fn rect_center(rect: otto_core::Rect) -> LPARAM {
        pack_point(rect.x + rect.width / 2, rect.y + rect.height / 2)
    }

    fn setup_finder_fixture() -> (WindowGuard, isize, RuntimeContext) {
        let hwnd = create_finder_window(SETTINGS_WND_PROC).expect("hidden finder window");
        let runtime = RuntimeContext::new(false);
        let mut window = FinderWindow::new(hwnd);
        window.watcher = None;
        set_fixed_layout(&mut window);
        *runtime.finder_window.borrow_mut() = Some(window);
        (WindowGuard(hwnd), hwnd, runtime)
    }

    fn clear_watcher(runtime: &RuntimeContext) {
        if let Some(w) = runtime.finder_window.borrow_mut().as_mut() {
            w.watcher = None;
        }
    }

    #[test]
    fn sidebar_and_toolbar_clicks_navigate_and_switch_views() {
        let (_guard, hwnd, runtime) = setup_finder_fixture();
        let (initial_location, this_pc_rect, icon_rect, back_rect) = {
            let guard = runtime.finder_window.borrow();
            let w = guard.as_ref().expect("finder window");
            let this_pc = w
                .layout
                .sidebar_slots
                .iter()
                .find(|slot| {
                    w.state
                        .sidebar_items
                        .get(slot.index)
                        .is_some_and(|item| item.location == FinderLocation::ThisPc)
                })
                .expect("This PC sidebar slot")
                .rect;
            let icon = w
                .layout
                .toolbar_buttons
                .iter()
                .find(|slot| slot.button == FinderToolbarButton::ViewMode(FinderViewMode::Icon))
                .expect("Icon view button")
                .rect;
            let back = w
                .layout
                .toolbar_buttons
                .iter()
                .find(|slot| slot.button == FinderToolbarButton::Back)
                .expect("Back button")
                .rect;
            (w.state.tab().location.clone(), this_pc, icon, back)
        };

        let pt = rect_center(this_pc_rect);
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONDOWN, WPARAM(0), pt).is_some());
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONUP, WPARAM(0), pt).is_some());
        assert_eq!(
            runtime
                .finder_window
                .borrow()
                .as_ref()
                .map(|w| w.state.tab().location.clone()),
            Some(FinderLocation::ThisPc)
        );

        if let Some(w) = runtime.finder_window.borrow_mut().as_mut() {
            set_fixed_layout(w);
        }
        let icon_pt = rect_center(icon_rect);
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONDOWN, WPARAM(0), icon_pt).is_some());
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONUP, WPARAM(0), icon_pt).is_some());
        assert_eq!(
            runtime
                .finder_window
                .borrow()
                .as_ref()
                .map(|w| w.state.tab().view_mode),
            Some(FinderViewMode::Icon)
        );

        if let Some(w) = runtime.finder_window.borrow_mut().as_mut() {
            set_fixed_layout(w);
        }
        let back_pt = rect_center(back_rect);
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONDOWN, WPARAM(0), back_pt).is_some());
        assert!(handle_message(&runtime, hwnd, WM_CAPTURECHANGED, WPARAM(0), LPARAM(0)).is_some());
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONUP, WPARAM(0), back_pt).is_some());
        assert_eq!(
            runtime
                .finder_window
                .borrow()
                .as_ref()
                .map(|w| w.state.tab().location.clone()),
            Some(FinderLocation::ThisPc)
        );

        assert!(handle_message(&runtime, hwnd, WM_LBUTTONDOWN, WPARAM(0), back_pt).is_some());
        assert!(handle_message(&runtime, hwnd, WM_LBUTTONUP, WPARAM(0), back_pt).is_some());
        assert_eq!(
            runtime
                .finder_window
                .borrow()
                .as_ref()
                .map(|w| w.state.tab().location.clone()),
            Some(initial_location)
        );
        clear_watcher(&runtime);
    }

    #[test]
    fn activate_defers_focus_to_default_window_proc() {
        let (_guard, hwnd, runtime) = setup_finder_fixture();
        assert!(handle_message(&runtime, hwnd, WM_ACTIVATE, WPARAM(1), LPARAM(0)).is_none());
        assert!(
            runtime
                .finder_window
                .borrow()
                .as_ref()
                .is_some_and(|w| w.flag(UI_ACTIVE))
        );
        clear_watcher(&runtime);
    }
}
