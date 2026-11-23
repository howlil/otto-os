use crate::bar::menu_bar::{BarItem, BarItemSet};
use crate::notch::clipboard::ClipboardLayoutInput;
use crate::notch::island::CompactSlotKind;
use crate::notch::media::NotchExpandedTab;
use crate::notch::pomodoro_view::PomodoroSurface;
use crate::notch::tasks::TasksLayoutInput;
use crate::preferences::NotchStyle;

use super::compact::*;
use super::compute::*;
use super::geometry::*;
use super::hit::*;
use super::input::*;
use super::metrics::*;
use super::model::*;
use super::notification::*;
use super::panel::*;
use super::slots::*;

const M: Metrics = Metrics::MACBOOK_PRO_14;

#[test]
fn bar_height_equals_notch_height_and_spans_screen() {
    let layout = compute_layout(&M, 1920, 96);
    assert_eq!(
        layout.bar,
        Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 32
        }
    );
    assert_eq!(layout.notch.height, layout.bar.height);
}

#[test]
fn notch_is_centered_with_ears_and_bottom_radius() {
    let layout = compute_layout(&M, 1920, 96);
    assert_eq!(layout.notch.x, 860);
    assert_eq!(layout.notch.width, 200);
    assert_eq!(layout.notch_corner_radius, 10);
    assert_eq!(layout.notch_ear_radius, 6);
}

#[test]
fn expanded_notch_uses_apple_dynamic_island_dimensions() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            panel_expansion: 1.0,
            pomodoro_surface: PomodoroSurface::Running,
            active_tab: NotchExpandedTab::Pomodoro,
            control_center_enabled: true,
            lyrics_progress: 0.0,
            tab_blend: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(layout.bar.height, 32);
    assert_eq!(layout.notch.width, 416);
    assert_eq!(layout.notch.height, 144);
    assert_eq!(layout.notch_corner_radius, 22);
    assert_eq!(layout.notch_ear_radius, 8);
    assert_eq!(
        layout.hit_test(
            layout.pomodoro.play_button.x + 10,
            layout.pomodoro.play_button.y + 10
        ),
        Some(HitTarget::PomodoroPlayPause)
    );
    assert_eq!(
        layout.hit_test(
            layout.pomodoro.reset_button.x + 8,
            layout.pomodoro.reset_button.y + 8
        ),
        Some(HitTarget::PomodoroReset)
    );
    assert_eq!(
        layout.hit_test(
            layout.pomodoro.skip_button.x + 8,
            layout.pomodoro.skip_button.y + 8
        ),
        Some(HitTarget::PomodoroSkip)
    );
    assert_eq!(
        layout.hit_test(
            layout.notch_tab_pomodoro.x + 8,
            layout.notch_tab_pomodoro.y + 8
        ),
        Some(HitTarget::NotchTabPomodoro)
    );
}

#[test]
fn expanded_notch_supports_media_tab_and_playback_controls() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            panel_expansion: 1.0,
            pomodoro_surface: PomodoroSurface::Running,
            active_tab: NotchExpandedTab::Media,
            control_center_enabled: true,
            lyrics_progress: 0.0,
            tab_blend: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(
        layout.hit_test(
            layout.notch_tab_pomodoro.x + 8,
            layout.notch_tab_pomodoro.y + 8,
        ),
        Some(HitTarget::NotchTabPomodoro)
    );
    assert_eq!(
        layout.hit_test(layout.notch_tab_media.x + 8, layout.notch_tab_media.y + 8,),
        Some(HitTarget::NotchTabMedia)
    );
    assert_eq!(
        layout.hit_test(
            layout.media.play_button.x + 8,
            layout.media.play_button.y + 8,
        ),
        Some(HitTarget::MediaPlayPause)
    );
    assert_eq!(
        layout.hit_test(
            layout.media.prev_button.x + 8,
            layout.media.prev_button.y + 8,
        ),
        Some(HitTarget::MediaPrevious)
    );
    assert_eq!(
        layout.hit_test(
            layout.media.next_button.x + 8,
            layout.media.next_button.y + 8,
        ),
        Some(HitTarget::MediaNext)
    );
    assert_eq!(
        layout.hit_test(layout.media.seekbar.x + 8, layout.media.seekbar.y + 4,),
        Some(HitTarget::MediaSeekbar)
    );
    assert!(layout.media.cover_art.bottom() <= layout.media.seekbar.y);
    assert!(layout.media.play_button.bottom() < layout.media.seekbar.y);
    assert!(layout.media.prev_button.right() < layout.media.play_button.x);
    assert!(layout.media.play_button.right() < layout.media.next_button.x);
    assert_eq!(layout.notch.right() - layout.media.next_button.right(), 16);
    let hero_center = layout.media.cover_art.y + layout.media.cover_art.height / 2;
    for button in [
        layout.media.prev_button,
        layout.media.play_button,
        layout.media.next_button,
    ] {
        assert_eq!(button.y + button.height / 2, hero_center);
    }
    assert!(layout.media.lyrics_toggle.bottom() <= layout.media.cover_art.y);
    assert_eq!(
        layout.notch.right() - layout.media.lyrics_toggle.right(),
        16
    );
    assert!(layout.media.source_label.right() < layout.media.lyrics_toggle.x);
}

#[test]
fn media_hero_shares_the_pomodoro_grid() {
    let media = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            pomodoro_surface: PomodoroSurface::Running,
            active_tab: NotchExpandedTab::Media,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(media.media.cover_art, media.pomodoro.icon);
    assert_eq!(media.media.prev_button, media.pomodoro.reset_button);
    assert_eq!(media.media.play_button, media.pomodoro.play_button);
    assert_eq!(media.media.next_button, media.pomodoro.skip_button);
    assert_eq!(media.media.title.x, media.pomodoro.time.x);
    assert_eq!(media.media.title.width, media.pomodoro.time.width);
    assert!(media.media.title.bottom() <= media.media.artist.y);
    assert!(media.media.artist.bottom() <= media.media.equalizer.y);
    assert!(media.media.reason.contains_rect(&media.media.artist));
    assert!(media.media.equalizer.bottom() <= media.media.cover_art.bottom());
    assert!(media.media.reason.right() < media.media.prev_button.x);
}

#[test]
fn lyrics_progress_interpolates_panel_drawer_and_seekbar_between_closed_and_open() {
    let layout_at = |lyrics_progress: f32| {
        compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                control_center_enabled: true,
                lyrics_progress,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        )
    };
    let closed = layout_at(0.0);
    let mid = layout_at(0.5);
    let open = layout_at(1.0);
    assert!(closed.notch.height < mid.notch.height && mid.notch.height < open.notch.height);
    assert!(
        closed.media.lyrics_drawer.height < mid.media.lyrics_drawer.height
            && mid.media.lyrics_drawer.height < open.media.lyrics_drawer.height
    );
    assert!(
        closed.media.seekbar.y < mid.media.seekbar.y && mid.media.seekbar.y < open.media.seekbar.y
    );
    assert_eq!(open.notch.height, 254);
    assert_eq!(closed.notch.height, 144);
    assert!((layout_at(2.0).lyrics_progress - 1.0).abs() < f32::EPSILON);
}

#[test]
fn expanded_media_controls_meet_target_size_and_fit_notch() {
    for lyrics_progress in [0.0, 1.0] {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                pomodoro_surface: PomodoroSurface::Running,
                active_tab: NotchExpandedTab::Media,
                control_center_enabled: true,
                lyrics_progress,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert!(layout.media.lyrics_toggle.width >= 24);
        assert!(layout.media.lyrics_toggle.height >= 24);
        assert!(layout.media.seekbar.height >= 24);
        assert!(layout.media.prev_button.width >= 24);
        assert!(layout.media.next_button.width >= 24);
        assert!(layout.media.play_button.width >= 24);
        assert!(layout.media.play_button.bottom() <= layout.notch.height - 4);
        assert!(layout.media.cover_art.bottom() <= layout.media.seekbar.y);
    }
}

#[test]
fn expanded_notch_supports_lyrics_drawer_and_toggle() {
    let layout_collapsed = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            panel_expansion: 1.0,
            pomodoro_surface: PomodoroSurface::Running,
            active_tab: NotchExpandedTab::Media,
            control_center_enabled: true,
            lyrics_progress: 0.0,
            tab_blend: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(layout_collapsed.notch.height, 144);
    assert_eq!(layout_collapsed.media.lyrics_drawer.height, 0);
    assert_eq!(
        layout_collapsed.hit_test(
            layout_collapsed.media.lyrics_toggle.x + 4,
            layout_collapsed.media.lyrics_toggle.y + 4,
        ),
        Some(HitTarget::MediaLyricsToggle)
    );

    let layout_expanded = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            hud_expansion: 0.0,
            compact_expansion: 0.0,
            panel_expansion: 1.0,
            pomodoro_surface: PomodoroSurface::Running,
            active_tab: NotchExpandedTab::Media,
            control_center_enabled: true,
            lyrics_progress: 1.0,
            tab_blend: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(layout_expanded.notch.height, 254);
    assert!(layout_expanded.media.lyrics_drawer.height > 100);
    assert!(layout_expanded.media.cover_art.bottom() <= layout_expanded.media.lyrics_drawer.y);
    assert!(layout_expanded.media.lyrics_drawer.bottom() <= layout_expanded.media.seekbar.y);
    assert!(layout_expanded.media.play_button.bottom() <= layout_expanded.media.lyrics_drawer.y);
    assert!(layout_expanded.media.seekbar.bottom() <= layout_expanded.notch.height - 16);
}

#[test]
fn compact_activities_use_style_specific_geometry() {
    let focus = NotchLayoutInput {
        hud_expansion: 0.0,
        compact_expansion: 1.0,
        compact_primary: Some(CompactSlotKind::Focus),
        panel_expansion: 0.0,
        pomodoro_surface: PomodoroSurface::Running,
        active_tab: NotchExpandedTab::Pomodoro,
        control_center_enabled: true,
        lyrics_progress: 0.0,
        tab_blend: 1.0,
        compact_title_points: 120,
        ..NotchLayoutInput::default()
    };
    let classic_focus = compute_layout_with_notch(&M, 1920, 96, "Finder", focus);
    assert_eq!(classic_focus.notch.width, 200);
    assert_eq!(classic_focus.notch.height, 32);

    let island_focus =
        compute_layout_with_notch(&Metrics::DYNAMIC_ISLAND, 1920, 96, "Finder", focus);
    assert_eq!(island_focus.notch.width, 184);
    assert_eq!(island_focus.notch.height, 36);
    assert_eq!(island_focus.notch.y, 2);
    assert_eq!(island_focus.notch_corner_radius, 18);
    assert!(island_focus.notch.contains_rect(&island_focus.compact_ring));
    assert!(island_focus.notch.contains_rect(&island_focus.compact_time));
    assert!(
        island_focus
            .compact_cycle_steps
            .iter()
            .all(|step| island_focus.notch.contains_rect(step))
    );

    let music = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            compact_expansion: 1.0,
            compact_primary: Some(CompactSlotKind::Media),
            active_tab: NotchExpandedTab::Media,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(music.notch.width, 292);
    assert_eq!(music.notch.height, 36);
    assert_eq!(music.notch.y, 2);

    let expanded_focus = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            ..focus
        },
    );
    assert_eq!(expanded_focus.notch.y, 2);
    assert!(expanded_focus.panel_window.height >= expanded_focus.notch.bottom() + 4);
}

#[test]
fn dimensions_scale_with_dpi() {
    let layout = compute_layout(&M, 3840, 192);
    assert_eq!(layout.bar.height, 64);
    assert_eq!(layout.notch.width, 400);
    assert_eq!(layout.notch_ear_radius, 12);
    assert_eq!(scale(10, 144), 15);
}

#[test]
fn start_is_left_and_tray_status_clock_are_right_of_notch() {
    let layout = compute_layout_for_app(&M, 1920, 96, "Finder");
    assert!(layout.slots.start_slot.right() <= layout.slots.app_slot.x);
    assert!(layout.slots.app_slot.right() <= layout.slots.file_slot.x);
    assert!(layout.slots.file_slot.right() <= layout.slots.edit_slot.x);
    assert!(layout.slots.edit_slot.right() <= layout.slots.view_slot.x);
    assert!(layout.slots.view_slot.right() <= layout.slots.window_slot.x);
    assert!(layout.slots.window_slot.right() <= layout.slots.help_slot.x);
    assert!(layout.slots.help_slot.right() < layout.notch.x);
    assert!(layout.slots.tray_slot.x >= layout.notch.right());
    assert!(layout.slots.status_slot.x >= layout.slots.tray_slot.right());
    assert!(layout.slots.search_slot.x >= layout.slots.status_slot.right());
    assert!(layout.slots.control_center_slot.x >= layout.slots.search_slot.right());
    assert!(layout.slots.notification_slot.x >= layout.slots.control_center_slot.right());
    assert!(layout.slots.clock_slot.x >= layout.slots.notification_slot.right());
    assert!(layout.slots.clock_slot.right() <= layout.bar.width);
}

#[test]
fn hit_test_identifies_each_slot() {
    let layout = compute_layout(&M, 1920, 96);
    assert_eq!(
        layout.hit_test(layout.slots.start_slot.x + 4, 16),
        Some(HitTarget::Start)
    );
    assert_eq!(
        layout.hit_test(layout.slots.app_slot.x + 4, 16),
        Some(HitTarget::AppMenu)
    );
    assert_eq!(
        layout.hit_test(layout.slots.file_slot.x + 4, 16),
        Some(HitTarget::FileMenu)
    );
    assert_eq!(
        layout.hit_test(layout.slots.edit_slot.x + 4, 16),
        Some(HitTarget::EditMenu)
    );
    assert_eq!(
        layout.hit_test(layout.slots.view_slot.x + 4, 16),
        Some(HitTarget::ViewMenu)
    );
    assert_eq!(
        layout.hit_test(layout.slots.window_slot.x + 4, 16),
        Some(HitTarget::WindowMenu)
    );
    assert_eq!(
        layout.hit_test(layout.slots.help_slot.x + 4, 16),
        Some(HitTarget::HelpMenu)
    );
    assert_eq!(
        layout.hit_test(layout.notch.x + 50, 16),
        Some(HitTarget::Notch)
    );
    assert_eq!(
        layout.hit_test(layout.slots.tray_slot.x + 4, 16),
        Some(HitTarget::Tray)
    );
    assert_eq!(
        layout.hit_test(layout.slots.status_wifi_rect.x + 8, 16),
        Some(HitTarget::StatusWifi)
    );
    assert_eq!(
        layout.hit_test(layout.slots.status_volume_rect.x + 8, 16),
        Some(HitTarget::StatusVolume)
    );
    assert_eq!(
        layout.hit_test(layout.slots.status_battery_rect.x + 8, 16),
        Some(HitTarget::StatusBattery)
    );
    assert_eq!(
        layout.hit_test(layout.slots.search_slot.x + 4, 16),
        Some(HitTarget::Search)
    );
    assert_eq!(
        layout.hit_test(layout.slots.control_center_slot.x + 4, 16),
        Some(HitTarget::ControlCenter)
    );
    assert_eq!(
        layout.hit_test(layout.slots.notification_slot.x + 4, 16),
        Some(HitTarget::Notifications)
    );
    assert_eq!(
        layout.hit_test(layout.slots.clock_slot.x + 10, 16),
        Some(HitTarget::Clock)
    );
    assert_eq!(layout.hit_test(750, 16), None);
}

#[test]
fn status_slot_splits_into_disjoint_icon_rects_across_dpis() {
    for dpi in [96, 144, 192] {
        let layout = compute_layout(&M, 1920, dpi);
        let wifi = layout.slots.status_wifi_rect;
        let vol = layout.slots.status_volume_rect;
        let bat = layout.slots.status_battery_rect;
        assert!(wifi.width > 0 && vol.width > 0 && bat.width > 0);
        assert_eq!(wifi.x, layout.slots.status_slot.x);
        assert_eq!(wifi.right(), vol.x);
        assert_eq!(vol.right(), bat.x);
        assert_eq!(bat.right(), layout.slots.status_slot.right());
        let cy = layout.slots.status_slot.height / 2;
        assert_eq!(
            layout.hit_test(wifi.x + wifi.width / 2, cy),
            Some(HitTarget::StatusWifi)
        );
        assert_eq!(
            layout.hit_test(vol.x + vol.width / 2, cy),
            Some(HitTarget::StatusVolume)
        );
        assert_eq!(
            layout.hit_test(bat.x + bat.width / 2, cy),
            Some(HitTarget::StatusBattery)
        );
    }
}

const EVERY_SLOT: [HitTarget; 13] = [
    HitTarget::Start,
    HitTarget::AppMenu,
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
    HitTarget::Tray,
    HitTarget::QuickSettings,
    HitTarget::Search,
    HitTarget::ControlCenter,
    HitTarget::Notifications,
    HitTarget::Clock,
];

fn assert_every_item_reachable(layout: &Layout) {
    let mut visible: Vec<Rect> = Vec::new();
    for target in EVERY_SLOT {
        let rect = layout.slot_rect(target).unwrap();
        if layout.is_hidden(target) {
            assert_eq!(rect.width, 0, "{target:?} is hidden but still has width");
            assert!(
                layout.slots.overflow_slot.width > 0,
                "{target:?} has no overflow"
            );
            continue;
        }
        assert!(
            rect.width > 0,
            "{target:?} is neither visible nor in overflow"
        );
        let hit = layout.hit_test(rect.x + rect.width / 2, rect.height / 2);
        if target == HitTarget::QuickSettings {
            assert!(matches!(
                hit,
                Some(
                    HitTarget::QuickSettings
                        | HitTarget::StatusWifi
                        | HitTarget::StatusVolume
                        | HitTarget::StatusBattery
                )
            ));
        } else {
            assert_eq!(hit, Some(target));
        }
        visible.push(rect);
    }
    if layout.slots.overflow_slot.width > 0 {
        let slot = layout.slots.overflow_slot;
        assert_eq!(
            layout.hit_test(slot.x + slot.width / 2, slot.height / 2),
            Some(HitTarget::MenuOverflow)
        );
        visible.push(slot);
    }
    visible.push(Rect {
        height: layout.bar.height,
        ..layout.notch
    });
    visible.sort_by_key(|rect| rect.x);
    for pair in visible.windows(2) {
        assert!(
            pair[0].right() <= pair[1].x,
            "{:?} overlaps {:?}",
            pair[0],
            pair[1]
        );
    }
    assert!(visible.first().unwrap().x >= 0);
    assert!(visible.last().unwrap().right() <= layout.bar.width);
}

#[test]
fn wide_screens_show_every_slot_without_overflow() {
    for (width, dpi) in [(1920, 96), (2560, 144), (3840, 192)] {
        let layout = compute_layout_for_app(&M, width, dpi, "Finder");
        assert!(layout.slots.hidden_targets.is_empty(), "{width}@{dpi}");
        assert_eq!(layout.slots.overflow_slot.width, 0);
        assert_every_item_reachable(&layout);
    }
}

#[test]
fn small_and_high_scale_screens_keep_every_item_reachable() {
    for (width, dpi) in [
        (1366, 96),
        (1366, 144),
        (1280, 120),
        (1024, 96),
        (1024, 144),
        (800, 96),
        (1920, 240),
        (1280, 192),
    ] {
        for app_name in ["Finder", "Visual Studio Installer"] {
            let layout = compute_layout_for_app(&M, width, dpi, app_name);
            assert_every_item_reachable(&layout);
        }
    }
}

#[test]
fn widened_notch_never_overlaps_menu_slots() {
    for (width, dpi) in [(1366, 144), (1024, 96), (1920, 96)] {
        let layout = compute_layout_with_notch(
            &M,
            width,
            dpi,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 1.0,
                compact_expansion: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert_every_item_reachable(&layout);
    }
}

#[test]
fn hidden_items_anchor_their_menus_at_the_overflow_slot() {
    let layout = compute_layout_for_app(&M, 800, 96, "Finder");
    assert!(!layout.slots.hidden_targets.is_empty());
    let hidden = layout.slots.hidden_targets[0];
    assert_eq!(
        layout.menu_anchor_rect(hidden),
        Some(layout.slots.overflow_slot)
    );
    assert_eq!(
        layout.menu_anchor_rect(HitTarget::Start),
        Some(layout.slots.start_slot)
    );
}

fn pomodoro_layout(surface: PomodoroSurface, dpi: u32) -> Layout {
    compute_layout_with_notch(
        &M,
        1920,
        dpi,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            pomodoro_surface: surface,
            ..NotchLayoutInput::default()
        },
    )
}

fn pomodoro_hit_rects(layout: &Layout) -> Vec<Rect> {
    let mut rects = vec![
        layout.notch_tab_pomodoro,
        layout.notch_tab_stats,
        layout.notch_tab_shelf,
        layout.notch_tab_media,
    ];
    if layout.pomodoro.surface == PomodoroSurface::Alert {
        rects.push(layout.pomodoro.alert_extend_button);
        rects.push(layout.pomodoro.alert_start_button);
    } else {
        rects.extend(layout.pomodoro.focus_modes);
        rects.push(layout.pomodoro.play_button);
        rects.push(layout.pomodoro.reset_button);
        rects.push(layout.pomodoro.skip_button);
        rects.extend(layout.pomodoro.presets);
    }
    rects.retain(|rect| !rect.is_empty());
    rects
}

#[test]
fn pomodoro_controls_fit_the_notch_and_keep_their_gaps_at_every_dpi() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let minimum = scale(24, dpi);
        for surface in [
            PomodoroSurface::Setup,
            PomodoroSurface::Ready,
            PomodoroSurface::Running,
            PomodoroSurface::Paused,
            PomodoroSurface::Alert,
        ] {
            let layout = pomodoro_layout(surface, dpi);
            let rects = pomodoro_hit_rects(&layout);
            for (index, rect) in rects.iter().enumerate() {
                assert!(
                    layout.notch.contains_rect(rect),
                    "{surface:?}@{dpi}: {rect:?} leaves the notch"
                );
                assert!(
                    rect.width >= minimum && rect.height >= minimum,
                    "{surface:?}@{dpi}: {rect:?} is below the hit minimum"
                );
                assert!(
                    rect.bottom() <= layout.notch.height - scale(16, dpi) + 1
                        || rect.y < scale(40, dpi),
                    "{surface:?}@{dpi}: {rect:?} crosses the bottom inset"
                );
                for other in &rects[index + 1..] {
                    let separated_x =
                        rect.right() + gap <= other.x || other.right() + gap <= rect.x;
                    let separated_y =
                        rect.bottom() + gap <= other.y || other.bottom() + gap <= rect.y;
                    assert!(
                        separated_x || separated_y,
                        "{surface:?}@{dpi}: {rect:?} is closer than the gap to {other:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn presets_exist_only_in_setup_and_resolve_to_their_index() {
    let setup = pomodoro_layout(PomodoroSurface::Setup, 96);
    for (index, preset) in setup.pomodoro.presets.iter().enumerate() {
        assert_eq!(
            setup.hit_test(preset.x + 4, preset.y + 4),
            Some(HitTarget::PomodoroPreset(index as u8))
        );
        assert_eq!(
            setup.notch_control_rect(HitTarget::PomodoroPreset(index as u8)),
            Some(*preset)
        );
    }
    assert!(setup.pomodoro.reset_button.is_empty());
    assert!(setup.pomodoro.skip_button.is_empty());
    assert!(setup.pomodoro.segments.iter().all(Rect::is_empty));

    let running = pomodoro_layout(PomodoroSurface::Running, 96);
    assert!(running.pomodoro.presets.iter().all(Rect::is_empty));
    assert_eq!(
        running.notch_control_rect(HitTarget::PomodoroPreset(0)),
        None
    );
    assert!(
        running
            .pomodoro
            .segments
            .iter()
            .all(|segment| !segment.is_empty())
    );
}

#[test]
fn focus_stats_tab_layouts_header_card_and_week_columns_across_dpis() {
    use crate::notch::focus_stats_view::FocusStatsRange;

    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let minimum = scale(24, dpi);
        for range in [FocusStatsRange::Today, FocusStatsRange::Week] {
            let layout = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    pomodoro_surface: PomodoroSurface::Running,
                    active_tab: NotchExpandedTab::Stats,
                    stats_progress: 1.0,
                    stats_range: range,
                    ..NotchLayoutInput::default()
                },
            );
            assert_eq!(layout.notch.height, scale(M.notch_stats_height, dpi));
            assert!(layout.notch.contains_rect(&layout.stats.body));
            assert!(layout.notch_tab_strip.right() + gap <= layout.stats.range_today.x);
            assert!(layout.notch_tab_stats.bottom() + gap <= layout.stats.body.y);
            let header_targets = [
                (
                    layout.stats.range_today,
                    HitTarget::FocusStatsRange(FocusStatsRange::Today),
                ),
                (
                    layout.stats.range_week,
                    HitTarget::FocusStatsRange(FocusStatsRange::Week),
                ),
                (
                    layout.stats.open_settings,
                    HitTarget::FocusStatsOpenSettings,
                ),
            ];
            for (rect, hit) in header_targets {
                assert!(layout.notch.contains_rect(&rect));
                assert!(rect.width >= minimum && rect.height >= minimum);
                assert_eq!(
                    layout.hit_test(rect.x + rect.width / 2, rect.y + rect.height / 2),
                    Some(hit)
                );
                assert_eq!(layout.notch_control_rect(hit), Some(rect));
            }
            assert!(layout.stats.range_today.right() + gap <= layout.stats.range_week.x);
            assert!(layout.stats.range_week.right() + gap <= layout.stats.open_settings.x);
            if range == FocusStatsRange::Week {
                for (index, day) in layout.stats.days.iter().enumerate() {
                    assert!(layout.stats.body.contains_rect(day));
                    assert!(day.width >= minimum && day.height >= minimum);
                    assert_eq!(
                        layout.hit_test(day.x + day.width / 2, day.y + day.height / 2),
                        Some(HitTarget::FocusStatsDay(index as u8))
                    );
                    if let Some(next) = layout.stats.days.get(index + 1) {
                        assert!(day.right() + gap <= next.x);
                    }
                }
            } else {
                assert!(layout.stats.days.iter().all(Rect::is_empty));
            }
        }
    }
}

#[test]
fn running_controls_read_reset_play_skip_with_the_primary_in_the_middle() {
    let layout = pomodoro_layout(PomodoroSurface::Running, 96);
    assert!(layout.pomodoro.reset_button.right() < layout.pomodoro.play_button.x);
    assert!(layout.pomodoro.play_button.right() < layout.pomodoro.skip_button.x);
    assert!(layout.pomodoro.play_button.width > layout.pomodoro.skip_button.width);
    assert_eq!(
        layout.notch.right() - layout.pomodoro.skip_button.right(),
        16
    );
    assert!(layout.pomodoro.time.right() < layout.pomodoro.reset_button.x);
}

#[test]
fn alert_surface_exposes_only_its_two_buttons() {
    let layout = pomodoro_layout(PomodoroSurface::Alert, 96);
    let start = layout.pomodoro.alert_start_button;
    let extend = layout.pomodoro.alert_extend_button;
    assert_eq!(
        layout.hit_test(start.x + 4, start.y + 4),
        Some(HitTarget::PomodoroStartNext)
    );
    assert_eq!(
        layout.hit_test(extend.x + 4, extend.y + 4),
        Some(HitTarget::PomodoroExtendFive)
    );
    assert_eq!(
        layout.hit_test(
            layout.notch_tab_pomodoro.x + 4,
            layout.notch_tab_pomodoro.y + 4
        ),
        Some(HitTarget::Notch)
    );
    assert_eq!(
        layout.notch_control_rect(HitTarget::PomodoroPlayPause),
        None
    );
    assert_eq!(
        layout.notch_control_rect(HitTarget::PomodoroStartNext),
        Some(start)
    );
    assert!(layout.pomodoro.subtitle.bottom() < start.y);
}

#[test]
fn progress_segments_span_the_content_width_and_the_footer_sits_below_them() {
    let layout = pomodoro_layout(PomodoroSurface::Running, 96);
    let first = layout.pomodoro.segments[0];
    let last = layout.pomodoro.segments[3];
    assert_eq!(first.x, layout.notch.x + 16);
    assert_eq!(last.right(), layout.notch.right() - 16);
    for pair in layout.pomodoro.segments.windows(2) {
        assert!(pair[0].right() < pair[1].x);
    }
    assert!(first.bottom() < layout.pomodoro.footer_left.y);
    assert!(layout.pomodoro.footer_left.right() <= layout.pomodoro.footer_right.x);
    assert!(layout.pomodoro.play_button.bottom() < first.y);
}

#[test]
fn compact_geometry_leaves_a_peek_between_ring_and_time_inside_the_notch() {
    for dpi in [96, 144, 192] {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert!(layout.notch.contains_rect(&layout.compact_ring));
        assert!(layout.notch.contains_rect(&layout.compact_time));
        assert!(layout.compact_peek.width > scale(120, dpi));
        assert!(layout.compact_ring.right() < layout.compact_peek.x);
        assert!(layout.compact_peek.right() < layout.compact_time.x);
    }
}

fn media_layout(lyrics_progress: f32, dpi: u32) -> Layout {
    compute_layout_with_notch(
        &M,
        1920,
        dpi,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: NotchExpandedTab::Media,
            lyrics_progress,
            tab_blend: 1.0,
            ..NotchLayoutInput::default()
        },
    )
}

#[test]
fn media_controls_fit_the_notch_and_keep_their_gaps_at_every_dpi() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let minimum = scale(24, dpi);
        for lyrics_progress in [0.0, 0.5, 1.0] {
            let layout = media_layout(lyrics_progress, dpi);
            let rects = [
                layout.notch_tab_pomodoro,
                layout.notch_tab_shelf,
                layout.notch_tab_media,
                layout.media.lyrics_toggle,
                layout.media.prev_button,
                layout.media.play_button,
                layout.media.next_button,
                layout.media.seekbar,
            ];
            for (index, rect) in rects.iter().enumerate() {
                assert!(
                    layout.notch.contains_rect(rect),
                    "{lyrics_progress}@{dpi}: {rect:?} leaves the notch"
                );
                assert!(
                    rect.width >= minimum && rect.height >= minimum,
                    "{lyrics_progress}@{dpi}: {rect:?} is below the hit minimum"
                );
                for other in &rects[index + 1..] {
                    let separated_x =
                        rect.right() + gap <= other.x || other.right() + gap <= rect.x;
                    let separated_y =
                        rect.bottom() + gap <= other.y || other.bottom() + gap <= rect.y;
                    assert!(
                        separated_x || separated_y,
                        "{lyrics_progress}@{dpi}: {rect:?} is closer than the gap to {other:?}"
                    );
                }
            }
            assert!(layout.media.seekbar.bottom() <= layout.notch.height - scale(16, dpi));
        }
    }
}

#[test]
fn opening_the_lyrics_drawer_moves_only_the_drawer_and_the_seek_bar() {
    let closed = media_layout(0.0, 96);
    let open = media_layout(1.0, 96);
    assert_eq!(closed.media.cover_art, open.media.cover_art);
    assert_eq!(closed.media.title, open.media.title);
    assert_eq!(closed.media.artist, open.media.artist);
    assert_eq!(closed.media.equalizer, open.media.equalizer);
    assert_eq!(closed.media.prev_button, open.media.prev_button);
    assert_eq!(closed.media.play_button, open.media.play_button);
    assert_eq!(closed.media.next_button, open.media.next_button);
    assert_eq!(closed.media.lyrics_toggle, open.media.lyrics_toggle);
    assert_eq!(closed.media.source_label, open.media.source_label);
    assert_eq!(closed.media.lyrics_drawer.height, 0);
    assert_eq!(open.media.lyrics_drawer.height, 102);
    assert_eq!(open.media.seekbar.y - closed.media.seekbar.y, 110);
    assert_eq!(closed.media.seekbar.y, 98);
}

#[test]
fn the_seek_bar_and_drawer_never_overlap_while_the_drawer_opens() {
    let mut previous_height = 0;
    for step in 0..=20 {
        let layout = media_layout(step as f32 / 20.0, 96);
        let drawer = layout.media.lyrics_drawer;
        assert!(drawer.height >= previous_height, "step {step}");
        previous_height = drawer.height;
        if drawer.height > 0 {
            assert!(
                drawer.bottom() + 12 <= layout.media.seekbar.y,
                "step {step}"
            );
            assert!(layout.media.cover_art.bottom() < drawer.y, "step {step}");
        }
    }
    assert_eq!(previous_height, 102);
}

#[test]
fn compact_title_sits_between_the_cover_and_the_trailing_badge() {
    for dpi in [96, 144, 192] {
        let layout = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                ..NotchLayoutInput::default()
            },
        );
        assert!(layout.notch.contains_rect(&layout.compact_title));
        assert!(layout.notch.contains_rect(&layout.compact_trailing));
        assert!(layout.compact_ring.right() < layout.compact_title.x);
        assert!(layout.compact_title.right() < layout.compact_trailing.x);
        assert!(layout.compact_title.width > scale(150, dpi));
        assert_eq!(
            layout.compact_title.x - layout.notch.x,
            layout.notch.right() - layout.compact_title.right()
        );
        assert_eq!(
            layout.notch.right() - layout.compact_trailing.right(),
            scale(12, dpi)
        );

        let live = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                media_live: true,
                ..NotchLayoutInput::default()
            },
        );
        assert!(live.compact_trailing.width > layout.compact_trailing.width);
        assert!(live.compact_title.right() < live.compact_trailing.x);
    }
}

#[test]
fn compact_notch_widens_to_fit_the_title_up_to_a_cap() {
    for dpi in [96, 144, 192] {
        let compact = |title_points| {
            compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    compact_expansion: 1.0,
                    compact_title_points: title_points,
                    ..NotchLayoutInput::default()
                },
            )
        };
        assert_eq!(compact(0).notch.width, scale(M.notch_compact_width, dpi));
        assert_eq!(compact(40).notch.width, scale(M.notch_compact_width, dpi));
        let wide = compact(300);
        assert!(wide.notch.width > scale(M.notch_compact_width, dpi));
        assert!(wide.compact_title.width >= scale(300, dpi) - 1);
        assert!(wide.notch.contains_rect(&wide.compact_title));
        assert_eq!(
            compact(5000).notch.width,
            scale(COMPACT_MAX_WIDTH_POINTS, dpi)
        );
    }
}

#[test]
fn disabled_control_center_has_no_hit_target() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            control_center_enabled: false,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(layout.slots.control_center_slot.width, 0);
    assert!(!layout.is_hidden(HitTarget::ControlCenter));
    assert_eq!(
        layout.hit_test(layout.slots.control_center_slot.x, 16),
        None
    );
}

#[test]
fn four_adaptive_tabs_fit_without_overlapping_header_labels_across_dpis() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let min_hit = scale(24, dpi);
        for active_tab in NotchExpandedTab::ALL {
            let layout = compute_layout_with_notch(
                &M,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    active_tab,
                    ..NotchLayoutInput::default()
                },
            );
            let tabs = [
                (
                    NotchExpandedTab::Pomodoro,
                    layout.notch_tab_pomodoro,
                    HitTarget::NotchTabPomodoro,
                ),
                (
                    NotchExpandedTab::Stats,
                    layout.notch_tab_stats,
                    HitTarget::NotchTabStats,
                ),
                (
                    NotchExpandedTab::Shelf,
                    layout.notch_tab_shelf,
                    HitTarget::NotchTabShelf,
                ),
                (
                    NotchExpandedTab::Clipboard,
                    layout.notch_tab_clipboard,
                    HitTarget::NotchTabClipboard,
                ),
                (
                    NotchExpandedTab::Tasks,
                    layout.notch_tab_tasks,
                    HitTarget::NotchTabTasks,
                ),
                (
                    NotchExpandedTab::Media,
                    layout.notch_tab_media,
                    HitTarget::NotchTabMedia,
                ),
            ];
            for (tab, rect, hit) in tabs {
                assert!(layout.notch.contains_rect(&rect));
                assert!(rect.width >= min_hit && rect.height >= min_hit);
                assert_eq!(layout.hit_test(rect.x + 8, rect.y + 8), Some(hit));
                assert_eq!(layout.notch_control_rect(hit), Some(rect));
                if tab == active_tab {
                    assert!(rect.width >= scale(64, dpi));
                } else {
                    assert_eq!(rect.width, scale(24, dpi));
                }
            }
            for pair in [
                (layout.notch_tab_pomodoro, layout.notch_tab_stats),
                (layout.notch_tab_stats, layout.notch_tab_shelf),
                (layout.notch_tab_shelf, layout.notch_tab_clipboard),
                (layout.notch_tab_clipboard, layout.notch_tab_tasks),
                (layout.notch_tab_tasks, layout.notch_tab_media),
            ] {
                assert_eq!(pair.1.x - pair.0.right(), gap);
            }
            assert!(layout.notch_tab_strip.right() + gap < layout.pomodoro.focus_modes[0].x);
            assert!(layout.notch_tab_strip.right() + gap < layout.pomodoro.session_label.x);
            assert!(layout.notch_tab_strip.right() + gap <= layout.stats.range_today.x);
            assert!(layout.notch_tab_strip.right() + gap < layout.media.source_label.x);
        }
    }
}

#[test]
fn absent_media_hides_fourth_tab_without_moving_the_first_three() {
    let with_media = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: NotchExpandedTab::Shelf,
            media_present: true,
            ..NotchLayoutInput::default()
        },
    );
    let without_media = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: NotchExpandedTab::Shelf,
            media_present: false,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(
        without_media.notch_tab_pomodoro,
        with_media.notch_tab_pomodoro
    );
    assert_eq!(without_media.notch_tab_shelf, with_media.notch_tab_shelf);
    assert_eq!(
        without_media.notch_tab_clipboard,
        with_media.notch_tab_clipboard
    );
    assert!(without_media.notch_tab_media.is_empty());
    assert_eq!(
        without_media.notch_control_rect(HitTarget::NotchTabMedia),
        None
    );
    assert!(without_media.notch_tab_strip.width < with_media.notch_tab_strip.width);
    assert_eq!(
        without_media.hit_test(
            with_media.notch_tab_media.x + 8,
            with_media.notch_tab_media.y + 8
        ),
        Some(HitTarget::Notch)
    );

    let shelf_only = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: NotchExpandedTab::Shelf,
            focus_enabled: false,
            shelf_enabled: true,
            clipboard_enabled: false,
            tasks_enabled: false,
            media_present: false,
            ..NotchLayoutInput::default()
        },
    );
    assert!(shelf_only.notch_tab_pomodoro.is_empty());
    assert!(shelf_only.notch_tab_clipboard.is_empty());
    assert!(shelf_only.notch_tab_media.is_empty());
    assert_eq!(
        shelf_only.notch_tab_shelf.x,
        with_media.notch_tab_pomodoro.x
    );
    assert!(
        shelf_only
            .notch_tab_strip
            .contains_rect(&shelf_only.notch_tab_shelf)
    );

    let no_tabs = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            focus_enabled: false,
            shelf_enabled: false,
            clipboard_enabled: false,
            tasks_enabled: false,
            media_present: false,
            ..NotchLayoutInput::default()
        },
    );
    assert!(no_tabs.notch_tab_strip.is_empty());
    assert_eq!(
        no_tabs.notch_control_rect(HitTarget::NotchTabPomodoro),
        None
    );
    assert_eq!(no_tabs.notch_control_rect(HitTarget::NotchTabShelf), None);
    assert_eq!(
        no_tabs.notch_control_rect(HitTarget::NotchTabClipboard),
        None
    );
}

#[test]
fn tasks_header_rows_and_buttons_stay_non_overlapping_across_dpis() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let layout = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Tasks,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                tasks: TasksLayoutInput {
                    visible_count: 4,
                    has_pager: true,
                    has_items: true,
                    has_done: true,
                    editing: false,
                },
                ..NotchLayoutInput::default()
            },
        );
        let header = [
            layout.notch_tab_strip,
            layout.privacy.mic_button,
            layout.tasks.pager_button,
            layout.tasks.add_button,
            layout.tasks.clear_button,
        ];
        for (index, rect) in header.iter().enumerate() {
            assert!(layout.notch.contains_rect(rect));
            assert!(!rect.is_empty());
            if let Some(next) = header.get(index + 1) {
                assert!(rect.right() + gap <= next.x);
            }
        }
        assert!(layout.tasks.editor.is_empty());
        assert!(layout.tasks.editor_input.is_empty());
        for (index, row) in layout.tasks.rows.iter().enumerate() {
            assert!(layout.tasks.body.contains_rect(row));
            let active = layout.tasks.check_buttons[index];
            let edit = layout.tasks.edit_buttons[index];
            let remove = layout.tasks.remove_buttons[index];
            assert!(row.contains_rect(&active));
            assert!(row.contains_rect(&edit));
            assert!(row.contains_rect(&remove));
            assert!(active.right() < edit.x);
            assert!(edit.right() < remove.x);
            if let Some(next) = layout.tasks.rows.get(index + 1) {
                assert!(row.bottom() < next.y);
            }
        }
        for (hit, rect) in [
            (HitTarget::TaskRemove(2), layout.tasks.remove_buttons[2]),
            (HitTarget::TaskEdit(2), layout.tasks.edit_buttons[2]),
            (HitTarget::TaskCheck(2), layout.tasks.check_buttons[2]),
            (HitTarget::TaskItem(2), layout.tasks.rows[2]),
            (HitTarget::TasksAdd, layout.tasks.add_button),
            (HitTarget::TasksClearDone, layout.tasks.clear_button),
            (HitTarget::TasksPager, layout.tasks.pager_button),
        ] {
            let probe = if matches!(hit, HitTarget::TaskItem(_)) {
                (rect.x + rect.width / 2, rect.y + 1)
            } else {
                (rect.x + rect.width / 2, rect.y + rect.height / 2)
            };
            assert_eq!(layout.hit_test(probe.0, probe.1), Some(hit));
            assert_eq!(layout.notch_control_rect(hit), Some(rect));
        }
    }
}

#[test]
fn tasks_inline_editor_expands_notch_and_keeps_rows_clear_across_dpis() {
    for dpi in [96, 120, 144, 192] {
        let closed = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Tasks,
                tasks: TasksLayoutInput {
                    visible_count: 4,
                    has_pager: false,
                    has_items: true,
                    has_done: false,
                    editing: false,
                },
                ..NotchLayoutInput::default()
            },
        );
        let editing = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Tasks,
                tasks: TasksLayoutInput {
                    visible_count: 4,
                    has_pager: false,
                    has_items: true,
                    has_done: false,
                    editing: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        assert!(editing.notch.height > closed.notch.height);
        assert!(editing.panel_window.height > closed.panel_window.height);
        assert!(editing.notch.contains_rect(&editing.tasks.editor));
        assert!(
            editing
                .tasks
                .editor
                .contains_rect(&editing.tasks.editor_input)
        );
        assert!(editing.tasks.editor.bottom() < editing.tasks.body.y);
        assert!(editing.notch.contains_rect(&editing.tasks.body));
        for (index, row) in editing.tasks.rows.iter().enumerate() {
            assert!(editing.tasks.body.contains_rect(row));
            assert!(row.contains_rect(&editing.tasks.check_buttons[index]));
            assert!(row.contains_rect(&editing.tasks.edit_buttons[index]));
            assert!(row.contains_rect(&editing.tasks.remove_buttons[index]));
        }
    }
}

#[test]
fn empty_tasks_expose_no_row_or_header_controls_beyond_add() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: NotchExpandedTab::Tasks,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.tasks.rows.iter().all(Rect::is_empty));
    assert!(layout.tasks.check_buttons.iter().all(Rect::is_empty));
    assert!(layout.tasks.edit_buttons.iter().all(Rect::is_empty));
    assert!(layout.tasks.editor.is_empty());
    assert!(layout.tasks.editor_input.is_empty());
    assert!(layout.tasks.clear_button.is_empty());
    assert!(layout.tasks.pager_button.is_empty());
    assert!(!layout.tasks.add_button.is_empty());
    assert_eq!(layout.notch_control_rect(HitTarget::TaskItem(0)), None);
    assert_eq!(layout.notch_control_rect(HitTarget::TaskEdit(0)), None);
}

#[test]
fn a_disabled_tasks_tab_has_no_rect() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            tasks_enabled: false,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.notch_tab_tasks.is_empty());
    assert_eq!(layout.notch_control_rect(HitTarget::NotchTabTasks), None);
}

#[test]
fn clipboard_header_and_rows_stay_non_overlapping_across_dpis() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let layout = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Clipboard,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                clipboard: ClipboardLayoutInput {
                    visible_count: 4,
                    has_pager: true,
                    has_items: true,
                    paused: false,
                },
                ..NotchLayoutInput::default()
            },
        );
        let row_one = [
            layout.notch_tab_strip,
            layout.privacy.mic_button,
            layout.clipboard.pager_button,
            layout.clipboard.pause_button,
            layout.clipboard.clear_button,
        ];
        for (index, rect) in row_one.iter().enumerate() {
            assert!(layout.notch.contains_rect(rect));
            assert!(!rect.is_empty());
            if let Some(next) = row_one.get(index + 1) {
                assert!(rect.right() + gap <= next.x);
            }
        }
        for (index, row) in layout.clipboard.rows.iter().enumerate() {
            assert!(layout.clipboard.body.contains_rect(row));
            assert!(row.contains_rect(&layout.clipboard.remove_buttons[index]));
            if let Some(next) = layout.clipboard.rows.get(index + 1) {
                assert!(row.bottom() < next.y);
            }
        }
        assert_eq!(
            layout.hit_test(
                layout.clipboard.remove_buttons[2].x + 4,
                layout.clipboard.remove_buttons[2].y + 4,
            ),
            Some(HitTarget::ClipboardRemoveItem(2))
        );
        assert_eq!(
            layout.hit_test(
                layout.clipboard.rows[2].x + 8,
                layout.clipboard.rows[2].y + 8
            ),
            Some(HitTarget::ClipboardItem(2))
        );
    }
}

#[test]
fn privacy_dots_and_panel_mic_button_keep_clearances_across_dpis() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let collapsed = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                compact_expansion: 1.0,
                compact_title_points: 120,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        assert!(collapsed.notch.contains_rect(&collapsed.privacy.dots));
        assert!(collapsed.compact_time.right() + gap <= collapsed.privacy.dots.x);
        assert!(collapsed.compact_trailing.right() + gap <= collapsed.privacy.dots.x);
        assert_eq!(
            collapsed.hit_test(
                collapsed.privacy.dots.x + 2,
                collapsed.privacy.dots.y + collapsed.privacy.dots.height / 2,
            ),
            Some(HitTarget::PrivacyIndicator)
        );

        let shelf_open = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        assert!(shelf_open.privacy.dots.is_empty());
        assert!(
            shelf_open
                .notch
                .contains_rect(&shelf_open.privacy.mic_button)
        );
        assert!(shelf_open.notch_tab_strip.right() + gap <= shelf_open.privacy.mic_button.x);
        assert!(shelf_open.privacy.mic_button.right() + gap <= shelf_open.shelf.add_button.x);
        assert_eq!(
            shelf_open.hit_test(
                shelf_open.privacy.mic_button.x + 4,
                shelf_open.privacy.mic_button.y + 4,
            ),
            Some(HitTarget::PrivacyMicMute)
        );

        let media_open = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Media,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        assert!(
            media_open
                .notch
                .contains_rect(&media_open.privacy.mic_button)
        );
        assert!(media_open.notch_tab_strip.right() + gap <= media_open.privacy.mic_button.x);
        assert_eq!(
            media_open.hit_test(
                media_open.privacy.mic_button.x + 4,
                media_open.privacy.mic_button.y + 4,
            ),
            Some(HitTarget::PrivacyMicMute)
        );
    }
}

#[test]
fn shelf_header_and_card_grid_stay_non_overlapping_across_dpis() {
    for dpi in [96, 120, 144, 192] {
        let gap = scale(4, dpi);
        let layout = compute_layout_with_notch(
            &M,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                privacy: PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                shelf: ShelfLayoutInput {
                    visible_count: 6,
                    has_pager: true,
                    has_items: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        let row_one = [
            layout.notch_tab_strip,
            layout.privacy.mic_button,
            layout.shelf.pager_button,
            layout.shelf.add_button,
            layout.shelf.clear_button,
        ];
        for (index, rect) in row_one.iter().enumerate() {
            assert!(layout.notch.contains_rect(rect));
            assert!(!rect.is_empty());
            if let Some(next) = row_one.get(index + 1) {
                assert!(rect.right() + gap <= next.x);
            }
        }
        for (index, card) in layout.shelf.cards.iter().enumerate() {
            assert!(layout.shelf.dropzone.contains_rect(card));
            assert!(card.contains_rect(&layout.shelf.copy_buttons[index]));
            assert!(card.contains_rect(&layout.shelf.remove_buttons[index]));
            for other in &layout.shelf.cards[index + 1..] {
                let sep_x = card.right() + gap <= other.x || other.right() + gap <= card.x;
                let sep_y = card.bottom() + gap <= other.y || other.bottom() + gap <= card.y;
                assert!(sep_x || sep_y);
            }
        }
        assert_eq!(
            layout.hit_test(
                layout.shelf.remove_buttons[4].x + 4,
                layout.shelf.remove_buttons[4].y + 4,
            ),
            Some(HitTarget::ShelfRemoveItem(4))
        );
        assert_eq!(
            layout.hit_test(
                layout.shelf.copy_buttons[4].x + 4,
                layout.shelf.copy_buttons[4].y + 4,
            ),
            Some(HitTarget::ShelfCopyItem(4))
        );
        assert_eq!(
            layout.hit_test(layout.shelf.cards[4].x + 8, layout.shelf.cards[4].y + 16),
            Some(HitTarget::ShelfItem(4))
        );
    }
}

#[test]
fn secondary_notch_circle_sits_to_the_right_of_main_notch_and_hits_cleanly() {
    let layout = compute_layout_with_notch(
        &Metrics::MACBOOK_PRO_14,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            compact_expansion: 1.0,
            secondary_expansion: 1.0,
            compact_primary: Some(CompactSlotKind::Focus),
            compact_secondary: Some(CompactSlotKind::Media),
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(layout.notch_secondary.width, 28);
    assert_eq!(layout.notch_secondary.height, 28);
    assert_eq!(layout.notch_secondary.x, layout.notch.right() + 8);
    assert_eq!(layout.notch_secondary.y, 2);
    assert!(layout.slots.tray_slot.x >= layout.notch_secondary.right() + 4);
    assert_eq!(
        layout.hit_test(layout.notch_secondary.x + 14, layout.notch_secondary.y + 14,),
        Some(HitTarget::NotchSecondary)
    );
    assert_eq!(
        layout.hit_test(layout.notch.x + 20, layout.notch.y + 14),
        Some(HitTarget::Notch)
    );
}

#[test]
fn dynamic_island_metrics_float_capsule_inside_40pt_bar_with_compact_and_hud_bounds() {
    assert_eq!(
        Metrics::for_style(NotchStyle::ClassicNotch),
        Metrics::MACBOOK_PRO_14
    );
    assert_eq!(
        Metrics::for_style(NotchStyle::DynamicIsland),
        Metrics::DYNAMIC_ISLAND
    );

    let idle = compute_layout(&Metrics::DYNAMIC_ISLAND, 1920, 96);
    assert_eq!(
        idle.bar,
        Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 40,
        }
    );
    assert_eq!(
        idle.notch,
        Rect {
            x: 900,
            y: 4,
            width: 120,
            height: 32,
        }
    );
    assert_eq!(idle.notch_corner_radius, 16);
    assert_eq!(idle.notch_ear_radius, 0);
    assert_eq!(idle.hit_test(idle.notch.x + 10, 2), None);
    assert_eq!(idle.hit_test(idle.notch.x + 10, 38), None);
    assert_eq!(idle.hit_test(idle.notch.x + 10, 20), Some(HitTarget::Notch));

    let compact = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            compact_expansion: 1.0,
            secondary_expansion: 1.0,
            privacy: PrivacyLayoutState {
                compact_dots: true,
                panel_mic_control: false,
            },
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(compact.notch.y, 2);
    assert_eq!(compact.notch.height, 36);
    assert_eq!(compact.notch_corner_radius, 18);
    assert!(compact.notch.contains_rect(&compact.compact_ring));
    assert!(compact.notch.contains_rect(&compact.compact_time));
    assert!(compact.notch.contains_rect(&compact.compact_title));
    assert!(compact.notch.contains_rect(&compact.compact_trailing));
    assert!(compact.notch.contains_rect(&compact.privacy.dots));
    assert!(compact.privacy.dots.x >= compact.compact_trailing.right());
    assert!(compact.privacy.dots.right() <= compact.notch.right() - 8);
    assert_eq!(compact.notch_secondary.y, compact.notch.y);
    assert_eq!(compact.notch_secondary.height, compact.notch.height);
    assert_eq!(compact.notch_secondary.width, compact.notch.height);

    let compact_hidpi = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        2880,
        144,
        "Finder",
        NotchLayoutInput {
            compact_expansion: 1.0,
            secondary_expansion: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(compact_hidpi.notch_secondary.y, compact_hidpi.notch.y);
    assert_eq!(
        compact_hidpi.notch_secondary.height,
        compact_hidpi.notch.height
    );
    assert_eq!(
        compact_hidpi.notch_secondary.width,
        compact_hidpi.notch.height
    );

    let hud = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            hud_expansion: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(hud.notch.y, 4);
    assert_eq!(hud.notch.height, 32);
    assert!(hud.notch.contains_rect(&hud.hud.glyph));
    assert!(hud.notch.contains_rect(&hud.hud.meter));
    assert!(hud.notch.contains_rect(&hud.hud.value));
    assert_eq!(idle.popup_anchor_y(96), 44);
    assert_eq!(
        compute_layout(&Metrics::DYNAMIC_ISLAND, 2880, 144).popup_anchor_y(144),
        66
    );
}

#[test]
fn dynamic_island_expands_floating_card_without_ears() {
    let expanded = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            pomodoro_surface: PomodoroSurface::Running,
            active_tab: NotchExpandedTab::Media,
            lyrics_progress: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(expanded.bar.height, 40);
    assert_eq!(expanded.notch.y, 4);
    assert_eq!(expanded.notch.width, 416);
    assert_eq!(expanded.notch.height, 254);
    assert_eq!(expanded.notch_corner_radius, 28);
    assert_eq!(expanded.notch_ear_radius, 0);
    assert!(expanded.panel_window.height >= expanded.notch.bottom() + 4);
    assert!(expanded.notch.contains_rect(&expanded.notch_tab_pomodoro));
    assert!(expanded.notch.contains_rect(&expanded.media.cover_art));
    assert!(expanded.notch.contains_rect(&expanded.media.lyrics_drawer));
    assert!(expanded.notch.contains_rect(&expanded.media.seekbar));
    assert_eq!(
        expanded.hit_test(
            expanded.media.play_button.x + 8,
            expanded.media.play_button.y + 8,
        ),
        Some(HitTarget::MediaPlayPause)
    );
}

#[test]
fn dynamic_island_expanded_concentricity_and_tab_clearance_across_dpis() {
    assert_eq!(
        Metrics::DYNAMIC_ISLAND.notch_expanded_corner_radius - PANEL_PAD_POINTS,
        12
    );
    for dpi in [96, 120, 144, 192] {
        let layout = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Pomodoro,
                ..NotchLayoutInput::default()
            },
        );
        assert_eq!(layout.notch_corner_radius, scale(28, dpi));
        assert_eq!(layout.notch_tab_strip.y - layout.notch.y, scale(8, dpi));
        assert_eq!(
            layout.notch_tab_pomodoro.y - layout.notch.y,
            scale(PANEL_TAB_TOP_POINTS, dpi)
        );
        assert!(
            layout.pomodoro.icon.y - layout.notch_tab_pomodoro.bottom() >= scale(4, dpi),
            "dpi {dpi}"
        );

        let shelf = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                ..NotchLayoutInput::default()
            },
        );
        assert!(
            shelf.shelf.dropzone.y - shelf.notch_tab_pomodoro.bottom() >= scale(4, dpi),
            "dpi {dpi}"
        );

        let clipboard = compute_layout_with_notch(
            &Metrics::DYNAMIC_ISLAND,
            1920,
            dpi,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Clipboard,
                ..NotchLayoutInput::default()
            },
        );
        assert!(
            clipboard.clipboard.body.y - clipboard.notch_tab_pomodoro.bottom() >= scale(4, dpi),
            "dpi {dpi}"
        );
    }
}

#[test]
fn menu_slot_estimation_uses_uniform_horizontal_padding() {
    assert_eq!(MENU_SLOT_HORIZONTAL_PAD_POINTS, 10);
    assert_eq!(
        estimate_menu_text_width_points("Finder"),
        8 + 5 + 8 + 8 + 8 + 5
    );
    assert_eq!(
        estimate_app_slot_points(&M, "Finder"),
        (estimate_menu_text_width_points("Finder") + MENU_SLOT_HORIZONTAL_PAD_POINTS * 2)
            .clamp(M.app_slot_min_width, M.app_slot_max_width)
    );
    assert_eq!(M.file_slot_width, 40);
    assert_eq!(M.edit_slot_width, 40);
    assert_eq!(M.view_slot_width, 46);
    assert_eq!(M.window_slot_width, 64);
    assert_eq!(M.help_slot_width, 44);
}

#[test]
fn press_progress_compresses_capsule_symmetrically_or_flush_at_top() {
    let island_idle = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        192,
        "Finder",
        NotchLayoutInput::default(),
    );
    let island_pressed = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        192,
        "Finder",
        NotchLayoutInput {
            press_progress: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert!(island_pressed.notch.width < island_idle.notch.width);
    assert!(island_pressed.notch.height < island_idle.notch.height);
    assert!(
        ((island_pressed.notch.x * 2 + island_pressed.notch.width)
            - (island_idle.notch.x * 2 + island_idle.notch.width))
            .abs()
            <= 1
    );
    assert!(
        ((island_pressed.notch.y * 2 + island_pressed.notch.height)
            - (island_idle.notch.y * 2 + island_idle.notch.height))
            .abs()
            <= 1
    );
    assert!((island_pressed.press_progress - 1.0).abs() < f32::EPSILON);

    let classic_idle = compute_layout_with_notch(
        &Metrics::MACBOOK_PRO_14,
        1920,
        192,
        "Finder",
        NotchLayoutInput::default(),
    );
    let classic_pressed = compute_layout_with_notch(
        &Metrics::MACBOOK_PRO_14,
        1920,
        192,
        "Finder",
        NotchLayoutInput {
            press_progress: 1.5,
            ..NotchLayoutInput::default()
        },
    );
    assert!(classic_pressed.notch.width < classic_idle.notch.width);
    assert!(classic_pressed.notch.height < classic_idle.notch.height);
    assert_eq!(classic_pressed.notch.y, 0);
    assert!((classic_pressed.press_progress - 1.0).abs() < f32::EPSILON);

    let expanded_pressed = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        192,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 0.3,
            press_progress: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    let expanded_unpressed = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        192,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 0.3,
            press_progress: 0.0,
            ..NotchLayoutInput::default()
        },
    );
    assert_eq!(expanded_pressed.notch, expanded_unpressed.notch);
}

#[test]
fn a_user_hidden_item_has_no_slot_and_is_not_an_overflow_entry() {
    let shown = compute_layout_with_notch(&M, 1920, 96, "Finder", NotchLayoutInput::default());
    let hidden_items = BarItemSet::default()
        .toggled(BarItem::Clock)
        .toggled(BarItem::Status);
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            hidden_items,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.slots.clock_slot.is_empty());
    assert!(layout.slots.status_slot.is_empty());
    assert!(layout.slots.status_wifi_rect.is_empty());
    assert!(layout.slots.hidden_targets.is_empty());
    assert_eq!(layout.slots.overflow_slot.width, 0);
    assert!(layout.slots.notification_slot.right() > shown.slots.notification_slot.right());
    let keyboard = layout.keyboard_targets();
    assert!(!keyboard.contains(&HitTarget::Clock));
    assert!(!keyboard.contains(&HitTarget::StatusWifi));
    assert!(keyboard.contains(&HitTarget::Notifications));
}

#[test]
fn the_widest_compact_notch_overflows_nothing_on_common_wide_screens() {
    let widest = NotchLayoutInput {
        compact_expansion: 1.0,
        compact_primary: Some(CompactSlotKind::Media),
        compact_title_points: 600,
        ..NotchLayoutInput::default()
    };
    for (width, dpi) in [(1366, 96), (1920, 96), (2560, 144)] {
        let layout = compute_layout_with_notch(&M, width, dpi, "Google Chrome", widest);
        assert!(layout.slots.hidden_targets.is_empty(), "{width}@{dpi}");
        assert_eq!(layout.slots.overflow_slot.width, 0, "{width}@{dpi}");
    }
}

fn notification_layout_for(ringing: bool) -> Layout {
    compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            notification: Some(if ringing {
                NotificationLayoutInput::ringing()
            } else {
                NotificationLayoutInput::banner(false, false)
            }),
            ..NotchLayoutInput::default()
        },
    )
}

fn center(rect: Rect) -> (i32, i32) {
    (rect.x + rect.width / 2, rect.y + rect.height / 2)
}

#[test]
fn a_ringing_card_is_taller_and_hits_accept_decline_and_the_card() {
    let layout = notification_layout_for(true);
    let card = layout.notification.expect("card");
    assert_eq!(layout.notch.height, NOTIFICATION_RING_HEIGHT_POINTS);
    assert!(card.ringing && card.close.is_empty());
    let (ax, ay) = center(card.accept);
    let (dx, dy) = center(card.decline);
    assert_eq!(layout.hit_test(ax, ay), Some(HitTarget::NotificationAccept));
    assert_eq!(
        layout.hit_test(dx, dy),
        Some(HitTarget::NotificationDecline)
    );
    let (tx, ty) = center(card.title);
    assert_eq!(layout.hit_test(tx, ty), Some(HitTarget::NotificationCard));
    assert_eq!(card.accept.x - card.decline.right(), 12);
    assert!(card.accept.right() <= card.card.right());
    assert!(card.accept.width >= 44 && card.decline.width >= 44);
    let card_mid = card.card.y + card.card.height / 2;
    let badge_mid = card.badge.y + card.badge.height / 2;
    let text_mid = i32::midpoint(card.meta.y, card.title.bottom());
    let button_mid = card.accept.y + card.accept.height / 2;
    assert_eq!(badge_mid, card_mid);
    assert_eq!(text_mid, card_mid);
    assert_eq!(button_mid, card_mid);
    assert_eq!(
        layout.keyboard_targets_with_media(true),
        vec![
            HitTarget::NotificationDecline,
            HitTarget::NotificationAccept
        ]
    );
}

#[test]
fn a_banner_card_has_a_close_control_zero_overlap_and_optional_queue_lip() {
    for metrics in [Metrics::MACBOOK_PRO_14, Metrics::DYNAMIC_ISLAND] {
        for dpi in [96, 120, 144, 192] {
            let layout = compute_layout_with_notch(
                &metrics,
                1920,
                dpi,
                "Finder",
                NotchLayoutInput {
                    panel_expansion: 1.0,
                    notification: Some(NotificationLayoutInput::banner(false, true)),
                    ..NotchLayoutInput::default()
                },
            );
            let card = layout.notification.expect("card");
            assert_eq!(
                layout.notch.height,
                scale(NOTIFICATION_BANNER_HEIGHT_POINTS, dpi)
            );
            assert!(card.accept.is_empty() && card.decline.is_empty());
            assert!(card.close.bottom() <= card.title.y);
            assert!(card.meta.right() < card.close.x);
            assert!(card.close.width >= scale(24, dpi));
            assert!(card.close.height >= scale(24, dpi));
            assert!(card.close.right() <= card.card.right());
            assert!(!card.stack_lip.is_empty());
            assert!(card.stack_lip.bottom() <= layout.panel_window.bottom());
            let (cx, cy) = center(card.close);
            assert_eq!(layout.hit_test(cx, cy), Some(HitTarget::NotificationClose));
            let (bx, by) = center(card.badge);
            assert_eq!(layout.hit_test(bx, by), Some(HitTarget::NotificationCard));
        }
    }
    let empty_layout = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            notification: Some(NotificationLayoutInput::banner(true, false)),
            ..NotchLayoutInput::default()
        },
    );
    let empty_card = empty_layout.notification.expect("empty banner");
    assert!(empty_card.subtitle.is_empty());
    assert!(empty_card.stack_lip.is_empty());
    assert!(empty_card.close.bottom() <= empty_card.title.y);
    assert!(empty_card.meta.y > empty_card.badge.y);
    assert_eq!(
        empty_layout.keyboard_targets_with_media(false),
        vec![HitTarget::NotificationCard, HitTarget::NotificationClose]
    );
    assert_eq!(
        empty_layout.notch_control_rect(HitTarget::NotificationAccept),
        None
    );
}

#[test]
fn a_notification_peek_uses_compact_bar_width_without_opening_panel_card() {
    let layout = compute_layout_with_notch(
        &Metrics::DYNAMIC_ISLAND,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            compact_expansion: 1.0,
            panel_expansion: 0.0,
            notification: Some(NotificationLayoutInput::peek()),
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.notification.is_none());
    assert_eq!(
        layout.notch.width,
        Metrics::DYNAMIC_ISLAND.notch_compact_width
    );
    assert_eq!(
        layout.notch.height,
        Metrics::DYNAMIC_ISLAND.notch_compact_height
    );
}

#[test]
fn without_a_notification_the_panel_keeps_its_tabs_and_height() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            panel_expansion: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.notification.is_none());
    assert_eq!(layout.notch.height, M.notch_expanded_height);
}

#[test]
fn disabling_top_bar_clears_bar_slots_while_keeping_notch_interactive() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            top_bar_enabled: false,
            notch_enabled: true,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.slots.start_slot.is_empty());
    assert!(layout.slots.clock_slot.is_empty());
    assert!(layout.slots.control_center_slot.is_empty());
    assert!(layout.slots.hidden_targets.is_empty());
    assert!(!layout.notch.is_empty());
    let (nx, ny) = center(layout.notch);
    assert_eq!(layout.hit_test(nx, ny), Some(HitTarget::Notch));
    assert_eq!(layout.hit_test(16, 16), None);
    assert_eq!(
        layout.keyboard_targets_with_media(false),
        vec![HitTarget::Notch]
    );
}

#[test]
fn disabling_notch_clears_notch_and_gives_top_bar_the_full_width() {
    let layout = compute_layout_with_notch(
        &M,
        820,
        96,
        "Visual Studio Code",
        NotchLayoutInput {
            top_bar_enabled: true,
            notch_enabled: false,
            panel_expansion: 1.0,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.notch.is_empty());
    assert!(layout.panel_window.is_empty());
    assert!((layout.panel_expansion - 0.0).abs() < f32::EPSILON);
    assert!(layout.slots.hidden_targets.is_empty());
    assert!(!layout.slots.help_slot.is_empty());
    assert!(!layout.slots.clock_slot.is_empty());
    assert!(layout.slots.help_slot.right() < layout.slots.tray_slot.x);
    assert!(
        !layout
            .keyboard_targets_with_media(true)
            .contains(&HitTarget::Notch)
    );
}

#[test]
fn disabling_both_top_bar_and_notch_leaves_no_hit_or_keyboard_targets() {
    let layout = compute_layout_with_notch(
        &M,
        1920,
        96,
        "Finder",
        NotchLayoutInput {
            top_bar_enabled: false,
            notch_enabled: false,
            ..NotchLayoutInput::default()
        },
    );
    assert!(layout.notch.is_empty());
    assert!(layout.slots.start_slot.is_empty());
    assert!(layout.slots.clock_slot.is_empty());
    assert_eq!(layout.hit_test(960, 16), None);
    assert_eq!(layout.hit_test(16, 16), None);
    assert!(layout.keyboard_targets_with_media(true).is_empty());
}

#[test]
fn hit_target_scopes_separate_the_bar_from_the_notch_surface() {
    assert_eq!(HitTarget::AppMenu.scope(), TargetScope::BarMenu);
    assert_eq!(HitTarget::StatusWifi.scope(), TargetScope::BarStatus);
    assert_eq!(HitTarget::Clock.scope(), TargetScope::BarTrailing);
    assert_eq!(HitTarget::ShelfItem(2).scope(), TargetScope::Shelf);
    assert_eq!(HitTarget::TaskCheck(1).scope(), TargetScope::Tasks);
    assert_eq!(HitTarget::MediaSeekbar.scope(), TargetScope::Media);
    assert_eq!(
        HitTarget::NotificationClose.scope(),
        TargetScope::Notification
    );
    assert!(HitTarget::Search.scope().is_bar());
    assert!(!HitTarget::Notch.scope().is_bar());
    assert!(HitTarget::PomodoroReset.scope().is_notch_surface());
}
