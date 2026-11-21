use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use otto_core::{
    FocusBlocklist, FocusSubMode, HitTarget, MotionMetrics, NotchExpandedTab,
    POMODORO_PRESETS_MINUTES, PomodoroPeek, PomodoroPeekFrame, PomodoroPhase, PomodoroScene,
    PomodoroStatus, PomodoroSurface, PomodoroTick, TIMER_PRESETS_MINUTES,
    warning_threshold_crossed,
};

use super::GLOW_BREATH_FLOOR;
use super::GLOW_SCENE_VISIBLE;
use super::NotchController;
use super::NotchEffect;
use super::NotchUpdate;
use super::PanelTrigger;
#[cfg(test)]
use otto_core::PomodoroState;

impl NotchController {
    pub(crate) fn set_focus_blocklist(&mut self, blocklist: FocusBlocklist) {
        self.focus_blocklist = blocklist;
    }

    pub(crate) const fn is_focus_blocking_active(&self) -> bool {
        self.focus_enabled
            && ((matches!(self.pomodoro.status, PomodoroStatus::Running)
                && matches!(self.pomodoro.phase, PomodoroPhase::Focus))
                || matches!(self.quick_timer.status, PomodoroStatus::Running)
                || self.stopwatch.running)
    }

    pub(super) fn active_focus_status(&self) -> PomodoroStatus {
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.pomodoro.status,
            FocusSubMode::Timer => self.quick_timer.status,
            FocusSubMode::Stopwatch => self.stopwatch.status(),
        }
    }

    pub(super) fn active_is_last_minute(&self) -> bool {
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.pomodoro.is_last_minute(),
            FocusSubMode::Timer => self.quick_timer.is_last_minute(),
            FocusSubMode::Stopwatch => false,
        }
    }

    pub(super) fn is_stopwatch_visible(&self) -> bool {
        self.presented
            && ((self.panel_expansion > GLOW_SCENE_VISIBLE
                && self.notch_tab == NotchExpandedTab::Pomodoro
                && self.focus_mode == FocusSubMode::Stopwatch)
                || (self.compact_expansion > GLOW_SCENE_VISIBLE
                    && !self.media_primary
                    && self.compact_focus_mode() == Some(FocusSubMode::Stopwatch)))
    }

    pub(super) fn active_surface(&self) -> PomodoroSurface {
        match self.focus_mode {
            FocusSubMode::Pomodoro => PomodoroSurface::of_state(&self.pomodoro),
            FocusSubMode::Timer => PomodoroSurface::of_timer_state(&self.quick_timer),
            FocusSubMode::Stopwatch => PomodoroSurface::of_stopwatch_state(&self.stopwatch),
        }
    }

    #[cfg(test)]
    pub(crate) fn pomodoro(&self) -> PomodoroState {
        self.pomodoro
    }

    pub(crate) fn pomodoro_scene(&self, now: Instant) -> PomodoroScene {
        let metrics = MotionMetrics::default();
        let animated = self.motion.allows_motion();
        let seconds = self.millis(now) as f32 / 1_000.0;
        let breath = if animated {
            0.5 - 0.5 * (TAU * seconds / metrics.pomodoro_glow_breathe_seconds).cos()
        } else {
            1.0
        };
        let wave = if animated {
            0.5 + 0.5 * (TAU * seconds / metrics.pomodoro_last_minute_pulse_seconds).sin()
        } else {
            1.0
        };
        PomodoroScene {
            state: self.pomodoro,
            timer: self.quick_timer,
            stopwatch: self.stopwatch,
            focus_mode: self.focus_mode,
            compact_mode: self.compact_focus_mode().unwrap_or(self.focus_mode),
            clock: self.clock_time,
            glow: self.glow_presence * (GLOW_BREATH_FLOOR + (1.0 - GLOW_BREATH_FLOOR) * breath),
            dim: self.dim_presence,
            pulse: self.pulse_presence * wave,
            peek: self.peek.as_ref().map(|peek| PomodoroPeekFrame {
                text: peek.text.clone(),
                alpha: self.peek_alpha(peek, now),
            }),
        }
    }

    pub(super) fn note_pomodoro_transition(&mut self, previous: PomodoroStatus, now: Instant) {
        self.pomodoro.mark_started(self.clock_time.minute_of_day());
        match (previous, self.pomodoro.status) {
            (
                PomodoroStatus::Idle | PomodoroStatus::Paused | PomodoroStatus::Alert,
                PomodoroStatus::Running,
            ) => self.show_peek(PomodoroPeek::Started, now),
            (PomodoroStatus::Running, PomodoroStatus::Paused) => {
                self.show_peek(PomodoroPeek::Paused, now);
            }
            _ => {}
        }
    }

    pub(super) fn handle_focus_mode_click(&mut self, mode: FocusSubMode) -> NotchUpdate {
        if self.focus_mode == mode {
            return NotchUpdate::default();
        }
        let now_ms = self.millis(Instant::now());
        if mode == FocusSubMode::Stopwatch {
            self.stopwatch_clock.sync(&mut self.stopwatch, now_ms, true);
        }
        self.focus_mode = mode;
        self.peek = None;
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(super) fn handle_pomodoro_click(&mut self, target: HitTarget) -> NotchUpdate {
        self.clear_focus_dismissed();
        let now = Instant::now();
        let now_ms = self.millis(now);
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.handle_pomodoro_mode_click(target, now, now_ms),
            FocusSubMode::Timer => self.handle_timer_mode_click(target, now_ms),
            FocusSubMode::Stopwatch => self.handle_stopwatch_mode_click(target, now_ms),
        }
    }

    pub(super) fn handle_pomodoro_mode_click(
        &mut self,
        target: HitTarget,
        now: Instant,
        now_ms: u64,
    ) -> NotchUpdate {
        let previous = self.pomodoro.status;
        match target {
            HitTarget::PomodoroPlayPause => {
                self.pomodoro.toggle_run();
                self.note_pomodoro_transition(previous, now);
            }
            HitTarget::PomodoroReset => {
                self.pomodoro.reset();
                self.peek = None;
            }
            HitTarget::PomodoroSkip => {
                self.pomodoro.skip_phase();
                self.peek = None;
            }
            HitTarget::PomodoroPreset(index) => {
                let Some(minutes) = POMODORO_PRESETS_MINUTES.get(usize::from(index)) else {
                    return NotchUpdate::default();
                };
                if !self.pomodoro.set_preset(*minutes) {
                    return NotchUpdate::default();
                }
            }
            HitTarget::PomodoroStartNext => {
                self.pomodoro.start_next_from_alert();
                self.alert_until = None;
                self.note_pomodoro_transition(previous, now);
            }
            HitTarget::PomodoroExtendFive => {
                self.pomodoro.extend_five_minutes();
                self.alert_until = None;
                self.pomodoro.mark_started(self.clock_time.minute_of_day());
                self.show_peek(PomodoroPeek::Extended, now);
            }
            _ => return NotchUpdate::default(),
        }
        self.clock.restart(now_ms);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(super) fn handle_timer_mode_click(
        &mut self,
        target: HitTarget,
        now_ms: u64,
    ) -> NotchUpdate {
        match target {
            HitTarget::PomodoroPlayPause => self.quick_timer.toggle_run(),
            HitTarget::PomodoroReset => self.quick_timer.reset(),
            HitTarget::PomodoroPreset(index) => {
                let Some(minutes) = TIMER_PRESETS_MINUTES.get(usize::from(index)) else {
                    return NotchUpdate::default();
                };
                if !self.quick_timer.set_preset(*minutes) {
                    return NotchUpdate::default();
                }
            }
            HitTarget::PomodoroStartNext => {
                self.quick_timer.start_next_from_alert();
                self.alert_until = None;
            }
            HitTarget::PomodoroExtendFive => {
                self.quick_timer.extend_five_minutes();
                self.alert_until = None;
            }
            _ => return NotchUpdate::default(),
        }
        self.timer_clock.restart(now_ms);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(super) fn handle_stopwatch_mode_click(
        &mut self,
        target: HitTarget,
        now_ms: u64,
    ) -> NotchUpdate {
        self.stopwatch_clock.sync(&mut self.stopwatch, now_ms, true);
        match target {
            HitTarget::PomodoroPlayPause => self.stopwatch.toggle_run(),
            HitTarget::PomodoroReset => self.stopwatch.reset(),
            HitTarget::PomodoroSkip => {
                if !self.stopwatch.record_lap() {
                    return NotchUpdate::default();
                }
            }
            _ => return NotchUpdate::default(),
        }
        self.stopwatch_clock.restart(now_ms);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(super) fn step_pomodoro_clock(&mut self, now: Instant) -> (bool, Option<NotchEffect>) {
        let now_ms = self.millis(now);
        let before = self.pomodoro;
        let tick = self.clock.sync(&mut self.pomodoro, now_ms);
        let (mut changed, mut effect) = self.apply_tick(tick, now);
        if tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Pomodoro;
        }
        let timer_tick = self.timer_clock.sync_timer(&mut self.quick_timer, now_ms);
        let (timer_changed, timer_effect) = self.apply_tick(timer_tick, now);
        if timer_tick == PomodoroTick::Completed {
            self.focus_mode = FocusSubMode::Timer;
        }
        changed |= timer_changed;
        if effect.is_none() {
            effect = timer_effect;
        }
        let sw_visible = self.is_stopwatch_visible();
        if self
            .stopwatch_clock
            .sync(&mut self.stopwatch, now_ms, sw_visible)
            && sw_visible
        {
            changed = true;
        }
        if warning_threshold_crossed(before, self.pomodoro) {
            self.show_peek(PomodoroPeek::FiveMinutesLeft, now);
            changed = true;
        }
        if self.peek.as_ref().is_some_and(|peek| now >= peek.until) {
            self.peek = None;
            changed = true;
        }
        if self
            .media_peek
            .as_ref()
            .is_some_and(|peek| now >= peek.until)
        {
            self.media_peek = None;
            changed = true;
        }
        let pause_hold = Duration::from_millis(MotionMetrics::default().media_pause_hold_ms);
        if self
            .media_paused_at
            .is_some_and(|paused_at| now >= paused_at + pause_hold)
        {
            self.media_paused_at = None;
            changed = true;
        }
        if self.hud_until.is_some_and(|until| now >= until) {
            self.hud_until = None;
            changed = true;
        }
        if self.has_alert()
            && let Some(until) = self.alert_until
            && now >= until
            && self.panel_trigger == PanelTrigger::Closed
        {
            self.dismiss_active_alert();
            changed = true;
        }
        if self.step_shelf_drag_leave(now) {
            changed = true;
        }
        if self.step_clipboard_feedback(now) {
            changed = true;
        }
        if !self.has_compact_focus() {
            self.clear_focus_dismissed();
        }
        (changed, effect)
    }
}
