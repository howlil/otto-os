mod actions;
mod appbar;
mod blur;
mod control_center;
mod dock;
mod file;
mod instance;
mod media;
mod motion;
mod notch;
mod os_events;
mod screen;
mod status;
mod theme;
mod trash;

pub use actions::{
    HOTKEY_TOGGLE_NOTCH, HOTKEY_TOGGLE_POMODORO, adjust_volume, execute_control_center_command,
    execute_system_menu_command, execute_top_bar_command, load_apple_fonts,
    open_calendar_notifications, open_quick_settings, open_spotlight_search, open_start_menu,
    open_task_view, play_pomodoro_chime, poll_menu_dismiss_keys, register_notch_hotkeys,
    show_apple_system_menu, show_quit_menu, show_top_bar_menu, toggle_tray_overflow,
    unregister_notch_hotkeys,
};
pub use appbar::AppBar;
pub use blur::enable_backdrop_blur;
pub use control_center::WindowsControlCenter;
pub use dock::{
    ActiveWindowTarget, DockEntry, DockItemKind, DockMenuAction, IconBitmap, WindowSnapshot,
    activate_or_launch, active_window_target, build_dock_context_menu_rows, capture_window,
    close_entry_windows, collect_dock_entries, current_process_id, cursor_position,
    execute_traffic_light, execute_window_control, extract_dock_icon, flush_compositor,
    focus_window_hwnd, foreground_app_snapshot, has_maximized_or_fullscreen_window,
    is_window_foreground, is_window_minimized, launch_new_instance, minimize_window_native,
    minimize_window_native_quiet, restore_window_native, restore_window_native_quiet,
    sample_titlebar_rgb, set_window_cloaked, set_window_transitions_enabled,
    show_dock_context_menu, toggle_pin_entry, window_bounds,
};
pub use file::replace_file_atomically;
pub use instance::InstanceGuard;
pub use media::{
    MediaCommand, NowPlaying, read_now_playing, send_media_command, shutdown_media_task,
};
pub use motion::motion_preference;
pub use notch::WindowsNotchPort;
pub use os_events::{install_os_event_hooks, suppress_native_osd, uninstall_os_event_hooks};
pub use screen::{PrimaryScreen, enable_per_monitor_dpi, primary_screen};
pub use status::{
    read_system_status, set_display_brightness, set_system_volume, step_display_brightness,
    step_system_volume, toggle_system_mute,
};
pub use theme::is_system_in_dark_mode;
pub use trash::{empty_recycle_bin, is_recycle_bin_full, shutdown_recycle_bin_task};
