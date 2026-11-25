use otto_core::{
    FinderEntryKind, FinderHit, FinderPreviewData, FinderPreviewKind, Rect, scale,
    tag_mask_contains,
};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;

use crate::appearance::tokens::rgba_hex as rgba;
use crate::kit::canvas::HwndCanvas;

use super::icons::draw_entry_icon;
use super::palette::{FinderFormats, FinderPaintState, FinderPalette};

pub(super) fn draw_preview_card_body(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    preview_bmp: Option<&ID2D1Bitmap>,
    preview: &FinderPreviewData,
    area: Rect,
    dpi: u32,
    palette: &FinderPalette,
) {
    if preview.path.is_empty() {
        canvas.text(
            "Select an item to preview",
            &formats.small_center,
            area,
            palette.muted,
        );
        return;
    }
    let visual_h = (area.height * 55 / 100).clamp(scale(92, dpi), scale(260, dpi));
    let visual_rect = Rect::new(area.x, area.y, area.width, visual_h);
    let r = scale(10, dpi) as f32;
    canvas.fill_round(visual_rect, r, palette.card);
    canvas.stroke_round(visual_rect, r, 1.0, palette.hairline);

    if let Some(bmp) = preview_bmp {
        let fit = fit_square_in(visual_rect, scale(14, dpi));
        canvas.draw_bitmap(bmp, fit);
    } else if preview.kind == FinderPreviewKind::Text
        && let Some(text) = &preview.text_snippet
    {
        let pad = scale(12, dpi);
        let text_r = Rect::new(
            visual_rect.x + pad,
            visual_rect.y + pad,
            (visual_rect.width - pad * 2).max(1),
            (visual_rect.height - pad * 2).max(1),
        );
        canvas.text(text, &formats.code_wrap, text_r, palette.text);
    } else {
        let icon_sz = scale(72, dpi)
            .min(visual_rect.width / 2)
            .min(visual_rect.height - 20);
        let icon_r = Rect::new(
            visual_rect.x + (visual_rect.width - icon_sz) / 2,
            visual_rect.y + (visual_rect.height - icon_sz) / 2,
            icon_sz,
            icon_sz,
        );
        let kind = if preview.kind == FinderPreviewKind::Folder {
            FinderEntryKind::Folder
        } else {
            FinderEntryKind::File
        };
        draw_entry_icon(canvas, icon_r, kind, palette);
    }

    let mut y = visual_rect.bottom() + scale(12, dpi);
    let line_h = scale(20, dpi);
    canvas.text(
        &preview.name,
        &formats.title,
        Rect::new(area.x, y, area.width, scale(24, dpi)),
        palette.text,
    );
    y += scale(26, dpi);
    let meta_line = format!("{} — {}", preview.kind_label, preview.size_label);
    canvas.text(
        &meta_line,
        &formats.small,
        Rect::new(area.x, y, area.width, line_h),
        palette.secondary,
    );
    y += line_h + scale(4, dpi);
    let rows = [
        ("Modified", preview.modified_label.as_str()),
        ("Created", preview.created_label.as_str()),
    ];
    for (label, value) in rows {
        if y + line_h > area.bottom() {
            break;
        }
        let split = area.width / 3;
        canvas.text(
            label,
            &formats.small,
            Rect::new(area.x, y, split, line_h),
            palette.muted,
        );
        canvas.tabular_text(
            value,
            &formats.row_trailing,
            Rect::new(area.x + split, y, area.width - split, line_h),
            palette.text,
        );
        y += line_h;
    }
}

pub(super) fn draw_inspector(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    preview_bmp: Option<&ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let Some(insp) = state.layout.inspector else {
        return;
    };
    let dpi = state.layout.dpi;
    canvas.push_clip(insp.panel);
    canvas.fill(insp.panel, palette.sidebar);
    canvas.fill(
        Rect::new(insp.panel.x, insp.panel.y, 1, insp.panel.height),
        palette.hairline,
    );

    let r = scale(10, dpi) as f32;
    canvas.fill_round(insp.preview_rect, r, palette.card);
    canvas.stroke_round(insp.preview_rect, r, 1.0, palette.hairline);
    if let Some(bmp) = preview_bmp {
        canvas.draw_bitmap(bmp, fit_square_in(insp.preview_rect, scale(10, dpi)));
    } else if let Some(entry) = state.state.primary_selected_entry() {
        let sz = scale(64, dpi);
        draw_entry_icon(
            canvas,
            Rect::new(
                insp.preview_rect.x + (insp.preview_rect.width - sz) / 2,
                insp.preview_rect.y + (insp.preview_rect.height - sz) / 2,
                sz,
                sz,
            ),
            entry.kind,
            palette,
        );
    }

    let pad = scale(16, dpi);
    let inner_w = (insp.panel.width - pad * 2).max(1);
    let title = state
        .state
        .primary_selected_entry()
        .map_or_else(|| state.state.tab().title.clone(), |e| e.name);
    let subtitle = state.state.primary_selected_entry().map_or_else(
        || state.state.status_bar_label(),
        |e| format!("{} · {}", e.kind_label, e.size_label()),
    );
    canvas.text(
        &title,
        &formats.title,
        Rect::new(
            insp.panel.x + pad,
            insp.preview_rect.bottom() + scale(12, dpi),
            inner_w,
            scale(22, dpi),
        ),
        palette.text,
    );
    canvas.text(
        &subtitle,
        &formats.small,
        Rect::new(
            insp.panel.x + pad,
            insp.preview_rect.bottom() + scale(36, dpi),
            inner_w,
            scale(18, dpi),
        ),
        palette.secondary,
    );

    let active_mask = state
        .state
        .primary_selected_entry()
        .map_or(0, |entry| entry.tag_mask);
    for (tag, rect) in insp.tag_rects {
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        let rad = rect.width as f32 * 0.36;
        canvas.fill_circle(cx, cy, rad, rgba(tag.rgb(), 1.0));
        if tag_mask_contains(active_mask, tag) {
            canvas.stroke_circle(cx, cy, rad + 2.5, 1.6, palette.text);
        }
    }

    let info_y = insp.preview_rect.bottom() + scale(104, dpi);
    draw_inspector_footer(
        canvas,
        formats,
        state,
        &insp,
        (info_y, pad, inner_w),
        palette,
    );
    canvas.pop_clip();
}

fn draw_inspector_footer(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    insp: &otto_core::FinderInspectorLayout,
    metrics: (i32, i32, i32),
    palette: &FinderPalette,
) {
    let (info_y, pad, inner_w) = metrics;
    let dpi = state.layout.dpi;
    if let Some(entry) = state.state.primary_selected_entry() {
        let line_h = scale(20, dpi);
        let rows = [
            ("Modified", entry.modified_label.as_str()),
            ("Created", entry.created_label.as_str()),
            ("Path", entry.path.as_str()),
        ];
        for (idx, (k, v)) in rows.into_iter().enumerate() {
            let y = info_y + (idx as i32) * line_h;
            if y + line_h < insp.open_button.y - scale(8, dpi) {
                canvas.text(
                    k,
                    &formats.small,
                    Rect::new(insp.panel.x + pad, y, scale(66, dpi), line_h),
                    palette.muted,
                );
                canvas.text(
                    v,
                    &formats.small,
                    Rect::new(
                        insp.panel.x + pad + scale(68, dpi),
                        y,
                        (inner_w - scale(68, dpi)).max(1),
                        line_h,
                    ),
                    palette.text,
                );
            }
        }
    }
    let btn_r = scale(6, dpi) as f32;
    let buttons = [
        (insp.open_button, "Open", FinderHit::InspectorOpen, true),
        (
            insp.copy_path_button,
            "Copy Path",
            FinderHit::InspectorCopyPath,
            false,
        ),
    ];
    for (rect, label, hit, primary) in buttons {
        let hovered = state.hovered == Some(&hit);
        let fill = if primary {
            palette.accent
        } else if hovered {
            palette.pill_active
        } else {
            palette.pill
        };
        canvas.fill_round(rect, btn_r, fill);
        canvas.text(
            label,
            &formats.small_center,
            rect,
            if primary {
                palette.on_accent
            } else {
                palette.text
            },
        );
    }
}

pub(super) fn draw_quick_look(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    preview_bmp: Option<&ID2D1Bitmap>,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let Some(ql) = state.layout.quick_look else {
        return;
    };
    let dpi = state.layout.dpi;
    canvas.fill(ql.backdrop, palette.overlay_backdrop);
    let r = scale(14, dpi) as f32;
    canvas.fill_round(ql.card, r, palette.sheet);
    canvas.stroke_round(ql.card, r, 1.0, palette.hairline);
    canvas.fill(
        Rect::new(ql.header.x, ql.header.bottom() - 1, ql.header.width, 1),
        palette.hairline,
    );

    canvas.fill_round(ql.close_button, scale(6, dpi) as f32, palette.pill);
    canvas.text("x", &formats.small_center, ql.close_button, palette.text);

    let title_x = ql.close_button.right() + scale(10, dpi);
    canvas.text(
        &state.state.preview.name,
        &formats.title,
        Rect::new(
            title_x,
            ql.header.y,
            (ql.open_button.x - title_x - scale(10, dpi)).max(1),
            ql.header.height,
        ),
        palette.text,
    );

    canvas.fill_round(ql.open_button, scale(6, dpi) as f32, palette.accent);
    canvas.text(
        "Open App",
        &formats.small_center,
        ql.open_button,
        palette.on_accent,
    );

    canvas.fill_round(ql.prev_button, scale(14, dpi) as f32, palette.pill);
    canvas.text("<", &formats.row_medium, ql.prev_button, palette.text);
    canvas.fill_round(ql.next_button, scale(14, dpi) as f32, palette.pill);
    canvas.text(">", &formats.row_medium, ql.next_button, palette.text);

    draw_preview_card_body(
        canvas,
        formats,
        preview_bmp,
        &state.state.preview,
        ql.content,
        dpi,
        palette,
    );
}

pub(super) fn draw_go_to_overlay(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let Some(go_to) = state.layout.go_to else {
        return;
    };
    let dpi = state.layout.dpi;
    canvas.fill(
        Rect::new(0, 0, state.layout.width, state.layout.height),
        palette.overlay_backdrop,
    );
    let r = scale(12, dpi) as f32;
    canvas.fill_round(go_to.card, r, palette.sheet);
    canvas.stroke_round(go_to.card, r, 1.0, palette.hairline);
    let pad = scale(16, dpi);
    canvas.text(
        "Go to Folder",
        &formats.title,
        Rect::new(
            go_to.card.x + pad,
            go_to.card.y + scale(12, dpi),
            go_to.card.width - pad * 2,
            scale(24, dpi),
        ),
        palette.text,
    );
    canvas.fill_round(go_to.input_rect, scale(6, dpi) as f32, palette.pill);
    canvas.stroke_round(go_to.input_rect, scale(6, dpi) as f32, 1.5, palette.accent);
    let text = format!("{}|", state.state.go_to.input);
    canvas.text(
        &text,
        &formats.row,
        Rect::new(
            go_to.input_rect.x + scale(8, dpi),
            go_to.input_rect.y,
            (go_to.input_rect.width - scale(16, dpi)).max(1),
            go_to.input_rect.height,
        ),
        palette.text,
    );
    if let Some(err) = &state.state.go_to.error {
        canvas.text(
            err,
            &formats.small,
            Rect::new(
                go_to.input_rect.x,
                go_to.input_rect.bottom() + scale(4, dpi),
                go_to.cancel_button.x - go_to.input_rect.x - scale(8, dpi),
                scale(20, dpi),
            ),
            palette.danger,
        );
    }
    canvas.fill_round(go_to.cancel_button, scale(6, dpi) as f32, palette.pill);
    canvas.text(
        "Cancel",
        &formats.small_center,
        go_to.cancel_button,
        palette.text,
    );
    canvas.fill_round(go_to.go_button, scale(6, dpi) as f32, palette.accent);
    canvas.text(
        "Go",
        &formats.small_center,
        go_to.go_button,
        palette.on_accent,
    );
}

fn fit_square_in(container: Rect, pad: i32) -> Rect {
    let side = (container.width - pad * 2)
        .min(container.height - pad * 2)
        .max(16);
    Rect::new(
        container.x + (container.width - side) / 2,
        container.y + (container.height - side) / 2,
        side,
        side,
    )
}
