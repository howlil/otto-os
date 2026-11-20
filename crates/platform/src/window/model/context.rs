use std::ffi::c_void;

use otto_core::{AppInfo, CommandOutcome, Rect, WindowIdentity, WindowInfo, WindowPresence};
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_F11;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, IsIconic, IsWindow, IsZoomed, PostMessageW, SC_RESTORE, SMTO_ABORTIFHUNG,
    SW_SHOWNA, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageTimeoutW, SetWindowPos,
    ShowWindow, WM_SYSCOMMAND,
};

use crate::dock::focus_window_hwnd;
use crate::window::hidden::{self, Concealment};
use crate::window::ops::visible_window_rect;

use super::inspect::enumerate_windows;
use super::inspect::foreground_root;
use super::inspect::inspect_window;
use super::inspect::monitor_rects;
use super::process::RESTORE_TIMEOUT_MS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationContext {
    pub app: AppInfo,
    pub other_apps_visible: bool,
}

#[must_use]
pub fn application_of(target: WindowIdentity, own_process_id: u32) -> Option<ApplicationContext> {
    let windows = enumerate_windows(own_process_id);
    let identity = windows
        .iter()
        .find(|window| window.identity.is_same_window(&target))?
        .app
        .clone();
    let apps = otto_core::group_windows(windows);
    let other_apps_visible = apps.iter().any(|app| {
        app.identity != identity
            && app
                .primary_windows()
                .any(|window| window.presence == WindowPresence::Visible)
    });
    let app = apps.into_iter().find(|app| app.identity == identity)?;
    Some(ApplicationContext {
        app,
        other_apps_visible,
    })
}

pub(super) fn live_window(target: WindowIdentity, own_process_id: u32) -> Option<WindowInfo> {
    let hwnd = HWND(target.hwnd as *mut c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return None;
    }
    inspect_window(target.hwnd, own_process_id)
        .filter(|window| window.identity.is_same_window(&target))
}

pub(super) fn post_outcome(posted: windows::core::Result<()>) -> CommandOutcome {
    match posted {
        Ok(()) => CommandOutcome::Completed,
        Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => CommandOutcome::Denied,
        Err(_) => CommandOutcome::TargetGone,
    }
}

pub(super) fn post_system_command(hwnd: HWND, command: u32) -> CommandOutcome {
    post_outcome(unsafe {
        PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM(command as usize),
            LPARAM(0),
        )
    })
}

pub(super) fn unhide(window: &WindowInfo) {
    if window.presence == WindowPresence::Hidden {
        unsafe {
            let _ = ShowWindow(
                HWND(window.identity.hwnd as *mut std::ffi::c_void),
                SW_SHOWNA,
            );
        }
        hidden::forget(window.identity.hwnd, Concealment::Hidden);
    }
}

pub(super) fn tile(hwnd: HWND, left_half: bool) -> CommandOutcome {
    let Some((_, work)) = monitor_rects(hwnd) else {
        return CommandOutcome::TargetGone;
    };
    if unsafe { IsZoomed(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
        unsafe {
            let _ = SendMessageTimeoutW(
                hwnd,
                WM_SYSCOMMAND,
                WPARAM(SC_RESTORE as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                RESTORE_TIMEOUT_MS,
                None,
            );
        }
    }
    let mut outer = RECT::default();
    if unsafe { GetWindowRect(hwnd, &raw mut outer) }.is_err() {
        return CommandOutcome::TargetGone;
    }
    let frame = visible_window_rect(hwnd);
    let half = tile_rect(work, left_half);
    let target = Rect {
        x: half.x - (frame.left - outer.left),
        y: half.y - (frame.top - outer.top),
        width: half.width + (frame.left - outer.left) + (outer.right - frame.right),
        height: half.height + (frame.top - outer.top) + (outer.bottom - frame.bottom),
    };
    post_outcome(unsafe {
        SetWindowPos(
            hwnd,
            None,
            target.x,
            target.y,
            target.width,
            target.height,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        )
    })
}

pub(super) const fn tile_rect(work: RECT, left_half: bool) -> Rect {
    let width = work.right - work.left;
    let half = width / 2;
    Rect {
        x: if left_half {
            work.left
        } else {
            work.left + half
        },
        y: work.top,
        width: if left_half { half } else { width - half },
        height: work.bottom - work.top,
    }
}

pub(super) fn toggle_full_screen(hwnd: HWND) -> CommandOutcome {
    focus_window_hwnd(hwnd.0 as isize);
    if foreground_root() != hwnd.0 as isize {
        return CommandOutcome::TimedOut;
    }
    crate::actions::send_key_chord(&[VK_F11]);
    CommandOutcome::Completed
}
