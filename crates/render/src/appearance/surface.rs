use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{PixelLimits, validate_bgra_buffer};
use windows::Win32::Foundation::{
    COLORREF, D2DERR_RECREATE_TARGET, E_FAIL, E_INVALIDARG, HWND, POINT, RECT, SIZE,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_PROPERTIES, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, ID2D1Bitmap,
    ID2D1DCRenderTarget, ID2D1Factory,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::core::{Error, Result};

const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

struct MemoryDc(HDC);

impl MemoryDc {
    fn create() -> Result<Self> {
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.0.is_null() {
            return Err(Error::from_hresult(E_FAIL));
        }
        Ok(Self(dc))
    }
}

impl Drop for MemoryDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

struct DibSection(HBITMAP);

impl DibSection {
    fn create(dc: &MemoryDc, width: i32, height: i32) -> Result<Self> {
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
        let dib =
            unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
        Ok(Self(dib))
    }
}

impl Drop for DibSection {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.0.0));
        }
    }
}

pub(crate) struct DibSurface {
    dc: MemoryDc,
    _dib: DibSection,
    previous: HGDIOBJ,
    size: (i32, i32),
}

impl DibSurface {
    pub(crate) fn new(width: i32, height: i32) -> Result<Self> {
        let size = (width.max(1), height.max(1));
        let dc = MemoryDc::create()?;
        let dib = DibSection::create(&dc, size.0, size.1)?;
        let previous = unsafe { SelectObject(dc.0, HGDIOBJ(dib.0.0)) };
        if previous.0.is_null() {
            return Err(Error::from_hresult(E_FAIL));
        }
        Ok(Self {
            dc,
            _dib: dib,
            previous,
            size,
        })
    }

    pub(crate) const fn size(&self) -> (i32, i32) {
        self.size
    }

    #[cfg(test)]
    #[allow(clippy::used_underscore_binding)]
    pub(crate) fn read_pixels(&self) -> Vec<u8> {
        use windows::Win32::Graphics::Gdi::{BITMAP, GetObjectW};
        let mut bitmap = BITMAP::default();
        unsafe {
            GetObjectW(
                HGDIOBJ(self._dib.0.0),
                size_of::<BITMAP>() as i32,
                Some((&raw mut bitmap).cast()),
            );
            let length = (self.size.0 * self.size.1 * 4) as usize;
            std::slice::from_raw_parts(bitmap.bmBits.cast::<u8>(), length).to_vec()
        }
    }

    pub(crate) fn bind(&self, target: &ID2D1DCRenderTarget, size: (i32, i32)) -> Result<()> {
        let (width, height) = self.clamp(size);
        unsafe {
            target.BindDC(
                self.dc.0,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )
        }
    }

    pub(crate) fn present(&self, hwnd: isize, origin: (i32, i32)) -> Result<()> {
        self.present_sized(hwnd, origin, self.size)
    }

    pub(crate) fn present_sized(
        &self,
        hwnd: isize,
        origin: (i32, i32),
        size: (i32, i32),
    ) -> Result<()> {
        let (width, height) = self.clamp(size);
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(
                HWND(hwnd as *mut c_void),
                None,
                Some(&POINT {
                    x: origin.0,
                    y: origin.1,
                }),
                Some(&SIZE {
                    cx: width,
                    cy: height,
                }),
                Some(self.dc.0),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }

    fn clamp(&self, size: (i32, i32)) -> (i32, i32) {
        (size.0.clamp(1, self.size.0), size.1.clamp(1, self.size.1))
    }
}

impl Drop for DibSurface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc.0, self.previous);
        }
    }
}

pub(crate) struct DrawSession<'a> {
    target: &'a ID2D1DCRenderTarget,
    open: bool,
}

impl<'a> DrawSession<'a> {
    pub(crate) fn begin(
        target: &'a ID2D1DCRenderTarget,
        surface: &DibSurface,
        size: (i32, i32),
    ) -> Result<Self> {
        surface.bind(target, size)?;
        unsafe {
            target.BeginDraw();
        }
        Ok(Self { target, open: true })
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.open = false;
        unsafe { self.target.EndDraw(None, None) }
    }
}

impl Drop for DrawSession<'_> {
    fn drop(&mut self) {
        if self.open {
            unsafe {
                let _ = self.target.EndDraw(None, None);
            }
        }
    }
}

pub(crate) fn draw_on<T>(
    target: &ID2D1DCRenderTarget,
    surface: &DibSurface,
    size: (i32, i32),
    paint: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let session = DrawSession::begin(target, surface, size)?;
    let painted = paint()?;
    session.finish()?;
    Ok(painted)
}

pub(crate) fn upload_bgra_bitmap(
    target: &ID2D1DCRenderTarget,
    width: u32,
    height: u32,
    pixels: &[u8],
    limits: PixelLimits,
) -> Result<ID2D1Bitmap> {
    let layout = validate_bgra_buffer(width, height, pixels.len(), limits)
        .map_err(|_| Error::from_hresult(E_INVALIDARG))?;
    let properties = D2D1_BITMAP_PROPERTIES {
        pixelFormat: PIXEL_FORMAT,
        dpiX: 96.0,
        dpiY: 96.0,
    };
    unsafe {
        target.CreateBitmap(
            D2D_SIZE_U {
                width: layout.width,
                height: layout.height,
            },
            Some(pixels.as_ptr().cast()),
            layout.stride,
            &properties,
        )
    }
}

pub(crate) fn fnv1a_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

pub(crate) fn create_dc_render_target(factory: &ID2D1Factory) -> Result<ID2D1DCRenderTarget> {
    let properties = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: PIXEL_FORMAT,
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    unsafe { factory.CreateDCRenderTarget(&properties) }
}

pub(crate) fn is_recreate_target(error: &Error) -> bool {
    error.code() == D2DERR_RECREATE_TARGET
}

pub(crate) fn draw_with_recovery<R, T>(
    renderer: &mut R,
    mut draw: impl FnMut(&mut R) -> Result<T>,
    recreate: impl FnOnce(&mut R) -> Result<()>,
) -> Result<T> {
    match draw(renderer) {
        Ok(value) => Ok(value),
        Err(error) if is_recreate_target(&error) => {
            recreate(renderer)?;
            draw(renderer)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_hash_distinguishes_equal_length_buffers() {
        let first = [1u8, 2, 3, 4];
        let second = [1u8, 2, 3, 5];
        assert_ne!(fnv1a_hash(&first), fnv1a_hash(&second));
        assert_eq!(fnv1a_hash(&first), fnv1a_hash(&[1u8, 2, 3, 4]));
    }

    #[test]
    fn is_recreate_target_matches_only_recreate_hresult() {
        assert!(is_recreate_target(&Error::from_hresult(
            D2DERR_RECREATE_TARGET
        )));
        assert!(!is_recreate_target(&Error::from_hresult(E_FAIL)));
    }

    #[test]
    fn draw_with_recovery_retries_once_on_recreate_target() {
        struct Dummy {
            attempts: usize,
            recreated: usize,
        }

        let mut dummy = Dummy {
            attempts: 0,
            recreated: 0,
        };
        let result = draw_with_recovery(
            &mut dummy,
            |state| {
                state.attempts += 1;
                if state.attempts == 1 {
                    Err(Error::from_hresult(D2DERR_RECREATE_TARGET))
                } else {
                    Ok(42)
                }
            },
            |state| {
                state.recreated += 1;
                Ok(())
            },
        )
        .expect("retry succeeds");
        assert_eq!(result, 42);
        assert_eq!(dummy.attempts, 2);
        assert_eq!(dummy.recreated, 1);
    }

    #[test]
    fn draw_with_recovery_propagates_non_recreate_errors_without_recreating() {
        let mut recreated = false;
        let error = draw_with_recovery(
            &mut recreated,
            |_| Err::<(), _>(Error::from_hresult(E_FAIL)),
            |flag| {
                *flag = true;
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), E_FAIL);
        assert!(!recreated);
    }

    #[test]
    fn draw_with_recovery_propagates_recreate_failure_without_second_draw() {
        let mut attempts = 0usize;
        let error = draw_with_recovery(
            &mut attempts,
            |count| {
                *count += 1;
                Err::<(), _>(Error::from_hresult(D2DERR_RECREATE_TARGET))
            },
            |_| Err(Error::from_hresult(E_INVALIDARG)),
        )
        .unwrap_err();
        assert_eq!(error.code(), E_INVALIDARG);
        assert_eq!(attempts, 1);
    }
}
