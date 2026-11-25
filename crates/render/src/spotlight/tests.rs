use std::sync::PoisonError;

use super::*;
use crate::release_shared_factories;
use crate::resource_tests::SERIAL;
use otto_core::{ContrastColors, ResolvedTheme, Rgba};

#[test]
fn spotlight_concentric_search_radius_and_squircle_shell_render_cleanly() {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let layout =
        otto_core::compute_spotlight_layout((100, 100), 640, 440, 96, SpotlightView::Grid, 2, 0);
    let expected_search_radius =
        (layout.corner_radius - layout.search_field.x as f32).max(scale(10, 96) as f32);
    assert!((expected_search_radius - 12.0).abs() < f32::EPSILON);

    let renderer = SpotlightRenderer::new(96).expect("spotlight renderer");
    let entries = [
        SpotlightPaintEntry {
            key: "finder",
            title: "Finder",
            apple_icon: Some(AppleIconKind::Finder),
        },
        SpotlightPaintEntry {
            key: "calc",
            title: "Calculator",
            apple_icon: None,
        },
    ];
    for opacity in [0.25, 1.0] {
        let state = SpotlightPaintState {
            layout: &layout,
            entries: &entries,
            mode: SpotlightMode::Search,
            view: SpotlightView::Grid,
            query: "2 + 2",
            cursor_byte: 5,
            selected_all: false,
            math_result: Some("4"),
            selected: Some(0),
            hovered: Some(1),
            content_opacity: opacity,
            appearance: Appearance::glass(ResolvedTheme::Dark),
        };
        renderer
            .draw(layout.window.width, layout.window.height, &state)
            .expect("spotlight draw");
    }
    assert!(renderer.surface.read_pixels().iter().any(|&byte| byte != 0));
    drop(renderer);
    release_shared_factories();
}

#[test]
fn spotlight_renders_light_mode_selected_all_and_high_contrast_cleanly() {
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let renderer = SpotlightRenderer::new(96).expect("spotlight renderer");
    let entries = [
        SpotlightPaintEntry {
            key: "finder",
            title: "Finder",
            apple_icon: Some(AppleIconKind::Finder),
        },
        SpotlightPaintEntry {
            key: "calc",
            title: "Calculator",
            apple_icon: None,
        },
    ];
    let hc_white = ContrastColors {
        window: Rgba::new(1.0, 1.0, 1.0, 1.0),
        window_text: Rgba::new(0.0, 0.0, 0.0, 1.0),
        highlight: Rgba::new(0.0, 0.0, 0.5, 1.0),
        highlight_text: Rgba::new(1.0, 1.0, 1.0, 1.0),
        button_face: Rgba::new(0.95, 0.95, 0.95, 1.0),
        gray_text: Rgba::new(0.35, 0.35, 0.35, 1.0),
    };
    let scenarios = [
        (
            SpotlightMode::Search,
            SpotlightView::Grid,
            Appearance::glass(ResolvedTheme::Light),
        ),
        (
            SpotlightMode::Apps,
            SpotlightView::List,
            Appearance {
                transparency: false,
                ..Appearance::glass(ResolvedTheme::Light)
            },
        ),
        (
            SpotlightMode::Search,
            SpotlightView::Grid,
            Appearance {
                contrast: Some(ContrastColors::BLACK_THEME),
                ..Appearance::glass(ResolvedTheme::Dark)
            },
        ),
        (
            SpotlightMode::Apps,
            SpotlightView::List,
            Appearance {
                contrast: Some(hc_white),
                ..Appearance::glass(ResolvedTheme::Light)
            },
        ),
    ];
    for (mode, view, appearance) in scenarios {
        let layout = otto_core::compute_spotlight_layout((100, 100), 640, 440, 96, view, 2, 0);
        let state = SpotlightPaintState {
            layout: &layout,
            entries: &entries,
            mode,
            view,
            query: "Notes",
            cursor_byte: 5,
            selected_all: true,
            math_result: None,
            selected: Some(0),
            hovered: Some(1),
            content_opacity: 1.0,
            appearance,
        };
        renderer
            .draw(layout.window.width, layout.window.height, &state)
            .expect("spotlight draw");
        assert!(renderer.surface.read_pixels().iter().any(|&byte| byte != 0));
    }
    drop(renderer);
    release_shared_factories();
}
