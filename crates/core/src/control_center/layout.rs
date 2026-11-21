use crate::bar::status::PowerMode;
use crate::dock::layout::{DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL};
use crate::layout::{Rect, scale};

use super::layout_types::ControlCenterHitTarget;
use super::layout_types::ControlCenterLayout;
use super::layout_types::ControlCenterMetrics;
use super::layout_types::ControlCenterPlacement;
use super::model::ControlCenterPage;
use super::snapshot::ControlCenterSnapshot;
use super::snapshot::SubPanelGroupLayout;
use super::snapshot::SubPanelGroupSpec;

#[must_use]
pub const fn next_dock_icon_size(current: i32) -> i32 {
    match current {
        DOCK_ICON_SMALL => DOCK_ICON_MEDIUM,
        DOCK_ICON_MEDIUM => DOCK_ICON_LARGE,
        _ => DOCK_ICON_SMALL,
    }
}

#[must_use]
pub const fn power_mode_label(mode: PowerMode) -> &'static str {
    match mode {
        PowerMode::BatterySaver => "Battery Saver",
        PowerMode::Standard => "Standard",
    }
}

#[must_use]
pub fn compute_control_center_layout_for_snapshot(
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    snapshot: &ControlCenterSnapshot,
) -> ControlCenterLayout {
    let groups = snapshot.subpanel_groups();
    compute_control_center_layout_with(metrics, placement, &groups)
}

pub(super) fn compute_control_center_layout_with(
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
) -> ControlCenterLayout {
    let dpi = placement.dpi.max(96);
    let width = scale(metrics.width, dpi);
    let pad = scale(metrics.padding, dpi);
    let gap = scale(metrics.gap, dpi);
    let inner_w = (width - pad * 2).max(scale(120, dpi));

    let mut layout = ControlCenterLayout {
        page: placement.page,
        shell_radius: scale(metrics.shell_radius, dpi),
        card_radius: scale(metrics.card_radius, dpi),
        row_height: scale(metrics.row_height, dpi),
        ..ControlCenterLayout::default()
    };

    let height = if placement.page == ControlCenterPage::Main {
        layout_main_page(&mut layout, metrics, dpi, pad, gap, inner_w)
    } else {
        layout_subpanel_page(
            &mut layout,
            metrics,
            placement,
            groups,
            dpi,
            pad,
            gap,
            inner_w,
        )
    };

    let margin = scale(8, dpi);
    let max_x = (placement.bounds.right() - width - margin).max(placement.bounds.x);
    let x = (placement.anchor_right - width).clamp(
        placement.bounds.x + margin.min(max_x - placement.bounds.x),
        max_x,
    );
    let max_y = (placement.bounds.bottom() - height - margin).max(placement.bounds.y);
    let y = placement.anchor_y.clamp(placement.bounds.y, max_y);

    layout.window = Rect {
        x,
        y,
        width,
        height,
    };
    layout
}

pub(super) fn layout_main_page(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let y2 = layout_main_top(layout, metrics, dpi, pad, gap, inner_w) + gap;
    let y4 = layout_main_sliders(layout, metrics, dpi, pad, gap, inner_w, y2) + gap;
    let h4 = layout_main_bottom(layout, metrics, dpi, pad, gap, inner_w, y4);
    y4 + h4 + pad
}

pub(super) fn layout_main_top(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let y1 = pad;
    let h1 = scale(metrics.connectivity_height, dpi);
    let left_w = (inner_w - gap) / 2;
    let right_w = inner_w - gap - left_w;
    let right_x = pad + left_w + gap;

    layout.connectivity_card = Rect {
        x: pad,
        y: y1,
        width: left_w,
        height: h1,
    };
    layout_connectivity_rows(layout, dpi);

    let split_h = (h1 - gap) / 2;
    layout.focus_toggle = Rect {
        x: right_x,
        y: y1,
        width: right_w,
        height: split_h,
    };
    let gear_d = scale(20, dpi);
    layout.focus_settings = Rect {
        x: layout.focus_toggle.right() - gear_d - scale(6, dpi),
        y: y1 + scale(6, dpi),
        width: gear_d,
        height: gear_d,
    };
    layout.battery_tile = Rect {
        x: right_x,
        y: layout.focus_toggle.bottom() + gap,
        width: right_w,
        height: h1 - gap - split_h,
    };
    y1 + h1
}

pub(super) fn layout_main_sliders(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y2: i32,
) -> i32 {
    let card_pad = scale(10, dpi);
    let card_hdr_h = scale(24, dpi);
    let slider_h = scale(26, dpi);
    let slider_x = pad + card_pad;
    let slider_w = (inner_w - card_pad * 2).max(1);

    let h2 = scale(metrics.display_card_height, dpi);
    let display_row_y = y2 + card_hdr_h + scale(4, dpi);
    layout.display_card = Rect {
        x: pad,
        y: y2,
        width: inner_w,
        height: h2,
    };
    layout.display_header = Rect {
        x: pad,
        y: y2,
        width: inner_w,
        height: card_hdr_h,
    };
    layout.display_slider = Rect {
        x: slider_x,
        y: display_row_y,
        width: slider_w,
        height: slider_h,
    };
    layout.display_icon = Rect {
        x: slider_x + scale(2, dpi),
        y: display_row_y,
        width: slider_h,
        height: slider_h,
    };

    let y3 = y2 + h2 + gap;
    let h3 = scale(metrics.sound_card_height, dpi);
    let sound_row_y = y3 + card_hdr_h + scale(4, dpi);
    layout.sound_card = Rect {
        x: pad,
        y: y3,
        width: inner_w,
        height: h3,
    };
    layout.sound_header = Rect {
        x: pad,
        y: y3,
        width: inner_w,
        height: card_hdr_h,
    };
    let mute_gap = scale(8, dpi);
    layout.sound_mute = Rect {
        x: slider_x,
        y: sound_row_y,
        width: slider_h,
        height: slider_h,
    };
    layout.sound_slider = Rect {
        x: slider_x + slider_h + mute_gap,
        y: sound_row_y,
        width: (slider_w - slider_h - mute_gap).max(1),
        height: slider_h,
    };
    y3 + h3
}

pub(super) fn layout_main_bottom(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y4: i32,
) -> i32 {
    let h4 = (scale(metrics.bottom_row_height, dpi) - gap - scale(46, dpi)).max(scale(64, dpi));
    let media_h = h4;
    layout.media_card = Rect {
        x: pad,
        y: y4,
        width: inner_w,
        height: media_h,
    };

    let btn_d = scale(28, dpi);
    let btn_gap = scale(6, dpi);
    let right_pad = scale(10, dpi);
    let controls_w = btn_d * 3 + btn_gap * 2;
    let btn_x0 = layout.media_card.right() - right_pad - controls_w;
    let btn_y = y4 + (media_h - btn_d) / 2;

    layout.media_body = Rect {
        x: pad,
        y: y4,
        width: (btn_x0 - pad - scale(4, dpi)).max(scale(80, dpi)),
        height: media_h,
    };
    layout.media_prev = Rect {
        x: btn_x0,
        y: btn_y,
        width: btn_d,
        height: btn_d,
    };
    layout.media_play_pause = Rect {
        x: layout.media_prev.right() + btn_gap,
        y: btn_y,
        width: btn_d,
        height: btn_d,
    };
    layout.media_next = Rect {
        x: layout.media_play_pause.right() + btn_gap,
        y: btn_y,
        width: btn_d,
        height: btn_d,
    };

    h4
}

pub(super) fn layout_connectivity_rows(layout: &mut ControlCenterLayout, dpi: u32) {
    let card = layout.connectivity_card;
    let inset = scale(8, dpi);
    let row_gap = scale(6, dpi);
    let slot_h = ((card.height - inset * 2 - row_gap) / 2).max(scale(40, dpi));
    let circle_d = scale(32, dpi).min(slot_h - scale(6, dpi));
    let row_x = card.x + scale(4, dpi);
    let row_w = (card.width - scale(8, dpi)).max(1);
    let circle_x = row_x + scale(6, dpi);

    let y0 = card.y + inset;
    layout.wifi_row = Rect {
        x: row_x,
        y: y0,
        width: row_w,
        height: slot_h,
    };
    layout.wifi_toggle = Rect {
        x: circle_x,
        y: y0 + (slot_h - circle_d) / 2,
        width: circle_d,
        height: circle_d,
    };

    let y1 = y0 + slot_h + row_gap;
    layout.bluetooth_row = Rect {
        x: row_x,
        y: y1,
        width: row_w,
        height: slot_h,
    };
    layout.bluetooth_toggle = Rect {
        x: circle_x,
        y: y1 + (slot_h - circle_d) / 2,
        width: circle_d,
        height: circle_d,
    };
}

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_subpanel_page(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let mut y = layout_subpanel_header(layout, metrics, placement.page, dpi, pad, gap, inner_w);
    match placement.page {
        ControlCenterPage::Sound => {
            y = layout_subpanel_sound(layout, metrics, dpi, pad, gap, inner_w, y);
        }
        ControlCenterPage::Display => {
            y = layout_subpanel_display(layout, metrics, dpi, pad, gap, inner_w, y);
        }
        ControlCenterPage::Main | ControlCenterPage::Wifi | ControlCenterPage::Bluetooth => {}
    }

    let footer_h = scale(metrics.footer_height, dpi);
    if placement.page != ControlCenterPage::Display {
        y = layout_subpanel_list(
            layout, metrics, placement, groups, dpi, pad, gap, inner_w, y, footer_h,
        );
    }
    layout_subpanel_footer(layout, placement.page, pad, gap, inner_w, y, footer_h);
    y + footer_h + pad
}

pub(super) fn layout_subpanel_header(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    page: ControlCenterPage,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
) -> i32 {
    let hdr_h = scale(metrics.header_height, dpi);
    layout.header_rect = Rect {
        x: pad,
        y: pad,
        width: inner_w,
        height: hdr_h,
    };
    let back_d = scale(28, dpi);
    layout.back_button = Rect {
        x: pad,
        y: pad + (hdr_h - back_d) / 2,
        width: back_d,
        height: back_d,
    };
    if matches!(page, ControlCenterPage::Wifi | ControlCenterPage::Bluetooth) {
        let toggle_w = scale(40, dpi);
        let toggle_h = scale(22, dpi);
        layout.header_toggle = Rect {
            x: pad + inner_w - toggle_w,
            y: pad + (hdr_h - toggle_h) / 2,
            width: toggle_w,
            height: toggle_h,
        };
    }
    pad + hdr_h + gap
}

pub(super) fn layout_subpanel_sound(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
) -> i32 {
    let slider_card_h = scale(metrics.slider_card_height, dpi);
    let card_pad = scale(10, dpi);
    let card_hdr_h = scale(24, dpi);
    let slider_h = scale(26, dpi);
    layout.sound_card = Rect {
        x: pad,
        y,
        width: inner_w,
        height: slider_card_h,
    };
    layout.sound_header = Rect {
        x: pad,
        y,
        width: inner_w,
        height: card_hdr_h,
    };
    let slider_x = pad + card_pad;
    let slider_w = (inner_w - card_pad * 2).max(1);
    let mute_gap = scale(8, dpi);
    let slider_y = y + card_hdr_h + scale(4, dpi);
    layout.sound_mute = Rect {
        x: slider_x,
        y: slider_y,
        width: slider_h,
        height: slider_h,
    };
    layout.sound_slider = Rect {
        x: slider_x + slider_h + mute_gap,
        y: slider_y,
        width: (slider_w - slider_h - mute_gap).max(1),
        height: slider_h,
    };
    y + slider_card_h + gap
}

pub(super) fn layout_subpanel_display(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
) -> i32 {
    let slider_card_h = scale(metrics.slider_card_height, dpi);
    let card_pad = scale(10, dpi);
    let card_hdr_h = scale(24, dpi);
    let slider_h = scale(26, dpi);
    layout.display_card = Rect {
        x: pad,
        y,
        width: inner_w,
        height: slider_card_h,
    };
    let dx = pad + card_pad;
    let dy = y + card_hdr_h + scale(4, dpi);
    layout.display_slider = Rect {
        x: dx,
        y: dy,
        width: (inner_w - card_pad * 2).max(1),
        height: slider_h,
    };
    layout.display_icon = Rect {
        x: dx + scale(2, dpi),
        y: dy,
        width: slider_h,
        height: slider_h,
    };
    y + slider_card_h + gap
}

pub(super) fn group_content_height(group: &SubPanelGroupSpec, row_h: i32) -> i32 {
    i32::try_from(group.count).unwrap_or(64) * row_h
}

pub(super) fn compute_subpanel_list_total_height(
    non_empty: &[SubPanelGroupSpec],
    row_h: i32,
    group_hdr_h: i32,
    group_gap: i32,
    pad_v: i32,
) -> i32 {
    if non_empty.is_empty() {
        return row_h;
    }
    let mut total_h = pad_v;
    for (idx, group) in non_empty.iter().enumerate() {
        if idx > 0 {
            total_h += group_gap;
        }
        if !group.title.is_empty() {
            total_h += group_hdr_h;
        }
        total_h += group_content_height(group, row_h);
    }
    total_h
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn layout_subpanel_list(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
    dpi: u32,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
    footer_h: i32,
) -> i32 {
    let row_h = scale(metrics.row_height, dpi);
    let group_hdr_h = scale(metrics.group_header_height, dpi);
    let group_gap = scale(metrics.group_gap, dpi);
    let bottom_pad = scale(4, dpi);

    let fallback = [SubPanelGroupSpec {
        title: "",
        count: placement.item_count,
    }];
    let specs: &[SubPanelGroupSpec] = if groups.is_empty() && placement.item_count > 0 {
        &fallback
    } else {
        groups
    };

    let non_empty: Vec<SubPanelGroupSpec> = specs.iter().copied().filter(|g| g.count > 0).collect();
    let total_h =
        compute_subpanel_list_total_height(&non_empty, row_h, group_hdr_h, group_gap, bottom_pad);

    let cap_rows = metrics.max_visible_rows.max(1);
    let cap_h = cap_rows * row_h + group_hdr_h * 2 + group_gap + scale(18, dpi);
    let max_win_h = (placement.bounds.height - scale(16, dpi)).max(scale(260, dpi));
    let footer_reserve = gap + footer_h + pad;
    let avail_h = (max_win_h - y - footer_reserve).max(row_h);
    let viewport_h = total_h.min(cap_h).min(avail_h);

    layout.list_viewport = Rect {
        x: pad,
        y,
        width: inner_w,
        height: viewport_h,
    };
    layout.max_scroll = (total_h - viewport_h).max(0);
    layout.scroll_offset = placement.scroll_offset.clamp(0, layout.max_scroll);

    let content_w = if layout.max_scroll > 0 {
        (inner_w - scale(8, dpi)).max(1)
    } else {
        inner_w
    };

    layout.items.reserve(placement.item_count);
    let mut cursor_y = y - layout.scroll_offset;
    for (g_idx, group) in non_empty.iter().enumerate() {
        if g_idx > 0 {
            cursor_y += group_gap;
        }
        let header = if group.title.is_empty() {
            Rect::default()
        } else {
            let h = Rect {
                x: pad,
                y: cursor_y,
                width: content_w,
                height: group_hdr_h,
            };
            cursor_y += group_hdr_h;
            h
        };
        let start_index = layout.items.len();
        let container_y = cursor_y;
        let group_h = group_content_height(group, row_h);
        let container = Rect {
            x: pad,
            y: container_y,
            width: content_w,
            height: group_h,
        };
        for _ in 0..group.count {
            if layout.items.len() >= placement.item_count {
                break;
            }
            layout.items.push(Rect {
                x: pad,
                y: cursor_y,
                width: content_w,
                height: row_h,
            });
            cursor_y += row_h;
        }
        let actual_count = layout.items.len() - start_index;
        if actual_count > 0 {
            layout.groups.push(SubPanelGroupLayout {
                title: group.title,
                header,
                container,
                start_index,
                count: actual_count,
            });
        }
    }

    while layout.items.len() < placement.item_count {
        layout.items.push(Rect {
            x: pad,
            y: cursor_y,
            width: content_w,
            height: row_h,
        });
        cursor_y += row_h;
    }

    y + viewport_h + gap
}

pub(super) fn layout_subpanel_footer(
    layout: &mut ControlCenterLayout,
    page: ControlCenterPage,
    pad: i32,
    gap: i32,
    inner_w: i32,
    y: i32,
    footer_h: i32,
) {
    if matches!(page, ControlCenterPage::Display) {
        let half = (inner_w - gap) / 2;
        layout.footer_primary = Rect {
            x: pad,
            y,
            width: half,
            height: footer_h,
        };
        layout.footer_secondary = Rect {
            x: pad + half + gap,
            y,
            width: inner_w - gap - half,
            height: footer_h,
        };
    } else {
        layout.footer_primary = Rect {
            x: pad,
            y,
            width: inner_w,
            height: footer_h,
        };
    }
}

impl ControlCenterLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<ControlCenterHitTarget> {
        if self.page == ControlCenterPage::Main {
            return self.hit_test_main(x, y);
        }
        if self.back_button.contains(x, y) {
            return Some(ControlCenterHitTarget::BackButton);
        }
        if self.header_toggle.contains(x, y) {
            return Some(ControlCenterHitTarget::HeaderToggle);
        }
        if self.page == ControlCenterPage::Display && self.display_slider.contains(x, y) {
            return Some(ControlCenterHitTarget::DisplaySlider);
        }
        if self.page == ControlCenterPage::Sound {
            if self.sound_mute.contains(x, y) {
                return Some(ControlCenterHitTarget::SoundMuteToggle);
            }
            if self.sound_slider.contains(x, y) {
                return Some(ControlCenterHitTarget::SoundSlider);
            }
        }
        if self.list_viewport.contains(x, y) {
            for (index, rect) in self.items.iter().enumerate() {
                if rect.contains(x, y) {
                    return Some(ControlCenterHitTarget::SubPanelItem(index));
                }
            }
        }
        if self.footer_primary.contains(x, y) {
            return Some(ControlCenterHitTarget::FooterPrimary);
        }
        if self.footer_secondary.contains(x, y) {
            return Some(ControlCenterHitTarget::FooterSecondary);
        }
        None
    }

    pub(super) fn hit_test_main(&self, x: i32, y: i32) -> Option<ControlCenterHitTarget> {
        let direct = [
            (self.wifi_toggle, ControlCenterHitTarget::WifiToggle),
            (self.wifi_row, ControlCenterHitTarget::WifiOpenPage),
            (
                self.bluetooth_toggle,
                ControlCenterHitTarget::BluetoothToggle,
            ),
            (
                self.bluetooth_row,
                ControlCenterHitTarget::BluetoothOpenPage,
            ),
            (
                self.focus_settings,
                ControlCenterHitTarget::FocusOpenSettings,
            ),
            (self.focus_toggle, ControlCenterHitTarget::FocusToggle),
            (self.battery_tile, ControlCenterHitTarget::BatteryTile),
            (self.display_slider, ControlCenterHitTarget::DisplaySlider),
            (self.display_header, ControlCenterHitTarget::DisplayOpenPage),
            (self.sound_mute, ControlCenterHitTarget::SoundMuteToggle),
            (self.sound_slider, ControlCenterHitTarget::SoundSlider),
            (self.sound_header, ControlCenterHitTarget::SoundOpenPage),
            (self.media_prev, ControlCenterHitTarget::MediaPrev),
            (
                self.media_play_pause,
                ControlCenterHitTarget::MediaPlayPause,
            ),
            (self.media_next, ControlCenterHitTarget::MediaNext),
            (self.media_body, ControlCenterHitTarget::MediaBody),
        ];
        for (rect, target) in direct {
            if rect.contains(x, y) {
                return Some(target);
            }
        }
        None
    }

    #[must_use]
    pub fn slider_rect(&self, target: ControlCenterHitTarget) -> Option<Rect> {
        match target {
            ControlCenterHitTarget::DisplaySlider if !self.display_slider.is_empty() => {
                Some(self.display_slider)
            }
            ControlCenterHitTarget::SoundSlider if !self.sound_slider.is_empty() => {
                Some(self.sound_slider)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn slider_value_at(&self, target: ControlCenterHitTarget, x: i32) -> Option<u8> {
        let rect = self.slider_rect(target)?;
        if x <= rect.x {
            return Some(0);
        }
        if x >= rect.right() {
            return Some(100);
        }
        let rel = i64::from(x - rect.x);
        let width = i64::from(rect.width.max(1));
        let percent = ((rel * 100 + width / 2) / width).clamp(0, 100);
        u8::try_from(percent).ok()
    }

    #[must_use]
    pub fn target_rect(&self, target: ControlCenterHitTarget) -> Option<Rect> {
        let rect = match target {
            ControlCenterHitTarget::WifiToggle => self.wifi_toggle,
            ControlCenterHitTarget::WifiOpenPage => self.wifi_row,
            ControlCenterHitTarget::BluetoothToggle => self.bluetooth_toggle,
            ControlCenterHitTarget::BluetoothOpenPage => self.bluetooth_row,
            ControlCenterHitTarget::FocusToggle => self.focus_toggle,
            ControlCenterHitTarget::FocusOpenSettings => self.focus_settings,
            ControlCenterHitTarget::BatteryTile => self.battery_tile,
            ControlCenterHitTarget::DisplayOpenPage => self.display_header,
            ControlCenterHitTarget::DisplaySlider => self.display_slider,
            ControlCenterHitTarget::SoundOpenPage => self.sound_header,
            ControlCenterHitTarget::SoundMuteToggle => self.sound_mute,
            ControlCenterHitTarget::SoundSlider => self.sound_slider,
            ControlCenterHitTarget::MediaBody => self.media_body,
            ControlCenterHitTarget::MediaPrev => self.media_prev,
            ControlCenterHitTarget::MediaPlayPause => self.media_play_pause,
            ControlCenterHitTarget::MediaNext => self.media_next,
            ControlCenterHitTarget::BackButton => self.back_button,
            ControlCenterHitTarget::HeaderToggle => self.header_toggle,
            ControlCenterHitTarget::SubPanelItem(index) => {
                let item = *self.items.get(index)?;
                return self.list_viewport.intersection(&item);
            }
            ControlCenterHitTarget::FooterPrimary => self.footer_primary,
            ControlCenterHitTarget::FooterSecondary => self.footer_secondary,
        };
        (!rect.is_empty()).then_some(rect)
    }
}
