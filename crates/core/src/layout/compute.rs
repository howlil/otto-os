use crate::bar::menu_bar::BarItem;
use crate::notch::hud::compute_hud_geometry;
use crate::notch::media::NotchExpandedTab;
use crate::notch::privacy::PRIVACY_COMPACT_EXTRA_POINTS;

use super::compact::CompactNotchGeometry;
use super::compact::compact_width_points;
use super::compact::compute_compact_notch_geometry;
use super::compact::compute_secondary_notch_rect;
use super::geometry::Rect;
use super::geometry::lerp_points;
use super::geometry::scale;
use super::hit::HitTarget;
use super::input::NotchLayoutInput;
use super::metrics::Metrics;
use super::model::Layout;
use super::model::{
    BarSlotsLayout, ClipboardPanelLayout, FocusStatsPanelLayout, MediaPanelLayout,
    PomodoroPanelLayout, PrivacyPanelLayout, ShelfPanelLayout, TasksPanelLayout,
};
use super::notification::NOTIFICATION_BANNER_HEIGHT_POINTS;
use super::notification::NOTIFICATION_RING_HEIGHT_POINTS;
use super::notification::NOTIFICATION_STACK_EXTRA_PANEL_POINTS;
use super::notification::compute_notification_layout;
use super::panel::ExpandedNotchControls;
use super::panel::compute_expanded_notch_controls;
use super::slots::LeftSlotInput;
use super::slots::SlotRow;
use super::slots::estimate_app_slot_points;
use super::slots::resolve_left_slots;
use super::slots::resolve_right_slots;
use super::tasks::TASK_EDITOR_EXTRA_HEIGHT_POINTS;
use crate::notch::notch_notification::NotificationStage;

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn compute_layout_with_notch(
    metrics: &Metrics,
    screen_width: i32,
    dpi: u32,
    app_name: &str,
    notch_input: NotchLayoutInput,
) -> Layout {
    let bar_height = scale(metrics.bar_height, dpi);
    let notch_enabled = notch_input.notch_enabled;
    let hud_t = if notch_enabled {
        notch_input.hud_expansion.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let compact_t = if notch_enabled {
        notch_input.compact_expansion.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let panel_t = if notch_enabled {
        notch_input.panel_expansion.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let compact_height_pts = lerp_points(
        metrics.notch_height,
        metrics.notch_compact_height,
        compact_t,
    );
    let compact_top_inset_pts = if metrics.notch_top_inset > 0 {
        (metrics.bar_height - compact_height_pts) / 2
    } else {
        metrics.notch_top_inset
    };
    let notch_y = scale(compact_top_inset_pts, dpi);
    let privacy_extra_pts = if notch_enabled && notch_input.privacy.compact_dots {
        PRIVACY_COMPACT_EXTRA_POINTS
    } else {
        0
    };

    let base_pts = metrics.notch_width + privacy_extra_pts;
    let is_notification_peek = notch_input
        .notification
        .is_some_and(|card| card.stage == NotificationStage::Peek);
    let compact_target = if is_notification_peek {
        metrics.notch_compact_width + privacy_extra_pts
    } else {
        compact_width_points(
            metrics,
            notch_input.compact_title_points,
            notch_input.media_live,
            notch_input.compact_primary,
        ) + privacy_extra_pts
    };
    let compact_pts = lerp_points(base_pts, compact_target, compact_t);
    let hud_pts = lerp_points(base_pts, metrics.notch_hud_width + privacy_extra_pts, hud_t);
    let collapsed_pts = compact_pts.max(hud_pts);
    let lyrics_t = notch_input.lyrics_progress.clamp(0.0, 1.0);
    let stats_t = notch_input.stats_progress.clamp(0.0, 1.0);
    let target_expanded_h = match notch_input.notification {
        Some(card) if card.is_ringing() => NOTIFICATION_RING_HEIGHT_POINTS,
        Some(_) => NOTIFICATION_BANNER_HEIGHT_POINTS,
        None => {
            let lyrics_h = lerp_points(
                metrics.notch_expanded_height,
                metrics.notch_lyrics_height,
                lyrics_t,
            );
            let stats_h = lerp_points(
                metrics.notch_expanded_height,
                metrics.notch_stats_height,
                stats_t,
            );
            let base = lyrics_h.max(stats_h);
            if notch_input.active_tab == NotchExpandedTab::Tasks && notch_input.tasks.editing {
                base.max(metrics.notch_expanded_height + TASK_EDITOR_EXTRA_HEIGHT_POINTS)
            } else {
                base
            }
        }
    };
    let notch_pts_w = lerp_points(collapsed_pts, metrics.notch_expanded_width, panel_t);
    let notch_pts_h = lerp_points(compact_height_pts, target_expanded_h, panel_t);

    let unscaled_w = scale(notch_pts_w, dpi);
    let unscaled_h = scale(notch_pts_h, dpi);
    let press_progress = if notch_enabled {
        notch_input.press_progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (notch_width, notch_height, notch_y) = if press_progress > 0.001 && panel_t < 0.25 {
        let press_scale = 1.0 - 0.03 * press_progress * (1.0 - panel_t * 4.0).clamp(0.0, 1.0);
        let compressed_w = (unscaled_w as f32 * press_scale).round() as i32;
        let compressed_h = (unscaled_h as f32 * press_scale).round() as i32;
        let compressed_y = if metrics.notch_top_inset > 0 {
            notch_y + (unscaled_h - compressed_h) / 2
        } else {
            notch_y
        };
        (compressed_w, compressed_h, compressed_y)
    } else {
        (unscaled_w, unscaled_h, notch_y)
    };
    let padding = scale(metrics.edge_padding, dpi);
    let left_gap = scale(metrics.left_slot_gap, dpi);
    let right_gap = scale(metrics.right_slot_gap, dpi);

    let bar = Rect {
        x: 0,
        y: 0,
        width: screen_width,
        height: bar_height,
    };
    let hud_width = scale(metrics.notch_hud_width, dpi);
    let hud_geometry = compute_hud_geometry(
        Rect {
            x: (screen_width - hud_width) / 2,
            y: scale(metrics.notch_top_inset, dpi),
            width: hud_width,
            height: scale(metrics.notch_height, dpi),
        },
        metrics,
        dpi,
    );
    let secondary_t = if notch_enabled {
        notch_input.secondary_expansion.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let swap_blend = notch_input.compact_swap_blend.clamp(0.0, 1.0);
    let (
        notch,
        panel_window,
        notification_layout,
        controls,
        compact,
        notch_secondary,
        compact_primary,
        compact_secondary,
    ) = if notch_enabled {
        let notch = Rect {
            x: (screen_width - notch_width) / 2,
            y: notch_y,
            width: notch_width,
            height: notch_height,
        };
        let panel_pts_w = metrics.notch_expanded_width + metrics.notch_expanded_ear_radius * 2 + 8;
        let extra_panel_pts = if notch_input
            .notification
            .is_some_and(|card| card.is_panel_card() && !card.is_ringing() && card.has_queue)
        {
            NOTIFICATION_STACK_EXTRA_PANEL_POINTS
        } else {
            4
        };
        let panel_pts_h = compact_top_inset_pts + target_expanded_h + extra_panel_pts;
        let panel_w = scale(panel_pts_w, dpi);
        let panel_h = scale(panel_pts_h, dpi);
        let panel_window = Rect {
            x: (screen_width - panel_w) / 2,
            y: 0,
            width: panel_w,
            height: panel_h,
        };
        let notification_layout = notch_input
            .notification
            .filter(|card| card.is_panel_card())
            .map(|card| compute_notification_layout(notch, dpi, card));
        let controls = compute_expanded_notch_controls(notch, dpi, &notch_input, metrics);
        let compact = compute_compact_notch_geometry(
            notch,
            bar_height,
            dpi,
            notch_input.privacy.compact_dots && panel_t <= 0.55,
            notch_input.media_live,
        );
        let notch_secondary = compute_secondary_notch_rect(
            notch,
            metrics,
            dpi,
            secondary_t * (1.0 - panel_t),
            swap_blend,
        );
        (
            notch,
            panel_window,
            notification_layout,
            controls,
            compact,
            notch_secondary,
            notch_input.compact_primary,
            notch_input.compact_secondary,
        )
    } else {
        (
            Rect::default(),
            Rect::default(),
            None,
            ExpandedNotchControls::default(),
            CompactNotchGeometry::default(),
            Rect::default(),
            None,
            None,
        )
    };

    let row = if notch_input.top_bar_enabled {
        let control_center_width = if notch_input.control_center_enabled {
            metrics.control_center_slot_width
        } else {
            0
        };
        let item_width = |item: BarItem, points: i32| {
            if notch_input.hidden_items.contains(item) {
                0
            } else {
                scale(points, dpi)
            }
        };
        let right_widths = [
            (
                HitTarget::Clock,
                item_width(BarItem::Clock, metrics.clock_slot_width),
            ),
            (
                HitTarget::Notifications,
                item_width(BarItem::Notifications, metrics.notification_slot_width),
            ),
            (HitTarget::ControlCenter, scale(control_center_width, dpi)),
            (
                HitTarget::Search,
                item_width(BarItem::Search, metrics.search_slot_width),
            ),
            (
                HitTarget::QuickSettings,
                item_width(BarItem::Status, metrics.status_slot_width),
            ),
            (
                HitTarget::Tray,
                item_width(BarItem::Tray, metrics.tray_slot_width),
            ),
        ];
        let right_start = if notch_enabled {
            notch.right().max(notch_secondary.right()) + right_gap
        } else {
            padding
        };
        let (right_slots, right_hidden) = resolve_right_slots(
            &right_widths,
            right_gap,
            right_start,
            screen_width - padding,
            bar_height,
        );
        let left_widths = [
            scale(metrics.start_slot_width, dpi),
            scale(estimate_app_slot_points(metrics, app_name), dpi),
            scale(metrics.file_slot_width, dpi),
            scale(metrics.edit_slot_width, dpi),
            scale(metrics.view_slot_width, dpi),
            scale(metrics.window_slot_width, dpi),
            scale(metrics.help_slot_width, dpi),
        ];
        let left_limit = if notch_enabled {
            (notch.x - left_gap).max(padding)
        } else {
            let right_edge = right_slots
                .iter()
                .map(|(_, rect)| rect.x)
                .min()
                .unwrap_or(screen_width - padding);
            (right_edge - left_gap).max(padding)
        };
        let mut row = resolve_left_slots(&LeftSlotInput {
            widths: &left_widths,
            overflow_width: scale(metrics.overflow_slot_width, dpi),
            gap: left_gap,
            start: padding,
            limit: left_limit,
            height: bar_height,
            force_overflow: !right_hidden.is_empty(),
        });
        row.slots.extend(right_slots);
        row.hidden.extend(right_hidden);
        row
    } else {
        SlotRow {
            slots: Vec::new(),
            hidden: Vec::new(),
            overflow: Rect::default(),
        }
    };

    let start_slot = row.rect(HitTarget::Start);
    let app_slot = row.rect(HitTarget::AppMenu);
    let file_slot = row.rect(HitTarget::FileMenu);
    let edit_slot = row.rect(HitTarget::EditMenu);
    let view_slot = row.rect(HitTarget::ViewMenu);
    let window_slot = row.rect(HitTarget::WindowMenu);
    let help_slot = row.rect(HitTarget::HelpMenu);
    let clock_slot = row.rect(HitTarget::Clock);
    let notification_slot = row.rect(HitTarget::Notifications);
    let control_center_slot = row.rect(HitTarget::ControlCenter);
    let search_slot = row.rect(HitTarget::Search);
    let status_slot = row.rect(HitTarget::QuickSettings);
    let icon_size = scale(metrics.icon_size, dpi);
    let (status_wifi_rect, status_volume_rect, status_battery_rect) =
        compute_status_sub_rects(status_slot, icon_size);
    let tray_slot = row.rect(HitTarget::Tray);

    let compact_corner_radius = if metrics.notch_top_inset > 0 {
        compact_height_pts / 2
    } else {
        metrics.notch_corner_radius
    };
    let corner_pts = lerp_points(
        compact_corner_radius,
        metrics.notch_expanded_corner_radius,
        panel_t,
    );
    let ear_pts = lerp_points(
        metrics.notch_ear_radius,
        metrics.notch_expanded_ear_radius,
        panel_t,
    );

    Layout {
        bar,
        notch,
        notch_secondary,
        panel_window,
        notch_corner_radius: scale(corner_pts, dpi),
        notch_ear_radius: scale(ear_pts, dpi),
        panel_expansion: panel_t,
        compact_expansion: compact_t,
        secondary_expansion: secondary_t * (1.0 - panel_t),
        compact_swap_blend: swap_blend,
        compact_primary,
        compact_secondary,
        focus_mode: notch_input.focus_mode,
        active_tab: notch_input.active_tab,
        previous_tab: notch_input.previous_tab,
        compact_ring: compact.ring,
        compact_peek: compact.peek,
        compact_cycle_steps: compact.cycle_steps,
        compact_time: compact.time,
        compact_title: compact.title,
        compact_trailing: compact.trailing,
        notch_tab_strip: controls.notch_tab_strip,
        notch_tab_pomodoro: controls.notch_tab_pomodoro,
        notch_tab_stats: controls.notch_tab_stats,
        notch_tab_shelf: controls.notch_tab_shelf,
        notch_tab_clipboard: controls.notch_tab_clipboard,
        notch_tab_tasks: controls.notch_tab_tasks,
        notch_tab_media: controls.notch_tab_media,
        lyrics_progress: notch_input.lyrics_progress.clamp(0.0, 1.0),
        stats_progress: stats_t,
        tab_blend: notch_input.tab_blend.clamp(0.0, 1.0),
        press_progress,
        icon_size,
        notification: notification_layout,
        hud: hud_geometry,
        pomodoro: PomodoroPanelLayout {
            surface: notch_input.pomodoro_surface,
            icon: controls.icon,
            time: controls.time,
            subtitle: controls.subtitle,
            session_label: controls.session_label,
            segments: controls.segments,
            footer_left: controls.footer_left,
            footer_right: controls.footer_right,
            presets: controls.presets,
            focus_modes: controls.focus_modes,
            play_button: controls.play_button,
            reset_button: controls.reset_button,
            skip_button: controls.skip_button,
            alert_start_button: controls.alert_start_button,
            alert_extend_button: controls.alert_extend_button,
        },
        stats: FocusStatsPanelLayout {
            body: controls.stats_body,
            range_today: controls.stats_range_today,
            range_week: controls.stats_range_week,
            open_settings: controls.stats_open_settings,
            days: controls.stats_days,
        },
        shelf: ShelfPanelLayout {
            add_button: controls.shelf_add_button,
            clear_button: controls.shelf_clear_button,
            pager_button: controls.shelf_pager_button,
            dropzone: controls.shelf_dropzone,
            cards: controls.shelf_cards,
            copy_buttons: controls.shelf_copy_buttons,
            remove_buttons: controls.shelf_remove_buttons,
        },
        clipboard: ClipboardPanelLayout {
            pause_button: controls.clipboard_pause_button,
            clear_button: controls.clipboard_clear_button,
            pager_button: controls.clipboard_pager_button,
            body: controls.clipboard_body,
            rows: controls.clipboard_rows,
            remove_buttons: controls.clipboard_remove_buttons,
        },
        tasks: TasksPanelLayout {
            add_button: controls.tasks_add_button,
            clear_button: controls.tasks_clear_button,
            pager_button: controls.tasks_pager_button,
            editor: controls.tasks_editor,
            editor_input: controls.tasks_editor_input,
            body: controls.tasks_body,
            rows: controls.tasks_rows,
            check_buttons: controls.tasks_check_buttons,
            edit_buttons: controls.tasks_edit_buttons,
            remove_buttons: controls.tasks_remove_buttons,
        },
        media: MediaPanelLayout {
            cover_art: controls.media_cover_art,
            source_label: controls.media_source_label,
            title: controls.media_title,
            artist: controls.media_artist,
            reason: controls.media_reason,
            equalizer: controls.media_equalizer,
            lyrics_toggle: controls.media_lyrics_toggle,
            lyrics_drawer: controls.media_lyrics_drawer,
            seekbar: controls.media_seekbar,
            play_button: controls.media_play_button,
            prev_button: controls.media_prev_button,
            next_button: controls.media_next_button,
        },
        privacy: PrivacyPanelLayout {
            dots: compact.privacy_dots,
            mic_button: controls.privacy_mic_button,
        },
        slots: BarSlotsLayout {
            start_slot,
            app_slot,
            file_slot,
            edit_slot,
            view_slot,
            window_slot,
            help_slot,
            overflow_slot: row.overflow,
            hidden_targets: row.hidden,
            tray_slot,
            status_slot,
            status_wifi_rect,
            status_volume_rect,
            status_battery_rect,
            search_slot,
            control_center_slot,
            notification_slot,
            clock_slot,
        },
    }
}

pub(super) fn compute_status_sub_rects(status_slot: Rect, icon_size: i32) -> (Rect, Rect, Rect) {
    if status_slot.is_empty() {
        return (Rect::default(), Rect::default(), Rect::default());
    }
    let unit = icon_size.max(1);
    let wifi_end =
        (status_slot.x + (unit * 148 + 50) / 100).clamp(status_slot.x, status_slot.right());
    let volume_end = (status_slot.x + (unit * 256 + 50) / 100).clamp(wifi_end, status_slot.right());
    let wifi = Rect {
        x: status_slot.x,
        y: status_slot.y,
        width: (wifi_end - status_slot.x).max(0),
        height: status_slot.height,
    };
    let volume = Rect {
        x: wifi_end,
        y: status_slot.y,
        width: (volume_end - wifi_end).max(0),
        height: status_slot.height,
    };
    let battery = Rect {
        x: volume_end,
        y: status_slot.y,
        width: (status_slot.right() - volume_end).max(0),
        height: status_slot.height,
    };
    (wifi, volume, battery)
}
