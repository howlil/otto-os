use otto_core::{AppleIconKind, SpotlightLayout, SpotlightMode, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::IDWriteTextFormat;
use windows_numerics::Vector2;

use crate::appearance::theme::LiquidGlassPalette;
use crate::kit::draw::{fill_squircle_or_rounded, measure_text_width};
use crate::kit::icons::{GlyphCtx, draw_finder_tile};
use crate::rect_to_d2d;

use super::{SpotlightPaintEntry, SpotlightRenderer, with_opacity};

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
        if selected || hovered {
            let fill = if selected {
                palette.menu_hover_fill
            } else {
                palette.hover_pill
            };
            let brush = self.brushes.color_brush(with_opacity(fill, opacity));
            fill_squircle_or_rounded(
                &self.factory,
                &self.target,
                layout.tile,
                scale(12, self.dpi) as f32,
                self.dpi,
                &brush,
            );
        }
        let icon_bounds = rect_to_d2d(layout.icon);
        if entry.apple_icon == Some(AppleIconKind::Finder) {
            let radius = (icon_bounds.right - icon_bounds.left) * 0.225;
            let tile = D2D1_ROUNDED_RECT {
                rect: icon_bounds,
                radiusX: radius,
                radiusY: radius,
            };
            let _ = draw_finder_tile(
                GlyphCtx {
                    factory: &self.factory,
                    target: &self.target,
                    stroke_style: &self.stroke_style,
                },
                &self.brushes,
                &tile,
                opacity,
                true,
            );
        } else if let Some(icon) = self.icons.get(entry.key) {
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
            let fill = self
                .brushes
                .color_brush(with_opacity(palette.card_fill, opacity));
            fill_squircle_or_rounded(
                &self.factory,
                &self.target,
                layout.icon,
                scale(13, self.dpi) as f32,
                self.dpi,
                &fill,
            );
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
            palette.on_accent
        } else {
            palette.text_primary
        };
        let label_brush = self.brushes.color_brush(with_opacity(label_color, opacity));
        self.draw_text(
            &self.label_format,
            entry.title,
            &rect_to_d2d(layout.label),
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
            &rect_to_d2d(layout.empty_state),
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
                    &self.stroke_style,
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
        crate::kit::draw::draw_text_on(
            &self.target,
            text,
            format,
            *bounds,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
        );
    }

    pub(super) fn measure_text_width(text: &str, format: &IDWriteTextFormat) -> Option<f32> {
        measure_text_width(text, format, 4096.0)
    }
}
