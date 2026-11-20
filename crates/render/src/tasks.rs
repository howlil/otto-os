use otto_core::{HitTarget, Layout, Rect, TaskItem, TasksState, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows_numerics::Vector2;

use crate::shelf::{SHELF_CLEAR_RED, SHELF_URL_INDIGO, center_of};
use crate::{CHARGING_GREEN, FOREGROUND, Renderer, rect_to_d2d, translate_rect};

const TASKS_HINT: &str = "Press Ctrl+Alt+Q to capture one";

impl Renderer {
    pub(crate) fn draw_expanded_tasks(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        tasks_state: &TasksState,
    ) {
        self.draw_tasks_header_controls(layout, offset_x, alpha, hovered, tasks_state);
        if tasks_state.items.is_empty() {
            self.draw_tasks_empty_state(layout, offset_x, alpha);
            return;
        }
        for (slot, item) in tasks_state.visible_items().iter().enumerate() {
            let Some(&raw_row) = layout.tasks_rows.get(slot) else {
                break;
            };
            if raw_row.is_empty() {
                continue;
            }
            let row = translate_rect(raw_row, -offset_x);
            let check = translate_rect(layout.tasks_check_buttons[slot], -offset_x);
            let remove = translate_rect(layout.tasks_remove_buttons[slot], -offset_x);
            self.draw_task_row(row, check, remove, slot, item, alpha, hovered);
        }
    }

    fn draw_tasks_header_controls(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        tasks_state: &TasksState,
    ) {
        if !layout.tasks_pager_button.is_empty() {
            let pager = translate_rect(layout.tasks_pager_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::TasksPager);
            let fill = self
                .brushes
                .alpha_brush(FOREGROUND, if is_hovered { 0.17 } else { 0.09 } * alpha);
            let border = self.brushes.alpha_brush(FOREGROUND, 0.15 * alpha);
            self.draw_shelf_pill(pager, &fill, Some(&border));
            let label = format!(
                "{}/{}",
                tasks_state.clamped_page() + 1,
                tasks_state.page_count().max(1)
            );
            let fg = self.brushes.alpha_brush(FOREGROUND, 0.90 * alpha);
            self.draw_text(&label, &self.text_tab, rect_to_d2d(pager), &fg);
        }

        if !layout.tasks_add_button.is_empty() {
            let add = translate_rect(layout.tasks_add_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::TasksAdd);
            let fill = self.brushes.alpha_brush(
                SHELF_URL_INDIGO,
                if is_hovered { 0.30 } else { 0.18 } * alpha,
            );
            let border = self.brushes.alpha_brush(
                SHELF_URL_INDIGO,
                if is_hovered { 0.62 } else { 0.40 } * alpha,
            );
            self.draw_shelf_pill(add, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(FOREGROUND, 0.94 * alpha);
            self.draw_text("+ Add", &self.text_tab, rect_to_d2d(add), &fg);
        }

        if !layout.tasks_clear_button.is_empty() {
            let clear = translate_rect(layout.tasks_clear_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::TasksClearDone);
            let (fill, border, fg_color) = if is_hovered {
                (
                    self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.24 * alpha),
                    self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.52 * alpha),
                    SHELF_CLEAR_RED,
                )
            } else {
                (
                    self.brushes.alpha_brush(FOREGROUND, 0.09 * alpha),
                    self.brushes.alpha_brush(FOREGROUND, 0.15 * alpha),
                    FOREGROUND,
                )
            };
            self.draw_shelf_pill(clear, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(fg_color, 0.92 * alpha);
            self.draw_text("Clear", &self.text_tab, rect_to_d2d(clear), &fg);
        }
    }

    fn draw_tasks_empty_state(&self, layout: &Layout, offset_x: i32, alpha: f32) {
        let body = translate_rect(layout.tasks_body, -offset_x);
        if body.width <= 0 || body.height <= 0 {
            return;
        }
        let radius = scale(12, self.dpi) as f32;
        let card = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(body),
            radiusX: radius,
            radiusY: radius,
        };
        let fill = self.brushes.alpha_brush(FOREGROUND, 0.045 * alpha);
        let border = self.brushes.alpha_brush(FOREGROUND, 0.16 * alpha);
        unsafe {
            self.target.FillRoundedRectangle(&card, &fill);
            self.target.DrawRoundedRectangle(&card, &border, 1.0, None);
        }
        let center = center_of(body);
        let icon_r = scale(8, self.dpi) as f32;
        let icon_brush = self.brushes.alpha_brush(FOREGROUND, 0.72 * alpha);
        self.draw_tasks_glyph(
            Vector2::new(center.X, body.y as f32 + scale(25, self.dpi) as f32),
            icon_r,
            &icon_brush,
        );
        let title_brush = self.brushes.alpha_brush(FOREGROUND, 0.90 * alpha);
        let title_rect = D2D_RECT_F {
            left: body.x as f32 + scale(12, self.dpi) as f32,
            top: body.y as f32 + scale(41, self.dpi) as f32,
            right: body.right() as f32 - scale(12, self.dpi) as f32,
            bottom: body.y as f32 + scale(61, self.dpi) as f32,
        };
        self.draw_text("No tasks yet", &self.text_tab, title_rect, &title_brush);
        let hint_brush = self.brushes.alpha_brush(FOREGROUND, 0.56 * alpha);
        let hint_rect = D2D_RECT_F {
            left: body.x as f32 + scale(16, self.dpi) as f32,
            top: body.y as f32 + scale(61, self.dpi) as f32,
            right: body.right() as f32 - scale(16, self.dpi) as f32,
            bottom: body.bottom() as f32 - scale(6, self.dpi) as f32,
        };
        self.draw_text(
            TASKS_HINT,
            &self.text_caption_center,
            hint_rect,
            &hint_brush,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_task_row(
        &self,
        row: Rect,
        check: Rect,
        remove: Rect,
        slot: usize,
        item: &TaskItem,
        alpha: f32,
        hovered: Option<HitTarget>,
    ) {
        let slot_index = u8::try_from(slot).ok();
        let row_hovered = slot_index.is_some_and(|idx| {
            matches!(
                hovered,
                Some(HitTarget::TaskItem(s) | HitTarget::TaskRemove(s) | HitTarget::TaskCheck(s))
                    if s == idx
            )
        });
        let remove_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::TaskRemove(idx)));
        let check_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::TaskCheck(idx)));

        let radius = scale(7, self.dpi) as f32;
        let shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(row),
            radiusX: radius,
            radiusY: radius,
        };
        let (fill, border) = if item.active {
            (
                self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.22 * alpha),
                self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.62 * alpha),
            )
        } else if row_hovered {
            (
                self.brushes.alpha_brush(FOREGROUND, 0.13 * alpha),
                self.brushes.alpha_brush(FOREGROUND, 0.24 * alpha),
            )
        } else {
            (
                self.brushes.alpha_brush(FOREGROUND, 0.065 * alpha),
                self.brushes.alpha_brush(FOREGROUND, 0.11 * alpha),
            )
        };
        unsafe {
            self.target.FillRoundedRectangle(&shape, &fill);
            self.target.DrawRoundedRectangle(&shape, &border, 1.0, None);
        }

        self.draw_task_check(check, item.done, check_hovered, alpha);

        let text_left = check.right() + scale(7, self.dpi);
        let text_right = if remove.is_empty() {
            row.right() - scale(8, self.dpi)
        } else {
            remove.x - scale(6, self.dpi)
        };
        if text_right > text_left {
            let text_rect = D2D_RECT_F {
                left: text_left as f32,
                top: row.y as f32,
                right: text_right as f32,
                bottom: row.bottom() as f32,
            };
            let text_alpha = if item.done {
                0.42
            } else if row_hovered || item.active {
                0.96
            } else {
                0.88
            };
            let brush = self.brushes.alpha_brush(FOREGROUND, text_alpha * alpha);
            self.draw_text(&item.text, &self.text_pomodoro_sub, text_rect, &brush);
        }
        if !remove.is_empty() {
            self.draw_mini_remove_button(remove, remove_hovered, row_hovered, alpha);
        }
    }

    fn draw_task_check(&self, check: Rect, done: bool, hovered: bool, alpha: f32) {
        if check.is_empty() {
            return;
        }
        let center = center_of(check);
        let radius = check.width.min(check.height) as f32 * 0.38;
        let ring = D2D1_ELLIPSE {
            point: center,
            radiusX: radius,
            radiusY: radius,
        };
        let stroke = (radius * 0.22).max(1.2);
        unsafe {
            if done {
                let fill = self.brushes.alpha_brush(CHARGING_GREEN, 0.92 * alpha);
                self.target.FillEllipse(&ring, &fill);
                let tick = self.brushes.alpha_brush(FOREGROUND, 0.95 * alpha);
                self.draw_check_stroke(center, radius, stroke, &tick);
            } else {
                let tone = if hovered { 0.86 } else { 0.55 };
                let line = self.brushes.alpha_brush(FOREGROUND, tone * alpha);
                self.target
                    .DrawEllipse(&ring, &line, stroke, &self.stroke_style);
            }
        }
    }

    fn draw_check_stroke(
        &self,
        center: Vector2,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let a = Vector2::new(center.X - r * 0.46, center.Y + r * 0.02);
        let b = Vector2::new(center.X - r * 0.1, center.Y + r * 0.4);
        let c = Vector2::new(center.X + r * 0.5, center.Y - r * 0.34);
        unsafe {
            self.target
                .DrawLine(a, b, brush, stroke, &self.stroke_style);
            self.target
                .DrawLine(b, c, brush, stroke, &self.stroke_style);
        }
    }

    pub(crate) fn draw_tasks_glyph(&self, center: Vector2, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (r * 0.24).max(1.25);
        let ring = D2D1_ELLIPSE {
            point: center,
            radiusX: r * 0.92,
            radiusY: r * 0.92,
        };
        unsafe {
            self.target
                .DrawEllipse(&ring, brush, stroke, &self.stroke_style);
        }
        self.draw_check_stroke(center, r * 1.9, stroke, brush);
    }
}
