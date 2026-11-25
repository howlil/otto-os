use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use otto_core::{
    CommandOutcome, HOG_SAMPLE_INTERVAL_SECS, HogTracker, ProcessAuditReport, ProcessSample,
    ProcessThrottleKind, RawProcessAuditSample, WindowPresence, classify_process_protection,
    summarize_process_audit,
};
use windows::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows::Win32::System::Threading::{
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, GetPriorityClass,
    NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_CREATION_FLAGS,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    SetPriorityClass, TerminateProcess,
};

use super::process_policy::{ProcessOrigin, ThrottleLedger, ThrottleReason};
use super::process_win32::{
    OwnedProcess, apply_power_throttling, foreground_process_id, is_verified_non_critical,
    list_processes, open_controllable, open_query, open_terminatable, process_executable_path,
    process_memory_bytes, process_session_id, process_times, query_power_throttling,
};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const JOIN_POLL: Duration = Duration::from_millis(10);
const MAX_RETIRED_WORKERS: usize = 8;
const MIN_CPU_DELTA_WINDOW: Duration = Duration::from_millis(250);

struct CleanupState {
    runtime: Option<CleanupRuntime>,
    worker_generation: u64,
    retired_workers: Vec<JoinHandle<()>>,
    ledger: ThrottleLedger,
    prev_cpu_sample: Option<(Instant, HashMap<u32, u64>)>,
}

static STATE: LazyLock<Mutex<CleanupState>> = LazyLock::new(|| {
    Mutex::new(CleanupState {
        runtime: None,
        worker_generation: 0,
        retired_workers: Vec::new(),
        ledger: ThrottleLedger::default(),
        prev_cpu_sample: None,
    })
});

struct CleanupRuntime {
    stop: Sender<()>,
    stop_flag: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

fn reap_retired_workers(retired: &mut Vec<JoinHandle<()>>) {
    let mut i = 0;
    while i < retired.len() {
        if retired[i].is_finished() {
            let done = retired.remove(i);
            let _ = done.join();
        } else {
            i += 1;
        }
    }
}

pub fn set_background_cleanup(enabled: bool) {
    if enabled {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        reap_retired_workers(&mut guard.retired_workers);
        if guard.runtime.is_none() {
            guard.worker_generation = guard.worker_generation.wrapping_add(1);
            let generation = guard.worker_generation;
            let stop_flag = Arc::new(AtomicBool::new(false));
            let worker_flag = Arc::clone(&stop_flag);
            let (stop, signal) = channel();
            if let Ok(handle) = thread::Builder::new()
                .name(String::from("otto-cleanup"))
                .spawn(move || cleanup_worker(generation, &worker_flag, &signal))
            {
                guard.runtime = Some(CleanupRuntime {
                    stop,
                    stop_flag,
                    handle,
                });
            }
        }
        return;
    }

    let runtime = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.worker_generation = guard.worker_generation.wrapping_add(1);
        let rt = guard.runtime.take();
        if let Some(ref rt) = rt {
            rt.stop_flag.store(true, Ordering::Release);
        }
        rt
    };
    if let Some(runtime) = runtime {
        drop(runtime.stop);
        let start = Instant::now();
        while !runtime.handle.is_finished() && start.elapsed() < SHUTDOWN_TIMEOUT {
            thread::sleep(JOIN_POLL);
        }
        if runtime.handle.is_finished() {
            let _ = runtime.handle.join();
        } else if let Ok(mut guard) = STATE.lock() {
            reap_retired_workers(&mut guard.retired_workers);
            if guard.retired_workers.len() < MAX_RETIRED_WORKERS {
                guard.retired_workers.push(runtime.handle);
            }
        }
    }
    let released = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.ledger.release_all_for_reason(ThrottleReason::AUTO)
    };
    for (pid, origin) in released {
        restore(pid, &origin);
    }
}

pub fn shutdown_background_cleanup() {
    set_background_cleanup(false);
    let released = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.ledger.release_all()
    };
    for (pid, origin) in released {
        restore(pid, &origin);
    }
}

pub(super) fn throttle_exe_for_focus(exe_name: &str) -> usize {
    let target = exe_name.to_ascii_lowercase();
    let pids: Vec<u32> = list_processes()
        .into_iter()
        .filter(|(pid, name)| *pid > 4 && name.eq_ignore_ascii_case(&target))
        .map(|(pid, _)| pid)
        .collect();
    let mut count = 0usize;
    for pid in pids {
        if acquire_throttle_for_reason(pid, ThrottleReason::FOCUS) {
            count += 1;
        }
    }
    count
}

pub(super) fn release_focus_throttles() {
    let released = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        guard.ledger.release_all_for_reason(ThrottleReason::FOCUS)
    };
    for (pid, origin) in released {
        restore(pid, &origin);
    }
}

fn is_worker_active(generation: u64, stop_flag: &AtomicBool) -> bool {
    !stop_flag.load(Ordering::Acquire)
        && STATE
            .lock()
            .is_ok_and(|guard| guard.worker_generation == generation)
}

fn cleanup_worker(generation: u64, stop_flag: &AtomicBool, signal: &Receiver<()>) {
    let mut tracker = HogTracker::default();
    let mut last_sample = Instant::now();
    while !stop_flag.load(Ordering::Acquire)
        && signal.recv_timeout(Duration::from_secs(HOG_SAMPLE_INTERVAL_SECS))
            == Err(RecvTimeoutError::Timeout)
    {
        if !is_worker_active(generation, stop_flag) {
            break;
        }
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
        if !is_worker_active(generation, stop_flag) {
            break;
        }

        for pid in decision.throttle {
            if !is_worker_active(generation, stop_flag) {
                break;
            }
            let _ = acquire_throttle_for_reason(pid, ThrottleReason::AUTO);
        }
        let restore_targets = {
            let Ok(mut guard) = STATE.lock() else {
                continue;
            };
            if stop_flag.load(Ordering::Acquire) || guard.worker_generation != generation {
                break;
            }
            let mut out = Vec::new();
            for pid in decision.restore {
                if let Some(origin) = guard.ledger.release_reason(pid, ThrottleReason::AUTO) {
                    out.push((pid, origin));
                }
            }
            let live: HashSet<u32> = processes.iter().map(|(pid, _)| *pid).collect();
            guard.ledger.prune_dead(&live);
            out
        };
        for (pid, origin) in restore_targets {
            restore(pid, &origin);
        }
    }

    let to_release = {
        let Ok(mut guard) = STATE.lock() else {
            return;
        };
        if guard.worker_generation == generation {
            let mut out = Vec::new();
            for pid in tracker.release_all() {
                if let Some(origin) = guard.ledger.release_reason(pid, ThrottleReason::AUTO) {
                    out.push((pid, origin));
                }
            }
            out.extend(guard.ledger.release_all_for_reason(ThrottleReason::AUTO));
            out
        } else {
            Vec::new()
        }
    };
    for (pid, origin) in to_release {
        restore(pid, &origin);
    }
}

struct AuditLockSnapshot {
    throttle_kinds: HashMap<u32, ProcessThrottleKind>,
    prev_cpu_sample: Option<(Instant, HashMap<u32, u64>)>,
}

fn snapshot_audit_state(pids: &[u32]) -> Option<AuditLockSnapshot> {
    let guard = STATE.lock().ok()?;
    let mut throttle_kinds = HashMap::with_capacity(pids.len());
    for &pid in pids {
        let kind = guard.ledger.throttle_kind(pid);
        if kind != ProcessThrottleKind::None {
            throttle_kinds.insert(pid, kind);
        }
    }
    Some(AuditLockSnapshot {
        throttle_kinds,
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
    let pids: Vec<u32> = processes.iter().map(|(pid, _)| *pid).collect();
    let fg_pid = foreground_process_id();
    let fg_exe = fg_pid.and_then(|pid| {
        processes
            .iter()
            .find(|(id, _)| *id == pid)
            .map(|(_, name)| name.clone())
    });
    let (visible_pids, visible_exes) = collect_visible_window_sets(own_pid);
    let Some(snapshot) = snapshot_audit_state(&pids) else {
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
        let is_critical = query_handle
            .as_ref()
            .is_none_or(|handle| !is_verified_non_critical(handle));
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
        let throttle_kind = snapshot
            .throttle_kinds
            .get(&pid)
            .copied()
            .unwrap_or(ProcessThrottleKind::None);

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

    let restore_list: Option<Vec<(u32, ProcessOrigin)>> = {
        let Ok(mut guard) = STATE.lock() else {
            return None;
        };
        let is_throttled = pids.iter().any(|pid| {
            guard
                .ledger
                .has_reason(*pid, ThrottleReason::MANUAL | ThrottleReason::AUTO)
        });
        is_throttled.then(|| {
            let mut out = Vec::new();
            for pid in &pids {
                if let Some(origin) = guard
                    .ledger
                    .release_reason(*pid, ThrottleReason::MANUAL | ThrottleReason::AUTO)
                {
                    out.push((*pid, origin));
                }
            }
            out
        })
    };

    if let Some(to_restore) = restore_list {
        for (pid, origin) in to_restore {
            restore(pid, &origin);
        }
        return Some(false);
    }

    let mut throttled_any = false;
    for pid in pids {
        if process_session_id(pid).is_none_or(|sid| sid == 0) {
            continue;
        }
        if acquire_throttle_for_reason(pid, ThrottleReason::MANUAL) {
            throttled_any = true;
        }
    }
    throttled_any.then_some(true)
}

#[must_use]
pub fn terminate_process_group(exe_name: &str, own_pid: u32) -> CommandOutcome {
    terminate_process_group_matching(exe_name, None, own_pid)
}

#[must_use]
pub(crate) fn terminate_process_group_matching(
    exe_name: &str,
    expected_exe_path: Option<&str>,
    own_pid: u32,
) -> CommandOutcome {
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
        let Some(sid) = process_session_id(pid).filter(|&sid| sid != 0) else {
            any_denied = true;
            continue;
        };
        let process = match unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                false,
                pid,
            )
        } {
            Ok(handle) => OwnedProcess(handle),
            Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                any_denied = true;
                continue;
            }
            Err(_) => continue,
        };
        if process_times(&process).is_none() || !is_verified_non_critical(&process) {
            any_denied = true;
            continue;
        }
        let Some(exe_path) = process_executable_path(&process) else {
            any_denied = true;
            continue;
        };
        let basename_ok = Path::new(&exe_path)
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(&target));
        let full_path_ok =
            expected_exe_path.is_none_or(|expected| exe_path.eq_ignore_ascii_case(expected.trim()));
        if !basename_ok
            || !full_path_ok
            || classify_process_protection(&target, pid, sid, false, true).is_some()
            || foreground_process_id() == Some(pid)
        {
            any_denied = true;
            continue;
        }
        if unsafe { TerminateProcess(process.0, 1) }.is_ok() {
            any_completed = true;
            terminated_pids.push(pid);
        } else {
            any_denied = true;
        }
    }

    if !terminated_pids.is_empty()
        && let Ok(mut guard) = STATE.lock()
    {
        for pid in terminated_pids {
            let _ = guard.ledger.release_reason(
                pid,
                ThrottleReason::AUTO | ThrottleReason::MANUAL | ThrottleReason::FOCUS,
            );
        }
    }

    if any_denied {
        CommandOutcome::Denied
    } else if any_completed {
        CommandOutcome::Completed
    } else {
        CommandOutcome::TargetGone
    }
}

fn controllable_cpu_time(pid: u32) -> Option<u64> {
    let process = open_controllable(pid)?;
    if !is_verified_non_critical(&process) {
        return None;
    }
    process_times(&process).map(|(_, cpu)| cpu)
}

fn throttle_process(process: &OwnedProcess, created: u64) -> Option<ProcessOrigin> {
    let original_power = query_power_throttling(process);
    let priority = unsafe { GetPriorityClass(process.0) };
    let lowerable =
        priority == NORMAL_PRIORITY_CLASS.0 || priority == ABOVE_NORMAL_PRIORITY_CLASS.0;
    let lowered =
        lowerable && unsafe { SetPriorityClass(process.0, BELOW_NORMAL_PRIORITY_CLASS) }.is_ok();
    let eco = original_power.is_some()
        && apply_power_throttling(
            process,
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        );
    (lowered || eco).then_some(ProcessOrigin {
        created,
        lowered_from: lowered.then_some(priority),
        power_throttling: eco.then_some(original_power).flatten(),
    })
}

fn acquire_throttle_for_reason(pid: u32, reason: u8) -> bool {
    let Some(process) = open_controllable(pid) else {
        return false;
    };
    if !is_verified_non_critical(&process) {
        return false;
    }
    let Some((created, _)) = process_times(&process) else {
        return false;
    };
    if let Ok(mut guard) = STATE.lock()
        && guard.ledger.add_reason_if_present(pid, created, reason)
    {
        return true;
    }
    let Some(origin) = throttle_process(&process, created) else {
        return false;
    };
    if let Ok(mut guard) = STATE.lock() {
        guard.ledger.record_or_add_reason(pid, reason, origin);
        true
    } else {
        restore(pid, &origin);
        false
    }
}

fn restore(pid: u32, origin: &ProcessOrigin) {
    let Some(process) = open_controllable(pid) else {
        return;
    };
    if process_times(&process).is_none_or(|(created, _)| created != origin.created) {
        return;
    }
    if let Some(snap) = origin.power_throttling {
        let _ = apply_power_throttling(&process, snap.control_mask, snap.state_mask);
    }
    if let Some(priority) = origin.lowered_from {
        let _ = unsafe { SetPriorityClass(process.0, PROCESS_CREATION_FLAGS(priority)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ChildGuard(std::process::Child);

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn own_process_is_listed_and_foreground_lookup_does_not_panic() {
        let own = std::process::id();
        assert!(list_processes().iter().any(|(pid, _)| *pid == own));
        let _ = foreground_process_id();
    }

    #[test]
    fn throttle_lowers_then_restores_a_child_process_and_power_throttling() {
        let child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 20 127.0.0.1 >NUL"])
            .spawn()
            .expect("spawn child");
        let guard = ChildGuard(child);
        let pid = guard.0.id();
        let process = open_controllable(pid).expect("open child");
        unsafe { SetPriorityClass(process.0, NORMAL_PRIORITY_CLASS) }.expect("normalize child");
        assert!(is_verified_non_critical(&process));
        let original = unsafe { GetPriorityClass(process.0) };
        assert_eq!(original, NORMAL_PRIORITY_CLASS.0);

        let before_power = query_power_throttling(&process).expect("query power throttling");
        let (created, _) = process_times(&process).expect("process times");
        let origin = throttle_process(&process, created).expect("throttle child");
        assert_eq!(
            unsafe { GetPriorityClass(process.0) },
            BELOW_NORMAL_PRIORITY_CLASS.0
        );
        let throttled_power = query_power_throttling(&process).expect("query throttled");
        assert_eq!(
            throttled_power.state_mask & PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        );
        restore(pid, &origin);
        assert_eq!(unsafe { GetPriorityClass(process.0) }, original);
        assert_eq!(query_power_throttling(&process), Some(before_power));

        assert!(apply_power_throttling(
            &process,
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            0
        ));
        let explicit_before = query_power_throttling(&process).expect("query explicit");
        let origin2 = throttle_process(&process, created).expect("throttle explicit child");
        restore(pid, &origin2);
        assert_eq!(query_power_throttling(&process), Some(explicit_before));
    }

    #[test]
    fn enabling_and_disabling_is_idempotent_across_rapid_cycles() {
        for _ in 0..3 {
            set_background_cleanup(true);
            set_background_cleanup(true);
            set_background_cleanup(false);
            set_background_cleanup(false);
        }
        let is_none = STATE.lock().expect("state lock").runtime.is_none();
        assert!(is_none);
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

    #[test]
    fn terminate_matching_rejects_mismatched_executable_path() {
        let child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 20 127.0.0.1 >NUL"])
            .spawn()
            .expect("spawn child");
        let guard = ChildGuard(child);
        let outcome = terminate_process_group_matching(
            "cmd.exe",
            Some(r"C:\nonexistent-mismatch\cmd.exe"),
            std::process::id(),
        );
        assert!(matches!(
            outcome,
            CommandOutcome::Denied | CommandOutcome::TargetGone
        ));
        let process = open_query(guard.0.id()).expect("child still alive");
        assert!(process_times(&process).is_some());
    }
}
