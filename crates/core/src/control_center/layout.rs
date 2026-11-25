use crate::bar::status::PowerMode;
use crate::dock::layout::{DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL};
use crate::layout::{Rect, scale};

use super::grid::CONTROL_GRID_COLUMNS;
use super::layout_types::{ControlCenterLayout, ControlCenterMetrics, ControlCenterPlacement};
use super::model::ControlCenterPage;
use super::snapshot::{ControlCenterSnapshot, SubPanelGroupLayout, SubPanelGroupSpec};
use super::tile_layout::{build_tile_layout, sync_legacy_rects_from_tiles};

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
    compute_control_center_layout_with(metrics, placement, &groups, snapshot)
}

pub(super) fn compute_control_center_layout_with(
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    groups: &[SubPanelGroupSpec],
    snapshot: &ControlCenterSnapshot,
) -> ControlCenterLayout {
    let dpi = placement.dpi.max(96);
    let mut layout = ControlCenterLayout {
        window: placement.bounds,
        page: placement.page,
        shell_radius: scale(metrics.shell_radius, dpi),
        card_radius: scale(metrics.card_radius, dpi),
        row_height: scale(
            if placement.page == ControlCenterPage::Gallery {
                metrics.gallery_row_height
            } else {
                metrics.row_height
            },
            dpi,
        ),
        ..ControlCenterLayout::default()
    };

    if placement.page == ControlCenterPage::Main {
        layout_canvas_main(&mut layout, metrics, placement, snapshot, dpi);
        return layout;
    }

    let panel_base_w = if placement.page == ControlCenterPage::Gallery {
        metrics.gallery_width
    } else {
        metrics.width
    };
    let panel_w = scale(panel_base_w, dpi);
    let pad = scale(metrics.padding, dpi);
    let gap = scale(metrics.gap, dpi);
    let inner_w = (panel_w - pad * 2).max(scale(120, dpi));

    let panel_h = layout_subpanel_page(
        &mut layout,
        metrics,
        placement,
        groups,
        dpi,
        pad,
        gap,
        inner_w,
    );

    let margin = scale(metrics.canvas_right_margin, dpi);
    let top_gap = scale(metrics.canvas_top_gap, dpi);
    let max_x = (placement.bounds.right() - panel_w - margin).max(placement.bounds.x);
    let panel_x = (placement.anchor_right - panel_w).clamp(
        placement.bounds.x + margin.min(max_x - placement.bounds.x),
        max_x,
    );
    let max_y = (placement.bounds.bottom() - panel_h - margin).max(placement.bounds.y);
    let panel_y = (placement.anchor_y + top_gap).clamp(placement.bounds.y, max_y);

    layout.canvas_panel = Rect {
        x: panel_x,
        y: panel_y,
        width: panel_w,
        height: panel_h,
    };
    offset_subpanel_rects(&mut layout, panel_x, panel_y);
    layout
}

fn layout_canvas_main(
    layout: &mut ControlCenterLayout,
    metrics: &ControlCenterMetrics,
    placement: ControlCenterPlacement,
    snapshot: &ControlCenterSnapshot,
    dpi: u32,
) {
    let cell_px = scale(metrics.cell_size, dpi);
    let gap_px = scale(metrics.cell_gap, dpi);
    let grid_w =
        cell_px * i32::from(CONTROL_GRID_COLUMNS) + gap_px * i32::from(CONTROL_GRID_COLUMNS - 1);
    let right_margin = scale(metrics.canvas_right_margin, dpi);
    let top_gap = scale(metrics.canvas_top_gap, dpi);
    let toolbar_h = scale(metrics.toolbar_height, dpi);
    let toolbar_gap = scale(10, dpi);
    let add_btn_h = scale(38, dpi);
    let add_btn_gap = scale(14, dpi);

    let grid_rows = snapshot.grid.row_count().max(1);
    let grid_h = i32::from(grid_rows) * cell_px + i32::from(grid_rows.saturating_sub(1)) * gap_px;
    let footer_extra = if snapshot.editing {
        add_btn_gap + add_btn_h
    } else {
        0
    };
    let total_h = toolbar_h + toolbar_gap + grid_h + footer_extra;

    let max_x = (placement.bounds.right() - grid_w - right_margin).max(placement.bounds.x);
    let grid_x = (placement.anchor_right - grid_w).clamp(
        placement.bounds.x + right_margin.min(max_x - placement.bounds.x),
        max_x,
    );
    let max_y = (placement.bounds.bottom() - total_h - right_margin).max(placement.bounds.y);
    let toolbar_y = (placement.anchor_y + top_gap)
        .max(placement.bounds.y + scale(8, dpi))
        .min(max_y);
    let grid_y = toolbar_y + toolbar_h + toolbar_gap;

    layout.canvas_panel = Rect {
        x: grid_x,
        y: toolbar_y,
        width: grid_w,
        height: total_h,
    };
    layout.grid_origin = (grid_x, grid_y);
    layout.cell_px = cell_px;
    layout.cell_gap_px = gap_px;
    layout.grid_rows = grid_rows;

    layout.toolbar_add = Rect {
        x: grid_x,
        y: toolbar_y,
        width: toolbar_h,
        height: toolbar_h,
    };
    let edit_w = scale(72, dpi);
    layout.toolbar_edit = Rect {
        x: grid_x + grid_w - edit_w,
        y: toolbar_y,
        width: edit_w,
        height: toolbar_h,
    };
    if snapshot.editing {
        let reset_w = scale(72, dpi);
        layout.toolbar_reset = Rect {
            x: layout.toolbar_edit.x - scale(8, dpi) - reset_w,
            y: toolbar_y,
            width: reset_w,
            height: toolbar_h,
        };
        let btn_w = scale(188, dpi).min(grid_w);
        layout.add_control_button = Rect {
            x: grid_x + (grid_w - btn_w) / 2,
            y: grid_y + grid_h + add_btn_gap,
            width: btn_w,
            height: add_btn_h,
        };
    }

    layout.tiles = snapshot
        .grid
        .tiles
        .iter()
        .map(|&tile| build_tile_layout(tile, (grid_x, grid_y), cell_px, gap_px, metrics, dpi))
        .collect();
    sync_legacy_rects_from_tiles(layout);
}

fn offset_rect(rect: &mut Rect, dx: i32, dy: i32) {
    if !rect.is_empty() {
        rect.x += dx;
        rect.y += dy;
    }
}

fn offset_subpanel_rects(layout: &mut ControlCenterLayout, dx: i32, dy: i32) {
    offset_rect(&mut layout.header_rect, dx, dy);
    offset_rect(&mut layout.back_button, dx, dy);
    offset_rect(&mut layout.header_toggle, dx, dy);
    offset_rect(&mut layout.display_card, dx, dy);
    offset_rect(&mut layout.display_header, dx, dy);
    offset_rect(&mut layout.display_icon, dx, dy);
    offset_rect(&mut layout.display_slider, dx, dy);
    offset_rect(&mut layout.sound_card, dx, dy);
    offset_rect(&mut layout.sound_header, dx, dy);
    offset_rect(&mut layout.sound_mute, dx, dy);
    offset_rect(&mut layout.sound_slider, dx, dy);
    offset_rect(&mut layout.list_viewport, dx, dy);
    offset_rect(&mut layout.footer_primary, dx, dy);
    offset_rect(&mut layout.footer_secondary, dx, dy);
    for group in &mut layout.groups {
        offset_rect(&mut group.header, dx, dy);
        offset_rect(&mut group.container, dx, dy);
    }
    for item in &mut layout.items {
        offset_rect(item, dx, dy);
    }
}

const CARD_PAD: i32 = 12;
const ROW_INSET: i32 = 4;
const CARD_HEADER_HEIGHT: i32 = 24;
const SLIDER_HEIGHT: i32 = 26;

pub(super) fn slider_card_parts(
    dpi: u32,
    pad: i32,
    inner_w: i32,
    y: i32,
    card_h: i32,
) -> (Rect, Rect, Rect, Rect) {
    let card_pad = scale(CARD_PAD, dpi);
    let row_inset = scale(ROW_INSET, dpi);
    let card_hdr_h = scale(CARD_HEADER_HEIGHT, dpi);
    let slider_h = scale(SLIDER_HEIGHT, dpi);
    let card = Rect {
        x: pad,
        y,
        width: inner_w,
        height: card_h,
    };
    let header = Rect {
        x: pad + row_inset,
        y: y + row_inset,
        width: (inner_w - row_inset * 2).max(1),
        height: card_hdr_h,
    };
    let slider_x = pad + card_pad;
    let slider_y = header.bottom() + scale(4, dpi);
    let slider_w = (inner_w - card_pad * 2).max(1);
    let slider = Rect {
        x: slider_x,
        y: slider_y,
        width: slider_w,
        height: slider_h,
    };
    let cap = Rect {
        x: slider_x,
        y: slider_y,
        width: slider_h,
        height: slider_h,
    };
    (card, header, cap, slider)
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
        ControlCenterPage::Main
        | ControlCenterPage::Wifi
        | ControlCenterPage::Bluetooth
        | ControlCenterPage::Gallery => {}
    }

    let footer_h = if placement.page == ControlCenterPage::Gallery {
        0
    } else {
        scale(metrics.footer_height, dpi)
    };
    if placement.page != ControlCenterPage::Display {
        y = layout_subpanel_list(
            layout, metrics, placement, groups, dpi, pad, gap, inner_w, y, footer_h,
        );
    }
    if footer_h > 0 {
        layout_subpanel_footer(layout, placement.page, pad, gap, inner_w, y, footer_h);
        y + footer_h + pad
    } else {
        y - gap + pad
    }
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
    let (card, header, mute, slider) = slider_card_parts(dpi, pad, inner_w, y, slider_card_h);
    layout.sound_card = card;
    layout.sound_header = header;
    layout.sound_mute = mute;
    layout.sound_slider = slider;
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
    let (card, header, icon, slider) = slider_card_parts(dpi, pad, inner_w, y, slider_card_h);
    layout.display_card = card;
    layout.display_header = header;
    layout.display_icon = icon;
    layout.display_slider = slider;
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
    let row_h = layout.row_height.max(1);
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

    let cap_rows = if placement.page == ControlCenterPage::Gallery {
        8
    } else {
        metrics.max_visible_rows.max(1)
    };
    let cap_h = cap_rows * row_h + group_hdr_h * 2 + group_gap + scale(18, dpi);
    let max_win_h = (placement.bounds.height - scale(48, dpi)).max(scale(260, dpi));
    let footer_reserve = if footer_h > 0 {
        gap + footer_h + pad
    } else {
        pad
    };
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
