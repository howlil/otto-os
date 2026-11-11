use otto_core::{GenieGeometry, GenieStrip, PixelLimits, Rect as CoreRect};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    ID2D1Bitmap, ID2D1DCRenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;

use crate::shared_d2d_factory;
use crate::surface::{DibSurface, draw_on, upload_bgra_bitmap};

const CLEAR: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F =
    windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

pub struct GenieRenderer {
    target: ID2D1DCRenderTarget,
    surface: DibSurface,
    canvas: CoreRect,
    bitmap: Option<ID2D1Bitmap>,
    bitmap_size: Option<(u32, u32)>,
}

impl GenieRenderer {
    pub fn new(canvas: CoreRect) -> Result<Self> {
        let surface = DibSurface::new(canvas.width, canvas.height)?;
        let factory = shared_d2d_factory()?;
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: PIXEL_FORMAT,
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = unsafe { factory.CreateDCRenderTarget(&properties)? };
        Ok(Self {
            target,
            surface,
            canvas,
            bitmap: None,
            bitmap_size: None,
        })
    }

    pub fn set_snapshot(&mut self, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
        self.clear_snapshot();
        self.surface.bind(&self.target, self.surface.size())?;
        let bitmap = upload_bgra_bitmap(
            &self.target,
            width,
            height,
            pixels,
            PixelLimits::WINDOW_SNAPSHOT,
        )?;
        self.bitmap = Some(bitmap);
        self.bitmap_size = Some((width, height));
        Ok(())
    }

    pub fn clear_snapshot(&mut self) {
        self.bitmap = None;
        self.bitmap_size = None;
    }

    pub fn present(&self, hwnd: isize, geometry: &GenieGeometry) -> Result<()> {
        let Some(bounds) = self.presentation_bounds(geometry) else {
            return Ok(());
        };
        self.draw(geometry, bounds)?;
        self.surface
            .present_sized(hwnd, (bounds.x, bounds.y), (bounds.width, bounds.height))
    }

    #[must_use]
    pub const fn canvas(&self) -> CoreRect {
        self.canvas
    }

    fn presentation_bounds(&self, geometry: &GenieGeometry) -> Option<CoreRect> {
        geometry.destination_bounds()?.intersection(&self.canvas)
    }

    fn draw(&self, geometry: &GenieGeometry, bounds: CoreRect) -> Result<()> {
        draw_on(
            &self.target,
            &self.surface,
            (bounds.width, bounds.height),
            || {
                let clear = CLEAR;
                unsafe {
                    self.target.Clear(Some(&raw const clear));
                }
                if let (Some(bitmap), Some((bitmap_width, bitmap_height))) =
                    (&self.bitmap, self.bitmap_size)
                {
                    for strip in &geometry.strips {
                        self.draw_strip(bitmap, bitmap_width, bitmap_height, strip, bounds);
                    }
                }
                Ok(())
            },
        )
    }

    fn draw_strip(
        &self,
        bitmap: &ID2D1Bitmap,
        bitmap_width: u32,
        bitmap_height: u32,
        strip: &GenieStrip,
        bounds: CoreRect,
    ) {
        let source = D2D_RECT_F {
            left: 0.0,
            top: strip.source_top.clamp(0.0, bitmap_height as f32),
            right: bitmap_width as f32,
            bottom: (strip.source_top + strip.source_height).clamp(0.0, bitmap_height as f32),
        };
        let destination = D2D_RECT_F {
            left: strip.destination_left - bounds.x as f32,
            top: (strip.destination_top - bounds.y as f32).floor(),
            right: strip.destination_left + strip.destination_width - bounds.x as f32,
            bottom: (strip.destination_top + strip.destination_height - bounds.y as f32).ceil(),
        };
        unsafe {
            self.target.DrawBitmap(
                bitmap,
                Some(&raw const destination),
                strip.opacity,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                Some(&raw const source),
            );
        }
    }
}
