use otto_core::{SpotlightLayout, SpotlightMode, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS, IDWriteTextFormat,
};
use windows_numerics::Vector2;

use crate::appearance::theme::LiquidGlassPalette;
use crate::shared_dwrite_factory;

use super::{RectExt, SpotlightPaintEntry, SpotlightRenderer, with_opacity};

impl SpotlightRenderer {
    pub(super) fn draw_result(
        &self,
        layout: &otto_core::SpotlightResultLayout,
        entry: &SpotlightPaintEntry<'_>,
        selected: bool,
        hovered: bool,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let tile = D2D1_ROUNDED_RECT {
            rect: layout.tile.to_d2d(),
            radiusX: scale(12, self.dpi) as f32,
            radiusY: scale(12, self.dpi) as f32,
        };
        if selected || hovered {
            let fill = if selected {
                palette.menu_hover_fill
            } else {
                palette.card_fill
            };
            unsafe {
                self.target.FillRoundedRectangle(
                    &tile,
                    &self.brushes.color_brush(with_opacity(fill, opacity)),
                );
            }
        }
        let icon_bounds = layout.icon.to_d2d();
        if let Some(icon) = self.icons.get(entry.key) {
            unsafe {
                self.target.DrawBitmap(
                    &icon.bitmap,
                    Some(&raw const icon_bounds),
                    opacity,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        } else {
            let icon_tile = D2D1_ROUNDED_RECT {
                rect: icon_bounds,
                radiusX: scale(13, self.dpi) as f32,
                radiusY: scale(13, self.dpi) as f32,
            };
            unsafe {
                self.target.FillRoundedRectangle(
                    &icon_tile,
                    &self
                        .brushes
                        .color_brush(with_opacity(palette.card_fill, opacity)),
                );
            }
            let label = entry.title.chars().next().map_or_else(
                || String::from("•"),
                |character| character.to_uppercase().to_string(),
            );
            let icon_brush = self
                .brushes
                .color_brush(with_opacity(palette.text_primary, opacity));
            self.draw_text(
                &self.icon_fallback_format,
                &label,
                &icon_bounds,
                &icon_brush,
            );
        }
        let label_color = if selected {
            palette.text_primary
        } else {
            palette.text_muted
        };
        let label_brush = self.brushes.color_brush(with_opacity(label_color, opacity));
        self.draw_text(
            &self.label_format,
            entry.title,
            &layout.label.to_d2d(),
            &label_brush,
        );
    }

    pub(super) fn draw_empty_state(
        &self,
        layout: &SpotlightLayout,
        mode: SpotlightMode,
        query: &str,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let message = match (mode, query.is_empty()) {
            (SpotlightMode::Search, true) => "Type to find a local app",
            _ => "No apps found",
        };
        let brush = self
            .brushes
            .color_brush(with_opacity(palette.text_muted, opacity));
        self.draw_text(
            &self.search_format,
            message,
            &layout.empty_state.to_d2d(),
            &brush,
        );
    }

    pub(super) fn draw_footer(
        &self,
        width: i32,
        height: i32,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let bounds = D2D_RECT_F {
            left: scale(18, self.dpi) as f32,
            top: (height - scale(27, self.dpi)) as f32,
            right: (width - scale(18, self.dpi)) as f32,
            bottom: (height - scale(8, self.dpi)) as f32,
        };
        let brush = self
            .brushes
            .color_brush(with_opacity(palette.text_muted, opacity));
        self.draw_text(
            &self.footer_format,
            "Enter to open  ·  Tab: grid/list  ·  Esc to close  ·  Ctrl+Alt+Space",
            &bounds,
            &brush,
        );
    }

    pub(super) fn draw_resize_grip(
        &self,
        layout: &SpotlightLayout,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let color = self
            .brushes
            .color_brush(with_opacity(palette.text_muted, opacity));
        let right = layout.resize_handle.right() - scale(5, self.dpi);
        let bottom = layout.resize_handle.bottom() - scale(5, self.dpi);
        for offset in 0..3 {
            let inset = scale(4 + offset * 4, self.dpi);
            unsafe {
                self.target.DrawLine(
                    Vector2::new((right - inset) as f32, bottom as f32),
                    Vector2::new(right as f32, (bottom - inset) as f32),
                    &color,
                    scale(1, self.dpi) as f32,
                    None,
                );
            }
        }
    }

    pub(super) fn draw_text(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        bounds: &D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        if text.is_empty() {
            return;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                bounds,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    pub(super) fn measure_text_width(text: &str, format: &IDWriteTextFormat) -> Option<f32> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let dwrite = shared_dwrite_factory().ok()?;
        unsafe {
            let layout = dwrite.CreateTextLayout(&wide, format, 4096.0, 64.0).ok()?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            Some(metrics.widthIncludingTrailingWhitespace)
        }
    }
}
