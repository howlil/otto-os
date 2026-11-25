use std::collections::HashMap;

use super::model::{
    FinderClipboardMode, FinderEntry, FinderLocation, FinderPreviewData, FinderSidebarItem,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinderListing {
    pub entries: Vec<FinderEntry>,
    pub free_space_label: String,
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
    fn duplicate_paths(&self, paths: &[String]) -> Vec<String>;
    fn recycle_paths(&self, paths: &[String]) -> bool;
    fn delete_paths_permanently(&self, paths: &[String]) -> bool;
    fn empty_trash(&self) -> bool;
    fn restore_trash_paths(&self, paths: &[String]) -> bool;
    fn copy_paths_to_clipboard(&self, paths: &[String], mode: FinderClipboardMode) -> bool;
    fn read_clipboard_paths(&self) -> Option<FinderClipboardData>;
    fn paste_paths(
        &self,
        sources: &[String],
        dest_dir: &str,
        move_items: bool,
    ) -> Vec<(String, String)>;
    fn load_preview(&self, entry: &FinderEntry) -> FinderPreviewData;
    fn path_exists_dir(&self, raw: &str) -> Option<String>;
    fn copy_text_to_clipboard(&self, text: &str) -> bool;
}
