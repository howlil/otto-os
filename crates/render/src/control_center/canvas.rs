use otto_core::{
    Appearance, ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot, Rect,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::ID2D1Brush;
use windows::core::Result;

use crate::appearance::surface::upload_bgra_bitmap;
use crate::appearance::tokens::with_alpha;
use crate::kit::draw::{
    draw_bitmap_rect_on, fill_shape, fill_squircle_or_rounded_f, stroke_squircle_or_rounded_f,
};
use crate::{LiquidGlassPalette, rect_to_d2d, scale};

use super::{ControlCenterPaintState, ControlCenterRenderer};

impl ControlCenterRenderer {
    pub(super) fn draw_canvas_backdrop(
        &self,
        width: i32,
        height: i32,
        appearance: &Appearance,
        palette: &LiquidGlassPalette,
        alpha: f32,
    ) -> Result<()> {
        let canvas_rect = D2D_RECT_F {
            left: 0.0,
            top: 0.0,
            right: width as f32,
            bottom: height as f32,
        };
        let wants_blur = appearance.uses_backdrop() && !palette.is_high_contrast;
        if wants_blur && self.draw_cached_canvas_blur(canvas_rect, alpha)? {
            let scrim_color = if palette.is_dark {
                D2D1_COLOR_F {
                    r: 0.03,
                    g: 0.04,
                    b: 0.07,
                    a: 0.34 * alpha,
                }
            } else {
                D2D1_COLOR_F {
                    r: 0.94,
                    g: 0.96,
                    b: 1.00,
                    a: 0.24 * alpha,
                }
            };
            let scrim = self.brushes.color_brush(scrim_color);
            fill_shape(&self.target, canvas_rect, 0.0, None, &scrim);
            return Ok(());
        }
        let fallback_color = if palette.is_high_contrast {
            with_alpha(palette.menu_fill, 0.96 * alpha)
        } else if palette.is_dark {
            D2D1_COLOR_F {
                r: 0.06,
                g: 0.07,
                b: 0.10,
                a: 0.90 * alpha,
            }
        } else {
            D2D1_COLOR_F {
                r: 0.88,
                g: 0.90,
                b: 0.94,
                a: 0.90 * alpha,
            }
        };
        let fill = self.brushes.color_brush(fallback_color);
        fill_shape(&self.target, canvas_rect, 0.0, None, &fill);
        Ok(())
    }

    fn draw_cached_canvas_blur(&self, dest: D2D_RECT_F, alpha: f32) -> Result<bool> {
        let Ok(pending) = self.canvas_blur_pixels.try_borrow() else {
            return Ok(false);
        };
        let Some((w, h, pixels)) = pending.as_ref() else {
            return Ok(false);
        };
        let Ok(mut gpu_slot) = self.canvas_blur.try_borrow_mut() else {
            return Ok(false);
        };
        if gpu_slot.is_none() {
            let bitmap = upload_bgra_bitmap(
                &self.target,
                *w,
                *h,
                pixels,
                otto_core::PixelLimits::BACKDROP,
            )?;
            *gpu_slot = Some(bitmap);
        }
        let Some(bitmap) = gpu_slot.as_ref() else {
            return Ok(false);
        };
        draw_bitmap_rect_on(&self.target, bitmap, dest, alpha);
        Ok(true)
    }

    pub(super) fn draw_canvas_toolbar(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let add = layout.toolbar_add;
        if !add.is_empty() {
            let r = add.height as f32 * 0.5;
            self.draw_floating_pill_bg(
                add,
                r,
                state.hovered == Some(ControlCenterHitTarget::ToolbarAdd),
                state.pressed == Some(ControlCenterHitTarget::ToolbarAdd),
                false,
                palette,
            );
            let cx = add.x as f32 + add.width as f32 * 0.5;
            let cy = add.y as f32 + add.height as f32 * 0.5;
            self.draw_plus_glyph(
                cx,
                cy,
                scale(6, self.dpi) as f32,
                &self.brushes.text_primary,
            );
        }

        let edit = layout.toolbar_edit;
        if !edit.is_empty() {
            let r = edit.height as f32 * 0.5;
            self.draw_floating_pill_bg(
                edit,
                r,
                state.hovered == Some(ControlCenterHitTarget::ToolbarEdit),
                state.pressed == Some(ControlCenterHitTarget::ToolbarEdit),
                snapshot.editing,
                palette,
            );
            let label = if snapshot.editing { "Done" } else { "Edit" };
            let text_brush = if snapshot.editing {
                &self.brushes.on_accent
            } else {
                &self.brushes.text_primary
            };
            self.draw_text(label, &self.center_format, rect_to_d2d(edit), text_brush);
        }

        if snapshot.editing && !layout.toolbar_reset.is_empty() {
            let reset = layout.toolbar_reset;
            let r = reset.height as f32 * 0.5;
            self.draw_floating_pill_bg(
                reset,
                r,
                state.hovered == Some(ControlCenterHitTarget::ToolbarReset),
                state.pressed == Some(ControlCenterHitTarget::ToolbarReset),
                false,
                palette,
            );
            self.draw_text(
                "Reset",
                &self.center_format,
                rect_to_d2d(reset),
                &self.brushes.text_primary,
            );
        }

        if snapshot.editing && !layout.add_control_button.is_empty() {
            let btn = layout.add_control_button;
            let r = btn.height as f32 * 0.5;
            self.draw_floating_pill_bg(
                btn,
                r,
                state.hovered == Some(ControlCenterHitTarget::AddControlButton),
                state.pressed == Some(ControlCenterHitTarget::AddControlButton),
                false,
                palette,
            );
            self.draw_text(
                "+ Add a Control",
                &self.center_format,
                rect_to_d2d(btn),
                &self.brushes.text_primary,
            );
        }
    }

    pub(super) fn draw_drag_preview(
        &self,
        preview: Rect,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) {
        if preview.is_empty() {
            return;
        }
        let d2d_r = rect_to_d2d(preview);
        let fill = self
            .brushes
            .color_brush(with_alpha(palette.toggle_active, 0.22));
        let rim = self
            .brushes
            .color_brush(with_alpha(palette.toggle_active, 0.85));
        let fill_b: &ID2D1Brush = (&fill).into();
        let rim_b: &ID2D1Brush = (&rim).into();
        fill_squircle_or_rounded_f(&self.factory, &self.target, d2d_r, radius, self.dpi, fill_b);
        stroke_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            d2d_r,
            radius,
            self.dpi,
            rim_b,
            2.0,
        );
    }

    pub(super) fn draw_floating_pill_bg(
        &self,
        rect: Rect,
        radius: f32,
        hovered: bool,
        pressed: bool,
        active: bool,
        palette: &LiquidGlassPalette,
    ) {
        let d2d_r = rect_to_d2d(rect);
        let fill_color = if active {
            palette.toggle_active
        } else if palette.is_high_contrast {
            palette.card_fill
        } else if palette.is_dark {
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.14,
            }
        } else {
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.56,
            }
        };
        let fill = self.brushes.color_brush(fill_color);
        let fill_b: &ID2D1Brush = (&fill).into();
        fill_squircle_or_rounded_f(&self.factory, &self.target, d2d_r, radius, self.dpi, fill_b);
        self.draw_interaction_overlay(rect, radius, hovered, pressed, palette);
        let rim_color = if palette.is_dark {
            D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.24,
            }
        } else {
            palette.menu_outer_border
        };
        let rim = self.brushes.color_brush(rim_color);
        let rim_b: &ID2D1Brush = (&rim).into();
        stroke_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            d2d_r,
            radius,
            self.dpi,
            rim_b,
            1.0,
        );
    }
}
