mod app_model;
mod bar_focus;
mod bar_visibility;
mod calendar;
mod capture_plan;
mod change_gate;
mod clipboard;
mod control_center;
mod cover_color;
mod dock;
mod dock_focus;
mod effects;
mod genie;
mod glass;
mod hud;
mod island;
mod layout;
mod lyrics;
mod material;
mod media;
mod media_view;
mod menu;
mod motion;
mod pixels;
mod pointer_edge;
mod pomodoro;
mod pomodoro_view;
mod ports;
mod preferences;
mod privacy;
mod shelf;
mod snapshot_cache;
mod stack;
mod status;
mod theme;
mod tick_cadence;
mod uia;
mod window_controls;

pub use app_model::{
    Activation, AppCommand, AppIdentity, AppInfo, Capability, CommandOutcome, StableOrder,
    WindowCommand, WindowIdentity, WindowInfo, WindowPresence, WindowRole, WindowTraits,
    decide_activation, group_windows,
};
pub use bar_focus::{BarFocus, BarFocusOutcome, BarKey};
pub use bar_visibility::{BarPointer, BarVisibility};
pub use calendar::{days_in_month, month_cells, month_title, shift_month, weekday_of};
pub use capture_plan::{
    BACKDROP_PADDING_PT, CaptureGate, CaptureRegion, capture_interval_for, capture_interval_ms,
    capture_wanted,
};
pub use change_gate::ChangeGate;
pub use clipboard::{
    CLIPBOARD_PAGE_SIZE, ClipboardItem, ClipboardItemId, ClipboardKind, ClipboardLayoutInput,
    ClipboardPayload, ClipboardState, MAX_CLIPBOARD_FILES, MAX_CLIPBOARD_ITEMS,
    MAX_CLIPBOARD_TEXT_BYTES,
};
pub use control_center::{
    AudioOutputDevice, BluetoothDevice, BluetoothDeviceKind, BluetoothState,
    ControlCenterClickOutcome, ControlCenterCommand, ControlCenterFocus, ControlCenterFocusOutcome,
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterMetrics, ControlCenterPage,
    ControlCenterPlacement, ControlCenterSnapshot, OttoSettingGroup, OttoSettingItem,
    OttoSettingKind, OttoSettingsCategory, OttoSettingsSnapshot, PluginSettingsState,
    SLIDER_KEY_STEP_PERCENT, SubPanelGroupLayout, SubPanelGroupSpec, SystemSettingsState,
    UPDATE_CHECKING_LABEL, WifiNetwork, WifiState, compute_control_center_layout,
    compute_control_center_layout_for_snapshot, control_center_action,
    control_center_focus_targets, focus_row_subtitle, format_update_status,
    is_update_available_status, next_dock_icon_size, parse_latest_release_tag, power_mode_label,
    scroll_offset_for_item, step_slider_percent, subpanel_opener_target,
};
pub use cover_color::cover_accent;
pub use dock::{
    AppleIconKind, DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL, DockInput, DockItemLayout,
    DockLayout, DockMetrics, DockOverflow, DockPosition, DockPreferences, DockSettingsChange,
    compute_dock_layout, dock_capacity, dock_entry_badge, dock_entry_progress,
    parse_title_badge_count, parse_title_progress, select_dock_click_window, smooth_damp,
    split_dock_overflow, step_toward,
};
pub use dock_focus::{DockFocus, DockFocusOutcome};
pub use effects::{
    CMD_DOCK_OVERFLOW_BASE, CMD_DOCK_SIZE_LARGE, CMD_DOCK_SIZE_MEDIUM, CMD_DOCK_SIZE_SMALL,
    CMD_DOCK_TOGGLE_HIDING, CMD_DOCK_TOGGLE_MAGNIFICATION, CMD_DOWNLOADS_ITEM_BASE,
    CMD_DOWNLOADS_OPEN_FOLDER, ControlCenterEffect, DockDropEffect, DockDropInput, DockEffect,
    DownloadsEffect, ThemeChange, WindowMenuEffect, control_center_effect, dock_activation_effect,
    dock_command_effect, dock_drop_effect, dock_overflow_choice, dock_settings_effect,
    dock_settings_rows, downloads_effect, window_menu_effect,
};
pub use genie::{
    GenieBoundary, GenieClip, GenieDirection, GenieGeometry, GenieStrip, compute_genie_geometry,
    genie_canvas, is_blank_frame,
};
pub use glass::{
    GlassBackdrop, GlassBitmap, GlassParams, GlassShape, Rgb, RgbBuffer, compose_glass,
    compose_glass_lit, glass_color, light_direction_for_pointer, soften_backdrop,
};
pub use hud::{
    HudGeometry, HudGlyph, HudLabel, HudPresentation, HudTone, HudValue, compute_hud_geometry,
    hud_content_alpha, present_hud,
};
pub use island::{
    CompactContent, CompactSlotKind, IslandInputs, IslandMode, IslandTargets, island_targets,
    resolve_island,
};
pub use layout::{
    HitTarget, Layout, Metrics, NotchLayoutInput, PrivacyLayoutState, Rect, ShelfLayoutInput,
    compute_layout, compute_layout_for_app, compute_layout_with_hud, compute_layout_with_notch,
    scale,
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
    KEYBOARD_SEEK_STEP_MS, LYRIC_DRIFT_TOLERANCE_MS, MediaCapabilities, MediaCommand,
    MediaSubtitle, NORMAL_PLAYBACK_RATE_PERMILLE, NotchExpandedTab, NowPlaying,
    POSITION_DRIFT_TOLERANCE_MS, PendingSeek, SEEK_CONFIRM_TOLERANCE_MS, SEEK_READBACK_TIMEOUT_MS,
    SeekReadback, TrackKey, expanded_media_subtitle, reconcile_seek, seek_target_ms,
    unsupported_reason,
};
pub use media_view::{
    LIVE_LABEL, MediaPeek, MediaPeekFrame, MediaScene, MediaView, source_label, unsupported_command,
};
pub use menu::*;
pub use motion::{
    MotionMetrics, MotionPreference, advance_bounce_phase, advance_genie_progress, animate_value,
    bounce_offset,
};
pub use pixels::{
    BgraLayout, CropRegion, PixelBufferError, PixelLimits, bgra_layout, crop_bgra,
    downsample_bgra_half, downsample_bgra_to_fit, resize_bgra, scale_dimension,
    validate_bgra_buffer,
};
pub use pointer_edge::PointerEdgeWatch;
pub use pomodoro::{
    FocusSubMode, POMODORO_EXTEND_LABEL, POMODORO_LAST_MINUTE_SECONDS, POMODORO_MAX_MINUTES,
    POMODORO_PRESETS_MINUTES, POMODORO_SUSPEND_GAP_MS, POMODORO_WARNING_SECONDS, PomodoroAlert,
    PomodoroClock, PomodoroPhase, PomodoroState, PomodoroStatus, PomodoroTick, QuickTimerState,
    StopwatchClock, StopwatchState, TIMER_PRESETS_MINUTES, format_duration_ms, format_mm_ss,
    warning_threshold_crossed,
};
pub use pomodoro_view::{
    POMODORO_SESSIONS_PER_CYCLE, PomodoroPeek, PomodoroPeekFrame, PomodoroProgress, PomodoroScene,
    PomodoroSurface, PomodoroTone, PomodoroView, format_minute_of_day,
};
pub use ports::{ControlCenterAction, ControlCenterPort, NotchAction, NotchPort};
pub use preferences::{AppPreferences, BrandIcon, NotchStyle};
pub use privacy::{MicState, PRIVACY_COMPACT_EXTRA_POINTS, PRIVACY_DOT_POINTS, PrivacyState};
pub use shelf::{
    MAX_SHELF_ITEMS, MAX_SHELF_PATH_LEN, MAX_SHELF_TEXT_BYTES, SHELF_HOVER_LEAVE_MS,
    SHELF_PAGE_SIZE, SHELF_THUMBNAIL_MAX_PX, ShelfItem, ShelfItemId, ShelfKindTag, ShelfPayload,
    ShelfState, ShelfThumbnail, classify_payload, decode_dropped_text, is_openable_shelf_url,
    is_unc_shelf_path, is_valid_shelf_file_path,
};
pub use snapshot_cache::{SnapshotBudget, SnapshotCache};
pub use stack::{
    DOWNLOADS_STACK_MAX_ITEMS, DownloadsStackEntry, DownloadsStackHit, DownloadsStackLayout,
    DownloadsStackTile, compute_downloads_stack_layout, downloads_stack_hit_test,
    next_downloads_stack_focus,
};
pub use status::{
    BatteryStatus, BluetoothConnectTracker, ClockTime, HudDeviceLabel, InputLangCode, NotchHudKind,
    NotchHudState, PowerMode, SystemStatus, detect_hud_event,
};
pub use theme::{ResolvedTheme, Rgba, ThemeMode};
pub use tick_cadence::TickCadence;
pub use uia::{
    AccessibleNode, AccessibleRole, AccessibleTree, build_bar_accessible_tree,
    build_control_center_accessible_tree, build_dock_accessible_tree, decode_bar_hit_target,
    decode_control_center_target, encode_bar_hit_target, encode_control_center_target,
};
pub use window_controls::{
    COVER_FEATHER_DIP, COVER_RIGHT_MARGIN_DIP, COVER_SAMPLE_GAP_DIP, COVER_SAMPLE_WIDTH_DIP,
    CoverBand, CoverStyle, NATIVE_CAPTION_BUTTON_DIP, NATIVE_CORNER_RADIUS_DIP, NativeCaption,
    NativeCaptionStyle, RESIZE_BORDER_DIP, ResizeEdge, TrafficLightAction, TrafficLightLayout,
    TrafficLightMetrics, WindowControlAction, WindowControlsLayout, WindowControlsMetrics,
    WindowControlsState, accepts_window_controls, compute_cover_layout,
    compute_traffic_light_layout, compute_window_controls_layout, cover_is_dark, cover_row_bands,
    extract_cover_rows, window_controls_top, window_controls_visible_under_bar,
};
