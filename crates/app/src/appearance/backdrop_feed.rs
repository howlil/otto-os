use std::time::Instant;

const CHANGE_MEMORY_MS: u64 = 1_500;

use otto_core::{CaptureGate, CaptureRegion, RgbBuffer};
use otto_platform::{BackdropRequest, BackdropWorker, post_window_message};

pub(crate) struct BackdropUpdate {
    pub(crate) pixels: RgbBuffer,
    pub(crate) origin: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CaptureAffinityPolicy {
    capture_visible: bool,
    hiding: bool,
}

impl CaptureAffinityPolicy {
    fn new(capture_visible: bool) -> (Self, Option<bool>) {
        let mut policy = Self {
            capture_visible,
            hiding: false,
        };
        let transition = policy.sync_hiding(!capture_visible);
        (policy, transition)
    }

    fn sync_hiding(&mut self, hide: bool) -> Option<bool> {
        if self.hiding == hide {
            return None;
        }
        self.hiding = hide;
        Some(hide)
    }

    fn set_capture_visible(&mut self, visible: bool) -> Option<bool> {
        self.capture_visible = visible;
        self.sync_hiding(!visible)
    }

    const fn allows_live_capture(self) -> bool {
        !self.capture_visible
    }

    fn release(&mut self) -> Option<bool> {
        self.sync_hiding(false)
    }
}

pub(crate) struct BackdropFeed {
    worker: BackdropWorker,
    gate: CaptureGate,
    epoch: Instant,
    next_id: u64,
    last_change_ms: Option<u64>,
    hidden_windows: Vec<isize>,
    policy: CaptureAffinityPolicy,
}

impl BackdropFeed {
    pub(crate) fn start(
        hwnd: isize,
        wake_message: u32,
        hidden_windows: Vec<isize>,
        capture_visible: bool,
    ) -> Self {
        let worker = BackdropWorker::spawn(move || {
            let _ = post_window_message(hwnd, wake_message, 0, 0);
        });
        let (policy, initial_hide) = CaptureAffinityPolicy::new(capture_visible);
        let feed = Self {
            worker,
            gate: CaptureGate::new(),
            epoch: Instant::now(),
            next_id: 0,
            last_change_ms: None,
            hidden_windows,
            policy,
        };
        if let Some(hide) = initial_hide {
            feed.apply_affinity(hide);
        }
        feed
    }

    fn apply_affinity(&self, hide: bool) {
        for hwnd in &self.hidden_windows {
            let _ = if hide {
                otto_platform::exclude_from_capture(*hwnd)
            } else {
                otto_platform::include_in_capture(*hwnd)
            };
        }
    }

    pub(crate) const fn is_active(&self) -> bool {
        self.gate.is_active()
    }

    pub(crate) fn set_capture_visible(&mut self, visible: bool) {
        if let Some(hide) = self.policy.set_capture_visible(visible) {
            self.apply_affinity(hide);
        }
        self.gate.reset();
        self.last_change_ms = None;
    }

    pub(crate) fn recently_changed(&self) -> bool {
        self.last_change_ms.is_some_and(|changed| {
            (self.epoch.elapsed().as_millis() as u64).saturating_sub(changed) < CHANGE_MEMORY_MS
        })
    }

    pub(crate) fn request(&mut self, region: CaptureRegion, interval_ms: u32, force: bool) {
        if !self.policy.allows_live_capture() {
            return;
        }
        let now_ms = self.epoch.elapsed().as_millis() as u64;
        if !self.gate.should_request(region, now_ms, interval_ms, force) {
            return;
        }
        if let Some(hide) = self.policy.sync_hiding(true) {
            self.apply_affinity(hide);
        }
        self.next_id += 1;
        self.worker.request(BackdropRequest {
            id: self.next_id,
            origin: region.origin(),
            width: region.width,
            height: region.height,
        });
    }

    pub(crate) fn poll(&mut self) -> Option<BackdropUpdate> {
        let frame = self.worker.take_frame()?;
        if !self.policy.allows_live_capture() {
            return None;
        }
        let width = u32::try_from(frame.pixels.width()).ok()?;
        let height = u32::try_from(frame.pixels.height()).ok()?;
        let accepted = self
            .gate
            .accepts(frame.origin, width, height, frame.signature);
        if accepted {
            self.last_change_ms = Some(self.epoch.elapsed().as_millis() as u64);
        }
        accepted.then_some(BackdropUpdate {
            pixels: frame.pixels,
            origin: frame.origin,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.gate.reset();
        self.last_change_ms = None;
    }
}

impl Drop for BackdropFeed {
    fn drop(&mut self) {
        if let Some(hide) = self.policy.release() {
            self.apply_affinity(hide);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CaptureAffinityPolicy;

    #[test]
    fn capture_visible_never_excludes_and_blocks_live_capture() {
        let (mut policy, initial) = CaptureAffinityPolicy::new(true);
        assert_eq!(initial, None);
        assert!(!policy.allows_live_capture());
        assert_eq!(policy.set_capture_visible(true), None);
        assert_eq!(policy.release(), None);
    }

    #[test]
    fn capture_hidden_excludes_immediately_and_stays_steady_until_toggle_or_drop() {
        let (mut policy, initial) = CaptureAffinityPolicy::new(false);
        assert_eq!(initial, Some(true));
        assert!(policy.allows_live_capture());
        assert_eq!(policy.sync_hiding(true), None);
        assert_eq!(policy.set_capture_visible(true), Some(false));
        assert!(!policy.allows_live_capture());
        assert_eq!(policy.set_capture_visible(false), Some(true));
        assert!(policy.allows_live_capture());
        assert_eq!(policy.release(), Some(false));
        assert_eq!(policy.release(), None);
    }
}
