use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, SHEmptyRecycleBinW, SHFILEOPSTRUCTW,
    SHFileOperationW,
};
use windows::core::{BOOL, HSTRING, PCWSTR};

use crate::com::ComGuard;
use crate::worker::join_within;

const TRASH_CACHE_TTL: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const DRIVE_FIXED: u32 = 3;

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

pub fn resume_recycle_bin_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

#[must_use]
pub(crate) fn recycle_path(path: &str) -> bool {
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
    let drives = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|bit| drives & (1 << bit) != 0)
        .map(|bit| format!("{}:\\", char::from(b'A' + bit)))
        .filter(|root| unsafe { GetDriveTypeW(&HSTRING::from(root.as_str())) } == DRIVE_FIXED)
        .any(|root| drive_bin_has_items(Path::new(&root)))
}

fn drive_bin_has_items(root: &Path) -> bool {
    fs::read_dir(root.join("$Recycle.Bin")).is_ok_and(|owners| {
        owners
            .flatten()
            .any(|owner| owner_bin_has_items(&owner.path()))
    })
}

fn owner_bin_has_items(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| {
        entries.any(|entry| {
            entry.is_ok_and(|entry| !entry.file_name().eq_ignore_ascii_case("desktop.ini"))
        })
    })
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
        owner_bin_has_items, resume_recycle_bin_task, shutdown_recycle_bin_task,
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
    fn an_owner_bin_with_only_desktop_ini_is_empty_and_any_other_entry_makes_it_full() {
        let dir = std::env::temp_dir().join(format!("otto-trash-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create probe dir");
        assert!(!owner_bin_has_items(&dir.join("missing")));
        assert!(!owner_bin_has_items(&dir));
        std::fs::write(dir.join("desktop.ini"), b"").expect("write desktop.ini");
        assert!(!owner_bin_has_items(&dir));
        std::fs::write(dir.join("$I1234.txt"), b"x").expect("write item");
        assert!(owner_bin_has_items(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "reads the real Recycle Bin and waits on the slow shell count"]
    fn the_probe_agrees_with_the_shell_and_is_far_faster() {
        use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
        use windows::core::PCWSTR;

        let started = std::time::Instant::now();
        let probed = super::query_recycle_bin_full();
        let probe_time = started.elapsed();
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..SHQUERYRBINFO::default()
        };
        let queried = unsafe { SHQueryRecycleBinW(PCWSTR::null(), &raw mut info) }.is_ok();
        assert!(queried);
        assert_eq!(probed, info.i64NumItems > 0 || info.i64Size > 0);
        assert!(
            probe_time < std::time::Duration::from_millis(250),
            "{probe_time:?}"
        );
        println!("probe {probe_time:?}, shell items {}", info.i64NumItems);
    }

    #[test]
    fn shutdown_prevents_trash_query_spawn_until_resumed() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let _resume = ResumeTrashGuard;
        shutdown_recycle_bin_task();
        assert!(SHUTTING_DOWN.load(Ordering::Acquire));
        assert!(!is_recycle_bin_full());
        assert!(TRASH_QUERY_TASK.lock().unwrap().is_none());
        resume_recycle_bin_task();
        assert!(!SHUTTING_DOWN.load(Ordering::Acquire));
    }
}
