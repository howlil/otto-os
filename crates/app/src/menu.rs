use std::cell::RefCell;
use std::time::{Duration, Instant};

use otto_core::{
    CMD_CAL_NEXT, CMD_CAL_PREVIOUS, CMD_CAL_SETTINGS, CMD_CAL_TODAY, CMD_CC_BLUETOOTH,
    CMD_CC_BRIGHTNESS, CMD_CC_CALENDAR, CMD_CC_DISPLAY, CMD_CC_FOCUS, CMD_CC_MEDIA_NEXT,
    CMD_CC_MEDIA_PREVIOUS, CMD_CC_MEDIA_TOGGLE, CMD_CC_SOUND, CMD_CC_THEME_AUTO, CMD_CC_THEME_DARK,
    CMD_CC_THEME_LIGHT, CMD_CC_THEME_TOGGLE, CMD_CC_VOLUME, CMD_CC_WIFI, CMD_WIN_MINIMIZE,
    ControlCenterAction, ControlCenterPort, HitTarget, MediaCommand, MenuAnchor, MenuRow,
    PopupMenuLayout, PopupMenuMetrics, Rect, build_calendar_rows, build_control_center_rows,
    build_top_bar_menu_rows, build_window_tiling_menu_rows, compute_popup_menu_layout, shift_month,
};
use otto_platform::{
    DockEntry, PrimaryScreen, WindowsControlCenter, build_dock_context_menu_rows, cursor_position,
    execute_top_bar_command, poll_menu_dismiss_keys, primary_screen,
};
use otto_render::PopupMenuRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, PostMessageW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetTimer, SetWindowPos, ShowWindow, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const MENU_POLL_TIMER: usize = 30;
const MENU_POLL_MS: u32 = 16;

thread_local! {
    static MENU: RefCell<Option<PopupMenu>> = const { RefCell::new(None) };
}

#[derive(Clone)]
enum MenuSource {
    TopBar {
        bar_hwnd: isize,
        target: HitTarget,
        app_name: String,
        target_hwnd: isize,
    },
    Dock {
        entry: DockEntry,
        target_rect: Option<Rect>,
    },
    WindowControls {
        target_hwnd: isize,
    },
    ControlCenter,
    Calendar {
        year: u16,
        month: u8,
        anchor_right: i32,
        anchor_y: i32,
    },
}

struct PopupMenu {
    hwnd: isize,
    renderer: PopupMenuRenderer,
    control_center: Box<dyn ControlCenterPort>,
    screen: PrimaryScreen,
    metrics: PopupMenuMetrics,
    rows: Vec<MenuRow>,
    layout: PopupMenuLayout,
    hovered_row: Option<usize>,
    source: Option<MenuSource>,
    visible: bool,
    dragging_row: Option<usize>,
    opened_at: Instant,
    was_mouse_down: bool,
}

pub fn start(hwnd: isize) -> Result<()> {
    let screen = primary_screen();
    let metrics = PopupMenuMetrics::default();
    let renderer = PopupMenuRenderer::new(screen.dpi)?;
    let layout = compute_popup_menu_layout(
        &metrics,
        screen.width,
        screen.height,
        screen.dpi,
        MenuAnchor::TopBar { x: 0, y: 0 },
        &[],
    );
    let menu = PopupMenu {
        hwnd,
        renderer,
        control_center: Box::new(WindowsControlCenter),
        screen,
        metrics,
        rows: Vec::new(),
        layout,
        hovered_row: None,
        source: None,
        visible: false,
        dragging_row: None,
        opened_at: Instant::now(),
        was_mouse_down: false,
    };
    MENU.with(|cell| *cell.borrow_mut() = Some(menu));
    Ok(())
}

pub fn stop() {
    MENU.with(|cell| {
        if let Some(mut menu) = cell.borrow_mut().take() {
            menu.hide();
        }
    });
}

#[must_use]
pub fn active_top_bar_target() -> Option<HitTarget> {
    MENU.with(|cell| {
        let guard = cell.try_borrow().ok()?;
        let menu = guard.as_ref()?;
        if !menu.visible {
            return None;
        }
        match menu.source.as_ref()? {
            MenuSource::TopBar { target, .. } => Some(*target),
            _ => None,
        }
    })
}

pub fn close() {
    with_menu(PopupMenu::hide);
}

pub fn open_top_bar_menu(
    bar_hwnd: isize,
    target: HitTarget,
    anchor_x: i32,
    anchor_y: i32,
    app_name: &str,
    target_hwnd: isize,
) {
    let rows = build_top_bar_menu_rows(target, app_name);
    let source = MenuSource::TopBar {
        bar_hwnd,
        target,
        app_name: String::from(app_name),
        target_hwnd,
    };
    with_menu(|menu| {
        menu.open(
            MenuAnchor::TopBar {
                x: anchor_x,
                y: anchor_y,
            },
            rows,
            source,
        );
    });
}

pub fn open_dock_menu(entry: DockEntry, center_x: i32, bottom_y: i32, target_rect: Option<Rect>) {
    let rows = build_dock_context_menu_rows(&entry);
    let source = MenuSource::Dock { entry, target_rect };
    with_menu(|menu| {
        menu.open(MenuAnchor::AboveDock { center_x, bottom_y }, rows, source);
    });
}

pub fn open_control_center(anchor_right: i32, anchor_y: i32) {
    with_menu(|menu| {
        let status = menu.control_center.read_system_status();
        let volume = if status.muted { 0 } else { status.volume };
        let media = menu.control_center.read_now_playing();
        let theme_mode = crate::theme_state::theme_mode();
        let rows = build_control_center_rows(
            volume,
            status.brightness,
            media
                .as_ref()
                .map(|m| (m.title.as_str(), m.artist.as_str(), m.playing)),
            theme_mode,
        );
        let width = otto_core::scale(menu.metrics.min_width, menu.screen.dpi);
        menu.open(
            MenuAnchor::BelowPoint {
                x: anchor_right - width,
                y: anchor_y,
            },
            rows,
            MenuSource::ControlCenter,
        );
    });
}

pub fn open_calendar(anchor_right: i32, anchor_y: i32) {
    let mut month = None;
    with_menu(|menu| {
        let clock = menu.control_center.read_system_status().clock;
        month = Some((clock.year, clock.month));
    });
    if let Some((year, month)) = month {
        open_calendar_month(year, month, anchor_right, anchor_y);
    }
}

fn open_calendar_month(year: u16, month: u8, anchor_right: i32, anchor_y: i32) {
    let mut today = None;
    with_menu(|menu| {
        let clock = menu.control_center.read_system_status().clock;
        today = (clock.year == year && clock.month == month).then_some(clock.day);
    });
    let rows = build_calendar_rows(year, month, today);
    with_menu(|menu| {
        let width = otto_core::scale(menu.metrics.min_width, menu.screen.dpi);
        menu.open(
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
pub fn is_calendar_open() -> bool {
    MENU.with(|cell| {
        cell.try_borrow().ok().is_some_and(|guard| {
            guard.as_ref().is_some_and(|menu| {
                menu.visible && matches!(menu.source, Some(MenuSource::Calendar { .. }))
            })
        })
    })
}

#[must_use]
pub fn is_control_center_open() -> bool {
    MENU.with(|cell| {
        cell.try_borrow().ok().is_some_and(|guard| {
            guard.as_ref().is_some_and(|menu| {
                menu.visible && matches!(menu.source, Some(MenuSource::ControlCenter))
            })
        })
    })
}

pub fn open_window_tiling_menu(target_hwnd: isize, maximized: bool, anchor_x: i32, anchor_y: i32) {
    let rows = build_window_tiling_menu_rows(maximized);
    let source = MenuSource::WindowControls { target_hwnd };
    with_menu(|menu| {
        menu.open(
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
    _hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_TIMER if wparam.0 == MENU_POLL_TIMER => {
            with_menu(PopupMenu::on_poll_timer);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_menu(|menu| menu.on_mouse_move(x, y));
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            with_menu(|menu| menu.dragging_row = None);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN if slider_pressed(lparam) => Some(LRESULT(0)),
        WM_LBUTTONDOWN | WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            let mut selected = None;
            with_menu(|menu| {
                selected = menu.on_click(x, y);
            });
            if let Some((source, cmd)) = selected {
                dispatch_selection(source, cmd);
            }
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_menu(PopupMenu::rebuild);
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

fn slider_pressed(lparam: LPARAM) -> bool {
    let (x, y) = unpack_point(lparam);
    let mut handled = false;
    with_menu(|menu| handled = menu.begin_slider_drag(x, y));
    handled
}

fn apply_slider(port: &dyn ControlCenterPort, id: usize, value: u8) {
    match id {
        CMD_CC_VOLUME => {
            let _ = port.set_volume(value);
        }
        CMD_CC_BRIGHTNESS => {
            let _ = port.set_brightness(value);
        }
        _ => {}
    }
}

fn with_menu(action: impl FnOnce(&mut PopupMenu)) {
    MENU.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut()
            && let Some(menu) = guard.as_mut()
        {
            action(menu);
        }
    });
}

pub fn repaint() {
    with_menu(|menu| menu.present());
}

fn dispatch_selection(source: MenuSource, cmd: usize) {
    match source {
        MenuSource::TopBar {
            bar_hwnd,
            app_name,
            target_hwnd,
            ..
        } => {
            if cmd == CMD_CC_THEME_TOGGLE {
                crate::theme_state::cycle_theme_mode();
                return;
            }
            if execute_top_bar_command(cmd, &app_name, target_hwnd) {
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
        MenuSource::Dock { entry, target_rect } => {
            crate::dock::execute_dock_menu_command(cmd, &entry, target_rect);
        }
        MenuSource::ControlCenter => with_menu(|menu| menu.execute_control_center_command(cmd)),
        MenuSource::Calendar {
            year,
            month,
            anchor_right,
            anchor_y,
        } => match cmd {
            CMD_CAL_PREVIOUS | CMD_CAL_NEXT => {
                let delta = if cmd == CMD_CAL_NEXT { 1 } else { -1 };
                let (next_year, next_month) = shift_month(year, month, delta);
                open_calendar_month(next_year, next_month, anchor_right, anchor_y);
            }
            CMD_CAL_TODAY => open_calendar(anchor_right, anchor_y),
            _ => with_menu(|menu| menu.execute_control_center_command(cmd)),
        },
        MenuSource::WindowControls { target_hwnd } => {
            if cmd == CMD_WIN_MINIMIZE {
                let _ = crate::genie::minimize_window(target_hwnd);
            } else {
                let _ = execute_top_bar_command(cmd, "", target_hwnd);
            }
        }
    }
}

impl PopupMenu {
    fn open(&mut self, anchor: MenuAnchor, rows: Vec<MenuRow>, source: MenuSource) {
        if rows.is_empty() {
            self.hide();
            return;
        }
        self.screen = primary_screen();
        self.layout = compute_popup_menu_layout(
            &self.metrics,
            self.screen.width,
            self.screen.height,
            self.screen.dpi,
            anchor,
            &rows,
        );
        self.rows = rows;
        self.hovered_row = None;
        self.dragging_row = None;
        self.source = Some(source);
        self.opened_at = Instant::now();
        self.was_mouse_down = true;
        self.visible = true;

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
        self.present();
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                MENU_POLL_TIMER,
                MENU_POLL_MS,
                None,
            );
        }
    }

    fn hide(&mut self) {
        if self.visible {
            self.visible = false;
            self.hovered_row = None;
            self.source = None;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), MENU_POLL_TIMER);
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            }
        }
    }

    fn rebuild(&mut self) {
        self.hide();
        self.screen = primary_screen();
        if let Ok(renderer) = PopupMenuRenderer::new(self.screen.dpi) {
            self.renderer = renderer;
        }
    }

    fn on_poll_timer(&mut self) {
        if !self.visible {
            return;
        }
        let (gx, gy) = cursor_position();
        let local_x = gx - self.layout.window.x;
        let local_y = gy - self.layout.window.y;
        let next_hover = if self.layout.window.contains(gx, gy) {
            self.layout.hit_test(local_x, local_y)
        } else {
            None
        };
        if next_hover != self.hovered_row {
            self.hovered_row = next_hover;
            self.present();
        }

        let (mouse_down, esc_down) = poll_menu_dismiss_keys();
        if esc_down {
            self.hide();
            return;
        }
        if mouse_down
            && !self.was_mouse_down
            && self.opened_at.elapsed() >= Duration::from_millis(60)
            && !self.layout.window.contains(gx, gy)
        {
            self.hide();
            return;
        }
        self.was_mouse_down = mouse_down;
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        if !self.visible {
            return;
        }
        if let Some(row) = self.dragging_row {
            self.update_slider(row, x);
            return;
        }
        let next = self.layout.hit_test(x, y);
        if next != self.hovered_row {
            self.hovered_row = next;
            self.present();
        }
    }

    fn begin_slider_drag(&mut self, x: i32, y: i32) -> bool {
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
        self.update_slider(row, x);
        true
    }

    fn update_slider(&mut self, row: usize, x: i32) {
        let Some(value) = self.layout.slider_value_at(row, x) else {
            return;
        };
        let Some(MenuRow::Slider {
            id, value: current, ..
        }) = self.rows.get_mut(row)
        else {
            return;
        };
        if *current == value {
            return;
        }
        *current = value;
        apply_slider(self.control_center.as_ref(), *id, value);
        self.present();
    }

    fn execute_control_center_command(&self, cmd: usize) {
        match cmd {
            CMD_CC_THEME_TOGGLE => {
                crate::theme_state::cycle_theme_mode();
                return;
            }
            CMD_CC_THEME_LIGHT => {
                crate::theme_state::set_theme_mode(otto_core::ThemeMode::Light);
                return;
            }
            CMD_CC_THEME_DARK => {
                crate::theme_state::set_theme_mode(otto_core::ThemeMode::Dark);
                return;
            }
            CMD_CC_THEME_AUTO => {
                crate::theme_state::set_theme_mode(otto_core::ThemeMode::Auto);
                return;
            }
            _ => {}
        }
        let action = match cmd {
            CMD_CC_MEDIA_TOGGLE => ControlCenterAction::Media(MediaCommand::TogglePlayPause),
            CMD_CC_MEDIA_NEXT => ControlCenterAction::Media(MediaCommand::Next),
            CMD_CC_MEDIA_PREVIOUS => ControlCenterAction::Media(MediaCommand::Previous),
            CMD_CC_CALENDAR => ControlCenterAction::OpenCalendar,
            CMD_CAL_SETTINGS => ControlCenterAction::OpenCalendarSettings,
            CMD_CC_WIFI => ControlCenterAction::OpenWifi,
            CMD_CC_BLUETOOTH => ControlCenterAction::OpenBluetooth,
            CMD_CC_FOCUS => ControlCenterAction::OpenFocus,
            CMD_CC_DISPLAY => ControlCenterAction::OpenDisplay,
            CMD_CC_SOUND => ControlCenterAction::OpenSound,
            _ => return,
        };
        self.control_center.execute(action);
    }

    fn on_click(&mut self, x: i32, y: i32) -> Option<(MenuSource, usize)> {
        if !self.visible {
            return None;
        }
        let row_idx = self.layout.hit_test(x, y)?;
        self.layout
            .slider_value_at(row_idx, x)
            .map_or(Some(()), |_| None)?;
        let cmd = self.layout.command_at(row_idx)?;
        let source = self.source.take()?;
        self.hide();
        Some((source, cmd))
    }

    fn present(&self) {
        if self.visible {
            let theme = crate::theme_state::resolved_theme();
            let _ =
                self.renderer
                    .present(self.hwnd, &self.layout, &self.rows, self.hovered_row, theme);
        }
    }
}
