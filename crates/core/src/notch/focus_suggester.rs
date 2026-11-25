use std::collections::BTreeMap;

use crate::notch::focus_activity::{
    AppUsageEntry, clean_app_display_name, is_browser_executable, is_non_trackable_focus_app,
    normalize_app_key,
};
use crate::notch::focus_mode::{
    BackgroundReliefPolicy, CategoryColor, CategoryIcon, FocusCategory, FocusRulePolicy,
    MAX_CATEGORY_APPS,
};
use crate::runtime::process_hog::{AuditedAppGroup, ProcessSafety};
use crate::settings::focus_settings::FocusCatalogApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArchetype {
    Coding,
    Study,
    Work,
    Creative,
    Writing,
    Hobby,
    General,
}

impl FocusArchetype {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Coding => "Coding & Engineering",
            Self::Study => "Study & Learning",
            Self::Work => "Work & Productivity",
            Self::Creative => "Design & Creative",
            Self::Writing => "Writing & Research",
            Self::Hobby => "Hobby & Personal",
            Self::General => "General Deep Focus",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSuggestionBucket {
    Allow,
    Block,
    BackgroundRelief,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSuggestionItem {
    pub app_key: String,
    pub display_name: String,
    pub bucket: AppSuggestionBucket,
    pub reason: &'static str,
    pub running_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategorySuggestion {
    pub archetype: FocusArchetype,
    pub suggested_color: CategoryColor,
    pub suggested_icon: CategoryIcon,
    pub suggested_focus_min: u32,
    pub suggested_short_break_min: u32,
    pub suggested_long_break_min: u32,
    pub suggested_policy: FocusRulePolicy,
    pub suggested_bg_policy: BackgroundReliefPolicy,
    pub allow_apps: Vec<AppSuggestionItem>,
    pub block_apps: Vec<AppSuggestionItem>,
    pub background_apps: Vec<AppSuggestionItem>,
    pub block_websites: Vec<String>,
}

const DEV_APPS: &[&str] = &[
    "code",
    "cursor",
    "windsurf",
    "devenv",
    "idea64",
    "rustrover",
    "pycharm64",
    "pycharm",
    "webstorm64",
    "webstorm",
    "clion64",
    "goland64",
    "rider64",
    "studio64",
    "windowsterminal",
    "wt",
    "powershell",
    "pwsh",
    "cmd",
    "mintty",
    "alacritty",
    "wezterm-gui",
    "kitty",
    "githubdesktop",
    "gitkraken",
    "sourcetree",
    "postman",
    "insomnia",
    "dbeaver",
    "pgadmin4",
    "datagrip64",
    "docker desktop",
    "sublime_text",
    "notepad++",
    "zed",
    "neovim",
    "nvim",
    "gvim",
];

const STUDY_APPS: &[&str] = &[
    "obsidian",
    "notion",
    "anki",
    "onenote",
    "winword",
    "powerpnt",
    "excel",
    "acrobat",
    "acrord32",
    "sumatrapdf",
    "foxitpdfreader",
    "zotero",
    "mendeley",
    "logseq",
    "remnote",
    "soffice",
    "libreoffice",
    "calibre",
    "kindle",
    "xodo",
    "drawboardpdf",
];

const WORK_APPS: &[&str] = &[
    "winword",
    "excel",
    "powerpnt",
    "outlook",
    "olk",
    "thunderbird",
    "slack",
    "ms-teams",
    "teams",
    "zoom",
    "notion",
    "obsidian",
    "onenote",
    "linear",
    "trello",
    "asana",
    "acrobat",
    "sumatrapdf",
];

const CREATIVE_APPS: &[&str] = &[
    "figma",
    "photoshop",
    "illustrator",
    "indesign",
    "afterfx",
    "premiere",
    "adobe premiere pro",
    "resolve",
    "blender",
    "gimp-2.10",
    "gimp",
    "inkscape",
    "krita",
    "aseprite",
    "clipstudiopaint",
    "fl64",
    "ableton live",
    "audacity",
    "obs64",
    "capcut",
    "affinitydesigner",
    "affinityphoto",
];

const WRITING_APPS: &[&str] = &[
    "obsidian",
    "notion",
    "winword",
    "typora",
    "scrivener",
    "ulysses",
    "logseq",
    "notepad++",
    "notepad",
    "soffice",
    "zotero",
];

const GAME_AND_LAUNCHER_APPS: &[&str] = &[
    "steam",
    "steamwebhelper",
    "epicgameslauncher",
    "riotclientservices",
    "riotclientux",
    "valorant",
    "leagueclient",
    "robloxplayerbeta",
    "eadesktop",
    "origin",
    "ubisoftconnect",
    "upc",
    "battle.net",
    "gog galaxy",
    "xboxapp",
    "playnite",
    "minecraft",
    "genshinimpact",
    "overwolf",
];

const SOCIAL_CHAT_APPS: &[&str] = &[
    "discord",
    "whatsapp",
    "telegram",
    "line",
    "messenger",
    "signal",
    "wechat",
    "viber",
    "kakaotalk",
];

const MEDIA_ENTERTAINMENT_APPS: &[&str] = &[
    "spotify",
    "applemusic",
    "netflix",
    "vlc",
    "mpv",
    "potplayermini64",
    "twitch",
];

#[must_use]
pub fn classify_archetype(raw_name: &str) -> FocusArchetype {
    let lower = raw_name.trim().to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "cod", "dev", "prog", "ngod", "rust", "web", "back", "front", "soft", "eng", "debug",
            "hack", "algo", "api", "app", "tech",
        ],
    ) {
        return FocusArchetype::Coding;
    }
    if contains_any(
        &lower,
        &[
            "belaj", "stud", "kuli", "skrip", "exam", "uji", "baca", "read", "math", "course",
            "kelas", "tugas", "riset", "research", "school", "sekolah", "kampus",
        ],
    ) {
        return FocusArchetype::Study;
    }
    if contains_any(
        &lower,
        &[
            "desain", "design", "fig", "art", "edit", "video", "3d", "blend", "foto", "photo",
            "draw", "gambar", "music", "musik", "audio", "creat",
        ],
    ) {
        return FocusArchetype::Creative;
    }
    if contains_any(
        &lower,
        &[
            "tulis", "nulis", "write", "writing", "blog", "novel", "buku", "artikel", "essay",
            "jurnal", "journal", "doc",
        ],
    ) {
        return FocusArchetype::Writing;
    }
    if contains_any(
        &lower,
        &[
            "kerj", "work", "offi", "kant", "meet", "admin", "finan", "report", "bisnis", "client",
            "proyek", "project",
        ],
    ) {
        return FocusArchetype::Work;
    }
    if contains_any(
        &lower,
        &["hobb", "hobi", "side", "personal", "santai", "fun"],
    ) {
        return FocusArchetype::Hobby;
    }
    FocusArchetype::General
}

fn collect_suggestion_candidates(
    catalog: &[FocusCatalogApp],
    recent_apps: &[AppUsageEntry],
    audited: &[AuditedAppGroup],
) -> BTreeMap<String, (String, Option<u64>, bool)> {
    let mut candidates: BTreeMap<String, (String, Option<u64>, bool)> = BTreeMap::new();
    for item in catalog {
        if is_non_trackable_focus_app(&item.block_key, &item.display_name) {
            continue;
        }
        let key = normalize_app_key(&item.block_key);
        if !key.is_empty() {
            candidates
                .entry(key)
                .or_insert_with(|| (item.display_name.clone(), None, false));
        }
    }
    for app in recent_apps {
        if is_non_trackable_focus_app(&app.app_key, &app.display_name) {
            continue;
        }
        let key = normalize_app_key(&app.app_key);
        if !key.is_empty() {
            let label = clean_app_display_name(&app.display_name, &key);
            candidates.entry(key).or_insert((label, None, false));
        }
    }
    for group in audited {
        let key = normalize_app_key(&group.exe_name);
        if key.is_empty() || is_non_trackable_focus_app(&key, &group.display_name) {
            continue;
        }
        let safe_bg = group.safety == ProcessSafety::SafeBackground
            && (group.can_throttle || group.can_terminate);
        candidates
            .entry(key)
            .and_modify(|entry| {
                entry.1 = Some(group.memory_bytes);
                entry.2 |= safe_bg;
            })
            .or_insert_with(|| {
                (
                    group.display_name.clone(),
                    Some(group.memory_bytes),
                    safe_bg,
                )
            });
    }
    candidates
}

fn classify_suggestion_candidates(
    archetype: FocusArchetype,
    candidates: BTreeMap<String, (String, Option<u64>, bool)>,
) -> (
    Vec<AppSuggestionItem>,
    Vec<AppSuggestionItem>,
    Vec<AppSuggestionItem>,
) {
    let mut allow_apps = Vec::new();
    let mut block_apps = Vec::new();
    let mut background_apps = Vec::new();

    for (key, (display_name, mem, safe_bg)) in candidates {
        let lower_name = display_name.to_ascii_lowercase();
        let is_game = matches_any_key(&key, &lower_name, GAME_AND_LAUNCHER_APPS);
        let is_social = matches_any_key(&key, &lower_name, SOCIAL_CHAT_APPS);
        let is_media = matches_any_key(&key, &lower_name, MEDIA_ENTERTAINMENT_APPS);
        let is_browser = is_browser_executable(&key);
        let matches_target = matches_archetype_app(archetype, &key, &lower_name);

        if matches_target {
            allow_apps.push(AppSuggestionItem {
                app_key: key,
                display_name,
                bucket: AppSuggestionBucket::Allow,
                reason: "Matches category tools",
                running_memory_bytes: mem,
            });
            continue;
        }
        if is_browser {
            allow_apps.push(AppSuggestionItem {
                app_key: key,
                display_name,
                bucket: AppSuggestionBucket::Allow,
                reason: "Browser allowed (distracting sites blocked)",
                running_memory_bytes: mem,
            });
            continue;
        }
        if is_game || (is_social && archetype != FocusArchetype::Work) {
            let reason = if is_game {
                "Game / launcher distraction"
            } else {
                "Chat / social distraction"
            };
            block_apps.push(AppSuggestionItem {
                app_key: key.clone(),
                display_name: display_name.clone(),
                bucket: AppSuggestionBucket::Block,
                reason,
                running_memory_bytes: mem,
            });
            if is_game || safe_bg || mem.is_some_and(|b| b >= 80 * 1024 * 1024) {
                background_apps.push(AppSuggestionItem {
                    app_key: key,
                    display_name,
                    bucket: AppSuggestionBucket::BackgroundRelief,
                    reason: "Recommended EcoQoS / background close",
                    running_memory_bytes: mem,
                });
            }
            continue;
        }
        if is_media && matches!(archetype, FocusArchetype::Study | FocusArchetype::Work) {
            block_apps.push(AppSuggestionItem {
                app_key: key.clone(),
                display_name: display_name.clone(),
                bucket: AppSuggestionBucket::Block,
                reason: "Media distraction",
                running_memory_bytes: mem,
            });
            if safe_bg {
                background_apps.push(AppSuggestionItem {
                    app_key: key,
                    display_name,
                    bucket: AppSuggestionBucket::BackgroundRelief,
                    reason: "Throttle in background",
                    running_memory_bytes: mem,
                });
            }
            continue;
        }
        if safe_bg && mem.is_some_and(|b| b >= 120 * 1024 * 1024) {
            background_apps.push(AppSuggestionItem {
                app_key: key,
                display_name,
                bucket: AppSuggestionBucket::BackgroundRelief,
                reason: "Heavy background app (EcoQoS)",
                running_memory_bytes: mem,
            });
        }
    }
    (allow_apps, block_apps, background_apps)
}

#[must_use]
pub fn suggest_for_category(
    name: &str,
    catalog: &[FocusCatalogApp],
    recent_apps: &[AppUsageEntry],
    audited: &[AuditedAppGroup],
) -> CategorySuggestion {
    let archetype = classify_archetype(name);
    let (color, icon, focus_min, short_b, long_b, policy, bg_policy) =
        archetype_defaults(archetype);
    let candidates = collect_suggestion_candidates(catalog, recent_apps, audited);
    let (mut allow_apps, mut block_apps, mut background_apps) =
        classify_suggestion_candidates(archetype, candidates);

    if allow_apps.is_empty() {
        for (key, label) in fallback_allow_apps(archetype) {
            allow_apps.push(AppSuggestionItem {
                app_key: String::from(*key),
                display_name: String::from(*label),
                bucket: AppSuggestionBucket::Allow,
                reason: "Recommended starter tool",
                running_memory_bytes: None,
            });
        }
    }
    if block_apps.is_empty() {
        for (key, label) in [
            ("discord", "Discord"),
            ("steam", "Steam"),
            ("whatsapp", "WhatsApp"),
            ("telegram", "Telegram"),
        ] {
            block_apps.push(AppSuggestionItem {
                app_key: String::from(key),
                display_name: String::from(label),
                bucket: AppSuggestionBucket::Block,
                reason: "Common distraction",
                running_memory_bytes: None,
            });
        }
    }

    background_apps.sort_by(|a, b| {
        b.running_memory_bytes
            .unwrap_or(0)
            .cmp(&a.running_memory_bytes.unwrap_or(0))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    CategorySuggestion {
        archetype,
        suggested_color: color,
        suggested_icon: icon,
        suggested_focus_min: focus_min,
        suggested_short_break_min: short_b,
        suggested_long_break_min: long_b,
        suggested_policy: policy,
        suggested_bg_policy: bg_policy,
        allow_apps,
        block_apps,
        background_apps,
        block_websites: default_blocked_websites(archetype),
    }
}

pub fn apply_suggestion_to_category(cat: &mut FocusCategory, suggestion: &CategorySuggestion) {
    cat.policy = suggestion.suggested_policy;
    cat.background_policy = suggestion.suggested_bg_policy;
    cat.focus_minutes = suggestion.suggested_focus_min;
    cat.short_break_minutes = suggestion.suggested_short_break_min;
    cat.long_break_minutes = suggestion.suggested_long_break_min;
    for item in &suggestion.allow_apps {
        if !cat.allowed_apps.contains(&item.app_key) && cat.allowed_apps.len() < MAX_CATEGORY_APPS {
            cat.allowed_apps.push(item.app_key.clone());
        }
    }
    for item in &suggestion.block_apps {
        if !cat.blocked_apps.contains(&item.app_key) && cat.blocked_apps.len() < MAX_CATEGORY_APPS {
            cat.blocked_apps.push(item.app_key.clone());
        }
    }
    for site in &suggestion.block_websites {
        let _ = cat.add_blocked_website(site);
    }
}

const fn archetype_defaults(
    archetype: FocusArchetype,
) -> (
    CategoryColor,
    CategoryIcon,
    u32,
    u32,
    u32,
    FocusRulePolicy,
    BackgroundReliefPolicy,
) {
    match archetype {
        FocusArchetype::Coding => (
            CategoryColor::Orange,
            CategoryIcon::Code,
            45,
            10,
            20,
            FocusRulePolicy::Allowlist,
            BackgroundReliefPolicy::EcoQosThrottle,
        ),
        FocusArchetype::Study => (
            CategoryColor::Blue,
            CategoryIcon::Book,
            35,
            5,
            15,
            FocusRulePolicy::Allowlist,
            BackgroundReliefPolicy::EcoQosThrottle,
        ),
        FocusArchetype::Work => (
            CategoryColor::Indigo,
            CategoryIcon::Briefcase,
            25,
            5,
            15,
            FocusRulePolicy::Blocklist,
            BackgroundReliefPolicy::EcoQosThrottle,
        ),
        FocusArchetype::Creative => (
            CategoryColor::Pink,
            CategoryIcon::Palette,
            50,
            10,
            20,
            FocusRulePolicy::Allowlist,
            BackgroundReliefPolicy::EcoQosThrottle,
        ),
        FocusArchetype::Writing => (
            CategoryColor::Teal,
            CategoryIcon::Pen,
            30,
            5,
            15,
            FocusRulePolicy::Allowlist,
            BackgroundReliefPolicy::EcoQosThrottle,
        ),
        FocusArchetype::Hobby => (
            CategoryColor::Purple,
            CategoryIcon::Sparkle,
            50,
            10,
            20,
            FocusRulePolicy::Blocklist,
            BackgroundReliefPolicy::ShieldOnly,
        ),
        FocusArchetype::General => (
            CategoryColor::Green,
            CategoryIcon::Sparkle,
            25,
            5,
            15,
            FocusRulePolicy::Blocklist,
            BackgroundReliefPolicy::EcoQosThrottle,
        ),
    }
}

fn matches_archetype_app(archetype: FocusArchetype, key: &str, lower_name: &str) -> bool {
    match archetype {
        FocusArchetype::Coding => matches_any_key(key, lower_name, DEV_APPS),
        FocusArchetype::Study => matches_any_key(key, lower_name, STUDY_APPS),
        FocusArchetype::Work => matches_any_key(key, lower_name, WORK_APPS),
        FocusArchetype::Creative => matches_any_key(key, lower_name, CREATIVE_APPS),
        FocusArchetype::Writing => matches_any_key(key, lower_name, WRITING_APPS),
        FocusArchetype::Hobby => {
            matches_any_key(key, lower_name, CREATIVE_APPS)
                || matches_any_key(key, lower_name, MEDIA_ENTERTAINMENT_APPS)
        }
        FocusArchetype::General => {
            matches_any_key(key, lower_name, STUDY_APPS)
                || matches_any_key(key, lower_name, DEV_APPS)
        }
    }
}

fn fallback_allow_apps(archetype: FocusArchetype) -> &'static [(&'static str, &'static str)] {
    match archetype {
        FocusArchetype::Coding => &[
            ("code", "VS Code"),
            ("windowsterminal", "Windows Terminal"),
            ("chrome", "Chrome"),
        ],
        FocusArchetype::Study => &[
            ("obsidian", "Obsidian"),
            ("notion", "Notion"),
            ("anki", "Anki"),
            ("chrome", "Chrome"),
        ],
        FocusArchetype::Work => &[
            ("winword", "Word"),
            ("excel", "Excel"),
            ("slack", "Slack"),
            ("chrome", "Chrome"),
        ],
        FocusArchetype::Creative => &[
            ("figma", "Figma"),
            ("blender", "Blender"),
            ("photoshop", "Photoshop"),
        ],
        FocusArchetype::Writing => &[
            ("obsidian", "Obsidian"),
            ("notion", "Notion"),
            ("winword", "Word"),
        ],
        FocusArchetype::Hobby | FocusArchetype::General => {
            &[("obsidian", "Obsidian"), ("chrome", "Chrome")]
        }
    }
}

fn default_blocked_websites(archetype: FocusArchetype) -> Vec<String> {
    let list: &[&str] = match archetype {
        FocusArchetype::Hobby => &["x.com", "tiktok.com"],
        FocusArchetype::Work => &[
            "tiktok.com",
            "instagram.com",
            "netflix.com",
            "twitch.tv",
            "9gag.com",
        ],
        _ => &[
            "youtube.com",
            "tiktok.com",
            "instagram.com",
            "x.com",
            "reddit.com",
            "netflix.com",
            "twitch.tv",
        ],
    };
    list.iter().map(|s| String::from(*s)).collect()
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

fn matches_any_key(key: &str, lower_name: &str, list: &[&str]) -> bool {
    list.iter().any(|candidate| {
        key == *candidate
            || key.contains(candidate)
            || (candidate.len() >= 4 && lower_name.contains(candidate))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_app(key: &str, name: &str) -> FocusCatalogApp {
        FocusCatalogApp {
            block_key: String::from(key),
            display_name: String::from(name),
            detail: String::new(),
            icon_source: String::new(),
        }
    }

    #[test]
    fn suggests_dev_tools_and_blocks_games_for_coding_category() {
        let catalog = vec![
            catalog_app("code", "Visual Studio Code"),
            catalog_app("windowsterminal", "Windows Terminal"),
            catalog_app("discord", "Discord"),
            catalog_app("steam", "Steam"),
            catalog_app("chrome", "Google Chrome"),
        ];
        let suggestion = suggest_for_category("Ngoding Rust", &catalog, &[], &[]);
        assert_eq!(suggestion.archetype, FocusArchetype::Coding);
        assert_eq!(suggestion.suggested_policy, FocusRulePolicy::Allowlist);
        assert!(suggestion.allow_apps.iter().any(|a| a.app_key == "code"));
        assert!(
            suggestion
                .allow_apps
                .iter()
                .any(|a| a.app_key == "windowsterminal")
        );
        assert!(suggestion.block_apps.iter().any(|a| a.app_key == "steam"));
        assert!(
            suggestion
                .background_apps
                .iter()
                .any(|a| a.app_key == "steam")
        );
    }

    #[test]
    fn classifies_indonesian_study_category_and_suggests_notes_apps() {
        let catalog = vec![
            catalog_app("obsidian", "Obsidian"),
            catalog_app("anki", "Anki"),
            catalog_app("robloxplayerbeta", "Roblox"),
        ];
        let suggestion = suggest_for_category("Belajar Skripsi", &catalog, &[], &[]);
        assert_eq!(suggestion.archetype, FocusArchetype::Study);
        assert!(
            suggestion
                .allow_apps
                .iter()
                .any(|a| a.app_key == "obsidian")
        );
        assert!(suggestion.allow_apps.iter().any(|a| a.app_key == "anki"));
        assert!(
            suggestion
                .block_apps
                .iter()
                .any(|a| a.app_key == "robloxplayerbeta")
        );
    }
}
