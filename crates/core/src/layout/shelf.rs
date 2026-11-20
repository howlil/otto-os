use crate::notch::shelf::SHELF_PAGE_SIZE;

use super::geometry::Rect;
use super::geometry::scale;
use super::input::ShelfLayoutInput;
use super::panel::PANEL_TAB_GAP_POINTS;
use super::panel::PRIVACY_COMPACT_MIC_WIDTH_POINTS;

pub(super) const SHELF_ADD_WIDTH_POINTS: i32 = 40;
pub(super) const SHELF_CLEAR_WIDTH_POINTS: i32 = 38;
pub(super) const SHELF_PAGER_WIDTH_POINTS: i32 = 28;
pub(super) const SHELF_BODY_TOP_POINTS: i32 = 38;
pub(super) const SHELF_BODY_HEIGHT_POINTS: i32 = 90;
pub(super) const SHELF_GRID_COLS: usize = 3;
pub(super) const SHELF_GRID_GAP_POINTS: i32 = 6;
pub(super) const SHELF_CARD_MINI_POINTS: i32 = 18;
pub(super) const SHELF_CARD_MINI_PAD_POINTS: i32 = 3;
pub(super) const SHELF_CARD_MINI_GAP_POINTS: i32 = 2;

pub(super) struct ShelfControlsGeometry {
    pub(super) add_button: Rect,
    pub(super) clear_button: Rect,
    pub(super) pager_button: Rect,
    pub(super) mic_button: Rect,
    pub(super) dropzone: Rect,
    pub(super) cards: [Rect; SHELF_PAGE_SIZE],
    pub(super) copy_buttons: [Rect; SHELF_PAGE_SIZE],
    pub(super) remove_buttons: [Rect; SHELF_PAGE_SIZE],
}

pub(super) struct ShelfHeaderButtons {
    pub(super) add: Rect,
    pub(super) clear: Rect,
    pub(super) pager: Rect,
    pub(super) mic: Rect,
}

pub(super) fn compute_shelf_header_buttons(
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    shelf: ShelfLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> ShelfHeaderButtons {
    let gap = scale(PANEL_TAB_GAP_POINTS, dpi);
    let clear = if shelf.has_items {
        let clear_w = scale(SHELF_CLEAR_WIDTH_POINTS, dpi);
        Rect {
            x: content_right - clear_w,
            y: tab_y,
            width: clear_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let add_right = if clear.is_empty() {
        content_right
    } else {
        clear.x - gap
    };
    let add_w = scale(SHELF_ADD_WIDTH_POINTS, dpi);
    let add = Rect {
        x: add_right - add_w,
        y: tab_y,
        width: add_w,
        height: tab_h,
    };
    let pager = if shelf.has_pager {
        let pager_w = scale(SHELF_PAGER_WIDTH_POINTS, dpi);
        Rect {
            x: add.x - gap - pager_w,
            y: tab_y,
            width: pager_w,
            height: tab_h,
        }
    } else {
        Rect::default()
    };
    let mic_right = if pager.is_empty() { add.x } else { pager.x };
    let mic = if panel_mic_control {
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
    ShelfHeaderButtons {
        add,
        clear,
        pager,
        mic,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_shelf_controls(
    notch_y: i32,
    content_left: i32,
    content_right: i32,
    tab_y: i32,
    tab_h: i32,
    shelf: ShelfLayoutInput,
    panel_mic_control: bool,
    dpi: u32,
) -> ShelfControlsGeometry {
    let header =
        compute_shelf_header_buttons(content_right, tab_y, tab_h, shelf, panel_mic_control, dpi);
    let body_top = notch_y + scale(SHELF_BODY_TOP_POINTS, dpi);
    let body_h = scale(SHELF_BODY_HEIGHT_POINTS, dpi);
    let body_w = (content_right - content_left).max(0);
    let dropzone = Rect {
        x: content_left,
        y: body_top,
        width: body_w,
        height: body_h,
    };
    let grid_gap = scale(SHELF_GRID_GAP_POINTS, dpi);
    let card_w = ((body_w - grid_gap * 2) / SHELF_GRID_COLS as i32).max(0);
    let card_h = ((body_h - grid_gap) / 2).max(0);
    let mini_size = scale(SHELF_CARD_MINI_POINTS, dpi);
    let mini_pad = scale(SHELF_CARD_MINI_PAD_POINTS, dpi);
    let mini_gap = scale(SHELF_CARD_MINI_GAP_POINTS, dpi);
    let visible = usize::from(shelf.visible_count).min(SHELF_PAGE_SIZE);
    let mut cards = [Rect::default(); SHELF_PAGE_SIZE];
    let mut copy_buttons = [Rect::default(); SHELF_PAGE_SIZE];
    let mut remove_buttons = [Rect::default(); SHELF_PAGE_SIZE];
    for index in 0..visible {
        let col = (index % SHELF_GRID_COLS) as i32;
        let row = (index / SHELF_GRID_COLS) as i32;
        let card = Rect {
            x: content_left + col * (card_w + grid_gap),
            y: body_top + row * (card_h + grid_gap),
            width: card_w,
            height: card_h,
        };
        let remove = Rect {
            x: card.right() - mini_pad - mini_size,
            y: card.y + mini_pad,
            width: mini_size,
            height: mini_size,
        };
        let copy = Rect {
            x: remove.x - mini_gap - mini_size,
            y: card.y + mini_pad,
            width: mini_size,
            height: mini_size,
        };
        cards[index] = card;
        copy_buttons[index] = copy;
        remove_buttons[index] = remove;
    }
    ShelfControlsGeometry {
        add_button: header.add,
        clear_button: header.clear,
        pager_button: header.pager,
        mic_button: header.mic,
        dropzone,
        cards,
        copy_buttons,
        remove_buttons,
    }
}
