use crate::notch::clipboard::{CLIPBOARD_PAGE_SIZE, ClipboardLayoutInput};
use crate::notch::tasks::{TASKS_PAGE_SIZE, TasksLayoutInput};

use super::clipboard::CLIPBOARD_ROW_REMOVE_PAD_POINTS;
use super::clipboard::compute_clipboard_controls;
use super::geometry::Rect;
use super::geometry::scale;

pub(super) const TASK_CHECK_BUTTON_POINTS: i32 = 16;

const _: () = assert!(TASKS_PAGE_SIZE == CLIPBOARD_PAGE_SIZE);

pub(super) struct TasksControlsGeometry {
    pub(super) add_button: Rect,
    pub(super) clear_button: Rect,
    pub(super) pager_button: Rect,
    pub(super) mic_button: Rect,
    pub(super) body: Rect,
    pub(super) rows: [Rect; TASKS_PAGE_SIZE],
    pub(super) check_buttons: [Rect; TASKS_PAGE_SIZE],
    pub(super) remove_buttons: [Rect; TASKS_PAGE_SIZE],
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
    let check_size = scale(TASK_CHECK_BUTTON_POINTS, dpi);
    let pad = scale(CLIPBOARD_ROW_REMOVE_PAD_POINTS, dpi);
    let mut check_buttons = [Rect::default(); TASKS_PAGE_SIZE];
    for (button, row) in check_buttons.iter_mut().zip(shared.rows) {
        if !row.is_empty() {
            *button = Rect {
                x: row.x + pad,
                y: row.y + (row.height - check_size) / 2,
                width: check_size,
                height: check_size,
            };
        }
    }
    TasksControlsGeometry {
        add_button: shared.pause_button,
        clear_button: shared.clear_button,
        pager_button: shared.pager_button,
        mic_button: shared.mic_button,
        body: shared.body,
        rows: shared.rows,
        check_buttons,
        remove_buttons: shared.remove_buttons,
    }
}
