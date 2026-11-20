use std::fs;
use std::path::{Path, PathBuf};

use otto_core::TasksState;

use crate::diagnostics::log_error;
use crate::file::{data_dir, replace_file_atomically};

const TASKS_FILE: &str = "tasks.txt";

fn tasks_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(TASKS_FILE))
}

#[must_use]
pub fn load_tasks() -> TasksState {
    tasks_path()
        .map(|path| load_tasks_from(&path))
        .unwrap_or_default()
}

#[must_use]
pub fn save_tasks(state: &TasksState) -> bool {
    let Some(path) = tasks_path() else {
        return false;
    };
    let saved = save_tasks_to(&path, state);
    if !saved {
        log_error("tasks", "failed to save the task list");
    }
    saved
}

fn load_tasks_from(path: &Path) -> TasksState {
    fs::read_to_string(path)
        .map(|contents| TasksState::decode(&contents))
        .unwrap_or_default()
}

fn save_tasks_to(path: &Path, state: &TasksState) -> bool {
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    replace_file_atomically(path, state.encode().as_bytes()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{load_tasks_from, save_tasks_to};
    use otto_core::{TaskAddOutcome, TasksState};

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("otto-os-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn tasks_survive_a_save_and_load_cycle() {
        let dir = scratch_dir("tasks-roundtrip");
        let path = dir.join("nested").join("tasks.txt");
        let mut state = TasksState::default();
        assert!(matches!(
            state.add("kirim invoice"),
            TaskAddOutcome::Added(_)
        ));
        assert!(save_tasks_to(&path, &state));
        let loaded = load_tasks_from(&path);
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].text, "kirim invoice");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_or_corrupt_file_loads_as_an_empty_list() {
        let dir = scratch_dir("tasks-corrupt");
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("tasks.txt");
        assert!(load_tasks_from(&path).items.is_empty());
        std::fs::write(&path, [0xff, 0xfe, 0x00, 0x01]).expect("write");
        assert!(load_tasks_from(&path).items.is_empty());
        std::fs::write(&path, "not a task file").expect("write");
        assert!(load_tasks_from(&path).items.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
