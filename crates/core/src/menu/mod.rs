mod builders;
mod commands;
mod layout;
mod notification_center;
mod notifications;
mod rows;
mod top_bar;

#[cfg(test)]
mod tests;

pub use self::builders::{
    build_apple_menu_rows, build_calendar_rows, build_notification_rows, build_top_bar_menu_rows,
    build_tray_menu_rows, build_window_tiling_menu_rows,
};
pub use self::commands::{
    CMD_APP_ACTIVATE, CMD_APP_HIDE, CMD_APP_HIDE_OTHERS, CMD_APP_QUIT, CMD_APP_SETTINGS,
    CMD_APP_SHOW_ALL, CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_SETTINGS, CMD_CAL_TODAY,
    CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT,
    CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND, CMD_CC_WIFI, CMD_CONTROL_CENTER,
    CMD_DOCK_EMPTY_TRASH, CMD_DOCK_HIDE, CMD_DOCK_NEW_WINDOW, CMD_DOCK_PIN, CMD_DOCK_QUIT,
    CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, CMD_EDIT_COPY, CMD_EDIT_CUT, CMD_EDIT_FIND,
    CMD_EDIT_PASTE, CMD_EDIT_REDO, CMD_EDIT_SELECT_ALL, CMD_EDIT_UNDO, CMD_FILE_NEW,
    CMD_FILE_NEW_FOLDER, CMD_FILE_NEW_TAB, CMD_FILE_OPEN, CMD_FILE_SAVE, CMD_FILE_SAVE_AS,
    CMD_HELP_F1, CMD_LOCK, CMD_NONE, CMD_NOTIF_CLEAR_ALL, CMD_NOTIF_DISMISS_BASE,
    CMD_NOTIF_REQUEST_ACCESS, CMD_OPEN_LOGS, CMD_OTTO_SETTINGS, CMD_OVERFLOW_MENU_BASE, CMD_QUIT,
    CMD_RESTART, CMD_SETTINGS, CMD_SHOW_DESKTOP, CMD_SHUTDOWN, CMD_SLEEP, CMD_START, CMD_TASK_VIEW,
    CMD_TASKMGR, CMD_TILE_LEFT, CMD_TILE_RIGHT, CMD_TOGGLE_DOCK, CMD_TOGGLE_NOTCH,
    CMD_TOGGLE_TOP_BAR, CMD_VIEW_ACTUAL_SIZE, CMD_VIEW_AS_DETAILS, CMD_VIEW_AS_ICONS,
    CMD_VIEW_AS_LIST, CMD_VIEW_FULLSCREEN, CMD_VIEW_ZOOM_IN, CMD_VIEW_ZOOM_OUT,
    CMD_WIN_BRING_ALL_TO_FRONT, CMD_WIN_CLOSE, CMD_WIN_MAXIMIZE, CMD_WIN_MINIMIZE,
    MAX_VISIBLE_NOTIFICATIONS, NOTIFICATION_CARD_HEIGHT, NOTIFICATION_CARD_WIDTH,
};
pub use self::layout::MenuTextMeasure;
pub use self::layout::{
    EstimatedMenuText, MenuAnchor, PopupMenuLayout, PopupMenuMetrics, PopupMenuPlacement,
    PopupMenuRowLayout, compute_popup_menu_layout,
};
pub use self::notification_center::{
    NOTIFICATION_CENTER_CARD_HEIGHT, NOTIFICATION_CENTER_CARD_RADIUS,
    NOTIFICATION_CENTER_MAX_HEIGHT, NOTIFICATION_CENTER_PAD, NOTIFICATION_CENTER_WIDTH,
    NotificationCardKind, NotificationCardLayout, NotificationCenterHit, NotificationCenterLayout,
    NotificationCenterStateCardLayout, NotificationGroup, NotificationGroupHeaderLayout,
    compute_notification_center_layout, group_notifications,
};
pub use self::notifications::{
    NotificationAccess, NotificationBadge, NotificationCenterSnapshot, NotificationSummary,
    badge_ink_is_dark, format_relative_age, notification_badge, notification_dismiss_index,
};
pub use self::rows::MenuRow;
pub use self::top_bar::{
    APPLICATION_MENUS, AppMenuState, MenuContext, OVERFLOW_TARGETS, WindowMenuEntry,
    WindowMenuState, build_menu_overflow_rows, is_top_bar_menu_target, overflow_menu_target,
    top_bar_menu_title,
};
