use std::time::{Duration, Instant};

use crate::focus_shield::{self, ShieldTarget};
use crate::host::RuntimeContext;
use crate::win_msg::{
    DEADLINE_TIMER, HUD_ANIM_TIMER, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    STATUS_TIMER, WM_APP_IDLE_WAKE,
};
use otto_core::{
    DayKey, HudDeviceLabel, LyricsStatus, NotchHudKind, detect_hud_event, inspect_browser_context,
    is_browser_executable,
};
use otto_platform::{
    LyricsOutcome, cursor_position, foreground_application, foreground_title_generation,
    is_escape_down, motion_preference, notify_bluetooth_hardware_change, read_system_status,
    register_idle_wake, request_browser_url_inspection, save_focus_activity, suppress_native_osd,
    take_browser_url_inspection, take_keystroke_count, take_mouse_click_count,
    unregister_idle_wake, user_idle_ms, watch_top_edge, window_change_generations,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer, SetWindowPos,
};

use super::Bar;
use super::HUD_ANIM_MS;
use super::MAINTAIN_EVERY_TICKS;
use super::MIN_DEADLINE_MS;
use super::STATUS_POLL_MS;
use super::periodic_tick_due;

impl Bar {
    pub(super) fn ensure_status_timer(&mut self) {
        if self.status_timer_running || (!self.top_bar_enabled && !self.notch_enabled) {
            return;
        }
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                STATUS_TIMER,
                STATUS_POLL_MS,
                None,
            );
        }
        register_idle_wake(self.hwnd, WM_APP_IDLE_WAKE);
        self.status_timer_running = true;
    }

    pub(super) fn stop_status_timer(&mut self) {
        if !self.status_timer_running {
            return;
        }
        self.status_timer_running = false;
        unregister_idle_wake();
        unsafe {
            let hwnd = Some(HWND(self.hwnd as *mut _));
            let _ = KillTimer(hwnd, STATUS_TIMER);
            let _ = KillTimer(hwnd, DEADLINE_TIMER);
        }
    }

    pub(super) fn ensure_anim_timer(&mut self) {
        if !self.top_bar_enabled && !self.notch_enabled {
            return;
        }
        if !self.animating {
            let timer = unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    HUD_ANIM_TIMER,
                    HUD_ANIM_MS,
                    None,
                )
            };
            if timer != 0 {
                otto_platform::begin_fine_timing();
                self.animating = true;
                self.settle_ticks = 10;
            }
        }
    }

    pub(super) fn stop_anim_timer(&mut self) {
        if self.animating {
            self.animating = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), HUD_ANIM_TIMER);
            }
            otto_platform::end_fine_timing();
        }
    }

    pub(super) fn wake(&mut self, runtime: &RuntimeContext) {
        if !self.status_timer_running {
            return;
        }
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
            self.run_tick(runtime, true);
        }
    }

    pub(super) fn wake_timer_only(&mut self) {
        if !self.status_timer_running {
            return;
        }
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
        }
    }

    pub(super) fn idle_wake(&mut self, runtime: &RuntimeContext) {
        if !self.status_timer_running {
            return;
        }
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
        }
        self.run_tick(runtime, true);
    }

    pub(super) fn set_status_interval(&self, interval: u32) {
        if !self.status_timer_running {
            return;
        }
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                STATUS_TIMER,
                interval,
                None,
            );
        }
    }

    pub(super) const fn periodic_due(&self, every_ticks: u8) -> bool {
        periodic_tick_due(self.maintain_ticks, every_ticks)
    }

    pub(super) fn hidden_for_full_screen(&self) -> bool {
        (self.top_bar_enabled || self.notch_enabled)
            && self.visibility.tracks_pointer()
            && !self.visibility.is_visible()
    }

    pub(super) fn status_busy(&self, runtime: &RuntimeContext, dirty: bool) -> bool {
        let reveal_needs_polling = self.visibility.tracks_pointer()
            && (self.visibility.is_visible() || window_change_generations().is_none());
        dirty || reveal_needs_polling || self.needs_status_polling(runtime)
    }

    pub(super) fn needs_status_polling(&self, runtime: &RuntimeContext) -> bool {
        self.animating
            || self.settle_ticks > 0
            || self.panel_visible
            || self.hovered.is_some()
            || self.focus.is_active()
            || self.notch.is_seek_dragging()
            || self.notch.hud_state().is_some()
            || self.notch.lyrics_view().status == LyricsStatus::Loading
            || crate::menu::active_top_bar_target(runtime).is_some()
            || crate::control_center::is_open(runtime)
            || crate::menu::is_calendar_open(runtime)
            || crate::menu::is_notification_open(runtime)
    }

    pub(super) fn refresh_motion(&mut self) {
        let next_motion = motion_preference();
        if next_motion != self.motion {
            self.motion = next_motion;
            self.ensure_anim_timer();
        }
    }

    pub(super) fn tick(&mut self, runtime: &RuntimeContext) {
        self.run_tick(runtime, false);
    }

    pub(super) fn run_tick(&mut self, runtime: &RuntimeContext, force_media: bool) {
        self.maintain_ticks = self.maintain_ticks.wrapping_add(1);
        self.track_reveal(runtime);
        if !self.cadence.is_idle() || self.periodic_due(MAINTAIN_EVERY_TICKS) {
            crate::window_controls::poll_foreground(runtime);
        }
        if self.periodic_due(MAINTAIN_EVERY_TICKS) {
            if self.notch_enabled && self.notch.hud_enabled() {
                suppress_native_osd();
            }
            self.appbar.maintain(
                self.screen.width,
                self.screen.height,
                self.layout.bar.height,
            );
            if self.panel_visible && self.shown() {
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
                self.io.raise_quick_capture();
            }
        }
        let mut dirty = false;
        while let Some(response) = self.io.poll_lyrics() {
            if self.io.is_current_lyrics_generation(response.generation)
                && let Some(media) = self.notch.media()
                && media.title == response.title
                && media.artist == response.artist
            {
                let (sheet, status) = match response.outcome {
                    LyricsOutcome::Found { sheet, .. } => (Some(sheet), LyricsStatus::Ready),
                    LyricsOutcome::NotFound => (
                        None,
                        LyricsStatus::NotFound {
                            searched_online: self.io.online_lyrics_enabled,
                        },
                    ),
                    LyricsOutcome::Failed => (None, LyricsStatus::Failed),
                };
                self.notch.finish_lyrics(sheet, status);
                self.recompute_layout();
                self.ensure_anim_timer();
                dirty = true;
            }
        }
        let foreground_changed = self
            .foreground_gate
            .should_refresh(window_change_generations().map(|changes| changes.geometry));
        let (active_changed, title_changed, fg_dirty) =
            self.sync_foreground(runtime, foreground_changed);
        dirty |= fg_dirty;
        self.sync_full_screen(runtime);
        self.sync_backdrop(runtime, foreground_changed);
        self.sync_keyboard_focus(runtime);
        if self.notch.is_seek_dragging() && is_escape_down() {
            self.release_seek(runtime, false);
        }
        if self.refresh_media(force_media) {
            dirty = true;
        }
        if self.update_focus_activity(runtime, active_changed, title_changed) {
            dirty = true;
        }
        dirty |= self.sync_notification_feed();
        if dirty {
            self.ensure_anim_timer();
            self.present(runtime);
        }
        self.schedule_deadline();
        watch_top_edge(
            self.hidden_for_full_screen(),
            self.screen.width,
            self.screen.height,
        );
        let busy = self.status_busy(runtime, dirty);
        self.observe_cadence(busy);
    }

    pub(super) fn observe_cadence(&mut self, busy: bool) {
        if let Some(interval) = self.cadence.observe(busy) {
            self.set_status_interval(interval);
            if self.cadence.is_idle() {
                otto_platform::trim_working_set();
            }
        }
    }

    pub(super) fn sync_notification_feed(&mut self) -> bool {
        if !self.notch_enabled {
            return false;
        }
        let Some(snapshot) = otto_platform::take_notification_changes() else {
            return false;
        };
        if !self
            .notch
            .ingest_notifications(&snapshot.items, Instant::now())
        {
            return false;
        }
        self.recompute_layout();
        self.sync_panel_visibility();
        true
    }

    pub(super) fn refresh_system_status(&mut self) -> bool {
        let mut latest = read_system_status();
        let previous = self.controls.status();
        if latest.input_lang.is_empty() && !previous.input_lang.is_empty() {
            latest.input_lang = previous.input_lang;
        }
        let mut dirty = false;
        if self.notch_enabled
            && self.notch.hud_enabled()
            && let Some(event) = detect_hud_event(previous, latest)
        {
            self.notch.trigger_hud(event);
            dirty = true;
        }
        self.notch.set_clock(latest.clock);
        if self
            .notch
            .set_dnd_active(latest.dnd_active.unwrap_or(false))
        {
            dirty = true;
        }
        if self.controls.apply_status(latest) {
            dirty = true;
        }
        dirty
    }

    pub(super) fn schedule_deadline(&self) {
        let hwnd = Some(HWND(self.hwnd as *mut _));
        if !self.notch_enabled {
            unsafe {
                let _ = KillTimer(hwnd, DEADLINE_TIMER);
            }
            return;
        }
        let now = Instant::now();
        match self.notch.next_deadline(now) {
            Some(deadline) => {
                let wait = deadline
                    .saturating_duration_since(now)
                    .as_millis()
                    .max(MIN_DEADLINE_MS);
                unsafe {
                    SetTimer(
                        hwnd,
                        DEADLINE_TIMER,
                        u32::try_from(wait).unwrap_or(u32::MAX),
                        None,
                    );
                }
            }
            None => unsafe {
                let _ = KillTimer(hwnd, DEADLINE_TIMER);
            },
        }
    }

    pub(super) fn on_deadline(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let (cx, cy) = cursor_position();
        let hovered = self.layout.notch.contains(cx, cy)
            || matches!(
                self.focus.current(),
                Some(
                    otto_core::HitTarget::NotificationCard
                        | otto_core::HitTarget::NotificationClose
                )
            );
        let mut update = self.notch.sync_clock(now);
        if self.notch.sync_notifications(now, hovered) {
            update.changed = true;
        }
        self.apply_notch_update(runtime, update);
        self.schedule_deadline();
    }

    pub(super) fn on_power_event(&mut self, runtime: &RuntimeContext, event: usize) {
        let now = Instant::now();
        match event {
            PBT_APMSUSPEND => {
                if let Ok(mut fa) = runtime.focus_activity.try_borrow_mut() {
                    fa.reset_sample_clock();
                    if fa.is_dirty() && save_focus_activity(&fa) {
                        fa.mark_saved();
                        self.last_focus_save = now;
                    }
                }
                let update = self.notch.suspend(now);
                self.apply_notch_update(runtime, update);
            }
            PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC => {
                if let Ok(mut fa) = runtime.focus_activity.try_borrow_mut() {
                    fa.reset_sample_clock();
                }
                self.notch.resume(now);
                notify_bluetooth_hardware_change();
                self.schedule_deadline();
            }
            _ => {}
        }
    }

    fn sync_foreground(
        &mut self,
        runtime: &RuntimeContext,
        foreground_changed: bool,
    ) -> (bool, bool, bool) {
        let title_changed = self
            .title_gate
            .should_refresh(foreground_title_generation());
        let mut active_changed = false;
        let mut dirty = false;
        if (foreground_changed || title_changed)
            && let Some(foreground) = foreground_application(self.own_pid)
        {
            if let Some(win) = foreground.window.as_ref() {
                self.remember_app_icon_hint(&win.executable, &win.executable);
                self.remember_app_icon_hint(&win.app.key(), &win.executable);
            }
            let change = self
                .controls
                .apply_foreground(foreground.name, foreground.window);
            active_changed = change.app || change.window || title_changed;
            if change.app {
                self.recompute_layout();
                dirty = true;
            }
            if change.window {
                if crate::menu::active_top_bar_target(runtime).is_some()
                    || crate::menu::is_calendar_open(runtime)
                    || crate::menu::is_notification_open(runtime)
                {
                    crate::menu::close(runtime);
                    dirty = true;
                }
                if self.notch.collapse_from_external_focus_change() {
                    self.recompute_layout();
                    self.sync_panel_visibility();
                    dirty = true;
                }
            }
        }
        (active_changed, title_changed, dirty)
    }

    #[allow(clippy::too_many_lines)]
    fn update_focus_activity(
        &mut self,
        runtime: &RuntimeContext,
        active_changed: bool,
        title_changed: bool,
    ) -> bool {
        let active_window = self.controls.active_window().cloned();
        let active_hwnd = active_window.as_ref().map_or(0, |win| win.identity.hwnd);
        if title_changed
            && self
                .last_browser_url
                .as_ref()
                .is_some_and(|(hwnd, _)| *hwnd == active_hwnd)
        {
            self.last_browser_url = None;
        }
        let mut url_arrived = false;
        if let Some((inspected_hwnd, maybe_url)) = take_browser_url_inspection()
            && let Some(url) = maybe_url
        {
            if inspected_hwnd == active_hwnd {
                url_arrived = self
                    .last_browser_url
                    .as_ref()
                    .is_none_or(|(h, old)| *h != inspected_hwnd || old != &url);
            }
            self.last_browser_url = Some((inspected_hwnd, url));
        }

        let is_browser = active_window
            .as_ref()
            .is_some_and(|win| is_browser_executable(&win.executable));
        if is_browser && (active_changed || title_changed) {
            request_browser_url_inspection(active_hwnd);
        }

        let day = DayKey::from_clock(self.controls.status().clock);
        let now = Instant::now();
        let now_ms = u64::try_from(now.saturating_duration_since(self.focus_epoch).as_millis())
            .unwrap_or(u64::MAX);
        let idle_ms = user_idle_ms();
        let clicks = take_mouse_click_count();
        let keystrokes = take_keystroke_count();
        let focus_running = self.notch_enabled && self.notch.is_focus_blocking_active();
        focus_shield::sync(runtime, focus_running);

        let mut blocked_hud: Option<NotchHudKind> = None;
        let mut block_target: Option<ShieldTarget> = None;
        let mut stats_updated = false;
        if let Ok(mut fa) = runtime.focus_activity.try_borrow_mut() {
            if active_changed
                || url_arrived
                || (fa.current_app_key().is_none() && active_window.is_some())
            {
                if let Some(win) = active_window.as_ref() {
                    let app_key = if win.executable.is_empty() {
                        win.app.key()
                    } else {
                        win.executable.clone()
                    };
                    let cached_url = self
                        .last_browser_url
                        .as_ref()
                        .filter(|(hwnd, _)| *hwnd == win.identity.hwnd)
                        .map(|(_, url)| url.as_str());
                    let browser_info =
                        is_browser.then(|| inspect_browser_context(cached_url, &win.title));
                    fa.observe_foreground(day, &app_key, &win.app_name, browser_info);
                } else {
                    fa.observe_foreground(day, "", "", None);
                }
            }
            fa.record_sample(day, now_ms, idle_ms, focus_running, clicks, keystrokes);
            stats_updated = fa.is_dirty();

            if focus_running
                && let Some(win) = active_window.as_ref()
                && self.last_blocked_window.is_none_or(|(hwnd, at)| {
                    hwnd != win.identity.hwnd
                        || now.saturating_duration_since(at) >= Duration::from_secs(2)
                })
                && let Some((enforcement, page)) = {
                    let domain = fa.current_domain().map(str::to_owned);
                    let page = if is_browser {
                        win.title.clone()
                    } else {
                        win.executable.clone()
                    };
                    if focus_shield::may_block(runtime, win.identity.hwnd, &page) {
                        let block_key = if win.executable.is_empty() {
                            win.app.key()
                        } else {
                            win.executable.clone()
                        };
                        fa.evaluate_block(true, &block_key, &win.app_name, domain.as_deref())
                            .map(|enforcement| (enforcement, page))
                    } else {
                        None
                    }
                }
            {
                self.last_blocked_window = Some((win.identity.hwnd, now));
                let target = HudDeviceLabel::from_name(enforcement.target_label());
                blocked_hud = Some(NotchHudKind::FocusBlocked { target });
                block_target = Some(ShieldTarget {
                    window: win.identity.hwnd,
                    label: enforcement.target_label().to_owned(),
                    page,
                    browser: is_browser,
                });
            }

            if fa.is_dirty()
                && (blocked_hud.is_some()
                    || now.saturating_duration_since(self.last_focus_save)
                        >= Duration::from_secs(30))
                && save_focus_activity(&fa)
            {
                fa.mark_saved();
                self.last_focus_save = now;
            }
        }

        if let Some(target) = block_target {
            focus_shield::block(runtime, target);
        }
        let mut notch_stats_dirty = false;
        if stats_updated && self.periodic_due(MAINTAIN_EVERY_TICKS) {
            crate::settings_window::repaint_if_focus_page(runtime);
            if self.panel_visible && self.notch.active_tab() == otto_core::NotchExpandedTab::Stats {
                notch_stats_dirty = true;
            }
        }
        if let Some(kind) = blocked_hud {
            if self.notch.hud_enabled() {
                self.notch.trigger_hud(kind);
            }
            return true;
        }
        notch_stats_dirty
    }
}
