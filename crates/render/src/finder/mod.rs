mod chrome;
mod icons;
mod overlays;
mod palette;
mod views;

use std::collections::HashMap;

use otto_core::PixelLimits;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::core::Result;

use crate::kit::canvas::HwndCanvas;

use self::chrome::{draw_banner, draw_bottom_bars, draw_sidebar, draw_tabs, draw_toolbar};
use self::overlays::{draw_go_to_overlay, draw_inspector, draw_quick_look};
pub use self::palette::FinderPaintState;
use self::palette::{FinderFormats, FinderPalette};
use self::views::draw_content_views;

const MAX_FINDER_THUMBS: usize = 160;

pub struct FinderRenderer {
    canvas: HwndCanvas,
    formats: FinderFormats,
    dpi: u32,
    thumbs: HashMap<String, ID2D1Bitmap>,
    preview_key: String,
    preview_bitmap: Option<ID2D1Bitmap>,
}

impl FinderRenderer {
    pub fn new(hwnd: isize, width: i32, height: i32, dpi: u32) -> Result<Self> {
        Ok(Self {
            canvas: HwndCanvas::new(hwnd, width, height)?,
            formats: FinderFormats::new(dpi)?,
            dpi,
            thumbs: HashMap::new(),
            preview_key: String::new(),
            preview_bitmap: None,
        })
    }

    pub fn resize(&self, width: i32, height: i32) -> Result<()> {
        self.canvas.resize(width, height)
    }

    pub fn set_dpi(&mut self, dpi: u32) -> Result<()> {
        if dpi != self.dpi {
            self.formats = FinderFormats::new(dpi)?;
            self.dpi = dpi;
        }
        Ok(())
    }

    #[must_use]
    pub fn has_thumbnail(&self, key: &str) -> bool {
        self.thumbs.contains_key(&key.to_ascii_lowercase())
    }

    pub fn set_thumbnail(
        &mut self,
        key: &str,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<()> {
        let norm = key.to_ascii_lowercase();
        if self.thumbs.len() >= MAX_FINDER_THUMBS && !self.thumbs.contains_key(&norm) {
            self.thumbs.clear();
        }
        let bmp = self
            .canvas
            .upload_bitmap(width, height, pixels, PixelLimits::ICON)?;
        self.thumbs.insert(norm, bmp);
        Ok(())
    }

    pub fn clear_thumbnails(&mut self) {
        self.thumbs.clear();
        self.preview_key.clear();
        self.preview_bitmap = None;
    }

    pub fn draw(&mut self, state: &FinderPaintState<'_>) -> Result<()> {
        let preview_path = state.state.preview.path.to_ascii_lowercase();
        if self.preview_key != preview_path {
            self.preview_key = preview_path;
            self.preview_bitmap = state.state.preview.bitmap.as_ref().and_then(|bmp| {
                self.canvas
                    .upload_bitmap(bmp.width, bmp.height, &bmp.pixels, PixelLimits::ICON)
                    .ok()
            });
        }
        let palette = FinderPalette::pick(state);
        self.canvas.begin_frame(palette.background);
        draw_content_views(
            &self.canvas,
            &self.formats,
            &self.thumbs,
            self.preview_bitmap.as_ref(),
            state,
            &palette,
        );
        draw_inspector(
            &self.canvas,
            &self.formats,
            self.preview_bitmap.as_ref(),
            state,
            &palette,
        );
        draw_banner(&self.canvas, &self.formats, state, &palette);
        draw_tabs(&self.canvas, &self.formats, state, &palette);
        draw_bottom_bars(&self.canvas, &self.formats, state, &palette);
        draw_sidebar(&self.canvas, &self.formats, state, &palette);
        draw_toolbar(&self.canvas, &self.formats, state, &palette);
        draw_quick_look(
            &self.canvas,
            &self.formats,
            self.preview_bitmap.as_ref(),
            state,
            &palette,
        );
        draw_go_to_overlay(&self.canvas, &self.formats, state, &palette);
        self.canvas.end_frame()
    }
}
