use std::collections::HashSet;
use std::time::Instant;

use otto_core::{
    BarWindowControlsView, FinderHit, FinderLayout, FinderLayoutInput, FinderLocation, FinderPort,
    FinderState, FinderTreeRow, FinderViewMode, Rect, WindowControlsState, compute_finder_layout,
};
use otto_platform::{
    FinderDirWatcher, WindowsFinderPort, extract_finder_thumbnail, is_settings_window_maximized,
    settings_client_size, settings_window_dpi, validate_window_rect,
};
use otto_render::{FinderPaintState, FinderRenderer};

use crate::host::RuntimeContext;

mod input;
mod lifecycle;
mod pointer;

pub(crate) use self::input::handle_message;
pub(crate) use self::lifecycle::{
    close, execute_menu_command, handle_top_bar_command, open, open_path, open_trash, owns_hwnd,
    refresh_appearance, reveal_path, take_hwnd,
};

pub(super) const UI_GLASS: u8 = 1 << 0;
pub(super) const UI_ACTIVE: u8 = 1 << 1;
pub(super) const UI_MAXIMIZED: u8 = 1 << 2;
pub(super) const UI_ALT_DOWN: u8 = 1 << 3;
pub(super) const UI_TRACKING_LEAVE: u8 = 1 << 4;
pub(super) const UI_GREEN_HOVER_ARMED: u8 = 1 << 5;
pub(super) const UI_DRAGGING_SIDEBAR: u8 = 1 << 6;
pub(super) const UI_DRAGGING_SLIDER: u8 = 1 << 7;

pub(crate) struct FinderWindow {
    pub(super) hwnd: isize,
    pub(super) state: FinderState,
    pub(super) layout: FinderLayout,
    pub(super) list_rows: Vec<FinderTreeRow>,
    pub(super) hovered: Option<FinderHit>,
    pub(super) pressed: Option<FinderHit>,
    pub(super) marquee_start: Option<(i32, i32)>,
    pub(super) marquee_rect: Option<Rect>,
    pub(super) drag_paths: Option<Vec<String>>,
    pub(super) spring_target: Option<(String, Instant)>,
    pub(super) last_click: Option<(String, Instant)>,
    pub(super) dpi: u32,
    pub(super) ui_flags: u8,
    pub(super) pending_high_surrogate: Option<u16>,
    pub(super) thumbs_attempted: HashSet<String>,
    pub(super) watcher: Option<FinderDirWatcher>,
    pub(super) renderer: Option<FinderRenderer>,
}

impl FinderWindow {
    pub(super) fn new(hwnd: isize) -> Self {
        let port = WindowsFinderPort;
        let dpi = settings_window_dpi(hwnd);
        let default_loc = port.default_location();
        let sidebar = port.sidebar_items();
        let mut state = FinderState::new(default_loc, sidebar);
        state.tags = port.load_tags();
        state.reload_active_tab(&port);
        let mut ui_flags = UI_ACTIVE;
        if is_settings_window_maximized(hwnd) {
            ui_flags |= UI_MAXIMIZED;
        }
        let mut win = Self {
            hwnd,
            state,
            layout: FinderLayout::default(),
            list_rows: Vec::new(),
            hovered: None,
            pressed: None,
            marquee_start: None,
            marquee_rect: None,
            drag_paths: None,
            spring_target: None,
            last_click: None,
            dpi,
            ui_flags,
            pending_high_surrogate: None,
            thumbs_attempted: HashSet::new(),
            watcher: None,
            renderer: None,
        };
        win.relayout();
        win.sync_watcher();
        win
    }

    pub(super) const fn flag(&self, bit: u8) -> bool {
        (self.ui_flags & bit) != 0
    }

    pub(super) fn set_flag(&mut self, bit: u8, value: bool) {
        if value {
            self.ui_flags |= bit;
        } else {
            self.ui_flags &= !bit;
        }
    }

    pub(super) fn relayout(&mut self) {
        let (width, height) = settings_client_size(self.hwnd);
        self.dpi = settings_window_dpi(self.hwnd);
        self.list_rows = self.state.visible_list_rows();
        self.layout = compute_finder_layout(&FinderLayoutInput {
            width,
            height,
            dpi: self.dpi,
            metrics: self.state.metrics,
            flags: self.state.flags,
            tabs: &self.state.tabs,
            active_tab: self.state.active_tab,
            sidebar_items: &self.state.sidebar_items,
            list_rows: &self.list_rows,
            visible_entries_count: self.state.tab().entries.len(),
            active_columns: &self.state.tab().miller_columns,
        });
    }

    pub(super) fn sync_watcher(&mut self) {
        let active_dir = self.state.active_dir_path();
        match active_dir {
            Some(dir) => {
                if self.watcher.as_ref().is_some_and(|w| w.matches_dir(&dir)) {
                    return;
                }
                self.watcher = FinderDirWatcher::start(&dir, self.hwnd);
            }
            None => self.watcher = None,
        }
    }

    pub(super) fn sync_visible_thumbnails(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let mut paths = Vec::new();
        match self.state.tab().view_mode {
            FinderViewMode::Icon => {
                for tile in &self.layout.icon_tiles {
                    paths.push((tile.path.clone(), 128u32));
                }
            }
            FinderViewMode::Gallery => {
                for thumb in &self.layout.gallery_thumbs {
                    paths.push((thumb.path.clone(), 96u32));
                }
            }
            FinderViewMode::List => {
                for row in &self.layout.list_rows {
                    paths.push((row.path.clone(), 32u32));
                }
            }
            FinderViewMode::Column => {
                for pane in &self.layout.column_panes {
                    for row in &pane.rows {
                        paths.push((row.path.clone(), 32u32));
                    }
                }
            }
        }
        let mut loaded = 0usize;
        for (path, px) in paths {
            if loaded >= 12 {
                break;
            }
            let key = path.to_ascii_lowercase();
            if renderer.has_thumbnail(&key) || !self.thumbs_attempted.insert(key) {
                continue;
            }
            if let Some(entry) = self.state.find_entry_by_path(&path)
                && !entry.is_dir()
                && let Some(bmp) = extract_finder_thumbnail(&path, px)
            {
                let _ = renderer.set_thumbnail(&path, bmp.width, bmp.height, &bmp.pixels);
                loaded += 1;
            }
        }
    }

    pub(super) fn window_controls_view(&self) -> BarWindowControlsView {
        let hovered = matches!(self.hovered, Some(FinderHit::WindowControl(_)));
        let pressed = match self.pressed {
            Some(FinderHit::WindowControl(action)) => Some(action),
            _ => None,
        };
        BarWindowControlsView {
            state: WindowControlsState {
                active: self.flag(UI_ACTIVE),
                close: true,
                minimize: true,
                zoom: true,
                alt_zoom_glyph: hovered && self.flag(UI_ALT_DOWN),
            },
            maximized: self.flag(UI_MAXIMIZED),
            hovered,
            pressed,
        }
    }

    pub(super) fn navigate_to(&mut self, location: FinderLocation) {
        let port = WindowsFinderPort;
        self.state.navigate(&port, location);
        self.thumbs_attempted.clear();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.clear_thumbnails();
        }
        self.relayout();
        self.sync_watcher();
    }
}

pub(super) fn with_window<R>(
    runtime: &RuntimeContext,
    action: impl FnOnce(&mut FinderWindow) -> R,
) -> Option<R> {
    runtime
        .finder_window
        .try_borrow_mut()
        .ok()
        .and_then(|mut guard| guard.as_mut().map(action))
}

pub(super) fn paint(runtime: &RuntimeContext) {
    let app = crate::theme_state::appearance(runtime);
    let accent = app.resolved_accent();
    let dark = app.content_theme().is_dark();
    let high_contrast = app.contrast.is_some();
    let _ = with_window(runtime, |window| {
        let (width, height) = settings_client_size(window.hwnd);
        if width <= 0 || height <= 0 {
            return;
        }
        if window.layout.width != width || window.layout.height != height {
            window.relayout();
        }
        if window.renderer.is_none() {
            match FinderRenderer::new(window.hwnd, width, height, window.dpi) {
                Ok(renderer) => window.renderer = Some(renderer),
                Err(error) => {
                    otto_platform::log_error("finder", &format!("renderer init failed: {error}"));
                }
            }
        }
        window.sync_visible_thumbnails();
        let wc = window.window_controls_view();
        let glass = window.flag(UI_GLASS);
        let paint_state = FinderPaintState {
            state: &window.state,
            layout: &window.layout,
            list_rows: &window.list_rows,
            hovered: window.hovered.as_ref(),
            marquee: window.marquee_rect,
            window_controls: wc,
            dark,
            glass,
            high_contrast,
            accent: accent.fill,
            on_accent: accent.on_accent,
        };
        let draw_result = window
            .renderer
            .as_mut()
            .map(|renderer| renderer.draw(&paint_state));
        if let Some(Err(error)) = draw_result {
            otto_platform::log_error("finder", &format!("draw failed: {error}"));
            window.renderer = None;
        }
        validate_window_rect(window.hwnd);
    });
}
