use crate::menu_bar::BarHideMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarPointer {
    pub on_bar_monitor: bool,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarVisibility {
    mode: BarHideMode,
    full_screen_app: bool,
    revealed: bool,
}

impl BarVisibility {
    pub fn set_mode(&mut self, mode: BarHideMode) -> bool {
        let before = self.is_visible();
        self.mode = mode;
        self.settle_reveal();
        before != self.is_visible()
    }

    pub fn set_full_screen_app(&mut self, active: bool) -> bool {
        let before = self.is_visible();
        self.full_screen_app = active;
        self.settle_reveal();
        before != self.is_visible()
    }

    pub fn track_pointer(
        &mut self,
        pointer: BarPointer,
        bar_height: i32,
        popup_open: bool,
    ) -> bool {
        if !self.hides() {
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
        !self.hides() || self.revealed
    }

    #[must_use]
    pub const fn tracks_pointer(&self) -> bool {
        self.hides()
    }

    const fn hides(self) -> bool {
        self.mode.hides(self.full_screen_app)
    }

    fn settle_reveal(&mut self) {
        if !self.hides() {
            self.revealed = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BarPointer, BarVisibility};
    use crate::menu_bar::BarHideMode;

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

    fn bar_in(mode: BarHideMode) -> BarVisibility {
        let mut bar = BarVisibility::default();
        bar.set_mode(mode);
        bar
    }

    #[test]
    fn each_mode_is_visible_or_hidden_in_desktop_and_full_screen() {
        let cases = [
            (BarHideMode::Always, false, true),
            (BarHideMode::Always, true, true),
            (BarHideMode::DesktopOnly, false, true),
            (BarHideMode::DesktopOnly, true, false),
            (BarHideMode::FullScreenOnly, false, false),
            (BarHideMode::FullScreenOnly, true, true),
            (BarHideMode::Never, false, false),
            (BarHideMode::Never, true, false),
        ];
        for (mode, full_screen, hidden) in cases {
            let mut bar = bar_in(mode);
            bar.set_full_screen_app(full_screen);
            assert_eq!(
                bar.is_visible(),
                !hidden,
                "{mode:?} full_screen={full_screen}"
            );
            assert_eq!(
                bar.tracks_pointer(),
                hidden,
                "{mode:?} full_screen={full_screen}"
            );
        }
    }

    #[test]
    fn always_mode_hides_on_the_desktop_and_reveals_at_the_top_edge() {
        let mut bar = bar_in(BarHideMode::Always);
        assert!(!bar.is_visible());
        assert!(bar.track_pointer(at(0), BAR, false));
        assert!(bar.is_visible());
        assert!(bar.track_pointer(at(BAR + 40), BAR, false));
        assert!(!bar.is_visible());
    }

    #[test]
    fn never_mode_ignores_the_pointer() {
        let mut bar = bar_in(BarHideMode::Never);
        bar.set_full_screen_app(true);
        assert!(!bar.track_pointer(at(0), BAR, false));
        assert!(bar.is_visible());
    }

    #[test]
    fn switching_to_a_visible_mode_clears_a_reveal_and_reports_the_change() {
        let mut bar = bar_in(BarHideMode::Always);
        bar.track_pointer(at(0), BAR, false);
        assert!(!bar.set_mode(BarHideMode::Never));
        assert!(bar.set_mode(BarHideMode::Always));
        assert!(!bar.is_visible());
    }

    #[test]
    fn switching_into_a_hiding_mode_hides_a_visible_bar() {
        let mut bar = BarVisibility::default();
        assert!(bar.is_visible());
        assert!(bar.set_mode(BarHideMode::Always));
        assert!(!bar.is_visible());
    }

    #[test]
    fn desktop_only_hides_after_leaving_full_screen() {
        let mut bar = bar_in(BarHideMode::DesktopOnly);
        bar.set_full_screen_app(true);
        assert!(bar.is_visible());
        assert!(bar.set_full_screen_app(false));
        assert!(!bar.is_visible());
    }
}
