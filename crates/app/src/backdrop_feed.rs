use std::ffi::c_void;
use std::time::Instant;

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
    hidden_windows: Vec<isize>,
    hiding: bool,
}

impl BackdropFeed {
    pub(crate) fn start(hwnd: isize, wake_message: u32, hidden_windows: Vec<isize>) -> Self {
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
            hidden_windows,
            hiding: false,
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
        let frame = self.worker.take_frame()?;
        let width = u32::try_from(frame.pixels.width()).ok()?;
        let height = u32::try_from(frame.pixels.height()).ok()?;
        self.gate
            .accepts(frame.origin, width, height, frame.signature)
            .then_some(BackdropUpdate {
                pixels: frame.pixels,
                origin: frame.origin,
            })
    }

    pub(crate) fn reset(&mut self) {
        self.gate.reset();
        self.set_hiding(false);
    }
}

impl Drop for BackdropFeed {
    fn drop(&mut self) {
        self.set_hiding(false);
    }
}
