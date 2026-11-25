use otto_core::{
    CallScene, HitTarget, Layout, NotificationLayout, NotificationPresentation, Rect, ShapeTokens,
    badge_ink_is_dark, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::surface::fnv1a_hash;
use crate::appearance::tokens::{CALL_ACCEPT_GREEN, SYSTEM_RED as CALL_RED};
use crate::kit::draw::{draw_focus_ring_for, inset_rect_f};
use crate::kit::icons::draw_close_x_glyph;
use crate::{
    BLACK, CachedCoverArt, CoverArt, EQUALIZER_GREEN, FOREGROUND, Renderer, brighten,
    build_squircle_path, decode_image_bytes, rect_to_d2d, rect_to_d2d as text_rect, rgb,
    translate_rect,
};
const STACK_LIP_FILL: D2D1_COLOR_F = rgb(14, 14, 18, 1.0);
const NOTIF_ICON_CACHE_CAP: usize = 16;
const RING_PULSE_PERIOD_SECS: f32 = 1.4;
const CALL_WAVE_WEIGHTS: [f32; 5] = [0.55, 0.85, 1.0, 0.80, 0.50];
const CALL_WAVE_STATIC: [f32; 5] = [0.45, 0.72, 0.90, 0.68, 0.42];

fn badge_color(rgb_bytes: [u8; 3]) -> D2D1_COLOR_F {
    rgb(rgb_bytes[0], rgb_bytes[1], rgb_bytes[2], 1.0)
}

fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

impl Renderer {
    fn get_or_decode_notif_icon(&self, bytes: &[u8]) -> Option<CoverArt> {
        let hash = fnv1a_hash(bytes);
        let len = bytes.len();
        if let Ok(guard) = self.cached_notif_icons.try_borrow()
            && let Some(entry) = guard
                .iter()
                .find(|cached| cached.hash == hash && cached.bytes_len == len)
        {
            return Some(entry.art.clone());
        }
        let art = decode_image_bytes(&self.target, bytes).ok()?;
        if let Ok(mut guard) = self.cached_notif_icons.try_borrow_mut() {
            if guard.len() >= NOTIF_ICON_CACHE_CAP {
                guard.remove(0);
            }
            guard.push(CachedCoverArt {
                hash,
                bytes_len: len,
                art: art.clone(),
            });
        }
        Some(art)
    }

    pub(crate) fn draw_notification_stack_lip(
        &self,
        layout: &Layout,
        card: &NotificationLayout,
        offset_x: i32,
    ) {
        let alpha = ((layout.panel_expansion - 0.22) / 0.78).clamp(0.0, 1.0);
        if alpha <= 0.01 {
            return;
        }
        let lip = translate_rect(card.stack_lip, -offset_x);
        let overlap = scale(18, self.dpi);
        let backing = D2D_RECT_F {
            left: lip.x as f32,
            top: (lip.y - overlap) as f32,
            right: lip.right() as f32,
            bottom: lip.bottom() as f32,
        };
        let radius = (layout.notch_corner_radius as f32 - scale(4, self.dpi) as f32).max(8.0);
        let ink = self.notch_ink();
        let fill = if self.high_contrast.get() {
            self.brushes.card_fill.clone()
        } else {
            self.brushes.alpha_brush(STACK_LIP_FILL, 0.84 * alpha)
        };
        let rim = self.brushes.alpha_brush(
            ink,
            if self.high_contrast.get() { 0.85 } else { 0.12 } * alpha,
        );
        if let Ok(geom) = build_squircle_path(&self.factory, backing, radius) {
            unsafe {
                self.target.FillGeometry(&geom, &fill, None);
                self.target
                    .DrawGeometry(&geom, &rim, 1.0, &self.stroke_style);
            }
        }
    }

    pub(crate) fn draw_compact_notification_peek(
        &self,
        layout: &Layout,
        content: &NotificationPresentation,
    ) {
        let alpha = ((layout.compact_expansion - 0.15) / 0.85).clamp(0.0, 1.0);
        if alpha <= 0.01 {
            return;
        }
        let fg = self.notch_ink();
        let badge_size = scale(20, self.dpi);
        let ring = layout.compact_ring;
        let badge = Rect {
            x: ring.x + (ring.width - badge_size) / 2,
            y: ring.y + (ring.height - badge_size) / 2,
            width: badge_size,
            height: badge_size,
        };
        let radius = scale(6, self.dpi) as f32;
        self.draw_notification_badge(badge, content, alpha, false, radius, true);

        let total_extra = usize::from(content.count.saturating_sub(1))
            .saturating_add(usize::from(content.pending_count));
        let title_rect = if total_extra > 0 {
            layout.compact_title
        } else {
            Rect {
                width: (layout.compact_trailing.right() - layout.compact_title.x).max(0),
                ..layout.compact_title
            }
        };
        let ink = self.brushes.alpha_brush(fg, 0.96 * alpha);
        self.draw_text(
            &content.headline,
            &self.text_leading,
            text_rect(title_rect),
            &ink,
        );
        if total_extra > 0 {
            let count_value = total_extra + 1;
            let count_text = if count_value > 99 {
                String::from("99+")
            } else {
                count_value.to_string()
            };
            let trailing = layout.compact_trailing;
            let pill_h = scale(18, self.dpi);
            let pill_w = trailing.width.max(scale(20, self.dpi));
            let pill = Rect {
                x: trailing.right() - pill_w,
                y: trailing.y + (trailing.height - pill_h) / 2,
                width: pill_w,
                height: pill_h,
            };
            let pill_bg = self.brushes.alpha_brush(fg, 0.14 * alpha);
            let pill_ink = self.brushes.alpha_brush(fg, 0.92 * alpha);
            self.draw_rounded_box(pill, pill_h as f32 * 0.5, &pill_bg);
            self.draw_tabular_text(&count_text, &self.text_tab, text_rect(pill), &pill_ink);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_notification_card(
        &self,
        layout: &Layout,
        card: &NotificationLayout,
        content: &NotificationPresentation,
        offset_x: i32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        anim_phase: f32,
    ) -> Result<()> {
        let local = NotificationLayout {
            card: translate_rect(card.card, -offset_x),
            stack_lip: translate_rect(card.stack_lip, -offset_x),
            badge: translate_rect(card.badge, -offset_x),
            meta: translate_rect(card.meta, -offset_x),
            trailing_meta: translate_rect(card.trailing_meta, -offset_x),
            title: translate_rect(card.title, -offset_x),
            subtitle: translate_rect(card.subtitle, -offset_x),
            close: translate_rect(card.close, -offset_x),
            accept: translate_rect(card.accept, -offset_x),
            decline: translate_rect(card.decline, -offset_x),
            ringing: card.ringing,
        };
        let alpha = ((layout.panel_expansion - 0.22) / 0.78).clamp(0.0, 1.0);
        if card.ringing {
            self.draw_call_card(&local, content, alpha, hovered, focused, anim_phase)
        } else {
            self.draw_message_card(layout, &local, content, alpha, hovered, focused);
            Ok(())
        }
    }

    fn draw_message_card(
        &self,
        layout: &Layout,
        card: &NotificationLayout,
        content: &NotificationPresentation,
        alpha: f32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
    ) {
        let fg = self.notch_ink();
        let over_card = matches!(
            hovered,
            Some(HitTarget::NotificationCard | HitTarget::NotificationClose)
        );
        if over_card {
            let lift = self.brushes.alpha_brush(fg, 0.04 * alpha);
            self.draw_rounded_box(card.card, layout.notch_corner_radius as f32, &lift);
        }

        let badge_radius = scale(12, self.dpi) as f32;
        self.draw_notification_badge(card.badge, content, alpha, false, badge_radius, false);

        let coalesce = if self.reduced_motion.get() {
            1.0
        } else {
            self.notif_coalesce.get().clamp(0.0, 1.0)
        };
        let text_alpha = alpha * (0.35 + 0.65 * coalesce);
        let text_shift = ((1.0 - coalesce) * scale(6, self.dpi) as f32).round();

        let meta_brush = self.brushes.alpha_brush(fg, 0.60 * alpha);
        let age_brush = self.brushes.alpha_brush(fg, 0.52 * alpha);
        let primary = self.brushes.alpha_brush(fg, 0.96 * text_alpha);
        let body = self.brushes.alpha_brush(fg, 0.78 * text_alpha);

        let meta = if content.count > 1 {
            format!("{} \u{b7} {}", content.app_name, content.count)
        } else {
            content.app_name.clone()
        };
        self.draw_text(
            &meta,
            &self.text_pomodoro_sub,
            text_rect(card.meta),
            &meta_brush,
        );

        if over_card || focused == Some(HitTarget::NotificationClose) {
            self.draw_close_control(
                card.close,
                hovered == Some(HitTarget::NotificationClose),
                alpha,
            );
        } else {
            let trailing = if content.pending_count > 0 {
                if content.timestamp_label.is_empty() {
                    format!("+{}", content.pending_count)
                } else {
                    format!(
                        "{} \u{b7} +{}",
                        content.timestamp_label, content.pending_count
                    )
                }
            } else {
                content.timestamp_label.clone()
            };
            if !trailing.is_empty() {
                self.draw_tabular_text(
                    &trailing,
                    &self.text_pomodoro_sub_trailing,
                    text_rect(card.trailing_meta),
                    &age_brush,
                );
            }
        }

        let shift_rect = |rect: Rect| D2D_RECT_F {
            left: rect.x as f32,
            top: rect.y as f32 + text_shift,
            right: rect.right() as f32,
            bottom: rect.bottom() as f32 + text_shift,
        };
        self.draw_text(
            &content.headline,
            &self.text_hud_label,
            shift_rect(card.title),
            &primary,
        );
        if !card.subtitle.is_empty() && !content.subtitle.is_empty() {
            self.draw_text(
                &content.subtitle,
                &self.text_notif_body,
                shift_rect(card.subtitle),
                &body,
            );
        }

        if focused == Some(HitTarget::NotificationCard) {
            self.draw_inset_squircle_focus_ring(
                card.card,
                layout.notch_corner_radius as f32,
                alpha,
            );
        } else if focused == Some(HitTarget::NotificationClose) {
            self.draw_circle_focus_ring(card.close, alpha);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_call_card(
        &self,
        card: &NotificationLayout,
        content: &NotificationPresentation,
        alpha: f32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        anim_phase: f32,
    ) -> Result<()> {
        self.draw_avatar_pulse_ring(card.badge, anim_phase, alpha);
        self.draw_notification_badge(
            card.badge,
            content,
            alpha,
            true,
            card.badge.width as f32 * 0.5,
            false,
        );

        let fg = self.notch_ink();
        let meta_brush = self.brushes.alpha_brush(fg, 0.62 * alpha);
        let primary = self.brushes.alpha_brush(fg, 0.98 * alpha);

        let mut meta_box = text_rect(card.meta);
        if content.video {
            let icon_half = scale(5, self.dpi) as f32;
            let icon_x = meta_box.left + icon_half;
            let icon_y = (meta_box.top + meta_box.bottom) * 0.5;
            let cam_brush = self.brushes.alpha_brush(EQUALIZER_GREEN, 0.95 * alpha);
            self.draw_camera_glyph(icon_x, icon_y, icon_half, &cam_brush);
            meta_box.left += icon_half * 2.0 + scale(6, self.dpi) as f32;
        }
        let call_meta = if content.app_name.is_empty() {
            content.subtitle.clone()
        } else if content.video {
            format!("{} \u{b7} Incoming Video", content.app_name)
        } else {
            format!("{} \u{b7} Incoming Call", content.app_name)
        };
        self.draw_text(&call_meta, &self.text_pomodoro_sub, meta_box, &meta_brush);
        self.draw_text(
            &content.headline,
            &self.text_notif_caller,
            text_rect(card.title),
            &primary,
        );

        self.draw_call_button(
            card.decline,
            CALL_RED,
            false,
            false,
            hovered == Some(HitTarget::NotificationDecline),
            alpha,
        )?;
        self.draw_call_button(
            card.accept,
            CALL_ACCEPT_GREEN,
            true,
            content.video,
            hovered == Some(HitTarget::NotificationAccept),
            alpha,
        )?;
        for (target, rect) in [
            (HitTarget::NotificationDecline, card.decline),
            (HitTarget::NotificationAccept, card.accept),
        ] {
            if focused == Some(target) {
                self.draw_circle_focus_ring(rect, alpha);
            }
        }
        Ok(())
    }

    fn draw_avatar_pulse_ring(&self, badge: Rect, anim_phase: f32, alpha: f32) {
        let cx = badge.x as f32 + badge.width as f32 * 0.5;
        let cy = badge.y as f32 + badge.height as f32 * 0.5;
        let base_r = badge.width as f32 * 0.5;
        let (ring_r, ring_alpha) = if self.reduced_motion.get() {
            (base_r + scale(2, self.dpi) as f32, 0.24 * alpha)
        } else {
            let t = (anim_phase / RING_PULSE_PERIOD_SECS).fract();
            let eased = ease_out_cubic(t);
            let expand = scale(7, self.dpi) as f32 * eased;
            (base_r + expand, 0.48 * (1.0 - eased) * alpha)
        };
        if ring_alpha <= 0.01 {
            return;
        }
        let stroke = (self.dpi as f32 / 96.0 * 1.6).max(1.2);
        let brush = self.brushes.alpha_brush(EQUALIZER_GREEN, ring_alpha);
        unsafe {
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: ring_r,
                    radiusY: ring_r,
                },
                &brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    fn draw_notification_badge(
        &self,
        rect: Rect,
        content: &NotificationPresentation,
        alpha: f32,
        circular: bool,
        radius: f32,
        small: bool,
    ) {
        let d2d_rect = rect_to_d2d(rect);
        let icon = content
            .icon_bytes
            .as_deref()
            .and_then(|bytes| self.get_or_decode_notif_icon(bytes));
        if let Some(art) = icon
            && self.fill_badge_bitmap(rect, d2d_rect, circular, radius, &art, alpha)
        {
            self.draw_badge_rim(d2d_rect, circular, radius, alpha);
            return;
        }
        let fill = self
            .brushes
            .alpha_brush(badge_color(content.badge.color), alpha);
        if circular {
            let cx = rect.x as f32 + rect.width as f32 * 0.5;
            let cy = rect.y as f32 + rect.height as f32 * 0.5;
            let r = rect.width as f32 * 0.5;
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: r,
                        radiusY: r,
                    },
                    &fill,
                );
            }
        } else if let Ok(geom) = build_squircle_path(&self.factory, d2d_rect, radius) {
            unsafe {
                self.target.FillGeometry(&geom, &fill, None);
            }
        } else {
            let shape = D2D1_ROUNDED_RECT {
                rect: d2d_rect,
                radiusX: radius,
                radiusY: radius,
            };
            unsafe {
                self.target.FillRoundedRectangle(&shape, &fill);
            }
        }
        self.draw_badge_rim(d2d_rect, circular, radius, alpha);

        let letter = content.badge.initial.to_string();
        let ink = if badge_ink_is_dark(content.badge.color) {
            self.brushes.alpha_brush(BLACK, 0.88 * alpha)
        } else {
            self.brushes.alpha_brush(FOREGROUND, 0.98 * alpha)
        };
        let format = if small {
            &self.text_notif_badge_sm
        } else {
            &self.text_notif_badge
        };
        self.draw_text(&letter, format, d2d_rect, &ink);
    }

    fn fill_badge_bitmap(
        &self,
        rect: Rect,
        _d2d_rect: D2D_RECT_F,
        circular: bool,
        radius: f32,
        art: &CoverArt,
        alpha: f32,
    ) -> bool {
        crate::kit::draw::fill_squircle_bitmap(
            &self.factory,
            &self.target,
            rect,
            circular,
            radius,
            &art.bitmap,
            alpha,
        )
    }

    fn draw_badge_rim(&self, d2d_rect: D2D_RECT_F, circular: bool, radius: f32, alpha: f32) {
        let rim = self.brushes.alpha_brush(self.notch_ink(), 0.14 * alpha);
        unsafe {
            if circular {
                let cx = (d2d_rect.left + d2d_rect.right) * 0.5;
                let cy = (d2d_rect.top + d2d_rect.bottom) * 0.5;
                let r = (d2d_rect.right - d2d_rect.left) * 0.5 - 0.5;
                self.target.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: r.max(1.0),
                        radiusY: r.max(1.0),
                    },
                    &rim,
                    1.0,
                    None,
                );
            } else if let Ok(geom) = build_squircle_path(&self.factory, d2d_rect, radius) {
                self.target.DrawGeometry(&geom, &rim, 1.0, None);
            }
        }
    }

    fn draw_inset_squircle_focus_ring(&self, rect: Rect, outer_radius: f32, alpha: f32) {
        let inset = scale(2, self.dpi) as f32;
        let inner = inset_rect_f(rect_to_d2d(rect), inset);
        let brush = self.brushes.alpha_brush(self.notch_ink(), 0.90 * alpha);
        draw_focus_ring_for(
            &self.factory,
            &self.target,
            inner,
            (outer_radius - inset).max(4.0),
            self.dpi,
            &brush,
        );
    }

    fn draw_circle_focus_ring(&self, rect: Rect, alpha: f32) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let stroke = shape.focus_ring_width as f32;
        let grow = scale(2, self.dpi) as f32 + stroke * 0.5;
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let r = rect.width.min(rect.height) as f32 * 0.5 + grow;
        let brush = self.brushes.alpha_brush(self.notch_ink(), 0.92 * alpha);
        unsafe {
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: r,
                    radiusY: r,
                },
                &brush,
                stroke,
                None,
            );
        }
    }

    fn draw_close_control(&self, rect: Rect, hovered: bool, alpha: f32) {
        let fg = self.notch_ink();
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let visual_r = (scale(20, self.dpi) as f32 * 0.5).min(rect.width as f32 * 0.5);
        let disc = self
            .brushes
            .alpha_brush(fg, if hovered { 0.24 } else { 0.14 } * alpha);
        let glyph = self
            .brushes
            .alpha_brush(fg, if hovered { 0.96 } else { 0.84 } * alpha);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: visual_r,
                    radiusY: visual_r,
                },
                &disc,
            );
        }
        draw_close_x_glyph(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            visual_r * 0.34,
            (self.dpi as f32 / 96.0 * 1.4).max(1.2),
            &glyph,
        );
    }

    fn draw_call_button(
        &self,
        rect: Rect,
        color: D2D1_COLOR_F,
        accept: bool,
        video: bool,
        hovered: bool,
        alpha: f32,
    ) -> Result<()> {
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let radius = rect.width as f32 * 0.5;
        let fill_color = if hovered {
            brighten(color, 0.10)
        } else {
            color
        };
        let fill = self.brushes.alpha_brush(fill_color, alpha);
        let rim = self
            .brushes
            .alpha_brush(FOREGROUND, if hovered { 0.24 } else { 0.14 } * alpha);
        let glyph = self.brushes.alpha_brush(FOREGROUND, 0.98 * alpha);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &fill,
            );
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: (radius - 0.5).max(1.0),
                    radiusY: (radius - 0.5).max(1.0),
                },
                &rim,
                1.0,
                None,
            );
        }
        if accept && video {
            self.draw_camera_glyph(cx, cy, radius * 0.42, &glyph);
            Ok(())
        } else {
            let angle = if accept { -135.0 } else { 0.0 };
            self.draw_handset(cx, cy, radius * 0.44, angle, &glyph)
        }
    }

    fn draw_handset(
        &self,
        cx: f32,
        cy: f32,
        scale_r: f32,
        angle_degrees: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let rad = angle_degrees.to_radians();
        let (sin_a, cos_a) = rad.sin_cos();
        let pt = |x: f32, y: f32| {
            let sx = x * scale_r;
            let sy = y * scale_r;
            Vector2::new(cx + sx * cos_a - sy * sin_a, cy + sx * sin_a + sy * cos_a)
        };
        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(pt(-0.98, 0.04), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(-0.56, -0.46),
                point2: pt(0.56, -0.46),
                point3: pt(0.98, 0.04),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(1.04, 0.14),
                point2: pt(1.00, 0.34),
                point3: pt(0.86, 0.36),
            });
            sink.AddLine(pt(0.56, 0.36));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(0.46, 0.36),
                point2: pt(0.42, 0.28),
                point3: pt(0.40, 0.14),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(0.22, -0.06),
                point2: pt(-0.22, -0.06),
                point3: pt(-0.40, 0.14),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(-0.42, 0.28),
                point2: pt(-0.46, 0.36),
                point3: pt(-0.56, 0.36),
            });
            sink.AddLine(pt(-0.86, 0.36));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(-1.00, 0.34),
                point2: pt(-1.04, 0.14),
                point3: pt(-0.98, 0.04),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&geometry, brush, None);
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
        let fg = self.notch_ink();
        if scene.muted {
            let slash = self.brushes.alpha_brush(fg, alpha);
            self.draw_mic_glyph(
                cx,
                cy,
                half * 0.72,
                (self.dpi as f32 / 96.0 * 1.4).max(1.2),
                true,
                &accent,
                &slash,
            )?;
        } else if scene.video {
            self.draw_camera_glyph(cx, cy, half * 0.70, &accent);
        } else {
            self.draw_handset(cx, cy, half * 0.68, -135.0, &accent)?;
        }
        self.draw_call_waveform(
            layout.compact_peek,
            scene.muted,
            anim_phase,
            accent_color,
            alpha,
        );
        let ink = self.brushes.alpha_brush(fg, 0.96 * alpha);
        self.draw_tabular_text(
            &scene.elapsed,
            &self.text_compact_time,
            text_rect(layout.compact_time),
            &ink,
        );
        Ok(())
    }

    fn draw_camera_glyph(&self, cx: f32, cy: f32, half: f32, brush: &ID2D1SolidColorBrush) {
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - half,
                top: cy - half * 0.58,
                right: cx + half * 0.28,
                bottom: cy + half * 0.58,
            },
            radiusX: half * 0.24,
            radiusY: half * 0.24,
        };
        unsafe {
            self.target.FillRoundedRectangle(&body, brush);
            if let Ok(lens) = self.factory.CreatePathGeometry()
                && let Ok(sink) = lens.Open()
            {
                sink.BeginFigure(Vector2::new(cx + half * 0.40, cy), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(Vector2::new(cx + half, cy - half * 0.46));
                sink.AddLine(Vector2::new(cx + half, cy + half * 0.46));
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
        let unit = self.dpi as f32 / 96.0;
        let bar_w = 2.4 * unit;
        let gap = 2.6 * unit;
        let count = CALL_WAVE_WEIGHTS.len();
        let total = count as f32 * bar_w + (count - 1) as f32 * gap;
        let left = area.x as f32 + (area.width as f32 - total) * 0.5;
        let cy = area.y as f32 + area.height as f32 * 0.5;
        let brush = self
            .brushes
            .alpha_brush(color, if muted { 0.55 } else { 0.95 } * alpha);
        let reduced = self.reduced_motion.get();
        for (index, &weight) in CALL_WAVE_WEIGHTS.iter().enumerate() {
            let height = if muted {
                bar_w
            } else if reduced {
                bar_w + 10.0 * CALL_WAVE_STATIC[index] * unit
            } else {
                let primary = 0.5 + 0.5 * (anim_phase * 7.2 + index as f32 * 1.15).sin();
                let harmonic = 0.5 + 0.5 * (anim_phase * 11.4 + index as f32 * 2.1).cos();
                let level = (0.65 * primary + 0.35 * harmonic) * weight;
                bar_w + (2.0 + 9.5 * level) * unit
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
