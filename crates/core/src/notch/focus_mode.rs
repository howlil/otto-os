use crate::notch::focus_activity::{
    is_non_trackable_focus_app, normalize_app_key, normalize_domain,
};
use crate::notch::focus_suggester::CategorySuggestion;

pub const MAX_FOCUS_CATEGORIES: usize = 12;
pub const MAX_CATEGORY_NAME_CHARS: usize = 24;
pub const MAX_CATEGORY_APPS: usize = 64;
pub const MAX_CATEGORY_WEBSITES: usize = 64;
pub const DEFAULT_CODING_CATEGORY_ID: &str = "coding";
pub const DEFAULT_STUDY_CATEGORY_ID: &str = "study";
pub const DEFAULT_WORK_CATEGORY_ID: &str = "work";
pub const DEFAULT_HOBBY_CATEGORY_ID: &str = "hobby";

pub use super::focus_mode_types::CategoryColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CategoryIcon {
    #[default]
    Code,
    Book,
    Briefcase,
    Sparkle,
    Palette,
    Pen,
    Terminal,
    Headphones,
}

impl CategoryIcon {
    pub const ALL: [Self; 8] = [
        Self::Code,
        Self::Book,
        Self::Briefcase,
        Self::Sparkle,
        Self::Palette,
        Self::Pen,
        Self::Terminal,
        Self::Headphones,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Book => "book",
            Self::Briefcase => "briefcase",
            Self::Sparkle => "sparkle",
            Self::Palette => "palette",
            Self::Pen => "pen",
            Self::Terminal => "terminal",
            Self::Headphones => "headphones",
        }
    }

    #[must_use]
    pub fn from_code(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "book" => Self::Book,
            "briefcase" => Self::Briefcase,
            "sparkle" => Self::Sparkle,
            "palette" => Self::Palette,
            "pen" => Self::Pen,
            "terminal" => Self::Terminal,
            "headphones" => Self::Headphones,
            _ => Self::Code,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::Book => "Study",
            Self::Briefcase => "Work",
            Self::Sparkle => "Hobby",
            Self::Palette => "Design",
            Self::Pen => "Write",
            Self::Terminal => "Terminal",
            Self::Headphones => "Audio",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Code => Self::Book,
            Self::Book => Self::Briefcase,
            Self::Briefcase => Self::Sparkle,
            Self::Sparkle => Self::Palette,
            Self::Palette => Self::Pen,
            Self::Pen => Self::Terminal,
            Self::Terminal => Self::Headphones,
            Self::Headphones => Self::Code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusRulePolicy {
    #[default]
    Allowlist,
    Blocklist,
}

impl FocusRulePolicy {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Allowlist => "allow",
            Self::Blocklist => "block",
        }
    }

    #[must_use]
    pub fn from_code(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "block" => Self::Blocklist,
            _ => Self::Allowlist,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Allowlist => "Only allow chosen apps",
            Self::Blocklist => "Block distractions only",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Allowlist => "Allowlist",
            Self::Blocklist => "Blocklist",
        }
    }

    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Allowlist => Self::Blocklist,
            Self::Blocklist => Self::Allowlist,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundReliefPolicy {
    ShieldOnly,
    #[default]
    EcoQosThrottle,
    EcoQosAndClose,
}

impl BackgroundReliefPolicy {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ShieldOnly => "shield",
            Self::EcoQosThrottle => "ecoqos",
            Self::EcoQosAndClose => "close",
        }
    }

    #[must_use]
    pub fn from_code(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "shield" => Self::ShieldOnly,
            "close" => Self::EcoQosAndClose,
            _ => Self::EcoQosThrottle,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ShieldOnly => "Off (Shield only)",
            Self::EcoQosThrottle => "Low Power automatically",
            Self::EcoQosAndClose => "Low Power + Close selected",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::ShieldOnly => "Shield only",
            Self::EcoQosThrottle => "Auto Low Power",
            Self::EcoQosAndClose => "Low Power + Close",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::ShieldOnly => Self::EcoQosThrottle,
            Self::EcoQosThrottle => Self::EcoQosAndClose,
            Self::EcoQosAndClose => Self::ShieldOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusCategory {
    pub id: String,
    pub name: String,
    pub icon: CategoryIcon,
    pub color: CategoryColor,
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub cycles_before_long_break: u8,
    pub policy: FocusRulePolicy,
    pub background_policy: BackgroundReliefPolicy,
    pub allowed_apps: Vec<String>,
    pub blocked_apps: Vec<String>,
    pub close_on_focus_apps: Vec<String>,
    pub blocked_websites: Vec<String>,
}

impl FocusCategory {
    #[must_use]
    pub fn coding_default() -> Self {
        Self {
            id: String::from(DEFAULT_CODING_CATEGORY_ID),
            name: String::from("Coding"),
            icon: CategoryIcon::Code,
            color: CategoryColor::Orange,
            focus_minutes: 45,
            short_break_minutes: 10,
            long_break_minutes: 20,
            cycles_before_long_break: 4,
            policy: FocusRulePolicy::Allowlist,
            background_policy: BackgroundReliefPolicy::EcoQosThrottle,
            allowed_apps: vec![
                String::from("code"),
                String::from("cursor"),
                String::from("windsurf"),
                String::from("devenv"),
                String::from("rustrover"),
                String::from("idea64"),
                String::from("windowsterminal"),
                String::from("wt"),
                String::from("powershell"),
                String::from("pwsh"),
                String::from("postman"),
                String::from("chrome"),
                String::from("msedge"),
                String::from("brave"),
                String::from("firefox"),
            ],
            blocked_apps: vec![
                String::from("discord"),
                String::from("steam"),
                String::from("epicgameslauncher"),
                String::from("riotclientservices"),
                String::from("whatsapp"),
                String::from("telegram"),
            ],
            close_on_focus_apps: Vec::new(),
            blocked_websites: vec![
                String::from("youtube.com"),
                String::from("tiktok.com"),
                String::from("instagram.com"),
                String::from("x.com"),
                String::from("reddit.com"),
            ],
        }
    }

    #[must_use]
    pub fn study_default() -> Self {
        Self {
            id: String::from(DEFAULT_STUDY_CATEGORY_ID),
            name: String::from("Belajar"),
            icon: CategoryIcon::Book,
            color: CategoryColor::Blue,
            focus_minutes: 35,
            short_break_minutes: 5,
            long_break_minutes: 15,
            cycles_before_long_break: 4,
            policy: FocusRulePolicy::Allowlist,
            background_policy: BackgroundReliefPolicy::EcoQosThrottle,
            allowed_apps: vec![
                String::from("obsidian"),
                String::from("notion"),
                String::from("anki"),
                String::from("onenote"),
                String::from("winword"),
                String::from("powerpnt"),
                String::from("sumatrapdf"),
                String::from("acrobat"),
                String::from("chrome"),
                String::from("msedge"),
                String::from("brave"),
                String::from("firefox"),
            ],
            blocked_apps: vec![
                String::from("discord"),
                String::from("steam"),
                String::from("epicgameslauncher"),
                String::from("robloxplayerbeta"),
                String::from("whatsapp"),
                String::from("telegram"),
            ],
            close_on_focus_apps: Vec::new(),
            blocked_websites: vec![
                String::from("youtube.com"),
                String::from("tiktok.com"),
                String::from("instagram.com"),
                String::from("x.com"),
                String::from("netflix.com"),
                String::from("twitch.tv"),
            ],
        }
    }

    #[must_use]
    pub fn work_default() -> Self {
        Self {
            id: String::from(DEFAULT_WORK_CATEGORY_ID),
            name: String::from("Kerja"),
            icon: CategoryIcon::Briefcase,
            color: CategoryColor::Indigo,
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            cycles_before_long_break: 4,
            policy: FocusRulePolicy::Blocklist,
            background_policy: BackgroundReliefPolicy::EcoQosThrottle,
            allowed_apps: vec![
                String::from("winword"),
                String::from("excel"),
                String::from("powerpnt"),
                String::from("outlook"),
                String::from("slack"),
                String::from("ms-teams"),
                String::from("chrome"),
                String::from("msedge"),
            ],
            blocked_apps: vec![
                String::from("steam"),
                String::from("epicgameslauncher"),
                String::from("riotclientservices"),
                String::from("robloxplayerbeta"),
                String::from("discord"),
            ],
            close_on_focus_apps: Vec::new(),
            blocked_websites: vec![
                String::from("tiktok.com"),
                String::from("instagram.com"),
                String::from("netflix.com"),
                String::from("twitch.tv"),
                String::from("9gag.com"),
            ],
        }
    }

    #[must_use]
    pub fn hobby_default() -> Self {
        Self {
            id: String::from(DEFAULT_HOBBY_CATEGORY_ID),
            name: String::from("Hobby"),
            icon: CategoryIcon::Sparkle,
            color: CategoryColor::Purple,
            focus_minutes: 50,
            short_break_minutes: 10,
            long_break_minutes: 20,
            cycles_before_long_break: 4,
            policy: FocusRulePolicy::Blocklist,
            background_policy: BackgroundReliefPolicy::ShieldOnly,
            allowed_apps: vec![
                String::from("figma"),
                String::from("blender"),
                String::from("spotify"),
                String::from("chrome"),
            ],
            blocked_apps: vec![String::from("slack"), String::from("ms-teams")],
            close_on_focus_apps: Vec::new(),
            blocked_websites: vec![String::from("x.com"), String::from("tiktok.com")],
        }
    }

    #[must_use]
    pub fn is_app_allowed(&self, app_key_raw: &str) -> bool {
        let key = normalize_app_key(app_key_raw);
        !key.is_empty() && self.allowed_apps.iter().any(|item| item == &key)
    }

    #[must_use]
    pub fn is_app_blocked_explicit(&self, app_key_raw: &str) -> bool {
        let key = normalize_app_key(app_key_raw);
        !key.is_empty() && self.blocked_apps.iter().any(|item| item == &key)
    }

    #[must_use]
    pub fn is_app_close_on_focus(&self, app_key_raw: &str) -> bool {
        let key = normalize_app_key(app_key_raw);
        !key.is_empty() && self.close_on_focus_apps.iter().any(|item| item == &key)
    }

    #[must_use]
    pub fn should_block_app(&self, app_key_raw: &str, display_name: &str) -> bool {
        if is_non_trackable_focus_app(app_key_raw, display_name) {
            return false;
        }
        let key = normalize_app_key(app_key_raw);
        if key.is_empty() {
            return false;
        }
        match self.policy {
            FocusRulePolicy::Allowlist => {
                if self.allowed_apps.is_empty() {
                    return self.is_app_blocked_explicit(&key);
                }
                !self.is_app_allowed(&key)
            }
            FocusRulePolicy::Blocklist => self.is_app_blocked_explicit(&key),
        }
    }

    #[must_use]
    pub fn should_block_website(&self, domain_raw: &str) -> bool {
        let Some(domain) = normalize_domain(domain_raw) else {
            return false;
        };
        self.blocked_websites
            .iter()
            .any(|blocked| domain == *blocked || domain.ends_with(&format!(".{blocked}")))
    }

    pub fn toggle_allowed_app(&mut self, app_key_raw: &str) -> bool {
        toggle_app_list(&mut self.allowed_apps, app_key_raw, MAX_CATEGORY_APPS)
    }

    pub fn toggle_blocked_app(&mut self, app_key_raw: &str) -> bool {
        toggle_app_list(&mut self.blocked_apps, app_key_raw, MAX_CATEGORY_APPS)
    }

    pub fn toggle_close_app(&mut self, app_key_raw: &str) -> bool {
        toggle_app_list(
            &mut self.close_on_focus_apps,
            app_key_raw,
            MAX_CATEGORY_APPS,
        )
    }

    pub fn add_blocked_website(&mut self, domain_raw: &str) -> bool {
        let Some(domain) = normalize_domain(domain_raw) else {
            return false;
        };
        if self.blocked_websites.contains(&domain)
            || self.blocked_websites.len() >= MAX_CATEGORY_WEBSITES
        {
            return false;
        }
        self.blocked_websites.push(domain);
        true
    }

    pub fn toggle_blocked_website(&mut self, domain_raw: &str) -> bool {
        let Some(domain) = normalize_domain(domain_raw) else {
            return false;
        };
        if let Some(pos) = self
            .blocked_websites
            .iter()
            .position(|item| item == &domain)
        {
            self.blocked_websites.remove(pos);
            return true;
        }
        if self.blocked_websites.len() >= MAX_CATEGORY_WEBSITES {
            return false;
        }
        self.blocked_websites.push(domain);
        true
    }

    #[must_use]
    pub fn schedule_summary(&self) -> String {
        format!(
            "{}m focus · {}m/{}m break · {}x",
            self.focus_minutes,
            self.short_break_minutes,
            self.long_break_minutes,
            self.cycles_before_long_break
        )
    }

    #[must_use]
    pub fn policy_summary(&self) -> String {
        match self.policy {
            FocusRulePolicy::Allowlist => format!(
                "{} apps allowed · {}",
                self.allowed_apps.len(),
                self.background_policy.short_label()
            ),
            FocusRulePolicy::Blocklist => format!(
                "{} apps blocked · {}",
                self.blocked_apps.len(),
                self.background_policy.short_label()
            ),
        }
    }

    #[must_use]
    pub fn hero_summary(&self) -> String {
        let rule_phrase = match self.policy {
            FocusRulePolicy::Allowlist => {
                format!("Only {} chosen apps allowed", self.allowed_apps.len())
            }
            FocusRulePolicy::Blocklist => {
                format!("{} distracting apps blocked", self.blocked_apps.len())
            }
        };
        format!(
            "{}m focus · {rule_phrase} · {}",
            self.focus_minutes,
            self.background_policy.label()
        )
    }
}

fn toggle_app_list(list: &mut Vec<String>, raw: &str, max_len: usize) -> bool {
    let key = normalize_app_key(raw);
    if key.is_empty() {
        return false;
    }
    if let Some(pos) = list.iter().position(|item| item == &key) {
        list.remove(pos);
        return true;
    }
    if list.len() >= max_len {
        return false;
    }
    list.push(key);
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusCategoryStore {
    pub categories: Vec<FocusCategory>,
    pub active_category_id: String,
}

impl Default for FocusCategoryStore {
    fn default() -> Self {
        Self {
            categories: vec![
                FocusCategory::coding_default(),
                FocusCategory::study_default(),
                FocusCategory::work_default(),
                FocusCategory::hobby_default(),
            ],
            active_category_id: String::from(DEFAULT_CODING_CATEGORY_ID),
        }
    }
}

impl FocusCategoryStore {
    #[must_use]
    pub fn active(&self) -> &FocusCategory {
        static FALLBACK_CATEGORY: std::sync::OnceLock<FocusCategory> = std::sync::OnceLock::new();
        self.categories
            .iter()
            .find(|cat| cat.id == self.active_category_id)
            .or_else(|| self.categories.first())
            .unwrap_or_else(|| FALLBACK_CATEGORY.get_or_init(FocusCategory::coding_default))
    }

    pub fn active_mut(&mut self) -> &mut FocusCategory {
        if self.categories.is_empty() {
            self.categories.push(FocusCategory::coding_default());
        }
        let index = self
            .categories
            .iter()
            .position(|cat| cat.id == self.active_category_id)
            .unwrap_or(0);
        &mut self.categories[index]
    }

    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&FocusCategory> {
        self.categories.iter().find(|cat| cat.id == id)
    }

    pub fn by_id_mut(&mut self, id: &str) -> Option<&mut FocusCategory> {
        self.categories.iter_mut().find(|cat| cat.id == id)
    }

    pub fn select(&mut self, id: &str) -> bool {
        if self.active_category_id == id || !self.categories.iter().any(|cat| cat.id == id) {
            return false;
        }
        id.clone_into(&mut self.active_category_id);
        true
    }

    pub fn cycle_next(&mut self) -> bool {
        if self.categories.len() <= 1 {
            return false;
        }
        let current = self
            .categories
            .iter()
            .position(|cat| cat.id == self.active_category_id)
            .unwrap_or(0);
        let next = (current + 1) % self.categories.len();
        self.active_category_id = self.categories[next].id.clone();
        true
    }

    pub fn create_category(
        &mut self,
        raw_name: &str,
        suggestion: Option<&CategorySuggestion>,
    ) -> Option<String> {
        if self.categories.len() >= MAX_FOCUS_CATEGORIES {
            return None;
        }
        let name = sanitize_category_name(raw_name);
        if name.is_empty() {
            return None;
        }
        let id = super::focus_mode_codec::unique_category_id(&name, &self.categories);
        let color = suggestion.map_or_else(
            || CategoryColor::ALL[self.categories.len() % CategoryColor::ALL.len()],
            |s| s.suggested_color,
        );
        let icon = suggestion.map_or(CategoryIcon::Sparkle, |s| s.suggested_icon);
        let mut category = FocusCategory {
            id: id.clone(),
            name,
            icon,
            color,
            focus_minutes: suggestion.map_or(25, |s| s.suggested_focus_min),
            short_break_minutes: suggestion.map_or(5, |s| s.suggested_short_break_min),
            long_break_minutes: suggestion.map_or(15, |s| s.suggested_long_break_min),
            cycles_before_long_break: 4,
            policy: suggestion.map_or(FocusRulePolicy::Blocklist, |s| s.suggested_policy),
            background_policy: suggestion.map_or(BackgroundReliefPolicy::EcoQosThrottle, |s| {
                s.suggested_bg_policy
            }),
            allowed_apps: Vec::new(),
            blocked_apps: Vec::new(),
            close_on_focus_apps: Vec::new(),
            blocked_websites: Vec::new(),
        };
        if let Some(sug) = suggestion {
            crate::notch::focus_suggester::apply_suggestion_to_category(&mut category, sug);
        }
        self.categories.push(category);
        self.active_category_id.clone_from(&id);
        Some(id)
    }

    pub fn rename_category(&mut self, id: &str, raw_name: &str) -> bool {
        let name = sanitize_category_name(raw_name);
        if name.is_empty()
            || self
                .categories
                .iter()
                .any(|cat| cat.id != id && cat.name.eq_ignore_ascii_case(&name))
        {
            return false;
        }
        let Some(cat) = self.by_id_mut(id) else {
            return false;
        };
        if cat.name == name {
            return false;
        }
        cat.name = name;
        true
    }

    pub fn delete_category(&mut self, id: &str) -> bool {
        if self.categories.len() <= 1 {
            return false;
        }
        let Some(pos) = self.categories.iter().position(|cat| cat.id == id) else {
            return false;
        };
        self.categories.remove(pos);
        if self.active_category_id == id {
            self.active_category_id = self.categories[0].id.clone();
        }
        true
    }

    pub fn ensure_valid(&mut self) {
        if self.categories.is_empty() {
            *self = Self::default();
            return;
        }
        if !self
            .categories
            .iter()
            .any(|cat| cat.id == self.active_category_id)
        {
            self.active_category_id = self.categories[0].id.clone();
        }
    }
}

#[must_use]
pub fn sanitize_category_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| !ch.is_control() && *ch != '\t')
        .take(MAX_CATEGORY_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_mode_allows_selected_and_blocks_others_while_protecting_system() {
        let coding = FocusCategory::coding_default();
        assert!(!coding.should_block_app("code", "Visual Studio Code"));
        assert!(coding.should_block_app("discord", "Discord"));
        assert!(coding.should_block_app("steam", "Steam"));
        assert!(!coding.should_block_app("explorer", "File Explorer"));
        assert!(!coding.should_block_app("otto-os", "Otto OS"));
    }

    #[test]
    fn store_round_trips_categories_and_selection() {
        let mut store = FocusCategoryStore::default();
        let created = store
            .create_category("Skripsi", None)
            .expect("create category");
        assert_eq!(store.active().id, created);
        let mut lines = Vec::new();
        store.encode_lines(&mut lines);

        let mut decoded = FocusCategoryStore::default();
        let mut saw_custom = false;
        for line in &lines {
            let parts: Vec<&str> = line.split('\t').collect();
            assert!(decoded.decode_line(&parts, &mut saw_custom));
        }
        decoded.ensure_valid();
        assert_eq!(decoded.categories.len(), store.categories.len());
        assert_eq!(decoded.active_category_id, created);
    }

    #[test]
    fn rename_and_delete_category_enforce_uniqueness_and_minimum_one_mode() {
        let mut store = FocusCategoryStore::default();
        assert!(store.rename_category("coding", "Backend Rust"));
        assert_eq!(store.active().name, "Backend Rust");
        assert_eq!(store.active().id, "coding");
        assert!(!store.rename_category("coding", ""));
        assert!(!store.rename_category("coding", "belajar"));

        let ids: Vec<String> = store.categories.iter().map(|c| c.id.clone()).collect();
        for id in ids.iter().skip(1) {
            assert!(store.delete_category(id));
        }
        assert_eq!(store.categories.len(), 1);
        assert!(!store.delete_category(&ids[0]));
    }
}
