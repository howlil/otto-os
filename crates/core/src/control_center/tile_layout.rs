use crate::layout::{Rect, scale};

use super::grid::{ControlKind, ControlTile};
use super::layout_types::{ControlCenterLayout, ControlCenterMetrics, ControlTileLayout};

pub(super) fn build_tile_layout(
    tile: ControlTile,
    origin: (i32, i32),
    cell_px: i32,
    gap_px: i32,
    metrics: &ControlCenterMetrics,
    dpi: u32,
) -> ControlTileLayout {
    let rect = Rect {
        x: origin.0 + i32::from(tile.col) * (cell_px + gap_px),
        y: origin.1 + i32::from(tile.row) * (cell_px + gap_px),
        width: i32::from(tile.cols) * cell_px + i32::from(tile.cols.saturating_sub(1)) * gap_px,
        height: i32::from(tile.rows) * cell_px + i32::from(tile.rows.saturating_sub(1)) * gap_px,
    };
    let radius = if tile.cols == 1 && tile.rows == 1 {
        rect.width.min(rect.height) / 2
    } else {
        scale(metrics.tile_radius, dpi).min(rect.width.min(rect.height) / 2)
    };
    let badge_d = scale(22, dpi);
    let remove_badge = Rect {
        x: rect.x - scale(4, dpi),
        y: rect.y - scale(4, dpi),
        width: badge_d,
        height: badge_d,
    };
    let handle_d = scale(24, dpi);
    let resize_handle = Rect {
        x: rect.right() - handle_d,
        y: rect.bottom() - handle_d,
        width: handle_d,
        height: handle_d,
    };
    let mut out = ControlTileLayout {
        id: tile.id,
        kind: tile.kind,
        col: tile.col,
        row: tile.row,
        cols: tile.cols,
        rows: tile.rows,
        rect,
        radius,
        remove_badge,
        resize_handle,
        slider_rect: Rect::default(),
        slider_vertical: false,
        primary_rect: Rect::default(),
        secondary_rect: Rect::default(),
        tertiary_rect: Rect::default(),
        detail_rect: Rect::default(),
    };
    populate_tile_subrects(&mut out, dpi);
    out
}

fn populate_tile_subrects(tile: &mut ControlTileLayout, dpi: u32) {
    match tile.kind {
        ControlKind::Connectivity => populate_connectivity_subrects(tile, dpi),
        ControlKind::Brightness | ControlKind::Volume => populate_slider_subrects(tile, dpi),
        ControlKind::NowPlaying => populate_media_subrects(tile, dpi),
        ControlKind::Focus => populate_focus_subrects(tile, dpi),
        ControlKind::Wifi | ControlKind::Bluetooth => populate_network_tile_subrects(tile, dpi),
        ControlKind::Battery
        | ControlKind::Mute
        | ControlKind::NightLight
        | ControlKind::Appearance
        | ControlKind::Screenshot
        | ControlKind::LockScreen
        | ControlKind::TaskView
        | ControlKind::Finder
        | ControlKind::Spotlight
        | ControlKind::Settings => {
            tile.primary_rect = tile.rect;
        }
    }
}

fn populate_connectivity_subrects(tile: &mut ControlTileLayout, dpi: u32) {
    let r = tile.rect;
    let pad = scale(10, dpi);
    if tile.cols == 1 && tile.rows == 1 {
        tile.primary_rect = r;
        return;
    }
    if tile.rows == 1 {
        let half_w = (r.width - scale(4, dpi)) / 2;
        tile.primary_rect = Rect {
            x: r.x,
            y: r.y,
            width: half_w,
            height: r.height,
        };
        tile.tertiary_rect = Rect {
            x: r.x + half_w + scale(4, dpi),
            y: r.y,
            width: r.width - half_w - scale(4, dpi),
            height: r.height,
        };
        return;
    }
    let row_inset = scale(4, dpi);
    let row_gap = scale(4, dpi);
    let slot_h = ((r.height - row_inset * 2 - row_gap) / 2).max(scale(32, dpi));
    let circle_d = scale(32, dpi).min(slot_h - scale(6, dpi));
    let row_x = r.x + row_inset;
    let row_w = (r.width - row_inset * 2).max(1);
    let circle_x = r.x + pad;

    let y0 = r.y + row_inset;
    tile.secondary_rect = Rect {
        x: row_x,
        y: y0,
        width: row_w,
        height: slot_h,
    };
    tile.primary_rect = Rect {
        x: circle_x,
        y: y0 + (slot_h - circle_d) / 2,
        width: circle_d,
        height: circle_d,
    };
    let y1 = y0 + slot_h + row_gap;
    tile.detail_rect = Rect {
        x: row_x,
        y: y1,
        width: row_w,
        height: slot_h,
    };
    tile.tertiary_rect = Rect {
        x: circle_x,
        y: y1 + (slot_h - circle_d) / 2,
        width: circle_d,
        height: circle_d,
    };
}

fn populate_slider_subrects(tile: &mut ControlTileLayout, dpi: u32) {
    let r = tile.rect;
    if tile.rows >= 2 && tile.cols == 1 {
        tile.slider_vertical = true;
        tile.slider_rect = r;
        if tile.kind == ControlKind::Volume {
            let cap_h = scale(34, dpi).min(r.height / 3);
            tile.primary_rect = Rect {
                x: r.x,
                y: r.bottom() - cap_h,
                width: r.width,
                height: cap_h,
            };
        }
        return;
    }
    tile.slider_vertical = false;
    if tile.cols >= 4 {
        let pad = scale(10, dpi);
        let hdr_h = scale(22, dpi);
        tile.detail_rect = Rect {
            x: r.x + scale(4, dpi),
            y: r.y + scale(4, dpi),
            width: (r.width - scale(8, dpi)).max(1),
            height: hdr_h,
        };
        let slider_h = scale(26, dpi);
        let slider_y = tile.detail_rect.bottom() + scale(4, dpi);
        tile.slider_rect = Rect {
            x: r.x + pad,
            y: slider_y,
            width: (r.width - pad * 2).max(1),
            height: slider_h,
        };
        if tile.kind == ControlKind::Volume {
            tile.primary_rect = Rect {
                x: tile.slider_rect.x,
                y: tile.slider_rect.y,
                width: slider_h,
                height: slider_h,
            };
        }
    } else {
        let inset = scale(8, dpi);
        let slider_h = (r.height - inset * 2).max(scale(24, dpi));
        let slider_y = r.y + (r.height - slider_h) / 2;
        tile.slider_rect = Rect {
            x: r.x + inset,
            y: slider_y,
            width: (r.width - inset * 2).max(1),
            height: slider_h,
        };
        if tile.kind == ControlKind::Volume {
            tile.primary_rect = Rect {
                x: tile.slider_rect.x,
                y: tile.slider_rect.y,
                width: slider_h.min(scale(30, dpi)),
                height: slider_h,
            };
        }
    }
}

fn populate_media_subrects(tile: &mut ControlTileLayout, dpi: u32) {
    let r = tile.rect;
    if tile.rows >= 2 && tile.cols <= 2 {
        let btn_d = scale(30, dpi);
        let btn_gap = scale(8, dpi);
        let row_w = btn_d * 3 + btn_gap * 2;
        let btn_y = r.bottom() - scale(12, dpi) - btn_d;
        let btn_x0 = r.x + (r.width - row_w) / 2;
        tile.primary_rect = Rect {
            x: r.x + scale(6, dpi),
            y: r.y + scale(6, dpi),
            width: r.width - scale(12, dpi),
            height: (btn_y - r.y - scale(10, dpi)).max(1),
        };
        tile.secondary_rect = Rect {
            x: btn_x0,
            y: btn_y,
            width: btn_d,
            height: btn_d,
        };
        tile.tertiary_rect = Rect {
            x: tile.secondary_rect.right() + btn_gap,
            y: btn_y,
            width: btn_d,
            height: btn_d,
        };
        tile.detail_rect = Rect {
            x: tile.tertiary_rect.right() + btn_gap,
            y: btn_y,
            width: btn_d,
            height: btn_d,
        };
        return;
    }
    let skip_d = scale(26, dpi);
    let play_d = scale(32, dpi);
    let btn_gap = scale(4, dpi);
    let controls_w = skip_d * 2 + play_d + btn_gap * 2;
    let btn_x0 = r.right() - scale(12, dpi) - controls_w;
    let skip_y = r.y + (r.height - skip_d) / 2;
    let play_y = r.y + (r.height - play_d) / 2;
    tile.primary_rect = Rect {
        x: r.x + scale(4, dpi),
        y: r.y + scale(4, dpi),
        width: (btn_x0 - (r.x + scale(10, dpi))).max(scale(40, dpi)),
        height: (r.height - scale(8, dpi)).max(1),
    };
    tile.secondary_rect = Rect {
        x: btn_x0,
        y: skip_y,
        width: skip_d,
        height: skip_d,
    };
    tile.tertiary_rect = Rect {
        x: tile.secondary_rect.right() + btn_gap,
        y: play_y,
        width: play_d,
        height: play_d,
    };
    tile.detail_rect = Rect {
        x: tile.tertiary_rect.right() + btn_gap,
        y: skip_y,
        width: skip_d,
        height: skip_d,
    };
}

fn populate_focus_subrects(tile: &mut ControlTileLayout, dpi: u32) {
    let r = tile.rect;
    tile.primary_rect = r;
    if tile.cols >= 2 {
        let gear_d = scale(24, dpi);
        tile.detail_rect = Rect {
            x: r.right() - gear_d - scale(8, dpi),
            y: r.y + (r.height - gear_d) / 2,
            width: gear_d,
            height: gear_d,
        };
    }
}

fn populate_network_tile_subrects(tile: &mut ControlTileLayout, dpi: u32) {
    let r = tile.rect;
    if tile.cols >= 2 {
        let icon_d = scale(32, dpi);
        let pad = scale(12, dpi);
        tile.primary_rect = Rect {
            x: r.x + pad,
            y: r.y + (r.height - icon_d) / 2,
            width: icon_d,
            height: icon_d,
        };
        tile.detail_rect = r;
    } else {
        tile.primary_rect = r;
    }
}

pub(super) fn sync_legacy_rects_from_tiles(layout: &mut ControlCenterLayout) {
    for tile in &layout.tiles {
        match tile.kind {
            ControlKind::Connectivity => {
                layout.connectivity_card = tile.rect;
                layout.wifi_toggle = tile.primary_rect;
                layout.wifi_row = tile.secondary_rect;
                layout.bluetooth_toggle = tile.tertiary_rect;
                layout.bluetooth_row = tile.detail_rect;
            }
            ControlKind::Focus => {
                layout.focus_toggle = tile.rect;
                layout.focus_icon = tile.primary_rect;
                layout.focus_settings = tile.detail_rect;
            }
            ControlKind::Battery => {
                layout.battery_tile = tile.rect;
                layout.battery_icon = tile.primary_rect;
            }
            ControlKind::Brightness => {
                layout.display_card = tile.rect;
                layout.display_header = tile.detail_rect;
                layout.display_slider = tile.slider_rect;
            }
            ControlKind::Volume => {
                layout.sound_card = tile.rect;
                layout.sound_header = tile.detail_rect;
                layout.sound_mute = tile.primary_rect;
                layout.sound_slider = tile.slider_rect;
            }
            ControlKind::NowPlaying => {
                layout.media_card = tile.rect;
                layout.media_body = tile.primary_rect;
                layout.media_prev = tile.secondary_rect;
                layout.media_play_pause = tile.tertiary_rect;
                layout.media_next = tile.detail_rect;
            }
            _ => {}
        }
    }
}
