use otto_core::{FinderEntryKind, FinderSidebarIcon, FinderToolbarButton, FinderViewMode, Rect};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use crate::appearance::tokens::{rgba_hex as rgba, with_alpha};
use crate::kit::canvas::HwndCanvas;

use super::palette::FinderPalette;

pub(super) fn draw_toolbar_glyph(
    canvas: &HwndCanvas,
    rect: Rect,
    button: FinderToolbarButton,
    color: D2D1_COLOR_F,
) {
    let cx = rect.x as f32 + rect.width as f32 * 0.5;
    let cy = rect.y as f32 + rect.height as f32 * 0.5;
    let s = (rect.width.min(rect.height) as f32 * 0.34).max(5.0);
    let sw = (s * 0.16).clamp(1.2, 2.0);
    match button {
        FinderToolbarButton::ToggleSidebar => {
            let box_r = Rect::new(
                (cx - s).round() as i32,
                (cy - s * 0.78).round() as i32,
                (s * 2.0).round() as i32,
                (s * 1.56).round() as i32,
            );
            canvas.stroke_round(box_r, 2.5, sw, color);
            let split = cx - s * 0.35;
            canvas.line(split, cy - s * 0.75, split, cy + s * 0.75, sw, color);
        }
        FinderToolbarButton::Back => {
            canvas.line(cx + s * 0.25, cy - s * 0.65, cx - s * 0.35, cy, sw, color);
            canvas.line(cx - s * 0.35, cy, cx + s * 0.25, cy + s * 0.65, sw, color);
        }
        FinderToolbarButton::Forward => {
            canvas.line(cx - s * 0.25, cy - s * 0.65, cx + s * 0.35, cy, sw, color);
            canvas.line(cx + s * 0.35, cy, cx - s * 0.25, cy + s * 0.65, sw, color);
        }
        FinderToolbarButton::ViewMode(mode) => {
            draw_view_mode_glyph(canvas, mode, (cx, cy, s, sw), color);
        }
        FinderToolbarButton::SortMenu => {
            canvas.line(
                cx - s * 0.35,
                cy - s * 0.65,
                cx - s * 0.35,
                cy + s * 0.65,
                sw,
                color,
            );
            canvas.line(
                cx - s * 0.68,
                cy - s * 0.28,
                cx - s * 0.35,
                cy - s * 0.65,
                sw,
                color,
            );
            canvas.line(
                cx + s * 0.35,
                cy - s * 0.65,
                cx + s * 0.35,
                cy + s * 0.65,
                sw,
                color,
            );
            canvas.line(
                cx + s * 0.68,
                cy + s * 0.28,
                cx + s * 0.35,
                cy + s * 0.65,
                sw,
                color,
            );
        }
        FinderToolbarButton::ActionMenu => {
            for i in -1..=1 {
                canvas.fill_circle(cx + (i as f32) * s * 0.62, cy, sw * 0.95, color);
            }
        }
        FinderToolbarButton::ToggleInspector => {
            canvas.stroke_circle(cx, cy, s * 0.82, sw, color);
            canvas.fill_circle(cx, cy - s * 0.35, sw * 0.65, color);
            canvas.line(cx, cy - s * 0.05, cx, cy + s * 0.45, sw, color);
        }
    }
}

fn draw_view_mode_glyph(
    canvas: &HwndCanvas,
    mode: FinderViewMode,
    geom: (f32, f32, f32, f32),
    color: D2D1_COLOR_F,
) {
    let (cx, cy, s, sw) = geom;
    match mode {
        FinderViewMode::Icon => {
            let cell = (s * 0.72).round() as i32;
            let gap = (s * 0.26).round() as i32;
            let ox = (cx - s * 0.85).round() as i32;
            let oy = (cy - s * 0.85).round() as i32;
            for r in 0..2 {
                for c in 0..2 {
                    canvas.stroke_round(
                        Rect::new(ox + c * (cell + gap), oy + r * (cell + gap), cell, cell),
                        1.5,
                        sw,
                        color,
                    );
                }
            }
        }
        FinderViewMode::List => {
            for i in -1..=1 {
                let y = cy + (i as f32) * s * 0.52;
                canvas.line(cx - s * 0.85, y, cx + s * 0.85, y, sw, color);
            }
        }
        FinderViewMode::Column => {
            let box_r = Rect::new(
                (cx - s).round() as i32,
                (cy - s * 0.78).round() as i32,
                (s * 2.0).round() as i32,
                (s * 1.56).round() as i32,
            );
            canvas.stroke_round(box_r, 2.0, sw, color);
            canvas.line(
                cx - s * 0.33,
                cy - s * 0.75,
                cx - s * 0.33,
                cy + s * 0.75,
                sw,
                color,
            );
            canvas.line(
                cx + s * 0.33,
                cy - s * 0.75,
                cx + s * 0.33,
                cy + s * 0.75,
                sw,
                color,
            );
        }
        FinderViewMode::Gallery => {
            let top_r = Rect::new(
                (cx - s * 0.85).round() as i32,
                (cy - s * 0.8).round() as i32,
                (s * 1.7).round() as i32,
                (s * 1.05).round() as i32,
            );
            canvas.stroke_round(top_r, 2.0, sw, color);
            let y = cy + s * 0.65;
            for i in -1..=1 {
                let x = cx + (i as f32) * s * 0.56;
                canvas.fill_circle(x, y, sw * 0.9, color);
            }
        }
    }
}

pub(super) fn draw_sidebar_icon(
    canvas: &HwndCanvas,
    rect: Rect,
    icon: FinderSidebarIcon,
    color: D2D1_COLOR_F,
) {
    let cx = rect.x as f32 + rect.width as f32 * 0.5;
    let cy = rect.y as f32 + rect.height as f32 * 0.5;
    let r = (rect.width.min(rect.height) as f32 * 0.38).max(4.5);
    let sw = 1.4;
    match icon {
        FinderSidebarIcon::Tag(tag) => {
            canvas.fill_circle(cx, cy, r * 0.72, rgba(tag.rgb(), 1.0));
        }
        FinderSidebarIcon::Recents => {
            canvas.stroke_circle(cx, cy, r, sw, color);
            canvas.line(cx, cy, cx, cy - r * 0.55, sw, color);
            canvas.line(cx, cy, cx + r * 0.45, cy + r * 0.15, sw, color);
        }
        FinderSidebarIcon::Downloads => {
            canvas.stroke_circle(cx, cy, r, sw, color);
            canvas.line(cx, cy - r * 0.48, cx, cy + r * 0.42, sw, color);
            canvas.line(cx - r * 0.34, cy + r * 0.1, cx, cy + r * 0.44, sw, color);
            canvas.line(cx + r * 0.34, cy + r * 0.1, cx, cy + r * 0.44, sw, color);
        }
        FinderSidebarIcon::Home => {
            canvas.line(cx - r, cy - r * 0.1, cx, cy - r * 0.92, sw, color);
            canvas.line(cx, cy - r * 0.92, cx + r, cy - r * 0.1, sw, color);
            canvas.stroke_round(
                Rect::new(
                    (cx - r * 0.68).round() as i32,
                    (cy - r * 0.12).round() as i32,
                    (r * 1.36).round() as i32,
                    (r * 1.02).round() as i32,
                ),
                1.5,
                sw,
                color,
            );
        }
        FinderSidebarIcon::Trash => {
            canvas.line(
                cx - r * 0.85,
                cy - r * 0.55,
                cx + r * 0.85,
                cy - r * 0.55,
                sw,
                color,
            );
            canvas.stroke_round(
                Rect::new(
                    (cx - r * 0.65).round() as i32,
                    (cy - r * 0.45).round() as i32,
                    (r * 1.3).round() as i32,
                    (r * 1.35).round() as i32,
                ),
                1.8,
                sw,
                color,
            );
        }
        FinderSidebarIcon::Drive | FinderSidebarIcon::Removable | FinderSidebarIcon::Computer => {
            canvas.stroke_round(
                Rect::new(
                    (cx - r).round() as i32,
                    (cy - r * 0.72).round() as i32,
                    (r * 2.0).round() as i32,
                    (r * 1.44).round() as i32,
                ),
                2.5,
                sw,
                color,
            );
            canvas.fill_circle(cx + r * 0.45, cy + r * 0.22, 1.5, color);
        }
        FinderSidebarIcon::Desktop
        | FinderSidebarIcon::Documents
        | FinderSidebarIcon::Pictures
        | FinderSidebarIcon::Music
        | FinderSidebarIcon::Videos
        | FinderSidebarIcon::Folder => {
            canvas.stroke_round(
                Rect::new(
                    (cx - r).round() as i32,
                    (cy - r * 0.68).round() as i32,
                    (r * 2.0).round() as i32,
                    (r * 1.44).round() as i32,
                ),
                2.2,
                sw,
                color,
            );
            canvas.line(
                cx - r * 0.9,
                cy - r * 0.28,
                cx + r * 0.9,
                cy - r * 0.28,
                sw * 0.9,
                color,
            );
        }
    }
}

pub(super) fn draw_entry_icon(
    canvas: &HwndCanvas,
    rect: Rect,
    kind: FinderEntryKind,
    palette: &FinderPalette,
) {
    let w = rect.width as f32;
    let h = rect.height as f32;
    let r = (w.min(h) * 0.14).clamp(2.5, 10.0);
    match kind {
        FinderEntryKind::Folder => {
            let tab_w = (w * 0.42).round() as i32;
            let tab_h = (h * 0.22).round() as i32;
            let top_y = rect.y + (h * 0.10).round() as i32;
            let body_y = rect.y + (h * 0.22).round() as i32;
            let body_h = (rect.bottom() - body_y - (h * 0.08).round() as i32).max(4);
            let back = rgba(0x3B_9CE6, 1.0);
            canvas.fill_round(Rect::new(rect.x, top_y, tab_w, tab_h + 4), r * 0.7, back);
            canvas.fill_round(
                Rect::new(rect.x, body_y, rect.width, body_h),
                r,
                palette.folder_blue,
            );
            canvas.fill(
                Rect::new(
                    rect.x + 2,
                    body_y + (body_h / 4),
                    (rect.width - 4).max(1),
                    (h * 0.04).max(1.0) as i32,
                ),
                rgba(0xFF_FFFF, 0.22),
            );
        }
        FinderEntryKind::Drive => {
            let body = Rect::new(
                rect.x,
                rect.y + (h * 0.2).round() as i32,
                rect.width,
                (h * 0.62).round() as i32,
            );
            canvas.fill_round(body, r, rgba(0x98_989D, 1.0));
            canvas.stroke_round(body, r, 1.0, rgba(0xFF_FFFF, 0.28));
            canvas.fill_circle(
                body.right() as f32 - w * 0.2,
                body.bottom() as f32 - h * 0.18,
                (w * 0.06).max(2.0),
                rgba(0x34_C759, 1.0),
            );
        }
        _ => {
            let inset_x = (w * 0.12).round() as i32;
            let sheet = Rect::new(
                rect.x + inset_x,
                rect.y + (h * 0.06).round() as i32,
                (rect.width - inset_x * 2).max(6),
                (h * 0.88).round() as i32,
            );
            let badge_color = match kind {
                FinderEntryKind::Image => rgba(0x34_C759, 1.0),
                FinderEntryKind::Audio => rgba(0xFF_2D55, 1.0),
                FinderEntryKind::Video => rgba(0xAF_52DE, 1.0),
                FinderEntryKind::Code => rgba(0x0A_84FF, 1.0),
                FinderEntryKind::Archive => rgba(0xFF_9500, 1.0),
                FinderEntryKind::App => rgba(0x58_56D6, 1.0),
                FinderEntryKind::Document => rgba(0x64_D2FF, 1.0),
                _ => rgba(0x8E_8E93, 1.0),
            };
            let paper = rgba(0xF7_F7FA, 0.96);
            canvas.fill_round(sheet, r * 0.8, paper);
            canvas.stroke_round(sheet, r * 0.8, 1.0, with_alpha(palette.text, 0.18));
            let stripe_h = (sheet.height / 4).max(4);
            canvas.fill_round(
                Rect::new(
                    sheet.x + 2,
                    sheet.bottom() - stripe_h - 2,
                    (sheet.width - 4).max(2),
                    stripe_h,
                ),
                (r * 0.4).max(1.5),
                badge_color,
            );
        }
    }
}

pub(super) fn draw_chevron(canvas: &HwndCanvas, rect: Rect, expanded: bool, color: D2D1_COLOR_F) {
    let cx = rect.x as f32 + rect.width as f32 * 0.5;
    let cy = rect.y as f32 + rect.height as f32 * 0.5;
    let d = (rect.width.min(rect.height) as f32 * 0.22).max(2.5);
    if expanded {
        canvas.line(cx - d, cy - d * 0.5, cx, cy + d * 0.6, 1.4, color);
        canvas.line(cx, cy + d * 0.6, cx + d, cy - d * 0.5, 1.4, color);
    } else {
        canvas.line(cx - d * 0.5, cy - d, cx + d * 0.6, cy, 1.4, color);
        canvas.line(cx + d * 0.6, cy, cx - d * 0.5, cy + d, 1.4, color);
    }
}
