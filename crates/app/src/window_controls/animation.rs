use crate::host::RuntimeContext;
use crate::win_msg::{GREEN_HOVER_TIMER, WINDOW_CONTROLS_ANIM_TIMER};
use otto_core::WindowControlAction;
use otto_platform::{is_virtual_key_down, kill_window_timer, set_window_timer};

use super::{MoveSizeState, Overlay, WINDOW_CONTROLS_ANIM_MS, is_alt_pressed};

const VK_LBUTTON: i32 = 0x01;

impl Overlay {
    pub(super) fn kill_timers(&self) {
        kill_window_timer(self.hwnd, WINDOW_CONTROLS_ANIM_TIMER);
        kill_window_timer(self.hwnd, GREEN_HOVER_TIMER);
    }

    pub(super) fn tick_animation(&mut self) {
        if self.target.is_none() {
            self.move_size = MoveSizeState::Idle;
            self.stop_anim_timer();
            return;
        }
        if self.move_size == MoveSizeState::Active {
            if is_virtual_key_down(VK_LBUTTON) {
                let _ = self.fast_follow_target();
                return;
            }
            self.move_size = MoveSizeState::Idle;
        }
        let settling = self.settle_ticks > 0;
        self.settle_ticks = self.settle_ticks.saturating_sub(1);
        let fill_changed = settling && self.resample_cover_fill();
        let alt_changed = self.hover.hovered && {
            let alt = is_alt_pressed();
            let changed = alt != self.hover.alt_down;
            self.hover.alt_down = alt;
            changed
        };
        if fill_changed || alt_changed {
            self.present();
        }
        if self.settle_ticks == 0 && !self.hover.hovered && self.move_size == MoveSizeState::Idle {
            self.stop_anim_timer();
        }
    }

    pub(super) fn ensure_anim_timer(&mut self) {
        if !self.animating {
            self.animating = true;
            let _ = set_window_timer(
                self.hwnd,
                WINDOW_CONTROLS_ANIM_TIMER,
                WINDOW_CONTROLS_ANIM_MS,
            );
        }
    }

    pub(super) fn stop_anim_timer(&mut self) {
        if self.animating {
            self.animating = false;
            kill_window_timer(self.hwnd, WINDOW_CONTROLS_ANIM_TIMER);
        }
    }

    pub(super) const fn pressed_visual(&self) -> Option<WindowControlAction> {
        match self.press {
            Some(press) if press.inside => Some(press.action),
            _ => None,
        }
    }

    pub(super) fn cancel_green_hover(&mut self) {
        if self.hover.green_hover_armed {
            self.hover.green_hover_armed = false;
            kill_window_timer(self.hwnd, GREEN_HOVER_TIMER);
        }
    }

    pub(super) fn on_green_hover(&mut self, runtime: &RuntimeContext) {
        self.cancel_green_hover();
        if self.press.is_none()
            && self.hover.hovered
            && crate::menu::active_top_bar_target(runtime).is_none()
        {
            self.open_window_menu(runtime);
        }
    }
}
