use otto_core::{
    ControlCenterHitTarget, ControlCenterSnapshot, ControlKind, ControlTileLayout, Rect,
    ResolvedTheme, focus_row_subtitle, power_mode_label,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::ID2D1Brush;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::tokens::{FOCUS_PURPLE, SYSTEM_RED_LIGHT};
use crate::kit::draw::{
    fill_ellipse_on, fill_squircle_or_rounded_f, pop_axis_clip_on, push_axis_clip_on,
};
use crate::{CHARGING_GREEN, LiquidGlassPalette, draw_speaker_glyph, rect_to_d2d, scale};

use super::{ControlCenterPaintState, ControlCenterRenderer, ControlVisualState, PointerState};

impl ControlCenterRenderer {
    pub(super) fn draw_brightness_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        if tile.slider_vertical {
            let hovered = state.hovered == Some(ControlCenterHitTarget::DisplaySlider);
            let pressed = state.pressed == Some(ControlCenterHitTarget::DisplaySlider);
            self.draw_vertical_capsule_slider(
                tile.slider_rect,
                radius,
                snapshot.status.brightness,
                (hovered, pressed),
                palette,
                |cx, cy, brush| self.draw_sun_glyph(cx, cy, scale(8, self.dpi) as f32, brush),
            );
            return;
        }
        self.draw_tile_surface(tile.rect, radius, false, false, palette);
        if tile.cols >= 4 {
            let value_text = snapshot
                .status
                .brightness
                .map_or_else(|| String::from("Unavailable"), |pct| format!("{pct}%"));
            self.draw_slider_header(tile.rect, tile.detail_rect, "Display", &value_text, true);
        }
        self.draw_capsule_slider(
            tile.slider_rect,
            snapshot.status.brightness,
            state.hovered == Some(ControlCenterHitTarget::DisplaySlider),
            state.pressed == Some(ControlCenterHitTarget::DisplaySlider),
            true,
            palette,
        );
    }

    pub(super) fn draw_volume_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let muted = snapshot.status.muted;
        let effective = if muted { 0 } else { snapshot.status.volume };
        if tile.slider_vertical {
            let hovered = state.hovered == Some(ControlCenterHitTarget::SoundSlider)
                || state.hovered == Some(ControlCenterHitTarget::SoundMuteToggle);
            let pressed = state.pressed == Some(ControlCenterHitTarget::SoundSlider)
                || state.pressed == Some(ControlCenterHitTarget::SoundMuteToggle);
            self.draw_vertical_capsule_slider(
                tile.slider_rect,
                radius,
                Some(effective),
                (hovered, pressed),
                palette,
                |cx, cy, brush| {
                    let half_h = scale(6, self.dpi) as f32;
                    let fg = if muted && !palette.is_high_contrast {
                        self.brushes.color_brush(SYSTEM_RED_LIGHT)
                    } else {
                        brush.clone()
                    };
                    let _ = draw_speaker_glyph(
                        &self.factory,
                        &self.target,
                        &self.stroke_style,
                        cx - half_h * 0.72,
                        cy,
                        half_h,
                        (scale(2, self.dpi) as f32 * 0.65).max(1.2),
                        snapshot.status.volume,
                        muted,
                        &fg,
                    );
                },
            );
            return;
        }
        self.draw_tile_surface(tile.rect, radius, false, false, palette);
        if tile.cols >= 4 {
            let output_label = snapshot
                .audio_outputs
                .iter()
                .find(|dev| dev.is_default)
                .map_or_else(|| String::from("Output"), |dev| dev.name.clone());
            let value_text = if muted || snapshot.status.volume == 0 {
                String::from("Muted")
            } else {
                format!("{}%", snapshot.status.volume)
            };
            let right_label = format!("{output_label} · {value_text}");
            self.draw_slider_header(tile.rect, tile.detail_rect, "Sound", &right_label, true);
        }
        self.draw_capsule_slider(
            tile.slider_rect,
            Some(effective),
            state.hovered == Some(ControlCenterHitTarget::SoundSlider),
            state.pressed == Some(ControlCenterHitTarget::SoundSlider),
            false,
            palette,
        );
    }

    pub(super) fn draw_vertical_capsule_slider(
        &self,
        rect: Rect,
        radius: f32,
        value: Option<u8>,
        pointer: (bool, bool),
        palette: &LiquidGlassPalette,
        glyph: impl FnOnce(f32, f32, &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush),
    ) {
        self.draw_tile_surface(rect, radius, pointer.0, pointer.1, palette);
        let d2d_r = rect_to_d2d(rect);
        let clip_geo = self.squircle_path(d2d_r, radius);
        if let Some(pct) = value {
            let clamped = pct.min(100);
            if clamped > 0 {
                let fill_h = (rect.height as f32 * (f32::from(clamped) / 100.0))
                    .max(rect.width as f32 * 0.36);
                let fill_top = (rect.bottom() as f32 - fill_h).max(rect.y as f32);
                let clip_r = D2D_RECT_F {
                    left: d2d_r.left,
                    top: fill_top,
                    right: d2d_r.right,
                    bottom: d2d_r.bottom,
                };
                let fill_color = if palette.is_high_contrast {
                    palette.slider_fill
                } else {
                    D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: if palette.is_dark { 0.90 } else { 0.95 },
                    }
                };
                let fill_brush = self.brushes.color_brush(fill_color);
                push_axis_clip_on(&self.target, clip_r);
                crate::kit::draw::fill_shape(
                    &self.target,
                    d2d_r,
                    radius,
                    clip_geo.as_ref(),
                    (&fill_brush).into(),
                );
                pop_axis_clip_on(&self.target);
            }
        }
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.bottom() as f32 - scale(19, self.dpi) as f32;
        let dark = self.slider_cap_dark_brush();
        let active_fill = value.is_some_and(|pct| pct > 18);
        let icon_brush = if palette.is_high_contrast && active_fill {
            &self.brushes.on_accent
        } else if active_fill {
            &dark
        } else {
            &self.brushes.text_muted
        };
        glyph(cx, cy, icon_brush);
    }

    pub(super) fn draw_now_playing_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_tile_surface(tile.rect, radius, false, false, palette);
        let media_enabled = snapshot.settings.system.media_module;
        let has_media = snapshot.media.is_some();
        let playing = snapshot.media.as_ref().is_some_and(|m| m.playing);
        let thumb_bytes = if media_enabled {
            snapshot.media.as_ref().and_then(|m| m.thumbnail.as_deref())
        } else {
            None
        };
        let (track_name, artist_name) = if media_enabled {
            match &snapshot.media {
                Some(media) => {
                    let sub = if media.artist.is_empty() {
                        "Playing"
                    } else {
                        media.artist.as_str()
                    };
                    (media.title.as_str(), sub)
                }
                None => ("Not Playing", "Open Media"),
            }
        } else {
            ("Media Off", "Enable in Settings")
        };

        let pad = scale(12, self.dpi) as f32;
        if tile.rows >= 2 && tile.cols <= 2 {
            let art_size = scale(44, self.dpi) as f32;
            let art_rect = D2D_RECT_F {
                left: tile.rect.x as f32 + pad,
                top: tile.rect.y as f32 + pad,
                right: tile.rect.x as f32 + pad + art_size,
                bottom: tile.rect.y as f32 + pad + art_size,
            };
            self.draw_cover_thumb(art_rect, scale(10, self.dpi) as f32, thumb_bytes, palette);
            let text_left = tile.rect.x as f32 + pad;
            let text_right = tile.rect.right() as f32 - pad;
            let text_mid = art_rect.bottom + scale(22, self.dpi) as f32;
            self.draw_two_line_label(
                text_left,
                text_right,
                text_mid,
                track_name,
                artist_name,
                &self.brushes.text_primary,
                &self.brushes.text_muted,
                false,
            );
        } else {
            let art_size = scale(46, self.dpi) as f32;
            let art_top = tile.rect.y as f32 + (tile.rect.height as f32 - art_size) * 0.5;
            let art_rect = D2D_RECT_F {
                left: tile.rect.x as f32 + pad,
                top: art_top,
                right: tile.rect.x as f32 + pad + art_size,
                bottom: art_top + art_size,
            };
            self.draw_cover_thumb(art_rect, scale(8, self.dpi) as f32, thumb_bytes, palette);
            let text_left = art_rect.right + scale(10, self.dpi) as f32;
            let text_right =
                (tile.secondary_rect.x as f32 - scale(8, self.dpi) as f32).max(text_left + 1.0);
            let mid = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
            self.draw_two_line_label(
                text_left,
                text_right,
                mid,
                track_name,
                artist_name,
                &self.brushes.text_primary,
                &self.brushes.text_muted,
                false,
            );
        }

        self.draw_now_playing_controls(
            tile,
            media_enabled && has_media,
            media_enabled,
            playing,
            state,
            palette,
        )
    }

    fn draw_now_playing_controls(
        &self,
        tile: &ControlTileLayout,
        controls_active: bool,
        media_enabled: bool,
        playing: bool,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        for (rect, target, kind) in [
            (tile.secondary_rect, ControlCenterHitTarget::MediaPrev, -1),
            (
                tile.tertiary_rect,
                ControlCenterHitTarget::MediaPlayPause,
                0,
            ),
            (tile.detail_rect, ControlCenterHitTarget::MediaNext, 1),
        ] {
            let cx = rect.x as f32 + rect.width as f32 * 0.5;
            let cy = rect.y as f32 + rect.height as f32 * 0.5;
            let btn_r = rect.width as f32 * 0.5;
            if kind == 0 {
                let pill_color = if !media_enabled {
                    palette.slider_track
                } else if controls_active {
                    palette.toggle_active
                } else {
                    palette.toggle_inactive
                };
                let bg = self.brushes.color_brush(pill_color);
                fill_ellipse_on(
                    &self.target,
                    Vector2::new(cx, cy),
                    btn_r,
                    btn_r,
                    (&bg).into(),
                );
            }
            if media_enabled {
                self.draw_interaction_overlay(
                    rect,
                    btn_r,
                    state.hovered == Some(target),
                    state.pressed == Some(target),
                    palette,
                );
            }
            let glyph_brush = if !media_enabled {
                &self.brushes.text_disabled
            } else if kind == 0 && controls_active {
                &self.brushes.on_accent
            } else {
                &self.brushes.text_primary
            };
            if kind == 0 {
                self.draw_play_pause_glyph(
                    cx,
                    cy,
                    scale(6, self.dpi) as f32,
                    playing,
                    glyph_brush,
                )?;
            } else {
                self.draw_skip_glyph(cx, cy, scale(5, self.dpi) as f32, kind > 0, glyph_brush)?;
            }
        }
        Ok(())
    }

    pub(super) fn draw_focus_grid_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let focus_available =
            snapshot.settings.plugins.notch && snapshot.settings.system.focus_module;
        let active = focus_available && snapshot.focus_active;
        let hovered = state.hovered == Some(ControlCenterHitTarget::FocusToggle);
        let pressed = state.pressed == Some(ControlCenterHitTarget::FocusToggle);
        let accent = if palette.is_high_contrast {
            palette.toggle_active
        } else {
            FOCUS_PURPLE
        };
        if tile.cols == 1 && tile.rows == 1 {
            self.draw_compact_toggle_tile(
                tile.rect,
                radius,
                (active, hovered, pressed),
                accent,
                palette,
                |cx, cy, half, brush| {
                    let _ = crate::draw_crescent_moon(
                        &self.factory,
                        &self.target,
                        Vector2::new(cx, cy),
                        half * 0.95,
                        brush,
                    );
                },
            );
            return Ok(());
        }
        self.draw_tile_surface(tile.rect, radius, hovered, pressed, palette);
        let pad = scale(12, self.dpi);
        let d = scale(32, self.dpi);
        let circle = Rect {
            x: tile.rect.x + pad,
            y: tile.rect.y + (tile.rect.height - d) / 2,
            width: d,
            height: d,
        };
        let (cx, cy, rad, fg) = self.draw_circle_badge(
            circle,
            ControlVisualState {
                active,
                enabled: focus_available,
                pointer: PointerState::Normal,
            },
            accent,
            palette,
        );
        crate::draw_crescent_moon(
            &self.factory,
            &self.target,
            Vector2::new(cx, cy),
            rad * 0.58,
            fg,
        )?;
        let text_left = circle.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = if tile.detail_rect.is_empty() {
            tile.rect.right() as f32 - scale(10, self.dpi) as f32
        } else {
            tile.detail_rect.x as f32 - scale(4, self.dpi) as f32
        };
        let mid = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
        let sub = focus_row_subtitle(focus_available, snapshot.focus_active);
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            "Focus",
            sub,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            false,
        );
        if !tile.detail_rect.is_empty() {
            let gx = tile.detail_rect.x as f32 + tile.detail_rect.width as f32 * 0.5;
            let gy = tile.detail_rect.y as f32 + tile.detail_rect.height as f32 * 0.5;
            self.draw_more_glyph(gx, gy, scale(5, self.dpi) as f32, &self.brushes.text_muted);
        }
        Ok(())
    }

    pub(super) fn draw_battery_grid_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let hovered = state.hovered == Some(ControlCenterHitTarget::BatteryTile);
        let pressed = state.pressed == Some(ControlCenterHitTarget::BatteryTile);
        let charging = snapshot.status.battery.charging;
        let accent = if palette.is_high_contrast {
            palette.toggle_active
        } else {
            CHARGING_GREEN
        };
        if tile.cols == 1 && tile.rows == 1 {
            self.draw_tile_surface(tile.rect, radius, hovered, pressed, palette);
            let cx = tile.rect.x as f32 + tile.rect.width as f32 * 0.5;
            let cy = tile.rect.y as f32 + tile.rect.height as f32 * 0.36;
            let icon_brush = if charging {
                self.brushes.color_brush(accent)
            } else {
                self.brushes.text_primary.clone()
            };
            self.draw_battery_glyph(
                cx,
                cy,
                scale(11, self.dpi) as f32,
                snapshot.status.battery.fill_fraction(),
                &icon_brush,
            );
            let label = snapshot
                .status
                .battery
                .percent
                .map_or_else(|| String::from("AC"), |pct| format!("{pct}%"));
            let label_r = D2D_RECT_F {
                left: tile.rect.x as f32 + scale(4, self.dpi) as f32,
                top: tile.rect.y as f32 + tile.rect.height as f32 * 0.52,
                right: tile.rect.right() as f32 - scale(4, self.dpi) as f32,
                bottom: tile.rect.bottom() as f32 - scale(6, self.dpi) as f32,
            };
            self.draw_tabular_text(
                &label,
                &self.center_format,
                label_r,
                &self.brushes.text_primary,
            );
            return;
        }
        self.draw_tile_surface(tile.rect, radius, hovered, pressed, palette);
        let pad = scale(12, self.dpi);
        let d = scale(32, self.dpi);
        let circle = Rect {
            x: tile.rect.x + pad,
            y: tile.rect.y + (tile.rect.height - d) / 2,
            width: d,
            height: d,
        };
        let (cx, cy, rad, fg) = self.draw_circle_badge(
            circle,
            ControlVisualState {
                active: charging,
                enabled: true,
                pointer: PointerState::Normal,
            },
            accent,
            palette,
        );
        self.draw_battery_glyph(
            cx,
            cy,
            rad * 0.62,
            snapshot.status.battery.fill_fraction(),
            fg,
        );
        let headline = if snapshot.status.battery.percent.is_some() {
            snapshot.status.battery.label()
        } else {
            String::from("AC Power")
        };
        let sub = if charging {
            "Charging"
        } else {
            power_mode_label(snapshot.status.power_mode)
        };
        let text_left = circle.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = tile.rect.right() as f32 - scale(12, self.dpi) as f32;
        let mid = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            &headline,
            sub,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            true,
        );
    }

    pub(super) fn draw_action_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let target = ControlCenterHitTarget::TileAction(tile.id);
        let hovered = state.hovered == Some(target);
        let pressed = state.pressed == Some(target);
        let (active, accent) = match tile.kind {
            ControlKind::Mute => (snapshot.status.muted, SYSTEM_RED_LIGHT),
            ControlKind::Appearance => {
                (snapshot.theme == ResolvedTheme::Dark, palette.toggle_active)
            }
            _ => (false, palette.toggle_active),
        };
        if tile.cols == 1 && tile.rows == 1 {
            self.draw_compact_toggle_tile(
                tile.rect,
                radius,
                (active, hovered, pressed),
                accent,
                palette,
                |cx, cy, half, brush| self.draw_control_kind_glyph(tile.kind, cx, cy, half, brush),
            );
            return;
        }
        self.draw_tile_surface(tile.rect, radius, hovered, pressed, palette);
        let pad = scale(12, self.dpi);
        let d = scale(32, self.dpi);
        let circle = Rect {
            x: tile.rect.x + pad,
            y: tile.rect.y + (tile.rect.height - d) / 2,
            width: d,
            height: d,
        };
        let (cx, cy, rad, fg) = self.draw_circle_badge(
            circle,
            ControlVisualState {
                active,
                enabled: true,
                pointer: PointerState::Normal,
            },
            accent,
            palette,
        );
        self.draw_control_kind_glyph(tile.kind, cx, cy, rad * 0.58, fg);
        let sub = match tile.kind {
            ControlKind::Mute => {
                if snapshot.status.muted {
                    "Muted"
                } else {
                    "Active"
                }
            }
            ControlKind::Appearance => {
                if snapshot.theme == ResolvedTheme::Dark {
                    "Dark"
                } else {
                    "Light"
                }
            }
            _ => tile.kind.subtitle(),
        };
        let text_left = circle.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = tile.rect.right() as f32 - scale(12, self.dpi) as f32;
        let mid = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            tile.kind.label(),
            sub,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            false,
        );
    }

    pub(super) fn draw_compact_toggle_tile(
        &self,
        rect: Rect,
        radius: f32,
        flags: (bool, bool, bool),
        accent: D2D1_COLOR_F,
        palette: &LiquidGlassPalette,
        glyph: impl FnOnce(f32, f32, f32, &windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush),
    ) {
        let (active, hovered, pressed) = flags;
        if active {
            let d2d_r = rect_to_d2d(rect);
            let bg = self.brushes.color_brush(accent);
            let b: &ID2D1Brush = (&bg).into();
            fill_squircle_or_rounded_f(&self.factory, &self.target, d2d_r, radius, self.dpi, b);
            self.draw_interaction_overlay(rect, radius, hovered, pressed, palette);
        } else {
            self.draw_tile_surface(rect, radius, hovered, pressed, palette);
        }
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let half = scale(12, self.dpi) as f32;
        let fg = if active {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        glyph(cx, cy, half, fg);
    }
}
