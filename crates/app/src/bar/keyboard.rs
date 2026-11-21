use crate::bar_controls::{BarControlAction, BarControlsController};
use crate::host::RuntimeContext;
use crate::notch::NotchHotkey;
use crate::win_msg::WM_APP_BAR_KEY;
use otto_core::{BarFocusOutcome, BarKey, HitTarget, build_bar_accessible_tree};
use otto_platform::{
    WM_APP_UIA_INVOKE, handle_wm_getobject, route_bar_keys, routes_keys_to, stop_routing_if_targets,
};

use super::Bar;
use super::active_notch_tab_target;
use super::is_notch_target;
use super::keyboard_seek_delta;
use super::modal::DeferredModalAction;

impl Bar {
    pub(super) fn keyboard_targets(&self) -> Vec<HitTarget> {
        let media_present = self
            .notch
            .display_media(crate::notch::unix_now_ms())
            .is_some();
        self.layout.keyboard_targets_with_media(media_present)
    }

    pub(super) fn enter_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if !self.shown() {
            return;
        }
        crate::menu::close(runtime);
        let targets = self.keyboard_targets();
        if self.focus.enter(&targets).is_some() && route_bar_keys(self.hwnd, WM_APP_BAR_KEY) {
            self.present(runtime);
        } else {
            self.focus.exit();
        }
    }

    pub(super) fn enter_from_dock(&mut self, runtime: &RuntimeContext, from_end: bool) {
        if !self.shown() {
            stop_routing_if_targets(self.hwnd);
            return;
        }
        let targets = self.keyboard_targets();
        let entered = if from_end {
            self.focus.enter_last(&targets)
        } else {
            self.focus.enter(&targets)
        };
        if entered.is_some() && route_bar_keys(self.hwnd, WM_APP_BAR_KEY) {
            self.present(runtime);
        } else {
            self.focus.exit();
            stop_routing_if_targets(self.hwnd);
        }
    }

    pub(super) fn leave_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        stop_routing_if_targets(self.hwnd);
        self.focus_notch_tab_on_expand = false;
        self.focus.exit();
        self.present(runtime);
    }

    pub(super) fn sync_keyboard_focus(&mut self, runtime: &RuntimeContext) {
        if self.focus_notch_tab_on_expand && self.layout.panel_expansion > 0.55 {
            self.focus_notch_tab_on_expand = false;
            let tab_target = active_notch_tab_target(self.notch.active_tab());
            let targets = self.keyboard_targets();
            if targets.contains(&tab_target)
                && self.focus.enter(&[tab_target]).is_some()
                && route_bar_keys(self.hwnd, WM_APP_BAR_KEY)
            {
                self.present(runtime);
                return;
            }
        }
        if !self.focus.is_active() {
            return;
        }
        if !routes_keys_to(self.hwnd) || !self.shown() {
            self.leave_keyboard_focus(runtime);
        } else if self.focus.revalidate(&self.keyboard_targets()) {
            self.present(runtime);
        }
    }

    pub(super) fn accessible_wm_getobject(
        &self,
        hwnd: isize,
        wparam: usize,
        lparam: isize,
        notch_panel_only: bool,
    ) -> Option<isize> {
        handle_wm_getobject(hwnd, wparam, lparam, WM_APP_UIA_INVOKE, || {
            let app_name = self.controls.active_app();
            let media_present = self
                .notch
                .display_media(crate::notch::unix_now_ms())
                .is_some();
            build_bar_accessible_tree(
                &self.layout,
                (0, 0),
                app_name,
                media_present,
                notch_panel_only,
            )
        })
    }

    pub(super) fn invoke_accessible_target(
        &mut self,
        runtime: &RuntimeContext,
        target: HitTarget,
    ) -> Option<DeferredModalAction> {
        self.ensure_anim_timer();
        if is_notch_target(target) {
            let modal = self.run_notch_target(runtime, target);
            self.present(runtime);
            return modal;
        }
        if let Some(action) = BarControlsController::click_action(target) {
            match action {
                BarControlAction::ControlCenter { page, trigger } => {
                    self.toggle_control_center(runtime, page, trigger, true);
                }
                other => self.perform(runtime, other),
            }
            self.present(runtime);
        }
        None
    }

    pub(super) fn on_bar_key(
        &mut self,
        runtime: &RuntimeContext,
        key: BarKey,
    ) -> Option<DeferredModalAction> {
        let targets = self.keyboard_targets();
        if let Some(from_end) = self.focus.dock_handoff(key, &targets)
            && crate::dock::keyboard_entry_count(runtime) > 0
        {
            self.focus.exit();
            crate::dock::enter_keyboard_focus(runtime, from_end);
            self.present(runtime);
            return None;
        }
        if self.focus.current() == Some(HitTarget::MediaSeekbar)
            && let Some(delta_ms) = keyboard_seek_delta(key)
        {
            self.seek_from_keyboard(runtime, delta_ms);
            return None;
        }
        match self.focus.handle(key, &targets) {
            BarFocusOutcome::Moved(_) => self.present(runtime),
            BarFocusOutcome::Exited => self.leave_keyboard_focus(runtime),
            BarFocusOutcome::Unchanged | BarFocusOutcome::Activate(HitTarget::MediaSeekbar) => {}
            BarFocusOutcome::Activate(target) if is_notch_target(target) => {
                let was_collapsed = self.layout.panel_expansion <= 0.55;
                let modal = self.run_notch_target(runtime, target);
                if matches!(target, HitTarget::Notch | HitTarget::PrivacyIndicator) && was_collapsed
                {
                    let tab_target = active_notch_tab_target(self.notch.active_tab());
                    let targets = self.keyboard_targets();
                    if self.layout.panel_expansion > 0.55 && targets.contains(&tab_target) {
                        let _ = self.focus.enter(&[tab_target]);
                        self.focus_notch_tab_on_expand = false;
                    } else if self.panel_visible {
                        let _ = self.focus.enter(&[HitTarget::Notch]);
                        self.focus_notch_tab_on_expand = true;
                    }
                } else {
                    self.focus_notch_tab_on_expand = false;
                    stop_routing_if_targets(self.hwnd);
                }
                self.present(runtime);
                return modal;
            }
            BarFocusOutcome::Activate(target) => {
                self.leave_keyboard_focus(runtime);
                self.present(runtime);
                if let Some(action) = BarControlsController::click_action(target) {
                    match action {
                        BarControlAction::ControlCenter { page, trigger } => {
                            self.toggle_control_center(runtime, page, trigger, true);
                        }
                        other => self.perform(runtime, other),
                    }
                }
            }
        }
        None
    }

    pub(super) fn on_hotkey(&mut self, runtime: &RuntimeContext, hotkey_id: i32) {
        if hotkey_id == otto_platform::HOTKEY_FOCUS_BAR {
            if self.focus.is_active() {
                self.leave_keyboard_focus(runtime);
            } else {
                self.enter_keyboard_focus(runtime);
            }
            return;
        }
        if !self.notch_enabled {
            return;
        }
        if hotkey_id == otto_platform::HOTKEY_QUICK_CAPTURE {
            self.open_inline_task_capture(runtime);
            return;
        }
        self.ensure_anim_timer();
        let Some(hotkey) = (match hotkey_id {
            otto_platform::HOTKEY_TOGGLE_NOTCH => Some(NotchHotkey::ToggleNotch),
            otto_platform::HOTKEY_TOGGLE_POMODORO => Some(NotchHotkey::TogglePomodoro),
            _ => None,
        }) else {
            return;
        };
        let update = self.notch.handle_hotkey(hotkey);
        if update.changed {
            self.recompute_layout();
            self.sync_panel_visibility();
            self.present(runtime);
        }
        self.schedule_deadline();
    }
}
