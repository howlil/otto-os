#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedTheme {
    Light,
    Dark,
}

impl ThemeMode {
    #[must_use]
    pub fn resolve(self, system_dark: bool) -> ResolvedTheme {
        match self {
            Self::Auto => {
                if system_dark {
                    ResolvedTheme::Dark
                } else {
                    ResolvedTheme::Light
                }
            }
            Self::Light => ResolvedTheme::Light,
            Self::Dark => ResolvedTheme::Dark,
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::Auto,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

impl ResolvedTheme {
    #[must_use]
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    #[must_use]
    pub const fn is_light(self) -> bool {
        matches!(self, Self::Light)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_auto_follows_system() {
        assert_eq!(ThemeMode::Auto.resolve(true), ResolvedTheme::Dark);
        assert_eq!(ThemeMode::Auto.resolve(false), ResolvedTheme::Light);
    }

    #[test]
    fn theme_mode_explicit_overrides_system() {
        assert_eq!(ThemeMode::Light.resolve(true), ResolvedTheme::Light);
        assert_eq!(ThemeMode::Light.resolve(false), ResolvedTheme::Light);
        assert_eq!(ThemeMode::Dark.resolve(true), ResolvedTheme::Dark);
        assert_eq!(ThemeMode::Dark.resolve(false), ResolvedTheme::Dark);
    }

    #[test]
    fn theme_mode_cycles_through_variants() {
        assert_eq!(ThemeMode::Auto.next(), ThemeMode::Dark);
        assert_eq!(ThemeMode::Dark.next(), ThemeMode::Light);
        assert_eq!(ThemeMode::Light.next(), ThemeMode::Auto);
    }
}
