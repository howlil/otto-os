use otto_core::{BadgeTone, Rect, Rgba, SettingsPage, settings_background, settings_text};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};

pub(super) const CARD_RADIUS_POINTS: i32 = 10;
pub(super) const ROW_PAD_POINTS: i32 = 16;
pub(super) const ROW_TILE_POINTS: i32 = 28;
pub(super) const SIDEBAR_TILE_POINTS: i32 = 24;
pub(super) const SIDEBAR_RADIUS_POINTS: i32 = 8;
pub(super) const SWITCH_WIDTH_POINTS: i32 = 38;
pub(super) const SWITCH_HEIGHT_POINTS: i32 = 22;
pub(super) const BADGE_WIDTH_POINTS: i32 = 84;
pub(super) const DOT_POINTS: i32 = 8;
pub(super) const KEYCAP_HEIGHT_POINTS: i32 = 22;
pub(super) const KEYCAP_GAP_POINTS: i32 = 4;
pub(super) const HERO_TILE_POINTS: i32 = 76;

pub(super) const fn rgba(hex: u32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: alpha,
    }
}

pub(super) const WHITE: D2D1_COLOR_F = rgba(0xFF_FFFF, 1.0);

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) glass: bool,
    pub(super) background: D2D1_COLOR_F,
    pub(super) sidebar: D2D1_COLOR_F,
    pub(super) card: D2D1_COLOR_F,
    pub(super) surface: D2D1_COLOR_F,
    pub(super) track: D2D1_COLOR_F,
    pub(super) segment_active: D2D1_COLOR_F,
    pub(super) chip: D2D1_COLOR_F,
    pub(super) bar_total: D2D1_COLOR_F,
    pub(super) bar_total_stroke: D2D1_COLOR_F,
    pub(super) hairline: D2D1_COLOR_F,
    pub(super) text: D2D1_COLOR_F,
    pub(super) secondary: D2D1_COLOR_F,
    pub(super) accent: D2D1_COLOR_F,
    pub(super) on_accent: D2D1_COLOR_F,
    pub(super) hover: D2D1_COLOR_F,
    pub(super) switch_on: D2D1_COLOR_F,
    pub(super) switch_off: D2D1_COLOR_F,
    pub(super) keycap: D2D1_COLOR_F,
    pub(super) good: D2D1_COLOR_F,
    pub(super) neutral: D2D1_COLOR_F,
    pub(super) warning: D2D1_COLOR_F,
    pub(super) bad: D2D1_COLOR_F,
}

pub(super) const fn from_rgba(color: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

impl Palette {
    pub(super) const fn pick(
        dark: bool,
        glass: bool,
        high_contrast: bool,
        accent: Rgba,
        on_accent: Rgba,
    ) -> Self {
        let mut base = Self::solid(dark, high_contrast);
        if !high_contrast {
            base.accent = from_rgba(accent);
            base.on_accent = from_rgba(on_accent);
        }
        if !glass || high_contrast {
            return base;
        }
        Self {
            glass: true,
            background: rgba(0, 0.0),
            sidebar: if dark {
                rgba(0x00_0000, 0.14)
            } else {
                rgba(0x00_0000, 0.03)
            },
            card: if dark {
                rgba(0xFF_FFFF, 0.055)
            } else {
                rgba(0xFF_FFFF, 0.72)
            },
            surface: if dark {
                rgba(0x00_0000, 0.22)
            } else {
                rgba(0xFF_FFFF, 0.88)
            },
            track: if dark {
                rgba(0xFF_FFFF, 0.07)
            } else {
                rgba(0x00_0000, 0.055)
            },
            segment_active: if dark {
                rgba(0xFF_FFFF, 0.14)
            } else {
                rgba(0xFF_FFFF, 0.94)
            },
            chip: if dark {
                rgba(0xFF_FFFF, 0.08)
            } else {
                rgba(0x00_0000, 0.05)
            },
            bar_total: if dark {
                rgba(0xFF_FFFF, 0.16)
            } else {
                rgba(0x00_0000, 0.12)
            },
            bar_total_stroke: if dark {
                rgba(0xFF_FFFF, 0.30)
            } else {
                rgba(0x00_0000, 0.26)
            },
            hairline: if dark {
                rgba(0xFF_FFFF, 0.09)
            } else {
                rgba(0x00_0000, 0.09)
            },
            hover: if dark {
                rgba(0xFF_FFFF, 0.08)
            } else {
                rgba(0x00_0000, 0.05)
            },
            keycap: if dark {
                rgba(0xFF_FFFF, 0.12)
            } else {
                rgba(0xFF_FFFF, 0.85)
            },
            ..base
        }
    }

    const fn solid(dark: bool, high_contrast: bool) -> Self {
        if dark {
            Self {
                glass: false,
                background: rgba(
                    if high_contrast {
                        0x00_0000
                    } else {
                        settings_background(true)
                    },
                    1.0,
                ),
                sidebar: rgba(if high_contrast { 0x0A_0A0C } else { 0x26_2629 }, 1.0),
                card: rgba(if high_contrast { 0x10_1014 } else { 0x2B_2B2E }, 1.0),
                surface: rgba(if high_contrast { 0x00_0000 } else { 0x20_2023 }, 1.0),
                track: rgba(if high_contrast { 0x1E_1E24 } else { 0x36_363A }, 1.0),
                segment_active: rgba(if high_contrast { 0x24_242B } else { 0x3D_3D42 }, 1.0),
                chip: rgba(if high_contrast { 0x1A_1A20 } else { 0x36_363A }, 1.0),
                bar_total: rgba(if high_contrast { 0x3E_3E46 } else { 0x48_484E }, 1.0),
                bar_total_stroke: rgba(if high_contrast { 0xFF_FFFF } else { 0x7A_7A82 }, 1.0),
                hairline: rgba(0xFF_FFFF, if high_contrast { 0.45 } else { 0.10 }),
                text: rgba(
                    if high_contrast {
                        0xFF_FFFF
                    } else {
                        settings_text(true)
                    },
                    1.0,
                ),
                secondary: rgba(if high_contrast { 0xD8_D8DE } else { 0xA1_A1A6 }, 1.0),
                accent: rgba(0x0A_84FF, 1.0),
                on_accent: WHITE,
                hover: rgba(0xFF_FFFF, if high_contrast { 0.16 } else { 0.07 }),
                switch_on: rgba(0x30_D158, 1.0),
                switch_off: rgba(0x5A_5A5F, 1.0),
                keycap: rgba(0x3E_3E42, 1.0),
                good: rgba(0x30_D158, 1.0),
                neutral: rgba(0x8E_8E93, 1.0),
                warning: rgba(0xFF_9F0A, 1.0),
                bad: rgba(0xFF_453A, 1.0),
            }
        } else {
            Self {
                glass: false,
                background: rgba(
                    if high_contrast {
                        0xFF_FFFF
                    } else {
                        settings_background(false)
                    },
                    1.0,
                ),
                sidebar: rgba(if high_contrast { 0xF4_F4F6 } else { 0xEA_EAEF }, 1.0),
                card: rgba(0xFF_FFFF, 1.0),
                surface: rgba(if high_contrast { 0xFF_FFFF } else { 0xF7_F7FA }, 1.0),
                track: rgba(if high_contrast { 0xE2_E2E8 } else { 0xEB_EBF0 }, 1.0),
                segment_active: rgba(0xFF_FFFF, 1.0),
                chip: rgba(if high_contrast { 0xEC_ECF2 } else { 0xEF_EFF4 }, 1.0),
                bar_total: rgba(if high_contrast { 0xC8_C8D0 } else { 0xD5_D5DC }, 1.0),
                bar_total_stroke: rgba(if high_contrast { 0x00_0000 } else { 0x8E_8E96 }, 1.0),
                hairline: rgba(0x00_0000, if high_contrast { 0.45 } else { 0.10 }),
                text: rgba(
                    if high_contrast {
                        0x00_0000
                    } else {
                        settings_text(false)
                    },
                    1.0,
                ),
                secondary: rgba(if high_contrast { 0x26_262A } else { 0x63_6366 }, 1.0),
                accent: rgba(0x00_7AFF, 1.0),
                on_accent: WHITE,
                hover: rgba(0x00_0000, if high_contrast { 0.10 } else { 0.05 }),
                switch_on: rgba(0x34_C759, 1.0),
                switch_off: rgba(0xD1_D1D6, 1.0),
                keycap: rgba(0xE9_E9EE, 1.0),
                good: rgba(0x34_C759, 1.0),
                neutral: rgba(0x8E_8E93, 1.0),
                warning: rgba(0xFF_9500, 1.0),
                bad: rgba(0xFF_3B30, 1.0),
            }
        }
    }

    pub(super) const fn tone(&self, tone: BadgeTone) -> D2D1_COLOR_F {
        match tone {
            BadgeTone::Good => self.good,
            BadgeTone::Neutral => self.neutral,
            BadgeTone::Warning => self.warning,
            BadgeTone::Bad => self.bad,
        }
    }
}

pub(super) const fn tint(page: SettingsPage) -> D2D1_COLOR_F {
    let hex = match page {
        SettingsPage::Overview | SettingsPage::About => 0x0A_84FF,
        SettingsPage::Modules => 0xAF_52DE,
        SettingsPage::TopBar => 0x32_ADE6,
        SettingsPage::Notch => 0x3A_3A3C,
        SettingsPage::Focus => 0x58_56D6,
        SettingsPage::Dock => 0x5E_5CE6,
        SettingsPage::WindowControls => 0xFF_9F0A,
        SettingsPage::Spotlight | SettingsPage::General => 0x8E_8E93,
        SettingsPage::ControlCenter => 0x63_6366,
        SettingsPage::Appearance => 0x30_B0C7,
        SettingsPage::Shortcuts => 0xFF_375F,
    };
    rgba(hex, 1.0)
}

pub(super) const fn with_alpha(color: D2D1_COLOR_F, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        a: color.a * alpha,
        ..color
    }
}

pub(super) const fn d2d(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: (rect.x + rect.width) as f32,
        bottom: (rect.y + rect.height) as f32,
    }
}

pub(super) const fn rect_f(x: f32, y: f32, width: f32, height: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}
