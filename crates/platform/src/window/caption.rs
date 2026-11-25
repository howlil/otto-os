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
    GWL_EXSTYLE, GWL_STYLE, GetSystemMetrics, GetWindowLongW, GetWindowRect, HTCAPTION, HTRIGHT,
    HTTOP, HTTOPRIGHT, IsWindow, IsZoomed, PostMessageW, SM_CXDRAG, SM_CYDRAG, SetForegroundWindow,
    WM_NCLBUTTONDOWN, WM_NCRBUTTONUP, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU,
    WS_THICKFRAME,
};

use crate::gdi::{CaptureCanvas, ScreenDc};
use crate::window::caption_probe::{
    ProbeEntry, cached_probe_entry, custom_chrome_eligible, fallback_caption_height_px,
    is_ghost_overlay_window, lookup_or_queue_probe, window_class_string,
};

const CORNER_DO_NOT_ROUND: u32 = 1;
const INVALID_PIXEL: u32 = 0xFFFF_FFFF;
const MIN_DWM_CAPTION_HEIGHT_DIP: i32 = 24;
const MAX_DWM_CAPTION_HEIGHT_DIP: i32 = 64;

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
    button_top_offset: i32,
    measured_width: i32,
}

fn normalize_dwm_buttons_for_dpi(
    buttons: RECT,
    window: RECT,
    window_dpi: u32,
    monitor_dpi: u32,
) -> RECT {
    if window_dpi == 0 || monitor_dpi == 0 || window_dpi == monitor_dpi {
        return buttons;
    }
    let physical_width = window.right - window.left;
    if physical_width <= 0 || buttons.right <= 0 {
        return buttons;
    }
    let expected_logical = (i64::from(physical_width) * i64::from(window_dpi)
        + i64::from(monitor_dpi / 2))
        / i64::from(monitor_dpi);
    let dist_logical = (i64::from(buttons.right) - expected_logical).abs();
    let dist_physical = (i64::from(buttons.right) - i64::from(physical_width)).abs();
    if dist_logical * 2 < dist_physical {
        let scale_coord = |value: i32| -> i32 {
            ((i64::from(value) * i64::from(monitor_dpi) + i64::from(window_dpi / 2))
                / i64::from(window_dpi)) as i32
        };
        return RECT {
            left: scale_coord(buttons.left),
            top: scale_coord(buttons.top),
            right: scale_coord(buttons.right),
            bottom: scale_coord(buttons.bottom),
        };
    }
    buttons
}

fn screen_caption_bounds(
    frame: RECT,
    window: RECT,
    buttons: RECT,
    window_dpi: u32,
    monitor_dpi: u32,
) -> ScreenCaptionBounds {
    let buttons = normalize_dwm_buttons_for_dpi(buttons, window, window_dpi, monitor_dpi);
    let raw_top = window.top + buttons.top;
    let raw_bottom = window.top + buttons.bottom;
    let visible_top = raw_top.max(frame.top);
    let visible_bottom = raw_bottom.max(visible_top);
    let height = if buttons.bottom > buttons.top {
        (visible_bottom - frame.top).max(0)
    } else {
        0
    };
    let button_top_offset = if height > 0 {
        (visible_top - frame.top).clamp(0, (height - 1).max(0))
    } else {
        0
    };
    ScreenCaptionBounds {
        frame_right: frame.right,
        top: frame.top,
        height,
        button_top_offset,
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
    effective_caption_style_with_eligibility(raw_style, has_dwm_buttons, full_screen, true)
}

fn effective_caption_style_with_eligibility(
    raw_style: u32,
    has_dwm_buttons: bool,
    full_screen: bool,
    allow_custom_chrome: bool,
) -> NativeCaptionStyle {
    let base = caption_style(raw_style);
    if full_screen {
        return base;
    }
    let custom_chrome = !base.has_caption && allow_custom_chrome;
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

pub(crate) fn effective_caption_style_for_window(
    window: HWND,
    raw_style: u32,
    full_screen: bool,
) -> NativeCaptionStyle {
    let (ex_style, zoomed) = unsafe {
        (
            GetWindowLongW(window, GWL_EXSTYLE) as u32,
            IsZoomed(window).as_bool(),
        )
    };
    if is_ghost_overlay_window(window, raw_style, ex_style) {
        return NativeCaptionStyle {
            has_caption: false,
            has_system_menu: false,
            can_minimize: false,
            can_maximize: false,
        };
    }
    let has_dwm_buttons = has_dwm_caption_buttons(window);
    let base = caption_style(raw_style);
    if base.has_caption {
        return effective_caption_style_with_eligibility(
            raw_style,
            has_dwm_buttons,
            full_screen,
            false,
        );
    }
    let class = window_class_string(window);
    let probe = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS).and_then(|frame| {
        let (_, monitor_dpi) = caption_window_and_monitor_dpi(window);
        cached_probe_entry(window, frame, monitor_dpi, zoomed)
    });
    let allow_custom = custom_chrome_eligible(&class, raw_style, has_dwm_buttons, probe);
    effective_caption_style_with_eligibility(raw_style, has_dwm_buttons, full_screen, allow_custom)
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

fn caption_window_and_monitor_dpi(window: HWND) -> (u32, u32) {
    let (mut dpi_x, mut dpi_y) = (0_u32, 0_u32);
    let (window_dpi, monitor_dpi) = unsafe {
        let win_dpi = GetDpiForWindow(window);
        let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mon_dpi =
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y).is_ok()
            {
                dpi_x
            } else {
                0
            };
        (win_dpi, mon_dpi)
    };
    let effective = match window_dpi.max(monitor_dpi) {
        0 => 96,
        dpi => dpi,
    };
    (
        if window_dpi == 0 {
            effective
        } else {
            window_dpi
        },
        effective,
    )
}

fn caption_dpi(window: HWND) -> u32 {
    caption_window_and_monitor_dpi(window).1
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
    maximized: bool,
) -> Option<NativeCaption> {
    let window_rect = window_screen_rect(window)?;
    let (window_dpi, monitor_dpi) = caption_window_and_monitor_dpi(window);
    let bounds = screen_caption_bounds(frame, window_rect, buttons, window_dpi, monitor_dpi);
    let min_height = scale(MIN_DWM_CAPTION_HEIGHT_DIP, monitor_dpi);
    let max_height = scale(MAX_DWM_CAPTION_HEIGHT_DIP, monitor_dpi);
    if bounds.measured_width <= 0 || bounds.height < min_height || bounds.height > max_height {
        return None;
    }
    let rounded_top_right = has_rounded_top_right(window, maximized);
    NativeCaption::new(
        style,
        bounds.frame_right,
        bounds.top,
        bounds.height,
        rounded_top_right,
        monitor_dpi,
    )
    .map(|caption| {
        caption
            .measured(bounds.measured_width)
            .with_top_border_inset(top_border_inset_for(rounded_top_right))
            .with_button_top_offset(bounds.button_top_offset)
    })
}

#[must_use]
pub(crate) fn native_caption_of(window: HWND) -> Option<NativeCaption> {
    let (raw_style, zoomed) = unsafe {
        (
            GetWindowLongW(window, GWL_STYLE) as u32,
            IsZoomed(window).as_bool(),
        )
    };
    let frame = dwm_rect(window, DWMWA_EXTENDED_FRAME_BOUNDS)?;
    let buttons = dwm_rect(window, DWMWA_CAPTION_BUTTON_BOUNDS)
        .filter(|rect| rect.right > rect.left && rect.bottom > rect.top)?;
    let full_screen = crate::window::model::is_full_screen(
        crate::window::model::covers_monitor(window),
        zoomed,
        raw_style,
    );
    let style = effective_caption_style(raw_style, true, full_screen);
    native_caption_with_buttons(window, style, frame, buttons, zoomed)
}

const fn top_border_inset_for(rounded_top_right: bool) -> bool {
    rounded_top_right
}

fn framework_caption_of_with_frame(
    window: HWND,
    class: &str,
    style: NativeCaptionStyle,
    frame: RECT,
    maximized: bool,
    dpi: u32,
) -> Option<NativeCaption> {
    let rounded_top_right = has_rounded_top_right(window, maximized);
    let height = fallback_caption_height_px(window, class, dpi, maximized);
    NativeCaption::new(
        style,
        frame.right,
        frame.top,
        height,
        rounded_top_right,
        dpi,
    )
    .map(|caption| caption.with_top_border_inset(top_border_inset_for(rounded_top_right)))
}

#[must_use]
pub(crate) fn covering_caption_of_with_frame(
    window: HWND,
    raw_style: u32,
    frame: RECT,
) -> Option<NativeCaption> {
    let (ex_style, zoomed) = unsafe {
        (
            GetWindowLongW(window, GWL_EXSTYLE) as u32,
            IsZoomed(window).as_bool(),
        )
    };
    if is_ghost_overlay_window(window, raw_style, ex_style) {
        return None;
    }
    let full_screen = crate::window::model::is_full_screen(
        crate::window::model::covers_monitor(window),
        zoomed,
        raw_style,
    );
    if full_screen {
        return None;
    }
    let buttons = dwm_rect(window, DWMWA_CAPTION_BUTTON_BOUNDS)
        .filter(|rect| rect.right > rect.left && rect.bottom > rect.top);
    let dpi = caption_dpi(window);
    let base = caption_style(raw_style);
    let class = window_class_string(window);
    let custom_chrome = !base.has_caption;
    let probe = lookup_or_queue_probe(window, frame, dpi, zoomed, custom_chrome);
    let allow_custom =
        !custom_chrome || custom_chrome_eligible(&class, raw_style, buttons.is_some(), probe);
    let style =
        effective_caption_style_with_eligibility(raw_style, buttons.is_some(), false, allow_custom);
    if !style.has_caption || !style.has_system_menu {
        return None;
    }
    if let Some(ProbeEntry::Measured(measured)) = probe {
        let rounded_top_right = has_rounded_top_right(window, zoomed);
        if let Some(caption) = NativeCaption::new(
            style,
            frame.right,
            frame.top,
            measured.height,
            rounded_top_right,
            dpi,
        ) {
            return Some(
                caption
                    .measured(measured.width)
                    .with_top_border_inset(top_border_inset_for(rounded_top_right))
                    .with_button_top_offset(measured.top_offset),
            );
        }
    }
    if !custom_chrome
        && let Some(buttons) = buttons
        && let Some(caption) = native_caption_with_buttons(window, style, frame, buttons, zoomed)
    {
        return Some(caption);
    }
    framework_caption_of_with_frame(window, &class, style, frame, zoomed, dpi)
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
    let (valid, maximized) =
        unsafe { (IsWindow(Some(window)).as_bool(), IsZoomed(window).as_bool()) };
    if !valid || !has_rounded_top_right(window, maximized) {
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
pub fn drag_threshold() -> (i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_CXDRAG).max(1),
            GetSystemMetrics(SM_CYDRAG).max(1),
        )
    }
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
        let bounds = screen_caption_bounds(frame, window, buttons, 96, 96);
        assert_eq!(
            bounds,
            ScreenCaptionBounds {
                frame_right: 900,
                top: 108,
                height: 40,
                button_top_offset: 4,
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
        assert_eq!(
            screen_caption_bounds(frame, window, buttons, 96, 96).top,
            108
        );
    }

    #[test]
    fn maximized_window_caption_clips_hidden_top_border_to_visible_height() {
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
            screen_caption_bounds(frame, window, buttons, 144, 144),
            ScreenCaptionBounds {
                frame_right: 2240,
                top: 60,
                height: 33,
                button_top_offset: 0,
                measured_width: 220,
            }
        );
    }

    #[test]
    fn dpi_virtualized_dwm_buttons_scale_to_physical_monitor_pixels() {
        let frame = RECT {
            left: 0,
            top: 100,
            right: 1500,
            bottom: 900,
        };
        let window = RECT {
            left: 0,
            top: 100,
            right: 1500,
            bottom: 900,
        };
        let logical_buttons = RECT {
            left: 862,
            top: 0,
            right: 1000,
            bottom: 30,
        };
        let bounds = screen_caption_bounds(frame, window, logical_buttons, 96, 144);
        assert_eq!(bounds.height, 45);
        assert_eq!(bounds.measured_width, 207);
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
        assert_eq!(
            screen_caption_bounds(frame, window, buttons, 144, 144).top,
            300
        );
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
}
