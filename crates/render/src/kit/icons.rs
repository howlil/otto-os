use std::f32::consts::PI;

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::theme::BrushPalette;
use crate::rgb;

#[derive(Clone, Copy)]
pub(crate) struct GlyphCtx<'a> {
    pub(crate) factory: &'a ID2D1Factory,
    pub(crate) target: &'a ID2D1RenderTarget,
    pub(crate) stroke_style: &'a ID2D1StrokeStyle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChevronDir {
    Up,
    Down,
    Left,
    Right,
}

pub(crate) fn draw_chevron_glyph(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    reach: f32,
    stroke: f32,
    dir: ChevronDir,
    brush: &ID2D1SolidColorBrush,
) {
    let (a, tip, b) = match dir {
        ChevronDir::Right => (
            Vector2::new(center.X - reach * 0.45, center.Y - reach * 0.85),
            Vector2::new(center.X + reach * 0.45, center.Y),
            Vector2::new(center.X - reach * 0.45, center.Y + reach * 0.85),
        ),
        ChevronDir::Left => (
            Vector2::new(center.X + reach * 0.45, center.Y - reach * 0.85),
            Vector2::new(center.X - reach * 0.45, center.Y),
            Vector2::new(center.X + reach * 0.45, center.Y + reach * 0.85),
        ),
        ChevronDir::Up => (
            Vector2::new(center.X - reach * 0.85, center.Y + reach * 0.45),
            Vector2::new(center.X, center.Y - reach * 0.45),
            Vector2::new(center.X + reach * 0.85, center.Y + reach * 0.45),
        ),
        ChevronDir::Down => (
            Vector2::new(center.X - reach * 0.85, center.Y - reach * 0.45),
            Vector2::new(center.X, center.Y + reach * 0.45),
            Vector2::new(center.X + reach * 0.85, center.Y - reach * 0.45),
        ),
    };
    unsafe {
        target.DrawLine(a, tip, brush, stroke, stroke_style);
        target.DrawLine(tip, b, brush, stroke, stroke_style);
    }
}

pub(crate) fn draw_double_chevron_glyph(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    reach: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let shift = reach * 0.65;
    draw_chevron_glyph(
        target,
        stroke_style,
        Vector2::new(center.X - shift, center.Y),
        reach,
        stroke,
        ChevronDir::Right,
        brush,
    );
    draw_chevron_glyph(
        target,
        stroke_style,
        Vector2::new(center.X + shift, center.Y),
        reach,
        stroke,
        ChevronDir::Right,
        brush,
    );
}

pub(crate) fn draw_close_x_glyph(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    arm: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) {
    unsafe {
        target.DrawLine(
            Vector2::new(center.X - arm, center.Y - arm),
            Vector2::new(center.X + arm, center.Y + arm),
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(
            Vector2::new(center.X + arm, center.Y - arm),
            Vector2::new(center.X - arm, center.Y + arm),
            brush,
            stroke,
            stroke_style,
        );
    }
}

pub(crate) fn draw_search_magnifier_glyph(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    radius: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let lens = Vector2::new(center.X - radius * 0.20, center.Y - radius * 0.20);
    let diag = 0.707_106_77_f32;
    let start = Vector2::new(lens.X + radius * diag, lens.Y + radius * diag);
    let end = Vector2::new(lens.X + radius * 1.82 * diag, lens.Y + radius * 1.82 * diag);
    unsafe {
        target.DrawEllipse(
            &D2D1_ELLIPSE {
                point: lens,
                radiusX: radius,
                radiusY: radius,
            },
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(start, end, brush, stroke * 1.12, stroke_style);
    }
}

pub(crate) fn draw_sun_glyph(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    radius: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let core_r = radius * 0.48;
    let inner = radius * 0.72;
    let outer = radius * 1.08;
    unsafe {
        target.FillEllipse(
            &D2D1_ELLIPSE {
                point: center,
                radiusX: core_r,
                radiusY: core_r,
            },
            brush,
        );
        for index in 0..8 {
            let angle = index as f32 * (PI * 0.25);
            let (sin_a, cos_a) = angle.sin_cos();
            target.DrawLine(
                Vector2::new(center.X + cos_a * inner, center.Y + sin_a * inner),
                Vector2::new(center.X + cos_a * outer, center.Y + sin_a * outer),
                brush,
                stroke,
                stroke_style,
            );
        }
    }
}

pub(crate) fn draw_battery_glyph(
    target: &ID2D1RenderTarget,
    center: Vector2,
    half: f32,
    stroke: f32,
    fraction: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let body_w = half * 1.46;
    let body_h = half * 0.88;
    let left = center.X - body_w * 0.5 - half * 0.08;
    let top = center.Y - body_h * 0.5;
    let body = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left,
            top,
            right: left + body_w,
            bottom: top + body_h,
        },
        radiusX: (half * 0.22).max(1.8),
        radiusY: (half * 0.22).max(1.8),
    };
    let inset = (stroke * 1.45).max(1.8);
    let inner_w = (body_w - inset * 2.0).max(0.0);
    let fill_w = inner_w * fraction.clamp(0.0, 1.0);
    let fill = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: left + inset,
            top: top + inset,
            right: left + inset + fill_w,
            bottom: top + body_h - inset,
        },
        radiusX: (half * 0.09).max(0.8),
        radiusY: (half * 0.09).max(0.8),
    };
    let cap_w = (half * 0.18).max(1.4);
    let cap_h = body_h * 0.42;
    let cap = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: left + body_w + (stroke * 0.7).max(1.0),
            top: center.Y - cap_h * 0.5,
            right: left + body_w + (stroke * 0.7).max(1.0) + cap_w,
            bottom: center.Y + cap_h * 0.5,
        },
        radiusX: cap_w * 0.5,
        radiusY: cap_w * 0.5,
    };
    unsafe {
        target.DrawRoundedRectangle(&body, brush, stroke, None);
        target.FillRoundedRectangle(&cap, brush);
        if fill_w > 0.5 {
            target.FillRoundedRectangle(&fill, brush);
        }
    }
}

pub(crate) fn draw_play_glyph_on(
    ctx: GlyphCtx<'_>,
    center: Vector2,
    radius: f32,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    unsafe {
        let geom = ctx.factory.CreatePathGeometry()?;
        let sink = geom.Open()?;
        sink.BeginFigure(
            Vector2::new(center.X - radius * 0.55, center.Y - radius * 0.85),
            D2D1_FIGURE_BEGIN_FILLED,
        );
        sink.AddLine(Vector2::new(center.X + radius * 0.90, center.Y));
        sink.AddLine(Vector2::new(
            center.X - radius * 0.55,
            center.Y + radius * 0.85,
        ));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        ctx.target.FillGeometry(&geom, brush, None);
    }
    Ok(())
}

pub(crate) fn draw_pause_glyph_on(
    target: &ID2D1RenderTarget,
    center: Vector2,
    radius: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let bar_w = (radius * 0.42).max(2.0);
    let bar_h = radius * 1.55;
    let gap = radius * 0.36;
    for dx in [-gap - bar_w * 0.5, gap + bar_w * 0.5] {
        let pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: center.X + dx - bar_w * 0.5,
                top: center.Y - bar_h * 0.5,
                right: center.X + dx + bar_w * 0.5,
                bottom: center.Y + bar_h * 0.5,
            },
            radiusX: bar_w * 0.45,
            radiusY: bar_w * 0.45,
        };
        unsafe {
            target.FillRoundedRectangle(&pill, brush);
        }
    }
}

pub(crate) fn draw_skip_glyph_on(
    ctx: GlyphCtx<'_>,
    center: Vector2,
    radius: f32,
    next: bool,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let dir = if next { 1.0 } else { -1.0 };
    let stroke = (radius * 0.30).max(1.5);
    unsafe {
        let geom = ctx.factory.CreatePathGeometry()?;
        let sink = geom.Open()?;
        sink.BeginFigure(
            Vector2::new(center.X - radius * 0.75 * dir, center.Y - radius * 0.72),
            D2D1_FIGURE_BEGIN_FILLED,
        );
        sink.AddLine(Vector2::new(center.X + radius * 0.35 * dir, center.Y));
        sink.AddLine(Vector2::new(
            center.X - radius * 0.75 * dir,
            center.Y + radius * 0.72,
        ));
        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
        sink.Close()?;
        ctx.target.FillGeometry(&geom, brush, None);

        let bar_x = center.X + radius * 0.62 * dir;
        ctx.target.DrawLine(
            Vector2::new(bar_x, center.Y - radius * 0.68),
            Vector2::new(bar_x, center.Y + radius * 0.68),
            brush,
            stroke,
            ctx.stroke_style,
        );
    }
    Ok(())
}

pub(crate) fn draw_music_note_glyph_on(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    radius: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let stroke = (radius * crate::GLYPH_STROKE_SCALE).max(1.4);
    let head_width = radius * 0.38;
    let head_height = radius * 0.28;
    let gap = radius * 0.75;
    let x1 = center.X - gap * 0.5;
    let x2 = center.X + gap * 0.5;
    let y_head1 = center.Y + radius * 0.45;
    let y_head2 = center.Y + radius * 0.25;
    let y_top = center.Y - radius * 0.70;
    unsafe {
        target.FillEllipse(
            &D2D1_ELLIPSE {
                point: Vector2::new(x1, y_head1),
                radiusX: head_width,
                radiusY: head_height,
            },
            brush,
        );
        target.FillEllipse(
            &D2D1_ELLIPSE {
                point: Vector2::new(x2, y_head2),
                radiusX: head_width,
                radiusY: head_height,
            },
            brush,
        );
        target.DrawLine(
            Vector2::new(x1 + head_width * 0.6, y_head1),
            Vector2::new(x1 + head_width * 0.6, y_top),
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(
            Vector2::new(x2 + head_width * 0.6, y_head2),
            Vector2::new(x2 + head_width * 0.6, y_top - radius * 0.2),
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(
            Vector2::new(x1 + head_width * 0.6, y_top),
            Vector2::new(x2 + head_width * 0.6, y_top - radius * 0.2),
            brush,
            stroke * 1.6,
            stroke_style,
        );
    }
}

pub(crate) fn draw_return_glyph(
    target: &ID2D1RenderTarget,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    radius: f32,
    stroke: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let right = center.X + radius * 0.78;
    let left = center.X - radius * 0.78;
    let top = center.Y - radius * 0.55;
    let mid = center.Y + radius * 0.30;
    let wing = radius * 0.45;
    unsafe {
        target.DrawLine(
            Vector2::new(right, top),
            Vector2::new(right, mid),
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(
            Vector2::new(right, mid),
            Vector2::new(left, mid),
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(
            Vector2::new(left + wing, mid - wing),
            Vector2::new(left, mid),
            brush,
            stroke,
            stroke_style,
        );
        target.DrawLine(
            Vector2::new(left + wing, mid + wing),
            Vector2::new(left, mid),
            brush,
            stroke,
            stroke_style,
        );
    }
}

pub(crate) fn draw_finder_tile(
    ctx: GlyphCtx<'_>,
    brushes: &BrushPalette,
    tile: &D2D1_ROUNDED_RECT,
    opacity: f32,
    with_rim: bool,
) -> Result<()> {
    let r = tile.rect;
    let w = r.right - r.left;
    let h = r.bottom - r.top;
    let cx = r.left + w * 0.5;
    let alpha = opacity.clamp(0.0, 1.0);

    unsafe {
        let light_blue = &brushes.color_brush(rgb(138, 214, 255, alpha));
        let deep_blue = &brushes.color_brush(rgb(28, 120, 242, alpha));
        let feature = &brushes.color_brush(rgb(20, 44, 84, 0.96 * alpha));

        ctx.target.FillRoundedRectangle(tile, light_blue);

        let left_half = D2D_RECT_F {
            left: r.left,
            top: r.top,
            right: cx,
            bottom: r.bottom,
        };
        let left_round = D2D1_ROUNDED_RECT {
            rect: left_half,
            radiusX: tile.radiusX,
            radiusY: tile.radiusY,
        };
        ctx.target.FillRoundedRectangle(&left_round, deep_blue);
        let seam_fill = D2D_RECT_F {
            left: cx - tile.radiusX * 0.6,
            top: r.top,
            right: cx + w * 0.02,
            bottom: r.bottom,
        };
        ctx.target.FillRectangle(&seam_fill, deep_blue);

        let stroke = (w * 0.045).max(1.5);
        let nose = ctx.factory.CreatePathGeometry()?;
        let sink = nose.Open()?;
        sink.BeginFigure(
            Vector2::new(cx + w * 0.03, r.top + h * 0.06),
            D2D1_FIGURE_BEGIN_HOLLOW,
        );
        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: Vector2::new(cx - w * 0.04, r.top + h * 0.30),
            point2: Vector2::new(cx - w * 0.06, r.top + h * 0.44),
            point3: Vector2::new(cx + w * 0.06, r.top + h * 0.54),
        });
        sink.AddLine(Vector2::new(cx - w * 0.01, r.bottom - h * 0.06));
        sink.EndFigure(D2D1_FIGURE_END_OPEN);
        sink.Close()?;
        ctx.target
            .DrawGeometry(&nose, feature, stroke, ctx.stroke_style);

        let eye_w = (w * 0.035).max(1.3);
        let eye_h = h * 0.11;
        let eye_y = r.top + h * 0.29;
        for ex in [r.left + w * 0.29, r.left + w * 0.71] {
            let eye = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: ex - eye_w,
                    top: eye_y,
                    right: ex + eye_w,
                    bottom: eye_y + eye_h,
                },
                radiusX: eye_w,
                radiusY: eye_w,
            };
            ctx.target.FillRoundedRectangle(&eye, feature);
        }

        let smile = ctx.factory.CreatePathGeometry()?;
        let s_sink = smile.Open()?;
        s_sink.BeginFigure(
            Vector2::new(r.left + w * 0.24, r.top + h * 0.65),
            D2D1_FIGURE_BEGIN_HOLLOW,
        );
        s_sink.AddBezier(&D2D1_BEZIER_SEGMENT {
            point1: Vector2::new(r.left + w * 0.38, r.top + h * 0.82),
            point2: Vector2::new(r.left + w * 0.62, r.top + h * 0.82),
            point3: Vector2::new(r.left + w * 0.76, r.top + h * 0.65),
        });
        s_sink.EndFigure(D2D1_FIGURE_END_OPEN);
        s_sink.Close()?;
        ctx.target
            .DrawGeometry(&smile, feature, stroke, ctx.stroke_style);

        if with_rim {
            let rim = &brushes.color_brush(rgb(255, 255, 255, 0.22 * alpha));
            ctx.target.DrawRoundedRectangle(tile, rim, 1.0, None);
        }
    }
    Ok(())
}
