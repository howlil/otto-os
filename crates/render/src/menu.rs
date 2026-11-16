use otto_core::{
    Appearance, EstimatedMenuText, GlassShape, MenuRow, MenuTextMeasure, PopupMenuLayout,
    PopupMenuRowLayout, Rect, RgbBuffer, month_cells, month_title, scale,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE,
    D2D1_LINE_JOIN_ROUND, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, IDWriteTextFormat,
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

use crate::glass::GlassPainter;
use crate::surface::{DibSurface, create_dc_render_target, draw_on, draw_with_recovery};
use crate::theme::{BrushPalette, LiquidGlassPalette};
use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub struct PopupMenuRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    brushes: BrushPalette,
    stroke_style: ID2D1StrokeStyle,
    label_format: IDWriteTextFormat,
    shortcut_format: IDWriteTextFormat,
    cell_format: IDWriteTextFormat,
    notif_meta_format: IDWriteTextFormat,
    notif_body_format: IDWriteTextFormat,
    surface: DibSurface,
    dpi: u32,
    glass: GlassPainter,
}

impl PopupMenuRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        let max_w = scale(360, dpi);
        let max_h = scale(520, dpi);
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

            let cell_format = create_text_format(
                &dwrite,
                shortcut_px,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;
            let notif_meta_format = create_text_format(
                &dwrite,
                scale(11, dpi) as f32,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;
            let notif_body_format = create_text_format(
                &dwrite,
                scale(12, dpi) as f32,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_TEXT_ALIGNMENT_LEADING,
            )?;

            let surface = DibSurface::new(max_w, max_h)?;

            Ok(Self {
                factory,
                target,
                brushes,
                stroke_style,
                label_format,
                shortcut_format,
                cell_format,
                notif_meta_format,
                notif_body_format,
                surface,
                dpi,
                glass: GlassPainter::default(),
            })
        }
    }

    pub fn set_backdrop(&self, capture: RgbBuffer, origin: (i32, i32)) {
        self.glass.set_backdrop(capture, origin);
    }

    pub fn set_pointer(&self, pointer: Option<(f32, f32)>) {
        self.glass.set_pointer(pointer);
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
        self.glass.invalidate_gpu();
        Ok(())
    }

    pub fn present(
        &mut self,
        hwnd: isize,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hovered_row: Option<usize>,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        draw_with_recovery(
            self,
            |renderer| renderer.draw(width, height, layout, rows, hovered_row, appearance),
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
        hovered_row: Option<usize>,
        appearance: Appearance,
    ) -> Result<()> {
        let (max_width, max_height) = self.surface.size();
        let width = layout.window.width.clamp(1, max_width);
        let height = layout.window.height.clamp(1, max_height);
        self.draw(width, height, layout, rows, hovered_row, appearance)
    }

    fn draw(
        &self,
        width: i32,
        height: i32,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hovered_row: Option<usize>,
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
                drawn = self.draw_row(layout, row_layout, row_model, hovered_row, &palette);
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
        let center = layout.viewport.x as f32 + layout.viewport.width as f32 * 0.5;
        let half = scale(5, self.dpi) as f32;
        let rise = scale(3, self.dpi) as f32;
        let brush = &self.brushes.text_muted;
        let chevron = |tip_y: f32, direction: f32| unsafe {
            let tip = Vector2::new(center, tip_y);
            for side in [-1.0, 1.0] {
                self.target.DrawLine(
                    Vector2::new(center + side * half, tip_y + direction * rise),
                    tip,
                    brush,
                    1.5,
                    &self.stroke_style,
                );
            }
        };
        if layout.scroll_offset > 0 {
            chevron(layout.viewport.y as f32 - rise * 0.5, 1.0);
        }
        if layout.scroll_offset < layout.max_scroll() {
            chevron(layout.viewport.bottom() as f32 + rise * 0.5, -1.0);
        }
    }

    fn measure(text: &str, format: &IDWriteTextFormat) -> Option<i32> {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut metrics = DWRITE_TEXT_METRICS::default();
        unsafe {
            let layout = shared_dwrite_factory()
                .ok()?
                .CreateTextLayout(&wide, format, 4096.0, 64.0)
                .ok()?;
            layout.GetMetrics(&raw mut metrics).ok()?;
        }
        Some(metrics.widthIncludingTrailingWhitespace.ceil() as i32 + 2)
    }

    #[expect(clippy::too_many_lines)]
    fn draw_row(
        &self,
        layout: &PopupMenuLayout,
        row_layout: &PopupMenuRowLayout,
        row_model: &MenuRow,
        hovered_row: Option<usize>,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        unsafe {
            match row_model {
                MenuRow::Separator => {
                    let sep_brush = &self.brushes.separator;
                    let y = row_layout.rect.y as f32 + row_layout.rect.height as f32 * 0.5;
                    let inset = scale(8, self.dpi) as f32;
                    self.target.DrawLine(
                        Vector2::new(row_layout.rect.x as f32 + inset, y),
                        Vector2::new(row_layout.rect.right() as f32 - inset, y),
                        sep_brush,
                        1.0,
                        None,
                    );
                }
                MenuRow::Calendar { year, month, today } => {
                    self.draw_calendar(row_layout, *year, *month, *today, palette);
                }
                MenuRow::Notification {
                    app_name,
                    title,
                    body,
                    timestamp_label,
                    ..
                } => {
                    self.draw_notification(
                        layout,
                        row_layout,
                        app_name,
                        title,
                        body,
                        timestamp_label,
                        hovered_row == Some(row_layout.index),
                        palette,
                    );
                }
                MenuRow::Slider { label, value, .. } => {
                    self.draw_slider(layout, row_layout, label, *value, palette);
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
                        let pill = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: row_layout.rect.x as f32,
                                top: row_layout.rect.y as f32,
                                right: row_layout.rect.right() as f32,
                                bottom: row_layout.rect.bottom() as f32,
                            },
                            radiusX: layout.row_corner_radius,
                            radiusY: layout.row_corner_radius,
                        };
                        self.target.FillRoundedRectangle(&pill, hover_brush);
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
                    let label_wide: Vec<u16> = label.encode_utf16().collect();
                    self.target.DrawText(
                        &label_wide,
                        &self.label_format,
                        &label_rect,
                        fg,
                        D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    if !shortcut.is_empty() {
                        let sb = if !*enabled {
                            &self.brushes.text_disabled
                        } else if is_hovered {
                            &self.brushes.text_primary
                        } else {
                            &self.brushes.text_muted
                        };
                        let short_wide: Vec<u16> = shortcut.encode_utf16().collect();
                        self.target.DrawText(
                            &short_wide,
                            &self.shortcut_format,
                            &text_rect,
                            sb,
                            D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                            DWRITE_MEASURING_MODE_NATURAL,
                        );
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
        palette: &LiquidGlassPalette,
    ) {
        let unit = row.rect.height as f32 / 8.0;
        let inset = scale(6, self.dpi) as f32;
        let left = row.rect.x as f32 + inset;
        let col_w = (row.rect.width as f32 - inset * 2.0) / 7.0;
        let top = row.rect.y as f32;
        unsafe {
            let text = &self.brushes.color_brush(palette.text_primary);
            let dim = &self.brushes.color_brush(palette.text_muted);
            let accent = &self.brushes.color_brush(palette.toggle_active);

            let title_rect = D2D_RECT_F {
                left: left + scale(4, self.dpi) as f32,
                top,
                right: left + col_w * 7.0,
                bottom: top + unit,
            };
            let title: Vec<u16> = month_title(year, month).encode_utf16().collect();
            self.target.DrawText(
                &title,
                &self.label_format,
                &title_rect,
                text,
                D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            for (col, letter) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
                let cell = D2D_RECT_F {
                    left: left + col as f32 * col_w,
                    top: top + unit,
                    right: left + (col as f32 + 1.0) * col_w,
                    bottom: top + unit * 2.0,
                };
                let wide: Vec<u16> = letter.encode_utf16().collect();
                self.target.DrawText(
                    &wide,
                    &self.cell_format,
                    &cell,
                    dim,
                    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }

            for (index, cell_day) in month_cells(year, month).iter().enumerate() {
                let Some(day) = cell_day else {
                    continue;
                };
                let cell = D2D_RECT_F {
                    left: left + (index % 7) as f32 * col_w,
                    top: top + unit * (2.0 + (index / 7) as f32),
                    right: left + ((index % 7) as f32 + 1.0) * col_w,
                    bottom: top + unit * (3.0 + (index / 7) as f32),
                };
                if today == Some(*day) {
                    let radius = (unit.min(col_w) * 0.42).max(6.0);
                    self.target.FillEllipse(
                        &D2D1_ELLIPSE {
                            point: Vector2::new(
                                (cell.left + cell.right) * 0.5,
                                (cell.top + cell.bottom) * 0.5,
                            ),
                            radiusX: radius,
                            radiusY: radius,
                        },
                        accent,
                    );
                }
                let wide: Vec<u16> = day.to_string().encode_utf16().collect();
                self.target.DrawText(
                    &wide,
                    &self.cell_format,
                    &cell,
                    text,
                    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }

    fn draw_slider(
        &self,
        layout: &PopupMenuLayout,
        row: &PopupMenuRowLayout,
        label: &str,
        value: u8,
        palette: &LiquidGlassPalette,
    ) {
        let inset = layout.slider_inset as f32;
        let left = row.rect.x as f32 + inset;
        let right = row.rect.right() as f32 - inset;
        let track_h = (scale(10, self.dpi) as f32).max(6.0);
        let track_top = row.rect.bottom() as f32 - track_h - scale(9, self.dpi) as f32;
        let fraction = f32::from(value.min(100)) / 100.0;
        let fill_right = left + (right - left) * fraction;
        let radius = track_h * 0.5;
        unsafe {
            let text = &self.brushes.color_brush(palette.text_primary);
            let label_rect = D2D_RECT_F {
                left,
                top: row.rect.y as f32 + scale(4, self.dpi) as f32,
                right,
                bottom: track_top,
            };
            let label_wide: Vec<u16> = label.encode_utf16().collect();
            self.target.DrawText(
                &label_wide,
                &self.label_format,
                &label_rect,
                text,
                D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            let percent_wide: Vec<u16> = format!("{value}%").encode_utf16().collect();
            let percent_brush = &self.brushes.color_brush(palette.text_muted);
            self.target.DrawText(
                &percent_wide,
                &self.shortcut_format,
                &label_rect,
                percent_brush,
                D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            let track = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left,
                    top: track_top,
                    right,
                    bottom: track_top + track_h,
                },
                radiusX: radius,
                radiusY: radius,
            };
            let track_brush = &self.brushes.color_brush(palette.slider_track);
            self.target.FillRoundedRectangle(&track, track_brush);
            if fill_right > left + 0.5 {
                let fill = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left,
                        top: track_top,
                        right: fill_right.max(left + track_h),
                        bottom: track_top + track_h,
                    },
                    radiusX: radius,
                    radiusY: radius,
                };
                let fill_brush = &self.brushes.color_brush(palette.slider_fill);
                self.target.FillRoundedRectangle(&fill, fill_brush);
            }
        }
    }

    fn draw_shell(
        &self,
        width: i32,
        height: i32,
        radius: f32,
        appearance: &Appearance,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let shell_shape = GlassShape::from_rect(
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            radius,
        );
        if self
            .glass
            .draw(&self.target, appearance, self.dpi, shell_shape, true)?
        {
            return Ok(());
        }

        let width = width as f32;
        let height = height as f32;
        let outer = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 0.5,
                top: 0.5,
                right: width - 0.5,
                bottom: height - 0.5,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let inner = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: 1.5,
                top: 1.5,
                right: width - 1.5,
                bottom: height - 1.5,
            },
            radiusX: (radius - 1.0).max(1.0),
            radiusY: (radius - 1.0).max(1.0),
        };
        unsafe {
            let fill = &self.brushes.color_brush(palette.menu_fill);
            let outer_border = &self.brushes.color_brush(palette.menu_outer_border);
            let inner_border = &self.brushes.specular_rim;
            self.target.FillRoundedRectangle(&outer, fill);
            self.target
                .DrawRoundedRectangle(&outer, outer_border, 1.0, None);
            self.target
                .DrawRoundedRectangle(&inner, inner_border, 1.0, None);
        }
        Ok(())
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
        layout: &PopupMenuLayout,
        row: &PopupMenuRowLayout,
        app_name: &str,
        title: &str,
        body: &str,
        timestamp_label: &str,
        hovered: bool,
        palette: &LiquidGlassPalette,
    ) {
        let gap_y = scale(2, self.dpi) as f32;
        let card_rect = D2D_RECT_F {
            left: row.rect.x as f32,
            top: row.rect.y as f32 + gap_y,
            right: row.rect.right() as f32,
            bottom: (row.rect.bottom() as f32 - gap_y).max(row.rect.y as f32 + gap_y + 1.0),
        };
        let radius = layout.row_corner_radius + 2.0;
        let card = D2D1_ROUNDED_RECT {
            rect: card_rect,
            radiusX: radius,
            radiusY: radius,
        };
        let fill = if hovered {
            self.brushes.color_brush(palette.menu_hover_fill)
        } else {
            self.brushes.card_fill.clone()
        };
        let (primary_fg, secondary_fg) = if hovered {
            (&self.brushes.on_accent, &self.brushes.on_accent)
        } else {
            (&self.brushes.text_primary, &self.brushes.text_muted)
        };
        unsafe {
            self.target.FillRoundedRectangle(&card, &fill);
            self.target
                .DrawRoundedRectangle(&card, &self.brushes.specular_rim, 1.0, None);
        }
        self.draw_notification_contents(
            card_rect,
            app_name,
            title,
            body,
            timestamp_label,
            primary_fg,
            secondary_fg,
        );
    }

    #[allow(clippy::too_many_arguments)]
    #[expect(clippy::similar_names)]
    fn draw_notification_contents(
        &self,
        card_rect: D2D_RECT_F,
        app_name: &str,
        title: &str,
        body: &str,
        timestamp_label: &str,
        primary_fg: &ID2D1SolidColorBrush,
        secondary_fg: &ID2D1SolidColorBrush,
    ) {
        let pad_x = scale(10, self.dpi) as f32;
        let pad_y = scale(4, self.dpi) as f32;
        let inner_left = card_rect.left + pad_x;
        let inner_right = (card_rect.right - pad_x).max(inner_left + 1.0);
        let close_col = scale(14, self.dpi) as f32;
        let ts_col = if timestamp_label.is_empty() {
            0.0
        } else {
            scale(36, self.dpi) as f32
        };
        let meta_h = scale(14, self.dpi) as f32;
        let meta_top = card_rect.top + pad_y;
        let meta_rect = D2D_RECT_F {
            left: inner_left,
            top: meta_top,
            right: (inner_right - close_col - ts_col).max(inner_left + 1.0),
            bottom: meta_top + meta_h,
        };
        let ts_rect = D2D_RECT_F {
            left: (inner_right - close_col - ts_col).max(inner_left),
            top: meta_top,
            right: (inner_right - close_col).max(inner_left + 1.0),
            bottom: meta_top + meta_h,
        };
        let close_cx = inner_right - close_col * 0.35;
        let close_cy = meta_top + meta_h * 0.5;
        let close_r = scale(3, self.dpi) as f32;
        let line2_top = meta_top + meta_h;
        let line2_h = if title.is_empty() || body.is_empty() {
            (card_rect.bottom - pad_y - line2_top).max(scale(14, self.dpi) as f32)
        } else {
            scale(15, self.dpi) as f32
        };
        let title_rect = D2D_RECT_F {
            left: inner_left,
            top: line2_top,
            right: inner_right,
            bottom: line2_top + line2_h,
        };
        let body_rect = D2D_RECT_F {
            left: inner_left,
            top: line2_top + line2_h,
            right: inner_right,
            bottom: (card_rect.bottom - pad_y * 0.5).max(line2_top + line2_h + 1.0),
        };
        let primary_text = if title.is_empty() { body } else { title };
        self.draw_clipped_text(&self.notif_meta_format, app_name, &meta_rect, secondary_fg);
        self.draw_clipped_text(
            &self.shortcut_format,
            timestamp_label,
            &ts_rect,
            secondary_fg,
        );
        self.draw_clipped_text(&self.label_format, primary_text, &title_rect, primary_fg);
        if !title.is_empty() {
            self.draw_clipped_text(&self.notif_body_format, body, &body_rect, secondary_fg);
        }
        unsafe {
            self.target.DrawLine(
                Vector2::new(close_cx - close_r, close_cy - close_r),
                Vector2::new(close_cx + close_r, close_cy + close_r),
                secondary_fg,
                1.25,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(close_cx - close_r, close_cy + close_r),
                Vector2::new(close_cx + close_r, close_cy - close_r),
                secondary_fg,
                1.25,
                &self.stroke_style,
            );
        }
    }

    fn draw_clipped_text(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        rect: &D2D_RECT_F,
        brush: &ID2D1SolidColorBrush,
    ) {
        if text.is_empty() {
            return;
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            self.target.DrawText(
                &wide,
                format,
                rect,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}
