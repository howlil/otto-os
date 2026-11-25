use otto_core::{
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot, ControlKind,
    ControlTileLayout, Rect,
};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::tokens::SYSTEM_RED;
use crate::kit::draw::{fill_ellipse_on, fill_squircle_or_rounded_f, stroke_squircle_or_rounded_f};
use crate::{LiquidGlassPalette, draw_wifi_glyph, rect_to_d2d, scale};

use super::{
    ControlCenterPaintState, ControlCenterRenderer, ControlVisualState, PointerState,
    connectivity_subtitle,
};

impl ControlCenterRenderer {
    pub(super) fn draw_modular_tiles(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        for tile in &layout.tiles {
            if state.dragging_tile == Some(tile.id) {
                continue;
            }
            self.draw_single_tile(tile, snapshot, state, palette)?;
            if state.editing {
                self.draw_tile_edit_chrome(tile, state, palette);
            }
        }
        Ok(())
    }

    fn draw_single_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let radius = tile.radius as f32;
        match tile.kind {
            ControlKind::Connectivity => {
                self.draw_connectivity_tile(tile, snapshot, state, radius, palette)
            }
            ControlKind::Wifi => self.draw_wifi_tile(tile, snapshot, state, radius, palette),
            ControlKind::Bluetooth => {
                self.draw_bluetooth_tile(tile, snapshot, state, radius, palette)
            }
            ControlKind::NowPlaying => {
                self.draw_now_playing_tile(tile, snapshot, state, radius, palette)
            }
            ControlKind::Brightness => {
                self.draw_brightness_tile(tile, snapshot, state, radius, palette);
                Ok(())
            }
            ControlKind::Volume => {
                self.draw_volume_tile(tile, snapshot, state, radius, palette);
                Ok(())
            }
            ControlKind::Focus => self.draw_focus_grid_tile(tile, snapshot, state, radius, palette),
            ControlKind::Battery => {
                self.draw_battery_grid_tile(tile, snapshot, state, radius, palette);
                Ok(())
            }
            _ => {
                self.draw_action_tile(tile, snapshot, state, radius, palette);
                Ok(())
            }
        }
    }

    pub(super) fn draw_tile_surface(
        &self,
        rect: Rect,
        radius: f32,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_r = rect_to_d2d(rect);
        let fill_color = if palette.is_high_contrast {
            palette.card_fill
        } else if palette.is_dark {
            D2D1_COLOR_F {
                r: 0.14,
                g: 0.16,
                b: 0.21,
                a: 0.62,
            }
        } else {
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.68,
            }
        };
        let fill = self.brushes.color_brush(fill_color);
        fill_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            d2d_r,
            radius,
            self.dpi,
            (&fill).into(),
        );
        self.draw_interaction_overlay(rect, radius, hovered, pressed, palette);
        let rim_color = if palette.is_high_contrast {
            palette.menu_outer_border
        } else if palette.is_dark {
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.20,
            }
        } else {
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.78,
            }
        };
        let rim = self.brushes.color_brush(rim_color);
        stroke_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            d2d_r,
            radius,
            self.dpi,
            (&rim).into(),
            1.0,
        );
    }

    fn draw_tile_edit_chrome(
        &self,
        tile: &ControlTileLayout,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let badge = tile.remove_badge;
        let bx = badge.x as f32 + badge.width as f32 * 0.5;
        let by = badge.y as f32 + badge.height as f32 * 0.5;
        let br = badge.width as f32 * 0.5;
        let hovered = state.hovered == Some(ControlCenterHitTarget::TileRemove(tile.id));
        let bg = self.brushes.color_brush(if hovered {
            SYSTEM_RED
        } else if palette.is_dark {
            D2D1_COLOR_F {
                r: 0.36,
                g: 0.38,
                b: 0.42,
                a: 0.94,
            }
        } else {
            D2D1_COLOR_F {
                r: 0.92,
                g: 0.93,
                b: 0.95,
                a: 0.96,
            }
        });
        fill_ellipse_on(&self.target, Vector2::new(bx, by), br, br, (&bg).into());
        let minus_fg = if hovered || palette.is_dark {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        self.draw_minus_glyph(bx, by, br * 0.48, minus_fg);

        if tile.kind.supported_sizes().len() > 1 {
            let resize_hovered = state.hovered == Some(ControlCenterHitTarget::TileResize(tile.id));
            let handle_brush = if resize_hovered {
                self.brushes.color_brush(palette.toggle_active)
            } else {
                self.brushes.text_primary.clone()
            };
            self.draw_resize_corner_glyph(
                tile.rect.right() as f32 - scale(4, self.dpi) as f32,
                tile.rect.bottom() as f32 - scale(4, self.dpi) as f32,
                scale(14, self.dpi) as f32,
                &handle_brush,
            );
        }
    }

    fn draw_connectivity_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_tile_surface(tile.rect, radius, false, false, palette);
        if tile.cols == 1 && tile.rows == 1 {
            let cx = tile.rect.x as f32 + tile.rect.width as f32 * 0.5;
            let cy = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
            let active = snapshot.wifi.enabled || snapshot.bluetooth.enabled;
            let brush = if active {
                self.brushes.color_brush(palette.toggle_active)
            } else {
                self.brushes.text_primary.clone()
            };
            return draw_wifi_glyph(
                &self.factory,
                &self.target,
                &self.stroke_style,
                cx,
                cy,
                scale(13, self.dpi) as f32,
                scale(2, self.dpi) as f32,
                &brush,
            );
        }
        if tile.rows == 1 {
            return self.draw_connectivity_compact_row(tile, snapshot, state, palette);
        }

        let row_radius = scale(10, self.dpi) as f32;
        self.draw_connectivity_row_hover(
            tile.secondary_rect,
            row_radius,
            ControlCenterHitTarget::WifiOpenPage,
            state,
            palette,
        );
        self.draw_connectivity_row(
            tile.primary_rect,
            tile.secondary_rect,
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
        self.draw_connectivity_row_hover(
            tile.detail_rect,
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
            tile.tertiary_rect,
            tile.detail_rect,
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

    fn draw_connectivity_compact_row(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let gap = scale(6, self.dpi);
        let d = (tile.rect.height - gap * 2).min(scale(42, self.dpi));
        let w_rect = Rect {
            x: tile.primary_rect.x + (tile.primary_rect.width - d) / 2,
            y: tile.primary_rect.y + (tile.primary_rect.height - d) / 2,
            width: d,
            height: d,
        };
        let b_rect = Rect {
            x: tile.tertiary_rect.x + (tile.tertiary_rect.width - d) / 2,
            y: tile.tertiary_rect.y + (tile.tertiary_rect.height - d) / 2,
            width: d,
            height: d,
        };
        let (wx, wy, wr, wb) = self.draw_circle_badge(
            w_rect,
            ControlVisualState {
                active: snapshot.wifi.supported && snapshot.wifi.enabled,
                enabled: snapshot.wifi.supported,
                pointer: PointerState::from_flags(
                    state.hovered == Some(ControlCenterHitTarget::WifiToggle),
                    state.pressed == Some(ControlCenterHitTarget::WifiToggle),
                ),
            },
            palette.toggle_active,
            palette,
        );
        draw_wifi_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            wx,
            wy,
            wr * 0.52,
            (wr * 0.14).max(1.25),
            wb,
        )?;
        let (bx, by, br, bb) = self.draw_circle_badge(
            b_rect,
            ControlVisualState {
                active: snapshot.bluetooth.supported && snapshot.bluetooth.enabled,
                enabled: snapshot.bluetooth.supported,
                pointer: PointerState::from_flags(
                    state.hovered == Some(ControlCenterHitTarget::BluetoothToggle),
                    state.pressed == Some(ControlCenterHitTarget::BluetoothToggle),
                ),
            },
            palette.toggle_active,
            palette,
        );
        self.draw_bluetooth_glyph(bx, by, br * 0.45, bb)
    }

    fn draw_wifi_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let active = snapshot.wifi.supported && snapshot.wifi.enabled;
        let hovered = state.hovered == Some(ControlCenterHitTarget::WifiToggle)
            || state.hovered == Some(ControlCenterHitTarget::WifiOpenPage);
        let pressed = state.pressed == Some(ControlCenterHitTarget::WifiToggle)
            || state.pressed == Some(ControlCenterHitTarget::WifiOpenPage);
        if tile.cols == 1 && tile.rows == 1 {
            self.draw_compact_toggle_tile(
                tile.rect,
                radius,
                (active, hovered, pressed),
                palette.toggle_active,
                palette,
                |cx, cy, half, brush| {
                    let _ = draw_wifi_glyph(
                        &self.factory,
                        &self.target,
                        &self.stroke_style,
                        cx,
                        cy,
                        half * 1.1,
                        (half * 0.22).max(1.3),
                        brush,
                    );
                },
            );
            return Ok(());
        }
        self.draw_tile_surface(tile.rect, radius, hovered, pressed, palette);
        let (cx, cy, rad, fg) = self.draw_circle_badge(
            tile.primary_rect,
            ControlVisualState {
                active,
                enabled: snapshot.wifi.supported,
                pointer: PointerState::Normal,
            },
            palette.toggle_active,
            palette,
        );
        draw_wifi_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx,
            cy,
            rad * 0.56,
            (rad * 0.14).max(1.25),
            fg,
        )?;
        let text_left = tile.primary_rect.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = tile.rect.right() as f32 - scale(12, self.dpi) as f32;
        let mid = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
        let sub = connectivity_subtitle(
            snapshot.wifi.supported,
            snapshot.wifi.enabled,
            snapshot.wifi.connected_ssid.as_deref(),
        );
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            "Wi-Fi",
            sub,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            false,
        );
        Ok(())
    }

    fn draw_bluetooth_tile(
        &self,
        tile: &ControlTileLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let active = snapshot.bluetooth.supported && snapshot.bluetooth.enabled;
        let hovered = state.hovered == Some(ControlCenterHitTarget::BluetoothToggle)
            || state.hovered == Some(ControlCenterHitTarget::BluetoothOpenPage);
        let pressed = state.pressed == Some(ControlCenterHitTarget::BluetoothToggle)
            || state.pressed == Some(ControlCenterHitTarget::BluetoothOpenPage);
        if tile.cols == 1 && tile.rows == 1 {
            self.draw_compact_toggle_tile(
                tile.rect,
                radius,
                (active, hovered, pressed),
                palette.toggle_active,
                palette,
                |cx, cy, half, brush| {
                    let _ = self.draw_bluetooth_glyph(cx, cy, half * 0.9, brush);
                },
            );
            return Ok(());
        }
        self.draw_tile_surface(tile.rect, radius, hovered, pressed, palette);
        let (cx, cy, rad, fg) = self.draw_circle_badge(
            tile.primary_rect,
            ControlVisualState {
                active,
                enabled: snapshot.bluetooth.supported,
                pointer: PointerState::Normal,
            },
            palette.toggle_active,
            palette,
        );
        self.draw_bluetooth_glyph(cx, cy, rad * 0.45, fg)?;
        let bt_connected = snapshot
            .bluetooth
            .devices
            .iter()
            .find(|device| device.connected)
            .map(|device| device.name.as_str());
        let text_left = tile.primary_rect.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = tile.rect.right() as f32 - scale(12, self.dpi) as f32;
        let mid = tile.rect.y as f32 + tile.rect.height as f32 * 0.5;
        let sub = connectivity_subtitle(
            snapshot.bluetooth.supported,
            snapshot.bluetooth.enabled,
            bt_connected,
        );
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            "Bluetooth",
            sub,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            false,
        );
        Ok(())
    }
}
