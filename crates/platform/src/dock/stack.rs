use std::fs;
use std::path::PathBuf;

use otto_core::ShelfThumbnail;

use super::StackItem;

pub(super) fn downloads_directory() -> Option<PathBuf> {
    let base = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(base).join("Downloads"))
}

pub(super) fn is_active_download_temp_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".crdownload", ".part", ".partial", ".tmp", ".opdownload"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

#[must_use]
pub fn read_downloads_stack_items(max_items: usize) -> Vec<StackItem> {
    let Some(downloads_dir) = downloads_directory() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(downloads_dir) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || name.eq_ignore_ascii_case("desktop.ini")
            || is_active_download_temp_name(&name)
        {
            continue;
        }
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        items.push((modified, StackItem { path, name, is_dir }));
    }
    items.sort_by_key(|b| std::cmp::Reverse(b.0));
    items
        .into_iter()
        .take(max_items)
        .map(|(_, item)| item)
        .collect()
}

#[must_use]
pub fn read_downloads_stack_thumbnails(
    items: &[StackItem],
    max_px: u32,
) -> Vec<Option<ShelfThumbnail>> {
    items
        .iter()
        .map(|item| {
            let path = item.path.to_string_lossy();
            crate::notch::shelf::extract_shell_thumbnail(&path, max_px, false).1
        })
        .collect()
}

#[must_use]
pub fn read_downloads_active_progress() -> Option<f32> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant, SystemTime};

    const CACHE_TTL: Duration = Duration::from_secs(2);
    const ACTIVE_WINDOW: Duration = Duration::from_mins(30);

    type DownloadProgressCacheEntry = Option<(Instant, Option<f32>)>;
    static CACHE: OnceLock<Mutex<DownloadProgressCacheEntry>> = OnceLock::new();
    let lock = CACHE.get_or_init(|| Mutex::new(None));
    let now = Instant::now();
    if let Ok(guard) = lock.lock()
        && let Some((cached_at, value)) = *guard
        && now.saturating_duration_since(cached_at) < CACHE_TTL
    {
        return value;
    }

    let now_sys = SystemTime::now();
    let mut active = false;
    if let Some(dir) = downloads_directory()
        && let Ok(entries) = fs::read_dir(dir)
    {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_active_download_temp_name(&name) {
                continue;
            }
            let recent = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|m| now_sys.duration_since(m).ok())
                .is_some_and(|age| age <= ACTIVE_WINDOW);
            if recent {
                active = true;
                break;
            }
        }
    }
    let result = active.then_some(0.5);
    if let Ok(mut guard) = lock.lock() {
        *guard = Some((Instant::now(), result));
    }
    result
}
