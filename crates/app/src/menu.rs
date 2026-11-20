use std::time::{Duration, Instant};

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use otto_core::{
    AppMenuState, BACKDROP_PADDING_PT, CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_TODAY,
    CMD_CONTROL_CENTER, CMD_DOCK_OVERFLOW_BASE, CMD_DOWNLOADS_ITEM_BASE, CMD_DOWNLOADS_OPEN_FOLDER,
    CMD_NOTIF_CLEAR_ALL, CMD_NOTIF_REQUEST_ACCESS, CMD_OPEN_LOGS, CMD_OTTO_SETTINGS, CMD_QUIT,
    CMD_TOGGLE_DOCK, CMD_TOGGLE_NOTCH, CMD_TOGGLE_TOP_BAR, CalendarControl, Capability,
    CaptureRegion, CommandOutcome, ControlCenterEffect, ControlCenterPage, ControlCenterPort,
    DOWNLOADS_STACK_MAX_ITEMS, DockPreferences, DownloadsEffect, DownloadsStackEntry,
    DownloadsStackHit, DownloadsStackLayout, EstimatedMenuText, HitTarget,
    MAX_VISIBLE_NOTIFICATIONS, MenuAnchor, MenuContext, MenuRow, MenuTextMeasure, PopupMenuLayout,
    PopupMenuMetrics, PopupMenuPlacement, Rect, ThemeChange, WindowCommand, WindowInfo,
    WindowMenuEffect, WindowMenuEntry, WindowMenuState, build_calendar_rows,
    build_menu_overflow_rows, build_notification_rows, build_top_bar_menu_rows,
    build_tray_menu_rows, build_window_tiling_menu_rows, capture_interval_for, capture_wanted,
    compute_downloads_stack_layout, compute_popup_menu_layout, control_center_effect,
    dock_overflow_choice, dock_settings_effect, dock_settings_rows, downloads_effect,
    downloads_stack_hit_test, next_downloads_stack_focus, notification_dismiss_index,
    overflow_menu_target, scale, shift_month, window_menu_effect,
};
use otto_platform::{
    ApplicationContext, DockEntry, MenuNavEvent, PrimaryScreen, StackItem, WindowsControlCenter,
    application_of, build_dock_context_menu_rows, current_process_id, cursor_position,
    execute_top_bar_command, focus_window_hwnd, is_running_on_battery, monitor_work_area_at,
    open_downloads_folder, open_path, poll_menu_events, primary_screen, read_downloads_stack_items,
    read_downloads_stack_thumbnails, signal_command_failure, window_command_capability,
};
use otto_render::{DownloadsStackRenderer, MenuHover, PopupMenuRenderer};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, PostMessageW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetTimer, SetWindowPos, ShowWindow, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const MENU_POLL_TIMER: usize = 30;
const MENU_POLL_MS: u32 = 16;
const OPEN_CLICK_GRACE: Duration = Duration::from_millis(60);
const WHEEL_ROWS_PER_NOTCH: i32 = 2;
const WM_APP_MENU_BACKDROP: u32 = 0x8000 + 28;

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
    Calendar {
        year: u16,
        month: u8,
        anchor_right: i32,
        anchor_y: i32,
    },
    Notifications {
        anchor_right: i32,
        anchor_y: i32,
        notification_ids: Vec<u32>,
    },
    DownloadsStack {
        items: Vec<StackItem>,
    },
    DockOverflow {
        entries: Vec<DockEntry>,
    },
    DockSettings,
    Tray {
        from_tray: bool,
    },
}

struct DownloadsStackView {
    items: Vec<StackItem>,
    entries: Vec<DownloadsStackEntry>,
    layout: DownloadsStackLayout,
    active_hit: DownloadsStackHit,
}

pub(crate) struct PopupMenu {
    hwnd: isize,
    renderer: Option<PopupMenuRenderer>,
    stack_renderer: Option<DownloadsStackRenderer>,
    stack_view: Option<DownloadsStackView>,
    control_center: Box<dyn ControlCenterPort>,
    screen: PrimaryScreen,
    metrics: PopupMenuMetrics,
    rows: Vec<MenuRow>,
    layout: PopupMenuLayout,
    anchor: MenuAnchor,
    scroll_offset: i32,
    hovered_row: Option<usize>,
    hovered_calendar: Option<CalendarControl>,
    source: Option<MenuSource>,
    visible: bool,
    opened_at: Instant,
    outside_since: Option<Instant>,
    was_mouse_down: bool,
    last_cursor: (i32, i32),
    backdrop: Option<BackdropFeed>,
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
        stack_renderer: None,
        stack_view: None,
        control_center: Box::new(WindowsControlCenter),
        screen,
        layout: resolve_layout(&metrics, screen.dpi, anchor, 0, &[], None),
        metrics,
        rows: Vec::new(),
        anchor,
        scroll_offset: 0,
        hovered_row: None,
        hovered_calendar: None,
        source: None,
        visible: false,
        opened_at: Instant::now(),
        outside_since: None,
        was_mouse_down: false,
        last_cursor: (0, 0),
        backdrop: None,
    };
    *runtime.menu.borrow_mut() = Some(menu);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    if let Ok(mut guard) = runtime.menu.try_borrow_mut()
        && let Some(mut menu) = guard.take()
    {
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

pub fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_menu(runtime, |menu| {
        if !enabled {
            menu.backdrop = None;
            if let Some(renderer) = menu.renderer.as_ref() {
                renderer.clear_backdrop();
            }
            if let Some(renderer) = menu.stack_renderer.as_ref() {
                renderer.clear_backdrop();
            }
        }
        menu.sync_backdrop(runtime, true);
        menu.present(runtime);
    });
}

pub fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_menu(runtime, |menu| {
        if let Some(feed) = menu.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        menu.sync_backdrop(runtime, true);
        menu.present(runtime);
    });
}

fn truncate_window_title(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let mut out = String::new();
    for ch in trimmed.chars().take(max_chars) {
        out.push(ch);
    }
    if trimmed.chars().count() > max_chars && max_chars > 1 {
        out.pop();
        out.push('…');
    }
    out
}

fn top_bar_context<'a>(
    request: &TopBarMenuRequest<'a>,
    app_context: Option<&ApplicationContext>,
    app_windows: &'a [WindowMenuEntry],
) -> MenuContext<'a> {
    let Some(window) = request.window else {
        return MenuContext {
            file_manager: request.app_name == "Finder",
            ..MenuContext::without_window(request.app_name)
        };
    };
    let app = if request.target == HitTarget::AppMenu {
        app_context.map_or(AppMenuState::NONE, |context| {
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
        app_windows,
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
    let own_pid = current_process_id();
    let app_context = request
        .window
        .filter(|_| matches!(request.target, HitTarget::AppMenu | HitTarget::WindowMenu))
        .and_then(|window| application_of(window.identity, own_pid));
    let app_windows: Vec<WindowMenuEntry> = app_context
        .as_ref()
        .filter(|_| request.target == HitTarget::WindowMenu)
        .map(|context| {
            context
                .app
                .primary_windows()
                .take(64)
                .map(|win| {
                    let title = if win.title.trim().is_empty() {
                        win.app_name.clone()
                    } else {
                        truncate_window_title(&win.title, 36)
                    };
                    WindowMenuEntry {
                        title,
                        active: win.active,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let rows = build_top_bar_menu_rows(
        request.target,
        &top_bar_context(request, app_context.as_ref(), &app_windows),
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

pub fn open_tray_menu(runtime: &RuntimeContext, anchor_x: i32, anchor_y: i32, above_cursor: bool) {
    let rows = build_tray_menu_rows(
        runtime.top_bar_enabled.get(),
        runtime.notch_enabled.get(),
        runtime.dock_enabled.get(),
    );
    let anchor = if above_cursor {
        MenuAnchor::AboveDock {
            center_x: anchor_x,
            bottom_y: anchor_y,
        }
    } else {
        MenuAnchor::BelowPoint {
            x: anchor_x,
            y: anchor_y,
        }
    };
    with_menu(runtime, |menu| {
        menu.open(
            runtime,
            anchor,
            rows,
            MenuSource::Tray {
                from_tray: above_cursor,
            },
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
    let items = read_downloads_stack_items(DOWNLOADS_STACK_MAX_ITEMS);
    with_menu(runtime, |menu| {
        menu.open_downloads_stack(runtime, center_x, bottom_y, items);
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
        let probe = MenuAnchor::BelowPoint {
            x: anchor_right - 1,
            y: anchor_y,
        };
        let width = resolve_layout(
            &menu.metrics,
            menu.screen.dpi,
            probe,
            0,
            &rows,
            menu.renderer.as_ref(),
        )
        .window
        .width;
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

pub fn open_notifications(runtime: &RuntimeContext, anchor_right: i32, anchor_y: i32) {
    with_menu(runtime, |menu| {
        let notifications = menu.control_center.read_notifications();
        let notification_ids = notifications
            .items
            .iter()
            .take(MAX_VISIBLE_NOTIFICATIONS)
            .map(|item| item.id)
            .collect();
        let rows = build_notification_rows(&notifications);
        let probe = MenuAnchor::BelowPoint {
            x: anchor_right - 1,
            y: anchor_y,
        };
        let width = resolve_layout(
            &menu.metrics,
            menu.screen.dpi,
            probe,
            0,
            &rows,
            menu.renderer.as_ref(),
        )
        .window
        .width;
        menu.open(
            runtime,
            MenuAnchor::BelowPoint {
                x: anchor_right - width,
                y: anchor_y,
            },
            rows,
            MenuSource::Notifications {
                anchor_right,
                anchor_y,
                notification_ids,
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
pub fn is_notification_open(runtime: &RuntimeContext) -> bool {
    runtime.menu.try_borrow().ok().is_some_and(|guard| {
        guard.as_ref().is_some_and(|menu| {
            menu.visible && matches!(menu.source, Some(MenuSource::Notifications { .. }))
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
        WM_APP_MENU_BACKDROP => {
            with_menu(runtime, |menu| menu.on_backdrop(runtime));
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
        WindowMenuEffect::OpenOttoSettings => {
            crate::control_center::open_otto_settings(runtime, false);
            false
        }
        WindowMenuEffect::OpenLogs => {
            let _ = otto_platform::open_logs_folder();
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

fn dispatch_tray_selection(runtime: &RuntimeContext, from_tray: bool, cmd: usize) {
    match cmd {
        CMD_OTTO_SETTINGS => crate::control_center::open_otto_settings(runtime, from_tray),
        CMD_CONTROL_CENTER => crate::control_center::toggle_from_host(
            runtime,
            ControlCenterPage::Main,
            false,
            from_tray,
        ),
        CMD_TOGGLE_TOP_BAR => {
            runtime.queue_plugin_toggle(
                crate::host::PluginId::TopBar,
                !runtime.top_bar_enabled.get(),
            );
        }
        CMD_TOGGLE_NOTCH => {
            runtime
                .pending_notch_enabled
                .set(Some(!runtime.notch_enabled.get()));
        }
        CMD_TOGGLE_DOCK => {
            runtime.queue_plugin_toggle(crate::host::PluginId::Dock, !runtime.dock_enabled.get());
        }
        CMD_OPEN_LOGS => {
            let _ = otto_platform::open_logs_folder();
        }
        CMD_QUIT => {
            let bar_hwnd = runtime.bar_hwnd.get();
            if bar_hwnd != 0 {
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
        _ => {}
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
        MenuSource::Notifications {
            anchor_right,
            anchor_y,
            notification_ids,
        } => match cmd {
            CMD_NOTIF_CLEAR_ALL => {
                with_menu(runtime, |menu| {
                    let _ = menu.control_center.clear_notifications();
                });
                open_notifications(runtime, anchor_right, anchor_y);
            }
            CMD_NOTIF_REQUEST_ACCESS => {
                with_menu(runtime, |menu| {
                    let _ = menu.control_center.request_notification_access();
                });
                open_notifications(runtime, anchor_right, anchor_y);
            }
            _ => {
                if let Some(index) = notification_dismiss_index(cmd, notification_ids.len()) {
                    let id = notification_ids[index];
                    with_menu(runtime, |menu| {
                        let _ = menu.control_center.dismiss_notification(id);
                    });
                    open_notifications(runtime, anchor_right, anchor_y);
                } else {
                    with_menu(runtime, |menu| {
                        menu.execute_control_center_command(runtime, cmd);
                    });
                }
            }
        },
        MenuSource::WindowControls { window } => {
            let _ = execute_window_menu_command(runtime, cmd, Some(&window));
        }
        MenuSource::DockSettings => {
            if cmd == CMD_OTTO_SETTINGS {
                crate::control_center::open_otto_settings(runtime, true);
            } else if let Some(change) = dock_settings_effect(cmd) {
                let next = runtime.dock_preferences.get().apply(change);
                runtime.pending_dock_preferences.set(Some(next));
            }
        }
        MenuSource::Tray { from_tray } => dispatch_tray_selection(runtime, from_tray, cmd),
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
        crate::spotlight::close(runtime);
        crate::control_center::close(runtime);
        self.screen = primary_screen();
        self.stack_view = None;
        self.stack_renderer = None;
        let _ = self.ensure_renderer();
        self.rows = rows;
        self.anchor = anchor;
        self.scroll_offset = 0;
        self.relayout();
        self.hovered_row = None;
        self.hovered_calendar = None;
        self.source = Some(source);
        self.opened_at = Instant::now();
        self.outside_since = None;
        self.was_mouse_down = true;
        self.last_cursor = cursor_position();
        self.visible = true;

        self.prime_backdrop(runtime);
        self.place();
        self.sync_backdrop(runtime, true);
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

    fn open_downloads_stack(
        &mut self,
        runtime: &RuntimeContext,
        center_x: i32,
        bottom_y: i32,
        items: Vec<StackItem>,
    ) {
        crate::spotlight::close(runtime);
        crate::control_center::close(runtime);
        self.screen = primary_screen();
        self.rows.clear();
        self.hovered_row = None;
        self.source = None;
        self.renderer = None;

        let entries: Vec<DownloadsStackEntry> = items
            .iter()
            .map(|item| DownloadsStackEntry {
                name: item.name.clone(),
                is_dir: item.is_dir,
            })
            .collect();
        let bounds = monitor_work_area_at(center_x, bottom_y);
        let layout =
            compute_downloads_stack_layout(center_x, bottom_y, bounds, self.screen.dpi, &entries);
        let thumb_px = scale(46, self.screen.dpi) as u32;
        let thumbs = read_downloads_stack_thumbnails(&items, thumb_px);
        if self.stack_renderer.is_none()
            && let Ok(renderer) = DownloadsStackRenderer::new(self.screen.dpi)
        {
            self.stack_renderer = Some(renderer);
        }
        if let Some(renderer) = self.stack_renderer.as_mut() {
            let _ = renderer.set_thumbnails(&thumbs);
        }
        self.layout.window = layout.window;
        self.stack_view = Some(DownloadsStackView {
            items,
            entries,
            layout,
            active_hit: DownloadsStackHit::None,
        });
        self.opened_at = Instant::now();
        self.outside_since = None;
        self.was_mouse_down = true;
        self.last_cursor = cursor_position();
        self.visible = true;

        self.prime_backdrop(runtime);
        self.place();
        self.sync_backdrop(runtime, true);
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

    fn hide(&mut self) {
        if !self.visible {
            return;
        }
        let restore_hwnd = self.target_hwnd();
        self.visible = false;
        self.outside_since = None;
        self.hovered_row = None;
        self.hovered_calendar = None;
        self.source = None;
        self.stack_view = None;
        self.stack_renderer = None;
        self.backdrop = None;
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
        self.stack_renderer = None;
    }

    fn take_selection(&mut self, row: usize) -> Option<(MenuSource, usize)> {
        let cmd = self.layout.command_at(row)?;
        let source = self.source.take()?;
        self.hide();
        Some((source, cmd))
    }

    fn calendar_control_at(&self, x: i32, y: i32) -> Option<CalendarControl> {
        self.layout.calendar_control_at(&self.rows, x, y)
    }

    fn take_calendar_selection(&mut self, control: CalendarControl) -> Option<(MenuSource, usize)> {
        if !matches!(self.source, Some(MenuSource::Calendar { .. })) {
            return None;
        }
        let source = self.source.take()?;
        self.hide();
        Some((source, control.command()))
    }

    fn take_stack_selection(&mut self, hit: DownloadsStackHit) -> Option<(MenuSource, usize)> {
        let cmd = match hit {
            DownloadsStackHit::OpenFolder => CMD_DOWNLOADS_OPEN_FOLDER,
            DownloadsStackHit::Item(index) => CMD_DOWNLOADS_ITEM_BASE + index,
            DownloadsStackHit::None => return None,
        };
        let stack = self.stack_view.take()?;
        self.hide();
        Some((MenuSource::DownloadsStack { items: stack.items }, cmd))
    }

    fn on_stack_poll_timer(&mut self, runtime: &RuntimeContext) -> Option<(MenuSource, usize)> {
        let cursor = cursor_position();
        let (gx, gy) = cursor;
        let (inside, pointed_hit, changed) = {
            let stack = self.stack_view.as_mut()?;
            let inside = stack.layout.window.contains(gx, gy);
            let mut changed = false;
            if cursor != self.last_cursor {
                self.last_cursor = cursor;
                let hit = if inside {
                    downloads_stack_hit_test(
                        &stack.layout,
                        gx - stack.layout.window.x,
                        gy - stack.layout.window.y,
                    )
                } else {
                    DownloadsStackHit::None
                };
                if hit != stack.active_hit {
                    stack.active_hit = hit;
                    changed = true;
                }
            }
            (inside, stack.active_hit, changed)
        };
        if changed {
            self.present(runtime);
        }

        let (mouse_down, nav_event) = poll_menu_events();
        match nav_event {
            Some(MenuNavEvent::Escape) => {
                self.hide();
                return None;
            }
            Some(MenuNavEvent::Enter) => {
                if let Some(selection) = self.take_stack_selection(pointed_hit) {
                    return Some(selection);
                }
            }
            Some(event) => {
                let updated = self.stack_view.as_mut().and_then(|stack| {
                    let (dx, dy) = match event {
                        MenuNavEvent::Left | MenuNavEvent::BackTab => (-1, 0),
                        MenuNavEvent::Right | MenuNavEvent::Tab => (1, 0),
                        MenuNavEvent::Up => (0, -1),
                        MenuNavEvent::Down => (0, 1),
                        _ => return None,
                    };
                    let next = next_downloads_stack_focus(
                        stack.active_hit,
                        stack.entries.len(),
                        stack.layout.columns,
                        dx,
                        dy,
                    );
                    (next != stack.active_hit).then(|| {
                        stack.active_hit = next;
                    })
                });
                if updated.is_some() {
                    self.present(runtime);
                }
            }
            None => {}
        }

        if mouse_down
            && !self.was_mouse_down
            && self.opened_at.elapsed() >= OPEN_CLICK_GRACE
            && !inside
        {
            self.hide();
            return None;
        }
        self.was_mouse_down = mouse_down;
        None
    }

    fn should_auto_close_window_controls(&mut self, gx: i32, gy: i32) -> bool {
        if !matches!(self.source, Some(MenuSource::WindowControls { .. })) {
            return false;
        }
        let padded = Rect {
            x: self.layout.window.x - 18,
            y: self.layout.window.y - 36,
            width: self.layout.window.width + 36,
            height: self.layout.window.height + 54,
        };
        if padded.contains(gx, gy) {
            self.outside_since = None;
            false
        } else {
            self.outside_since.get_or_insert(Instant::now());
            self.outside_since
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(180))
        }
    }

    fn track_pointer(&mut self, runtime: &RuntimeContext, cursor: (i32, i32), inside: bool) {
        let (local_x, local_y) = (
            cursor.0 - self.layout.window.x,
            cursor.1 - self.layout.window.y,
        );
        let pointed = inside
            .then(|| self.layout.hit_test(local_x, local_y))
            .flatten();
        if pointed.is_some() && pointed != self.hovered_row {
            self.hovered_row = pointed;
            self.present(runtime);
        }
        let control = inside
            .then(|| self.calendar_control_at(local_x, local_y))
            .flatten();
        if control != self.hovered_calendar {
            self.hovered_calendar = control;
            self.present(runtime);
        }
    }

    fn on_poll_timer(&mut self, runtime: &RuntimeContext) -> Option<(MenuSource, usize)> {
        if !self.visible {
            return None;
        }
        if self.stack_view.is_some() {
            return self.on_stack_poll_timer(runtime);
        }
        self.sync_backdrop(runtime, false);
        let cursor = cursor_position();
        let (gx, gy) = cursor;
        let inside = self.layout.window.contains(gx, gy);
        if cursor != self.last_cursor {
            self.last_cursor = cursor;
            self.track_pointer(runtime, (gx, gy), inside);
        }

        let (mouse_down, nav_event) = poll_menu_events();
        match nav_event {
            Some(MenuNavEvent::Escape) => {
                self.hide();
                return None;
            }
            Some(MenuNavEvent::Up | MenuNavEvent::BackTab) => {
                let row = self.layout.prev_interactive_row(self.hovered_row);
                self.select_row(runtime, row);
            }
            Some(MenuNavEvent::Down | MenuNavEvent::Tab) => {
                let row = self.layout.next_interactive_row(self.hovered_row);
                self.select_row(runtime, row);
            }
            Some(step @ (MenuNavEvent::Left | MenuNavEvent::Right)) => {
                let control = if matches!(step, MenuNavEvent::Left) {
                    CalendarControl::Previous
                } else {
                    CalendarControl::Next
                };
                if let Some(selection) = self.take_calendar_selection(control) {
                    return Some(selection);
                }
            }
            None => {}
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
        }

        let released = !mouse_down
            && self.was_mouse_down
            && self.opened_at.elapsed() >= OPEN_CLICK_GRACE
            && inside;
        let (local_x, local_y) = (gx - self.layout.window.x, gy - self.layout.window.y);
        if released && let Some(control) = self.calendar_control_at(local_x, local_y) {
            self.was_mouse_down = false;
            if let Some(selection) = self.take_calendar_selection(control) {
                return Some(selection);
            }
        }
        if released
            && let Some(row) = self.hovered_row
            && self.layout.hit_test(local_x, local_y) == Some(row)
        {
            self.was_mouse_down = false;
            if let Some(selection) = self.take_selection(row) {
                return Some(selection);
            }
        }
        if self.should_auto_close_window_controls(gx, gy) {
            self.hide();
            return None;
        }
        if mouse_down
            && !self.was_mouse_down
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
        if let Some(stack) = self.stack_view.as_mut() {
            let next = downloads_stack_hit_test(&stack.layout, x, y);
            if next != stack.active_hit {
                stack.active_hit = next;
                self.present(runtime);
            }
            return;
        }
        let next = self.layout.hit_test(x, y);
        let control = self.calendar_control_at(x, y);
        if next != self.hovered_row || control != self.hovered_calendar {
            self.hovered_row = next;
            self.hovered_calendar = control;
            self.present(runtime);
        }
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
        if let Some(stack) = self.stack_view.as_ref() {
            let hit = downloads_stack_hit_test(&stack.layout, x, y);
            return self.take_stack_selection(hit);
        }
        let row = self.layout.hit_test(x, y)?;
        self.take_selection(row)
    }

    fn prime_backdrop(&self, runtime: &RuntimeContext) {
        let appearance = crate::theme_state::appearance(runtime);
        if !capture_wanted(&appearance, runtime.glass_enabled.get()) {
            return;
        }
        let needs_backdrop = if self.stack_view.is_some() {
            self.stack_renderer
                .as_ref()
                .is_some_and(|renderer| !renderer.has_backdrop())
        } else {
            self.renderer
                .as_ref()
                .is_some_and(|renderer| !renderer.has_backdrop())
        };
        if !needs_backdrop {
            return;
        }
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.layout.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        let Some(pixels) =
            otto_platform::capture_backdrop_now(region.origin(), region.width, region.height)
        else {
            return;
        };
        let local = (
            region.x - self.layout.window.x,
            region.y - self.layout.window.y,
        );
        if self.stack_view.is_some() {
            if let Some(renderer) = self.stack_renderer.as_ref() {
                renderer.set_backdrop(pixels, local);
            }
        } else if let Some(renderer) = self.renderer.as_ref() {
            renderer.set_backdrop(pixels, local);
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
            if let Some(renderer) = self.stack_renderer.as_ref() {
                renderer.clear_backdrop();
            }
            return;
        }
        if self.backdrop.is_none() {
            self.backdrop = Some(BackdropFeed::start(
                self.hwnd,
                WM_APP_MENU_BACKDROP,
                vec![self.hwnd],
                runtime.capture_visible.get(),
            ));
        }
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.layout.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        let interval = capture_interval_for(is_running_on_battery(), self.visible);
        feed.request(region, interval, force);
    }

    fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (
            update.origin.0 - self.layout.window.x,
            update.origin.1 - self.layout.window.y,
        );
        if self.stack_view.is_some() {
            if let Some(renderer) = self.stack_renderer.as_ref() {
                renderer.set_backdrop(update.pixels, local);
            }
        } else if self.ensure_renderer()
            && let Some(renderer) = self.renderer.as_ref()
        {
            renderer.set_backdrop(update.pixels, local);
        }
        self.present(runtime);
    }

    fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible {
            return;
        }
        let appearance = crate::theme_state::appearance(runtime);
        if let (Some(stack), Some(renderer)) =
            (self.stack_view.as_ref(), self.stack_renderer.as_mut())
        {
            crate::host::log_present_error(
                "downloads_stack",
                renderer.present(
                    self.hwnd,
                    &stack.layout,
                    &stack.entries,
                    stack.active_hit,
                    appearance,
                ),
            );
            return;
        }
        let _ = self.ensure_renderer();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        crate::host::log_present_error(
            "menu",
            renderer.present(
                self.hwnd,
                &self.layout,
                &self.rows,
                MenuHover {
                    row: self.hovered_row,
                    calendar: self.hovered_calendar,
                },
                appearance,
            ),
        );
    }
}
