use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{
    MenuRow, PopupMenuLayout, PopupMenuRowLayout, ResolvedTheme, month_cells, month_title, scale,
};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_HOLLOW,
    D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_LINE_JOIN_ROUND,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1CreateFactory, ID2D1DCRenderTarget,
    ID2D1Factory, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_REGULAR,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::core::Result;
use windows_numerics::Vector2;

use crate::create_text_format;
use crate::theme::LiquidGlassPalette;

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

pub struct PopupMenuRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    label_format: IDWriteTextFormat,
    shortcut_format: IDWriteTextFormat,
    cell_format: IDWriteTextFormat,
    dc: HDC,
    dib: HBITMAP,
    previous: HGDIOBJ,
    max_size: (i32, i32),
    dpi: u32,
}

impl PopupMenuRenderer {
    pub fn new(dpi: u32) -> Result<Self> {
        let max_w = scale(360, dpi);
        let max_h = scale(520, dpi);
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: PIXEL_FORMAT,
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let target = factory.CreateDCRenderTarget(&properties)?;

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

            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
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

            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: max_w,
                    biHeight: -max_h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..BITMAPINFOHEADER::default()
                },
                ..BITMAPINFO::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let previous = SelectObject(dc, HGDIOBJ(dib.0));

            Ok(Self {
                factory,
                target,
                stroke_style,
                label_format,
                shortcut_format,
                cell_format,
                dc,
                dib,
                previous,
                max_size: (max_w, max_h),
                dpi,
            })
        }
    }

    pub fn present(
        &self,
        hwnd: isize,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hovered_row: Option<usize>,
        theme: ResolvedTheme,
    ) -> Result<()> {
        let width = layout.window.width.clamp(1, self.max_size.0);
        let height = layout.window.height.clamp(1, self.max_size.1);
        self.draw(width, height, layout, rows, hovered_row, theme)?;
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(
                HWND(hwnd as *mut c_void),
                None,
                Some(&POINT {
                    x: layout.window.x,
                    y: layout.window.y,
                }),
                Some(&SIZE {
                    cx: width,
                    cy: height,
                }),
                Some(self.dc),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }

    fn draw(
        &self,
        width: i32,
        height: i32,
        layout: &PopupMenuLayout,
        rows: &[MenuRow],
        hovered_row: Option<usize>,
        theme: ResolvedTheme,
    ) -> Result<()> {
        let palette = LiquidGlassPalette::for_theme(theme);
        unsafe {
            self.target.BindDC(
                self.dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )?;
            self.target.BeginDraw();
            let clear = CLEAR;
            self.target.Clear(Some(&raw const clear));

            self.draw_shell(width as f32, height as f32, layout.corner_radius, &palette)?;

            for (row_layout, row_model) in layout.rows.iter().zip(rows) {
                self.draw_row(layout, row_layout, row_model, hovered_row, &palette)?;
            }

            self.target.EndDraw(None, None)
        }
    }

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
                    let sep_brush = self
                        .target
                        .CreateSolidColorBrush(&palette.menu_separator, None)?;
                    let y = row_layout.rect.y as f32 + row_layout.rect.height as f32 * 0.5;
                    let inset = scale(8, self.dpi) as f32;
                    self.target.DrawLine(
                        Vector2::new(row_layout.rect.x as f32 + inset, y),
                        Vector2::new(row_layout.rect.right() as f32 - inset, y),
                        &sep_brush,
                        1.0,
                        None,
                    );
                }
                MenuRow::Calendar { year, month, today } => {
                    self.draw_calendar(row_layout, *year, *month, *today, palette)?;
                }
                MenuRow::Slider { label, value, .. } => {
                    self.draw_slider(layout, row_layout, label, *value, palette)?;
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
                        let hover_brush = self
                            .target
                            .CreateSolidColorBrush(&palette.menu_hover_fill, None)?;
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
                        self.target.FillRoundedRectangle(&pill, &hover_brush);
                    }

                    let fg_color = if *enabled {
                        palette.text_primary
                    } else {
                        palette.text_disabled
                    };
                    let fg = self.target.CreateSolidColorBrush(&fg_color, None)?;

                    if *checked {
                        self.draw_checkmark(row_layout, layout.check_col_width, &fg)?;
                    }

                    let text_rect = D2D_RECT_F {
                        left: row_layout.rect.x as f32 + layout.check_col_width,
                        top: row_layout.rect.y as f32,
                        right: row_layout.rect.right() as f32 - layout.right_pad,
                        bottom: row_layout.rect.bottom() as f32,
                    };
                    let label_wide: Vec<u16> = label.encode_utf16().collect();
                    self.target.DrawText(
                        &label_wide,
                        &self.label_format,
                        &text_rect,
                        &fg,
                        D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );

                    if !shortcut.is_empty() {
                        let short_color = if !*enabled {
                            palette.text_disabled
                        } else if is_hovered {
                            palette.text_primary
                        } else {
                            palette.text_muted
                        };
                        let sb = self.target.CreateSolidColorBrush(&short_color, None)?;
                        let short_wide: Vec<u16> = shortcut.encode_utf16().collect();
                        self.target.DrawText(
                            &short_wide,
                            &self.shortcut_format,
                            &text_rect,
                            &sb,
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
    ) -> Result<()> {
        let unit = row.rect.height as f32 / 8.0;
        let inset = scale(6, self.dpi) as f32;
        let left = row.rect.x as f32 + inset;
        let col_w = (row.rect.width as f32 - inset * 2.0) / 7.0;
        let top = row.rect.y as f32;
        unsafe {
            let text = self
                .target
                .CreateSolidColorBrush(&palette.text_primary, None)?;
            let dim = self
                .target
                .CreateSolidColorBrush(&palette.text_muted, None)?;
            let accent = self
                .target
                .CreateSolidColorBrush(&palette.toggle_active, None)?;

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
                &text,
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
                    &dim,
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
                        &accent,
                    );
                }
                let wide: Vec<u16> = day.to_string().encode_utf16().collect();
                self.target.DrawText(
                    &wide,
                    &self.cell_format,
                    &cell,
                    &text,
                    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
        Ok(())
    }

    fn draw_slider(
        &self,
        layout: &PopupMenuLayout,
        row: &PopupMenuRowLayout,
        label: &str,
        value: u8,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let inset = layout.slider_inset as f32;
        let left = row.rect.x as f32 + inset;
        let right = row.rect.right() as f32 - inset;
        let track_h = (scale(10, self.dpi) as f32).max(6.0);
        let track_top = row.rect.bottom() as f32 - track_h - scale(9, self.dpi) as f32;
        let fraction = f32::from(value.min(100)) / 100.0;
        let fill_right = left + (right - left) * fraction;
        let radius = track_h * 0.5;
        unsafe {
            let text = self
                .target
                .CreateSolidColorBrush(&palette.text_primary, None)?;
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
                &text,
                D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            let percent_wide: Vec<u16> = format!("{value}%").encode_utf16().collect();
            let percent_brush = self
                .target
                .CreateSolidColorBrush(&palette.text_muted, None)?;
            self.target.DrawText(
                &percent_wide,
                &self.shortcut_format,
                &label_rect,
                &percent_brush,
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
            let track_brush = self
                .target
                .CreateSolidColorBrush(&palette.slider_track, None)?;
            self.target.FillRoundedRectangle(&track, &track_brush);
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
                let fill_brush = self
                    .target
                    .CreateSolidColorBrush(&palette.slider_fill, None)?;
                self.target.FillRoundedRectangle(&fill, &fill_brush);
            }
        }
        Ok(())
    }

    fn draw_shell(
        &self,
        width: f32,
        height: f32,
        radius: f32,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
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
            let fill = self
                .target
                .CreateSolidColorBrush(&palette.menu_fill, None)?;
            let outer_border = self
                .target
                .CreateSolidColorBrush(&palette.menu_outer_border, None)?;
            let inner_border = self
                .target
                .CreateSolidColorBrush(&palette.menu_inner_border, None)?;
            self.target.FillRoundedRectangle(&outer, &fill);
            self.target
                .DrawRoundedRectangle(&outer, &outer_border, 1.0, None);
            self.target
                .DrawRoundedRectangle(&inner, &inner_border, 1.0, None);
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
}

impl Drop for PopupMenuRenderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.dib.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
