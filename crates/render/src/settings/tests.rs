use otto_core::{
    OttoSettingsSnapshot, PluginSettingsState, SettingsPage, build_content, compute_content_layout,
    compute_settings_layout, resolve_modules,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
};
use windows::core::w;

use super::{SettingsPaintState, SettingsRenderer};
use crate::release_shared_factories;

fn host_window() -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("settings"),
            WS_POPUP,
            0,
            0,
            900,
            600,
            None,
            None,
            None,
            None,
        )
        .expect("host window")
    }
}

#[test]
fn draws_every_page_in_both_themes_and_survives_resize_and_dpi_change() {
    let hwnd = host_window();
    let snapshot = OttoSettingsSnapshot {
        plugins: PluginSettingsState {
            top_bar: true,
            notch: true,
            dock: true,
            window_controls: true,
            genie: false,
        },
        ..OttoSettingsSnapshot::default()
    };
    let modules = resolve_modules(&snapshot, |_| None);
    let mut renderer = SettingsRenderer::new(hwnd.0 as isize, 900, 600, 96).expect("renderer");
    let layout = compute_settings_layout(900, 600, 96);
    for dark in [true, false] {
        for page in SettingsPage::ALL {
            let content = build_content(&snapshot, page, &modules, "1.0.0", None);
            let blocks = compute_content_layout(layout.viewport, &content, 96, 0);
            renderer
                .draw(&SettingsPaintState {
                    layout: &layout,
                    content: &content,
                    blocks: &blocks,
                    page,
                    scroll: 0,
                    hovered: None,
                    dark,
                    glass: true,
                })
                .expect("draw");
        }
    }
    renderer.resize(1200, 800).expect("resize");
    renderer.set_dpi(144).expect("dpi");
    let layout = compute_settings_layout(1200, 800, 144);
    let content = build_content(&snapshot, SettingsPage::Modules, &modules, "1.0.0", None);
    let probe = compute_content_layout(layout.viewport, &content, 144, 0);
    let scroll = layout.max_scroll(&probe);
    let blocks = compute_content_layout(layout.viewport, &content, 144, scroll);
    renderer
        .draw(&SettingsPaintState {
            layout: &layout,
            content: &content,
            blocks: &blocks,
            page: SettingsPage::Modules,
            scroll,
            hovered: None,
            dark: true,
            glass: false,
        })
        .expect("draw after resize");
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    release_shared_factories();
}
