use crate::bar::status::ClockTime;
use crate::notch::focus_mode::{CategoryColor, CategoryIcon};
use crate::notch::pomodoro::{
    POMODORO_PRESETS_MINUTES, PomodoroAlert, PomodoroPhase, PomodoroState, PomodoroStatus,
};

pub const POMODORO_SESSIONS_PER_CYCLE: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroTone {
    Focus,
    ShortBreak,
    LongBreak,
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

fn cycle_progress(state: &PomodoroState) -> [f32; 4] {
    let elapsed = 1.0 - state.progress();
    let completed = usize::from(
        state
            .completed_focus_sessions
            .min(POMODORO_SESSIONS_PER_CYCLE),
    );
    let current = usize::from(
        state
            .completed_focus_sessions
            .min(POMODORO_SESSIONS_PER_CYCLE - 1),
    );
    let mut segments = [0.0; 4];
    for (index, fill) in segments.iter_mut().enumerate() {
        *fill = if index < completed {
            1.0
        } else if state.phase == PomodoroPhase::Focus && index == current {
            elapsed
        } else {
            0.0
        }
    }
    segments
}

fn progress(state: &PomodoroState) -> PomodoroProgress {
    let elapsed = 1.0 - state.progress();
    match state.phase {
        PomodoroPhase::Focus => PomodoroProgress::Segments(cycle_progress(state)),
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
    pub clock: ClockTime,
    pub glow: f32,
    pub dim: f32,
    pub pulse: f32,
    pub peek: Option<PomodoroPeekFrame>,
    pub category_id: String,
    pub category_name: String,
    pub category_color: CategoryColor,
    pub category_icon: CategoryIcon,
    pub policy_summary: String,
    pub relief_summary: String,
    pub category_picker_open: bool,
    pub categories: Vec<(String, String, CategoryColor)>,
    pub custom_time_input: Option<String>,
}

impl PomodoroScene {
    #[must_use]
    pub fn steady(state: PomodoroState, clock: ClockTime) -> Self {
        Self {
            state,
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
            category_id: String::from("coding"),
            category_name: String::from("Coding"),
            category_color: CategoryColor::Orange,
            category_icon: CategoryIcon::Code,
            policy_summary: String::new(),
            relief_summary: String::new(),
            category_picker_open: false,
            categories: Vec::new(),
            custom_time_input: None,
        }
    }

    #[must_use]
    pub fn view(&self) -> PomodoroView {
        let mut view = PomodoroView::resolve(&self.state, self.clock);
        if let Some(raw) = &self.custom_time_input {
            view.headline = if raw.is_empty() {
                String::from("__:00")
            } else {
                format!("{raw}:00")
            };
            view.subtitle = String::from("Type 1-180 min · Enter to apply");
        } else if view.surface == PomodoroSurface::Setup && !self.policy_summary.is_empty() {
            view.subtitle.clone_from(&self.policy_summary);
        } else if matches!(
            view.surface,
            PomodoroSurface::Running | PomodoroSurface::Paused
        ) && self.state.phase == PomodoroPhase::Focus
            && !self.category_name.is_empty()
        {
            if view.surface == PomodoroSurface::Running {
                let ends = format_minute_of_day(self.state.ends_minute(self.clock.minute_of_day()));
                view.subtitle = if self.state.is_last_minute() {
                    format!("{} · almost done · ends {ends}", self.category_name)
                } else {
                    format!("{} · ends {ends}", self.category_name)
                };
            }
            if !self.relief_summary.is_empty() {
                view.footer_right.clone_from(&self.relief_summary);
            }
        }
        view
    }

    #[must_use]
    pub fn compact_cycle_progress(&self) -> [f32; 4] {
        cycle_progress(&self.state)
    }

    #[must_use]
    pub fn compact_ring(&self) -> (f32, PomodoroStatus, String) {
        (
            self.state.progress(),
            self.state.status,
            self.state.format_time(),
        )
    }

    #[must_use]
    pub const fn presets_minutes(&self) -> [u32; 4] {
        POMODORO_PRESETS_MINUTES
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notch::pomodoro::POMODORO_EXTEND_LABEL;

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
}
