use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_CONTROL_CENTER_KEY;
use otto_core::{
    BarKey, ControlCenterFocusOutcome, ControlCenterHitTarget, control_center_action,
    control_center_focus_targets, scale, scroll_offset_for_item, step_slider_percent,
};
use otto_platform::{release_capture_if_owned, route_bar_keys, stop_routing_if_targets};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::SetCapture;

use super::ControlCenterSurface;
use super::PostClickAction;
use super::WHEEL_SCROLL_STEP;

impl ControlCenterSurface {
    pub(super) fn enter_keyboard_focus(&mut self) -> bool {
        let targets = control_center_focus_targets(&self.layout, &self.snapshot);
        if self.focus.enter(&targets).is_some()
            && route_bar_keys(self.hwnd, WM_APP_CONTROL_CENTER_KEY)
        {
            self.ensure_focused_visible();
            true
        } else {
            self.focus.exit();
            false
        }
    }

    pub(super) fn leave_keyboard_focus(&mut self) -> bool {
        stop_routing_if_targets(self.hwnd);
        self.focus.exit()
    }

    pub(super) fn ensure_focused_visible(&mut self) {
        let Some(target) = self.focus.current() else {
            return;
        };
        if let Some(next_scroll) = scroll_offset_for_item(&self.layout, target) {
            self.scroll_offset = next_scroll;
            self.relayout();
        }
    }

    pub(super) fn begin_press(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if self.begin_drag(runtime, x, y) {
            return;
        }
        let Some(target) = self.layout.hit_test(x, y) else {
            return;
        };
        if control_center_action(&self.snapshot, target).is_none() {
            return;
        }
        let _ = self.leave_keyboard_focus();
        self.pressed = Some(target);
        self.hovered = Some(target);
        if self.hwnd != 0 {
            unsafe {
                let _ = SetCapture(HWND(self.hwnd as *mut _));
            }
        }
        self.present(runtime);
    }

    pub(super) fn release_press(
        &mut self,
        runtime: &RuntimeContext,
        x: i32,
        y: i32,
    ) -> PostClickAction {
        if self.dragging.is_some() {
            self.end_drag();
            return PostClickAction::None;
        }
        let pressed = self.pressed.take();
        let _ = release_capture_if_owned(self.hwnd);
        let current = self.layout.hit_test(x, y);
        if pressed.is_none() || pressed == current {
            let action = self.on_click(runtime, x, y);
            if matches!(action, PostClickAction::None) && pressed.is_some() {
                self.present(runtime);
            }
            action
        } else {
            self.hovered = current;
            self.present(runtime);
            PostClickAction::None
        }
    }

    pub(super) fn begin_drag(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> bool {
        let Some(target) = self.layout.hit_test(x, y) else {
            return false;
        };
        let is_slider = matches!(
            target,
            ControlCenterHitTarget::DisplaySlider if self.snapshot.status.brightness.is_some()
        ) || target == ControlCenterHitTarget::SoundSlider;
        if !is_slider {
            return false;
        }
        let _ = self.leave_keyboard_focus();
        self.dragging = Some(target);
        unsafe {
            let _ = SetCapture(HWND(self.hwnd as *mut _));
        }
        self.apply_slider(runtime, target, x);
        true
    }

    pub(super) fn end_drag(&mut self) {
        if self.dragging.take().is_some() {
            let _ = release_capture_if_owned(self.hwnd);
        }
    }

    pub(super) fn commit_slider_value(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        value: u8,
    ) {
        match target {
            ControlCenterHitTarget::DisplaySlider => {
                if let Some(pct) = self.port.set_brightness(value) {
                    self.snapshot.status.brightness = Some(pct);
                }
            }
            ControlCenterHitTarget::SoundSlider => {
                if let Some((vol, muted)) = self.port.set_volume(value) {
                    self.snapshot.status.volume = vol;
                    self.snapshot.status.muted = muted;
                }
            }
            _ => return,
        }
        self.present(runtime);
    }

    pub(super) fn apply_slider(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        x: i32,
    ) {
        let Some(value) = self.layout.slider_value_at(target, x) else {
            return;
        };
        self.commit_slider_value(runtime, target, value);
    }

    pub(super) fn step_slider(
        &mut self,
        runtime: &RuntimeContext,
        target: ControlCenterHitTarget,
        delta: i8,
    ) {
        let current = match target {
            ControlCenterHitTarget::DisplaySlider => match self.snapshot.status.brightness {
                Some(pct) => pct,
                None => return,
            },
            ControlCenterHitTarget::SoundSlider => {
                if self.snapshot.status.muted {
                    0
                } else {
                    self.snapshot.status.volume
                }
            }
            _ => return,
        };
        let next = step_slider_percent(current, delta);
        self.commit_slider_value(runtime, target, next);
    }

    pub(super) fn on_mouse_move(&mut self, runtime: &RuntimeContext, x: i32, y: i32) {
        if let Some(target) = self.dragging {
            self.apply_slider(runtime, target, x);
            return;
        }
        let next = self.layout.hit_test(x, y);
        let cleared_focus = next.is_some() && self.leave_keyboard_focus();
        let pressed_active_changed = self
            .pressed
            .is_some_and(|target| (self.hovered == Some(target)) != (next == Some(target)));
        if next != self.hovered || cleared_focus || pressed_active_changed {
            self.hovered = next;
            self.present(runtime);
        }
    }

    pub(super) fn on_wheel(&mut self, runtime: &RuntimeContext, delta: i16) {
        if !self.visible || self.layout.max_scroll <= 0 {
            return;
        }
        let cleared_focus = self.leave_keyboard_focus();
        let step = if delta > 0 {
            -scale(WHEEL_SCROLL_STEP, self.screen.dpi)
        } else {
            scale(WHEEL_SCROLL_STEP, self.screen.dpi)
        };
        let next = (self.scroll_offset + step).clamp(0, self.layout.max_scroll);
        if next != self.scroll_offset || cleared_focus {
            self.scroll_offset = next;
            self.relayout();
            self.present(runtime);
        }
    }

    pub(super) fn on_click(&mut self, runtime: &RuntimeContext, x: i32, y: i32) -> PostClickAction {
        if self.leave_keyboard_focus() {
            self.present(runtime);
        }
        let Some(target) = self.layout.hit_test(x, y) else {
            return PostClickAction::None;
        };
        let Some(outcome) = control_center_action(&self.snapshot, target) else {
            return PostClickAction::None;
        };
        PostClickAction::Command(outcome.command, outcome.dismiss)
    }

    pub(super) fn on_key(&mut self, runtime: &RuntimeContext, key: BarKey) -> PostClickAction {
        if !self.visible {
            let _ = self.leave_keyboard_focus();
            return PostClickAction::None;
        }
        let targets = control_center_focus_targets(&self.layout, &self.snapshot);
        match self.focus.handle(key, &targets) {
            ControlCenterFocusOutcome::Moved(_) => {
                self.ensure_focused_visible();
                self.present(runtime);
                PostClickAction::None
            }
            ControlCenterFocusOutcome::Exited => PostClickAction::Close,
            ControlCenterFocusOutcome::AdjustSlider { target, delta } => {
                self.step_slider(runtime, target, delta);
                PostClickAction::None
            }
            ControlCenterFocusOutcome::Activate(target) => {
                let Some(outcome) = control_center_action(&self.snapshot, target) else {
                    return PostClickAction::None;
                };
                PostClickAction::Command(outcome.command, outcome.dismiss)
            }
            ControlCenterFocusOutcome::Unchanged => PostClickAction::None,
        }
    }
}
