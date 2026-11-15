use otto_core::{ClipboardItem, ClipboardKind, ClipboardState, HitTarget, Layout, Rect, scale};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::shelf::{
    SHELF_CLEAR_RED, SHELF_FOLDER_BLUE, SHELF_TEXT_SLATE, SHELF_URL_INDIGO, center_of,
};
use crate::{AMBER_WARN, CHARGING_GREEN, FOREGROUND, Renderer, rect_to_d2d, translate_rect};

fn clipboard_kind_tint(kind: ClipboardKind) -> D2D1_COLOR_F {
    match kind {
        ClipboardKind::Link => SHELF_URL_INDIGO,
        ClipboardKind::Files => SHELF_FOLDER_BLUE,
        ClipboardKind::Text => SHELF_TEXT_SLATE,
    }
}

impl Renderer {
    pub(crate) fn draw_expanded_clipboard(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        clipboard_state: &ClipboardState,
    ) -> Result<()> {
        self.draw_clipboard_header_controls(layout, offset_x, alpha, hovered, clipboard_state);
        if clipboard_state.items.is_empty() {
            self.draw_clipboard_empty_state(layout, offset_x, alpha, clipboard_state.paused);
            return Ok(());
        }
        let visible = clipboard_state.visible_items();
        for (slot, item) in visible.iter().enumerate() {
            let Some(&raw_row) = layout.clipboard_rows.get(slot) else {
                break;
            };
            if raw_row.is_empty() {
                continue;
            }
            let row = translate_rect(raw_row, -offset_x);
            let remove = translate_rect(
                layout
                    .clipboard_remove_buttons
                    .get(slot)
                    .copied()
                    .unwrap_or_default(),
                -offset_x,
            );
            let copied = clipboard_state.copied_feedback_id == Some(item.id);
            self.draw_clipboard_row(row, remove, slot, item, copied, alpha, hovered)?;
        }
        Ok(())
    }

    fn draw_clipboard_header_controls(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        clipboard_state: &ClipboardState,
    ) {
        if !layout.clipboard_pager_button.is_empty() {
            let pager = translate_rect(layout.clipboard_pager_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::ClipboardPager);
            let fill = self
                .brushes
                .alpha_brush(FOREGROUND, if is_hovered { 0.17 } else { 0.09 } * alpha);
            let border = self.brushes.alpha_brush(FOREGROUND, 0.15 * alpha);
            self.draw_shelf_pill(pager, &fill, Some(&border));
            let label = format!(
                "{}/{}",
                clipboard_state.clamped_page() + 1,
                clipboard_state.page_count().max(1)
            );
            let fg = self.brushes.alpha_brush(FOREGROUND, 0.90 * alpha);
            self.draw_text(&label, &self.text_tab, rect_to_d2d(pager), &fg);
        }

        if !layout.clipboard_pause_button.is_empty() {
            let pause = translate_rect(layout.clipboard_pause_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::ClipboardPauseToggle);
            let (tint, label) = if clipboard_state.paused {
                (AMBER_WARN, "Paused")
            } else {
                (CHARGING_GREEN, "Live")
            };
            let fill = self
                .brushes
                .alpha_brush(tint, if is_hovered { 0.24 } else { 0.14 } * alpha);
            let border = self
                .brushes
                .alpha_brush(tint, if is_hovered { 0.56 } else { 0.36 } * alpha);
            self.draw_shelf_pill(pause, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(tint, 0.94 * alpha);
            self.draw_text(label, &self.text_tab, rect_to_d2d(pause), &fg);
        }

        if !layout.clipboard_clear_button.is_empty() && !clipboard_state.items.is_empty() {
            let clear = translate_rect(layout.clipboard_clear_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::ClipboardClear);
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

    fn draw_clipboard_empty_state(&self, layout: &Layout, offset_x: i32, alpha: f32, paused: bool) {
        let body = translate_rect(layout.clipboard_body, -offset_x);
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
        let border = self.brushes.alpha_brush(
            if paused { AMBER_WARN } else { FOREGROUND },
            if paused { 0.34 } else { 0.16 } * alpha,
        );
        unsafe {
            self.target.FillRoundedRectangle(&card, &fill);
            self.target.DrawRoundedRectangle(&card, &border, 1.0, None);
        }

        let center = center_of(body);
        let icon_r = scale(8, self.dpi) as f32;
        let stroke = (icon_r * 0.22).max(1.3);
        let icon_brush = self.brushes.alpha_brush(
            if paused { AMBER_WARN } else { FOREGROUND },
            if paused { 0.86 } else { 0.72 } * alpha,
        );
        self.draw_clipboard_glyph(
            Vector2::new(center.X, body.y as f32 + scale(25, self.dpi) as f32),
            icon_r,
            stroke,
            &icon_brush,
        );

        let title = if paused {
            "Clipboard capture is paused"
        } else {
            "Clipboard history is empty"
        };
        let subtitle = if paused {
            "Click Paused above to resume monitoring"
        } else {
            "Copy text, links, or files to keep them here"
        };
        let title_brush = self.brushes.alpha_brush(FOREGROUND, 0.90 * alpha);
        let title_rect = D2D_RECT_F {
            left: body.x as f32 + scale(12, self.dpi) as f32,
            top: body.y as f32 + scale(41, self.dpi) as f32,
            right: body.right() as f32 - scale(12, self.dpi) as f32,
            bottom: body.y as f32 + scale(61, self.dpi) as f32,
        };
        self.draw_text(title, &self.text_tab, title_rect, &title_brush);

        let hint_brush = self.brushes.alpha_brush(FOREGROUND, 0.56 * alpha);
        let hint_rect = D2D_RECT_F {
            left: body.x as f32 + scale(16, self.dpi) as f32,
            top: body.y as f32 + scale(61, self.dpi) as f32,
            right: body.right() as f32 - scale(16, self.dpi) as f32,
            bottom: body.bottom() as f32 - scale(6, self.dpi) as f32,
        };
        self.draw_text(subtitle, &self.text_caption_center, hint_rect, &hint_brush);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_clipboard_row(
        &self,
        row: Rect,
        remove: Rect,
        slot: usize,
        item: &ClipboardItem,
        copied: bool,
        alpha: f32,
        hovered: Option<HitTarget>,
    ) -> Result<()> {
        let slot_index = u8::try_from(slot).ok();
        let row_hovered = slot_index.is_some_and(|idx| {
            matches!(
                hovered,
                Some(HitTarget::ClipboardItem(s) | HitTarget::ClipboardRemoveItem(s)) if s == idx
            )
        });
        let remove_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::ClipboardRemoveItem(idx)));

        let radius = scale(7, self.dpi) as f32;
        let shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(row),
            radiusX: radius,
            radiusY: radius,
        };
        let (fill, border) = if copied {
            (
                self.brushes.alpha_brush(CHARGING_GREEN, 0.22 * alpha),
                self.brushes.alpha_brush(CHARGING_GREEN, 0.60 * alpha),
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

        self.draw_clipboard_row_kind_glyph(row, item.kind, copied, alpha)?;

        let meta_w = scale(48, self.dpi);
        let remove_gap = scale(6, self.dpi);
        let meta_right = if remove.is_empty() {
            row.right() - remove_gap
        } else {
            remove.x - remove_gap
        };
        let meta_left = (meta_right - meta_w).max(row.x + scale(24, self.dpi));
        let meta_rect = D2D_RECT_F {
            left: meta_left as f32,
            top: row.y as f32,
            right: meta_right as f32,
            bottom: row.bottom() as f32,
        };
        let (meta_text, meta_brush) = if copied {
            (
                "Copied",
                self.brushes.alpha_brush(CHARGING_GREEN, 0.96 * alpha),
            )
        } else {
            (
                item.meta.as_str(),
                self.brushes.alpha_brush(FOREGROUND, 0.56 * alpha),
            )
        };
        self.draw_text(
            meta_text,
            &self.text_pomodoro_sub_trailing,
            meta_rect,
            &meta_brush,
        );

        let text_left = row.x + scale(25, self.dpi);
        let text_right = (meta_left - remove_gap).max(text_left);
        if text_right > text_left {
            let preview_rect = D2D_RECT_F {
                left: text_left as f32,
                top: row.y as f32,
                right: text_right as f32,
                bottom: row.bottom() as f32,
            };
            let preview_brush = self.brushes.alpha_brush(
                FOREGROUND,
                if row_hovered || copied { 0.96 } else { 0.88 } * alpha,
            );
            self.draw_text(
                &item.preview,
                &self.text_pomodoro_sub,
                preview_rect,
                &preview_brush,
            );
        }

        if !remove.is_empty() {
            self.draw_mini_remove_button(remove, remove_hovered, row_hovered, alpha);
        }
        Ok(())
    }

    fn draw_clipboard_row_kind_glyph(
        &self,
        row: Rect,
        kind: ClipboardKind,
        copied: bool,
        alpha: f32,
    ) -> Result<()> {
        let badge_x = row.x as f32 + scale(13, self.dpi) as f32;
        let center_y = row.y as f32 + row.height as f32 * 0.5;
        let icon_r = scale(4, self.dpi) as f32;
        let icon_stroke = (icon_r * 0.26).max(1.2);
        let tint = if copied {
            CHARGING_GREEN
        } else {
            clipboard_kind_tint(kind)
        };
        let icon_brush = self.brushes.alpha_brush(tint, 0.92 * alpha);
        match kind {
            ClipboardKind::Link => {
                self.draw_url_kind_glyph(badge_x, center_y, icon_r, &icon_brush, icon_stroke);
            }
            ClipboardKind::Files => {
                let geom = self.build_folder_geometry(badge_x, center_y, icon_r)?;
                unsafe {
                    self.target.FillGeometry(&geom, &icon_brush, None);
                }
            }
            ClipboardKind::Text => {
                self.draw_text_kind_glyph(badge_x, center_y, icon_r, &icon_brush, icon_stroke);
            }
        }
        Ok(())
    }
}
