use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
const JOIN_TIMEOUT: Duration = Duration::from_millis(200);

pub struct FinderDirWatcher {
    stop_flag: Arc<AtomicBool>,
    stop_event: isize,
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
        let stop_event = stop_handle.0 as isize;
        let dir_owned = String::from(dir);
        let worker_dir = dir_owned.clone();
        let worker_flag = Arc::clone(&stop_flag);
        let worker = thread::Builder::new()
            .name(String::from("otto-finder-watch"))
            .spawn(move || {
                watch_loop(&worker_dir, hwnd, stop_event, &worker_flag);
            })
            .ok()?;
        Some(Self {
            stop_flag,
            stop_event,
            dir: dir_owned,
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn matches_dir(&self, dir: &str) -> bool {
        self.dir.eq_ignore_ascii_case(dir)
    }
}

impl Drop for FinderDirWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        let stop = HANDLE(self.stop_event as *mut c_void);
        unsafe {
            let _ = SetEvent(stop);
        }
        if let Some(handle) = self.worker.take() {
            let _ = join_within(handle, JOIN_TIMEOUT);
        }
        unsafe {
            let _ = CloseHandle(stop);
        }
    }
}

fn watch_loop(dir: &str, hwnd: isize, stop_event: isize, stop_flag: &AtomicBool) {
    let wide = HSTRING::from(dir);
    let flags = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE;
    let stop = HANDLE(stop_event as *mut c_void);
    let target = HWND(hwnd as *mut c_void);
    unsafe {
        let Ok(change) = FindFirstChangeNotificationW(PCWSTR(wide.as_ptr()), false, flags) else {
            return;
        };
        if change == INVALID_HANDLE_VALUE {
            return;
        }
        let handles = [stop, change];
        let mut last_post: Option<Instant> = None;
        while !stop_flag.load(Ordering::Acquire) {
            let signaled = WaitForMultipleObjects(&handles, false, INFINITE);
            if signaled.0 == 0 || stop_flag.load(Ordering::Acquire) {
                break;
            }
            if signaled.0 == 1 {
                let should_post = last_post.is_none_or(|prev| prev.elapsed() >= WATCH_DEBOUNCE);
                if should_post {
                    let _ =
                        PostMessageW(Some(target), WM_APP_FINDER_FS_CHANGE, WPARAM(0), LPARAM(0));
                    last_post = Some(Instant::now());
                }
                if FindNextChangeNotification(change).is_err() {
                    break;
                }
            } else {
                break;
            }
        }
        let _ = FindCloseChangeNotification(change);
    }
}
