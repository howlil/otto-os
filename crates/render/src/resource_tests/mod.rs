use std::sync::{Mutex, MutexGuard, PoisonError};

use super::{
    ControlCenterRenderer, DockRenderItem, DockRenderer, DownloadsStackRenderer, GenieRenderer,
    PopupMenuRenderer, Renderer, release_shared_factories, shared_d2d_factory,
};
use crate::appearance::surface::{
    DibSurface, DrawSession, create_dc_render_target, draw_on, upload_bgra_bitmap,
};
use otto_core::{PixelLimits, Rect};
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct2D::ID2D1DCRenderTarget;
use windows::Win32::System::Threading::{
    GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetGuiResources,
};
use windows::core::{IUnknown, Interface};

const ROUNDS: usize = 3;
const IMPOSSIBLE: i32 = 1 << 20;
const SURFACE_SIZE: (i32, i32) = (64, 48);
const BITMAP_SIDE: u32 = 16;
const SHORT_RUN_CYCLES: usize = 100;
const LONG_RUN_CYCLES: usize = 10_000;

pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

struct SerialResourceTest {
    _lock: MutexGuard<'static, ()>,
}

impl SerialResourceTest {
    fn begin() -> Self {
        Self {
            _lock: SERIAL.lock().unwrap_or_else(PoisonError::into_inner),
        }
    }
}

impl Drop for SerialResourceTest {
    fn drop(&mut self) {
        release_shared_factories();
    }
}

#[derive(Clone, Copy)]
struct GuiCounts {
    gdi: u32,
    user: u32,
}

impl GuiCounts {
    fn now() -> Self {
        let process = unsafe { GetCurrentProcess() };
        unsafe {
            Self {
                gdi: GetGuiResources(process, GR_GDIOBJECTS),
                user: GetGuiResources(process, GR_USEROBJECTS),
            }
        }
    }

    fn assert_no_growth(self, label: &str) {
        let after = Self::now();
        assert!(
            after.gdi <= self.gdi,
            "{label}: GDI objects grew from {} to {}",
            self.gdi,
            after.gdi
        );
        assert!(
            after.user <= self.user,
            "{label}: USER objects grew from {} to {}",
            self.user,
            after.user
        );
    }
}

fn observed_refs<T: Interface>(object: &T) -> u32 {
    let unknown: IUnknown = object.cast().expect("IUnknown view");
    let vtable = unknown.vtable();
    unsafe {
        let count = (vtable.AddRef)(unknown.as_raw());
        (vtable.Release)(unknown.as_raw());
        count
    }
}

fn dc_target() -> ID2D1DCRenderTarget {
    let factory = shared_d2d_factory().expect("shared factory");
    create_dc_render_target(&factory).expect("DC render target")
}

fn exercise_successful_constructions() {
    for _ in 0..ROUNDS {
        construct_each_renderer_once();
    }
}

const RENDERER_KINDS: usize = 6;

fn construct_renderer(kind: usize) {
    match kind % RENDERER_KINDS {
        0 => drop(DibSurface::new(320, 200).expect("surface")),
        1 => drop(ControlCenterRenderer::new(96).expect("control center")),
        2 => drop(DockRenderer::new(800, 160, 96).expect("dock")),
        3 => drop(PopupMenuRenderer::new(144).expect("popup menu")),
        4 => drop(
            GenieRenderer::new(Rect {
                x: -200,
                y: 0,
                width: 640,
                height: 480,
            })
            .expect("genie"),
        ),
        _ => drop(Renderer::new(1920, 48, 600, 432, 144, 0.0).expect("bar")),
    }
}

fn construct_each_renderer_once() {
    (0..RENDERER_KINDS).for_each(construct_renderer);
}

fn exercise_failed_constructions() {
    for _ in 0..ROUNDS {
        assert!(DibSurface::new(IMPOSSIBLE, IMPOSSIBLE).is_err());
        assert!(DockRenderer::new(IMPOSSIBLE, IMPOSSIBLE, 96).is_err());
        assert!(
            GenieRenderer::new(Rect {
                x: 0,
                y: 0,
                width: IMPOSSIBLE,
                height: IMPOSSIBLE,
            })
            .is_err()
        );
        assert!(Renderer::new(1920, 48, IMPOSSIBLE, IMPOSSIBLE, 144, 0.0).is_err());
    }
}

fn upload_one_bitmap(target: &ID2D1DCRenderTarget, pixels: &[u8]) {
    drop(
        upload_bgra_bitmap(target, BITMAP_SIDE, BITMAP_SIDE, pixels, PixelLimits::ICON)
            .expect("bitmap upload"),
    );
}

fn bitmap_pixels() -> Vec<u8> {
    vec![0xFFu8; (BITMAP_SIDE * BITMAP_SIDE * 4) as usize]
}

fn surface_draw_and_upload_cycle(target: &ID2D1DCRenderTarget, pixels: &[u8]) {
    let surface = DibSurface::new(SURFACE_SIZE.0, SURFACE_SIZE.1).expect("surface");
    draw_on(target, &surface, SURFACE_SIZE, || Ok(())).expect("draw");
    upload_one_bitmap(target, pixels);
}

fn steady_across_cycles(
    label: &str,
    target: &ID2D1DCRenderTarget,
    cycles: usize,
    cycle: impl Fn(&ID2D1DCRenderTarget),
) {
    cycle(target);
    let baseline = GuiCounts::now();
    let refs = observed_refs(target);
    for _ in 0..cycles {
        cycle(target);
    }
    baseline.assert_no_growth(label);
    assert_eq!(
        observed_refs(target),
        refs,
        "{label} changed the target reference count"
    );
}

#[test]
fn constructing_and_dropping_renderers_releases_every_gdi_object() {
    let _serial = SerialResourceTest::begin();
    exercise_successful_constructions();
    exercise_failed_constructions();
    let baseline = GuiCounts::now();
    exercise_successful_constructions();
    baseline.assert_no_growth("successful constructions");
    exercise_failed_constructions();
    baseline.assert_no_growth("failed constructions");
    release_shared_factories();
    baseline.assert_no_growth("factory release");
}

#[test]
fn abandoned_draw_session_ends_the_draw_and_leaves_the_target_usable() {
    let _serial = SerialResourceTest::begin();
    let target = dc_target();
    let surface = DibSurface::new(SURFACE_SIZE.0, SURFACE_SIZE.1).expect("surface");
    drop(DrawSession::begin(&target, &surface, SURFACE_SIZE).expect("warm-up session"));
    draw_on(&target, &surface, SURFACE_SIZE, || Ok(())).expect("warm-up draw");
    let baseline = GuiCounts::now();
    let refs = observed_refs(&target);
    for _ in 0..ROUNDS {
        drop(DrawSession::begin(&target, &surface, SURFACE_SIZE).expect("session"));
    }
    draw_on(&target, &surface, SURFACE_SIZE, || Ok(()))
        .expect("target draws after abandoned sessions");
    baseline.assert_no_growth("abandoned draw sessions");
    assert_eq!(
        observed_refs(&target),
        refs,
        "abandoned draw sessions changed the target reference count"
    );
}

#[test]
fn rejected_bitmap_uploads_fail_before_any_resource_is_created() {
    let _serial = SerialResourceTest::begin();
    let target = dc_target();
    let limits = PixelLimits {
        max_dimension: 16,
        max_bytes: 64,
    };
    let pixels = vec![0u8; 256];
    let rejected = [(0, 4, 64), (4, 0, 64), (17, 1, 68), (4, 4, 63), (8, 4, 128)];
    let baseline = GuiCounts::now();
    let refs = observed_refs(&target);
    for _ in 0..ROUNDS {
        for (width, height, length) in rejected {
            let error = upload_bgra_bitmap(&target, width, height, &pixels[..length], limits)
                .expect_err("oversized or undersized upload must be rejected");
            assert_eq!(
                error.code(),
                E_INVALIDARG,
                "{width}x{height} with {length} bytes"
            );
        }
    }
    baseline.assert_no_growth("rejected bitmap uploads");
    assert_eq!(
        observed_refs(&target),
        refs,
        "rejected bitmap uploads changed the target reference count"
    );
}

#[test]
fn shelf_render_and_thumbnail_churn_releases_bitmaps_and_gdi_handles() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let make_layout = |shelf_input: otto_core::ShelfLayoutInput| {
        otto_core::compute_layout_with_notch(
            &metrics,
            1920,
            144,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Shelf,
                previous_tab: otto_core::NotchExpandedTab::Shelf,
                tab_blend: 1.0,
                shelf: shelf_input,
                ..otto_core::NotchLayoutInput::default()
            },
        )
    };
    let empty_layout = make_layout(otto_core::ShelfLayoutInput::default());
    let renderer = Renderer::new(
        1920,
        48,
        empty_layout.panel_window.width,
        empty_layout.panel_window.height,
        144,
        0.0,
    )
    .expect("renderer");
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    let lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    let mut shelf = otto_core::ShelfState::default();
    let make_thumb = |seed: u8| otto_core::ShelfThumbnail {
        width: BITMAP_SIDE,
        height: BITMAP_SIDE,
        bgra: vec![seed; (BITMAP_SIDE * BITMAP_SIDE * 4) as usize],
    };

    let clipboard = otto_core::ClipboardState::default();

    let draw_once = |layout: &otto_core::Layout, state: &otto_core::ShelfState| {
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout,
                hovered: Some(otto_core::HitTarget::ShelfItem(0)),
                focused: None,
                pomodoro: &pomodoro,
                media: None,
                anim_phase: 0.0,
                lyrics,
                shelf_state: state,
                clipboard_state: &clipboard,
                tasks_state: &otto_core::TasksState::default(),
                privacy: &privacy,
                notification: None,
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("shelf draw");
    };

    for i in 0..otto_core::MAX_SHELF_ITEMS {
        shelf.push_payload(
            otto_core::ShelfPayload::File(format!(r"C:\Temp\img_{i}.png")),
            false,
            Some(make_thumb((i as u8).saturating_add(1))),
        );
    }
    let populated_layout = make_layout(otto_core::ShelfLayoutInput {
        visible_count: u8::try_from(shelf.visible_items().len()).unwrap_or(0),
        has_pager: shelf.page_count() > 1,
        has_items: true,
    });
    draw_once(&populated_layout, &shelf);
    let baseline = GuiCounts::now();

    for cycle in 0..30 {
        let next_page = usize::from(cycle % 2 == 1);
        shelf.page = next_page;
        draw_once(&populated_layout, &shelf);
        shelf.push_payload(
            otto_core::ShelfPayload::File(format!(r"C:\Temp\extra_{cycle}.png")),
            false,
            Some(make_thumb(((cycle % 200) + 1) as u8)),
        );
        draw_once(&populated_layout, &shelf);
        assert!(renderer.cached_shelf_bitmaps.borrow().len() <= otto_core::MAX_SHELF_ITEMS);
    }

    shelf.clear();
    draw_once(&empty_layout, &shelf);
    assert!(renderer.cached_shelf_bitmaps.borrow().is_empty());
    baseline.assert_no_growth("shelf thumbnail churn");
}

fn sample_batch_shelf(drag_hover: bool, count: usize) -> otto_core::ShelfState {
    let mut shelf = otto_core::ShelfState {
        drag_hover,
        ..otto_core::ShelfState::default()
    };
    for i in 0..count {
        let payload = match i % 4 {
            0 => otto_core::ShelfPayload::File(format!(r"C:\Temp\clip_{i}.png")),
            1 => otto_core::ShelfPayload::File(format!(r"C:\Projects\repo_{i}")),
            2 => otto_core::ShelfPayload::Url(format!("https://example.com/{i}")),
            _ => otto_core::ShelfPayload::Text(format!("Snippet line {i}")),
        };
        let is_dir = i % 4 == 1;
        let thumb = (!is_dir && i % 4 == 0).then(|| otto_core::ShelfThumbnail {
            width: BITMAP_SIDE,
            height: BITMAP_SIDE,
            bgra: vec![
                u8::try_from((i % 200) + 1).unwrap_or(1);
                (BITMAP_SIDE * BITMAP_SIDE * 4) as usize
            ],
        });
        shelf.push_payload(payload, is_dir, thumb);
    }
    shelf
}

#[expect(clippy::too_many_lines)]
fn exercise_batch_1_2_bars_for_dpi(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    media: &otto_core::MediaScene,
    privacy: &otto_core::PrivacyState,
) {
    let status = otto_core::SystemStatus {
        caps_lock: true,
        num_lock: true,
        power_mode: otto_core::PowerMode::BatterySaver,
        input_lang: otto_core::InputLangCode::from_tag("US"),
        ..otto_core::SystemStatus::default()
    };
    let privacy_layout = otto_core::PrivacyLayoutState {
        compact_dots: true,
        panel_mic_control: true,
    };
    let compact_layout = otto_core::compute_layout_with_notch(
        metrics,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            compact_expansion: 1.0,
            media_present: true,
            privacy: privacy_layout,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let huds = [
        None,
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::NumLock { enabled: true },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::Welcome,
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::KeyboardLayout {
                code: otto_core::InputLangCode::from_tag("EN"),
            },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::BatterySaver { enabled: true },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::Dnd { enabled: true },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::BluetoothConnected {
                name: otto_core::HudDeviceLabel::from_name("AirPods Pro"),
                kind: otto_core::BluetoothDeviceKind::Headphones,
                battery_percent: Some(85),
            },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
        Some(otto_core::NotchHudState {
            kind: otto_core::NotchHudKind::BluetoothConnected {
                name: otto_core::HudDeviceLabel::from_name("MX Master 3S"),
                kind: otto_core::BluetoothDeviceKind::Mouse,
                battery_percent: None,
            },
            expansion: 1.0,
            content_alpha: 1.0,
            level: 1.0,
        }),
    ];
    for theme in [
        otto_core::ResolvedTheme::Dark,
        otto_core::ResolvedTheme::Light,
    ] {
        let appearance = otto_core::Appearance {
            transparency: false,
            ..otto_core::Appearance::glass(theme)
        };
        for hud in huds {
            let mut input = otto_core::NotchLayoutInput {
                hud_expansion: if hud.is_some() { 1.0 } else { 0.0 },
                compact_expansion: if hud.is_none() { 1.0 } else { 0.0 },
                media_present: true,
                privacy: privacy_layout,
                ..otto_core::NotchLayoutInput::default()
            };
            let layout = if hud.is_some() {
                input.hud_expansion = 1.0;
                otto_core::compute_layout_with_notch(metrics, 1920, dpi, "Finder", input)
            } else {
                compact_layout.clone()
            };
            renderer
                .draw_bar(&crate::BarPaintState {
                    layout: &layout,
                    status,
                    app_name: "Finder",
                    hovered: Some(otto_core::HitTarget::PrivacyIndicator),
                    focused: Some(otto_core::HitTarget::Notch),
                    hud,
                    pomodoro,
                    media: Some(media),
                    anim_phase: 0.45,
                    appearance,
                    privacy,
                    call: None,
                    notification: None,
                })
                .expect("batch 1-2 bar draw");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn exercise_batch_1_focus_and_alert(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    media: &otto_core::MediaScene,
    privacy: &otto_core::PrivacyState,
) {
    let privacy_layout = otto_core::PrivacyLayoutState {
        compact_dots: true,
        panel_mic_control: true,
    };
    let empty_shelf = otto_core::ShelfState::default();
    let empty_clipboard = otto_core::ClipboardState::default();
    let idle_lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    let mut running_pomo = pomodoro.clone();
    running_pomo.state.status = otto_core::PomodoroStatus::Running;
    let layout = otto_core::compute_layout_with_notch(
        metrics,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            pomodoro_surface: running_pomo.view().surface,
            media_present: true,
            privacy: privacy_layout,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    renderer
        .draw_panel(&crate::NotchPanelPaintState {
            layout: &layout,
            hovered: Some(otto_core::HitTarget::PrivacyMicMute),
            focused: Some(otto_core::HitTarget::PomodoroPlayPause),
            pomodoro: &running_pomo,
            media: Some(media),
            anim_phase: 0.5,
            lyrics: idle_lyrics,
            shelf_state: &empty_shelf,
            clipboard_state: &empty_clipboard,
            tasks_state: &otto_core::TasksState::default(),
            privacy,
            notification: None,
            focus_stats: &otto_core::FocusStatsView::default(),
        })
        .expect("pomodoro panel draw");

    let mut alert_pomo = pomodoro.clone();
    alert_pomo.state.status = otto_core::PomodoroStatus::Alert;
    alert_pomo.state.alert = Some(otto_core::PomodoroAlert {
        completed_phase: otto_core::PomodoroPhase::Focus,
        next_phase: otto_core::PomodoroPhase::ShortBreak,
    });
    let alert_layout = otto_core::compute_layout_with_notch(
        metrics,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            pomodoro_surface: otto_core::PomodoroSurface::Alert,
            media_present: true,
            privacy: privacy_layout,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    renderer
        .draw_panel(&crate::NotchPanelPaintState {
            layout: &alert_layout,
            hovered: Some(otto_core::HitTarget::PomodoroStartNext),
            focused: None,
            pomodoro: &alert_pomo,
            media: Some(media),
            anim_phase: 0.5,
            lyrics: idle_lyrics,
            shelf_state: &empty_shelf,
            clipboard_state: &empty_clipboard,
            tasks_state: &otto_core::TasksState::default(),
            privacy,
            notification: None,
            focus_stats: &otto_core::FocusStatsView::default(),
        })
        .expect("pomodoro alert draw");
}

#[allow(clippy::too_many_arguments)]
fn exercise_batch_2_shelf(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    media: &otto_core::MediaScene,
    privacy: &otto_core::PrivacyState,
) {
    let privacy_layout = otto_core::PrivacyLayoutState {
        compact_dots: true,
        panel_mic_control: true,
    };
    let empty_shelf = sample_batch_shelf(true, 0);
    let full_shelf = sample_batch_shelf(false, 9);
    let empty_clipboard = otto_core::ClipboardState::default();
    let idle_lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    for (shelf_state, shelf_input) in [
        (&empty_shelf, otto_core::ShelfLayoutInput::default()),
        (
            &full_shelf,
            otto_core::ShelfLayoutInput {
                visible_count: u8::try_from(full_shelf.visible_items().len()).unwrap_or(0),
                has_pager: full_shelf.page_count() > 1,
                has_items: true,
            },
        ),
    ] {
        let layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Shelf,
                previous_tab: otto_core::NotchExpandedTab::Shelf,
                shelf: shelf_input,
                media_present: true,
                privacy: privacy_layout,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout: &layout,
                hovered: Some(otto_core::HitTarget::ShelfAdd),
                focused: None,
                pomodoro,
                media: Some(media),
                anim_phase: 0.5,
                lyrics: idle_lyrics,
                shelf_state,
                clipboard_state: &empty_clipboard,
                tasks_state: &otto_core::TasksState::default(),
                privacy,
                notification: None,
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("shelf tab draw");
    }
}

fn sample_batch_clipboard(
    paused: bool,
    count: usize,
    copied_first: bool,
) -> otto_core::ClipboardState {
    let mut state = otto_core::ClipboardState::default();
    for index in 0..count {
        let payload = match index % 3 {
            0 => otto_core::ClipboardPayload::Text(format!(
                "clipboard snippet line {index} with long preview text"
            )),
            1 => otto_core::ClipboardPayload::Link(format!("https://example.com/item/{index}")),
            _ => otto_core::ClipboardPayload::Files(vec![
                format!(r"C:\Temp\clip_{index}.txt"),
                format!(r"C:\Temp\extra_{index}.png"),
            ]),
        };
        state.push_payload(payload);
    }
    state.paused = paused;
    if copied_first && let Some(first) = state.items.first().map(|item| item.id) {
        state.mark_copied(first);
    }
    state
}

#[allow(clippy::too_many_arguments)]
fn exercise_batch_3_clipboard(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    media: &otto_core::MediaScene,
    privacy: &otto_core::PrivacyState,
) {
    let privacy_layout = otto_core::PrivacyLayoutState {
        compact_dots: true,
        panel_mic_control: true,
    };
    let empty_shelf = otto_core::ShelfState::default();
    let idle_lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    let live_empty = sample_batch_clipboard(false, 0, false);
    let paused_empty = sample_batch_clipboard(true, 0, false);
    let populated = sample_batch_clipboard(false, 6, true);
    for (clipboard_state, hover) in [
        (
            &live_empty,
            Some(otto_core::HitTarget::ClipboardPauseToggle),
        ),
        (&paused_empty, None),
        (&populated, Some(otto_core::HitTarget::ClipboardItem(0))),
    ] {
        let layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Clipboard,
                previous_tab: otto_core::NotchExpandedTab::Clipboard,
                clipboard: clipboard_state.layout_input(),
                media_present: true,
                privacy: privacy_layout,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout: &layout,
                hovered: hover,
                focused: Some(otto_core::HitTarget::NotchTabClipboard),
                pomodoro,
                media: Some(media),
                anim_phase: 0.5,
                lyrics: idle_lyrics,
                shelf_state: &empty_shelf,
                clipboard_state,
                tasks_state: &otto_core::TasksState::default(),
                privacy,
                notification: None,
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("clipboard tab draw");
    }
}

fn exercise_tasks_tab(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    privacy: &otto_core::PrivacyState,
) {
    let empty_shelf = otto_core::ShelfState::default();
    let empty_clipboard = otto_core::ClipboardState::default();
    let idle_lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    let empty = otto_core::TasksState::default();
    let mut empty_adding = otto_core::TasksState::default();
    let _ = empty_adding.begin_add();
    let mut populated = otto_core::TasksState::default();
    for index in 0..6 {
        let outcome = populated.add(&format!("render task {index} with a fairly long title"));
        if let otto_core::TaskAddOutcome::Added(id) = outcome {
            if index == 1 {
                populated.toggle_done(id);
            }
            if index == 2 {
                populated.toggle_active(id);
            }
        }
    }
    let mut populated_editing = populated.clone();
    if let Some(first) = populated_editing.items.first().map(|item| item.id) {
        let _ = populated_editing.begin_edit(first);
    }
    for (tasks_state, hover) in [
        (&empty, Some(otto_core::HitTarget::TasksAdd)),
        (&empty_adding, Some(otto_core::HitTarget::TasksAdd)),
        (&populated, Some(otto_core::HitTarget::TaskItem(0))),
        (&populated, Some(otto_core::HitTarget::TaskCheck(1))),
        (&populated, Some(otto_core::HitTarget::TaskEdit(1))),
        (&populated, Some(otto_core::HitTarget::TaskRemove(2))),
        (&populated_editing, Some(otto_core::HitTarget::TaskEdit(0))),
    ] {
        let layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Tasks,
                previous_tab: otto_core::NotchExpandedTab::Tasks,
                tasks: tasks_state.layout_input(),
                ..otto_core::NotchLayoutInput::default()
            },
        );
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout: &layout,
                hovered: hover,
                focused: Some(otto_core::HitTarget::NotchTabTasks),
                pomodoro,
                media: None,
                anim_phase: 0.0,
                lyrics: idle_lyrics,
                shelf_state: &empty_shelf,
                clipboard_state: &empty_clipboard,
                tasks_state,
                privacy,
                notification: None,
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("tasks tab draw");
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn exercise_batch_4_notifications(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    privacy: &otto_core::PrivacyState,
) {
    let empty_shelf = otto_core::ShelfState::default();
    let empty_clipboard = otto_core::ClipboardState::default();
    let idle_lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    let peek_notification = otto_core::NotchNotification {
        id: 1,
        app_id: String::from("Teams.App"),
        app_name: String::from("Teams"),
        kind: otto_core::NotificationKind::Message,
        title: String::from("Ayu Lestari"),
        body: String::from("Are we still meeting at three? I have the slides ready."),
        timestamp_label: String::from("now"),
        badge: otto_core::notification_badge("Teams"),
        icon_bytes: None,
        count: 2,
    };
    let peek_content =
        otto_core::present_notification(&peek_notification, otto_core::NotificationStage::Peek, 1);
    let peek_layout = otto_core::compute_layout_with_notch(
        metrics,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            compact_expansion: 1.0,
            notification: Some(otto_core::NotificationLayoutInput::peek()),
            ..otto_core::NotchLayoutInput::default()
        },
    );
    renderer.set_notification_unread(true);
    renderer
        .draw_bar(&crate::BarPaintState {
            layout: &peek_layout,
            status: otto_core::SystemStatus::default(),
            app_name: "Finder",
            hovered: None,
            focused: None,
            hud: None,
            pomodoro,
            media: None,
            anim_phase: 0.3,
            appearance: otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
            privacy,
            call: None,
            notification: Some(&peek_content),
        })
        .expect("notification peek draw");
    renderer.set_notification_unread(false);

    for (kind, stage, layout_input, body, pending) in [
        (
            otto_core::NotificationKind::Message,
            otto_core::NotificationStage::Banner,
            otto_core::NotificationLayoutInput::banner(false, true),
            "Are we still meeting at three? I have the slides ready.",
            2,
        ),
        (
            otto_core::NotificationKind::Message,
            otto_core::NotificationStage::Banner,
            otto_core::NotificationLayoutInput::banner(true, false),
            "",
            0,
        ),
        (
            otto_core::NotificationKind::VideoCall,
            otto_core::NotificationStage::Ringing,
            otto_core::NotificationLayoutInput::ringing(),
            "Incoming video call",
            0,
        ),
    ] {
        let notification = otto_core::NotchNotification {
            id: 1,
            app_id: String::from("Teams.App"),
            app_name: String::from("Teams"),
            kind,
            title: String::from("Ayu Lestari"),
            body: String::from(body),
            timestamp_label: String::from("2m"),
            badge: otto_core::notification_badge("Teams"),
            icon_bytes: None,
            count: 2,
        };
        let content = otto_core::present_notification(&notification, stage, pending);
        let layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                notification: Some(layout_input),
                ..otto_core::NotchLayoutInput::default()
            },
        );
        let hovered = if layout_input.is_ringing() {
            otto_core::HitTarget::NotificationAccept
        } else {
            otto_core::HitTarget::NotificationClose
        };
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout: &layout,
                hovered: Some(hovered),
                focused: Some(otto_core::HitTarget::NotificationCard),
                pomodoro,
                media: None,
                anim_phase: 0.5,
                lyrics: idle_lyrics,
                shelf_state: &empty_shelf,
                clipboard_state: &empty_clipboard,
                tasks_state: &otto_core::TasksState::default(),
                privacy,
                notification: Some(&content),
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("notification card draw");
    }
}

#[allow(clippy::too_many_arguments)]
fn exercise_batch_1_2_panels_for_dpi(
    renderer: &Renderer,
    metrics: &otto_core::Metrics,
    dpi: u32,
    pomodoro: &otto_core::PomodoroScene,
    media: &otto_core::MediaScene,
    lyrics_sheet: &otto_core::LyricsSheet,
    privacy: &otto_core::PrivacyState,
) {
    exercise_batch_1_focus_and_alert(renderer, metrics, dpi, pomodoro, media, privacy);
    exercise_batch_2_shelf(renderer, metrics, dpi, pomodoro, media, privacy);
    exercise_batch_3_clipboard(renderer, metrics, dpi, pomodoro, media, privacy);
    exercise_batch_4_notifications(renderer, metrics, dpi, pomodoro, privacy);
    let privacy_layout = otto_core::PrivacyLayoutState {
        compact_dots: true,
        panel_mic_control: true,
    };
    let empty_shelf = otto_core::ShelfState::default();
    let empty_clipboard = otto_core::ClipboardState::default();
    let idle_lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    let ready_lyrics = otto_core::LyricsView {
        sheet: Some(lyrics_sheet),
        status: otto_core::LyricsStatus::Ready,
        animated: true,
    };
    for (lyrics_progress, lyrics) in [(0.0_f32, idle_lyrics), (1.0_f32, ready_lyrics)] {
        let layout = otto_core::compute_layout_with_notch(
            metrics,
            1920,
            dpi,
            "Finder",
            otto_core::NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: otto_core::NotchExpandedTab::Media,
                previous_tab: otto_core::NotchExpandedTab::Media,
                media_present: true,
                lyrics_progress,
                privacy: privacy_layout,
                ..otto_core::NotchLayoutInput::default()
            },
        );
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout: &layout,
                hovered: Some(otto_core::HitTarget::MediaPlayPause),
                focused: None,
                pomodoro,
                media: Some(media),
                anim_phase: 0.5,
                lyrics,
                shelf_state: &empty_shelf,
                clipboard_state: &empty_clipboard,
                tasks_state: &otto_core::TasksState::default(),
                privacy,
                notification: None,
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("media tab draw");
    }
}

fn exercise_batch_1_2_for_dpi(dpi: u32) {
    let metrics = otto_core::Metrics::default();
    let max_layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: otto_core::NotchExpandedTab::Media,
            previous_tab: otto_core::NotchExpandedTab::Media,
            media_present: true,
            lyrics_progress: 1.0,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let renderer = Renderer::new(
        1920,
        max_layout.bar.height,
        max_layout.panel_window.width,
        max_layout.panel_window.height,
        dpi,
        0.0,
    )
    .expect("renderer");
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let media = otto_core::MediaScene::steady(
        otto_core::NowPlaying {
            source_app: String::from("Spotify.exe"),
            title: String::from("Midnight Signal"),
            artist: String::from("Lumen Avenue"),
            playing: true,
            position_ms: 1_500,
            duration_ms: 180_000,
            ..otto_core::NowPlaying::default()
        },
        Some(String::from("First lyric line")),
    );
    let lyrics_sheet = otto_core::parse_lrc("[00:01.00]First lyric line\n[00:05.00]Next line");
    let privacy = otto_core::PrivacyState {
        mic: otto_core::MicState::Active { muted: false },
        camera_active: true,
        location_active: false,
        app_name: Some(String::from("meet.exe")),
    };
    exercise_batch_1_2_bars_for_dpi(&renderer, &metrics, dpi, &pomodoro, &media, &privacy);
    exercise_batch_1_2_panels_for_dpi(
        &renderer,
        &metrics,
        dpi,
        &pomodoro,
        &media,
        &lyrics_sheet,
        &privacy,
    );
}

fn exercise_tasks_for_dpi(dpi: u32) {
    let metrics = otto_core::Metrics::default();
    let layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: otto_core::NotchExpandedTab::Tasks,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let renderer = Renderer::new(
        1920,
        layout.bar.height,
        layout.panel_window.width,
        layout.panel_window.height,
        dpi,
        0.0,
    )
    .expect("renderer");
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    exercise_tasks_tab(&renderer, &metrics, dpi, &pomodoro, &privacy);
}

#[test]
fn the_tasks_tab_renders_cleanly_across_dpis_without_leaking() {
    let _serial = SerialResourceTest::begin();
    exercise_tasks_for_dpi(96);
    let baseline = GuiCounts::now();
    for dpi in [96u32, 144, 192] {
        exercise_tasks_for_dpi(dpi);
    }
    baseline.assert_no_growth("tasks tab multi-dpi offscreen render");
    release_shared_factories();
    baseline.assert_no_growth("tasks tab factory release");
}

#[test]
fn batch_1_and_2_panels_and_huds_render_cleanly_across_dpis_without_leaking() {
    let _serial = SerialResourceTest::begin();
    exercise_batch_1_2_for_dpi(96);
    let baseline = GuiCounts::now();
    for dpi in [96u32, 144, 192] {
        exercise_batch_1_2_for_dpi(dpi);
    }
    baseline.assert_no_growth("batch 1 and 2 multi-dpi offscreen render");
    release_shared_factories();
    baseline.assert_no_growth("batch 1 and 2 factory release");
}

#[test]
#[allow(clippy::too_many_lines)]
fn control_center_renders_main_and_subpanels_across_dpis_without_leaking() {
    use otto_core::{
        Appearance, AudioOutputDevice, BluetoothDevice, BluetoothDeviceKind, ControlCenterMetrics,
        ControlCenterPage, ControlCenterPlacement, ControlCenterSnapshot, ResolvedTheme,
        WifiNetwork,
    };
    let _serial = SerialResourceTest::begin();
    let mut snap = ControlCenterSnapshot::default();
    snap.status.brightness = Some(72);
    snap.status.volume = 64;
    snap.settings.plugins.dock = true;
    snap.settings.plugins.top_bar = true;
    snap.settings.plugins.genie = true;
    snap.settings.plugins.window_controls = true;
    snap.settings.dock.hide_taskbar = true;
    snap.settings.dock.reserve_space = true;
    snap.settings.dock.autohide = true;
    snap.settings.system.media_module = true;
    snap.settings.system.focus_module = true;
    snap.settings.system.clipboard_history = true;
    snap.settings.system.drop_shelf = true;
    snap.settings.system.notch_hud = true;
    snap.settings.system.privacy_indicators = true;
    snap.settings.online_lyrics = true;
    snap.wifi.supported = true;
    snap.wifi.enabled = true;
    snap.wifi.connected_ssid = Some(String::from("Studio-5G"));
    snap.wifi.networks.push(WifiNetwork {
        ssid: String::from("Studio-5G"),
        signal_percent: 90,
        secured: true,
        connected: true,
        saved: true,
    });
    snap.bluetooth.supported = true;
    snap.bluetooth.enabled = true;
    snap.bluetooth.devices.push(BluetoothDevice {
        id: String::from("1"),
        name: String::from("AirPods Pro"),
        kind: BluetoothDeviceKind::Headphones,
        connected: true,
        battery_percent: Some(85),
    });
    snap.audio_outputs.push(AudioOutputDevice {
        id: String::from("out-1"),
        name: String::from("Speakers"),
        is_default: true,
    });
    let bounds = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };
    let mut exercise = |dpi: u32| {
        let mut renderer = super::ControlCenterRenderer::new(dpi).expect("cc renderer");
        for page in [
            ControlCenterPage::Main,
            ControlCenterPage::Wifi,
            ControlCenterPage::Bluetooth,
            ControlCenterPage::Sound,
            ControlCenterPage::Display,
            ControlCenterPage::Gallery,
        ] {
            snap.page = page;
            snap.editing = page == ControlCenterPage::Main;
            let layout = otto_core::compute_control_center_layout_for_snapshot(
                &ControlCenterMetrics::DEFAULT,
                ControlCenterPlacement {
                    bounds,
                    dpi,
                    anchor_right: 1750,
                    anchor_y: 36,
                    page,
                    item_count: snap.subpanel_item_count(page),
                    scroll_offset: 0,
                },
                &snap,
            );
            let focused = (page == ControlCenterPage::Main)
                .then_some(otto_core::ControlCenterHitTarget::WifiToggle);
            renderer
                .draw_to_surface(
                    &layout,
                    &snap,
                    super::ControlCenterPaintState {
                        hovered: None,
                        pressed: None,
                        focused,
                        morph_progress: None,
                        editing: snap.editing,
                        ..super::ControlCenterPaintState::default()
                    },
                    &Appearance::glass(ResolvedTheme::Dark),
                )
                .expect("cc draw");
            assert!(!renderer.surface_pixels().is_empty());
        }
    };
    exercise(96);
    let baseline = GuiCounts::now();
    for dpi in [96, 144, 192] {
        exercise(dpi);
    }
    baseline.assert_no_growth("control center render cycles");
    release_shared_factories();
    baseline.assert_no_growth("control center factory release");
}

#[test]
fn repeated_bitmap_uploads_release_every_bitmap() {
    let _serial = SerialResourceTest::begin();
    let target = dc_target();
    let pixels = bitmap_pixels();
    steady_across_cycles("repeated bitmap uploads", &target, SHORT_RUN_CYCLES, |dc| {
        upload_one_bitmap(dc, &pixels);
    });
}

#[test]
#[ignore = "10,000 renderer cycles (about 16 minutes); run with `cargo test -p otto-render -- --ignored`"]
fn ten_thousand_renderer_cycles_keep_gdi_and_user_objects_flat() {
    let _serial = SerialResourceTest::begin();
    construct_each_renderer_once();
    let baseline = GuiCounts::now();
    for cycle in 0..LONG_RUN_CYCLES {
        construct_renderer(cycle);
    }
    baseline.assert_no_growth("10,000 renderer cycles");
    release_shared_factories();
    baseline.assert_no_growth("factory release after 10,000 renderer cycles");
}

#[test]
#[ignore = "10,000 surface, draw and bitmap cycles; run with `cargo test -p otto-render -- --ignored`"]
fn ten_thousand_surface_draw_and_bitmap_cycles_keep_gdi_and_user_objects_flat() {
    let _serial = SerialResourceTest::begin();
    let target = dc_target();
    let pixels = bitmap_pixels();
    steady_across_cycles(
        "10,000 surface, draw and bitmap cycles",
        &target,
        LONG_RUN_CYCLES,
        |dc| surface_draw_and_upload_cycle(dc, &pixels),
    );
}

fn one_by_one_bmp(blue: u8, green: u8, red: u8) -> Vec<u8> {
    vec![
        b'B', b'M', 58, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1,
        0, 24, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, blue,
        green, red, 0,
    ]
}

#[test]
fn cover_art_cache_detects_in_place_byte_mutation() {
    struct ComApartmentGuard(bool);

    impl ComApartmentGuard {
        fn enter() -> Self {
            use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

            Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
        }
    }

    impl Drop for ComApartmentGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    windows::Win32::System::Com::CoUninitialize();
                }
            }
        }
    }

    let _serial = SerialResourceTest::begin();
    let _com = ComApartmentGuard::enter();
    let renderer = Renderer::new(320, 64, 320, 240, 96, 0.25).expect("renderer");
    let mut bytes = one_by_one_bmp(20, 40, 220);
    let original_ptr = bytes.as_ptr();
    let first = renderer
        .get_or_decode_cover_art(&bytes)
        .expect("initial cover decode");
    let cached = renderer
        .get_or_decode_cover_art(&bytes)
        .expect("cached cover hit");
    assert_eq!(first.bitmap.as_raw(), cached.bitmap.as_raw());

    bytes[54] = 240;
    bytes[56] = 15;
    assert_eq!(bytes.as_ptr(), original_ptr);

    let updated = renderer
        .get_or_decode_cover_art(&bytes)
        .expect("mutated cover decode");
    assert_ne!(first.bitmap.as_raw(), updated.bitmap.as_raw());
}

#[test]
fn shelf_bitmap_cache_detects_in_place_byte_mutation() {
    let _serial = SerialResourceTest::begin();
    let renderer = Renderer::new(320, 64, 320, 240, 96, 0.25).expect("renderer");
    let mut shelf = otto_core::ShelfState::default();
    shelf.push_payload(
        otto_core::ShelfPayload::File(String::from(r"C:\Temp\sample.png")),
        false,
        Some(otto_core::ShelfThumbnail {
            width: 2,
            height: 2,
            bgra: vec![
                10, 20, 200, 255, 10, 20, 200, 255, 10, 20, 200, 255, 10, 20, 200, 255,
            ],
        }),
    );
    let mut item = shelf.items[0].clone();
    let original_ptr = item.thumbnail.as_ref().unwrap().bgra.as_ptr();
    let first = renderer
        .resolve_shelf_bitmap(&item)
        .expect("initial shelf bitmap");
    let cached = renderer
        .resolve_shelf_bitmap(&item)
        .expect("cached shelf bitmap");
    assert_eq!(first.as_raw(), cached.as_raw());

    item.thumbnail.as_mut().unwrap().bgra[0] = 250;
    item.thumbnail.as_mut().unwrap().bgra[2] = 15;
    assert_eq!(item.thumbnail.as_ref().unwrap().bgra.as_ptr(), original_ptr);

    let updated = renderer
        .resolve_shelf_bitmap(&item)
        .expect("mutated shelf bitmap");
    assert_ne!(first.as_raw(), updated.as_raw());
}

#[test]
fn renderer_recreate_target_keeps_drawing_and_gui_counts_flat() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let layout = otto_core::compute_layout(&metrics, 1920, 96);
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    let status = otto_core::SystemStatus::default();
    let mut renderer = Renderer::new(
        layout.bar.width,
        layout.bar.height,
        layout.panel_window.width,
        layout.panel_window.height,
        96,
        0.25,
    )
    .expect("renderer");
    let draw_once = |r: &Renderer| {
        r.draw_bar(&crate::BarPaintState {
            layout: &layout,
            status,
            app_name: "Finder",
            hovered: None,
            focused: None,
            hud: None,
            pomodoro: &pomodoro,
            media: None,
            anim_phase: 0.0,
            appearance: otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
            privacy: &privacy,
            call: None,
            notification: None,
        })
        .expect("draw bar");
    };
    draw_once(&renderer);
    let baseline = GuiCounts::now();
    for _ in 0..ROUNDS {
        renderer.recreate_target().expect("recreate target");
        draw_once(&renderer);
    }
    baseline.assert_no_growth("renderer target recreate");
}

#[test]
fn secondary_renderers_recreate_target_without_error() {
    let _serial = SerialResourceTest::begin();
    let pixels = bitmap_pixels();
    let mut control_center = super::ControlCenterRenderer::new(96).expect("control center");
    let mut dock = DockRenderer::new(400, 96, 96).expect("dock");
    dock.set_icon(String::from("app"), BITMAP_SIDE, BITMAP_SIDE, &pixels)
        .expect("dock icon");
    let mut menu = PopupMenuRenderer::new(96).expect("menu");

    let baseline = GuiCounts::now();
    for _ in 0..ROUNDS {
        control_center
            .recreate_target()
            .expect("control center recreate");
        dock.recreate_target().expect("dock recreate");
        assert!(!dock.has_icon("app"));
        menu.recreate_target().expect("menu recreate");
    }
    baseline.assert_no_growth("secondary renderers target recreate");
}

#[test]
fn calendar_and_notification_menus_render_across_dpi_without_resource_growth() {
    let _serial = SerialResourceTest::begin();
    let snapshot = otto_core::NotificationCenterSnapshot {
        access: otto_core::NotificationAccess::Allowed,
        items: vec![
            otto_core::NotificationSummary {
                id: 1,
                app_id: String::from("Slack"),
                app_name: String::from("Slack"),
                title: String::from("Design review starting in 10 minutes"),
                body: String::from("Thread in #otto-ui with updated spacing tokens"),
                timestamp_label: String::from("2m"),
                icon_bytes: None,
            },
            otto_core::NotificationSummary {
                id: 2,
                app_id: String::from("Mail"),
                app_name: String::from("Mail"),
                title: String::from("Weekly build summary"),
                body: String::new(),
                timestamp_label: String::from("1h"),
                icon_bytes: None,
            },
        ],
    };
    let row_sets = [
        otto_core::build_calendar_rows(2026, 4, Some(15)),
        otto_core::build_notification_rows(&snapshot),
    ];
    let metrics = otto_core::PopupMenuMetrics::default();
    let placement_at = |dpi| otto_core::PopupMenuPlacement {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        dpi,
        anchor: otto_core::MenuAnchor::BelowPoint { x: 1580, y: 36 },
        scroll_offset: 0,
    };

    let render_dpis = || {
        for &dpi in &[96, 144, 192] {
            let menu = PopupMenuRenderer::new(dpi).expect("popup menu");
            for rows in &row_sets {
                let layout =
                    otto_core::compute_popup_menu_layout(&metrics, placement_at(dpi), rows, &menu);
                for round in 0..ROUNDS {
                    let hovered = if round % 2 == 0 {
                        rows.iter()
                            .position(|row| matches!(row, otto_core::MenuRow::Notification { .. }))
                    } else {
                        None
                    };
                    menu.render_preview(
                        &layout,
                        rows,
                        crate::MenuHover {
                            row: hovered,
                            calendar: None,
                        },
                        otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
                    )
                    .expect("render calendar or notification menu");
                }
            }
        }
    };

    render_dpis();
    let baseline = GuiCounts::now();
    render_dpis();
    baseline.assert_no_growth("calendar and notification menus");
}

#[test]
#[expect(clippy::too_many_lines)]
fn dock_badges_progress_and_downloads_stack_render_without_leaking() {
    let _serial = SerialResourceTest::begin();
    let pixels = bitmap_pixels();
    let thumbs = vec![
        Some(otto_core::ShelfThumbnail {
            width: BITMAP_SIDE,
            height: BITMAP_SIDE,
            bgra: pixels,
        }),
        None,
        None,
    ];
    let stack_items = vec![
        otto_core::DownloadsStackEntry {
            name: String::from("Wallpaper.png"),
            is_dir: false,
        },
        otto_core::DownloadsStackEntry {
            name: String::from("Builds"),
            is_dir: true,
        },
        otto_core::DownloadsStackEntry {
            name: String::from("Release Notes.pdf"),
            is_dir: false,
        },
    ];
    let bounds = Rect {
        x: 0,
        y: 32,
        width: 1920,
        height: 1048,
    };
    let render_once = || {
        for &dpi in &[96_u32, 144, 192] {
            let mut stack = DownloadsStackRenderer::new(dpi).expect("stack renderer");
            stack.set_thumbnails(&thumbs).expect("stack thumbs");
            let stack_layout =
                otto_core::compute_downloads_stack_layout(1400, 960, bounds, dpi, &stack_items);
            stack
                .render_preview(
                    &stack_layout,
                    &stack_items,
                    otto_core::DownloadsStackHit::Item(1),
                    otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
                    true,
                )
                .expect("render downloads stack");
            stack.recreate_target().expect("stack recreate");
            stack.clear_thumbnails();

            let metrics = otto_core::DockMetrics::MACOS_SONOMA;
            let dock_layout = otto_core::compute_dock_layout(
                &metrics,
                otto_core::DockInput {
                    screen_width: 1920,
                    dpi,
                    app_count: 1,
                    right_count: 1,
                    has_trash: true,
                    cursor_x: None,
                    mag_strength: 0.0,
                    bounce_offsets: &[],
                    item_presence: &[],
                    hide_progress: 0.0,
                },
            );
            let dock = DockRenderer::new(dock_layout.window_width, dock_layout.window_height, dpi)
                .expect("dock renderer");
            let items = [
                DockRenderItem {
                    key: "finder",
                    title: "Finder",
                    apple_icon: Some(otto_core::AppleIconKind::Finder),
                    running: true,
                    active: true,
                    hidden: false,
                    focused: false,
                    badge_count: Some(12),
                    progress: None,
                },
                DockRenderItem {
                    key: "downloads",
                    title: "Downloads",
                    apple_icon: Some(otto_core::AppleIconKind::Folder),
                    running: false,
                    active: false,
                    hidden: false,
                    focused: false,
                    badge_count: Some(120),
                    progress: Some(0.65),
                },
            ];
            dock.render_preview(
                &dock_layout,
                &items,
                otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
            )
            .expect("render dock badges");
        }
    };

    render_once();
    let baseline = GuiCounts::now();
    render_once();
    baseline.assert_no_growth("dock badges and downloads stack");
}

#[test]
fn squircle_corner_blends_cleanly_between_circular_pill_and_g2_spline() {
    let pill = super::SquircleCorner::new(18.0, 200.0, 36.0);
    assert!(pill.smoothing.abs() < f32::EPSILON);
    assert!((pill.reach - 18.0).abs() < 1.0e-4);
    assert!((pill.arc_ctrl_1.1 - 18.0 * (1.0 - 0.552_284_8)).abs() < 1.0e-3);

    let full = super::SquircleCorner::new(16.0, 120.0, 80.0);
    assert!((full.smoothing - 1.0).abs() < 1.0e-5);
    assert!((full.reach - 1.528_665 * 16.0).abs() < 1.0e-3);
    assert!((full.arc_start.0 - 0.076_12 * 16.0).abs() < 1.0e-2);
    assert!((full.arc_start.1 - 0.617_32 * 16.0).abs() < 1.0e-2);
    assert!((full.edge_ctrl_2 - 0.801_09 * 16.0).abs() < 1.0e-2);

    for (w, h, r) in [
        (200.0_f32, 24.0_f32, 12.0_f32),
        (24.0, 200.0, 12.0),
        (40.0, 40.0, 14.0),
        (120.0, 44.0, 16.0),
        (12.0, 12.0, 20.0),
    ] {
        let clamped = r.clamp(0.0, w.min(h) * 0.5);
        let corner = super::SquircleCorner::new(clamped, w, h);
        assert!(corner.reach <= w * 0.5 + 1.0e-4);
        assert!(corner.reach <= h * 0.5 + 1.0e-4);
        assert!(corner.reach >= corner.edge_ctrl_1 - 1.0e-4);
        assert!(corner.edge_ctrl_1 >= corner.edge_ctrl_2 - 1.0e-4);
        assert!(corner.edge_ctrl_2 >= corner.arc_start.1 - 1.0e-4);
        assert!(corner.arc_start.1 >= corner.arc_ctrl_1.1 - 1.0e-4);
        assert!(corner.arc_ctrl_1.1 >= corner.arc_ctrl_2.1 - 1.0e-4);
        assert!(corner.arc_ctrl_2.1 >= corner.arc_end.1 - 1.0e-4);
        assert!(corner.arc_end.1 >= 0.0);
        assert!((corner.arc_start.0 - corner.arc_end.1).abs() < 1.0e-4);
        assert!((corner.arc_ctrl_1.0 - corner.arc_ctrl_2.1).abs() < 1.0e-4);
    }
}

#[test]
fn floating_notch_squircle_tabular_rim_and_secondary_circle_do_not_leak() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let mut layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        144,
        "Finder",
        otto_core::NotchLayoutInput {
            compact_expansion: 1.0,
            secondary_expansion: 0.45,
            media_present: true,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    layout.notch.y = 8;
    layout.notch.height = 42;
    layout.notch_corner_radius = 18;
    layout.notch_ear_radius = 0;
    layout.notch_secondary = Rect {
        x: layout.notch.right() + 4,
        y: layout.notch.y,
        width: 42,
        height: 42,
    };
    let renderer = Renderer::new(
        1920,
        layout.bar.height.max(64),
        layout.panel_window.width,
        layout.panel_window.height,
        144,
        0.0,
    )
    .expect("renderer");
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    let status = otto_core::SystemStatus::default();
    let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);

    let exercise = || {
        for _ in 0..20 {
            renderer
                .draw_bar(&crate::BarPaintState {
                    layout: &layout,
                    status,
                    app_name: "Finder",
                    hovered: Some(otto_core::HitTarget::NotchSecondary),
                    focused: None,
                    hud: None,
                    pomodoro: &pomodoro,
                    media: None,
                    anim_phase: 0.3,
                    appearance,
                    privacy: &privacy,
                    call: None,
                    notification: None,
                })
                .expect("floating bar draw");
            let rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: 8.0,
                top: 8.0,
                right: 120.0,
                bottom: 48.0,
            };
            drop(super::build_squircle_path(&renderer.factory, rect, 12.0).expect("squircle path"));
        }
    };

    exercise();
    let baseline = GuiCounts::now();
    exercise();
    baseline.assert_no_growth("floating notch squircle tabular rim and secondary circle");
}

#[test]
fn shared_tabular_typography_reuses_instance_and_releases_cleanly() {
    let _serial = SerialResourceTest::begin();
    let first = super::shared_tabular_typography().expect("first typography");
    let second = super::shared_tabular_typography().expect("second typography");
    assert_eq!(first.as_raw(), second.as_raw());
    release_shared_factories();
    assert!(super::TABULAR_TYPOGRAPHY.with(|cell| cell.borrow().is_none()));
    let _rebuilt = super::shared_tabular_typography().expect("rebuilt typography");
}

#[test]
fn squircle_cache_reuses_geometry_and_caps_at_64_entries() {
    let _serial = SerialResourceTest::begin();
    let factory = shared_d2d_factory().expect("d2d factory");
    let rect = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
        left: 4.0,
        top: 4.0,
        right: 84.0,
        bottom: 44.0,
    };
    let first = super::build_squircle_path(&factory, rect, 12.0).expect("first squircle");
    let second = super::build_squircle_path(&factory, rect, 12.0).expect("cached squircle");
    assert_eq!(first.as_raw(), second.as_raw());

    for i in 0..70 {
        let r = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
            left: i as f32,
            top: 0.0,
            right: i as f32 + 40.0,
            bottom: 40.0,
        };
        let _ = super::build_squircle_path(&factory, r, 10.0).expect("squircle");
    }
    let len = super::SQUIRCLE_CACHE.with(|cell| cell.borrow().len());
    assert_eq!(len, super::SQUIRCLE_CACHE_CAPACITY);

    release_shared_factories();
    let cleared_len = super::SQUIRCLE_CACHE.with(|cell| cell.borrow().len());
    assert_eq!(cleared_len, 0);
}

fn draw_test_bar(
    renderer: &Renderer,
    layout: &otto_core::Layout,
    hud: Option<otto_core::NotchHudState>,
) -> windows::core::Result<()> {
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    let status = otto_core::SystemStatus::default();
    let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);
    renderer.draw_bar(&crate::BarPaintState {
        layout,
        status,
        app_name: "Finder",
        hovered: None,
        focused: None,
        hud,
        pomodoro: &pomodoro,
        media: None,
        anim_phase: 0.0,
        appearance,
        privacy: &privacy,
        call: None,
        notification: None,
    })
}

#[test]
fn compact_call_activity_draws_for_voice_video_and_muted_calls() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        96,
        "Finder",
        otto_core::NotchLayoutInput {
            compact_expansion: 1.0,
            compact_primary: Some(otto_core::CompactSlotKind::Call),
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let renderer = Renderer::new(
        layout.bar.width,
        layout.bar.height,
        layout.panel_window.width,
        layout.panel_window.height,
        96,
        0.25,
    )
    .expect("renderer");
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    let status = otto_core::SystemStatus::default();
    let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);
    for (video, muted, reduced) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        renderer.set_reduced_motion(reduced);
        let scene = otto_core::CallScene {
            video,
            muted,
            elapsed: String::from("12:34"),
        };
        renderer
            .draw_bar(&crate::BarPaintState {
                layout: &layout,
                status,
                app_name: "Finder",
                hovered: None,
                focused: None,
                hud: None,
                pomodoro: &pomodoro,
                media: None,
                anim_phase: 0.4,
                appearance,
                privacy: &privacy,
                call: Some(&scene),
                notification: None,
            })
            .expect("call compact draw");
    }
    renderer.set_reduced_motion(false);
}

#[test]
fn bar_frame_key_memoizes_when_panel_open_and_skips_present() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let open_layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        96,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let mut renderer = Renderer::new(
        open_layout.bar.width,
        open_layout.bar.height,
        open_layout.panel_window.width,
        open_layout.panel_window.height,
        96,
        0.25,
    )
    .expect("renderer");
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let privacy = otto_core::PrivacyState::default();
    let status = otto_core::SystemStatus::default();
    let appearance = otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark);

    assert!(renderer.last_bar_frame.borrow().is_none());
    draw_test_bar(&renderer, &open_layout, None).expect("open draw_bar");
    assert!(renderer.last_bar_frame.borrow().is_some());

    let panel_hover_fields = renderer
        .bar_frame_fields(
            &open_layout,
            status,
            Some(otto_core::HitTarget::PomodoroPlayPause),
            Some(otto_core::HitTarget::PomodoroReset),
            None,
            appearance,
        )
        .expect("panel hover key");
    assert!(
        renderer
            .last_bar_frame
            .borrow()
            .as_ref()
            .is_some_and(|cached| cached.matches(&panel_hover_fields, "Finder"))
    );

    renderer.last_bar_target.set(Some((0, (0, 0))));
    renderer
        .present(
            0,
            (0, 0),
            &crate::BarPaintState {
                layout: &open_layout,
                status,
                app_name: "Finder",
                hovered: Some(otto_core::HitTarget::PomodoroPlayPause),
                focused: Some(otto_core::HitTarget::PomodoroReset),
                hud: None,
                pomodoro: &pomodoro,
                media: None,
                anim_phase: 0.5,
                appearance,
                privacy: &privacy,
                call: None,
                notification: None,
            },
        )
        .expect("memoized present skips UpdateLayeredWindow");

    let bar_hover_fields = renderer
        .bar_frame_fields(
            &open_layout,
            status,
            Some(otto_core::HitTarget::Start),
            None,
            None,
            appearance,
        )
        .expect("bar hover key");
    assert!(
        !renderer
            .last_bar_frame
            .borrow()
            .as_ref()
            .is_some_and(|cached| cached.matches(&bar_hover_fields, "Finder"))
    );
    assert!(
        !renderer
            .last_bar_frame
            .borrow()
            .as_ref()
            .is_some_and(|cached| cached.matches(&panel_hover_fields, "Safari"))
    );
}

#[test]
fn bar_frame_key_invalidates_on_backdrop_brand_recreate_closed_and_hud() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let open_layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        96,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let closed_layout = otto_core::compute_layout(&metrics, 1920, 96);
    let mut renderer = Renderer::new(
        open_layout.bar.width,
        open_layout.bar.height,
        open_layout.panel_window.width,
        open_layout.panel_window.height,
        96,
        0.25,
    )
    .expect("renderer");
    let draw_open = |r: &Renderer| {
        draw_test_bar(r, &open_layout, None).expect("draw open bar");
        assert!(r.last_bar_frame.borrow().is_some());
    };

    draw_open(&renderer);
    renderer.set_brand_icon(otto_core::BrandIcon::Windows);
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    renderer.set_brand_icon(otto_core::BrandIcon::ClassicApple);
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    renderer.set_notification_unread(true);
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    renderer.clear_bar_backdrop();
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    renderer.set_bar_backdrop(otto_core::RgbBuffer::filled(2, 2, [0.1, 0.2, 0.3]), (0, 0));
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    renderer.recreate_target().expect("recreate_target");
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    draw_test_bar(&renderer, &closed_layout, None).expect("closed bar resets key");
    assert!(renderer.last_bar_frame.borrow().is_none());

    draw_open(&renderer);
    let active_hud = otto_core::NotchHudState {
        kind: otto_core::NotchHudKind::Volume {
            percent: 50,
            muted: false,
        },
        expansion: 1.0,
        content_alpha: 1.0,
        level: 0.5,
    };
    draw_test_bar(&renderer, &open_layout, Some(active_hud)).expect("active hud resets key");
    assert!(renderer.last_bar_frame.borrow().is_none());
}

#[test]
fn panel_surface_allocates_lazily_and_releases_gdi_objects_on_release() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::default();
    let layout = otto_core::compute_layout_with_notch(
        &metrics,
        1920,
        96,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            ..otto_core::NotchLayoutInput::default()
        },
    );
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let shelf = otto_core::ShelfState::default();
    let clipboard = otto_core::ClipboardState::default();
    let tasks = otto_core::TasksState::default();
    let privacy = otto_core::PrivacyState::default();
    let renderer = Renderer::new(
        layout.bar.width,
        layout.bar.height,
        layout.panel_window.width,
        layout.panel_window.height,
        96,
        0.0,
    )
    .expect("renderer");
    assert!(renderer.panel_surface.borrow().is_none());

    draw_test_bar(&renderer, &layout, None).expect("warm-up bar draw");
    assert!(renderer.panel_surface.borrow().is_none());

    let baseline = GuiCounts::now();
    for round in 0..2 {
        renderer
            .draw_panel(&crate::NotchPanelPaintState {
                layout: &layout,
                hovered: None,
                focused: None,
                pomodoro: &pomodoro,
                media: None,
                anim_phase: 0.0,
                lyrics: otto_core::LyricsView {
                    sheet: None,
                    status: otto_core::LyricsStatus::Idle,
                    animated: false,
                },
                shelf_state: &shelf,
                clipboard_state: &clipboard,
                tasks_state: &tasks,
                privacy: &privacy,
                notification: None,
                focus_stats: &otto_core::FocusStatsView::default(),
            })
            .expect("draw panel");
        assert!(renderer.panel_surface.borrow().is_some());
        assert!(
            GuiCounts::now().gdi > baseline.gdi,
            "round {round}: panel surface should allocate GDI objects"
        );

        renderer
            .set_stats_app_icon(String::from("code"), 2, 2, &[0xFF; 16])
            .expect("stats icon");
        assert!(renderer.has_stats_app_icon("code"));

        renderer.release_notch_resources();
        assert!(renderer.panel_surface.borrow().is_none());
        assert!(renderer.cached_cover_art.borrow().is_none());
        assert!(renderer.cached_shelf_bitmaps.borrow().is_empty());
        assert!(renderer.cached_stats_icons.borrow().is_empty());
        baseline.assert_no_growth("panel surface release");
    }
}

fn sample_focus_stats_activity() -> otto_core::FocusActivityState {
    let mut activity = otto_core::FocusActivityState::default();
    activity.today = otto_core::DailyActivityRecord {
        day: otto_core::DayKey {
            year: 2026,
            month: 11,
            day: 22,
        },
        total_active_ms: 3 * 3_600_000 + 25 * 60_000,
        focus_session_ms: 95 * 60_000,
        blocked_attempts: 3,
        apps: vec![
            otto_core::AppUsageEntry {
                app_key: String::from("code"),
                display_name: String::from("VS Code"),
                active_ms: 120 * 60_000,
                open_count: 2,
                click_count: 50,
                keystroke_count: 400,
            },
            otto_core::AppUsageEntry {
                app_key: String::from("chrome"),
                display_name: String::from("Chrome"),
                active_ms: 60 * 60_000,
                open_count: 4,
                click_count: 40,
                keystroke_count: 90,
            },
            otto_core::AppUsageEntry {
                app_key: String::from("windowsterminal"),
                display_name: String::from("Terminal"),
                active_ms: 25 * 60_000,
                open_count: 1,
                click_count: 10,
                keystroke_count: 120,
            },
        ],
        websites: vec![otto_core::WebsiteUsageEntry {
            domain: String::from("github.com"),
            active_ms: 30 * 60_000,
            visit_count: 3,
        }],
        ..otto_core::DailyActivityRecord::default()
    };
    activity.history.push(otto_core::DailyActivityRecord {
        day: otto_core::DayKey {
            year: 2026,
            month: 11,
            day: 21,
        },
        total_active_ms: 2 * 3_600_000,
        focus_session_ms: 60 * 60_000,
        ..otto_core::DailyActivityRecord::default()
    });
    activity
}

fn focus_stats_test_layout(range: otto_core::FocusStatsRange, dpi: u32) -> otto_core::Layout {
    otto_core::compute_layout_with_notch(
        &otto_core::Metrics::DYNAMIC_ISLAND,
        1920,
        dpi,
        "Finder",
        otto_core::NotchLayoutInput {
            panel_expansion: 1.0,
            active_tab: otto_core::NotchExpandedTab::Stats,
            previous_tab: otto_core::NotchExpandedTab::Stats,
            pomodoro_surface: otto_core::PomodoroSurface::Running,
            stats_progress: 1.0,
            stats_range: range,
            ..otto_core::NotchLayoutInput::default()
        },
    )
}

fn draw_focus_stats_case(
    renderer: &Renderer,
    layout: &otto_core::Layout,
    stats: &otto_core::FocusStatsView,
    hover: Option<otto_core::HitTarget>,
    focused: Option<otto_core::HitTarget>,
) {
    let pomodoro = otto_core::PomodoroScene::steady(
        otto_core::PomodoroState::new(),
        otto_core::ClockTime::default(),
    );
    let lyrics = otto_core::LyricsView {
        sheet: None,
        status: otto_core::LyricsStatus::Idle,
        animated: false,
    };
    renderer
        .draw_panel(&crate::NotchPanelPaintState {
            layout,
            hovered: hover,
            focused,
            pomodoro: &pomodoro,
            media: None,
            anim_phase: 0.0,
            lyrics,
            shelf_state: &otto_core::ShelfState::default(),
            clipboard_state: &otto_core::ClipboardState::default(),
            tasks_state: &otto_core::TasksState::default(),
            privacy: &otto_core::PrivacyState::default(),
            notification: None,
            focus_stats: stats,
        })
        .expect("focus stats draw");
}

#[test]
fn focus_stats_tab_renders_today_and_week_without_leaks() {
    let _serial = SerialResourceTest::begin();
    let metrics = otto_core::Metrics::DYNAMIC_ISLAND;
    let activity = sample_focus_stats_activity();
    let empty_stats = otto_core::FocusStatsView::default();
    let today_stats = otto_core::FocusStatsView::resolve(
        &activity,
        otto_core::FocusStatsRange::Today,
        None,
        None,
    );
    let week_stats = otto_core::FocusStatsView::resolve(
        &activity,
        otto_core::FocusStatsRange::Week,
        Some(5),
        Some(6),
    );

    for dpi in [96, 144] {
        let week_layout = focus_stats_test_layout(otto_core::FocusStatsRange::Week, dpi);
        let today_layout = focus_stats_test_layout(otto_core::FocusStatsRange::Today, dpi);
        let renderer = Renderer::new(
            1920,
            otto_core::scale(metrics.bar_height, dpi),
            week_layout.panel_window.width,
            week_layout.panel_window.height,
            dpi,
            0.0,
        )
        .expect("renderer");
        renderer
            .set_stats_app_icon(String::from("code"), 2, 2, &[0xFF; 16])
            .expect("stats icon");
        for (layout, stats, hover) in [
            (
                &today_layout,
                &empty_stats,
                Some(otto_core::HitTarget::NotchTabStats),
            ),
            (
                &today_layout,
                &today_stats,
                Some(otto_core::HitTarget::FocusStatsOpenSettings),
            ),
            (
                &week_layout,
                &week_stats,
                Some(otto_core::HitTarget::FocusStatsDay(6)),
            ),
        ] {
            draw_focus_stats_case(
                &renderer,
                layout,
                stats,
                hover,
                Some(otto_core::HitTarget::NotchTabStats),
            );
        }
        let baseline = GuiCounts::now();
        for _ in 0..10 {
            draw_focus_stats_case(
                &renderer,
                &today_layout,
                &today_stats,
                Some(otto_core::HitTarget::FocusStatsOpenSettings),
                None,
            );
            draw_focus_stats_case(
                &renderer,
                &week_layout,
                &week_stats,
                Some(otto_core::HitTarget::FocusStatsDay(6)),
                None,
            );
        }
        baseline.assert_no_growth("focus stats tab draw loop");
    }
}
