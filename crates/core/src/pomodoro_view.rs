use crate::pomodoro::{
    FocusSubMode, POMODORO_PRESETS_MINUTES, PomodoroAlert, PomodoroPhase, PomodoroState,
    PomodoroStatus, QuickTimerState, StopwatchState, TIMER_PRESETS_MINUTES, format_duration_ms,
};
use crate::status::ClockTime;

pub const POMODORO_SESSIONS_PER_CYCLE: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroTone {
    Focus,
    ShortBreak,
    LongBreak,
    Timer,
    Stopwatch,
}

impl PomodoroTone {
    #[must_use]
    pub const fn of_phase(phase: PomodoroPhase) -> Self {
        match phase {
            PomodoroPhase::Focus => Self::Focus,
            PomodoroPhase::ShortBreak => Self::ShortBreak,
            PomodoroPhase::LongBreak => Self::LongBreak,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PomodoroSurface {
    #[default]
    Setup,
    Ready,
    Running,
    Paused,
    Alert,
}

impl PomodoroSurface {
    #[must_use]
    pub const fn of_state(state: &PomodoroState) -> Self {
        match state.status {
            PomodoroStatus::Alert => Self::Alert,
            PomodoroStatus::Running => Self::Running,
            PomodoroStatus::Paused => Self::Paused,
            PomodoroStatus::Idle => match state.phase {
                PomodoroPhase::Focus => Self::Setup,
                PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak => Self::Ready,
            },
        }
    }

    #[must_use]
    pub const fn of_timer_state(state: &QuickTimerState) -> Self {
        match state.status {
            PomodoroStatus::Alert => Self::Alert,
            PomodoroStatus::Running => Self::Running,
            PomodoroStatus::Paused => Self::Paused,
            PomodoroStatus::Idle => Self::Setup,
        }
    }

    #[must_use]
    pub const fn of_stopwatch_state(state: &StopwatchState) -> Self {
        match state.status() {
            PomodoroStatus::Running => Self::Running,
            PomodoroStatus::Paused => Self::Paused,
            PomodoroStatus::Idle | PomodoroStatus::Alert => Self::Ready,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PomodoroProgress {
    Segments([f32; 4]),
    Bar(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PomodoroView {
    pub surface: PomodoroSurface,
    pub tone: PomodoroTone,
    pub dimmed: bool,
    pub urgent: bool,
    pub headline: String,
    pub subtitle: String,
    pub footer_left: String,
    pub footer_right: String,
    pub session_label: String,
    pub progress: PomodoroProgress,
}

impl PomodoroView {
    #[must_use]
    pub fn resolve(state: &PomodoroState, now: ClockTime) -> Self {
        let surface = PomodoroSurface::of_state(state);
        let tone_phase = state.alert.map_or(state.phase, |alert| alert.next_phase);
        Self {
            surface,
            tone: PomodoroTone::of_phase(tone_phase),
            dimmed: surface == PomodoroSurface::Paused,
            urgent: state.is_last_minute(),
            headline: headline(state, surface),
            subtitle: subtitle(state, surface, now),
            footer_left: footer_left(state, surface),
            footer_right: footer_right(state, surface),
            session_label: session_label(state, surface),
            progress: progress(state),
        }
    }

    #[must_use]
    pub fn resolve_timer(state: &QuickTimerState, now: ClockTime) -> Self {
        let surface = PomodoroSurface::of_timer_state(state);
        let ends = || format_minute_of_day(state.ends_minute(now.minute_of_day()));
        let headline = if surface == PomodoroSurface::Alert {
            String::from("Timer complete")
        } else {
            state.format_time()
        };
        let subtitle = match surface {
            PomodoroSurface::Setup | PomodoroSurface::Ready => {
                String::from("Pick a quick timer or scroll to adjust")
            }
            PomodoroSurface::Running if state.is_last_minute() => {
                format!("Almost done · ends {}", ends())
            }
            PomodoroSurface::Running => format!("Countdown · ends {}", ends()),
            PomodoroSurface::Paused => {
                format!("Paused · {} min left", state.remaining_seconds.div_ceil(60))
            }
            PomodoroSurface::Alert => String::from("Your quick timer finished"),
        };
        let (footer_left, footer_right) =
            if matches!(surface, PomodoroSurface::Running | PomodoroSurface::Paused) {
                (
                    String::from("Quick countdown"),
                    String::from("Alerts at 00:00"),
                )
            } else {
                (String::new(), String::new())
            };
        Self {
            surface,
            tone: PomodoroTone::Timer,
            dimmed: surface == PomodoroSurface::Paused,
            urgent: state.is_last_minute(),
            headline,
            subtitle,
            footer_left,
            footer_right,
            session_label: String::from("Timer"),
            progress: PomodoroProgress::Bar(1.0 - state.progress()),
        }
    }

    #[must_use]
    pub fn resolve_stopwatch(state: &StopwatchState) -> Self {
        let surface = PomodoroSurface::of_stopwatch_state(state);
        let subtitle = match surface {
            PomodoroSurface::Running => match state.last_lap_ms {
                Some(lap_ms) => format!(
                    "Running · Lap {} at {}",
                    state.lap_count,
                    format_duration_ms(lap_ms)
                ),
                None => String::from("Counting up"),
            },
            PomodoroSurface::Paused => {
                if state.lap_count > 0 {
                    format!("Paused · {} laps recorded", state.lap_count)
                } else {
                    String::from("Paused · Press play to resume")
                }
            }
            _ => String::from("Ready · Start counting up"),
        };
        let footer_left = state.last_lap_ms.map_or_else(String::new, |lap_ms| {
            format!("Last lap {}", format_duration_ms(lap_ms))
        });
        let footer_right = if state.lap_count > 0 {
            format!("{} laps", state.lap_count)
        } else {
            String::new()
        };
        let session_label = if state.lap_count > 0 {
            format!("Lap {}", state.lap_count)
        } else {
            String::from("Stopwatch")
        };
        Self {
            surface,
            tone: PomodoroTone::Stopwatch,
            dimmed: surface == PomodoroSurface::Paused,
            urgent: false,
            headline: state.format_time(),
            subtitle,
            footer_left,
            footer_right,
            session_label,
            progress: PomodoroProgress::Bar(state.progress()),
        }
    }
}

#[must_use]
pub fn format_minute_of_day(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60 % 24, minute % 60)
}

fn headline(state: &PomodoroState, surface: PomodoroSurface) -> String {
    match (surface, state.alert) {
        (PomodoroSurface::Alert, Some(alert)) => alert.title().to_owned(),
        _ => state.format_time(),
    }
}

fn subtitle(state: &PomodoroState, surface: PomodoroSurface, now: ClockTime) -> String {
    let ends = || format_minute_of_day(state.ends_minute(now.minute_of_day()));
    match surface {
        PomodoroSurface::Setup => String::from("Pick a length or scroll to fine-tune"),
        PomodoroSurface::Ready => match state.phase {
            PomodoroPhase::LongBreak => String::from("Ready for a long break"),
            PomodoroPhase::Focus | PomodoroPhase::ShortBreak => String::from("Ready for a break"),
        },
        PomodoroSurface::Running => match state.phase {
            PomodoroPhase::Focus if state.is_last_minute() => {
                format!("Almost there · ends {}", ends())
            }
            PomodoroPhase::Focus => format!("Deep focus · ends {}", ends()),
            PomodoroPhase::ShortBreak => format!("Break · back to focus at {}", ends()),
            PomodoroPhase::LongBreak => String::from("Long break · you earned it"),
        },
        PomodoroSurface::Paused => {
            format!("Paused · {} min left", state.remaining_seconds.div_ceil(60))
        }
        PomodoroSurface::Alert => state
            .alert
            .map_or_else(String::new, PomodoroAlert::subtitle),
    }
}

fn footer_left(state: &PomodoroState, surface: PomodoroSurface) -> String {
    match (surface, state.phase) {
        (PomodoroSurface::Setup | PomodoroSurface::Alert, _) => String::new(),
        (_, PomodoroPhase::Focus) => state.started_minute.map_or_else(String::new, |minute| {
            format!("Started {}", format_minute_of_day(minute))
        }),
        (_, PomodoroPhase::ShortBreak) => String::from("Look away from the screen for a moment"),
        (_, PomodoroPhase::LongBreak) => String::from("Stand up, stretch, refill your water"),
    }
}

fn footer_right(state: &PomodoroState, surface: PomodoroSurface) -> String {
    match (surface, state.phase) {
        (PomodoroSurface::Setup | PomodoroSurface::Alert, _) | (_, PomodoroPhase::ShortBreak) => {
            String::new()
        }
        (_, PomodoroPhase::Focus) => {
            if state.completed_focus_sessions.saturating_add(1) >= POMODORO_SESSIONS_PER_CYCLE {
                String::from("Long break next")
            } else {
                String::from("Short break next")
            }
        }
        (_, PomodoroPhase::LongBreak) => String::from("Then a fresh cycle"),
    }
}

fn session_label(state: &PomodoroState, surface: PomodoroSurface) -> String {
    let completed = state
        .completed_focus_sessions
        .min(POMODORO_SESSIONS_PER_CYCLE);
    if surface == PomodoroSurface::Alert {
        return format!("{completed} of {POMODORO_SESSIONS_PER_CYCLE} done");
    }
    match state.phase {
        PomodoroPhase::Focus => format!(
            "Session {} of {POMODORO_SESSIONS_PER_CYCLE}",
            completed.min(POMODORO_SESSIONS_PER_CYCLE - 1) + 1
        ),
        PomodoroPhase::ShortBreak => {
            format!("Session {completed} of {POMODORO_SESSIONS_PER_CYCLE}")
        }
        PomodoroPhase::LongBreak => {
            format!("{POMODORO_SESSIONS_PER_CYCLE} of {POMODORO_SESSIONS_PER_CYCLE} done")
        }
    }
}

fn progress(state: &PomodoroState) -> PomodoroProgress {
    let elapsed = 1.0 - state.progress();
    match state.phase {
        PomodoroPhase::Focus => {
            let current = usize::from(
                state
                    .completed_focus_sessions
                    .min(POMODORO_SESSIONS_PER_CYCLE - 1),
            );
            let mut segments = [0.0; 4];
            for (index, fill) in segments.iter_mut().enumerate() {
                *fill = match index.cmp(&current) {
                    std::cmp::Ordering::Less => 1.0,
                    std::cmp::Ordering::Equal => elapsed,
                    std::cmp::Ordering::Greater => 0.0,
                };
            }
            PomodoroProgress::Segments(segments)
        }
        PomodoroPhase::ShortBreak | PomodoroPhase::LongBreak => PomodoroProgress::Bar(elapsed),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroPeek {
    Started,
    Paused,
    Extended,
    FiveMinutesLeft,
}

impl PomodoroPeek {
    #[must_use]
    pub fn text(self, state: &PomodoroState) -> String {
        match self {
            Self::Started => format!(
                "{} · {} min",
                state.phase.short_label(),
                state.total_seconds.div_ceil(60)
            ),
            Self::Paused => String::from("Paused"),
            Self::Extended => String::from("+5 min added"),
            Self::FiveMinutesLeft => String::from("5 minutes left"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PomodoroPeekFrame {
    pub text: String,
    pub alpha: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PomodoroScene {
    pub state: PomodoroState,
    pub timer: QuickTimerState,
    pub stopwatch: StopwatchState,
    pub focus_mode: FocusSubMode,
    pub compact_mode: FocusSubMode,
    pub clock: ClockTime,
    pub glow: f32,
    pub dim: f32,
    pub pulse: f32,
    pub peek: Option<PomodoroPeekFrame>,
}

impl PomodoroScene {
    #[must_use]
    pub fn steady(state: PomodoroState, clock: ClockTime) -> Self {
        Self {
            state,
            timer: QuickTimerState::new(),
            stopwatch: StopwatchState::new(),
            focus_mode: FocusSubMode::Pomodoro,
            compact_mode: FocusSubMode::Pomodoro,
            clock,
            glow: if state.status == PomodoroStatus::Running {
                1.0
            } else {
                0.0
            },
            dim: if state.status == PomodoroStatus::Paused {
                1.0
            } else {
                0.0
            },
            pulse: if state.is_last_minute() { 1.0 } else { 0.0 },
            peek: None,
        }
    }

    #[must_use]
    pub fn view_for_mode(&self, mode: FocusSubMode) -> PomodoroView {
        match mode {
            FocusSubMode::Pomodoro => PomodoroView::resolve(&self.state, self.clock),
            FocusSubMode::Timer => PomodoroView::resolve_timer(&self.timer, self.clock),
            FocusSubMode::Stopwatch => PomodoroView::resolve_stopwatch(&self.stopwatch),
        }
    }

    #[must_use]
    pub fn view(&self) -> PomodoroView {
        self.view_for_mode(self.focus_mode)
    }

    #[must_use]
    pub fn compact_view(&self) -> PomodoroView {
        self.view_for_mode(self.compact_mode)
    }

    #[must_use]
    pub fn compact_ring(&self) -> (f32, PomodoroStatus, String) {
        match self.compact_mode {
            FocusSubMode::Pomodoro => (
                self.state.progress(),
                self.state.status,
                self.state.format_time(),
            ),
            FocusSubMode::Timer => (
                self.timer.progress(),
                self.timer.status,
                self.timer.format_time(),
            ),
            FocusSubMode::Stopwatch => (
                self.stopwatch.progress(),
                self.stopwatch.status(),
                self.stopwatch.format_time(),
            ),
        }
    }

    #[must_use]
    pub const fn active_status(&self) -> PomodoroStatus {
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.state.status,
            FocusSubMode::Timer => self.timer.status,
            FocusSubMode::Stopwatch => self.stopwatch.status(),
        }
    }

    #[must_use]
    pub const fn presets_minutes(&self) -> Option<[u32; 4]> {
        match self.focus_mode {
            FocusSubMode::Pomodoro => Some(POMODORO_PRESETS_MINUTES),
            FocusSubMode::Timer => Some(TIMER_PRESETS_MINUTES),
            FocusSubMode::Stopwatch => None,
        }
    }

    #[must_use]
    pub const fn selected_preset_seconds(&self) -> u32 {
        match self.focus_mode {
            FocusSubMode::Pomodoro => self.state.total_seconds,
            FocusSubMode::Timer => self.timer.total_seconds,
            FocusSubMode::Stopwatch => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pomodoro::POMODORO_EXTEND_LABEL;

    fn at(hour: u8, minute: u8) -> ClockTime {
        ClockTime {
            hour,
            minute,
            ..ClockTime::default()
        }
    }

    fn running_at(now: ClockTime) -> PomodoroState {
        let mut state = PomodoroState::new();
        state.toggle_run();
        state.mark_started(now.minute_of_day());
        state
    }

    #[test]
    fn idle_focus_is_the_setup_surface_and_idle_break_is_ready() {
        let mut state = PomodoroState::new();
        assert_eq!(PomodoroSurface::of_state(&state), PomodoroSurface::Setup);
        state.skip_phase();
        assert_eq!(PomodoroSurface::of_state(&state), PomodoroSurface::Ready);
        state.toggle_run();
        assert_eq!(PomodoroSurface::of_state(&state), PomodoroSurface::Running);
        state.toggle_run();
        assert_eq!(PomodoroSurface::of_state(&state), PomodoroSurface::Paused);
    }

    #[test]
    fn running_focus_reports_end_time_and_start_time() {
        let now = at(12, 23);
        let view = PomodoroView::resolve(&running_at(now), now);
        assert_eq!(view.headline, "25:00");
        assert_eq!(view.subtitle, "Deep focus · ends 12:48");
        assert_eq!(view.footer_left, "Started 12:23");
        assert_eq!(view.footer_right, "Short break next");
        assert_eq!(view.session_label, "Session 1 of 4");
        assert!(!view.urgent && !view.dimmed);
    }

    #[test]
    fn end_time_wraps_past_midnight() {
        let now = at(23, 50);
        let state = running_at(now);
        assert_eq!(
            format_minute_of_day(state.ends_minute(now.minute_of_day())),
            "00:15"
        );
    }

    #[test]
    fn fourth_session_announces_the_long_break() {
        let now = at(9, 0);
        let mut state = running_at(now);
        state.completed_focus_sessions = 3;
        let view = PomodoroView::resolve(&state, now);
        assert_eq!(view.footer_right, "Long break next");
        assert_eq!(view.session_label, "Session 4 of 4");
    }

    #[test]
    fn focus_segments_fill_completed_sessions_and_the_current_one_partially() {
        let now = at(9, 0);
        let mut state = running_at(now);
        state.completed_focus_sessions = 2;
        state.remaining_seconds = state.total_seconds / 2;
        let PomodoroProgress::Segments(segments) = PomodoroView::resolve(&state, now).progress
        else {
            panic!("focus uses segments");
        };
        let near = |value: f32, expected: f32| (value - expected).abs() < 0.001;
        assert!(near(segments[0], 1.0));
        assert!(near(segments[1], 1.0));
        assert!(near(segments[2], 0.5));
        assert!(near(segments[3], 0.0));
    }

    #[test]
    fn breaks_use_one_continuous_bar_with_a_tip() {
        let now = at(9, 0);
        let mut state = running_at(now);
        assert!(state.tick_seconds(25 * 60));
        state.toggle_run();
        state.remaining_seconds = state.total_seconds / 4 * 3;
        let view = PomodoroView::resolve(&state, now);
        let PomodoroProgress::Bar(fill) = view.progress else {
            panic!("breaks use a bar");
        };
        assert!((fill - 0.25).abs() < 0.001);
        assert_eq!(view.tone, PomodoroTone::ShortBreak);
        assert_eq!(view.footer_left, "Look away from the screen for a moment");
        assert_eq!(view.subtitle, "Break · back to focus at 09:04");
        assert_eq!(view.session_label, "Session 1 of 4");
    }

    #[test]
    fn long_break_has_its_own_tone_and_copy() {
        let now = at(9, 0);
        let mut state = PomodoroState::new();
        state.completed_focus_sessions = 3;
        state.toggle_run();
        assert!(state.tick_seconds(25 * 60));
        state.toggle_run();
        let view = PomodoroView::resolve(&state, now);
        assert_eq!(view.tone, PomodoroTone::LongBreak);
        assert_eq!(view.subtitle, "Long break · you earned it");
        assert_eq!(view.session_label, "4 of 4 done");
        assert_eq!(view.footer_right, "Then a fresh cycle");
    }

    #[test]
    fn paused_dims_and_names_the_minutes_left() {
        let now = at(9, 0);
        let mut state = running_at(now);
        state.remaining_seconds = 12 * 60;
        state.toggle_run();
        let view = PomodoroView::resolve(&state, now);
        assert!(view.dimmed);
        assert_eq!(view.subtitle, "Paused · 12 min left");
        assert_eq!(view.footer_left, "Started 09:00");
    }

    #[test]
    fn last_minute_is_urgent_only_while_a_focus_session_runs() {
        let now = at(9, 0);
        let mut state = running_at(now);
        state.remaining_seconds = 61;
        assert!(!state.is_last_minute());
        state.remaining_seconds = 60;
        assert!(state.is_last_minute());
        assert!(PomodoroView::resolve(&state, now).urgent);
        state.toggle_run();
        assert!(!state.is_last_minute());
        let mut break_state = PomodoroState::new();
        break_state.skip_phase();
        break_state.toggle_run();
        break_state.remaining_seconds = 30;
        assert!(!break_state.is_last_minute());
    }

    #[test]
    fn alert_surface_carries_the_celebration_copy_in_the_next_tone() {
        let now = at(9, 0);
        let mut state = running_at(now);
        assert!(state.tick_seconds(25 * 60));
        let view = PomodoroView::resolve(&state, now);
        assert_eq!(view.surface, PomodoroSurface::Alert);
        assert_eq!(view.tone, PomodoroTone::ShortBreak);
        assert_eq!(view.headline, "Focus complete");
        assert_eq!(view.subtitle, "Nice work. Take a 5-minute break?");
        assert_eq!(view.session_label, "1 of 4 done");
        assert_eq!(POMODORO_EXTEND_LABEL, "+5 min");
        assert_eq!(
            state.alert.map(PomodoroAlert::action_label),
            Some("Start break")
        );
    }

    #[test]
    fn steady_scene_lights_the_glow_only_while_running_and_dims_only_while_paused() {
        let now = at(9, 0);
        let mut state = PomodoroState::new();
        let idle = PomodoroScene::steady(state, now);
        assert!(idle.glow < 0.001 && idle.dim < 0.001 && idle.peek.is_none());
        state.toggle_run();
        assert!(PomodoroScene::steady(state, now).glow > 0.999);
        state.remaining_seconds = 30;
        assert!(PomodoroScene::steady(state, now).pulse > 0.999);
        state.toggle_run();
        let paused = PomodoroScene::steady(state, now);
        assert!(paused.dim > 0.999 && paused.glow < 0.001);
        assert_eq!(paused.view().surface, PomodoroSurface::Paused);
    }

    #[test]
    fn peeks_describe_the_event_in_short_text() {
        let state = PomodoroState::new();
        assert_eq!(PomodoroPeek::Started.text(&state), "Focus · 25 min");
        assert_eq!(PomodoroPeek::Paused.text(&state), "Paused");
        assert_eq!(PomodoroPeek::Extended.text(&state), "+5 min added");
        assert_eq!(PomodoroPeek::FiveMinutesLeft.text(&state), "5 minutes left");
    }

    #[test]
    fn timer_and_stopwatch_views_reflect_their_sub_mode_state() {
        let now = at(14, 10);
        let mut scene = PomodoroScene::steady(PomodoroState::new(), now);
        scene.focus_mode = FocusSubMode::Timer;
        scene.timer.set_preset(10);
        let timer_setup = scene.view();
        assert_eq!(timer_setup.tone, PomodoroTone::Timer);
        assert_eq!(timer_setup.headline, "10:00");
        assert_eq!(timer_setup.session_label, "Timer");

        scene.timer.toggle_run();
        let timer_running = scene.view();
        assert_eq!(timer_running.subtitle, "Countdown · ends 14:20");

        scene.focus_mode = FocusSubMode::Stopwatch;
        scene.stopwatch.toggle_run();
        scene.stopwatch.advance_ms(65_000);
        scene.stopwatch.record_lap();
        let sw_view = scene.view();
        assert_eq!(sw_view.tone, PomodoroTone::Stopwatch);
        assert_eq!(sw_view.headline, "01:05");
        assert_eq!(sw_view.session_label, "Lap 1");
        assert_eq!(sw_view.footer_left, "Last lap 01:05");
        assert_eq!(sw_view.footer_right, "1 laps");
    }
}
