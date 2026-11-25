use std::collections::HashMap;

use super::model::{
    FinderClipboardMode, FinderEntry, FinderLocation, FinderPreviewData, FinderSidebarItem,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum FinderListingStatus {
    #[default]
    Ok,
    NotFound,
    AccessDenied,
    Unavailable,
    Partial {
        skipped_entries: usize,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinderListing {
    pub entries: Vec<FinderEntry>,
    pub free_space_label: String,
    pub status: FinderListingStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderItemStatus {
    Completed,
    Skipped,
    Conflict,
    Canceled,
    Failed,
    Partial,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderItemOutcome {
    pub source: String,
    pub destination: Option<String>,
    pub status: FinderItemStatus,
    pub error: Option<String>,
}

impl FinderItemOutcome {
    #[must_use]
    pub fn completed(source: impl Into<String>, destination: Option<String>) -> Self {
        Self {
            source: source.into(),
            destination,
            status: FinderItemStatus::Completed,
            error: None,
        }
    }

    #[must_use]
    pub fn with_status(
        source: impl Into<String>,
        destination: Option<String>,
        status: FinderItemStatus,
        error: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            destination,
            status,
            error: Some(error.into()),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinderBatchOutcome {
    pub items: Vec<FinderItemOutcome>,
}

impl FinderBatchOutcome {
    #[must_use]
    pub fn any_mutated(&self) -> bool {
        self.items.iter().any(|item| {
            matches!(
                item.status,
                FinderItemStatus::Completed | FinderItemStatus::Partial
            )
        })
    }

    #[must_use]
    pub fn all_completed(&self) -> bool {
        !self.items.is_empty()
            && self
                .items
                .iter()
                .all(|item| item.status == FinderItemStatus::Completed)
    }

    #[must_use]
    pub fn completed_pairs(&self) -> Vec<(String, String)> {
        self.items
            .iter()
            .filter(|item| item.status == FinderItemStatus::Completed)
            .filter_map(|item| {
                item.destination
                    .as_ref()
                    .map(|dst| (item.source.clone(), dst.clone()))
            })
            .collect()
    }

    #[must_use]
    pub fn completed_destinations(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.status == FinderItemStatus::Completed)
            .filter_map(|item| item.destination.clone())
            .collect()
    }

    #[must_use]
    pub fn failed_sources(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.status != FinderItemStatus::Completed)
            .map(|item| item.source.clone())
            .collect()
    }

    #[must_use]
    pub fn summary_notice(&self, action: &str) -> Option<String> {
        if self.items.is_empty() || self.all_completed() {
            return None;
        }
        let completed = self
            .items
            .iter()
            .filter(|item| item.status == FinderItemStatus::Completed)
            .count();
        let failed = self.items.len().saturating_sub(completed);
        let first_err = self
            .items
            .iter()
            .find(|item| item.status != FinderItemStatus::Completed)
            .and_then(|item| item.error.as_deref());
        if completed == 0 {
            Some(match first_err {
                Some(err) => format!("{action} failed: {err}"),
                None => format!("{action} failed for {failed} item(s)"),
            })
        } else {
            Some(match first_err {
                Some(err) => format!("{action}: {completed} succeeded, {failed} failed ({err})"),
                None => format!("{action}: {completed} succeeded, {failed} failed"),
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderClipboardData {
    pub paths: Vec<String>,
    pub mode: FinderClipboardMode,
}

pub trait FinderPort {
    fn sidebar_items(&self) -> Vec<FinderSidebarItem>;
    fn default_location(&self) -> FinderLocation;
    fn list_location(
        &self,
        location: &FinderLocation,
        recursive_query: Option<&str>,
    ) -> FinderListing;
    fn list_tagged(&self, tag_mask_bit: u8, tags: &HashMap<String, u8>) -> FinderListing;
    fn load_tags(&self) -> HashMap<String, u8>;
    fn save_tags(&self, tags: &HashMap<String, u8>) -> bool;
    fn open_path(&self, path: &str) -> bool;
    fn open_in_terminal(&self, dir_path: &str) -> bool;
    fn create_folder(&self, parent_dir: &str) -> Option<String>;
    fn rename_path(&self, path: &str, new_name: &str) -> Option<String>;
    fn duplicate_paths(&self, paths: &[String]) -> FinderBatchOutcome;
    fn recycle_paths(&self, paths: &[String]) -> FinderBatchOutcome;
    fn delete_paths_permanently(&self, paths: &[String]) -> FinderBatchOutcome;
    fn empty_trash(&self) -> bool;
    fn restore_trash_paths(&self, paths: &[String]) -> FinderBatchOutcome;
    fn copy_paths_to_clipboard(&self, paths: &[String], mode: FinderClipboardMode) -> bool;
    fn read_clipboard_paths(&self) -> Option<FinderClipboardData>;
    fn paste_paths(
        &self,
        sources: &[String],
        dest_dir: &str,
        move_items: bool,
    ) -> FinderBatchOutcome;
    fn move_exact(&self, moves: &[(String, String)]) -> FinderBatchOutcome;
    fn load_preview(&self, entry: &FinderEntry) -> FinderPreviewData;
    fn path_exists_dir(&self, raw: &str) -> Option<String>;
    fn copy_text_to_clipboard(&self, text: &str) -> bool;
}
