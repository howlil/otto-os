use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::win_msg::WM_APP_GENIE_DONE;
use otto_core::{GenieDirection, MotionPreference, Rect, WindowIdentity, compute_genie_geometry};
use otto_platform::{ComGuard, join_within, post_window_message};
use otto_render::GenieRenderer;
use windows::core::Result;

use super::FRAME_WAIT_TIMEOUT_MS;
use super::WORKER_SHUTDOWN_TIMEOUT;

#[derive(Clone)]
pub(super) struct CachedSnapshot {
    pub(super) rect: Rect,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: Arc<Vec<u8>>,
}

pub(super) struct CapturedFrame {
    pub(super) rect: Rect,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixels: Arc<Vec<u8>>,
    pub(super) cache_eligible: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GenieAnimation {
    pub(super) generation: u64,
    pub(super) window_id: WindowIdentity,
    pub(super) direction: GenieDirection,
}

pub(super) struct RenderJob {
    pub(super) generation: u64,
    pub(super) notify_hwnd: isize,
    pub(super) canvas: Rect,
    pub(super) window: Rect,
    pub(super) dock: Rect,
    pub(super) corner_radius_px: f32,
    pub(super) duration_seconds: f32,
    pub(super) direction: GenieDirection,
    pub(super) motion: MotionPreference,
    pub(super) snapshot_width: u32,
    pub(super) snapshot_height: u32,
    pub(super) snapshot_pixels: Arc<Vec<u8>>,
}

pub(super) enum RenderCommand {
    Begin(RenderJob),
    Clear(u64),
    Shutdown,
}

pub(super) struct WorkerState {
    pub(super) generation: u64,
    pub(super) pending: Option<RenderCommand>,
    pub(super) frame0_generation: u64,
    pub(super) frame0_ok: bool,
    pub(super) renderer_warm: bool,
    pub(super) stopping: bool,
}

pub(super) struct WorkerShared {
    pub(super) state: Mutex<WorkerState>,
    pub(super) wake_worker: Condvar,
    pub(super) wake_ui: Condvar,
}

impl WorkerShared {
    pub(super) fn new() -> Self {
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

    pub(super) fn lock(&self) -> MutexGuard<'_, WorkerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn bump_generation(&self) -> u64 {
        let mut guard = self.lock();
        guard.generation = guard.generation.saturating_add(1);
        guard.generation
    }

    pub(super) fn send_command(&self, mut command: RenderCommand) {
        {
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
        }
        self.wake_worker.notify_all();
    }

    pub(super) fn wait_frame0(&self, generation: u64, timeout: Duration) -> bool {
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

    pub(super) fn publish_frame0(&self, generation: u64, ok: bool) {
        let notify = {
            let mut guard = self.lock();
            if ok {
                guard.renderer_warm = true;
            }
            if generation >= guard.frame0_generation {
                guard.frame0_generation = generation;
                guard.frame0_ok = ok;
                true
            } else {
                false
            }
        };
        if notify {
            self.wake_ui.notify_all();
        }
    }

    pub(super) fn mark_renderer_cold(&self) {
        self.lock().renderer_warm = false;
    }

    pub(super) fn is_renderer_warm(&self) -> bool {
        self.lock().renderer_warm
    }

    pub(super) fn is_superseded(&self, generation: u64) -> bool {
        let guard = self.lock();
        guard.stopping || guard.generation != generation
    }

    pub(super) fn next_command(&self) -> Option<RenderCommand> {
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

pub(super) struct RenderWorker {
    pub(super) shared: Arc<WorkerShared>,
    pub(super) handle: Option<JoinHandle<()>>,
}

impl RenderWorker {
    pub(super) fn spawn() -> Option<Self> {
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

    pub(super) fn stop(&mut self) {
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

pub(super) fn prepare_renderer(
    renderer: &mut Option<GenieRenderer>,
    bound_hwnd: &mut isize,
    job: &RenderJob,
) -> Result<()> {
    let active = if let Some(active) = renderer.as_mut() {
        active
    } else {
        let mut created = GenieRenderer::new(job.canvas)?;
        if job.notify_hwnd != 0 {
            created.bind_window(job.notify_hwnd)?;
            *bound_hwnd = job.notify_hwnd;
        }
        renderer.insert(created)
    };
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

pub(super) fn post_worker_done(notify_hwnd: isize, generation: u64) {
    if notify_hwnd == 0 {
        return;
    }
    if let Ok(wparam) = usize::try_from(generation) {
        let _ = post_window_message(notify_hwnd, WM_APP_GENIE_DONE, wparam, 0);
    }
}

pub(super) fn run_job(
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

pub(super) fn worker_main(shared: &WorkerShared) {
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
