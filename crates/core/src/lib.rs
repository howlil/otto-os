mod app_model;
mod bar_visibility;
mod calendar;
mod dock;
mod genie;
mod layout;
mod lyrics;
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
pub use bar_visibility::{BarPointer, BarVisibility};
pub use calendar::{days_in_month, month_cells, month_title, shift_month, weekday_of};
pub use dock::{
    AppleIconKind, DockInput, DockItemLayout, DockLayout, DockMetrics, DockPosition,
    DockPreferences, compute_dock_layout, select_dock_click_window, smooth_damp, step_toward,
};
pub use genie::{
    GenieDirection, GenieGeometry, GenieStrip, compute_genie_geometry, genie_canvas, is_blank_frame,
};
pub use layout::{
    HitTarget, Layout, Metrics, NotchLayoutInput, Rect, compute_layout, compute_layout_for_app,
    compute_layout_with_hud, compute_layout_with_notch, scale,
};
pub use lyrics::{
    LyricLine, LyricsSheet, MAX_LRC_LINES, MAX_LRC_TIMESTAMPS_PER_LINE, MAX_LRC_TOTAL_TEXT_BYTES,
    parse_lrc,
};
pub use media::{MediaCommand, NotchExpandedTab, NowPlaying};
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
