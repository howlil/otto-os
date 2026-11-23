use std::time::Instant;

use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_BAR_BACKDROP;
use otto_core::{BACKDROP_PADDING_PT, CaptureRegion, Rect, capture_interval_for, capture_wanted};
use otto_core::{BarPointer, HitTarget, Metrics, compute_layout_with_notch, scale};
use otto_platform::is_running_on_battery;
use otto_platform::{cursor_position, primary_screen, watch_top_edge, window_bounds};
use otto_render::Renderer;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow,
};

use super::Bar;
use super::max_panel_surface_size;

impl Bar {
    pub(super) fn ensure_renderer(&mut self, runtime: &RuntimeContext) -> bool {
        if self.renderer.is_some() {
            return true;
        }
        if !self.top_bar_enabled && !self.notch_enabled {
            return false;
        }
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
            self.renderer = Some(renderer);
            self.stats_icons_attempted.clear();
        }
        self.renderer.is_some()
    }

    pub(super) fn release_renderer_if_unused(&mut self, runtime: &RuntimeContext) {
        if self.top_bar_enabled || self.notch_enabled {
            return;
        }
        self.stop_anim_timer();
        self.stop_status_timer();
        watch_top_edge(false, self.screen.width, self.screen.height);
        self.backdrop = None;
        self.io.stop_lyrics();
        self.renderer = None;
        self.stats_icons_attempted.clear();
        self.panel_visible = false;
        self.last_panel_rect = None;
        unsafe {
            let _ = ShowWindow(HWND(self.hwnd as *mut _), SW_HIDE);
            let _ = ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE);
        }
        if !crate::control_center::is_open(runtime) {
            otto_platform::stop_idle_audio_task();
            otto_platform::stop_idle_connectivity_task();
        }
    }

    pub(super) fn recompute_layout(&mut self) {
        if self.notch_enabled
            && let Some(renderer) = self.renderer.as_ref()
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
        self.notch.cancel_task_editor();
        self.io.hide_quick_capture(false);
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
            self.sync_quick_capture_overlay();
        } else if self.panel_visible {
            self.io.hide_quick_capture(false);
            let _ = unsafe { ShowWindow(HWND(self.panel_hwnd as *mut _), SW_HIDE) };
            self.panel_visible = false;
            self.last_panel_rect = None;
            if let Some(renderer) = self.renderer.as_ref() {
                renderer.release_panel_surface();
            }
        } else if self.io.is_quick_capture_visible() {
            self.io.hide_quick_capture(false);
        }
    }

    pub(super) fn rebuild(&mut self, runtime: &RuntimeContext) {
        self.rebuild_with_metrics(runtime, Metrics::for_style(runtime.notch_style()));
    }

    pub(super) fn rebuild_with_metrics(&mut self, runtime: &RuntimeContext, metrics: Metrics) {
        self.last_panel_rect = None;
        let previous_screen = self.screen;
        let carried_backdrop = self.renderer.as_ref().and_then(Renderer::take_bar_backdrop);
        self.screen = primary_screen();
        self.metrics = metrics;
        self.recompute_layout();
        if self.top_bar_enabled || self.notch_enabled {
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
                if self.screen.width == previous_screen.width
                    && self.screen.height == previous_screen.height
                    && let Some((pixels, origin)) = carried_backdrop
                {
                    renderer.set_bar_backdrop(pixels, origin);
                }
                self.renderer = Some(renderer);
                self.stats_icons_attempted.clear();
                if let Some(feed) = self.backdrop.as_mut() {
                    feed.reset();
                }
            }
        } else {
            self.renderer = None;
            self.stats_icons_attempted.clear();
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
        if self.top_bar_enabled || self.notch_enabled {
            self.ensure_anim_timer();
        }
    }

    pub(super) fn sync_backdrop(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = capture_wanted(&appearance, runtime.glass_enabled.get())
            && self.top_bar_enabled
            && self.menu_prefs.show_background;
        let visible = self.top_bar_chrome_visible();
        if !wanted {
            let had_feed = self.backdrop.take().is_some();
            let had_backdrop = self
                .renderer
                .as_ref()
                .is_some_and(Renderer::has_bar_backdrop);
            if let Some(renderer) = self.renderer.as_ref() {
                renderer.clear_bar_backdrop();
            }
            if (had_feed || had_backdrop) && self.shown() {
                self.present(runtime);
            }
            return;
        }
        if !visible {
            return;
        }
        if self.backdrop.is_none() {
            self.backdrop = Some(BackdropFeed::start(
                self.hwnd,
                WM_APP_BAR_BACKDROP,
                vec![self.hwnd, self.panel_hwnd],
                runtime.capture_visible.get(),
            ));
        }
        let Some(feed) = self.backdrop.as_mut() else {
            return;
        };
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
        if let Some(renderer) = self.renderer.as_ref()
            && !renderer.has_bar_backdrop()
        {
            let can_prime_screen =
                !runtime.capture_visible.get() || !otto_platform::is_surface_visible(self.hwnd);
            let pixels =
                otto_platform::capture_wallpaper_now(region.origin(), region.width, region.height)
                    .or_else(|| {
                        can_prime_screen
                            .then(|| {
                                otto_platform::capture_backdrop_now(
                                    region.origin(),
                                    region.width,
                                    region.height,
                                )
                            })
                            .flatten()
                    });
            if let Some(pixels) = pixels {
                renderer.set_bar_backdrop(pixels, region.origin());
            }
        }
        let active = (self.panel_visible || self.hovered.is_some()) && feed.recently_changed();
        let interval = capture_interval_for(is_running_on_battery(), active);
        feed.request(region, interval, force);
    }

    pub(super) fn on_backdrop(&mut self, runtime: &RuntimeContext) {
        let Some(update) = self.backdrop.as_mut().and_then(BackdropFeed::poll) else {
            return;
        };
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.set_bar_backdrop(update.pixels, update.origin);
            self.present(runtime);
        }
    }

    pub(super) fn remember_app_icon_hint(&mut self, app_key: &str, executable: &str) {
        if executable.is_empty() {
            return;
        }
        let normalized = otto_core::normalize_app_key(app_key);
        if normalized.is_empty() {
            return;
        }
        if self.stats_icon_hints.len() >= 64 && !self.stats_icon_hints.contains_key(&normalized) {
            self.stats_icon_hints.clear();
        }
        self.stats_icon_hints
            .insert(normalized, executable.to_owned());
    }

    fn resolve_stats_app_icon(&mut self, key: &str) -> Option<otto_core::IconBitmap> {
        if let Some(exe) = self.stats_icon_hints.get(key)
            && let Some(bmp) = otto_platform::extract_app_icon_bitmap(exe, Some(exe.as_str()))
        {
            return Some(bmp);
        }
        let catalog = self.stats_catalog.get_or_insert_with(|| {
            otto_platform::collect_focus_catalog_apps()
                .into_iter()
                .map(|entry| (entry.block_key, (entry.icon_source, entry.detail)))
                .collect()
        });
        if let Some((icon_source, detail)) = catalog.get(key)
            && let Some(bmp) = otto_platform::extract_app_icon_bitmap(
                icon_source,
                (!detail.is_empty()).then_some(detail.as_str()),
            )
        {
            return Some(bmp);
        }
        otto_platform::extract_app_icon_bitmap(key, None)
    }

    fn sync_stats_app_icons(&mut self, stats: &otto_core::FocusStatsView) {
        let needs_any = stats.top_apps.iter().any(|app| {
            !app.app_key.is_empty()
                && !self.stats_icons_attempted.contains(&app.app_key)
                && self
                    .renderer
                    .as_ref()
                    .is_some_and(|r| !r.has_stats_app_icon(&app.app_key))
        });
        if !needs_any {
            return;
        }
        for app in &stats.top_apps {
            let key = app.app_key.as_str();
            if key.is_empty()
                || self
                    .renderer
                    .as_ref()
                    .is_none_or(|r| r.has_stats_app_icon(key))
                || !self.stats_icons_attempted.insert(app.app_key.clone())
            {
                continue;
            }
            if let Some(bmp) = self.resolve_stats_app_icon(key)
                && let Some(renderer) = self.renderer.as_ref()
            {
                let _ = renderer.set_stats_app_icon(
                    app.app_key.clone(),
                    bmp.width,
                    bmp.height,
                    &bmp.pixels,
                );
            }
        }
    }

    fn resolve_focus_stats(
        &self,
        runtime: &RuntimeContext,
        hovered: Option<HitTarget>,
    ) -> otto_core::FocusStatsView {
        let stats_visible = self.panel_visible
            && (self.notch.active_tab() == otto_core::NotchExpandedTab::Stats
                || self.layout.previous_tab == otto_core::NotchExpandedTab::Stats);
        if !stats_visible {
            return otto_core::FocusStatsView::default();
        }
        let engaged_day = match hovered {
            Some(HitTarget::FocusStatsDay(index)) => Some(usize::from(index)),
            _ => match self.focus.current() {
                Some(HitTarget::FocusStatsDay(index)) => Some(usize::from(index)),
                _ => None,
            },
        };
        runtime
            .focus_activity
            .try_borrow()
            .map(|activity| {
                otto_core::FocusStatsView::resolve(
                    &activity,
                    self.notch.stats_range(),
                    self.notch.stats_selected_day(),
                    engaged_day,
                )
            })
            .unwrap_or_default()
    }

    pub(super) fn present(&mut self, runtime: &RuntimeContext) {
        runtime
            .focus_active
            .set(self.notch_enabled && self.notch.focus_active());
        if !self.shown() || !self.ensure_renderer(runtime) {
            return;
        }
        let hovered = self
            .hovered
            .or_else(|| crate::menu::active_top_bar_target(runtime))
            .or_else(|| {
                crate::menu::is_notification_open(runtime).then_some(HitTarget::Notifications)
            })
            .or_else(|| crate::menu::is_calendar_open(runtime).then_some(HitTarget::Clock));
        let focus_stats = self.resolve_focus_stats(runtime, hovered);
        self.sync_stats_app_icons(&focus_stats);
        let appearance = crate::theme_state::appearance(runtime);
        let now = Instant::now();
        let media = self.notch.media_scene(now, crate::notch::unix_now_ms());
        let status = self.controls.status();
        let pomodoro = self.notch.pomodoro_scene(now);
        let privacy = self.notch.privacy_state();
        let call = self.notch.call_scene(now);
        let notification = self.notch.notification_presentation(now);
        let notif_unread = self.notch.has_unread_notifications();
        let notif_coalesce = self.notch.notification_coalesce_progress(now);
        let reduced_motion = !self.motion.allows_motion();
        let show_bg = self.top_bar_enabled && self.menu_prefs.show_background;
        let hide_chrome = !self.top_bar_chrome_visible();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.set_bar_background(show_bg);
        renderer.set_bar_chrome_hidden(hide_chrome);
        renderer.set_notification_unread(notif_unread);
        renderer.set_reduced_motion(reduced_motion);
        renderer.set_notification_coalesce(notif_coalesce);
        crate::host::log_present_error(
            "bar",
            renderer.present(
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
                call.as_ref(),
                notification.as_ref(),
            ),
        );
        if self.panel_visible {
            crate::host::log_present_error(
                "bar-panel",
                renderer.present_panel(
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
                    notification.as_ref(),
                    &focus_stats,
                ),
            );
        }
    }
}
