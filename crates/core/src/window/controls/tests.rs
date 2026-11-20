use super::caption::*;
use super::cover::*;
use super::layout::*;
use super::state::*;
use crate::app_model::AppInfo;
use crate::app_model::WindowCommand;
use crate::app_model::WindowInfo;
use crate::app_model::WindowPresence;
use crate::app_model::{AppIdentity, WindowIdentity, WindowRole, WindowTraits};
use crate::layout::scale;

fn window(traits: WindowTraits) -> WindowInfo {
    WindowInfo {
        identity: WindowIdentity {
            hwnd: 1,
            process_id: 2,
            thread_id: 3,
            generation: 4,
        },
        app: AppIdentity::executable("app.exe"),
        app_name: String::from("App"),
        executable: String::from("app.exe"),
        title: String::from("App"),
        role: WindowRole::Primary,
        presence: WindowPresence::Visible,
        zoomed: false,
        full_screen: false,
        active: true,
        traits,
    }
}

fn sibling(hwnd: isize) -> WindowInfo {
    let mut sibling = window(WindowTraits::STANDARD);
    sibling.identity.hwnd = hwnd;
    sibling
}

fn application(windows: Vec<WindowInfo>) -> AppInfo {
    crate::app_model::group_windows(windows).remove(0)
}

#[test]
fn alt_widens_only_close_and_minimize_to_the_whole_application() {
    use WindowControlAction::{Close, Minimize, ToggleMaximize};
    assert_eq!(window_control_scope(Close, true), CommandScope::Application);
    assert_eq!(
        window_control_scope(Minimize, true),
        CommandScope::Application
    );
    assert_eq!(window_control_scope(Close, false), CommandScope::Window);
    assert_eq!(window_control_scope(Minimize, false), CommandScope::Window);
    assert_eq!(
        window_control_scope(ToggleMaximize, true),
        CommandScope::Window
    );
}

#[test]
fn application_targets_start_with_the_clicked_window_and_skip_unavailable_ones() {
    let clicked = sibling(1);
    let mut minimized = sibling(2);
    minimized.presence = WindowPresence::Minimized;
    let mut hidden = sibling(3);
    hidden.presence = WindowPresence::Hidden;
    let mut dialog = sibling(4);
    dialog.role = WindowRole::OwnedDialog;
    let mut fixed = sibling(5);
    fixed.traits.closable = false;
    let plain = sibling(6);
    let app = application(vec![
        plain.clone(),
        minimized.clone(),
        hidden,
        dialog,
        fixed.clone(),
        clicked.clone(),
    ]);

    let minimize = application_command_targets(&app, clicked.identity, WindowCommand::Minimize);
    assert_eq!(
        minimize,
        vec![clicked.identity, plain.identity, fixed.identity]
    );

    let close = application_command_targets(&app, clicked.identity, WindowCommand::Close);
    assert_eq!(
        close,
        vec![clicked.identity, plain.identity, minimized.identity]
    );
}

#[test]
fn application_targets_keep_a_clicked_dialog_but_not_other_dialogs() {
    let mut clicked = sibling(1);
    clicked.role = WindowRole::OwnedDialog;
    let mut other_dialog = sibling(2);
    other_dialog.role = WindowRole::OwnedDialog;
    let main = sibling(3);
    let app = application(vec![other_dialog, main.clone(), clicked.clone()]);
    let targets = application_command_targets(&app, clicked.identity, WindowCommand::Close);
    assert_eq!(targets, vec![clicked.identity, main.identity]);
}

#[test]
fn controls_cover_every_visible_captioned_window_even_when_its_actions_are_unavailable() {
    assert!(accepts_window_controls(&window(WindowTraits::STANDARD)));
    let mut borderless = WindowTraits::STANDARD;
    borderless.title_bar = false;
    assert!(!accepts_window_controls(&window(borderless)));
    let mut full = window(WindowTraits::STANDARD);
    full.full_screen = true;
    assert!(!accepts_window_controls(&full));
    let mut inert = WindowTraits::STANDARD;
    inert.closable = false;
    inert.minimizable = false;
    inert.maximizable = false;
    assert!(accepts_window_controls(&window(inert)));
    let mut blocked = WindowTraits::STANDARD;
    blocked.blocked_by_modal = true;
    assert!(accepts_window_controls(&window(blocked)));
    let mut hung = WindowTraits::STANDARD;
    hung.responding = false;
    assert!(accepts_window_controls(&window(hung)));
    let mut elevated = WindowTraits::STANDARD;
    elevated.elevated = true;
    assert!(!accepts_window_controls(&window(elevated)));
}

#[test]
fn disabled_caption_buttons_disable_the_matching_control() {
    let mut dialog = WindowTraits::STANDARD;
    dialog.minimizable = false;
    dialog.maximizable = false;
    let state = WindowControlsState::of(&window(dialog));
    assert!(state.allows(WindowControlAction::Close));
    assert!(!state.allows(WindowControlAction::Minimize));
    assert!(!state.allows(WindowControlAction::ToggleMaximize));
    let mut blocked = WindowTraits::STANDARD;
    blocked.blocked_by_modal = true;
    assert!(!WindowControlsState::of(&window(blocked)).any());
}

#[test]
fn full_screen_capable_window_keeps_the_green_control_without_zoom() {
    let mut browser = WindowTraits::STANDARD;
    browser.maximizable = false;
    browser.full_screen_shortcut = true;
    assert!(WindowControlsState::of(&window(browser)).zoom);
}

#[test]
fn top_is_clamped_below_the_bar_only_on_the_bar_monitor() {
    assert_eq!(window_controls_top(0, 9, Some(48), false), 48);
    assert_eq!(window_controls_top(48, 9, Some(48), true), 57);
    assert_eq!(window_controls_top(300, 9, Some(48), false), 309);
    assert_eq!(window_controls_top(-400, 9, None, false), -391);
    assert_eq!(window_controls_top(0, 9, None, true), 9);
}

#[test]
fn window_controls_hit_test_identifies_red_yellow_green_on_left() {
    let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 96);
    let cy = layout.center_y as i32;
    assert_eq!(
        layout.hit_test(layout.centers_x[0] as i32, cy),
        Some(WindowControlAction::Close)
    );
    assert_eq!(
        layout.hit_test(layout.centers_x[1] as i32, cy),
        Some(WindowControlAction::Minimize)
    );
    assert_eq!(
        layout.hit_test(layout.centers_x[2] as i32, cy),
        Some(WindowControlAction::ToggleMaximize)
    );
    assert!(layout.centers_x[0] < layout.centers_x[1]);
    assert!(layout.centers_x[1] < layout.centers_x[2]);
    assert_eq!(layout.hit_test(2, 2), None);
}

#[test]
fn hit_slop_scales_with_the_monitor_dpi() {
    let layout = compute_window_controls_layout(&WindowControlsMetrics::MACOS_SONOMA, 192);
    let cy = layout.center_y as i32;
    let just_inside_close = layout.centers_x[0] as i32 + 18;
    assert_eq!(
        layout.hit_test(just_inside_close, cy),
        Some(WindowControlAction::Close)
    );
}

#[test]
fn layout_follows_each_monitor_dpi_so_mixed_dpi_controls_stay_aligned() {
    let metrics = WindowControlsMetrics::MACOS_SONOMA;
    for dpi in [96, 120, 144, 192] {
        let layout = compute_window_controls_layout(&metrics, dpi);
        assert_eq!(layout.width, scale(metrics.width, dpi));
        assert_eq!(layout.height, scale(metrics.height, dpi));
        assert_eq!(layout.offset_y, scale(metrics.offset_y, dpi));
        let last_edge = layout.centers_x[2] + layout.radius;
        assert!(last_edge <= layout.width as f32, "dpi {dpi}");
        assert!(layout.center_y + layout.radius <= layout.height as f32);
        let cy = layout.center_y as i32;
        assert_eq!(
            layout.hit_test(layout.centers_x[2] as i32, cy),
            Some(WindowControlAction::ToggleMaximize),
            "dpi {dpi}"
        );
    }
    let standard = compute_window_controls_layout(&metrics, 96);
    let high = compute_window_controls_layout(&metrics, 192);
    assert_eq!(high.width, standard.width * 2);
    assert!((high.radius - standard.radius * 2.0).abs() <= 1.0);
}

const STANDARD_STYLE: NativeCaptionStyle = NativeCaptionStyle {
    has_caption: true,
    has_system_menu: true,
    can_minimize: true,
    can_maximize: true,
};

fn caption(style: NativeCaptionStyle) -> NativeCaption {
    NativeCaption::new(style, 1191, 300, 44, true, 144).unwrap()
}

#[test]
fn three_native_buttons_are_right_aligned_to_the_visible_frame() {
    let caption = caption(STANDARD_STYLE);
    let cover = caption.cover_rect();
    assert_eq!(cover.width, 69 * 3);
    assert_eq!(cover.right(), 1191);
    assert_eq!(cover.y, 300);
    assert_eq!(cover.height, 44);
}

#[test]
fn a_close_only_window_covers_a_single_button() {
    let style = NativeCaptionStyle {
        can_minimize: false,
        can_maximize: false,
        ..STANDARD_STYLE
    };
    let caption = caption(style);
    assert_eq!(caption.slots, 1);
    assert_eq!(caption.cover_rect().width, 69);
    assert_eq!(caption.minimize_button(), None);
}

#[test]
fn a_window_with_only_a_maximize_box_still_shows_three_slots_but_no_minimize() {
    let style = NativeCaptionStyle {
        can_minimize: false,
        ..STANDARD_STYLE
    };
    let caption = caption(style);
    assert_eq!(caption.slots, 3);
    assert_eq!(caption.minimize_button(), None);
}

#[test]
fn the_native_minimize_button_is_the_leftmost_native_slot() {
    let caption = caption(STANDARD_STYLE);
    let button = caption.minimize_button().unwrap();
    assert_eq!(button.x, 1191 - 69 * 3);
    assert_eq!(button.width, 69);
    assert!(button.contains(button.x, 300));
    assert!(!button.contains(button.right(), 300));
}

#[test]
fn windows_without_a_caption_or_system_menu_have_no_native_caption() {
    for style in [
        NativeCaptionStyle {
            has_caption: false,
            ..STANDARD_STYLE
        },
        NativeCaptionStyle {
            has_system_menu: false,
            ..STANDARD_STYLE
        },
    ] {
        assert_eq!(NativeCaption::new(style, 1191, 300, 44, true, 144), None);
    }
    assert_eq!(
        NativeCaption::new(STANDARD_STYLE, 1191, 300, 0, true, 144),
        None
    );
}

#[test]
fn a_plausible_measured_width_replaces_the_standard_cover_width() {
    let measured = caption(STANDARD_STYLE).measured(220);
    let cover = measured.cover_rect();
    assert_eq!(cover.width, 220);
    assert_eq!(cover.right(), 1191);
    assert_eq!(measured.minimize_button().unwrap().width, 73);
}

#[test]
fn an_implausible_measured_width_keeps_the_standard_cover_width() {
    for width in [0, 40, 400] {
        let measured = caption(STANDARD_STYLE).measured(width);
        assert_eq!(measured.cover_rect().width, 69 * 3, "width {width}");
    }
}

#[test]
fn a_standard_caption_derives_its_height_from_the_dpi() {
    let caption = NativeCaption::standard(STANDARD_STYLE, 1191, 300, true, 144).unwrap();
    assert_eq!(caption.height, 45);
    assert_eq!(caption.cover_rect().width, 69 * 3);
    assert_eq!(caption.cover_rect().right(), 1191);
    let none = NativeCaptionStyle {
        has_system_menu: false,
        ..STANDARD_STYLE
    };
    assert_eq!(NativeCaption::standard(none, 1191, 300, true, 144), None);
}

#[test]
fn the_corner_radius_follows_rounding_and_dpi() {
    let rounded = caption(STANDARD_STYLE);
    assert!((rounded.corner_radius() - 12.0).abs() < 0.001);
    let square = NativeCaption::new(STANDARD_STYLE, 1191, 300, 44, false, 144).unwrap();
    assert!(square.corner_radius().abs() < 0.001);
}

#[test]
fn the_cover_layout_orders_buttons_like_windows_with_mac_spacing_on_right() {
    let layout =
        compute_cover_layout(caption(STANDARD_STYLE), &WindowControlsMetrics::default()).unwrap();
    assert_eq!(layout.actions[0], WindowControlAction::Minimize);
    assert_eq!(layout.actions[1], WindowControlAction::ToggleMaximize);
    assert_eq!(layout.actions[2], WindowControlAction::Close);
    assert_eq!(layout.width, 207);
    assert!((layout.centers_x[0] - 126.0).abs() < 0.01);
    assert!((layout.centers_x[1] - 156.0).abs() < 0.01);
    assert!((layout.centers_x[2] - 186.0).abs() < 0.01);
}

#[test]
fn the_cover_hit_test_matches_the_right_aligned_circle_group_and_leaves_left_cover_empty() {
    let layout =
        compute_cover_layout(caption(STANDARD_STYLE), &WindowControlsMetrics::default()).unwrap();
    let cy = layout.center_y as i32;
    assert_eq!(layout.hit_test(10, cy), None);
    assert_eq!(layout.hit_test(90, cy), None);
    assert!(!layout.hits_group(90, cy));
    assert!(layout.hits_group(126, cy));
    assert_eq!(
        layout.hit_test(126, cy),
        Some(WindowControlAction::Minimize)
    );
    assert_eq!(
        layout.hit_test(156, cy),
        Some(WindowControlAction::ToggleMaximize)
    );
    assert_eq!(layout.hit_test(186, cy), Some(WindowControlAction::Close));
    assert_eq!(layout.hit_test(207, cy), None);
    assert_eq!(layout.hit_test(156, 44), None);
    assert_eq!(layout.hit_test(-1, cy), None);
}

#[test]
fn a_single_slot_layout_only_answers_for_close() {
    let style = NativeCaptionStyle {
        can_minimize: false,
        can_maximize: false,
        ..STANDARD_STYLE
    };
    let layout = compute_cover_layout(caption(style), &WindowControlsMetrics::default()).unwrap();
    let cy = layout.center_y as i32;
    assert_eq!(layout.slots, 1);
    assert_eq!(layout.hit_test(48, cy), Some(WindowControlAction::Close));
    assert_eq!(layout.hit_test(10, cy), None);
    assert_eq!(layout.hit_test(70, cy), None);
    assert_eq!(layout.center_of(WindowControlAction::Close), Some(48.0));
    assert_eq!(layout.center_of(WindowControlAction::Minimize), None);
}

#[test]
fn inactive_windows_keep_buttons_uncolored_until_hovered() {
    let mut bg = window(WindowTraits::STANDARD);
    bg.active = false;
    let state = WindowControlsState::of(&bg);
    assert!(!state.shows_color(WindowControlAction::Close, false));
    assert!(!state.shows_color(WindowControlAction::Minimize, false));
    assert!(state.shows_color(WindowControlAction::Close, true));
    assert!(state.shows_color(WindowControlAction::Minimize, true));
}

#[test]
fn cover_row_extraction_filters_stray_columns_and_coalesces_bands_with_border_inset() {
    let bgra = [
        10, 20, 30, 255, 10, 20, 30, 255, 250, 250, 250, 255, 40, 50, 60, 255, 40, 50, 60, 255, 40,
        50, 60, 255, 40, 50, 60, 255, 40, 50, 60, 255, 40, 50, 60, 255,
    ];
    let rows = extract_cover_rows(&bgra, 3, 3).unwrap();
    assert_eq!(rows, vec![[30, 20, 10], [60, 50, 40], [60, 50, 40]]);
    let inset_bands = cover_row_bands(&rows, 3, true);
    assert_eq!(
        inset_bands,
        vec![CoverBand {
            top: 1,
            bottom: 3,
            rgb: [60, 50, 40],
        }]
    );
    let full_bands = cover_row_bands(&rows, 3, false);
    assert_eq!(full_bands.len(), 2);
    assert_eq!(full_bands[0].top, 0);
    assert_eq!(full_bands[0].bottom, 1);
    assert_eq!(full_bands[1].top, 1);
    assert_eq!(full_bands[1].bottom, 3);
}

#[test]
fn the_left_layout_keeps_its_circle_hit_test_and_order() {
    let layout = compute_window_controls_layout(&WindowControlsMetrics::default(), 96);
    assert_eq!(layout.actions[0], WindowControlAction::Close);
    assert_eq!(
        layout.center_of(WindowControlAction::ToggleMaximize),
        Some(layout.centers_x[2])
    );
    assert_eq!(
        layout.hit_test(layout.centers_x[1] as i32, layout.center_y as i32),
        Some(WindowControlAction::Minimize)
    );
}

#[test]
fn extract_cover_rows_ignores_embedded_icon_spikes_and_preserves_gradients() {
    let width = 12;
    let height = 5;
    let mut bgra = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let base = (220 - y * 5) as u8;
        for x in 0..width {
            let (r, g, b) = if (1..=3).contains(&y) && (2..=5).contains(&x) {
                (220, 40, 40)
            } else {
                (base, base + 5, base + 10)
            };
            bgra.extend_from_slice(&[b, g, r, 255]);
        }
    }
    let rows = extract_cover_rows(&bgra, width, height).unwrap();
    assert_eq!(rows.len(), height);
    for (y, row) in rows.iter().enumerate() {
        let base = (220 - y * 5) as u8;
        assert_eq!(*row, [base, base + 5, base + 10], "row {y}");
    }
}

#[test]
fn cover_is_dark_distinguishes_dark_and_light_titlebars() {
    assert!(cover_is_dark(&[[30, 30, 34], [32, 32, 36], [34, 34, 38]]));
    assert!(!cover_is_dark(&[
        [240, 240, 244],
        [242, 242, 246],
        [244, 244, 248]
    ]));
    assert!(cover_is_dark(&[]));
}

#[test]
fn clipped_sample_rect_clamps_to_frame_left_and_bar_bottom() {
    let wide = NativeCaption::new(STANDARD_STYLE, 500, 10, 30, true, 96).unwrap();
    let clipped = wide.clipped_sample_rect(0, 36);
    assert_eq!(clipped.x, wide.cover_rect().x - 2 - 24);
    assert_eq!(clipped.width, 24);
    assert_eq!(clipped.y, 36);
    assert_eq!(clipped.height, 4);

    let narrow = NativeCaption::new(STANDARD_STYLE, 160, 10, 30, true, 96).unwrap();
    let clipped_left = narrow.clipped_sample_rect(5, 0);
    assert_eq!(clipped_left.x, 7);
    assert_eq!(clipped_left.right(), narrow.cover_rect().x - 2);

    let hidden = wide.clipped_sample_rect(0, 45);
    assert!(hidden.is_empty());
}

#[test]
fn resize_edge_matches_outer_four_dip_rim_when_unmaximized_and_resizable() {
    let caption = NativeCaption::new(STANDARD_STYLE, 500, 100, 30, true, 96).unwrap();
    let layout = compute_cover_layout(caption, &WindowControlsMetrics::default()).unwrap();
    assert_eq!(
        layout.resize_edge(10, 0, false, true, 96),
        Some(ResizeEdge::Top)
    );
    assert_eq!(
        layout.resize_edge(layout.width - 1, 0, false, true, 96),
        Some(ResizeEdge::TopRight)
    );
    assert_eq!(
        layout.resize_edge(layout.width - 1, 15, false, true, 96),
        Some(ResizeEdge::Right)
    );
    assert_eq!(layout.resize_edge(10, 15, false, true, 96), None);
    assert_eq!(
        layout.resize_edge(layout.centers_x[2] as i32, 0, false, true, 96),
        Some(ResizeEdge::Top)
    );
    assert_eq!(
        layout.resize_edge(
            layout.centers_x[2] as i32,
            layout.center_y as i32,
            false,
            true,
            96
        ),
        None
    );
    assert_eq!(layout.resize_edge(10, 0, true, true, 96), None);
    assert_eq!(layout.resize_edge(10, 0, false, false, 96), None);
}

#[test]
fn window_controls_remain_visible_under_bar_until_bottom_edge_is_covered() {
    assert!(window_controls_visible_under_bar(10, 30, Some(38)));
    assert!(!window_controls_visible_under_bar(8, 30, Some(38)));
    assert!(!window_controls_visible_under_bar(0, 30, Some(38)));
    assert!(window_controls_visible_under_bar(0, 30, None));
}

#[test]
fn alt_zoom_glyph_defaults_to_false_and_toggles_via_builder() {
    let state = WindowControlsState::of(&window(WindowTraits::STANDARD));
    assert!(!state.alt_zoom_glyph);
    assert!(state.with_alt_zoom_glyph(true).alt_zoom_glyph);
    assert!(
        !state
            .with_alt_zoom_glyph(true)
            .with_alt_zoom_glyph(false)
            .alt_zoom_glyph
    );
}
