use std::time::Instant;

use crate::host::RuntimeContext;
use otto_core::{
    GenieDirection, MotionMetrics, MotionPreference, Rect, SnapshotBudget, SnapshotCache,
    WindowIdentity, advance_genie_progress, compute_genie_geometry, genie_canvas,
};
use otto_platform::{
    WindowSnapshot, capture_window, flush_compositor, is_window_minimized, minimize_window_native,
    minimize_window_native_quiet, motion_preference, restore_window_native,
    restore_window_native_quiet, set_window_transitions_enabled, virtual_screen, window_identity,
};
use otto_render::GenieRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SetTimer, ShowWindow, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_MOUSEACTIVATE, WM_TIMER,
};
use windows::core::Result;

const GENIE_TIMER: usize = 30;

#[derive(Debug, Clone, Copy)]
struct GenieAnimation {
    window_id: WindowIdentity,
    window: Rect,
    dock: Rect,
    progress: f32,
    direction: GenieDirection,
}

pub(crate) struct Genie {
    hwnd: isize,
    renderer: Option<GenieRenderer>,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    snapshots: SnapshotCache<WindowIdentity, WindowSnapshot>,
    animation: Option<GenieAnimation>,
    last_tick: Instant,
    timer_running: bool,
}

#[allow(clippy::unnecessary_wraps)]
pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let genie = Genie::new(hwnd);
    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut _), SW_HIDE);
    }
    *runtime.genie.borrow_mut() = Some(genie);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    if let Some(mut genie) = runtime.genie.borrow_mut().take() {
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
        WM_TIMER if wparam.0 == GENIE_TIMER => {
            with_genie(runtime, |genie| {
                genie.on_timer();
                true
            });
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_genie(runtime, |genie| {
                genie.rebuild();
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
    if !motion_preference().allows_motion() {
        minimize_window_native(hwnd);
        return true;
    }
    let Some(target) = target.or_else(|| crate::dock::dock_target_for_window(runtime, hwnd)) else {
        minimize_window_native(hwnd);
        return true;
    };
    let (Some(window_id), Some(snapshot)) = (window_identity(hwnd), capture_window(hwnd)) else {
        minimize_window_native(hwnd);
        return true;
    };
    if !with_genie(runtime, |genie| {
        genie.begin_minimize(window_id, snapshot, target)
    }) {
        minimize_window_native(hwnd);
    }
    true
}

#[must_use]
pub fn restore_window_to(runtime: &RuntimeContext, hwnd: isize, target: Option<Rect>) -> bool {
    if !is_window_minimized(hwnd) {
        return false;
    }
    if !motion_preference().allows_motion() {
        restore_window_native(hwnd);
        return true;
    }
    let (Some(target), Some(window_id)) = (
        target.or_else(|| crate::dock::dock_target_for_window(runtime, hwnd)),
        window_identity(hwnd),
    ) else {
        restore_window_native(hwnd);
        return true;
    };
    if !with_genie(runtime, |genie| genie.begin_restore(window_id, target)) {
        restore_window_native(hwnd);
    }
    true
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
            renderer: None,
            motion: motion_preference(),
            motion_metrics: MotionMetrics::default(),
            snapshots: SnapshotCache::new(SnapshotBudget::GENIE),
            animation: None,
            last_tick: Instant::now(),
            timer_running: false,
        }
    }

    fn prepare_renderer(&mut self, window: Rect, dock: Rect) -> bool {
        let Some(canvas) = genie_canvas(window, dock, virtual_screen()) else {
            return false;
        };
        if self
            .renderer
            .as_ref()
            .is_some_and(|renderer| renderer.canvas() == canvas)
        {
            return true;
        }
        self.renderer = GenieRenderer::new(canvas).ok();
        self.renderer.is_some()
    }

    fn upload(&mut self, snapshot: &WindowSnapshot, dock: Rect) -> bool {
        if !self.prepare_renderer(snapshot.rect, dock) {
            return false;
        }
        self.renderer.as_mut().is_some_and(|renderer| {
            renderer
                .set_snapshot(snapshot.width, snapshot.height, &snapshot.pixels)
                .is_ok()
        })
    }

    fn start_overlay(&mut self, animation: GenieAnimation) -> bool {
        let hwnd = animation.window_id.hwnd;
        if !set_window_transitions_enabled(hwnd, false) {
            return false;
        }
        if !self.present_initial(animation) || !self.arm_timer() {
            let _ = set_window_transitions_enabled(hwnd, true);
            self.clear_overlay();
            return false;
        }
        self.animation = Some(animation);
        true
    }

    fn begin_minimize(
        &mut self,
        window_id: WindowIdentity,
        snapshot: WindowSnapshot,
        dock: Rect,
    ) -> bool {
        self.cancel_active_animation();
        self.prune_snapshots();
        self.motion = motion_preference();
        if !self.upload(&snapshot, dock) {
            return false;
        }
        let animation = GenieAnimation {
            window_id,
            window: snapshot.rect,
            dock,
            progress: 0.0,
            direction: GenieDirection::Minimize,
        };
        if !self.start_overlay(animation) {
            return false;
        }
        let _ = minimize_window_native_quiet(window_id.hwnd);
        let bytes = snapshot.pixels.len();
        let _ = self.snapshots.insert(window_id, snapshot, bytes);
        true
    }

    fn begin_restore(&mut self, window_id: WindowIdentity, dock: Rect) -> bool {
        self.cancel_active_animation();
        self.prune_snapshots();
        self.motion = motion_preference();
        let Some(snapshot) = self.snapshots.get(&window_id).cloned() else {
            return false;
        };
        if !self.upload(&snapshot, dock) {
            self.snapshots.remove(&window_id);
            return false;
        }
        let animation = GenieAnimation {
            window_id,
            window: snapshot.rect,
            dock,
            progress: 0.0,
            direction: GenieDirection::Restore,
        };
        if !self.start_overlay(animation) {
            self.snapshots.remove(&window_id);
            return false;
        }
        true
    }

    fn present_initial(&mut self, animation: GenieAnimation) -> bool {
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNOACTIVATE);
        }
        if self.present(animation).is_err() {
            return false;
        }
        flush_compositor();
        true
    }

    fn on_timer(&mut self) {
        let Some(animation) = self.animation.as_mut() else {
            self.disarm_timer();
            return;
        };
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_tick = now;
        let duration = match animation.direction {
            GenieDirection::Minimize => self.motion_metrics.genie_minimize_duration_seconds,
            GenieDirection::Restore => self.motion_metrics.genie_restore_duration_seconds,
        };
        animation.progress = advance_genie_progress(animation.progress, dt, duration, self.motion);
        let state = *animation;
        if self.present(state).is_err() || state.progress >= 1.0 {
            self.finish(state);
        }
    }

    fn settle_native_state(&mut self, animation: GenieAnimation) {
        let hwnd = animation.window_id.hwnd;
        match animation.direction {
            GenieDirection::Minimize => {
                let _ = set_window_transitions_enabled(hwnd, true);
                if !is_window_minimized(hwnd) {
                    self.snapshots.remove(&animation.window_id);
                    minimize_window_native(hwnd);
                }
            }
            GenieDirection::Restore => {
                let _ = restore_window_native_quiet(hwnd);
                flush_compositor();
                let _ = set_window_transitions_enabled(hwnd, true);
                if is_window_minimized(hwnd) {
                    restore_window_native(hwnd);
                }
                self.snapshots.remove(&animation.window_id);
            }
        }
    }

    fn finish(&mut self, animation: GenieAnimation) {
        self.animation = None;
        self.settle_native_state(animation);
        self.clear_overlay();
    }

    fn cancel_active_animation(&mut self) {
        if let Some(animation) = self.animation {
            self.finish(animation);
        }
    }

    fn prune_snapshots(&mut self) {
        self.snapshots.retain(|window_id| {
            window_identity(window_id.hwnd).as_ref() == Some(window_id)
                && is_window_minimized(window_id.hwnd)
        });
    }

    fn arm_timer(&mut self) -> bool {
        if self.timer_running {
            return true;
        }
        self.last_tick = Instant::now();
        let timer = unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                GENIE_TIMER,
                self.motion_metrics.animation_tick_ms,
                None,
            )
        };
        self.timer_running = timer != 0;
        self.timer_running
    }

    fn disarm_timer(&mut self) {
        if !self.timer_running {
            return;
        }
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), GENIE_TIMER);
        }
        self.timer_running = false;
    }

    fn clear_overlay(&mut self) {
        self.disarm_timer();
        self.renderer = None;
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
        }
        otto_platform::trim_working_set();
    }

    fn rebuild(&mut self) {
        self.cancel_active_animation();
        self.snapshots.clear();
        self.renderer = None;
        self.motion = motion_preference();
    }

    fn present(&mut self, animation: GenieAnimation) -> Result<()> {
        let geometry = compute_genie_geometry(
            animation.window,
            animation.dock,
            animation.progress,
            animation.direction,
        );
        match self.renderer.as_ref() {
            Some(renderer) => renderer.present(self.hwnd, &geometry),
            None => Err(windows::core::Error::from_hresult(
                windows::Win32::Foundation::E_FAIL,
            )),
        }
    }

    fn shutdown(&mut self) {
        self.cancel_active_animation();
        self.snapshots.clear();
        self.renderer = None;
        self.disarm_timer();
    }
}

#[cfg(test)]
mod tests {
    use super::Genie;
    use otto_core::WindowIdentity;

    fn missing_window() -> WindowIdentity {
        WindowIdentity {
            hwnd: 0x7FFF_FFF0,
            process_id: 0xFFFF_FFF0,
            thread_id: 0xFFFF_FFF1,
            generation: 1,
        }
    }

    #[test]
    fn restore_without_a_cached_snapshot_falls_back_to_native() {
        let mut genie = Genie::new(0);
        let dock = otto_core::Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 48,
        };
        assert!(!genie.begin_restore(missing_window(), dock));
        assert!(genie.animation.is_none());
    }

    #[test]
    fn shutdown_without_animation_leaves_nothing_running() {
        let mut genie = Genie::new(0);
        genie.shutdown();
        assert!(genie.animation.is_none());
        assert!(!genie.timer_running);
        assert!(genie.snapshots.is_empty());
    }
}
