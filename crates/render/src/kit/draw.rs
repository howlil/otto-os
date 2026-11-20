use otto_core::{Rect, ShapeTokens, scale};
use std::f32::consts::PI;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
    D2D1_GRADIENT_STOP,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE,
    D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    D2D1_ROUNDED_RECT, D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1LinearGradientBrush, ID2D1PathGeometry,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_MEASURING_MODE_NATURAL, IDWriteTextFormat};
use windows::core::Result;
use windows_numerics::{Matrix3x2, Vector2};

use crate::FOREGROUND;
use crate::Renderer;
use crate::build_squircle_path;
use crate::draw_dwrite_tabular_text;
use crate::rect_to_d2d;

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
        let rect = rect_to_d2d(box_rect);
        if radius >= 6.0
            && let Ok(geom) = build_squircle_path(&self.factory, rect, radius)
        {
            unsafe {
                self.target.FillGeometry(&geom, brush, None);
            }
            return;
        }
        let pill = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&pill, brush);
        }
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
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
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
