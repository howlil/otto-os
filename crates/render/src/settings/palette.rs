use otto_core::{
    BadgeTone, ContrastColors, Rgba, SettingsPage, settings_background, settings_text,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};

pub(super) const CARD_RADIUS_POINTS: i32 = 12;
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

use crate::appearance::tokens::{
    FOCUS_PURPLE, SYSTEM_BLUE, SYSTEM_BLUE_LIGHT, SYSTEM_CYAN, SYSTEM_GRAY, SYSTEM_GREEN,
    SYSTEM_GREEN_LIGHT, SYSTEM_INDIGO, SYSTEM_ORANGE, SYSTEM_ORANGE_LIGHT, SYSTEM_PINK,
    SYSTEM_PURPLE, SYSTEM_RED, SYSTEM_RED_LIGHT, SYSTEM_TEAL,
};
pub(super) use crate::appearance::tokens::{WHITE, from_rgba, rgba_hex as rgba, with_alpha};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) glass: bool,
    pub(super) high_contrast: bool,
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

impl Palette {
    pub(super) const fn pick(
        dark: bool,
        glass: bool,
        high_contrast: bool,
        contrast: Option<ContrastColors>,
        accent: Rgba,
        on_accent: Rgba,
    ) -> Self {
        if let Some(system) = contrast {
            return Self::from_contrast(system, dark);
        }
        let mut base = Self::solid(dark, high_contrast);
        if !high_contrast {
            base.accent = from_rgba(accent);
            base.on_accent = from_rgba(on_accent);
            base.switch_on = base.accent;
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

    const fn from_contrast(system: ContrastColors, dark: bool) -> Self {
        let base = Self::solid(dark, true);
        let window = from_rgba(system.window);
        let text = from_rgba(system.window_text);
        let highlight = from_rgba(system.highlight);
        let highlight_text = from_rgba(system.highlight_text);
        let button_face = from_rgba(system.button_face);
        let gray_text = from_rgba(system.gray_text);
        Self {
            glass: false,
            high_contrast: true,
            background: window,
            sidebar: window,
            card: button_face,
            surface: window,
            track: button_face,
            segment_active: button_face,
            chip: button_face,
            bar_total: gray_text,
            bar_total_stroke: text,
            hairline: text,
            text,
            secondary: text,
            accent: highlight,
            on_accent: highlight_text,
            hover: with_alpha(text, 0.14),
            switch_on: highlight,
            switch_off: gray_text,
            keycap: button_face,
            ..base
        }
    }

    const fn solid(dark: bool, high_contrast: bool) -> Self {
        if dark {
            Self::solid_dark(high_contrast)
        } else {
            Self::solid_light(high_contrast)
        }
    }

    const fn solid_dark(high_contrast: bool) -> Self {
        Self {
            glass: false,
            high_contrast,
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
            accent: SYSTEM_BLUE,
            on_accent: WHITE,
            hover: rgba(0xFF_FFFF, if high_contrast { 0.16 } else { 0.07 }),
            switch_on: SYSTEM_BLUE,
            switch_off: rgba(0x5A_5A5F, 1.0),
            keycap: rgba(0x3E_3E42, 1.0),
            good: SYSTEM_GREEN,
            neutral: SYSTEM_GRAY,
            warning: SYSTEM_ORANGE,
            bad: SYSTEM_RED,
        }
    }

    const fn solid_light(high_contrast: bool) -> Self {
        Self {
            glass: false,
            high_contrast,
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
            accent: SYSTEM_BLUE_LIGHT,
            on_accent: WHITE,
            hover: rgba(0x00_0000, if high_contrast { 0.10 } else { 0.05 }),
            switch_on: SYSTEM_BLUE_LIGHT,
            switch_off: rgba(0xD1_D1D6, 1.0),
            keycap: rgba(0xE9_E9EE, 1.0),
            good: SYSTEM_GREEN_LIGHT,
            neutral: SYSTEM_GRAY,
            warning: SYSTEM_ORANGE_LIGHT,
            bad: SYSTEM_RED_LIGHT,
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
    match page {
        SettingsPage::Overview | SettingsPage::About => SYSTEM_BLUE,
        SettingsPage::Modules => SYSTEM_PURPLE,
        SettingsPage::TopBar => SYSTEM_CYAN,
        SettingsPage::Notch => rgba(0x3A_3A3C, 1.0),
        SettingsPage::Focus => SYSTEM_INDIGO,
        SettingsPage::Dock => FOCUS_PURPLE,
        SettingsPage::WindowControls => SYSTEM_ORANGE,
        SettingsPage::Spotlight | SettingsPage::General => SYSTEM_GRAY,
        SettingsPage::ControlCenter => rgba(0x63_6366, 1.0),
        SettingsPage::Appearance => SYSTEM_TEAL,
        SettingsPage::Shortcuts => SYSTEM_PINK,
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
