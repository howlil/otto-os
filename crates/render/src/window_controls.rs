use otto_core::{
    Appearance, CoverStyle, ResolvedTheme, Rgba, WindowControlAction, WindowControlsLayout,
    WindowControlsState, cover_row_bands,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_ROUND,
    D2D1_DASH_STYLE_SOLID, D2D1_ELLIPSE, D2D1_LINE_JOIN_ROUND, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::glow::linear_gradient;
use crate::shared_d2d_factory;
use crate::surface::{DibSurface, create_dc_render_target, draw_on, draw_with_recovery};

const BASE_BUTTON_RADIUS: f32 = 6.0;
const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
const RED_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.373,
    b: 0.337,
    a: 1.0,
};
const RED_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.878,
    g: 0.267,
    b: 0.243,
    a: 1.0,
};
const YELLOW_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.741,
    b: 0.180,
    a: 1.0,
};
const YELLOW_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.871,
    g: 0.631,
    b: 0.137,
    a: 1.0,
};
const GREEN_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.153,
    g: 0.788,
    b: 0.247,
    a: 1.0,
};
const GREEN_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.102,
    g: 0.671,
    b: 0.161,
    a: 1.0,
};
const INACTIVE_DARK_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.86,
    g: 0.86,
    b: 0.89,
    a: 0.22,
};
const INACTIVE_DARK_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.86,
    g: 0.86,
    b: 0.89,
    a: 0.32,
};
const INACTIVE_LIGHT_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.48,
    g: 0.48,
    b: 0.50,
    a: 0.28,
};
const INACTIVE_LIGHT_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.40,
    g: 0.40,
    b: 0.42,
    a: 0.38,
};
const GLYPH_DARK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.22,
    g: 0.08,
    b: 0.04,
    a: 0.90,
};
const fn pressed_color(color: D2D1_COLOR_F) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r * 0.76,
        g: color.g * 0.76,
        b: color.b * 0.76,
        a: color.a,
    }
}

const fn base_colors_for(action: WindowControlAction) -> (D2D1_COLOR_F, D2D1_COLOR_F) {
    match action {
        WindowControlAction::Close => (RED_FILL, RED_BORDER),
        WindowControlAction::Minimize => (YELLOW_FILL, YELLOW_BORDER),
        WindowControlAction::ToggleMaximize => (GREEN_FILL, GREEN_BORDER),
    }
}

const fn d2d(color: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

const fn rgb_to_d2d(rgb: [u8; 3]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: rgb[0] as f32 / 255.0,
        g: rgb[1] as f32 / 255.0,
        b: rgb[2] as f32 / 255.0,
        a: 1.0,
    }
}

struct ControlBrushes {
    red_fill: ID2D1SolidColorBrush,
    red_border: ID2D1SolidColorBrush,
    yellow_fill: ID2D1SolidColorBrush,
    yellow_border: ID2D1SolidColorBrush,
    green_fill: ID2D1SolidColorBrush,
    green_border: ID2D1SolidColorBrush,
    disabled_fill: ID2D1SolidColorBrush,
    disabled_border: ID2D1SolidColorBrush,
    pressed_fill: ID2D1SolidColorBrush,
    pressed_border: ID2D1SolidColorBrush,
    glyph_brush: ID2D1SolidColorBrush,
    cover_brush: ID2D1SolidColorBrush,
}

impl ControlBrushes {
    fn new(target: &ID2D1DCRenderTarget) -> Result<Self> {
        unsafe {
            Ok(Self {
                red_fill: target.CreateSolidColorBrush(&RED_FILL, None)?,
                red_border: target.CreateSolidColorBrush(&RED_BORDER, None)?,
                yellow_fill: target.CreateSolidColorBrush(&YELLOW_FILL, None)?,
                yellow_border: target.CreateSolidColorBrush(&YELLOW_BORDER, None)?,
                green_fill: target.CreateSolidColorBrush(&GREEN_FILL, None)?,
                green_border: target.CreateSolidColorBrush(&GREEN_BORDER, None)?,
                disabled_fill: target.CreateSolidColorBrush(&INACTIVE_DARK_FILL, None)?,
                disabled_border: target.CreateSolidColorBrush(&INACTIVE_DARK_BORDER, None)?,
                pressed_fill: target.CreateSolidColorBrush(&RED_FILL, None)?,
                pressed_border: target.CreateSolidColorBrush(&RED_BORDER, None)?,
                glyph_brush: target.CreateSolidColorBrush(&GLYPH_DARK, None)?,
                cover_brush: target.CreateSolidColorBrush(&CLEAR, None)?,
            })
        }
    }
}

pub struct WindowControlsRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    red_fill: ID2D1SolidColorBrush,
    red_border: ID2D1SolidColorBrush,
    yellow_fill: ID2D1SolidColorBrush,
    yellow_border: ID2D1SolidColorBrush,
    green_fill: ID2D1SolidColorBrush,
    green_border: ID2D1SolidColorBrush,
    disabled_fill: ID2D1SolidColorBrush,
    disabled_border: ID2D1SolidColorBrush,
    pressed_fill: ID2D1SolidColorBrush,
    pressed_border: ID2D1SolidColorBrush,
    glyph_brush: ID2D1SolidColorBrush,
    cover_brush: ID2D1SolidColorBrush,
    surface: DibSurface,
}

pub type TrafficLightRenderer = WindowControlsRenderer;

impl WindowControlsRenderer {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = create_dc_render_target(&factory)?;

            let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 2.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let stroke_style = factory.CreateStrokeStyle(&stroke_props, None)?;

            let brushes = ControlBrushes::new(&target)?;
            let surface = DibSurface::new(width, height)?;

            Ok(Self {
                factory,
                target,
                stroke_style,
                red_fill: brushes.red_fill,
                red_border: brushes.red_border,
                yellow_fill: brushes.yellow_fill,
                yellow_border: brushes.yellow_border,
                green_fill: brushes.green_fill,
                green_border: brushes.green_border,
                disabled_fill: brushes.disabled_fill,
                disabled_border: brushes.disabled_border,
                pressed_fill: brushes.pressed_fill,
                pressed_border: brushes.pressed_border,
                glyph_brush: brushes.glyph_brush,
                cover_brush: brushes.cover_brush,
                surface,
            })
        }
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = ControlBrushes::new(&target)?;
        self.target = target;
        self.red_fill = brushes.red_fill;
        self.red_border = brushes.red_border;
        self.yellow_fill = brushes.yellow_fill;
        self.yellow_border = brushes.yellow_border;
        self.green_fill = brushes.green_fill;
        self.green_border = brushes.green_border;
        self.disabled_fill = brushes.disabled_fill;
        self.disabled_border = brushes.disabled_border;
        self.pressed_fill = brushes.pressed_fill;
        self.pressed_border = brushes.pressed_border;
        self.glyph_brush = brushes.glyph_brush;
        self.cover_brush = brushes.cover_brush;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &mut self,
        hwnd: isize,
        origin: (i32, i32),
        layout: &WindowControlsLayout,
        state: WindowControlsState,
        maximized: bool,
        hovered: bool,
        pressed: Option<WindowControlAction>,
        appearance: Appearance,
        cover: Option<&CoverStyle>,
    ) -> Result<()> {
        let dark_surface = cover.map_or(appearance.theme == ResolvedTheme::Dark, |style| {
            style.is_dark
        });
        let contrast_fill = appearance.contrast.map(|system| d2d(system.window));
        draw_with_recovery(
            self,
            |renderer| {
                let high_contrast = renderer.apply_appearance(&appearance, dark_surface);
                renderer.draw(
                    layout,
                    state,
                    maximized,
                    hovered || high_contrast,
                    pressed,
                    cover,
                    contrast_fill,
                )
            },
            Self::recreate_target,
        )?;
        self.surface.present(hwnd, origin)
    }

    fn apply_appearance(&self, appearance: &Appearance, dark_surface: bool) -> bool {
        let (inactive_fill, inactive_border) = if dark_surface {
            (INACTIVE_DARK_FILL, INACTIVE_DARK_BORDER)
        } else {
            (INACTIVE_LIGHT_FILL, INACTIVE_LIGHT_BORDER)
        };
        let colors = match appearance.contrast {
            Some(system) => [
                d2d(system.window),
                d2d(system.window_text),
                d2d(system.window),
                d2d(system.window_text),
                d2d(system.window),
                d2d(system.window_text),
                d2d(system.window),
                d2d(system.gray_text),
                d2d(system.window_text),
            ],
            None => [
                RED_FILL,
                RED_BORDER,
                YELLOW_FILL,
                YELLOW_BORDER,
                GREEN_FILL,
                GREEN_BORDER,
                inactive_fill,
                inactive_border,
                GLYPH_DARK,
            ],
        };
        let brushes = [
            &self.red_fill,
            &self.red_border,
            &self.yellow_fill,
            &self.yellow_border,
            &self.green_fill,
            &self.green_border,
            &self.disabled_fill,
            &self.disabled_border,
            &self.glyph_brush,
        ];
        for (brush, color) in brushes.into_iter().zip(colors) {
            unsafe {
                brush.SetColor(&raw const color);
            }
        }
        appearance.contrast.is_some()
    }

    fn sync_inactive_palette(&self, dark_surface: bool) {
        let (fill, border) = if dark_surface {
            (INACTIVE_DARK_FILL, INACTIVE_DARK_BORDER)
        } else {
            (INACTIVE_LIGHT_FILL, INACTIVE_LIGHT_BORDER)
        };
        unsafe {
            self.disabled_fill.SetColor(&raw const fill);
            self.disabled_border.SetColor(&raw const border);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        layout: &WindowControlsLayout,
        state: WindowControlsState,
        maximized: bool,
        hovered: bool,
        pressed: Option<WindowControlAction>,
        cover: Option<&CoverStyle>,
        contrast_fill: Option<D2D1_COLOR_F>,
    ) -> Result<()> {
        if contrast_fill.is_none()
            && let Some(style) = cover
        {
            self.sync_inactive_palette(style.is_dark);
        }
        let show_active_glyphs = hovered || pressed.is_some();
        draw_on(
            &self.target,
            &self.surface,
            self.surface.size(),
            || unsafe {
                let clear = CLEAR;
                self.target.Clear(Some(&raw const clear));
                if let Some(style) = cover {
                    self.draw_cover(layout, style, contrast_fill)?;
                }

                let cy = layout.center_y;
                let r = layout.radius;
                let glyph_scale = (r / BASE_BUTTON_RADIUS).max(1.0);
                let slots = layout.slots.min(layout.centers_x.len());

                for slot in 0..slots {
                    let action = layout.actions[slot];
                    let is_pressed =
                        contrast_fill.is_none() && pressed == Some(action) && state.allows(action);
                    let (fill, border) = self.circle_brushes(
                        action,
                        state.shows_color(action, show_active_glyphs),
                        is_pressed,
                    );
                    self.draw_circle(&fill, &border, layout.centers_x[slot], cy, r);
                }

                if show_active_glyphs {
                    for slot in 0..slots {
                        let action = layout.actions[slot];
                        if !state.allows(action) {
                            continue;
                        }
                        let cx = layout.centers_x[slot];
                        match action {
                            WindowControlAction::Close => {
                                self.draw_close_glyph(&self.glyph_brush, cx, cy, glyph_scale);
                            }
                            WindowControlAction::Minimize => {
                                self.draw_minimize_glyph(&self.glyph_brush, cx, cy, glyph_scale);
                            }
                            WindowControlAction::ToggleMaximize => {
                                self.draw_zoom_glyph(
                                    &self.glyph_brush,
                                    cx,
                                    cy,
                                    maximized,
                                    glyph_scale,
                                )?;
                            }
                        }
                    }
                }
                Ok(())
            },
        )
    }

    fn circle_brushes(
        &self,
        action: WindowControlAction,
        enabled: bool,
        pressed: bool,
    ) -> (ID2D1SolidColorBrush, ID2D1SolidColorBrush) {
        if !enabled {
            return (self.disabled_fill.clone(), self.disabled_border.clone());
        }
        if pressed {
            let (fill, border) = base_colors_for(action);
            let dark_fill = pressed_color(fill);
            let dark_border = pressed_color(border);
            unsafe {
                self.pressed_fill.SetColor(&raw const dark_fill);
                self.pressed_border.SetColor(&raw const dark_border);
            }
            return (self.pressed_fill.clone(), self.pressed_border.clone());
        }
        match action {
            WindowControlAction::Close => (self.red_fill.clone(), self.red_border.clone()),
            WindowControlAction::Minimize => (self.yellow_fill.clone(), self.yellow_border.clone()),
            WindowControlAction::ToggleMaximize => {
                (self.green_fill.clone(), self.green_border.clone())
            }
        }
    }

    unsafe fn draw_cover(
        &self,
        layout: &WindowControlsLayout,
        style: &CoverStyle,
        contrast_fill: Option<D2D1_COLOR_F>,
    ) -> Result<()> {
        unsafe {
            let top = if style.top_border_inset { 1.0 } else { 0.0 };
            let right_inset = if style.right_border_inset { 1.0 } else { 0.0 };
            let right = (layout.width as f32 - right_inset).max(1.0);
            let height = layout.height as f32;
            if height <= top {
                return Ok(());
            }
            let corner_inset = if style.top_border_inset || style.right_border_inset {
                1.0
            } else {
                0.0
            };
            let r = (style.top_right_radius - corner_inset).max(0.0);
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.BeginFigure(Vector2::new(0.0, top), D2D1_FIGURE_BEGIN_FILLED);
            if r > 0.0 {
                sink.AddLine(Vector2::new(right - r, top));
                sink.AddArc(&D2D1_ARC_SEGMENT {
                    point: Vector2::new(right, top + r),
                    size: D2D_SIZE_F {
                        width: r,
                        height: r,
                    },
                    rotationAngle: 0.0,
                    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                    arcSize: D2D1_ARC_SIZE_SMALL,
                });
            } else {
                sink.AddLine(Vector2::new(right, top));
            }
            sink.AddLine(Vector2::new(right, height));
            sink.AddLine(Vector2::new(0.0, height));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;

            if let Some(color) = contrast_fill {
                self.cover_brush.SetColor(&raw const color);
                self.target.FillGeometry(&path, &self.cover_brush, None);
                return Ok(());
            }

            let bands = cover_row_bands(
                &style.rows,
                layout.height.max(0) as usize,
                style.top_border_inset,
            );
            let feather = style.feather_width.clamp(0.0, (right * 0.25).max(0.0));
            for band in bands {
                let color = rgb_to_d2d(band.rgb);
                self.cover_brush.SetColor(&raw const color);
                let rect_top = band.top as f32;
                let rect_bottom = band.bottom as f32;
                if feather > 0.0 {
                    let fade = linear_gradient(
                        &self.target,
                        Vector2::new(0.0, rect_top),
                        Vector2::new(feather, rect_top),
                        D2D1_COLOR_F { a: 0.0, ..color },
                        color,
                    )?;
                    let fade_rect = D2D_RECT_F {
                        left: 0.0,
                        top: rect_top,
                        right: feather,
                        bottom: rect_bottom,
                    };
                    if rect_top < top + r && feather > right - r {
                        self.target
                            .PushAxisAlignedClip(&fade_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                        self.target.FillGeometry(&path, &fade, None);
                        self.target.PopAxisAlignedClip();
                    } else {
                        self.target.FillRectangle(&fade_rect, &fade);
                    }
                }
                let solid_rect = D2D_RECT_F {
                    left: feather,
                    top: rect_top,
                    right,
                    bottom: rect_bottom,
                };
                if rect_top >= top + r {
                    self.target.FillRectangle(&solid_rect, &self.cover_brush);
                } else {
                    self.target
                        .PushAxisAlignedClip(&solid_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                    self.target.FillGeometry(&path, &self.cover_brush, None);
                    self.target.PopAxisAlignedClip();
                }
            }
        }
        Ok(())
    }

    unsafe fn draw_circle(
        &self,
        fill: &ID2D1SolidColorBrush,
        border: &ID2D1SolidColorBrush,
        cx: f32,
        cy: f32,
        radius: f32,
    ) {
        let ellipse = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillEllipse(&ellipse, fill);
            self.target
                .DrawEllipse(&ellipse, border, 0.5, Some(&self.stroke_style));
        }
    }

    unsafe fn draw_close_glyph(&self, brush: &ID2D1SolidColorBrush, cx: f32, cy: f32, scale: f32) {
        let arm = 3.0 * scale;
        let p0 = Vector2::new(cx - arm, cy - arm);
        let p1 = Vector2::new(cx + arm, cy + arm);
        let p2 = Vector2::new(cx + arm, cy - arm);
        let p3 = Vector2::new(cx - arm, cy + arm);
        unsafe {
            self.target
                .DrawLine(p0, p1, brush, 1.2 * scale, Some(&self.stroke_style));
            self.target
                .DrawLine(p2, p3, brush, 1.2 * scale, Some(&self.stroke_style));
        }
    }

    unsafe fn draw_minimize_glyph(
        &self,
        brush: &ID2D1SolidColorBrush,
        cx: f32,
        cy: f32,
        scale: f32,
    ) {
        let half = 3.5 * scale;
        let p0 = Vector2::new(cx - half, cy);
        let p1 = Vector2::new(cx + half, cy);
        unsafe {
            self.target
                .DrawLine(p0, p1, brush, 1.3 * scale, Some(&self.stroke_style));
        }
    }

    unsafe fn draw_zoom_glyph(
        &self,
        brush: &ID2D1SolidColorBrush,
        cx: f32,
        cy: f32,
        maximized: bool,
        scale: f32,
    ) -> Result<()> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            if maximized {
                let size = 2.6 * scale;
                let gap = 1.0 * scale;
                let left = cx - size - gap;
                let right = cx + gap;
                let top = cy - size * 0.5;
                let bottom = cy + size * 0.5;

                sink.BeginFigure(Vector2::new(left, top), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(Vector2::new(left + size, top));
                sink.AddLine(Vector2::new(left + size, bottom));
                sink.AddLine(Vector2::new(left, bottom));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);

                sink.BeginFigure(Vector2::new(right, top), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(Vector2::new(right + size, top));
                sink.AddLine(Vector2::new(right + size, bottom));
                sink.AddLine(Vector2::new(right, bottom));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            } else {
                let outer = 3.6 * scale;
                let leg = 2.4 * scale;

                sink.BeginFigure(
                    Vector2::new(cx - outer, cy - outer),
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                sink.AddLine(Vector2::new(cx - outer + leg, cy - outer));
                sink.AddLine(Vector2::new(cx - outer, cy - outer + leg));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);

                sink.BeginFigure(
                    Vector2::new(cx + outer, cy + outer),
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                sink.AddLine(Vector2::new(cx + outer - leg, cy + outer));
                sink.AddLine(Vector2::new(cx + outer, cy + outer - leg));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            sink.Close()?;
            self.target.FillGeometry(&path, brush, None);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_tests::SERIAL;
    use otto_core::{
        NativeCaption, NativeCaptionStyle, WindowControlsMetrics, compute_cover_layout,
    };
    use std::sync::PoisonError;

    const STYLE: NativeCaptionStyle = NativeCaptionStyle {
        has_caption: true,
        has_system_menu: true,
        can_minimize: true,
        can_maximize: true,
    };

    #[test]
    fn inset_cover_preserves_outer_border_and_inactive_buttons_stay_gray_until_hovered() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let caption = NativeCaption::new(STYLE, 500, 100, 30, true, 96).unwrap();
        let layout = compute_cover_layout(caption, &WindowControlsMetrics::default()).unwrap();
        let renderer = WindowControlsRenderer::new(layout.width, layout.height).unwrap();
        let mut rows = vec![[0x20, 0x20, 0x20]; 30];
        rows[0] = [0xFF, 0x00, 0x00];
        let cover = CoverStyle {
            rows,
            top_right_radius: caption.corner_radius(),
            top_border_inset: true,
            right_border_inset: true,
            feather_width: 0.0,
            is_dark: true,
        };
        let inactive = WindowControlsState {
            active: false,
            close: true,
            minimize: true,
            zoom: true,
        };
        renderer
            .draw(&layout, inactive, false, false, None, Some(&cover), None)
            .unwrap();
        let pixels = renderer.surface.read_pixels();
        let stride = (layout.width * 4) as usize;
        assert_eq!(pixels[20 * 4 + 3], 0);
        assert_eq!(
            pixels[15 * stride + ((layout.width - 1) as usize) * 4 + 3],
            0
        );
        assert_eq!(pixels[15 * stride + 10 * 4 + 3], 255);
        let close_x = layout.centers_x[2].round() as usize;
        let sample_y = (layout.center_y.round() as usize).saturating_sub(4);
        let idx = sample_y * stride + close_x * 4;
        let (b, g, r) = (pixels[idx], pixels[idx + 1], pixels[idx + 2]);
        assert!(
            r.abs_diff(g) <= 6 && g.abs_diff(b) <= 6,
            "inactive close button should be neutral gray (rgb={r},{g},{b})"
        );

        renderer
            .draw(&layout, inactive, false, true, None, Some(&cover), None)
            .unwrap();
        let hovered_pixels = renderer.surface.read_pixels();
        let (hb, hg, hr) = (
            hovered_pixels[idx],
            hovered_pixels[idx + 1],
            hovered_pixels[idx + 2],
        );
        drop(renderer);
        crate::release_shared_factories();
        assert!(
            hr > 180 && hg < 140 && hb < 140,
            "hovered inactive close button should light up red (rgb={hr},{hg},{hb})"
        );
    }

    #[test]
    fn feather_and_split_top_inset_and_pressed_circle_and_light_inactive_render_accurately() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let caption = NativeCaption::new(STYLE, 500, 100, 30, true, 96).unwrap();
        let layout = compute_cover_layout(caption, &WindowControlsMetrics::default()).unwrap();
        let renderer = WindowControlsRenderer::new(layout.width, layout.height).unwrap();
        let stride = (layout.width * 4) as usize;
        let light_cover = CoverStyle {
            rows: vec![[0xF3, 0xF3, 0xF3]; 30],
            top_right_radius: caption.corner_radius(),
            top_border_inset: false,
            right_border_inset: true,
            feather_width: 6.0,
            is_dark: false,
        };
        let active = WindowControlsState {
            active: true,
            close: true,
            minimize: true,
            zoom: true,
        };
        let inactive = WindowControlsState {
            active: false,
            ..active
        };
        renderer
            .draw(
                &layout,
                inactive,
                false,
                false,
                None,
                Some(&light_cover),
                None,
            )
            .unwrap();
        let light_pixels = renderer.surface.read_pixels();
        assert_eq!(light_pixels[20 * 4 + 3], 255);
        assert_eq!(
            light_pixels[15 * stride + ((layout.width - 1) as usize) * 4 + 3],
            0
        );
        let left_edge_alpha = light_pixels[15 * stride + 3];
        let mid_feather_alpha = light_pixels[15 * stride + 3 * 4 + 3];
        let solid_alpha = light_pixels[15 * stride + 7 * 4 + 3];
        assert!(left_edge_alpha < mid_feather_alpha);
        assert!(mid_feather_alpha < solid_alpha);
        assert_eq!(solid_alpha, 255);

        let close_x = layout.centers_x[2].round() as usize;
        let cy = layout.center_y.round() as usize;
        let close_idx = (cy - 4) * stride + close_x * 4;
        assert!(light_pixels[close_idx + 2] < 230 && light_pixels[close_idx + 2] > 170);

        renderer
            .draw(&layout, active, false, true, None, Some(&light_cover), None)
            .unwrap();
        let unpressed = renderer.surface.read_pixels();
        renderer
            .draw(
                &layout,
                active,
                false,
                true,
                Some(WindowControlAction::Close),
                Some(&light_cover),
                None,
            )
            .unwrap();
        let pressed = renderer.surface.read_pixels();
        drop(renderer);
        crate::release_shared_factories();
        assert!(pressed[close_idx + 2] < unpressed[close_idx + 2]);
    }
}
