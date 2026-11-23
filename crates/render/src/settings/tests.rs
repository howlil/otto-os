use otto_core::{
    AccentColor, AppearancePreferences, AppearanceStyle, DayKey, FocusAppFilter, FocusCatalogApp,
    FocusPageState, FocusSessionStatus, FocusSettingsTab, FocusTextField, GeneralPageState,
    OttoSettingsSnapshot, PluginSettingsState, ProcessAuditFilter, ProcessThrottleKind,
    RawProcessAuditSample, ReleaseUpdateInfo, ResolvedTheme, Rgba, SettingsHit, SettingsPage,
    ThemeMode, UpdateState, build_content, compute_content_layout, compute_settings_layout,
    inspect_browser_context, resolve_modules, summarize_process_audit,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
};
use windows::core::w;

use super::{SettingsPaintState, SettingsRenderer};
use crate::release_shared_factories;

const TEST_ACCENT: Rgba = Rgba::new(0.0, 0.412, 0.922, 1.0);
const TEST_ON_ACCENT: Rgba = Rgba::new(1.0, 1.0, 1.0, 1.0);

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

fn populated_snapshot() -> OttoSettingsSnapshot {
    let mut snapshot = OttoSettingsSnapshot {
        plugins: PluginSettingsState {
            top_bar: true,
            notch: true,
            dock: true,
            window_controls: true,
            genie: false,
        },
        ..OttoSettingsSnapshot::default()
    };
    let day = DayKey {
        year: 2026,
        month: 11,
        day: 22,
    };
    let browser = inspect_browser_context(
        Some("https://www.google.com/search?q=otto"),
        "otto - Google Search - Chrome",
    );
    snapshot
        .focus_activity
        .observe_foreground(day, "chrome.exe", "Google Chrome", Some(browser));
    snapshot
        .focus_activity
        .record_sample(day, 1_000, 0, true, 5, 25);
    snapshot
        .focus_activity
        .record_sample(day, 15_000, 0, true, 3, 18);
    let _ = snapshot.focus_activity.blocklist.add_app("discord");
    let _ = snapshot.focus_activity.blocklist.add_website("youtube.com");
    snapshot
}

fn sample_update_states() -> [UpdateState; 2] {
    [
        UpdateState::Available(ReleaseUpdateInfo {
            version: String::from("1.2.1"),
            tag: String::from("v1.2.1"),
            installer_name: String::from("otto-os-setup-1.2.1-x64.exe"),
            installer_url: String::from(
                "https://github.com/howlil/otto-os/releases/download/v1.2.1/otto-os-setup-1.2.1-x64.exe",
            ),
            checksums_url: Some(String::from(
                "https://github.com/howlil/otto-os/releases/download/v1.2.1/SHA256SUMS.txt",
            )),
            release_url: String::from("https://github.com/howlil/otto-os/releases/tag/v1.2.1"),
        }),
        UpdateState::Downloading {
            version: String::from("1.2.1"),
            percent: Some(64),
        },
    ]
}

#[test]
fn draws_every_page_in_both_themes_and_survives_resize_and_dpi_change() {
    let hwnd = host_window();
    let snapshot = populated_snapshot();
    let modules = resolve_modules(&snapshot, |_| None);
    let mut renderer = SettingsRenderer::new(hwnd.0 as isize, 900, 600, 96).expect("renderer");
    let layout = compute_settings_layout(900, 600, 96);
    let default_focus = FocusPageState::default();
    for dark in [true, false] {
        for page in SettingsPage::ALL {
            let content = build_content(
                &snapshot,
                page,
                &modules,
                "1.0.0",
                &UpdateState::Idle,
                &default_focus,
            );
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
                    high_contrast: false,
                    accent: TEST_ACCENT,
                    on_accent: TEST_ON_ACCENT,
                })
                .expect("draw");
        }
    }

    for state in sample_update_states() {
        let content = build_content(
            &snapshot,
            SettingsPage::About,
            &modules,
            "1.0.0",
            &state,
            &default_focus,
        );
        let blocks = compute_content_layout(layout.viewport, &content, 96, 0);
        renderer
            .draw(&SettingsPaintState {
                layout: &layout,
                content: &content,
                blocks: &blocks,
                page: SettingsPage::About,
                scroll: 0,
                hovered: None,
                dark: true,
                glass: true,
                high_contrast: false,
                accent: TEST_ACCENT,
                on_accent: TEST_ON_ACCENT,
            })
            .expect("draw update state");
    }
    renderer.resize(1200, 800).expect("resize");
    renderer.set_dpi(144).expect("dpi");
    let layout = compute_settings_layout(1200, 800, 144);
    let content = build_content(
        &snapshot,
        SettingsPage::Modules,
        &modules,
        "1.0.0",
        &UpdateState::Idle,
        &default_focus,
    );
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
            high_contrast: false,
            accent: TEST_ACCENT,
            on_accent: TEST_ON_ACCENT,
        })
        .expect("draw after resize");
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    release_shared_factories();
}

#[test]
fn draws_focus_tabs_chart_and_high_contrast_modes() {
    let hwnd = host_window();
    let snapshot = populated_snapshot();
    let modules = resolve_modules(&snapshot, |_| None);
    let mut renderer = SettingsRenderer::new(hwnd.0 as isize, 900, 600, 96).expect("renderer");
    let layout = compute_settings_layout(900, 600, 96);
    let focus_states = [
        FocusPageState {
            tab: FocusSettingsTab::BlockApps,
            app_filter: FocusAppFilter::AllApps,
            app_search: String::from("ch"),
            website_input: String::from("bad"),
            website_error: Some(String::from("Enter a valid domain")),
            focused_field: FocusTextField::AppSearch,
            session: FocusSessionStatus::RunningCountdown {
                remaining_seconds: 24 * 60,
            },
            catalog: vec![FocusCatalogApp {
                block_key: String::from("chrome"),
                display_name: String::from("Google Chrome"),
                detail: String::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
                icon_source: String::from(r"shell:AppsFolder\Chrome"),
            }],
            ..FocusPageState::default()
        },
        FocusPageState {
            tab: FocusSettingsTab::Activity,
            details_expanded: true,
            selected_bar_day: Some(6),
            ..FocusPageState::default()
        },
    ];
    for (focus_state, high_contrast) in focus_states.iter().zip([false, true]) {
        let content = build_content(
            &snapshot,
            SettingsPage::Focus,
            &modules,
            "1.0.0",
            &UpdateState::Idle,
            focus_state,
        );
        let blocks = compute_content_layout(layout.viewport, &content, 96, 0);
        renderer
            .draw(&SettingsPaintState {
                layout: &layout,
                content: &content,
                blocks: &blocks,
                page: SettingsPage::Focus,
                scroll: 0,
                hovered: Some(SettingsHit::Row { section: 1, row: 2 }),
                dark: true,
                glass: !high_contrast,
                high_contrast,
                accent: TEST_ACCENT,
                on_accent: TEST_ON_ACCENT,
            })
            .expect("draw focus state");
    }
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    release_shared_factories();
}

#[test]
fn draws_appearance_page_across_styles_themes_and_accents() {
    let hwnd = host_window();
    let mut snapshot = populated_snapshot();
    let modules = resolve_modules(&snapshot, |_| None);
    let mut renderer = SettingsRenderer::new(hwnd.0 as isize, 900, 600, 96).expect("renderer");
    let layout = compute_settings_layout(900, 600, 96);
    let default_focus = FocusPageState::default();
    for (style, theme, accent, high_contrast) in [
        (
            AppearanceStyle::LiquidGlass,
            ThemeMode::Dark,
            AccentColor::Blue,
            false,
        ),
        (
            AppearanceStyle::Frosted,
            ThemeMode::Light,
            AccentColor::Emerald,
            false,
        ),
        (
            AppearanceStyle::Tinted,
            ThemeMode::Dark,
            AccentColor::Rose,
            false,
        ),
        (
            AppearanceStyle::Solid,
            ThemeMode::Light,
            AccentColor::System,
            true,
        ),
    ] {
        snapshot.appearance = AppearancePreferences {
            style,
            theme,
            accent,
        };
        let resolved_theme = match theme {
            ThemeMode::Light => ResolvedTheme::Light,
            ThemeMode::Dark | ThemeMode::Auto => ResolvedTheme::Dark,
        };
        let resolved_accent = accent.resolve(resolved_theme, Some(Rgba::new(0.8, 0.3, 0.2, 1.0)));
        let content = build_content(
            &snapshot,
            SettingsPage::Appearance,
            &modules,
            "1.0.0",
            &UpdateState::Idle,
            &default_focus,
        );
        let blocks = compute_content_layout(layout.viewport, &content, 96, 0);
        renderer
            .draw(&SettingsPaintState {
                layout: &layout,
                content: &content,
                blocks: &blocks,
                page: SettingsPage::Appearance,
                scroll: 0,
                hovered: Some(SettingsHit::Part {
                    section: 0,
                    row: 0,
                    part: 2,
                }),
                dark: resolved_theme.is_dark(),
                glass: style.uses_glass() && !high_contrast,
                high_contrast,
                accent: resolved_accent.fill,
                on_accent: resolved_accent.on_accent,
            })
            .expect("draw appearance page");
    }
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    release_shared_factories();
}

fn sample_general_audit_report() -> otto_core::ProcessAuditReport {
    summarize_process_audit(&[
        RawProcessAuditSample {
            pid: 42,
            exe_name: String::from("discord.exe"),
            exe_path: Some(String::from(r"C:\Apps\Discord.exe")),
            memory_bytes: 220 * 1024 * 1024,
            cpu_tenths_percent: 14,
            session_id: 1,
            is_critical: false,
            controllable: true,
            terminatable: true,
            is_foreground: false,
            has_visible_window: false,
            throttle_kind: ProcessThrottleKind::AutoEcoQos,
        },
        RawProcessAuditSample {
            pid: 43,
            exe_name: String::from("slack_updater.exe"),
            exe_path: Some(String::from(r"C:\Apps\SlackUpdater.exe")),
            memory_bytes: 200 * 1024 * 1024,
            cpu_tenths_percent: 2,
            session_id: 1,
            is_critical: false,
            controllable: true,
            terminatable: true,
            is_foreground: false,
            has_visible_window: false,
            throttle_kind: ProcessThrottleKind::None,
        },
        RawProcessAuditSample {
            pid: 44,
            exe_name: String::from("onedrive.exe"),
            exe_path: Some(String::from(r"C:\Apps\OneDrive.exe")),
            memory_bytes: 240 * 1024 * 1024,
            cpu_tenths_percent: 1,
            session_id: 1,
            is_critical: false,
            controllable: true,
            terminatable: true,
            is_foreground: false,
            has_visible_window: false,
            throttle_kind: ProcessThrottleKind::None,
        },
        RawProcessAuditSample {
            pid: 55,
            exe_name: String::from("svchost.exe"),
            exe_path: Some(String::from(r"C:\Windows\System32\svchost.exe")),
            memory_bytes: 190 * 1024 * 1024,
            cpu_tenths_percent: 3,
            session_id: 0,
            is_critical: false,
            controllable: false,
            terminatable: false,
            is_foreground: false,
            has_visible_window: false,
            throttle_kind: ProcessThrottleKind::None,
        },
    ])
}

#[test]
fn draws_general_process_audit_across_themes_and_high_contrast() {
    let hwnd = host_window();
    let mut snapshot = populated_snapshot();
    snapshot.system.background_cleanup = true;
    snapshot.process_audit = GeneralPageState {
        filter: ProcessAuditFilter::All,
        audit: sample_general_audit_report(),
        ..GeneralPageState::default()
    };
    let modules = resolve_modules(&snapshot, |_| None);
    let mut renderer = SettingsRenderer::new(hwnd.0 as isize, 900, 600, 96).expect("renderer");
    let layout = compute_settings_layout(900, 600, 96);
    let default_focus = FocusPageState::default();
    for (dark, high_contrast) in [(true, false), (false, false), (true, true)] {
        let content = build_content(
            &snapshot,
            SettingsPage::General,
            &modules,
            "1.0.0",
            &UpdateState::Idle,
            &default_focus,
        );
        let blocks = compute_content_layout(layout.viewport, &content, 96, 0);
        renderer
            .draw(&SettingsPaintState {
                layout: &layout,
                content: &content,
                blocks: &blocks,
                page: SettingsPage::General,
                scroll: 0,
                hovered: Some(SettingsHit::Part {
                    section: 2,
                    row: 2,
                    part: 1,
                }),
                dark,
                glass: !high_contrast,
                high_contrast,
                accent: TEST_ACCENT,
                on_accent: TEST_ON_ACCENT,
            })
            .expect("draw general audit page");
    }
    drop(renderer);
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    release_shared_factories();
}
