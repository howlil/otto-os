use std::time::{Duration, Instant};

use otto_core::{
    MotionMetrics, MotionPreference, NotchHudKind, NotchHudState, animate_value, hud_content_alpha,
};

use super::NotchController;

impl NotchController {
    pub(crate) fn hud_state(&self) -> Option<NotchHudState> {
        self.hud_kind.map(|kind| NotchHudState {
            kind,
            expansion: self.hud_expansion,
            content_alpha: hud_content_alpha(self.hud_expansion, &MotionMetrics::default()),
            level: self.hud_level,
        })
    }

    pub(crate) fn trigger_hud(&mut self, kind: NotchHudKind) {
        if !self.hud_enabled {
            return;
        }
        if self.hud_expansion < 0.05 {
            self.hud_level = kind.target_level();
        }
        self.hud_kind = Some(kind);
        self.hud_until =
            Some(Instant::now() + Duration::from_millis(MotionMetrics::default().hud_hold_ms));
    }

    pub(super) fn step_hud(
        &mut self,
        hud_active: bool,
        target_hud: f32,
        dt: f32,
        motion: MotionPreference,
    ) -> bool {
        let rates = MotionMetrics::default();
        let next_hud = animate_value(
            self.hud_expansion,
            target_hud,
            rates.hud_expand_rate,
            dt,
            motion,
        );
        let target_level = self
            .hud_kind
            .map_or(self.hud_level, NotchHudKind::target_level);
        let next_level = animate_value(
            self.hud_level,
            target_level,
            rates.hud_meter_rate,
            dt,
            motion,
        );
        let mut changed = false;
        if !hud_active && next_hud <= 0.008 {
            if self.hud_expansion != 0.0 || self.hud_kind.is_some() {
                self.hud_expansion = 0.0;
                self.hud_kind = None;
                changed = true;
            }
        } else if (next_hud - self.hud_expansion).abs() > 0.0005 {
            self.hud_expansion = next_hud;
            changed = true;
        }
        if (next_level - self.hud_level).abs() > 0.0005 {
            self.hud_level = next_level;
            changed = true;
        }
        changed
    }
}
