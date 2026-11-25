use crate::layout::{Rect, scale};
use crate::menu::{CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_TODAY};

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

#[must_use]
pub(crate) const fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[must_use]
pub const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
    }
}

#[must_use]
pub fn weekday_of(year: u16, month: u8, day: u8) -> u8 {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let month_index = usize::from(month.clamp(1, 12)) - 1;
    let adjusted_year = i32::from(year) - i32::from(month < 3);
    let value = adjusted_year + adjusted_year / 4 - adjusted_year / 100
        + adjusted_year / 400
        + OFFSETS[month_index]
        + i32::from(day);
    value.rem_euclid(7) as u8
}

#[must_use]
pub fn shift_month(year: u16, month: u8, delta: i32) -> (u16, u8) {
    let index = i32::from(year) * 12 + i32::from(month) - 1 + delta;
    let clamped = index.clamp(12, 9999 * 12 + 11);
    ((clamped / 12) as u16, (clamped % 12 + 1) as u8)
}

#[must_use]
pub fn month_name(month: u8) -> &'static str {
    MONTH_NAMES[usize::from(month.clamp(1, 12)) - 1]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDay {
    pub day: u8,
    pub in_month: bool,
}

#[must_use]
pub fn month_grid(year: u16, month: u8) -> [CalendarDay; GRID_CELLS] {
    let first = usize::from(weekday_of(year, month, 1));
    let (previous_year, previous_month) = shift_month(year, month, -1);
    let previous_length = usize::from(days_in_month(previous_year, previous_month));
    let length = usize::from(days_in_month(year, month));
    std::array::from_fn(|index| {
        if index < first {
            CalendarDay {
                day: (previous_length - (first - index) + 1) as u8,
                in_month: false,
            }
        } else if index < first + length {
            CalendarDay {
                day: (index - first + 1) as u8,
                in_month: true,
            }
        } else {
            CalendarDay {
                day: (index - first - length + 1) as u8,
                in_month: false,
            }
        }
    })
}

pub(crate) const GRID_COLUMNS: usize = 7;
pub(crate) const GRID_ROWS: usize = 6;
pub(crate) const GRID_CELLS: usize = GRID_COLUMNS * GRID_ROWS;
pub const CALENDAR_POPUP_WIDTH: i32 = 252;

const INSET_X: i32 = 8;
const HEADER_HEIGHT: i32 = 40;
const WEEKDAY_HEIGHT: i32 = 22;
const CELL_HEIGHT: i32 = 30;
const BOTTOM_PAD: i32 = 8;
const CONTROL_SIZE: i32 = 26;
const CONTROL_GAP: i32 = 2;
const TITLE_INSET: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarControl {
    Previous,
    Today,
    Next,
}

impl CalendarControl {
    #[must_use]
    pub const fn command(self) -> usize {
        match self {
            Self::Previous => CMD_CAL_PREVIOUS,
            Self::Today => CMD_CAL_TODAY,
            Self::Next => CMD_CAL_NEXT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarCell {
    pub rect: Rect,
    pub day: CalendarDay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarLayout {
    pub title: Rect,
    pub controls: [(CalendarControl, Rect); 3],
    pub weekdays: [Rect; GRID_COLUMNS],
    pub cells: [CalendarCell; GRID_CELLS],
}

impl CalendarLayout {
    #[must_use]
    pub fn control_at(&self, x: i32, y: i32) -> Option<CalendarControl> {
        self.controls
            .iter()
            .find(|(_, rect)| rect.contains(x, y))
            .map(|(control, _)| *control)
    }
}

#[must_use]
pub fn calendar_row_height(dpi: u32) -> i32 {
    scale(
        HEADER_HEIGHT + WEEKDAY_HEIGHT + CELL_HEIGHT * GRID_ROWS as i32 + BOTTOM_PAD,
        dpi,
    )
}

#[must_use]
pub fn compute_calendar_layout(row: Rect, dpi: u32, year: u16, month: u8) -> CalendarLayout {
    let inset = scale(INSET_X, dpi);
    let header = scale(HEADER_HEIGHT, dpi);
    let weekday = scale(WEEKDAY_HEIGHT, dpi);
    let cell_height = scale(CELL_HEIGHT, dpi);
    let control = scale(CONTROL_SIZE, dpi);
    let gap = scale(CONTROL_GAP, dpi);
    let grid_left = row.x + inset;
    let grid_width = (row.width - inset * 2).max(GRID_COLUMNS as i32);
    let column_edge = |column: usize| grid_left + grid_width * column as i32 / GRID_COLUMNS as i32;
    let column = |column: usize, top: i32, height: i32| Rect {
        x: column_edge(column),
        y: top,
        width: column_edge(column + 1) - column_edge(column),
        height,
    };

    let controls_width = control * 3 + gap * 2;
    let controls_left = row.x + row.width - inset - controls_width;
    let control_top = row.y + (header - control) / 2;
    let control_rect = |index: i32| Rect {
        x: controls_left + index * (control + gap),
        y: control_top,
        width: control,
        height: control,
    };
    let title_left = grid_left + scale(TITLE_INSET, dpi);

    let weekday_top = row.y + header;
    let grid_top = weekday_top + weekday;
    let grid = month_grid(year, month);
    CalendarLayout {
        title: Rect {
            x: title_left,
            y: row.y,
            width: (controls_left - title_left).max(0),
            height: header,
        },
        controls: [
            (CalendarControl::Previous, control_rect(0)),
            (CalendarControl::Today, control_rect(1)),
            (CalendarControl::Next, control_rect(2)),
        ],
        weekdays: std::array::from_fn(|index| column(index, weekday_top, weekday)),
        cells: std::array::from_fn(|index| CalendarCell {
            rect: column(
                index % GRID_COLUMNS,
                grid_top + (index / GRID_COLUMNS) as i32 * cell_height,
                cell_height,
            ),
            day: grid[index],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_matches_known_dates() {
        assert_eq!(weekday_of(2000, 1, 1), 6);
        assert_eq!(weekday_of(2024, 2, 29), 4);
        assert_eq!(weekday_of(2026, 11, 11), 3);
    }

    #[test]
    fn february_length_follows_leap_rules() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2025, 4), 30);
    }

    #[test]
    fn shift_month_crosses_year_boundaries() {
        assert_eq!(shift_month(2026, 12, 1), (2027, 1));
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2026, 6, 0), (2026, 6));
    }

    fn in_month(grid: &[CalendarDay]) -> Vec<u8> {
        grid.iter()
            .filter(|cell| cell.in_month)
            .map(|cell| cell.day)
            .collect()
    }

    #[test]
    fn month_grid_places_the_first_day_on_its_weekday() {
        let grid = month_grid(2026, 11);
        assert_eq!(
            grid[0],
            CalendarDay {
                day: 1,
                in_month: true
            }
        );
        assert_eq!(in_month(&grid), (1..=30).collect::<Vec<u8>>());
        assert_eq!(month_name(11), "November");
    }

    #[test]
    fn month_grid_fills_leading_and_trailing_cells_from_the_neighbouring_months() {
        let grid = month_grid(2026, 4);
        let first = usize::from(weekday_of(2026, 4, 1));
        assert_eq!(first, 3);
        assert_eq!(
            grid[..first]
                .iter()
                .map(|cell| (cell.day, cell.in_month))
                .collect::<Vec<_>>(),
            vec![(29, false), (30, false), (31, false)]
        );
        assert_eq!(
            grid[first + 30],
            CalendarDay {
                day: 1,
                in_month: false
            }
        );
        assert_eq!(grid.len(), 42);
    }

    #[test]
    fn month_grid_crosses_year_boundaries_and_leap_days() {
        let january = month_grid(2026, 1);
        let first = usize::from(weekday_of(2026, 1, 1));
        assert_eq!(first, 4);
        assert_eq!(
            january[..first]
                .iter()
                .map(|cell| cell.day)
                .collect::<Vec<_>>(),
            vec![28, 29, 30, 31]
        );
        let march = month_grid(2024, 3);
        let march_first = usize::from(weekday_of(2024, 3, 1));
        assert_eq!(march[march_first - 1].day, 29);
    }

    fn row() -> Rect {
        Rect {
            x: 5,
            y: 5,
            width: 242,
            height: calendar_row_height(96),
        }
    }

    #[test]
    fn layout_fits_inside_the_row_without_overlap_at_every_dpi() {
        for dpi in [96, 120, 144, 192] {
            let row = Rect {
                x: 5,
                y: 5,
                width: scale(242, dpi),
                height: calendar_row_height(dpi),
            };
            let layout = compute_calendar_layout(row, dpi, 2026, 11);
            for cell in &layout.cells {
                assert!(cell.rect.x >= row.x && cell.rect.right() <= row.right());
                assert!(cell.rect.y >= row.y && cell.rect.bottom() <= row.bottom());
            }
            for pair in layout.cells.windows(2) {
                if pair[0].rect.y == pair[1].rect.y {
                    assert_eq!(pair[0].rect.right(), pair[1].rect.x);
                }
            }
            for (_, control) in &layout.controls {
                assert!(control.width >= scale(24, dpi) && control.height >= scale(24, dpi));
                assert!(control.x >= row.x && control.right() <= row.right());
            }
            assert!(layout.title.right() <= layout.controls[0].1.x);
        }
    }

    #[test]
    fn weekday_labels_share_the_columns_of_the_day_cells() {
        let layout = compute_calendar_layout(row(), 96, 2026, 11);
        for (column, weekday) in layout.weekdays.iter().enumerate() {
            assert_eq!(weekday.x, layout.cells[column].rect.x);
            assert_eq!(weekday.width, layout.cells[column].rect.width);
            assert!(weekday.bottom() <= layout.cells[column].rect.y);
        }
    }

    #[test]
    fn header_controls_are_ordered_hit_testable_and_map_to_commands() {
        let layout = compute_calendar_layout(row(), 96, 2026, 11);
        let [previous, today, next] = layout.controls;
        assert!(previous.1.right() <= today.1.x && today.1.right() <= next.1.x);
        assert!(previous.1.width >= 24 && previous.1.height >= 24);
        let center = |rect: Rect| (rect.x + rect.width / 2, rect.y + rect.height / 2);
        for (control, rect) in layout.controls {
            let (x, y) = center(rect);
            assert_eq!(layout.control_at(x, y), Some(control));
        }
        assert_eq!(layout.control_at(row().x + 2, row().y + 2), None);
        assert_eq!(CalendarControl::Previous.command(), CMD_CAL_PREVIOUS);
        assert_eq!(CalendarControl::Today.command(), CMD_CAL_TODAY);
        assert_eq!(CalendarControl::Next.command(), CMD_CAL_NEXT);
    }
}
