use std::f32::consts::PI;

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::core::Result;
use windows_numerics::Vector2;

use super::ControlCenterRenderer;

impl ControlCenterRenderer {
    pub(super) fn draw_bluetooth_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        crate::draw_bluetooth_symbol(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx,
            cy,
            half,
            brush,
        )
    }

    pub(super) fn draw_battery_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        fraction: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let w = half * 1.48;
        let h = half * 0.88;
        let left = cx - w * 0.56;
        let top = cy - h * 0.5;
        let r = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + w,
                bottom: top + h,
            },
            radiusX: 2.2,
            radiusY: 2.2,
        };
        let fill_w = ((w - 4.0) * fraction.clamp(0.0, 1.0)).max(0.0);
        let fill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + 2.0,
                top: top + 2.0,
                right: left + 2.0 + fill_w,
                bottom: top + h - 2.0,
            },
            radiusX: 1.0,
            radiusY: 1.0,
        };
        let nub = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + w + 1.1,
                top: cy - h * 0.24,
                right: left + w + 2.6,
                bottom: cy + h * 0.24,
            },
            radiusX: 0.8,
            radiusY: 0.8,
        };
        unsafe {
            self.target.DrawRoundedRectangle(&r, brush, 1.25, None);
            self.target.FillRoundedRectangle(&nub, brush);
            if fill_w > 0.5 {
                self.target.FillRoundedRectangle(&fill, brush);
            }
        }
    }

    pub(super) fn draw_sun_glyph(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius * 0.52,
                    radiusY: radius * 0.52,
                },
                brush,
            );
            for i in 0..8 {
                let angle = (i as f32) * (PI * 0.25);
                let (s, c) = angle.sin_cos();
                self.target.DrawLine(
                    Vector2::new(cx + c * radius * 0.78, cy + s * radius * 0.78),
                    Vector2::new(cx + c * radius * 1.15, cy + s * radius * 1.15),
                    brush,
                    1.2,
                    &self.stroke_style,
                );
            }
        }
    }

    pub(super) fn draw_more_glyph(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let dot_r = (radius * 0.22).max(1.1);
        let spread = (radius * 0.72).max(3.2);
        unsafe {
            for offset in [-spread, 0.0, spread] {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx + offset, cy),
                        radiusX: dot_r,
                        radiusY: dot_r,
                    },
                    brush,
                );
            }
        }
    }

    pub(super) fn draw_note_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx - half * 0.3, cy + half * 0.45),
                    radiusX: half * 0.34,
                    radiusY: half * 0.26,
                },
                brush,
            );
            self.target.DrawLine(
                Vector2::new(cx, cy + half * 0.42),
                Vector2::new(cx, cy - half * 0.72),
                brush,
                1.5,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx, cy - half * 0.72),
                Vector2::new(cx + half * 0.55, cy - half * 0.46),
                brush,
                1.8,
                &self.stroke_style,
            );
        }
    }

    pub(super) fn draw_play_pause_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        playing: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            if playing {
                let bar_w = (half * 0.42).max(1.8);
                let gap = half * 0.35;
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left: cx - gap - bar_w,
                        top: cy - half,
                        right: cx - gap,
                        bottom: cy + half,
                    },
                    brush,
                );
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left: cx + gap,
                        top: cy - half,
                        right: cx + gap + bar_w,
                        bottom: cy + half,
                    },
                    brush,
                );
                return Ok(());
            }
            let tri = self.factory.CreatePathGeometry()?;
            let sink = tri.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - half * 0.55, cy - half),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + half * 0.85, cy));
            sink.AddLine(Vector2::new(cx - half * 0.55, cy + half));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&tri, brush, None);
        }
        Ok(())
    }

    pub(super) fn draw_skip_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        next: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let dir = if next { 1.0 } else { -1.0 };
        unsafe {
            let tri = self.factory.CreatePathGeometry()?;
            let sink = tri.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - half * 0.75 * dir, cy - half * 0.8),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + half * 0.40 * dir, cy));
            sink.AddLine(Vector2::new(cx - half * 0.75 * dir, cy + half * 0.8));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&tri, brush, None);
            let bar_x = cx + half * 0.58 * dir;
            self.target.DrawLine(
                Vector2::new(bar_x, cy - half * 0.78),
                Vector2::new(bar_x, cy + half * 0.78),
                brush,
                (half * 0.30).max(1.5),
                &self.stroke_style,
            );
        }
        Ok(())
    }
}
