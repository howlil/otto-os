use std::cell::Cell;

use otto_core::{ResolvedTheme, ThemeMode};

thread_local! {
    static THEME_MODE: Cell<ThemeMode> = const { Cell::new(ThemeMode::Auto) };
}

pub fn theme_mode() -> ThemeMode {
    THEME_MODE.with(Cell::get)
}

pub fn set_theme_mode(mode: ThemeMode) {
    THEME_MODE.with(|cell| cell.set(mode));
    refresh_theme();
}

pub fn cycle_theme_mode() -> ThemeMode {
    let next = theme_mode().next();
    set_theme_mode(next);
    next
}

pub fn resolved_theme() -> ResolvedTheme {
    let system_dark = otto_platform::is_system_in_dark_mode();
    theme_mode().resolve(system_dark)
}

pub fn refresh_theme() {
    crate::bar::repaint();
    crate::dock::repaint();
    crate::menu::repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_cycles_and_stores() {
        set_theme_mode(ThemeMode::Auto);
        assert_eq!(theme_mode(), ThemeMode::Auto);
        assert_eq!(cycle_theme_mode(), ThemeMode::Dark);
        assert_eq!(theme_mode(), ThemeMode::Dark);
        assert_eq!(cycle_theme_mode(), ThemeMode::Light);
        assert_eq!(theme_mode(), ThemeMode::Light);
        assert_eq!(cycle_theme_mode(), ThemeMode::Auto);
        assert_eq!(theme_mode(), ThemeMode::Auto);
    }
}
