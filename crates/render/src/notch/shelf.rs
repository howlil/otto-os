use otto_core::{
    HitTarget, Layout, MAX_SHELF_ITEMS, PixelLimits, Rect, ShelfItem, ShelfItemId, ShelfKindTag,
    ShelfState, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BRUSH_PROPERTIES,
    D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_ROUNDED_RECT, ID2D1Bitmap, ID2D1PathGeometry,
    ID2D1SolidColorBrush,
};
use windows::core::Result;
use windows_numerics::{Matrix3x2, Vector2};

use crate::appearance::surface::{fnv1a_hash, upload_bgra_bitmap};
use crate::appearance::tokens::SYSTEM_RED;
pub(crate) use crate::kit::draw::center_of;
use crate::kit::icons::draw_close_x_glyph;
use crate::{
    AMBER_WARN, CHARGING_GREEN, MUSIC_ACCENT, Renderer, build_squircle_path, rect_to_d2d, rgb,
    translate_rect,
};

const SHELF_DROP_BLUE: D2D1_COLOR_F = rgb(90, 178, 255, 1.0);
pub(crate) const SHELF_CLEAR_RED: D2D1_COLOR_F = SYSTEM_RED;
pub(crate) const SHELF_FOLDER_BLUE: D2D1_COLOR_F = rgb(94, 175, 255, 1.0);
const SHELF_AUDIO_VIOLET: D2D1_COLOR_F = rgb(175, 130, 255, 1.0);
const SHELF_CODE_TEAL: D2D1_COLOR_F = rgb(64, 212, 200, 1.0);
pub(crate) const SHELF_URL_INDIGO: D2D1_COLOR_F = rgb(100, 155, 255, 1.0);
pub(crate) const SHELF_TEXT_SLATE: D2D1_COLOR_F = rgb(198, 206, 220, 1.0);

pub(crate) struct CachedShelfBitmap {
    pub(crate) id: ShelfItemId,
    hash: u64,
    bytes_len: usize,
    bitmap: ID2D1Bitmap,
}

fn kind_tint(kind: ShelfKindTag) -> D2D1_COLOR_F {
    match kind {
        ShelfKindTag::Folder => SHELF_FOLDER_BLUE,
        ShelfKindTag::Image => CHARGING_GREEN,
        ShelfKindTag::Audio => SHELF_AUDIO_VIOLET,
        ShelfKindTag::Video => MUSIC_ACCENT,
        ShelfKindTag::Pdf => SHELF_CLEAR_RED,
        ShelfKindTag::Archive => AMBER_WARN,
        ShelfKindTag::Code => SHELF_CODE_TEAL,
        ShelfKindTag::Document | ShelfKindTag::Url => SHELF_URL_INDIGO,
        ShelfKindTag::Text => SHELF_TEXT_SLATE,
    }
}

impl Renderer {
    pub(crate) fn draw_expanded_shelf(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        shelf_state: &ShelfState,
    ) -> Result<()> {
        self.prune_shelf_bitmaps(shelf_state);
        self.draw_shelf_header_controls(layout, offset_x, alpha, hovered, shelf_state);

        let visible = shelf_state.visible_items();
        if visible.is_empty() {
            self.draw_shelf_empty_dropzone(layout, offset_x, alpha, shelf_state.drag_hover);
            return Ok(());
        }

        if shelf_state.drag_hover {
            let zone = translate_rect(layout.shelf.dropzone, -offset_x);
            let zone_radius = scale(12, self.dpi) as f32;
            let zone_shape = D2D1_ROUNDED_RECT {
                rect: rect_to_d2d(zone),
                radiusX: zone_radius,
                radiusY: zone_radius,
            };
            let bg = self.brushes.alpha_brush(SHELF_DROP_BLUE, 0.08 * alpha);
            let border = self.brushes.alpha_brush(SHELF_DROP_BLUE, 0.72 * alpha);
            unsafe {
                self.target.FillRoundedRectangle(&zone_shape, &bg);
                self.target.DrawRoundedRectangle(
                    &zone_shape,
                    &border,
                    1.5,
                    &self.dashed_stroke_style,
                );
            }
        }

        for (slot, item) in visible.iter().enumerate() {
            let Some(&raw_card) = layout.shelf.cards.get(slot) else {
                break;
            };
            if raw_card.is_empty() {
                continue;
            }
            let card = translate_rect(raw_card, -offset_x);
            let copy = layout
                .shelf
                .copy_buttons
                .get(slot)
                .copied()
                .map(|rect| translate_rect(rect, -offset_x))
                .unwrap_or_default();
            let remove = layout
                .shelf
                .remove_buttons
                .get(slot)
                .copied()
                .map(|rect| translate_rect(rect, -offset_x))
                .unwrap_or_default();
            self.draw_shelf_card(card, copy, remove, slot, item, alpha, hovered)?;
        }
        Ok(())
    }

    fn prune_shelf_bitmaps(&self, shelf_state: &ShelfState) {
        let mut cache = self.cached_shelf_bitmaps.borrow_mut();
        cache.retain(|entry| shelf_state.items.iter().any(|item| item.id == entry.id));
    }

    pub(crate) fn resolve_shelf_bitmap(&self, item: &ShelfItem) -> Option<ID2D1Bitmap> {
        let thumb = item.thumbnail.as_ref()?;
        let hash = fnv1a_hash(&thumb.bgra);
        let bytes_len = thumb.bgra.len();
        {
            let cache = self.cached_shelf_bitmaps.borrow();
            if let Some(existing) = cache.iter().find(|entry| {
                entry.id == item.id && entry.hash == hash && entry.bytes_len == bytes_len
            }) {
                return Some(existing.bitmap.clone());
            }
        }
        let bitmap = upload_bgra_bitmap(
            &self.target,
            thumb.width,
            thumb.height,
            &thumb.bgra,
            PixelLimits::ICON,
        )
        .ok()?;
        let mut cache = self.cached_shelf_bitmaps.borrow_mut();
        cache.retain(|entry| entry.id != item.id);
        while cache.len() >= MAX_SHELF_ITEMS {
            cache.remove(0);
        }
        cache.push(CachedShelfBitmap {
            id: item.id,
            hash,
            bytes_len,
            bitmap: bitmap.clone(),
        });
        Some(bitmap)
    }

    fn draw_shelf_header_controls(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        hovered: Option<HitTarget>,
        shelf_state: &ShelfState,
    ) {
        let ink = self.notch_ink();
        if !layout.shelf.pager_button.is_empty() {
            let pager = translate_rect(layout.shelf.pager_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::ShelfPager);
            let fill = self
                .brushes
                .alpha_brush(ink, if is_hovered { 0.17 } else { 0.09 } * alpha);
            let border = self.brushes.alpha_brush(ink, 0.15 * alpha);
            self.draw_shelf_pill(pager, &fill, Some(&border));
            let label = format!(
                "{}/{}",
                shelf_state.page + 1,
                shelf_state.page_count().max(1)
            );
            let fg = self.brushes.alpha_brush(ink, 0.90 * alpha);
            self.draw_tabular_text(&label, &self.text_tab, rect_to_d2d(pager), &fg);
        }

        if !layout.shelf.add_button.is_empty() {
            let add = translate_rect(layout.shelf.add_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::ShelfAdd);
            let fill = self
                .brushes
                .alpha_brush(ink, if is_hovered { 0.18 } else { 0.10 } * alpha);
            let border = self.brushes.alpha_brush(ink, 0.18 * alpha);
            self.draw_shelf_pill(add, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(ink, 0.95 * alpha);
            let plus_r = scale(4, self.dpi) as f32;
            let plus_cx = add.x as f32 + scale(10, self.dpi) as f32;
            let cy = add.y as f32 + add.height as f32 * 0.5;
            self.draw_plus_glyph(plus_cx, cy, plus_r, &fg);
            let text_rect = D2D_RECT_F {
                left: add.x as f32 + scale(15, self.dpi) as f32,
                top: add.y as f32,
                right: add.right() as f32 - scale(4, self.dpi) as f32,
                bottom: add.bottom() as f32,
            };
            self.draw_text("Add", &self.text_tab, text_rect, &fg);
        }

        if !layout.shelf.clear_button.is_empty() && !shelf_state.items.is_empty() {
            let clear = translate_rect(layout.shelf.clear_button, -offset_x);
            let is_hovered = hovered == Some(HitTarget::ShelfClear);
            let (fill, border, fg_color) = if is_hovered {
                (
                    self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.24 * alpha),
                    self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.52 * alpha),
                    SHELF_CLEAR_RED,
                )
            } else {
                (
                    self.brushes.alpha_brush(ink, 0.09 * alpha),
                    self.brushes.alpha_brush(ink, 0.15 * alpha),
                    ink,
                )
            };
            self.draw_shelf_pill(clear, &fill, Some(&border));
            let fg = self.brushes.alpha_brush(fg_color, 0.92 * alpha);
            self.draw_text("Clear", &self.text_tab, rect_to_d2d(clear), &fg);
        }
    }

    fn draw_shelf_empty_dropzone(
        &self,
        layout: &Layout,
        offset_x: i32,
        alpha: f32,
        drag_hover: bool,
    ) {
        let zone = translate_rect(layout.shelf.dropzone, -offset_x);
        if zone.width <= 0 || zone.height <= 0 {
            return;
        }
        let ink = self.notch_ink();
        let radius = scale(12, self.dpi) as f32;
        let (fill, border, stroke_w) = if drag_hover {
            (
                self.brushes.alpha_brush(SHELF_DROP_BLUE, 0.16 * alpha),
                self.brushes.alpha_brush(SHELF_DROP_BLUE, 0.90 * alpha),
                1.6,
            )
        } else {
            (
                self.brushes.alpha_brush(ink, 0.045 * alpha),
                self.brushes.alpha_brush(
                    ink,
                    if self.high_contrast.get() { 0.72 } else { 0.24 } * alpha,
                ),
                1.2,
            )
        };
        self.draw_rounded_box(zone, radius, &fill);
        let inset = stroke_w * 0.5;
        let stroke_rect = crate::kit::draw::inset_rect_f(rect_to_d2d(zone), inset);
        if let Ok(geom) = build_squircle_path(&self.factory, stroke_rect, (radius - inset).max(1.0))
        {
            unsafe {
                self.target
                    .DrawGeometry(&geom, &border, stroke_w, &self.dashed_stroke_style);
            }
        }

        let center = center_of(zone);
        let icon_color = if drag_hover { SHELF_DROP_BLUE } else { ink };
        let icon_brush = self
            .brushes
            .alpha_brush(icon_color, if drag_hover { 0.96 } else { 0.78 } * alpha);
        let icon_r = scale(8, self.dpi) as f32;
        self.draw_tray_glyph(
            Vector2::new(center.X, zone.y as f32 + scale(26, self.dpi) as f32),
            icon_r,
            &icon_brush,
        );

        let title = if drag_hover {
            "Release to keep in Pocket"
        } else {
            "Drop files, links, or text here"
        };
        let title_brush = self
            .brushes
            .alpha_brush(ink, if drag_hover { 0.98 } else { 0.90 } * alpha);
        let title_rect = D2D_RECT_F {
            left: zone.x as f32 + scale(12, self.dpi) as f32,
            top: zone.y as f32 + scale(42, self.dpi) as f32,
            right: zone.right() as f32 - scale(12, self.dpi) as f32,
            bottom: zone.y as f32 + scale(62, self.dpi) as f32,
        };
        self.draw_text(title, &self.text_tab, title_rect, &title_brush);

        let hint_brush = self.brushes.alpha_brush(ink, 0.54 * alpha);
        let hint_rect = D2D_RECT_F {
            left: zone.x as f32 + scale(16, self.dpi) as f32,
            top: zone.y as f32 + scale(62, self.dpi) as f32,
            right: zone.right() as f32 - scale(16, self.dpi) as f32,
            bottom: zone.bottom() as f32 - scale(6, self.dpi) as f32,
        };
        self.draw_text(
            "Click Add to browse or drag cards out",
            &self.text_caption_center,
            hint_rect,
            &hint_brush,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_shelf_card(
        &self,
        card: Rect,
        copy: Rect,
        remove: Rect,
        slot: usize,
        item: &ShelfItem,
        alpha: f32,
        hovered: Option<HitTarget>,
    ) -> Result<()> {
        let ink = self.notch_ink();
        let slot_index = u8::try_from(slot).ok();
        let card_hovered = slot_index.is_some_and(|idx| {
            matches!(
                hovered,
                Some(
                    HitTarget::ShelfItem(s)
                        | HitTarget::ShelfCopyItem(s)
                        | HitTarget::ShelfRemoveItem(s)
                ) if s == idx
            )
        });
        let card_radius = scale(10, self.dpi) as f32;
        let fill = self
            .brushes
            .alpha_brush(ink, if card_hovered { 0.13 } else { 0.075 } * alpha);
        let border = self
            .brushes
            .alpha_brush(ink, if card_hovered { 0.26 } else { 0.12 } * alpha);
        self.draw_rounded_box(card, card_radius, &fill);
        self.draw_rounded_box_stroke(card, card_radius, &border, 1.0);

        let tile_pad = scale(7, self.dpi);
        let tile_size = (card.height - tile_pad * 2).max(scale(22, self.dpi));
        let tile = Rect {
            x: card.x + tile_pad,
            y: card.y + (card.height - tile_size) / 2,
            width: tile_size,
            height: tile_size,
        };
        self.draw_shelf_tile(tile, item, alpha)?;

        let copy_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::ShelfCopyItem(idx)));
        let remove_hovered =
            slot_index.is_some_and(|idx| hovered == Some(HitTarget::ShelfRemoveItem(idx)));
        if !copy.is_empty() {
            self.draw_mini_copy_button(copy, copy_hovered, card_hovered, alpha);
        }
        if !remove.is_empty() {
            self.draw_mini_remove_button(remove, remove_hovered, card_hovered, alpha);
        }

        let text_left = tile.right() + scale(7, self.dpi);
        let title_right = if copy.is_empty() {
            card.right() - tile_pad
        } else {
            copy.x - scale(3, self.dpi)
        };
        if title_right > text_left + scale(12, self.dpi) {
            let fg = self.brushes.alpha_brush(ink, 0.94 * alpha);
            let title_rect = D2D_RECT_F {
                left: text_left as f32,
                top: (card.y + scale(5, self.dpi)) as f32,
                right: title_right as f32,
                bottom: (card.y + scale(21, self.dpi)) as f32,
            };
            self.draw_text(&item.label, &self.text_pomodoro_sub, title_rect, &fg);
        }
        let sub_right = card.right() - tile_pad;
        if sub_right > text_left + scale(12, self.dpi) {
            let sub = self.brushes.alpha_brush(ink, 0.56 * alpha);
            let sub_rect = D2D_RECT_F {
                left: text_left as f32,
                top: (card.y + scale(21, self.dpi)) as f32,
                right: sub_right as f32,
                bottom: (card.bottom() - scale(4, self.dpi)) as f32,
            };
            self.draw_text(&item.subtitle, &self.text_caption, sub_rect, &sub);
        }
        Ok(())
    }

    fn draw_shelf_tile(&self, tile: Rect, item: &ShelfItem, alpha: f32) -> Result<()> {
        let radius = scale(7, self.dpi) as f32;
        let shape = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(tile),
            radiusX: radius,
            radiusY: radius,
        };
        if let Some(bitmap) = self.resolve_shelf_bitmap(item)
            && self.fill_tile_bitmap(&shape, tile, &bitmap, alpha)
        {
            let rim = self.brushes.alpha_brush(self.notch_ink(), 0.16 * alpha);
            self.draw_rounded_box_stroke(tile, radius, &rim, 1.0);
            return Ok(());
        }
        let tint = kind_tint(item.kind);
        let bg = self.brushes.alpha_brush(tint, 0.20 * alpha);
        let border = self.brushes.alpha_brush(tint, 0.36 * alpha);
        let glyph_brush = self.brushes.alpha_brush(tint, 0.96 * alpha);
        self.draw_rounded_box(tile, radius, &bg);
        self.draw_rounded_box_stroke(tile, radius, &border, 1.0);
        let center = center_of(tile);
        self.draw_kind_glyph(
            center.X,
            center.Y,
            tile.width as f32 * 0.28,
            item.kind,
            &glyph_brush,
        )
    }

    fn fill_tile_bitmap(
        &self,
        shape: &D2D1_ROUNDED_RECT,
        tile: Rect,
        bitmap: &ID2D1Bitmap,
        alpha: f32,
    ) -> bool {
        unsafe {
            let size = bitmap.GetSize();
            if size.width <= 0.0 || size.height <= 0.0 {
                return false;
            }
            let scale_x = tile.width as f32 / size.width;
            let scale_y = tile.height as f32 / size.height;
            let scale = scale_x.max(scale_y);
            let draw_w = size.width * scale;
            let draw_h = size.height * scale;
            let tx = tile.x as f32 + (tile.width as f32 - draw_w) * 0.5;
            let ty = tile.y as f32 + (tile.height as f32 - draw_h) * 0.5;
            let brush_props = D2D1_BRUSH_PROPERTIES {
                opacity: alpha,
                transform: Matrix3x2 {
                    M11: scale,
                    M12: 0.0,
                    M21: 0.0,
                    M22: scale,
                    M31: tx,
                    M32: ty,
                },
            };
            let bmp_props = D2D1_BITMAP_BRUSH_PROPERTIES {
                extendModeX: D2D1_EXTEND_MODE_CLAMP,
                extendModeY: D2D1_EXTEND_MODE_CLAMP,
                interpolationMode: D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
            };
            let Ok(brush) =
                self.target
                    .CreateBitmapBrush(bitmap, Some(&bmp_props), Some(&brush_props))
            else {
                return false;
            };
            self.target.FillRoundedRectangle(shape, &brush);
            true
        }
    }

    fn draw_mini_copy_button(&self, rect: Rect, hovered: bool, card_hovered: bool, alpha: f32) {
        let ink = self.notch_ink();
        let radius = (rect.height as f32 * 0.5).max(4.0);
        if hovered || card_hovered {
            let bg = self
                .brushes
                .alpha_brush(ink, if hovered { 0.22 } else { 0.08 } * alpha);
            self.draw_rounded_box(rect, radius, &bg);
        }
        let icon_alpha = if hovered {
            0.96
        } else if card_hovered {
            0.74
        } else {
            0.44
        };
        let fg = self.brushes.alpha_brush(ink, icon_alpha * alpha);
        let c = center_of(rect);
        let s = (rect.width as f32 * 0.24).max(3.0);
        let stroke = (s * 0.30).max(1.0);
        let back = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: c.X - s * 0.95,
                top: c.Y - s * 0.95,
                right: c.X + s * 0.45,
                bottom: c.Y + s * 0.45,
            },
            radiusX: 1.5,
            radiusY: 1.5,
        };
        let front = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: c.X - s * 0.35,
                top: c.Y - s * 0.35,
                right: c.X + s * 1.05,
                bottom: c.Y + s * 1.05,
            },
            radiusX: 1.5,
            radiusY: 1.5,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&back, &fg, stroke, &self.stroke_style);
            self.target
                .DrawRoundedRectangle(&front, &fg, stroke, &self.stroke_style);
        }
    }

    pub(crate) fn draw_mini_remove_button(
        &self,
        rect: Rect,
        hovered: bool,
        card_hovered: bool,
        alpha: f32,
    ) {
        let ink = self.notch_ink();
        let radius = (rect.height as f32 * 0.5).max(4.0);
        if hovered {
            let bg = self.brushes.alpha_brush(SHELF_CLEAR_RED, 0.28 * alpha);
            self.draw_rounded_box(rect, radius, &bg);
        } else if card_hovered {
            let bg = self.brushes.alpha_brush(ink, 0.08 * alpha);
            self.draw_rounded_box(rect, radius, &bg);
        }
        let color = if hovered { SHELF_CLEAR_RED } else { ink };
        let icon_alpha = if hovered {
            0.96
        } else if card_hovered {
            0.74
        } else {
            0.44
        };
        let fg = self.brushes.alpha_brush(color, icon_alpha * alpha);
        let r = (rect.width as f32 * 0.21).max(2.6);
        let stroke = (r * 0.34).max(1.1);
        draw_close_x_glyph(
            &self.target,
            &self.stroke_style,
            center_of(rect),
            r,
            stroke,
            &fg,
        );
    }

    pub(crate) fn draw_shelf_pill(
        &self,
        rect: Rect,
        fill: &ID2D1SolidColorBrush,
        border: Option<&ID2D1SolidColorBrush>,
    ) {
        let radius = (rect.height as f32 * 0.5).max(scale(6, self.dpi) as f32);
        let pill = D2D1_ROUNDED_RECT {
            rect: rect_to_d2d(rect),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillRoundedRectangle(&pill, fill);
            if let Some(stroke) = border {
                self.target.DrawRoundedRectangle(&pill, stroke, 1.0, None);
            }
        }
    }

    pub(crate) fn draw_plus_glyph(&self, cx: f32, cy: f32, r: f32, brush: &ID2D1SolidColorBrush) {
        let stroke = (r * 0.34).max(1.2);
        unsafe {
            self.target.DrawLine(
                Vector2::new(cx - r, cy),
                Vector2::new(cx + r, cy),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx, cy - r),
                Vector2::new(cx, cy + r),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    fn draw_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        kind: ShelfKindTag,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let stroke = (r * 0.24).max(1.25);
        match kind {
            ShelfKindTag::Folder => {
                let geom = self.build_folder_geometry(cx, cy, r)?;
                unsafe {
                    self.target.FillGeometry(&geom, brush, None);
                }
            }
            ShelfKindTag::Image => self.draw_image_kind_glyph(cx, cy, r, brush, stroke),
            ShelfKindTag::Audio => self.draw_music_note_glyph(cx, cy, r * 0.96, brush),
            ShelfKindTag::Video => self.draw_video_kind_glyph(cx, cy, r, brush, stroke),
            ShelfKindTag::Pdf | ShelfKindTag::Document => {
                self.draw_document_glyph(cx, cy, r, brush, stroke, kind == ShelfKindTag::Pdf);
            }
            ShelfKindTag::Archive => self.draw_archive_kind_glyph(cx, cy, r, brush, stroke),
            ShelfKindTag::Code => self.draw_code_kind_glyph(cx, cy, r, brush, stroke),
            ShelfKindTag::Url => self.draw_url_kind_glyph(cx, cy, r, brush, stroke),
            ShelfKindTag::Text => self.draw_text_kind_glyph(cx, cy, r, brush, stroke),
        }
        Ok(())
    }

    fn draw_image_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
    ) {
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - r * 1.05,
                top: cy - r * 0.85,
                right: cx + r * 1.05,
                bottom: cy + r * 0.85,
            },
            radiusX: r * 0.25,
            radiusY: r * 0.25,
        };
        let sun = D2D1_ELLIPSE {
            point: Vector2::new(cx - r * 0.48, cy - r * 0.32),
            radiusX: r * 0.20,
            radiusY: r * 0.20,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&box_rect, brush, stroke, &self.stroke_style);
            self.target.FillEllipse(&sun, brush);
            self.target.DrawLine(
                Vector2::new(cx - r * 0.78, cy + r * 0.56),
                Vector2::new(cx - r * 0.10, cy - r * 0.05),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - r * 0.10, cy - r * 0.05),
                Vector2::new(cx + r * 0.25, cy + r * 0.28),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + r * 0.10, cy + r * 0.36),
                Vector2::new(cx + r * 0.45, cy - r * 0.02),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + r * 0.45, cy - r * 0.02),
                Vector2::new(cx + r * 0.82, cy + r * 0.56),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    fn draw_video_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
    ) {
        let frame = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - r * 1.05,
                top: cy - r * 0.82,
                right: cx + r * 1.05,
                bottom: cy + r * 0.82,
            },
            radiusX: r * 0.25,
            radiusY: r * 0.25,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&frame, brush, stroke, &self.stroke_style);
        }
        let _ = self.draw_play_glyph(cx + r * 0.06, cy, r * 0.46, brush);
    }

    fn draw_archive_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
    ) {
        let box_rect = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: cx - r * 0.95,
                top: cy - r * 0.95,
                right: cx + r * 0.95,
                bottom: cy + r * 0.95,
            },
            radiusX: r * 0.24,
            radiusY: r * 0.24,
        };
        unsafe {
            self.target
                .DrawRoundedRectangle(&box_rect, brush, stroke, &self.stroke_style);
            self.target.DrawLine(
                Vector2::new(cx, cy - r * 0.90),
                Vector2::new(cx, cy + r * 0.90),
                brush,
                stroke * 0.8,
                &self.stroke_style,
            );
            for step in [-0.45_f32, 0.0, 0.45] {
                let y = cy + r * step;
                self.target.DrawLine(
                    Vector2::new(cx - r * 0.34, y),
                    Vector2::new(cx + r * 0.34, y),
                    brush,
                    stroke * 0.8,
                    &self.stroke_style,
                );
            }
        }
    }

    fn draw_code_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
    ) {
        unsafe {
            self.target.DrawLine(
                Vector2::new(cx - r * 0.45, cy - r * 0.50),
                Vector2::new(cx - r * 0.98, cy),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - r * 0.98, cy),
                Vector2::new(cx - r * 0.45, cy + r * 0.50),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + r * 0.45, cy - r * 0.50),
                Vector2::new(cx + r * 0.98, cy),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + r * 0.98, cy),
                Vector2::new(cx + r * 0.45, cy + r * 0.50),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + r * 0.18, cy - r * 0.65),
                Vector2::new(cx - r * 0.18, cy + r * 0.65),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    pub(crate) fn draw_url_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
    ) {
        let outer = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: r * 0.95,
            radiusY: r * 0.95,
        };
        let inner = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: r * 0.44,
            radiusY: r * 0.95,
        };
        unsafe {
            self.target.DrawEllipse(&outer, brush, stroke, None);
            self.target.DrawEllipse(&inner, brush, stroke * 0.85, None);
            self.target.DrawLine(
                Vector2::new(cx - r * 0.92, cy),
                Vector2::new(cx + r * 0.92, cy),
                brush,
                stroke * 0.85,
                &self.stroke_style,
            );
        }
    }

    pub(crate) fn draw_text_kind_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
    ) {
        unsafe {
            let w = r * 0.86;
            self.target.DrawLine(
                Vector2::new(cx - w, cy - r * 0.55),
                Vector2::new(cx + w, cy - r * 0.55),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - w, cy),
                Vector2::new(cx + w, cy),
                brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx - w, cy + r * 0.55),
                Vector2::new(cx + r * 0.42, cy + r * 0.55),
                brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    pub(crate) fn build_folder_geometry(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
    ) -> Result<ID2D1PathGeometry> {
        unsafe {
            let geom = self.factory.CreatePathGeometry()?;
            let sink = geom.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - r * 1.05, cy - r * 0.72),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLines(&[
                Vector2::new(cx - r * 0.25, cy - r * 0.72),
                Vector2::new(cx - r * 0.02, cy - r * 0.42),
                Vector2::new(cx + r * 1.05, cy - r * 0.42),
                Vector2::new(cx + r * 1.05, cy + r * 0.78),
                Vector2::new(cx - r * 1.05, cy + r * 0.78),
            ]);
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            Ok(geom)
        }
    }

    fn draw_document_glyph(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        brush: &ID2D1SolidColorBrush,
        stroke: f32,
        emphasize_badge: bool,
    ) {
        let left = cx - r * 0.76;
        let right = cx + r * 0.76;
        let top = cy - r * 0.96;
        let bottom = cy + r * 0.96;
        let fold = r * 0.46;
        if let Ok(geom) = unsafe { self.factory.CreatePathGeometry() }
            && let Ok(sink) = unsafe { geom.Open() }
        {
            unsafe {
                sink.BeginFigure(Vector2::new(left, top), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddLines(&[
                    Vector2::new(right - fold, top),
                    Vector2::new(right, top + fold),
                    Vector2::new(right, bottom),
                    Vector2::new(left, bottom),
                ]);
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.BeginFigure(Vector2::new(right - fold, top), D2D1_FIGURE_BEGIN_HOLLOW);
                sink.AddLines(&[
                    Vector2::new(right - fold, top + fold),
                    Vector2::new(right, top + fold),
                ]);
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                let _ = sink.Close();
                self.target
                    .DrawGeometry(&geom, brush, stroke, &self.stroke_style);
            }
        }
        unsafe {
            if emphasize_badge {
                let badge = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cx - r * 0.48,
                        top: cy + r * 0.04,
                        right: cx + r * 0.48,
                        bottom: cy + r * 0.56,
                    },
                    radiusX: 1.5,
                    radiusY: 1.5,
                };
                self.target.FillRoundedRectangle(&badge, brush);
            } else {
                self.target.DrawLine(
                    Vector2::new(cx - r * 0.44, cy + r * 0.06),
                    Vector2::new(cx + r * 0.44, cy + r * 0.06),
                    brush,
                    stroke * 0.85,
                    &self.stroke_style,
                );
                self.target.DrawLine(
                    Vector2::new(cx - r * 0.44, cy + r * 0.44),
                    Vector2::new(cx + r * 0.24, cy + r * 0.44),
                    brush,
                    stroke * 0.85,
                    &self.stroke_style,
                );
            }
        }
    }
}
