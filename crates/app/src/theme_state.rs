use crate::host::RuntimeContext;
use otto_core::{Appearance, ThemeMode};

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
    runtime.system_appearance.get().resolve(ThemeMode::Auto)
}

pub fn refresh_system_appearance(runtime: &RuntimeContext) {
    let latest = otto_platform::read_system_appearance();
    if latest != runtime.system_appearance.get() {
        runtime.system_appearance.set(latest);
        sync_effective_glass(runtime);
    }
}

pub fn sync_effective_glass(runtime: &RuntimeContext) {
    let effective = runtime.blur_supported.get() && runtime.system_appearance.get().transparency;
    if effective != runtime.glass_enabled.get() {
        runtime.glass_enabled.set(effective);
        crate::bar::set_glass_enabled(runtime, effective);
        crate::dock::set_glass_enabled(runtime, effective);
        crate::control_center::set_glass_enabled(runtime, effective);
        crate::menu::set_glass_enabled(runtime, effective);
    }
    refresh_theme(runtime);
}

pub fn refresh_theme(runtime: &RuntimeContext) {
    let appearance = appearance(runtime);
    crate::bar::apply_appearance(runtime, appearance);
    crate::dock::repaint(runtime);
    crate::menu::repaint(runtime);
    crate::control_center::repaint(runtime);
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
    fn appearance_ignores_legacy_explicit_modes_and_follows_the_system() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: true,
            transparency: false,
            contrast: None,
        });
        set_theme_mode(&runtime, ThemeMode::Light);
        let resolved = appearance(&runtime);
        assert_eq!(resolved.theme, otto_core::ResolvedTheme::Dark);
        assert_eq!(resolved.variant(), otto_core::MaterialVariant::Opaque);
    }

    #[test]
    fn appearance_keeps_clear_glass_independent_of_the_legacy_preference() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: false,
            transparency: true,
            contrast: None,
        });
        runtime.liquid_glass_pref.set(true);
        assert_eq!(
            appearance(&runtime).variant(),
            otto_core::MaterialVariant::Glass
        );
        runtime.liquid_glass_pref.set(false);
        assert_eq!(
            appearance(&runtime).variant(),
            otto_core::MaterialVariant::Glass
        );
    }
}
