use crate::notch::focus_activity::{
    FOCUS_HISTORY_DAYS, FocusActivityState, clean_app_display_name, format_screen_time,
    is_browser_executable, is_non_trackable_focus_app, normalize_app_key,
};
use crate::settings::focus_settings::{DayBar, build_weekly_chart};

pub const NOTCH_STATS_TOP_APPS: usize = 3;
pub const NOTCH_STATS_DAYS: usize = FOCUS_HISTORY_DAYS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusStatsRange {
    #[default]
    Today,
    Week,
}

impl FocusStatsRange {
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Today => 0,
            Self::Week => 1,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "7 Days",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusUsageRow {
    pub app_key: String,
    pub label: String,
    pub badge: String,
    pub duration_text: String,
    pub active_ms: u64,
    pub share_percent: u8,
    pub color_index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusStatsView {
    pub range: FocusStatsRange,
    pub today_screen_time: String,
    pub today_focus_time: String,
    pub today_focus_percent: u8,
    pub today_blocked_count: u32,
    pub top_apps: Vec<FocusUsageRow>,
    pub weekly_days: Vec<DayBar>,
    pub weekly_avg_percent: u8,
    pub weekly_avg_label: String,
    pub weekly_readout: String,
    pub weekly_blocked_count: u32,
    pub active_day: Option<usize>,
    pub selected_day: Option<usize>,
    pub has_today_activity: bool,
    pub has_week_activity: bool,
}

impl Default for FocusStatsView {
    fn default() -> Self {
        Self::resolve(
            &FocusActivityState::default(),
            FocusStatsRange::Today,
            None,
            None,
        )
    }
}

fn rounded_percent(part: u64, whole: u64) -> u8 {
    if whole == 0 || part == 0 {
        return 0;
    }
    let clamped = part.min(whole);
    let pct = (clamped.saturating_mul(100) + whole / 2) / whole;
    u8::try_from(pct.min(100)).unwrap_or(100)
}

fn app_badge_initial(label: &str) -> String {
    label
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map_or_else(|| String::from("?"), |ch| ch.to_uppercase().to_string())
}

fn app_color_index(app_key: &str) -> u8 {
    let mut hash: u32 = 2_166_136_261;
    for byte in app_key.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash % 6) as u8
}

struct WeeklyStatsSummary {
    days: Vec<DayBar>,
    avg_percent: u8,
    avg_label: String,
    readout: String,
    blocked_count: u32,
    active_day: Option<usize>,
    selected_day: Option<usize>,
    has_activity: bool,
}

fn resolve_top_apps(activity: &FocusActivityState) -> Vec<FocusUsageRow> {
    let top_site = activity
        .today
        .sorted_websites()
        .into_iter()
        .find(|site| site.active_ms > 0)
        .map(|site| site.domain);
    let ranked_apps: Vec<_> = activity
        .today
        .sorted_apps()
        .into_iter()
        .filter(|app| {
            app.active_ms > 0 && !is_non_trackable_focus_app(&app.app_key, &app.display_name)
        })
        .take(NOTCH_STATS_TOP_APPS)
        .collect();
    let peak_app_ms = ranked_apps.first().map_or(0, |app| app.active_ms);
    let mut attached_site = false;
    ranked_apps
        .into_iter()
        .map(|app| {
            let key = normalize_app_key(&app.app_key);
            let clean_name = clean_app_display_name(&app.display_name, &key);
            let label = if !attached_site
                && is_browser_executable(&key)
                && let Some(domain) = top_site.as_deref()
            {
                attached_site = true;
                format!("{clean_name} · {domain}")
            } else {
                clean_name.clone()
            };
            FocusUsageRow {
                badge: app_badge_initial(&clean_name),
                color_index: app_color_index(&key),
                duration_text: format_screen_time(app.active_ms),
                share_percent: rounded_percent(app.active_ms, peak_app_ms).max(4),
                active_ms: app.active_ms,
                app_key: key,
                label,
            }
        })
        .collect()
}

fn resolve_weekly_stats(
    activity: &FocusActivityState,
    selected_day: Option<usize>,
    hovered_day: Option<usize>,
) -> WeeklyStatsSummary {
    let chart = build_weekly_chart(activity, selected_day);
    let summary = activity.rolling_summary();
    let avg_ms = summary.total_active_ms / u64::try_from(summary.days_counted.max(1)).unwrap_or(1);
    let weekly_peak = chart
        .days
        .iter()
        .map(|day| day.active_ms)
        .max()
        .unwrap_or(0);
    let avg_percent = if weekly_peak > 0 && avg_ms > 0 {
        rounded_percent(avg_ms, weekly_peak).max(1)
    } else {
        0
    };
    let avg_label = if avg_ms > 0 {
        format_screen_time(avg_ms)
    } else {
        String::new()
    };
    let active_day = hovered_day
        .or(chart.selected_day)
        .filter(|index| *index < chart.days.len());
    let readout = if let Some(day) = active_day.and_then(|idx| chart.days.get(idx)) {
        let prefix = if day.is_today {
            format!("Today ({})", day.day_label)
        } else {
            day.day_label.clone()
        };
        format!(
            "{prefix} · {} · {} focus",
            format_screen_time(day.active_ms),
            format_screen_time(day.focus_ms)
        )
    } else if summary.total_active_ms == 0 {
        String::from("No screen time recorded yet")
    } else {
        format!(
            "Avg {}/day · {}% focus",
            format_screen_time(avg_ms),
            chart.focus_ratio_percent
        )
    };

    WeeklyStatsSummary {
        days: chart.days,
        avg_percent,
        avg_label,
        readout,
        blocked_count: summary.blocked_attempts,
        active_day,
        selected_day: chart.selected_day,
        has_activity: summary.total_active_ms > 0,
    }
}

impl FocusStatsView {
    #[must_use]
    pub fn resolve(
        activity: &FocusActivityState,
        range: FocusStatsRange,
        selected_day: Option<usize>,
        hovered_day: Option<usize>,
    ) -> Self {
        let today_total = activity.today.total_active_ms;
        let today_focus = activity.today.focus_session_ms.min(today_total);
        let today_screen_time = format_screen_time(today_total);
        let today_focus_time = format_screen_time(today_focus);
        let today_focus_percent = rounded_percent(today_focus, today_total);
        let top_apps = resolve_top_apps(activity);
        let weekly = resolve_weekly_stats(activity, selected_day, hovered_day);

        Self {
            range,
            today_screen_time,
            today_focus_time,
            today_focus_percent,
            today_blocked_count: activity.today.blocked_attempts,
            top_apps,
            weekly_days: weekly.days,
            weekly_avg_percent: weekly.avg_percent,
            weekly_avg_label: weekly.avg_label,
            weekly_readout: weekly.readout,
            weekly_blocked_count: weekly.blocked_count,
            active_day: weekly.active_day,
            selected_day: weekly.selected_day,
            has_today_activity: today_total > 0,
            has_week_activity: weekly.has_activity,
        }
    }

    #[must_use]
    pub const fn active_blocked_count(&self) -> u32 {
        match self.range {
            FocusStatsRange::Today => self.today_blocked_count,
            FocusStatsRange::Week => self.weekly_blocked_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notch::focus_activity::{
        AppUsageEntry, DailyActivityRecord, DayKey, WebsiteUsageEntry,
    };

    fn sample_activity() -> FocusActivityState {
        let mut state = FocusActivityState::default();
        state.today = DailyActivityRecord {
            day: DayKey {
                year: 2026,
                month: 11,
                day: 22,
            },
            total_active_ms: 3 * 3_600_000 + 20 * 60_000,
            focus_session_ms: 80 * 60_000,
            blocked_attempts: 2,
            apps: vec![
                AppUsageEntry {
                    app_key: String::from("code"),
                    display_name: String::from("VS Code"),
                    active_ms: 120 * 60_000,
                    open_count: 3,
                    click_count: 100,
                    keystroke_count: 500,
                },
                AppUsageEntry {
                    app_key: String::from("chrome"),
                    display_name: String::from("Chrome"),
                    active_ms: 60 * 60_000,
                    open_count: 5,
                    click_count: 80,
                    keystroke_count: 120,
                },
                AppUsageEntry {
                    app_key: String::from("windowsterminal"),
                    display_name: String::from("Terminal"),
                    active_ms: 20 * 60_000,
                    open_count: 2,
                    click_count: 10,
                    keystroke_count: 90,
                },
                AppUsageEntry {
                    app_key: String::from("notepad"),
                    display_name: String::from("Notepad"),
                    active_ms: 5 * 60_000,
                    open_count: 1,
                    click_count: 2,
                    keystroke_count: 10,
                },
            ],
            websites: vec![WebsiteUsageEntry {
                domain: String::from("github.com"),
                active_ms: 35 * 60_000,
                visit_count: 4,
            }],
            ..DailyActivityRecord::default()
        };
        state.history.push(DailyActivityRecord {
            day: DayKey {
                year: 2026,
                month: 11,
                day: 21,
            },
            total_active_ms: 100 * 60_000,
            focus_session_ms: 50 * 60_000,
            blocked_attempts: 1,
            ..DailyActivityRecord::default()
        });
        state
    }

    #[test]
    fn empty_activity_resolves_cleanly_without_panics_or_fake_data() {
        let view = FocusStatsView::default();
        assert_eq!(view.today_screen_time, "0m");
        assert_eq!(view.today_focus_time, "0m");
        assert_eq!(view.today_focus_percent, 0);
        assert!(view.top_apps.is_empty());
        assert_eq!(view.weekly_days.len(), NOTCH_STATS_DAYS);
        assert_eq!(view.weekly_avg_percent, 0);
        assert!(view.weekly_avg_label.is_empty());
        assert_eq!(view.weekly_readout, "No screen time recorded yet");
        assert!(!view.has_today_activity);
        assert!(!view.has_week_activity);
    }

    #[test]
    fn today_view_ranks_top_three_apps_and_attaches_top_domain_to_browser() {
        let activity = sample_activity();
        let view = FocusStatsView::resolve(&activity, FocusStatsRange::Today, None, None);
        assert_eq!(view.today_screen_time, "3h 20m");
        assert_eq!(view.today_focus_time, "1h 20m");
        assert_eq!(view.today_focus_percent, 40);
        assert_eq!(view.today_blocked_count, 2);
        assert_eq!(view.active_blocked_count(), 2);
        assert_eq!(view.top_apps.len(), 3);
        assert_eq!(view.top_apps[0].label, "VS Code");
        assert_eq!(view.top_apps[0].badge, "V");
        assert_eq!(view.top_apps[0].share_percent, 100);
        assert_eq!(view.top_apps[1].label, "Chrome · github.com");
        assert_eq!(view.top_apps[1].share_percent, 50);
        assert_eq!(view.top_apps[2].label, "Terminal");
        assert_eq!(view.top_apps[2].duration_text, "20m");
        assert!(view.has_today_activity);
    }

    #[test]
    fn week_view_computes_average_line_and_hover_readout_precedence() {
        let activity = sample_activity();
        let default_week = FocusStatsView::resolve(&activity, FocusStatsRange::Week, None, None);
        assert_eq!(default_week.weekly_days.len(), 7);
        assert_eq!(default_week.weekly_avg_label, "2h 30m");
        assert_eq!(default_week.weekly_avg_percent, 75);
        assert_eq!(default_week.weekly_readout, "Avg 2h 30m/day · 43% focus");
        assert_eq!(default_week.active_blocked_count(), 3);

        let hovered = FocusStatsView::resolve(&activity, FocusStatsRange::Week, Some(5), Some(6));
        assert_eq!(hovered.active_day, Some(6));
        assert_eq!(hovered.selected_day, Some(5));
        assert!(hovered.weekly_readout.starts_with("Today ("));
        assert!(hovered.weekly_readout.contains("3h 20m"));
        assert!(hovered.weekly_readout.contains("1h 20m focus"));

        let selected_only =
            FocusStatsView::resolve(&activity, FocusStatsRange::Week, Some(5), None);
        assert_eq!(selected_only.active_day, Some(5));
        assert!(selected_only.weekly_readout.contains("1h 40m"));
        assert!(selected_only.weekly_readout.contains("50m focus"));
    }
}
