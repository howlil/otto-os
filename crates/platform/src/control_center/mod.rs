mod storage;
mod updates;

use otto_core::{
    AudioOutputDevice, BluetoothState, CMD_CAL_SETTINGS, CMD_CC_BLUETOOTH, CMD_CC_CALENDAR,
    CMD_CC_DISPLAY, CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT, CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE,
    CMD_CC_SOUND, CMD_CC_WIFI, ControlCenterAction, ControlCenterPort, MediaCommand,
    NotificationCenterPort, NowPlaying, SystemStatus, UpdatesPort, WifiState,
};

use crate::actions::{
    execute_control_center_command, lock_screen, open_night_light_settings, open_power_settings,
    open_task_view, trigger_screenshot,
};
use crate::bar::audio::{read_audio_outputs, set_audio_output, set_system_mute, set_system_volume};
use crate::bar::connectivity::{
    connect_wifi, read_bluetooth_state, read_wifi_state, set_bluetooth_enabled, set_wifi_enabled,
    toggle_bluetooth_device,
};
use crate::bar::status::{read_system_status, set_display_brightness};
use crate::notch::media::read_now_playing;

pub use self::storage::{load_control_center_grid, save_control_center_grid};
pub(crate) use self::updates::resume_update_check_task;
pub use self::updates::{cleanup_stale_updates, shutdown_update_check_task};
use self::updates::{
    launch_staged_installer, mark_update_install_failed, open_releases_page,
    poll_async_update_state, start_async_update_check, start_async_update_install,
    take_ready_installer_path,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsControlCenter;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsUpdatesPort;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsNotificationPort;

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

    fn read_audio_outputs(&self) -> Vec<AudioOutputDevice> {
        read_audio_outputs()
    }

    fn set_audio_output(&self, id: &str) -> bool {
        set_audio_output(id)
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
            ControlCenterAction::LockScreen => {
                lock_screen();
                return;
            }
            ControlCenterAction::Screenshot => {
                trigger_screenshot();
                return;
            }
            ControlCenterAction::TaskView => {
                open_task_view();
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
            | ControlCenterAction::OpenPowerSettings
            | ControlCenterAction::LockScreen
            | ControlCenterAction::Screenshot
            | ControlCenterAction::TaskView => unreachable!(),
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
}

impl UpdatesPort for WindowsUpdatesPort {
    fn start_update_check(&self) -> bool {
        start_async_update_check()
    }

    fn poll_update_state(&self) -> otto_core::UpdateState {
        poll_async_update_state()
    }

    fn start_update_install(&self) -> bool {
        start_async_update_install()
    }

    fn take_ready_installer(&self) -> Option<String> {
        take_ready_installer_path()
    }

    fn launch_update_installer(&self, path: &str) -> bool {
        launch_staged_installer(path)
    }

    fn mark_update_install_failed(&self, message: &str) {
        mark_update_install_failed(message);
    }

    fn open_releases_page(&self) {
        open_releases_page();
    }
}

impl NotificationCenterPort for WindowsNotificationPort {
    fn read_notifications(&self) -> otto_core::NotificationCenterSnapshot {
        crate::notch::notifications::read_notifications_snapshot()
    }

    fn dismiss_notification(&self, id: u32) -> bool {
        crate::notch::notifications::dismiss_notification(id)
    }

    fn remove_notification(&self, id: u32) -> bool {
        crate::notch::notifications::remove_notification(id)
    }

    fn clear_notifications(&self) -> bool {
        crate::notch::notifications::clear_notifications()
    }

    fn request_notification_access(&self) -> otto_core::NotificationAccess {
        crate::notch::notifications::request_notification_access()
    }
}
