use std::mem::size_of;

use otto_core::{Rect, SettingsMetrics, scale, settings_background, settings_text};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWINDOWATTRIBUTE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{ClientToScreen, InvalidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DestroyWindow, GetClientRect, GetForegroundWindow,
    IDC_ARROW, IDI_APPLICATION, IsIconic, IsWindow, IsWindowVisible, LoadCursorW, LoadIconW,
    MINMAXINFO, RegisterClassExW, SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    WINDOW_EX_STYLE, WNDCLASSEXW, WNDPROC, WS_OVERLAPPEDWINDOW,
};
use windows::core::{PCWSTR, Result, w};

use crate::blur::{BackdropMaterial, apply_backdrop_material};
use crate::screen::{monitor_work_area_at, primary_screen};

const SETTINGS_CLASS: PCWSTR = w!("OttoSettings");
const SETTINGS_TITLE: PCWSTR = w!("Otto Settings");
const EMBEDDED_APP_ICON_ID: usize = 1;
const DWMWA_USE_IMMERSIVE_DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(20);
const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
const DWMWA_CAPTION_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(35);
const DWMWA_TEXT_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(36);

const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
const DWMWA_COLOR_DEFAULT: u32 = 0xFFFF_FFFF;

const fn colorref(rgb: u32) -> u32 {
    ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF)
}

fn handle(hwnd: isize) -> HWND {
    HWND(hwnd as *mut _)
}

pub fn create_settings_window(wnd_proc: WNDPROC) -> Result<isize> {
    let metrics = SettingsMetrics::DEFAULT;
    let screen = primary_screen();
    let area = monitor_work_area_at(0, 0);
    let width = scale(metrics.default_width, screen.dpi).min(area.width);
    let height = scale(metrics.default_height, screen.dpi).min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let icon = LoadIconW(
            Some(instance.into()),
            PCWSTR(EMBEDDED_APP_ICON_ID as *const u16),
        )
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: wnd_proc,
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: icon,
            hIconSm: icon,
            lpszClassName: SETTINGS_CLASS,
            ..WNDCLASSEXW::default()
        };
        let _ = RegisterClassExW(&class);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            SETTINGS_CLASS,
            SETTINGS_TITLE,
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        Ok(hwnd.0 as isize)
    }
}

#[must_use]
pub fn settings_glass_available() -> bool {
    !crate::theme::is_high_contrast_active()
}

pub fn apply_settings_chrome(hwnd: isize, dark: bool, glass: bool) {
    let dark_mode = i32::from(dark);
    let solid = colorref(settings_background(dark));
    let text = colorref(settings_text(dark));
    let glass = glass && apply_backdrop_material(hwnd, BackdropMaterial::Acrylic);
    if !glass {
        let _ = apply_backdrop_material(hwnd, BackdropMaterial::None);
    }
    let margin = if glass { -1 } else { 0 };
    let margins = MARGINS {
        cxLeftWidth: margin,
        cxRightWidth: margin,
        cyTopHeight: margin,
        cyBottomHeight: margin,
    };
    let (caption, border) = if glass {
        (DWMWA_COLOR_NONE, DWMWA_COLOR_DEFAULT)
    } else {
        (solid, solid)
    };
    unsafe {
        let window = handle(hwnd);
        let _ = DwmExtendFrameIntoClientArea(window, &margins);
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const dark_mode).cast(),
            size_of::<i32>() as u32,
        );
        for (attribute, color) in [
            (DWMWA_CAPTION_COLOR, caption),
            (DWMWA_BORDER_COLOR, border),
            (DWMWA_TEXT_COLOR, text),
        ] {
            let _ = DwmSetWindowAttribute(
                window,
                attribute,
                (&raw const color).cast(),
                size_of::<u32>() as u32,
            );
        }
    }
}

pub fn present_settings_window(hwnd: isize) {
    let window = handle(hwnd);
    unsafe {
        let command = if IsIconic(window).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(window, command);
        let _ = SetForegroundWindow(window);
    }
}

pub fn hide_settings_window(hwnd: isize) {
    unsafe {
        let _ = ShowWindow(handle(hwnd), SW_HIDE);
    }
}

#[must_use]
pub fn is_settings_window_visible(hwnd: isize) -> bool {
    unsafe { IsWindow(Some(handle(hwnd))).as_bool() && IsWindowVisible(handle(hwnd)).as_bool() }
}

#[must_use]
pub fn is_settings_window_foreground(hwnd: isize) -> bool {
    unsafe { GetForegroundWindow() == handle(hwnd) }
}

#[must_use]
pub fn settings_window_dpi(hwnd: isize) -> u32 {
    let dpi = unsafe { GetDpiForWindow(handle(hwnd)) };
    if dpi == 0 { 96 } else { dpi }
}

#[must_use]
pub fn settings_client_size(hwnd: isize) -> (i32, i32) {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(handle(hwnd), &raw mut rect);
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

#[must_use]
pub fn settings_client_screen_rect(hwnd: isize) -> Option<Rect> {
    if hwnd == 0 {
        return None;
    }
    let window = handle(hwnd);
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    unsafe {
        GetClientRect(window, &raw mut rect).ok()?;
        if !ClientToScreen(window, &raw mut origin).as_bool() {
            return None;
        }
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    (width > 0 && height > 0).then_some(Rect {
        x: origin.x,
        y: origin.y,
        width,
        height,
    })
}

pub fn apply_settings_min_track(hwnd: isize, lparam: LPARAM) {
    let info = lparam.0 as *mut MINMAXINFO;
    if info.is_null() {
        return;
    }
    let metrics = SettingsMetrics::DEFAULT;
    let dpi = settings_window_dpi(hwnd);
    unsafe {
        (*info).ptMinTrackSize.x = scale(metrics.min_width, dpi);
        (*info).ptMinTrackSize.y = scale(metrics.min_height, dpi);
    }
}

#[must_use]
pub fn suggested_dpi_rect(lparam: LPARAM) -> Option<Rect> {
    let rect = lparam.0 as *const RECT;
    if rect.is_null() {
        return None;
    }
    let rect = unsafe { *rect };
    Some(Rect {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    })
}

pub fn invalidate_settings_window(hwnd: isize) {
    unsafe {
        let _ = InvalidateRect(Some(handle(hwnd)), None, false);
    }
}

pub fn destroy_settings_window(hwnd: isize) {
    unsafe {
        let _ = DestroyWindow(handle(hwnd));
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;

    use super::{
        create_settings_window, destroy_settings_window, hide_settings_window,
        is_settings_window_visible, present_settings_window, settings_client_size,
    };

    unsafe extern "system" fn proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    #[test]
    fn the_window_is_created_hidden_and_shows_and_hides() {
        let hwnd = create_settings_window(Some(proc)).expect("settings window");
        assert!(!is_settings_window_visible(hwnd));
        present_settings_window(hwnd);
        assert!(is_settings_window_visible(hwnd));
        let (width, height) = settings_client_size(hwnd);
        assert!(width >= 700 && height >= 400, "{width}x{height}");
        hide_settings_window(hwnd);
        assert!(!is_settings_window_visible(hwnd));
        destroy_settings_window(hwnd);
        assert!(!is_settings_window_visible(hwnd));
    }
}
