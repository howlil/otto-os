mod fs;
mod ops;
mod watcher;

pub use self::ops::{WindowsFinderPort, extract_finder_thumbnail};
pub use self::watcher::{FinderDirWatcher, WM_APP_FINDER_FS_CHANGE};

#[cfg(test)]
mod tests {
    use std::fs as std_fs;

    use otto_core::{FinderLocation, FinderPort};

    use super::WindowsFinderPort;

    #[test]
    fn creates_renames_duplicates_and_deletes_inside_temp_dir() {
        let base = std::env::temp_dir().join(format!("otto-finder-test-{}", std::process::id()));
        let _ = std_fs::remove_dir_all(&base);
        std_fs::create_dir_all(&base).expect("create temp test root");
        let port = WindowsFinderPort;
        let base_str = base.to_string_lossy().into_owned();

        let created = port.create_folder(&base_str).expect("create folder");
        let renamed = port.rename_path(&created, "Design").expect("rename folder");
        assert!(renamed.ends_with("Design"));

        let file_path = base.join("hello.txt");
        std_fs::write(&file_path, "hello finder").expect("write file");
        let file_str = file_path.to_string_lossy().into_owned();
        let copies = port.duplicate_paths(std::slice::from_ref(&file_str));
        assert_eq!(copies.len(), 1);
        assert!(copies[0].ends_with("hello copy.txt"));

        let listing = port.list_location(&FinderLocation::Path(base_str), None);
        assert!(listing.entries.iter().any(|e| e.name == "Design"));
        assert!(listing.entries.iter().any(|e| e.name == "hello.txt"));
        assert!(listing.entries.iter().any(|e| e.name == "hello copy.txt"));

        assert!(port.delete_paths_permanently(&[file_str, copies[0].clone(), renamed]));
        let _ = std_fs::remove_dir_all(&base);
    }
}
