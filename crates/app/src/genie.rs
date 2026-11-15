use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::host::RuntimeContext;
use otto_core::{
    GenieDirection, MotionMetrics, MotionPreference, Rect, SnapshotBudget, SnapshotCache,
    WindowIdentity, compute_genie_geometry, downsample_bgra_to_fit, genie_canvas,
};
use otto_platform::{
    ComGuard, IconBitmap, capture_window, flush_compositor, is_shift_down, is_window_minimized,
    is_window_uipi, join_within, minimize_window_native, minimize_window_native_quiet,
    motion_preference, place_overlay_below, restore_window_native, restore_window_native_quiet,
    restored_window_bounds, set_window_transitions_enabled, virtual_screen, window_bounds,
    window_corner_radius_px, window_identity,
};
use otto_render::{GenieRenderer, build_fallback_card_snapshot};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MA_NOACTIVATE, PostMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER,
    SetWindowPos, ShowWindow, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_MOUSEACTIVATE,
};
use windows::core::Result;

const WM_APP_GENIE_DONE: u32 = 0x8000 + 27;
const SNAPSHOT_CACHE_MAX_DIMENSION: u32 = 960;
const FIRST_FRAME0_TIMEOUT: Duration = Duration::from_secs(2);
const FRAME0_TIMEOUT: Duration = Duration::from_millis(35);
const FRAME_WAIT_TIMEOUT_MS: u32 = 50;
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(450);

#[derive(Clone)]
struct CachedSnapshot {
    rect: Rect,
    width: u32,
    height: u32,
    pixels: Arc<Vec<u8>>,
}

struct CapturedFrame {
    rect: Rect,
    width: u32,
    height: u32,
    pixels: Arc<Vec<u8>>,
    cache_eligible: bool,
}

#[derive(Debug, Clone, Copy)]
struct GenieAnimation {
    generation: u64,
    window_id: WindowIdentity,
    direction: GenieDirection,
}

struct RenderJob {
    generation: u64,
    notify_hwnd: isize,
    canvas: Rect,
    window: Rect,
    dock: Rect,
    corner_radius_px: f32,
    duration_seconds: f32,
    direction: GenieDirection,
    motion: MotionPreference,
    snapshot_width: u32,
    snapshot_height: u32,
    snapshot_pixels: Arc<Vec<u8>>,
}

enum RenderCommand {
    Begin(RenderJob),
    Clear(u64),
    Shutdown,
}

struct WorkerState {
    generation: u64,
    pending: Option<RenderCommand>,
    frame0_generation: u64,
    frame0_ok: bool,
    renderer_warm: bool,
    stopping: bool,
}

struct WorkerShared {
    state: Mutex<WorkerState>,
    wake_worker: Condvar,
    wake_ui: Condvar,
}

impl WorkerShared {
    fn new() -> Self {
        Self {
            state: Mutex::new(WorkerState {
                generation: 0,
                pending: None,
                frame0_generation: 0,
                frame0_ok: false,
                renderer_warm: false,
                stopping: false,
            }),
            wake_worker: Condvar::new(),
            wake_ui: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, WorkerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn bump_generation(&self) -> u64 {
        let mut guard = self.lock();
        guard.generation = guard.generation.saturating_add(1);
        guard.generation
    }

    fn send_command(&self, mut command: RenderCommand) {
        let mut guard = self.lock();
        match &mut command {
            RenderCommand::Begin(job) => guard.generation = job.generation,
            RenderCommand::Clear(generation) => {
                if guard.stopping || *generation < guard.generation {
                    return;
                }
                if *generation == guard.generation {
                    *generation = guard.generation.saturating_add(1);
                }
                guard.generation = *generation;
            }
            RenderCommand::Shutdown => {
                guard.generation = guard.generation.saturating_add(1);
                guard.stopping = true;
            }
        }
        guard.pending = Some(command);
        self.wake_worker.notify_all();
    }

    fn wait_frame0(&self, generation: u64, timeout: Duration) -> bool {
        let mut guard = self.lock();
        let deadline = Instant::now() + timeout;
        while guard.frame0_generation < generation && !guard.stopping {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next_guard, result) = self
                .wake_ui
                .wait_timeout(guard, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            guard = next_guard;
            if result.timed_out() {
                break;
            }
        }
        guard.frame0_generation >= generation && guard.frame0_ok
    }

    fn publish_frame0(&self, generation: u64, ok: bool) {
        let mut guard = self.lock();
        if ok {
            guard.renderer_warm = true;
        }
        if generation >= guard.frame0_generation {
            guard.frame0_generation = generation;
            guard.frame0_ok = ok;
            self.wake_ui.notify_all();
        }
    }

    fn mark_renderer_cold(&self) {
        self.lock().renderer_warm = false;
    }

    fn is_renderer_warm(&self) -> bool {
        self.lock().renderer_warm
    }

    fn is_superseded(&self, generation: u64) -> bool {
        let guard = self.lock();
        guard.stopping || guard.generation != generation
    }

    fn next_command(&self) -> Option<RenderCommand> {
        let mut guard = self.lock();
        loop {
            if let Some(command) = guard.pending.take() {
                return Some(command);
            }
            if guard.stopping {
                return None;
            }
            guard = self
                .wake_worker
                .wait(guard)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

struct RenderWorker {
    shared: Arc<WorkerShared>,
    handle: Option<JoinHandle<()>>,
}

impl RenderWorker {
    fn spawn() -> Option<Self> {
        let shared = Arc::new(WorkerShared::new());
        let worker_shared = Arc::clone(&shared);
        let handle = thread::Builder::new()
            .name(String::from("otto-genie-render"))
            .spawn(move || worker_main(&worker_shared))
            .ok()?;
        Some(Self {
            shared,
            handle: Some(handle),
        })
    }

    fn stop(&mut self) {
        self.shared.send_command(RenderCommand::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = join_within(handle, WORKER_SHUTDOWN_TIMEOUT);
        }
    }
}

impl Drop for RenderWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn prepare_renderer(
    renderer: &mut Option<GenieRenderer>,
    bound_hwnd: &mut isize,
    job: &RenderJob,
) -> Result<()> {
    if renderer.is_none() {
        let mut created = GenieRenderer::new(job.canvas)?;
        if job.notify_hwnd != 0 {
            created.bind_window(job.notify_hwnd)?;
            *bound_hwnd = job.notify_hwnd;
        }
        *renderer = Some(created);
    }
    let active = renderer.as_mut().expect("renderer initialized above");
    if job.notify_hwnd != 0 && *bound_hwnd != job.notify_hwnd {
        active.bind_window(job.notify_hwnd)?;
        *bound_hwnd = job.notify_hwnd;
    }
    active.retarget_canvas(job.canvas)?;
    active.set_snapshot(
        job.snapshot_width,
        job.snapshot_height,
        &job.snapshot_pixels,
    )?;
    Ok(())
}

fn post_worker_done(notify_hwnd: isize, generation: u64) {
    if notify_hwnd == 0 {
        return;
    }
    if let Ok(wparam) = usize::try_from(generation) {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(notify_hwnd as *mut _)),
                WM_APP_GENIE_DONE,
                WPARAM(wparam),
                LPARAM(0),
            );
        }
    }
}

fn run_job(
    shared: &WorkerShared,
    renderer: &mut Option<GenieRenderer>,
    bound_hwnd: &mut isize,
    job: &RenderJob,
) {
    if shared.is_superseded(job.generation) {
        shared.publish_frame0(job.generation, false);
        return;
    }
    if prepare_renderer(renderer, bound_hwnd, job).is_err() {
        *renderer = None;
        *bound_hwnd = 0;
        shared.mark_renderer_cold();
        shared.publish_frame0(job.generation, false);
        return;
    }
    let Some(active) = renderer.as_mut() else {
        shared.mark_renderer_cold();
        shared.publish_frame0(job.generation, false);
        return;
    };

    let initial_progress = if job.motion.allows_motion() { 0.0 } else { 1.0 };
    let initial = compute_genie_geometry(job.window, job.dock, initial_progress, job.direction);
    if active
        .present_frame(&initial, job.window, job.corner_radius_px, false, true)
        .is_err()
    {
        *renderer = None;
        *bound_hwnd = 0;
        shared.mark_renderer_cold();
        shared.publish_frame0(job.generation, false);
        return;
    }
    shared.publish_frame0(job.generation, true);

    if initial_progress >= 1.0 {
        post_worker_done(job.notify_hwnd, job.generation);
        return;
    }

    let started = Instant::now();
    let duration = job.duration_seconds.max(0.016);
    loop {
        if shared.is_superseded(job.generation) {
            return;
        }
        let _ = active.wait_for_next_frame(FRAME_WAIT_TIMEOUT_MS);
        if shared.is_superseded(job.generation) {
            return;
        }
        let elapsed = started.elapsed().as_secs_f32();
        let progress = (elapsed / duration).clamp(0.0, 1.0);
        let geometry = compute_genie_geometry(job.window, job.dock, progress, job.direction);
        if active
            .present_frame(&geometry, job.window, job.corner_radius_px, true, false)
            .is_err()
        {
            *renderer = None;
            *bound_hwnd = 0;
            shared.mark_renderer_cold();
            post_worker_done(job.notify_hwnd, job.generation);
            return;
        }
        if progress >= 1.0 {
            post_worker_done(job.notify_hwnd, job.generation);
            return;
        }
    }
}

fn worker_main(shared: &WorkerShared) {
    let _com = ComGuard::multithreaded();
    let mut renderer: Option<GenieRenderer> = None;
    let mut bound_hwnd: isize = 0;

    while let Some(command) = shared.next_command() {
        match command {
            RenderCommand::Begin(job) => {
                run_job(shared, &mut renderer, &mut bound_hwnd, &job);
            }
            RenderCommand::Clear(generation) => {
                if !shared.is_superseded(generation)
                    && let Some(active) = renderer.as_mut()
                {
                    active.clear_snapshot();
                    let _ = active.clear_and_present();
                }
            }
            RenderCommand::Shutdown => break,
        }
    }
}

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
    *runtime.genie.borrow_mut() = Some(genie);
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
mod tests {
    use super::Genie;
    use otto_core::{Rect, WindowIdentity};

    fn missing_window() -> WindowIdentity {
        WindowIdentity {
            hwnd: 0x7FFF_FFF0,
            process_id: 0xFFFF_FFF0,
            thread_id: 0xFFFF_FFF1,
            generation: 1,
        }
    }

    #[test]
    fn restore_without_cached_snapshot_or_bounds_falls_back_to_native() {
        let mut genie = Genie::new(0);
        let dock = Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 48,
        };
        assert!(!genie.begin_restore(missing_window(), dock, None, 8.0, None, true, false, &[],));
        assert!(genie.animation.is_none());
    }

    #[test]
    fn restore_with_fallback_bounds_animates_synthetic_card_without_cache_hit() {
        let mut genie = Genie::new(0);
        let dock = Rect {
            x: 920,
            y: 1020,
            width: 48,
            height: 48,
        };
        let bounds = Rect {
            x: 160,
            y: 120,
            width: 640,
            height: 480,
        };
        let started = genie.begin_restore(
            missing_window(),
            dock,
            Some(bounds),
            8.0,
            None,
            true,
            false,
            &[],
        );
        assert!(started);
        assert!(genie.animation.is_some());
        genie.animation = None;
        genie.shutdown();
    }

    #[test]
    fn stale_worker_completion_does_not_clear_newer_animation() {
        let mut genie = Genie::new(0);
        let dock = Rect {
            x: 920,
            y: 1020,
            width: 48,
            height: 48,
        };
        let bounds = Rect {
            x: 160,
            y: 120,
            width: 640,
            height: 480,
        };
        assert!(genie.begin_restore(
            missing_window(),
            dock,
            Some(bounds),
            8.0,
            None,
            true,
            false,
            &[],
        ));
        let active_gen = genie.animation.expect("active animation").generation;
        genie.finish_generation(active_gen.saturating_sub(1));
        assert!(genie.animation.is_some());
        genie.animation = None;
        genie.shutdown();
    }

    #[test]
    fn shutdown_without_animation_leaves_nothing_running() {
        let mut genie = Genie::new(0);
        genie.shutdown();
        assert!(genie.animation.is_none());
        assert!(genie.worker.is_none());
        assert!(genie.snapshots.is_empty());
    }

    #[test]
    fn fallback_target_stays_valid_when_dock_is_absent() {
        let runtime = crate::host::RuntimeContext::new(false);
        let target = crate::dock::fallback_target_for_window(&runtime, 0);
        assert!(target.width >= 24);
        assert!(target.height >= 24);
        assert!(target.y > 0);
    }

    #[test]
    fn wm_close_is_absorbed_without_destroying_surface() {
        let runtime = crate::host::RuntimeContext::new(false);
        assert_eq!(
            super::handle_message(
                &runtime,
                0,
                super::WM_CLOSE,
                super::WPARAM(0),
                super::LPARAM(0)
            ),
            Some(super::LRESULT(0))
        );
    }

    #[test]
    fn clear_after_timeout_supersedes_in_flight_job_and_stays_valid_for_worker() {
        let shared = super::WorkerShared::new();
        let active_generation = shared.bump_generation();
        assert!(!shared.is_superseded(active_generation));

        shared.send_command(super::RenderCommand::Clear(active_generation));

        assert!(shared.is_superseded(active_generation));
        let Some(super::RenderCommand::Clear(clear_generation)) = shared.next_command() else {
            panic!("expected queued Clear command");
        };
        assert!(clear_generation > active_generation);
        assert!(!shared.is_superseded(clear_generation));

        let next_generation = shared.bump_generation();
        shared.send_command(super::RenderCommand::Clear(active_generation));
        assert!(shared.is_superseded(clear_generation));
        assert!(!shared.is_superseded(next_generation));
        assert!(shared.lock().pending.is_none());
    }
}
