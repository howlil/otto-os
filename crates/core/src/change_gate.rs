#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeGate {
    seen: Option<u64>,
    quiet_ticks: u32,
    fallback_ticks: u32,
}

impl ChangeGate {
    #[must_use]
    pub const fn new(fallback_ticks: u32) -> Self {
        Self {
            seen: None,
            quiet_ticks: 0,
            fallback_ticks,
        }
    }

    pub fn should_refresh(&mut self, generation: Option<u64>) -> bool {
        self.quiet_ticks = self.quiet_ticks.saturating_add(1);
        let changed = generation.is_none() || generation != self.seen;
        if changed || self.quiet_ticks >= self.fallback_ticks {
            self.seen = generation;
            self.quiet_ticks = 0;
            return true;
        }
        false
    }

    pub fn invalidate(&mut self) {
        self.seen = None;
    }
}

#[cfg(test)]
mod tests {
    use super::ChangeGate;

    #[test]
    fn first_tick_always_refreshes() {
        let mut gate = ChangeGate::new(10);
        assert!(gate.should_refresh(Some(0)));
    }

    #[test]
    fn unchanged_generation_waits_for_the_fallback() {
        let mut gate = ChangeGate::new(3);
        assert!(gate.should_refresh(Some(5)));
        assert!(!gate.should_refresh(Some(5)));
        assert!(!gate.should_refresh(Some(5)));
        assert!(gate.should_refresh(Some(5)));
        assert!(!gate.should_refresh(Some(5)));
    }

    #[test]
    fn a_new_generation_refreshes_immediately_and_restarts_the_fallback() {
        let mut gate = ChangeGate::new(3);
        assert!(gate.should_refresh(Some(1)));
        assert!(!gate.should_refresh(Some(1)));
        assert!(gate.should_refresh(Some(2)));
        assert!(!gate.should_refresh(Some(2)));
        assert!(!gate.should_refresh(Some(2)));
        assert!(gate.should_refresh(Some(2)));
    }

    #[test]
    fn missing_events_fall_back_to_polling_every_tick() {
        let mut gate = ChangeGate::new(100);
        assert!(gate.should_refresh(None));
        assert!(gate.should_refresh(None));
        assert!(gate.should_refresh(None));
    }

    #[test]
    fn invalidation_forces_the_next_refresh() {
        let mut gate = ChangeGate::new(100);
        assert!(gate.should_refresh(Some(7)));
        assert!(!gate.should_refresh(Some(7)));
        gate.invalidate();
        assert!(gate.should_refresh(Some(7)));
        assert!(!gate.should_refresh(Some(7)));
    }
}
