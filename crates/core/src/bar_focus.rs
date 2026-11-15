use crate::layout::{HitTarget, Layout};
use crate::media::NotchExpandedTab;
use crate::pomodoro::FocusSubMode;

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
        let mut bar_items: Vec<(i32, HitTarget)> = KEYBOARD_CANDIDATES
            .into_iter()
            .filter(|target| !self.is_hidden(*target))
            .filter_map(|target| {
                self.slot_rect(target)
                    .filter(|slot| !slot.is_empty())
                    .map(|slot| (slot.x, target))
            })
            .collect();
        if !self.privacy_dots.is_empty() {
            bar_items.push((self.privacy_dots.x, HitTarget::PrivacyIndicator));
        }
        if self.panel_expansion <= 0.55 && !self.notch.is_empty() {
            bar_items.push((self.notch.x, HitTarget::Notch));
        }
        if self.secondary_expansion > 0.25 && !self.notch_secondary.is_empty() {
            bar_items.push((self.notch_secondary.x, HitTarget::NotchSecondary));
        }
        bar_items.sort_by_key(|(x, _)| *x);
        let mut targets: Vec<HitTarget> = bar_items.into_iter().map(|(_, target)| target).collect();
        let showing_media = media_present && self.active_tab == NotchExpandedTab::Media;
        targets.extend(
            NOTCH_KEYBOARD_CANDIDATES
                .into_iter()
                .filter(|target| self.notch_control_rect(*target).is_some())
                .filter(|target| match target {
                    HitTarget::NotchTabPomodoro
                    | HitTarget::NotchTabShelf
                    | HitTarget::NotchTabClipboard
                    | HitTarget::PomodoroExtendFive
                    | HitTarget::PomodoroStartNext
                    | HitTarget::PrivacyMicMute => true,
                    HitTarget::NotchTabMedia => media_present,
                    HitTarget::FocusMode(_)
                    | HitTarget::PomodoroPreset(_)
                    | HitTarget::PomodoroReset
                    | HitTarget::PomodoroPlayPause
                    | HitTarget::PomodoroSkip => self.active_tab == NotchExpandedTab::Pomodoro,
                    HitTarget::ShelfPager
                    | HitTarget::ShelfAdd
                    | HitTarget::ShelfClear
                    | HitTarget::ShelfItem(_)
                    | HitTarget::ShelfCopyItem(_)
                    | HitTarget::ShelfRemoveItem(_) => self.active_tab == NotchExpandedTab::Shelf,
                    HitTarget::ClipboardPager
                    | HitTarget::ClipboardPauseToggle
                    | HitTarget::ClipboardClear
                    | HitTarget::ClipboardItem(_)
                    | HitTarget::ClipboardRemoveItem(_) => {
                        self.active_tab == NotchExpandedTab::Clipboard
                    }
                    _ => showing_media,
                }),
        );
        targets
    }
}

const NOTCH_KEYBOARD_CANDIDATES: [HitTarget; 54] = [
    HitTarget::NotchTabPomodoro,
    HitTarget::NotchTabShelf,
    HitTarget::NotchTabClipboard,
    HitTarget::NotchTabMedia,
    HitTarget::FocusMode(FocusSubMode::Pomodoro),
    HitTarget::FocusMode(FocusSubMode::Timer),
    HitTarget::FocusMode(FocusSubMode::Stopwatch),
    HitTarget::PomodoroPreset(0),
    HitTarget::PomodoroPreset(1),
    HitTarget::PomodoroPreset(2),
    HitTarget::PomodoroPreset(3),
    HitTarget::PomodoroReset,
    HitTarget::PomodoroPlayPause,
    HitTarget::PomodoroSkip,
    HitTarget::PomodoroExtendFive,
    HitTarget::PomodoroStartNext,
    HitTarget::PrivacyMicMute,
    HitTarget::ShelfPager,
    HitTarget::ShelfAdd,
    HitTarget::ShelfClear,
    HitTarget::ShelfItem(0),
    HitTarget::ShelfCopyItem(0),
    HitTarget::ShelfRemoveItem(0),
    HitTarget::ShelfItem(1),
    HitTarget::ShelfCopyItem(1),
    HitTarget::ShelfRemoveItem(1),
    HitTarget::ShelfItem(2),
    HitTarget::ShelfCopyItem(2),
    HitTarget::ShelfRemoveItem(2),
    HitTarget::ShelfItem(3),
    HitTarget::ShelfCopyItem(3),
    HitTarget::ShelfRemoveItem(3),
    HitTarget::ShelfItem(4),
    HitTarget::ShelfCopyItem(4),
    HitTarget::ShelfRemoveItem(4),
    HitTarget::ShelfItem(5),
    HitTarget::ShelfCopyItem(5),
    HitTarget::ShelfRemoveItem(5),
    HitTarget::ClipboardPager,
    HitTarget::ClipboardPauseToggle,
    HitTarget::ClipboardClear,
    HitTarget::ClipboardItem(0),
    HitTarget::ClipboardRemoveItem(0),
    HitTarget::ClipboardItem(1),
    HitTarget::ClipboardRemoveItem(1),
    HitTarget::ClipboardItem(2),
    HitTarget::ClipboardRemoveItem(2),
    HitTarget::ClipboardItem(3),
    HitTarget::ClipboardRemoveItem(3),
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
    Up,
    Down,
    First,
    Last,
    Activate,
    Exit,
    PreviousTab,
    NextTab,
}

impl BarKey {
    const ALL: [Self; 10] = [
        Self::Previous,
        Self::Next,
        Self::Up,
        Self::Down,
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
        if key == BarKey::Activate || key == BarKey::Down {
            self.current = None;
            return BarFocusOutcome::Activate(current);
        }
        let last = targets.len() - 1;
        let next = match key {
            BarKey::First => 0,
            BarKey::Last => last,
            BarKey::Next | BarKey::NextTab => (index + 1) % targets.len(),
            BarKey::Previous | BarKey::PreviousTab => index.checked_sub(1).unwrap_or(last),
            BarKey::Up | BarKey::Down | BarKey::Activate | BarKey::Exit => {
                return BarFocusOutcome::Unchanged;
            }
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
    use crate::pomodoro::FocusSubMode;
    use crate::pomodoro_view::PomodoroSurface;

    fn open_media_layout(media_tab: bool) -> crate::layout::Layout {
        open_layout(
            if media_tab {
                NotchExpandedTab::Media
            } else {
                NotchExpandedTab::Pomodoro
            },
            PomodoroSurface::Running,
        )
    }

    fn open_layout(
        active_tab: NotchExpandedTab,
        pomodoro_surface: PomodoroSurface,
    ) -> crate::layout::Layout {
        compute_layout_with_notch(
            &Metrics::MACBOOK_PRO_14,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                hud_expansion: 0.0,
                compact_expansion: 0.0,
                panel_expansion: 1.0,
                pomodoro_surface,
                active_tab,
                control_center_enabled: true,
                lyrics_progress: 0.0,
                tab_blend: 1.0,
                ..NotchLayoutInput::default()
            },
        )
    }

    #[test]
    fn notch_media_controls_join_keyboard_order_only_on_the_open_media_tab() {
        let media = open_media_layout(true);
        let targets = media.keyboard_targets_with_media(true);
        assert_eq!(
            &targets[targets.len() - 9..],
            &[
                HitTarget::NotchTabPomodoro,
                HitTarget::NotchTabShelf,
                HitTarget::NotchTabClipboard,
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
        assert!(without_media.contains(&HitTarget::NotchTabShelf));
        assert!(without_media.contains(&HitTarget::NotchTabClipboard));

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

        let shelf_targets = open_layout(NotchExpandedTab::Shelf, PomodoroSurface::Running)
            .keyboard_targets_with_media(true);
        assert!(!shelf_targets.contains(&HitTarget::PomodoroPlayPause));
        assert!(!shelf_targets.contains(&HitTarget::MediaPlayPause));
        assert!(shelf_targets.contains(&HitTarget::NotchTabShelf));
        assert!(shelf_targets.contains(&HitTarget::NotchTabClipboard));

        let closed = compute_layout_for_app(&Metrics::MACBOOK_PRO_14, 1920, 96, "Finder");
        assert!(
            !closed
                .keyboard_targets_with_media(true)
                .contains(&HitTarget::MediaPlayPause)
        );
    }

    fn notch_tail(layout: &crate::layout::Layout, count: usize) -> Vec<HitTarget> {
        let targets = layout.keyboard_targets_with_media(true);
        targets[targets.len() - count..].to_vec()
    }

    #[test]
    fn pomodoro_keyboard_order_follows_the_surface() {
        assert_eq!(
            notch_tail(
                &open_layout(NotchExpandedTab::Pomodoro, PomodoroSurface::Setup),
                12
            ),
            [
                HitTarget::NotchTabPomodoro,
                HitTarget::NotchTabShelf,
                HitTarget::NotchTabClipboard,
                HitTarget::NotchTabMedia,
                HitTarget::FocusMode(FocusSubMode::Pomodoro),
                HitTarget::FocusMode(FocusSubMode::Timer),
                HitTarget::FocusMode(FocusSubMode::Stopwatch),
                HitTarget::PomodoroPreset(0),
                HitTarget::PomodoroPreset(1),
                HitTarget::PomodoroPreset(2),
                HitTarget::PomodoroPreset(3),
                HitTarget::PomodoroPlayPause,
            ]
        );
        assert_eq!(
            notch_tail(
                &open_layout(NotchExpandedTab::Pomodoro, PomodoroSurface::Running),
                10
            ),
            [
                HitTarget::NotchTabPomodoro,
                HitTarget::NotchTabShelf,
                HitTarget::NotchTabClipboard,
                HitTarget::NotchTabMedia,
                HitTarget::FocusMode(FocusSubMode::Pomodoro),
                HitTarget::FocusMode(FocusSubMode::Timer),
                HitTarget::FocusMode(FocusSubMode::Stopwatch),
                HitTarget::PomodoroReset,
                HitTarget::PomodoroPlayPause,
                HitTarget::PomodoroSkip,
            ]
        );
        assert_eq!(
            notch_tail(
                &open_layout(NotchExpandedTab::Pomodoro, PomodoroSurface::Alert),
                2
            ),
            [HitTarget::PomodoroExtendFive, HitTarget::PomodoroStartNext]
        );
    }

    #[test]
    fn alert_keyboard_order_hides_every_other_notch_control() {
        let alert = open_layout(NotchExpandedTab::Pomodoro, PomodoroSurface::Alert);
        let targets = alert.keyboard_targets_with_media(true);
        assert!(!targets.contains(&HitTarget::NotchTabPomodoro));
        assert!(!targets.contains(&HitTarget::PomodoroPlayPause));
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

    #[test]
    fn down_activates_and_up_leaves_bar_focus_unchanged() {
        let mut focus = BarFocus::default();
        assert_eq!(focus.enter(&TARGETS), Some(HitTarget::Start));
        assert_eq!(
            focus.handle(BarKey::Up, &TARGETS),
            BarFocusOutcome::Unchanged
        );
        assert_eq!(
            focus.handle(BarKey::Down, &TARGETS),
            BarFocusOutcome::Activate(HitTarget::Start)
        );
        assert!(!focus.is_active());
    }

    #[test]
    fn clipboard_keyboard_order_follows_visible_items() {
        let layout = compute_layout_with_notch(
            &Metrics::MACBOOK_PRO_14,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Clipboard,
                clipboard: crate::ClipboardLayoutInput {
                    visible_count: 2,
                    has_pager: true,
                    has_items: true,
                    paused: false,
                },
                ..NotchLayoutInput::default()
            },
        );
        let targets = layout.keyboard_targets_with_media(false);
        assert!(targets.contains(&HitTarget::ClipboardPager));
        assert!(targets.contains(&HitTarget::ClipboardPauseToggle));
        assert!(targets.contains(&HitTarget::ClipboardClear));
        assert!(targets.contains(&HitTarget::ClipboardItem(0)));
        assert!(targets.contains(&HitTarget::ClipboardRemoveItem(0)));
        assert!(targets.contains(&HitTarget::ClipboardItem(1)));
        assert!(targets.contains(&HitTarget::ClipboardRemoveItem(1)));
        assert!(!targets.contains(&HitTarget::ClipboardItem(2)));
        assert!(!targets.contains(&HitTarget::ClipboardRemoveItem(2)));
    }

    #[test]
    fn collapsed_notch_secondary_privacy_and_shelf_actions_join_keyboard_order() {
        let collapsed = compute_layout_with_notch(
            &Metrics::MACBOOK_PRO_14,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 0.0,
                secondary_expansion: 1.0,
                privacy: crate::layout::PrivacyLayoutState {
                    compact_dots: true,
                    panel_mic_control: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        let collapsed_targets = collapsed.keyboard_targets_with_media(false);
        assert!(collapsed_targets.contains(&HitTarget::PrivacyIndicator));
        assert!(collapsed_targets.contains(&HitTarget::Notch));
        assert!(collapsed_targets.contains(&HitTarget::NotchSecondary));

        let shelf = compute_layout_with_notch(
            &Metrics::MACBOOK_PRO_14,
            1920,
            96,
            "Finder",
            NotchLayoutInput {
                panel_expansion: 1.0,
                active_tab: NotchExpandedTab::Shelf,
                shelf: crate::layout::ShelfLayoutInput {
                    visible_count: 2,
                    has_pager: false,
                    has_items: true,
                },
                ..NotchLayoutInput::default()
            },
        );
        let shelf_targets = shelf.keyboard_targets_with_media(false);
        assert!(!shelf_targets.contains(&HitTarget::Notch));
        assert!(shelf_targets.contains(&HitTarget::ShelfItem(0)));
        assert!(shelf_targets.contains(&HitTarget::ShelfCopyItem(0)));
        assert!(shelf_targets.contains(&HitTarget::ShelfRemoveItem(0)));
        assert!(shelf_targets.contains(&HitTarget::ShelfItem(1)));
        assert!(shelf_targets.contains(&HitTarget::ShelfCopyItem(1)));
        assert!(shelf_targets.contains(&HitTarget::ShelfRemoveItem(1)));
        assert!(!shelf_targets.contains(&HitTarget::ShelfItem(2)));
    }
}
