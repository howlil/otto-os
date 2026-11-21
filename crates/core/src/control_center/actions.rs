use crate::notch::media::MediaCommand;
use crate::ports::ControlCenterAction;
use crate::settings_window::SettingsPage;

use super::commands::ControlCenterClickOutcome;
use super::commands::ControlCenterCommand;
use super::layout_types::ControlCenterHitTarget;
use super::layout_types::ControlCenterLayout;
use super::model::ControlCenterPage;
use super::snapshot::ControlCenterSnapshot;

#[must_use]
pub fn control_center_action(
    snapshot: &ControlCenterSnapshot,
    target: ControlCenterHitTarget,
) -> Option<ControlCenterClickOutcome> {
    let outcome =
        match target {
            ControlCenterHitTarget::WifiToggle => {
                if !snapshot.wifi.supported {
                    return None;
                }
                stay(ControlCenterCommand::ToggleWifi)
            }
            ControlCenterHitTarget::WifiOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Wifi))
            }
            ControlCenterHitTarget::BluetoothToggle => {
                if !snapshot.bluetooth.supported {
                    return None;
                }
                stay(ControlCenterCommand::ToggleBluetooth)
            }
            ControlCenterHitTarget::BluetoothOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Bluetooth))
            }
            ControlCenterHitTarget::FocusToggle => {
                if snapshot.settings.plugins.notch && snapshot.settings.system.focus_module {
                    stay(ControlCenterCommand::ToggleFocus)
                } else {
                    dismiss(ControlCenterCommand::OpenSettingsWindow(
                        SettingsPage::Notch,
                    ))
                }
            }
            ControlCenterHitTarget::FocusOpenSettings => dismiss(
                ControlCenterCommand::OpenSettingsWindow(SettingsPage::Focus),
            ),
            ControlCenterHitTarget::BatteryTile => dismiss(ControlCenterCommand::OpenSettings(
                ControlCenterAction::OpenPowerSettings,
            )),
            ControlCenterHitTarget::DisplayOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Display))
            }
            ControlCenterHitTarget::DisplaySlider | ControlCenterHitTarget::SoundSlider => {
                return None;
            }
            ControlCenterHitTarget::SoundOpenPage => {
                stay(ControlCenterCommand::OpenPage(ControlCenterPage::Sound))
            }
            ControlCenterHitTarget::SoundMuteToggle => stay(ControlCenterCommand::ToggleMute),
            ControlCenterHitTarget::MediaBody => {
                if snapshot.settings.plugins.notch && snapshot.settings.system.media_module {
                    dismiss(ControlCenterCommand::OpenNotchMedia)
                } else {
                    dismiss(ControlCenterCommand::OpenSettingsWindow(
                        SettingsPage::Notch,
                    ))
                }
            }
            ControlCenterHitTarget::MediaPrev => snapshot
                .settings
                .system
                .media_module
                .then_some(stay(ControlCenterCommand::Media(MediaCommand::Previous)))?,
            ControlCenterHitTarget::MediaPlayPause => {
                snapshot.settings.system.media_module.then_some(stay(
                    ControlCenterCommand::Media(MediaCommand::TogglePlayPause),
                ))?
            }
            ControlCenterHitTarget::MediaNext => snapshot
                .settings
                .system
                .media_module
                .then_some(stay(ControlCenterCommand::Media(MediaCommand::Next)))?,
            ControlCenterHitTarget::BackButton => stay(ControlCenterCommand::Back),
            ControlCenterHitTarget::HeaderToggle => match snapshot.page {
                ControlCenterPage::Wifi if snapshot.wifi.supported => {
                    stay(ControlCenterCommand::ToggleWifi)
                }
                ControlCenterPage::Bluetooth if snapshot.bluetooth.supported => {
                    stay(ControlCenterCommand::ToggleBluetooth)
                }
                _ => return None,
            },
            ControlCenterHitTarget::SubPanelItem(index) => {
                return subpanel_item_action(snapshot, index);
            }
            ControlCenterHitTarget::FooterPrimary => return footer_primary_action(snapshot.page),
            ControlCenterHitTarget::FooterSecondary => return footer_secondary_action(snapshot),
        };
    Some(outcome)
}

#[must_use]
pub fn is_update_available_status(status: Option<&str>) -> bool {
    status.is_some_and(|text| text.starts_with("Update v"))
}

#[must_use]
pub const fn focus_row_subtitle(focus_module: bool, focus_active: bool) -> &'static str {
    if !focus_module {
        "Disabled"
    } else if focus_active {
        "Focus On"
    } else {
        "Off"
    }
}

#[must_use]
pub fn step_slider_percent(current: u8, delta: i8) -> u8 {
    let stepped = (i16::from(current.min(100)) + i16::from(delta)).clamp(0, 100);
    u8::try_from(stepped).unwrap_or(0)
}

#[must_use]
pub const fn subpanel_opener_target(page: ControlCenterPage) -> Option<ControlCenterHitTarget> {
    match page {
        ControlCenterPage::Main => None,
        ControlCenterPage::Wifi => Some(ControlCenterHitTarget::WifiOpenPage),
        ControlCenterPage::Bluetooth => Some(ControlCenterHitTarget::BluetoothOpenPage),
        ControlCenterPage::Sound => Some(ControlCenterHitTarget::SoundOpenPage),
        ControlCenterPage::Display => Some(ControlCenterHitTarget::DisplayOpenPage),
    }
}

#[must_use]
pub fn scroll_offset_for_item(
    layout: &ControlCenterLayout,
    target: ControlCenterHitTarget,
) -> Option<i32> {
    let ControlCenterHitTarget::SubPanelItem(index) = target else {
        return None;
    };
    let item = layout.items.get(index)?;
    if layout.list_viewport.is_empty() || layout.max_scroll <= 0 {
        return None;
    }
    let header_h = layout
        .groups
        .iter()
        .find(|group| group.start_index == index && !group.header.is_empty())
        .map_or(0, |group| group.header.height);
    let item_top = item.y - header_h + layout.scroll_offset - layout.list_viewport.y;
    let item_bottom = item.y + item.height + layout.scroll_offset - layout.list_viewport.y;
    let next = if item.y - header_h < layout.list_viewport.y {
        item_top
    } else if item.bottom() > layout.list_viewport.bottom() {
        item_bottom - layout.list_viewport.height
    } else {
        return None;
    }
    .clamp(0, layout.max_scroll);
    (next != layout.scroll_offset).then_some(next)
}

#[must_use]
pub fn control_center_focus_targets(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> Vec<ControlCenterHitTarget> {
    if layout.page == ControlCenterPage::Main {
        main_focus_targets(layout, snapshot)
    } else {
        subpanel_focus_targets(layout, snapshot)
    }
}

pub(super) fn main_focus_targets(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> Vec<ControlCenterHitTarget> {
    let mut candidates = vec![
        (
            layout.wifi_toggle,
            snapshot.wifi.supported,
            ControlCenterHitTarget::WifiToggle,
        ),
        (layout.wifi_row, true, ControlCenterHitTarget::WifiOpenPage),
        (
            layout.bluetooth_toggle,
            snapshot.bluetooth.supported,
            ControlCenterHitTarget::BluetoothToggle,
        ),
        (
            layout.bluetooth_row,
            true,
            ControlCenterHitTarget::BluetoothOpenPage,
        ),
    ];
    candidates.extend([
        (
            layout.focus_toggle,
            true,
            ControlCenterHitTarget::FocusToggle,
        ),
        (
            layout.focus_settings,
            true,
            ControlCenterHitTarget::FocusOpenSettings,
        ),
        (
            layout.battery_tile,
            true,
            ControlCenterHitTarget::BatteryTile,
        ),
        (
            layout.display_header,
            true,
            ControlCenterHitTarget::DisplayOpenPage,
        ),
        (
            layout.display_slider,
            snapshot.status.brightness.is_some(),
            ControlCenterHitTarget::DisplaySlider,
        ),
        (
            layout.sound_header,
            true,
            ControlCenterHitTarget::SoundOpenPage,
        ),
        (
            layout.sound_mute,
            true,
            ControlCenterHitTarget::SoundMuteToggle,
        ),
        (
            layout.sound_slider,
            true,
            ControlCenterHitTarget::SoundSlider,
        ),
        (layout.media_body, true, ControlCenterHitTarget::MediaBody),
        (
            layout.media_prev,
            snapshot.settings.system.media_module,
            ControlCenterHitTarget::MediaPrev,
        ),
        (
            layout.media_play_pause,
            snapshot.settings.system.media_module,
            ControlCenterHitTarget::MediaPlayPause,
        ),
        (
            layout.media_next,
            snapshot.settings.system.media_module,
            ControlCenterHitTarget::MediaNext,
        ),
    ]);
    candidates
        .into_iter()
        .filter_map(|(rect, enabled, target)| (enabled && !rect.is_empty()).then_some(target))
        .collect()
}

pub(super) fn subpanel_focus_targets(
    layout: &ControlCenterLayout,
    snapshot: &ControlCenterSnapshot,
) -> Vec<ControlCenterHitTarget> {
    let mut out = Vec::new();
    if !layout.back_button.is_empty() {
        out.push(ControlCenterHitTarget::BackButton);
    }
    match layout.page {
        ControlCenterPage::Wifi if snapshot.wifi.supported && !layout.header_toggle.is_empty() => {
            out.push(ControlCenterHitTarget::HeaderToggle);
        }
        ControlCenterPage::Bluetooth
            if snapshot.bluetooth.supported && !layout.header_toggle.is_empty() =>
        {
            out.push(ControlCenterHitTarget::HeaderToggle);
        }
        ControlCenterPage::Sound => {
            if !layout.sound_mute.is_empty() {
                out.push(ControlCenterHitTarget::SoundMuteToggle);
            }
            if !layout.sound_slider.is_empty() {
                out.push(ControlCenterHitTarget::SoundSlider);
            }
        }
        ControlCenterPage::Display
            if snapshot.status.brightness.is_some() && !layout.display_slider.is_empty() =>
        {
            out.push(ControlCenterHitTarget::DisplaySlider);
        }
        _ => {}
    }
    for index in 0..layout.items.len() {
        out.push(ControlCenterHitTarget::SubPanelItem(index));
    }
    if !layout.footer_primary.is_empty() {
        out.push(ControlCenterHitTarget::FooterPrimary);
    }
    if !layout.footer_secondary.is_empty() {
        out.push(ControlCenterHitTarget::FooterSecondary);
    }
    out
}

pub(super) fn subpanel_item_action(
    snapshot: &ControlCenterSnapshot,
    index: usize,
) -> Option<ControlCenterClickOutcome> {
    match snapshot.page {
        ControlCenterPage::Wifi => {
            let network = snapshot.wifi.networks.get(index)?;
            Some(stay(ControlCenterCommand::ConnectWifi(
                network.ssid.clone(),
            )))
        }
        ControlCenterPage::Bluetooth => {
            let device = snapshot.bluetooth.devices.get(index)?;
            Some(stay(ControlCenterCommand::ToggleBluetoothDevice(
                device.id.clone(),
                !device.connected,
            )))
        }
        ControlCenterPage::Sound => {
            let device = snapshot.audio_outputs.get(index)?;
            Some(stay(ControlCenterCommand::SelectAudioOutput(
                device.id.clone(),
            )))
        }
        ControlCenterPage::Main | ControlCenterPage::Display => None,
    }
}

pub(super) fn footer_primary_action(page: ControlCenterPage) -> Option<ControlCenterClickOutcome> {
    match page {
        ControlCenterPage::Wifi => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenWifi,
        ))),
        ControlCenterPage::Bluetooth => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenBluetooth,
        ))),
        ControlCenterPage::Sound => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenSound,
        ))),
        ControlCenterPage::Display => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenDisplay,
        ))),
        ControlCenterPage::Main => None,
    }
}

pub(super) fn footer_secondary_action(
    snapshot: &ControlCenterSnapshot,
) -> Option<ControlCenterClickOutcome> {
    match snapshot.page {
        ControlCenterPage::Display => Some(dismiss(ControlCenterCommand::OpenSettings(
            ControlCenterAction::OpenNightLight,
        ))),
        ControlCenterPage::Main
        | ControlCenterPage::Wifi
        | ControlCenterPage::Bluetooth
        | ControlCenterPage::Sound => None,
    }
}

pub(super) const fn stay(command: ControlCenterCommand) -> ControlCenterClickOutcome {
    ControlCenterClickOutcome {
        command,
        dismiss: false,
    }
}

pub(super) const fn dismiss(command: ControlCenterCommand) -> ControlCenterClickOutcome {
    ControlCenterClickOutcome {
        command,
        dismiss: true,
    }
}
