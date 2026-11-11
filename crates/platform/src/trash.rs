use std::mem::size_of;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::UI::Shell::{SHEmptyRecycleBinW, SHQUERYRBINFO, SHQueryRecycleBinW};
use windows::core::PCWSTR;

static TRASH_CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
static TRASH_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[must_use]
pub fn is_recycle_bin_full() -> bool {
    let now = Instant::now();
    if let Ok(guard) = TRASH_CACHE.lock()
        && let Some((checked_at, full)) = *guard
        && now.duration_since(checked_at) < Duration::from_secs(2)
    {
        return full;
    }
    let mut info = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        ..SHQUERYRBINFO::default()
    };
    let full = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &raw mut info).is_ok() }
        && (info.i64NumItems > 0 || info.i64Size > 0);
    if let Ok(mut guard) = TRASH_CACHE.lock() {
        *guard = Some((now, full));
    }
    full
}

pub fn empty_recycle_bin() {
    invalidate_cache();
    let Ok(mut task) = TRASH_TASK.lock() else {
        return;
    };
    if let Some(handle) = task.take() {
        if !handle.is_finished() {
            *task = Some(handle);
            return;
        }
        let _ = handle.join();
    }
    *task = Some(thread::spawn(|| {
        unsafe {
            let _ = SHEmptyRecycleBinW(None, PCWSTR::null(), 0);
        }
        invalidate_cache();
    }));
}

pub fn shutdown_recycle_bin_task() {
    let handle = TRASH_TASK.lock().ok().and_then(|mut task| task.take());
    if let Some(handle) = handle {
        let _ = handle.join();
    }
}

fn invalidate_cache() {
    if let Ok(mut guard) = TRASH_CACHE.lock() {
        *guard = None;
    }
}
