use otto_core::{HudGlyph, HudTone, HudValue, Layout, NotchHudState, Rect, present_hud, scale};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
    D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1SolidColorBrush,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::AMBER_WARN;
use crate::BLACK;
use crate::CHARGING_GREEN;
use crate::FOREGROUND;
use crate::Renderer;
use crate::draw_bluetooth_symbol;
use crate::draw_crescent_moon;
use crate::rect_to_d2d;

impl Renderer {
    pub(crate) fn hud_tone_brush(&self, tone: HudTone, alpha: f32) -> ID2D1SolidColorBrush {
        match tone {
            HudTone::Neutral => self.brushes.alpha_brush(FOREGROUND, 0.95 * alpha),
            HudTone::Positive => self.brushes.alpha_brush(CHARGING_GREEN, 0.96 * alpha),
            HudTone::Warning => self.brushes.alpha_brush(AMBER_WARN, 0.96 * alpha),
            HudTone::Muted => self.brushes.alpha_brush(FOREGROUND, 0.58 * alpha),
        }
    }

    #[expect(clippy::too_many_lines)]
    pub(crate) fn draw_notch_hud(&self, layout: &Layout, hud: NotchHudState) -> Result<()> {
        let presentation = present_hud(hud.kind);
        let alpha = hud.content_alpha.clamp(0.0, 1.0);
        let geometry = layout.hud;
        let stroke = (layout.icon_size as f32 * 0.105).max(1.35);
        let tone = self.hud_tone_brush(presentation.tone, alpha);
        let label_brush = self.brushes.alpha_brush(FOREGROUND, 0.95 * alpha);
        let track = self.brushes.alpha_brush(FOREGROUND, 0.16 * alpha);

        let glyph = geometry.glyph;
        let gcx = glyph.x as f32 + glyph.width as f32 * 0.5;
        let gcy = glyph.y as f32 + glyph.height as f32 * 0.5;
        let half = glyph.width as f32 * 0.5;
        let volume = match presentation.value {
            HudValue::Percent(percent) => percent,
            HudValue::Muted | HudValue::Pill { .. } | HudValue::Text(_) | HudValue::Badge(_) => 0,
        };
        match presentation.glyph {
            HudGlyph::Speaker => {
                self.draw_speaker_icon(
                    glyph.x as f32,
                    gcy,
                    half * 0.9,
                    stroke,
                    volume,
                    false,
                    &tone,
                )?;
            }
            HudGlyph::SpeakerMuted => {
                self.draw_speaker_icon(glyph.x as f32, gcy, half * 0.9, stroke, 0, true, &tone)?;
            }
            HudGlyph::Sun => self.draw_sun_icon(gcx, gcy, half * 0.85, stroke, &tone),
            HudGlyph::Bolt => self.draw_charging_bolt(gcx, gcy, half * 0.9, &tone)?,
            HudGlyph::Battery => {
                let level = presentation.level.unwrap_or(0.0);
                self.draw_battery_glyph(gcx, gcy, half, stroke, level, &tone);
            }
            HudGlyph::CapsLock => self.draw_capslock_icon(gcx, gcy, half * 0.9, stroke, &tone)?,
            HudGlyph::NumLock => self.draw_numlock_icon(gcx, gcy, half * 0.9, stroke, &tone),
            HudGlyph::Keyboard => self.draw_keyboard_icon(gcx, gcy, half * 0.9, stroke, &tone),
            HudGlyph::BatterySaver => {
                self.draw_battery_saver_icon(gcx, gcy, half * 0.9, stroke, &tone)?;
            }
            HudGlyph::Moon => {
                draw_crescent_moon(
                    &self.factory,
                    &self.target,
                    Vector2::new(gcx, gcy),
                    half * 0.85,
                    &tone,
                )?;
            }
            HudGlyph::Bluetooth => {
                draw_bluetooth_symbol(
                    &self.factory,
                    &self.target,
                    &self.stroke_style,
                    gcx,
                    gcy,
                    half * 0.85,
                    &tone,
                )?;
            }
            HudGlyph::Headphones => {
                self.draw_headphones_icon(gcx, gcy, half * 0.88, stroke, &tone)?;
            }
            HudGlyph::Otto => {
                self.draw_otto_mark(gcx, gcy, half * 0.88, stroke, &tone);
            }
        }

        let label_rect = if presentation.level.is_some() {
            rect_to_d2d(geometry.label)
        } else {
            let max_right = match presentation.value {
                HudValue::Badge(text) => {
                    let pill_w = self.hud_badge_width(text);
                    let gap = scale(8, self.dpi) as f32;
                    (geometry.value.right() as f32 - pill_w - gap)
                        .min(geometry.meter.right() as f32)
                        .max(geometry.label.x as f32)
                }
                _ => geometry.meter.right() as f32,
            };
            D2D_RECT_F {
                left: geometry.label.x as f32,
                top: geometry.label.y as f32,
                right: max_right,
                bottom: geometry.label.bottom() as f32,
            }
        };
        self.draw_text(
            presentation.label.as_str(),
            &self.text_hud_label,
            label_rect,
            &label_brush,
        );

        if let Some(level) = presentation.level {
            self.draw_hud_meter(geometry.meter, level, &tone, &track);
        }

        match presentation.value {
            HudValue::Percent(percent) => self.draw_tabular_text(
                &format!("{percent}%"),
                &self.text_hud_value,
                rect_to_d2d(geometry.value),
                &tone,
            ),
            HudValue::Muted => self.draw_text(
                "Muted",
                &self.text_hud_value,
                rect_to_d2d(geometry.value),
                &tone,
            ),
            HudValue::Pill { on } => self.draw_hud_pill(geometry.value, on, alpha),
            HudValue::Text(code) => self.draw_hud_text_badge(geometry.value, code.as_str(), alpha),
            HudValue::Badge(text) => self.draw_hud_text_badge(geometry.value, text, alpha),
        }
        Ok(())
    }

    pub(crate) fn draw_hud_meter(
        &self,
        meter: Rect,
        level: f32,
        fill: &ID2D1SolidColorBrush,
        track: &ID2D1SolidColorBrush,
    ) {
        let radius = meter.height as f32 * 0.5;
        let track_shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(meter),
            radiusX: radius,
            radiusY: radius,
        };
        let filled_w = meter.width as f32 * level.clamp(0.0, 1.0);
        unsafe {
            self.target.FillRoundedRectangle(&track_shape, track);
            if filled_w > 0.0 {
                let filled = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: meter.x as f32,
                        top: meter.y as f32,
                        right: meter.x as f32 + filled_w,
                        bottom: meter.bottom() as f32,
                    },
                    radiusX: radius,
                    radiusY: radius,
                };
                self.target.FillRoundedRectangle(&filled, fill);
            }
        }
    }

    pub(crate) fn draw_hud_pill(&self, value: Rect, on: bool, alpha: f32) {
        let pill_w = scale(34, self.dpi) as f32;
        let pill_h = scale(16, self.dpi) as f32;
        let cy = value.y as f32 + value.height as f32 * 0.5;
        let rect = D2D_RECT_F {
            left: value.right() as f32 - pill_w,
            top: cy - pill_h * 0.5,
            right: value.right() as f32,
            bottom: cy + pill_h * 0.5,
        };
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: pill_h * 0.5,
            radiusY: pill_h * 0.5,
        };
        if on {
            let fill = self.brushes.alpha_brush(CHARGING_GREEN, 0.96 * alpha);
            let ink = self.brushes.alpha_brush(BLACK, 0.92 * alpha);
            unsafe {
                self.target.FillRoundedRectangle(&rounded, &fill);
            }
            self.draw_text("ON", &self.text_app_bold, rect, &ink);
        } else {
            let outline = self.brushes.alpha_brush(FOREGROUND, 0.58 * alpha);
            unsafe {
                self.target
                    .DrawRoundedRectangle(&rounded, &outline, 1.0, None);
            }
            self.draw_text("OFF", &self.text_app_bold, rect, &outline);
        }
    }

    pub(crate) fn hud_badge_width(&self, text: &str) -> f32 {
        let min_w = scale(34, self.dpi) as f32;
        let char_w = scale(6, self.dpi) as f32;
        let pad_w = scale(12, self.dpi) as f32;
        (text.chars().count() as f32 * char_w + pad_w).max(min_w)
    }

    pub(crate) fn draw_hud_text_badge(&self, value: Rect, text: &str, alpha: f32) {
        let pill_w = self.hud_badge_width(text);
        let pill_h = scale(16, self.dpi) as f32;
        let cy = value.y as f32 + value.height as f32 * 0.5;
        let rect = D2D_RECT_F {
            left: value.right() as f32 - pill_w,
            top: cy - pill_h * 0.5,
            right: value.right() as f32,
            bottom: cy + pill_h * 0.5,
        };
        let rounded = D2D1_ROUNDED_RECT {
            rect,
            radiusX: pill_h * 0.5,
            radiusY: pill_h * 0.5,
        };
        let fill = self.brushes.alpha_brush(FOREGROUND, 0.14 * alpha);
        let outline = self.brushes.alpha_brush(FOREGROUND, 0.42 * alpha);
        let ink = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        unsafe {
            self.target.FillRoundedRectangle(&rounded, &fill);
            self.target
                .DrawRoundedRectangle(&rounded, &outline, 1.0, None);
        }
        self.draw_text(text, &self.text_app_bold, rect, &ink);
    }

    pub(crate) fn draw_otto_mark(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let ring = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        let eye_width = (radius * 0.27).max(stroke * 0.65);
        let eye_height = radius * 0.62;
        unsafe {
            self.target
                .DrawEllipse(&ring, brush, stroke, Some(&self.stroke_style));
            for offset in [-radius * 0.35, radius * 0.35] {
                let eye = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx + offset - eye_width * 0.5,
                        top: cy - eye_height * 0.5,
                        right: cx + offset + eye_width * 0.5,
                        bottom: cy + eye_height * 0.5,
                    },
                    radiusX: eye_width * 0.5,
                    radiusY: eye_width * 0.5,
                };
                self.target.FillRoundedRectangle(&eye, brush);
            }
        }
    }

    pub(crate) fn draw_battery_glyph(
        &self,
        cx: f32,
        cy: f32,
        half: f32,
        stroke: f32,
        level: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let body_w = half * 1.44;
        let body_h = half * 0.88;
        let left = cx - body_w * 0.5 - half * 0.1;
        let top = cy - body_h * 0.5;
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right: left + body_w,
                bottom: top + body_h,
            },
            radiusX: half * 0.22,
            radiusY: half * 0.22,
        };
        let inset = stroke * 1.6;
        let inner_w = (body_w - inset * 2.0).max(0.0);
        let fill_w = inner_w * level.clamp(0.04, 1.0);
        let fill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + inset,
                top: top + inset,
                right: left + inset + fill_w,
                bottom: top + body_h - inset,
            },
            radiusX: half * 0.08,
            radiusY: half * 0.08,
        };
        let terminal = D2D_RECT_F {
            left: left + body_w + stroke * 0.5,
            top: cy - half * 0.14,
            right: left + body_w + stroke * 0.5 + half * 0.18,
            bottom: cy + half * 0.14,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&body, brush, stroke * 0.95, None);
            self.target.FillRectangle(&terminal, brush);
            self.target.FillRoundedRectangle(&fill, brush);
        }
    }

    pub(crate) fn draw_sun_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let core_r = r * 0.46;
        let inner = r * 0.68;
        let outer = r * 1.04;
        let diag = 0.707_106_77_f32;
        unsafe {
            let core = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: core_r,
                radiusY: core_r,
            };
            self.target.FillEllipse(&core, brush);
            for (dx, dy) in [
                (1.0_f32, 0.0_f32),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (diag, diag),
                (-diag, diag),
                (diag, -diag),
                (-diag, -diag),
            ] {
                self.target.DrawLine(
                    Vector2::new(cx + dx * inner, cy + dy * inner),
                    Vector2::new(cx + dx * outer, cy + dy * outer),
                    brush,
                    stroke * 0.92,
                    &self.stroke_style,
                );
            }
        }
    }

    pub(crate) fn draw_capslock_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            let arrow = self.factory.CreatePathGeometry()?;
            let sink = arrow.Open()?;
            sink.BeginFigure(Vector2::new(cx, cy - r * 0.85), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(cx + r * 0.75, cy - r * 0.05));
            sink.AddLine(Vector2::new(cx + r * 0.32, cy - r * 0.05));
            sink.AddLine(Vector2::new(cx + r * 0.32, cy + r * 0.35));
            sink.AddLine(Vector2::new(cx - r * 0.32, cy + r * 0.35));
            sink.AddLine(Vector2::new(cx - r * 0.32, cy - r * 0.05));
            sink.AddLine(Vector2::new(cx - r * 0.75, cy - r * 0.05));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&arrow, brush, None);

            self.target.DrawLine(
                Vector2::new(cx - r * 0.32, cy + r * 0.68),
                Vector2::new(cx + r * 0.32, cy + r * 0.68),
                brush,
                stroke * 1.15,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    pub(crate) fn draw_numlock_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let half_w = r * 0.78;
        let half_h = r * 0.74;
        let cap = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - half_w,
                top: cy - half_h,
                right: cx + half_w,
                bottom: cy + half_h,
            },
            radiusX: r * 0.24,
            radiusY: r * 0.24,
        };
        let line_w = (stroke * 0.85).max(1.1);
        let vx = r * 0.21;
        let vy = r * 0.42;
        let hx = r * 0.42;
        let hy = r * 0.19;
        unsafe {
            self.target
                .DrawRoundedRectangle(&cap, brush, stroke * 0.92, None);
            self.target.DrawLine(
                Vector2::new(cx - vx, cy - vy),
                Vector2::new(cx - vx, cy + vy),
                brush,
                line_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + vx, cy - vy),
                Vector2::new(cx + vx, cy + vy),
                brush,
                line_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - hx, cy - hy),
                Vector2::new(cx + hx, cy - hy),
                brush,
                line_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - hx, cy + hy),
                Vector2::new(cx + hx, cy + hy),
                brush,
                line_w,
                &self.stroke_style,
            );
        }
    }

    pub(crate) fn draw_keyboard_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let half_w = r * 0.94;
        let half_h = r * 0.64;
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - half_w,
                top: cy - half_h,
                right: cx + half_w,
                bottom: cy + half_h,
            },
            radiusX: r * 0.22,
            radiusY: r * 0.22,
        };
        let key_h = r * 0.16;
        let row1_y = cy - r * 0.26;
        let row2_y = cy + r * 0.10;
        let key_w = r * 0.22;
        unsafe {
            self.target
                .DrawRoundedRectangle(&body, brush, stroke * 0.92, None);
            for offset in [-0.54_f32, -0.18, 0.18, 0.54] {
                let kx = cx + r * offset - key_w * 0.5;
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left: kx,
                        top: row1_y,
                        right: kx + key_w,
                        bottom: row1_y + key_h,
                    },
                    brush,
                );
            }
            let space_hw = r * 0.44;
            self.target.FillRectangle(
                &D2D_RECT_F {
                    left: cx - space_hw,
                    top: row2_y,
                    right: cx + space_hw,
                    bottom: row2_y + key_h,
                },
                brush,
            );
        }
    }

    pub(crate) fn draw_battery_saver_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let base_x = cx - r * 0.54;
        let base_y = cy + r * 0.48;
        let tip_x = cx + r * 0.72;
        let tip_y = cy - r * 0.68;
        unsafe {
            let leaf = self.factory.CreatePathGeometry()?;
            let sink = leaf.Open()?;
            sink.BeginFigure(Vector2::new(base_x, base_y), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx - r * 0.72, cy - r * 0.52),
                point2: Vector2::new(cx + r * 0.08, cy - r * 0.78),
                point3: Vector2::new(tip_x, tip_y),
            });
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx + r * 0.78, cy - r * 0.02),
                point2: Vector2::new(cx + r * 0.42, cy + r * 0.68),
                point3: Vector2::new(base_x, base_y),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&leaf, brush, None);
            self.target.DrawLine(
                Vector2::new(cx - r * 0.80, cy + r * 0.76),
                Vector2::new(cx - r * 0.16, cy + r * 0.12),
                brush,
                (stroke * 0.95).max(1.2),
                &self.stroke_style,
            );
        }
        Ok(())
    }

    #[expect(clippy::similar_names)]
    pub(crate) fn draw_headphones_icon(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        stroke: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let arch_rx = r * 0.72;
        let arch_ry = r * 0.76;
        let cup_y = cy + r * 0.08;
        let cup_w = r * 0.34;
        let cup_h = r * 0.64;
        let cup_bottom = cup_y + cup_h;
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(Vector2::new(cx - arch_rx, cup_y), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(cx + arch_rx, cup_y),
                size: D2D_SIZE_F {
                    width: arch_rx,
                    height: arch_ry,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&geom, brush, (stroke * 0.95).max(1.2), &self.stroke_style);
            for side in [-1.0_f32, 1.0_f32] {
                let left = cx + side * arch_rx - cup_w * 0.5;
                let cup = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left,
                        top: cup_y - r * 0.08,
                        right: left + cup_w,
                        bottom: cup_bottom,
                    },
                    radiusX: cup_w * 0.45,
                    radiusY: cup_w * 0.45,
                };
                self.target.FillRoundedRectangle(&cup, brush);
            }
        }
        Ok(())
    }
}
