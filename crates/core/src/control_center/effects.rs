use crate::menu::{
    CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS,
    CMD_CC_LYRICS_ONLINE, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE,
    CMD_CC_SOUND, CMD_CC_THEME_AUTO, CMD_CC_THEME_DARK, CMD_CC_THEME_LIGHT, CMD_CC_THEME_TOGGLE,
    CMD_CC_WIFI,
};
use crate::notch::media::MediaCommand;
use crate::ports::ControlCenterAction;
use crate::theme::ThemeMode;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{
        CMD_CC_MEDIA_NEXT, CMD_CC_THEME_DARK, CMD_CC_THEME_TOGGLE, CMD_CC_VOLUME, CMD_CC_WIFI,
    };

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
}
