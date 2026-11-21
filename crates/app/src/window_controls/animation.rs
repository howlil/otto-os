use crate::host::RuntimeContext;
use crate::win_msg::{GREEN_HOVER_TIMER, WINDOW_CONTROLS_ANIM_TIMER};
use otto_core::WindowControlAction;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

use super::Overlay;
use super::WINDOW_CONTROLS_ANIM_MS;
use super::is_alt_pressed;

impl Overlay {
    pub(super) fn kill_timers(&self) {
        unsafe {
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), WINDOW_CONTROLS_ANIM_TIMER);
            let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), GREEN_HOVER_TIMER);
        }
    }

    pub(super) fn tick_animation(&mut self) {
        if self.target.is_none() {
            self.stop_anim_timer();
            return;
        }
        let settling = self.settle_ticks > 0;
        self.settle_ticks = self.settle_ticks.saturating_sub(1);
        let fill_changed = settling && self.resample_cover_fill();
        let alt_changed = self.hovered && {
            let alt = is_alt_pressed();
            let changed = alt != self.alt_down;
            self.alt_down = alt;
            changed
        };
        if fill_changed || alt_changed {
            self.present();
        }
        if self.settle_ticks == 0 && !self.hovered {
            self.stop_anim_timer();
        }
    }

    pub(super) fn ensure_anim_timer(&mut self) {
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

    pub(super) fn stop_anim_timer(&mut self) {
        if self.animating {
            self.animating = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), WINDOW_CONTROLS_ANIM_TIMER);
            }
        }
    }

    pub(super) const fn pressed_visual(&self) -> Option<WindowControlAction> {
        match self.press {
            Some(press) if press.inside => Some(press.action),
            _ => None,
        }
    }

    pub(super) fn cancel_green_hover(&mut self) {
        if self.green_hover_armed {
            self.green_hover_armed = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), GREEN_HOVER_TIMER);
            }
        }
    }

    pub(super) fn on_green_hover(&mut self, runtime: &RuntimeContext) {
        self.cancel_green_hover();
        if self.press.is_none()
            && self.hovered
            && crate::menu::active_top_bar_target(runtime).is_none()
        {
            self.open_window_menu(runtime);
        }
    }
}
