use otto_core::SettingsPage;
use otto_platform::{
    apply_settings_chrome, create_settings_window, destroy_settings_window, hide_settings_window,
    invalidate_settings_window, is_settings_window_foreground, is_settings_window_visible,
    present_settings_window, read_launch_at_login, settings_glass_available,
};

use crate::host::RuntimeContext;

use super::{SettingsWindow, refresh_model, with_window};

fn ensure_window(runtime: &RuntimeContext) -> Option<isize> {
    if let Some(hwnd) = with_window(runtime, |window| window.hwnd) {
        return Some(hwnd);
    }
    let hwnd = create_settings_window(crate::host::SETTINGS_WND_PROC).ok()?;
    let mut state = SettingsWindow::new(hwnd);
    state.launch_at_login = read_launch_at_login();
    if let Ok(mut guard) = runtime.settings_window.try_borrow_mut() {
        *guard = Some(state);
        Some(hwnd)
    } else {
        destroy_settings_window(hwnd);
        None
    }
}

pub(crate) fn open(runtime: &RuntimeContext, page: Option<SettingsPage>) {
    let _ = otto_platform::close_tray_overflow();
    crate::menu::close(runtime);
    crate::control_center::close(runtime);
    let Some(hwnd) = ensure_window(runtime) else {
        otto_platform::signal_command_failure();
        return;
    };
    with_window(runtime, |window| {
        window.launch_at_login = read_launch_at_login();
        if let Some(page) = page {
            window.select_page(page);
        }
        window.relayout();
    });
    refresh_model(runtime);
    let glass = settings_glass_available();
    apply_settings_chrome(hwnd, otto_platform::is_system_in_dark_mode(), glass);
    with_window(runtime, |window| {
        window.glass = glass;
        window.renderer = None;
    });
    present_settings_window(hwnd);
    invalidate_settings_window(hwnd);
}

pub(crate) fn toggle(runtime: &RuntimeContext) {
    let visible_and_active = with_window(runtime, |window| {
        is_settings_window_visible(window.hwnd) && is_settings_window_foreground(window.hwnd)
    })
    .unwrap_or(false);
    if visible_and_active {
        close(runtime);
    } else {
        open(runtime, None);
    }
}

pub(super) fn close(runtime: &RuntimeContext) {
    with_window(runtime, |window| hide_settings_window(window.hwnd));
}

pub(crate) fn repaint(runtime: &RuntimeContext) {
    let visible =
        with_window(runtime, |window| is_settings_window_visible(window.hwnd)).unwrap_or(false);
    if !visible {
        return;
    }
    refresh_model(runtime);
    with_window(runtime, |window| invalidate_settings_window(window.hwnd));
}

pub(crate) fn take_hwnd(runtime: &RuntimeContext) -> isize {
    runtime
        .settings_window
        .try_borrow_mut()
        .ok()
        .and_then(|mut guard| guard.take())
        .map_or(0, |window| window.hwnd)
}
