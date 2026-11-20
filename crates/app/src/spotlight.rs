use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use otto_core::{
    BACKDROP_PADDING_PT, CaptureRegion, ClipboardPayload, MotionMetrics, MotionPreference,
    NotchPort, Rect, SPOTLIGHT_DEFAULT_HEIGHT_POINTS, SPOTLIGHT_DEFAULT_WIDTH_POINTS, SpotlightHit,
    SpotlightLayout, SpotlightMode, SpotlightMove, SpotlightView, animate_value,
    capture_interval_for, capture_wanted, compute_spotlight_layout, move_spotlight_selection,
    rank_spotlight_results, scale,
};
use otto_platform::{
    ClipboardRead, DockEntry, IconBitmap, PrimaryScreen, WindowsNotchPort, close_tray_overflow,
    collect_spotlight_apps, cursor_position, extract_dock_icon, is_running_on_battery,
    launch_new_instance, monitor_work_area_at, motion_preference, primary_screen,
    read_clipboard_payload, release_capture_if_owned,
};
use otto_render::{SpotlightPaintEntry, SpotlightPaintState, SpotlightRenderer};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, ReleaseCapture, SetCapture, SetFocus, VK_BACK, VK_DELETE, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, SW_HIDE, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetForegroundWindow, SetTimer, SetWindowPos, ShowWindow, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_TIMER,
};
const SPOTLIGHT_TIMER: usize = 32;
const SPOTLIGHT_TICK_MS: u32 = 16;
const SPOTLIGHT_CATALOG_TTL: Duration = Duration::from_mins(5);
const SPOTLIGHT_MAX_QUERY_CHARS: usize = 160;
const SPOTLIGHT_MAX_WIDTH_POINTS: i32 = 1_200;
const SPOTLIGHT_MAX_HEIGHT_POINTS: i32 = 900;
const SPOTLIGHT_BACKDROP_MESSAGE: u32 = 0x8000 + 29;
const WM_UNICHAR: u32 = 0x0109;
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

    fn load_catalog(&mut self) {
        if self
            .catalog_loaded_at
            .is_some_and(|loaded| loaded.elapsed() < SPOTLIGHT_CATALOG_TTL)
        {
            return;
        }
        self.apps = collect_spotlight_apps();
        self.catalog_loaded_at = Some(Instant::now());
        self.icons_attempted.clear();
        self.rebuild_results();
    }

    fn rebuild_results(&mut self) {
        self.math_result = SpotlightMode::evaluate_math(&self.query);
        self.filtered = if self.mode == SpotlightMode::Search && self.query.is_empty() {
            Vec::new()
        } else {
            rank_spotlight_results(
                self.apps.iter().map(|entry| entry.title.as_str()),
                &self.query,
            )
        };
        self.first_visible = 0;
        self.hovered = None;
        self.selected = if self.filtered.is_empty() {
            None
        } else if self.mode == SpotlightMode::Search && !self.query.is_empty() {
            Some(0)
        } else {
            None
        };
    }

    fn open(&mut self, runtime: &RuntimeContext, mode: SpotlightMode, trigger: Option<Rect>) {
        self.screen = primary_screen();
        if let Err(error) = self.ensure_renderer() {
            crate::host::log_present_error("spotlight renderer", Err(error));
            if self.visible {
                self.hide();
            }
            return;
        }
        self.motion = motion_preference();
        let trigger = trigger.unwrap_or_else(|| self.default_trigger());
        self.work_area = monitor_work_area_at(trigger.x, trigger.y);
        self.load_catalog();
        self.query.clear();
        self.query_state = QueryEditorState::default();
        self.math_result = None;
        self.pending_high_surrogate = None;
        self.mode = mode;
        self.view = match mode {
            SpotlightMode::Search => SpotlightView::List,
            SpotlightMode::Apps => SpotlightView::Grid,
        };
        self.rebuild_results();
        self.target_window = self.target_rect();
        self.source_window = self.source_rect(trigger);
        self.window = if self.motion.allows_motion() {
            self.source_window
        } else {
            self.target_window
        };
        self.morph_progress = self.motion.allows_motion().then_some(0.0);
        self.morph_target = 1.0;
        self.morph_tick_at = Instant::now();
        self.closing = false;
        self.restore_focus_on_close = false;
        if !self.visible {
            self.previous_foreground =
                unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0
                    as isize;
        }
        self.visible = true;
        self.pending_click = None;
        self.dragging = None;
        crate::menu::close(runtime);
        crate::control_center::close(runtime);
        let _ = close_tray_overflow();
        self.prime_window_backdrop(runtime, false);
        self.place();
        self.sync_backdrop(runtime, true);
        self.present(runtime);
        if self.morph_progress.is_some() {
            self.start_morph_timer();
        } else {
            self.stop_morph_timer();
        }
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNORMAL);
            let _ = SetForegroundWindow(HWND(self.hwnd as *mut _));
            let _ = SetFocus(Some(HWND(self.hwnd as *mut _)));
        }
    }

    fn default_trigger(&self) -> Rect {
        let x = self.screen.width / 2;
        Rect {
            x: x - scale(24, self.screen.dpi),
            y: scale(16, self.screen.dpi),
            width: scale(48, self.screen.dpi),
            height: scale(28, self.screen.dpi),
        }
    }

    fn target_rect(&self) -> Rect {
        let bounds = self.work_area;
        let minimum_width = scale(420, self.screen.dpi).min(bounds.width);
        let minimum_height = scale(300, self.screen.dpi).min(bounds.height);
        let width = scale(SPOTLIGHT_DEFAULT_WIDTH_POINTS, self.screen.dpi)
            .min(scale(SPOTLIGHT_MAX_WIDTH_POINTS, self.screen.dpi))
            .min((bounds.width - scale(32, self.screen.dpi)).max(minimum_width))
            .max(minimum_width);
        let height = scale(SPOTLIGHT_DEFAULT_HEIGHT_POINTS, self.screen.dpi)
            .min(scale(SPOTLIGHT_MAX_HEIGHT_POINTS, self.screen.dpi))
            .min((bounds.height - scale(24, self.screen.dpi)).max(minimum_height))
            .max(minimum_height);
        Rect {
            x: bounds.x + (bounds.width - width) / 2,
            y: (bounds.y + scale(20, self.screen.dpi))
                .clamp(bounds.y, (bounds.bottom() - height).max(bounds.y)),
            width,
            height,
        }
    }

    fn source_rect(&self, trigger: Rect) -> Rect {
        let width = scale(420, self.screen.dpi).min(self.target_window.width);
        let height = scale(300, self.screen.dpi).min(self.target_window.height);
        let center = (
            trigger.x + trigger.width / 2,
            trigger.y + trigger.height / 2,
        );
        Rect {
            x: center.0 - width / 2,
            y: center.1 - height / 2,
            width,
            height,
        }
    }

    fn clamp_window(&self, mut rect: Rect, bounds: Rect) -> Rect {
        let minimum_width = scale(420, self.screen.dpi).min(bounds.width);
        let minimum_height = scale(300, self.screen.dpi).min(bounds.height);
        rect.width = rect.width.clamp(
            minimum_width,
            scale(SPOTLIGHT_MAX_WIDTH_POINTS, self.screen.dpi).min(bounds.width),
        );
        rect.height = rect.height.clamp(
            minimum_height,
            scale(SPOTLIGHT_MAX_HEIGHT_POINTS, self.screen.dpi).min(bounds.height),
        );
        rect.x = rect
            .x
            .clamp(bounds.x, (bounds.right() - rect.width).max(bounds.x));
        rect.y = rect
            .y
            .clamp(bounds.y, (bounds.bottom() - rect.height).max(bounds.y));
        rect
    }

    fn request_close(&mut self, restore_focus: bool) {
        if !self.visible {
            return;
        }
        self.dragging = None;
        self.pending_click = None;
        self.restore_focus_on_close = restore_focus;
        if self.motion.allows_motion() && self.morph_progress.is_some() {
            self.closing = true;
            self.morph_target = 0.0;
            self.morph_tick_at = Instant::now();
            self.start_morph_timer();
        } else if self.motion.allows_motion() {
            self.closing = true;
            self.morph_progress = Some(1.0);
            self.morph_target = 0.0;
            self.morph_tick_at = Instant::now();
            self.start_morph_timer();
        } else {
            self.hide();
        }
    }

    fn start_morph_timer(&self) {
        unsafe {
            let _ = SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                SPOTLIGHT_TIMER,
                SPOTLIGHT_TICK_MS,
                None,
            );
        }
    }

    fn stop_morph_timer(&self) {
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), SPOTLIGHT_TIMER);
        }
    }

    fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.closing = false;
        self.morph_progress = None;
        self.pending_click = None;
        self.dragging = None;
        self.backdrop = None;
        let _ = release_capture_if_owned(self.hwnd);
        unsafe {
            let hwnd = HWND(self.hwnd as *mut _);
            let _ = KillTimer(Some(hwnd), SPOTLIGHT_TIMER);
            let _ = ShowWindow(hwnd, SW_HIDE);
            if self.restore_focus_on_close && self.previous_foreground != 0 {
                let _ = SetForegroundWindow(HWND(self.previous_foreground as *mut _));
            }
        }
        self.restore_focus_on_close = false;
        self.renderer = None;
        otto_platform::trim_working_set();
    }

    fn advance_morph(&mut self, runtime: &RuntimeContext) -> bool {
        let Some(progress) = self.morph_progress else {
            self.stop_morph_timer();
            return false;
        };
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.morph_tick_at)
            .as_secs_f32()
            .clamp(0.001, 0.1);
        self.morph_tick_at = now;
        let next = animate_value(
            progress,
            self.morph_target,
            MotionMetrics::default().spotlight_morph_rate,
            dt,
            self.motion,
        );
        self.morph_progress = Some(next);
        self.window = interpolate_rect(self.source_window, self.target_window, next);
        self.place();
        if self.closing && next <= 0.005 {
            self.morph_progress = None;
            self.hide();
            return false;
        }
        if !self.closing && next >= 0.995 {
            self.window = self.target_window;
            self.morph_progress = None;
            self.stop_morph_timer();
            self.prime_window_backdrop(runtime, true);
            self.sync_backdrop(runtime, true);
        }
        self.present(runtime);
        true
    }

    fn toggle_view(&mut self, view: SpotlightView) {
        if self.view != view {
            self.view = view;
            self.first_visible = 0;
            self.hovered = None;
            self.sync_layout();
            if self.selected.is_some() {
                self.reveal_selection();
            }
        }
    }

    fn set_mode(&mut self, mode: SpotlightMode) {
        if self.mode != mode {
            self.mode = mode;
            self.view = match mode {
                SpotlightMode::Search => SpotlightView::List,
                SpotlightMode::Apps => SpotlightView::Grid,
            };
            self.rebuild_results();
        }
    }

    fn set_query(&mut self, value: char) {
        if SpotlightMode::insert_char(
            &mut self.query,
            &mut self.query_state.cursor,
            &mut self.query_state.selected_all,
            value,
            SPOTLIGHT_MAX_QUERY_CHARS,
        ) {
            self.rebuild_results();
        }
    }

    fn backspace(&mut self) {
        if SpotlightMode::backspace(
            &mut self.query,
            &mut self.query_state.cursor,
            &mut self.query_state.selected_all,
        ) {
            self.rebuild_results();
        }
    }

    fn set_character(&mut self, unit: u16) {
        if (0xD800..=0xDBFF).contains(&unit) {
            self.pending_high_surrogate = Some(unit);
            return;
        }
        if (0xDC00..=0xDFFF).contains(&unit) {
            if let Some(high) = self.pending_high_surrogate.take() {
                let high = u32::from(high - 0xD800);
                let low = u32::from(unit - 0xDC00);
                if let Some(character) = char::from_u32(0x1_0000 + (high << 10) + low) {
                    self.set_query(character);
                }
            }
            return;
        }
        self.pending_high_surrogate = None;
        if let Some(character) = char::from_u32(u32::from(unit)) {
            self.set_query(character);
        }
    }

    fn on_ctrl_keydown(&mut self, runtime: &RuntimeContext, key: u16) -> bool {
        match key {
            0x41 => {
                if !self.query.is_empty() {
                    self.query_state.selected_all = true;
                    self.present(runtime);
                }
                true
            }
            0x56 => {
                if let ClipboardRead::Captured {
                    payload: ClipboardPayload::Text(text) | ClipboardPayload::Link(text),
                    ..
                } = read_clipboard_payload()
                    && SpotlightMode::insert_text(
                        &mut self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                        &text,
                        SPOTLIGHT_MAX_QUERY_CHARS,
                    )
                {
                    self.rebuild_results();
                    self.present(runtime);
                }
                true
            }
            0x43 => {
                if let Some(result) = &self.math_result {
                    let _ =
                        WindowsNotchPort.copy_clipboard(&ClipboardPayload::Text(result.clone()));
                } else if self.query_state.selected_all && !self.query.is_empty() {
                    let _ = WindowsNotchPort
                        .copy_clipboard(&ClipboardPayload::Text(self.query.clone()));
                }
                true
            }
            value if value == VK_BACK.0 => {
                if SpotlightMode::delete_word_backward(
                    &mut self.query,
                    &mut self.query_state.cursor,
                    &mut self.query_state.selected_all,
                ) {
                    self.rebuild_results();
                    self.present(runtime);
                }
                true
            }
            _ => false,
        }
    }

    fn on_keydown(&mut self, runtime: &RuntimeContext, key: u16) {
        let ctrl_down = (unsafe { GetAsyncKeyState(0x11) } as u16 & 0x8000) != 0;
        if ctrl_down && self.on_ctrl_keydown(runtime, key) {
            return;
        }
        let caret_nav_all =
            self.mode == SpotlightMode::Search && (self.filtered.is_empty() || ctrl_down);
        let caret_nav_horizontal =
            caret_nav_all || self.filtered.is_empty() || self.view == SpotlightView::List;
        match key {
            value if value == VK_ESCAPE.0 => self.request_close(true),
            value if value == VK_RETURN.0 => self.activate_selected(),
            value if value == VK_BACK.0 => {
                self.backspace();
                self.present(runtime);
            }
            value if value == VK_DELETE.0 => {
                if SpotlightMode::delete_forward(
                    &mut self.query,
                    &mut self.query_state.cursor,
                    &mut self.query_state.selected_all,
                ) {
                    self.rebuild_results();
                    self.present(runtime);
                }
            }
            value if value == VK_TAB.0 => {
                self.toggle_view(match self.view {
                    SpotlightView::Grid => SpotlightView::List,
                    SpotlightView::List => SpotlightView::Grid,
                });
                self.present(runtime);
            }
            value if value == VK_HOME.0 => {
                if caret_nav_all || self.filtered.is_empty() {
                    if SpotlightMode::move_cursor_home(
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::Home);
                    self.present(runtime);
                }
            }
            value if value == VK_END.0 => {
                if caret_nav_all || self.filtered.is_empty() {
                    if SpotlightMode::move_cursor_end(
                        &self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::End);
                    self.present(runtime);
                }
            }
            value if value == VK_LEFT.0 => {
                if caret_nav_horizontal {
                    if SpotlightMode::move_cursor_left(
                        &self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::Left);
                    self.present(runtime);
                }
            }
            value if value == VK_RIGHT.0 => {
                if caret_nav_horizontal {
                    if SpotlightMode::move_cursor_right(
                        &self.query,
                        &mut self.query_state.cursor,
                        &mut self.query_state.selected_all,
                    ) {
                        self.present(runtime);
                    }
                } else {
                    self.move_selection(SpotlightMove::Right);
                    self.present(runtime);
                }
            }
            value if value == VK_UP.0 => {
                self.move_selection(SpotlightMove::Up);
                self.present(runtime);
            }
            value if value == VK_DOWN.0 => {
                self.move_selection(SpotlightMove::Down);
                self.present(runtime);
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, movement: SpotlightMove) {
        self.selected = move_spotlight_selection(
            self.selected,
            self.filtered.len(),
            self.view,
            self.layout.as_ref().map_or(1, |layout| layout.columns),
            movement,
        );
        self.reveal_selection();
    }

    fn reveal_selection(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        if layout.results.iter().any(|result| result.index == selected) {
            return;
        }
        if self.view == SpotlightView::Grid {
            self.first_visible = selected / layout.columns.max(1) * layout.columns.max(1);
        } else {
            let visible = layout.results.len().max(1);
            self.first_visible = selected.saturating_sub(visible.saturating_sub(1));
        }
    }

    fn scroll(&mut self, delta: i16) {
        if self.filtered.is_empty() || delta == 0 {
            return;
        }
        let unit = if self.view == SpotlightView::Grid {
            self.layout.as_ref().map_or(6, |layout| layout.columns)
        } else {
            1
        };
        let current = self.first_visible as isize;
        let steps = -isize::from(delta.signum()) * unit as isize;
        let max_start = self.filtered.len().saturating_sub(1) as isize;
        let next = (current + steps).clamp(0, max_start) as usize;
        self.first_visible = if self.view == SpotlightView::Grid {
            next / unit * unit
        } else {
            next
        };
        self.selected = Some(self.first_visible.min(self.filtered.len() - 1));
    }

    fn activate_selected(&mut self) {
        if let Some(selected) = self
            .selected
            .or_else(|| (!self.filtered.is_empty()).then_some(0))
        {
            self.launch_result(selected);
            return;
        }
        if let Some(result) = self.math_result.clone() {
            let _ = WindowsNotchPort.copy_clipboard(&ClipboardPayload::Text(result));
            self.request_close(true);
        }
    }

    fn launch_result(&mut self, selected: usize) {
        let Some(app_index) = self.filtered.get(selected).copied() else {
            return;
        };
        let Some(entry) = self.apps.get(app_index).cloned() else {
            return;
        };
        self.hide();
        launch_new_instance(&entry);
    }

    fn handle_click(&mut self, runtime: &RuntimeContext, hit: SpotlightHit) {
        match hit {
            SpotlightHit::SearchTab => {
                self.set_mode(SpotlightMode::Search);
                self.present(runtime);
            }
            SpotlightHit::AppsTab => {
                self.set_mode(SpotlightMode::Apps);
                self.present(runtime);
            }
            SpotlightHit::GridView => {
                self.toggle_view(SpotlightView::Grid);
                self.present(runtime);
            }
            SpotlightHit::ListView => {
                self.toggle_view(SpotlightView::List);
                self.present(runtime);
            }
            SpotlightHit::Result(index) => self.launch_result(index),
            SpotlightHit::SearchField | SpotlightHit::Drag | SpotlightHit::Resize => {}
        }
    }

    fn begin_pointer_action(&mut self, hit: SpotlightHit) {
        if matches!(hit, SpotlightHit::Drag | SpotlightHit::Resize) {
            self.dragging = Some(PointerDrag {
                start_cursor: cursor_position(),
                start_window: self.window,
                resizing: hit == SpotlightHit::Resize,
            });
            unsafe {
                let _ = SetCapture(HWND(self.hwnd as *mut _));
            }
        } else if let SpotlightHit::Result(index) = hit {
            self.selected = Some(index);
            self.pending_click = Some(hit);
        } else {
            self.pending_click = Some(hit);
        }
    }

    fn on_pointer_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if let Some(drag) = self.dragging {
            let cursor = cursor_position();
            let delta = (
                cursor.0 - drag.start_cursor.0,
                cursor.1 - drag.start_cursor.1,
            );
            let mut rect = drag.start_window;
            if drag.resizing {
                rect.width += delta.0;
                rect.height += delta.1;
            } else {
                rect.x += delta.0;
                rect.y += delta.1;
            }
            self.window = self.clamp_window(rect, self.work_area);
            self.target_window = self.window;
            self.source_window = self.window;
            self.morph_progress = None;
            self.place();
            self.sync_backdrop(runtime, false);
            self.present(runtime);
            return;
        }
        let local = (x, y);
        let hovered = self
            .layout
            .as_ref()
            .and_then(|layout| layout.hit_test(local.0, local.1))
            .and_then(|hit| match hit {
                SpotlightHit::Result(index) => Some(index),
                _ => None,
            });
        if hovered != self.hovered {
            self.hovered = hovered;
            self.present(runtime);
        }
    }

    fn end_pointer_action(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if self.dragging.take().is_some() {
            unsafe {
                let _ = ReleaseCapture();
            }
            self.sync_backdrop(runtime, true);
            self.present(runtime);
            return;
        }
        let pressed = self.pending_click.take();
        let released = self
            .layout
            .as_ref()
            .and_then(|layout| layout.hit_test(x, y));
        if pressed == released
            && let Some(hit) = pressed
        {
            self.handle_click(runtime, hit);
        }
    }

    fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (
            update.origin.0 - self.window.x,
            update.origin.1 - self.window.y,
        );
        if let Err(error) = self.ensure_renderer() {
            crate::host::log_present_error("spotlight renderer", Err(error));
            self.hide();
            return;
        }
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.set_backdrop(update.pixels, local);
        }
        self.present(runtime);
    }

    fn prime_window_backdrop(&self, runtime: &RuntimeContext, force: bool) {
        if !capture_wanted(
            &crate::theme_state::appearance(runtime),
            runtime.glass_enabled.get(),
        ) {
            return;
        }
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        if !force && renderer.has_backdrop() {
            return;
        }
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        if let Some(pixels) =
            otto_platform::capture_backdrop_now(region.origin(), region.width, region.height)
        {
            renderer.set_backdrop(pixels, (region.x - self.window.x, region.y - self.window.y));
        }
    }

    fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = self.visible
            && self.hwnd != 0
            && capture_wanted(&appearance, runtime.glass_enabled.get());
        if !wanted {
            if let Some(feed) = self.backdrop.as_mut()
                && feed.is_active()
            {
                feed.reset();
            }
            if let Some(renderer) = self.renderer.as_ref() {
                renderer.clear_backdrop();
            }
            return;
        }
        if !force
            && self.morph_progress.is_some()
            && self
                .renderer
                .as_ref()
                .is_some_and(SpotlightRenderer::has_backdrop)
        {
            return;
        }
        if self.backdrop.is_none() {
            self.backdrop = Some(BackdropFeed::start(
                self.hwnd,
                SPOTLIGHT_BACKDROP_MESSAGE,
                vec![self.hwnd],
                runtime.capture_visible.get(),
            ));
        }
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let capture_rect = if self.morph_progress.is_some() {
            self.target_window
        } else {
            self.window
        };
        let Some(region) = CaptureRegion::padded_to_screen(
            capture_rect,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        let interval = capture_interval_for(is_running_on_battery(), self.visible);
        if let Some(feed) = self.backdrop.as_mut() {
            feed.request(region, interval, force);
        }
    }

    fn ensure_renderer(&mut self) -> windows::core::Result<()> {
        if self.renderer.is_none() {
            self.renderer = Some(SpotlightRenderer::new(self.screen.dpi)?);
        }
        Ok(())
    }

    fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible || self.hwnd == 0 {
            return;
        }
        if let Err(error) = self.ensure_renderer() {
            crate::host::log_present_error("spotlight renderer", Err(error));
            self.hide();
            return;
        }
        let layout = compute_spotlight_layout(
            (self.window.x, self.window.y),
            self.window.width,
            self.window.height,
            self.screen.dpi,
            self.view,
            self.filtered.len(),
            self.first_visible,
        );
        self.layout = Some(layout.clone());
        let visible_icon_keys: HashSet<&str> = layout
            .results
            .iter()
            .filter_map(|result| self.filtered.get(result.index))
            .filter_map(|app_index| self.apps.get(*app_index))
            .map(|entry| entry.key.as_str())
            .collect();
        self.icons_attempted
            .retain(|key| visible_icon_keys.contains(key.as_str()));
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.retain_visible_icons(&visible_icon_keys);
        }
        let mut icons = Vec::<(String, IconBitmap)>::new();
        for result in &layout.results {
            let Some(app_index) = self.filtered.get(result.index).copied() else {
                continue;
            };
            let Some(entry) = self.apps.get(app_index) else {
                continue;
            };
            if self.icons_attempted.insert(entry.key.clone())
                && let Some(icon) = extract_dock_icon(entry)
            {
                icons.push((entry.key.clone(), icon));
            }
        }
        let paint_entries: Vec<SpotlightPaintEntry<'_>> = self
            .filtered
            .iter()
            .filter_map(|index| self.apps.get(*index))
            .map(|entry| SpotlightPaintEntry {
                key: &entry.key,
                title: &entry.title,
            })
            .collect();
        let state = SpotlightPaintState {
            layout: &layout,
            entries: &paint_entries,
            mode: self.mode,
            view: self.view,
            query: &self.query,
            cursor_byte: self.query_state.cursor,
            selected_all: self.query_state.selected_all,
            math_result: self.math_result.as_deref(),
            selected: self.selected,
            hovered: self.hovered,
            content_opacity: self.morph_progress.unwrap_or(1.0),
            appearance: crate::theme_state::appearance(runtime),
        };
        let render_result = self.renderer.as_mut().map(|renderer| {
            for (key, icon) in icons {
                let _ = renderer.set_icon(key, icon.width, icon.height, &icon.pixels);
            }
            renderer.present(self.hwnd, &state)
        });
        if let Some(Err(error)) = render_result {
            crate::host::log_present_error("spotlight", Err(error));
            self.hide();
        }
    }

    fn place(&self) {
        unsafe {
            let _ = SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                self.window.x,
                self.window.y,
                self.window.width,
                self.window.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    fn wheel(&mut self, runtime: &RuntimeContext, delta: i16) {
        self.scroll(delta);
        self.present(runtime);
    }

    fn sync_layout(&mut self) {
        self.layout = Some(compute_spotlight_layout(
            (self.window.x, self.window.y),
            self.window.width,
            self.window.height,
            self.screen.dpi,
            self.view,
            self.filtered.len(),
            self.first_visible,
        ));
    }

    fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.screen = primary_screen();
        self.renderer = None;
        self.icons_attempted.clear();
        if self.visible {
            self.work_area = monitor_work_area_at(self.window.x, self.window.y);
            self.target_window = self.target_rect();
            self.source_window = self.window;
            self.window = self.clamp_window(self.window, self.work_area);
            self.sync_layout();
            self.sync_backdrop(runtime, true);
            self.present(runtime);
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
        SPOTLIGHT_BACKDROP_MESSAGE => {
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
mod tests {
    use super::SpotlightSurface;
    use otto_core::SpotlightMode;

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
}
