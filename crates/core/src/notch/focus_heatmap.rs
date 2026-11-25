use std::collections::HashMap;

use crate::notch::focus_activity::{DayKey, format_screen_time};
use crate::notch::focus_mode::{CategoryColor, FocusCategory};
use crate::settings::focus_settings::{previous_day, weekday_short};

pub const HEATMAP_MAX_DAYS: usize = 365;
pub const NOTCH_HEATMAP_WEEKS: usize = 16;
pub const SETTINGS_HEATMAP_WEEKS: usize = 26;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyCategoryFocus {
    pub day: DayKey,
    pub category_id: String,
    pub focus_ms: u64,
    pub sessions: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusHeatmapLedger {
    pub entries: Vec<DailyCategoryFocus>,
}

impl FocusHeatmapLedger {
    pub fn record_focus(&mut self, day: DayKey, category_id: &str, delta_ms: u64) {
        if !day.is_valid() || category_id.is_empty() || delta_ms == 0 {
            return;
        }
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.day == day && e.category_id == category_id)
        {
            entry.focus_ms = entry.focus_ms.saturating_add(delta_ms);
            return;
        }
        self.entries.push(DailyCategoryFocus {
            day,
            category_id: category_id.to_owned(),
            focus_ms: delta_ms,
            sessions: 0,
        });
        self.prune(day);
    }

    pub fn record_completed_session(&mut self, day: DayKey, category_id: &str) {
        if !day.is_valid() || category_id.is_empty() {
            return;
        }
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.day == day && e.category_id == category_id)
        {
            entry.sessions = entry.sessions.saturating_add(1);
            return;
        }
        self.entries.push(DailyCategoryFocus {
            day,
            category_id: category_id.to_owned(),
            focus_ms: 0,
            sessions: 1,
        });
        self.prune(day);
    }

    pub fn prune(&mut self, today: DayKey) {
        if !today.is_valid() {
            return;
        }
        let mut cutoff = today;
        for _ in 0..HEATMAP_MAX_DAYS {
            cutoff = previous_day(cutoff);
        }
        self.entries.retain(|entry| entry.day >= cutoff);
    }

    pub fn encode_lines(&self, lines: &mut Vec<String>) {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| {
            b.day
                .cmp(&a.day)
                .then_with(|| a.category_id.cmp(&b.category_id))
        });
        for entry in sorted {
            if entry.day.is_valid() && (entry.focus_ms > 0 || entry.sessions > 0) {
                lines.push(format!(
                    "HEAT\t{}\t{}\t{}\t{}",
                    entry.day.encode(),
                    entry.category_id,
                    entry.focus_ms,
                    entry.sessions,
                ));
            }
        }
    }

    pub fn decode_line(&mut self, parts: &[&str]) -> bool {
        if parts.first().copied() != Some("HEAT") || parts.len() < 5 {
            return false;
        }
        let Some(day) = DayKey::decode(parts[1]) else {
            return true;
        };
        let category_id = parts[2].trim();
        if category_id.is_empty() {
            return true;
        }
        let focus_ms = parts[3].parse().unwrap_or(0);
        let sessions = parts[4].parse().unwrap_or(0);
        if focus_ms == 0 && sessions == 0 {
            return true;
        }
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.day == day && e.category_id == category_id)
        {
            existing.focus_ms = existing.focus_ms.saturating_add(focus_ms);
            existing.sessions = existing.sessions.saturating_add(sessions);
        } else {
            self.entries.push(DailyCategoryFocus {
                day,
                category_id: category_id.to_owned(),
                focus_ms,
                sessions,
            });
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HeatmapFilter {
    #[default]
    All,
    Category(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatmapCell {
    pub day: DayKey,
    pub focus_ms: u64,
    pub sessions: u16,
    pub level: u8,
    pub is_today: bool,
    pub top_category: Option<(String, u64)>,
    pub second_category: Option<(String, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryShareItem {
    pub category_id: String,
    pub name: String,
    pub color: CategoryColor,
    pub focus_ms: u64,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusHeatmapView {
    pub weeks: usize,
    pub filter: HeatmapFilter,
    pub accent: Option<CategoryColor>,
    pub cells: Vec<HeatmapCell>,
    pub total_focus_ms: u64,
    pub current_streak: u16,
    pub best_streak: u16,
    pub completed_sessions: u32,
    pub selected_cell: Option<usize>,
    pub readout: String,
    pub shares: Vec<CategoryShareItem>,
}

impl Default for FocusHeatmapView {
    fn default() -> Self {
        build_heatmap_view(
            &FocusHeatmapLedger::default(),
            &[],
            &HeatmapFilter::All,
            DayKey {
                year: 2026,
                month: 1,
                day: 1,
            },
            NOTCH_HEATMAP_WEEKS,
            None,
        )
    }
}

type RawHeatmapDay = (
    DayKey,
    u64,
    u16,
    bool,
    Option<(String, u64)>,
    Option<(String, u64)>,
);

fn collect_raw_heatmap_days(
    days: Vec<DayKey>,
    by_day_all: &HashMap<DayKey, Vec<(String, u64, u16)>>,
    categories: &[FocusCategory],
    filter: &HeatmapFilter,
) -> (Vec<RawHeatmapDay>, u64, u64, u32) {
    let total_days = days.len();
    let mut raw_cells = Vec::with_capacity(total_days);
    let mut peak_ms: u64 = 0;
    let mut total_focus_ms: u64 = 0;
    let mut completed_sessions: u32 = 0;

    for (idx, day) in days.into_iter().enumerate() {
        let mut focus_ms: u64 = 0;
        let mut sessions: u16 = 0;
        let mut breakdown: Vec<(String, u64)> = Vec::new();

        if let Some(items) = by_day_all.get(&day) {
            for (cat_id, ms, sess) in items {
                let matches = match filter {
                    HeatmapFilter::All => true,
                    HeatmapFilter::Category(target) => cat_id == target,
                };
                if matches {
                    focus_ms = focus_ms.saturating_add(*ms);
                    sessions = sessions.saturating_add(*sess);
                    if *ms > 0 {
                        let name = categories
                            .iter()
                            .find(|c| c.id == *cat_id)
                            .map_or_else(|| cat_id.clone(), |c| c.name.clone());
                        breakdown.push((name, *ms));
                    }
                }
            }
        }
        breakdown.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        peak_ms = peak_ms.max(focus_ms);
        total_focus_ms = total_focus_ms.saturating_add(focus_ms);
        completed_sessions = completed_sessions.saturating_add(u32::from(sessions));
        raw_cells.push((
            day,
            focus_ms,
            sessions,
            idx + 1 == total_days,
            breakdown.first().cloned(),
            breakdown.get(1).cloned(),
        ));
    }
    (raw_cells, peak_ms, total_focus_ms, completed_sessions)
}

#[must_use]
pub fn build_heatmap_view(
    ledger: &FocusHeatmapLedger,
    categories: &[FocusCategory],
    filter: &HeatmapFilter,
    today: DayKey,
    weeks: usize,
    selected_cell: Option<usize>,
) -> FocusHeatmapView {
    let weeks = weeks.max(1);
    let total_days = weeks * 7;
    let anchor = if today.is_valid() {
        today
    } else {
        DayKey {
            year: 2026,
            month: 1,
            day: 1,
        }
    };

    let mut days = Vec::with_capacity(total_days);
    let mut cursor = anchor;
    for _ in 0..total_days {
        days.push(cursor);
        cursor = previous_day(cursor);
    }
    days.reverse();

    let mut by_day_all: HashMap<DayKey, Vec<(String, u64, u16)>> = HashMap::new();
    let mut category_totals: HashMap<String, u64> = HashMap::new();
    let first_day = days[0];

    for entry in &ledger.entries {
        if entry.day < first_day || entry.day > anchor {
            continue;
        }
        by_day_all.entry(entry.day).or_default().push((
            entry.category_id.clone(),
            entry.focus_ms,
            entry.sessions,
        ));
        *category_totals
            .entry(entry.category_id.clone())
            .or_insert(0) += entry.focus_ms;
    }

    let (raw_cells, peak_ms, total_focus_ms, completed_sessions) =
        collect_raw_heatmap_days(days, &by_day_all, categories, filter);

    let scale_peak = peak_ms.max(60 * 60 * 1_000);
    let mut cells = Vec::with_capacity(total_days);
    let mut best_streak: u16 = 0;
    let mut running_streak: u16 = 0;
    for (day, focus_ms, sessions, is_today, top_category, second_category) in raw_cells {
        if focus_ms > 0 {
            running_streak = running_streak.saturating_add(1);
            best_streak = best_streak.max(running_streak);
        } else {
            running_streak = 0;
        }
        let level = intensity_level(focus_ms, scale_peak);
        cells.push(HeatmapCell {
            day,
            focus_ms,
            sessions,
            level,
            is_today,
            top_category,
            second_category,
        });
    }

    let mut current_streak: u16 = 0;
    for cell in cells.iter().rev() {
        if cell.focus_ms > 0 {
            current_streak = current_streak.saturating_add(1);
        } else if !cell.is_today {
            break;
        }
    }

    let accent = match filter {
        HeatmapFilter::All => None,
        HeatmapFilter::Category(id) => categories.iter().find(|c| c.id == *id).map(|c| c.color),
    };

    let shares = build_category_shares(&category_totals, categories);
    let clamped_selected = selected_cell.filter(|idx| *idx < cells.len());
    let readout = format_heatmap_readout(
        &cells,
        categories,
        filter,
        clamped_selected,
        total_focus_ms,
        current_streak,
        weeks,
    );

    FocusHeatmapView {
        weeks,
        filter: filter.clone(),
        accent,
        cells,
        total_focus_ms,
        current_streak,
        best_streak,
        completed_sessions,
        selected_cell: clamped_selected,
        readout,
        shares,
    }
}

fn intensity_level(focus_ms: u64, peak_ms: u64) -> u8 {
    if focus_ms == 0 {
        return 0;
    }
    let ratio = focus_ms.saturating_mul(100) / peak_ms.max(1);
    if ratio <= 25 {
        1
    } else if ratio <= 50 {
        2
    } else if ratio <= 75 {
        3
    } else {
        4
    }
}

fn build_category_shares(
    totals: &HashMap<String, u64>,
    categories: &[FocusCategory],
) -> Vec<CategoryShareItem> {
    let grand: u64 = totals.values().copied().sum();
    let mut items: Vec<CategoryShareItem> = categories
        .iter()
        .map(|cat| {
            let ms = totals.get(&cat.id).copied().unwrap_or(0);
            let pct = if grand > 0 && ms > 0 {
                u8::try_from(((ms.saturating_mul(100) + grand / 2) / grand).clamp(1, 100))
                    .unwrap_or(100)
            } else {
                0
            };
            CategoryShareItem {
                category_id: cat.id.clone(),
                name: cat.name.clone(),
                color: cat.color,
                focus_ms: ms,
                percent: pct,
            }
        })
        .collect();
    items.sort_by(|a, b| {
        b.focus_ms
            .cmp(&a.focus_ms)
            .then_with(|| a.name.cmp(&b.name))
    });
    items
}

fn format_heatmap_readout(
    cells: &[HeatmapCell],
    categories: &[FocusCategory],
    filter: &HeatmapFilter,
    selected: Option<usize>,
    total_focus_ms: u64,
    current_streak: u16,
    weeks: usize,
) -> String {
    if let Some(cell) = selected.and_then(|idx| cells.get(idx)) {
        let date_label = format!(
            "{} {:02}/{:02}",
            weekday_short(cell.day),
            cell.day.day,
            cell.day.month
        );
        if cell.focus_ms == 0 {
            return format!("{date_label} · No focus recorded");
        }
        return match (&cell.top_category, &cell.second_category) {
            (Some((first, first_ms)), Some((second, second_ms)))
                if matches!(filter, HeatmapFilter::All) =>
            {
                format!(
                    "{date_label} · {} ({} {}, {} {})",
                    format_screen_time(cell.focus_ms),
                    first,
                    format_screen_time(*first_ms),
                    second,
                    format_screen_time(*second_ms)
                )
            }
            (Some((first, _)), _) => {
                format!(
                    "{date_label} · {} ({first})",
                    format_screen_time(cell.focus_ms)
                )
            }
            _ => format!("{date_label} · {}", format_screen_time(cell.focus_ms)),
        };
    }
    if total_focus_ms == 0 {
        return String::from("Start a Focus session to build your heatmap");
    }
    match filter {
        HeatmapFilter::All => format!(
            "{current_streak}d streak · {} in {weeks}w",
            format_screen_time(total_focus_ms)
        ),
        HeatmapFilter::Category(id) => {
            let name = categories
                .iter()
                .find(|c| c.id == *id)
                .map_or(id.as_str(), |c| c.name.as_str());
            format!(
                "{name} · {current_streak}d streak · {}",
                format_screen_time(total_focus_ms)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heatmap_tracks_per_category_and_combined_totals_and_streaks() {
        let today = DayKey {
            year: 2026,
            month: 11,
            day: 25,
        };
        let yesterday = previous_day(today);
        let categories = vec![
            FocusCategory::coding_default(),
            FocusCategory::study_default(),
        ];
        let mut ledger = FocusHeatmapLedger::default();
        ledger.record_focus(yesterday, "coding", 45 * 60 * 1_000);
        ledger.record_focus(today, "coding", 60 * 60 * 1_000);
        ledger.record_focus(today, "study", 30 * 60 * 1_000);
        ledger.record_completed_session(today, "coding");

        let all = build_heatmap_view(
            &ledger,
            &categories,
            &HeatmapFilter::All,
            today,
            NOTCH_HEATMAP_WEEKS,
            None,
        );
        assert_eq!(all.cells.len(), NOTCH_HEATMAP_WEEKS * 7);
        assert_eq!(all.current_streak, 2);
        assert_eq!(all.best_streak, 2);
        assert_eq!(all.completed_sessions, 1);
        assert_eq!(all.total_focus_ms, 135 * 60 * 1_000);

        let study = build_heatmap_view(
            &ledger,
            &categories,
            &HeatmapFilter::Category(String::from("study")),
            today,
            NOTCH_HEATMAP_WEEKS,
            None,
        );
        assert_eq!(study.total_focus_ms, 30 * 60 * 1_000);
        assert_eq!(study.current_streak, 1);
        assert_eq!(study.accent, Some(CategoryColor::Blue));
    }
}
