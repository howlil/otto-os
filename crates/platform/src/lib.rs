mod actions;
mod appbar;
mod audio;
mod backdrop;
mod blur;
mod caption;
mod capture;
mod clipboard;
mod com;
mod connectivity;
mod control_center;
mod diagnostics;
mod dock;
mod file;
mod gdi;
mod hidden_windows;
mod idle_wake;
mod instance;
mod keyboard_hook;
mod lyrics;
mod media;
mod motion;
mod native_minimize;
mod notch;
mod notifications;
mod os_events;
mod os_version;
mod privacy;
mod screen;
mod shelf;
mod startup;
mod status;
mod theme;
mod trash;
mod uia;
mod window_model;
mod winrt_io;
mod worker;

pub use actions::{
    HOTKEY_CONTROL_CENTER, HOTKEY_FOCUS_BAR, HOTKEY_TOGGLE_NOTCH, HOTKEY_TOGGLE_POMODORO,
    MenuCommandResult, MenuNavEvent, TrayOverflowOutcome, adjust_volume, close_tray_overflow,
    execute_control_center_command, execute_top_bar_command, is_escape_down, is_shift_down,
    load_apple_fonts, open_calendar_notifications, open_downloads_folder, open_path,
    open_quick_settings, open_spotlight_search, open_start_menu, open_task_view,
    play_pomodoro_chime, poll_menu_events, register_notch_hotkeys, remove_apple_fonts,
    signal_command_failure, toggle_tray_overflow, trim_working_set, unregister_notch_hotkeys,
};
pub use appbar::{
    APPBAR_CALLBACK_MESSAGE, AppBar, ShellNotification, classify_shell_notification,
    restore_shell_from_recovery,
};
pub use audio::{
    set_system_mute, set_system_volume, shutdown_audio_task, step_system_volume, toggle_system_mute,
};
pub use backdrop::{
    BackdropFrame, BackdropRequest, BackdropWorker, exclude_from_capture, include_in_capture,
};
pub use blur::{BackdropMaterial, apply_backdrop_material, enable_backdrop_blur};
pub use caption::{
    activate_window_no_restore, double_click_ms, drag_threshold, drag_threshold_exceeded,
    forward_caption_context_menu, forward_caption_drag, forward_resize_drag, native_caption,
    place_overlay_above, place_overlay_below, sample_caption_strip, sample_screen_color,
    window_corner_radius_px,
};
pub use capture::release_capture_if_owned;
pub use clipboard::{
    ClipboardListenerGuard, ClipboardRead, WM_CLIPBOARDUPDATE, clipboard_sequence_number,
    read_clipboard_payload,
};
pub use com::{ComGuard, OleGuard};
pub use connectivity::{
    WM_APP_CONTROL_CENTER_REFRESH, notify_bluetooth_hardware_change, register_bluetooth_hud_target,
    shutdown_connectivity_task, take_bluetooth_hud_event, unregister_bluetooth_hud_target,
};
pub use control_center::{WindowsControlCenter, shutdown_update_check_task};
pub use diagnostics::{log_error, log_info, logs_dir, open_logs_folder};
pub use dock::{
    ActiveWindowTarget, DockEntry, DockEntryOptions, DockItemKind, IconBitmap, OVERFLOW_ENTRY_KEY,
    PinnedApp, StackItem, WindowSnapshot, build_dock_context_menu_rows, capture_window,
    collect_dock_entries, collect_dock_entries_with_options, current_process_id, cursor_position,
    enable_drag_accept_files, extract_dock_icon, extract_dropped_files, extract_window_icon,
    flush_compositor, focus_window_hwnd, has_maximized_or_fullscreen_window, is_window_cloaked,
    is_window_foreground, is_window_minimized, is_window_uipi, launch_new_instance,
    minimize_window_native, minimize_window_native_quiet, open_file_with_dock_entry,
    other_applications_visible, overflow_dock_entry, read_downloads_active_progress,
    read_downloads_stack_items, read_downloads_stack_thumbnails, remove_pinned_app,
    reorder_pinned_apps, restore_window_native, restore_window_native_quiet,
    restored_window_bounds, set_window_transitions_enabled, toggle_pin_entry,
    visible_window_targets, window_bounds,
};
pub use file::{DATA_DIR_OVERRIDE, data_dir, replace_file_atomically};
pub use idle_wake::{
    acknowledge_idle_wake, register_idle_wake, unregister_idle_wake, watch_bottom_edge,
    watch_top_edge,
};
pub use instance::{InstanceGuard, quit_running_instance};
pub use keyboard_hook::{
    install_keyboard_hook, is_routing_bar_keys, is_volume_hud_hook_enabled, route_bar_keys,
    routes_keys_to, set_volume_hud_hook_enabled, stop_routing_bar_keys, stop_routing_if_targets,
    uninstall_keyboard_hook,
};
pub use lyrics::{
    LyricsOutcome, LyricsRequest, LyricsResponse, LyricsService, LyricsSource, clean_track_title,
};
pub use media::{
    MediaCommand, NowPlaying, read_now_playing, resume_media_task, send_media_command,
    shutdown_media_task,
};
pub use motion::motion_preference;
pub use native_minimize::{
    acknowledge_native_minimize, install_native_minimize_hook, uninstall_native_minimize_hook,
};
pub use notch::WindowsNotchPort;
pub use notifications::{
    clear_notifications, dismiss_notification, read_notification_app_counts,
    read_notifications_snapshot, request_notification_access, shutdown_notifications_task,
};
pub use os_events::{
    WindowChangeGenerations, acknowledge_window_sync, install_window_event_hooks,
    install_window_tracking_hook, is_name_change_sync, poll_foreground_change, suppress_native_osd,
    uninstall_window_event_hooks, uninstall_window_tracking_hook, window_change_generations,
};
pub use os_version::{
    MIN_WINDOWS_11_BUILD, current_windows_build_label, is_supported_windows_build,
    is_supported_windows_version, read_windows_build, show_error_dialog,
};
pub use privacy::{resume_privacy_task, shutdown_privacy_task};
pub use screen::{
    PrimaryScreen, enable_per_monitor_dpi, monitor_work_area_at, primary_screen, virtual_screen,
};
pub use shelf::{
    ShelfDropEvent, ShelfDropQueue, ShelfDropRegion, ShelfDropTargetGuard, WM_APP_SHELF_EVENT,
    register_shelf_drop_target,
};
pub use startup::{read_launch_at_login, set_launch_at_login};
pub use status::{
    read_system_status, set_display_brightness, shutdown_status_task, step_display_brightness,
};
pub use theme::{
    PowerNotificationGuard, is_battery_saver_active, is_high_contrast_active,
    is_running_on_battery, is_system_in_dark_mode, is_transparency_enabled, read_system_appearance,
    system_contrast_colors,
};
pub use trash::{
    empty_recycle_bin, is_recycle_bin_full, resume_recycle_bin_task, shutdown_recycle_bin_task,
};
pub use uia::{WM_APP_UIA_INVOKE, handle_wm_getobject};
pub use window_model::{
    ApplicationContext, ForegroundApplication, application_of, enumerate_windows,
    execute_window_command, foreground_application, hide_application, hide_other_applications,
    inspect_window, quit_application, restore_hidden_windows, show_application,
    window_command_capability, window_identity,
};
pub use worker::join_within;

pub fn resume_background_tasks() {
    audio::resume_audio_task();
    connectivity::resume_connectivity_task();
    control_center::resume_update_check_task();
    media::resume_media_task();
    notifications::resume_notifications_task();
    privacy::resume_privacy_task();
    status::resume_status_task();
    trash::resume_recycle_bin_task();
}

pub fn shutdown_background_tasks() {
    media::shutdown_media_task();
    audio::shutdown_audio_task();
    privacy::shutdown_privacy_task();
    connectivity::shutdown_connectivity_task();
    trash::shutdown_recycle_bin_task();
    status::shutdown_status_task();
    notifications::shutdown_notifications_task();
    control_center::shutdown_update_check_task();
}
