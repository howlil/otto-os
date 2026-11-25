use std::cell::RefCell;

use otto_core::{
    Appearance, CoverStyle, Rect, ResolvedTheme, Rgba, WindowControlAction, WindowControlsLayout,
    WindowControlsState, cover_row_bands,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_CAP_STYLE_ROUND,
    D2D1_DASH_STYLE_SOLID, D2D1_ELLIPSE, D2D1_LINE_JOIN_ROUND, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1DCRenderTarget, ID2D1Factory, ID2D1LinearGradientBrush,
    ID2D1PathGeometry, ID2D1RenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery,
};
use crate::kit::draw::linear_gradient;
use crate::{CLEAR, shared_d2d_factory};

const BASE_BUTTON_RADIUS: f32 = 6.0;
const FEATHER_BRUSH_CACHE_LIMIT: usize = 32;
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

#[derive(Clone, Copy)]
pub(crate) struct HighContrastTrafficLights {
    pub(crate) window: D2D1_COLOR_F,
    pub(crate) text: D2D1_COLOR_F,
    pub(crate) disabled: D2D1_COLOR_F,
}

#[derive(Clone, Copy)]
pub(crate) struct TrafficLightsStyle {
    pub(crate) state: WindowControlsState,
    pub(crate) maximized: bool,
    pub(crate) hovered: bool,
    pub(crate) pressed: Option<WindowControlAction>,
    pub(crate) dark_surface: bool,
    pub(crate) high_contrast: Option<HighContrastTrafficLights>,
}

pub struct WindowControlsPresent<'a> {
    pub layout: &'a WindowControlsLayout,
    pub state: WindowControlsState,
    pub maximized: bool,
    pub hovered: bool,
    pub pressed: Option<WindowControlAction>,
    pub appearance: Appearance,
    pub cover: Option<&'a CoverStyle>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CoverPathKey {
    width: i32,
    height: i32,
    top_border_inset: bool,
    right_border_inset: bool,
    top_right_radius_bits: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FeatherBrushKey {
    rgb: [u8; 3],
    feather_bits: u32,
}

pub struct WindowControlsRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    brush: ID2D1SolidColorBrush,
    cover_brush: ID2D1SolidColorBrush,
    cached_cover_path: RefCell<Option<(CoverPathKey, ID2D1PathGeometry)>>,
    cached_feather_brushes: RefCell<Vec<(FeatherBrushKey, ID2D1LinearGradientBrush)>>,
    surface: DibSurface,
}

impl WindowControlsRenderer {
    pub fn new(width: i32, height: i32) -> Result<Self> {
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
        let (stroke_style, brush, cover_brush) = unsafe {
            (
                factory.CreateStrokeStyle(&stroke_props, None)?,
                target.CreateSolidColorBrush(&RED_FILL, None)?,
                target.CreateSolidColorBrush(&CLEAR, None)?,
            )
        };
        let surface = DibSurface::new(width, height)?;
        Ok(Self {
            factory,
            target,
            stroke_style,
            brush,
            cover_brush,
            cached_cover_path: RefCell::new(None),
            cached_feather_brushes: RefCell::new(Vec::new()),
            surface,
        })
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let (brush, cover_brush) = unsafe {
            (
                target.CreateSolidColorBrush(&RED_FILL, None)?,
                target.CreateSolidColorBrush(&CLEAR, None)?,
            )
        };
        self.target = target;
        self.brush = brush;
        self.cover_brush = cover_brush;
        if let Ok(mut cache) = self.cached_feather_brushes.try_borrow_mut() {
            cache.clear();
        }
        Ok(())
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        origin: (i32, i32),
        frame: &WindowControlsPresent<'_>,
    ) -> Result<()> {
        draw_with_recovery(self, |renderer| renderer.draw(frame), Self::recreate_target)?;
        self.surface.present(hwnd, origin)
    }

    fn draw(&self, frame: &WindowControlsPresent<'_>) -> Result<()> {
        let dark_surface = frame
            .cover
            .map_or(frame.appearance.theme == ResolvedTheme::Dark, |style| {
                style.is_dark
            });
        let contrast_fill = frame
            .appearance
            .contrast
            .map(|system| d2d_rgba(system.window));
        let high_contrast = frame
            .appearance
            .contrast
            .map(|system| HighContrastTrafficLights {
                window: d2d_rgba(system.window),
                text: d2d_rgba(system.window_text),
                disabled: d2d_rgba(system.gray_text),
            });
        let slots = frame.layout.slots.min(frame.layout.centers_x.len());
        let mut centers = [(WindowControlAction::Close, 0.0_f32, 0.0_f32); 3];
        for (slot, dest) in centers.iter_mut().enumerate().take(slots) {
            *dest = (
                frame.layout.actions[slot],
                frame.layout.centers_x[slot],
                frame.layout.center_y,
            );
        }
        draw_on(&self.target, &self.surface, self.surface.size(), || {
            let clear = CLEAR;
            unsafe {
                self.target.Clear(Some(&raw const clear));
            }
            if let Some(style) = frame.cover {
                self.draw_cover(frame.layout, style, contrast_fill)?;
            }
            draw_traffic_lights_centers(
                &self.factory,
                &self.target,
                &self.brush,
                &self.stroke_style,
                &centers[..slots],
                frame.layout.radius,
                TrafficLightsStyle {
                    state: frame.state,
                    maximized: frame.maximized,
                    hovered: frame.hovered,
                    pressed: frame.pressed,
                    dark_surface,
                    high_contrast,
                },
            )
        })
    }

    fn cover_path(
        &self,
        key: CoverPathKey,
        top: f32,
        right: f32,
        height: f32,
        r: f32,
    ) -> Result<ID2D1PathGeometry> {
        if let Ok(cached) = self.cached_cover_path.try_borrow()
            && let Some((held, path)) = cached.as_ref()
            && *held == key
        {
            return Ok(path.clone());
        }
        let path = unsafe {
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
            path
        };
        if let Ok(mut slot) = self.cached_cover_path.try_borrow_mut() {
            *slot = Some((key, path.clone()));
        }
        Ok(path)
    }

    fn feather_brush(
        &self,
        rgb: [u8; 3],
        color: D2D1_COLOR_F,
        feather: f32,
    ) -> Result<ID2D1LinearGradientBrush> {
        let key = FeatherBrushKey {
            rgb,
            feather_bits: feather.to_bits(),
        };
        if let Ok(cache) = self.cached_feather_brushes.try_borrow()
            && let Some((_, brush)) = cache.iter().find(|(k, _)| *k == key)
        {
            return Ok(brush.clone());
        }
        let brush = linear_gradient(
            &self.target,
            Vector2::new(0.0, 0.0),
            Vector2::new(feather, 0.0),
            D2D1_COLOR_F {
                a: 1.0 / 255.0,
                ..color
            },
            color,
        )?;
        if let Ok(mut cache) = self.cached_feather_brushes.try_borrow_mut() {
            if cache.len() >= FEATHER_BRUSH_CACHE_LIMIT {
                cache.remove(0);
            }
            cache.push((key, brush.clone()));
        }
        Ok(brush)
    }

    fn draw_cover(
        &self,
        layout: &WindowControlsLayout,
        style: &CoverStyle,
        contrast_fill: Option<D2D1_COLOR_F>,
    ) -> Result<()> {
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
        let path = self.cover_path(
            CoverPathKey {
                width: layout.width,
                height: layout.height,
                top_border_inset: style.top_border_inset,
                right_border_inset: style.right_border_inset,
                top_right_radius_bits: style.top_right_radius.to_bits(),
            },
            top,
            right,
            height,
            r,
        )?;

        if let Some(color) = contrast_fill {
            unsafe {
                self.cover_brush.SetColor(&raw const color);
                self.target.FillGeometry(&path, &self.cover_brush, None);
            }
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
            let rect_top = band.top as f32;
            let rect_bottom = band.bottom as f32;
            let fade = if feather > 0.0 {
                Some(self.feather_brush(band.rgb, color, feather)?)
            } else {
                None
            };
            unsafe {
                self.cover_brush.SetColor(&raw const color);
                if let Some(fade) = fade {
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
}

pub(crate) const fn d2d_rgba(color: Rgba) -> D2D1_COLOR_F {
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

const fn circle_colors(
    action: WindowControlAction,
    enabled: bool,
    pressed: bool,
    dark_surface: bool,
    high_contrast: Option<HighContrastTrafficLights>,
) -> (D2D1_COLOR_F, D2D1_COLOR_F) {
    if let Some(hc) = high_contrast {
        return if enabled {
            (hc.window, hc.text)
        } else {
            (hc.window, hc.disabled)
        };
    }
    if !enabled {
        return if dark_surface {
            (INACTIVE_DARK_FILL, INACTIVE_DARK_BORDER)
        } else {
            (INACTIVE_LIGHT_FILL, INACTIVE_LIGHT_BORDER)
        };
    }
    let (fill, border) = base_colors_for(action);
    if pressed {
        (pressed_color(fill), pressed_color(border))
    } else {
        (fill, border)
    }
}

pub(crate) fn draw_traffic_lights(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    stroke_style: &ID2D1StrokeStyle,
    slots: &[(WindowControlAction, Rect)],
    radius: f32,
    style: TrafficLightsStyle,
) -> Result<()> {
    let mut centers = [(WindowControlAction::Close, 0.0_f32, 0.0_f32); 3];
    let mut count = 0;
    for &(action, rect) in slots {
        if rect.is_empty() || count >= centers.len() {
            continue;
        }
        centers[count] = (
            action,
            rect.x as f32 + rect.width as f32 * 0.5,
            rect.y as f32 + rect.height as f32 * 0.5,
        );
        count += 1;
    }
    draw_traffic_lights_centers(
        factory,
        target,
        brush,
        stroke_style,
        &centers[..count],
        radius,
        style,
    )
}

fn draw_traffic_lights_centers(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    stroke_style: &ID2D1StrokeStyle,
    centers: &[(WindowControlAction, f32, f32)],
    radius: f32,
    style: TrafficLightsStyle,
) -> Result<()> {
    let show_glyphs = style.hovered || style.pressed.is_some() || style.high_contrast.is_some();
    let glyph_scale = (radius / BASE_BUTTON_RADIUS).max(0.85);
    let glyph_color = style.high_contrast.map_or(GLYPH_DARK, |hc| hc.text);

    for &(action, cx, cy) in centers {
        let enabled = style.state.shows_color(action, show_glyphs);
        let is_pressed = style.high_contrast.is_none()
            && style.pressed == Some(action)
            && style.state.allows(action);
        let (fill, border) = circle_colors(
            action,
            enabled,
            is_pressed,
            style.dark_surface,
            style.high_contrast,
        );
        let ellipse = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            brush.SetColor(&raw const fill);
            target.FillEllipse(&ellipse, brush);
            brush.SetColor(&raw const border);
            target.DrawEllipse(&ellipse, brush, 0.5, Some(stroke_style));
        }
    }

    if !show_glyphs {
        return Ok(());
    }

    unsafe {
        brush.SetColor(&raw const glyph_color);
    }
    for &(action, cx, cy) in centers {
        if !style.state.allows(action) {
            continue;
        }
        match action {
            WindowControlAction::Close => {
                draw_close_glyph(target, brush, stroke_style, cx, cy, glyph_scale);
            }
            WindowControlAction::Minimize => {
                draw_minimize_glyph(target, brush, stroke_style, cx, cy, glyph_scale);
            }
            WindowControlAction::ToggleMaximize => {
                if style.state.alt_zoom_glyph {
                    draw_plus_zoom_glyph(target, brush, stroke_style, cx, cy, glyph_scale);
                } else {
                    draw_zoom_glyph(factory, target, brush, cx, cy, style.maximized, glyph_scale)?;
                }
            }
        }
    }
    Ok(())
}

fn draw_close_glyph(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    stroke_style: &ID2D1StrokeStyle,
    cx: f32,
    cy: f32,
    scale: f32,
) {
    let arm = 2.8 * scale;
    let p0 = Vector2::new(cx - arm, cy - arm);
    let p1 = Vector2::new(cx + arm, cy + arm);
    let p2 = Vector2::new(cx + arm, cy - arm);
    let p3 = Vector2::new(cx - arm, cy + arm);
    unsafe {
        target.DrawLine(p0, p1, brush, 1.2 * scale, Some(stroke_style));
        target.DrawLine(p2, p3, brush, 1.2 * scale, Some(stroke_style));
    }
}

fn draw_minimize_glyph(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    stroke_style: &ID2D1StrokeStyle,
    cx: f32,
    cy: f32,
    scale: f32,
) {
    let half = 3.3 * scale;
    let p0 = Vector2::new(cx - half, cy);
    let p1 = Vector2::new(cx + half, cy);
    unsafe {
        target.DrawLine(p0, p1, brush, 1.3 * scale, Some(stroke_style));
    }
}

fn draw_plus_zoom_glyph(
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    stroke_style: &ID2D1StrokeStyle,
    cx: f32,
    cy: f32,
    scale: f32,
) {
    let half = 2.3 * scale;
    let stroke_width = 1.25 * scale;
    let h0 = Vector2::new(cx - half, cy);
    let h1 = Vector2::new(cx + half, cy);
    let v0 = Vector2::new(cx, cy - half);
    let v1 = Vector2::new(cx, cy + half);
    unsafe {
        target.DrawLine(h0, h1, brush, stroke_width, Some(stroke_style));
        target.DrawLine(v0, v1, brush, stroke_width, Some(stroke_style));
    }
}

fn draw_zoom_glyph(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    brush: &ID2D1SolidColorBrush,
    cx: f32,
    cy: f32,
    maximized: bool,
    scale: f32,
) -> Result<()> {
    unsafe {
        let path = factory.CreatePathGeometry()?;
        let sink = path.Open()?;
        if maximized {
            let size = 2.5 * scale;
            let gap = 0.9 * scale;
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
        } else {
            let outer = 3.4 * scale;
            let leg = 2.3 * scale;

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
        }
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        target.FillGeometry(&path, brush, None);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::PoisonError;

    use otto_core::{
        NativeCaption, NativeCaptionStyle, WindowControlsMetrics, compute_cover_layout,
    };

    use super::*;
    use crate::release_shared_factories;
    use crate::resource_tests::SERIAL;

    const STYLE: NativeCaptionStyle = NativeCaptionStyle {
        has_caption: true,
        has_system_menu: true,
        can_minimize: true,
        can_maximize: true,
    };

    fn render_sample(
        bg: D2D1_COLOR_F,
        style: TrafficLightsStyle,
    ) -> (Vec<u8>, [(WindowControlAction, Rect); 3]) {
        let factory = shared_d2d_factory().unwrap();
        let target = create_dc_render_target(&factory).unwrap();
        let brush = unsafe { target.CreateSolidColorBrush(&bg, None) }.unwrap();
        let stroke = unsafe {
            factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 2.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )
        }
        .unwrap();
        let surface = DibSurface::new(60, 32).unwrap();
        let slots = [
            (
                WindowControlAction::Close,
                Rect {
                    x: 0,
                    y: 0,
                    width: 20,
                    height: 32,
                },
            ),
            (
                WindowControlAction::Minimize,
                Rect {
                    x: 20,
                    y: 0,
                    width: 20,
                    height: 32,
                },
            ),
            (
                WindowControlAction::ToggleMaximize,
                Rect {
                    x: 40,
                    y: 0,
                    width: 20,
                    height: 32,
                },
            ),
        ];
        draw_on(&target, &surface, surface.size(), || {
            unsafe {
                target.Clear(Some(&raw const bg));
            }
            draw_traffic_lights(&factory, &target, &brush, &stroke, &slots, 5.5, style)
        })
        .unwrap();
        let pixels = surface.read_pixels();
        drop(surface);
        drop(stroke);
        drop(brush);
        drop(target);
        drop(factory);
        (pixels, slots)
    }

    fn rgb_at(pixels: &[u8], x: usize, y: usize) -> [u8; 3] {
        let offset = (y * 60 + x) * 4;
        [pixels[offset + 2], pixels[offset + 1], pixels[offset]]
    }

    #[test]
    fn active_circles_render_macos_red_yellow_green_and_pressed_darkens() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let active = WindowControlsState {
            active: true,
            close: true,
            minimize: true,
            zoom: true,
            alt_zoom_glyph: false,
        };
        let bg = D2D1_COLOR_F {
            r: 0.12,
            g: 0.12,
            b: 0.12,
            a: 1.0,
        };
        let (rest, _) = render_sample(
            bg,
            TrafficLightsStyle {
                state: active,
                maximized: false,
                hovered: false,
                pressed: None,
                dark_surface: true,
                high_contrast: None,
            },
        );
        let red = rgb_at(&rest, 10, 16);
        let yellow = rgb_at(&rest, 30, 16);
        let green = rgb_at(&rest, 50, 16);
        assert!(red[0] > 230 && red[1] < 120);
        assert!(yellow[0] > 230 && yellow[1] > 160 && yellow[2] < 80);
        assert!(green[1] > 180 && green[0] < 80);

        let (pressed, _) = render_sample(
            bg,
            TrafficLightsStyle {
                state: active,
                maximized: false,
                hovered: true,
                pressed: Some(WindowControlAction::Close),
                dark_surface: true,
                high_contrast: None,
            },
        );
        let pressed_red = rgb_at(&pressed, 10, 12);
        assert!(pressed_red[0] < red[0] - 30);
        release_shared_factories();
    }

    #[test]
    fn inactive_circles_remain_neutral_and_adapt_to_light_and_dark_surfaces() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let inactive = WindowControlsState {
            active: false,
            close: true,
            minimize: true,
            zoom: true,
            alt_zoom_glyph: false,
        };
        let dark_bg = D2D1_COLOR_F {
            r: 0.12,
            g: 0.12,
            b: 0.12,
            a: 1.0,
        };
        let light_bg = D2D1_COLOR_F {
            r: 0.95,
            g: 0.95,
            b: 0.95,
            a: 1.0,
        };
        let (dark_pixels, _) = render_sample(
            dark_bg,
            TrafficLightsStyle {
                state: inactive,
                maximized: false,
                hovered: false,
                pressed: None,
                dark_surface: true,
                high_contrast: None,
            },
        );
        let (light_pixels, _) = render_sample(
            light_bg,
            TrafficLightsStyle {
                state: inactive,
                maximized: false,
                hovered: false,
                pressed: None,
                dark_surface: false,
                high_contrast: None,
            },
        );
        let d = rgb_at(&dark_pixels, 10, 16);
        let l = rgb_at(&light_pixels, 10, 16);
        assert!(d[0].abs_diff(d[1]) <= 10);
        assert!(l[0].abs_diff(l[1]) <= 10);
        assert!(d[0] > 35 && l[0] < 235);
        release_shared_factories();
    }

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
            alt_zoom_glyph: false,
        };
        let appearance = Appearance::glass(ResolvedTheme::Dark);
        renderer
            .draw(&WindowControlsPresent {
                layout: &layout,
                state: inactive,
                maximized: false,
                hovered: false,
                pressed: None,
                appearance,
                cover: Some(&cover),
            })
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
            .draw(&WindowControlsPresent {
                layout: &layout,
                state: inactive,
                maximized: false,
                hovered: true,
                pressed: None,
                appearance,
                cover: Some(&cover),
            })
            .unwrap();
        let hovered_pixels = renderer.surface.read_pixels();
        let (hb, hg, hr) = (
            hovered_pixels[idx],
            hovered_pixels[idx + 1],
            hovered_pixels[idx + 2],
        );
        drop(renderer);
        release_shared_factories();
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
            alt_zoom_glyph: false,
        };
        let inactive = WindowControlsState {
            active: false,
            ..active
        };
        let appearance = Appearance::glass(ResolvedTheme::Light);
        renderer
            .draw(&WindowControlsPresent {
                layout: &layout,
                state: inactive,
                maximized: false,
                hovered: false,
                pressed: None,
                appearance,
                cover: Some(&light_cover),
            })
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
            .draw(&WindowControlsPresent {
                layout: &layout,
                state: active,
                maximized: false,
                hovered: true,
                pressed: None,
                appearance,
                cover: Some(&light_cover),
            })
            .unwrap();
        let unpressed = renderer.surface.read_pixels();
        renderer
            .draw(&WindowControlsPresent {
                layout: &layout,
                state: active,
                maximized: false,
                hovered: true,
                pressed: Some(WindowControlAction::Close),
                appearance,
                cover: Some(&light_cover),
            })
            .unwrap();
        let pressed = renderer.surface.read_pixels();
        drop(renderer);
        release_shared_factories();
        assert!(pressed[close_idx + 2] < unpressed[close_idx + 2]);
    }

    #[test]
    fn alt_zoom_glyph_draws_centered_plus_on_green_button() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let caption = NativeCaption::new(STYLE, 500, 100, 30, true, 96).unwrap();
        let layout = compute_cover_layout(caption, &WindowControlsMetrics::default()).unwrap();
        let renderer = WindowControlsRenderer::new(layout.width, layout.height).unwrap();
        let stride = (layout.width * 4) as usize;
        let cover = CoverStyle {
            rows: vec![[0x20, 0x20, 0x20]; 30],
            top_right_radius: caption.corner_radius(),
            top_border_inset: true,
            right_border_inset: true,
            feather_width: 0.0,
            is_dark: true,
        };
        let state = WindowControlsState {
            active: true,
            close: true,
            minimize: true,
            zoom: true,
            alt_zoom_glyph: false,
        };
        let appearance = Appearance::glass(ResolvedTheme::Dark);
        renderer
            .draw(&WindowControlsPresent {
                layout: &layout,
                state,
                maximized: false,
                hovered: true,
                pressed: None,
                appearance,
                cover: Some(&cover),
            })
            .unwrap();
        let standard_pixels = renderer.surface.read_pixels();
        renderer
            .draw(&WindowControlsPresent {
                layout: &layout,
                state: state.with_alt_zoom_glyph(true),
                maximized: false,
                hovered: true,
                pressed: None,
                appearance,
                cover: Some(&cover),
            })
            .unwrap();
        let alt_pixels = renderer.surface.read_pixels();
        drop(renderer);
        release_shared_factories();

        let zoom_x = layout.centers_x[1].round() as usize;
        let cy = layout.center_y.round() as usize;
        let center_idx = cy * stride + zoom_x * 4;
        assert!(
            standard_pixels[center_idx + 1] > 170,
            "standard zoom glyph leaves green center open"
        );
        assert!(
            alt_pixels[center_idx + 1] < 90,
            "alt plus glyph paints dark stroke at green button center"
        );
    }
}
