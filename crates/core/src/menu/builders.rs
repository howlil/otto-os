use crate::layout::HitTarget;

use super::commands::CMD_APP_ACTIVATE;
use super::commands::CMD_APP_HIDE;
use super::commands::CMD_APP_HIDE_OTHERS;
use super::commands::CMD_APP_QUIT;
use super::commands::CMD_APP_SETTINGS;
use super::commands::CMD_APP_SHOW_ALL;
use super::commands::CMD_CAL_SETTINGS;
use super::commands::CMD_CC_CALENDAR;
use super::commands::CMD_CONTROL_CENTER;
use super::commands::CMD_DOCK_WINDOW_BASE;
use super::commands::CMD_EDIT_COPY;
use super::commands::CMD_EDIT_CUT;
use super::commands::CMD_EDIT_FIND;
use super::commands::CMD_EDIT_PASTE;
use super::commands::CMD_EDIT_REDO;
use super::commands::CMD_EDIT_SELECT_ALL;
use super::commands::CMD_EDIT_UNDO;
use super::commands::CMD_FILE_NEW;
use super::commands::CMD_FILE_NEW_FOLDER;
use super::commands::CMD_FILE_NEW_TAB;
use super::commands::CMD_FILE_OPEN;
use super::commands::CMD_FILE_SAVE;
use super::commands::CMD_FILE_SAVE_AS;
use super::commands::CMD_HELP_F1;
use super::commands::CMD_LOCK;
use super::commands::CMD_NOTIF_CLEAR_ALL;
use super::commands::CMD_NOTIF_DISMISS_BASE;
use super::commands::CMD_NOTIF_REQUEST_ACCESS;
use super::commands::CMD_OPEN_LOGS;
use super::commands::CMD_OTTO_SETTINGS;
use super::commands::CMD_QUIT;
use super::commands::CMD_RESTART;
use super::commands::CMD_SETTINGS;
use super::commands::CMD_SHOW_DESKTOP;
use super::commands::CMD_SHUTDOWN;
use super::commands::CMD_SLEEP;
use super::commands::CMD_START;
use super::commands::CMD_TASK_VIEW;
use super::commands::CMD_TASKMGR;
use super::commands::CMD_TILE_LEFT;
use super::commands::CMD_TILE_RIGHT;
use super::commands::CMD_TOGGLE_DOCK;
use super::commands::CMD_TOGGLE_NOTCH;
use super::commands::CMD_TOGGLE_TOP_BAR;
use super::commands::CMD_VIEW_ACTUAL_SIZE;
use super::commands::CMD_VIEW_AS_DETAILS;
use super::commands::CMD_VIEW_AS_ICONS;
use super::commands::CMD_VIEW_AS_LIST;
use super::commands::CMD_VIEW_FULLSCREEN;
use super::commands::CMD_VIEW_ZOOM_IN;
use super::commands::CMD_VIEW_ZOOM_OUT;
use super::commands::CMD_WIN_BRING_ALL_TO_FRONT;
use super::commands::CMD_WIN_CLOSE;
use super::commands::CMD_WIN_MAXIMIZE;
use super::commands::CMD_WIN_MINIMIZE;
use super::commands::MAX_VISIBLE_NOTIFICATIONS;
use super::notifications::NotificationAccess;
use super::notifications::NotificationCenterSnapshot;
use super::rows::MenuRow;
use super::rows::push_capable;
use super::rows::trim_separators;
use super::top_bar::MenuContext;
use super::top_bar::WindowMenuState;
use super::top_bar::top_bar_menu_title;

#[must_use]
pub fn build_apple_menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::item(CMD_START, "Open Start", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_OTTO_SETTINGS, "Otto Settings...", "⌃⌥,"),
        MenuRow::item(CMD_OPEN_LOGS, "Open Logs Folder", ""),
        MenuRow::item(CMD_SETTINGS, "System Settings...", ""),
        MenuRow::item(CMD_TASKMGR, "Force Quit (Task Manager)...", "⌥⌘⎋"),
        MenuRow::Separator,
        MenuRow::item(CMD_SLEEP, "Sleep", ""),
        MenuRow::item(CMD_RESTART, "Restart...", ""),
        MenuRow::item(CMD_SHUTDOWN, "Shut Down...", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_LOCK, "Lock Screen", "⌃⌘Q"),
        MenuRow::item(CMD_QUIT, "Quit Otto Bar", ""),
    ]
}

#[must_use]
pub fn build_tray_menu_rows(top_bar: bool, notch: bool, dock: bool) -> Vec<MenuRow> {
    vec![
        MenuRow::item(CMD_OTTO_SETTINGS, "Otto Settings...", "⌃⌥,"),
        MenuRow::item(CMD_CONTROL_CENTER, "Control Center", "⌃⌥C"),
        MenuRow::Separator,
        MenuRow::check_item(CMD_TOGGLE_TOP_BAR, "Top Bar", top_bar, true),
        MenuRow::check_item(CMD_TOGGLE_NOTCH, "Notch", notch, true),
        MenuRow::check_item(CMD_TOGGLE_DOCK, "Otto Dock", dock, true),
        MenuRow::Separator,
        MenuRow::item(CMD_OPEN_LOGS, "Open Logs Folder", ""),
        MenuRow::item(CMD_QUIT, "Quit Otto OS", ""),
    ]
}

pub(super) fn build_app_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let name = context.app_name;
    let mut rows = Vec::new();
    if context.window.is_some() || context.file_manager {
        rows.push(MenuRow::item(CMD_APP_ACTIVATE, format!("Show {name}"), ""));
        rows.push(MenuRow::Separator);
    }
    push_capable(
        &mut rows,
        context.shortcuts(),
        CMD_APP_SETTINGS,
        "Settings...",
        "⌘,",
    );
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        context.app.hide,
        CMD_APP_HIDE,
        format!("Hide {name}"),
        "⌘H",
    );
    push_capable(
        &mut rows,
        context.app.hide_others,
        CMD_APP_HIDE_OTHERS,
        "Hide Others",
        "⌥⌘H",
    );
    push_capable(
        &mut rows,
        context.app.show_all,
        CMD_APP_SHOW_ALL,
        "Show All",
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        context.app.quit,
        CMD_APP_QUIT,
        format!("Quit {name}"),
        "⌘Q",
    );
    rows
}

pub(super) fn build_file_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let keys = context.shortcuts();
    let close = context.window_capability(|window| window.close);
    let mut rows = Vec::new();
    if context.file_manager {
        push_capable(&mut rows, keys, CMD_FILE_NEW_FOLDER, "New Folder", "⇧⌃N");
        push_capable(&mut rows, keys, CMD_FILE_NEW, "New Finder Window", "⌃N");
        push_capable(&mut rows, keys, CMD_FILE_NEW_TAB, "New Tab", "⌃T");
    } else {
        push_capable(&mut rows, keys, CMD_FILE_NEW, "New", "⌃N");
        if context.tabbed {
            push_capable(&mut rows, keys, CMD_FILE_NEW_TAB, "New Tab", "⌃T");
        }
        push_capable(&mut rows, keys, CMD_FILE_OPEN, "Open...", "⌃O");
        rows.push(MenuRow::Separator);
        push_capable(&mut rows, keys, CMD_FILE_SAVE, "Save", "⌃S");
        push_capable(&mut rows, keys, CMD_FILE_SAVE_AS, "Save As...", "⇧⌃S");
    }
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, close, CMD_WIN_CLOSE, "Close Window", "⌘W");
    rows
}

pub(super) fn build_edit_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let keys = context.shortcuts();
    let mut rows = Vec::new();
    push_capable(&mut rows, keys, CMD_EDIT_UNDO, "Undo", "⌃Z");
    push_capable(&mut rows, keys, CMD_EDIT_REDO, "Redo", "⌃Y");
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, keys, CMD_EDIT_CUT, "Cut", "⌃X");
    push_capable(&mut rows, keys, CMD_EDIT_COPY, "Copy", "⌃C");
    push_capable(&mut rows, keys, CMD_EDIT_PASTE, "Paste", "⌃V");
    push_capable(&mut rows, keys, CMD_EDIT_SELECT_ALL, "Select All", "⌃A");
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, keys, CMD_EDIT_FIND, "Find...", "⌃F");
    rows
}

pub(super) fn full_screen_label(context: &MenuContext<'_>) -> &'static str {
    if context
        .window
        .is_some_and(|window| window.full_screen_active)
    {
        "Exit Full Screen"
    } else {
        "Enter Full Screen"
    }
}

pub(super) fn build_view_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let keys = context.shortcuts();
    let full_screen = context.window_capability(|window| window.full_screen);
    let mut rows = Vec::new();
    if context.file_manager {
        push_capable(&mut rows, keys, CMD_VIEW_AS_ICONS, "as Icons", "⇧⌃2");
        push_capable(&mut rows, keys, CMD_VIEW_AS_LIST, "as List", "⇧⌃5");
        push_capable(&mut rows, keys, CMD_VIEW_AS_DETAILS, "as Details", "⇧⌃6");
    } else {
        push_capable(&mut rows, keys, CMD_VIEW_ZOOM_IN, "Zoom In", "⌃+");
        push_capable(&mut rows, keys, CMD_VIEW_ZOOM_OUT, "Zoom Out", "⌃-");
        push_capable(&mut rows, keys, CMD_VIEW_ACTUAL_SIZE, "Actual Size", "⌃0");
    }
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        full_screen,
        CMD_VIEW_FULLSCREEN,
        full_screen_label(context),
        "F11",
    );
    rows
}

pub(super) fn build_window_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let tile = context.window_capability(|window| window.tile);
    let mut rows = Vec::new();
    push_capable(
        &mut rows,
        context.window_capability(|window| window.minimize),
        CMD_WIN_MINIMIZE,
        "Minimize",
        "⌘M",
    );
    push_capable(
        &mut rows,
        context.window_capability(|window| window.zoom),
        CMD_WIN_MAXIMIZE,
        "Zoom",
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        tile,
        CMD_TILE_LEFT,
        "Tile Window to Left of Screen",
        "",
    );
    push_capable(
        &mut rows,
        tile,
        CMD_TILE_RIGHT,
        "Tile Window to Right of Screen",
        "",
    );
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(CMD_TASK_VIEW, "Mission Control", "⊞Tab"));
    rows.push(MenuRow::item(CMD_SHOW_DESKTOP, "Show Desktop", "⊞D"));
    if !context.app_windows.is_empty() {
        rows.push(MenuRow::Separator);
        rows.push(MenuRow::item(
            CMD_WIN_BRING_ALL_TO_FRONT,
            "Bring All to Front",
            "",
        ));
        rows.push(MenuRow::Separator);
        for (index, entry) in context.app_windows.iter().enumerate() {
            rows.push(MenuRow::check_item(
                CMD_DOCK_WINDOW_BASE + index,
                entry.title.clone(),
                entry.active,
                true,
            ));
        }
    }
    rows
}

#[must_use]
pub fn build_top_bar_menu_rows(target: HitTarget, context: &MenuContext<'_>) -> Vec<MenuRow> {
    let rows = match target {
        HitTarget::Start => build_apple_menu_rows(),
        HitTarget::AppMenu => build_app_menu_rows(context),
        HitTarget::FileMenu => build_file_menu_rows(context),
        HitTarget::EditMenu => build_edit_menu_rows(context),
        HitTarget::ViewMenu => build_view_menu_rows(context),
        HitTarget::WindowMenu => build_window_menu_rows(context),
        HitTarget::HelpMenu => {
            let mut rows = Vec::new();
            push_capable(
                &mut rows,
                context.shortcuts(),
                CMD_HELP_F1,
                format!("{} Help", context.app_name),
                "F1",
            );
            rows
        }
        _ => Vec::new(),
    };
    let rows = trim_separators(rows);
    if rows.is_empty() && top_bar_menu_title(target).is_some() {
        return vec![MenuRow::heading("No Actions Available")];
    }
    rows
}

#[must_use]
pub fn build_calendar_rows(year: u16, month: u8, today: Option<u8>) -> Vec<MenuRow> {
    vec![
        MenuRow::Calendar { year, month, today },
        MenuRow::Separator,
        MenuRow::item(CMD_CAL_SETTINGS, "Date & Time Settings...", ""),
    ]
}

#[must_use]
pub fn build_notification_rows(notifications: &NotificationCenterSnapshot) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    match notifications.access {
        NotificationAccess::Allowed => {
            if notifications.items.is_empty() {
                rows.push(MenuRow::heading("Notifications"));
                rows.push(MenuRow::heading("No Recent Notifications"));
            } else {
                let visible: Vec<_> = notifications
                    .items
                    .iter()
                    .take(MAX_VISIBLE_NOTIFICATIONS)
                    .collect();
                let mut app_ids = Vec::new();
                for item in &visible {
                    if !app_ids.contains(&item.app_id) {
                        app_ids.push(item.app_id.clone());
                    }
                }
                for app_id in app_ids {
                    let group: Vec<_> = visible
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| item.app_id == app_id)
                        .collect();
                    let app_name = &group[0].1.app_name;
                    let heading = if group.len() > 1 {
                        format!("{app_name} ({})", group.len())
                    } else {
                        app_name.clone()
                    };
                    rows.push(MenuRow::heading(heading));
                    for (index, item) in group {
                        rows.push(MenuRow::Notification {
                            id: CMD_NOTIF_DISMISS_BASE + index,
                            app_name: item.app_name.clone(),
                            title: item.title.clone(),
                            body: item.body.clone(),
                            timestamp_label: item.timestamp_label.clone(),
                        });
                    }
                }
                rows.push(MenuRow::item(
                    CMD_NOTIF_CLEAR_ALL,
                    "Clear All Notifications",
                    "",
                ));
            }
        }
        NotificationAccess::Denied => {
            rows.push(MenuRow::heading("Notification Access Denied"));
            rows.push(MenuRow::item(
                CMD_NOTIF_REQUEST_ACCESS,
                "Check Notification Permission",
                "",
            ));
        }
        NotificationAccess::RequiresPackageIdentity => {
            rows.push(MenuRow::heading("Notifications"));
            rows.push(MenuRow::heading("Requires Package Identity"));
        }
        NotificationAccess::Unavailable => {
            rows.push(MenuRow::heading("Notifications"));
            rows.push(MenuRow::item(
                CMD_NOTIF_REQUEST_ACCESS,
                "Enable Notification Feed",
                "",
            ));
        }
    }
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(
        CMD_CC_CALENDAR,
        "Open Windows Notifications",
        "",
    ));
    rows
}

#[must_use]
pub fn build_window_tiling_menu_rows(window: &WindowMenuState) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    push_capable(
        &mut rows,
        window.zoom,
        CMD_WIN_MAXIMIZE,
        if window.zoomed { "Restore" } else { "Zoom" },
        "",
    );
    push_capable(
        &mut rows,
        window.full_screen,
        CMD_VIEW_FULLSCREEN,
        if window.full_screen_active {
            "Exit Full Screen"
        } else {
            "Enter Full Screen"
        },
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        window.tile,
        CMD_TILE_LEFT,
        "Tile Window to Left of Screen",
        "",
    );
    push_capable(
        &mut rows,
        window.tile,
        CMD_TILE_RIGHT,
        "Tile Window to Right of Screen",
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, window.minimize, CMD_WIN_MINIMIZE, "Minimize", "");
    push_capable(&mut rows, window.close, CMD_WIN_CLOSE, "Close Window", "");
    let rows = trim_separators(rows);
    if rows.is_empty() {
        return vec![MenuRow::heading("No Actions Available")];
    }
    rows
}
