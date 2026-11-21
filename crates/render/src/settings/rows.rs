use otto_core::{CardLayout, Rect, SettingsHit, SettingsRow, SettingsRowValue};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use super::palette::{
    BADGE_WIDTH_POINTS, CARD_RADIUS_POINTS, DOT_POINTS, KEYCAP_GAP_POINTS, KEYCAP_HEIGHT_POINTS,
    Palette, ROW_PAD_POINTS, ROW_TILE_POINTS, SWITCH_HEIGHT_POINTS, SWITCH_WIDTH_POINTS, WHITE,
    rect_f, with_alpha,
};
use super::{SettingsPaintState, SettingsRenderer};

impl SettingsRenderer {
    pub(super) fn draw_card(
        &self,
        state: &SettingsPaintState<'_>,
        section_index: usize,
        title: &str,
        rows: &[SettingsRow],
        card: &CardLayout,
        palette: &Palette,
    ) {
        let pad = self.px(ROW_PAD_POINTS);
        if !title.is_empty() {
            self.text(
                title,
                &self.formats.section,
                Rect {
                    x: card.header.x + pad,
                    width: card.header.width - pad,
                    ..card.header
                },
                palette.secondary,
            );
        }
        let radius = self.px(CARD_RADIUS_POINTS) as f32;
        self.fill_round(card.card, radius, palette.card);
        if palette.glass {
            self.stroke_round_f(
                rect_f(
                    card.card.x as f32 + 0.5,
                    card.card.y as f32 + 0.5,
                    card.card.width as f32 - 1.0,
                    card.card.height as f32 - 1.0,
                ),
                radius,
                1.0,
                palette.rim,
            );
        }
        let inner_radius = (radius - 3.0).max(2.0);
        for (index, (row, rect)) in rows.iter().zip(&card.rows).enumerate() {
            let hovered = state.hovered
                == Some(SettingsHit::Row {
                    section: section_index,
                    row: index,
                });
            if hovered && row.action.is_some() {
                let inset = self.px(4);
                self.fill_round(
                    Rect {
                        x: rect.x + inset,
                        y: rect.y + inset / 2,
                        width: rect.width - inset * 2,
                        height: rect.height - inset,
                    },
                    inner_radius,
                    palette.hover,
                );
            }
            let text_x = self.draw_row(row, *rect, palette);
            if index + 1 < rows.len() {
                self.fill(
                    Rect {
                        x: text_x,
                        y: rect.bottom() - 1,
                        width: rect.right() - text_x,
                        height: 1,
                    },
                    palette.hairline,
                );
            }
        }
    }

    fn draw_row(&self, row: &SettingsRow, rect: Rect, palette: &Palette) -> i32 {
        let alpha = if row.muted { 0.55 } else { 1.0 };
        let pad = self.px(ROW_PAD_POINTS);
        let mut text_x = rect.x + pad;
        if let Some(icon) = row.icon {
            let tile = self.px(ROW_TILE_POINTS);
            self.draw_tile(
                Rect {
                    x: rect.x + pad,
                    y: rect.y + (rect.height - tile) / 2,
                    width: tile,
                    height: tile,
                },
                icon,
            );
            text_x += tile + self.px(12);
        }
        let right = rect.right() - pad;
        let trailing = self.draw_value(row, rect, right, palette, alpha);
        let label_width = (trailing - text_x - self.px(12)).max(0);
        if row.subtitle.is_empty() {
            self.text(
                &row.label,
                &self.formats.label,
                Rect {
                    x: text_x,
                    y: rect.y,
                    width: label_width,
                    height: rect.height,
                },
                with_alpha(palette.text, alpha),
            );
        } else {
            let half = rect.height / 2;
            self.text(
                &row.label,
                &self.formats.label,
                Rect {
                    x: text_x,
                    y: rect.y + self.px(6),
                    width: label_width,
                    height: half - self.px(6),
                },
                with_alpha(palette.text, alpha),
            );
            self.text(
                &row.subtitle,
                &self.formats.subtitle,
                Rect {
                    x: text_x,
                    y: rect.y + half,
                    width: label_width,
                    height: rect.height - half - self.px(8),
                },
                with_alpha(palette.secondary, alpha),
            );
        }
        text_x
    }

    fn draw_value(
        &self,
        row: &SettingsRow,
        rect: Rect,
        right: i32,
        palette: &Palette,
        alpha: f32,
    ) -> i32 {
        let secondary = with_alpha(palette.secondary, alpha);
        match &row.value {
            SettingsRowValue::Toggle(on) => {
                let width = self.px(SWITCH_WIDTH_POINTS);
                self.draw_switch(rect, right, *on, row.action.is_some(), palette);
                let mut left = right - width;
                if let Some((label, tone)) = row.badge {
                    let badge = self.px(BADGE_WIDTH_POINTS);
                    let gap = self.px(12);
                    self.text(
                        label,
                        &self.formats.value,
                        Rect {
                            x: left - gap - badge,
                            y: rect.y,
                            width: badge,
                            height: rect.height,
                        },
                        secondary,
                    );
                    let dot = self.px(DOT_POINTS);
                    let label_width = self.px(label.chars().count() as i32 * 7);
                    self.fill_circle(
                        (left - gap - label_width - dot) as f32,
                        rect.y as f32 + rect.height as f32 / 2.0,
                        dot as f32 / 2.0,
                        with_alpha(palette.tone(tone), alpha),
                    );
                    left -= gap + badge;
                }
                left
            }
            SettingsRowValue::Choice(label) => {
                let chevrons = self.px(12);
                self.draw_updown(right - chevrons / 2, rect, secondary);
                let width = self.px(96);
                self.text(
                    label,
                    &self.formats.value,
                    Rect {
                        x: right - chevrons - self.px(6) - width,
                        y: rect.y,
                        width,
                        height: rect.height,
                    },
                    secondary,
                );
                right - chevrons - width
            }
            SettingsRowValue::Detail(label) => {
                let chevron = self.px(8);
                self.draw_chevron(right - chevron / 2, rect, secondary);
                let width = self.px(96);
                self.text(
                    label,
                    &self.formats.value,
                    Rect {
                        x: right - chevron - self.px(8) - width,
                        y: rect.y,
                        width,
                        height: rect.height,
                    },
                    secondary,
                );
                right - chevron - width
            }
            SettingsRowValue::Text(text) => {
                let width = (rect.width * 45) / 100;
                self.text(
                    text,
                    &self.formats.value,
                    Rect {
                        x: right - width,
                        y: rect.y,
                        width,
                        height: rect.height,
                    },
                    secondary,
                );
                right - width
            }
            SettingsRowValue::Keys(keys) => self.draw_keys(keys, rect, right, palette),
        }
    }

    fn draw_keys(&self, keys: &str, rect: Rect, right: i32, palette: &Palette) -> i32 {
        let height = self.px(KEYCAP_HEIGHT_POINTS);
        let gap = self.px(KEYCAP_GAP_POINTS);
        let mut x = right;
        for key in keys.split('+').rev() {
            let width = self.px(10 + key.chars().count() as i32 * 7).max(height);
            x -= width;
            let cap = Rect {
                x,
                y: rect.y + (rect.height - height) / 2,
                width,
                height,
            };
            self.fill_round(cap, self.px(5) as f32, palette.keycap);
            self.text(key, &self.formats.keycap, cap, palette.text);
            x -= gap;
        }
        x
    }

    fn draw_switch(&self, row: Rect, right: i32, on: bool, enabled: bool, palette: &Palette) {
        let width = self.px(SWITCH_WIDTH_POINTS);
        let height = self.px(SWITCH_HEIGHT_POINTS);
        let track = Rect {
            x: right - width,
            y: row.y + (row.height - height) / 2,
            width,
            height,
        };
        let alpha = if enabled { 1.0 } else { 0.4 };
        let fill = if on {
            palette.switch_on
        } else {
            palette.switch_off
        };
        self.fill_round(track, height as f32 / 2.0, with_alpha(fill, alpha));
        let pad = self.px(2);
        let thumb = height - pad * 2;
        let thumb_x = if on {
            track.right() - pad - thumb
        } else {
            track.x + pad
        };
        self.fill_circle(
            thumb_x as f32 + thumb as f32 / 2.0,
            track.y as f32 + height as f32 / 2.0,
            thumb as f32 / 2.0,
            with_alpha(WHITE, alpha),
        );
    }

    fn draw_chevron(&self, cx: i32, row: Rect, color: D2D1_COLOR_F) {
        let cy = row.y as f32 + row.height as f32 / 2.0;
        let reach = self.px(4) as f32;
        let x = cx as f32;
        self.line(x - reach / 2.0, cy - reach, x + reach / 2.0, cy, 1.5, color);
        self.line(x + reach / 2.0, cy, x - reach / 2.0, cy + reach, 1.5, color);
    }

    fn draw_updown(&self, cx: i32, row: Rect, color: D2D1_COLOR_F) {
        let cy = row.y as f32 + row.height as f32 / 2.0;
        let reach = self.px(3) as f32;
        let x = cx as f32;
        let gap = self.px(2) as f32;
        self.line(x - reach, cy - gap, x, cy - gap - reach, 1.4, color);
        self.line(x, cy - gap - reach, x + reach, cy - gap, 1.4, color);
        self.line(x - reach, cy + gap, x, cy + gap + reach, 1.4, color);
        self.line(x, cy + gap + reach, x + reach, cy + gap, 1.4, color);
    }
}
