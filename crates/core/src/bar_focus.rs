use crate::layout::{HitTarget, Layout};
use crate::media::NotchExpandedTab;

const KEYBOARD_CANDIDATES: [HitTarget; 13] = [
    HitTarget::Start,
    HitTarget::AppMenu,
    HitTarget::FileMenu,
    HitTarget::EditMenu,
    HitTarget::ViewMenu,
    HitTarget::WindowMenu,
    HitTarget::HelpMenu,
    HitTarget::MenuOverflow,
    HitTarget::Tray,
    HitTarget::QuickSettings,
    HitTarget::Search,
    HitTarget::ControlCenter,
    HitTarget::Clock,
];

impl Layout {
    #[must_use]
    pub fn keyboard_targets(&self) -> Vec<HitTarget> {
        let mut targets: Vec<(i32, HitTarget)> = KEYBOARD_CANDIDATES
            .into_iter()
            .filter(|target| !self.is_hidden(*target))
            .filter_map(|target| {
                self.slot_rect(target)
                    .filter(|slot| !slot.is_empty())
                    .map(|slot| (slot.x, target))
            })
            .collect();
        targets.sort_by_key(|(x, _)| *x);
        targets.into_iter().map(|(_, target)| target).collect()
    }

    #[must_use]
    pub fn keyboard_targets_with_media(&self, media_present: bool) -> Vec<HitTarget> {
        let mut targets = self.keyboard_targets();
        let showing_media = media_present && self.active_tab == NotchExpandedTab::Media;
        targets.extend(
            NOTCH_KEYBOARD_CANDIDATES
                .into_iter()
                .filter(|target| self.notch_control_rect(*target).is_some())
                .filter(|target| match target {
                    HitTarget::NotchTabPomodoro => true,
                    HitTarget::PomodoroPlayPause
                    | HitTarget::PomodoroReset
                    | HitTarget::PomodoroSkip => !showing_media,
                    _ => media_present,
                }),
        );
        targets
    }
}

const NOTCH_KEYBOARD_CANDIDATES: [HitTarget; 10] = [
    HitTarget::NotchTabPomodoro,
    HitTarget::PomodoroPlayPause,
    HitTarget::PomodoroReset,
    HitTarget::PomodoroSkip,
    HitTarget::NotchTabMedia,
    HitTarget::MediaLyricsToggle,
    HitTarget::MediaPrevious,
    HitTarget::MediaPlayPause,
    HitTarget::MediaNext,
    HitTarget::MediaSeekbar,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarKey {
    Previous,
    Next,
    First,
    Last,
    Activate,
    Exit,
    PreviousTab,
    NextTab,
}

impl BarKey {
    const ALL: [Self; 8] = [
        Self::Previous,
        Self::Next,
        Self::First,
        Self::Last,
        Self::Activate,
        Self::Exit,
        Self::PreviousTab,
        Self::NextTab,
    ];

    #[must_use]
    pub fn code(self) -> usize {
        Self::ALL
            .iter()
            .position(|key| *key == self)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn from_code(code: usize) -> Option<Self> {
        Self::ALL.get(code).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarFocusOutcome {
    Moved(HitTarget),
    Activate(HitTarget),
    Exited,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarFocus {
    current: Option<HitTarget>,
}

impl BarFocus {
    #[must_use]
    pub const fn current(self) -> Option<HitTarget> {
        self.current
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        self.current.is_some()
    }

    pub fn enter(&mut self, targets: &[HitTarget]) -> Option<HitTarget> {
        self.current = targets.first().copied();
        self.current
    }

    pub fn enter_last(&mut self, targets: &[HitTarget]) -> Option<HitTarget> {
        self.current = targets.last().copied();
        self.current
    }

    #[must_use]
    pub fn dock_handoff(&self, key: BarKey, targets: &[HitTarget]) -> Option<bool> {
        let current = self.current?;
        match key {
            BarKey::NextTab if targets.last() == Some(&current) => Some(false),
            BarKey::PreviousTab if targets.first() == Some(&current) => Some(true),
            _ => None,
        }
    }

    pub fn exit(&mut self) -> bool {
        self.current.take().is_some()
    }

    pub fn revalidate(&mut self, targets: &[HitTarget]) -> bool {
        match self.current {
            Some(current) if !targets.contains(&current) => {
                self.current = targets.first().copied();
                true
            }
            _ => false,
        }
    }

    pub fn handle(&mut self, key: BarKey, targets: &[HitTarget]) -> BarFocusOutcome {
        let Some(current) = self.current else {
            return BarFocusOutcome::Unchanged;
        };
        if key == BarKey::Exit || targets.is_empty() {
            self.current = None;
            return BarFocusOutcome::Exited;
        }
        let Some(index) = targets.iter().position(|target| *target == current) else {
            let first = targets[0];
            self.current = Some(first);
            return BarFocusOutcome::Moved(first);
        };
        if key == BarKey::Activate {
            self.current = None;
            return BarFocusOutcome::Activate(current);
        }
        let last = targets.len() - 1;
        let next = match key {
            BarKey::First => 0,
            BarKey::Last => last,
            BarKey::Next | BarKey::NextTab => (index + 1) % targets.len(),
            BarKey::Previous | BarKey::PreviousTab => index.checked_sub(1).unwrap_or(last),
            BarKey::Activate | BarKey::Exit => return BarFocusOutcome::Unchanged,
        };
        let target = targets[next];
        self.current = Some(target);
        if target == current {
            BarFocusOutcome::Unchanged
        } else {
            BarFocusOutcome::Moved(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BarFocus, BarFocusOutcome, BarKey};
    use crate::layout::{
        HitTarget, Metrics, NotchLayoutInput, compute_layout_for_app, compute_layout_with_notch,
    };
    use crate::media::NotchExpandedTab;

    fn open_media_layout(media_tab: bool) -> crate::layout::Layout {
        compute_layout_with_notch(
            &Metrics::MACBOOK_PRO_14,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                alert_mode: false,
                active_tab: if media_tab {
                    NotchExpandedTab::Media
                } else {
                    NotchExpandedTab::Pomodoro
                },
                control_center_enabled: true,
                lyrics_expanded: false,
            },
        )
    }

    #[test]
    fn notch_media_controls_join_keyboard_order_only_on_the_open_media_tab() {
        let media = open_media_layout(true);
        let targets = media.keyboard_targets_with_media(true);
        assert_eq!(
            &targets[targets.len() - 7..],
            &[
                HitTarget::NotchTabPomodoro,
                HitTarget::NotchTabMedia,
                HitTarget::MediaLyricsToggle,
                HitTarget::MediaPrevious,
                HitTarget::MediaPlayPause,
                HitTarget::MediaNext,
                HitTarget::MediaSeekbar,
            ]
        );
        let without_media = media.keyboard_targets_with_media(false);
        assert!(!without_media.contains(&HitTarget::MediaPlayPause));
        assert!(!without_media.contains(&HitTarget::NotchTabMedia));
        assert!(without_media.contains(&HitTarget::NotchTabPomodoro));

        let pomodoro_targets = open_media_layout(false).keyboard_targets_with_media(true);
        assert!(pomodoro_targets.contains(&HitTarget::NotchTabMedia));
        assert!(pomodoro_targets.contains(&HitTarget::PomodoroPlayPause));
        assert!(pomodoro_targets.contains(&HitTarget::PomodoroReset));
        assert!(pomodoro_targets.contains(&HitTarget::PomodoroSkip));
        assert!(!pomodoro_targets.contains(&HitTarget::MediaPlayPause));
        assert!(
            !media
                .keyboard_targets_with_media(true)
                .contains(&HitTarget::PomodoroPlayPause)
        );

        let closed = compute_layout_for_app(&Metrics::MACBOOK_PRO_14, 1920, 96, "Finder");
        assert!(
            !closed
                .keyboard_targets_with_media(true)
                .contains(&HitTarget::MediaPlayPause)
        );
    }

    #[test]
    fn keyboard_order_follows_the_visible_slots_left_to_right() {
        let layout = compute_layout_for_app(&Metrics::MACBOOK_PRO_14, 1920, 96, "Finder");
        let targets = layout.keyboard_targets();
        assert_eq!(targets.first(), Some(&HitTarget::Start));
        assert!(targets.contains(&HitTarget::Clock));
        assert!(!targets.contains(&HitTarget::Notch));
        let xs: Vec<i32> = targets
            .iter()
            .map(|target| layout.slot_rect(*target).unwrap().x)
            .collect();
        assert!(xs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn overflowed_slots_are_reached_through_the_overflow_target() {
        let layout = compute_layout_for_app(&Metrics::MACBOOK_PRO_14, 800, 96, "Finder");
        let targets = layout.keyboard_targets();
        assert!(!layout.hidden_targets.is_empty());
        for hidden in &layout.hidden_targets {
            assert!(!targets.contains(hidden));
        }
        assert!(targets.contains(&HitTarget::MenuOverflow));
    }

    const TARGETS: [HitTarget; 4] = [
        HitTarget::Start,
        HitTarget::AppMenu,
        HitTarget::ControlCenter,
        HitTarget::Clock,
    ];

    fn focused() -> BarFocus {
        let mut focus = BarFocus::default();
        focus.enter(&TARGETS);
        focus
    }

    #[test]
    fn entering_focuses_the_first_target_and_nothing_when_empty() {
        let mut focus = BarFocus::default();
        assert_eq!(focus.enter(&TARGETS), Some(HitTarget::Start));
        assert!(focus.is_active());
        let mut empty = BarFocus::default();
        assert_eq!(empty.enter(&[]), None);
        assert!(!empty.is_active());
    }

    #[test]
    fn arrows_move_and_wrap_around_both_ends() {
        let mut focus = focused();
        assert_eq!(
            focus.handle(BarKey::Previous, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::Clock)
        );
        assert_eq!(
            focus.handle(BarKey::Next, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::Start)
        );
        assert_eq!(
            focus.handle(BarKey::Next, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::AppMenu)
        );
    }

    #[test]
    fn home_and_end_jump_to_the_edges() {
        let mut focus = focused();
        assert_eq!(
            focus.handle(BarKey::Last, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::Clock)
        );
        assert_eq!(
            focus.handle(BarKey::First, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::Start)
        );
        assert_eq!(
            focus.handle(BarKey::First, &TARGETS),
            BarFocusOutcome::Unchanged
        );
    }

    #[test]
    fn activation_and_escape_leave_focus_mode() {
        let mut focus = focused();
        focus.handle(BarKey::Next, &TARGETS);
        assert_eq!(
            focus.handle(BarKey::Activate, &TARGETS),
            BarFocusOutcome::Activate(HitTarget::AppMenu)
        );
        assert!(!focus.is_active());
        let mut focus = focused();
        assert_eq!(
            focus.handle(BarKey::Exit, &TARGETS),
            BarFocusOutcome::Exited
        );
        assert_eq!(
            focus.handle(BarKey::Next, &TARGETS),
            BarFocusOutcome::Unchanged
        );
    }

    #[test]
    fn a_target_that_disappears_moves_focus_to_the_first_remaining_one() {
        let mut focus = focused();
        focus.handle(BarKey::Last, &TARGETS);
        let shrunk = [HitTarget::Start, HitTarget::AppMenu];
        assert!(focus.revalidate(&shrunk));
        assert_eq!(focus.current(), Some(HitTarget::Start));
        assert!(!focus.revalidate(&shrunk));
        assert_eq!(focus.handle(BarKey::Next, &[]), BarFocusOutcome::Exited);
    }

    #[test]
    fn tab_keys_step_like_arrows() {
        let mut focus = focused();
        assert_eq!(
            focus.handle(BarKey::NextTab, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::AppMenu)
        );
        assert_eq!(
            focus.handle(BarKey::PreviousTab, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::Start)
        );
        assert_eq!(
            focus.handle(BarKey::PreviousTab, &TARGETS),
            BarFocusOutcome::Moved(HitTarget::Clock)
        );
    }

    #[test]
    fn tab_past_either_end_hands_over_to_the_dock() {
        let mut focus = focused();
        assert_eq!(focus.dock_handoff(BarKey::NextTab, &TARGETS), None);
        focus.handle(BarKey::Last, &TARGETS);
        assert_eq!(focus.dock_handoff(BarKey::NextTab, &TARGETS), Some(false));
        assert_eq!(focus.dock_handoff(BarKey::PreviousTab, &TARGETS), None);
        focus.handle(BarKey::First, &TARGETS);
        assert_eq!(
            focus.dock_handoff(BarKey::PreviousTab, &TARGETS),
            Some(true)
        );
        assert_eq!(focus.dock_handoff(BarKey::Next, &TARGETS), None);
    }

    #[test]
    fn a_vanished_focus_recovers_to_the_first_target_instead_of_activating_it() {
        let mut focus = focused();
        let remaining = [HitTarget::AppMenu, HitTarget::ControlCenter];
        assert_eq!(
            focus.handle(BarKey::Activate, &remaining),
            BarFocusOutcome::Moved(HitTarget::AppMenu)
        );
        assert_eq!(focus.current(), Some(HitTarget::AppMenu));
        assert_eq!(
            focus.handle(BarKey::Activate, &remaining),
            BarFocusOutcome::Activate(HitTarget::AppMenu)
        );
    }

    #[test]
    fn a_single_target_bar_hands_off_on_both_tabs_and_keeps_focus_otherwise() {
        let only = [HitTarget::Clock];
        let mut focus = BarFocus::default();
        focus.enter(&only);
        assert_eq!(focus.dock_handoff(BarKey::NextTab, &only), Some(false));
        assert_eq!(focus.dock_handoff(BarKey::PreviousTab, &only), Some(true));
        assert_eq!(focus.dock_handoff(BarKey::Next, &only), None);
        assert_eq!(
            focus.handle(BarKey::NextTab, &only),
            BarFocusOutcome::Unchanged
        );
        assert!(focus.is_active());
    }

    #[test]
    fn enter_last_focuses_the_final_target() {
        let mut focus = BarFocus::default();
        assert_eq!(focus.enter_last(&TARGETS), Some(HitTarget::Clock));
        assert_eq!(focus.enter_last(&[]), None);
    }

    #[test]
    fn key_codes_round_trip() {
        for key in BarKey::ALL {
            assert_eq!(BarKey::from_code(key.code()), Some(key));
        }
        assert_eq!(BarKey::from_code(BarKey::ALL.len()), None);
    }
}
