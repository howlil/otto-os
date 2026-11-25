use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use otto_core::{
    ClipboardPayload, FinderClipboardData, FinderClipboardMode, FinderEntry, FinderEntryKind,
    FinderListing, FinderLocation, FinderPort, FinderPreviewData, FinderSidebarItem, IconBitmap,
    format_file_size,
};
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
use windows::core::{HSTRING, PCWSTR};

use super::fs::{
    build_default_sidebar, build_preview_base, create_unique_folder, delete_fs_permanently,
    duplicate_fs_paths, list_directory, list_recents, list_tagged_entries, list_trash_entries,
    load_tags_file, paste_fs_paths, rename_fs_path, resolve_user_dir_path, restore_trash_items,
    save_tags_file, user_home_dir,
};
use crate::dock::trash::{empty_recycle_bin, recycle_path};
use crate::notch::clipboard::{ClipboardRead, copy_clipboard_payload, read_clipboard_payload};
use crate::notch::shelf::extract_shell_thumbnail;

const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;

static LAST_CUT_SEQ: AtomicU32 = AtomicU32::new(0);
static LAST_WAS_CUT: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub(super) struct VolumeInfo {
    pub(super) root: String,
    pub(super) label: String,
    pub(super) removable: bool,
    pub(super) free_bytes: u64,
    pub(super) total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsFinderPort;

#[must_use]
pub fn extract_finder_thumbnail(path: &str, max_px: u32) -> Option<IconBitmap> {
    let (_, thumb) = extract_shell_thumbnail(path, max_px, false);
    thumb.map(|t| IconBitmap {
        width: t.width,
        height: t.height,
        pixels: t.bgra,
    })
}

fn query_volumes() -> Vec<VolumeInfo> {
    let mut volumes = Vec::new();
    unsafe {
        let mask = GetLogicalDrives();
        for bit in 0..26u8 {
            if (mask & (1 << bit)) == 0 {
                continue;
            }
            let letter = char::from(b'A' + bit);
            let root = format!("{letter}:\\");
            let wide = HSTRING::from(root.as_str());
            let dtype = GetDriveTypeW(PCWSTR(wide.as_ptr()));
            if dtype != DRIVE_FIXED && dtype != DRIVE_REMOVABLE && dtype != DRIVE_REMOTE {
                continue;
            }
            let mut free = 0u64;
            let mut total = 0u64;
            let _ = GetDiskFreeSpaceExW(
                PCWSTR(wide.as_ptr()),
                Some(&raw mut free),
                Some(&raw mut total),
                None,
            );
            let label = match dtype {
                DRIVE_REMOVABLE => format!("Removable ({letter}:)"),
                DRIVE_REMOTE => format!("Network ({letter}:)"),
                _ => format!("Local Disk ({letter}:)"),
            };
            volumes.push(VolumeInfo {
                root,
                label,
                removable: dtype == DRIVE_REMOVABLE,
                free_bytes: free,
                total_bytes: total,
            });
        }
    }
    volumes
}

fn free_space_for_path(path: &str, volumes: &[VolumeInfo]) -> String {
    let upper = path.to_ascii_uppercase();
    volumes
        .iter()
        .find(|vol| upper.starts_with(&vol.root.to_ascii_uppercase()))
        .filter(|vol| vol.total_bytes > 0)
        .map_or_else(String::new, |vol| {
            format!("{} available", format_file_size(vol.free_bytes))
        })
}

impl FinderPort for WindowsFinderPort {
    fn sidebar_items(&self) -> Vec<FinderSidebarItem> {
        let drives: Vec<(String, String, bool)> = query_volumes()
            .into_iter()
            .map(|v| (v.root, v.label, v.removable))
            .collect();
        build_default_sidebar(&drives)
    }

    fn default_location(&self) -> FinderLocation {
        let home = user_home_dir();
        let docs = home.join("Documents");
        let target = if docs.is_dir() { docs } else { home };
        FinderLocation::Path(target.to_string_lossy().into_owned())
    }

    fn list_location(
        &self,
        location: &FinderLocation,
        recursive_query: Option<&str>,
    ) -> FinderListing {
        let volumes = query_volumes();
        match location {
            FinderLocation::Path(dir) => FinderListing {
                entries: list_directory(Path::new(dir), recursive_query),
                free_space_label: free_space_for_path(dir, &volumes),
            },
            FinderLocation::Recents => FinderListing {
                entries: list_recents(),
                free_space_label: String::new(),
            },
            FinderLocation::Trash => {
                let roots: Vec<String> = volumes.iter().map(|v| v.root.clone()).collect();
                FinderListing {
                    entries: list_trash_entries(&roots),
                    free_space_label: String::new(),
                }
            }
            FinderLocation::ThisPc => {
                let entries = volumes
                    .into_iter()
                    .map(|vol| FinderEntry {
                        path: vol.root,
                        name: vol.label,
                        kind: FinderEntryKind::Drive,
                        kind_label: if vol.removable {
                            String::from("Removable Drive")
                        } else {
                            String::from("Local Disk")
                        },
                        size_bytes: vol.total_bytes,
                        modified_unix: 0,
                        created_unix: 0,
                        modified_label: String::from("--"),
                        created_label: String::from("--"),
                        flags: 0,
                        tag_mask: 0,
                        drive_free_bytes: Some(vol.free_bytes),
                        drive_total_bytes: Some(vol.total_bytes),
                        trash_origin: None,
                    })
                    .collect();
                FinderListing {
                    entries,
                    free_space_label: String::new(),
                }
            }
            FinderLocation::Tag(_) => FinderListing::default(),
        }
    }

    fn list_tagged(&self, tag_mask_bit: u8, tags: &HashMap<String, u8>) -> FinderListing {
        list_tagged_entries(tag_mask_bit, tags)
    }

    fn load_tags(&self) -> HashMap<String, u8> {
        load_tags_file()
    }

    fn save_tags(&self, tags: &HashMap<String, u8>) -> bool {
        save_tags_file(tags)
    }

    fn open_path(&self, path: &str) -> bool {
        crate::actions::open_path(Path::new(path));
        true
    }

    fn open_in_terminal(&self, dir_path: &str) -> bool {
        Command::new("wt.exe")
            .args(["-d", dir_path])
            .spawn()
            .or_else(|_| {
                Command::new("powershell.exe")
                    .args([
                        "-NoExit",
                        "-Command",
                        &format!("Set-Location -LiteralPath '{dir_path}'"),
                    ])
                    .spawn()
            })
            .is_ok()
    }

    fn create_folder(&self, parent_dir: &str) -> Option<String> {
        create_unique_folder(parent_dir)
    }

    fn rename_path(&self, path: &str, new_name: &str) -> Option<String> {
        rename_fs_path(path, new_name)
    }

    fn duplicate_paths(&self, paths: &[String]) -> Vec<String> {
        duplicate_fs_paths(paths)
    }

    fn recycle_paths(&self, paths: &[String]) -> bool {
        let mut all_ok = true;
        for path in paths {
            if !recycle_path(path) {
                all_ok = false;
            }
        }
        all_ok
    }

    fn delete_paths_permanently(&self, paths: &[String]) -> bool {
        delete_fs_permanently(paths)
    }

    fn empty_trash(&self) -> bool {
        empty_recycle_bin();
        true
    }

    fn restore_trash_paths(&self, paths: &[String]) -> bool {
        restore_trash_items(paths)
    }

    fn copy_paths_to_clipboard(&self, paths: &[String], mode: FinderClipboardMode) -> bool {
        if let Some(seq) = copy_clipboard_payload(&ClipboardPayload::Files(paths.to_vec())) {
            LAST_CUT_SEQ.store(seq, Ordering::Release);
            LAST_WAS_CUT.store(mode == FinderClipboardMode::Cut, Ordering::Release);
            true
        } else {
            false
        }
    }

    fn read_clipboard_paths(&self) -> Option<FinderClipboardData> {
        match read_clipboard_payload() {
            ClipboardRead::Captured {
                seq,
                payload: ClipboardPayload::Files(paths),
            } => {
                let is_cut = LAST_WAS_CUT.load(Ordering::Acquire)
                    && LAST_CUT_SEQ.load(Ordering::Acquire) == seq;
                Some(FinderClipboardData {
                    paths,
                    mode: if is_cut {
                        FinderClipboardMode::Cut
                    } else {
                        FinderClipboardMode::Copy
                    },
                })
            }
            _ => None,
        }
    }

    fn paste_paths(
        &self,
        sources: &[String],
        dest_dir: &str,
        move_items: bool,
    ) -> Vec<(String, String)> {
        let moved = paste_fs_paths(sources, dest_dir, move_items);
        if move_items && !moved.is_empty() {
            LAST_WAS_CUT.store(false, Ordering::Release);
        }
        moved
    }

    fn load_preview(&self, entry: &FinderEntry) -> FinderPreviewData {
        let mut preview = build_preview_base(entry);
        if !entry.is_dir()
            && let Some(bitmap) = extract_finder_thumbnail(&entry.path, 256)
        {
            preview.dimensions_label = Some(format!("{} × {}", bitmap.width, bitmap.height));
            preview.bitmap = Some(bitmap);
        }
        preview
    }

    fn path_exists_dir(&self, raw: &str) -> Option<String> {
        resolve_user_dir_path(raw)
    }

    fn copy_text_to_clipboard(&self, text: &str) -> bool {
        copy_clipboard_payload(&ClipboardPayload::Text(String::from(text))).is_some()
    }
}
