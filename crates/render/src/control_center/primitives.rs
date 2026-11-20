use crate::{CLEAR, LiquidGlassPalette, rect_to_d2d, scale};
use otto_core::{
    Appearance, ControlCenterHitTarget, ControlCenterLayout, GlassShape, Rect, ShapeTokens,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
    ID2D1Brush, ID2D1PathGeometry, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_MEASURING_MODE_NATURAL, IDWriteTextFormat};
use windows::core::Result;
use windows_numerics::Vector2;

use super::ControlCenterRenderer;
use super::ControlVisualState;
use super::d2d_rect_inset;
use super::d2d_rect_size;

impl ControlCenterRenderer {
    pub(super) fn draw_focus_ring(
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

    pub(super) fn squircle_path(&self, rect: D2D_RECT_F, radius: f32) -> Option<ID2D1PathGeometry> {
        (radius >= scale(10, self.dpi) as f32)
            .then(|| crate::build_squircle_path(&self.factory, rect, radius).ok())
            .flatten()
    }

    pub(super) fn fill_shape(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        geom: Option<&ID2D1PathGeometry>,
        brush: &ID2D1Brush,
    ) {
        if let Some(geom) = geom {
            unsafe {
                self.target.FillGeometry(geom, brush, None);
            }
            return;
        }
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&rounded, brush);
        }
    }

    pub(super) fn draw_shape(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        geom: Option<&ID2D1PathGeometry>,
        brush: &ID2D1Brush,
        stroke_width: f32,
    ) {
        if let Some(geom) = geom {
            unsafe {
                self.target.DrawGeometry(geom, brush, stroke_width, None);
            }
            return;
        }
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&rounded, brush, stroke_width, None);
        }
    }

    pub(super) fn draw_squircle_or_rounded(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        brush: &ID2D1Brush,
        stroke_width: f32,
    ) {
        let geom = self.squircle_path(rect, radius);
        self.draw_shape(rect, radius, geom.as_ref(), brush, stroke_width);
    }

    pub(super) fn draw_shell(
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

        let outer_rect = d2d_rect_size(width, height);
        let outer_geom = self.squircle_path(outer_rect, radius);
        let inner_rect = d2d_rect_inset(width, height, 0.75);
        let inner_radius = (radius - 0.75).max(1.0);
        self.fill_shape(outer_rect, radius, outer_geom.as_ref(), &fill);
        self.draw_squircle_or_rounded(inner_rect, inner_radius, rim, 1.0);
        self.draw_shape(outer_rect, radius, outer_geom.as_ref(), &outer, 1.0);
        Ok(())
    }

    pub(super) fn draw_card(
        &self,
        rect: Rect,
        radius: f32,
        hovered: bool,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_r = rect_to_d2d(rect);
        let outer_geom = self.squircle_path(d2d_r, radius);
        let fill = self.brushes.color_brush(palette.card_fill);
        self.fill_shape(d2d_r, radius, outer_geom.as_ref(), &fill);
        let sheen_alpha = palette.control_center_card_sheen_alpha();
        if sheen_alpha > 0.0 {
            let sheen_top = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: sheen_alpha,
            };
            let sheen_clear = D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.0,
            };
            if let Ok(sheen) = crate::notch::glow::linear_gradient(
                &self.target,
                Vector2::new(d2d_r.left, d2d_r.top),
                Vector2::new(d2d_r.left, d2d_r.top + rect.height as f32 * 0.48),
                sheen_top,
                sheen_clear,
            ) {
                self.fill_shape(d2d_r, radius, outer_geom.as_ref(), &sheen);
            }
        }
        self.draw_interaction_overlay(rect, radius, hovered, pressed, palette);
        if palette.is_high_contrast {
            self.draw_shape(
                d2d_r,
                radius,
                outer_geom.as_ref(),
                &self.brushes.specular_rim,
                0.9,
            );
        } else {
            let inner_d2d = D2D_RECT_F {
                left: d2d_r.left + 0.5,
                top: d2d_r.top + 0.5,
                right: d2d_r.right - 0.5,
                bottom: d2d_r.bottom - 0.5,
            };
            let inner_radius = (radius - 0.5).max(1.0);
            if palette.is_translucent {
                let top_lip = D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: if palette.is_dark { 0.16 } else { 0.90 },
                };
                let bottom_lip = D2D1_COLOR_F {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: if palette.is_dark { 0.05 } else { 0.36 },
                };
                if let Ok(rim) = crate::notch::glow::linear_gradient(
                    &self.target,
                    Vector2::new(d2d_r.left, d2d_r.top),
                    Vector2::new(d2d_r.right, d2d_r.bottom),
                    top_lip,
                    bottom_lip,
                ) {
                    self.draw_squircle_or_rounded(inner_d2d, inner_radius, &rim, 0.9);
                } else {
                    self.draw_squircle_or_rounded(
                        inner_d2d,
                        inner_radius,
                        &self.brushes.specular_rim,
                        0.9,
                    );
                }
            } else {
                self.draw_squircle_or_rounded(
                    inner_d2d,
                    inner_radius,
                    &self.brushes.specular_rim,
                    0.9,
                );
            }
            if !palette.is_dark {
                let edge = self.brushes.color_brush(palette.menu_outer_border);
                self.draw_shape(d2d_r, radius, outer_geom.as_ref(), &edge, 0.75);
            }
        }
    }

    pub(super) fn draw_interaction_overlay(
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
        let d2d_r = rect_to_d2d(rect);
        let is_pill_or_circle = radius + 0.5 >= rect.width.min(rect.height) as f32 * 0.5;
        let geom = if is_pill_or_circle {
            None
        } else {
            self.squircle_path(d2d_r, radius)
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
            self.fill_shape(d2d_r, radius, geom.as_ref(), &brush);
        } else {
            self.fill_shape(d2d_r, radius, geom.as_ref(), &self.brushes.hover);
        }
    }

    pub(super) fn draw_hover_pill(
        &self,
        rect: Rect,
        radius: f32,
        pressed: bool,
        palette: &LiquidGlassPalette,
    ) {
        self.draw_interaction_overlay(rect, radius, true, pressed, palette);
    }

    pub(super) fn draw_footer_button(
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

    pub(super) fn draw_toggle_pill(
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

    pub(super) fn draw_capsule_slider(
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

    pub(super) fn draw_capsule_slider_fill(
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

    pub(super) fn draw_capsule_slider_sun(
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

    pub(super) fn draw_text(
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

    pub(super) fn draw_chevron(
        &self,
        cx: f32,
        cy: f32,
        size: f32,
        right: bool,
        brush: &ID2D1SolidColorBrush,
    ) {
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
}
