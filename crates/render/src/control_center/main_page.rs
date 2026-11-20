use crate::appearance::surface::fnv1a_hash;
use crate::{
    CHARGING_GREEN, CoverArt, LiquidGlassPalette, decode_image_bytes, draw_speaker_glyph,
    draw_wifi_glyph, rect_to_d2d, scale,
};
use otto_core::{
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot, Rect, focus_row_subtitle,
    power_mode_label,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_ELLIPSE,
    D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush,
};
use windows::core::Result;
use windows_numerics::Vector2;

use super::CachedCover;
use super::ControlCenterPaintState;
use super::ControlCenterRenderer;
use super::ControlVisualState;
use super::FOCUS_PURPLE;
use super::PointerState;
use super::connectivity_subtitle;

impl ControlCenterRenderer {
    pub(super) fn draw_main_page(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let radius = layout.card_radius as f32;
        self.draw_connectivity_card(layout, snapshot, state, radius, palette)?;
        self.draw_focus_tile(layout, snapshot, state, radius, palette)?;
        self.draw_battery_tile(layout, snapshot, state, radius, palette);
        self.draw_display_card(layout, snapshot, state, radius, palette);
        self.draw_sound_card(layout, snapshot, state, radius, palette)?;
        self.draw_media_card(layout, snapshot, state, radius, palette)?;
        Ok(())
    }

    pub(super) fn draw_connectivity_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(layout.connectivity_card, radius, false, false, palette);
        let row_radius = scale(9, self.dpi) as f32;

        self.draw_connectivity_row_hover(
            layout.wifi_row,
            row_radius,
            ControlCenterHitTarget::WifiOpenPage,
            state,
            palette,
        );
        self.draw_connectivity_row(
            layout.wifi_toggle,
            layout.wifi_row,
            ControlVisualState {
                active: snapshot.wifi.supported && snapshot.wifi.enabled,
                enabled: snapshot.wifi.supported,
                pointer: PointerState::from_flags(
                    state.hovered == Some(ControlCenterHitTarget::WifiToggle),
                    state.pressed == Some(ControlCenterHitTarget::WifiToggle),
                ),
            },
            palette.toggle_active,
            "Wi-Fi",
            connectivity_subtitle(
                snapshot.wifi.supported,
                snapshot.wifi.enabled,
                snapshot.wifi.connected_ssid.as_deref(),
            ),
            palette,
            |cx, cy, r, brush| {
                draw_wifi_glyph(
                    &self.factory,
                    &self.target,
                    &self.stroke_style,
                    cx,
                    cy,
                    r * 0.85,
                    (r * 0.22).max(1.25),
                    brush,
                )
            },
        )?;

        self.draw_connectivity_divider(layout, palette);

        self.draw_connectivity_row_hover(
            layout.bluetooth_row,
            row_radius,
            ControlCenterHitTarget::BluetoothOpenPage,
            state,
            palette,
        );
        let bt_connected = snapshot
            .bluetooth
            .devices
            .iter()
            .find(|device| device.connected)
            .map(|device| device.name.as_str());
        self.draw_connectivity_row(
            layout.bluetooth_toggle,
            layout.bluetooth_row,
            ControlVisualState {
                active: snapshot.bluetooth.supported && snapshot.bluetooth.enabled,
                enabled: snapshot.bluetooth.supported,
                pointer: PointerState::from_flags(
                    state.hovered == Some(ControlCenterHitTarget::BluetoothToggle),
                    state.pressed == Some(ControlCenterHitTarget::BluetoothToggle),
                ),
            },
            palette.toggle_active,
            "Bluetooth",
            connectivity_subtitle(
                snapshot.bluetooth.supported,
                snapshot.bluetooth.enabled,
                bt_connected,
            ),
            palette,
            |cx, cy, r, brush| self.draw_bluetooth_glyph(cx, cy, r * 0.72, brush),
        )
    }

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

    pub(super) fn draw_connectivity_divider(
        &self,
        layout: &ControlCenterLayout,
        palette: &LiquidGlassPalette,
    ) {
        let div_y = (layout.wifi_row.bottom() + layout.bluetooth_row.y) as f32 * 0.5;
        let div_x0 = layout.wifi_toggle.right() as f32 + scale(8, self.dpi) as f32;
        let div_x1 = layout.connectivity_card.right() as f32 - scale(12, self.dpi) as f32;
        if div_x1 > div_x0 {
            let sep = self.brushes.color_brush(palette.menu_separator);
            unsafe {
                self.target.DrawLine(
                    Vector2::new(div_x0, div_y),
                    Vector2::new(div_x1, div_y),
                    &sep,
                    1.0,
                    None,
                );
            }
        }
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
        let cx = circle.x as f32 + circle.width as f32 * 0.5;
        let cy = circle.y as f32 + circle.height as f32 * 0.5;
        let rad = circle.width as f32 * 0.5;
        let bg_color = if visual.active {
            accent
        } else {
            palette.toggle_inactive
        };
        let bg = self.brushes.color_brush(bg_color);
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
                    D2D1_COLOR_F {
                        r: if palette.is_dark { 1.0 } else { 0.0 },
                        g: if palette.is_dark { 1.0 } else { 0.0 },
                        b: if palette.is_dark { 1.0 } else { 0.0 },
                        a: 0.18,
                    }
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
        glyph(cx, cy, rad * 0.58, glyph_brush)?;

        let text_left = circle.right() as f32 + scale(8, self.dpi) as f32;
        let text_right = row.right() as f32 - scale(14, self.dpi) as f32;
        let mid = row.y as f32 + row.height as f32 * 0.5;
        let title_rect = D2D_RECT_F {
            left: text_left,
            top: row.y as f32 + scale(4, self.dpi) as f32,
            right: text_right.max(text_left + 1.0),
            bottom: mid + scale(2, self.dpi) as f32,
        };
        let sub_rect = D2D_RECT_F {
            left: text_left,
            top: mid - scale(1, self.dpi) as f32,
            right: text_right.max(text_left + 1.0),
            bottom: row.bottom() as f32 - scale(4, self.dpi) as f32,
        };
        self.draw_text(
            title,
            &self.body_format,
            title_rect,
            &self.brushes.text_primary,
        );
        self.draw_text(
            subtitle,
            &self.subtitle_format,
            sub_rect,
            &self.brushes.text_muted,
        );

        self.draw_chevron(
            row.right() as f32 - scale(8, self.dpi) as f32,
            mid,
            scale(4, self.dpi) as f32,
            true,
            &self.brushes.text_muted,
        );
        Ok(())
    }

    pub(super) fn draw_focus_tile(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let tile = layout.focus_toggle;
        self.draw_card(
            tile,
            radius,
            state.hovered == Some(ControlCenterHitTarget::FocusToggle),
            state.pressed == Some(ControlCenterHitTarget::FocusToggle),
            palette,
        );
        let pad = scale(8, self.dpi) as f32;
        let circle_d = scale(24, self.dpi) as f32;
        let cx = tile.x as f32 + pad + circle_d * 0.5;
        let cy = tile.y as f32 + pad + circle_d * 0.5;
        let rad = circle_d * 0.5;
        let focus_available =
            snapshot.settings.plugins.notch && snapshot.settings.system.focus_module;
        let active = focus_available && snapshot.focus_active;
        let bg = self.brushes.color_brush(if active {
            FOCUS_PURPLE
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
        }
        let moon_fg = if active {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        crate::draw_crescent_moon(
            &self.factory,
            &self.target,
            Vector2::new(cx, cy),
            rad * 0.58,
            moon_fg,
        )?;

        let gear_btn = layout.focus_settings;
        let dnd_active = snapshot.status.dnd_active == Some(true);
        let gx = gear_btn.x as f32 + gear_btn.width as f32 * 0.5;
        let gy = gear_btn.y as f32 + gear_btn.height as f32 * 0.5;
        let gr = gear_btn.width as f32 * 0.5;
        if dnd_active {
            let tint = self.brushes.color_brush(D2D1_COLOR_F {
                a: 0.28,
                ..FOCUS_PURPLE
            });
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(gx, gy),
                        radiusX: gr,
                        radiusY: gr,
                    },
                    &tint,
                );
            }
        }
        self.draw_interaction_overlay(
            gear_btn,
            gr,
            state.hovered == Some(ControlCenterHitTarget::FocusOpenSettings),
            state.pressed == Some(ControlCenterHitTarget::FocusOpenSettings),
            palette,
        );
        let gear_purple = dnd_active.then(|| self.brushes.color_brush(FOCUS_PURPLE));
        let gear_fg = gear_purple.as_ref().unwrap_or(&self.brushes.text_muted);
        self.draw_more_glyph(gx, gy, scale(5, self.dpi) as f32, gear_fg);

        let text_top = tile.y as f32 + scale(35, self.dpi) as f32;
        let sub_top = tile.y as f32 + scale(49, self.dpi) as f32;
        let title_r = D2D_RECT_F {
            left: tile.x as f32 + pad,
            top: text_top,
            right: tile.right() as f32 - pad,
            bottom: sub_top + scale(2, self.dpi) as f32,
        };
        let sub_r = D2D_RECT_F {
            left: tile.x as f32 + pad,
            top: sub_top,
            right: tile.right() as f32 - pad,
            bottom: tile.bottom() as f32 - scale(5, self.dpi) as f32,
        };
        let sub = focus_row_subtitle(focus_available, snapshot.focus_active);
        self.draw_text(
            "Focus",
            &self.body_format,
            title_r,
            &self.brushes.text_primary,
        );
        self.draw_text(sub, &self.subtitle_format, sub_r, &self.brushes.text_muted);
        Ok(())
    }

    pub(super) fn draw_battery_tile(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let tile = layout.battery_tile;
        self.draw_card(
            tile,
            radius,
            state.hovered == Some(ControlCenterHitTarget::BatteryTile),
            state.pressed == Some(ControlCenterHitTarget::BatteryTile),
            palette,
        );
        let pad = scale(8, self.dpi) as f32;
        let circle_d = scale(24, self.dpi) as f32;
        let cx = tile.x as f32 + pad + circle_d * 0.5;
        let cy = tile.y as f32 + pad + circle_d * 0.5;
        let rad = circle_d * 0.5;
        let charging = snapshot.status.battery.charging;
        let bg = self.brushes.color_brush(if charging {
            CHARGING_GREEN
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
        }
        let fg = if charging {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        self.draw_battery_glyph(
            cx,
            cy,
            rad * 0.78,
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
        let text_top = tile.y as f32 + scale(35, self.dpi) as f32;
        let sub_top = tile.y as f32 + scale(49, self.dpi) as f32;
        let title_r = D2D_RECT_F {
            left: tile.x as f32 + pad,
            top: text_top,
            right: tile.right() as f32 - pad,
            bottom: sub_top + scale(2, self.dpi) as f32,
        };
        let sub_r = D2D_RECT_F {
            left: tile.x as f32 + pad,
            top: sub_top,
            right: tile.right() as f32 - pad,
            bottom: tile.bottom() as f32 - scale(5, self.dpi) as f32,
        };
        self.draw_text(
            &headline,
            &self.body_format,
            title_r,
            &self.brushes.text_primary,
        );
        self.draw_text(sub, &self.subtitle_format, sub_r, &self.brushes.text_muted);
    }

    pub(super) fn draw_display_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        self.draw_card(layout.display_card, radius, false, false, palette);
        if state.hovered == Some(ControlCenterHitTarget::DisplayOpenPage)
            || state.pressed == Some(ControlCenterHitTarget::DisplayOpenPage)
        {
            self.draw_hover_pill(
                layout.display_header,
                scale(7, self.dpi) as f32,
                state.pressed == Some(ControlCenterHitTarget::DisplayOpenPage),
                palette,
            );
        }
        let pad = scale(10, self.dpi) as f32;
        let hdr = layout.display_header;
        let value_text = snapshot
            .status
            .brightness
            .map_or_else(|| String::from("Unavailable"), |pct| format!("{pct}%"));
        let left_r = D2D_RECT_F {
            left: hdr.x as f32 + pad,
            top: hdr.y as f32 + scale(4, self.dpi) as f32,
            right: hdr.right() as f32 - scale(90, self.dpi) as f32,
            bottom: hdr.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: hdr.right() as f32 - scale(100, self.dpi) as f32,
            top: hdr.y as f32 + scale(4, self.dpi) as f32,
            right: hdr.right() as f32 - scale(20, self.dpi) as f32,
            bottom: hdr.bottom() as f32,
        };
        self.draw_text(
            "Display",
            &self.title_format,
            left_r,
            &self.brushes.text_primary,
        );
        self.draw_text(
            &value_text,
            &self.value_format,
            right_r,
            &self.brushes.text_muted,
        );
        self.draw_chevron(
            hdr.right() as f32 - scale(12, self.dpi) as f32,
            hdr.y as f32 + hdr.height as f32 * 0.5 + scale(2, self.dpi) as f32,
            scale(4, self.dpi) as f32,
            true,
            &self.brushes.text_muted,
        );

        self.draw_capsule_slider(
            layout.display_slider,
            snapshot.status.brightness,
            state.hovered == Some(ControlCenterHitTarget::DisplaySlider),
            state.pressed == Some(ControlCenterHitTarget::DisplaySlider),
            true,
            palette,
        );
    }

    pub(super) fn draw_sound_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(layout.sound_card, radius, false, false, palette);
        if state.hovered == Some(ControlCenterHitTarget::SoundOpenPage)
            || state.pressed == Some(ControlCenterHitTarget::SoundOpenPage)
        {
            self.draw_hover_pill(
                layout.sound_header,
                scale(7, self.dpi) as f32,
                state.pressed == Some(ControlCenterHitTarget::SoundOpenPage),
                palette,
            );
        }
        let pad = scale(10, self.dpi) as f32;
        let hdr = layout.sound_header;
        let output_label = snapshot
            .audio_outputs
            .iter()
            .find(|dev| dev.is_default)
            .map_or_else(|| String::from("Output"), |dev| dev.name.clone());
        let value_text = if snapshot.status.muted || snapshot.status.volume == 0 {
            String::from("Muted")
        } else {
            format!("{}%", snapshot.status.volume)
        };
        let right_label = format!("{output_label} · {value_text}");
        let left_r = D2D_RECT_F {
            left: hdr.x as f32 + pad,
            top: hdr.y as f32 + scale(4, self.dpi) as f32,
            right: hdr.x as f32 + scale(72, self.dpi) as f32,
            bottom: hdr.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: hdr.x as f32 + scale(74, self.dpi) as f32,
            top: hdr.y as f32 + scale(4, self.dpi) as f32,
            right: hdr.right() as f32 - scale(20, self.dpi) as f32,
            bottom: hdr.bottom() as f32,
        };
        self.draw_text(
            "Sound",
            &self.title_format,
            left_r,
            &self.brushes.text_primary,
        );
        self.draw_text(
            &right_label,
            &self.value_format,
            right_r,
            &self.brushes.text_muted,
        );
        self.draw_chevron(
            hdr.right() as f32 - scale(12, self.dpi) as f32,
            hdr.y as f32 + hdr.height as f32 * 0.5 + scale(2, self.dpi) as f32,
            scale(4, self.dpi) as f32,
            true,
            &self.brushes.text_muted,
        );

        self.draw_sound_controls(layout, snapshot, state, palette)
    }

    pub(super) fn draw_sound_controls(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let mute = layout.sound_mute;
        let mute_r = mute.height as f32 * 0.5;
        let muted = snapshot.status.muted;
        let mute_rr = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(mute),
            radiusX: mute_r,
            radiusY: mute_r,
        };
        let mute_bg = self.brushes.color_brush(if muted {
            D2D1_COLOR_F {
                r: 0.94,
                g: 0.28,
                b: 0.24,
                a: if palette.is_high_contrast { 1.0 } else { 0.24 },
            }
        } else {
            palette.toggle_inactive
        });
        unsafe {
            self.target.FillRoundedRectangle(&mute_rr, &mute_bg);
            let border = self.brushes.color_brush(if muted {
                D2D1_COLOR_F {
                    r: 0.98,
                    g: 0.42,
                    b: 0.36,
                    a: 0.65,
                }
            } else {
                palette.menu_separator
            });
            self.target
                .DrawRoundedRectangle(&mute_rr, &border, 0.8, None);
        }
        self.draw_interaction_overlay(
            mute,
            mute_r,
            state.hovered == Some(ControlCenterHitTarget::SoundMuteToggle),
            state.pressed == Some(ControlCenterHitTarget::SoundMuteToggle),
            palette,
        );
        let cx = mute.x as f32 + mute.width as f32 * 0.5;
        let cy = mute.y as f32 + mute.height as f32 * 0.5;
        let muted_red = self.brushes.color_brush(D2D1_COLOR_F {
            r: 1.0,
            g: 0.45,
            b: 0.40,
            a: 0.98,
        });
        let icon_brush = if muted && !palette.is_high_contrast {
            &muted_red
        } else if muted {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        let half_h = scale(4, self.dpi) as f32;
        draw_speaker_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx - half_h * 0.72,
            cy,
            half_h,
            (scale(2, self.dpi) as f32 * 0.6).max(1.1),
            snapshot.status.volume,
            muted,
            icon_brush,
        )?;

        let effective = if muted { 0 } else { snapshot.status.volume };
        self.draw_capsule_slider(
            layout.sound_slider,
            Some(effective),
            state.hovered == Some(ControlCenterHitTarget::SoundSlider),
            state.pressed == Some(ControlCenterHitTarget::SoundSlider),
            false,
            palette,
        );
        Ok(())
    }

    pub(super) fn draw_media_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(layout.media_card, radius, false, false, palette);
        let media_enabled = snapshot.settings.system.media_module;
        if media_enabled
            && (state.hovered == Some(ControlCenterHitTarget::MediaBody)
                || state.pressed == Some(ControlCenterHitTarget::MediaBody))
        {
            self.draw_hover_pill(
                layout.media_body,
                scale(10, self.dpi) as f32,
                state.pressed == Some(ControlCenterHitTarget::MediaBody),
                palette,
            );
        }
        let pad = scale(12, self.dpi) as f32;
        let art_size = scale(48, self.dpi) as f32;
        let art_top =
            layout.media_body.y as f32 + (layout.media_body.height as f32 - art_size) * 0.5;
        let art_rect = D2D_RECT_F {
            left: layout.media_body.x as f32 + pad,
            top: art_top,
            right: layout.media_body.x as f32 + pad + art_size,
            bottom: art_top + art_size,
        };
        let thumb_bytes = if media_enabled {
            snapshot.media.as_ref().and_then(|m| m.thumbnail.as_deref())
        } else {
            None
        };
        self.draw_cover_thumb(art_rect, scale(8, self.dpi) as f32, thumb_bytes, palette);

        let text_left = art_rect.right + scale(10, self.dpi) as f32;
        let text_right =
            (layout.media_body.right() as f32 - scale(6, self.dpi) as f32).max(text_left + 1.0);
        let title_r = D2D_RECT_F {
            left: text_left,
            top: art_top + scale(4, self.dpi) as f32,
            right: text_right,
            bottom: art_top + art_size * 0.54,
        };
        let artist_r = D2D_RECT_F {
            left: text_left,
            top: art_top + art_size * 0.48,
            right: text_right,
            bottom: art_rect.bottom - scale(2, self.dpi) as f32,
        };
        let (title, artist, playing) = if media_enabled {
            match &snapshot.media {
                Some(media) => {
                    let artist = if media.artist.is_empty() {
                        "Playing"
                    } else {
                        media.artist.as_str()
                    };
                    (media.title.as_str(), artist, media.playing)
                }
                None => ("Not Playing", "Open Media", false),
            }
        } else {
            ("Now Playing Off", "Enable in Modules & Settings", false)
        };
        let title_brush = if media_enabled {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_muted
        };
        let sub_brush = if media_enabled {
            &self.brushes.text_muted
        } else {
            &self.brushes.text_disabled
        };
        self.draw_text(title, &self.body_format, title_r, title_brush);
        self.draw_text(artist, &self.subtitle_format, artist_r, sub_brush);

        self.draw_media_transport_buttons(
            layout,
            media_enabled,
            snapshot.media.is_some(),
            playing,
            state,
            palette,
        )
    }

    pub(super) fn draw_media_transport_buttons(
        &self,
        layout: &ControlCenterLayout,
        media_enabled: bool,
        has_media: bool,
        playing: bool,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let controls_active = media_enabled && has_media;
        for (rect, target, kind) in [
            (layout.media_prev, ControlCenterHitTarget::MediaPrev, -1),
            (
                layout.media_play_pause,
                ControlCenterHitTarget::MediaPlayPause,
                0,
            ),
            (layout.media_next, ControlCenterHitTarget::MediaNext, 1),
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
                unsafe {
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(cx, cy),
                            radiusX: btn_r,
                            radiusY: btn_r,
                        },
                        &bg,
                    );
                }
            }
            if controls_active {
                self.draw_interaction_overlay(
                    rect,
                    btn_r,
                    state.hovered == Some(target),
                    state.pressed == Some(target),
                    palette,
                );
            }
            let glyph_brush = if !controls_active {
                &self.brushes.text_disabled
            } else if kind == 0 {
                &self.brushes.on_accent
            } else {
                &self.brushes.text_primary
            };
            let size = scale(if kind == 0 { 5 } else { 4 }, self.dpi) as f32;
            match kind {
                -1 => self.draw_skip_glyph(cx, cy, size, false, glyph_brush)?,
                0 => self.draw_play_pause_glyph(cx, cy, size, playing, glyph_brush)?,
                _ => self.draw_skip_glyph(cx, cy, size, true, glyph_brush)?,
            }
        }
        Ok(())
    }

    pub(super) fn draw_cover_thumb(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        bytes: Option<&[u8]>,
        palette: &LiquidGlassPalette,
    ) {
        if let Some(raw) = bytes
            && let Some(art) = self.cached_or_decode_cover(raw)
            && let Ok(clip) = unsafe {
                self.factory
                    .CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                        rect,
                        radiusX: radius,
                        radiusY: radius,
                    })
            }
            && let Ok(layer) = unsafe { self.target.CreateLayer(None) }
        {
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: rect,
                geometricMask: std::mem::ManuallyDrop::new(Some(clip.into())),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: windows_numerics::Matrix3x2::identity(),
                opacity: 1.0,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            unsafe {
                self.target.PushLayer(&params, &layer);
                self.target.DrawBitmap(
                    &art.bitmap,
                    Some(&rect),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
                self.target.PopLayer();
            }
            std::mem::ManuallyDrop::into_inner(params.geometricMask);
            return;
        }
        let tile = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        let fill = self.brushes.color_brush(palette.toggle_inactive);
        unsafe {
            self.target.FillRoundedRectangle(&tile, &fill);
        }
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
            && cached.len == bytes.len()
        {
            return Some(cached.art.clone());
        }
        let art = decode_image_bytes(&self.target, bytes).ok()?;
        *self.cached_cover.borrow_mut() = Some(CachedCover {
            hash,
            len: bytes.len(),
            art: art.clone(),
        });
        Some(art)
    }
}
