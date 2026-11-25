use otto_core::{AccentColor, AppearanceStyle, Rect, Rgba, SettingsHit};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use super::palette::{Palette, ROW_PAD_POINTS, WHITE, from_rgba, rect_f, rgba, with_alpha};
use super::{SettingsPaintState, SettingsRenderer};

fn blend_d2d(base: D2D1_COLOR_F, accent: D2D1_COLOR_F, factor: f32, alpha: f32) -> D2D1_COLOR_F {
    let inv = 1.0 - factor;
    D2D1_COLOR_F {
        r: base.r * inv + accent.r * factor,
        g: base.g * inv + accent.g * factor,
        b: base.b * inv + accent.b * factor,
        a: alpha,
    }
}

fn rgb_to_rgba(rgb: u32) -> Rgba {
    Rgba::new(
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        1.0,
    )
}

fn preview_surface_colors(
    style: AppearanceStyle,
    dark: bool,
    accent: D2D1_COLOR_F,
    palette: &Palette,
) -> (D2D1_COLOR_F, D2D1_COLOR_F) {
    if palette.high_contrast {
        return (palette.background, palette.text);
    }
    match (style, dark) {
        (AppearanceStyle::LiquidGlass, true) => (rgba(0x16_1A26, 0.42), rgba(0xFF_FFFF, 0.72)),
        (AppearanceStyle::LiquidGlass, false) => (rgba(0xFF_FFFF, 0.50), rgba(0xFF_FFFF, 0.92)),
        (AppearanceStyle::Frosted, true) => (rgba(0x1E_2028, 0.78), rgba(0xFF_FFFF, 0.26)),
        (AppearanceStyle::Frosted, false) => (rgba(0xF6_F7FA, 0.84), rgba(0x00_0000, 0.16)),
        (AppearanceStyle::Tinted, true) => (
            blend_d2d(rgba(0x14_161E, 1.0), accent, 0.42, 0.68),
            blend_d2d(WHITE, accent, 0.45, 0.68),
        ),
        (AppearanceStyle::Tinted, false) => (
            blend_d2d(rgba(0xF7_F8FB, 1.0), accent, 0.24, 0.76),
            blend_d2d(WHITE, accent, 0.35, 0.76),
        ),
        (AppearanceStyle::Solid, true) => (rgba(0x1E_1F24, 1.0), rgba(0xFF_FFFF, 0.16)),
        (AppearanceStyle::Solid, false) => (rgba(0xF5_F6F9, 1.0), rgba(0x00_0000, 0.16)),
    }
}

impl SettingsRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_style_picker(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        selected: AppearanceStyle,
        dark: bool,
        accent_rgb: u32,
        parts: &[Rect],
        palette: &Palette,
    ) {
        let (section_index, row_index) = coords;
        let accent = rgba(accent_rgb, 1.0);
        for (part_index, (style, card_rect)) in AppearanceStyle::ALL.iter().zip(parts).enumerate() {
            let is_selected = *style == selected;
            let is_hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: part_index,
                });
            let label_h = self.px(20);
            let preview_h = (card_rect.height - label_h - self.px(4)).max(self.px(36));
            let preview_rect = Rect {
                x: card_rect.x,
                y: card_rect.y,
                width: card_rect.width,
                height: preview_h,
            };
            self.draw_style_preview_canvas(
                preview_rect,
                *style,
                dark,
                accent,
                state.high_contrast,
                palette,
            );
            let radius = self.px(8) as f32;
            let (border_width, border_color) = if is_selected {
                (2.0, palette.accent)
            } else if is_hovered {
                (1.5, with_alpha(palette.text, 0.45))
            } else {
                (1.0, palette.hairline)
            };
            let inset = border_width * 0.5;
            self.stroke_round_f(
                rect_f(
                    preview_rect.x as f32 + inset,
                    preview_rect.y as f32 + inset,
                    (preview_rect.width as f32 - border_width).max(1.0),
                    (preview_rect.height as f32 - border_width).max(1.0),
                ),
                radius,
                border_width,
                border_color,
            );
            let label_rect = Rect {
                x: card_rect.x,
                y: preview_rect.bottom() + self.px(4),
                width: card_rect.width,
                height: label_h,
            };
            self.text(
                style.label(),
                &self.formats.caption_center,
                label_rect,
                if is_selected {
                    palette.text
                } else {
                    palette.secondary
                },
            );
        }
    }

    fn draw_style_preview_backdrop(
        &self,
        rect: Rect,
        radius: f32,
        dark: bool,
        accent: D2D1_COLOR_F,
        high_contrast: bool,
    ) {
        let wallpaper = if high_contrast {
            if dark {
                rgba(0x10_1014, 1.0)
            } else {
                rgba(0xEC_ECF0, 1.0)
            }
        } else if dark {
            rgba(0x14_2034, 1.0)
        } else {
            rgba(0xCB_DEEE, 1.0)
        };
        self.fill_round(rect, radius, wallpaper);
        if high_contrast {
            return;
        }
        let orb_a = if dark {
            blend_d2d(rgba(0x2A_5CA8, 1.0), accent, 0.35, 0.55)
        } else {
            blend_d2d(rgba(0x74_B8F8, 1.0), accent, 0.25, 0.50)
        };
        let orb_b = if dark {
            rgba(0x9A_4B8E, 0.38)
        } else {
            rgba(0xF7_C89E, 0.55)
        };
        let w = rect.width as f32;
        let h = rect.height as f32;
        self.fill_circle(
            rect.x as f32 + w * 0.25,
            rect.y as f32 + h * 0.38,
            w * 0.34,
            orb_a,
        );
        self.fill_circle(
            rect.x as f32 + w * 0.80,
            rect.y as f32 + h * 0.72,
            w * 0.30,
            orb_b,
        );
    }

    fn draw_style_preview_canvas(
        &self,
        rect: Rect,
        style: AppearanceStyle,
        dark: bool,
        accent: D2D1_COLOR_F,
        high_contrast: bool,
        palette: &Palette,
    ) {
        let radius = self.px(8) as f32;
        self.canvas.push_clip(rect);
        self.draw_style_preview_backdrop(rect, radius, dark, accent, high_contrast);

        let (shell_fill, shell_rim) = preview_surface_colors(style, dark, accent, palette);
        let bar_h = self.px(8).max(5);
        self.fill(
            Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: bar_h,
            },
            shell_fill,
        );
        let notch_w = (rect.width / 4).max(self.px(16));
        let notch_rect = Rect {
            x: rect.x + (rect.width - notch_w) / 2,
            y: rect.y,
            width: notch_w,
            height: bar_h + self.px(1),
        };
        self.fill_round(
            notch_rect,
            self.px(3) as f32,
            if high_contrast {
                palette.background
            } else {
                rgba(0x00_0000, 1.0)
            },
        );

        let card_w = (rect.width * 56 / 100).max(self.px(24));
        let card_h = (rect.height * 38 / 100).max(self.px(14));
        let mini_card = Rect {
            x: rect.x + (rect.width - card_w) / 2,
            y: rect.y + bar_h + self.px(6),
            width: card_w,
            height: card_h,
        };
        let card_radius = self.px(5) as f32;
        self.fill_round(mini_card, card_radius, shell_fill);
        self.stroke_round_f(
            rect_f(
                mini_card.x as f32 + 0.5,
                mini_card.y as f32 + 0.5,
                (mini_card.width as f32 - 1.0).max(1.0),
                (mini_card.height as f32 - 1.0).max(1.0),
            ),
            card_radius,
            1.0,
            shell_rim,
        );
        let pill_w = (mini_card.width * 42 / 100).max(self.px(10));
        let pill_h = self.px(4).max(3);
        self.fill_round(
            Rect {
                x: mini_card.x + self.px(5),
                y: mini_card.y + (mini_card.height - pill_h) / 2,
                width: pill_w,
                height: pill_h,
            },
            pill_h as f32 * 0.5,
            palette.accent,
        );

        let dock_w = (rect.width * 62 / 100).max(self.px(28));
        let dock_h = self.px(8).max(5);
        let dock_rect = Rect {
            x: rect.x + (rect.width - dock_w) / 2,
            y: rect.bottom() - dock_h - self.px(4),
            width: dock_w,
            height: dock_h,
        };
        let dock_radius = dock_h as f32 * 0.5;
        self.fill_round(dock_rect, dock_radius, shell_fill);
        self.stroke_round_f(
            rect_f(
                dock_rect.x as f32 + 0.5,
                dock_rect.y as f32 + 0.5,
                (dock_rect.width as f32 - 1.0).max(1.0),
                (dock_rect.height as f32 - 1.0).max(1.0),
            ),
            dock_radius,
            0.8,
            shell_rim,
        );
        self.canvas.pop_clip();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_accent_swatches(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        selected: AccentColor,
        system_accent_rgb: Option<u32>,
        selected_label: &str,
        geometry: (Rect, &[Rect]),
        palette: &Palette,
    ) {
        let (section_index, row_index) = coords;
        let (rect, parts) = geometry;
        let sys_rgba = system_accent_rgb.map(rgb_to_rgba);
        for (part_index, (accent, slot)) in AccentColor::ALL.iter().zip(parts).enumerate() {
            let is_selected = *accent == selected;
            let is_hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: part_index,
                });
            let cx = slot.x as f32 + slot.width as f32 * 0.5;
            let cy = slot.y as f32 + slot.height as f32 * 0.5;
            let dot_r = (slot.width.min(slot.height) as f32 * 0.34).max(7.0);
            if is_hovered && !is_selected {
                self.fill_circle(cx, cy, dot_r + 3.5, palette.hover);
            }
            let swatch = accent.swatch(sys_rgba);
            self.fill_circle(cx, cy, dot_r, from_rgba(swatch));
            if *accent == AccentColor::System {
                self.stroke_circle(cx, cy, dot_r - 2.0, 1.2, with_alpha(WHITE, 0.72));
            }
            if is_selected {
                self.stroke_circle(
                    cx,
                    cy,
                    dot_r + 3.0,
                    2.0,
                    if state.high_contrast {
                        palette.text
                    } else {
                        from_rgba(swatch)
                    },
                );
                let mark = if swatch.relative_luminance() > 0.45 {
                    rgba(0x0D_0D12, 0.95)
                } else {
                    WHITE
                };
                self.fill_circle(cx, cy, (dot_r * 0.28).max(2.5), mark);
            }
        }
        let pad = self.px(ROW_PAD_POINTS);
        let label_w = self.px(84);
        self.text(
            selected_label,
            &self.formats.value,
            Rect {
                x: rect.right() - pad - label_w,
                y: rect.y,
                width: label_w,
                height: rect.height,
            },
            palette.secondary,
        );
    }
}
