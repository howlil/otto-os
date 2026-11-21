mod fonts;
mod hotkeys;
mod keys;
mod menu_commands;
mod shell;
mod system;
mod tray;

#[cfg(test)]
mod tests;

pub use self::fonts::{load_apple_fonts, remove_apple_fonts};
pub use self::hotkeys::{
    HOTKEY_CONTROL_CENTER, HOTKEY_FOCUS_BAR, HOTKEY_OTTO_SETTINGS, HOTKEY_QUICK_CAPTURE,
    HOTKEY_SPOTLIGHT, HOTKEY_TOGGLE_DOCK, HOTKEY_TOGGLE_NOTCH, HOTKEY_TOGGLE_POMODORO,
    HOTKEY_TOGGLE_TOP_BAR, register_capture_hotkey, register_notch_hotkeys,
    register_settings_hotkey, register_shell_hotkeys, unregister_capture_hotkey,
    unregister_notch_hotkeys, unregister_settings_hotkey, unregister_shell_hotkeys,
};
pub(crate) use self::keys::send_key_chord;
pub use self::keys::{MenuNavEvent, is_escape_down, is_shift_down, poll_menu_events};
pub use self::menu_commands::{
    MenuCommandResult, execute_control_center_command, execute_top_bar_command,
};
pub use self::shell::{
    adjust_volume, open_calendar_notifications, open_downloads_folder, open_path, open_start_menu,
    open_task_view,
};
pub(crate) use self::shell::{
    open_external_url, open_night_light_settings, open_power_settings, shell_exec_ok,
};
pub use self::system::{play_pomodoro_chime, signal_command_failure, trim_working_set};
pub use self::tray::{TrayOverflowOutcome, close_tray_overflow, toggle_tray_overflow};
