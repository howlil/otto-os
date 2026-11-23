use super::{
    SpotlightSurface, SpotlightToggleDecision, SpotlightToggleSource, SpotlightVisibility,
    decide_spotlight_toggle,
};
use otto_core::SpotlightMode;

#[test]
fn toggle_decision_closes_matching_mode_switches_other_mode_and_absorbs_blur_race() {
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Closed,
            SpotlightMode::Search,
            SpotlightToggleSource::Pointer,
            false,
        ),
        SpotlightToggleDecision::Open
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Open(SpotlightMode::Search),
            SpotlightMode::Search,
            SpotlightToggleSource::Pointer,
            false,
        ),
        SpotlightToggleDecision::Close
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Open(SpotlightMode::Apps),
            SpotlightMode::Search,
            SpotlightToggleSource::Pointer,
            false,
        ),
        SpotlightToggleDecision::Open
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Open(SpotlightMode::Apps),
            SpotlightMode::Search,
            SpotlightToggleSource::Hotkey,
            false,
        ),
        SpotlightToggleDecision::Close
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Closing,
            SpotlightMode::Search,
            SpotlightToggleSource::Pointer,
            true,
        ),
        SpotlightToggleDecision::IgnoreRecentDismiss
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Closed,
            SpotlightMode::Search,
            SpotlightToggleSource::Pointer,
            true,
        ),
        SpotlightToggleDecision::IgnoreRecentDismiss
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Closing,
            SpotlightMode::Search,
            SpotlightToggleSource::Pointer,
            false,
        ),
        SpotlightToggleDecision::Open
    );
    assert_eq!(
        decide_spotlight_toggle(
            SpotlightVisibility::Closed,
            SpotlightMode::Search,
            SpotlightToggleSource::Hotkey,
            true,
        ),
        SpotlightToggleDecision::Open
    );
}

#[test]
fn surface_query_edits_track_cursor_selection_and_math_result() {
    let mut surface = SpotlightSurface::new(0);
    for ch in "12+34".chars() {
        surface.set_query(ch);
    }
    assert_eq!(surface.query, "12+34");
    assert_eq!(surface.query_state.cursor, 5);
    assert_eq!(surface.math_result.as_deref(), Some("46"));

    assert!(SpotlightMode::move_cursor_left(
        &surface.query,
        &mut surface.query_state.cursor,
        &mut surface.query_state.selected_all
    ));
    assert!(SpotlightMode::move_cursor_left(
        &surface.query,
        &mut surface.query_state.cursor,
        &mut surface.query_state.selected_all
    ));
    surface.set_query('1');
    assert_eq!(surface.query, "12+134");
    assert_eq!(surface.math_result.as_deref(), Some("146"));

    surface.query_state.selected_all = true;
    surface.backspace();
    assert_eq!(surface.query, "");
    assert_eq!(surface.query_state.cursor, 0);
    assert!(!surface.query_state.selected_all);
    assert_eq!(surface.math_result, None);
}

#[test]
fn surface_ignores_control_chars_in_set_query() {
    let mut surface = SpotlightSurface::new(0);
    surface.set_query('2');
    surface.set_query('\x01');
    surface.set_query('+');
    surface.set_query('\x03');
    surface.set_query('2');
    assert_eq!(surface.query, "2+2");
    assert_eq!(surface.math_result.as_deref(), Some("4"));
}
