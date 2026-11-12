mod app_model;
mod bar_focus;
mod bar_visibility;
mod calendar;
mod change_gate;
mod dock;
mod effects;
mod genie;
mod island;
mod layout;
mod lyrics;
mod material;
mod media;
mod menu;
mod motion;
mod pixels;
mod pomodoro;
mod ports;
mod snapshot_cache;
mod status;
mod theme;
mod window_controls;

pub use app_model::{
    Activation, AppCommand, AppIdentity, AppInfo, Capability, CommandOutcome, StableOrder,
    WindowCommand, WindowIdentity, WindowInfo, WindowPresence, WindowRole, WindowTraits,
    decide_activation, group_windows,
};
pub use bar_focus::{BarFocus, BarFocusOutcome, BarKey};
pub use bar_visibility::{BarPointer, BarVisibility};
pub use calendar::{days_in_month, month_cells, month_title, shift_month, weekday_of};
pub use change_gate::ChangeGate;
pub use dock::{
    AppleIconKind, DockInput, DockItemLayout, DockLayout, DockMetrics, DockPosition,
    DockPreferences, compute_dock_layout, select_dock_click_window, smooth_damp, step_toward,
};
pub use effects::{
    CMD_DOWNLOADS_ITEM_BASE, CMD_DOWNLOADS_OPEN_FOLDER, ControlCenterEffect, DockDropEffect,
    DockDropInput, DockEffect, DownloadsEffect, ThemeChange, WindowMenuEffect,
    control_center_effect, dock_activation_effect, dock_command_effect, dock_drop_effect,
    downloads_effect, window_menu_effect,
};
pub use genie::{
    GenieDirection, GenieGeometry, GenieStrip, compute_genie_geometry, genie_canvas, is_blank_frame,
};
pub use island::{
    CompactContent, IslandInputs, IslandMode, IslandTargets, island_targets, resolve_island,
};
pub use layout::{
    HitTarget, Layout, Metrics, NotchLayoutInput, Rect, compute_layout, compute_layout_for_app,
    compute_layout_with_hud, compute_layout_with_notch, scale,
};
pub use lyrics::{
    LyricLine, LyricsSheet, LyricsStatus, LyricsView, MAX_LRC_LINES, MAX_LRC_TIMESTAMPS_PER_LINE,
    MAX_LRC_TOTAL_TEXT_BYTES, parse_lrc,
};
pub use material::{
    Appearance, ContrastColors, InteractionState, MaterialVariant, SemanticColors, ShapeTokens,
    SystemAppearance,
};
pub use media::{
    MediaCapabilities, MediaCommand, NORMAL_PLAYBACK_RATE_PERMILLE, NotchExpandedTab, NowPlaying,
    PendingSeek, SEEK_CONFIRM_TOLERANCE_MS, SEEK_READBACK_TIMEOUT_MS, SeekReadback, TrackKey,
    reconcile_seek, seek_target_ms,
};
pub use menu::*;
pub use motion::{
    MotionMetrics, MotionPreference, advance_bounce_phase, advance_genie_progress, animate_value,
    bounce_offset,
};
pub use pixels::{
    BgraLayout, CropRegion, PixelBufferError, PixelLimits, bgra_layout, crop_bgra, resize_bgra,
    scale_dimension, validate_bgra_buffer,
};
pub use pomodoro::{
    POMODORO_SUSPEND_GAP_MS, PomodoroAlert, PomodoroClock, PomodoroPhase, PomodoroState,
    PomodoroStatus, PomodoroTick,
};
pub use ports::{ControlCenterAction, ControlCenterPort, NotchAction, NotchPort};
pub use snapshot_cache::{SnapshotBudget, SnapshotCache};
pub use status::{
    BatteryStatus, ClockTime, NotchHudKind, NotchHudState, SystemStatus, detect_hud_event,
};
pub use theme::{ResolvedTheme, Rgba, ThemeMode};
pub use window_controls::{
    TrafficLightAction, TrafficLightLayout, TrafficLightMetrics, WindowControlAction,
    WindowControlsLayout, WindowControlsMetrics, WindowControlsState, accepts_window_controls,
    compute_traffic_light_layout, compute_window_controls_layout, window_controls_top,
};
