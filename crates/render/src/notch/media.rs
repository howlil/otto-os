use otto_core::{
    HitTarget, Layout, LyricsView, MediaScene, MediaSubtitle, MediaView, Rect, scale,
    unsupported_command,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BRUSH_PROPERTIES,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_TEXT_METRICS, IDWriteTextFormat};
use windows::core::Result;
use windows_numerics::{Matrix3x2, Vector2};

use crate::appearance::palette::{TonePalette, muted};
use crate::{
    BLACK, CoverArt, DISABLED_CONTROL_ALPHA, FOREGROUND, MUSIC_ACCENT, Renderer, brighten,
    rect_to_d2d, rgb, shared_dwrite_factory, translate_rect,
};

const ART_TINT_MUTING: f32 = 0.25;
const PAUSED_COVER_FADE: f32 = 0.45;
const LIVE_RED: D2D1_COLOR_F = rgb(255, 69, 58, 1.0);

pub(crate) fn media_palette(accent: Option<D2D1_COLOR_F>, dim: f32) -> TonePalette {
    let accent = muted(accent.unwrap_or(MUSIC_ACCENT), ART_TINT_MUTING);
    TonePalette {
        from: accent,
        to: accent,
        accent,
        text: accent,
    }
    .dimmed(dim)
}

fn center_of(rect: Rect) -> Vector2 {
    Vector2::new(
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + rect.height as f32 * 0.5,
    )
}

impl Renderer {
    pub(crate) fn draw_cover_tile(
        &self,
        tile: Rect,
        radius: f32,
        art: Option<&CoverArt>,
        opacity: f32,
        alpha: f32,
    ) {
        let shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(tile),
            radiusX: radius,
            radiusY: radius,
        };
        if let Some(art) = art
            && self.fill_cover(&shape, tile, art, opacity)
        {
            let rim = self.brushes.alpha_brush(FOREGROUND, 0.15 * alpha);
            unsafe {
                self.target.DrawRoundedRectangle(&shape, &rim, 1.0, None);
            }
            return;
        }
        let tile_bg = self.brushes.alpha_brush(MUSIC_ACCENT, 0.20 * alpha);
        let note = self.brushes.alpha_brush(MUSIC_ACCENT, 0.96 * alpha);
        self.draw_rounded_box(tile, radius, &tile_bg);
        let center = center_of(tile);
        self.draw_music_note_glyph(center.X, center.Y, tile.width as f32 * 0.26, &note);
    }

    fn fill_cover(
        &self,
        shape: &D2D1_ROUNDED_RECT,
        tile: Rect,
        art: &CoverArt,
        opacity: f32,
    ) -> bool {
        unsafe {
            let size = art.bitmap.GetSize();
            if size.width <= 0.0 || size.height <= 0.0 {
                return false;
            }
            let properties = D2D1_BRUSH_PROPERTIES {
                opacity,
                transform: Matrix3x2 {
                    M11: tile.width as f32 / size.width,
                    M12: 0.0,
                    M21: 0.0,
                    M22: tile.height as f32 / size.height,
                    M31: tile.x as f32,
                    M32: tile.y as f32,
                },
            };
            let bitmap_properties = D2D1_BITMAP_BRUSH_PROPERTIES {
                extendModeX: D2D1_EXTEND_MODE_CLAMP,
                extendModeY: D2D1_EXTEND_MODE_CLAMP,
                interpolationMode: D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            };
            let Ok(brush) = self.target.CreateBitmapBrush(
                &art.bitmap,
                Some(&raw const bitmap_properties),
                Some(&raw const properties),
            ) else {
                return false;
            };
            self.target.FillRoundedRectangle(shape, &brush);
        }
        true
    }

    fn text_width(text: &str, format: &IDWriteTextFormat) -> f32 {
        let Ok(dwrite) = shared_dwrite_factory() else {
            return 0.0;
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let Ok(layout) = dwrite.CreateTextLayout(&wide, format, 4096.0, 64.0) else {
                return 0.0;
            };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut metrics).is_err() {
                return 0.0;
            }
            metrics.width
        }
    }

    #[must_use]
    pub fn compact_title_points(&self, text: &str) -> f32 {
        (Self::text_width(text, &self.text_leading) * 96.0 / self.dpi.max(1) as f32).ceil()
    }

    fn draw_equalizer_or_dots(
        &self,
        cx: f32,
        cy: f32,
        playing: bool,
        anim_phase: f32,
        accent: D2D1_COLOR_F,
        alpha: f32,
    ) {
        if playing {
            let brush = self.brushes.alpha_brush(accent, 0.88 * alpha);
            self.draw_audio_equalizer(cx, cy, anim_phase, &brush);
            return;
        }
        let step = (scale(2, self.dpi) as f32).max(1.8) * 2.0;
        let radius = (scale(1, self.dpi) as f32).max(0.9);
        let brush = self.brushes.alpha_brush(FOREGROUND, 0.40 * alpha);
        for offset in [-1.5f32, -0.5, 0.5, 1.5] {
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx + offset * step, cy),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                );
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn draw_compact_media(
        &self,
        layout: &Layout,
        scene: &MediaScene,
        anim_phase: f32,
        alpha_scale: f32,
    ) {
        let alpha = ((layout.compact_expansion - 0.15) / 0.85).clamp(0.0, 1.0)
            * alpha_scale.clamp(0.0, 1.0);
        if alpha <= 0.005 {
            return;
        }
        let media = &scene.media;
        let art = media
            .thumbnail
            .as_ref()
            .and_then(|thumbnail| self.get_or_decode_cover_art(thumbnail));
        let palette = media_palette(art.as_ref().and_then(|cover| cover.accent), scene.dim);
        let view = scene.view(None);
        let bar_cy = layout.bar.height as f32 * 0.5;

        let size = scale(20, self.dpi);
        let center = center_of(layout.compact_ring);
        let tile = Rect {
            x: (center.X - size as f32 * 0.5).round() as i32,
            y: (center.Y - size as f32 * 0.5).round() as i32,
            width: size,
            height: size,
        };
        let radius = scale(6, self.dpi) as f32;
        self.draw_cover_tile(
            tile,
            radius,
            art.as_ref(),
            alpha * (1.0 - PAUSED_COVER_FADE * scene.dim),
            alpha,
        );

        let title_rect = layout.compact_title;
        let ink = 0.92 - 0.30 * scene.dim;
        if let Some(peek) = &scene.peek {
            let strength = peek.alpha.clamp(0.0, 1.0) * alpha;
            let muted = self.brushes.alpha_brush(FOREGROUND, 0.55 * strength);
            let strong = self.brushes.alpha_brush(FOREGROUND, 0.95 * strength);
            let label_width =
                Self::text_width(peek.label, &self.text_leading) + scale(6, self.dpi) as f32;
            let title_width = Self::text_width(&peek.title, &self.text_leading);
            let start_x = title_rect.x as f32
                + ((title_rect.width as f32 - (label_width + title_width)).max(0.0) * 0.5);
            self.draw_text(
                peek.label,
                &self.text_leading,
                D2D_RECT_F {
                    left: start_x,
                    top: title_rect.y as f32,
                    right: title_rect.right() as f32,
                    bottom: title_rect.bottom() as f32,
                },
                &muted,
            );
            self.draw_text(
                &peek.title,
                &self.text_leading,
                D2D_RECT_F {
                    left: start_x + label_width,
                    top: title_rect.y as f32,
                    right: title_rect.right() as f32,
                    bottom: title_rect.bottom() as f32,
                },
                &strong,
            );
        } else {
            let brush = if view.compact_is_lyric {
                self.brushes.alpha_brush(palette.accent, 0.95 * alpha)
            } else {
                self.brushes.alpha_brush(FOREGROUND, ink * alpha)
            };
            self.draw_text(
                &view.compact_text,
                &self.text_compact_center,
                rect_to_d2d(title_rect),
                &brush,
            );
        }

        let trailing = layout.compact_trailing;
        if view.live {
            self.draw_live_badge(trailing, alpha);
        } else {
            self.draw_equalizer_or_dots(
                trailing.x as f32 + trailing.width as f32 * 0.5,
                bar_cy,
                !view.paused,
                anim_phase,
                palette.accent,
                alpha,
            );
        }
    }

    pub(crate) fn draw_secondary_media_circle(
        &self,
        layout: &Layout,
        scene: &MediaScene,
        _anim_phase: f32,
        alpha_scale: f32,
    ) -> Result<()> {
        let alpha = ((layout.secondary_expansion - 0.15) / 0.85).clamp(0.0, 1.0)
            * alpha_scale.clamp(0.0, 1.0);
        if alpha <= 0.005 || layout.notch_secondary.is_empty() {
            return Ok(());
        }
        let media = &scene.media;
        let art = media
            .thumbnail
            .as_ref()
            .and_then(|thumbnail| self.get_or_decode_cover_art(thumbnail));
        let palette = media_palette(art.as_ref().and_then(|cover| cover.accent), scene.dim);
        let view = scene.view(None);
        let circle = layout.notch_secondary;
        let center = center_of(circle);
        let size = (circle.width - scale(8, self.dpi)).max(scale(14, self.dpi));
        let tile = Rect {
            x: (center.X - size as f32 * 0.5).round() as i32,
            y: (center.Y - size as f32 * 0.5).round() as i32,
            width: size,
            height: size,
        };
        let radius = size as f32 * 0.5;
        self.draw_cover_tile(
            tile,
            radius,
            art.as_ref(),
            alpha * (1.0 - PAUSED_COVER_FADE * scene.dim),
            alpha,
        );

        let ring_r = (circle.width as f32 * 0.5 - scale(2, self.dpi) as f32).max(radius + 1.0);
        let stroke = (scale(3, self.dpi) as f32 * 0.5).max(1.4);
        let ring_progress = if view.live {
            1.0
        } else {
            view.progress.clamp(0.08, 1.0)
        };
        let active_brush = self.brushes.alpha_brush(palette.accent, 0.92 * alpha);
        let track_brush = self.brushes.alpha_brush(FOREGROUND, 0.16 * alpha);
        self.draw_progress_ring(
            center.X,
            center.Y,
            ring_r,
            stroke,
            ring_progress,
            &active_brush,
            &track_brush,
        )?;
        Ok(())
    }

    fn draw_live_badge(&self, trailing: Rect, alpha: f32) {
        let text_width = Self::text_width("LIVE", &self.text_tab);
        let pad = scale(5, self.dpi) as f32;
        let height = scale(14, self.dpi) as f32;
        let right = trailing.right() as f32;
        let left = right - text_width - pad * 2.0;
        let top = trailing.y as f32 + (trailing.height as f32 - height) * 0.5;
        let pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right,
                bottom: top + height,
            },
            radiusX: height * 0.5,
            radiusY: height * 0.5,
        };
        let fill = self.brushes.alpha_brush(LIVE_RED, 0.22 * alpha);
        let ink = self.brushes.alpha_brush(LIVE_RED, 0.95 * alpha);
        unsafe {
            self.target.FillRoundedRectangle(&pill, &fill);
        }
        self.draw_text("LIVE", &self.text_tab, pill.rect, &ink);
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn draw_expanded_media(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        focused: Option<HitTarget>,
        scene: &MediaScene,
        anim_phase: f32,
        lyrics: LyricsView<'_>,
    ) -> Result<()> {
        let media = &scene.media;
        let cover = translate_rect(layout.media.cover_art, -offset_x);
        let art = media
            .thumbnail
            .as_ref()
            .and_then(|thumbnail| self.get_or_decode_cover_art(thumbnail));
        let palette = media_palette(art.as_ref().and_then(|cover| cover.accent), scene.dim);
        let engaged_unsupported = [hovered, focused]
            .into_iter()
            .flatten()
            .find_map(|target| unsupported_command(target, media.capabilities));
        let lyric_line = lyrics
            .sheet
            .and_then(|sheet| sheet.active_line(media.position_ms));
        let view = MediaView::resolve(media, engaged_unsupported, lyric_line);
        let radius = scale(12, self.dpi) as f32;
        self.draw_cover_tile(
            cover,
            radius,
            art.as_ref(),
            alpha * (1.0 - PAUSED_COVER_FADE * scene.dim),
            alpha,
        );

        let fg = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.60 * alpha);
        self.draw_text(
            &media.title,
            &self.text_alert_title,
            rect_to_d2d(translate_rect(layout.media.title, -offset_x)),
            &fg,
        );
        match view.subtitle_kind {
            MediaSubtitle::Unsupported(_) => self.draw_text(
                &view.subtitle,
                &self.text_wrap,
                rect_to_d2d(translate_rect(layout.media.reason, -offset_x)),
                &muted,
            ),
            MediaSubtitle::Lyric => {
                let accent = self.brushes.alpha_brush(palette.accent, 0.95 * alpha);
                self.draw_text(
                    &view.subtitle,
                    &self.text_pomodoro_sub,
                    rect_to_d2d(translate_rect(layout.media.artist, -offset_x)),
                    &accent,
                );
            }
            MediaSubtitle::Artist => self.draw_text(
                &view.subtitle,
                &self.text_pomodoro_sub,
                rect_to_d2d(translate_rect(layout.media.artist, -offset_x)),
                &muted,
            ),
        }
        if !matches!(view.subtitle_kind, MediaSubtitle::Unsupported(_)) {
            let eq = translate_rect(layout.media.equalizer, -offset_x);
            self.draw_equalizer_or_dots(
                eq.x as f32 + scale(7, self.dpi) as f32,
                eq.y as f32 + eq.height as f32 * 0.5,
                !view.paused,
                anim_phase,
                palette.accent,
                alpha,
            );
        }

        let source_brush = self.brushes.alpha_brush(FOREGROUND, 0.55 * alpha);
        self.draw_text(
            &view.source,
            &self.text_pomodoro_sub_trailing,
            rect_to_d2d(translate_rect(layout.media.source_label, -offset_x)),
            &source_brush,
        );
        self.draw_lyrics_toggle_button(
            translate_rect(layout.media.lyrics_toggle, -offset_x),
            alpha,
            hovered == Some(HitTarget::MediaLyricsToggle),
            layout.lyrics_progress >= 0.5,
            &palette,
        );

        if layout.lyrics_progress > 0.0 {
            let drawer = translate_rect(layout.media.lyrics_drawer, -offset_x);
            let drawer_alpha = alpha * ((layout.lyrics_progress - 0.2) / 0.8).clamp(0.0, 1.0);
            self.draw_lyrics_drawer(drawer, drawer_alpha, media, lyrics, anim_phase);
        }

        let seek_engaged =
            hovered == Some(HitTarget::MediaSeekbar) || focused == Some(HitTarget::MediaSeekbar);
        self.draw_media_seekbar(
            translate_rect(layout.media.seekbar, -offset_x),
            alpha,
            seek_engaged,
            scene,
            &view,
            &palette,
        );
        self.draw_media_action_buttons(layout, offset_x, alpha, hovered, scene, &palette)
    }

    fn draw_lyrics_toggle_button(
        &self,
        rect: Rect,
        alpha: f32,
        hovered: bool,
        expanded: bool,
        palette: &TonePalette,
    ) {
        let radius = scale(8, self.dpi) as f32;
        let (colour, strength) = if expanded {
            (palette.accent, 0.90)
        } else if hovered {
            (brighten(FOREGROUND, 0.2), 0.20)
        } else {
            (FOREGROUND, 0.10)
        };
        let fill = self.brushes.alpha_brush(colour, strength * alpha);
        self.draw_rounded_box(rect, radius, &fill);
        let glyph_colour = if expanded {
            BLACK
        } else if hovered {
            FOREGROUND
        } else {
            rgb(200, 205, 215, 1.0)
        };
        let glyph = self.brushes.alpha_brush(glyph_colour, 0.92 * alpha);
        self.draw_quote_glyph(center_of(rect), scale(5, self.dpi) as f32, &glyph);
    }

    fn draw_quote_glyph(&self, center: Vector2, g: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (g * 0.34).max(1.4);
        for dx in [-0.6f32, 0.6] {
            let x = center.X + g * dx;
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(x, center.Y - g * 0.2),
                        radiusX: g * 0.42,
                        radiusY: g * 0.42,
                    },
                    brush,
                );
                self.target.DrawLine(
                    Vector2::new(x + g * 0.32, center.Y - g * 0.1),
                    Vector2::new(x - g * 0.12, center.Y + g * 0.85),
                    brush,
                    stroke,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_media_seekbar(
        &self,
        seek: Rect,
        alpha: f32,
        engaged: bool,
        scene: &MediaScene,
        view: &MediaView,
        palette: &TonePalette,
    ) {
        let track_h = scale(4, self.dpi);
        let track = Rect {
            x: seek.x,
            y: seek.y + (seek.height - track_h) / 2,
            width: seek.width,
            height: track_h,
        };
        let lit = if engaged && view.seekable {
            TonePalette {
                from: brighten(palette.from, 0.14),
                to: brighten(palette.to, 0.14),
                ..*palette
            }
        } else {
            *palette
        };
        if view.live {
            self.draw_drifting_lip(track, scene.live_phase, &lit, alpha);
        } else {
            self.draw_progress_lip(track, view.progress, &lit, alpha);
            if engaged && view.seekable {
                let thumb_x = track.x as f32 + track.width as f32 * view.progress.clamp(0.0, 1.0);
                let thumb = self.brushes.alpha_brush(FOREGROUND, 0.98 * alpha);
                let radius = scale(5, self.dpi) as f32;
                unsafe {
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(thumb_x, track.y as f32 + track_h as f32 * 0.5),
                            radiusX: radius,
                            radiusY: radius,
                        },
                        &thumb,
                    );
                }
            }
        }
        let label_top = (track.bottom() + scale(4, self.dpi)) as f32;
        let label_bottom = label_top + scale(14, self.dpi) as f32;
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.55 * alpha);
        self.draw_tabular_text(
            &view.elapsed,
            &self.text_pomodoro_sub,
            D2D_RECT_F {
                left: seek.x as f32,
                top: label_top,
                right: seek.x as f32 + scale(72, self.dpi) as f32,
                bottom: label_bottom,
            },
            &muted,
        );
        let remaining_brush = if view.live {
            self.brushes.alpha_brush(FOREGROUND, 0.85 * alpha)
        } else {
            muted
        };
        self.draw_tabular_text(
            &view.remaining,
            &self.text_pomodoro_sub_trailing,
            D2D_RECT_F {
                left: seek.right() as f32 - scale(72, self.dpi) as f32,
                top: label_top,
                right: seek.right() as f32,
                bottom: label_bottom,
            },
            &remaining_brush,
        );
    }

    fn draw_media_action_buttons(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &MediaScene,
        palette: &TonePalette,
    ) -> Result<()> {
        let media = &scene.media;
        let capabilities = media.capabilities;
        let fg = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let disabled = self
            .brushes
            .alpha_brush(FOREGROUND, DISABLED_CONTROL_ALPHA * alpha);
        let fill_for = |target: HitTarget, enabled: bool| {
            let strength = if !enabled {
                0.06
            } else if hovered == Some(target) {
                0.25
            } else {
                0.14
            };
            self.brushes.alpha_brush(FOREGROUND, strength * alpha)
        };
        let size = scale(6, self.dpi) as f32;

        let previous = translate_rect(layout.media.prev_button, -offset_x);
        self.draw_circle_button(
            previous,
            &fill_for(HitTarget::MediaPrevious, capabilities.previous),
        );
        let centre = center_of(previous);
        self.draw_previous_glyph(
            centre.X,
            centre.Y,
            size,
            if capabilities.previous {
                &fg
            } else {
                &disabled
            },
        )?;

        let play = translate_rect(layout.media.play_button, -offset_x);
        let play_centre = center_of(play);
        let enabled = capabilities.play_pause;
        let play_color = if hovered == Some(HitTarget::MediaPlayPause) && enabled {
            brighten(palette.accent, 0.12)
        } else {
            palette.accent
        };
        let play_alpha = if enabled {
            alpha
        } else {
            DISABLED_CONTROL_ALPHA * alpha
        };
        self.fill_solid_circle(play_centre, play.width as f32 * 0.5, play_color, play_alpha);
        let ink = self
            .brushes
            .alpha_brush(BLACK, (0.92 - scene.dim * 0.2) * alpha);
        let glyph = scale(8, self.dpi) as f32;
        if media.playing {
            self.draw_pause_glyph(play_centre.X, play_centre.Y, glyph, &ink);
        } else {
            self.draw_play_glyph(play_centre.X, play_centre.Y, glyph, &ink)?;
        }

        let next = translate_rect(layout.media.next_button, -offset_x);
        self.draw_circle_button(next, &fill_for(HitTarget::MediaNext, capabilities.next));
        let centre = center_of(next);
        self.draw_skip_glyph(
            centre.X,
            centre.Y,
            size,
            if capabilities.next { &fg } else { &disabled },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saturation(colour: D2D1_COLOR_F) -> f32 {
        colour.r.max(colour.g).max(colour.b) - colour.r.min(colour.g).min(colour.b)
    }

    #[test]
    fn a_missing_accent_falls_back_to_the_music_pink() {
        let palette = media_palette(None, 0.0);
        assert!(palette.accent.r > palette.accent.g);
        assert!(saturation(palette.accent) < saturation(MUSIC_ACCENT));
    }

    #[test]
    fn the_cover_accent_drives_the_whole_palette() {
        let blue = rgb(70, 110, 230, 1.0);
        let palette = media_palette(Some(blue), 0.0);
        assert!(palette.accent.b > palette.accent.r);
        assert!(saturation(palette.accent) < saturation(blue));
        assert!((palette.from.r - palette.to.r).abs() < f32::EPSILON);
        assert!((palette.from.g - palette.to.g).abs() < f32::EPSILON);
        assert!((palette.from.b - palette.to.b).abs() < f32::EPSILON);
    }

    #[test]
    fn pausing_steps_the_palette_back_without_losing_its_hue() {
        let live = media_palette(Some(rgb(70, 110, 230, 1.0)), 0.0);
        let paused = media_palette(Some(rgb(70, 110, 230, 1.0)), 1.0);
        assert!(saturation(paused.accent) < saturation(live.accent) * 0.6);
        assert!(paused.accent.b >= paused.accent.r);
    }

    #[test]
    fn dimming_by_zero_changes_nothing() {
        let base = media_palette(Some(rgb(255, 90, 125, 1.0)), 0.0);
        let same = base.dimmed(0.0);
        assert!((same.accent.r - base.accent.r).abs() < f32::EPSILON);
    }
}
