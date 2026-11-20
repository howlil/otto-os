use std::ffi::c_void;

use otto_core::{
    AppIdentity, AppInfo, Capability, CommandOutcome, WindowCommand, WindowIdentity, WindowInfo,
    WindowPresence,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOP, IsWindow, IsWindowVisible, PostMessageW, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE,
    SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow, WM_CLOSE,
};

use crate::dock::focus_window_hwnd;
use crate::window::hidden::{self, Concealment};

use super::context::live_window;
use super::context::post_outcome;
use super::context::post_system_command;
use super::context::tile;
use super::context::toggle_full_screen;
use super::context::unhide;
use super::inspect::enumerate_windows;
use super::inspect::owner_ids;
use super::process::ProcessHandle;
use super::process::process_generation;

#[must_use]
pub fn window_command_capability(
    target: WindowIdentity,
    command: WindowCommand,
    own_process_id: u32,
) -> Option<Capability> {
    live_window(target, own_process_id).map(|window| window.capability(command))
}

#[must_use]
pub fn execute_window_command(
    target: WindowIdentity,
    command: WindowCommand,
    own_process_id: u32,
) -> CommandOutcome {
    let Some(window) = live_window(target, own_process_id) else {
        return CommandOutcome::TargetGone;
    };
    if let Some(blocked) = CommandOutcome::from_capability(window.capability(command)) {
        return blocked;
    }
    let hwnd = HWND(target.hwnd as *mut c_void);
    match command {
        WindowCommand::Focus => {
            unhide(&window);
            focus_window_hwnd(target.hwnd);
            CommandOutcome::Completed
        }
        WindowCommand::Close => post_close(hwnd),
        WindowCommand::Minimize => post_system_command(hwnd, SC_MINIMIZE),
        WindowCommand::Zoom => post_system_command(
            hwnd,
            if window.zoomed {
                SC_RESTORE
            } else {
                SC_MAXIMIZE
            },
        ),
        WindowCommand::ToggleFullScreen => toggle_full_screen(hwnd),
        WindowCommand::TileLeft => tile(hwnd, true),
        WindowCommand::TileRight => tile(hwnd, false),
    }
}

pub(super) fn summarize(outcomes: impl IntoIterator<Item = CommandOutcome>) -> CommandOutcome {
    outcomes
        .into_iter()
        .max_by_key(|outcome| outcome_priority(*outcome))
        .unwrap_or(CommandOutcome::TargetGone)
}

pub(super) const fn outcome_priority(outcome: CommandOutcome) -> u8 {
    match outcome {
        CommandOutcome::Completed => 6,
        CommandOutcome::Denied => 5,
        CommandOutcome::TemporarilyUnavailable => 4,
        CommandOutcome::TimedOut => 3,
        CommandOutcome::Cancelled => 2,
        CommandOutcome::Unsupported => 1,
        CommandOutcome::TargetGone => 0,
    }
}

pub(super) fn hide_window(window: &WindowInfo) -> CommandOutcome {
    if window.traits.elevated {
        return CommandOutcome::Denied;
    }
    let hwnd = HWND(window.identity.hwnd as *mut std::ffi::c_void);
    if !hidden::record(window.identity, Concealment::Hidden) {
        return CommandOutcome::TemporarilyUnavailable;
    }
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        hidden::forget(window.identity.hwnd, Concealment::Hidden);
        return CommandOutcome::Denied;
    }
    CommandOutcome::Completed
}

pub(super) fn hide_windows<'a>(
    windows: impl Iterator<Item = &'a WindowInfo>,
    own_process_id: u32,
) -> CommandOutcome {
    summarize(
        windows
            .filter(|window| window.presence != WindowPresence::Hidden)
            .filter_map(|stale| live_window(stale.identity, own_process_id))
            .map(|live| hide_window(&live)),
    )
}

#[must_use]
pub fn hide_application(app: &AppInfo, own_process_id: u32) -> CommandOutcome {
    hide_windows(app.windows.iter(), own_process_id)
}

#[must_use]
pub fn hide_other_applications(keep: &AppIdentity, own_process_id: u32) -> CommandOutcome {
    let windows = enumerate_windows(own_process_id);
    hide_windows(
        windows.iter().filter(|window| &window.app != keep),
        own_process_id,
    )
}

#[must_use]
pub fn show_application(app: &AppInfo, own_process_id: u32) -> CommandOutcome {
    let live: Vec<WindowInfo> = app
        .windows
        .iter()
        .filter_map(|stale| live_window(stale.identity, own_process_id))
        .collect();
    for window in &live {
        unhide(window);
    }
    let target = live
        .iter()
        .find(|window| window.is_primary() && window.presence == WindowPresence::Visible)
        .or_else(|| live.iter().find(|window| window.is_primary()))
        .or_else(|| live.first());
    if let Some(target) = target {
        for window in live.iter().rev() {
            if window.is_primary()
                && window.presence == WindowPresence::Visible
                && window.identity.hwnd != target.identity.hwnd
            {
                let hwnd = HWND(window.identity.hwnd as *mut c_void);
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNA);
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
        }
        focus_window_hwnd(target.identity.hwnd);
    }
    summarize(live.iter().map(|_| CommandOutcome::Completed))
}

#[must_use]
pub fn quit_application(app: &AppInfo, own_process_id: u32) -> CommandOutcome {
    summarize(
        app.primary_windows()
            .filter_map(|stale| live_window(stale.identity, own_process_id))
            .map(|live| close_window(&live)),
    )
}

pub(super) fn close_window(window: &WindowInfo) -> CommandOutcome {
    if let Some(blocked) = CommandOutcome::from_capability(window.capability(WindowCommand::Close))
    {
        return blocked;
    }
    unhide(window);
    post_close(HWND(window.identity.hwnd as *mut c_void))
}

pub(super) fn post_close(hwnd: HWND) -> CommandOutcome {
    post_outcome(unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) })
}

pub(crate) fn identity_is_live(identity: WindowIdentity) -> bool {
    let hwnd = HWND(identity.hwnd as *mut c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return false;
    }
    let (process_id, thread_id) = owner_ids(hwnd);
    process_id == identity.process_id
        && thread_id == identity.thread_id
        && ProcessHandle::open(process_id)
            .and_then(|process| process_generation(&process))
            .is_some_and(|generation| generation == identity.generation)
}

pub(crate) fn current_identity(hwnd: isize) -> Option<WindowIdentity> {
    let handle = HWND(hwnd as *mut c_void);
    if !unsafe { IsWindow(Some(handle)) }.as_bool() {
        return None;
    }
    let (process_id, thread_id) = owner_ids(handle);
    let generation =
        ProcessHandle::open(process_id).and_then(|process| process_generation(&process))?;
    Some(WindowIdentity {
        hwnd,
        process_id,
        thread_id,
        generation,
    })
}

#[must_use]
pub fn window_identity(hwnd: isize) -> Option<WindowIdentity> {
    current_identity(hwnd)
}
