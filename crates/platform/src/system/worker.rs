use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

const JOIN_POLL: Duration = Duration::from_millis(10);

#[must_use]
pub fn join_within(handle: JoinHandle<()>, limit: Duration) -> bool {
    let start = Instant::now();
    while !handle.is_finished() && start.elapsed() < limit {
        thread::sleep(JOIN_POLL);
    }
    if handle.is_finished() {
        let _ = handle.join();
        true
    } else {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HookStartupPhase {
    Pending,
    Ready(u32),
    Failed,
    Cancelled,
}

pub(crate) struct HookStartup {
    phase: Mutex<HookStartupPhase>,
    changed: Condvar,
}

impl HookStartup {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            phase: Mutex::new(HookStartupPhase::Pending),
            changed: Condvar::new(),
        })
    }

    pub(crate) fn publish(&self, thread_id: Option<u32>) -> bool {
        let mut phase = self.phase.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(*phase, HookStartupPhase::Cancelled) {
            self.changed.notify_all();
            return false;
        }
        *phase = match thread_id {
            Some(id) => HookStartupPhase::Ready(id),
            None => HookStartupPhase::Failed,
        };
        self.changed.notify_all();
        thread_id.is_some()
    }

    pub(crate) fn wait_ready(&self, timeout: Duration) -> Option<u32> {
        let mut phase = self.phase.lock().unwrap_or_else(PoisonError::into_inner);
        let deadline = Instant::now() + timeout;
        while matches!(*phase, HookStartupPhase::Pending) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (next, result) = self
                .changed
                .wait_timeout(phase, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            phase = next;
            if result.timed_out() {
                break;
            }
        }
        match *phase {
            HookStartupPhase::Ready(thread_id) => Some(thread_id),
            HookStartupPhase::Failed | HookStartupPhase::Cancelled => None,
            HookStartupPhase::Pending => {
                *phase = HookStartupPhase::Cancelled;
                None
            }
        }
    }
}

pub(crate) struct HookThread {
    pub(crate) thread_id: u32,
    pub(crate) handle: JoinHandle<()>,
}

pub(crate) fn install_hook_thread(
    slot: &Mutex<Option<HookThread>>,
    name: &str,
    ready_timeout: Duration,
    shutdown_timeout: Duration,
    main: impl FnOnce(Arc<HookStartup>) + Send + 'static,
) -> bool {
    let Ok(mut guard) = slot.lock() else {
        return false;
    };
    if guard.is_some() {
        return true;
    }
    let startup = HookStartup::new();
    let worker_startup = Arc::clone(&startup);
    let Ok(handle) = thread::Builder::new()
        .name(String::from(name))
        .spawn(move || main(worker_startup))
    else {
        return false;
    };
    if let Some(thread_id) = startup.wait_ready(ready_timeout) {
        *guard = Some(HookThread { thread_id, handle });
        true
    } else {
        let _ = join_within(handle, shutdown_timeout);
        false
    }
}

pub(crate) fn uninstall_hook_thread(slot: &Mutex<Option<HookThread>>, shutdown_timeout: Duration) {
    let Some(thread) = slot.lock().ok().and_then(|mut guard| guard.take()) else {
        return;
    };
    unsafe {
        let _ = PostThreadMessageW(thread.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    }
    let _ = join_within(thread.handle, shutdown_timeout);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{HookStartup, join_within};

    #[test]
    fn joins_a_finished_worker() {
        let handle = thread::spawn(|| {});
        assert!(join_within(handle, Duration::from_millis(500)));
    }

    #[test]
    fn gives_up_on_a_worker_that_outlives_the_limit() {
        let (release, blocked) = channel::<()>();
        let handle = thread::spawn(move || {
            let _ = blocked.recv();
        });
        assert!(!join_within(handle, Duration::from_millis(30)));
        drop(release);
    }

    #[test]
    fn hook_startup_timeout_cancels_late_worker_publication() {
        let startup = HookStartup::new();
        let worker_startup = Arc::clone(&startup);
        let (allow_publish, proceed) = channel::<()>();
        let handle = thread::spawn(move || {
            let _ = proceed.recv();
            worker_startup.publish(Some(99))
        });
        assert_eq!(startup.wait_ready(Duration::from_millis(5)), None);
        let _ = allow_publish.send(());
        let accepted = handle.join().expect("worker joins cleanly");
        assert!(!accepted);
    }

    #[test]
    fn hook_startup_accepts_thread_published_before_timeout() {
        let startup = HookStartup::new();
        let worker_startup = Arc::clone(&startup);
        let handle = thread::spawn(move || worker_startup.publish(Some(77)));
        assert_eq!(startup.wait_ready(Duration::from_millis(500)), Some(77));
        assert!(handle.join().expect("worker joins cleanly"));
    }

    #[test]
    fn hook_startup_failure_wakes_waiter_early_with_none() {
        let startup = HookStartup::new();
        let worker_startup = Arc::clone(&startup);
        let handle = thread::spawn(move || worker_startup.publish(None));
        let started = Instant::now();
        assert_eq!(startup.wait_ready(Duration::from_secs(5)), None);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!handle.join().expect("worker joins cleanly"));
    }
}
