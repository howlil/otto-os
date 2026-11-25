use crate::notch::focus_mode::FocusCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PomodoroPhase {
    #[default]
    Focus,
    ShortBreak,
    LongBreak,
}

impl PomodoroPhase {
    #[must_use]
    pub const fn default_minutes(self) -> u32 {
        match self {
            Self::Focus => 25,
            Self::ShortBreak => 5,
            Self::LongBreak => 15,
        }
    }

    #[must_use]
    pub const fn default_seconds(self) -> u32 {
        self.default_minutes() * 60
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Focus => "Focus",
            Self::ShortBreak => "Break",
            Self::LongBreak => "Rest",
        }
    }

    #[must_use]
    pub const fn advance_after_completion(self, completed_focus_sessions: u8) -> (Self, u8) {
        self.advance_with_cycles(completed_focus_sessions, 4)
    }

    #[must_use]
    pub const fn advance_with_cycles(
        self,
        completed_focus_sessions: u8,
        cycles_before_long: u8,
    ) -> (Self, u8) {
        let target = if cycles_before_long < 2 {
            2
        } else {
            cycles_before_long
        };
        match self {
            Self::Focus => {
                let next_count = (completed_focus_sessions % target) + 1;
                if next_count >= target {
                    (Self::LongBreak, target)
                } else {
                    (Self::ShortBreak, next_count)
                }
            }
            Self::ShortBreak => (Self::Focus, completed_focus_sessions % target),
            Self::LongBreak => (Self::Focus, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroSchedule {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub cycles_before_long_break: u8,
}

impl Default for PomodoroSchedule {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            cycles_before_long_break: 4,
        }
    }
}

impl PomodoroSchedule {
    #[must_use]
    pub fn from_category(cat: &FocusCategory) -> Self {
        Self {
            focus_minutes: cat.focus_minutes.clamp(1, POMODORO_MAX_MINUTES),
            short_break_minutes: cat.short_break_minutes.clamp(1, 60),
            long_break_minutes: cat.long_break_minutes.clamp(1, 90),
            cycles_before_long_break: cat.cycles_before_long_break.clamp(2, 8),
        }
    }

    #[must_use]
    pub const fn seconds_for(self, phase: PomodoroPhase) -> u32 {
        match phase {
            PomodoroPhase::Focus => self.focus_minutes * 60,
            PomodoroPhase::ShortBreak => self.short_break_minutes * 60,
            PomodoroPhase::LongBreak => self.long_break_minutes * 60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PomodoroStatus {
    #[default]
    Idle,
    Running,
    Paused,
    Alert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroAlert {
    pub completed_phase: PomodoroPhase,
    pub next_phase: PomodoroPhase,
}

impl PomodoroAlert {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self.completed_phase {
            PomodoroPhase::Focus => "Focus complete",
            PomodoroPhase::ShortBreak => "Break complete",
            PomodoroPhase::LongBreak => "Long break complete",
        }
    }

    #[must_use]
    pub fn subtitle(self) -> String {
        let next_minutes = self.next_phase.default_minutes();
        match (self.completed_phase, self.next_phase) {
            (PomodoroPhase::Focus, PomodoroPhase::LongBreak) => {
                format!("4 sessions done. Take a {next_minutes}-minute break?")
            }
            (PomodoroPhase::Focus, _) => {
                format!("Nice work. Take a {next_minutes}-minute break?")
            }
            (PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak, _) => {
                format!("Ready for your next {next_minutes}-minute focus?")
            }
        }
    }

    #[must_use]
    pub const fn action_label(self) -> &'static str {
        match self.next_phase {
            PomodoroPhase::Focus => "Start focus",
            PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak => "Start break",
        }
    }
}

const MINUTES_PER_DAY: u32 = 24 * 60;
pub const POMODORO_EXTEND_LABEL: &str = "+5 min";
pub const POMODORO_PRESETS_MINUTES: [u32; 4] = [15, 25, 45, 60];
pub const POMODORO_MAX_MINUTES: u32 = 180;
pub const POMODORO_LAST_MINUTE_SECONDS: u32 = 60;
pub const POMODORO_WARNING_SECONDS: u32 = 5 * 60;

#[must_use]
pub fn format_mm_ss(total_seconds: u32) -> String {
    let mins = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{mins:02}:{secs:02}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroState {
    pub phase: PomodoroPhase,
    pub status: PomodoroStatus,
    pub total_seconds: u32,
    pub remaining_seconds: u32,
    pub completed_focus_sessions: u8,
    pub alert: Option<PomodoroAlert>,
    pub started_minute: Option<u16>,
    pub schedule: PomodoroSchedule,
}

impl Default for PomodoroState {
    fn default() -> Self {
        Self::new()
    }
}

impl PomodoroState {
    #[must_use]
    pub const fn new() -> Self {
        let schedule = PomodoroSchedule {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            cycles_before_long_break: 4,
        };
        let secs = schedule.seconds_for(PomodoroPhase::Focus);
        Self {
            phase: PomodoroPhase::Focus,
            status: PomodoroStatus::Idle,
            total_seconds: secs,
            remaining_seconds: secs,
            completed_focus_sessions: 0,
            alert: None,
            started_minute: None,
            schedule,
        }
    }

    pub fn apply_schedule(&mut self, schedule: PomodoroSchedule) {
        self.schedule = schedule;
        if self.status == PomodoroStatus::Idle {
            let secs = self.schedule.seconds_for(self.phase);
            self.total_seconds = secs;
            self.remaining_seconds = secs;
        }
    }

    pub fn toggle_run(&mut self) {
        match self.status {
            PomodoroStatus::Idle | PomodoroStatus::Paused => {
                self.status = PomodoroStatus::Running;
                self.alert = None;
            }
            PomodoroStatus::Running => {
                self.status = PomodoroStatus::Paused;
            }
            PomodoroStatus::Alert => {
                self.start_next_from_alert();
            }
        }
    }

    pub fn start_next_from_alert(&mut self) {
        self.alert = None;
        self.started_minute = None;
        self.status = PomodoroStatus::Running;
    }

    pub fn dismiss_alert(&mut self) {
        if self.status == PomodoroStatus::Alert {
            self.alert = None;
            self.started_minute = None;
            self.status = PomodoroStatus::Idle;
        }
    }

    pub fn mark_started(&mut self, now_minute: u16) {
        if self.status == PomodoroStatus::Running && self.started_minute.is_none() {
            self.started_minute = Some(now_minute);
        }
    }

    pub fn set_preset(&mut self, minutes: u32) -> bool {
        if self.status != PomodoroStatus::Idle || self.phase != PomodoroPhase::Focus {
            return false;
        }
        let clamped = minutes.clamp(1, POMODORO_MAX_MINUTES);
        let seconds = clamped * 60;
        if seconds == self.remaining_seconds && seconds == self.total_seconds {
            return false;
        }
        self.schedule.focus_minutes = clamped;
        self.total_seconds = seconds;
        self.remaining_seconds = seconds;
        true
    }

    pub fn set_custom_minutes(&mut self, minutes: u32) -> bool {
        if !matches!(self.status, PomodoroStatus::Idle | PomodoroStatus::Paused) {
            return false;
        }
        let clamped = minutes.clamp(1, POMODORO_MAX_MINUTES);
        let seconds = clamped * 60;
        if seconds == self.remaining_seconds && seconds == self.total_seconds {
            return false;
        }
        if self.phase == PomodoroPhase::Focus {
            self.schedule.focus_minutes = clamped;
        }
        self.total_seconds = seconds;
        self.remaining_seconds = seconds;
        true
    }

    #[must_use]
    pub fn ends_minute(self, now_minute: u16) -> u16 {
        let remaining_minutes = self.remaining_seconds.div_ceil(60);
        ((u32::from(now_minute) + remaining_minutes) % MINUTES_PER_DAY) as u16
    }

    #[must_use]
    pub const fn is_last_minute(self) -> bool {
        matches!(self.status, PomodoroStatus::Running)
            && matches!(self.phase, PomodoroPhase::Focus)
            && self.remaining_seconds > 0
            && self.remaining_seconds <= POMODORO_LAST_MINUTE_SECONDS
    }

    pub fn extend_five_minutes(&mut self) {
        const EXTRA_SECONDS: u32 = 5 * 60;
        if let Some(alert) = self.alert.take() {
            if alert.completed_phase == PomodoroPhase::Focus && self.completed_focus_sessions > 0 {
                self.completed_focus_sessions -= 1;
            }
            self.phase = alert.completed_phase;
            self.total_seconds = EXTRA_SECONDS;
            self.remaining_seconds = EXTRA_SECONDS;
            self.status = PomodoroStatus::Running;
            self.started_minute = None;
            return;
        }
        self.remaining_seconds = self
            .remaining_seconds
            .saturating_add(EXTRA_SECONDS)
            .min(POMODORO_MAX_MINUTES * 60);
        self.total_seconds = self.total_seconds.max(self.remaining_seconds);
    }

    pub fn reset(&mut self) {
        self.alert = None;
        self.started_minute = None;
        self.status = PomodoroStatus::Idle;
        self.remaining_seconds = self.total_seconds;
    }

    pub fn skip_phase(&mut self) {
        let (next_phase, next_count) = self.phase.advance_with_cycles(
            self.completed_focus_sessions,
            self.schedule.cycles_before_long_break,
        );
        self.phase = next_phase;
        self.completed_focus_sessions = next_count;
        self.total_seconds = self.schedule.seconds_for(next_phase);
        self.remaining_seconds = self.total_seconds;
        self.status = PomodoroStatus::Idle;
        self.alert = None;
        self.started_minute = None;
    }

    pub fn adjust_minutes(&mut self, delta_minutes: i32) -> bool {
        if !matches!(self.status, PomodoroStatus::Idle | PomodoroStatus::Paused) {
            return false;
        }
        let current_minutes = self.remaining_seconds.div_ceil(60) as i32;
        let next_minutes =
            (current_minutes + delta_minutes).clamp(1, POMODORO_MAX_MINUTES as i32) as u32;
        let next_seconds = next_minutes * 60;
        if next_seconds == self.remaining_seconds && next_seconds == self.total_seconds {
            return false;
        }
        if self.phase == PomodoroPhase::Focus {
            self.schedule.focus_minutes = next_minutes;
        }
        self.total_seconds = next_seconds;
        self.remaining_seconds = next_seconds;
        true
    }

    pub fn tick_seconds(&mut self, elapsed: u32) -> bool {
        if self.status != PomodoroStatus::Running || elapsed == 0 {
            return false;
        }
        if elapsed < self.remaining_seconds {
            self.remaining_seconds -= elapsed;
            return false;
        }
        let completed_phase = self.phase;
        let (next_phase, next_count) = completed_phase.advance_with_cycles(
            self.completed_focus_sessions,
            self.schedule.cycles_before_long_break,
        );
        self.phase = next_phase;
        self.completed_focus_sessions = next_count;
        self.total_seconds = self.schedule.seconds_for(next_phase);
        self.remaining_seconds = self.total_seconds;
        self.status = PomodoroStatus::Alert;
        self.started_minute = None;
        self.alert = Some(PomodoroAlert {
            completed_phase,
            next_phase,
        });
        true
    }

    #[must_use]
    pub fn format_time(self) -> String {
        format_mm_ss(self.remaining_seconds)
    }

    #[must_use]
    pub fn progress(self) -> f32 {
        if self.total_seconds == 0 {
            return 0.0;
        }
        (self.remaining_seconds as f32 / self.total_seconds as f32).clamp(0.0, 1.0)
    }

    #[must_use]
    pub const fn is_compact_active(self) -> bool {
        matches!(
            self.status,
            PomodoroStatus::Running | PomodoroStatus::Paused
        )
    }

    #[must_use]
    pub const fn status_hint(self) -> &'static str {
        match self.status {
            PomodoroStatus::Running => "Running · Alt+P to pause",
            PomodoroStatus::Paused => "Paused · Scroll to adjust",
            PomodoroStatus::Idle => "Ready · Scroll to adjust",
            PomodoroStatus::Alert => "Session complete",
        }
    }
}

pub const POMODORO_SUSPEND_GAP_MS: u64 = 30_000;

#[must_use]
pub fn warning_threshold_crossed(before: PomodoroState, after: PomodoroState) -> bool {
    before.status == PomodoroStatus::Running
        && after.status == PomodoroStatus::Running
        && before.phase == PomodoroPhase::Focus
        && after.phase == PomodoroPhase::Focus
        && before.total_seconds > POMODORO_WARNING_SECONDS
        && before.remaining_seconds > POMODORO_WARNING_SECONDS
        && after.remaining_seconds <= POMODORO_WARNING_SECONDS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroTick {
    Unchanged,
    Advanced,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PomodoroClock {
    last_sync_ms: Option<u64>,
    carry_ms: u64,
}

impl PomodoroClock {
    pub fn restart(&mut self, now_ms: u64) {
        self.last_sync_ms = Some(now_ms);
        self.carry_ms = 0;
    }

    fn advance_carry(&mut self, running: bool, now_ms: u64) -> Option<u32> {
        let last = self.last_sync_ms.replace(now_ms)?;
        if !running {
            self.carry_ms = 0;
            return None;
        }
        let gap = now_ms.saturating_sub(last);
        if gap > POMODORO_SUSPEND_GAP_MS {
            return None;
        }
        self.carry_ms += gap;
        let whole_seconds = self.carry_ms / 1_000;
        self.carry_ms %= 1_000;
        if whole_seconds == 0 {
            return None;
        }
        Some(u32::try_from(whole_seconds).unwrap_or(u32::MAX))
    }

    pub fn sync(&mut self, state: &mut PomodoroState, now_ms: u64) -> PomodoroTick {
        let Some(elapsed) = self.advance_carry(state.status == PomodoroStatus::Running, now_ms)
        else {
            return PomodoroTick::Unchanged;
        };
        if state.tick_seconds(elapsed) {
            self.carry_ms = 0;
            PomodoroTick::Completed
        } else {
            PomodoroTick::Advanced
        }
    }

    pub fn suspend(&mut self, state: &mut PomodoroState, now_ms: u64) -> PomodoroTick {
        let tick = self.sync(state, now_ms);
        self.last_sync_ms = None;
        tick
    }

    pub fn resume(&mut self, now_ms: u64) {
        self.last_sync_ms = Some(now_ms);
    }

    #[must_use]
    pub fn next_wakeup_ms(&self, state: &PomodoroState, now_ms: u64) -> Option<u64> {
        (state.status == PomodoroStatus::Running).then(|| now_ms + (1_000 - self.carry_ms.min(999)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running() -> (PomodoroState, PomodoroClock) {
        let mut state = PomodoroState::new();
        state.toggle_run();
        let mut clock = PomodoroClock::default();
        clock.restart(0);
        (state, clock)
    }

    #[test]
    fn clock_counts_whole_seconds_regardless_of_how_often_it_is_synced() {
        let (mut coarse, mut coarse_clock) = running();
        assert_eq!(
            coarse_clock.sync(&mut coarse, 10_400),
            PomodoroTick::Advanced
        );
        let (mut fine, mut fine_clock) = running();
        for now in (8..=10_400).step_by(8) {
            let _ = fine_clock.sync(&mut fine, now);
        }
        assert_eq!(coarse.remaining_seconds, 25 * 60 - 10);
        assert_eq!(fine.remaining_seconds, coarse.remaining_seconds);
    }

    #[test]
    fn clock_keeps_sub_second_remainder_between_syncs() {
        let (mut state, mut clock) = running();
        assert_eq!(clock.sync(&mut state, 600), PomodoroTick::Unchanged);
        assert_eq!(clock.sync(&mut state, 1_200), PomodoroTick::Advanced);
        assert_eq!(state.remaining_seconds, 25 * 60 - 1);
        assert_eq!(clock.next_wakeup_ms(&state, 1_200), Some(2_000));
    }

    #[test]
    fn paused_or_idle_timer_does_not_accumulate_time() {
        let (mut state, mut clock) = running();
        state.toggle_run();
        assert_eq!(clock.sync(&mut state, 20_000), PomodoroTick::Unchanged);
        assert_eq!(clock.next_wakeup_ms(&state, 20_000), None);
        state.toggle_run();
        clock.restart(20_000);
        assert_eq!(clock.sync(&mut state, 21_000), PomodoroTick::Advanced);
        assert_eq!(state.remaining_seconds, 25 * 60 - 1);
    }

    #[test]
    fn system_sleep_does_not_consume_timer_time() {
        let (mut state, mut clock) = running();
        assert_eq!(clock.suspend(&mut state, 5_000), PomodoroTick::Advanced);
        assert_eq!(clock.sync(&mut state, 3_600_000), PomodoroTick::Unchanged);
        clock.resume(3_600_000);
        assert_eq!(clock.sync(&mut state, 3_602_000), PomodoroTick::Advanced);
        assert_eq!(state.remaining_seconds, 25 * 60 - 7);
    }

    #[test]
    fn unannounced_long_gap_is_treated_as_sleep() {
        let (mut state, mut clock) = running();
        assert_eq!(
            clock.sync(&mut state, POMODORO_SUSPEND_GAP_MS + 1),
            PomodoroTick::Unchanged
        );
        assert_eq!(state.remaining_seconds, 25 * 60);
        assert_eq!(
            clock.sync(&mut state, POMODORO_SUSPEND_GAP_MS + 2_001),
            PomodoroTick::Advanced
        );
        assert_eq!(state.remaining_seconds, 25 * 60 - 2);
    }

    #[test]
    fn clock_reports_completion_exactly_once() {
        let (mut state, mut clock) = running();
        let mut now = 0;
        let mut completions = 0;
        while now < 26 * 60 * 1_000 {
            now += 20_000;
            if clock.sync(&mut state, now) == PomodoroTick::Completed {
                completions += 1;
            }
        }
        assert_eq!(completions, 1);
        assert_eq!(state.status, PomodoroStatus::Alert);
    }

    #[test]
    fn new_state_starts_at_25_minute_focus_idle() {
        let state = PomodoroState::new();
        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert_eq!(state.status, PomodoroStatus::Idle);
        assert_eq!(state.remaining_seconds, 25 * 60);
        assert_eq!(state.format_time(), "25:00");
        assert!(!state.is_compact_active());
    }

    #[test]
    fn completing_four_focus_sessions_transitions_to_long_break() {
        let mut state = PomodoroState::new();
        for expected_session in 1..=3 {
            state.toggle_run();
            assert!(state.tick_seconds(25 * 60));
            assert_eq!(state.status, PomodoroStatus::Alert);
            assert_eq!(state.phase, PomodoroPhase::ShortBreak);
            assert_eq!(state.completed_focus_sessions, expected_session);
            state.toggle_run();
            assert!(state.tick_seconds(5 * 60));
            assert_eq!(state.phase, PomodoroPhase::Focus);
        }
        state.toggle_run();
        assert!(state.tick_seconds(25 * 60));
        assert_eq!(state.status, PomodoroStatus::Alert);
        assert_eq!(state.phase, PomodoroPhase::LongBreak);
        assert_eq!(state.completed_focus_sessions, 4);
        assert_eq!(state.remaining_seconds, 15 * 60);

        state.toggle_run();
        assert!(state.tick_seconds(15 * 60));
        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert_eq!(state.completed_focus_sessions, 0);
    }

    #[test]
    fn scroll_adjustment_clamps_between_one_and_max_minutes_when_not_running() {
        let mut state = PomodoroState::new();
        assert!(state.adjust_minutes(5));
        assert_eq!(state.remaining_seconds, 30 * 60);
        assert!(state.adjust_minutes(-100));
        assert_eq!(state.remaining_seconds, 60);
        assert!(state.adjust_minutes(250));
        assert_eq!(state.remaining_seconds, POMODORO_MAX_MINUTES * 60);

        state.toggle_run();
        assert!(!state.adjust_minutes(5));
        assert_eq!(state.remaining_seconds, POMODORO_MAX_MINUTES * 60);
    }

    #[test]
    fn presets_apply_only_to_an_idle_focus_session() {
        let mut state = PomodoroState::new();
        for minutes in POMODORO_PRESETS_MINUTES {
            assert!(state.set_preset(minutes));
            assert_eq!(state.remaining_seconds, minutes * 60);
            assert_eq!(state.total_seconds, minutes * 60);
        }
        assert!(!state.set_preset(60));
        state.toggle_run();
        assert!(!state.set_preset(15));
        assert_eq!(state.remaining_seconds, 60 * 60);

        let mut on_break = PomodoroState::new();
        on_break.skip_phase();
        assert!(!on_break.set_preset(15));
        assert_eq!(on_break.remaining_seconds, 5 * 60);
    }

    #[test]
    fn started_minute_is_recorded_once_and_cleared_when_the_session_changes() {
        let mut state = PomodoroState::new();
        state.mark_started(100);
        assert_eq!(state.started_minute, None);
        state.toggle_run();
        state.mark_started(100);
        state.mark_started(200);
        assert_eq!(state.started_minute, Some(100));
        state.toggle_run();
        state.toggle_run();
        assert_eq!(state.started_minute, Some(100));
        state.reset();
        assert_eq!(state.started_minute, None);

        state.toggle_run();
        state.mark_started(300);
        assert!(state.tick_seconds(25 * 60));
        assert_eq!(state.started_minute, None);
    }

    #[test]
    fn warning_fires_once_when_five_minutes_remain_in_a_long_enough_focus() {
        let mut state = PomodoroState::new();
        state.toggle_run();
        let before = state;
        state.remaining_seconds = POMODORO_WARNING_SECONDS;
        assert!(warning_threshold_crossed(before, state));
        let after = state;
        state.remaining_seconds -= 1;
        assert!(!warning_threshold_crossed(after, state));

        let mut short = PomodoroState::new();
        short.adjust_minutes(-22);
        short.toggle_run();
        let before = short;
        short.remaining_seconds = 100;
        assert!(!warning_threshold_crossed(before, short));
    }

    #[test]
    fn alert_copy_follows_the_completed_and_next_phase() {
        let mut state = PomodoroState::new();
        state.toggle_run();
        assert!(state.tick_seconds(25 * 60));
        let alert = state.alert.expect("alert");
        assert_eq!(alert.title(), "Focus complete");
        assert_eq!(alert.subtitle(), "Nice work. Take a 5-minute break?");
        assert_eq!(alert.action_label(), "Start break");
        state.toggle_run();
        assert!(state.tick_seconds(5 * 60));
        let alert = state.alert.expect("alert");
        assert_eq!(alert.subtitle(), "Ready for your next 25-minute focus?");
        assert_eq!(alert.action_label(), "Start focus");
    }

    #[test]
    fn extend_five_minutes_from_alert_resumes_completed_phase() {
        let mut state = PomodoroState::new();
        state.toggle_run();
        assert!(state.tick_seconds(25 * 60));
        assert_eq!(state.status, PomodoroStatus::Alert);
        assert_eq!(state.completed_focus_sessions, 1);

        state.extend_five_minutes();
        assert_eq!(state.status, PomodoroStatus::Running);
        assert_eq!(state.phase, PomodoroPhase::Focus);
        assert_eq!(state.remaining_seconds, 5 * 60);
        assert_eq!(state.completed_focus_sessions, 0);
    }
}
