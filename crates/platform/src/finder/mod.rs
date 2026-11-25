mod fs;
mod mutate;
mod ops;
mod recycle;
mod watcher;

pub use self::ops::{WindowsFinderPort, extract_finder_thumbnail};
pub use self::watcher::{FinderDirWatcher, WM_APP_FINDER_FS_CHANGE};

#[cfg(test)]
mod tests {
    use std::fs as std_fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use otto_core::{FinderClipboardMode, FinderItemStatus, FinderLocation, FinderPort};

    use super::WindowsFinderPort;
    use super::mutate::{is_same_or_descendant, move_exact_paths, paste_fs_paths};
    use super::ops::{
        build_powershell_terminal_command, consume_completed_cuts, record_clipboard_cut,
        resolve_clipboard_cut_mode,
    };
    use super::recycle::{parse_recycle_info_bytes, restore_trash_items};
    use super::watcher::{FinderDirWatcher, WatcherDebounce};

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("otto-finder-{label}-{}", std::process::id()));
            let _ = std_fs::remove_dir_all(&path);
            std_fs::create_dir_all(&path).expect("create temp test root");
            Self(path)
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std_fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn creates_renames_duplicates_and_deletes_inside_temp_dir() {
        let base = TempDirGuard::new("crud");
        let port = WindowsFinderPort;
        let base_str = base.0.to_string_lossy().into_owned();

        let created = port.create_folder(&base_str).expect("create folder");
        let renamed = port.rename_path(&created, "Design").expect("rename folder");
        assert!(renamed.ends_with("Design"));

        let file_path = base.0.join("hello.txt");
        std_fs::write(&file_path, "hello finder").expect("write file");
        let file_str = file_path.to_string_lossy().into_owned();
        let copies = port.duplicate_paths(std::slice::from_ref(&file_str));
        let copy_paths = copies.completed_destinations();
        assert_eq!(copy_paths.len(), 1);
        assert!(copy_paths[0].ends_with("hello copy.txt"));

        let listing = port.list_location(&FinderLocation::Path(base_str), None);
        assert!(listing.entries.iter().any(|e| e.name == "Design"));
        assert!(listing.entries.iter().any(|e| e.name == "hello.txt"));
        assert!(listing.entries.iter().any(|e| e.name == "hello copy.txt"));

        let deleted = port.delete_paths_permanently(&[file_str, copy_paths[0].clone(), renamed]);
        assert!(deleted.all_completed());
    }

    #[test]
    fn rejects_self_and_descendant_copy_while_allowing_prefix_sibling() {
        let base = TempDirGuard::new("containment");
        let folder_a = base.0.join("A");
        let child = folder_a.join("child");
        let sibling_ab = base.0.join("AB");
        std_fs::create_dir_all(&child).expect("create A/child");
        std_fs::create_dir_all(&sibling_ab).expect("create AB");
        std_fs::write(folder_a.join("data.txt"), b"keep").expect("write data");

        assert!(is_same_or_descendant(&folder_a, &folder_a).expect("same"));
        assert!(is_same_or_descendant(&folder_a, &child).expect("descendant"));
        assert!(!is_same_or_descendant(&folder_a, &sibling_ab).expect("prefix sibling"));

        let src = vec![folder_a.to_string_lossy().into_owned()];
        let into_child = paste_fs_paths(&src, &child.to_string_lossy(), false);
        assert_eq!(into_child.items[0].status, FinderItemStatus::Conflict);
        assert!(!child.join("A").exists());

        let into_sibling = paste_fs_paths(&src, &sibling_ab.to_string_lossy(), false);
        assert_eq!(into_sibling.items[0].status, FinderItemStatus::Completed);
        assert!(sibling_ab.join("A").join("data.txt").exists());
    }

    #[test]
    fn move_exact_refuses_to_overwrite_existing_destination() {
        let base = TempDirGuard::new("conflict");
        let src = base.0.join("src.txt");
        let dst = base.0.join("dst.txt");
        std_fs::write(&src, b"source").expect("write src");
        std_fs::write(&dst, b"existing").expect("write dst");

        let outcome = move_exact_paths(&[(
            src.to_string_lossy().into_owned(),
            dst.to_string_lossy().into_owned(),
        )]);
        assert_eq!(outcome.items[0].status, FinderItemStatus::Conflict);
        assert_eq!(std_fs::read(&src).expect("read src"), b"source");
        assert_eq!(std_fs::read(&dst).expect("read dst"), b"existing");
    }

    #[test]
    fn powershell_terminal_command_uses_current_dir_without_interpolation() {
        let dir = PathBuf::from(r"C:\Temp\O'Brien; whoami $(calc)");
        let cmd = build_powershell_terminal_command(&dir);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["-NoExit"]);
        assert_eq!(cmd.get_current_dir(), Some(dir.as_path()));
        assert!(!WindowsFinderPort.open_in_terminal(r"C:\nonexistent-otto-dir-xyz"));
    }

    #[test]
    fn recycle_parser_rejects_malformed_and_restores_without_overwrite() {
        assert!(parse_recycle_info_bytes(&[0u8; 10]).is_none());

        let mut bad_ver = vec![0u8; 40];
        bad_ver[0..8].copy_from_slice(&3u64.to_le_bytes());
        assert!(parse_recycle_info_bytes(&bad_ver).is_none());

        let mut v2 = Vec::new();
        v2.extend_from_slice(&2u64.to_le_bytes());
        v2.extend_from_slice(&123u64.to_le_bytes());
        v2.extend_from_slice(&132_000_000_000_000_000u64.to_le_bytes());
        let path_u16: Vec<u16> = r"C:\Temp\restored.txt\0".encode_utf16().collect();
        let units: Vec<u16> = "C:\\Temp\\restored.txt\0".encode_utf16().collect();
        let _ = path_u16;
        v2.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for u in &units {
            v2.extend_from_slice(&u.to_le_bytes());
        }
        let parsed = parse_recycle_info_bytes(&v2).expect("valid v2");
        assert_eq!(parsed.0, 123);
        assert_eq!(parsed.2, r"C:\Temp\restored.txt");

        let base = TempDirGuard::new("recycle-restore");
        let bin_dir = base.0.join("bin");
        let target_dir = base.0.join("target");
        std_fs::create_dir_all(&bin_dir).expect("create bin");
        std_fs::create_dir_all(&target_dir).expect("create target");
        let occupant = target_dir.join("item.txt");
        std_fs::write(&occupant, b"occupant").expect("write occupant");
        let payload = bin_dir.join("$R123.txt");
        std_fs::write(&payload, b"trashed").expect("write payload");
        let info = bin_dir.join("$I123.txt");
        let target_str = format!("{}\0", occupant.to_string_lossy());
        let target_u16: Vec<u16> = target_str.encode_utf16().collect();
        let mut info_bytes = Vec::new();
        info_bytes.extend_from_slice(&2u64.to_le_bytes());
        info_bytes.extend_from_slice(&7u64.to_le_bytes());
        info_bytes.extend_from_slice(&132_000_000_000_000_000u64.to_le_bytes());
        info_bytes.extend_from_slice(&(target_u16.len() as u32).to_le_bytes());
        for u in &target_u16 {
            info_bytes.extend_from_slice(&u.to_le_bytes());
        }
        std_fs::write(&info, &info_bytes).expect("write $I");

        let outcome = restore_trash_items(&[payload.to_string_lossy().into_owned()]);
        assert_eq!(outcome.items[0].status, FinderItemStatus::Conflict);
        assert!(info.exists());
        assert!(payload.exists());
        assert_eq!(std_fs::read(&occupant).expect("occupant"), b"occupant");
    }

    #[test]
    fn watcher_debounce_delivers_trailing_edge_and_idle_infinite_wait() {
        let mut d = WatcherDebounce::default();
        let t0 = Instant::now();
        assert_eq!(d.next_wait_ms(t0), u32::MAX);
        assert!(d.on_event(t0));
        assert!(!d.on_event(t0 + Duration::from_millis(20)));
        assert!(d.next_wait_ms(t0 + Duration::from_millis(20)) <= 160);
        assert!(!d.on_timeout(t0 + Duration::from_millis(50)));
        assert!(d.on_timeout(t0 + Duration::from_millis(200)));
        assert_eq!(d.next_wait_ms(t0 + Duration::from_millis(200)), u32::MAX);
        assert!(FinderDirWatcher::start("", 0).is_none());
    }

    #[test]
    fn partial_cut_tracking_preserves_remaining_items_until_moved_or_seq_changes() {
        let a = String::from(r"C:\Temp\a.txt");
        let b = String::from(r"C:\Temp\b.txt");
        record_clipboard_cut(77, &[a.clone(), b.clone()], FinderClipboardMode::Cut);
        consume_completed_cuts(std::slice::from_ref(&a));
        let clip = resolve_clipboard_cut_mode(77, vec![a, b.clone()]);
        assert_eq!(clip.mode, FinderClipboardMode::Cut);
        assert_eq!(clip.paths, vec![b]);
        let stale = resolve_clipboard_cut_mode(78, vec![String::from(r"C:\Temp\c.txt")]);
        assert_eq!(stale.mode, FinderClipboardMode::Copy);
    }
}
