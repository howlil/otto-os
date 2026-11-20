mod updates;

use otto_core::{
    AudioOutputDevice, BluetoothState, CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR,
    CMD_CC_DISPLAY, CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE,
    CMD_CC_SOUND, CMD_CC_WIFI, ControlCenterAction, ControlCenterPort, MediaCommand, NowPlaying,
    SystemStatus, WifiState,
};

use crate::actions::{
    execute_control_center_command, open_night_light_settings, open_power_settings,
};
use crate::bar::audio::{read_audio_outputs, set_audio_output, set_system_mute, set_system_volume};
use crate::bar::connectivity::{
    connect_wifi, read_bluetooth_state, read_wifi_state, set_bluetooth_enabled, set_wifi_enabled,
    toggle_bluetooth_device,
};
use crate::bar::status::{read_system_status, set_display_brightness};
use crate::notch::media::read_now_playing;

pub(crate) use self::updates::resume_update_check_task;
pub use self::updates::shutdown_update_check_task;
use self::updates::{
    check_for_updates_sync, open_releases_page, poll_async_update_status, start_async_update_check,
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

    fn set_muted(&self, muted: bool) -> Option<(u8, bool)> {
        set_system_mute(muted)
    }

    fn set_brightness(&self, percent: u8) -> Option<u8> {
        set_display_brightness(percent)
    }

    fn read_launch_at_login(&self) -> bool {
        crate::startup::read_launch_at_login()
    }

    fn set_launch_at_login(&self, enabled: bool) -> bool {
        crate::startup::set_launch_at_login(enabled)
    }

    fn open_logs_folder(&self) {
        let _ = crate::diagnostics::open_logs_folder();
    }

    fn read_audio_outputs(&self) -> Vec<AudioOutputDevice> {
        read_audio_outputs()
    }

    fn set_audio_output(&self, id: &str) -> bool {
        set_audio_output(id)
    }

    fn check_for_updates(&self) -> Option<String> {
        Some(check_for_updates_sync())
    }

    fn start_update_check(&self) -> bool {
        start_async_update_check()
    }

    fn poll_update_status(&self) -> Option<String> {
        poll_async_update_status()
    }

    fn open_releases_page(&self) {
        open_releases_page();
    }

    fn execute(&self, action: ControlCenterAction) {
        match action {
            ControlCenterAction::Media(cmd @ MediaCommand::Seek(_)) => {
                if !crate::send_media_command(cmd) {
                    crate::signal_command_failure();
                }
                return;
            }
            ControlCenterAction::OpenNightLight => {
                open_night_light_settings();
                return;
            }
            ControlCenterAction::OpenPowerSettings => {
                open_power_settings();
                return;
            }
            _ => {}
        }
        let command = match action {
            ControlCenterAction::Media(MediaCommand::TogglePlayPause) => CMD_CC_MEDIA_TOGGLE,
            ControlCenterAction::Media(MediaCommand::Next) => CMD_CC_MEDIA_NEXT,
            ControlCenterAction::Media(MediaCommand::Previous) => CMD_CC_MEDIA_PREVIOUS,
            ControlCenterAction::Media(MediaCommand::Seek(_))
            | ControlCenterAction::OpenNightLight
            | ControlCenterAction::OpenPowerSettings => unreachable!(),
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

    fn read_wifi_state(&self) -> WifiState {
        read_wifi_state()
    }

    fn set_wifi_enabled(&self, enabled: bool) -> bool {
        set_wifi_enabled(enabled)
    }

    fn connect_wifi(&self, ssid: &str) -> bool {
        connect_wifi(ssid)
    }

    fn read_bluetooth_state(&self) -> BluetoothState {
        read_bluetooth_state()
    }

    fn set_bluetooth_enabled(&self, enabled: bool) -> bool {
        set_bluetooth_enabled(enabled)
    }

    fn toggle_bluetooth_device(&self, id: &str, connect: bool) -> bool {
        toggle_bluetooth_device(id, connect)
    }

    fn read_notifications(&self) -> otto_core::NotificationCenterSnapshot {
        crate::notch::notifications::read_notifications_snapshot()
    }

    fn dismiss_notification(&self, id: u32) -> bool {
        crate::notch::notifications::dismiss_notification(id)
    }

    fn clear_notifications(&self) -> bool {
        crate::notch::notifications::clear_notifications()
    }

    fn request_notification_access(&self) -> otto_core::NotificationAccess {
        crate::notch::notifications::request_notification_access()
    }
}
