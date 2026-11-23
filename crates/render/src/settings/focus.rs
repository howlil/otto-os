use otto_core::{
    FilterChip, FocusTextField, KpiItem, Rect, SegmentedOption, SettingsHit, SettingsRow,
    WeeklyChart,
};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::D2D1_BITMAP_INTERPOLATION_MODE_LINEAR;
use windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS;

use super::palette::{
    CARD_RADIUS_POINTS, Palette, ROW_PAD_POINTS, ROW_TILE_POINTS, SWITCH_WIDTH_POINTS, WHITE, d2d,
    rect_f, rgba, with_alpha,
};
use super::{SettingsPaintState, SettingsRenderer};
use crate::shared_dwrite_factory;

fn scale_bar_height(percent: u8, max_height: i32) -> i32 {
    i32::from(percent) * max_height / 100
}

impl SettingsRenderer {
    pub(super) fn draw_app_icon_slot(
        &self,
        rect: Rect,
        icon_key: &str,
        label: &str,
        palette: &Palette,
    ) {
        if let Some(bitmap) = self.app_icons.get(icon_key) {
            unsafe {
                self.target.DrawBitmap(
                    bitmap,
                    Some(&d2d(rect)),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
            return;
        }
        let radius = rect.width as f32 * 0.24;
        self.fill_round(rect, radius, palette.chip);
        self.stroke_round_f(
            rect_f(
                rect.x as f32 + 0.5,
                rect.y as f32 + 0.5,
                rect.width as f32 - 1.0,
                rect.height as f32 - 1.0,
            ),
            radius,
            1.0,
            palette.hairline,
        );
        let initial: String = label
            .chars()
            .find(|ch| ch.is_alphanumeric())
            .map_or_else(|| String::from("?"), |ch| ch.to_uppercase().to_string());
        self.text(&initial, &self.formats.chip, rect, palette.secondary);
    }

    pub(super) fn draw_segmented_row(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        options: &[SegmentedOption],
        selected: usize,
        geometry: (Rect, &[Rect]),
        palette: &Palette,
    ) {
        let (section_index, row_index) = coords;
        let (rect, parts) = geometry;
        let pad = self.px(ROW_PAD_POINTS);
        let track = Rect {
            x: rect.x + pad,
            y: rect.y + self.px(4),
            width: (rect.width - pad * 2).max(0),
            height: (rect.height - self.px(8)).max(0),
        };
        let track_radius = self.px(8) as f32;
        self.fill_round(track, track_radius, palette.track);
        self.stroke_round_f(
            rect_f(
                track.x as f32 + 0.5,
                track.y as f32 + 0.5,
                track.width as f32 - 1.0,
                track.height as f32 - 1.0,
            ),
            track_radius,
            1.0,
            palette.hairline,
        );
        let seg_radius = self.px(6) as f32;
        for (part_index, (option, seg_rect)) in options.iter().zip(parts).enumerate() {
            let is_selected = part_index == selected;
            let is_hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: part_index,
                });
            if is_selected {
                self.fill_round(*seg_rect, seg_radius, palette.segment_active);
                self.stroke_round_f(
                    rect_f(
                        seg_rect.x as f32 + 0.5,
                        seg_rect.y as f32 + 0.5,
                        seg_rect.width as f32 - 1.0,
                        seg_rect.height as f32 - 1.0,
                    ),
                    seg_radius,
                    if state.high_contrast { 1.5 } else { 1.0 },
                    if state.high_contrast {
                        palette.accent
                    } else {
                        palette.hairline
                    },
                );
            } else if is_hovered {
                self.fill_round(*seg_rect, seg_radius, palette.hover);
            }
            self.text(
                option.label,
                &self.formats.chip,
                *seg_rect,
                if is_selected {
                    palette.text
                } else {
                    palette.secondary
                },
            );
        }
    }

    pub(super) fn draw_filter_chips_row(
        &self,
        state: &SettingsPaintState<'_>,
        coords: (usize, usize),
        chips: &[FilterChip],
        selected: usize,
        parts: &[Rect],
        palette: &Palette,
    ) {
        let (section_index, row_index) = coords;
        for (part_index, (chip, chip_rect)) in chips.iter().zip(parts).enumerate() {
            let is_selected = part_index == selected;
            let is_hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: part_index,
                });
            let radius = chip_rect.height as f32 / 2.0;
            if is_selected {
                self.fill_round(*chip_rect, radius, palette.accent);
                if state.high_contrast {
                    self.stroke_round_f(
                        rect_f(
                            chip_rect.x as f32 + 0.5,
                            chip_rect.y as f32 + 0.5,
                            chip_rect.width as f32 - 1.0,
                            chip_rect.height as f32 - 1.0,
                        ),
                        radius,
                        1.5,
                        WHITE,
                    );
                }
            } else {
                let fill = if is_hovered {
                    palette.hover
                } else {
                    palette.chip
                };
                self.fill_round(*chip_rect, radius, fill);
                self.stroke_round_f(
                    rect_f(
                        chip_rect.x as f32 + 0.5,
                        chip_rect.y as f32 + 0.5,
                        chip_rect.width as f32 - 1.0,
                        chip_rect.height as f32 - 1.0,
                    ),
                    radius,
                    1.0,
                    palette.hairline,
                );
            }
            self.text(
                &chip.label,
                &self.formats.chip,
                *chip_rect,
                if is_selected { WHITE } else { palette.text },
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_text_input_row(
        &self,
        state: &SettingsPaintState<'_>,
        section_index: usize,
        row_index: usize,
        field: FocusTextField,
        text: &str,
        placeholder: &str,
        button_label: Option<&str>,
        focused: bool,
        error: Option<&str>,
        parts: &[Rect],
        rect: Rect,
        palette: &Palette,
    ) {
        let Some(&input_part) = parts.first() else {
            return;
        };
        let visual_box = if button_label.is_none()
            && let Some(&clear_part) = parts.get(1)
        {
            Rect {
                x: input_part.x,
                y: input_part.y,
                width: (clear_part.right() + self.px(4) - input_part.x).max(input_part.width),
                height: input_part.height,
            }
        } else {
            input_part
        };
        let radius = self.px(7) as f32;
        self.fill_round(visual_box, radius, palette.surface);
        let border_color = if error.is_some() {
            palette.bad
        } else if focused {
            palette.accent
        } else {
            palette.hairline
        };
        self.stroke_round_f(
            rect_f(
                visual_box.x as f32 + 0.5,
                visual_box.y as f32 + 0.5,
                visual_box.width as f32 - 1.0,
                visual_box.height as f32 - 1.0,
            ),
            radius,
            if focused || error.is_some() { 1.5 } else { 1.0 },
            border_color,
        );

        let mut text_left = input_part.x + self.px(10);
        if field == FocusTextField::AppSearch {
            let icon_r = self.px(4) as f32;
            let cx = input_part.x as f32 + self.px(15) as f32;
            let cy = input_part.y as f32 + input_part.height as f32 / 2.0 - 1.0;
            self.stroke_circle(cx, cy, icon_r, 1.4, palette.secondary);
            self.line(
                cx + icon_r * 0.72,
                cy + icon_r * 0.72,
                cx + icon_r * 1.55,
                cy + icon_r * 1.55,
                1.5,
                palette.secondary,
            );
            text_left = input_part.x + self.px(28);
        }
        let text_rect = Rect {
            x: text_left,
            y: input_part.y,
            width: (input_part.right() - text_left - self.px(8)).max(0),
            height: input_part.height,
        };
        if text.is_empty() {
            self.text(
                placeholder,
                &self.formats.sidebar,
                text_rect,
                palette.secondary,
            );
        } else {
            self.text(text, &self.formats.sidebar, text_rect, palette.text);
        }
        if focused {
            let advance = measure_text_width(text, &self.formats.sidebar, text_rect.width)
                .min((text_rect.width - 2).max(0) as f32);
            let caret_x = text_rect.x as f32 + advance + 1.0;
            let caret_top = input_part.y as f32 + self.px(8) as f32;
            let caret_bot = input_part.bottom() as f32 - self.px(8) as f32;
            self.line(caret_x, caret_top, caret_x, caret_bot, 1.5, palette.accent);
        }

        if let Some(&second) = parts.get(1) {
            let hovered = state.hovered
                == Some(SettingsHit::Part {
                    section: section_index,
                    row: row_index,
                    part: 1,
                });
            self.draw_text_input_button(second, text, button_label, hovered, radius, palette);
        }

        if let Some(err) = error {
            self.text(
                err,
                &self.formats.caption_leading,
                Rect {
                    x: visual_box.x + self.px(4),
                    y: visual_box.bottom() + self.px(3),
                    width: (rect.right() - visual_box.x - self.px(ROW_PAD_POINTS)).max(0),
                    height: self.px(18),
                },
                palette.bad,
            );
        }
    }

    fn draw_text_input_button(
        &self,
        button_rect: Rect,
        text: &str,
        button_label: Option<&str>,
        hovered: bool,
        radius: f32,
        palette: &Palette,
    ) {
        if let Some(label) = button_label {
            let enabled = !text.trim().is_empty();
            let fill = if enabled {
                if hovered {
                    with_alpha(palette.accent, 0.88)
                } else {
                    palette.accent
                }
            } else {
                palette.chip
            };
            self.fill_round(button_rect, radius, fill);
            if !enabled {
                self.stroke_round_f(
                    rect_f(
                        button_rect.x as f32 + 0.5,
                        button_rect.y as f32 + 0.5,
                        button_rect.width as f32 - 1.0,
                        button_rect.height as f32 - 1.0,
                    ),
                    radius,
                    1.0,
                    palette.hairline,
                );
            }
            self.text(
                label,
                &self.formats.chip,
                button_rect,
                if enabled { WHITE } else { palette.secondary },
            );
        } else {
            if hovered {
                self.fill_round(button_rect, button_rect.height as f32 / 2.0, palette.hover);
            }
            let cx = button_rect.x as f32 + button_rect.width as f32 / 2.0;
            let cy = button_rect.y as f32 + button_rect.height as f32 / 2.0;
            let arm = self.px(4) as f32;
            self.line(
                cx - arm,
                cy - arm,
                cx + arm,
                cy + arm,
                1.5,
                palette.secondary,
            );
            self.line(
                cx + arm,
                cy - arm,
                cx - arm,
                cy + arm,
                1.5,
                palette.secondary,
            );
        }
    }

    pub(super) fn draw_kpi_strip(&self, items: &[KpiItem; 3], parts: &[Rect], palette: &Palette) {
        let radius = self.px(8) as f32;
        for (item, tile) in items.iter().zip(parts) {
            self.fill_round(*tile, radius, palette.surface);
            self.stroke_round_f(
                rect_f(
                    tile.x as f32 + 0.5,
                    tile.y as f32 + 0.5,
                    tile.width as f32 - 1.0,
                    tile.height as f32 - 1.0,
                ),
                radius,
                1.0,
                palette.hairline,
            );
            let pad_x = self.px(12);
            let inner_w = (tile.width - pad_x * 2).max(0);
            self.text(
                item.label,
                &self.formats.caption_leading,
                Rect {
                    x: tile.x + pad_x,
                    y: tile.y + self.px(8),
                    width: inner_w,
                    height: self.px(16),
                },
                palette.secondary,
            );
            self.text(
                &item.value,
                &self.formats.kpi_value,
                Rect {
                    x: tile.x + pad_x,
                    y: tile.y + self.px(24),
                    width: inner_w,
                    height: self.px(26),
                },
                palette.text,
            );
            self.text(
                &item.detail,
                &self.formats.caption_leading,
                Rect {
                    x: tile.x + pad_x,
                    y: tile.bottom() - self.px(20),
                    width: inner_w,
                    height: self.px(15),
                },
                palette.secondary,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_weekly_chart(
        &self,
        state: &SettingsPaintState<'_>,
        section_index: usize,
        row_index: usize,
        row: &SettingsRow,
        chart: &WeeklyChart,
        parts: &[Rect],
        rect: Rect,
        palette: &Palette,
    ) {
        let hovered_day = match state.hovered {
            Some(SettingsHit::Part {
                section,
                row: hit_row,
                part,
            }) if section == section_index && hit_row == row_index && part < chart.days.len() => {
                Some(part)
            }
            _ => None,
        };
        let active_day = hovered_day.or(chart.selected_day);
        self.draw_weekly_chart_header(row, chart, active_day, rect, state.high_contrast, palette);

        let label_h = self.px(20);
        if let Some(first) = parts.first()
            && let Some(last) = parts.last()
        {
            let baseline = first.bottom() - label_h - self.px(4);
            self.fill(
                Rect {
                    x: first.x,
                    y: baseline,
                    width: (last.right() - first.x).max(0),
                    height: 1,
                },
                palette.hairline,
            );
        }

        for (index, (day, col)) in chart.days.iter().zip(parts).enumerate() {
            self.draw_weekly_chart_day(
                day,
                *col,
                active_day == Some(index),
                chart.selected_day == Some(index),
                state.high_contrast,
                palette,
            );
        }
    }

    fn draw_weekly_chart_header(
        &self,
        row: &SettingsRow,
        chart: &WeeklyChart,
        active_day: Option<usize>,
        rect: Rect,
        high_contrast: bool,
        palette: &Palette,
    ) {
        let pad = self.px(ROW_PAD_POINTS);
        let legend_width = self.px(148);
        let header_w = (rect.width - pad * 2 - legend_width - self.px(8)).max(0);
        self.text(
            &row.label,
            &self.formats.label,
            Rect {
                x: rect.x + pad,
                y: rect.y + self.px(10),
                width: header_w,
                height: self.px(20),
            },
            palette.text,
        );
        let readout = active_day
            .and_then(|day| chart.day_readout(day))
            .unwrap_or_else(|| chart.summary_text.clone());
        let readout_color = if active_day.is_some() {
            palette.accent
        } else {
            palette.secondary
        };
        self.text(
            &readout,
            &self.formats.subtitle,
            Rect {
                x: rect.x + pad,
                y: rect.y + self.px(31),
                width: (rect.width - pad * 2).max(0),
                height: self.px(18),
            },
            readout_color,
        );

        let legend_y = rect.y + self.px(13);
        let swatch = self.px(10);
        let focus_swatch_x = rect.right() - pad - self.px(52);
        let total_swatch_x = focus_swatch_x - self.px(66);
        let total_box = Rect {
            x: total_swatch_x,
            y: legend_y + self.px(2),
            width: swatch,
            height: swatch,
        };
        self.fill_round(total_box, 2.0, palette.bar_total);
        self.stroke_round_f(
            rect_f(
                total_box.x as f32 + 0.5,
                total_box.y as f32 + 0.5,
                total_box.width as f32 - 1.0,
                total_box.height as f32 - 1.0,
            ),
            2.0,
            1.0,
            palette.bar_total_stroke,
        );
        self.text(
            "Total",
            &self.formats.caption_leading,
            Rect {
                x: total_box.right() + self.px(5),
                y: legend_y,
                width: self.px(44),
                height: self.px(16),
            },
            palette.secondary,
        );

        let focus_box = Rect {
            x: focus_swatch_x,
            y: legend_y + self.px(2),
            width: swatch,
            height: swatch,
        };
        self.fill_round(focus_box, 2.0, palette.accent);
        self.draw_focus_hatch(focus_box, high_contrast);
        self.text(
            "Focus",
            &self.formats.caption_leading,
            Rect {
                x: focus_box.right() + self.px(5),
                y: legend_y,
                width: self.px(44),
                height: self.px(16),
            },
            palette.secondary,
        );
    }

    fn draw_weekly_chart_day(
        &self,
        day: &otto_core::DayBar,
        col: Rect,
        is_active: bool,
        is_selected: bool,
        high_contrast: bool,
        palette: &Palette,
    ) {
        let label_h = self.px(20);
        let bar_top_pad = self.px(6);
        if is_active {
            let col_radius = self.px(6) as f32;
            self.fill_round(col, col_radius, palette.hover);
            if is_selected || high_contrast {
                self.stroke_round_f(
                    rect_f(
                        col.x as f32 + 0.5,
                        col.y as f32 + 0.5,
                        col.width as f32 - 1.0,
                        col.height as f32 - 1.0,
                    ),
                    col_radius,
                    1.25,
                    palette.accent,
                );
            }
        }
        let baseline = col.bottom() - label_h - self.px(4);
        let max_bar_h = (baseline - col.y - bar_top_pad).max(self.px(12));
        let bar_w = self.px(24).min((col.width - self.px(10)).max(self.px(8)));
        let bar_x = col.x + (col.width - bar_w) / 2;
        let bar_radius = self.px(4) as f32;

        let total_h = if day.active_ms > 0 {
            scale_bar_height(day.active_percent, max_bar_h).max(self.px(4))
        } else {
            self.px(2)
        };
        let total_rect = Rect {
            x: bar_x,
            y: baseline - total_h,
            width: bar_w,
            height: total_h,
        };
        self.fill_round(total_rect, bar_radius, palette.bar_total);
        if day.active_ms > 0 {
            self.stroke_round_f(
                rect_f(
                    total_rect.x as f32 + 0.5,
                    total_rect.y as f32 + 0.5,
                    total_rect.width as f32 - 1.0,
                    total_rect.height as f32 - 1.0,
                ),
                bar_radius,
                1.0,
                palette.bar_total_stroke,
            );
        }

        if day.focus_ms > 0 {
            let focus_h = scale_bar_height(day.focus_percent, max_bar_h)
                .max(self.px(4))
                .min(total_h);
            let focus_rect = Rect {
                x: bar_x,
                y: baseline - focus_h,
                width: bar_w,
                height: focus_h,
            };
            self.fill_round(focus_rect, bar_radius, palette.accent);
            self.draw_focus_hatch(focus_rect, high_contrast);
            if high_contrast {
                self.stroke_round_f(
                    rect_f(
                        focus_rect.x as f32 + 0.5,
                        focus_rect.y as f32 + 0.5,
                        focus_rect.width as f32 - 1.0,
                        focus_rect.height as f32 - 1.0,
                    ),
                    bar_radius,
                    1.2,
                    WHITE,
                );
            }
        }

        let label_rect = Rect {
            x: col.x,
            y: baseline + self.px(4),
            width: col.width,
            height: label_h - self.px(2),
        };
        let label_color = if is_active || day.is_today {
            palette.text
        } else {
            palette.secondary
        };
        self.text(
            &day.day_label,
            &self.formats.caption_center,
            label_rect,
            label_color,
        );
        if day.is_today {
            self.fill_circle(
                col.x as f32 + col.width as f32 / 2.0,
                col.bottom() as f32 - 2.0,
                1.8,
                palette.accent,
            );
        }
    }

    fn draw_focus_hatch(&self, rect: Rect, high_contrast: bool) {
        let step = self.px(6).max(4);
        let stripe = with_alpha(WHITE, if high_contrast { 0.60 } else { 0.26 });
        let mut offset = step;
        while offset < rect.height + rect.width {
            let x0 = (rect.x + offset).min(rect.right() - 2);
            let y0 = rect.y + (offset - (x0 - rect.x));
            let y1 = (rect.y + offset).min(rect.bottom() - 2);
            let x1 = rect.x + (offset - (y1 - rect.y));
            if x0 > x1 && y1 > y0 {
                self.line(x0 as f32, y0 as f32, x1 as f32, y1 as f32, 1.0, stripe);
            }
            offset += step;
        }
    }

    pub(super) fn draw_usage_bar_row(
        &self,
        row: &SettingsRow,
        duration: &str,
        percent: u8,
        blocked: bool,
        rect: Rect,
        palette: &Palette,
    ) -> i32 {
        let pad = self.px(ROW_PAD_POINTS);
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

        let right = rect.right() - pad;
        let switch_w = self.px(SWITCH_WIDTH_POINTS);
        self.draw_switch(rect, right, blocked, row.action.is_some(), palette);

        let duration_w = self.px(66);
        let duration_x = right - switch_w - self.px(10) - duration_w;
        self.text(
            duration,
            &self.formats.value,
            Rect {
                x: duration_x,
                y: rect.y,
                width: duration_w,
                height: rect.height,
            },
            palette.text,
        );

        let bar_area_w = (duration_x - text_x - self.px(14)).max(0);
        self.text(
            &row.label,
            &self.formats.label,
            Rect {
                x: text_x,
                y: rect.y + self.px(7),
                width: bar_area_w,
                height: self.px(20),
            },
            palette.text,
        );

        let bar_h = self.px(6).max(4);
        let bar_y = rect.y + self.px(34);
        let track_rect = Rect {
            x: text_x,
            y: bar_y,
            width: bar_area_w,
            height: bar_h,
        };
        let bar_radius = bar_h as f32 / 2.0;
        self.fill_round(track_rect, bar_radius, palette.track);
        let fill_w = (bar_area_w * i32::from(percent) / 100)
            .max(if percent > 0 { bar_h } else { 0 })
            .min(bar_area_w);
        if fill_w > 0 {
            self.fill_round(
                Rect {
                    width: fill_w,
                    ..track_rect
                },
                bar_radius,
                palette.accent,
            );
        }
        text_x
    }

    pub(super) fn draw_disclosure_chevron(
        &self,
        cx: i32,
        row: Rect,
        expanded: bool,
        color: D2D1_COLOR_F,
    ) {
        let cy = row.y as f32 + row.height as f32 / 2.0;
        let reach = self.px(4) as f32;
        let x = cx as f32;
        if expanded {
            self.line(x - reach, cy - reach / 2.0, x, cy + reach / 2.0, 1.5, color);
            self.line(x, cy + reach / 2.0, x + reach, cy - reach / 2.0, 1.5, color);
        } else {
            self.line(x - reach / 2.0, cy - reach, x + reach / 2.0, cy, 1.5, color);
            self.line(x + reach / 2.0, cy, x - reach / 2.0, cy + reach, 1.5, color);
        }
    }

    pub(super) fn draw_hover_detail_bar(&self, state: &SettingsPaintState<'_>, palette: &Palette) {
        let Some((section, row)) = (match state.hovered {
            Some(SettingsHit::Row { section, row } | SettingsHit::Part { section, row, .. }) => {
                Some((section, row))
            }
            _ => None,
        }) else {
            return;
        };
        let Some(hint) = state
            .content
            .row(section, row)
            .and_then(|item| item.detail_hint.as_deref())
            .filter(|hint| !hint.is_empty())
        else {
            return;
        };
        let pad = self.px(12);
        let height = self.px(28);
        let area = Rect {
            x: state.layout.column.x,
            y: state.layout.content.bottom() - height - self.px(8),
            width: state.layout.column.width,
            height,
        };
        if area.width <= pad * 2 {
            return;
        }
        let radius = self.px(CARD_RADIUS_POINTS) as f32;
        self.fill_round(
            area,
            radius,
            if state.dark {
                rgba(0x1C_1C1F, 0.94)
            } else {
                rgba(0xFF_FFFF, 0.96)
            },
        );
        self.stroke_round_f(
            rect_f(
                area.x as f32 + 0.5,
                area.y as f32 + 0.5,
                area.width as f32 - 1.0,
                area.height as f32 - 1.0,
            ),
            radius,
            1.0,
            palette.hairline,
        );
        let text = format!("Path: {hint}");
        self.text(
            &text,
            &self.formats.caption_leading,
            Rect {
                x: area.x + pad,
                y: area.y,
                width: area.width - pad * 2,
                height: area.height,
            },
            palette.secondary,
        );
    }
}

fn measure_text_width(
    text: &str,
    format: &windows::Win32::Graphics::DirectWrite::IDWriteTextFormat,
    max_width: i32,
) -> f32 {
    if text.is_empty() || max_width <= 0 {
        return 0.0;
    }
    let Ok(dwrite) = shared_dwrite_factory() else {
        return 0.0;
    };
    let wide: Vec<u16> = text.encode_utf16().collect();
    let Ok(layout) = (unsafe { dwrite.CreateTextLayout(&wide, format, max_width as f32, 64.0) })
    else {
        return 0.0;
    };
    let mut metrics = DWRITE_TEXT_METRICS::default();
    if unsafe { layout.GetMetrics(&mut metrics) }.is_ok() {
        metrics.widthIncludingTrailingWhitespace
    } else {
        0.0
    }
}
