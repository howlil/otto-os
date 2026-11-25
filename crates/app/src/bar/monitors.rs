use crate::backdrop_feed::BackdropFeed;
use crate::host::RuntimeContext;
use crate::win_msg::WM_APP_BAR_BACKDROP;
use otto_core::{
    BACKDROP_PADDING_PT, BarPointer, BarVisibility, BrandIcon, CaptureRegion, HitTarget, Layout,
    MotionPreference, NotchLayoutInput, PomodoroScene, PomodoroState, PrivacyState, Rect,
    capture_interval_for, capture_wanted, compute_layout_with_notch, scale,
};
use otto_platform::{
    DisplayMonitor, TopEdgeReservation, capture_backdrop_now, cursor_position,
    fullscreen_on_monitor, hide_window, is_running_on_battery, is_surface_visible, place,
    release_capture_if_owned, secondary_monitors, watch_top_edges,
};
use otto_render::Renderer;

use super::{Bar, with_bar};

pub(super) struct SecondaryBar {
    pub(super) hwnd: isize,
    pub(super) monitor: DisplayMonitor,
    pub(super) reservation: Option<TopEdgeReservation>,
    pub(super) renderer: Option<Renderer>,
    pub(super) layout: Layout,
    pub(super) visibility: BarVisibility,
    pub(super) shell_full_screen: bool,
    pub(super) hovered: Option<HitTarget>,
    pub(super) tracking_mouse: bool,
    pub(super) backdrop: Option<BackdropFeed>,
}

impl SecondaryBar {
    pub(super) fn teardown(mut self, runtime: &RuntimeContext) {
        let _ = release_capture_if_owned(self.hwnd);
        self.backdrop = None;
        self.reservation = None;
        self.renderer = None;
        hide_window(self.hwnd);
        runtime.queue_destroy_hwnd(self.hwnd);
    }

    pub(super) fn place(&self) {
        if !self.visibility.is_visible() {
            return;
        }
        place(
            self.hwnd,
            Rect {
                x: self.monitor.bounds.x,
                y: self.monitor.bounds.y,
                width: self.layout.bar.width,
                height: self.layout.bar.height,
            },
        );
    }

    pub(super) fn anchor_right(&self, target: HitTarget) -> i32 {
        let local = self
            .layout
            .menu_anchor_rect(target)
            .map_or(self.monitor.bounds.width, |slot| slot.right());
        self.monitor.bounds.x + local
    }

    pub(super) fn popup_anchor_y(&self) -> i32 {
        self.monitor.bounds.y + self.layout.popup_anchor_y(self.monitor.dpi)
    }
}

pub(crate) fn on_taskbar_created(runtime: &RuntimeContext) {
    with_bar(runtime, |bar| {
        if bar.top_bar_enabled {
            bar.appbar
                .reserve_top(bar.screen.width, bar.screen.height, bar.layout.bar.height);
            for sec in &mut bar.secondary_bars {
                sec.reservation = TopEdgeReservation::register(sec.hwnd);
                if let Some(reservation) = sec.reservation.as_mut() {
                    let _ = reservation.reserve(sec.monitor.bounds, sec.layout.bar.height);
                }
                sec.shell_full_screen = false;
                sec.place();
            }
        }
        bar.appbar
            .maintain(bar.screen.width, bar.screen.height, bar.layout.bar.height);
    });
}

impl Bar {
    pub(super) fn has_secondary_hwnd(&self, hwnd: isize) -> bool {
        self.secondary_bars.iter().any(|bar| bar.hwnd == hwnd)
    }

    pub(super) fn secondary_index(&self, hwnd: isize) -> Option<usize> {
        self.secondary_bars.iter().position(|bar| bar.hwnd == hwnd)
    }

    fn secondary_layout_input(&self) -> NotchLayoutInput {
        NotchLayoutInput {
            top_bar_enabled: self.top_bar_enabled,
            notch_enabled: false,
            hidden_items: self.menu_prefs.hidden_items,
            control_center_enabled: self.control_center_enabled,
            ..NotchLayoutInput::default()
        }
    }

    fn create_secondary_bar(
        &self,
        runtime: &RuntimeContext,
        monitor: DisplayMonitor,
    ) -> Option<SecondaryBar> {
        let hwnd = crate::host::create_secondary_bar_window()?;
        let layout = compute_layout_with_notch(
            &self.metrics,
            monitor.bounds.width,
            monitor.dpi,
            self.controls.active_app(),
            self.secondary_layout_input(),
        );
        let renderer =
            Renderer::new(layout.bar.width, layout.bar.height, 1, 1, monitor.dpi, 0.0).ok();
        if let Some(renderer) = renderer.as_ref() {
            renderer.set_brand_icon(runtime.brand_icon());
        }
        let mut reservation = TopEdgeReservation::register(hwnd);
        if let Some(res) = reservation.as_mut() {
            let _ = res.reserve(monitor.bounds, layout.bar.height);
        }
        let mut visibility = BarVisibility::default();
        let _ = visibility.set_mode(self.menu_prefs.hide_mode);
        let _ = visibility.set_full_screen_app(fullscreen_on_monitor(monitor.bounds, self.own_pid));
        Some(SecondaryBar {
            hwnd,
            monitor,
            reservation,
            renderer,
            layout,
            visibility,
            shell_full_screen: false,
            hovered: None,
            tracking_mouse: false,
            backdrop: None,
        })
    }

    fn update_existing_secondary(
        &self,
        runtime: &RuntimeContext,
        sec: &mut SecondaryBar,
        monitor: DisplayMonitor,
        input: NotchLayoutInput,
    ) {
        let prev = sec.monitor;
        let carried = sec.renderer.as_ref().and_then(Renderer::take_bar_backdrop);
        sec.monitor = monitor;
        sec.layout = compute_layout_with_notch(
            &self.metrics,
            monitor.bounds.width,
            monitor.dpi,
            self.controls.active_app(),
            input,
        );
        let needs_renderer = sec.renderer.is_none()
            || prev.bounds.width != monitor.bounds.width
            || prev.bounds.height != monitor.bounds.height
            || prev.dpi != monitor.dpi;
        if needs_renderer
            && let Ok(renderer) = Renderer::new(
                sec.layout.bar.width,
                sec.layout.bar.height,
                1,
                1,
                monitor.dpi,
                0.0,
            )
        {
            renderer.set_brand_icon(runtime.brand_icon());
            if prev.bounds == monitor.bounds
                && let Some((pixels, origin)) = carried
            {
                renderer.set_bar_backdrop(pixels, origin);
            }
            sec.renderer = Some(renderer);
            if let Some(feed) = sec.backdrop.as_mut() {
                feed.reset();
            }
        }
        if sec.reservation.is_none() {
            sec.reservation = TopEdgeReservation::register(sec.hwnd);
        }
        if let Some(res) = sec.reservation.as_mut() {
            let _ = res.reserve(monitor.bounds, sec.layout.bar.height);
        }
        let _ = sec.visibility.set_mode(self.menu_prefs.hide_mode);
    }

    pub(super) fn sync_secondary_bars(&mut self, runtime: &RuntimeContext) {
        if !self.top_bar_enabled {
            self.teardown_secondary_bars(runtime);
            self.sync_top_edge_watches();
            return;
        }
        let monitors = secondary_monitors();
        let mut existing = std::mem::take(&mut self.secondary_bars);
        let mut next = Vec::with_capacity(monitors.len());
        let input = self.secondary_layout_input();
        for monitor in monitors {
            if let Some(pos) = existing.iter().position(|bar| bar.monitor.id == monitor.id) {
                let mut sec = existing.remove(pos);
                self.update_existing_secondary(runtime, &mut sec, monitor, input);
                next.push(sec);
            } else if let Some(sec) = self.create_secondary_bar(runtime, monitor) {
                next.push(sec);
            }
        }
        for stale in existing {
            stale.teardown(runtime);
        }
        self.secondary_bars = next;
        runtime.set_secondary_bar_hwnds(self.secondary_bars.iter().map(|b| b.hwnd).collect());
        self.sync_secondary_backdrops(runtime, true);
        for index in 0..self.secondary_bars.len() {
            self.apply_secondary_visibility(runtime, index);
        }
        self.sync_top_edge_watches();
    }

    pub(super) fn teardown_secondary_bars(&mut self, runtime: &RuntimeContext) {
        for sec in self.secondary_bars.drain(..) {
            sec.teardown(runtime);
        }
        runtime.set_secondary_bar_hwnds(Vec::new());
    }

    pub(super) fn recompute_secondary_layouts(&mut self) {
        let input = self.secondary_layout_input();
        let app_name = self.controls.active_app();
        for sec in &mut self.secondary_bars {
            sec.layout = compute_layout_with_notch(
                &self.metrics,
                sec.monitor.bounds.width,
                sec.monitor.dpi,
                app_name,
                input,
            );
        }
    }

    pub(super) fn apply_secondary_brand_icon(&self, brand_icon: BrandIcon) {
        for sec in &self.secondary_bars {
            if let Some(renderer) = sec.renderer.as_ref() {
                renderer.set_brand_icon(brand_icon);
            }
        }
    }

    pub(super) fn apply_secondary_capture_visible(&mut self, visible: bool) {
        for sec in &mut self.secondary_bars {
            if let Some(feed) = sec.backdrop.as_mut() {
                feed.set_capture_visible(visible);
            }
            if visible && let Some(renderer) = sec.renderer.as_ref() {
                renderer.clear_bar_backdrop();
            }
        }
    }

    pub(super) fn clear_secondary_backdrops(&mut self) {
        for sec in &mut self.secondary_bars {
            sec.backdrop = None;
            if let Some(renderer) = sec.renderer.as_ref() {
                renderer.clear_bar_backdrop();
            }
        }
    }

    pub(super) fn apply_secondary_menu_mode(&mut self, runtime: &RuntimeContext) {
        let mode = self.menu_prefs.hide_mode;
        for index in 0..self.secondary_bars.len() {
            if self.secondary_bars[index].visibility.set_mode(mode) {
                self.apply_secondary_visibility(runtime, index);
            }
        }
    }

    pub(super) fn track_secondary_reveal(&mut self, runtime: &RuntimeContext) {
        let (gx, gy) = cursor_position();
        let popup_open = crate::menu::active_top_bar_target(runtime).is_some()
            || crate::control_center::is_open(runtime)
            || crate::menu::is_calendar_open(runtime)
            || crate::menu::is_notification_open(runtime);
        for index in 0..self.secondary_bars.len() {
            let bounds = self.secondary_bars[index].monitor.bounds;
            let bar_height = self.secondary_bars[index].layout.bar.height;
            let pointer = BarPointer {
                on_bar_monitor: bounds.contains(gx, gy),
                y: gy - bounds.y,
            };
            if self.secondary_bars[index]
                .visibility
                .track_pointer(pointer, bar_height, popup_open)
            {
                self.apply_secondary_visibility(runtime, index);
            }
        }
    }

    pub(super) fn sync_secondary_full_screen(&mut self, runtime: &RuntimeContext) {
        for index in 0..self.secondary_bars.len() {
            let bounds = self.secondary_bars[index].monitor.bounds;
            let active = self.secondary_bars[index].shell_full_screen
                || fullscreen_on_monitor(bounds, self.own_pid);
            if self.secondary_bars[index]
                .visibility
                .set_full_screen_app(active)
            {
                self.apply_secondary_visibility(runtime, index);
            }
        }
    }

    pub(super) fn sync_top_edge_watches(&self) {
        let mut watched = Vec::new();
        if self.hidden_for_full_screen() {
            watched.push(Rect {
                x: 0,
                y: 0,
                width: self.screen.width,
                height: self.screen.height,
            });
        }
        for sec in &self.secondary_bars {
            if self.top_bar_enabled
                && sec.visibility.tracks_pointer()
                && !sec.visibility.is_visible()
            {
                watched.push(sec.monitor.bounds);
            }
        }
        watch_top_edges(&watched);
    }

    pub(super) fn secondary_needs_polling(&self) -> bool {
        self.secondary_bars.iter().any(|sec| {
            sec.hovered.is_some()
                || (sec.visibility.tracks_pointer() && sec.visibility.is_visible())
        })
    }

    pub(super) fn sync_secondary_backdrops(&mut self, runtime: &RuntimeContext, force: bool) {
        let appearance = crate::theme_state::appearance(runtime);
        let wanted = capture_wanted(&appearance, runtime.glass_enabled.get())
            && self.top_bar_enabled
            && self.menu_prefs.show_background;
        for index in 0..self.secondary_bars.len() {
            let visible =
                self.top_bar_enabled && self.secondary_bars[index].visibility.is_visible();
            if !wanted {
                let had_feed = self.secondary_bars[index].backdrop.take().is_some();
                let had_backdrop = self.secondary_bars[index]
                    .renderer
                    .as_ref()
                    .is_some_and(Renderer::has_bar_backdrop);
                if let Some(renderer) = self.secondary_bars[index].renderer.as_ref() {
                    renderer.clear_bar_backdrop();
                }
                if (had_feed || had_backdrop) && visible {
                    self.present_secondary(runtime, index);
                }
                continue;
            }
            if !visible {
                continue;
            }
            if self.secondary_bars[index].backdrop.is_none() {
                let hwnd = self.secondary_bars[index].hwnd;
                self.secondary_bars[index].backdrop = Some(BackdropFeed::start(
                    hwnd,
                    WM_APP_BAR_BACKDROP,
                    vec![hwnd],
                    runtime.capture_visible.get(),
                ));
            }
            let mon = self.secondary_bars[index].monitor;
            let bar_rect = Rect {
                x: mon.bounds.x,
                y: mon.bounds.y,
                width: self.secondary_bars[index].layout.bar.width,
                height: self.secondary_bars[index].layout.bar.height,
            };
            let pad = scale(BACKDROP_PADDING_PT, mon.dpi);
            let Some(region) = CaptureRegion::padded_to_bounds(bar_rect, pad, mon.bounds) else {
                continue;
            };
            let sec = &mut self.secondary_bars[index];
            if let Some(renderer) = sec.renderer.as_ref()
                && !renderer.has_bar_backdrop()
            {
                let can_prime = !runtime.capture_visible.get() || !is_surface_visible(sec.hwnd);
                if can_prime
                    && let Some(pixels) =
                        capture_backdrop_now(region.origin(), region.width, region.height)
                {
                    renderer.set_bar_backdrop(
                        pixels,
                        (region.x - mon.bounds.x, region.y - mon.bounds.y),
                    );
                }
            }
            if let Some(feed) = sec.backdrop.as_mut() {
                let active = sec.hovered.is_some() && feed.recently_changed();
                let interval = capture_interval_for(is_running_on_battery(), active);
                feed.request(region, interval, force);
            }
        }
    }

    pub(super) fn present_secondary_bars(&mut self, runtime: &RuntimeContext) {
        for index in 0..self.secondary_bars.len() {
            self.present_secondary(runtime, index);
        }
    }

    pub(super) fn present_secondary(&mut self, runtime: &RuntimeContext, index: usize) {
        if !self.top_bar_enabled || !self.secondary_bars[index].visibility.is_visible() {
            return;
        }
        let appearance = crate::theme_state::appearance(runtime);
        let status = self.controls.status();
        let app_name = self.controls.active_app();
        let notif_unread = self.notch.has_unread_notifications();
        let reduced_motion = !self.motion.allows_motion();
        let show_bg = self.menu_prefs.show_background;
        let pomodoro = PomodoroScene::steady(PomodoroState::default(), status.clock);
        let privacy = PrivacyState::default();
        let sec = &mut self.secondary_bars[index];
        let origin = sec.monitor.origin();
        let Some(renderer) = sec.renderer.as_mut() else {
            return;
        };
        renderer.set_bar_background(show_bg);
        renderer.set_bar_chrome_hidden(false);
        renderer.set_notification_unread(notif_unread);
        renderer.set_reduced_motion(reduced_motion);
        renderer.set_notification_coalesce(1.0);
        crate::host::log_present_error(
            "secondary-bar",
            renderer.present(
                sec.hwnd,
                origin,
                &otto_render::BarPaintState {
                    layout: &sec.layout,
                    status,
                    app_name,
                    hovered: sec.hovered,
                    focused: None,
                    hud: None,
                    pomodoro: &pomodoro,
                    media: None,
                    anim_phase: 0.0,
                    appearance,
                    privacy: &privacy,
                    call: None,
                    notification: None,
                },
            ),
        );
    }

    pub(super) fn secondary_anchor_for_cursor(
        &self,
        trigger: HitTarget,
    ) -> Option<(i32, i32, MotionPreference)> {
        let (cx, cy) = cursor_position();
        let sec = self
            .secondary_bars
            .iter()
            .find(|bar| bar.monitor.bounds.contains(cx, cy))?;
        Some((sec.anchor_right(trigger), sec.popup_anchor_y(), self.motion))
    }

    pub(super) fn secondary_search_trigger_for_cursor(&self) -> Option<Rect> {
        let (cx, cy) = cursor_position();
        let sec = self
            .secondary_bars
            .iter()
            .find(|bar| bar.monitor.bounds.contains(cx, cy))?;
        let slot = sec.layout.slots.search_slot;
        (slot.width > 0).then_some(Rect {
            x: sec.monitor.bounds.x + slot.x,
            y: sec.monitor.bounds.y + slot.y,
            width: slot.width,
            height: slot.height,
        })
    }

    pub(super) fn apply_secondary_visibility(&mut self, runtime: &RuntimeContext, index: usize) {
        if self.top_bar_enabled && self.secondary_bars[index].visibility.is_visible() {
            self.secondary_bars[index].place();
            self.present_secondary(runtime, index);
        } else {
            hide_window(self.secondary_bars[index].hwnd);
        }
    }
}
