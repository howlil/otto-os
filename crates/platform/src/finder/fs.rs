use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use otto_core::{
    ENTRY_FLAG_HIDDEN, ENTRY_FLAG_READ_ONLY, ENTRY_FLAG_SYMLINK, FinderEntry, FinderEntryKind,
    FinderListing, FinderLocation, FinderPreviewData, FinderPreviewKind, FinderSidebarIcon,
    FinderSidebarItem, FinderSidebarSection, FinderTag, classify_entry_kind, format_file_size,
    format_tag_mask, format_unix_timestamp, is_text_preview_extension, parse_tag_mask,
    path_display_name,
};

use crate::file::{data_dir, replace_file_atomically};

const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const WINDOWS_EPOCH_OFFSET_SECS: i64 = 11_644_473_600;
const TAGS_FILE: &str = "finder_tags.txt";
const MAX_PREVIEW_TEXT_BYTES: usize = 32_768;
const MAX_RECURSIVE_RESULTS: usize = 250;
const MAX_RECURSIVE_DEPTH: usize = 4;

pub(super) fn filetime_to_unix_secs(ft: u64) -> i64 {
    let secs = (ft / 10_000_000) as i64;
    secs.saturating_sub(WINDOWS_EPOCH_OFFSET_SECS)
}

#[must_use]
pub(super) fn user_home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE").map_or_else(|| PathBuf::from(r"C:\"), PathBuf::from)
}

#[must_use]
pub(super) fn build_default_sidebar(drives: &[(String, String, bool)]) -> Vec<FinderSidebarItem> {
    let home = user_home_dir();
    let mut items = vec![FinderSidebarItem {
        section: FinderSidebarSection::Favorites,
        label: String::from("Recents"),
        icon: FinderSidebarIcon::Recents,
        location: FinderLocation::Recents,
        ejectable: false,
    }];
    let folders = [
        ("Desktop", "Desktop", FinderSidebarIcon::Desktop),
        ("Documents", "Documents", FinderSidebarIcon::Documents),
        ("Downloads", "Downloads", FinderSidebarIcon::Downloads),
        ("Pictures", "Pictures", FinderSidebarIcon::Pictures),
        ("Music", "Music", FinderSidebarIcon::Music),
        ("Videos", "Videos", FinderSidebarIcon::Videos),
    ];
    for (label, sub, icon) in folders {
        let candidate = home.join(sub);
        if candidate.is_dir() {
            items.push(FinderSidebarItem {
                section: FinderSidebarSection::Favorites,
                label: String::from(label),
                icon,
                location: FinderLocation::Path(candidate.to_string_lossy().into_owned()),
                ejectable: false,
            });
        }
    }
    items.push(FinderSidebarItem {
        section: FinderSidebarSection::Favorites,
        label: path_display_name(&home.to_string_lossy()),
        icon: FinderSidebarIcon::Home,
        location: FinderLocation::Path(home.to_string_lossy().into_owned()),
        ejectable: false,
    });
    items.push(FinderSidebarItem {
        section: FinderSidebarSection::Locations,
        label: String::from("This PC"),
        icon: FinderSidebarIcon::Computer,
        location: FinderLocation::ThisPc,
        ejectable: false,
    });
    for (root, label, removable) in drives {
        items.push(FinderSidebarItem {
            section: FinderSidebarSection::Locations,
            label: label.clone(),
            icon: if *removable {
                FinderSidebarIcon::Removable
            } else {
                FinderSidebarIcon::Drive
            },
            location: FinderLocation::Path(root.clone()),
            ejectable: *removable,
        });
    }
    items.push(FinderSidebarItem {
        section: FinderSidebarSection::Locations,
        label: String::from("Trash"),
        icon: FinderSidebarIcon::Trash,
        location: FinderLocation::Trash,
        ejectable: false,
    });
    for tag in FinderTag::ALL {
        items.push(FinderSidebarItem {
            section: FinderSidebarSection::Tags,
            label: String::from(tag.label()),
            icon: FinderSidebarIcon::Tag(tag),
            location: FinderLocation::Tag(tag),
            ejectable: false,
        });
    }
    items
}

#[must_use]
pub(super) fn entry_from_path(path: &Path) -> Option<FinderEntry> {
    let meta = fs::symlink_metadata(path).ok()?;
    let target_meta = if meta.file_type().is_symlink() {
        fs::metadata(path).unwrap_or(meta)
    } else {
        meta
    };
    let name = path.file_name().map_or_else(
        || path_display_name(&path.to_string_lossy()),
        |n| n.to_string_lossy().into_owned(),
    );
    let is_dir = target_meta.is_dir();
    let (kind, kind_label) = classify_entry_kind(&name, is_dir);
    let attrs = target_meta.file_attributes();
    let mut flags = 0u8;
    if (attrs & FILE_ATTRIBUTE_HIDDEN) != 0 || name.starts_with('.') {
        flags |= ENTRY_FLAG_HIDDEN;
    }
    if (attrs & FILE_ATTRIBUTE_READONLY) != 0 {
        flags |= ENTRY_FLAG_READ_ONLY;
    }
    if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        flags |= ENTRY_FLAG_SYMLINK;
    }
    let modified_unix = filetime_to_unix_secs(target_meta.last_write_time());
    let created_unix = filetime_to_unix_secs(target_meta.creation_time());
    Some(FinderEntry {
        path: path.to_string_lossy().into_owned(),
        name,
        kind,
        kind_label: String::from(kind_label),
        size_bytes: if is_dir { 0 } else { target_meta.file_size() },
        modified_unix,
        created_unix,
        modified_label: format_unix_timestamp(modified_unix),
        created_label: format_unix_timestamp(created_unix),
        flags,
        tag_mask: 0,
        drive_free_bytes: None,
        drive_total_bytes: None,
        trash_origin: None,
    })
}

#[must_use]
pub(super) fn list_directory(dir: &Path, recursive_query: Option<&str>) -> Vec<FinderEntry> {
    if let Some(query) = recursive_query {
        let needle = query.trim().to_ascii_lowercase();
        if !needle.is_empty() {
            let mut results = Vec::new();
            search_recursive(dir, &needle, 0, &mut results);
            return results;
        }
    }
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for item in read.flatten() {
        if let Some(entry) = entry_from_path(&item.path()) {
            entries.push(entry);
        }
    }
    entries
}

fn search_recursive(dir: &Path, needle: &str, depth: usize, out: &mut Vec<FinderEntry>) {
    if depth > MAX_RECURSIVE_DEPTH || out.len() >= MAX_RECURSIVE_RESULTS {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        if out.len() >= MAX_RECURSIVE_RESULTS {
            break;
        }
        let path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        if name.to_ascii_lowercase().contains(needle)
            && let Some(entry) = entry_from_path(&path)
        {
            out.push(entry);
        }
        if depth < MAX_RECURSIVE_DEPTH
            && item
                .file_type()
                .is_ok_and(|ft| ft.is_dir() && !ft.is_symlink())
            && !name.eq_ignore_ascii_case("node_modules")
            && !name.eq_ignore_ascii_case("target")
            && !name.eq_ignore_ascii_case(".git")
        {
            search_recursive(&path, needle, depth + 1, out);
        }
    }
}

#[must_use]
pub(super) fn list_recents() -> Vec<FinderEntry> {
    let home = user_home_dir();
    let mut entries = Vec::new();
    for sub in ["Desktop", "Documents", "Downloads", "Pictures"] {
        let dir = home.join(sub);
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            if item.file_type().is_ok_and(|ft| ft.is_file())
                && let Some(entry) = entry_from_path(&item.path())
                && !entry.is_hidden()
            {
                entries.push(entry);
            }
        }
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.modified_unix));
    entries.truncate(80);
    entries
}

#[must_use]
pub(super) fn list_trash_entries(drive_roots: &[String]) -> Vec<FinderEntry> {
    let mut out = Vec::new();
    for root in drive_roots {
        let bin = Path::new(root).join("$Recycle.Bin");
        let Ok(owners) = fs::read_dir(&bin) else {
            continue;
        };
        for owner in owners.flatten() {
            let owner_dir = owner.path();
            let Ok(files) = fs::read_dir(&owner_dir) else {
                continue;
            };
            for file in files.flatten() {
                let name = file.file_name().to_string_lossy().into_owned();
                if let Some(suffix) = name.strip_prefix("$I")
                    && let Some(entry) = parse_recycle_metadata(&owner_dir, &file.path(), suffix)
                {
                    out.push(entry);
                }
            }
        }
    }
    out.sort_by_key(|entry| std::cmp::Reverse(entry.modified_unix));
    out
}

fn parse_recycle_metadata(owner_dir: &Path, info_path: &Path, suffix: &str) -> Option<FinderEntry> {
    let data = fs::read(info_path).ok()?;
    if data.len() < 28 {
        return None;
    }
    let version = u64::from_le_bytes(data.get(0..8)?.try_into().ok()?);
    let size_bytes = u64::from_le_bytes(data.get(8..16)?.try_into().ok()?);
    let deleted_ft = u64::from_le_bytes(data.get(16..24)?.try_into().ok()?);
    let utf16_offset = if version >= 2 { 28 } else { 24 };
    let raw_bytes = data.get(utf16_offset..)?;
    let mut u16s = Vec::with_capacity(raw_bytes.len() / 2);
    for chunk in raw_bytes.chunks_exact(2) {
        let code = u16::from_le_bytes([chunk[0], chunk[1]]);
        if code == 0 {
            break;
        }
        u16s.push(code);
    }
    let original_path = String::from_utf16_lossy(&u16s);
    if original_path.is_empty() {
        return None;
    }
    let payload_path = owner_dir.join(format!("$R{suffix}"));
    if !payload_path.exists() {
        return None;
    }
    let is_dir = payload_path.is_dir();
    let display_name = path_display_name(&original_path);
    let (kind, kind_label) = classify_entry_kind(&display_name, is_dir);
    let modified_unix = filetime_to_unix_secs(deleted_ft);
    Some(FinderEntry {
        path: payload_path.to_string_lossy().into_owned(),
        name: display_name,
        kind,
        kind_label: String::from(kind_label),
        size_bytes,
        modified_unix,
        created_unix: modified_unix,
        modified_label: format_unix_timestamp(modified_unix),
        created_label: format_unix_timestamp(modified_unix),
        flags: 0,
        tag_mask: 0,
        drive_free_bytes: None,
        drive_total_bytes: None,
        trash_origin: Some(original_path),
    })
}

#[must_use]
pub(super) fn restore_trash_items(paths: &[String]) -> bool {
    let mut restored_any = false;
    for payload_raw in paths {
        let payload = Path::new(payload_raw);
        let Some(parent) = payload.parent() else {
            continue;
        };
        let Some(name) = payload.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(suffix) = name.strip_prefix("$R") else {
            continue;
        };
        let info_path = parent.join(format!("$I{suffix}"));
        let Some(entry) = parse_recycle_metadata(parent, &info_path, suffix) else {
            continue;
        };
        let Some(origin) = entry.trash_origin else {
            continue;
        };
        let target = PathBuf::from(&origin);
        if let Some(target_parent) = target.parent() {
            let _ = fs::create_dir_all(target_parent);
        }
        let final_target = unique_target_path(&target);
        if fs::rename(payload, &final_target).is_ok() {
            let _ = fs::remove_file(info_path);
            restored_any = true;
        }
    }
    restored_any
}

#[must_use]
pub(super) fn list_tagged_entries(bit: u8, tags: &HashMap<String, u8>) -> FinderListing {
    let mut entries = Vec::new();
    for (path_key, &mask) in tags {
        if (mask & bit) == 0 {
            continue;
        }
        let path = Path::new(path_key);
        if let Some(mut entry) = entry_from_path(path) {
            entry.tag_mask = mask;
            entries.push(entry);
        }
    }
    FinderListing {
        entries,
        free_space_label: String::new(),
    }
}

#[must_use]
pub(super) fn load_tags_file() -> HashMap<String, u8> {
    let Some(path) = data_dir().map(|dir| dir.join(TAGS_FILE)) else {
        return HashMap::new();
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for line in contents.lines() {
        if let Some((tags_raw, path_raw)) = line.split_once('\t') {
            let mask = parse_tag_mask(tags_raw);
            if mask != 0 && !path_raw.is_empty() {
                map.insert(path_raw.to_ascii_lowercase(), mask);
            }
        }
    }
    map
}

#[must_use]
pub(super) fn save_tags_file(tags: &HashMap<String, u8>) -> bool {
    let Some(dir) = data_dir() else {
        return false;
    };
    if fs::create_dir_all(&dir).is_err() {
        return false;
    }
    let mut lines: Vec<String> = tags
        .iter()
        .filter(|&(_, &mask)| mask != 0)
        .map(|(path, &mask)| format!("{}\t{path}", format_tag_mask(mask)))
        .collect();
    lines.sort();
    let payload = lines.join("\n");
    replace_file_atomically(&dir.join(TAGS_FILE), payload.as_bytes()).is_ok()
}

#[must_use]
pub(super) fn create_unique_folder(parent_dir: &str) -> Option<String> {
    let parent = Path::new(parent_dir);
    if !parent.is_dir() {
        return None;
    }
    let mut candidate = parent.join("untitled folder");
    let mut n = 2u32;
    while candidate.exists() {
        candidate = parent.join(format!("untitled folder {n}"));
        n += 1;
    }
    fs::create_dir(&candidate).ok()?;
    Some(candidate.to_string_lossy().into_owned())
}

#[must_use]
pub(super) fn rename_fs_path(path: &str, new_name: &str) -> Option<String> {
    let src = Path::new(path);
    let parent = src.parent()?;
    let dst = parent.join(new_name);
    if src == dst {
        return Some(String::from(path));
    }
    if dst.exists()
        && !src
            .to_string_lossy()
            .eq_ignore_ascii_case(&dst.to_string_lossy())
    {
        return None;
    }
    fs::rename(src, &dst).ok()?;
    Some(dst.to_string_lossy().into_owned())
}

#[must_use]
pub(super) fn duplicate_fs_paths(paths: &[String]) -> Vec<String> {
    let mut created = Vec::new();
    for raw in paths {
        let src = Path::new(raw);
        let Some(parent) = src.parent() else {
            continue;
        };
        let dst = unique_copy_target(parent, src);
        if copy_entry_recursive(src, &dst).is_ok() {
            created.push(dst.to_string_lossy().into_owned());
        }
    }
    created
}

#[must_use]
pub(super) fn paste_fs_paths(
    sources: &[String],
    dest_dir: &str,
    move_items: bool,
) -> Vec<(String, String)> {
    let dest_root = Path::new(dest_dir);
    if !dest_root.is_dir() {
        return Vec::new();
    }
    let mut done = Vec::new();
    for raw in sources {
        let src = Path::new(raw);
        if !src.exists() {
            continue;
        }
        let Some(name) = src.file_name() else {
            continue;
        };
        let initial_dst = dest_root.join(name);
        if move_items && src == initial_dst {
            continue;
        }
        let dst = if initial_dst.exists() {
            unique_copy_target(dest_root, src)
        } else {
            initial_dst
        };
        let ok = if move_items {
            fs::rename(src, &dst).is_ok()
                || (copy_entry_recursive(src, &dst).is_ok() && remove_entry_recursive(src).is_ok())
        } else {
            copy_entry_recursive(src, &dst).is_ok()
        };
        if ok {
            done.push((raw.clone(), dst.to_string_lossy().into_owned()));
        }
    }
    done
}

#[must_use]
pub(super) fn delete_fs_permanently(paths: &[String]) -> bool {
    let mut all_ok = true;
    for raw in paths {
        if remove_entry_recursive(Path::new(raw)).is_err() {
            all_ok = false;
        }
    }
    all_ok
}

fn copy_entry_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for item in fs::read_dir(src)? {
            let item = item?;
            let child_src = item.path();
            let child_dst = dst.join(item.file_name());
            copy_entry_recursive(&child_src, &child_dst)?;
        }
    } else {
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn remove_entry_recursive(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn unique_target_path(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }
    let parent = target.parent().unwrap_or_else(|| Path::new(r"C:\"));
    unique_copy_target(parent, target)
}

fn unique_copy_target(dest_dir: &Path, src: &Path) -> PathBuf {
    let name = src.file_name().map_or_else(
        || String::from("item"),
        |n| n.to_string_lossy().into_owned(),
    );
    let is_dir = src.is_dir();
    let (stem, ext) = if is_dir {
        (name.as_str(), "")
    } else if let Some((s, e)) = name.rsplit_once('.')
        && !s.is_empty()
    {
        (s, e)
    } else {
        (name.as_str(), "")
    };
    let mut n = 1u32;
    loop {
        let candidate_name = match (n, ext.is_empty()) {
            (1, true) => format!("{stem} copy"),
            (1, false) => format!("{stem} copy.{ext}"),
            (k, true) => format!("{stem} copy {k}"),
            (k, false) => format!("{stem} copy {k}.{ext}"),
        };
        let candidate = dest_dir.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

#[must_use]
pub(super) fn resolve_user_dir_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = if let Some(rest) = trimmed.strip_prefix('~') {
        let home = user_home_dir().to_string_lossy().into_owned();
        format!("{home}{rest}")
    } else {
        expand_env_vars(trimmed)
    };
    let path = PathBuf::from(&expanded);
    path.is_dir().then(|| path.to_string_lossy().into_owned())
}

fn expand_env_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('%') {
            let var = &after[..end];
            if let Ok(val) = std::env::var(var) {
                out.push_str(&val);
            } else {
                out.push('%');
                out.push_str(var);
                out.push('%');
            }
            rest = &after[end + 1..];
        } else {
            out.push('%');
            rest = after;
            break;
        }
    }
    out.push_str(rest);
    out
}

#[must_use]
pub(super) fn build_preview_base(entry: &FinderEntry) -> FinderPreviewData {
    let mut preview = FinderPreviewData {
        path: entry.path.clone(),
        name: entry.name.clone(),
        kind: if entry.is_dir() {
            FinderPreviewKind::Folder
        } else if entry.kind == FinderEntryKind::Image {
            FinderPreviewKind::Image
        } else if is_text_preview_extension(&entry.name) {
            FinderPreviewKind::Text
        } else {
            FinderPreviewKind::File
        },
        kind_label: entry.kind_label.clone(),
        size_label: entry.size_label(),
        modified_label: entry.modified_label.clone(),
        created_label: entry.created_label.clone(),
        item_count: None,
        dimensions_label: None,
        text_snippet: None,
        bitmap: None,
    };
    if entry.is_dir() {
        if let Ok(read) = fs::read_dir(&entry.path) {
            let mut count = 0usize;
            let mut total_bytes = 0u64;
            for item in read.flatten().take(2000) {
                count += 1;
                if let Ok(meta) = item.metadata()
                    && meta.is_file()
                {
                    total_bytes = total_bytes.saturating_add(meta.len());
                }
            }
            preview.item_count = Some(count);
            preview.size_label = format!("{count} items ({})", format_file_size(total_bytes));
        }
    } else if preview.kind == FinderPreviewKind::Text
        && let Ok(mut file) = fs::File::open(&entry.path)
    {
        let mut buf = vec![0u8; MAX_PREVIEW_TEXT_BYTES];
        if let Ok(read_len) = file.read(&mut buf) {
            buf.truncate(read_len);
            let text = String::from_utf8_lossy(&buf)
                .replace("\r\n", "\n")
                .replace('\t', "    ");
            preview.text_snippet = Some(text);
        }
    }
    preview
}
