use std::collections::BTreeSet;

use crate::host::RuntimeContext;
use otto_core::DockEntry;
use otto_core::{
    AppMenuState, CMD_DOCK_OVERFLOW_BASE, DOWNLOADS_STACK_MAX_ITEMS, DockPreferences, HitTarget,
    MenuAnchor, MenuContext, MenuRow, Rect, WindowInfo, WindowMenuEntry, WindowMenuState,
    build_calendar_rows, build_menu_overflow_rows, build_top_bar_menu_rows, build_tray_menu_rows,
    build_window_tiling_menu_rows, dock_settings_rows,
};
use otto_platform::{
    ApplicationContext, application_of, build_dock_context_menu_rows, current_process_id,
    read_downloads_stack_items,
};

use super::MenuSource;
use super::TopBarMenuRequest;
use super::resolve_layout;
use super::with_menu;

pub(super) fn truncate_window_title(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let mut out = String::new();
    for ch in trimmed.chars().take(max_chars) {
        out.push(ch);
    }
    if trimmed.chars().count() > max_chars && max_chars > 1 {
        out.pop();
        out.push('…');
    }
    out
}

pub(super) fn top_bar_context<'a>(
    request: &TopBarMenuRequest<'a>,
    app_context: Option<&ApplicationContext>,
    app_windows: &'a [WindowMenuEntry],
) -> MenuContext<'a> {
    let Some(window) = request.window else {
        return MenuContext {
            file_manager: request.app_name == "Finder",
            ..MenuContext::without_window(request.app_name)
        };
    };
    let app = if request.target == HitTarget::AppMenu {
        app_context.map_or(AppMenuState::NONE, |context| {
            AppMenuState::of(&context.app, context.other_apps_visible)
        })
    } else {
        AppMenuState::NONE
    };
    MenuContext {
        app_name: request.app_name,
        file_manager: request.app_name == "Finder",
        tabbed: window.traits.full_screen_shortcut,
        window: Some(WindowMenuState::of(window)),
        app,
        app_windows,
    }
}

pub fn open_top_bar_menu(runtime: &RuntimeContext, request: &TopBarMenuRequest<'_>) {
    let anchor = MenuAnchor::TopBar {
        x: request.anchor_x,
        y: request.anchor_y,
    };
    if request.target == HitTarget::MenuOverflow {
        let rows = build_menu_overflow_rows(request.hidden, request.app_name);
        with_menu(runtime, |menu| {
            menu.open(runtime, anchor, rows, MenuSource::Overflow);
        });
        return;
    }
    let own_pid = current_process_id();
    let app_context = request
        .window
        .filter(|_| matches!(request.target, HitTarget::AppMenu | HitTarget::WindowMenu))
        .and_then(|window| application_of(window.identity, own_pid));
    let app_windows: Vec<WindowMenuEntry> = app_context
        .as_ref()
        .filter(|_| request.target == HitTarget::WindowMenu)
        .map(|context| {
            context
                .app
                .primary_windows()
                .take(64)
                .map(|win| {
                    let title = if win.title.trim().is_empty() {
                        win.app_name.clone()
                    } else {
                        truncate_window_title(&win.title, 36)
                    };
                    WindowMenuEntry {
                        title,
                        active: win.active,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let rows = build_top_bar_menu_rows(
        request.target,
        &top_bar_context(request, app_context.as_ref(), &app_windows),
    );
    let source = MenuSource::TopBar {
        bar_hwnd: request.bar_hwnd,
        target: request.target,
        window: request.window.cloned(),
    };
    with_menu(runtime, |menu| menu.open(runtime, anchor, rows, source));
}

pub fn open_dock_menu(
    runtime: &RuntimeContext,
    entry: DockEntry,
    other_apps_visible: bool,
    center_x: i32,
    bottom_y: i32,
    target_rect: Option<Rect>,
) {
    let rows = build_dock_context_menu_rows(&entry, other_apps_visible);
    let source = MenuSource::Dock { entry, target_rect };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            rows,
            source,
        );
    });
}

pub fn open_dock_settings_menu(
    runtime: &RuntimeContext,
    center_x: i32,
    bottom_y: i32,
    preferences: DockPreferences,
) {
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            dock_settings_rows(preferences),
            MenuSource::DockSettings,
        );
    });
}

pub fn open_tray_menu(runtime: &RuntimeContext, anchor_x: i32, anchor_y: i32, above_cursor: bool) {
    let rows = build_tray_menu_rows(
        runtime.top_bar_enabled.get(),
        runtime.notch_enabled.get(),
        runtime.dock_enabled.get(),
    );
    let anchor = if above_cursor {
        MenuAnchor::AboveDock {
            center_x: anchor_x,
            bottom_y: anchor_y,
        }
    } else {
        MenuAnchor::BelowPoint {
            x: anchor_x,
            y: anchor_y,
        }
    };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            anchor,
            rows,
            MenuSource::Tray {
                from_tray: above_cursor,
            },
        );
    });
}

pub fn open_dock_overflow_menu(
    runtime: &RuntimeContext,
    center_x: i32,
    bottom_y: i32,
    entries: Vec<DockEntry>,
) {
    let mut rows = vec![MenuRow::heading("More Apps"), MenuRow::Separator];
    for (index, entry) in entries.iter().enumerate() {
        let marker = if entry.running { "\u{2022} " } else { "" };
        rows.push(MenuRow::item(
            CMD_DOCK_OVERFLOW_BASE + index,
            format!("{marker}{}", entry.title),
            "",
        ));
    }
    let source = MenuSource::DockOverflow { entries };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            rows,
            source,
        );
    });
}

pub fn open_downloads_stack_menu(runtime: &RuntimeContext, center_x: i32, bottom_y: i32) {
    let items = read_downloads_stack_items(DOWNLOADS_STACK_MAX_ITEMS);
    with_menu(runtime, |menu| {
        menu.open_downloads_stack(runtime, center_x, bottom_y, items);
    });
}

pub fn open_calendar(runtime: &RuntimeContext, anchor_right: i32, anchor_y: i32) {
    let mut month = None;
    with_menu(runtime, |menu| {
        let clock = menu.control_center.read_system_status().clock;
        month = Some((clock.year, clock.month));
    });
    if let Some((year, month)) = month {
        open_calendar_month(runtime, year, month, anchor_right, anchor_y);
    }
}

pub(super) fn open_calendar_month(
    runtime: &RuntimeContext,
    year: u16,
    month: u8,
    anchor_right: i32,
    anchor_y: i32,
) {
    with_menu(runtime, |menu| {
        let clock = menu.control_center.read_system_status().clock;
        let today = (clock.year == year && clock.month == month).then_some(clock.day);
        let rows = build_calendar_rows(year, month, today);
        let probe = MenuAnchor::BelowPoint {
            x: anchor_right - 1,
            y: anchor_y,
        };
        let width = resolve_layout(
            &menu.metrics,
            menu.screen.dpi,
            probe,
            0,
            &rows,
            menu.renderer.as_ref(),
        )
        .window
        .width;
        menu.open(
            runtime,
            MenuAnchor::BelowPoint {
                x: anchor_right - width,
                y: anchor_y,
            },
            rows,
            MenuSource::Calendar {
                year,
                month,
                anchor_right,
                anchor_y,
            },
        );
    });
}

pub fn open_notifications(runtime: &RuntimeContext, anchor_right: i32, anchor_y: i32) {
    with_menu(runtime, |menu| {
        menu.open_notification_center(runtime, anchor_right, anchor_y, BTreeSet::new(), 0);
    });
}

#[must_use]
pub fn is_calendar_open(runtime: &RuntimeContext) -> bool {
    runtime.menu.try_borrow().ok().is_some_and(|guard| {
        guard.as_ref().is_some_and(|menu| {
            menu.visible && matches!(menu.source, Some(MenuSource::Calendar { .. }))
        })
    })
}

#[must_use]
pub fn is_notification_open(runtime: &RuntimeContext) -> bool {
    runtime.menu.try_borrow().ok().is_some_and(|guard| {
        guard
            .as_ref()
            .is_some_and(|menu| menu.visible && menu.notif_view.is_some())
    })
}

pub fn open_window_tiling_menu(
    runtime: &RuntimeContext,
    window: &WindowInfo,
    anchor_x: i32,
    anchor_y: i32,
) {
    let rows = build_window_tiling_menu_rows(&WindowMenuState::of(window));
    let source = MenuSource::WindowControls {
        window: window.clone(),
    };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::BelowPoint {
                x: anchor_x,
                y: anchor_y,
            },
            rows,
            source,
        );
    });
}
