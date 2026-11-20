use crate::host::RuntimeContext;
use otto_core::{Appearance, ResolvedTheme, ThemeMode};

pub fn theme_mode(runtime: &RuntimeContext) -> ThemeMode {
    runtime.theme_mode.get()
}

pub fn set_theme_mode(runtime: &RuntimeContext, mode: ThemeMode) {
    runtime.theme_mode.set(mode);
    runtime.pending_theme_mode.set(Some(mode));
    refresh_theme(runtime);
}

pub fn cycle_theme_mode(runtime: &RuntimeContext) -> ThemeMode {
    let next = theme_mode(runtime).next();
    set_theme_mode(runtime, next);
    next
}

pub fn appearance(runtime: &RuntimeContext) -> Appearance {
    let system = runtime.system_appearance.get();
    if system.contrast.is_none() && runtime.liquid_glass_pref.get() {
        Appearance::glass(ResolvedTheme::Dark)
    } else {
        Appearance {
            theme: ThemeMode::Auto.resolve(system.dark),
            transparency: false,
            contrast: system.contrast,
        }
    }
}

pub fn refresh_system_appearance(runtime: &RuntimeContext) {
    let latest = otto_platform::read_system_appearance();
    if latest != runtime.system_appearance.get() {
        runtime.system_appearance.set(latest);
        sync_effective_glass(runtime);
    }
}

pub fn sync_effective_glass(runtime: &RuntimeContext) {
    let effective = runtime.blur_supported.get()
        && runtime.liquid_glass_pref.get()
        && runtime.system_appearance.get().contrast.is_none();
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

pub fn refresh_theme(runtime: &RuntimeContext) {
    let appearance = appearance(runtime);
    crate::bar::apply_appearance(runtime, appearance);
    crate::dock::repaint(runtime);
    crate::menu::repaint(runtime);
    crate::control_center::repaint(runtime);
    crate::spotlight::repaint(runtime);
    crate::window_controls::apply_appearance(runtime, appearance);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_cycles_and_stores() {
        let runtime = RuntimeContext::new(false);
        set_theme_mode(&runtime, ThemeMode::Auto);
        assert_eq!(theme_mode(&runtime), ThemeMode::Auto);
        assert_eq!(cycle_theme_mode(&runtime), ThemeMode::Dark);
        assert_eq!(theme_mode(&runtime), ThemeMode::Dark);
        assert_eq!(cycle_theme_mode(&runtime), ThemeMode::Light);
        assert_eq!(theme_mode(&runtime), ThemeMode::Light);
        assert_eq!(cycle_theme_mode(&runtime), ThemeMode::Auto);
        assert_eq!(theme_mode(&runtime), ThemeMode::Auto);
    }

    #[test]
    fn appearance_uses_clear_dark_glass_when_liquid_glass_pref_enabled_without_contrast() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: false,
            transparency: false,
            contrast: None,
        });
        runtime.liquid_glass_pref.set(true);
        set_theme_mode(&runtime, ThemeMode::Light);
        let resolved = appearance(&runtime);
        assert_eq!(resolved.theme, ResolvedTheme::Dark);
        assert_eq!(resolved.variant(), otto_core::MaterialVariant::Glass);
    }

    #[test]
    fn appearance_falls_back_to_opaque_or_high_contrast_when_disabled_or_high_contrast() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: false,
            transparency: true,
            contrast: None,
        });
        runtime.liquid_glass_pref.set(false);
        let opaque = appearance(&runtime);
        assert_eq!(opaque.theme, ResolvedTheme::Light);
        assert_eq!(opaque.variant(), otto_core::MaterialVariant::Opaque);

        runtime.liquid_glass_pref.set(true);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: true,
            transparency: true,
            contrast: Some(otto_core::ContrastColors::BLACK_THEME),
        });
        let contrast = appearance(&runtime);
        assert_eq!(contrast.variant(), otto_core::MaterialVariant::HighContrast);
    }

    #[test]
    fn sync_effective_glass_tracks_blur_pref_and_contrast_ignoring_system_transparency() {
        let runtime = RuntimeContext::new(false);
        runtime.blur_supported.set(true);
        runtime.liquid_glass_pref.set(true);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: false,
            transparency: false,
            contrast: None,
        });
        sync_effective_glass(&runtime);
        assert!(runtime.glass_enabled.get());

        runtime.liquid_glass_pref.set(false);
        sync_effective_glass(&runtime);
        assert!(!runtime.glass_enabled.get());

        runtime.liquid_glass_pref.set(true);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: true,
            transparency: true,
            contrast: Some(otto_core::ContrastColors::BLACK_THEME),
        });
        sync_effective_glass(&runtime);
        assert!(!runtime.glass_enabled.get());
    }
}
