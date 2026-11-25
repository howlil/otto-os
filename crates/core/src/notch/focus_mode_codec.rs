use super::focus_activity::normalize_app_key;
use super::focus_mode::{
    BackgroundReliefPolicy, CategoryColor, CategoryIcon, FocusCategory, FocusCategoryStore,
    FocusRulePolicy, MAX_CATEGORY_APPS, MAX_FOCUS_CATEGORIES, sanitize_category_name,
};

impl FocusCategoryStore {
    pub fn encode_lines(&self, lines: &mut Vec<String>) {
        for cat in &self.categories {
            lines.push(format!(
                "FCAT\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                sanitize_tab(&cat.id),
                sanitize_tab(&cat.name),
                cat.icon.code(),
                cat.color.code(),
                cat.focus_minutes,
                cat.short_break_minutes,
                cat.long_break_minutes,
                cat.cycles_before_long_break,
                cat.policy.code(),
                cat.background_policy.code(),
            ));
            for app in &cat.allowed_apps {
                lines.push(format!(
                    "FAPP\t{}\tallow\t{}",
                    sanitize_tab(&cat.id),
                    sanitize_tab(app)
                ));
            }
            for app in &cat.blocked_apps {
                lines.push(format!(
                    "FAPP\t{}\tblock\t{}",
                    sanitize_tab(&cat.id),
                    sanitize_tab(app)
                ));
            }
            for app in &cat.close_on_focus_apps {
                lines.push(format!(
                    "FAPP\t{}\tclose\t{}",
                    sanitize_tab(&cat.id),
                    sanitize_tab(app)
                ));
            }
            for site in &cat.blocked_websites {
                lines.push(format!(
                    "FWEB\t{}\tblock\t{}",
                    sanitize_tab(&cat.id),
                    sanitize_tab(site)
                ));
            }
        }
        lines.push(format!("FSEL\t{}", sanitize_tab(&self.active_category_id)));
    }

    pub fn decode_line(&mut self, parts: &[&str], saw_custom: &mut bool) -> bool {
        match parts.first().copied() {
            Some("FCAT") if parts.len() >= 11 => {
                if !*saw_custom {
                    self.categories.clear();
                    *saw_custom = true;
                }
                if self.categories.len() >= MAX_FOCUS_CATEGORIES {
                    return true;
                }
                let id = sanitize_category_id(parts[1]);
                let name = sanitize_category_name(parts[2]);
                if id.is_empty() || name.is_empty() || self.categories.iter().any(|c| c.id == id) {
                    return true;
                }
                self.categories.push(FocusCategory {
                    id,
                    name,
                    icon: CategoryIcon::from_code(parts[3]),
                    color: CategoryColor::from_code(parts[4]),
                    focus_minutes: parts[5].parse().unwrap_or(25).clamp(1, 180),
                    short_break_minutes: parts[6].parse().unwrap_or(5).clamp(1, 60),
                    long_break_minutes: parts[7].parse().unwrap_or(15).clamp(1, 90),
                    cycles_before_long_break: parts[8].parse().unwrap_or(4).clamp(2, 8),
                    policy: FocusRulePolicy::from_code(parts[9]),
                    background_policy: BackgroundReliefPolicy::from_code(parts[10]),
                    allowed_apps: Vec::new(),
                    blocked_apps: Vec::new(),
                    close_on_focus_apps: Vec::new(),
                    blocked_websites: Vec::new(),
                });
                true
            }
            Some("FAPP") if parts.len() >= 4 => {
                let id = sanitize_category_id(parts[1]);
                if let Some(cat) = self.by_id_mut(&id) {
                    let key = normalize_app_key(parts[3]);
                    if !key.is_empty() {
                        match parts[2] {
                            "allow"
                                if !cat.allowed_apps.contains(&key)
                                    && cat.allowed_apps.len() < MAX_CATEGORY_APPS =>
                            {
                                cat.allowed_apps.push(key);
                            }
                            "block"
                                if !cat.blocked_apps.contains(&key)
                                    && cat.blocked_apps.len() < MAX_CATEGORY_APPS =>
                            {
                                cat.blocked_apps.push(key);
                            }
                            "close"
                                if !cat.close_on_focus_apps.contains(&key)
                                    && cat.close_on_focus_apps.len() < MAX_CATEGORY_APPS =>
                            {
                                cat.close_on_focus_apps.push(key);
                            }
                            _ => {}
                        }
                    }
                }
                true
            }
            Some("FWEB") if parts.len() >= 4 => {
                let id = sanitize_category_id(parts[1]);
                if let Some(cat) = self.by_id_mut(&id) {
                    let _ = cat.add_blocked_website(parts[3]);
                }
                true
            }
            Some("FSEL") if parts.len() >= 2 => {
                let id = sanitize_category_id(parts[1]);
                if !id.is_empty() {
                    self.active_category_id = id;
                }
                true
            }
            _ => false,
        }
    }
}

fn sanitize_category_id(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(24)
        .collect()
}

fn sanitize_tab(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !ch.is_control() && *ch != '\t')
        .collect()
}

pub(super) fn unique_category_id(name: &str, existing: &[FocusCategory]) -> String {
    let base: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(18)
        .collect();
    let stem = if base.is_empty() {
        String::from("mode")
    } else {
        base
    };
    if !existing.iter().any(|c| c.id == stem) {
        return stem;
    }
    for suffix in 2..=99 {
        let candidate = format!("{stem}-{suffix}");
        if !existing.iter().any(|c| c.id == candidate) {
            return candidate;
        }
    }
    format!("mode-{}", existing.len() + 1)
}
