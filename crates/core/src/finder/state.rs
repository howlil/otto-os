use std::collections::HashMap;

pub use super::format::{compare_natural, sort_entries};
use super::model::{
    FINDER_ICON_SIZE_MAX, FINDER_ICON_SIZE_MIN, FINDER_MAX_TABS, FINDER_MAX_UNDO,
    FINDER_SIDEBAR_MAX_POINTS, FINDER_SIDEBAR_MIN_POINTS, FinderClipboardMode, FinderEntry,
    FinderFlags, FinderGoToState, FinderLocation, FinderMetrics, FinderMillerColumn,
    FinderPreviewData, FinderRenameState, FinderSidebarItem, FinderSortKey, FinderSortOrder,
    FinderTab, FinderTag, FinderTreeRow, FinderUndoAction, FinderViewMode, format_file_size,
    parent_path_of, stem_selection_end, tag_mask_toggle,
};
use super::port::{FinderItemStatus, FinderListing, FinderListingStatus, FinderPort};

#[derive(Clone, Debug)]
pub struct FinderState {
    pub tabs: Vec<FinderTab>,
    pub active_tab: usize,
    pub next_tab_id: u64,
    pub sidebar_items: Vec<FinderSidebarItem>,
    pub flags: FinderFlags,
    pub metrics: FinderMetrics,
    pub tags: HashMap<String, u8>,
    pub rename: Option<FinderRenameState>,
    pub go_to: FinderGoToState,
    pub preview: FinderPreviewData,
    pub undo_stack: Vec<FinderUndoAction>,
    pub pending_perm_delete: Vec<String>,
    pub type_ahead: String,
    pub type_ahead_tick: u64,
    pub status_notice: Option<String>,
    pub last_clipboard_mode: FinderClipboardMode,
}

impl FinderState {
    #[must_use]
    pub fn new(default_location: FinderLocation, sidebar_items: Vec<FinderSidebarItem>) -> Self {
        Self {
            tabs: vec![FinderTab::new(1, default_location, FinderViewMode::List)],
            active_tab: 0,
            next_tab_id: 2,
            sidebar_items,
            flags: FinderFlags::default(),
            metrics: FinderMetrics::DEFAULT,
            tags: HashMap::new(),
            rename: None,
            go_to: FinderGoToState::default(),
            preview: FinderPreviewData::default(),
            undo_stack: Vec::new(),
            pending_perm_delete: Vec::new(),
            type_ahead: String::new(),
            type_ahead_tick: 0,
            status_notice: None,
            last_clipboard_mode: FinderClipboardMode::Copy,
        }
    }

    #[must_use]
    pub fn tab(&self) -> &FinderTab {
        &self.tabs[self.active_tab.min(self.tabs.len().saturating_sub(1))]
    }

    pub fn tab_mut(&mut self) -> &mut FinderTab {
        let index = self.active_tab.min(self.tabs.len().saturating_sub(1));
        &mut self.tabs[index]
    }

    #[must_use]
    pub fn active_dir_path(&self) -> Option<String> {
        let tab = self.tab();
        if tab.view_mode == FinderViewMode::Column
            && let Some(col) = tab.miller_columns.last()
            && let Some(path) = col.location.fs_path()
        {
            return Some(path.to_owned());
        }
        tab.location.fs_path().map(str::to_owned)
    }

    #[must_use]
    pub fn selected_entries(&self) -> Vec<FinderEntry> {
        let tab = self.tab();
        let mut out = Vec::new();
        for path in &tab.selected_paths {
            if let Some(entry) = self.find_entry_by_path(path) {
                out.push(entry);
            }
        }
        out
    }

    #[must_use]
    pub fn primary_selected_entry(&self) -> Option<FinderEntry> {
        let path = self.tab().primary_selected_path()?;
        self.find_entry_by_path(path)
    }

    #[must_use]
    pub fn find_entry_by_path(&self, path: &str) -> Option<FinderEntry> {
        let tab = self.tab();
        if let Some(entry) = tab
            .entries
            .iter()
            .find(|candidate| candidate.path.eq_ignore_ascii_case(path))
        {
            return Some(entry.clone());
        }
        for children in tab.expanded_children.values() {
            if let Some(entry) = children
                .iter()
                .find(|candidate| candidate.path.eq_ignore_ascii_case(path))
            {
                return Some(entry.clone());
            }
        }
        for col in &tab.miller_columns {
            if let Some(entry) = col
                .entries
                .iter()
                .find(|candidate| candidate.path.eq_ignore_ascii_case(path))
            {
                return Some(entry.clone());
            }
        }
        None
    }

    #[must_use]
    pub fn visible_list_rows(&self) -> Vec<FinderTreeRow> {
        let tab = self.tab();
        let mut rows = Vec::with_capacity(tab.entries.len());
        append_tree_rows(&tab.entries, &tab.expanded_children, 0, &mut rows);
        rows
    }

    #[must_use]
    pub fn visible_ordered_paths(&self) -> Vec<String> {
        let tab = self.tab();
        match tab.view_mode {
            FinderViewMode::List => self
                .visible_list_rows()
                .into_iter()
                .map(|row| row.entry.path)
                .collect(),
            FinderViewMode::Column => tab
                .miller_columns
                .last()
                .map(|col| col.entries.iter().map(|entry| entry.path.clone()).collect())
                .unwrap_or_default(),
            FinderViewMode::Icon | FinderViewMode::Gallery => {
                tab.entries.iter().map(|entry| entry.path.clone()).collect()
            }
        }
    }

    #[must_use]
    pub fn status_bar_label(&self) -> String {
        if let Some(notice) = &self.status_notice {
            return notice.clone();
        }
        let tab = self.tab();
        match tab.listing_status {
            FinderListingStatus::NotFound => return String::from("Folder not found"),
            FinderListingStatus::AccessDenied => return String::from("Access denied"),
            FinderListingStatus::Unavailable => return String::from("Location unavailable"),
            FinderListingStatus::Ok | FinderListingStatus::Partial { .. } => {}
        }
        let total = tab.entries.len();
        let selected_count = tab.selected_paths.len();
        let items_part = if selected_count > 0 {
            let selected_bytes: u64 = self
                .selected_entries()
                .into_iter()
                .filter(|entry| !entry.is_dir())
                .map(|entry| entry.size_bytes)
                .sum();
            if selected_bytes > 0 {
                format!(
                    "{selected_count} of {total} selected ({})",
                    format_file_size(selected_bytes)
                )
            } else {
                format!("{selected_count} of {total} selected")
            }
        } else if total == 1 {
            String::from("1 item")
        } else {
            format!("{total} items")
        };
        let mut label = if tab.free_space_label.is_empty() {
            items_part
        } else {
            format!("{items_part}, {}", tab.free_space_label)
        };
        if let FinderListingStatus::Partial { skipped_entries } = tab.listing_status
            && skipped_entries > 0
        {
            label = format!("{label}, {skipped_entries} item(s) unreadable");
        }
        label
    }

    pub fn reload_active_tab<P: FinderPort>(&mut self, port: &P) {
        let location = self.tab().location.clone();
        let query = self.tab().search_query.clone();
        let recursive = self.flags.has(FinderFlags::SEARCH_RECURSIVE) && !query.trim().is_empty();
        let listing = fetch_listing(
            port,
            &location,
            &self.tags,
            recursive.then_some(query.as_str()),
        );
        self.apply_listing(listing);
        if self.tab().view_mode == FinderViewMode::Column {
            self.rebuild_miller_columns(port);
        }
        self.refresh_preview(port);
    }

    pub fn apply_listing(&mut self, listing: FinderListing) {
        let show_hidden = self.flags.has(FinderFlags::SHOW_HIDDEN);
        let folders_on_top = self.flags.has(FinderFlags::FOLDERS_ON_TOP);
        let tab_index = self.active_tab.min(self.tabs.len().saturating_sub(1));
        let tags = &self.tags;
        let tab = &mut self.tabs[tab_index];
        let query = tab.search_query.trim().to_ascii_lowercase();
        let mut entries = listing.entries;
        for entry in &mut entries {
            entry.tag_mask = tags
                .get(&entry.path.to_ascii_lowercase())
                .copied()
                .unwrap_or(0);
        }
        entries.retain(|entry| {
            if !show_hidden && entry.is_hidden() {
                return false;
            }
            if !query.is_empty() && !entry.name.to_ascii_lowercase().contains(&query) {
                return false;
            }
            true
        });
        sort_entries(&mut entries, tab.sort_key, tab.sort_order, folders_on_top);
        tab.title = tab.location.title();
        tab.entries = entries;
        tab.free_space_label = listing.free_space_label;
        tab.listing_status = listing.status;
        tab.selected_paths.retain(|path| {
            tab.entries
                .iter()
                .any(|entry| entry.path.eq_ignore_ascii_case(path))
                || tab.expanded_children.values().any(|list| {
                    list.iter()
                        .any(|entry| entry.path.eq_ignore_ascii_case(path))
                })
        });
    }

    pub fn navigate<P: FinderPort>(&mut self, port: &P, location: FinderLocation) {
        let current = self.tab().location.clone();
        if current != location {
            let tab = self.tab_mut();
            tab.history_back.push(current);
            if tab.history_back.len() > 64 {
                tab.history_back.remove(0);
            }
            tab.history_forward.clear();
            tab.location = location;
            tab.scroll_y = 0;
            tab.scroll_x = 0;
            tab.selected_paths.clear();
            tab.anchor_path = None;
            tab.expanded_children.clear();
            tab.miller_columns.clear();
        }
        self.rename = None;
        self.flags.set(FinderFlags::CONFIRM_PERM_DELETE, false);
        self.pending_perm_delete.clear();
        self.status_notice = None;
        self.reload_active_tab(port);
    }

    pub fn go_back<P: FinderPort>(&mut self, port: &P) -> bool {
        let Some(prev) = self.tab_mut().history_back.pop() else {
            return false;
        };
        let current = self.tab().location.clone();
        let tab = self.tab_mut();
        tab.history_forward.push(current);
        tab.location = prev;
        tab.scroll_y = 0;
        tab.scroll_x = 0;
        tab.selected_paths.clear();
        tab.expanded_children.clear();
        tab.miller_columns.clear();
        self.rename = None;
        self.flags.set(FinderFlags::CONFIRM_PERM_DELETE, false);
        self.pending_perm_delete.clear();
        self.reload_active_tab(port);
        true
    }

    pub fn go_forward<P: FinderPort>(&mut self, port: &P) -> bool {
        let Some(next) = self.tab_mut().history_forward.pop() else {
            return false;
        };
        let current = self.tab().location.clone();
        let tab = self.tab_mut();
        tab.history_back.push(current);
        tab.location = next;
        tab.scroll_y = 0;
        tab.scroll_x = 0;
        tab.selected_paths.clear();
        tab.expanded_children.clear();
        tab.miller_columns.clear();
        self.rename = None;
        self.flags.set(FinderFlags::CONFIRM_PERM_DELETE, false);
        self.pending_perm_delete.clear();
        self.reload_active_tab(port);
        true
    }

    pub fn go_enclosing_folder<P: FinderPort>(&mut self, port: &P) -> bool {
        let target = match &self.tab().location {
            FinderLocation::Path(path) => {
                parent_path_of(path).map_or(FinderLocation::ThisPc, FinderLocation::Path)
            }
            _ => return false,
        };
        self.navigate(port, target);
        true
    }

    pub fn set_view_mode<P: FinderPort>(&mut self, port: &P, mode: FinderViewMode) {
        if self.tab().view_mode == mode {
            return;
        }
        let tab = self.tab_mut();
        tab.view_mode = mode;
        tab.scroll_y = 0;
        tab.scroll_x = 0;
        if mode == FinderViewMode::Column {
            self.rebuild_miller_columns(port);
        }
        if mode == FinderViewMode::Gallery
            && self.tab().selected_paths.is_empty()
            && let Some(first) = self.tab().entries.first().map(|entry| entry.path.clone())
        {
            self.tab_mut().selected_paths = vec![first];
        }
        self.refresh_preview(port);
    }

    pub fn set_sort<P: FinderPort>(&mut self, port: &P, key: FinderSortKey) {
        let tab = self.tab_mut();
        if tab.sort_key == key {
            tab.sort_order = tab.sort_order.toggled();
        } else {
            tab.sort_key = key;
            tab.sort_order = FinderSortOrder::Ascending;
        }
        self.reload_active_tab(port);
    }

    pub fn toggle_folder_expand<P: FinderPort>(&mut self, port: &P, path: &str) {
        let key = path.to_ascii_lowercase();
        if self.tab_mut().expanded_children.remove(&key).is_some() {
            return;
        }
        let loc = FinderLocation::Path(path.to_owned());
        let listing = fetch_listing(port, &loc, &self.tags, None);
        let show_hidden = self.flags.has(FinderFlags::SHOW_HIDDEN);
        let folders_on_top = self.flags.has(FinderFlags::FOLDERS_ON_TOP);
        let sort_key = self.tab().sort_key;
        let sort_order = self.tab().sort_order;
        let mut children = listing.entries;
        for entry in &mut children {
            entry.tag_mask = self
                .tags
                .get(&entry.path.to_ascii_lowercase())
                .copied()
                .unwrap_or(0);
        }
        children.retain(|entry| show_hidden || !entry.is_hidden());
        sort_entries(&mut children, sort_key, sort_order, folders_on_top);
        self.tab_mut().expanded_children.insert(key, children);
    }

    pub fn rebuild_miller_columns<P: FinderPort>(&mut self, port: &P) {
        let tab = self.tab();
        let root_col = FinderMillerColumn {
            location: tab.location.clone(),
            title: tab.title.clone(),
            entries: tab.entries.clone(),
            selected_path: tab.selected_paths.last().cloned(),
            scroll_y: 0,
        };
        let selected_dir = root_col.selected_path.as_ref().and_then(|sel| {
            root_col
                .entries
                .iter()
                .find(|entry| entry.path.eq_ignore_ascii_case(sel) && entry.is_dir())
                .map(|entry| entry.path.clone())
        });
        let mut columns = vec![root_col];
        if let Some(dir_path) = selected_dir {
            columns.push(self.load_miller_column(port, &dir_path));
        }
        self.tab_mut().miller_columns = columns;
    }

    pub fn select_miller_item<P: FinderPort>(
        &mut self,
        port: &P,
        col_index: usize,
        path: &str,
        multi: bool,
    ) {
        let Some(col) = self.tab_mut().miller_columns.get_mut(col_index) else {
            return;
        };
        col.selected_path = Some(path.to_owned());
        let entry = col
            .entries
            .iter()
            .find(|candidate| candidate.path.eq_ignore_ascii_case(path))
            .cloned();
        self.tab_mut().miller_columns.truncate(col_index + 1);
        if multi {
            toggle_selection(&mut self.tab_mut().selected_paths, path);
        } else {
            self.tab_mut().selected_paths = vec![path.to_owned()];
            self.tab_mut().anchor_path = Some(path.to_owned());
        }
        if let Some(entry) = entry
            && entry.is_dir()
        {
            let next_col = self.load_miller_column(port, &entry.path);
            self.tab_mut().miller_columns.push(next_col);
        }
        self.refresh_preview(port);
    }

    fn load_miller_column<P: FinderPort>(&self, port: &P, dir_path: &str) -> FinderMillerColumn {
        let loc = FinderLocation::Path(dir_path.to_owned());
        let listing = fetch_listing(port, &loc, &self.tags, None);
        let show_hidden = self.flags.has(FinderFlags::SHOW_HIDDEN);
        let folders_on_top = self.flags.has(FinderFlags::FOLDERS_ON_TOP);
        let tab = self.tab();
        let mut entries = listing.entries;
        for entry in &mut entries {
            entry.tag_mask = self
                .tags
                .get(&entry.path.to_ascii_lowercase())
                .copied()
                .unwrap_or(0);
        }
        entries.retain(|entry| show_hidden || !entry.is_hidden());
        sort_entries(&mut entries, tab.sort_key, tab.sort_order, folders_on_top);
        FinderMillerColumn {
            title: loc.title(),
            location: loc,
            entries,
            selected_path: None,
            scroll_y: 0,
        }
    }

    pub fn select_path<P: FinderPort>(&mut self, port: &P, path: &str, ctrl: bool, shift: bool) {
        if shift && let Some(anchor) = self.tab().anchor_path.clone() {
            let ordered = self.visible_ordered_paths();
            let a = ordered.iter().position(|p| p.eq_ignore_ascii_case(&anchor));
            let b = ordered.iter().position(|p| p.eq_ignore_ascii_case(path));
            if let (Some(i), Some(j)) = (a, b) {
                let (lo, hi) = if i <= j { (i, j) } else { (j, i) };
                self.tab_mut().selected_paths = ordered[lo..=hi].to_vec();
                self.refresh_preview(port);
                return;
            }
        }
        if ctrl {
            toggle_selection(&mut self.tab_mut().selected_paths, path);
        } else {
            self.tab_mut().selected_paths = vec![path.to_owned()];
        }
        self.tab_mut().anchor_path = Some(path.to_owned());
        self.refresh_preview(port);
    }

    pub fn select_all<P: FinderPort>(&mut self, port: &P) {
        let ordered = self.visible_ordered_paths();
        self.tab_mut().selected_paths = ordered;
        self.refresh_preview(port);
    }

    pub fn step_selection<P: FinderPort>(&mut self, port: &P, delta: isize, extend: bool) {
        let ordered = self.visible_ordered_paths();
        if ordered.is_empty() {
            return;
        }
        let current_idx = self
            .tab()
            .primary_selected_path()
            .and_then(|sel| ordered.iter().position(|p| p.eq_ignore_ascii_case(sel)));
        let next_idx = match current_idx {
            Some(idx) => {
                let target = idx as isize + delta;
                target.clamp(0, (ordered.len() - 1) as isize) as usize
            }
            None if delta >= 0 => 0,
            None => ordered.len() - 1,
        };
        let target_path = ordered[next_idx].clone();
        if self.tab().view_mode == FinderViewMode::Column {
            let last_col = self.tab().miller_columns.len().saturating_sub(1);
            self.select_miller_item(port, last_col, &target_path, false);
        } else {
            self.select_path(port, &target_path, false, extend);
        }
    }

    pub fn open_tab<P: FinderPort>(&mut self, port: &P, location: FinderLocation) {
        if self.tabs.len() >= FINDER_MAX_TABS {
            self.navigate(port, location);
            return;
        }
        let mode = self.tab().view_mode;
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(FinderTab::new(id, location, mode));
        self.active_tab = self.tabs.len() - 1;
        self.reload_active_tab(port);
    }

    pub fn close_tab<P: FinderPort>(&mut self, port: &P, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.rename = None;
        self.refresh_preview(port);
        true
    }

    pub fn select_tab<P: FinderPort>(&mut self, port: &P, index: usize) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }
        self.active_tab = index;
        self.rename = None;
        self.refresh_preview(port);
    }

    pub fn cycle_tab<P: FinderPort>(&mut self, port: &P, forward: bool) {
        let len = self.tabs.len();
        if len <= 1 {
            return;
        }
        let next = if forward {
            (self.active_tab + 1) % len
        } else {
            (self.active_tab + len - 1) % len
        };
        self.select_tab(port, next);
    }

    pub fn begin_rename(&mut self) -> bool {
        let Some(entry) = self.primary_selected_entry() else {
            return false;
        };
        if entry.kind == super::model::FinderEntryKind::Drive {
            return false;
        }
        let select_end = stem_selection_end(&entry.name, entry.is_dir());
        self.rename = Some(FinderRenameState {
            path: entry.path,
            draft: entry.name,
            select_end,
            error: None,
        });
        true
    }

    pub fn commit_rename<P: FinderPort>(&mut self, port: &P) -> bool {
        let Some(state) = self.rename.take() else {
            return false;
        };
        let trimmed = state.draft.trim();
        if trimmed.is_empty() || trimmed.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
            self.rename = Some(FinderRenameState {
                error: Some(String::from("Invalid filename")),
                ..state
            });
            return false;
        }
        let Some(new_path) = port.rename_path(&state.path, trimmed) else {
            self.rename = Some(FinderRenameState {
                error: Some(String::from("Rename failed")),
                ..state
            });
            return false;
        };
        if let Some(mask) = self.tags.remove(&state.path.to_ascii_lowercase()) {
            self.tags.insert(new_path.to_ascii_lowercase(), mask);
            let _ = port.save_tags(&self.tags);
        }
        self.push_undo(FinderUndoAction::Rename {
            from: state.path,
            to: new_path.clone(),
        });
        self.tab_mut().selected_paths = vec![new_path];
        self.reload_active_tab(port);
        true
    }

    pub fn toggle_tag_on_selection<P: FinderPort>(&mut self, port: &P, tag: FinderTag) {
        let paths = self.tab().selected_paths.clone();
        if paths.is_empty() {
            return;
        }
        for path in paths {
            let key = path.to_ascii_lowercase();
            let current = self.tags.get(&key).copied().unwrap_or(0);
            let next = tag_mask_toggle(current, tag);
            if next == 0 {
                self.tags.remove(&key);
            } else {
                self.tags.insert(key, next);
            }
        }
        let _ = port.save_tags(&self.tags);
        self.reload_active_tab(port);
    }

    pub fn apply_type_ahead<P: FinderPort>(&mut self, port: &P, ch: char, now_ms: u64) {
        if now_ms.saturating_sub(self.type_ahead_tick) > 900 {
            self.type_ahead.clear();
        }
        self.type_ahead_tick = now_ms;
        self.type_ahead.push(ch.to_ascii_lowercase());
        let prefix = self.type_ahead.clone();
        let found = self
            .tab()
            .entries
            .iter()
            .find(|entry| entry.name.to_ascii_lowercase().starts_with(&prefix))
            .map(|entry| entry.path.clone());
        if let Some(path) = found {
            self.select_path(port, &path, false, false);
        }
    }

    pub fn push_undo(&mut self, action: FinderUndoAction) {
        self.undo_stack.push(action);
        if self.undo_stack.len() > FINDER_MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo<P: FinderPort>(&mut self, port: &P) -> bool {
        let Some(action) = self.undo_stack.pop() else {
            return false;
        };
        match action {
            FinderUndoAction::Rename { from, to } => {
                let old_name = super::model::path_display_name(&from);
                if let Some(restored) = port.rename_path(&to, &old_name) {
                    if let Some(mask) = self.tags.remove(&to.to_ascii_lowercase()) {
                        self.tags.insert(restored.to_ascii_lowercase(), mask);
                        let _ = port.save_tags(&self.tags);
                    }
                    self.status_notice = None;
                    self.reload_active_tab(port);
                    self.tab_mut().selected_paths = vec![restored];
                    true
                } else {
                    self.undo_stack.push(FinderUndoAction::Rename { from, to });
                    self.status_notice = Some(String::from("Could not undo rename"));
                    false
                }
            }
            FinderUndoAction::Move { moves } => {
                let pairs: Vec<(String, String)> = moves
                    .iter()
                    .rev()
                    .map(|(src, dst)| (dst.clone(), src.clone()))
                    .collect();
                let outcome = port.move_exact(&pairs);
                let mut remaining = Vec::new();
                for ((src, dst), item) in moves.iter().rev().zip(outcome.items.iter()) {
                    if item.status != FinderItemStatus::Completed {
                        remaining.push((src.clone(), dst.clone()));
                    }
                }
                remaining.reverse();
                if !remaining.is_empty() {
                    self.undo_stack
                        .push(FinderUndoAction::Move { moves: remaining });
                }
                if outcome.any_mutated() {
                    self.reload_active_tab(port);
                }
                self.status_notice = outcome.summary_notice("Undo move");
                outcome.all_completed()
            }
            FinderUndoAction::NewFolder { path } => {
                let outcome = port.recycle_paths(std::slice::from_ref(&path));
                if !outcome.all_completed() {
                    self.undo_stack.push(FinderUndoAction::NewFolder { path });
                }
                if outcome.any_mutated() {
                    self.reload_active_tab(port);
                }
                self.status_notice = outcome.summary_notice("Undo new folder");
                outcome.all_completed()
            }
        }
    }

    pub fn refresh_preview<P: FinderPort>(&mut self, port: &P) {
        let needs_preview = self.flags.has(FinderFlags::QUICK_LOOK)
            || self.flags.has(FinderFlags::INSPECTOR)
            || matches!(
                self.tab().view_mode,
                FinderViewMode::Column | FinderViewMode::Gallery
            );
        if !needs_preview {
            return;
        }
        if let Some(entry) = self.primary_selected_entry() {
            if self.preview.path.eq_ignore_ascii_case(&entry.path) {
                return;
            }
            self.preview = port.load_preview(&entry);
        } else {
            self.preview = FinderPreviewData::default();
        }
    }

    pub fn set_sidebar_width(&mut self, width_points: i32) {
        self.metrics.sidebar_width =
            width_points.clamp(FINDER_SIDEBAR_MIN_POINTS, FINDER_SIDEBAR_MAX_POINTS);
    }

    pub fn set_icon_size(&mut self, size_points: i32) {
        self.metrics.icon_size = size_points.clamp(FINDER_ICON_SIZE_MIN, FINDER_ICON_SIZE_MAX);
    }
}

fn fetch_listing<P: FinderPort>(
    port: &P,
    location: &FinderLocation,
    tags: &HashMap<String, u8>,
    recursive_query: Option<&str>,
) -> FinderListing {
    match location {
        FinderLocation::Tag(tag) => port.list_tagged(tag.bit(), tags),
        _ => port.list_location(location, recursive_query),
    }
}

fn append_tree_rows(
    entries: &[FinderEntry],
    expanded: &HashMap<String, Vec<FinderEntry>>,
    depth: u16,
    out: &mut Vec<FinderTreeRow>,
) {
    for entry in entries {
        let key = entry.path.to_ascii_lowercase();
        let children = if entry.is_dir() && depth < 6 {
            expanded.get(&key)
        } else {
            None
        };
        out.push(FinderTreeRow {
            entry: entry.clone(),
            depth,
            expanded: children.is_some(),
        });
        if let Some(list) = children {
            append_tree_rows(list, expanded, depth + 1, out);
        }
    }
}

fn toggle_selection(selected: &mut Vec<String>, path: &str) {
    if let Some(pos) = selected.iter().position(|p| p.eq_ignore_ascii_case(path)) {
        selected.remove(pos);
    } else {
        selected.push(path.to_owned());
    }
}
