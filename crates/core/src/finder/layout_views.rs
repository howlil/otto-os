use crate::layout::{Rect, scale};

use super::layout::{
    FinderColumnHeaderSlot, FinderColumnPaneSlot, FinderColumnRowSlot, FinderGalleryThumbSlot,
    FinderGoToLayout, FinderIconTileSlot, FinderInspectorLayout, FinderLayout, FinderListRowSlot,
    FinderQuickLookLayout,
};
use super::model::{
    FINDER_ICON_SIZE_MAX, FINDER_ICON_SIZE_MIN, FINDER_LIST_ROW_HEIGHT_POINTS, FinderMillerColumn,
    FinderSortKey, FinderTab, FinderTag, FinderTreeRow,
};

pub(super) fn layout_icon_view(
    layout: &mut FinderLayout,
    tab: &FinderTab,
    icon_size_points: i32,
    dpi: u32,
) {
    let stage = layout.content_stage;
    let clamped = icon_size_points.clamp(FINDER_ICON_SIZE_MIN, FINDER_ICON_SIZE_MAX);
    let icon_px = scale(clamped, dpi);
    let cell_w = icon_px + scale(44, dpi);
    let label_h = scale(34, dpi);
    let cell_h = icon_px + label_h + scale(18, dpi);
    let pad_x = scale(16, dpi);
    let pad_y = scale(14, dpi);
    let gap_x = scale(12, dpi);
    let gap_y = scale(12, dpi);
    let inner_w = (stage.width - pad_x * 2).max(cell_w);
    let cols = ((inner_w + gap_x) / (cell_w + gap_x)).max(1) as usize;
    let total_rows = tab.entries.len().div_ceil(cols);
    let total_h = pad_y * 2 + (total_rows as i32) * (cell_h + gap_y);
    layout.content_scroll_height = total_h.max(stage.height);

    let scroll_y = tab.scroll_y.max(0);
    for (index, entry) in tab.entries.iter().enumerate() {
        let col = (index % cols) as i32;
        let row = (index / cols) as i32;
        let x = stage.x + pad_x + col * (cell_w + gap_x);
        let y = stage.y + pad_y + row * (cell_h + gap_y) - scroll_y;
        if y + cell_h < stage.y || y > stage.bottom() {
            continue;
        }
        let rect = Rect::new(x, y, cell_w, cell_h);
        let icon_rect = Rect::new(
            x + (cell_w - icon_px) / 2,
            y + scale(6, dpi),
            icon_px,
            icon_px,
        );
        let label_rect = Rect::new(
            x + scale(4, dpi),
            icon_rect.bottom() + scale(4, dpi),
            cell_w - scale(8, dpi),
            label_h,
        );
        layout.icon_tiles.push(FinderIconTileSlot {
            index,
            path: entry.path.clone(),
            rect,
            icon_rect,
            label_rect,
        });
    }
}

pub(super) fn layout_list_view(
    layout: &mut FinderLayout,
    tab: &FinderTab,
    rows: &[FinderTreeRow],
    dpi: u32,
) {
    let hdr = layout.list_header;
    let stage = layout.content_stage;
    let date_w = scale(152, dpi);
    let size_w = scale(92, dpi);
    let kind_w = scale(116, dpi);
    let name_w = (stage.width - date_w - size_w - kind_w).max(scale(160, dpi));

    let name_x = stage.x;
    let date_x = name_x + name_w;
    let size_x = date_x + date_w;
    let kind_x = size_x + size_w;

    layout.list_column_headers = vec![
        FinderColumnHeaderSlot {
            key: FinderSortKey::Name,
            rect: Rect::new(name_x, hdr.y, name_w, hdr.height),
        },
        FinderColumnHeaderSlot {
            key: FinderSortKey::DateModified,
            rect: Rect::new(date_x, hdr.y, date_w, hdr.height),
        },
        FinderColumnHeaderSlot {
            key: FinderSortKey::Size,
            rect: Rect::new(size_x, hdr.y, size_w, hdr.height),
        },
        FinderColumnHeaderSlot {
            key: FinderSortKey::Kind,
            rect: Rect::new(kind_x, hdr.y, (stage.right() - kind_x).max(0), hdr.height),
        },
    ];

    let row_h = scale(FINDER_LIST_ROW_HEIGHT_POINTS, dpi);
    let total_h = (rows.len() as i32) * row_h + scale(12, dpi);
    layout.content_scroll_height = total_h.max(stage.height);

    let scroll_y = tab.scroll_y.max(0);
    let indent_step = scale(16, dpi);
    let chevron_sz = scale(16, dpi);
    let left_pad = scale(10, dpi);
    for (index, row) in rows.iter().enumerate() {
        let y = stage.y + (index as i32) * row_h - scroll_y;
        if y + row_h < stage.y || y > stage.bottom() {
            continue;
        }
        let indent = (i32::from(row.depth)) * indent_step;
        let chevron_rect = row.entry.is_dir().then(|| {
            Rect::new(
                name_x + left_pad + indent,
                y + (row_h - chevron_sz) / 2,
                chevron_sz,
                chevron_sz,
            )
        });
        let name_start = name_x + left_pad + indent + chevron_sz + scale(4, dpi);
        layout.list_rows.push(FinderListRowSlot {
            index,
            path: row.entry.path.clone(),
            rect: Rect::new(stage.x, y, stage.width, row_h),
            chevron_rect,
            name_rect: Rect::new(name_start, y, (date_x - name_start).max(1), row_h),
            date_rect: Rect::new(date_x, y, date_w, row_h),
            size_rect: Rect::new(size_x, y, size_w, row_h),
            kind_rect: Rect::new(kind_x, y, (stage.right() - kind_x).max(0), row_h),
            depth: row.depth,
            expanded: row.expanded,
        });
    }
}

pub(super) fn layout_column_view(
    layout: &mut FinderLayout,
    tab: &FinderTab,
    columns: &[FinderMillerColumn],
    column_width_points: i32,
    dpi: u32,
) {
    let stage = layout.content_stage;
    let col_w = scale(column_width_points, dpi);
    let row_h = scale(FINDER_LIST_ROW_HEIGHT_POINTS, dpi);
    let preview_w = scale(268, dpi);
    let total_w = (columns.len() as i32) * col_w + preview_w;
    layout.content_scroll_width = total_w.max(stage.width);
    layout.content_scroll_height = stage.height;

    let scroll_x = tab.scroll_x.max(0);
    for (col_index, col) in columns.iter().enumerate() {
        let x = stage.x + (col_index as i32) * col_w - scroll_x;
        let pane_rect = Rect::new(x, stage.y, col_w, stage.height);
        if pane_rect.right() < stage.x || pane_rect.x > stage.right() {
            continue;
        }
        let mut rows = Vec::new();
        let col_scroll = col.scroll_y.max(0);
        for (row_index, entry) in col.entries.iter().enumerate() {
            let y = stage.y + scale(6, dpi) + (row_index as i32) * row_h - col_scroll;
            if y + row_h < stage.y || y > stage.bottom() {
                continue;
            }
            rows.push(FinderColumnRowSlot {
                col_index,
                row_index,
                path: entry.path.clone(),
                rect: Rect::new(x + scale(6, dpi), y, col_w - scale(12, dpi), row_h),
                is_dir: entry.is_dir(),
            });
        }
        layout.column_panes.push(FinderColumnPaneSlot {
            col_index,
            rect: pane_rect,
            rows,
        });
    }

    let preview_x = stage.x + (columns.len() as i32) * col_w - scroll_x;
    let remaining_w = (stage.right() - preview_x).max(preview_w);
    if preview_x < stage.right() {
        layout.column_preview_rect = Some(Rect::new(preview_x, stage.y, remaining_w, stage.height));
    }
}

pub(super) fn layout_gallery_view(layout: &mut FinderLayout, tab: &FinderTab, dpi: u32) {
    let stage = layout.content_stage;
    let strip_h = scale(104, dpi).min(stage.height / 3);
    let main_h = (stage.height - strip_h).max(1);
    layout.gallery_stage_rect = Some(Rect::new(stage.x, stage.y, stage.width, main_h));
    let strip = Rect::new(stage.x, stage.y + main_h, stage.width, strip_h);
    layout.gallery_strip_rect = Some(strip);

    let thumb_w = scale(76, dpi);
    let thumb_h = scale(76, dpi);
    let gap = scale(10, dpi);
    let pad = scale(14, dpi);
    let total_w = pad * 2 + (tab.entries.len() as i32) * (thumb_w + gap);
    layout.content_scroll_width = total_w.max(stage.width);
    layout.content_scroll_height = stage.height;

    let scroll_x = tab.scroll_x.max(0);
    let thumb_y = strip.y + (strip.height - thumb_h) / 2;
    for (index, entry) in tab.entries.iter().enumerate() {
        let x = strip.x + pad + (index as i32) * (thumb_w + gap) - scroll_x;
        if x + thumb_w < strip.x || x > strip.right() {
            continue;
        }
        layout.gallery_thumbs.push(FinderGalleryThumbSlot {
            index,
            path: entry.path.clone(),
            rect: Rect::new(x, thumb_y, thumb_w, thumb_h),
        });
    }
}

pub(super) fn layout_inspector(panel: Rect, dpi: u32) -> FinderInspectorLayout {
    let pad = scale(16, dpi);
    let inner_w = (panel.width - pad * 2).max(1);
    let preview_h = scale(140, dpi);
    let preview_rect = Rect::new(panel.x + pad, panel.y + pad, inner_w, preview_h);
    let tag_y = preview_rect.bottom() + scale(72, dpi);
    let dot_sz = scale(20, dpi);
    let dot_gap = scale(8, dpi);
    let mut tag_rects = [(FinderTag::Red, Rect::new(0, 0, 0, 0)); 7];
    for (idx, tag) in FinderTag::ALL.into_iter().enumerate() {
        tag_rects[idx] = (
            tag,
            Rect::new(
                panel.x + pad + (idx as i32) * (dot_sz + dot_gap),
                tag_y,
                dot_sz,
                dot_sz,
            ),
        );
    }
    let btn_h = scale(28, dpi);
    let btn_gap = scale(8, dpi);
    let copy_y = panel.bottom() - pad - btn_h;
    let open_y = copy_y - btn_gap - btn_h;
    FinderInspectorLayout {
        panel,
        preview_rect,
        tag_rects,
        open_button: Rect::new(panel.x + pad, open_y, inner_w, btn_h),
        copy_path_button: Rect::new(panel.x + pad, copy_y, inner_w, btn_h),
    }
}

pub(super) fn layout_quick_look(width: i32, height: i32, dpi: u32) -> FinderQuickLookLayout {
    let card_w = (width - scale(120, dpi)).clamp(scale(420, dpi), scale(760, dpi));
    let card_h = (height - scale(96, dpi)).clamp(scale(320, dpi), scale(560, dpi));
    let cx = (width - card_w) / 2;
    let cy = (height - card_h) / 2;
    let card = Rect::new(cx, cy, card_w, card_h);
    let hdr_h = scale(42, dpi);
    let header = Rect::new(cx, cy, card_w, hdr_h);
    let close_sz = scale(24, dpi);
    let close_button = Rect::new(
        cx + scale(12, dpi),
        cy + (hdr_h - close_sz) / 2,
        close_sz,
        close_sz,
    );
    let open_w = scale(124, dpi);
    let open_h = scale(26, dpi);
    let open_button = Rect::new(
        card.right() - scale(12, dpi) - open_w,
        cy + (hdr_h - open_h) / 2,
        open_w,
        open_h,
    );
    let nav_sz = scale(28, dpi);
    let nav_y = cy + hdr_h + (card_h - hdr_h - nav_sz) / 2;
    let prev_button = Rect::new(cx + scale(10, dpi), nav_y, nav_sz, nav_sz);
    let next_button = Rect::new(
        card.right() - scale(10, dpi) - nav_sz,
        nav_y,
        nav_sz,
        nav_sz,
    );
    let content = Rect::new(
        cx + scale(46, dpi),
        cy + hdr_h + scale(12, dpi),
        (card_w - scale(92, dpi)).max(1),
        (card_h - hdr_h - scale(24, dpi)).max(1),
    );
    FinderQuickLookLayout {
        backdrop: Rect::new(0, 0, width, height),
        card,
        header,
        close_button,
        open_button,
        content,
        prev_button,
        next_button,
    }
}

pub(super) fn layout_go_to_overlay(width: i32, height: i32, dpi: u32) -> FinderGoToLayout {
    let card_w = scale(460, dpi).min(width - scale(48, dpi));
    let card_h = scale(126, dpi);
    let cx = (width - card_w) / 2;
    let cy = scale(92, dpi)
        .min((height - card_h) / 2)
        .max(scale(24, dpi));
    let pad = scale(16, dpi);
    let input_rect = Rect::new(
        cx + pad,
        cy + scale(42, dpi),
        card_w - pad * 2,
        scale(30, dpi),
    );
    let btn_w = scale(76, dpi);
    let btn_h = scale(26, dpi);
    let btn_y = cy + card_h - pad - btn_h;
    let go_button = Rect::new(cx + card_w - pad - btn_w, btn_y, btn_w, btn_h);
    let cancel_button = Rect::new(go_button.x - scale(8, dpi) - btn_w, btn_y, btn_w, btn_h);
    FinderGoToLayout {
        card: Rect::new(cx, cy, card_w, card_h),
        input_rect,
        go_button,
        cancel_button,
    }
}
