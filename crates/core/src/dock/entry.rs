use std::path::PathBuf;

use crate::app_model::{AppInfo, WindowPresence};
use crate::dock::layout::AppleIconKind;

#[derive(Debug, Clone)]
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackItem {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockItemKind {
    Finder,
    Apps,
    App { exe_path: String },
    Downloads,
    Trash { is_full: bool },
    Overflow { hidden: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct DockEntry {
    pub key: String,
    pub title: String,
    pub kind: DockItemKind,
    pub apple_icon: Option<AppleIconKind>,
    pub app: Option<AppInfo>,
    pub hwnds: Vec<isize>,
    pub window_titles: Vec<String>,
    pub running: bool,
    pub active: bool,
    pub pinned: bool,
    pub hidden: bool,
    pub minimized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DockEntryOptions {
    pub show_downloads: bool,
    pub show_trash: bool,
}

impl Default for DockEntryOptions {
    fn default() -> Self {
        Self {
            show_downloads: true,
            show_trash: true,
        }
    }
}

#[must_use]
pub fn other_applications_visible(entries: &[DockEntry], key: &str) -> bool {
    entries.iter().any(|entry| {
        entry.key != key
            && entry.app.as_ref().is_some_and(|app| {
                app.primary_windows()
                    .any(|window| window.presence == WindowPresence::Visible)
            })
    })
}
