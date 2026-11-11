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
static TRASH_TASK: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

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

fn store_cache(full: bool) {
    if let Ok(mut guard) = TRASH_CACHE.lock() {
        *guard = TrashCache {
            value: Some(full),
            checked_at: Some(Instant::now()),
        };
    }
}

fn spawn_refresh() {
    if TRASH_QUERY_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = thread::Builder::new()
        .name(String::from("otto-trash-query"))
        .spawn(|| {
            store_cache(query_recycle_bin_full());
            TRASH_QUERY_PENDING.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        TRASH_QUERY_PENDING.store(false, Ordering::Release);
    }
}

#[must_use]
pub fn is_recycle_bin_full() -> bool {
    let cache = TRASH_CACHE.lock().map_or(
        TrashCache {
            value: None,
            checked_at: None,
        },
        |guard| *guard,
    );
    let Some(full) = cache.value else {
        let full = query_recycle_bin_full();
        store_cache(full);
        return full;
    };
    if !cache.is_fresh() {
        spawn_refresh();
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
        let _com = ComGuard::single_threaded();
        unsafe {
            let _ = SHEmptyRecycleBinW(None, PCWSTR::null(), 0);
        }
        invalidate_cache();
    }));
}

pub fn shutdown_recycle_bin_task() {
    let handle = TRASH_TASK.lock().ok().and_then(|mut task| task.take());
    if let Some(handle) = handle {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
}

fn invalidate_cache() {
    if let Ok(mut guard) = TRASH_CACHE.lock() {
        guard.checked_at = None;
    }
}
