use crate::appearance::surface::fnv1a_hash;
use crate::appearance::tokens::{SYSTEM_RED, SYSTEM_RED_LIGHT, pressed_overlay, with_alpha};
use crate::kit::draw::{fill_squircle_bitmap, fill_squircle_or_rounded_f};
use crate::{CoverArt, LiquidGlassPalette, decode_image_bytes, draw_speaker_glyph, scale};
use otto_core::{ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot, Rect};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, ID2D1SolidColorBrush};
use windows::core::Result;
use windows_numerics::Vector2;

use super::ControlCenterPaintState;
use super::ControlCenterRenderer;
use super::ControlVisualState;
use crate::CachedCoverArt;

impl ControlCenterRenderer {
    pub(super) fn draw_connectivity_row_hover(
        &self,
        row: Rect,
        row_radius: f32,
        target: ControlCenterHitTarget,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        if state.hovered == Some(target) || state.pressed == Some(target) {
            self.draw_hover_pill(row, row_radius, state.pressed == Some(target), palette);
        }
    }

    pub(super) fn draw_circle_badge(
        &self,
        circle: Rect,
        visual: ControlVisualState,
        accent: D2D1_COLOR_F,
        palette: &LiquidGlassPalette,
    ) -> (f32, f32, f32, &ID2D1SolidColorBrush) {
        let cx = circle.x as f32 + circle.width as f32 * 0.5;
        let cy = circle.y as f32 + circle.height as f32 * 0.5;
        let rad = circle.width as f32 * 0.5;
        let bg = self.brushes.color_brush(if visual.active {
            accent
        } else {
            palette.toggle_inactive
        });
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: rad,
                    radiusY: rad,
                },
                &bg,
            );
            if visual.enabled && visual.pointer.is_hovered() {
                let overlay = if visual.pointer.is_pressed() {
                    pressed_overlay(palette.hover_pill, palette.is_high_contrast)
                } else {
                    palette.hover_pill
                };
                let brush = self.brushes.color_brush(overlay);
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: rad,
                        radiusY: rad,
                    },
                    &brush,
                );
            }
        }
        let glyph_brush = if visual.active {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        (cx, cy, rad, glyph_brush)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_two_line_label(
        &self,
        left: f32,
        right: f32,
        center_y: f32,
        title: &str,
        subtitle: &str,
        title_brush: &ID2D1SolidColorBrush,
        sub_brush: &ID2D1SolidColorBrush,
        tabular_title: bool,
    ) {
        let title_h = scale(16, self.dpi) as f32;
        let sub_h = scale(14, self.dpi) as f32;
        let line_gap = scale(2, self.dpi) as f32;
        let top = center_y - (title_h + line_gap + sub_h) * 0.5;
        let clamped_right = right.max(left + 1.0);
        let title_rect = D2D_RECT_F {
            left,
            top,
            right: clamped_right,
            bottom: top + title_h,
        };
        let sub_rect = D2D_RECT_F {
            left,
            top: top + title_h + line_gap,
            right: clamped_right,
            bottom: top + title_h + line_gap + sub_h,
        };
        if tabular_title {
            self.draw_tabular_text(title, &self.title_format, title_rect, title_brush);
        } else {
            self.draw_text(title, &self.title_format, title_rect, title_brush);
        }
        self.draw_text(subtitle, &self.subtitle_format, sub_rect, sub_brush);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_connectivity_row(
        &self,
        circle: Rect,
        row: Rect,
        visual: ControlVisualState,
        accent: D2D1_COLOR_F,
        title: &str,
        subtitle: &str,
        palette: &LiquidGlassPalette,
        glyph: impl FnOnce(f32, f32, f32, &ID2D1SolidColorBrush) -> Result<()>,
    ) -> Result<()> {
        let (cx, cy, rad, glyph_brush) = self.draw_circle_badge(circle, visual, accent, palette);
        glyph(cx, cy, rad * 0.58, glyph_brush)?;

        let text_left = circle.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = row.right() as f32 - scale(18, self.dpi) as f32;
        let mid = row.y as f32 + row.height as f32 * 0.5;
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            title,
            subtitle,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            false,
        );

        self.draw_chevron(
            row.right() as f32 - scale(10, self.dpi) as f32,
            mid,
            scale(4, self.dpi) as f32,
            true,
            &self.brushes.text_muted,
        );
        Ok(())
    }

    pub(super) fn draw_slider_header(
        &self,
        card: Rect,
        header: Rect,
        title: &str,
        value_label: &str,
        show_chevron: bool,
    ) {
        let pad = scale(12, self.dpi) as f32;
        let left_w = scale(84, self.dpi) as f32;
        let right_edge = if show_chevron {
            card.right() as f32 - scale(24, self.dpi) as f32
        } else {
            card.right() as f32 - pad
        };
        let left_r = D2D_RECT_F {
            left: card.x as f32 + pad,
            top: header.y as f32,
            right: (card.x as f32 + pad + left_w).min(right_edge),
            bottom: header.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: left_r.right + scale(4, self.dpi) as f32,
            top: header.y as f32,
            right: right_edge,
            bottom: header.bottom() as f32,
        };
        self.draw_text(
            title,
            &self.title_format,
            left_r,
            &self.brushes.text_primary,
        );
        self.draw_tabular_text(
            value_label,
            &self.value_format,
            right_r,
            &self.brushes.text_muted,
        );
        if show_chevron {
            self.draw_chevron(
                card.right() as f32 - scale(14, self.dpi) as f32,
                header.y as f32 + header.height as f32 * 0.5,
                scale(4, self.dpi) as f32,
                true,
                &self.brushes.text_muted,
            );
        }
    }

    pub(super) fn draw_sound_controls(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let muted = snapshot.status.muted;
        let effective = if muted { 0 } else { snapshot.status.volume };
        self.draw_capsule_slider(
            layout.sound_slider,
            Some(effective),
            state.hovered == Some(ControlCenterHitTarget::SoundSlider),
            state.pressed == Some(ControlCenterHitTarget::SoundSlider),
            false,
            palette,
        );

        let mute = layout.sound_mute;
        let mute_r = mute.height as f32 * 0.5;
        let cx = mute.x as f32 + mute.width as f32 * 0.5;
        let cy = mute.y as f32 + mute.height as f32 * 0.5;
        if muted {
            let mute_bg = self.brushes.color_brush(if palette.is_high_contrast {
                palette.toggle_active
            } else {
                with_alpha(SYSTEM_RED, 0.24)
            });
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: mute_r,
                        radiusY: mute_r,
                    },
                    &mute_bg,
                );
            }
        }
        let mute_hovered = state.hovered == Some(ControlCenterHitTarget::SoundMuteToggle);
        let mute_pressed = state.pressed == Some(ControlCenterHitTarget::SoundMuteToggle);
        if mute_hovered || mute_pressed {
            if effective > 0 && !palette.is_high_contrast {
                let cap_overlay = self.brushes.color_brush(D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: if mute_pressed { 0.18 } else { 0.10 },
                });
                unsafe {
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(cx, cy),
                            radiusX: mute_r,
                            radiusY: mute_r,
                        },
                        &cap_overlay,
                    );
                }
            } else {
                self.draw_interaction_overlay(mute, mute_r, mute_hovered, mute_pressed, palette);
            }
        }
        let glyph_dark = self.slider_cap_dark_brush();
        let muted_red = self.brushes.color_brush(SYSTEM_RED_LIGHT);
        let icon_brush = if muted {
            if palette.is_high_contrast {
                &self.brushes.on_accent
            } else {
                &muted_red
            }
        } else if effective > 0 {
            if palette.is_high_contrast {
                &self.brushes.on_accent
            } else {
                &glyph_dark
            }
        } else {
            &self.brushes.text_muted
        };
        let half_h = scale(5, self.dpi) as f32;
        draw_speaker_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx - half_h * 0.86,
            cy,
            half_h,
            (scale(2, self.dpi) as f32 * 0.62).max(1.15),
            snapshot.status.volume,
            muted,
            icon_brush,
        )
    }

    pub(super) fn draw_cover_thumb(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        bytes: Option<&[u8]>,
        palette: &LiquidGlassPalette,
    ) {
        let box_rect = Rect {
            x: rect.left.round() as i32,
            y: rect.top.round() as i32,
            width: (rect.right - rect.left).round().max(1.0) as i32,
            height: (rect.bottom - rect.top).round().max(1.0) as i32,
        };
        if let Some(raw) = bytes
            && let Some(art) = self.cached_or_decode_cover(raw)
            && fill_squircle_bitmap(
                &self.factory,
                &self.target,
                box_rect,
                false,
                radius,
                &art.bitmap,
                1.0,
            )
        {
            return;
        }
        let fill = self.brushes.color_brush(palette.toggle_inactive);
        fill_squircle_or_rounded_f(&self.factory, &self.target, rect, radius, self.dpi, &fill);
        let cx = (rect.left + rect.right) * 0.5;
        let cy = (rect.top + rect.bottom) * 0.5;
        self.draw_note_glyph(
            cx,
            cy,
            (rect.right - rect.left) * 0.25,
            &self.brushes.text_muted,
        );
    }

    pub(super) fn cached_or_decode_cover(&self, bytes: &[u8]) -> Option<CoverArt> {
        let hash = fnv1a_hash(bytes);
        if let Some(cached) = self.cached_cover.borrow().as_ref()
            && cached.hash == hash
            && cached.bytes_len == bytes.len()
        {
            return Some(cached.art.clone());
        }
        let art = decode_image_bytes(&self.target, bytes).ok()?;
        *self.cached_cover.borrow_mut() = Some(CachedCoverArt {
            hash,
            bytes_len: bytes.len(),
            art: art.clone(),
        });
        Some(art)
    }
}
