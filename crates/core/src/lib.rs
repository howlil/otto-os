mod appearance;
mod bar;
mod control_center;
mod dock;
mod finder;
mod layout;
mod menu;
mod notch;
mod runtime;
mod settings;
mod spotlight;
mod window;

pub(crate) use appearance::{capture_plan, cover_color, glass, material, pixels, theme};
pub(crate) use runtime::{
    change_gate, flags, motion, pointer_edge, ports, process_hog, tick_cadence, uia,
};
pub(crate) use settings::{focus_settings, preferences, settings_content, settings_window};
pub(crate) use window::controls as window_controls;
pub(crate) use window::{app_model, genie, snapshot_cache};

pub use app_model::{
    Activation, AppCommand, AppIdentity, AppInfo, Capability, CommandOutcome, StableOrder,
    WindowCommand, WindowIdentity, WindowInfo, WindowPresence, WindowRole, WindowTraits,
    decide_activation, group_windows,
};
pub use bar::bar_focus::{BarFocus, BarFocusOutcome, BarKey};
pub use bar::bar_visibility::{BarPointer, BarVisibility};
pub use bar::calendar::{
    CALENDAR_POPUP_WIDTH, CalendarCell, CalendarControl, CalendarDay, CalendarLayout,
    calendar_row_height, compute_calendar_layout, days_in_month, month_grid, month_name,
    shift_month, weekday_of,
};
pub use bar::menu_bar::{BarHideMode, BarItem, BarItemSet, MenuBarPreferences};
pub use bar::status::{
    BatteryStatus, BluetoothConnectTracker, ClockTime, HudDeviceLabel, InputLangCode, NotchHudKind,
    NotchHudState, PowerMode, SystemStatus, detect_hud_event,
};
pub use capture_plan::{
    BACKDROP_PADDING_PT, CaptureGate, CaptureRegion, capture_interval_for, capture_interval_ms,
    capture_wanted,
};
pub use change_gate::ChangeGate;
pub use control_center::{
    AudioOutputDevice, BluetoothDevice, BluetoothDeviceKind, BluetoothState, CONTROL_GRID_COLUMNS,
    CONTROL_GRID_MAX_ROWS, CONTROL_GRID_MAX_TILES, ControlCategory, ControlCenterClickOutcome,
    ControlCenterCommand, ControlCenterFocus, ControlCenterFocusOutcome, ControlCenterGrid,
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterMetrics, ControlCenterPage,
    ControlCenterPlacement, ControlCenterSnapshot, ControlKind, ControlSize, ControlTile,
    ControlTileLayout, RELEASE_CHECKSUMS_ASSET, ReleaseUpdateInfo, SLIDER_KEY_STEP_PERCENT, Sha256,
    SubPanelGroupLayout, SubPanelGroupSpec, UPDATE_CHECKING_LABEL, UpdateState, WifiNetwork,
    WifiState, compute_control_center_layout_for_snapshot, control_center_action,
    control_center_focus_targets, expected_installer_asset_name, focus_row_subtitle,
    format_update_status, is_allowed_release_asset_url, is_allowed_release_page_url,
    is_update_available_status, next_dock_icon_size, parse_latest_release_tag,
    parse_release_update, parse_sha256_for_asset, parse_sha256_hex, power_mode_label,
    scroll_offset_for_item, sha256_digest, step_slider_percent, subpanel_opener_target,
};
pub use cover_color::cover_accent;
pub use dock::effects::{
    CMD_DOCK_OVERFLOW_BASE, CMD_DOCK_SIZE_LARGE, CMD_DOCK_SIZE_MEDIUM, CMD_DOCK_SIZE_SMALL,
    CMD_DOCK_TOGGLE_HIDING, CMD_DOCK_TOGGLE_MAGNIFICATION, CMD_DOWNLOADS_ITEM_BASE,
    CMD_DOWNLOADS_OPEN_FOLDER, DockDropEffect, DockDropInput, DockEffect, DownloadsEffect,
    dock_activation_effect, dock_command_effect, dock_drop_effect, dock_overflow_choice,
    dock_settings_effect, dock_settings_rows, downloads_effect,
};
pub use dock::entry::{
    DockEntry, DockEntryOptions, DockItemKind, IconBitmap, StackItem, other_applications_visible,
};
pub use dock::focus::{DockFocus, DockFocusOutcome};
pub use dock::layout::{
    AppleIconKind, DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL, DockInput, DockItemLayout,
    DockLayout, DockMetrics, DockOverflow, DockPosition, DockPreferences, DockSettingsChange,
    compute_dock_layout, dock_capacity, dock_entry_badge, dock_entry_progress, dock_rest_zone,
    dock_rest_zone_in_bounds, dock_should_hide, parse_title_badge_count, parse_title_progress,
    smooth_damp, split_dock_overflow, step_toward,
};
pub use dock::stack::{
    DOWNLOADS_STACK_MAX_ITEMS, DownloadsStackEntry, DownloadsStackHit, DownloadsStackLayout,
    DownloadsStackTile, compute_downloads_stack_layout, downloads_stack_hit_test,
    next_downloads_stack_focus,
};
pub use finder::{
    CMD_FINDER_COPY, CMD_FINDER_COPY_PATH, CMD_FINDER_CUT, CMD_FINDER_DUPLICATE,
    CMD_FINDER_GET_INFO, CMD_FINDER_MOVE_HERE, CMD_FINDER_NEW_FOLDER, CMD_FINDER_OPEN,
    CMD_FINDER_OPEN_NEW_TAB, CMD_FINDER_OPEN_TERMINAL, CMD_FINDER_PASTE, CMD_FINDER_QUICK_LOOK,
    CMD_FINDER_RENAME, CMD_FINDER_SORT_DATE, CMD_FINDER_SORT_KIND, CMD_FINDER_SORT_NAME,
    CMD_FINDER_SORT_SIZE, CMD_FINDER_SORT_TAGS, CMD_FINDER_TAG_BASE, CMD_FINDER_TOGGLE_FOLDERS_TOP,
    CMD_FINDER_TOGGLE_HIDDEN, CMD_FINDER_TOGGLE_PATH_BAR, CMD_FINDER_TOGGLE_STATUS_BAR,
    CMD_FINDER_TRASH, ENTRY_FLAG_HIDDEN, ENTRY_FLAG_READ_ONLY, ENTRY_FLAG_SYMLINK,
    FINDER_COLUMN_WIDTH_POINTS, FINDER_DEFAULT_HEIGHT_POINTS, FINDER_DEFAULT_WIDTH_POINTS,
    FINDER_ICON_SIZE_DEFAULT, FINDER_ICON_SIZE_MAX, FINDER_ICON_SIZE_MIN,
    FINDER_INSPECTOR_WIDTH_POINTS, FINDER_LIST_HEADER_HEIGHT_POINTS, FINDER_LIST_ROW_HEIGHT_POINTS,
    FINDER_MAX_TABS, FINDER_MAX_UNDO, FINDER_MIN_HEIGHT_POINTS, FINDER_MIN_WIDTH_POINTS,
    FINDER_PATH_BAR_HEIGHT_POINTS, FINDER_SIDEBAR_DEFAULT_POINTS, FINDER_SIDEBAR_MAX_POINTS,
    FINDER_SIDEBAR_MIN_POINTS, FINDER_STATUS_BAR_HEIGHT_POINTS, FINDER_TAB_BAR_HEIGHT_POINTS,
    FINDER_TAG_COUNT, FINDER_TOOLBAR_HEIGHT_POINTS, FinderBannerLayout, FinderBatchOutcome,
    FinderBreadcrumb, FinderBreadcrumbSlot, FinderClipboardData, FinderClipboardMode,
    FinderColumnHeaderSlot, FinderColumnPaneSlot, FinderColumnRowSlot, FinderEntry,
    FinderEntryKind, FinderFlags, FinderGalleryThumbSlot, FinderGoToLayout, FinderGoToState,
    FinderHit, FinderIconTileSlot, FinderInspectorLayout, FinderItemOutcome, FinderItemStatus,
    FinderKey, FinderKeyMods, FinderKeyOutcome, FinderLayout, FinderLayoutInput, FinderListRowSlot,
    FinderListing, FinderListingStatus, FinderLocation, FinderMetrics, FinderMillerColumn,
    FinderPort, FinderPreviewData, FinderPreviewKind, FinderQuickLookLayout, FinderRenameState,
    FinderSidebarHeadingSlot, FinderSidebarIcon, FinderSidebarItem, FinderSidebarSection,
    FinderSidebarSlot, FinderSortKey, FinderSortOrder, FinderState, FinderTab, FinderTabSlot,
    FinderTag, FinderToolbarButton, FinderToolbarSlot, FinderTreeRow, FinderUndoAction,
    FinderViewMode, FinderWindowControlSlot, breadcrumbs_for_location,
    build_finder_action_menu_rows, build_finder_context_menu_rows, build_finder_sort_menu_rows,
    classify_entry_kind, compare_natural, compute_finder_layout, confirm_permanent_delete,
    copy_selected, create_new_folder, duplicate_selected, execute_finder_menu_command,
    format_file_size, format_tag_mask, format_unix_timestamp, handle_finder_key,
    is_text_preview_extension, move_selected_to_trash, open_selected, parent_path_of,
    parse_tag_mask, paste_clipboard, path_display_name, request_permanent_delete, sort_entries,
    stem_selection_end, submit_go_to_folder, tag_mask_contains, tag_mask_toggle,
};
pub use flags::{EXPERIMENTAL_FLAGS_ENV, ExperimentalFlag, ExperimentalFlags};
pub use focus_settings::{
    DayBar, FOCUS_ALL_APPS_INITIAL_LIMIT, FOCUS_ALL_APPS_PAGE_STEP, FOCUS_SEARCH_MAX_CHARS,
    FOCUS_SEARCH_MAX_RESULTS, FOCUS_WEBSITE_INPUT_MAX_CHARS, FilterChip, FocusAppFilter,
    FocusCatalogApp, FocusDisclosure, FocusPageState, FocusSessionStatus, FocusSettingsTab,
    FocusTextField, KpiItem, SegmentedOption, WeeklyChart, build_weekly_chart,
    format_remaining_brief, previous_day, weekday_short,
};
pub use genie::{
    GenieBoundary, GenieClip, GenieDirection, GenieGeometry, GenieStrip, compute_genie_geometry,
    genie_canvas, is_blank_frame,
};
pub use glass::{
    GlassBackdrop, GlassBitmap, GlassParams, GlassShape, LIGHT_DIRECTION, Rgb, RgbBuffer,
    compose_glass_lit, glass_color, soften_backdrop,
};
pub use layout::{
    HitTarget, Layout, Metrics, NOTCH_CATEGORY_OPTIONS, NOTCH_HEATMAP_CELLS, NOTCH_HEATMAP_FILTERS,
    NOTIFICATION_BANNER_HEIGHT_POINTS, NOTIFICATION_RING_HEIGHT_POINTS,
    NOTIFICATION_STACK_EXTRA_PANEL_POINTS, NOTIFICATION_STACK_LIP_HEIGHT_POINTS, NotchLayoutInput,
    NotificationLayout, NotificationLayoutInput, PrivacyLayoutState, Rect, ShelfLayoutInput,
    TargetScope, compute_layout, compute_layout_for_app, compute_layout_with_hud,
    compute_layout_with_notch, scale,
};
pub use material::{
    Appearance, ContrastColors, InteractionState, MaterialVariant, SemanticColors, ShapeTokens,
    SystemAppearance,
};
pub use menu::{
    APPLICATION_MENUS, AppMenuState, CMD_APP_ACTIVATE, CMD_APP_HIDE, CMD_APP_HIDE_OTHERS,
    CMD_APP_QUIT, CMD_APP_SETTINGS, CMD_APP_SHOW_ALL, CMD_CAL_NEXT, CMD_CAL_PREVIOUS,
    CMD_CAL_SETTINGS, CMD_CAL_TODAY, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY,
    CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND,
    CMD_CC_WIFI, CMD_CONTROL_CENTER, CMD_DOCK_EMPTY_TRASH, CMD_DOCK_HIDE, CMD_DOCK_NEW_WINDOW,
    CMD_DOCK_PIN, CMD_DOCK_QUIT, CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, CMD_EDIT_COPY,
    CMD_EDIT_CUT, CMD_EDIT_FIND, CMD_EDIT_PASTE, CMD_EDIT_REDO, CMD_EDIT_SELECT_ALL, CMD_EDIT_UNDO,
    CMD_FILE_NEW, CMD_FILE_NEW_FOLDER, CMD_FILE_NEW_TAB, CMD_FILE_OPEN, CMD_FILE_SAVE,
    CMD_FILE_SAVE_AS, CMD_HELP_F1, CMD_LOCK, CMD_NONE, CMD_NOTIF_CLEAR_ALL, CMD_NOTIF_DISMISS_BASE,
    CMD_NOTIF_REQUEST_ACCESS, CMD_OPEN_LOGS, CMD_OTTO_SETTINGS, CMD_OVERFLOW_MENU_BASE, CMD_QUIT,
    CMD_RESTART, CMD_SETTINGS, CMD_SHOW_DESKTOP, CMD_SHUTDOWN, CMD_SLEEP, CMD_START, CMD_TASK_VIEW,
    CMD_TASKMGR, CMD_TILE_LEFT, CMD_TILE_RIGHT, CMD_TOGGLE_DOCK, CMD_TOGGLE_NOTCH,
    CMD_TOGGLE_TOP_BAR, CMD_VIEW_ACTUAL_SIZE, CMD_VIEW_AS_DETAILS, CMD_VIEW_AS_ICONS,
    CMD_VIEW_AS_LIST, CMD_VIEW_FULLSCREEN, CMD_VIEW_ZOOM_IN, CMD_VIEW_ZOOM_OUT,
    CMD_WIN_BRING_ALL_TO_FRONT, CMD_WIN_CLOSE, CMD_WIN_MAXIMIZE, CMD_WIN_MINIMIZE,
    EstimatedMenuText, MAX_VISIBLE_NOTIFICATIONS, MenuAnchor, MenuContext, MenuRow,
    MenuTextMeasure, NOTIFICATION_CARD_HEIGHT, NOTIFICATION_CARD_WIDTH,
    NOTIFICATION_CENTER_CARD_HEIGHT, NOTIFICATION_CENTER_CARD_RADIUS,
    NOTIFICATION_CENTER_MAX_HEIGHT, NOTIFICATION_CENTER_PAD, NOTIFICATION_CENTER_WIDTH,
    NotificationAccess, NotificationBadge, NotificationCardKind, NotificationCardLayout,
    NotificationCenterHit, NotificationCenterLayout, NotificationCenterSnapshot,
    NotificationCenterStateCardLayout, NotificationGroup, NotificationGroupHeaderLayout,
    NotificationSummary, OVERFLOW_TARGETS, PopupMenuLayout, PopupMenuMetrics, PopupMenuPlacement,
    PopupMenuRowLayout, WindowMenuEntry, WindowMenuState, badge_ink_is_dark, build_apple_menu_rows,
    build_calendar_rows, build_menu_overflow_rows, build_notification_rows,
    build_top_bar_menu_rows, build_tray_menu_rows, build_window_tiling_menu_rows,
    compute_notification_center_layout, compute_popup_menu_layout, format_relative_age,
    group_notifications, is_top_bar_menu_target, notification_badge, notification_dismiss_index,
    overflow_menu_target, top_bar_menu_title,
};
pub use motion::{
    MotionMetrics, MotionPreference, SpringState, advance_bounce_phase, animate_value,
    bounce_offset, step_spring,
};
pub use notch::call_activity::{
    CALL_DEBOUNCE_MS, CallActivity, CallScene, CallTracker, detect_call_activity,
    format_call_duration,
};
pub use notch::clipboard::{
    CLIPBOARD_PAGE_SIZE, ClipboardItem, ClipboardItemId, ClipboardKind, ClipboardLayoutInput,
    ClipboardPayload, ClipboardState, MAX_CLIPBOARD_FILES, MAX_CLIPBOARD_ITEMS,
    MAX_CLIPBOARD_TEXT_BYTES,
};
pub use notch::focus_activity::{
    AppUsageEntry, BrowserInspection, DailyActivityRecord, DayKey, FOCUS_HISTORY_DAYS,
    FOCUS_IDLE_TIMEOUT_MS, FOCUS_MAX_STEP_MS, FocusActivityState, FocusBlockEnforcement,
    FocusBlocklist, FocusRollingSummary, MAX_BLOCKED_APPS, MAX_BLOCKED_WEBSITES,
    MAX_SEARCH_QUERY_CHARS, MAX_TRACKED_APPS, MAX_TRACKED_SEARCHES, MAX_TRACKED_WEBSITES,
    PRESET_BLOCKED_APPS, PRESET_BLOCKED_WEBSITES, SearchQueryEntry, WebsiteUsageEntry,
    clean_app_display_name, compact_app_id, estimated_words, format_compact_count,
    format_screen_time, inspect_browser_context, is_browser_executable, is_non_trackable_focus_app,
    normalize_app_key, normalize_domain,
};
pub use notch::focus_heatmap::{
    CategoryShareItem, DailyCategoryFocus, FocusHeatmapLedger, FocusHeatmapView, HEATMAP_MAX_DAYS,
    HeatmapCell, HeatmapFilter, NOTCH_HEATMAP_WEEKS, SETTINGS_HEATMAP_WEEKS, build_heatmap_view,
};
pub use notch::focus_mode::{
    BackgroundReliefPolicy, CategoryColor, CategoryIcon, DEFAULT_CODING_CATEGORY_ID,
    DEFAULT_HOBBY_CATEGORY_ID, DEFAULT_STUDY_CATEGORY_ID, DEFAULT_WORK_CATEGORY_ID, FocusCategory,
    FocusCategoryStore, FocusRulePolicy, MAX_CATEGORY_APPS, MAX_CATEGORY_NAME_CHARS,
    MAX_CATEGORY_WEBSITES, MAX_FOCUS_CATEGORIES, sanitize_category_name,
};
pub use notch::focus_shield::{
    FocusShieldDismissal, FocusShieldLayout, focus_shield_layout, is_back_hit,
};
pub use notch::focus_stats_view::{
    FocusStatsRange, FocusStatsView, FocusUsageRow, NOTCH_STATS_DAYS, NOTCH_STATS_TOP_APPS,
};
pub use notch::focus_suggester::{
    AppSuggestionBucket, AppSuggestionItem, CategorySuggestion, FocusArchetype,
    apply_suggestion_to_category, classify_archetype, suggest_for_category,
};
pub use notch::hud::{
    HudGeometry, HudGlyph, HudLabel, HudPresentation, HudTone, HudValue, compute_hud_geometry,
    hud_content_alpha, present_hud,
};
pub use notch::island::{
    CompactContent, CompactSlotKind, IslandInputs, IslandMode, IslandTargets, island_targets,
    resolve_island,
};
pub use notch::lyrics::{
    LyricLine, LyricsSheet, LyricsStatus, LyricsView, MAX_LRC_LINES, MAX_LRC_TIMESTAMPS_PER_LINE,
    MAX_LRC_TOTAL_TEXT_BYTES, parse_lrc,
};
pub use notch::media::{
    KEYBOARD_SEEK_STEP_MS, LYRIC_DRIFT_TOLERANCE_MS, MediaCapabilities, MediaCommand,
    MediaSubtitle, NORMAL_PLAYBACK_RATE_PERMILLE, NotchExpandedTab, NowPlaying,
    POSITION_DRIFT_TOLERANCE_MS, PendingSeek, SEEK_CONFIRM_TOLERANCE_MS, SEEK_READBACK_TIMEOUT_MS,
    SeekReadback, TrackKey, expanded_media_subtitle, reconcile_seek, seek_target_ms,
    unsupported_reason,
};
pub use notch::media_view::{
    LIVE_LABEL, MediaPeek, MediaPeekFrame, MediaScene, MediaView, source_label, unsupported_command,
};
pub use notch::notch_notification::{
    CALL_RING_TIMEOUT_MS, NOTIFICATION_BASE_HOLD_MS, NOTIFICATION_COALESCE_FADE_MS,
    NOTIFICATION_COALESCE_MS, NOTIFICATION_HOVER_GRACE_MS, NOTIFICATION_MAX_HOLD_MS,
    NOTIFICATION_PEEK_MS, NOTIFICATION_QUEUE_LIMIT, NotchNotification, NotificationKind,
    NotificationPresentation, NotificationQueue, NotificationStage, classify_notification,
    notification_hold_ms, present_notification,
};
pub use notch::notch_rect::{NOTCH_RECT_PROPERTY, pack_notch_rect, unpack_notch_rect};
pub use notch::pomodoro::{
    POMODORO_EXTEND_LABEL, POMODORO_LAST_MINUTE_SECONDS, POMODORO_MAX_MINUTES,
    POMODORO_PRESETS_MINUTES, POMODORO_SUSPEND_GAP_MS, POMODORO_WARNING_SECONDS, PomodoroAlert,
    PomodoroClock, PomodoroPhase, PomodoroSchedule, PomodoroState, PomodoroStatus, PomodoroTick,
    format_mm_ss, warning_threshold_crossed,
};
pub use notch::pomodoro_view::{
    POMODORO_SESSIONS_PER_CYCLE, PomodoroPeek, PomodoroPeekFrame, PomodoroProgress, PomodoroScene,
    PomodoroSurface, PomodoroTone, PomodoroView, format_minute_of_day,
};
pub use notch::privacy::{
    MicState, PRIVACY_COMPACT_EXTRA_POINTS, PRIVACY_DOT_POINTS, PrivacyState,
};
pub use notch::shelf::{
    MAX_SHELF_ITEMS, MAX_SHELF_PATH_LEN, MAX_SHELF_TEXT_BYTES, SHELF_HOVER_LEAVE_MS,
    SHELF_PAGE_SIZE, SHELF_THUMBNAIL_MAX_PX, ShelfItem, ShelfItemId, ShelfKindTag, ShelfPayload,
    ShelfState, ShelfThumbnail, classify_payload, decode_dropped_text, is_openable_shelf_url,
    is_unc_shelf_path, is_valid_shelf_file_path,
};
pub use notch::tasks::{
    MAX_TASK_TEXT_BYTES, MAX_TASKS, TASKS_PAGE_SIZE, TaskAddOutcome, TaskEditOutcome,
    TaskEditorMode, TaskId, TaskItem, TasksLayoutInput, TasksState, normalize_task_text,
};
pub use pixels::{
    BgraLayout, CropRegion, PixelBufferError, PixelLimits, bgra_layout, crop_bgra,
    downsample_bgra_to_fit, resize_bgra, scale_dimension, validate_bgra_buffer,
};
pub use pointer_edge::{MonitorEdgeWatch, PointerEdgeWatch};
pub use ports::{
    ControlCenterAction, ControlCenterPort, DockPort, NotchAction, NotchPort,
    NotificationCenterPort, UpdatesPort,
};
pub use preferences::{AppPreferences, BrandIcon, NotchStyle};
pub use process_hog::{
    AuditedAppGroup, HOG_SAMPLE_INTERVAL_SECS, HogDecision, HogTracker,
    MEMORY_BUFFER_THRESHOLD_BYTES, ProcessAuditFilter, ProcessAuditReport, ProcessSafety,
    ProcessSample, ProcessThrottleKind, ProtectionReason, RawProcessAuditSample,
    SAFE_TO_END_MEMORY_THRESHOLD_BYTES, classify_process_protection, format_cpu_tenths,
    format_memory_bytes, friendly_process_display_name, summarize_process_audit,
};
pub use settings::general_settings::{
    GeneralPageState, PROCESS_AUDIT_INITIAL_LIMIT, PROCESS_AUDIT_PAGE_STEP,
};
pub use settings::model::{
    OttoSettingItem, OttoSettingKind, OttoSettingSection, OttoSettingsSnapshot,
    PluginSettingsState, SystemSettingsState,
};
pub use settings_content::{
    BadgeTone, CardLayout, ContentLayout, SettingsAction, SettingsContent, SettingsHero,
    SettingsRow, SettingsRowValue, SettingsSection, build_content, compute_content_layout,
};
pub use settings_window::{
    MODULE_COUNT, ModuleGroup, ModuleId, ModuleRow, ModuleStatus, SETTINGS_PAGE_COUNT,
    SettingsGroup, SettingsGroupHeading, SettingsHit, SettingsLayout, SettingsMetrics,
    SettingsPage, SettingsPageSlot, SettingsWindowControlSlot, compute_settings_layout,
    compute_settings_layout_with, module_enabled, modules_running, resolve_modules,
    settings_background, settings_text,
};
pub use snapshot_cache::{SnapshotBudget, SnapshotCache};
pub use spotlight::{
    SPOTLIGHT_DEFAULT_HEIGHT_POINTS, SPOTLIGHT_DEFAULT_WIDTH_POINTS, SpotlightHit, SpotlightLayout,
    SpotlightMode, SpotlightMove, SpotlightResultLayout, SpotlightView, compute_spotlight_layout,
    move_spotlight_selection, rank_spotlight_results,
};
pub use theme::{
    AccentColor, AppearancePreferences, AppearanceStyle, ResolvedAccent, ResolvedTheme, Rgba,
    ThemeMode,
};
pub use tick_cadence::TickCadence;
pub use uia::{
    AccessibleNode, AccessibleRole, AccessibleTree, build_bar_accessible_tree,
    build_control_center_accessible_tree, build_dock_accessible_tree, decode_bar_hit_target,
    decode_control_center_target, encode_bar_hit_target, encode_control_center_target,
};
pub use window::effects::{WindowMenuEffect, window_menu_effect};
pub use window_controls::{
    BarWindowControlsView, COVER_FEATHER_DIP, COVER_RIGHT_MARGIN_DIP, COVER_SAMPLE_GAP_DIP,
    COVER_SAMPLE_WIDTH_DIP, CommandScope, CoverBand, CoverStyle, NATIVE_CAPTION_BUTTON_DIP,
    NATIVE_CORNER_RADIUS_DIP, NativeCaption, NativeCaptionStyle, RESIZE_BORDER_DIP, ResizeEdge,
    WindowControlAction, WindowControlsLayout, WindowControlsMetrics, WindowControlsState,
    accepts_window_controls, active_bar_window_controls, application_command_targets,
    compute_cover_layout, cover_is_dark, cover_row_bands, extract_cover_rows, window_control_scope,
    window_controls_visible_under_bar,
};
