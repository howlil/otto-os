use crate::bar::menu_bar::BarItem;
use crate::layout::{Rect, scale};
use crate::settings::model::*;
use crate::settings_window::SettingsPage;

use super::actions::*;
use super::commands::*;
use super::layout::*;
use super::layout_types::*;
use super::model::*;
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
    compute_control_center_layout_for_snapshot(
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
        &ControlCenterSnapshot::default(),
    )
}

#[test]
fn main_layout_places_window_within_work_area_across_dpis() {
    for dpi in [96, 144, 192] {
        let layout = main_layout(dpi);
        assert_eq!(layout.window, bounds());
        assert!(layout.canvas_panel.width > 0 && layout.canvas_panel.height > 0);
        assert!(layout.canvas_panel.right() <= bounds().right());
        assert!(layout.canvas_panel.bottom() <= bounds().bottom());
        assert!(layout.connectivity_card.width > 0);
        assert_eq!(layout.focus_toggle.width, layout.battery_tile.width);
        assert_eq!(layout.focus_toggle.height, layout.battery_tile.height);
        assert!(layout.display_slider.height > layout.display_slider.width);
        assert!(layout.sound_slider.height > layout.sound_slider.width);
        assert_eq!(layout.sound_slider.height, layout.display_slider.height);
        assert_eq!(layout.sound_mute.width, layout.sound_slider.width);
        assert_eq!(layout.sound_mute.bottom(), layout.sound_slider.bottom());
        assert!(layout.sound_mute.height >= scale(24, dpi));
        assert!(layout.media_body.width > 0);
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
    assert_eq!(
        layout.hit_test(mx, layout.sound_slider.y + 12),
        Some(ControlCenterHitTarget::SoundSlider)
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
    assert_eq!(
        layout.hit_test(8, 8),
        Some(ControlCenterHitTarget::CanvasBackground)
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
fn release_update_parser_extracts_verified_assets_and_rejects_untrusted_urls() {
    let json = r#"{
        "html_url": "https://github.com/howlil/otto-os/releases/tag/v1.2.1",
        "assets": [
            {
                "name": "otto-os-1.2.1-windows-x64.zip",
                "uploader": { "name": "bot", "browser_download_url": "https://evil.example/a.exe" },
                "browser_download_url": "https://github.com/howlil/otto-os/releases/download/v1.2.1/otto-os-1.2.1-windows-x64.zip"
            },
            {
                "name": "otto-os-setup-1.2.1-x64.exe",
                "uploader": { "login": "github-actions" },
                "browser_download_url": "https://github.com/howlil/otto-os/releases/download/v1.2.1/otto-os-setup-1.2.1-x64.exe"
            },
            {
                "name": "SHA256SUMS.txt",
                "browser_download_url": "https://github.com/howlil/otto-os/releases/download/v1.2.1/SHA256SUMS.txt"
            }
        ],
        "tag_name": "v1.2.1"
    }"#;
    let parsed = parse_release_update("1.2.0", 200, Some(json));
    let UpdateState::Available(info) = parsed else {
        panic!("expected Available, got {parsed:?}");
    };
    assert_eq!(info.version, "1.2.1");
    assert_eq!(info.tag, "v1.2.1");
    assert_eq!(info.installer_name, "otto-os-setup-1.2.1-x64.exe");
    assert!(info.can_auto_install());
    assert_eq!(
        info.release_url,
        "https://github.com/howlil/otto-os/releases/tag/v1.2.1"
    );

    let untrusted = r#"{
        "tag_name": "v1.2.1",
        "html_url": "https://evil.example/phish",
        "assets": [
            {
                "name": "otto-os-setup-1.2.1-x64.exe",
                "browser_download_url": "https://evil.example/otto-os-setup-1.2.1-x64.exe"
            },
            {
                "name": "SHA256SUMS.txt",
                "browser_download_url": "https://github.com/howlil/otto-os/releases/download/v1.2.1/SHA256SUMS.txt"
            }
        ]
    }"#;
    let UpdateState::Available(blocked) = parse_release_update("1.2.0", 200, Some(untrusted))
    else {
        panic!("expected Available");
    };
    assert!(!blocked.can_auto_install());
    assert_eq!(
        blocked.release_url,
        "https://github.com/howlil/otto-os/releases"
    );
    assert!(!is_allowed_release_asset_url(
        "https://github.com/howlil/otto-os/releases/download/../evil/otto-os-setup-1.2.1-x64.exe",
        "../evil",
        "otto-os-setup-1.2.1-x64.exe"
    ));
    assert_eq!(
        expected_installer_asset_name("v1.2.1-beta.2").as_deref(),
        Some("otto-os-setup-1.2.1-x64.exe")
    );
}

#[test]
fn focus_targets_respect_page_and_capabilities() {
    let mut snap = ControlCenterSnapshot::default();
    let layout = compute_control_center_layout_for_snapshot(
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
        &snap,
    );
    let limited = super::control_center_focus_targets(&layout, &snap);
    assert!(!limited.contains(&ControlCenterHitTarget::WifiToggle));
    assert!(!limited.contains(&ControlCenterHitTarget::BluetoothToggle));
    assert!(!limited.contains(&ControlCenterHitTarget::DisplaySlider));
    assert!(limited.contains(&ControlCenterHitTarget::SoundSlider));
    assert!(limited.contains(&ControlCenterHitTarget::MediaBody));
    assert_eq!(
        limited.last(),
        Some(&ControlCenterHitTarget::TileAction(10))
    );

    snap.wifi.supported = true;
    snap.bluetooth.supported = true;
    snap.status.brightness = Some(55);
    let full = super::control_center_focus_targets(&layout, &snap);
    assert_eq!(full.first(), Some(&ControlCenterHitTarget::WifiToggle));
    assert!(full.contains(&ControlCenterHitTarget::BluetoothToggle));
    assert!(full.contains(&ControlCenterHitTarget::DisplaySlider));

    snap.page = ControlCenterPage::Display;
    let display_layout = compute_control_center_layout_for_snapshot(
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
        &snap,
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
    assert!(layout.focus_toggle.width > 0 && layout.focus_toggle.height > 0);
}

#[test]
fn control_center_shell_and_cards_are_concentric() {
    let metrics = ControlCenterMetrics::DEFAULT;
    assert_eq!(metrics.shell_radius, metrics.card_radius + metrics.padding);
    let layout = main_layout(96);
    assert_eq!(layout.shell_radius, 26);
    assert_eq!(layout.card_radius, 14);
    assert_eq!(layout.connectivity_card.x, layout.canvas_panel.x);
    assert_eq!(layout.connectivity_card.y, layout.grid_origin.1);
}

#[test]
fn modular_grid_resizes_moves_adds_removes_and_persists_cleanly() {
    use super::grid::{ControlCenterGrid, ControlKind};

    let mut grid = ControlCenterGrid::default_ipad_grid();
    assert!(grid.contains_kind(ControlKind::Connectivity));
    assert!(grid.contains_kind(ControlKind::Brightness));
    assert!(!grid.contains_kind(ControlKind::Wifi));

    let wifi_id = grid.add_control(ControlKind::Wifi).expect("add wifi");
    assert!(grid.contains_kind(ControlKind::Wifi));
    assert!(grid.add_control(ControlKind::Wifi).is_none());

    assert!(grid.resize_tile(wifi_id, 2, 1));
    assert_eq!(grid.tile(wifi_id).map(|t| (t.cols, t.rows)), Some((2, 1)));
    for (idx, a) in grid.tiles.iter().enumerate() {
        for b in grid.tiles.iter().skip(idx + 1) {
            assert!(!a.overlaps(b), "tiles {a:?} and {b:?} overlap");
        }
    }

    assert!(grid.move_tile(wifi_id, 0, 0));
    for (idx, a) in grid.tiles.iter().enumerate() {
        for b in grid.tiles.iter().skip(idx + 1) {
            assert!(!a.overlaps(b), "tiles {a:?} and {b:?} overlap after move");
        }
    }

    let serialized = grid.format_config();
    let parsed = ControlCenterGrid::parse_config(&serialized);
    assert_eq!(parsed.tiles.len(), grid.tiles.len());
    assert!(ControlCenterGrid::is_future_version("version=2\n"));
    assert!(!ControlCenterGrid::is_future_version("version=1\n"));

    assert!(grid.remove_tile(wifi_id));
    assert!(!grid.contains_kind(ControlKind::Wifi));
    grid.reset();
    assert_eq!(grid.tiles.len(), ControlCenterGrid::default().tiles.len());
}

#[test]
fn vertical_and_horizontal_sliders_and_edit_mode_hit_testing_work() {
    let layout = main_layout(96);
    let r = layout.display_slider;
    assert_eq!(
        layout.slider_value_at_point(ControlCenterHitTarget::DisplaySlider, r.x + 10, r.bottom()),
        Some(0)
    );
    assert_eq!(
        layout.slider_value_at_point(ControlCenterHitTarget::DisplaySlider, r.x + 10, r.y),
        Some(100)
    );
    let mid_pct = layout
        .slider_value_at_point(
            ControlCenterHitTarget::DisplaySlider,
            r.x + 10,
            r.y + r.height / 2,
        )
        .unwrap();
    assert!((48..=52).contains(&mid_pct));

    let mut snap = ControlCenterSnapshot {
        editing: true,
        ..ControlCenterSnapshot::default()
    };
    let edit_layout = compute_control_center_layout_for_snapshot(
        &ControlCenterMetrics::DEFAULT,
        ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1750,
            anchor_y: 36,
            page: ControlCenterPage::Main,
            item_count: 0,
            scroll_offset: 0,
        },
        &snap,
    );
    let first = edit_layout.tiles[0];
    assert_eq!(
        edit_layout.hit_test_with_editing(first.remove_badge.x + 4, first.remove_badge.y + 4, true),
        Some(ControlCenterHitTarget::TileRemove(first.id))
    );
    assert_eq!(
        edit_layout.hit_test_with_editing(
            first.resize_handle.right() - 4,
            first.resize_handle.bottom() - 4,
            true
        ),
        Some(ControlCenterHitTarget::TileResize(first.id))
    );
    assert_eq!(
        edit_layout.hit_test_with_editing(
            first.rect.x + first.rect.width / 2,
            first.rect.y + first.rect.height / 2,
            true
        ),
        Some(ControlCenterHitTarget::TileMove(first.id))
    );
    snap.page = ControlCenterPage::Gallery;
    let gallery_layout = compute_control_center_layout_for_snapshot(
        &ControlCenterMetrics::DEFAULT,
        ControlCenterPlacement {
            bounds: bounds(),
            dpi: 96,
            anchor_right: 1750,
            anchor_y: 36,
            page: ControlCenterPage::Gallery,
            item_count: snap.subpanel_item_count(ControlCenterPage::Gallery),
            scroll_offset: 0,
        },
        &snap,
    );
    assert_eq!(gallery_layout.items.len(), 17);
    assert!(gallery_layout.canvas_panel.x > 1000);
}
