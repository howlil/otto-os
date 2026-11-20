use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use otto_core::WindowIdentity;

use crate::file::replace_file_atomically;

const LEDGER_FILE: &str = "hidden_windows.txt";
const MAX_LEDGER_ENTRIES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Concealment {
    Hidden,
    TransitionsDisabled,
    RoundedCorners,
}

impl Concealment {
    const fn tag(self) -> &'static str {
        match self {
            Self::Hidden => "h",
            Self::TransitionsDisabled => "t",
            Self::RoundedCorners => "r",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "h" => Some(Self::Hidden),
            "t" => Some(Self::TransitionsDisabled),
            "r" => Some(Self::RoundedCorners),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LedgerEntry {
    pub identity: WindowIdentity,
    pub kind: Concealment,
}

impl LedgerEntry {
    fn same_slot(&self, hwnd: isize, kind: Concealment) -> bool {
        self.identity.hwnd == hwnd && self.kind == kind
    }
}

static LEDGER: Mutex<Vec<LedgerEntry>> = Mutex::new(Vec::new());

fn lock_ledger() -> MutexGuard<'static, Vec<LedgerEntry>> {
    LEDGER.lock().unwrap_or_else(PoisonError::into_inner)
}

fn ledger_path() -> Option<PathBuf> {
    crate::file::data_dir().map(|dir| dir.join(LEDGER_FILE))
}

fn upsert(entries: &mut Vec<LedgerEntry>, entry: LedgerEntry) -> bool {
    if let Some(known) = entries
        .iter_mut()
        .find(|known| known.same_slot(entry.identity.hwnd, entry.kind))
    {
        *known = entry;
        return true;
    }
    if entries.len() >= MAX_LEDGER_ENTRIES {
        return false;
    }
    entries.push(entry);
    true
}

fn parse_entry(line: &str) -> Option<LedgerEntry> {
    let mut fields = line.trim().split(',').map(str::trim);
    let identity = WindowIdentity {
        hwnd: fields.next()?.parse().ok()?,
        process_id: fields.next()?.parse().ok()?,
        thread_id: fields.next()?.parse().ok()?,
        generation: fields.next()?.parse().ok()?,
    };
    let kind = match fields.next() {
        Some(tag) => Concealment::from_tag(tag)?,
        None => Concealment::Hidden,
    };
    (fields.next().is_none() && identity.hwnd != 0 && identity.process_id != 0)
        .then_some(LedgerEntry { identity, kind })
}

fn parse_ledger(content: &str) -> Vec<LedgerEntry> {
    let mut entries: Vec<LedgerEntry> = Vec::new();
    for entry in content.lines().filter_map(parse_entry) {
        upsert(&mut entries, entry);
    }
    entries
}

fn format_ledger(entries: &[LedgerEntry]) -> String {
    use std::fmt::Write;
    let mut text = String::new();
    for entry in entries {
        let id = entry.identity;
        let _ = writeln!(
            text,
            "{},{},{},{},{}",
            id.hwnd,
            id.process_id,
            id.thread_id,
            id.generation,
            entry.kind.tag()
        );
    }
    text
}

fn persist(entries: &[LedgerEntry]) {
    let Some(path) = ledger_path() else {
        return;
    };
    if entries.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = replace_file_atomically(&path, format_ledger(entries).as_bytes());
}

pub(crate) fn record(identity: WindowIdentity, kind: Concealment) -> bool {
    let mut ledger = lock_ledger();
    if !upsert(&mut ledger, LedgerEntry { identity, kind }) {
        return false;
    }
    persist(&ledger);
    true
}

pub(crate) fn forget(hwnd: isize, kind: Concealment) {
    let mut ledger = lock_ledger();
    let before = ledger.len();
    ledger.retain(|known| !known.same_slot(hwnd, kind));
    if ledger.len() != before {
        persist(&ledger);
    }
}

pub(crate) fn retain_kind(kind: Concealment, keep: impl Fn(WindowIdentity) -> bool) {
    let mut ledger = lock_ledger();
    let before = ledger.len();
    ledger.retain(|entry| entry.kind != kind || keep(entry.identity));
    if ledger.len() != before {
        persist(&ledger);
    }
}

pub(crate) fn take_kind(kind: Concealment) -> Vec<LedgerEntry> {
    let mut ledger = lock_ledger();
    let (taken, kept): (Vec<_>, Vec<_>) = ledger.drain(..).partition(|entry| entry.kind == kind);
    *ledger = kept;
    if !taken.is_empty() {
        persist(&ledger);
    }
    taken
}

pub(crate) fn recorded(kind: Concealment) -> Vec<WindowIdentity> {
    lock_ledger()
        .iter()
        .filter(|entry| entry.kind == kind)
        .map(|entry| entry.identity)
        .collect()
}

pub(crate) fn take_all() -> Vec<LedgerEntry> {
    let mut entries = std::mem::take(&mut *lock_ledger());
    if let Some(path) = ledger_path() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            for entry in parse_ledger(&content) {
                if !entries
                    .iter()
                    .any(|known| known.same_slot(entry.identity.hwnd, entry.kind))
                {
                    entries.push(entry);
                }
            }
        }
        let _ = std::fs::remove_file(path);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::{
        Concealment, LedgerEntry, MAX_LEDGER_ENTRIES, format_ledger, parse_entry, parse_ledger,
        upsert,
    };
    use otto_core::WindowIdentity;

    fn entry(hwnd: isize, kind: Concealment) -> LedgerEntry {
        LedgerEntry {
            identity: WindowIdentity {
                hwnd,
                process_id: 42,
                thread_id: 43,
                generation: 133_000_000_000_000_000,
            },
            kind,
        }
    }

    #[test]
    fn ledger_round_trips_every_identity_field_and_kind() {
        let entries = vec![
            entry(100, Concealment::Hidden),
            entry(200, Concealment::TransitionsDisabled),
            entry(300, Concealment::RoundedCorners),
        ];
        assert_eq!(parse_ledger(&format_ledger(&entries)), entries);
    }

    #[test]
    fn the_same_window_can_hold_several_concealments_at_once() {
        let mut entries = Vec::new();
        assert!(upsert(&mut entries, entry(100, Concealment::Hidden)));
        assert!(upsert(
            &mut entries,
            entry(100, Concealment::RoundedCorners)
        ));
        assert!(upsert(
            &mut entries,
            entry(100, Concealment::RoundedCorners)
        ));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn legacy_four_field_lines_mean_hidden() {
        assert_eq!(
            parse_entry("100,42,43,133000000000000000"),
            Some(entry(100, Concealment::Hidden))
        );
    }

    #[test]
    fn malformed_and_zero_entries_are_ignored() {
        assert!(parse_entry("").is_none());
        assert!(parse_entry("1,2,3").is_none());
        assert!(parse_entry("1,2,3,4,x").is_none());
        assert!(parse_entry("1,2,3,4,h,extra").is_none());
        assert!(parse_entry("x,2,3,4").is_none());
        assert!(parse_entry("0,2,3,4").is_none());
        assert!(parse_entry("1,0,3,4").is_none());
        assert_eq!(parse_ledger("garbage\n100,42,43,7\n").len(), 1);
    }

    #[test]
    fn the_same_window_may_be_both_hidden_and_without_transitions() {
        let text = "5,42,43,7,h\n5,42,43,7,t\n5,42,43,7,t\n";
        assert_eq!(parse_ledger(text).len(), 2);
    }

    #[test]
    fn ledger_is_bounded_and_deduplicated() {
        use std::fmt::Write;
        let mut text = String::new();
        for hwnd in 1..=(MAX_LEDGER_ENTRIES * 2) {
            let _ = writeln!(text, "{hwnd},42,43,7");
            let _ = writeln!(text, "{hwnd},42,43,7");
        }
        assert_eq!(parse_ledger(&text).len(), MAX_LEDGER_ENTRIES);
    }

    fn entry_with_generation(hwnd: isize, generation: u64, kind: Concealment) -> LedgerEntry {
        LedgerEntry {
            identity: WindowIdentity {
                hwnd,
                process_id: 42,
                thread_id: 43,
                generation,
            },
            kind,
        }
    }

    #[test]
    fn later_line_for_a_reused_handle_wins_over_stale_identity() {
        let parsed = parse_ledger("5,42,43,7,h\n5,42,43,9,h\n");
        assert_eq!(
            parsed,
            vec![entry_with_generation(5, 9, Concealment::Hidden)]
        );
    }

    #[test]
    fn upsert_replaces_identity_of_an_existing_slot() {
        let mut entries = vec![entry_with_generation(5, 7, Concealment::Hidden)];
        assert!(upsert(
            &mut entries,
            entry_with_generation(5, 9, Concealment::Hidden)
        ));
        assert_eq!(
            entries,
            vec![entry_with_generation(5, 9, Concealment::Hidden)]
        );
    }

    #[test]
    fn upsert_refuses_new_slots_once_full_but_still_updates_known_ones() {
        let mut entries: Vec<LedgerEntry> = (1..=MAX_LEDGER_ENTRIES as isize)
            .map(|hwnd| entry_with_generation(hwnd, 7, Concealment::Hidden))
            .collect();
        assert!(!upsert(
            &mut entries,
            entry_with_generation(-1, 7, Concealment::Hidden)
        ));
        assert!(upsert(
            &mut entries,
            entry_with_generation(1, 8, Concealment::Hidden)
        ));
        assert_eq!(entries.len(), MAX_LEDGER_ENTRIES);
        assert_eq!(entries[0].identity.generation, 8);
    }
}
