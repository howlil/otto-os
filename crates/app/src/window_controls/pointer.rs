use std::mem::size_of;
use std::time::{Duration, Instant};

use crate::host::RuntimeContext;
use crate::win_msg::GREEN_HOVER_TIMER;
use otto_core::{Capability, ResizeEdge, WindowCommand, WindowControlAction, scale};
use otto_platform::{
    activate_window_no_restore, cursor_position, double_click_ms, drag_threshold,
    execute_window_command, forward_caption_context_menu, forward_caption_drag,
    forward_resize_drag, inspect_window, release_capture_if_owned,
};
use windows::Win32::Foundation::{HWND, LRESULT};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    IDC_SIZENESW, IDC_SIZENS, IDC_SIZEWE, LoadCursorW, SetCursor, SetTimer,
};

use super::DOUBLE_CLICK_SLOP_DIP;
use super::GREEN_HOVER_MS;
use super::Overlay;
use super::PendingCaptionDrag;
use super::PressState;
use super::is_alt_pressed;
use super::should_start_caption_drag;
use super::step_press_move;
use super::step_press_release;

impl Overlay {
    pub(super) fn resize_edge_at(&self, x: i32, y: i32) -> Option<ResizeEdge> {
        let target = self.target?;
        let resizable = self
            .window
            .as_ref()
            .is_some_and(|window| window.traits.resizable);
        self.layout
            .resize_edge(x, y, target.maximized, resizable, target.dpi)
    }

    pub(super) fn on_set_cursor(&self) -> Option<LRESULT> {
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

    pub(super) fn on_mouse_move(&mut self, x: i32, y: i32) {
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
            if !self.hovered && group_hovered {
                let _ = self.resample_cover_fill();
            }
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

    pub(super) fn on_mouse_leave(&mut self) {
        self.tracking_mouse = false;
        self.cancel_green_hover();
        if self.press.is_none() && (self.hovered || self.alt_down) {
            self.hovered = false;
            self.alt_down = false;
            self.present();
        } else {
            self.alt_down = false;
        }
        if self.settle_ticks == 0 {
            self.stop_anim_timer();
        }
    }

    pub(super) fn on_left_down(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
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

    pub(super) fn on_left_up(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
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

    pub(super) fn on_capture_changed(&mut self) {
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

    pub(super) fn on_empty_cover_down(&mut self, x: i32, y: i32) {
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

    pub(super) fn on_right_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
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
}
