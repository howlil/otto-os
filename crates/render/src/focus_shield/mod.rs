use otto_core::{Appearance, FocusShieldLayout, Rect, focus_shield_layout, scale};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, ID2D1DCRenderTarget, ID2D1Factory,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_TEXT_ALIGNMENT_CENTER, IDWriteTextFormat,
};
use windows::core::Result;

use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery,
};
use crate::appearance::theme::{BrushPalette, LiquidGlassPalette};
use crate::kit::draw::{
    draw_focus_ring_for, draw_framed_squircle_shell, draw_text_on, fill_squircle_or_rounded,
    inset_rect_f,
};
use crate::{CLEAR, create_text_format, rect_to_d2d, shared_d2d_factory, shared_dwrite_factory};

const DARK_SCRIM: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.03,
    g: 0.03,
    b: 0.04,
    a: 0.86,
};
const LIGHT_SCRIM: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.94,
    g: 0.95,
    b: 0.97,
    a: 0.84,
};
const CARD_RADIUS_DIP: i32 = 18;
const TITLE_TOP_DIP: i32 = 36;
const SUBJECT_TOP_DIP: i32 = 70;
const CAPTION_TOP_DIP: i32 = 104;
const LINE_HEIGHT_DIP: i32 = 26;

struct ShieldFormats {
    dpi: u32,
    title: IDWriteTextFormat,
    subject: IDWriteTextFormat,
    caption: IDWriteTextFormat,
    button: IDWriteTextFormat,
}

impl ShieldFormats {
    fn new(dpi: u32) -> Result<Self> {
        let dwrite = shared_dwrite_factory()?;
        let make = |points: i32, weight| unsafe {
            create_text_format(
                &dwrite,
                scale(points, dpi) as f32,
                weight,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )
        };
        Ok(Self {
            dpi,
            title: make(18, DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
            subject: make(15, DWRITE_FONT_WEIGHT_MEDIUM)?,
            caption: make(12, DWRITE_FONT_WEIGHT_REGULAR)?,
            button: make(14, DWRITE_FONT_WEIGHT_SEMI_BOLD)?,
        })
    }
}

pub struct FocusShieldRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    brushes: BrushPalette,
    formats: ShieldFormats,
    surface: DibSurface,
}

impl FocusShieldRenderer {
    pub fn new(width: i32, height: i32, dpi: u32) -> Result<Self> {
        let dpi = dpi.max(96);
        let factory = shared_d2d_factory()?;
        let target = create_dc_render_target(&factory)?;
        let brushes = BrushPalette::new(&target)?;
        let formats = ShieldFormats::new(dpi)?;
        let surface = DibSurface::new(width, height)?;
        Ok(Self {
            factory,
            target,
            brushes,
            formats,
            surface,
        })
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        origin: (i32, i32),
        dpi: u32,
        subject: &str,
        appearance: Appearance,
    ) -> Result<()> {
        let dpi = dpi.max(96);
        if self.formats.dpi != dpi {
            self.formats = ShieldFormats::new(dpi)?;
        }
        let size = self.surface.size();
        draw_with_recovery(
            self,
            |renderer| renderer.draw(size, dpi, subject, &appearance),
            Self::recreate_target,
        )?;
        self.surface.present(hwnd, origin)
    }

    fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        Ok(())
    }

    fn draw(
        &self,
        size: (i32, i32),
        dpi: u32,
        subject: &str,
        appearance: &Appearance,
    ) -> Result<()> {
        let layout = focus_shield_layout(size.0, size.1, dpi);
        let palette = LiquidGlassPalette::for_appearance(appearance);
        self.brushes.update_theme(&palette);
        draw_on(&self.target, &self.surface, size, || {
            self.paint(size, dpi, &layout, subject, &palette);
            Ok(())
        })
    }

    fn paint(
        &self,
        size: (i32, i32),
        dpi: u32,
        layout: &FocusShieldLayout,
        subject: &str,
        palette: &LiquidGlassPalette,
    ) {
        let clear = CLEAR;
        unsafe { self.target.Clear(Some(&raw const clear)) };
        let scrim = if palette.is_high_contrast {
            palette.bar_background
        } else if palette.is_dark {
            DARK_SCRIM
        } else {
            LIGHT_SCRIM
        };
        let card_fill = if palette.is_high_contrast {
            palette.menu_fill
        } else {
            D2D1_COLOR_F {
                a: 1.0,
                ..palette.menu_fill
            }
        };

        let card = layout.card;
        let radius = scale(CARD_RADIUS_DIP, dpi) as f32;
        let full = Rect {
            x: 0,
            y: 0,
            width: size.0,
            height: size.1,
        };
        let scrim_brush = self.brushes.color_brush(scrim);
        unsafe {
            self.target.FillRectangle(&rect_to_d2d(full), &scrim_brush);
        }

        let card_brush = self.brushes.color_brush(card_fill);
        let outer_border = self.brushes.color_brush(palette.menu_outer_border);
        draw_framed_squircle_shell(
            &self.factory,
            &self.target,
            card,
            radius,
            &card_brush,
            &outer_border,
            &self.brushes.specular_rim,
        );

        let text_options = D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP;
        draw_text_on(
            &self.target,
            "Blocked during Focus",
            &self.formats.title,
            rect_to_d2d(band(card, dpi, TITLE_TOP_DIP)),
            &self.brushes.text_primary,
            text_options,
        );
        draw_text_on(
            &self.target,
            subject,
            &self.formats.subject,
            rect_to_d2d(band(card, dpi, SUBJECT_TOP_DIP)),
            &self.brushes.text_primary,
            text_options,
        );
        draw_text_on(
            &self.target,
            "Back returns you to where you were. Esc also works.",
            &self.formats.caption,
            rect_to_d2d(band(card, dpi, CAPTION_TOP_DIP)),
            &self.brushes.text_muted,
            text_options,
        );

        let back = layout.back;
        let button_radius = back.height as f32 * 0.5;
        let button_brush = self.brushes.color_brush(palette.toggle_active);
        fill_squircle_or_rounded(
            &self.factory,
            &self.target,
            back,
            button_radius,
            dpi,
            &button_brush,
        );
        let ring_expand = scale(3, dpi) as f32;
        let focus_rect = inset_rect_f(rect_to_d2d(back), -ring_expand);
        draw_focus_ring_for(
            &self.factory,
            &self.target,
            focus_rect,
            button_radius + ring_expand,
            dpi,
            &self.brushes.text_primary,
        );
        draw_text_on(
            &self.target,
            "Back",
            &self.formats.button,
            rect_to_d2d(back),
            &self.brushes.on_accent,
            text_options,
        );
    }
}

fn band(card: Rect, dpi: u32, top_dip: i32) -> Rect {
    let pad = scale(20, dpi);
    Rect {
        x: card.x + pad,
        y: card.y + scale(top_dip, dpi),
        width: (card.width - pad * 2).max(1),
        height: scale(LINE_HEIGHT_DIP, dpi),
    }
}
