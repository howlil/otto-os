use otto_core::scale;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE, D2D1_ARC_SIZE_SMALL, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
    D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::GLYPH_STROKE_SCALE;
use crate::Renderer;

impl Renderer {
    pub(crate) fn draw_play_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - r * 0.55, cy - r * 0.85),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + r * 0.90, cy));
            sink.AddLine(Vector2::new(cx - r * 0.55, cy + r * 0.85));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geom, brush, None);
        }
        Ok(())
    }

    pub(crate) fn draw_pause_glyph(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) {
        let bar_w = (r * 0.42).max(2.2);
        let bar_h = r * 1.55;
        let gap = r * 0.36;
        for dx in [-gap - bar_w * 0.5, gap + bar_w * 0.5] {
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: cx + dx - bar_w * 0.5,
                    top: cy - bar_h * 0.5,
                    right: cx + dx + bar_w * 0.5,
                    bottom: cy + bar_h * 0.5,
                },
                radiusX: bar_w * 0.45,
                radiusY: bar_w * 0.45,
            };
            unsafe {
                self.target.FillRoundedRectangle(&pill, brush);
            }
        }
    }

    pub(crate) fn draw_reset_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.28).max(1.5);
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            let start = Vector2::new(cx, cy - r);
            let end = Vector2::new(cx - r * 0.86, cy - r * 0.50);
            sink.BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end,
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_LARGE,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, brush, stroke, &self.stroke_style);

            self.target.DrawLine(
                start,
                Vector2::new(cx - r * 0.45, cy - r * 1.25),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                start,
                Vector2::new(cx - r * 0.40, cy - r * 0.65),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    pub(crate) fn draw_skip_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.30).max(1.6);
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - r * 0.75, cy - r * 0.72),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + r * 0.35, cy));
            sink.AddLine(Vector2::new(cx - r * 0.75, cy + r * 0.72));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geom, brush, None);

            self.target.DrawLine(
                Vector2::new(cx + r * 0.62, cy - r * 0.68),
                Vector2::new(cx + r * 0.62, cy + r * 0.68),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    pub(crate) fn draw_previous_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.30).max(1.6);
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx + r * 0.75, cy - r * 0.72),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx - r * 0.35, cy));
            sink.AddLine(Vector2::new(cx + r * 0.75, cy + r * 0.72));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geom, brush, None);

            self.target.DrawLine(
                Vector2::new(cx - r * 0.62, cy - r * 0.68),
                Vector2::new(cx - r * 0.62, cy + r * 0.68),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    pub(crate) fn draw_music_note_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (r * GLYPH_STROKE_SCALE).max(1.5);
        let head_radius_x = r * 0.38;
        let head_radius_y = r * 0.28;
        let gap = r * 0.75;
        let x1 = cx - gap * 0.5;
        let x2 = cx + gap * 0.5;
        let y_head1 = cy + r * 0.45;
        let y_head2 = cy + r * 0.25;
        let y_top = cy - r * 0.70;
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(x1, y_head1),
                    radiusX: head_radius_x,
                    radiusY: head_radius_y,
                },
                brush,
            );
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(x2, y_head2),
                    radiusX: head_radius_x,
                    radiusY: head_radius_y,
                },
                brush,
            );
            self.target.DrawLine(
                Vector2::new(x1 + head_radius_x * 0.6, y_head1),
                Vector2::new(x1 + head_radius_x * 0.6, y_top),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(x2 + head_radius_x * 0.6, y_head2),
                Vector2::new(x2 + head_radius_x * 0.6, y_top - r * 0.2),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(x1 + head_radius_x * 0.6, y_top),
                Vector2::new(x2 + head_radius_x * 0.6, y_top - r * 0.2),
                brush,
                stroke * 1.6,
                &self.stroke_style,
            );
        }
    }

    pub(crate) fn draw_equalizer_bars(
        &self,
        cx: f32,
        cy: f32,
        max_h: f32,
        bars: [f32; 4],
        brush: &ID2D1SolidColorBrush,
    ) {
        let bar_w = (scale(2, self.dpi) as f32).max(1.8);
        let gap = (scale(2, self.dpi) as f32).max(1.8);
        let min_h = (scale(3, self.dpi) as f32).min(max_h * 0.45);
        let offsets = [-1.5f32, -0.5, 0.5, 1.5];
        for (i, &dx_mult) in offsets.iter().enumerate() {
            let bx = cx + dx_mult * (bar_w + gap);
            let level = bars[i].clamp(0.0, 1.0);
            let h = min_h + level * (max_h - min_h).max(0.0);
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: bx - bar_w * 0.5,
                    top: cy - h * 0.5,
                    right: bx + bar_w * 0.5,
                    bottom: cy + h * 0.5,
                },
                radiusX: bar_w * 0.45,
                radiusY: bar_w * 0.45,
            };
            unsafe {
                self.target.FillRoundedRectangle(&pill, brush);
            }
        }
    }

    pub(crate) fn draw_audio_equalizer(
        &self,
        cx: f32,
        cy: f32,
        anim_phase: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let phases = [0.0f32, 1.6, 3.1, 4.7];
        let speeds = [5.5f32, 7.8, 6.2, 8.5];
        let bars = std::array::from_fn(|i| {
            ((anim_phase * speeds[i] + phases[i]).sin() * 0.5 + 0.5).clamp(0.0, 1.0)
        });
        let max_h = scale(12, self.dpi) as f32;
        self.draw_equalizer_bars(cx, cy, max_h, bars, brush);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_wifi_glyph(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    cx: f32,
    cy: f32,
    radius: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let base_y = cy + radius * 0.56;
    unsafe {
        let dot = D2D1_ELLIPSE {
            point: Vector2::new(cx, base_y),
            radiusX: stroke * 0.80,
            radiusY: stroke * 0.80,
        };
        target.FillEllipse(&dot, brush);

        for factor in [0.54_f32, 0.96_f32] {
            let r = radius * factor;
            let dx = r * 0.72;
            let dy = r * 0.72;
            let geometry = factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(Vector2::new(cx - dx, base_y - dy), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(cx + dx, base_y - dy),
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            target.DrawGeometry(&geometry, brush, stroke, stroke_style);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_speaker_glyph(
    factory: &ID2D1Factory,
    target: &ID2D1DCRenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    x: f32,
    cy: f32,
    half_h: f32,
    stroke: f32,
    volume: u8,
    muted: bool,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let box_h = half_h * 0.44;
    let box_w = half_h * 0.40;
    let cone_w = half_h * 0.92;
    unsafe {
        let cone = factory.CreatePathGeometry()?;
        let sink = cone.Open()?;
        sink.BeginFigure(Vector2::new(x, cy - box_h), D2D1_FIGURE_BEGIN_FILLED);
        sink.AddLine(Vector2::new(x + box_w, cy - box_h));
        sink.AddLine(Vector2::new(x + cone_w, cy - half_h * 0.88));
        sink.AddLine(Vector2::new(x + cone_w, cy + half_h * 0.88));
        sink.AddLine(Vector2::new(x + box_w, cy + box_h));
        sink.AddLine(Vector2::new(x, cy + box_h));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        target.FillGeometry(&cone, brush, None);

        if muted || volume == 0 {
            target.DrawLine(
                Vector2::new(x + cone_w + stroke * 0.65, cy - half_h * 0.55),
                Vector2::new(x + cone_w + stroke * 2.55, cy + half_h * 0.55),
                brush,
                stroke * 0.95,
                stroke_style,
            );
            target.DrawLine(
                Vector2::new(x + cone_w + stroke * 2.55, cy - half_h * 0.55),
                Vector2::new(x + cone_w + stroke * 0.65, cy + half_h * 0.55),
                brush,
                stroke * 0.95,
                stroke_style,
            );
            return Ok(());
        }

        let wave_count = if volume <= 35 { 1 } else { 2 };
        for &(offset, scale_y) in &[(1.10_f32, 0.46_f32), (2.25_f32, 0.78_f32)][..wave_count] {
            let wave = factory.CreatePathGeometry()?;
            let wsink = wave.Open()?;
            let wx = x + cone_w + stroke * offset;
            let wy = half_h * scale_y;
            wsink.BeginFigure(Vector2::new(wx, cy - wy), D2D1_FIGURE_BEGIN_HOLLOW);
            wsink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(wx, cy + wy),
                size: D2D_SIZE_F {
                    width: wy * 1.15,
                    height: wy * 1.15,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            wsink.EndFigure(D2D1_FIGURE_END_OPEN);
            wsink.Close()?;
            target.DrawGeometry(&wave, brush, stroke * 0.90, stroke_style);
        }
    }
    Ok(())
}
