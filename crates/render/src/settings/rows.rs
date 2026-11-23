use otto_core::{BadgeTone, CardLayout, Rect, SettingsHit, SettingsRow, SettingsRowValue};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use super::palette::{
    BADGE_WIDTH_POINTS, CARD_RADIUS_POINTS, DOT_POINTS, KEYCAP_GAP_POINTS, KEYCAP_HEIGHT_POINTS,
    Palette, ROW_PAD_POINTS, ROW_TILE_POINTS, SWITCH_HEIGHT_POINTS, SWITCH_WIDTH_POINTS, WHITE,
    rect_f, with_alpha,
};
use super::{SettingsPaintState, SettingsRenderer};

struct ProcessAuditVisual<'a> {
    status_label: &'static str,
    status_tone: BadgeTone,
    throttle_label: Option<&'a str>,
    throttled: bool,
    terminate_label: Option<&'a str>,
    recommended_end: bool,
}

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
        self.stroke_round_f(
            rect_f(
                card.card.x as f32 + 0.5,
                card.card.y as f32 + 0.5,
                card.card.width as f32 - 1.0,
                card.card.height as f32 - 1.0,
            ),
            radius,
            1.0,
            palette.hairline,
        );
        let inner_radius = (radius - 3.0).max(2.0);
        for (index, (row, rect)) in rows.iter().zip(&card.rows).enumerate() {
            let parts = card.row_parts.get(index).map_or(&[][..], Vec::as_slice);
            let hovered = state.hovered
                == Some(SettingsHit::Row {
                    section: section_index,
                    row: index,
                });
            if hovered && row.action.is_some() && parts.is_empty() {
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
            let text_x =
                self.draw_card_row(state, (section_index, index), row, parts, *rect, palette);
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

    fn draw_card_row(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        row: &SettingsRow,
        parts: &[Rect],
        rect: Rect,
        palette: &Palette,
    ) -> i32 {
        let pad = self.px(ROW_PAD_POINTS);
        let (section_index, index) = coords;
        match &row.value {
            SettingsRowValue::Segmented { options, selected } => {
                self.draw_segmented_row(state, coords, options, *selected, (rect, parts), palette);
                rect.x + pad
            }
            SettingsRowValue::FilterChips { chips, selected } => {
                self.draw_filter_chips_row(state, coords, chips, *selected, parts, palette);
                rect.x + pad
            }
            SettingsRowValue::TextInput {
                field,
                text,
                placeholder,
                button_label,
                focused,
                error,
            } => {
                self.draw_text_input_row(
                    state,
                    section_index,
                    index,
                    *field,
                    text,
                    placeholder,
                    *button_label,
                    *focused,
                    error.as_deref(),
                    parts,
                    rect,
                    palette,
                );
                rect.x + pad
            }
            SettingsRowValue::KpiStrip(items) => {
                self.draw_kpi_strip(items, parts, palette);
                rect.x + pad
            }
            SettingsRowValue::WeeklyChart(chart) => {
                self.draw_weekly_chart(
                    state,
                    section_index,
                    index,
                    row,
                    chart,
                    parts,
                    rect,
                    palette,
                );
                rect.x + pad
            }
            SettingsRowValue::UsageBar {
                duration,
                percent,
                blocked,
            } => self.draw_usage_bar_row(row, duration, *percent, *blocked, rect, palette),
            SettingsRowValue::StylePicker {
                selected,
                dark,
                accent_rgb,
            } => {
                self.draw_style_picker(
                    state,
                    coords,
                    *selected,
                    *dark,
                    *accent_rgb,
                    parts,
                    palette,
                );
                rect.x + pad
            }
            SettingsRowValue::AccentSwatches {
                selected,
                system_accent_rgb,
            } => {
                self.draw_accent_swatches(
                    state,
                    coords,
                    *selected,
                    *system_accent_rgb,
                    row.label.as_str(),
                    (rect, parts),
                    palette,
                );
                rect.x + pad
            }
            SettingsRowValue::ProcessAudit { .. } => {
                self.draw_process_audit_row(state, coords, row, (rect, parts), palette)
            }
            _ => self.draw_row(row, rect, palette),
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
        } else if let Some(icon_key) = row.app_icon.as_deref() {
            let tile = self.px(ROW_TILE_POINTS);
            self.draw_app_icon_slot(
                Rect {
                    x: rect.x + pad,
                    y: rect.y + (rect.height - tile) / 2,
                    width: tile,
                    height: tile,
                },
                icon_key,
                &row.label,
                palette,
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
            SettingsRowValue::Disclosure { expanded } => {
                let chevron = self.px(10);
                self.draw_disclosure_chevron(right - chevron / 2, rect, *expanded, secondary);
                right - chevron - self.px(8)
            }
            SettingsRowValue::Segmented { .. }
            | SettingsRowValue::FilterChips { .. }
            | SettingsRowValue::TextInput { .. }
            | SettingsRowValue::KpiStrip(_)
            | SettingsRowValue::WeeklyChart(_)
            | SettingsRowValue::UsageBar { .. }
            | SettingsRowValue::StylePicker { .. }
            | SettingsRowValue::AccentSwatches { .. }
            | SettingsRowValue::ProcessAudit { .. } => right,
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

    pub(super) fn draw_switch(
        &self,
        row: Rect,
        right: i32,
        on: bool,
        enabled: bool,
        palette: &Palette,
    ) {
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

    fn draw_audit_pill(
        &self,
        part_rect: Rect,
        label: &str,
        colors: (D2D1_COLOR_F, D2D1_COLOR_F, D2D1_COLOR_F),
    ) {
        let (fill, border, text_color) = colors;
        let radius = part_rect.height as f32 / 2.0;
        self.fill_round(part_rect, radius, fill);
        self.stroke_round_f(
            rect_f(
                part_rect.x as f32 + 0.5,
                part_rect.y as f32 + 0.5,
                part_rect.width as f32 - 1.0,
                part_rect.height as f32 - 1.0,
            ),
            radius,
            1.0,
            border,
        );
        self.text(label, &self.formats.chip, part_rect, text_color);
    }

    fn draw_status_dot_pill(
        &self,
        pill: Rect,
        label: &str,
        tone: BadgeTone,
        high_contrast: bool,
        palette: &Palette,
    ) {
        let radius = pill.height as f32 / 2.0;
        self.fill_round(pill, radius, palette.chip);
        self.stroke_round_f(
            rect_f(
                pill.x as f32 + 0.5,
                pill.y as f32 + 0.5,
                pill.width as f32 - 1.0,
                pill.height as f32 - 1.0,
            ),
            radius,
            1.0,
            palette.hairline,
        );
        let dot = self.px(6);
        self.fill_circle(
            (pill.x + self.px(10) + dot / 2) as f32,
            pill.y as f32 + pill.height as f32 / 2.0,
            dot as f32 / 2.0,
            palette.tone(tone),
        );
        self.text(
            label,
            &self.formats.caption_leading,
            Rect {
                x: pill.x + self.px(10) + dot + self.px(6),
                y: pill.y,
                width: (pill.width - self.px(22) - dot).max(0),
                height: pill.height,
            },
            if high_contrast {
                palette.text
            } else {
                palette.secondary
            },
        );
    }

    fn draw_process_protected_pill(
        &self,
        rect: Rect,
        pad: i32,
        visual: &ProcessAuditVisual<'_>,
        high_contrast: bool,
        palette: &Palette,
    ) -> i32 {
        let pill_w = self.px(92);
        let pill_h = self.px(24);
        let pill = Rect {
            x: rect.right() - pad - pill_w,
            y: rect.y + (rect.height - pill_h) / 2,
            width: pill_w,
            height: pill_h,
        };
        self.draw_status_dot_pill(
            pill,
            visual.status_label,
            visual.status_tone,
            high_contrast,
            palette,
        );
        pill.x
    }

    fn end_task_pill_colors(
        hovered: bool,
        recommended: bool,
        high_contrast: bool,
        palette: &Palette,
    ) -> (D2D1_COLOR_F, D2D1_COLOR_F, D2D1_COLOR_F) {
        if hovered {
            (
                with_alpha(palette.bad, if recommended { 0.28 } else { 0.22 }),
                palette.bad,
                if high_contrast {
                    palette.text
                } else {
                    palette.bad
                },
            )
        } else if recommended {
            (
                with_alpha(palette.bad, 0.12),
                with_alpha(palette.bad, 0.55),
                if high_contrast {
                    palette.text
                } else {
                    palette.bad
                },
            )
        } else {
            (palette.chip, palette.hairline, palette.text)
        }
    }

    fn draw_process_action_pills(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        visual: &ProcessAuditVisual<'_>,
        parts: &[Rect],
        palette: &Palette,
    ) -> i32 {
        let (section_index, row_index) = coords;
        if let (Some(label), Some(&part_rect)) = (
            visual.throttle_label.or(visual.terminate_label),
            parts.first(),
        ) {
            let hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: 0,
                });
            let is_throttle = visual.throttle_label.is_some();
            let colors = if !is_throttle {
                Self::end_task_pill_colors(
                    hovered,
                    visual.recommended_end,
                    state.high_contrast,
                    palette,
                )
            } else if visual.throttled {
                (
                    with_alpha(palette.good, if hovered { 0.28 } else { 0.18 }),
                    palette.good,
                    palette.text,
                )
            } else {
                (
                    if hovered { palette.hover } else { palette.chip },
                    if hovered {
                        palette.accent
                    } else {
                        palette.hairline
                    },
                    palette.text,
                )
            };
            self.draw_audit_pill(part_rect, label, colors);
        }
        if let (Some(label), Some(&part_rect)) = (visual.terminate_label, parts.get(1)) {
            let hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: 1,
                });
            let colors = Self::end_task_pill_colors(
                hovered,
                visual.recommended_end,
                state.high_contrast,
                palette,
            );
            self.draw_audit_pill(part_rect, label, colors);
        }
        parts[0].x
    }

    fn draw_process_audit_row(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        row: &SettingsRow,
        geometry: (Rect, &[Rect]),
        palette: &Palette,
    ) -> i32 {
        let (rect, parts) = geometry;
        let pad = self.px(ROW_PAD_POINTS);
        let SettingsRowValue::ProcessAudit {
            status_label,
            status_tone,
            throttle_label,
            throttled,
            terminate_label,
            recommended_end,
            ..
        } = &row.value
        else {
            return rect.x + pad;
        };
        let visual = ProcessAuditVisual {
            status_label,
            status_tone: *status_tone,
            throttle_label: *throttle_label,
            throttled: *throttled,
            terminate_label: *terminate_label,
            recommended_end: *recommended_end,
        };
        let mut text_x = rect.x + pad;
        if let Some(icon_key) = row.app_icon.as_deref() {
            let tile = self.px(ROW_TILE_POINTS);
            self.draw_app_icon_slot(
                Rect {
                    x: rect.x + pad,
                    y: rect.y + (rect.height - tile) / 2,
                    width: tile,
                    height: tile,
                },
                icon_key,
                &row.label,
                palette,
            );
            text_x += tile + self.px(12);
        }

        let mut trailing_left = if parts.is_empty() {
            self.draw_process_protected_pill(rect, pad, &visual, state.high_contrast, palette)
        } else {
            self.draw_process_action_pills(state, coords, &visual, parts, palette)
        };
        if visual.recommended_end {
            let pill_w = self.px(98);
            let pill_h = self.px(22);
            let gap = self.px(8);
            let pill_x = trailing_left - gap - pill_w;
            if pill_x - text_x >= self.px(150) {
                self.draw_status_dot_pill(
                    Rect {
                        x: pill_x,
                        y: rect.y + (rect.height - pill_h) / 2,
                        width: pill_w,
                        height: pill_h,
                    },
                    "Safe to End",
                    BadgeTone::Warning,
                    state.high_contrast,
                    palette,
                );
                trailing_left = pill_x;
            }
        }

        let label_width = (trailing_left - text_x - self.px(12)).max(0);
        self.text(
            &row.label,
            &self.formats.label,
            Rect {
                x: text_x,
                y: rect.y + self.px(8),
                width: label_width,
                height: self.px(20),
            },
            palette.text,
        );
        self.text(
            &row.subtitle,
            &self.formats.subtitle,
            Rect {
                x: text_x,
                y: rect.y + self.px(29),
                width: label_width,
                height: self.px(18),
            },
            palette.secondary,
        );
        text_x
    }
}
