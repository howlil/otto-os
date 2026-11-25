use super::focus_activity_parse::{extract_search_from_title, infer_domain_from_title};
use crate::bar::status::ClockTime;
use crate::notch::focus_heatmap::FocusHeatmapLedger;
use crate::notch::focus_mode::FocusCategoryStore;

pub const FOCUS_IDLE_TIMEOUT_MS: u64 = 180_000;
pub const FOCUS_MAX_STEP_MS: u64 = 15_000;
pub const FOCUS_HISTORY_DAYS: usize = 7;
pub const MAX_TRACKED_APPS: usize = 64;
pub const MAX_TRACKED_WEBSITES: usize = 64;
pub const MAX_TRACKED_SEARCHES: usize = 64;
pub const MAX_BLOCKED_APPS: usize = 64;
pub const MAX_BLOCKED_WEBSITES: usize = 64;
pub const MAX_SEARCH_QUERY_CHARS: usize = 120;

pub const PRESET_BLOCKED_APPS: [(&str, &str); 8] = [
    ("discord", "Discord"),
    ("steam", "Steam"),
    ("epicgameslauncher", "Epic Games"),
    ("riotclientservices", "Riot Client"),
    ("robloxplayerbeta", "Roblox"),
    ("spotify", "Spotify"),
    ("whatsapp", "WhatsApp"),
    ("telegram", "Telegram"),
];

pub const PRESET_BLOCKED_WEBSITES: [(&str, &str); 20] = [
    ("youtube.com", "YouTube"),
    ("x.com", "X / Twitter"),
    ("reddit.com", "Reddit"),
    ("instagram.com", "Instagram"),
    ("tiktok.com", "TikTok"),
    ("facebook.com", "Facebook"),
    ("threads.net", "Threads"),
    ("snapchat.com", "Snapchat"),
    ("pinterest.com", "Pinterest"),
    ("tumblr.com", "Tumblr"),
    ("bsky.app", "Bluesky"),
    ("9gag.com", "9GAG"),
    ("discord.com", "Discord (web)"),
    ("netflix.com", "Netflix"),
    ("twitch.tv", "Twitch"),
    ("roblox.com", "Roblox"),
    ("poki.com", "Poki"),
    ("crazygames.com", "CrazyGames"),
    ("miniclip.com", "Miniclip"),
    ("y8.com", "Y8"),
];

const BROWSER_EXECUTABLES: [&str; 12] = [
    "chrome", "msedge", "brave", "firefox", "arc", "vivaldi", "opera", "chromium", "zen", "floorp",
    "waterfox", "thorium",
];

const SYSTEM_AND_SERVICE_APP_KEYS: &[&str] = &[
    "otto-os",
    "explorer",
    "finder",
    "applicationframehost",
    "applicationframe",
    "shellexperiencehost",
    "startmenuexperiencehost",
    "searchhost",
    "searchapp",
    "searchui",
    "textinputhost",
    "lockapp",
    "widgets",
    "widgetservice",
    "dwm",
    "sihost",
    "taskhostw",
    "dllhost",
    "svchost",
    "conhost",
    "openconsole",
    "backgroundtaskhost",
    "runtimebroker",
    "ctfmon",
    "smartscreen",
    "consent",
    "fontdrvhost",
    "audiodg",
    "wlanext",
    "spoolsv",
    "werfault",
    "wermgr",
    "csrss",
    "wininit",
    "winlogon",
    "lsass",
    "services",
    "smss",
    "msiexec",
    "systemsettings",
    "taskmgr",
    "sechealthui",
    "securityhealthsystray",
    "securityhealthservice",
    "windowsdefender",
    "coreai",
    "clicktodo",
    "control",
    "mmc",
    "regedit",
    "appverif",
    "charmap",
    "cleanmgr",
    "dfrgui",
    "diskmgmt",
    "devmgmt",
    "eventvwr",
    "compmgmt",
    "comexp",
    "mdsched",
    "msconfig",
    "msinfo32",
    "odbcad32",
    "perfmon",
    "resmon",
    "psr",
    "recoverydrive",
    "rstrui",
    "iscsicpl",
    "shrpubw",
    "utilman",
    "verifier",
    "winver",
    "wfs",
    "wsreset",
    "dxdiag",
    "hh",
    "winhlp32",
    "splwow64",
    "powershell_ise",
    "wf",
    "taskschd",
    "secpol",
    "gpedit",
    "lusrmgr",
    "certmgr",
    "certlm",
    "tpm",
    "wmimgmt",
    "printmanagement",
    "magnify",
    "narrator",
    "osk",
    "jucheck",
    "javacpl",
    "crashpad_handler",
    "crashreporter",
    "credentialuibroker",
    "bioenrollmenthost",
    "creddialoghost",
    "win32webviewhost",
    "wsl",
];

const SYSTEM_AND_NOISE_TITLES: &[&str] = &[
    "otto os",
    "otto settings",
    "file explorer",
    "finder",
    "windows explorer",
    "applicationframe",
    "application frame host",
    "application verifier",
    "application verifier (x64)",
    "application verifier (x86)",
    "application verifier (wow)",
    "character map",
    "check for updates",
    "click to do",
    "component services",
    "computer management",
    "control panel",
    "windows tools",
    "administrative tools",
    "disk cleanup",
    "defragment and optimize drives",
    "event viewer",
    "resource monitor",
    "performance monitor",
    "system configuration",
    "system information",
    "registry editor",
    "task manager",
    "task scheduler",
    "services",
    "device manager",
    "disk management",
    "local security policy",
    "print management",
    "odbc data sources (32-bit)",
    "odbc data sources (64-bit)",
    "recovery drive",
    "steps recorder",
    "iscsi initiator",
    "windows defender firewall",
    "windows defender firewall with advanced security",
    "windows memory diagnostic",
    "windows powershell ise",
    "windows powershell ise 5.1",
    "windows powershell ise (x86)",
    "windows security",
    "windows software development kit",
    "windows fax and scan",
    "windows backup",
    "get started",
    "magnifier",
    "narrator",
    "on-screen keyboard",
    "live captions",
    "voice access",
    "settings",
    "system settings",
    "default programs",
    "run",
    "documents",
    "downloads",
    "pictures",
    "music",
    "videos",
    "desktop",
    "career",
    "wsl",
];

const NON_DOMAIN_FILE_EXTENSIONS: &[&str] = &[
    "pdf", "exe", "msi", "tmp", "zip", "rar", "7z", "tar", "gz", "png", "jpg", "jpeg", "gif",
    "webp", "svg", "ico", "bmp", "mp3", "mp4", "mkv", "wav", "avi", "mov", "flac", "ogg", "doc",
    "docx", "xls", "xlsx", "ppt", "pptx", "txt", "csv", "json", "xml", "yaml", "yml", "toml",
    "ini", "cfg", "log", "js", "ts", "tsx", "jsx", "css", "scss", "html", "htm", "chm", "hlp",
    "msc", "cpl", "dll", "sys", "bat", "cmd", "ps1", "vbs", "lnk", "iso", "dmg",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DayKey {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl DayKey {
    #[must_use]
    pub const fn from_clock(clock: ClockTime) -> Self {
        Self {
            year: clock.year,
            month: clock.month,
            day: clock.day,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.year >= 2000 && self.month >= 1 && self.month <= 12 && self.day >= 1 && self.day <= 31
    }

    #[must_use]
    pub fn encode(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    #[must_use]
    pub fn decode(raw: &str) -> Option<Self> {
        let mut parts = raw.trim().split('-');
        let year: u16 = parts.next()?.parse().ok()?;
        let month: u8 = parts.next()?.parse().ok()?;
        let day: u8 = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        let key = Self { year, month, day };
        key.is_valid().then_some(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsageEntry {
    pub app_key: String,
    pub display_name: String,
    pub active_ms: u64,
    pub open_count: u32,
    pub click_count: u64,
    pub keystroke_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebsiteUsageEntry {
    pub domain: String,
    pub active_ms: u64,
    pub visit_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQueryEntry {
    pub query: String,
    pub engine: String,
    pub search_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DailyActivityRecord {
    pub day: DayKey,
    pub total_active_ms: u64,
    pub focus_session_ms: u64,
    pub total_clicks: u64,
    pub total_keystrokes: u64,
    pub blocked_attempts: u32,
    pub apps: Vec<AppUsageEntry>,
    pub websites: Vec<WebsiteUsageEntry>,
    pub searches: Vec<SearchQueryEntry>,
}

impl DailyActivityRecord {
    #[must_use]
    pub fn sorted_apps(&self) -> Vec<AppUsageEntry> {
        let mut apps = self.apps.clone();
        apps.sort_by(|a, b| {
            b.active_ms
                .cmp(&a.active_ms)
                .then_with(|| b.open_count.cmp(&a.open_count))
                .then_with(|| {
                    (b.click_count + b.keystroke_count).cmp(&(a.click_count + a.keystroke_count))
                })
                .then_with(|| a.display_name.cmp(&b.display_name))
        });
        apps
    }

    #[must_use]
    pub fn sorted_websites(&self) -> Vec<WebsiteUsageEntry> {
        let mut sites = self.websites.clone();
        sites.sort_by(|a, b| {
            b.active_ms
                .cmp(&a.active_ms)
                .then_with(|| b.visit_count.cmp(&a.visit_count))
                .then_with(|| a.domain.cmp(&b.domain))
        });
        sites
    }

    #[must_use]
    pub fn sorted_searches(&self) -> Vec<SearchQueryEntry> {
        let mut searches = self.searches.clone();
        searches.sort_by(|a, b| {
            b.search_count
                .cmp(&a.search_count)
                .then_with(|| a.query.cmp(&b.query))
        });
        searches
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusBlocklist {
    pub blocked_apps: Vec<String>,
    pub blocked_websites: Vec<String>,
    pub block_notifications: bool,
    pub allow_calls_during_focus: bool,
}

impl Default for FocusBlocklist {
    fn default() -> Self {
        Self {
            blocked_apps: Vec::new(),
            blocked_websites: Vec::new(),
            block_notifications: true,
            allow_calls_during_focus: true,
        }
    }
}

impl FocusBlocklist {
    #[must_use]
    pub fn is_app_blocked(&self, app_key_or_exe: &str, display_name: &str) -> bool {
        let norm_key = normalize_app_key(app_key_or_exe);
        let norm_display = normalize_app_key(display_name);
        let compact_key = compact_app_id(&norm_key);
        let compact_display = compact_app_id(&norm_display);
        self.blocked_apps.iter().any(|blocked| {
            let norm_blocked = normalize_app_key(blocked);
            let compact_blocked = compact_app_id(&norm_blocked);
            (!norm_key.is_empty() && norm_blocked == norm_key)
                || (!norm_display.is_empty() && norm_blocked == norm_display)
                || (compact_blocked.len() >= 3
                    && (compact_blocked == compact_key || compact_blocked == compact_display))
        })
    }

    pub fn add_app(&mut self, raw: &str) -> bool {
        if is_non_trackable_focus_app(raw, "") {
            return false;
        }
        let key = normalize_app_key(raw);
        if key.is_empty() || self.blocked_apps.contains(&key) {
            return false;
        }
        if self.blocked_apps.len() >= MAX_BLOCKED_APPS {
            return false;
        }
        self.blocked_apps.push(key);
        true
    }

    pub fn toggle_app(&mut self, raw: &str) -> bool {
        let key = normalize_app_key(raw);
        if key.is_empty() {
            return false;
        }
        let compact = compact_app_id(&key);
        if let Some(pos) = self.blocked_apps.iter().position(|item| {
            let norm = normalize_app_key(item);
            norm == key || (compact.len() >= 3 && compact_app_id(&norm) == compact)
        }) {
            self.blocked_apps.remove(pos);
            false
        } else if !is_non_trackable_focus_app(raw, "") && self.blocked_apps.len() < MAX_BLOCKED_APPS
        {
            self.blocked_apps.push(key);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_website_blocked(&self, domain: &str) -> bool {
        let Some(norm) = normalize_domain(domain) else {
            return false;
        };
        self.blocked_websites
            .iter()
            .any(|blocked| domain_matches_rule(&norm, blocked))
    }

    pub fn add_website(&mut self, raw: &str) -> bool {
        let Some(domain) = normalize_domain(raw) else {
            return false;
        };
        if self.blocked_websites.contains(&domain) {
            return false;
        }
        if self.blocked_websites.len() >= MAX_BLOCKED_WEBSITES {
            return false;
        }
        self.blocked_websites.push(domain);
        true
    }

    pub fn toggle_website(&mut self, raw: &str) -> bool {
        let Some(domain) = normalize_domain(raw) else {
            return false;
        };
        if let Some(pos) = self
            .blocked_websites
            .iter()
            .position(|item| *item == domain)
        {
            self.blocked_websites.remove(pos);
            false
        } else if self.blocked_websites.len() < MAX_BLOCKED_WEBSITES {
            self.blocked_websites.push(domain);
            true
        } else {
            false
        }
    }
}

fn domain_matches_rule(domain: &str, rule: &str) -> bool {
    if domain == rule {
        return true;
    }
    if domain.len() > rule.len()
        && domain.ends_with(rule)
        && domain.as_bytes()[domain.len() - rule.len() - 1] == b'.'
    {
        return true;
    }
    (rule == "x.com" && (domain == "twitter.com" || domain.ends_with(".twitter.com")))
        || (rule == "twitter.com" && (domain == "x.com" || domain.ends_with(".x.com")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusBlockEnforcement {
    App { label: String },
    Website { domain: String },
}

impl FocusBlockEnforcement {
    #[must_use]
    pub fn target_label(&self) -> &str {
        match self {
            Self::App { label } => label,
            Self::Website { domain } => domain,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowserInspection {
    pub domain: Option<String>,
    pub search_query: Option<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FocusRollingSummary {
    pub total_active_ms: u64,
    pub focus_session_ms: u64,
    pub total_clicks: u64,
    pub total_keystrokes: u64,
    pub blocked_attempts: u32,
    pub days_counted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusActivityState {
    pub today: DailyActivityRecord,
    pub history: Vec<DailyActivityRecord>,
    pub blocklist: FocusBlocklist,
    pub categories: FocusCategoryStore,
    pub heatmap: FocusHeatmapLedger,
    current_app_key: Option<String>,
    current_app_name: Option<String>,
    current_domain: Option<String>,
    last_search_key: Option<String>,
    last_sample_ms: Option<u64>,
    dirty: bool,
}

impl FocusActivityState {
    #[must_use]
    pub fn current_app_key(&self) -> Option<&str> {
        self.current_app_key.as_deref()
    }

    #[must_use]
    pub fn current_domain(&self) -> Option<&str> {
        self.current_domain.as_deref()
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn reset_sample_clock(&mut self) {
        self.last_sample_ms = None;
    }

    pub fn reset_stats(&mut self) {
        let day = self.today.day;
        self.today = DailyActivityRecord {
            day,
            ..DailyActivityRecord::default()
        };
        self.history.clear();
        self.heatmap.entries.clear();
        self.last_search_key = None;
        self.dirty = true;
    }

    pub fn record_completed_focus_session(&mut self, day: DayKey, category_id: &str) {
        self.rollover_if_needed(day);
        self.heatmap.record_completed_session(day, category_id);
        self.dirty = true;
    }

    pub fn rollover_if_needed(&mut self, day: DayKey) {
        if !day.is_valid() {
            return;
        }
        if !self.today.day.is_valid() {
            self.today.day = day;
            return;
        }
        if self.today.day == day {
            return;
        }
        let previous = std::mem::replace(
            &mut self.today,
            DailyActivityRecord {
                day,
                ..DailyActivityRecord::default()
            },
        );
        if previous.total_active_ms > 0
            || previous.total_clicks > 0
            || previous.total_keystrokes > 0
            || !previous.apps.is_empty()
        {
            self.history.insert(0, previous);
            self.history.truncate(FOCUS_HISTORY_DAYS);
        }
        self.last_search_key = None;
        self.dirty = true;
    }

    pub fn observe_foreground(
        &mut self,
        day: DayKey,
        app_key_raw: &str,
        display_name: &str,
        browser: Option<BrowserInspection>,
    ) {
        self.rollover_if_needed(day);
        let app_key = normalize_app_key(app_key_raw);
        if app_key.is_empty() || is_non_trackable_focus_app(app_key_raw, display_name) {
            self.current_app_key = None;
            self.current_app_name = None;
            self.current_domain = None;
            self.last_search_key = None;
            return;
        }
        let clean_name = clean_app_display_name(display_name, &app_key);
        let app_changed = self.current_app_key.as_deref() != Some(app_key.as_str());
        if app_changed {
            self.current_app_key = Some(app_key.clone());
            self.current_app_name = Some(clean_name.clone());
            let entry = ensure_app_entry(&mut self.today.apps, &app_key, &clean_name);
            entry.open_count = entry.open_count.saturating_add(1);
            self.dirty = true;
        }

        if let Some(info) = browser {
            if let Some(domain) = info.domain {
                if self.current_domain.as_deref() != Some(domain.as_str()) {
                    let site = ensure_website_entry(&mut self.today.websites, &domain);
                    site.visit_count = site.visit_count.saturating_add(1);
                    self.current_domain = Some(domain);
                    self.dirty = true;
                }
            } else if app_changed {
                self.current_domain = None;
            }
            if let Some((query, engine)) = info.search_query {
                let search_key =
                    format!("{}:{}", engine.to_ascii_lowercase(), query.to_lowercase());
                if self.last_search_key.as_deref() != Some(search_key.as_str()) {
                    let search = ensure_search_entry(&mut self.today.searches, &query, &engine);
                    search.search_count = search.search_count.saturating_add(1);
                    self.last_search_key = Some(search_key);
                    self.dirty = true;
                }
            }
        } else {
            self.current_domain = None;
            self.last_search_key = None;
        }
    }

    pub fn record_sample(
        &mut self,
        day: DayKey,
        now_ms: u64,
        idle_ms: u64,
        focus_running: bool,
        delta_clicks: u64,
        delta_keystrokes: u64,
    ) {
        let cat_id = self.categories.active_category_id.clone();
        self.record_sample_for_category(
            day,
            now_ms,
            idle_ms,
            focus_running,
            &cat_id,
            (delta_clicks, delta_keystrokes),
        );
    }

    pub fn record_sample_for_category(
        &mut self,
        day: DayKey,
        now_ms: u64,
        idle_ms: u64,
        focus_running: bool,
        category_id: &str,
        input_deltas: (u64, u64),
    ) {
        let (delta_clicks, delta_keystrokes) = input_deltas;
        self.rollover_if_needed(day);
        let delta_ms = match self.last_sample_ms {
            Some(prev) if now_ms > prev => (now_ms - prev).min(FOCUS_MAX_STEP_MS),
            _ => 0,
        };
        self.last_sample_ms = Some(now_ms);

        if idle_ms < FOCUS_IDLE_TIMEOUT_MS && delta_ms > 0 {
            self.today.total_active_ms = self.today.total_active_ms.saturating_add(delta_ms);
            if focus_running {
                self.today.focus_session_ms = self.today.focus_session_ms.saturating_add(delta_ms);
                self.heatmap.record_focus(day, category_id, delta_ms);
            }
            if let Some(app_key) = self.current_app_key.clone() {
                let name = self
                    .current_app_name
                    .clone()
                    .unwrap_or_else(|| app_key.clone());
                let entry = ensure_app_entry(&mut self.today.apps, &app_key, &name);
                entry.active_ms = entry.active_ms.saturating_add(delta_ms);
            }
            if let Some(domain) = self.current_domain.clone() {
                let site = ensure_website_entry(&mut self.today.websites, &domain);
                site.active_ms = site.active_ms.saturating_add(delta_ms);
            }
            self.dirty = true;
        }

        if delta_clicks > 0 || delta_keystrokes > 0 {
            self.today.total_clicks = self.today.total_clicks.saturating_add(delta_clicks);
            self.today.total_keystrokes =
                self.today.total_keystrokes.saturating_add(delta_keystrokes);
            if let Some(app_key) = self.current_app_key.clone() {
                let name = self
                    .current_app_name
                    .clone()
                    .unwrap_or_else(|| app_key.clone());
                let entry = ensure_app_entry(&mut self.today.apps, &app_key, &name);
                entry.click_count = entry.click_count.saturating_add(delta_clicks);
                entry.keystroke_count = entry.keystroke_count.saturating_add(delta_keystrokes);
            }
            self.dirty = true;
        }
    }

    pub fn evaluate_block(
        &mut self,
        focus_running: bool,
        app_key_raw: &str,
        display_name: &str,
        domain: Option<&str>,
    ) -> Option<FocusBlockEnforcement> {
        if !focus_running {
            return None;
        }
        let cat_blocks_app = self
            .categories
            .active()
            .should_block_app(app_key_raw, display_name);
        if cat_blocks_app || self.blocklist.is_app_blocked(app_key_raw, display_name) {
            self.today.blocked_attempts = self.today.blocked_attempts.saturating_add(1);
            self.dirty = true;
            let label = sanitize_label(display_name, &normalize_app_key(app_key_raw), 40);
            return Some(FocusBlockEnforcement::App { label });
        }
        if let Some(raw_domain) = domain
            && (self.categories.active().should_block_website(raw_domain)
                || self.blocklist.is_website_blocked(raw_domain))
        {
            let norm = normalize_domain(raw_domain).unwrap_or_else(|| raw_domain.to_owned());
            self.today.blocked_attempts = self.today.blocked_attempts.saturating_add(1);
            self.dirty = true;
            return Some(FocusBlockEnforcement::Website { domain: norm });
        }
        None
    }

    #[must_use]
    pub fn should_block_notification(
        &self,
        focus_running: bool,
        is_call: bool,
        source_app: &str,
    ) -> bool {
        if !focus_running {
            return false;
        }
        if self.blocklist.is_app_blocked(source_app, source_app) {
            return true;
        }
        if !self.blocklist.block_notifications {
            return false;
        }
        if is_call && self.blocklist.allow_calls_during_focus {
            return false;
        }
        true
    }

    #[must_use]
    pub fn rolling_summary(&self) -> FocusRollingSummary {
        let mut summary = FocusRollingSummary {
            total_active_ms: self.today.total_active_ms,
            focus_session_ms: self.today.focus_session_ms,
            total_clicks: self.today.total_clicks,
            total_keystrokes: self.today.total_keystrokes,
            blocked_attempts: self.today.blocked_attempts,
            days_counted: 1,
        };
        for day in self
            .history
            .iter()
            .take(FOCUS_HISTORY_DAYS.saturating_sub(1))
        {
            summary.total_active_ms = summary.total_active_ms.saturating_add(day.total_active_ms);
            summary.focus_session_ms = summary
                .focus_session_ms
                .saturating_add(day.focus_session_ms);
            summary.total_clicks = summary.total_clicks.saturating_add(day.total_clicks);
            summary.total_keystrokes = summary
                .total_keystrokes
                .saturating_add(day.total_keystrokes);
            summary.blocked_attempts = summary
                .blocked_attempts
                .saturating_add(day.blocked_attempts);
            summary.days_counted += 1;
        }
        summary
    }

    #[must_use]
    pub fn encode(&self) -> String {
        let mut lines = vec![String::from("v1")];
        lines.push(format!(
            "CONF\t{}\t{}",
            u8::from(self.blocklist.block_notifications),
            u8::from(self.blocklist.allow_calls_during_focus)
        ));
        for app in &self.blocklist.blocked_apps {
            lines.push(format!("BAPP\t{}", sanitize_field(app)));
        }
        for site in &self.blocklist.blocked_websites {
            lines.push(format!("BWEB\t{}", sanitize_field(site)));
        }
        encode_day_record(&self.today, &mut lines);
        for record in self.history.iter().take(FOCUS_HISTORY_DAYS) {
            encode_day_record(record, &mut lines);
        }
        self.categories.encode_lines(&mut lines);
        self.heatmap.encode_lines(&mut lines);
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }

    #[must_use]
    pub fn decode(raw: &str) -> Self {
        let mut lines = raw.lines();
        if lines.next().map(str::trim) != Some("v1") {
            return Self::default();
        }
        let mut state = Self::default();
        let mut records: Vec<DailyActivityRecord> = Vec::new();
        let mut saw_custom_cat = false;
        for line in lines {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.first().copied() {
                Some("CONF") if parts.len() >= 3 => {
                    state.blocklist.block_notifications = parts[1] != "0";
                    state.blocklist.allow_calls_during_focus = parts[2] != "0";
                }
                Some("BAPP") if parts.len() >= 2 => {
                    let _ = state.blocklist.add_app(parts[1]);
                }
                Some("BWEB") if parts.len() >= 2 => {
                    let _ = state.blocklist.add_website(parts[1]);
                }
                Some("DAY") if parts.len() >= 7 => {
                    if let Some(day) = DayKey::decode(parts[1]) {
                        records.push(DailyActivityRecord {
                            day,
                            total_active_ms: parts[2].parse().unwrap_or(0),
                            focus_session_ms: parts[3].parse().unwrap_or(0),
                            total_clicks: parts[4].parse().unwrap_or(0),
                            total_keystrokes: parts[5].parse().unwrap_or(0),
                            blocked_attempts: parts[6].parse().unwrap_or(0),
                            apps: Vec::new(),
                            websites: Vec::new(),
                            searches: Vec::new(),
                        });
                    }
                }
                Some("APP") if parts.len() >= 7 => {
                    if let Some(current) = records.last_mut()
                        && decode_app_line(current, &parts)
                    {
                        state.dirty = true;
                    }
                }
                Some("WEB") if parts.len() >= 4 => {
                    if let Some(current) = records.last_mut()
                        && current.websites.len() < MAX_TRACKED_WEBSITES
                    {
                        if let Some(domain) = normalize_domain(parts[1]) {
                            current.websites.push(WebsiteUsageEntry {
                                domain,
                                active_ms: parts[2].parse().unwrap_or(0),
                                visit_count: parts[3].parse().unwrap_or(0),
                            });
                        } else {
                            state.dirty = true;
                        }
                    }
                }
                Some("SRC") if parts.len() >= 4 => {
                    if let Some(current) = records.last_mut()
                        && current.searches.len() < MAX_TRACKED_SEARCHES
                    {
                        let query = sanitize_label(parts[1], "", MAX_SEARCH_QUERY_CHARS);
                        let engine = sanitize_label(parts[2], "Web", 24);
                        if !query.is_empty() {
                            current.searches.push(SearchQueryEntry {
                                query,
                                engine,
                                search_count: parts[3].parse().unwrap_or(0),
                            });
                        }
                    }
                }
                _ => {
                    if !state.categories.decode_line(&parts, &mut saw_custom_cat) {
                        let _ = state.heatmap.decode_line(&parts);
                    }
                }
            }
        }
        state.categories.ensure_valid();
        records.sort_by_key(|record| std::cmp::Reverse(record.day));
        if let Some(first) = records.first().cloned() {
            state.today = first;
            state.history = records
                .into_iter()
                .skip(1)
                .take(FOCUS_HISTORY_DAYS)
                .collect();
        }
        state
    }
}

fn decode_app_line(current: &mut DailyActivityRecord, parts: &[&str]) -> bool {
    if is_non_trackable_focus_app(parts[1], parts[2]) {
        return true;
    }
    let key = normalize_app_key(parts[1]);
    if key.is_empty() {
        return true;
    }
    let clean_name = clean_app_display_name(parts[2], &key);
    let mut normalized = key != parts[1] || clean_name != parts[2];
    let entry = ensure_app_entry(&mut current.apps, &key, &clean_name);
    if entry.active_ms > 0 || entry.open_count > 0 {
        normalized = true;
    }
    entry.active_ms = entry
        .active_ms
        .saturating_add(parts[3].parse().unwrap_or(0));
    entry.open_count = entry
        .open_count
        .saturating_add(parts[4].parse().unwrap_or(0));
    entry.click_count = entry
        .click_count
        .saturating_add(parts[5].parse().unwrap_or(0));
    entry.keystroke_count = entry
        .keystroke_count
        .saturating_add(parts[6].parse().unwrap_or(0));
    normalized
}

fn encode_day_record(record: &DailyActivityRecord, lines: &mut Vec<String>) {
    if !record.day.is_valid() {
        return;
    }
    lines.push(format!(
        "DAY\t{}\t{}\t{}\t{}\t{}\t{}",
        record.day.encode(),
        record.total_active_ms,
        record.focus_session_ms,
        record.total_clicks,
        record.total_keystrokes,
        record.blocked_attempts,
    ));
    for app in record.sorted_apps().into_iter().take(MAX_TRACKED_APPS) {
        lines.push(format!(
            "APP\t{}\t{}\t{}\t{}\t{}\t{}",
            sanitize_field(&app.app_key),
            sanitize_field(&app.display_name),
            app.active_ms,
            app.open_count,
            app.click_count,
            app.keystroke_count,
        ));
    }
    for site in record
        .sorted_websites()
        .into_iter()
        .take(MAX_TRACKED_WEBSITES)
    {
        lines.push(format!(
            "WEB\t{}\t{}\t{}",
            sanitize_field(&site.domain),
            site.active_ms,
            site.visit_count,
        ));
    }
    for search in record
        .sorted_searches()
        .into_iter()
        .take(MAX_TRACKED_SEARCHES)
    {
        lines.push(format!(
            "SRC\t{}\t{}\t{}",
            sanitize_field(&search.query),
            sanitize_field(&search.engine),
            search.search_count,
        ));
    }
}

fn ensure_app_entry<'a>(
    apps: &'a mut Vec<AppUsageEntry>,
    app_key: &str,
    display_name: &str,
) -> &'a mut AppUsageEntry {
    if let Some(pos) = apps.iter().position(|item| item.app_key == app_key) {
        return &mut apps[pos];
    }
    if apps.len() >= MAX_TRACKED_APPS
        && let Some((min_idx, _)) = apps
            .iter()
            .enumerate()
            .min_by_key(|(_, item)| (item.active_ms, item.open_count))
    {
        apps.swap_remove(min_idx);
    }
    apps.push(AppUsageEntry {
        app_key: app_key.to_owned(),
        display_name: display_name.to_owned(),
        active_ms: 0,
        open_count: 0,
        click_count: 0,
        keystroke_count: 0,
    });
    let last = apps.len() - 1;
    &mut apps[last]
}

fn ensure_website_entry<'a>(
    sites: &'a mut Vec<WebsiteUsageEntry>,
    domain: &str,
) -> &'a mut WebsiteUsageEntry {
    if let Some(pos) = sites.iter().position(|item| item.domain == domain) {
        return &mut sites[pos];
    }
    if sites.len() >= MAX_TRACKED_WEBSITES
        && let Some((min_idx, _)) = sites
            .iter()
            .enumerate()
            .min_by_key(|(_, item)| (item.active_ms, item.visit_count))
    {
        sites.swap_remove(min_idx);
    }
    sites.push(WebsiteUsageEntry {
        domain: domain.to_owned(),
        active_ms: 0,
        visit_count: 0,
    });
    let last = sites.len() - 1;
    &mut sites[last]
}

fn ensure_search_entry<'a>(
    searches: &'a mut Vec<SearchQueryEntry>,
    query: &str,
    engine: &str,
) -> &'a mut SearchQueryEntry {
    if let Some(pos) = searches.iter().position(|item| {
        item.engine.eq_ignore_ascii_case(engine) && item.query.eq_ignore_ascii_case(query)
    }) {
        return &mut searches[pos];
    }
    if searches.len() >= MAX_TRACKED_SEARCHES
        && let Some((min_idx, _)) = searches
            .iter()
            .enumerate()
            .min_by_key(|(_, item)| item.search_count)
    {
        searches.swap_remove(min_idx);
    }
    searches.push(SearchQueryEntry {
        query: query.to_owned(),
        engine: engine.to_owned(),
        search_count: 0,
    });
    let last = searches.len() - 1;
    &mut searches[last]
}

fn sanitize_field(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !matches!(ch, '\t' | '\n' | '\r') && !ch.is_control())
        .collect::<String>()
        .trim()
        .to_owned()
}

pub(super) fn sanitize_label(raw: &str, fallback: &str, max_chars: usize) -> String {
    let cleaned: String = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|ch| !ch.is_control())
        .take(max_chars)
        .collect();
    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned
    }
}

#[must_use]
pub fn compact_app_id(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn extract_aumid_stem(lower: &str) -> String {
    let body = lower.strip_prefix("aumid:").unwrap_or(lower).trim();
    let (pkg_family, app_id) = body.split_once('!').unwrap_or((body, ""));
    let pkg_name = match pkg_family.rsplit_once('_') {
        Some((prefix, hash))
            if !prefix.is_empty()
                && (8..=20).contains(&hash.len())
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) =>
        {
            prefix
        }
        _ => pkg_family,
    };
    let seg = pkg_name.rsplit('.').next().unwrap_or(pkg_name).trim();
    let app_seg = app_id.rsplit('.').next().unwrap_or(app_id).trim();
    if (seg.is_empty() || matches!(seg, "app" | "client" | "desktop" | "main" | "root"))
        && !app_seg.is_empty()
        && !matches!(app_seg, "app" | "main" | "root")
    {
        app_seg.to_owned()
    } else {
        seg.to_owned()
    }
}

fn canonicalize_app_stem(stem: &str) -> String {
    match stem {
        "whatsappdesktop" | "whatsapp.root" | "whatsapp" => String::from("whatsapp"),
        "applemusicwin" | "apple music" | "applemusic" => String::from("applemusic"),
        "spotifymusic" | "spotify" => String::from("spotify"),
        "windowscalculator" | "calculatorapp" | "calc" => String::from("calculator"),
        "windowscamera" => String::from("camera"),
        "windowsphotos" => String::from("photos"),
        "windowsterminal" | "windows terminal" | "terminal" | "wt" => {
            String::from("windowsterminal")
        }
        "visual studio code" | "vscode" => String::from("code"),
        "telegramdesktop" | "telegram desktop" => String::from("telegram"),
        _ => stem.to_owned(),
    }
}

#[must_use]
pub fn normalize_app_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    let stem = if lower.starts_with("aumid:") || lower.contains('!') {
        extract_aumid_stem(&lower)
    } else {
        let file_part = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed).trim();
        let file_lower = file_part.to_ascii_lowercase();
        let without_ext = file_lower
            .strip_suffix(".exe")
            .or_else(|| file_lower.strip_suffix(".lnk"))
            .unwrap_or(&file_lower)
            .trim();
        without_ext
            .strip_suffix(".root")
            .or_else(|| without_ext.strip_suffix(".app"))
            .unwrap_or(without_ext)
            .trim()
            .to_owned()
    };
    let cleaned: String = stem
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(ch, '\t' | '\n' | '\r'))
        .collect();
    canonicalize_app_stem(&cleaned)
}

fn is_windows_system_path_or_guid(raw_lower: &str) -> bool {
    raw_lower.contains("\\windows\\system32\\")
        || raw_lower.contains("\\windows\\syswow64\\")
        || raw_lower.contains("\\windows\\immersivecontrolpanel\\")
        || raw_lower.contains("\\windows\\winsxs\\")
        || raw_lower.contains("\\windows\\servicing\\")
        || raw_lower.ends_with("\\windows\\explorer.exe")
        || raw_lower.ends_with("\\windows\\regedit.exe")
        || raw_lower.ends_with("\\windows\\hh.exe")
        || raw_lower.ends_with("\\windows\\winhlp32.exe")
        || raw_lower.ends_with("\\windows\\splwow64.exe")
        || raw_lower.starts_with("{1ac14e77-02e7-4e5d-b744-2eb1ae5198b7}")
        || raw_lower.starts_with("{d65231b0-b2f1-4857-a4ce-a8e7c6ea7d27}")
        || raw_lower.starts_with("{f38bf404-1d43-42f2-9305-67de0b28fc23}")
}

fn has_non_app_target_extension(raw_lower: &str) -> bool {
    [
        ".tmp", ".msc", ".cpl", ".chm", ".hlp", ".url", ".html", ".htm", ".pdf", ".txt", ".md",
        ".doc", ".docx", ".ini", ".log",
    ]
    .iter()
    .any(|ext| raw_lower.ends_with(ext))
}

fn is_documentation_or_aux_title(title: &str) -> bool {
    title.contains("documentation")
        || title.ends_with(" help")
        || title.starts_with("help ")
        || title.ends_with(" manual")
        || title.ends_with(" user guide")
        || title.ends_with(" reference guide")
        || title.ends_with(" release notes")
        || title.ends_with(" readme")
        || title.ends_with(" read me")
        || title.ends_with(" changelog")
        || title.ends_with(" license")
        || title.ends_with(" software development kit")
        || title.starts_with("uninstall ")
        || title.starts_with("remove ")
        || title.starts_with("check for update")
        || title.ends_with(" updater")
        || title.ends_with(" installer")
        || title.starts_with("otto os setup")
}

#[must_use]
pub fn is_non_trackable_focus_app(raw_key_or_path: &str, display_name: &str) -> bool {
    let raw_lower = raw_key_or_path.trim().to_ascii_lowercase();
    if !raw_lower.is_empty()
        && (is_windows_system_path_or_guid(&raw_lower)
            || has_non_app_target_extension(&raw_lower)
            || raw_lower.contains("otto-os-setup")
            || raw_lower.contains("unins00"))
    {
        return true;
    }
    let key = normalize_app_key(raw_key_or_path);
    if !key.is_empty()
        && (SYSTEM_AND_SERVICE_APP_KEYS.contains(&key.as_str())
            || key.starts_with("otto-os")
            || key.starts_with("unins00")
            || key.starts_with("uninstall"))
    {
        return true;
    }
    let title = display_name.trim().to_ascii_lowercase();
    if !title.is_empty()
        && (SYSTEM_AND_NOISE_TITLES.contains(&title.as_str())
            || is_documentation_or_aux_title(&title))
    {
        return true;
    }
    false
}

fn title_case_app_stem(stem: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for ch in stem.chars() {
        if matches!(ch, '-' | '_' | ' ' | '.') {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            capitalize = true;
        } else if capitalize {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() { stem.to_owned() } else { out }
}

#[must_use]
pub fn clean_app_display_name(raw_name: &str, canonical_key: &str) -> String {
    match canonical_key {
        "whatsapp" => return String::from("WhatsApp"),
        "applemusic" => return String::from("Apple Music"),
        "windowsterminal" => return String::from("Terminal"),
        "claude" => return String::from("Claude"),
        "spotify" => return String::from("Spotify"),
        "discord" => return String::from("Discord"),
        "telegram" => return String::from("Telegram"),
        "steam" => return String::from("Steam"),
        _ => {}
    }
    let cleaned = sanitize_label(raw_name, canonical_key, 48);
    let stripped = cleaned
        .strip_suffix(".Root")
        .or_else(|| cleaned.strip_suffix(".root"))
        .or_else(|| cleaned.strip_suffix(".exe"))
        .or_else(|| cleaned.strip_suffix(".EXE"))
        .unwrap_or(&cleaned)
        .trim();
    if stripped.is_empty() {
        return title_case_app_stem(canonical_key);
    }
    let compact_raw = compact_app_id(stripped);
    let compact_key = compact_app_id(canonical_key);
    if !compact_key.is_empty()
        && (compact_raw == compact_key
            || (stripped.chars().count() == 16
                && compact_key.len() > compact_raw.len()
                && compact_key.starts_with(&compact_raw)))
        && (canonical_key.contains('-') || canonical_key.contains('_'))
    {
        return title_case_app_stem(canonical_key);
    }
    stripped.to_owned()
}

#[must_use]
pub fn is_browser_executable(executable_or_app: &str) -> bool {
    let key = normalize_app_key(executable_or_app);
    BROWSER_EXECUTABLES.contains(&key.as_str())
}

#[must_use]
pub fn normalize_domain(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let lower_raw = text.to_ascii_lowercase();
    if lower_raw.starts_with("file:")
        || lower_raw.starts_with("about:")
        || lower_raw.starts_with("chrome:")
        || lower_raw.starts_with("edge:")
        || lower_raw.starts_with("brave:")
    {
        return None;
    }
    if let Some((_, rest)) = text.split_once("://") {
        text = rest;
    }
    let host_part = text.split(['/', '?', '#']).next().unwrap_or("").trim();
    let without_auth = host_part.rsplit('@').next().unwrap_or(host_part);
    let host_only = without_auth.split(':').next().unwrap_or("").trim();
    let lower = host_only.to_ascii_lowercase();
    let stripped = lower.strip_prefix("www.").unwrap_or(&lower);
    let trimmed = stripped.trim_end_matches('.');
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed.contains('.')
        || trimmed.contains("..")
        || trimmed.starts_with(['.', '-'])
        || trimmed.ends_with(['.', '-'])
    {
        return None;
    }
    let valid = trimmed
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
    if !valid {
        return None;
    }
    let tld = trimmed.rsplit('.').next()?;
    if tld.len() < 2
        || !tld.bytes().all(|b| b.is_ascii_lowercase())
        || NON_DOMAIN_FILE_EXTENSIONS.contains(&tld)
    {
        return None;
    }
    Some(trimmed.to_owned())
}

#[must_use]
pub fn inspect_browser_context(url: Option<&str>, window_title: &str) -> BrowserInspection {
    let mut inspection = BrowserInspection::default();
    if let Some(raw_url) = url {
        let trimmed = raw_url.trim();
        inspection.domain = normalize_domain(trimmed);
        if let Some(domain) = inspection.domain.as_deref() {
            inspection.search_query = extract_search_from_url(domain, trimmed);
        }
    }
    let clean_title = strip_browser_suffix(window_title);
    if inspection.search_query.is_none() {
        inspection.search_query = extract_search_from_title(&clean_title);
    }
    if inspection.domain.is_none() {
        if let Some((_, engine)) = &inspection.search_query {
            inspection.domain = match engine.as_str() {
                "Google" => Some(String::from("google.com")),
                "Bing" => Some(String::from("bing.com")),
                "DuckDuckGo" => Some(String::from("duckduckgo.com")),
                "Brave" => Some(String::from("search.brave.com")),
                _ => None,
            };
        }
        if inspection.domain.is_none() {
            inspection.domain = infer_domain_from_title(&clean_title);
        }
    }
    inspection
}

fn extract_search_from_url(domain: &str, url: &str) -> Option<(String, String)> {
    let (_, query_part) = url.split_once('?')?;
    let query_part = query_part.split('#').next().unwrap_or(query_part);
    let path_part = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split_once('/')
        .map_or("", |(_, path)| path)
        .split('?')
        .next()
        .unwrap_or("");
    let (engine, param_name): (&str, &str) = if domain == "google.com"
        || domain.starts_with("google.")
        || domain.ends_with(".google.com")
    {
        ("Google", "q")
    } else if domain == "bing.com" || domain.ends_with(".bing.com") {
        ("Bing", "q")
    } else if domain == "duckduckgo.com" {
        ("DuckDuckGo", "q")
    } else if domain == "search.brave.com" {
        ("Brave", "q")
    } else if domain == "youtube.com" && path_part.starts_with("results") {
        ("YouTube", "search_query")
    } else if domain == "github.com" && path_part.starts_with("search") {
        ("GitHub", "q")
    } else if domain == "reddit.com" && path_part.starts_with("search") {
        ("Reddit", "q")
    } else if domain == "yahoo.com" || domain.ends_with(".yahoo.com") {
        ("Yahoo", "p")
    } else {
        return None;
    };

    for pair in query_part.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key == param_name {
            let decoded = decode_url_component(value);
            let normalized = sanitize_label(&decoded, "", MAX_SEARCH_QUERY_CHARS);
            if !normalized.is_empty() {
                return Some((normalized, String::from(engine)));
            }
        }
    }
    None
}

fn decode_url_component(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                    out.push((hi << 4) | lo);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

const fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn strip_browser_suffix(title: &str) -> String {
    let mut cleaned = title.replace('\u{200b}', "");
    loop {
        let trimmed = cleaned.trim();
        let next = [
            " - Google Chrome",
            " - Microsoft Edge",
            " - Brave",
            " — Mozilla Firefox",
            " - Mozilla Firefox",
            " - Arc",
            " - Vivaldi",
            " - Opera",
            " - Chromium",
            " - Zen Browser",
            " - Personal",
            " - InPrivate",
            " - Incognito",
        ]
        .into_iter()
        .find_map(|suffix| trimmed.strip_suffix(suffix));
        if let Some(stripped) = next {
            cleaned = stripped.to_owned();
        } else {
            return trimmed.to_owned();
        }
    }
}

#[must_use]
pub fn format_screen_time(ms: u64) -> String {
    let total_secs = ms / 1_000;
    let hours = total_secs / 3_600;
    let mins = (total_secs % 3_600) / 60;
    if hours > 0 {
        format!("{hours}h {mins:02}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else if total_secs > 0 {
        format!("{total_secs}s")
    } else {
        String::from("0m")
    }
}

#[must_use]
pub fn format_compact_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 10_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[must_use]
pub const fn estimated_words(keystrokes: u64) -> u64 {
    keystrokes / 5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u8) -> DayKey {
        DayKey {
            year: 2026,
            month: 11,
            day: d,
        }
    }

    #[test]
    fn normalizes_app_keys_and_domains() {
        assert_eq!(
            normalize_app_key(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
            "chrome"
        );
        assert_eq!(normalize_app_key("Code.EXE"), "code");
        assert!(is_browser_executable("chrome.exe"));
        assert!(is_browser_executable("msedge"));
        assert!(!is_browser_executable("notepad.exe"));

        assert_eq!(
            normalize_domain("https://www.YouTube.com/watch?v=123").as_deref(),
            Some("youtube.com")
        );
        assert_eq!(
            normalize_domain("docs.rs:443/tokio").as_deref(),
            Some("docs.rs")
        );
        assert_eq!(normalize_domain("not a domain"), None);
    }

    #[test]
    fn extracts_domains_and_search_queries_from_urls_and_titles() {
        let google_url = inspect_browser_context(
            Some("https://www.google.com/search?q=rust+win32+ui+automation&ie=UTF-8"),
            "rust win32 ui automation - Google Search - Google Chrome",
        );
        assert_eq!(google_url.domain.as_deref(), Some("google.com"));
        assert_eq!(
            google_url.search_query,
            Some((
                String::from("rust win32 ui automation"),
                String::from("Google")
            ))
        );

        let yt_url = inspect_browser_context(
            Some("https://www.youtube.com/results?search_query=lofi%20beats"),
            "lofi beats - YouTube - Microsoft\u{200b} Edge",
        );
        assert_eq!(yt_url.domain.as_deref(), Some("youtube.com"));
        assert_eq!(
            yt_url.search_query,
            Some((String::from("lofi beats"), String::from("YouTube")))
        );

        let title_only = inspect_browser_context(None, "otto-os/main - GitHub - Google Chrome");
        assert_eq!(title_only.domain.as_deref(), Some("github.com"));
        assert_eq!(title_only.search_query, None);

        let title_search =
            inspect_browser_context(None, "how to focus - Penelusuran Google - Brave");
        assert_eq!(title_search.domain.as_deref(), Some("google.com"));
        assert_eq!(
            title_search.search_query,
            Some((String::from("how to focus"), String::from("Google")))
        );
    }

    #[test]
    fn tracks_active_time_opens_clicks_typing_and_pauses_when_idle() {
        let mut state = FocusActivityState::default();
        state.observe_foreground(day(22), "code.exe", "Visual Studio Code", None);
        state.record_sample(day(22), 1_000, 0, true, 12, 85);
        state.record_sample(day(22), 6_000, 1_000, true, 5, 20);
        state.record_sample(day(22), 12_000, FOCUS_IDLE_TIMEOUT_MS, true, 0, 0);

        let chrome_site = inspect_browser_context(
            Some("https://www.youtube.com/watch?v=abc"),
            "Video - YouTube - Google Chrome",
        );
        state.observe_foreground(day(22), "chrome.exe", "Google Chrome", Some(chrome_site));
        state.record_sample(day(22), 15_000, 0, false, 3, 10);

        assert_eq!(state.today.total_active_ms, 8_000);
        assert_eq!(state.today.focus_session_ms, 5_000);
        assert_eq!(state.today.total_clicks, 20);
        assert_eq!(state.today.total_keystrokes, 115);

        let apps = state.today.sorted_apps();
        assert_eq!(apps[0].app_key, "code");
        assert_eq!(apps[0].active_ms, 5_000);
        assert_eq!(apps[0].open_count, 1);
        assert_eq!(apps[0].click_count, 17);
        assert_eq!(apps[0].keystroke_count, 105);

        assert_eq!(state.today.websites.len(), 1);
        assert_eq!(state.today.websites[0].domain, "youtube.com");
        assert_eq!(state.today.websites[0].active_ms, 3_000);
        assert_eq!(state.today.websites[0].visit_count, 1);
    }

    #[test]
    fn blocklist_matches_apps_subdomains_and_notifications() {
        let mut state = FocusActivityState::default();
        assert!(state.blocklist.add_app("Discord.exe"));
        assert!(state.blocklist.add_website("youtube.com"));
        assert!(state.blocklist.add_website("x.com"));

        assert_eq!(
            state.evaluate_block(false, "discord.exe", "Discord", None),
            None
        );
        assert_eq!(
            state.evaluate_block(true, "discord.exe", "Discord", None),
            Some(FocusBlockEnforcement::App {
                label: String::from("Discord")
            })
        );
        assert_eq!(
            state.evaluate_block(true, "chrome.exe", "Chrome", Some("music.youtube.com")),
            Some(FocusBlockEnforcement::Website {
                domain: String::from("music.youtube.com")
            })
        );
        assert_eq!(
            state.evaluate_block(true, "chrome.exe", "Chrome", Some("twitter.com")),
            Some(FocusBlockEnforcement::Website {
                domain: String::from("twitter.com")
            })
        );
        assert_eq!(state.today.blocked_attempts, 3);

        assert!(state.should_block_notification(true, false, "Slack"));
        assert!(!state.should_block_notification(true, true, "Teams"));
        assert!(state.should_block_notification(true, true, "Discord"));
        assert!(!state.should_block_notification(false, false, "Discord"));
    }

    #[test]
    fn rolls_over_seven_days_and_roundtrips_codec() {
        let mut state = FocusActivityState::default();
        assert!(state.blocklist.add_app("steam"));
        assert!(state.blocklist.add_website("reddit.com"));
        for d in 10..=19 {
            state.observe_foreground(day(d), "code.exe", "VS Code", None);
            state.record_sample(day(d), u64::from(d) * 10_000, 0, true, 4, 20);
            state.record_sample(day(d), u64::from(d) * 10_000 + 2_000, 0, true, 2, 10);
        }
        assert_eq!(state.today.day, day(19));
        assert_eq!(state.history.len(), FOCUS_HISTORY_DAYS);

        let encoded = state.encode();
        let decoded = FocusActivityState::decode(&encoded);
        assert_eq!(decoded.today, state.today);
        assert_eq!(decoded.history, state.history);
        assert_eq!(decoded.blocklist, state.blocklist);

        let summary = decoded.rolling_summary();
        assert_eq!(summary.days_counted, FOCUS_HISTORY_DAYS);
        assert_eq!(summary.total_clicks, 6 * FOCUS_HISTORY_DAYS as u64);
    }

    #[test]
    fn canonicalizes_packaged_aumids_and_filters_system_services_and_docs() {
        assert_eq!(
            normalize_app_key("aumid:claude_pzs8sxrjxfjjc!claude"),
            "claude"
        );
        assert_eq!(
            normalize_app_key("aumid:5319275a.whatsappdesktop_cv1g1gvanyjgm!app"),
            "whatsapp"
        );
        assert_eq!(normalize_app_key("WhatsApp.Root.exe"), "whatsapp");
        assert_eq!(
            normalize_app_key("aumid:microsoft.windowsterminal_8wekyb3d8bbwe!app"),
            "windowsterminal"
        );
        assert_eq!(normalize_app_key("Apple Music"), "applemusic");
        assert_eq!(normalize_app_key("AppleMusic.exe"), "applemusic");

        assert_eq!(
            clean_app_display_name("WhatsApp.Root", "whatsapp"),
            "WhatsApp"
        );
        assert_eq!(
            clean_app_display_name("Notespace deskto", "notespace-desktop"),
            "Notespace Desktop"
        );
        assert_eq!(
            clean_app_display_name("Herdr app", "herdr-app"),
            "Herdr App"
        );

        for (raw_key, title) in [
            (
                r"C:\Windows\System32\appverif.exe",
                "Application Verifier (X64)",
            ),
            (r"C:\WINDOWS\system32\control.exe", "Windows Tools"),
            ("applicationframehost", "ApplicationFrame"),
            ("explorer", "File Explorer"),
            ("taskmgr", "Taskmgr"),
            ("msiexec", "Msiexec"),
            ("otto-os-setup-1.2.0-x64.tmp", "Otto os setup 1."),
            ("systemsettings", "Settings"),
            ("cmake-doc", "CMake Documentation"),
            ("uwp-doc", "Documentation for UWP Apps"),
            ("desktop-doc", "Documentation for Desktop Apps"),
            ("winrar-help", "WinRAR help"),
            ("java-update", "Check For Updates"),
            ("comexp.msc", "Component Services"),
            ("compmgmt.msc", "Computer Management"),
            ("cleanmgr", "Disk Cleanup"),
            ("eventvwr", "Event Viewer"),
            ("wf.msc", "Windows Defender Firewall with Advanced Security"),
            ("mdsched", "Windows Memory Diagnostic"),
            ("powershell_ise", "Windows PowerShell ISE 5.1"),
            ("sechealthui", "Windows Security"),
            ("sdk", "Windows Software Development Kit"),
            ("documents", "Documents"),
            ("career", "career"),
        ] {
            assert!(
                is_non_trackable_focus_app(raw_key, title),
                "expected ({raw_key}, {title}) to be filtered out"
            );
        }

        for (raw_key, title) in [
            ("claude", "Claude"),
            ("brave", "Brave"),
            ("applemusic", "Apple Music"),
            ("cmake-gui", "CMake (cmake-gui)"),
            ("docker", "Docker Desktop"),
            ("dolby", "Dolby Access"),
            ("excel", "Excel"),
            ("winrar", "WinRAR"),
            ("winword", "Word"),
            ("code", "Visual Studio Code"),
            ("discord", "Discord"),
        ] {
            assert!(
                !is_non_trackable_focus_app(raw_key, title),
                "expected ({raw_key}, {title}) to be kept as user app"
            );
        }

        assert_eq!(normalize_domain("ijazah-500kb.pdf"), None);
        assert_eq!(normalize_domain("otto-os-setup-1.2.0-x64.exe"), None);
        assert_eq!(normalize_domain("file:///C:/Users/me/doc.pdf"), None);
    }

    #[test]
    fn decode_scrubs_system_entries_and_merges_canonical_duplicates() {
        let raw = "\
v1
CONF\t1\t1
BAPP\twhatsapp
DAY\t2026-11-23\t30000\t10000\t10\t50\t0
APP\taumid:claude_pzs8sxrjxfjjc!claude\tClaude\t20000\t2\t5\t25
APP\tclaude\tClaude\t5000\t1\t2\t10
APP\tapplicationframehost\tApplicationFrame\t3000\t1\t1\t0
APP\ttaskmgr\tTaskmgr\t2000\t1\t0\t0
WEB\tgithub.com\t15000\t2
WEB\tijazah-500kb.pdf\t8000\t1
";
        let state = FocusActivityState::decode(raw);
        assert!(state.is_dirty());
        assert_eq!(state.today.apps.len(), 1);
        assert_eq!(state.today.apps[0].app_key, "claude");
        assert_eq!(state.today.apps[0].display_name, "Claude");
        assert_eq!(state.today.apps[0].active_ms, 25_000);
        assert_eq!(state.today.apps[0].open_count, 3);
        assert_eq!(state.today.websites.len(), 1);
        assert_eq!(state.today.websites[0].domain, "github.com");
    }
}
