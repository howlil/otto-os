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
        if !layout.is_consistent() {
            return None;
        }
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

    pub(crate) fn as_bytes(&self) -> &[u8] {
        unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts(self.dib.bits.cast::<u8>(), self.layout.byte_len)
        }
    }

    #[cfg(test)]
    pub(crate) fn copy_pixels(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl Drop for CaptureCanvas {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc.0, self.previous);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otto_core::{PixelLimits, bgra_layout};
    use windows::Win32::System::Threading::{GR_GDIOBJECTS, GetCurrentProcess, GetGuiResources};

    const CYCLES: usize = 200;

    fn gdi_object_count() -> u32 {
        unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) }
    }

    fn acquire_screen() -> ScreenDc {
        ScreenDc::acquire().expect("the desktop screen DC should be available")
    }

    fn capture_once(screen: &ScreenDc, layout: BgraLayout) {
        let canvas = CaptureCanvas::create(screen, layout).expect("canvas should be created");
        assert_eq!(canvas.copy_pixels().len(), layout.byte_len);
    }

    #[test]
    fn repeated_canvas_cycles_do_not_grow_the_gdi_object_count() {
        let layout = bgra_layout(64, 32, PixelLimits::ICON).unwrap();
        let screen = acquire_screen();
        capture_once(&screen, layout);
        let before = gdi_object_count();
        for _ in 0..CYCLES {
            capture_once(&screen, layout);
        }
        let growth = gdi_object_count().saturating_sub(before);
        assert!(
            growth < CYCLES as u32 / 2,
            "GDI object count grew by {growth} over {CYCLES} canvas cycles"
        );
    }

    #[test]
    fn failed_dib_allocation_releases_the_memory_dc_it_was_given() {
        let oversized = bgra_layout(
            1 << 20,
            1 << 20,
            PixelLimits {
                max_dimension: 1 << 20,
                max_bytes: usize::MAX,
            },
        )
        .expect("the layout itself is well formed on 64-bit targets");
        let screen = acquire_screen();
        let before = gdi_object_count();
        for _ in 0..CYCLES / 4 {
            assert!(CaptureCanvas::create(&screen, oversized).is_none());
        }
        let growth = gdi_object_count().saturating_sub(before);
        assert!(
            growth < CYCLES as u32 / 2,
            "GDI object count grew by {growth} after failed canvas allocations"
        );
    }

    #[test]
    fn canvas_refuses_a_layout_that_contradicts_its_own_size() {
        let screen = acquire_screen();
        let lying = BgraLayout {
            width: 4,
            height: 4,
            stride: 64,
            byte_len: 256,
        };
        assert!(CaptureCanvas::create(&screen, lying).is_none());
    }
}
