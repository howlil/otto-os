use crate::bar::calendar::{
    CALENDAR_POPUP_WIDTH, CalendarControl, calendar_row_height, compute_calendar_layout,
};
use crate::layout::{Rect, scale};

use super::commands::CMD_NONE;
use super::commands::NOTIFICATION_CARD_HEIGHT;
use super::commands::NOTIFICATION_CARD_WIDTH;
use super::rows::MenuRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAnchor {
    TopBar { x: i32, y: i32 },
    AboveDock { center_x: i32, bottom_y: i32 },
    BelowPoint { x: i32, y: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupMenuMetrics {
    pub min_width: i32,
    pub max_width: i32,
    pub pad_x: i32,
    pub pad_y: i32,
    pub row_height: i32,
    pub separator_height: i32,
    pub corner_radius: i32,
    pub row_corner_radius: i32,
    pub check_col_width: i32,
    pub shortcut_gap: i32,
    pub right_pad: i32,
    pub screen_margin: i32,
    pub max_height: i32,
}

impl PopupMenuMetrics {
    pub const MACOS_SONOMA: Self = Self {
        min_width: 216,
        max_width: 320,
        pad_x: 5,
        pad_y: 5,
        row_height: 24,
        separator_height: 9,
        corner_radius: 13,
        row_corner_radius: 8,
        check_col_width: 20,
        shortcut_gap: 24,
        right_pad: 12,
        screen_margin: 6,
        max_height: 520,
    };
}

impl Default for PopupMenuMetrics {
    fn default() -> Self {
        Self::MACOS_SONOMA
    }
}

pub trait MenuTextMeasure {
    fn label_width(&self, text: &str) -> i32;

    fn shortcut_width(&self, text: &str) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstimatedMenuText {
    pub dpi: u32,
}

impl MenuTextMeasure for EstimatedMenuText {
    fn label_width(&self, text: &str) -> i32 {
        text.chars().count() as i32 * scale(7, self.dpi).max(5)
    }

    fn shortcut_width(&self, text: &str) -> i32 {
        text.chars().count() as i32 * scale(9, self.dpi).max(6)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupMenuRowLayout {
    pub index: usize,
    pub id: usize,
    pub rect: Rect,
    pub is_separator: bool,
    pub is_calendar: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PopupMenuLayout {
    pub dpi: u32,
    pub window: Rect,
    pub viewport: Rect,
    pub content_height: i32,
    pub scroll_offset: i32,
    pub corner_radius: f32,
    pub row_corner_radius: f32,
    pub check_col_width: f32,
    pub shortcut_col_width: f32,
    pub right_pad: f32,
    pub rows: Vec<PopupMenuRowLayout>,
}

impl PopupMenuLayout {
    #[must_use]
    pub const fn max_scroll(&self) -> i32 {
        let overflow = self.content_height - self.viewport.height;
        if overflow > 0 { overflow } else { 0 }
    }

    #[must_use]
    pub const fn is_scrollable(&self) -> bool {
        self.max_scroll() > 0
    }

    #[must_use]
    pub fn scrolled_by(&self, delta: i32) -> i32 {
        (self.scroll_offset + delta).clamp(0, self.max_scroll())
    }

    #[must_use]
    pub fn scroll_revealing(&self, row_index: usize) -> i32 {
        let Some(row) = self.rows.iter().find(|row| row.index == row_index) else {
            return self.scroll_offset;
        };
        let above = self.viewport.y - row.rect.y;
        let below = row.rect.bottom() - self.viewport.bottom();
        if above > 0 {
            self.scrolled_by(-above)
        } else if below > 0 {
            self.scrolled_by(below)
        } else {
            self.scroll_offset
        }
    }

    #[must_use]
    pub fn is_row_reachable(&self, row: &PopupMenuRowLayout) -> bool {
        row.rect.bottom() > self.viewport.y && row.rect.y < self.viewport.bottom()
    }

    #[must_use]
    pub fn hit_test(&self, local_x: i32, local_y: i32) -> Option<usize> {
        if !self.viewport.contains(local_x, local_y) {
            return None;
        }
        self.rows
            .iter()
            .find(|row| !row.is_separator && row.enabled && row.rect.contains(local_x, local_y))
            .map(|row| row.index)
    }

    #[must_use]
    pub fn calendar_control_at(
        &self,
        rows: &[MenuRow],
        local_x: i32,
        local_y: i32,
    ) -> Option<CalendarControl> {
        if !self.viewport.contains(local_x, local_y) {
            return None;
        }
        let row = self
            .rows
            .iter()
            .find(|row| row.is_calendar && row.rect.contains(local_x, local_y))?;
        let MenuRow::Calendar { year, month, .. } = rows.get(row.index)? else {
            return None;
        };
        compute_calendar_layout(row.rect, self.dpi, *year, *month).control_at(local_x, local_y)
    }

    #[must_use]
    pub fn command_at(&self, row_index: usize) -> Option<usize> {
        self.rows
            .iter()
            .find(|row| row.index == row_index && !row.is_separator && row.enabled && row.id != 0)
            .map(|row| row.id)
    }

    pub(super) fn interactive_rows(&self) -> Vec<usize> {
        self.rows
            .iter()
            .filter(|row| !row.is_separator && row.enabled)
            .map(|row| row.index)
            .collect()
    }

    #[must_use]
    pub fn next_interactive_row(&self, current: Option<usize>) -> Option<usize> {
        let interactive = self.interactive_rows();
        let Some(cur) = current else {
            return interactive.first().copied();
        };
        if let Some(pos) = interactive.iter().position(|&idx| idx == cur) {
            interactive.get((pos + 1) % interactive.len()).copied()
        } else {
            interactive
                .iter()
                .copied()
                .find(|&idx| idx > cur)
                .or_else(|| interactive.first().copied())
        }
    }

    #[must_use]
    pub fn prev_interactive_row(&self, current: Option<usize>) -> Option<usize> {
        let interactive = self.interactive_rows();
        let Some(cur) = current else {
            return interactive.last().copied();
        };
        if let Some(pos) = interactive.iter().position(|&idx| idx == cur) {
            interactive
                .get((pos + interactive.len() - 1) % interactive.len())
                .copied()
        } else {
            interactive
                .iter()
                .copied()
                .rfind(|&idx| idx < cur)
                .or_else(|| interactive.last().copied())
        }
    }

    #[must_use]
    pub fn find_row_by_prefix(
        &self,
        rows: &[MenuRow],
        prefix_char: char,
        after: Option<usize>,
    ) -> Option<usize> {
        let wanted = prefix_char.to_lowercase().next()?;
        let matches: Vec<usize> = self
            .rows
            .iter()
            .filter(|row| !row.is_separator && row.enabled)
            .filter(|row| {
                matches!(
                    rows.get(row.index),
                    Some(MenuRow::Item { label, .. })
                        if label.chars().next().and_then(|first| first.to_lowercase().next())
                            == Some(wanted)
                ) || matches!(
                    rows.get(row.index),
                    Some(MenuRow::Notification { app_name, title, .. })
                        if app_name
                            .chars()
                            .chain(title.chars())
                            .next()
                            .and_then(|first| first.to_lowercase().next())
                            == Some(wanted)
                )
            })
            .map(|row| row.index)
            .collect();
        after
            .and_then(|current| matches.iter().copied().find(|&index| index > current))
            .or_else(|| matches.first().copied())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupMenuPlacement {
    pub bounds: Rect,
    pub dpi: u32,
    pub anchor: MenuAnchor,
    pub scroll_offset: i32,
}

pub(super) struct ColumnWidths {
    pub(super) width: i32,
    pub(super) shortcut: i32,
}

pub(super) fn measure_columns(
    metrics: &PopupMenuMetrics,
    dpi: u32,
    rows: &[MenuRow],
    measure: &dyn MenuTextMeasure,
) -> ColumnWidths {
    let min_w = scale(metrics.min_width, dpi);
    let max_w = scale(metrics.max_width, dpi);
    let fixed = scale(metrics.pad_x, dpi) * 2
        + scale(metrics.check_col_width, dpi)
        + scale(metrics.right_pad, dpi);
    let (mut label, mut shortcut) = (0, 0);
    let mut notification_card_width = 0;
    let mut calendar_width = 0;
    for row in rows {
        match row {
            MenuRow::Item {
                label: text,
                shortcut: keys,
                ..
            } => {
                label = label.max(measure.label_width(text));
                if !keys.is_empty() {
                    shortcut = shortcut.max(measure.shortcut_width(keys));
                }
            }
            MenuRow::Notification { .. } => {
                notification_card_width = scale(NOTIFICATION_CARD_WIDTH, dpi);
            }
            MenuRow::Calendar { .. } => {
                calendar_width = scale(CALENDAR_POPUP_WIDTH, dpi);
            }
            MenuRow::Separator => {}
        }
    }
    let gap = if shortcut > 0 {
        scale(metrics.shortcut_gap, dpi)
    } else {
        0
    };
    ColumnWidths {
        width: (fixed + label + gap + shortcut)
            .max(notification_card_width)
            .max(calendar_width)
            .clamp(min_w, max_w.max(min_w)),
        shortcut,
    }
}

#[must_use]
pub fn compute_popup_menu_layout(
    metrics: &PopupMenuMetrics,
    placement: PopupMenuPlacement,
    rows: &[MenuRow],
    measure: &dyn MenuTextMeasure,
) -> PopupMenuLayout {
    let dpi = placement.dpi;
    let bounds = placement.bounds;
    let pad_x = scale(metrics.pad_x, dpi);
    let pad_y = scale(metrics.pad_y, dpi);
    let row_h = scale(metrics.row_height, dpi);
    let margin = scale(metrics.screen_margin, dpi);
    let columns = measure_columns(metrics, dpi, rows, measure);
    let width = columns
        .width
        .min((bounds.width - margin * 2).max(pad_x * 2 + 1));

    let heights: Vec<(usize, i32, bool, bool)> = rows
        .iter()
        .map(|row| match row {
            MenuRow::Item { id, enabled, .. } => (*id, row_h, false, *enabled),
            MenuRow::Calendar { .. } => (CMD_NONE, calendar_row_height(dpi), true, false),
            MenuRow::Notification { id, .. } => {
                (*id, scale(NOTIFICATION_CARD_HEIGHT, dpi), false, true)
            }
            MenuRow::Separator => (CMD_NONE, scale(metrics.separator_height, dpi), true, false),
        })
        .collect();
    let content_height: i32 = heights.iter().map(|entry| entry.1).sum::<i32>().max(row_h);

    let available = (bounds.height - margin * 2)
        .min(scale(metrics.max_height, dpi))
        .saturating_sub(pad_y * 2)
        .max(row_h);
    let viewport_height = content_height.min(available);
    let height = viewport_height + pad_y * 2;
    let scroll_offset = placement
        .scroll_offset
        .clamp(0, (content_height - viewport_height).max(0));

    let mut y = pad_y - scroll_offset;
    let mut row_layouts = Vec::with_capacity(rows.len());
    for (index, (id, row_height, is_separator, enabled)) in heights.into_iter().enumerate() {
        row_layouts.push(PopupMenuRowLayout {
            index,
            id,
            rect: Rect {
                x: pad_x,
                y,
                width: width - pad_x * 2,
                height: row_height,
            },
            is_separator,
            is_calendar: matches!(rows[index], MenuRow::Calendar { .. }),
            enabled,
        });
        y += row_height;
    }

    let (raw_x, raw_y) = match placement.anchor {
        MenuAnchor::TopBar { x, y } | MenuAnchor::BelowPoint { x, y } => (x, y),
        MenuAnchor::AboveDock { center_x, bottom_y } => (center_x - width / 2, bottom_y - height),
    };
    let min_x = bounds.x + margin;
    let min_y = bounds.y + margin;
    let win_x = raw_x.clamp(min_x, (bounds.right() - width - margin).max(min_x));
    let win_y = raw_y.clamp(min_y, (bounds.bottom() - height - margin).max(min_y));

    PopupMenuLayout {
        dpi,
        window: Rect {
            x: win_x,
            y: win_y,
            width,
            height,
        },
        viewport: Rect {
            x: 0,
            y: pad_y,
            width,
            height: viewport_height,
        },
        content_height,
        scroll_offset,
        corner_radius: scale(metrics.corner_radius, dpi) as f32,
        row_corner_radius: scale(metrics.row_corner_radius, dpi) as f32,
        check_col_width: scale(metrics.check_col_width, dpi) as f32,
        shortcut_col_width: columns.shortcut as f32,
        right_pad: scale(metrics.right_pad, dpi) as f32,
        rows: row_layouts,
    }
}
