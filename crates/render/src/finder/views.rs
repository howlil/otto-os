use std::collections::HashMap;

use otto_core::{
    FinderEntry, FinderHit, FinderSortOrder, FinderTag, FinderViewMode, Rect, scale,
    tag_mask_contains,
};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

use crate::appearance::tokens::{rgba_hex as rgba, with_alpha};
use crate::kit::canvas::HwndCanvas;

use super::icons::{draw_chevron, draw_entry_icon};
use super::overlays::draw_preview_card_body;
use super::palette::{FinderFormats, FinderPaintState, FinderPalette};

pub(super) fn draw_content_views(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    preview_bmp: Option<&ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    canvas.fill(layout.content_stage, palette.content);
    match state.state.tab().view_mode {
        FinderViewMode::Icon => draw_icon_view(canvas, formats, thumbs, state, palette),
        FinderViewMode::List => draw_list_view(canvas, formats, thumbs, state, palette),
        FinderViewMode::Column => {
            draw_column_view(canvas, formats, thumbs, preview_bmp, state, palette);
        }
        FinderViewMode::Gallery => {
            draw_gallery_view(canvas, formats, thumbs, preview_bmp, state, palette);
        }
    }
    if let Some(marquee) = state.marquee
        && !marquee.is_empty()
    {
        canvas.fill_round(marquee, 3.0, with_alpha(palette.accent, 0.16));
        canvas.stroke_round(marquee, 3.0, 1.0, palette.accent);
    }
}

fn is_selected(state: &FinderPaintState<'_>, path: &str) -> bool {
    state
        .state
        .tab()
        .selected_paths
        .iter()
        .any(|sel| sel.eq_ignore_ascii_case(path))
}

fn draw_tag_dots(canvas: &HwndCanvas, mask: u8, right_x: i32, cy: f32, dpi: u32) {
    if mask == 0 {
        return;
    }
    let r = scale(4, dpi) as f32;
    let step = scale(9, dpi) as f32;
    let mut x = right_x as f32 - r;
    for tag in FinderTag::ALL.into_iter().rev() {
        if tag_mask_contains(mask, tag) {
            canvas.fill_circle(x, cy, r, rgba(tag.rgb(), 1.0));
            x -= step;
        }
    }
}

fn draw_entry_thumb_or_glyph(
    canvas: &HwndCanvas,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    entry: &FinderEntry,
    rect: Rect,
    palette: &FinderPalette,
) {
    if !entry.is_dir()
        && let Some(bmp) = thumbs.get(&entry.path.to_ascii_lowercase())
    {
        canvas.draw_bitmap(bmp, rect);
        return;
    }
    draw_entry_icon(canvas, rect, entry.kind, palette);
}

fn draw_icon_view(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    let tab = state.state.tab();
    canvas.push_clip(layout.content_stage);
    if tab.entries.is_empty() {
        canvas.text(
            "This folder is empty",
            &formats.row,
            layout.content_stage,
            palette.muted,
        );
        canvas.pop_clip();
        return;
    }
    let tile_r = scale(8, dpi) as f32;
    for slot in &layout.icon_tiles {
        let Some(entry) = tab.entries.get(slot.index) else {
            continue;
        };
        let selected = is_selected(state, &entry.path);
        let hovered = matches!(
            state.hovered,
            Some(FinderHit::Item { path, .. }) if path.eq_ignore_ascii_case(&entry.path)
        );
        if selected {
            canvas.fill_round(
                Rect::new(
                    slot.icon_rect.x - 4,
                    slot.icon_rect.y - 4,
                    slot.icon_rect.width + 8,
                    slot.icon_rect.height + 8,
                ),
                tile_r,
                palette.hover,
            );
        } else if hovered {
            canvas.fill_round(slot.rect, tile_r, palette.hover);
        }
        draw_entry_thumb_or_glyph(canvas, thumbs, entry, slot.icon_rect, palette);
        let rename_draft = state
            .state
            .rename
            .as_ref()
            .filter(|r| r.path.eq_ignore_ascii_case(&entry.path));
        if let Some(rename) = rename_draft {
            canvas.fill_round(slot.label_rect, 4.0, palette.sheet);
            canvas.stroke_round(slot.label_rect, 4.0, 1.5, palette.accent);
            let txt = format!("{}|", rename.draft);
            canvas.text(
                &txt,
                &formats.small_wrap_center,
                slot.label_rect,
                palette.text,
            );
        } else {
            if selected {
                canvas.fill_round(slot.label_rect, 5.0, palette.accent);
            }
            canvas.text(
                &entry.name,
                &formats.small_wrap_center,
                slot.label_rect,
                if selected {
                    palette.on_accent
                } else {
                    palette.text
                },
            );
        }
        if entry.tag_mask != 0 {
            draw_tag_dots(
                canvas,
                entry.tag_mask,
                slot.icon_rect.right(),
                slot.icon_rect.bottom() as f32 - 4.0,
                dpi,
            );
        }
    }
    canvas.pop_clip();
}

fn draw_list_view(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    draw_list_header(canvas, formats, state, palette);
    let layout = state.layout;
    let dpi = layout.dpi;
    canvas.push_clip(layout.content_stage);
    if state.list_rows.is_empty() {
        canvas.text(
            "No items",
            &formats.row,
            Rect::new(
                layout.content_stage.x + scale(20, dpi),
                layout.content_stage.y + scale(16, dpi),
                layout.content_stage.width - scale(40, dpi),
                scale(28, dpi),
            ),
            palette.muted,
        );
        canvas.pop_clip();
        return;
    }
    for slot in &layout.list_rows {
        draw_list_row(canvas, formats, thumbs, state, slot, palette);
    }
    canvas.pop_clip();
}

fn draw_list_header(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    let tab = state.state.tab();
    canvas.fill(layout.list_header, palette.toolbar);
    canvas.fill(
        Rect::new(
            layout.list_header.x,
            layout.list_header.bottom() - 1,
            layout.list_header.width,
            1,
        ),
        palette.hairline,
    );
    let pad = scale(10, dpi);
    for hdr in &layout.list_column_headers {
        let hovered = state.hovered == Some(&FinderHit::ListColumnHeader(hdr.key));
        if hovered {
            canvas.fill(hdr.rect, palette.hover);
        }
        let active = tab.sort_key == hdr.key;
        let label = if active {
            let arrow = if tab.sort_order == FinderSortOrder::Ascending {
                "▲"
            } else {
                "▼"
            };
            format!("{} {arrow}", hdr.key.label())
        } else {
            String::from(hdr.key.label())
        };
        canvas.text(
            &label,
            &formats.section,
            Rect::new(
                hdr.rect.x + pad,
                hdr.rect.y,
                (hdr.rect.width - pad * 2).max(1),
                hdr.rect.height,
            ),
            if active {
                palette.text
            } else {
                palette.secondary
            },
        );
        canvas.fill(
            Rect::new(hdr.rect.right() - 1, hdr.rect.y + 4, 1, hdr.rect.height - 8),
            palette.hairline,
        );
    }
}

fn draw_list_row(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    slot: &otto_core::FinderListRowSlot,
    palette: &FinderPalette,
) {
    let Some(row) = state.list_rows.get(slot.index) else {
        return;
    };
    let dpi = state.layout.dpi;
    let pad = scale(10, dpi);
    let icon_sz = scale(18, dpi);
    let radius = scale(6, dpi) as f32;
    let entry = &row.entry;
    let selected = is_selected(state, &entry.path);
    let hovered = matches!(
        state.hovered,
        Some(FinderHit::Item { path, .. }) if path.eq_ignore_ascii_case(&entry.path)
    );
    let row_pill = Rect::new(
        slot.rect.x + scale(6, dpi),
        slot.rect.y + 1,
        (slot.rect.width - scale(12, dpi)).max(1),
        (slot.rect.height - 2).max(1),
    );
    if selected {
        canvas.fill_round(row_pill, radius, palette.accent);
    } else if hovered {
        canvas.fill_round(row_pill, radius, palette.hover);
    } else if slot.index % 2 == 1 {
        canvas.fill(slot.rect, palette.zebra);
    }
    let fg = if selected {
        palette.on_accent
    } else {
        palette.text
    };
    let sub_fg = if selected {
        with_alpha(palette.on_accent, 0.84)
    } else {
        palette.secondary
    };
    if let Some(chev) = slot.chevron_rect {
        draw_chevron(canvas, chev, slot.expanded, sub_fg);
    }
    let icon_rect = Rect::new(
        slot.name_rect.x,
        slot.name_rect.y + (slot.name_rect.height - icon_sz) / 2,
        icon_sz,
        icon_sz,
    );
    draw_entry_thumb_or_glyph(canvas, thumbs, entry, icon_rect, palette);
    let name_text_x = icon_rect.right() + scale(8, dpi);
    let name_box = Rect::new(
        name_text_x,
        slot.name_rect.y,
        (slot.name_rect.right() - name_text_x - scale(26, dpi)).max(1),
        slot.name_rect.height,
    );
    if let Some(rename) = state
        .state
        .rename
        .as_ref()
        .filter(|r| r.path.eq_ignore_ascii_case(&entry.path))
    {
        let edit_r = Rect::new(
            name_box.x - 3,
            name_box.y + 3,
            name_box.width + 6,
            (name_box.height - 6).max(1),
        );
        canvas.fill_round(edit_r, 4.0, palette.sheet);
        canvas.stroke_round(edit_r, 4.0, 1.5, palette.accent);
        canvas.text(
            &format!("{}|", rename.draft),
            &formats.row,
            name_box,
            palette.text,
        );
    } else {
        canvas.text(&entry.name, &formats.row, name_box, fg);
    }
    draw_tag_dots(
        canvas,
        entry.tag_mask,
        slot.name_rect.right() - scale(6, dpi),
        slot.rect.y as f32 + slot.rect.height as f32 * 0.5,
        dpi,
    );
    draw_list_row_meta(canvas, formats, entry, slot, pad, sub_fg);
}

fn draw_list_row_meta(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    entry: &FinderEntry,
    slot: &otto_core::FinderListRowSlot,
    pad: i32,
    sub_fg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
) {
    canvas.tabular_text(
        &entry.modified_label,
        &formats.small,
        Rect::new(
            slot.date_rect.x + pad,
            slot.date_rect.y,
            (slot.date_rect.width - pad * 2).max(1),
            slot.date_rect.height,
        ),
        sub_fg,
    );
    canvas.tabular_text(
        &entry.size_label(),
        &formats.row_trailing,
        Rect::new(
            slot.size_rect.x + pad,
            slot.size_rect.y,
            (slot.size_rect.width - pad * 2).max(1),
            slot.size_rect.height,
        ),
        sub_fg,
    );
    canvas.text(
        &entry.kind_label,
        &formats.small,
        Rect::new(
            slot.kind_rect.x + pad,
            slot.kind_rect.y,
            (slot.kind_rect.width - pad * 2).max(1),
            slot.kind_rect.height,
        ),
        sub_fg,
    );
}

fn draw_column_view(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    preview_bmp: Option<&ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    let tab = state.state.tab();
    canvas.push_clip(layout.content_stage);
    let icon_sz = scale(18, dpi);
    let radius = scale(6, dpi) as f32;
    let last_col_idx = tab.miller_columns.len().saturating_sub(1);

    for pane in &layout.column_panes {
        let Some(col) = tab.miller_columns.get(pane.col_index) else {
            continue;
        };
        canvas.fill(
            Rect::new(pane.rect.right() - 1, pane.rect.y, 1, pane.rect.height),
            palette.hairline,
        );
        for row_slot in &pane.rows {
            let Some(entry) = col.entries.get(row_slot.row_index) else {
                continue;
            };
            let is_col_selected = col
                .selected_path
                .as_deref()
                .is_some_and(|sel| sel.eq_ignore_ascii_case(&entry.path));
            let active_accent = is_col_selected && pane.col_index == last_col_idx;
            let hovered = matches!(
                state.hovered,
                Some(FinderHit::Item { path, col_index: Some(ci) })
                    if *ci == pane.col_index && path.eq_ignore_ascii_case(&entry.path)
            );
            if active_accent {
                canvas.fill_round(row_slot.rect, radius, palette.accent);
            } else if is_col_selected {
                canvas.fill_round(row_slot.rect, radius, palette.selection_fill);
            } else if hovered {
                canvas.fill_round(row_slot.rect, radius, palette.hover);
            }
            let icon_r = Rect::new(
                row_slot.rect.x + scale(6, dpi),
                row_slot.rect.y + (row_slot.rect.height - icon_sz) / 2,
                icon_sz,
                icon_sz,
            );
            draw_entry_thumb_or_glyph(canvas, thumbs, entry, icon_r, palette);
            let fg = if active_accent {
                palette.on_accent
            } else {
                palette.text
            };
            let text_x = icon_r.right() + scale(8, dpi);
            let text_w = (row_slot.rect.right() - text_x - scale(20, dpi)).max(1);
            canvas.text(
                &entry.name,
                &formats.row,
                Rect::new(text_x, row_slot.rect.y, text_w, row_slot.rect.height),
                fg,
            );
            if entry.is_dir() {
                draw_chevron(
                    canvas,
                    Rect::new(
                        row_slot.rect.right() - scale(18, dpi),
                        row_slot.rect.y + (row_slot.rect.height - scale(14, dpi)) / 2,
                        scale(14, dpi),
                        scale(14, dpi),
                    ),
                    false,
                    if active_accent {
                        palette.on_accent
                    } else {
                        palette.muted
                    },
                );
            }
        }
    }

    if let Some(preview_rect) = layout.column_preview_rect {
        let pad = scale(18, dpi);
        let inner = Rect::new(
            preview_rect.x + pad,
            preview_rect.y + pad,
            (preview_rect.width - pad * 2).max(1),
            (preview_rect.height - pad * 2).max(1),
        );
        draw_preview_card_body(
            canvas,
            formats,
            preview_bmp,
            &state.state.preview,
            inner,
            dpi,
            palette,
        );
    }
    canvas.pop_clip();
}

fn draw_gallery_view(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    thumbs: &HashMap<String, ID2D1Bitmap>,
    preview_bmp: Option<&ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    let tab = state.state.tab();
    canvas.push_clip(layout.content_stage);

    if let Some(stage_r) = layout.gallery_stage_rect {
        let pad = scale(24, dpi);
        let inner = Rect::new(
            stage_r.x + pad,
            stage_r.y + pad,
            (stage_r.width - pad * 2).max(1),
            (stage_r.height - pad * 2).max(1),
        );
        draw_preview_card_body(
            canvas,
            formats,
            preview_bmp,
            &state.state.preview,
            inner,
            dpi,
            palette,
        );
    }

    if let Some(strip) = layout.gallery_strip_rect {
        canvas.fill(strip, palette.toolbar);
        canvas.fill(
            Rect::new(strip.x, strip.y, strip.width, 1),
            palette.hairline,
        );
        let radius = scale(8, dpi) as f32;
        for slot in &layout.gallery_thumbs {
            let Some(entry) = tab.entries.get(slot.index) else {
                continue;
            };
            let selected = is_selected(state, &entry.path);
            let hovered = matches!(
                state.hovered,
                Some(FinderHit::Item { path, .. }) if path.eq_ignore_ascii_case(&entry.path)
            );
            if selected {
                canvas.fill_round(slot.rect, radius, palette.selection_fill);
                canvas.stroke_round(slot.rect, radius, 2.0, palette.accent);
            } else if hovered {
                canvas.fill_round(slot.rect, radius, palette.hover);
            }
            let icon_pad = scale(12, dpi);
            let icon_r = Rect::new(
                slot.rect.x + icon_pad,
                slot.rect.y + scale(6, dpi),
                (slot.rect.width - icon_pad * 2).max(8),
                (slot.rect.height - scale(26, dpi)).max(8),
            );
            draw_entry_thumb_or_glyph(canvas, thumbs, entry, icon_r, palette);
            canvas.text(
                &entry.name,
                &formats.small_center,
                Rect::new(
                    slot.rect.x + 2,
                    slot.rect.bottom() - scale(18, dpi),
                    slot.rect.width - 4,
                    scale(16, dpi),
                ),
                palette.text,
            );
        }
    }
    canvas.pop_clip();
}
