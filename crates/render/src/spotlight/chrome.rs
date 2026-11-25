use otto_core::{Appearance, Rect, SpotlightLayout, SpotlightMode, SpotlightView, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::theme::LiquidGlassPalette;
use crate::kit::draw::{fill_squircle_or_rounded, stroke_squircle_or_rounded};
use crate::kit::icons::draw_search_magnifier_glyph;
use crate::{draw_dwrite_tabular_text, rect_to_d2d};

use super::{SpotlightPaintState, SpotlightRenderer, with_opacity};

impl SpotlightRenderer {
    pub(super) fn draw_shell(
        &self,
        width: i32,
        height: i32,
        radius: f32,
        morph_progress: f32,
        appearance: &Appearance,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let morph_progress = morph_progress.clamp(0.0, 1.0);
        let start_width = scale(48, self.dpi).min(width);
        let start_height = scale(28, self.dpi).min(height);
        let start_x = (width - start_width) / 2;
        let start_y = (height - start_height) / 2;
        let morph_rect = Rect {
            x: (start_x as f32 * (1.0 - morph_progress)).round() as i32,
            y: (start_y as f32 * (1.0 - morph_progress)).round() as i32,
            width: start_width + ((width - start_width) as f32 * morph_progress).round() as i32,
            height: start_height + ((height - start_height) as f32 * morph_progress).round() as i32,
        };
        let morph_radius = (start_height as f32 * 0.5
            + (radius - start_height as f32 * 0.5) * morph_progress)
            .min(morph_rect.width as f32 * 0.5)
            .min(morph_rect.height as f32 * 0.5);
        crate::kit::draw::draw_glass_or_fallback_shell(
            &self.factory,
            &self.target,
            &self.glass,
            &self.brushes,
            morph_rect,
            morph_radius,
            self.dpi,
            appearance,
            palette,
        )
    }

    fn draw_search_field_chrome(
        &self,
        layout: &SpotlightLayout,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let radius =
            (layout.corner_radius - layout.search_field.x as f32).max(scale(10, self.dpi) as f32);
        let fill = self
            .brushes
            .color_brush(with_opacity(palette.card_fill, opacity));
        fill_squircle_or_rounded(
            &self.factory,
            &self.target,
            layout.search_field,
            radius,
            self.dpi,
            &fill,
        );
        let border_color = if palette.is_high_contrast {
            palette.specular_rim
        } else {
            palette.menu_outer_border
        };
        let border = self
            .brushes
            .color_brush(with_opacity(border_color, opacity));
        stroke_squircle_or_rounded(
            &self.factory,
            &self.target,
            layout.search_field,
            radius,
            self.dpi,
            &border,
            1.0,
        );
        self.draw_search_magnifier(layout, opacity, palette);
    }

    pub(super) fn draw_search(
        &self,
        state: &SpotlightPaintState<'_, '_>,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let layout = state.layout;
        let query = state.query;
        self.draw_search_field_chrome(layout, opacity, palette);
        let mut text_rect = D2D_RECT_F {
            left: (layout.search_field.x + scale(42, self.dpi)) as f32,
            top: layout.search_field.y as f32,
            right: (layout.search_field.right() - scale(12, self.dpi)) as f32,
            bottom: layout.search_field.bottom() as f32,
        };
        if let Some(result) = state.math_result {
            text_rect.right = self.draw_math_pill(layout, &text_rect, result, opacity, palette);
        }
        let text = if query.is_empty() {
            "Search local apps"
        } else {
            query
        };
        let mut cursor_byte = state.cursor_byte.min(query.len());
        while cursor_byte > 0 && !query.is_char_boundary(cursor_byte) {
            cursor_byte -= 1;
        }
        let caret_x = if query.is_empty() {
            let caret_x = text_rect.left;
            text_rect.left += scale(4, self.dpi) as f32;
            caret_x
        } else {
            text_rect.left
                + Self::measure_text_width(&query[..cursor_byte], &self.search_format)
                    .unwrap_or(0.0)
        };
        let is_selected_all = state.selected_all && !query.is_empty();
        if is_selected_all {
            let total_w = Self::measure_text_width(query, &self.search_format).unwrap_or(0.0);
            let sel_right = (text_rect.left + total_w).min(text_rect.right);
            let sel_shape = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: text_rect.left,
                    top: text_rect.top + scale(8, self.dpi) as f32,
                    right: sel_right.max(text_rect.left),
                    bottom: text_rect.bottom - scale(8, self.dpi) as f32,
                },
                radiusX: scale(4, self.dpi) as f32,
                radiusY: scale(4, self.dpi) as f32,
            };
            unsafe {
                self.target.FillRoundedRectangle(
                    &sel_shape,
                    &self
                        .brushes
                        .color_brush(with_opacity(palette.menu_hover_fill, opacity)),
                );
            }
        }
        let text_color = if query.is_empty() {
            palette.text_muted
        } else if is_selected_all {
            palette.on_accent
        } else {
            palette.text_primary
        };
        let text_brush = self.brushes.color_brush(with_opacity(text_color, opacity));
        self.draw_text(&self.search_format, text, &text_rect, &text_brush);
        let caret_width = (scale(2, self.dpi) as f32 * 0.75).max(1.5);
        let max_caret_x = (text_rect.right - caret_width).max(text_rect.left);
        let caret_x = caret_x.clamp(text_rect.left, max_caret_x);
        let caret = D2D_RECT_F {
            left: caret_x,
            top: text_rect.top + scale(10, self.dpi) as f32,
            right: caret_x + caret_width,
            bottom: text_rect.bottom - scale(10, self.dpi) as f32,
        };
        let caret_brush = self
            .brushes
            .color_brush(with_opacity(palette.toggle_active, opacity));
        unsafe {
            self.target.FillRectangle(&caret, &caret_brush);
        }
    }

    fn draw_search_magnifier(
        &self,
        layout: &SpotlightLayout,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let icon_center = Vector2::new(
            (layout.search_field.x + scale(20, self.dpi)) as f32,
            (layout.search_field.y + layout.search_field.height / 2) as f32,
        );
        let icon_brush = self
            .brushes
            .color_brush(with_opacity(palette.text_muted, opacity));
        draw_search_magnifier_glyph(
            &self.target,
            &self.stroke_style,
            icon_center,
            scale(6, self.dpi) as f32,
            (scale(2, self.dpi) as f32 * 0.9).max(1.5),
            &icon_brush,
        );
    }

    fn draw_math_pill(
        &self,
        layout: &SpotlightLayout,
        text_rect: &D2D_RECT_F,
        result: &str,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) -> f32 {
        let pill_text = format!("= {result}  (↵ Copy)");
        let measured = Self::measure_text_width(&pill_text, &self.label_format).unwrap_or(0.0);
        let pad_x = scale(12, self.dpi) as f32;
        let max_pill_w = ((text_rect.right - text_rect.left) * 0.58).max(0.0);
        let pill_w = (measured + pad_x * 2.0).min(max_pill_w);
        if pill_w <= pad_x {
            return text_rect.right;
        }
        let pill_margin_y = scale(7, self.dpi) as f32;
        let pill_right = (layout.search_field.right() - scale(8, self.dpi)) as f32;
        let pill_left = (pill_right - pill_w).max(text_rect.left + scale(24, self.dpi) as f32);
        let pill_rect = D2D_RECT_F {
            left: pill_left,
            top: text_rect.top + pill_margin_y,
            right: pill_right,
            bottom: text_rect.bottom - pill_margin_y,
        };
        let pill_radius = ((pill_rect.bottom - pill_rect.top) * 0.5).max(1.0);
        let pill_shape = D2D1_ROUNDED_RECT {
            rect: pill_rect,
            radiusX: pill_radius,
            radiusY: pill_radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(
                &pill_shape,
                &self
                    .brushes
                    .color_brush(with_opacity(palette.menu_hover_fill, opacity)),
            );
        }
        let pill_brush = self
            .brushes
            .color_brush(with_opacity(palette.on_accent, opacity));
        draw_dwrite_tabular_text(
            &self.target,
            &pill_text,
            &self.label_format,
            pill_rect,
            &pill_brush,
        );
        (pill_left - scale(10, self.dpi) as f32).max(text_rect.left)
    }

    fn draw_segmented_track(
        &self,
        bounds: Rect,
        radius: f32,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let track_fill = self
            .brushes
            .color_brush(with_opacity(palette.card_fill, opacity));
        fill_squircle_or_rounded(
            &self.factory,
            &self.target,
            bounds,
            radius,
            self.dpi,
            &track_fill,
        );
        let border_color = if palette.is_high_contrast {
            palette.specular_rim
        } else {
            palette.menu_outer_border
        };
        let track_border = self
            .brushes
            .color_brush(with_opacity(border_color, opacity));
        stroke_squircle_or_rounded(
            &self.factory,
            &self.target,
            bounds,
            radius,
            self.dpi,
            &track_border,
            1.0,
        );
    }

    pub(super) fn draw_tabs(
        &self,
        layout: &SpotlightLayout,
        mode: SpotlightMode,
        view: SpotlightView,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        self.draw_segmented_track(
            layout.search_tab.union(&layout.apps_tab),
            scale(10, self.dpi) as f32,
            opacity,
            palette,
        );
        self.draw_segmented_track(
            layout.grid_view.union(&layout.list_view),
            scale(8, self.dpi) as f32,
            opacity,
            palette,
        );
        self.draw_tab(
            layout.search_tab,
            "Search",
            mode == SpotlightMode::Search,
            opacity,
            palette,
        );
        self.draw_tab(
            layout.apps_tab,
            "Apps",
            mode == SpotlightMode::Apps,
            opacity,
            palette,
        );
        self.draw_view_button(
            layout.grid_view,
            view == SpotlightView::Grid,
            true,
            opacity,
            palette,
        );
        self.draw_view_button(
            layout.list_view,
            view == SpotlightView::List,
            false,
            opacity,
            palette,
        );
    }

    fn draw_tab(
        &self,
        bounds: Rect,
        label: &str,
        active: bool,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        if active {
            let inset = scale(2, self.dpi);
            let pill = Rect {
                x: bounds.x + inset,
                y: bounds.y + inset,
                width: (bounds.width - inset * 2).max(1),
                height: (bounds.height - inset * 2).max(1),
            };
            let fill = self
                .brushes
                .color_brush(with_opacity(palette.menu_hover_fill, opacity));
            fill_squircle_or_rounded(
                &self.factory,
                &self.target,
                pill,
                scale(8, self.dpi) as f32,
                self.dpi,
                &fill,
            );
        }
        let text_color = if active {
            palette.on_accent
        } else {
            palette.text_muted
        };
        let text_brush = self.brushes.color_brush(with_opacity(text_color, opacity));
        self.draw_text(&self.label_format, label, &rect_to_d2d(bounds), &text_brush);
    }

    fn draw_view_button(
        &self,
        bounds: Rect,
        active: bool,
        grid: bool,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        if active {
            let inset = scale(2, self.dpi);
            let pill = Rect {
                x: bounds.x + inset,
                y: bounds.y + inset,
                width: (bounds.width - inset * 2).max(1),
                height: (bounds.height - inset * 2).max(1),
            };
            let fill = self
                .brushes
                .color_brush(with_opacity(palette.menu_hover_fill, opacity));
            fill_squircle_or_rounded(
                &self.factory,
                &self.target,
                pill,
                scale(6, self.dpi) as f32,
                self.dpi,
                &fill,
            );
        }
        let text_color = if active {
            palette.on_accent
        } else {
            palette.text_muted
        };
        let icon_brush = self.brushes.color_brush(with_opacity(text_color, opacity));
        let center_x = bounds.x + bounds.width / 2;
        let center_y = bounds.y + bounds.height / 2;
        if grid {
            let size = scale(5, self.dpi) as f32;
            let gap = scale(2, self.dpi) as f32;
            for row in 0..2 {
                for column in 0..2 {
                    let x = center_x as f32 - size - gap / 2.0 + column as f32 * (size + gap);
                    let y = center_y as f32 - size - gap / 2.0 + row as f32 * (size + gap);
                    unsafe {
                        self.target.FillRoundedRectangle(
                            &D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: x,
                                    top: y,
                                    right: x + size,
                                    bottom: y + size,
                                },
                                radiusX: scale(1, self.dpi) as f32,
                                radiusY: scale(1, self.dpi) as f32,
                            },
                            &icon_brush,
                        );
                    }
                }
            }
        } else {
            let line_width = scale(15, self.dpi) as f32;
            let line_height = scale(2, self.dpi) as f32;
            for row in 0..3 {
                let y = center_y as f32 - scale(5, self.dpi) as f32
                    + row as f32 * scale(5, self.dpi) as f32;
                unsafe {
                    self.target.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: center_x as f32 - line_width / 2.0,
                                top: y,
                                right: center_x as f32 + line_width / 2.0,
                                bottom: y + line_height,
                            },
                            radiusX: line_height,
                            radiusY: line_height,
                        },
                        &icon_brush,
                    );
                }
            }
        }
    }
}
