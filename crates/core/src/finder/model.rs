use std::collections::HashMap;

use crate::dock::entry::IconBitmap;

pub const FINDER_DEFAULT_WIDTH_POINTS: i32 = 1080;
pub const FINDER_DEFAULT_HEIGHT_POINTS: i32 = 700;
pub const FINDER_MIN_WIDTH_POINTS: i32 = 720;
pub const FINDER_MIN_HEIGHT_POINTS: i32 = 460;
pub const FINDER_TOOLBAR_HEIGHT_POINTS: i32 = 52;
pub const FINDER_TAB_BAR_HEIGHT_POINTS: i32 = 28;
pub const FINDER_PATH_BAR_HEIGHT_POINTS: i32 = 24;
pub const FINDER_STATUS_BAR_HEIGHT_POINTS: i32 = 24;
pub const FINDER_SIDEBAR_DEFAULT_POINTS: i32 = 204;
pub const FINDER_SIDEBAR_MIN_POINTS: i32 = 156;
pub const FINDER_SIDEBAR_MAX_POINTS: i32 = 320;
pub const FINDER_INSPECTOR_WIDTH_POINTS: i32 = 264;
pub const FINDER_COLUMN_WIDTH_POINTS: i32 = 224;
pub const FINDER_LIST_HEADER_HEIGHT_POINTS: i32 = 26;
pub const FINDER_LIST_ROW_HEIGHT_POINTS: i32 = 28;
pub const FINDER_ICON_SIZE_MIN: i32 = 48;
pub const FINDER_ICON_SIZE_DEFAULT: i32 = 72;
pub const FINDER_ICON_SIZE_MAX: i32 = 128;
pub const FINDER_MAX_TABS: usize = 16;
pub const FINDER_MAX_UNDO: usize = 32;
pub const FINDER_TAG_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FinderViewMode {
    Icon,
    #[default]
    List,
    Column,
    Gallery,
}

impl FinderViewMode {
    pub const ALL: [Self; 4] = [Self::Icon, Self::List, Self::Column, Self::Gallery];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Icon => "Icons",
            Self::List => "List",
            Self::Column => "Columns",
            Self::Gallery => "Gallery",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Icon => 0,
            Self::List => 1,
            Self::Column => 2,
            Self::Gallery => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FinderSortKey {
    #[default]
    Name,
    DateModified,
    Size,
    Kind,
    Tags,
}

impl FinderSortKey {
    pub const ALL: [Self; 5] = [
        Self::Name,
        Self::DateModified,
        Self::Size,
        Self::Kind,
        Self::Tags,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::DateModified => "Date Modified",
            Self::Size => "Size",
            Self::Kind => "Kind",
            Self::Tags => "Tags",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FinderSortOrder {
    #[default]
    Ascending,
    Descending,
}

impl FinderSortOrder {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FinderTag {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Gray,
}

impl FinderTag {
    pub const ALL: [Self; FINDER_TAG_COUNT] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Purple,
        Self::Gray,
    ];

    #[must_use]
    pub const fn bit(self) -> u8 {
        match self {
            Self::Red => 1 << 0,
            Self::Orange => 1 << 1,
            Self::Yellow => 1 << 2,
            Self::Green => 1 << 3,
            Self::Blue => 1 << 4,
            Self::Purple => 1 << 5,
            Self::Gray => 1 << 6,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Gray => "Gray",
        }
    }

    #[must_use]
    pub const fn rgb(self) -> u32 {
        match self {
            Self::Red => 0xFF_3B30,
            Self::Orange => 0xFF_9500,
            Self::Yellow => 0xFF_CC00,
            Self::Green => 0x34_C759,
            Self::Blue => 0x00_7AFF,
            Self::Purple => 0xAF_52DE,
            Self::Gray => 0x8E_8E93,
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "red" => Some(Self::Red),
            "orange" => Some(Self::Orange),
            "yellow" => Some(Self::Yellow),
            "green" => Some(Self::Green),
            "blue" => Some(Self::Blue),
            "purple" => Some(Self::Purple),
            "gray" | "grey" => Some(Self::Gray),
            _ => None,
        }
    }
}

#[must_use]
pub fn tag_mask_contains(mask: u8, tag: FinderTag) -> bool {
    (mask & tag.bit()) != 0
}

#[must_use]
pub fn tag_mask_toggle(mask: u8, tag: FinderTag) -> u8 {
    mask ^ tag.bit()
}

#[must_use]
pub fn parse_tag_mask(raw: &str) -> u8 {
    let mut mask = 0u8;
    for token in raw.split(',') {
        if let Some(tag) = FinderTag::parse(token) {
            mask |= tag.bit();
        }
    }
    mask
}

#[must_use]
pub fn format_tag_mask(mask: u8) -> String {
    FinderTag::ALL
        .iter()
        .copied()
        .filter(|tag| tag_mask_contains(mask, *tag))
        .map(|tag| tag.label().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinderLocation {
    Path(String),
    Recents,
    Trash,
    ThisPc,
    Tag(FinderTag),
}

impl FinderLocation {
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Recents => String::from("Recents"),
            Self::Trash => String::from("Trash"),
            Self::ThisPc => String::from("This PC"),
            Self::Tag(tag) => format!("{} Tag", tag.label()),
            Self::Path(path) => path_display_name(path),
        }
    }

    #[must_use]
    pub fn fs_path(&self) -> Option<&str> {
        match self {
            Self::Path(path) => Some(path.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderSidebarSection {
    Favorites,
    Locations,
    Tags,
}

impl FinderSidebarSection {
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Favorites => "Favorites",
            Self::Locations => "Locations",
            Self::Tags => "Tags",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderSidebarIcon {
    Recents,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Home,
    Folder,
    Computer,
    Drive,
    Removable,
    Trash,
    Tag(FinderTag),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderSidebarItem {
    pub section: FinderSidebarSection,
    pub label: String,
    pub icon: FinderSidebarIcon,
    pub location: FinderLocation,
    pub ejectable: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FinderEntryKind {
    Folder,
    Drive,
    Image,
    Audio,
    Video,
    Document,
    Code,
    Archive,
    App,
    #[default]
    File,
}

impl FinderEntryKind {
    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(self, Self::Folder | Self::Drive)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderEntry {
    pub path: String,
    pub name: String,
    pub kind: FinderEntryKind,
    pub kind_label: String,
    pub size_bytes: u64,
    pub modified_unix: i64,
    pub created_unix: i64,
    pub modified_label: String,
    pub created_label: String,
    pub flags: u8,
    pub tag_mask: u8,
    pub drive_free_bytes: Option<u64>,
    pub drive_total_bytes: Option<u64>,
    pub trash_origin: Option<String>,
}

pub const ENTRY_FLAG_HIDDEN: u8 = 1 << 0;
pub const ENTRY_FLAG_READ_ONLY: u8 = 1 << 1;
pub const ENTRY_FLAG_SYMLINK: u8 = 1 << 2;

impl FinderEntry {
    #[must_use]
    pub const fn is_dir(&self) -> bool {
        self.kind.is_container()
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        (self.flags & ENTRY_FLAG_HIDDEN) != 0
    }

    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        (self.flags & ENTRY_FLAG_READ_ONLY) != 0
    }

    #[must_use]
    pub fn size_label(&self) -> String {
        if self.kind == FinderEntryKind::Folder {
            String::from("--")
        } else if let (Some(free), Some(total)) = (self.drive_free_bytes, self.drive_total_bytes) {
            format!(
                "{} free of {}",
                format_file_size(free),
                format_file_size(total)
            )
        } else {
            format_file_size(self.size_bytes)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderTreeRow {
    pub entry: FinderEntry,
    pub depth: u16,
    pub expanded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderMillerColumn {
    pub location: FinderLocation,
    pub title: String,
    pub entries: Vec<FinderEntry>,
    pub selected_path: Option<String>,
    pub scroll_y: i32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FinderPreviewKind {
    #[default]
    Empty,
    Image,
    Text,
    Folder,
    File,
}

#[derive(Clone, Debug, Default)]
pub struct FinderPreviewData {
    pub path: String,
    pub name: String,
    pub kind: FinderPreviewKind,
    pub kind_label: String,
    pub size_label: String,
    pub modified_label: String,
    pub created_label: String,
    pub item_count: Option<usize>,
    pub dimensions_label: Option<String>,
    pub text_snippet: Option<String>,
    pub bitmap: Option<IconBitmap>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderFlags(u16);

impl FinderFlags {
    pub const SIDEBAR: u16 = 1 << 0;
    pub const PATH_BAR: u16 = 1 << 1;
    pub const STATUS_BAR: u16 = 1 << 2;
    pub const PREVIEW_PANE: u16 = 1 << 3;
    pub const INSPECTOR: u16 = 1 << 4;
    pub const SHOW_HIDDEN: u16 = 1 << 5;
    pub const FOLDERS_ON_TOP: u16 = 1 << 6;
    pub const QUICK_LOOK: u16 = 1 << 7;
    pub const GO_TO_FOLDER: u16 = 1 << 8;
    pub const CONFIRM_PERM_DELETE: u16 = 1 << 9;
    pub const SEARCH_FOCUSED: u16 = 1 << 10;
    pub const SEARCH_RECURSIVE: u16 = 1 << 11;

    #[must_use]
    pub const fn new_default() -> Self {
        Self(Self::SIDEBAR | Self::PATH_BAR | Self::STATUS_BAR | Self::FOLDERS_ON_TOP)
    }

    #[must_use]
    pub const fn has(self, mask: u16) -> bool {
        (self.0 & mask) != 0
    }

    pub fn set(&mut self, mask: u16, value: bool) {
        if value {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    pub fn toggle(&mut self, mask: u16) {
        self.0 ^= mask;
    }
}

impl Default for FinderFlags {
    fn default() -> Self {
        Self::new_default()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderRenameState {
    pub path: String,
    pub draft: String,
    pub select_end: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinderGoToState {
    pub input: String,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinderUndoAction {
    Rename { from: String, to: String },
    Move { moves: Vec<(String, String)> },
    NewFolder { path: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderTab {
    pub id: u64,
    pub title: String,
    pub location: FinderLocation,
    pub history_back: Vec<FinderLocation>,
    pub history_forward: Vec<FinderLocation>,
    pub view_mode: FinderViewMode,
    pub sort_key: FinderSortKey,
    pub sort_order: FinderSortOrder,
    pub entries: Vec<FinderEntry>,
    pub expanded_children: HashMap<String, Vec<FinderEntry>>,
    pub miller_columns: Vec<FinderMillerColumn>,
    pub selected_paths: Vec<String>,
    pub anchor_path: Option<String>,
    pub scroll_y: i32,
    pub scroll_x: i32,
    pub search_query: String,
    pub free_space_label: String,
    pub listing_status: super::port::FinderListingStatus,
}

impl FinderTab {
    #[must_use]
    pub fn new(id: u64, location: FinderLocation, view_mode: FinderViewMode) -> Self {
        let title = location.title();
        Self {
            id,
            title,
            location,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            view_mode,
            sort_key: FinderSortKey::Name,
            sort_order: FinderSortOrder::Ascending,
            entries: Vec::new(),
            expanded_children: HashMap::new(),
            miller_columns: Vec::new(),
            selected_paths: Vec::new(),
            anchor_path: None,
            scroll_y: 0,
            scroll_x: 0,
            search_query: String::new(),
            free_space_label: String::new(),
            listing_status: super::port::FinderListingStatus::Ok,
        }
    }

    #[must_use]
    pub fn primary_selected_path(&self) -> Option<&str> {
        self.selected_paths.last().map(String::as_str)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinderMetrics {
    pub default_width: i32,
    pub default_height: i32,
    pub min_width: i32,
    pub min_height: i32,
    pub toolbar_height: i32,
    pub tab_bar_height: i32,
    pub path_bar_height: i32,
    pub status_bar_height: i32,
    pub sidebar_width: i32,
    pub inspector_width: i32,
    pub column_width: i32,
    pub icon_size: i32,
}

impl FinderMetrics {
    pub const DEFAULT: Self = Self {
        default_width: FINDER_DEFAULT_WIDTH_POINTS,
        default_height: FINDER_DEFAULT_HEIGHT_POINTS,
        min_width: FINDER_MIN_WIDTH_POINTS,
        min_height: FINDER_MIN_HEIGHT_POINTS,
        toolbar_height: FINDER_TOOLBAR_HEIGHT_POINTS,
        tab_bar_height: FINDER_TAB_BAR_HEIGHT_POINTS,
        path_bar_height: FINDER_PATH_BAR_HEIGHT_POINTS,
        status_bar_height: FINDER_STATUS_BAR_HEIGHT_POINTS,
        sidebar_width: FINDER_SIDEBAR_DEFAULT_POINTS,
        inspector_width: FINDER_INSPECTOR_WIDTH_POINTS,
        column_width: FINDER_COLUMN_WIDTH_POINTS,
        icon_size: FINDER_ICON_SIZE_DEFAULT,
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinderBreadcrumb {
    pub label: String,
    pub location: FinderLocation,
}

pub use super::format::{
    breadcrumbs_for_location, classify_entry_kind, format_file_size, format_unix_timestamp,
    is_text_preview_extension, parent_path_of, path_display_name, stem_selection_end,
};
