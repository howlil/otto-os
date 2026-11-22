use otto_core::{BadgeTone, Rect, SettingsPage, settings_background, settings_text};
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
pub(super) const BAR_WIDTH_POINTS: i32 = 8;
pub(super) const BAR_GAP_POINTS: i32 = 4;
pub(super) const BAR_INSET_POINTS: i32 = 14;
pub(super) const BAR_TRACK_ALPHA: f32 = 0.35;
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
    pub(super) rim: D2D1_COLOR_F,
    pub(super) background: D2D1_COLOR_F,
    pub(super) sidebar: D2D1_COLOR_F,
    pub(super) card: D2D1_COLOR_F,
    pub(super) hairline: D2D1_COLOR_F,
    pub(super) text: D2D1_COLOR_F,
    pub(super) secondary: D2D1_COLOR_F,
    pub(super) accent: D2D1_COLOR_F,
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
    pub(super) const fn pick(dark: bool, glass: bool) -> Self {
        let base = Self::solid(dark);
        if !glass {
            return base;
        }
        Self {
            glass: true,
            rim: if dark {
                rgba(0xFF_FFFF, 0.22)
            } else {
                rgba(0xFF_FFFF, 0.76)
            },
            background: rgba(settings_background(dark), 0.24),
            sidebar: if dark {
                rgba(0x06_080E, 0.34)
            } else {
                rgba(0xFF_FFFF, 0.34)
            },
            card: if dark {
                rgba(0x0D_1018, 0.38)
            } else {
                rgba(0xFF_FFFF, 0.56)
            },
            hairline: if dark {
                rgba(0xFF_FFFF, 0.14)
            } else {
                rgba(0x00_0000, 0.10)
            },
            hover: if dark {
                rgba(0xFF_FFFF, 0.10)
            } else {
                rgba(0x00_0000, 0.06)
            },
            keycap: if dark {
                rgba(0xFF_FFFF, 0.16)
            } else {
                rgba(0xFF_FFFF, 0.80)
            },
            ..base
        }
    }

    const fn solid(dark: bool) -> Self {
        if dark {
            Self {
                glass: false,
                rim: rgba(0, 0.0),
                background: rgba(settings_background(true), 1.0),
                sidebar: rgba(0x28_282B, 1.0),
                card: rgba(0x2C_2C2F, 1.0),
                hairline: rgba(0xFF_FFFF, 0.09),
                text: rgba(settings_text(true), 1.0),
                secondary: rgba(0xA1_A1A6, 1.0),
                accent: rgba(0x0A_84FF, 1.0),
                hover: rgba(0xFF_FFFF, 0.06),
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
                rim: rgba(0, 0.0),
                background: rgba(settings_background(false), 1.0),
                sidebar: rgba(0xE6_E6EB, 1.0),
                card: rgba(0xFF_FFFF, 1.0),
                hairline: rgba(0x00_0000, 0.10),
                text: rgba(settings_text(false), 1.0),
                secondary: rgba(0x6E_6E73, 1.0),
                accent: rgba(0x00_7AFF, 1.0),
                hover: rgba(0x00_0000, 0.05),
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
