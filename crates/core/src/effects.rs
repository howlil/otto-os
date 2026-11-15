use crate::app_model::{Activation, AppInfo, CommandOutcome, WindowIdentity, decide_activation};
use crate::dock::{
    DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL, DockPreferences, DockSettingsChange,
};
use crate::media::MediaCommand;
use crate::menu::{
    CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS,
    CMD_CC_LYRICS_ONLINE, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE,
    CMD_CC_SOUND, CMD_CC_THEME_AUTO, CMD_CC_THEME_DARK, CMD_CC_THEME_LIGHT, CMD_CC_THEME_TOGGLE,
    CMD_CC_WIFI, CMD_DOCK_EMPTY_TRASH, CMD_DOCK_HIDE, CMD_DOCK_NEW_WINDOW, CMD_DOCK_PIN,
    CMD_DOCK_QUIT, CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, CMD_OPEN_LOGS, CMD_OTTO_SETTINGS,
    CMD_WIN_MINIMIZE, MenuRow,
};
use crate::ports::ControlCenterAction;
use crate::theme::ThemeMode;

pub const CMD_DOWNLOADS_OPEN_FOLDER: usize = 200;
pub const CMD_DOWNLOADS_ITEM_BASE: usize = 300;
pub const CMD_DOCK_OVERFLOW_BASE: usize = 400;
pub const CMD_DOCK_TOGGLE_HIDING: usize = 500;
pub const CMD_DOCK_TOGGLE_MAGNIFICATION: usize = 501;
pub const CMD_DOCK_SIZE_SMALL: usize = 502;
pub const CMD_DOCK_SIZE_MEDIUM: usize = 503;
pub const CMD_DOCK_SIZE_LARGE: usize = 504;

#[must_use]
pub fn dock_settings_effect(cmd: usize) -> Option<DockSettingsChange> {
    match cmd {
        CMD_DOCK_TOGGLE_HIDING => Some(DockSettingsChange::ToggleHiding),
        CMD_DOCK_TOGGLE_MAGNIFICATION => Some(DockSettingsChange::ToggleMagnification),
        CMD_DOCK_SIZE_SMALL => Some(DockSettingsChange::IconSize(DOCK_ICON_SMALL)),
        CMD_DOCK_SIZE_MEDIUM => Some(DockSettingsChange::IconSize(DOCK_ICON_MEDIUM)),
        CMD_DOCK_SIZE_LARGE => Some(DockSettingsChange::IconSize(DOCK_ICON_LARGE)),
        _ => None,
    }
}

#[must_use]
pub fn dock_settings_rows(preferences: DockPreferences) -> Vec<MenuRow> {
    let hiding = if preferences.autohide {
        "Turn Hiding Off"
    } else {
        "Turn Hiding On"
    };
    let magnification = if preferences.magnification {
        "Turn Magnification Off"
    } else {
        "Turn Magnification On"
    };
    let size = |id: usize, label: &str, value: i32| {
        MenuRow::check_item(id, label, preferences.icon_size == value, true)
    };
    vec![
        MenuRow::item(CMD_DOCK_TOGGLE_HIDING, hiding, ""),
        MenuRow::item(CMD_DOCK_TOGGLE_MAGNIFICATION, magnification, ""),
        MenuRow::Separator,
        MenuRow::heading("Icon Size"),
        size(CMD_DOCK_SIZE_SMALL, "Small", DOCK_ICON_SMALL),
        size(CMD_DOCK_SIZE_MEDIUM, "Medium", DOCK_ICON_MEDIUM),
        size(CMD_DOCK_SIZE_LARGE, "Large", DOCK_ICON_LARGE),
    ]
}

#[must_use]
pub fn dock_overflow_choice(cmd: usize, hidden_count: usize) -> Option<usize> {
    cmd.checked_sub(CMD_DOCK_OVERFLOW_BASE)
        .filter(|index| *index < hidden_count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockEffect {
    Launch,
    Unhide,
    ShowWindow(WindowIdentity),
    TogglePin,
    EmptyTrash,
    Hide,
    Quit,
    Fail(CommandOutcome),
}

#[must_use]
pub fn dock_activation_effect(app: Option<&AppInfo>) -> DockEffect {
    let Some(app) = app else {
        return DockEffect::Launch;
    };
    match decide_activation(app) {
        Activation::Launch => DockEffect::Launch,
        Activation::Unhide => DockEffect::Unhide,
        Activation::Restore(window) | Activation::Focus(window) => DockEffect::ShowWindow(window),
    }
}

#[must_use]
pub fn dock_command_effect(cmd: usize, app: Option<&AppInfo>) -> Option<DockEffect> {
    if let Some(index) = cmd.checked_sub(CMD_DOCK_WINDOW_BASE) {
        let window = app
            .and_then(|app| app.primary_windows().nth(index))
            .map(|window| window.identity);
        return Some(window.map_or(
            DockEffect::Fail(CommandOutcome::TargetGone),
            DockEffect::ShowWindow,
        ));
    }
    match cmd {
        CMD_DOCK_PIN => Some(DockEffect::TogglePin),
        CMD_DOCK_TOGGLE => Some(dock_activation_effect(app)),
        CMD_DOCK_NEW_WINDOW => Some(DockEffect::Launch),
        CMD_DOCK_EMPTY_TRASH => Some(DockEffect::EmptyTrash),
        CMD_DOCK_HIDE => app.map(|_| DockEffect::Hide),
        CMD_DOCK_QUIT => app.map(|_| DockEffect::Quit),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockDropInput {
    pub source: usize,
    pub source_reorderable: bool,
    pub beyond_unpin_line: bool,
    pub target: Option<usize>,
    pub target_reorderable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockDropEffect {
    Unpin,
    Reorder { target: usize },
    Nothing,
}

#[must_use]
pub fn dock_drop_effect(input: DockDropInput) -> DockDropEffect {
    if input.beyond_unpin_line && input.source_reorderable {
        return DockDropEffect::Unpin;
    }
    match input.target {
        Some(target)
            if target != input.source && input.source_reorderable && input.target_reorderable =>
        {
            DockDropEffect::Reorder { target }
        }
        _ => DockDropEffect::Nothing,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChange {
    Cycle,
    Set(ThemeMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterEffect {
    ToggleOnlineLyrics,
    Theme(ThemeChange),
    Action(ControlCenterAction),
}

#[must_use]
pub fn control_center_effect(cmd: usize) -> Option<ControlCenterEffect> {
    let action = match cmd {
        CMD_CC_LYRICS_ONLINE => return Some(ControlCenterEffect::ToggleOnlineLyrics),
        CMD_CC_THEME_TOGGLE => return Some(ControlCenterEffect::Theme(ThemeChange::Cycle)),
        CMD_CC_THEME_LIGHT => {
            return Some(ControlCenterEffect::Theme(ThemeChange::Set(
                ThemeMode::Light,
            )));
        }
        CMD_CC_THEME_DARK => {
            return Some(ControlCenterEffect::Theme(ThemeChange::Set(
                ThemeMode::Dark,
            )));
        }
        CMD_CC_THEME_AUTO => {
            return Some(ControlCenterEffect::Theme(ThemeChange::Set(
                ThemeMode::Auto,
            )));
        }
        CMD_CC_MEDIA_TOGGLE => ControlCenterAction::Media(MediaCommand::TogglePlayPause),
        CMD_CC_MEDIA_NEXT => ControlCenterAction::Media(MediaCommand::Next),
        CMD_CC_MEDIA_PREVIOUS => ControlCenterAction::Media(MediaCommand::Previous),
        CMD_CC_CALENDAR => ControlCenterAction::OpenCalendar,
        CMD_CAL_SETTINGS => ControlCenterAction::OpenCalendarSettings,
        CMD_CC_WIFI => ControlCenterAction::OpenWifi,
        CMD_CC_BLUETOOTH => ControlCenterAction::OpenBluetooth,
        CMD_CC_FOCUS => ControlCenterAction::OpenFocus,
        CMD_CC_DISPLAY => ControlCenterAction::OpenDisplay,
        CMD_CC_SOUND => ControlCenterAction::OpenSound,
        _ => return None,
    };
    Some(ControlCenterEffect::Action(action))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMenuEffect {
    CycleTheme,
    GenieMinimize,
    OpenOttoSettings,
    OpenLogs,
    Native(usize),
}

#[must_use]
pub fn window_menu_effect(cmd: usize, minimize_available: bool) -> WindowMenuEffect {
    match cmd {
        CMD_CC_THEME_TOGGLE => WindowMenuEffect::CycleTheme,
        CMD_OTTO_SETTINGS => WindowMenuEffect::OpenOttoSettings,
        CMD_OPEN_LOGS => WindowMenuEffect::OpenLogs,
        CMD_WIN_MINIMIZE if minimize_available => WindowMenuEffect::GenieMinimize,
        _ => WindowMenuEffect::Native(cmd),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadsEffect {
    OpenFolder,
    OpenItem(usize),
}

#[must_use]
pub fn downloads_effect(cmd: usize, item_count: usize) -> Option<DownloadsEffect> {
    if cmd == CMD_DOWNLOADS_OPEN_FOLDER {
        return Some(DownloadsEffect::OpenFolder);
    }
    cmd.checked_sub(CMD_DOWNLOADS_ITEM_BASE)
        .filter(|index| *index < item_count)
        .map(DownloadsEffect::OpenItem)
}

#[cfg(test)]
mod tests {
    use super::{
        CMD_DOWNLOADS_ITEM_BASE, CMD_DOWNLOADS_OPEN_FOLDER, ControlCenterEffect, DockDropEffect,
        DockDropInput, DockEffect, DownloadsEffect, ThemeChange, WindowMenuEffect,
        control_center_effect, dock_activation_effect, dock_command_effect, dock_drop_effect,
        downloads_effect, window_menu_effect,
    };
    use crate::app_model::{
        AppIdentity, AppInfo, CommandOutcome, WindowIdentity, WindowInfo, WindowPresence,
        WindowRole, WindowTraits, group_windows,
    };
    use crate::media::MediaCommand;
    use crate::menu::{
        CMD_CC_MEDIA_NEXT, CMD_CC_THEME_DARK, CMD_CC_THEME_TOGGLE, CMD_CC_VOLUME, CMD_CC_WIFI,
        CMD_DOCK_EMPTY_TRASH, CMD_DOCK_HIDE, CMD_DOCK_NEW_WINDOW, CMD_DOCK_PIN, CMD_DOCK_QUIT,
        CMD_DOCK_TOGGLE, CMD_DOCK_WINDOW_BASE, CMD_WIN_CLOSE, CMD_WIN_MINIMIZE,
    };
    use crate::ports::ControlCenterAction;
    use crate::theme::ThemeMode;

    const EXECUTABLE: &str = "editor.exe";

    fn window(hwnd: isize, presence: WindowPresence) -> WindowInfo {
        WindowInfo {
            identity: WindowIdentity {
                hwnd,
                process_id: 7,
                thread_id: 8,
                generation: 9,
            },
            app: AppIdentity::executable(EXECUTABLE),
            app_name: String::from("Editor"),
            executable: String::from(EXECUTABLE),
            title: format!("Window {hwnd}"),
            role: WindowRole::Primary,
            presence,
            zoomed: false,
            full_screen: false,
            active: false,
            traits: WindowTraits::STANDARD,
        }
    }

    fn app(windows: Vec<WindowInfo>) -> AppInfo {
        group_windows(windows).remove(0)
    }

    #[test]
    fn activating_an_entry_without_a_running_app_launches_it() {
        assert_eq!(dock_activation_effect(None), DockEffect::Launch);
    }

    #[test]
    fn activating_a_running_app_shows_its_window() {
        let running = app(vec![window(11, WindowPresence::Visible)]);
        assert!(matches!(
            dock_activation_effect(Some(&running)),
            DockEffect::ShowWindow(identity) if identity.hwnd == 11
        ));
    }

    #[test]
    fn dock_menu_commands_map_to_one_effect_each() {
        let running = app(vec![window(11, WindowPresence::Visible)]);
        assert_eq!(
            dock_command_effect(CMD_DOCK_PIN, None),
            Some(DockEffect::TogglePin)
        );
        assert_eq!(
            dock_command_effect(CMD_DOCK_NEW_WINDOW, Some(&running)),
            Some(DockEffect::Launch)
        );
        assert_eq!(
            dock_command_effect(CMD_DOCK_EMPTY_TRASH, None),
            Some(DockEffect::EmptyTrash)
        );
        assert_eq!(
            dock_command_effect(CMD_DOCK_HIDE, Some(&running)),
            Some(DockEffect::Hide)
        );
        assert_eq!(
            dock_command_effect(CMD_DOCK_QUIT, Some(&running)),
            Some(DockEffect::Quit)
        );
        assert_eq!(
            dock_command_effect(CMD_DOCK_TOGGLE, None),
            Some(DockEffect::Launch)
        );
        assert_eq!(dock_command_effect(CMD_DOCK_WINDOW_BASE - 1, None), None);
    }

    #[test]
    fn hide_and_quit_need_a_running_app() {
        assert_eq!(dock_command_effect(CMD_DOCK_HIDE, None), None);
        assert_eq!(dock_command_effect(CMD_DOCK_QUIT, None), None);
    }

    #[test]
    fn window_rows_pick_the_indexed_primary_window_or_report_it_gone() {
        let running = app(vec![
            window(11, WindowPresence::Visible),
            window(12, WindowPresence::Minimized),
        ]);
        assert!(matches!(
            dock_command_effect(CMD_DOCK_WINDOW_BASE + 1, Some(&running)),
            Some(DockEffect::ShowWindow(identity)) if identity.hwnd == 12
        ));
        assert_eq!(
            dock_command_effect(CMD_DOCK_WINDOW_BASE + 5, Some(&running)),
            Some(DockEffect::Fail(CommandOutcome::TargetGone))
        );
        assert_eq!(
            dock_command_effect(CMD_DOCK_WINDOW_BASE, None),
            Some(DockEffect::Fail(CommandOutcome::TargetGone))
        );
    }

    fn drop_input() -> DockDropInput {
        DockDropInput {
            source: 3,
            source_reorderable: true,
            beyond_unpin_line: false,
            target: Some(5),
            target_reorderable: true,
        }
    }

    #[test]
    fn dragging_a_pin_past_the_unpin_line_removes_it() {
        let input = DockDropInput {
            beyond_unpin_line: true,
            ..drop_input()
        };
        assert_eq!(dock_drop_effect(input), DockDropEffect::Unpin);
    }

    #[test]
    fn fixed_items_are_never_unpinned_or_reordered() {
        let input = DockDropInput {
            source_reorderable: false,
            beyond_unpin_line: true,
            ..drop_input()
        };
        assert_eq!(dock_drop_effect(input), DockDropEffect::Nothing);
        let input = DockDropInput {
            target_reorderable: false,
            ..drop_input()
        };
        assert_eq!(dock_drop_effect(input), DockDropEffect::Nothing);
    }

    #[test]
    fn dropping_on_another_pin_reorders_and_on_itself_does_nothing() {
        assert_eq!(
            dock_drop_effect(drop_input()),
            DockDropEffect::Reorder { target: 5 }
        );
        let input = DockDropInput {
            target: Some(3),
            ..drop_input()
        };
        assert_eq!(dock_drop_effect(input), DockDropEffect::Nothing);
        let input = DockDropInput {
            target: None,
            ..drop_input()
        };
        assert_eq!(dock_drop_effect(input), DockDropEffect::Nothing);
    }

    #[test]
    fn control_center_commands_resolve_to_theme_lyrics_or_port_actions() {
        assert_eq!(
            control_center_effect(CMD_CC_THEME_TOGGLE),
            Some(ControlCenterEffect::Theme(ThemeChange::Cycle))
        );
        assert_eq!(
            control_center_effect(CMD_CC_THEME_DARK),
            Some(ControlCenterEffect::Theme(ThemeChange::Set(
                ThemeMode::Dark
            )))
        );
        assert_eq!(
            control_center_effect(CMD_CC_MEDIA_NEXT),
            Some(ControlCenterEffect::Action(ControlCenterAction::Media(
                MediaCommand::Next
            )))
        );
        assert_eq!(
            control_center_effect(CMD_CC_WIFI),
            Some(ControlCenterEffect::Action(ControlCenterAction::OpenWifi))
        );
        assert_eq!(control_center_effect(CMD_CC_VOLUME), None);
    }

    #[test]
    fn minimize_uses_genie_only_when_the_window_allows_it() {
        assert_eq!(
            window_menu_effect(CMD_WIN_MINIMIZE, true),
            WindowMenuEffect::GenieMinimize
        );
        assert_eq!(
            window_menu_effect(CMD_WIN_MINIMIZE, false),
            WindowMenuEffect::Native(CMD_WIN_MINIMIZE)
        );
        assert_eq!(
            window_menu_effect(CMD_WIN_CLOSE, true),
            WindowMenuEffect::Native(CMD_WIN_CLOSE)
        );
        assert_eq!(
            window_menu_effect(CMD_CC_THEME_TOGGLE, false),
            WindowMenuEffect::CycleTheme
        );
    }

    #[test]
    fn dock_settings_rows_reflect_and_change_the_preferences() {
        use crate::dock::{DOCK_ICON_LARGE, DockPreferences, DockSettingsChange};
        use crate::menu::MenuRow;
        let prefs = DockPreferences::default();
        let rows = super::dock_settings_rows(prefs);
        let labels: Vec<String> = rows
            .iter()
            .filter_map(|row| match row {
                MenuRow::Item { label, checked, .. } => Some(format!("{label}:{checked}")),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&String::from("Turn Hiding On:false")));
        assert!(labels.contains(&String::from("Turn Magnification Off:false")));
        assert!(labels.contains(&String::from("Medium:true")));
        assert!(labels.contains(&String::from("Large:false")));
        assert_eq!(
            super::dock_settings_effect(super::CMD_DOCK_SIZE_LARGE),
            Some(DockSettingsChange::IconSize(DOCK_ICON_LARGE))
        );
        assert_eq!(
            super::dock_settings_effect(super::CMD_DOCK_TOGGLE_HIDING),
            Some(DockSettingsChange::ToggleHiding)
        );
        assert_eq!(super::dock_settings_effect(499), None);
    }

    #[test]
    fn overflow_rows_pick_an_existing_hidden_app() {
        assert_eq!(
            super::dock_overflow_choice(super::CMD_DOCK_OVERFLOW_BASE, 3),
            Some(0)
        );
        assert_eq!(
            super::dock_overflow_choice(super::CMD_DOCK_OVERFLOW_BASE + 2, 3),
            Some(2)
        );
        assert_eq!(
            super::dock_overflow_choice(super::CMD_DOCK_OVERFLOW_BASE + 3, 3),
            None
        );
        assert_eq!(
            super::dock_overflow_choice(super::CMD_DOCK_OVERFLOW_BASE - 1, 3),
            None
        );
    }

    #[test]
    fn downloads_rows_open_the_folder_or_an_existing_item() {
        assert_eq!(
            downloads_effect(CMD_DOWNLOADS_OPEN_FOLDER, 0),
            Some(DownloadsEffect::OpenFolder)
        );
        assert_eq!(
            downloads_effect(CMD_DOWNLOADS_ITEM_BASE + 2, 3),
            Some(DownloadsEffect::OpenItem(2))
        );
        assert_eq!(downloads_effect(CMD_DOWNLOADS_ITEM_BASE + 3, 3), None);
        assert_eq!(downloads_effect(CMD_DOWNLOADS_ITEM_BASE - 1, 3), None);
    }
}
