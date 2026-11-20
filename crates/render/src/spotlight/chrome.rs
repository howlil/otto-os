use otto_core::{
    Appearance, GlassShape, Rect, SpotlightLayout, SpotlightMode, SpotlightView, scale,
};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1Brush, ID2D1PathGeometry,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::theme::LiquidGlassPalette;

use super::{CLEAR, RectExt, SpotlightPaintState, SpotlightRenderer, with_opacity};

impl SpotlightRenderer {
    fn fill_shape(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        geom: Option<&ID2D1PathGeometry>,
        brush: &ID2D1Brush,
    ) {
        if let Some(geom) = geom {
            unsafe {
                self.target.FillGeometry(geom, brush, None);
            }
            return;
        }
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&rounded, brush);
        }
    }

    fn draw_shape(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        geom: Option<&ID2D1PathGeometry>,
        brush: &ID2D1Brush,
        stroke_width: f32,
    ) {
        if let Some(geom) = geom {
            unsafe {
                self.target.DrawGeometry(geom, brush, stroke_width, None);
            }
            return;
        }
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&rounded, brush, stroke_width, None);
        }
    }

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
        let shape = GlassShape::from_rect(morph_rect, morph_radius);
        if self
            .glass
            .draw(&self.target, appearance, self.dpi, shape, true)?
        {
            return Ok(());
        }
        let outer_rect = D2D_RECT_F {
            left: morph_rect.x as f32 + 0.5,
            top: morph_rect.y as f32 + 0.5,
            right: morph_rect.right() as f32 - 0.5,
            bottom: morph_rect.bottom() as f32 - 0.5,
        };
        let inner_rect = D2D_RECT_F {
            left: morph_rect.x as f32 + 1.5,
            top: morph_rect.y as f32 + 1.5,
            right: morph_rect.right() as f32 - 1.5,
            bottom: morph_rect.bottom() as f32 - 1.5,
        };
        let inner_radius = (morph_radius - 1.0).max(1.0);
        let outer_geom = crate::build_squircle_path(&self.factory, outer_rect, morph_radius).ok();
        let inner_geom = crate::build_squircle_path(&self.factory, inner_rect, inner_radius).ok();
        let fill = self.brushes.color_brush(palette.menu_fill);
        let outer_border = self.brushes.color_brush(palette.menu_outer_border);
        self.fill_shape(outer_rect, morph_radius, outer_geom.as_ref(), &fill);
        self.draw_shape(
            outer_rect,
            morph_radius,
            outer_geom.as_ref(),
            &outer_border,
            1.0,
        );
        self.draw_shape(
            inner_rect,
            inner_radius,
            inner_geom.as_ref(),
            &self.brushes.specular_rim,
            1.0,
        );
        Ok(())
    }

    pub(super) fn draw_search(
        &self,
        state: &SpotlightPaintState<'_, '_>,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let layout = state.layout;
        let query = state.query;
        let rect = layout.search_field.to_d2d();
        let radius =
            (layout.corner_radius - layout.search_field.x as f32).max(scale(10, self.dpi) as f32);
        let geom = crate::build_squircle_path(&self.factory, rect, radius).ok();
        let fill = self
            .brushes
            .color_brush(with_opacity(palette.card_fill, opacity));
        self.fill_shape(rect, radius, geom.as_ref(), &fill);
        self.draw_search_magnifier(layout, opacity, palette);
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
        if state.selected_all && !query.is_empty() {
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
        } else {
            palette.text_primary
        };
        let text_brush = self.brushes.color_brush(with_opacity(text_color, opacity));
        self.draw_text(&self.search_format, text, &text_rect, &text_brush);
        let caret_width = scale(1, self.dpi).max(1) as f32;
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
            .color_brush(with_opacity(palette.text_primary, opacity));
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
        let icon_center = (
            layout.search_field.x + scale(20, self.dpi),
            layout.search_field.y + layout.search_field.height / 2,
        );
        let magnifier = D2D1_ELLIPSE {
            point: Vector2::new(icon_center.0 as f32, icon_center.1 as f32),
            radiusX: scale(6, self.dpi) as f32,
            radiusY: scale(6, self.dpi) as f32,
        };
        let icon_brush = self
            .brushes
            .color_brush(with_opacity(palette.text_muted, opacity));
        unsafe {
            self.target.DrawEllipse(
                &magnifier,
                &icon_brush,
                scale(2, self.dpi) as f32 * 0.9,
                None,
            );
            self.target.DrawLine(
                Vector2::new(
                    (icon_center.0 + scale(4, self.dpi)) as f32,
                    (icon_center.1 + scale(4, self.dpi)) as f32,
                ),
                Vector2::new(
                    (icon_center.0 + scale(9, self.dpi)) as f32,
                    (icon_center.1 + scale(9, self.dpi)) as f32,
                ),
                &icon_brush,
                scale(2, self.dpi) as f32 * 0.9,
                None,
            );
        }
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
            .color_brush(with_opacity(palette.text_primary, opacity));
        self.draw_text(&self.label_format, &pill_text, &pill_rect, &pill_brush);
        (pill_left - scale(10, self.dpi) as f32).max(text_rect.left)
    }

    pub(super) fn draw_tabs(
        &self,
        layout: &SpotlightLayout,
        mode: SpotlightMode,
        view: SpotlightView,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
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
        let shape = D2D1_ROUNDED_RECT {
            rect: bounds.to_d2d(),
            radiusX: scale(10, self.dpi) as f32,
            radiusY: scale(10, self.dpi) as f32,
        };
        let fill = if active {
            with_opacity(palette.menu_hover_fill, opacity)
        } else {
            with_opacity(CLEAR, opacity)
        };
        unsafe {
            self.target
                .FillRoundedRectangle(&shape, &self.brushes.color_brush(fill));
        }
        let text_color = if active {
            palette.text_primary
        } else {
            palette.text_muted
        };
        let text_brush = self.brushes.color_brush(with_opacity(text_color, opacity));
        self.draw_text(&self.label_format, label, &bounds.to_d2d(), &text_brush);
    }

    fn draw_view_button(
        &self,
        bounds: Rect,
        active: bool,
        grid: bool,
        opacity: f32,
        palette: &LiquidGlassPalette,
    ) {
        let shape = D2D1_ROUNDED_RECT {
            rect: bounds.to_d2d(),
            radiusX: scale(8, self.dpi) as f32,
            radiusY: scale(8, self.dpi) as f32,
        };
        let fill = if active {
            with_opacity(palette.menu_hover_fill, opacity)
        } else {
            with_opacity(CLEAR, opacity)
        };
        unsafe {
            self.target
                .FillRoundedRectangle(&shape, &self.brushes.color_brush(fill));
        }
        let text_color = if active {
            palette.text_primary
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
