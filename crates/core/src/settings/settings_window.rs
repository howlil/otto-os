use super::model::{OttoSettingItem, OttoSettingKind, OttoSettingsSnapshot};
use crate::layout::{Rect, scale};
use crate::settings_content::ContentLayout;
use crate::window::controls::WindowControlAction;

pub const SETTINGS_PAGE_COUNT: usize = 13;

#[must_use]
pub const fn settings_background(dark: bool) -> u32 {
    if dark { 0x1E_1E20 } else { 0xF2_F2F7 }
}

#[must_use]
pub const fn settings_text(dark: bool) -> u32 {
    if dark { 0xF5_F5F7 } else { 0x1D_1D1F }
}

pub const MODULE_COUNT: usize = 14;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsGroup {
    Home,
    Surfaces,
    Otto,
}

impl SettingsGroup {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Home => "",
            Self::Surfaces => "Surfaces",
            Self::Otto => "Otto",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsPage {
    Overview,
    Modules,
    TopBar,
    Notch,
    Focus,
    Dock,
    WindowControls,
    Spotlight,
    ControlCenter,
    Appearance,
    Shortcuts,
    General,
    About,
}

impl SettingsPage {
    pub const ALL: [Self; SETTINGS_PAGE_COUNT] = [
        Self::Overview,
        Self::Modules,
        Self::TopBar,
        Self::Notch,
        Self::Focus,
        Self::Dock,
        Self::WindowControls,
        Self::Spotlight,
        Self::ControlCenter,
        Self::Appearance,
        Self::Shortcuts,
        Self::General,
        Self::About,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Modules => "Modules",
            Self::TopBar => "Top Bar",
            Self::Notch => "Notch",
            Self::Focus => "Focus",
            Self::Dock => "Dock",
            Self::WindowControls => "Window Controls",
            Self::Spotlight => "Spotlight",
            Self::ControlCenter => "Control Center",
            Self::Appearance => "Appearance",
            Self::Shortcuts => "Shortcuts",
            Self::General => "General",
            Self::About => "About",
        }
    }

    #[must_use]
    pub const fn group(self) -> SettingsGroup {
        match self {
            Self::Overview | Self::Modules => SettingsGroup::Home,
            Self::TopBar
            | Self::Notch
            | Self::Focus
            | Self::Dock
            | Self::WindowControls
            | Self::Spotlight
            | Self::ControlCenter => SettingsGroup::Surfaces,
            Self::Appearance | Self::Shortcuts | Self::General | Self::About => SettingsGroup::Otto,
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Modules => 1,
            Self::TopBar => 2,
            Self::Notch => 3,
            Self::Focus => 4,
            Self::Dock => 5,
            Self::WindowControls => 6,
            Self::Spotlight => 7,
            Self::ControlCenter => 8,
            Self::Appearance => 9,
            Self::Shortcuts => 10,
            Self::General => 11,
            Self::About => 12,
        }
    }

    #[must_use]
    pub const fn for_kind(kind: OttoSettingKind) -> Self {
        match kind {
            OttoSettingKind::TopBar
            | OttoSettingKind::MenuBarBackground
            | OttoSettingKind::MenuBarKeepNotch
            | OttoSettingKind::MenuBarTray
            | OttoSettingKind::MenuBarStatus
            | OttoSettingKind::MenuBarSearch
            | OttoSettingKind::MenuBarNotifications
            | OttoSettingKind::MenuBarClock => Self::TopBar,
            OttoSettingKind::Notch
            | OttoSettingKind::DynamicIsland
            | OttoSettingKind::NotchHud
            | OttoSettingKind::PrivacyIndicators
            | OttoSettingKind::MediaModule
            | OttoSettingKind::OnlineLyrics
            | OttoSettingKind::ClipboardHistory
            | OttoSettingKind::QuickCapture
            | OttoSettingKind::DropShelf => Self::Notch,
            OttoSettingKind::FocusModule => Self::Focus,
            OttoSettingKind::Dock
            | OttoSettingKind::DockHideTaskbar
            | OttoSettingKind::DockShowDownloads
            | OttoSettingKind::DockShowTrash
            | OttoSettingKind::DockAutohide
            | OttoSettingKind::DockReserveSpace
            | OttoSettingKind::DockMagnification => Self::Dock,
            OttoSettingKind::WindowControls | OttoSettingKind::Genie => Self::WindowControls,
            OttoSettingKind::CaptureVisible => Self::Appearance,
            OttoSettingKind::Hotkeys => Self::Shortcuts,
            OttoSettingKind::LaunchAtLogin
            | OttoSettingKind::BackgroundCleanup
            | OttoSettingKind::Finder => Self::General,
        }
    }

    #[must_use]
    pub const fn for_item(item: OttoSettingItem) -> Self {
        match item {
            OttoSettingItem::Toggle(kind) => Self::for_kind(kind),
            OttoSettingItem::DockIconSize => Self::Dock,
            OttoSettingItem::MenuBarHide | OttoSettingItem::BrandLogo => Self::TopBar,
        }
    }

    #[must_use]
    pub const fn neighbor(self, forward: bool) -> Option<Self> {
        let index = self.index();
        if forward {
            if index + 1 < SETTINGS_PAGE_COUNT {
                Some(Self::ALL[index + 1])
            } else {
                None
            }
        } else if index > 0 {
            Some(Self::ALL[index - 1])
        } else {
            None
        }
    }
}

impl OttoSettingsSnapshot {
    #[must_use]
    pub fn page_items(&self, page: SettingsPage) -> Vec<OttoSettingItem> {
        OttoSettingItem::ALL
            .into_iter()
            .filter(|item| SettingsPage::for_item(*item) == page && self.is_item_available(*item))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleGroup {
    Surfaces,
    NotchTools,
    System,
}

impl ModuleGroup {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Surfaces => "Surfaces",
            Self::NotchTools => "Notch tools",
            Self::System => "System",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModuleId {
    TopBar,
    Notch,
    Dock,
    Finder,
    WindowControls,
    Genie,
    Spotlight,
    ControlCenter,
    Media,
    Focus,
    Clipboard,
    Pocket,
    QuickCapture,
    BackgroundCleanup,
}

impl ModuleId {
    pub const ALL: [Self; MODULE_COUNT] = [
        Self::TopBar,
        Self::Notch,
        Self::Dock,
        Self::Finder,
        Self::WindowControls,
        Self::Genie,
        Self::Spotlight,
        Self::ControlCenter,
        Self::Media,
        Self::Focus,
        Self::Clipboard,
        Self::Pocket,
        Self::QuickCapture,
        Self::BackgroundCleanup,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopBar => "Top Bar",
            Self::Notch => "Notch",
            Self::Dock => "Dock",
            Self::Finder => "Finder",
            Self::WindowControls => "Window Controls",
            Self::Genie => "Genie",
            Self::Spotlight => "Spotlight",
            Self::ControlCenter => "Control Center",
            Self::Media => "Now Playing",
            Self::Focus => "Focus",
            Self::Clipboard => "Clipboard",
            Self::Pocket => "Pocket",
            Self::QuickCapture => "Quick Capture",
            Self::BackgroundCleanup => "Background Cleanup",
        }
    }

    #[must_use]
    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::TopBar => "Menus, tray, status, clock",
            Self::Notch => "Focus, music, HUD, notifications",
            Self::Dock => "Apps, Downloads, Trash",
            Self::Finder => "macOS-style File Explorer",
            Self::WindowControls => "Close, minimize, zoom circles",
            Self::Genie => "Minimize into the Dock",
            Self::Spotlight => "Local app search",
            Self::ControlCenter => "Sound, display, now playing",
            Self::Media => "Playback in the Notch",
            Self::Focus => "Pomodoro focus sessions",
            Self::Clipboard => "Recent copies in the Notch",
            Self::Pocket => "File drop shelf in the Notch",
            Self::QuickCapture => "Tasks and Ctrl+Alt+Q capture",
            Self::BackgroundCleanup => "Throttles CPU hogs, never kills",
        }
    }

    #[must_use]
    pub const fn group(self) -> ModuleGroup {
        match self {
            Self::TopBar
            | Self::Notch
            | Self::Dock
            | Self::Finder
            | Self::WindowControls
            | Self::Genie
            | Self::Spotlight
            | Self::ControlCenter => ModuleGroup::Surfaces,
            Self::Media | Self::Focus | Self::Clipboard | Self::Pocket | Self::QuickCapture => {
                ModuleGroup::NotchTools
            }
            Self::BackgroundCleanup => ModuleGroup::System,
        }
    }

    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::Media | Self::Focus | Self::Clipboard | Self::Pocket | Self::QuickCapture => {
                Some(Self::Notch)
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn setting_kind(self) -> Option<OttoSettingKind> {
        match self {
            Self::TopBar => Some(OttoSettingKind::TopBar),
            Self::Notch => Some(OttoSettingKind::Notch),
            Self::Dock => Some(OttoSettingKind::Dock),
            Self::Finder => Some(OttoSettingKind::Finder),
            Self::WindowControls => Some(OttoSettingKind::WindowControls),
            Self::Genie => Some(OttoSettingKind::Genie),
            Self::Media => Some(OttoSettingKind::MediaModule),
            Self::Focus => Some(OttoSettingKind::FocusModule),
            Self::Clipboard => Some(OttoSettingKind::ClipboardHistory),
            Self::Pocket => Some(OttoSettingKind::DropShelf),
            Self::QuickCapture => Some(OttoSettingKind::QuickCapture),
            Self::BackgroundCleanup => Some(OttoSettingKind::BackgroundCleanup),
            Self::Spotlight | Self::ControlCenter => None,
        }
    }

    #[must_use]
    pub const fn page(self) -> SettingsPage {
        match self {
            Self::TopBar => SettingsPage::TopBar,
            Self::Notch | Self::Media | Self::Clipboard | Self::Pocket | Self::QuickCapture => {
                SettingsPage::Notch
            }
            Self::Focus => SettingsPage::Focus,
            Self::Dock => SettingsPage::Dock,
            Self::WindowControls | Self::Genie => SettingsPage::WindowControls,
            Self::Spotlight => SettingsPage::Spotlight,
            Self::ControlCenter => SettingsPage::ControlCenter,
            Self::BackgroundCleanup | Self::Finder => SettingsPage::General,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleStatus {
    Off,
    Starting,
    Running,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModuleRow {
    pub id: ModuleId,
    pub enabled: bool,
    pub status: ModuleStatus,
    pub paused_by: Option<ModuleId>,
}

impl ModuleRow {
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.status, ModuleStatus::Running)
    }

    #[must_use]
    pub const fn can_toggle(&self) -> bool {
        self.id.setting_kind().is_some()
    }
}

#[must_use]
pub fn module_enabled(snapshot: &OttoSettingsSnapshot, id: ModuleId) -> bool {
    id.setting_kind()
        .is_none_or(|kind| snapshot.is_enabled(kind))
}

#[must_use]
pub fn resolve_modules(
    snapshot: &OttoSettingsSnapshot,
    host_status: impl Fn(ModuleId) -> Option<ModuleStatus>,
) -> Vec<ModuleRow> {
    ModuleId::ALL
        .into_iter()
        .map(|id| {
            let enabled = module_enabled(snapshot, id);
            let paused_by = id
                .parent()
                .filter(|parent| !module_enabled(snapshot, *parent));
            let status = if !enabled || paused_by.is_some() {
                ModuleStatus::Off
            } else {
                host_status(id).unwrap_or(ModuleStatus::Running)
            };
            ModuleRow {
                id,
                enabled,
                status,
                paused_by,
            }
        })
        .collect()
}

#[must_use]
pub fn modules_running(rows: &[ModuleRow]) -> usize {
    rows.iter().filter(|row| row.is_running()).count()
}

impl ModuleStatus {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Failed => "Failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsMetrics {
    pub sidebar_width: i32,
    pub rail_width: i32,
    pub rail_below_width: i32,
    pub titlebar_height: i32,
    pub sidebar_padding: i32,
    pub sidebar_item_height: i32,
    pub sidebar_group_height: i32,
    pub content_padding: i32,
    pub content_max_width: i32,
    pub header_height: i32,
    pub default_width: i32,
    pub default_height: i32,
    pub min_width: i32,
    pub min_height: i32,
}

impl SettingsMetrics {
    pub const DEFAULT: Self = Self {
        sidebar_width: 240,
        rail_width: 60,
        rail_below_width: 760,
        titlebar_height: 36,
        sidebar_padding: 10,
        sidebar_item_height: 32,
        sidebar_group_height: 28,
        content_padding: 24,
        content_max_width: 680,
        header_height: 52,
        default_width: 920,
        default_height: 620,
        min_width: 720,
        min_height: 480,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsWindowControlSlot {
    pub action: WindowControlAction,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsPageSlot {
    pub page: SettingsPage,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsGroupHeading {
    pub group: SettingsGroup,
    pub rect: Rect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsLayout {
    pub sidebar: Rect,
    pub sidebar_titlebar: Rect,
    pub window_controls_group: Rect,
    pub window_controls: [SettingsWindowControlSlot; 3],
    pub content: Rect,
    pub column: Rect,
    pub rail: bool,
    pub pages: Vec<SettingsPageSlot>,
    pub header: Rect,
    pub viewport: Rect,
    pub group_headings: Vec<SettingsGroupHeading>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsHit {
    WindowControl(WindowControlAction),
    Page(SettingsPage),
    Row {
        section: usize,
        row: usize,
    },
    Part {
        section: usize,
        row: usize,
        part: usize,
    },
    Content,
}

#[must_use]
pub fn compute_settings_layout(width: i32, height: i32, dpi: u32) -> SettingsLayout {
    compute_settings_layout_with(&SettingsMetrics::DEFAULT, width, height, dpi)
}

#[must_use]
pub fn compute_settings_layout_with(
    metrics: &SettingsMetrics,
    width: i32,
    height: i32,
    dpi: u32,
) -> SettingsLayout {
    let width = width.max(0);
    let height = height.max(0);
    let rail = width < scale(metrics.rail_below_width, dpi);
    let sidebar_width = scale(
        if rail {
            metrics.rail_width
        } else {
            metrics.sidebar_width
        },
        dpi,
    )
    .min(width);
    let sidebar = Rect {
        x: 0,
        y: 0,
        width: sidebar_width,
        height,
    };
    let content = Rect {
        x: sidebar_width,
        y: 0,
        width: width - sidebar_width,
        height,
    };
    let padding = scale(metrics.content_padding, dpi);
    let column_width = (content.width - padding * 2)
        .min(scale(metrics.content_max_width, dpi))
        .max(0);
    let header = Rect {
        x: content.x,
        y: 0,
        width: content.width,
        height: scale(metrics.header_height, dpi).min(height),
    };
    let column = Rect {
        x: content.x + (content.width - column_width) / 2,
        y: header.bottom(),
        width: column_width,
        height: (height - header.bottom()).max(0),
    };

    let side_pad = scale(metrics.sidebar_padding, dpi);
    let titlebar_height = scale(metrics.titlebar_height, dpi).min(height);
    let sidebar_titlebar = Rect {
        x: 0,
        y: 0,
        width: sidebar_width,
        height: titlebar_height,
    };
    let (window_controls_group, window_controls) =
        compute_settings_window_controls(sidebar_titlebar, rail, dpi);
    let item_height = scale(metrics.sidebar_item_height, dpi);
    let group_height = if rail {
        side_pad
    } else {
        scale(metrics.sidebar_group_height, dpi)
    };
    let mut pages = Vec::with_capacity(SETTINGS_PAGE_COUNT);
    let mut group_headings = Vec::new();
    let mut y = titlebar_height + scale(2, dpi);
    let mut last_group = SettingsGroup::Home;
    for page in SettingsPage::ALL {
        if page.group() != last_group {
            last_group = page.group();
            if !rail {
                group_headings.push(SettingsGroupHeading {
                    group: last_group,
                    rect: Rect {
                        x: side_pad,
                        y,
                        width: (sidebar_width - side_pad * 2).max(0),
                        height: group_height,
                    },
                });
            }
            y += group_height;
        }
        pages.push(SettingsPageSlot {
            page,
            rect: Rect {
                x: side_pad,
                y,
                width: (sidebar_width - side_pad * 2).max(0),
                height: item_height,
            },
        });
        y += item_height;
    }

    SettingsLayout {
        sidebar,
        sidebar_titlebar,
        window_controls_group,
        window_controls,
        content,
        column,
        rail,
        pages,
        header,
        viewport: column,
        group_headings,
    }
}

fn compute_settings_window_controls(
    titlebar: Rect,
    rail: bool,
    dpi: u32,
) -> (Rect, [SettingsWindowControlSlot; 3]) {
    const ACTIONS: [WindowControlAction; 3] = [
        WindowControlAction::Close,
        WindowControlAction::Minimize,
        WindowControlAction::ToggleMaximize,
    ];
    if titlebar.is_empty() {
        return (
            Rect::default(),
            ACTIONS.map(|action| SettingsWindowControlSlot {
                action,
                rect: Rect::default(),
            }),
        );
    }
    let cell_w = scale(if rail { 16 } else { 20 }, dpi).max(1);
    let group_w = (cell_w * 3).min(titlebar.width);
    let start_x = if rail {
        (titlebar.width - group_w) / 2
    } else {
        scale(10, dpi).min((titlebar.width - group_w).max(0))
    };
    let group = Rect {
        x: start_x,
        y: titlebar.y,
        width: group_w,
        height: titlebar.height,
    };
    let mut index = 0;
    let slots = ACTIONS.map(|action| {
        let x = start_x + index * cell_w;
        index += 1;
        let width = cell_w.min((titlebar.right() - x).max(0));
        SettingsWindowControlSlot {
            action,
            rect: Rect {
                x,
                y: titlebar.y,
                width,
                height: titlebar.height,
            },
        }
    });
    (group, slots)
}

impl SettingsLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32, content: &ContentLayout) -> Option<SettingsHit> {
        if let Some(slot) = self
            .window_controls
            .iter()
            .find(|slot| slot.rect.contains(x, y))
        {
            return Some(SettingsHit::WindowControl(slot.action));
        }
        if let Some(slot) = self.pages.iter().find(|slot| slot.rect.contains(x, y)) {
            return Some(SettingsHit::Page(slot.page));
        }
        if self.viewport.contains(x, y) {
            if let Some((section, row, part)) = content.part_at(x, y) {
                return Some(SettingsHit::Part { section, row, part });
            }
            if let Some((section, row)) = content.row_at(x, y) {
                return Some(SettingsHit::Row { section, row });
            }
        }
        self.content.contains(x, y).then_some(SettingsHit::Content)
    }

    #[must_use]
    pub const fn is_titlebar_drag(&self, x: i32, y: i32) -> bool {
        if self.window_controls_group.contains(x, y) {
            return false;
        }
        self.sidebar_titlebar.contains(x, y) || self.header.contains(x, y)
    }

    #[must_use]
    pub const fn max_scroll(&self, content: &ContentLayout) -> i32 {
        let overflow = content.total_height - self.viewport.height;
        if overflow > 0 { overflow } else { 0 }
    }

    #[must_use]
    pub fn page_rect(&self, page: SettingsPage) -> Option<Rect> {
        self.pages
            .iter()
            .find(|slot| slot.page == page)
            .map(|slot| slot.rect)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleId, ModuleStatus, SETTINGS_PAGE_COUNT, SettingsGroup, SettingsHit, SettingsPage,
        compute_settings_layout, modules_running, resolve_modules,
    };
    use crate::settings::model::{
        OttoSettingItem, OttoSettingKind, OttoSettingsSnapshot, PluginSettingsState,
        SystemSettingsState,
    };
    use crate::settings_content::ContentLayout;

    fn snapshot() -> OttoSettingsSnapshot {
        OttoSettingsSnapshot {
            plugins: PluginSettingsState {
                top_bar: true,
                notch: true,
                dock: true,
                window_controls: true,
                genie: true,
            },
            system: SystemSettingsState {
                media_module: true,
                focus_module: true,
                clipboard_history: true,
                drop_shelf: true,
                finder_enabled: true,
                ..SystemSettingsState::default()
            },
            ..OttoSettingsSnapshot::default()
        }
    }

    #[test]
    fn page_index_matches_position_in_all() {
        for (position, page) in SettingsPage::ALL.into_iter().enumerate() {
            assert_eq!(page.index(), position);
        }
    }

    #[test]
    fn every_setting_item_lands_on_a_page_that_lists_rows() {
        for item in OttoSettingItem::ALL {
            let page = SettingsPage::for_item(item);
            assert!(
                !matches!(
                    page,
                    SettingsPage::Overview
                        | SettingsPage::Modules
                        | SettingsPage::Spotlight
                        | SettingsPage::ControlCenter
                        | SettingsPage::About
                ),
                "{item:?} landed on {page:?}"
            );
        }
    }

    #[test]
    fn page_items_partition_all_items_when_everything_is_on() {
        let mut snapshot = snapshot();
        snapshot.dock.hide_taskbar = true;
        let listed: usize = SettingsPage::ALL
            .into_iter()
            .map(|page| snapshot.page_items(page).len())
            .sum();
        let available = OttoSettingItem::ALL
            .into_iter()
            .filter(|item| snapshot.is_item_available(*item))
            .count();
        assert_eq!(listed, available);
    }

    #[test]
    fn dependent_rows_disappear_with_their_master() {
        let mut snapshot = snapshot();
        assert!(
            snapshot
                .page_items(SettingsPage::Dock)
                .contains(&OttoSettingItem::DockIconSize)
        );
        snapshot.plugins.dock = false;
        assert_eq!(
            snapshot.page_items(SettingsPage::Dock),
            vec![OttoSettingItem::Toggle(OttoSettingKind::Dock)]
        );
    }

    #[test]
    fn neighbor_stops_at_both_ends() {
        assert_eq!(SettingsPage::Overview.neighbor(false), None);
        assert_eq!(
            SettingsPage::Overview.neighbor(true),
            Some(SettingsPage::Modules)
        );
        assert_eq!(SettingsPage::About.neighbor(true), None);
        assert_eq!(
            SettingsPage::About.neighbor(false),
            Some(SettingsPage::General)
        );
    }

    #[test]
    fn every_module_belongs_to_a_page_and_toggles_through_a_setting_when_it_can() {
        for id in ModuleId::ALL {
            let page = id.page();
            assert!(SettingsPage::ALL.contains(&page));
            if let Some(kind) = id.setting_kind() {
                assert!(
                    SettingsPage::for_kind(kind) == page
                        || matches!(id, ModuleId::Genie | ModuleId::BackgroundCleanup),
                    "{id:?}"
                );
            }
        }
    }

    #[test]
    fn modules_run_when_enabled_and_status_is_unknown() {
        let rows = resolve_modules(&snapshot(), |_| None);
        assert_eq!(rows.len(), ModuleId::ALL.len());
        let off: Vec<_> = rows
            .iter()
            .filter(|row| !row.is_running())
            .map(|row| row.id)
            .collect();
        assert_eq!(
            off,
            vec![ModuleId::QuickCapture, ModuleId::BackgroundCleanup]
        );
        assert_eq!(modules_running(&rows), ModuleId::ALL.len() - 2);
    }

    #[test]
    fn switching_the_notch_off_pauses_its_tools_without_touching_their_flags() {
        let mut snapshot = snapshot();
        snapshot.plugins.notch = false;
        let rows = resolve_modules(&snapshot, |_| Some(ModuleStatus::Running));
        let media = rows.iter().find(|row| row.id == ModuleId::Media).unwrap();
        assert!(media.enabled);
        assert_eq!(media.paused_by, Some(ModuleId::Notch));
        assert_eq!(media.status, ModuleStatus::Off);
        let dock = rows.iter().find(|row| row.id == ModuleId::Dock).unwrap();
        assert_eq!(dock.paused_by, None);
        assert!(dock.is_running());
    }

    #[test]
    fn host_failure_and_starting_states_pass_through_for_enabled_modules() {
        let rows = resolve_modules(&snapshot(), |id| match id {
            ModuleId::Dock => Some(ModuleStatus::Failed),
            ModuleId::Genie => Some(ModuleStatus::Starting),
            _ => None,
        });
        let status = |id| rows.iter().find(|row| row.id == id).unwrap().status;
        assert_eq!(status(ModuleId::Dock), ModuleStatus::Failed);
        assert_eq!(status(ModuleId::Genie), ModuleStatus::Starting);
    }

    #[test]
    fn a_disabled_module_reads_off_even_if_the_host_still_reports_running() {
        let mut snapshot = snapshot();
        snapshot.plugins.genie = false;
        let rows = resolve_modules(&snapshot, |_| Some(ModuleStatus::Running));
        let genie = rows.iter().find(|row| row.id == ModuleId::Genie).unwrap();
        assert_eq!(genie.status, ModuleStatus::Off);
        assert!(!genie.enabled);
    }

    #[test]
    fn spotlight_and_control_center_cannot_toggle_until_they_have_a_setting() {
        let rows = resolve_modules(&snapshot(), |_| None);
        for id in [ModuleId::Spotlight, ModuleId::ControlCenter] {
            let row = rows.iter().find(|row| row.id == id).unwrap();
            assert!(!row.can_toggle());
            assert!(row.is_running());
        }
    }

    #[test]
    fn wide_window_uses_the_full_sidebar_and_a_centered_capped_column() {
        let layout = compute_settings_layout(1200, 800, 96);
        assert!(!layout.rail);
        assert_eq!(layout.sidebar.width, 240);
        assert_eq!(layout.content.x, 240);
        assert_eq!(layout.column.width, 680);
        assert_eq!(
            layout.column.x - layout.content.x,
            layout.content.right() - layout.column.right()
        );
    }

    #[test]
    fn narrow_window_collapses_the_sidebar_to_a_rail() {
        let layout = compute_settings_layout(720, 480, 96);
        assert!(layout.rail);
        assert_eq!(layout.sidebar.width, 60);
        assert_eq!(layout.content.width, 720 - 60);
        assert!(layout.column.width <= layout.content.width);
    }

    #[test]
    fn sidebar_slots_are_ordered_disjoint_and_inside_the_sidebar() {
        for width in [720, 1000] {
            let layout = compute_settings_layout(width, 700, 96);
            assert_eq!(layout.pages.len(), SETTINGS_PAGE_COUNT);
            for pair in layout.pages.windows(2) {
                assert!(pair[0].rect.bottom() <= pair[1].rect.y);
            }
            for slot in &layout.pages {
                assert!(slot.rect.x >= layout.sidebar.x);
                assert!(slot.rect.right() <= layout.sidebar.right());
            }
        }
    }

    #[test]
    fn layout_scales_with_dpi() {
        let normal = compute_settings_layout(1200, 800, 96);
        let large = compute_settings_layout(1800, 1200, 144);
        assert_eq!(large.sidebar.width, normal.sidebar.width * 3 / 2);
        assert_eq!(large.header.height, normal.header.height * 3 / 2);
    }

    #[test]
    fn hit_test_resolves_pages_content_and_outside() {
        let layout = compute_settings_layout(1000, 700, 96);
        let empty = ContentLayout::default();
        let modules = layout.page_rect(SettingsPage::Modules).unwrap();
        assert_eq!(
            layout.hit_test(modules.x + 1, modules.y + 1, &empty),
            Some(SettingsHit::Page(SettingsPage::Modules))
        );
        assert_eq!(layout.hit_test(600, 20, &empty), Some(SettingsHit::Content));
        assert_eq!(layout.hit_test(2000, 10, &empty), None);
    }

    #[test]
    fn zero_sized_client_does_not_panic_or_go_negative() {
        let layout = compute_settings_layout(0, 0, 96);
        assert_eq!(layout.sidebar.width, 0);
        assert!(layout.column.width >= 0);
        assert_eq!(layout.hit_test(0, 0, &ContentLayout::default()), None);
    }

    #[test]
    fn group_headings_sit_above_their_first_page_and_vanish_in_the_rail() {
        let wide = compute_settings_layout(1000, 700, 96);
        let groups: Vec<_> = wide.group_headings.iter().map(|h| h.group).collect();
        assert_eq!(groups, vec![SettingsGroup::Surfaces, SettingsGroup::Otto]);
        let top_bar = wide.page_rect(SettingsPage::TopBar).unwrap();
        assert_eq!(wide.group_headings[0].rect.bottom(), top_bar.y);
        let rail = compute_settings_layout(720, 700, 96);
        assert!(rail.group_headings.is_empty());
    }

    #[test]
    fn settings_window_controls_hit_test_and_exclude_titlebar_drag_in_wide_and_rail_modes() {
        let empty = ContentLayout::default();
        for (width, height) in [(1000, 700), (720, 480)] {
            let layout = compute_settings_layout(width, height, 96);
            assert!(!layout.sidebar_titlebar.is_empty());
            assert_eq!(layout.window_controls.len(), 3);
            assert!(
                layout.pages.last().unwrap().rect.bottom() <= layout.sidebar.bottom(),
                "sidebar pages must fit inside {width}x{height}"
            );
            for slot in layout.window_controls {
                let cx = slot.rect.x + slot.rect.width / 2;
                let cy = slot.rect.y + slot.rect.height / 2;
                assert_eq!(
                    layout.hit_test(cx, cy, &empty),
                    Some(SettingsHit::WindowControl(slot.action))
                );
                assert!(!layout.is_titlebar_drag(cx, cy));
            }
            assert!(layout.is_titlebar_drag(
                layout.header.x + layout.header.width / 2,
                layout.header.y + 8
            ));
        }
    }
}
