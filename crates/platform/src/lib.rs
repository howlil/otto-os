mod actions;
mod appbar;
mod audio;
mod blur;
mod com;
mod control_center;
mod dock;
mod file;
mod gdi;
mod hidden_windows;
mod instance;
mod keyboard_hook;
mod lyrics;
mod media;
mod motion;
mod notch;
mod os_events;
mod screen;
mod status;
mod theme;
mod trash;
mod window_model;
mod winrt_io;
mod worker;

pub use actions::{
    HOTKEY_TOGGLE_NOTCH, HOTKEY_TOGGLE_POMODORO, MenuCommandResult, MenuNavEvent, adjust_volume,
    execute_control_center_command, execute_top_bar_command, is_escape_down, load_apple_fonts,
    open_calendar_notifications, open_downloads_folder, open_path, open_quick_settings,
    open_spotlight_search, open_start_menu, open_task_view, play_pomodoro_chime, poll_menu_events,
    register_notch_hotkeys, signal_command_failure, toggle_tray_overflow, trim_working_set,
    unregister_notch_hotkeys,
};
pub use appbar::{
    APPBAR_CALLBACK_MESSAGE, AppBar, ShellNotification, classify_shell_notification,
    restore_shell_from_recovery,
};
pub use audio::{set_system_volume, shutdown_audio_task, step_system_volume, toggle_system_mute};
pub use blur::{BackdropMaterial, apply_backdrop_material, enable_backdrop_blur};
pub use control_center::WindowsControlCenter;
pub use dock::{
    ActiveWindowTarget, DockEntry, DockItemKind, IconBitmap, PinnedApp, StackItem, WindowSnapshot,
    active_window_target, build_dock_context_menu_rows, capture_window, collect_dock_entries,
    current_process_id, cursor_position, enable_drag_accept_files, extract_dock_icon,
    extract_dropped_files, flush_compositor, focus_window_hwnd, has_maximized_or_fullscreen_window,
    is_window_cloaked, is_window_foreground, is_window_minimized, launch_new_instance,
    minimize_window_native, minimize_window_native_quiet, open_file_with_dock_entry,
    other_applications_visible, read_downloads_stack_items, remove_pinned_app, reorder_pinned_apps,
    restore_window_native, restore_window_native_quiet, set_window_transitions_enabled,
    toggle_pin_entry, window_bounds,
};
pub use file::{DATA_DIR_OVERRIDE, data_dir, replace_file_atomically};
pub use instance::InstanceGuard;
pub use keyboard_hook::{install_keyboard_hook, uninstall_keyboard_hook};
pub use lyrics::{
    LyricsOutcome, LyricsRequest, LyricsResponse, LyricsService, LyricsSource, clean_track_title,
};
pub use media::{
    MediaCommand, NowPlaying, read_now_playing, send_media_command, shutdown_media_task,
};
pub use motion::motion_preference;
pub use notch::WindowsNotchPort;
pub use os_events::{
    WindowChangeGenerations, acknowledge_window_sync, install_window_event_hooks,
    install_window_tracking_hook, poll_foreground_change, suppress_native_osd,
    uninstall_window_event_hooks, uninstall_window_tracking_hook, window_change_generations,
};
pub use screen::{
    PrimaryScreen, enable_per_monitor_dpi, monitor_work_area_at, primary_screen, virtual_screen,
};
pub use status::{read_system_status, set_display_brightness, step_display_brightness};
pub use theme::{is_high_contrast_active, is_system_in_dark_mode, is_transparency_enabled};
pub use trash::{empty_recycle_bin, is_recycle_bin_full, shutdown_recycle_bin_task};
pub use window_model::{
    ApplicationContext, ForegroundApplication, application_of, enumerate_windows,
    execute_window_command, foreground_application, hide_application, hide_other_applications,
    inspect_window, quit_application, restore_hidden_windows, show_application,
    window_command_capability, window_identity,
};
pub use worker::join_within;
