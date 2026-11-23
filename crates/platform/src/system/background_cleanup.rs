use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{LazyLock, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{
    CommandOutcome, HOG_SAMPLE_INTERVAL_SECS, HogTracker, ProcessAuditReport, ProcessSample,
    ProcessThrottleKind, RawProcessAuditSample, WindowPresence, classify_process_protection,
    summarize_process_audit,
};
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, FILETIME, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, GetPriorityClass, GetProcessTimes,
    IsProcessCritical, NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_CREATION_FLAGS,
    PROCESS_NAME_WIN32, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_TERMINATE,
    ProcessPowerThrottling, QueryFullProcessImageNameW, SetPriorityClass, SetProcessInformation,
    TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::{BOOL, PWSTR};

use crate::worker::join_within;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const MIN_CPU_DELTA_WINDOW: Duration = Duration::from_millis(250);

struct CleanupState {
    runtime: Option<CleanupRuntime>,
    auto_origins: HashMap<u32, Origin>,
    manual_origins: HashMap<u32, Origin>,
    prev_cpu_sample: Option<(Instant, HashMap<u32, u64>)>,
}

static STATE: LazyLock<Mutex<CleanupState>> = LazyLock::new(|| {
    Mutex::new(CleanupState {
        runtime: None,
        auto_origins: HashMap::new(),
        manual_origins: HashMap::new(),
        prev_cpu_sample: None,
    })
});

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
    if enabled {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        if guard.runtime.is_none() {
            let (stop, signal) = channel();
            if let Ok(handle) = thread::Builder::new()
                .name(String::from("otto-cleanup"))
                .spawn(move || cleanup_worker(&signal))
            {
                guard.runtime = Some(CleanupRuntime { stop, handle });
            }
        }
        return;
    }

    let runtime = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.runtime.take()
    };
    if let Some(runtime) = runtime {
        drop(runtime.stop);
        let _ = join_within(runtime.handle, SHUTDOWN_TIMEOUT);
    }
    let released: Vec<(u32, Origin)> = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.auto_origins.drain().collect()
    };
    for (pid, origin) in released {
        restore(pid, &origin);
    }
}

pub fn shutdown_background_cleanup() {
    set_background_cleanup(false);
    let released: Vec<(u32, Origin)> = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.manual_origins.drain().collect()
    };
    for (pid, origin) in released {
        restore(pid, &origin);
    }
}

fn cleanup_worker(signal: &Receiver<()>) {
    let mut tracker = HogTracker::default();
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

        let throttle_targets: Vec<u32> = {
            let Ok(guard) = STATE.lock() else {
                continue;
            };
            decision
                .throttle
                .into_iter()
                .filter(|pid| !guard.manual_origins.contains_key(pid))
                .collect()
        };
        let mut newly_throttled = Vec::new();
        for pid in throttle_targets {
            if let Some(origin) = throttle(pid) {
                newly_throttled.push((pid, origin));
            }
        }
        let restore_targets: Vec<(u32, Origin)> = if let Ok(mut guard) = STATE.lock() {
            let mut out = Vec::new();
            for pid in decision.restore {
                if let Some(origin) = guard.auto_origins.remove(&pid) {
                    out.push((pid, origin));
                }
            }
            out
        } else {
            Vec::new()
        };
        for (pid, origin) in restore_targets {
            restore(pid, &origin);
        }

        let live: HashSet<u32> = processes.iter().map(|(pid, _)| *pid).collect();
        if let Ok(mut guard) = STATE.lock() {
            for (pid, origin) in newly_throttled {
                guard.auto_origins.insert(pid, origin);
            }
            guard.auto_origins.retain(|pid, _| live.contains(pid));
            guard.manual_origins.retain(|pid, _| live.contains(pid));
        }
    }

    let to_release: Vec<(u32, Origin)> = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        let mut out = Vec::new();
        for pid in tracker.release_all() {
            if let Some(origin) = guard.auto_origins.remove(&pid) {
                out.push((pid, origin));
            }
        }
        for (pid, origin) in guard.auto_origins.drain() {
            out.push((pid, origin));
        }
        out
    };
    for (pid, origin) in to_release {
        restore(pid, &origin);
    }
}

struct AuditLockSnapshot {
    auto_throttled: HashSet<u32>,
    manual_throttled: HashSet<u32>,
    prev_cpu_sample: Option<(Instant, HashMap<u32, u64>)>,
}

fn snapshot_audit_state() -> Option<AuditLockSnapshot> {
    let guard = STATE.lock().ok()?;
    Some(AuditLockSnapshot {
        auto_throttled: guard.auto_origins.keys().copied().collect(),
        manual_throttled: guard.manual_origins.keys().copied().collect(),
        prev_cpu_sample: guard.prev_cpu_sample.clone(),
    })
}

fn collect_visible_window_sets(own_pid: u32) -> (HashSet<u32>, HashSet<String>) {
    let visible_windows = crate::window::model::enumerate_windows(own_pid);
    let mut visible_pids = HashSet::new();
    let mut visible_exes = HashSet::new();
    for window in visible_windows {
        if window.presence != WindowPresence::Hidden {
            visible_pids.insert(window.identity.process_id);
            if let Some(file_name) = Path::new(&window.executable)
                .file_name()
                .and_then(|name| name.to_str())
            {
                visible_exes.insert(file_name.to_ascii_lowercase());
            }
        }
    }
    (visible_pids, visible_exes)
}

fn cpu_tenths_since_prev(
    pid: u32,
    cpu_now: Option<u64>,
    elapsed_100ns: Option<u64>,
    prev_map: Option<&HashMap<u32, u64>>,
    logical_cpus: u64,
) -> u16 {
    let (Some(now_cpu), Some(window_100ns), Some(before_map)) = (cpu_now, elapsed_100ns, prev_map)
    else {
        return 0;
    };
    before_map.get(&pid).map_or(0, |before| {
        let delta = now_cpu.saturating_sub(*before);
        let tenths = delta.saturating_mul(1000) / (window_100ns * logical_cpus);
        u16::try_from(tenths).unwrap_or(1000).min(1000)
    })
}

#[must_use]
pub fn collect_process_audit(own_pid: u32) -> ProcessAuditReport {
    let processes = list_processes();
    let fg_pid = foreground_process_id();
    let fg_exe = fg_pid.and_then(|pid| {
        processes
            .iter()
            .find(|(id, _)| *id == pid)
            .map(|(_, name)| name.clone())
    });
    let (visible_pids, visible_exes) = collect_visible_window_sets(own_pid);
    let Some(snapshot) = snapshot_audit_state() else {
        return ProcessAuditReport::default();
    };

    let now = Instant::now();
    let logical_cpus = thread::available_parallelism()
        .map_or(1, |count| u64::try_from(count.get()).unwrap_or(1))
        .max(1);
    let elapsed_100ns = snapshot.prev_cpu_sample.as_ref().and_then(|(prev_at, _)| {
        let duration = now.duration_since(*prev_at);
        (duration >= MIN_CPU_DELTA_WINDOW).then(|| {
            u64::try_from(duration.as_nanos() / 100)
                .unwrap_or(u64::MAX)
                .max(1)
        })
    });
    let prev_map = snapshot.prev_cpu_sample.as_ref().map(|(_, by_pid)| by_pid);

    let mut next_cpu_map = HashMap::with_capacity(processes.len());
    let mut path_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut samples = Vec::with_capacity(processes.len());

    for (pid, exe_name) in processes {
        if pid == 0 {
            continue;
        }
        let session_id = process_session_id(pid).unwrap_or(0);
        let query_handle = open_query(pid);
        let is_critical = query_handle.as_ref().is_some_and(process_is_critical);
        let memory_bytes = query_handle.as_ref().map_or(0, process_memory_bytes);
        let cpu_now = query_handle
            .as_ref()
            .and_then(|handle| process_times(handle).map(|(_, cpu)| cpu));
        if let Some(cpu) = cpu_now {
            next_cpu_map.insert(pid, cpu);
        }
        let exe_path = path_cache
            .entry(exe_name.clone())
            .or_insert_with(|| query_handle.as_ref().and_then(process_executable_path))
            .clone();
        drop(query_handle);

        let cpu_tenths_percent =
            cpu_tenths_since_prev(pid, cpu_now, elapsed_100ns, prev_map, logical_cpus);
        let controllable = session_id != 0 && !is_critical && open_controllable(pid).is_some();
        let is_foreground = fg_pid == Some(pid)
            || fg_exe
                .as_deref()
                .is_some_and(|name| name == exe_name.as_str());
        let has_visible_window =
            visible_pids.contains(&pid) || visible_exes.contains(exe_name.as_str());
        let protected =
            classify_process_protection(&exe_name, pid, session_id, is_critical, controllable)
                .is_some();
        let terminatable = !protected
            && !is_foreground
            && pid != own_pid
            && controllable
            && open_terminatable(pid).is_some();
        let throttle_kind = if snapshot.manual_throttled.contains(&pid) {
            ProcessThrottleKind::ManualEcoQos
        } else if snapshot.auto_throttled.contains(&pid) {
            ProcessThrottleKind::AutoEcoQos
        } else {
            ProcessThrottleKind::None
        };

        samples.push(RawProcessAuditSample {
            pid,
            exe_name,
            exe_path,
            memory_bytes,
            cpu_tenths_percent,
            session_id,
            is_critical,
            controllable,
            terminatable,
            is_foreground,
            has_visible_window,
            throttle_kind,
        });
    }

    if (snapshot.prev_cpu_sample.is_none() || elapsed_100ns.is_some())
        && let Ok(mut guard) = STATE.lock()
    {
        guard.prev_cpu_sample = Some((now, next_cpu_map));
    }

    summarize_process_audit(&samples)
}

pub fn toggle_process_group_throttle(exe_name: &str) -> Option<bool> {
    let target = exe_name.trim().to_ascii_lowercase();
    if target.is_empty() || classify_process_protection(&target, 100, 1, false, true).is_some() {
        return None;
    }
    let pids: Vec<u32> = list_processes()
        .into_iter()
        .filter(|(pid, name)| *pid > 4 && name == &target)
        .map(|(pid, _)| pid)
        .collect();
    if pids.is_empty() {
        return None;
    }

    let restore_list: Option<Vec<(u32, Origin)>> = {
        let Ok(mut guard) = STATE.lock() else {
            return None;
        };
        let is_throttled = pids.iter().any(|pid| {
            guard.manual_origins.contains_key(pid) || guard.auto_origins.contains_key(pid)
        });
        if is_throttled {
            let mut out = Vec::new();
            for pid in &pids {
                if let Some(origin) = guard.manual_origins.remove(pid) {
                    out.push((*pid, origin));
                }
                if let Some(origin) = guard.auto_origins.remove(pid) {
                    out.push((*pid, origin));
                }
            }
            Some(out)
        } else {
            None
        }
    };

    if let Some(to_restore) = restore_list {
        for (pid, origin) in to_restore {
            restore(pid, &origin);
        }
        return Some(false);
    }

    let mut newly_throttled = Vec::new();
    for pid in pids {
        if process_session_id(pid).is_none_or(|sid| sid == 0) {
            continue;
        }
        if open_query(pid).is_none_or(|handle| process_is_critical(&handle)) {
            continue;
        }
        if let Some(origin) = throttle(pid) {
            newly_throttled.push((pid, origin));
        }
    }
    if newly_throttled.is_empty() {
        return None;
    }
    if let Ok(mut guard) = STATE.lock() {
        for (pid, origin) in newly_throttled {
            guard.manual_origins.insert(pid, origin);
        }
    }
    Some(true)
}

#[must_use]
pub fn terminate_process_group(exe_name: &str, own_pid: u32) -> CommandOutcome {
    let target = exe_name.trim().to_ascii_lowercase();
    if target.is_empty() || classify_process_protection(&target, 100, 1, false, true).is_some() {
        return CommandOutcome::Denied;
    }
    let processes = list_processes();
    let fg_pid = foreground_process_id();
    let fg_matches = fg_pid.is_some_and(|pid| {
        processes
            .iter()
            .any(|(id, name)| *id == pid && name == &target)
    });
    if fg_matches {
        return CommandOutcome::Denied;
    }

    let pids: Vec<u32> = processes
        .into_iter()
        .filter(|(pid, name)| *pid > 4 && *pid != own_pid && name == &target)
        .map(|(pid, _)| pid)
        .collect();
    if pids.is_empty() {
        return CommandOutcome::TargetGone;
    }

    let mut any_completed = false;
    let mut any_denied = false;
    let mut terminated_pids = Vec::new();
    for pid in pids {
        if process_session_id(pid).is_none_or(|sid| sid == 0) {
            any_denied = true;
            continue;
        }
        if open_query(pid).is_none_or(|handle| process_is_critical(&handle)) {
            any_denied = true;
            continue;
        }
        match unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) } {
            Ok(handle) => {
                let process = OwnedProcess(handle);
                if unsafe { TerminateProcess(process.0, 1) }.is_ok() {
                    any_completed = true;
                    terminated_pids.push(pid);
                } else {
                    any_denied = true;
                }
            }
            Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                any_denied = true;
            }
            Err(_) => {}
        }
    }

    if !terminated_pids.is_empty()
        && let Ok(mut guard) = STATE.lock()
    {
        for pid in terminated_pids {
            guard.auto_origins.remove(&pid);
            guard.manual_origins.remove(&pid);
        }
    }

    if any_completed {
        CommandOutcome::Completed
    } else if any_denied {
        CommandOutcome::Denied
    } else {
        CommandOutcome::TargetGone
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
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    (pid != 0).then_some(pid)
}

fn open_query(pid: u32) -> Option<OwnedProcess> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    Some(OwnedProcess(handle))
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

fn open_terminatable(pid: u32) -> Option<OwnedProcess> {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }.ok()?;
    Some(OwnedProcess(handle))
}

fn process_session_id(pid: u32) -> Option<u32> {
    let mut session_id = 0u32;
    unsafe { ProcessIdToSessionId(pid, &raw mut session_id) }.ok()?;
    Some(session_id)
}

fn process_is_critical(process: &OwnedProcess) -> bool {
    let mut critical = BOOL::default();
    unsafe { IsProcessCritical(process.0, &raw mut critical) }.is_ok() && critical.as_bool()
}

fn process_memory_bytes(process: &OwnedProcess) -> u64 {
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

fn process_executable_path(process: &OwnedProcess) -> Option<String> {
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
    if process_is_critical(&process) {
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

    #[test]
    fn collect_process_audit_finds_protected_system_processes_and_blocks_unsafe_actions() {
        let report = collect_process_audit(std::process::id());
        assert!(report.total_processes > 0);
        assert!(report.protected_group_count > 0);
        assert!(toggle_process_group_throttle("explorer.exe").is_none());
        assert_eq!(
            terminate_process_group("explorer.exe", std::process::id()),
            CommandOutcome::Denied
        );
        assert_eq!(
            terminate_process_group("svchost.exe", std::process::id()),
            CommandOutcome::Denied
        );
    }
}
