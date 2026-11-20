use std::cell::RefCell;
use std::collections::HashMap;

use otto_core::{Appearance, MaterialVariant, ResolvedTheme, Rgba};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{ID2D1DCRenderTarget, ID2D1SolidColorBrush};
use windows::core::Result;

const BRUSH_CACHE_LIMIT: usize = 256;

const fn d2d(color: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LiquidGlassPalette {
    pub bar_background: D2D1_COLOR_F,
    pub menu_fill: D2D1_COLOR_F,
    pub menu_outer_border: D2D1_COLOR_F,
    pub menu_inner_border: D2D1_COLOR_F,
    pub menu_hover_fill: D2D1_COLOR_F,
    pub menu_separator: D2D1_COLOR_F,
    pub text_primary: D2D1_COLOR_F,
    pub text_muted: D2D1_COLOR_F,
    pub text_disabled: D2D1_COLOR_F,
    pub dock_fill: D2D1_COLOR_F,
    pub dock_inner_border: D2D1_COLOR_F,
    pub dock_outer_border: D2D1_COLOR_F,
    pub dock_divider: D2D1_COLOR_F,
    pub dock_dot: D2D1_COLOR_F,
    pub hover_pill: D2D1_COLOR_F,
    pub card_fill: D2D1_COLOR_F,
    pub slider_track: D2D1_COLOR_F,
    pub slider_fill: D2D1_COLOR_F,
    pub toggle_active: D2D1_COLOR_F,
    pub toggle_inactive: D2D1_COLOR_F,
    pub specular_rim: D2D1_COLOR_F,
    pub notch_obsidian: D2D1_COLOR_F,
    pub on_accent: D2D1_COLOR_F,
    pub is_dark: bool,
    pub is_high_contrast: bool,
    pub is_translucent: bool,
}

impl LiquidGlassPalette {
    #[must_use]
    pub fn for_appearance(appearance: &Appearance) -> Self {
        let colors = appearance.colors();
        let variant = appearance.variant();
        Self {
            bar_background: d2d(colors.bar_background),
            menu_fill: d2d(colors.menu_fill),
            menu_outer_border: d2d(colors.menu_outer_border),
            menu_inner_border: d2d(colors.menu_inner_border),
            menu_hover_fill: d2d(colors.menu_hover_fill),
            menu_separator: d2d(colors.menu_separator),
            text_primary: d2d(colors.text_primary),
            text_muted: d2d(colors.text_muted),
            text_disabled: d2d(colors.text_disabled),
            dock_fill: d2d(colors.dock_fill),
            dock_inner_border: d2d(colors.dock_inner_border),
            dock_outer_border: d2d(colors.dock_outer_border),
            dock_divider: d2d(colors.dock_divider),
            dock_dot: d2d(colors.dock_dot),
            hover_pill: d2d(colors.hover_pill),
            card_fill: d2d(colors.card_fill),
            slider_track: d2d(colors.slider_track),
            slider_fill: d2d(colors.slider_fill),
            toggle_active: d2d(colors.toggle_active),
            toggle_inactive: d2d(colors.toggle_inactive),
            specular_rim: d2d(colors.specular_rim),
            notch_obsidian: d2d(colors.notch_obsidian),
            on_accent: d2d(colors.on_accent),
            is_dark: appearance.theme == ResolvedTheme::Dark,
            is_high_contrast: variant == MaterialVariant::HighContrast,
            is_translucent: variant == MaterialVariant::Glass,
        }
    }

    #[must_use]
    pub fn for_control_center(appearance: &Appearance) -> Self {
        let mut palette = Self::for_appearance(appearance);
        if palette.is_translucent && !palette.is_high_contrast {
            if palette.is_dark {
                palette.card_fill = D2D1_COLOR_F {
                    r: 0.07,
                    g: 0.08,
                    b: 0.11,
                    a: 0.34,
                };
                palette.text_muted.a = 0.84;
            } else {
                palette.card_fill = palette.menu_fill;
                palette.text_muted.a = 0.72;
            }
        }
        palette
    }

    pub(crate) const fn control_center_card_sheen_alpha(&self) -> f32 {
        if self.is_translucent && !self.is_high_contrast {
            if self.is_dark { 0.03 } else { 0.24 }
        } else {
            0.0
        }
    }
}

fn brush_key(color: D2D1_COLOR_F) -> u32 {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    (channel(color.r) << 24) | (channel(color.g) << 16) | (channel(color.b) << 8) | channel(color.a)
}

pub struct BrushPalette {
    pub solid: ID2D1SolidColorBrush,
    pub stroke: ID2D1SolidColorBrush,
    pub text_primary: ID2D1SolidColorBrush,
    pub text_muted: ID2D1SolidColorBrush,
    pub text_disabled: ID2D1SolidColorBrush,
    pub hover: ID2D1SolidColorBrush,
    pub accent: ID2D1SolidColorBrush,
    pub black: ID2D1SolidColorBrush,
    pub white: ID2D1SolidColorBrush,
    pub notch_obsidian: ID2D1SolidColorBrush,
    pub specular_rim: ID2D1SolidColorBrush,
    pub separator: ID2D1SolidColorBrush,
    pub card_fill: ID2D1SolidColorBrush,
    pub on_accent: ID2D1SolidColorBrush,
    target: ID2D1DCRenderTarget,
    cache: RefCell<HashMap<u32, ID2D1SolidColorBrush>>,
}

impl BrushPalette {
    #[allow(clippy::too_many_lines)]
    pub fn new(target: &ID2D1DCRenderTarget) -> Result<Self> {
        unsafe {
            Ok(Self {
                solid: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    None,
                )?,
                stroke: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.20,
                    },
                    None,
                )?,
                text_primary: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.95,
                    },
                    None,
                )?,
                text_muted: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.52,
                    },
                    None,
                )?,
                text_disabled: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.34,
                    },
                    None,
                )?,
                hover: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.12,
                    },
                    None,
                )?,
                accent: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.039,
                        g: 0.518,
                        b: 1.0,
                        a: 1.0,
                    },
                    None,
                )?,
                black: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    None,
                )?,
                white: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                    None,
                )?,
                notch_obsidian: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    None,
                )?,
                specular_rim: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.22,
                    },
                    None,
                )?,
                separator: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.14,
                    },
                    None,
                )?,
                card_fill: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.08,
                    },
                    None,
                )?,
                on_accent: target.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    },
                    None,
                )?,
                target: target.clone(),
                cache: RefCell::new(HashMap::new()),
            })
        }
    }

    pub fn update_theme(&self, palette: &LiquidGlassPalette) {
        unsafe {
            self.text_primary.SetColor(&palette.text_primary);
            self.text_muted.SetColor(&palette.text_muted);
            self.text_disabled.SetColor(&palette.text_disabled);
            self.hover.SetColor(&palette.hover_pill);
            self.accent.SetColor(&palette.slider_fill);
            self.stroke.SetColor(&palette.menu_inner_border);
            self.notch_obsidian.SetColor(&palette.notch_obsidian);
            self.specular_rim.SetColor(&palette.specular_rim);
            self.separator.SetColor(&palette.menu_separator);
            self.card_fill.SetColor(&palette.card_fill);
            self.on_accent.SetColor(&palette.on_accent);
        }
    }

    #[must_use]
    pub fn color_brush(&self, color: D2D1_COLOR_F) -> ID2D1SolidColorBrush {
        let key = brush_key(color);
        if let Some(brush) = self.cache.borrow().get(&key) {
            return brush.clone();
        }
        let Ok(brush) = (unsafe { self.target.CreateSolidColorBrush(&color, None) }) else {
            unsafe {
                self.solid.SetColor(&color);
            }
            return self.solid.clone();
        };
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= BRUSH_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, brush.clone());
        brush
    }

    #[must_use]
    pub fn alpha_brush(&self, base: D2D1_COLOR_F, alpha: f32) -> ID2D1SolidColorBrush {
        self.color_brush(D2D1_COLOR_F {
            r: base.r,
            g: base.g,
            b: base.b,
            a: (base.a * alpha).clamp(0.0, 1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{LiquidGlassPalette, brush_key};
    use otto_core::{Appearance, ContrastColors, GlassParams, ResolvedTheme, Rgba};
    use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

    const fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
        D2D1_COLOR_F { r, g, b, a }
    }

    fn rgba(color: D2D1_COLOR_F) -> Rgba {
        Rgba::new(color.r, color.g, color.b, color.a)
    }

    #[test]
    fn distinct_colors_never_share_a_cache_key() {
        let dot = color(1.0, 1.0, 1.0, 0.92);
        let hidden_dot = color(1.0, 1.0, 1.0, 0.92 * 0.45);
        assert_ne!(brush_key(dot), brush_key(hidden_dot));
        assert_eq!(brush_key(dot), brush_key(color(1.0, 1.0, 1.0, 0.9201)));
        assert_eq!(
            brush_key(color(2.0, -1.0, 0.0, 1.0)),
            brush_key(color(1.0, 0.0, 0.0, 1.0))
        );
    }

    #[test]
    fn palette_follows_the_material_variant() {
        let glass = LiquidGlassPalette::for_appearance(&Appearance::glass(ResolvedTheme::Dark));
        assert!(glass.is_translucent);
        assert!(!glass.is_high_contrast);
        let opaque = LiquidGlassPalette::for_appearance(&Appearance {
            transparency: false,
            ..Appearance::glass(ResolvedTheme::Light)
        });
        assert!(!opaque.is_translucent);
        assert!((opaque.bar_background.a - 1.0).abs() < f32::EPSILON);
        let contrast = LiquidGlassPalette::for_appearance(&Appearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..Appearance::glass(ResolvedTheme::Light)
        });
        assert!(contrast.is_high_contrast);
        assert!((contrast.text_primary.r - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn control_center_text_meets_wcag_aa_on_glass_cards_across_adaptive_midtones() {
        for theme in [ResolvedTheme::Dark, ResolvedTheme::Light] {
            let appearance = Appearance::glass(theme);
            let palette = LiquidGlassPalette::for_control_center(&appearance);
            let glass = GlassParams::for_theme(theme);
            let levels: &[f32] = match theme {
                ResolvedTheme::Dark => &[0.0, 0.25, 0.44, 0.5, 0.56],
                ResolvedTheme::Light => &[0.44, 0.5, 0.56, 0.75, 1.0],
            };

            for &level in levels {
                let graded = glass.grade([level; 3], 1.0);
                let backdrop = Rgba::new(graded[0], graded[1], graded[2], 1.0);
                let shell = Rgba::new(1.0, 1.0, 1.0, glass.sheen_alpha).blend_over(backdrop);
                let card = rgba(palette.card_fill).blend_over(shell);

                for sheen in [0.0, palette.control_center_card_sheen_alpha()] {
                    let background = Rgba::new(1.0, 1.0, 1.0, sheen).blend_over(card);
                    for (name, text) in [
                        ("primary", rgba(palette.text_primary)),
                        ("muted", rgba(palette.text_muted)),
                    ] {
                        let ratio = text.contrast_ratio(background);
                        assert!(
                            ratio >= 4.5,
                            "{theme:?} backdrop {level} sheen {sheen} {name}: {ratio:.2}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn control_center_palette_preserves_opaque_and_high_contrast_fallbacks() {
        let appearances = [
            Appearance {
                transparency: false,
                ..Appearance::glass(ResolvedTheme::Dark)
            },
            Appearance {
                contrast: Some(ContrastColors::BLACK_THEME),
                ..Appearance::glass(ResolvedTheme::Light)
            },
        ];

        for appearance in appearances {
            let base = LiquidGlassPalette::for_appearance(&appearance);
            let control_center = LiquidGlassPalette::for_control_center(&appearance);
            assert!((control_center.card_fill.r - base.card_fill.r).abs() < f32::EPSILON);
            assert!((control_center.card_fill.g - base.card_fill.g).abs() < f32::EPSILON);
            assert!((control_center.card_fill.b - base.card_fill.b).abs() < f32::EPSILON);
            assert!((control_center.card_fill.a - base.card_fill.a).abs() < f32::EPSILON);
            assert!((control_center.text_muted.a - base.text_muted.a).abs() < f32::EPSILON);
            assert!(control_center.control_center_card_sheen_alpha().abs() < f32::EPSILON);
        }
    }
}
