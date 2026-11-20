mod chrome;
mod results;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use otto_core::{
    Appearance, PixelLimits, Rect, RgbBuffer, SpotlightLayout, SpotlightMode, SpotlightView, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{ID2D1Bitmap, ID2D1DCRenderTarget, ID2D1Factory};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, IDWriteTextFormat,
};
use windows::core::Result;

use crate::appearance::glass::GlassPainter;
use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery, upload_bgra_bitmap,
};
use crate::appearance::theme::{BrushPalette, LiquidGlassPalette};
use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

pub(super) const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

#[derive(Clone, Copy)]
pub struct SpotlightPaintEntry<'a> {
    pub key: &'a str,
    pub title: &'a str,
}

pub struct SpotlightPaintState<'paint, 'text> {
    pub layout: &'paint SpotlightLayout,
    pub entries: &'paint [SpotlightPaintEntry<'text>],
    pub mode: SpotlightMode,
    pub view: SpotlightView,
    pub query: &'paint str,
    pub cursor_byte: usize,
    pub selected_all: bool,
    pub math_result: Option<&'paint str>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub content_opacity: f32,
    pub appearance: Appearance,
}

pub(super) struct CachedSpotlightIcon {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: Vec<u8>,
    pub(super) bitmap: ID2D1Bitmap,
}

pub struct SpotlightRenderer {
    pub(super) factory: ID2D1Factory,
    pub(super) target: ID2D1DCRenderTarget,
    pub(super) brushes: BrushPalette,
    pub(super) search_format: IDWriteTextFormat,
    pub(super) label_format: IDWriteTextFormat,
    pub(super) icon_fallback_format: IDWriteTextFormat,
    pub(super) footer_format: IDWriteTextFormat,
    pub(super) surface: DibSurface,
    pub(super) dpi: u32,
    pub(super) glass: GlassPainter,
    pub(super) icons: HashMap<String, CachedSpotlightIcon>,
}

impl SpotlightRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = create_dc_render_target(&factory)?;
            let brushes = BrushPalette::new(&target)?;
            let dwrite = shared_dwrite_factory()?;
            let search_format = create_text_format(
                &dwrite,
                scale(17, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let label_format = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let icon_fallback_format = create_text_format(
                &dwrite,
                scale(25, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let footer_format = create_text_format(
                &dwrite,
                scale(10, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            Ok(Self {
                factory,
                target,
                brushes,
                search_format,
                label_format,
                icon_fallback_format,
                footer_format,
                surface: DibSurface::new(scale(1_200, dpi), scale(900, dpi))?,
                dpi,
                glass: GlassPainter::default(),
                icons: HashMap::new(),
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn clear_backdrop(&self) {
        self.glass.clear_backdrop();
    }

    #[must_use]
    pub fn has_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    #[must_use]
    pub fn has_icon(&self, key: &str) -> bool {
        self.icons.contains_key(key)
    }

    pub fn retain_visible_icons(&mut self, visible_keys: &HashSet<&str>) {
        self.icons
            .retain(|key, _| visible_keys.contains(key.as_str()));
    }

    pub fn set_icon(&mut self, key: String, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
        self.surface.bind(&self.target, self.surface.size())?;
        let bitmap = upload_bgra_bitmap(&self.target, width, height, pixels, PixelLimits::ICON)?;
        self.icons.insert(
            key,
            CachedSpotlightIcon {
                width,
                height,
                pixels: pixels.to_vec(),
                bitmap,
            },
        );
        Ok(())
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.glass.invalidate_gpu();
        self.upload_icons()
    }

    pub fn present(&mut self, hwnd: isize, state: &SpotlightPaintState<'_, '_>) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = state.layout.window.width.clamp(1, max_width);
        let height = state.layout.window.height.clamp(1, max_height);
        draw_with_recovery(
            self,
            |renderer| renderer.draw(width, height, state),
            Self::recreate_target,
        )?;
        self.surface.present_sized(
            hwnd,
            (state.layout.window.x, state.layout.window.y),
            (width, height),
        )
    }

    pub(super) fn draw(
        &self,
        width: i32,
        height: i32,
        state: &SpotlightPaintState<'_, '_>,
    ) -> Result<()> {
        let opacity = state.content_opacity.clamp(0.0, 1.0);
        let appearance = self.glass.adaptive_appearance(&state.appearance);
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        let clear = CLEAR;
        draw_on(&self.target, &self.surface, (width, height), || unsafe {
            self.target.Clear(Some(&raw const clear));
            self.brushes.update_theme(&palette);
            self.draw_shell(
                width,
                height,
                state.layout.corner_radius,
                opacity,
                &appearance,
                &palette,
            )?;
            self.draw_search(state, opacity, &palette);
            self.draw_tabs(state.layout, state.mode, state.view, opacity, &palette);
            if opacity > 0.04 {
                if state.entries.is_empty() {
                    self.draw_empty_state(state.layout, state.mode, state.query, opacity, &palette);
                } else {
                    for item in &state.layout.results {
                        let Some(entry) = state.entries.get(item.index) else {
                            continue;
                        };
                        self.draw_result(
                            item,
                            entry,
                            state.selected == Some(item.index),
                            state.hovered == Some(item.index),
                            opacity,
                            &palette,
                        );
                    }
                }
                self.draw_footer(width, height, opacity, &palette);
            }
            self.draw_resize_grip(state.layout, opacity, &palette);
            Ok(())
        })
    }

    fn upload_icons(&mut self) -> Result<()> {
        self.surface.bind(&self.target, self.surface.size())?;
        for icon in self.icons.values_mut() {
            icon.bitmap = upload_bgra_bitmap(
                &self.target,
                icon.width,
                icon.height,
                &icon.pixels,
                PixelLimits::ICON,
            )?;
        }
        Ok(())
    }
}

pub(super) fn with_opacity(mut color: D2D1_COLOR_F, opacity: f32) -> D2D1_COLOR_F {
    color.a *= opacity.clamp(0.0, 1.0);
    color
}

pub(super) trait RectExt {
    fn to_d2d(self) -> D2D_RECT_F;
}

impl RectExt for Rect {
    fn to_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x as f32,
            top: self.y as f32,
            right: self.right() as f32,
            bottom: self.bottom() as f32,
        }
    }
}
