use crate::appearance::surface::fnv1a_hash;
use otto_core::{CompactSlotKind, HitTarget, Layout, MediaScene, PomodoroScene, Rect, scale};
use std::cell::RefCell;
use windows::Win32::Graphics::Direct2D::Common::D2D1_FIGURE_END_CLOSED;
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, ID2D1PathGeometry};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::CachedCoverArt;
use crate::CachedNotchGeometry;
use crate::CoverArt;
use crate::FOREGROUND;
use crate::Renderer;
use crate::decode_image_bytes;
use crate::write_attached_notch_figure;
use crate::write_floating_notch_figure;

impl Renderer {
    pub(crate) fn draw_compact_notch(
        &self,
        layout: &Layout,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        call: Option<&otto_core::CallScene>,
        anim_phase: f32,
    ) -> Result<()> {
        let primary = layout.compact_primary.unwrap_or_else(|| {
            if pomodoro.state.is_compact_active()
                || pomodoro.timer.is_compact_active()
                || pomodoro.stopwatch.is_compact_active()
            {
                CompactSlotKind::Focus
            } else {
                CompactSlotKind::Media
            }
        });
        let blend = layout.compact_swap_blend.clamp(0.0, 1.0);
        let slide = scale(8, self.dpi) as f32;
        if blend < 0.995 {
            self.set_content_offset(blend * slide, 0.0);
            match primary {
                CompactSlotKind::Focus => {
                    if let Some(scene) = media {
                        self.draw_compact_media(layout, scene, anim_phase, 1.0 - blend);
                    }
                }
                CompactSlotKind::Media => {
                    self.draw_compact_pomodoro(layout, pomodoro, 1.0 - blend)?;
                }
                CompactSlotKind::Call => {}
            }
            self.set_content_offset((1.0 - blend) * slide, 0.0);
        }
        let result = match primary {
            CompactSlotKind::Focus => self.draw_compact_pomodoro(layout, pomodoro, blend),
            CompactSlotKind::Media => {
                if let Some(scene) = media {
                    self.draw_compact_media(layout, scene, anim_phase, blend);
                }
                Ok(())
            }
            CompactSlotKind::Call => match call {
                Some(scene) => self.draw_compact_call(layout, scene, anim_phase, 1.0),
                None => Ok(()),
            },
        };
        if blend < 0.995 {
            self.set_content_offset(0.0, 0.0);
        }
        result
    }

    pub(crate) fn draw_secondary_notch_frame(
        &self,
        layout: &Layout,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        is_high_contrast: bool,
    ) {
        let rect = layout.notch_secondary;
        let r = rect.width.min(rect.height) as f32 * 0.5;
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let ellipse = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: r,
            radiusY: r,
        };
        unsafe {
            self.target
                .FillEllipse(&ellipse, &self.brushes.notch_obsidian);
            if layout.notch.y > 0 && !is_high_contrast {
                let floating_rim = self.brushes.alpha_brush(FOREGROUND, 0.14);
                self.target
                    .DrawEllipse(&ellipse, &floating_rim, 1.0, &self.stroke_style);
            } else {
                self.target.DrawEllipse(
                    &ellipse,
                    &self.brushes.specular_rim,
                    1.0,
                    &self.stroke_style,
                );
            }
            if hovered == Some(HitTarget::NotchSecondary) {
                let hover_fill = self.brushes.alpha_brush(FOREGROUND, 0.10);
                let hover_rim = self.brushes.alpha_brush(FOREGROUND, 0.28);
                self.target.FillEllipse(&ellipse, &hover_fill);
                self.target
                    .DrawEllipse(&ellipse, &hover_rim, 1.2, &self.stroke_style);
            }
        }
        if focused == Some(HitTarget::NotchSecondary) {
            self.draw_control_focus_ring(rect, &self.brushes.text_primary);
        }
    }

    pub(crate) fn draw_secondary_notch_content(
        &self,
        layout: &Layout,
        pomodoro: &PomodoroScene,
        media: Option<&MediaScene>,
        anim_phase: f32,
    ) -> Result<()> {
        let primary = layout.compact_primary.unwrap_or(CompactSlotKind::Focus);
        let secondary = layout.compact_secondary.unwrap_or(match primary {
            CompactSlotKind::Focus | CompactSlotKind::Call => CompactSlotKind::Media,
            CompactSlotKind::Media => CompactSlotKind::Focus,
        });
        let blend = layout.compact_swap_blend.clamp(0.0, 1.0);
        let alpha_scale = 0.35 + 0.65 * blend;
        match secondary {
            CompactSlotKind::Focus => {
                self.draw_secondary_pomodoro_circle(layout, pomodoro, alpha_scale)
            }
            CompactSlotKind::Media => {
                if let Some(scene) = media {
                    self.draw_secondary_media_circle(layout, scene, anim_phase, alpha_scale)?;
                }
                Ok(())
            }
            CompactSlotKind::Call => Ok(()),
        }
    }

    pub(crate) fn get_or_decode_cover_art(&self, thumbnail: &[u8]) -> Option<CoverArt> {
        let hash = fnv1a_hash(thumbnail);
        let len = thumbnail.len();
        if let Some(cached) = self.cached_cover_art.borrow().as_ref()
            && cached.hash == hash
            && cached.bytes_len == len
        {
            return Some(cached.art.clone());
        }
        let art = decode_image_bytes(&self.target, thumbnail).ok()?;
        *self.cached_cover_art.borrow_mut() = Some(CachedCoverArt {
            hash,
            bytes_len: len,
            art: art.clone(),
        });
        Some(art)
    }
}

impl Renderer {
    pub(crate) fn notch_geometry_rect(
        &self,
        n: Rect,
        corner_radius: f32,
        ear_radius: f32,
        cache: &RefCell<Option<CachedNotchGeometry>>,
    ) -> Result<ID2D1PathGeometry> {
        let cr_bits = corner_radius.to_bits();
        let er_bits = ear_radius.to_bits();
        if let Some(cached) = cache.borrow().as_ref()
            && cached.rect == n
            && cached.corner_radius_bits == cr_bits
            && cached.ear_radius_bits == er_bits
        {
            return Ok(cached.geometry.clone());
        }

        let (left, right) = (n.x as f32, n.right() as f32);
        let top = n.y as f32;
        let bottom = n.bottom() as f32;

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            if ear_radius <= 0.5 {
                let cr = corner_radius
                    .min(n.width as f32 * 0.5)
                    .min(n.height as f32 * 0.5)
                    .max(0.0);
                write_floating_notch_figure(&sink, left, top, right, bottom, cr);
            } else {
                write_attached_notch_figure(
                    &sink,
                    left,
                    top,
                    right,
                    bottom,
                    corner_radius.min(n.height as f32 * 0.5).max(0.0),
                    ear_radius.max(1.0),
                );
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            *cache.borrow_mut() = Some(CachedNotchGeometry {
                rect: n,
                corner_radius_bits: cr_bits,
                ear_radius_bits: er_bits,
                geometry: geometry.clone(),
            });
            Ok(geometry)
        }
    }
}
