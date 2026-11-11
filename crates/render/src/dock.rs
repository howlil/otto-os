use std::collections::HashMap;
use std::f32::consts::PI;
use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{AppleIconKind, DockItemLayout, DockLayout, ResolvedTheme, scale};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT,
    D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
    D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_PROPERTIES, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_NO_SNAP, D2D1_ELLIPSE, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_LINE_JOIN_ROUND,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE, ID2D1Bitmap,
    ID2D1DCRenderTarget, ID2D1Factory, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_ALIGNMENT_CENTER,
    IDWriteTextFormat,
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

use crate::theme::{BrushPalette, LiquidGlassPalette};
use crate::{create_text_format, shared_d2d_factory, shared_dwrite_factory};

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
const ICON_SHADOW: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.28,
};
const ICON_RIM: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.30,
};
const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

const fn rgb(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a,
    }
}

pub struct DockRenderItem<'a> {
    pub key: &'a str,
    pub title: &'a str,
    pub apple_icon: Option<AppleIconKind>,
    pub running: bool,
    pub active: bool,
}

pub struct DockRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    tooltip_format: IDWriteTextFormat,
    dc: HDC,
    dib: HBITMAP,
    previous: HGDIOBJ,
    size: (i32, i32),
    dpi: u32,
    font_px: f32,
    icons: HashMap<String, ID2D1Bitmap>,
    brushes: BrushPalette,
}

impl DockRenderer {
    pub fn new(width: i32, height: i32, dpi: u32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
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

            let dwrite = shared_dwrite_factory()?;
            let font_px = scale(12, dpi) as f32;
            let tooltip_format = create_text_format(
                &dwrite,
                font_px,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            )?;

            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
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
            let brushes = BrushPalette::new(&target)?;

            Ok(Self {
                factory,
                target,
                stroke_style,
                tooltip_format,
                dc,
                dib,
                previous,
                size: (width, height),
                dpi,
                font_px,
                icons: HashMap::new(),
                brushes,
            })
        }
    }

    #[must_use]
    pub fn has_icon(&self, key: &str) -> bool {
        self.icons.contains_key(key)
    }

    pub fn set_icon(&mut self, key: String, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
        let props = D2D1_BITMAP_PROPERTIES {
            pixelFormat: PIXEL_FORMAT,
            dpiX: 96.0,
            dpiY: 96.0,
        };
        let bitmap = unsafe {
            self.target.CreateBitmap(
                D2D_SIZE_U { width, height },
                Some(pixels.as_ptr().cast()),
                width * 4,
                &props,
            )?
        };
        self.icons.insert(key, bitmap);
        Ok(())
    }

    pub fn retain_icons(&mut self, alive: &[&str]) {
        self.icons.retain(|k, _| alive.contains(&k.as_str()));
    }

    pub fn present(
        &self,
        hwnd: isize,
        origin: (i32, i32),
        layout: &DockLayout,
        items: &[DockRenderItem<'_>],
        theme: ResolvedTheme,
    ) -> Result<()> {
        self.draw(layout, items, theme)?;
        let (width, height) = self.size;
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
                    x: origin.0,
                    y: origin.1,
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
        layout: &DockLayout,
        items: &[DockRenderItem<'_>],
        theme: ResolvedTheme,
    ) -> Result<()> {
        let palette = LiquidGlassPalette::for_theme(theme);
        let (width, height) = self.size;
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
            self.brushes.update_theme(&palette);

            if layout.pill.width > 0 && layout.pill.height > 0 {
                self.draw_pill(layout, &palette);
            }

            if let Some(div_x) = layout.divider_x {
                let div_brush = self.brushes.scratch(0, palette.dock_divider);
                let pad_v = layout.pill.height as f32 * 0.20;
                self.target.DrawLine(
                    Vector2::new(div_x, layout.pill.y as f32 + pad_v),
                    Vector2::new(div_x, layout.pill.bottom() as f32 - pad_v),
                    div_brush,
                    (scale(1, self.dpi) as f32).max(1.0),
                    None,
                );
            }

            let dot_brush = self.brushes.scratch(1, palette.dock_dot);
            let active_dot_brush = &self.brushes.text_primary;

            for (slot, item) in layout.items.iter().zip(items) {
                if slot.opacity < 0.03 || slot.size < 2.0 {
                    continue;
                }
                if let Some(apple_kind) = item.apple_icon {
                    self.draw_apple_icon(slot, apple_kind)?;
                } else if let Some(bitmap) = self.icons.get(item.key) {
                    let dest = D2D_RECT_F {
                        left: slot.x,
                        top: slot.y,
                        right: slot.x + slot.size,
                        bottom: slot.y + slot.size,
                    };
                    self.target.DrawBitmap(
                        bitmap,
                        Some(&raw const dest),
                        slot.opacity,
                        D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                        None,
                    );
                }

                if item.running && slot.opacity > 0.5 {
                    let r = layout.dot_radius;
                    let (cx, cy) = slot.dot_center;
                    let ellipse = D2D1_ELLIPSE {
                        point: Vector2::new(cx, cy),
                        radiusX: if item.active { r * 1.25 } else { r },
                        radiusY: r,
                    };
                    let b = if item.active {
                        active_dot_brush
                    } else {
                        dot_brush
                    };
                    self.target.FillEllipse(&ellipse, b);
                }
            }

            if let Some(idx) = layout.hovered_index
                && let (Some(slot), Some(item)) = (layout.items.get(idx), items.get(idx))
            {
                self.draw_tooltip(
                    layout,
                    slot.x + slot.size * 0.5,
                    slot.y,
                    item.title,
                    &palette,
                )?;
            }

            self.target.EndDraw(None, None)
        }
    }

    fn squircle_bounds(slot: &DockItemLayout) -> (D2D1_ROUNDED_RECT, D2D1_ROUNDED_RECT) {
        let inset = slot.size * (18.0 / 256.0);
        let left = slot.x + inset;
        let top = slot.y + inset;
        let right = slot.x + slot.size - inset;
        let bottom = slot.y + slot.size - inset;
        let radius = (right - left) * (49.0 / 220.0);
        let drop = (slot.size * 0.022).max(1.2);

        let tile = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right,
                bottom,
            },
            radiusX: radius,
            radiusY: radius,
        };
        let shadow = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: left + 0.5,
                top: top + drop,
                right: right - 0.5,
                bottom: bottom + drop,
            },
            radiusX: radius,
            radiusY: radius,
        };
        (tile, shadow)
    }

    fn draw_apple_icon(&self, slot: &DockItemLayout, kind: AppleIconKind) -> Result<()> {
        let (tile, shadow) = Self::squircle_bounds(slot);
        unsafe {
            let shadow_brush = self.brushes.scratch(0, ICON_SHADOW);
            self.target.FillRoundedRectangle(&shadow, shadow_brush);

            match kind {
                AppleIconKind::Finder => self.draw_finder_tile(&tile)?,
                AppleIconKind::Launchpad => self.draw_launchpad_tile(&tile),
                AppleIconKind::Trash => self.draw_trash_tile(&tile, false)?,
                AppleIconKind::TrashFull => self.draw_trash_tile(&tile, true)?,
                AppleIconKind::Terminal => self.draw_terminal_tile(&tile)?,
                AppleIconKind::Settings => self.draw_settings_tile(&tile),
                AppleIconKind::Browser => self.draw_safari_tile(&tile)?,
                AppleIconKind::Notes => self.draw_notes_tile(&tile)?,
                AppleIconKind::Code => self.draw_code_tile(&tile)?,
                AppleIconKind::Music => self.draw_music_tile(&tile),
                AppleIconKind::Folder => self.draw_folder_tile(&tile),
            }

            let rim_brush = self.brushes.scratch(1, ICON_RIM);
            self.target
                .DrawRoundedRectangle(&tile, rim_brush, 1.1, None);
        }
        Ok(())
    }

    fn draw_finder_tile(&self, tile: &D2D1_ROUNDED_RECT) -> Result<()> {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        let cx = r.left + w * 0.5;

        unsafe {
            let light_blue = self.brushes.scratch(2, rgb(138, 214, 255, 1.0));
            let deep_blue = self.brushes.scratch(3, rgb(28, 120, 242, 1.0));
            let feature = self.brushes.scratch(4, rgb(20, 44, 84, 0.96));

            self.target.FillRoundedRectangle(tile, light_blue);

            let left_half = D2D_RECT_F {
                left: r.left,
                top: r.top,
                right: cx,
                bottom: r.bottom,
            };
            let left_round = D2D1_ROUNDED_RECT {
                rect: left_half,
                radiusX: tile.radiusX,
                radiusY: tile.radiusY,
            };
            self.target.FillRoundedRectangle(&left_round, deep_blue);
            let seam_fill = D2D_RECT_F {
                left: cx - tile.radiusX * 0.6,
                top: r.top,
                right: cx + w * 0.02,
                bottom: r.bottom,
            };
            self.target.FillRectangle(&seam_fill, deep_blue);

            let stroke = (w * 0.045).max(1.5);
            let nose = self.factory.CreatePathGeometry()?;
            let sink = nose.Open()?;
            sink.BeginFigure(
                Vector2::new(cx + w * 0.03, r.top + h * 0.06),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(cx - w * 0.04, r.top + h * 0.30),
                point2: Vector2::new(cx - w * 0.06, r.top + h * 0.44),
                point3: Vector2::new(cx + w * 0.06, r.top + h * 0.54),
            });
            sink.AddLine(Vector2::new(cx - w * 0.01, r.bottom - h * 0.06));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&nose, feature, stroke, &self.stroke_style);

            let eye_w = (w * 0.035).max(1.3);
            let eye_h = h * 0.11;
            let eye_y = r.top + h * 0.29;
            for ex in [r.left + w * 0.29, r.left + w * 0.71] {
                let eye = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: ex - eye_w,
                        top: eye_y,
                        right: ex + eye_w,
                        bottom: eye_y + eye_h,
                    },
                    radiusX: eye_w,
                    radiusY: eye_w,
                };
                self.target.FillRoundedRectangle(&eye, feature);
            }

            let smile = self.factory.CreatePathGeometry()?;
            let s_sink = smile.Open()?;
            s_sink.BeginFigure(
                Vector2::new(r.left + w * 0.24, r.top + h * 0.65),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            s_sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: Vector2::new(r.left + w * 0.38, r.top + h * 0.82),
                point2: Vector2::new(r.left + w * 0.62, r.top + h * 0.82),
                point3: Vector2::new(r.left + w * 0.76, r.top + h * 0.65),
            });
            s_sink.EndFigure(D2D1_FIGURE_END_OPEN);
            s_sink.Close()?;
            self.target
                .DrawGeometry(&smile, feature, stroke, &self.stroke_style);
        }
        Ok(())
    }

    fn draw_trash_tile(&self, tile: &D2D1_ROUNDED_RECT, is_full: bool) -> Result<()> {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        let cx = r.left + w * 0.5;

        unsafe {
            let plate = self.brushes.scratch(2, rgb(232, 236, 242, 1.0));
            let bin_fill = self.brushes.scratch(3, rgb(180, 188, 198, 1.0));
            let bin_dark = self.brushes.scratch(4, rgb(104, 112, 124, 0.95));
            let rim_light = self.brushes.scratch(5, rgb(248, 250, 252, 1.0));

            self.target.FillRoundedRectangle(tile, plate);

            let top_y = r.top + h * 0.24;
            let bot_y = r.top + h * 0.82;
            let top_hw = w * 0.28;
            let bot_hw = w * 0.21;

            let body = self.factory.CreatePathGeometry()?;
            let sink = body.Open()?;
            sink.BeginFigure(Vector2::new(cx - top_hw, top_y), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(Vector2::new(cx + top_hw, top_y));
            sink.AddLine(Vector2::new(cx + bot_hw, bot_y));
            sink.AddLine(Vector2::new(cx - bot_hw, bot_y));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&body, bin_fill, None);
            self.target
                .DrawGeometry(&body, bin_dark, (w * 0.025).max(1.0), &self.stroke_style);

            let rim_ellipse = D2D1_ELLIPSE {
                point: Vector2::new(cx, top_y),
                radiusX: top_hw,
                radiusY: h * 0.055,
            };
            self.target.FillEllipse(&rim_ellipse, rim_light);

            if is_full {
                let paper_fill = self.brushes.scratch(6, rgb(252, 250, 244, 0.98));
                let paper_crease = self.brushes.scratch(7, rgb(156, 164, 176, 0.90));
                let blobs = [
                    (cx - w * 0.11, top_y + h * 0.01, w * 0.095, h * 0.075),
                    (cx + w * 0.09, top_y + h * 0.02, w * 0.10, h * 0.08),
                    (cx - w * 0.01, top_y - h * 0.02, w * 0.105, h * 0.082),
                    (cx + w * 0.02, top_y + h * 0.24, w * 0.15, h * 0.14),
                ];
                for (bx, by, rx, ry) in blobs {
                    let e = D2D1_ELLIPSE {
                        point: Vector2::new(bx, by),
                        radiusX: rx,
                        radiusY: ry,
                    };
                    self.target.FillEllipse(&e, paper_fill);
                    self.target
                        .DrawEllipse(&e, paper_crease, (w * 0.018).max(0.9), None);
                }
            }

            self.target.DrawEllipse(
                &rim_ellipse,
                bin_dark,
                (w * 0.025).max(1.0),
                &self.stroke_style,
            );

            for t in [-0.6_f32, -0.3, 0.0, 0.3, 0.6] {
                let x_top = cx + top_hw * 0.82 * t;
                let x_bot = cx + bot_hw * 0.82 * t;
                self.target.DrawLine(
                    Vector2::new(x_top, top_y + h * 0.08),
                    Vector2::new(x_bot, bot_y - h * 0.05),
                    bin_dark,
                    (w * 0.022).max(1.0),
                    &self.stroke_style,
                );
            }
        }
        Ok(())
    }

    fn draw_launchpad_tile(&self, tile: &D2D1_ROUNDED_RECT) {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;

        unsafe {
            let plate = self.brushes.scratch(2, rgb(38, 42, 52, 1.0));
            self.target.FillRoundedRectangle(tile, plate);

            let colors = [
                rgb(10, 132, 255, 1.0),
                rgb(255, 149, 0, 1.0),
                rgb(48, 209, 88, 1.0),
                rgb(191, 90, 242, 1.0),
                rgb(255, 55, 95, 1.0),
                rgb(255, 214, 10, 1.0),
                rgb(100, 210, 255, 1.0),
                rgb(255, 69, 58, 1.0),
                rgb(94, 92, 230, 1.0),
            ];
            let cell = w * 0.175;
            let gap = w * 0.065;
            let grid_w = cell * 3.0 + gap * 2.0;
            let start_x = r.left + (w - grid_w) * 0.5;
            let start_y = r.top + (h - grid_w) * 0.5;
            let rad = (cell * 0.28).max(1.8);

            for (idx, color) in colors.iter().enumerate() {
                let col = (idx % 3) as f32;
                let row = (idx / 3) as f32;
                let cell_x = start_x + col * (cell + gap);
                let cell_y = start_y + row * (cell + gap);
                let brush = self.brushes.scratch(3, *color);
                let mini = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: cell_x,
                        top: cell_y,
                        right: cell_x + cell,
                        bottom: cell_y + cell,
                    },
                    radiusX: rad,
                    radiusY: rad,
                };
                self.target.FillRoundedRectangle(&mini, brush);
            }
        }
    }

    fn draw_code_tile(&self, tile: &D2D1_ROUNDED_RECT) -> Result<()> {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        let cy = r.top + h * 0.5;

        unsafe {
            let bg = self.brushes.scratch(2, rgb(24, 30, 42, 1.0));
            let cyan = self.brushes.scratch(3, rgb(36, 172, 242, 1.0));
            let deep_cyan = self.brushes.scratch(4, rgb(0, 120, 212, 1.0));

            self.target.FillRoundedRectangle(tile, bg);

            let stroke = (w * 0.075).max(1.8);
            let left_chev = self.factory.CreatePathGeometry()?;
            let s1 = left_chev.Open()?;
            s1.BeginFigure(
                Vector2::new(r.left + w * 0.40, cy - h * 0.20),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            s1.AddLine(Vector2::new(r.left + w * 0.22, cy));
            s1.AddLine(Vector2::new(r.left + w * 0.40, cy + h * 0.20));
            s1.EndFigure(D2D1_FIGURE_END_OPEN);
            s1.Close()?;
            self.target
                .DrawGeometry(&left_chev, cyan, stroke, &self.stroke_style);

            let right_chev = self.factory.CreatePathGeometry()?;
            let s2 = right_chev.Open()?;
            s2.BeginFigure(
                Vector2::new(r.left + w * 0.60, cy - h * 0.20),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            s2.AddLine(Vector2::new(r.left + w * 0.78, cy));
            s2.AddLine(Vector2::new(r.left + w * 0.60, cy + h * 0.20));
            s2.EndFigure(D2D1_FIGURE_END_OPEN);
            s2.Close()?;
            self.target
                .DrawGeometry(&right_chev, deep_cyan, stroke, &self.stroke_style);

            self.target.DrawLine(
                Vector2::new(r.left + w * 0.54, cy - h * 0.23),
                Vector2::new(r.left + w * 0.46, cy + h * 0.23),
                cyan,
                (stroke * 0.82).max(1.4),
                &self.stroke_style,
            );
        }
        Ok(())
    }

    fn draw_music_tile(&self, tile: &D2D1_ROUNDED_RECT) {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;

        unsafe {
            let coral = self.brushes.scratch(2, rgb(250, 45, 72, 1.0));
            let white = &self.brushes.white;

            self.target.FillRoundedRectangle(tile, coral);

            let left_x = r.left + w * 0.36;
            let right_x = r.left + w * 0.68;
            let top_left_y = r.top + h * 0.28;
            let top_right_y = r.top + h * 0.22;
            let bot_left_y = r.top + h * 0.68;
            let bot_right_y = r.top + h * 0.62;
            let stem_w = (w * 0.065).max(1.6);

            self.target.DrawLine(
                Vector2::new(left_x, top_left_y),
                Vector2::new(left_x, bot_left_y),
                white,
                stem_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(right_x, top_right_y),
                Vector2::new(right_x, bot_right_y),
                white,
                stem_w,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(left_x, top_left_y),
                Vector2::new(right_x, top_right_y),
                white,
                (stem_w * 1.45).max(2.2),
                &self.stroke_style,
            );

            let note_width = w * 0.095;
            let note_height = h * 0.072;
            for (hx, hy) in [
                (left_x - note_width * 0.55, bot_left_y),
                (right_x - note_width * 0.55, bot_right_y),
            ] {
                let note_head = D2D1_ELLIPSE {
                    point: Vector2::new(hx, hy),
                    radiusX: note_width,
                    radiusY: note_height,
                };
                self.target.FillEllipse(&note_head, white);
            }
        }
    }

    fn draw_folder_tile(&self, tile: &D2D1_ROUNDED_RECT) {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        let cx = r.left + w * 0.5;

        unsafe {
            let plate = self.brushes.scratch(2, rgb(238, 244, 252, 1.0));
            let back_blue = self.brushes.scratch(3, rgb(42, 152, 240, 1.0));
            let front_blue = self.brushes.scratch(4, rgb(96, 194, 255, 1.0));
            let arrow_brush = self.brushes.scratch(5, rgb(24, 118, 210, 0.92));

            self.target.FillRoundedRectangle(tile, plate);

            let tab_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: r.left + w * 0.14,
                    top: r.top + h * 0.22,
                    right: r.left + w * 0.48,
                    bottom: r.top + h * 0.42,
                },
                radiusX: w * 0.06,
                radiusY: w * 0.06,
            };
            self.target.FillRoundedRectangle(&tab_rect, back_blue);

            let back_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: r.left + w * 0.14,
                    top: r.top + h * 0.28,
                    right: r.right - w * 0.14,
                    bottom: r.bottom - h * 0.18,
                },
                radiusX: w * 0.07,
                radiusY: w * 0.07,
            };
            self.target.FillRoundedRectangle(&back_rect, back_blue);

            let front_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: r.left + w * 0.14,
                    top: r.top + h * 0.36,
                    right: r.right - w * 0.14,
                    bottom: r.bottom - h * 0.18,
                },
                radiusX: w * 0.07,
                radiusY: w * 0.07,
            };
            self.target.FillRoundedRectangle(&front_rect, front_blue);

            let stroke = (w * 0.048).max(1.3);
            let ay0 = r.top + h * 0.46;
            let ay1 = r.top + h * 0.67;
            self.target.DrawLine(
                Vector2::new(cx, ay0),
                Vector2::new(cx, ay1),
                arrow_brush,
                stroke,
                &self.stroke_style,
            );
            let wing = w * 0.09;
            self.target.DrawLine(
                Vector2::new(cx - wing, ay1 - wing * 0.85),
                Vector2::new(cx, ay1),
                arrow_brush,
                stroke,
                &self.stroke_style,
            );
            self.target.DrawLine(
                Vector2::new(cx + wing, ay1 - wing * 0.85),
                Vector2::new(cx, ay1),
                arrow_brush,
                stroke,
                &self.stroke_style,
            );
        }
    }

    fn draw_terminal_tile(&self, tile: &D2D1_ROUNDED_RECT) -> Result<()> {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;

        unsafe {
            let outer = self.brushes.scratch(2, rgb(58, 62, 70, 1.0));
            let inner = self.brushes.scratch(3, rgb(24, 26, 31, 1.0));
            let glyph = self.brushes.scratch(4, rgb(245, 247, 250, 0.96));

            self.target.FillRoundedRectangle(tile, outer);
            let pad = (w * 0.055).max(1.8);
            let inner_rect = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: r.left + pad,
                    top: r.top + pad,
                    right: r.right - pad,
                    bottom: r.bottom - pad,
                },
                radiusX: (tile.radiusX - pad).max(2.0),
                radiusY: (tile.radiusY - pad).max(2.0),
            };
            self.target.FillRoundedRectangle(&inner_rect, inner);

            let stroke = (w * 0.06).max(1.6);
            let chevron = self.factory.CreatePathGeometry()?;
            let sink = chevron.Open()?;
            sink.BeginFigure(
                Vector2::new(r.left + w * 0.22, r.top + h * 0.28),
                D2D1_FIGURE_BEGIN_HOLLOW,
            );
            sink.AddLine(Vector2::new(r.left + w * 0.38, r.top + h * 0.41));
            sink.AddLine(Vector2::new(r.left + w * 0.22, r.top + h * 0.54));
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;
            self.target
                .DrawGeometry(&chevron, glyph, stroke, &self.stroke_style);

            self.target.DrawLine(
                Vector2::new(r.left + w * 0.44, r.top + h * 0.54),
                Vector2::new(r.left + w * 0.64, r.top + h * 0.54),
                glyph,
                stroke,
                &self.stroke_style,
            );
        }
        Ok(())
    }

    fn draw_settings_tile(&self, tile: &D2D1_ROUNDED_RECT) {
        let r = tile.rect;
        let w = r.right - r.left;
        let cx = r.left + w * 0.5;
        let cy = r.top + (r.bottom - r.top) * 0.5;

        unsafe {
            let plate = self.brushes.scratch(2, rgb(208, 213, 221, 1.0));
            let well = self.brushes.scratch(3, rgb(58, 62, 70, 1.0));
            let gear = self.brushes.scratch(4, rgb(218, 223, 230, 0.96));

            self.target.FillRoundedRectangle(tile, plate);

            let well_ellipse = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: w * 0.37,
                radiusY: w * 0.37,
            };
            self.target.FillEllipse(&well_ellipse, well);

            let outer_r = w * 0.28;
            let inner_r = w * 0.12;
            let stroke = (w * 0.048).max(1.4);

            for i in 0..3 {
                let angle = (i as f32) * (PI / 3.0);
                let (sin_a, cos_a) = angle.sin_cos();
                self.target.DrawLine(
                    Vector2::new(cx - cos_a * outer_r, cy - sin_a * outer_r),
                    Vector2::new(cx + cos_a * outer_r, cy + sin_a * outer_r),
                    gear,
                    stroke,
                    &self.stroke_style,
                );
            }

            let outer_ring = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: outer_r,
                radiusY: outer_r,
            };
            let inner_ring = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: inner_r,
                radiusY: inner_r,
            };
            self.target
                .DrawEllipse(&outer_ring, gear, stroke * 1.1, &self.stroke_style);
            self.target.FillEllipse(&inner_ring, well);
            self.target
                .DrawEllipse(&inner_ring, gear, stroke * 0.85, &self.stroke_style);
        }
    }

    fn draw_safari_tile(&self, tile: &D2D1_ROUNDED_RECT) -> Result<()> {
        let r = tile.rect;
        let w = r.right - r.left;
        let cx = r.left + w * 0.5;
        let cy = r.top + (r.bottom - r.top) * 0.5;

        unsafe {
            let plate = self.brushes.scratch(2, rgb(250, 252, 255, 1.0));
            let dial = self.brushes.scratch(3, rgb(24, 132, 247, 1.0));
            let white = self.brushes.scratch(4, rgb(255, 255, 255, 0.95));
            let red = self.brushes.scratch(5, rgb(255, 59, 48, 0.98));

            self.target.FillRoundedRectangle(tile, plate);

            let dial_r = w * 0.41;
            let dial_ellipse = D2D1_ELLIPSE {
                point: Vector2::new(cx, cy),
                radiusX: dial_r,
                radiusY: dial_r,
            };
            self.target.FillEllipse(&dial_ellipse, dial);

            for i in 0..12 {
                let angle = (i as f32) * (PI / 6.0);
                let (sin_a, cos_a) = angle.sin_cos();
                let r0 = dial_r * 0.78;
                let r1 = dial_r * 0.91;
                self.target.DrawLine(
                    Vector2::new(cx + cos_a * r0, cy + sin_a * r0),
                    Vector2::new(cx + cos_a * r1, cy + sin_a * r1),
                    white,
                    (w * 0.022).max(1.0),
                    &self.stroke_style,
                );
            }

            let needle_len = dial_r * 0.78;
            let needle_hw = dial_r * 0.14;
            let unit = 0.707_106_77_f32;

            let ne = self.factory.CreatePathGeometry()?;
            let s1 = ne.Open()?;
            s1.BeginFigure(
                Vector2::new(cx + unit * needle_len, cy - unit * needle_len),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            s1.AddLine(Vector2::new(cx + unit * needle_hw, cy + unit * needle_hw));
            s1.AddLine(Vector2::new(cx - unit * needle_hw, cy - unit * needle_hw));
            s1.EndFigure(D2D1_FIGURE_END_CLOSED);
            s1.Close()?;
            self.target.FillGeometry(&ne, red, None);

            let sw = self.factory.CreatePathGeometry()?;
            let s2 = sw.Open()?;
            s2.BeginFigure(
                Vector2::new(cx - unit * needle_len, cy + unit * needle_len),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            s2.AddLine(Vector2::new(cx + unit * needle_hw, cy + unit * needle_hw));
            s2.AddLine(Vector2::new(cx - unit * needle_hw, cy - unit * needle_hw));
            s2.EndFigure(D2D1_FIGURE_END_CLOSED);
            s2.Close()?;
            self.target.FillGeometry(&sw, white, None);
        }
        Ok(())
    }

    fn draw_notes_tile(&self, tile: &D2D1_ROUNDED_RECT) -> Result<()> {
        let r = tile.rect;
        let w = r.right - r.left;
        let h = r.bottom - r.top;

        unsafe {
            let paper = self.brushes.scratch(2, rgb(252, 251, 247, 1.0));
            let header = self.brushes.scratch(3, rgb(245, 184, 41, 1.0));
            let rule = self.brushes.scratch(4, rgb(214, 212, 206, 1.0));

            self.target.FillRoundedRectangle(tile, paper);

            let header_h = h * 0.28;
            let header_path = self.factory.CreatePathGeometry()?;
            let sink = header_path.Open()?;
            let rad = tile.radiusX;
            sink.BeginFigure(
                Vector2::new(r.left, r.top + header_h),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(r.left, r.top + rad));
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(r.left + rad, r.top),
                size: D2D_SIZE_F {
                    width: rad,
                    height: rad,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.AddLine(Vector2::new(r.right - rad, r.top));
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: Vector2::new(r.right, r.top + rad),
                size: D2D_SIZE_F {
                    width: rad,
                    height: rad,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.AddLine(Vector2::new(r.right, r.top + header_h));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&header_path, header, None);

            for frac in [0.46_f32, 0.62, 0.78] {
                let y = r.top + h * frac;
                self.target.DrawLine(
                    Vector2::new(r.left + w * 0.14, y),
                    Vector2::new(r.right - w * 0.14, y),
                    rule,
                    (w * 0.025).max(1.0),
                    &self.stroke_style,
                );
            }
        }
        Ok(())
    }

    fn draw_pill(&self, layout: &DockLayout, palette: &LiquidGlassPalette) {
        let p = layout.pill;
        let r = layout.pill_radius;
        let outer = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: p.x as f32,
                top: p.y as f32,
                right: p.right() as f32,
                bottom: p.bottom() as f32,
            },
            radiusX: r,
            radiusY: r,
        };
        let inner = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: p.x as f32 + 1.0,
                top: p.y as f32 + 1.0,
                right: p.right() as f32 - 1.0,
                bottom: p.bottom() as f32 - 1.0,
            },
            radiusX: (r - 1.0).max(1.0),
            radiusY: (r - 1.0).max(1.0),
        };
        unsafe {
            let fill = self.brushes.scratch(2, palette.dock_fill);
            let outer_border = self.brushes.scratch(3, palette.dock_outer_border);
            let inner_border = self.brushes.scratch(4, palette.dock_inner_border);

            self.target.FillRoundedRectangle(&outer, fill);
            self.target
                .DrawRoundedRectangle(&outer, outer_border, 1.0, None);
            self.target
                .DrawRoundedRectangle(&inner, inner_border, 1.0, None);
        }
    }

    fn draw_tooltip(
        &self,
        layout: &DockLayout,
        center_x: f32,
        icon_top_y: f32,
        title: &str,
        palette: &LiquidGlassPalette,
    ) -> Result<()> {
        let pad_h = scale(12, self.dpi) as f32;
        let est_w = (title.chars().count() as f32 * self.font_px * 0.56 + pad_h * 2.0)
            .max(scale(52, self.dpi) as f32);
        let h = layout.tooltip_height;
        let bottom = icon_top_y - layout.tooltip_gap;
        let top = (bottom - h).max(2.0);
        let left = (center_x - est_w * 0.5).clamp(4.0, (self.size.0 as f32 - est_w - 4.0).max(4.0));
        let right = left + est_w;
        let radius = scale(6, self.dpi) as f32;
        let arrow_size = scale(5, self.dpi) as f32;

        let pill = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left,
                top,
                right,
                bottom,
            },
            radiusX: radius,
            radiusY: radius,
        };

        unsafe {
            let bg = self.brushes.scratch(2, palette.menu_fill);
            let border = self.brushes.scratch(3, palette.dock_inner_border);
            let text_brush = self.brushes.scratch(4, palette.text_primary);

            self.target.FillRoundedRectangle(&pill, bg);
            self.target.DrawRoundedRectangle(&pill, border, 1.0, None);

            let pointer = self.factory.CreatePathGeometry()?;
            let sink = pointer.Open()?;
            sink.BeginFigure(
                Vector2::new(center_x - arrow_size, bottom - 0.5),
                D2D1_FIGURE_BEGIN_FILLED,
            );
            sink.AddLine(Vector2::new(center_x + arrow_size, bottom - 0.5));
            sink.AddLine(Vector2::new(center_x, bottom + arrow_size));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            sink.Close()?;
            self.target.FillGeometry(&pointer, bg, None);

            let wide: Vec<u16> = title.encode_utf16().collect();
            self.target.DrawText(
                &wide,
                &self.tooltip_format,
                &pill.rect,
                text_brush,
                D2D1_DRAW_TEXT_OPTIONS_NO_SNAP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }
}

impl Drop for DockRenderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.dib.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
