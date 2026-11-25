use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WPARAM};
use windows::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
    FILE_NOTIFY_CHANGE_SIZE, FindCloseChangeNotification, FindFirstChangeNotificationW,
    FindNextChangeNotification,
};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows::core::{HSTRING, PCWSTR};

use crate::worker::join_within;

pub const WM_APP_FINDER_FS_CHANGE: u32 = 0x8000 + 74;

const WATCH_DEBOUNCE: Duration = Duration::from_millis(160);
const JOIN_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_RETIRED_WATCHERS: usize = 8;
const WAIT_TIMEOUT_CODE: u32 = 258;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;

static NEXT_WATCHER_GEN: AtomicUsize = AtomicUsize::new(1);
static RETIRED_WATCHERS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

struct OwnedEvent(HANDLE);

unsafe impl Send for OwnedEvent {}
unsafe impl Sync for OwnedEvent {}

impl Drop for OwnedEvent {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[derive(Debug, Default)]
pub(super) struct WatcherDebounce {
    last_post: Option<Instant>,
    pending_deadline: Option<Instant>,
}

impl WatcherDebounce {
    pub(super) fn on_event(&mut self, now: Instant) -> bool {
        if self
            .last_post
            .is_none_or(|prev| now.saturating_duration_since(prev) >= WATCH_DEBOUNCE)
        {
            self.last_post = Some(now);
            self.pending_deadline = None;
            true
        } else {
            if self.pending_deadline.is_none()
                && let Some(prev) = self.last_post
            {
                self.pending_deadline = Some(prev + WATCH_DEBOUNCE);
            }
            false
        }
    }

    pub(super) fn next_wait_ms(&self, now: Instant) -> u32 {
        self.pending_deadline.map_or(INFINITE, |deadline| {
            let remaining = deadline.saturating_duration_since(now);
            u32::try_from(remaining.as_millis())
                .unwrap_or(u32::MAX)
                .min(INFINITE - 1)
        })
    }

    pub(super) fn on_timeout(&mut self, now: Instant) -> bool {
        if self
            .pending_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            self.last_post = Some(now);
            self.pending_deadline = None;
            true
        } else {
            false
        }
    }
}

pub struct FinderDirWatcher {
    stop_flag: Arc<AtomicBool>,
    stop_event: Arc<OwnedEvent>,
    generation: usize,
    dir: String,
    worker: Option<JoinHandle<()>>,
}

impl FinderDirWatcher {
    #[must_use]
    pub fn start(dir: &str, hwnd: isize) -> Option<Self> {
        if dir.is_empty() || hwnd == 0 {
            return None;
        }
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_handle = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }.ok()?;
        let stop_event = Arc::new(OwnedEvent(stop_handle));
        let generation = NEXT_WATCHER_GEN.fetch_add(1, Ordering::Relaxed);
        let dir_owned = String::from(dir);
        let worker_dir = dir_owned.clone();
        let worker_flag = Arc::clone(&stop_flag);
        let worker_event = Arc::clone(&stop_event);
        let worker = thread::Builder::new()
            .name(String::from("otto-finder-watch"))
            .spawn(move || {
                watch_loop(&worker_dir, hwnd, generation, &worker_event, &worker_flag);
            })
            .ok()?;
        Some(Self {
            stop_flag,
            stop_event,
            generation,
            dir: dir_owned,
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn matches_dir(&self, dir: &str) -> bool {
        self.dir.eq_ignore_ascii_case(dir)
    }

    #[must_use]
    pub const fn generation(&self) -> usize {
        self.generation
    }
}

impl Drop for FinderDirWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        unsafe {
            let _ = SetEvent(self.stop_event.0);
        }
        if let Some(handle) = self.worker.take() {
            if handle.is_finished() {
                let _ = handle.join();
                return;
            }
            if let Ok(mut retired) = RETIRED_WATCHERS.lock() {
                let mut i = 0;
                while i < retired.len() {
                    if retired[i].is_finished() {
                        let done = retired.remove(i);
                        let _ = done.join();
                    } else {
                        i += 1;
                    }
                }
                if retired.len() >= MAX_RETIRED_WATCHERS {
                    let oldest = retired.remove(0);
                    drop(retired);
                    let _ = join_within(oldest, JOIN_TIMEOUT);
                    if let Ok(mut retired) = RETIRED_WATCHERS.lock() {
                        retired.push(handle);
                    }
                } else {
                    retired.push(handle);
                }
            }
        }
    }
}

fn post_change(target: HWND, generation: usize) -> bool {
    match unsafe {
        PostMessageW(
            Some(target),
            WM_APP_FINDER_FS_CHANGE,
            WPARAM(generation),
            LPARAM(0),
        )
    } {
        Ok(()) => true,
        Err(err) => {
            let code = (err.code().0 as u32) & 0xFFFF;
            code != ERROR_INVALID_WINDOW_HANDLE
        }
    }
}

fn watch_loop(
    dir: &str,
    hwnd: isize,
    generation: usize,
    stop_event: &OwnedEvent,
    stop_flag: &AtomicBool,
) {
    let wide = HSTRING::from(dir);
    let flags = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE;
    let target = HWND(hwnd as *mut c_void);
    unsafe {
        let Ok(change) = FindFirstChangeNotificationW(PCWSTR(wide.as_ptr()), false, flags) else {
            return;
        };
        if change == INVALID_HANDLE_VALUE {
            return;
        }
        let handles = [stop_event.0, change];
        let mut debounce = WatcherDebounce::default();
        while !stop_flag.load(Ordering::Acquire) {
            let wait_ms = debounce.next_wait_ms(Instant::now());
            let signaled = WaitForMultipleObjects(&handles, false, wait_ms);
            if signaled.0 == 0 || stop_flag.load(Ordering::Acquire) {
                break;
            }
            if signaled.0 == 1 {
                if debounce.on_event(Instant::now()) && !post_change(target, generation) {
                    break;
                }
                if FindNextChangeNotification(change).is_err() {
                    break;
                }
            } else if signaled.0 == WAIT_TIMEOUT_CODE {
                if debounce.on_timeout(Instant::now()) && !post_change(target, generation) {
                    break;
                }
            } else {
                break;
            }
        }
        let _ = FindCloseChangeNotification(change);
    }
}
