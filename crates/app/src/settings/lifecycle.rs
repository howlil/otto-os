use otto_core::{
    ContentLayout, FocusSettingsTab, FocusTextField, SettingsContent, SettingsPage, UpdatesPort,
};
use otto_platform::{
    WindowsUpdatesPort, apply_settings_chrome, create_settings_window, destroy_settings_window,
    hide_settings_window, invalidate_settings_window, is_settings_window_foreground,
    is_settings_window_maximized, is_settings_window_visible, kill_window_timer,
    present_settings_window, read_launch_at_login, settings_glass_available,
};

use crate::host::RuntimeContext;
use crate::win_msg::GREEN_HOVER_TIMER;

use super::actions::{arm_update_timer, disarm_audit_timer, sync_audit_timer};
use super::{CatalogLoadState, SettingsWindow, refresh_model, with_window};

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

pub(crate) fn open_focus_activity(runtime: &RuntimeContext) {
    open_with_focus_tab(
        runtime,
        Some(SettingsPage::Focus),
        Some(FocusSettingsTab::Insights),
    );
}

pub(crate) fn open_focus_modes(runtime: &RuntimeContext) {
    open_with_focus_tab(
        runtime,
        Some(SettingsPage::Focus),
        Some(FocusSettingsTab::Modes),
    );
}

pub(crate) fn open(runtime: &RuntimeContext, page: Option<SettingsPage>) {
    open_with_focus_tab(runtime, page, None);
}

fn open_with_focus_tab(
    runtime: &RuntimeContext,
    page: Option<SettingsPage>,
    focus_tab: Option<FocusSettingsTab>,
) {
    let _ = otto_platform::close_tray_overflow();
    crate::menu::close(runtime);
    crate::control_center::close(runtime);
    let Some(hwnd) = ensure_window(runtime) else {
        otto_platform::signal_command_failure();
        return;
    };
    let update_state = WindowsUpdatesPort.poll_update_state();
    let busy_update = update_state.is_busy();
    with_window(runtime, |window| {
        window.launch_at_login = read_launch_at_login();
        window.update_state = update_state;
        if let Some(page) = page {
            window.select_page(page);
        }
        if let Some(tab) = focus_tab {
            window.focus_state.select_tab(tab);
        }
        window.relayout();
    });
    if busy_update {
        arm_update_timer(hwnd);
    }
    refresh_model(runtime);
    if let Some(active_page) = with_window(runtime, |window| window.page) {
        sync_audit_timer(hwnd, active_page);
    }
    let app = crate::theme_state::appearance(runtime);
    let glass = apply_settings_chrome(
        hwnd,
        app.content_theme().is_dark(),
        app.uses_backdrop() && settings_glass_available(),
    );
    with_window(runtime, |window| {
        window.glass = glass;
        window.active = true;
        window.maximized = is_settings_window_maximized(hwnd);
        window.alt_down = false;
        window.renderer = None;
        window.icons_attempted.clear();
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
    let closed_hwnd = with_window(runtime, |window| {
        if window.green_hover_armed {
            window.green_hover_armed = false;
            kill_window_timer(window.hwnd, GREEN_HOVER_TIMER);
        }
        hide_settings_window(window.hwnd);
        window.renderer = None;
        window.content = SettingsContent::default();
        window.blocks = ContentLayout::default();
        window.hovered = None;
        window.pressed = None;
        window.alt_down = false;
        window.tracking_leave = false;
        window.icons_attempted.clear();
        window.focus_state.focused_field = FocusTextField::None;
        window.focus_state.website_error = None;
        window.focus_state.catalog.clear();
        window.focus_catalog = CatalogLoadState::Unloaded;
        window.audit_loaded = CatalogLoadState::Unloaded;
        window.general_state.status_note = None;
        window.general_state.audit.groups.clear();
        window.pending_high_surrogate = None;
        window.hwnd
    });
    if let Some(hwnd) = closed_hwnd {
        disarm_audit_timer(hwnd);
        otto_platform::trim_working_set();
    }
}

pub(crate) fn refresh_appearance(runtime: &RuntimeContext) {
    let Some(hwnd) = with_window(runtime, |window| {
        is_settings_window_visible(window.hwnd).then_some(window.hwnd)
    })
    .flatten() else {
        return;
    };
    refresh_model(runtime);
    let app = crate::theme_state::appearance(runtime);
    let glass = apply_settings_chrome(
        hwnd,
        app.content_theme().is_dark(),
        app.uses_backdrop() && settings_glass_available(),
    );
    with_window(runtime, |window| {
        window.glass = glass;
        invalidate_settings_window(window.hwnd);
    });
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

pub(crate) fn refresh_process_audit_if_general(runtime: &RuntimeContext) {
    with_window(runtime, |window| {
        if window.page == SettingsPage::General && is_settings_window_visible(window.hwnd) {
            window.refresh_process_audit();
        }
    });
}

pub(crate) fn repaint_if_focus_page(runtime: &RuntimeContext) {
    let on_focus = with_window(runtime, |window| {
        window.page == SettingsPage::Focus && is_settings_window_visible(window.hwnd)
    })
    .unwrap_or(false);
    if on_focus {
        repaint(runtime);
    }
}

pub(crate) fn take_hwnd(runtime: &RuntimeContext) -> isize {
    runtime
        .settings_window
        .try_borrow_mut()
        .ok()
        .and_then(|mut guard| guard.take())
        .map_or(0, |window| window.hwnd)
}
