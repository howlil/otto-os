use crate::notch::clipboard::{CLIPBOARD_PAGE_SIZE, ClipboardLayoutInput};
use crate::notch::tasks::{TASKS_PAGE_SIZE, TasksLayoutInput};

use super::clipboard::CLIPBOARD_ROW_REMOVE_PAD_POINTS;
use super::clipboard::compute_clipboard_controls;
use super::geometry::Rect;
use super::geometry::scale;

pub(super) const TASK_CHECK_BUTTON_POINTS: i32 = 16;
pub(super) const TASK_ACTION_GAP_POINTS: i32 = 4;
pub(super) const TASK_EDITOR_HEIGHT_POINTS: i32 = 28;
pub(super) const TASK_EDITOR_GAP_POINTS: i32 = 4;
pub(super) const TASK_EDITOR_EXTRA_HEIGHT_POINTS: i32 =
    TASK_EDITOR_HEIGHT_POINTS + TASK_EDITOR_GAP_POINTS;
pub(super) const TASK_EDITOR_ICON_WIDTH_POINTS: i32 = 26;
pub(super) const TASK_EDITOR_HINT_WIDTH_POINTS: i32 = 34;
pub(super) const TASK_EDITOR_PAD_Y_POINTS: i32 = 4;

const _: () = assert!(TASKS_PAGE_SIZE == CLIPBOARD_PAGE_SIZE);

pub(super) struct TasksControlsGeometry {
    pub(super) add_button: Rect,
    pub(super) clear_button: Rect,
    pub(super) pager_button: Rect,
    pub(super) mic_button: Rect,
    pub(super) editor: Rect,
    pub(super) editor_input: Rect,
    pub(super) body: Rect,
    pub(super) rows: [Rect; TASKS_PAGE_SIZE],
    pub(super) check_buttons: [Rect; TASKS_PAGE_SIZE],
    pub(super) edit_buttons: [Rect; TASKS_PAGE_SIZE],
    pub(super) remove_buttons: [Rect; TASKS_PAGE_SIZE],
}

const fn offset_y(rect: Rect, dy: i32) -> Rect {
    if rect.width <= 0 || rect.height <= 0 {
        return rect;
    }
    Rect {
        y: rect.y + dy,
        ..rect
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_tasks_controls(
    notch_y: i32,
    content_left: i32,
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    tasks: TasksLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> TasksControlsGeometry {
    let shared = compute_clipboard_controls(
        notch_y,
        content_left,
        content_right,
        tab_y,
        tab_h,
        ClipboardLayoutInput {
            visible_count: tasks.visible_count,
            has_pager: tasks.has_pager,
            has_items: tasks.has_done,
            paused: false,
        },
        panel_mic_control,
        dpi,
    );
    let (editor, editor_input, body_shift) = if tasks.editing {
        let editor_h = scale(TASK_EDITOR_HEIGHT_POINTS, dpi);
        let editor_gap = scale(TASK_EDITOR_GAP_POINTS, dpi);
        let editor = Rect {
            x: shared.body.x,
            y: shared.body.y,
            width: shared.body.width,
            height: editor_h,
        };
        let icon_w = scale(TASK_EDITOR_ICON_WIDTH_POINTS, dpi);
        let hint_w = scale(TASK_EDITOR_HINT_WIDTH_POINTS, dpi);
        let pad_y = scale(TASK_EDITOR_PAD_Y_POINTS, dpi);
        let editor_input = Rect {
            x: editor.x + icon_w,
            y: editor.y + pad_y,
            width: (editor.width - icon_w - hint_w).max(0),
            height: (editor.height - pad_y * 2).max(0),
        };
        (editor, editor_input, editor_h + editor_gap)
    } else {
        (Rect::default(), Rect::default(), 0)
    };
    let body = offset_y(shared.body, body_shift);
    let mut rows = [Rect::default(); TASKS_PAGE_SIZE];
    let mut remove_buttons = [Rect::default(); TASKS_PAGE_SIZE];
    let mut check_buttons = [Rect::default(); TASKS_PAGE_SIZE];
    let mut edit_buttons = [Rect::default(); TASKS_PAGE_SIZE];
    let check_size = scale(TASK_CHECK_BUTTON_POINTS, dpi);
    let pad = scale(CLIPBOARD_ROW_REMOVE_PAD_POINTS, dpi);
    let action_gap = scale(TASK_ACTION_GAP_POINTS, dpi);
    for index in 0..TASKS_PAGE_SIZE {
        let row = offset_y(shared.rows[index], body_shift);
        let remove = offset_y(shared.remove_buttons[index], body_shift);
        rows[index] = row;
        remove_buttons[index] = remove;
        if !row.is_empty() {
            check_buttons[index] = Rect {
                x: row.x + pad,
                y: row.y + (row.height - check_size) / 2,
                width: check_size,
                height: check_size,
            };
            if !remove.is_empty() {
                edit_buttons[index] = Rect {
                    x: remove.x - action_gap - remove.width,
                    y: remove.y,
                    width: remove.width,
                    height: remove.height,
                };
            }
        }
    }
    TasksControlsGeometry {
        add_button: shared.pause_button,
        clear_button: shared.clear_button,
        pager_button: shared.pager_button,
        mic_button: shared.mic_button,
        editor,
        editor_input,
        body,
        rows,
        check_buttons,
        edit_buttons,
        remove_buttons,
    }
}
