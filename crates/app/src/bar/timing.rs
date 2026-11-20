use std::time::Instant;

use crate::host::RuntimeContext;
use crate::win_msg::{
    DEADLINE_TIMER, HUD_ANIM_TIMER, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    STATUS_TIMER,
};
use otto_core::{LyricsStatus, detect_hud_event};
use otto_platform::{
    LyricsOutcome, cursor_position, foreground_application, is_escape_down, motion_preference,
    notify_bluetooth_hardware_change, read_system_status, suppress_native_osd, watch_top_edge,
    window_change_generations,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, KillTimer, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetTimer, SetWindowPos,
};

use super::Bar;
use super::HUD_ANIM_MS;
use super::MAINTAIN_EVERY_TICKS;
use super::MIN_DEADLINE_MS;
use super::periodic_tick_due;

impl Bar {
    pub(super) fn ensure_anim_timer(&mut self) {
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
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
            self.run_tick(runtime, true);
        }
    }

    pub(super) fn wake_timer_only(&mut self) {
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
        }
    }

    pub(super) fn idle_wake(&mut self, runtime: &RuntimeContext) {
        if let Some(interval) = self.cadence.wake() {
            self.set_status_interval(interval);
        }
        self.run_tick(runtime, true);
    }

    pub(super) fn set_status_interval(&self, interval: u32) {
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
        if foreground_changed && let Some(foreground) = foreground_application(self.own_pid) {
            let change = self
                .controls
                .apply_foreground(foreground.name, foreground.window);
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
        self.sync_full_screen(runtime);
        self.sync_backdrop(runtime, foreground_changed);
        self.sync_keyboard_focus(runtime);
        if self.notch.is_seek_dragging() && is_escape_down() {
            self.release_seek(runtime, false);
        }
        if self.refresh_media(force_media) {
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
        let hovered = self.layout.notch.contains(cx, cy);
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
                let update = self.notch.suspend(now);
                self.apply_notch_update(runtime, update);
            }
            PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC => {
                self.notch.resume(now);
                notify_bluetooth_hardware_change();
                self.schedule_deadline();
            }
            _ => {}
        }
    }
}
