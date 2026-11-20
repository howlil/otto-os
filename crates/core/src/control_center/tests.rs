use crate::bar::menu_bar::BarItem;
use crate::layout::{Rect, scale};
use crate::settings_window::SettingsPage;

use super::actions::*;
use super::commands::*;
use super::layout::*;
use super::layout_types::*;
use super::model::*;
use super::settings::*;
use super::snapshot::*;
use super::update::*;

fn bounds() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    }
}

fn wifi_list_snapshot() -> ControlCenterSnapshot {
    let mut snap = ControlCenterSnapshot {
        page: ControlCenterPage::Wifi,
        ..ControlCenterSnapshot::default()
    };
    snap.wifi.supported = true;
    snap.wifi.networks = (0..14)
        .map(|index| WifiNetwork {
            ssid: format!("network-{index}"),
            ..WifiNetwork::default()
        })
        .collect();
    snap
}

fn main_layout(dpi: u32) -> ControlCenterLayout {
    compute_control_center_layout(
        &ControlCenterMetrics::DEFAULT,
        ControlCenterPlacement {
            bounds: bounds(),
            dpi,
            anchor_right: 1750,
            anchor_y: 36,
            page: ControlCenterPage::Main,
            item_count: 0,
            scroll_offset: 0,
        },
    )
}

#[test]
fn main_layout_places_window_within_work_area_across_dpis() {
    for dpi in [96, 144, 192] {
        let layout = main_layout(dpi);
        assert!(layout.window.width > 0 && layout.window.height > 0);
        assert!(layout.window.right() <= bounds().right());
        assert!(layout.window.bottom() <= bounds().bottom());
        assert!(layout.connectivity_card.width > 0);
        assert_eq!(layout.focus_toggle.x, layout.battery_tile.x);
        assert_eq!(layout.focus_toggle.width, layout.battery_tile.width);
        assert!(layout.focus_toggle.bottom() < layout.battery_tile.y);
        assert!(layout.display_slider.width > 200);
        assert!(layout.sound_slider.width > 180);
        assert_eq!(layout.sound_mute.x, layout.display_slider.x);
        assert!(layout.sound_mute.right() < layout.sound_slider.x);
        assert_eq!(layout.display_slider.right(), layout.sound_slider.right());
        assert!(layout.sound_mute.height >= scale(24, dpi));
        assert!(layout.media_body.right() <= layout.media_prev.x);
    }
}

#[test]
fn hit_test_prioritizes_nested_connectivity_media_and_sound_controls() {
    let layout = main_layout(96);
    let center = |r: Rect| (r.x + r.width / 2, r.y + r.height / 2);

    let (x, y) = center(layout.wifi_toggle);
    assert_eq!(
        layout.hit_test(x, y),
        Some(ControlCenterHitTarget::WifiToggle)
    );
    assert_eq!(
        layout.hit_test(layout.wifi_row.right() - 10, layout.wifi_row.y + 8),
        Some(ControlCenterHitTarget::WifiOpenPage)
    );
    let (bx, by) = center(layout.bluetooth_toggle);
    assert_eq!(
        layout.hit_test(bx, by),
        Some(ControlCenterHitTarget::BluetoothToggle)
    );
    let (battery_x, battery_y) = center(layout.battery_tile);
    assert_eq!(
        layout.hit_test(battery_x, battery_y),
        Some(ControlCenterHitTarget::BatteryTile)
    );
    let (fsx, fsy) = center(layout.focus_settings);
    assert_eq!(
        layout.hit_test(fsx, fsy),
        Some(ControlCenterHitTarget::FocusOpenSettings)
    );
    assert_eq!(
        layout.hit_test(
            layout.focus_toggle.x + 12,
            layout.focus_toggle.bottom() - 12
        ),
        Some(ControlCenterHitTarget::FocusToggle)
    );
    let (mx, my) = center(layout.sound_mute);
    assert_eq!(
        layout.hit_test(mx, my),
        Some(ControlCenterHitTarget::SoundMuteToggle)
    );
    let (px, py) = center(layout.media_play_pause);
    assert_eq!(
        layout.hit_test(px, py),
        Some(ControlCenterHitTarget::MediaPlayPause)
    );
    let (mbx, mby) = center(layout.media_body);
    assert_eq!(
        layout.hit_test(mbx, mby),
        Some(ControlCenterHitTarget::MediaBody)
    );
}

#[test]
fn slider_value_clamps_bounds_and_interpolates_midpoint() {
    let layout = main_layout(96);
    assert_eq!(
        layout.slider_value_at(ControlCenterHitTarget::DisplaySlider, 0),
        Some(0)
    );
    assert_eq!(
        layout.slider_value_at(
            ControlCenterHitTarget::DisplaySlider,
            layout.display_slider.right() + 50
        ),
        Some(100)
    );
    let mid = layout.display_slider.x + layout.display_slider.width / 2;
    let pct = layout
        .slider_value_at(ControlCenterHitTarget::DisplaySlider, mid)
        .unwrap();
    assert!((49..=51).contains(&pct));
    assert_eq!(
        layout.slider_value_at(ControlCenterHitTarget::BatteryTile, mid),
        None
    );
}

#[test]
fn scrollable_subpanel_clips_items_to_viewport_and_clamps_scroll() {
    let snap = wifi_list_snapshot();
    let count = snap.subpanel_item_count(ControlCenterPage::Wifi);
    let layout = compute_control_center_layout_for_snapshot(
        &ControlCenterMetrics::DEFAULT,
        ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1750,
            anchor_y: 36,
            page: ControlCenterPage::Wifi,
            item_count: count,
            scroll_offset: 9999,
        },
        &snap,
    );
    assert_eq!(count, 14);
    assert!(layout.max_scroll > 0);
    assert_eq!(layout.scroll_offset, layout.max_scroll);
    let last_idx = count - 1;
    let last = layout.items[last_idx];
    assert!(last.bottom() <= layout.list_viewport.bottom());
    assert_eq!(
        layout.hit_test(last.x + 12, last.y + 12),
        Some(ControlCenterHitTarget::SubPanelItem(last_idx))
    );
}

#[test]
fn actions_keep_controls_open_except_external_handoffs() {
    let mut snap = ControlCenterSnapshot {
        page: ControlCenterPage::Wifi,
        ..ControlCenterSnapshot::default()
    };
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::MediaPlayPause),
        None
    );
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::MediaBody),
        Some(ControlCenterClickOutcome {
            command: ControlCenterCommand::OpenSettingsWindow(SettingsPage::Notch),
            dismiss: true,
        })
    );
    snap.settings.system.media_module = true;
    snap.settings.plugins.notch = true;
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::MediaPlayPause).map(|o| o.dismiss),
        Some(false)
    );
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::MediaBody).map(|o| o.dismiss),
        Some(true)
    );
    snap.settings.plugins.notch = false;
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::MediaBody),
        Some(ControlCenterClickOutcome {
            command: ControlCenterCommand::OpenSettingsWindow(SettingsPage::Notch),
            dismiss: true,
        })
    );
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::FooterPrimary).map(|o| o.dismiss),
        Some(true)
    );
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::FooterSecondary),
        None
    );
    snap.page = ControlCenterPage::Main;
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::WifiToggle),
        None
    );
    snap.wifi.supported = true;
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::WifiToggle),
        Some(ControlCenterClickOutcome {
            command: ControlCenterCommand::ToggleWifi,
            dismiss: false,
        })
    );
}

#[test]
fn release_tag_parser_ignores_nested_objects_and_braces_in_strings() {
    let json = r#"{
        "author": { "tag_name": "v9.9.9" },
        "body": "notes with {braces} and \"quotes\"",
        "tag_name": "v0.2.1"
    }"#;
    assert_eq!(parse_latest_release_tag(json).as_deref(), Some("v0.2.1"));
    assert_eq!(parse_latest_release_tag(r#"{"tag_name":"   "}"#), None);
    assert_eq!(parse_latest_release_tag("{invalid"), None);
}

#[test]
fn update_status_reports_newer_matching_unreleased_and_errors() {
    assert_eq!(
        format_update_status("0.1.0", 200, Some(r#"{"tag_name":"v0.2.0"}"#)),
        "Update v0.2.0"
    );
    assert_eq!(
        format_update_status("0.1.0", 200, Some(r#"{"tag_name":"v0.1.0"}"#)),
        "Up to date"
    );
    assert_eq!(
        format_update_status("0.2.0", 200, Some(r#"{"tag_name":"v0.1.5"}"#)),
        "Up to date"
    );
    assert_eq!(
        format_update_status("0.1.0-beta.1", 200, Some(r#"{"tag_name":"v0.1.0"}"#)),
        "Update v0.1.0"
    );
    assert_eq!(
        format_update_status("0.1.0", 200, Some(r#"{"tag_name":"v0.1.0-rc.1"}"#)),
        "Up to date"
    );
    assert_eq!(format_update_status("0.1.0", 404, None), "No releases yet");
    assert_eq!(format_update_status("0.1.0", 502, None), "Check failed");
    assert_eq!(
        format_update_status("0.1.0", 200, Some(r#"{"name":"no tag"}"#)),
        "Check failed"
    );
}

#[test]
fn focus_targets_respect_page_and_capabilities() {
    let mut snap = ControlCenterSnapshot::default();
    let layout = compute_control_center_layout(
        &ControlCenterMetrics::DEFAULT,
        ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1700,
            anchor_y: 34,
            page: ControlCenterPage::Main,
            item_count: 0,
            scroll_offset: 0,
        },
    );
    let limited = super::control_center_focus_targets(&layout, &snap);
    assert!(!limited.contains(&ControlCenterHitTarget::WifiToggle));
    assert!(!limited.contains(&ControlCenterHitTarget::BluetoothToggle));
    assert!(!limited.contains(&ControlCenterHitTarget::DisplaySlider));
    assert!(limited.contains(&ControlCenterHitTarget::SoundSlider));
    assert_eq!(limited.last(), Some(&ControlCenterHitTarget::MediaBody));

    snap.wifi.supported = true;
    snap.bluetooth.supported = true;
    snap.status.brightness = Some(55);
    let full = super::control_center_focus_targets(&layout, &snap);
    assert_eq!(full.first(), Some(&ControlCenterHitTarget::WifiToggle));
    assert!(full.contains(&ControlCenterHitTarget::BluetoothToggle));
    assert!(full.contains(&ControlCenterHitTarget::DisplaySlider));

    snap.page = ControlCenterPage::Display;
    let display_layout = compute_control_center_layout(
        &ControlCenterMetrics::DEFAULT,
        ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1700,
            anchor_y: 34,
            page: ControlCenterPage::Display,
            item_count: 0,
            scroll_offset: 0,
        },
    );
    let display_targets = super::control_center_focus_targets(&display_layout, &snap);
    assert_eq!(
        display_targets,
        vec![
            ControlCenterHitTarget::BackButton,
            ControlCenterHitTarget::DisplaySlider,
            ControlCenterHitTarget::FooterPrimary,
            ControlCenterHitTarget::FooterSecondary,
        ]
    );
}

#[test]
fn control_center_focus_navigates_adjusts_sliders_and_preserves_activation() {
    use super::{ControlCenterFocus, ControlCenterFocusOutcome, SLIDER_KEY_STEP_PERCENT};
    use crate::bar::bar_focus::BarKey;

    let targets = [
        ControlCenterHitTarget::BackButton,
        ControlCenterHitTarget::SoundMuteToggle,
        ControlCenterHitTarget::SoundSlider,
        ControlCenterHitTarget::SubPanelItem(0),
        ControlCenterHitTarget::FooterPrimary,
    ];
    let mut focus = ControlCenterFocus::default();
    assert_eq!(
        focus.enter(&targets),
        Some(ControlCenterHitTarget::BackButton)
    );
    assert_eq!(
        focus.handle(BarKey::Previous, &targets),
        ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::FooterPrimary)
    );
    assert_eq!(
        focus.handle(BarKey::Down, &targets),
        ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::BackButton)
    );
    assert_eq!(
        focus.handle(BarKey::NextTab, &targets),
        ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SoundMuteToggle)
    );
    assert_eq!(
        focus.handle(BarKey::Activate, &targets),
        ControlCenterFocusOutcome::Activate(ControlCenterHitTarget::SoundMuteToggle)
    );
    assert_eq!(
        focus.current(),
        Some(ControlCenterHitTarget::SoundMuteToggle)
    );
    assert_eq!(
        focus.handle(BarKey::Next, &targets),
        ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SoundSlider)
    );
    assert_eq!(
        focus.handle(BarKey::Next, &targets),
        ControlCenterFocusOutcome::AdjustSlider {
            target: ControlCenterHitTarget::SoundSlider,
            delta: SLIDER_KEY_STEP_PERCENT,
        }
    );
    assert_eq!(
        focus.handle(BarKey::Previous, &targets),
        ControlCenterFocusOutcome::AdjustSlider {
            target: ControlCenterHitTarget::SoundSlider,
            delta: -SLIDER_KEY_STEP_PERCENT,
        }
    );
    assert_eq!(
        focus.handle(BarKey::Down, &targets),
        ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SubPanelItem(0))
    );
    assert_eq!(
        focus.handle(BarKey::Up, &targets),
        ControlCenterFocusOutcome::Moved(ControlCenterHitTarget::SoundSlider)
    );
    assert_eq!(
        focus.handle(BarKey::Exit, &targets),
        ControlCenterFocusOutcome::Exited
    );
    assert!(!focus.is_active());
}

#[test]
fn focus_scroll_reveals_offscreen_subpanel_items_and_steps_slider_safely() {
    let snap = wifi_list_snapshot();
    let place = |scroll_offset| ControlCenterPlacement {
        bounds: bounds(),
        dpi: 96,
        anchor_right: 1700,
        anchor_y: 34,
        page: ControlCenterPage::Wifi,
        item_count: 14,
        scroll_offset,
    };
    let mut layout =
        compute_control_center_layout_for_snapshot(&ControlCenterMetrics::DEFAULT, place(0), &snap);
    assert_eq!(
        super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(0)),
        None
    );
    let scroll_down =
        super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(13))
            .expect("bottom item should scroll into view");
    assert!(scroll_down > 0 && scroll_down <= layout.max_scroll);

    layout = compute_control_center_layout_for_snapshot(
        &ControlCenterMetrics::DEFAULT,
        place(scroll_down),
        &snap,
    );
    assert_eq!(
        super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(13)),
        None
    );
    assert_eq!(
        super::scroll_offset_for_item(&layout, ControlCenterHitTarget::SubPanelItem(0)),
        Some(0)
    );
    assert_eq!(
        super::scroll_offset_for_item(&layout, ControlCenterHitTarget::BackButton),
        None
    );
    assert_eq!(super::step_slider_percent(98, 5), 100);
    assert_eq!(super::step_slider_percent(2, -5), 0);
    assert_eq!(
        super::subpanel_opener_target(ControlCenterPage::Wifi),
        Some(ControlCenterHitTarget::WifiOpenPage)
    );
    assert_eq!(super::subpanel_opener_target(ControlCenterPage::Main), None);
}

#[test]
fn reserve_space_needs_the_dock_and_a_hidden_windows_taskbar() {
    let reserve = OttoSettingItem::Toggle(OttoSettingKind::DockReserveSpace);
    let mut settings = OttoSettingsSnapshot::default();
    assert!(!settings.is_item_available(reserve));
    settings.plugins.dock = true;
    settings.dock.hide_taskbar = false;
    assert!(!settings.is_item_available(reserve));
    settings.dock.hide_taskbar = true;
    assert!(settings.is_item_available(reserve));
    settings.dock.reserve_space = true;
    assert!(settings.is_enabled(OttoSettingKind::DockReserveSpace));
}

#[test]
fn menu_bar_items_report_their_visibility_from_the_hidden_set() {
    let mut snap = ControlCenterSnapshot::default();
    snap.settings.plugins.top_bar = true;
    assert!(snap.settings.is_enabled(OttoSettingKind::MenuBarClock));
    assert!(snap.settings.is_enabled(OttoSettingKind::MenuBarBackground));
    snap.settings.menu_bar.hidden_items =
        snap.settings.menu_bar.hidden_items.toggled(BarItem::Clock);
    assert!(!snap.settings.is_enabled(OttoSettingKind::MenuBarClock));
    assert!(snap.settings.is_enabled(OttoSettingKind::MenuBarTray));
}

#[test]
fn focus_row_separates_pomodoro_toggle_and_windows_dnd_settings_trigger() {
    assert_eq!(focus_row_subtitle(false, false), "Disabled");
    assert_eq!(focus_row_subtitle(false, true), "Disabled");
    assert_eq!(focus_row_subtitle(true, false), "Off");
    assert_eq!(focus_row_subtitle(true, true), "Focus On");

    let mut snap = ControlCenterSnapshot::default();
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::FocusToggle),
        Some(ControlCenterClickOutcome {
            command: ControlCenterCommand::OpenSettingsWindow(SettingsPage::Notch),
            dismiss: true,
        })
    );
    snap.settings.system.focus_module = true;
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::FocusToggle),
        Some(ControlCenterClickOutcome {
            command: ControlCenterCommand::OpenSettingsWindow(SettingsPage::Notch),
            dismiss: true,
        })
    );
    snap.settings.plugins.notch = true;
    assert_eq!(
        control_center_action(&snap, ControlCenterHitTarget::FocusToggle),
        Some(ControlCenterClickOutcome {
            command: ControlCenterCommand::ToggleFocus,
            dismiss: false,
        })
    );
    let layout = main_layout(96);
    assert!(layout.focus_toggle.contains(
        layout.focus_settings.x + layout.focus_settings.width / 2,
        layout.focus_settings.y + layout.focus_settings.height / 2
    ));
    assert!(layout.focus_settings.width >= 20);
}

#[test]
fn control_center_shell_and_cards_are_concentric() {
    let metrics = ControlCenterMetrics::DEFAULT;
    assert_eq!(metrics.shell_radius, metrics.card_radius + metrics.padding);
    let layout = main_layout(96);
    assert_eq!(layout.shell_radius, 26);
    assert_eq!(layout.card_radius, 14);
    assert_eq!(layout.connectivity_card.x, 12);
    assert_eq!(layout.connectivity_card.y, 12);
}
