use std::fs;
use std::path::PathBuf;

use otto_core::ControlCenterGrid;

use crate::system::file::{data_dir, replace_file_atomically};

const CONTROL_CENTER_CONFIG_FILE: &str = "control_center.txt";

fn config_path() -> Option<PathBuf> {
    Some(data_dir()?.join(CONTROL_CENTER_CONFIG_FILE))
}

#[must_use]
pub fn load_control_center_grid() -> ControlCenterGrid {
    let Some(path) = config_path() else {
        return ControlCenterGrid::default();
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return ControlCenterGrid::default();
    };
    ControlCenterGrid::parse_config(&contents)
}

#[must_use]
pub fn save_control_center_grid(grid: &ControlCenterGrid) -> bool {
    let Some(path) = config_path() else {
        return false;
    };
    if let Ok(existing) = fs::read_to_string(&path)
        && ControlCenterGrid::is_future_version(&existing)
    {
        return false;
    }
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    replace_file_atomically(&path, grid.format_config().as_bytes()).is_ok()
}
