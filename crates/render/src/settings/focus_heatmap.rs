use otto_core::{FocusHeatmapView, Rect, SettingsHit, SettingsRow, SettingsRowValue};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use crate::appearance::tokens::{SYSTEM_GREEN, category_d2d_color};

use super::palette::{Palette, ROW_PAD_POINTS, rect_f, with_alpha};
use super::{SettingsPaintState, SettingsRenderer};

fn settings_heat_fill(base: D2D1_COLOR_F, level: u8, palette: &Palette) -> D2D1_COLOR_F {
    match level {
        0 => palette.track,
        1 => with_alpha(base, 0.28),
        2 => with_alpha(base, 0.52),
        3 => with_alpha(base, 0.76),
        _ => base,
    }
}

impl SettingsRenderer {
    pub(super) fn draw_focus_heatmap_row(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        row: &SettingsRow,
        parts: &[Rect],
        rect: Rect,
        palette: &Palette,
    ) {
        let SettingsRowValue::FocusHeatmap(heatmap) = &row.value else {
            return;
        };
        let (section_index, row_index) = coords;
        let pad = self.px(ROW_PAD_POINTS);
        let base = heatmap.accent.map_or(SYSTEM_GREEN, category_d2d_color);

        let title_rect = Rect {
            x: rect.x + pad,
            y: rect.y + self.px(10),
            width: (rect.width - pad * 2 - self.px(96)).max(0),
            height: self.px(18),
        };
        self.text(&row.label, &self.formats.label, title_rect, palette.text);

        let sub_rect = Rect {
            x: rect.x + pad,
            y: title_rect.bottom() + self.px(2),
            width: (rect.width - pad * 2).max(0),
            height: self.px(16),
        };
        self.text(
            &row.subtitle,
            &self.formats.subtitle,
            sub_rect,
            palette.secondary,
        );

        let swatch = self.px(9).max(6);
        let gap = self.px(3).max(2);
        let legend_x = rect.right() - pad - (swatch * 5 + gap * 4);
        let legend_y = rect.y + self.px(14);
        for level in 0u8..5u8 {
            let box_rect = Rect {
                x: legend_x + i32::from(level) * (swatch + gap),
                y: legend_y,
                width: swatch,
                height: swatch,
            };
            self.fill_round(box_rect, 2.5, settings_heat_fill(base, level, palette));
        }

        if let Some(first) = parts.first() {
            let step = parts
                .get(1)
                .map_or_else(|| first.height + self.px(3), |second| second.y - first.y);
            for (row_idx, label) in [(1i32, "Mon"), (3, "Wed"), (5, "Fri")] {
                let lbl_rect = Rect {
                    x: rect.x + pad,
                    y: first.y + row_idx * step - self.px(2),
                    width: self.px(24),
                    height: self.px(14),
                };
                self.text(
                    label,
                    &self.formats.caption_leading,
                    lbl_rect,
                    palette.secondary,
                );
            }
        }

        for (index, (cell, cell_rect)) in heatmap.cells.iter().zip(parts).enumerate() {
            let radius = (cell_rect.width as f32 * 0.24).max(2.0);
            self.fill_round(
                *cell_rect,
                radius,
                settings_heat_fill(base, cell.level, palette),
            );
            let is_selected = heatmap.selected_cell == Some(index);
            let is_hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: index,
                });
            if is_selected || is_hovered || cell.is_today {
                let stroke_color = if is_selected || is_hovered {
                    palette.text
                } else {
                    palette.secondary
                };
                self.stroke_round_f(
                    rect_f(
                        cell_rect.x as f32 + 0.5,
                        cell_rect.y as f32 + 0.5,
                        (cell_rect.width as f32 - 1.0).max(1.0),
                        (cell_rect.height as f32 - 1.0).max(1.0),
                    ),
                    radius,
                    1.0,
                    stroke_color,
                );
            }
        }

        self.draw_heatmap_share_bar(heatmap, rect, pad, palette);
    }

    fn draw_heatmap_share_bar(
        &self,
        heatmap: &FocusHeatmapView,
        rect: Rect,
        pad: i32,
        palette: &Palette,
    ) {
        let bar_y = rect.bottom() - self.px(16);
        let bar_h = self.px(6).max(4);
        let bar_w = (rect.width - pad * 2).max(0);
        let bar_rect = Rect {
            x: rect.x + pad,
            y: bar_y,
            width: bar_w,
            height: bar_h,
        };
        let bar_radius = bar_h as f32 * 0.5;
        self.fill_round(bar_rect, bar_radius, palette.track);
        let mut cursor_x = bar_rect.x;
        let total_ms: u64 = heatmap.shares.iter().map(|s| s.focus_ms).sum();
        for share in &heatmap.shares {
            if share.focus_ms == 0 {
                continue;
            }
            let Some(scaled) = (bar_w as u64 * share.focus_ms).checked_div(total_ms) else {
                break;
            };
            let seg_w = (scaled as i32).max(bar_h).min(bar_rect.right() - cursor_x);
            if seg_w <= 0 {
                break;
            }
            self.fill_round(
                Rect {
                    x: cursor_x,
                    y: bar_y,
                    width: seg_w,
                    height: bar_h,
                },
                bar_radius,
                category_d2d_color(share.color),
            );
            cursor_x += seg_w + self.px(2);
        }
    }
}
