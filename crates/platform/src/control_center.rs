use otto_core::{
    CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS,
    CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND, CMD_CC_WIFI,
    ControlCenterAction, ControlCenterPort, MediaCommand, NowPlaying, SystemStatus,
};

use crate::{
    execute_control_center_command, read_now_playing, read_system_status, set_display_brightness,
    set_system_volume,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsControlCenter;

impl ControlCenterPort for WindowsControlCenter {
    fn read_system_status(&self) -> SystemStatus {
        read_system_status()
    }

    fn read_now_playing(&self) -> Option<NowPlaying> {
        read_now_playing()
    }

    fn set_volume(&self, percent: u8) -> Option<(u8, bool)> {
        set_system_volume(percent)
    }

    fn set_brightness(&self, percent: u8) -> u8 {
        set_display_brightness(percent)
    }

    fn execute(&self, action: ControlCenterAction) {
        if let ControlCenterAction::Media(cmd @ MediaCommand::Seek(_)) = action {
            super::send_media_command(cmd);
            return;
        }
        let command = match action {
            ControlCenterAction::Media(MediaCommand::TogglePlayPause) => CMD_CC_MEDIA_TOGGLE,
            ControlCenterAction::Media(MediaCommand::Next) => CMD_CC_MEDIA_NEXT,
            ControlCenterAction::Media(MediaCommand::Previous) => CMD_CC_MEDIA_PREVIOUS,
            ControlCenterAction::Media(MediaCommand::Seek(_)) => unreachable!(),
            ControlCenterAction::OpenCalendar => CMD_CC_CALENDAR,
            ControlCenterAction::OpenCalendarSettings => CMD_CAL_SETTINGS,
            ControlCenterAction::OpenWifi => CMD_CC_WIFI,
            ControlCenterAction::OpenBluetooth => CMD_CC_BLUETOOTH,
            ControlCenterAction::OpenFocus => CMD_CC_FOCUS,
            ControlCenterAction::OpenDisplay => CMD_CC_DISPLAY,
            ControlCenterAction::OpenSound => CMD_CC_SOUND,
        };
        execute_control_center_command(command);
    }
}
