use crate::app_model::{AppCommand, AppInfo, Capability, WindowCommand, WindowInfo};
use crate::layout::{HitTarget, Rect, scale};

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
pub const CMD_APP_HIDE: usize = 18;
pub const CMD_APP_HIDE_OTHERS: usize = 19;
pub const CMD_APP_QUIT: usize = 25;
pub const CMD_APP_SHOW_ALL: usize = 27;

pub const CMD_FILE_NEW: usize = 20;
pub const CMD_FILE_NEW_TAB: usize = 21;
pub const CMD_FILE_OPEN: usize = 22;
pub const CMD_FILE_SAVE: usize = 23;
pub const CMD_FILE_SAVE_AS: usize = 24;
pub const CMD_FILE_NEW_FOLDER: usize = 26;

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
pub const CMD_VIEW_AS_ICONS: usize = 44;
pub const CMD_VIEW_AS_LIST: usize = 45;
pub const CMD_VIEW_AS_DETAILS: usize = 46;

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
pub const CMD_OTTO_SETTINGS: usize = 80;
pub const CMD_OPEN_LOGS: usize = 81;
pub const CMD_NOTIF_CLEAR_ALL: usize = 82;
pub const CMD_NOTIF_REQUEST_ACCESS: usize = 83;
pub const CMD_OVERFLOW_MENU_BASE: usize = 90;
pub const CMD_DOCK_PIN: usize = 100;
pub const CMD_DOCK_TOGGLE: usize = 101;
pub const CMD_DOCK_NEW_WINDOW: usize = 102;
pub const CMD_DOCK_EMPTY_TRASH: usize = 103;
pub const CMD_DOCK_QUIT: usize = 104;
pub const CMD_DOCK_HIDE: usize = 105;
pub const CMD_DOCK_WINDOW_BASE: usize = 200;
pub const CMD_NOTIF_DISMISS_BASE: usize = 1000;
pub const MAX_VISIBLE_NOTIFICATIONS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSummary {
    pub id: u32,
    pub app_id: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationAccess {
    Allowed,
    Denied,
    #[default]
    RequiresPackageIdentity,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NotificationCenterSnapshot {
    pub access: NotificationAccess,
    pub items: Vec<NotificationSummary>,
}

#[must_use]
pub const fn notification_dismiss_index(cmd: usize, len: usize) -> Option<usize> {
    if cmd < CMD_NOTIF_DISMISS_BASE {
        return None;
    }
    let idx = cmd - CMD_NOTIF_DISMISS_BASE;
    if idx < len { Some(idx) } else { None }
}

#[must_use]
pub fn format_relative_age(elapsed_secs: u64) -> String {
    if elapsed_secs < 60 {
        String::from("now")
    } else if elapsed_secs < 3600 {
        format!("{}m", elapsed_secs / 60)
    } else if elapsed_secs < 86_400 {
        format!("{}h", elapsed_secs / 3600)
    } else {
        format!("{}d", elapsed_secs / 86_400)
    }
}

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
    Notification {
        id: usize,
        app_name: String,
        title: String,
        body: String,
        timestamp_label: String,
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
    pub fn item_with_enabled(
        id: usize,
        label: impl Into<String>,
        shortcut: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self::Item {
            id,
            label: label.into(),
            shortcut: shortcut.into(),
            checked: false,
            enabled,
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

    #[must_use]
    pub fn heading(label: impl Into<String>) -> Self {
        Self::item_with_enabled(CMD_NONE, label, "", false)
    }
}

fn push_capable(
    rows: &mut Vec<MenuRow>,
    capability: Capability,
    id: usize,
    label: impl Into<String>,
    shortcut: &str,
) {
    match capability {
        Capability::Available => rows.push(MenuRow::item(id, label, shortcut)),
        Capability::Unsupported => {}
        Capability::TemporarilyUnavailable | Capability::Denied => {
            rows.push(MenuRow::item_with_enabled(id, label, shortcut, false));
        }
    }
}

fn trim_separators(mut rows: Vec<MenuRow>) -> Vec<MenuRow> {
    rows.dedup_by(|next, previous| {
        matches!(next, MenuRow::Separator) && matches!(previous, MenuRow::Separator)
    });
    while matches!(rows.first(), Some(MenuRow::Separator)) {
        rows.remove(0);
    }
    while matches!(rows.last(), Some(MenuRow::Separator)) {
        rows.pop();
    }
    rows
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
        corner_radius: 10,
        row_corner_radius: 5,
        check_col_width: 20,
        shortcut_gap: 24,
        right_pad: 12,
        slider_height: 46,
        slider_inset: 14,
        calendar_cell_height: 26,
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
    pub is_slider: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PopupMenuLayout {
    pub window: Rect,
    pub viewport: Rect,
    pub content_height: i32,
    pub scroll_offset: i32,
    pub corner_radius: f32,
    pub row_corner_radius: f32,
    pub check_col_width: f32,
    pub shortcut_col_width: f32,
    pub right_pad: f32,
    pub slider_inset: i32,
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

    fn interactive_rows(&self) -> Vec<usize> {
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

struct ColumnWidths {
    width: i32,
    shortcut: i32,
}

fn measure_columns(
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
                notification_card_width = scale(284, dpi);
            }
            MenuRow::Slider { .. } | MenuRow::Calendar { .. } | MenuRow::Separator => {}
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

    let heights: Vec<(usize, i32, bool, bool, bool)> = rows
        .iter()
        .map(|row| match row {
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
            MenuRow::Notification { id, .. } => (*id, scale(52, dpi), false, false, true),
            MenuRow::Separator => (
                CMD_NONE,
                scale(metrics.separator_height, dpi),
                true,
                false,
                false,
            ),
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
    for (index, (id, row_height, is_separator, is_slider, enabled)) in
        heights.into_iter().enumerate()
    {
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
            is_slider,
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
            | HitTarget::MenuOverflow
    )
}

#[must_use]
pub const fn top_bar_menu_title(target: HitTarget) -> Option<&'static str> {
    match target {
        HitTarget::FileMenu => Some("File"),
        HitTarget::EditMenu => Some("Edit"),
        HitTarget::ViewMenu => Some("View"),
        HitTarget::WindowMenu => Some("Window"),
        HitTarget::HelpMenu => Some("Help"),
        _ => None,
    }
}

pub const APPLICATION_MENUS: [HitTarget; 5] = [
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
];

pub const OVERFLOW_TARGETS: [HitTarget; 13] = [
    HitTarget::Start,
    HitTarget::AppMenu,
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
    HitTarget::Tray,
    HitTarget::QuickSettings,
    HitTarget::Search,
    HitTarget::ControlCenter,
    HitTarget::Notifications,
    HitTarget::Clock,
];

fn overflow_title(target: HitTarget, app_name: &str) -> Option<String> {
    let title = match target {
        HitTarget::Start => "System",
        HitTarget::AppMenu => app_name,
        HitTarget::Tray => "Tray Icons",
        HitTarget::QuickSettings => "Quick Settings",
        HitTarget::Search => "Search",
        HitTarget::ControlCenter => "Control Center",
        HitTarget::Notifications => "Notifications",
        HitTarget::Clock => "Calendar",
        other => top_bar_menu_title(other)?,
    };
    Some(String::from(title))
}

#[must_use]
pub fn build_menu_overflow_rows(hidden: &[HitTarget], app_name: &str) -> Vec<MenuRow> {
    hidden
        .iter()
        .filter_map(|target| {
            let position = OVERFLOW_TARGETS
                .iter()
                .position(|candidate| candidate == target)?;
            Some(MenuRow::item(
                CMD_OVERFLOW_MENU_BASE + position,
                overflow_title(*target, app_name)?,
                "",
            ))
        })
        .collect()
}

#[must_use]
pub fn overflow_menu_target(command: usize) -> Option<HitTarget> {
    OVERFLOW_TARGETS
        .get(command.checked_sub(CMD_OVERFLOW_MENU_BASE)?)
        .copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowMenuState {
    pub close: Capability,
    pub minimize: Capability,
    pub zoom: Capability,
    pub full_screen: Capability,
    pub tile: Capability,
    pub shortcuts: Capability,
    pub zoomed: bool,
    pub full_screen_active: bool,
}

impl WindowMenuState {
    #[must_use]
    pub const fn of(window: &WindowInfo) -> Self {
        let shortcuts = if window.traits.elevated {
            Capability::Denied
        } else if !window.traits.responding || window.traits.blocked_by_modal {
            Capability::TemporarilyUnavailable
        } else {
            Capability::Available
        };
        Self {
            close: window.capability(WindowCommand::Close),
            minimize: window.capability(WindowCommand::Minimize),
            zoom: window.capability(WindowCommand::Zoom),
            full_screen: window.capability(WindowCommand::ToggleFullScreen),
            tile: window.capability(WindowCommand::TileLeft),
            shortcuts,
            zoomed: window.zoomed,
            full_screen_active: window.full_screen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMenuState {
    pub hide: Capability,
    pub hide_others: Capability,
    pub show_all: Capability,
    pub quit: Capability,
}

impl AppMenuState {
    pub const NONE: Self = Self {
        hide: Capability::Unsupported,
        hide_others: Capability::Unsupported,
        show_all: Capability::Unsupported,
        quit: Capability::Unsupported,
    };

    #[must_use]
    pub fn of(app: &AppInfo, other_apps_visible: bool) -> Self {
        Self {
            hide: app.capability(AppCommand::Hide, other_apps_visible),
            hide_others: app.capability(AppCommand::HideOthers, other_apps_visible),
            show_all: match app.capability(AppCommand::ShowAll, other_apps_visible) {
                Capability::Available => Capability::Available,
                _ => Capability::Unsupported,
            },
            quit: app.capability(AppCommand::Quit, other_apps_visible),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuContext<'a> {
    pub app_name: &'a str,
    pub file_manager: bool,
    pub tabbed: bool,
    pub window: Option<WindowMenuState>,
    pub app: AppMenuState,
}

impl<'a> MenuContext<'a> {
    #[must_use]
    pub const fn without_window(app_name: &'a str) -> Self {
        Self {
            app_name,
            file_manager: false,
            tabbed: false,
            window: None,
            app: AppMenuState::NONE,
        }
    }

    fn shortcuts(&self) -> Capability {
        self.window
            .map_or(Capability::Unsupported, |window| window.shortcuts)
    }

    fn window_capability(&self, select: impl Fn(&WindowMenuState) -> Capability) -> Capability {
        self.window.as_ref().map_or(Capability::Unsupported, select)
    }
}

#[must_use]
pub fn build_apple_menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::item(CMD_START, "Open Start", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_OTTO_SETTINGS, "Otto Settings...", ""),
        MenuRow::item(CMD_OPEN_LOGS, "Open Logs Folder", ""),
        MenuRow::item(CMD_SETTINGS, "System Settings...", ""),
        MenuRow::item(CMD_TASKMGR, "Force Quit (Task Manager)...", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_SLEEP, "Sleep", ""),
        MenuRow::item(CMD_RESTART, "Restart...", ""),
        MenuRow::item(CMD_SHUTDOWN, "Shut Down...", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_LOCK, "Lock Screen", ""),
        MenuRow::item(CMD_QUIT, "Quit Otto Bar", ""),
    ]
}

fn build_app_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let name = context.app_name;
    let mut rows = Vec::new();
    if context.window.is_some() || context.file_manager {
        rows.push(MenuRow::item(CMD_APP_ACTIVATE, format!("Show {name}"), ""));
        rows.push(MenuRow::Separator);
    }
    push_capable(
        &mut rows,
        context.app.hide,
        CMD_APP_HIDE,
        format!("Hide {name}"),
        "",
    );
    push_capable(
        &mut rows,
        context.app.hide_others,
        CMD_APP_HIDE_OTHERS,
        "Hide Others",
        "",
    );
    push_capable(
        &mut rows,
        context.app.show_all,
        CMD_APP_SHOW_ALL,
        "Show All",
        "",
    );
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(
        CMD_TASKMGR,
        "Force Quit (Task Manager)...",
        "",
    ));
    push_capable(
        &mut rows,
        context.app.quit,
        CMD_APP_QUIT,
        format!("Quit {name}"),
        "",
    );
    rows
}

fn build_file_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let keys = context.shortcuts();
    let close = context.window_capability(|window| window.close);
    let mut rows = Vec::new();
    if context.file_manager {
        push_capable(&mut rows, keys, CMD_FILE_NEW_FOLDER, "New Folder", "⇧⌃N");
        push_capable(&mut rows, keys, CMD_FILE_NEW, "New Finder Window", "⌃N");
        push_capable(&mut rows, keys, CMD_FILE_NEW_TAB, "New Tab", "⌃T");
    } else {
        push_capable(&mut rows, keys, CMD_FILE_NEW, "New", "⌃N");
        if context.tabbed {
            push_capable(&mut rows, keys, CMD_FILE_NEW_TAB, "New Tab", "⌃T");
        }
        push_capable(&mut rows, keys, CMD_FILE_OPEN, "Open...", "⌃O");
        rows.push(MenuRow::Separator);
        push_capable(&mut rows, keys, CMD_FILE_SAVE, "Save", "⌃S");
        push_capable(&mut rows, keys, CMD_FILE_SAVE_AS, "Save As...", "⇧⌃S");
    }
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, close, CMD_WIN_CLOSE, "Close Window", "");
    rows
}

fn build_edit_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let keys = context.shortcuts();
    let mut rows = Vec::new();
    push_capable(&mut rows, keys, CMD_EDIT_UNDO, "Undo", "⌃Z");
    push_capable(&mut rows, keys, CMD_EDIT_REDO, "Redo", "⌃Y");
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, keys, CMD_EDIT_CUT, "Cut", "⌃X");
    push_capable(&mut rows, keys, CMD_EDIT_COPY, "Copy", "⌃C");
    push_capable(&mut rows, keys, CMD_EDIT_PASTE, "Paste", "⌃V");
    push_capable(&mut rows, keys, CMD_EDIT_SELECT_ALL, "Select All", "⌃A");
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, keys, CMD_EDIT_FIND, "Find...", "⌃F");
    rows
}

fn full_screen_label(context: &MenuContext<'_>) -> &'static str {
    if context
        .window
        .is_some_and(|window| window.full_screen_active)
    {
        "Exit Full Screen"
    } else {
        "Enter Full Screen"
    }
}

fn build_view_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let keys = context.shortcuts();
    let full_screen = context.window_capability(|window| window.full_screen);
    let mut rows = Vec::new();
    if context.file_manager {
        push_capable(&mut rows, keys, CMD_VIEW_AS_ICONS, "as Icons", "⇧⌃2");
        push_capable(&mut rows, keys, CMD_VIEW_AS_LIST, "as List", "⇧⌃5");
        push_capable(&mut rows, keys, CMD_VIEW_AS_DETAILS, "as Details", "⇧⌃6");
    } else {
        push_capable(&mut rows, keys, CMD_VIEW_ZOOM_IN, "Zoom In", "⌃+");
        push_capable(&mut rows, keys, CMD_VIEW_ZOOM_OUT, "Zoom Out", "⌃-");
        push_capable(&mut rows, keys, CMD_VIEW_ACTUAL_SIZE, "Actual Size", "⌃0");
    }
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        full_screen,
        CMD_VIEW_FULLSCREEN,
        full_screen_label(context),
        "F11",
    );
    rows
}

fn build_window_menu_rows(context: &MenuContext<'_>) -> Vec<MenuRow> {
    let tile = context.window_capability(|window| window.tile);
    let mut rows = Vec::new();
    push_capable(
        &mut rows,
        context.window_capability(|window| window.minimize),
        CMD_WIN_MINIMIZE,
        "Minimize",
        "",
    );
    push_capable(
        &mut rows,
        context.window_capability(|window| window.zoom),
        CMD_WIN_MAXIMIZE,
        "Zoom",
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        tile,
        CMD_TILE_LEFT,
        "Tile Window to Left of Screen",
        "",
    );
    push_capable(
        &mut rows,
        tile,
        CMD_TILE_RIGHT,
        "Tile Window to Right of Screen",
        "",
    );
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(CMD_TASK_VIEW, "Mission Control", "⊞Tab"));
    rows.push(MenuRow::item(CMD_SHOW_DESKTOP, "Show Desktop", "⊞D"));
    rows
}

#[must_use]
pub fn build_top_bar_menu_rows(target: HitTarget, context: &MenuContext<'_>) -> Vec<MenuRow> {
    let rows = match target {
        HitTarget::Start => build_apple_menu_rows(),
        HitTarget::AppMenu => build_app_menu_rows(context),
        HitTarget::FileMenu => build_file_menu_rows(context),
        HitTarget::EditMenu => build_edit_menu_rows(context),
        HitTarget::ViewMenu => build_view_menu_rows(context),
        HitTarget::WindowMenu => build_window_menu_rows(context),
        HitTarget::HelpMenu => {
            let mut rows = Vec::new();
            push_capable(
                &mut rows,
                context.shortcuts(),
                CMD_HELP_F1,
                format!("{} Help", context.app_name),
                "F1",
            );
            rows
        }
        _ => Vec::new(),
    };
    let rows = trim_separators(rows);
    if rows.is_empty() && top_bar_menu_title(target).is_some() {
        return vec![MenuRow::heading("No Actions Available")];
    }
    rows
}

#[must_use]
pub fn build_calendar_rows(year: u16, month: u8, today: Option<u8>) -> Vec<MenuRow> {
    vec![
        MenuRow::Calendar { year, month, today },
        MenuRow::Separator,
        MenuRow::item(CMD_CAL_PREVIOUS, "Previous Month", ""),
        MenuRow::item(CMD_CAL_NEXT, "Next Month", ""),
        MenuRow::item(CMD_CAL_TODAY, "Go to Today", ""),
        MenuRow::Separator,
        MenuRow::item(CMD_CAL_SETTINGS, "Date & Time Settings...", ""),
    ]
}

#[must_use]
pub fn build_notification_rows(notifications: &NotificationCenterSnapshot) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    match notifications.access {
        NotificationAccess::Allowed => {
            if notifications.items.is_empty() {
                rows.push(MenuRow::heading("Notifications"));
                rows.push(MenuRow::heading("No Recent Notifications"));
            } else {
                let visible: Vec<_> = notifications
                    .items
                    .iter()
                    .take(MAX_VISIBLE_NOTIFICATIONS)
                    .collect();
                let mut app_ids = Vec::new();
                for item in &visible {
                    if !app_ids.contains(&item.app_id) {
                        app_ids.push(item.app_id.clone());
                    }
                }
                for app_id in app_ids {
                    let group: Vec<_> = visible
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| item.app_id == app_id)
                        .collect();
                    let app_name = &group[0].1.app_name;
                    let heading = if group.len() > 1 {
                        format!("{app_name} ({})", group.len())
                    } else {
                        app_name.clone()
                    };
                    rows.push(MenuRow::heading(heading));
                    for (index, item) in group {
                        rows.push(MenuRow::Notification {
                            id: CMD_NOTIF_DISMISS_BASE + index,
                            app_name: item.app_name.clone(),
                            title: item.title.clone(),
                            body: item.body.clone(),
                            timestamp_label: item.timestamp_label.clone(),
                        });
                    }
                }
                rows.push(MenuRow::item(
                    CMD_NOTIF_CLEAR_ALL,
                    "Clear All Notifications",
                    "",
                ));
            }
        }
        NotificationAccess::Denied => {
            rows.push(MenuRow::heading("Notification Access Denied"));
            rows.push(MenuRow::item(
                CMD_NOTIF_REQUEST_ACCESS,
                "Check Notification Permission",
                "",
            ));
        }
        NotificationAccess::RequiresPackageIdentity => {
            rows.push(MenuRow::heading("Notifications"));
            rows.push(MenuRow::heading("Requires Package Identity"));
        }
        NotificationAccess::Unavailable => {
            rows.push(MenuRow::heading("Notifications"));
            rows.push(MenuRow::item(
                CMD_NOTIF_REQUEST_ACCESS,
                "Enable Notification Feed",
                "",
            ));
        }
    }
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(
        CMD_CC_CALENDAR,
        "Open Windows Notifications",
        "",
    ));
    rows
}

#[must_use]
pub fn build_window_tiling_menu_rows(window: &WindowMenuState) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    push_capable(
        &mut rows,
        window.zoom,
        CMD_WIN_MAXIMIZE,
        if window.zoomed { "Restore" } else { "Zoom" },
        "",
    );
    push_capable(
        &mut rows,
        window.full_screen,
        CMD_VIEW_FULLSCREEN,
        if window.full_screen_active {
            "Exit Full Screen"
        } else {
            "Enter Full Screen"
        },
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(
        &mut rows,
        window.tile,
        CMD_TILE_LEFT,
        "Tile Window to Left of Screen",
        "",
    );
    push_capable(
        &mut rows,
        window.tile,
        CMD_TILE_RIGHT,
        "Tile Window to Right of Screen",
        "",
    );
    rows.push(MenuRow::Separator);
    push_capable(&mut rows, window.minimize, CMD_WIN_MINIMIZE, "Minimize", "");
    push_capable(&mut rows, window.close, CMD_WIN_CLOSE, "Close Window", "");
    let rows = trim_separators(rows);
    if rows.is_empty() {
        return vec![MenuRow::heading("No Actions Available")];
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_model::{
        AppIdentity, WindowIdentity, WindowPresence, WindowRole, WindowTraits, group_windows,
    };

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const TEXT: EstimatedMenuText = EstimatedMenuText { dpi: 96 };

    fn placement(bounds: Rect, anchor: MenuAnchor, scroll_offset: i32) -> PopupMenuPlacement {
        PopupMenuPlacement {
            bounds,
            dpi: 96,
            anchor,
            scroll_offset,
        }
    }

    fn layout_for(rows: &[MenuRow], anchor: MenuAnchor) -> PopupMenuLayout {
        compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            placement(SCREEN, anchor, 0),
            rows,
            &TEXT,
        )
    }

    fn window(traits: WindowTraits) -> WindowInfo {
        WindowInfo {
            identity: WindowIdentity {
                hwnd: 1,
                process_id: 2,
                thread_id: 3,
                generation: 4,
            },
            app: AppIdentity::executable("code.exe"),
            app_name: String::from("Code"),
            executable: String::from("code.exe"),
            title: String::from("main.rs"),
            role: WindowRole::Primary,
            presence: WindowPresence::Visible,
            zoomed: false,
            full_screen: false,
            active: true,
            traits,
        }
    }

    fn context(traits: WindowTraits) -> MenuContext<'static> {
        let info = window(traits);
        let app = group_windows(vec![info.clone()]).remove(0);
        MenuContext {
            app_name: "Code",
            file_manager: false,
            tabbed: false,
            window: Some(WindowMenuState::of(&info)),
            app: AppMenuState::of(&app, true),
        }
    }

    fn ids(rows: &[MenuRow], only_enabled: bool) -> Vec<usize> {
        rows.iter()
            .filter_map(|row| match row {
                MenuRow::Item { id, enabled, .. } if *enabled || !only_enabled => Some(*id),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn popup_menu_layout_sizes_and_hits_enabled_rows() {
        let rows = build_top_bar_menu_rows(HitTarget::WindowMenu, &context(WindowTraits::STANDARD));
        let layout = layout_for(&rows, MenuAnchor::TopBar { x: 200, y: 34 });
        assert!(layout.window.width >= 216);
        assert_eq!(layout.rows.len(), rows.len());
        let first_rect = layout.rows[0].rect;
        assert_eq!(layout.hit_test(first_rect.x + 8, first_rect.y + 8), Some(0));
        assert_eq!(layout.command_at(0), Some(CMD_WIN_MINIMIZE));
        let sep_rect = layout.rows[2].rect;
        assert_eq!(layout.hit_test(sep_rect.x + 8, sep_rect.y + 4), None);
        assert!(!layout.is_scrollable());
    }

    #[test]
    fn keyboard_navigation_cycles_interactive_rows_and_skips_separators() {
        let rows = build_top_bar_menu_rows(HitTarget::FileMenu, &context(WindowTraits::STANDARD));
        let layout = layout_for(&rows, MenuAnchor::TopBar { x: 100, y: 34 });
        let first = layout.next_interactive_row(None);
        assert_eq!(first, Some(0));
        let second = layout.next_interactive_row(first);
        assert_eq!(second, Some(1));
        let third = layout.next_interactive_row(second);
        assert_eq!(third, Some(3));
        assert_eq!(layout.prev_interactive_row(third), Some(1));
        assert_eq!(
            layout.prev_interactive_row(first),
            layout.interactive_rows().last().copied()
        );
    }

    #[test]
    fn type_to_select_cycles_through_rows_sharing_a_first_letter() {
        let rows = build_top_bar_menu_rows(HitTarget::FileMenu, &context(WindowTraits::STANDARD));
        let layout = layout_for(&rows, MenuAnchor::TopBar { x: 100, y: 34 });
        let save = layout.find_row_by_prefix(&rows, 's', None);
        let save_as = layout.find_row_by_prefix(&rows, 'S', save);
        assert_eq!(save, Some(3));
        assert_eq!(save_as, Some(4));
        assert_eq!(layout.find_row_by_prefix(&rows, 's', save_as), save);
        assert_eq!(layout.find_row_by_prefix(&rows, 'z', None), None);
    }

    #[test]
    fn menus_without_a_target_window_offer_no_window_or_shortcut_actions() {
        let bare = MenuContext::without_window("Finder");
        for target in APPLICATION_MENUS {
            let rows = build_top_bar_menu_rows(target, &bare);
            let enabled = ids(&rows, true);
            assert!(
                enabled
                    .iter()
                    .all(|id| matches!(*id, CMD_TASK_VIEW | CMD_SHOW_DESKTOP)),
                "{target:?} offered {enabled:?}"
            );
            assert!(!rows.is_empty());
        }
    }

    #[test]
    fn unsupported_window_actions_are_absent_and_blocked_ones_are_disabled() {
        let mut fixed = WindowTraits::STANDARD;
        fixed.maximizable = false;
        fixed.resizable = false;
        let rows = build_top_bar_menu_rows(HitTarget::WindowMenu, &context(fixed));
        let all = ids(&rows, false);
        assert!(all.contains(&CMD_WIN_MINIMIZE));
        assert!(!all.contains(&CMD_WIN_MAXIMIZE));
        assert!(!all.contains(&CMD_TILE_LEFT));

        let mut elevated = WindowTraits::STANDARD;
        elevated.elevated = true;
        let rows = build_top_bar_menu_rows(HitTarget::EditMenu, &context(elevated));
        assert!(ids(&rows, true).is_empty());
        assert!(ids(&rows, false).contains(&CMD_EDIT_PASTE));
    }

    #[test]
    fn full_screen_row_appears_only_for_applications_that_support_it() {
        let plain = build_top_bar_menu_rows(HitTarget::ViewMenu, &context(WindowTraits::STANDARD));
        assert!(!ids(&plain, false).contains(&CMD_VIEW_FULLSCREEN));
        let mut capable = WindowTraits::STANDARD;
        capable.full_screen_shortcut = true;
        let rows = build_top_bar_menu_rows(HitTarget::ViewMenu, &context(capable));
        assert!(ids(&rows, true).contains(&CMD_VIEW_FULLSCREEN));
    }

    #[test]
    fn app_menu_separates_hide_quit_and_window_close() {
        let rows = build_top_bar_menu_rows(HitTarget::AppMenu, &context(WindowTraits::STANDARD));
        let enabled = ids(&rows, true);
        assert!(enabled.contains(&CMD_APP_HIDE));
        assert!(enabled.contains(&CMD_APP_HIDE_OTHERS));
        assert!(enabled.contains(&CMD_APP_QUIT));
        assert!(!enabled.contains(&CMD_WIN_CLOSE));
        assert!(!enabled.contains(&CMD_WIN_MINIMIZE));
        assert!(!enabled.contains(&CMD_APP_SHOW_ALL));
    }

    #[test]
    fn file_manager_menu_maps_labels_to_distinct_commands() {
        let mut finder = context(WindowTraits::STANDARD);
        finder.file_manager = true;
        let file = ids(&build_top_bar_menu_rows(HitTarget::FileMenu, &finder), true);
        assert!(file.contains(&CMD_FILE_NEW_FOLDER));
        assert!(file.contains(&CMD_FILE_NEW));
        assert!(!file.contains(&CMD_FILE_SAVE));
        let view = ids(&build_top_bar_menu_rows(HitTarget::ViewMenu, &finder), true);
        assert_eq!(
            &view[..3],
            &[CMD_VIEW_AS_ICONS, CMD_VIEW_AS_LIST, CMD_VIEW_AS_DETAILS]
        );
    }

    #[test]
    fn above_dock_anchor_centers_and_clamps_within_bounds() {
        let state = WindowMenuState::of(&window(WindowTraits::STANDARD));
        let rows = build_window_tiling_menu_rows(&state);
        let layout = layout_for(
            &rows,
            MenuAnchor::AboveDock {
                center_x: 960,
                bottom_y: 1000,
            },
        );
        let center = layout.window.x + layout.window.width / 2;
        assert!((center - 960).abs() <= 1);
        assert_eq!(layout.window.bottom(), 1000);
    }

    #[test]
    fn popup_is_placed_inside_the_anchor_monitor_work_area() {
        let secondary = Rect {
            x: -1280,
            y: 40,
            width: 1280,
            height: 720,
        };
        let rows = build_apple_menu_rows();
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            placement(secondary, MenuAnchor::BelowPoint { x: -40, y: 0 }, 0),
            &rows,
            &TEXT,
        );
        assert!(layout.window.x >= secondary.x);
        assert!(layout.window.right() <= secondary.right());
        assert!(layout.window.y >= secondary.y);
        assert!(layout.window.bottom() <= secondary.bottom());
    }

    #[test]
    fn tall_menu_scrolls_so_every_row_stays_reachable() {
        let rows: Vec<MenuRow> = (0..60)
            .map(|index| MenuRow::item(CMD_DOCK_WINDOW_BASE + index, format!("Window {index}"), ""))
            .collect();
        let small = Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 400,
        };
        let anchor = MenuAnchor::BelowPoint { x: 10, y: 10 };
        let mut layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            placement(small, anchor, 0),
            &rows,
            &TEXT,
        );
        assert!(layout.is_scrollable());
        assert!(layout.window.bottom() <= small.bottom());
        let mut reached = Vec::new();
        for index in 0..rows.len() {
            let offset = layout.scroll_revealing(index);
            layout = compute_popup_menu_layout(
                &PopupMenuMetrics::MACOS_SONOMA,
                placement(small, anchor, offset),
                &rows,
                &TEXT,
            );
            let row = layout.rows[index];
            assert!(row.rect.y >= layout.viewport.y);
            assert!(row.rect.bottom() <= layout.viewport.bottom());
            if layout.hit_test(row.rect.x + 4, row.rect.y + 4) == Some(index) {
                reached.push(index);
            }
        }
        assert_eq!(reached.len(), rows.len());
        assert_eq!(layout.scrolled_by(10_000), layout.max_scroll());
        assert_eq!(layout.scrolled_by(-10_000), 0);
    }

    #[test]
    fn rows_scrolled_outside_the_viewport_are_not_hit() {
        let rows: Vec<MenuRow> = (0..60)
            .map(|index| MenuRow::item(300 + index, format!("Row {index}"), ""))
            .collect();
        let small = Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 300,
        };
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            placement(small, MenuAnchor::BelowPoint { x: 10, y: 10 }, 48),
            &rows,
            &TEXT,
        );
        let hidden = layout.rows[0];
        assert!(!layout.is_row_reachable(&hidden));
        assert_eq!(layout.hit_test(hidden.rect.x + 4, hidden.rect.y + 4), None);
        assert_eq!(layout.hit_test(8, 2), None);
    }

    #[test]
    fn shortcut_column_is_sized_from_measured_text() {
        struct Wide;
        impl MenuTextMeasure for Wide {
            fn label_width(&self, text: &str) -> i32 {
                text.chars().count() as i32 * 9
            }
            fn shortcut_width(&self, _text: &str) -> i32 {
                64
            }
        }
        let rows = vec![MenuRow::item(1, "Save", "Ctrl+Shift+S")];
        let layout = compute_popup_menu_layout(
            &PopupMenuMetrics::MACOS_SONOMA,
            placement(SCREEN, MenuAnchor::BelowPoint { x: 10, y: 10 }, 0),
            &rows,
            &Wide,
        );
        assert!((layout.shortcut_col_width - 64.0).abs() < f32::EPSILON);
        let label_right =
            layout.window.width as f32 - layout.right_pad - layout.shortcut_col_width - 24.0 - 5.0;
        assert!(label_right >= layout.check_col_width + 36.0);
    }

    #[test]
    fn calendar_row_is_tall_and_not_interactive() {
        let rows = build_calendar_rows(2026, 11, Some(11));
        let layout = layout_for(&rows, MenuAnchor::BelowPoint { x: 1500, y: 34 });
        let cal = layout.rows[0].rect;
        assert_eq!(cal.height, 26 * 8);
        assert_eq!(layout.hit_test(cal.x + 10, cal.y + 10), None);
        assert_eq!(layout.command_at(2), Some(CMD_CAL_PREVIOUS));
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row, MenuRow::Notification { .. }))
        );
    }

    #[test]
    fn top_bar_menu_targets_are_recognized() {
        assert!(is_top_bar_menu_target(HitTarget::Start));
        assert!(is_top_bar_menu_target(HitTarget::AppMenu));
        assert!(is_top_bar_menu_target(HitTarget::HelpMenu));
        assert!(is_top_bar_menu_target(HitTarget::MenuOverflow));
        assert!(!is_top_bar_menu_target(HitTarget::Notch));
    }

    #[test]
    fn overflow_rows_round_trip_to_their_menu_targets() {
        let hidden = [
            HitTarget::WindowMenu,
            HitTarget::HelpMenu,
            HitTarget::Search,
        ];
        let rows = build_menu_overflow_rows(&hidden, "Code");
        let commands = ids(&rows, true);
        assert_eq!(commands.len(), 3);
        assert_eq!(
            overflow_menu_target(commands[0]),
            Some(HitTarget::WindowMenu)
        );
        assert_eq!(overflow_menu_target(commands[1]), Some(HitTarget::HelpMenu));
        assert_eq!(overflow_menu_target(commands[2]), Some(HitTarget::Search));
        assert_eq!(overflow_menu_target(CMD_OVERFLOW_MENU_BASE + 13), None);
        assert_eq!(overflow_menu_target(3), None);
    }

    #[test]
    fn tiling_menu_reports_when_no_action_is_possible() {
        let mut locked = WindowTraits::STANDARD;
        locked.closable = false;
        locked.minimizable = false;
        locked.maximizable = false;
        locked.resizable = false;
        let rows = build_window_tiling_menu_rows(&WindowMenuState::of(&window(locked)));
        assert_eq!(rows.len(), 1);
        assert!(ids(&rows, true).is_empty());
    }

    #[test]
    fn notification_rows_group_apps_and_cover_access_modes() {
        let populated = NotificationCenterSnapshot {
            access: NotificationAccess::Allowed,
            items: (0..7)
                .map(|i| NotificationSummary {
                    id: 100 + i,
                    app_id: if i == 1 {
                        String::from("Contoso.Chat")
                    } else {
                        String::from("Contoso.Mail")
                    },
                    app_name: if i == 1 {
                        String::from("Chat")
                    } else {
                        String::from("Mail")
                    },
                    title: format!("Message {i}"),
                    body: String::from("Body preview"),
                    timestamp_label: format_relative_age(120),
                })
                .collect(),
        };
        let rows = build_notification_rows(&populated);
        let notif_rows: Vec<_> = rows
            .iter()
            .filter(|row| matches!(row, MenuRow::Notification { .. }))
            .collect();
        assert_eq!(notif_rows.len(), MAX_VISIBLE_NOTIFICATIONS);
        assert!(rows.iter().any(|row| matches!(
            row,
            MenuRow::Item { label, enabled: false, .. } if label == "Mail (4)"
        )));
        assert!(rows.iter().any(|row| matches!(
            row,
            MenuRow::Item { label, enabled: false, .. } if label == "Chat"
        )));
        assert_eq!(
            notification_dismiss_index(CMD_NOTIF_DISMISS_BASE + 2, 5),
            Some(2)
        );
        assert_eq!(
            notification_dismiss_index(CMD_NOTIF_DISMISS_BASE + 5, 5),
            None
        );
        assert_eq!(notification_dismiss_index(CMD_NOTIF_CLEAR_ALL, 5), None);

        let layout = layout_for(&rows, MenuAnchor::BelowPoint { x: 1500, y: 34 });
        assert!(layout.window.width >= 284);
        assert!(rows.iter().any(|row| matches!(
            row,
            MenuRow::Item { id, .. } if *id == CMD_NOTIF_CLEAR_ALL
        )));

        let denied = NotificationCenterSnapshot {
            access: NotificationAccess::Denied,
            items: Vec::new(),
        };
        let denied_rows = build_notification_rows(&denied);
        assert!(denied_rows.iter().any(|row| matches!(
            row,
            MenuRow::Item { id, .. } if *id == CMD_NOTIF_REQUEST_ACCESS
        )));

        let unpackaged = NotificationCenterSnapshot::default();
        let unpackaged_rows = build_notification_rows(&unpackaged);
        assert!(unpackaged_rows.iter().any(|row| matches!(
            row,
            MenuRow::Item { id, .. } if *id == CMD_CC_CALENDAR
        )));
    }
}
