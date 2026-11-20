use otto_core::{DockPort, dock_rest_zone};
use otto_platform::DockReservation;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos,
};

use super::Dock;

impl Dock {
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

    pub(super) fn refresh_cover(&mut self) {
        self.fullscreen_covered =
            self.port
                .fullscreen_on_primary(self.screen.width, self.screen.height, self.own_pid);
        self.window_overlaps = self.preferences.autohide
            && dock_rest_zone(&self.layout, self.screen.height)
                .is_some_and(|zone| self.port.window_overlaps_zone(zone, self.own_pid));
    }

    pub(super) fn sync_reservation(&mut self) {
        if !(self.preferences.reserve_space && self.preferences.hide_taskbar) {
            self.reservation = None;
            return;
        }
        if self.reservation.is_none() {
            self.reservation = DockReservation::register(self.hwnd);
        }
        if let Some(reservation) = self.reservation.as_mut() {
            reservation.reserve(
                self.screen.width,
                self.screen.height,
                self.layout.rest_height,
            );
        }
    }
}
