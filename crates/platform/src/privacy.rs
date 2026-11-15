use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use otto_core::{MicState, PrivacyState};
use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET,
    REG_NOTIFY_CHANGE_NAME, RegCloseKey, RegEnumKeyExW, RegNotifyChangeKeyValue, RegOpenKeyExW,
    RegQueryValueExW,
};
use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

use crate::worker::join_within;

const CONSENT_MICROPHONE_PATH: PCWSTR = w!(
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone"
);
const CONSENT_WEBCAM_PATH: PCWSTR = w!(
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\webcam"
);
const READY_TIMEOUT: Duration = Duration::from_millis(250);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);

static CAMERA_ACTIVE: AtomicBool = AtomicBool::new(false);
static LOADED: AtomicBool = AtomicBool::new(false);
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static CONSENT_APP_NAME: Mutex<Option<String>> = Mutex::new(None);
static RUNTIME: Mutex<Option<PrivacyRuntime>> = Mutex::new(None);

struct OwnedHandle(HANDLE);

unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    fn as_handle(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() && !self.0.0.is_null() {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

struct OwnedHkey(HKEY);

impl Drop for OwnedHkey {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { RegCloseKey(self.0) };
        }
    }
}

struct PrivacyRuntime {
    handle: JoinHandle<()>,
    shutdown_event: Arc<OwnedHandle>,
}

struct RegWatch {
    hkey: OwnedHkey,
    event: OwnedHandle,
}

impl RegWatch {
    fn open(subkey: PCWSTR) -> Option<Self> {
        let mut hkey = HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey,
                None,
                KEY_READ | KEY_NOTIFY,
                &mut hkey,
            )
        }
        .is_err()
        {
            return None;
        }
        let hkey = OwnedHkey(hkey);
        let event = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.ok()?;
        Some(Self {
            hkey,
            event: OwnedHandle(event),
        })
    }

    fn arm(&self) -> bool {
        unsafe {
            RegNotifyChangeKeyValue(
                self.hkey.0,
                true,
                REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                Some(self.event.0),
                true,
            )
        }
        .is_ok()
    }
}

#[must_use]
pub(crate) fn is_consent_entry_active(start: u64, stop: Option<u64>) -> bool {
    start > 0 && stop == Some(0)
}

#[must_use]
pub(crate) fn format_consent_app_name(raw_key: &str, packaged: bool) -> String {
    if !packaged {
        let exe = raw_key
            .rsplit(['#', '\\', '/'])
            .next()
            .unwrap_or(raw_key)
            .trim();
        return strip_exe_suffix(exe).to_owned();
    }
    let family = raw_key
        .split('!')
        .next()
        .unwrap_or(raw_key)
        .split('_')
        .next()
        .unwrap_or(raw_key);
    family
        .rsplit('.')
        .next()
        .unwrap_or(family)
        .trim()
        .to_owned()
}

fn strip_exe_suffix(name: &str) -> &str {
    let bytes = name.as_bytes();
    if bytes.len() >= 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".exe") {
        &name[..name.len() - 4]
    } else {
        name
    }
}

fn extract_nonpackaged_exe_lower(raw_key: &str) -> Option<String> {
    let exe = raw_key.rsplit(['#', '\\', '/']).next()?.trim();
    (!exe.is_empty()).then(|| exe.to_ascii_lowercase())
}

fn query_qword(hkey: HKEY, name: PCWSTR) -> Option<u64> {
    let mut value = 0u64;
    let mut size = size_of::<u64>() as u32;
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            name,
            None,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    (status.is_ok() && size as usize == size_of::<u64>()).then_some(value)
}

fn enum_subkeys(hkey: HKEY) -> Vec<String> {
    let mut result = Vec::new();
    let mut buf = [0u16; 512];
    let mut index = 0u32;
    loop {
        let mut len = buf.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                hkey,
                index,
                Some(PWSTR(buf.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status.is_ok()
            && let Ok(name) = String::from_utf16(&buf[..len as usize])
        {
            result.push(name);
        }
        index += 1;
    }
    result
}

fn open_subkey(parent: HKEY, name: &str) -> Option<OwnedHkey> {
    let wide = HSTRING::from(name);
    let mut child = HKEY::default();
    if unsafe { RegOpenKeyExW(parent, &wide, None, KEY_READ, &mut child) }.is_err() {
        return None;
    }
    Some(OwnedHkey(child))
}

fn is_subkey_active(parent: HKEY, name: &str) -> bool {
    let Some(child) = open_subkey(parent, name) else {
        return false;
    };
    let Some(start) = query_qword(child.0, w!("LastUsedTimeStart")) else {
        return false;
    };
    let stop = query_qword(child.0, w!("LastUsedTimeStop"));
    is_consent_entry_active(start, stop)
}

fn running_process_names_lower() -> Vec<String> {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let _guard = OwnedHandle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut names = Vec::new();
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
        return names;
    }
    loop {
        let len = entry
            .szExeFile
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(entry.szExeFile.len());
        if let Ok(name) = String::from_utf16(&entry.szExeFile[..len]) {
            names.push(name.to_ascii_lowercase());
        }
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    names
}

fn scan_consent_store(base_path: PCWSTR, verify_nonpackaged_process: bool) -> Option<String> {
    let mut root = HKEY::default();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, base_path, None, KEY_READ, &mut root) }.is_err() {
        return None;
    }
    let root = OwnedHkey(root);
    for name in enum_subkeys(root.0) {
        if name.eq_ignore_ascii_case("NonPackaged") {
            continue;
        }
        if is_subkey_active(root.0, &name) {
            return Some(format_consent_app_name(&name, true));
        }
    }
    let nonpackaged = open_subkey(root.0, "NonPackaged")?;
    let active_nonpackaged: Vec<String> = enum_subkeys(nonpackaged.0)
        .into_iter()
        .filter(|name| is_subkey_active(nonpackaged.0, name))
        .collect();
    if active_nonpackaged.is_empty() {
        return None;
    }
    if !verify_nonpackaged_process {
        return active_nonpackaged
            .first()
            .map(|name| format_consent_app_name(name, false));
    }
    let running = running_process_names_lower();
    active_nonpackaged
        .iter()
        .find(|name| {
            extract_nonpackaged_exe_lower(name)
                .is_some_and(|exe| running.iter().any(|proc_name| proc_name == &exe))
        })
        .map(|name| format_consent_app_name(name, false))
}

fn refresh_consent_state() {
    let cam_app = scan_consent_store(CONSENT_WEBCAM_PATH, true);
    let mic_app = scan_consent_store(CONSENT_MICROPHONE_PATH, true);
    CAMERA_ACTIVE.store(cam_app.is_some(), Ordering::Release);
    if let Ok(mut guard) = CONSENT_APP_NAME.lock() {
        *guard = cam_app.or(mic_app);
    }
    LOADED.store(true, Ordering::Release);
}

fn arm_watches(watches: &[RegWatch]) {
    for watch in watches {
        let _ = watch.arm();
    }
}

fn privacy_worker(shutdown: &OwnedHandle, ready: &SyncSender<()>) {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        let _ = ready.send(());
        return;
    }
    refresh_consent_state();
    let mut watches = Vec::with_capacity(2);
    if let Some(watch) = RegWatch::open(CONSENT_WEBCAM_PATH) {
        watches.push(watch);
    }
    if let Some(watch) = RegWatch::open(CONSENT_MICROPHONE_PATH) {
        watches.push(watch);
    }
    arm_watches(&watches);
    let _ = ready.send(());
    if watches.is_empty() {
        let _ = unsafe { WaitForMultipleObjects(&[shutdown.as_handle()], false, INFINITE) };
        return;
    }
    let mut handles = Vec::with_capacity(1 + watches.len());
    handles.push(shutdown.as_handle());
    handles.extend(watches.iter().map(|watch| watch.event.0));
    loop {
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            break;
        }
        let waited = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        if waited == WAIT_OBJECT_0 || SHUTTING_DOWN.load(Ordering::Acquire) {
            break;
        }
        let index = waited.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
        if index >= handles.len() {
            break;
        }
        arm_watches(&watches);
        refresh_consent_state();
        crate::idle_wake::request_idle_wake();
    }
}

pub fn resume_privacy_task() {
    SHUTTING_DOWN.store(false, Ordering::Release);
    LOADED.store(false, Ordering::Release);
}

fn start_runtime() -> bool {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return false;
    }
    let Ok(mut guard) = RUNTIME.lock() else {
        return false;
    };
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return false;
    }
    if guard.is_some() {
        return true;
    }
    let Ok(shutdown) = (unsafe { CreateEventW(None, true, false, PCWSTR::null()) }) else {
        return false;
    };
    let shutdown_event = Arc::new(OwnedHandle(shutdown));
    let (ready_tx, ready_rx) = sync_channel(1);
    let worker_shutdown = Arc::clone(&shutdown_event);
    let Ok(handle) = thread::Builder::new()
        .name(String::from("otto-privacy"))
        .spawn(move || privacy_worker(&worker_shutdown, &ready_tx))
    else {
        return false;
    };
    *guard = Some(PrivacyRuntime {
        handle,
        shutdown_event,
    });
    drop(guard);
    let _ = ready_rx.recv_timeout(READY_TIMEOUT);
    true
}

#[must_use]
pub(crate) fn read_privacy_state() -> PrivacyState {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return PrivacyState {
            mic: MicState::Unavailable,
            camera_active: false,
            app_name: None,
        };
    }
    if !LOADED.load(Ordering::Acquire) {
        let _ = start_runtime();
    }
    let mic = super::audio::read_mic_state();
    let camera_active = CAMERA_ACTIVE.load(Ordering::Acquire);
    let app_name = CONSENT_APP_NAME
        .try_lock()
        .ok()
        .and_then(|guard| guard.clone());
    PrivacyState {
        mic,
        camera_active,
        app_name,
    }
}

#[must_use]
pub(crate) fn toggle_mic_mute() -> Option<bool> {
    super::audio::toggle_mic_mute()
}

pub fn shutdown_privacy_task() {
    SHUTTING_DOWN.store(true, Ordering::Release);
    let _ = stop_runtime();
}

fn stop_runtime() -> bool {
    let Some(runtime) = RUNTIME.lock().ok().and_then(|mut guard| guard.take()) else {
        return true;
    };
    let _ = unsafe { SetEvent(runtime.shutdown_event.as_handle()) };
    let joined = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    CAMERA_ACTIVE.store(false, Ordering::Release);
    if !SHUTTING_DOWN.load(Ordering::Acquire) {
        LOADED.store(false, Ordering::Release);
    }
    if let Ok(mut guard) = CONSENT_APP_NAME.lock() {
        *guard = None;
    }
    joined
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, PoisonError};

    use super::{
        RUNTIME, format_consent_app_name, is_consent_entry_active, read_privacy_state,
        resume_privacy_task, shutdown_privacy_task, stop_runtime,
    };

    static RUNTIME_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct PrivacyResumeGuard;

    impl Drop for PrivacyResumeGuard {
        fn drop(&mut self) {
            let _ = stop_runtime();
            resume_privacy_task();
        }
    }

    #[test]
    fn nonpackaged_consent_keys_extract_clean_executable_names() {
        assert_eq!(
            format_consent_app_name("C:#Program Files#OBS Studio#bin#64bit#obs64.exe", false),
            "obs64"
        );
        assert_eq!(
            format_consent_app_name("C:#Users#dev#AppData#Local#Zoom#bin#Zoom.EXE", false),
            "Zoom"
        );
        assert_eq!(
            format_consent_app_name("C:#Users#dev#app🔥", false),
            "app🔥"
        );
        assert_eq!(
            format_consent_app_name("C:#Users#dev#cam🔥.exe", false),
            "cam🔥"
        );
    }

    #[test]
    fn packaged_consent_keys_strip_publisher_hash_and_prefix() {
        assert_eq!(
            format_consent_app_name("Microsoft.WindowsCamera_8wekyb3d8bbwe", true),
            "WindowsCamera"
        );
        assert_eq!(
            format_consent_app_name("Microsoft.WindowsSoundRecorder_8wekyb3d8bbwe!App", true),
            "WindowsSoundRecorder"
        );
    }

    #[test]
    fn consent_activity_requires_start_timestamp_and_zero_stop_timestamp() {
        assert!(is_consent_entry_active(133_500_000_000, Some(0)));
        assert!(!is_consent_entry_active(0, Some(0)));
        assert!(!is_consent_entry_active(
            133_500_000_000,
            Some(133_500_001_000)
        ));
        assert!(!is_consent_entry_active(133_500_000_000, None));
    }

    #[test]
    fn privacy_watcher_starts_on_read_and_exits_cleanly_on_shutdown() {
        let _serial = RUNTIME_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = PrivacyResumeGuard;
        resume_privacy_task();
        let _ = read_privacy_state();
        let alive = RUNTIME
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|runtime| !runtime.handle.is_finished());
        assert!(alive);
        assert!(stop_runtime());
        assert!(RUNTIME.lock().unwrap().is_none());
    }

    #[test]
    fn shutdown_prevents_privacy_worker_respawn_until_resumed() {
        let _serial = RUNTIME_TEST_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _resume = PrivacyResumeGuard;
        resume_privacy_task();
        let _ = read_privacy_state();
        shutdown_privacy_task();
        assert!(RUNTIME.lock().unwrap().is_none());

        let state = read_privacy_state();
        assert!(!state.camera_active);
        assert!(state.app_name.is_none());
        assert!(RUNTIME.lock().unwrap().is_none());
    }
}
