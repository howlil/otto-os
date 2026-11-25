use crate::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MonitorEdgeWatch {
    pub bounds: Rect,
    pub top: bool,
    pub bottom_band: Option<i32>,
}

impl MonitorEdgeWatch {
    #[must_use]
    pub const fn is_watching(&self) -> bool {
        (self.top || self.bottom_band.is_some()) && self.bounds.width > 0 && self.bounds.height > 0
    }

    #[must_use]
    pub const fn touches(&self, x: i32, y: i32) -> bool {
        if !self.is_watching() || !self.bounds.contains(x, y) {
            return false;
        }
        let at_top = self.top && y <= self.bounds.y;
        let at_bottom = match self.bottom_band {
            Some(band) => y >= self.bounds.bottom() - band,
            None => false,
        };
        at_top || at_bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PointerEdgeWatch {
    pub screen_width: i32,
    pub screen_height: i32,
    pub top: bool,
    pub bottom_band: Option<i32>,
}

impl PointerEdgeWatch {
    #[must_use]
    pub const fn is_watching(&self) -> bool {
        self.top || self.bottom_band.is_some()
    }

    #[must_use]
    pub const fn touches(&self, x: i32, y: i32) -> bool {
        MonitorEdgeWatch {
            bounds: Rect {
                x: 0,
                y: 0,
                width: self.screen_width,
                height: self.screen_height,
            },
            top: self.top,
            bottom_band: self.bottom_band,
        }
        .touches(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::{MonitorEdgeWatch, PointerEdgeWatch};
    use crate::layout::Rect;

    const fn watch(top: bool, bottom_band: Option<i32>) -> PointerEdgeWatch {
        PointerEdgeWatch {
            screen_width: 1920,
            screen_height: 1080,
            top,
            bottom_band,
        }
    }

    #[test]
    fn nothing_is_watched_by_default() {
        let edges = PointerEdgeWatch::default();
        assert!(!edges.is_watching());
        assert!(!edges.touches(0, 0));
    }

    #[test]
    fn top_edge_is_the_first_row_only() {
        let edges = watch(true, None);
        assert!(edges.touches(500, 0));
        assert!(!edges.touches(500, 1));
        assert!(!edges.touches(500, 1079));
    }

    #[test]
    fn bottom_band_covers_the_last_rows() {
        let edges = watch(false, Some(6));
        assert!(edges.touches(500, 1079));
        assert!(edges.touches(500, 1074));
        assert!(!edges.touches(500, 1073));
        assert!(!edges.touches(500, 0));
    }

    #[test]
    fn pointer_on_another_monitor_never_touches() {
        let edges = watch(true, Some(6));
        assert!(!edges.touches(-10, 0));
        assert!(!edges.touches(1920, 1079));
        assert!(!edges.touches(500, -1));
        assert!(!edges.touches(500, 1080));
    }

    #[test]
    fn monitor_edge_watch_handles_negative_and_offset_monitors() {
        let left = MonitorEdgeWatch {
            bounds: Rect {
                x: -1920,
                y: -200,
                width: 1920,
                height: 1080,
            },
            top: true,
            bottom_band: Some(6),
        };
        assert!(left.touches(-1000, -200));
        assert!(!left.touches(-1000, -199));
        assert!(left.touches(-1000, 879));
        assert!(left.touches(-1000, 874));
        assert!(!left.touches(-1000, 873));
        assert!(!left.touches(0, -200));
        assert!(!left.touches(-1000, -201));
    }
}
