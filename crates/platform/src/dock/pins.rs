use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(super) const USER_MODEL_KEY_PREFIX: &str = "aumid:";
pub(super) const MAX_PINNED_APPS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedApp {
    pub key: String,
    pub executable: String,
}

pub(super) fn parse_pinned_line(line: &str) -> Option<PinnedApp> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if line.starts_with(USER_MODEL_KEY_PREFIX) {
        let (key, executable) = line.split_once('|')?;
        let key = key.trim();
        let executable = executable.trim();
        let has_identity = key.len() > USER_MODEL_KEY_PREFIX.len();
        return (has_identity && !executable.is_empty()).then(|| PinnedApp {
            key: String::from(key),
            executable: String::from(executable),
        });
    }
    Some(PinnedApp {
        key: line.to_lowercase(),
        executable: String::from(line),
    })
}

pub(super) fn parse_pinned(content: &str) -> Vec<PinnedApp> {
    let mut seen = HashSet::new();
    content
        .lines()
        .filter_map(parse_pinned_line)
        .filter(|pin| seen.insert(pin.key.clone()))
        .collect()
}

pub(super) fn toggled_pins(
    mut pins: Vec<PinnedApp>,
    key: &str,
    executable: &str,
) -> Vec<PinnedApp> {
    if let Some(position) = pins.iter().position(|pin| pin.key == key) {
        pins.remove(position);
    } else if pins.len() < MAX_PINNED_APPS {
        pins.push(PinnedApp {
            key: String::from(key),
            executable: String::from(executable),
        });
    }
    pins
}

pub(super) fn format_pinned(pins: &[PinnedApp]) -> String {
    let mut body = String::new();
    for pin in pins {
        if pin.key.starts_with(USER_MODEL_KEY_PREFIX) {
            body.push_str(&pin.key);
            body.push('|');
        }
        body.push_str(&pin.executable);
        body.push('\n');
    }
    body
}

pub(super) fn move_pin(pins: &mut Vec<PinnedApp>, moved_key: &str, target_key: &str) -> bool {
    let Some(from) = pins.iter().position(|pin| pin.key == moved_key) else {
        return false;
    };
    let Some(to) = pins.iter().position(|pin| pin.key == target_key) else {
        return false;
    };
    if from == to {
        return false;
    }
    let pin = pins.remove(from);
    pins.insert(to, pin);
    true
}

pub(super) fn insert_pin_at(
    mut pins: Vec<PinnedApp>,
    key: &str,
    executable: &str,
    target_key: &str,
) -> Option<Vec<PinnedApp>> {
    if pins.iter().any(|pin| pin.key == key) || pins.len() >= MAX_PINNED_APPS {
        return None;
    }
    let target_pos = pins.iter().position(|pin| pin.key == target_key)?;
    pins.insert(
        target_pos,
        PinnedApp {
            key: String::from(key),
            executable: String::from(executable),
        },
    );
    Some(pins)
}

pub(super) fn pinned_config_path() -> Option<PathBuf> {
    crate::file::data_dir().map(|dir| dir.join("pinned.txt"))
}

pub(super) fn pinned_from_read(read: io::Result<String>) -> Option<Vec<PinnedApp>> {
    match read {
        Ok(content) => Some(parse_pinned(&content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

pub(super) fn read_pinned_file() -> Option<Vec<PinnedApp>> {
    let path = pinned_config_path()?;
    pinned_from_read(fs::read_to_string(path))
}

pub(super) fn available_pinned_apps() -> Vec<PinnedApp> {
    match pinned_config_path().map(fs::read_to_string) {
        Some(Err(error)) if error.kind() == io::ErrorKind::NotFound => {
            let defaults = discover_default_pinned_apps();
            let _ = save_pinned_apps(&defaults);
            return defaults;
        }
        _ => {}
    }
    read_pinned_file()
        .unwrap_or_default()
        .into_iter()
        .filter(|pin| Path::new(&pin.executable).exists())
        .collect()
}

pub(super) fn build_default_pinned_apps(
    candidates: &[PathBuf],
    exists: impl Fn(&Path) -> bool,
) -> Vec<PinnedApp> {
    let mut pins = Vec::new();
    for path in candidates {
        if !exists(path) {
            continue;
        }
        let exe = path.to_string_lossy().into_owned();
        let key = exe.to_ascii_lowercase();
        if !pins.iter().any(|existing: &PinnedApp| existing.key == key) {
            pins.push(PinnedApp {
                key,
                executable: exe,
            });
        }
    }
    pins
}

pub(super) fn discover_default_pinned_apps() -> Vec<PinnedApp> {
    let mut candidates = Vec::new();
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(pf86).join("Microsoft\\Edge\\Application\\msedge.exe"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(&pf).join("Microsoft\\Edge\\Application\\msedge.exe"));
        candidates.push(PathBuf::from(pf).join("Google\\Chrome\\Application\\chrome.exe"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(&local).join("Microsoft\\WindowsApps\\wt.exe"));
        candidates.push(PathBuf::from(local).join("Programs\\Microsoft VS Code\\Code.exe"));
    }
    if let Some(windir) = std::env::var_os("WINDIR") {
        candidates.push(PathBuf::from(&windir).join("System32\\notepad.exe"));
    }
    let mut pins = build_default_pinned_apps(&candidates, std::path::Path::exists);
    pins.truncate(4);
    pins
}

#[must_use]
pub fn remove_pinned_app(key: &str) -> bool {
    let Some(mut list) = read_pinned_file() else {
        return false;
    };
    let before = list.len();
    list.retain(|pin| pin.key != key);
    list.len() != before && save_pinned_apps(&list)
}

#[must_use]
pub fn reorder_pinned_apps(moved_key: &str, target_key: &str) -> bool {
    let Some(mut list) = read_pinned_file() else {
        return false;
    };
    move_pin(&mut list, moved_key, target_key) && save_pinned_apps(&list)
}

pub(super) fn save_pinned_apps(apps: &[PinnedApp]) -> bool {
    let Some(path) = pinned_config_path() else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    crate::file::replace_file_atomically(&path, format_pinned(apps).as_bytes()).is_ok()
}
