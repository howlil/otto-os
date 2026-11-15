use std::cell::RefCell;
use std::f32::consts::PI;

use super::glass::GlassPainter;
use super::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery, fnv1a_hash,
};
use super::{
    BrushPalette, CHARGING_GREEN, CLEAR, CoverArt, LiquidGlassPalette, create_text_format,
    decode_image_bytes, draw_speaker_glyph, draw_wifi_glyph, rect_to_d2d, scale,
    shared_d2d_factory, shared_dwrite_factory,
};
use otto_core::{
    Appearance, BluetoothDevice, BluetoothDeviceKind, ControlCenterHitTarget, ControlCenterLayout,
    ControlCenterPage, ControlCenterSnapshot, GlassShape, OttoSettingItem, OttoSettingsCategory,
    Rect, RgbBuffer, ShapeTokens, ThemeMode, WifiNetwork, focus_row_subtitle, power_mode_label,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_CAP_STYLE_ROUND,
    D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
    D2D1_ELLIPSE, D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS, D2D1_LINE_JOIN_ROUND,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, ID2D1DCRenderTarget, ID2D1Factory,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::Vector2;

const FOCUS_PURPLE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.43,
    g: 0.36,
    b: 0.96,
    a: 1.0,
};

struct CachedCover {
    hash: u64,
    len: usize,
    art: CoverArt,
}

pub struct ControlCenterRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    brushes: BrushPalette,
    surface: DibSurface,
    title_format: IDWriteTextFormat,
    body_format: IDWriteTextFormat,
    subtitle_format: IDWriteTextFormat,
    value_format: IDWriteTextFormat,
    center_format: IDWriteTextFormat,
    dpi: u32,
    cached_cover: RefCell<Option<CachedCover>>,
    glass: GlassPainter,
}

impl ControlCenterRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        let dpi = dpi.max(96);
        let factory = shared_d2d_factory()?;
        let dwrite = shared_dwrite_factory()?;
        unsafe {
            let target = create_dc_render_target(&factory)?;
            let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 2.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let stroke_style = factory.CreateStrokeStyle(&stroke_props, None)?;
            let brushes = BrushPalette::new(&target)?;
            let scale_f = dpi as f32 / 96.0;
            let title_format = create_text_format(
                &dwrite,
                13.0 * scale_f,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let body_format = create_text_format(
                &dwrite,
                12.0 * scale_f,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let subtitle_format = create_text_format(
                &dwrite,
                10.5 * scale_f,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let value_format = create_text_format(
                &dwrite,
                11.0 * scale_f,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;
            let center_format = create_text_format(
                &dwrite,
                12.0 * scale_f,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let surface = DibSurface::new(scale(360, dpi), scale(640, dpi))?;
            Ok(Self {
                factory,
                target,
                stroke_style,
                brushes,
                surface,
                title_format,
                body_format,
                subtitle_format,
                value_format,
                center_format,
                dpi,
                cached_cover: RefCell::new(None),
                glass: GlassPainter::default(),
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn set_pointer(&self, pointer: Option<(f32, f32)>) {
        self.glass.set_pointer(pointer);
    }

    pub fn clear_backdrop(&self) {
        self.glass.clear_backdrop();
    }

    #[must_use]
    pub fn has_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.cached_cover.borrow_mut().take();
        self.glass.invalidate_gpu();
        Ok(())
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        focused: Option<ControlCenterHitTarget>,
        appearance: &Appearance,
    ) -> Result<()> {
        draw_with_recovery(
            self,
            |renderer| renderer.draw_to_surface(layout, snapshot, hovered, focused, appearance),
            Self::recreate_target,
        )?;
        self.surface.present_sized(
            hwnd,
            (layout.window.x, layout.window.y),
            (layout.window.width, layout.window.height),
        )
    }

    pub fn draw_to_surface(
        &mut self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        focused: Option<ControlCenterHitTarget>,
        appearance: &Appearance,
    ) -> Result<()> {
        let w = layout.window.width.max(1);
        let h = layout.window.height.max(1);
        let (sw, sh) = self.surface.size();
        if w > sw || h > sh {
            self.surface = DibSurface::new(w.max(sw), h.max(sh))?;
        }
        let palette = LiquidGlassPalette::for_appearance(appearance);
        self.brushes.update_theme(&palette);

        let target = self.target.clone();
        draw_on(&target, &self.surface, (w, h), || {
            self.draw_shell(w, h, layout.shell_radius as f32, appearance, &palette)?;
            match layout.page {
                ControlCenterPage::Main => {
                    self.draw_main_page(layout, snapshot, hovered, &palette)?;
                }
                _ => self.draw_subpanel_page(layout, snapshot, hovered, &palette)?,
            }
            self.draw_focus_ring(layout, focused);
            Ok(())
        })
    }

    fn draw_focus_ring(
        &self,
        layout: &ControlCenterLayout,
        focused: Option<ControlCenterHitTarget>,
    ) {
        let Some(target) = focused else {
            return;
        };
        let Some(slot) = layout.target_rect(target) else {
            return;
        };
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let stroke = shape.focus_ring_width as f32;
        let inset = stroke * 0.5;
        let max_radius = (slot.width.min(slot.height) as f32 * 0.5).max(1.0);
        let radius = if matches!(
            target,
            ControlCenterHitTarget::WifiToggle
                | ControlCenterHitTarget::BluetoothToggle
                | ControlCenterHitTarget::FocusToggle
                | ControlCenterHitTarget::SoundMuteToggle
                | ControlCenterHitTarget::HeaderToggle
        ) {
            max_radius
        } else if matches!(
            target,
            ControlCenterHitTarget::DisplaySlider | ControlCenterHitTarget::SoundSlider
        ) {
            (slot.height as f32 * 0.5).max(1.0)
        } else {
            (shape.focus_ring_radius as f32).min(max_radius)
        };
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: slot.x as f32 + inset,
                top: slot.y as f32 + inset,
                right: slot.right() as f32 - inset,
                bottom: slot.bottom() as f32 - inset,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.DrawRoundedRectangle(
                &rect,
                &self.brushes.text_primary,
                stroke,
                &self.stroke_style,
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn surface_pixels(&self) -> Vec<u8> {
        self.surface.read_pixels()
    }

    fn draw_shell(
        &self,
        width: i32,
        height: i32,
        radius: f32,
        appearance: &Appearance,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let transparent = CLEAR;
        unsafe {
            self.target.Clear(Some(&raw const transparent));
        }
        let shell_shape = GlassShape::from_rect(
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            radius,
        );
        if self
            .glass
            .draw(&self.target, appearance, self.dpi, shell_shape, true)?
        {
            return Ok(());
        }

        let fill = self.brushes.color_brush(palette.menu_fill);
        let outer = self.brushes.color_brush(palette.menu_outer_border);
        let rim = &self.brushes.specular_rim;

        let outer_rect = D2D1_ROUNDED_RECT {
            rect: d2d_rect_size(width, height),
            radiusX: radius,
            radiusY: radius,
        };
        let inner_rect = D2D1_ROUNDED_RECT {
            rect: d2d_rect_inset(width, height, 0.75),
            radiusX: (radius - 0.75).max(1.0),
            radiusY: (radius - 0.75).max(1.0),
        };
        unsafe {
            self.target.FillRoundedRectangle(&outer_rect, &fill);
            self.target
                .DrawRoundedRectangle(&inner_rect, rim, 1.0, None);
            self.target
                .DrawRoundedRectangle(&outer_rect, &outer, 1.0, None);
        }
        Ok(())
    }

    fn draw_card(&self, rect: Rect, radius: f32, hovered: bool, palette: &LiquidGlassPalette) {
        let r = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        let fill = self.brushes.color_brush(palette.card_fill);
        unsafe {
            self.target.FillRoundedRectangle(&r, &fill);
            if hovered {
                self.target.FillRoundedRectangle(&r, &self.brushes.hover);
            }
            if palette.is_high_contrast {
                self.target
                    .DrawRoundedRectangle(&r, &self.brushes.specular_rim, 0.85, None);
            }
        }
    }

    fn draw_hover_pill(&self, rect: Rect, radius: f32) {
        let pill = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&pill, &self.brushes.hover);
        }
    }

    fn draw_main_page(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let radius = layout.card_radius as f32;
        self.draw_connectivity_card(layout, snapshot, hovered, radius, palette)?;
        self.draw_theme_tile(layout, snapshot, hovered, radius, palette);
        self.draw_battery_tile(layout, snapshot, hovered, radius, palette);
        self.draw_display_card(layout, snapshot, hovered, radius, palette);
        self.draw_sound_card(layout, snapshot, hovered, radius, palette)?;
        self.draw_media_card(layout, snapshot, hovered, radius, palette)?;
        self.draw_otto_settings_tile(layout, snapshot, hovered, radius, palette);
        Ok(())
    }

    fn draw_connectivity_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(layout.connectivity_card, radius, false, palette);
        let row_radius = scale(8, self.dpi) as f32;

        if hovered == Some(ControlCenterHitTarget::WifiOpenPage) {
            self.draw_hover_pill(layout.wifi_row, row_radius);
        }
        let wifi_sub = if !snapshot.wifi.supported {
            "Unavailable"
        } else if snapshot.wifi.enabled {
            snapshot.wifi.connected_ssid.as_deref().unwrap_or("On")
        } else {
            "Off"
        };
        self.draw_connectivity_row(
            layout.wifi_toggle,
            layout.wifi_row,
            snapshot.wifi.supported && snapshot.wifi.enabled,
            hovered == Some(ControlCenterHitTarget::WifiToggle),
            palette.toggle_active,
            "Wi-Fi",
            wifi_sub,
            true,
            palette,
            |cx, cy, r, brush| {
                let s = (r * 0.22).max(1.25);
                draw_wifi_glyph(
                    &self.factory,
                    &self.target,
                    &self.stroke_style,
                    cx,
                    cy,
                    r * 0.85,
                    s,
                    brush,
                )
            },
        )?;

        if hovered == Some(ControlCenterHitTarget::BluetoothOpenPage) {
            self.draw_hover_pill(layout.bluetooth_row, row_radius);
        }
        let bt_connected = snapshot
            .bluetooth
            .devices
            .iter()
            .find(|device| device.connected)
            .map(|device| device.name.as_str());
        let bt_sub = if !snapshot.bluetooth.supported {
            "Unavailable"
        } else if snapshot.bluetooth.enabled {
            bt_connected.unwrap_or("On")
        } else {
            "Off"
        };
        self.draw_connectivity_row(
            layout.bluetooth_toggle,
            layout.bluetooth_row,
            snapshot.bluetooth.supported && snapshot.bluetooth.enabled,
            hovered == Some(ControlCenterHitTarget::BluetoothToggle),
            palette.toggle_active,
            "Bluetooth",
            bt_sub,
            true,
            palette,
            |cx, cy, r, brush| self.draw_bluetooth_glyph(cx, cy, r * 0.72, brush),
        )?;

        self.draw_focus_row(layout, snapshot, hovered, row_radius, palette)
    }

    fn draw_focus_row(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        row_radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        if hovered == Some(ControlCenterHitTarget::FocusToggle) {
            self.draw_hover_pill(layout.focus_toggle, row_radius);
        }
        if hovered == Some(ControlCenterHitTarget::FocusOpenSettings) {
            self.draw_hover_pill(layout.focus_settings, row_radius);
        }
        let focus_circle = Rect {
            x: layout.wifi_toggle.x,
            y: layout.focus_toggle.y + (layout.focus_toggle.height - layout.wifi_toggle.height) / 2,
            width: layout.wifi_toggle.width,
            height: layout.wifi_toggle.height,
        };
        let focus_sub =
            focus_row_subtitle(snapshot.settings.system.focus_module, snapshot.focus_active);
        self.draw_connectivity_row(
            focus_circle,
            layout.focus_toggle,
            snapshot.settings.system.focus_module && snapshot.focus_active,
            false,
            FOCUS_PURPLE,
            "Pomodoro",
            focus_sub,
            false,
            palette,
            |cx, cy, r, brush| {
                crate::draw_crescent_moon(
                    &self.factory,
                    &self.target,
                    Vector2::new(cx, cy),
                    r * 0.72,
                    brush,
                )
            },
        )?;
        let win_dnd_active = snapshot.status.dnd_active == Some(true);
        if win_dnd_active && hovered != Some(ControlCenterHitTarget::FocusOpenSettings) {
            let tint = self.brushes.color_brush(D2D1_COLOR_F {
                a: 0.24,
                ..FOCUS_PURPLE
            });
            let pill = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(layout.focus_settings),
                radiusX: row_radius,
                radiusY: row_radius,
            };
            unsafe {
                self.target.FillRoundedRectangle(&pill, &tint);
            }
        }
        let dnd_brush = if win_dnd_active {
            Some(self.brushes.color_brush(FOCUS_PURPLE))
        } else {
            None
        };
        let dnd_fg = dnd_brush.as_ref().unwrap_or(&self.brushes.text_muted);
        let moon_x = layout.focus_settings.x as f32 + layout.focus_settings.width as f32 * 0.34;
        let settings_cy =
            layout.focus_settings.y as f32 + layout.focus_settings.height as f32 * 0.5;
        crate::draw_crescent_moon(
            &self.factory,
            &self.target,
            Vector2::new(moon_x, settings_cy),
            scale(6, self.dpi) as f32,
            dnd_fg,
        )?;
        let chevron_x = layout.focus_settings.right() as f32 - scale(9, self.dpi) as f32;
        self.draw_chevron(
            chevron_x,
            settings_cy,
            scale(4, self.dpi) as f32,
            true,
            dnd_fg,
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_connectivity_row(
        &self,
        circle: Rect,
        row: Rect,
        active: bool,
        circle_hovered: bool,
        accent: D2D1_COLOR_F,
        title: &str,
        subtitle: &str,
        chevron: bool,
        palette: &LiquidGlassPalette,
        glyph: impl FnOnce(f32, f32, f32, &ID2D1SolidColorBrush) -> Result<()>,
    ) -> Result<()> {
        let cx = circle.x as f32 + circle.width as f32 * 0.5;
        let cy = circle.y as f32 + circle.height as f32 * 0.5;
        let rad = circle.width as f32 * 0.5;
        let bg_color = if active {
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
            if circle_hovered {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: rad,
                        radiusY: rad,
                    },
                    &self.brushes.hover,
                );
            }
        }
        let glyph_brush = if active {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        glyph(cx, cy, rad * 0.58, glyph_brush)?;

        let text_left = circle.right() as f32 + scale(7, self.dpi) as f32;
        let text_right = if chevron {
            row.right() as f32 - scale(14, self.dpi) as f32
        } else {
            row.right() as f32 - scale(4, self.dpi) as f32
        };
        let mid = row.y as f32 + row.height as f32 * 0.5;
        let title_rect = D2D_RECT_F {
            left: text_left,
            top: row.y as f32 + scale(2, self.dpi) as f32,
            right: text_right.max(text_left + 1.0),
            bottom: mid + scale(2, self.dpi) as f32,
        };
        let sub_rect = D2D_RECT_F {
            left: text_left,
            top: mid - scale(1, self.dpi) as f32,
            right: text_right.max(text_left + 1.0),
            bottom: row.bottom() as f32 - scale(2, self.dpi) as f32,
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

        if chevron {
            self.draw_chevron(
                row.right() as f32 - scale(8, self.dpi) as f32,
                mid,
                scale(4, self.dpi) as f32,
                true,
                &self.brushes.text_muted,
            );
        }
        Ok(())
    }

    fn draw_theme_tile(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let tile = layout.theme_tile;
        self.draw_card(
            tile,
            radius,
            hovered == Some(ControlCenterHitTarget::ThemeTile),
            palette,
        );
        let pad = scale(10, self.dpi) as f32;
        let circle_d = scale(28, self.dpi) as f32;
        let cx = tile.x as f32 + pad + circle_d * 0.5;
        let cy = tile.y as f32 + tile.height as f32 * 0.5;
        let rad = circle_d * 0.5;
        let active = snapshot.theme_mode != ThemeMode::Auto;
        let bg = self.brushes.color_brush(if active {
            palette.toggle_active
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
        let fg = if active {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        self.draw_theme_glyph(cx, cy, rad * 0.54, fg);

        let text_left = tile.x as f32 + pad + circle_d + scale(8, self.dpi) as f32;
        let mid = tile.y as f32 + tile.height as f32 * 0.5;
        let title_r = D2D_RECT_F {
            left: text_left,
            top: mid - scale(16, self.dpi) as f32,
            right: tile.right() as f32 - pad,
            bottom: mid + scale(2, self.dpi) as f32,
        };
        let sub_r = D2D_RECT_F {
            left: text_left,
            top: mid,
            right: tile.right() as f32 - pad,
            bottom: mid + scale(16, self.dpi) as f32,
        };
        self.draw_text(
            "Theme",
            &self.body_format,
            title_r,
            &self.brushes.text_primary,
        );
        self.draw_text(
            snapshot.theme_mode.label(),
            &self.subtitle_format,
            sub_r,
            &self.brushes.text_muted,
        );
    }

    fn draw_battery_tile(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let tile = layout.battery_tile;
        self.draw_card(
            tile,
            radius,
            hovered == Some(ControlCenterHitTarget::BatteryTile),
            palette,
        );
        let pad = scale(10, self.dpi) as f32;
        let circle_d = scale(28, self.dpi) as f32;
        let cx = tile.x as f32 + pad + circle_d * 0.5;
        let cy = tile.y as f32 + tile.height as f32 * 0.5;
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
            rad * 0.70,
            snapshot.status.battery.fill_fraction(),
            fg,
        );

        let text_left = tile.x as f32 + pad + circle_d + scale(8, self.dpi) as f32;
        let mid = tile.y as f32 + tile.height as f32 * 0.5;
        let headline = if snapshot.status.battery.percent.is_some() {
            format!("Battery {}", snapshot.status.battery.label())
        } else {
            String::from("AC Power")
        };
        let sub = if charging {
            "Charging"
        } else {
            power_mode_label(snapshot.status.power_mode)
        };
        let title_r = D2D_RECT_F {
            left: text_left,
            top: mid - scale(16, self.dpi) as f32,
            right: tile.right() as f32 - pad,
            bottom: mid + scale(2, self.dpi) as f32,
        };
        let sub_r = D2D_RECT_F {
            left: text_left,
            top: mid,
            right: tile.right() as f32 - pad,
            bottom: mid + scale(16, self.dpi) as f32,
        };
        self.draw_text(
            &headline,
            &self.body_format,
            title_r,
            &self.brushes.text_primary,
        );
        self.draw_text(sub, &self.subtitle_format, sub_r, &self.brushes.text_muted);
    }

    fn draw_display_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        self.draw_card(layout.display_card, radius, false, palette);
        if hovered == Some(ControlCenterHitTarget::DisplayOpenPage) {
            self.draw_hover_pill(layout.display_header, scale(6, self.dpi) as f32);
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

        self.draw_sun_icon_badge(
            layout.display_icon,
            snapshot.status.brightness.is_some(),
            palette,
        );
        self.draw_slider_pill(
            layout.display_slider,
            snapshot.status.brightness,
            hovered == Some(ControlCenterHitTarget::DisplaySlider),
            palette,
        );
    }

    fn draw_sun_icon_badge(&self, badge: Rect, available: bool, palette: &LiquidGlassPalette) {
        let cx = badge.x as f32 + badge.width as f32 * 0.5;
        let cy = badge.y as f32 + badge.height as f32 * 0.5;
        let rad = badge.width as f32 * 0.5;
        let bg = self.brushes.color_brush(if available {
            palette.toggle_inactive
        } else {
            palette.slider_track
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
        let icon_brush = if available {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_disabled
        };
        self.draw_sun_glyph(cx, cy, scale(5, self.dpi) as f32, icon_brush);
    }

    fn draw_sound_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(layout.sound_card, radius, false, palette);
        if hovered == Some(ControlCenterHitTarget::SoundOpenPage) {
            self.draw_hover_pill(layout.sound_header, scale(6, self.dpi) as f32);
        }
        let pad = scale(10, self.dpi) as f32;
        let hdr = layout.sound_header;
        let output_label = snapshot
            .audio_outputs
            .iter()
            .find(|dev| dev.is_default)
            .map_or_else(
                || String::from("Sound"),
                |dev| format!("Sound · {}", dev.name),
            );
        let value_text = if snapshot.status.muted || snapshot.status.volume == 0 {
            String::from("Muted")
        } else {
            format!("{}%", snapshot.status.volume)
        };
        let left_r = D2D_RECT_F {
            left: hdr.x as f32 + pad,
            top: hdr.y as f32 + scale(4, self.dpi) as f32,
            right: hdr.right() as f32 - scale(76, self.dpi) as f32,
            bottom: hdr.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: hdr.right() as f32 - scale(86, self.dpi) as f32,
            top: hdr.y as f32 + scale(4, self.dpi) as f32,
            right: hdr.right() as f32 - scale(20, self.dpi) as f32,
            bottom: hdr.bottom() as f32,
        };
        self.draw_text(
            &output_label,
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

        self.draw_sound_controls(layout, snapshot, hovered, palette)
    }

    fn draw_sound_controls(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let mute = layout.sound_mute;
        let cx = mute.x as f32 + mute.width as f32 * 0.5;
        let cy = mute.y as f32 + mute.height as f32 * 0.5;
        let rad = mute.width as f32 * 0.5;
        let mute_bg = self.brushes.color_brush(if snapshot.status.muted {
            palette.toggle_inactive
        } else {
            palette.toggle_active
        });
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: rad,
                    radiusY: rad,
                },
                &mute_bg,
            );
            if hovered == Some(ControlCenterHitTarget::SoundMuteToggle) {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: rad,
                        radiusY: rad,
                    },
                    &self.brushes.hover,
                );
            }
        }
        let icon_brush = if snapshot.status.muted {
            &self.brushes.text_muted
        } else {
            &self.brushes.on_accent
        };
        let half_h = scale(5, self.dpi) as f32;
        draw_speaker_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx - half_h * 0.9,
            cy,
            half_h,
            (scale(2, self.dpi) as f32 * 0.65).max(1.2),
            snapshot.status.volume,
            snapshot.status.muted,
            icon_brush,
        )?;

        let effective = if snapshot.status.muted {
            0
        } else {
            snapshot.status.volume
        };
        self.draw_slider_pill(
            layout.sound_slider,
            Some(effective),
            hovered == Some(ControlCenterHitTarget::SoundSlider),
            palette,
        );
        Ok(())
    }

    fn draw_media_card(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(layout.media_card, radius, false, palette);
        let media_enabled = snapshot.settings.system.media_module;
        if media_enabled && hovered == Some(ControlCenterHitTarget::MediaBody) {
            self.draw_hover_pill(layout.media_body, scale(10, self.dpi) as f32);
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
            hovered,
            palette,
        )
    }

    fn draw_media_transport_buttons(
        &self,
        layout: &ControlCenterLayout,
        media_enabled: bool,
        has_media: bool,
        playing: bool,
        hovered: Option<ControlCenterHitTarget>,
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
            let pill_color = if !media_enabled {
                palette.slider_track
            } else if kind == 0 && controls_active {
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
                if controls_active && hovered == Some(target) {
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(cx, cy),
                            radiusX: btn_r,
                            radiusY: btn_r,
                        },
                        &self.brushes.hover,
                    );
                }
            }
            let glyph_brush = if !controls_active {
                &self.brushes.text_disabled
            } else if kind == 0 {
                &self.brushes.on_accent
            } else {
                &self.brushes.text_primary
            };
            let size = scale(5, self.dpi) as f32;
            match kind {
                -1 => self.draw_skip_glyph(cx, cy, size, false, glyph_brush)?,
                0 => self.draw_play_pause_glyph(cx, cy, size, playing, glyph_brush)?,
                _ => self.draw_skip_glyph(cx, cy, size, true, glyph_brush)?,
            }
        }
        Ok(())
    }

    fn draw_otto_settings_tile(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let tile = layout.otto_settings_tile;
        self.draw_card(
            tile,
            radius,
            hovered == Some(ControlCenterHitTarget::OttoSettingsTile),
            palette,
        );
        let cy = tile.y as f32 + tile.height as f32 * 0.5;
        let circle_r = scale(13, self.dpi) as f32;
        let cx = tile.x as f32 + scale(10, self.dpi) as f32 + circle_r;
        let bg = self.brushes.color_brush(palette.toggle_inactive);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: circle_r,
                    radiusY: circle_r,
                },
                &bg,
            );
        }
        self.draw_gear_glyph(cx, cy, circle_r * 0.56, &self.brushes.text_primary);

        let chev_x = tile.right() as f32 - scale(12, self.dpi) as f32;
        self.draw_chevron(
            chev_x,
            cy,
            scale(4, self.dpi) as f32,
            true,
            &self.brushes.text_muted,
        );

        let (active, total) = snapshot.settings.active_module_count();
        let badge_label = format!("{active}/{total} active");
        let badge_w = scale(84, self.dpi) as f32;
        let badge_h = scale(22, self.dpi) as f32;
        let badge_right = chev_x - scale(10, self.dpi) as f32;
        let badge_left = badge_right - badge_w;
        let badge_r = Rect {
            x: badge_left.round() as i32,
            y: (cy - badge_h * 0.5).round() as i32,
            width: badge_w.round() as i32,
            height: badge_h.round() as i32,
        };
        let badge_rr = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(badge_r),
            radiusX: badge_h * 0.5,
            radiusY: badge_h * 0.5,
        };
        let badge_bg = self.brushes.color_brush(palette.toggle_inactive);
        unsafe {
            self.target.FillRoundedRectangle(&badge_rr, &badge_bg);
        }
        self.draw_text(
            &badge_label,
            &self.center_format,
            rect_to_d2d(badge_r),
            &self.brushes.text_muted,
        );

        let title_r = D2D_RECT_F {
            left: cx + circle_r + scale(10, self.dpi) as f32,
            top: tile.y as f32,
            right: (badge_left - scale(8, self.dpi) as f32)
                .max(cx + circle_r + scale(12, self.dpi) as f32),
            bottom: tile.bottom() as f32,
        };
        self.draw_text(
            "Modules & Settings",
            &self.title_format,
            title_r,
            &self.brushes.text_primary,
        );
    }

    fn draw_subpanel_page(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_subpanel_header(layout, snapshot, hovered, palette);
        match layout.page {
            ControlCenterPage::Sound => {
                self.draw_card(layout.sound_card, layout.card_radius as f32, false, palette);
                self.draw_sound_controls(layout, snapshot, hovered, palette)?;
                self.draw_scrollable_list(layout, snapshot, hovered, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    if snapshot.audio_switch_failed {
                        "Open Sound Settings..."
                    } else {
                        "Sound Settings..."
                    },
                    hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::Display => {
                self.draw_card(
                    layout.display_card,
                    layout.card_radius as f32,
                    false,
                    palette,
                );
                self.draw_sun_icon_badge(
                    layout.display_icon,
                    snapshot.status.brightness.is_some(),
                    palette,
                );
                self.draw_slider_pill(
                    layout.display_slider,
                    snapshot.status.brightness,
                    hovered == Some(ControlCenterHitTarget::DisplaySlider),
                    palette,
                );
                self.draw_theme_selector_strip(layout, snapshot, hovered, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Display Settings...",
                    hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
                self.draw_footer_button(
                    layout.footer_secondary,
                    "Night Light...",
                    hovered == Some(ControlCenterHitTarget::FooterSecondary),
                    palette,
                );
            }
            ControlCenterPage::Wifi => {
                self.draw_scrollable_list(layout, snapshot, hovered, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Wi-Fi Settings...",
                    hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::Bluetooth => {
                self.draw_scrollable_list(layout, snapshot, hovered, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Bluetooth Settings...",
                    hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::OttoSettings => {
                self.draw_category_tabs(layout, snapshot, hovered, palette);
                self.draw_scrollable_list(layout, snapshot, hovered, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Logs Folder",
                    hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
                let update_label = snapshot.update_status.as_deref().unwrap_or("Check Updates");
                self.draw_footer_button(
                    layout.footer_secondary,
                    update_label,
                    hovered == Some(ControlCenterHitTarget::FooterSecondary),
                    palette,
                );
            }
            ControlCenterPage::Main => {}
        }
        Ok(())
    }

    fn draw_subpanel_header(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) {
        let back = layout.back_button;
        let back_r = scale(8, self.dpi) as f32;
        if hovered == Some(ControlCenterHitTarget::BackButton) {
            self.draw_hover_pill(back, back_r);
        }
        self.draw_chevron(
            back.x as f32 + back.width as f32 * 0.5,
            back.y as f32 + back.height as f32 * 0.5,
            scale(5, self.dpi) as f32,
            false,
            &self.brushes.text_primary,
        );

        let title = match layout.page {
            ControlCenterPage::Wifi => "Wi-Fi",
            ControlCenterPage::Bluetooth => "Bluetooth",
            ControlCenterPage::Sound => "Sound",
            ControlCenterPage::Display => "Display",
            ControlCenterPage::OttoSettings => "Modules & Settings",
            ControlCenterPage::Main => "",
        };
        let title_r = D2D_RECT_F {
            left: back.right() as f32 + scale(6, self.dpi) as f32,
            top: layout.header_rect.y as f32,
            right: if layout.header_toggle.is_empty() {
                layout.header_rect.right() as f32
            } else {
                layout.header_toggle.x as f32 - scale(8, self.dpi) as f32
            },
            bottom: layout.header_rect.bottom() as f32,
        };
        self.draw_text(
            title,
            &self.title_format,
            title_r,
            &self.brushes.text_primary,
        );

        if !layout.header_toggle.is_empty() {
            let (enabled, supported) = match layout.page {
                ControlCenterPage::Wifi => (snapshot.wifi.enabled, snapshot.wifi.supported),
                ControlCenterPage::Bluetooth => {
                    (snapshot.bluetooth.enabled, snapshot.bluetooth.supported)
                }
                _ => (false, false),
            };
            self.draw_toggle_pill(
                layout.header_toggle,
                enabled,
                supported,
                hovered == Some(ControlCenterHitTarget::HeaderToggle),
                palette,
            );
        }
    }

    fn draw_theme_selector_strip(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) {
        let modes = [ThemeMode::Auto, ThemeMode::Light, ThemeMode::Dark];
        let r = scale(9, self.dpi) as f32;
        for (mode, rect) in modes.into_iter().zip(layout.theme_options) {
            let selected = snapshot.theme_mode == mode;
            let is_hover = hovered == Some(ControlCenterHitTarget::ThemeOption(mode));
            let rr = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(rect),
                radiusX: r,
                radiusY: r,
            };
            let bg = self.brushes.color_brush(if selected {
                palette.toggle_active
            } else {
                palette.card_fill
            });
            unsafe {
                self.target.FillRoundedRectangle(&rr, &bg);
                if is_hover && !selected {
                    self.target.FillRoundedRectangle(&rr, &self.brushes.hover);
                }
                if palette.is_high_contrast {
                    self.target
                        .DrawRoundedRectangle(&rr, &self.brushes.specular_rim, 0.75, None);
                }
            }
            let fg = if selected {
                &self.brushes.on_accent
            } else {
                &self.brushes.text_primary
            };
            self.draw_text(mode.label(), &self.center_format, rect_to_d2d(rect), fg);
        }
    }

    fn draw_category_tabs(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) {
        for (category, rect) in OttoSettingsCategory::ALL
            .into_iter()
            .zip(layout.category_tabs)
        {
            if rect.is_empty() {
                continue;
            }
            let selected = snapshot.otto_category == category;
            let is_hover = hovered == Some(ControlCenterHitTarget::OttoCategoryTab(category));
            let r = rect.height as f32 * 0.5;
            let rr = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(rect),
                radiusX: r,
                radiusY: r,
            };
            let bg = self.brushes.color_brush(if selected {
                palette.toggle_active
            } else {
                palette.card_fill
            });
            unsafe {
                self.target.FillRoundedRectangle(&rr, &bg);
                if is_hover && !selected {
                    self.target.FillRoundedRectangle(&rr, &self.brushes.hover);
                }
                if palette.is_high_contrast {
                    self.target
                        .DrawRoundedRectangle(&rr, &self.brushes.specular_rim, 0.75, None);
                }
            }
            let fg = if selected {
                &self.brushes.on_accent
            } else if is_hover {
                &self.brushes.text_primary
            } else {
                &self.brushes.text_muted
            };
            self.draw_text(category.label(), &self.center_format, rect_to_d2d(rect), fg);
        }
    }

    fn draw_scrollable_list(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        if vp.is_empty() {
            return;
        }
        unsafe {
            self.target
                .PushAxisAlignedClip(&rect_to_d2d(vp), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
        self.draw_list_contents(layout, snapshot, hovered, palette);
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    fn draw_list_contents(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        hovered: Option<ControlCenterHitTarget>,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        if layout.items.is_empty() {
            let empty = match layout.page {
                ControlCenterPage::Wifi => {
                    if !snapshot.wifi.supported {
                        "Wi-Fi adapter unavailable"
                    } else if !snapshot.wifi.enabled {
                        "Turn on Wi-Fi to view networks"
                    } else if snapshot.wifi.busy {
                        "Scanning networks..."
                    } else {
                        "No networks found"
                    }
                }
                ControlCenterPage::Bluetooth => {
                    if !snapshot.bluetooth.supported {
                        "Bluetooth adapter unavailable"
                    } else if !snapshot.bluetooth.enabled {
                        "Turn on Bluetooth to view devices"
                    } else if snapshot.bluetooth.busy {
                        "Scanning devices..."
                    } else {
                        "No paired Bluetooth devices"
                    }
                }
                ControlCenterPage::Sound => "No audio output devices found",
                _ => "",
            };
            if !empty.is_empty() {
                self.draw_text(
                    empty,
                    &self.center_format,
                    rect_to_d2d(vp),
                    &self.brushes.text_muted,
                );
            }
            return;
        }

        let otto_items = (layout.page == ControlCenterPage::OttoSettings)
            .then(|| snapshot.settings.visible_items(snapshot.otto_category));
        let row_radius = scale(9, self.dpi) as f32;
        for (index, rect) in layout.items.iter().enumerate() {
            if rect.bottom() < vp.y || rect.y > vp.bottom() {
                continue;
            }
            let item_available = otto_items
                .as_ref()
                .and_then(|items| items.get(index).copied())
                .is_none_or(|item| snapshot.settings.is_item_available(item));
            let is_hover =
                item_available && hovered == Some(ControlCenterHitTarget::SubPanelItem(index));
            self.draw_card(*rect, row_radius, is_hover, palette);
            match layout.page {
                ControlCenterPage::Wifi => {
                    if let Some(net) = snapshot.wifi.networks.get(index) {
                        self.draw_wifi_network_row(*rect, net, palette);
                    }
                }
                ControlCenterPage::Bluetooth => {
                    if let Some(dev) = snapshot.bluetooth.devices.get(index) {
                        self.draw_bluetooth_device_row(*rect, dev, palette);
                    }
                }
                ControlCenterPage::Sound => {
                    if let Some(dev) = snapshot.audio_outputs.get(index) {
                        self.draw_audio_output_row(*rect, &dev.name, dev.is_default, palette);
                    }
                }
                ControlCenterPage::OttoSettings => {
                    if let Some(item) = otto_items
                        .as_ref()
                        .and_then(|items| items.get(index).copied())
                    {
                        self.draw_otto_setting_row(*rect, item, snapshot, is_hover, palette);
                    }
                }
                ControlCenterPage::Main | ControlCenterPage::Display => {}
            }
        }

        if layout.max_scroll > 0 {
            self.draw_list_scrollbar(vp, layout.scroll_offset, layout.max_scroll, palette);
        }
    }

    fn draw_list_scrollbar(
        &self,
        vp: Rect,
        scroll_offset: i32,
        max_scroll: i32,
        palette: &LiquidGlassPalette,
    ) {
        let bar_w = scale(3, self.dpi) as f32;
        let track_h = vp.height as f32;
        let total_h = (vp.height + max_scroll) as f32;
        let thumb_h = (track_h * (track_h / total_h)).max(scale(18, self.dpi) as f32);
        let frac = scroll_offset as f32 / max_scroll as f32;
        let thumb_y = vp.y as f32 + frac * (track_h - thumb_h);
        let thumb_r = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: vp.right() as f32 - bar_w,
                top: thumb_y,
                right: vp.right() as f32,
                bottom: thumb_y + thumb_h,
            },
            radiusX: bar_w * 0.5,
            radiusY: bar_w * 0.5,
        };
        let brush = self.brushes.color_brush(palette.text_disabled);
        unsafe {
            self.target.FillRoundedRectangle(&thumb_r, &brush);
        }
    }

    fn draw_wifi_network_row(&self, rect: Rect, net: &WifiNetwork, palette: &LiquidGlassPalette) {
        let pad = scale(10, self.dpi) as f32;
        let label_r = D2D_RECT_F {
            left: rect.x as f32 + pad,
            top: rect.y as f32,
            right: rect.right() as f32 - scale(76, self.dpi) as f32,
            bottom: rect.bottom() as f32,
        };
        let status_r = D2D_RECT_F {
            left: rect.right() as f32 - scale(84, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - pad,
            bottom: rect.bottom() as f32,
        };
        self.draw_text(
            &net.ssid,
            &self.body_format,
            label_r,
            &self.brushes.text_primary,
        );
        let right_label = if net.connected {
            format!("Connected · {}%", net.signal_percent)
        } else if net.secured {
            format!("{}% · Lock", net.signal_percent)
        } else {
            format!("{}%", net.signal_percent)
        };
        let status_brush = if net.connected {
            self.brushes.color_brush(palette.toggle_active)
        } else {
            self.brushes.text_muted.clone()
        };
        self.draw_text(&right_label, &self.value_format, status_r, &status_brush);
    }

    fn draw_bluetooth_device_row(
        &self,
        rect: Rect,
        device: &BluetoothDevice,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(10, self.dpi) as f32;
        let kind = match device.kind {
            BluetoothDeviceKind::Headphones => "Audio",
            BluetoothDeviceKind::Speaker => "Speaker",
            BluetoothDeviceKind::Mouse => "Mouse",
            BluetoothDeviceKind::Keyboard => "Keyboard",
            BluetoothDeviceKind::Phone => "Phone",
            BluetoothDeviceKind::Other => "Device",
        };
        let label_r = D2D_RECT_F {
            left: rect.x as f32 + pad,
            top: rect.y as f32,
            right: rect.right() as f32 - scale(96, self.dpi) as f32,
            bottom: rect.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: rect.right() as f32 - scale(96, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - pad,
            bottom: rect.bottom() as f32,
        };
        self.draw_text(
            &device.name,
            &self.body_format,
            label_r,
            &self.brushes.text_primary,
        );
        let right_text = if device.connected {
            device.battery_percent.map_or_else(
                || String::from("Connected"),
                |pct| format!("Connected · {pct}%"),
            )
        } else {
            String::from(kind)
        };
        let brush = if device.connected {
            self.brushes.color_brush(palette.toggle_active)
        } else {
            self.brushes.text_muted.clone()
        };
        self.draw_text(&right_text, &self.value_format, right_r, &brush);
    }

    fn draw_audio_output_row(
        &self,
        rect: Rect,
        name: &str,
        is_default: bool,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(10, self.dpi) as f32;
        let dot_r = scale(4, self.dpi) as f32;
        let cx = rect.x as f32 + pad + dot_r;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let accent = self.brushes.color_brush(palette.toggle_active);
        unsafe {
            let ellipse = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: dot_r,
                radiusY: dot_r,
            };
            if is_default {
                self.target.FillEllipse(&ellipse, &accent);
            } else {
                self.target
                    .DrawEllipse(&ellipse, &self.brushes.text_muted, 1.0, None);
            }
        }
        let label_r = D2D_RECT_F {
            left: cx + dot_r + scale(8, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - scale(56, self.dpi) as f32,
            bottom: rect.bottom() as f32,
        };
        let right_r = D2D_RECT_F {
            left: rect.right() as f32 - scale(56, self.dpi) as f32,
            top: rect.y as f32,
            right: rect.right() as f32 - pad,
            bottom: rect.bottom() as f32,
        };
        self.draw_text(name, &self.body_format, label_r, &self.brushes.text_primary);
        if is_default {
            self.draw_text("Active", &self.value_format, right_r, &accent);
        }
    }

    fn draw_otto_setting_row(
        &self,
        rect: Rect,
        item: OttoSettingItem,
        snapshot: &ControlCenterSnapshot,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(12, self.dpi) as f32;
        let available = snapshot.settings.is_item_available(item);
        let control_left = match item {
            OttoSettingItem::Toggle(kind) => {
                let toggle_w = scale(38, self.dpi);
                let toggle_h = scale(20, self.dpi);
                let toggle_rect = Rect {
                    x: rect.right() - scale(12, self.dpi) - toggle_w,
                    y: rect.y + (rect.height - toggle_h) / 2,
                    width: toggle_w,
                    height: toggle_h,
                };
                self.draw_toggle_pill(
                    toggle_rect,
                    snapshot.settings.is_enabled(kind),
                    available,
                    hovered && available,
                    palette,
                );
                toggle_rect.x as f32
            }
            OttoSettingItem::DockIconSize => {
                let chip_w = scale(64, self.dpi);
                let chip_h = scale(22, self.dpi);
                let chip_rect = Rect {
                    x: rect.right() - scale(12, self.dpi) - chip_w,
                    y: rect.y + (rect.height - chip_h) / 2,
                    width: chip_w,
                    height: chip_h,
                };
                let rr = D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(chip_rect),
                    radiusX: chip_h as f32 * 0.5,
                    radiusY: chip_h as f32 * 0.5,
                };
                let chip_bg = self.brushes.color_brush(if available {
                    palette.toggle_inactive
                } else {
                    palette.slider_track
                });
                unsafe {
                    self.target.FillRoundedRectangle(&rr, &chip_bg);
                    if available && hovered {
                        self.target.FillRoundedRectangle(&rr, &self.brushes.hover);
                    }
                }
                let val_brush = if available {
                    &self.brushes.text_primary
                } else {
                    &self.brushes.text_disabled
                };
                self.draw_text(
                    snapshot.settings.dock_size_label(),
                    &self.center_format,
                    rect_to_d2d(chip_rect),
                    val_brush,
                );
                chip_rect.x as f32
            }
        };

        let text_left = rect.x as f32 + pad;
        let text_right = (control_left - scale(8, self.dpi) as f32).max(text_left + 1.0);
        let title_r = D2D_RECT_F {
            left: text_left,
            top: rect.y as f32 + scale(5, self.dpi) as f32,
            right: text_right,
            bottom: rect.y as f32 + scale(25, self.dpi) as f32,
        };
        let subtitle_r = D2D_RECT_F {
            left: text_left,
            top: rect.y as f32 + scale(23, self.dpi) as f32,
            right: text_right,
            bottom: rect.bottom() as f32 - scale(4, self.dpi) as f32,
        };
        let title_brush = if available {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_disabled
        };
        let subtitle_brush = if available {
            &self.brushes.text_muted
        } else {
            &self.brushes.text_disabled
        };
        self.draw_text(item.label(), &self.body_format, title_r, title_brush);
        self.draw_text(
            snapshot.settings.item_subtitle(item),
            &self.subtitle_format,
            subtitle_r,
            subtitle_brush,
        );
    }

    fn draw_footer_button(
        &self,
        rect: Rect,
        label: &str,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        if rect.is_empty() {
            return;
        }
        self.draw_card(rect, scale(9, self.dpi) as f32, hovered, palette);
        self.draw_text(
            label,
            &self.center_format,
            rect_to_d2d(rect),
            &self.brushes.text_primary,
        );
    }

    fn draw_toggle_pill(
        &self,
        rect: Rect,
        enabled: bool,
        supported: bool,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let r = rect.height as f32 * 0.5;
        let bg_color = if !supported {
            palette.slider_track
        } else if enabled {
            palette.toggle_active
        } else {
            palette.toggle_inactive
        };
        let track = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: r,
            radiusY: r,
        };
        let bg = self.brushes.color_brush(bg_color);
        let pad = (rect.height as f32 * 0.14).max(2.0);
        let knob_r = (r - pad).max(2.0);
        let knob_x = if enabled {
            rect.right() as f32 - pad - knob_r
        } else {
            rect.x as f32 + pad + knob_r
        };
        let knob_y = rect.y as f32 + r;
        let knob_brush = if supported {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_disabled
        };
        unsafe {
            self.target.FillRoundedRectangle(&track, &bg);
            if hovered && supported {
                self.target
                    .FillRoundedRectangle(&track, &self.brushes.hover);
            }
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(knob_x, knob_y),
                    radiusX: knob_r,
                    radiusY: knob_r,
                },
                knob_brush,
            );
        }
    }

    fn draw_slider_pill(
        &self,
        rect: Rect,
        value: Option<u8>,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let radius = rect.height as f32 * 0.5;
        let track = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        let track_brush = self.brushes.color_brush(palette.slider_track);
        unsafe {
            self.target.FillRoundedRectangle(&track, &track_brush);
            if hovered && value.is_some() {
                self.target
                    .FillRoundedRectangle(&track, &self.brushes.hover);
            }
        }
        let Some(pct) = value else {
            self.draw_text(
                "Unavailable",
                &self.center_format,
                rect_to_d2d(rect),
                &self.brushes.text_disabled,
            );
            return;
        };
        let clamped = pct.min(100);
        let fill_w = (rect.width as f32 * (f32::from(clamped) / 100.0)).max(rect.height as f32);
        let fill_right = (rect.x as f32 + fill_w).min(rect.right() as f32);
        let fill_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.x as f32,
                top: rect.y as f32,
                right: fill_right,
                bottom: rect.bottom() as f32,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let fill_brush = self.brushes.color_brush(palette.slider_fill);
        let knob_r = (radius - scale(2, self.dpi) as f32).max(4.0);
        let knob_x = (rect.x as f32
            + (rect.width as f32 - rect.height as f32) * (f32::from(clamped) / 100.0)
            + radius)
            .clamp(rect.x as f32 + radius, rect.right() as f32 - radius);
        let knob_y = rect.y as f32 + radius;
        unsafe {
            if clamped > 0 {
                self.target.FillRoundedRectangle(&fill_rect, &fill_brush);
            }
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(knob_x, knob_y),
                    radiusX: knob_r,
                    radiusY: knob_r,
                },
                &self.brushes.on_accent,
            );
        }
    }

    fn draw_cover_thumb(
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

    fn cached_or_decode_cover(&self, bytes: &[u8]) -> Option<CoverArt> {
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

    fn draw_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                &rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    fn draw_chevron(&self, cx: f32, cy: f32, size: f32, right: bool, brush: &ID2D1SolidColorBrush) {
        let dir = if right { 1.0 } else { -1.0 };
        let dx = size * 0.45 * dir;
        let dy = size * 0.80;
        let stroke = (scale(2, self.dpi) as f32 * 0.72).max(1.3);
        unsafe {
            self.target.DrawLine(
                Vector2::new(cx - dx, cy - dy),
                Vector2::new(cx + dx, cy),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + dx, cy),
                Vector2::new(cx - dx, cy + dy),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    fn draw_bluetooth_glyph(
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

    fn draw_theme_glyph(&self, cx: f32, cy: f32, radius: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (radius * 0.24).max(1.2);
        unsafe {
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                brush,
                stroke,
                None,
            );
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx + radius * 0.28, cy),
                    radiusX: radius * 0.42,
                    radiusY: radius * 0.62,
                },
                brush,
            );
        }
    }

    fn draw_battery_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        fraction: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let w = half * 1.45;
        let h = half * 0.84;
        let left = cx - w * 0.52;
        let top = cy - h * 0.5;
        let r = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + w,
                bottom: top + h,
            },
            radiusX: 2.0,
            radiusY: 2.0,
        };
        let fill_w = ((w - 4.0) * fraction.clamp(0.0, 1.0)).max(0.0);
        let fill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + 2.0,
                top: top + 2.0,
                right: left + 2.0 + fill_w,
                bottom: top + h - 2.0,
            },
            radiusX: 1.0,
            radiusY: 1.0,
        };
        unsafe {
            self.target.DrawRoundedRectangle(&r, brush, 1.2, None);
            if fill_w > 0.5 {
                self.target.FillRoundedRectangle(&fill, brush);
            }
        }
    }

    fn draw_sun_glyph(&self, cx: f32, cy: f32, radius: f32, brush: &ID2D1SolidColorBrush) {
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius * 0.52,
                    radiusY: radius * 0.52,
                },
                brush,
            );
            for i in 0..8 {
                let angle = (i as f32) * (PI * 0.25);
                let (s, c) = angle.sin_cos();
                self.target.DrawLine(
                    Vector2::new(cx + c * radius * 0.78, cy + s * radius * 0.78),
                    Vector2::new(cx + c * radius * 1.15, cy + s * radius * 1.15),
                    brush,
                    1.2,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_gear_glyph(&self, cx: f32, cy: f32, radius: f32, brush: &ID2D1SolidColorBrush) {
        unsafe {
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius * 0.62,
                    radiusY: radius * 0.62,
                },
                brush,
                (radius * 0.26).max(1.3),
                None,
            );
            for i in 0..6 {
                let angle = (i as f32) * (PI / 3.0);
                let (s, c) = angle.sin_cos();
                self.target.DrawLine(
                    Vector2::new(cx + c * radius * 0.76, cy + s * radius * 0.76),
                    Vector2::new(cx + c * radius * 1.12, cy + s * radius * 1.12),
                    brush,
                    (radius * 0.26).max(1.3),
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_note_glyph(&self, cx: f32, cy: f32, half: f32, brush: &ID2D1SolidColorBrush) {
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx - half * 0.3, cy + half * 0.45),
                    radiusX: half * 0.34,
                    radiusY: half * 0.26,
                },
                brush,
            );
            self.target.DrawLine(
                Vector2::new(cx, cy + half * 0.42),
                Vector2::new(cx, cy - half * 0.72),
                brush,
                1.5,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx, cy - half * 0.72),
                Vector2::new(cx + half * 0.55, cy - half * 0.46),
                brush,
                1.8,
                &self.stroke_style,
            );
        }
    }

    fn draw_play_pause_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        playing: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            if playing {
                let bar_w = (half * 0.42).max(1.8);
                let gap = half * 0.35;
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left: cx - gap - bar_w,
                        top: cy - half,
                        right: cx - gap,
                        bottom: cy + half,
                    },
                    brush,
                );
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left: cx + gap,
                        top: cy - half,
                        right: cx + gap + bar_w,
                        bottom: cy + half,
                    },
                    brush,
                );
                return Ok(());
            }
            let tri = self.factory.CreatePathGeometry()?;
            let sink = tri.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - half * 0.55, cy - half),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + half * 0.85, cy));
            sink.AddLine(Vector2::new(cx - half * 0.55, cy + half));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&tri, brush, None);
        }
        Ok(())
    }

    fn draw_skip_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        next: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let dir = if next { 1.0 } else { -1.0 };
        unsafe {
            let tri = self.factory.CreatePathGeometry()?;
            let sink = tri.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - half * 0.75 * dir, cy - half * 0.8),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx + half * 0.40 * dir, cy));
            sink.AddLine(Vector2::new(cx - half * 0.75 * dir, cy + half * 0.8));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&tri, brush, None);
            let bar_x = cx + half * 0.58 * dir;
            self.target.DrawLine(
                Vector2::new(bar_x, cy - half * 0.78),
                Vector2::new(bar_x, cy + half * 0.78),
                brush,
                (half * 0.30).max(1.5),
                &self.stroke_style,
            );
        }
        Ok(())
    }
}

const fn d2d_rect_size(width: i32, height: i32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: 0.0,
        top: 0.0,
        right: width as f32,
        bottom: height as f32,
    }
}

const fn d2d_rect_inset(width: i32, height: i32, inset: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: inset,
        top: inset,
        right: width as f32 - inset,
        bottom: height as f32 - inset,
    }
}
