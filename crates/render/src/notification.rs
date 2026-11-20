use otto_core::{CallScene, HitTarget, Layout, NotificationLayout, NotificationPresentation, Rect};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
    D2D1_SWEEP_DIRECTION_CLOCKWISE,
};
use windows::core::Result;
use windows_numerics::Vector2;

use super::{EQUALIZER_GREEN, FOREGROUND, Renderer, rect_to_d2d, rgb, translate_rect};

const CALL_RED: D2D1_COLOR_F = rgb(255, 69, 58, 1.0);
const HANDSET_ARC: f32 = 0.95;

fn text_rect(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: rect.right() as f32,
        bottom: rect.bottom() as f32,
    }
}

fn badge_color(rgb_bytes: [u8; 3]) -> D2D1_COLOR_F {
    rgb(rgb_bytes[0], rgb_bytes[1], rgb_bytes[2], 1.0)
}

impl Renderer {
    pub(crate) fn draw_notification_card(
        &self,
        layout: &Layout,
        card: &NotificationLayout,
        content: &NotificationPresentation,
        offset_x: i32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
    ) -> Result<()> {
        let local = NotificationLayout {
            card: translate_rect(card.card, -offset_x),
            badge: translate_rect(card.badge, -offset_x),
            meta: translate_rect(card.meta, -offset_x),
            title: translate_rect(card.title, -offset_x),
            subtitle: translate_rect(card.subtitle, -offset_x),
            close: translate_rect(card.close, -offset_x),
            accept: translate_rect(card.accept, -offset_x),
            decline: translate_rect(card.decline, -offset_x),
            ringing: card.ringing,
        };
        let alpha = ((layout.panel_expansion - 0.22) / 0.5).clamp(0.0, 1.0);
        if card.ringing {
            self.draw_call_card(&local, content, alpha, hovered, focused)
        } else {
            self.draw_message_card(&local, content, alpha, hovered, focused);
            Ok(())
        }
    }

    fn draw_message_card(
        &self,
        card: &NotificationLayout,
        content: &NotificationPresentation,
        alpha: f32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
    ) {
        self.draw_badge(card.badge, content, alpha, false);
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.60 * alpha);
        let primary = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let body = self.brushes.alpha_brush(FOREGROUND, 0.78 * alpha);
        let meta = if content.count > 1 {
            format!("{} \u{b7} {}", content.app_name, content.count)
        } else {
            content.app_name.clone()
        };
        self.draw_text(&meta, &self.text_caption, text_rect(card.meta), &muted);
        self.draw_text(
            &content.headline,
            &self.text_hud_label,
            text_rect(card.title),
            &primary,
        );
        self.draw_text(
            &content.subtitle,
            &self.text_wrap,
            text_rect(card.subtitle),
            &body,
        );
        let over_card = matches!(
            hovered,
            Some(HitTarget::NotificationCard | HitTarget::NotificationClose)
        );
        if over_card || focused == Some(HitTarget::NotificationClose) {
            self.draw_close_control(
                card.close,
                hovered == Some(HitTarget::NotificationClose),
                alpha,
            );
        }
        if focused == Some(HitTarget::NotificationCard) {
            self.draw_control_focus_ring(card.card, &self.brushes.text_primary);
        }
    }

    fn draw_call_card(
        &self,
        card: &NotificationLayout,
        content: &NotificationPresentation,
        alpha: f32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
    ) -> Result<()> {
        self.draw_badge(card.badge, content, alpha, true);
        let primary = self.brushes.alpha_brush(FOREGROUND, 0.98 * alpha);
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.64 * alpha);
        self.draw_text(
            &content.headline,
            &self.text_alert_title,
            text_rect(card.title),
            &primary,
        );
        self.draw_text(
            &content.subtitle,
            &self.text_pomodoro_sub,
            text_rect(card.subtitle),
            &muted,
        );
        self.draw_call_button(
            card.decline,
            CALL_RED,
            140.0,
            hovered == Some(HitTarget::NotificationDecline),
            alpha,
        )?;
        self.draw_call_button(
            card.accept,
            EQUALIZER_GREEN,
            -135.0,
            hovered == Some(HitTarget::NotificationAccept),
            alpha,
        )?;
        for (target, rect) in [
            (HitTarget::NotificationDecline, card.decline),
            (HitTarget::NotificationAccept, card.accept),
        ] {
            if focused == Some(target) {
                self.draw_control_focus_ring(rect, &self.brushes.text_primary);
            }
        }
        Ok(())
    }

    fn draw_badge(
        &self,
        rect: Rect,
        content: &NotificationPresentation,
        alpha: f32,
        circular: bool,
    ) {
        let fill = self
            .brushes
            .alpha_brush(badge_color(content.badge.color), alpha);
        let radius = if circular {
            rect.width as f32 * 0.5
        } else {
            rect.width as f32 * 0.28
        };
        let shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        let letter = content.badge.initial.to_string();
        let ink = self.brushes.alpha_brush(FOREGROUND, alpha);
        unsafe {
            self.target.FillRoundedRectangle(&shape, &fill);
        }
        self.draw_text(
            &letter,
            &self.text_compact_center,
            D2D_RECT_F {
                top: rect.y as f32 + rect.height as f32 * 0.5 - 9.0 * self.dpi as f32 / 96.0,
                bottom: rect.bottom() as f32,
                ..text_rect(rect)
            },
            &ink,
        );
    }

    fn draw_close_control(&self, rect: Rect, hovered: bool, alpha: f32) {
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let disc = self
            .brushes
            .alpha_brush(FOREGROUND, if hovered { 0.22 } else { 0.12 } * alpha);
        let glyph = self.brushes.alpha_brush(FOREGROUND, 0.86 * alpha);
        let arm = rect.width as f32 * 0.17;
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: rect.width as f32 * 0.5,
                    radiusY: rect.width as f32 * 0.5,
                },
                &disc,
            );
            for sign in [-1.0_f32, 1.0] {
                self.target.DrawLine(
                    Vector2::new(cx - arm, cy - arm * sign),
                    Vector2::new(cx + arm, cy + arm * sign),
                    &glyph,
                    (self.dpi as f32 / 96.0 * 1.4).max(1.2),
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_call_button(
        &self,
        rect: Rect,
        color: D2D1_COLOR_F,
        bulge_degrees: f32,
        hovered: bool,
        alpha: f32,
    ) -> Result<()> {
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let radius = rect.width as f32 * 0.5;
        let fill = self
            .brushes
            .alpha_brush(color, if hovered { 1.0 } else { 0.92 } * alpha);
        let glyph = self.brushes.alpha_brush(FOREGROUND, alpha);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &fill,
            );
        }
        self.draw_handset(cx, cy, radius * 0.40, bulge_degrees, &glyph)
    }

    fn draw_handset(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        bulge_degrees: f32,
        brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    ) -> Result<()> {
        let bulge = bulge_degrees.to_radians();
        let half_span = HANDSET_ARC * 0.5;
        let center_x = cx - radius * bulge.cos() * 0.5;
        let center_y = cy - radius * bulge.sin() * 0.5;
        let point = |angle: f32| {
            Vector2::new(
                center_x + radius * angle.cos(),
                center_y + radius * angle.sin(),
            )
        };
        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(point(bulge - half_span), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: point(bulge + half_span),
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target.DrawGeometry(
                &geometry,
                brush,
                (self.dpi as f32 / 96.0 * 4.0).max(3.0),
                &self.stroke_style,
            );
        }
        Ok(())
    }

    pub(crate) fn draw_compact_call(
        &self,
        layout: &Layout,
        scene: &CallScene,
        anim_phase: f32,
        alpha: f32,
    ) -> Result<()> {
        let ring = layout.compact_ring;
        let cx = ring.x as f32 + ring.width as f32 * 0.5;
        let cy = ring.y as f32 + ring.height as f32 * 0.5;
        let half = ring.width as f32 * 0.5;
        let accent_color = if scene.muted {
            CALL_RED
        } else {
            EQUALIZER_GREEN
        };
        let accent = self.brushes.alpha_brush(accent_color, alpha);
        if scene.muted {
            let slash = self.brushes.alpha_brush(FOREGROUND, alpha);
            self.draw_mic_glyph(
                cx,
                cy,
                half * 0.8,
                (self.dpi as f32 / 96.0 * 1.4).max(1.2),
                true,
                &accent,
                &slash,
            )?;
        } else if scene.video {
            self.draw_camera_glyph(cx, cy, half * 0.78, &accent);
        } else {
            self.draw_handset(cx, cy, half * 0.62, -135.0, &accent)?;
        }
        self.draw_call_waveform(
            layout.compact_peek,
            scene.muted,
            anim_phase,
            accent_color,
            alpha,
        );
        let ink = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        self.draw_tabular_text(
            &scene.elapsed,
            &self.text_compact_time,
            text_rect(layout.compact_time),
            &ink,
        );
        Ok(())
    }

    fn draw_camera_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush,
    ) {
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - half,
                top: cy - half * 0.58,
                right: cx + half * 0.25,
                bottom: cy + half * 0.58,
            },
            radiusX: half * 0.22,
            radiusY: half * 0.22,
        };
        unsafe {
            self.target.FillRoundedRectangle(&body, brush);
            if let Ok(lens) = self.factory.CreatePathGeometry()
                && let Ok(sink) = lens.Open()
            {
                sink.BeginFigure(Vector2::new(cx + half * 0.42, cy), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(Vector2::new(cx + half, cy - half * 0.5));
                sink.AddLine(Vector2::new(cx + half, cy + half * 0.5));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                let _ = sink.Close();
                self.target.FillGeometry(&lens, brush, None);
            }
        }
    }

    fn draw_call_waveform(
        &self,
        area: Rect,
        muted: bool,
        anim_phase: f32,
        color: D2D1_COLOR_F,
        alpha: f32,
    ) {
        const BARS: i32 = 5;
        let unit = self.dpi as f32 / 96.0;
        let bar_w = 2.6 * unit;
        let gap = 2.8 * unit;
        let total = BARS as f32 * bar_w + (BARS - 1) as f32 * gap;
        let left = area.x as f32 + (area.width as f32 - total) * 0.5;
        let cy = area.y as f32 + area.height as f32 * 0.5;
        let brush = self
            .brushes
            .alpha_brush(color, if muted { 0.55 } else { 0.95 } * alpha);
        for index in 0..BARS {
            let wave = 0.5 + 0.5 * (anim_phase * 9.0 + index as f32 * 1.3).sin();
            let height = if muted {
                bar_w
            } else {
                bar_w + (3.0 + 8.0 * wave) * unit
            };
            let x = left + index as f32 * (bar_w + gap);
            let shape = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x,
                    top: cy - height * 0.5,
                    right: x + bar_w,
                    bottom: cy + height * 0.5,
                },
                radiusX: bar_w * 0.5,
                radiusY: bar_w * 0.5,
            };
            unsafe {
                self.target.FillRoundedRectangle(&shape, &brush);
            }
        }
    }
}
