use crate::bar::status::SystemStatus;
use crate::layout::Rect;
use crate::notch::media::NowPlaying;

use super::model::AudioOutputDevice;
use super::model::BluetoothState;
use super::model::ControlCenterPage;
use super::model::WifiState;
use super::settings::OttoSettingsSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubPanelGroupSpec {
    pub title: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubPanelGroupLayout {
    pub title: &'static str,
    pub header: Rect,
    pub container: Rect,
    pub start_index: usize,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ControlCenterSnapshot {
    pub page: ControlCenterPage,
    pub status: SystemStatus,
    pub focus_active: bool,
    pub media: Option<NowPlaying>,
    pub wifi: WifiState,
    pub bluetooth: BluetoothState,
    pub audio_outputs: Vec<AudioOutputDevice>,
    pub audio_switch_failed: bool,
    pub settings: OttoSettingsSnapshot,
}

impl ControlCenterSnapshot {
    #[must_use]
    pub fn subpanel_item_count(&self, page: ControlCenterPage) -> usize {
        match page {
            ControlCenterPage::Main | ControlCenterPage::Display => 0,
            ControlCenterPage::Wifi => self.wifi.networks.len(),
            ControlCenterPage::Bluetooth => self.bluetooth.devices.len(),
            ControlCenterPage::Sound => self.audio_outputs.len(),
        }
    }

    #[must_use]
    pub fn subpanel_groups(&self) -> Vec<SubPanelGroupSpec> {
        match self.page {
            ControlCenterPage::Main | ControlCenterPage::Display => Vec::new(),
            ControlCenterPage::Wifi => non_empty_group("Wi-Fi Networks", self.wifi.networks.len()),
            ControlCenterPage::Bluetooth => {
                non_empty_group("Devices", self.bluetooth.devices.len())
            }
            ControlCenterPage::Sound => non_empty_group("Output Devices", self.audio_outputs.len()),
        }
    }
}

pub(super) fn non_empty_group(title: &'static str, count: usize) -> Vec<SubPanelGroupSpec> {
    if count == 0 {
        Vec::new()
    } else {
        vec![SubPanelGroupSpec { title, count }]
    }
}
