use std::ffi::c_void;
use std::fs;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::{FocusActivityState, normalize_app_key, normalize_domain};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationValuePattern, TreeScope_Descendants,
    UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_PATHMUSTEXIST, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
};
use windows::core::{PWSTR, w};

use crate::com::ComGuard;
use crate::diagnostics::log_error;
use crate::file::{data_dir, replace_file_atomically};
use crate::worker::join_within;

const FOCUS_ACTIVITY_FILE: &str = "focus_activity.txt";
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_UIA_EDIT_PROBES: i32 = 12;

#[derive(Default)]
struct UiaWorkerQueue {
    pending_hwnd: isize,
    pending_seq: u64,
    last_processed_seq: u64,
}

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static QUEUE: OnceLock<(Mutex<UiaWorkerQueue>, Condvar)> = OnceLock::new();
static WORKER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static RESULT: Mutex<Option<(isize, Option<String>)>> = Mutex::new(None);

fn queue_pair() -> &'static (Mutex<UiaWorkerQueue>, Condvar) {
    QUEUE.get_or_init(|| (Mutex::new(UiaWorkerQueue::default()), Condvar::new()))
}

fn focus_activity_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(FOCUS_ACTIVITY_FILE))
}

#[must_use]
pub fn load_focus_activity() -> FocusActivityState {
    focus_activity_path()
        .map(|path| load_focus_activity_from(&path))
        .unwrap_or_default()
}

#[must_use]
pub fn save_focus_activity(state: &FocusActivityState) -> bool {
    let Some(path) = focus_activity_path() else {
        return false;
    };
    let saved = save_focus_activity_to(&path, state);
    if !saved {
        log_error("focus_activity", "failed to save focus activity");
    }
    saved
}

fn load_focus_activity_from(path: &Path) -> FocusActivityState {
    fs::read_to_string(path)
        .map(|contents| FocusActivityState::decode(&contents))
        .unwrap_or_default()
}

fn save_focus_activity_to(path: &Path, state: &FocusActivityState) -> bool {
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    replace_file_atomically(path, state.encode().as_bytes()).is_ok()
}

#[must_use]
pub fn user_idle_ms() -> u64 {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if unsafe { GetLastInputInfo(&raw mut info) }.as_bool() {
        let now = unsafe { GetTickCount() };
        u64::from(now.wrapping_sub(info.dwTime))
    } else {
        0
    }
}

struct CoTaskStr(PWSTR);

impl Drop for CoTaskStr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CoTaskMemFree(Some(self.0.0.cast::<c_void>()));
            }
        }
    }
}

impl CoTaskStr {
    fn to_string_bounded(&self, max_len: usize) -> Option<String> {
        if self.0.is_null() {
            return None;
        }
        let mut len = 0usize;
        unsafe {
            while len < max_len && *self.0.0.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(self.0.0, len);
            Some(String::from_utf16_lossy(slice))
        }
    }
}

#[must_use]
pub fn pick_blocked_app_executable(owner_hwnd: isize) -> Option<String> {
    let _com = ComGuard::single_threaded();
    let dialog = unsafe {
        CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
    }
    .ok()?;
    let options = unsafe { dialog.GetOptions() }.ok()?;
    let desired = options | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST;
    unsafe { dialog.SetOptions(desired) }.ok()?;
    let filters = [COMDLG_FILTERSPEC {
        pszName: w!("Applications (*.exe)"),
        pszSpec: w!("*.exe"),
    }];
    let _ = unsafe { dialog.SetFileTypes(&filters) };
    let parent_hwnd = (owner_hwnd != 0).then_some(HWND(owner_hwnd as *mut c_void));
    unsafe { dialog.Show(parent_hwnd) }.ok()?;
    let item = unsafe { dialog.GetResult() }.ok()?;
    let raw_name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.ok()?;
    let owned = CoTaskStr(raw_name);
    let path = owned.to_string_bounded(4096)?;
    let key = normalize_app_key(&path);
    (!key.is_empty()).then_some(key)
}

pub fn resume_focus_uia_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
}

pub fn shutdown_focus_uia_task() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    let (_, condvar) = queue_pair();
    condvar.notify_all();
    let handle = WORKER.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(handle) = handle {
        let _ = join_within(handle, SHUTDOWN_TIMEOUT);
    }
}

fn ensure_worker_started() {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    let mut guard = WORKER.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.as_ref().is_some_and(|handle| !handle.is_finished()) {
        return;
    }
    if let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-focus-uia"))
        .spawn(worker_loop)
    {
        *guard = Some(handle);
    }
}

pub fn request_browser_url_inspection(hwnd: isize) {
    if hwnd == 0 || SHUTTING_DOWN.load(Ordering::Acquire) {
        return;
    }
    ensure_worker_started();
    let (mutex, condvar) = queue_pair();
    let mut state = mutex.lock().unwrap_or_else(PoisonError::into_inner);
    state.pending_hwnd = hwnd;
    state.pending_seq = state.pending_seq.wrapping_add(1);
    condvar.notify_one();
}

#[must_use]
pub fn take_browser_url_inspection() -> Option<(isize, Option<String>)> {
    RESULT.lock().unwrap_or_else(PoisonError::into_inner).take()
}

fn worker_loop() {
    let _com = ComGuard::multithreaded();
    let automation: Option<IUIAutomation> =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok();
    let (mutex, condvar) = queue_pair();
    loop {
        let hwnd = {
            let mut state = mutex.lock().unwrap_or_else(PoisonError::into_inner);
            while !SHUTTING_DOWN.load(Ordering::Acquire)
                && state.pending_seq == state.last_processed_seq
            {
                state = condvar.wait(state).unwrap_or_else(PoisonError::into_inner);
            }
            if SHUTTING_DOWN.load(Ordering::Acquire) {
                return;
            }
            state.last_processed_seq = state.pending_seq;
            state.pending_hwnd
        };
        if hwnd == 0 {
            continue;
        }
        let url = automation
            .as_ref()
            .and_then(|uia| read_browser_address_bar(uia, hwnd));
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            return;
        }
        *RESULT.lock().unwrap_or_else(PoisonError::into_inner) = Some((hwnd, url));
        crate::idle_wake::request_idle_wake();
    }
}

fn read_browser_address_bar(automation: &IUIAutomation, hwnd: isize) -> Option<String> {
    let root = unsafe { automation.ElementFromHandle(HWND(hwnd as *mut c_void)) }.ok()?;
    let edit_cond = unsafe {
        automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_EditControlTypeId.0),
        )
    }
    .ok()?;
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &edit_cond) }.ok()?;
    let count = unsafe { elements.Length() }
        .unwrap_or(0)
        .min(MAX_UIA_EDIT_PROBES);
    let mut fallback: Option<String> = None;
    for index in 0..count {
        let Ok(element) = (unsafe { elements.GetElement(index) }) else {
            continue;
        };
        let Ok(value_pattern) = (unsafe {
            element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
        }) else {
            continue;
        };
        let Ok(raw_value) = (unsafe { value_pattern.CurrentValue() }) else {
            continue;
        };
        let value = raw_value.to_string();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let name = unsafe { element.CurrentName() }
            .ok()
            .map(|bstr| bstr.to_string().to_ascii_lowercase())
            .unwrap_or_default();
        let looks_like_address_bar = name.contains("address")
            || name.contains("url")
            || name.contains("search or enter")
            || name.contains("alamat");
        if normalize_domain(trimmed).is_some() {
            if looks_like_address_bar {
                return Some(trimmed.to_owned());
            }
            if fallback.is_none() {
                fallback = Some(trimmed.to_owned());
            }
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::{load_focus_activity_from, save_focus_activity_to, user_idle_ms};
    use otto_core::{DayKey, FocusActivityState, inspect_browser_context};

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("otto-os-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn focus_activity_survives_save_and_load_roundtrip() {
        let dir = scratch_dir("focus-activity-roundtrip");
        let path = dir.join("nested").join("focus_activity.txt");
        let day = DayKey {
            year: 2026,
            month: 11,
            day: 22,
        };
        let mut state = FocusActivityState::default();
        assert!(state.blocklist.add_app("discord"));
        assert!(state.blocklist.add_website("youtube.com"));
        let browser = inspect_browser_context(
            Some("https://www.google.com/search?q=otto+os"),
            "otto os - Google Search - Google Chrome",
        );
        state.observe_foreground(day, "chrome.exe", "Google Chrome", Some(browser));
        state.record_sample(day, 1_000, 0, true, 5, 25);
        state.record_sample(day, 4_000, 0, true, 2, 10);

        assert!(save_focus_activity_to(&path, &state));
        let loaded = load_focus_activity_from(&path);
        assert_eq!(loaded.today, state.today);
        assert_eq!(loaded.blocklist, state.blocklist);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn user_idle_ms_returns_a_bounded_reading() {
        let _ = user_idle_ms();
    }
}
