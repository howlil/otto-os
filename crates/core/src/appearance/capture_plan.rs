use crate::layout::Rect;
use crate::material::Appearance;

const ACTIVE_INTERVAL_MS: u32 = 33;
const ACTIVE_ON_BATTERY_INTERVAL_MS: u32 = 120;
const CALM_INTERVAL_MS: u32 = 400;
const CALM_ON_BATTERY_INTERVAL_MS: u32 = 1_000;
pub const BACKDROP_PADDING_PT: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    #[must_use]
    pub fn clipped_to_bounds(rect: Rect, bounds: Rect) -> Option<Self> {
        let left = rect.x.max(bounds.x);
        let top = rect.y.max(bounds.y);
        let right = rect.right().min(bounds.right());
        let bottom = rect.bottom().min(bounds.bottom());
        (right > left && bottom > top).then(|| Self {
            x: left,
            y: top,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }

    #[must_use]
    pub fn padded_to_bounds(rect: Rect, padding: i32, bounds: Rect) -> Option<Self> {
        let pad = padding.max(0);
        let padded = Rect {
            x: rect.x.saturating_sub(pad),
            y: rect.y.saturating_sub(pad),
            width: rect.width.saturating_add(pad.saturating_mul(2)),
            height: rect.height.saturating_add(pad.saturating_mul(2)),
        };
        Self::clipped_to_bounds(padded, bounds)
    }

    #[must_use]
    pub fn clipped_to_screen(rect: Rect, screen_width: i32, screen_height: i32) -> Option<Self> {
        Self::clipped_to_bounds(
            rect,
            Rect {
                x: 0,
                y: 0,
                width: screen_width,
                height: screen_height,
            },
        )
    }

    #[must_use]
    pub fn padded_to_screen(
        rect: Rect,
        padding: i32,
        screen_width: i32,
        screen_height: i32,
    ) -> Option<Self> {
        Self::padded_to_bounds(
            rect,
            padding,
            Rect {
                x: 0,
                y: 0,
                width: screen_width,
                height: screen_height,
            },
        )
    }

    #[must_use]
    pub const fn origin(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    #[must_use]
    pub const fn local_origin(&self, window_origin: (i32, i32)) -> (i32, i32) {
        (self.x - window_origin.0, self.y - window_origin.1)
    }
}

#[must_use]
pub const fn capture_interval_ms(on_battery: bool) -> u32 {
    capture_interval_for(on_battery, false)
}

#[must_use]
pub const fn capture_interval_for(on_battery: bool, active: bool) -> u32 {
    match (on_battery, active) {
        (true, true) => ACTIVE_ON_BATTERY_INTERVAL_MS,
        (true, false) => CALM_ON_BATTERY_INTERVAL_MS,
        (false, true) => ACTIVE_INTERVAL_MS,
        (false, false) => CALM_INTERVAL_MS,
    }
}

#[must_use]
pub fn capture_wanted(appearance: &Appearance, glass_enabled: bool) -> bool {
    glass_enabled && appearance.uses_backdrop()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CaptureGate {
    region: Option<CaptureRegion>,
    last_request_ms: Option<u64>,
    accepted_region: Option<CaptureRegion>,
    accepted_signature: Option<u64>,
}

impl CaptureGate {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            region: None,
            last_request_ms: None,
            accepted_region: None,
            accepted_signature: None,
        }
    }

    #[must_use]
    pub const fn region(&self) -> Option<CaptureRegion> {
        self.region
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.region.is_some()
    }

    pub fn should_request(
        &mut self,
        region: CaptureRegion,
        now_ms: u64,
        interval_ms: u32,
        force: bool,
    ) -> bool {
        let moved = self.region != Some(region);
        let due = self
            .last_request_ms
            .is_none_or(|last| now_ms.saturating_sub(last) >= u64::from(interval_ms));
        if !(force || moved || due) {
            return false;
        }
        self.region = Some(region);
        self.last_request_ms = Some(now_ms);
        true
    }

    pub fn accepts(&mut self, origin: (i32, i32), width: u32, height: u32, signature: u64) -> bool {
        if self.region.is_none() || width == 0 || height == 0 {
            return false;
        }
        let incoming = CaptureRegion {
            x: origin.0,
            y: origin.1,
            width,
            height,
        };
        if self.accepted_region == Some(incoming) && self.accepted_signature == Some(signature) {
            return false;
        }
        self.accepted_region = Some(incoming);
        self.accepted_signature = Some(signature);
        true
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureGate, CaptureRegion, capture_interval_for, capture_interval_ms, capture_wanted,
    };
    use crate::layout::Rect;
    use crate::material::{Appearance, ContrastColors};
    use crate::theme::ResolvedTheme;

    fn region(x: i32) -> CaptureRegion {
        CaptureRegion {
            x,
            y: 700,
            width: 400,
            height: 80,
        }
    }

    #[test]
    fn regions_are_clipped_to_the_screen_and_empty_ones_vanish() {
        let inside = Rect {
            x: 100,
            y: 100,
            width: 300,
            height: 50,
        };
        assert_eq!(
            CaptureRegion::clipped_to_screen(inside, 1920, 1080),
            Some(CaptureRegion {
                x: 100,
                y: 100,
                width: 300,
                height: 50
            })
        );
        let hanging = Rect {
            x: -20,
            y: 1060,
            width: 2000,
            height: 100,
        };
        assert_eq!(
            CaptureRegion::clipped_to_screen(hanging, 1920, 1080),
            Some(CaptureRegion {
                x: 0,
                y: 1060,
                width: 1920,
                height: 20
            })
        );
        let below = Rect {
            x: 0,
            y: 1080,
            width: 100,
            height: 100,
        };
        assert_eq!(CaptureRegion::clipped_to_screen(below, 1920, 1080), None);
        assert_eq!(
            CaptureRegion::padded_to_screen(inside, 20, 1920, 1080),
            Some(CaptureRegion {
                x: 80,
                y: 80,
                width: 340,
                height: 90,
            })
        );
        let top_bar = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 32,
        };
        assert_eq!(
            CaptureRegion::padded_to_screen(top_bar, 24, 1920, 1080),
            Some(CaptureRegion {
                x: 0,
                y: 0,
                width: 1920,
                height: 56,
            })
        );
        let left_monitor = Rect {
            x: -1920,
            y: -120,
            width: 1920,
            height: 1080,
        };
        let secondary_bar = Rect {
            x: -1920,
            y: -120,
            width: 1920,
            height: 32,
        };
        assert_eq!(
            CaptureRegion::padded_to_bounds(secondary_bar, 24, left_monitor),
            Some(CaptureRegion {
                x: -1920,
                y: -120,
                width: 1920,
                height: 56,
            })
        );
    }

    #[test]
    fn local_origin_is_relative_to_the_window() {
        let clipped = CaptureRegion {
            x: 0,
            y: 1060,
            width: 1920,
            height: 20,
        };
        assert_eq!(clipped.local_origin((0, 1000)), (0, 60));
    }

    #[test]
    fn capture_slows_down_on_battery() {
        assert_eq!(capture_interval_ms(false), 400);
        assert!(capture_interval_ms(true) > capture_interval_ms(false));
    }

    #[test]
    fn active_capture_runs_faster_than_calm_while_staying_slower_on_battery() {
        assert_eq!(capture_interval_for(false, true), 33);
        assert_eq!(capture_interval_for(false, false), 400);
        assert_eq!(capture_interval_for(true, true), 120);
        assert_eq!(capture_interval_for(true, false), 1_000);
        assert!(capture_interval_for(false, true) < capture_interval_for(false, false));
        assert!(capture_interval_for(true, true) < capture_interval_for(true, false));
        assert!(capture_interval_for(true, true) > capture_interval_for(false, true));
    }

    #[test]
    fn capture_needs_the_glass_material_and_the_user_option() {
        let glass = Appearance::glass(ResolvedTheme::Dark);
        assert!(capture_wanted(&glass, true));
        assert!(!capture_wanted(&glass, false));
        let saver = Appearance {
            transparency: false,
            ..glass
        };
        assert!(!capture_wanted(&saver, true));
        let contrast = Appearance {
            contrast: Some(ContrastColors::BLACK_THEME),
            ..glass
        };
        assert!(!capture_wanted(&contrast, true));
    }

    #[test]
    fn requests_follow_the_interval_unless_forced_or_the_region_moves() {
        let mut gate = CaptureGate::new();
        assert!(!gate.is_active());
        assert!(gate.should_request(region(0), 0, 400, false));
        assert!(gate.is_active());
        assert!(!gate.should_request(region(0), 399, 400, false));
        assert!(gate.should_request(region(0), 400, 400, false));
        assert!(!gate.should_request(region(0), 500, 400, false));
        assert!(gate.should_request(region(0), 500, 400, true));
        assert!(gate.should_request(region(10), 501, 400, false));
        assert_eq!(gate.region(), Some(region(10)));
    }

    #[test]
    fn frames_are_accepted_once_per_region_and_signature_even_when_in_flight() {
        let mut gate = CaptureGate::new();
        assert!(!gate.accepts((0, 700), 400, 80, 1));
        gate.should_request(region(0), 0, 400, false);
        assert!(!gate.accepts((0, 700), 0, 80, 1));
        assert!(!gate.accepts((0, 700), 400, 0, 1));
        assert!(gate.accepts((0, 700), 400, 80, 1));
        assert!(!gate.accepts((0, 700), 400, 80, 1));
        assert!(gate.accepts((0, 700), 400, 80, 2));
        assert!(!gate.accepts((0, 700), 400, 80, 2));
        assert!(gate.accepts((5, 700), 400, 80, 2));
        assert!(!gate.accepts((5, 700), 400, 80, 2));
        assert!(gate.accepts((0, 700), 399, 80, 2));
        assert!(gate.accepts((0, 700), 400, 81, 2));
    }

    #[test]
    fn reset_forgets_the_region_and_the_accepted_frame() {
        let mut gate = CaptureGate::new();
        gate.should_request(region(0), 0, 400, false);
        assert!(gate.accepts((0, 700), 400, 80, 9));
        assert!(!gate.accepts((0, 700), 400, 80, 9));
        gate.reset();
        assert!(!gate.is_active());
        assert!(!gate.accepts((0, 700), 400, 80, 9));
        assert!(gate.should_request(region(0), 1, 400, false));
        assert!(gate.accepts((0, 700), 400, 80, 9));
    }
}
