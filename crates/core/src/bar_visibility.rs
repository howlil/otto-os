#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarPointer {
    pub on_bar_monitor: bool,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarVisibility {
    full_screen_app: bool,
    revealed: bool,
}

impl BarVisibility {
    pub fn set_full_screen_app(&mut self, active: bool) -> bool {
        let before = self.is_visible();
        self.full_screen_app = active;
        if !active {
            self.revealed = false;
        }
        before != self.is_visible()
    }

    pub fn track_pointer(
        &mut self,
        pointer: BarPointer,
        bar_height: i32,
        popup_open: bool,
    ) -> bool {
        if !self.full_screen_app {
            return false;
        }
        let before = self.is_visible();
        let at_edge = pointer.on_bar_monitor && pointer.y <= 0;
        let over_bar = pointer.on_bar_monitor && pointer.y < bar_height;
        self.revealed = if self.revealed {
            over_bar || popup_open
        } else {
            at_edge
        };
        before != self.is_visible()
    }

    #[must_use]
    pub const fn is_visible(&self) -> bool {
        !self.full_screen_app || self.revealed
    }

    #[must_use]
    pub const fn tracks_pointer(&self) -> bool {
        self.full_screen_app
    }
}

#[cfg(test)]
mod tests {
    use super::{BarPointer, BarVisibility};

    const BAR: i32 = 32;

    fn at(y: i32) -> BarPointer {
        BarPointer {
            on_bar_monitor: true,
            y,
        }
    }

    #[test]
    fn bar_is_visible_without_a_full_screen_app() {
        let mut bar = BarVisibility::default();
        assert!(bar.is_visible());
        assert!(!bar.track_pointer(at(0), BAR, false));
        assert!(bar.is_visible());
    }

    #[test]
    fn full_screen_app_hides_the_bar() {
        let mut bar = BarVisibility::default();
        assert!(bar.set_full_screen_app(true));
        assert!(!bar.is_visible());
        assert!(bar.set_full_screen_app(false));
        assert!(bar.is_visible());
    }

    #[test]
    fn pointer_at_the_top_edge_reveals_until_it_leaves_the_bar() {
        let mut bar = BarVisibility::default();
        bar.set_full_screen_app(true);
        assert!(!bar.track_pointer(at(10), BAR, false));
        assert!(bar.track_pointer(at(0), BAR, false));
        assert!(bar.is_visible());
        assert!(!bar.track_pointer(at(20), BAR, false));
        assert!(bar.is_visible());
        assert!(bar.track_pointer(at(BAR + 40), BAR, false));
        assert!(!bar.is_visible());
    }

    #[test]
    fn open_popup_keeps_a_revealed_bar_visible() {
        let mut bar = BarVisibility::default();
        bar.set_full_screen_app(true);
        bar.track_pointer(at(0), BAR, false);
        assert!(!bar.track_pointer(at(400), BAR, true));
        assert!(bar.is_visible());
        assert!(bar.track_pointer(at(400), BAR, false));
    }

    #[test]
    fn pointer_on_another_monitor_never_reveals() {
        let mut bar = BarVisibility::default();
        bar.set_full_screen_app(true);
        let elsewhere = BarPointer {
            on_bar_monitor: false,
            y: 0,
        };
        assert!(!bar.track_pointer(elsewhere, BAR, false));
        assert!(!bar.is_visible());
    }

    #[test]
    fn pointer_tracking_is_needed_only_while_a_full_screen_app_is_active() {
        let mut bar = BarVisibility::default();
        assert!(!bar.tracks_pointer());
        bar.set_full_screen_app(true);
        assert!(bar.tracks_pointer());
        bar.track_pointer(at(0), BAR, false);
        assert!(bar.tracks_pointer());
        bar.set_full_screen_app(false);
        assert!(!bar.tracks_pointer());
    }

    #[test]
    fn leaving_full_screen_clears_a_previous_reveal() {
        let mut bar = BarVisibility::default();
        bar.set_full_screen_app(true);
        bar.track_pointer(at(0), BAR, false);
        bar.set_full_screen_app(false);
        bar.set_full_screen_app(true);
        assert!(!bar.is_visible());
    }
}
