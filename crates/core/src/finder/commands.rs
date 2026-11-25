use crate::menu::MenuRow;

use super::model::{
    FinderClipboardMode, FinderFlags, FinderLocation, FinderSortKey, FinderTag, FinderUndoAction,
    FinderViewMode,
};
use super::port::FinderPort;
use super::state::FinderState;

pub const CMD_FINDER_OPEN: usize = 6001;
pub const CMD_FINDER_QUICK_LOOK: usize = 6002;
pub const CMD_FINDER_OPEN_NEW_TAB: usize = 6003;
pub const CMD_FINDER_GET_INFO: usize = 6004;
pub const CMD_FINDER_RENAME: usize = 6005;
pub const CMD_FINDER_DUPLICATE: usize = 6006;
pub const CMD_FINDER_COPY: usize = 6007;
pub const CMD_FINDER_CUT: usize = 6008;
pub const CMD_FINDER_PASTE: usize = 6009;
pub const CMD_FINDER_MOVE_HERE: usize = 6010;
pub const CMD_FINDER_COPY_PATH: usize = 6011;
pub const CMD_FINDER_NEW_FOLDER: usize = 6012;
pub const CMD_FINDER_TRASH: usize = 6013;
pub const CMD_FINDER_OPEN_TERMINAL: usize = 6015;
pub const CMD_FINDER_TOGGLE_HIDDEN: usize = 6016;
pub const CMD_FINDER_TOGGLE_PATH_BAR: usize = 6017;
pub const CMD_FINDER_TOGGLE_STATUS_BAR: usize = 6018;
pub const CMD_FINDER_TOGGLE_FOLDERS_TOP: usize = 6019;
pub const CMD_FINDER_SORT_NAME: usize = 6020;
pub const CMD_FINDER_SORT_DATE: usize = 6021;
pub const CMD_FINDER_SORT_SIZE: usize = 6022;
pub const CMD_FINDER_SORT_KIND: usize = 6023;
pub const CMD_FINDER_SORT_TAGS: usize = 6024;
pub const CMD_FINDER_TAG_BASE: usize = 6030;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FinderKeyMods(u8);

impl FinderKeyMods {
    pub const CTRL: u8 = 1 << 0;
    pub const SHIFT: u8 = 1 << 1;
    pub const ALT: u8 = 1 << 2;

    #[must_use]
    pub const fn new(ctrl: bool, shift: bool, alt: bool) -> Self {
        let mut bits = 0;
        if ctrl {
            bits |= Self::CTRL;
        }
        if shift {
            bits |= Self::SHIFT;
        }
        if alt {
            bits |= Self::ALT;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn ctrl(self) -> bool {
        (self.0 & Self::CTRL) != 0
    }

    #[must_use]
    pub const fn shift(self) -> bool {
        (self.0 & Self::SHIFT) != 0
    }

    #[must_use]
    pub const fn alt(self) -> bool {
        (self.0 & Self::ALT) != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderKey {
    Enter,
    Escape,
    Space,
    Backspace,
    Delete,
    Tab,
    F2,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Char(char),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinderKeyOutcome {
    Ignored,
    Handled,
    CloseWindow,
}

#[must_use]
pub fn handle_finder_key<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
    mods: FinderKeyMods,
    now_ms: u64,
) -> FinderKeyOutcome {
    if state.flags.has(FinderFlags::GO_TO_FOLDER) {
        return handle_go_to_key(state, port, key);
    }
    if state.rename.is_some() {
        return handle_rename_key(state, port, key);
    }
    if state.flags.has(FinderFlags::SEARCH_FOCUSED) {
        return handle_search_key(state, port, key);
    }
    if state.flags.has(FinderFlags::QUICK_LOOK) {
        return handle_quick_look_key(state, port, key);
    }
    if mods.ctrl() {
        return handle_ctrl_shortcut(state, port, key, mods);
    }
    if mods.alt() {
        return match key {
            FinderKey::Left => {
                let _ = state.go_back(port);
                FinderKeyOutcome::Handled
            }
            FinderKey::Right => {
                let _ = state.go_forward(port);
                FinderKeyOutcome::Handled
            }
            _ => FinderKeyOutcome::Ignored,
        };
    }
    handle_plain_key(state, port, key, mods, now_ms)
}

fn handle_go_to_key<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
) -> FinderKeyOutcome {
    match key {
        FinderKey::Escape => {
            state.flags.set(FinderFlags::GO_TO_FOLDER, false);
            state.go_to.error = None;
            FinderKeyOutcome::Handled
        }
        FinderKey::Enter => {
            submit_go_to_folder(state, port);
            FinderKeyOutcome::Handled
        }
        FinderKey::Backspace => {
            state.go_to.input.pop();
            state.go_to.error = None;
            FinderKeyOutcome::Handled
        }
        FinderKey::Char(ch) if !ch.is_control() => {
            state.go_to.input.push(ch);
            state.go_to.error = None;
            FinderKeyOutcome::Handled
        }
        _ => FinderKeyOutcome::Handled,
    }
}

pub fn submit_go_to_folder<P: FinderPort>(state: &mut FinderState, port: &P) {
    let raw = state.go_to.input.trim().to_owned();
    if raw.is_empty() {
        state.go_to.error = Some(String::from("Enter a folder path"));
        return;
    }
    if let Some(resolved) = port.path_exists_dir(&raw) {
        state.flags.set(FinderFlags::GO_TO_FOLDER, false);
        state.go_to.error = None;
        state.navigate(port, FinderLocation::Path(resolved));
    } else {
        state.go_to.error = Some(String::from("Folder not found"));
    }
}

fn handle_rename_key<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
) -> FinderKeyOutcome {
    match key {
        FinderKey::Escape => {
            state.rename = None;
            FinderKeyOutcome::Handled
        }
        FinderKey::Enter => {
            let _ = state.commit_rename(port);
            FinderKeyOutcome::Handled
        }
        FinderKey::Backspace => {
            if let Some(rename) = state.rename.as_mut() {
                if rename.select_end > 0 {
                    let tail: String = rename.draft.chars().skip(rename.select_end).collect();
                    rename.draft = tail;
                    rename.select_end = 0;
                } else {
                    rename.draft.pop();
                }
                rename.error = None;
            }
            FinderKeyOutcome::Handled
        }
        FinderKey::Char(ch) if !ch.is_control() => {
            if let Some(rename) = state.rename.as_mut() {
                if rename.select_end > 0 {
                    let tail: String = rename.draft.chars().skip(rename.select_end).collect();
                    rename.draft = format!("{ch}{tail}");
                    rename.select_end = 0;
                } else {
                    rename.draft.push(ch);
                }
                rename.error = None;
            }
            FinderKeyOutcome::Handled
        }
        _ => FinderKeyOutcome::Handled,
    }
}

fn handle_search_key<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
) -> FinderKeyOutcome {
    match key {
        FinderKey::Escape => {
            state.flags.set(FinderFlags::SEARCH_FOCUSED, false);
            if !state.tab().search_query.is_empty() {
                state.tab_mut().search_query.clear();
                state.reload_active_tab(port);
            }
            FinderKeyOutcome::Handled
        }
        FinderKey::Enter => {
            state.flags.set(FinderFlags::SEARCH_FOCUSED, false);
            state.reload_active_tab(port);
            FinderKeyOutcome::Handled
        }
        FinderKey::Backspace => {
            state.tab_mut().search_query.pop();
            state.reload_active_tab(port);
            FinderKeyOutcome::Handled
        }
        FinderKey::Char(ch) if !ch.is_control() => {
            state.tab_mut().search_query.push(ch);
            state.reload_active_tab(port);
            FinderKeyOutcome::Handled
        }
        _ => FinderKeyOutcome::Handled,
    }
}

fn handle_quick_look_key<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
) -> FinderKeyOutcome {
    match key {
        FinderKey::Space | FinderKey::Escape => {
            state.flags.set(FinderFlags::QUICK_LOOK, false);
            FinderKeyOutcome::Handled
        }
        FinderKey::Down | FinderKey::Right => {
            state.step_selection(port, 1, false);
            FinderKeyOutcome::Handled
        }
        FinderKey::Up | FinderKey::Left => {
            state.step_selection(port, -1, false);
            FinderKeyOutcome::Handled
        }
        FinderKey::Enter => {
            state.flags.set(FinderFlags::QUICK_LOOK, false);
            open_selected(state, port);
            FinderKeyOutcome::Handled
        }
        _ => FinderKeyOutcome::Handled,
    }
}

fn handle_ctrl_shortcut<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
    mods: FinderKeyMods,
) -> FinderKeyOutcome {
    match key {
        FinderKey::Char('1') => state.set_view_mode(port, FinderViewMode::Icon),
        FinderKey::Char('2') => state.set_view_mode(port, FinderViewMode::List),
        FinderKey::Char('3') => state.set_view_mode(port, FinderViewMode::Column),
        FinderKey::Char('4') => state.set_view_mode(port, FinderViewMode::Gallery),
        FinderKey::Char('o' | 'O') | FinderKey::Down => open_selected(state, port),
        FinderKey::Up => {
            let _ = state.go_enclosing_folder(port);
        }
        FinderKey::Char('[') => {
            let _ = state.go_back(port);
        }
        FinderKey::Char(']') => {
            let _ = state.go_forward(port);
        }
        FinderKey::Char('t' | 'T') => {
            let loc = state.tab().location.clone();
            state.open_tab(port, loc);
        }
        FinderKey::Char('w' | 'W') => {
            if !state.close_tab(port, state.active_tab) {
                return FinderKeyOutcome::CloseWindow;
            }
        }
        FinderKey::Tab => state.cycle_tab(port, !mods.shift()),
        FinderKey::Char('f' | 'F') => state.flags.set(FinderFlags::SEARCH_FOCUSED, true),
        FinderKey::Char('i' | 'I') => {
            state.flags.toggle(FinderFlags::INSPECTOR);
            state.refresh_preview(port);
        }
        FinderKey::Char('a' | 'A') => state.select_all(port),
        FinderKey::Char('c' | 'C') => copy_selected(state, port, FinderClipboardMode::Copy),
        FinderKey::Char('x' | 'X') => copy_selected(state, port, FinderClipboardMode::Cut),
        FinderKey::Char('v' | 'V') => paste_clipboard(state, port, mods.alt()),
        FinderKey::Char('d' | 'D') => duplicate_selected(state, port),
        FinderKey::Char('z' | 'Z') => {
            let _ = state.undo(port);
        }
        FinderKey::Char('n' | 'N') if mods.shift() => create_new_folder(state, port),
        FinderKey::Char('g' | 'G') if mods.shift() => {
            state.go_to.input = state.active_dir_path().unwrap_or_default();
            state.go_to.error = None;
            state.flags.set(FinderFlags::GO_TO_FOLDER, true);
        }
        FinderKey::Char('.') if mods.shift() => {
            state.flags.toggle(FinderFlags::SHOW_HIDDEN);
            state.reload_active_tab(port);
        }
        FinderKey::Char('s' | 'S') if mods.alt() => state.flags.toggle(FinderFlags::SIDEBAR),
        FinderKey::Char('p' | 'P') if mods.shift() => {
            state.flags.toggle(FinderFlags::PREVIEW_PANE);
            state.refresh_preview(port);
        }
        FinderKey::Backspace if mods.alt() => request_permanent_delete(state),
        FinderKey::Backspace => move_selected_to_trash(state, port),
        _ => return FinderKeyOutcome::Ignored,
    }
    FinderKeyOutcome::Handled
}

fn handle_plain_key<P: FinderPort>(
    state: &mut FinderState,
    port: &P,
    key: FinderKey,
    mods: FinderKeyMods,
    now_ms: u64,
) -> FinderKeyOutcome {
    match key {
        FinderKey::Enter | FinderKey::F2 => {
            let _ = state.begin_rename();
        }
        FinderKey::Space => {
            if state.primary_selected_entry().is_some() {
                state.flags.toggle(FinderFlags::QUICK_LOOK);
                state.refresh_preview(port);
            }
        }
        FinderKey::Delete if mods.shift() => request_permanent_delete(state),
        FinderKey::Delete => move_selected_to_trash(state, port),
        FinderKey::Backspace => {
            let _ = state.go_enclosing_folder(port);
        }
        FinderKey::Escape => {
            state.flags.set(FinderFlags::CONFIRM_PERM_DELETE, false);
            state.tab_mut().selected_paths.clear();
            state.status_notice = None;
        }
        FinderKey::Down => state.step_selection(port, 1, mods.shift()),
        FinderKey::Up => state.step_selection(port, -1, mods.shift()),
        FinderKey::Right => handle_right_arrow(state, port),
        FinderKey::Left => handle_left_arrow(state, port),
        FinderKey::Home => {
            if let Some(first) = state.visible_ordered_paths().first().cloned() {
                state.select_path(port, &first, false, mods.shift());
            }
        }
        FinderKey::End => {
            if let Some(last) = state.visible_ordered_paths().last().cloned() {
                state.select_path(port, &last, false, mods.shift());
            }
        }
        FinderKey::Char(ch) if ch.is_alphanumeric() => {
            state.apply_type_ahead(port, ch, now_ms);
        }
        _ => return FinderKeyOutcome::Ignored,
    }
    FinderKeyOutcome::Handled
}

fn handle_right_arrow<P: FinderPort>(state: &mut FinderState, port: &P) {
    match state.tab().view_mode {
        FinderViewMode::List => {
            if let Some(entry) = state.primary_selected_entry()
                && entry.is_dir()
                && !state
                    .tab()
                    .expanded_children
                    .contains_key(&entry.path.to_ascii_lowercase())
            {
                state.toggle_folder_expand(port, &entry.path);
            }
        }
        FinderViewMode::Column => {
            let cols = state.tab().miller_columns.len();
            if cols >= 2 {
                let target_col = cols - 1;
                if let Some(first) = state
                    .tab()
                    .miller_columns
                    .get(target_col)
                    .and_then(|col| col.entries.first())
                    .map(|entry| entry.path.clone())
                {
                    state.select_miller_item(port, target_col, &first, false);
                }
            }
        }
        FinderViewMode::Icon | FinderViewMode::Gallery => state.step_selection(port, 1, false),
    }
}

fn handle_left_arrow<P: FinderPort>(state: &mut FinderState, port: &P) {
    match state.tab().view_mode {
        FinderViewMode::List => {
            if let Some(entry) = state.primary_selected_entry()
                && state
                    .tab()
                    .expanded_children
                    .contains_key(&entry.path.to_ascii_lowercase())
            {
                state.toggle_folder_expand(port, &entry.path);
            }
        }
        FinderViewMode::Column => {
            let cols = state.tab().miller_columns.len();
            if cols >= 2 {
                state.tab_mut().miller_columns.pop();
                if let Some(sel) = state
                    .tab()
                    .miller_columns
                    .last()
                    .and_then(|col| col.selected_path.clone())
                {
                    state.tab_mut().selected_paths = vec![sel];
                    state.refresh_preview(port);
                }
            }
        }
        FinderViewMode::Icon | FinderViewMode::Gallery => state.step_selection(port, -1, false),
    }
}

pub fn open_selected<P: FinderPort>(state: &mut FinderState, port: &P) {
    let entries = state.selected_entries();
    if entries.is_empty() {
        return;
    }
    if entries.len() == 1 && entries[0].is_dir() {
        let next = FinderLocation::Path(entries[0].path.clone());
        state.navigate(port, next);
        return;
    }
    for entry in entries {
        if entry.is_dir() {
            state.open_tab(port, FinderLocation::Path(entry.path));
        } else {
            let _ = port.open_path(&entry.path);
        }
    }
}

pub fn create_new_folder<P: FinderPort>(state: &mut FinderState, port: &P) {
    let Some(dir) = state.active_dir_path() else {
        return;
    };
    let Some(created) = port.create_folder(&dir) else {
        return;
    };
    state.push_undo(FinderUndoAction::NewFolder {
        path: created.clone(),
    });
    state.reload_active_tab(port);
    state.tab_mut().selected_paths = vec![created];
    let _ = state.begin_rename();
}

pub fn duplicate_selected<P: FinderPort>(state: &mut FinderState, port: &P) {
    let paths = state.tab().selected_paths.clone();
    if paths.is_empty() {
        return;
    }
    let created = port.duplicate_paths(&paths);
    if !created.is_empty() {
        state.reload_active_tab(port);
        state.tab_mut().selected_paths = created;
        state.refresh_preview(port);
    }
}

pub fn copy_selected<P: FinderPort>(state: &mut FinderState, port: &P, mode: FinderClipboardMode) {
    let paths = state.tab().selected_paths.clone();
    if paths.is_empty() {
        return;
    }
    if port.copy_paths_to_clipboard(&paths, mode) {
        state.last_clipboard_mode = mode;
        let verb = if mode == FinderClipboardMode::Cut {
            "Cut"
        } else {
            "Copied"
        };
        state.status_notice = Some(format!("{verb} {} item(s)", paths.len()));
    }
}

pub fn paste_clipboard<P: FinderPort>(state: &mut FinderState, port: &P, force_move: bool) {
    let Some(dest) = state.active_dir_path() else {
        return;
    };
    let Some(clip) = port.read_clipboard_paths() else {
        return;
    };
    let move_items = force_move || clip.mode == FinderClipboardMode::Cut;
    let moved = port.paste_paths(&clip.paths, &dest, move_items);
    if !moved.is_empty() {
        let new_selection: Vec<String> = moved.iter().map(|(_, dst)| dst.clone()).collect();
        if move_items {
            state.push_undo(FinderUndoAction::Move { moves: moved });
        }
        state.reload_active_tab(port);
        state.tab_mut().selected_paths = new_selection;
        state.status_notice = None;
        state.refresh_preview(port);
    }
}

pub fn move_selected_to_trash<P: FinderPort>(state: &mut FinderState, port: &P) {
    let paths = state.tab().selected_paths.clone();
    if paths.is_empty() {
        return;
    }
    if port.recycle_paths(&paths) {
        state.tab_mut().selected_paths.clear();
        state.reload_active_tab(port);
    }
}

pub fn request_permanent_delete(state: &mut FinderState) {
    if !state.tab().selected_paths.is_empty() {
        state.flags.set(FinderFlags::CONFIRM_PERM_DELETE, true);
    }
}

pub fn confirm_permanent_delete<P: FinderPort>(state: &mut FinderState, port: &P) {
    state.flags.set(FinderFlags::CONFIRM_PERM_DELETE, false);
    let paths = state.tab().selected_paths.clone();
    if paths.is_empty() {
        return;
    }
    if port.delete_paths_permanently(&paths) {
        state.tab_mut().selected_paths.clear();
        state.reload_active_tab(port);
    }
}

#[must_use]
pub fn build_finder_context_menu_rows(state: &FinderState, has_selection: bool) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    if has_selection {
        rows.push(MenuRow::item(CMD_FINDER_OPEN, "Open", "⌃O"));
        rows.push(MenuRow::item(CMD_FINDER_QUICK_LOOK, "Quick Look", "Space"));
        if state.primary_selected_entry().is_some_and(|e| e.is_dir()) {
            rows.push(MenuRow::item(
                CMD_FINDER_OPEN_NEW_TAB,
                "Open in New Tab",
                "",
            ));
        }
        rows.push(MenuRow::Separator);
        rows.push(MenuRow::item(CMD_FINDER_GET_INFO, "Get Info", "⌃I"));
        rows.push(MenuRow::item(CMD_FINDER_RENAME, "Rename", "⏎"));
        rows.push(MenuRow::item(CMD_FINDER_DUPLICATE, "Duplicate", "⌃D"));
        rows.push(MenuRow::Separator);
        rows.push(MenuRow::item(CMD_FINDER_COPY, "Copy", "⌃C"));
        rows.push(MenuRow::item(CMD_FINDER_CUT, "Cut", "⌃X"));
        rows.push(MenuRow::item(CMD_FINDER_COPY_PATH, "Copy Path", ""));
        rows.push(MenuRow::Separator);
        for (idx, tag) in FinderTag::ALL.into_iter().enumerate() {
            let active = state
                .primary_selected_entry()
                .is_some_and(|e| super::model::tag_mask_contains(e.tag_mask, tag));
            rows.push(MenuRow::check_item(
                CMD_FINDER_TAG_BASE + idx,
                format!("Tag: {}", tag.label()),
                active,
                true,
            ));
        }
        rows.push(MenuRow::Separator);
        rows.push(MenuRow::item(CMD_FINDER_TRASH, "Move to Trash", "Del"));
    } else {
        rows.push(MenuRow::item(CMD_FINDER_NEW_FOLDER, "New Folder", "⇧⌃N"));
        rows.push(MenuRow::item(CMD_FINDER_PASTE, "Paste Item", "⌃V"));
        rows.push(MenuRow::item(CMD_FINDER_MOVE_HERE, "Move Item Here", "⌥⌃V"));
    }
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::item(
        CMD_FINDER_OPEN_TERMINAL,
        "Open in Terminal",
        "",
    ));
    rows
}

#[must_use]
pub fn build_finder_sort_menu_rows(state: &FinderState) -> Vec<MenuRow> {
    let tab = state.tab();
    let pairs = [
        (FinderSortKey::Name, CMD_FINDER_SORT_NAME),
        (FinderSortKey::DateModified, CMD_FINDER_SORT_DATE),
        (FinderSortKey::Size, CMD_FINDER_SORT_SIZE),
        (FinderSortKey::Kind, CMD_FINDER_SORT_KIND),
        (FinderSortKey::Tags, CMD_FINDER_SORT_TAGS),
    ];
    let mut rows = Vec::new();
    for (key, cmd) in pairs {
        rows.push(MenuRow::check_item(
            cmd,
            key.label(),
            tab.sort_key == key,
            true,
        ));
    }
    rows.push(MenuRow::Separator);
    rows.push(MenuRow::check_item(
        CMD_FINDER_TOGGLE_FOLDERS_TOP,
        "Keep Folders on Top",
        state.flags.has(FinderFlags::FOLDERS_ON_TOP),
        true,
    ));
    rows
}

#[must_use]
pub fn build_finder_action_menu_rows(state: &FinderState) -> Vec<MenuRow> {
    vec![
        MenuRow::item(CMD_FINDER_NEW_FOLDER, "New Folder", "⇧⌃N"),
        MenuRow::Separator,
        MenuRow::check_item(
            CMD_FINDER_TOGGLE_HIDDEN,
            "Show Hidden Files",
            state.flags.has(FinderFlags::SHOW_HIDDEN),
            true,
        ),
        MenuRow::check_item(
            CMD_FINDER_TOGGLE_PATH_BAR,
            "Show Path Bar",
            state.flags.has(FinderFlags::PATH_BAR),
            true,
        ),
        MenuRow::check_item(
            CMD_FINDER_TOGGLE_STATUS_BAR,
            "Show Status Bar",
            state.flags.has(FinderFlags::STATUS_BAR),
            true,
        ),
        MenuRow::Separator,
        MenuRow::item(CMD_FINDER_OPEN_TERMINAL, "Open in Terminal", ""),
    ]
}

pub fn execute_finder_menu_command<P: FinderPort>(state: &mut FinderState, port: &P, cmd: usize) {
    if (CMD_FINDER_TAG_BASE..CMD_FINDER_TAG_BASE + FinderTag::ALL.len()).contains(&cmd) {
        let tag = FinderTag::ALL[cmd - CMD_FINDER_TAG_BASE];
        state.toggle_tag_on_selection(port, tag);
        return;
    }
    match cmd {
        CMD_FINDER_OPEN => open_selected(state, port),
        CMD_FINDER_QUICK_LOOK => {
            if state.primary_selected_entry().is_some() {
                state.flags.set(FinderFlags::QUICK_LOOK, true);
                state.refresh_preview(port);
            }
        }
        CMD_FINDER_OPEN_NEW_TAB => {
            if let Some(entry) = state.primary_selected_entry()
                && entry.is_dir()
            {
                state.open_tab(port, FinderLocation::Path(entry.path));
            }
        }
        CMD_FINDER_GET_INFO => {
            state.flags.set(FinderFlags::INSPECTOR, true);
            state.refresh_preview(port);
        }
        CMD_FINDER_RENAME => {
            let _ = state.begin_rename();
        }
        CMD_FINDER_DUPLICATE => duplicate_selected(state, port),
        CMD_FINDER_COPY => copy_selected(state, port, FinderClipboardMode::Copy),
        CMD_FINDER_CUT => copy_selected(state, port, FinderClipboardMode::Cut),
        CMD_FINDER_PASTE => paste_clipboard(state, port, false),
        CMD_FINDER_MOVE_HERE => paste_clipboard(state, port, true),
        CMD_FINDER_COPY_PATH => {
            if let Some(path) = state
                .tab()
                .primary_selected_path()
                .map(str::to_owned)
                .or_else(|| state.active_dir_path())
            {
                let _ = port.copy_text_to_clipboard(&path);
                state.status_notice = Some(String::from("Path copied"));
            }
        }
        CMD_FINDER_NEW_FOLDER => create_new_folder(state, port),
        CMD_FINDER_TRASH => move_selected_to_trash(state, port),
        CMD_FINDER_OPEN_TERMINAL => {
            if let Some(dir) = state.active_dir_path() {
                let _ = port.open_in_terminal(&dir);
            }
        }
        CMD_FINDER_TOGGLE_HIDDEN => {
            state.flags.toggle(FinderFlags::SHOW_HIDDEN);
            state.reload_active_tab(port);
        }
        CMD_FINDER_TOGGLE_PATH_BAR => state.flags.toggle(FinderFlags::PATH_BAR),
        CMD_FINDER_TOGGLE_STATUS_BAR => state.flags.toggle(FinderFlags::STATUS_BAR),
        CMD_FINDER_TOGGLE_FOLDERS_TOP => {
            state.flags.toggle(FinderFlags::FOLDERS_ON_TOP);
            state.reload_active_tab(port);
        }
        CMD_FINDER_SORT_NAME => state.set_sort(port, FinderSortKey::Name),
        CMD_FINDER_SORT_DATE => state.set_sort(port, FinderSortKey::DateModified),
        CMD_FINDER_SORT_SIZE => state.set_sort(port, FinderSortKey::Size),
        CMD_FINDER_SORT_KIND => state.set_sort(port, FinderSortKey::Kind),
        CMD_FINDER_SORT_TAGS => state.set_sort(port, FinderSortKey::Tags),
        _ => {}
    }
}
