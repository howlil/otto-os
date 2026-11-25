use otto_core::{FocusStatsView, HitTarget, Layout, Rect, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use crate::appearance::tokens::{SYSTEM_GREEN, category_d2d_color};
use crate::{Renderer, rect_to_d2d, translate_rect};

fn heatmap_level_color(
    base: D2D1_COLOR_F,
    ink: D2D1_COLOR_F,
    level: u8,
    alpha: f32,
) -> (D2D1_COLOR_F, f32) {
    match level {
        0 => (ink, 0.08 * alpha),
        1 => (base, 0.28 * alpha),
        2 => (base, 0.52 * alpha),
        3 => (base, 0.76 * alpha),
        _ => (base, alpha),
    }
}

impl Renderer {
    pub(crate) fn draw_focus_stats_heatmap(
        &self,
        layout: &Layout,
        offset_x: i32,
        body: Rect,
        alpha: f32,
        hovered: Option<HitTarget>,
        stats: &FocusStatsView,
    ) {
        let ink = self.notch_ink();
        let accent = stats
            .heatmap
            .accent
            .map_or(SYSTEM_GREEN, category_d2d_color);

        for (index, (cat_id, label, color)) in stats.heatmap_filters.iter().enumerate() {
            let Some(&raw_rect) = layout.stats.heatmap_filters.get(index) else {
                break;
            };
            let rect = translate_rect(raw_rect, -offset_x);
            if rect.is_empty() {
                continue;
            }
            let active = match (&stats.heatmap.filter, cat_id) {
                (otto_core::HeatmapFilter::All, None) => true,
                (otto_core::HeatmapFilter::Category(cur), Some(id)) => cur == id,
                _ => false,
            };
            let is_hovered = hovered == Some(HitTarget::FocusHeatmapFilter(index as u8));
            let tint = category_d2d_color(*color);
            let fill = if active {
                self.brushes.alpha_brush(tint, 0.22 * alpha)
            } else if is_hovered {
                self.brushes.alpha_brush(ink, 0.12 * alpha)
            } else {
                self.brushes.alpha_brush(ink, 0.06 * alpha)
            };
            let radius = rect.height as f32 * 0.5;
            self.draw_rounded_box(rect, radius, &fill);
            if active {
                let border = self.brushes.alpha_brush(tint, 0.45 * alpha);
                self.draw_rounded_box_stroke(rect, radius, &border, 1.0);
            }
            let text_brush = if active {
                self.brushes.alpha_brush(tint, 0.98 * alpha)
            } else {
                self.brushes.alpha_brush(ink, 0.68 * alpha)
            };
            self.draw_text(label, &self.text_tab, rect_to_d2d(rect), &text_brush);
        }

        for (index, cell) in stats.heatmap.cells.iter().enumerate() {
            let Some(&raw_rect) = layout.stats.heatmap_cells.get(index) else {
                break;
            };
            let rect = translate_rect(raw_rect, -offset_x);
            if rect.is_empty() {
                continue;
            }
            let (col, col_alpha) = heatmap_level_color(accent, ink, cell.level, alpha);
            let brush = self.brushes.alpha_brush(col, col_alpha);
            let radius = (rect.width as f32 * 0.26).max(2.0);
            self.draw_rounded_box(rect, radius, &brush);
            let is_selected = stats.heatmap.selected_cell == Some(index);
            let is_hovered = hovered == Some(HitTarget::FocusHeatmapCell(index as u8));
            if is_selected || is_hovered || cell.is_today {
                let ring_alpha = if is_selected || is_hovered { 0.9 } else { 0.42 };
                let ring = self.brushes.alpha_brush(ink, ring_alpha * alpha);
                self.draw_rounded_box_stroke(rect, radius, &ring, 1.0);
            }
        }

        let pad_x = scale(12, self.dpi);
        let footer_h = scale(16, self.dpi);
        let footer_y = body.bottom() - footer_h - scale(4, self.dpi);
        let legend_w = scale(56, self.dpi);
        let readout_rect = Rect {
            x: body.x + pad_x,
            y: footer_y,
            width: (body.width - pad_x * 2 - legend_w - scale(6, self.dpi)).max(0),
            height: footer_h,
        };
        let readout_brush = self.brushes.alpha_brush(ink, 0.84 * alpha);
        self.draw_text(
            &stats.heatmap.readout,
            &self.text_pomodoro_sub,
            rect_to_d2d(readout_rect),
            &readout_brush,
        );

        let swatch = scale(7, self.dpi).max(5);
        let gap = scale(3, self.dpi).max(2);
        let legend_x = body.right() - pad_x - (swatch * 5 + gap * 4);
        let legend_y = footer_y + (footer_h - swatch) / 2;
        for level in 0u8..5u8 {
            let (col, col_alpha) = heatmap_level_color(accent, ink, level, alpha);
            let brush = self.brushes.alpha_brush(col, col_alpha);
            let box_rect = Rect {
                x: legend_x + i32::from(level) * (swatch + gap),
                y: legend_y,
                width: swatch,
                height: swatch,
            };
            self.draw_rounded_box(box_rect, 2.0, &brush);
        }
    }
}
