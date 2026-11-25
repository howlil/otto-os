use crate::appearance::theme::{AccentColor, AppearancePreferences, AppearanceStyle, ThemeMode};
use crate::bar::menu_bar::{BarHideMode, BarItemSet, MenuBarPreferences};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BrandIcon {
    #[default]
    Otto,
    Windows,
    ClassicApple,
}

impl BrandIcon {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Otto => Self::Windows,
            Self::Windows => Self::ClassicApple,
            Self::ClassicApple => Self::Otto,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Otto => "Otto",
            Self::Windows => "Windows",
            Self::ClassicApple => "Apple",
        }
    }

    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Otto => "otto",
            Self::Windows => "windows",
            Self::ClassicApple => "apple",
        }
    }

    #[must_use]
    pub fn parse_config(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "otto" => Some(Self::Otto),
            "windows" => Some(Self::Windows),
            "apple" | "classic_apple" => Some(Self::ClassicApple),
            _ => None,
        }
    }
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
    pub brand_icon: BrandIcon,
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
    pub finder_enabled: bool,
    pub menu_bar: MenuBarPreferences,
    pub quick_capture: bool,
    pub appearance: AppearancePreferences,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            capture_visible: false,
            hotkeys_enabled: true,
            brand_icon: BrandIcon::Otto,
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
            finder_enabled: true,
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
            if let Some((key, value)) = line.trim().split_once('=') {
                prefs.apply_config_entry(key, value);
            }
        }
        prefs
    }

    fn apply_config_entry(&mut self, key: &str, value: &str) {
        if let Some(flag) = self.bool_field_mut(key) {
            if let Some(parsed) = parse_bool(value) {
                *flag = parsed;
            }
            return;
        }
        match key {
            "brand_icon" => {
                if let Some(icon) = BrandIcon::parse_config(value) {
                    self.brand_icon = icon;
                }
            }
            "classic_logo" => {
                if let Some(classic) = parse_bool(value) {
                    self.brand_icon = if classic {
                        BrandIcon::ClassicApple
                    } else {
                        BrandIcon::Otto
                    };
                }
            }
            "menu_bar_hide" => {
                if let Some(mode) = BarHideMode::parse_config(value) {
                    self.menu_bar.hide_mode = mode;
                }
            }
            "menu_bar_hidden" => self.menu_bar.hidden_items = BarItemSet::parse_config(value),
            "appearance_style" => {
                if let Some(style) = AppearanceStyle::parse_config(value) {
                    self.appearance.style = style;
                }
            }
            "appearance_theme" => {
                if let Some(theme) = ThemeMode::parse_config(value) {
                    self.appearance.theme = theme;
                }
            }
            "appearance_accent" => {
                if let Some(accent) = AccentColor::parse_config(value) {
                    self.appearance.accent = accent;
                }
            }
            _ => {}
        }
    }

    fn bool_field_mut(&mut self, key: &str) -> Option<&mut bool> {
        match key {
            "capture_visible" => Some(&mut self.capture_visible),
            "hotkeys_enabled" => Some(&mut self.hotkeys_enabled),
            "notch_enabled" => Some(&mut self.notch_enabled),
            "dynamic_island" => Some(&mut self.dynamic_island),
            "onboarding_seen" => Some(&mut self.onboarding_seen),
            "privacy_indicators" => Some(&mut self.privacy_indicators),
            "clipboard_history" => Some(&mut self.clipboard_history),
            "drop_shelf" => Some(&mut self.drop_shelf),
            "notch_hud" => Some(&mut self.notch_hud),
            "media_module" => Some(&mut self.media_module),
            "focus_module" => Some(&mut self.focus_module),
            "background_cleanup" => Some(&mut self.background_cleanup),
            "finder_enabled" => Some(&mut self.finder_enabled),
            "menu_bar_background" => Some(&mut self.menu_bar.show_background),
            "menu_bar_keep_notch" => Some(&mut self.menu_bar.keep_notch),
            "quick_capture" => Some(&mut self.quick_capture),
            _ => None,
        }
    }

    #[must_use]
    pub fn config_lines(&self) -> String {
        format!(
            "capture_visible={}\nhotkeys_enabled={}\nbrand_icon={}\nnotch_enabled={}\ndynamic_island={}\nonboarding_seen={}\nprivacy_indicators={}\nclipboard_history={}\ndrop_shelf={}\nnotch_hud={}\nmedia_module={}\nfocus_module={}\nbackground_cleanup={}\nfinder_enabled={}\nmenu_bar_hide={}\nmenu_bar_hidden={}\nmenu_bar_background={}\nmenu_bar_keep_notch={}\nquick_capture={}\nappearance_style={}\nappearance_theme={}\nappearance_accent={}",
            self.capture_visible,
            self.hotkeys_enabled,
            self.brand_icon.config_value(),
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
            self.finder_enabled,
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
        self.brand_icon
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
        assert_eq!(prefs.brand_icon, BrandIcon::Otto);
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
        assert!(prefs.finder_enabled);
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
            brand_icon: BrandIcon::Windows,
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
            finder_enabled: false,
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
        assert_eq!(prefs.brand_icon(), BrandIcon::Windows);
        assert_eq!(prefs.notch_style(), NotchStyle::DynamicIsland);
        let text = prefs.config_lines();
        assert_eq!(AppPreferences::parse_config(&text), prefs);

        let apple_prefs = AppPreferences {
            brand_icon: BrandIcon::ClassicApple,
            ..AppPreferences::default()
        };
        assert_eq!(
            AppPreferences::parse_config(&apple_prefs.config_lines()).brand_icon(),
            BrandIcon::ClassicApple
        );
        assert_eq!(
            AppPreferences::parse_config("classic_logo=true\n").brand_icon(),
            BrandIcon::ClassicApple
        );
        assert_eq!(BrandIcon::Otto.next(), BrandIcon::Windows);
        assert_eq!(BrandIcon::Windows.next(), BrandIcon::ClassicApple);
        assert_eq!(BrandIcon::ClassicApple.next(), BrandIcon::Otto);

        let partial = "theme_mode=light\ncapture_visible=maybe\nhotkeys_enabled=false\nbrand_icon=bogus\nliquid_glass=false\nappearance_style=bogus\nappearance_theme=neon\nappearance_accent=chartreuse\n";
        let parsed = AppPreferences::parse_config(partial);
        assert!(!parsed.capture_visible);
        assert!(!parsed.hotkeys_enabled);
        assert_eq!(parsed.brand_icon, BrandIcon::Otto);
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
        assert!(parsed.finder_enabled);
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
