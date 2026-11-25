use std::f32::consts::PI;

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{D2D1_ROUNDED_RECT, ID2D1Brush, ID2D1SolidColorBrush};
use windows::core::Result;
use windows_numerics::Vector2;

use super::ControlCenterRenderer;
use crate::kit::draw::{draw_line_on, fill_ellipse_on, stroke_ellipse_on};
use crate::kit::icons::{
    GlyphCtx, draw_battery_glyph, draw_music_note_glyph_on, draw_pause_glyph_on,
    draw_play_glyph_on, draw_search_magnifier_glyph, draw_skip_glyph_on, draw_sun_glyph,
};

impl ControlCenterRenderer {
    fn glyph_ctx(&self) -> GlyphCtx<'_> {
        GlyphCtx {
            factory: &self.factory,
            target: &self.target,
            stroke_style: &self.stroke_style,
        }
    }

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
        let stroke = (half * 0.16).max(1.25);
        draw_battery_glyph(
            &self.target,
            Vector2::new(cx, cy),
            half,
            stroke,
            fraction,
            brush,
        );
    }

    pub(super) fn draw_sun_glyph(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (radius * 0.24).max(1.2);
        draw_sun_glyph(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            radius,
            stroke,
            brush,
        );
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
        let b: &ID2D1Brush = brush.into();
        for offset in [-spread, 0.0, spread] {
            fill_ellipse_on(&self.target, Vector2::new(cx + offset, cy), dot_r, dot_r, b);
        }
    }

    pub(super) fn draw_note_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        draw_music_note_glyph_on(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            half,
            brush,
        );
    }

    pub(super) fn draw_play_pause_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        playing: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        if playing {
            draw_pause_glyph_on(&self.target, Vector2::new(cx, cy), half, brush);
            return Ok(());
        }
        draw_play_glyph_on(self.glyph_ctx(), Vector2::new(cx, cy), half, brush)
    }

    pub(super) fn draw_skip_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        next: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        draw_skip_glyph_on(self.glyph_ctx(), Vector2::new(cx, cy), half, next, brush)
    }

    pub(super) fn draw_plus_glyph(&self, cx: f32, cy: f32, arm: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (arm * 0.26).max(1.5);
        let b: &ID2D1Brush = brush.into();
        draw_line_on(
            &self.target,
            Vector2::new(cx - arm, cy),
            Vector2::new(cx + arm, cy),
            b,
            stroke,
            Some(&self.stroke_style),
        );
        draw_line_on(
            &self.target,
            Vector2::new(cx, cy - arm),
            Vector2::new(cx, cy + arm),
            b,
            stroke,
            Some(&self.stroke_style),
        );
    }

    pub(super) fn draw_minus_glyph(
        &self,
        cx: f32,
        cy: f32,
        arm: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (arm * 0.30).max(1.6);
        let b: &ID2D1Brush = brush.into();
        draw_line_on(
            &self.target,
            Vector2::new(cx - arm, cy),
            Vector2::new(cx + arm, cy),
            b,
            stroke,
            Some(&self.stroke_style),
        );
    }

    pub(super) fn draw_resize_corner_glyph(
        &self,
        right: f32,
        bottom: f32,
        span: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let b: &ID2D1Brush = brush.into();
        let stroke = (span * 0.18).max(2.0);
        let r = span * 0.62;
        let cx = right - span * 0.85;
        let cy = bottom - span * 0.85;
        let steps = 6;
        let mut prev = Vector2::new(cx + r, cy);
        for step in 1..=steps {
            let angle = (step as f32 / steps as f32) * (PI * 0.5);
            let next = Vector2::new(cx + r * angle.cos(), cy + r * angle.sin());
            draw_line_on(
                &self.target,
                prev,
                next,
                b,
                stroke,
                Some(&self.stroke_style),
            );
            prev = next;
        }
    }

    pub(super) fn draw_lock_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.18).max(1.4);
        let b: &ID2D1Brush = brush.into();
        let body = D2D_RECT_F {
            left: cx - half * 0.65,
            top: cy - half * 0.05,
            right: cx + half * 0.65,
            bottom: cy + half * 0.82,
        };
        crate::kit::draw::fill_shape(&self.target, body, half * 0.22, None, b);
        stroke_ellipse_on(
            &self.target,
            Vector2::new(cx, cy - half * 0.25),
            half * 0.38,
            half * 0.44,
            b,
            stroke,
        );
    }

    pub(super) fn draw_screenshot_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.18).max(1.4);
        let b: &ID2D1Brush = brush.into();
        let arm = half * 0.34;
        let corners = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];
        for (sx, sy) in corners {
            let x0 = cx + sx * half * 0.72;
            let y0 = cy + sy * half * 0.72;
            draw_line_on(
                &self.target,
                Vector2::new(x0, y0),
                Vector2::new(x0 - sx * arm, y0),
                b,
                stroke,
                Some(&self.stroke_style),
            );
            draw_line_on(
                &self.target,
                Vector2::new(x0, y0),
                Vector2::new(x0, y0 - sy * arm),
                b,
                stroke,
                Some(&self.stroke_style),
            );
        }
        fill_ellipse_on(
            &self.target,
            Vector2::new(cx, cy),
            half * 0.22,
            half * 0.22,
            b,
        );
    }

    pub(super) fn draw_theme_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.18).max(1.4);
        let b: &ID2D1Brush = brush.into();
        let r = half * 0.78;
        stroke_ellipse_on(&self.target, Vector2::new(cx, cy), r, r, b, stroke);
        fill_ellipse_on(
            &self.target,
            Vector2::new(cx - r * 0.22, cy),
            r * 0.46,
            r * 0.72,
            b,
        );
    }

    pub(super) fn draw_task_view_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.16).max(1.3);
        let b: &ID2D1Brush = brush.into();
        let gap = half * 0.14;
        let box_s = half * 0.58;
        for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let x = cx + dx * (gap + box_s * 0.5);
            let y = cy + dy * (gap + box_s * 0.5);
            let rect = D2D_RECT_F {
                left: x - box_s * 0.5,
                top: y - box_s * 0.5,
                right: x + box_s * 0.5,
                bottom: y + box_s * 0.5,
            };
            crate::kit::draw::stroke_shape(&self.target, rect, half * 0.16, None, b, stroke);
        }
    }

    pub(super) fn draw_finder_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.18).max(1.35);
        let b: &ID2D1Brush = brush.into();
        let rect = D2D_RECT_F {
            left: cx - half * 0.78,
            top: cy - half * 0.56,
            right: cx + half * 0.78,
            bottom: cy + half * 0.62,
        };
        crate::kit::draw::stroke_shape(&self.target, rect, half * 0.20, None, b, stroke);
        draw_line_on(
            &self.target,
            Vector2::new(rect.left + stroke, cy - half * 0.15),
            Vector2::new(rect.right - stroke, cy - half * 0.15),
            b,
            stroke,
            Some(&self.stroke_style),
        );
    }

    pub(super) fn draw_spotlight_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.20).max(1.4);
        draw_search_magnifier_glyph(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            half * 0.48,
            stroke,
            brush,
        );
    }

    pub(super) fn draw_settings_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.18).max(1.35);
        let b: &ID2D1Brush = brush.into();
        let r = half * 0.42;
        stroke_ellipse_on(&self.target, Vector2::new(cx, cy), r, r, b, stroke);
        let inner = half * 0.56;
        let outer = half * 0.84;
        for idx in 0..6 {
            let angle = idx as f32 * (PI / 3.0);
            let (s, c) = angle.sin_cos();
            draw_line_on(
                &self.target,
                Vector2::new(cx + c * inner, cy + s * inner),
                Vector2::new(cx + c * outer, cy + s * outer),
                b,
                stroke,
                Some(&self.stroke_style),
            );
        }
    }

    pub(super) fn draw_night_light_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let _ = D2D1_ROUNDED_RECT::default();
        self.draw_sun_glyph(cx, cy, half * 0.72, brush);
    }
}
