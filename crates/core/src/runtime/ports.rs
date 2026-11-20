use std::collections::HashMap;

use crate::app_model::StableOrder;
use crate::app_model::{AppIdentity, AppInfo, CommandOutcome, WindowCommand, WindowIdentity};
use crate::bar::status::SystemStatus;
use crate::dock::entry::{DockEntry, DockEntryOptions, IconBitmap};
use crate::layout::Rect;
use crate::notch::clipboard::ClipboardPayload;
use crate::notch::media::{MediaCommand, NowPlaying};
use crate::notch::privacy::PrivacyState;
use crate::notch::shelf::{ShelfPayload, ShelfThumbnail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotchAction {
    Media(MediaCommand),
    PlayPomodoroChime,
    ToggleMicMute,
    ShelfOpen(ShelfPayload),
    ShelfCopy(ShelfPayload),
    ShelfReveal(ShelfPayload),
    NotificationOpen(u32),
    NotificationDismiss(u32),
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

pub trait DockPort {
    fn collect_entries(
        &self,
        own_pid: u32,
        order: &mut StableOrder,
        options: DockEntryOptions,
    ) -> Vec<DockEntry>;

    fn overflow_entry(&self, hidden: usize) -> DockEntry;

    fn icon_for_entry(&self, entry: &DockEntry) -> Option<IconBitmap>;

    fn icon_for_window(&self, hwnd: isize) -> Option<IconBitmap>;

    fn downloads_progress(&self) -> Option<f32>;

    fn notification_counts(&self) -> HashMap<String, u32>;

    fn is_window_minimized(&self, hwnd: isize) -> bool;

    fn window_overlaps_zone(&self, zone: Rect, own_pid: u32) -> bool;

    fn fullscreen_on_primary(&self, width: i32, height: i32, own_pid: u32) -> bool;

    fn toggle_pin(&self, entry: &DockEntry);

    fn remove_pin(&self, key: &str) -> bool;

    fn reorder_pins(&self, moved_key: &str, target_key: &str) -> bool;

    fn pin_at_target(&self, entry: &DockEntry, target_key: &str) -> bool;

    fn launch_new_instance(&self, entry: &DockEntry);

    fn open_file_with(&self, entry: &DockEntry, path: &str);

    fn reveal_in_explorer(&self, entry: &DockEntry);

    fn force_quit(&self, entry: &DockEntry, own_pid: u32) -> CommandOutcome;

    fn window_command(
        &self,
        target: WindowIdentity,
        command: WindowCommand,
        own_pid: u32,
    ) -> CommandOutcome;

    fn hide_application(&self, app: &AppInfo, own_pid: u32) -> CommandOutcome;

    fn show_application(&self, app: &AppInfo, own_pid: u32) -> CommandOutcome;

    fn quit_application(&self, app: &AppInfo, own_pid: u32) -> CommandOutcome;

    fn hide_other_applications(&self, keep: &AppIdentity, own_pid: u32) -> CommandOutcome;

    fn empty_recycle_bin(&self);
}
