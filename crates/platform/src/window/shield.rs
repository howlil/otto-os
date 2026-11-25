use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{Rect, ResizeEdge};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS;
use windows::Win32::Graphics::Gdi::{ClientToScreen, ValidateRect};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, VK_LEFT, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GW_HWNDNEXT, GWL_EXSTYLE, GetForegroundWindow, GetSystemMetrics, GetWindow,
    GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, HWND_TOPMOST, IDC_SIZENESW,
    IDC_SIZENS, IDC_SIZEWE, IsIconic, IsWindowVisible, KillTimer, LoadCursorW, PostMessageW,
    SM_CXDRAG, SM_CYDRAG, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetCursor, SetForegroundWindow, SetTimer,
    SetWindowPos, ShowWindow, WS_EX_TOOLWINDOW,
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

#[must_use]
pub fn set_window_timer(window: isize, timer_id: usize, interval_ms: u32) -> bool {
    if window == 0 {
        return false;
    }
    unsafe { SetTimer(Some(handle(window)), timer_id, interval_ms, None) != 0 }
}

pub fn kill_window_timer(window: isize, timer_id: usize) {
    if window != 0 {
        unsafe {
            let _ = KillTimer(Some(handle(window)), timer_id);
        }
    }
}

#[must_use]
pub fn post_window_message(window: isize, message: u32, wparam: usize, lparam: isize) -> bool {
    if window == 0 {
        return false;
    }
    unsafe {
        PostMessageW(
            Some(handle(window)),
            message,
            WPARAM(wparam),
            LPARAM(lparam),
        )
        .is_ok()
    }
}

pub fn assert_window_topmost(window: isize) {
    if window != 0 {
        unsafe {
            let _ = SetWindowPos(
                handle(window),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

pub fn place_no_zorder(window: isize, rect: Rect) {
    if window != 0 {
        unsafe {
            let _ = SetWindowPos(
                handle(window),
                None,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    }
}

pub fn show_window_no_activate(window: isize) {
    if window != 0 {
        unsafe {
            let _ = ShowWindow(handle(window), SW_SHOWNOACTIVATE);
        }
    }
}

pub fn show_and_focus_window(window: isize) {
    if window != 0 {
        let hwnd = handle(window);
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }
    }
}

#[must_use]
pub fn raw_foreground_window() -> isize {
    unsafe { GetForegroundWindow() }.0 as isize
}

pub fn set_foreground_window(window: isize) {
    if window != 0 {
        unsafe {
            let _ = SetForegroundWindow(handle(window));
        }
    }
}

#[must_use]
pub fn track_mouse_leave(window: isize) -> bool {
    if window == 0 {
        return false;
    }
    let mut track = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: handle(window),
        dwHoverTime: 0,
    };
    unsafe { TrackMouseEvent(&raw mut track) }.is_ok()
}

pub fn capture_mouse(window: isize) {
    if window != 0 {
        unsafe {
            let _ = SetCapture(handle(window));
        }
    }
}

pub fn release_mouse_capture() {
    unsafe {
        let _ = ReleaseCapture();
    }
}

#[must_use]
pub fn client_to_screen(window: isize, x: i32, y: i32) -> (i32, i32) {
    let mut point = POINT { x, y };
    if window != 0 {
        unsafe {
            let _ = ClientToScreen(handle(window), &raw mut point);
        }
    }
    (point.x, point.y)
}

#[must_use]
pub fn drag_thresholds() -> (i32, i32) {
    (
        unsafe { GetSystemMetrics(SM_CXDRAG) }.max(4),
        unsafe { GetSystemMetrics(SM_CYDRAG) }.max(4),
    )
}

#[must_use]
pub fn is_virtual_key_down(vk: i32) -> bool {
    (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
}

#[must_use]
pub fn set_resize_cursor(edge: ResizeEdge) -> bool {
    let id = match edge {
        ResizeEdge::Top => IDC_SIZENS,
        ResizeEdge::TopRight => IDC_SIZENESW,
        ResizeEdge::Right => IDC_SIZEWE,
    };
    unsafe {
        let Ok(cursor) = LoadCursorW(None, id) else {
            return false;
        };
        let _ = SetCursor(Some(cursor));
    }
    true
}

pub fn validate_window_rect(window: isize) {
    if window != 0 {
        unsafe {
            let _ = ValidateRect(Some(handle(window)), None);
        }
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
