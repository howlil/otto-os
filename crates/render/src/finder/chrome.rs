use otto_core::{
    FINDER_ICON_SIZE_MAX, FINDER_ICON_SIZE_MIN, FinderFlags, FinderHit, FinderLocation,
    FinderToolbarButton, FinderViewMode, Rect, scale,
};

use crate::appearance::tokens::with_alpha;
use crate::kit::canvas::HwndCanvas;
use crate::window::controls::{HighContrastTrafficLights, TrafficLightsStyle};

use super::icons::{draw_chevron, draw_sidebar_icon, draw_toolbar_glyph};
use super::palette::{FinderFormats, FinderPaintState, FinderPalette};

pub(super) fn draw_toolbar(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    canvas.fill(layout.toolbar, palette.toolbar);
    canvas.fill(
        Rect::new(0, layout.toolbar.bottom() - 1, layout.width, 1),
        palette.hairline,
    );

    if layout.window_controls.len() == 3 {
        let slots = [
            (
                layout.window_controls[0].action,
                layout.window_controls[0].rect,
            ),
            (
                layout.window_controls[1].action,
                layout.window_controls[1].rect,
            ),
            (
                layout.window_controls[2].action,
                layout.window_controls[2].rect,
            ),
        ];
        let high_contrast = state.high_contrast.then_some(HighContrastTrafficLights {
            window: palette.background,
            text: palette.text,
            disabled: palette.secondary,
        });
        canvas.draw_traffic_lights(
            &slots,
            scale(6, dpi) as f32,
            TrafficLightsStyle {
                state: state.window_controls.state,
                maximized: state.window_controls.maximized,
                hovered: state.window_controls.hovered,
                pressed: state.window_controls.pressed,
                dark_surface: state.dark,
                high_contrast,
            },
        );
    }

    let first_view = layout
        .toolbar_buttons
        .iter()
        .find(|s| s.button == FinderToolbarButton::ViewMode(FinderViewMode::Icon));
    let last_view = layout
        .toolbar_buttons
        .iter()
        .find(|s| s.button == FinderToolbarButton::ViewMode(FinderViewMode::Gallery));
    if let (Some(first), Some(last)) = (first_view, last_view) {
        let seg_box = Rect::new(
            first.rect.x,
            first.rect.y,
            last.rect.right() - first.rect.x,
            first.rect.height,
        );
        canvas.fill_round(seg_box, scale(7, dpi) as f32, palette.pill);
    }

    let radius = scale(6, dpi) as f32;
    for slot in &layout.toolbar_buttons {
        let hovered = state.hovered == Some(&FinderHit::Toolbar(slot.button));
        let is_view = matches!(slot.button, FinderToolbarButton::ViewMode(_));
        if slot.active && is_view {
            canvas.fill_round(
                Rect::new(
                    slot.rect.x + 2,
                    slot.rect.y + 2,
                    slot.rect.width - 4,
                    slot.rect.height - 4,
                ),
                (radius - 1.5).max(2.0),
                palette.pill_active,
            );
        } else if slot.active {
            canvas.fill_round(slot.rect, radius, palette.selection_fill);
        } else if hovered && slot.enabled {
            canvas.fill_round(slot.rect, radius, palette.hover);
        }
        let color = if !slot.enabled {
            palette.muted
        } else if slot.active && !is_view {
            palette.accent
        } else {
            palette.text
        };
        draw_toolbar_glyph(canvas, slot.rect, slot.button, color);
    }

    let title = &state.state.tab().title;
    canvas.text(title, &formats.title, layout.title_rect, palette.text);

    draw_search_box(canvas, formats, state, palette);
}

fn draw_search_box(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    let box_r = layout.search_box;
    let focused = state.state.flags.has(FinderFlags::SEARCH_FOCUSED);
    let radius = scale(7, dpi) as f32;
    canvas.fill_round(box_r, radius, palette.pill);
    canvas.stroke_round(
        box_r,
        radius,
        if focused { 1.5 } else { 1.0 },
        if focused {
            palette.accent
        } else {
            palette.hairline
        },
    );
    let mag_r = scale(4, dpi) as f32;
    let mag_x = box_r.x as f32 + scale(12, dpi) as f32;
    let mag_y = box_r.y as f32 + box_r.height as f32 * 0.46;
    canvas.stroke_circle(mag_x, mag_y, mag_r, 1.3, palette.secondary);
    canvas.line(
        mag_x + mag_r * 0.72,
        mag_y + mag_r * 0.72,
        mag_x + mag_r * 1.35,
        mag_y + mag_r * 1.35,
        1.3,
        palette.secondary,
    );
    let query = &state.state.tab().search_query;
    let text_x = box_r.x + scale(24, dpi);
    let right_limit = layout.search_scope_pill.map_or_else(
        || box_r.right() - scale(8, dpi),
        |pill| pill.x - scale(4, dpi),
    );
    let text_rect = Rect::new(text_x, box_r.y, (right_limit - text_x).max(1), box_r.height);
    if query.is_empty() {
        canvas.text("Search", &formats.small, text_rect, palette.muted);
    } else {
        let display = if focused {
            format!("{query}|")
        } else {
            query.clone()
        };
        canvas.text(&display, &formats.small, text_rect, palette.text);
    }
    if let Some(pill) = layout.search_scope_pill {
        let recursive = state.state.flags.has(FinderFlags::SEARCH_RECURSIVE);
        canvas.fill_round(
            pill,
            scale(4, dpi) as f32,
            if recursive {
                palette.accent
            } else {
                palette.hover
            },
        );
        canvas.text(
            if recursive { "Deep" } else { "Here" },
            &formats.badge,
            pill,
            if recursive {
                palette.on_accent
            } else {
                palette.secondary
            },
        );
    }
    if let Some(clear) = layout.search_clear {
        canvas.fill_circle(
            clear.x as f32 + clear.width as f32 * 0.5,
            clear.y as f32 + clear.height as f32 * 0.5,
            clear.width as f32 * 0.42,
            palette.hover,
        );
        canvas.text("x", &formats.badge, clear, palette.secondary);
    }
}

pub(super) fn draw_sidebar(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    if layout.sidebar.width <= 0 {
        return;
    }
    let dpi = layout.dpi;
    canvas.push_clip(layout.sidebar);
    canvas.fill(layout.sidebar, palette.sidebar);
    canvas.fill(
        Rect::new(
            layout.sidebar.right() - 1,
            layout.sidebar.y,
            1,
            layout.sidebar.height,
        ),
        palette.hairline,
    );
    for heading in &layout.sidebar_headings {
        canvas.text(
            heading.section.title(),
            &formats.section,
            heading.rect,
            palette.muted,
        );
    }
    let radius = scale(7, dpi) as f32;
    let icon_sz = scale(18, dpi);
    for slot in &layout.sidebar_slots {
        let Some(item) = state.state.sidebar_items.get(slot.index) else {
            continue;
        };
        let hovered = state.hovered == Some(&FinderHit::SidebarItem(slot.index));
        if slot.active {
            canvas.fill_round(slot.rect, radius, palette.selection_fill);
        } else if hovered {
            canvas.fill_round(slot.rect, radius, palette.hover);
        }
        let icon_rect = Rect::new(
            slot.rect.x + scale(8, dpi),
            slot.rect.y + (slot.rect.height - icon_sz) / 2,
            icon_sz,
            icon_sz,
        );
        let icon_color = if slot.active {
            palette.accent
        } else {
            palette.secondary
        };
        draw_sidebar_icon(canvas, icon_rect, item.icon, icon_color);
        let label_x = icon_rect.right() + scale(8, dpi);
        let label_rect = Rect::new(
            label_x,
            slot.rect.y,
            (slot.rect.right() - label_x - scale(6, dpi)).max(1),
            slot.rect.height,
        );
        canvas.text(
            &item.label,
            if slot.active {
                &formats.row_medium
            } else {
                &formats.row
            },
            label_rect,
            palette.text,
        );
    }
    canvas.pop_clip();
}

pub(super) fn draw_tabs(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    if layout.tab_bar.is_empty() {
        return;
    }
    let dpi = layout.dpi;
    canvas.fill(layout.tab_bar, palette.sidebar);
    canvas.fill(
        Rect::new(
            layout.tab_bar.x,
            layout.tab_bar.bottom() - 1,
            layout.tab_bar.width,
            1,
        ),
        palette.hairline,
    );
    for slot in &layout.tab_slots {
        let Some(tab) = state.state.tabs.get(slot.index) else {
            continue;
        };
        let hovered = state.hovered == Some(&FinderHit::Tab(slot.index))
            || state.hovered == Some(&FinderHit::TabClose(slot.index));
        if slot.active {
            canvas.fill(slot.rect, palette.content);
        } else if hovered {
            canvas.fill(slot.rect, palette.hover);
        }
        canvas.fill(
            Rect::new(
                slot.rect.right() - 1,
                slot.rect.y + 4,
                1,
                slot.rect.height - 8,
            ),
            palette.hairline,
        );
        if hovered || slot.active {
            canvas.text("x", &formats.badge, slot.close_rect, palette.secondary);
        }
        let title_x = slot.close_rect.right() + scale(4, dpi);
        canvas.text(
            &tab.title,
            &formats.small_center,
            Rect::new(
                title_x,
                slot.rect.y,
                (slot.rect.right() - title_x - scale(14, dpi)).max(1),
                slot.rect.height,
            ),
            if slot.active {
                palette.text
            } else {
                palette.secondary
            },
        );
    }
    if state.hovered == Some(&FinderHit::TabAdd) {
        canvas.fill(layout.tab_add_button, palette.hover);
    }
    canvas.text(
        "+",
        &formats.row_medium,
        layout.tab_add_button,
        palette.secondary,
    );
}

pub(super) fn draw_banner(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let Some(banner) = state.layout.banner else {
        return;
    };
    let dpi = state.layout.dpi;
    let is_perm = state.state.flags.has(FinderFlags::CONFIRM_PERM_DELETE);
    let bg = if is_perm {
        with_alpha(palette.danger, 0.16)
    } else {
        palette.toolbar
    };
    canvas.fill(banner.rect, bg);
    canvas.fill(
        Rect::new(
            banner.rect.x,
            banner.rect.bottom() - 1,
            banner.rect.width,
            1,
        ),
        palette.hairline,
    );
    let text_rect = Rect::new(
        banner.rect.x + scale(14, dpi),
        banner.rect.y,
        (banner.secondary_button.x - banner.rect.x - scale(20, dpi)).max(1),
        banner.rect.height,
    );
    let (msg, primary, secondary) = if is_perm {
        (
            format!(
                "Permanently delete {} selected item(s)? This cannot be undone.",
                state.state.tab().selected_paths.len()
            ),
            "Delete Forever",
            "Cancel",
        )
    } else if state.state.tab().location == FinderLocation::Trash {
        (
            String::from("Trash — items can be restored or emptied permanently."),
            "Empty Trash",
            "Put Back",
        )
    } else {
        return;
    };
    canvas.text(&msg, &formats.small, text_rect, palette.text);
    let r = scale(5, dpi) as f32;
    canvas.fill_round(banner.secondary_button, r, palette.pill);
    canvas.text(
        secondary,
        &formats.small_center,
        banner.secondary_button,
        palette.text,
    );
    canvas.fill_round(
        banner.primary_button,
        r,
        if is_perm {
            palette.danger
        } else {
            palette.accent
        },
    );
    canvas.text(
        primary,
        &formats.small_center,
        banner.primary_button,
        palette.on_accent,
    );
}

pub(super) fn draw_bottom_bars(
    canvas: &HwndCanvas,
    formats: &FinderFormats,
    state: &FinderPaintState<'_>,
    palette: &FinderPalette,
) {
    let layout = state.layout;
    let dpi = layout.dpi;
    if !layout.path_bar.is_empty() {
        canvas.fill(layout.path_bar, palette.toolbar);
        canvas.fill(
            Rect::new(
                layout.path_bar.x,
                layout.path_bar.y,
                layout.path_bar.width,
                1,
            ),
            palette.hairline,
        );
        for (i, slot) in layout.breadcrumbs.iter().enumerate() {
            let hovered = state.hovered == Some(&FinderHit::Breadcrumb(slot.index));
            if hovered {
                canvas.fill_round(
                    Rect::new(
                        slot.rect.x,
                        slot.rect.y + 3,
                        slot.rect.width,
                        (slot.rect.height - 6).max(1),
                    ),
                    scale(4, dpi) as f32,
                    palette.hover,
                );
            }
            canvas.text(
                &slot.breadcrumb.label,
                &formats.small_center,
                slot.rect,
                if i + 1 == layout.breadcrumbs.len() {
                    palette.text
                } else {
                    palette.secondary
                },
            );
            if i + 1 < layout.breadcrumbs.len() {
                draw_chevron(
                    canvas,
                    Rect::new(
                        slot.rect.right() + 2,
                        slot.rect.y + 4,
                        scale(12, dpi),
                        scale(16, dpi),
                    ),
                    false,
                    palette.muted,
                );
            }
        }
    }
    if !layout.status_bar.is_empty() {
        canvas.fill(layout.status_bar, palette.toolbar);
        canvas.fill(
            Rect::new(
                layout.status_bar.x,
                layout.status_bar.y,
                layout.status_bar.width,
                1,
            ),
            palette.hairline,
        );
        let status = state.state.status_bar_label();
        canvas.tabular_text(
            &status,
            &formats.small_center,
            layout.status_bar,
            palette.secondary,
        );
        if let Some(slider) = layout.icon_size_slider {
            let cy = slider.y as f32 + slider.height as f32 * 0.5;
            canvas.line(
                slider.x as f32,
                cy,
                slider.right() as f32,
                cy,
                2.0,
                palette.hairline,
            );
            let frac = (state.state.metrics.icon_size - FINDER_ICON_SIZE_MIN) as f32
                / (FINDER_ICON_SIZE_MAX - FINDER_ICON_SIZE_MIN).max(1) as f32;
            let knob_x = slider.x as f32 + frac.clamp(0.0, 1.0) * slider.width as f32;
            canvas.fill_circle(knob_x, cy, scale(5, dpi) as f32, palette.accent);
        }
    }
}
