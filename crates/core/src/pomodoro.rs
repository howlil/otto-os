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
    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::Focus => "FOCUS",
            Self::ShortBreak => "SHORT BREAK",
            Self::LongBreak => "LONG BREAK",
        }
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
    pub const fn cycle_next(self) -> Self {
        match self {
            Self::Focus => Self::ShortBreak,
            Self::ShortBreak => Self::LongBreak,
            Self::LongBreak => Self::Focus,
        }
    }

    #[must_use]
    pub const fn advance_after_completion(self, completed_focus_sessions: u8) -> (Self, u8) {
        match self {
            Self::Focus => {
                let next_count = (completed_focus_sessions % 4) + 1;
                if next_count >= 4 {
                    (Self::LongBreak, 4)
                } else {
                    (Self::ShortBreak, next_count)
                }
            }
            Self::ShortBreak => (Self::Focus, completed_focus_sessions % 4),
            Self::LongBreak => (Self::Focus, 0),
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
            PomodoroPhase::Focus => "Focus Complete",
            PomodoroPhase::ShortBreak => "Break Complete",
            PomodoroPhase::LongBreak => "Long Break Complete",
        }
    }

    #[must_use]
    pub const fn subtitle(self) -> &'static str {
        match (self.completed_phase, self.next_phase) {
            (PomodoroPhase::Focus, PomodoroPhase::LongBreak) => {
                "4 sessions done · Take a 15m break"
            }
            (PomodoroPhase::Focus, _) => "Time for a 5m short break",
            (PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak, _) => {
                "Ready for your next 25m focus"
            }
        }
    }

    #[must_use]
    pub const fn action_label(self) -> &'static str {
        match self.next_phase {
            PomodoroPhase::Focus => "Start Focus",
            PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak => "Start Break",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroState {
    pub phase: PomodoroPhase,
    pub status: PomodoroStatus,
    pub total_seconds: u32,
    pub remaining_seconds: u32,
    pub completed_focus_sessions: u8,
    pub alert: Option<PomodoroAlert>,
}

impl Default for PomodoroState {
    fn default() -> Self {
        Self::new()
    }
}

impl PomodoroState {
    #[must_use]
    pub const fn new() -> Self {
        let secs = PomodoroPhase::Focus.default_seconds();
        Self {
            phase: PomodoroPhase::Focus,
            status: PomodoroStatus::Idle,
            total_seconds: secs,
            remaining_seconds: secs,
            completed_focus_sessions: 0,
            alert: None,
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
        self.status = PomodoroStatus::Running;
    }

    pub fn dismiss_alert(&mut self) {
        if self.status == PomodoroStatus::Alert {
            self.alert = None;
            self.status = PomodoroStatus::Idle;
        }
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
            return;
        }
        self.remaining_seconds = self
            .remaining_seconds
            .saturating_add(EXTRA_SECONDS)
            .min(90 * 60);
        self.total_seconds = self.total_seconds.max(self.remaining_seconds);
    }

    pub fn reset(&mut self) {
        self.alert = None;
        self.status = PomodoroStatus::Idle;
        self.remaining_seconds = self.total_seconds;
    }

    pub fn skip_phase(&mut self) {
        let (next_phase, next_count) = self
            .phase
            .advance_after_completion(self.completed_focus_sessions);
        self.phase = next_phase;
        self.completed_focus_sessions = next_count;
        self.total_seconds = next_phase.default_seconds();
        self.remaining_seconds = self.total_seconds;
        self.status = PomodoroStatus::Idle;
        self.alert = None;
    }

    pub fn cycle_mode(&mut self) {
        let next_phase = self.phase.cycle_next();
        self.phase = next_phase;
        self.total_seconds = next_phase.default_seconds();
        self.remaining_seconds = self.total_seconds;
        self.status = PomodoroStatus::Idle;
        self.alert = None;
    }

    pub fn adjust_minutes(&mut self, delta_minutes: i32) -> bool {
        if !matches!(self.status, PomodoroStatus::Idle | PomodoroStatus::Paused) {
            return false;
        }
        let current_minutes = self.remaining_seconds.div_ceil(60) as i32;
        let next_minutes = (current_minutes + delta_minutes).clamp(1, 90) as u32;
        let next_seconds = next_minutes * 60;
        if next_seconds == self.remaining_seconds && next_seconds == self.total_seconds {
            return false;
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
        let (next_phase, next_count) =
            completed_phase.advance_after_completion(self.completed_focus_sessions);
        self.phase = next_phase;
        self.completed_focus_sessions = next_count;
        self.total_seconds = next_phase.default_seconds();
        self.remaining_seconds = self.total_seconds;
        self.status = PomodoroStatus::Alert;
        self.alert = Some(PomodoroAlert {
            completed_phase,
            next_phase,
        });
        true
    }

    #[must_use]
    pub fn format_time(self) -> String {
        let mins = self.remaining_seconds / 60;
        let secs = self.remaining_seconds % 60;
        format!("{mins:02}:{secs:02}")
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn scroll_adjustment_clamps_between_one_and_ninety_minutes_when_not_running() {
        let mut state = PomodoroState::new();
        assert!(state.adjust_minutes(5));
        assert_eq!(state.remaining_seconds, 30 * 60);
        assert!(state.adjust_minutes(-100));
        assert_eq!(state.remaining_seconds, 60);
        assert!(state.adjust_minutes(200));
        assert_eq!(state.remaining_seconds, 90 * 60);

        state.toggle_run();
        assert!(!state.adjust_minutes(5));
        assert_eq!(state.remaining_seconds, 90 * 60);
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
