use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::{MENU_POLL_TIMER, WM_APP_MENU_BACKDROP};
use otto_core::{
    BACKDROP_PADDING_PT, CMD_DOWNLOADS_ITEM_BASE, CMD_DOWNLOADS_OPEN_FOLDER, CalendarControl,
    CaptureRegion, ControlCenterAction, ControlCenterPort, DownloadsStackEntry, DownloadsStackHit,
    DownloadsStackLayout, EstimatedMenuText, HitTarget, MenuAnchor, MenuRow, MenuTextMeasure,
    NotificationAccess, NotificationCenterHit, NotificationCenterLayout, NotificationGroup,
    PopupMenuLayout, PopupMenuMetrics, PopupMenuPlacement, Rect, WindowInfo, capture_interval_for,
    capture_wanted, compute_downloads_stack_layout, compute_notification_center_layout,
    compute_popup_menu_layout, downloads_stack_hit_test, group_notifications,
    next_downloads_stack_focus, scale,
};
use otto_core::{DockEntry, StackItem};
use otto_platform::{
    MenuNavEvent, PrimaryScreen, WindowsControlCenter, cursor_position, focus_window_hwnd,
    is_running_on_battery, monitor_work_area_at, poll_menu_events, primary_screen,
    read_downloads_stack_thumbnails,
};
use otto_render::{DownloadsStackRenderer, MenuHover, PopupMenuRenderer};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer,
    SetWindowPos, ShowWindow, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

mod dispatch;
mod open;

use self::dispatch::dispatch_selection;
pub use self::open::is_calendar_open;
pub use self::open::is_notification_open;
pub use self::open::open_calendar;
pub use self::open::open_dock_menu;
pub use self::open::open_dock_overflow_menu;
pub use self::open::open_dock_settings_menu;
pub use self::open::open_downloads_stack_menu;
pub use self::open::open_notifications;
pub use self::open::open_top_bar_menu;
pub use self::open::open_tray_menu;
pub use self::open::open_window_tiling_menu;

const MENU_POLL_MS: u32 = 16;
const OPEN_CLICK_GRACE: Duration = Duration::from_millis(60);
const WHEEL_ROWS_PER_NOTCH: i32 = 2;

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

pub(super) struct NotificationCenterView {
    anchor_right: i32,
    anchor_y: i32,
    access: NotificationAccess,
    groups: Vec<NotificationGroup>,
    expanded_apps: BTreeSet<String>,
    layout: NotificationCenterLayout,
    active_hit: NotificationCenterHit,
    keyboard_focus: bool,
    scroll_offset: i32,
}

enum PopupAction {
    Menu(Box<MenuSource>, usize),
    NotificationCenter(NotificationCenterHit),
}

pub(crate) struct PopupMenu {
    hwnd: isize,
    renderer: Option<PopupMenuRenderer>,
    stack_renderer: Option<DownloadsStackRenderer>,
    stack_view: Option<DownloadsStackView>,
    notif_view: Option<NotificationCenterView>,
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
        notif_view: None,
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
    let mut slot = runtime
        .menu
        .try_borrow_mut()
        .map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    *slot = Some(menu);
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
pub fn is_open(runtime: &RuntimeContext) -> bool {
    runtime
        .menu
        .try_borrow()
        .ok()
        .and_then(|guard| guard.as_ref().map(|menu| menu.visible))
        .unwrap_or(false)
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
            if let Some(action) = selected {
                dispatch_action(runtime, action);
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
            if let Some(action) = selected {
                dispatch_action(runtime, action);
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
        self.notif_view = None;
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
        self.notif_view = None;

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
        if !self.visible {
            return;
        }
        let notches = i32::from(delta) / 120;
        let step = if notches == 0 {
            -i32::from(delta.signum())
        } else {
            -notches
        };
        if let Some(notif) = self.notif_view.as_mut() {
            if !notif.layout.is_scrollable() {
                return;
            }
            let scroll_step = scale(36, self.screen.dpi);
            let offset = notif.layout.scrolled_by(step * scroll_step);
            if offset != notif.scroll_offset {
                notif.scroll_offset = offset;
                let bounds = monitor_work_area_at(notif.anchor_right - 1, notif.anchor_y);
                notif.layout = compute_notification_center_layout(
                    notif.access,
                    &notif.groups,
                    bounds,
                    self.screen.dpi,
                    notif.anchor_right,
                    notif.anchor_y,
                    notif.scroll_offset,
                );
                notif.scroll_offset = notif.layout.scroll_offset;
                self.present(runtime);
            }
            return;
        }
        if !self.layout.is_scrollable() {
            return;
        }
        let row_height = scale(self.metrics.row_height, self.screen.dpi);
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
        self.notif_view = None;
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

    fn open_notification_center(
        &mut self,
        runtime: &RuntimeContext,
        anchor_right: i32,
        anchor_y: i32,
        expanded_apps: BTreeSet<String>,
        scroll_offset: i32,
    ) {
        crate::spotlight::close(runtime);
        crate::control_center::close(runtime);
        self.screen = primary_screen();
        self.rows.clear();
        self.hovered_row = None;
        self.hovered_calendar = None;
        self.source = None;
        self.stack_view = None;
        self.stack_renderer = None;
        let _ = self.ensure_renderer();

        let snapshot = self.control_center.read_notifications();
        let groups = group_notifications(&snapshot.items, &expanded_apps);
        let bounds = monitor_work_area_at(anchor_right - 1, anchor_y);
        let layout = compute_notification_center_layout(
            snapshot.access,
            &groups,
            bounds,
            self.screen.dpi,
            anchor_right,
            anchor_y,
            scroll_offset,
        );
        self.layout.window = layout.window;
        self.notif_view = Some(NotificationCenterView {
            anchor_right,
            anchor_y,
            access: snapshot.access,
            groups,
            expanded_apps,
            scroll_offset: layout.scroll_offset,
            layout,
            active_hit: NotificationCenterHit::None,
            keyboard_focus: false,
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

    fn relayout_notification_center(&mut self, runtime: &RuntimeContext) {
        let Some(notif) = self.notif_view.as_mut() else {
            return;
        };
        for group in &mut notif.groups {
            group.expanded = notif.expanded_apps.contains(&group.app_key);
        }
        let bounds = monitor_work_area_at(notif.anchor_right - 1, notif.anchor_y);
        notif.layout = compute_notification_center_layout(
            notif.access,
            &notif.groups,
            bounds,
            self.screen.dpi,
            notif.anchor_right,
            notif.anchor_y,
            notif.scroll_offset,
        );
        notif.scroll_offset = notif.layout.scroll_offset;
        let size_changed = self.layout.window != notif.layout.window;
        self.layout.window = notif.layout.window;
        if size_changed {
            self.place();
            self.sync_backdrop(runtime, true);
        }
        self.present(runtime);
    }

    fn step_notif_focus(&mut self, runtime: &RuntimeContext, forward: bool) {
        let Some(notif) = self.notif_view.as_mut() else {
            return;
        };
        let next = if forward {
            notif.layout.next_hit(notif.active_hit)
        } else {
            notif.layout.prev_hit(notif.active_hit)
        };
        notif.active_hit = next;
        notif.keyboard_focus = true;
        let scroll = notif.layout.scroll_revealing(next);
        if scroll != notif.scroll_offset {
            notif.scroll_offset = scroll;
            let bounds = monitor_work_area_at(notif.anchor_right - 1, notif.anchor_y);
            notif.layout = compute_notification_center_layout(
                notif.access,
                &notif.groups,
                bounds,
                self.screen.dpi,
                notif.anchor_right,
                notif.anchor_y,
                notif.scroll_offset,
            );
        }
        self.present(runtime);
    }

    fn on_notif_poll_timer(&mut self, runtime: &RuntimeContext) -> Option<PopupAction> {
        let cursor = cursor_position();
        let (gx, gy) = cursor;
        let (inside, pointed_hit, changed) = {
            let notif = self.notif_view.as_mut()?;
            let inside = notif.layout.window.contains(gx, gy);
            let mut changed = false;
            if cursor != self.last_cursor {
                self.last_cursor = cursor;
                let hit = if inside {
                    notif
                        .layout
                        .hit_test(gx - notif.layout.window.x, gy - notif.layout.window.y)
                } else {
                    NotificationCenterHit::None
                };
                if hit != notif.active_hit || notif.keyboard_focus {
                    notif.active_hit = hit;
                    notif.keyboard_focus = false;
                    changed = true;
                }
            }
            (inside, notif.active_hit, changed)
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
                if pointed_hit != NotificationCenterHit::None {
                    return Some(PopupAction::NotificationCenter(pointed_hit));
                }
            }
            Some(MenuNavEvent::Up | MenuNavEvent::BackTab) => {
                self.step_notif_focus(runtime, false);
            }
            Some(MenuNavEvent::Down | MenuNavEvent::Tab) => {
                self.step_notif_focus(runtime, true);
            }
            _ => {}
        }

        let released = !mouse_down
            && self.was_mouse_down
            && self.opened_at.elapsed() >= OPEN_CLICK_GRACE
            && inside;
        if released {
            self.was_mouse_down = false;
            let hit = self
                .notif_view
                .as_ref()
                .map_or(NotificationCenterHit::None, |n| {
                    n.layout
                        .hit_test(gx - n.layout.window.x, gy - n.layout.window.y)
                });
            if hit != NotificationCenterHit::None {
                return Some(PopupAction::NotificationCenter(hit));
            }
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

    fn on_poll_timer(&mut self, runtime: &RuntimeContext) -> Option<PopupAction> {
        if !self.visible {
            return None;
        }
        if self.notif_view.is_some() {
            return self.on_notif_poll_timer(runtime);
        }
        if self.stack_view.is_some() {
            return self
                .on_stack_poll_timer(runtime)
                .map(|(source, cmd)| PopupAction::Menu(Box::new(source), cmd));
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
                    return Some(PopupAction::Menu(Box::new(selection.0), selection.1));
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
                    return Some(PopupAction::Menu(Box::new(selection.0), selection.1));
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
                return Some(PopupAction::Menu(Box::new(selection.0), selection.1));
            }
        }
        if released
            && let Some(row) = self.hovered_row
            && self.layout.hit_test(local_x, local_y) == Some(row)
        {
            self.was_mouse_down = false;
            if let Some(selection) = self.take_selection(row) {
                return Some(PopupAction::Menu(Box::new(selection.0), selection.1));
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
        if let Some(notif) = self.notif_view.as_mut() {
            let next = notif.layout.hit_test(x, y);
            if next != notif.active_hit || notif.keyboard_focus {
                notif.active_hit = next;
                notif.keyboard_focus = false;
                self.present(runtime);
            }
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

    fn on_click(&mut self, x: i32, y: i32) -> Option<PopupAction> {
        if !self.visible {
            return None;
        }
        if let Some(notif) = self.notif_view.as_ref() {
            let hit = notif.layout.hit_test(x, y);
            return (hit != NotificationCenterHit::None)
                .then_some(PopupAction::NotificationCenter(hit));
        }
        if let Some(stack) = self.stack_view.as_ref() {
            let hit = downloads_stack_hit_test(&stack.layout, x, y);
            return self
                .take_stack_selection(hit)
                .map(|(source, cmd)| PopupAction::Menu(Box::new(source), cmd));
        }
        let row = self.layout.hit_test(x, y)?;
        self.take_selection(row)
            .map(|(source, cmd)| PopupAction::Menu(Box::new(source), cmd))
    }

    fn prime_backdrop(&self, runtime: &RuntimeContext) {
        let appearance = crate::theme_state::appearance(runtime);
        if !capture_wanted(&appearance, runtime.glass_enabled.get())
            || (runtime.capture_visible.get() && otto_platform::is_surface_visible(self.hwnd))
        {
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
        if let Some(notif) = self.notif_view.as_ref() {
            crate::host::log_present_error(
                "notification_center",
                renderer.present_notification_center(
                    self.hwnd,
                    &notif.layout,
                    notif.active_hit,
                    notif.keyboard_focus,
                    appearance,
                ),
            );
            return;
        }
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

fn dispatch_action(runtime: &RuntimeContext, action: PopupAction) {
    match action {
        PopupAction::Menu(source, cmd) => dispatch_selection(runtime, *source, cmd),
        PopupAction::NotificationCenter(hit) => dispatch_notification_hit(runtime, hit),
    }
}

fn refresh_notification_center(runtime: &RuntimeContext, reset_view: bool) {
    with_menu(runtime, |menu| {
        if let Some(notif) = menu.notif_view.as_ref() {
            let (anchor_right, anchor_y) = (notif.anchor_right, notif.anchor_y);
            let (expanded, scroll) = if reset_view {
                (BTreeSet::new(), 0)
            } else {
                (notif.expanded_apps.clone(), notif.scroll_offset)
            };
            menu.open_notification_center(runtime, anchor_right, anchor_y, expanded, scroll);
        }
    });
}

fn toggle_notification_group(runtime: &RuntimeContext, group_index: usize, expand: bool) {
    with_menu(runtime, |menu| {
        if let Some(notif) = menu.notif_view.as_mut()
            && let Some(group) = notif.groups.get(group_index)
        {
            if expand {
                notif.expanded_apps.insert(group.app_key.clone());
            } else {
                notif.expanded_apps.remove(&group.app_key);
            }
        }
        menu.relayout_notification_center(runtime);
    });
}

fn dispatch_notification_hit(runtime: &RuntimeContext, hit: NotificationCenterHit) {
    match hit {
        NotificationCenterHit::None => {}
        NotificationCenterHit::ExpandGroup(idx) => toggle_notification_group(runtime, idx, true),
        NotificationCenterHit::CollapseGroup(idx) => toggle_notification_group(runtime, idx, false),
        NotificationCenterHit::OpenCard(id) => {
            with_menu(runtime, |menu| {
                menu.hide();
                let _ = menu.control_center.dismiss_notification(id);
            });
        }
        NotificationCenterHit::CloseCard(id) => {
            with_menu(runtime, |menu| {
                let _ = menu.control_center.remove_notification(id);
            });
            refresh_notification_center(runtime, false);
        }
        NotificationCenterHit::ClearGroup(group_index) => {
            with_menu(runtime, |menu| {
                let Some(notif) = menu.notif_view.as_mut() else {
                    return;
                };
                let Some(group) = notif.groups.get(group_index) else {
                    return;
                };
                notif.expanded_apps.remove(&group.app_key);
                let ids: Vec<u32> = group.items.iter().map(|item| item.id).collect();
                for id in ids {
                    let _ = menu.control_center.remove_notification(id);
                }
            });
            refresh_notification_center(runtime, false);
        }
        NotificationCenterHit::ClearAll => {
            with_menu(runtime, |menu| {
                let _ = menu.control_center.clear_notifications();
            });
            refresh_notification_center(runtime, true);
        }
        NotificationCenterHit::AccessAction => {
            with_menu(runtime, |menu| {
                let _ = menu.control_center.request_notification_access();
            });
            refresh_notification_center(runtime, false);
        }
        NotificationCenterHit::OpenWindowsNotifications => {
            with_menu(runtime, |menu| {
                menu.hide();
                menu.control_center
                    .execute(ControlCenterAction::OpenCalendar);
            });
        }
    }
}
