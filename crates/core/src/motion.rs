use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPreference {
    Full,
    Reduced,
}

impl MotionPreference {
    #[must_use]
    pub const fn from_animations_enabled(enabled: bool) -> Self {
        if enabled { Self::Full } else { Self::Reduced }
    }

    #[must_use]
    pub const fn allows_motion(self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionMetrics {
    pub dock_magnify_rate: f32,
    pub dock_hide_rate: f32,
    pub dock_presence_enter_rate: f32,
    pub dock_presence_exit_rate: f32,
    pub window_controls_follow_rate: f32,
    pub traffic_follow_rate: f32,
    pub genie_minimize_duration_seconds: f32,
    pub genie_restore_duration_seconds: f32,
    pub bounce_duration_seconds: f32,
    pub animation_tick_ms: u32,
}

impl MotionMetrics {
    pub const MACOS_LIKE: Self = Self {
        dock_magnify_rate: 22.0,
        dock_hide_rate: 16.0,
        dock_presence_enter_rate: 18.0,
        dock_presence_exit_rate: 20.0,
        window_controls_follow_rate: 38.0,
        traffic_follow_rate: 38.0,
        genie_minimize_duration_seconds: 0.42,
        genie_restore_duration_seconds: 0.36,
        bounce_duration_seconds: 0.44,
        animation_tick_ms: 16,
    };
}

impl Default for MotionMetrics {
    fn default() -> Self {
        Self::MACOS_LIKE
    }
}

#[must_use]
pub fn animate_value(
    current: f32,
    target: f32,
    rate: f32,
    dt: f32,
    preference: MotionPreference,
) -> f32 {
    if preference.allows_motion() {
        super::dock::smooth_damp(current, target, rate, dt)
    } else {
        target
    }
}

#[must_use]
pub fn advance_bounce_phase(
    phase: f32,
    dt: f32,
    duration_seconds: f32,
    preference: MotionPreference,
) -> f32 {
    if !preference.allows_motion() {
        return 0.0;
    }
    (phase + dt / duration_seconds.max(0.001)).min(1.0)
}

#[must_use]
pub fn bounce_offset(phase: f32) -> f32 {
    let progress = phase.clamp(0.0, 1.0);
    if progress >= 1.0 {
        return 0.0;
    }
    let wave = (progress * PI * 2.0).sin().abs();
    wave * (1.0 - progress * 0.45)
}

#[must_use]
pub fn advance_genie_progress(
    progress: f32,
    dt: f32,
    duration_seconds: f32,
    preference: MotionPreference,
) -> f32 {
    if !preference.allows_motion() {
        return 1.0;
    }
    (progress + dt / duration_seconds.max(0.001)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() <= f32::EPSILON);
    }

    #[test]
    fn reduced_motion_settles_without_intermediate_frames() {
        assert_close(
            animate_value(0.2, 1.0, 20.0, 0.008, MotionPreference::Reduced),
            1.0,
        );
        assert_close(
            advance_bounce_phase(0.4, 0.008, 0.44, MotionPreference::Reduced),
            0.0,
        );
    }

    #[test]
    fn full_motion_moves_toward_target_without_overshoot() {
        let next = animate_value(0.0, 1.0, 20.0, 0.008, MotionPreference::Full);
        assert!(next > 0.0);
        assert!(next < 1.0);
    }

    #[test]
    fn motion_reverses_from_current_progress_without_resetting() {
        let opening = animate_value(0.0, 1.0, 20.0, 0.050, MotionPreference::Full);
        let closing = animate_value(opening, 0.0, 20.0, 0.008, MotionPreference::Full);
        let reopening = animate_value(closing, 1.0, 20.0, 0.008, MotionPreference::Full);
        assert!(opening > 0.0);
        assert!(closing < opening);
        assert!(reopening > closing);
        assert!(reopening < 1.0);
    }

    #[test]
    fn bounce_reaches_rest_at_its_duration() {
        assert_close(
            advance_bounce_phase(0.99, 0.44, 0.44, MotionPreference::Full),
            1.0,
        );
        assert_close(bounce_offset(1.0), 0.0);
    }

    #[test]
    fn genie_progress_reaches_rest_at_its_duration() {
        assert_close(
            advance_genie_progress(0.9, 0.52, 0.52, MotionPreference::Full),
            1.0,
        );
        assert_close(
            advance_genie_progress(0.4, 0.008, 0.52, MotionPreference::Reduced),
            1.0,
        );
    }
}
