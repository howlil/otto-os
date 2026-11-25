use otto_core::{HitTarget, Layout, PomodoroScene, Rect, scale};

use crate::appearance::tokens::category_d2d_color;
use crate::kit::draw::center_of;
use crate::kit::icons::{ChevronDir, draw_chevron_glyph};
use crate::{Renderer, rect_to_d2d, translate_rect};

impl Renderer {
    pub(crate) fn draw_focus_category_pill(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
    ) {
        let rect = translate_rect(layout.pomodoro.category_pill, -offset_x);
        if rect.is_empty() || scene.category_name.is_empty() {
            return;
        }
        let accent = category_d2d_color(scene.category_color);
        let is_hovered = hovered == Some(HitTarget::PomodoroCategoryPill);
        let fill_alpha = if scene.category_picker_open {
            0.28
        } else if is_hovered {
            0.22
        } else {
            0.16
        };
        let radius = rect.height as f32 * 0.5;
        let fill = self.brushes.alpha_brush(accent, fill_alpha * alpha);
        self.draw_rounded_box(rect, radius, &fill);
        let border = self.brushes.alpha_brush(accent, 0.38 * alpha);
        self.draw_rounded_box_stroke(rect, radius, &border, 1.0);

        let dot_r = scale(3, self.dpi) as f32;
        let dot_x = rect.x as f32 + scale(10, self.dpi) as f32;
        let dot_y = rect.y as f32 + rect.height as f32 * 0.5;
        self.fill_solid_circle(
            windows_numerics::Vector2::new(dot_x, dot_y),
            dot_r,
            accent,
            alpha,
        );

        let chev_reserve = scale(14, self.dpi);
        let text_rect = Rect {
            x: rect.x + scale(16, self.dpi),
            y: rect.y,
            width: (rect.width - scale(18, self.dpi) - chev_reserve).max(0),
            height: rect.height,
        };
        let text_brush = self.brushes.alpha_brush(accent, 0.98 * alpha);
        self.draw_text(
            &scene.category_name,
            &self.text_tab,
            rect_to_d2d(text_rect),
            &text_brush,
        );

        let chev_center =
            windows_numerics::Vector2::new(rect.right() as f32 - scale(9, self.dpi) as f32, dot_y);
        let dir = if scene.category_picker_open {
            ChevronDir::Up
        } else {
            ChevronDir::Down
        };
        draw_chevron_glyph(
            &self.target,
            &self.stroke_style,
            chev_center,
            scale(3, self.dpi) as f32,
            1.4,
            dir,
            &text_brush,
        );
    }

    pub(crate) fn draw_focus_settings_button(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
    ) {
        let rect = translate_rect(layout.pomodoro.open_settings, -offset_x);
        if rect.is_empty() {
            return;
        }
        let fg = self.notch_ink();
        let fill_alpha = if hovered == Some(HitTarget::PomodoroOpenSettings) {
            0.22
        } else {
            0.10
        };
        self.draw_circle_button(rect, &self.brushes.alpha_brush(fg, fill_alpha * alpha));
        let stroke_brush = self.brushes.alpha_brush(fg, 0.85 * alpha);
        let bar_w = scale(10, self.dpi).max(6);
        let bar_h = scale(2, self.dpi).max(1);
        let bar_x = rect.x + (rect.width - bar_w) / 2;
        let top_y = rect.y + rect.height / 2 - scale(3, self.dpi);
        let bot_y = rect.y + rect.height / 2 + scale(2, self.dpi);
        for (y, knob_dx) in [(top_y, scale(3, self.dpi)), (bot_y, scale(7, self.dpi))] {
            self.draw_rounded_box(
                Rect {
                    x: bar_x,
                    y,
                    width: bar_w,
                    height: bar_h,
                },
                bar_h as f32 * 0.5,
                &stroke_brush,
            );
            self.fill_solid_circle(
                windows_numerics::Vector2::new(
                    (bar_x + knob_dx) as f32,
                    y as f32 + bar_h as f32 * 0.5,
                ),
                (bar_h as f32 * 1.15).max(1.8),
                fg,
                0.95 * alpha,
            );
        }
    }

    pub(crate) fn draw_focus_duration_steppers(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
    ) {
        let fg = self.notch_ink();
        for (raw_rect, target, symbol) in [
            (
                layout.pomodoro.duration_dec,
                HitTarget::PomodoroDurationDec,
                "-",
            ),
            (
                layout.pomodoro.duration_inc,
                HitTarget::PomodoroDurationInc,
                "+",
            ),
        ] {
            let rect = translate_rect(raw_rect, -offset_x);
            if rect.is_empty() {
                continue;
            }
            let fill_alpha = if hovered == Some(target) { 0.22 } else { 0.11 };
            self.draw_circle_button(rect, &self.brushes.alpha_brush(fg, fill_alpha * alpha));
            let ink = self.brushes.alpha_brush(fg, 0.90 * alpha);
            self.draw_text(symbol, &self.text_app_bold, rect_to_d2d(rect), &ink);
        }

        let time_rect = translate_rect(layout.pomodoro.time, -offset_x);
        if !time_rect.is_empty()
            && (scene.custom_time_input.is_some() || hovered == Some(HitTarget::PomodoroTimeEdit))
        {
            let accent = category_d2d_color(scene.category_color);
            let highlight = if scene.custom_time_input.is_some() {
                0.20
            } else {
                0.08
            };
            let bg = self.brushes.alpha_brush(fg, highlight * alpha);
            let radius = scale(8, self.dpi) as f32;
            self.draw_rounded_box(time_rect, radius, &bg);
            if scene.custom_time_input.is_some() {
                let ring = self.brushes.alpha_brush(accent, 0.85 * alpha);
                self.draw_rounded_box_stroke(time_rect, radius, &ring, 1.5);
            }
        }
    }

    pub(crate) fn draw_focus_category_picker_strip(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        scene: &PomodoroScene,
    ) {
        let fg = self.notch_ink();
        for (index, (id, name, color)) in scene.categories.iter().enumerate() {
            let Some(&raw_rect) = layout.pomodoro.category_options.get(index) else {
                break;
            };
            let rect = translate_rect(raw_rect, -offset_x);
            if rect.is_empty() {
                continue;
            }
            let active = *id == scene.category_id;
            let is_hovered = hovered == Some(HitTarget::PomodoroCategoryOption(index as u8));
            let tint = category_d2d_color(*color);
            let radius = rect.height as f32 * 0.5;
            let fill = if active {
                self.brushes.alpha_brush(tint, 0.24 * alpha)
            } else if is_hovered {
                self.brushes.alpha_brush(fg, 0.14 * alpha)
            } else {
                self.brushes.alpha_brush(fg, 0.08 * alpha)
            };
            self.draw_rounded_box(rect, radius, &fill);
            if active {
                let border = self.brushes.alpha_brush(tint, 0.50 * alpha);
                self.draw_rounded_box_stroke(rect, radius, &border, 1.0);
            }
            let dot_r = scale(3, self.dpi) as f32;
            let dot_c = center_of(Rect {
                x: rect.x + scale(4, self.dpi),
                y: rect.y,
                width: scale(10, self.dpi),
                height: rect.height,
            });
            self.fill_solid_circle(dot_c, dot_r, tint, alpha);
            let label_rect = Rect {
                x: rect.x + scale(14, self.dpi),
                y: rect.y,
                width: (rect.width - scale(18, self.dpi)).max(0),
                height: rect.height,
            };
            let text_brush = if active {
                self.brushes.alpha_brush(tint, 0.98 * alpha)
            } else {
                self.brushes.alpha_brush(fg, 0.78 * alpha)
            };
            self.draw_text(name, &self.text_tab, rect_to_d2d(label_rect), &text_brush);
        }
    }
}
