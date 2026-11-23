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
    pub fn to_rgb_u32(self) -> u32 {
        let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (ch(self.r) << 16) | (ch(self.g) << 8) | ch(self.b)
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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedTheme {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AppearanceStyle {
    #[default]
    LiquidGlass,
    Frosted,
    Tinted,
    Solid,
}

impl AppearanceStyle {
    pub const ALL: [Self; 4] = [Self::LiquidGlass, Self::Frosted, Self::Tinted, Self::Solid];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LiquidGlass => "Liquid Glass",
            Self::Frosted => "Frosted",
            Self::Tinted => "Tinted",
            Self::Solid => "Solid",
        }
    }

    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::LiquidGlass => "liquid_glass",
            Self::Frosted => "frosted",
            Self::Tinted => "tinted",
            Self::Solid => "solid",
        }
    }

    #[must_use]
    pub fn parse_config(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "liquid_glass" | "clear" | "glass" => Some(Self::LiquidGlass),
            "frosted" => Some(Self::Frosted),
            "tinted" => Some(Self::Tinted),
            "solid" | "opaque" => Some(Self::Solid),
            _ => None,
        }
    }

    #[must_use]
    pub const fn uses_glass(self) -> bool {
        !matches!(self, Self::Solid)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AccentColor {
    System,
    #[default]
    Blue,
    Purple,
    Rose,
    Amber,
    Emerald,
    Graphite,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedAccent {
    pub fill: Rgba,
    pub on_accent: Rgba,
    pub tint: [f32; 3],
}

const WHITE_INK: Rgba = Rgba::new(1.0, 1.0, 1.0, 1.0);
const DARK_INK: Rgba = Rgba::new(0.05, 0.05, 0.07, 1.0);
const MIN_ACCENT_CONTRAST: f32 = 4.6;

impl AccentColor {
    pub const ALL: [Self; 7] = [
        Self::System,
        Self::Blue,
        Self::Purple,
        Self::Rose,
        Self::Amber,
        Self::Emerald,
        Self::Graphite,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Rose => "Rose",
            Self::Amber => "Amber",
            Self::Emerald => "Emerald",
            Self::Graphite => "Graphite",
        }
    }

    #[must_use]
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Rose => "rose",
            Self::Amber => "amber",
            Self::Emerald => "emerald",
            Self::Graphite => "graphite",
        }
    }

    #[must_use]
    pub fn parse_config(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "system" => Some(Self::System),
            "blue" => Some(Self::Blue),
            "purple" => Some(Self::Purple),
            "rose" | "pink" => Some(Self::Rose),
            "amber" | "orange" => Some(Self::Amber),
            "emerald" | "green" => Some(Self::Emerald),
            "graphite" | "gray" | "grey" => Some(Self::Graphite),
            _ => None,
        }
    }

    #[must_use]
    pub const fn swatch(self, system_accent: Option<Rgba>) -> Rgba {
        match self {
            Self::System => match system_accent {
                Some(color) => Rgba::new(color.r, color.g, color.b, 1.0),
                None => Rgba::new(0.039, 0.518, 1.0, 1.0),
            },
            Self::Blue => Rgba::new(0.039, 0.518, 1.0, 1.0),
            Self::Purple => Rgba::new(0.686, 0.322, 0.871, 1.0),
            Self::Rose => Rgba::new(1.0, 0.216, 0.373, 1.0),
            Self::Amber => Rgba::new(1.0, 0.624, 0.039, 1.0),
            Self::Emerald => Rgba::new(0.188, 0.820, 0.345, 1.0),
            Self::Graphite => Rgba::new(0.557, 0.557, 0.576, 1.0),
        }
    }

    #[must_use]
    pub fn resolve(self, theme: ResolvedTheme, system_accent: Option<Rgba>) -> ResolvedAccent {
        let dark = theme.is_dark();
        let (raw_fill, tint) = match self {
            Self::System => {
                let raw = system_accent.unwrap_or_else(|| Rgba::new(0.0, 0.412, 0.922, 1.0));
                return calibrate_accent(raw);
            }
            Self::Blue => (
                if dark {
                    Rgba::new(0.0, 0.412, 0.922, 1.0)
                } else {
                    Rgba::new(0.0, 0.392, 0.882, 1.0)
                },
                [0.14, 0.46, 0.96],
            ),
            Self::Purple => (
                if dark {
                    Rgba::new(0.537, 0.267, 0.941, 1.0)
                } else {
                    Rgba::new(0.486, 0.204, 0.859, 1.0)
                },
                [0.58, 0.30, 0.92],
            ),
            Self::Rose => (
                if dark {
                    Rgba::new(0.851, 0.157, 0.388, 1.0)
                } else {
                    Rgba::new(0.800, 0.125, 0.345, 1.0)
                },
                [0.92, 0.24, 0.45],
            ),
            Self::Amber => (
                if dark {
                    Rgba::new(0.745, 0.353, 0.0, 1.0)
                } else {
                    Rgba::new(0.690, 0.314, 0.0, 1.0)
                },
                [0.94, 0.56, 0.14],
            ),
            Self::Emerald => (
                if dark {
                    Rgba::new(0.055, 0.525, 0.314, 1.0)
                } else {
                    Rgba::new(0.043, 0.475, 0.275, 1.0)
                },
                [0.18, 0.76, 0.42],
            ),
            Self::Graphite => (
                if dark {
                    Rgba::new(0.361, 0.380, 0.431, 1.0)
                } else {
                    Rgba::new(0.310, 0.329, 0.380, 1.0)
                },
                [0.48, 0.50, 0.56],
            ),
        };
        let mut calibrated = calibrate_accent(raw_fill);
        calibrated.tint = tint;
        calibrated
    }
}

fn calibrate_accent(raw: Rgba) -> ResolvedAccent {
    let base = Rgba::new(
        raw.r.clamp(0.0, 1.0),
        raw.g.clamp(0.0, 1.0),
        raw.b.clamp(0.0, 1.0),
        1.0,
    );
    let tint = [base.r, base.g, base.b];
    let white_ratio = WHITE_INK.contrast_ratio(base);
    if white_ratio >= MIN_ACCENT_CONTRAST {
        return ResolvedAccent {
            fill: base,
            on_accent: WHITE_INK,
            tint,
        };
    }
    let dark_ratio = DARK_INK.contrast_ratio(base);
    if dark_ratio >= MIN_ACCENT_CONTRAST {
        return ResolvedAccent {
            fill: base,
            on_accent: DARK_INK,
            tint,
        };
    }
    let mut fill = base;
    for _ in 0..16 {
        fill = Rgba::new(fill.r * 0.9, fill.g * 0.9, fill.b * 0.9, 1.0);
        if WHITE_INK.contrast_ratio(fill) >= MIN_ACCENT_CONTRAST {
            break;
        }
    }
    ResolvedAccent {
        fill,
        on_accent: WHITE_INK,
        tint,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AppearancePreferences {
    pub style: AppearanceStyle,
    pub theme: ThemeMode,
    pub accent: AccentColor,
}

impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            style: AppearanceStyle::LiquidGlass,
            theme: ThemeMode::Dark,
            accent: AccentColor::Blue,
        }
    }
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

    #[test]
    fn appearance_style_and_accent_round_trip_config_values() {
        for style in AppearanceStyle::ALL {
            assert_eq!(
                AppearanceStyle::parse_config(style.config_value()),
                Some(style)
            );
        }
        assert_eq!(AppearanceStyle::parse_config("unknown"), None);
        assert!(AppearanceStyle::LiquidGlass.uses_glass());
        assert!(AppearanceStyle::Frosted.uses_glass());
        assert!(AppearanceStyle::Tinted.uses_glass());
        assert!(!AppearanceStyle::Solid.uses_glass());

        for accent in AccentColor::ALL {
            assert_eq!(
                AccentColor::parse_config(accent.config_value()),
                Some(accent)
            );
        }
        assert_eq!(AccentColor::parse_config("unknown"), None);
        assert_eq!(
            AppearancePreferences::default(),
            AppearancePreferences {
                style: AppearanceStyle::LiquidGlass,
                theme: ThemeMode::Dark,
                accent: AccentColor::Blue,
            }
        );
    }

    #[test]
    fn every_accent_including_extreme_system_colors_meets_wcag_aa_on_accent() {
        let system_samples = [
            None,
            Some(Rgba::new(1.0, 1.0, 0.0, 1.0)),
            Some(Rgba::new(0.0, 0.95, 0.95, 1.0)),
            Some(Rgba::new(0.45, 0.48, 0.50, 1.0)),
            Some(Rgba::new(0.05, 0.08, 0.12, 1.0)),
        ];
        for theme in [ResolvedTheme::Dark, ResolvedTheme::Light] {
            for accent in AccentColor::ALL {
                for sys in system_samples {
                    let resolved = accent.resolve(theme, sys);
                    let ratio = resolved.on_accent.contrast_ratio(resolved.fill);
                    assert!(
                        ratio >= 4.5,
                        "{theme:?} {accent:?} {sys:?}: on_accent contrast {ratio:.2}"
                    );
                }
            }
        }
    }
}
