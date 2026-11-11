use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Duration;

    use super::join_within;

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
}
