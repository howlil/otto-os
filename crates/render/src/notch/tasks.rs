use otto_core::{HitTarget, Layout, Rect, TaskEditorMode, TaskItem, TasksState, scale};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, ID2D1SolidColorBrush};
use windows_numerics::Vector2;

use crate::kit::icons::draw_return_glyph;
use crate::notch::shelf::{SHELF_CLEAR_RED, SHELF_URL_INDIGO, center_of};
use crate::{CHARGING_GREEN, FOREGROUND, Renderer, rect_to_d2d, rgb, translate_rect};

const TASKS_HINT: &str = "Click + Add or press Ctrl+Alt+Q";
const TASKS_EDITING_HINT: &str = "Enter to save · Shift+Enter for next · Esc to cancel";
const EDITOR_FILL: D2D1_COLOR_F = rgb(20, 21, 26, 1.0);

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
        if let Some(mode) = tasks_state.editor {
            self.draw_tasks_editor(layout, offset_x, alpha, mode);
        }
        if tasks_state.items.is_empty() {
            self.draw_tasks_empty_state(layout, offset_x, alpha, tasks_state.is_editing());
            return;
        }
        let editing_id = tasks_state.editing_id();
        for (slot, item) in tasks_state.visible_items().iter().enumerate() {
            let Some(&raw_row) = layout.tasks.rows.get(slot) else {
                break;
            };
            if raw_row.is_empty() {
                continue;
            }
            let row = translate_rect(raw_row, -offset_x);
            let check = translate_rect(layout.tasks.check_buttons[slot], -offset_x);
            let edit = translate_rect(layout.tasks.edit_buttons[slot], -offset_x);
            let remove = translate_rect(layout.tasks.remove_buttons[slot], -offset_x);
            let is_editing = editing_id == Some(item.id);
            self.draw_task_row(
                row, check, edit, remove, slot, item, is_editing, alpha, hovered,
            );
        }
    }

    fn draw_tasks_editor(&self, layout: &Layout, offset_x: i32, alpha: f32, mode: TaskEditorMode) {
        if layout.tasks.editor.is_empty() {
            return;
        }
        let ink = self.notch_ink();
        let editor = translate_rect(layout.tasks.editor, -offset_x);
        let input = translate_rect(layout.tasks.editor_input, -offset_x);
        let radius = scale(8, self.dpi) as f32;
        let border = self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.72 * alpha);
        if self.high_contrast.get() {
            self.draw_rounded_box(editor, radius, &self.brushes.card_fill);
        } else {
            let fill = self.brushes.alpha_brush(EDITOR_FILL, alpha);
            self.draw_rounded_box(editor, radius, &fill);
        }
        self.draw_rounded_box_stroke(editor, radius, &border, 1.2);

        let icon_brush = self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.94 * alpha);
        let icon_cx = (editor.x + input.x) as f32 * 0.5;
        let cy = editor.y as f32 + editor.height as f32 * 0.5;
        let icon_r = scale(4, self.dpi) as f32;
        match mode {
            TaskEditorMode::Add => self.draw_plus_glyph(icon_cx, cy, icon_r, &icon_brush),
            TaskEditorMode::Edit(_) => {
                self.draw_pencil_glyph(icon_cx, cy, icon_r * 1.1, &icon_brush);
            }
        }

        let hint_left = input.right() + scale(4, self.dpi);
        let hint_right = editor.right() - scale(6, self.dpi);
        if hint_right > hint_left {
            let hint_h = scale(16, self.dpi);
            let hint_rect = Rect {
                x: hint_left,
                y: editor.y + (editor.height - hint_h) / 2,
                width: hint_right - hint_left,
                height: hint_h,
            };
            let hint_bg = self.brushes.alpha_brush(ink, 0.10 * alpha);
            let hint_border = self.brushes.alpha_brush(ink, 0.16 * alpha);
            let pill_r = scale(4, self.dpi) as f32;
            self.draw_rounded_box(hint_rect, pill_r, &hint_bg);
            self.draw_rounded_box_stroke(hint_rect, pill_r, &hint_border, 1.0);
            let glyph_brush = self.brushes.alpha_brush(ink, 0.80 * alpha);
            let g_r = (scale(4, self.dpi) as f32).min(hint_rect.width as f32 * 0.30);
            let g_stroke = (g_r * 0.32).max(1.2);
            draw_return_glyph(
                &self.target,
                &self.stroke_style,
                center_of(hint_rect),
                g_r,
                g_stroke,
                &glyph_brush,
            );
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
        let ink = self.notch_ink();
        if !layout.tasks.pager_button.is_empty() {
            let pager = translate_rect(layout.tasks.pager_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::TasksPager);
            let fill = self
                .brushes
                .alpha_brush(ink, if is_hovered { 0.17 } else { 0.09 } * alpha);
            let border = self.brushes.alpha_brush(ink, 0.15 * alpha);
            self.draw_shelf_pill(pager, &fill, Some(&border));
            let label = format!(
                "{}/{}",
                tasks_state.clamped_page() + 1,
                tasks_state.page_count().max(1)
            );
            let fg = self.brushes.alpha_brush(ink, 0.90 * alpha);
            self.draw_tabular_text(&label, &self.text_tab, rect_to_d2d(pager), &fg);
        }

        if !layout.tasks.add_button.is_empty() {
            let add = translate_rect(layout.tasks.add_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::TasksAdd);
            let is_adding = tasks_state.editor == Some(TaskEditorMode::Add);
            let fill = self.brushes.alpha_brush(
                SHELF_URL_INDIGO,
                if is_hovered || is_adding { 0.30 } else { 0.18 } * alpha,
            );
            let border = self.brushes.alpha_brush(
                SHELF_URL_INDIGO,
                if is_hovered || is_adding { 0.68 } else { 0.40 } * alpha,
            );
            self.draw_shelf_pill(add, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(ink, 0.94 * alpha);
            if is_adding {
                self.draw_text("Close", &self.text_tab, rect_to_d2d(add), &fg);
            } else {
                let plus_r = scale(4, self.dpi) as f32;
                let plus_cx = add.x as f32 + scale(10, self.dpi) as f32;
                let cy = add.y as f32 + add.height as f32 * 0.5;
                self.draw_plus_glyph(plus_cx, cy, plus_r, &fg);
                let text_rect = D2D_RECT_F {
                    left: add.x as f32 + scale(15, self.dpi) as f32,
                    top: add.y as f32,
                    right: add.right() as f32 - scale(4, self.dpi) as f32,
                    bottom: add.bottom() as f32,
                };
                self.draw_text("Add", &self.text_tab, text_rect, &fg);
            }
        }

        if !layout.tasks.clear_button.is_empty() {
            let clear = translate_rect(layout.tasks.clear_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::TasksClearDone);
            let (fill, border, fg_color) = if is_hovered {
                (
                    self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.24 * alpha),
                    self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.52 * alpha),
                    SHELF_CLEAR_RED,
                )
            } else {
                (
                    self.brushes.alpha_brush(ink, 0.09 * alpha),
                    self.brushes.alpha_brush(ink, 0.15 * alpha),
                    ink,
                )
            };
            self.draw_shelf_pill(clear, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(fg_color, 0.92 * alpha);
            self.draw_text("Clear", &self.text_tab, rect_to_d2d(clear), &fg);
        }
    }

    fn draw_tasks_empty_state(&self, layout: &Layout, offset_x: i32, alpha: f32, editing: bool) {
        let body = translate_rect(layout.tasks.body, -offset_x);
        if body.width <= 0 || body.height <= 0 {
            return;
        }
        let ink = self.notch_ink();
        let radius = scale(12, self.dpi) as f32;
        let fill = self.brushes.alpha_brush(ink, 0.045 * alpha);
        let border = self.brushes.alpha_brush(
            ink,
            if self.high_contrast.get() { 0.72 } else { 0.16 } * alpha,
        );
        self.draw_rounded_box(body, radius, &fill);
        self.draw_rounded_box_stroke(body, radius, &border, 1.0);
        let center = center_of(body);
        let icon_r = scale(8, self.dpi) as f32;
        let icon_brush = self.brushes.alpha_brush(ink, 0.72 * alpha);
        self.draw_tasks_glyph(
            Vector2::new(center.X, body.y as f32 + scale(25, self.dpi) as f32),
            icon_r,
            &icon_brush,
        );
        let title_brush = self.brushes.alpha_brush(ink, 0.90 * alpha);
        let title_rect = D2D_RECT_F {
            left: body.x as f32 + scale(12, self.dpi) as f32,
            top: body.y as f32 + scale(41, self.dpi) as f32,
            right: body.right() as f32 - scale(12, self.dpi) as f32,
            bottom: body.y as f32 + scale(61, self.dpi) as f32,
        };
        let title = if editing {
            "Type a note above"
        } else {
            "No tasks yet"
        };
        self.draw_text(title, &self.text_tab, title_rect, &title_brush);
        let hint_brush = self.brushes.alpha_brush(ink, 0.56 * alpha);
        let hint_rect = D2D_RECT_F {
            left: body.x as f32 + scale(16, self.dpi) as f32,
            top: body.y as f32 + scale(61, self.dpi) as f32,
            right: body.right() as f32 - scale(16, self.dpi) as f32,
            bottom: body.bottom() as f32 - scale(6, self.dpi) as f32,
        };
        let hint = if editing {
            TASKS_EDITING_HINT
        } else {
            TASKS_HINT
        };
        self.draw_text(hint, &self.text_caption_center, hint_rect, &hint_brush);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_task_row(
        &self,
        row: Rect,
        check: Rect,
        edit: Rect,
        remove: Rect,
        slot: usize,
        item: &TaskItem,
        is_editing: bool,
        alpha: f32,
        hovered: Option<HitTarget>,
    ) {
        let ink = self.notch_ink();
        let slot_index = u8::try_from(slot).ok();
        let row_hovered = slot_index.is_some_and(|idx| {
            matches!(
                hovered,
                Some(
                    HitTarget::TaskItem(s)
                        | HitTarget::TaskEdit(s)
                        | HitTarget::TaskRemove(s)
                        | HitTarget::TaskCheck(s)
                ) if s == idx
            )
        });
        let edit_hovered = slot_index.is_some_and(|idx| hovered == Some(HitTarget::TaskEdit(idx)));
        let remove_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::TaskRemove(idx)));
        let check_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::TaskCheck(idx)));

        let radius = scale(8, self.dpi) as f32;
        let (fill, border) = if is_editing {
            (
                self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.24 * alpha),
                self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.78 * alpha),
            )
        } else if item.active {
            (
                self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.22 * alpha),
                self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.62 * alpha),
            )
        } else if row_hovered {
            (
                self.brushes.alpha_brush(ink, 0.13 * alpha),
                self.brushes.alpha_brush(ink, 0.24 * alpha),
            )
        } else {
            (
                self.brushes.alpha_brush(ink, 0.065 * alpha),
                self.brushes.alpha_brush(ink, 0.11 * alpha),
            )
        };
        self.draw_rounded_box(row, radius, &fill);
        self.draw_rounded_box_stroke(row, radius, &border, 1.0);

        self.draw_task_check(check, item.done, check_hovered, alpha);

        let text_left = check.right() + scale(7, self.dpi);
        let text_right = if !edit.is_empty() {
            edit.x - scale(6, self.dpi)
        } else if !remove.is_empty() {
            remove.x - scale(6, self.dpi)
        } else {
            row.right() - scale(8, self.dpi)
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
            } else if row_hovered || item.active || is_editing {
                0.96
            } else {
                0.88
            };
            let brush = self.brushes.alpha_brush(ink, text_alpha * alpha);
            self.draw_text(&item.text, &self.text_pomodoro_sub, text_rect, &brush);
        }
        if !edit.is_empty() {
            self.draw_mini_edit_button(edit, edit_hovered || is_editing, row_hovered, alpha);
        }
        if !remove.is_empty() {
            self.draw_mini_remove_button(remove, remove_hovered, row_hovered, alpha);
        }
    }

    fn draw_mini_edit_button(&self, rect: Rect, active: bool, row_hovered: bool, alpha: f32) {
        let ink = self.notch_ink();
        let radius = (rect.height as f32 * 0.5).max(4.0);
        if active {
            let bg = self.brushes.alpha_brush(SHELF_URL_INDIGO, 0.28 * alpha);
            self.draw_rounded_box(rect, radius, &bg);
        } else if row_hovered {
            let bg = self.brushes.alpha_brush(ink, 0.08 * alpha);
            self.draw_rounded_box(rect, radius, &bg);
        }
        let color = if active { SHELF_URL_INDIGO } else { ink };
        let icon_alpha = if active {
            0.96
        } else if row_hovered {
            0.76
        } else {
            0.42
        };
        let fg = self.brushes.alpha_brush(color, icon_alpha * alpha);
        let c = center_of(rect);
        let r = (rect.width as f32 * 0.25).max(3.0);
        self.draw_pencil_glyph(c.X, c.Y, r, &fg);
    }

    fn draw_pencil_glyph(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (r * 0.34).max(1.1);
        let tip = Vector2::new(cx - r * 0.92, cy + r * 0.92);
        let body_start = Vector2::new(cx - r * 0.42, cy + r * 0.42);
        let body_end = Vector2::new(cx + r * 0.72, cy - r * 0.72);
        let cap_left = Vector2::new(cx + r * 0.34, cy - r * 0.88);
        let cap_right = Vector2::new(cx + r * 0.88, cy - r * 0.34);
        unsafe {
            self.target.DrawLine(
                body_start,
                body_end,
                brush,
                stroke * 1.35,
                &self.stroke_style,
            );
            self.target
                .DrawLine(tip, body_start, brush, stroke * 0.9, &self.stroke_style);
            self.target
                .DrawLine(cap_left, cap_right, brush, stroke, &self.stroke_style);
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
                let line = self.brushes.alpha_brush(self.notch_ink(), tone * alpha);
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
