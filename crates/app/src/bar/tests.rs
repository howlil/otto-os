use super::{
    DeferredModalAction, HitTarget, LPARAM, LRESULT, MAINTAIN_EVERY_TICKS, MEDIA_EVERY_TICKS,
    NotchExpandedTab, RuntimeContext, WM_CLOSE, WPARAM, handle_message, is_primary_bar_window,
    is_upward_touch_swipe, periodic_tick_due, queue_modal_action, should_refresh_media,
    should_refresh_system_status, should_wheel_collapse_notch,
};

#[test]
fn periodic_maintenance_keeps_strided_intervals_while_idle() {
    assert!(periodic_tick_due(0, MAINTAIN_EVERY_TICKS));
    assert!(!periodic_tick_due(1, MAINTAIN_EVERY_TICKS));
    assert!(!periodic_tick_due(7, MAINTAIN_EVERY_TICKS));
    assert!(periodic_tick_due(8, MAINTAIN_EVERY_TICKS));
}

#[test]
fn wake_events_bypass_media_and_status_stride_gates() {
    assert!(!should_refresh_media(false, 1));
    assert!(should_refresh_media(true, 1));
    assert!(should_refresh_media(false, MEDIA_EVERY_TICKS));
    assert!(!should_refresh_system_status(false, true, 1));
    assert!(should_refresh_system_status(true, true, 1));
    assert!(should_refresh_system_status(false, false, 1));
    assert!(should_refresh_system_status(false, true, MEDIA_EVERY_TICKS));
}

#[test]
fn primary_bar_window_check_distinguishes_bar_and_panel_hwnds() {
    let runtime = RuntimeContext::new(false);
    assert!(!is_primary_bar_window(&runtime, 0));
    runtime.bar_hwnd.set(101);
    assert!(is_primary_bar_window(&runtime, 101));
    assert!(!is_primary_bar_window(&runtime, 202));
    assert_eq!(
        handle_message(&runtime, 202, WM_CLOSE, WPARAM(0), LPARAM(0)),
        Some(LRESULT(0))
    );
}

#[test]
fn queue_modal_action_keeps_first_pending_modal_until_drained() {
    let runtime = RuntimeContext::new(false);
    queue_modal_action(&runtime, DeferredModalAction::PickShelfFiles);
    queue_modal_action(
        &runtime,
        DeferredModalAction::StartShelfDrag {
            capture_hwnd: 55,
            payload: otto_core::ShelfPayload::Text(String::from("ignored")),
        },
    );
    let first = runtime.pending_modal_action.borrow_mut().take();
    assert!(matches!(first, Some(DeferredModalAction::PickShelfFiles)));
}

#[test]
fn wheel_collapse_preserves_pomodoro_and_shelf_body_scrolling() {
    assert!(should_wheel_collapse_notch(
        true,
        Some(HitTarget::Notch),
        NotchExpandedTab::Pomodoro,
        120,
    ));
    assert!(!should_wheel_collapse_notch(
        true,
        Some(HitTarget::Notch),
        NotchExpandedTab::Pomodoro,
        -120,
    ));
    assert!(should_wheel_collapse_notch(
        false,
        Some(HitTarget::NotchTabPomodoro),
        NotchExpandedTab::Pomodoro,
        120,
    ));
    assert!(!should_wheel_collapse_notch(
        false,
        Some(HitTarget::PomodoroPreset(0)),
        NotchExpandedTab::Pomodoro,
        120,
    ));
    assert!(!should_wheel_collapse_notch(
        false,
        Some(HitTarget::ShelfItem(0)),
        NotchExpandedTab::Shelf,
        120,
    ));
    assert!(should_wheel_collapse_notch(
        false,
        Some(HitTarget::Notch),
        NotchExpandedTab::Media,
        120,
    ));
}

#[test]
fn upward_touch_swipe_requires_vertical_displacement_over_threshold() {
    assert!(is_upward_touch_swipe((500, 100), (505, 70), 24));
    assert!(!is_upward_touch_swipe((500, 100), (500, 80), 24));
    assert!(!is_upward_touch_swipe((500, 100), (500, 130), 24));
    assert!(!is_upward_touch_swipe((500, 100), (550, 70), 24));
}

#[test]
fn end_session_message_queues_pending_end_session_when_wparam_nonzero() {
    let runtime = RuntimeContext::new(false);
    assert_eq!(
        handle_message(&runtime, 0, super::WM_ENDSESSION, WPARAM(0), LPARAM(0)),
        Some(LRESULT(0))
    );
    assert!(!runtime.pending_end_session.get());
    assert_eq!(
        handle_message(&runtime, 0, super::WM_ENDSESSION, WPARAM(1), LPARAM(0)),
        Some(LRESULT(0))
    );
    assert!(runtime.pending_end_session.get());
}

#[test]
fn prepare_modal_action_returns_none_without_active_bar() {
    let runtime = RuntimeContext::new(false);
    assert!(super::prepare_modal_action(&runtime, DeferredModalAction::PickShelfFiles).is_none());
}

#[test]
fn mouse_move_uses_non_blocking_wake_while_clicks_and_hotkeys_wake_synchronously() {
    assert!(!super::wakes_status_tick(super::WM_MOUSEMOVE));
    assert!(super::wakes_status_tick(super::WM_LBUTTONDOWN));
    assert!(super::wakes_status_tick(super::WM_HOTKEY));
    assert!(super::wakes_status_tick(super::WM_POWERBROADCAST));
}
