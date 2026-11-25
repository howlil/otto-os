use otto_core::{
    ControlCenterHitTarget, ControlCenterLayout, ControlCenterSnapshot, ControlKind, Rect,
};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::ID2D1SolidColorBrush;

use crate::appearance::tokens::{FOCUS_PURPLE, SYSTEM_RED_LIGHT, with_alpha};
use crate::kit::draw::{fill_squircle_or_rounded_f, pop_axis_clip_on, push_axis_clip_on};
use crate::{
    CHARGING_GREEN, LiquidGlassPalette, draw_speaker_glyph, draw_wifi_glyph, rect_to_d2d, scale,
};

use super::{ControlCenterPaintState, ControlCenterRenderer};

impl ControlCenterRenderer {
    pub(super) fn draw_gallery_page(
        &self,
        layout: &ControlCenterLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        self.draw_subpanel_header(layout, snapshot, state, palette);
        let vp = layout.list_viewport;
        if vp.is_empty() {
            return;
        }
        push_axis_clip_on(&self.target, rect_to_d2d(vp));
        for group in &layout.groups {
            self.draw_gallery_group(layout, group, snapshot, state, palette);
        }
        if layout.max_scroll > 0 {
            self.draw_list_scrollbar(vp, layout.scroll_offset, layout.max_scroll, palette);
        }
        pop_axis_clip_on(&self.target);
    }

    fn draw_gallery_group(
        &self,
        layout: &ControlCenterLayout,
        group: &otto_core::SubPanelGroupLayout,
        snapshot: &ControlCenterSnapshot,
        state: ControlCenterPaintState,
        palette: &LiquidGlassPalette,
    ) {
        let vp = layout.list_viewport;
        if !group.header.is_empty()
            && group.header.bottom() >= vp.y
            && group.header.y <= vp.bottom()
        {
            let hdr_r = D2D_RECT_F {
                left: group.header.x as f32 + scale(6, self.dpi) as f32,
                top: group.header.y as f32 + scale(5, self.dpi) as f32,
                right: group.header.right() as f32,
                bottom: group.header.bottom() as f32 - scale(3, self.dpi) as f32,
            };
            self.draw_text(
                group.title,
                &self.subtitle_format,
                hdr_r,
                &self.brushes.text_muted,
            );
        }
        if group.container.bottom() < vp.y || group.container.y > vp.bottom() {
            return;
        }
        let group_radius = layout.card_radius as f32;
        self.draw_card(group.container, group_radius, false, false, palette);
        for offset in 0..group.count {
            let index = group.start_index + offset;
            let Some(rect) = layout.items.get(index).copied() else {
                break;
            };
            if rect.bottom() < vp.y || rect.y > vp.bottom() {
                continue;
            }
            if offset > 0 {
                let sep = self.brushes.color_brush(palette.menu_separator);
                let inset = scale(12, self.dpi) as f32;
                crate::kit::draw::draw_line_on(
                    &self.target,
                    windows_numerics::Vector2::new(rect.x as f32 + inset, rect.y as f32),
                    windows_numerics::Vector2::new(rect.right() as f32 - inset, rect.y as f32),
                    (&sep).into(),
                    1.0,
                    None,
                );
            }
            let Some(&kind) = ControlKind::ALL.get(index) else {
                continue;
            };
            let added = snapshot.grid.contains_kind(kind);
            let target = ControlCenterHitTarget::GalleryItem(index);
            if !added {
                self.draw_interaction_overlay(
                    rect,
                    0.0,
                    state.hovered == Some(target),
                    state.pressed == Some(target),
                    palette,
                );
            }
            self.draw_gallery_row(rect, kind, added, palette);
        }
    }

    fn draw_gallery_row(
        &self,
        rect: Rect,
        kind: ControlKind,
        added: bool,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(12, self.dpi) as f32;
        let icon_d = scale(34, self.dpi);
        let icon_rect = Rect {
            x: rect.x + scale(12, self.dpi),
            y: rect.y + (rect.height - icon_d) / 2,
            width: icon_d,
            height: icon_d,
        };
        let badge_color = gallery_badge_color(kind, palette);
        let badge_brush = self.brushes.color_brush(badge_color);
        fill_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            rect_to_d2d(icon_rect),
            icon_d as f32 * 0.34,
            self.dpi,
            (&badge_brush).into(),
        );
        let cx = icon_rect.x as f32 + icon_d as f32 * 0.5;
        let cy = icon_rect.y as f32 + icon_d as f32 * 0.5;
        self.draw_control_kind_glyph(kind, cx, cy, icon_d as f32 * 0.32, &self.brushes.on_accent);

        let pill_w = scale(62, self.dpi) as f32;
        let pill_h = scale(24, self.dpi) as f32;
        let pill_r = D2D_RECT_F {
            left: rect.right() as f32 - pad - pill_w,
            top: rect.y as f32 + (rect.height as f32 - pill_h) * 0.5,
            right: rect.right() as f32 - pad,
            bottom: rect.y as f32 + (rect.height as f32 + pill_h) * 0.5,
        };
        let pill_bg = self.brushes.color_brush(if added {
            palette.toggle_inactive
        } else {
            palette.toggle_active
        });
        fill_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            pill_r,
            pill_h * 0.5,
            self.dpi,
            (&pill_bg).into(),
        );
        let pill_text = if added {
            &self.brushes.text_muted
        } else {
            &self.brushes.on_accent
        };
        self.draw_text(
            if added { "Added" } else { "+ Add" },
            &self.subtitle_format,
            D2D_RECT_F {
                left: pill_r.left + scale(10, self.dpi) as f32,
                ..pill_r
            },
            pill_text,
        );

        let text_left = icon_rect.right() as f32 + scale(10, self.dpi) as f32;
        let text_right = (pill_r.left - scale(8, self.dpi) as f32).max(text_left + 1.0);
        let mid = rect.y as f32 + rect.height as f32 * 0.5;
        let sizes = kind.supported_sizes_summary();
        let subtitle = format!("{} · {}", kind.subtitle(), sizes);
        self.draw_two_line_label(
            text_left,
            text_right,
            mid,
            kind.label(),
            &subtitle,
            &self.brushes.text_primary,
            &self.brushes.text_muted,
            false,
        );
    }

    pub(super) fn draw_control_kind_glyph(
        &self,
        kind: ControlKind,
        cx: f32,
        cy: f32,
        half: f32,
        brush: &ID2D1SolidColorBrush,
    ) {
        let stroke = (half * 0.22).max(1.3);
        match kind {
            ControlKind::Connectivity | ControlKind::Wifi => {
                let _ = draw_wifi_glyph(
                    &self.factory,
                    &self.target,
                    &self.stroke_style,
                    cx,
                    cy,
                    half * 1.15,
                    stroke,
                    brush,
                );
            }
            ControlKind::Bluetooth => {
                let _ = self.draw_bluetooth_glyph(cx, cy, half * 0.95, brush);
            }
            ControlKind::NowPlaying => self.draw_note_glyph(cx, cy, half * 0.95, brush),
            ControlKind::Brightness => self.draw_sun_glyph(cx, cy, half * 0.95, brush),
            ControlKind::Volume | ControlKind::Mute => {
                let _ = draw_speaker_glyph(
                    &self.factory,
                    &self.target,
                    &self.stroke_style,
                    cx - half * 0.42,
                    cy,
                    half * 0.72,
                    stroke,
                    if kind == ControlKind::Mute { 0 } else { 75 },
                    kind == ControlKind::Mute,
                    brush,
                );
            }
            ControlKind::Focus => {
                let _ = crate::draw_crescent_moon(
                    &self.factory,
                    &self.target,
                    windows_numerics::Vector2::new(cx, cy),
                    half * 0.9,
                    brush,
                );
            }
            ControlKind::Battery => self.draw_battery_glyph(cx, cy, half, 0.75, brush),
            ControlKind::NightLight => self.draw_night_light_glyph(cx, cy, half, brush),
            ControlKind::Appearance => self.draw_theme_glyph(cx, cy, half, brush),
            ControlKind::Screenshot => self.draw_screenshot_glyph(cx, cy, half, brush),
            ControlKind::LockScreen => self.draw_lock_glyph(cx, cy, half, brush),
            ControlKind::TaskView => self.draw_task_view_glyph(cx, cy, half, brush),
            ControlKind::Finder => self.draw_finder_glyph(cx, cy, half, brush),
            ControlKind::Spotlight => self.draw_spotlight_glyph(cx, cy, half, brush),
            ControlKind::Settings => self.draw_settings_glyph(cx, cy, half, brush),
        }
    }
}

fn gallery_badge_color(kind: ControlKind, palette: &LiquidGlassPalette) -> D2D1_COLOR_F {
    if palette.is_high_contrast {
        return palette.toggle_active;
    }
    match kind {
        ControlKind::Connectivity | ControlKind::Wifi | ControlKind::Bluetooth => {
            palette.toggle_active
        }
        ControlKind::Focus => FOCUS_PURPLE,
        ControlKind::Battery => CHARGING_GREEN,
        ControlKind::NowPlaying | ControlKind::Mute => SYSTEM_RED_LIGHT,
        ControlKind::NightLight | ControlKind::Brightness => D2D1_COLOR_F {
            r: 0.98,
            g: 0.64,
            b: 0.16,
            a: 1.0,
        },
        _ => with_alpha(palette.toggle_active, 0.85),
    }
}
