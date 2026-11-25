use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{
    CropRegion, PixelLimits, Rect, bgra_layout, crop_bgra, is_blank_frame, resize_bgra,
    scale_dimension,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_TRANSITIONS_FORCEDISABLED, DwmFlush,
    DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    MONITORINFO, MonitorFromPoint, MonitorFromWindow, SRCCOPY,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GA_ROOT, GW_OWNER, GWL_EXSTYLE, GetAncestor, GetClassNameW, GetCursorPos,
    GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowPlacement, GetWindowRect,
    GetWindowTextLengthW, GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindowVisible,
    IsZoomed, PostMessageW, SC_MINIMIZE, SC_RESTORE, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
    SetForegroundWindow, WINDOWPLACEMENT, WM_NULL, WM_SYSCOMMAND, WPF_RESTORETOMAXIMIZED,
    WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};
use windows::core::BOOL;

use crate::gdi::{CaptureCanvas, ScreenDc};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};

pub(crate) const MAX_OWNER_DEPTH: usize = 16;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWindowTarget {
    pub hwnd: isize,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub maximized: bool,
    pub dpi: u32,
    pub on_primary: bool,
    pub active: bool,
    pub sample_unobstructed: bool,
    pub caption: Option<otto_core::NativeCaption>,
}

#[derive(Debug, Clone)]
pub struct WindowSnapshot {
    pub rect: Rect,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[must_use]
pub fn cursor_position() -> (i32, i32) {
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
    }
    (pt.x, pt.y)
}

pub(crate) const COVER_WIDTH_SLACK: i32 = 12;
pub(crate) const COVER_HEIGHT_SLACK: i32 = 56;
pub(crate) const COVER_BOTTOM_SLACK: i32 = 8;

pub(crate) fn window_covers_dock(
    on_primary: bool,
    zoomed: bool,
    rect: RECT,
    screen_width: i32,
    screen_height: i32,
) -> bool {
    if !on_primary {
        return false;
    }
    zoomed
        || (rect.right - rect.left >= screen_width - COVER_WIDTH_SLACK
            && rect.bottom - rect.top >= screen_height - COVER_HEIGHT_SLACK
            && rect.bottom >= screen_height - COVER_BOTTOM_SLACK)
}

pub(crate) fn window_is_fullscreen_on_primary(
    on_primary: bool,
    zoomed: bool,
    rect: RECT,
    screen_width: i32,
    screen_height: i32,
) -> bool {
    on_primary
        && !zoomed
        && screen_width > 0
        && screen_height > 0
        && rect.left <= 0
        && rect.top <= 0
        && rect.right >= screen_width
        && rect.bottom >= screen_height
}

pub(crate) const fn rects_overlap(a: RECT, b: RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

#[must_use]
pub fn has_maximized_or_fullscreen_window(
    screen_width: i32,
    screen_height: i32,
    own_pid: u32,
) -> bool {
    struct Probe {
        screen_width: i32,
        screen_height: i32,
        own_pid: u32,
        primary_monitor: HMONITOR,
        found: bool,
    }

    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = unsafe { &mut *(lparam.0 as *mut Probe) };
        if is_normal_visible_window(hwnd, probe.own_pid) {
            let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            let on_primary = monitor == probe.primary_monitor;
            let zoomed = on_primary && unsafe { IsZoomed(hwnd) }.as_bool();
            let rect = if on_primary {
                visible_window_rect(hwnd)
            } else {
                RECT::default()
            };
            if window_covers_dock(
                on_primary,
                zoomed,
                rect,
                probe.screen_width,
                probe.screen_height,
            ) {
                probe.found = true;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    let primary_monitor =
        unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };

    let mut probe = Probe {
        screen_width,
        screen_height,
        own_pid,
        primary_monitor,
        found: false,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.found
}

pub(crate) const MIN_TARGET_WIDTH: i32 = 200;
pub(crate) const MIN_TARGET_HEIGHT: i32 = 80;

pub(crate) fn window_target(
    hwnd: HWND,
    rect: RECT,
    primary: HMONITOR,
    active: bool,
    above: &[Rect],
) -> ActiveWindowTarget {
    let (_, monitor_dpi) = window_and_monitor_dpi(hwnd);
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let caption = crate::window::caption::covering_caption_of(hwnd);
    let sample_unobstructed = caption.is_some_and(|cap| {
        let sample = cap.sample_rect();
        !above
            .iter()
            .any(|higher| higher.intersection(&sample).is_some())
    });
    ActiveWindowTarget {
        hwnd: hwnd.0 as isize,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        maximized: unsafe { IsZoomed(hwnd).as_bool() },
        dpi: monitor_dpi,
        on_primary: monitor == primary,
        active,
        sample_unobstructed,
        caption,
    }
}

pub(crate) fn should_record_sample_occluder(hwnd: HWND, own_pid: u32) -> Option<(bool, Rect, u32)> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        if pid == 0 {
            return None;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TRANSPARENT.0 != 0 {
            return None;
        }
        let mut class_buf = [0u16; 128];
        let class_len = window_class_utf16(hwnd, &mut class_buf);
        let class = &class_buf[..class_len];
        if is_shell_or_desktop_utf16(class) {
            return None;
        }
        let own_app =
            pid == own_pid && (is_own_settings_utf16(class) || utf16_eq_ascii(class, "OttoFinder"));
        if pid == own_pid
            && !own_app
            && !utf16_eq_ascii(class, "OttoPopupMenu")
            && !utf16_eq_ascii(class, "OttoNotchPanel")
            && !utf16_eq_ascii(class, "OttoControlCenter")
            && !utf16_eq_ascii(class, "OttoSpotlight")
        {
            return None;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return None;
        }
        let rect = visible_window_rect(hwnd);
        let bounds = Rect {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        };
        if bounds.width <= 0 || bounds.height <= 0 {
            return None;
        }
        let is_normal = (pid != own_pid || own_app)
            && !(ex_style & WS_EX_TOOLWINDOW.0 != 0 && ex_style & WS_EX_APPWINDOW.0 == 0)
            && ex_style & WS_EX_NOACTIVATE.0 == 0
            && GetWindowTextLengthW(hwnd) > 0;
        Some((is_normal, bounds, pid))
    }
}

pub(crate) struct TargetSweep {
    own_pid: u32,
    primary: HMONITOR,
    foreground: HWND,
    active_root: HWND,
    above_normal: Vec<Rect>,
    sample_occluders: Vec<Rect>,
    uipi_cache: Vec<(u32, bool)>,
    targets: Vec<ActiveWindowTarget>,
}

impl TargetSweep {
    fn is_uipi_blocked(&mut self, hwnd: HWND, pid: u32) -> bool {
        if let Some(&(_, blocked)) = self.uipi_cache.iter().find(|&&(cached, _)| cached == pid) {
            return blocked;
        }
        let blocked = crate::window::model::is_window_uipi_blocked(hwnd);
        self.uipi_cache.push((pid, blocked));
        blocked
    }
}

unsafe extern "system" fn collect_window_target(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let sweep = unsafe { &mut *(lparam.0 as *mut TargetSweep) };
    let Some((is_normal, bounds, pid)) = should_record_sample_occluder(hwnd, sweep.own_pid) else {
        return BOOL(1);
    };
    if is_normal {
        if pid != sweep.own_pid
            && bounds.width >= MIN_TARGET_WIDTH
            && bounds.height >= MIN_TARGET_HEIGHT
            && !sweep.is_uipi_blocked(hwnd, pid)
        {
            let rect = RECT {
                left: bounds.x,
                top: bounds.y,
                right: bounds.right(),
                bottom: bounds.bottom(),
            };
            let active = hwnd == sweep.foreground || hwnd == sweep.active_root;
            let target = window_target(hwnd, rect, sweep.primary, active, &sweep.sample_occluders);
            let above = &sweep.above_normal;
            if target
                .caption
                .is_some_and(|c| !above.iter().any(|a| a.contains_rect(&c.cover_rect())))
            {
                sweep.targets.push(target);
            }
        }
        sweep.above_normal.push(bounds);
    }
    sweep.sample_occluders.push(bounds);
    BOOL(1)
}

#[must_use]
pub fn visible_window_targets(own_pid: u32) -> Vec<ActiveWindowTarget> {
    let foreground = crate::os_events::current_foreground();
    let active_root = if foreground.0.is_null() {
        foreground
    } else {
        visible_root_owner(foreground)
    };
    let mut sweep = TargetSweep {
        own_pid,
        primary: unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) },
        foreground,
        active_root,
        above_normal: Vec::new(),
        sample_occluders: Vec::new(),
        uipi_cache: Vec::new(),
        targets: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(collect_window_target), LPARAM(&raw mut sweep as isize));
    }
    sweep.targets
}

#[must_use]
pub fn window_bounds(hwnd_val: isize) -> Option<Rect> {
    let rect = visible_window_rect(HWND(hwnd_val as *mut c_void));
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    (width > 0 && height > 0).then_some(Rect {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

#[must_use]
pub fn restored_window_bounds(hwnd_val: isize) -> Option<Rect> {
    let hwnd = HWND(hwnd_val as *mut c_void);
    if !unsafe { IsIconic(hwnd).as_bool() } {
        return window_bounds(hwnd_val);
    }
    let mut placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..WINDOWPLACEMENT::default()
    };
    unsafe { GetWindowPlacement(hwnd, &raw mut placement) }.ok()?;
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    let has_info = unsafe { GetMonitorInfoW(monitor, &raw mut info).as_bool() };
    if placement.flags.0 & WPF_RESTORETOMAXIMIZED.0 != 0 && has_info {
        let work = info.rcWork;
        let width = work.right - work.left;
        let height = work.bottom - work.top;
        return (width > 0 && height > 0).then_some(Rect {
            x: work.left,
            y: work.top,
            width,
            height,
        });
    }
    let normal = placement.rcNormalPosition;
    let width = normal.right - normal.left;
    let height = normal.bottom - normal.top;
    if width <= 0 || height <= 0 {
        return None;
    }
    let (dx, dy) = if has_info {
        (
            info.rcWork.left - info.rcMonitor.left,
            info.rcWork.top - info.rcMonitor.top,
        )
    } else {
        (0, 0)
    };
    Some(Rect {
        x: normal.left + dx,
        y: normal.top + dy,
        width,
        height,
    })
}

#[must_use]
pub fn is_window_uipi(hwnd_val: isize) -> bool {
    crate::window::model::is_window_uipi_blocked(HWND(hwnd_val as *mut c_void))
}

pub(crate) const CAPTURE_RESPONSIVENESS_PROBE_MS: u32 = 50;

pub(crate) fn is_window_unobstructed(target: HWND, frame: Rect) -> bool {
    struct OcclusionProbe {
        target: HWND,
        frame: Rect,
        own_pid: u32,
        reached_target: bool,
        occluded: bool,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let probe = unsafe { &mut *(lparam.0 as *mut OcclusionProbe) };
        if hwnd == probe.target {
            probe.reached_target = true;
            return BOOL(0);
        }
        if let Some((_, bounds, _)) = should_record_sample_occluder(hwnd, probe.own_pid)
            && bounds.intersection(&probe.frame).is_some()
        {
            probe.occluded = true;
            return BOOL(0);
        }
        BOOL(1)
    }
    if !unsafe { IsWindowVisible(target).as_bool() } || unsafe { IsIconic(target).as_bool() } {
        return false;
    }
    let mut cloaked = 0u32;
    let _ = unsafe {
        DwmGetWindowAttribute(
            target,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    if cloaked != 0 {
        return false;
    }
    let mut probe = OcclusionProbe {
        target,
        frame,
        own_pid: current_process_id(),
        reached_target: false,
        occluded: false,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&raw mut probe as isize));
    }
    probe.reached_target && !probe.occluded
}

pub(crate) fn copy_from_screen(
    screen: &ScreenDc,
    frame: Rect,
    frame_layout: otto_core::BgraLayout,
) -> Option<Vec<u8>> {
    let canvas = CaptureCanvas::create(screen, frame_layout)?;
    unsafe {
        BitBlt(
            canvas.dc(),
            0,
            0,
            frame.width,
            frame.height,
            Some(screen.handle()),
            frame.x,
            frame.y,
            SRCCOPY,
        )
    }
    .ok()?;
    let bytes = canvas.as_bytes();
    (!is_blank_frame(frame_layout.width, frame_layout.height, bytes)).then(|| bytes.to_vec())
}

pub(crate) fn capture_via_print_window(
    hwnd: HWND,
    screen: &ScreenDc,
    frame: Rect,
    frame_layout: otto_core::BgraLayout,
) -> Option<Vec<u8>> {
    if !is_window_responsive(hwnd) {
        return None;
    }
    let outer = outer_window_rect(hwnd).unwrap_or(frame);
    let canvas_layout = bgra_layout(
        u32::try_from(outer.width.max(frame.width)).ok()?,
        u32::try_from(outer.height.max(frame.height)).ok()?,
        PixelLimits::WINDOW_SNAPSHOT,
    )
    .ok()?;
    let canvas = CaptureCanvas::create(screen, canvas_layout)?;
    let (window_dpi, monitor_dpi) = window_and_monitor_dpi(hwnd);
    let painted_length =
        |length: i32| scale_dimension(u32::try_from(length).unwrap_or(0), window_dpi, monitor_dpi);
    let printed_region = CropRegion {
        x: painted_length(frame.x - outer.x).min(canvas_layout.width - 1),
        y: painted_length(frame.y - outer.y).min(canvas_layout.height - 1),
        width: painted_length(frame.width),
        height: painted_length(frame.height),
    };
    let printed_layout = bgra_layout(
        printed_region.width,
        printed_region.height,
        PixelLimits::WINDOW_SNAPSHOT,
    )
    .ok()?;
    unsafe { PrintWindow(hwnd, canvas.dc(), PRINT_WINDOW_FLAGS(2)) }
        .as_bool()
        .then(|| crop_bgra(canvas.as_bytes(), canvas.layout(), printed_region))
        .flatten()
        .filter(|pixels| !is_blank_frame(printed_layout.width, printed_layout.height, pixels))
        .and_then(|pixels| resize_bgra(&pixels, printed_layout, frame_layout))
}

#[must_use]
pub fn capture_window(hwnd_val: isize) -> Option<WindowSnapshot> {
    let hwnd = HWND(hwnd_val as *mut c_void);
    let frame = window_bounds(hwnd_val)?;
    if !unsafe { IsWindowVisible(hwnd).as_bool() } || unsafe { IsIconic(hwnd).as_bool() } {
        return None;
    }
    let frame_layout = bgra_layout(
        u32::try_from(frame.width).ok()?,
        u32::try_from(frame.height).ok()?,
        PixelLimits::WINDOW_SNAPSHOT,
    )
    .ok()?;
    let screen = ScreenDc::acquire()?;
    let virtual_bounds = crate::screen::virtual_screen();
    let on_screen = virtual_bounds.contains_rect(&frame);
    let unobstructed = on_screen && is_window_unobstructed(hwnd, frame);

    let mut pixels = if unobstructed && !unsafe { IsHungAppWindow(hwnd).as_bool() } {
        copy_from_screen(&screen, frame, frame_layout)
    } else {
        None
    };

    if pixels.is_none() {
        pixels = capture_via_print_window(hwnd, &screen, frame, frame_layout);
    }

    if pixels.is_none() && !unobstructed && on_screen {
        let foreground = unsafe { GetForegroundWindow() };
        if foreground == hwnd || visible_root_owner(foreground) == hwnd {
            pixels = copy_from_screen(&screen, frame, frame_layout);
        }
    }

    let pixels = pixels?;
    Some(WindowSnapshot {
        rect: frame,
        width: frame_layout.width,
        height: frame_layout.height,
        pixels: opaque(pixels),
    })
}

pub(crate) fn window_and_monitor_dpi(hwnd: HWND) -> (u32, u32) {
    let window_dpi = unsafe { GetDpiForWindow(hwnd) };
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let (mut dpi_x, mut dpi_y) = (0u32, 0u32);
    let monitor_dpi = if unsafe {
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
    }
    .is_ok()
    {
        dpi_x
    } else {
        window_dpi
    };
    if window_dpi == 0 || monitor_dpi == 0 {
        (96, 96)
    } else {
        (window_dpi, monitor_dpi)
    }
}

pub(crate) fn opaque(mut pixels: Vec<u8>) -> Vec<u8> {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    pixels
}

pub(crate) fn is_window_responsive(hwnd: HWND) -> bool {
    let mut reply = 0usize;
    unsafe {
        !IsHungAppWindow(hwnd).as_bool()
            && SendMessageTimeoutW(
                hwnd,
                WM_NULL,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                CAPTURE_RESPONSIVENESS_PROBE_MS,
                Some(&raw mut reply),
            )
            .0 != 0
    }
}

pub(crate) fn outer_window_rect(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    (width > 0 && height > 0).then_some(Rect {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

#[must_use]
pub fn is_window_cloaked(hwnd_val: isize) -> bool {
    let mut cloaked = 0u32;
    let read = unsafe {
        DwmGetWindowAttribute(
            HWND(hwnd_val as *mut c_void),
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    read.is_ok() && cloaked != 0
}

pub(crate) fn suppress_transitions(hwnd_val: isize) -> bool {
    use crate::window::hidden::{Concealment, forget, record};
    let Some(identity) = crate::window::model::current_identity(hwnd_val) else {
        return false;
    };
    if !record(identity, Concealment::TransitionsDisabled) {
        return false;
    }
    let applied = set_dwm_transition_policy(HWND(hwnd_val as *mut c_void), false);
    if !applied {
        forget(hwnd_val, Concealment::TransitionsDisabled);
    }
    applied
}

pub(crate) fn release_transitions(hwnd_val: isize) -> bool {
    let applied = set_dwm_transition_policy(HWND(hwnd_val as *mut c_void), true);
    if applied {
        crate::window::hidden::forget(
            hwnd_val,
            crate::window::hidden::Concealment::TransitionsDisabled,
        );
    }
    applied
}

pub fn flush_compositor() {
    unsafe {
        let _ = DwmFlush();
    }
}

#[must_use]
pub fn set_window_transitions_enabled(hwnd_val: isize, enabled: bool) -> bool {
    if enabled {
        release_transitions(hwnd_val)
    } else {
        suppress_transitions(hwnd_val)
    }
}

#[must_use]
pub fn is_window_minimized(hwnd_val: isize) -> bool {
    unsafe { IsIconic(HWND(hwnd_val as *mut c_void)).as_bool() }
}

#[must_use]
pub fn is_window_foreground(hwnd_val: isize) -> bool {
    foreground_root_hwnd() == Some(hwnd_val)
}

pub fn minimize_window_native(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    enable_dwm_transitions(hwnd);
    post_minimize(hwnd);
}

#[must_use]
pub fn minimize_window_native_quiet(hwnd_val: isize) -> bool {
    let quiet = suppress_transitions(hwnd_val);
    post_minimize(HWND(hwnd_val as *mut c_void));
    quiet
}

pub(crate) fn post_minimize(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM(SC_MINIMIZE as usize),
            LPARAM(0),
        );
    }
}

pub fn restore_window_native(hwnd_val: isize) {
    let hwnd = HWND(hwnd_val as *mut c_void);
    enable_dwm_transitions(hwnd);
    send_restore(hwnd);
}

#[must_use]
pub fn restore_window_native_quiet(hwnd_val: isize) -> bool {
    let quiet = suppress_transitions(hwnd_val);
    send_restore(HWND(hwnd_val as *mut c_void));
    quiet
}

pub(crate) fn send_restore(hwnd: HWND) {
    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_SYSCOMMAND,
            WPARAM(SC_RESTORE as usize),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            80,
            None,
        );
        let _ = SetForegroundWindow(hwnd);
    }
}

#[must_use]
pub fn current_process_id() -> u32 {
    unsafe { GetCurrentProcessId() }
}

pub(crate) fn enable_dwm_transitions(hwnd: HWND) {
    let _ = release_transitions(hwnd.0 as isize);
}

pub(crate) fn set_dwm_transition_policy(hwnd: HWND, enabled: bool) -> bool {
    let disable = u32::from(!enabled);
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&raw const disable).cast(),
            size_of::<u32>() as u32,
        )
        .is_ok()
    }
}

pub(crate) fn window_class_utf16(hwnd: HWND, buf: &mut [u16; 128]) -> usize {
    let len = unsafe { GetClassNameW(hwnd, buf) };
    if len <= 0 {
        0
    } else {
        (len as usize).min(buf.len())
    }
}

pub(crate) fn utf16_eq_ascii(wide: &[u16], ascii: &str) -> bool {
    wide.len() == ascii.len()
        && wide
            .iter()
            .zip(ascii.bytes())
            .all(|(&unit, byte)| unit == u16::from(byte))
}

pub(crate) fn is_own_settings_utf16(wide: &[u16]) -> bool {
    utf16_eq_ascii(wide, "OttoSettings")
}

pub(crate) fn is_own_settings_window(hwnd: HWND) -> bool {
    window_class_is(hwnd, "OttoSettings")
}

pub(crate) fn window_class_is(hwnd: HWND, expected: &str) -> bool {
    let mut buf = [0u16; 128];
    let len = window_class_utf16(hwnd, &mut buf);
    utf16_eq_ascii(&buf[..len], expected)
}

pub(crate) fn is_shell_or_desktop_utf16(wide: &[u16]) -> bool {
    const CLASSES: [&str; 10] = [
        "Progman",
        "WorkerW",
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Windows.UI.Core.CoreWindow",
        "XamlExplorerHostIslandWindow",
        "ForegroundStaging",
        "MultitaskingViewFrame",
        "NotifyIconOverflowWindow",
        "TopLevelWindowForOverflowXamlIsland",
    ];
    CLASSES.iter().any(|name| utf16_eq_ascii(wide, name))
}

pub(crate) fn is_shell_or_desktop_window(hwnd: HWND) -> bool {
    let mut buf = [0u16; 128];
    let len = window_class_utf16(hwnd, &mut buf);
    is_shell_or_desktop_utf16(&buf[..len])
}

pub(crate) fn visible_window_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut rect).cast(),
            size_of::<RECT>() as u32,
        )
    };
    if hr.is_err() || rect.right <= rect.left || rect.bottom <= rect.top {
        unsafe {
            let _ = GetWindowRect(hwnd, &mut rect);
        }
    }
    rect
}

pub(crate) fn is_normal_visible_window(hwnd: HWND, own_pid: u32) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        if pid == 0 || (pid == own_pid && !is_own_settings_window(hwnd)) {
            return false;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 && ex_style & WS_EX_APPWINDOW.0 == 0 {
            return false;
        }
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return false;
        }
        if is_shell_or_desktop_window(hwnd) {
            return false;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        cloaked == 0 && GetWindowTextLengthW(hwnd) > 0
    }
}

pub(crate) fn visible_root_owner(window: HWND) -> HWND {
    let mut current = unsafe { GetAncestor(window, GA_ROOT) };
    if current.0.is_null() {
        current = window;
    }
    for _ in 0..MAX_OWNER_DEPTH {
        let owner = unsafe { GetWindow(current, GW_OWNER) }.unwrap_or_default();
        if owner.0.is_null() || !unsafe { IsWindowVisible(owner) }.as_bool() {
            break;
        }
        current = owner;
    }
    current
}

pub(crate) fn foreground_root_hwnd() -> Option<isize> {
    let foreground = crate::os_events::current_foreground();
    (!foreground.0.is_null()).then(|| visible_root_owner(foreground).0 as isize)
}
