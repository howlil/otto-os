use otto_core::{Appearance, Rgba, WindowControlsLayout, WindowControlsState};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID, D2D1_ELLIPSE, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_LINE_JOIN_ROUND, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_STROKE_STYLE_PROPERTIES, ID2D1DCRenderTarget, ID2D1Factory,
    ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Result;
use windows_numerics::Vector2;

use crate::shared_d2d_factory;
use crate::surface::{DibSurface, draw_on};

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
const DISABLED_FILL: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.80,
    g: 0.80,
    b: 0.82,
    a: 0.55,
};
const DISABLED_BORDER: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.55,
    g: 0.55,
    b: 0.58,
    a: 0.55,
};
const GLYPH_DARK: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.22,
    g: 0.08,
    b: 0.04,
    a: 0.90,
};
const fn d2d(color: Rgba) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

const PIXEL_FORMAT: D2D1_PIXEL_FORMAT = D2D1_PIXEL_FORMAT {
    format: DXGI_FORMAT_B8G8R8A8_UNORM,
    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
};

pub struct WindowControlsRenderer {
    factory: ID2D1Factory,
    target: ID2D1DCRenderTarget,
    stroke_style: ID2D1StrokeStyle,
    red_fill: ID2D1SolidColorBrush,
    red_border: ID2D1SolidColorBrush,
    yellow_fill: ID2D1SolidColorBrush,
    yellow_border: ID2D1SolidColorBrush,
    green_fill: ID2D1SolidColorBrush,
    green_border: ID2D1SolidColorBrush,
    disabled_fill: ID2D1SolidColorBrush,
    disabled_border: ID2D1SolidColorBrush,
    glyph_brush: ID2D1SolidColorBrush,
    surface: DibSurface,
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

            let red_fill = target.CreateSolidColorBrush(&RED_FILL, None)?;
            let red_border = target.CreateSolidColorBrush(&RED_BORDER, None)?;
            let yellow_fill = target.CreateSolidColorBrush(&YELLOW_FILL, None)?;
            let yellow_border = target.CreateSolidColorBrush(&YELLOW_BORDER, None)?;
            let green_fill = target.CreateSolidColorBrush(&GREEN_FILL, None)?;
            let green_border = target.CreateSolidColorBrush(&GREEN_BORDER, None)?;
            let disabled_fill = target.CreateSolidColorBrush(&DISABLED_FILL, None)?;
            let disabled_border = target.CreateSolidColorBrush(&DISABLED_BORDER, None)?;
            let glyph_brush = target.CreateSolidColorBrush(&GLYPH_DARK, None)?;

            let surface = DibSurface::new(width, height)?;

            Ok(Self {
                factory,
                target,
                stroke_style,
                red_fill,
                red_border,
                yellow_fill,
                yellow_border,
                green_fill,
                green_border,
                disabled_fill,
                disabled_border,
                glyph_brush,
                surface,
            })
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &self,
        hwnd: isize,
        origin: (i32, i32),
        layout: &WindowControlsLayout,
        state: WindowControlsState,
        maximized: bool,
        hovered: bool,
        appearance: Appearance,
    ) -> Result<()> {
        let high_contrast = self.apply_appearance(&appearance);
        self.draw(layout, state, maximized, hovered || high_contrast)?;
        self.surface.present(hwnd, origin)
    }

    fn apply_appearance(&self, appearance: &Appearance) -> bool {
        let colors = match appearance.contrast {
            Some(system) => [
                d2d(system.window),
                d2d(system.window_text),
                d2d(system.window),
                d2d(system.window_text),
                d2d(system.window),
                d2d(system.window_text),
                d2d(system.window),
                d2d(system.gray_text),
                d2d(system.window_text),
            ],
            None => [
                RED_FILL,
                RED_BORDER,
                YELLOW_FILL,
                YELLOW_BORDER,
                GREEN_FILL,
                GREEN_BORDER,
                DISABLED_FILL,
                DISABLED_BORDER,
                GLYPH_DARK,
            ],
        };
        let brushes = [
            &self.red_fill,
            &self.red_border,
            &self.yellow_fill,
            &self.yellow_border,
            &self.green_fill,
            &self.green_border,
            &self.disabled_fill,
            &self.disabled_border,
            &self.glyph_brush,
        ];
        for (brush, color) in brushes.into_iter().zip(colors) {
            unsafe {
                brush.SetColor(&raw const color);
            }
        }
        appearance.contrast.is_some()
    }

    fn draw(
        &self,
        layout: &WindowControlsLayout,
        state: WindowControlsState,
        maximized: bool,
        hovered: bool,
    ) -> Result<()> {
        draw_on(
            &self.target,
            &self.surface,
            self.surface.size(),
            || unsafe {
                let clear = CLEAR;
                self.target.Clear(Some(&raw const clear));

                let [close_x, min_x, max_x] = layout.centers_x;
                let cy = layout.center_y;
                let r = layout.radius;

                let pick = |enabled: bool,
                            fill: &'_ ID2D1SolidColorBrush,
                            border: &'_ ID2D1SolidColorBrush| {
                    if enabled {
                        (fill.clone(), border.clone())
                    } else {
                        (self.disabled_fill.clone(), self.disabled_border.clone())
                    }
                };
                let (fill, border) = pick(state.close, &self.red_fill, &self.red_border);
                self.draw_circle(&fill, &border, close_x, cy, r);
                let (fill, border) = pick(state.minimize, &self.yellow_fill, &self.yellow_border);
                self.draw_circle(&fill, &border, min_x, cy, r);
                let (fill, border) = pick(state.zoom, &self.green_fill, &self.green_border);
                self.draw_circle(&fill, &border, max_x, cy, r);

                if hovered {
                    if state.close {
                        self.draw_close_glyph(&self.glyph_brush, close_x, cy);
                    }
                    if state.minimize {
                        self.draw_minimize_glyph(&self.glyph_brush, min_x, cy);
                    }
                    if state.zoom {
                        self.draw_zoom_glyph(&self.glyph_brush, max_x, cy, maximized)?;
                    }
                }
                Ok(())
            },
        )
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
