use std::collections::{HashMap, HashSet};

pub const HOG_SAMPLE_INTERVAL_SECS: u64 = 10;
const HOG_CPU_PERCENT_OF_CORE: u64 = 20;
const HOG_SUSTAINED_SAMPLES: u8 = 4;
const NEVER_THROTTLE: [&str; 3] = ["explorer.exe", "otto-os.exe", "dwm.exe"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSample {
    pub pid: u32,
    pub name: String,
    pub cpu_100ns: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HogDecision {
    pub throttle: Vec<u32>,
    pub restore: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct HogTracker {
    previous_cpu: HashMap<u32, u64>,
    streak: HashMap<u32, u8>,
    throttled: HashSet<u32>,
}

impl HogTracker {
    pub fn update(
        &mut self,
        samples: &[ProcessSample],
        elapsed_100ns: u64,
        foreground_name: Option<&str>,
    ) -> HogDecision {
        let mut decision = HogDecision::default();
        let mut previous_cpu = HashMap::with_capacity(samples.len());
        let mut streak = HashMap::new();
        for sample in samples {
            previous_cpu.insert(sample.pid, sample.cpu_100ns);
            let in_foreground = foreground_name.is_some_and(|name| name == sample.name);
            if self.throttled.contains(&sample.pid) {
                if in_foreground {
                    self.throttled.remove(&sample.pid);
                    decision.restore.push(sample.pid);
                }
                continue;
            }
            let busy = self.previous_cpu.get(&sample.pid).is_some_and(|&before| {
                sample.cpu_100ns.saturating_sub(before) * 100
                    >= elapsed_100ns * HOG_CPU_PERCENT_OF_CORE
            }) && elapsed_100ns > 0;
            if !busy || in_foreground || NEVER_THROTTLE.contains(&sample.name.as_str()) {
                continue;
            }
            let count = self.streak.get(&sample.pid).copied().unwrap_or(0) + 1;
            if count >= HOG_SUSTAINED_SAMPLES {
                self.throttled.insert(sample.pid);
                decision.throttle.push(sample.pid);
            } else {
                streak.insert(sample.pid, count);
            }
        }
        self.throttled.retain(|pid| previous_cpu.contains_key(pid));
        self.previous_cpu = previous_cpu;
        self.streak = streak;
        decision
    }

    pub fn release_all(&mut self) -> Vec<u32> {
        self.previous_cpu.clear();
        self.streak.clear();
        self.throttled.drain().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERVAL: u64 = HOG_SAMPLE_INTERVAL_SECS * 10_000_000;
    const BUSY_STEP: u64 = INTERVAL / 2;

    fn sample(pid: u32, name: &str, cpu: u64) -> ProcessSample {
        ProcessSample {
            pid,
            name: name.to_string(),
            cpu_100ns: cpu,
        }
    }

    fn run(
        tracker: &mut HogTracker,
        rounds: u64,
        name: &str,
        foreground: Option<&str>,
    ) -> HogDecision {
        let mut last = HogDecision::default();
        for round in 0..=rounds {
            last = tracker.update(&[sample(7, name, round * BUSY_STEP)], INTERVAL, foreground);
        }
        last
    }

    #[test]
    fn sustained_background_load_is_throttled_once() {
        let mut tracker = HogTracker::default();
        let decision = run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            Some("code.exe"),
        );
        assert_eq!(decision.throttle, vec![7]);
        let again = tracker.update(
            &[sample(7, "sync.exe", 99 * BUSY_STEP)],
            INTERVAL,
            Some("code.exe"),
        );
        assert_eq!(again, HogDecision::default());
    }

    #[test]
    fn short_spike_is_ignored() {
        let mut tracker = HogTracker::default();
        let decision = run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES) - 1,
            "sync.exe",
            None,
        );
        assert!(decision.throttle.is_empty());
    }

    #[test]
    fn foreground_family_and_protected_names_are_exempt() {
        let mut tracker = HogTracker::default();
        let rounds = u64::from(HOG_SUSTAINED_SAMPLES) + 2;
        assert!(
            run(&mut tracker, rounds, "chrome.exe", Some("chrome.exe"))
                .throttle
                .is_empty()
        );
        let mut tracker = HogTracker::default();
        assert!(
            run(&mut tracker, rounds, "explorer.exe", None)
                .throttle
                .is_empty()
        );
    }

    #[test]
    fn idle_process_is_never_throttled() {
        let mut tracker = HogTracker::default();
        for _ in 0..10 {
            let decision = tracker.update(&[sample(7, "idle.exe", 5)], INTERVAL, None);
            assert!(decision.throttle.is_empty());
        }
    }

    #[test]
    fn throttled_process_is_restored_when_it_becomes_foreground() {
        let mut tracker = HogTracker::default();
        run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            None,
        );
        let decision = tracker.update(
            &[sample(7, "sync.exe", 99 * BUSY_STEP)],
            INTERVAL,
            Some("sync.exe"),
        );
        assert_eq!(decision.restore, vec![7]);
    }

    #[test]
    fn release_all_returns_live_throttled_pids_and_exited_ones_are_dropped() {
        let mut tracker = HogTracker::default();
        run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            None,
        );
        assert_eq!(tracker.release_all(), vec![7]);
        run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            None,
        );
        tracker.update(&[], INTERVAL, None);
        assert!(tracker.release_all().is_empty());
    }
}
