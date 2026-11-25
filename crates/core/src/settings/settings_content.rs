use super::appearance_settings::{accent_for_part, build_appearance_page, style_for_part};
use super::focus_insights_settings::{
    FOCUS_HEATMAP_ROW_HEIGHT_POINTS, build_focus_page, focus_heatmap_parts, kpi_parts,
    weekly_chart_parts,
};
use super::focus_settings::{
    FilterChip, FocusAppFilter, FocusDisclosure, FocusPageState, FocusSettingsTab, FocusTextField,
    KpiItem, SegmentedOption, WeeklyChart,
};
use super::general_settings::build_general_page;
use super::model::{OttoSettingItem, OttoSettingKind, OttoSettingSection, OttoSettingsSnapshot};
use crate::appearance::theme::{AccentColor, AppearanceStyle, ThemeMode};
use crate::control_center::UpdateState;
use crate::layout::{Rect, scale};
use crate::notch::focus_heatmap::FocusHeatmapView;
use crate::notch::focus_mode::{CategoryColor, FocusRulePolicy};
use crate::runtime::process_hog::ProcessAuditFilter;
use crate::settings_window::{
    ModuleGroup, ModuleId, ModuleRow, ModuleStatus, SettingsPage, modules_running,
};

const ROW_HEIGHT_POINTS: i32 = 52;
const COMPACT_ROW_HEIGHT_POINTS: i32 = 44;
const SEGMENTED_ROW_HEIGHT_POINTS: i32 = 48;
const FILTER_CHIPS_ROW_HEIGHT_POINTS: i32 = 42;
const INPUT_ROW_HEIGHT_POINTS: i32 = 52;
const INPUT_ERROR_ROW_HEIGHT_POINTS: i32 = 70;
const KPI_ROW_HEIGHT_POINTS: i32 = 86;
const WEEKLY_CHART_ROW_HEIGHT_POINTS: i32 = 188;
const USAGE_BAR_ROW_HEIGHT_POINTS: i32 = 56;
const STYLE_PICKER_ROW_HEIGHT_POINTS: i32 = 114;
const ACCENT_SWATCHES_ROW_HEIGHT_POINTS: i32 = 54;
const SECTION_HEADER_POINTS: i32 = 28;
const SECTION_GAP_POINTS: i32 = 14;
const FOOTER_LINE_POINTS: i32 = 16;
const FOOTER_PAD_POINTS: i32 = 6;
const FOOTER_INSET_POINTS: i32 = 16;
const FOOTER_CHAR_POINTS: i32 = 6;
const HERO_HEIGHT_POINTS: i32 = 168;
const TOP_PAD_POINTS: i32 = 8;
const BOTTOM_PAD_POINTS: i32 = 28;
const ROW_INSET_POINTS: i32 = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsAction {
    Toggle(OttoSettingKind),
    CycleDockIconSize,
    CycleMenuBarHide,
    CycleBrandIcon,
    Open(SettingsPage),
    OpenLogs,
    CheckForUpdates,
    InstallUpdate,
    OpenReleases,
    ToggleFocusBlockNotifications,
    ToggleFocusAllowCalls,
    ToggleBlockedApp(String),
    ToggleBlockedWebsite(String),
    PickBlockedAppFile,
    ResetFocusActivityStats,
    SetFocusTab(FocusSettingsTab),
    SetFocusAppFilter(FocusAppFilter),
    FocusInput(FocusTextField),
    ClearFocusInput(FocusTextField),
    SubmitBlockedWebsite,
    ExpandFocusAllApps,
    SelectFocusChartDay(usize),
    ToggleFocusActivityDetails,
    SelectFocusCategory(String),
    SubmitFocusCategory,
    SubmitRenameFocusCategory,
    DeleteFocusCategory(String),
    CycleFocusCategoryColor,
    SetFocusCategoryColor(CategoryColor),
    AdjustFocusCategoryDuration(i32),
    CycleFocusCategoryBreaks,
    ToggleFocusCategoryPolicy,
    SetFocusCategoryPolicy(FocusRulePolicy),
    CycleFocusCategoryBackgroundPolicy,
    ApplyFocusCategorySuggestions,
    ToggleCategoryAllowedApp(String),
    ToggleCategoryBlockedApp(String),
    ToggleCategoryApp(String),
    ToggleCategoryWebsite(String),
    ToggleCategoryCloseApp(String),
    ToggleFocusDisclosure(FocusDisclosure),
    SetFocusHeatmapFilter(Option<String>),
    SelectFocusHeatmapCell(usize),
    SetAppearanceStyle(AppearanceStyle),
    SetAppearanceTheme(ThemeMode),
    SetAppearanceAccent(AccentColor),
    SetProcessAuditFilter(ProcessAuditFilter),
    RefreshProcessAudit,
    ExpandProcessAuditList,
    ToggleProcessThrottle(String),
    TerminateProcessGroup(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeTone {
    Good,
    Neutral,
    Warning,
    Bad,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsRowValue {
    Toggle(bool),
    Choice(&'static str),
    Detail(&'static str),
    Text(String),
    Keys(&'static str),
    Segmented {
        options: Vec<SegmentedOption>,
        selected: usize,
    },
    FilterChips {
        chips: Vec<FilterChip>,
        selected: usize,
    },
    TextInput {
        field: FocusTextField,
        text: String,
        placeholder: &'static str,
        button_label: Option<&'static str>,
        focused: bool,
        error: Option<String>,
    },
    KpiStrip([KpiItem; 3]),
    WeeklyChart(WeeklyChart),
    FocusHeatmap(FocusHeatmapView),
    UsageBar {
        duration: String,
        percent: u8,
        blocked: bool,
    },
    Disclosure {
        expanded: bool,
    },
    StylePicker {
        selected: AppearanceStyle,
        dark: bool,
        accent_rgb: u32,
    },
    AccentSwatches {
        selected: AccentColor,
        system_accent_rgb: Option<u32>,
    },
    ProcessAudit {
        status_label: &'static str,
        status_tone: BadgeTone,
        throttle_label: Option<&'static str>,
        throttle_action: Option<SettingsAction>,
        throttled: bool,
        terminate_label: Option<&'static str>,
        terminate_action: Option<SettingsAction>,
        recommended_end: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsRow {
    pub icon: Option<SettingsPage>,
    pub app_icon: Option<String>,
    pub detail_hint: Option<String>,
    pub label: String,
    pub subtitle: String,
    pub value: SettingsRowValue,
    pub badge: Option<(&'static str, BadgeTone)>,
    pub action: Option<SettingsAction>,
    pub muted: bool,
}

impl SettingsRow {
    pub(crate) fn new(
        label: impl Into<String>,
        subtitle: impl Into<String>,
        value: SettingsRowValue,
    ) -> Self {
        Self {
            icon: None,
            app_icon: None,
            detail_hint: None,
            label: label.into(),
            subtitle: subtitle.into(),
            value,
            badge: None,
            action: None,
            muted: false,
        }
    }

    pub(crate) fn with_icon(mut self, icon: SettingsPage) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn with_action(mut self, action: SettingsAction) -> Self {
        self.action = Some(action);
        self
    }

    const fn height_points(&self) -> i32 {
        match &self.value {
            SettingsRowValue::Segmented { .. } => SEGMENTED_ROW_HEIGHT_POINTS,
            SettingsRowValue::FilterChips { .. } => FILTER_CHIPS_ROW_HEIGHT_POINTS,
            SettingsRowValue::TextInput { error, .. } => {
                if error.is_some() {
                    INPUT_ERROR_ROW_HEIGHT_POINTS
                } else {
                    INPUT_ROW_HEIGHT_POINTS
                }
            }
            SettingsRowValue::KpiStrip(_) => KPI_ROW_HEIGHT_POINTS,
            SettingsRowValue::WeeklyChart(_) => WEEKLY_CHART_ROW_HEIGHT_POINTS,
            SettingsRowValue::FocusHeatmap(_) => FOCUS_HEATMAP_ROW_HEIGHT_POINTS,
            SettingsRowValue::UsageBar { .. } | SettingsRowValue::ProcessAudit { .. } => {
                USAGE_BAR_ROW_HEIGHT_POINTS
            }
            SettingsRowValue::StylePicker { .. } => STYLE_PICKER_ROW_HEIGHT_POINTS,
            SettingsRowValue::AccentSwatches { .. } => ACCENT_SWATCHES_ROW_HEIGHT_POINTS,
            _ => {
                if self.subtitle.is_empty() {
                    COMPACT_ROW_HEIGHT_POINTS
                } else {
                    ROW_HEIGHT_POINTS
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsSection {
    pub title: &'static str,
    pub footer: Option<&'static str>,
    pub rows: Vec<SettingsRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsHero {
    pub title: String,
    pub subtitle: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SettingsContent {
    pub hero: Option<SettingsHero>,
    pub sections: Vec<SettingsSection>,
}

impl SettingsContent {
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.sections.iter().map(|section| section.rows.len()).sum()
    }

    #[must_use]
    pub fn row(&self, section: usize, row: usize) -> Option<&SettingsRow> {
        self.sections.get(section)?.rows.get(row)
    }

    #[must_use]
    pub fn part_action(&self, section: usize, row: usize, part: usize) -> Option<SettingsAction> {
        let row = self.row(section, row)?;
        match &row.value {
            SettingsRowValue::Segmented { options, .. } => {
                options.get(part).map(|option| option.action.clone())
            }
            SettingsRowValue::FilterChips { chips, .. } => {
                chips.get(part).map(|chip| chip.action.clone())
            }
            SettingsRowValue::TextInput {
                field,
                text,
                button_label,
                ..
            } => match part {
                0 => Some(SettingsAction::FocusInput(*field)),
                1 if button_label.is_some() => Some(match field {
                    FocusTextField::CategoryName => SettingsAction::SubmitFocusCategory,
                    FocusTextField::CategoryRename => SettingsAction::SubmitRenameFocusCategory,
                    _ => SettingsAction::SubmitBlockedWebsite,
                }),
                1 if !text.is_empty() => Some(SettingsAction::ClearFocusInput(*field)),
                _ => None,
            },
            SettingsRowValue::WeeklyChart(chart) => {
                (part < chart.days.len()).then_some(SettingsAction::SelectFocusChartDay(part))
            }
            SettingsRowValue::FocusHeatmap(heatmap) => {
                (part < heatmap.cells.len()).then_some(SettingsAction::SelectFocusHeatmapCell(part))
            }
            SettingsRowValue::StylePicker { .. } => {
                style_for_part(part).map(SettingsAction::SetAppearanceStyle)
            }
            SettingsRowValue::AccentSwatches { .. } => {
                accent_for_part(part).map(SettingsAction::SetAppearanceAccent)
            }
            SettingsRowValue::ProcessAudit {
                throttle_action,
                terminate_action,
                ..
            } => match part {
                0 => throttle_action.clone().or_else(|| terminate_action.clone()),
                1 => terminate_action.clone(),
                _ => None,
            },
            SettingsRowValue::KpiStrip(_) => None,
            _ => row.action.clone(),
        }
    }
}

const fn status_tone(status: ModuleStatus) -> BadgeTone {
    match status {
        ModuleStatus::Running => BadgeTone::Good,
        ModuleStatus::Off => BadgeTone::Neutral,
        ModuleStatus::Starting => BadgeTone::Warning,
        ModuleStatus::Failed => BadgeTone::Bad,
    }
}

pub(crate) const fn module_badge(row: ModuleRow) -> (&'static str, BadgeTone) {
    if row.paused_by.is_some() {
        ("Paused", BadgeTone::Neutral)
    } else {
        (row.status.label(), status_tone(row.status))
    }
}

fn module_subtitle(row: ModuleRow) -> String {
    match row.paused_by {
        Some(parent) => format!("Needs {}", parent.label()),
        None => row.id.subtitle().to_owned(),
    }
}

fn module_toggle_row(row: ModuleRow) -> SettingsRow {
    let mut out = SettingsRow::new(
        row.id.label(),
        module_subtitle(row),
        SettingsRowValue::Toggle(row.enabled),
    )
    .with_icon(row.id.page());
    out.badge = Some(module_badge(row));
    out.action = row.id.setting_kind().map(SettingsAction::Toggle);
    out.muted = row.paused_by.is_some();
    out
}

fn module_link_row(row: ModuleRow) -> SettingsRow {
    let mut out = SettingsRow::new(
        row.id.label(),
        module_subtitle(row),
        SettingsRowValue::Detail(module_badge(row).0),
    )
    .with_icon(row.id.page())
    .with_action(SettingsAction::Open(row.id.page()));
    out.badge = None;
    out.muted = row.paused_by.is_some();
    out
}

pub(crate) fn setting_row(
    snapshot: &OttoSettingsSnapshot,
    item: OttoSettingItem,
    page: SettingsPage,
) -> SettingsRow {
    let (value, action) = match item {
        OttoSettingItem::Toggle(kind) => (
            SettingsRowValue::Toggle(snapshot.is_enabled(kind)),
            SettingsAction::Toggle(kind),
        ),
        OttoSettingItem::DockIconSize => (
            SettingsRowValue::Choice(snapshot.dock_size_label()),
            SettingsAction::CycleDockIconSize,
        ),
        OttoSettingItem::MenuBarHide => (
            SettingsRowValue::Choice(snapshot.menu_bar.hide_mode.label()),
            SettingsAction::CycleMenuBarHide,
        ),
        OttoSettingItem::BrandLogo => (
            SettingsRowValue::Choice(snapshot.system.brand_icon.label()),
            SettingsAction::CycleBrandIcon,
        ),
    };
    let row =
        SettingsRow::new(item.label(), snapshot.item_subtitle(item), value).with_action(action);
    if item.section() == OttoSettingSection::Main {
        row.with_icon(page)
    } else {
        row
    }
}

const fn section_footer(page: SettingsPage, section: OttoSettingSection) -> Option<&'static str> {
    match (page, section) {
        (_, OttoSettingSection::WindowsTaskbar) => {
            Some("Reserve Screen Space appears once the Windows taskbar is hidden.")
        }
        (_, OttoSettingSection::Visibility) => {
            Some("A hidden bar returns when the pointer reaches the top edge.")
        }
        (_, OttoSettingSection::ShowInBar) => {
            Some("Hidden items return as soon as you switch them back on.")
        }
        (_, OttoSettingSection::Live) => {
            Some("Online Lyrics fetches lyrics from the network when it is on.")
        }
        (_, OttoSettingSection::Pockets) => {
            Some("Clipboard history stays in memory and is never saved to disk.")
        }
        (SettingsPage::Appearance, OttoSettingSection::Main) => {
            Some("Liquid Glass falls back to solid surfaces when Windows transparency is off.")
        }
        (SettingsPage::Shortcuts, OttoSettingSection::Main) => Some(
            "Ctrl+Alt+, Ctrl+Alt+B and Ctrl+Alt+D keep working when this is off, so you can always get back here.",
        ),
        _ => None,
    }
}

fn setting_sections(snapshot: &OttoSettingsSnapshot, page: SettingsPage) -> Vec<SettingsSection> {
    let mut grouped: Vec<(OttoSettingSection, Vec<SettingsRow>)> = Vec::new();
    for item in snapshot.page_items(page) {
        let section = item.section();
        let row = setting_row(snapshot, item, page);
        match grouped.iter_mut().find(|(known, _)| *known == section) {
            Some((_, rows)) => rows.push(row),
            None => grouped.push((section, vec![row])),
        }
    }
    grouped
        .into_iter()
        .map(|(section, rows)| SettingsSection {
            title: section.title(),
            footer: section_footer(page, section),
            rows,
        })
        .collect()
}

fn module_status_row(
    modules: &[ModuleRow],
    id: ModuleId,
    page: SettingsPage,
) -> Option<SettingsRow> {
    let row = *modules.iter().find(|row| row.id == id)?;
    let mut out = SettingsRow::new(
        id.label(),
        id.subtitle(),
        SettingsRowValue::Text(module_badge(row).0.to_owned()),
    )
    .with_icon(page);
    out.badge = None;
    Some(out)
}

fn keys_row(label: &'static str, keys: &'static str) -> SettingsRow {
    SettingsRow::new(label, "", SettingsRowValue::Keys(keys))
}

fn overview(modules: &[ModuleRow], version: &str) -> SettingsContent {
    let group = |wanted: ModuleGroup| -> Vec<SettingsRow> {
        modules
            .iter()
            .filter(|row| row.id.group() == wanted)
            .copied()
            .map(module_link_row)
            .collect()
    };
    let mut system = group(ModuleGroup::System);
    system.push(
        SettingsRow::new(
            "Shortcuts",
            "Global keys and recovery",
            SettingsRowValue::Detail(""),
        )
        .with_icon(SettingsPage::Shortcuts)
        .with_action(SettingsAction::Open(SettingsPage::Shortcuts)),
    );
    SettingsContent {
        hero: Some(SettingsHero {
            title: String::from("Otto OS"),
            subtitle: format!(
                "Version {version} · {} of {} modules running",
                modules_running(modules),
                modules.len()
            ),
        }),
        sections: vec![
            SettingsSection {
                title: "Surfaces",
                footer: None,
                rows: group(ModuleGroup::Surfaces),
            },
            SettingsSection {
                title: "Notch tools",
                footer: None,
                rows: group(ModuleGroup::NotchTools),
            },
            SettingsSection {
                title: "System",
                footer: None,
                rows: system,
            },
        ],
    }
}

fn modules_page(modules: &[ModuleRow]) -> SettingsContent {
    let group = |wanted: ModuleGroup| -> Vec<SettingsRow> {
        modules
            .iter()
            .filter(|row| row.id.group() == wanted)
            .copied()
            .map(module_toggle_row)
            .collect()
    };
    SettingsContent {
        hero: None,
        sections: vec![
            SettingsSection {
                title: ModuleGroup::Surfaces.title(),
                footer: Some("A module you turn off stops right away. Its settings are kept."),
                rows: group(ModuleGroup::Surfaces),
            },
            SettingsSection {
                title: ModuleGroup::NotchTools.title(),
                footer: Some(
                    "Notch tools pause while the Notch is off. Their own switches stay as you set them.",
                ),
                rows: group(ModuleGroup::NotchTools),
            },
            SettingsSection {
                title: ModuleGroup::System.title(),
                footer: None,
                rows: group(ModuleGroup::System),
            },
        ],
    }
}

fn shortcuts_page(snapshot: &OttoSettingsSnapshot) -> SettingsContent {
    let mut sections = setting_sections(snapshot, SettingsPage::Shortcuts);
    sections.push(SettingsSection {
        title: "Keys",
        footer: Some("Ctrl+Alt+Q needs Quick Capture. Alt+N and Alt+P need the Notch."),
        rows: vec![
            keys_row("Open Otto Settings", "Ctrl+Alt+,"),
            keys_row("Open Control Center", "Ctrl+Alt+C"),
            keys_row("Open Spotlight", "Ctrl+Alt+Space"),
            keys_row("Toggle Top Bar", "Ctrl+Alt+B"),
            keys_row("Toggle Dock", "Ctrl+Alt+D"),
            keys_row("Quick Capture", "Ctrl+Alt+Q"),
            keys_row("Expand Notch", "Alt+N"),
            keys_row("Start or Pause Focus", "Alt+P"),
        ],
    });
    SettingsContent {
        hero: None,
        sections,
    }
}

fn surface_page(
    modules: &[ModuleRow],
    id: ModuleId,
    page: SettingsPage,
    keys_label: &'static str,
    keys: &'static str,
    footer: &'static str,
) -> SettingsContent {
    SettingsContent {
        hero: None,
        sections: vec![
            SettingsSection {
                title: "",
                footer: None,
                rows: module_status_row(modules, id, page).into_iter().collect(),
            },
            SettingsSection {
                title: "Shortcut",
                footer: Some(footer),
                rows: vec![keys_row(keys_label, keys)],
            },
        ],
    }
}

fn update_rows(version: &str, update: &UpdateState) -> Vec<SettingsRow> {
    match update {
        UpdateState::Idle => vec![
            SettingsRow::new(
                "Check for Updates",
                "Look for a newer release on GitHub",
                SettingsRowValue::Detail("Check"),
            )
            .with_action(SettingsAction::CheckForUpdates),
        ],
        UpdateState::Checking => vec![SettingsRow::new(
            "Checking for Updates...",
            "Contacting GitHub Releases",
            SettingsRowValue::Text(String::from("Checking...")),
        )],
        UpdateState::UpToDate => vec![
            SettingsRow::new(
                "Otto OS is Up to Date",
                format!("Version {version} is the latest release"),
                SettingsRowValue::Detail("Check Again"),
            )
            .with_action(SettingsAction::CheckForUpdates),
        ],
        UpdateState::NoReleases => vec![
            SettingsRow::new(
                "No Releases Yet",
                "No published releases found on GitHub",
                SettingsRowValue::Detail("Check Again"),
            )
            .with_action(SettingsAction::CheckForUpdates),
        ],
        UpdateState::Available(info) if info.can_auto_install() => vec![
            SettingsRow::new(
                format!("Update to v{}", info.version),
                "Download, verify SHA-256, and install automatically",
                SettingsRowValue::Detail("Install"),
            )
            .with_action(SettingsAction::InstallUpdate),
            SettingsRow::new(
                "Release Notes on GitHub",
                format!("View {} changes or download manually", info.tag),
                SettingsRowValue::Detail("Open"),
            )
            .with_action(SettingsAction::OpenReleases),
        ],
        UpdateState::Available(info) => vec![
            SettingsRow::new(
                format!("Update v{} Available", info.version),
                "Verified installer unavailable — download from GitHub",
                SettingsRowValue::Detail("Open"),
            )
            .with_action(SettingsAction::OpenReleases),
        ],
        UpdateState::Downloading {
            version: target,
            percent,
        } => {
            let progress = percent.map_or_else(|| String::from("..."), |pct| format!("{pct}%"));
            vec![SettingsRow::new(
                format!("Downloading v{target}..."),
                "Fetching verified installer from GitHub Releases",
                SettingsRowValue::Text(progress),
            )]
        }
        UpdateState::Verifying { version: target } => vec![SettingsRow::new(
            format!("Verifying v{target}..."),
            "Checking SHA-256 integrity before installing",
            SettingsRowValue::Text(String::from("Verifying")),
        )],
        UpdateState::Installing { version: target } => vec![SettingsRow::new(
            format!("Installing v{target}..."),
            "Otto OS will close, apply the patch, and relaunch",
            SettingsRowValue::Text(String::from("Installing")),
        )],
        UpdateState::Failed {
            message,
            retry_available: Some(info),
        } if info.can_auto_install() => vec![
            SettingsRow::new(
                format!("Update to v{} Failed", info.version),
                message.clone(),
                SettingsRowValue::Detail("Retry"),
            )
            .with_action(SettingsAction::InstallUpdate),
            SettingsRow::new(
                "Release Notes on GitHub",
                "Download the installer manually instead",
                SettingsRowValue::Detail("Open"),
            )
            .with_action(SettingsAction::OpenReleases),
        ],
        UpdateState::Failed { message, .. } => vec![
            SettingsRow::new(
                "Update Check Failed",
                message.clone(),
                SettingsRowValue::Detail("Retry"),
            )
            .with_action(SettingsAction::CheckForUpdates),
        ],
    }
}

fn about_page(version: &str, update: &UpdateState) -> SettingsContent {
    let hero_subtitle = match update {
        UpdateState::Available(info) => {
            format!("Version {version} · Update v{} available", info.version)
        }
        _ => format!("Version {version}"),
    };
    SettingsContent {
        hero: Some(SettingsHero {
            title: String::from("Otto OS"),
            subtitle: hero_subtitle,
        }),
        sections: vec![
            SettingsSection {
                title: "Software Update",
                footer: Some(
                    "Updates are downloaded only when you click Install and verified with SHA-256.",
                ),
                rows: update_rows(version, update),
            },
            SettingsSection {
                title: "Details",
                footer: None,
                rows: vec![
                    SettingsRow::new("Version", "", SettingsRowValue::Text(version.to_owned())),
                    SettingsRow::new(
                        "Settings folder",
                        "",
                        SettingsRowValue::Text(String::from("%LOCALAPPDATA%\\otto-os")),
                    ),
                ],
            },
            SettingsSection {
                title: "Support",
                footer: Some(
                    "If Otto exits unexpectedly, run it with --restore-shell to bring back the Windows taskbar.",
                ),
                rows: vec![
                    SettingsRow::new("Open Logs Folder", "", SettingsRowValue::Detail(""))
                        .with_action(SettingsAction::OpenLogs),
                ],
            },
        ],
    }
}

pub(crate) fn percent_of(value: u64, peak: u64) -> u8 {
    u8::try_from(value.min(peak).saturating_mul(100) / peak.max(1)).unwrap_or(100)
}

#[must_use]
pub fn build_content(
    snapshot: &OttoSettingsSnapshot,
    page: SettingsPage,
    modules: &[ModuleRow],
    version: &str,
    update: &UpdateState,
    focus: &FocusPageState,
) -> SettingsContent {
    match page {
        SettingsPage::Overview => overview(modules, version),
        SettingsPage::Modules => modules_page(modules),
        SettingsPage::Focus => build_focus_page(snapshot, modules, focus),
        SettingsPage::Appearance => build_appearance_page(snapshot),
        SettingsPage::Shortcuts => shortcuts_page(snapshot),
        SettingsPage::General => build_general_page(snapshot, modules),
        SettingsPage::About => about_page(version, update),
        SettingsPage::Spotlight => surface_page(
            modules,
            ModuleId::Spotlight,
            page,
            "Open Spotlight",
            "Ctrl+Alt+Space",
            "Search covers installed apps only, not files or the web.",
        ),
        SettingsPage::ControlCenter => surface_page(
            modules,
            ModuleId::ControlCenter,
            page,
            "Open Control Center",
            "Ctrl+Alt+C",
            "Wi-Fi, Bluetooth, and Focus open Windows Settings.",
        ),
        _ => SettingsContent {
            hero: None,
            sections: setting_sections(snapshot, page),
        },
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CardLayout {
    pub header: Rect,
    pub card: Rect,
    pub rows: Vec<Rect>,
    pub row_parts: Vec<Vec<Rect>>,
    pub footer: Rect,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentLayout {
    pub hero: Rect,
    pub cards: Vec<CardLayout>,
    pub total_height: i32,
}

impl ContentLayout {
    #[must_use]
    pub fn row_at(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        self.cards.iter().enumerate().find_map(|(section, card)| {
            card.rows
                .iter()
                .position(|rect| rect.contains(x, y))
                .map(|row| (section, row))
        })
    }

    #[must_use]
    pub fn part_at(&self, x: i32, y: i32) -> Option<(usize, usize, usize)> {
        self.cards.iter().enumerate().find_map(|(section, card)| {
            card.row_parts.iter().enumerate().find_map(|(row, parts)| {
                parts
                    .iter()
                    .position(|rect| rect.contains(x, y))
                    .map(|part| (section, row, part))
            })
        })
    }
}

fn segmented_parts(rect: Rect, count: usize, inner_x: i32, inner_w: i32, dpi: u32) -> Vec<Rect> {
    if count == 0 || inner_w <= 0 {
        return Vec::new();
    }
    let track_pad = scale(3, dpi);
    let gap = scale(4, dpi);
    let y = rect.y + scale(7, dpi);
    let h = (rect.height - scale(14, dpi)).max(scale(26, dpi));
    let usable = (inner_w - track_pad * 2 - gap * (count as i32 - 1)).max(count as i32);
    let seg_w = usable / count as i32;
    (0..count)
        .map(|index| {
            let x = inner_x + track_pad + index as i32 * (seg_w + gap);
            let width = if index + 1 == count {
                (inner_x + inner_w - track_pad - x).max(seg_w)
            } else {
                seg_w
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

fn filter_chip_parts(
    rect: Rect,
    chips: &[FilterChip],
    inner_x: i32,
    inner_w: i32,
    dpi: u32,
) -> Vec<Rect> {
    let count = chips.len();
    if count == 0 || inner_w <= 0 {
        return Vec::new();
    }
    let gap = scale(8, dpi);
    let y = rect.y + scale(6, dpi);
    let h = (rect.height - scale(12, dpi)).max(scale(26, dpi));
    let desired: Vec<i32> = chips
        .iter()
        .map(|chip| {
            let chars = i32::try_from(chip.label.chars().count()).unwrap_or(8);
            scale((chars * 7 + 24).max(70), dpi)
        })
        .collect();
    let total_gaps = gap * (count as i32 - 1);
    let total_desired: i32 = desired.iter().sum::<i32>() + total_gaps;
    let mut x = inner_x;
    let mut out = Vec::with_capacity(count);
    for (index, width) in desired.into_iter().enumerate() {
        let final_w = if total_desired > inner_w && total_desired > total_gaps {
            let available = (inner_w - total_gaps).max(count as i32);
            (width * available / (total_desired - total_gaps).max(1)).max(scale(44, dpi))
        } else {
            width
        };
        let clamped = final_w.min((inner_x + inner_w - x).max(0));
        out.push(Rect {
            x,
            y,
            width: clamped,
            height: h,
        });
        x += clamped;
        if index + 1 < count {
            x += gap;
        }
    }
    out
}

fn text_input_parts(
    rect: Rect,
    text: &str,
    button_label: Option<&str>,
    inner_x: i32,
    inner_w: i32,
    dpi: u32,
) -> Vec<Rect> {
    if inner_w <= 0 {
        return Vec::new();
    }
    let y = rect.y + scale(8, dpi);
    let h = scale(36, dpi);
    if let Some(label) = button_label {
        let chars = i32::try_from(label.chars().count()).unwrap_or(3);
        let btn_w = scale((chars * 9 + 30).max(64), dpi).min(inner_w / 3);
        let gap = scale(8, dpi);
        let input_w = (inner_w - btn_w - gap).max(0);
        vec![
            Rect {
                x: inner_x,
                y,
                width: input_w,
                height: h,
            },
            Rect {
                x: inner_x + input_w + gap,
                y,
                width: btn_w,
                height: h,
            },
        ]
    } else if !text.is_empty() {
        let clear_size = scale(28, dpi).min(h);
        let clear_pad = scale(4, dpi);
        let clear_x = (inner_x + inner_w - clear_size - clear_pad).max(inner_x);
        let input_w = (clear_x - inner_x).max(0);
        vec![
            Rect {
                x: inner_x,
                y,
                width: input_w,
                height: h,
            },
            Rect {
                x: clear_x,
                y: y + (h - clear_size) / 2,
                width: clear_size,
                height: clear_size,
            },
        ]
    } else {
        vec![Rect {
            x: inner_x,
            y,
            width: inner_w,
            height: h,
        }]
    }
}

fn style_picker_parts(rect: Rect, inner_x: i32, inner_w: i32, dpi: u32) -> Vec<Rect> {
    let count = AppearanceStyle::ALL.len();
    if inner_w <= 0 {
        return Vec::new();
    }
    let gap = scale(10, dpi);
    let pad_y = scale(12, dpi);
    let h = (rect.height - pad_y * 2).max(scale(60, dpi));
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
                y: rect.y + pad_y,
                width,
                height: h,
            }
        })
        .collect()
}

fn accent_swatch_parts(rect: Rect, inner_x: i32, inner_w: i32, dpi: u32) -> Vec<Rect> {
    let count = AccentColor::ALL.len();
    if inner_w <= 0 {
        return Vec::new();
    }
    let label_reserve = scale(84, dpi);
    let usable_w = (inner_w - label_reserve).max(scale(140, dpi)).min(inner_w);
    let slot = scale(32, dpi).min((usable_w / count as i32).max(scale(20, dpi)));
    let gap =
        scale(8, dpi).min(((usable_w - slot * count as i32).max(0)) / (count as i32 - 1).max(1));
    let y = rect.y + (rect.height - slot) / 2;
    (0..count)
        .map(|index| Rect {
            x: inner_x + index as i32 * (slot + gap),
            y,
            width: slot,
            height: slot,
        })
        .collect()
}

fn process_audit_parts(
    rect: Rect,
    has_throttle: bool,
    has_terminate: bool,
    inner_x: i32,
    inner_w: i32,
    dpi: u32,
) -> Vec<Rect> {
    if inner_w <= 0 || (!has_throttle && !has_terminate) {
        return Vec::new();
    }
    let gap = scale(6, dpi);
    let btn_w = scale(72, dpi);
    let btn_h = scale(26, dpi);
    let y = rect.y + (rect.height - btn_h) / 2;
    let right = inner_x + inner_w;
    if has_throttle && has_terminate {
        let terminate_x = (right - btn_w).max(inner_x);
        let throttle_x = (terminate_x - gap - btn_w).max(inner_x);
        vec![
            Rect {
                x: throttle_x,
                y,
                width: btn_w,
                height: btn_h,
            },
            Rect {
                x: terminate_x,
                y,
                width: btn_w,
                height: btn_h,
            },
        ]
    } else {
        vec![Rect {
            x: (right - btn_w).max(inner_x),
            y,
            width: btn_w,
            height: btn_h,
        }]
    }
}

fn compute_row_parts(rect: Rect, value: &SettingsRowValue, dpi: u32) -> Vec<Rect> {
    let pad = scale(ROW_INSET_POINTS, dpi);
    let inner_x = rect.x + pad;
    let inner_w = (rect.width - pad * 2).max(0);
    match value {
        SettingsRowValue::Segmented { options, .. } => {
            segmented_parts(rect, options.len(), inner_x, inner_w, dpi)
        }
        SettingsRowValue::FilterChips { chips, .. } => {
            filter_chip_parts(rect, chips, inner_x, inner_w, dpi)
        }
        SettingsRowValue::TextInput {
            text, button_label, ..
        } => text_input_parts(rect, text, *button_label, inner_x, inner_w, dpi),
        SettingsRowValue::KpiStrip(_) => kpi_parts(rect, inner_x, inner_w, dpi),
        SettingsRowValue::WeeklyChart(chart) => {
            weekly_chart_parts(rect, chart.days.len(), inner_x, inner_w, dpi)
        }
        SettingsRowValue::FocusHeatmap(heatmap) => {
            focus_heatmap_parts(rect, heatmap.weeks, inner_x, inner_w, dpi)
        }
        SettingsRowValue::StylePicker { .. } => style_picker_parts(rect, inner_x, inner_w, dpi),
        SettingsRowValue::AccentSwatches { .. } => accent_swatch_parts(rect, inner_x, inner_w, dpi),
        SettingsRowValue::ProcessAudit {
            throttle_action,
            terminate_action,
            ..
        } => process_audit_parts(
            rect,
            throttle_action.is_some(),
            terminate_action.is_some(),
            inner_x,
            inner_w,
            dpi,
        ),
        _ => Vec::new(),
    }
}

fn footer_lines(text: &str, width: i32, dpi: u32) -> i32 {
    let per_line = (width / scale(FOOTER_CHAR_POINTS, dpi).max(1)).max(1) as usize;
    let chars = text.chars().count();
    chars.div_ceil(per_line).max(1) as i32
}

#[must_use]
pub fn compute_content_layout(
    viewport: Rect,
    content: &SettingsContent,
    dpi: u32,
    scroll: i32,
) -> ContentLayout {
    let top = viewport.y + scale(TOP_PAD_POINTS, dpi);
    let mut y = top - scroll;
    let gap = scale(SECTION_GAP_POINTS, dpi);
    let mut hero = Rect::default();
    if content.hero.is_some() {
        hero = Rect {
            x: viewport.x,
            y,
            width: viewport.width,
            height: scale(HERO_HEIGHT_POINTS, dpi),
        };
        y += hero.height + gap;
    }
    let mut cards = Vec::with_capacity(content.sections.len());
    for section in &content.sections {
        if section.rows.is_empty() {
            continue;
        }
        let mut header = Rect::default();
        if !section.title.is_empty() {
            header = Rect {
                x: viewport.x,
                y,
                width: viewport.width,
                height: scale(SECTION_HEADER_POINTS, dpi),
            };
            y += header.height;
        }
        let card_top = y;
        let mut rows = Vec::with_capacity(section.rows.len());
        let mut row_parts = Vec::with_capacity(section.rows.len());
        for row in &section.rows {
            let height = scale(row.height_points(), dpi);
            let rect = Rect {
                x: viewport.x,
                y,
                width: viewport.width,
                height,
            };
            row_parts.push(compute_row_parts(rect, &row.value, dpi));
            rows.push(rect);
            y += height;
        }
        let card = Rect {
            x: viewport.x,
            y: card_top,
            width: viewport.width,
            height: y - card_top,
        };
        let mut footer = Rect::default();
        if let Some(text) = section.footer {
            let inset = scale(FOOTER_INSET_POINTS, dpi);
            let width = (viewport.width - inset * 2).max(0);
            footer = Rect {
                x: viewport.x + inset,
                y: y + scale(FOOTER_PAD_POINTS, dpi),
                width,
                height: footer_lines(text, width, dpi) * scale(FOOTER_LINE_POINTS, dpi),
            };
            y = footer.bottom() + scale(FOOTER_PAD_POINTS, dpi);
        }
        y += gap;
        cards.push(CardLayout {
            header,
            card,
            rows,
            row_parts,
            footer,
        });
    }
    ContentLayout {
        hero,
        cards,
        total_height: (y + scroll) - viewport.y + scale(BOTTOM_PAD_POINTS, dpi),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SettingsAction, SettingsRowValue, build_content, compute_content_layout, percent_of,
    };
    use crate::dock::layout::DOCK_ICON_LARGE;
    use crate::layout::Rect;
    use crate::settings::model::{
        OttoSettingKind, OttoSettingsSnapshot, PluginSettingsState, SystemSettingsState,
    };
    use crate::settings_window::{ModuleId, SettingsPage, resolve_modules};

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

    fn content(snapshot: &OttoSettingsSnapshot, page: SettingsPage) -> super::SettingsContent {
        let modules = resolve_modules(snapshot, |_| None);
        build_content(
            snapshot,
            page,
            &modules,
            "1.0.0",
            &super::UpdateState::Idle,
            &super::FocusPageState::default(),
        )
    }

    #[test]
    fn no_page_is_empty_and_no_section_is_empty() {
        let snapshot = snapshot();
        for page in SettingsPage::ALL {
            let content = content(&snapshot, page);
            assert!(content.row_count() > 0, "{page:?} has no rows");
            assert!(
                content
                    .sections
                    .iter()
                    .all(|section| !section.rows.is_empty()),
                "{page:?} has an empty section"
            );
        }
    }

    #[test]
    fn setting_pages_list_exactly_their_available_items() {
        let mut snapshot = snapshot();
        snapshot.dock.hide_taskbar = true;
        for page in [
            SettingsPage::TopBar,
            SettingsPage::Notch,
            SettingsPage::Dock,
            SettingsPage::WindowControls,
        ] {
            let content = content(&snapshot, page);
            assert_eq!(
                content.row_count(),
                snapshot.page_items(page).len(),
                "{page:?}"
            );
        }
    }

    #[test]
    fn dock_page_groups_rows_under_their_section_titles_with_a_master_first() {
        let mut snapshot = snapshot();
        snapshot.dock.hide_taskbar = true;
        snapshot.dock.icon_size = DOCK_ICON_LARGE;
        let content = content(&snapshot, SettingsPage::Dock);
        let titles: Vec<_> = content
            .sections
            .iter()
            .map(|section| section.title)
            .collect();
        assert_eq!(
            titles,
            vec!["", "Behavior", "Show on Dock", "Windows Taskbar"]
        );
        let master = &content.sections[0].rows[0];
        assert_eq!(
            master.action,
            Some(SettingsAction::Toggle(OttoSettingKind::Dock))
        );
        assert_eq!(master.icon, Some(SettingsPage::Dock));
        let size = &content.sections[1].rows[0];
        assert_eq!(size.value, SettingsRowValue::Choice("Large"));
        assert!(content.sections[3].footer.is_some());
    }

    #[test]
    fn overview_has_a_hero_and_links_every_module_to_its_page() {
        let snapshot = snapshot();
        let content = content(&snapshot, SettingsPage::Overview);
        let hero = content.hero.as_ref().unwrap();
        assert_eq!(hero.title, "Otto OS");
        assert!(hero.subtitle.contains("12 of 14 modules running"));
        let links = content
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .filter(|row| matches!(row.action, Some(SettingsAction::Open(_))))
            .count();
        assert_eq!(links, ModuleId::ALL.len() + 1);
    }

    #[test]
    fn modules_page_toggles_through_settings_and_shows_why_a_child_is_paused() {
        let mut snapshot = snapshot();
        snapshot.plugins.notch = false;
        let content = content(&snapshot, SettingsPage::Modules);
        let media = content
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .find(|row| row.label == "Now Playing")
            .unwrap();
        assert_eq!(media.subtitle, "Needs Notch");
        assert_eq!(media.badge.map(|badge| badge.0), Some("Paused"));
        assert!(media.muted);
        assert_eq!(
            media.action,
            Some(SettingsAction::Toggle(OttoSettingKind::MediaModule))
        );
        let spotlight = content
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .find(|row| row.label == "Spotlight")
            .unwrap();
        assert_eq!(spotlight.action, None);
    }

    #[test]
    fn shortcuts_page_pairs_the_master_switch_with_every_key() {
        let content = content(&snapshot(), SettingsPage::Shortcuts);
        assert_eq!(content.sections[0].rows.len(), 1);
        let keys = content.sections.last().unwrap();
        assert_eq!(keys.rows.len(), 8);
        assert!(
            keys.rows
                .iter()
                .all(|row| matches!(row.value, SettingsRowValue::Keys(_)))
        );
    }

    #[test]
    fn about_offers_the_logs_folder() {
        let content = content(&snapshot(), SettingsPage::About);
        assert!(content.hero.is_some());
        assert!(
            content
                .sections
                .iter()
                .flat_map(|section| &section.rows)
                .any(|row| row.action == Some(SettingsAction::OpenLogs))
        );
    }

    fn update_actions(state: &super::UpdateState) -> Vec<Option<SettingsAction>> {
        let snapshot = snapshot();
        let modules = resolve_modules(&snapshot, |_| None);
        build_content(
            &snapshot,
            SettingsPage::About,
            &modules,
            "1.0.0",
            state,
            &super::FocusPageState::default(),
        )
        .sections
        .into_iter()
        .find(|section| section.title == "Software Update")
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.action)
        .collect()
    }

    #[test]
    fn the_update_section_covers_check_install_progress_and_retry_states() {
        use crate::control_center::{ReleaseUpdateInfo, UpdateState};

        assert_eq!(
            update_actions(&UpdateState::Idle),
            vec![Some(SettingsAction::CheckForUpdates)]
        );
        assert_eq!(update_actions(&UpdateState::Checking), vec![None]);
        assert_eq!(
            update_actions(&UpdateState::UpToDate),
            vec![Some(SettingsAction::CheckForUpdates)]
        );
        let installable = ReleaseUpdateInfo {
            version: String::from("1.2.1"),
            tag: String::from("v1.2.1"),
            installer_name: String::from("otto-os-setup-1.2.1-x64.exe"),
            installer_url: String::from(
                "https://github.com/howlil/otto-os/releases/download/v1.2.1/otto-os-setup-1.2.1-x64.exe",
            ),
            checksums_url: Some(String::from(
                "https://github.com/howlil/otto-os/releases/download/v1.2.1/SHA256SUMS.txt",
            )),
            release_url: String::from("https://github.com/howlil/otto-os/releases/tag/v1.2.1"),
        };
        assert_eq!(
            update_actions(&UpdateState::Available(installable.clone())),
            vec![
                Some(SettingsAction::InstallUpdate),
                Some(SettingsAction::OpenReleases),
            ]
        );
        let without_checksum = ReleaseUpdateInfo {
            checksums_url: None,
            ..installable.clone()
        };
        assert_eq!(
            update_actions(&UpdateState::Available(without_checksum)),
            vec![Some(SettingsAction::OpenReleases)]
        );
        assert_eq!(
            update_actions(&UpdateState::Downloading {
                version: String::from("1.2.1"),
                percent: Some(45),
            }),
            vec![None]
        );
        assert_eq!(
            update_actions(&UpdateState::Verifying {
                version: String::from("1.2.1"),
            }),
            vec![None]
        );
        assert_eq!(
            update_actions(&UpdateState::Installing {
                version: String::from("1.2.1"),
            }),
            vec![None]
        );
        assert_eq!(
            update_actions(&UpdateState::Failed {
                message: String::from("Download failed"),
                retry_available: Some(installable),
            }),
            vec![
                Some(SettingsAction::InstallUpdate),
                Some(SettingsAction::OpenReleases),
            ]
        );
        assert_eq!(
            update_actions(&UpdateState::Failed {
                message: String::from("Could not reach GitHub Releases"),
                retry_available: None,
            }),
            vec![Some(SettingsAction::CheckForUpdates)]
        );
    }

    fn viewport() -> Rect {
        Rect {
            x: 300,
            y: 52,
            width: 640,
            height: 500,
        }
    }

    #[test]
    fn cards_stack_without_overlap_and_rows_fill_their_card() {
        let content = content(&snapshot(), SettingsPage::Modules);
        let layout = compute_content_layout(viewport(), &content, 96, 0);
        assert_eq!(layout.cards.len(), content.sections.len());
        for pair in layout.cards.windows(2) {
            assert!(pair[0].card.bottom() <= pair[1].card.y);
        }
        for card in &layout.cards {
            let rows_height: i32 = card.rows.iter().map(|rect| rect.height).sum();
            assert_eq!(rows_height, card.card.height);
            assert_eq!(card.rows[0].y, card.card.y);
        }
    }

    #[test]
    fn scrolling_moves_everything_and_keeps_the_total_height() {
        let content = content(&snapshot(), SettingsPage::Modules);
        let rest = compute_content_layout(viewport(), &content, 96, 0);
        let scrolled = compute_content_layout(viewport(), &content, 96, 120);
        assert_eq!(rest.total_height, scrolled.total_height);
        assert_eq!(rest.cards[0].rows[0].y - scrolled.cards[0].rows[0].y, 120);
    }

    #[test]
    fn row_hit_testing_finds_the_row_under_the_point() {
        let content = content(&snapshot(), SettingsPage::Modules);
        let layout = compute_content_layout(viewport(), &content, 96, 0);
        let rect = layout.cards[1].rows[2];
        assert_eq!(layout.row_at(rect.x + 4, rect.y + 4), Some((1, 2)));
        assert_eq!(layout.row_at(0, 0), None);
    }

    #[test]
    fn footers_grow_taller_when_the_column_is_narrow() {
        let content = content(&snapshot(), SettingsPage::Modules);
        let wide = compute_content_layout(viewport(), &content, 96, 0);
        let narrow = compute_content_layout(
            Rect {
                width: 260,
                ..viewport()
            },
            &content,
            96,
            0,
        );
        assert!(narrow.cards[0].footer.height > wide.cards[0].footer.height);
    }

    #[test]
    fn hero_takes_space_above_the_first_card() {
        let content = content(&snapshot(), SettingsPage::Overview);
        let layout = compute_content_layout(viewport(), &content, 96, 0);
        assert!(layout.hero.height > 0);
        assert!(layout.cards[0].header.y >= layout.hero.bottom());
    }

    #[test]
    fn focus_page_supports_tabs_hit_tested_parts_and_activity_chart() {
        use super::{FocusPageState, FocusSettingsTab};
        use crate::notch::focus_activity::{DayKey, inspect_browser_context};

        let mut snap = snapshot();
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
            .record_sample(day, 6_000, 0, true, 4, 25);
        assert!(snap.focus_activity.blocklist.add_website("reddit.com"));

        let modules = resolve_modules(&snap, |_| None);
        let block_page = build_content(
            &snap,
            SettingsPage::Focus,
            &modules,
            "1.0.0",
            &super::UpdateState::Idle,
            &FocusPageState::default(),
        );
        let block_layout = compute_content_layout(viewport(), &block_page, 96, 0);
        let tab_parts = &block_layout.cards[0].row_parts[1];
        assert_eq!(tab_parts.len(), 2);
        let insights_tab = tab_parts[1];
        assert_eq!(
            block_layout.part_at(insights_tab.x + 4, insights_tab.y + 4),
            Some((0, 1, 1))
        );
        assert_eq!(
            block_page.part_action(0, 1, 0),
            Some(SettingsAction::SetFocusTab(FocusSettingsTab::Modes))
        );
        assert_eq!(
            block_page.part_action(0, 1, 1),
            Some(SettingsAction::SetFocusTab(FocusSettingsTab::Insights))
        );

        let activity_state = FocusPageState {
            tab: FocusSettingsTab::Insights,
            details_expanded: true,
            ..FocusPageState::default()
        };
        let activity_page = build_content(
            &snap,
            SettingsPage::Focus,
            &modules,
            "1.0.0",
            &super::UpdateState::Idle,
            &activity_state,
        );
        let activity_layout = compute_content_layout(viewport(), &activity_page, 96, 0);
        let chart_card = activity_layout
            .cards
            .iter()
            .find(|card| card.row_parts.first().is_some_and(|p| p.len() == 7))
            .expect("7-day chart card");
        assert_eq!(chart_card.row_parts[0].len(), 7);
        let labels: Vec<&str> = activity_page
            .sections
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.label.as_str()))
            .collect();
        assert!(labels.contains(&"Google Chrome"));
        assert!(labels.contains(&"google.com"));
        assert!(labels.contains(&"otto focus"));
        assert!(labels.contains(&"Reset Activity & Screen Time Stats"));
        let clicks_row = activity_page
            .sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .find(|row| row.label == "Clicks & keys today")
            .expect("clicks row");
        assert_eq!(
            clicks_row.value,
            SettingsRowValue::Text(String::from("16 clicks · 85 keys"))
        );
    }

    #[test]
    fn day_bars_scale_against_the_busiest_day() {
        assert_eq!(percent_of(0, 200), 0);
        assert_eq!(percent_of(50, 200), 25);
        assert_eq!(percent_of(400, 200), 100);
    }

    #[test]
    fn general_page_supports_audit_filters_and_action_parts() {
        use crate::runtime::process_hog::{
            ProcessAuditFilter, ProcessThrottleKind, RawProcessAuditSample, summarize_process_audit,
        };
        use crate::settings::general_settings::GeneralPageState;

        let mut snap = snapshot();
        snap.system.background_cleanup = true;
        snap.process_audit = GeneralPageState {
            filter: ProcessAuditFilter::Background,
            audit: summarize_process_audit(&[
                RawProcessAuditSample {
                    pid: 42,
                    exe_name: String::from("discord.exe"),
                    exe_path: Some(String::from(r"C:\Apps\Discord.exe")),
                    memory_bytes: 180 * 1024 * 1024,
                    cpu_tenths_percent: 12,
                    session_id: 1,
                    is_critical: false,
                    controllable: true,
                    terminatable: true,
                    is_foreground: false,
                    has_visible_window: false,
                    throttle_kind: ProcessThrottleKind::None,
                },
                RawProcessAuditSample {
                    pid: 55,
                    exe_name: String::from("svchost.exe"),
                    exe_path: Some(String::from(r"C:\Windows\System32\svchost.exe")),
                    memory_bytes: 200 * 1024 * 1024,
                    cpu_tenths_percent: 4,
                    session_id: 0,
                    is_critical: false,
                    controllable: false,
                    terminatable: false,
                    is_foreground: false,
                    has_visible_window: false,
                    throttle_kind: ProcessThrottleKind::None,
                },
            ]),
            ..GeneralPageState::default()
        };

        let modules = resolve_modules(&snap, |_| None);
        let page = build_content(
            &snap,
            SettingsPage::General,
            &modules,
            "1.0.0",
            &super::UpdateState::Idle,
            &super::FocusPageState::default(),
        );
        let layout = compute_content_layout(viewport(), &page, 96, 0);
        let audit_section = 2;
        let discord_row = page.sections[audit_section]
            .rows
            .iter()
            .position(|row| row.label == "Discord")
            .expect("discord row");
        let parts = &layout.cards[audit_section].row_parts[discord_row];
        assert_eq!(parts.len(), 2);
        assert_eq!(
            layout.part_at(parts[0].x + 2, parts[0].y + 2),
            Some((audit_section, discord_row, 0))
        );
        assert_eq!(
            page.part_action(audit_section, discord_row, 0),
            Some(SettingsAction::ToggleProcessThrottle(String::from(
                "discord.exe"
            )))
        );
        assert_eq!(
            page.part_action(audit_section, discord_row, 1),
            Some(SettingsAction::TerminateProcessGroup(String::from(
                "discord.exe"
            )))
        );
    }
}
