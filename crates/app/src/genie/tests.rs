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
