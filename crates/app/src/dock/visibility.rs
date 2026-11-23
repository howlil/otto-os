use crate::host::RuntimeContext;
use otto_core::{DockPort, dock_rest_zone};
use otto_platform::{DockReservation, ShellNotification};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos,
};

use super::Dock;

pub(super) const fn should_assert_topmost(should_hide: bool, higher_overlay_open: bool) -> bool {
    !should_hide && !higher_overlay_open
}

impl Dock {
    pub(super) const fn is_fullscreen_covered(&self) -> bool {
        self.fullscreen_covered || self.shell_full_screen
    }

    pub(super) fn higher_overlay_open(runtime: &RuntimeContext) -> bool {
        crate::menu::is_open(runtime)
            || crate::control_center::is_open(runtime)
            || crate::spotlight::is_open(runtime)
            || crate::focus_shield::is_open(runtime)
    }

    pub(super) fn place(&self) {
        let (ox, oy) = self.window_origin();
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                ox,
                oy,
                self.layout.window_width,
                self.layout.window_height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    pub(super) fn ensure_topmost(&self, runtime: &RuntimeContext) {
        if !should_assert_topmost(self.anim.should_hide, Self::higher_overlay_open(runtime)) {
            return;
        }
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }

    pub(super) fn refresh_cover(&mut self) {
        self.fullscreen_covered =
            self.port
                .fullscreen_on_primary(self.screen.width, self.screen.height, self.own_pid);
        self.window_overlaps = self.preferences.autohide
            && dock_rest_zone(&self.layout, self.screen.height)
                .is_some_and(|zone| self.port.window_overlaps_zone(zone, self.own_pid));
    }

    pub(super) fn sync_reservation(&mut self) -> bool {
        if !(self.preferences.reserve_space && self.preferences.hide_taskbar) {
            self.reservation = None;
            self.shell_full_screen = false;
            return false;
        }
        let mut updated = false;
        if self.reservation.is_none() {
            self.reservation = DockReservation::register(self.hwnd);
            updated = self.reservation.is_some();
        }
        if let Some(reservation) = self.reservation.as_mut() {
            updated |= reservation.reserve(
                self.screen.width,
                self.screen.height,
                self.layout.rest_height,
            );
        }
        updated
    }

    pub(super) fn on_shell_notification(
        &mut self,
        runtime: &RuntimeContext,
        notification: ShellNotification,
    ) {
        match notification {
            ShellNotification::PositionChanged => {
                if let Some(reservation) = self.reservation.as_mut() {
                    let _ = reservation.reapply(
                        self.screen.width,
                        self.screen.height,
                        self.layout.rest_height,
                    );
                }
                if should_assert_topmost(self.anim.should_hide, Self::higher_overlay_open(runtime))
                {
                    self.place();
                }
            }
            ShellNotification::FullScreenApp(active) => {
                self.shell_full_screen = active;
                self.refresh_cover();
                self.sync_hide_from_pointer(runtime);
                if !active {
                    self.ensure_topmost(runtime);
                }
            }
        }
    }

    pub(super) fn on_taskbar_created(&mut self, runtime: &RuntimeContext) {
        self.reservation = None;
        self.shell_full_screen = false;
        let _ = self.sync_reservation();
        if should_assert_topmost(self.anim.should_hide, Self::higher_overlay_open(runtime)) {
            self.place();
        }
    }
}
