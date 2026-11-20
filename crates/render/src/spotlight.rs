use std::collections::{HashMap, HashSet};

use otto_core::{
    Appearance, GlassShape, PixelLimits, Rect, RgbBuffer, SpotlightLayout, SpotlightMode,
    SpotlightView, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1Bitmap, ID2D1Brush,
    ID2D1DCRenderTarget, ID2D1Factory, ID2D1PathGeometry,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_METRICS, IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::glass::GlassPainter;
use crate::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery, upload_bgra_bitmap,
};
use crate::theme::{BrushPalette, LiquidGlassPalette};
use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

#[derive(Clone, Copy)]
pub struct SpotlightPaintEntry<'a> {
    pub key: &'a str,
    pub title: &'a str,
}

pub struct SpotlightPaintState<'paint, 'text> {
    pub layout: &'paint SpotlightLayout,
    pub entries: &'paint [SpotlightPaintEntry<'text>],
    pub mode: SpotlightMode,
    pub view: SpotlightView,
    pub query: &'paint str,
    pub cursor_byte: usize,
    pub selected_all: bool,
    pub math_result: Option<&'paint str>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub content_opacity: f32,
    pub appearance: Appearance,
}

struct CachedSpotlightIcon {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    bitmap: ID2D1Bitmap,
}

pub struct SpotlightRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    brushes: BrushPalette,
    search_format: IDWriteTextFormat,
    label_format: IDWriteTextFormat,
    icon_fallback_format: IDWriteTextFormat,
    footer_format: IDWriteTextFormat,
    surface: DibSurface,
    dpi: u32,
    glass: GlassPainter,
    icons: HashMap<String, CachedSpotlightIcon>,
}

impl SpotlightRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = create_dc_render_target(&factory)?;
            let brushes = BrushPalette::new(&target)?;
            let dwrite = shared_dwrite_factory()?;
            let search_format = create_text_format(
                &dwrite,
                scale(17, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let label_format = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let icon_fallback_format = create_text_format(
                &dwrite,
                scale(25, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let footer_format = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            Ok(Self {
                factory,
                target,
                brushes,
                search_format,
                label_format,
                icon_fallback_format,
                footer_format,
                surface: DibSurface::new(scale(1_200, dpi), scale(900, dpi))?,
                dpi,
                glass: GlassPainter::default(),
                icons: HashMap::new(),
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn clear_backdrop(&self) {
        self.glass.clear_backdrop();
    }

    #[must_use]
    pub fn has_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    #[must_use]
    pub fn has_icon(&self, key: &str) -> bool {
        self.icons.contains_key(key)
    }

    pub fn retain_visible_icons(&mut self, visible_keys: &HashSet<&str>) {
        self.icons
            .retain(|key, _| visible_keys.contains(key.as_str()));
    }

    pub fn set_icon(&mut self, key: String, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
        self.surface.bind(&self.target, self.surface.size())?;
        let bitmap = upload_bgra_bitmap(&self.target, width, height, pixels, PixelLimits::ICON)?;
        self.icons.insert(
            key,
            CachedSpotlightIcon {
                width,
                height,
                pixels: pixels.to_vec(),
                bitmap,
            },
        );
        Ok(())
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.glass.invalidate_gpu();
        self.upload_icons()
    }

    pub fn present(&mut self, hwnd: isize, state: &SpotlightPaintState<'_, '_>) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = state.layout.window.width.clamp(1, max_width);
        let height = state.layout.window.height.clamp(1, max_height);
        draw_with_recovery(
            self,
            |renderer| renderer.draw(width, height, state),
            Self::recreate_target,
        )?;
        self.surface.present_sized(
            hwnd,
            (state.layout.window.x, state.layout.window.y),
            (width, height),
        )
    }

    fn draw(&self, width: i32, height: i32, state: &SpotlightPaintState<'_, '_>) -> Result<()> {
        let opacity = state.content_opacity.clamp(0.0, 1.0);
        let appearance = self.glass.adaptive_appearance(&state.appearance);
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        let clear = CLEAR;
        draw_on(&self.target, &self.surface, (width, height), || unsafe {
            self.target.Clear(Some(&raw const clear));
            self.brushes.update_theme(&palette);
            self.draw_shell(
                width,
                height,
                state.layout.corner_radius,
                opacity,
                &appearance,
                &palette,
            )?;
            self.draw_search(state, opacity, &palette);
            self.draw_tabs(state.layout, state.mode, state.view, opacity, &palette);
            if opacity > 0.04 {
                if state.entries.is_empty() {
                    self.draw_empty_state(state.layout, state.mode, state.query, opacity, &palette);
                } else {
                    for item in &state.layout.results {
                        let Some(entry) = state.entries.get(item.index) else {
                            continue;
                        };
                        self.draw_result(
                            item,
                            entry,
                            state.selected == Some(item.index),
                            state.hovered == Some(item.index),
                            opacity,
                            &palette,
                        );
                    }
                }
                self.draw_footer(width, height, opacity, &palette);
            }
            self.draw_resize_grip(state.layout, opacity, &palette);
            Ok(())
        })
    }

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

    fn draw_shell(
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

    fn draw_search(
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

    fn draw_tabs(
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

    fn draw_result(
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

    fn draw_empty_state(
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

    fn draw_footer(&self, width: i32, height: i32, opacity: f32, palette: &LiquidGlassPalette) {
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

    fn draw_resize_grip(
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

    fn draw_text(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        bounds: &D2D_RECT_F,
        brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
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

    fn measure_text_width(text: &str, format: &IDWriteTextFormat) -> Option<f32> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let dwrite = shared_dwrite_factory().ok()?;
        unsafe {
            let layout = dwrite.CreateTextLayout(&wide, format, 4096.0, 64.0).ok()?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            Some(metrics.widthIncludingTrailingWhitespace)
        }
    }

    fn upload_icons(&mut self) -> Result<()> {
        self.surface.bind(&self.target, self.surface.size())?;
        for icon in self.icons.values_mut() {
            icon.bitmap = upload_bgra_bitmap(
                &self.target,
                icon.width,
                icon.height,
                &icon.pixels,
                PixelLimits::ICON,
            )?;
        }
        Ok(())
    }
}

fn with_opacity(mut color: D2D1_COLOR_F, opacity: f32) -> D2D1_COLOR_F {
    color.a *= opacity.clamp(0.0, 1.0);
    color
}

trait RectExt {
    fn to_d2d(self) -> D2D_RECT_F;
}

impl RectExt for Rect {
    fn to_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x as f32,
            top: self.y as f32,
            right: self.right() as f32,
            bottom: self.bottom() as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::PoisonError;

    use super::*;
    use crate::release_shared_factories;
    use crate::resource_tests::SERIAL;
    use otto_core::ResolvedTheme;

    #[test]
    fn spotlight_concentric_search_radius_and_squircle_shell_render_cleanly() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let layout = otto_core::compute_spotlight_layout(
            (100, 100),
            640,
            440,
            96,
            SpotlightView::Grid,
            2,
            0,
        );
        let expected_search_radius =
            (layout.corner_radius - layout.search_field.x as f32).max(scale(10, 96) as f32);
        assert!((expected_search_radius - 12.0).abs() < f32::EPSILON);

        let renderer = SpotlightRenderer::new(96).expect("spotlight renderer");
        let entries = [
            SpotlightPaintEntry {
                key: "notes",
                title: "Notes",
            },
            SpotlightPaintEntry {
                key: "calc",
                title: "Calculator",
            },
        ];
        for opacity in [0.25, 1.0] {
            let state = SpotlightPaintState {
                layout: &layout,
                entries: &entries,
                mode: SpotlightMode::Search,
                view: SpotlightView::Grid,
                query: "2 + 2",
                cursor_byte: 5,
                selected_all: false,
                math_result: Some("4"),
                selected: Some(0),
                hovered: Some(1),
                content_opacity: opacity,
                appearance: Appearance::glass(ResolvedTheme::Dark),
            };
            renderer
                .draw(layout.window.width, layout.window.height, &state)
                .expect("spotlight draw");
        }
        assert!(renderer.surface.read_pixels().iter().any(|&byte| byte != 0));
        drop(renderer);
        release_shared_factories();
    }
}
