use crate::layout::{HitTarget, Layout};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarKey {
    Previous,
    Next,
    First,
    Last,
    Activate,
    Exit,
}

impl BarKey {
    const ALL: [Self; 6] = [
        Self::Previous,
        Self::Next,
        Self::First,
        Self::Last,
        Self::Activate,
        Self::Exit,
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
        if key == BarKey::Activate {
            self.current = None;
            return BarFocusOutcome::Activate(current);
        }
        let last = targets.len() - 1;
        let index = targets.iter().position(|target| *target == current);
        let next = match (key, index) {
            (BarKey::First, _) | (BarKey::Next, None) => 0,
            (BarKey::Last, _) | (BarKey::Previous, None) => last,
            (BarKey::Next, Some(index)) => (index + 1) % targets.len(),
            (BarKey::Previous, Some(index)) => index.checked_sub(1).unwrap_or(last),
            (BarKey::Activate | BarKey::Exit, _) => return BarFocusOutcome::Unchanged,
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
    use crate::layout::{HitTarget, Metrics, compute_layout_for_app};

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
    fn key_codes_round_trip() {
        for key in [
            BarKey::Previous,
            BarKey::Next,
            BarKey::First,
            BarKey::Last,
            BarKey::Activate,
            BarKey::Exit,
        ] {
            assert_eq!(BarKey::from_code(key.code()), Some(key));
        }
        assert_eq!(BarKey::from_code(99), None);
    }
}
