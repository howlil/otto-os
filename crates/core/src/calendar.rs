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
pub const fn is_leap_year(year: u16) -> bool {
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
pub fn month_title(year: u16, month: u8) -> String {
    let name = MONTH_NAMES[usize::from(month.clamp(1, 12)) - 1];
    format!("{name} {year}")
}

#[must_use]
pub fn month_cells(year: u16, month: u8) -> [Option<u8>; 42] {
    let mut cells = [None; 42];
    let first = usize::from(weekday_of(year, month, 1));
    for day in 1..=days_in_month(year, month) {
        cells[first + usize::from(day) - 1] = Some(day);
    }
    cells
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

    #[test]
    fn month_cells_place_first_day_on_its_weekday() {
        let cells = month_cells(2026, 11);
        assert_eq!(cells[0], Some(1));
        assert_eq!(cells.iter().flatten().count(), 30);
        assert_eq!(cells[29], Some(30));
        assert_eq!(month_title(2026, 11), "November 2026");
    }
}
