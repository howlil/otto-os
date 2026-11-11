use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::Shell::{
    ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA, SHAppBarMessage,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SHOW_WINDOW_CMD, SPI_GETWORKAREA, SPI_SETWORKAREA, SPIF_SENDCHANGE, SW_HIDE,
    SW_SHOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, ShowWindow, SystemParametersInfoW,
};
use windows::core::{PCWSTR, w};

#[derive(Debug)]
pub struct AppBar {
    hwnd: HWND,
    original_state: usize,
    original_work_area: RECT,
    taskbar_hidden: bool,
}

impl AppBar {
    #[must_use]
    pub fn register(hwnd: isize) -> Self {
        let hwnd = HWND(hwnd as *mut _);
        let mut data = Self::data(hwnd);
        let original_state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) };
        let mut original_work_area = Self::get_work_area();
        original_work_area.top = 0;
        Self {
            hwnd,
            original_state,
            original_work_area,
            taskbar_hidden: false,
        }
    }

    pub fn reserve_top(&self, screen_width: i32, screen_height: i32, bar_height: i32) {
        let desired = RECT {
            left: 0,
            top: bar_height,
            right: screen_width,
            bottom: screen_height,
        };
        if Self::get_work_area() != desired {
            Self::set_work_area(desired);
        }
    }

    pub fn hide_system_taskbar(&mut self) {
        self.set_taskbar_state(self.original_state | ABS_AUTOHIDE as usize);
        Self::set_tray_visibility(SW_HIDE);
        self.taskbar_hidden = true;
    }

    pub fn maintain(&self, screen_width: i32, screen_height: i32, bar_height: i32) {
        if self.taskbar_hidden {
            Self::set_tray_visibility(SW_HIDE);
        }
        self.reserve_top(screen_width, screen_height, bar_height);
    }

    fn get_work_area() -> RECT {
        let mut rect = RECT::default();
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some((&raw mut rect).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            );
        }
        rect
    }

    fn set_work_area(mut rect: RECT) {
        unsafe {
            let _ = SystemParametersInfoW(
                SPI_SETWORKAREA,
                0,
                Some((&raw mut rect).cast()),
                SPIF_SENDCHANGE,
            );
        }
    }

    fn set_taskbar_state(&self, state: usize) {
        let mut data = Self::data(self.hwnd);
        data.lParam = LPARAM(state as isize);
        unsafe {
            SHAppBarMessage(ABM_SETSTATE, &mut data);
        }
    }

    fn set_tray_visibility(cmd: SHOW_WINDOW_CMD) {
        unsafe {
            if let Ok(tray) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null())
                && !tray.0.is_null()
            {
                let _ = ShowWindow(tray, cmd);
            }
        }
    }

    fn data(hwnd: HWND) -> APPBARDATA {
        APPBARDATA {
            cbSize: size_of::<APPBARDATA>() as u32,
            hWnd: hwnd,
            ..APPBARDATA::default()
        }
    }
}

impl Drop for AppBar {
    fn drop(&mut self) {
        Self::set_work_area(self.original_work_area);
        if self.taskbar_hidden {
            Self::set_tray_visibility(SW_SHOW);
            self.set_taskbar_state(self.original_state);
        }
    }
}
