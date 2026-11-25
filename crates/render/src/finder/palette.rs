use otto_core::{
    BarWindowControlsView, FinderHit, FinderLayout, FinderState, FinderTreeRow, Rect, Rgba,
    settings_background, settings_text,
};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
    IDWriteTextFormat,
};
use windows::core::Result;

use crate::appearance::tokens::{WHITE, from_rgba, rgba_hex as rgba, with_alpha};
use crate::kit::canvas::make_text_format;

pub struct FinderPaintState<'a> {
    pub state: &'a FinderState,
    pub layout: &'a FinderLayout,
    pub list_rows: &'a [FinderTreeRow],
    pub hovered: Option<&'a FinderHit>,
    pub marquee: Option<Rect>,
    pub window_controls: BarWindowControlsView,
    pub dark: bool,
    pub glass: bool,
    pub high_contrast: bool,
    pub accent: Rgba,
    pub on_accent: Rgba,
}

#[derive(Clone, Copy)]
pub(super) struct FinderPalette {
    pub(super) background: D2D1_COLOR_F,
    pub(super) sidebar: D2D1_COLOR_F,
    pub(super) toolbar: D2D1_COLOR_F,
    pub(super) content: D2D1_COLOR_F,
    pub(super) zebra: D2D1_COLOR_F,
    pub(super) card: D2D1_COLOR_F,
    pub(super) overlay_backdrop: D2D1_COLOR_F,
    pub(super) sheet: D2D1_COLOR_F,
    pub(super) hairline: D2D1_COLOR_F,
    pub(super) text: D2D1_COLOR_F,
    pub(super) secondary: D2D1_COLOR_F,
    pub(super) muted: D2D1_COLOR_F,
    pub(super) accent: D2D1_COLOR_F,
    pub(super) on_accent: D2D1_COLOR_F,
    pub(super) selection_fill: D2D1_COLOR_F,
    pub(super) hover: D2D1_COLOR_F,
    pub(super) pill: D2D1_COLOR_F,
    pub(super) pill_active: D2D1_COLOR_F,
    pub(super) danger: D2D1_COLOR_F,
    pub(super) folder_blue: D2D1_COLOR_F,
}

impl FinderPalette {
    pub(super) fn pick(state: &FinderPaintState<'_>) -> Self {
        let dark = state.dark;
        let glass = state.glass && !state.high_contrast;
        let accent = if state.high_contrast {
            if dark { WHITE } else { rgba(0x00_0000, 1.0) }
        } else {
            from_rgba(state.accent)
        };
        let on_accent = if state.high_contrast {
            if dark { rgba(0x00_0000, 1.0) } else { WHITE }
        } else {
            from_rgba(state.on_accent)
        };
        let text = rgba(settings_text(dark), 1.0);
        let bg = rgba(settings_background(dark), 1.0);
        Self {
            background: if glass { rgba(0, 0.0) } else { bg },
            sidebar: if glass {
                if dark {
                    rgba(0x00_0000, 0.16)
                } else {
                    rgba(0x00_0000, 0.03)
                }
            } else if dark {
                rgba(0x23_2326, 1.0)
            } else {
                rgba(0xEB_EBF0, 1.0)
            },
            toolbar: if glass {
                if dark {
                    rgba(0x1E_1E20, 0.44)
                } else {
                    rgba(0xFF_FFFF, 0.56)
                }
            } else if dark {
                rgba(0x28_282B, 1.0)
            } else {
                rgba(0xF6_F6F8, 1.0)
            },
            content: if dark {
                rgba(0x1C_1C1E, if glass { 0.86 } else { 1.0 })
            } else {
                rgba(0xFF_FFFF, if glass { 0.92 } else { 1.0 })
            },
            zebra: if dark {
                rgba(0xFF_FFFF, 0.035)
            } else {
                rgba(0x00_0000, 0.025)
            },
            card: if dark {
                rgba(0x2C_2C2E, 0.96)
            } else {
                rgba(0xF5_F5F7, 0.98)
            },
            overlay_backdrop: rgba(0x00_0000, if dark { 0.56 } else { 0.34 }),
            sheet: if dark {
                rgba(0x24_2427, 0.98)
            } else {
                rgba(0xFC_FCFE, 0.99)
            },
            hairline: if dark {
                rgba(0xFF_FFFF, 0.10)
            } else {
                rgba(0x00_0000, 0.10)
            },
            text,
            secondary: with_alpha(text, 0.64),
            muted: with_alpha(text, 0.44),
            accent,
            on_accent,
            selection_fill: with_alpha(accent, if dark { 0.30 } else { 0.18 }),
            hover: if dark {
                rgba(0xFF_FFFF, 0.08)
            } else {
                rgba(0x00_0000, 0.06)
            },
            pill: if dark {
                rgba(0xFF_FFFF, 0.08)
            } else {
                rgba(0x00_0000, 0.06)
            },
            pill_active: if dark {
                rgba(0xFF_FFFF, 0.18)
            } else {
                rgba(0xFF_FFFF, 0.94)
            },
            danger: rgba(0xFF_3B30, 1.0),
            folder_blue: rgba(0x52_B6F9, 1.0),
        }
    }
}

pub(super) struct FinderFormats {
    pub(super) title: IDWriteTextFormat,
    pub(super) section: IDWriteTextFormat,
    pub(super) row: IDWriteTextFormat,
    pub(super) row_medium: IDWriteTextFormat,
    pub(super) row_trailing: IDWriteTextFormat,
    pub(super) small: IDWriteTextFormat,
    pub(super) small_center: IDWriteTextFormat,
    pub(super) small_wrap_center: IDWriteTextFormat,
    pub(super) code_wrap: IDWriteTextFormat,
    pub(super) badge: IDWriteTextFormat,
}

impl FinderFormats {
    pub(super) fn new(dpi: u32) -> Result<Self> {
        let make = |pt, weight, align, wrap| make_text_format(pt, dpi, weight, align, wrap);
        Ok(Self {
            title: make(
                14,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                false,
            )?,
            section: make(
                11,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                false,
            )?,
            row: make(
                13,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                false,
            )?,
            row_medium: make(
                13,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                false,
            )?,
            row_trailing: make(
                12,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
                false,
            )?,
            small: make(
                11,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                false,
            )?,
            small_center: make(
                11,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                false,
            )?,
            small_wrap_center: make(
                11,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                true,
            )?,
            code_wrap: make(
                12,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                true,
            )?,
            badge: make(
                10,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                false,
            )?,
        })
    }
}
