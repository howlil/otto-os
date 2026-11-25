use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetProcessInformation, GetProcessTimes, IsProcessCritical, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_TERMINATE,
    ProcessPowerThrottling, QueryFullProcessImageNameW, SetProcessInformation,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::{BOOL, PWSTR};

use super::process_policy::PowerThrottlingSnapshot;

pub(super) struct OwnedProcess(pub(super) HANDLE);

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn list_processes() -> Vec<(u32, String)> {
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let snapshot = OwnedProcess(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes = Vec::new();
    if unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_err() {
        return processes;
    }
    loop {
        let len = entry
            .szExeFile
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(entry.szExeFile.len());
        if let Ok(name) = String::from_utf16(&entry.szExeFile[..len]) {
            processes.push((entry.th32ProcessID, name.to_ascii_lowercase()));
        }
        if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() {
            break;
        }
    }
    processes
}

pub(super) fn foreground_process_id() -> Option<u32> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    (pid != 0).then_some(pid)
}

pub(super) fn open_query(pid: u32) -> Option<OwnedProcess> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    Some(OwnedProcess(handle))
}

pub(super) fn open_controllable(pid: u32) -> Option<OwnedProcess> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid,
        )
    }
    .ok()?;
    Some(OwnedProcess(handle))
}

pub(super) fn open_terminatable(pid: u32) -> Option<OwnedProcess> {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }.ok()?;
    Some(OwnedProcess(handle))
}

pub(super) fn process_session_id(pid: u32) -> Option<u32> {
    let mut session_id = 0u32;
    unsafe { ProcessIdToSessionId(pid, &raw mut session_id) }.ok()?;
    Some(session_id)
}

fn process_is_critical(process: &OwnedProcess) -> Option<bool> {
    let mut critical = BOOL::default();
    unsafe { IsProcessCritical(process.0, &raw mut critical) }
        .ok()
        .map(|()| critical.as_bool())
}

pub(super) fn is_verified_non_critical(process: &OwnedProcess) -> bool {
    process_is_critical(process) == Some(false)
}

pub(super) fn process_memory_bytes(process: &OwnedProcess) -> u64 {
    let mut pmc = PROCESS_MEMORY_COUNTERS_EX {
        cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    if unsafe { K32GetProcessMemoryInfo(process.0, (&raw mut pmc).cast(), pmc.cb) }.as_bool() {
        let bytes = if pmc.PrivateUsage > 0 {
            pmc.PrivateUsage
        } else {
            pmc.WorkingSetSize
        };
        u64::try_from(bytes).unwrap_or(0)
    } else {
        0
    }
}

pub(super) fn process_executable_path(process: &OwnedProcess) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut length = u32::try_from(buffer.len()).unwrap_or(1024);
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut length,
        )
    }
    .ok()?;
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    (!path.trim().is_empty()).then_some(path)
}

fn file_time(ft: FILETIME) -> u64 {
    (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime)
}

pub(super) fn process_times(process: &OwnedProcess) -> Option<(u64, u64)> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) }
        .ok()?;
    Some((file_time(created), file_time(kernel) + file_time(user)))
}

pub(super) fn query_power_throttling(process: &OwnedProcess) -> Option<PowerThrottlingSnapshot> {
    let mut state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: 0,
        StateMask: 0,
    };
    unsafe {
        GetProcessInformation(
            process.0,
            ProcessPowerThrottling,
            (&raw mut state).cast::<c_void>(),
            size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    }
    .ok()?;
    Some(PowerThrottlingSnapshot {
        control_mask: state.ControlMask,
        state_mask: state.StateMask,
    })
}

pub(super) fn apply_power_throttling(
    process: &OwnedProcess,
    control_mask: u32,
    state_mask: u32,
) -> bool {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: control_mask,
        StateMask: state_mask,
    };
    unsafe {
        SetProcessInformation(
            process.0,
            ProcessPowerThrottling,
            (&raw const state).cast::<c_void>(),
            size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    }
    .is_ok()
}
