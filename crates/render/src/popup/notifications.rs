use otto_core::{
    Appearance, NotificationCardKind, NotificationCardLayout, NotificationCenterHit,
    NotificationCenterLayout, Rect, badge_ink_is_dark, notification_badge, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ELLIPSE};
use windows::core::Result;
use windows_numerics::Vector2;

use super::menu::PopupMenuRenderer;
use crate::appearance::surface::{draw_on, draw_with_recovery, fnv1a_hash};
use crate::appearance::theme::LiquidGlassPalette;
use crate::kit::draw::draw_focus_ring_for;
use crate::kit::icons::{ChevronDir, draw_chevron_glyph, draw_close_x_glyph};
use crate::{
    BLACK, CLEAR, CachedCoverArt, CoverArt, build_squircle_path, decode_image_bytes,
    draw_bell_glyph, rect_to_d2d,
};

struct NcCardTextSlot<'a> {
    card_rect: D2D_RECT_F,
    text_left: f32,
    text_right: f32,
    trailing_w: f32,
    primary_text: &'a str,
    body_text: &'a str,
    stack_extra: usize,
    hovered_card: bool,
    close_hovered: bool,
}

const ICON_CACHE_CAP: usize = 16;

impl PopupMenuRenderer {
    pub fn present_notification_center(
        &mut self,
        hwnd: isize,
        layout: &NotificationCenterLayout,
        active_hit: NotificationCenterHit,
        keyboard_focus: bool,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        draw_with_recovery(
            self,
            |renderer| {
                renderer.draw_notification_center(
                    width,
                    height,
                    layout,
                    active_hit,
                    keyboard_focus,
                    appearance,
                )
            },
            Self::recreate_target,
        )?;
        self.surface
            .present_sized(hwnd, (layout.window.x, layout.window.y), (width, height))
    }

    #[cfg(test)]
    pub(crate) fn render_notification_center_preview(
        &self,
        layout: &NotificationCenterLayout,
        active_hit: NotificationCenterHit,
        keyboard_focus: bool,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        self.draw_notification_center(
            width,
            height,
            layout,
            active_hit,
            keyboard_focus,
            appearance,
        )
    }

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
            if guard.len() >= ICON_CACHE_CAP {
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

    fn draw_notification_center(
        &self,
        width: i32,
        height: i32,
        layout: &NotificationCenterLayout,
        active_hit: NotificationCenterHit,
        keyboard_focus: bool,
        appearance: Appearance,
    ) -> Result<()> {
        let appearance = self.glass.adaptive_appearance(&appearance);
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        draw_on(&self.target, &self.surface, (width, height), || unsafe {
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));
            self.brushes.update_theme(&palette);

            self.draw_shell(width, height, layout.shell_radius, &appearance, &palette)?;
            self.draw_nc_header(layout, active_hit, &palette);

            let viewport = rect_to_d2d(layout.viewport);
            self.target
                .PushAxisAlignedClip(&raw const viewport, D2D1_ANTIALIAS_MODE_ALIASED);
            self.draw_nc_viewport(layout, active_hit, &palette);
            self.target.PopAxisAlignedClip();

            self.draw_nc_scroll_hints(layout);
            if keyboard_focus && let Some(rect) = layout.target_rect(active_hit) {
                self.draw_nc_focus_ring(rect, layout, active_hit, &palette);
            }
            Ok(())
        })
    }

    fn draw_nc_header(
        &self,
        layout: &NotificationCenterLayout,
        active_hit: NotificationCenterHit,
        palette: &LiquidGlassPalette,
    ) {
        let title_rect = rect_to_d2d(layout.title_rect);
        self.draw_clipped_text(
            &self.notif_header_format,
            "Notifications",
            &title_rect,
            &self.brushes.text_primary,
        );
        if let Some(clear_rect) = layout.clear_all_rect {
            self.draw_nc_quiet_action(
                clear_rect,
                layout.pill_radius,
                "Clear All",
                active_hit == NotificationCenterHit::ClearAll,
                palette,
            );
        }
    }

    fn draw_nc_viewport(
        &self,
        layout: &NotificationCenterLayout,
        active_hit: NotificationCenterHit,
        palette: &LiquidGlassPalette,
    ) {
        for header in &layout.group_headers {
            if !layout.is_rect_visible(header.rect) {
                continue;
            }
            let title_right = header.collapse_rect.x - scale(6, self.dpi);
            let title_rect = D2D_RECT_F {
                left: (header.rect.x + scale(4, self.dpi)) as f32,
                top: header.rect.y as f32,
                right: title_right.max(header.rect.x + scale(8, self.dpi)) as f32,
                bottom: header.rect.bottom() as f32,
            };
            self.draw_clipped_text(
                &self.notif_title_format,
                &header.app_name,
                &title_rect,
                &self.brushes.text_primary,
            );
            self.draw_nc_pill(
                header.collapse_rect,
                layout.pill_radius,
                "Show less",
                active_hit == NotificationCenterHit::CollapseGroup(header.group_index),
                palette,
            );
            self.draw_nc_pill(
                header.clear_rect,
                layout.pill_radius,
                "Clear",
                active_hit == NotificationCenterHit::ClearGroup(header.group_index),
                palette,
            );
        }

        for card in &layout.cards {
            if !layout.is_rect_visible(card.rect) {
                continue;
            }
            self.draw_nc_card(card, layout.card_radius, active_hit, palette);
        }

        if let Some(state) = &layout.state_card
            && layout.is_rect_visible(state.rect)
        {
            self.draw_nc_state_card(
                state,
                layout.card_radius,
                layout.pill_radius,
                active_hit,
                palette,
            );
        }
    }

    fn draw_nc_card(
        &self,
        card: &NotificationCardLayout,
        radius: f32,
        active_hit: NotificationCenterHit,
        palette: &LiquidGlassPalette,
    ) {
        let hovered_card = match card.kind {
            NotificationCardKind::CollapsedStack { .. } => matches!(
                active_hit,
                NotificationCenterHit::ExpandGroup(g) | NotificationCenterHit::ClearGroup(g)
                    if g == card.group_index
            ),
            NotificationCardKind::Single | NotificationCardKind::ExpandedItem => {
                active_hit == NotificationCenterHit::OpenCard(card.id)
                    || active_hit == NotificationCenterHit::CloseCard(card.id)
            }
        };
        let card_rect = rect_to_d2d(card.rect);
        self.draw_nc_card_surface(card, card_rect, radius, hovered_card, palette);

        let icon_size = scale(32, self.dpi);
        let icon_rect = Rect {
            x: card.rect.x + scale(10, self.dpi),
            y: card.rect.y + (card.rect.height - icon_size) / 2,
            width: icon_size,
            height: icon_size,
        };
        self.draw_nc_app_icon(
            icon_rect,
            &card.app_name,
            card.icon_bytes.as_deref(),
            palette,
        );
        self.draw_nc_card_text(
            card,
            card_rect,
            icon_rect.right() as f32,
            hovered_card,
            active_hit,
            palette,
        );
    }

    fn draw_nc_card_surface(
        &self,
        card: &NotificationCardLayout,
        card_rect: D2D_RECT_F,
        radius: f32,
        hovered_card: bool,
        palette: &LiquidGlassPalette,
    ) {
        for (idx, lip) in card.lips.iter().enumerate().rev() {
            let lip_rect = rect_to_d2d(*lip);
            let clip_top = if idx == 0 {
                card_rect.bottom
            } else {
                card.lips[idx - 1].bottom() as f32
            };
            let lip_clip = D2D_RECT_F {
                left: lip_rect.left - 2.0,
                top: clip_top,
                right: lip_rect.right + 2.0,
                bottom: lip_rect.bottom + 2.0,
            };
            let lip_radius = (radius - scale(2 * (idx as i32 + 1), self.dpi) as f32).max(5.0);
            let geom = build_squircle_path(&self.factory, lip_rect, lip_radius).ok();
            let lip_alpha = if idx == 0 { 0.72 } else { 0.48 };
            let fill = self.brushes.alpha_brush(palette.card_fill, lip_alpha);
            let rim_alpha = if palette.is_high_contrast {
                0.85
            } else if idx == 0 {
                0.08
            } else {
                0.05
            };
            let rim = self.brushes.alpha_brush(palette.text_primary, rim_alpha);
            unsafe {
                self.target
                    .PushAxisAlignedClip(&raw const lip_clip, D2D1_ANTIALIAS_MODE_ALIASED);
            }
            self.fill_shape(lip_rect, lip_radius, geom.as_ref(), &fill);
            self.draw_shape(lip_rect, lip_radius, geom.as_ref(), &rim, 1.0);
            unsafe {
                self.target.PopAxisAlignedClip();
            }
        }

        let card_geom = build_squircle_path(&self.factory, card_rect, radius).ok();
        self.fill_shape(
            card_rect,
            radius,
            card_geom.as_ref(),
            &self.brushes.card_fill,
        );
        if hovered_card {
            let lift = self.brushes.alpha_brush(palette.text_primary, 0.06);
            self.fill_shape(card_rect, radius, card_geom.as_ref(), &lift);
        }
        if palette.is_high_contrast {
            self.draw_shape(
                card_rect,
                radius,
                card_geom.as_ref(),
                &self.brushes.specular_rim,
                1.0,
            );
        } else {
            let rim_alpha = if hovered_card { 0.13 } else { 0.07 };
            let rim = self.brushes.alpha_brush(palette.text_primary, rim_alpha);
            self.draw_shape(card_rect, radius, card_geom.as_ref(), &rim, 1.0);
        }
    }

    fn draw_nc_stack_count_pill(
        &self,
        left: f32,
        top: f32,
        width: f32,
        height: f32,
        label: &str,
        palette: &LiquidGlassPalette,
    ) {
        let rect = D2D_RECT_F {
            left,
            top,
            right: left + width,
            bottom: top + height,
        };
        let radius = height * 0.5;
        let fill = self.brushes.alpha_brush(palette.text_primary, 0.12);
        let ink = self.brushes.alpha_brush(palette.text_primary, 0.88);
        self.fill_shape(rect, radius, None, &fill);
        crate::draw_dwrite_tabular_text(&self.target, label, &self.notif_count_format, rect, &ink);
    }

    fn draw_nc_card_eyebrow_rows(
        &self,
        card: &NotificationCardLayout,
        slot: &NcCardTextSlot<'_>,
        palette: &LiquidGlassPalette,
    ) {
        let has_body = !slot.body_text.is_empty();
        let row1_right = (slot.text_right - slot.trailing_w - scale(4, self.dpi) as f32)
            .max(slot.text_left + 1.0);
        let eyebrow_h = scale(13, self.dpi) as f32;
        let title_h = scale(16, self.dpi) as f32;
        let row_gap = scale(2, self.dpi) as f32;
        let card_h = slot.card_rect.bottom - slot.card_rect.top;
        let top_pad = if has_body {
            scale(7, self.dpi) as f32
        } else {
            ((card_h - eyebrow_h - title_h - row_gap) * 0.5).max(scale(7, self.dpi) as f32)
        };
        let eyebrow_top = slot.card_rect.top + top_pad;
        let eyebrow_brush = self.brushes.alpha_brush(palette.text_primary, 0.62);
        let age_brush = self.brushes.alpha_brush(palette.text_primary, 0.52);
        let mut app_right = row1_right;
        if slot.stack_extra > 0 {
            let badge = format!("+{}", slot.stack_extra);
            let pill_h = scale(13, self.dpi) as f32;
            let pill_w = (Self::measure(&badge, &self.notif_count_format).unwrap_or(16)
                + scale(8, self.dpi)) as f32;
            let pill_gap = scale(5, self.dpi) as f32;
            let max_app_w = (row1_right - slot.text_left - pill_w - pill_gap).max(0.0);
            let measured_app =
                Self::measure(&card.app_name, &self.notif_meta_format).unwrap_or(60) as f32;
            let app_w = measured_app.min(max_app_w);
            app_right = slot.text_left + app_w;
            let pill_x = (app_right + pill_gap).min((row1_right - pill_w).max(slot.text_left));
            self.draw_nc_stack_count_pill(
                pill_x,
                eyebrow_top + (eyebrow_h - pill_h) * 0.5,
                pill_w,
                pill_h,
                &badge,
                palette,
            );
        }
        let eyebrow_rect = D2D_RECT_F {
            left: slot.text_left,
            top: eyebrow_top,
            right: app_right.max(slot.text_left + 1.0),
            bottom: eyebrow_top + eyebrow_h,
        };
        self.draw_clipped_text(
            &self.notif_meta_format,
            &card.app_name,
            &eyebrow_rect,
            &eyebrow_brush,
        );
        let trailing_rect = D2D_RECT_F {
            left: (slot.text_right - slot.trailing_w).max(slot.text_left),
            top: eyebrow_top,
            right: slot.text_right,
            bottom: eyebrow_top + eyebrow_h,
        };
        if slot.hovered_card {
            self.draw_nc_close_button(card.close_rect, slot.close_hovered, palette);
        } else {
            crate::draw_dwrite_tabular_text(
                &self.target,
                &card.timestamp_label,
                &self.notif_meta_trailing_format,
                trailing_rect,
                &age_brush,
            );
        }
        let title_top = eyebrow_top + eyebrow_h + row_gap;
        let title_rect = D2D_RECT_F {
            left: slot.text_left,
            top: title_top,
            right: slot.text_right,
            bottom: title_top + title_h,
        };
        self.draw_clipped_text(
            &self.notif_title_format,
            slot.primary_text,
            &title_rect,
            &self.brushes.text_primary,
        );
        if has_body {
            let body_top = title_top + title_h + scale(2, self.dpi) as f32;
            let body_bottom =
                (slot.card_rect.bottom - scale(7, self.dpi) as f32).max(body_top + 1.0);
            let body_rect = D2D_RECT_F {
                left: slot.text_left,
                top: body_top,
                right: slot.text_right,
                bottom: body_bottom,
            };
            let body_brush = self.brushes.alpha_brush(palette.text_primary, 0.76);
            self.draw_clipped_text(
                &self.notif_body_format,
                slot.body_text,
                &body_rect,
                &body_brush,
            );
        }
    }

    fn draw_nc_card_text(
        &self,
        card: &NotificationCardLayout,
        card_rect: D2D_RECT_F,
        icon_right: f32,
        hovered_card: bool,
        active_hit: NotificationCenterHit,
        palette: &LiquidGlassPalette,
    ) {
        let pad_x = scale(10, self.dpi) as f32;
        let text_left = icon_right + scale(10, self.dpi) as f32;
        let text_right = (card_rect.right - pad_x).max(text_left + 1.0);
        let trailing_w = scale(32, self.dpi) as f32;
        let primary_text = if card.title.is_empty() {
            card.app_name.as_str()
        } else {
            card.title.as_str()
        };
        let body_text = if card.title.is_empty() {
            ""
        } else {
            card.body.trim()
        };
        let stack_extra = match card.kind {
            NotificationCardKind::CollapsedStack { total_count } => total_count.saturating_sub(1),
            NotificationCardKind::Single | NotificationCardKind::ExpandedItem => 0,
        };
        let close_hovered = match card.kind {
            NotificationCardKind::CollapsedStack { .. } => {
                active_hit == NotificationCenterHit::ClearGroup(card.group_index)
            }
            NotificationCardKind::Single | NotificationCardKind::ExpandedItem => {
                active_hit == NotificationCenterHit::CloseCard(card.id)
            }
        };
        let show_eyebrow = !card.app_name.is_empty()
            && (!card.app_name.eq_ignore_ascii_case(primary_text)
                || stack_extra > 0
                || !body_text.is_empty());
        if show_eyebrow {
            let slot = NcCardTextSlot {
                card_rect,
                text_left,
                text_right,
                trailing_w,
                primary_text,
                body_text,
                stack_extra,
                hovered_card,
                close_hovered,
            };
            self.draw_nc_card_eyebrow_rows(card, &slot, palette);
            return;
        }
        let row1_right = (text_right - trailing_w - scale(4, self.dpi) as f32).max(text_left + 1.0);
        let title_h = scale(17, self.dpi) as f32;
        let title_top = card_rect.top + ((card_rect.bottom - card_rect.top) - title_h) * 0.5;
        let title_rect = D2D_RECT_F {
            left: text_left,
            top: title_top,
            right: row1_right,
            bottom: title_top + title_h,
        };
        self.draw_clipped_text(
            &self.notif_title_format,
            primary_text,
            &title_rect,
            &self.brushes.text_primary,
        );
        let trailing_rect = D2D_RECT_F {
            left: (text_right - trailing_w).max(text_left),
            top: title_top,
            right: text_right,
            bottom: title_top + title_h,
        };
        let age_brush = self.brushes.alpha_brush(palette.text_primary, 0.52);
        if hovered_card {
            self.draw_nc_close_button(card.close_rect, close_hovered, palette);
        } else {
            crate::draw_dwrite_tabular_text(
                &self.target,
                &card.timestamp_label,
                &self.notif_meta_trailing_format,
                trailing_rect,
                &age_brush,
            );
        }
    }

    fn draw_nc_app_icon(
        &self,
        rect: Rect,
        app_name: &str,
        icon_bytes: Option<&[u8]>,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_rect = rect_to_d2d(rect);
        let radius = rect.width as f32 * 0.25;
        if let Some(bytes) = icon_bytes
            && let Some(art) = self.get_or_decode_notif_icon(bytes)
            && crate::kit::draw::fill_squircle_bitmap(
                &self.factory,
                &self.target,
                rect,
                false,
                radius,
                &art.bitmap,
                1.0,
            )
        {
            return;
        }
        let badge = notification_badge(app_name);
        let fill = self.brushes.color_brush(D2D1_COLOR_F {
            r: f32::from(badge.color[0]) / 255.0,
            g: f32::from(badge.color[1]) / 255.0,
            b: f32::from(badge.color[2]) / 255.0,
            a: 1.0,
        });
        let geom = build_squircle_path(&self.factory, d2d_rect, radius).ok();
        self.fill_shape(d2d_rect, radius, geom.as_ref(), &fill);
        let rim = self.brushes.alpha_brush(palette.text_primary, 0.10);
        self.draw_shape(d2d_rect, radius, geom.as_ref(), &rim, 1.0);
        let mut buffer = [0u8; 4];
        let initial = badge.initial.encode_utf8(&mut buffer);
        let ink = if badge_ink_is_dark(badge.color) {
            self.brushes.alpha_brush(BLACK, 0.88)
        } else {
            self.brushes.white.clone()
        };
        self.draw_clipped_text(&self.notif_badge_format, initial, &d2d_rect, &ink);
    }

    fn draw_nc_close_button(&self, area: Rect, hovered: bool, palette: &LiquidGlassPalette) {
        let r = scale(8, self.dpi) as f32;
        let center = Vector2::new(
            area.x as f32 + area.width as f32 * 0.5,
            area.y as f32 + area.height as f32 * 0.5,
        );
        let bg_alpha = if hovered { 0.26 } else { 0.14 };
        let disc = self.brushes.alpha_brush(palette.text_primary, bg_alpha);
        let x_brush = if hovered {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_muted
        };
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: r,
                    radiusY: r,
                },
                &disc,
            );
        }
        draw_close_x_glyph(
            &self.target,
            &self.stroke_style,
            center,
            r * 0.36,
            (scale(1, self.dpi) as f32 * 1.3).max(1.3),
            x_brush,
        );
    }

    fn draw_nc_state_card(
        &self,
        state: &otto_core::NotificationCenterStateCardLayout,
        card_radius: f32,
        pill_radius: f32,
        active_hit: NotificationCenterHit,
        palette: &LiquidGlassPalette,
    ) {
        let card_rect = rect_to_d2d(state.rect);
        let geom = build_squircle_path(&self.factory, card_rect, card_radius).ok();
        self.fill_shape(
            card_rect,
            card_radius,
            geom.as_ref(),
            &self.brushes.card_fill,
        );
        if palette.is_high_contrast {
            self.draw_shape(
                card_rect,
                card_radius,
                geom.as_ref(),
                &self.brushes.specular_rim,
                1.0,
            );
        } else {
            let rim = self.brushes.alpha_brush(palette.text_primary, 0.07);
            self.draw_shape(card_rect, card_radius, geom.as_ref(), &rim, 1.0);
        }

        let bell_x = card_rect.left + (card_rect.right - card_rect.left) * 0.5;
        let bell_y = card_rect.top + scale(24, self.dpi) as f32;
        let bell_r = scale(10, self.dpi) as f32;
        let bell_brush = self.brushes.alpha_brush(palette.text_primary, 0.48);
        let _ = draw_bell_glyph(
            &self.factory,
            &self.target,
            bell_x,
            bell_y,
            bell_r,
            &bell_brush,
        );

        let title_rect = D2D_RECT_F {
            left: card_rect.left + scale(14, self.dpi) as f32,
            top: card_rect.top + scale(40, self.dpi) as f32,
            right: card_rect.right - scale(14, self.dpi) as f32,
            bottom: card_rect.top + scale(58, self.dpi) as f32,
        };
        self.draw_clipped_text(
            &self.notif_pill_format,
            &state.title,
            &title_rect,
            &self.brushes.text_primary,
        );
        let sub_rect = D2D_RECT_F {
            left: card_rect.left + scale(16, self.dpi) as f32,
            top: card_rect.top + scale(58, self.dpi) as f32,
            right: card_rect.right - scale(16, self.dpi) as f32,
            bottom: card_rect.bottom - scale(10, self.dpi) as f32,
        };
        self.draw_clipped_text(
            &self.notif_body_format,
            &state.subtitle,
            &sub_rect,
            &self.brushes.text_muted,
        );

        if let (Some(action_rect), Some(label)) = (state.action_rect, state.action_label.as_deref())
        {
            self.draw_nc_pill(
                action_rect,
                pill_radius,
                label,
                active_hit == state.action_hit,
                palette,
            );
        }
    }

    fn draw_nc_quiet_action(
        &self,
        rect: Rect,
        radius: f32,
        label: &str,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_rect = rect_to_d2d(rect);
        if hovered {
            let geom = build_squircle_path(&self.factory, d2d_rect, radius).ok();
            let fill = self.brushes.alpha_brush(palette.text_primary, 0.13);
            self.fill_shape(d2d_rect, radius, geom.as_ref(), &fill);
        }
        let fg = if hovered {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_muted
        };
        self.draw_clipped_text(&self.notif_pill_format, label, &d2d_rect, fg);
    }

    fn draw_nc_pill(
        &self,
        rect: Rect,
        radius: f32,
        label: &str,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_rect = rect_to_d2d(rect);
        let geom = build_squircle_path(&self.factory, d2d_rect, radius).ok();
        let fill_alpha = if hovered { 0.16 } else { 0.07 };
        let fill = self.brushes.alpha_brush(palette.text_primary, fill_alpha);
        self.fill_shape(d2d_rect, radius, geom.as_ref(), &fill);
        if palette.is_high_contrast {
            let rim = self.brushes.alpha_brush(palette.text_primary, 0.35);
            self.draw_shape(d2d_rect, radius, geom.as_ref(), &rim, 1.0);
        }
        let fg = if hovered {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_muted
        };
        self.draw_clipped_text(&self.notif_pill_format, label, &d2d_rect, fg);
    }

    fn draw_nc_scroll_hints(&self, layout: &NotificationCenterLayout) {
        if !layout.is_scrollable() {
            return;
        }
        let center_x = layout.viewport.x as f32 + layout.viewport.width as f32 * 0.5;
        let reach = scale(5, self.dpi) as f32;
        let stroke = (scale(2, self.dpi) as f32 * 0.75).max(1.4);
        if layout.scroll_offset > 0 {
            draw_chevron_glyph(
                &self.target,
                &self.stroke_style,
                Vector2::new(center_x, layout.viewport.y as f32),
                reach,
                stroke,
                ChevronDir::Up,
                &self.brushes.text_muted,
            );
        }
        if layout.scroll_offset < layout.max_scroll() {
            draw_chevron_glyph(
                &self.target,
                &self.stroke_style,
                Vector2::new(center_x, layout.viewport.bottom() as f32),
                reach,
                stroke,
                ChevronDir::Down,
                &self.brushes.text_muted,
            );
        }
    }

    fn draw_nc_focus_ring(
        &self,
        rect: Rect,
        layout: &NotificationCenterLayout,
        active_hit: NotificationCenterHit,
        _palette: &LiquidGlassPalette,
    ) {
        let is_circle_close = match active_hit {
            NotificationCenterHit::CloseCard(_) => true,
            NotificationCenterHit::ClearGroup(group_index) => layout.cards.iter().any(|c| {
                c.group_index == group_index
                    && matches!(c.kind, NotificationCardKind::CollapsedStack { .. })
            }),
            _ => false,
        };
        if is_circle_close {
            let cx = rect.x as f32 + rect.width as f32 * 0.5;
            let cy = rect.y as f32 + rect.height as f32 * 0.5;
            let r = scale(10, self.dpi) as f32;
            let ring_rect = D2D_RECT_F {
                left: cx - r,
                top: cy - r,
                right: cx + r,
                bottom: cy + r,
            };
            draw_focus_ring_for(
                &self.factory,
                &self.target,
                ring_rect,
                r,
                self.dpi,
                &self.brushes.text_primary,
            );
            return;
        }
        let radius = match active_hit {
            NotificationCenterHit::OpenCard(_) | NotificationCenterHit::ExpandGroup(_) => {
                layout.card_radius
            }
            _ => layout.pill_radius,
        };
        draw_focus_ring_for(
            &self.factory,
            &self.target,
            rect_to_d2d(rect),
            radius,
            self.dpi,
            &self.brushes.text_primary,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::PoisonError;

    use otto_core::{
        NotificationAccess, NotificationSummary, compute_notification_center_layout,
        group_notifications,
    };

    use super::*;
    use crate::release_shared_factories;
    use crate::resource_tests::SERIAL;

    fn sample_bgra_icon() -> std::sync::Arc<[u8]> {
        let mut raw = Vec::with_capacity(12 + 16);
        raw.extend_from_slice(b"BGRA");
        raw.extend_from_slice(&2u32.to_le_bytes());
        raw.extend_from_slice(&2u32.to_le_bytes());
        raw.extend_from_slice(&[
            60, 210, 80, 255, 60, 210, 80, 255, 60, 210, 80, 255, 60, 210, 80, 255,
        ]);
        std::sync::Arc::from(raw.into_boxed_slice())
    }

    #[test]
    fn notification_center_renders_stacks_expanded_groups_and_empty_states() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let menu = PopupMenuRenderer::new(96).expect("popup menu");
        let items = vec![
            NotificationSummary {
                id: 1,
                app_id: String::from("Contoso.Mail"),
                app_name: String::from("Mail"),
                title: String::from("Design review"),
                body: String::from(
                    "Updated Notification Center layout with compact hierarchy and crisp icons",
                ),
                timestamp_label: String::from("2m"),
                icon_bytes: Some(sample_bgra_icon()),
            },
            NotificationSummary {
                id: 2,
                app_id: String::from("Contoso.Mail"),
                app_name: String::from("Mail"),
                title: String::from("Build green"),
                body: String::from("Workspace tests passed"),
                timestamp_label: String::from("5m"),
                icon_bytes: None,
            },
            NotificationSummary {
                id: 3,
                app_id: String::from("Contoso.Chat"),
                app_name: String::from("Chat"),
                title: String::from("You may have new messages"),
                body: String::new(),
                timestamp_label: String::from("now"),
                icon_bytes: None,
            },
        ];
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let appearance = Appearance::glass(otto_core::ResolvedTheme::Dark);

        let collapsed = group_notifications(&items, &BTreeSet::new());
        let collapsed_layout = compute_notification_center_layout(
            NotificationAccess::Allowed,
            &collapsed,
            bounds,
            96,
            1880,
            36,
            0,
        );
        menu.render_notification_center_preview(
            &collapsed_layout,
            NotificationCenterHit::ExpandGroup(0),
            true,
            appearance,
        )
        .expect("collapsed notification center");

        let mut expanded_set = BTreeSet::new();
        expanded_set.insert(String::from("contoso.mail"));
        let expanded = group_notifications(&items, &expanded_set);
        let expanded_layout = compute_notification_center_layout(
            NotificationAccess::Allowed,
            &expanded,
            bounds,
            96,
            1880,
            36,
            0,
        );
        menu.render_notification_center_preview(
            &expanded_layout,
            NotificationCenterHit::CloseCard(2),
            false,
            appearance,
        )
        .expect("expanded notification center");

        let empty_layout = compute_notification_center_layout(
            NotificationAccess::Denied,
            &[],
            bounds,
            96,
            1880,
            36,
            0,
        );
        menu.render_notification_center_preview(
            &empty_layout,
            NotificationCenterHit::AccessAction,
            true,
            appearance,
        )
        .expect("denied notification center");

        drop(menu);
        release_shared_factories();
    }
}
