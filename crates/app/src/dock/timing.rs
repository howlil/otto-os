use std::time::Instant;

use crate::host::RuntimeContext;
use crate::win_msg::{DOCK_ANIM_TIMER, DOCK_POLL_TIMER};
use otto_core::{DockPort, advance_bounce_phase, animate_value, dock_should_hide};
use otto_platform::{
    cursor_position, motion_preference, watch_bottom_edge, window_change_generations,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

use super::DOCK_ENTRIES_FALLBACK_TICKS;
use super::DOCK_REVEAL_BAND;
use super::Dock;
use super::actions::sync_icons;

impl Dock {
    fn refresh_badges(&mut self, _runtime: &RuntimeContext, force: bool) -> bool {
        self.badge_poll_tick = self.badge_poll_tick.wrapping_add(1);
        if !force
            && !self
                .badge_poll_tick
                .is_multiple_of(DOCK_ENTRIES_FALLBACK_TICKS)
        {
            return false;
        }
        let next_counts = self.port.notification_counts();
        let next_progress = if self.preferences.show_downloads {
            self.port.downloads_progress()
        } else {
            None
        };
        if next_counts != self.notification_counts || next_progress != self.downloads_progress {
            self.notification_counts = next_counts;
            self.downloads_progress = next_progress;
            return true;
        }
        false
    }

    fn sync_focus_badge_suppression(&mut self, runtime: &RuntimeContext) -> bool {
        let suppressed = runtime.focus_active.get()
            && runtime
                .focus_activity
                .try_borrow()
                .ok()
                .is_some_and(|fa| fa.blocklist.block_notifications);
        if suppressed != self.last_focus_badge_suppressed {
            self.last_focus_badge_suppressed = suppressed;
            return true;
        }
        false
    }

    pub(super) fn refresh_motion(&mut self) {
        let next_motion = motion_preference();
        if next_motion != self.motion {
            self.motion = next_motion;
            self.ensure_animating();
        }
    }

    pub(super) fn sync_hide_from_pointer(&mut self, runtime: &RuntimeContext) -> bool {
        let (gx, gy) = cursor_position();
        let (_, win_top) = self.window_origin();
        let local_x = gx;
        let local_y = gy - win_top;

        let should_auto_hide = dock_should_hide(
            self.preferences,
            self.is_fullscreen_covered(),
            self.window_overlaps,
        );

        let at_bottom_edge = gy >= self.screen.height - DOCK_REVEAL_BAND;
        let in_interactive_zone = self.anim.hide_progress < 0.85
            && local_x >= self.layout.pill.x - 20
            && local_x <= self.layout.pill.right() + 20
            && local_y >= self.layout.pill.y - 36;

        let target_hide = should_auto_hide && !at_bottom_edge && !in_interactive_zone;
        if target_hide != self.anim.should_hide {
            self.anim.should_hide = target_hide;
            if self.anim.should_hide {
                self.hovering = false;
            } else {
                self.ensure_topmost(runtime);
            }
            self.ensure_animating();
        }

        if self.hovering && !in_interactive_zone {
            self.hovering = false;
            self.ensure_animating();
        } else if !self.anim.should_hide && in_interactive_zone && self.anim.hide_progress < 0.25 {
            let new_cx = local_x as f32;
            if !self.hovering
                || self
                    .last_cursor_x
                    .is_none_or(|old| (old - new_cx).abs() > 0.5)
            {
                let started_hover = !self.hovering;
                self.hovering = true;
                self.last_cursor_x = Some(new_cx);
                if started_hover {
                    self.ensure_topmost(runtime);
                }
                self.ensure_animating();
            }
        }
        should_auto_hide
    }

    pub(super) fn on_poll_timer(&mut self, runtime: &RuntimeContext) {
        let mut dirty = self.cancel_drag_on_escape();
        let generations = window_change_generations();
        if self.refresh_entries_if_changed(generations.map(|changes| changes.topology)) {
            dirty = true;
        }
        let cover_geometry_changed = self
            .fullscreen_gate
            .should_refresh(generations.map(|changes| changes.geometry));
        if cover_geometry_changed {
            self.refresh_cover();
        }
        let reservation_changed = self.sync_reservation();
        dirty |= self.refresh_badges(runtime, dirty);

        let should_auto_hide = self.sync_hide_from_pointer(runtime);
        if cover_geometry_changed || reservation_changed || (dirty && self.anim.bounce_phase > 0.0)
        {
            self.ensure_topmost(runtime);
        }

        if self.sync_focus_badge_suppression(runtime) {
            dirty = true;
        }
        if dirty && !self.anim.timer_running {
            self.present(runtime);
        }
        let hidden = self.anim.should_hide;
        watch_bottom_edge(
            hidden.then_some(DOCK_REVEAL_BAND),
            self.screen.width,
            self.screen.height,
        );
        let reveal_needs_polling = should_auto_hide && (!hidden || generations.is_none());
        self.sync_keyboard_focus(runtime);
        let geometry_changed = self
            .backdrop_gate
            .should_refresh(generations.map(|changes| changes.geometry));
        self.sync_backdrop(runtime, geometry_changed);
        let busy = dirty
            || self.focus.is_active()
            || reveal_needs_polling
            || self.hovering
            || self.anim.timer_running
            || self.drag.start_idx.is_some();
        if let Some(interval) = self.cadence.observe(busy) {
            self.set_poll_interval(interval);
        }
    }

    pub(super) fn wake(&mut self, runtime: &RuntimeContext) {
        if let Some(interval) = self.cadence.wake() {
            self.set_poll_interval(interval);
            self.on_poll_timer(runtime);
        }
    }

    pub(super) fn wake_timer_only(&mut self) {
        if let Some(interval) = self.cadence.wake() {
            self.set_poll_interval(interval);
        }
    }

    pub(super) fn set_poll_interval(&self, interval: u32) {
        unsafe {
            SetTimer(
                Some(HWND(self.hwnd as *mut _)),
                DOCK_POLL_TIMER,
                interval,
                None,
            );
        }
    }

    pub(super) fn on_anim_timer(&mut self, runtime: &RuntimeContext) {
        let now = Instant::now();
        let dt = now
            .duration_since(self.anim.last_tick)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.anim.last_tick = now;

        self.cancel_drag_on_escape();
        self.track_hover_from_cursor();

        let target_mag = if self.hovering && !self.anim.should_hide {
            1.0
        } else {
            0.0
        };
        let target_hide = if self.anim.should_hide { 1.0 } else { 0.0 };

        self.anim.mag_strength = animate_value(
            self.anim.mag_strength,
            target_mag,
            self.motion_metrics.dock_magnify_rate,
            dt,
            self.motion,
        );
        self.anim.hide_progress = animate_value(
            self.anim.hide_progress,
            target_hide,
            self.motion_metrics.dock_hide_rate,
            dt,
            self.motion,
        );

        if self.anim.bounce_phase > 0.0 {
            self.anim.bounce_phase = advance_bounce_phase(
                self.anim.bounce_phase,
                dt,
                self.motion_metrics.bounce_duration_seconds,
                self.motion,
            );
            if self.anim.bounce_phase >= 1.0 {
                self.anim.bounce_phase = 0.0;
                self.anim.bounce_key = None;
            }
        }

        let mut presence_animating = false;
        for entry in &self.entries {
            if let Some(cur) = self.presence.get_mut(&entry.key) {
                let next = animate_value(
                    *cur,
                    1.0,
                    self.motion_metrics.dock_presence_enter_rate,
                    dt,
                    self.motion,
                );
                if (next - 1.0).abs() > 0.002 {
                    *cur = next;
                    presence_animating = true;
                } else {
                    self.presence.remove(&entry.key);
                }
            }
        }
        if !self.closing_entries.is_empty() {
            let mut still_closing = Vec::new();
            for closing in self.closing_entries.drain(..) {
                let cur = self.presence.get(&closing.key).copied().unwrap_or(1.0);
                let next = animate_value(
                    cur,
                    0.0,
                    self.motion_metrics.dock_presence_exit_rate,
                    dt,
                    self.motion,
                );
                if next > 0.03 {
                    self.presence.insert(closing.key.clone(), next);
                    still_closing.push(closing);
                    presence_animating = true;
                } else {
                    self.presence.remove(&closing.key);
                }
            }
            self.closing_entries = still_closing;
        }

        self.recompute_layout();
        self.present(runtime);

        let mag_done = (self.anim.mag_strength - target_mag).abs() < 0.001;
        let hide_done = (self.anim.hide_progress - target_hide).abs() < 0.001;
        let bounce_done = self.anim.bounce_phase == 0.0;

        if mag_done && hide_done && bounce_done && !presence_animating {
            if !self.hovering {
                self.last_cursor_x = None;
            }
            let all = self.combined_entries();
            sync_icons(&mut self.renderer, &all);
            self.stop_animating();
        }
    }

    pub(super) fn ensure_animating(&mut self) {
        if !self.anim.timer_running {
            let timer = unsafe {
                SetTimer(
                    Some(HWND(self.hwnd as *mut _)),
                    DOCK_ANIM_TIMER,
                    self.motion_metrics.animation_tick_ms,
                    None,
                )
            };
            if timer != 0 {
                otto_platform::begin_fine_timing();
                self.anim.timer_running = true;
                self.anim.last_tick = Instant::now();
            }
        }
    }

    pub(super) fn stop_animating(&mut self) {
        if self.anim.timer_running {
            self.anim.timer_running = false;
            unsafe {
                let _ = KillTimer(Some(HWND(self.hwnd as *mut _)), DOCK_ANIM_TIMER);
            }
            otto_platform::end_fine_timing();
        }
    }
}
