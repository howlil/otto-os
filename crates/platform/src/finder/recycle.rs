use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use otto_core::{
    FinderBatchOutcome, FinderEntry, FinderItemOutcome, FinderItemStatus, classify_entry_kind,
    format_unix_timestamp, path_display_name,
};

use super::fs::filetime_to_unix_secs;
use super::mutate::move_file_no_replace;

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const MAX_RECYCLE_INFO_BYTES: u64 = 65_536;
const MAX_RECYCLE_PATH_CHARS: usize = 32_768;

fn is_valid_restore_path(raw: &str) -> bool {
    if raw.is_empty() || raw.starts_with(r"\\") {
        return false;
    }
    if raw.chars().any(char::is_control) {
        return false;
    }
    let bytes = raw.as_bytes();
    if bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || (bytes[2] != b'\\' && bytes[2] != b'/')
    {
        return false;
    }
    let path = Path::new(raw);
    !path.components().any(|c| matches!(c, Component::ParentDir))
}

#[must_use]
pub(super) fn parse_recycle_info_bytes(data: &[u8]) -> Option<(u64, u64, String)> {
    if data.len() < 28 || (data.len() as u64) > MAX_RECYCLE_INFO_BYTES {
        return None;
    }
    let version = u64::from_le_bytes(data.get(0..8)?.try_into().ok()?);
    let size_bytes = u64::from_le_bytes(data.get(8..16)?.try_into().ok()?);
    let deleted_ft = u64::from_le_bytes(data.get(16..24)?.try_into().ok()?);
    let u16s = match version {
        1 => {
            let raw_bytes = data.get(24..)?;
            let mut units = Vec::with_capacity(raw_bytes.len().min(520) / 2);
            for chunk in raw_bytes.chunks_exact(2) {
                let code = u16::from_le_bytes([chunk[0], chunk[1]]);
                if code == 0 {
                    break;
                }
                if units.len() >= 260 {
                    return None;
                }
                units.push(code);
            }
            units
        }
        2 => {
            let char_count =
                usize::try_from(u32::from_le_bytes(data.get(24..28)?.try_into().ok()?)).ok()?;
            if char_count == 0 || char_count > MAX_RECYCLE_PATH_CHARS {
                return None;
            }
            let byte_len = char_count.checked_mul(2)?;
            let end = 28usize.checked_add(byte_len)?;
            let raw_bytes = data.get(28..end)?;
            let mut units: Vec<u16> = raw_bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            if units.last() == Some(&0) {
                units.pop();
            }
            if units.contains(&0) {
                return None;
            }
            units
        }
        _ => return None,
    };
    let original_path = String::from_utf16(&u16s).ok()?;
    if !is_valid_restore_path(&original_path) {
        return None;
    }
    Some((size_bytes, deleted_ft, original_path))
}

fn parse_recycle_metadata(owner_dir: &Path, info_path: &Path, suffix: &str) -> Option<FinderEntry> {
    let meta = fs::symlink_metadata(info_path).ok()?;
    if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
        || meta.file_type().is_symlink()
        || !meta.is_file()
        || meta.len() > MAX_RECYCLE_INFO_BYTES
    {
        return None;
    }
    let data = fs::read(info_path).ok()?;
    let (size_bytes, deleted_ft, original_path) = parse_recycle_info_bytes(&data)?;
    let payload_path = owner_dir.join(format!("$R{suffix}"));
    let payload_meta = fs::symlink_metadata(&payload_path).ok()?;
    let is_dir = payload_meta.is_dir();
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

#[must_use]
pub(super) fn restore_trash_items(paths: &[String]) -> FinderBatchOutcome {
    let mut items = Vec::with_capacity(paths.len());
    for payload_raw in paths {
        let payload = Path::new(payload_raw);
        let Some(parent) = payload.parent() else {
            items.push(FinderItemOutcome::with_status(
                payload_raw,
                None,
                FinderItemStatus::Failed,
                "Invalid recycle payload path",
            ));
            continue;
        };
        let Some(name) = payload.file_name().and_then(|n| n.to_str()) else {
            items.push(FinderItemOutcome::with_status(
                payload_raw,
                None,
                FinderItemStatus::Failed,
                "Invalid recycle payload filename",
            ));
            continue;
        };
        let Some(suffix) = name.strip_prefix("$R") else {
            items.push(FinderItemOutcome::with_status(
                payload_raw,
                None,
                FinderItemStatus::Failed,
                "Not a recycle bin payload",
            ));
            continue;
        };
        let info_path = parent.join(format!("$I{suffix}"));
        let Some(entry) = parse_recycle_metadata(parent, &info_path, suffix) else {
            items.push(FinderItemOutcome::with_status(
                payload_raw,
                None,
                FinderItemStatus::Failed,
                "Invalid or missing recycle metadata",
            ));
            continue;
        };
        let Some(origin) = entry.trash_origin else {
            items.push(FinderItemOutcome::with_status(
                payload_raw,
                None,
                FinderItemStatus::Failed,
                "Missing original path",
            ));
            continue;
        };
        let target = PathBuf::from(&origin);
        if let Some(target_parent) = target.parent()
            && let Err(err) = fs::create_dir_all(target_parent)
        {
            items.push(FinderItemOutcome::with_status(
                payload_raw,
                Some(origin),
                FinderItemStatus::Failed,
                err.to_string(),
            ));
            continue;
        }
        match move_file_no_replace(payload, &target) {
            Ok(()) => {
                let _ = fs::remove_file(info_path);
                items.push(FinderItemOutcome::completed(payload_raw, Some(origin)));
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::AlreadyExists
                    || matches!(err.raw_os_error(), Some(80 | 183)) =>
            {
                items.push(FinderItemOutcome::with_status(
                    payload_raw,
                    Some(origin),
                    FinderItemStatus::Conflict,
                    "Destination already exists",
                ));
            }
            Err(err) => items.push(FinderItemOutcome::with_status(
                payload_raw,
                Some(origin),
                FinderItemStatus::Failed,
                err.to_string(),
            )),
        }
    }
    FinderBatchOutcome { items }
}
