use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows::core::{Error, PCWSTR, Result};

pub fn replace_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, contents).map_err(io_error)?;
    let source = wide_path(&temp_path);
    let destination = wide_path(path);
    let result = unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
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
    use std::fs;

    use super::replace_file_atomically;

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
