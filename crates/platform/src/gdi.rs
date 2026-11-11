use std::ffi::c_void;
use std::mem::size_of;

use otto_core::BgraLayout;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GdiFlush, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SelectObject,
};

pub(crate) struct ScreenDc(HDC);

impl ScreenDc {
    pub(crate) fn acquire() -> Option<Self> {
        let dc = unsafe { GetDC(None) };
        (!dc.0.is_null()).then_some(Self(dc))
    }

    pub(crate) const fn handle(&self) -> HDC {
        self.0
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        unsafe {
            ReleaseDC(None, self.0);
        }
    }
}

struct MemoryDc(HDC);

impl MemoryDc {
    fn compatible_with(screen: &ScreenDc) -> Option<Self> {
        let dc = unsafe { CreateCompatibleDC(Some(screen.handle())) };
        (!dc.0.is_null()).then_some(Self(dc))
    }
}

impl Drop for MemoryDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

struct DibSection {
    bitmap: HBITMAP,
    bits: *mut c_void,
}

impl DibSection {
    fn create(screen: &ScreenDc, layout: BgraLayout) -> Option<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: i32::try_from(layout.width).ok()?,
                biHeight: -i32::try_from(layout.height).ok()?,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(
                Some(screen.handle()),
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )
        }
        .ok()?;
        if bits.is_null() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            return None;
        }
        Some(Self { bitmap, bits })
    }
}

impl Drop for DibSection {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
        }
    }
}

pub(crate) struct CaptureCanvas {
    dc: MemoryDc,
    dib: DibSection,
    previous: HGDIOBJ,
    layout: BgraLayout,
}

impl CaptureCanvas {
    pub(crate) fn create(screen: &ScreenDc, layout: BgraLayout) -> Option<Self> {
        let dc = MemoryDc::compatible_with(screen)?;
        let dib = DibSection::create(screen, layout)?;
        let previous = unsafe { SelectObject(dc.0, HGDIOBJ(dib.bitmap.0)) };
        if previous.0.is_null() {
            return None;
        }
        Some(Self {
            dc,
            dib,
            previous,
            layout,
        })
    }

    pub(crate) const fn dc(&self) -> HDC {
        self.dc.0
    }

    pub(crate) const fn layout(&self) -> BgraLayout {
        self.layout
    }

    pub(crate) fn copy_pixels(&self) -> Vec<u8> {
        unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts(self.dib.bits.cast::<u8>(), self.layout.byte_len).to_vec()
        }
    }
}

impl Drop for CaptureCanvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc.0, self.previous);
        }
    }
}
