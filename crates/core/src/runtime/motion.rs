use std::f32::consts::{PI, TAU};

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
    pub genie_minimize_duration_seconds: f32,
    pub genie_restore_duration_seconds: f32,
    pub genie_slow_motion_scale: f32,
    pub bounce_duration_seconds: f32,
    pub notch_tab_morph_rate: f32,
    pub notch_lyrics_morph_rate: f32,
    pub control_center_morph_rate: f32,
    pub spotlight_morph_rate: f32,
    pub hud_expand_rate: f32,
    pub hud_meter_rate: f32,
    pub hud_content_start: f32,
    pub hud_content_end: f32,
    pub hud_hold_ms: u64,
    pub pomodoro_peek_ms: u64,
    pub media_pause_hold_ms: u64,
    pub media_live_drift_seconds: f32,
    pub pomodoro_glow_frame_ms: u64,
    pub pomodoro_glow_breathe_seconds: f32,
    pub pomodoro_last_minute_pulse_seconds: f32,
    pub pomodoro_tone_rate: f32,
    pub animation_tick_ms: u32,
    pub island_expand_response: f32,
    pub island_expand_damping: f32,
    pub island_compact_response: f32,
    pub island_compact_damping: f32,
    pub island_secondary_response: f32,
    pub island_secondary_damping: f32,
    pub island_press_response: f32,
    pub island_press_damping: f32,
}

impl MotionMetrics {
    #[must_use]
    pub fn genie_duration_seconds(self, slow_motion: bool, is_restore: bool) -> f32 {
        let base = if is_restore {
            self.genie_restore_duration_seconds
        } else {
            self.genie_minimize_duration_seconds
        };
        if slow_motion {
            base * self.genie_slow_motion_scale.max(1.0)
        } else {
            base
        }
    }

    pub const MACOS_LIKE: Self = Self {
        dock_magnify_rate: 22.0,
        dock_hide_rate: 16.0,
        dock_presence_enter_rate: 18.0,
        dock_presence_exit_rate: 20.0,
        genie_minimize_duration_seconds: 0.42,
        genie_restore_duration_seconds: 0.36,
        genie_slow_motion_scale: 4.0,
        bounce_duration_seconds: 0.44,
        notch_tab_morph_rate: 22.0,
        notch_lyrics_morph_rate: 18.0,
        control_center_morph_rate: 22.0,
        spotlight_morph_rate: 22.0,
        hud_expand_rate: 18.0,
        hud_meter_rate: 22.0,
        hud_content_start: 0.30,
        hud_content_end: 0.80,
        hud_hold_ms: 1_650,
        pomodoro_peek_ms: 2_500,
        media_pause_hold_ms: 8_000,
        media_live_drift_seconds: 3.0,
        pomodoro_glow_frame_ms: 80,
        pomodoro_glow_breathe_seconds: 4.0,
        pomodoro_last_minute_pulse_seconds: 1.0,
        pomodoro_tone_rate: 14.0,
        animation_tick_ms: 16,
        island_expand_response: 0.30,
        island_expand_damping: 0.84,
        island_compact_response: 0.26,
        island_compact_damping: 0.86,
        island_secondary_response: 0.32,
        island_secondary_damping: 0.78,
        island_press_response: 0.12,
        island_press_damping: 0.92,
    };
}

impl Default for MotionMetrics {
    fn default() -> Self {
        Self::MACOS_LIKE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpringState {
    pub value: f32,
    pub velocity: f32,
}

impl SpringState {
    pub fn step(
        &mut self,
        target: f32,
        response_seconds: f32,
        damping_ratio: f32,
        dt: f32,
        preference: MotionPreference,
    ) -> bool {
        step_spring(
            &mut self.value,
            &mut self.velocity,
            target,
            response_seconds,
            damping_ratio,
            dt,
            preference,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub fn step_spring(
    value: &mut f32,
    velocity: &mut f32,
    target: f32,
    response_seconds: f32,
    damping_ratio: f32,
    dt: f32,
    preference: MotionPreference,
) -> bool {
    if !preference.allows_motion() || response_seconds <= 0.001 {
        let changed = (*value - target).abs() > f32::EPSILON || velocity.abs() > f32::EPSILON;
        *value = target;
        *velocity = 0.0;
        return changed;
    }
    let prev_value = *value;
    let prev_velocity = *velocity;
    let dt = dt.clamp(0.001, 0.050);
    let substeps = ((dt / 0.002).ceil() as usize).clamp(1, 64);
    let h = dt / substeps as f32;
    let omega_n = TAU / response_seconds.max(0.01);
    let omega_sq = omega_n * omega_n;
    let two_zeta_omega = 2.0 * damping_ratio * omega_n;
    let mut x = *value;
    let mut v = *velocity;
    for _ in 0..substeps {
        let accel = omega_sq * (target - x) - two_zeta_omega * v;
        v += accel * h;
        x += v * h;
    }
    if (x - target).abs() <= 0.002 && v.abs() <= 0.015 {
        *value = target;
        *velocity = 0.0;
        (prev_value - target).abs() > f32::EPSILON || prev_velocity.abs() > f32::EPSILON
    } else {
        *value = x;
        *velocity = v;
        true
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
        crate::dock::layout::smooth_damp(current, target, rate, dt)
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
    fn under_damped_spring_overshoots_target_when_damping_below_one() {
        let mut spring = SpringState::default();
        let mut max_value = 0.0_f32;
        for _ in 0..40 {
            let _ = spring.step(1.0, 0.30, 0.65, 0.016, MotionPreference::Full);
            max_value = max_value.max(spring.value);
        }
        assert!(max_value > 1.0);
    }

    #[test]
    fn spring_preserves_momentum_on_mid_flight_target_reversal() {
        let mut spring = SpringState::default();
        for _ in 0..5 {
            assert!(spring.step(1.0, 0.30, 0.84, 0.016, MotionPreference::Full));
        }
        let velocity_before = spring.velocity;
        let value_before = spring.value;
        assert!(velocity_before > 0.0);
        assert!(spring.step(0.0, 0.30, 0.84, 0.004, MotionPreference::Full));
        assert!(spring.value > value_before);
        assert!(spring.velocity < velocity_before);
    }

    #[test]
    fn spring_settles_to_exact_target_under_600ms_and_reports_settled() {
        let mut spring = SpringState::default();
        let mut elapsed = 0.0_f32;
        while elapsed < 0.60 {
            let _ = spring.step(1.0, 0.30, 0.84, 0.016, MotionPreference::Full);
            elapsed += 0.016;
        }
        assert_close(spring.value, 1.0);
        assert_close(spring.velocity, 0.0);
        assert!(!spring.step(1.0, 0.30, 0.84, 0.016, MotionPreference::Full));
    }

    #[test]
    fn reduced_motion_spring_snaps_immediately_with_zero_velocity() {
        let mut spring = SpringState {
            value: 0.25,
            velocity: 3.5,
        };
        assert!(spring.step(1.0, 0.30, 0.84, 0.016, MotionPreference::Reduced));
        assert_close(spring.value, 1.0);
        assert_close(spring.velocity, 0.0);
        assert!(!spring.step(1.0, 0.30, 0.84, 0.016, MotionPreference::Reduced));
    }
}
