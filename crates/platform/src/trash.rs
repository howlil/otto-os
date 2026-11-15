use std::mem::size_of;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, SHEmptyRecycleBinW, SHFILEOPSTRUCTW,
    SHFileOperationW, SHQUERYRBINFO, SHQueryRecycleBinW,
};
use windows::core::{BOOL, PCWSTR};

use crate::com::ComGuard;
use crate::worker::join_within;

const TRASH_CACHE_TTL: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
struct TrashCache {
    value: Option<bool>,
    checked_at: Option<Instant>,
}

impl TrashCache {
    fn is_fresh(self) -> bool {
        self.checked_at
            .is_some_and(|checked_at| checked_at.elapsed() < TRASH_CACHE_TTL)
    }
}

static TRASH_CACHE: Mutex<TrashCache> = Mutex::new(TrashCache {
    value: None,
    checked_at: None,
});
static TRASH_QUERY_PENDING: AtomicBool = AtomicBool::new(false);
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static TRASH_QUERY_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static TRASH_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

struct TrashQueryGuard;

impl Drop for TrashQueryGuard {
    fn drop(&mut self) {
        TRASH_QUERY_PENDING.store(false, Ordering::Release);
    }
}

pub(crate) fn resume_recycle_bin_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

#[must_use]
pub fn recycle_path(path: &str) -> bool {
    let mut wide: Vec<u16> = path.encode_utf16().collect();
    wide.push(0);
    wide.push(0);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: HWND::default(),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(wide.as_ptr()),
        pTo: PCWSTR::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT).0 as u16,
        fAnyOperationsAborted: BOOL(0),
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: PCWSTR::null(),
    };
    let res = unsafe { SHFileOperationW(&raw mut op) };
    invalidate_cache();
    res == 0 && !op.fAnyOperationsAborted.as_bool()
}

fn query_recycle_bin_full() -> bool {
    let mut info = SHQUERYRBINFO {
        cbSize: size_of::<SHQUERYRBINFO>() as u32,
        ..SHQUERYRBINFO::default()
    };
    let queried = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &raw mut info).is_ok() };
    queried && (info.i64NumItems > 0 || info.i64Size > 0)
}

fn store_cache(full: bool) -> bool {
    if let Ok(mut guard) = TRASH_CACHE.lock() {
        let changed = guard.value != Some(full);
        *guard = TrashCache {
            value: Some(full),
            checked_at: Some(Instant::now()),
        };
        return changed;
    }
    false
}

fn spawn_refresh() {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    if TRASH_QUERY_PENDING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let query_guard = TrashQueryGuard;
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    let Ok(mut task_slot) = TRASH_QUERY_TASK.lock() else {
        return;
    };
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    if let Some(prev) = task_slot.take() {
        if prev.is_finished() {
            let _ = prev.join();
        } else {
            *task_slot = Some(prev);
            return;
        }
    }
    let spawned = thread::Builder::new()
        .name(String::from("otto-trash-query"))
        .spawn(move || {
            let _query_guard = query_guard;
            let _com = ComGuard::single_threaded();
            if SHUTTING_DOWN.load(Ordering::Acquire) {
                return;
            }
            let full = query_recycle_bin_full();
            if !SHUTTING_DOWN.load(Ordering::Acquire) && store_cache(full) {
                crate::idle_wake::request_idle_wake();
            }
        });
    if let Ok(handle) = spawned {
        *task_slot = Some(handle);
    }
}

#[must_use]
pub fn is_recycle_bin_full() -> bool {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return false;
    }
    let cache = TRASH_CACHE.lock().map_or(
        TrashCache {
            value: None,
            checked_at: None,
        },
        |guard| *guard,
    );
    if !cache.is_fresh() {
        spawn_refresh();
    }
    cache.value.unwrap_or(false)
}

pub fn empty_recycle_bin() {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    invalidate_cache();
    let Ok(mut task) = TRASH_TASK.lock() else {
        return;
    };
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    if let Some(handle) = task.take() {
        if !handle.is_finished() {
            *task = Some(handle);
            return;
        }
        let _ = handle.join();
    }
    if let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-trash-empty"))
        .spawn(|| {
            let _com = ComGuard::single_threaded();
            if SHUTTING_DOWN.load(Ordering::Acquire) {
                return;
            }
            unsafe {
                let _ = SHEmptyRecycleBinW(None, PCWSTR::null(), 0);
            }
            if !SHUTTING_DOWN.load(Ordering::Acquire) {
                invalidate_cache();
            }
        })
    {
        *task = Some(handle);
    }
}

pub fn shutdown_recycle_bin_task() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    if let Some(handle) = TRASH_QUERY_TASK
        .lock()
        .ok()
        .and_then(|mut task| task.take())
    {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
    if let Some(handle) = TRASH_TASK.lock().ok().and_then(|mut task| task.take()) {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
}

fn invalidate_cache() {
    if let Ok(mut guard) = TRASH_CACHE.lock() {
        guard.checked_at = None;
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::atomic::Ordering;
    use std::sync::{Mutex, PoisonError};

    use super::{
        SHUTTING_DOWN, TRASH_QUERY_PENDING, TRASH_QUERY_TASK, TrashQueryGuard, is_recycle_bin_full,
        resume_recycle_bin_task, shutdown_recycle_bin_task,
    };

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    struct ResumeTrashGuard;

    impl Drop for ResumeTrashGuard {
        fn drop(&mut self) {
            shutdown_recycle_bin_task();
            resume_recycle_bin_task();
        }
    }

    #[test]
    fn trash_query_guard_resets_pending_flag_on_unwind() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        TRASH_QUERY_PENDING.store(true, Ordering::Release);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _guard = TrashQueryGuard;
            panic!("simulated panic during recycle bin query");
        }));
        assert!(outcome.is_err());
        assert!(!TRASH_QUERY_PENDING.load(Ordering::Acquire));
    }

    #[test]
    fn shutdown_prevents_trash_query_spawn_until_resumed() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let _resume = ResumeTrashGuard;
        shutdown_recycle_bin_task();
        assert!(SHUTTING_DOWN.load(Ordering::Acquire));
        assert!(!is_recycle_bin_full());
        assert!(TRASH_QUERY_TASK.lock().unwrap().is_none());
    }
}
