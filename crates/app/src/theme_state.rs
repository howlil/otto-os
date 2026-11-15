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
    runtime.system_appearance.get().resolve(theme_mode(runtime))
}

pub fn refresh_system_appearance(runtime: &RuntimeContext) {
    let latest = otto_platform::read_system_appearance();
    if latest != runtime.system_appearance.get() {
        runtime.system_appearance.set(latest);
        refresh_theme(runtime);
    }
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
    fn appearance_combines_the_system_snapshot_with_the_user_mode() {
        let runtime = RuntimeContext::new(false);
        runtime.system_appearance.set(otto_core::SystemAppearance {
            dark: true,
            transparency: false,
            contrast: None,
        });
        set_theme_mode(&runtime, ThemeMode::Light);
        let resolved = appearance(&runtime);
        assert_eq!(resolved.theme, otto_core::ResolvedTheme::Light);
        assert_eq!(resolved.variant(), otto_core::MaterialVariant::Opaque);
    }
}
