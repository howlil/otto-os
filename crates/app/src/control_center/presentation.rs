use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_CONTROL_CENTER_BACKDROP;
use otto_core::{
    BACKDROP_PADDING_PT, CaptureRegion, ControlCenterPlacement, capture_interval_for,
    capture_wanted, compute_control_center_layout_for_snapshot, control_center_focus_targets,
    scale,
};
use otto_platform::{is_running_on_battery, monitor_work_area_at, primary_screen};
use otto_render::{ControlCenterPaintState, ControlCenterRenderer};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos,
};
use windows::core::Result;

use super::ControlCenterSurface;
use crate::settings::settings_snapshot;

impl ControlCenterSurface {
    pub(super) fn ensure_renderer(&mut self) -> Result<()> {
        if self.renderer.is_none() {
            self.renderer = Some(ControlCenterRenderer::new(self.screen.dpi)?);
        }
        Ok(())
    }

    pub(super) fn sync_snapshot(&mut self, runtime: &RuntimeContext) {
        self.snapshot.status = self.port.read_system_status();
        self.snapshot.focus_active = runtime.notch_enabled.get()
            && runtime.focus_module_enabled.get()
            && runtime.focus_active.get();
        self.snapshot.media = if runtime.notch_enabled.get() && runtime.media_module_enabled.get() {
            self.port.read_now_playing()
        } else {
            None
        };
        self.snapshot.wifi = self.port.read_wifi_state();
        self.snapshot.bluetooth = self.port.read_bluetooth_state();
        self.snapshot.audio_outputs = self.port.read_audio_outputs();
        self.snapshot.settings = settings_snapshot(runtime, self.port.read_launch_at_login());
    }

    pub(super) fn relayout(&mut self) {
        let page = self.snapshot.page;
        let bounds = monitor_work_area_at(self.anchor_right, self.anchor_y);
        self.layout = compute_control_center_layout_for_snapshot(
            &self.metrics,
            ControlCenterPlacement {
                bounds,
                dpi: self.screen.dpi,
                anchor_right: self.anchor_right,
                anchor_y: self.anchor_y,
                page,
                item_count: self.snapshot.subpanel_item_count(page),
                scroll_offset: self.scroll_offset,
            },
            &self.snapshot,
        );
        self.scroll_offset = self.layout.scroll_offset;
        if self.focus.is_active() {
            let targets = control_center_focus_targets(&self.layout, &self.snapshot);
            let _ = self.focus.revalidate(&targets);
        }
    }

    pub(super) fn place(&self) {
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                self.layout.window.x,
                self.layout.window.y,
                self.layout.window.width,
                self.layout.window.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    pub(super) fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = self.visible
            && self.hwnd != 0
            && capture_wanted(&appearance, runtime.glass_enabled.get());
        if !wanted {
            if let Some(feed) = self.backdrop.as_mut()
                && feed.is_active()
            {
                feed.reset();
            }
            if let Some(renderer) = self.renderer.as_ref() {
                renderer.clear_backdrop();
            }
            return;
        }
        if self.backdrop.is_none() {
            self.backdrop = Some(BackdropFeed::start(
                self.hwnd,
                WM_APP_CONTROL_CENTER_BACKDROP,
                vec![self.hwnd],
                runtime.capture_visible.get(),
            ));
        }
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.layout.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        let interval = capture_interval_for(is_running_on_battery(), self.visible);
        feed.request(region, interval, force);
    }

    pub(super) fn prime_backdrop(&mut self, runtime: &RuntimeContext) {
        if !capture_wanted(
            &crate::theme_state::appearance(runtime),
            runtime.glass_enabled.get(),
        ) || (runtime.capture_visible.get() && otto_platform::is_surface_visible(self.hwnd))
        {
            return;
        }
        if self.ensure_renderer().is_err() {
            return;
        }
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        if renderer.has_backdrop() {
            return;
        }
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) = CaptureRegion::padded_to_screen(
            self.layout.window,
            pad,
            self.screen.width,
            self.screen.height,
        ) else {
            return;
        };
        if let Some(pixels) =
            otto_platform::capture_backdrop_now(region.origin(), region.width, region.height)
        {
            renderer.set_backdrop(
                pixels,
                (
                    region.x - self.layout.window.x,
                    region.y - self.layout.window.y,
                ),
            );
        }
    }

    pub(super) fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        let local = (
            update.origin.0 - self.layout.window.x,
            update.origin.1 - self.layout.window.y,
        );
        if self.ensure_renderer().is_ok()
            && let Some(renderer) = self.renderer.as_ref()
        {
            renderer.set_backdrop(update.pixels, local);
        }
        self.present(runtime);
    }

    pub(super) fn present(&mut self, runtime: &RuntimeContext) {
        if !self.visible || self.hwnd == 0 {
            return;
        }
        let appearance = crate::theme_state::appearance(runtime);
        if self.ensure_renderer().is_err() {
            return;
        }
        if let Some(renderer) = self.renderer.as_mut() {
            let paint_state = ControlCenterPaintState {
                hovered: self.hovered,
                pressed: self.pressed.filter(|target| Some(*target) == self.hovered),
                focused: self.focus.current(),
                morph_progress: self.morph_progress,
            };
            crate::host::log_present_error(
                "control-center",
                renderer.present(
                    self.hwnd,
                    &self.layout,
                    &self.snapshot,
                    paint_state,
                    &appearance,
                ),
            );
        }
    }

    pub(super) fn rebuild(&mut self, runtime: &RuntimeContext) {
        if !self.visible {
            return;
        }
        let prev_dpi = self.screen.dpi;
        self.screen = primary_screen();
        if self.screen.dpi != prev_dpi {
            self.renderer = None;
        }
        self.sync_snapshot(runtime);
        self.relayout();
        self.place();
        self.sync_backdrop(runtime, true);
        self.present(runtime);
    }
}
