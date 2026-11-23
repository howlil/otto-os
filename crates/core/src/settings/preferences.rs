use crate::appearance::theme::{AccentColor, AppearancePreferences, AppearanceStyle, ThemeMode};
use crate::bar::menu_bar::{BarHideMode, BarItemSet, MenuBarPreferences};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BrandIcon {
    #[default]
    Otto,
    ClassicApple,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotchStyle {
    #[default]
    ClassicNotch,
    DynamicIsland,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppPreferences {
    pub capture_visible: bool,
    pub hotkeys_enabled: bool,
    pub classic_logo: bool,
    pub notch_enabled: bool,
    pub dynamic_island: bool,
    pub onboarding_seen: bool,
    pub privacy_indicators: bool,
    pub clipboard_history: bool,
    pub drop_shelf: bool,
    pub notch_hud: bool,
    pub media_module: bool,
    pub focus_module: bool,
    pub background_cleanup: bool,
    pub menu_bar: MenuBarPreferences,
    pub quick_capture: bool,
    pub appearance: AppearancePreferences,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            capture_visible: false,
            hotkeys_enabled: true,
            classic_logo: false,
            notch_enabled: true,
            dynamic_island: false,
            onboarding_seen: false,
            privacy_indicators: true,
            clipboard_history: true,
            drop_shelf: true,
            notch_hud: true,
            media_module: true,
            focus_module: true,
            background_cleanup: false,
            menu_bar: MenuBarPreferences::default(),
            quick_capture: false,
            appearance: AppearancePreferences::default(),
        }
    }
}

impl AppPreferences {
    #[must_use]
    pub fn parse_config(contents: &str) -> Self {
        let mut prefs = Self::default();
        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("capture_visible=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.capture_visible = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("hotkeys_enabled=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.hotkeys_enabled = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("classic_logo=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.classic_logo = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("notch_enabled=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.notch_enabled = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("dynamic_island=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.dynamic_island = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("onboarding_seen=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.onboarding_seen = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("privacy_indicators=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.privacy_indicators = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("clipboard_history=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.clipboard_history = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("drop_shelf=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.drop_shelf = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("notch_hud=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.notch_hud = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("media_module=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.media_module = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("focus_module=") {
                if let Some(parsed) = parse_bool(value) {
                    prefs.focus_module = parsed;
                }
            } else if let Some(value) = trimmed.strip_prefix("background_cleanup=")
                && let Some(parsed) = parse_bool(value)
            {
                prefs.background_cleanup = parsed;
            } else if let Some(value) = trimmed.strip_prefix("menu_bar_hide=") {
                if let Some(mode) = BarHideMode::parse_config(value) {
                    prefs.menu_bar.hide_mode = mode;
                }
            } else if let Some(value) = trimmed.strip_prefix("menu_bar_hidden=") {
                prefs.menu_bar.hidden_items = BarItemSet::parse_config(value);
            } else if let Some(value) = trimmed.strip_prefix("menu_bar_background=")
                && let Some(parsed) = parse_bool(value)
            {
                prefs.menu_bar.show_background = parsed;
            } else if let Some(value) = trimmed.strip_prefix("menu_bar_keep_notch=")
                && let Some(parsed) = parse_bool(value)
            {
                prefs.menu_bar.keep_notch = parsed;
            } else if let Some(value) = trimmed.strip_prefix("quick_capture=")
                && let Some(parsed) = parse_bool(value)
            {
                prefs.quick_capture = parsed;
            } else if let Some(value) = trimmed.strip_prefix("appearance_style=")
                && let Some(style) = AppearanceStyle::parse_config(value)
            {
                prefs.appearance.style = style;
            } else if let Some(value) = trimmed.strip_prefix("appearance_theme=")
                && let Some(theme) = ThemeMode::parse_config(value)
            {
                prefs.appearance.theme = theme;
            } else if let Some(value) = trimmed.strip_prefix("appearance_accent=")
                && let Some(accent) = AccentColor::parse_config(value)
            {
                prefs.appearance.accent = accent;
            }
        }
        prefs
    }

    #[must_use]
    pub fn config_lines(&self) -> String {
        format!(
            "capture_visible={}\nhotkeys_enabled={}\nclassic_logo={}\nnotch_enabled={}\ndynamic_island={}\nonboarding_seen={}\nprivacy_indicators={}\nclipboard_history={}\ndrop_shelf={}\nnotch_hud={}\nmedia_module={}\nfocus_module={}\nbackground_cleanup={}\nmenu_bar_hide={}\nmenu_bar_hidden={}\nmenu_bar_background={}\nmenu_bar_keep_notch={}\nquick_capture={}\nappearance_style={}\nappearance_theme={}\nappearance_accent={}",
            self.capture_visible,
            self.hotkeys_enabled,
            self.classic_logo,
            self.notch_enabled,
            self.dynamic_island,
            self.onboarding_seen,
            self.privacy_indicators,
            self.clipboard_history,
            self.drop_shelf,
            self.notch_hud,
            self.media_module,
            self.focus_module,
            self.background_cleanup,
            self.menu_bar.hide_mode.config_value(),
            self.menu_bar.hidden_items.config_value(),
            self.menu_bar.show_background,
            self.menu_bar.keep_notch,
            self.quick_capture,
            self.appearance.style.config_value(),
            self.appearance.theme.config_value(),
            self.appearance.accent.config_value(),
        )
    }

    #[must_use]
    pub const fn brand_icon(&self) -> BrandIcon {
        if self.classic_logo {
            BrandIcon::ClassicApple
        } else {
            BrandIcon::Otto
        }
    }

    #[must_use]
    pub const fn notch_style(&self) -> NotchStyle {
        if self.dynamic_island {
            NotchStyle::DynamicIsland
        } else {
            NotchStyle::ClassicNotch
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_use_otto_logo_classic_notch_and_safe_hotkeys() {
        let prefs = AppPreferences::default();
        assert!(!prefs.capture_visible);
        assert!(prefs.hotkeys_enabled);
        assert!(!prefs.classic_logo);
        assert!(prefs.notch_enabled);
        assert!(!prefs.dynamic_island);
        assert!(!prefs.onboarding_seen);
        assert!(prefs.privacy_indicators);
        assert!(prefs.clipboard_history);
        assert!(prefs.drop_shelf);
        assert!(prefs.notch_hud);
        assert!(prefs.media_module);
        assert!(prefs.focus_module);
        assert!(!prefs.background_cleanup);
        assert_eq!(prefs.menu_bar, MenuBarPreferences::default());
        assert!(!prefs.quick_capture);
        assert_eq!(prefs.brand_icon(), BrandIcon::Otto);
        assert_eq!(prefs.notch_style(), NotchStyle::ClassicNotch);
    }

    #[test]
    fn config_round_trips_and_ignores_invalid_and_legacy_values() {
        let prefs = AppPreferences {
            capture_visible: true,
            hotkeys_enabled: false,
            classic_logo: true,
            notch_enabled: false,
            dynamic_island: true,
            onboarding_seen: true,
            privacy_indicators: false,
            clipboard_history: false,
            drop_shelf: false,
            notch_hud: false,
            media_module: false,
            focus_module: false,
            background_cleanup: true,
            menu_bar: MenuBarPreferences {
                hide_mode: BarHideMode::Always,
                hidden_items: BarItemSet::parse_config("tray,clock"),
                show_background: false,
                keep_notch: true,
            },
            quick_capture: true,
            appearance: AppearancePreferences {
                style: AppearanceStyle::Tinted,
                theme: ThemeMode::Auto,
                accent: AccentColor::Emerald,
            },
        };
        assert_eq!(prefs.brand_icon(), BrandIcon::ClassicApple);
        assert_eq!(prefs.notch_style(), NotchStyle::DynamicIsland);
        let text = prefs.config_lines();
        assert_eq!(AppPreferences::parse_config(&text), prefs);

        let partial = "theme_mode=light\ncapture_visible=maybe\nhotkeys_enabled=false\nliquid_glass=false\nappearance_style=bogus\nappearance_theme=neon\nappearance_accent=chartreuse\n";
        let parsed = AppPreferences::parse_config(partial);
        assert!(!parsed.capture_visible);
        assert!(!parsed.hotkeys_enabled);
        assert!(!parsed.classic_logo);
        assert!(parsed.notch_enabled);
        assert!(!parsed.dynamic_island);
        assert!(!parsed.onboarding_seen);
        assert!(parsed.privacy_indicators);
        assert!(parsed.clipboard_history);
        assert!(parsed.drop_shelf);
        assert!(parsed.notch_hud);
        assert!(parsed.media_module);
        assert!(parsed.focus_module);
        assert!(!parsed.background_cleanup);
        assert_eq!(parsed.menu_bar, MenuBarPreferences::default());
        assert!(!parsed.quick_capture);
        assert_eq!(parsed.appearance, AppearancePreferences::default());
    }

    #[test]
    fn menu_bar_keys_ignore_invalid_values_independently() {
        let parsed = AppPreferences::parse_config(
            "menu_bar_hide=sometimes
menu_bar_hidden=search,bogus
menu_bar_background=maybe
",
        );
        assert_eq!(parsed.menu_bar.hide_mode, BarHideMode::FullScreenOnly);
        assert_eq!(parsed.menu_bar.hidden_items.config_value(), "search");
        assert!(parsed.menu_bar.show_background);
    }
}
