use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{HOG_SAMPLE_INTERVAL_SECS, HogTracker, ProcessSample};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, GetPriorityClass, GetProcessTimes,
    IsProcessCritical, NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_CREATION_FLAGS,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
    ProcessPowerThrottling, SetPriorityClass, SetProcessInformation,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::BOOL;

use crate::worker::join_within;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

static RUNTIME: Mutex<Option<CleanupRuntime>> = Mutex::new(None);

struct CleanupRuntime {
    stop: Sender<()>,
    handle: JoinHandle<()>,
}

struct Origin {
    created: u64,
    lowered_from: Option<u32>,
}

struct OwnedProcess(HANDLE);

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub fn set_background_cleanup(enabled: bool) {
    let Ok(mut guard) = RUNTIME.lock() else {
        return;
    };
    if enabled && guard.is_none() {
        let (stop, signal) = channel();
        if let Ok(handle) = thread::Builder::new()
            .name(String::from("otto-cleanup"))
            .spawn(move || cleanup_worker(&signal))
        {
            *guard = Some(CleanupRuntime { stop, handle });
        }
    } else if !enabled && let Some(runtime) = guard.take() {
        drop(runtime.stop);
        let _ = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    }
}

fn cleanup_worker(signal: &Receiver<()>) {
    let mut tracker = HogTracker::default();
    let mut origins: HashMap<u32, Origin> = HashMap::new();
    let mut last_sample = Instant::now();
    while let Err(RecvTimeoutError::Timeout) =
        signal.recv_timeout(Duration::from_secs(HOG_SAMPLE_INTERVAL_SECS))
    {
        let now = Instant::now();
        let elapsed_100ns =
            u64::try_from(now.duration_since(last_sample).as_nanos() / 100).unwrap_or(u64::MAX);
        last_sample = now;
        let processes = list_processes();
        let foreground_name = foreground_process_id().and_then(|pid| {
            processes
                .iter()
                .find(|(id, _)| *id == pid)
                .map(|(_, name)| name.as_str())
        });
        let samples: Vec<ProcessSample> = processes
            .iter()
            .filter_map(|(pid, name)| {
                Some(ProcessSample {
                    pid: *pid,
                    name: name.clone(),
                    cpu_100ns: controllable_cpu_time(*pid)?,
                })
            })
            .collect();
        let decision = tracker.update(&samples, elapsed_100ns, foreground_name);
        for pid in decision.throttle {
            if let Some(origin) = throttle(pid) {
                origins.insert(pid, origin);
            }
        }
        for pid in decision.restore {
            if let Some(origin) = origins.remove(&pid) {
                restore(pid, &origin);
            }
        }
        let live: HashSet<u32> = samples.iter().map(|sample| sample.pid).collect();
        origins.retain(|pid, _| live.contains(pid));
    }
    for pid in tracker.release_all() {
        if let Some(origin) = origins.remove(&pid) {
            restore(pid, &origin);
        }
    }
}

fn list_processes() -> Vec<(u32, String)> {
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

fn foreground_process_id() -> Option<u32> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

fn open_controllable(pid: u32) -> Option<OwnedProcess> {
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

fn file_time(ft: FILETIME) -> u64 {
    (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime)
}

fn process_times(process: &OwnedProcess) -> Option<(u64, u64)> {
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

fn controllable_cpu_time(pid: u32) -> Option<u64> {
    let process = open_controllable(pid)?;
    let mut critical = BOOL::default();
    unsafe { IsProcessCritical(process.0, &mut critical) }.ok()?;
    if critical.as_bool() {
        return None;
    }
    process_times(&process).map(|(_, cpu)| cpu)
}

fn set_eco_qos(process: &OwnedProcess, eco: bool) -> bool {
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: if eco {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
        StateMask: if eco {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
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

fn throttle(pid: u32) -> Option<Origin> {
    let process = open_controllable(pid)?;
    let (created, _) = process_times(&process)?;
    let priority = unsafe { GetPriorityClass(process.0) };
    let lowerable =
        priority == NORMAL_PRIORITY_CLASS.0 || priority == ABOVE_NORMAL_PRIORITY_CLASS.0;
    let lowered =
        lowerable && unsafe { SetPriorityClass(process.0, BELOW_NORMAL_PRIORITY_CLASS) }.is_ok();
    let eco = set_eco_qos(&process, true);
    (lowered || eco).then_some(Origin {
        created,
        lowered_from: lowered.then_some(priority),
    })
}

fn restore(pid: u32, origin: &Origin) {
    let Some(process) = open_controllable(pid) else {
        return;
    };
    if process_times(&process).is_none_or(|(created, _)| created != origin.created) {
        return;
    }
    set_eco_qos(&process, false);
    if let Some(priority) = origin.lowered_from {
        let _ = unsafe { SetPriorityClass(process.0, PROCESS_CREATION_FLAGS(priority)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_listed_and_foreground_lookup_does_not_panic() {
        let own = std::process::id();
        assert!(list_processes().iter().any(|(pid, _)| *pid == own));
        let _ = foreground_process_id();
    }

    #[test]
    fn throttle_lowers_then_restores_a_child_process() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 20 127.0.0.1 >NUL"])
            .spawn()
            .expect("spawn child");
        let pid = child.id();
        let process = open_controllable(pid).expect("open child");
        unsafe { SetPriorityClass(process.0, NORMAL_PRIORITY_CLASS) }.expect("normalize child");
        let original = unsafe { GetPriorityClass(process.0) };
        assert_eq!(original, NORMAL_PRIORITY_CLASS.0);
        let origin = throttle(pid).expect("throttle child");
        assert_eq!(
            unsafe { GetPriorityClass(process.0) },
            BELOW_NORMAL_PRIORITY_CLASS.0
        );
        restore(pid, &origin);
        assert_eq!(unsafe { GetPriorityClass(process.0) }, original);
        drop(process);
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn enabling_and_disabling_is_idempotent() {
        set_background_cleanup(true);
        set_background_cleanup(true);
        set_background_cleanup(false);
        set_background_cleanup(false);
    }
}
