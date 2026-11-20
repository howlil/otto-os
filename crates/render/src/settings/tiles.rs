use otto_core::{Rect, SettingsPage};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_MEASURING_MODE_NATURAL, IDWriteTextFormat};
use windows_numerics::Vector2;

use super::SettingsRenderer;
use super::palette::{WHITE, d2d, rect_f, rgba, tint};

impl SettingsRenderer {
    pub(super) fn draw_app_icon(&self, tile: Rect) {
        let size = tile.width as f32;
        self.fill_round(tile, size * 0.225, rgba(0x0A_84FF, 1.0));
        let cx = tile.x as f32 + size / 2.0;
        let cy = tile.y as f32 + size / 2.0;
        self.stroke_circle(cx, cy, size * 0.25, size * 0.1, WHITE);
        self.fill_circle(cx + size * 0.2, cy - size * 0.2, size * 0.065, WHITE);
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn draw_tile(&self, tile: Rect, page: SettingsPage) {
        let size = tile.width as f32;
        self.fill_round(tile, size * 0.26, tint(page));
        let g = size * 0.56;
        let ox = tile.x as f32 + (size - g) / 2.0;
        let oy = tile.y as f32 + (size - g) / 2.0;
        let u = g / 10.0;
        let w = (u * 0.85).max(1.0);
        let px = |x: f32| ox + x * u;
        let py = |y: f32| oy + y * u;
        match page {
            SettingsPage::Overview => {
                for (x, y) in [(0.0, 0.0), (5.6, 0.0), (0.0, 5.6), (5.6, 5.6)] {
                    self.fill_round_f(rect_f(px(x), py(y), u * 4.4, u * 4.4), u, WHITE);
                }
            }
            SettingsPage::Modules => {
                self.fill_round_f(rect_f(px(0.0), py(2.2), u * 10.0, u * 5.6), u * 2.8, WHITE);
                self.fill_circle(px(7.2), py(5.0), u * 1.9, tint(page));
            }
            SettingsPage::TopBar => {
                self.stroke_round_f(
                    rect_f(px(0.4), py(1.2), u * 9.2, u * 7.6),
                    u * 1.6,
                    w,
                    WHITE,
                );
                self.fill_round_f(rect_f(px(0.4), py(1.2), u * 9.2, u * 2.4), u * 1.2, WHITE);
            }
            SettingsPage::Notch => {
                self.stroke_round_f(
                    rect_f(px(0.4), py(1.2), u * 9.2, u * 7.6),
                    u * 1.6,
                    w,
                    WHITE,
                );
                self.fill_round_f(rect_f(px(3.2), py(1.2), u * 3.6, u * 2.2), u * 1.0, WHITE);
            }
            SettingsPage::Dock => {
                self.stroke_round_f(
                    rect_f(px(0.4), py(1.2), u * 9.2, u * 7.6),
                    u * 1.6,
                    w,
                    WHITE,
                );
                self.fill_round_f(rect_f(px(1.8), py(6.0), u * 6.4, u * 1.8), u * 0.9, WHITE);
            }
            SettingsPage::WindowControls => {
                for x in [1.6, 5.0, 8.4] {
                    self.fill_circle(px(x), py(5.0), u * 1.3, WHITE);
                }
            }
            SettingsPage::Spotlight => {
                self.stroke_circle(px(4.2), py(4.2), u * 3.2, w, WHITE);
                self.line(px(6.7), py(6.7), px(9.4), py(9.4), w, WHITE);
            }
            SettingsPage::ControlCenter => {
                self.line(px(0.4), py(2.8), px(9.6), py(2.8), w, WHITE);
                self.fill_circle(px(6.6), py(2.8), u * 1.7, WHITE);
                self.line(px(0.4), py(7.2), px(9.6), py(7.2), w, WHITE);
                self.fill_circle(px(3.2), py(7.2), u * 1.7, WHITE);
            }
            SettingsPage::Appearance => {
                self.stroke_circle(px(5.0), py(5.0), u * 4.2, w, WHITE);
                self.fill_circle(px(5.0), py(5.0), u * 2.0, WHITE);
            }
            SettingsPage::Shortcuts => {
                self.stroke_round_f(
                    rect_f(px(0.6), py(0.6), u * 8.8, u * 8.8),
                    u * 2.2,
                    w,
                    WHITE,
                );
                self.line(px(2.8), py(6.0), px(5.0), py(3.6), w, WHITE);
                self.line(px(5.0), py(3.6), px(7.2), py(6.0), w, WHITE);
            }
            SettingsPage::General => {
                self.stroke_circle(px(5.0), py(5.0), u * 2.2, w, WHITE);
                for step in 0..8 {
                    let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                    let (sin, cos) = angle.sin_cos();
                    self.line(
                        px(5.0 + cos * 3.7),
                        py(5.0 + sin * 3.7),
                        px(5.0 + cos * 4.8),
                        py(5.0 + sin * 4.8),
                        w * 1.3,
                        WHITE,
                    );
                }
            }
            SettingsPage::About => {
                self.stroke_circle(px(5.0), py(5.0), u * 4.4, w, WHITE);
                self.fill_circle(px(5.0), py(3.1), u * 0.7, WHITE);
                self.line(px(5.0), py(4.7), px(5.0), py(7.5), w, WHITE);
            }
        }
    }

    pub(super) fn fill(&self, rect: Rect, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillRectangle(&d2d(rect), &self.brush);
        }
    }

    pub(super) fn fill_round(&self, rect: Rect, radius: f32, color: D2D1_COLOR_F) {
        self.fill_round_f(
            rect_f(
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
            ),
            radius,
            color,
        );
    }

    pub(super) fn fill_round_f(&self, rect: D2D_RECT_F, radius: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
            );
        }
    }

    pub(super) fn stroke_round_f(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        width: f32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
                width,
                &self.stroke,
            );
        }
    }

    pub(super) fn fill_circle(&self, cx: f32, cy: f32, radius: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
            );
        }
    }

    pub(super) fn stroke_circle(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        width: f32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
                width,
                &self.stroke,
            );
        }
    }

    pub(super) fn line(&self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawLine(
                Vector2::new(x0, y0),
                Vector2::new(x1, y1),
                &self.brush,
                width,
                &self.stroke,
            );
        }
    }

    pub(super) fn text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: Rect,
        color: D2D1_COLOR_F,
    ) {
        if text.is_empty() || rect.is_empty() {
            return;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawText(
                &wide,
                format,
                &d2d(rect),
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}
