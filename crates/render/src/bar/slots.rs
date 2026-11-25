use otto_core::{BrandIcon, Layout, Rect, ShapeTokens, SystemStatus, scale};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::Win32::Graphics::DirectWrite::IDWriteTextFormat;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::CHARGING_GREEN;
use crate::Renderer;
use crate::appearance::tokens::SYSTEM_RED;
use crate::kit::draw::{draw_focus_ring_for, fill_squircle_or_rounded};
use crate::kit::glyphs::draw_speaker_glyph;
use crate::kit::glyphs::draw_wifi_glyph;
use crate::kit::icons::{
    ChevronDir, draw_chevron_glyph, draw_double_chevron_glyph, draw_search_magnifier_glyph,
};
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

    pub(crate) fn draw_overflow_slot(&self, layout: &Layout, fg: &ID2D1SolidColorBrush) {
        let slot = layout.slots.overflow_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let reach = (unit * 0.28).max(3.2);
        let stroke = (unit * 0.11).max(1.4);
        draw_double_chevron_glyph(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            reach,
            stroke,
            fg,
        );
    }

    pub(crate) fn draw_search_slot(&self, layout: &Layout, fg: &ID2D1SolidColorBrush) {
        let slot = layout.slots.search_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let r = unit * 0.34;
        let stroke = (unit * 0.11).max(1.4);
        draw_search_magnifier_glyph(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            r,
            stroke,
            fg,
        );
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
        fill_squircle_or_rounded(
            &self.factory,
            &self.target,
            pill,
            shape.slot_radius as f32,
            self.dpi,
            brush,
        );
    }

    pub(crate) fn draw_slot_focus_ring(&self, slot: Rect, brush: &ID2D1SolidColorBrush) {
        let shape = ShapeTokens::GOLDEN_GATE.scaled(self.dpi);
        let ring = ShapeTokens::GOLDEN_GATE.bar_slot_focus_rect(slot, self.dpi);
        draw_focus_ring_for(
            &self.factory,
            &self.target,
            rect_to_d2d(ring),
            shape.focus_ring_radius as f32,
            self.dpi,
            brush,
        );
    }

    pub(crate) fn draw_brand_logo(
        &self,
        icon: BrandIcon,
        layout: &Layout,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        match icon {
            BrandIcon::Otto => Ok(()),
            BrandIcon::ClassicApple => {
                let (leaf, body) = apple_logo_curves(layout);
                self.fill_bezier_layers(brush, &[(&[leaf.as_slice(), body.as_slice()], 1.0)])
            }
            BrandIcon::Windows => {
                let (top_panes, bottom_panes) = windows_logo_panes(layout);
                let top_figs = top_panes.map(|(pts, radii)| quad_fillet_figure(pts, radii));
                let bottom_figs = bottom_panes.map(|(pts, radii)| quad_fillet_figure(pts, radii));
                let top_refs = [top_figs[0].as_slice(), top_figs[1].as_slice()];
                let bottom_refs = [bottom_figs[0].as_slice(), bottom_figs[1].as_slice()];
                self.fill_bezier_layers(
                    brush,
                    &[(top_refs.as_slice(), 1.0), (bottom_refs.as_slice(), 0.78)],
                )
            }
        }
    }

    fn fill_bezier_layers(
        &self,
        brush: &ID2D1SolidColorBrush,
        layers: &[BezierLayer<'_>],
    ) -> Result<()> {
        unsafe {
            let prev_opacity = brush.GetOpacity();
            for &(figures, opacity_scale) in layers {
                let geometry = self.factory.CreatePathGeometry()?;
                let sink = geometry.Open()?;
                for segments in figures {
                    let Some((Some(first_start), first_bez)) = segments.first() else {
                        continue;
                    };
                    sink.BeginFigure(*first_start, D2D1_FIGURE_BEGIN_FILLED);
                    sink.AddBezier(first_bez);
                    for (line_to, bez) in &segments[1..] {
                        if let Some(pt) = line_to {
                            sink.AddLine(*pt);
                        }
                        sink.AddBezier(bez);
                    }
                    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                }
                sink.Close()?;
                brush.SetOpacity(prev_opacity * opacity_scale);
                self.target.FillGeometry(&geometry, brush, None);
            }
            brush.SetOpacity(prev_opacity);
        }
        Ok(())
    }

    pub(crate) fn draw_tray_chevron(&self, layout: &Layout, brush: &ID2D1SolidColorBrush) {
        let slot = layout.slots.tray_slot;
        let cx = slot.x as f32 + slot.width as f32 * 0.5;
        let cy = slot.height as f32 * 0.5;
        let unit = layout.icon_size as f32;
        let reach = (unit * 0.35).max(3.6);
        let stroke = (unit * 0.11).max(1.4);
        draw_chevron_glyph(
            &self.target,
            &self.stroke_style,
            Vector2::new(cx, cy),
            reach,
            stroke,
            ChevronDir::Up,
            brush,
        );
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
        self.draw_tabular_text(&label, &self.text_leading, text_rect, fg);
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
            let dot_color = if self.high_contrast.get() {
                self.brushes.toggle_active()
            } else {
                SYSTEM_RED
            };
            let accent = self.brushes.color_brush(dot_color);
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

type BezierStep = (Option<Vector2>, D2D1_BEZIER_SEGMENT);
type BezierLayer<'a> = (&'a [&'a [BezierStep]], f32);
type PaneSpec = ([Vector2; 4], [f32; 4]);

fn apple_logo_curves(layout: &Layout) -> ([BezierStep; 2], [BezierStep; 9]) {
    let size = layout.icon_size as f32 * 1.08;
    let x0 = layout.slots.start_slot.x as f32 + (layout.slots.start_slot.width as f32 - size) * 0.5;
    let y0 = (layout.slots.start_slot.height as f32 - size) * 0.5;
    let pt = |nx: f32, ny: f32| Vector2::new(x0 + nx * size, y0 + ny * size);
    let bez = |x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32| D2D1_BEZIER_SEGMENT {
        point1: pt(x1, y1),
        point2: pt(x2, y2),
        point3: pt(x3, y3),
    };
    let leaf = [
        (
            Some(pt(0.67, 0.02)),
            bez(0.55, 0.03, 0.46, 0.12, 0.48, 0.24),
        ),
        (None, bez(0.59, 0.24, 0.68, 0.15, 0.67, 0.02)),
    ];
    let body = [
        (
            Some(pt(0.49, 0.29)),
            bez(0.43, 0.29, 0.38, 0.24, 0.32, 0.24),
        ),
        (None, bez(0.19, 0.24, 0.10, 0.36, 0.12, 0.54)),
        (None, bez(0.14, 0.72, 0.24, 0.94, 0.33, 0.94)),
        (None, bez(0.39, 0.94, 0.42, 0.89, 0.49, 0.89)),
        (None, bez(0.56, 0.89, 0.59, 0.94, 0.65, 0.94)),
        (None, bez(0.73, 0.94, 0.81, 0.81, 0.86, 0.69)),
        (None, bez(0.73, 0.63, 0.71, 0.43, 0.82, 0.36)),
        (None, bez(0.77, 0.28, 0.70, 0.24, 0.64, 0.24)),
        (None, bez(0.57, 0.24, 0.54, 0.29, 0.49, 0.29)),
    ];
    (leaf, body)
}

fn windows_logo_panes(layout: &Layout) -> ([PaneSpec; 2], [PaneSpec; 2]) {
    let slot = layout.slots.start_slot;
    let size = layout.icon_size as f32 * 0.94;
    let x0 = slot.x as f32 + (slot.width as f32 - size) * 0.5;
    let y0 = slot.y as f32 + (slot.height as f32 - size) * 0.5;
    let pt = |nx: f32, ny: f32| Vector2::new(x0 + nx * size, y0 + ny * size);
    let top_y = |nx: f32| 0.16 - 0.10 * ((nx - 0.06) / 0.88);
    let bot_y = |nx: f32| 0.84 + 0.10 * ((nx - 0.06) / 0.88);
    let r_out = size * 0.075;
    let r_in = size * 0.028;

    let top = [
        (
            [
                pt(0.06, top_y(0.06)),
                pt(0.425, top_y(0.425)),
                pt(0.425, 0.465),
                pt(0.06, 0.465),
            ],
            [r_out, r_in, r_in, r_in],
        ),
        (
            [
                pt(0.495, top_y(0.495)),
                pt(0.94, top_y(0.94)),
                pt(0.94, 0.465),
                pt(0.495, 0.465),
            ],
            [r_in, r_out, r_in, r_in],
        ),
    ];
    let bottom = [
        (
            [
                pt(0.06, 0.535),
                pt(0.425, 0.535),
                pt(0.425, bot_y(0.425)),
                pt(0.06, bot_y(0.06)),
            ],
            [r_in, r_in, r_in, r_out],
        ),
        (
            [
                pt(0.495, 0.535),
                pt(0.94, 0.535),
                pt(0.94, bot_y(0.94)),
                pt(0.495, bot_y(0.495)),
            ],
            [r_in, r_in, r_out, r_in],
        ),
    ];
    (top, bottom)
}

fn quad_fillet_figure(pts: [Vector2; 4], radii: [f32; 4]) -> [BezierStep; 4] {
    [
        quad_corner_fillet(pts[3], pts[0], pts[1], radii[0]),
        quad_corner_fillet(pts[0], pts[1], pts[2], radii[1]),
        quad_corner_fillet(pts[1], pts[2], pts[3], radii[2]),
        quad_corner_fillet(pts[2], pts[3], pts[0], radii[3]),
    ]
}

fn quad_corner_fillet(prev: Vector2, curr: Vector2, next: Vector2, radius: f32) -> BezierStep {
    let v1 = Vector2::new(prev.X - curr.X, prev.Y - curr.Y);
    let v2 = Vector2::new(next.X - curr.X, next.Y - curr.Y);
    let len1 = v1.X.hypot(v1.Y).max(1e-4);
    let len2 = v2.X.hypot(v2.Y).max(1e-4);
    let d = radius.min(len1 * 0.42).min(len2 * 0.42);
    let a = Vector2::new(curr.X + v1.X / len1 * d, curr.Y + v1.Y / len1 * d);
    let b = Vector2::new(curr.X + v2.X / len2 * d, curr.Y + v2.Y / len2 * d);
    let c1 = Vector2::new(
        a.X + (curr.X - a.X) * (2.0 / 3.0),
        a.Y + (curr.Y - a.Y) * (2.0 / 3.0),
    );
    let c2 = Vector2::new(
        b.X + (curr.X - b.X) * (2.0 / 3.0),
        b.Y + (curr.Y - b.Y) * (2.0 / 3.0),
    );
    (
        Some(a),
        D2D1_BEZIER_SEGMENT {
            point1: c1,
            point2: c2,
            point3: b,
        },
    )
}
