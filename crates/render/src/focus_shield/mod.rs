use otto_core::{FocusShieldLayout, Rect, focus_shield_layout, scale};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ROUNDED_RECT,
    ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    IDWriteTextFormat,
};
use windows::core::Result;

use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery,
};
use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
const SCRIM: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.03,
    g: 0.03,
    b: 0.04,
    a: 0.86,
};
const CARD_EDGE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.16,
};
const CARD: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.13,
    g: 0.13,
    b: 0.15,
    a: 1.0,
};
const ACCENT: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.145,
    g: 0.537,
    b: 0.976,
    a: 1.0,
};
const PRIMARY_TEXT: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const SECONDARY_TEXT: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.66,
};

const CARD_RADIUS_DIP: i32 = 18;
const TITLE_TOP_DIP: i32 = 36;
const SUBJECT_TOP_DIP: i32 = 70;
const CAPTION_TOP_DIP: i32 = 104;
const LINE_HEIGHT_DIP: i32 = 26;

pub struct FocusShieldRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    surface: DibSurface,
}

impl FocusShieldRenderer {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        let factory = shared_d2d_factory()?;
        let target = create_dc_render_target(&factory)?;
        let surface = DibSurface::new(width, height)?;
        Ok(Self {
            factory,
            target,
            surface,
        })
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        origin: (i32, i32),
        dpi: u32,
        subject: &str,
    ) -> Result<()> {
        let size = self.surface.size();
        draw_with_recovery(
            self,
            |renderer| renderer.draw(size, dpi, subject),
            Self::recreate_target,
        )?;
        self.surface.present(hwnd, origin)
    }

    fn recreate_target(&mut self) -> Result<()> {
        self.target = create_dc_render_target(&self.factory)?;
        Ok(())
    }

    fn draw(&self, size: (i32, i32), dpi: u32, subject: &str) -> Result<()> {
        let layout = focus_shield_layout(size.0, size.1, dpi);
        draw_on(&self.target, &self.surface, size, || {
            self.paint(size, dpi, &layout, subject)
        })
    }

    fn paint(
        &self,
        size: (i32, i32),
        dpi: u32,
        layout: &FocusShieldLayout,
        subject: &str,
    ) -> Result<()> {
        let dwrite = shared_dwrite_factory()?;
        let title_format = unsafe {
            create_text_format(
                &dwrite,
                px(dpi, 18),
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?
        };
        let subject_format = unsafe {
            create_text_format(
                &dwrite,
                px(dpi, 15),
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?
        };
        let caption_format = unsafe {
            create_text_format(
                &dwrite,
                px(dpi, 12),
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?
        };
        let button_format = unsafe {
            create_text_format(
                &dwrite,
                px(dpi, 14),
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?
        };
        unsafe { button_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)? };

        let clear = CLEAR;
        unsafe { self.target.Clear(Some(&raw const clear)) };
        let card = layout.card;
        let radius = scale(CARD_RADIUS_DIP, dpi) as f32;
        self.fill(
            Rect {
                x: 0,
                y: 0,
                width: size.0,
                height: size.1,
            },
            SCRIM,
            0.0,
        )?;
        self.fill(card, CARD_EDGE, radius)?;
        self.fill(
            Rect {
                x: card.x + 1,
                y: card.y + 1,
                width: card.width - 2,
                height: card.height - 2,
            },
            CARD,
            radius - 1.0,
        )?;
        self.text(
            "Blocked during Focus",
            &title_format,
            band(card, dpi, TITLE_TOP_DIP),
            PRIMARY_TEXT,
        )?;
        self.text(
            subject,
            &subject_format,
            band(card, dpi, SUBJECT_TOP_DIP),
            PRIMARY_TEXT,
        )?;
        self.text(
            "Back returns you to where you were. Esc also works.",
            &caption_format,
            band(card, dpi, CAPTION_TOP_DIP),
            SECONDARY_TEXT,
        )?;
        let back = layout.back;
        self.fill(back, ACCENT, back.height as f32 / 2.0)?;
        self.text("Back", &button_format, back, PRIMARY_TEXT)
    }

    fn fill(&self, rect: Rect, color: D2D1_COLOR_F, radius: f32) -> Result<()> {
        let brush = self.brush(color)?;
        let rounded = D2D1_ROUNDED_RECT {
            rect: to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe { self.target.FillRoundedRectangle(&rounded, &brush) };
        Ok(())
    }

    fn text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: Rect,
        color: D2D1_COLOR_F,
    ) -> Result<()> {
        let brush = self.brush(color)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let bounds = to_d2d(rect);
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                &bounds,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }

    fn brush(&self, color: D2D1_COLOR_F) -> Result<ID2D1SolidColorBrush> {
        unsafe { self.target.CreateSolidColorBrush(&color, None) }
    }
}

fn px(dpi: u32, points: i32) -> f32 {
    scale(points, dpi) as f32
}

fn band(card: Rect, dpi: u32, top_dip: i32) -> Rect {
    Rect {
        x: card.x,
        y: card.y + scale(top_dip, dpi),
        width: card.width,
        height: scale(LINE_HEIGHT_DIP, dpi),
    }
}

fn to_d2d(rect: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.x as f32,
        top: rect.y as f32,
        right: rect.right() as f32,
        bottom: rect.bottom() as f32,
    }
}
