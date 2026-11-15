use crate::clipboard::ClipboardPayload;
use crate::media::{MediaCommand, NowPlaying};
use crate::privacy::PrivacyState;
use crate::shelf::{ShelfPayload, ShelfThumbnail};
use crate::status::SystemStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotchAction {
    Media(MediaCommand),
    PlayPomodoroChime,
    ToggleMicMute,
    ShelfOpen(ShelfPayload),
    ShelfCopy(ShelfPayload),
    ShelfReveal(ShelfPayload),
}

pub trait NotchPort {
    fn read_now_playing(&self) -> Option<NowPlaying>;

    fn read_privacy_state(&self) -> PrivacyState;

    fn pick_shelf_files(&self) -> Vec<String>;

    fn inspect_shelf_file(&self, path: &str, max_px: u32) -> (bool, Option<ShelfThumbnail>);

    fn start_shelf_drag(&self, hwnd: isize, payload: &ShelfPayload) -> bool;

    fn copy_clipboard(&self, payload: &ClipboardPayload) -> Option<u32>;

    fn execute(&self, action: NotchAction);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCenterAction {
    Media(MediaCommand),
    OpenCalendar,
    OpenCalendarSettings,
    OpenWifi,
    OpenBluetooth,
    OpenFocus,
    OpenDisplay,
    OpenNightLight,
    OpenSound,
    OpenPowerSettings,
}

pub trait ControlCenterPort {
    fn read_system_status(&self) -> SystemStatus;

    fn read_now_playing(&self) -> Option<NowPlaying>;

    fn set_volume(&self, percent: u8) -> Option<(u8, bool)>;

    fn set_muted(&self, _muted: bool) -> Option<(u8, bool)> {
        None
    }

    fn set_brightness(&self, percent: u8) -> Option<u8>;

    fn execute(&self, action: ControlCenterAction);

    fn read_wifi_state(&self) -> crate::control_center::WifiState {
        crate::control_center::WifiState::default()
    }

    fn set_wifi_enabled(&self, _enabled: bool) -> bool {
        false
    }

    fn connect_wifi(&self, _ssid: &str) -> bool {
        false
    }

    fn read_bluetooth_state(&self) -> crate::control_center::BluetoothState {
        crate::control_center::BluetoothState::default()
    }

    fn set_bluetooth_enabled(&self, _enabled: bool) -> bool {
        false
    }

    fn toggle_bluetooth_device(&self, _id: &str, _connect: bool) -> bool {
        false
    }

    fn read_audio_outputs(&self) -> Vec<crate::control_center::AudioOutputDevice> {
        Vec::new()
    }

    fn set_audio_output(&self, _id: &str) -> bool {
        false
    }

    fn read_launch_at_login(&self) -> bool {
        false
    }

    fn set_launch_at_login(&self, _enabled: bool) -> bool {
        false
    }

    fn open_logs_folder(&self) {}

    fn check_for_updates(&self) -> Option<String> {
        None
    }

    fn start_update_check(&self) -> bool {
        false
    }

    fn poll_update_status(&self) -> Option<String> {
        None
    }

    fn open_releases_page(&self) {}

    fn read_notifications(&self) -> crate::NotificationCenterSnapshot {
        crate::NotificationCenterSnapshot::default()
    }

    fn dismiss_notification(&self, _id: u32) -> bool {
        false
    }

    fn clear_notifications(&self) -> bool {
        false
    }

    fn request_notification_access(&self) -> crate::NotificationAccess {
        crate::NotificationAccess::Unavailable
    }
}
