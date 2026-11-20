use super::layout::*;
use super::mode::*;
use super::ranking::*;

#[test]
fn search_ranks_prefixes_before_word_and_substring_matches() {
    let names = [
        "Windows Calculator",
        "Calculator",
        "Character Map",
        "Notepad",
    ];
    assert_eq!(rank_spotlight_results(names, "calc"), vec![1, 0]);
    assert_eq!(rank_spotlight_results(names, "char"), vec![2]);
    assert!(rank_spotlight_results(names, "missing").is_empty());
}

#[test]
fn grid_layout_bounds_visible_tiles_and_hit_targets() {
    let layout = compute_spotlight_layout((100, 40), 780, 600, 96, SpotlightView::Grid, 30, 0);
    assert!(layout.results.len() <= 30);
    assert!(layout.results.iter().all(|result| {
        result.tile.x >= 0
            && result.tile.y >= 0
            && result.tile.right() <= 780
            && result.tile.bottom() <= 600
    }));
    let first = layout.results.first().expect("visible result");
    assert_eq!(
        layout.hit_test(first.tile.x + 2, first.tile.y + 2),
        Some(SpotlightHit::Result(first.index))
    );
    assert_eq!(layout.window.x, 100);
}

#[test]
fn list_layout_and_selection_moves_use_single_row_steps() {
    let layout = compute_spotlight_layout((0, 0), 780, 600, 96, SpotlightView::List, 100, 0);
    assert_eq!(layout.columns, 1);
    assert!(layout.results.len() < 100);
    assert_eq!(
        move_spotlight_selection(Some(1), 8, SpotlightView::List, 1, SpotlightMove::Down),
        Some(2)
    );
    assert_eq!(
        move_spotlight_selection(Some(4), 8, SpotlightView::Grid, 4, SpotlightMove::Down),
        Some(7)
    );
    assert_eq!(
        move_spotlight_selection(Some(0), 8, SpotlightView::Grid, 4, SpotlightMove::Up),
        Some(0)
    );
    assert_eq!(
        move_spotlight_selection(None, 0, SpotlightView::Grid, 6, SpotlightMove::Down),
        None
    );
}

#[test]
fn navigation_without_a_selection_starts_at_the_first_result() {
    assert_eq!(
        move_spotlight_selection(None, 8, SpotlightView::Grid, 4, SpotlightMove::Down),
        Some(0)
    );
    assert_eq!(
        move_spotlight_selection(None, 8, SpotlightView::List, 1, SpotlightMove::End),
        Some(7)
    );
}

#[test]
fn spotlight_modes_have_explicit_search_and_apps_states() {
    assert_eq!(SpotlightMode::default(), SpotlightMode::Search);
    assert_ne!(SpotlightMode::Apps, SpotlightMode::Search);
}

#[test]
fn evaluate_math_handles_precedence_parens_power_and_formatting() {
    assert_eq!(
        SpotlightMode::evaluate_math("2 + 3 * 4").as_deref(),
        Some("14")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("(2 + 3) * 4").as_deref(),
        Some("20")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("2500 + 2500").as_deref(),
        Some("5,000")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("1,000 + 2_000").as_deref(),
        Some("3,000")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("5 / 2").as_deref(),
        Some("2.5")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("10 ÷ 4").as_deref(),
        Some("2.5")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("3 × 1000").as_deref(),
        Some("3,000")
    );
    assert_eq!(SpotlightMode::evaluate_math("10 % 3").as_deref(), Some("1"));
    assert_eq!(
        SpotlightMode::evaluate_math("2 ^ 3 ^ 2").as_deref(),
        Some("512")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("-5 + 12").as_deref(),
        Some("7")
    );
    assert_eq!(
        SpotlightMode::evaluate_math("0.1 + 0.2").as_deref(),
        Some("0.3")
    );
    assert_eq!(SpotlightMode::evaluate_math("1 - 1").as_deref(), Some("0"));
    assert_eq!(
        SpotlightMode::evaluate_math("1 - 1001").as_deref(),
        Some("-1,000")
    );
}

#[test]
fn evaluate_math_rejects_plain_numbers_apps_and_invalid_expressions() {
    assert_eq!(SpotlightMode::evaluate_math(""), None);
    assert_eq!(SpotlightMode::evaluate_math("   "), None);
    assert_eq!(SpotlightMode::evaluate_math("42"), None);
    assert_eq!(SpotlightMode::evaluate_math("-42"), None);
    assert_eq!(SpotlightMode::evaluate_math("(42)"), None);
    assert_eq!(SpotlightMode::evaluate_math("7zip"), None);
    assert_eq!(SpotlightMode::evaluate_math("1password"), None);
    assert_eq!(SpotlightMode::evaluate_math("1 / 0"), None);
    assert_eq!(SpotlightMode::evaluate_math("1 % 0"), None);
    assert_eq!(SpotlightMode::evaluate_math("(-1) ^ 0.5"), None);
    assert_eq!(SpotlightMode::evaluate_math("2 + "), None);
    assert_eq!(SpotlightMode::evaluate_math("(2 + 3"), None);
    assert_eq!(SpotlightMode::evaluate_math("2 + 3)"), None);
}

#[test]
fn caret_editing_inserts_deletes_and_navigates_utf8_safely() {
    let mut query = String::new();
    let mut cursor = 0;
    let mut selected_all = false;

    assert!(!SpotlightMode::insert_char(
        &mut query,
        &mut cursor,
        &mut selected_all,
        '\x01',
        10
    ));
    assert!(SpotlightMode::insert_char(
        &mut query,
        &mut cursor,
        &mut selected_all,
        'a',
        10
    ));
    assert!(SpotlightMode::insert_char(
        &mut query,
        &mut cursor,
        &mut selected_all,
        'é',
        10
    ));
    assert!(SpotlightMode::insert_char(
        &mut query,
        &mut cursor,
        &mut selected_all,
        'c',
        10
    ));
    assert_eq!(query, "aéc");
    assert_eq!(cursor, query.len());

    assert!(SpotlightMode::move_cursor_left(
        &query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(cursor, "aé".len());
    assert!(SpotlightMode::insert_char(
        &mut query,
        &mut cursor,
        &mut selected_all,
        'b',
        10
    ));
    assert_eq!(query, "aébc");
    assert_eq!(cursor, "aéb".len());

    assert!(SpotlightMode::backspace(
        &mut query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(query, "aéc");
    assert!(SpotlightMode::delete_forward(
        &mut query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(query, "aé");
    assert!(!SpotlightMode::delete_forward(
        &mut query,
        &mut cursor,
        &mut selected_all
    ));

    assert!(SpotlightMode::move_cursor_home(
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(cursor, 0);
    assert!(!SpotlightMode::move_cursor_home(
        &mut cursor,
        &mut selected_all
    ));
    assert!(SpotlightMode::move_cursor_right(
        &query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(cursor, 1);
    assert!(SpotlightMode::move_cursor_end(
        &query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(cursor, query.len());
}

#[test]
fn caret_editing_handles_select_all_word_delete_and_paste_sanitization() {
    let mut query = String::from("hello world");
    let mut cursor = query.len();
    let mut selected_all = false;

    assert!(SpotlightMode::delete_word_backward(
        &mut query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(query, "hello ");
    assert_eq!(cursor, 6);

    assert!(SpotlightMode::delete_word_backward(
        &mut query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(query, "");
    assert_eq!(cursor, 0);

    assert!(SpotlightMode::insert_text(
        &mut query,
        &mut cursor,
        &mut selected_all,
        "12\r\n+\t\x0734",
        20
    ));
    assert_eq!(query, "12 + 34");
    assert_eq!(cursor, 7);

    selected_all = true;
    assert!(SpotlightMode::move_cursor_left(
        &query,
        &mut cursor,
        &mut selected_all
    ));
    assert!(!selected_all);
    assert_eq!(cursor, 0);

    selected_all = true;
    assert!(SpotlightMode::insert_text(
        &mut query,
        &mut cursor,
        &mut selected_all,
        "abcdef",
        4
    ));
    assert_eq!(query, "abcd");
    assert_eq!(cursor, 4);
    assert!(!selected_all);

    selected_all = true;
    assert!(SpotlightMode::backspace(
        &mut query,
        &mut cursor,
        &mut selected_all
    ));
    assert_eq!(query, "");
    assert_eq!(cursor, 0);
    assert!(!selected_all);
}

#[test]
fn spotlight_outer_shell_and_search_inset_follow_concentric_grid() {
    let layout = compute_spotlight_layout((0, 0), 780, 600, 96, SpotlightView::Grid, 0, 0);
    assert!((layout.corner_radius - 28.0).abs() < f32::EPSILON);
    assert_eq!(layout.search_field.x, 16);
    assert_eq!(layout.window.width - layout.search_field.right(), 16);
    assert_eq!(layout.corner_radius as i32 - layout.search_field.x, 12);

    let hidpi = compute_spotlight_layout((0, 0), 1170, 900, 144, SpotlightView::Grid, 0, 0);
    assert!((hidpi.corner_radius - 42.0).abs() < f32::EPSILON);
    assert_eq!(hidpi.search_field.x, 24);
    assert_eq!(hidpi.corner_radius as i32 - hidpi.search_field.x, 18);
}
