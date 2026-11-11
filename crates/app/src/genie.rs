use std::collections::HashMap;
use std::time::Instant;

use crate::host::RuntimeContext;
use otto_core::{
    GenieDirection, MotionMetrics, MotionPreference, Rect, advance_genie_progress,
    compute_genie_geometry,
};
use otto_platform::{
    WindowSnapshot, capture_window, flush_compositor, is_window_minimized, minimize_window_native,
    minimize_window_native_quiet, motion_preference, primary_screen, restore_window_native,
    restore_window_native_quiet, set_window_cloaked, set_window_transitions_enabled,
};
use otto_render::GenieRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SetTimer, ShowWindow, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_MOUSEACTIVATE, WM_TIMER,
};
use windows::core::Result;

const GENIE_TIMER: usize = 30;
const MAX_GENIE_SNAPSHOTS: usize = 2;

#[derive(Debug, Clone, Copy)]
struct GenieAnimation {
    hwnd: isize,
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
    snapshots: HashMap<isize, WindowSnapshot>,
    snapshot_order: Vec<isize>,
    animation: Option<GenieAnimation>,
    last_tick: Instant,
    timer_running: bool,
}

#[allow(clippy::unnecessary_wraps)]
pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let genie = Genie {
        hwnd,
        renderer: None,
        motion: motion_preference(),
        motion_metrics: MotionMetrics::default(),
        snapshots: HashMap::new(),
        snapshot_order: Vec::new(),
        animation: None,
        last_tick: Instant::now(),
        timer_running: false,
    };
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
    let Some(snapshot) = capture_window(hwnd) else {
        minimize_window_native(hwnd);
        return true;
    };
    let Some(target) = target.or_else(|| crate::dock::dock_target_for_window(runtime, hwnd)) else {
        minimize_window_native(hwnd);
        return true;
    };
    if !with_genie(runtime, |genie| {
        genie.begin_minimize(hwnd, &snapshot, target)
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
    let Some(target) = target.or_else(|| crate::dock::dock_target_for_window(runtime, hwnd)) else {
        restore_window_native(hwnd);
        return true;
    };
    let started = with_genie(runtime, |genie| genie.begin_restore(hwnd, target));
    if !started {
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
    fn ensure_renderer(&mut self) -> Result<()> {
        if self.renderer.is_none() {
            let screen = primary_screen();
            self.renderer = Some(GenieRenderer::new(screen.width, screen.height)?);
        }
        Ok(())
    }

    fn store_snapshot(&mut self, hwnd: isize, snapshot: WindowSnapshot) {
        self.snapshots.insert(hwnd, snapshot);
        self.touch_snapshot(hwnd);
        while self.snapshot_order.len() > MAX_GENIE_SNAPSHOTS {
            let oldest = self.snapshot_order.remove(0);
            self.remove_snapshot(oldest);
        }
    }

    fn touch_snapshot(&mut self, hwnd: isize) {
        self.snapshot_order.retain(|&id| id != hwnd);
        if self.snapshots.contains_key(&hwnd) {
            self.snapshot_order.push(hwnd);
        }
    }

    fn remove_snapshot(&mut self, hwnd: isize) {
        self.snapshots.remove(&hwnd);
        self.snapshot_order.retain(|&id| id != hwnd);
    }

    fn begin_minimize(&mut self, hwnd: isize, snapshot: &WindowSnapshot, dock: Rect) -> bool {
        self.cancel_active_animation();
        self.prune_snapshots();
        self.motion = motion_preference();
        if !self.motion.allows_motion() {
            minimize_window_native(hwnd);
            return true;
        }
        if self.ensure_renderer().is_err() {
            return false;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return false;
        };
        if renderer
            .set_snapshot(snapshot.width, snapshot.height, &snapshot.pixels)
            .is_err()
        {
            return false;
        }

        let animation = GenieAnimation {
            hwnd,
            window: snapshot.rect,
            dock,
            progress: 0.0,
            direction: GenieDirection::Minimize,
        };
        if !self.present_initial(animation) || !self.arm_timer() {
            self.clear_overlay();
            return false;
        }

        if !set_window_cloaked(hwnd, true) {
            self.clear_overlay();
            return false;
        }
        self.animation = Some(animation);
        minimize_window_native_quiet(hwnd);
        self.store_snapshot(hwnd, snapshot.clone());
        true
    }

    fn begin_restore(&mut self, hwnd: isize, dock: Rect) -> bool {
        self.cancel_active_animation();
        self.prune_snapshots();
        self.motion = motion_preference();
        let Some(snapshot) = self.snapshots.get(&hwnd).cloned() else {
            return false;
        };
        self.touch_snapshot(hwnd);
        if !self.motion.allows_motion() {
            self.remove_snapshot(hwnd);
            return false;
        }
        if self.ensure_renderer().is_err() {
            self.remove_snapshot(hwnd);
            return false;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            self.remove_snapshot(hwnd);
            return false;
        };
        if renderer
            .set_snapshot(snapshot.width, snapshot.height, &snapshot.pixels)
            .is_err()
        {
            self.remove_snapshot(hwnd);
            return false;
        }

        let animation = GenieAnimation {
            hwnd,
            window: snapshot.rect,
            dock,
            progress: 0.0,
            direction: GenieDirection::Restore,
        };
        if !self.present_initial(animation) || !self.arm_timer() {
            self.clear_overlay();
            return false;
        }

        if !set_window_cloaked(hwnd, true) {
            self.clear_overlay();
            return false;
        }
        self.animation = Some(animation);
        restore_window_native_quiet(hwnd);
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

    fn finish(&mut self, animation: GenieAnimation) {
        let _ = set_window_cloaked(animation.hwnd, false);
        set_window_transitions_enabled(animation.hwnd, true);
        self.settle_native_state(animation);
        self.animation = None;
        self.clear_overlay();
    }

    fn cancel_active_animation(&mut self) {
        if let Some(animation) = self.animation.take() {
            let _ = set_window_cloaked(animation.hwnd, false);
            set_window_transitions_enabled(animation.hwnd, true);
            self.settle_native_state(animation);
        }
        self.clear_overlay();
    }

    fn settle_native_state(&mut self, animation: GenieAnimation) {
        let minimized = is_window_minimized(animation.hwnd);
        match (animation.direction, minimized) {
            (GenieDirection::Minimize, false) => {
                self.remove_snapshot(animation.hwnd);
                minimize_window_native(animation.hwnd);
            }
            (GenieDirection::Restore, true) => restore_window_native(animation.hwnd),
            (GenieDirection::Restore, false) => {
                self.remove_snapshot(animation.hwnd);
            }
            (GenieDirection::Minimize, true) => {}
        }
    }

    fn prune_snapshots(&mut self) {
        self.snapshots.retain(|hwnd, _| is_window_minimized(*hwnd));
        self.snapshot_order
            .retain(|hwnd| self.snapshots.contains_key(hwnd));
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
        self.renderer = None;
    }

    fn present(&mut self, animation: GenieAnimation) -> Result<()> {
        let geometry = compute_genie_geometry(
            animation.window,
            animation.dock,
            animation.progress,
            animation.direction,
        );
        self.ensure_renderer()?;
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.present(self.hwnd, &geometry)
        } else {
            Ok(())
        }
    }

    fn shutdown(&mut self) {
        self.cancel_active_animation();
        self.snapshots.clear();
        self.snapshot_order.clear();
        self.renderer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_genie() -> Genie {
        Genie {
            hwnd: 0,
            renderer: None,
            motion: MotionPreference::Full,
            motion_metrics: MotionMetrics::default(),
            snapshots: HashMap::new(),
            snapshot_order: Vec::new(),
            animation: None,
            last_tick: Instant::now(),
            timer_running: false,
        }
    }

    fn snapshot() -> WindowSnapshot {
        WindowSnapshot {
            rect: Rect::default(),
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        }
    }

    #[test]
    fn snapshot_cache_is_bounded_and_evicts_least_recent_entry() {
        let mut genie = test_genie();
        genie.store_snapshot(1, snapshot());
        genie.store_snapshot(2, snapshot());
        genie.touch_snapshot(1);
        genie.store_snapshot(3, snapshot());

        assert!(genie.snapshots.contains_key(&1));
        assert!(!genie.snapshots.contains_key(&2));
        assert!(genie.snapshots.contains_key(&3));
        assert_eq!(genie.snapshot_order, vec![1, 3]);
    }
}
