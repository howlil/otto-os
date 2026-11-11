use crate::host::RuntimeContext;
use otto_core::{ResolvedTheme, ThemeMode};

pub fn theme_mode(runtime: &RuntimeContext) -> ThemeMode {
    runtime.theme_mode.get()
}

pub fn set_theme_mode(runtime: &RuntimeContext, mode: ThemeMode) {
    runtime.theme_mode.set(mode);
    refresh_theme(runtime);
}

pub fn cycle_theme_mode(runtime: &RuntimeContext) -> ThemeMode {
    let next = theme_mode(runtime).next();
    set_theme_mode(runtime, next);
    next
}

pub fn resolved_theme(runtime: &RuntimeContext) -> ResolvedTheme {
    let system_dark = otto_platform::is_system_in_dark_mode();
    theme_mode(runtime).resolve(system_dark)
}

pub fn refresh_theme(runtime: &RuntimeContext) {
    crate::bar::repaint(runtime);
    crate::dock::repaint(runtime);
    crate::menu::repaint(runtime);
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
}
