use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use otto_core::{
    FocusBlocklist, FocusCategoryStore, FocusSessionStatus, FocusStatsRange, HeatmapFilter,
    HitTarget, MotionMetrics, NOTCH_STATS_DAYS, POMODORO_PRESETS_MINUTES, PomodoroPeek,
    PomodoroPeekFrame, PomodoroPhase, PomodoroScene, PomodoroSchedule, PomodoroStatus,
    PomodoroSurface, PomodoroTick, warning_threshold_crossed,
};

use super::GLOW_BREATH_FLOOR;
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

    pub(crate) fn set_focus_categories(&mut self, store: FocusCategoryStore) {
        let schedule = PomodoroSchedule::from_category(store.active());
        self.focus_categories = store;
        self.pomodoro.apply_schedule(schedule);
        if let HeatmapFilter::Category(id) = &self.heatmap_filter
            && self.focus_categories.by_id(id).is_none()
        {
            self.heatmap_filter = HeatmapFilter::All;
        }
    }

    pub(crate) fn set_focus_relief_summary(&mut self, summary: String) {
        self.focus_relief_summary = summary;
    }

    pub(crate) fn active_category_id(&self) -> &str {
        &self.focus_categories.active_category_id
    }

    pub(crate) const fn stats_range(&self) -> FocusStatsRange {
        self.stats_range
    }

    pub(crate) const fn stats_selected_day(&self) -> Option<usize> {
        self.stats_selected_day
    }

    pub(crate) const fn heatmap_filter(&self) -> &HeatmapFilter {
        &self.heatmap_filter
    }

    pub(crate) const fn heatmap_selected_cell(&self) -> Option<usize> {
        self.heatmap_selected_cell
    }

    pub(crate) const fn is_editing_custom_time(&self) -> bool {
        self.custom_time_input.is_some()
    }

    pub(crate) fn handle_custom_time_char(&mut self, ch: char) -> NotchUpdate {
        let Some(buf) = self.custom_time_input.as_mut() else {
            return NotchUpdate::default();
        };
        if !ch.is_ascii_digit() || buf.len() >= 3 {
            return NotchUpdate::default();
        }
        buf.push(ch);
        NotchUpdate {
            changed: true,
            effect: None,
        }
    }

    pub(crate) fn handle_custom_time_backspace(&mut self) -> NotchUpdate {
        let Some(buf) = self.custom_time_input.as_mut() else {
            return NotchUpdate::default();
        };
        if buf.pop().is_some() {
            NotchUpdate {
                changed: true,
                effect: None,
            }
        } else {
            NotchUpdate::default()
        }
    }

    pub(crate) fn commit_custom_time(&mut self) -> NotchUpdate {
        let Some(raw) = self.custom_time_input.take() else {
            return NotchUpdate::default();
        };
        let Ok(minutes) = raw.parse::<u32>() else {
            return NotchUpdate {
                changed: true,
                effect: None,
            };
        };
        let applied = self.pomodoro.set_custom_minutes(minutes);
        let effect = (applied && self.pomodoro.phase == PomodoroPhase::Focus).then(|| {
            let clamped = self.pomodoro.schedule.focus_minutes;
            self.focus_categories.active_mut().focus_minutes = clamped;
            NotchEffect::UpdateFocusDuration {
                category_id: self.focus_categories.active_category_id.clone(),
                minutes: clamped,
            }
        });
        NotchUpdate {
            changed: true,
            effect,
        }
    }

    pub(crate) fn cancel_custom_time(&mut self) -> NotchUpdate {
        if self.custom_time_input.take().is_some() {
            NotchUpdate {
                changed: true,
                effect: None,
            }
        } else {
            NotchUpdate::default()
        }
    }

    pub(super) fn handle_focus_stats_click(&mut self, target: HitTarget) -> NotchUpdate {
        match target {
            HitTarget::FocusStatsRange(range) => {
                if self.stats_range == range {
                    return NotchUpdate::default();
                }
                self.stats_range = range;
                self.stats_selected_day = None;
                self.heatmap_selected_cell = None;
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            HitTarget::FocusStatsDay(index) => {
                let idx = usize::from(index);
                if idx >= NOTCH_STATS_DAYS {
                    return NotchUpdate::default();
                }
                self.stats_selected_day = if self.stats_selected_day == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            HitTarget::FocusHeatmapFilter(index) => {
                let next = if index == 0 {
                    HeatmapFilter::All
                } else if let Some(cat) =
                    self.focus_categories.categories.get(usize::from(index - 1))
                {
                    HeatmapFilter::Category(cat.id.clone())
                } else {
                    return NotchUpdate::default();
                };
                if self.heatmap_filter == next {
                    return NotchUpdate::default();
                }
                self.heatmap_filter = next;
                self.heatmap_selected_cell = None;
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            HitTarget::FocusHeatmapCell(index) => {
                let idx = usize::from(index);
                self.heatmap_selected_cell = if self.heatmap_selected_cell == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                NotchUpdate {
                    changed: true,
                    effect: None,
                }
            }
            HitTarget::FocusStatsOpenSettings => {
                self.panel_trigger = PanelTrigger::Closed;
                self.hover_since = None;
                self.hover_blocked = true;
                NotchUpdate {
                    changed: true,
                    effect: Some(NotchEffect::OpenFocusActivitySettings),
                }
            }
            _ => NotchUpdate::default(),
        }
    }

    pub(crate) const fn is_focus_blocking_active(&self) -> bool {
        self.focus_enabled
            && matches!(self.pomodoro.status, PomodoroStatus::Running)
            && matches!(self.pomodoro.phase, PomodoroPhase::Focus)
    }

    pub(crate) const fn focus_session_status(&self) -> FocusSessionStatus {
        if !self.focus_enabled {
            return FocusSessionStatus::NotRunning;
        }
        if matches!(self.pomodoro.status, PomodoroStatus::Running)
            && matches!(self.pomodoro.phase, PomodoroPhase::Focus)
        {
            return FocusSessionStatus::RunningCountdown {
                remaining_seconds: self.pomodoro.remaining_seconds,
            };
        }
        if matches!(self.pomodoro.status, PomodoroStatus::Running) {
            return FocusSessionStatus::OnBreak {
                remaining_seconds: self.pomodoro.remaining_seconds,
            };
        }
        if matches!(self.pomodoro.status, PomodoroStatus::Paused)
            && matches!(self.pomodoro.phase, PomodoroPhase::Focus)
        {
            return FocusSessionStatus::PausedCountdown {
                remaining_seconds: self.pomodoro.remaining_seconds,
            };
        }
        FocusSessionStatus::NotRunning
    }

    pub(super) const fn active_focus_status(&self) -> PomodoroStatus {
        self.pomodoro.status
    }

    pub(super) const fn active_is_last_minute(&self) -> bool {
        self.pomodoro.is_last_minute()
    }

    pub(super) const fn active_surface(&self) -> PomodoroSurface {
        PomodoroSurface::of_state(&self.pomodoro)
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
        let active = self.focus_categories.active();
        let categories = self
            .focus_categories
            .categories
            .iter()
            .take(otto_core::NOTCH_CATEGORY_OPTIONS)
            .map(|c| (c.id.clone(), c.name.clone(), c.color))
            .collect();
        PomodoroScene {
            state: self.pomodoro,
            clock: self.clock_time,
            glow: self.glow_presence * (GLOW_BREATH_FLOOR + (1.0 - GLOW_BREATH_FLOOR) * breath),
            dim: self.dim_presence,
            pulse: self.pulse_presence * wave,
            peek: self.peek.as_ref().map(|peek| PomodoroPeekFrame {
                text: peek.text.clone(),
                alpha: self.peek_alpha(peek, now),
            }),
            category_id: active.id.clone(),
            category_name: active.name.clone(),
            category_color: active.color,
            category_icon: active.icon,
            policy_summary: active.policy_summary(),
            relief_summary: if self.is_focus_blocking_active() {
                self.focus_relief_summary.clone()
            } else {
                String::new()
            },
            category_picker_open: self.category_picker_open,
            categories,
            custom_time_input: self.custom_time_input.clone(),
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

    fn handle_category_or_duration_click(&mut self, target: HitTarget) -> Option<NotchUpdate> {
        match target {
            HitTarget::PomodoroOpenSettings => Some(NotchUpdate {
                changed: false,
                effect: Some(NotchEffect::OpenFocusModeSettings),
            }),
            HitTarget::PomodoroCategoryPill => {
                self.custom_time_input = None;
                self.category_picker_open = !self.category_picker_open;
                Some(NotchUpdate {
                    changed: true,
                    effect: None,
                })
            }
            HitTarget::PomodoroCategoryOption(index) => {
                let Some(cat_id) = self
                    .focus_categories
                    .categories
                    .get(usize::from(index))
                    .map(|cat| cat.id.clone())
                else {
                    return Some(NotchUpdate::default());
                };
                self.category_picker_open = false;
                self.custom_time_input = None;
                let selected = self.focus_categories.select(&cat_id);
                if selected {
                    let schedule = PomodoroSchedule::from_category(self.focus_categories.active());
                    self.pomodoro.apply_schedule(schedule);
                }
                Some(NotchUpdate {
                    changed: true,
                    effect: selected.then_some(NotchEffect::SelectFocusCategory(cat_id)),
                })
            }
            HitTarget::PomodoroDurationDec | HitTarget::PomodoroDurationInc => {
                let delta = if target == HitTarget::PomodoroDurationDec {
                    -5
                } else {
                    5
                };
                self.custom_time_input = None;
                if !self.pomodoro.adjust_minutes(delta) {
                    return Some(NotchUpdate::default());
                }
                let effect = (self.pomodoro.phase == PomodoroPhase::Focus).then(|| {
                    let mins = self.pomodoro.schedule.focus_minutes;
                    self.focus_categories.active_mut().focus_minutes = mins;
                    NotchEffect::UpdateFocusDuration {
                        category_id: self.focus_categories.active_category_id.clone(),
                        minutes: mins,
                    }
                });
                Some(NotchUpdate {
                    changed: true,
                    effect,
                })
            }
            HitTarget::PomodoroTimeEdit => {
                if !matches!(
                    self.pomodoro.status,
                    PomodoroStatus::Idle | PomodoroStatus::Paused
                ) {
                    return Some(NotchUpdate::default());
                }
                self.category_picker_open = false;
                self.custom_time_input = Some(String::new());
                Some(NotchUpdate {
                    changed: true,
                    effect: None,
                })
            }
            _ => None,
        }
    }

    pub(super) fn handle_pomodoro_click(&mut self, target: HitTarget) -> NotchUpdate {
        self.clear_focus_dismissed();
        let now = Instant::now();
        let now_ms = self.millis(now);
        if let Some(update) = self.handle_category_or_duration_click(target) {
            if update.changed {
                self.clock.restart(now_ms);
            }
            return update;
        }
        let previous = self.pomodoro.status;
        let mut effect = None;
        match target {
            HitTarget::PomodoroPlayPause => {
                self.category_picker_open = false;
                self.custom_time_input = None;
                self.pomodoro.toggle_run();
                self.note_pomodoro_transition(previous, now);
            }
            HitTarget::PomodoroReset => {
                self.category_picker_open = false;
                self.custom_time_input = None;
                self.pomodoro.reset();
                self.peek = None;
            }
            HitTarget::PomodoroSkip => {
                self.category_picker_open = false;
                self.custom_time_input = None;
                self.pomodoro.skip_phase();
                self.peek = None;
            }
            HitTarget::PomodoroPreset(index) => {
                self.custom_time_input = None;
                let Some(minutes) = POMODORO_PRESETS_MINUTES.get(usize::from(index)) else {
                    return NotchUpdate::default();
                };
                if !self.pomodoro.set_preset(*minutes) {
                    return NotchUpdate::default();
                }
                self.focus_categories.active_mut().focus_minutes = *minutes;
                effect = Some(NotchEffect::UpdateFocusDuration {
                    category_id: self.focus_categories.active_category_id.clone(),
                    minutes: *minutes,
                });
            }
            HitTarget::PomodoroStartNext => {
                self.category_picker_open = false;
                self.custom_time_input = None;
                self.pomodoro.start_next_from_alert();
                self.alert_until = None;
                self.note_pomodoro_transition(previous, now);
            }
            HitTarget::PomodoroExtendFive => {
                self.category_picker_open = false;
                self.custom_time_input = None;
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
            effect,
        }
    }

    pub(super) fn step_pomodoro_clock(&mut self, now: Instant) -> (bool, Option<NotchEffect>) {
        let now_ms = self.millis(now);
        let before = self.pomodoro;
        let tick = self.clock.sync(&mut self.pomodoro, now_ms);
        let (mut changed, mut effect) = self.apply_tick(tick, now);
        if tick == PomodoroTick::Completed && before.phase == PomodoroPhase::Focus {
            self.focus_relief_summary.clear();
            if effect.is_none() {
                effect = Some(NotchEffect::RecordCompletedSession {
                    category_id: self.focus_categories.active_category_id.clone(),
                });
            }
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
