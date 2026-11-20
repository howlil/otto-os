#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub fn blend_over(self, background: Self) -> Self {
        let alpha = self.a.clamp(0.0, 1.0);
        Self {
            r: self.r * alpha + background.r * (1.0 - alpha),
            g: self.g * alpha + background.g * (1.0 - alpha),
            b: self.b * alpha + background.b * (1.0 - alpha),
            a: 1.0,
        }
    }

    #[must_use]
    pub fn relative_luminance(self) -> f32 {
        let lin = |c: f32| -> f32 {
            let clamped = c.clamp(0.0, 1.0);
            if clamped <= 0.04045 {
                clamped / 12.92
            } else {
                ((clamped + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    #[must_use]
    pub fn contrast_ratio(self, background: Self) -> f32 {
        let effective_fg = if self.a < 0.999 {
            self.blend_over(background)
        } else {
            self
        };
        let l1 = effective_fg.relative_luminance();
        let l2 = background.relative_luminance();
        let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[must_use]
    pub fn is_wcag_aa_normal(self, background: Self) -> bool {
        self.contrast_ratio(background) >= 4.5
    }

    #[must_use]
    pub fn is_wcag_aa_large(self, background: Self) -> bool {
        self.contrast_ratio(background) >= 3.0
    }
}

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

    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub fn parse_config(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
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

    #[test]
    fn theme_mode_round_trips_config_values() {
        for mode in [ThemeMode::Auto, ThemeMode::Light, ThemeMode::Dark] {
            assert_eq!(ThemeMode::parse_config(mode.config_value()), Some(mode));
        }
        assert_eq!(ThemeMode::parse_config(" DARK "), Some(ThemeMode::Dark));
        assert_eq!(ThemeMode::parse_config("unknown"), None);
    }

    #[test]
    fn rgba_contrast_ratio_black_and_white() {
        let white = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let black = Rgba::new(0.0, 0.0, 0.0, 1.0);
        let ratio = white.contrast_ratio(black);
        assert!((ratio - 21.0).abs() < 0.1);
        assert!(white.is_wcag_aa_normal(black));
        assert!(white.is_wcag_aa_large(black));
    }

    #[test]
    fn rgba_contrast_ratio_dark_mode_palette() {
        let text_primary = Rgba::new(1.0, 1.0, 1.0, 0.95);
        let menu_bg = Rgba::new(0.125, 0.125, 0.149, 1.0);
        assert!(text_primary.is_wcag_aa_normal(menu_bg));
    }

    #[test]
    fn rgba_contrast_ratio_light_mode_palette() {
        let text_primary = Rgba::new(0.08, 0.08, 0.10, 0.94);
        let menu_bg = Rgba::new(0.96, 0.96, 0.98, 1.0);
        assert!(text_primary.is_wcag_aa_normal(menu_bg));
    }
}
