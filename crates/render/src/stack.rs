use otto_core::{
    Appearance, DownloadsStackEntry, DownloadsStackHit, DownloadsStackLayout, PixelLimits,
    ShelfThumbnail, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_LINE_JOIN_ROUND,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, ID2D1Bitmap, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery, upload_bgra_bitmap,
};
use crate::theme::{BrushPalette, LiquidGlassPalette};
use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

const FOLDER_BLUE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.25,
    g: 0.68,
    b: 0.96,
    a: 1.0,
};

const FOLDER_BACK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.16,
    g: 0.54,
    b: 0.86,
    a: 1.0,
};

const DOC_PAGE: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.92,
    g: 0.94,
    b: 0.98,
    a: 0.96,
};

const DOC_CORNER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.72,
    g: 0.78,
    b: 0.88,
    a: 0.95,
};

pub struct DownloadsStackRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    brushes: BrushPalette,
    stroke_style: ID2D1StrokeStyle,
    header_format: IDWriteTextFormat,
    label_format: IDWriteTextFormat,
    footer_format: IDWriteTextFormat,
    surface: DibSurface,
    dpi: u32,
    raw_thumbnails: Vec<Option<ShelfThumbnail>>,
    bitmaps: Vec<Option<ID2D1Bitmap>>,
}

impl DownloadsStackRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        let max_w = scale(420, dpi);
        let max_h = scale(420, dpi);
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = create_dc_render_target(&factory)?;
            let brushes = BrushPalette::new(&target)?;
            let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 2.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let stroke_style = factory.CreateStrokeStyle(&stroke_props, None)?;
            let dwrite = shared_dwrite_factory()?;
            let header_format = create_text_format(
                &dwrite,
                scale(13, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let label_format = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let footer_format = create_text_format(
                &dwrite,
                scale(12, dpi) as f32,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let surface = DibSurface::new(max_w, max_h)?;
            Ok(Self {
                factory,
                target,
                brushes,
                stroke_style,
                header_format,
                label_format,
                footer_format,
                surface,
                dpi,
                raw_thumbnails: Vec::new(),
                bitmaps: Vec::new(),
            })
        }
    }

    pub fn set_thumbnails(&mut self, thumbs: &[Option<ShelfThumbnail>]) -> Result<()> {
        self.bitmaps.clear();
        self.raw_thumbnails = thumbs.to_vec();
        self.upload_all_thumbnails()
    }

    pub fn clear_thumbnails(&mut self) {
        self.bitmaps.clear();
        self.raw_thumbnails.clear();
    }

    fn upload_all_thumbnails(&mut self) -> Result<()> {
        self.bitmaps.clear();
        self.surface.bind(&self.target, self.surface.size())?;
        for entry in &self.raw_thumbnails {
            let uploaded = match entry {
                Some(thumb) => upload_bgra_bitmap(
                    &self.target,
                    thumb.width,
                    thumb.height,
                    &thumb.bgra,
                    PixelLimits::ICON,
                )
                .ok(),
                None => None,
            };
            self.bitmaps.push(uploaded);
        }
        Ok(())
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.upload_all_thumbnails()
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        layout: &DownloadsStackLayout,
        items: &[DownloadsStackEntry],
        active_hit: DownloadsStackHit,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        draw_with_recovery(
            self,
            |renderer| renderer.draw(width, height, layout, items, active_hit, appearance),
            Self::recreate_target,
        )?;
        self.surface
            .present_sized(hwnd, (layout.window.x, layout.window.y), (width, height))
    }

    #[cfg(test)]
    pub(crate) fn render_preview(
        &self,
        layout: &DownloadsStackLayout,
        items: &[DownloadsStackEntry],
        active_hit: DownloadsStackHit,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        self.draw(width, height, layout, items, active_hit, appearance)
    }

    fn draw(
        &self,
        width: i32,
        height: i32,
        layout: &DownloadsStackLayout,
        items: &[DownloadsStackEntry],
        active_hit: DownloadsStackHit,
        appearance: Appearance,
    ) -> Result<()> {
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        draw_on(&self.target, &self.surface, (width, height), || unsafe {
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));
            self.brushes.update_theme(&palette);

            self.draw_shell(width as f32, height as f32, layout.corner_radius, &palette);
            self.draw_text(
                &self.header_format,
                "Downloads",
                &layout.header_rect.to_d2d(),
                &self.brushes.text_primary,
            );

            if let Some(empty_rect) = layout.empty_rect {
                self.draw_text(
                    &self.footer_format,
                    "Downloads is Empty",
                    &empty_rect.to_d2d(),
                    &self.brushes.text_muted,
                );
            } else {
                for tile in &layout.tiles {
                    let Some(entry) = items.get(tile.index) else {
                        continue;
                    };
                    let selected = active_hit == DownloadsStackHit::Item(tile.index);
                    let bmp = self.bitmaps.get(tile.index).and_then(|item| item.as_ref());
                    self.draw_tile(tile, entry, bmp, layout.tile_radius, selected, &palette)?;
                }
            }

            let sep_y = (layout.open_folder_rect.y - scale(4, self.dpi)) as f32 + 0.5;
            let sep_brush = &self.brushes.color_brush(palette.menu_separator);
            self.target.DrawLine(
                Vector2::new(layout.open_folder_rect.x as f32, sep_y),
                Vector2::new(layout.open_folder_rect.right() as f32, sep_y),
                sep_brush,
                1.0,
                None,
            );

            let footer_hovered = active_hit == DownloadsStackHit::OpenFolder;
            let footer_fill = if footer_hovered {
                self.brushes.color_brush(palette.menu_hover_fill)
            } else {
                self.brushes.card_fill.clone()
            };
            let footer_rr = D2D1_ROUNDED_RECT {
                rect: layout.open_folder_rect.to_d2d(),
                radiusX: layout.tile_radius,
                radiusY: layout.tile_radius,
            };
            self.target.FillRoundedRectangle(&footer_rr, &footer_fill);
            let footer_fg = if footer_hovered {
                &self.brushes.on_accent
            } else {
                &self.brushes.text_primary
            };
            self.draw_text(
                &self.footer_format,
                "Open in Explorer",
                &layout.open_folder_rect.to_d2d(),
                footer_fg,
            );
            Ok(())
        })
    }

    fn draw_shell(&self, width: f32, height: f32, radius: f32, palette: &LiquidGlassPalette) {
        let outer = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 0.5,
                top: 0.5,
                right: width - 0.5,
                bottom: height - 0.5,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let inner = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 1.5,
                top: 1.5,
                right: width - 1.5,
                bottom: height - 1.5,
            },
            radiusX: (radius - 1.0).max(1.0),
            radiusY: (radius - 1.0).max(1.0),
        };
        unsafe {
            let fill = &self.brushes.color_brush(palette.menu_fill);
            let border = &self.brushes.color_brush(palette.menu_outer_border);
            self.target.FillRoundedRectangle(&outer, fill);
            self.target.DrawRoundedRectangle(&outer, border, 1.0, None);
            self.target
                .DrawRoundedRectangle(&inner, &self.brushes.specular_rim, 1.0, None);
        }
    }

    fn draw_tile(
        &self,
        tile: &otto_core::DownloadsStackTile,
        entry: &DownloadsStackEntry,
        bitmap: Option<&ID2D1Bitmap>,
        radius: f32,
        selected: bool,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let tile_rect = tile.rect.to_d2d();
        let rr = D2D1_ROUNDED_RECT {
            rect: tile_rect,
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            if selected {
                let hover_brush = &self.brushes.color_brush(palette.menu_hover_fill);
                self.target.FillRoundedRectangle(&rr, hover_brush);
            } else {
                self.target
                    .FillRoundedRectangle(&rr, &self.brushes.card_fill);
            }
            self.target
                .DrawRoundedRectangle(&rr, &self.brushes.specular_rim, 1.0, None);

            let thumb = tile.thumb_rect.to_d2d();
            if let Some(bmp) = bitmap {
                self.target.DrawBitmap(
                    bmp,
                    Some(&raw const thumb),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            } else if tile.is_dir {
                self.draw_folder_glyph(thumb);
            } else {
                self.draw_document_glyph(thumb)?;
            }
        }
        let fg = if selected {
            &self.brushes.on_accent
        } else {
            &self.brushes.text_primary
        };
        self.draw_text(
            &self.label_format,
            &entry.name,
            &tile.label_rect.to_d2d(),
            fg,
        );
        Ok(())
    }

    fn draw_folder_glyph(&self, rect: D2D_RECT_F) {
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        let back = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left + w * 0.08,
                top: rect.top + h * 0.16,
                right: rect.left + w * 0.48,
                bottom: rect.top + h * 0.44,
            },
            radiusX: w * 0.08,
            radiusY: w * 0.08,
        };
        let body = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: rect.left + w * 0.08,
                top: rect.top + h * 0.28,
                right: rect.right - w * 0.08,
                bottom: rect.bottom - h * 0.12,
            },
            radiusX: w * 0.12,
            radiusY: w * 0.12,
        };
        unsafe {
            let back_brush = &self.brushes.color_brush(FOLDER_BACK);
            let front_brush = &self.brushes.color_brush(FOLDER_BLUE);
            self.target.FillRoundedRectangle(&back, back_brush);
            self.target.FillRoundedRectangle(&body, front_brush);
        }
    }

    fn draw_document_glyph(&self, rect: D2D_RECT_F) -> Result<()> {
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        let left = rect.left + w * 0.18;
        let right = rect.right - w * 0.18;
        let top = rect.top + h * 0.08;
        let bottom = rect.bottom - h * 0.08;
        let fold = (right - left) * 0.30;
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.BeginFigure(Vector2::new(left, top), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(right - fold, top));
            sink.AddLine(Vector2::new(right, top + fold));
            sink.AddLine(Vector2::new(right, bottom));
            sink.AddLine(Vector2::new(left, bottom));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            let page_brush = &self.brushes.color_brush(DOC_PAGE);
            let corner_brush = &self.brushes.color_brush(DOC_CORNER);
            self.target.FillGeometry(&path, page_brush, None);
            let fold_path = self.factory.CreatePathGeometry()?;
            let fold_sink = fold_path.Open()?;
            fold_sink.BeginFigure(Vector2::new(right - fold, top), D2D1_FIGURE_BEGIN_FILLED);
            fold_sink.AddLine(Vector2::new(right, top + fold));
            fold_sink.AddLine(Vector2::new(right - fold, top + fold));
            fold_sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            fold_sink.Close()?;
            self.target.FillGeometry(&fold_path, corner_brush, None);
            for line_y in [0.46_f32, 0.62, 0.78] {
                let y = top + (bottom - top) * line_y;
                self.target.DrawLine(
                    Vector2::new(left + w * 0.10, y),
                    Vector2::new(right - w * 0.10, y),
                    corner_brush,
                    1.2,
                    &self.stroke_style,
                );
            }
        }
        Ok(())
    }

    fn draw_text(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        rect: &D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        if text.is_empty() {
            return;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

trait RectExt {
    fn to_d2d(self) -> D2D_RECT_F;
}

impl RectExt for otto_core::Rect {
    fn to_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x as f32,
            top: self.y as f32,
            right: self.right() as f32,
            bottom: self.bottom() as f32,
        }
    }
}
