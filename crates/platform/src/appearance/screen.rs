use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use windows::core::BOOL;

const MONITORINFOF_PRIMARY_FLAG: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimaryScreen {
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMonitor {
    pub id: isize,
    pub bounds: otto_core::Rect,
    pub work_area: otto_core::Rect,
    pub dpi: u32,
    pub primary: bool,
}

impl DisplayMonitor {
    #[must_use]
    pub const fn as_primary_screen(self) -> PrimaryScreen {
        PrimaryScreen {
            width: self.bounds.width,
            height: self.bounds.height,
            dpi: self.dpi,
        }
    }

    #[must_use]
    pub const fn origin(self) -> (i32, i32) {
        (self.bounds.x, self.bounds.y)
    }
}

pub fn enable_per_monitor_dpi() {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

fn rect_from_win(rect: RECT) -> otto_core::Rect {
    otto_core::Rect {
        x: rect.left,
        y: rect.top,
        width: (rect.right - rect.left).max(0),
        height: (rect.bottom - rect.top).max(0),
    }
}

fn read_monitor(monitor: HMONITOR) -> Option<DisplayMonitor> {
    if monitor.0.is_null() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    let (mut dpi_x, mut dpi_y) = (96, 96);
    let ok = unsafe {
        let found = GetMonitorInfoW(monitor, &raw mut info).as_bool();
        if found && GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_err() {
            dpi_x = 96;
        }
        found
    };
    ok.then(|| DisplayMonitor {
        id: monitor.0 as isize,
        bounds: rect_from_win(info.rcMonitor),
        work_area: rect_from_win(info.rcWork),
        dpi: dpi_x.max(96),
        primary: (info.dwFlags & MONITORINFOF_PRIMARY_FLAG) != 0,
    })
}

fn fallback_primary_monitor() -> DisplayMonitor {
    let screen = primary_screen();
    let rect = otto_core::Rect {
        x: 0,
        y: 0,
        width: screen.width,
        height: screen.height,
    };
    DisplayMonitor {
        id: 0,
        bounds: rect,
        work_area: rect,
        dpi: screen.dpi.max(96),
        primary: true,
    }
}

#[must_use]
pub fn primary_screen() -> PrimaryScreen {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let (mut dpi_x, mut dpi_y) = (96, 96);
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_err() {
            dpi_x = 96;
        }
        PrimaryScreen {
            width: GetSystemMetrics(SM_CXSCREEN),
            height: GetSystemMetrics(SM_CYSCREEN),
            dpi: dpi_x.max(96),
        }
    }
}

#[must_use]
pub fn display_monitors() -> Vec<DisplayMonitor> {
    unsafe extern "system" fn visit(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        if let Some(info) = read_monitor(monitor) {
            let list = unsafe { &mut *(lparam.0 as *mut Vec<DisplayMonitor>) };
            list.push(info);
        }
        BOOL(1)
    }

    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(visit), LPARAM(&raw mut monitors as isize));
    }
    if monitors.is_empty() {
        monitors.push(fallback_primary_monitor());
        return monitors;
    }
    monitors.sort_by_key(|m| (!m.primary, m.bounds.x, m.bounds.y, m.id));
    monitors
}

#[must_use]
pub fn secondary_monitors() -> Vec<DisplayMonitor> {
    display_monitors()
        .into_iter()
        .filter(|monitor| !monitor.primary)
        .collect()
}

#[must_use]
pub fn monitor_at(x: i32, y: i32) -> DisplayMonitor {
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    read_monitor(monitor).unwrap_or_else(fallback_primary_monitor)
}

#[must_use]
pub fn monitor_for_window(hwnd: isize) -> DisplayMonitor {
    if hwnd == 0 {
        return monitor_at(0, 0);
    }
    let monitor = unsafe { MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST) };
    read_monitor(monitor).unwrap_or_else(fallback_primary_monitor)
}

#[must_use]
pub fn monitor_work_area_at(x: i32, y: i32) -> otto_core::Rect {
    monitor_at(x, y).work_area
}

#[must_use]
pub fn virtual_screen() -> otto_core::Rect {
    unsafe {
        otto_core::Rect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}
