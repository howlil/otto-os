pub const MAX_SHELF_ITEMS: usize = 12;
pub const SHELF_PAGE_SIZE: usize = 6;
pub const SHELF_HOVER_LEAVE_MS: u64 = 220;
pub const SHELF_THUMBNAIL_MAX_PX: u32 = 128;
pub const MAX_SHELF_TEXT_BYTES: usize = 65_536;
pub const MAX_SHELF_PATH_LEN: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShelfItemId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfPayload {
    File(String),
    Text(String),
    Url(String),
}

impl ShelfPayload {
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::File(a), Self::File(b)) | (Self::Url(a), Self::Url(b)) => {
                a.eq_ignore_ascii_case(b)
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn display_text(&self) -> &str {
        match self {
            Self::File(path) => path.as_str(),
            Self::Text(text) => text.as_str(),
            Self::Url(url) => url.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShelfKindTag {
    Folder,
    Image,
    Audio,
    Video,
    Pdf,
    Archive,
    Code,
    Document,
    Url,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfThumbnail {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfItem {
    pub id: ShelfItemId,
    pub label: String,
    pub subtitle: String,
    pub payload: ShelfPayload,
    pub kind: ShelfKindTag,
    pub thumbnail: Option<ShelfThumbnail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShelfState {
    pub items: Vec<ShelfItem>,
    pub page: usize,
    pub drag_hover: bool,
    pub next_id: u64,
}

impl ShelfState {
    pub fn push_payload(
        &mut self,
        payload: ShelfPayload,
        is_dir: bool,
        thumbnail: Option<ShelfThumbnail>,
    ) -> Option<ShelfItemId> {
        let kind = classify_payload(&payload, is_dir);
        let (label, subtitle) = describe_payload(&payload, kind);
        if label.is_empty() {
            return None;
        }
        if let Some(existing_index) = self
            .items
            .iter()
            .position(|item| item.payload.matches(&payload))
        {
            let mut item = self.items.remove(existing_index);
            item.kind = kind;
            item.label = label;
            item.subtitle = subtitle;
            if thumbnail.is_some() {
                item.thumbnail = thumbnail;
            }
            let id = item.id;
            self.items.insert(0, item);
            self.page = 0;
            return Some(id);
        }
        self.next_id = self.next_id.saturating_add(1).max(1);
        let id = ShelfItemId(self.next_id);
        self.items.insert(
            0,
            ShelfItem {
                id,
                label,
                subtitle,
                payload,
                kind,
                thumbnail,
            },
        );
        self.items.truncate(MAX_SHELF_ITEMS);
        self.page = 0;
        Some(id)
    }

    pub fn remove(&mut self, id: ShelfItemId) -> bool {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        self.items.remove(index);
        self.clamp_page();
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.items.clear();
        self.page = 0;
        true
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.items.len().div_ceil(SHELF_PAGE_SIZE).max(1)
    }

    #[must_use]
    pub fn visible_items(&self) -> &[ShelfItem] {
        let page = self.page.min(self.page_count().saturating_sub(1));
        let start = page.saturating_mul(SHELF_PAGE_SIZE).min(self.items.len());
        let end = (start + SHELF_PAGE_SIZE).min(self.items.len());
        &self.items[start..end]
    }

    #[must_use]
    pub fn visible_item_at(&self, slot: u8) -> Option<&ShelfItem> {
        self.visible_items().get(usize::from(slot))
    }

    pub fn step_page(&mut self, delta: i32) -> bool {
        let pages = self.page_count();
        if pages <= 1 || delta == 0 {
            return false;
        }
        let next = (self.page as i32 + delta).rem_euclid(pages as i32) as usize;
        if next == self.page {
            return false;
        }
        self.page = next;
        true
    }

    pub fn set_drag_hover(&mut self, active: bool) -> bool {
        if self.drag_hover == active {
            return false;
        }
        self.drag_hover = active;
        true
    }

    fn clamp_page(&mut self) {
        let max_page = self.page_count().saturating_sub(1);
        self.page = self.page.min(max_page);
    }
}

#[must_use]
pub fn classify_payload(payload: &ShelfPayload, is_dir: bool) -> ShelfKindTag {
    match payload {
        ShelfPayload::Url(_) => ShelfKindTag::Url,
        ShelfPayload::Text(_) => ShelfKindTag::Text,
        ShelfPayload::File(path) => {
            if is_dir {
                return ShelfKindTag::Folder;
            }
            let ext = file_extension_lower(path);
            match ext.as_str() {
                "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "ico" | "tif" | "tiff" => {
                    ShelfKindTag::Image
                }
                "mp3" | "flac" | "wav" | "m4a" | "aac" | "ogg" | "opus" => ShelfKindTag::Audio,
                "mp4" | "mkv" | "mov" | "webm" | "avi" | "wmv" => ShelfKindTag::Video,
                "pdf" => ShelfKindTag::Pdf,
                "zip" | "7z" | "rar" | "tar" | "gz" | "xz" => ShelfKindTag::Archive,
                "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "json" | "toml" | "yaml" | "yml"
                | "html" | "css" | "c" | "cpp" | "h" => ShelfKindTag::Code,
                _ => ShelfKindTag::Document,
            }
        }
    }
}

fn file_extension_lower(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => String::new(),
    }
}

fn describe_payload(payload: &ShelfPayload, kind: ShelfKindTag) -> (String, String) {
    match payload {
        ShelfPayload::File(path) => {
            let trimmed = path.trim_end_matches(['\\', '/']);
            let name = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed).trim();
            let sub = if kind == ShelfKindTag::Folder {
                String::from("Folder")
            } else {
                let ext = file_extension_lower(path);
                if ext.is_empty() {
                    String::from("File")
                } else {
                    ext.to_ascii_uppercase()
                }
            };
            (String::from(name), sub)
        }
        ShelfPayload::Url(url) => {
            let trimmed = url.trim();
            let host = trimmed
                .split_once("://")
                .map_or(trimmed, |(_, rest)| rest)
                .split('/')
                .next()
                .unwrap_or(trimmed);
            (String::from(host), String::from("Link"))
        }
        ShelfPayload::Text(text) => {
            let line = text
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
                .trim();
            let preview: String = line.chars().take(40).collect();
            (preview, format!("{} chars", text.chars().count()))
        }
    }
}

#[must_use]
pub fn is_openable_shelf_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_SHELF_TEXT_BYTES
        || trimmed
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let has_http_scheme = trimmed
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || trimmed
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"));
    if !has_http_scheme {
        return false;
    }
    let Some((_, after_scheme)) = trimmed.split_once("://") else {
        return false;
    };
    let host = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && !host.starts_with('.') && !host.starts_with(':')
}

#[must_use]
pub fn is_valid_shelf_file_path(path: &str) -> bool {
    let trimmed = path.trim();
    !trimmed.is_empty() && trimmed.len() <= MAX_SHELF_PATH_LEN && !path.contains('\0')
}

#[must_use]
pub fn is_unc_shelf_path(path: &str) -> bool {
    let trimmed = path.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 {
        return false;
    }
    let is_slash = |b: u8| b == b'\\' || b == b'/';
    if !(is_slash(bytes[0]) && is_slash(bytes[1])) {
        return false;
    }
    if bytes.len() >= 4 && matches!(bytes[2], b'?' | b'.') && is_slash(bytes[3]) {
        let rest = &bytes[4..];
        return rest.len() >= 4 && rest[..3].eq_ignore_ascii_case(b"unc") && is_slash(rest[3]);
    }
    true
}

#[must_use]
pub fn decode_dropped_text(raw: &str) -> Option<ShelfPayload> {
    let clean = raw.replace('\0', "");
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_openable_shelf_url(trimmed) {
        return Some(ShelfPayload::Url(String::from(trimmed)));
    }
    let bounded = truncate_utf8_bytes(trimmed, MAX_SHELF_TEXT_BYTES).trim_end();
    if bounded.is_empty() {
        return None;
    }
    Some(ShelfPayload::Text(String::from(bounded)))
}

pub(crate) fn truncate_utf8_bytes(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SHELF_ITEMS, MAX_SHELF_PATH_LEN, MAX_SHELF_TEXT_BYTES, SHELF_PAGE_SIZE, ShelfKindTag,
        ShelfPayload, ShelfState, classify_payload, decode_dropped_text, is_openable_shelf_url,
        is_unc_shelf_path, is_valid_shelf_file_path,
    };

    #[test]
    fn shelf_deduplicates_moves_recent_to_front_and_caps_at_twelve_items() {
        let mut shelf = ShelfState::default();
        for index in 0..14 {
            let path = format!("C:\\Temp\\file_{index}.txt");
            assert!(
                shelf
                    .push_payload(ShelfPayload::File(path), false, None)
                    .is_some()
            );
        }
        assert_eq!(shelf.items.len(), MAX_SHELF_ITEMS);
        assert_eq!(shelf.page_count(), 2);
        assert_eq!(shelf.visible_items().len(), SHELF_PAGE_SIZE);
        assert_eq!(shelf.items[0].label, "file_13.txt");
        assert_eq!(shelf.items[11].label, "file_2.txt");

        let readded_id = shelf
            .push_payload(
                ShelfPayload::File(String::from("c:\\temp\\file_5.txt")),
                false,
                None,
            )
            .unwrap();
        assert_eq!(shelf.items.len(), MAX_SHELF_ITEMS);
        assert_eq!(shelf.items[0].id, readded_id);
        assert_eq!(shelf.items[0].label, "file_5.txt");
    }

    #[test]
    fn paging_and_removal_clamp_safely() {
        let mut shelf = ShelfState::default();
        for index in 0..7 {
            let _ = shelf.push_payload(ShelfPayload::Text(format!("snippet {index}")), false, None);
        }
        assert_eq!(shelf.page_count(), 2);
        assert!(shelf.step_page(1));
        assert_eq!(shelf.page, 1);
        assert_eq!(shelf.visible_items().len(), 1);
        let id_on_page_two = shelf.visible_item_at(0).unwrap().id;
        assert!(shelf.remove(id_on_page_two));
        assert_eq!(shelf.page_count(), 1);
        assert_eq!(shelf.page, 0);
        assert!(shelf.clear());
        assert!(!shelf.clear());
    }

    #[test]
    fn classify_payload_identifies_folders_media_code_urls_and_text() {
        assert_eq!(
            classify_payload(&ShelfPayload::File(String::from("C:\\Projects")), true),
            ShelfKindTag::Folder
        );
        assert_eq!(
            classify_payload(&ShelfPayload::File(String::from("C:\\photo.PNG")), false),
            ShelfKindTag::Image
        );
        assert_eq!(
            classify_payload(&ShelfPayload::File(String::from("C:\\main.rs")), false),
            ShelfKindTag::Code
        );
        assert_eq!(
            classify_payload(
                &ShelfPayload::Url(String::from("https://otto.dev/docs")),
                false
            ),
            ShelfKindTag::Url
        );
        assert_eq!(
            classify_payload(&ShelfPayload::Text(String::from("hello world")), false),
            ShelfKindTag::Text
        );
    }

    #[test]
    fn decode_dropped_text_enforces_http_allowlist_and_utf8_size_cap() {
        assert_eq!(decode_dropped_text("   \n\t\0 "), None);
        assert_eq!(
            decode_dropped_text("  HTTPS://otto.dev/docs?a=1 "),
            Some(ShelfPayload::Url(String::from("HTTPS://otto.dev/docs?a=1")))
        );
        for blocked in [
            "javascript:alert(1)",
            "file:///C:/Windows/System32/cmd.exe",
            "ms-msdt:id",
            "vbscript:msgbox(1)",
            "https://",
            "https://otto.dev/a b",
        ] {
            assert!(!is_openable_shelf_url(blocked));
            assert_eq!(
                decode_dropped_text(blocked),
                Some(ShelfPayload::Text(String::from(blocked)))
            );
        }

        let oversized = "é".repeat(40_000);
        let decoded = decode_dropped_text(&oversized).expect("non-empty truncated snippet");
        let ShelfPayload::Text(text) = decoded else {
            panic!("oversized snippet must decode as text");
        };
        assert!(text.len() <= MAX_SHELF_TEXT_BYTES);
        assert!(text.len() >= MAX_SHELF_TEXT_BYTES - 2);
    }

    #[test]
    fn valid_shelf_file_path_rejects_empty_embedded_null_and_oversized() {
        assert!(is_valid_shelf_file_path(r"C:\Temp\notes.txt"));
        assert!(!is_valid_shelf_file_path("   "));
        assert!(!is_valid_shelf_file_path("C:\\Temp\\bad\0.txt"));
        let too_long = "a".repeat(MAX_SHELF_PATH_LEN + 1);
        assert!(!is_valid_shelf_file_path(&too_long));
    }

    #[test]
    fn unc_path_detector_matches_network_paths_and_skips_local_extended() {
        assert!(is_unc_shelf_path(r"\\fileserver\share\notes.pdf"));
        assert!(is_unc_shelf_path("//fileserver/share/notes.pdf"));
        assert!(is_unc_shelf_path(r"  \\?\UNC\fileserver\share\notes.pdf"));
        assert!(is_unc_shelf_path(r"\\?\unc\fileserver\share\notes.pdf"));
        assert!(is_unc_shelf_path(r"\\.\UNC\fileserver\share\notes.pdf"));
        assert!(!is_unc_shelf_path(r"C:\Temp\notes.txt"));
        assert!(!is_unc_shelf_path(r"\\?\C:\Temp\notes.txt"));
        assert!(!is_unc_shelf_path(r"\\?\🔥"));
        assert!(!is_unc_shelf_path(r"\\?\ébc\file"));
        assert!(is_unc_shelf_path(r"\\?\unc\🔥\share"));
        assert!(!is_unc_shelf_path(""));
    }
}
