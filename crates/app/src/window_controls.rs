use std::cell::RefCell;
use std::mem::size_of;
use std::time::Instant;

use otto_core::{
    MotionMetrics, MotionPreference, WindowControlAction, WindowControlsLayout,
    WindowControlsMetrics, animate_value, compute_window_controls_layout,
};
use otto_platform::{
    ActiveWindowTarget, active_window_target, current_process_id, execute_window_control,
    install_window_tracking_hook, motion_preference, primary_screen, sample_titlebar_rgb,
    uninstall_window_tracking_hook,
};
use otto_render::WindowControlsRenderer;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SetTimer, SetWindowPos, ShowWindow, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER,
};
use windows::core::Result;

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_APP_WINDOW_SYNC: u32 = 0x8000 + 20;
const WINDOW_CONTROLS_ANIM_TIMER: usize = 20;
const WINDOW_CONTROLS_ANIM_MS: u32 = 16;

thread_local! {
    static WINDOW_CONTROLS: RefCell<Option<WindowControls>> = const { RefCell::new(None) };
}

#[allow(clippy::struct_excessive_bools)]
struct WindowControls {
    hwnd: isize,
    own_pid: u32,
    renderer: WindowControlsRenderer,
    metrics: WindowControlsMetrics,
    layout: WindowControlsLayout,
    target: Option<ActiveWindowTarget>,
    render_origin: (f32, f32),
    titlebar_rgb: (u8, u8, u8),
    settle_ticks: u8,
    hovered: bool,
    visible: bool,
    tracking_mouse: bool,
    animating: bool,
    motion: MotionPreference,
    motion_metrics: MotionMetrics,
    last_tick: Instant,
}

pub fn start(hwnd: isize) -> Result<()> {
    let own_pid = current_process_id();
    let screen = primary_screen();
    let metrics = WindowControlsMetrics::default();
    let layout = compute_window_controls_layout(&metrics, screen.dpi);
    let renderer = WindowControlsRenderer::new(layout.width, layout.height)?;

    let mut wc = WindowControls {
        hwnd,
        own_pid,
        renderer,
        metrics,
        layout,
        target: None,
        render_origin: (0.0, 0.0),
        titlebar_rgb: (32, 33, 36),
        settle_ticks: 0,
        hovered: false,
        visible: false,
        tracking_mouse: false,
        animating: false,
        motion: motion_preference(),
        motion_metrics: MotionMetrics::default(),
        last_tick: Instant::now(),
    };
    install_window_tracking_hook(hwnd, WM_APP_WINDOW_SYNC);
    wc.sync_target();

    WINDOW_CONTROLS.with(|cell| *cell.borrow_mut() = Some(wc));
    Ok(())
}

pub fn stop() {
    uninstall_window_tracking_hook();
    WINDOW_CONTROLS.with(|cell| {
        if let Some(controls) = cell.borrow_mut().take() {
            unsafe {
                let _ = KillTimer(
                    Some(HWND(controls.hwnd as *mut _)),
                    WINDOW_CONTROLS_ANIM_TIMER,
                );
            }
        }
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
        WM_APP_WINDOW_SYNC => {
            with_window_controls(WindowControls::sync_target);
            Some(LRESULT(0))
        }
        WM_TIMER if wparam.0 == WINDOW_CONTROLS_ANIM_TIMER => {
            with_window_controls(WindowControls::tick_animation);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            with_window_controls(WindowControls::on_mouse_move);
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_window_controls(WindowControls::on_mouse_leave);
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_window_controls(|wc| wc.on_left_click(x, y));
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_window_controls(|wc| wc.on_right_click(x, y));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_window_controls(WindowControls::rebuild);
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

fn with_window_controls(action: impl FnOnce(&mut WindowControls)) {
    WINDOW_CONTROLS.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut()
            && let Some(wc) = guard.as_mut()
        {
            action(wc);
        }
    });
}

impl WindowControls {
    fn target_origin(&self, target: ActiveWindowTarget) -> (f32, f32) {
        (
            (target.right - self.layout.width - self.layout.offset_right) as f32,
            (target.top + self.layout.offset_y) as f32,
        )
    }

    fn render_origin(&self) -> (i32, i32) {
        (
            self.render_origin.0.round() as i32,
            self.render_origin.1.round() as i32,
        )
    }

    fn rebuild(&mut self) {
        let screen = primary_screen();
        self.layout = compute_window_controls_layout(&self.metrics, screen.dpi);
        if let Ok(renderer) = WindowControlsRenderer::new(self.layout.width, self.layout.height) {
            self.renderer = renderer;
        }
        self.target = None;
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
        let next = active_window_target(self.own_pid);
        if let Some(target) = next {
            let previous_target = self.target;
            let target_changed = previous_target != Some(target);
            let window_changed = previous_target.is_none_or(|current| current.hwnd != target.hwnd);
            let desired_origin = self.target_origin(target);
            self.target = Some(target);
            let was_invisible = !self.visible;
            if was_invisible || window_changed {
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
                self.titlebar_rgb = sample_titlebar_rgb(target, self.layout.width);
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
        } else {
            self.stop_anim_timer();
            if self.visible {
                self.visible = false;
                self.hovered = false;
                self.target = None;
                unsafe {
                    let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
                }
            }
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
        if self.settle_ticks > 0 {
            self.settle_ticks -= 1;
            if self.settle_ticks.is_multiple_of(2) {
                let sampled = sample_titlebar_rgb(target, self.layout.width);
                if sampled != self.titlebar_rgb {
                    self.titlebar_rgb = sampled;
                }
            }
        }
        self.present();
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

    fn on_mouse_move(&mut self) {
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
    }

    fn on_mouse_leave(&mut self) {
        self.tracking_mouse = false;
        if self.hovered {
            self.hovered = false;
            self.present();
        }
    }

    fn on_left_click(&mut self, x: i32, y: i32) {
        crate::menu::close();
        let Some(action) = self.layout.hit_test(x, y) else {
            return;
        };
        let Some(target) = self.target else {
            return;
        };
        if action == WindowControlAction::Minimize {
            let _ = crate::genie::minimize_window(target.hwnd);
        } else {
            execute_window_control(target.hwnd, action);
        }
    }

    fn on_right_click(&mut self, x: i32, y: i32) {
        let Some(target) = self.target else {
            return;
        };
        let _ = self.layout.hit_test(x, y);
        let (ox, oy) = self.render_origin();
        crate::menu::open_window_tiling_menu(
            target.hwnd,
            target.maximized,
            ox,
            oy + self.layout.height + 4,
        );
    }

    fn present(&self) {
        let Some(target) = self.target else {
            return;
        };
        let _ = self.renderer.present(
            self.hwnd,
            self.render_origin(),
            &self.layout,
            self.titlebar_rgb,
            target.maximized,
            self.hovered,
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
