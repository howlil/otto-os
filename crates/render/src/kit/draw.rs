use otto_core::{Appearance, GlassShape, Rect, ShapeTokens, scale};
use std::f32::consts::PI;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
    D2D1_GRADIENT_STOP,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL,
    D2D1_DRAW_TEXT_OPTIONS, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1Brush, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1LinearGradientBrush, ID2D1PathGeometry, ID2D1RenderTarget,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS, IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::{Matrix3x2, Vector2};

use crate::FOREGROUND;
use crate::Renderer;
use crate::appearance::glass::GlassPainter;
use crate::appearance::theme::{BrushPalette, LiquidGlassPalette};
use crate::build_squircle_path;
use crate::draw_dwrite_tabular_text;

pub(crate) const fn rect_to_d2d(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: rect.right() as f32,
        bottom: rect.bottom() as f32,
    }
}
use crate::shared_dwrite_factory;

pub(crate) fn center_of(rect: Rect) -> Vector2 {
    Vector2 {
        X: rect.x as f32 + rect.width as f32 * 0.5,
        Y: rect.y as f32 + rect.height as f32 * 0.5,
    }
}

pub(crate) fn inset_rect_f(rect: D2D_RECT_F, inset: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left + inset,
        top: rect.top + inset,
        right: (rect.right - inset).max(rect.left + inset),
        bottom: (rect.bottom - inset).max(rect.top + inset),
    }
}

pub(crate) fn squircle_threshold(dpi: u32) -> f32 {
    scale(6, dpi) as f32
}

fn should_use_squircle(rect: D2D_RECT_F, radius: f32, dpi: u32) -> bool {
    let min_dim = (rect.right - rect.left).min(rect.bottom - rect.top);
    radius >= squircle_threshold(dpi) && radius + 0.5 < min_dim * 0.5
}

pub(crate) fn linear_gradient(
    target: &ID2D1RenderTarget,
    start: Vector2,
    end: Vector2,
    from: D2D1_COLOR_F,
    to: D2D1_COLOR_F,
) -> Result<ID2D1LinearGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: from,
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: to,
        },
    ];
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        target.CreateLinearGradientBrush(
            &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: start,
                endPoint: end,
            },
            None,
            &collection,
        )
    }
}

pub(crate) fn fill_shape(
    target: &ID2D1RenderTarget,
    rect: D2D_RECT_F,
    radius: f32,
    geom: Option<&ID2D1PathGeometry>,
    brush: &ID2D1Brush,
) {
    if let Some(geom) = geom {
        unsafe {
            target.FillGeometry(geom, brush, None);
        }
        return;
    }
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: radius,
        radiusY: radius,
    };
    unsafe {
        target.FillRoundedRectangle(&rounded, brush);
    }
}

pub(crate) fn stroke_shape(
    target: &ID2D1RenderTarget,
    rect: D2D_RECT_F,
    radius: f32,
    geom: Option<&ID2D1PathGeometry>,
    brush: &ID2D1Brush,
    stroke_width: f32,
) {
    if let Some(geom) = geom {
        unsafe {
            target.DrawGeometry(geom, brush, stroke_width, None);
        }
        return;
    }
    let rounded = D2D1_ROUNDED_RECT {
        rect,
        radiusX: radius,
        radiusY: radius,
    };
    unsafe {
        target.DrawRoundedRectangle(&rounded, brush, stroke_width, None);
    }
}

pub(crate) fn fill_squircle_or_rounded_f(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    rect: D2D_RECT_F,
    radius: f32,
    dpi: u32,
    brush: &ID2D1Brush,
) {
    let geom = should_use_squircle(rect, radius, dpi)
        .then(|| build_squircle_path(factory, rect, radius).ok())
        .flatten();
    fill_shape(target, rect, radius, geom.as_ref(), brush);
}

pub(crate) fn fill_squircle_or_rounded(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    box_rect: Rect,
    radius: f32,
    dpi: u32,
    brush: &ID2D1Brush,
) {
    fill_squircle_or_rounded_f(factory, target, rect_to_d2d(box_rect), radius, dpi, brush);
}

pub(crate) fn stroke_squircle_or_rounded_f(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    rect: D2D_RECT_F,
    radius: f32,
    dpi: u32,
    brush: &ID2D1Brush,
    stroke_width: f32,
) {
    let geom = should_use_squircle(rect, radius, dpi)
        .then(|| build_squircle_path(factory, rect, radius).ok())
        .flatten();
    stroke_shape(target, rect, radius, geom.as_ref(), brush, stroke_width);
}

pub(crate) fn stroke_squircle_or_rounded(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    box_rect: Rect,
    radius: f32,
    dpi: u32,
    brush: &ID2D1Brush,
    stroke_width: f32,
) {
    let inset = stroke_width * 0.5;
    let rect = inset_rect_f(rect_to_d2d(box_rect), inset);
    stroke_squircle_or_rounded_f(
        factory,
        target,
        rect,
        (radius - inset).max(0.0),
        dpi,
        brush,
        stroke_width,
    );
}

pub(crate) fn draw_focus_ring_for(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    rect: D2D_RECT_F,
    radius: f32,
    dpi: u32,
    brush: &ID2D1Brush,
) {
    let stroke = ShapeTokens::GOLDEN_GATE.scaled(dpi).focus_ring_width as f32;
    let inset = stroke * 0.5;
    let ring_rect = inset_rect_f(rect, inset);
    let ring_radius = (radius - inset).max(1.0);
    stroke_squircle_or_rounded_f(factory, target, ring_rect, ring_radius, dpi, brush, stroke);
}

pub(crate) fn measure_text_width(
    text: &str,
    format: &IDWriteTextFormat,
    max_width: f32,
) -> Option<f32> {
    if text.is_empty() {
        return Some(0.0);
    }
    let dwrite = shared_dwrite_factory().ok()?;
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let layout = dwrite
            .CreateTextLayout(&wide, format, max_width.max(1.0), 64.0)
            .ok()?;
        let mut metrics = DWRITE_TEXT_METRICS::default();
        layout.GetMetrics(&raw mut metrics).ok()?;
        Some(metrics.widthIncludingTrailingWhitespace)
    }
}

pub(crate) fn draw_text_on(
    target: &ID2D1RenderTarget,
    text: &str,
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
    options: D2D1_DRAW_TEXT_OPTIONS,
) {
    if text.is_empty() || rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            format,
            &rect,
            brush,
            options,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
}

pub(crate) fn draw_framed_squircle_shell(
    factory: &ID2D1Factory,
    target: &ID2D1RenderTarget,
    bounds: Rect,
    radius: f32,
    fill: &ID2D1Brush,
    outer_border: &ID2D1Brush,
    inner_rim: &ID2D1Brush,
) {
    let raw = rect_to_d2d(bounds);
    let outer_rect = inset_rect_f(raw, 0.5);
    let inner_rect = inset_rect_f(raw, 1.5);
    let inner_radius = (radius - 1.0).max(1.0);
    let outer_geom = build_squircle_path(factory, outer_rect, radius).ok();
    let inner_geom = build_squircle_path(factory, inner_rect, inner_radius).ok();
    fill_shape(target, outer_rect, radius, outer_geom.as_ref(), fill);
    stroke_shape(
        target,
        outer_rect,
        radius,
        outer_geom.as_ref(),
        outer_border,
        1.0,
    );
    stroke_shape(
        target,
        inner_rect,
        inner_radius,
        inner_geom.as_ref(),
        inner_rim,
        1.0,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_glass_or_fallback_shell(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    glass: &GlassPainter,
    brushes: &BrushPalette,
    bounds: Rect,
    radius: f32,
    dpi: u32,
    appearance: &Appearance,
    palette: &LiquidGlassPalette,
) -> Result<()> {
    let shape = GlassShape::from_rect(bounds, radius);
    if glass.draw(target, appearance, dpi, shape, true)? {
        return Ok(());
    }
    let fill = brushes.color_brush(palette.menu_fill);
    let outer_border = brushes.color_brush(palette.menu_outer_border);
    draw_framed_squircle_shell(
        factory,
        target,
        bounds,
        radius,
        &fill,
        &outer_border,
        &brushes.specular_rim,
    );
    Ok(())
}

impl Renderer {
    pub(crate) fn set_content_offset(&self, dx: f32, dy: f32) {
        let (dx, dy) = (dx.round(), dy.round());
        self.content_offset.set((dx, dy));
        let transform = Matrix3x2::translation(dx, dy);
        unsafe {
            self.target.SetTransform(&raw const transform);
        }
    }

    pub(crate) fn draw_control_focus_ring(&self, rect: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let stroke = shape.focus_ring_width as f32;
        let grow = scale(3, self.dpi) as f32 + stroke * 0.5;
        let ring = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.x as f32 - grow,
                top: rect.y as f32 - grow,
                right: rect.right() as f32 + grow,
                bottom: rect.bottom() as f32 + grow,
            },
            radiusX: rect.width as f32 * 0.5 + grow,
            radiusY: rect.height as f32 * 0.5 + grow,
        };
        unsafe {
            self.target.DrawRoundedRectangle(&ring, brush, stroke, None);
        }
    }

    pub(crate) fn push_clip(&self, rect: D2D_RECT_F) {
        unsafe {
            self.target
                .PushAxisAlignedClip(&raw const rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
    }

    pub(crate) fn pop_clip(&self) {
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    pub(crate) fn set_content_scale(&self, factor: f32, anchor_x: f32, anchor_y: f32) {
        let (offset_x, offset_y) = self.content_offset.get();
        let transform = Matrix3x2 {
            M11: factor,
            M12: 0.0,
            M21: 0.0,
            M22: factor,
            M31: offset_x + anchor_x * (1.0 - factor),
            M32: offset_y + anchor_y * (1.0 - factor),
        };
        unsafe {
            self.target.SetTransform(&raw const transform);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_progress_ring(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        progress: f32,
        active_brush: &ID2D1SolidColorBrush,
        track_brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let circle = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawEllipse(&circle, track_brush, stroke, &self.stroke_style);

            let clamped = progress.clamp(0.0, 1.0);
            if clamped <= 0.005 {
                return Ok(());
            }
            if clamped >= 0.998 {
                self.target
                    .DrawEllipse(&circle, active_brush, stroke, &self.stroke_style);
                return Ok(());
            }

            let start_angle = -PI * 0.5;
            let sweep = clamped * PI * 2.0;
            let end_angle = start_angle + sweep;
            let start = Vector2::new(
                cx + radius * start_angle.cos(),
                cy + radius * start_angle.sin(),
            );
            let end = Vector2::new(cx + radius * end_angle.cos(), cy + radius * end_angle.sin());
            let arc_size = if sweep > PI {
                D2D1_ARC_SIZE_LARGE
            } else {
                D2D1_ARC_SIZE_SMALL
            };

            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: arc_size,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, active_brush, stroke, &self.stroke_style);
        }
        Ok(())
    }

    pub(crate) fn draw_circle_button(&self, rect: Rect, brush: &ID2D1SolidColorBrush) {
        let r = rect.width.min(rect.height) as f32 * 0.5;
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: r,
                    radiusY: r,
                },
                brush,
            );
        }
    }

    pub(crate) fn draw_rounded_box(
        &self,
        box_rect: Rect,
        radius: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        fill_squircle_or_rounded(
            &self.factory,
            &self.target,
            box_rect,
            radius,
            self.dpi,
            brush,
        );
    }

    pub(crate) fn draw_rounded_box_stroke(
        &self,
        box_rect: Rect,
        radius: f32,
        brush: &ID2D1SolidColorBrush,
        stroke_width: f32,
    ) {
        stroke_squircle_or_rounded(
            &self.factory,
            &self.target,
            box_rect,
            radius,
            self.dpi,
            brush,
            stroke_width,
        );
    }

    pub(crate) fn notch_ink(&self) -> D2D1_COLOR_F {
        self.brushes.notch_ink()
    }

    pub(crate) fn draw_notch_rim(
        &self,
        geom: &ID2D1PathGeometry,
        top_y: f32,
        bottom_y: f32,
        is_high_contrast: bool,
    ) {
        if is_high_contrast {
            unsafe {
                self.target
                    .DrawGeometry(geom, &self.brushes.specular_rim, 1.0, &self.stroke_style);
            }
            return;
        }
        let (top, mid, bottom) = Appearance::glass(otto_core::ResolvedTheme::Dark)
            .colors()
            .notch_rim_stops(false);
        self.draw_directional_rim(geom, top_y, bottom_y, top.a, mid.a, bottom.a);
    }

    pub(crate) fn draw_directional_rim(
        &self,
        geom: &ID2D1PathGeometry,
        top_y: f32,
        bottom_y: f32,
        top_alpha: f32,
        mid_alpha: f32,
        bottom_alpha: f32,
    ) {
        let stops = [
            D2D1_GRADIENT_STOP {
                position: 0.0,
                color: D2D1_COLOR_F {
                    a: top_alpha,
                    ..FOREGROUND
                },
            },
            D2D1_GRADIENT_STOP {
                position: 0.5,
                color: D2D1_COLOR_F {
                    a: mid_alpha,
                    ..FOREGROUND
                },
            },
            D2D1_GRADIENT_STOP {
                position: 1.0,
                color: D2D1_COLOR_F {
                    a: bottom_alpha,
                    ..FOREGROUND
                },
            },
        ];
        let end_y = bottom_y.max(top_y + 1.0);
        let brush: Option<ID2D1LinearGradientBrush> = unsafe {
            self.target
                .CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)
                .and_then(|collection| {
                    self.target.CreateLinearGradientBrush(
                        &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                            startPoint: Vector2::new(0.0, top_y),
                            endPoint: Vector2::new(0.0, end_y),
                        },
                        None,
                        &collection,
                    )
                })
                .ok()
        };
        unsafe {
            if let Some(gradient) = brush {
                self.target
                    .DrawGeometry(geom, &gradient, 1.0, &self.stroke_style);
            } else {
                let fallback = self.brushes.alpha_brush(FOREGROUND, mid_alpha);
                self.target
                    .DrawGeometry(geom, &fallback, 1.0, &self.stroke_style);
            }
        }
    }
}

impl Renderer {
    pub(crate) fn draw_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        draw_text_on(
            &self.target,
            text,
            format,
            rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
        );
    }

    pub(crate) fn draw_tabular_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        draw_dwrite_tabular_text(&self.target, text, format, rect, brush);
    }
}

pub(crate) fn fill_squircle_bitmap(
    factory: &windows::Win32::Graphics::Direct2D::ID2D1Factory,
    target: &windows::Win32::Graphics::Direct2D::ID2D1DCRenderTarget,
    rect: Rect,
    circular: bool,
    radius: f32,
    bitmap: &windows::Win32::Graphics::Direct2D::ID2D1Bitmap,
    alpha: f32,
) -> bool {
    let d2d_rect = rect_to_d2d(rect);
    unsafe {
        let size = bitmap.GetSize();
        if size.width <= 0.0 || size.height <= 0.0 {
            return false;
        }
        let properties = windows::Win32::Graphics::Direct2D::D2D1_BRUSH_PROPERTIES {
            opacity: alpha,
            transform: Matrix3x2 {
                M11: rect.width as f32 / size.width,
                M12: 0.0,
                M21: 0.0,
                M22: rect.height as f32 / size.height,
                M31: rect.x as f32,
                M32: rect.y as f32,
            },
        };
        let bitmap_properties = windows::Win32::Graphics::Direct2D::D2D1_BITMAP_BRUSH_PROPERTIES {
            extendModeX: D2D1_EXTEND_MODE_CLAMP,
            extendModeY: D2D1_EXTEND_MODE_CLAMP,
            interpolationMode:
                windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
        };
        let Ok(brush) = target.CreateBitmapBrush(
            bitmap,
            Some(&raw const bitmap_properties),
            Some(&raw const properties),
        ) else {
            return false;
        };
        if circular {
            let cx = rect.x as f32 + rect.width as f32 * 0.5;
            let cy = rect.y as f32 + rect.height as f32 * 0.5;
            let r = rect.width as f32 * 0.5;
            target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: r,
                    radiusY: r,
                },
                &brush,
            );
        } else if let Ok(geom) = build_squircle_path(factory, d2d_rect, radius) {
            target.FillGeometry(&geom, &brush, None);
        } else {
            let shape = D2D1_ROUNDED_RECT {
                rect: d2d_rect,
                radiusX: radius,
                radiusY: radius,
            };
            target.FillRoundedRectangle(&shape, &brush);
        }
    }
    true
}

pub(crate) fn fill_ellipse_on(
    target: &ID2D1RenderTarget,
    center: Vector2,
    rx: f32,
    ry: f32,
    brush: &ID2D1Brush,
) {
    let ellipse = D2D1_ELLIPSE {
        point: center,
        radiusX: rx,
        radiusY: ry,
    };
    unsafe {
        target.FillEllipse(&ellipse, brush);
    }
}

pub(crate) fn stroke_ellipse_on(
    target: &ID2D1RenderTarget,
    center: Vector2,
    rx: f32,
    ry: f32,
    brush: &ID2D1Brush,
    stroke_width: f32,
) {
    let ellipse = D2D1_ELLIPSE {
        point: center,
        radiusX: rx,
        radiusY: ry,
    };
    unsafe {
        target.DrawEllipse(&ellipse, brush, stroke_width, None);
    }
}

pub(crate) fn draw_line_on(
    target: &ID2D1RenderTarget,
    a: Vector2,
    b: Vector2,
    brush: &ID2D1Brush,
    stroke_width: f32,
    stroke_style: Option<&ID2D1StrokeStyle>,
) {
    unsafe {
        target.DrawLine(a, b, brush, stroke_width, stroke_style);
    }
}

pub(crate) fn draw_bitmap_rect_on(
    target: &ID2D1RenderTarget,
    bitmap: &windows::Win32::Graphics::Direct2D::ID2D1Bitmap,
    dest: D2D_RECT_F,
    opacity: f32,
) {
    unsafe {
        target.DrawBitmap(
            bitmap,
            Some(&raw const dest),
            opacity,
            windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            None,
        );
    }
}

pub(crate) fn push_axis_clip_on(target: &ID2D1RenderTarget, rect: D2D_RECT_F) {
    unsafe {
        target.PushAxisAlignedClip(&rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    }
}

pub(crate) fn pop_axis_clip_on(target: &ID2D1RenderTarget) {
    unsafe {
        target.PopAxisAlignedClip();
    }
}
