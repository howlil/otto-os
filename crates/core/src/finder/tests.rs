use std::cell::RefCell;
use std::collections::HashMap;

use super::commands::{
    CMD_FINDER_SORT_SIZE, CMD_FINDER_TAG_BASE, FinderKey, FinderKeyMods, FinderKeyOutcome,
    build_finder_context_menu_rows, confirm_permanent_delete, execute_finder_menu_command,
    handle_finder_key,
};
use super::layout::{FinderHit, FinderLayoutInput, FinderToolbarButton, compute_finder_layout};
use super::model::{
    ENTRY_FLAG_HIDDEN, FinderClipboardMode, FinderEntry, FinderEntryKind, FinderFlags,
    FinderLocation, FinderPreviewData, FinderPreviewKind, FinderSidebarIcon, FinderSidebarItem,
    FinderSidebarSection, FinderSortKey, FinderTag, FinderViewMode, breadcrumbs_for_location,
    classify_entry_kind, format_file_size, format_tag_mask, parent_path_of, parse_tag_mask,
    stem_selection_end, tag_mask_contains,
};
use super::port::{
    FinderBatchOutcome, FinderClipboardData, FinderItemOutcome, FinderListing, FinderListingStatus,
    FinderPort,
};
use super::state::{FinderState, compare_natural};

struct MockFinderPort {
    dirs: RefCell<HashMap<String, Vec<FinderEntry>>>,
    opened: RefCell<Vec<String>>,
    recycled: RefCell<Vec<String>>,
    deleted: RefCell<Vec<String>>,
    clipboard: RefCell<Option<FinderClipboardData>>,
    pasted_moves: RefCell<Vec<(Vec<String>, String, bool)>>,
    saved_tags: RefCell<HashMap<String, u8>>,
}

impl MockFinderPort {
    fn sample() -> Self {
        let mut dirs = HashMap::new();
        dirs.insert(
            String::from(r"c:\users\demo\documents"),
            vec![
                entry(
                    r"C:\Users\Demo\Documents\Projects",
                    "Projects",
                    true,
                    0,
                    200,
                    0,
                ),
                entry(
                    r"C:\Users\Demo\Documents\report.pdf",
                    "report.pdf",
                    false,
                    4200,
                    300,
                    0,
                ),
                entry(
                    r"C:\Users\Demo\Documents\notes 2.txt",
                    "notes 2.txt",
                    false,
                    120,
                    100,
                    0,
                ),
                entry(
                    r"C:\Users\Demo\Documents\notes 10.txt",
                    "notes 10.txt",
                    false,
                    950,
                    150,
                    0,
                ),
                entry(
                    r"C:\Users\Demo\Documents\.env",
                    ".env",
                    false,
                    64,
                    50,
                    ENTRY_FLAG_HIDDEN,
                ),
            ],
        );
        dirs.insert(
            String::from(r"c:\users\demo\documents\projects"),
            vec![entry(
                r"C:\Users\Demo\Documents\Projects\otto.rs",
                "otto.rs",
                false,
                8192,
                400,
                0,
            )],
        );
        Self {
            dirs: RefCell::new(dirs),
            opened: RefCell::new(Vec::new()),
            recycled: RefCell::new(Vec::new()),
            deleted: RefCell::new(Vec::new()),
            clipboard: RefCell::new(None),
            pasted_moves: RefCell::new(Vec::new()),
            saved_tags: RefCell::new(HashMap::new()),
        }
    }
}

fn entry(path: &str, name: &str, is_dir: bool, size: u64, modified: i64, flags: u8) -> FinderEntry {
    let (kind, kind_label) = classify_entry_kind(name, is_dir);
    FinderEntry {
        path: String::from(path),
        name: String::from(name),
        kind,
        kind_label: String::from(kind_label),
        size_bytes: size,
        modified_unix: modified,
        created_unix: modified,
        modified_label: String::from("Today"),
        created_label: String::from("Today"),
        flags,
        tag_mask: 0,
        drive_free_bytes: None,
        drive_total_bytes: None,
        trash_origin: None,
    }
}

impl FinderPort for MockFinderPort {
    fn sidebar_items(&self) -> Vec<FinderSidebarItem> {
        vec![
            FinderSidebarItem {
                section: FinderSidebarSection::Favorites,
                label: String::from("Documents"),
                icon: FinderSidebarIcon::Documents,
                location: FinderLocation::Path(String::from(r"C:\Users\Demo\Documents")),
                ejectable: false,
            },
            FinderSidebarItem {
                section: FinderSidebarSection::Locations,
                label: String::from("Trash"),
                icon: FinderSidebarIcon::Trash,
                location: FinderLocation::Trash,
                ejectable: false,
            },
        ]
    }

    fn default_location(&self) -> FinderLocation {
        FinderLocation::Path(String::from(r"C:\Users\Demo\Documents"))
    }

    fn list_location(
        &self,
        location: &FinderLocation,
        _recursive_query: Option<&str>,
    ) -> FinderListing {
        let key = match location {
            FinderLocation::Path(path) => path.to_ascii_lowercase(),
            _ => String::new(),
        };
        let entries = self.dirs.borrow().get(&key).cloned().unwrap_or_default();
        FinderListing {
            entries,
            free_space_label: String::from("128.4 GB available"),
            status: FinderListingStatus::Ok,
        }
    }

    fn list_tagged(&self, bit: u8, tags: &HashMap<String, u8>) -> FinderListing {
        let mut entries = Vec::new();
        for list in self.dirs.borrow().values() {
            for item in list {
                if tags
                    .get(&item.path.to_ascii_lowercase())
                    .is_some_and(|mask| (mask & bit) != 0)
                {
                    entries.push(item.clone());
                }
            }
        }
        FinderListing {
            entries,
            free_space_label: String::new(),
            status: FinderListingStatus::Ok,
        }
    }

    fn load_tags(&self) -> HashMap<String, u8> {
        self.saved_tags.borrow().clone()
    }

    fn save_tags(&self, tags: &HashMap<String, u8>) -> bool {
        *self.saved_tags.borrow_mut() = tags.clone();
        true
    }

    fn open_path(&self, path: &str) -> bool {
        self.opened.borrow_mut().push(String::from(path));
        true
    }

    fn open_in_terminal(&self, _dir_path: &str) -> bool {
        true
    }

    fn create_folder(&self, parent_dir: &str) -> Option<String> {
        let created = format!(r"{parent_dir}\untitled folder");
        let key = parent_dir.to_ascii_lowercase();
        self.dirs.borrow_mut().entry(key).or_default().push(entry(
            &created,
            "untitled folder",
            true,
            0,
            500,
            0,
        ));
        Some(created)
    }

    fn rename_path(&self, path: &str, new_name: &str) -> Option<String> {
        let parent = parent_path_of(path)?;
        let next = format!(r"{parent}\{new_name}");
        let key = parent.to_ascii_lowercase();
        let mut dirs = self.dirs.borrow_mut();
        let list = dirs.get_mut(&key)?;
        let item = list
            .iter_mut()
            .find(|item| item.path.eq_ignore_ascii_case(path))?;
        item.path.clone_from(&next);
        item.name = String::from(new_name);
        Some(next)
    }

    fn duplicate_paths(&self, paths: &[String]) -> FinderBatchOutcome {
        FinderBatchOutcome {
            items: paths
                .iter()
                .map(|p| FinderItemOutcome::completed(p, Some(format!("{p} copy"))))
                .collect(),
        }
    }

    fn recycle_paths(&self, paths: &[String]) -> FinderBatchOutcome {
        self.recycled.borrow_mut().extend(paths.iter().cloned());
        FinderBatchOutcome {
            items: paths
                .iter()
                .map(|p| FinderItemOutcome::completed(p, None))
                .collect(),
        }
    }

    fn delete_paths_permanently(&self, paths: &[String]) -> FinderBatchOutcome {
        self.deleted.borrow_mut().extend(paths.iter().cloned());
        FinderBatchOutcome {
            items: paths
                .iter()
                .map(|p| FinderItemOutcome::completed(p, None))
                .collect(),
        }
    }

    fn empty_trash(&self) -> bool {
        true
    }

    fn restore_trash_paths(&self, paths: &[String]) -> FinderBatchOutcome {
        FinderBatchOutcome {
            items: paths
                .iter()
                .map(|p| FinderItemOutcome::completed(p, None))
                .collect(),
        }
    }

    fn copy_paths_to_clipboard(&self, paths: &[String], mode: FinderClipboardMode) -> bool {
        *self.clipboard.borrow_mut() = Some(FinderClipboardData {
            paths: paths.to_vec(),
            mode,
        });
        true
    }

    fn read_clipboard_paths(&self) -> Option<FinderClipboardData> {
        self.clipboard.borrow().clone()
    }

    fn paste_paths(
        &self,
        sources: &[String],
        dest_dir: &str,
        move_items: bool,
    ) -> FinderBatchOutcome {
        self.pasted_moves
            .borrow_mut()
            .push((sources.to_vec(), String::from(dest_dir), move_items));
        FinderBatchOutcome {
            items: sources
                .iter()
                .map(|src| {
                    FinderItemOutcome::completed(src, Some(format!(r"{dest_dir}\moved.dat")))
                })
                .collect(),
        }
    }

    fn move_exact(&self, moves: &[(String, String)]) -> FinderBatchOutcome {
        FinderBatchOutcome {
            items: moves
                .iter()
                .map(|(src, dst)| FinderItemOutcome::completed(src, Some(dst.clone())))
                .collect(),
        }
    }

    fn load_preview(&self, item: &FinderEntry) -> FinderPreviewData {
        FinderPreviewData {
            path: item.path.clone(),
            name: item.name.clone(),
            kind: if item.is_dir() {
                FinderPreviewKind::Folder
            } else {
                FinderPreviewKind::Text
            },
            kind_label: item.kind_label.clone(),
            size_label: item.size_label(),
            modified_label: item.modified_label.clone(),
            created_label: item.created_label.clone(),
            item_count: item.is_dir().then_some(1),
            dimensions_label: None,
            text_snippet: Some(String::from("sample preview")),
            bitmap: None,
        }
    }

    fn path_exists_dir(&self, raw: &str) -> Option<String> {
        self.dirs
            .borrow()
            .contains_key(&raw.to_ascii_lowercase())
            .then(|| String::from(raw))
    }

    fn copy_text_to_clipboard(&self, _text: &str) -> bool {
        true
    }
}

#[test]
fn natural_sort_folders_on_top_and_hidden_toggle() {
    let port = MockFinderPort::sample();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());
    state.reload_active_tab(&port);
    let names: Vec<&str> = state
        .tab()
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["Projects", "notes 2.txt", "notes 10.txt", "report.pdf"]
    );
    assert_eq!(
        compare_natural("file 2", "file 10"),
        std::cmp::Ordering::Less
    );

    let outcome = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Char('.'),
        FinderKeyMods::new(true, true, false),
        10,
    );
    assert_eq!(outcome, FinderKeyOutcome::Handled);
    assert!(state.tab().entries.iter().any(|e| e.name == ".env"));
}

#[test]
fn macos_hybrid_shortcuts_rename_stem_quick_look_move_and_trash() {
    let port = MockFinderPort::sample();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());
    state.reload_active_tab(&port);
    state.select_path(&port, r"C:\Users\Demo\Documents\report.pdf", false, false);

    assert_eq!(stem_selection_end("report.pdf", false), 6);
    assert_eq!(stem_selection_end("Projects", true), 8);
    assert_eq!(stem_selection_end(".gitignore", false), 10);

    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Enter,
        FinderKeyMods::default(),
        0,
    );
    let rename = state.rename.as_ref().expect("inline rename started");
    assert_eq!(rename.select_end, 6);
    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Char('X'),
        FinderKeyMods::default(),
        0,
    );
    assert_eq!(state.rename.as_ref().expect("draft").draft, "X.pdf");
    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Enter,
        FinderKeyMods::default(),
        0,
    );
    assert!(state.rename.is_none());
    assert!(state.tab().entries.iter().any(|e| e.name == "X.pdf"));

    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Space,
        FinderKeyMods::default(),
        0,
    );
    assert!(state.flags.has(FinderFlags::QUICK_LOOK));
    assert_eq!(state.preview.name, "X.pdf");
    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Escape,
        FinderKeyMods::default(),
        0,
    );
    assert!(!state.flags.has(FinderFlags::QUICK_LOOK));

    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Char('c'),
        FinderKeyMods::new(true, false, false),
        0,
    );
    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Char('v'),
        FinderKeyMods::new(true, false, true),
        0,
    );
    assert!(port.pasted_moves.borrow()[0].2);

    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Delete,
        FinderKeyMods::default(),
        0,
    );
    assert_eq!(port.recycled.borrow().len(), 1);

    state.select_path(&port, r"C:\Users\Demo\Documents\notes 2.txt", false, false);
    let _ = handle_finder_key(
        &mut state,
        &port,
        FinderKey::Delete,
        FinderKeyMods::new(false, true, false),
        0,
    );
    assert!(state.flags.has(FinderFlags::CONFIRM_PERM_DELETE));
    confirm_permanent_delete(&mut state, &port);
    assert_eq!(port.deleted.borrow().len(), 1);
}

#[test]
fn list_tree_expand_miller_columns_and_all_four_view_layouts_hit_test() {
    let port = MockFinderPort::sample();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());
    state.reload_active_tab(&port);
    state.toggle_folder_expand(&port, r"C:\Users\Demo\Documents\Projects");
    let rows = state.visible_list_rows();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].entry.name, "otto.rs");

    for mode in FinderViewMode::ALL {
        state.set_view_mode(&port, mode);
        if mode == FinderViewMode::Column {
            state.select_miller_item(&port, 0, r"C:\Users\Demo\Documents\Projects", false);
            assert_eq!(state.tab().miller_columns.len(), 2);
        }
        let rows = state.visible_list_rows();
        let layout = compute_finder_layout(&FinderLayoutInput {
            width: 1120,
            height: 720,
            dpi: 96,
            metrics: state.metrics,
            flags: state.flags,
            tabs: &state.tabs,
            active_tab: state.active_tab,
            sidebar_items: &state.sidebar_items,
            list_rows: &rows,
            visible_entries_count: state.tab().entries.len(),
            active_columns: &state.tab().miller_columns,
        });
        let view_btn = layout
            .toolbar_buttons
            .iter()
            .find(|b| b.button == FinderToolbarButton::ViewMode(mode))
            .expect("view button present");
        assert!(view_btn.active);
        assert_eq!(
            layout.hit_test(view_btn.rect.x + 2, view_btn.rect.y + 2),
            Some(FinderHit::Toolbar(FinderToolbarButton::ViewMode(mode)))
        );
    }
}

#[test]
fn tags_breadcrumbs_menu_commands_and_size_formatting() {
    let port = MockFinderPort::sample();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());
    state.reload_active_tab(&port);
    state.select_path(&port, r"C:\Users\Demo\Documents\report.pdf", false, false);

    execute_finder_menu_command(&mut state, &port, CMD_FINDER_TAG_BASE);
    let tagged = state.primary_selected_entry().expect("selected");
    assert!(tag_mask_contains(tagged.tag_mask, FinderTag::Red));
    assert_eq!(
        parse_tag_mask(&format_tag_mask(tagged.tag_mask)),
        tagged.tag_mask
    );

    execute_finder_menu_command(&mut state, &port, CMD_FINDER_SORT_SIZE);
    assert_eq!(state.tab().sort_key, FinderSortKey::Size);

    let menu = build_finder_context_menu_rows(&state, true);
    assert!(menu.len() > 10);
    let action_menu = super::build_finder_action_menu_rows(&state);
    for row in menu.iter().chain(action_menu.iter()) {
        if let crate::MenuRow::Item { label, .. } = row {
            assert!(
                !label.to_ascii_lowercase().contains("explorer"),
                "Finder menu should not expose Explorer option: {label}"
            );
        }
    }

    let crumbs = breadcrumbs_for_location(&FinderLocation::Path(String::from(
        r"C:\Users\Demo\Documents",
    )));
    assert_eq!(crumbs.len(), 4);
    assert_eq!(crumbs[0].label, "C:");
    assert_eq!(crumbs[3].label, "Documents");
    assert_eq!(format_file_size(1_500_000), "1.5 MB");
    assert_eq!(
        classify_entry_kind("demo.rs", false).0,
        FinderEntryKind::Code
    );
}

struct FaultyFinderPort {
    listing_status: RefCell<FinderListingStatus>,
    open_ok: RefCell<bool>,
    move_exact_outcomes: RefCell<Vec<FinderItemOutcome>>,
    recycle_outcomes: RefCell<Vec<FinderItemOutcome>>,
    delete_outcomes: RefCell<Vec<FinderItemOutcome>>,
    rename_ok: RefCell<bool>,
    recorded_exact_moves: RefCell<Vec<Vec<(String, String)>>>,
    recorded_deletes: RefCell<Vec<Vec<String>>>,
}

impl FaultyFinderPort {
    fn new() -> Self {
        Self {
            listing_status: RefCell::new(FinderListingStatus::Ok),
            open_ok: RefCell::new(true),
            move_exact_outcomes: RefCell::new(Vec::new()),
            recycle_outcomes: RefCell::new(Vec::new()),
            delete_outcomes: RefCell::new(Vec::new()),
            rename_ok: RefCell::new(true),
            recorded_exact_moves: RefCell::new(Vec::new()),
            recorded_deletes: RefCell::new(Vec::new()),
        }
    }
}

impl FinderPort for FaultyFinderPort {
    fn sidebar_items(&self) -> Vec<FinderSidebarItem> {
        Vec::new()
    }

    fn default_location(&self) -> FinderLocation {
        FinderLocation::Path(String::from(r"C:\Users\Demo\Documents"))
    }

    fn list_location(
        &self,
        _location: &FinderLocation,
        _recursive_query: Option<&str>,
    ) -> FinderListing {
        FinderListing {
            entries: vec![
                entry(r"C:\Users\Demo\Documents\a.txt", "a.txt", false, 10, 100, 0),
                entry(r"C:\Users\Demo\Documents\b.txt", "b.txt", false, 20, 100, 0),
            ],
            free_space_label: String::from("10 GB available"),
            status: self.listing_status.borrow().clone(),
        }
    }

    fn list_tagged(&self, _bit: u8, _tags: &HashMap<String, u8>) -> FinderListing {
        FinderListing::default()
    }

    fn load_tags(&self) -> HashMap<String, u8> {
        HashMap::new()
    }

    fn save_tags(&self, _tags: &HashMap<String, u8>) -> bool {
        true
    }

    fn open_path(&self, _path: &str) -> bool {
        *self.open_ok.borrow()
    }

    fn open_in_terminal(&self, _dir_path: &str) -> bool {
        true
    }

    fn create_folder(&self, _parent_dir: &str) -> Option<String> {
        None
    }

    fn rename_path(&self, path: &str, new_name: &str) -> Option<String> {
        if *self.rename_ok.borrow() {
            let parent = parent_path_of(path)?;
            Some(format!(r"{parent}\{new_name}"))
        } else {
            None
        }
    }

    fn duplicate_paths(&self, _paths: &[String]) -> FinderBatchOutcome {
        FinderBatchOutcome::default()
    }

    fn recycle_paths(&self, _paths: &[String]) -> FinderBatchOutcome {
        FinderBatchOutcome {
            items: self.recycle_outcomes.borrow().clone(),
        }
    }

    fn delete_paths_permanently(&self, paths: &[String]) -> FinderBatchOutcome {
        self.recorded_deletes.borrow_mut().push(paths.to_vec());
        FinderBatchOutcome {
            items: self.delete_outcomes.borrow().clone(),
        }
    }

    fn empty_trash(&self) -> bool {
        true
    }

    fn restore_trash_paths(&self, _paths: &[String]) -> FinderBatchOutcome {
        FinderBatchOutcome::default()
    }

    fn copy_paths_to_clipboard(&self, _paths: &[String], _mode: FinderClipboardMode) -> bool {
        true
    }

    fn read_clipboard_paths(&self) -> Option<FinderClipboardData> {
        None
    }

    fn paste_paths(
        &self,
        _sources: &[String],
        _dest_dir: &str,
        _move_items: bool,
    ) -> FinderBatchOutcome {
        FinderBatchOutcome::default()
    }

    fn move_exact(&self, moves: &[(String, String)]) -> FinderBatchOutcome {
        self.recorded_exact_moves.borrow_mut().push(moves.to_vec());
        FinderBatchOutcome {
            items: self.move_exact_outcomes.borrow().clone(),
        }
    }

    fn load_preview(&self, _entry: &FinderEntry) -> FinderPreviewData {
        FinderPreviewData::default()
    }

    fn path_exists_dir(&self, _raw: &str) -> Option<String> {
        None
    }

    fn copy_text_to_clipboard(&self, _text: &str) -> bool {
        true
    }
}

#[test]
fn listing_status_labels_distinguish_errors_and_partial_results() {
    let port = FaultyFinderPort::new();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());

    *port.listing_status.borrow_mut() = FinderListingStatus::NotFound;
    state.reload_active_tab(&port);
    assert_eq!(state.status_bar_label(), "Folder not found");

    *port.listing_status.borrow_mut() = FinderListingStatus::AccessDenied;
    state.reload_active_tab(&port);
    assert_eq!(state.status_bar_label(), "Access denied");

    *port.listing_status.borrow_mut() = FinderListingStatus::Unavailable;
    state.reload_active_tab(&port);
    assert_eq!(state.status_bar_label(), "Location unavailable");

    *port.listing_status.borrow_mut() = FinderListingStatus::Partial { skipped_entries: 3 };
    state.reload_active_tab(&port);
    assert!(state.status_bar_label().contains("3 item(s) unreadable"));
}

#[test]
fn undo_move_uses_exact_destination_and_retains_failed_items() {
    let port = FaultyFinderPort::new();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());
    state.push_undo(super::model::FinderUndoAction::Move {
        moves: vec![
            (
                String::from(r"C:\Src\one.txt"),
                String::from(r"C:\Dst\one.txt"),
            ),
            (
                String::from(r"C:\Src\two.txt"),
                String::from(r"C:\Dst\two.txt"),
            ),
        ],
    });
    *port.move_exact_outcomes.borrow_mut() = vec![
        FinderItemOutcome::with_status(
            r"C:\Dst\two.txt",
            Some(r"C:\Src\two.txt".to_owned()),
            super::port::FinderItemStatus::Conflict,
            "Destination already exists",
        ),
        FinderItemOutcome::completed(r"C:\Dst\one.txt", Some(r"C:\Src\one.txt".to_owned())),
    ];
    assert!(!state.undo(&port));
    assert!(state.status_notice.is_some());
    assert_eq!(state.undo_stack.len(), 1);
    assert_eq!(
        state.undo_stack[0],
        super::model::FinderUndoAction::Move {
            moves: vec![(
                String::from(r"C:\Src\two.txt"),
                String::from(r"C:\Dst\two.txt")
            )],
        }
    );

    *port.rename_ok.borrow_mut() = false;
    state.push_undo(super::model::FinderUndoAction::Rename {
        from: String::from(r"C:\Src\a.txt"),
        to: String::from(r"C:\Src\b.txt"),
    });
    assert!(!state.undo(&port));
    assert_eq!(state.undo_stack.len(), 2);
}

#[test]
fn partial_delete_retains_failed_selection_and_uses_authorized_snapshot() {
    let port = FaultyFinderPort::new();
    let mut state = FinderState::new(port.default_location(), port.sidebar_items());
    state.reload_active_tab(&port);
    state.tab_mut().selected_paths = vec![
        String::from(r"C:\Users\Demo\Documents\a.txt"),
        String::from(r"C:\Users\Demo\Documents\b.txt"),
    ];
    super::commands::request_permanent_delete(&mut state);
    state.tab_mut().selected_paths = vec![String::from(r"C:\Users\Demo\Documents\other.txt")];
    *port.delete_outcomes.borrow_mut() = vec![
        FinderItemOutcome::completed(r"C:\Users\Demo\Documents\a.txt", None),
        FinderItemOutcome::with_status(
            r"C:\Users\Demo\Documents\b.txt",
            None,
            super::port::FinderItemStatus::Failed,
            "Access denied",
        ),
    ];
    confirm_permanent_delete(&mut state, &port);
    assert_eq!(
        port.recorded_deletes.borrow()[0],
        vec![
            String::from(r"C:\Users\Demo\Documents\a.txt"),
            String::from(r"C:\Users\Demo\Documents\b.txt"),
        ]
    );
    assert_eq!(
        state.tab().selected_paths,
        vec![String::from(r"C:\Users\Demo\Documents\b.txt")]
    );
    assert!(state.status_notice.is_some());

    *port.open_ok.borrow_mut() = false;
    state.tab_mut().selected_paths = vec![String::from(r"C:\Users\Demo\Documents\a.txt")];
    super::commands::open_selected(&mut state, &port);
    assert_eq!(state.status_notice.as_deref(), Some("Could not open a.txt"));
}
