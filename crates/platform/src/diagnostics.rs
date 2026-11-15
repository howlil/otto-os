use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

const LOGS_SUBDIR: &str = "logs";
const LOG_FILE_NAME: &str = "otto-os.log";
const BACKUP_LOG_FILE_NAME: &str = "otto-os.log.1";
const MAX_LOG_BYTES: u64 = 512 * 1024;
const MAX_MESSAGE_BYTES: usize = 512;

static LOG_LOCK: Mutex<()> = Mutex::new(());

#[must_use]
pub fn logs_dir() -> Option<PathBuf> {
    crate::file::data_dir().map(|dir| dir.join(LOGS_SUBDIR))
}

pub fn log_info(component: &str, message: &str) {
    write_log("INFO", component, message);
}

pub fn log_error(component: &str, message: &str) {
    write_log("ERROR", component, message);
}

#[must_use]
pub fn open_logs_folder() -> bool {
    let Some(dir) = logs_dir() else {
        return false;
    };
    if fs::create_dir_all(&dir).is_err() {
        return false;
    }
    crate::actions::open_path(&dir);
    true
}

fn write_log(level: &str, component: &str, message: &str) {
    let Some(dir) = logs_dir() else {
        return;
    };
    let line = format_log_line(level, component, message);
    let _guard = LOG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = append_bounded_log(&dir, MAX_LOG_BYTES, &line);
}

fn sanitize_fragment(raw: &str, max_bytes: usize) -> String {
    let mut out = String::with_capacity(raw.len().min(max_bytes));
    for ch in raw.chars() {
        let mapped = if ch.is_control() { ' ' } else { ch };
        if out.len() + mapped.len_utf8() > max_bytes {
            break;
        }
        out.push(mapped);
    }
    out
}

fn format_log_line(level: &str, component: &str, message: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let pid = std::process::id();
    let clean_level = sanitize_fragment(level, 16);
    let clean_component = sanitize_fragment(component, 32);
    let clean_message = sanitize_fragment(message, MAX_MESSAGE_BYTES);
    format!("[ts:{ts}] [pid:{pid}] [{clean_level}] [{clean_component}] {clean_message}\n")
}

fn append_bounded_log(dir: &Path, max_bytes: u64, line: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let primary = dir.join(LOG_FILE_NAME);
    let backup = dir.join(BACKUP_LOG_FILE_NAME);
    let current_len = fs::metadata(&primary).ok().map_or(0, |meta| meta.len());
    if current_len > 0 && current_len.saturating_add(line.len() as u64) > max_bytes {
        let _ = fs::remove_file(&backup);
        let _ = fs::rename(&primary, &backup);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&primary)?;
    file.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        BACKUP_LOG_FILE_NAME, LOG_FILE_NAME, MAX_MESSAGE_BYTES, append_bounded_log,
        format_log_line, sanitize_fragment,
    };

    #[test]
    fn sanitizes_control_chars_and_bounds_line_length() {
        let dirty = format!("line1\r\nline2\t{}", "a".repeat(MAX_MESSAGE_BYTES + 128));
        let clean = sanitize_fragment(&dirty, MAX_MESSAGE_BYTES);
        assert!(!clean.contains('\r'));
        assert!(!clean.contains('\n'));
        assert!(clean.len() <= MAX_MESSAGE_BYTES);

        let formatted = format_log_line("INFO", "app", "started\nok");
        assert!(formatted.ends_with('\n'));
        assert!(formatted.contains("[INFO] [app] started ok"));
    }

    #[test]
    fn rotates_log_file_when_exceeding_max_bytes() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("otto-os-log-test-{suffix}"));
        let _ = fs::remove_dir_all(&dir);

        let first = "12345678901234567890\n";
        append_bounded_log(&dir, 30, first).expect("write first entry");
        append_bounded_log(&dir, 30, first).expect("rotate and write second entry");

        let backup = fs::read_to_string(dir.join(BACKUP_LOG_FILE_NAME)).expect("read backup");
        let primary = fs::read_to_string(dir.join(LOG_FILE_NAME)).expect("read primary");
        assert_eq!(backup, first);
        assert_eq!(primary, first);

        let _ = fs::remove_dir_all(&dir);
    }
}
