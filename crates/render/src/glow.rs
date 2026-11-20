use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D1_GRADIENT_STOP};
#[cfg(test)]
use windows::Win32::Graphics::Direct2D::{
    D2D1_ELLIPSE, D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES, ID2D1RadialGradientBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
    ID2D1DCRenderTarget, ID2D1LinearGradientBrush,
};
use windows::core::Result;
use windows_numerics::Vector2;

#[cfg(test)]
use crate::theme::BrushPalette;

#[cfg(test)]
const GLOW_LAYERS: [GlowLayer; 3] = [
    GlowLayer {
        spread: 1.2,
        alpha: 0.30,
    },
    GlowLayer {
        spread: 2.4,
        alpha: 0.16,
    },
    GlowLayer {
        spread: 3.8,
        alpha: 0.07,
    },
];

#[cfg(test)]
#[derive(Clone, Copy)]
struct GlowLayer {
    spread: f32,
    alpha: f32,
}

pub(crate) fn linear_gradient(
    target: &ID2D1DCRenderTarget,
    start: Vector2,
    end: Vector2,
    from: D2D1_COLOR_F,
    to: D2D1_COLOR_F,
) -> Result<ID2D1LinearGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color: from,
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: to,
        },
    ];
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        target.CreateLinearGradientBrush(
            &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: start,
                endPoint: end,
            },
            None,
            &collection,
        )
    }
}

#[cfg(test)]
pub(crate) fn radial_fade(
    target: &ID2D1DCRenderTarget,
    center: Vector2,
    radius: f32,
    color: D2D1_COLOR_F,
) -> Result<ID2D1RadialGradientBrush> {
    let stops = [
        D2D1_GRADIENT_STOP {
            position: 0.0,
            color,
        },
        D2D1_GRADIENT_STOP {
            position: 1.0,
            color: D2D1_COLOR_F { a: 0.0, ..color },
        },
    ];
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        target.CreateRadialGradientBrush(
            &D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                center,
                gradientOriginOffset: Vector2::new(0.0, 0.0),
                radiusX: radius,
                radiusY: radius,
            },
            None,
            &collection,
        )
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_ring_glow(
    target: &ID2D1DCRenderTarget,
    brushes: &BrushPalette,
    stroke_style: &ID2D1StrokeStyle,
    center: Vector2,
    radius: f32,
    stroke: f32,
    color: D2D1_COLOR_F,
    intensity: f32,
) {
    let intensity = intensity.clamp(0.0, 1.0);
    if intensity <= 0.0 {
        return;
    }
    let ellipse = D2D1_ELLIPSE {
        point: center,
        radiusX: radius,
        radiusY: radius,
    };
    for layer in GLOW_LAYERS.iter().rev() {
        let brush = brushes.alpha_brush(color, layer.alpha * intensity);
        unsafe {
            target.DrawEllipse(&ellipse, &brush, stroke + layer.spread * 2.0, stroke_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_tests::SERIAL;
    use crate::surface::{DibSurface, create_dc_render_target, draw_on};
    use crate::{release_shared_factories, shared_d2d_factory};
    use std::sync::PoisonError;
    use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
    use windows::Win32::Graphics::Direct2D::D2D1_STROKE_STYLE_PROPERTIES;

    const ORANGE: D2D1_COLOR_F = D2D1_COLOR_F {
        r: 1.0,
        g: 0.62,
        b: 0.04,
        a: 1.0,
    };
    const RED: D2D1_COLOR_F = D2D1_COLOR_F {
        r: 1.0,
        g: 0.27,
        b: 0.23,
        a: 1.0,
    };
    const SIZE: (i32, i32) = (96, 48);

    fn paint_on_fresh_target(
        paint: impl FnOnce(&ID2D1DCRenderTarget, &BrushPalette, &ID2D1StrokeStyle) -> Result<()>,
    ) {
        let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        {
            let factory = shared_d2d_factory().expect("factory");
            let target = create_dc_render_target(&factory).expect("DC target");
            let stroke_style = unsafe {
                factory.CreateStrokeStyle(&D2D1_STROKE_STYLE_PROPERTIES::default(), None)
            }
            .expect("stroke style");
            let brushes = BrushPalette::new(&target).expect("brushes");
            let surface = DibSurface::new(SIZE.0, SIZE.1).expect("surface");
            draw_on(&target, &surface, SIZE, || {
                paint(&target, &brushes, &stroke_style)
            })
            .expect("paints without a device error");
        }
        release_shared_factories();
    }

    #[test]
    fn gradient_brush_and_stacked_glow_draw_on_the_dc_target() {
        paint_on_fresh_target(|target, brushes, stroke_style| {
            let gradient = linear_gradient(
                target,
                Vector2::new(8.0, 0.0),
                Vector2::new(88.0, 0.0),
                ORANGE,
                RED,
            )?;
            unsafe {
                target.FillRectangle(
                    &D2D_RECT_F {
                        left: 8.0,
                        top: 40.0,
                        right: 88.0,
                        bottom: 43.0,
                    },
                    &gradient,
                );
            }
            draw_ring_glow(
                target,
                brushes,
                stroke_style,
                Vector2::new(24.0, 20.0),
                8.0,
                2.0,
                ORANGE,
                1.0,
            );
            Ok(())
        });
    }

    #[test]
    fn radial_fade_fills_an_ellipse_on_the_dc_target() {
        paint_on_fresh_target(|target, _brushes, _stroke_style| {
            let fade = radial_fade(target, Vector2::new(24.0, 24.0), 20.0, ORANGE)?;
            unsafe {
                target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2::new(24.0, 24.0),
                        radiusX: 20.0,
                        radiusY: 20.0,
                    },
                    &fade,
                );
            }
            Ok(())
        });
    }

    #[test]
    fn zero_intensity_glow_paints_nothing() {
        paint_on_fresh_target(|target, brushes, stroke_style| {
            draw_ring_glow(
                target,
                brushes,
                stroke_style,
                Vector2::new(16.0, 16.0),
                6.0,
                2.0,
                ORANGE,
                0.0,
            );
            Ok(())
        });
    }
}
