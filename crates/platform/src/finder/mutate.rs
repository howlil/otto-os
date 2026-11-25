use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use otto_core::{FinderBatchOutcome, FinderItemOutcome, FinderItemStatus};
use windows::Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW};
use windows::core::{HSTRING, PCWSTR};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const ERROR_NOT_SAME_DEVICE_CODE: i32 = 17;
const ERROR_FILE_EXISTS_CODE: i32 = 80;
const ERROR_ALREADY_EXISTS_CODE: i32 = 183;
const MAX_COPY_DEPTH: usize = 64;
const MAX_UNIQUE_ATTEMPTS: u32 = 10_000;

fn io_error_from_win32(err: &windows::core::Error) -> std::io::Error {
    let raw = err.code().0 as u32;
    if (raw & 0xFFFF_0000) == 0x8007_0000 {
        std::io::Error::from_raw_os_error((raw & 0xFFFF) as i32)
    } else {
        std::io::Error::other(err.to_string())
    }
}

fn is_already_exists_error(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::AlreadyExists
        || matches!(
            err.raw_os_error(),
            Some(ERROR_FILE_EXISTS_CODE | ERROR_ALREADY_EXISTS_CODE)
        )
}

fn is_cross_device_error(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::CrossesDevices
        || err.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE_CODE)
}

fn is_reparse_point(meta: &fs::Metadata) -> bool {
    (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 || meta.file_type().is_symlink()
}

pub(super) fn move_file_no_replace(src: &Path, dst: &Path) -> std::io::Result<()> {
    let src_wide = HSTRING::from(src.as_os_str());
    let dst_wide = HSTRING::from(dst.as_os_str());
    unsafe {
        MoveFileExW(
            PCWSTR(src_wide.as_ptr()),
            PCWSTR(dst_wide.as_ptr()),
            MOVE_FILE_FLAGS(0),
        )
    }
    .map_err(|err| io_error_from_win32(&err))
}

fn canonicalize_existing_or_ancestor(path: &Path) -> std::io::Result<PathBuf> {
    let mut cur = path.to_path_buf();
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(canon) = fs::canonicalize(&cur) {
            let mut result = canon;
            for part in suffix.into_iter().rev() {
                if part == "." {
                    continue;
                }
                if part == ".." {
                    if !result.pop() {
                        return Err(std::io::Error::new(
                            ErrorKind::InvalidInput,
                            "Path escapes root",
                        ));
                    }
                } else {
                    result.push(part);
                }
            }
            return Ok(result);
        }
        let Some(name) = cur.file_name().map(std::ffi::OsStr::to_os_string) else {
            return fs::canonicalize(path);
        };
        suffix.push(name);
        if !cur.pop() {
            return fs::canonicalize(path);
        }
    }
}

fn normalized_components(path: &Path) -> Vec<String> {
    let mut parts = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(prefix) => {
                let raw = prefix.as_os_str().to_string_lossy();
                let stripped = raw
                    .strip_prefix(r"\\?\UNC\")
                    .map_or_else(|| raw.strip_prefix(r"\\?\").unwrap_or(&raw), |u| u);
                parts.push(stripped.to_ascii_lowercase());
            }
            Component::RootDir => parts.push(String::from("\\")),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = parts.pop();
            }
            Component::Normal(seg) => parts.push(seg.to_string_lossy().to_ascii_lowercase()),
        }
    }
    parts
}

pub(super) fn is_same_or_descendant(src: &Path, dst: &Path) -> std::io::Result<bool> {
    let src_canon = canonicalize_existing_or_ancestor(src)?;
    let dst_canon = canonicalize_existing_or_ancestor(dst)?;
    let src_parts = normalized_components(&src_canon);
    let dst_parts = normalized_components(&dst_canon);
    Ok(!src_parts.is_empty()
        && dst_parts.len() >= src_parts.len()
        && dst_parts[..src_parts.len()] == src_parts[..])
}

pub(super) fn is_same_canonical_path(a: &Path, b: &Path) -> std::io::Result<bool> {
    let a_canon = canonicalize_existing_or_ancestor(a)?;
    let b_canon = canonicalize_existing_or_ancestor(b)?;
    Ok(normalized_components(&a_canon) == normalized_components(&b_canon))
}

pub(super) fn remove_entry_safe(path: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if is_reparse_point(&meta) {
        if meta.is_dir() {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        }
    } else if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn copy_file_exclusive(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut reader = fs::File::open(src)?;
    let mut writer = OpenOptions::new().write(true).create_new(true).open(dst)?;
    if let Err(err) = std::io::copy(&mut reader, &mut writer) {
        drop(writer);
        let _ = fs::remove_file(dst);
        return Err(err);
    }
    Ok(())
}

fn copy_dir_exclusive(src: &Path, dst: &Path, depth: usize) -> std::io::Result<()> {
    if depth > MAX_COPY_DEPTH {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Directory nesting exceeds maximum copy depth",
        ));
    }
    let meta = fs::symlink_metadata(src)?;
    if is_reparse_point(&meta) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Refusing to traverse reparse point",
        ));
    }
    fs::create_dir(dst)?;
    let result = (|| -> std::io::Result<()> {
        for item in fs::read_dir(src)? {
            let item = item?;
            let child_src = item.path();
            let child_dst = dst.join(item.file_name());
            copy_entry_exclusive(&child_src, &child_dst, depth + 1)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_entry_safe(dst);
    }
    result
}

fn copy_entry_exclusive(src: &Path, dst: &Path, depth: usize) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if is_reparse_point(&meta) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Refusing to follow reparse point",
        ));
    }
    if meta.is_dir() {
        copy_dir_exclusive(src, dst, depth)
    } else {
        copy_file_exclusive(src, dst)
    }
}

fn copy_candidate_name(name: &str, is_dir: bool, attempt: u32) -> String {
    if attempt == 0 {
        return String::from(name);
    }
    let (stem, ext) = if is_dir {
        (name, "")
    } else if let Some((s, e)) = name.rsplit_once('.')
        && !s.is_empty()
    {
        (s, e)
    } else {
        (name, "")
    };
    match (attempt, ext.is_empty()) {
        (1, true) => format!("{stem} copy"),
        (1, false) => format!("{stem} copy.{ext}"),
        (k, true) => format!("{stem} copy {k}"),
        (k, false) => format!("{stem} copy {k}.{ext}"),
    }
}

fn reserve_and_copy(
    src: &Path,
    dest_root: &Path,
    prefer_base_name: bool,
) -> Result<PathBuf, (FinderItemStatus, String)> {
    let meta = fs::symlink_metadata(src).map_err(|e| (FinderItemStatus::Failed, e.to_string()))?;
    if is_reparse_point(&meta) {
        return Err((
            FinderItemStatus::Conflict,
            String::from("Refusing to copy link or junction"),
        ));
    }
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| {
            (
                FinderItemStatus::Failed,
                String::from("Invalid source name"),
            )
        })?;
    let start_attempt = u32::from(!prefer_base_name);
    for attempt in start_attempt..=MAX_UNIQUE_ATTEMPTS {
        let candidate_name = copy_candidate_name(&name, meta.is_dir(), attempt);
        let candidate = dest_root.join(candidate_name);
        if is_same_or_descendant(src, &candidate).unwrap_or(true) {
            return Err((
                FinderItemStatus::Conflict,
                String::from("Cannot copy folder into itself"),
            ));
        }
        match copy_entry_exclusive(src, &candidate, 0) {
            Ok(()) => return Ok(candidate),
            Err(err) if is_already_exists_error(&err) => {}
            Err(err) => return Err((FinderItemStatus::Failed, err.to_string())),
        }
    }
    Err((
        FinderItemStatus::Conflict,
        String::from("Too many conflicting destination names"),
    ))
}

fn move_with_unique_target(src: &Path, dest_root: &Path, is_dir: bool) -> FinderItemOutcome {
    let raw = src.to_string_lossy().into_owned();
    let Some(name) = src.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return FinderItemOutcome::with_status(raw, None, FinderItemStatus::Failed, "Invalid name");
    };
    let base_dst = dest_root.join(&name);
    if is_same_canonical_path(src, &base_dst).unwrap_or(false) {
        return FinderItemOutcome::with_status(
            raw,
            None,
            FinderItemStatus::Skipped,
            "Source is already in destination",
        );
    }
    for attempt in 0..=MAX_UNIQUE_ATTEMPTS {
        let candidate = dest_root.join(copy_candidate_name(&name, is_dir, attempt));
        if is_same_or_descendant(src, &candidate).unwrap_or(true) {
            return FinderItemOutcome::with_status(
                raw,
                None,
                FinderItemStatus::Conflict,
                "Cannot move folder into itself",
            );
        }
        match move_file_no_replace(src, &candidate) {
            Ok(()) => {
                return FinderItemOutcome::completed(
                    raw,
                    Some(candidate.to_string_lossy().into_owned()),
                );
            }
            Err(err) if is_already_exists_error(&err) => {}
            Err(err) if is_cross_device_error(&err) => {
                return cross_device_move(src, &raw, dest_root);
            }
            Err(err) => {
                return FinderItemOutcome::with_status(
                    raw,
                    None,
                    FinderItemStatus::Failed,
                    err.to_string(),
                );
            }
        }
    }
    FinderItemOutcome::with_status(
        raw,
        None,
        FinderItemStatus::Conflict,
        "Too many conflicting destination names",
    )
}

fn cross_device_move(src: &Path, raw: &str, dest_root: &Path) -> FinderItemOutcome {
    match reserve_and_copy(src, dest_root, true) {
        Ok(dst) => {
            let dst_str = dst.to_string_lossy().into_owned();
            if let Err(err) = remove_entry_safe(src) {
                FinderItemOutcome::with_status(
                    raw,
                    Some(dst_str),
                    FinderItemStatus::Partial,
                    format!("Copied but could not remove source: {err}"),
                )
            } else {
                FinderItemOutcome::completed(raw, Some(dst_str))
            }
        }
        Err((status, msg)) => FinderItemOutcome::with_status(raw, None, status, msg),
    }
}

#[must_use]
pub(super) fn create_unique_folder(parent_dir: &str) -> Option<String> {
    let parent = Path::new(parent_dir);
    if !parent.is_dir() {
        return None;
    }
    for n in 1..=MAX_UNIQUE_ATTEMPTS {
        let name = if n == 1 {
            String::from("untitled folder")
        } else {
            format!("untitled folder {n}")
        };
        let candidate = parent.join(name);
        match fs::create_dir(&candidate) {
            Ok(()) => return Some(candidate.to_string_lossy().into_owned()),
            Err(err) if is_already_exists_error(&err) => {}
            Err(_) => return None,
        }
    }
    None
}

#[must_use]
pub(super) fn rename_fs_path(path: &str, new_name: &str) -> Option<String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
    {
        return None;
    }
    let src = Path::new(path);
    let parent = src.parent()?;
    let dst = parent.join(trimmed);
    if src == dst {
        return Some(String::from(path));
    }
    let case_only = src
        .to_string_lossy()
        .eq_ignore_ascii_case(&dst.to_string_lossy());
    if case_only {
        fs::rename(src, &dst).ok()?;
    } else {
        move_file_no_replace(src, &dst).ok()?;
    }
    Some(dst.to_string_lossy().into_owned())
}

#[must_use]
pub(super) fn duplicate_fs_paths(paths: &[String]) -> FinderBatchOutcome {
    let mut items = Vec::with_capacity(paths.len());
    for raw in paths {
        let src = Path::new(raw);
        let Some(parent) = src.parent() else {
            items.push(FinderItemOutcome::with_status(
                raw,
                None,
                FinderItemStatus::Failed,
                "Missing parent directory",
            ));
            continue;
        };
        match reserve_and_copy(src, parent, false) {
            Ok(dst) => items.push(FinderItemOutcome::completed(
                raw,
                Some(dst.to_string_lossy().into_owned()),
            )),
            Err((status, msg)) => {
                items.push(FinderItemOutcome::with_status(raw, None, status, msg));
            }
        }
    }
    FinderBatchOutcome { items }
}

#[must_use]
pub(super) fn paste_fs_paths(
    sources: &[String],
    dest_dir: &str,
    move_items: bool,
) -> FinderBatchOutcome {
    let dest_root = Path::new(dest_dir);
    if !dest_root.is_dir() {
        return FinderBatchOutcome {
            items: sources
                .iter()
                .map(|s| {
                    FinderItemOutcome::with_status(
                        s,
                        None,
                        FinderItemStatus::Failed,
                        "Destination is not a directory",
                    )
                })
                .collect(),
        };
    }
    let mut items = Vec::with_capacity(sources.len());
    for raw in sources {
        let src = Path::new(raw);
        let Ok(meta) = fs::symlink_metadata(src) else {
            items.push(FinderItemOutcome::with_status(
                raw,
                None,
                FinderItemStatus::Failed,
                "Source item not found",
            ));
            continue;
        };
        if is_same_or_descendant(src, dest_root).unwrap_or(true) {
            items.push(FinderItemOutcome::with_status(
                raw,
                None,
                FinderItemStatus::Conflict,
                "Cannot copy or move folder into itself",
            ));
            continue;
        }
        if move_items {
            items.push(move_with_unique_target(src, dest_root, meta.is_dir()));
        } else {
            match reserve_and_copy(src, dest_root, true) {
                Ok(dst) => items.push(FinderItemOutcome::completed(
                    raw,
                    Some(dst.to_string_lossy().into_owned()),
                )),
                Err((status, msg)) => {
                    items.push(FinderItemOutcome::with_status(raw, None, status, msg));
                }
            }
        }
    }
    FinderBatchOutcome { items }
}

#[must_use]
pub(super) fn move_exact_paths(moves: &[(String, String)]) -> FinderBatchOutcome {
    let mut items = Vec::with_capacity(moves.len());
    for (src_raw, dst_raw) in moves {
        let src = Path::new(src_raw);
        let dst = Path::new(dst_raw);
        if fs::symlink_metadata(src).is_err() {
            items.push(FinderItemOutcome::with_status(
                src_raw,
                Some(dst_raw.clone()),
                FinderItemStatus::Failed,
                "Source no longer exists",
            ));
            continue;
        }
        let Some(dst_parent) = dst.parent() else {
            items.push(FinderItemOutcome::with_status(
                src_raw,
                Some(dst_raw.clone()),
                FinderItemStatus::Failed,
                "Missing destination parent",
            ));
            continue;
        };
        if !dst_parent.is_dir() {
            items.push(FinderItemOutcome::with_status(
                src_raw,
                Some(dst_raw.clone()),
                FinderItemStatus::Failed,
                "Destination parent folder not found",
            ));
            continue;
        }
        if is_same_or_descendant(src, dst).unwrap_or(true) {
            items.push(FinderItemOutcome::with_status(
                src_raw,
                Some(dst_raw.clone()),
                FinderItemStatus::Conflict,
                "Cannot move folder into itself",
            ));
            continue;
        }
        match move_file_no_replace(src, dst) {
            Ok(()) => items.push(FinderItemOutcome::completed(src_raw, Some(dst_raw.clone()))),
            Err(err) if is_already_exists_error(&err) => {
                items.push(FinderItemOutcome::with_status(
                    src_raw,
                    Some(dst_raw.clone()),
                    FinderItemStatus::Conflict,
                    "Destination already exists",
                ));
            }
            Err(err) if is_cross_device_error(&err) => match copy_entry_exclusive(src, dst, 0) {
                Ok(()) => {
                    if let Err(rm_err) = remove_entry_safe(src) {
                        items.push(FinderItemOutcome::with_status(
                            src_raw,
                            Some(dst_raw.clone()),
                            FinderItemStatus::Partial,
                            format!("Restored but could not remove source: {rm_err}"),
                        ));
                    } else {
                        items.push(FinderItemOutcome::completed(src_raw, Some(dst_raw.clone())));
                    }
                }
                Err(cp_err) if is_already_exists_error(&cp_err) => {
                    items.push(FinderItemOutcome::with_status(
                        src_raw,
                        Some(dst_raw.clone()),
                        FinderItemStatus::Conflict,
                        "Destination already exists",
                    ));
                }
                Err(cp_err) => items.push(FinderItemOutcome::with_status(
                    src_raw,
                    Some(dst_raw.clone()),
                    FinderItemStatus::Failed,
                    cp_err.to_string(),
                )),
            },
            Err(err) => items.push(FinderItemOutcome::with_status(
                src_raw,
                Some(dst_raw.clone()),
                FinderItemStatus::Failed,
                err.to_string(),
            )),
        }
    }
    FinderBatchOutcome { items }
}

#[must_use]
pub(super) fn delete_fs_permanently(paths: &[String]) -> FinderBatchOutcome {
    let mut items = Vec::with_capacity(paths.len());
    for raw in paths {
        match remove_entry_safe(Path::new(raw)) {
            Ok(()) => items.push(FinderItemOutcome::completed(raw, None)),
            Err(err) => items.push(FinderItemOutcome::with_status(
                raw,
                None,
                FinderItemStatus::Failed,
                err.to_string(),
            )),
        }
    }
    FinderBatchOutcome { items }
}
