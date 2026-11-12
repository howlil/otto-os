use std::mem::size_of;
use std::time::Instant;

use crate::host::RuntimeContext;
use otto_core::{
    Appearance, Capability, CommandOutcome, Metrics, MotionMetrics, MotionPreference,
    WindowCommand, WindowControlAction, WindowControlsLayout, WindowControlsMetrics,
    WindowControlsState, WindowInfo, accepts_window_controls, animate_value,
    compute_window_controls_layout, scale, window_controls_top,
};
use otto_platform::{
    ActiveWindowTarget, acknowledge_native_minimize, acknowledge_window_sync, active_window_target,
    current_process_id, execute_window_command, inspect_window, install_native_minimize_hook,
    install_window_tracking_hook, minimize_window_native, motion_preference,
    poll_foreground_change, primary_screen, signal_command_failure, uninstall_native_minimize_hook,
    uninstall_window_tracking_hook,
};
use otto_render::WindowControlsRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SetTimer, SetWindowPos, ShowWindow, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_APP_WINDOW_SYNC: u32 = 0x8000 + 20;
const WM_APP_NATIVE_MINIMIZE: u32 = 0x8000 + 24;
const WINDOW_CONTROLS_ANIM_TIMER: usize = 20;
const WINDOW_CONTROLS_ANIM_MS: u32 = 16;
const GREEN_HOVER_TIMER: usize = 21;
const GREEN_HOVER_MS: u32 = 600;

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct WindowControls {
    hwnd: isize,
    own_pid: u32,
    appearance: Appearance,
    renderer: WindowControlsRenderer,
    metrics: WindowControlsMetrics,
    layout: WindowControlsLayout,
    target: Option<ActiveWindowTarget>,
    window: Option<WindowInfo>,
    state: WindowControlsState,
    layout_dpi: u32,
    render_origin: (f32, f32),
    settle_ticks: u8,
    green_hover_armed: bool,
    hovered: bool,
    visible: bool,
    tracking_mouse: bool,
    animating: bool,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    last_tick: Instant,
}

pub fn start(runtime: &RuntimeContext, hwnd: isize) -> Result<()> {
    let own_pid = current_process_id();
    let screen = primary_screen();
    let metrics = WindowControlsMetrics::default();
    let layout = compute_window_controls_layout(&metrics, screen.dpi);
    let renderer = WindowControlsRenderer::new(layout.width, layout.height)?;

    let mut wc = WindowControls {
        hwnd,
        own_pid,
        appearance: crate::theme_state::appearance(runtime),
        renderer,
        metrics,
        layout,
        target: None,
        window: None,
        state: WindowControlsState::default(),
        layout_dpi: screen.dpi,
        render_origin: (0.0, 0.0),
        settle_ticks: 0,
        green_hover_armed: false,
        hovered: false,
        visible: false,
        tracking_mouse: false,
        animating: false,
        motion: motion_preference(),
        motion_metrics: MotionMetrics::default(),
        last_tick: Instant::now(),
    };
    install_window_tracking_hook(hwnd, WM_APP_WINDOW_SYNC);
    let _ = install_native_minimize_hook(hwnd, WM_APP_NATIVE_MINIMIZE);
    wc.sync_target();

    *runtime.window_controls.borrow_mut() = Some(wc);
    Ok(())
}

pub fn stop(runtime: &RuntimeContext) {
    uninstall_native_minimize_hook();
    uninstall_window_tracking_hook();
    if let Some(controls) = runtime.window_controls.borrow_mut().take() {
        unsafe {
            let _ = KillTimer(
                Some(HWND(controls.hwnd as *mut _)),
                WINDOW_CONTROLS_ANIM_TIMER,
            );
            let _ = KillTimer(Some(HWND(controls.hwnd as *mut _)), GREEN_HOVER_TIMER);
        }
    }
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
        WM_APP_WINDOW_SYNC => {
            acknowledge_window_sync();
            with_window_controls(runtime, WindowControls::sync_target);
            Some(LRESULT(0))
        }
        WM_APP_NATIVE_MINIMIZE => {
            minimize_from_native_button(runtime, wparam.0 as isize);
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == WINDOW_CONTROLS_ANIM_TIMER => {
            with_window_controls(runtime, WindowControls::tick_animation);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_window_controls(runtime, |wc| wc.on_mouse_move(x, y));
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == GREEN_HOVER_TIMER => {
            with_window_controls(runtime, |wc| wc.on_green_hover(runtime));
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_window_controls(runtime, WindowControls::on_mouse_leave);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_window_controls(runtime, |wc| wc.on_left_click(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_window_controls(runtime, |wc| wc.on_right_click(runtime, x, y));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_window_controls(runtime, WindowControls::rebuild);
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
        if controls.visible {
            controls.present();
        }
    });
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
        with_window_controls(runtime, WindowControls::sync_target);
    }
}

fn with_window_controls(runtime: &RuntimeContext, action: impl FnOnce(&mut WindowControls)) {
    if let Ok(mut guard) = runtime.window_controls.try_borrow_mut()
        && let Some(wc) = guard.as_mut()
    {
        action(wc);
    }
}

impl WindowControls {
    fn bar_height_for(target: ActiveWindowTarget) -> Option<i32> {
        target
            .on_primary
            .then(|| scale(Metrics::default().notch_height, target.dpi))
    }

    fn target_origin(&self, target: ActiveWindowTarget) -> (f32, f32) {
        let x = (target.left + self.layout.offset_x) as f32;
        let y = window_controls_top(
            target.top,
            self.layout.offset_y,
            Self::bar_height_for(target),
            target.maximized,
        ) as f32;
        (x, y)
    }

    fn title_bar_visible(&self, target: ActiveWindowTarget) -> bool {
        Self::bar_height_for(target)
            .is_none_or(|bar| target.maximized || target.top + self.layout.height > bar)
    }

    fn match_target_dpi(&mut self, dpi: u32) {
        if dpi == self.layout_dpi {
            return;
        }
        let layout = compute_window_controls_layout(&self.metrics, dpi);
        if let Ok(renderer) = WindowControlsRenderer::new(layout.width, layout.height) {
            self.layout = layout;
            self.renderer = renderer;
            self.layout_dpi = dpi;
        }
    }

    fn refresh_window(&mut self, target: ActiveWindowTarget, reinspect: bool) -> bool {
        if reinspect || self.window.is_none() {
            self.window = inspect_window(target.hwnd, self.own_pid);
        }
        let accepted = self.window.as_ref().is_some_and(accepts_window_controls);
        self.state = self
            .window
            .as_ref()
            .map_or_else(WindowControlsState::default, WindowControlsState::of);
        accepted
    }

    fn hide(&mut self) {
        self.stop_anim_timer();
        self.cancel_green_hover();
        if self.visible {
            self.visible = false;
            self.hovered = false;
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            }
        }
        self.target = None;
        self.window = None;
    }

    fn render_origin(&self) -> (i32, i32) {
        (
            self.render_origin.0.round() as i32,
            self.render_origin.1.round() as i32,
        )
    }

    fn rebuild(&mut self) {
        let screen = primary_screen();
        self.layout_dpi = 0;
        self.match_target_dpi(screen.dpi);
        self.hide();
        self.sync_target();
    }

    fn sync_target(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_tick = now;
        self.motion = motion_preference();
        let Some(target) = active_window_target(self.own_pid) else {
            self.hide();
            return;
        };
        self.match_target_dpi(target.dpi);
        let previous = self.target;
        let window_changed = previous.is_none_or(|current| current.hwnd != target.hwnd);
        let shape_changed = previous.is_none_or(|current| {
            current.maximized != target.maximized
                || current.right - current.left != target.right - target.left
                || current.bottom - current.top != target.bottom - target.top
        });
        if !self.title_bar_visible(target)
            || !self.refresh_window(target, window_changed || shape_changed)
        {
            self.hide();
            return;
        }
        let target_changed = previous != Some(target);
        let desired_origin = self.target_origin(target);
        self.target = Some(target);
        let was_invisible = !self.visible;
        if was_invisible || window_changed {
            self.cancel_green_hover();
            self.render_origin = desired_origin;
            self.visible = true;
            unsafe {
                let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_SHOWNOACTIVATE);
            }
        } else {
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
        let origin_changed = self.render_origin != desired_origin;
        if target_changed {
            self.settle_ticks = 20;
        }
        if target_changed || origin_changed || was_invisible {
            self.present();
        }
        if origin_changed || self.settle_ticks > 0 {
            self.ensure_anim_timer();
        } else {
            self.stop_anim_timer();
        }
    }

    fn tick_animation(&mut self) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_tick = now;
        let Some(target) = self.target else {
            self.stop_anim_timer();
            return;
        };
        let desired_origin = self.target_origin(target);
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
        let origin_changed = self.render_origin != desired_origin;
        let settling = self.settle_ticks > 0;
        self.settle_ticks = self.settle_ticks.saturating_sub(1);
        if origin_changed || settling {
            self.present();
        }
        if !origin_changed && self.settle_ticks == 0 {
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

    fn on_mouse_move(&mut self, x: i32, y: i32) {
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
        if !self.hovered {
            self.hovered = true;
            self.present();
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
        if self.hovered && crate::menu::active_top_bar_target(runtime).is_none() {
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
        let anchor_x = ox + self.layout.centers_x[2].round() as i32 - self.layout.height / 2;
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
        if self.hovered {
            self.hovered = false;
            self.present();
        }
    }

    fn on_left_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        self.cancel_green_hover();
        crate::menu::close(runtime);
        let Some(action) = self.layout.hit_test(x, y) else {
            return;
        };
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let alt_down = unsafe { (GetAsyncKeyState(i32::from(VK_MENU.0)) as u16 & 0x8000) != 0 };
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
        if command == WindowCommand::Minimize {
            let _ = crate::genie::minimize_window(runtime, identity.hwnd);
            return;
        }
        if execute_window_command(identity, command, self.own_pid) != CommandOutcome::Completed {
            signal_command_failure();
        }
    }

    fn on_right_click(&mut self, runtime: &RuntimeContext, _x: i32, _y: i32) {
        self.cancel_green_hover();
        self.open_window_menu(runtime);
    }

    fn present(&self) {
        let Some(target) = self.target else {
            return;
        };
        let _ = self.renderer.present(
            self.hwnd,
            self.render_origin(),
            &self.layout,
            self.state,
            target.maximized,
            self.hovered,
            self.appearance,
        );
        unsafe {
            let _ = SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}
