use std::collections::{BTreeMap, HashMap};

use crate::bar::calendar::{days_in_month, weekday_of};
use crate::notch::focus_activity::{
    DayKey, FOCUS_HISTORY_DAYS, FocusActivityState, PRESET_BLOCKED_APPS, PRESET_BLOCKED_WEBSITES,
    clean_app_display_name, compact_app_id, estimated_words, format_compact_count,
    format_screen_time, is_non_trackable_focus_app, normalize_app_key, normalize_domain,
};
use crate::settings::model::{OttoSettingItem, OttoSettingKind, OttoSettingsSnapshot};
use crate::settings_content::{
    BadgeTone, SettingsAction, SettingsContent, SettingsRow, SettingsRowValue, SettingsSection,
    module_badge, percent_of, setting_row,
};
use crate::settings_window::{ModuleId, ModuleRow, SettingsPage};

pub const FOCUS_ALL_APPS_INITIAL_LIMIT: usize = 12;
pub const FOCUS_ALL_APPS_PAGE_STEP: usize = 12;
pub const FOCUS_SEARCH_MAX_RESULTS: usize = 24;
pub const FOCUS_SEARCH_MAX_CHARS: usize = 80;
pub const FOCUS_WEBSITE_INPUT_MAX_CHARS: usize = 120;
const SUGGESTED_WEBSITES_DEFAULT_COUNT: usize = 6;
const WEEKDAY_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusSettingsTab {
    #[default]
    BlockApps,
    Activity,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusAppFilter {
    Blocked,
    #[default]
    Recent,
    AllApps,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusTextField {
    #[default]
    None,
    AppSearch,
    WebsiteDomain,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusSessionStatus {
    #[default]
    NotRunning,
    RunningCountdown {
        remaining_seconds: u32,
    },
    RunningStopwatch {
        elapsed_seconds: u32,
    },
    PausedCountdown {
        remaining_seconds: u32,
    },
    PausedStopwatch {
        elapsed_seconds: u32,
    },
    OnBreak {
        remaining_seconds: u32,
    },
}

impl FocusSessionStatus {
    #[must_use]
    pub const fn is_blocking_now(self) -> bool {
        matches!(
            self,
            Self::RunningCountdown { .. } | Self::RunningStopwatch { .. }
        )
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::NotRunning => String::from("Not running"),
            Self::RunningCountdown { remaining_seconds } => {
                format!("{} remaining", format_remaining_brief(remaining_seconds))
            }
            Self::RunningStopwatch { elapsed_seconds } => {
                format!("Running ({})", format_remaining_brief(elapsed_seconds))
            }
            Self::PausedCountdown { remaining_seconds } => {
                format!(
                    "Paused ({} left)",
                    format_remaining_brief(remaining_seconds)
                )
            }
            Self::PausedStopwatch { elapsed_seconds } => {
                format!("Paused ({})", format_remaining_brief(elapsed_seconds))
            }
            Self::OnBreak { remaining_seconds } => {
                format!("Break ({} left)", format_remaining_brief(remaining_seconds))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusCatalogApp {
    pub block_key: String,
    pub display_name: String,
    pub detail: String,
    pub icon_source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentedOption {
    pub label: &'static str,
    pub action: SettingsAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterChip {
    pub label: String,
    pub action: SettingsAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KpiItem {
    pub label: &'static str,
    pub value: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DayBar {
    pub day_label: String,
    pub active_ms: u64,
    pub focus_ms: u64,
    pub active_percent: u8,
    pub focus_percent: u8,
    pub is_today: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeeklyChart {
    pub days: Vec<DayBar>,
    pub summary_text: String,
    pub selected_day: Option<usize>,
    pub focus_ratio_percent: u8,
}

impl WeeklyChart {
    #[must_use]
    pub fn day_readout(&self, index: usize) -> Option<String> {
        let day = self.days.get(index)?;
        let prefix = if day.is_today {
            format!("Today ({})", day.day_label)
        } else {
            day.day_label.clone()
        };
        Some(format!(
            "{prefix} · Total {} · Focus {}",
            format_screen_time(day.active_ms),
            format_screen_time(day.focus_ms)
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FocusPageState {
    pub tab: FocusSettingsTab,
    pub app_filter: FocusAppFilter,
    pub app_search: String,
    pub all_apps_limit: usize,
    pub website_input: String,
    pub website_error: Option<String>,
    pub focused_field: FocusTextField,
    pub details_expanded: bool,
    pub selected_bar_day: Option<usize>,
    pub catalog: Vec<FocusCatalogApp>,
    pub session: FocusSessionStatus,
}

impl Default for FocusPageState {
    fn default() -> Self {
        Self {
            tab: FocusSettingsTab::BlockApps,
            app_filter: FocusAppFilter::Recent,
            app_search: String::new(),
            all_apps_limit: FOCUS_ALL_APPS_INITIAL_LIMIT,
            website_input: String::new(),
            website_error: None,
            focused_field: FocusTextField::None,
            details_expanded: false,
            selected_bar_day: None,
            catalog: Vec::new(),
            session: FocusSessionStatus::NotRunning,
        }
    }
}

impl FocusPageState {
    pub fn select_tab(&mut self, tab: FocusSettingsTab) {
        if self.tab != tab {
            self.tab = tab;
            self.focused_field = FocusTextField::None;
            self.website_error = None;
        }
    }

    pub fn select_filter(&mut self, filter: FocusAppFilter) {
        self.app_filter = filter;
        if filter == FocusAppFilter::AllApps {
            self.all_apps_limit = FOCUS_ALL_APPS_INITIAL_LIMIT;
        }
    }

    pub fn expand_all_apps(&mut self) {
        self.all_apps_limit = self.all_apps_limit.saturating_add(FOCUS_ALL_APPS_PAGE_STEP);
    }

    pub fn focus_field(&mut self, field: FocusTextField) {
        self.focused_field = field;
        if field == FocusTextField::WebsiteDomain {
            self.website_error = None;
        }
    }

    pub fn clear_field(&mut self, field: FocusTextField) -> bool {
        match field {
            FocusTextField::None => false,
            FocusTextField::AppSearch => {
                if self.app_search.is_empty() {
                    false
                } else {
                    self.app_search.clear();
                    true
                }
            }
            FocusTextField::WebsiteDomain => {
                let had_text = !self.website_input.is_empty() || self.website_error.is_some();
                self.website_input.clear();
                self.website_error = None;
                had_text
            }
        }
    }

    pub fn insert_char(&mut self, ch: char) -> bool {
        if ch.is_control() {
            return false;
        }
        match self.focused_field {
            FocusTextField::None => false,
            FocusTextField::AppSearch => {
                if self.app_search.chars().count() >= FOCUS_SEARCH_MAX_CHARS {
                    return false;
                }
                self.app_search.push(ch);
                true
            }
            FocusTextField::WebsiteDomain => {
                if self.website_input.chars().count() >= FOCUS_WEBSITE_INPUT_MAX_CHARS {
                    return false;
                }
                self.website_input.push(ch);
                self.website_error = None;
                true
            }
        }
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        let mut changed = false;
        for ch in text.chars() {
            if ch == '\r' || ch == '\n' {
                break;
            }
            if !ch.is_control() && self.insert_char(ch) {
                changed = true;
            }
        }
        changed
    }

    pub fn backspace(&mut self) -> bool {
        match self.focused_field {
            FocusTextField::None => false,
            FocusTextField::AppSearch => self.app_search.pop().is_some(),
            FocusTextField::WebsiteDomain => {
                self.website_error = None;
                self.website_input.pop().is_some()
            }
        }
    }

    pub fn step_chart_day(&mut self, delta: i32) -> bool {
        if self.tab != FocusSettingsTab::Activity {
            return false;
        }
        let max_index = FOCUS_HISTORY_DAYS.saturating_sub(1) as i32;
        let current = self.selected_bar_day.map_or(max_index, |day| day as i32);
        let next = (current + delta).clamp(0, max_index) as usize;
        if self.selected_bar_day == Some(next) {
            return false;
        }
        self.selected_bar_day = Some(next);
        true
    }
}

#[must_use]
pub fn format_remaining_brief(seconds: u32) -> String {
    if seconds == 0 {
        return String::from("0 min");
    }
    if seconds < 60 {
        return String::from("< 1 min");
    }
    let minutes = seconds.div_ceil(60);
    let hours = minutes / 60;
    let rem_minutes = minutes % 60;
    if hours > 0 && rem_minutes > 0 {
        format!("{hours}h {rem_minutes}m")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        format!("{minutes} min")
    }
}

#[must_use]
pub fn previous_day(day: DayKey) -> DayKey {
    if !day.is_valid() {
        return DayKey::default();
    }
    if day.day > 1 {
        return DayKey {
            year: day.year,
            month: day.month,
            day: day.day - 1,
        };
    }
    let (year, month) = if day.month > 1 {
        (day.year, day.month - 1)
    } else {
        (day.year.saturating_sub(1).max(2000), 12)
    };
    DayKey {
        year,
        month,
        day: days_in_month(year, month),
    }
}

#[must_use]
pub fn weekday_short(day: DayKey) -> &'static str {
    if !day.is_valid() {
        return "Day";
    }
    WEEKDAY_SHORT[usize::from(weekday_of(day.year, day.month, day.day)) % 7]
}

fn collect_weekly_slots(activity: &FocusActivityState) -> Vec<(String, u64, u64, bool)> {
    let mut records_by_day: HashMap<DayKey, (u64, u64)> = HashMap::new();
    for record in &activity.history {
        if record.day.is_valid() {
            records_by_day.insert(
                record.day,
                (record.total_active_ms, record.focus_session_ms),
            );
        }
    }
    if activity.today.day.is_valid() {
        records_by_day.insert(
            activity.today.day,
            (
                activity.today.total_active_ms,
                activity.today.focus_session_ms,
            ),
        );
    }

    let mut slots = Vec::with_capacity(FOCUS_HISTORY_DAYS);
    if activity.today.day.is_valid() {
        let mut cursor = activity.today.day;
        let mut keys = Vec::with_capacity(FOCUS_HISTORY_DAYS);
        for _ in 0..FOCUS_HISTORY_DAYS {
            keys.push(cursor);
            cursor = previous_day(cursor);
        }
        keys.reverse();
        for (index, key) in keys.into_iter().enumerate() {
            let (active_ms, focus_ms) = records_by_day.get(&key).copied().unwrap_or((0, 0));
            let is_today = index + 1 == FOCUS_HISTORY_DAYS;
            slots.push((weekday_short(key).to_owned(), active_ms, focus_ms, is_today));
        }
        return slots;
    }

    let fallback_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Today"];
    let mut hist: Vec<(u64, u64)> = activity
        .history
        .iter()
        .take(FOCUS_HISTORY_DAYS.saturating_sub(1))
        .rev()
        .map(|record| (record.total_active_ms, record.focus_session_ms))
        .collect();
    while hist.len() < FOCUS_HISTORY_DAYS.saturating_sub(1) {
        hist.insert(0, (0, 0));
    }
    hist.push((
        activity.today.total_active_ms,
        activity.today.focus_session_ms,
    ));
    for (index, (active_ms, focus_ms)) in hist.into_iter().enumerate() {
        let label = fallback_labels[index % fallback_labels.len()];
        slots.push((
            String::from(label),
            active_ms,
            focus_ms,
            index + 1 == FOCUS_HISTORY_DAYS,
        ));
    }
    slots
}

#[must_use]
pub fn build_weekly_chart(
    activity: &FocusActivityState,
    selected_day: Option<usize>,
) -> WeeklyChart {
    let slots = collect_weekly_slots(activity);
    let peak = slots
        .iter()
        .map(|(_, active, _, _)| *active)
        .max()
        .unwrap_or(0)
        .max(1);
    let days: Vec<DayBar> = slots
        .into_iter()
        .map(|(day_label, active_ms, focus_ms, is_today)| DayBar {
            day_label,
            active_ms,
            focus_ms: focus_ms.min(active_ms),
            active_percent: percent_of(active_ms, peak),
            focus_percent: percent_of(focus_ms.min(active_ms), peak),
            is_today,
        })
        .collect();

    let summary = activity.rolling_summary();
    let average_active_ms =
        summary.total_active_ms / u64::try_from(summary.days_counted.max(1)).unwrap_or(1);
    let focus_ratio_percent = if summary.total_active_ms > 0 {
        percent_of(
            summary.focus_session_ms.min(summary.total_active_ms),
            summary.total_active_ms,
        )
    } else {
        0
    };
    let summary_text = if summary.total_active_ms == 0 {
        String::from("No screen time recorded yet")
    } else {
        format!(
            "Avg {}/day · {}% in Focus",
            format_screen_time(average_active_ms),
            focus_ratio_percent
        )
    };
    let clamped_selected = selected_day.filter(|index| *index < days.len());
    WeeklyChart {
        days,
        summary_text,
        selected_day: clamped_selected,
        focus_ratio_percent,
    }
}

#[derive(Clone, Debug)]
struct ResolvedApp {
    block_key: String,
    display_name: String,
    detail: Option<String>,
    icon_source: Option<String>,
    today_active_ms: u64,
    week_active_ms: u64,
    open_count: u32,
    blocked: bool,
    in_recent: bool,
    in_catalog: bool,
}

fn prettify_app_key(key: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for ch in key.chars() {
        if ch == '-' || ch == '_' || ch == ' ' {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            capitalize = true;
        } else if capitalize {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() { key.to_owned() } else { out }
}

fn preset_label(key: &str) -> Option<&'static str> {
    PRESET_BLOCKED_APPS
        .iter()
        .find(|(preset_key, _)| *preset_key == key)
        .map(|(_, label)| *label)
}

fn find_matching_app_key(
    map: &BTreeMap<String, ResolvedApp>,
    key: &str,
    display_name: &str,
) -> Option<String> {
    if map.contains_key(key) {
        return Some(key.to_owned());
    }
    let compact_key = compact_app_id(key);
    let compact_display = compact_app_id(display_name);
    map.iter().find_map(|(existing_key, existing)| {
        let existing_compact_key = compact_app_id(existing_key);
        let existing_compact_display = compact_app_id(&existing.display_name);
        let matches_key = !compact_key.is_empty()
            && (compact_key == existing_compact_key || compact_key == existing_compact_display);
        let matches_display = !compact_display.is_empty()
            && (compact_display == existing_compact_key
                || compact_display == existing_compact_display);
        (matches_key || matches_display).then(|| existing_key.clone())
    })
}

fn insert_catalog_apps(
    map: &mut BTreeMap<String, ResolvedApp>,
    activity: &FocusActivityState,
    catalog: &[FocusCatalogApp],
) {
    for item in catalog {
        if is_non_trackable_focus_app(&item.block_key, &item.display_name)
            || (!item.detail.is_empty()
                && is_non_trackable_focus_app(&item.detail, &item.display_name))
        {
            continue;
        }
        let key = normalize_app_key(&item.block_key);
        if key.is_empty() {
            continue;
        }
        let target_key =
            find_matching_app_key(map, &key, &item.display_name).unwrap_or_else(|| key.clone());
        let blocked = activity
            .blocklist
            .is_app_blocked(&target_key, &item.display_name);
        let detail = (!item.detail.is_empty()).then(|| item.detail.clone());
        let icon_source = (!item.icon_source.is_empty()).then(|| item.icon_source.clone());
        map.entry(target_key.clone())
            .and_modify(|existing| {
                if existing.detail.is_none() {
                    existing.detail.clone_from(&detail);
                }
                if existing.icon_source.is_none() {
                    existing.icon_source.clone_from(&icon_source);
                }
                existing.blocked |= blocked;
                existing.in_catalog = true;
            })
            .or_insert_with(|| ResolvedApp {
                block_key: target_key,
                display_name: item.display_name.clone(),
                detail,
                icon_source,
                today_active_ms: 0,
                week_active_ms: 0,
                open_count: 0,
                blocked,
                in_recent: false,
                in_catalog: true,
            });
    }
}

fn insert_usage_apps(
    map: &mut BTreeMap<String, ResolvedApp>,
    activity: &FocusActivityState,
    apps: &[crate::AppUsageEntry],
    is_today: bool,
) {
    for app in apps {
        if is_non_trackable_focus_app(&app.app_key, &app.display_name) {
            continue;
        }
        let key = normalize_app_key(&app.app_key);
        if key.is_empty() {
            continue;
        }
        let clean_name = clean_app_display_name(&app.display_name, &key);
        let target_key =
            find_matching_app_key(map, &key, &clean_name).unwrap_or_else(|| key.clone());
        let blocked = activity.blocklist.is_app_blocked(&target_key, &clean_name);
        map.entry(target_key.clone())
            .and_modify(|existing| {
                if is_today {
                    existing.today_active_ms =
                        existing.today_active_ms.saturating_add(app.active_ms);
                    existing.open_count = existing.open_count.saturating_add(app.open_count);
                }
                existing.week_active_ms = existing.week_active_ms.saturating_add(app.active_ms);
                existing.blocked |= blocked;
                existing.in_recent = true;
            })
            .or_insert_with(|| ResolvedApp {
                block_key: target_key.clone(),
                display_name: if clean_name.is_empty() {
                    preset_label(&target_key)
                        .map_or_else(|| prettify_app_key(&target_key), str::to_owned)
                } else {
                    clean_name
                },
                detail: None,
                icon_source: None,
                today_active_ms: if is_today { app.active_ms } else { 0 },
                week_active_ms: app.active_ms,
                open_count: app.open_count,
                blocked,
                in_recent: true,
                in_catalog: false,
            });
    }
}

fn merge_focus_apps(
    activity: &FocusActivityState,
    catalog: &[FocusCatalogApp],
) -> Vec<ResolvedApp> {
    let mut map: BTreeMap<String, ResolvedApp> = BTreeMap::new();
    insert_catalog_apps(&mut map, activity, catalog);
    insert_usage_apps(&mut map, activity, &activity.today.apps, true);
    for day in &activity.history {
        insert_usage_apps(&mut map, activity, &day.apps, false);
    }

    for blocked_key in &activity.blocklist.blocked_apps {
        if is_non_trackable_focus_app(blocked_key, "") {
            continue;
        }
        let key = normalize_app_key(blocked_key);
        if key.is_empty() {
            continue;
        }
        let display =
            preset_label(&key).map_or_else(|| clean_app_display_name("", &key), str::to_owned);
        let target_key = find_matching_app_key(&map, &key, &display).unwrap_or_else(|| key.clone());
        map.entry(target_key.clone())
            .and_modify(|existing| {
                existing.blocked = true;
            })
            .or_insert_with(|| ResolvedApp {
                block_key: target_key,
                display_name: display,
                detail: None,
                icon_source: None,
                today_active_ms: 0,
                week_active_ms: 0,
                open_count: 0,
                blocked: true,
                in_recent: false,
                in_catalog: false,
            });
    }

    if catalog.is_empty() {
        for (preset_key, label) in PRESET_BLOCKED_APPS {
            let key = normalize_app_key(preset_key);
            let blocked = activity.blocklist.is_app_blocked(&key, label);
            map.entry(key.clone()).or_insert_with(|| ResolvedApp {
                block_key: key,
                display_name: String::from(label),
                detail: None,
                icon_source: None,
                today_active_ms: 0,
                week_active_ms: 0,
                open_count: 0,
                blocked,
                in_recent: false,
                in_catalog: true,
            });
        }
    }

    map.into_values().collect()
}

fn matches_app_query(app: &ResolvedApp, query_lower: &str) -> bool {
    query_lower.is_empty()
        || app.display_name.to_lowercase().contains(query_lower)
        || app.block_key.contains(query_lower)
}

fn app_usage_subtitle(app: &ResolvedApp) -> String {
    if app.today_active_ms > 0 {
        format!("Used {} today", format_screen_time(app.today_active_ms))
    } else if app.week_active_ms > 0 {
        format!("Used {} this week", format_screen_time(app.week_active_ms))
    } else if app.blocked {
        String::from("Blocked during Focus")
    } else {
        String::from("Installed app")
    }
}

fn app_toggle_row(app: &ResolvedApp) -> SettingsRow {
    let mut row = SettingsRow::new(
        app.display_name.clone(),
        app_usage_subtitle(app),
        SettingsRowValue::Toggle(app.blocked),
    )
    .with_action(SettingsAction::ToggleBlockedApp(app.block_key.clone()));
    row.app_icon = Some(
        app.icon_source
            .clone()
            .unwrap_or_else(|| app.block_key.clone()),
    );
    row.detail_hint.clone_from(&app.detail);
    row
}

fn sort_by_usage_then_name(items: &mut [&ResolvedApp]) {
    items.sort_by(|a, b| {
        b.today_active_ms
            .cmp(&a.today_active_ms)
            .then_with(|| b.week_active_ms.cmp(&a.week_active_ms))
            .then_with(|| b.open_count.cmp(&a.open_count))
            .then_with(|| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            })
    });
}

fn sort_alphabetically(items: &mut [&ResolvedApp]) {
    items.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
}

fn focus_header_section(
    snapshot: &OttoSettingsSnapshot,
    modules: &[ModuleRow],
    state: &FocusPageState,
) -> SettingsSection {
    let mut feature_row = setting_row(
        snapshot,
        OttoSettingItem::Toggle(OttoSettingKind::FocusModule),
        SettingsPage::Focus,
    );
    let module_row = modules
        .iter()
        .find(|row| row.id == ModuleId::Focus)
        .copied();
    let feature_paused = module_row.is_some_and(|row| row.paused_by.is_some());
    let feature_on = snapshot.is_enabled(OttoSettingKind::FocusModule) && !feature_paused;
    let feature_state = if feature_paused {
        "Paused (Needs Notch)"
    } else if feature_on {
        "On"
    } else {
        "Off"
    };
    feature_row.label = format!("Focus feature: {feature_state}");
    feature_row.subtitle = if !feature_on {
        String::from("Session: Not running · Turn on Focus to use sessions and blocking")
    } else if state.session.is_blocking_now() {
        format!("Session: {} · Blocking active now", state.session.label())
    } else {
        format!(
            "Session: {} · Blocking runs only during a Focus session",
            state.session.label()
        )
    };
    feature_row.badge = Some(if !feature_on {
        module_row.map_or(("Off", BadgeTone::Neutral), module_badge)
    } else if state.session.is_blocking_now() {
        ("Active", BadgeTone::Good)
    } else {
        ("Idle", BadgeTone::Neutral)
    });
    feature_row.muted = feature_paused;

    let tabs_row = SettingsRow::new(
        "",
        "",
        SettingsRowValue::Segmented {
            options: vec![
                SegmentedOption {
                    label: "Block apps",
                    action: SettingsAction::SetFocusTab(FocusSettingsTab::BlockApps),
                },
                SegmentedOption {
                    label: "Activity",
                    action: SettingsAction::SetFocusTab(FocusSettingsTab::Activity),
                },
            ],
            selected: match state.tab {
                FocusSettingsTab::BlockApps => 0,
                FocusSettingsTab::Activity => 1,
            },
        },
    );

    SettingsSection {
        title: "",
        footer: None,
        rows: vec![feature_row, tabs_row],
    }
}

fn empty_apps_row(
    filter: FocusAppFilter,
    query_lower: &str,
    all_query_matches: usize,
) -> SettingsRow {
    if !query_lower.is_empty() && filter != FocusAppFilter::AllApps && all_query_matches > 0 {
        let filter_name = match filter {
            FocusAppFilter::Blocked => "Blocked",
            FocusAppFilter::Recent => "Recent",
            FocusAppFilter::AllApps => "All apps",
        };
        return SettingsRow::new(
            format!("No matches in {filter_name}"),
            format!("Show {all_query_matches} matching installed apps in All apps"),
            SettingsRowValue::Detail("All apps"),
        )
        .with_action(SettingsAction::SetFocusAppFilter(FocusAppFilter::AllApps));
    }
    match filter {
        FocusAppFilter::Blocked => SettingsRow::new(
            if query_lower.is_empty() {
                "No apps blocked yet"
            } else {
                "No blocked apps match your search"
            },
            "Browse Recent or All apps to toggle apps to block during Focus",
            SettingsRowValue::Detail("All apps"),
        )
        .with_action(SettingsAction::SetFocusAppFilter(FocusAppFilter::AllApps)),
        FocusAppFilter::Recent => SettingsRow::new(
            if query_lower.is_empty() {
                "No recent apps detected yet"
            } else {
                "No recent apps match your search"
            },
            "Browse All apps to pick from installed Windows apps",
            SettingsRowValue::Detail("All apps"),
        )
        .with_action(SettingsAction::SetFocusAppFilter(FocusAppFilter::AllApps)),
        FocusAppFilter::AllApps => SettingsRow::new(
            "No installed apps match your search",
            "Try another name or choose an executable manually below",
            SettingsRowValue::Text(String::from("0")),
        ),
    }
}

fn build_during_focus_section(activity: &FocusActivityState) -> SettingsSection {
    let blocklist = &activity.blocklist;
    SettingsSection {
        title: "During Focus",
        footer: None,
        rows: vec![
            SettingsRow::new(
                "Block notifications",
                "Hide Notch banners and Dock badges while Focus is running",
                SettingsRowValue::Toggle(blocklist.block_notifications),
            )
            .with_action(SettingsAction::ToggleFocusBlockNotifications),
            SettingsRow::new(
                "Allow incoming calls",
                "Let ringing calls through unless the calling app is blocked",
                SettingsRowValue::Toggle(blocklist.allow_calls_during_focus),
            )
            .with_action(SettingsAction::ToggleFocusAllowCalls),
        ],
    }
}

fn build_block_apps_controls(
    state: &FocusPageState,
    blocked_count: usize,
    recent_count: usize,
    all_count: usize,
) -> Vec<SettingsRow> {
    vec![
        SettingsRow::new(
            "",
            "",
            SettingsRowValue::TextInput {
                field: FocusTextField::AppSearch,
                text: state.app_search.clone(),
                placeholder: "Search installed apps...",
                button_label: None,
                focused: state.focused_field == FocusTextField::AppSearch,
                error: None,
            },
        ),
        SettingsRow::new(
            "",
            "",
            SettingsRowValue::FilterChips {
                chips: vec![
                    FilterChip {
                        label: format!("Blocked ({blocked_count})"),
                        action: SettingsAction::SetFocusAppFilter(FocusAppFilter::Blocked),
                    },
                    FilterChip {
                        label: format!("Recent ({recent_count})"),
                        action: SettingsAction::SetFocusAppFilter(FocusAppFilter::Recent),
                    },
                    FilterChip {
                        label: format!("All apps ({all_count})"),
                        action: SettingsAction::SetFocusAppFilter(FocusAppFilter::AllApps),
                    },
                ],
                selected: match state.app_filter {
                    FocusAppFilter::Blocked => 0,
                    FocusAppFilter::Recent => 1,
                    FocusAppFilter::AllApps => 2,
                },
            },
        ),
    ]
}

fn build_block_apps_sections(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsSection> {
    let all = merge_focus_apps(activity, &state.catalog);
    let query_lower = state.app_search.trim().to_lowercase();
    let blocked_count = all.iter().filter(|app| app.blocked).count();
    let recent_count = all.iter().filter(|app| app.in_recent).count();
    let mut rows = build_block_apps_controls(state, blocked_count, recent_count, all.len());

    let mut filtered: Vec<&ResolvedApp> = all
        .iter()
        .filter(|app| match state.app_filter {
            FocusAppFilter::Blocked => app.blocked,
            FocusAppFilter::Recent => app.in_recent,
            FocusAppFilter::AllApps => app.in_catalog || app.in_recent || app.blocked,
        })
        .filter(|app| matches_app_query(app, &query_lower))
        .collect();

    if state.app_filter == FocusAppFilter::AllApps && query_lower.is_empty() {
        sort_alphabetically(&mut filtered);
    } else {
        sort_by_usage_then_name(&mut filtered);
    }

    let total_matching = filtered.len();
    let visible_limit = match state.app_filter {
        FocusAppFilter::AllApps if query_lower.is_empty() => state.all_apps_limit,
        FocusAppFilter::AllApps => FOCUS_SEARCH_MAX_RESULTS,
        FocusAppFilter::Blocked | FocusAppFilter::Recent => total_matching,
    };

    if filtered.is_empty() {
        let all_query_matches = if query_lower.is_empty() {
            0
        } else {
            all.iter()
                .filter(|app| matches_app_query(app, &query_lower))
                .count()
        };
        rows.push(empty_apps_row(
            state.app_filter,
            &query_lower,
            all_query_matches,
        ));
    } else {
        for app in filtered.iter().take(visible_limit) {
            rows.push(app_toggle_row(app));
        }
        if total_matching > visible_limit {
            let shown = visible_limit.min(total_matching);
            let mut more = SettingsRow::new(
                format!("Show more apps ({shown} of {total_matching})"),
                "Or type in the search box above to filter immediately",
                SettingsRowValue::Detail("More"),
            );
            more.action = (state.app_filter == FocusAppFilter::AllApps && query_lower.is_empty())
                .then_some(SettingsAction::ExpandFocusAllApps);
            rows.push(more);
        }
    }

    rows.push(
        SettingsRow::new(
            "+ Choose an app manually",
            "Pick an .exe for portable apps not listed above",
            SettingsRowValue::Detail(""),
        )
        .with_action(SettingsAction::PickBlockedAppFile),
    );

    vec![
        SettingsSection {
            title: "Block apps",
            footer: Some(
                "Toggles apply immediately during Focus. Covered apps get a Back overlay and are never closed.",
            ),
            rows,
        },
        build_during_focus_section(activity),
        SettingsSection {
            title: "Blocked websites",
            footer: Some(
                "Blocked browser tabs get a Back overlay during Focus. Type a domain and press Enter or click Add.",
            ),
            rows: build_blocked_websites_rows(activity, state),
        },
    ]
}

fn build_blocked_websites_rows(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsRow> {
    let blocklist = &activity.blocklist;
    let mut rows = vec![SettingsRow::new(
        "",
        "",
        SettingsRowValue::TextInput {
            field: FocusTextField::WebsiteDomain,
            text: state.website_input.clone(),
            placeholder: "Enter domain (e.g. youtube.com)...",
            button_label: Some("Add"),
            focused: state.focused_field == FocusTextField::WebsiteDomain,
            error: state.website_error.clone(),
        },
    )];

    let filter = state.website_input.trim().to_lowercase();
    let normalized_filter = normalize_domain(&filter);
    for domain in &blocklist.blocked_websites {
        let matches = filter.is_empty()
            || domain.contains(&filter)
            || normalized_filter
                .as_deref()
                .is_some_and(|norm| domain.contains(norm));
        if !matches {
            continue;
        }
        let subtitle = PRESET_BLOCKED_WEBSITES
            .iter()
            .find(|(preset, _)| *preset == domain)
            .map_or_else(
                || String::from("Blocked during Focus"),
                |(_, label)| format!("{label} · Blocked during Focus"),
            );
        rows.push(
            SettingsRow::new(domain.clone(), subtitle, SettingsRowValue::Toggle(true))
                .with_action(SettingsAction::ToggleBlockedWebsite(domain.clone())),
        );
    }

    let mut suggested_shown = 0usize;
    for (preset_domain, preset_label) in PRESET_BLOCKED_WEBSITES {
        if blocklist.is_website_blocked(preset_domain) {
            continue;
        }
        let matches = filter.is_empty()
            || preset_domain.contains(&filter)
            || preset_label.to_lowercase().contains(&filter);
        if !matches {
            continue;
        }
        if filter.is_empty() && suggested_shown >= SUGGESTED_WEBSITES_DEFAULT_COUNT {
            break;
        }
        suggested_shown += 1;
        rows.push(
            SettingsRow::new(
                preset_domain,
                format!("{preset_label} · Suggested"),
                SettingsRowValue::Toggle(false),
            )
            .with_action(SettingsAction::ToggleBlockedWebsite(String::from(
                preset_domain,
            ))),
        );
    }

    rows
}

fn build_kpi_section(activity: &FocusActivityState) -> SettingsSection {
    let today = &activity.today;
    let summary = activity.rolling_summary();
    let focus_pct = if today.total_active_ms > 0 {
        percent_of(
            today.focus_session_ms.min(today.total_active_ms),
            today.total_active_ms,
        )
    } else {
        0
    };
    SettingsSection {
        title: "",
        footer: None,
        rows: vec![SettingsRow::new(
            "",
            "",
            SettingsRowValue::KpiStrip([
                KpiItem {
                    label: "Screen time today",
                    value: format_screen_time(today.total_active_ms),
                    detail: String::from("Active time"),
                },
                KpiItem {
                    label: "Focus time",
                    value: format_screen_time(today.focus_session_ms),
                    detail: format!("{focus_pct}% of today"),
                },
                KpiItem {
                    label: "Blocked attempts",
                    value: today.blocked_attempts.to_string(),
                    detail: format!("7-day: {}", summary.blocked_attempts),
                },
            ]),
        )],
    }
}

fn build_most_used_apps_section(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> SettingsSection {
    let catalog_icons: HashMap<String, (String, Option<String>, Option<String>)> = state
        .catalog
        .iter()
        .map(|item| {
            (
                compact_app_id(&item.block_key),
                (
                    item.display_name.clone(),
                    (!item.icon_source.is_empty()).then(|| item.icon_source.clone()),
                    (!item.detail.is_empty()).then(|| item.detail.clone()),
                ),
            )
        })
        .collect();
    let sorted_apps: Vec<_> = activity
        .today
        .sorted_apps()
        .into_iter()
        .filter(|app| !is_non_trackable_focus_app(&app.app_key, &app.display_name))
        .collect();
    let app_peak = sorted_apps
        .first()
        .map_or(1, |entry| entry.active_ms.max(1));
    let mut app_rows: Vec<SettingsRow> = sorted_apps
        .into_iter()
        .take(8)
        .map(|app| {
            let key = normalize_app_key(&app.app_key);
            let compact = compact_app_id(&key);
            let (cat_name, icon_source, detail) = catalog_icons
                .get(&compact)
                .cloned()
                .unwrap_or_else(|| (String::new(), None, None));
            let label = if cat_name.is_empty() {
                clean_app_display_name(&app.display_name, &key)
            } else {
                cat_name
            };
            let blocked = activity.blocklist.is_app_blocked(&key, &label);
            let mut row = SettingsRow::new(
                label,
                "",
                SettingsRowValue::UsageBar {
                    duration: format_screen_time(app.active_ms),
                    percent: percent_of(app.active_ms, app_peak),
                    blocked,
                },
            )
            .with_action(SettingsAction::ToggleBlockedApp(key.clone()));
            row.app_icon = Some(icon_source.unwrap_or(key));
            row.detail_hint = detail;
            row
        })
        .collect();
    if app_rows.is_empty() {
        app_rows.push(SettingsRow::new(
            "No app activity recorded yet",
            "Use your apps normally to see daily screen time per app",
            SettingsRowValue::Text(String::from("Idle")),
        ));
    }
    SettingsSection {
        title: "Most-used apps today",
        footer: Some("Toggle any app to block or allow it directly during Focus."),
        rows: app_rows,
    }
}

fn build_activity_details_section(
    activity: &FocusActivityState,
    details_expanded: bool,
) -> SettingsSection {
    let today = &activity.today;
    let summary = activity.rolling_summary();
    let mut detail_rows = vec![
        SettingsRow::new(
            "Activity details",
            if details_expanded {
                "Clicks, keystrokes, search topics, and reset"
            } else {
                "Collapsed · Click to view clicks, keys, searches, and reset"
            },
            SettingsRowValue::Disclosure {
                expanded: details_expanded,
            },
        )
        .with_action(SettingsAction::ToggleFocusActivityDetails),
    ];

    if details_expanded {
        detail_rows.push(SettingsRow::new(
            "Clicks & keys today",
            format!(
                "7-day: {} clicks · {} keys (~{}w)",
                format_compact_count(summary.total_clicks),
                format_compact_count(summary.total_keystrokes),
                format_compact_count(estimated_words(summary.total_keystrokes))
            ),
            SettingsRowValue::Text(format!(
                "{} clicks · {} keys",
                format_compact_count(today.total_clicks),
                format_compact_count(today.total_keystrokes)
            )),
        ));

        let searches = today.sorted_searches();
        if searches.is_empty() {
            detail_rows.push(SettingsRow::new(
                "Most searched topics",
                "No searches recorded yet today",
                SettingsRowValue::Text(String::from("0")),
            ));
        } else {
            for search in searches.into_iter().take(6) {
                detail_rows.push(SettingsRow::new(
                    search.query,
                    format!("{} search", search.engine),
                    SettingsRowValue::Text(format!("{}x", search.search_count)),
                ));
            }
        }

        detail_rows.push(
            SettingsRow::new(
                "Reset Activity & Screen Time Stats",
                "Clear today and 7-day history while keeping your blocklist",
                SettingsRowValue::Detail("Clear"),
            )
            .with_action(SettingsAction::ResetFocusActivityStats),
        );
    }

    SettingsSection {
        title: "Details & data",
        footer: Some(
            "Saved locally in %LOCALAPPDATA%\\otto-os\\focus_activity.txt. Counts totals only; typed characters and passwords are never recorded.",
        ),
        rows: detail_rows,
    }
}

fn build_activity_sections(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsSection> {
    let chart = build_weekly_chart(activity, state.selected_bar_day);
    let chart_section = SettingsSection {
        title: "7-day trend",
        footer: Some(
            "Screen time pauses after 3 minutes of inactivity. Hover, click, or use Left/Right keys to inspect a day.",
        ),
        rows: vec![SettingsRow::new(
            "Screen time & Focus",
            chart.summary_text.clone(),
            SettingsRowValue::WeeklyChart(chart),
        )],
    };

    let mut sections = vec![
        build_kpi_section(activity),
        chart_section,
        build_most_used_apps_section(activity, state),
    ];

    let sorted_sites = activity.today.sorted_websites();
    if !sorted_sites.is_empty() {
        let site_peak = sorted_sites
            .first()
            .map_or(1, |entry| entry.active_ms.max(1));
        let site_rows: Vec<SettingsRow> = sorted_sites
            .into_iter()
            .take(6)
            .map(|site| {
                let blocked = activity.blocklist.is_website_blocked(&site.domain);
                SettingsRow::new(
                    site.domain.clone(),
                    "",
                    SettingsRowValue::UsageBar {
                        duration: format_screen_time(site.active_ms),
                        percent: percent_of(site.active_ms, site_peak),
                        blocked,
                    },
                )
                .with_action(SettingsAction::ToggleBlockedWebsite(site.domain))
            })
            .collect();
        sections.push(SettingsSection {
            title: "Top websites today",
            footer: Some("Detected natively from browser tabs with zero extensions."),
            rows: site_rows,
        });
    }

    sections.push(build_activity_details_section(
        activity,
        state.details_expanded,
    ));
    sections
}

#[must_use]
pub fn build_focus_page(
    snapshot: &OttoSettingsSnapshot,
    modules: &[ModuleRow],
    state: &FocusPageState,
) -> SettingsContent {
    let mut sections = vec![focus_header_section(snapshot, modules, state)];
    match state.tab {
        FocusSettingsTab::BlockApps => {
            sections.extend(build_block_apps_sections(&snapshot.focus_activity, state));
        }
        FocusSettingsTab::Activity => {
            sections.extend(build_activity_sections(&snapshot.focus_activity, state));
        }
    }
    SettingsContent {
        hero: None,
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notch::focus_activity::inspect_browser_context;
    use crate::settings::model::{PluginSettingsState, SystemSettingsState};
    use crate::settings_window::resolve_modules;

    fn snapshot_with_activity() -> OttoSettingsSnapshot {
        let mut snap = OttoSettingsSnapshot {
            plugins: PluginSettingsState {
                top_bar: true,
                notch: true,
                dock: true,
                window_controls: true,
                genie: true,
            },
            system: SystemSettingsState {
                focus_module: true,
                ..SystemSettingsState::default()
            },
            ..OttoSettingsSnapshot::default()
        };
        let day = DayKey {
            year: 2026,
            month: 11,
            day: 22,
        };
        let site = inspect_browser_context(
            Some("https://www.google.com/search?q=otto+focus"),
            "otto focus - Google Search - Google Chrome",
        );
        snap.focus_activity
            .observe_foreground(day, "chrome.exe", "Google Chrome", Some(site));
        snap.focus_activity
            .record_sample(day, 1_000, 0, true, 12, 60);
        snap.focus_activity
            .record_sample(day, 11_000, 0, true, 4, 25);
        snap.focus_activity
            .observe_foreground(day, "code.exe", "Visual Studio Code", None);
        snap.focus_activity
            .record_sample(day, 21_000, 0, false, 2, 10);
        assert!(snap.focus_activity.blocklist.add_app("discord"));
        assert!(snap.focus_activity.blocklist.add_website("reddit.com"));
        snap
    }

    fn sample_catalog() -> Vec<FocusCatalogApp> {
        vec![
            FocusCatalogApp {
                block_key: String::from("chrome"),
                display_name: String::from("Google Chrome"),
                detail: String::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
                icon_source: String::from(r"shell:AppsFolder\Chrome"),
            },
            FocusCatalogApp {
                block_key: String::from("discord"),
                display_name: String::from("Discord"),
                detail: String::from(r"C:\Users\me\AppData\Local\Discord\Discord.exe"),
                icon_source: String::from(r"shell:AppsFolder\Discord"),
            },
            FocusCatalogApp {
                block_key: String::from("code"),
                display_name: String::from("Visual Studio Code"),
                detail: String::from(r"C:\Apps\Code.exe"),
                icon_source: String::from(r"shell:AppsFolder\VSCode"),
            },
            FocusCatalogApp {
                block_key: String::from("calculator"),
                display_name: String::from("Calculator"),
                detail: String::from("Installed app"),
                icon_source: String::from(
                    r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
                ),
            },
        ]
    }

    #[test]
    fn header_disambiguates_focus_feature_from_running_session() {
        let snap = snapshot_with_activity();
        let modules = resolve_modules(&snap, |_| None);

        let idle_state = FocusPageState {
            session: FocusSessionStatus::NotRunning,
            ..FocusPageState::default()
        };
        let idle_page = build_focus_page(&snap, &modules, &idle_state);
        let status_row = &idle_page.sections[0].rows[0];
        assert_eq!(status_row.label, "Focus feature: On");
        assert!(status_row.subtitle.contains("Session: Not running"));
        assert_eq!(status_row.badge, Some(("Idle", BadgeTone::Neutral)));

        let running_state = FocusPageState {
            session: FocusSessionStatus::RunningCountdown {
                remaining_seconds: 24 * 60,
            },
            ..FocusPageState::default()
        };
        let running_page = build_focus_page(&snap, &modules, &running_state);
        let running_row = &running_page.sections[0].rows[0];
        assert_eq!(running_row.label, "Focus feature: On");
        assert!(running_row.subtitle.contains("24 min remaining"));
        assert_eq!(running_row.badge, Some(("Active", BadgeTone::Good)));
    }

    #[test]
    fn block_apps_tab_merges_usage_catalog_and_toggles_and_demotes_manual_picker() {
        let snap = snapshot_with_activity();
        let modules = resolve_modules(&snap, |_| None);
        let state = FocusPageState {
            tab: FocusSettingsTab::BlockApps,
            app_filter: FocusAppFilter::Recent,
            catalog: sample_catalog(),
            ..FocusPageState::default()
        };
        let page = build_focus_page(&snap, &modules, &state);
        let block_section = page
            .sections
            .iter()
            .find(|section| section.title == "Block apps")
            .expect("block apps section");
        let chrome = block_section
            .rows
            .iter()
            .find(|row| row.label == "Google Chrome")
            .expect("recent chrome row");
        assert_eq!(chrome.value, SettingsRowValue::Toggle(false));
        assert_eq!(
            chrome.action,
            Some(SettingsAction::ToggleBlockedApp(String::from("chrome")))
        );
        assert!(chrome.subtitle.contains("today"));
        assert!(
            chrome
                .detail_hint
                .as_deref()
                .is_some_and(|p| p.contains("chrome.exe"))
        );
        let last = block_section.rows.last().expect("manual fallback row");
        assert_eq!(last.label, "+ Choose an app manually");
        assert_eq!(last.action, Some(SettingsAction::PickBlockedAppFile));
    }

    #[test]
    fn all_apps_caps_unfiltered_list_and_search_filters_by_name() {
        let snap = snapshot_with_activity();
        let modules = resolve_modules(&snap, |_| None);
        let catalog: Vec<FocusCatalogApp> = (0..25)
            .map(|idx| FocusCatalogApp {
                block_key: format!("app{idx}"),
                display_name: format!("Tool {idx:02}"),
                detail: format!(r"C:\Apps\app{idx}.exe"),
                icon_source: format!(r"shell:AppsFolder\app{idx}"),
            })
            .collect();
        let mut state = FocusPageState {
            tab: FocusSettingsTab::BlockApps,
            app_filter: FocusAppFilter::AllApps,
            catalog,
            ..FocusPageState::default()
        };
        let page = build_focus_page(&snap, &modules, &state);
        let section = &page.sections[1];
        assert!(
            section
                .rows
                .iter()
                .any(|row| row.action == Some(SettingsAction::ExpandFocusAllApps))
        );
        let toggle_count = section
            .rows
            .iter()
            .filter(|row| matches!(row.value, SettingsRowValue::Toggle(_)))
            .count();
        assert_eq!(toggle_count, FOCUS_ALL_APPS_INITIAL_LIMIT);

        state.app_search = String::from("Tool 17");
        let filtered_page = build_focus_page(&snap, &modules, &state);
        let filtered_labels: Vec<&str> = filtered_page.sections[1]
            .rows
            .iter()
            .filter(|row| matches!(row.value, SettingsRowValue::Toggle(_)))
            .map(|row| row.label.as_str())
            .collect();
        assert_eq!(filtered_labels, vec!["Tool 17"]);
    }

    #[test]
    fn activity_tab_hides_clicks_and_searches_until_details_expanded() {
        let snap = snapshot_with_activity();
        let modules = resolve_modules(&snap, |_| None);
        let mut state = FocusPageState {
            tab: FocusSettingsTab::Activity,
            catalog: sample_catalog(),
            ..FocusPageState::default()
        };
        let collapsed = build_focus_page(&snap, &modules, &state);
        let labels: Vec<&str> = collapsed
            .sections
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.label.as_str()))
            .collect();
        assert!(!labels.contains(&"Clicks & keys today"));
        assert!(!labels.contains(&"otto focus"));
        assert!(!labels.contains(&"Reset Activity & Screen Time Stats"));
        assert!(labels.contains(&"Google Chrome"));
        let chart_row = collapsed
            .sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .find(|row| matches!(row.value, SettingsRowValue::WeeklyChart(_)))
            .expect("weekly chart row");
        let SettingsRowValue::WeeklyChart(chart) = &chart_row.value else {
            panic!("expected weekly chart");
        };
        assert_eq!(chart.days.len(), 7);
        assert_eq!(chart.days[6].day_label, "Sun");
        assert!(chart.days[6].is_today);

        state.details_expanded = true;
        let expanded = build_focus_page(&snap, &modules, &state);
        let expanded_labels: Vec<&str> = expanded
            .sections
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.label.as_str()))
            .collect();
        assert!(expanded_labels.contains(&"Clicks & keys today"));
        assert!(expanded_labels.contains(&"otto focus"));
        assert!(expanded_labels.contains(&"Reset Activity & Screen Time Stats"));
    }

    #[test]
    fn previous_day_crosses_month_and_leap_year_boundaries() {
        assert_eq!(
            previous_day(DayKey {
                year: 2024,
                month: 3,
                day: 1
            }),
            DayKey {
                year: 2024,
                month: 2,
                day: 29
            }
        );
        assert_eq!(
            previous_day(DayKey {
                year: 2026,
                month: 1,
                day: 1
            }),
            DayKey {
                year: 2025,
                month: 12,
                day: 31
            }
        );
    }

    #[test]
    fn merge_focus_apps_unifies_redundant_keys_and_drops_system_or_doc_entries() {
        let mut snap = snapshot_with_activity();
        let day = DayKey {
            year: 2026,
            month: 11,
            day: 22,
        };
        snap.focus_activity.observe_foreground(
            day,
            "aumid:claude_pzs8sxrjxfjjc!claude",
            "Claude",
            None,
        );
        snap.focus_activity
            .record_sample(day, 31_000, 0, false, 1, 5);
        snap.focus_activity
            .observe_foreground(day, "AppleMusic.exe", "AppleMusic", None);
        snap.focus_activity
            .record_sample(day, 41_000, 0, false, 1, 5);
        snap.focus_activity.observe_foreground(
            day,
            "ApplicationFrameHost.exe",
            "ApplicationFrame",
            None,
        );
        snap.focus_activity
            .record_sample(day, 45_000, 0, false, 1, 0);

        let catalog = vec![
            FocusCatalogApp {
                block_key: String::from("claude"),
                display_name: String::from("Claude"),
                detail: String::new(),
                icon_source: String::from(r"shell:AppsFolder\Claude_pzs8sxrjxfjjc!Claude"),
            },
            FocusCatalogApp {
                block_key: String::from("apple music"),
                display_name: String::from("Apple Music"),
                detail: String::new(),
                icon_source: String::from(
                    r"shell:AppsFolder\AppleInc.AppleMusicWin_nzyj5cx40ttqa!App",
                ),
            },
            FocusCatalogApp {
                block_key: String::from("appverif"),
                display_name: String::from("Application Verifier (X64)"),
                detail: String::from(r"C:\Windows\System32\appverif.exe"),
                icon_source: String::from(r"shell:AppsFolder\appverif"),
            },
            FocusCatalogApp {
                block_key: String::from("cmake documentation"),
                display_name: String::from("CMake Documentation"),
                detail: String::new(),
                icon_source: String::from(r"shell:AppsFolder\cmake_doc"),
            },
        ];

        let merged = merge_focus_apps(&snap.focus_activity, &catalog);
        let claude_rows: Vec<_> = merged
            .iter()
            .filter(|app| app.display_name == "Claude")
            .collect();
        assert_eq!(claude_rows.len(), 1);
        assert!(claude_rows[0].today_active_ms > 0);
        assert_eq!(
            claude_rows[0].icon_source.as_deref(),
            Some(r"shell:AppsFolder\Claude_pzs8sxrjxfjjc!Claude")
        );

        let apple_music_rows: Vec<_> = merged
            .iter()
            .filter(|app| app.display_name == "Apple Music" || app.display_name == "AppleMusic")
            .collect();
        assert_eq!(apple_music_rows.len(), 1);
        assert_eq!(apple_music_rows[0].display_name, "Apple Music");
        assert!(apple_music_rows[0].today_active_ms > 0);

        assert!(
            !merged
                .iter()
                .any(|app| app.display_name.contains("Verifier")
                    || app.display_name.contains("ApplicationFrame")
                    || app.display_name.contains("Documentation"))
        );
    }
}
