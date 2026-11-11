use std::ffi::c_void;
use std::mem::size_of;

use otto_core::WindowControlsLayout;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_ELLIPSE, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_LINE_JOIN_ROUND, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
    ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush, ID2D1StrokeStyle,
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

use crate::shared_d2d_factory;

const CLEAR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
const RED_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.373,
    b: 0.337,
    a: 1.0,
};
const RED_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.878,
    g: 0.267,
    b: 0.243,
    a: 1.0,
};
const YELLOW_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 0.741,
    b: 0.180,
    a: 1.0,
};
const YELLOW_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.871,
    g: 0.631,
    b: 0.137,
    a: 1.0,
};
const GREEN_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.153,
    g: 0.788,
    b: 0.247,
    a: 1.0,
};
const GREEN_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.102,
    g: 0.671,
    b: 0.161,
    a: 1.0,
};
const GLYPH_DARK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.22,
    g: 0.08,
    b: 0.04,
    a: 0.90,
};
const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

pub struct WindowControlsRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    bg_brush: ID2D1SolidColorBrush,
    red_fill: ID2D1SolidColorBrush,
    red_border: ID2D1SolidColorBrush,
    yellow_fill: ID2D1SolidColorBrush,
    yellow_border: ID2D1SolidColorBrush,
    green_fill: ID2D1SolidColorBrush,
    green_border: ID2D1SolidColorBrush,
    glyph_brush: ID2D1SolidColorBrush,
    dc: HDC,
    dib: HBITMAP,
    previous: HGDIOBJ,
    size: (i32, i32),
}

pub type TrafficLightRenderer = WindowControlsRenderer;

impl WindowControlsRenderer {
    pub fn new(width: i32, height: i32) -> Result<Self> {
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

            let bg_brush = target.CreateSolidColorBrush(&CLEAR, None)?;
            let red_fill = target.CreateSolidColorBrush(&RED_FILL, None)?;
            let red_border = target.CreateSolidColorBrush(&RED_BORDER, None)?;
            let yellow_fill = target.CreateSolidColorBrush(&YELLOW_FILL, None)?;
            let yellow_border = target.CreateSolidColorBrush(&YELLOW_BORDER, None)?;
            let green_fill = target.CreateSolidColorBrush(&GREEN_FILL, None)?;
            let green_border = target.CreateSolidColorBrush(&GREEN_BORDER, None)?;
            let glyph_brush = target.CreateSolidColorBrush(&GLYPH_DARK, None)?;

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

            Ok(Self {
                factory,
                target,
                stroke_style,
                bg_brush,
                red_fill,
                red_border,
                yellow_fill,
                yellow_border,
                green_fill,
                green_border,
                glyph_brush,
                dc,
                dib,
                previous,
                size: (width, height),
            })
        }
    }

    pub fn present(
        &self,
        hwnd: isize,
        origin: (i32, i32),
        layout: &WindowControlsLayout,
        titlebar_rgb: (u8, u8, u8),
        maximized: bool,
        hovered: bool,
    ) -> Result<()> {
        self.draw(layout, titlebar_rgb, maximized, hovered)?;
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
        layout: &WindowControlsLayout,
        titlebar_rgb: (u8, u8, u8),
        maximized: bool,
        hovered: bool,
    ) -> Result<()> {
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

            let bg_color = D2D1_COLOR_F {
                r: f32::from(titlebar_rgb.0) / 255.0,
                g: f32::from(titlebar_rgb.1) / 255.0,
                b: f32::from(titlebar_rgb.2) / 255.0,
                a: 1.0,
            };
            self.bg_brush.SetColor(&bg_color);
            if maximized {
                let rect = D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: width as f32,
                    bottom: height as f32,
                };
                self.target.FillRectangle(&rect, &self.bg_brush);
            } else {
                let rounded = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: 0.0,
                        top: 0.0,
                        right: width as f32,
                        bottom: height as f32,
                    },
                    radiusX: 8.0,
                    radiusY: 8.0,
                };
                self.target.FillRoundedRectangle(&rounded, &self.bg_brush);
            }

            let [max_x, min_x, close_x] = layout.centers_x;
            let cy = layout.center_y;
            let r = layout.radius;

            self.draw_circle(&self.green_fill, &self.green_border, max_x, cy, r);
            self.draw_circle(&self.yellow_fill, &self.yellow_border, min_x, cy, r);
            self.draw_circle(&self.red_fill, &self.red_border, close_x, cy, r);

            if hovered {
                self.draw_zoom_glyph(&self.glyph_brush, max_x, cy, maximized)?;
                self.draw_minimize_glyph(&self.glyph_brush, min_x, cy);
                self.draw_close_glyph(&self.glyph_brush, close_x, cy);
            }

            self.target.EndDraw(None, None)
        }
    }

    unsafe fn draw_circle(
        &self,
        fill: &ID2D1SolidColorBrush,
        border: &ID2D1SolidColorBrush,
        cx: f32,
        cy: f32,
        radius: f32,
    ) {
        let ellipse = D2D1_ELLIPSE {
            point: Vector2::new(cx, cy),
            radiusX: radius,
            radiusY: radius,
        };
        unsafe {
            self.target.FillEllipse(&ellipse, fill);
            self.target
                .DrawEllipse(&ellipse, border, 0.5, Some(&self.stroke_style));
        }
    }

    unsafe fn draw_close_glyph(&self, brush: &ID2D1SolidColorBrush, cx: f32, cy: f32) {
        let arm = 3.0;
        let p0 = Vector2::new(cx - arm, cy - arm);
        let p1 = Vector2::new(cx + arm, cy + arm);
        let p2 = Vector2::new(cx + arm, cy - arm);
        let p3 = Vector2::new(cx - arm, cy + arm);
        unsafe {
            self.target
                .DrawLine(p0, p1, brush, 1.2, Some(&self.stroke_style));
            self.target
                .DrawLine(p2, p3, brush, 1.2, Some(&self.stroke_style));
        }
    }

    unsafe fn draw_minimize_glyph(&self, brush: &ID2D1SolidColorBrush, cx: f32, cy: f32) {
        let half = 3.5;
        let p0 = Vector2::new(cx - half, cy);
        let p1 = Vector2::new(cx + half, cy);
        unsafe {
            self.target
                .DrawLine(p0, p1, brush, 1.3, Some(&self.stroke_style));
        }
    }

    unsafe fn draw_zoom_glyph(
        &self,
        brush: &ID2D1SolidColorBrush,
        cx: f32,
        cy: f32,
        maximized: bool,
    ) -> Result<()> {
        unsafe {
            let path = self.factory.CreatePathGeometry()?;
            let sink = path.Open()?;
            if maximized {
                let size = 2.6;
                let gap = 1.0;
                let left = cx - size - gap;
                let right = cx + gap;
                let top = cy - size * 0.5;
                let bottom = cy + size * 0.5;

                sink.BeginFigure(Vector2::new(left, top), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(Vector2::new(left + size, top));
                sink.AddLine(Vector2::new(left + size, bottom));
                sink.AddLine(Vector2::new(left, bottom));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);

                sink.BeginFigure(Vector2::new(right, top), D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLine(Vector2::new(right + size, top));
                sink.AddLine(Vector2::new(right + size, bottom));
                sink.AddLine(Vector2::new(right, bottom));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            } else {
                let outer = 3.6;
                let leg = 2.4;

                sink.BeginFigure(
                    Vector2::new(cx - outer, cy - outer),
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                sink.AddLine(Vector2::new(cx - outer + leg, cy - outer));
                sink.AddLine(Vector2::new(cx - outer, cy - outer + leg));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);

                sink.BeginFigure(
                    Vector2::new(cx + outer, cy + outer),
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                sink.AddLine(Vector2::new(cx + outer - leg, cy + outer));
                sink.AddLine(Vector2::new(cx + outer, cy + outer - leg));
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            }
            sink.Close()?;
            self.target.FillGeometry(&path, brush, None);
        }
        Ok(())
    }
}

impl Drop for WindowControlsRenderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.dib.0));
            let _ = DeleteDC(self.dc);
        }
    }
}
