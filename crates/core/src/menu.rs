use crate::layout::{HitTarget, Rect, scale};
use crate::theme::ThemeMode;

pub const CMD_NONE: usize = 0;
pub const CMD_QUIT: usize = 1;
pub const CMD_START: usize = 2;
pub const CMD_SETTINGS: usize = 3;
pub const CMD_TASKMGR: usize = 4;
pub const CMD_LOCK: usize = 5;
pub const CMD_SLEEP: usize = 6;
pub const CMD_RESTART: usize = 7;
pub const CMD_SHUTDOWN: usize = 8;

pub const CMD_APP_ACTIVATE: usize = 10;
pub const CMD_WIN_MINIMIZE: usize = 11;
pub const CMD_WIN_MAXIMIZE: usize = 12;
pub const CMD_WIN_CLOSE: usize = 13;
pub const CMD_SHOW_DESKTOP: usize = 14;
pub const CMD_TILE_LEFT: usize = 15;
pub const CMD_TILE_RIGHT: usize = 16;
pub const CMD_TASK_VIEW: usize = 17;

pub const CMD_FILE_NEW: usize = 20;
pub const CMD_FILE_NEW_TAB: usize = 21;
pub const CMD_FILE_OPEN: usize = 22;
pub const CMD_FILE_SAVE: usize = 23;
pub const CMD_FILE_SAVE_AS: usize = 24;

pub const CMD_EDIT_UNDO: usize = 30;
pub const CMD_EDIT_REDO: usize = 31;
pub const CMD_EDIT_CUT: usize = 32;
pub const CMD_EDIT_COPY: usize = 33;
pub const CMD_EDIT_PASTE: usize = 34;
pub const CMD_EDIT_SELECT_ALL: usize = 35;
pub const CMD_EDIT_FIND: usize = 36;

pub const CMD_VIEW_FULLSCREEN: usize = 40;
pub const CMD_VIEW_ZOOM_IN: usize = 41;
pub const CMD_VIEW_ZOOM_OUT: usize = 42;
pub const CMD_VIEW_ACTUAL_SIZE: usize = 43;

pub const CMD_HELP_PALETTE: usize = 50;
pub const CMD_HELP_F1: usize = 51;

pub const CMD_CC_VOLUME: usize = 60;
pub const CMD_CC_BRIGHTNESS: usize = 61;
pub const CMD_CC_MEDIA_TOGGLE: usize = 62;
pub const CMD_CC_MEDIA_NEXT: usize = 63;
pub const CMD_CC_MEDIA_PREVIOUS: usize = 64;
pub const CMD_CC_WIFI: usize = 65;
pub const CMD_CC_BLUETOOTH: usize = 66;
pub const CMD_CC_FOCUS: usize = 67;
pub const CMD_CC_DISPLAY: usize = 68;
pub const CMD_CC_SOUND: usize = 69;
pub const CMD_CC_CALENDAR: usize = 70;
pub const CMD_CAL_PREVIOUS: usize = 71;
pub const CMD_CAL_NEXT: usize = 72;
pub const CMD_CAL_TODAY: usize = 73;
pub const CMD_CAL_SETTINGS: usize = 74;
pub const CMD_CC_THEME_TOGGLE: usize = 75;
pub const CMD_CC_THEME_LIGHT: usize = 76;
pub const CMD_CC_THEME_DARK: usize = 77;
pub const CMD_CC_THEME_AUTO: usize = 78;
pub const CMD_CC_LYRICS_ONLINE: usize = 79;
pub const CMD_DOCK_PIN: usize = 100;
pub const CMD_DOCK_TOGGLE: usize = 101;
pub const CMD_DOCK_NEW_WINDOW: usize = 102;
pub const CMD_DOCK_EMPTY_TRASH: usize = 103;
pub const CMD_DOCK_QUIT: usize = 104;
pub const CMD_DOCK_WINDOW_BASE: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuRow {
    Item {
        id: usize,
        label: String,
        shortcut: String,
        checked: bool,
        enabled: bool,
    },
    Slider {
        id: usize,
        label: String,
        value: u8,
    },
    Calendar {
        year: u16,
        month: u8,
        today: Option<u8>,
    },
    Separator,
}

impl MenuRow {
    #[must_use]
    pub fn item(id: usize, label: impl Into<String>, shortcut: impl Into<String>) -> Self {
        Self::Item {
            id,
            label: label.into(),
            shortcut: shortcut.into(),
            checked: false,
            enabled: true,
        }
    }

    #[must_use]
    pub fn check_item(id: usize, label: impl Into<String>, checked: bool, enabled: bool) -> Self {
        Self::Item {
            id,
            label: label.into(),
            shortcut: String::new(),
            checked,
            enabled,
        }
    }
}

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
    pub slider_height: i32,
    pub slider_inset: i32,
    pub calendar_cell_height: i32,
}

impl PopupMenuMetrics {
    pub const MACOS_SONOMA: Self = Self {
        min_width: 216,
        max_width: 320,
        pad_x: 5,
        pad_y: 5,
        row_height: 24,
        separator_height: 9,
        corner_radius: 10,
        row_corner_radius: 5,
        check_col_width: 20,
        shortcut_gap: 24,
        right_pad: 12,
        slider_height: 46,
        slider_inset: 14,
        calendar_cell_height: 26,
    };
}

impl Default for PopupMenuMetrics {
    fn default() -> Self {
        Self::MACOS_SONOMA
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupMenuRowLayout {
    pub index: usize,
    pub id: usize,
    pub rect: Rect,
    pub is_separator: bool,
    pub is_slider: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PopupMenuLayout {
    pub window: Rect,
    pub corner_radius: f32,
    pub row_corner_radius: f32,
    pub check_col_width: f32,
    pub right_pad: f32,
    pub slider_inset: i32,
    pub rows: Vec<PopupMenuRowLayout>,
}

impl PopupMenuLayout {
    #[must_use]
    pub fn hit_test(&self, local_x: i32, local_y: i32) -> Option<usize> {
        self.rows
            .iter()
            .find(|row| !row.is_separator && row.enabled && row.rect.contains(local_x, local_y))
            .map(|row| row.index)
    }

    #[must_use]
    pub fn slider_value_at(&self, row_index: usize, local_x: i32) -> Option<u8> {
        let row = self
            .rows
            .iter()
            .find(|row| row.index == row_index && row.is_slider)?;
        let left = row.rect.x + self.slider_inset;
        let span = (row.rect.width - self.slider_inset * 2).max(1);
        let ratio = (local_x - left) as f32 / span as f32;
        Some((ratio.clamp(0.0, 1.0) * 100.0).round() as u8)
    }

    #[must_use]
    pub fn command_at(&self, row_index: usize) -> Option<usize> {
        self.rows
            .iter()
            .find(|row| row.index == row_index && !row.is_separator && row.enabled && row.id != 0)
            .map(|row| row.id)
    }
}

#[must_use]
pub fn compute_popup_menu_layout(
    metrics: &PopupMenuMetrics,
    screen_width: i32,
    screen_height: i32,
    dpi: u32,
    anchor: MenuAnchor,
    rows: &[MenuRow],
) -> PopupMenuLayout {
    let min_w = scale(metrics.min_width, dpi);
    let max_w = scale(metrics.max_width, dpi);
    let pad_x = scale(metrics.pad_x, dpi);
    let pad_y = scale(metrics.pad_y, dpi);
    let row_h = scale(metrics.row_height, dpi);
    let sep_h = scale(metrics.separator_height, dpi);
    let check_w = scale(metrics.check_col_width, dpi);
    let shortcut_gap = scale(metrics.shortcut_gap, dpi);
    let right_pad = scale(metrics.right_pad, dpi);
    let char_w = scale(7, dpi).max(5);
    let shortcut_char_w = scale(9, dpi).max(6);

    let mut content_w = min_w;
    for row in rows {
        if let MenuRow::Item {
            label, shortcut, ..
        } = row
        {
            let label_px = label.chars().count() as i32 * char_w;
            let short_px = if shortcut.is_empty() {
                0
            } else {
                shortcut_gap + shortcut.chars().count() as i32 * shortcut_char_w
            };
            let candidate = pad_x * 2 + check_w + label_px + short_px + right_pad;
            content_w = content_w.max(candidate);
        }
    }
    let width = content_w.clamp(min_w, max_w);

    let mut y = pad_y;
    let mut row_layouts = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let (id, height, is_separator, is_slider, enabled) = match row {
            MenuRow::Item { id, enabled, .. } => (*id, row_h, false, false, *enabled),
            MenuRow::Slider { id, .. } => {
                (*id, scale(metrics.slider_height, dpi), false, true, true)
            }
            MenuRow::Calendar { .. } => (
                CMD_NONE,
                scale(metrics.calendar_cell_height, dpi) * 8,
                true,
                false,
                false,
            ),
            MenuRow::Separator => (CMD_NONE, sep_h, true, false, false),
        };
        row_layouts.push(PopupMenuRowLayout {
            index,
            id,
            rect: Rect {
                x: pad_x,
                y,
                width: width - pad_x * 2,
                height,
            },
            is_separator,
            is_slider,
            enabled,
        });
        y += height;
    }
    let height = (y + pad_y).max(row_h + pad_y * 2);

    let margin = scale(6, dpi);
    let (raw_x, raw_y) = match anchor {
        MenuAnchor::TopBar { x, y } | MenuAnchor::BelowPoint { x, y } => (x, y),
        MenuAnchor::AboveDock { center_x, bottom_y } => (center_x - width / 2, bottom_y - height),
    };
    let win_x = raw_x.clamp(margin, (screen_width - width - margin).max(margin));
    let win_y = raw_y.clamp(margin, (screen_height - height - margin).max(margin));

    PopupMenuLayout {
        window: Rect {
            x: win_x,
            y: win_y,
            width,
            height,
        },
        corner_radius: scale(metrics.corner_radius, dpi) as f32,
        row_corner_radius: scale(metrics.row_corner_radius, dpi) as f32,
        check_col_width: check_w as f32,
        right_pad: right_pad as f32,
        slider_inset: scale(metrics.slider_inset, dpi),
        rows: row_layouts,
    }
}

#[must_use]
pub fn is_top_bar_menu_target(target: HitTarget) -> bool {
    matches!(
        target,
        HitTarget::Start
            | HitTarget::AppMenu
            | HitTarget::FileMenu
            | HitTarget::EditMenu
            | HitTarget::ViewMenu
            | HitTarget::WindowMenu
            | HitTarget::HelpMenu
    )
}

#[must_use]
pub fn build_apple_menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::item(CMD_START, "Open Launchpad / Start", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_SETTINGS, "System Settings...", "⌘,"),
        MenuRow::item(CMD_TASKMGR, "Force Quit Applications...", "⌥⌘⎋"),
        MenuRow::Separator,
        MenuRow::item(CMD_CC_THEME_TOGGLE, "Toggle Theme (Light / Dark)", "⌘T"),
        MenuRow::Separator,
        MenuRow::item(CMD_SLEEP, "Sleep", ""),
        MenuRow::item(CMD_RESTART, "Restart...", ""),
        MenuRow::item(CMD_SHUTDOWN, "Shut Down...", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_LOCK, "Lock Screen", "⌃⌘Q"),
        MenuRow::item(CMD_QUIT, "Quit Otto Bar", "⌘Q"),
    ]
}

#[must_use]
pub fn build_top_bar_menu_rows(target: HitTarget, app_name: &str) -> Vec<MenuRow> {
    match target {
        HitTarget::Start => build_apple_menu_rows(),
        HitTarget::AppMenu => vec![
            MenuRow::item(CMD_APP_ACTIVATE, format!("Show {app_name}"), ""),
            MenuRow::Separator,
            MenuRow::item(CMD_SETTINGS, "Settings...", "⌘,"),
            MenuRow::Separator,
            MenuRow::item(CMD_WIN_MINIMIZE, format!("Hide {app_name}"), "⌘H"),
            MenuRow::item(CMD_SHOW_DESKTOP, "Hide Others", "⌥⌘H"),
            MenuRow::Separator,
            MenuRow::item(CMD_TASKMGR, "Force Quit...", "⌥⌘⎋"),
            MenuRow::item(CMD_WIN_CLOSE, format!("Quit {app_name}"), "⌘Q"),
        ],
        HitTarget::FileMenu => vec![
            MenuRow::item(CMD_FILE_NEW, "New Window", "⌘N"),
            MenuRow::item(CMD_FILE_NEW_TAB, "New Tab", "⌘T"),
            MenuRow::item(CMD_FILE_OPEN, "Open...", "⌘O"),
            MenuRow::Separator,
            MenuRow::item(CMD_FILE_SAVE, "Save", "⌘S"),
            MenuRow::item(CMD_FILE_SAVE_AS, "Save As...", "⇧⌘S"),
            MenuRow::Separator,
            MenuRow::item(CMD_WIN_CLOSE, "Close Window", "⌘W"),
        ],
        HitTarget::EditMenu => vec![
            MenuRow::item(CMD_EDIT_UNDO, "Undo", "⌘Z"),
            MenuRow::item(CMD_EDIT_REDO, "Redo", "⇧⌘Z"),
            MenuRow::Separator,
            MenuRow::item(CMD_EDIT_CUT, "Cut", "⌘X"),
            MenuRow::item(CMD_EDIT_COPY, "Copy", "⌘C"),
            MenuRow::item(CMD_EDIT_PASTE, "Paste", "⌘V"),
            MenuRow::item(CMD_EDIT_SELECT_ALL, "Select All", "⌘A"),
            MenuRow::Separator,
            MenuRow::item(CMD_EDIT_FIND, "Find...", "⌘F"),
        ],
        HitTarget::ViewMenu => vec![
            MenuRow::item(CMD_VIEW_FULLSCREEN, "Enter Full Screen", "⌃⌘F"),
            MenuRow::Separator,
            MenuRow::item(CMD_VIEW_ZOOM_IN, "Zoom In", "⌘+"),
            MenuRow::item(CMD_VIEW_ZOOM_OUT, "Zoom Out", "⌘-"),
            MenuRow::item(CMD_VIEW_ACTUAL_SIZE, "Actual Size", "⌘0"),
        ],
        HitTarget::WindowMenu => vec![
            MenuRow::item(CMD_WIN_MINIMIZE, "Minimize", "⌘M"),
            MenuRow::item(CMD_WIN_MAXIMIZE, "Zoom", "⌃⌘F"),
            MenuRow::Separator,
            MenuRow::item(CMD_TILE_LEFT, "Tile Window to Left of Screen", "⌃⌥←"),
            MenuRow::item(CMD_TILE_RIGHT, "Tile Window to Right of Screen", "⌃⌥→"),
            MenuRow::Separator,
            MenuRow::item(CMD_TASK_VIEW, "Mission Control", "⌃↑"),
            MenuRow::item(CMD_SHOW_DESKTOP, "Show Desktop", "F11"),
        ],
        HitTarget::HelpMenu => vec![
            MenuRow::item(CMD_HELP_PALETTE, "Search / Command Palette", "⇧⌘P"),
            MenuRow::item(CMD_HELP_F1, format!("{app_name} Help"), "⌘?"),
        ],
        _ => Vec::new(),
    }
}

#[must_use]
pub fn build_control_center_rows(
    volume: u8,
    brightness: u8,
    now_playing: Option<(&str, &str, bool)>,
    theme_mode: ThemeMode,
    online_lyrics: bool,
) -> Vec<MenuRow> {
    let mut rows = vec![
        MenuRow::Slider {
            id: CMD_CC_VOLUME,
            label: String::from("Sound"),
            value: volume,
        },
        MenuRow::Slider {
            id: CMD_CC_BRIGHTNESS,
            label: String::from("Display"),
            value: brightness,
        },
        MenuRow::Separator,
        MenuRow::item(
            CMD_CC_THEME_TOGGLE,
            format!("Theme: {} (Click to Cycle)", theme_mode.label()),
            "⌘T",
        ),
        MenuRow::check_item(
            CMD_CC_LYRICS_ONLINE,
            if online_lyrics {
                "Online Lyrics: On"
            } else {
                "Online Lyrics: Off"
            },
            online_lyrics,
            true,
        ),
        MenuRow::Separator,
    ];
    if let Some((title, artist, playing)) = now_playing {
        let headline = if artist.is_empty() {
            String::from(title)
        } else {
            format!("{title} - {artist}")
        };
        rows.push(MenuRow::check_item(0, headline, false, false));
        rows.push(MenuRow::item(
            CMD_CC_MEDIA_TOGGLE,
            if playing { "Pause" } else { "Play" },
            "Space",
        ));
        rows.push(MenuRow::item(CMD_CC_MEDIA_NEXT, "Next Track", "??"));
        rows.push(MenuRow::item(CMD_CC_MEDIA_PREVIOUS, "Previous Track", "??"));
        rows.push(MenuRow::Separator);
    }
    rows.push(MenuRow::item(CMD_CC_WIFI, "Wi-Fi Settings...", ""));
    rows.push(MenuRow::item(CMD_CC_BLUETOOTH, "Bluetooth Settings...", ""));
    rows.push(MenuRow::item(CMD_CC_FOCUS, "Focus Settings...", ""));
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(CMD_CC_DISPLAY, "Display Settings...", ""));
    rows.push(MenuRow::item(CMD_CC_SOUND, "Sound Settings...", ""));
    rows
}

#[must_use]
pub fn build_calendar_rows(year: u16, month: u8, today: Option<u8>) -> Vec<MenuRow> {
    vec![
        MenuRow::Calendar { year, month, today },
        MenuRow::Separator,
        MenuRow::item(CMD_CAL_PREVIOUS, "Previous Month", "??"),
        MenuRow::item(CMD_CAL_NEXT, "Next Month", "??"),
        MenuRow::item(CMD_CAL_TODAY, "Go to Today", "?T"),
        MenuRow::Separator,
        MenuRow::item(CMD_CC_CALENDAR, "Notifications", ""),
        MenuRow::item(CMD_CAL_SETTINGS, "Date & Time Settings...", ""),
    ]
}

#[must_use]
pub fn build_window_tiling_menu_rows(maximized: bool) -> Vec<MenuRow> {
    let zoom_label = if maximized {
        "Exit Full Screen / Restore"
    } else {
        "Enter Full Screen / Zoom"
    };
    vec![
        MenuRow::item(CMD_WIN_MAXIMIZE, zoom_label, "⌃⌘F"),
        MenuRow::Separator,
        MenuRow::item(CMD_TILE_LEFT, "Tile Window to Left of Screen", "⌃⌥←"),
        MenuRow::item(CMD_TILE_RIGHT, "Tile Window to Right of Screen", "⌃⌥→"),
        MenuRow::Separator,
        MenuRow::item(CMD_WIN_MINIMIZE, "Minimize", "⌘M"),
        MenuRow::item(CMD_WIN_CLOSE, "Close Window", "⌘W"),
    ]
}

#[must_use]
pub fn build_traffic_tiling_menu_rows(maximized: bool) -> Vec<MenuRow> {
    build_window_tiling_menu_rows(maximized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_menu_layout_sizes_and_hits_enabled_rows() {
        let rows = build_top_bar_menu_rows(HitTarget::WindowMenu, "Code");
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            1920,
            1080,
            96,
            MenuAnchor::TopBar { x: 200, y: 34 },
            &rows,
        );
        assert!(layout.window.width >= 216);
        assert_eq!(layout.rows.len(), rows.len());
        let first_rect = layout.rows[0].rect;
        assert_eq!(layout.hit_test(first_rect.x + 8, first_rect.y + 8), Some(0));
        assert_eq!(layout.command_at(0), Some(CMD_WIN_MINIMIZE));
        let sep_rect = layout.rows[2].rect;
        assert_eq!(layout.hit_test(sep_rect.x + 8, sep_rect.y + 4), None);
    }

    #[test]
    fn above_dock_anchor_centers_and_clamps_within_screen() {
        let rows = build_traffic_tiling_menu_rows(false);
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            1920,
            1080,
            96,
            MenuAnchor::AboveDock {
                center_x: 960,
                bottom_y: 1000,
            },
            &rows,
        );
        let center = layout.window.x + layout.window.width / 2;
        assert!((center - 960).abs() <= 1);
        assert_eq!(layout.window.bottom(), 1000);
    }

    #[test]
    fn control_center_slider_maps_pointer_to_percent() {
        let rows = build_control_center_rows(40, 70, None, ThemeMode::Auto, false);
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            1920,
            1080,
            96,
            MenuAnchor::BelowPoint { x: 1500, y: 34 },
            &rows,
        );
        let row = layout.rows[0].rect;
        let left = row.x + layout.slider_inset;
        let right = row.right() - layout.slider_inset;
        assert_eq!(layout.slider_value_at(0, left - 50), Some(0));
        assert_eq!(layout.slider_value_at(0, right + 50), Some(100));
        let mid = layout
            .slider_value_at(0, i32::midpoint(left, right))
            .unwrap();
        assert!((49..=51).contains(&mid));
        assert_eq!(layout.slider_value_at(3, left), None);
    }

    #[test]
    fn control_center_exposes_online_lyrics_as_a_checked_opt_in() {
        let rows = build_control_center_rows(40, 70, None, ThemeMode::Auto, true);
        assert!(rows.iter().any(|row| matches!(
            row,
            MenuRow::Item {
                id: CMD_CC_LYRICS_ONLINE,
                checked: true,
                enabled: true,
                ..
            }
        )));
    }

    #[test]
    fn calendar_row_is_tall_and_not_interactive() {
        let rows = build_calendar_rows(2026, 11, Some(11));
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            1920,
            1080,
            96,
            MenuAnchor::BelowPoint { x: 1500, y: 34 },
            &rows,
        );
        let cal = layout.rows[0].rect;
        assert_eq!(cal.height, 26 * 8);
        assert_eq!(layout.hit_test(cal.x + 10, cal.y + 10), None);
        assert_eq!(layout.command_at(2), Some(CMD_CAL_PREVIOUS));
    }

    #[test]
    fn top_bar_menu_targets_are_recognized() {
        assert!(is_top_bar_menu_target(HitTarget::Start));
        assert!(is_top_bar_menu_target(HitTarget::AppMenu));
        assert!(is_top_bar_menu_target(HitTarget::HelpMenu));
        assert!(!is_top_bar_menu_target(HitTarget::Notch));
    }
}
