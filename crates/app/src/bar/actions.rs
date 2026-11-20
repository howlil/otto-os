use crate::bar_controls::BarControlAction;
use crate::host::RuntimeContext;
use crate::menu::TopBarMenuRequest;
use otto_core::{ControlCenterAction, ControlCenterPage, HitTarget};
use otto_platform::{
    TrayOverflowOutcome, close_tray_overflow, signal_command_failure, toggle_tray_overflow,
};

use super::Bar;

impl Bar {
    pub(super) fn open_menu(
        &self,
        runtime: &RuntimeContext,
        target: HitTarget,
        anchor_x: Option<i32>,
    ) {
        let Some(slot) = self.layout.menu_anchor_rect(target) else {
            return;
        };
        crate::menu::open_top_bar_menu(
            runtime,
            &TopBarMenuRequest {
                bar_hwnd: self.hwnd,
                target,
                anchor_x: anchor_x.unwrap_or(slot.x),
                anchor_y: self.layout.popup_anchor_y(self.screen.dpi),
                app_name: self.controls.active_app(),
                window: self.controls.active_window(),
                hidden: &self.layout.slots.hidden_targets,
            },
        );
    }

    pub(super) fn toggle_control_center(
        &self,
        runtime: &RuntimeContext,
        page: ControlCenterPage,
        trigger: HitTarget,
        focused: bool,
    ) {
        if !self.control_center_enabled {
            let action = match page {
                ControlCenterPage::Wifi => Some(ControlCenterAction::OpenWifi),
                ControlCenterPage::Sound => Some(ControlCenterAction::OpenSound),
                _ => None,
            };
            if let Some(action) = action {
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                crate::control_center::open_settings_action(action);
            }
            return;
        }
        let _ = close_tray_overflow();
        if crate::control_center::is_open(runtime)
            && crate::control_center::active_page(runtime) == Some(page)
        {
            crate::control_center::close(runtime);
            return;
        }
        crate::menu::close(runtime);
        let anchor_right = self.anchor_right(trigger);
        let anchor_y = self.layout.popup_anchor_y(self.screen.dpi);
        if focused {
            crate::control_center::open_at_page_focused(
                runtime,
                page,
                anchor_right,
                anchor_y,
                self.motion,
            );
        } else {
            crate::control_center::open_at_page(runtime, page, anchor_right, anchor_y, self.motion);
        }
    }

    pub(super) fn anchor_right(&self, target: HitTarget) -> i32 {
        self.layout
            .menu_anchor_rect(target)
            .map_or(self.screen.width, |slot| slot.right())
    }

    pub(super) fn perform(&mut self, runtime: &RuntimeContext, action: BarControlAction) {
        match action {
            BarControlAction::TopBarMenu(target) => {
                let _ = close_tray_overflow();
                if crate::menu::active_top_bar_target(runtime) == Some(target) {
                    crate::menu::close(runtime);
                } else {
                    self.open_menu(runtime, target, None);
                }
                self.present(runtime);
            }
            BarControlAction::Tray => {
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                let anchor = self
                    .layout
                    .menu_anchor_rect(HitTarget::Tray)
                    .unwrap_or(self.layout.slots.tray_slot);
                if toggle_tray_overflow(
                    anchor.x + anchor.width / 2,
                    self.layout.bar.height,
                    self.screen.width,
                ) == TrayOverflowOutcome::Unavailable
                {
                    signal_command_failure();
                }
            }
            BarControlAction::ControlCenter { page, trigger } => {
                self.toggle_control_center(runtime, page, trigger, false);
            }
            BarControlAction::OpenSettings(action) => {
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                crate::control_center::open_settings_action(action);
            }
            BarControlAction::Search => {
                self.leave_keyboard_focus(runtime);
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                let trigger = (self.layout.slots.search_slot.width > 0)
                    .then_some(self.layout.slots.search_slot);
                crate::spotlight::open_search(runtime, trigger);
            }
            BarControlAction::Notifications => {
                let _ = close_tray_overflow();
                if crate::menu::is_notification_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    crate::menu::open_notifications(
                        runtime,
                        self.anchor_right(HitTarget::Notifications),
                        self.layout.popup_anchor_y(self.screen.dpi),
                    );
                }
            }
            BarControlAction::Clock => {
                let _ = close_tray_overflow();
                if crate::menu::is_calendar_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    crate::menu::open_calendar(
                        runtime,
                        self.anchor_right(HitTarget::Clock),
                        self.layout.popup_anchor_y(self.screen.dpi),
                    );
                }
            }
            BarControlAction::AdjustVolume(_) | BarControlAction::StepBrightness(_) => {}
        }
    }
}
