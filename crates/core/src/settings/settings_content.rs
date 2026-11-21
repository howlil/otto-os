use super::model::{OttoSettingItem, OttoSettingKind, OttoSettingSection, OttoSettingsSnapshot};
use crate::control_center::{UPDATE_CHECKING_LABEL, is_update_available_status};
use crate::dock::layout::{DOCK_ICON_LARGE, DOCK_ICON_MEDIUM, DOCK_ICON_SMALL};
use crate::layout::{Rect, scale};
use crate::notch::focus_activity::{
    PRESET_BLOCKED_APPS, PRESET_BLOCKED_WEBSITES, estimated_words, format_compact_count,
    format_screen_time, normalize_app_key,
};
use crate::settings_window::{
    ModuleGroup, ModuleId, ModuleRow, ModuleStatus, SettingsPage, modules_running,
};

const ROW_HEIGHT_POINTS: i32 = 52;
const COMPACT_ROW_HEIGHT_POINTS: i32 = 44;
const SECTION_HEADER_POINTS: i32 = 28;
const SECTION_GAP_POINTS: i32 = 14;
const FOOTER_LINE_POINTS: i32 = 16;
const FOOTER_PAD_POINTS: i32 = 6;
const FOOTER_INSET_POINTS: i32 = 16;
const FOOTER_CHAR_POINTS: i32 = 6;
const HERO_HEIGHT_POINTS: i32 = 168;
const TOP_PAD_POINTS: i32 = 8;
const BOTTOM_PAD_POINTS: i32 = 28;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsAction {
    Toggle(OttoSettingKind),
    CycleDockIconSize,
    CycleMenuBarHide,
    Open(SettingsPage),
    OpenLogs,
    CheckForUpdates,
    OpenReleases,
    ToggleFocusBlockNotifications,
    ToggleFocusAllowCalls,
    ToggleBlockedApp(String),
    ToggleBlockedWebsite(String),
    PickBlockedAppFile,
    AddBlockedWebsiteFromClipboard,
    ResetFocusActivityStats,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsRow {
    pub icon: Option<SettingsPage>,
    pub label: String,
    pub subtitle: String,
    pub value: SettingsRowValue,
    pub badge: Option<(&'static str, BadgeTone)>,
    pub action: Option<SettingsAction>,
    pub muted: bool,
}

impl SettingsRow {
    fn new(label: impl Into<String>, subtitle: impl Into<String>, value: SettingsRowValue) -> Self {
        Self {
            icon: None,
            label: label.into(),
            subtitle: subtitle.into(),
            value,
            badge: None,
            action: None,
            muted: false,
        }
    }

    fn with_icon(mut self, icon: SettingsPage) -> Self {
        self.icon = Some(icon);
        self
    }

    fn with_action(mut self, action: SettingsAction) -> Self {
        self.action = Some(action);
        self
    }

    const fn height_points(&self) -> i32 {
        if self.subtitle.is_empty() {
            COMPACT_ROW_HEIGHT_POINTS
        } else {
            ROW_HEIGHT_POINTS
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
}

const fn dock_size_label(size: i32) -> &'static str {
    match size {
        DOCK_ICON_SMALL => "Small",
        DOCK_ICON_MEDIUM => "Medium",
        DOCK_ICON_LARGE => "Large",
        _ => "Custom",
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

const fn module_badge(row: ModuleRow) -> (&'static str, BadgeTone) {
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

fn setting_row(
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
            SettingsRowValue::Choice(dock_size_label(snapshot.dock.icon_size)),
            SettingsAction::CycleDockIconSize,
        ),
        OttoSettingItem::MenuBarHide => (
            SettingsRowValue::Choice(snapshot.menu_bar.hide_mode.label()),
            SettingsAction::CycleMenuBarHide,
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
        (SettingsPage::General, OttoSettingSection::Main) => Some(
            "Background Cleanup slows apps that keep using the CPU in the background. It never closes them.",
        ),
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

fn update_row(update_status: Option<&str>) -> SettingsRow {
    let checking = update_status == Some(UPDATE_CHECKING_LABEL);
    let (subtitle, action) = if checking {
        (UPDATE_CHECKING_LABEL.to_owned(), None)
    } else if is_update_available_status(update_status) {
        (
            update_status.unwrap_or_default().to_owned(),
            Some(SettingsAction::OpenReleases),
        )
    } else {
        (
            update_status.map_or_else(|| String::from("Look for a newer release"), str::to_owned),
            Some(SettingsAction::CheckForUpdates),
        )
    };
    let mut row = SettingsRow::new("Check for Updates", subtitle, SettingsRowValue::Detail(""));
    row.action = action;
    row
}

fn about_page(version: &str, update_status: Option<&str>) -> SettingsContent {
    SettingsContent {
        hero: Some(SettingsHero {
            title: String::from("Otto OS"),
            subtitle: format!("Version {version}"),
        }),
        sections: vec![
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
                    update_row(update_status),
                    SettingsRow::new("Open Logs Folder", "", SettingsRowValue::Detail(""))
                        .with_action(SettingsAction::OpenLogs),
                ],
            },
        ],
    }
}

fn focus_summary_rows(activity: &crate::FocusActivityState) -> Vec<SettingsRow> {
    let today = &activity.today;
    let summary = activity.rolling_summary();
    vec![
        SettingsRow::new(
            "Screen Time Today",
            format!(
                "7-day total: {} across {} day(s)",
                format_screen_time(summary.total_active_ms),
                summary.days_counted
            ),
            SettingsRowValue::Text(format_screen_time(today.total_active_ms)),
        ),
        SettingsRow::new(
            "Focus Session Time",
            format!(
                "7-day Focus: {} · {} blocked today ({} in 7d)",
                format_screen_time(summary.focus_session_ms),
                today.blocked_attempts,
                summary.blocked_attempts
            ),
            SettingsRowValue::Text(format_screen_time(today.focus_session_ms)),
        ),
        SettingsRow::new(
            "Mouse Clicks",
            format!(
                "7-day total: {} clicks",
                format_compact_count(summary.total_clicks)
            ),
            SettingsRowValue::Text(format_compact_count(today.total_clicks)),
        ),
        SettingsRow::new(
            "Typing Activity",
            format!(
                "7-day total: {} keys (~{} words)",
                format_compact_count(summary.total_keystrokes),
                format_compact_count(estimated_words(summary.total_keystrokes))
            ),
            SettingsRowValue::Text(format!(
                "{} keys (~{}w)",
                format_compact_count(today.total_keystrokes),
                format_compact_count(estimated_words(today.total_keystrokes))
            )),
        ),
    ]
}

fn focus_apps_rows(activity: &crate::FocusActivityState) -> Vec<SettingsRow> {
    let mut rows: Vec<SettingsRow> = activity
        .today
        .sorted_apps()
        .into_iter()
        .take(10)
        .map(|app| {
            let blocked = activity
                .blocklist
                .is_app_blocked(&app.app_key, &app.display_name);
            let words = estimated_words(app.keystroke_count);
            let mut row = SettingsRow::new(
                app.display_name,
                format!(
                    "{} open · {}x opened · {} clicks · {} keys (~{}w)",
                    format_screen_time(app.active_ms),
                    app.open_count,
                    format_compact_count(app.click_count),
                    format_compact_count(app.keystroke_count),
                    format_compact_count(words)
                ),
                SettingsRowValue::Toggle(blocked),
            )
            .with_action(SettingsAction::ToggleBlockedApp(app.app_key));
            row.badge = Some(if blocked {
                ("Blocked", BadgeTone::Warning)
            } else {
                ("Allowed", BadgeTone::Good)
            });
            row
        })
        .collect();
    if rows.is_empty() {
        rows.push(SettingsRow::new(
            "No app activity recorded yet",
            "Use your apps normally to see hours, opens, clicks, and typing counts",
            SettingsRowValue::Text(String::from("Idle")),
        ));
    }
    rows
}

fn focus_sites_rows(activity: &crate::FocusActivityState) -> Vec<SettingsRow> {
    let mut rows: Vec<SettingsRow> = activity
        .today
        .sorted_websites()
        .into_iter()
        .take(10)
        .map(|site| {
            let blocked = activity.blocklist.is_website_blocked(&site.domain);
            let mut row = SettingsRow::new(
                site.domain.clone(),
                format!(
                    "{} active · {} visits",
                    format_screen_time(site.active_ms),
                    site.visit_count
                ),
                SettingsRowValue::Toggle(blocked),
            )
            .with_action(SettingsAction::ToggleBlockedWebsite(site.domain));
            row.badge = Some(if blocked {
                ("Blocked", BadgeTone::Warning)
            } else {
                ("Allowed", BadgeTone::Good)
            });
            row
        })
        .collect();
    if rows.is_empty() {
        rows.push(SettingsRow::new(
            "No websites recorded yet",
            "Visit sites in Chrome, Edge, Brave, Firefox, or Arc to track domains",
            SettingsRowValue::Text(String::from("Waiting")),
        ));
    }
    rows
}

fn focus_searches_rows(activity: &crate::FocusActivityState) -> Vec<SettingsRow> {
    let mut rows: Vec<SettingsRow> = activity
        .today
        .sorted_searches()
        .into_iter()
        .take(8)
        .map(|search| {
            SettingsRow::new(
                search.query,
                format!("{} search", search.engine),
                SettingsRowValue::Text(format!("{}x", search.search_count)),
            )
        })
        .collect();
    if rows.is_empty() {
        rows.push(SettingsRow::new(
            "No searches recorded yet",
            "Searches on Google, Bing, DuckDuckGo, YouTube, and GitHub appear here",
            SettingsRowValue::Text(String::from("0")),
        ));
    }
    rows
}

fn focus_blocklist_rows(activity: &crate::FocusActivityState) -> Vec<SettingsRow> {
    let blocklist = &activity.blocklist;
    let mut rows = vec![
        SettingsRow::new(
            "+ Block App from File...",
            "Choose an .exe application to auto-minimize during Focus",
            SettingsRowValue::Detail(""),
        )
        .with_action(SettingsAction::PickBlockedAppFile),
        SettingsRow::new(
            "+ Block Website from Clipboard",
            "Paste a copied URL or domain into the Focus blocklist",
            SettingsRowValue::Detail(""),
        )
        .with_action(SettingsAction::AddBlockedWebsiteFromClipboard),
    ];
    for custom_app in &blocklist.blocked_apps {
        if PRESET_BLOCKED_APPS
            .iter()
            .any(|(preset_key, _)| *preset_key == custom_app)
        {
            continue;
        }
        rows.push(
            SettingsRow::new(
                format!("App: {custom_app}"),
                "Custom blocked application during Focus",
                SettingsRowValue::Toggle(true),
            )
            .with_action(SettingsAction::ToggleBlockedApp(custom_app.clone())),
        );
    }
    for (preset_key, preset_label) in PRESET_BLOCKED_APPS {
        let key = normalize_app_key(preset_key);
        let blocked = blocklist.is_app_blocked(&key, preset_label);
        rows.push(
            SettingsRow::new(
                format!("App: {preset_label}"),
                "Auto-minimize this app while Focus Mode runs",
                SettingsRowValue::Toggle(blocked),
            )
            .with_action(SettingsAction::ToggleBlockedApp(key)),
        );
    }
    for custom_site in &blocklist.blocked_websites {
        if PRESET_BLOCKED_WEBSITES
            .iter()
            .any(|(preset_domain, _)| *preset_domain == custom_site)
        {
            continue;
        }
        rows.push(
            SettingsRow::new(
                format!("Site: {custom_site}"),
                "Custom blocked website during Focus",
                SettingsRowValue::Toggle(true),
            )
            .with_action(SettingsAction::ToggleBlockedWebsite(custom_site.clone())),
        );
    }
    for (preset_domain, preset_label) in PRESET_BLOCKED_WEBSITES {
        let blocked = blocklist.is_website_blocked(preset_domain);
        rows.push(
            SettingsRow::new(
                format!("{preset_label} ({preset_domain})"),
                "Block and minimize browser tabs on this domain during Focus",
                SettingsRowValue::Toggle(blocked),
            )
            .with_action(SettingsAction::ToggleBlockedWebsite(String::from(
                preset_domain,
            ))),
        );
    }
    rows
}

fn focus_page(snapshot: &OttoSettingsSnapshot, modules: &[ModuleRow]) -> SettingsContent {
    let activity = &snapshot.focus_activity;
    let today = &activity.today;
    let blocklist = &activity.blocklist;

    let mut focus_toggle = setting_row(
        snapshot,
        OttoSettingItem::Toggle(OttoSettingKind::FocusModule),
        SettingsPage::Focus,
    );
    if let Some(module_row) = modules
        .iter()
        .find(|row| row.id == ModuleId::Focus)
        .copied()
    {
        focus_toggle.badge = Some(module_badge(module_row));
        focus_toggle.muted = module_row.paused_by.is_some();
    }

    SettingsContent {
        hero: Some(SettingsHero {
            title: String::from("Focus & Screen Time"),
            subtitle: format!(
                "{} active today · {} in Focus · {} clicks · {} keys",
                format_screen_time(today.total_active_ms),
                format_screen_time(today.focus_session_ms),
                format_compact_count(today.total_clicks),
                format_compact_count(today.total_keystrokes),
            ),
        }),
        sections: vec![
            SettingsSection {
                title: "",
                footer: Some(
                    "Blocking applies while a Focus session is active. Screen time pauses after 3 minutes of inactivity.",
                ),
                rows: vec![
                    focus_toggle,
                    SettingsRow::new(
                        "Block Notifications in Focus",
                        "Suppress Notch notification banners and Dock badges during Focus",
                        SettingsRowValue::Toggle(blocklist.block_notifications),
                    )
                    .with_action(SettingsAction::ToggleFocusBlockNotifications),
                    SettingsRow::new(
                        "Allow Incoming Calls in Focus",
                        "Let ringing calls through unless the calling app is blocked",
                        SettingsRowValue::Toggle(blocklist.allow_calls_during_focus),
                    )
                    .with_action(SettingsAction::ToggleFocusAllowCalls),
                ],
            },
            SettingsSection {
                title: "Today & 7-Day Activity",
                footer: Some(
                    "Clicks and keystrokes count totals only. Typed characters and passwords are never recorded.",
                ),
                rows: focus_summary_rows(activity),
            },
            SettingsSection {
                title: "Most Used & Most Opened Apps",
                footer: Some("Toggle any app switch to block and auto-minimize it during Focus."),
                rows: focus_apps_rows(activity),
            },
            SettingsSection {
                title: "Top Websites",
                footer: Some(
                    "Detected natively from browser tabs and address bars with zero extensions.",
                ),
                rows: focus_sites_rows(activity),
            },
            SettingsSection {
                title: "Most Searched Topics",
                footer: None,
                rows: focus_searches_rows(activity),
            },
            SettingsSection {
                title: "Focus Blocklist & Distraction Presets",
                footer: Some(
                    "Blocked apps and websites minimize automatically without closing unsaved work.",
                ),
                rows: focus_blocklist_rows(activity),
            },
            SettingsSection {
                title: "Activity Data",
                footer: Some("Saved locally in %LOCALAPPDATA%\\otto-os\\focus_activity.txt."),
                rows: vec![
                    SettingsRow::new(
                        "Reset Activity & Screen Time Stats",
                        "Clear Today and 7-day history while keeping your blocklist",
                        SettingsRowValue::Detail("Clear"),
                    )
                    .with_action(SettingsAction::ResetFocusActivityStats),
                ],
            },
        ],
    }
}

#[must_use]
pub fn build_content(
    snapshot: &OttoSettingsSnapshot,
    page: SettingsPage,
    modules: &[ModuleRow],
    version: &str,
    update_status: Option<&str>,
) -> SettingsContent {
    match page {
        SettingsPage::Overview => overview(modules, version),
        SettingsPage::Modules => modules_page(modules),
        SettingsPage::Focus => focus_page(snapshot, modules),
        SettingsPage::Shortcuts => shortcuts_page(snapshot),
        SettingsPage::About => about_page(version, update_status),
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
        for row in &section.rows {
            let height = scale(row.height_points(), dpi);
            rows.push(Rect {
                x: viewport.x,
                y,
                width: viewport.width,
                height,
            });
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
    use super::{SettingsAction, SettingsRowValue, build_content, compute_content_layout};
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
                ..SystemSettingsState::default()
            },
            ..OttoSettingsSnapshot::default()
        }
    }

    fn content(snapshot: &OttoSettingsSnapshot, page: SettingsPage) -> super::SettingsContent {
        let modules = resolve_modules(snapshot, |_| None);
        build_content(snapshot, page, &modules, "1.0.0", None)
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
            SettingsPage::Appearance,
            SettingsPage::General,
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
        assert!(hero.subtitle.contains("11 of 13 modules running"));
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

    fn update_action(status: Option<&str>) -> Option<SettingsAction> {
        let snapshot = snapshot();
        let modules = resolve_modules(&snapshot, |_| None);
        build_content(&snapshot, SettingsPage::About, &modules, "1.0.0", status)
            .sections
            .iter()
            .flat_map(|section| &section.rows)
            .find(|row| row.label == "Check for Updates")
            .and_then(|row| row.action.clone())
    }

    #[test]
    fn the_update_row_checks_waits_or_opens_the_release_page() {
        assert_eq!(update_action(None), Some(SettingsAction::CheckForUpdates));
        assert_eq!(update_action(Some("Checking...")), None);
        assert_eq!(
            update_action(Some("Up to date")),
            Some(SettingsAction::CheckForUpdates)
        );
        assert_eq!(
            update_action(Some("Update v9.9.9")),
            Some(SettingsAction::OpenReleases)
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
    fn focus_page_renders_analytics_and_blocklist_controls() {
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

        let page = content(&snap, SettingsPage::Focus);
        let hero = page.hero.as_ref().expect("focus hero");
        assert_eq!(hero.title, "Focus & Screen Time");
        assert!(hero.subtitle.contains("16 clicks"));
        assert!(hero.subtitle.contains("85 keys"));
        let labels: Vec<&str> = page
            .sections
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.label.as_str()))
            .collect();
        assert!(labels.contains(&"Google Chrome"));
        assert!(labels.contains(&"google.com"));
        assert!(labels.contains(&"otto focus"));
        assert!(labels.contains(&"Reset Activity & Screen Time Stats"));
    }
}
