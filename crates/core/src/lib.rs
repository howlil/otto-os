mod calendar;
mod dock;
mod genie;
mod layout;
mod lyrics;
mod media;
mod menu;
mod motion;
mod pomodoro;
mod ports;
mod status;
mod theme;
mod window_controls;

pub use calendar::{days_in_month, month_cells, month_title, shift_month, weekday_of};
pub use dock::{
    AppleIconKind, DockInput, DockItemLayout, DockLayout, DockMetrics, compute_dock_layout,
    select_dock_click_window, smooth_damp, step_toward,
};
pub use genie::{
    GenieDirection, GenieGeometry, GenieStrip, compute_genie_geometry, is_blank_frame,
};
pub use layout::{
    HitTarget, Layout, Metrics, NotchLayoutInput, Rect, compute_layout, compute_layout_for_app,
    compute_layout_with_hud, compute_layout_with_notch, scale,
};
pub use lyrics::{LyricLine, LyricsSheet, parse_lrc};
pub use media::{MediaCommand, NotchExpandedTab, NowPlaying};
pub use menu::{
    CMD_APP_ACTIVATE, CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_SETTINGS, CMD_CAL_TODAY,
    CMD_CC_BLUETOOTH, CMD_CC_BRIGHTNESS, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS,
    CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND, CMD_CC_THEME_AUTO,
    CMD_CC_THEME_DARK, CMD_CC_THEME_LIGHT, CMD_CC_THEME_TOGGLE, CMD_CC_VOLUME, CMD_CC_WIFI,
    CMD_DOCK_EMPTY_TRASH, CMD_DOCK_NEW_WINDOW, CMD_DOCK_PIN, CMD_DOCK_QUIT, CMD_DOCK_TOGGLE,
    CMD_DOCK_WINDOW_BASE, CMD_EDIT_COPY, CMD_EDIT_CUT, CMD_EDIT_FIND, CMD_EDIT_PASTE,
    CMD_EDIT_REDO, CMD_EDIT_SELECT_ALL, CMD_EDIT_UNDO, CMD_FILE_NEW, CMD_FILE_NEW_TAB,
    CMD_FILE_OPEN, CMD_FILE_SAVE, CMD_FILE_SAVE_AS, CMD_HELP_F1, CMD_HELP_PALETTE, CMD_LOCK,
    CMD_NONE, CMD_QUIT, CMD_RESTART, CMD_SETTINGS, CMD_SHOW_DESKTOP, CMD_SHUTDOWN, CMD_SLEEP,
    CMD_START, CMD_TASK_VIEW, CMD_TASKMGR, CMD_TILE_LEFT, CMD_TILE_RIGHT, CMD_VIEW_ACTUAL_SIZE,
    CMD_VIEW_FULLSCREEN, CMD_VIEW_ZOOM_IN, CMD_VIEW_ZOOM_OUT, CMD_WIN_CLOSE, CMD_WIN_MAXIMIZE,
    CMD_WIN_MINIMIZE, MenuAnchor, MenuRow, PopupMenuLayout, PopupMenuMetrics, PopupMenuRowLayout,
    build_apple_menu_rows, build_calendar_rows, build_control_center_rows, build_top_bar_menu_rows,
    build_traffic_tiling_menu_rows, build_window_tiling_menu_rows, compute_popup_menu_layout,
    is_top_bar_menu_target,
};
pub use motion::{
    MotionMetrics, MotionPreference, advance_bounce_phase, advance_genie_progress, animate_value,
    bounce_offset,
};
pub use pomodoro::{PomodoroAlert, PomodoroPhase, PomodoroState, PomodoroStatus};
pub use ports::{ControlCenterAction, ControlCenterPort, NotchAction, NotchPort};
pub use status::{
    BatteryStatus, ClockTime, NotchHudKind, NotchHudState, SystemStatus, detect_hud_event,
};
pub use theme::{ResolvedTheme, ThemeMode};
pub use window_controls::{
    TrafficLightAction, TrafficLightLayout, TrafficLightMetrics, WindowControlAction,
    WindowControlsLayout, WindowControlsMetrics, compute_traffic_light_layout,
    compute_window_controls_layout,
};
