mod actions;
mod appearance;
mod bar;
mod control_center;
mod dock;
mod input;
mod notch;
mod settings;
mod spotlight;
mod system;
mod window;

pub(crate) use appearance::{backdrop, blur, gdi, screen, theme};
pub(crate) use bar::appbar;
pub(crate) use input::{capture, idle_wake, keyboard_hook, motion, os_events, uia};
pub(crate) use settings as settings_window;
pub(crate) use system::{
    background_cleanup, com, diagnostics, file, instance, os_version, startup, timer_resolution,
    winrt_io, worker,
};

pub use actions::{
    HOTKEY_CONTROL_CENTER, HOTKEY_FOCUS_BAR, HOTKEY_OTTO_SETTINGS, HOTKEY_QUICK_CAPTURE,
    HOTKEY_SPOTLIGHT, HOTKEY_TOGGLE_DOCK, HOTKEY_TOGGLE_NOTCH, HOTKEY_TOGGLE_POMODORO,
    HOTKEY_TOGGLE_TOP_BAR, MenuCommandResult, MenuNavEvent, TrayOverflowOutcome, adjust_volume,
    close_tray_overflow, execute_control_center_command, execute_top_bar_command, is_escape_down,
    is_shift_down, load_apple_fonts, open_calendar_notifications, open_downloads_folder, open_path,
    open_start_menu, open_task_view, play_pomodoro_chime, poll_menu_events,
    register_capture_hotkey, register_notch_hotkeys, register_settings_hotkey,
    register_shell_hotkeys, remove_apple_fonts, signal_command_failure, toggle_tray_overflow,
    trim_working_set, unregister_capture_hotkey, unregister_notch_hotkeys,
    unregister_settings_hotkey, unregister_shell_hotkeys,
};
pub use appbar::{
    APPBAR_CALLBACK_MESSAGE, AppBar, DOCK_APPBAR_CALLBACK_MESSAGE, DockReservation,
    ShellNotification, classify_shell_notification, restore_shell_from_recovery,
};
pub use backdrop::{
    BackdropFrame, BackdropRequest, BackdropWorker, capture_backdrop_now, capture_wallpaper_now,
    exclude_from_capture, include_in_capture, is_surface_visible, release_backdrop_capture_cache,
};
pub use background_cleanup::{
    collect_process_audit, set_background_cleanup, shutdown_background_cleanup,
    terminate_process_group, toggle_process_group_throttle,
};
pub use bar::audio::{
    set_system_mute, set_system_volume, shutdown_audio_task, step_system_volume,
    stop_idle_audio_task, toggle_system_mute,
};
pub use bar::connectivity::{
    WM_APP_CONTROL_CENTER_REFRESH, notify_bluetooth_hardware_change, register_bluetooth_hud_target,
    shutdown_connectivity_task, stop_idle_connectivity_task, take_bluetooth_hud_event,
    unregister_bluetooth_hud_target,
};
pub use bar::status::{
    read_system_status, set_display_brightness, shutdown_status_task, step_display_brightness,
};
pub use bar::tray_icon::{TrayIcon, TrayIconEvent, WM_APP_TRAY_ICON, classify_tray_icon_message};
pub use capture::release_capture_if_owned;
pub use com::{ComGuard, OleGuard};
pub use control_center::{WindowsControlCenter, cleanup_stale_updates, shutdown_update_check_task};
pub use diagnostics::{log_error, log_info, logs_dir, open_logs_folder};
pub use dock::trash::{
    empty_recycle_bin, is_recycle_bin_full, resume_recycle_bin_task, shutdown_recycle_bin_task,
};
pub use dock::{
    PinnedApp, WindowsDockPort, build_dock_context_menu_rows, collect_dock_entries,
    collect_dock_entries_with_options, enable_drag_accept_files, extract_app_icon_bitmap,
    extract_dock_icon, extract_dropped_files, extract_window_icon, focus_window_hwnd,
    launch_new_instance, open_file_with_dock_entry, overflow_dock_entry,
    read_downloads_active_progress, read_downloads_stack_items, read_downloads_stack_thumbnails,
    remove_pinned_app, reorder_pinned_apps, toggle_pin_entry,
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
    take_keystroke_count, uninstall_keyboard_hook,
};
pub use motion::motion_preference;
pub use notch::capture_input::{CaptureEvent, CaptureSubmission, QuickCaptureInput};
pub use notch::clipboard::{
    ClipboardListenerGuard, ClipboardRead, WM_CLIPBOARDUPDATE, clipboard_sequence_number,
    read_clipboard_payload,
};
pub use notch::focus_activity::{
    load_focus_activity, pick_blocked_app_executable, request_browser_url_inspection,
    save_focus_activity, take_browser_url_inspection, user_idle_ms,
};
pub use notch::lyrics::{
    LyricsOutcome, LyricsRequest, LyricsResponse, LyricsService, LyricsSource, clean_track_title,
};
pub use notch::media::{
    MediaCommand, NowPlaying, read_now_playing, resume_media_task, send_media_command,
    shutdown_media_task,
};
pub use notch::notch_rect::{publish_notch_rect, withdraw_notch_rect};
pub use notch::notifications::{
    clear_notifications, dismiss_notification, read_notification_app_counts,
    read_notifications_snapshot, remove_notification, request_notification_access,
    resume_notifications_task, shutdown_notifications_task, take_notification_changes,
};
pub use notch::port::WindowsNotchPort;
pub use notch::privacy::{resume_privacy_task, shutdown_privacy_task};
pub use notch::shelf::{
    ShelfDropEvent, ShelfDropQueue, ShelfDropRegion, ShelfDropTargetGuard, WM_APP_SHELF_EVENT,
    register_shelf_drop_target,
};
pub use notch::tasks::{load_tasks, save_tasks};
pub use os_events::{
    WindowChangeGenerations, acknowledge_window_sync, foreground_title_generation,
    install_window_event_hooks, install_window_tracking_hook, is_name_change_sync,
    poll_foreground_change, suppress_native_osd, uninstall_window_event_hooks,
    uninstall_window_tracking_hook, window_change_generations,
};
pub use os_version::{
    MIN_WINDOWS_11_BUILD, current_windows_build_label, is_supported_windows_build,
    is_supported_windows_version, read_windows_build, show_error_dialog,
};
pub use screen::{
    PrimaryScreen, enable_per_monitor_dpi, monitor_work_area_at, primary_screen, virtual_screen,
};
pub use settings_window::{
    apply_settings_chrome, apply_settings_min_track, create_settings_window,
    destroy_settings_window, hide_settings_window, invalidate_settings_window,
    is_settings_window_foreground, is_settings_window_visible, present_settings_window,
    settings_client_size, settings_glass_available, settings_window_dpi, suggested_dpi_rect,
};
pub use spotlight::{collect_focus_catalog_apps, collect_spotlight_apps};
pub use startup::{read_launch_at_login, set_launch_at_login};
pub use theme::{
    PowerNotificationGuard, is_battery_saver_active, is_high_contrast_active,
    is_running_on_battery, is_system_in_dark_mode, is_transparency_enabled, read_system_appearance,
    system_contrast_colors,
};
pub use uia::{WM_APP_UIA_INVOKE, handle_wm_getobject};
pub use window::caption::{
    activate_window_no_restore, double_click_ms, drag_threshold, drag_threshold_exceeded,
    forward_caption_context_menu, forward_caption_drag, forward_resize_drag, native_caption,
    place_overlay_above, place_overlay_below, sample_caption_strip, window_corner_radius_px,
};
pub use window::corners::{restore_rounded_corners, round_window_corners};
pub use window::model::{
    ApplicationContext, ForegroundApplication, application_of, enumerate_windows,
    execute_window_command, foreground_application, hide_application, hide_other_applications,
    inspect_window, quit_application, restore_hidden_windows, show_application,
    window_command_capability, window_identity,
};
pub use window::native_minimize::{
    acknowledge_native_minimize, install_native_minimize_hook, take_mouse_click_count,
    uninstall_native_minimize_hook,
};
pub use window::ops::{
    ActiveWindowTarget, WindowSnapshot, capture_window, current_process_id, cursor_position,
    flush_compositor, has_maximized_or_fullscreen_window, is_window_cloaked, is_window_foreground,
    is_window_minimized, is_window_uipi, minimize_window_native, minimize_window_native_quiet,
    restore_window_native, restore_window_native_quiet, restored_window_bounds,
    set_window_transitions_enabled, visible_window_targets, window_bounds,
};
pub use window::shield::{
    activate_window, back_in_browser, destroy_window, foreground_window, hide_window, place,
    show_over, window_below, window_dpi, window_frame,
};
pub use worker::join_within;

pub fn resume_background_tasks() {
    bar::audio::resume_audio_task();
    bar::connectivity::resume_connectivity_task();
    control_center::resume_update_check_task();
    notch::focus_activity::resume_focus_uia_task();
    notch::media::resume_media_task();
    notch::notifications::resume_notifications_task();
    notch::privacy::resume_privacy_task();
    bar::status::resume_status_task();
    dock::trash::resume_recycle_bin_task();
}

pub fn shutdown_background_tasks() {
    background_cleanup::shutdown_background_cleanup();
    backdrop::release_backdrop_capture_cache();
    notch::focus_activity::shutdown_focus_uia_task();
    notch::media::shutdown_media_task();
    bar::audio::shutdown_audio_task();
    notch::privacy::shutdown_privacy_task();
    bar::connectivity::shutdown_connectivity_task();
    dock::trash::shutdown_recycle_bin_task();
    bar::status::shutdown_status_task();
    notch::notifications::shutdown_notifications_task();
    control_center::shutdown_update_check_task();
}

pub use timer_resolution::{begin_fine_timing, end_fine_timing};
