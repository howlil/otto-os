use crate::layout::{Rect, scale};
use crate::notch::focus_activity::{FocusActivityState, format_screen_time};
use crate::notch::focus_heatmap::{HeatmapFilter, SETTINGS_HEATMAP_WEEKS, build_heatmap_view};
use crate::settings::focus_modes_settings::build_focus_modes_sections;
use crate::settings::focus_settings::{
    FilterChip, FocusPageState, FocusSettingsTab, KpiItem, build_activity_sections,
    focus_header_section,
};
use crate::settings::model::OttoSettingsSnapshot;
use crate::settings_content::{
    SettingsAction, SettingsContent, SettingsRow, SettingsRowValue, SettingsSection,
};
use crate::settings_window::ModuleRow;

pub(crate) const FOCUS_HEATMAP_ROW_HEIGHT_POINTS: i32 = 212;

#[must_use]
pub(crate) fn kpi_parts(rect: Rect, inner_x: i32, inner_w: i32, dpi: u32) -> Vec<Rect> {
    if inner_w <= 0 {
        return Vec::new();
    }
    let gap = scale(10, dpi);
    let y = rect.y + scale(10, dpi);
    let h = (rect.height - scale(20, dpi)).max(0);
    let col_w = (inner_w - gap * 2).max(3) / 3;
    (0..3)
        .map(|index| {
            let x = inner_x + index * (col_w + gap);
            let width = if index == 2 {
                (inner_x + inner_w - x).max(col_w)
            } else {
                col_w
            };
            Rect {
                x,
                y,
                width,
                height: h,
            }
        })
        .collect()
}

#[must_use]
pub(crate) fn focus_heatmap_parts(
    rect: Rect,
    weeks: usize,
    inner_x: i32,
    inner_w: i32,
    dpi: u32,
) -> Vec<Rect> {
    if weeks == 0 || inner_w <= 0 {
        return Vec::new();
    }
    let cols = weeks as i32;
    let rows = 7i32;
    let label_w = scale(26, dpi);
    let plot_x = inner_x + label_w;
    let plot_w = (inner_w - label_w).max(cols);
    let plot_top = rect.y + scale(54, dpi);
    let plot_bottom = rect.bottom() - scale(26, dpi);
    let plot_h = (plot_bottom - plot_top).max(rows * 4);
    let gap = scale(3, dpi).max(2);
    let cell_w = (plot_w - gap * (cols - 1)) / cols;
    let cell_h = (plot_h - gap * (rows - 1)) / rows;
    let cell = cell_w.min(cell_h).max(4);
    let total_w = cols * cell + (cols - 1) * gap;
    let total_h = rows * cell + (rows - 1) * gap;
    let gx = plot_x + (plot_w - total_w).max(0) / 2;
    let gy = plot_top + (plot_h - total_h).max(0) / 2;
    let mut parts = Vec::with_capacity(weeks * 7);
    for col in 0..cols {
        for row in 0..rows {
            parts.push(Rect {
                x: gx + col * (cell + gap),
                y: gy + row * (cell + gap),
                width: cell,
                height: cell,
            });
        }
    }
    parts
}

#[must_use]
pub(crate) fn weekly_chart_parts(
    rect: Rect,
    count: usize,
    inner_x: i32,
    inner_w: i32,
    dpi: u32,
) -> Vec<Rect> {
    if count == 0 || inner_w <= 0 {
        return Vec::new();
    }
    let plot_top = rect.y + scale(58, dpi);
    let plot_bottom = rect.bottom() - scale(10, dpi);
    let plot_h = (plot_bottom - plot_top).max(scale(48, dpi));
    let gap = scale(6, dpi);
    let usable = (inner_w - gap * (count as i32 - 1)).max(count as i32);
    let col_w = usable / count as i32;
    (0..count)
        .map(|index| {
            let x = inner_x + index as i32 * (col_w + gap);
            let width = if index + 1 == count {
                (inner_x + inner_w - x).max(col_w)
            } else {
                col_w
            };
            Rect {
                x,
                y: plot_top,
                width,
                height: plot_h,
            }
        })
        .collect()
}

#[must_use]
pub(crate) fn build_focus_insights_sections(
    activity: &FocusActivityState,
    state: &FocusPageState,
) -> Vec<SettingsSection> {
    let mut filter_chips = vec![FilterChip {
        label: String::from("All"),
        action: SettingsAction::SetFocusHeatmapFilter(None),
    }];
    let mut selected_chip = 0usize;
    for (idx, cat) in activity.categories.categories.iter().enumerate() {
        if state.heatmap_category_id.as_deref() == Some(cat.id.as_str()) {
            selected_chip = idx + 1;
        }
        filter_chips.push(FilterChip {
            label: cat.name.clone(),
            action: SettingsAction::SetFocusHeatmapFilter(Some(cat.id.clone())),
        });
    }

    let filter = match &state.heatmap_category_id {
        Some(id) => HeatmapFilter::Category(id.clone()),
        None => HeatmapFilter::All,
    };
    let heatmap = build_heatmap_view(
        &activity.heatmap,
        &activity.categories.categories,
        &filter,
        activity.today.day,
        SETTINGS_HEATMAP_WEEKS,
        state.selected_heatmap_cell,
    );

    let kpi_row = SettingsRow::new(
        "",
        "",
        SettingsRowValue::KpiStrip([
            KpiItem {
                label: "Total focus (26w)",
                value: format_screen_time(heatmap.total_focus_ms),
                detail: format!("{} sessions", heatmap.completed_sessions),
            },
            KpiItem {
                label: "Current streak",
                value: format!("{}d", heatmap.current_streak),
                detail: String::from("Consecutive days"),
            },
            KpiItem {
                label: "Best streak",
                value: format!("{}d", heatmap.best_streak),
                detail: String::from("Last 26 weeks"),
            },
        ]),
    );

    let heatmap_row = SettingsRow::new(
        "Contribution heatmap",
        heatmap.readout.clone(),
        SettingsRowValue::FocusHeatmap(heatmap.clone()),
    );

    let share_rows: Vec<SettingsRow> = heatmap
        .shares
        .iter()
        .map(|share| {
            SettingsRow::new(
                share.name.clone(),
                format!("{}% of total focus", share.percent),
                SettingsRowValue::UsageBar {
                    duration: format_screen_time(share.focus_ms),
                    percent: share.percent,
                    blocked: false,
                },
            )
            .with_action(SettingsAction::SetFocusHeatmapFilter(Some(
                share.category_id.clone(),
            )))
        })
        .collect();

    let mut sections = vec![
        SettingsSection {
            title: "Focus consistency",
            footer: None,
            rows: vec![
                SettingsRow::new(
                    "",
                    "",
                    SettingsRowValue::FilterChips {
                        chips: filter_chips,
                        selected: selected_chip,
                    },
                ),
                kpi_row,
                heatmap_row,
            ],
        },
        SettingsSection {
            title: "Where your focus went",
            footer: Some("Click any category to filter the heatmap above."),
            rows: share_rows,
        },
    ];
    sections.extend(build_activity_sections(activity, state));
    sections
}

#[must_use]
pub(crate) fn build_focus_page(
    snapshot: &OttoSettingsSnapshot,
    modules: &[ModuleRow],
    state: &FocusPageState,
) -> SettingsContent {
    let mut sections = vec![focus_header_section(snapshot, modules, state)];
    match state.tab {
        FocusSettingsTab::Modes => {
            sections.extend(build_focus_modes_sections(snapshot, state));
        }
        FocusSettingsTab::Insights => {
            sections.extend(build_focus_insights_sections(
                &snapshot.focus_activity,
                state,
            ));
        }
    }
    SettingsContent {
        hero: None,
        sections,
    }
}
