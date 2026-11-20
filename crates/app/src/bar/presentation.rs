use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use otto_core::{BACKDROP_PADDING_PT, CaptureRegion, Rect, capture_interval_for, capture_wanted};
use otto_core::{BarPointer, HitTarget, Metrics, compute_layout_with_notch, scale};
use otto_platform::is_running_on_battery;
use otto_platform::{cursor_position, primary_screen, window_bounds};
use otto_render::Renderer;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow,
};

use super::Bar;
use super::max_panel_surface_size;

impl Bar {
    pub(super) fn recompute_layout(&mut self) {
        let renderer = &self.renderer;
        if self.notch_enabled
            && self.notch.sync_compact_title_width(
                Instant::now(),
                crate::notch::unix_now_ms(),
                |text| renderer.compact_title_points(text),
            )
        {
            self.ensure_anim_timer();
        }
        let notch_input = self.notch_input();
        let settled = notch_input.panel_expansion <= 0.0 && notch_input.press_progress <= 0.0;
        self.layout = compute_layout_with_notch(
            &self.metrics,
            self.screen.width,
            self.screen.dpi,
            self.controls.active_app(),
            notch_input,
        );
        if !self.notch_enabled {
            otto_platform::withdraw_notch_rect(self.hwnd);
        } else if settled {
            let island = self.layout.notch.union(&self.layout.notch_secondary);
            otto_platform::publish_notch_rect(
                self.hwnd,
                otto_core::Rect {
                    y: self.layout.notch.y,
                    height: self.layout.notch.height,
                    ..island
                },
            );
        }
        self.sync_bar_shelf_region();
    }

    pub(super) fn on_shell_notification(
        &mut self,
        runtime: &RuntimeContext,
        notification: otto_platform::ShellNotification,
    ) {
        match notification {
            otto_platform::ShellNotification::PositionChanged => {
                if self.top_bar_enabled {
                    self.appbar.reserve_top(
                        self.screen.width,
                        self.screen.height,
                        self.layout.bar.height,
                    );
                }
            }
            otto_platform::ShellNotification::FullScreenApp(active) => {
                self.shell_full_screen = active;
                self.sync_full_screen(runtime);
            }
        }
    }

    pub(super) fn foreground_covers_primary(&self) -> bool {
        self.controls.active_window().is_some_and(|window| {
            window.full_screen
                && window_bounds(window.identity.hwnd).is_some_and(|frame| {
                    frame.x <= 0
                        && frame.y <= 0
                        && frame.right() >= self.screen.width
                        && frame.bottom() >= self.screen.height
                })
        })
    }

    pub(super) fn sync_full_screen(&mut self, runtime: &RuntimeContext) {
        let active = self.shell_full_screen || self.foreground_covers_primary();
        if self.visibility.set_full_screen_app(active) {
            self.apply_visibility(runtime);
        }
    }

    pub(super) fn track_reveal(&mut self, runtime: &RuntimeContext) {
        let (x, y) = cursor_position();
        let pointer = BarPointer {
            on_bar_monitor: x >= 0 && x < self.screen.width && y >= 0 && y < self.screen.height,
            y,
        };
        let popup_open = crate::menu::active_top_bar_target(runtime).is_some()
            || crate::control_center::is_open(runtime)
            || crate::menu::is_calendar_open(runtime)
            || crate::menu::is_notification_open(runtime);
        if self
            .visibility
            .track_pointer(pointer, self.layout.bar.height, popup_open)
        {
            self.apply_visibility(runtime);
        }
    }

    pub(super) fn apply_visibility(&mut self, runtime: &RuntimeContext) {
        self.notch.set_presented(self.notch_enabled && self.shown());
        if !self.visibility.is_visible() {
            crate::menu::close(runtime);
        }
        if self.shown() {
            self.place();
            self.present(runtime);
            self.schedule_deadline();
            return;
        }
        self.schedule_deadline();
        crate::menu::close(runtime);
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            let _ = ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE);
        }
        self.panel_visible = false;
        self.last_panel_rect = None;
    }

    pub(super) const fn top_bar_chrome_visible(&self) -> bool {
        self.top_bar_enabled && self.visibility.is_visible()
    }

    pub(super) fn shown(&self) -> bool {
        if self.top_bar_enabled {
            self.visibility.is_visible() || (self.notch_enabled && self.menu_prefs.keep_notch)
        } else {
            self.notch_enabled && self.visibility.is_visible()
        }
    }

    pub(super) fn place(&self) {
        if !self.shown() {
            return;
        }
        let _ = unsafe {
            SetWindowPos(
                HWND(self.hwnd as *mut _),
                Some(HWND_TOPMOST),
                0,
                0,
                self.layout.bar.width,
                self.layout.bar.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
    }

    pub(super) fn sync_panel_visibility(&mut self) {
        let should_show =
            self.notch_enabled && self.notch.panel_expansion() > 0.005 && self.shown();
        if should_show {
            if !self.panel_visible || self.last_panel_rect != Some(self.layout.panel_window) {
                let _ = unsafe {
                    SetWindowPos(
                        HWND(self.panel_hwnd as *mut _),
                        Some(HWND_TOPMOST),
                        self.layout.panel_window.x,
                        self.layout.panel_window.y,
                        self.layout.panel_window.width,
                        self.layout.panel_window.height,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    )
                };
                self.last_panel_rect = Some(self.layout.panel_window);
            }
            self.panel_visible = true;
        } else if self.panel_visible {
            let _ = unsafe { ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE) };
            self.panel_visible = false;
            self.last_panel_rect = None;
        }
    }

    pub(super) fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.rebuild_with_metrics(runtime, Metrics::for_style(runtime.notch_style()));
    }

    pub(super) fn rebuild_with_metrics(&mut self, runtime: &RuntimeContext, metrics: Metrics) {
        self.last_panel_rect = None;
        self.screen = primary_screen();
        self.metrics = metrics;
        self.recompute_layout();
        let (max_panel_w, max_panel_h) = max_panel_surface_size(self.screen.dpi);
        if let Ok(renderer) = Renderer::new(
            self.layout.bar.width,
            self.layout.bar.height,
            max_panel_w,
            max_panel_h,
            self.screen.dpi,
            0.0,
        ) {
            renderer.set_brand_icon(runtime.brand_icon());
            self.renderer = renderer;
            if let Some(feed) = self.backdrop.as_mut() {
                feed.reset();
            }
        }
        self.appbar.rebase(self.screen.width, self.screen.height);
        if self.top_bar_enabled {
            self.appbar.reserve_top(
                self.screen.width,
                self.screen.height,
                self.layout.bar.height,
            );
        }
        self.sync_backdrop(runtime, true);
        self.place();
        self.present(runtime);
        self.ensure_anim_timer();
    }

    pub(super) fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = capture_wanted(&appearance, self.backdrop.is_some())
            && self.top_bar_enabled
            && self.menu_prefs.show_background;
        let visible = self.top_bar_chrome_visible();
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
        if !wanted {
            if feed.is_active() {
                feed.reset();
                self.renderer.clear_bar_backdrop();
                self.present(runtime);
            }
            return;
        }
        if !visible {
            return;
        }
        let bar_rect = Rect {
            x: 0,
            y: 0,
            width: self.layout.bar.width,
            height: self.layout.bar.height,
        };
        let pad = scale(BACKDROP_PADDING_PT, self.screen.dpi);
        let Some(region) =
            CaptureRegion::padded_to_screen(bar_rect, pad, self.screen.width, self.screen.height)
        else {
            return;
        };
        if !self.renderer.has_bar_backdrop()
            && let Some(pixels) =
                otto_platform::capture_backdrop_now(region.origin(), region.width, region.height)
        {
            self.renderer.set_bar_backdrop(pixels, region.origin());
        }
        let active = (self.panel_visible || self.hovered.is_some()) && feed.recently_changed();
        let interval = capture_interval_for(is_running_on_battery(), active);
        feed.request(region, interval, force);
    }

    pub(super) fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        self.renderer.set_bar_backdrop(update.pixels, update.origin);
        self.present(runtime);
    }

    pub(super) fn present(&mut self, runtime: &RuntimeContext) {
        runtime
            .focus_active
            .set(self.notch_enabled && self.notch.focus_active());
        if !self.shown() {
            return;
        }
        let hovered = self
            .hovered
            .or_else(|| crate::menu::active_top_bar_target(runtime))
            .or_else(|| {
                crate::menu::is_notification_open(runtime).then_some(HitTarget::Notifications)
            })
            .or_else(|| crate::menu::is_calendar_open(runtime).then_some(HitTarget::Clock));
        self.renderer
            .set_bar_background(self.top_bar_enabled && self.menu_prefs.show_background);
        self.renderer
            .set_bar_chrome_hidden(!self.top_bar_chrome_visible());
        let appearance = crate::theme_state::appearance(runtime);
        let media = self
            .notch
            .media_scene(Instant::now(), crate::notch::unix_now_ms());
        let status = self.controls.status();
        let pomodoro = self.notch.pomodoro_scene(Instant::now());
        let privacy = self.notch.privacy_state();
        crate::host::log_present_error(
            "bar",
            self.renderer.present(
                self.hwnd,
                (0, 0),
                &self.layout,
                status,
                self.controls.active_app(),
                hovered,
                self.focus.current(),
                self.notch.hud_state(),
                &pomodoro,
                media.as_ref(),
                self.notch.anim_phase(),
                appearance,
                privacy,
                self.notch.call_scene(Instant::now()).as_ref(),
            ),
        );
        if self.panel_visible {
            crate::host::log_present_error(
                "bar-panel",
                self.renderer.present_panel(
                    self.panel_hwnd,
                    &self.layout,
                    hovered,
                    self.focus.current(),
                    &pomodoro,
                    media.as_ref(),
                    self.notch.anim_phase(),
                    self.notch.lyrics_view(),
                    self.notch.shelf_state(),
                    self.notch.clipboard_state(),
                    self.notch.tasks_state(),
                    privacy,
                    self.notch
                        .notification_presentation(Instant::now())
                        .as_ref(),
                ),
            );
        }
    }
}
