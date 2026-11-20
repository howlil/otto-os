use std::collections::HashMap;
use std::mem::size_of;
use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

use super::inspect::owner_ids;

pub(super) const DWM_CLOAKED_SHELL: u32 = 2;
pub(super) const FRAME_HOST_CLASS: &str = "ApplicationFrameWindow";
pub(super) const HOSTED_CORE_CLASS: &str = "Windows.UI.Core.CoreWindow";
pub(super) const RESTORE_TIMEOUT_MS: u32 = 80;
pub(super) const FULL_SCREEN_SHORTCUT_APPS: &[&str] = &[
    "chrome",
    "msedge",
    "firefox",
    "brave",
    "opera",
    "vivaldi",
    "explorer",
    "windowsterminal",
    "code",
];

#[derive(Debug, Clone)]
pub(super) struct ProcessFacts {
    pub(super) generation: u64,
    pub(super) executable: String,
    pub(super) user_model_id: Option<String>,
    pub(super) elevated: bool,
}

pub(super) struct ProcessHandle(HANDLE);

impl ProcessHandle {
    pub(super) fn open(process_id: u32) -> Option<Self> {
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
            .ok()
            .map(Self)
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub(super) fn filetime_value(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

pub(super) fn process_generation(process: &ProcessHandle) -> Option<u64> {
    let mut created = FILETIME::default();
    let mut ignored = [FILETIME::default(); 3];
    unsafe {
        GetProcessTimes(
            process.0,
            &raw mut created,
            &raw mut ignored[0],
            &raw mut ignored[1],
            &raw mut ignored[2],
        )
    }
    .ok()?;
    Some(filetime_value(created))
}

pub(super) fn process_executable(process: &ProcessHandle) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut length,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

pub(super) fn process_user_model_id(process: &ProcessHandle) -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut length = buffer.len() as u32;
    let status = unsafe {
        GetApplicationUserModelId(process.0, &raw mut length, Some(PWSTR(buffer.as_mut_ptr())))
    };
    if status.0 != 0 || length <= 1 {
        return None;
    }
    let id = String::from_utf16_lossy(&buffer[..length as usize - 1]);
    (!id.trim().is_empty()).then_some(id)
}

pub(super) fn token_is_elevated(process: HANDLE) -> Option<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) }.ok()?;
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some((&raw mut elevation).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &raw mut returned,
        )
    };
    unsafe {
        let _ = CloseHandle(token);
    }
    read.ok()?;
    Some(elevation.TokenIsElevated != 0)
}

pub(super) fn own_process_is_elevated() -> bool {
    static ELEVATED: OnceLock<bool> = OnceLock::new();
    *ELEVATED.get_or_init(|| token_is_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false))
}

pub(crate) fn is_window_uipi_blocked(hwnd: HWND) -> bool {
    if own_process_is_elevated() {
        return false;
    }
    let (process_id, _) = owner_ids(hwnd);
    if process_id == 0 {
        return false;
    }
    let Some(process) = ProcessHandle::open(process_id) else {
        return true;
    };
    token_is_elevated(process.0).unwrap_or(true)
}

pub(super) fn read_process_facts(process_id: u32) -> Option<ProcessFacts> {
    let process = ProcessHandle::open(process_id)?;
    let target_elevated = token_is_elevated(process.0).unwrap_or(true);
    Some(ProcessFacts {
        generation: process_generation(&process)?,
        executable: process_executable(&process)?,
        user_model_id: process_user_model_id(&process),
        elevated: target_elevated && !own_process_is_elevated(),
    })
}

pub(super) type ProcessCache = HashMap<u32, Option<ProcessFacts>>;

pub(super) fn cached_process_facts(
    cache: &mut ProcessCache,
    process_id: u32,
) -> Option<ProcessFacts> {
    cache
        .entry(process_id)
        .or_insert_with(|| read_process_facts(process_id))
        .clone()
}
