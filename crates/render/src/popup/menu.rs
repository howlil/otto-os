use std::cell::RefCell;

use otto_core::{
    Appearance, CalendarControl, EstimatedMenuText, MenuRow, MenuTextMeasure,
    NOTIFICATION_CENTER_CARD_RADIUS, NOTIFICATION_CENTER_MAX_HEIGHT, NOTIFICATION_CENTER_WIDTH,
    PopupMenuLayout, PopupMenuRowLayout, Rect, RgbBuffer, badge_ink_is_dark,
    compute_calendar_layout, month_name, notification_badge, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE,
    D2D1_LINE_JOIN_ROUND, D2D1_STROKE_STYLE_PROPERTIES, ID2D1Brush, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1PathGeometry, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_WORD_WRAPPING_WRAP, IDWriteTextFormat,
};
use windows::core::Result;
use windows_numerics::Vector2;

impl MenuTextMeasure for PopupMenuRenderer {
    fn label_width(&self, text: &str) -> i32 {
        Self::measure(text, &self.label_format)
            .unwrap_or_else(|| EstimatedMenuText { dpi: self.dpi }.label_width(text))
    }

    fn shortcut_width(&self, text: &str) -> i32 {
        Self::measure(text, &self.shortcut_format)
            .unwrap_or_else(|| EstimatedMenuText { dpi: self.dpi }.shortcut_width(text))
    }
}

use crate::appearance::glass::GlassPainter;
use crate::appearance::surface::{
    DibSurface, create_dc_render_target, draw_on, draw_with_recovery,
};
use crate::appearance::theme::{BrushPalette, LiquidGlassPalette};
use crate::appearance::tokens::{SYSTEM_RED as TODAY_FILL, WHITE as TODAY_INK};
use crate::kit::draw::{draw_glass_or_fallback_shell, draw_text_on, measure_text_width};
use crate::kit::icons::{ChevronDir, draw_chevron_glyph, draw_close_x_glyph};
use crate::{
    BLACK, CLEAR, CachedCoverArt, create_text_format, draw_dwrite_tabular_text, rect_to_d2d,
    shared_d2d_factory, shared_dwrite_factory,
};

const ADJACENT_DAY_OPACITY: f32 = 0.55;
const NOTIFICATION_ICON_SIZE: i32 = 32;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuHover {
    pub row: Option<usize>,
    pub calendar: Option<CalendarControl>,
}

pub struct PopupMenuRenderer {
    pub(super) factory: ID2D1Factory,
    pub(super) target: ID2D1DCRenderTarget,
    pub(super) brushes: BrushPalette,
    pub(super) stroke_style: ID2D1StrokeStyle,
    label_format: IDWriteTextFormat,
    shortcut_format: IDWriteTextFormat,
    pub(super) month_format: IDWriteTextFormat,
    year_format: IDWriteTextFormat,
    weekday_format: IDWriteTextFormat,
    day_format: IDWriteTextFormat,
    today_format: IDWriteTextFormat,
    pub(super) notif_header_format: IDWriteTextFormat,
    pub(super) notif_title_format: IDWriteTextFormat,
    pub(super) notif_body_format: IDWriteTextFormat,
    pub(super) notif_meta_format: IDWriteTextFormat,
    pub(super) notif_meta_trailing_format: IDWriteTextFormat,
    pub(super) notif_count_format: IDWriteTextFormat,
    pub(super) notif_pill_format: IDWriteTextFormat,
    pub(super) notif_badge_format: IDWriteTextFormat,
    pub(super) cached_notif_icons: RefCell<Vec<CachedCoverArt>>,
    pub(super) surface: DibSurface,
    pub(super) dpi: u32,
    pub(super) glass: GlassPainter,
}

struct NotifFormats {
    header: IDWriteTextFormat,
    title: IDWriteTextFormat,
    body: IDWriteTextFormat,
    meta: IDWriteTextFormat,
    meta_trailing: IDWriteTextFormat,
    count: IDWriteTextFormat,
    pill: IDWriteTextFormat,
    badge: IDWriteTextFormat,
}

fn create_notif_formats(
    dwrite: &windows::Win32::Graphics::DirectWrite::IDWriteFactory,
    dpi: u32,
) -> Result<NotifFormats> {
    let dpi_scale = dpi as f32 / 96.0;
    unsafe {
        let header = create_text_format(
            dwrite,
            scale(13, dpi) as f32,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let title = create_text_format(
            dwrite,
            12.5 * dpi_scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let body = create_text_format(
            dwrite,
            11.5 * dpi_scale,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        body.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
        body.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
        let meta = create_text_format(
            dwrite,
            10.5 * dpi_scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
        )?;
        let meta_trailing = create_text_format(
            dwrite,
            10.5 * dpi_scale,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
        )?;
        let count = create_text_format(
            dwrite,
            scale(10, dpi) as f32,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let pill = create_text_format(
            dwrite,
            scale(11, dpi) as f32,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        let badge = create_text_format(
            dwrite,
            scale(14, dpi) as f32,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
        )?;
        Ok(NotifFormats {
            header,
            title,
            body,
            meta,
            meta_trailing,
            count,
            pill,
            badge,
        })
    }
}

impl PopupMenuRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        let max_w = scale(NOTIFICATION_CENTER_WIDTH + 8, dpi);
        let max_h = scale(NOTIFICATION_CENTER_MAX_HEIGHT + 8, dpi);
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = create_dc_render_target(&factory)?;
            let brushes = BrushPalette::new(&target)?;

            let stroke_props = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 2.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let stroke_style = factory.CreateStrokeStyle(&stroke_props, None)?;

            let dwrite = shared_dwrite_factory()?;
            let label_px = scale(13, dpi) as f32;
            let shortcut_px = scale(12, dpi) as f32;
            let label_format = create_text_format(
                &dwrite,
                label_px,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let shortcut_format = create_text_format(
                &dwrite,
                shortcut_px,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
            )?;

            let title_px = scale(15, dpi) as f32;
            let day_px = scale(13, dpi) as f32;
            let month_format = create_text_format(
                &dwrite,
                title_px,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let year_format = create_text_format(
                &dwrite,
                title_px,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let weekday_format = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let day_format = create_text_format(
                &dwrite,
                day_px,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let today_format = create_text_format(
                &dwrite,
                day_px,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let notif = create_notif_formats(&dwrite, dpi)?;

            let surface = DibSurface::new(max_w, max_h)?;

            Ok(Self {
                factory,
                target,
                brushes,
                stroke_style,
                label_format,
                shortcut_format,
                month_format,
                year_format,
                weekday_format,
                day_format,
                today_format,
                notif_header_format: notif.header,
                notif_title_format: notif.title,
                notif_body_format: notif.body,
                notif_meta_format: notif.meta,
                notif_meta_trailing_format: notif.meta_trailing,
                notif_count_format: notif.count,
                notif_pill_format: notif.pill,
                notif_badge_format: notif.badge,
                cached_notif_icons: RefCell::new(Vec::new()),
                surface,
                dpi,
                glass: GlassPainter::default(),
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn clear_backdrop(&self) {
        self.glass.clear_backdrop();
    }

    #[must_use]
    pub fn has_backdrop(&self) -> bool {
        self.glass.has_backdrop()
    }

    pub(crate) fn recreate_target(&mut self) -> Result<()> {
        let target = create_dc_render_target(&self.factory)?;
        let brushes = BrushPalette::new(&target)?;
        self.target = target;
        self.brushes = brushes;
        self.cached_notif_icons.borrow_mut().clear();
        self.glass.invalidate_gpu();
        Ok(())
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hover: MenuHover,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        draw_with_recovery(
            self,
            |renderer| renderer.draw(width, height, layout, rows, hover, appearance),
            Self::recreate_target,
        )?;
        self.surface
            .present_sized(hwnd, (layout.window.x, layout.window.y), (width, height))
    }

    #[cfg(test)]
    pub(crate) fn render_preview(
        &self,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hover: MenuHover,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        self.draw(width, height, layout, rows, hover, appearance)
    }

    fn draw(
        &self,
        width: i32,
        height: i32,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hover: MenuHover,
        appearance: Appearance,
    ) -> Result<()> {
        let appearance = self.glass.adaptive_appearance(&appearance);
        let palette = LiquidGlassPalette::for_appearance(&appearance);
        draw_on(&self.target, &self.surface, (width, height), || unsafe {
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));
            self.brushes.update_theme(&palette);

            self.draw_shell(width, height, layout.corner_radius, &appearance, &palette)?;

            let viewport = D2D_RECT_F {
                left: layout.viewport.x as f32,
                top: layout.viewport.y as f32,
                right: layout.viewport.right() as f32,
                bottom: layout.viewport.bottom() as f32,
            };
            self.target
                .PushAxisAlignedClip(&raw const viewport, D2D1_ANTIALIAS_MODE_ALIASED);
            let mut drawn = Ok(());
            for (row_layout, row_model) in layout.rows.iter().zip(rows) {
                if !layout.is_row_reachable(row_layout) {
                    continue;
                }
                drawn = self.draw_row(layout, row_layout, row_model, hover, &palette);
                if drawn.is_err() {
                    break;
                }
            }
            self.target.PopAxisAlignedClip();
            drawn?;
            self.draw_scroll_hints(layout);
            Ok(())
        })
    }

    fn draw_scroll_hints(&self, layout: &PopupMenuLayout) {
        if !layout.is_scrollable() {
            return;
        }
        let center_x = layout.viewport.x as f32 + layout.viewport.width as f32 * 0.5;
        let reach = scale(5, self.dpi) as f32;
        let stroke = (scale(2, self.dpi) as f32 * 0.75).max(1.4);
        if layout.scroll_offset > 0 {
            draw_chevron_glyph(
                &self.target,
                &self.stroke_style,
                Vector2::new(center_x, layout.viewport.y as f32),
                reach,
                stroke,
                ChevronDir::Up,
                &self.brushes.text_muted,
            );
        }
        if layout.scroll_offset < layout.max_scroll() {
            draw_chevron_glyph(
                &self.target,
                &self.stroke_style,
                Vector2::new(center_x, layout.viewport.bottom() as f32),
                reach,
                stroke,
                ChevronDir::Down,
                &self.brushes.text_muted,
            );
        }
    }

    pub(super) fn measure(text: &str, format: &IDWriteTextFormat) -> Option<i32> {
        Some(measure_text_width(text, format, 4096.0)?.ceil() as i32 + 2)
    }

    #[expect(clippy::too_many_lines)]
    fn draw_row(
        &self,
        layout: &PopupMenuLayout,
        row_layout: &PopupMenuRowLayout,
        row_model: &MenuRow,
        hover: MenuHover,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let hovered_row = hover.row;
        unsafe {
            match row_model {
                MenuRow::Separator => {
                    let sep_brush = &self.brushes.separator;
                    let y = row_layout.rect.y as f32 + row_layout.rect.height as f32 * 0.5;
                    let inset = scale(12, self.dpi) as f32;
                    self.target.DrawLine(
                        Vector2::new(row_layout.rect.x as f32 + inset, y),
                        Vector2::new(row_layout.rect.right() as f32 - inset, y),
                        sep_brush,
                        1.0,
                        None,
                    );
                }
                MenuRow::Calendar { year, month, today } => {
                    self.draw_calendar(row_layout, *year, *month, *today, hover.calendar, palette);
                }
                MenuRow::Notification {
                    app_name,
                    title,
                    body,
                    timestamp_label,
                    ..
                } => {
                    self.draw_notification(
                        row_layout,
                        app_name,
                        title,
                        body,
                        timestamp_label,
                        hovered_row == Some(row_layout.index),
                        palette,
                    );
                }
                MenuRow::Item {
                    label,
                    shortcut,
                    checked,
                    enabled,
                    ..
                } => {
                    let is_hovered = *enabled && hovered_row == Some(row_layout.index);
                    if is_hovered {
                        let hover_brush = &self.brushes.color_brush(palette.menu_hover_fill);
                        let pill_rect = D2D_RECT_F {
                            left: row_layout.rect.x as f32,
                            top: row_layout.rect.y as f32,
                            right: row_layout.rect.right() as f32,
                            bottom: row_layout.rect.bottom() as f32,
                        };
                        let geom = crate::build_squircle_path(
                            &self.factory,
                            pill_rect,
                            layout.row_corner_radius,
                        )
                        .ok();
                        self.fill_shape(
                            pill_rect,
                            layout.row_corner_radius,
                            geom.as_ref(),
                            hover_brush,
                        );
                    }

                    let fg = if is_hovered {
                        &self.brushes.on_accent
                    } else if *enabled {
                        &self.brushes.text_primary
                    } else {
                        &self.brushes.text_disabled
                    };

                    if *checked {
                        self.draw_checkmark(row_layout, layout.check_col_width, fg)?;
                    }

                    let text_rect = D2D_RECT_F {
                        left: row_layout.rect.x as f32 + layout.check_col_width,
                        top: row_layout.rect.y as f32,
                        right: row_layout.rect.right() as f32 - layout.right_pad,
                        bottom: row_layout.rect.bottom() as f32,
                    };
                    let shortcut_column = if layout.shortcut_col_width > 0.0 {
                        layout.shortcut_col_width + scale(12, self.dpi) as f32
                    } else {
                        0.0
                    };
                    let label_rect = D2D_RECT_F {
                        right: (text_rect.right - shortcut_column).max(text_rect.left + 1.0),
                        ..text_rect
                    };
                    self.draw_clipped_text(&self.label_format, label, &label_rect, fg);

                    if !shortcut.is_empty() {
                        let hovered_shortcut = is_hovered.then(|| {
                            self.brushes.alpha_brush(
                                palette.on_accent,
                                if palette.is_high_contrast { 1.0 } else { 0.88 },
                            )
                        });
                        let sb = if !*enabled {
                            &self.brushes.text_disabled
                        } else if let Some(ref brush) = hovered_shortcut {
                            brush
                        } else {
                            &self.brushes.text_muted
                        };
                        self.draw_clipped_text(&self.shortcut_format, shortcut, &text_rect, sb);
                    }
                }
            }
        }
        Ok(())
    }

    fn draw_calendar(
        &self,
        row: &PopupMenuRowLayout,
        year: u16,
        month: u8,
        today: Option<u8>,
        hovered: Option<CalendarControl>,
        palette: &LiquidGlassPalette,
    ) {
        let layout = compute_calendar_layout(row.rect, self.dpi, year, month);
        let text = self.brushes.color_brush(palette.text_primary);
        let muted = self.brushes.color_brush(palette.text_muted);
        let faint = self.brushes.color_brush(D2D1_COLOR_F {
            a: palette.text_muted.a * ADJACENT_DAY_OPACITY,
            ..palette.text_muted
        });
        let (today_fill, today_ink) = if palette.is_high_contrast {
            (palette.toggle_active, palette.on_accent)
        } else {
            (TODAY_FILL, TODAY_INK)
        };
        let today_fill = self.brushes.color_brush(today_fill);
        let today_ink = self.brushes.color_brush(today_ink);

        let month_name = month_name(month);
        let month_width = Self::measure(month_name, &self.month_format).unwrap_or(0);
        let year_left = layout.title.x + month_width + scale(5, self.dpi);
        self.draw_text(
            month_name,
            &self.month_format,
            &rect_to_d2d(layout.title),
            &text,
        );
        self.draw_text(
            &year.to_string(),
            &self.year_format,
            &D2D_RECT_F {
                left: year_left as f32,
                ..rect_to_d2d(layout.title)
            },
            &muted,
        );

        for (control, rect) in layout.controls {
            self.draw_calendar_control(control, rect, hovered == Some(control), palette, &text);
        }

        for (rect, letter) in layout
            .weekdays
            .iter()
            .zip(["S", "M", "T", "W", "T", "F", "S"])
        {
            self.draw_text(letter, &self.weekday_format, &rect_to_d2d(*rect), &muted);
        }

        for cell in &layout.cells {
            let rect = rect_to_d2d(cell.rect);
            let is_today = cell.day.in_month && today == Some(cell.day.day);
            let (format, brush) = if is_today {
                (&self.today_format, &today_ink)
            } else if cell.day.in_month {
                (&self.day_format, &text)
            } else {
                (&self.day_format, &faint)
            };
            if is_today {
                let radius = (cell.rect.width.min(cell.rect.height) as f32 * 0.5
                    - scale(2, self.dpi) as f32)
                    .max(6.0);
                unsafe {
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(
                                (rect.left + rect.right) * 0.5,
                                (rect.top + rect.bottom) * 0.5,
                            ),
                            radiusX: radius,
                            radiusY: radius,
                        },
                        &today_fill,
                    );
                }
            }
            draw_dwrite_tabular_text(&self.target, &cell.day.day.to_string(), format, rect, brush);
        }
    }

    fn draw_calendar_control(
        &self,
        control: CalendarControl,
        rect: Rect,
        hovered: bool,
        palette: &LiquidGlassPalette,
        ink: &ID2D1SolidColorBrush,
    ) {
        let cx = rect.x as f32 + rect.width as f32 * 0.5;
        let cy = rect.y as f32 + rect.height as f32 * 0.5;
        unsafe {
            if hovered {
                let radius = rect.width.min(rect.height) as f32 * 0.5;
                self.target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &self.brushes.color_brush(palette.hover_pill),
                );
            }
            let reach = scale(4, self.dpi) as f32;
            let stroke = (scale(2, self.dpi) as f32 * 0.8).max(1.4);
            match control {
                CalendarControl::Today => {
                    let radius = scale(3, self.dpi) as f32;
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(cx, cy),
                            radiusX: radius,
                            radiusY: radius,
                        },
                        ink,
                    );
                }
                CalendarControl::Previous | CalendarControl::Next => {
                    let dir = if control == CalendarControl::Previous {
                        ChevronDir::Left
                    } else {
                        ChevronDir::Right
                    };
                    draw_chevron_glyph(
                        &self.target,
                        &self.stroke_style,
                        Vector2::new(cx, cy),
                        reach,
                        stroke,
                        dir,
                        ink,
                    );
                }
            }
        }
    }

    fn draw_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: &D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        draw_text_on(
            &self.target,
            text,
            format,
            *rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
        );
    }

    pub(super) fn fill_shape(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        geom: Option<&ID2D1PathGeometry>,
        brush: &ID2D1Brush,
    ) {
        crate::kit::draw::fill_shape(&self.target, rect, radius, geom, brush);
    }

    pub(super) fn draw_shape(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        geom: Option<&ID2D1PathGeometry>,
        brush: &ID2D1Brush,
        stroke_width: f32,
    ) {
        crate::kit::draw::stroke_shape(&self.target, rect, radius, geom, brush, stroke_width);
    }

    pub(super) fn draw_shell(
        &self,
        width: i32,
        height: i32,
        radius: f32,
        appearance: &Appearance,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        draw_glass_or_fallback_shell(
            &self.factory,
            &self.target,
            &self.glass,
            &self.brushes,
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            radius,
            self.dpi,
            appearance,
            palette,
        )
    }

    fn draw_checkmark(
        &self,
        row: &PopupMenuRowLayout,
        check_col_width: f32,
        brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        let cx = row.rect.x as f32 + check_col_width * 0.52;
        let cy = row.rect.y as f32 + row.rect.height as f32 * 0.52;
        let s = (scale(4, self.dpi) as f32).max(3.5);
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            sink.BeginFigure(
                Vector2::new(cx - s * 0.95, cy + s * 0.05),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            sink.AddLine(Vector2::new(cx - s * 0.22, cy + s * 0.82));
            sink.AddLine(Vector2::new(cx + s * 1.05, cy - s * 0.85));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            let stroke = (scale(2, self.dpi) as f32 * 0.85).max(1.5);
            self.target
                .DrawGeometry(&path, brush, stroke, &self.stroke_style);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_notification(
        &self,
        row: &PopupMenuRowLayout,
        app_name: &str,
        title: &str,
        body: &str,
        timestamp_label: &str,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let gap_y = scale(3, self.dpi) as f32;
        let card_rect = D2D_RECT_F {
            left: row.rect.x as f32,
            top: row.rect.y as f32 + gap_y,
            right: row.rect.right() as f32,
            bottom: (row.rect.bottom() as f32 - gap_y).max(row.rect.y as f32 + gap_y + 1.0),
        };
        let radius = scale(NOTIFICATION_CENTER_CARD_RADIUS, self.dpi) as f32;
        let card_geom = crate::build_squircle_path(&self.factory, card_rect, radius).ok();
        self.fill_shape(
            card_rect,
            radius,
            card_geom.as_ref(),
            &self.brushes.card_fill,
        );
        if hovered {
            let lift = self.brushes.alpha_brush(palette.text_primary, 0.07);
            self.fill_shape(card_rect, radius, card_geom.as_ref(), &lift);
        }
        self.draw_shape(
            card_rect,
            radius,
            card_geom.as_ref(),
            &self.brushes.specular_rim,
            1.0,
        );
        self.draw_notification_contents(
            card_rect,
            app_name,
            title,
            body,
            timestamp_label,
            hovered,
            palette,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_notification_contents(
        &self,
        card_rect: D2D_RECT_F,
        app_name: &str,
        title: &str,
        body: &str,
        timestamp_label: &str,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let pad = scale(12, self.dpi) as f32;
        let icon = scale(NOTIFICATION_ICON_SIZE, self.dpi) as f32;
        let card_height = card_rect.bottom - card_rect.top;
        let icon_rect = D2D_RECT_F {
            left: card_rect.left + pad,
            top: card_rect.top + (card_height - icon) * 0.5,
            right: card_rect.left + pad + icon,
            bottom: card_rect.top + (card_height + icon) * 0.5,
        };
        self.draw_notification_badge(icon_rect, app_name);

        let text_left = icon_rect.right + scale(10, self.dpi) as f32;
        let text_right = (card_rect.right - pad).max(text_left + 1.0);
        let top_pad = scale(10, self.dpi) as f32;
        let title_h = scale(16, self.dpi) as f32;
        let trailing_w = scale(34, self.dpi) as f32;
        let title_top = card_rect.top + top_pad;
        let primary_text = if title.is_empty() { body } else { title };
        let title_rect = D2D_RECT_F {
            left: text_left,
            top: title_top,
            right: (text_right - trailing_w - scale(6, self.dpi) as f32).max(text_left + 1.0),
            bottom: title_top + title_h,
        };
        let trailing_rect = D2D_RECT_F {
            left: (text_right - trailing_w).max(text_left),
            top: title_top,
            right: text_right,
            bottom: title_top + title_h,
        };
        let body_rect = D2D_RECT_F {
            left: text_left,
            top: title_top + title_h,
            right: text_right,
            bottom: (title_top + title_h + scale(32, self.dpi) as f32)
                .min(card_rect.bottom - scale(4, self.dpi) as f32)
                .max(title_top + title_h + 1.0),
        };
        let primary = &self.brushes.text_primary;
        let secondary = &self.brushes.text_muted;
        self.draw_clipped_text(&self.notif_title_format, primary_text, &title_rect, primary);
        if hovered {
            self.draw_notification_close(trailing_rect, palette);
        } else {
            draw_dwrite_tabular_text(
                &self.target,
                timestamp_label,
                &self.shortcut_format,
                trailing_rect,
                secondary,
            );
        }
        if !title.is_empty() {
            self.draw_clipped_text(&self.notif_body_format, body, &body_rect, secondary);
        }
    }

    fn draw_notification_badge(&self, rect: D2D_RECT_F, app_name: &str) {
        let badge = notification_badge(app_name);
        let radius = (rect.right - rect.left) * 0.26;
        let fill = self.brushes.color_brush(D2D1_COLOR_F {
            r: f32::from(badge.color[0]) / 255.0,
            g: f32::from(badge.color[1]) / 255.0,
            b: f32::from(badge.color[2]) / 255.0,
            a: 1.0,
        });
        let geom = crate::build_squircle_path(&self.factory, rect, radius).ok();
        self.fill_shape(rect, radius, geom.as_ref(), &fill);
        let mut buffer = [0u8; 4];
        let initial = badge.initial.encode_utf8(&mut buffer);
        let ink = if badge_ink_is_dark(badge.color) {
            self.brushes.alpha_brush(BLACK, 0.88)
        } else {
            self.brushes.white.clone()
        };
        self.draw_clipped_text(&self.today_format, initial, &rect, &ink);
    }

    pub(super) fn draw_notification_close(&self, area: D2D_RECT_F, palette: &LiquidGlassPalette) {
        let radius = scale(8, self.dpi) as f32;
        let center = Vector2::new(area.right - radius, (area.top + area.bottom) * 0.5);
        let disc = self.brushes.alpha_brush(palette.text_primary, 0.16);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: radius,
                    radiusY: radius,
                },
                &disc,
            );
        }
        draw_close_x_glyph(
            &self.target,
            &self.stroke_style,
            center,
            radius * 0.38,
            (scale(1, self.dpi) as f32 * 1.25).max(1.25),
            &self.brushes.text_muted,
        );
    }

    pub(super) fn draw_clipped_text(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        rect: &D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        draw_text_on(
            &self.target,
            text,
            format,
            *rect,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::PoisonError;

    use super::*;
    use crate::release_shared_factories;
    use crate::resource_tests::SERIAL;
    use otto_core::{MenuAnchor, PopupMenuMetrics, PopupMenuPlacement, ResolvedTheme};

    #[test]
    fn popup_menu_renders_hovered_item_separator_and_notification_with_squircle_paths() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let menu = PopupMenuRenderer::new(96).expect("popup menu");
        let rows = [
            MenuRow::item(1, "New Window", "Ctrl+N"),
            MenuRow::Separator,
            MenuRow::Notification {
                id: 2,
                app_name: String::from("Mail"),
                title: String::from("Build green"),
                body: String::from("All checks passed"),
                timestamp_label: String::from("1m"),
            },
        ];
        let layout = otto_core::compute_popup_menu_layout(
            &PopupMenuMetrics::default(),
            PopupMenuPlacement {
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                dpi: 96,
                anchor: MenuAnchor::BelowPoint { x: 400, y: 36 },
                scroll_offset: 0,
            },
            &rows,
            &menu,
        );
        assert!((layout.row_corner_radius - 8.0).abs() < f32::EPSILON);
        let appearance = Appearance::glass(ResolvedTheme::Dark);
        let hover_row = |row| MenuHover {
            row: Some(row),
            calendar: None,
        };
        menu.render_preview(&layout, &rows, hover_row(0), appearance)
            .expect("hovered item render");
        menu.render_preview(&layout, &rows, hover_row(2), appearance)
            .expect("hovered notification render");
        drop(menu);
        release_shared_factories();
    }

    fn pixel_at(menu: &PopupMenuRenderer, x: i32, y: i32) -> [u8; 4] {
        let (width, _) = menu.surface.size();
        let pixels = menu.surface.read_pixels();
        let at = (y as usize * width as usize + x as usize) * 4;
        [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
    }

    #[test]
    fn calendar_marks_today_with_a_red_disc_and_lights_the_hovered_header_control() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let menu = PopupMenuRenderer::new(96).expect("popup menu");
        let rows = otto_core::build_calendar_rows(2026, 11, Some(19));
        let layout = otto_core::compute_popup_menu_layout(
            &PopupMenuMetrics::default(),
            PopupMenuPlacement {
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                dpi: 96,
                anchor: MenuAnchor::BelowPoint { x: 400, y: 36 },
                scroll_offset: 0,
            },
            &rows,
            &menu,
        );
        let appearance = Appearance::glass(ResolvedTheme::Dark);
        let calendar = compute_calendar_layout(layout.rows[0].rect, 96, 2026, 11);
        let center = |rect: Rect| (rect.x + rect.width / 2, rect.y + rect.height / 2);

        menu.render_preview(&layout, &rows, MenuHover::default(), appearance)
            .expect("calendar render");
        let today = calendar
            .cells
            .iter()
            .find(|cell| cell.day.in_month && cell.day.day == 19)
            .expect("today cell")
            .rect;
        let (today_x, today_y) = center(today);
        let [blue, green, red, alpha] = pixel_at(&menu, today_x - 10, today_y);
        assert!(
            alpha == 255 && red > 200 && green < 120 && blue < 110,
            "today disc is not red: {red} {green} {blue} {alpha}"
        );
        let other = calendar
            .cells
            .iter()
            .find(|cell| cell.day.in_month && cell.day.day == 20)
            .expect("neighbour cell")
            .rect;
        let (other_x, other_y) = center(other);
        let [_, other_green, other_red, _] = pixel_at(&menu, other_x - 10, other_y);
        assert!(other_red.abs_diff(other_green) < 40);

        let (_, previous) = calendar.controls[0];
        let (px, py) = center(previous);
        let idle = pixel_at(&menu, px - 10, py);
        menu.render_preview(
            &layout,
            &rows,
            MenuHover {
                row: None,
                calendar: Some(CalendarControl::Previous),
            },
            appearance,
        )
        .expect("hovered calendar control render");
        let hovered = pixel_at(&menu, px - 10, py);
        assert_ne!(idle, hovered);
        drop(menu);
        release_shared_factories();
    }

    #[test]
    fn light_mode_hovered_menu_shortcut_uses_high_contrast_on_accent_ink() {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let menu = PopupMenuRenderer::new(96).expect("popup menu");
        let rows = [MenuRow::item(1, "New Window", "Ctrl+N")];
        let layout = otto_core::compute_popup_menu_layout(
            &PopupMenuMetrics::default(),
            PopupMenuPlacement {
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                dpi: 96,
                anchor: MenuAnchor::BelowPoint { x: 400, y: 36 },
                scroll_offset: 0,
            },
            &rows,
            &menu,
        );
        let light = Appearance {
            transparency: false,
            ..Appearance::glass(ResolvedTheme::Light)
        };
        menu.render_preview(
            &layout,
            &rows,
            MenuHover {
                row: Some(0),
                calendar: None,
            },
            light,
        )
        .expect("light hovered menu render");
        let row = layout.rows[0].rect;
        let [pill_b, _, pill_r, _] = pixel_at(&menu, row.x + 8, row.y + 4);
        assert!(
            pill_b > 180 && pill_r < 80,
            "expected blue accent hover pill, got rgb({pill_r}, _, {pill_b})"
        );
        let right = (row.right() as f32 - layout.right_pad).round() as i32;
        let left = (right as f32 - layout.shortcut_col_width).floor() as i32;
        let mut max_r = 0u8;
        let mut min_b = u8::MAX;
        for y in (row.y + 2)..(row.bottom() - 2) {
            for x in left.max(row.x)..right {
                let [b, _, r, _] = pixel_at(&menu, x, y);
                max_r = max_r.max(r);
                min_b = min_b.min(b);
            }
        }
        assert!(
            max_r > pill_r.saturating_add(120),
            "expected bright on_accent shortcut ink over hover pill, got max_r={max_r}, pill_r={pill_r}"
        );
        assert!(
            min_b >= pill_b.saturating_sub(30),
            "shortcut ink darkened hover pill blue channel: min_b={min_b}, pill_b={pill_b}"
        );
        drop(menu);
        release_shared_factories();
    }
}
