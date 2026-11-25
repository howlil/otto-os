use std::cmp::Ordering;

use super::model::{
    FinderBreadcrumb, FinderEntry, FinderEntryKind, FinderLocation, FinderSortKey, FinderSortOrder,
};

#[must_use]
pub fn path_display_name(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return String::from("This PC");
    }
    if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
        return format!("Local Disk ({trimmed})");
    }
    trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|part| !part.is_empty())
        .map_or_else(|| trimmed.to_owned(), str::to_owned)
}

#[must_use]
pub fn parent_path_of(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if trimmed.len() <= 2 && trimmed.ends_with(':') {
        return None;
    }
    let idx = trimmed.rfind(['\\', '/'])?;
    let parent = &trimmed[..idx];
    if parent.len() == 2 && parent.ends_with(':') {
        Some(format!("{parent}\\"))
    } else if parent.is_empty() {
        None
    } else {
        Some(parent.to_owned())
    }
}

#[must_use]
pub fn breadcrumbs_for_location(location: &FinderLocation) -> Vec<FinderBreadcrumb> {
    match location {
        FinderLocation::Recents
        | FinderLocation::Trash
        | FinderLocation::ThisPc
        | FinderLocation::Tag(_) => vec![FinderBreadcrumb {
            label: location.title(),
            location: location.clone(),
        }],
        FinderLocation::Path(raw) => {
            let mut crumbs = Vec::new();
            let cleaned = raw.replace('/', "\\");
            let trimmed = cleaned.trim_end_matches('\\');
            let mut parts = trimmed.split('\\').filter(|part| !part.is_empty());
            let mut acc = String::new();
            if let Some(first) = parts.next() {
                acc.push_str(first);
                if first.ends_with(':') {
                    acc.push('\\');
                }
                crumbs.push(FinderBreadcrumb {
                    label: first.to_owned(),
                    location: FinderLocation::Path(acc.clone()),
                });
            }
            for segment in parts {
                if !acc.ends_with('\\') {
                    acc.push('\\');
                }
                acc.push_str(segment);
                crumbs.push(FinderBreadcrumb {
                    label: segment.to_owned(),
                    location: FinderLocation::Path(acc.clone()),
                });
            }
            crumbs
        }
    }
}

#[must_use]
pub fn classify_entry_kind(name: &str, is_dir: bool) -> (FinderEntryKind, &'static str) {
    if is_dir {
        return (FinderEntryKind::Folder, "Folder");
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" | "tif" | "tiff") => {
            (FinderEntryKind::Image, "Image")
        }
        Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma") => {
            (FinderEntryKind::Audio, "Audio")
        }
        Some("mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "m4v") => {
            (FinderEntryKind::Video, "Movie")
        }
        Some("pdf") => (FinderEntryKind::Document, "PDF Document"),
        Some("doc" | "docx" | "rtf" | "odt" | "pages") => (FinderEntryKind::Document, "Document"),
        Some("xls" | "xlsx" | "csv" | "ods") => (FinderEntryKind::Document, "Spreadsheet"),
        Some("ppt" | "pptx" | "key") => (FinderEntryKind::Document, "Presentation"),
        Some("txt" | "md" | "log") => (FinderEntryKind::Document, "Plain Text"),
        Some(
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "c" | "cpp" | "h" | "hpp" | "cs"
            | "java" | "json" | "toml" | "yaml" | "yml" | "xml" | "html" | "css" | "ps1" | "sh"
            | "bat" | "sql",
        ) => (FinderEntryKind::Code, "Source Code"),
        Some("zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "iso") => {
            (FinderEntryKind::Archive, "Archive")
        }
        Some("exe" | "msi" | "msix" | "lnk" | "appx" | "cmd") => {
            (FinderEntryKind::App, "Application")
        }
        _ => (FinderEntryKind::File, "Document"),
    }
}

#[must_use]
pub fn stem_selection_end(name: &str, is_dir: bool) -> usize {
    if is_dir || name.starts_with('.') && !name[1..].contains('.') {
        return name.chars().count();
    }
    match name.rfind('.') {
        Some(byte_idx) if byte_idx > 0 => name[..byte_idx].chars().count(),
        _ => name.chars().count(),
    }
}

#[must_use]
pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    const TB: f64 = 1_000_000_000_000.0;
    let value = bytes as f64;
    if value >= TB {
        format!("{:.2} TB", value / TB)
    } else if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

#[must_use]
pub fn format_unix_timestamp(unix_secs: i64) -> String {
    if unix_secs <= 0 {
        return String::from("--");
    }
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let (year, month, day) = civil_from_days(days);
    let month_name = match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    };
    format!("{month_name} {day}, {year} {hour:02}:{minute:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(m <= 2);
    (year, m as u32, d as u32)
}

#[must_use]
pub fn is_text_preview_extension(name: &str) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "txt"
            | "md"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "xml"
            | "html"
            | "css"
            | "scss"
            | "py"
            | "go"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "java"
            | "ps1"
            | "sh"
            | "bat"
            | "cmd"
            | "ini"
            | "cfg"
            | "conf"
            | "env"
            | "gitignore"
            | "log"
            | "csv"
            | "sql"
    )
}

pub fn sort_entries(
    entries: &mut [FinderEntry],
    key: FinderSortKey,
    order: FinderSortOrder,
    folders_on_top: bool,
) {
    entries.sort_by(|a, b| {
        if folders_on_top && a.is_dir() != b.is_dir() {
            return if a.is_dir() {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let primary = match key {
            FinderSortKey::Name => compare_natural(&a.name, &b.name),
            FinderSortKey::DateModified => a.modified_unix.cmp(&b.modified_unix),
            FinderSortKey::Size => a.size_bytes.cmp(&b.size_bytes),
            FinderSortKey::Kind => a
                .kind_label
                .to_ascii_lowercase()
                .cmp(&b.kind_label.to_ascii_lowercase()),
            FinderSortKey::Tags => a.tag_mask.cmp(&b.tag_mask),
        };
        let ordered = match order {
            FinderSortOrder::Ascending => primary,
            FinderSortOrder::Descending => primary.reverse(),
        };
        ordered.then_with(|| compare_natural(&a.name, &b.name))
    });
}

#[must_use]
pub fn compare_natural(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek(), bi.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&ac), Some(&bc)) if ac.is_ascii_digit() && bc.is_ascii_digit() => {
                let an = take_number(&mut ai);
                let bn = take_number(&mut bi);
                let cmp = an.cmp(&bn);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            (Some(_), Some(_)) => {
                let ac = ai.next().map_or('\0', |c| c.to_ascii_lowercase());
                let bc = bi.next().map_or('\0', |c| c.to_ascii_lowercase());
                let cmp = ac.cmp(&bc);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
        }
    }
}

fn take_number(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> u64 {
    let mut value = 0u64;
    while let Some(&ch) = iter.peek() {
        if let Some(digit) = ch.to_digit(10) {
            value = value.saturating_mul(10).saturating_add(u64::from(digit));
            iter.next();
        } else {
            break;
        }
    }
    value
}
