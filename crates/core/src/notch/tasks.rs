use crate::notch::shelf::truncate_utf8_bytes;

pub const MAX_TASKS: usize = 50;
pub const TASKS_PAGE_SIZE: usize = 4;
pub const MAX_TASK_TEXT_BYTES: usize = 200;
const TASKS_FILE_HEADER: &str = "v1";
const FLAG_OPEN: &str = "0";
const FLAG_DONE: &str = "1";
const FLAG_ACTIVE: &str = "2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    pub id: TaskId,
    pub text: String,
    pub done: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAddOutcome {
    Added(TaskId),
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEditOutcome {
    Updated,
    Unchanged,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEditorMode {
    Add,
    Edit(TaskId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent tasks layout flags"
)]
pub struct TasksLayoutInput {
    pub visible_count: u8,
    pub has_pager: bool,
    pub has_items: bool,
    pub has_done: bool,
    pub editing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TasksState {
    pub items: Vec<TaskItem>,
    pub page: usize,
    pub next_id: u64,
    pub editor: Option<TaskEditorMode>,
}

#[must_use]
pub fn normalize_task_text(raw: &str) -> Option<String> {
    let mut collapsed = String::new();
    let mut pending_space = false;
    for ch in raw.chars() {
        if ch.is_control() || ch.is_whitespace() {
            pending_space = !collapsed.is_empty();
            continue;
        }
        if pending_space {
            collapsed.push(' ');
            pending_space = false;
        }
        collapsed.push(ch);
    }
    let bounded = truncate_utf8_bytes(&collapsed, MAX_TASK_TEXT_BYTES).trim_end();
    (!bounded.is_empty()).then(|| String::from(bounded))
}

impl TasksState {
    pub fn add(&mut self, raw: &str) -> TaskAddOutcome {
        let Some(text) = normalize_task_text(raw) else {
            return TaskAddOutcome::Rejected;
        };
        if self.items.len() >= MAX_TASKS {
            let Some(oldest_done) = self.items.iter().rposition(|item| item.done) else {
                return TaskAddOutcome::Rejected;
            };
            self.items.remove(oldest_done);
        }
        let id = self.allocate_id();
        self.items.insert(
            0,
            TaskItem {
                id,
                text,
                done: false,
                active: false,
            },
        );
        self.page = 0;
        TaskAddOutcome::Added(id)
    }

    pub fn edit(&mut self, id: TaskId, raw: &str) -> TaskEditOutcome {
        let Some(text) = normalize_task_text(raw) else {
            return TaskEditOutcome::Rejected;
        };
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return TaskEditOutcome::Rejected;
        };
        if item.text == text {
            return TaskEditOutcome::Unchanged;
        }
        item.text = text;
        TaskEditOutcome::Updated
    }

    pub fn begin_add(&mut self) -> bool {
        self.editor.replace(TaskEditorMode::Add) != Some(TaskEditorMode::Add)
    }

    pub fn begin_edit(&mut self, id: TaskId) -> Option<&str> {
        let index = self.items.iter().position(|item| item.id == id)?;
        self.editor = Some(TaskEditorMode::Edit(id));
        self.items.get(index).map(|item| item.text.as_str())
    }

    pub fn cancel_editor(&mut self) -> bool {
        self.editor.take().is_some()
    }

    #[must_use]
    pub const fn is_editing(&self) -> bool {
        self.editor.is_some()
    }

    #[must_use]
    pub const fn editing_id(&self) -> Option<TaskId> {
        match self.editor {
            Some(TaskEditorMode::Edit(id)) => Some(id),
            Some(TaskEditorMode::Add) | None => None,
        }
    }

    pub fn toggle_done(&mut self, id: TaskId) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return false;
        };
        item.done = !item.done;
        if item.done {
            item.active = false;
        }
        true
    }

    pub fn toggle_active(&mut self, id: TaskId) -> bool {
        let Some(target) = self.items.iter().find(|item| item.id == id) else {
            return false;
        };
        if target.done {
            return false;
        }
        let activate = !target.active;
        for item in &mut self.items {
            item.active = activate && item.id == id;
        }
        true
    }

    pub fn remove(&mut self, id: TaskId) -> bool {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        self.items.remove(index);
        if self.editor == Some(TaskEditorMode::Edit(id)) {
            self.editor = None;
        }
        self.clamp_page();
        true
    }

    pub fn clear_done(&mut self) -> bool {
        let before = self.items.len();
        self.items.retain(|item| !item.done);
        if self.items.len() == before {
            return false;
        }
        if let Some(TaskEditorMode::Edit(id)) = self.editor
            && self.items.iter().all(|item| item.id != id)
        {
            self.editor = None;
        }
        self.clamp_page();
        true
    }

    #[must_use]
    pub fn active_task(&self) -> Option<&TaskItem> {
        self.items.iter().find(|item| item.active)
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.items.len().div_ceil(TASKS_PAGE_SIZE).max(1)
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
    pub fn visible_items(&self) -> &[TaskItem] {
        let start = self.clamped_page() * TASKS_PAGE_SIZE;
        let end = (start + TASKS_PAGE_SIZE).min(self.items.len());
        self.items.get(start..end).unwrap_or(&[])
    }

    #[must_use]
    pub fn visible_item_at(&self, slot: u8) -> Option<&TaskItem> {
        self.visible_items().get(usize::from(slot))
    }

    #[must_use]
    pub fn layout_input(&self) -> TasksLayoutInput {
        TasksLayoutInput {
            visible_count: u8::try_from(self.visible_items().len()).unwrap_or(0),
            has_pager: self.page_count() > 1,
            has_items: !self.items.is_empty(),
            has_done: self.items.iter().any(|item| item.done),
            editing: self.editor.is_some(),
        }
    }

    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::from(TASKS_FILE_HEADER);
        out.push('\n');
        for item in self.items.iter().rev() {
            let flag = if item.done {
                FLAG_DONE
            } else if item.active {
                FLAG_ACTIVE
            } else {
                FLAG_OPEN
            };
            out.push_str(flag);
            out.push('\t');
            out.push_str(&item.text);
            out.push('\n');
        }
        out
    }

    #[must_use]
    pub fn decode(contents: &str) -> Self {
        let mut lines = contents.lines();
        let mut state = Self::default();
        if lines.next().map(str::trim) != Some(TASKS_FILE_HEADER) {
            return state;
        }
        for line in lines {
            let Some((flag, raw)) = line.split_once('\t') else {
                continue;
            };
            let Some(text) = normalize_task_text(raw) else {
                continue;
            };
            if !matches!(flag, FLAG_OPEN | FLAG_DONE | FLAG_ACTIVE) {
                continue;
            }
            let done = flag == FLAG_DONE;
            let active = flag == FLAG_ACTIVE && state.active_task().is_none();
            if state.items.len() >= MAX_TASKS {
                break;
            }
            let id = state.allocate_id();
            state.items.insert(
                0,
                TaskItem {
                    id,
                    text,
                    done,
                    active,
                },
            );
        }
        state
    }

    fn allocate_id(&mut self) -> TaskId {
        self.next_id = self.next_id.saturating_add(1);
        TaskId(self.next_id)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::{
        MAX_TASK_TEXT_BYTES, MAX_TASKS, TASKS_PAGE_SIZE, TaskAddOutcome, TaskEditOutcome,
        TaskEditorMode, TaskId, TasksState, normalize_task_text,
    };

    fn added(state: &mut TasksState, text: &str) -> TaskId {
        match state.add(text) {
            TaskAddOutcome::Added(id) => id,
            TaskAddOutcome::Rejected => panic!("task {text:?} was rejected"),
        }
    }

    #[test]
    fn normalization_collapses_whitespace_and_rejects_blank_text() {
        assert_eq!(
            normalize_task_text("  kirim\t invoice \r\n besok\0 "),
            Some(String::from("kirim invoice besok"))
        );
        assert_eq!(normalize_task_text(" \t\r\n\0"), None);
        assert_eq!(normalize_task_text(""), None);
    }

    #[test]
    fn normalization_truncates_on_a_utf8_boundary() {
        let long = "é".repeat(MAX_TASK_TEXT_BYTES);
        let text = normalize_task_text(&long).expect("text");
        assert!(text.len() <= MAX_TASK_TEXT_BYTES);
        assert!(text.chars().all(|ch| ch == 'é'));
        let emoji = "😀".repeat(MAX_TASK_TEXT_BYTES);
        let text = normalize_task_text(&emoji).expect("emoji");
        assert!(text.len() <= MAX_TASK_TEXT_BYTES);
        assert_eq!(text.len() % 4, 0);
    }

    #[test]
    fn new_tasks_go_first_and_reset_the_page() {
        let mut state = TasksState::default();
        for index in 0..6 {
            added(&mut state, &format!("task {index}"));
        }
        state.page = 1;
        added(&mut state, "latest");
        assert_eq!(state.items[0].text, "latest");
        assert_eq!(state.page, 0);
    }

    #[test]
    fn a_full_list_drops_the_oldest_done_task_then_rejects() {
        let mut state = TasksState::default();
        let first = added(&mut state, "first");
        for index in 1..MAX_TASKS {
            added(&mut state, &format!("task {index}"));
        }
        assert_eq!(state.items.len(), MAX_TASKS);
        assert_eq!(state.add("overflow"), TaskAddOutcome::Rejected);
        assert!(state.toggle_done(first));
        assert!(matches!(state.add("fresh"), TaskAddOutcome::Added(_)));
        assert_eq!(state.items.len(), MAX_TASKS);
        assert!(state.items.iter().all(|item| item.id != first));
        assert_eq!(state.items[0].text, "fresh");
    }

    #[test]
    fn at_most_one_task_is_active_and_done_tasks_cannot_be_active() {
        let mut state = TasksState::default();
        let a = added(&mut state, "a");
        let b = added(&mut state, "b");
        assert!(state.toggle_active(a));
        assert!(state.toggle_active(b));
        assert_eq!(state.items.iter().filter(|item| item.active).count(), 1);
        assert_eq!(state.active_task().map(|item| item.id), Some(b));
        assert!(state.toggle_done(b));
        assert!(state.active_task().is_none());
        assert!(!state.toggle_active(b));
        assert!(state.toggle_active(a));
        assert!(state.toggle_active(a));
        assert!(state.active_task().is_none());
    }

    #[test]
    fn removing_and_clearing_keep_the_page_in_range() {
        let mut state = TasksState::default();
        let mut ids = Vec::new();
        for index in 0..=TASKS_PAGE_SIZE {
            ids.push(added(&mut state, &format!("task {index}")));
        }
        assert_eq!(state.page_count(), 2);
        assert!(state.step_page(1));
        assert_eq!(state.visible_items().len(), 1);
        assert!(state.remove(state.visible_items()[0].id));
        assert_eq!(state.clamped_page(), 0);
        assert!(!state.remove(TaskId(9_999)));
        assert!(!state.clear_done());
        assert!(state.toggle_done(ids[2]));
        assert!(state.layout_input().has_done);
        assert!(state.clear_done());
        assert_eq!(state.items.len(), TASKS_PAGE_SIZE - 1);
        assert!(!state.layout_input().has_done);
    }

    #[test]
    fn encoding_round_trips_order_done_and_active() {
        let mut state = TasksState::default();
        let a = added(&mut state, "pertama");
        added(&mut state, "kedua 😀 é");
        let c = added(&mut state, "ketiga");
        state.toggle_done(a);
        state.toggle_active(c);
        let decoded = TasksState::decode(&state.encode());
        let summary = |s: &TasksState| {
            s.items
                .iter()
                .map(|item| (item.text.clone(), item.done, item.active))
                .collect::<Vec<_>>()
        };
        assert_eq!(summary(&decoded), summary(&state));
        assert_eq!(decoded.encode(), state.encode());
    }

    #[test]
    fn decoding_is_tolerant_and_never_trusts_the_file() {
        assert!(TasksState::decode("").items.is_empty());
        assert!(TasksState::decode("v2\n0\tx\n").items.is_empty());
        let messy = "v1\n0\tgood\nnotab\n9\tbad flag\n1\t   \n2\tfirst active\n2\tsecond active\n";
        let decoded = TasksState::decode(messy);
        let texts: Vec<_> = decoded
            .items
            .iter()
            .map(|item| item.text.as_str())
            .collect();
        assert_eq!(texts, ["second active", "first active", "good"]);
        assert_eq!(decoded.items.iter().filter(|item| item.active).count(), 1);
        assert_eq!(
            decoded.active_task().map(|item| item.text.as_str()),
            Some("first active")
        );
    }

    #[test]
    fn decoding_caps_the_list_size() {
        let mut contents = String::from("v1\n");
        for index in 0..(MAX_TASKS + 10) {
            writeln!(contents, "0\ttask {index}").expect("write to a string");
        }
        assert_eq!(TasksState::decode(&contents).items.len(), MAX_TASKS);
    }

    #[test]
    fn editing_a_task_normalizes_in_place_and_preserves_flags() {
        let mut state = TasksState::default();
        let first = added(&mut state, "first");
        let second = added(&mut state, "second");
        assert!(state.toggle_active(first));
        assert!(state.toggle_done(second));

        assert_eq!(
            state.edit(first, "  first \t updated\r\n "),
            TaskEditOutcome::Updated
        );
        assert_eq!(state.items[1].id, first);
        assert_eq!(state.items[1].text, "first updated");
        assert!(state.items[1].active);
        assert!(!state.items[1].done);

        assert_eq!(
            state.edit(first, "first updated"),
            TaskEditOutcome::Unchanged
        );
        assert_eq!(state.edit(first, "   \t "), TaskEditOutcome::Rejected);
        assert_eq!(
            state.edit(TaskId(999), "missing"),
            TaskEditOutcome::Rejected
        );
        assert_eq!(state.items[1].text, "first updated");

        assert_eq!(
            state.edit(second, "second updated"),
            TaskEditOutcome::Updated
        );
        assert!(state.items[0].done);
        assert_eq!(state.items[0].text, "second updated");
    }

    #[test]
    fn editor_mode_tracks_add_edit_and_clears_when_target_is_removed() {
        let mut state = TasksState::default();
        let a = added(&mut state, "alpha");
        let b = added(&mut state, "beta");

        assert!(!state.layout_input().editing);
        assert!(state.begin_add());
        assert_eq!(state.editor, Some(TaskEditorMode::Add));
        assert!(state.layout_input().editing);
        assert_eq!(state.editing_id(), None);

        assert_eq!(state.begin_edit(a), Some("alpha"));
        assert_eq!(state.editor, Some(TaskEditorMode::Edit(a)));
        assert_eq!(state.editing_id(), Some(a));

        assert!(state.remove(a));
        assert_eq!(state.editor, None);
        assert!(!state.is_editing());

        assert!(state.toggle_done(b));
        assert_eq!(state.begin_edit(b), Some("beta"));
        assert!(state.clear_done());
        assert_eq!(state.editor, None);
    }
}
