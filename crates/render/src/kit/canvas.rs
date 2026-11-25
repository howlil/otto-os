use otto_core::{PixelLimits, Rect, WindowControlAction, scale};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_CAP_STYLE_ROUND,
    D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LINE_JOIN_ROUND, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES, ID2D1Bitmap,
    ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT, DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT,
    DWRITE_WORD_WRAPPING_WRAP, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::appearance::surface::upload_bgra_bitmap;
use crate::appearance::tokens::rgba_hex;
use crate::kit::draw::{fill_squircle_or_rounded_f, stroke_squircle_or_rounded_f};
use crate::kit::icons::{
    ChevronDir, draw_chevron_glyph, draw_close_x_glyph, draw_search_magnifier_glyph,
};
use crate::window::controls::{TrafficLightsStyle, draw_traffic_lights};
use crate::{create_text_format, rect_to_d2d, shared_d2d_factory, shared_dwrite_factory};

pub(crate) fn make_text_format(
    points: i32,
    dpi: u32,
    weight: DWRITE_FONT_WEIGHT,
    align: DWRITE_TEXT_ALIGNMENT,
    wrap: bool,
) -> Result<IDWriteTextFormat> {
    let dwrite = shared_dwrite_factory()?;
    unsafe {
        let format = create_text_format(&dwrite, scale(points, dpi) as f32, weight, align)?;
        if wrap {
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
        }
        Ok(format)
    }
}

pub(crate) struct HwndCanvas {
    pub(crate) factory: ID2D1Factory,
    pub(crate) target: ID2D1HwndRenderTarget,
    pub(crate) brush: ID2D1SolidColorBrush,
    pub(crate) stroke: ID2D1StrokeStyle,
}

impl HwndCanvas {
    pub(crate) fn new(hwnd: isize, width: i32, height: i32) -> Result<Self> {
        unsafe {
            let factory = shared_d2d_factory()?;
            let target = factory.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    ..D2D1_RENDER_TARGET_PROPERTIES::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: HWND(hwnd as *mut _),
                    pixelSize: D2D_SIZE_U {
                        width: width.max(1) as u32,
                        height: height.max(1) as u32,
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                },
            )?;
            target.SetDpi(96.0, 96.0);
            let brush = target.CreateSolidColorBrush(&rgba_hex(0, 1.0), None)?;
            let stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 2.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )?;
            Ok(Self {
                factory,
                target,
                brush,
                stroke,
            })
        }
    }

    pub(crate) fn resize(&self, width: i32, height: i32) -> Result<()> {
        unsafe {
            self.target.Resize(&D2D_SIZE_U {
                width: width.max(1) as u32,
                height: height.max(1) as u32,
            })
        }
    }

    pub(crate) fn begin_frame(&self, clear: D2D1_COLOR_F) {
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&clear));
        }
    }

    pub(crate) fn end_frame(&self) -> Result<()> {
        unsafe { self.target.EndDraw(None, None) }
    }

    pub(crate) fn push_clip(&self, rect: Rect) {
        unsafe {
            self.target
                .PushAxisAlignedClip(&rect_to_d2d(rect), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
    }

    pub(crate) fn pop_clip(&self) {
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }

    pub(crate) fn fill(&self, rect: Rect, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillRectangle(&rect_to_d2d(rect), &self.brush);
        }
    }

    pub(crate) fn fill_squircle_f(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        dpi: u32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
        }
        fill_squircle_or_rounded_f(&self.factory, &self.target, rect, radius, dpi, &self.brush);
    }

    pub(crate) fn stroke_squircle_f(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        dpi: u32,
        width: f32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
        }
        stroke_squircle_or_rounded_f(
            &self.factory,
            &self.target,
            rect,
            radius,
            dpi,
            &self.brush,
            width,
        );
    }

    pub(crate) fn chevron(
        &self,
        center: Vector2,
        reach: f32,
        width: f32,
        dir: ChevronDir,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
        }
        draw_chevron_glyph(
            &self.target,
            &self.stroke,
            center,
            reach,
            width,
            dir,
            &self.brush,
        );
    }

    pub(crate) fn close_x(&self, center: Vector2, arm: f32, width: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
        }
        draw_close_x_glyph(&self.target, &self.stroke, center, arm, width, &self.brush);
    }

    pub(crate) fn search_magnifier(
        &self,
        center: Vector2,
        radius: f32,
        width: f32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
        }
        draw_search_magnifier_glyph(
            &self.target,
            &self.stroke,
            center,
            radius,
            width,
            &self.brush,
        );
    }

    pub(crate) fn fill_round(&self, rect: Rect, radius: f32, color: D2D1_COLOR_F) {
        self.fill_round_f(rect_to_d2d(rect), radius, color);
    }

    pub(crate) fn fill_round_f(&self, rect: D2D_RECT_F, radius: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
            );
        }
    }

    pub(crate) fn stroke_round(&self, rect: Rect, radius: f32, width: f32, color: D2D1_COLOR_F) {
        let inset = D2D_RECT_F {
            left: rect.x as f32 + 0.5,
            top: rect.y as f32 + 0.5,
            right: rect.right() as f32 - 0.5,
            bottom: rect.bottom() as f32 - 0.5,
        };
        self.stroke_round_f(inset, radius, width, color);
    }

    pub(crate) fn stroke_round_f(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        width: f32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect,
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
                width,
                &self.stroke,
            );
        }
    }

    pub(crate) fn fill_circle(&self, cx: f32, cy: f32, radius: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
            );
        }
    }

    pub(crate) fn stroke_circle(
        &self,
        cx: f32,
        cy: f32,
        radius: f32,
        width: f32,
        color: D2D1_COLOR_F,
    ) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2::new(cx, cy),
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
                width,
                &self.stroke,
            );
        }
    }

    pub(crate) fn line(&self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: D2D1_COLOR_F) {
        unsafe {
            self.brush.SetColor(&color);
            self.target.DrawLine(
                Vector2::new(x0, y0),
                Vector2::new(x1, y1),
                &self.brush,
                width,
                &self.stroke,
            );
        }
    }

    pub(crate) fn text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: Rect,
        color: D2D1_COLOR_F,
    ) {
        if text.is_empty() || rect.is_empty() {
            return;
        }
        unsafe {
            self.brush.SetColor(&color);
        }
        crate::kit::draw::draw_text_on(
            &self.target,
            text,
            format,
            rect_to_d2d(rect),
            &self.brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
        );
    }

    pub(crate) fn tabular_text(
        &self,
        text: &str,
        format: &IDWriteTextFormat,
        rect: Rect,
        color: D2D1_COLOR_F,
    ) {
        if text.is_empty() || rect.is_empty() {
            return;
        }
        unsafe {
            self.brush.SetColor(&color);
        }
        crate::draw_dwrite_tabular_text(&self.target, text, format, rect_to_d2d(rect), &self.brush);
    }

    pub(crate) fn upload_bitmap(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
        limits: PixelLimits,
    ) -> Result<ID2D1Bitmap> {
        upload_bgra_bitmap(&self.target, width, height, pixels, limits)
    }

    pub(crate) fn draw_bitmap(&self, bitmap: &ID2D1Bitmap, rect: Rect) {
        unsafe {
            self.target.DrawBitmap(
                bitmap,
                Some(&rect_to_d2d(rect)),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                None,
            );
        }
    }

    pub(crate) fn draw_traffic_lights(
        &self,
        slots: &[(WindowControlAction, Rect)],
        radius: f32,
        style: TrafficLightsStyle,
    ) {
        let _ = draw_traffic_lights(
            &self.factory,
            &self.target,
            &self.brush,
            &self.stroke,
            slots,
            radius,
            style,
        );
    }
}
