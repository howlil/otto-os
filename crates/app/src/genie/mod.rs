use std::sync::Arc;
use std::time::Duration;

use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_GENIE_DONE;
use otto_core::IconBitmap;
use otto_core::{
    GenieDirection, MotionMetrics, MotionPreference, Rect, SnapshotBudget, SnapshotCache,
    WindowIdentity, downsample_bgra_to_fit, genie_canvas,
};
use otto_platform::{
    capture_window, flush_compositor, is_shift_down, is_window_minimized, is_window_uipi,
    minimize_window_native, minimize_window_native_quiet, motion_preference, place_overlay_below,
    restore_window_native, restore_window_native_quiet, restored_window_bounds,
    set_window_transitions_enabled, virtual_screen, window_bounds, window_corner_radius_px,
    window_identity,
};
use otto_render::build_fallback_card_snapshot;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
    ShowWindow, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_MOUSEACTIVATE,
};
use windows::core::Result;

mod worker;

use self::worker::CachedSnapshot;
use self::worker::CapturedFrame;
use self::worker::GenieAnimation;
use self::worker::RenderCommand;
use self::worker::RenderJob;
use self::worker::RenderWorker;
use self::worker::WorkerShared;

const SNAPSHOT_CACHE_MAX_DIMENSION: u32 = 960;
const FIRST_FRAME0_TIMEOUT: Duration = Duration::from_secs(4);
const FRAME0_TIMEOUT: Duration = Duration::from_millis(35);
const FRAME_WAIT_TIMEOUT_MS: u32 = 50;
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(450);

pub(crate) struct Genie {
    hwnd: isize,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    snapshots: SnapshotCache<WindowIdentity, CachedSnapshot>,
    animation: Option<GenieAnimation>,
    shared: Arc<WorkerShared>,
    worker: Option<RenderWorker>,
}

#[allow(clippy::unnecessary_wraps)]
pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let genie = Genie::new(hwnd);
    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut _), SW_HIDE);
    }
    let mut slot = runtime
        .genie
        .try_borrow_mut()
        .map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    *slot = Some(genie);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    if let Ok(mut guard) = runtime.genie.try_borrow_mut()
        && let Some(mut genie) = guard.take()
    {
        genie.shutdown();
    }
}

pub fn handle_message(
    runtime: &RuntimeContext,
    _hwnd: isize,
    message: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_CLOSE => Some(LRESULT(0)),
        WM_APP_GENIE_DONE => {
            let done_generation = wparam.0 as u64;
            with_genie(runtime, |genie| {
                genie.finish_generation(done_generation);
                true
            });
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_genie(runtime, |genie| {
                genie.cancel_active_animation();
                true
            });
            Some(LRESULT(0))
        }
        _ => None,
    }
}

#[must_use]
pub fn minimize_window(runtime: &RuntimeContext, hwnd: isize) -> bool {
    minimize_window_to(runtime, hwnd, None)
}

#[must_use]
pub fn minimize_window_to(runtime: &RuntimeContext, hwnd: isize, target: Option<Rect>) -> bool {
    if !motion_preference().allows_motion() || is_window_uipi(hwnd) || !runtime.genie_enabled.get()
    {
        minimize_window_native(hwnd);
        return true;
    }
    let Some(window_id) = window_identity(hwnd) else {
        minimize_window_native(hwnd);
        return false;
    };
    let target = target
        .or_else(|| crate::dock::dock_target_for_window(runtime, hwnd))
        .unwrap_or_else(|| crate::dock::fallback_target_for_window(runtime, hwnd));
    let anchors = overlay_anchors(runtime);
    let slow_motion = is_shift_down();
    let corner_radius = window_corner_radius_px(hwnd);
    let (icon, dark_theme) = crate::dock::fallback_card_context(runtime, hwnd);

    let captured = capture_minimize_frame(hwnd, icon.as_ref(), dark_theme);
    let Some(frame) = captured else {
        minimize_window_native(hwnd);
        return true;
    };

    let started = with_genie(runtime, |genie| {
        genie.begin_minimize(
            window_id,
            target,
            &frame,
            corner_radius,
            slow_motion,
            &anchors,
        )
    });
    if !started {
        minimize_window_native(hwnd);
    }
    true
}

#[must_use]
pub fn restore_window_to(runtime: &RuntimeContext, hwnd: isize, target: Option<Rect>) -> bool {
    if !is_window_minimized(hwnd) {
        return false;
    }
    if !motion_preference().allows_motion() || is_window_uipi(hwnd) || !runtime.genie_enabled.get()
    {
        restore_window_native(hwnd);
        return true;
    }
    let Some(window_id) = window_identity(hwnd) else {
        restore_window_native(hwnd);
        return true;
    };
    let target = target
        .or_else(|| crate::dock::dock_target_for_window(runtime, hwnd))
        .unwrap_or_else(|| crate::dock::fallback_target_for_window(runtime, hwnd));
    let anchors = overlay_anchors(runtime);
    let slow_motion = is_shift_down();
    let fallback_bounds = restored_window_bounds(hwnd);
    let corner_radius = window_corner_radius_px(hwnd);
    let (icon, dark_theme) = crate::dock::fallback_card_context(runtime, hwnd);

    let started = with_genie(runtime, |genie| {
        genie.begin_restore(
            window_id,
            target,
            fallback_bounds,
            corner_radius,
            icon.as_ref(),
            dark_theme,
            slow_motion,
            &anchors,
        )
    });
    if !started {
        restore_window_native(hwnd);
    }
    true
}

fn overlay_anchors(runtime: &RuntimeContext) -> [isize; 3] {
    [
        runtime.bar_hwnd.get(),
        runtime.dock_hwnd.get(),
        runtime.window_controls_hwnd.get(),
    ]
}

fn capture_minimize_frame(
    hwnd: isize,
    icon: Option<&IconBitmap>,
    dark_theme: bool,
) -> Option<CapturedFrame> {
    if let Some(snapshot) = capture_window(hwnd) {
        return Some(CapturedFrame {
            rect: snapshot.rect,
            width: snapshot.width,
            height: snapshot.height,
            pixels: Arc::new(snapshot.pixels),
            cache_eligible: true,
        });
    }
    let bounds = window_bounds(hwnd)?;
    let icon_tuple = icon.map(|bmp| (bmp.width, bmp.height, bmp.pixels.as_slice()));
    let (width, height, pixels) =
        build_fallback_card_snapshot(bounds.width, bounds.height, icon_tuple, dark_theme)?;
    Some(CapturedFrame {
        rect: bounds,
        width,
        height,
        pixels: Arc::new(pixels),
        cache_eligible: false,
    })
}

fn with_genie(runtime: &RuntimeContext, action: impl FnOnce(&mut Genie) -> bool) -> bool {
    let Ok(mut guard) = runtime.genie.try_borrow_mut() else {
        return false;
    };
    let Some(genie) = guard.as_mut() else {
        return false;
    };
    action(genie)
}

impl Genie {
    fn new(hwnd: isize) -> Self {
        Self {
            hwnd,
            motion: motion_preference(),
            motion_metrics: MotionMetrics::default(),
            snapshots: SnapshotCache::new(SnapshotBudget::GENIE),
            animation: None,
            shared: Arc::new(WorkerShared::new()),
            worker: None,
        }
    }

    fn ensure_worker(&mut self) -> Option<Duration> {
        if let Some(worker) = &self.worker {
            return Some(if worker.shared.is_renderer_warm() {
                FRAME0_TIMEOUT
            } else {
                FIRST_FRAME0_TIMEOUT
            });
        }
        let worker = RenderWorker::spawn()?;
        self.shared = Arc::clone(&worker.shared);
        self.worker = Some(worker);
        Some(FIRST_FRAME0_TIMEOUT)
    }

    #[allow(clippy::too_many_arguments)]
    fn start_animation(
        &mut self,
        window_id: WindowIdentity,
        window: Rect,
        dock: Rect,
        corner_radius_px: f32,
        width: u32,
        height: u32,
        pixels: Arc<Vec<u8>>,
        direction: GenieDirection,
        slow_motion: bool,
        anchors: &[isize],
    ) -> bool {
        let Some(canvas) = genie_canvas(window, dock, virtual_screen()) else {
            return false;
        };
        let is_restore = matches!(direction, GenieDirection::Restore);
        let duration_seconds = self
            .motion_metrics
            .genie_duration_seconds(slow_motion, is_restore);
        let motion = motion_preference();
        self.motion = motion;

        let Some(frame0_timeout) = self.ensure_worker() else {
            return false;
        };

        let generation = self.shared.bump_generation();
        if self.hwnd != 0 {
            unsafe {
                let _ = SetWindowPos(
                    HWND(self.hwnd as *mut _),
                    None,
                    canvas.x,
                    canvas.y,
                    canvas.width,
                    canvas.height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
            let _ = place_overlay_below(self.hwnd, anchors);
        }

        let _ = set_window_transitions_enabled(window_id.hwnd, false);
        let job = RenderJob {
            generation,
            notify_hwnd: self.hwnd,
            canvas,
            window,
            dock,
            corner_radius_px,
            duration_seconds,
            direction,
            motion,
            snapshot_width: width,
            snapshot_height: height,
            snapshot_pixels: pixels,
        };
        self.shared.send_command(RenderCommand::Begin(job));

        if !self.shared.wait_frame0(generation, frame0_timeout) {
            let _ = set_window_transitions_enabled(window_id.hwnd, true);
            self.shared.send_command(RenderCommand::Clear(generation));
            return false;
        }

        if self.hwnd != 0 {
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNOACTIVATE);
            }
        }
        self.animation = Some(GenieAnimation {
            generation,
            window_id,
            direction,
        });
        true
    }

    fn begin_minimize(
        &mut self,
        window_id: WindowIdentity,
        dock: Rect,
        captured: &CapturedFrame,
        corner_radius_px: f32,
        slow_motion: bool,
        anchors: &[isize],
    ) -> bool {
        self.cancel_active_animation();
        self.prune_snapshots();

        if !self.start_animation(
            window_id,
            captured.rect,
            dock,
            corner_radius_px,
            captured.width,
            captured.height,
            Arc::clone(&captured.pixels),
            GenieDirection::Minimize,
            slow_motion,
            anchors,
        ) {
            return false;
        }

        flush_compositor();
        let _ = minimize_window_native_quiet(window_id.hwnd);

        if captured.cache_eligible
            && let Some((cw, ch, compact)) = downsample_bgra_to_fit(
                &captured.pixels,
                captured.width,
                captured.height,
                SNAPSHOT_CACHE_MAX_DIMENSION,
            )
        {
            let byte_len = compact.len();
            self.snapshots.insert(
                window_id,
                CachedSnapshot {
                    rect: captured.rect,
                    width: cw,
                    height: ch,
                    pixels: Arc::new(compact),
                },
                byte_len,
            );
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_restore(
        &mut self,
        window_id: WindowIdentity,
        dock: Rect,
        fallback_bounds: Option<Rect>,
        corner_radius_px: f32,
        icon: Option<&IconBitmap>,
        dark_theme: bool,
        slow_motion: bool,
        anchors: &[isize],
    ) -> bool {
        self.cancel_active_animation();
        self.prune_snapshots();

        let (rect, width, height, pixels) = if let Some(cached) = self.snapshots.get(&window_id) {
            (
                cached.rect,
                cached.width,
                cached.height,
                Arc::clone(&cached.pixels),
            )
        } else {
            let Some(rect) = fallback_bounds else {
                return false;
            };
            let icon_tuple = icon.map(|bmp| (bmp.width, bmp.height, bmp.pixels.as_slice()));
            let Some((width, height, card_pixels)) =
                build_fallback_card_snapshot(rect.width, rect.height, icon_tuple, dark_theme)
            else {
                return false;
            };
            (rect, width, height, Arc::new(card_pixels))
        };

        self.start_animation(
            window_id,
            rect,
            dock,
            corner_radius_px,
            width,
            height,
            pixels,
            GenieDirection::Restore,
            slow_motion,
            anchors,
        )
    }

    fn finish_generation(&mut self, generation: u64) {
        let Some(animation) = self.animation else {
            return;
        };
        if animation.generation != generation {
            return;
        }
        self.animation = None;
        self.settle_native_state(animation);
        self.clear_overlay(animation.generation);
    }

    fn settle_native_state(&mut self, animation: GenieAnimation) {
        let hwnd = animation.window_id.hwnd;
        match animation.direction {
            GenieDirection::Minimize => {
                if !is_window_minimized(hwnd) {
                    minimize_window_native(hwnd);
                }
                let _ = set_window_transitions_enabled(hwnd, true);
            }
            GenieDirection::Restore => {
                self.snapshots.remove(&animation.window_id);
                if is_window_minimized(hwnd) && !restore_window_native_quiet(hwnd) {
                    restore_window_native(hwnd);
                }
                flush_compositor();
                let _ = set_window_transitions_enabled(hwnd, true);
            }
        }
    }

    fn cancel_active_animation(&mut self) {
        if let Some(animation) = self.animation.take() {
            let next_generation = self.shared.bump_generation();
            self.settle_native_state(animation);
            self.clear_overlay(next_generation);
        }
    }

    fn prune_snapshots(&mut self) {
        self.snapshots.retain(|window_id| {
            window_identity(window_id.hwnd).as_ref() == Some(window_id)
                && is_window_minimized(window_id.hwnd)
        });
    }

    fn clear_overlay(&mut self, generation: u64) {
        self.shared.send_command(RenderCommand::Clear(generation));
        if self.hwnd != 0 {
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            }
        }
    }

    fn stop_worker(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
    }

    fn shutdown(&mut self) {
        self.cancel_active_animation();
        self.stop_worker();
        if self.hwnd != 0 {
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            }
        }
        self.snapshots.clear();
    }
}

impl Drop for Genie {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests;
