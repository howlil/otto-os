use std::mem::size_of;
use std::time::{Duration, Instant};

use crate::host::RuntimeContext;
use otto_core::{
    Appearance, COVER_FEATHER_DIP, Capability, CommandOutcome, CommandScope, CoverStyle, Metrics,
    MotionMetrics, MotionPreference, NativeCaption, NotchStyle, ResizeEdge, ResolvedTheme,
    WindowCommand, WindowControlAction, WindowControlsLayout, WindowControlsMetrics,
    WindowControlsState, WindowIdentity, WindowInfo, accepts_window_controls, animate_value,
    application_command_targets, compute_cover_layout, compute_window_controls_layout,
    cover_is_dark, scale, window_control_scope, window_controls_visible_under_bar,
};
use otto_platform::{
    ActiveWindowTarget, acknowledge_native_minimize, acknowledge_window_sync,
    activate_window_no_restore, application_of, current_process_id, cursor_position,
    double_click_ms, drag_threshold, drag_threshold_exceeded, execute_window_command,
    forward_caption_context_menu, forward_caption_drag, forward_resize_drag, inspect_window,
    install_native_minimize_hook, install_window_tracking_hook, is_name_change_sync,
    minimize_window_native, motion_preference, place_overlay_above, poll_foreground_change,
    primary_screen, release_capture_if_owned, restore_rounded_corners, round_window_corners,
    sample_caption_strip, signal_command_failure, uninstall_native_minimize_hook,
    uninstall_window_tracking_hook, visible_window_targets,
};
use otto_render::WindowControlsRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GWL_EXSTYLE, GetWindowLongW, HTCLIENT, IDC_SIZENESW, IDC_SIZENS, IDC_SIZEWE,
    KillTimer, LoadCursorW, MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SetCursor, SetTimer,
    ShowWindow, WINDOW_EX_STYLE, WM_CAPTURECHANGED, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_SETCURSOR,
    WM_TIMER, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, Result, w};

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_APP_WINDOW_SYNC: u32 = 0x8000 + 20;
const WM_APP_NATIVE_MINIMIZE: u32 = 0x8000 + 24;
const WINDOW_CONTROLS_ANIM_TIMER: usize = 20;
const WINDOW_CONTROLS_ANIM_MS: u32 = 16;
const GREEN_HOVER_TIMER: usize = 21;
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

    fn kill_timers(&self) {
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), WINDOW_CONTROLS_ANIM_TIMER);
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), GREEN_HOVER_TIMER);
        }
    }

    fn bar_height_for(target: ActiveWindowTarget, bar_height_pts: i32) -> Option<i32> {
        target.on_primary.then(|| scale(bar_height_pts, target.dpi))
    }

    fn placement(target: ActiveWindowTarget, bar_height_pts: i32) -> Placement {
        match target.caption {
            Some(caption) => {
                if window_controls_visible_under_bar(
                    caption.top,
                    caption.height,
                    Self::bar_height_for(target, bar_height_pts),
                ) {
                    Placement::Cover(caption)
                } else {
                    Placement::Hidden
                }
            }
            None => Placement::Hidden,
        }
    }

    fn target_origin(&self) -> (f32, f32) {
        self.cover.as_ref().map_or(self.render_origin, |cover| {
            let rect = cover.caption.cover_rect();
            (rect.x as f32, rect.y as f32)
        })
    }

    fn use_layout(&mut self, layout: WindowControlsLayout) {
        if layout != self.layout {
            self.renderer = None;
            self.layout = layout;
        }
    }

    fn sample_bounds(
        target: ActiveWindowTarget,
        caption: NativeCaption,
        bar_height_pts: i32,
    ) -> (i32, i32) {
        let min_top = Self::bar_height_for(target, bar_height_pts).unwrap_or(caption.top);
        (target.left, min_top)
    }

    fn sample_cover_rows(
        &self,
        target: ActiveWindowTarget,
        caption: NativeCaption,
        allow_live: bool,
    ) -> Vec<[u8; 3]> {
        let (frame_left, min_top) = Self::sample_bounds(target, caption, self.bar_height);
        if allow_live && let Some(rows) = sample_caption_strip(caption, frame_left, min_top) {
            return rows;
        }
        if let Some(existing) = self.cover.as_ref()
            && !existing.rows.is_empty()
        {
            return existing.rows.clone();
        }
        let fallback = match self.appearance.theme {
            ResolvedTheme::Dark => FALLBACK_DARK_FILL,
            ResolvedTheme::Light => FALLBACK_LIGHT_FILL,
        };
        vec![fallback; caption.height.max(1) as usize]
    }

    fn resample_cover_fill(&mut self) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        if !target.sample_unobstructed {
            return false;
        }
        let Some(caption) = self.cover.as_ref().map(|cover| cover.caption) else {
            return false;
        };
        let rows = self.sample_cover_rows(target, caption, true);
        if self.cover.as_ref().is_some_and(|cover| cover.rows == rows) {
            return false;
        }
        self.cover = Some(CoverState { caption, rows });
        true
    }

    fn cover_style(&self) -> Option<CoverStyle> {
        let dpi = self.target.map_or(96, |target| target.dpi);
        self.cover.as_ref().map(|cover| CoverStyle {
            rows: cover.rows.clone(),
            top_right_radius: cover.caption.corner_radius(),
            top_border_inset: cover.caption.top_border_inset,
            right_border_inset: cover.caption.rounded_top_right,
            feather_width: scale(COVER_FEATHER_DIP, dpi) as f32,
            is_dark: cover_is_dark(&cover.rows),
        })
    }

    fn refresh_window(&mut self, target: ActiveWindowTarget, reinspect: bool) -> bool {
        if reinspect || self.window.is_none() {
            self.window = inspect_window(target.hwnd, self.own_pid);
        }
        let accepted = self.window.as_ref().is_some_and(accepts_window_controls);
        let mut state = self
            .window
            .as_ref()
            .map_or_else(WindowControlsState::default, WindowControlsState::of);
        state.active = target.active;
        self.state = state;
        accepted
    }

    fn hide(&mut self) {
        self.stop_anim_timer();
        self.cancel_green_hover();
        self.press = None;
        self.pending_drag = None;
        let _ = release_capture_if_owned(self.hwnd);
        if self.visible {
            self.visible = false;
            self.hovered = false;
            self.alt_down = false;
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            }
        }
        self.target = None;
        self.window = None;
        self.cover = None;
        self.last_caption_click = None;
        self.renderer = None;
    }

    fn render_origin(&self) -> (i32, i32) {
        (
            self.render_origin.0.round() as i32,
            self.render_origin.1.round() as i32,
        )
    }

    fn sync_target(&mut self, target: Option<ActiveWindowTarget>, settle_active: bool) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_tick = now;
        self.motion = motion_preference();
        let Some(target) = target else {
            self.hide();
            return;
        };
        let placement = Self::placement(target, self.bar_height);
        let layout = match placement {
            Placement::Hidden => None,
            Placement::Cover(caption) => compute_cover_layout(caption, &self.metrics),
        };
        let Some(layout) = layout else {
            self.hide();
            return;
        };
        self.use_layout(layout);
        let previous = self.target;
        let window_changed = previous.is_none_or(|current| current.hwnd != target.hwnd);
        let shape_changed = previous.is_none_or(|current| {
            current.maximized != target.maximized
                || current.right - current.left != target.right - target.left
                || current.bottom - current.top != target.bottom - target.top
        });
        if !self.refresh_window(target, window_changed || shape_changed) {
            self.hide();
            return;
        }
        if window_changed {
            if self.press.take().is_some() || self.pending_drag.take().is_some() {
                let _ = release_capture_if_owned(self.hwnd);
            }
            self.hovered = false;
            self.alt_down = false;
            self.cover = None;
        }
        let cover = match placement {
            Placement::Cover(caption) => Some(CoverState {
                caption,
                rows: self.sample_cover_rows(target, caption, target.sample_unobstructed),
            }),
            Placement::Hidden => None,
        };
        let cover_changed = cover != self.cover;
        self.cover = cover;
        let target_changed = previous != Some(target) || cover_changed;
        let desired_origin = self.target_origin();
        self.target = Some(target);
        let was_invisible = !self.visible;
        if was_invisible || window_changed || self.cover.is_some() {
            if was_invisible || window_changed {
                self.cancel_green_hover();
            }
            self.render_origin = desired_origin;
            if was_invisible {
                self.visible = true;
                unsafe {
                    let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNOACTIVATE);
                }
            }
        } else {
            self.follow(desired_origin, dt);
        }
        let origin_changed = self.render_origin != desired_origin;
        if target_changed {
            self.settle_ticks = 20;
        } else if settle_active {
            self.settle_ticks = self.settle_ticks.max(8);
        }
        if target_changed || origin_changed || was_invisible {
            self.present();
        }
        if origin_changed || self.settle_ticks > 0 || self.hovered {
            self.ensure_anim_timer();
        } else {
            self.stop_anim_timer();
        }
    }

    fn follow(&mut self, desired_origin: (f32, f32), dt: f32) {
        if self.cover.is_some() {
            self.render_origin = desired_origin;
            return;
        }
        self.render_origin.0 = animate_value(
            self.render_origin.0,
            desired_origin.0,
            self.motion_metrics.window_controls_follow_rate,
            dt,
            self.motion,
        );
        self.render_origin.1 = animate_value(
            self.render_origin.1,
            desired_origin.1,
            self.motion_metrics.window_controls_follow_rate,
            dt,
            self.motion,
        );
    }

    fn tick_animation(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_tick = now;
        if self.target.is_none() {
            self.stop_anim_timer();
            return;
        }
        let desired_origin = self.target_origin();
        self.follow(desired_origin, dt);
        let origin_changed = self.render_origin != desired_origin;
        let settling = self.settle_ticks > 0;
        self.settle_ticks = self.settle_ticks.saturating_sub(1);
        let fill_changed = settling && self.resample_cover_fill();
        let alt_changed = self.hovered && {
            let alt = is_alt_pressed();
            let changed = alt != self.alt_down;
            self.alt_down = alt;
            changed
        };
        if origin_changed || fill_changed || alt_changed {
            self.present();
        }
        if !origin_changed && self.settle_ticks == 0 && !self.hovered {
            self.stop_anim_timer();
        }
    }

    fn ensure_anim_timer(&mut self) {
        if !self.animating {
            self.animating = true;
            unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    WINDOW_CONTROLS_ANIM_TIMER,
                    WINDOW_CONTROLS_ANIM_MS,
                    None,
                );
            }
        }
    }

    fn stop_anim_timer(&mut self) {
        if self.animating {
            self.animating = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), WINDOW_CONTROLS_ANIM_TIMER);
            }
        }
    }

    const fn pressed_visual(&self) -> Option<WindowControlAction> {
        match self.press {
            Some(press) if press.inside => Some(press.action),
            _ => None,
        }
    }

    fn resize_edge_at(&self, x: i32, y: i32) -> Option<ResizeEdge> {
        let target = self.target?;
        let resizable = self
            .window
            .as_ref()
            .is_some_and(|window| window.traits.resizable);
        self.layout
            .resize_edge(x, y, target.maximized, resizable, target.dpi)
    }

    fn on_set_cursor(&self) -> Option<LRESULT> {
        let (sx, sy) = cursor_position();
        let (ox, oy) = self.render_origin();
        let edge = self.resize_edge_at(sx - ox, sy - oy)?;
        let id = match edge {
            ResizeEdge::Top => IDC_SIZENS,
            ResizeEdge::TopRight => IDC_SIZENESW,
            ResizeEdge::Right => IDC_SIZEWE,
        };
        if let Ok(cursor) = unsafe { LoadCursorW(None, id) } {
            unsafe {
                let _ = SetCursor(Some(cursor));
            }
            return Some(LRESULT(1));
        }
        None
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        if let Some(press) = &mut self.press {
            let (group_hovered, press_changed) = step_press_move(press, &self.layout, x, y);
            let alt_down = group_hovered && is_alt_pressed();
            if press_changed || self.hovered != group_hovered || self.alt_down != alt_down {
                self.hovered = group_hovered;
                self.alt_down = alt_down;
                self.present();
            }
            return;
        }
        if let Some(drag) =
            should_start_caption_drag(&mut self.pending_drag, x, y, drag_threshold())
        {
            self.last_caption_click = None;
            let _ = release_capture_if_owned(self.hwnd);
            let _ =
                forward_caption_drag(drag.target_hwnd, drag.start_screen.0, drag.start_screen.1);
            return;
        }
        if self.pending_drag.is_some() {
            return;
        }
        if !self.tracking_mouse {
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: HWND(self.hwnd as *mut _),
                dwHoverTime: 0,
            };
            if unsafe { TrackMouseEvent(&mut track) }.is_ok() {
                self.tracking_mouse = true;
            }
        }
        let group_hovered = self.layout.hits_group(x, y);
        let alt_down = group_hovered && is_alt_pressed();
        if self.hovered != group_hovered || self.alt_down != alt_down {
            self.hovered = group_hovered;
            self.alt_down = alt_down;
            self.present();
        }
        if group_hovered {
            self.ensure_anim_timer();
        }
        let over_green = self.state.zoom
            && self.layout.hit_test(x, y) == Some(WindowControlAction::ToggleMaximize);
        if over_green && !self.green_hover_armed {
            self.green_hover_armed = unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    GREEN_HOVER_TIMER,
                    GREEN_HOVER_MS,
                    None,
                )
            } != 0;
        } else if !over_green {
            self.cancel_green_hover();
        }
    }

    fn cancel_green_hover(&mut self) {
        if self.green_hover_armed {
            self.green_hover_armed = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), GREEN_HOVER_TIMER);
            }
        }
    }

    fn on_green_hover(&mut self, runtime: &RuntimeContext) {
        self.cancel_green_hover();
        if self.press.is_none()
            && self.hovered
            && crate::menu::active_top_bar_target(runtime).is_none()
        {
            self.open_window_menu(runtime);
        }
    }

    fn open_window_menu(&mut self, runtime: &RuntimeContext) {
        let Some(target) = self.target else {
            return;
        };
        let Some(window) = inspect_window(target.hwnd, self.own_pid) else {
            return;
        };
        let (ox, oy) = self.render_origin();
        let Some(zoom_center) = self.layout.center_of(WindowControlAction::ToggleMaximize) else {
            return;
        };
        let anchor_x = ox + zoom_center.round() as i32 - self.layout.height / 2;
        crate::menu::open_window_tiling_menu(
            runtime,
            &window,
            anchor_x,
            oy + self.layout.height + 2,
        );
        self.window = Some(window);
    }

    fn on_mouse_leave(&mut self) {
        self.tracking_mouse = false;
        self.cancel_green_hover();
        if self.press.is_none() && (self.hovered || self.alt_down) {
            self.hovered = false;
            self.alt_down = false;
            self.present();
        } else {
            self.alt_down = false;
        }
        if self.settle_ticks == 0 && self.render_origin == self.target_origin() {
            self.stop_anim_timer();
        }
    }

    fn on_left_down(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        self.cancel_green_hover();
        crate::menu::close(runtime);
        if let Some(edge) = self.resize_edge_at(x, y) {
            self.last_caption_click = None;
            self.pending_drag = None;
            self.press = None;
            if let Some(target) = self.target {
                let (ox, oy) = self.render_origin();
                let _ = forward_resize_drag(target.hwnd, edge, ox + x, oy + y);
            }
            return;
        }
        if let Some(action) = self.layout.hit_test(x, y) {
            self.last_caption_click = None;
            self.pending_drag = None;
            if !self.state.allows(action) {
                return;
            }
            self.press = Some(PressState {
                action,
                inside: true,
            });
            self.hovered = true;
            unsafe {
                let _ = SetCapture(HWND(self.hwnd as *mut _));
            }
            self.present();
            return;
        }
        if !self.layout.hits_group(x, y) {
            self.on_empty_cover_down(x, y);
        }
    }

    fn on_left_up(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        self.pending_drag = None;
        let had_press = self.press.is_some();
        let (execute, group_hovered) = step_press_release(&mut self.press, &self.layout, x, y);
        let _ = release_capture_if_owned(self.hwnd);
        if had_press {
            self.hovered = group_hovered;
            if !group_hovered {
                self.alt_down = false;
            }
            self.present();
        }
        if let Some(action) = execute {
            self.execute_button_action(runtime, action);
        }
    }

    fn on_capture_changed(&mut self) {
        self.pending_drag = None;
        if self.press.take().is_some() {
            let (sx, sy) = cursor_position();
            let (ox, oy) = self.render_origin();
            self.hovered = self.layout.hits_group(sx - ox, sy - oy);
            if !self.hovered {
                self.alt_down = false;
            }
            self.present();
        }
    }

    fn execute_button_action(&self, runtime: &RuntimeContext, action: WindowControlAction) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let alt_down = is_alt_pressed();
        let command = match action {
            WindowControlAction::Close => WindowCommand::Close,
            WindowControlAction::Minimize => WindowCommand::Minimize,
            WindowControlAction::ToggleMaximize if alt_down => WindowCommand::ToggleFullScreen,
            WindowControlAction::ToggleMaximize => WindowCommand::Zoom,
        };
        let identity = window.identity;
        let live = inspect_window(identity.hwnd, self.own_pid)
            .filter(|live| live.identity.is_same_window(&identity));
        let Some(live) = live else {
            signal_command_failure();
            return;
        };
        if live.capability(command) != Capability::Available {
            signal_command_failure();
            return;
        }
        if window_control_scope(action, alt_down) == CommandScope::Application {
            self.execute_on_application(runtime, identity, command);
            return;
        }
        if command == WindowCommand::Minimize {
            let _ = crate::genie::minimize_window(runtime, identity.hwnd);
            return;
        }
        if execute_window_command(identity, command, self.own_pid) != CommandOutcome::Completed {
            signal_command_failure();
        }
    }

    fn execute_on_application(
        &self,
        runtime: &RuntimeContext,
        clicked: WindowIdentity,
        command: WindowCommand,
    ) {
        let targets = application_of(clicked, self.own_pid)
            .map(|context| application_command_targets(&context.app, clicked, command))
            .unwrap_or_default();
        if targets.is_empty() {
            signal_command_failure();
            return;
        }
        let mut failed = false;
        for target in targets {
            let completed = if command == WindowCommand::Minimize && target.is_same_window(&clicked)
            {
                crate::genie::minimize_window(runtime, target.hwnd)
            } else {
                execute_window_command(target, command, self.own_pid) == CommandOutcome::Completed
            };
            failed |= !completed;
        }
        if failed {
            signal_command_failure();
        }
    }

    fn on_empty_cover_down(&mut self, x: i32, y: i32) {
        let Some(target) = self.target else {
            return;
        };
        let now = Instant::now();
        let slop = scale(DOUBLE_CLICK_SLOP_DIP, target.dpi);
        let is_double_click = self
            .last_caption_click
            .is_some_and(|(prev_time, (px, py))| {
                now.duration_since(prev_time) <= Duration::from_millis(u64::from(double_click_ms()))
                    && (x - px).abs() <= slop
                    && (y - py).abs() <= slop
            });
        if is_double_click {
            self.last_caption_click = None;
            self.pending_drag = None;
            if let Some(window) = self.window.as_ref() {
                let identity = window.identity;
                if let Some(live) = inspect_window(identity.hwnd, self.own_pid)
                    .filter(|live| live.identity.is_same_window(&identity))
                    && live.capability(WindowCommand::Zoom) == Capability::Available
                {
                    let _ = execute_window_command(identity, WindowCommand::Zoom, self.own_pid);
                }
            }
            return;
        }
        self.last_caption_click = Some((now, (x, y)));
        let _ = activate_window_no_restore(target.hwnd);
        let (ox, oy) = self.render_origin();
        self.pending_drag = Some(PendingCaptionDrag {
            target_hwnd: target.hwnd,
            start_client: (x, y),
            start_screen: (ox + x, oy + y),
        });
        unsafe {
            let _ = SetCapture(HWND(self.hwnd as *mut _));
        }
    }

    fn on_right_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        self.cancel_green_hover();
        if self.layout.hits_group(x, y) {
            self.open_window_menu(runtime);
        } else if self.resize_edge_at(x, y).is_none()
            && let Some(target) = self.target
        {
            let (ox, oy) = self.render_origin();
            let _ = forward_caption_context_menu(target.hwnd, ox + x, oy + y);
        }
    }

    fn ensure_renderer(&mut self) -> bool {
        if self.renderer.is_none() {
            match WindowControlsRenderer::new(self.layout.width, self.layout.height) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    crate::host::log_present_error("window-controls renderer", Err(error));
                    return false;
                }
            }
        }
        true
    }

    fn present(&mut self) {
        let Some(target) = self.target else {
            return;
        };
        if !self.ensure_renderer() {
            return;
        }
        let cover = self.cover_style();
        let render_origin = self.render_origin();
        let pressed = self.pressed_visual();
        let state = self
            .state
            .with_alt_zoom_glyph(self.hovered && self.alt_down);
        if let Some(renderer) = self.renderer.as_mut() {
            crate::host::log_present_error(
                "window-controls",
                renderer.present(
                    self.hwnd,
                    render_origin,
                    &self.layout,
                    state,
                    target.maximized,
                    self.hovered,
                    pressed,
                    self.appearance,
                    cover.as_ref(),
                ),
            );
        }
        let _ = place_overlay_above(self.hwnd, target.hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Overlay, PendingCaptionDrag, Placement, PressState, plan_overlay_slots,
        should_start_caption_drag, step_press_move, step_press_release,
    };
    use otto_core::{
        Metrics, NativeCaption, NativeCaptionStyle, WindowControlAction, WindowControlsMetrics,
        compute_cover_layout,
    };
    use otto_platform::ActiveWindowTarget;

    #[test]
    fn a_window_keeps_the_overlay_that_already_covers_it() {
        let slots = plan_overlay_slots(&[Some(7), Some(3)], &[3, 7]);
        assert_eq!(slots, vec![Some(7), Some(3)]);
    }

    #[test]
    fn a_new_window_takes_a_free_overlay_before_a_new_one_is_added() {
        let slots = plan_overlay_slots(&[Some(7), None], &[7, 9]);
        assert_eq!(slots, vec![Some(7), Some(9)]);
        let grown = plan_overlay_slots(&[Some(7)], &[7, 9]);
        assert_eq!(grown, vec![Some(7), Some(9)]);
    }

    #[test]
    fn an_overlay_whose_window_is_gone_is_released_and_reused() {
        let slots = plan_overlay_slots(&[Some(7), Some(3)], &[3, 9]);
        assert_eq!(slots, vec![Some(9), Some(3)]);
        let emptied = plan_overlay_slots(&[Some(7), Some(3)], &[]);
        assert_eq!(emptied, vec![None, None]);
    }

    fn sample_caption() -> NativeCaption {
        NativeCaption::new(
            NativeCaptionStyle {
                has_caption: true,
                has_system_menu: true,
                can_minimize: true,
                can_maximize: true,
            },
            500,
            10,
            30,
            true,
            96,
        )
        .unwrap()
    }

    #[test]
    fn press_and_release_cancels_when_pointer_leaves_button_and_executes_when_returned() {
        let layout =
            compute_cover_layout(sample_caption(), &WindowControlsMetrics::default()).unwrap();
        let close_x = layout.centers_x[2].round() as i32;
        let cy = layout.center_y.round() as i32;
        let mut press = Some(PressState {
            action: WindowControlAction::Close,
            inside: true,
        });
        let (hovered_out, changed_out) = step_press_move(press.as_mut().unwrap(), &layout, 10, cy);
        assert!(changed_out);
        assert!(!hovered_out);
        assert!(!press.unwrap().inside);

        let (exec_cancel, _) = step_press_release(&mut press, &layout, 10, cy);
        assert_eq!(exec_cancel, None);
        assert!(press.is_none());

        let mut press_back = Some(PressState {
            action: WindowControlAction::Close,
            inside: false,
        });
        let (hovered_in, changed_in) =
            step_press_move(press_back.as_mut().unwrap(), &layout, close_x, cy);
        assert!(changed_in);
        assert!(hovered_in);
        let (exec_ok, hovered_after) = step_press_release(&mut press_back, &layout, close_x, cy);
        assert_eq!(exec_ok, Some(WindowControlAction::Close));
        assert!(hovered_after);
    }

    #[test]
    fn pending_caption_drag_waits_for_threshold_before_forwarding() {
        let mut pending = Some(PendingCaptionDrag {
            target_hwnd: 42,
            start_client: (20, 15),
            start_screen: (400, 100),
        });
        assert_eq!(
            should_start_caption_drag(&mut pending, 23, 17, (4, 4)),
            None
        );
        assert!(pending.is_some());
        let drag = should_start_caption_drag(&mut pending, 24, 15, (4, 4)).unwrap();
        assert_eq!(drag.target_hwnd, 42);
        assert!(pending.is_none());
    }

    #[test]
    fn overlay_remains_active_under_partial_top_bar_overlap_and_hides_when_fully_covered() {
        let partial_caption = sample_caption();
        let target = ActiveWindowTarget {
            hwnd: 99,
            left: 100,
            top: 10,
            right: 500,
            bottom: 400,
            maximized: false,
            dpi: 96,
            on_primary: true,
            active: true,
            sample_unobstructed: true,
            caption: Some(partial_caption),
        };
        assert_eq!(
            Overlay::placement(target, Metrics::MACBOOK_PRO_14.bar_height),
            Placement::Cover(partial_caption)
        );
        assert_eq!(
            Overlay::placement(target, Metrics::DYNAMIC_ISLAND.bar_height),
            Placement::Hidden
        );

        let covered_caption = NativeCaption::new(
            NativeCaptionStyle {
                has_caption: true,
                has_system_menu: true,
                can_minimize: true,
                can_maximize: true,
            },
            500,
            0,
            30,
            true,
            96,
        )
        .unwrap();
        let covered_target = ActiveWindowTarget {
            caption: Some(covered_caption),
            ..target
        };
        assert_eq!(
            Overlay::placement(covered_target, Metrics::MACBOOK_PRO_14.bar_height),
            Placement::Hidden
        );
    }

    #[test]
    fn wm_close_is_absorbed_without_destroying_surface() {
        let runtime = crate::host::RuntimeContext::new(false);
        assert_eq!(
            super::handle_message(
                &runtime,
                0,
                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            ),
            Some(windows::Win32::Foundation::LRESULT(0))
        );
    }

    #[test]
    fn overlay_starts_without_renderer_and_updates_layout_lazily() {
        let mut overlay = Overlay::new(
            0,
            1,
            otto_core::Appearance::glass(otto_core::ResolvedTheme::Dark),
            32,
        );
        assert!(overlay.renderer.is_none());
        let mut next_layout = overlay.layout;
        next_layout.width += 10;
        overlay.use_layout(next_layout);
        assert_eq!(overlay.layout, next_layout);
        assert!(overlay.renderer.is_none());
        overlay.hide();
        assert!(overlay.renderer.is_none());
    }
}
