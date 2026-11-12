use crate::bar_focus::BarKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockFocusOutcome {
    Moved(usize),
    Activate(usize),
    Exited,
    ReturnToBar,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DockFocus {
    current: Option<usize>,
}

impl DockFocus {
    #[must_use]
    pub const fn current(self) -> Option<usize> {
        self.current
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        self.current.is_some()
    }

    pub fn enter(&mut self, count: usize, from_end: bool) -> Option<usize> {
        self.current = match (count, from_end) {
            (0, _) => None,
            (count, true) => Some(count - 1),
            (_, false) => Some(0),
        };
        self.current
    }

    pub fn exit(&mut self) -> bool {
        self.current.take().is_some()
    }

    pub fn revalidate(&mut self, count: usize) -> bool {
        match self.current {
            Some(current) if current >= count => {
                self.current = (count > 0).then_some(0);
                true
            }
            _ => false,
        }
    }

    pub fn handle(&mut self, key: BarKey, count: usize) -> DockFocusOutcome {
        let Some(current) = self.current else {
            return DockFocusOutcome::Unchanged;
        };
        if key == BarKey::Exit || count == 0 {
            self.current = None;
            return DockFocusOutcome::Exited;
        }
        if current >= count {
            self.current = Some(0);
            return DockFocusOutcome::Moved(0);
        }
        if key == BarKey::Activate {
            self.current = None;
            return DockFocusOutcome::Activate(current);
        }
        let last = count - 1;
        let next = match key {
            BarKey::Next => Some((current + 1) % count),
            BarKey::Previous => Some(current.checked_sub(1).unwrap_or(last)),
            BarKey::First => Some(0),
            BarKey::Last => Some(last),
            BarKey::NextTab => current.checked_add(1).filter(|index| *index <= last),
            BarKey::PreviousTab => current.checked_sub(1),
            BarKey::Activate | BarKey::Exit => None,
        };
        match next {
            Some(index) if index == current => DockFocusOutcome::Unchanged,
            Some(index) => {
                self.current = Some(index);
                DockFocusOutcome::Moved(index)
            }
            None if matches!(key, BarKey::NextTab | BarKey::PreviousTab) => {
                self.current = None;
                DockFocusOutcome::ReturnToBar
            }
            None => DockFocusOutcome::Unchanged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DockFocus, DockFocusOutcome};
    use crate::bar_focus::BarKey;

    fn at(index: usize) -> DockFocus {
        let mut focus = DockFocus::default();
        focus.enter(5, false);
        for _ in 0..index {
            focus.handle(BarKey::NextTab, 5);
        }
        focus
    }

    #[test]
    fn entering_focuses_an_edge_and_an_empty_dock_takes_no_focus() {
        let mut focus = DockFocus::default();
        assert_eq!(focus.enter(4, false), Some(0));
        assert_eq!(focus.enter(4, true), Some(3));
        assert_eq!(DockFocus::default().enter(0, false), None);
    }

    #[test]
    fn tab_stops_at_the_ends_so_the_bar_can_take_over() {
        let mut focus = at(4);
        assert_eq!(
            focus.handle(BarKey::NextTab, 5),
            DockFocusOutcome::ReturnToBar
        );
        assert!(!focus.is_active());

        let mut focus = at(0);
        assert_eq!(
            focus.handle(BarKey::PreviousTab, 5),
            DockFocusOutcome::ReturnToBar
        );
        assert!(!focus.is_active());
    }

    #[test]
    fn arrows_wrap_while_tab_does_not() {
        let mut focus = at(4);
        assert_eq!(focus.handle(BarKey::Next, 5), DockFocusOutcome::Moved(0));
        assert_eq!(
            focus.handle(BarKey::Previous, 5),
            DockFocusOutcome::Moved(4)
        );
    }

    #[test]
    fn home_and_end_jump_to_the_edges() {
        let mut focus = at(2);
        assert_eq!(focus.handle(BarKey::Last, 5), DockFocusOutcome::Moved(4));
        assert_eq!(focus.handle(BarKey::First, 5), DockFocusOutcome::Moved(0));
        assert_eq!(focus.handle(BarKey::First, 5), DockFocusOutcome::Unchanged);
    }

    #[test]
    fn activate_reports_the_index_and_leaves_focus() {
        let mut focus = at(3);
        assert_eq!(
            focus.handle(BarKey::Activate, 5),
            DockFocusOutcome::Activate(3)
        );
        assert!(!focus.is_active());
    }

    #[test]
    fn escape_exits_without_activating() {
        let mut focus = at(1);
        assert_eq!(focus.handle(BarKey::Exit, 5), DockFocusOutcome::Exited);
        assert!(!focus.is_active());
    }

    #[test]
    fn exiting_after_escape_reports_nothing_left_to_clear() {
        let mut focus = at(1);
        assert_eq!(focus.handle(BarKey::Exit, 5), DockFocusOutcome::Exited);
        assert!(!focus.exit());
    }

    #[test]
    fn a_stale_index_is_never_reported_past_the_end_of_the_dock() {
        let mut focus = at(4);
        assert_eq!(
            focus.handle(BarKey::Previous, 3),
            DockFocusOutcome::Moved(0)
        );
        assert_eq!(focus.current(), Some(0));
        let mut focus = at(4);
        assert_eq!(
            focus.handle(BarKey::Activate, 3),
            DockFocusOutcome::Moved(0)
        );
        assert!(focus.is_active());
        let mut focus = at(4);
        assert_eq!(focus.handle(BarKey::Exit, 0), DockFocusOutcome::Exited);
    }

    #[test]
    fn a_vanished_item_moves_focus_back_to_the_first() {
        let mut focus = at(4);
        assert!(focus.revalidate(3));
        assert_eq!(focus.current(), Some(0));
        assert!(!focus.revalidate(3));
        assert!(focus.revalidate(0));
        assert!(!focus.is_active());
    }
}
