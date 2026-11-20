use crate::notch::clipboard::{CLIPBOARD_PAGE_SIZE, ClipboardLayoutInput};

use super::geometry::Rect;
use super::geometry::scale;
use super::panel::PANEL_TAB_GAP_POINTS;
use super::panel::PRIVACY_COMPACT_MIC_WIDTH_POINTS;

pub(super) const CLIPBOARD_PAUSE_WIDTH_POINTS: i32 = 42;
pub(super) const CLIPBOARD_CLEAR_WIDTH_POINTS: i32 = 38;
pub(super) const CLIPBOARD_PAGER_WIDTH_POINTS: i32 = 28;
pub(super) const CLIPBOARD_BODY_TOP_POINTS: i32 = 38;
pub(super) const CLIPBOARD_BODY_HEIGHT_POINTS: i32 = 90;
pub(super) const CLIPBOARD_ROW_GAP_POINTS: i32 = 3;
pub(super) const CLIPBOARD_ROW_REMOVE_POINTS: i32 = 16;
pub(super) const CLIPBOARD_ROW_REMOVE_PAD_POINTS: i32 = 4;

pub(super) struct ClipboardControlsGeometry {
    pub(super) pause_button: Rect,
    pub(super) clear_button: Rect,
    pub(super) pager_button: Rect,
    pub(super) mic_button: Rect,
    pub(super) body: Rect,
    pub(super) rows: [Rect; CLIPBOARD_PAGE_SIZE],
    pub(super) remove_buttons: [Rect; CLIPBOARD_PAGE_SIZE],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_clipboard_controls(
    notch_y: i32,
    content_left: i32,
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    clipboard: ClipboardLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> ClipboardControlsGeometry {
    let gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let clear_button = if clipboard.has_items {
        let clear_w = scale(CLIPBOARD_CLEAR_WIDTH_POINTS, dpi);
        Rect {
            x: content_right - clear_w,
            y: tab_y,
            width: clear_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let pause_right = if clear_button.is_empty() {
        content_right
    } else {
        clear_button.x - gap
    };
    let pause_w = scale(CLIPBOARD_PAUSE_WIDTH_POINTS, dpi);
    let pause_button = Rect {
        x: pause_right - pause_w,
        y: tab_y,
        width: pause_w,
        height: tab_h,
    };
    let pager_button = if clipboard.has_pager {
        let pager_w = scale(CLIPBOARD_PAGER_WIDTH_POINTS, dpi);
        Rect {
            x: pause_button.x - gap - pager_w,
            y: tab_y,
            width: pager_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let mic_right = if pager_button.is_empty() {
        pause_button.x
    } else {
        pager_button.x
    };
    let mic_button = if panel_mic_control {
        let mic_w = scale(PRIVACY_COMPACT_MIC_WIDTH_POINTS, dpi);
        Rect {
            x: mic_right - gap - mic_w,
            y: tab_y,
            width: mic_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };

    let body_top = notch_y + scale(CLIPBOARD_BODY_TOP_POINTS, dpi);
    let body_h = scale(CLIPBOARD_BODY_HEIGHT_POINTS, dpi);
    let body_w = (content_right - content_left).max(0);
    let body = Rect {
        x: content_left,
        y: body_top,
        width: body_w,
        height: body_h,
    };
    let row_gap = scale(CLIPBOARD_ROW_GAP_POINTS, dpi);
    let row_h =
        ((body_h - row_gap * (CLIPBOARD_PAGE_SIZE as i32 - 1)) / CLIPBOARD_PAGE_SIZE as i32).max(0);
    let remove_size = scale(CLIPBOARD_ROW_REMOVE_POINTS, dpi);
    let remove_pad = scale(CLIPBOARD_ROW_REMOVE_PAD_POINTS, dpi);
    let visible = usize::from(clipboard.visible_count).min(CLIPBOARD_PAGE_SIZE);
    let mut rows = [Rect::default(); CLIPBOARD_PAGE_SIZE];
    let mut remove_buttons = [Rect::default(); CLIPBOARD_PAGE_SIZE];
    for index in 0..visible {
        let row = Rect {
            x: content_left,
            y: body_top + (row_h + row_gap) * index as i32,
            width: body_w,
            height: row_h,
        };
        let remove = Rect {
            x: row.right() - remove_pad - remove_size,
            y: row.y + (row_h - remove_size) / 2,
            width: remove_size,
            height: remove_size,
        };
        rows[index] = row;
        remove_buttons[index] = remove;
    }
    ClipboardControlsGeometry {
        pause_button,
        clear_button,
        pager_button,
        mic_button,
        body,
        rows,
        remove_buttons,
    }
}
