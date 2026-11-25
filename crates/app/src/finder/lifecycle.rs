use otto_core::{
    CMD_EDIT_COPY, CMD_EDIT_CUT, CMD_EDIT_FIND, CMD_EDIT_PASTE, CMD_EDIT_SELECT_ALL, CMD_EDIT_UNDO,
    CMD_FILE_NEW, CMD_FILE_NEW_FOLDER, CMD_FILE_NEW_TAB, CMD_VIEW_AS_DETAILS, CMD_VIEW_AS_ICONS,
    CMD_VIEW_AS_LIST, CMD_WIN_CLOSE, CMD_WIN_MAXIMIZE, CMD_WIN_MINIMIZE, FinderClipboardMode,
    FinderFlags, FinderLocation, FinderViewMode, copy_selected, create_new_folder,
    execute_finder_menu_command, parent_path_of, paste_clipboard,
};
use otto_platform::{
    WindowsFinderPort, apply_settings_chrome, create_finder_window, destroy_settings_window,
    enable_drag_accept_files, hide_settings_window, invalidate_settings_window,
    is_settings_window_foreground, is_settings_window_maximized, is_settings_window_visible,
    kill_window_timer, minimize_settings_window, present_settings_window, settings_glass_available,
    toggle_settings_window_maximize,
};

use crate::host::RuntimeContext;
use crate::win_msg::{FINDER_SPRING_TIMER, GREEN_HOVER_TIMER};

use super::{
    FinderWindow, UI_ACTIVE, UI_ALT_DOWN, UI_GLASS, UI_GREEN_HOVER_ARMED, UI_MAXIMIZED,
    UI_TRACKING_LEAVE, with_window,
};

fn ensure_window(runtime: &RuntimeContext) -> Option<isize> {
    if let Some(hwnd) = with_window(runtime, |window| window.hwnd) {
        return Some(hwnd);
    }
    runtime.creating_finder.set(true);
    let created = create_finder_window(crate::host::SETTINGS_WND_PROC);
    runtime.creating_finder.set(false);
    let hwnd = created.ok()?;
    enable_drag_accept_files(hwnd);
    let state = FinderWindow::new(hwnd);
    if let Ok(mut guard) = runtime.finder_window.try_borrow_mut() {
        *guard = Some(state);
        Some(hwnd)
    } else {
        destroy_settings_window(hwnd);
        None
    }
}

pub(crate) fn owns_hwnd(runtime: &RuntimeContext, hwnd: isize) -> bool {
    runtime
        .finder_window
        .try_borrow()
        .ok()
        .and_then(|guard| guard.as_ref().map(|w| w.hwnd == hwnd))
        .unwrap_or(false)
}

fn is_foreground(runtime: &RuntimeContext) -> bool {
    runtime
        .finder_window
        .try_borrow()
        .ok()
        .and_then(|guard| {
            guard.as_ref().map(|w| {
                is_settings_window_visible(w.hwnd) && is_settings_window_foreground(w.hwnd)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn open(runtime: &RuntimeContext, location: Option<FinderLocation>) {
    let _ = otto_platform::close_tray_overflow();
    crate::menu::close(runtime);
    crate::control_center::close(runtime);
    let Some(hwnd) = ensure_window(runtime) else {
        otto_platform::signal_command_failure();
        return;
    };
    let app = crate::theme_state::appearance(runtime);
    let glass = apply_settings_chrome(
        hwnd,
        app.content_theme().is_dark(),
        app.uses_backdrop() && settings_glass_available(),
    );
    let port = WindowsFinderPort;
    with_window(runtime, |window| {
        window.state.sidebar_items = otto_core::FinderPort::sidebar_items(&port);
        if let Some(loc) = location {
            window.navigate_to(loc);
        } else {
            window.state.reload_active_tab(&port);
            window.relayout();
            window.sync_watcher();
        }
        window.set_flag(UI_GLASS, glass);
        window.set_flag(UI_ACTIVE, true);
        window.set_flag(UI_MAXIMIZED, is_settings_window_maximized(hwnd));
        window.set_flag(UI_ALT_DOWN, false);
        window.renderer = None;
        window.thumbs_attempted.clear();
    });
    present_settings_window(hwnd);
    invalidate_settings_window(hwnd);
}

pub(crate) fn open_path(runtime: &RuntimeContext, path: &str) {
    open(runtime, Some(FinderLocation::Path(String::from(path))));
}

pub(crate) fn open_trash(runtime: &RuntimeContext) {
    open(runtime, Some(FinderLocation::Trash));
}

pub(crate) fn reveal_path(runtime: &RuntimeContext, path: &str) {
    let target_dir = if std::path::Path::new(path).is_dir() {
        String::from(path)
    } else {
        parent_path_of(path).unwrap_or_else(|| String::from(path))
    };
    open(runtime, Some(FinderLocation::Path(target_dir)));
    let port = WindowsFinderPort;
    with_window(runtime, |window| {
        window.state.select_path(&port, path, false, false);
        window.relayout();
        invalidate_settings_window(window.hwnd);
    });
}

pub(crate) fn close(runtime: &RuntimeContext) {
    let closed_hwnd = with_window(runtime, |window| {
        if window.flag(UI_GREEN_HOVER_ARMED) {
            window.set_flag(UI_GREEN_HOVER_ARMED, false);
            kill_window_timer(window.hwnd, GREEN_HOVER_TIMER);
        }
        kill_window_timer(window.hwnd, FINDER_SPRING_TIMER);
        hide_settings_window(window.hwnd);
        window.watcher = None;
        window.renderer = None;
        window.hovered = None;
        window.pressed = None;
        window.marquee_start = None;
        window.marquee_rect = None;
        window.drag_paths = None;
        window.spring_target = None;
        window.thumbs_attempted.clear();
        window.set_flag(UI_ALT_DOWN, false);
        window.set_flag(UI_TRACKING_LEAVE, false);
        window.hwnd
    });
    if closed_hwnd.is_some() {
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
    let app = crate::theme_state::appearance(runtime);
    let glass = apply_settings_chrome(
        hwnd,
        app.content_theme().is_dark(),
        app.uses_backdrop() && settings_glass_available(),
    );
    with_window(runtime, |window| {
        window.set_flag(UI_GLASS, glass);
        invalidate_settings_window(window.hwnd);
    });
}

pub(crate) fn take_hwnd(runtime: &RuntimeContext) -> Option<isize> {
    let Ok(mut guard) = runtime.finder_window.try_borrow_mut() else {
        return None;
    };
    guard.take().map(|mut window| {
        window.watcher = None;
        window.renderer = None;
        window.hwnd
    })
}

pub(crate) fn execute_menu_command(runtime: &RuntimeContext, cmd: usize) {
    let port = WindowsFinderPort;
    with_window(runtime, |window| {
        execute_finder_menu_command(&mut window.state, &port, cmd);
        window.relayout();
        window.sync_watcher();
        invalidate_settings_window(window.hwnd);
    });
}

pub(crate) fn handle_top_bar_command(runtime: &RuntimeContext, cmd: usize) -> bool {
    if !is_foreground(runtime) {
        return false;
    }
    let port = WindowsFinderPort;
    match cmd {
        CMD_WIN_CLOSE => {
            let closed_tab = with_window(runtime, |window| {
                let closed = window.state.close_tab(&port, window.state.active_tab);
                if closed {
                    window.relayout();
                    window.sync_watcher();
                    invalidate_settings_window(window.hwnd);
                }
                closed
            })
            .unwrap_or(false);
            if !closed_tab {
                close(runtime);
            }
            true
        }
        CMD_WIN_MINIMIZE => {
            if let Some(hwnd) = with_window(runtime, |w| w.hwnd)
                && !crate::genie::minimize_window(runtime, hwnd)
            {
                minimize_settings_window(hwnd);
            }
            true
        }
        CMD_WIN_MAXIMIZE => {
            if let Some(hwnd) = with_window(runtime, |w| w.hwnd) {
                toggle_settings_window_maximize(hwnd);
            }
            true
        }
        CMD_FILE_NEW_FOLDER | CMD_FILE_NEW | CMD_FILE_NEW_TAB | CMD_EDIT_UNDO | CMD_EDIT_CUT
        | CMD_EDIT_COPY | CMD_EDIT_PASTE | CMD_EDIT_SELECT_ALL | CMD_EDIT_FIND
        | CMD_VIEW_AS_ICONS | CMD_VIEW_AS_LIST | CMD_VIEW_AS_DETAILS => {
            with_window(runtime, |window| {
                match cmd {
                    CMD_FILE_NEW_FOLDER => create_new_folder(&mut window.state, &port),
                    CMD_FILE_NEW | CMD_FILE_NEW_TAB => {
                        let loc = window.state.tab().location.clone();
                        window.state.open_tab(&port, loc);
                    }
                    CMD_EDIT_UNDO => {
                        let _ = window.state.undo(&port);
                    }
                    CMD_EDIT_CUT => {
                        copy_selected(&mut window.state, &port, FinderClipboardMode::Cut);
                    }
                    CMD_EDIT_COPY => {
                        copy_selected(&mut window.state, &port, FinderClipboardMode::Copy);
                    }
                    CMD_EDIT_PASTE => paste_clipboard(&mut window.state, &port, false),
                    CMD_EDIT_SELECT_ALL => window.state.select_all(&port),
                    CMD_EDIT_FIND => window.state.flags.set(FinderFlags::SEARCH_FOCUSED, true),
                    CMD_VIEW_AS_ICONS => window.state.set_view_mode(&port, FinderViewMode::Icon),
                    CMD_VIEW_AS_LIST => window.state.set_view_mode(&port, FinderViewMode::List),
                    CMD_VIEW_AS_DETAILS => {
                        window.state.set_view_mode(&port, FinderViewMode::Column);
                    }
                    _ => {}
                }
                window.relayout();
                window.sync_watcher();
                invalidate_settings_window(window.hwnd);
            });
            true
        }
        _ => false,
    }
}
