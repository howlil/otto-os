use crate::host::RuntimeContext;
use otto_core::Appearance;

pub(crate) fn appearance(runtime: &RuntimeContext) -> Appearance {
    let system = runtime.system_appearance.get();
    let preferences = runtime.appearance_preferences.get();
    system.resolve_preferences(preferences)
}

pub(crate) fn refresh_system_appearance(runtime: &RuntimeContext) {
    let latest = otto_platform::read_system_appearance();
    if latest != runtime.system_appearance.get() {
        runtime.system_appearance.set(latest);
        sync_effective_glass(runtime);
    }
}

pub(crate) fn sync_effective_glass(runtime: &RuntimeContext) {
    let effective = runtime.blur_supported.get() && appearance(runtime).uses_backdrop();
    if effective != runtime.glass_enabled.get() {
        runtime.glass_enabled.set(effective);
        crate::bar::set_glass_enabled(runtime, effective);
        crate::dock::set_glass_enabled(runtime, effective);
        crate::control_center::set_glass_enabled(runtime, effective);
        crate::menu::set_glass_enabled(runtime, effective);
        crate::spotlight::set_glass_enabled(runtime, effective);
    }
    refresh_theme(runtime);
}

pub(crate) fn refresh_theme(runtime: &RuntimeContext) {
    let appearance = appearance(runtime);
    crate::bar::apply_appearance(runtime, appearance);
    crate::dock::repaint(runtime);
    crate::menu::repaint(runtime);
    crate::control_center::repaint(runtime);
    crate::spotlight::repaint(runtime);
    crate::settings_window::refresh_appearance(runtime);
    crate::finder::refresh_appearance(runtime);
}

#[cfg(test)]
mod tests {
    use super::*;
    use otto_core::{
        AccentColor, AppearancePreferences, AppearanceStyle, ContrastColors, MaterialVariant,
        ResolvedTheme, Rgba, SystemAppearance, ThemeMode,
    };

    #[test]
    fn default_appearance_uses_dark_liquid_glass_with_blue_accent() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(SystemAppearance {
            dark: false,
            transparency: true,
            contrast: None,
            accent: None,
        });
        let resolved = appearance(&runtime);
        assert_eq!(resolved.theme, ResolvedTheme::Dark);
        assert_eq!(resolved.style, AppearanceStyle::LiquidGlass);
        assert_eq!(resolved.accent, AccentColor::Blue);
        assert_eq!(resolved.variant(), MaterialVariant::Glass);
    }

    #[test]
    fn user_preferences_drive_theme_style_and_accent_resolution() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(SystemAppearance {
            dark: false,
            transparency: true,
            contrast: None,
            accent: Some(Rgba::new(0.85, 0.25, 0.35, 1.0)),
        });
        runtime.appearance_preferences.set(AppearancePreferences {
            style: AppearanceStyle::Tinted,
            theme: ThemeMode::Auto,
            accent: AccentColor::System,
        });
        let resolved = appearance(&runtime);
        assert_eq!(resolved.theme, ResolvedTheme::Light);
        assert_eq!(resolved.style, AppearanceStyle::Tinted);
        assert_eq!(resolved.accent, AccentColor::System);
        assert_eq!(resolved.variant(), MaterialVariant::Glass);
    }

    #[test]
    fn appearance_falls_back_to_high_contrast_when_contrast_theme_is_active() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(SystemAppearance {
            dark: true,
            transparency: true,
            contrast: Some(ContrastColors::BLACK_THEME),
            accent: None,
        });
        let contrast = appearance(&runtime);
        assert_eq!(contrast.variant(), MaterialVariant::HighContrast);
    }

    #[test]
    fn sync_effective_glass_respects_solid_style_system_transparency_and_high_contrast() {
        let runtime = RuntimeContext::new(false);
        runtime.blur_supported.set(true);
        runtime.system_appearance.set(SystemAppearance {
            dark: true,
            transparency: true,
            contrast: None,
            accent: None,
        });
        sync_effective_glass(&runtime);
        assert!(runtime.glass_enabled.get());

        runtime.appearance_preferences.set(AppearancePreferences {
            style: AppearanceStyle::Solid,
            ..AppearancePreferences::default()
        });
        sync_effective_glass(&runtime);
        assert!(!runtime.glass_enabled.get());

        runtime.appearance_preferences.set(AppearancePreferences {
            style: AppearanceStyle::Frosted,
            ..AppearancePreferences::default()
        });
        sync_effective_glass(&runtime);
        assert!(runtime.glass_enabled.get());

        runtime.system_appearance.set(SystemAppearance {
            dark: true,
            transparency: false,
            contrast: None,
            accent: None,
        });
        sync_effective_glass(&runtime);
        assert!(!runtime.glass_enabled.get());

        runtime.system_appearance.set(SystemAppearance {
            dark: true,
            transparency: true,
            contrast: Some(ContrastColors::BLACK_THEME),
            accent: None,
        });
        sync_effective_glass(&runtime);
        assert!(!runtime.glass_enabled.get());
    }
}
