use crate::layout::scale;
use crate::theme::{ResolvedTheme, Rgba, ThemeMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialVariant {
    Glass,
    Opaque,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastColors {
    pub window: Rgba,
    pub window_text: Rgba,
    pub highlight: Rgba,
    pub highlight_text: Rgba,
    pub gray_text: Rgba,
    pub button_face: Rgba,
}

impl ContrastColors {
    pub const BLACK_THEME: Self = Self {
        window: Rgba::new(0.0, 0.0, 0.0, 1.0),
        window_text: Rgba::new(1.0, 1.0, 1.0, 1.0),
        highlight: Rgba::new(0.102, 0.922, 1.0, 1.0),
        highlight_text: Rgba::new(0.0, 0.0, 0.0, 1.0),
        gray_text: Rgba::new(0.247, 0.949, 0.247, 1.0),
        button_face: Rgba::new(0.0, 0.0, 0.0, 1.0),
    };

    pub const WHITE_THEME: Self = Self {
        window: Rgba::new(1.0, 1.0, 1.0, 1.0),
        window_text: Rgba::new(0.0, 0.0, 0.0, 1.0),
        highlight: Rgba::new(0.216, 0.0, 0.431, 1.0),
        highlight_text: Rgba::new(1.0, 1.0, 1.0, 1.0),
        gray_text: Rgba::new(0.376, 0.0, 0.0, 1.0),
        button_face: Rgba::new(1.0, 1.0, 1.0, 1.0),
    };

    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.window.relative_luminance() < self.window_text.relative_luminance()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemAppearance {
    pub dark: bool,
    pub transparency: bool,
    pub contrast: Option<ContrastColors>,
}

impl SystemAppearance {
    pub const DEFAULT: Self = Self {
        dark: true,
        transparency: true,
        contrast: None,
    };

    #[must_use]
    pub fn resolve(self, mode: ThemeMode) -> Appearance {
        Appearance {
            theme: mode.resolve(self.dark),
            transparency: self.transparency,
            contrast: self.contrast,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    pub theme: ResolvedTheme,
    pub transparency: bool,
    pub contrast: Option<ContrastColors>,
}

impl Appearance {
    #[must_use]
    pub const fn glass(theme: ResolvedTheme) -> Self {
        Self {
            theme,
            transparency: true,
            contrast: None,
        }
    }

    #[must_use]
    pub const fn variant(&self) -> MaterialVariant {
        if self.contrast.is_some() {
            MaterialVariant::HighContrast
        } else if self.transparency {
            MaterialVariant::Glass
        } else {
            MaterialVariant::Opaque
        }
    }

    #[must_use]
    pub const fn uses_backdrop(&self) -> bool {
        matches!(self.variant(), MaterialVariant::Glass)
    }

    #[must_use]
    pub fn content_theme(&self) -> ResolvedTheme {
        match self.contrast {
            Some(contrast) if contrast.is_dark() => ResolvedTheme::Dark,
            Some(_) => ResolvedTheme::Light,
            None => self.theme,
        }
    }

    #[must_use]
    pub fn colors(&self) -> SemanticColors {
        SemanticColors::resolve(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticColors {
    pub bar_background: Rgba,
    pub menu_fill: Rgba,
    pub menu_outer_border: Rgba,
    pub menu_inner_border: Rgba,
    pub menu_hover_fill: Rgba,
    pub menu_separator: Rgba,
    pub text_primary: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,
    pub dock_fill: Rgba,
    pub dock_inner_border: Rgba,
    pub dock_outer_border: Rgba,
    pub dock_divider: Rgba,
    pub dock_dot: Rgba,
    pub hover_pill: Rgba,
    pub card_fill: Rgba,
    pub slider_track: Rgba,
    pub slider_fill: Rgba,
    pub toggle_active: Rgba,
    pub toggle_inactive: Rgba,
    pub specular_rim: Rgba,
    pub notch_obsidian: Rgba,
    pub on_accent: Rgba,
}

impl SemanticColors {
    #[must_use]
    pub fn resolve(appearance: &Appearance) -> Self {
        match (
            appearance.contrast,
            appearance.transparency,
            appearance.theme,
        ) {
            (Some(contrast), _, _) => Self::high_contrast(&contrast),
            (None, true, ResolvedTheme::Dark) => Self::glass_dark(),
            (None, true, ResolvedTheme::Light) => Self::glass_light(),
            (None, false, ResolvedTheme::Dark) => Self::opaque_dark(),
            (None, false, ResolvedTheme::Light) => Self::opaque_light(),
        }
    }

    const fn glass_dark() -> Self {
        Self {
            bar_background: Rgba::new(0.0, 0.0, 0.0, 0.28),
            menu_fill: Rgba::new(0.125, 0.125, 0.149, 0.94),
            menu_outer_border: Rgba::new(0.0, 0.0, 0.0, 0.48),
            menu_inner_border: Rgba::new(1.0, 1.0, 1.0, 0.20),
            menu_hover_fill: Rgba::new(0.0, 0.392, 0.882, 0.96),
            menu_separator: Rgba::new(1.0, 1.0, 1.0, 0.14),
            text_primary: Rgba::new(1.0, 1.0, 1.0, 0.95),
            text_muted: Rgba::new(1.0, 1.0, 1.0, 0.52),
            text_disabled: Rgba::new(1.0, 1.0, 1.0, 0.34),
            dock_fill: Rgba::new(0.11, 0.11, 0.14, 0.68),
            dock_inner_border: Rgba::new(1.0, 1.0, 1.0, 0.22),
            dock_outer_border: Rgba::new(0.0, 0.0, 0.0, 0.42),
            dock_divider: Rgba::new(1.0, 1.0, 1.0, 0.22),
            dock_dot: Rgba::new(1.0, 1.0, 1.0, 0.92),
            hover_pill: Rgba::new(1.0, 1.0, 1.0, 0.16),
            card_fill: Rgba::new(1.0, 1.0, 1.0, 0.08),
            slider_track: Rgba::new(1.0, 1.0, 1.0, 0.16),
            slider_fill: Rgba::new(1.0, 1.0, 1.0, 0.92),
            toggle_active: Rgba::new(0.039, 0.518, 1.0, 1.0),
            toggle_inactive: Rgba::new(1.0, 1.0, 1.0, 0.12),
            specular_rim: Rgba::new(1.0, 1.0, 1.0, 0.22),
            notch_obsidian: Rgba::new(0.0, 0.0, 0.0, 1.0),
            on_accent: Rgba::new(1.0, 1.0, 1.0, 1.0),
        }
    }

    const fn glass_light() -> Self {
        Self {
            bar_background: Rgba::new(1.0, 1.0, 1.0, 0.38),
            menu_fill: Rgba::new(0.96, 0.96, 0.98, 0.88),
            menu_outer_border: Rgba::new(0.0, 0.0, 0.0, 0.14),
            menu_inner_border: Rgba::new(1.0, 1.0, 1.0, 0.65),
            menu_hover_fill: Rgba::new(0.0, 0.392, 0.882, 0.96),
            menu_separator: Rgba::new(0.0, 0.0, 0.0, 0.08),
            text_primary: Rgba::new(0.08, 0.08, 0.10, 0.94),
            text_muted: Rgba::new(0.20, 0.20, 0.24, 0.58),
            text_disabled: Rgba::new(0.0, 0.0, 0.0, 0.26),
            dock_fill: Rgba::new(0.98, 0.98, 1.0, 0.72),
            dock_inner_border: Rgba::new(1.0, 1.0, 1.0, 0.70),
            dock_outer_border: Rgba::new(0.0, 0.0, 0.0, 0.16),
            dock_divider: Rgba::new(0.0, 0.0, 0.0, 0.14),
            dock_dot: Rgba::new(0.10, 0.10, 0.12, 0.90),
            hover_pill: Rgba::new(0.0, 0.0, 0.0, 0.08),
            card_fill: Rgba::new(0.0, 0.0, 0.0, 0.05),
            slider_track: Rgba::new(0.0, 0.0, 0.0, 0.10),
            slider_fill: Rgba::new(0.039, 0.518, 1.0, 0.90),
            toggle_active: Rgba::new(0.039, 0.518, 1.0, 1.0),
            toggle_inactive: Rgba::new(0.0, 0.0, 0.0, 0.08),
            specular_rim: Rgba::new(1.0, 1.0, 1.0, 0.60),
            notch_obsidian: Rgba::new(0.0, 0.0, 0.0, 1.0),
            on_accent: Rgba::new(1.0, 1.0, 1.0, 1.0),
        }
    }

    const fn opaque_dark() -> Self {
        let mut colors = Self::glass_dark();
        colors.bar_background = Rgba::new(0.11, 0.11, 0.13, 1.0);
        colors.menu_fill = Rgba::new(0.14, 0.14, 0.17, 1.0);
        colors.dock_fill = Rgba::new(0.13, 0.13, 0.16, 1.0);
        colors
    }

    const fn opaque_light() -> Self {
        let mut colors = Self::glass_light();
        colors.bar_background = Rgba::new(0.94, 0.94, 0.96, 1.0);
        colors.menu_fill = Rgba::new(0.97, 0.97, 0.99, 1.0);
        colors.dock_fill = Rgba::new(0.96, 0.96, 0.98, 1.0);
        colors
    }

    const fn high_contrast(system: &ContrastColors) -> Self {
        Self {
            bar_background: system.window,
            menu_fill: system.window,
            menu_outer_border: system.window_text,
            menu_inner_border: system.window_text,
            menu_hover_fill: system.highlight,
            menu_separator: system.window_text,
            text_primary: system.window_text,
            text_muted: system.window_text,
            text_disabled: system.gray_text,
            dock_fill: system.window,
            dock_inner_border: system.window_text,
            dock_outer_border: system.window_text,
            dock_divider: system.window_text,
            dock_dot: system.window_text,
            hover_pill: system.highlight,
            card_fill: system.button_face,
            slider_track: system.gray_text,
            slider_fill: system.highlight,
            toggle_active: system.highlight,
            toggle_inactive: system.gray_text,
            specular_rim: system.window_text,
            notch_obsidian: system.window,
            on_accent: system.highlight_text,
        }
    }

    #[must_use]
    pub const fn state_fill(&self, state: InteractionState) -> Option<Rgba> {
        match state {
            InteractionState::Normal | InteractionState::Disabled => None,
            InteractionState::Hover | InteractionState::Focused => Some(self.hover_pill),
            InteractionState::Pressed => Some(Rgba::new(
                self.hover_pill.r,
                self.hover_pill.g,
                self.hover_pill.b,
                pressed_alpha(self.hover_pill.a),
            )),
            InteractionState::Selected => Some(self.menu_hover_fill),
        }
    }

    #[must_use]
    pub const fn state_content(&self, state: InteractionState) -> Rgba {
        match state {
            InteractionState::Disabled => self.text_disabled,
            InteractionState::Selected => self.on_accent,
            InteractionState::Normal
            | InteractionState::Hover
            | InteractionState::Pressed
            | InteractionState::Focused => self.text_primary,
        }
    }

    #[must_use]
    pub const fn focus_ring(&self) -> Rgba {
        self.text_primary
    }
}

const fn pressed_alpha(alpha: f32) -> f32 {
    let raised = alpha * 1.6;
    if raised > 1.0 { 1.0 } else { raised }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionState {
    Normal,
    Hover,
    Pressed,
    Focused,
    Selected,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeTokens {
    pub slot_radius: i32,
    pub slot_inset_y: i32,
    pub focus_ring_radius: i32,
    pub focus_ring_inset_y: i32,
    pub focus_ring_width: i32,
}

impl ShapeTokens {
    pub const GOLDEN_GATE: Self = Self {
        slot_radius: 6,
        slot_inset_y: 4,
        focus_ring_radius: 7,
        focus_ring_inset_y: 3,
        focus_ring_width: 2,
    };

    #[must_use]
    pub fn scaled(self, dpi: u32) -> Self {
        Self {
            slot_radius: scale(self.slot_radius, dpi),
            slot_inset_y: scale(self.slot_inset_y, dpi),
            focus_ring_radius: scale(self.focus_ring_radius, dpi),
            focus_ring_inset_y: scale(self.focus_ring_inset_y, dpi),
            focus_ring_width: scale(self.focus_ring_width, dpi).max(self.focus_ring_width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Appearance, ContrastColors, InteractionState, MaterialVariant, SemanticColors, ShapeTokens,
        SystemAppearance,
    };
    use crate::theme::{ResolvedTheme, Rgba, ThemeMode};

    #[test]
    fn system_appearance_resolves_against_the_user_theme_mode() {
        let system = SystemAppearance {
            dark: false,
            transparency: false,
            contrast: None,
        };
        let auto = system.resolve(ThemeMode::Auto);
        assert_eq!(auto.theme, ResolvedTheme::Light);
        assert_eq!(auto.variant(), MaterialVariant::Opaque);
        assert_eq!(system.resolve(ThemeMode::Dark).theme, ResolvedTheme::Dark);
        let contrast = SystemAppearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..system
        };
        assert_eq!(
            contrast.resolve(ThemeMode::Light).variant(),
            MaterialVariant::HighContrast
        );
    }

    const DARK_ACRYLIC: Rgba = Rgba::new(0.13, 0.13, 0.14, 1.0);
    const LIGHT_ACRYLIC: Rgba = Rgba::new(0.93, 0.93, 0.94, 1.0);

    fn appearance(theme: ResolvedTheme, transparency: bool) -> Appearance {
        Appearance {
            theme,
            transparency,
            contrast: None,
        }
    }

    fn surface(fill: Rgba, backdrop: Rgba) -> Rgba {
        if fill.a < 0.999 {
            fill.blend_over(backdrop)
        } else {
            fill
        }
    }

    fn all_appearances() -> Vec<(Appearance, Rgba)> {
        vec![
            (appearance(ResolvedTheme::Dark, true), DARK_ACRYLIC),
            (appearance(ResolvedTheme::Light, true), LIGHT_ACRYLIC),
            (
                appearance(ResolvedTheme::Dark, false),
                Rgba::new(1.0, 1.0, 1.0, 1.0),
            ),
            (
                appearance(ResolvedTheme::Light, false),
                Rgba::new(0.0, 0.0, 0.0, 1.0),
            ),
            (
                Appearance {
                    contrast: Some(ContrastColors::BLACK_THEME),
                    ..appearance(ResolvedTheme::Light, true)
                },
                Rgba::new(1.0, 1.0, 1.0, 1.0),
            ),
            (
                Appearance {
                    contrast: Some(ContrastColors::WHITE_THEME),
                    ..appearance(ResolvedTheme::Dark, true)
                },
                Rgba::new(0.0, 0.0, 0.0, 1.0),
            ),
        ]
    }

    #[test]
    fn variant_follows_contrast_then_transparency() {
        assert_eq!(
            appearance(ResolvedTheme::Dark, true).variant(),
            MaterialVariant::Glass
        );
        assert_eq!(
            appearance(ResolvedTheme::Dark, false).variant(),
            MaterialVariant::Opaque
        );
        let contrast = Appearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..appearance(ResolvedTheme::Light, true)
        };
        assert_eq!(contrast.variant(), MaterialVariant::HighContrast);
        assert!(!contrast.uses_backdrop());
        assert!(appearance(ResolvedTheme::Light, true).uses_backdrop());
        assert!(!appearance(ResolvedTheme::Light, false).uses_backdrop());
    }

    #[test]
    fn high_contrast_content_theme_follows_the_system_scheme() {
        let black = Appearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..appearance(ResolvedTheme::Light, true)
        };
        let white = Appearance {
            contrast: Some(ContrastColors::WHITE_THEME),
            ..appearance(ResolvedTheme::Dark, true)
        };
        assert_eq!(black.content_theme(), ResolvedTheme::Dark);
        assert_eq!(white.content_theme(), ResolvedTheme::Light);
        assert_eq!(
            appearance(ResolvedTheme::Light, true).content_theme(),
            ResolvedTheme::Light
        );
    }

    #[test]
    fn opaque_and_high_contrast_surfaces_never_let_the_wallpaper_through() {
        for transparency_off in [
            appearance(ResolvedTheme::Dark, false),
            appearance(ResolvedTheme::Light, false),
        ] {
            let colors = transparency_off.colors();
            assert!(colors.bar_background.a >= 0.999);
            assert!(colors.menu_fill.a >= 0.999);
            assert!(colors.dock_fill.a >= 0.999);
        }
        let colors = SemanticColors::resolve(&Appearance {
            contrast: Some(ContrastColors::WHITE_THEME),
            ..appearance(ResolvedTheme::Dark, true)
        });
        assert_eq!(colors.bar_background, ContrastColors::WHITE_THEME.window);
        assert_eq!(colors.text_primary, ContrastColors::WHITE_THEME.window_text);
        assert_eq!(colors.on_accent, ContrastColors::WHITE_THEME.highlight_text);
    }

    #[test]
    fn primary_text_meets_wcag_aa_on_every_surface() {
        for (appearance, backdrop) in all_appearances() {
            let colors = appearance.colors();
            for (name, fill) in [
                ("bar", colors.bar_background),
                ("menu", colors.menu_fill),
                ("dock", colors.dock_fill),
            ] {
                let background = surface(fill, backdrop);
                let ratio = colors.text_primary.contrast_ratio(background);
                assert!(
                    ratio >= 4.5,
                    "{:?} {:?} {name}: primary text contrast {ratio:.2}",
                    appearance.variant(),
                    appearance.theme
                );
            }
        }
    }

    #[test]
    fn secondary_text_meets_wcag_aa_large_on_the_bar_and_menu() {
        for (appearance, backdrop) in all_appearances() {
            let colors = appearance.colors();
            for fill in [colors.bar_background, colors.menu_fill] {
                let ratio = colors.text_muted.contrast_ratio(surface(fill, backdrop));
                assert!(
                    ratio >= 3.0,
                    "{:?} {:?}: muted {ratio:.2}",
                    appearance.variant(),
                    appearance.theme
                );
            }
        }
    }

    #[test]
    fn selected_rows_use_readable_text_on_the_accent() {
        for (appearance, backdrop) in all_appearances() {
            let colors = appearance.colors();
            let selected = surface(colors.menu_hover_fill, surface(colors.menu_fill, backdrop));
            let content = colors.state_content(InteractionState::Selected);
            let ratio = content.contrast_ratio(selected);
            assert!(
                ratio >= 4.5,
                "{:?} {:?}: selected {ratio:.2}",
                appearance.variant(),
                appearance.theme
            );
        }
    }

    #[test]
    fn interaction_states_resolve_to_distinct_layers() {
        let colors = appearance(ResolvedTheme::Dark, true).colors();
        assert_eq!(colors.state_fill(InteractionState::Normal), None);
        assert_eq!(colors.state_fill(InteractionState::Disabled), None);
        let hover = colors.state_fill(InteractionState::Hover).unwrap();
        let pressed = colors.state_fill(InteractionState::Pressed).unwrap();
        assert!(pressed.a > hover.a);
        assert_eq!(
            colors.state_fill(InteractionState::Selected),
            Some(colors.menu_hover_fill)
        );
        assert_eq!(
            colors.state_content(InteractionState::Disabled),
            colors.text_disabled
        );
        assert_eq!(colors.focus_ring(), colors.text_primary);
    }

    #[test]
    fn shape_tokens_scale_with_dpi_and_keep_a_visible_focus_ring() {
        let base = ShapeTokens::GOLDEN_GATE;
        assert_eq!(base.scaled(96), base);
        let doubled = base.scaled(192);
        assert_eq!(doubled.slot_radius, base.slot_radius * 2);
        assert_eq!(doubled.focus_ring_width, base.focus_ring_width * 2);
        assert!(base.scaled(72).focus_ring_width >= base.focus_ring_width);
    }
}
