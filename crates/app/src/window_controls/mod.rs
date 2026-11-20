use std::time::Instant;

use crate::host::RuntimeContext;
use crate::win_msg::{
    GREEN_HOVER_TIMER, WINDOW_CONTROLS_ANIM_TIMER, WM_APP_NATIVE_MINIMIZE, WM_APP_WINDOW_SYNC,
    WM_MOUSELEAVE,
};
use otto_core::{
    Appearance, Capability, Metrics, MotionMetrics, MotionPreference, NativeCaption, NotchStyle,
    WindowCommand, WindowControlAction, WindowControlsLayout, WindowControlsMetrics,
    WindowControlsState, WindowInfo, compute_window_controls_layout,
};
use otto_platform::{
    ActiveWindowTarget, acknowledge_native_minimize, acknowledge_window_sync, current_process_id,
    drag_threshold_exceeded, inspect_window, install_native_minimize_hook,
    install_window_tracking_hook, is_name_change_sync, minimize_window_native, motion_preference,
    place_overlay_above, poll_foreground_change, primary_screen, restore_rounded_corners,
    round_window_corners, uninstall_native_minimize_hook, uninstall_window_tracking_hook,
    visible_window_targets,
};
use otto_render::WindowControlsRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GWL_EXSTYLE, GetWindowLongW, HTCLIENT, MA_NOACTIVATE, SW_HIDE, ShowWindow,
    WINDOW_EX_STYLE, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_SETCURSOR, WM_TIMER,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};

mod actions;
mod animation;
mod geometry;
mod pointer;
mod presentation;

const WINDOW_CONTROLS_ANIM_MS: u32 = 16;
const GREEN_HOVER_MS: u32 = 600;
const MAX_OVERLAYS: usize = 24;
pub(crate) const OVERLAY_CLASS: PCWSTR = w!("OttoWindowControls");
const DOUBLE_CLICK_SLOP_DIP: i32 = 4;
const FALLBACK_DARK_FILL: [u8; 3] = [0x20, 0x20, 0x20];
const FALLBACK_LIGHT_FILL: [u8; 3] = [0xF3, 0xF3, 0xF3];

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoverState {
    caption: NativeCaption,
    rows: Vec<[u8; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Placement {
    Hidden,
    Cover(NativeCaption),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PressState {
    action: WindowControlAction,
    inside: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingCaptionDrag {
    target_hwnd: isize,
    start_client: (i32, i32),
    start_screen: (i32, i32),
}

fn step_press_move(
    press: &mut PressState,
    layout: &WindowControlsLayout,
    x: i32,
    y: i32,
) -> (bool, bool) {
    let now_inside = layout.hit_test(x, y) == Some(press.action);
    let group_hovered = layout.hits_group(x, y);
    let changed = press.inside != now_inside;
    press.inside = now_inside;
    (group_hovered, changed)
}

fn step_press_release(
    press: &mut Option<PressState>,
    layout: &WindowControlsLayout,
    x: i32,
    y: i32,
) -> (Option<WindowControlAction>, bool) {
    let group_hovered = layout.hits_group(x, y);
    let Some(active) = press.take() else {
        return (None, group_hovered);
    };
    let execute =
        (active.inside && layout.hit_test(x, y) == Some(active.action)).then_some(active.action);
    (execute, group_hovered)
}

fn should_start_caption_drag(
    pending: &mut Option<PendingCaptionDrag>,
    x: i32,
    y: i32,
    threshold: (i32, i32),
) -> Option<PendingCaptionDrag> {
    let drag = (*pending)?;
    if !drag_threshold_exceeded(drag.start_client, (x, y), threshold) {
        return None;
    }
    *pending = None;
    Some(drag)
}

fn is_alt_pressed() -> bool {
    (unsafe { GetAsyncKeyState(i32::from(VK_MENU.0)) } as u16 & 0x8000) != 0
}

#[allow(clippy::struct_excessive_bools)]
struct Overlay {
    hwnd: isize,
    own_pid: u32,
    appearance: Appearance,
    bar_height: i32,
    renderer: Option<WindowControlsRenderer>,
    metrics: WindowControlsMetrics,
    layout: WindowControlsLayout,
    target: Option<ActiveWindowTarget>,
    window: Option<WindowInfo>,
    state: WindowControlsState,
    cover: Option<CoverState>,
    render_origin: (f32, f32),
    settle_ticks: u8,
    green_hover_armed: bool,
    hovered: bool,
    alt_down: bool,
    press: Option<PressState>,
    pending_drag: Option<PendingCaptionDrag>,
    visible: bool,
    tracking_mouse: bool,
    animating: bool,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    last_tick: Instant,
    last_caption_click: Option<(Instant, (i32, i32))>,
}

pub(crate) struct WindowControls {
    primary: isize,
    own_pid: u32,
    appearance: Appearance,
    bar_height: i32,
    overlays: Vec<Overlay>,
}

fn plan_overlay_slots(held: &[Option<isize>], wanted: &[isize]) -> Vec<Option<isize>> {
    let mut slots: Vec<Option<isize>> = held
        .iter()
        .map(|current| current.filter(|window| wanted.contains(window)))
        .collect();
    for window in wanted {
        if slots.contains(&Some(*window)) {
            continue;
        }
        match slots.iter().position(Option::is_none) {
            Some(free) => slots[free] = Some(*window),
            None => slots.push(Some(*window)),
        }
    }
    slots
}

fn effective_bar_height(runtime: &RuntimeContext, style: NotchStyle) -> i32 {
    if runtime.top_bar_enabled.get() {
        Metrics::for_style(style).bar_height
    } else {
        0
    }
}

#[allow(clippy::unnecessary_wraps)]
pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let own_pid = current_process_id();
    let appearance = crate::theme_state::appearance(runtime);
    let bar_height = effective_bar_height(runtime, runtime.notch_style());
    let overlay = Overlay::new(hwnd, own_pid, appearance, bar_height);
    let mut controls = WindowControls {
        primary: hwnd,
        own_pid,
        appearance,
        bar_height,
        overlays: vec![overlay],
    };
    install_window_tracking_hook(hwnd, WM_APP_WINDOW_SYNC);
    let _ = install_native_minimize_hook(hwnd, WM_APP_NATIVE_MINIMIZE);
    controls.sync();
    *runtime.window_controls.borrow_mut() = Some(controls);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    uninstall_native_minimize_hook();
    uninstall_window_tracking_hook();
    let _ = restore_rounded_corners();
    if let Ok(mut guard) = runtime.window_controls.try_borrow_mut()
        && let Some(controls) = guard.take()
    {
        for overlay in &controls.overlays {
            overlay.kill_timers();
            unsafe {
                let _ = ShowWindow(HWND(overlay.hwnd as *mut _), SW_HIDE);
            }
            if overlay.hwnd != controls.primary {
                runtime.queue_destroy_hwnd(overlay.hwnd);
            }
        }
    }
}

pub fn handle_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_CLOSE => Some(LRESULT(0)),
        WM_SETCURSOR => {
            if (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
                with_overlay_result(runtime, hwnd, Overlay::on_set_cursor)
            } else {
                None
            }
        }
        WM_APP_WINDOW_SYNC => {
            acknowledge_window_sync();
            let settle_active = is_name_change_sync(lparam.0 as u32);
            with_window_controls(runtime, |controls| controls.sync_internal(settle_active));
            Some(LRESULT(0))
        }
        WM_APP_NATIVE_MINIMIZE => {
            minimize_from_native_button(runtime, wparam.0 as isize);
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == WINDOW_CONTROLS_ANIM_TIMER => {
            with_overlay(runtime, hwnd, Overlay::tick_animation);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_overlay(runtime, hwnd, |overlay| overlay.on_mouse_move(x, y));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == GREEN_HOVER_TIMER => {
            with_overlay(runtime, hwnd, |overlay| overlay.on_green_hover(runtime));
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_overlay(runtime, hwnd, Overlay::on_mouse_leave);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_overlay(runtime, hwnd, |overlay| {
                overlay.on_left_down(runtime, x, y);
            });
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_overlay(runtime, hwnd, |overlay| {
                overlay.on_left_up(runtime, x, y);
            });
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            with_overlay(runtime, hwnd, Overlay::on_capture_changed);
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_overlay(runtime, hwnd, |overlay| {
                overlay.on_right_click(runtime, x, y);
            });
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_window_controls(runtime, |controls| {
                if controls.primary == hwnd {
                    controls.rebuild();
                }
            });
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

pub fn apply_appearance(runtime: &RuntimeContext, appearance: Appearance) {
    with_window_controls(runtime, |controls| {
        controls.appearance = appearance;
        for overlay in &mut controls.overlays {
            overlay.appearance = appearance;
            if overlay.visible {
                overlay.settle_ticks = overlay.settle_ticks.max(20);
                overlay.ensure_anim_timer();
                overlay.present();
            }
        }
    });
}

pub fn set_notch_style(runtime: &RuntimeContext, style: NotchStyle) {
    let bar_height = effective_bar_height(runtime, style);
    with_window_controls(runtime, |controls| {
        if controls.bar_height == bar_height {
            return;
        }
        controls.bar_height = bar_height;
        for overlay in &mut controls.overlays {
            overlay.bar_height = bar_height;
        }
        controls.rebuild();
    });
}

pub fn sync_bar_height(runtime: &RuntimeContext) {
    set_notch_style(runtime, runtime.notch_style());
}

fn minimize_from_native_button(runtime: &RuntimeContext, window: isize) {
    acknowledge_native_minimize(window);
    let can_minimize = inspect_window(window, current_process_id())
        .is_some_and(|live| live.capability(WindowCommand::Minimize) == Capability::Available);
    if can_minimize {
        let _ = crate::genie::minimize_window(runtime, window);
    } else {
        minimize_window_native(window);
    }
}

pub fn poll_foreground(runtime: &RuntimeContext) {
    if poll_foreground_change() {
        with_window_controls(runtime, WindowControls::sync);
    }
}

fn with_window_controls(runtime: &RuntimeContext, action: impl FnOnce(&mut WindowControls)) {
    if let Ok(mut guard) = runtime.window_controls.try_borrow_mut()
        && let Some(controls) = guard.as_mut()
    {
        action(controls);
    }
}

fn with_overlay(runtime: &RuntimeContext, hwnd: isize, action: impl FnOnce(&mut Overlay)) {
    with_window_controls(runtime, |controls| {
        if let Some(overlay) = controls
            .overlays
            .iter_mut()
            .find(|overlay| overlay.hwnd == hwnd)
        {
            action(overlay);
        }
    });
}

fn with_overlay_result(
    runtime: &RuntimeContext,
    hwnd: isize,
    action: impl FnOnce(&Overlay) -> Option<LRESULT>,
) -> Option<LRESULT> {
    let Ok(guard) = runtime.window_controls.try_borrow() else {
        return None;
    };
    let controls = guard.as_ref()?;
    let overlay = controls
        .overlays
        .iter()
        .find(|overlay| overlay.hwnd == hwnd)?;
    action(overlay)
}

fn spawn_overlay_window(template: isize) -> Option<isize> {
    let ex_style = WINDOW_EX_STYLE(
        (unsafe { GetWindowLongW(HWND(template as *mut _), GWL_EXSTYLE) } as u32)
            & !WS_EX_TOPMOST.0,
    );
    let instance = unsafe { GetModuleHandleW(None) }.ok()?;
    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            OVERLAY_CLASS,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .ok()?;
    Some(hwnd.0 as isize)
}

impl WindowControls {
    fn spawn_overlay(&mut self) -> bool {
        if self.overlays.len() >= MAX_OVERLAYS {
            return false;
        }
        let Some(hwnd) = spawn_overlay_window(self.primary) else {
            return false;
        };
        self.overlays.push(Overlay::new(
            hwnd,
            self.own_pid,
            self.appearance,
            self.bar_height,
        ));
        true
    }

    fn sync(&mut self) {
        self.sync_internal(false);
    }

    fn sync_internal(&mut self, settle_active: bool) {
        let targets: Vec<ActiveWindowTarget> = visible_window_targets(self.own_pid)
            .into_iter()
            .filter(|target| {
                matches!(
                    Overlay::placement(*target, self.bar_height),
                    Placement::Cover(_)
                )
            })
            .take(MAX_OVERLAYS)
            .collect();
        for target in &targets {
            let _ = round_window_corners(target.hwnd);
        }
        let held: Vec<Option<isize>> = self
            .overlays
            .iter()
            .map(|overlay| overlay.target.map(|target| target.hwnd))
            .collect();
        let wanted: Vec<isize> = targets.iter().map(|target| target.hwnd).collect();
        let mut slots = plan_overlay_slots(&held, &wanted);
        while self.overlays.len() < slots.len() {
            if !self.spawn_overlay() {
                slots.truncate(self.overlays.len());
            }
        }
        for (overlay, slot) in self.overlays.iter_mut().zip(slots) {
            let target = slot.and_then(|hwnd| targets.iter().find(|target| target.hwnd == hwnd));
            overlay.sync_target(
                target.copied(),
                settle_active && target.is_some_and(|item| item.active),
            );
        }
        for target in &targets {
            if let Some(overlay) = self.overlays.iter_mut().find(|overlay| {
                overlay.visible && overlay.target.is_some_and(|item| item.hwnd == target.hwnd)
            }) && !place_overlay_above(overlay.hwnd, target.hwnd)
            {
                overlay.hide();
            }
        }
    }

    fn rebuild(&mut self) {
        for overlay in &mut self.overlays {
            overlay.hide();
        }
        self.sync();
    }
}

impl Overlay {
    fn new(hwnd: isize, own_pid: u32, appearance: Appearance, bar_height: i32) -> Self {
        let metrics = WindowControlsMetrics::default();
        let layout = compute_window_controls_layout(&metrics, primary_screen().dpi);
        Self {
            hwnd,
            own_pid,
            appearance,
            bar_height,
            renderer: None,
            metrics,
            layout,
            target: None,
            window: None,
            state: WindowControlsState::default(),
            cover: None,
            render_origin: (0.0, 0.0),
            settle_ticks: 0,
            green_hover_armed: false,
            hovered: false,
            alt_down: false,
            press: None,
            pending_drag: None,
            visible: false,
            tracking_mouse: false,
            animating: false,
            motion: motion_preference(),
            motion_metrics: MotionMetrics::default(),
            last_tick: Instant::now(),
            last_caption_click: None,
        }
    }
}

#[cfg(test)]
mod tests;
