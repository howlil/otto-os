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
    Rect, RgbBuffer, ShapeTokens, WifiNetwork, focus_row_subtitle, power_mode_label,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControlCenterPaintState {
    pub hovered: Option<ControlCenterHitTarget>,
    pub pressed: Option<ControlCenterHitTarget>,
    pub focused: Option<ControlCenterHitTarget>,
}

#[derive(Clone, Copy)]
struct ControlVisualState {
    active: bool,
    enabled: bool,
    pointer: PointerState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PointerState {
    Normal,
    Hovered,
    Pressed,
}

impl PointerState {
    const fn from_flags(hovered: bool, pressed: bool) -> Self {
        if pressed {
            Self::Pressed
        } else if hovered {
            Self::Hovered
        } else {
            Self::Normal
        }
    }

    const fn is_hovered(self) -> bool {
        matches!(self, Self::Hovered | Self::Pressed)
    }

    const fn is_pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }
}

fn connectivity_subtitle(supported: bool, enabled: bool, active_label: Option<&str>) -> &str {
    if !supported {
        "Unavailable"
    } else if enabled {
        active_label.unwrap_or("On")
    } else {
        "Off"
    }
}

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
    tab_format: IDWriteTextFormat,
    badge_format: IDWriteTextFormat,
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
            let tab_format = create_text_format(
                &dwrite,
                11.5 * scale_f,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let badge_format = create_text_format(
                &dwrite,
                9.5 * scale_f,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let surface = DibSurface::new(scale(384, dpi), scale(640, dpi))?;
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
                tab_format,
                badge_format,
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
        state: ControlCenterPaintState,
        appearance: &Appearance,
    ) -> Result<()> {
        draw_with_recovery(
            self,
            |renderer| renderer.draw_to_surface(layout, snapshot, state, appearance),
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
        state: ControlCenterPaintState,
        appearance: &Appearance,
    ) -> Result<()> {
        let w = layout.window.width.max(1);
        let h = layout.window.height.max(1);
        let (sw, sh) = self.surface.size();
        if w > sw || h > sh {
            self.surface = DibSurface::new(w.max(sw), h.max(sh))?;
        }
        let appearance = self.glass.adaptive_appearance(appearance);
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        self.brushes.update_theme(&palette);

        let target = self.target.clone();
        draw_on(&target, &self.surface, (w, h), || {
            self.draw_shell(w, h, layout.shell_radius as f32, &appearance, &palette)?;
            match layout.page {
                ControlCenterPage::Main => {
                    self.draw_main_page(layout, snapshot, state, &palette)?;
                }
                _ => self.draw_subpanel_page(layout, snapshot, state, &palette)?,
            }
            self.draw_focus_ring(layout, state.focused);
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
                | ControlCenterHitTarget::FocusOpenSettings
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

    fn draw_card(
        &self,
        rect: Rect,
        radius: f32,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_r = rect_to_d2d(rect);
        let r = D2D1_ROUNDED_RECT {
            rect: d2d_r,
            radiusX: radius,
            radiusY: radius,
        };
        let fill = self.brushes.color_brush(palette.card_fill);
        unsafe {
            self.target.FillRoundedRectangle(&r, &fill);
        }
        if palette.is_translucent && !palette.is_high_contrast {
            let sheen_top = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: if palette.is_dark { 0.08 } else { 0.24 },
            };
            let sheen_clear = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.0,
            };
            if let Ok(sheen) = crate::glow::linear_gradient(
                &self.target,
                Vector2::new(d2d_r.left, d2d_r.top),
                Vector2::new(d2d_r.left, d2d_r.top + rect.height as f32 * 0.48),
                sheen_top,
                sheen_clear,
            ) {
                unsafe {
                    self.target.FillRoundedRectangle(&r, &sheen);
                }
            }
        }
        self.draw_interaction_overlay(rect, radius, hovered, pressed, palette);
        unsafe {
            if palette.is_high_contrast {
                self.target
                    .DrawRoundedRectangle(&r, &self.brushes.specular_rim, 0.9, None);
            } else {
                let inner_r = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: d2d_r.left + 0.5,
                        top: d2d_r.top + 0.5,
                        right: d2d_r.right - 0.5,
                        bottom: d2d_r.bottom - 0.5,
                    },
                    radiusX: (radius - 0.5).max(1.0),
                    radiusY: (radius - 0.5).max(1.0),
                };
                if palette.is_translucent {
                    let top_lip = D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: if palette.is_dark { 0.40 } else { 0.90 },
                    };
                    let bottom_lip = D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: if palette.is_dark { 0.12 } else { 0.36 },
                    };
                    if let Ok(rim) = crate::glow::linear_gradient(
                        &self.target,
                        Vector2::new(d2d_r.left, d2d_r.top),
                        Vector2::new(d2d_r.right, d2d_r.bottom),
                        top_lip,
                        bottom_lip,
                    ) {
                        self.target.DrawRoundedRectangle(&inner_r, &rim, 0.9, None);
                    } else {
                        self.target.DrawRoundedRectangle(
                            &inner_r,
                            &self.brushes.specular_rim,
                            0.9,
                            None,
                        );
                    }
                } else {
                    self.target.DrawRoundedRectangle(
                        &inner_r,
                        &self.brushes.specular_rim,
                        0.9,
                        None,
                    );
                }
                if !palette.is_dark {
                    let edge = self.brushes.color_brush(palette.menu_outer_border);
                    self.target.DrawRoundedRectangle(&r, &edge, 0.75, None);
                }
            }
        }
    }

    fn draw_interaction_overlay(
        &self,
        rect: Rect,
        radius: f32,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        if !hovered && !pressed {
            return;
        }
        let pill = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        if pressed {
            let press_color = if palette.is_dark {
                D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.22,
                }
            } else {
                D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.12,
                }
            };
            let brush = self.brushes.color_brush(press_color);
            unsafe {
                self.target.FillRoundedRectangle(&pill, &brush);
            }
        } else {
            unsafe {
                self.target.FillRoundedRectangle(&pill, &self.brushes.hover);
            }
        }
    }

    fn draw_hover_pill(
        &self,
        rect: Rect,
        radius: f32,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        self.draw_interaction_overlay(rect, radius, true, pressed, palette);
    }

    fn draw_main_page(
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
        self.draw_otto_settings_tile(layout, snapshot, state, radius, palette);
        Ok(())
    }

    fn draw_connectivity_card(
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

    fn draw_connectivity_row_hover(
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

    fn draw_connectivity_divider(
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
    fn draw_connectivity_row(
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

    fn draw_focus_tile(
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
        let active = snapshot.settings.system.focus_module && snapshot.focus_active;
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
        let sub = focus_row_subtitle(snapshot.settings.system.focus_module, snapshot.focus_active);
        self.draw_text(
            "Focus",
            &self.body_format,
            title_r,
            &self.brushes.text_primary,
        );
        self.draw_text(sub, &self.subtitle_format, sub_r, &self.brushes.text_muted);
        Ok(())
    }

    fn draw_battery_tile(
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

    fn draw_display_card(
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

    fn draw_sound_card(
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

    fn draw_sound_controls(
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

    fn draw_media_card(
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

    fn draw_media_transport_buttons(
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

    fn draw_otto_settings_tile(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        let tile = layout.otto_settings_tile;
        self.draw_card(
            tile,
            radius,
            state.hovered == Some(ControlCenterHitTarget::OttoSettingsTile),
            state.pressed == Some(ControlCenterHitTarget::OttoSettingsTile),
            palette,
        );
        let cy = tile.y as f32 + tile.height as f32 * 0.5;
        let circle_r = scale(13, self.dpi) as f32;
        let cx = tile.x as f32 + scale(12, self.dpi) as f32 + circle_r;
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
        let badge_w = scale(80, self.dpi) as f32;
        let badge_right = chev_x - scale(8, self.dpi) as f32;
        let badge_left = badge_right - badge_w;
        let badge_r = D2D_RECT_F {
            left: badge_left,
            top: tile.y as f32,
            right: badge_right,
            bottom: tile.bottom() as f32,
        };
        self.draw_text(
            &badge_label,
            &self.value_format,
            badge_r,
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
            "Otto Modules & Settings",
            &self.title_format,
            title_r,
            &self.brushes.text_primary,
        );
    }

    fn draw_subpanel_page(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_subpanel_header(layout, snapshot, state, palette);
        match layout.page {
            ControlCenterPage::Sound => {
                self.draw_sound_subpanel(layout, snapshot, state, palette)?;
            }
            ControlCenterPage::Display => {
                self.draw_display_subpanel(layout, snapshot, state, palette);
            }
            ControlCenterPage::Wifi => {
                self.draw_scrollable_list(layout, snapshot, state, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Wi-Fi Settings...",
                    state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::Bluetooth => {
                self.draw_scrollable_list(layout, snapshot, state, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Bluetooth Settings...",
                    state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
            }
            ControlCenterPage::OttoSettings => {
                self.draw_category_tabs(layout, snapshot, state, palette);
                if !layout.error_banner.is_empty()
                    && let Some(err) = snapshot.last_error.as_deref()
                {
                    self.draw_error_banner(layout.error_banner, err);
                }
                self.draw_scrollable_list(layout, snapshot, state, palette);
                self.draw_footer_button(
                    layout.footer_primary,
                    "Logs Folder",
                    state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
                    state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
                    palette,
                );
                let update_label = snapshot.update_status.as_deref().unwrap_or("Check Updates");
                self.draw_footer_button(
                    layout.footer_secondary,
                    update_label,
                    state.hovered == Some(ControlCenterHitTarget::FooterSecondary),
                    state.pressed == Some(ControlCenterHitTarget::FooterSecondary),
                    palette,
                );
            }
            ControlCenterPage::Main => {}
        }
        Ok(())
    }

    fn draw_sound_subpanel(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        self.draw_card(
            layout.sound_card,
            layout.card_radius as f32,
            false,
            false,
            palette,
        );
        let hdr = layout.sound_header;
        let pad = scale(10, self.dpi) as f32;
        let value_text = if snapshot.status.muted || snapshot.status.volume == 0 {
            String::from("Muted")
        } else {
            format!("{}%", snapshot.status.volume)
        };
        self.draw_text(
            "Volume",
            &self.title_format,
            D2D_RECT_F {
                left: hdr.x as f32 + pad,
                top: hdr.y as f32 + scale(4, self.dpi) as f32,
                right: hdr.right() as f32 - scale(92, self.dpi) as f32,
                bottom: hdr.bottom() as f32,
            },
            &self.brushes.text_primary,
        );
        self.draw_text(
            &value_text,
            &self.value_format,
            D2D_RECT_F {
                left: hdr.right() as f32 - scale(90, self.dpi) as f32,
                top: hdr.y as f32 + scale(4, self.dpi) as f32,
                right: hdr.right() as f32 - pad,
                bottom: hdr.bottom() as f32,
            },
            &self.brushes.text_muted,
        );
        self.draw_sound_controls(layout, snapshot, state, palette)?;
        self.draw_scrollable_list(layout, snapshot, state, palette);
        self.draw_footer_button(
            layout.footer_primary,
            if snapshot.audio_switch_failed {
                "Open Sound Settings..."
            } else {
                "Sound Settings..."
            },
            state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
            state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
            palette,
        );
        Ok(())
    }

    fn draw_display_subpanel(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let card = layout.display_card;
        self.draw_card(card, layout.card_radius as f32, false, false, palette);
        let pad = scale(10, self.dpi) as f32;
        let val_text = snapshot
            .status
            .brightness
            .map_or_else(|| String::from("Unavailable"), |pct| format!("{pct}%"));
        self.draw_text(
            "Brightness",
            &self.title_format,
            D2D_RECT_F {
                left: card.x as f32 + pad,
                top: card.y as f32 + scale(4, self.dpi) as f32,
                right: card.right() as f32 - scale(90, self.dpi) as f32,
                bottom: card.y as f32 + scale(26, self.dpi) as f32,
            },
            &self.brushes.text_primary,
        );
        self.draw_text(
            &val_text,
            &self.value_format,
            D2D_RECT_F {
                left: card.right() as f32 - scale(90, self.dpi) as f32,
                top: card.y as f32 + scale(4, self.dpi) as f32,
                right: card.right() as f32 - pad,
                bottom: card.y as f32 + scale(26, self.dpi) as f32,
            },
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
        self.draw_footer_button(
            layout.footer_primary,
            "Display Settings...",
            state.hovered == Some(ControlCenterHitTarget::FooterPrimary),
            state.pressed == Some(ControlCenterHitTarget::FooterPrimary),
            palette,
        );
        self.draw_footer_button(
            layout.footer_secondary,
            "Night Light...",
            state.hovered == Some(ControlCenterHitTarget::FooterSecondary),
            state.pressed == Some(ControlCenterHitTarget::FooterSecondary),
            palette,
        );
    }

    fn draw_subpanel_header(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let back = layout.back_button;
        let back_r = back.height as f32 * 0.5;
        let back_bg = self.brushes.color_brush(palette.toggle_inactive);
        unsafe {
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(back),
                    radiusX: back_r,
                    radiusY: back_r,
                },
                &back_bg,
            );
        }
        self.draw_interaction_overlay(
            back,
            back_r,
            state.hovered == Some(ControlCenterHitTarget::BackButton),
            state.pressed == Some(ControlCenterHitTarget::BackButton),
            palette,
        );
        self.draw_chevron(
            back.x as f32 + back.width as f32 * 0.5,
            back.y as f32 + back.height as f32 * 0.5,
            scale(4, self.dpi) as f32,
            false,
            &self.brushes.text_primary,
        );

        let title = match layout.page {
            ControlCenterPage::Wifi => "Wi-Fi",
            ControlCenterPage::Bluetooth => "Bluetooth",
            ControlCenterPage::Sound => "Sound",
            ControlCenterPage::Display => "Display",
            ControlCenterPage::OttoSettings => "Otto Modules & Settings",
            ControlCenterPage::Main => "",
        };
        let title_r = D2D_RECT_F {
            left: back.right() as f32 + scale(10, self.dpi) as f32,
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
                ControlVisualState {
                    active: enabled,
                    enabled: supported,
                    pointer: PointerState::from_flags(
                        state.hovered == Some(ControlCenterHitTarget::HeaderToggle),
                        state.pressed == Some(ControlCenterHitTarget::HeaderToggle),
                    ),
                },
                palette,
            );
        }
    }

    fn draw_category_tabs(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let track = Rect {
            x: layout.category_tabs[0].x,
            y: layout.category_tabs[0].y,
            width: layout.category_tabs[3].right() - layout.category_tabs[0].x,
            height: layout.category_tabs[0].height,
        };
        let r = track.height as f32 * 0.5;
        let track_bg = self.brushes.color_brush(palette.slider_track);
        unsafe {
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(track),
                    radiusX: r,
                    radiusY: r,
                },
                &track_bg,
            );
        }
        let thumb_inset = scale(2, self.dpi) as f32;
        for (category, rect) in OttoSettingsCategory::ALL
            .into_iter()
            .zip(layout.category_tabs)
        {
            if rect.is_empty() {
                continue;
            }
            let target = ControlCenterHitTarget::OttoCategoryTab(category);
            let selected = snapshot.otto_category == category;
            let is_hover = state.hovered == Some(target);
            let is_pressed = state.pressed == Some(target);
            let pill_r = (r - thumb_inset).max(2.0);
            if selected {
                let thumb = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: rect.x as f32 + thumb_inset,
                        top: rect.y as f32 + thumb_inset,
                        right: rect.right() as f32 - thumb_inset,
                        bottom: rect.bottom() as f32 - thumb_inset,
                    },
                    radiusX: pill_r,
                    radiusY: pill_r,
                };
                let thumb_color = if palette.is_high_contrast {
                    palette.toggle_active
                } else if palette.is_dark {
                    D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.24,
                    }
                } else {
                    D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.96,
                    }
                };
                let bg = self.brushes.color_brush(thumb_color);
                unsafe {
                    self.target.FillRoundedRectangle(&thumb, &bg);
                    if !palette.is_dark && !palette.is_high_contrast {
                        let rim = self.brushes.color_brush(palette.menu_outer_border);
                        self.target.DrawRoundedRectangle(&thumb, &rim, 0.75, None);
                    }
                }
            } else if is_hover || is_pressed {
                self.draw_interaction_overlay(rect, pill_r, is_hover, is_pressed, palette);
            }
            let fg = if selected {
                if palette.is_high_contrast {
                    &self.brushes.on_accent
                } else {
                    &self.brushes.text_primary
                }
            } else if is_hover {
                &self.brushes.text_primary
            } else {
                &self.brushes.text_muted
            };
            self.draw_text(category.label(), &self.tab_format, rect_to_d2d(rect), fg);
        }
    }

    fn draw_error_banner(&self, rect: Rect, message: &str) {
        let radius = scale(9, self.dpi) as f32;
        let rr = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        let fill = self.brushes.color_brush(D2D1_COLOR_F {
            r: 0.92,
            g: 0.24,
            b: 0.22,
            a: 0.22,
        });
        let stroke = self.brushes.color_brush(D2D1_COLOR_F {
            r: 0.96,
            g: 0.36,
            b: 0.32,
            a: 0.75,
        });
        unsafe {
            self.target.FillRoundedRectangle(&rr, &fill);
            self.target.DrawRoundedRectangle(&rr, &stroke, 1.0, None);
        }
        let pad = scale(10, self.dpi) as f32;
        self.draw_text(
            message,
            &self.subtitle_format,
            D2D_RECT_F {
                left: rect.x as f32 + pad,
                top: rect.y as f32,
                right: rect.right() as f32 - pad,
                bottom: rect.bottom() as f32,
            },
            &self.brushes.text_primary,
        );
    }

    fn draw_scrollable_list(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
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
        self.draw_list_contents(layout, snapshot, state, palette);
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    fn draw_list_contents(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        if layout.items.is_empty() {
            self.draw_empty_list_state(layout.page, vp, snapshot);
            return;
        }
        let otto_items = (layout.page == ControlCenterPage::OttoSettings)
            .then(|| snapshot.settings.visible_items(snapshot.otto_category));
        for group in &layout.groups {
            self.draw_list_group(
                layout,
                group,
                otto_items.as_deref(),
                snapshot,
                state,
                palette,
            );
        }
        if layout.max_scroll > 0 {
            self.draw_list_scrollbar(vp, layout.scroll_offset, layout.max_scroll, palette);
        }
    }

    fn draw_empty_list_state(
        &self,
        page: ControlCenterPage,
        vp: Rect,
        snapshot: &ControlCenterSnapshot,
    ) {
        let empty = match page {
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
    }

    fn draw_list_group(
        &self,
        layout: &ControlCenterLayout,
        group: &otto_core::SubPanelGroupLayout,
        otto_items: Option<&[OttoSettingItem]>,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        let group_radius = layout.card_radius as f32;
        let sep_inset = scale(12, self.dpi) as f32;

        if !group.header.is_empty()
            && group.header.bottom() >= vp.y
            && group.header.y <= vp.bottom()
        {
            let hdr_r = D2D_RECT_F {
                left: group.header.x as f32 + scale(6, self.dpi) as f32,
                top: group.header.y as f32 + scale(5, self.dpi) as f32,
                right: group.header.right() as f32,
                bottom: group.header.bottom() as f32 - scale(3, self.dpi) as f32,
            };
            self.draw_text(
                group.title,
                &self.subtitle_format,
                hdr_r,
                &self.brushes.text_muted,
            );
        }
        if group.container.bottom() < vp.y || group.container.y > vp.bottom() {
            return;
        }
        self.draw_card(group.container, group_radius, false, false, palette);

        let clip_geo = unsafe {
            self.factory
                .CreateRoundedRectangleGeometry(&D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(group.container),
                    radiusX: group_radius,
                    radiusY: group_radius,
                })
        }
        .ok();
        let clip_layer = clip_geo
            .as_ref()
            .and_then(|_| unsafe { self.target.CreateLayer(None) }.ok());
        let mut active_mask = None;
        if let (Some(geo), Some(layer)) = (clip_geo, clip_layer.as_ref()) {
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: rect_to_d2d(group.container),
                geometricMask: std::mem::ManuallyDrop::new(Some(geo.into())),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: windows_numerics::Matrix3x2::identity(),
                opacity: 1.0,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            unsafe {
                self.target.PushLayer(&params, layer);
            }
            active_mask = Some(params.geometricMask);
        }

        for offset in 0..group.count {
            let index = group.start_index + offset;
            let Some(rect) = layout.items.get(index).copied() else {
                break;
            };
            if rect.bottom() < vp.y || rect.y > vp.bottom() {
                continue;
            }
            if offset > 0 {
                let sep = self.brushes.color_brush(palette.menu_separator);
                unsafe {
                    self.target.DrawLine(
                        Vector2::new(rect.x as f32 + sep_inset, rect.y as f32),
                        Vector2::new(rect.right() as f32 - sep_inset, rect.y as f32),
                        &sep,
                        1.0,
                        None,
                    );
                }
            }
            let item_available = otto_items
                .and_then(|items| items.get(index).copied())
                .is_none_or(|item| snapshot.settings.is_item_available(item));
            let target = ControlCenterHitTarget::SubPanelItem(index);
            let is_hover = item_available && state.hovered == Some(target);
            let is_pressed = item_available && state.pressed == Some(target);
            self.draw_interaction_overlay(rect, 0.0, is_hover, is_pressed, palette);
            self.draw_subpanel_row(
                layout.page,
                rect,
                index,
                otto_items,
                snapshot,
                is_hover,
                is_pressed,
                palette,
            );
        }

        if let Some(mask) = active_mask {
            unsafe {
                self.target.PopLayer();
            }
            std::mem::ManuallyDrop::into_inner(mask);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_subpanel_row(
        &self,
        page: ControlCenterPage,
        rect: Rect,
        index: usize,
        otto_items: Option<&[OttoSettingItem]>,
        snapshot: &ControlCenterSnapshot,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        match page {
            ControlCenterPage::Wifi => {
                if let Some(net) = snapshot.wifi.networks.get(index) {
                    self.draw_wifi_network_row(rect, net, palette);
                }
            }
            ControlCenterPage::Bluetooth => {
                if let Some(dev) = snapshot.bluetooth.devices.get(index) {
                    self.draw_bluetooth_device_row(rect, dev, palette);
                }
            }
            ControlCenterPage::Sound => {
                if let Some(dev) = snapshot.audio_outputs.get(index) {
                    self.draw_audio_output_row(rect, &dev.name, dev.is_default, palette);
                }
            }
            ControlCenterPage::OttoSettings => {
                if let Some(item) = otto_items.and_then(|items| items.get(index).copied()) {
                    self.draw_otto_setting_row(rect, item, snapshot, hovered, pressed, palette);
                }
            }
            ControlCenterPage::Main | ControlCenterPage::Display => {}
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
        let pad_v = scale(2, self.dpi) as f32;
        let track_h = (vp.height as f32 - pad_v * 2.0).max(8.0);
        let total_h = (vp.height + max_scroll) as f32;
        let thumb_h = (track_h * (track_h / total_h)).max(scale(20, self.dpi) as f32);
        let frac = scroll_offset as f32 / max_scroll as f32;
        let thumb_y = vp.y as f32 + pad_v + frac * (track_h - thumb_h);
        let bar_right = vp.right() as f32 - scale(1, self.dpi) as f32;
        let thumb_r = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bar_right - bar_w,
                top: thumb_y,
                right: bar_right,
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
        pressed: bool,
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
                    ControlVisualState {
                        active: snapshot.settings.is_enabled(kind),
                        enabled: available,
                        pointer: PointerState::from_flags(
                            hovered && available,
                            pressed && available,
                        ),
                    },
                    palette,
                );
                toggle_rect.x as f32
            }
            OttoSettingItem::DockIconSize => self.draw_icon_size_chip(
                rect,
                snapshot.settings.dock_size_label(),
                available,
                hovered,
                pressed,
                palette,
            ),
        };

        let mut right_bound = control_left - scale(8, self.dpi) as f32;
        if let Some(badge_text) = snapshot.settings.dependency_badge(item) {
            right_bound = self.draw_dependency_badge(
                rect,
                badge_text,
                rect.x as f32 + pad + scale(72, self.dpi) as f32,
                right_bound,
                palette,
            );
        }

        let text_left = rect.x as f32 + pad;
        let text_right = right_bound.max(text_left + 1.0);
        let title_r = D2D_RECT_F {
            left: text_left,
            top: rect.y as f32 + scale(4, self.dpi) as f32,
            right: text_right,
            bottom: rect.y as f32 + scale(23, self.dpi) as f32,
        };
        let subtitle_r = D2D_RECT_F {
            left: text_left,
            top: rect.y as f32 + scale(21, self.dpi) as f32,
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

    fn draw_icon_size_chip(
        &self,
        rect: Rect,
        label: &str,
        available: bool,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) -> f32 {
        let chip_w = scale(64, self.dpi);
        let chip_h = scale(22, self.dpi);
        let chip_rect = Rect {
            x: rect.right() - scale(12, self.dpi) - chip_w,
            y: rect.y + (rect.height - chip_h) / 2,
            width: chip_w,
            height: chip_h,
        };
        let chip_r = chip_h as f32 * 0.5;
        let rr = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(chip_rect),
            radiusX: chip_r,
            radiusY: chip_r,
        };
        let chip_bg = self.brushes.color_brush(if available {
            palette.toggle_inactive
        } else {
            palette.slider_track
        });
        unsafe {
            self.target.FillRoundedRectangle(&rr, &chip_bg);
        }
        if available {
            self.draw_interaction_overlay(chip_rect, chip_r, hovered, pressed, palette);
        }
        let val_brush = if available {
            &self.brushes.text_primary
        } else {
            &self.brushes.text_disabled
        };
        self.draw_text(
            label,
            &self.center_format,
            rect_to_d2d(chip_rect),
            val_brush,
        );
        chip_rect.x as f32
    }

    fn draw_dependency_badge(
        &self,
        rect: Rect,
        badge_text: &str,
        min_left: f32,
        right_bound: f32,
        palette: &LiquidGlassPalette,
    ) -> f32 {
        let badge_w = scale(82, self.dpi) as f32;
        let badge_h = scale(18, self.dpi) as f32;
        let badge_left = right_bound - badge_w;
        if badge_left <= min_left {
            return right_bound;
        }
        let badge_top = rect.y as f32 + (rect.height as f32 - badge_h) * 0.5;
        let badge_rect = D2D_RECT_F {
            left: badge_left,
            top: badge_top,
            right: right_bound,
            bottom: badge_top + badge_h,
        };
        let badge_rr = D2D1_ROUNDED_RECT {
            rect: badge_rect,
            radiusX: badge_h * 0.5,
            radiusY: badge_h * 0.5,
        };
        let badge_bg = self.brushes.color_brush(palette.slider_track);
        unsafe {
            self.target.FillRoundedRectangle(&badge_rr, &badge_bg);
        }
        self.draw_text(
            badge_text,
            &self.badge_format,
            badge_rect,
            &self.brushes.text_disabled,
        );
        badge_left - scale(6, self.dpi) as f32
    }

    fn draw_footer_button(
        &self,
        rect: Rect,
        label: &str,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        if rect.is_empty() {
            return;
        }
        self.draw_card(rect, scale(10, self.dpi) as f32, hovered, pressed, palette);
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
        visual: ControlVisualState,
        palette: &LiquidGlassPalette,
    ) {
        let r = rect.height as f32 * 0.5;
        let bg_color = if !visual.enabled {
            D2D1_COLOR_F {
                a: palette.slider_track.a * 0.65,
                ..palette.slider_track
            }
        } else if visual.active {
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
        let knob_x = if visual.active {
            rect.right() as f32 - pad - knob_r
        } else {
            rect.x as f32 + pad + knob_r
        };
        let knob_y = rect.y as f32 + r;
        let knob_brush = if visual.enabled {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_disabled
        };
        unsafe {
            self.target.FillRoundedRectangle(&track, &bg);
        }
        if visual.enabled {
            self.draw_interaction_overlay(
                rect,
                r,
                visual.pointer.is_hovered(),
                visual.pointer.is_pressed(),
                palette,
            );
        }
        unsafe {
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

    fn draw_capsule_slider(
        &self,
        rect: Rect,
        value: Option<u8>,
        hovered: bool,
        pressed: bool,
        show_sun: bool,
        palette: &LiquidGlassPalette,
    ) {
        let radius = rect.height as f32 * 0.5;
        let track = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        let track_brush = self.brushes.color_brush(palette.slider_track);
        let track_rim = self.brushes.color_brush(palette.menu_separator);
        unsafe {
            self.target.FillRoundedRectangle(&track, &track_brush);
            self.target
                .DrawRoundedRectangle(&track, &track_rim, 0.75, None);
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
        self.draw_interaction_overlay(rect, radius, hovered, pressed, palette);
        let clamped = pct.min(100);
        self.draw_capsule_slider_fill(rect, radius, clamped, palette);
        if show_sun {
            self.draw_capsule_slider_sun(
                rect.x as f32 + radius,
                rect.y as f32 + radius,
                clamped,
                palette,
            );
        }
    }

    fn draw_capsule_slider_fill(
        &self,
        rect: Rect,
        radius: f32,
        clamped: u8,
        palette: &LiquidGlassPalette,
    ) {
        if clamped == 0 {
            return;
        }
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
        let capsule_white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: if palette.is_dark { 0.92 } else { 0.96 },
        };
        let fill_color = if palette.is_high_contrast {
            palette.slider_fill
        } else {
            capsule_white
        };
        let fill_brush = self.brushes.color_brush(fill_color);
        unsafe {
            self.target.FillRoundedRectangle(&fill_rect, &fill_brush);
            if !palette.is_dark && !palette.is_high_contrast {
                let edge = self.brushes.color_brush(palette.menu_outer_border);
                self.target
                    .DrawRoundedRectangle(&fill_rect, &edge, 0.75, None);
            }
        }
    }

    fn draw_capsule_slider_sun(
        &self,
        cap_cx: f32,
        cy: f32,
        clamped: u8,
        palette: &LiquidGlassPalette,
    ) {
        let glyph_dark = self.brushes.color_brush(D2D1_COLOR_F {
            r: 0.10,
            g: 0.10,
            b: 0.14,
            a: 0.88,
        });
        let glyph_brush = if clamped > 0 {
            if palette.is_high_contrast {
                &self.brushes.on_accent
            } else {
                &glyph_dark
            }
        } else {
            &self.brushes.text_muted
        };
        self.draw_sun_glyph(cap_cx, cy, scale(5, self.dpi) as f32, glyph_brush);
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

    fn draw_battery_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        fraction: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let w = half * 1.48;
        let h = half * 0.88;
        let left = cx - w * 0.56;
        let top = cy - h * 0.5;
        let r = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + w,
                bottom: top + h,
            },
            radiusX: 2.2,
            radiusY: 2.2,
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
        let nub = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + w + 1.1,
                top: cy - h * 0.24,
                right: left + w + 2.6,
                bottom: cy + h * 0.24,
            },
            radiusX: 0.8,
            radiusY: 0.8,
        };
        unsafe {
            self.target.DrawRoundedRectangle(&r, brush, 1.25, None);
            self.target.FillRoundedRectangle(&nub, brush);
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

    fn draw_more_glyph(&self, cx: f32, cy: f32, radius: f32, brush: &ID2D1SolidColorBrush) {
        let dot_r = (radius * 0.22).max(1.1);
        let spread = (radius * 0.72).max(3.2);
        unsafe {
            for offset in [-spread, 0.0, spread] {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx + offset, cy),
                        radiusX: dot_r,
                        radiusY: dot_r,
                    },
                    brush,
                );
            }
        }
    }

    fn draw_gear_glyph(&self, cx: f32, cy: f32, radius: f32, brush: &ID2D1SolidColorBrush) {
        unsafe {
            let ring_stroke = (radius * 0.30).max(1.5);
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius * 0.56,
                    radiusY: radius * 0.56,
                },
                brush,
                ring_stroke,
                None,
            );
            for i in 0..8 {
                let angle = (i as f32) * (PI * 0.25);
                let (s, c) = angle.sin_cos();
                self.target.DrawLine(
                    Vector2::new(cx + c * radius * 0.74, cy + s * radius * 0.74),
                    Vector2::new(cx + c * radius * 0.96, cy + s * radius * 0.96),
                    brush,
                    (radius * 0.28).max(1.5),
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
