use std::time::{Duration, Instant};

use otto_core::{
    FocusSubMode, MotionMetrics, MotionPreference, NotchAction, NotchExpandedTab, PomodoroPeek,
    PomodoroStatus, PomodoroTick, animate_value, island_targets, step_spring,
};

use super::ALERT_HOLD_MS;
use super::ActivePeek;
use super::GLOW_SCENE_VISIBLE;
use super::HOVER_DEBOUNCE_MS;
use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;
use super::PEEK_FADE_SECONDS;
use super::PanelTrigger;
use super::unix_now_ms;

impl NotchController {
    pub(crate) fn suspend(&mut self, now: Instant) -> NotchUpdate {
        let now_ms = self.millis(now);
        let tick = self.clock.suspend(&mut self.pomodoro, now_ms);
        let timer_tick = self
            .timer_clock
            .suspend_timer(&mut self.quick_timer, now_ms);
        let sw_changed = self.stopwatch_clock.suspend(&mut self.stopwatch, now_ms);
        let (mut changed, mut effect) = self.apply_tick(tick, now);
        if tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Pomodoro;
        }
        let (timer_changed, timer_effect) = self.apply_tick(timer_tick, now);
        if timer_tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Timer;
        }
        changed |= timer_changed || sw_changed;
        if effect.is_none() {
            effect = timer_effect;
        }
        NotchUpdate { changed, effect }
    }

    pub(crate) fn resume(&mut self, now: Instant) {
        let now_ms = self.millis(now);
        self.clock.resume(now_ms);
        self.timer_clock.resume(now_ms);
        self.stopwatch_clock.resume(now_ms);
    }

    pub(crate) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let now_ms = self.millis(now);
        let pomodoro = self
            .clock
            .next_wakeup_ms(&self.pomodoro, now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let timer = self
            .timer_clock
            .next_timer_wakeup_ms(&self.quick_timer, now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let stopwatch = self
            .stopwatch_clock
            .next_wakeup_ms(&self.stopwatch, now_ms, self.is_stopwatch_visible())
            .map(|at| self.epoch + Duration::from_millis(at));
        let alert = self.alert_until.filter(|_| self.has_alert());
        let call = self
            .call
            .next_wake_ms(now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let notification = self
            .notifications
            .next_wake_ms(now_ms)
            .map(|at| self.epoch + Duration::from_millis(at));
        let media = self.media_repaint_in(unix_now_ms()).map(|wait| now + wait);
        let metrics = MotionMetrics::default();
        let frame = self
            .scene_frame_active()
            .then(|| now + Duration::from_millis(metrics.pomodoro_glow_frame_ms));
        let peek = self.peek.as_ref().map(|peek| peek.until);
        let media_peek = self.media_peek.as_ref().map(|peek| peek.until);
        let paused_hold = self
            .media_paused_at
            .map(|at| at + Duration::from_millis(metrics.media_pause_hold_ms));
        [
            pomodoro,
            timer,
            stopwatch,
            alert,
            call,
            notification,
            self.hud_until,
            media,
            frame,
            peek,
            media_peek,
            paused_hold,
            self.shelf_drag_leave_until,
            self.clipboard_copied_until,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(crate) fn panel_expansion(&self) -> f32 {
        self.panel_expansion
    }

    pub(super) fn peek_alpha(&self, peek: &ActivePeek, now: Instant) -> f32 {
        self.fade_alpha(peek.shown_at, peek.until, now)
    }

    pub(super) fn fade_alpha(&self, shown_at: Instant, until: Instant, now: Instant) -> f32 {
        if !self.motion.allows_motion() {
            return 1.0;
        }
        let entered = now.saturating_duration_since(shown_at).as_secs_f32();
        let remaining = until.saturating_duration_since(now).as_secs_f32();
        (entered.min(remaining) / PEEK_FADE_SECONDS).clamp(0.0, 1.0)
    }

    pub(super) fn show_peek(&mut self, kind: PomodoroPeek, now: Instant) {
        let hold = Duration::from_millis(MotionMetrics::default().pomodoro_peek_ms);
        self.peek = Some(ActivePeek {
            text: kind.text(&self.pomodoro),
            shown_at: now,
            until: now + hold,
        });
    }

    pub(super) fn settle(value: f32, target: f32, dt: f32, motion: MotionPreference) -> f32 {
        let rate = MotionMetrics::default().pomodoro_tone_rate;
        Self::snap_to_target(animate_value(value, target, rate, dt, motion), target)
    }

    pub(super) fn step_scene_at(&mut self, now: Instant, motion: MotionPreference) -> bool {
        let dt = now
            .saturating_duration_since(self.scene_step_at)
            .as_secs_f32()
            .clamp(0.001, 0.25);
        self.scene_step_at = now;
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let status = self.active_focus_status();
        let glow = Self::settle(
            self.glow_presence,
            flag(status == PomodoroStatus::Running),
            dt,
            motion,
        );
        let dim = Self::settle(
            self.dim_presence,
            flag(status == PomodoroStatus::Paused),
            dt,
            motion,
        );
        let pulse = Self::settle(
            self.pulse_presence,
            flag(self.active_is_last_minute()),
            dt,
            motion,
        );
        let media_dim = Self::settle(
            self.media_dim_presence,
            flag(self.media.as_ref().is_some_and(|media| !media.playing)),
            dt,
            motion,
        );
        let changed = (glow - self.glow_presence).abs() > f32::EPSILON
            || (dim - self.dim_presence).abs() > f32::EPSILON
            || (pulse - self.pulse_presence).abs() > f32::EPSILON
            || (media_dim - self.media_dim_presence).abs() > f32::EPSILON;
        self.glow_presence = glow;
        self.dim_presence = dim;
        self.pulse_presence = pulse;
        self.media_dim_presence = media_dim;
        changed
    }

    pub(super) fn scene_frame_active(&self) -> bool {
        let panel_live_media = self.panel_expansion > GLOW_SCENE_VISIBLE
            && self.notch_tab == NotchExpandedTab::Media
            && self
                .media
                .as_ref()
                .is_some_and(|media| media.playing && !media.has_known_duration());
        let countdown_running = self.pomodoro.status == PomodoroStatus::Running
            || self.quick_timer.status == PomodoroStatus::Running;
        let ringing = self.notification_stage(self.last_anim_instant)
            == Some(otto_core::NotificationStage::Ringing);
        let coalescing = self
            .notifications
            .coalesce_progress(self.millis(self.last_anim_instant))
            < 0.999;
        self.presented
            && self.motion.allows_motion()
            && (self.compact_expansion > GLOW_SCENE_VISIBLE
                || self.panel_expansion > GLOW_SCENE_VISIBLE)
            && (countdown_running
                || self.peek.is_some()
                || self.media_peek.is_some()
                || panel_live_media
                || self.call_shown
                || ringing
                || coalescing)
    }

    pub(crate) fn anim_phase(&self) -> f32 {
        self.anim_phase
    }

    pub(crate) fn advance(
        &mut self,
        now: Instant,
        cursor_inside_notch: bool,
        panel_visible: bool,
        motion: MotionPreference,
    ) -> NotchUpdate {
        let dt = now
            .saturating_duration_since(self.last_anim_instant)
            .as_secs_f32()
            .clamp(0.001, 0.050);
        self.last_anim_instant = now;
        self.motion = motion;
        self.anim_phase = (self.anim_phase + dt) % 1000.0;

        let (mut changed, effect) = self.step_pomodoro_clock(now);
        if self.step_scene_at(now, motion) {
            changed = true;
        }
        if cursor_inside_notch && self.promote_notification_peek(now) {
            changed = true;
        }
        if self
            .notifications
            .tick(self.millis(now), cursor_inside_notch)
        {
            changed = true;
        }
        self.step_hover_tracking(now, cursor_inside_notch, panel_visible);

        let hud_active = self.hud_until.is_some_and(|until| now < until);
        let inputs = self.island_inputs(now);
        if let Some(primary) = inputs.compact_content() {
            self.last_primary_slot = Some(primary.kind());
        }
        if let Some(secondary) = inputs.secondary_compact_content() {
            self.last_secondary_slot = Some(secondary.kind());
        } else {
            self.media_primary = false;
        }
        let targets = island_targets(&inputs);
        let target_hud = targets.hud;

        if self.step_morph_blends(dt, motion) {
            changed = true;
        }

        if self.step_hud(hud_active, target_hud, dt, motion) {
            changed = true;
        }

        let metrics = MotionMetrics::default();
        let press_target = if self.press_active && self.panel_expansion < 0.25 {
            1.0
        } else {
            0.0
        };
        for (value, velocity, target, response, damping) in [
            (
                &mut self.compact_expansion,
                &mut self.compact_velocity,
                targets.compact,
                metrics.island_compact_response,
                metrics.island_compact_damping,
            ),
            (
                &mut self.secondary_expansion,
                &mut self.secondary_velocity,
                targets.secondary,
                metrics.island_secondary_response,
                metrics.island_secondary_damping,
            ),
            (
                &mut self.panel_expansion,
                &mut self.panel_velocity,
                targets.panel,
                metrics.island_expand_response,
                metrics.island_expand_damping,
            ),
            (
                &mut self.press_progress,
                &mut self.press_velocity,
                press_target,
                metrics.island_press_response,
                metrics.island_press_damping,
            ),
        ] {
            if step_spring(value, velocity, target, response, damping, dt, motion) {
                changed = true;
            }
        }

        NotchUpdate { changed, effect }
    }

    pub(super) fn step_morph_blends(&mut self, dt: f32, motion: MotionPreference) -> bool {
        let mut changed = false;
        let morph = MotionMetrics::default();
        let next_swap_blend = animate_value(
            self.compact_swap_blend,
            1.0,
            morph.notch_tab_morph_rate,
            dt,
            motion,
        );
        let resolved_swap_blend = Self::snap_to_target(next_swap_blend, 1.0);
        if (resolved_swap_blend - self.compact_swap_blend).abs() > f32::EPSILON {
            self.compact_swap_blend = resolved_swap_blend;
            changed = true;
        }
        let lyrics_target = if self.lyrics_expanded && self.notch_tab == NotchExpandedTab::Media {
            1.0
        } else {
            0.0
        };
        let stats_target = if self.notch_tab == NotchExpandedTab::Stats && !self.has_alert() {
            1.0
        } else {
            0.0
        };
        let next_tab_blend =
            animate_value(self.tab_blend, 1.0, morph.notch_tab_morph_rate, dt, motion);
        let next_lyrics_progress = animate_value(
            self.lyrics_progress,
            lyrics_target,
            morph.notch_lyrics_morph_rate,
            dt,
            motion,
        );
        let next_stats_progress = animate_value(
            self.stats_progress,
            stats_target,
            morph.notch_lyrics_morph_rate,
            dt,
            motion,
        );
        let resolved_tab_blend = Self::snap_to_target(next_tab_blend, 1.0);
        let resolved_lyrics_progress = Self::snap_to_target(next_lyrics_progress, lyrics_target);
        let resolved_stats_progress = Self::snap_to_target(next_stats_progress, stats_target);
        if (resolved_tab_blend - self.tab_blend).abs() > f32::EPSILON {
            self.tab_blend = resolved_tab_blend;
            changed = true;
        }
        if (resolved_tab_blend - 1.0).abs() <= f32::EPSILON && self.previous_tab != self.notch_tab {
            self.previous_tab = self.notch_tab;
            changed = true;
        }
        if (resolved_lyrics_progress - self.lyrics_progress).abs() > f32::EPSILON {
            self.lyrics_progress = resolved_lyrics_progress;
            changed = true;
        }
        if (resolved_stats_progress - self.stats_progress).abs() > f32::EPSILON {
            self.stats_progress = resolved_stats_progress;
            changed = true;
        }
        if self.lyrics_is_live() {
            changed = true;
        }
        let next_title = animate_value(
            self.compact_title_points,
            self.compact_title_target,
            16.0,
            dt,
            motion,
        );
        let resolved_title = if (next_title - self.compact_title_target).abs() <= 0.25 {
            self.compact_title_target
        } else {
            next_title
        };
        if (resolved_title - self.compact_title_points).abs() > f32::EPSILON {
            self.compact_title_points = resolved_title;
            changed = true;
        }
        changed
    }

    pub(super) fn snap_to_target(value: f32, target: f32) -> f32 {
        if (value - target).abs() <= 0.002 {
            target
        } else {
            value
        }
    }

    pub(super) fn apply_tick(
        &mut self,
        tick: PomodoroTick,
        now: Instant,
    ) -> (bool, Option<NotchEffect>) {
        if tick == PomodoroTick::Completed {
            self.alert_until = Some(now + Duration::from_millis(ALERT_HOLD_MS));
            return (
                true,
                Some(NotchEffect::Platform(NotchAction::PlayPomodoroChime)),
            );
        }
        (tick != PomodoroTick::Unchanged, None)
    }

    pub(super) fn step_hover_tracking(
        &mut self,
        now: Instant,
        inside_notch: bool,
        panel_visible: bool,
    ) {
        if self.hover_since.is_none()
            && self.panel_trigger == PanelTrigger::Closed
            && !panel_visible
        {
            return;
        }
        if inside_notch {
            if self.panel_expansion > 0.55 && self.panel_trigger == PanelTrigger::Pinned {
                self.panel_trigger = PanelTrigger::PinnedHovered;
            }
            if self.panel_trigger == PanelTrigger::Closed
                && self.has_openable_panel()
                && self.notification_stage(now).is_none()
                && let Some(since) = self.hover_since
                && now.duration_since(since) >= Duration::from_millis(HOVER_DEBOUNCE_MS)
            {
                if matches!(
                    self.notch_tab,
                    NotchExpandedTab::Pomodoro | NotchExpandedTab::Media
                ) {
                    self.sync_tab_to_primary(now);
                }
                self.ensure_valid_tab();
                self.panel_trigger = PanelTrigger::Hovered;
            }
        } else {
            if self.is_panel_held_open() {
                return;
            }
            self.hover_since = None;
            self.hover_blocked = false;
            if matches!(
                self.panel_trigger,
                PanelTrigger::Hovered | PanelTrigger::PinnedHovered
            ) {
                self.panel_trigger = PanelTrigger::Closed;
            }
        }
    }
}
