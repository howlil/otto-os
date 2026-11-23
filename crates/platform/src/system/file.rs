use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows::core::{Error, PCWSTR, Result};

pub const DATA_DIR_OVERRIDE: &str = "OTTO_OS_DATA_DIR";
pub const PROD_DATA_DIR_NAME: &str = "otto-os";
pub const DEV_DATA_DIR_NAME: &str = "otto-os-dev";

#[must_use]
pub const fn default_data_dir_name(is_debug_build: bool) -> &'static str {
    if is_debug_build {
        DEV_DATA_DIR_NAME
    } else {
        PROD_DATA_DIR_NAME
    }
}

#[must_use]
pub fn resolve_data_dir(
    override_dir: Option<&OsStr>,
    local_app_data: Option<&OsStr>,
    is_debug_build: bool,
) -> Option<PathBuf> {
    if let Some(overridden) = override_dir
        && !overridden.is_empty()
    {
        return Some(PathBuf::from(overridden));
    }
    local_app_data.map(|root| PathBuf::from(root).join(default_data_dir_name(is_debug_build)))
}

#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    resolve_data_dir(
        std::env::var_os(DATA_DIR_OVERRIDE).as_deref(),
        std::env::var_os("LOCALAPPDATA").as_deref(),
        cfg!(debug_assertions),
    )
}

pub(crate) fn move_file_atomically(source_path: &Path, destination_path: &Path) -> Result<()> {
    let source = wide_path(source_path);
    let destination = wide_path(destination_path);
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
}

pub fn replace_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, contents).map_err(io_error)?;
    let result = move_file_atomically(&temp_path, path);
    if result.is_err() {
        let _ = fs::remove_file(temp_path);
    }
    result
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn io_error(_error: std::io::Error) -> Error {
    Error::new(
        windows::Win32::Foundation::E_FAIL,
        "Unable to write Otto plugin configuration",
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::path::PathBuf;

    use super::{replace_file_atomically, resolve_data_dir};

    #[test]
    fn debug_and_release_resolve_distinct_default_data_dirs_while_honoring_override() {
        let root = Some(OsStr::new(r"C:\Users\Demo\AppData\Local"));
        assert_eq!(
            resolve_data_dir(None, root, true),
            Some(PathBuf::from(r"C:\Users\Demo\AppData\Local\otto-os-dev"))
        );
        assert_eq!(
            resolve_data_dir(None, root, false),
            Some(PathBuf::from(r"C:\Users\Demo\AppData\Local\otto-os"))
        );
        assert_eq!(
            resolve_data_dir(Some(OsStr::new(r"D:\CustomOtto")), root, true),
            Some(PathBuf::from(r"D:\CustomOtto"))
        );
        assert_eq!(resolve_data_dir(Some(OsStr::new("")), None, false), None);
    }

    #[test]
    fn replaces_existing_file_contents() {
        let path =
            std::env::temp_dir().join(format!("otto-os-plugin-test-{}.txt", std::process::id()));
        let temp_path = path.with_extension("tmp");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&temp_path);

        replace_file_atomically(&path, b"first").expect("first config write");
        assert_eq!(
            fs::read_to_string(&path).expect("first config read"),
            "first"
        );
        replace_file_atomically(&path, b"second").expect("replacement config write");
        assert_eq!(
            fs::read_to_string(&path).expect("replacement config read"),
            "second"
        );

        let _ = fs::remove_file(path);
        let _ = fs::remove_file(temp_path);
    }
}
