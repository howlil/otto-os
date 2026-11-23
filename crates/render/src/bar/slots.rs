use otto_core::{Layout, Rect, ShapeTokens, SystemStatus, scale};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::Win32::Graphics::DirectWrite::IDWriteTextFormat;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::CHARGING_GREEN;
use crate::Renderer;
use crate::kit::glyphs::draw_speaker_glyph;
use crate::kit::glyphs::draw_wifi_glyph;
use crate::rect_to_d2d;

impl Renderer {
    pub(crate) fn draw_left_menu_bar(
        &self,
        layout: &Layout,
        app_name: &str,
        fg: &ID2D1SolidColorBrush,
    ) {
        self.draw_slot_label(layout.slots.app_slot, app_name, &self.text_app_bold, fg);
        for (slot, label) in [
            (layout.slots.file_slot, "File"),
            (layout.slots.edit_slot, "Edit"),
            (layout.slots.view_slot, "View"),
            (layout.slots.window_slot, "Window"),
            (layout.slots.help_slot, "Help"),
        ] {
            self.draw_slot_label(slot, label, &self.text_menu, fg);
        }
    }

    pub(crate) fn draw_slot_label(
        &self,
        slot: Rect,
        label: &str,
        format: &IDWriteTextFormat,
        brush: &ID2D1SolidColorBrush,
    ) {
        if slot.width <= 0 {
            return;
        }
        let rect = D2D_RECT_F {
            left: slot.x as f32,
            top: 0.0,
            right: slot.right() as f32,
            bottom: slot.bottom() as f32,
        };
        self.draw_text(label, format, rect, brush);
    }

    pub(crate) fn draw_search_slot(&self, layout: &Layout, fg: &ID2D1SolidColorBrush) {
        let slot = layout.slots.search_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let r = unit * 0.34;
        let stroke = (unit * 0.11).max(1.4);
        let center = Vector2::new(cx - unit * 0.07, cy - unit * 0.07);

        unsafe {
            let circle = D2D1_ELLIPSE {
                point: center,
                radiusX: r,
                radiusY: r,
            };
            self.target
                .DrawEllipse(&circle, fg, stroke, &self.stroke_style);

            let diag = 0.707_106_77_f32;
            let start = Vector2::new(cx - unit * 0.07 + r * diag, cy - unit * 0.07 + r * diag);
            let end = Vector2::new(
                cx - unit * 0.07 + (r + unit * 0.28) * diag,
                cy - unit * 0.07 + (r + unit * 0.28) * diag,
            );
            self.target
                .DrawLine(start, end, fg, stroke * 1.15, &self.stroke_style);
        }
    }

    pub(crate) fn draw_control_center_slot(
        &self,
        layout: &Layout,
        fg: &ID2D1SolidColorBrush,
        muted: &ID2D1SolidColorBrush,
    ) {
        let slot = layout.slots.control_center_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let pill_w = unit * 0.94;
        let pill_h = unit * 0.36;
        let gap_y = unit * 0.24;
        let stroke = (unit * 0.095).max(1.25);

        for (dy, knob_right) in [(-gap_y, false), (gap_y, true)] {
            let row_cy = cy + dy;
            let left = cx - pill_w * 0.5;
            let right = cx + pill_w * 0.5;
            let radius = pill_h * 0.5;
            let pill = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left,
                    top: row_cy - radius,
                    right,
                    bottom: row_cy + radius,
                },
                radiusX: radius,
                radiusY: radius,
            };
            let knob_r = (radius - stroke * 0.45).max(1.8);
            let knob_x = if knob_right {
                right - radius
            } else {
                left + radius
            };
            let knob = D2D1_ELLIPSE {
                point: Vector2::new(knob_x, row_cy),
                radiusX: knob_r,
                radiusY: knob_r,
            };
            unsafe {
                self.target
                    .DrawRoundedRectangle(&pill, muted, stroke, &self.stroke_style);
                self.target.FillEllipse(&knob, fg);
            }
        }
    }

    pub(crate) fn draw_slot_pill(&self, slot: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let pill = ShapeTokens::GOLDEN_GATE.bar_slot_pill_rect(slot, self.dpi);
        let radius = shape.slot_radius as f32;
        let rect = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(pill),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&rect, brush);
        }
    }

    pub(crate) fn draw_slot_focus_ring(&self, slot: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let ring = ShapeTokens::GOLDEN_GATE.bar_slot_focus_rect(slot, self.dpi);
        let radius = shape.focus_ring_radius as f32;
        let stroke = shape.focus_ring_width as f32;
        let inset = stroke * 0.5;
        let rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: ring.x as f32 + inset,
                top: ring.y as f32 + inset,
                right: ring.right() as f32 - inset,
                bottom: ring.bottom() as f32 - inset,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&rect, brush, stroke, &self.stroke_style);
        }
    }

    pub(crate) fn draw_apple_logo(
        &self,
        layout: &Layout,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let size = layout.icon_size as f32 * 1.08;
        let x0 =
            layout.slots.start_slot.x as f32 + (layout.slots.start_slot.width as f32 - size) * 0.5;
        let y0 = (layout.slots.start_slot.height as f32 - size) * 0.5;
        let pt = |nx: f32, ny: f32| Vector2::new(x0 + nx * size, y0 + ny * size);
        let bez = |x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32| D2D1_BEZIER_SEGMENT {
            point1: pt(x1, y1),
            point2: pt(x2, y2),
            point3: pt(x3, y3),
        };

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;

            sink.BeginFigure(pt(0.67, 0.02), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&bez(0.55, 0.03, 0.46, 0.12, 0.48, 0.24));
            sink.AddBezier(&bez(0.59, 0.24, 0.68, 0.15, 0.67, 0.02));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);

            sink.BeginFigure(pt(0.49, 0.29), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&bez(0.43, 0.29, 0.38, 0.24, 0.32, 0.24));
            sink.AddBezier(&bez(0.19, 0.24, 0.10, 0.36, 0.12, 0.54));
            sink.AddBezier(&bez(0.14, 0.72, 0.24, 0.94, 0.33, 0.94));
            sink.AddBezier(&bez(0.39, 0.94, 0.42, 0.89, 0.49, 0.89));
            sink.AddBezier(&bez(0.56, 0.89, 0.59, 0.94, 0.65, 0.94));
            sink.AddBezier(&bez(0.73, 0.94, 0.81, 0.81, 0.86, 0.69));
            sink.AddBezier(&bez(0.73, 0.63, 0.71, 0.43, 0.82, 0.36));
            sink.AddBezier(&bez(0.77, 0.28, 0.70, 0.24, 0.64, 0.24));
            sink.AddBezier(&bez(0.57, 0.24, 0.54, 0.29, 0.49, 0.29));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);

            sink.Close()?;
            self.target.FillGeometry(&geometry, brush, None);
        }
        Ok(())
    }

    pub(crate) fn draw_tray_chevron(
        &self,
        layout: &Layout,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let slot = layout.slots.tray_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let dx = unit * 0.30;
        let dy = unit * 0.16;
        let stroke = (unit * 0.11).max(1.4);

        unsafe {
            let geometry = self.factory.CreatePathGeometry()?;
            let sink = geometry.Open()?;
            sink.BeginFigure(Vector2::new(cx - dx, cy + dy), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddLine(Vector2::new(cx, cy - dy));
            sink.AddLine(Vector2::new(cx + dx, cy + dy));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geometry, brush, stroke, &self.stroke_style);
        }
        Ok(())
    }

    pub(crate) fn draw_status_slot(
        &self,
        layout: &Layout,
        status: SystemStatus,
        fg: &ID2D1SolidColorBrush,
        muted: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let slot = layout.slots.status_slot;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let stroke = (unit * 0.105).max(1.35);

        let wifi_cx = slot.x as f32 + unit * 0.80;
        self.draw_wifi_icon(wifi_cx, cy, unit * 0.48, stroke, fg)?;

        let speaker_x = wifi_cx + unit * 0.88;
        self.draw_speaker_icon(
            speaker_x,
            cy,
            unit * 0.44,
            stroke,
            status.volume,
            status.muted,
            fg,
        )?;

        let bat_x = speaker_x + unit * 1.38;
        let bat_w = unit * 1.40;
        let bat_h = unit * 0.72;
        let bat_top = cy - bat_h * 0.5;
        let corner = (bat_h * 0.28).max(2.0);

        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bat_x,
                top: bat_top,
                right: bat_x + bat_w,
                bottom: bat_top + bat_h,
            },
            radiusX: corner,
            radiusY: corner,
        };
        let cap_h = bat_h * 0.40;
        let cap_w = (stroke * 1.15).max(1.5);
        let cap = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bat_x + bat_w + 1.2,
                top: cy - cap_h * 0.5,
                right: bat_x + bat_w + 1.2 + cap_w,
                bottom: cy + cap_h * 0.5,
            },
            radiusX: cap_w * 0.5,
            radiusY: cap_w * 0.5,
        };

        let inset = (stroke * 1.30).max(1.8);
        let max_fill_w = (bat_w - inset * 2.0).max(0.0);
        let fill_w = (max_fill_w * status.battery.fill_fraction()).max(if max_fill_w > 0.0 {
            1.5
        } else {
            0.0
        });
        let fill_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: bat_x + inset,
                top: bat_top + inset,
                right: bat_x + inset + fill_w,
                bottom: bat_top + bat_h - inset,
            },
            radiusX: (corner - 1.2).max(1.0),
            radiusY: (corner - 1.2).max(1.0),
        };

        unsafe {
            self.target
                .DrawRoundedRectangle(&body, muted, stroke * 0.9, &self.stroke_style);
            self.target.FillRoundedRectangle(&cap, muted);
            if status.battery.charging {
                let green = &self.brushes.color_brush(CHARGING_GREEN);
                self.target.FillRoundedRectangle(&fill_rect, green);
                self.draw_charging_bolt(bat_x + bat_w * 0.5, cy, bat_h * 0.42, fg)?;
            } else {
                self.target.FillRoundedRectangle(&fill_rect, fg);
            }
        }

        let text_left = bat_x + bat_w + cap_w + unit * 0.34;
        let text_rect = D2D_RECT_F {
            left: text_left,
            top: 0.0,
            right: slot.right() as f32,
            bottom: slot.height as f32,
        };
        let label = status.battery.label();
        self.draw_text(&label, &self.text_leading, text_rect, fg);
        Ok(())
    }

    pub(crate) fn draw_charging_bolt(
        &self,
        cx: f32,
        cy: f32,
        half_h: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let w = half_h * 0.65;
        unsafe {
            let bolt = self.factory.CreatePathGeometry()?;
            let sink = bolt.Open()?;
            sink.BeginFigure(
                Vector2::new(cx + w * 0.15, cy - half_h),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(cx - w, cy + half_h * 0.10));
            sink.AddLine(Vector2::new(cx - w * 0.05, cy + half_h * 0.10));
            sink.AddLine(Vector2::new(cx - w * 0.15, cy + half_h));
            sink.AddLine(Vector2::new(cx + w, cy - half_h * 0.10));
            sink.AddLine(Vector2::new(cx + w * 0.05, cy - half_h * 0.10));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&bolt, brush, None);
        }
        Ok(())
    }

    pub(crate) fn draw_wifi_icon(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = stroke.max(1.0);
        draw_wifi_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            cx,
            cy,
            radius,
            stroke,
            brush,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_speaker_icon(
        &self,
        x: f32,
        cy: f32,
        half_h: f32,
        stroke: f32,
        volume: u8,
        muted: bool,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        draw_speaker_glyph(
            &self.factory,
            &self.target,
            &self.stroke_style,
            x,
            cy,
            half_h,
            stroke,
            volume,
            muted,
            brush,
        )
    }
}

impl Renderer {
    pub(crate) fn draw_clock_slot(
        &self,
        layout: &Layout,
        status: SystemStatus,
        fg: &ID2D1SolidColorBrush,
    ) {
        let slot = layout.slots.clock_slot;
        let pad_x = scale(6, self.dpi) as f32;
        let text_rect = D2D_RECT_F {
            left: slot.x as f32 + pad_x,
            top: 0.0,
            right: slot.right() as f32 - pad_x,
            bottom: slot.height as f32,
        };
        let text = status.clock.format();
        self.draw_tabular_text(&text, &self.text_leading, text_rect, fg);
    }

    pub(crate) fn draw_notification_slot(
        &self,
        layout: &Layout,
        fg: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let slot = layout.slots.notification_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let r = layout.icon_size as f32 * 0.42;
        self.draw_bell_icon(cx, cy, r, fg)?;
        if self.notif_unread.get() {
            let dot_r = (scale(5, self.dpi) as f32 * 0.5).max(2.2);
            let dot = D2D1_ELLIPSE {
                point: Vector2::new(cx + r * 0.72, cy - r * 0.64),
                radiusX: dot_r,
                radiusY: dot_r,
            };
            let accent = self.brushes.alpha_brush(crate::rgb(255, 69, 58, 1.0), 1.0);
            unsafe {
                self.target.FillEllipse(&dot, &accent);
            }
        }
        Ok(())
    }

    pub(crate) fn draw_bell_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        crate::draw_bell_glyph(&self.factory, &self.target, cx, cy, r, brush)
    }
}
