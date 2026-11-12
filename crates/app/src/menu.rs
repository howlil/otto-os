use std::time::{Duration, Instant};

use crate::host::RuntimeContext;
use otto_core::{
    AppMenuState, CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_TODAY, CMD_CC_BRIGHTNESS, CMD_CC_VOLUME,
    CMD_DOCK_OVERFLOW_BASE, CMD_DOWNLOADS_ITEM_BASE, CMD_DOWNLOADS_OPEN_FOLDER, Capability,
    CommandOutcome, ControlCenterEffect, ControlCenterPort, DockPreferences, DownloadsEffect,
    EstimatedMenuText, HitTarget, MenuAnchor, MenuContext, MenuRow, MenuTextMeasure,
    PopupMenuLayout, PopupMenuMetrics, PopupMenuPlacement, Rect, ThemeChange, WindowCommand,
    WindowInfo, WindowMenuEffect, WindowMenuState, build_calendar_rows, build_control_center_rows,
    build_menu_overflow_rows, build_top_bar_menu_rows, build_window_tiling_menu_rows,
    compute_popup_menu_layout, control_center_effect, dock_overflow_choice, dock_settings_effect,
    dock_settings_rows, downloads_effect, overflow_menu_target, scale, shift_month,
    window_menu_effect,
};
use otto_platform::{
    DockEntry, MenuNavEvent, PrimaryScreen, StackItem, WindowsControlCenter, application_of,
    build_dock_context_menu_rows, current_process_id, cursor_position, execute_top_bar_command,
    focus_window_hwnd, monitor_work_area_at, open_downloads_folder, open_path, poll_menu_events,
    primary_screen, read_downloads_stack_items, signal_command_failure, window_command_capability,
};
use otto_render::PopupMenuRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, PostMessageW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetTimer, SetWindowPos, ShowWindow, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const MENU_POLL_TIMER: usize = 30;
const MENU_POLL_MS: u32 = 16;
const OPEN_CLICK_GRACE: Duration = Duration::from_millis(60);
const WHEEL_ROWS_PER_NOTCH: i32 = 2;
const DOWNLOADS_VISIBLE_ITEMS: usize = 24;

pub(crate) struct TopBarMenuRequest<'a> {
    pub bar_hwnd: isize,
    pub target: HitTarget,
    pub anchor_x: i32,
    pub anchor_y: i32,
    pub app_name: &'a str,
    pub window: Option<&'a WindowInfo>,
    pub hidden: &'a [HitTarget],
}

#[derive(Clone)]
enum MenuSource {
    TopBar {
        bar_hwnd: isize,
        target: HitTarget,
        window: Option<WindowInfo>,
    },
    Overflow,
    Dock {
        entry: DockEntry,
        target_rect: Option<Rect>,
    },
    WindowControls {
        window: WindowInfo,
    },
    ControlCenter,
    Calendar {
        year: u16,
        month: u8,
        anchor_right: i32,
        anchor_y: i32,
    },
    DownloadsStack {
        items: Vec<StackItem>,
    },
    DockOverflow {
        entries: Vec<DockEntry>,
    },
    DockSettings,
}

pub(crate) struct PopupMenu {
    hwnd: isize,
    renderer: Option<PopupMenuRenderer>,
    control_center: Box<dyn ControlCenterPort>,
    screen: PrimaryScreen,
    metrics: PopupMenuMetrics,
    rows: Vec<MenuRow>,
    layout: PopupMenuLayout,
    anchor: MenuAnchor,
    scroll_offset: i32,
    hovered_row: Option<usize>,
    source: Option<MenuSource>,
    visible: bool,
    dragging_row: Option<usize>,
    opened_at: Instant,
    was_mouse_down: bool,
    last_cursor: (i32, i32),
}

fn anchor_point(anchor: MenuAnchor) -> (i32, i32) {
    match anchor {
        MenuAnchor::TopBar { x, y } | MenuAnchor::BelowPoint { x, y } => (x, y),
        MenuAnchor::AboveDock { center_x, bottom_y } => (center_x, bottom_y),
    }
}

fn resolve_layout(
    metrics: &PopupMenuMetrics,
    dpi: u32,
    anchor: MenuAnchor,
    scroll_offset: i32,
    rows: &[MenuRow],
    renderer: Option<&PopupMenuRenderer>,
) -> PopupMenuLayout {
    let (x, y) = anchor_point(anchor);
    let placement = PopupMenuPlacement {
        bounds: monitor_work_area_at(x, y),
        dpi,
        anchor,
        scroll_offset,
    };
    let estimated = EstimatedMenuText { dpi };
    let measure: &dyn MenuTextMeasure = match renderer {
        Some(renderer) => renderer,
        None => &estimated,
    };
    compute_popup_menu_layout(metrics, placement, rows, measure)
}

#[allow(clippy::unnecessary_wraps)]
pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let screen = primary_screen();
    let metrics = PopupMenuMetrics::default();
    let anchor = MenuAnchor::TopBar { x: 0, y: 0 };
    let menu = PopupMenu {
        hwnd,
        renderer: None,
        control_center: Box::new(WindowsControlCenter),
        screen,
        layout: resolve_layout(&metrics, screen.dpi, anchor, 0, &[], None),
        metrics,
        rows: Vec::new(),
        anchor,
        scroll_offset: 0,
        hovered_row: None,
        source: None,
        visible: false,
        dragging_row: None,
        opened_at: Instant::now(),
        was_mouse_down: false,
        last_cursor: (0, 0),
    };
    *runtime.menu.borrow_mut() = Some(menu);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    if let Some(mut menu) = runtime.menu.borrow_mut().take() {
        menu.hide();
    }
}

#[must_use]
pub fn active_top_bar_target(runtime: &RuntimeContext) -> Option<HitTarget> {
    let guard = runtime.menu.try_borrow().ok()?;
    let menu = guard.as_ref()?;
    if !menu.visible {
        return None;
    }
    match menu.source.as_ref()? {
        MenuSource::TopBar { target, .. } => Some(*target),
        MenuSource::Overflow => Some(HitTarget::MenuOverflow),
        _ => None,
    }
}

pub fn close(runtime: &RuntimeContext) {
    with_menu(runtime, PopupMenu::hide);
}

fn top_bar_context<'a>(request: &TopBarMenuRequest<'a>, own_pid: u32) -> MenuContext<'a> {
    let Some(window) = request.window else {
        return MenuContext {
            file_manager: request.app_name == "Finder",
            ..MenuContext::without_window(request.app_name)
        };
    };
    let app = if request.target == HitTarget::AppMenu {
        application_of(window.identity, own_pid).map_or(AppMenuState::NONE, |context| {
            AppMenuState::of(&context.app, context.other_apps_visible)
        })
    } else {
        AppMenuState::NONE
    };
    MenuContext {
        app_name: request.app_name,
        file_manager: request.app_name == "Finder",
        tabbed: window.traits.full_screen_shortcut,
        window: Some(WindowMenuState::of(window)),
        app,
    }
}

pub fn open_top_bar_menu(runtime: &RuntimeContext, request: &TopBarMenuRequest<'_>) {
    let anchor = MenuAnchor::TopBar {
        x: request.anchor_x,
        y: request.anchor_y,
    };
    if request.target == HitTarget::MenuOverflow {
        let rows = build_menu_overflow_rows(request.hidden, request.app_name);
        with_menu(runtime, |menu| {
            menu.open(runtime, anchor, rows, MenuSource::Overflow);
        });
        return;
    }
    let rows = build_top_bar_menu_rows(
        request.target,
        &top_bar_context(request, current_process_id()),
    );
    let source = MenuSource::TopBar {
        bar_hwnd: request.bar_hwnd,
        target: request.target,
        window: request.window.cloned(),
    };
    with_menu(runtime, |menu| menu.open(runtime, anchor, rows, source));
}

pub fn open_dock_menu(
    runtime: &RuntimeContext,
    entry: DockEntry,
    other_apps_visible: bool,
    center_x: i32,
    bottom_y: i32,
    target_rect: Option<Rect>,
) {
    let rows = build_dock_context_menu_rows(&entry, other_apps_visible);
    let source = MenuSource::Dock { entry, target_rect };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            rows,
            source,
        );
    });
}

pub fn open_dock_settings_menu(
    runtime: &RuntimeContext,
    center_x: i32,
    bottom_y: i32,
    preferences: DockPreferences,
) {
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            dock_settings_rows(preferences),
            MenuSource::DockSettings,
        );
    });
}

pub fn open_dock_overflow_menu(
    runtime: &RuntimeContext,
    center_x: i32,
    bottom_y: i32,
    entries: Vec<DockEntry>,
) {
    let mut rows = vec![MenuRow::heading("More Apps"), MenuRow::Separator];
    for (index, entry) in entries.iter().enumerate() {
        let marker = if entry.running { "\u{2022} " } else { "" };
        rows.push(MenuRow::item(
            CMD_DOCK_OVERFLOW_BASE + index,
            format!("{marker}{}", entry.title),
            "",
        ));
    }
    let source = MenuSource::DockOverflow { entries };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            rows,
            source,
        );
    });
}

pub fn open_downloads_stack_menu(runtime: &RuntimeContext, center_x: i32, bottom_y: i32) {
    let items = read_downloads_stack_items(DOWNLOADS_VISIBLE_ITEMS);
    let mut rows = vec![MenuRow::heading("Downloads"), MenuRow::Separator];
    if items.is_empty() {
        rows.push(MenuRow::heading("No Recent Downloads"));
    } else {
        for (index, item) in items.iter().enumerate() {
            let prefix = if item.is_dir { "📁 " } else { "📄 " };
            rows.push(MenuRow::item(
                CMD_DOWNLOADS_ITEM_BASE + index,
                format!("{prefix}{}", item.name),
                "",
            ));
        }
    }
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(
        CMD_DOWNLOADS_OPEN_FOLDER,
        "Open in Finder",
        "",
    ));

    let source = MenuSource::DownloadsStack { items };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::AboveDock { center_x, bottom_y },
            rows,
            source,
        );
    });
}

pub fn open_control_center(runtime: &RuntimeContext, anchor_right: i32, anchor_y: i32) {
    with_menu(runtime, |menu| {
        let status = menu.control_center.read_system_status();
        let volume = if status.muted { 0 } else { status.volume };
        let media = menu.control_center.read_now_playing();
        let theme_mode = crate::theme_state::theme_mode(runtime);
        let rows = build_control_center_rows(
            volume,
            status.brightness,
            media
                .as_ref()
                .map(|m| (m.title.as_str(), m.artist.as_str(), m.playing)),
            theme_mode,
            runtime.online_lyrics_enabled.get(),
        );
        let width = scale(menu.metrics.min_width, menu.screen.dpi);
        menu.open(
            runtime,
            MenuAnchor::BelowPoint {
                x: anchor_right - width,
                y: anchor_y,
            },
            rows,
            MenuSource::ControlCenter,
        );
    });
}

pub fn open_calendar(runtime: &RuntimeContext, anchor_right: i32, anchor_y: i32) {
    let mut month = None;
    with_menu(runtime, |menu| {
        let clock = menu.control_center.read_system_status().clock;
        month = Some((clock.year, clock.month));
    });
    if let Some((year, month)) = month {
        open_calendar_month(runtime, year, month, anchor_right, anchor_y);
    }
}

fn open_calendar_month(
    runtime: &RuntimeContext,
    year: u16,
    month: u8,
    anchor_right: i32,
    anchor_y: i32,
) {
    with_menu(runtime, |menu| {
        let clock = menu.control_center.read_system_status().clock;
        let today = (clock.year == year && clock.month == month).then_some(clock.day);
        let rows = build_calendar_rows(year, month, today);
        let width = scale(menu.metrics.min_width, menu.screen.dpi);
        menu.open(
            runtime,
            MenuAnchor::BelowPoint {
                x: anchor_right - width,
                y: anchor_y,
            },
            rows,
            MenuSource::Calendar {
                year,
                month,
                anchor_right,
                anchor_y,
            },
        );
    });
}

#[must_use]
pub fn is_calendar_open(runtime: &RuntimeContext) -> bool {
    runtime.menu.try_borrow().ok().is_some_and(|guard| {
        guard.as_ref().is_some_and(|menu| {
            menu.visible && matches!(menu.source, Some(MenuSource::Calendar { .. }))
        })
    })
}

#[must_use]
pub fn is_control_center_open(runtime: &RuntimeContext) -> bool {
    runtime.menu.try_borrow().ok().is_some_and(|guard| {
        guard.as_ref().is_some_and(|menu| {
            menu.visible && matches!(menu.source, Some(MenuSource::ControlCenter))
        })
    })
}

pub fn open_window_tiling_menu(
    runtime: &RuntimeContext,
    window: &WindowInfo,
    anchor_x: i32,
    anchor_y: i32,
) {
    let rows = build_window_tiling_menu_rows(&WindowMenuState::of(window));
    let source = MenuSource::WindowControls {
        window: window.clone(),
    };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            MenuAnchor::BelowPoint {
                x: anchor_x,
                y: anchor_y,
            },
            rows,
            source,
        );
    });
}

pub fn handle_message(
    runtime: &RuntimeContext,
    _hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_TIMER if wparam.0 == MENU_POLL_TIMER => {
            let mut selected = None;
            with_menu(runtime, |menu| selected = menu.on_poll_timer(runtime));
            if let Some((source, cmd)) = selected {
                dispatch_selection(runtime, source, cmd);
            }
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_menu(runtime, |menu| menu.on_mouse_move(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            with_menu(runtime, |menu| menu.on_wheel(runtime, delta));
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            with_menu(runtime, PopupMenu::end_slider_drag);
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_menu(runtime, |menu| menu.dragging_row = None);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN if slider_pressed(runtime, lparam) => Some(LRESULT(0)),
        WM_LBUTTONDOWN | WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            let mut selected = None;
            with_menu(runtime, |menu| {
                selected = menu.on_click(x, y);
            });
            if let Some((source, cmd)) = selected {
                dispatch_selection(runtime, source, cmd);
            }
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_menu(runtime, PopupMenu::rebuild);
            Some(LRESULT(0))
        }
        WM_CLOSE => {
            with_menu(runtime, PopupMenu::hide);
            Some(LRESULT(0))
        }
        _ => None,
    }
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

fn slider_pressed(runtime: &RuntimeContext, lparam: LPARAM) -> bool {
    let (x, y) = unpack_point(lparam);
    let mut handled = false;
    with_menu(runtime, |menu| {
        handled = menu.begin_slider_drag(runtime, x, y);
    });
    handled
}

fn apply_slider(port: &dyn ControlCenterPort, id: usize, value: u8) -> Option<u8> {
    match id {
        CMD_CC_VOLUME => port.set_volume(value).map(|(volume, _)| volume),
        CMD_CC_BRIGHTNESS => Some(port.set_brightness(value)),
        _ => None,
    }
}

fn with_menu(runtime: &RuntimeContext, action: impl FnOnce(&mut PopupMenu)) {
    if let Ok(mut guard) = runtime.menu.try_borrow_mut()
        && let Some(menu) = guard.as_mut()
    {
        action(menu);
    }
}

pub fn repaint(runtime: &RuntimeContext) {
    with_menu(runtime, |menu| menu.present(runtime));
}

fn report(outcome: CommandOutcome) {
    if !matches!(
        outcome,
        CommandOutcome::Completed | CommandOutcome::Cancelled
    ) {
        signal_command_failure();
    }
}

fn execute_window_menu_command(
    runtime: &RuntimeContext,
    cmd: usize,
    window: Option<&WindowInfo>,
) -> bool {
    let own_pid = current_process_id();
    let minimize_available = window.is_some_and(|window| {
        window_command_capability(window.identity, WindowCommand::Minimize, own_pid)
            == Some(Capability::Available)
    });
    match window_menu_effect(cmd, minimize_available) {
        WindowMenuEffect::CycleTheme => {
            crate::theme_state::cycle_theme_mode(runtime);
            false
        }
        WindowMenuEffect::GenieMinimize => {
            if let Some(window) = window {
                let _ = crate::genie::minimize_window(runtime, window.identity.hwnd);
            }
            false
        }
        WindowMenuEffect::Native(cmd) => {
            let result =
                execute_top_bar_command(cmd, window.map(|window| window.identity), own_pid);
            report(result.outcome);
            result.quit_requested
        }
    }
}

fn dispatch_selection(runtime: &RuntimeContext, source: MenuSource, cmd: usize) {
    match source {
        MenuSource::TopBar {
            bar_hwnd, window, ..
        } => {
            if execute_window_menu_command(runtime, cmd, window.as_ref()) {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(bar_hwnd as *mut _)),
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                    )
                };
            }
        }
        MenuSource::Overflow => {
            if let Some(target) = overflow_menu_target(cmd) {
                crate::bar::activate_hidden_target(runtime, target);
            }
        }
        MenuSource::Dock { entry, target_rect } => {
            crate::dock::execute_dock_menu_command(runtime, cmd, &entry, target_rect);
        }
        MenuSource::ControlCenter => {
            with_menu(runtime, |menu| {
                menu.execute_control_center_command(runtime, cmd);
            });
        }
        MenuSource::Calendar {
            year,
            month,
            anchor_right,
            anchor_y,
        } => match cmd {
            CMD_CAL_PREVIOUS | CMD_CAL_NEXT => {
                let delta = if cmd == CMD_CAL_NEXT { 1 } else { -1 };
                let (next_year, next_month) = shift_month(year, month, delta);
                open_calendar_month(runtime, next_year, next_month, anchor_right, anchor_y);
            }
            CMD_CAL_TODAY => open_calendar(runtime, anchor_right, anchor_y),
            _ => with_menu(runtime, |menu| {
                menu.execute_control_center_command(runtime, cmd);
            }),
        },
        MenuSource::WindowControls { window } => {
            let _ = execute_window_menu_command(runtime, cmd, Some(&window));
        }
        MenuSource::DockSettings => {
            if let Some(change) = dock_settings_effect(cmd) {
                let next = runtime.dock_preferences.get().apply(change);
                runtime.pending_dock_preferences.set(Some(next));
            }
        }
        MenuSource::DockOverflow { entries } => {
            if let Some(index) = dock_overflow_choice(cmd, entries.len()) {
                crate::dock::activate_overflow_entry(runtime, &entries[index]);
            }
        }
        MenuSource::DownloadsStack { items } => match downloads_effect(cmd, items.len()) {
            Some(DownloadsEffect::OpenFolder) => open_downloads_folder(),
            Some(DownloadsEffect::OpenItem(index)) => open_path(&items[index].path),
            None => {}
        },
    }
}

impl PopupMenu {
    fn open(
        &mut self,
        runtime: &RuntimeContext,
        anchor: MenuAnchor,
        rows: Vec<MenuRow>,
        source: MenuSource,
    ) {
        if rows.is_empty() {
            self.hide();
            return;
        }
        self.screen = primary_screen();
        let _ = self.ensure_renderer();
        self.rows = rows;
        self.anchor = anchor;
        self.scroll_offset = 0;
        self.relayout();
        self.hovered_row = None;
        self.dragging_row = None;
        self.source = Some(source);
        self.opened_at = Instant::now();
        self.was_mouse_down = true;
        self.last_cursor = cursor_position();
        self.visible = true;

        self.place();
        self.present(runtime);
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                MENU_POLL_TIMER,
                MENU_POLL_MS,
                None,
            );
        }
    }

    fn relayout(&mut self) {
        self.layout = resolve_layout(
            &self.metrics,
            self.screen.dpi,
            self.anchor,
            self.scroll_offset,
            &self.rows,
            self.renderer.as_ref(),
        );
        self.scroll_offset = self.layout.scroll_offset;
    }

    fn place(&self) {
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                self.layout.window.x,
                self.layout.window.y,
                self.layout.window.width,
                self.layout.window.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    fn scroll_to(&mut self, runtime: &RuntimeContext, offset: i32) {
        if offset != self.scroll_offset {
            self.scroll_offset = offset;
            self.relayout();
        }
        self.present(runtime);
    }

    fn select_row(&mut self, runtime: &RuntimeContext, row: Option<usize>) {
        self.hovered_row = row;
        let offset = row.map_or(self.scroll_offset, |index| {
            self.layout.scroll_revealing(index)
        });
        self.scroll_to(runtime, offset);
    }

    fn on_wheel(&mut self, runtime: &RuntimeContext, delta: i16) {
        if !self.visible || !self.layout.is_scrollable() {
            return;
        }
        let row_height = scale(self.metrics.row_height, self.screen.dpi);
        let notches = i32::from(delta) / 120;
        let step = if notches == 0 {
            -i32::from(delta.signum())
        } else {
            -notches
        };
        let offset = self
            .layout
            .scrolled_by(step * row_height * WHEEL_ROWS_PER_NOTCH);
        self.scroll_to(runtime, offset);
    }

    fn ensure_renderer(&mut self) -> bool {
        if self.renderer.is_none()
            && let Ok(renderer) = PopupMenuRenderer::new(self.screen.dpi)
        {
            self.renderer = Some(renderer);
        }
        self.renderer.is_some()
    }

    fn target_hwnd(&self) -> Option<isize> {
        match self.source.as_ref()? {
            MenuSource::TopBar {
                window: Some(window),
                ..
            }
            | MenuSource::WindowControls { window } => Some(window.identity.hwnd),
            _ => None,
        }
    }

    fn end_slider_drag(&mut self) {
        if self.dragging_row.take().is_some() {
            unsafe {
                let _ = ReleaseCapture();
            }
        }
    }

    fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.end_slider_drag();
        let restore_hwnd = self.target_hwnd();
        self.visible = false;
        self.hovered_row = None;
        self.source = None;
        self.renderer = None;
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), MENU_POLL_TIMER);
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
        }
        if let Some(hwnd) = restore_hwnd {
            focus_window_hwnd(hwnd);
        }
        otto_platform::trim_working_set();
    }

    fn rebuild(&mut self) {
        self.hide();
        self.screen = primary_screen();
        self.renderer = None;
    }

    fn take_selection(&mut self, row: usize) -> Option<(MenuSource, usize)> {
        let cmd = self.layout.command_at(row)?;
        let source = self.source.take()?;
        self.hide();
        Some((source, cmd))
    }

    fn on_poll_timer(&mut self, runtime: &RuntimeContext) -> Option<(MenuSource, usize)> {
        if !self.visible {
            return None;
        }
        let cursor = cursor_position();
        let (gx, gy) = cursor;
        let inside = self.layout.window.contains(gx, gy);
        if cursor != self.last_cursor && self.dragging_row.is_none() {
            self.last_cursor = cursor;
            let pointed = inside
                .then(|| {
                    self.layout
                        .hit_test(gx - self.layout.window.x, gy - self.layout.window.y)
                })
                .flatten();
            if pointed.is_some() && pointed != self.hovered_row {
                self.hovered_row = pointed;
                self.present(runtime);
            }
        }

        let (mouse_down, nav_event) = poll_menu_events();
        match nav_event {
            Some(MenuNavEvent::Escape) => {
                self.hide();
                return None;
            }
            Some(MenuNavEvent::Up) => {
                let row = self.layout.prev_interactive_row(self.hovered_row);
                self.select_row(runtime, row);
            }
            Some(MenuNavEvent::Down) => {
                let row = self.layout.next_interactive_row(self.hovered_row);
                self.select_row(runtime, row);
            }
            Some(MenuNavEvent::Char(ch)) => {
                if let Some(row) = self
                    .layout
                    .find_row_by_prefix(&self.rows, ch, self.hovered_row)
                {
                    self.select_row(runtime, Some(row));
                }
            }
            Some(MenuNavEvent::Enter) => {
                if let Some(row) = self.hovered_row
                    && let Some(selection) = self.take_selection(row)
                {
                    return Some(selection);
                }
            }
            None => {}
        }

        if mouse_down
            && !self.was_mouse_down
            && self.dragging_row.is_none()
            && self.opened_at.elapsed() >= OPEN_CLICK_GRACE
            && !inside
        {
            self.hide();
            return None;
        }
        self.was_mouse_down = mouse_down;
        None
    }

    fn on_mouse_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if !self.visible {
            return;
        }
        if let Some(row) = self.dragging_row {
            self.update_slider(runtime, row, x);
            return;
        }
        let next = self.layout.hit_test(x, y);
        if next != self.hovered_row {
            self.hovered_row = next;
            self.present(runtime);
        }
    }

    fn begin_slider_drag(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> bool {
        if !self.visible {
            return false;
        }
        let Some(row) = self.layout.hit_test(x, y) else {
            return false;
        };
        if self.layout.slider_value_at(row, x).is_none() {
            return false;
        }
        self.dragging_row = Some(row);
        unsafe {
            let _ = SetCapture(HWND(self.hwnd as *mut _));
        }
        self.update_slider(runtime, row, x);
        true
    }

    fn update_slider(&mut self, runtime: &RuntimeContext, row: usize, x: i32) {
        let Some(requested) = self.layout.slider_value_at(row, x) else {
            return;
        };
        let Some(MenuRow::Slider {
            id, value: current, ..
        }) = self.rows.get_mut(row)
        else {
            return;
        };
        if *current == requested {
            return;
        }
        if let Some(applied) = apply_slider(self.control_center.as_ref(), *id, requested) {
            *current = applied.min(100);
        }
        self.present(runtime);
    }

    fn execute_control_center_command(&self, runtime: &RuntimeContext, cmd: usize) {
        match control_center_effect(cmd) {
            Some(ControlCenterEffect::ToggleOnlineLyrics) => {
                let enabled = !runtime.online_lyrics_enabled.get();
                runtime.pending_online_lyrics.set(Some(enabled));
            }
            Some(ControlCenterEffect::Theme(ThemeChange::Cycle)) => {
                crate::theme_state::cycle_theme_mode(runtime);
            }
            Some(ControlCenterEffect::Theme(ThemeChange::Set(mode))) => {
                crate::theme_state::set_theme_mode(runtime, mode);
            }
            Some(ControlCenterEffect::Action(action)) => self.control_center.execute(action),
            None => {}
        }
    }

    fn on_click(&mut self, x: i32, y: i32) -> Option<(MenuSource, usize)> {
        if !self.visible {
            return None;
        }
        let row = self.layout.hit_test(x, y)?;
        if self.layout.slider_value_at(row, x).is_some() {
            return None;
        }
        self.take_selection(row)
    }

    fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible {
            return;
        }
        let _ = self.ensure_renderer();
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let appearance = crate::theme_state::appearance(runtime);
        let _ = renderer.present(
            self.hwnd,
            &self.layout,
            &self.rows,
            self.hovered_row,
            appearance,
        );
    }
}
