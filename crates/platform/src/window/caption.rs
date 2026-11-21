use std::mem::size_of;

use otto_core::{
    NATIVE_CORNER_RADIUS_DIP, NativeCaption, NativeCaptionStyle, PixelLimits, ResizeEdge,
    bgra_layout, extract_cover_rows, scale,
};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_BUTTON_BOUNDS, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWINDOWATTRIBUTE, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, GetDC, GetPixel, MONITOR_DEFAULTTONEAREST, MonitorFromWindow, ReleaseDC, SRCCOPY,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetDoubleClickTime, ReleaseCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_HWNDNEXT, GW_HWNDPREV, GWL_EXSTYLE, GWL_STYLE, GetSystemMetrics, GetWindow,
    GetWindowLongW, GetWindowRect, HTCAPTION, HTRIGHT, HTTOP, HTTOPRIGHT, HWND_NOTOPMOST, HWND_TOP,
    HWND_TOPMOST, IsWindow, IsWindowVisible, IsZoomed, PostMessageW, SM_CXDRAG, SM_CYDRAG,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow,
    SetWindowPos, WM_NCLBUTTONDOWN, WM_NCRBUTTONUP, WS_CAPTION, WS_EX_TOPMOST, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
};
use windows::core::BOOL;

use crate::gdi::{CaptureCanvas, ScreenDc};

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

fn window_screen_rect(window: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(window, &mut rect) }.ok()?;
    Some(rect)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenCaptionBounds {
    frame_right: i32,
    top: i32,
    height: i32,
    measured_width: i32,
}

fn screen_caption_bounds(frame: RECT, window: RECT, buttons: RECT) -> ScreenCaptionBounds {
    let buttons_top = (window.top + buttons.top).max(frame.top);
    let buttons_height = (buttons.bottom - buttons.top).max(0);
    let height = if buttons_height > 0 {
        (buttons_top - frame.top) + buttons_height
    } else {
        0
    };
    ScreenCaptionBounds {
        frame_right: frame.right,
        top: frame.top,
        height,
        measured_width: buttons.right - buttons.left,
    }
}

pub(crate) fn caption_style(style: u32) -> NativeCaptionStyle {
    NativeCaptionStyle {
        has_caption: style & WS_CAPTION.0 == WS_CAPTION.0,
        has_system_menu: style & WS_SYSMENU.0 != 0,
        can_minimize: style & WS_MINIMIZEBOX.0 != 0,
        can_maximize: style & WS_MAXIMIZEBOX.0 != 0,
    }
}

pub(crate) fn effective_caption_style(
    raw_style: u32,
    has_dwm_buttons: bool,
    full_screen: bool,
) -> NativeCaptionStyle {
    let base = caption_style(raw_style);
    if full_screen {
        return base;
    }
    let custom_chrome = !base.has_caption
        && (has_dwm_buttons
            || raw_style & (WS_SYSMENU.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0)
                != 0);
    let custom_resizable = custom_chrome && (raw_style & WS_THICKFRAME.0 != 0);
    NativeCaptionStyle {
        has_caption: base.has_caption || custom_chrome,
        has_system_menu: base.has_system_menu
            || custom_chrome
            || (base.has_caption && has_dwm_buttons),
        can_minimize: base.can_minimize || custom_resizable,
        can_maximize: base.can_maximize || custom_resizable,
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

pub(crate) fn has_dwm_caption_buttons(window: HWND) -> bool {
    dwm_rect(window, DWMWA_CAPTION_BUTTON_BOUNDS)
        .is_some_and(|buttons| buttons.right > buttons.left && buttons.bottom > buttons.top)
}

fn native_caption_with_buttons(
    window: HWND,
    style: NativeCaptionStyle,
    frame: RECT,
    buttons: RECT,
) -> Option<NativeCaption> {
    let window_rect = window_screen_rect(window)?;
    let bounds = screen_caption_bounds(frame, window_rect, buttons);
    if bounds.measured_width <= 0 || bounds.height <= 0 {
        return None;
    }
    let maximized = unsafe { IsZoomed(window) }.as_bool();
    let rounded_top_right = has_rounded_top_right(window, maximized);
    NativeCaption::new(
        style,
        bounds.frame_right,
        bounds.top,
        bounds.height,
        rounded_top_right,
        caption_dpi(window),
    )
    .map(|caption| {
        caption
            .measured(bounds.measured_width)
            .with_top_border_inset(top_border_inset_for(rounded_top_right))
    })
}

#[must_use]
pub(crate) fn native_caption_of(window: HWND) -> Option<NativeCaption> {
    let raw_style = unsafe { GetWindowLongW(window, GWL_STYLE) } as u32;
    let frame = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS)?;
    let buttons = dwm_rect(window, DWMWA_CAPTION_BUTTON_BOUNDS)
        .filter(|rect| rect.right > rect.left && rect.bottom > rect.top)?;
    let zoomed = unsafe { IsZoomed(window) }.as_bool();
    let full_screen = !zoomed
        && (raw_style & WS_CAPTION.0 != WS_CAPTION.0)
        && crate::window::model::covers_monitor(window);
    let style = effective_caption_style(raw_style, true, full_screen);
    native_caption_with_buttons(window, style, frame, buttons)
}

const fn top_border_inset_for(rounded_top_right: bool) -> bool {
    rounded_top_right
}

fn standard_caption_of_with_frame(
    window: HWND,
    style: NativeCaptionStyle,
    frame: RECT,
) -> Option<NativeCaption> {
    let maximized = unsafe { IsZoomed(window) }.as_bool();
    NativeCaption::standard(
        style,
        frame.right,
        frame.top,
        has_rounded_top_right(window, maximized),
        caption_dpi(window),
    )
}

#[must_use]
pub(crate) fn covering_caption_of_with_frame(
    window: HWND,
    raw_style: u32,
    frame: RECT,
) -> Option<NativeCaption> {
    let buttons = dwm_rect(window, DWMWA_CAPTION_BUTTON_BOUNDS)
        .filter(|rect| rect.right > rect.left && rect.bottom > rect.top);
    let zoomed = unsafe { IsZoomed(window) }.as_bool();
    let full_screen = !zoomed
        && (raw_style & WS_CAPTION.0 != WS_CAPTION.0)
        && crate::window::model::covers_monitor(window);
    let style = effective_caption_style(raw_style, buttons.is_some(), full_screen);
    buttons
        .and_then(|buttons| native_caption_with_buttons(window, style, frame, buttons))
        .or_else(|| standard_caption_of_with_frame(window, style, frame))
}

#[must_use]
pub(crate) fn covering_caption_of(window: HWND) -> Option<NativeCaption> {
    let raw_style = unsafe { GetWindowLongW(window, GWL_STYLE) } as u32;
    let frame = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS)?;
    covering_caption_of_with_frame(window, raw_style, frame)
}

#[must_use]
pub fn native_caption(window_hwnd: isize) -> Option<NativeCaption> {
    native_caption_of(HWND(window_hwnd as *mut _))
}

#[must_use]
pub fn window_corner_radius_px(window_hwnd: isize) -> f32 {
    let window = HWND(window_hwnd as *mut _);
    if !unsafe { IsWindow(Some(window)).as_bool() } {
        return 0.0;
    }
    let maximized = unsafe { IsZoomed(window).as_bool() };
    if !has_rounded_top_right(window, maximized) {
        return 0.0;
    }
    scale(NATIVE_CORNER_RADIUS_DIP, caption_dpi(window)) as f32
}

#[must_use]
fn sample_screen_color(x: i32, y: i32) -> Option<[u8; 3]> {
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

#[must_use]
pub fn sample_caption_strip(
    caption: NativeCaption,
    frame_left: i32,
    min_top: i32,
) -> Option<Vec<[u8; 3]>> {
    let rect = caption.clipped_sample_rect(frame_left, min_top);
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    let width = u32::try_from(rect.width).ok()?;
    let height = u32::try_from(rect.height).ok()?;
    let layout = bgra_layout(width, height, PixelLimits::ICON).ok()?;
    let screen = ScreenDc::acquire()?;
    let canvas = CaptureCanvas::create(&screen, layout)?;
    let copied = unsafe {
        BitBlt(
            canvas.dc(),
            0,
            0,
            rect.width,
            rect.height,
            Some(screen.handle()),
            rect.x,
            rect.y,
            SRCCOPY,
        )
    };
    let top_skip = (rect.y - caption.top).max(0) as usize;
    if copied.is_ok()
        && let Some(rows) = extract_cover_rows(canvas.as_bytes(), width as usize, height as usize)
    {
        return Some(pad_skipped_top_rows(rows, top_skip));
    }
    drop(canvas);
    drop(screen);
    sample_screen_color(rect.x + rect.width / 2, rect.y + rect.height / 2)
        .map(|rgb| vec![rgb; top_skip + height as usize])
}

fn pad_skipped_top_rows(rows: Vec<[u8; 3]>, top_skip: usize) -> Vec<[u8; 3]> {
    if top_skip == 0 {
        return rows;
    }
    let Some(&first) = rows.first() else {
        return rows;
    };
    let mut padded = Vec::with_capacity(top_skip + rows.len());
    padded.resize(top_skip, first);
    padded.extend_from_slice(&rows);
    padded
}

const fn pack_screen_coords(screen_x: i32, screen_y: i32) -> LPARAM {
    let packed = ((screen_y as i16 as u16 as usize) << 16) | (screen_x as i16 as u16 as usize);
    LPARAM(packed as isize)
}

#[must_use]
pub fn activate_window_no_restore(target_hwnd: isize) -> bool {
    unsafe { SetForegroundWindow(HWND(target_hwnd as *mut _)).as_bool() }
}

fn forward_nonclient_lbutton_down(
    target_hwnd: isize,
    hit_test: u32,
    screen_x: i32,
    screen_y: i32,
) -> bool {
    let target = HWND(target_hwnd as *mut _);
    unsafe {
        let _ = ReleaseCapture();
        let _ = SetForegroundWindow(target);
        PostMessageW(
            Some(target),
            WM_NCLBUTTONDOWN,
            WPARAM(hit_test as usize),
            pack_screen_coords(screen_x, screen_y),
        )
        .is_ok()
    }
}

#[must_use]
pub fn forward_caption_drag(target_hwnd: isize, screen_x: i32, screen_y: i32) -> bool {
    forward_nonclient_lbutton_down(target_hwnd, HTCAPTION, screen_x, screen_y)
}

#[must_use]
pub fn forward_resize_drag(
    target_hwnd: isize,
    edge: ResizeEdge,
    screen_x: i32,
    screen_y: i32,
) -> bool {
    let hit_test = match edge {
        ResizeEdge::Top => HTTOP,
        ResizeEdge::TopRight => HTTOPRIGHT,
        ResizeEdge::Right => HTRIGHT,
    };
    forward_nonclient_lbutton_down(target_hwnd, hit_test, screen_x, screen_y)
}

#[must_use]
pub fn forward_caption_context_menu(target_hwnd: isize, screen_x: i32, screen_y: i32) -> bool {
    let target = HWND(target_hwnd as *mut _);
    unsafe {
        let _ = SetForegroundWindow(target);
        PostMessageW(
            Some(target),
            WM_NCRBUTTONUP,
            WPARAM(HTCAPTION as usize),
            pack_screen_coords(screen_x, screen_y),
        )
        .is_ok()
    }
}

#[must_use]
pub fn place_overlay_above(overlay_hwnd: isize, target_hwnd: isize) -> bool {
    let overlay = HWND(overlay_hwnd as *mut _);
    let target = HWND(target_hwnd as *mut _);
    unsafe {
        if !IsWindow(Some(overlay)).as_bool() || !IsWindow(Some(target)).as_bool() {
            return false;
        }
        let target_ex = GetWindowLongW(target, GWL_EXSTYLE) as u32;
        let overlay_ex = GetWindowLongW(overlay, GWL_EXSTYLE) as u32;
        let target_topmost = target_ex & WS_EX_TOPMOST.0 != 0;
        let overlay_topmost = overlay_ex & WS_EX_TOPMOST.0 != 0;
        if overlay_topmost && !target_topmost {
            let _ = SetWindowPos(
                overlay,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let prev = GetWindow(target, GW_HWNDPREV)
            .ok()
            .filter(|hwnd| !hwnd.0.is_null());
        if prev == Some(overlay) && overlay_topmost == target_topmost {
            if !IsWindowVisible(overlay).as_bool() {
                let _ = SetWindowPos(
                    overlay,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            return true;
        }
        let insert_after = if target_topmost {
            prev.unwrap_or(HWND_TOPMOST)
        } else {
            match prev {
                Some(hwnd) if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOPMOST.0 != 0 => {
                    HWND_TOP
                }
                Some(hwnd) => hwnd,
                None => HWND_TOP,
            }
        };
        let _ = SetWindowPos(
            overlay,
            Some(insert_after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    true
}

fn lowest_anchor_hwnd(overlay: HWND, anchors: &[isize]) -> Option<HWND> {
    struct Probe<'a> {
        overlay: HWND,
        anchors: &'a [isize],
        lowest: Option<HWND>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = unsafe { &mut *(lparam.0 as *mut Probe<'_>) };
        let raw = hwnd.0 as isize;
        if hwnd != probe.overlay
            && probe.anchors.contains(&raw)
            && unsafe { IsWindowVisible(hwnd).as_bool() }
            && (unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0) != 0
        {
            probe.lowest = Some(hwnd);
        }
        BOOL(1)
    }
    if anchors.is_empty() {
        return None;
    }
    let mut probe = Probe {
        overlay,
        anchors,
        lowest: None,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.lowest
}

#[must_use]
pub fn place_overlay_below(overlay_hwnd: isize, anchors: &[isize]) -> bool {
    let overlay = HWND(overlay_hwnd as *mut _);
    unsafe {
        if !IsWindow(Some(overlay)).as_bool() {
            return false;
        }
        let Some(anchor) = lowest_anchor_hwnd(overlay, anchors) else {
            return SetWindowPos(
                overlay,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .is_ok();
        };
        let overlay_topmost = (GetWindowLongW(overlay, GWL_EXSTYLE) as u32) & WS_EX_TOPMOST.0 != 0;
        if !overlay_topmost {
            let _ = SetWindowPos(
                overlay,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let already_below = GetWindow(anchor, GW_HWNDNEXT)
            .ok()
            .is_some_and(|next| next == overlay);
        if already_below {
            return true;
        }
        SetWindowPos(
            overlay,
            Some(anchor),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .is_ok()
    }
}

#[must_use]
pub fn drag_threshold() -> (i32, i32) {
    (
        unsafe { GetSystemMetrics(SM_CXDRAG) }.max(1),
        unsafe { GetSystemMetrics(SM_CYDRAG) }.max(1),
    )
}

#[must_use]
pub const fn drag_threshold_exceeded(
    start: (i32, i32),
    current: (i32, i32),
    threshold: (i32, i32),
) -> bool {
    (current.0 - start.0).abs() >= threshold.0 || (current.1 - start.1).abs() >= threshold.1
}

#[must_use]
pub fn double_click_ms() -> u32 {
    unsafe { GetDoubleClickTime() }
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
    use super::{
        ScreenCaptionBounds, caption_style, drag_threshold_exceeded, pad_skipped_top_rows,
        rgb_from_colorref, screen_caption_bounds, top_border_inset_for,
    };
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
    };

    #[test]
    #[ignore = "reads every open window on the live desktop"]
    fn every_open_windows_caption_starts_inside_its_visible_frame() {
        use std::sync::Mutex;

        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
        };
        use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, IsWindowVisible, IsZoomed};
        use windows::core::BOOL;

        use super::{DWMWA_EXTENDED_FRAME_BOUNDS, dwm_rect, native_caption_of};

        static SEEN: Mutex<Vec<isize>> = Mutex::new(Vec::new());
        unsafe extern "system" fn visit(hwnd: HWND, _: LPARAM) -> BOOL {
            if unsafe { IsWindowVisible(hwnd) }.as_bool() {
                SEEN.lock().unwrap().push(hwnd.0 as isize);
            }
            BOOL(1)
        }

        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        SEEN.lock().unwrap().clear();
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(0)) };
        let windows = SEEN.lock().unwrap().clone();
        let mut checked = 0;
        for raw in windows {
            let hwnd = HWND(raw as *mut _);
            let Some(caption) = native_caption_of(hwnd) else {
                continue;
            };
            let Some(frame) = dwm_rect(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS) else {
                continue;
            };
            println!(
                "zoomed={} caption.top={} frame.top={} height={}",
                unsafe { IsZoomed(hwnd) }.as_bool(),
                caption.top,
                frame.top,
                caption.height
            );
            assert!(caption.top >= frame.top, "{} < {}", caption.top, frame.top);
            checked += 1;
        }
        unsafe { SetThreadDpiAwarenessContext(previous) };
        assert!(checked > 0);
    }

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

    #[test]
    fn top_border_inset_follows_rounded_top_right() {
        assert!(top_border_inset_for(true));
        assert!(!top_border_inset_for(false));
    }

    #[test]
    fn effective_caption_style_enables_custom_chrome_until_fullscreen() {
        use super::{WS_THICKFRAME, effective_caption_style};
        let custom = effective_caption_style(WS_THICKFRAME.0, true, false);
        assert!(custom.has_caption && custom.has_system_menu);
        assert!(custom.can_minimize && custom.can_maximize);

        let fullscreen = effective_caption_style(WS_THICKFRAME.0, true, true);
        assert!(!fullscreen.has_caption);
    }

    #[test]
    fn caption_button_bounds_translate_from_window_to_screen_coordinates() {
        let frame = RECT {
            left: 0,
            top: 108,
            right: 900,
            bottom: 700,
        };
        let window = RECT {
            left: 0,
            top: 100,
            right: 914,
            bottom: 714,
        };
        let buttons = RECT {
            left: 770,
            top: 12,
            right: 914,
            bottom: 48,
        };
        let bounds = screen_caption_bounds(frame, window, buttons);
        assert_eq!(
            bounds,
            ScreenCaptionBounds {
                frame_right: 900,
                top: 108,
                height: 40,
                measured_width: 144,
            }
        );
    }

    #[test]
    fn caption_never_starts_above_the_visible_frame() {
        let frame = RECT {
            left: 0,
            top: 108,
            right: 900,
            bottom: 700,
        };
        let window = RECT {
            left: 0,
            top: 100,
            right: 914,
            bottom: 714,
        };
        let buttons = RECT {
            left: 770,
            top: 6,
            right: 914,
            bottom: 42,
        };
        assert_eq!(screen_caption_bounds(frame, window, buttons).top, 108);
    }

    #[test]
    fn maximized_window_caption_starts_at_the_visible_frame_not_the_hidden_border() {
        let frame = RECT {
            left: 0,
            top: 60,
            right: 2240,
            bottom: 1301,
        };
        let window = RECT {
            left: -11,
            top: 49,
            right: 2251,
            bottom: 1312,
        };
        let buttons = RECT {
            left: 2030,
            top: 0,
            right: 2250,
            bottom: 44,
        };
        assert_eq!(
            screen_caption_bounds(frame, window, buttons),
            ScreenCaptionBounds {
                frame_right: 2240,
                top: 60,
                height: 44,
                measured_width: 220,
            }
        );
    }

    #[test]
    fn restored_window_with_flush_frame_keeps_the_window_top() {
        let frame = RECT {
            left: 509,
            top: 300,
            right: 1191,
            bottom: 711,
        };
        let window = RECT {
            left: 500,
            top: 300,
            right: 1200,
            bottom: 720,
        };
        let buttons = RECT {
            left: 470,
            top: 0,
            right: 690,
            bottom: 44,
        };
        assert_eq!(screen_caption_bounds(frame, window, buttons).top, 300);
    }

    #[test]
    fn skipped_top_rows_pad_sampled_strip_to_caption_origin() {
        let padded = pad_skipped_top_rows(vec![[10, 20, 30], [40, 50, 60]], 3);
        assert_eq!(
            padded,
            vec![
                [10, 20, 30],
                [10, 20, 30],
                [10, 20, 30],
                [10, 20, 30],
                [40, 50, 60],
            ]
        );
    }

    #[test]
    fn drag_threshold_distinguishes_click_jitter_from_intentional_drag() {
        assert!(!drag_threshold_exceeded((100, 20), (103, 22), (4, 4)));
        assert!(drag_threshold_exceeded((100, 20), (104, 20), (4, 4)));
        assert!(drag_threshold_exceeded((100, 20), (100, 24), (4, 4)));
    }

    #[test]
    fn invalid_handles_reject_corner_radius_and_below_placement() {
        assert!((super::window_corner_radius_px(0) - 0.0).abs() < 0.001);
        assert!(!super::place_overlay_below(0, &[1, 2]));
    }
}
