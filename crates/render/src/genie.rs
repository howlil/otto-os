use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{GenieGeometry, GenieStrip, Rect as CoreRect};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    ID2D1Bitmap, ID2D1DCRenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::core::Result;

use crate::shared_d2d_factory;

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
    dc: HDC,
    dib: HBITMAP,
    previous: HGDIOBJ,
    size: (i32, i32),
    bitmap: Option<ID2D1Bitmap>,
    bitmap_size: Option<(u32, u32)>,
}

impl GenieRenderer {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: PIXEL_FORMAT,
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let target = factory.CreateDCRenderTarget(&properties)?;
            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..BITMAPINFOHEADER::default()
                },
                ..BITMAPINFO::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let previous = SelectObject(dc, HGDIOBJ(dib.0));
            Ok(Self {
                target,
                dc,
                dib,
                previous,
                size: (width, height),
                bitmap: None,
                bitmap_size: None,
            })
        }
    }

    pub fn set_snapshot(&mut self, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: PIXEL_FORMAT,
            dpiX: 96.0,
            dpiY: 96.0,
        };
        let bitmap = unsafe {
            self.target.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(pixels.as_ptr().cast()),
                width.saturating_mul(4),
                &properties,
            )?
        };
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
        let blend = BLENDFUNCTION {
            BlendOp: windows::Win32::Graphics::Gdi::AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: windows::Win32::Graphics::Gdi::AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(
                HWND(hwnd as *mut c_void),
                None,
                Some(&POINT {
                    x: bounds.x,
                    y: bounds.y,
                }),
                Some(&SIZE {
                    cx: bounds.width,
                    cy: bounds.height,
                }),
                Some(self.dc),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }

    fn presentation_bounds(&self, geometry: &GenieGeometry) -> Option<CoreRect> {
        let bounds = geometry.destination_bounds()?;
        let left = bounds.x.clamp(0, self.size.0);
        let top = bounds.y.clamp(0, self.size.1);
        let right = bounds.right().clamp(left, self.size.0);
        let bottom = bounds.bottom().clamp(top, self.size.1);
        (right > left && bottom > top).then_some(CoreRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    fn draw(&self, geometry: &GenieGeometry, bounds: CoreRect) -> Result<()> {
        unsafe {
            self.target.BindDC(
                self.dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: bounds.width,
                    bottom: bounds.height,
                },
            )?;
            self.target.BeginDraw();
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));
            if let (Some(bitmap), Some((bitmap_width, bitmap_height))) =
                (&self.bitmap, self.bitmap_size)
            {
                for strip in &geometry.strips {
                    self.draw_strip(bitmap, bitmap_width, bitmap_height, strip, bounds);
                }
            }
            self.target.EndDraw(None, None)
        }
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
            top: strip.destination_top - bounds.y as f32,
            right: strip.destination_left + strip.destination_width - bounds.x as f32,
            bottom: strip.destination_top + strip.destination_height - bounds.y as f32,
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

impl Drop for GenieRenderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.dib.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
