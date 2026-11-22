use std::ffi::c_void;

use otto_core::Rect;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS;
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LEFT, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GW_HWNDNEXT, GWL_EXSTYLE, GetForegroundWindow, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowThreadProcessId, HWND_TOPMOST, IsIconic, IsWindowVisible,
    SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow,
    WS_EX_TOOLWINDOW,
};

use super::caption::dwm_rect;
use super::ops::foreground_root_hwnd;
use crate::actions::send_key_chord;

const WINDOW_WALK_LIMIT: usize = 64;

fn handle(window: isize) -> HWND {
    HWND(window as *mut c_void)
}

#[must_use]
pub fn foreground_window() -> Option<isize> {
    foreground_root_hwnd()
}

#[must_use]
pub fn window_frame(window: isize) -> Option<Rect> {
    let hwnd = handle(window);
    if unsafe { IsIconic(hwnd) }.as_bool() {
        return None;
    }
    let frame = dwm_rect(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS)?;
    Some(Rect {
        x: frame.left,
        y: frame.top,
        width: frame.right - frame.left,
        height: frame.bottom - frame.top,
    })
}

#[must_use]
pub fn window_dpi(window: isize) -> u32 {
    match unsafe { GetDpiForWindow(handle(window)) } {
        0 => 96,
        dpi => dpi,
    }
}

pub fn show_over(window: isize, rect: Rect) {
    place(window, rect);
    activate_window(window);
}

pub fn place(window: isize, rect: Rect) {
    unsafe {
        let _ = SetWindowPos(
            handle(window),
            Some(HWND_TOPMOST),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );
    }
}

pub fn hide_window(window: isize) {
    unsafe {
        let _ = ShowWindow(handle(window), SW_HIDE);
    }
}

pub fn destroy_window(window: isize) {
    unsafe {
        let _ = DestroyWindow(handle(window));
    }
}

pub fn activate_window(window: isize) {
    let _ = try_activate(window);
}

fn try_activate(window: isize) -> bool {
    unsafe {
        let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let current_thread = GetCurrentThreadId();
        let attached = foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
        let activated = SetForegroundWindow(handle(window)).as_bool();
        if attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
        activated
    }
}

pub fn back_in_browser(window: isize) {
    if try_activate(window) {
        send_key_chord(&[VK_MENU, VK_LEFT]);
    }
}

#[must_use]
pub fn window_below(window: isize) -> Option<isize> {
    let own_process = unsafe { GetCurrentProcessId() };
    let mut candidate = unsafe { GetWindow(handle(window), GW_HWNDNEXT) }.ok();
    for _ in 0..WINDOW_WALK_LIMIT {
        let current = candidate.filter(|hwnd| !hwnd.is_invalid())?;
        if is_switch_target(current, own_process) {
            return Some(current.0 as isize);
        }
        candidate = unsafe { GetWindow(current, GW_HWNDNEXT) }.ok();
    }
    None
}

fn is_switch_target(window: HWND, own_process: u32) -> bool {
    let mut process = 0_u32;
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut process)) };
    let ex_style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
    process != own_process
        && unsafe { IsWindowVisible(window) }.as_bool()
        && !unsafe { IsIconic(window) }.as_bool()
        && ex_style & WS_EX_TOOLWINDOW.0 == 0
        && unsafe { GetWindowTextLengthW(window) } > 0
}
