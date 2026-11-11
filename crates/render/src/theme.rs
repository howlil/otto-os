use otto_core::ResolvedTheme;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{ID2D1DCRenderTarget, ID2D1SolidColorBrush};
use windows::core::Result;

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
}

impl LiquidGlassPalette {
    #[must_use]
    pub const fn for_theme(theme: ResolvedTheme) -> Self {
        match theme {
            ResolvedTheme::Dark => Self::dark(),
            ResolvedTheme::Light => Self::light(),
        }
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn dark() -> Self {
        Self {
            bar_background: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.28,
            },
            menu_fill: D2D1_COLOR_F {
                r: 0.125,
                g: 0.125,
                b: 0.149,
                a: 0.94,
            },
            menu_outer_border: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.48,
            },
            menu_inner_border: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.20,
            },
            menu_hover_fill: D2D1_COLOR_F {
                r: 0.039,
                g: 0.518,
                b: 1.0,
                a: 0.94,
            },
            menu_separator: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.14,
            },
            text_primary: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.95,
            },
            text_muted: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.52,
            },
            text_disabled: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.34,
            },
            dock_fill: D2D1_COLOR_F {
                r: 0.11,
                g: 0.11,
                b: 0.14,
                a: 0.68,
            },
            dock_inner_border: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.22,
            },
            dock_outer_border: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.42,
            },
            dock_divider: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.22,
            },
            dock_dot: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.92,
            },
            hover_pill: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.16,
            },
            card_fill: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.08,
            },
            slider_track: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.16,
            },
            slider_fill: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.92,
            },
            toggle_active: D2D1_COLOR_F {
                r: 0.039,
                g: 0.518,
                b: 1.0,
                a: 1.0,
            },
            toggle_inactive: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.12,
            },
        }
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn light() -> Self {
        Self {
            bar_background: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.38,
            },
            menu_fill: D2D1_COLOR_F {
                r: 0.96,
                g: 0.96,
                b: 0.98,
                a: 0.88,
            },
            menu_outer_border: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.14,
            },
            menu_inner_border: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.65,
            },
            menu_hover_fill: D2D1_COLOR_F {
                r: 0.039,
                g: 0.518,
                b: 1.0,
                a: 0.94,
            },
            menu_separator: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.08,
            },
            text_primary: D2D1_COLOR_F {
                r: 0.08,
                g: 0.08,
                b: 0.10,
                a: 0.94,
            },
            text_muted: D2D1_COLOR_F {
                r: 0.20,
                g: 0.20,
                b: 0.24,
                a: 0.58,
            },
            text_disabled: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.26,
            },
            dock_fill: D2D1_COLOR_F {
                r: 0.98,
                g: 0.98,
                b: 1.0,
                a: 0.72,
            },
            dock_inner_border: D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.70,
            },
            dock_outer_border: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.16,
            },
            dock_divider: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.14,
            },
            dock_dot: D2D1_COLOR_F {
                r: 0.10,
                g: 0.10,
                b: 0.12,
                a: 0.90,
            },
            hover_pill: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.08,
            },
            card_fill: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.05,
            },
            slider_track: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.10,
            },
            slider_fill: D2D1_COLOR_F {
                r: 0.039,
                g: 0.518,
                b: 1.0,
                a: 0.90,
            },
            toggle_active: D2D1_COLOR_F {
                r: 0.039,
                g: 0.518,
                b: 1.0,
                a: 1.0,
            },
            toggle_inactive: D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.08,
            },
        }
    }
}

pub struct BrushPalette {
    pub solid: ID2D1SolidColorBrush,
    pub stroke: ID2D1SolidColorBrush,
    pub text_primary: ID2D1SolidColorBrush,
    pub text_muted: ID2D1SolidColorBrush,
    pub hover: ID2D1SolidColorBrush,
    pub accent: ID2D1SolidColorBrush,
    pub black: ID2D1SolidColorBrush,
    pub white: ID2D1SolidColorBrush,
    scratches: [ID2D1SolidColorBrush; 8],
}

impl BrushPalette {
    pub fn new(target: &ID2D1DCRenderTarget) -> Result<Self> {
        let clear = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
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
                scratches: [
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                    target.CreateSolidColorBrush(&clear, None)?,
                ],
            })
        }
    }

    pub fn update_theme(&self, palette: &LiquidGlassPalette) {
        unsafe {
            self.text_primary.SetColor(&palette.text_primary);
            self.text_muted.SetColor(&palette.text_muted);
            self.hover.SetColor(&palette.hover_pill);
            self.accent.SetColor(&palette.slider_fill);
            self.stroke.SetColor(&palette.menu_inner_border);
        }
    }

    #[must_use]
    pub fn scratch(&self, slot: usize, color: D2D1_COLOR_F) -> &ID2D1SolidColorBrush {
        let brush = &self.scratches[slot % self.scratches.len()];
        unsafe {
            brush.SetColor(&color);
        }
        brush
    }

    #[must_use]
    pub fn alpha_brush(
        &self,
        slot: usize,
        base: D2D1_COLOR_F,
        alpha: f32,
    ) -> &ID2D1SolidColorBrush {
        self.scratch(
            slot,
            D2D1_COLOR_F {
                r: base.r,
                g: base.g,
                b: base.b,
                a: alpha,
            },
        )
    }
}
