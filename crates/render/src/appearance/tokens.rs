use otto_core::{CategoryColor, Rgba};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub(crate) fn category_d2d_color(color: CategoryColor) -> D2D1_COLOR_F {
    let (r, g, b) = color.rgb();
    rgb(r, g, b, 1.0)
}

pub(crate) const fn rgb(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

pub(crate) const fn rgba_hex(hex: u32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: alpha,
    }
}

pub(crate) const fn from_rgba(color: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

pub(crate) const fn with_alpha(color: D2D1_COLOR_F, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        a: color.a * alpha,
        ..color
    }
}

pub(crate) const fn brighten(color: D2D1_COLOR_F, amount: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

pub(crate) const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub(crate) const BLACK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub(crate) const WHITE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

pub(crate) const FOREGROUND: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.95,
};

pub(crate) const SYSTEM_BLUE: D2D1_COLOR_F = rgb(10, 132, 255, 1.0);
pub(crate) const ACCENT_BLUE: D2D1_COLOR_F = SYSTEM_BLUE;
pub(crate) const SYSTEM_BLUE_LIGHT: D2D1_COLOR_F = rgb(0, 122, 255, 1.0);
pub(crate) const SYSTEM_GREEN: D2D1_COLOR_F = rgb(48, 209, 88, 1.0);
pub(crate) const SYSTEM_GREEN_LIGHT: D2D1_COLOR_F = rgb(52, 199, 89, 1.0);
pub(crate) const SYSTEM_ORANGE: D2D1_COLOR_F = rgb(255, 159, 10, 1.0);
pub(crate) const SYSTEM_ORANGE_LIGHT: D2D1_COLOR_F = rgb(255, 149, 0, 1.0);
pub(crate) const SYSTEM_RED: D2D1_COLOR_F = rgb(255, 69, 58, 1.0);
pub(crate) const SYSTEM_RED_LIGHT: D2D1_COLOR_F = rgb(255, 59, 48, 1.0);
pub(crate) const SYSTEM_GRAY: D2D1_COLOR_F = rgb(142, 142, 147, 1.0);
pub(crate) const SYSTEM_PURPLE: D2D1_COLOR_F = rgb(175, 82, 222, 1.0);
pub(crate) const SYSTEM_CYAN: D2D1_COLOR_F = rgb(50, 173, 230, 1.0);
pub(crate) const SYSTEM_INDIGO: D2D1_COLOR_F = rgb(94, 92, 230, 1.0);
pub(crate) const SYSTEM_TEAL: D2D1_COLOR_F = rgb(48, 176, 199, 1.0);
pub(crate) const SYSTEM_PINK: D2D1_COLOR_F = rgb(255, 55, 95, 1.0);
pub(crate) const FOCUS_PURPLE: D2D1_COLOR_F = rgb(88, 86, 214, 1.0);

pub(crate) const FOLDER_BLUE: D2D1_COLOR_F = rgb(96, 194, 255, 1.0);
pub(crate) const FOLDER_BACK: D2D1_COLOR_F = rgb(42, 152, 240, 1.0);
pub(crate) const DOC_PAGE: D2D1_COLOR_F = rgb(235, 240, 250, 0.96);
pub(crate) const DOC_CORNER: D2D1_COLOR_F = rgb(184, 199, 224, 0.95);

pub(crate) const FOCUS_ORANGE: D2D1_COLOR_F = rgb(255, 146, 48, 1.0);
pub(crate) const LONG_BREAK_BLUE: D2D1_COLOR_F = rgb(0, 145, 255, 1.0);
pub(crate) const URGENT_CORAL: D2D1_COLOR_F = rgb(255, 66, 69, 1.0);
pub(crate) const CALL_ACCEPT_GREEN: D2D1_COLOR_F = rgb(36, 161, 72, 1.0);
pub(crate) const CHARGING_GREEN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.19,
    g: 0.82,
    b: 0.35,
    a: 0.96,
};
pub(crate) const AMBER_WARN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.62,
    b: 0.04,
    a: 0.96,
};
pub(crate) const MUSIC_ACCENT: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.988,
    g: 0.235,
    b: 0.329,
    a: 1.0,
};
pub(crate) const EQUALIZER_GREEN: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.188,
    g: 0.820,
    b: 0.345,
    a: 1.0,
};

pub(crate) const fn pressed_overlay(
    hover_pill: D2D1_COLOR_F,
    is_high_contrast: bool,
) -> D2D1_COLOR_F {
    if is_high_contrast {
        return hover_pill;
    }
    let raised = hover_pill.a * 1.6;
    D2D1_COLOR_F {
        a: if raised > 1.0 { 1.0 } else { raised },
        ..hover_pill
    }
}
