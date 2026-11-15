use crate::shelf::{is_openable_shelf_url, is_valid_shelf_file_path, truncate_utf8_bytes};

pub const MAX_CLIPBOARD_ITEMS: usize = 12;
pub const CLIPBOARD_PAGE_SIZE: usize = 4;
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_CLIPBOARD_FILES: usize = 6;
const MAX_PREVIEW_CHARS: usize = 52;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClipboardItemId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardKind {
    Text,
    Link,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Text(String),
    Link(String),
    Files(Vec<String>),
}

impl ClipboardPayload {
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Link(a), Self::Link(b)) => a.eq_ignore_ascii_case(b),
            (Self::Files(a), Self::Files(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(left, right)| left.eq_ignore_ascii_case(right))
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn normalize(self) -> Option<Self> {
        match self {
            Self::Text(raw) => {
                let cleaned = raw.replace('\0', "");
                let trimmed = cleaned.trim();
                if trimmed.is_empty() {
                    return None;
                }
                if is_openable_shelf_url(trimmed) {
                    let link = truncate_utf8_bytes(trimmed, MAX_CLIPBOARD_TEXT_BYTES).trim_end();
                    return (!link.is_empty() && is_openable_shelf_url(link))
                        .then(|| Self::Link(String::from(link)));
                }
                let bounded = truncate_utf8_bytes(&cleaned, MAX_CLIPBOARD_TEXT_BYTES);
                if bounded.trim().is_empty() {
                    return None;
                }
                Some(Self::Text(String::from(bounded)))
            }
            Self::Link(raw) => {
                let cleaned = raw.replace('\0', "");
                let trimmed = truncate_utf8_bytes(cleaned.trim(), MAX_CLIPBOARD_TEXT_BYTES).trim();
                (is_openable_shelf_url(trimmed)).then(|| Self::Link(String::from(trimmed)))
            }
            Self::Files(paths) => {
                let valid: Vec<String> = paths
                    .into_iter()
                    .filter(|path| is_valid_shelf_file_path(path))
                    .take(MAX_CLIPBOARD_FILES)
                    .collect();
                (!valid.is_empty()).then_some(Self::Files(valid))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardItem {
    pub id: ClipboardItemId,
    pub kind: ClipboardKind,
    pub preview: String,
    pub meta: String,
    pub payload: ClipboardPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClipboardLayoutInput {
    pub visible_count: u8,
    pub has_pager: bool,
    pub has_items: bool,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipboardState {
    pub items: Vec<ClipboardItem>,
    pub page: usize,
    pub paused: bool,
    pub next_id: u64,
    pub copied_feedback_id: Option<ClipboardItemId>,
}

impl ClipboardState {
    pub fn push_payload(&mut self, payload: ClipboardPayload) -> Option<ClipboardItemId> {
        if self.paused {
            return None;
        }
        let normalized = payload.normalize()?;
        let (kind, preview, meta) = summarize_payload(&normalized);
        if preview.is_empty() {
            return None;
        }
        if let Some(existing_idx) = self
            .items
            .iter()
            .position(|item| item.payload.matches(&normalized))
        {
            let mut existing = self.items.remove(existing_idx);
            existing.kind = kind;
            existing.preview = preview;
            existing.meta = meta;
            existing.payload = normalized;
            let id = existing.id;
            self.items.insert(0, existing);
            self.page = 0;
            return Some(id);
        }
        self.next_id = self.next_id.saturating_add(1);
        let id = ClipboardItemId(self.next_id);
        self.items.insert(
            0,
            ClipboardItem {
                id,
                kind,
                preview,
                meta,
                payload: normalized,
            },
        );
        self.items.truncate(MAX_CLIPBOARD_ITEMS);
        self.page = 0;
        Some(id)
    }

    pub const fn toggle_pause(&mut self) -> bool {
        self.paused = !self.paused;
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.items.clear();
        self.page = 0;
        self.copied_feedback_id = None;
        true
    }

    pub fn remove(&mut self, id: ClipboardItemId) -> bool {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        self.items.remove(index);
        if self.copied_feedback_id == Some(id) {
            self.copied_feedback_id = None;
        }
        self.clamp_page();
        true
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.items.len().div_ceil(CLIPBOARD_PAGE_SIZE).max(1)
    }

    #[must_use]
    pub fn clamped_page(&self) -> usize {
        self.page.min(self.page_count().saturating_sub(1))
    }

    pub fn clamp_page(&mut self) {
        self.page = self.clamped_page();
    }

    pub fn step_page(&mut self, delta: isize) -> bool {
        let count = self.page_count();
        if count <= 1 {
            return false;
        }
        let current = self.clamped_page().cast_signed();
        let next = (current + delta)
            .rem_euclid(count.cast_signed())
            .cast_unsigned();
        if next == self.page {
            return false;
        }
        self.page = next;
        true
    }

    #[must_use]
    pub fn visible_items(&self) -> &[ClipboardItem] {
        let start = self.clamped_page() * CLIPBOARD_PAGE_SIZE;
        let end = (start + CLIPBOARD_PAGE_SIZE).min(self.items.len());
        self.items.get(start..end).unwrap_or(&[])
    }

    #[must_use]
    pub fn visible_item_at(&self, slot: u8) -> Option<&ClipboardItem> {
        self.visible_items().get(usize::from(slot))
    }

    pub fn mark_copied(&mut self, id: ClipboardItemId) {
        if self.items.iter().any(|item| item.id == id) {
            self.copied_feedback_id = Some(id);
        }
    }

    pub fn clear_copied_feedback(&mut self) -> bool {
        self.copied_feedback_id.take().is_some()
    }

    #[must_use]
    pub fn layout_input(&self) -> ClipboardLayoutInput {
        ClipboardLayoutInput {
            visible_count: u8::try_from(self.visible_items().len()).unwrap_or(0),
            has_pager: self.page_count() > 1,
            has_items: !self.items.is_empty(),
            paused: self.paused,
        }
    }
}

fn summarize_payload(payload: &ClipboardPayload) -> (ClipboardKind, String, String) {
    match payload {
        ClipboardPayload::Link(url) => (
            ClipboardKind::Link,
            sanitize_preview(url),
            format_byte_meta(url.len()),
        ),
        ClipboardPayload::Text(text) => (
            ClipboardKind::Text,
            sanitize_preview(text),
            format_byte_meta(text.len()),
        ),
        ClipboardPayload::Files(paths) => {
            let lead = paths.first().map_or("", |p| path_leaf_name(p));
            let preview = if paths.len() > 1 {
                sanitize_preview(&format!("{lead} (+{})", paths.len() - 1))
            } else {
                sanitize_preview(lead)
            };
            let meta = if paths.len() == 1 {
                String::from("1 file")
            } else {
                format!("{} files", paths.len())
            };
            (ClipboardKind::Files, preview, meta)
        }
    }
}

fn path_leaf_name(path: &str) -> &str {
    let trimmed = path.trim().trim_end_matches(['\\', '/']);
    let leaf = trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|segment| !segment.is_empty());
    leaf.unwrap_or(trimmed)
}

fn sanitize_preview(raw: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    let mut count = 0usize;
    for ch in raw.chars() {
        if ch.is_control() || ch.is_whitespace() {
            if count > 0 {
                pending_space = true;
            }
            continue;
        }
        if count >= MAX_PREVIEW_CHARS {
            out.push('…');
            break;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
            count += 1;
            if count >= MAX_PREVIEW_CHARS {
                out.push('…');
                break;
            }
        }
        out.push(ch);
        count += 1;
    }
    out
}

fn format_byte_meta(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        let kb = bytes / 1024;
        format!("{kb} KB")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CLIPBOARD_PAGE_SIZE, ClipboardKind, ClipboardPayload, ClipboardState, MAX_CLIPBOARD_FILES,
        MAX_CLIPBOARD_ITEMS, MAX_CLIPBOARD_TEXT_BYTES,
    };

    #[test]
    fn deduplicates_moves_recent_to_front_and_caps_at_twelve_items() {
        let mut state = ClipboardState::default();
        for index in 0..14 {
            assert!(
                state
                    .push_payload(ClipboardPayload::Text(format!("snippet {index}")))
                    .is_some()
            );
        }
        assert_eq!(state.items.len(), MAX_CLIPBOARD_ITEMS);
        assert_eq!(state.items[0].preview, "snippet 13");
        let existing_id = state.items[5].id;
        let pushed_id = state.push_payload(ClipboardPayload::Text(String::from("snippet 8")));
        assert_eq!(pushed_id, Some(existing_id));
        assert_eq!(state.items.len(), MAX_CLIPBOARD_ITEMS);
        assert_eq!(state.items[0].id, existing_id);
        assert_eq!(state.page, 0);
    }

    #[test]
    fn paused_mode_blocks_captures_without_dropping_existing_items() {
        let mut state = ClipboardState::default();
        let first = state
            .push_payload(ClipboardPayload::Text(String::from("keep me")))
            .expect("captured");
        assert!(state.toggle_pause());
        assert!(state.paused);
        assert!(
            state
                .push_payload(ClipboardPayload::Text(String::from("ignored while paused")))
                .is_none()
        );
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].id, first);
    }

    #[test]
    fn normalizes_urls_whitespace_utf8_cap_and_file_limits() {
        let mut state = ClipboardState::default();
        assert!(
            state
                .push_payload(ClipboardPayload::Text(String::from("  \n\t  ")))
                .is_none()
        );

        let url_id = state
            .push_payload(ClipboardPayload::Text(String::from(
                " https://otto.local/docs ",
            )))
            .expect("link captured");
        assert_eq!(state.items[0].id, url_id);
        assert_eq!(state.items[0].kind, ClipboardKind::Link);
        assert_eq!(state.items[0].preview, "https://otto.local/docs");

        let large = "é".repeat(12_000);
        assert!(state.push_payload(ClipboardPayload::Text(large)).is_some());
        let ClipboardPayload::Text(ref saved) = state.items[0].payload else {
            panic!("expected bounded text payload");
        };
        assert!(saved.len() <= MAX_CLIPBOARD_TEXT_BYTES);
        assert!(saved.len() >= MAX_CLIPBOARD_TEXT_BYTES - 2);

        let too_many_files: Vec<String> = (0..9).map(|i| format!(r"C:\Temp\doc_{i}.txt")).collect();
        assert!(
            state
                .push_payload(ClipboardPayload::Files(too_many_files))
                .is_some()
        );
        assert_eq!(state.items[0].kind, ClipboardKind::Files);
        assert_eq!(state.items[0].meta, format!("{MAX_CLIPBOARD_FILES} files"));
        assert!(state.items[0].preview.contains("doc_0.txt (+5)"));
    }

    #[test]
    fn paging_remove_and_copied_feedback_clamp_safely() {
        let mut state = ClipboardState::default();
        for index in 0..6 {
            state.push_payload(ClipboardPayload::Text(format!("line\n\t{index}")));
        }
        assert_eq!(state.page_count(), 2);
        assert_eq!(state.visible_items().len(), CLIPBOARD_PAGE_SIZE);
        assert!(state.step_page(1));
        assert_eq!(state.visible_items().len(), 2);

        let remove_a = state.visible_item_at(0).expect("row 0").id;
        state.mark_copied(remove_a);
        assert_eq!(state.copied_feedback_id, Some(remove_a));
        assert!(state.remove(remove_a));
        assert_eq!(state.copied_feedback_id, None);
        let remove_b = state.visible_item_at(0).expect("remaining row").id;
        assert!(state.remove(remove_b));
        assert_eq!(state.page_count(), 1);
        assert_eq!(state.clamped_page(), 0);
        assert!(state.clear());
        assert!(!state.clear());
    }
}
