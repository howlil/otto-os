use otto_core::{
    DayBar, FocusStatsRange, FocusStatsView, FocusUsageRow, HitTarget, Layout, Rect, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1SolidColorBrush};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::{FOREGROUND, Renderer, rect_to_d2d, rgb, translate_rect};

const SCREEN_TIME_BLUE: D2D1_COLOR_F = rgb(10, 132, 255, 1.0);
const FOCUS_ORANGE: D2D1_COLOR_F = rgb(255, 146, 48, 1.0);
const SHIELD_GREEN: D2D1_COLOR_F = rgb(48, 209, 88, 1.0);
const BADGE_TINTS: [D2D1_COLOR_F; 6] = [
    rgb(10, 132, 255, 1.0),
    rgb(94, 92, 230, 1.0),
    rgb(48, 209, 88, 1.0),
    rgb(255, 159, 10, 1.0),
    rgb(0, 160, 210, 1.0),
    rgb(191, 90, 242, 1.0),
];

#[derive(Clone, Copy)]
struct WeekDayColumnStyle {
    baseline: i32,
    max_bar_h: i32,
    is_active: bool,
    is_selected: bool,
    high_contrast: bool,
    alpha: f32,
}

impl Renderer {
    fn is_high_contrast_notch(&self) -> bool {
        unsafe { self.brushes.specular_rim.GetColor().a >= 0.85 }
    }

    pub(crate) fn draw_expanded_focus_stats(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        stats: &FocusStatsView,
    ) -> Result<()> {
        if alpha <= 0.005 {
            return Ok(());
        }
        self.draw_focus_stats_header(layout, offset_x, alpha, hovered, stats);
        let body = translate_rect(layout.stats.body, -offset_x);
        if body.is_empty() {
            return Ok(());
        }
        let high_contrast = self.is_high_contrast_notch();
        let radius = scale(12, self.dpi) as f32;
        let fill = self
            .brushes
            .alpha_brush(FOREGROUND, if high_contrast { 0.09 } else { 0.055 } * alpha);
        self.draw_rounded_box(body, radius, &fill);
        let border = self
            .brushes
            .alpha_brush(FOREGROUND, if high_contrast { 0.72 } else { 0.10 } * alpha);
        let border_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: body.x as f32 + 0.5,
                top: body.y as f32 + 0.5,
                right: body.right() as f32 - 0.5,
                bottom: body.bottom() as f32 - 0.5,
            },
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&border_rect, &border, 1.0, None);
        }

        self.push_clip(rect_to_d2d(body));
        let result = match stats.range {
            FocusStatsRange::Today => self.draw_focus_stats_today(body, alpha, stats),
            FocusStatsRange::Week => {
                self.draw_focus_stats_week(layout, offset_x, body, alpha, hovered, stats);
                Ok(())
            }
        };
        self.pop_clip();
        result
    }

    fn draw_focus_stats_header(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        stats: &FocusStatsView,
    ) {
        let today_rect = translate_rect(layout.stats.range_today, -offset_x);
        let week_rect = translate_rect(layout.stats.range_week, -offset_x);
        let details_rect = translate_rect(layout.stats.open_settings, -offset_x);
        if !today_rect.is_empty() && !week_rect.is_empty() {
            self.draw_stats_range_switch(today_rect, week_rect, alpha, hovered, stats.range);
        }
        if !details_rect.is_empty() {
            self.draw_stats_details_button(details_rect, alpha, hovered);
        }
    }

    fn draw_stats_range_switch(
        &self,
        today_rect: Rect,
        week_rect: Rect,
        alpha: f32,
        hovered: Option<HitTarget>,
        active_range: FocusStatsRange,
    ) {
        let track = today_rect.union(&week_rect);
        let track_brush = self.brushes.alpha_brush(FOREGROUND, 0.075 * alpha);
        self.draw_rounded_box(track, track.height as f32 * 0.5, &track_brush);

        let inset = scale(2, self.dpi);
        for (range, rect) in [
            (FocusStatsRange::Today, today_rect),
            (FocusStatsRange::Week, week_rect),
        ] {
            let active = active_range == range;
            let is_hovered = hovered == Some(HitTarget::FocusStatsRange(range));
            let pill = Rect {
                x: rect.x + inset,
                y: rect.y + inset,
                width: (rect.width - inset * 2).max(0),
                height: (rect.height - inset * 2).max(0),
            };
            if active {
                let fill = self.brushes.alpha_brush(FOREGROUND, 0.18 * alpha);
                self.draw_rounded_box(pill, pill.height as f32 * 0.5, &fill);
            } else if is_hovered {
                let fill = self.brushes.alpha_brush(FOREGROUND, 0.08 * alpha);
                self.draw_rounded_box(pill, pill.height as f32 * 0.5, &fill);
            }
            let ink = self.brushes.alpha_brush(
                FOREGROUND,
                if active {
                    0.96
                } else if is_hovered {
                    0.82
                } else {
                    0.56
                } * alpha,
            );
            self.draw_text(range.label(), &self.text_tab, rect_to_d2d(rect), &ink);
        }
    }

    fn draw_stats_details_button(
        &self,
        details_rect: Rect,
        alpha: f32,
        hovered: Option<HitTarget>,
    ) {
        let is_hovered = hovered == Some(HitTarget::FocusStatsOpenSettings);
        let fill = self
            .brushes
            .alpha_brush(FOREGROUND, if is_hovered { 0.14 } else { 0.07 } * alpha);
        self.draw_rounded_box(details_rect, details_rect.height as f32 * 0.5, &fill);
        let ink = self
            .brushes
            .alpha_brush(FOREGROUND, if is_hovered { 0.94 } else { 0.64 } * alpha);
        let chevron_pad = scale(13, self.dpi);
        self.draw_text(
            "Details",
            &self.text_pomodoro_sub_trailing,
            D2D_RECT_F {
                left: details_rect.x as f32,
                top: details_rect.y as f32,
                right: (details_rect.right() - chevron_pad) as f32,
                bottom: details_rect.bottom() as f32,
            },
            &ink,
        );
        let cx = details_rect.right() as f32 - scale(7, self.dpi) as f32;
        let cy = details_rect.y as f32 + details_rect.height as f32 * 0.5;
        self.draw_right_chevron(cx, cy, scale(3, self.dpi) as f32 * 0.85, &ink);
    }

    fn draw_focus_stats_today(&self, body: Rect, alpha: f32, stats: &FocusStatsView) -> Result<()> {
        let pad_x = scale(14, self.dpi);
        let pad_y = scale(12, self.dpi);
        let inner_left = body.x + pad_x;
        let inner_right = body.right() - pad_x;
        let inner_top = body.y + pad_y;
        let inner_bottom = body.bottom() - pad_y;
        let left_w = scale(150, self.dpi);
        let divider_x = inner_left + left_w + scale(10, self.dpi);
        let right_left = divider_x + scale(10, self.dpi);

        self.draw_today_hero_column(inner_left, inner_top, left_w, alpha, stats)?;

        let divider = self.brushes.alpha_brush(FOREGROUND, 0.09 * alpha);
        unsafe {
            self.target.DrawLine(
                Vector2::new(
                    divider_x as f32 + 0.5,
                    (inner_top + scale(4, self.dpi)) as f32,
                ),
                Vector2::new(
                    divider_x as f32 + 0.5,
                    (inner_bottom - scale(4, self.dpi)) as f32,
                ),
                &divider,
                1.0,
                None,
            );
        }

        self.draw_today_usage_column(right_left, inner_right, inner_top, alpha, stats);
        Ok(())
    }

    fn draw_today_hero_column(
        &self,
        inner_left: i32,
        inner_top: i32,
        left_w: i32,
        alpha: f32,
        stats: &FocusStatsView,
    ) -> Result<()> {
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.54 * alpha);
        let primary = self.brushes.alpha_brush(FOREGROUND, 0.96 * alpha);
        let cap_h = scale(14, self.dpi);
        self.draw_text(
            "Screen Time",
            &self.text_caption,
            D2D_RECT_F {
                left: inner_left as f32,
                top: inner_top as f32,
                right: (inner_left + scale(76, self.dpi)) as f32,
                bottom: (inner_top + cap_h) as f32,
            },
            &muted,
        );
        if stats.today_blocked_count > 0 {
            let label = format!("{} blocked", stats.today_blocked_count);
            let badge_ink = self.brushes.alpha_brush(SHIELD_GREEN, 0.88 * alpha);
            self.draw_tabular_text(
                &label,
                &self.text_stats_trailing,
                D2D_RECT_F {
                    left: (inner_left + scale(76, self.dpi)) as f32,
                    top: inner_top as f32,
                    right: (inner_left + left_w) as f32,
                    bottom: (inner_top + cap_h) as f32,
                },
                &badge_ink,
            );
        }
        self.draw_tabular_text(
            &stats.today_screen_time,
            &self.text_stats_hero,
            D2D_RECT_F {
                left: inner_left as f32,
                top: (inner_top + scale(16, self.dpi)) as f32,
                right: (inner_left + left_w) as f32,
                bottom: (inner_top + scale(48, self.dpi)) as f32,
            },
            &primary,
        );

        let ring_r = scale(17, self.dpi) as f32;
        let ring_stroke = (scale(3, self.dpi) as f32 * 1.15).max(2.6);
        let ring_center = Vector2::new(
            inner_left as f32 + ring_r + ring_stroke * 0.5,
            (inner_top + scale(82, self.dpi)) as f32,
        );
        let ring_active = self.brushes.alpha_brush(FOCUS_ORANGE, 0.96 * alpha);
        let ring_track = self.brushes.alpha_brush(FOREGROUND, 0.13 * alpha);
        self.draw_progress_ring(
            ring_center.X,
            ring_center.Y,
            ring_r,
            ring_stroke,
            f32::from(stats.today_focus_percent) / 100.0,
            &ring_active,
            &ring_track,
        )?;
        let pct_label = format!("{}%", stats.today_focus_percent);
        let pct_ink = self.brushes.alpha_brush(FOREGROUND, 0.92 * alpha);
        self.draw_tabular_text(
            &pct_label,
            &self.text_stats_badge,
            D2D_RECT_F {
                left: ring_center.X - ring_r,
                top: ring_center.Y - scale(8, self.dpi) as f32,
                right: ring_center.X + ring_r,
                bottom: ring_center.Y + scale(8, self.dpi) as f32,
            },
            &pct_ink,
        );

        let focus_text_x = inner_left + scale(48, self.dpi);
        self.draw_tabular_text(
            &stats.today_focus_time,
            &self.text_stats_value,
            D2D_RECT_F {
                left: focus_text_x as f32,
                top: (inner_top + scale(64, self.dpi)) as f32,
                right: (inner_left + left_w) as f32,
                bottom: (inner_top + scale(82, self.dpi)) as f32,
            },
            &primary,
        );
        self.draw_text(
            "Focus time",
            &self.text_caption,
            D2D_RECT_F {
                left: focus_text_x as f32,
                top: (inner_top + scale(82, self.dpi)) as f32,
                right: (inner_left + left_w) as f32,
                bottom: (inner_top + scale(98, self.dpi)) as f32,
            },
            &muted,
        );
        Ok(())
    }

    fn draw_today_usage_column(
        &self,
        right_left: i32,
        inner_right: i32,
        inner_top: i32,
        alpha: f32,
        stats: &FocusStatsView,
    ) {
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.54 * alpha);
        self.draw_text(
            "Most Used",
            &self.text_caption,
            D2D_RECT_F {
                left: right_left as f32,
                top: inner_top as f32,
                right: inner_right as f32,
                bottom: (inner_top + scale(14, self.dpi)) as f32,
            },
            &muted,
        );

        if stats.top_apps.is_empty() {
            let empty_main = self.brushes.alpha_brush(FOREGROUND, 0.52 * alpha);
            let empty_sub = self.brushes.alpha_brush(FOREGROUND, 0.36 * alpha);
            self.draw_text(
                "No app usage yet",
                &self.text_pomodoro_sub,
                D2D_RECT_F {
                    left: right_left as f32,
                    top: (inner_top + scale(42, self.dpi)) as f32,
                    right: inner_right as f32,
                    bottom: (inner_top + scale(60, self.dpi)) as f32,
                },
                &empty_main,
            );
            self.draw_text(
                "Tracks active apps automatically",
                &self.text_caption,
                D2D_RECT_F {
                    left: right_left as f32,
                    top: (inner_top + scale(60, self.dpi)) as f32,
                    right: inner_right as f32,
                    bottom: (inner_top + scale(76, self.dpi)) as f32,
                },
                &empty_sub,
            );
            return;
        }

        let right_w = (inner_right - right_left).max(0);
        let row_h = scale(28, self.dpi);
        let row_gap = scale(4, self.dpi);
        let list_top = inner_top + scale(20, self.dpi);
        for (rank, item) in stats.top_apps.iter().enumerate() {
            let row_rect = Rect {
                x: right_left,
                y: list_top + (row_h + row_gap) * rank as i32,
                width: right_w,
                height: row_h,
            };
            self.draw_usage_row(row_rect, rank, item, alpha);
        }
    }

    fn draw_usage_row(&self, row: Rect, rank: usize, item: &FocusUsageRow, alpha: f32) {
        let badge_size = scale(20, self.dpi);
        let badge_rect = Rect {
            x: row.x,
            y: row.y + scale(2, self.dpi),
            width: badge_size,
            height: badge_size,
        };
        let badge_r = scale(5, self.dpi) as f32;
        let drew_icon = self.stats_app_icon(&item.app_key).is_some_and(|bitmap| {
            crate::kit::draw::fill_squircle_bitmap(
                &self.factory,
                &self.target,
                badge_rect,
                false,
                badge_r,
                &bitmap,
                alpha,
            )
        });
        if !drew_icon {
            let tint = BADGE_TINTS[usize::from(item.color_index) % BADGE_TINTS.len()];
            let badge_fill = self.brushes.alpha_brush(tint, 0.24 * alpha);
            let badge_border = self.brushes.alpha_brush(tint, 0.46 * alpha);
            let badge_box = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(badge_rect),
                radiusX: badge_r,
                radiusY: badge_r,
            };
            unsafe {
                self.target.FillRoundedRectangle(&badge_box, &badge_fill);
                self.target
                    .DrawRoundedRectangle(&badge_box, &badge_border, 1.0, None);
            }
            let badge_ink = self.brushes.alpha_brush(FOREGROUND, 0.94 * alpha);
            self.draw_text(
                &item.badge,
                &self.text_stats_badge,
                rect_to_d2d(badge_rect),
                &badge_ink,
            );
        }

        let content_x = badge_rect.right() + scale(8, self.dpi);
        let dur_w = scale(46, self.dpi);
        let label_right = (row.right() - dur_w - scale(4, self.dpi)).max(content_x);
        let label_h = scale(16, self.dpi);
        let label_ink = self.brushes.alpha_brush(FOREGROUND, 0.92 * alpha);
        self.draw_text(
            &item.label,
            &self.text_pomodoro_sub,
            D2D_RECT_F {
                left: content_x as f32,
                top: row.y as f32,
                right: label_right as f32,
                bottom: (row.y + label_h) as f32,
            },
            &label_ink,
        );
        let dur_ink = self.brushes.alpha_brush(FOREGROUND, 0.64 * alpha);
        self.draw_tabular_text(
            &item.duration_text,
            &self.text_stats_trailing,
            D2D_RECT_F {
                left: (row.right() - dur_w) as f32,
                top: row.y as f32,
                right: row.right() as f32,
                bottom: (row.y + label_h) as f32,
            },
            &dur_ink,
        );

        let bar_h = scale(4, self.dpi).max(3);
        let bar_y = row.y + scale(20, self.dpi);
        let bar_w = (row.right() - content_x).max(0);
        let bar_r = bar_h as f32 * 0.5;
        let track = Rect {
            x: content_x,
            y: bar_y,
            width: bar_w,
            height: bar_h,
        };
        let track_brush = self.brushes.alpha_brush(FOREGROUND, 0.10 * alpha);
        self.draw_rounded_box(track, bar_r, &track_brush);

        let fill_w = (bar_w * i32::from(item.share_percent) / 100)
            .max(bar_h)
            .min(bar_w);
        if fill_w > 0 {
            let strength = match rank {
                0 => 0.92,
                1 => 0.64,
                _ => 0.44,
            };
            let fill_brush = self.brushes.alpha_brush(SCREEN_TIME_BLUE, strength * alpha);
            self.draw_rounded_box(
                Rect {
                    width: fill_w,
                    ..track
                },
                bar_r,
                &fill_brush,
            );
        }
    }

    fn draw_focus_stats_week(
        &self,
        layout: &Layout,
        offset_x: i32,
        body: Rect,
        alpha: f32,
        hovered: Option<HitTarget>,
        stats: &FocusStatsView,
    ) {
        let pad_x = scale(14, self.dpi);
        let inner_left = body.x + pad_x;
        let inner_right = body.right() - pad_x;
        let readout_y = body.y + scale(10, self.dpi);
        let readout_h = scale(18, self.dpi);
        let legend_w = scale(106, self.dpi);
        let readout_ink = self.brushes.alpha_brush(
            FOREGROUND,
            if stats.active_day.is_some() {
                0.96
            } else {
                0.80
            } * alpha,
        );
        self.draw_tabular_text(
            &stats.weekly_readout,
            &self.text_pomodoro_sub,
            D2D_RECT_F {
                left: inner_left as f32,
                top: readout_y as f32,
                right: (inner_right - legend_w - scale(8, self.dpi)) as f32,
                bottom: (readout_y + readout_h) as f32,
            },
            &readout_ink,
        );
        self.draw_week_legend(inner_right, readout_y, readout_h, alpha);

        let days_rects: Vec<Rect> = layout
            .stats
            .days
            .iter()
            .map(|r| translate_rect(*r, -offset_x))
            .collect();
        let (Some(&first), Some(&last)) = (days_rects.first(), days_rects.last()) else {
            return;
        };
        if first.is_empty() || last.is_empty() {
            return;
        }

        let label_h = scale(18, self.dpi);
        let baseline = first.bottom() - label_h;
        let plot_top = first.y + scale(6, self.dpi);
        let max_bar_h = (baseline - plot_top).max(scale(14, self.dpi));
        let high_contrast = self.is_high_contrast_notch();

        let base_brush = self.brushes.alpha_brush(FOREGROUND, 0.12 * alpha);
        unsafe {
            self.target.DrawLine(
                Vector2::new(first.x as f32, baseline as f32 + 0.5),
                Vector2::new(last.right() as f32, baseline as f32 + 0.5),
                &base_brush,
                1.0,
                None,
            );
        }

        for (index, (day, col)) in stats.weekly_days.iter().zip(&days_rects).enumerate() {
            let is_hovered = hovered == Some(HitTarget::FocusStatsDay(index as u8));
            let is_selected = stats.selected_day == Some(index);
            let style = WeekDayColumnStyle {
                baseline,
                max_bar_h,
                is_active: stats.active_day == Some(index) || is_hovered,
                is_selected,
                high_contrast,
                alpha,
            };
            self.draw_week_day_column(day, *col, style);
        }

        if stats.weekly_avg_percent > 0 {
            self.draw_week_average_line(
                first.x,
                last.right(),
                plot_top,
                baseline,
                max_bar_h,
                stats.weekly_avg_percent,
                high_contrast,
                alpha,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_week_average_line(
        &self,
        left: i32,
        right: i32,
        plot_top: i32,
        baseline: i32,
        max_bar_h: i32,
        avg_percent: u8,
        high_contrast: bool,
        alpha: f32,
    ) {
        let avg_h = (i32::from(avg_percent) * max_bar_h / 100).clamp(scale(3, self.dpi), max_bar_h);
        let avg_y = (baseline - avg_h) as f32 + 0.5;
        let avg_line = self.brushes.alpha_brush(
            SHIELD_GREEN,
            if high_contrast { 0.80 } else { 0.44 } * alpha,
        );
        unsafe {
            self.target.DrawLine(
                Vector2::new(left as f32, avg_y),
                Vector2::new(right as f32, avg_y),
                &avg_line,
                1.0,
                &self.dashed_stroke_style,
            );
        }
        let avg_ink = self.brushes.alpha_brush(SHIELD_GREEN, 0.80 * alpha);
        let avg_label_h = scale(12, self.dpi) as f32;
        self.draw_text(
            "avg",
            &self.text_stats_trailing,
            D2D_RECT_F {
                left: (right - scale(36, self.dpi)) as f32,
                top: (avg_y - avg_label_h - 1.0).max(plot_top as f32 - 2.0),
                right: right as f32,
                bottom: avg_y,
            },
            &avg_ink,
        );
    }

    fn draw_week_legend(&self, right: i32, y: i32, height: i32, alpha: f32) {
        let swatch = scale(6, self.dpi);
        let swatch_y = y + (height - swatch) / 2;
        let muted = self.brushes.alpha_brush(FOREGROUND, 0.54 * alpha);

        let focus_text_w = scale(34, self.dpi);
        let focus_swatch_x = right - focus_text_w - swatch - scale(4, self.dpi);
        let focus_box = Rect {
            x: focus_swatch_x,
            y: swatch_y,
            width: swatch,
            height: swatch,
        };
        let focus_brush = self.brushes.alpha_brush(FOCUS_ORANGE, 0.94 * alpha);
        self.draw_rounded_box(focus_box, 2.0, &focus_brush);
        self.draw_text(
            "Focus",
            &self.text_caption,
            D2D_RECT_F {
                left: (focus_box.right() + scale(4, self.dpi)) as f32,
                top: y as f32,
                right: right as f32,
                bottom: (y + height) as f32,
            },
            &muted,
        );

        let screen_text_w = scale(38, self.dpi);
        let screen_swatch_x =
            focus_swatch_x - scale(8, self.dpi) - screen_text_w - swatch - scale(4, self.dpi);
        let screen_box = Rect {
            x: screen_swatch_x,
            y: swatch_y,
            width: swatch,
            height: swatch,
        };
        let screen_brush = self.brushes.alpha_brush(SCREEN_TIME_BLUE, 0.80 * alpha);
        self.draw_rounded_box(screen_box, 2.0, &screen_brush);
        self.draw_text(
            "Screen",
            &self.text_caption,
            D2D_RECT_F {
                left: (screen_box.right() + scale(4, self.dpi)) as f32,
                top: y as f32,
                right: (focus_swatch_x - scale(6, self.dpi)) as f32,
                bottom: (y + height) as f32,
            },
            &muted,
        );
    }

    fn draw_week_day_column(&self, day: &DayBar, col: Rect, style: WeekDayColumnStyle) {
        if style.is_active {
            self.draw_week_day_backdrop(col, style);
        }
        self.draw_week_day_bars(day, col, style);
        self.draw_week_day_label(day, col, style);
    }

    fn draw_week_day_backdrop(&self, col: Rect, style: WeekDayColumnStyle) {
        let col_r = scale(6, self.dpi) as f32;
        let hover_fill = self.brushes.alpha_brush(FOREGROUND, 0.08 * style.alpha);
        self.draw_rounded_box(col, col_r, &hover_fill);
        if style.is_selected || style.high_contrast {
            let rim = self.brushes.alpha_brush(
                if style.high_contrast {
                    FOREGROUND
                } else {
                    FOCUS_ORANGE
                },
                if style.high_contrast { 0.72 } else { 0.42 } * style.alpha,
            );
            let frame = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: col.x as f32 + 0.5,
                    top: col.y as f32 + 0.5,
                    right: col.right() as f32 - 0.5,
                    bottom: col.bottom() as f32 - 0.5,
                },
                radiusX: col_r,
                radiusY: col_r,
            };
            unsafe {
                self.target.DrawRoundedRectangle(&frame, &rim, 1.0, None);
            }
        }
    }

    fn draw_week_day_bars(&self, day: &DayBar, col: Rect, style: WeekDayColumnStyle) {
        let bar_w =
            scale(18, self.dpi).min((col.width - scale(8, self.dpi)).max(scale(6, self.dpi)));
        let bar_x = col.x + (col.width - bar_w) / 2;
        let bar_r = scale(4, self.dpi) as f32;

        let total_h = if day.active_ms > 0 {
            (i32::from(day.active_percent) * style.max_bar_h / 100).max(scale(4, self.dpi))
        } else {
            scale(2, self.dpi)
        };
        let total_rect = Rect {
            x: bar_x,
            y: style.baseline - total_h,
            width: bar_w,
            height: total_h,
        };
        let total_brush = if day.active_ms == 0 {
            self.brushes.alpha_brush(FOREGROUND, 0.14 * style.alpha)
        } else {
            let opacity = if style.is_active || day.is_today {
                0.84
            } else {
                0.52
            };
            self.brushes
                .alpha_brush(SCREEN_TIME_BLUE, opacity * style.alpha)
        };
        self.draw_rounded_box(total_rect, bar_r.min(total_h as f32 * 0.5), &total_brush);

        if day.focus_ms > 0 {
            let focus_h = (i32::from(day.focus_percent) * style.max_bar_h / 100)
                .max(scale(4, self.dpi))
                .min(total_h);
            let focus_rect = Rect {
                x: bar_x,
                y: style.baseline - focus_h,
                width: bar_w,
                height: focus_h,
            };
            let focus_opacity = if style.is_active || day.is_today {
                0.96
            } else {
                0.82
            };
            let focus_brush = self
                .brushes
                .alpha_brush(FOCUS_ORANGE, focus_opacity * style.alpha);
            self.draw_rounded_box(focus_rect, bar_r.min(focus_h as f32 * 0.5), &focus_brush);
            if style.high_contrast {
                let hc_rim = self.brushes.alpha_brush(FOREGROUND, 0.85 * style.alpha);
                let frame = D2D1_ROUNDED_RECT {
                    rect: rect_to_d2d(focus_rect),
                    radiusX: bar_r.min(focus_h as f32 * 0.5),
                    radiusY: bar_r.min(focus_h as f32 * 0.5),
                };
                unsafe {
                    self.target.DrawRoundedRectangle(&frame, &hc_rim, 1.0, None);
                }
            }
        }
    }

    fn draw_week_day_label(&self, day: &DayBar, col: Rect, style: WeekDayColumnStyle) {
        let label_ink = self.brushes.alpha_brush(
            FOREGROUND,
            if style.is_active || day.is_today {
                0.94
            } else {
                0.52
            } * style.alpha,
        );
        self.draw_text(
            &day.day_label,
            &self.text_caption_center,
            D2D_RECT_F {
                left: col.x as f32,
                top: (style.baseline + scale(2, self.dpi)) as f32,
                right: col.right() as f32,
                bottom: (col.bottom() - scale(3, self.dpi)) as f32,
            },
            &label_ink,
        );
        if day.is_today {
            let dot_r = (scale(1, self.dpi) as f32 * 1.6).max(1.5);
            let dot_brush = self.brushes.alpha_brush(FOCUS_ORANGE, 0.96 * style.alpha);
            unsafe {
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(
                            col.x as f32 + col.width as f32 * 0.5,
                            col.bottom() as f32 - dot_r,
                        ),
                        radiusX: dot_r,
                        radiusY: dot_r,
                    },
                    &dot_brush,
                );
            }
        }
    }

    pub(crate) fn draw_stats_bars_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (r * 0.34).max(1.3);
        let spacing = r * 0.72;
        let bottom = cy + r * 0.78;
        let heights = [r * 0.72, r * 1.48, r * 1.08];
        let xs = [cx - spacing, cx, cx + spacing];
        unsafe {
            for (x, h) in xs.into_iter().zip(heights) {
                self.target.DrawLine(
                    Vector2::new(x, bottom),
                    Vector2::new(x, bottom - h),
                    brush,
                    stroke,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_right_chevron(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (r * 0.48).max(1.25);
        unsafe {
            self.target.DrawLine(
                Vector2::new(cx - r * 0.5, cy - r),
                Vector2::new(cx + r * 0.5, cy),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + r * 0.5, cy),
                Vector2::new(cx - r * 0.5, cy + r),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }
}
