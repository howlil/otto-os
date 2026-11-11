use std::path::PathBuf;
use std::sync::Mutex;

use otto_core::WindowIdentity;

use crate::file::replace_file_atomically;

const LEDGER_FILE: &str = "hidden_windows.txt";
const MAX_LEDGER_ENTRIES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Concealment {
    Hidden,
    TransitionsDisabled,
}

impl Concealment {
    const fn tag(self) -> &'static str {
        match self {
            Self::Hidden => "h",
            Self::TransitionsDisabled => "t",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "h" => Some(Self::Hidden),
            "t" => Some(Self::TransitionsDisabled),
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

fn ledger_path() -> Option<PathBuf> {
    crate::file::data_dir().map(|dir| dir.join(LEDGER_FILE))
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
        if entries.len() >= MAX_LEDGER_ENTRIES {
            break;
        }
        if !entries
            .iter()
            .any(|known| known.same_slot(entry.identity.hwnd, entry.kind))
        {
            entries.push(entry);
        }
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
    let Ok(mut ledger) = LEDGER.lock() else {
        return false;
    };
    if ledger
        .iter()
        .any(|known| known.same_slot(identity.hwnd, kind))
    {
        return true;
    }
    if ledger.len() >= MAX_LEDGER_ENTRIES {
        return false;
    }
    ledger.push(LedgerEntry { identity, kind });
    persist(&ledger);
    true
}

pub(crate) fn forget(hwnd: isize, kind: Concealment) {
    let Ok(mut ledger) = LEDGER.lock() else {
        return;
    };
    let before = ledger.len();
    ledger.retain(|known| !known.same_slot(hwnd, kind));
    if ledger.len() != before {
        persist(&ledger);
    }
}

pub(crate) fn recorded(kind: Concealment) -> Vec<WindowIdentity> {
    LEDGER
        .lock()
        .map(|ledger| {
            ledger
                .iter()
                .filter(|entry| entry.kind == kind)
                .map(|entry| entry.identity)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn take_all() -> Vec<LedgerEntry> {
    let mut entries = LEDGER
        .lock()
        .map(|mut ledger| std::mem::take(&mut *ledger))
        .unwrap_or_default();
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
        ];
        assert_eq!(parse_ledger(&format_ledger(&entries)), entries);
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
}
