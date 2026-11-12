use std::mem::size_of;

use otto_core::{NativeCaption, NativeCaptionStyle};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_BUTTON_BOUNDS, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWINDOWATTRIBUTE, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetDC, GetPixel, MONITOR_DEFAULTTONEAREST, MonitorFromWindow, ReleaseDC,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongW, IsZoomed, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
};

const CORNER_DO_NOT_ROUND: u32 = 1;
const INVALID_PIXEL: u32 = 0xFFFF_FFFF;

pub(crate) fn dwm_rect(window: HWND, attribute: DWMWINDOWATTRIBUTE) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            window,
            attribute,
            std::ptr::from_mut(&mut rect).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .ok()?;
    Some(rect)
}

pub(crate) fn caption_style(style: u32) -> NativeCaptionStyle {
    NativeCaptionStyle {
        has_caption: style & WS_CAPTION.0 == WS_CAPTION.0,
        has_system_menu: style & WS_SYSMENU.0 != 0,
        can_minimize: style & WS_MINIMIZEBOX.0 != 0,
        can_maximize: style & WS_MAXIMIZEBOX.0 != 0,
    }
}

fn corner_preference(window: HWND) -> Option<u32> {
    let mut preference = 0_u32;
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_mut(&mut preference).cast(),
            size_of::<u32>() as u32,
        )
    }
    .ok()?;
    Some(preference)
}

fn has_rounded_top_right(window: HWND, maximized: bool) -> bool {
    !maximized && corner_preference(window).is_some_and(|value| value != CORNER_DO_NOT_ROUND)
}

fn caption_dpi(window: HWND) -> u32 {
    let window_dpi = unsafe { GetDpiForWindow(window) };
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    let (mut dpi_x, mut dpi_y) = (0_u32, 0_u32);
    let monitor_dpi =
        if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) }
            .is_ok()
        {
            dpi_x
        } else {
            0
        };
    match window_dpi.max(monitor_dpi) {
        0 => 96,
        dpi => dpi,
    }
}

#[must_use]
pub(crate) fn native_caption_of(window: HWND) -> Option<NativeCaption> {
    let style = caption_style(unsafe { GetWindowLongW(window, GWL_STYLE) } as u32);
    let frame = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS)?;
    let buttons = dwm_rect(window, DWMWA_CAPTION_BUTTON_BOUNDS)?;
    let height = buttons.bottom - buttons.top;
    if buttons.right <= buttons.left || height <= 0 {
        return None;
    }
    let maximized = unsafe { IsZoomed(window) }.as_bool();
    NativeCaption::new(
        style,
        frame.right,
        frame.top + buttons.top,
        height,
        has_rounded_top_right(window, maximized),
        caption_dpi(window),
    )
}

#[must_use]
pub fn native_caption(window_hwnd: isize) -> Option<NativeCaption> {
    native_caption_of(HWND(window_hwnd as *mut _))
}

#[must_use]
pub fn sample_screen_color(x: i32, y: i32) -> Option<[u8; 3]> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let pixel = GetPixel(screen, x, y).0;
        let _ = ReleaseDC(None, screen);
        rgb_from_colorref(pixel)
    }
}

fn rgb_from_colorref(pixel: u32) -> Option<[u8; 3]> {
    (pixel != INVALID_PIXEL).then_some([
        (pixel & 0xFF) as u8,
        ((pixel >> 8) & 0xFF) as u8,
        ((pixel >> 16) & 0xFF) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::{caption_style, rgb_from_colorref};
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
    };

    #[test]
    fn a_standard_overlapped_window_style_has_every_caption_button() {
        let style =
            caption_style(WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0);
        assert!(style.has_caption && style.has_system_menu);
        assert!(style.can_minimize && style.can_maximize);
    }

    #[test]
    fn a_border_without_the_full_caption_is_not_a_caption() {
        let style = caption_style(0x0080_0000 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);
        assert!(!style.has_caption);
    }

    #[test]
    fn a_dialog_style_has_no_minimize_or_maximize_box() {
        let style = caption_style(WS_CAPTION.0 | WS_SYSMENU.0);
        assert!(style.has_caption && style.has_system_menu);
        assert!(!style.can_minimize && !style.can_maximize);
    }

    #[test]
    fn colours_come_back_in_red_green_blue_order() {
        assert_eq!(rgb_from_colorref(0x0030_2010), Some([0x10, 0x20, 0x30]));
    }

    #[test]
    fn a_failed_pixel_read_is_not_a_colour() {
        assert_eq!(rgb_from_colorref(0xFFFF_FFFF), None);
    }
}
