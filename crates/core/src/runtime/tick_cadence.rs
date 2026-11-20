#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickCadence {
    active_ms: u32,
    idle_ms: u32,
    idle_after_ticks: u32,
    quiet_ticks: u32,
}

impl TickCadence {
    #[must_use]
    pub const fn new(active_ms: u32, idle_ms: u32, idle_after_ticks: u32) -> Self {
        Self {
            active_ms,
            idle_ms,
            idle_after_ticks,
            quiet_ticks: 0,
        }
    }

    #[must_use]
    pub const fn is_idle(&self) -> bool {
        self.quiet_ticks >= self.idle_after_ticks
    }

    #[must_use]
    pub const fn interval_ms(&self) -> u32 {
        if self.is_idle() {
            self.idle_ms
        } else {
            self.active_ms
        }
    }

    pub fn observe(&mut self, busy: bool) -> Option<u32> {
        let before = self.interval_ms();
        self.quiet_ticks = if busy {
            0
        } else {
            self.quiet_ticks.saturating_add(1)
        };
        self.changed_from(before)
    }

    pub fn wake(&mut self) -> Option<u32> {
        if !self.is_idle() {
            return None;
        }
        let before = self.interval_ms();
        self.quiet_ticks = 0;
        self.changed_from(before)
    }

    fn changed_from(&self, before: u32) -> Option<u32> {
        let after = self.interval_ms();
        (after != before).then_some(after)
    }
}

#[cfg(test)]
mod tests {
    use super::TickCadence;

    #[test]
    fn starts_active() {
        let cadence = TickCadence::new(120, 1000, 3);
        assert!(!cadence.is_idle());
        assert_eq!(cadence.interval_ms(), 120);
    }

    #[test]
    fn slows_down_only_after_enough_quiet_ticks() {
        let mut cadence = TickCadence::new(120, 1000, 3);
        assert_eq!(cadence.observe(false), None);
        assert_eq!(cadence.observe(false), None);
        assert_eq!(cadence.observe(false), Some(1000));
        assert!(cadence.is_idle());
        assert_eq!(cadence.observe(false), None);
    }

    #[test]
    fn busy_tick_restarts_the_quiet_count() {
        let mut cadence = TickCadence::new(120, 1000, 3);
        cadence.observe(false);
        cadence.observe(false);
        assert_eq!(cadence.observe(true), None);
        assert_eq!(cadence.observe(false), None);
        assert_eq!(cadence.observe(false), None);
        assert_eq!(cadence.observe(false), Some(1000));
    }

    #[test]
    fn busy_tick_while_idle_returns_to_the_active_interval() {
        let mut cadence = TickCadence::new(150, 1000, 1);
        assert_eq!(cadence.observe(false), Some(1000));
        assert_eq!(cadence.observe(true), Some(150));
        assert!(!cadence.is_idle());
    }

    #[test]
    fn wake_only_acts_when_idle() {
        let mut cadence = TickCadence::new(120, 1000, 2);
        cadence.observe(false);
        assert_eq!(cadence.wake(), None);
        assert_eq!(cadence.observe(false), Some(1000));
        assert_eq!(cadence.wake(), Some(120));
        assert_eq!(cadence.wake(), None);
    }

    #[test]
    fn wake_while_active_keeps_the_quiet_count() {
        let mut cadence = TickCadence::new(120, 1000, 2);
        cadence.observe(false);
        cadence.wake();
        assert_eq!(cadence.observe(false), Some(1000));
    }

    #[test]
    fn zero_idle_threshold_is_idle_immediately() {
        let cadence = TickCadence::new(120, 1000, 0);
        assert!(cadence.is_idle());
        assert_eq!(cadence.interval_ms(), 1000);
    }
}
