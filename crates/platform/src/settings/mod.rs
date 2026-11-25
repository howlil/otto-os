use std::mem::size_of;

use otto_core::{FinderMetrics, Rect, SettingsMetrics, scale, settings_background, settings_text};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWINDOWATTRIBUTE, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect,
    GetForegroundWindow, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT,
    HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, IDI_APPLICATION, IsIconic, IsWindow,
    IsWindowVisible, IsZoomed, LoadCursorW, LoadIconW, MINMAXINFO, NCCALCSIZE_PARAMS,
    RegisterClassExW, SW_HIDE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos,
    ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_NCCALCSIZE, WM_NCHITTEST, WNDCLASSEXW, WNDPROC,
    WS_OVERLAPPEDWINDOW, WS_SYSMENU,
};
use windows::core::{PCWSTR, Result, w};

use crate::blur::{BackdropMaterial, apply_backdrop_material};
use crate::screen::{monitor_work_area_at, primary_screen};

const SETTINGS_CLASS: PCWSTR = w!("OttoSettings");
const SETTINGS_TITLE: PCWSTR = w!("Otto Settings");
const FINDER_CLASS: PCWSTR = w!("OttoFinder");
const FINDER_TITLE: PCWSTR = w!("Finder");
const EMBEDDED_APP_ICON_ID: usize = 1;
const DWMWA_USE_IMMERSIVE_DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(20);
const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
const DWMWA_CAPTION_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(35);
const DWMWA_TEXT_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(36);
const DWMWCP_ROUND: u32 = 2;
const MICA_CUSTOM_FRAME_STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0 & !WS_SYSMENU.0);

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
    create_mica_window(
        SETTINGS_CLASS,
        SETTINGS_TITLE,
        metrics.default_width,
        metrics.default_height,
        wnd_proc,
    )
}

pub fn create_finder_window(wnd_proc: WNDPROC) -> Result<isize> {
    let metrics = FinderMetrics::DEFAULT;
    create_mica_window(
        FINDER_CLASS,
        FINDER_TITLE,
        metrics.default_width,
        metrics.default_height,
        wnd_proc,
    )
}

fn create_mica_window(
    class_name: PCWSTR,
    title: PCWSTR,
    default_width: i32,
    default_height: i32,
    wnd_proc: WNDPROC,
) -> Result<isize> {
    let screen = primary_screen();
    let area = monitor_work_area_at(0, 0);
    let width = scale(default_width, screen.dpi).min(area.width);
    let height = scale(default_height, screen.dpi).min(area.height);
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
            lpszClassName: class_name,
            ..WNDCLASSEXW::default()
        };
        let _ = RegisterClassExW(&class);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            title,
            MICA_CUSTOM_FRAME_STYLE,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let round = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const round).cast(),
            size_of::<u32>() as u32,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        Ok(hwnd.0 as isize)
    }
}

#[must_use]
pub fn settings_glass_available() -> bool {
    !crate::theme::is_high_contrast_active()
        && crate::theme::is_transparency_enabled()
        && !crate::theme::is_battery_saver_active()
}

#[must_use]
pub fn apply_settings_chrome(hwnd: isize, dark: bool, glass: bool) -> bool {
    let dark_mode = i32::from(dark);
    let solid = colorref(settings_background(dark));
    let text = colorref(settings_text(dark));
    let mica = glass && apply_backdrop_material(hwnd, BackdropMaterial::Mica);
    if !mica {
        let _ = apply_backdrop_material(hwnd, BackdropMaterial::None);
    }
    let margins = if mica {
        MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        }
    } else {
        MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 1,
            cyBottomHeight: 0,
        }
    };
    let (caption, border) = if mica {
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
        let round = DWMWCP_ROUND;
        for (attribute, color) in [
            (DWMWA_WINDOW_CORNER_PREFERENCE, round),
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
        let _ = SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    mica
}

#[must_use]
pub fn handle_settings_nccalcsize(hwnd: isize, wparam: usize, lparam: isize) -> Option<isize> {
    if wparam == 0 || lparam == 0 {
        return None;
    }
    let window = handle(hwnd);
    let params = lparam as *mut NCCALCSIZE_PARAMS;
    unsafe {
        let original = (*params).rgrc[0];
        let _ = DefWindowProcW(window, WM_NCCALCSIZE, WPARAM(wparam), LPARAM(lparam));
        if IsZoomed(window).as_bool() {
            let top_inset = (original.bottom - (*params).rgrc[0].bottom).max(0);
            (*params).rgrc[0].top = original.top + top_inset;
        } else {
            (*params).rgrc[0].top = original.top;
        }
    }
    Some(0)
}

#[must_use]
pub fn handle_settings_ncactivate(hwnd: isize) -> Option<isize> {
    let window = handle(hwnd);
    if unsafe { IsIconic(window).as_bool() } {
        return None;
    }
    Some(1)
}

const fn is_resize_border_hit(hit: u32) -> bool {
    matches!(
        hit,
        HTLEFT | HTRIGHT | HTTOP | HTTOPLEFT | HTTOPRIGHT | HTBOTTOM | HTBOTTOMLEFT | HTBOTTOMRIGHT
    )
}

#[must_use]
pub fn handle_settings_nchittest(
    hwnd: isize,
    lparam: isize,
    is_titlebar_drag: impl FnOnce(i32, i32) -> bool,
) -> Option<isize> {
    let window = handle(hwnd);
    let mut pt = POINT {
        x: i32::from((lparam & 0xFFFF) as i16),
        y: i32::from(((lparam >> 16) & 0xFFFF) as i16),
    };
    let (default_hit, mapped, zoomed) = unsafe {
        let hit = DefWindowProcW(window, WM_NCHITTEST, WPARAM(0), LPARAM(lparam)).0 as u32;
        if is_resize_border_hit(hit) {
            return Some(hit as isize);
        }
        (
            hit,
            ScreenToClient(window, &raw mut pt).as_bool(),
            IsZoomed(window).as_bool(),
        )
    };
    let _ = default_hit;
    if !mapped {
        return Some(HTCLIENT as isize);
    }
    let (width, _) = settings_client_size(hwnd);
    let dpi = settings_window_dpi(hwnd);
    if !zoomed {
        let top_edge = scale(5, dpi).max(4);
        let corner = scale(12, dpi).max(8);
        if pt.y >= 0 && pt.y < top_edge {
            let hit = if pt.x < corner {
                HTTOPLEFT
            } else if pt.x >= width - corner {
                HTTOPRIGHT
            } else {
                HTTOP
            };
            return Some(hit as isize);
        }
    }
    if is_titlebar_drag(pt.x, pt.y) {
        return Some(HTCAPTION as isize);
    }
    Some(HTCLIENT as isize)
}

#[must_use]
pub fn is_settings_window_maximized(hwnd: isize) -> bool {
    unsafe { IsZoomed(handle(hwnd)).as_bool() }
}

pub fn toggle_settings_window_maximize(hwnd: isize) {
    let window = handle(hwnd);
    unsafe {
        let command = if IsZoomed(window).as_bool() {
            SW_RESTORE
        } else {
            SW_MAXIMIZE
        };
        let _ = ShowWindow(window, command);
    }
}

pub fn minimize_settings_window(hwnd: isize) {
    unsafe {
        let _ = ShowWindow(handle(hwnd), SW_MINIMIZE);
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

pub fn apply_settings_min_track(hwnd: isize, lparam: LPARAM) {
    let metrics = SettingsMetrics::DEFAULT;
    apply_min_track(hwnd, lparam, metrics.min_width, metrics.min_height);
}

pub fn apply_finder_min_track(hwnd: isize, lparam: LPARAM) {
    let metrics = FinderMetrics::DEFAULT;
    apply_min_track(hwnd, lparam, metrics.min_width, metrics.min_height);
}

fn apply_min_track(hwnd: isize, lparam: LPARAM, min_width: i32, min_height: i32) {
    let info = lparam.0 as *mut MINMAXINFO;
    if info.is_null() {
        return;
    }
    let dpi = settings_window_dpi(hwnd);
    unsafe {
        (*info).ptMinTrackSize.x = scale(min_width, dpi);
        (*info).ptMinTrackSize.y = scale(min_height, dpi);
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

    #[test]
    fn custom_mica_frames_omit_sysmenu_so_dwm_never_draws_native_caption_buttons() {
        use std::mem::size_of;
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Dwm::{DWMWA_CAPTION_BUTTON_BOUNDS, DwmGetWindowAttribute};
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_STYLE, GetWindowLongW, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
            WS_THICKFRAME,
        };

        let hwnd = super::create_finder_window(Some(proc)).expect("finder window");
        let raw = HWND(hwnd as *mut _);
        let style = unsafe { GetWindowLongW(raw, GWL_STYLE) } as u32;
        assert_eq!(style & WS_SYSMENU.0, 0);
        assert_eq!(style & WS_CAPTION.0, WS_CAPTION.0);
        assert_ne!(style & WS_THICKFRAME.0, 0);
        assert_ne!(style & WS_MINIMIZEBOX.0, 0);
        assert_ne!(style & WS_MAXIMIZEBOX.0, 0);
        let mut buttons = RECT::default();
        let read = unsafe {
            DwmGetWindowAttribute(
                raw,
                DWMWA_CAPTION_BUTTON_BOUNDS,
                (&raw mut buttons).cast(),
                size_of::<RECT>() as u32,
            )
        };
        if read.is_ok() {
            assert!(
                buttons.right <= buttons.left || buttons.bottom <= buttons.top,
                "expected empty DWM caption buttons, got {buttons:?}"
            );
        }
        assert_eq!(super::handle_settings_ncactivate(hwnd), Some(1));
        destroy_settings_window(hwnd);
    }
}
