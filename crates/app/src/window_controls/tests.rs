use super::{
    Overlay, PendingCaptionDrag, Placement, PressState, plan_overlay_slots,
    should_start_caption_drag, step_press_move, step_press_release,
};
use otto_core::{
    Metrics, NativeCaption, NativeCaptionStyle, WindowControlAction, WindowControlsMetrics,
    compute_cover_layout,
};
use otto_platform::ActiveWindowTarget;

#[test]
fn a_window_keeps_the_overlay_that_already_covers_it() {
    let slots = plan_overlay_slots(&[Some(7), Some(3)], &[3, 7]);
    assert_eq!(slots, vec![Some(7), Some(3)]);
}

#[test]
fn a_new_window_takes_a_free_overlay_before_a_new_one_is_added() {
    let slots = plan_overlay_slots(&[Some(7), None], &[7, 9]);
    assert_eq!(slots, vec![Some(7), Some(9)]);
    let grown = plan_overlay_slots(&[Some(7)], &[7, 9]);
    assert_eq!(grown, vec![Some(7), Some(9)]);
}

#[test]
fn an_overlay_whose_window_is_gone_is_released_and_reused() {
    let slots = plan_overlay_slots(&[Some(7), Some(3)], &[3, 9]);
    assert_eq!(slots, vec![Some(9), Some(3)]);
    let emptied = plan_overlay_slots(&[Some(7), Some(3)], &[]);
    assert_eq!(emptied, vec![None, None]);
}

fn sample_caption() -> NativeCaption {
    NativeCaption::new(
        NativeCaptionStyle {
            has_caption: true,
            has_system_menu: true,
            can_minimize: true,
            can_maximize: true,
        },
        500,
        10,
        30,
        true,
        96,
    )
    .unwrap()
}

#[test]
fn press_and_release_cancels_when_pointer_leaves_button_and_executes_when_returned() {
    let layout = compute_cover_layout(sample_caption(), &WindowControlsMetrics::default()).unwrap();
    let close_x = layout.centers_x[2].round() as i32;
    let cy = layout.center_y.round() as i32;
    let mut press = Some(PressState {
        action: WindowControlAction::Close,
        inside: true,
    });
    let (hovered_out, changed_out) = step_press_move(press.as_mut().unwrap(), &layout, 10, cy);
    assert!(changed_out);
    assert!(!hovered_out);
    assert!(!press.unwrap().inside);

    let (exec_cancel, _) = step_press_release(&mut press, &layout, 10, cy);
    assert_eq!(exec_cancel, None);
    assert!(press.is_none());

    let mut press_back = Some(PressState {
        action: WindowControlAction::Close,
        inside: false,
    });
    let (hovered_in, changed_in) =
        step_press_move(press_back.as_mut().unwrap(), &layout, close_x, cy);
    assert!(changed_in);
    assert!(hovered_in);
    let (exec_ok, hovered_after) = step_press_release(&mut press_back, &layout, close_x, cy);
    assert_eq!(exec_ok, Some(WindowControlAction::Close));
    assert!(hovered_after);
}

#[test]
fn pending_caption_drag_waits_for_threshold_before_forwarding() {
    let mut pending = Some(PendingCaptionDrag {
        target_hwnd: 42,
        start_client: (20, 15),
        start_screen: (400, 100),
    });
    assert_eq!(
        should_start_caption_drag(&mut pending, 23, 17, (4, 4)),
        None
    );
    assert!(pending.is_some());
    let drag = should_start_caption_drag(&mut pending, 24, 15, (4, 4)).unwrap();
    assert_eq!(drag.target_hwnd, 42);
    assert!(pending.is_none());
}

#[test]
fn overlay_remains_active_under_partial_top_bar_overlap_and_hides_when_fully_covered() {
    let partial_caption = sample_caption();
    let target = ActiveWindowTarget {
        hwnd: 99,
        left: 100,
        top: 10,
        right: 500,
        bottom: 400,
        maximized: false,
        dpi: 96,
        on_primary: true,
        active: true,
        sample_unobstructed: true,
        caption: Some(partial_caption),
    };
    assert_eq!(
        Overlay::placement(target, Metrics::MACBOOK_PRO_14.bar_height),
        Placement::Cover(partial_caption)
    );
    assert_eq!(
        Overlay::placement(target, Metrics::DYNAMIC_ISLAND.bar_height),
        Placement::Hidden
    );

    let covered_caption = NativeCaption::new(
        NativeCaptionStyle {
            has_caption: true,
            has_system_menu: true,
            can_minimize: true,
            can_maximize: true,
        },
        500,
        0,
        30,
        true,
        96,
    )
    .unwrap();
    let covered_target = ActiveWindowTarget {
        caption: Some(covered_caption),
        ..target
    };
    assert_eq!(
        Overlay::placement(covered_target, Metrics::MACBOOK_PRO_14.bar_height),
        Placement::Hidden
    );
}

#[test]
fn wm_close_is_absorbed_without_destroying_surface() {
    let runtime = crate::host::RuntimeContext::new(false);
    assert_eq!(
        super::handle_message(
            &runtime,
            0,
            windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        ),
        Some(windows::Win32::Foundation::LRESULT(0))
    );
}

#[test]
fn overlay_starts_without_renderer_and_updates_layout_lazily() {
    let mut overlay = Overlay::new(
        0,
        1,
        otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
        32,
    );
    assert!(overlay.renderer.is_none());
    let mut next_layout = overlay.layout;
    next_layout.width += 10;
    overlay.use_layout(next_layout);
    assert_eq!(overlay.layout, next_layout);
    assert!(overlay.renderer.is_none());
    overlay.hide();
    assert!(overlay.renderer.is_none());
}
