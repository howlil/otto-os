use std::ffi::c_void;
use std::sync::{Mutex, PoisonError};

use otto_core::WindowIdentity;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DEFAULT, DWMWCP_ROUND,
    DwmGetWindowAttribute, DwmSetWindowAttribute,
};

use crate::hidden_windows::{self, Concealment};
use crate::window_model::{current_identity, identity_is_live, owner_ids};

const MAX_SEEN: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Seen {
    hwnd: isize,
    process_id: u32,
    thread_id: u32,
    applied: bool,
}

static SEEN: Mutex<Vec<Seen>> = Mutex::new(Vec::new());

fn handle(hwnd: isize) -> HWND {
    HWND(hwnd as *mut c_void)
}

pub(crate) fn preference(hwnd: isize) -> Option<DWM_WINDOW_CORNER_PREFERENCE> {
    let mut value = DWMWCP_DEFAULT;
    unsafe {
        DwmGetWindowAttribute(
            handle(hwnd),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw mut value).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    }
    .ok()?;
    Some(value)
}

pub(crate) fn set_preference(hwnd: isize, value: DWM_WINDOW_CORNER_PREFERENCE) -> bool {
    unsafe {
        DwmSetWindowAttribute(
            handle(hwnd),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const value).cast(),
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        )
    }
    .is_ok()
}

fn remembered(hwnd: isize, process_id: u32, thread_id: u32) -> Option<bool> {
    SEEN.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|seen| {
            seen.hwnd == hwnd && seen.process_id == process_id && seen.thread_id == thread_id
        })
        .map(|seen| seen.applied)
}

fn remember(entry: Seen) {
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    seen.retain(|known| known.hwnd != entry.hwnd);
    if seen.len() >= MAX_SEEN {
        seen.remove(0);
    }
    seen.push(entry);
}

fn record_with_pruning(identity: WindowIdentity) -> bool {
    if hidden_windows::record(identity, Concealment::RoundedCorners) {
        return true;
    }
    hidden_windows::retain_kind(Concealment::RoundedCorners, identity_is_live);
    hidden_windows::record(identity, Concealment::RoundedCorners)
}

fn apply(hwnd: isize) -> Option<bool> {
    let identity = current_identity(hwnd)?;
    if preference(hwnd) != Some(DWMWCP_DEFAULT) {
        return Some(false);
    }
    if !record_with_pruning(identity) {
        return None;
    }
    if set_preference(hwnd, DWMWCP_ROUND) {
        return Some(true);
    }
    hidden_windows::forget(hwnd, Concealment::RoundedCorners);
    Some(false)
}

#[must_use]
pub fn round_window_corners(hwnd: isize) -> bool {
    let (process_id, thread_id) = owner_ids(handle(hwnd));
    if let Some(applied) = remembered(hwnd, process_id, thread_id) {
        return applied;
    }
    let Some(applied) = apply(hwnd) else {
        return false;
    };
    remember(Seen {
        hwnd,
        process_id,
        thread_id,
        applied,
    });
    applied
}

pub(crate) fn reset(hwnd: isize) -> bool {
    set_preference(hwnd, DWMWCP_DEFAULT)
}

pub fn restore_rounded_corners() -> usize {
    SEEN.lock().unwrap_or_else(PoisonError::into_inner).clear();
    hidden_windows::take_kind(Concealment::RoundedCorners)
        .into_iter()
        .filter(|entry| identity_is_live(entry.identity))
        .filter(|entry| reset(entry.identity.hwnd))
        .count()
}
