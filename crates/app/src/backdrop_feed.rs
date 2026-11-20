use std::ffi::c_void;
use std::time::Instant;

const CHANGE_MEMORY_MS: u64 = 1_500;

use otto_core::{CaptureGate, CaptureRegion, RgbBuffer};
use otto_platform::{BackdropRequest, BackdropWorker};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub(crate) struct BackdropUpdate {
    pub(crate) pixels: RgbBuffer,
    pub(crate) origin: (i32, i32),
}

pub(crate) struct BackdropFeed {
    worker: BackdropWorker,
    gate: CaptureGate,
    epoch: Instant,
    next_id: u64,
    last_change_ms: Option<u64>,
    hidden_windows: Vec<isize>,
    hiding: bool,
    capture_visible: bool,
}

impl BackdropFeed {
    pub(crate) fn start(
        hwnd: isize,
        wake_message: u32,
        hidden_windows: Vec<isize>,
        capture_visible: bool,
    ) -> Self {
        let worker = BackdropWorker::spawn(move || unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd as *mut c_void)),
                wake_message,
                WPARAM(0),
                LPARAM(0),
            );
        });
        Self {
            worker,
            gate: CaptureGate::new(),
            epoch: Instant::now(),
            next_id: 0,
            last_change_ms: None,
            hidden_windows,
            hiding: false,
            capture_visible,
        }
    }

    fn set_hiding(&mut self, hide: bool) {
        if self.hiding == hide {
            return;
        }
        self.hiding = hide;
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
        self.capture_visible = visible;
        if !visible && self.gate.is_active() {
            self.set_hiding(true);
        }
    }

    pub(crate) fn recently_changed(&self) -> bool {
        self.last_change_ms.is_some_and(|changed| {
            (self.epoch.elapsed().as_millis() as u64).saturating_sub(changed) < CHANGE_MEMORY_MS
        })
    }

    pub(crate) fn request(&mut self, region: CaptureRegion, interval_ms: u32, force: bool) {
        let now_ms = self.epoch.elapsed().as_millis() as u64;
        if !self.gate.should_request(region, now_ms, interval_ms, force) {
            return;
        }
        self.set_hiding(true);
        self.next_id += 1;
        self.worker.request(BackdropRequest {
            id: self.next_id,
            origin: region.origin(),
            width: region.width,
            height: region.height,
        });
    }

    pub(crate) fn poll(&mut self) -> Option<BackdropUpdate> {
        let frame = self.worker.take_frame();
        if self.capture_visible && !self.worker.is_capturing() {
            self.set_hiding(false);
        }
        let frame = frame?;
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
        self.set_hiding(false);
    }
}

impl Drop for BackdropFeed {
    fn drop(&mut self) {
        self.set_hiding(false);
    }
}
