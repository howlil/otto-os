use std::collections::{HashMap, HashSet};

use otto_core::ProcessThrottleKind;

pub(crate) struct ThrottleReason;

impl ThrottleReason {
    pub(crate) const AUTO: u8 = 1 << 0;
    pub(crate) const MANUAL: u8 = 1 << 1;
    pub(crate) const FOCUS: u8 = 1 << 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PowerThrottlingSnapshot {
    pub(crate) control_mask: u32,
    pub(crate) state_mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessOrigin {
    pub(crate) created: u64,
    pub(crate) lowered_from: Option<u32>,
    pub(crate) power_throttling: Option<PowerThrottlingSnapshot>,
}

struct ProcessThrottleEntry {
    origin: ProcessOrigin,
    reasons: u8,
}

#[derive(Default)]
pub(crate) struct ThrottleLedger {
    entries: HashMap<u32, ProcessThrottleEntry>,
}

impl ThrottleLedger {
    #[must_use]
    pub(crate) fn has_reason(&self, pid: u32, reason: u8) -> bool {
        self.entries
            .get(&pid)
            .is_some_and(|entry| (entry.reasons & reason) != 0)
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn has_any(&self, pid: u32) -> bool {
        self.entries.contains_key(&pid)
    }

    #[must_use]
    pub(crate) fn throttle_kind(&self, pid: u32) -> ProcessThrottleKind {
        let Some(entry) = self.entries.get(&pid) else {
            return ProcessThrottleKind::None;
        };
        if (entry.reasons & ThrottleReason::MANUAL) != 0 {
            ProcessThrottleKind::ManualEcoQos
        } else if (entry.reasons & (ThrottleReason::AUTO | ThrottleReason::FOCUS)) != 0 {
            ProcessThrottleKind::AutoEcoQos
        } else {
            ProcessThrottleKind::None
        }
    }

    pub(crate) fn add_reason_if_present(&mut self, pid: u32, created: u64, reason: u8) -> bool {
        if let Some(entry) = self.entries.get_mut(&pid)
            && entry.origin.created == created
        {
            entry.reasons |= reason;
            return true;
        }
        false
    }

    pub(crate) fn record_or_add_reason(&mut self, pid: u32, reason: u8, origin: ProcessOrigin) {
        if let Some(entry) = self.entries.get_mut(&pid)
            && entry.origin.created == origin.created
        {
            entry.reasons |= reason;
            return;
        }
        self.entries.insert(
            pid,
            ProcessThrottleEntry {
                origin,
                reasons: reason,
            },
        );
    }

    pub(crate) fn release_reason(&mut self, pid: u32, reason: u8) -> Option<ProcessOrigin> {
        let entry = self.entries.get_mut(&pid)?;
        entry.reasons &= !reason;
        if entry.reasons == 0 {
            self.entries.remove(&pid).map(|removed| removed.origin)
        } else {
            None
        }
    }

    pub(crate) fn release_all_for_reason(&mut self, reason: u8) -> Vec<(u32, ProcessOrigin)> {
        let mut to_remove = Vec::new();
        for (&pid, entry) in &mut self.entries {
            entry.reasons &= !reason;
            if entry.reasons == 0 {
                to_remove.push(pid);
            }
        }
        to_remove
            .into_iter()
            .filter_map(|pid| self.entries.remove(&pid).map(|e| (pid, e.origin)))
            .collect()
    }

    pub(crate) fn release_all(&mut self) -> Vec<(u32, ProcessOrigin)> {
        self.entries
            .drain()
            .map(|(pid, entry)| (pid, entry.origin))
            .collect()
    }

    pub(crate) fn prune_dead(&mut self, live: &HashSet<u32>) {
        self.entries.retain(|pid, _| live.contains(pid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(created: u64) -> ProcessOrigin {
        ProcessOrigin {
            created,
            lowered_from: Some(0x20),
            power_throttling: Some(PowerThrottlingSnapshot {
                control_mask: 0,
                state_mask: 0,
            }),
        }
    }

    #[test]
    fn overlapping_reasons_restore_only_on_final_release() {
        let mut ledger = ThrottleLedger::default();
        ledger.record_or_add_reason(42, ThrottleReason::AUTO, origin(100));
        assert_eq!(ledger.throttle_kind(42), ProcessThrottleKind::AutoEcoQos);

        assert!(ledger.add_reason_if_present(42, 100, ThrottleReason::FOCUS));
        assert!(ledger.has_reason(42, ThrottleReason::AUTO));
        assert!(ledger.has_reason(42, ThrottleReason::FOCUS));

        assert!(ledger.release_reason(42, ThrottleReason::AUTO).is_none());
        assert!(ledger.has_any(42));

        let restored = ledger.release_reason(42, ThrottleReason::FOCUS);
        assert_eq!(restored, Some(origin(100)));
        assert!(!ledger.has_any(42));
        assert_eq!(ledger.throttle_kind(42), ProcessThrottleKind::None);
    }

    #[test]
    fn manual_toggle_off_preserves_focus_reason_and_origin() {
        let mut ledger = ThrottleLedger::default();
        ledger.record_or_add_reason(77, ThrottleReason::MANUAL, origin(500));
        ledger.record_or_add_reason(
            77,
            ThrottleReason::FOCUS,
            ProcessOrigin {
                created: 500,
                lowered_from: Some(0x4000),
                power_throttling: None,
            },
        );
        assert_eq!(ledger.throttle_kind(77), ProcessThrottleKind::ManualEcoQos);
        assert!(
            ledger
                .release_reason(77, ThrottleReason::MANUAL | ThrottleReason::AUTO)
                .is_none()
        );
        assert_eq!(ledger.throttle_kind(77), ProcessThrottleKind::AutoEcoQos);

        let all_focus = ledger.release_all_for_reason(ThrottleReason::FOCUS);
        assert_eq!(all_focus, vec![(77, origin(500))]);
        assert!(!ledger.has_any(77));
    }

    #[test]
    fn pid_reuse_with_different_creation_time_replaces_stale_entry() {
        let mut ledger = ThrottleLedger::default();
        ledger.record_or_add_reason(99, ThrottleReason::AUTO, origin(100));
        assert!(!ledger.add_reason_if_present(99, 200, ThrottleReason::FOCUS));
        ledger.record_or_add_reason(99, ThrottleReason::FOCUS, origin(200));
        assert!(!ledger.has_reason(99, ThrottleReason::AUTO));
        assert_eq!(
            ledger.release_reason(99, ThrottleReason::FOCUS),
            Some(origin(200))
        );
    }
}
