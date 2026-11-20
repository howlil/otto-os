use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::{SPOTLIGHT_TIMER, WM_APP_SPOTLIGHT_BACKDROP, WM_UNICHAR};
use otto_core::DockEntry;
use otto_core::{
    MotionPreference, Rect, SPOTLIGHT_DEFAULT_HEIGHT_POINTS, SPOTLIGHT_DEFAULT_WIDTH_POINTS,
    SpotlightHit, SpotlightLayout, SpotlightMode, SpotlightView, scale,
};
use otto_platform::{PrimaryScreen, monitor_work_area_at, primary_screen};
use otto_render::SpotlightRenderer;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_KILLFOCUS,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_TIMER,
};

mod catalog;
mod input;
mod lifecycle;
mod presentation;

const SPOTLIGHT_TICK_MS: u32 = 16;
const SPOTLIGHT_CATALOG_TTL: Duration = Duration::from_mins(5);
const SPOTLIGHT_MAX_QUERY_CHARS: usize = 160;
const SPOTLIGHT_MAX_WIDTH_POINTS: i32 = 1_200;
const SPOTLIGHT_MAX_HEIGHT_POINTS: i32 = 900;
const UNICODE_NOCHAR: usize = 0xFFFF;

#[derive(Clone, Copy, Default)]
struct QueryEditorState {
    cursor: usize,
    selected_all: bool,
}

#[derive(Clone, Copy)]
struct PointerDrag {
    start_cursor: (i32, i32),
    start_window: Rect,
    resizing: bool,
}

pub(crate) struct SpotlightSurface {
    hwnd: isize,
    screen: PrimaryScreen,
    work_area: Rect,
    renderer: Option<SpotlightRenderer>,
    apps: Vec<DockEntry>,
    catalog_loaded_at: Option<Instant>,
    icons_attempted: HashSet<String>,
    query: String,
    query_state: QueryEditorState,
    math_result: Option<String>,
    pending_high_surrogate: Option<u16>,
    mode: SpotlightMode,
    view: SpotlightView,
    filtered: Vec<usize>,
    selected: Option<usize>,
    hovered: Option<usize>,
    first_visible: usize,
    layout: Option<SpotlightLayout>,
    window: Rect,
    source_window: Rect,
    target_window: Rect,
    visible: bool,
    motion: MotionPreference,
    morph_progress: Option<f32>,
    morph_target: f32,
    morph_tick_at: Instant,
    closing: bool,
    restore_focus_on_close: bool,
    previous_foreground: isize,
    pending_click: Option<SpotlightHit>,
    dragging: Option<PointerDrag>,
    backdrop: Option<BackdropFeed>,
}

impl SpotlightSurface {
    fn new(hwnd: isize) -> Self {
        Self {
            hwnd,
            screen: primary_screen(),
            work_area: monitor_work_area_at(0, 0),
            renderer: None,
            apps: Vec::new(),
            catalog_loaded_at: None,
            icons_attempted: HashSet::new(),
            query: String::new(),
            query_state: QueryEditorState::default(),
            math_result: None,
            pending_high_surrogate: None,
            mode: SpotlightMode::Search,
            view: SpotlightView::List,
            filtered: Vec::new(),
            selected: None,
            hovered: None,
            first_visible: 0,
            layout: None,
            window: Rect {
                x: 0,
                y: 0,
                width: scale(420, 96),
                height: scale(300, 96),
            },
            source_window: Rect {
                x: 0,
                y: 0,
                width: scale(420, 96),
                height: scale(300, 96),
            },
            target_window: Rect {
                x: 0,
                y: 0,
                width: scale(SPOTLIGHT_DEFAULT_WIDTH_POINTS, 96),
                height: scale(SPOTLIGHT_DEFAULT_HEIGHT_POINTS, 96),
            },
            visible: false,
            motion: MotionPreference::Full,
            morph_progress: None,
            morph_target: 1.0,
            morph_tick_at: Instant::now(),
            closing: false,
            restore_focus_on_close: false,
            previous_foreground: 0,
            pending_click: None,
            dragging: None,
            backdrop: None,
        }
    }
}

pub fn start(runtime: &RuntimeContext, hwnd: isize) {
    *runtime.spotlight.borrow_mut() = Some(SpotlightSurface::new(hwnd));
}

pub fn stop(runtime: &RuntimeContext) {
    if let Ok(mut guard) = runtime.spotlight.try_borrow_mut()
        && let Some(mut surface) = guard.take()
    {
        surface.restore_focus_on_close = true;
        surface.hide();
    }
}

pub fn open_search(runtime: &RuntimeContext, trigger: Option<Rect>) {
    with_spotlight(runtime, |surface| {
        surface.open(runtime, SpotlightMode::Search, trigger);
    });
}

pub fn open_apps(runtime: &RuntimeContext, trigger: Option<Rect>) {
    with_spotlight(runtime, |surface| {
        surface.open(runtime, SpotlightMode::Apps, trigger);
    });
}

pub fn toggle_from_hotkey(runtime: &RuntimeContext, trigger: Option<Rect>) {
    with_spotlight(runtime, |surface| {
        if surface.visible {
            surface.request_close(true);
        } else {
            surface.open(runtime, SpotlightMode::Search, trigger);
        }
    });
}

pub fn close(runtime: &RuntimeContext) {
    with_spotlight(runtime, |surface| {
        surface.restore_focus_on_close = true;
        surface.hide();
    });
}

pub fn repaint(runtime: &RuntimeContext) {
    with_spotlight(runtime, |surface| {
        if surface.visible {
            surface.present(runtime);
        }
    });
}

pub fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_spotlight(runtime, |surface| {
        if !enabled {
            surface.backdrop = None;
            if let Some(renderer) = surface.renderer.as_ref() {
                renderer.clear_backdrop();
            }
        }
        if surface.visible {
            surface.sync_backdrop(runtime, true);
            surface.present(runtime);
        }
    });
}

pub fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_spotlight(runtime, |surface| {
        if let Some(feed) = surface.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if hwnd == 0 {
        return None;
    }
    match message {
        WM_TIMER if wparam.0 == SPOTLIGHT_TIMER => {
            with_spotlight(runtime, |surface| {
                if surface.advance_morph(runtime) {
                    surface.sync_backdrop(runtime, false);
                }
            });
            Some(LRESULT(0))
        }
        WM_CHAR => {
            with_spotlight(runtime, |surface| {
                surface.set_character(wparam.0 as u16);
                surface.present(runtime);
            });
            Some(LRESULT(0))
        }
        WM_UNICHAR => {
            if wparam.0 == UNICODE_NOCHAR {
                return Some(LRESULT(1));
            }
            with_spotlight(runtime, |surface| {
                if let Some(character) = char::from_u32(wparam.0 as u32) {
                    surface.set_query(character);
                    surface.present(runtime);
                }
            });
            Some(LRESULT(0))
        }
        windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN => {
            with_spotlight(runtime, |surface| {
                surface.on_keydown(runtime, wparam.0 as u16);
            });
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_spotlight(runtime, |surface| surface.on_pointer_move(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_spotlight(runtime, |surface| {
                surface.sync_layout();
                if let Some(hit) = surface
                    .layout
                    .as_ref()
                    .and_then(|layout| layout.hit_test(x, y))
                {
                    surface.begin_pointer_action(hit);
                }
            });
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_spotlight(runtime, |surface| surface.end_pointer_action(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            with_spotlight(runtime, |surface| surface.wheel(runtime, delta));
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_spotlight(runtime, |surface| {
                surface.dragging = None;
                surface.pending_click = None;
            });
            Some(LRESULT(0))
        }
        WM_KILLFOCUS => {
            with_spotlight(runtime, |surface| surface.request_close(false));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_spotlight(runtime, |surface| surface.rebuild(runtime));
            Some(LRESULT(0))
        }
        WM_APP_SPOTLIGHT_BACKDROP => {
            with_spotlight(runtime, |surface| surface.on_backdrop(runtime));
            Some(LRESULT(0))
        }
        WM_CLOSE => {
            with_spotlight(runtime, |surface| surface.request_close(true));
            Some(LRESULT(0))
        }
        _ => None,
    }
}

fn with_spotlight(runtime: &RuntimeContext, action: impl FnOnce(&mut SpotlightSurface)) {
    if let Ok(mut guard) = runtime.spotlight.try_borrow_mut()
        && let Some(surface) = guard.as_mut()
    {
        action(surface);
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn interpolate_rect(from: Rect, to: Rect, progress: f32) -> Rect {
    let progress = progress.clamp(0.0, 1.0);
    let interpolate =
        |start: i32, end: i32| (start as f32 + (end - start) as f32 * progress).round() as i32;
    Rect {
        x: interpolate(from.x, to.x),
        y: interpolate(from.y, to.y),
        width: interpolate(from.width, to.width),
        height: interpolate(from.height, to.height),
    }
}

#[cfg(test)]
mod tests;
