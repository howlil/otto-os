use std::time::{Duration, Instant};

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::{
    CC_POLL_TIMER, WM_APP_CONTROL_CENTER_BACKDROP, WM_APP_CONTROL_CENTER_KEY, WM_GETOBJECT,
};
use otto_core::{
    BarKey, ControlCenterAction, ControlCenterCommand, ControlCenterFocus, ControlCenterHitTarget,
    ControlCenterLayout, ControlCenterMetrics, ControlCenterPage, ControlCenterPort,
    ControlCenterSnapshot, MotionMetrics, MotionPreference, build_control_center_accessible_tree,
    control_center_action, decode_control_center_target, scale,
};
use otto_platform::{
    DisplayMonitor, MenuNavEvent, WM_APP_CONTROL_CENTER_REFRESH, WM_APP_UIA_INVOKE,
    WindowsControlCenter, cursor_position, handle_wm_getobject, monitor_at,
};
use otto_render::ControlCenterRenderer;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MA_NOACTIVATE, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP, WM_TIMER,
};

mod commands;
mod input;
mod lifecycle;
mod presentation;

use self::commands::execute_command;
use self::commands::refresh_and_present;

const CC_POLL_MS: u32 = MotionMetrics::MACOS_LIKE.animation_tick_ms;
const OPEN_CLICK_GRACE: Duration = Duration::from_millis(60);
const WHEEL_SCROLL_STEP: i32 = 36;

pub(crate) struct ControlCenterSurface {
    hwnd: isize,
    port: Box<dyn ControlCenterPort>,
    renderer: Option<ControlCenterRenderer>,
    monitor: DisplayMonitor,
    metrics: ControlCenterMetrics,
    snapshot: ControlCenterSnapshot,
    layout: ControlCenterLayout,
    anchor_right: i32,
    anchor_y: i32,
    scroll_offset: i32,
    hovered: Option<ControlCenterHitTarget>,
    pressed: Option<ControlCenterHitTarget>,
    focus: ControlCenterFocus,
    drag: Option<DragMode>,
    visible: bool,
    opened_at: Instant,
    dismissed_outside_at: Option<Instant>,
    was_mouse_down: bool,
    last_cursor: (i32, i32),
    backdrop: Option<BackdropFeed>,
    motion: MotionPreference,
    morph_progress: Option<f32>,
    morph_tick_at: Instant,
}

#[derive(Clone, Copy)]
enum DragMode {
    Slider {
        target: ControlCenterHitTarget,
    },
    ResizeTile {
        tile_id: u32,
        start_pos: (i32, i32),
        moved: bool,
        preview_cols: u8,
        preview_rows: u8,
    },
    MoveTile {
        tile_id: u32,
        start_pos: (i32, i32),
        moved: bool,
        preview_col: u8,
        preview_row: u8,
    },
}

enum PostClickAction {
    None,
    Close,
    Command(ControlCenterCommand, bool),
}

impl ControlCenterSurface {
    fn new(hwnd: isize) -> Self {
        let snapshot = ControlCenterSnapshot {
            grid: otto_platform::load_control_center_grid(),
            ..ControlCenterSnapshot::default()
        };
        Self {
            hwnd,
            port: Box::new(WindowsControlCenter),
            renderer: None,
            monitor: monitor_at(0, 0),
            metrics: ControlCenterMetrics::DEFAULT,
            snapshot,
            layout: ControlCenterLayout::default(),
            anchor_right: 0,
            anchor_y: 0,
            scroll_offset: 0,
            hovered: None,
            pressed: None,
            focus: ControlCenterFocus::default(),
            drag: None,
            visible: false,
            opened_at: Instant::now(),
            dismissed_outside_at: None,
            was_mouse_down: false,
            last_cursor: (0, 0),
            backdrop: None,
            motion: MotionPreference::Full,
            morph_progress: None,
            morph_tick_at: Instant::now(),
        }
    }
}

pub(crate) fn start(runtime: &RuntimeContext, hwnd: isize) {
    if let Ok(mut slot) = runtime.control_center.try_borrow_mut() {
        *slot = Some(ControlCenterSurface::new(hwnd));
    }
}

pub(crate) fn stop(runtime: &RuntimeContext) {
    if let Ok(mut guard) = runtime.control_center.try_borrow_mut()
        && let Some(mut surface) = guard.take()
    {
        surface.hide();
    }
}

pub(crate) fn close(runtime: &RuntimeContext) {
    with_cc(runtime, ControlCenterSurface::hide);
}

#[must_use]
pub(crate) fn is_open(runtime: &RuntimeContext) -> bool {
    runtime
        .control_center
        .try_borrow()
        .ok()
        .and_then(|guard| guard.as_ref().map(|surface| surface.visible))
        .unwrap_or(false)
}

#[must_use]
pub(crate) fn active_page(runtime: &RuntimeContext) -> Option<ControlCenterPage> {
    runtime.control_center.try_borrow().ok().and_then(|guard| {
        guard
            .as_ref()
            .filter(|surface| surface.visible)
            .map(|surface| surface.snapshot.page)
    })
}

pub(crate) fn open_at_page(
    runtime: &RuntimeContext,
    page: ControlCenterPage,
    anchor_right: i32,
    anchor_y: i32,
    motion: MotionPreference,
) {
    crate::spotlight::close(runtime);
    crate::menu::close(runtime);
    with_cc(runtime, |surface| {
        surface.open(runtime, page, anchor_right, anchor_y, false, motion);
    });
}

pub(crate) fn open_at_page_focused(
    runtime: &RuntimeContext,
    page: ControlCenterPage,
    anchor_right: i32,
    anchor_y: i32,
    motion: MotionPreference,
) {
    crate::spotlight::close(runtime);
    crate::menu::close(runtime);
    with_cc(runtime, |surface| {
        surface.open(runtime, page, anchor_right, anchor_y, true, motion);
    });
}

fn resolve_anchor(
    runtime: &RuntimeContext,
    trigger: otto_core::HitTarget,
    from_tray: bool,
) -> (i32, i32, MotionPreference) {
    if !from_tray
        && runtime.top_bar_enabled.get()
        && let Some(anchor) = crate::bar::control_center_anchor(runtime, trigger)
    {
        return anchor;
    }
    let (cx, cy) = cursor_position();
    let monitor = monitor_at(cx, cy);
    let motion = otto_platform::motion_preference();
    let anchor_right = monitor.bounds.right() - scale(8, monitor.dpi);
    let anchor_y = if from_tray {
        monitor.bounds.bottom() - 1
    } else {
        monitor.bounds.y + scale(12, monitor.dpi)
    };
    (anchor_right, anchor_y, motion)
}

pub(crate) fn toggle_from_host(
    runtime: &RuntimeContext,
    page: ControlCenterPage,
    focused: bool,
    from_tray: bool,
) {
    let _ = otto_platform::close_tray_overflow();
    if is_open(runtime) && active_page(runtime) == Some(page) {
        close(runtime);
        return;
    }
    if from_tray {
        let mut suppressed = false;
        with_cc(runtime, |surface| {
            if surface
                .dismissed_outside_at
                .take()
                .is_some_and(|at| at.elapsed() < Duration::from_millis(250))
            {
                suppressed = true;
            }
        });
        if suppressed {
            return;
        }
    }
    let (anchor_right, anchor_y, motion) =
        resolve_anchor(runtime, otto_core::HitTarget::ControlCenter, from_tray);
    if focused {
        open_at_page_focused(runtime, page, anchor_right, anchor_y, motion);
    } else {
        open_at_page(runtime, page, anchor_right, anchor_y, motion);
    }
}

pub(crate) fn open_settings_action(action: ControlCenterAction) {
    WindowsControlCenter.execute(action);
}

pub(crate) fn repaint(runtime: &RuntimeContext) {
    with_cc(runtime, |surface| {
        if surface.visible {
            surface.sync_snapshot(runtime);
            surface.relayout();
            surface.place();
            surface.sync_backdrop(runtime, true);
            surface.present(runtime);
        }
    });
}

pub(crate) fn set_glass_enabled(runtime: &RuntimeContext, enabled: bool) {
    with_cc(runtime, |surface| {
        if !enabled {
            surface.backdrop = None;
            if let Some(renderer) = surface.renderer.as_ref() {
                renderer.clear_backdrop();
            }
        }
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub(crate) fn set_capture_visible(runtime: &RuntimeContext, visible: bool) {
    with_cc(runtime, |surface| {
        if let Some(feed) = surface.backdrop.as_mut() {
            feed.set_capture_visible(visible);
        }
        surface.sync_backdrop(runtime, true);
        surface.present(runtime);
    });
}

pub(crate) fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_GETOBJECT => runtime
            .control_center
            .try_borrow()
            .ok()
            .and_then(|guard| {
                let surface = guard.as_ref().filter(|surface| surface.visible)?;
                handle_wm_getobject(hwnd, wparam.0, lparam.0, WM_APP_UIA_INVOKE, || {
                    build_control_center_accessible_tree(&surface.layout, &surface.snapshot)
                })
            })
            .map(LRESULT),
        WM_APP_UIA_INVOKE => {
            if let Some(target) = i32::try_from(wparam.0)
                .ok()
                .and_then(decode_control_center_target)
            {
                let mut post = PostClickAction::None;
                with_cc(runtime, |surface| {
                    if surface.visible
                        && let Some(outcome) = control_center_action(&surface.snapshot, target)
                    {
                        post = PostClickAction::Command(outcome.command, outcome.dismiss);
                    }
                });
                apply_post_action(runtime, post);
            }
            Some(LRESULT(0))
        }
        WM_APP_CONTROL_CENTER_KEY => {
            if let Some(key) = BarKey::from_code(wparam.0) {
                let mut post = PostClickAction::None;
                with_cc(runtime, |surface| post = surface.on_key(runtime, key));
                apply_post_action(runtime, post);
            }
            Some(LRESULT(0))
        }
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_TIMER if wparam.0 == CC_POLL_TIMER => {
            let mut post = PostClickAction::None;
            with_cc(runtime, |surface| post = surface.on_poll_timer(runtime));
            apply_post_action(runtime, post);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_cc(runtime, |surface| surface.on_mouse_move(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            with_cc(runtime, |surface| surface.on_wheel(runtime, delta));
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            let mut post = PostClickAction::None;
            with_cc(runtime, |surface| {
                post = surface.release_press(runtime, x, y);
            });
            apply_post_action(runtime, post);
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_cc(runtime, |surface| {
                surface.drag = None;
                if surface.pressed.take().is_some() {
                    surface.present(runtime);
                }
            });
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_cc(runtime, |surface| surface.begin_press(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            with_cc(runtime, |surface| surface.toggle_edit_mode(runtime));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_cc(runtime, |surface| surface.rebuild(runtime));
            Some(LRESULT(0))
        }
        WM_APP_CONTROL_CENTER_REFRESH => {
            repaint(runtime);
            Some(LRESULT(0))
        }
        WM_APP_CONTROL_CENTER_BACKDROP => Some(LRESULT(0)),
        WM_CLOSE => {
            with_cc(runtime, ControlCenterSurface::hide);
            Some(LRESULT(0))
        }
        _ => None,
    }
}

fn apply_post_action(runtime: &RuntimeContext, action: PostClickAction) {
    match action {
        PostClickAction::None => {}
        PostClickAction::Close => close(runtime),
        PostClickAction::Command(command, dismiss) => {
            let skip_refresh = matches!(
                command,
                ControlCenterCommand::OpenPage(_) | ControlCenterCommand::Back
            );
            if dismiss {
                close(runtime);
                execute_command(runtime, command);
            } else {
                execute_command(runtime, command);
                if !skip_refresh {
                    refresh_and_present(runtime);
                }
            }
        }
    }
}

fn with_cc(runtime: &RuntimeContext, action: impl FnOnce(&mut ControlCenterSurface)) {
    if let Ok(mut guard) = runtime.control_center.try_borrow_mut()
        && let Some(surface) = guard.as_mut()
    {
        action(surface);
    }
}

fn poll_dismiss_action(
    nav_event: Option<MenuNavEvent>,
    mouse_down: bool,
    was_mouse_down: &mut bool,
    grace_elapsed: bool,
    inside_window: bool,
) -> bool {
    let pressed_now = mouse_down && !*was_mouse_down;
    *was_mouse_down = mouse_down;
    if matches!(nav_event, Some(MenuNavEvent::Escape)) {
        return true;
    }
    grace_elapsed && pressed_now && !inside_window
}

const fn unpack_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

#[cfg(test)]
mod tests;
