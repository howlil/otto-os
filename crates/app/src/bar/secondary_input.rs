use crate::backdrop_feed::BackdropFeed;
use crate::bar_controls::{BarControlAction, BarControlsController};
use crate::host::RuntimeContext;
use crate::menu::TopBarMenuRequest;
use crate::win_msg::{WM_APP_BAR_BACKDROP, WM_GETOBJECT, WM_MOUSELEAVE};
use otto_core::{
    ControlCenterAction, ControlCenterPage, HitTarget, NotchHudKind, Rect,
    build_bar_accessible_tree, decode_bar_hit_target, is_top_bar_menu_target,
};
use otto_platform::{
    ShellNotification, TrayOverflowOutcome, WM_APP_UIA_INVOKE, adjust_volume, close_tray_overflow,
    handle_wm_getobject, hide_window, read_system_status, signal_command_failure,
    step_display_brightness, toggle_tray_overflow_in_bounds, track_mouse_leave,
};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MA_NOACTIVATE, WM_CLOSE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONUP,
};

use super::{Bar, unpack_point, with_bar, with_bar_result};

fn handle_secondary_pointer_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match message {
        WM_MOUSEMOVE => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| {
                bar.on_secondary_mouse_move(runtime, hwnd, x, y);
            });
            Some(LRESULT(0))
        }
        WM_MOUSELEAVE => {
            with_bar(runtime, |bar| bar.on_secondary_mouse_leave(runtime, hwnd));
            Some(LRESULT(0))
        }
        WM_LBUTTONDOWN => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| {
                bar.on_secondary_left_click(runtime, hwnd, x, y);
            });
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| {
                bar.on_secondary_right_click(runtime, hwnd, x, y);
            });
            Some(LRESULT(0))
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let (x, y) = unpack_point(lparam);
            with_bar(runtime, |bar| {
                bar.on_secondary_mouse_wheel(runtime, hwnd, x, y, delta);
            });
            Some(LRESULT(0))
        }
        _ => None,
    }
}

pub(super) fn handle_secondary_message(
    runtime: &RuntimeContext,
    hwnd: isize,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    if let Some(result) = handle_secondary_pointer_message(runtime, hwnd, message, wparam, lparam) {
        return Some(result);
    }
    match message {
        WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
        WM_CLOSE => {
            hide_window(hwnd);
            Some(LRESULT(0))
        }
        WM_APP_BAR_BACKDROP => {
            with_bar(runtime, |bar| bar.on_secondary_backdrop(runtime, hwnd));
            Some(LRESULT(0))
        }
        otto_platform::APPBAR_CALLBACK_MESSAGE => {
            if let Some(notification) =
                otto_platform::classify_shell_notification(wparam.0, lparam.0)
            {
                with_bar(runtime, |bar| {
                    bar.on_secondary_shell_notification(runtime, hwnd, notification);
                });
            }
            Some(LRESULT(0))
        }
        WM_GETOBJECT => with_bar_result(runtime, |bar| {
            bar.secondary_wm_getobject(hwnd, wparam.0, lparam.0)
        })
        .flatten()
        .map(LRESULT),
        WM_APP_UIA_INVOKE => {
            if let Some(target) = i32::try_from(wparam.0).ok().and_then(decode_bar_hit_target) {
                with_bar(runtime, |bar| {
                    if let Some(index) = bar.secondary_index(hwnd)
                        && let Some(action) = BarControlsController::click_action(target)
                    {
                        bar.perform_on_secondary(runtime, index, action);
                    }
                });
            }
            Some(LRESULT(0))
        }
        WM_DPICHANGED => {
            with_bar(runtime, |bar| bar.sync_secondary_bars(runtime));
            Some(LRESULT(0))
        }
        WM_DISPLAYCHANGE => Some(LRESULT(0)),
        _ => None,
    }
}

impl Bar {
    fn on_secondary_backdrop(&mut self, runtime: &RuntimeContext, hwnd: isize) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let Some(update) = self.secondary_bars[index]
            .backdrop
            .as_mut()
            .and_then(BackdropFeed::poll)
        else {
            return;
        };
        let origin = self.secondary_bars[index].monitor.origin();
        if let Some(renderer) = self.secondary_bars[index].renderer.as_ref() {
            renderer.set_bar_backdrop(
                update.pixels,
                (update.origin.0 - origin.0, update.origin.1 - origin.1),
            );
            self.present_secondary(runtime, index);
        }
    }

    fn on_secondary_shell_notification(
        &mut self,
        runtime: &RuntimeContext,
        hwnd: isize,
        notification: ShellNotification,
    ) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        match notification {
            ShellNotification::PositionChanged => {
                if self.top_bar_enabled {
                    let bounds = self.secondary_bars[index].monitor.bounds;
                    let height = self.secondary_bars[index].layout.bar.height;
                    if let Some(res) = self.secondary_bars[index].reservation.as_mut() {
                        let _ = res.reapply(bounds, height);
                    }
                    self.secondary_bars[index].place();
                }
            }
            ShellNotification::FullScreenApp(active) => {
                self.secondary_bars[index].shell_full_screen = active;
                self.sync_secondary_full_screen(runtime);
            }
        }
    }

    fn on_secondary_mouse_move(&mut self, runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        if !self.secondary_bars[index].tracking_mouse && track_mouse_leave(hwnd) {
            self.secondary_bars[index].tracking_mouse = true;
        }
        let target = self.secondary_bars[index].layout.hit_test(x, y);
        if let Some(open_target) = crate::menu::active_top_bar_target(runtime)
            && let Some(hit) = target
            && is_top_bar_menu_target(hit)
            && hit != open_target
        {
            self.open_secondary_menu(runtime, index, hit, None);
        }
        if target != self.secondary_bars[index].hovered {
            self.secondary_bars[index].hovered = target;
            self.present_secondary(runtime, index);
        }
    }

    fn on_secondary_mouse_leave(&mut self, runtime: &RuntimeContext, hwnd: isize) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let sec = &mut self.secondary_bars[index];
        sec.tracking_mouse = false;
        if sec.hovered.take().is_some() {
            self.present_secondary(runtime, index);
        }
    }

    fn on_secondary_left_click(&mut self, runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32) {
        self.leave_keyboard_focus(runtime);
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let Some(target) = self.secondary_bars[index].layout.hit_test(x, y) else {
            return;
        };
        if let Some(action) = BarControlsController::click_action(target) {
            self.perform_on_secondary(runtime, index, action);
        }
    }

    fn on_secondary_right_click(&mut self, runtime: &RuntimeContext, hwnd: isize, x: i32, y: i32) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        if !self.top_bar_enabled || !self.secondary_bars[index].visibility.is_visible() {
            return;
        }
        let hit = self.secondary_bars[index].layout.hit_test(x, y);
        let anchor_local = if hit == Some(HitTarget::Start) {
            self.secondary_bars[index].layout.slots.start_slot.x
        } else {
            x
        };
        let anchor_x = self.secondary_bars[index].monitor.bounds.x + anchor_local;
        self.open_secondary_menu(runtime, index, HitTarget::Start, Some(anchor_x));
        self.present_secondary(runtime, index);
    }

    fn on_secondary_mouse_wheel(
        &mut self,
        runtime: &RuntimeContext,
        hwnd: isize,
        x: i32,
        y: i32,
        delta: i16,
    ) {
        let Some(index) = self.secondary_index(hwnd) else {
            return;
        };
        let hit = self.secondary_bars[index]
            .hovered
            .or_else(|| self.secondary_bars[index].layout.hit_test(x, y));
        let Some(target) = hit else {
            return;
        };
        let Some(action) = BarControlsController::wheel_action(target, delta, false) else {
            return;
        };
        match action {
            BarControlAction::AdjustVolume(delta) => {
                adjust_volume(delta);
                let latest = read_system_status();
                self.controls.apply_status(latest);
                self.notch.trigger_hud(NotchHudKind::Volume {
                    percent: if latest.muted { 0 } else { latest.volume },
                    muted: latest.muted,
                });
            }
            BarControlAction::StepBrightness(delta) => {
                if let Some(pct) = step_display_brightness(delta) {
                    let mut status = self.controls.status();
                    status.brightness = Some(pct);
                    self.controls.apply_status(status);
                    self.notch
                        .trigger_hud(NotchHudKind::Brightness { percent: pct });
                }
            }
            _ => return,
        }
        self.ensure_anim_timer();
        self.present(runtime);
    }

    fn open_secondary_menu(
        &self,
        runtime: &RuntimeContext,
        index: usize,
        target: HitTarget,
        anchor_x: Option<i32>,
    ) {
        let sec = &self.secondary_bars[index];
        let Some(slot) = sec.layout.menu_anchor_rect(target) else {
            return;
        };
        crate::menu::open_top_bar_menu(
            runtime,
            &TopBarMenuRequest {
                bar_hwnd: sec.hwnd,
                target,
                anchor_x: anchor_x.unwrap_or(sec.monitor.bounds.x + slot.x),
                anchor_y: sec.popup_anchor_y(),
                app_name: self.controls.active_app(),
                window: self.controls.active_window(),
                hidden: &sec.layout.slots.hidden_targets,
            },
        );
    }

    fn toggle_secondary_control_center(
        &self,
        runtime: &RuntimeContext,
        index: usize,
        page: ControlCenterPage,
        trigger: HitTarget,
    ) {
        if !self.control_center_enabled {
            let fallback = match page {
                ControlCenterPage::Wifi => Some(ControlCenterAction::OpenWifi),
                ControlCenterPage::Sound => Some(ControlCenterAction::OpenSound),
                _ => None,
            };
            if let Some(cc_action) = fallback {
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                crate::control_center::open_settings_action(cc_action);
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
        let sec = &self.secondary_bars[index];
        crate::control_center::open_at_page(
            runtime,
            page,
            sec.anchor_right(trigger),
            sec.popup_anchor_y(),
            self.motion,
        );
    }

    fn perform_on_secondary(
        &mut self,
        runtime: &RuntimeContext,
        index: usize,
        action: BarControlAction,
    ) {
        match action {
            BarControlAction::TopBarMenu(target) => {
                let _ = close_tray_overflow();
                if crate::menu::active_top_bar_target(runtime) == Some(target) {
                    crate::menu::close(runtime);
                } else {
                    self.open_secondary_menu(runtime, index, target, None);
                }
                self.present_secondary(runtime, index);
            }
            BarControlAction::Tray => {
                crate::spotlight::close(runtime);
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                let sec = &self.secondary_bars[index];
                let anchor = sec
                    .layout
                    .menu_anchor_rect(HitTarget::Tray)
                    .unwrap_or(sec.layout.slots.tray_slot);
                let center_x = sec.monitor.bounds.x + anchor.x + anchor.width / 2;
                let bar_bottom = sec.monitor.bounds.y + sec.layout.bar.height;
                if toggle_tray_overflow_in_bounds(center_x, bar_bottom, sec.monitor.bounds)
                    == TrayOverflowOutcome::Unavailable
                {
                    signal_command_failure();
                }
            }
            BarControlAction::ControlCenter { page, trigger } => {
                self.toggle_secondary_control_center(runtime, index, page, trigger);
            }
            BarControlAction::OpenSettings(cc_action) => {
                let _ = close_tray_overflow();
                crate::spotlight::close(runtime);
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                crate::control_center::open_settings_action(cc_action);
            }
            BarControlAction::Search => {
                self.leave_keyboard_focus(runtime);
                let _ = close_tray_overflow();
                crate::menu::close(runtime);
                crate::control_center::close(runtime);
                let sec = &self.secondary_bars[index];
                let slot = sec.layout.slots.search_slot;
                let trigger = (slot.width > 0).then_some(Rect {
                    x: sec.monitor.bounds.x + slot.x,
                    y: sec.monitor.bounds.y + slot.y,
                    width: slot.width,
                    height: slot.height,
                });
                crate::spotlight::toggle_search(runtime, trigger);
            }
            BarControlAction::Notifications => {
                let _ = close_tray_overflow();
                if crate::menu::is_notification_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    let sec = &self.secondary_bars[index];
                    crate::menu::open_notifications(
                        runtime,
                        sec.anchor_right(HitTarget::Notifications),
                        sec.popup_anchor_y(),
                    );
                }
            }
            BarControlAction::Clock => {
                let _ = close_tray_overflow();
                if crate::menu::is_calendar_open(runtime) {
                    crate::menu::close(runtime);
                } else {
                    let sec = &self.secondary_bars[index];
                    crate::menu::open_calendar(
                        runtime,
                        sec.anchor_right(HitTarget::Clock),
                        sec.popup_anchor_y(),
                    );
                }
            }
            BarControlAction::AdjustVolume(_) | BarControlAction::StepBrightness(_) => {}
        }
    }

    fn secondary_wm_getobject(&self, hwnd: isize, wparam: usize, lparam: isize) -> Option<isize> {
        let index = self.secondary_index(hwnd)?;
        let sec = &self.secondary_bars[index];
        let origin = sec.monitor.origin();
        handle_wm_getobject(hwnd, wparam, lparam, WM_APP_UIA_INVOKE, || {
            build_bar_accessible_tree(
                &sec.layout,
                origin,
                self.controls.active_app(),
                false,
                false,
            )
        })
    }
}
