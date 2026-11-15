use otto_core::scale;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
};
use windows_numerics::Vector2;

use crate::glow::{linear_gradient, radial_fade};
use crate::{FOREGROUND, Renderer};

pub(crate) const AMBIENT_RADIUS_PT: i32 = 96;
pub(crate) const AMBIENT_PEAK_ALPHA: f32 = 0.20;
const DIM_MIX: f32 = 0.65;
const DIM_LUMINANCE: f32 = 0.72;
const LIP_HALO_POINTS: i32 = 2;
const LIP_HALO_ALPHA: f32 = 0.20;
const SOFT_SEGMENT_SHARE: f32 = 0.30;

#[derive(Clone, Copy)]
pub(crate) struct TonePalette {
    pub from: D2D1_COLOR_F,
    pub to: D2D1_COLOR_F,
    pub accent: D2D1_COLOR_F,
    pub text: D2D1_COLOR_F,
}

impl TonePalette {
    pub(crate) fn from_base(base: D2D1_COLOR_F, lift: f32) -> Self {
        let white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: base.a,
        };
        Self {
            from: mix(base, white, lift),
            to: base,
            accent: base,
            text: mix(base, white, lift * 0.5),
        }
    }

    pub(crate) fn dimmed(self, dim: f32) -> Self {
        let amount = dim.clamp(0.0, 1.0) * DIM_MIX;
        if amount <= 0.0 {
            return self;
        }
        Self {
            from: muted(self.from, amount),
            to: muted(self.to, amount),
            accent: muted(self.accent, amount),
            text: muted(self.text, amount),
        }
    }
}

pub(crate) fn mix(a: D2D1_COLOR_F, b: D2D1_COLOR_F, t: f32) -> D2D1_COLOR_F {
    let t = t.clamp(0.0, 1.0);
    D2D1_COLOR_F {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub(crate) fn muted(color: D2D1_COLOR_F, amount: f32) -> D2D1_COLOR_F {
    let luminance = (color.r * 0.30 + color.g * 0.59 + color.b * 0.11) * DIM_LUMINANCE;
    mix(
        color,
        D2D1_COLOR_F {
            r: luminance,
            g: luminance,
            b: luminance,
            a: color.a,
        },
        amount,
    )
}

pub(crate) const fn with_alpha(color: D2D1_COLOR_F, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        a: color.a * alpha,
        ..color
    }
}

impl Renderer {
    pub(crate) fn fill_gradient_rounded(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        colors: &TonePalette,
        alpha: f32,
    ) {
        let shape = D2D1_ROUNDED_RECT {
            rect,
            radiusX: radius,
            radiusY: radius,
        };
        let gradient = linear_gradient(
            &self.target,
            Vector2::new(rect.left, rect.top),
            Vector2::new(rect.right, rect.top),
            with_alpha(colors.from, alpha),
            with_alpha(colors.to, alpha),
        );
        unsafe {
            if let Ok(brush) = gradient {
                self.target.FillRoundedRectangle(&shape, &brush);
            } else {
                let solid = self.brushes.alpha_brush(colors.accent, alpha);
                self.target.FillRoundedRectangle(&shape, &solid);
            }
        }
    }

    pub(crate) fn fill_gradient_circle(
        &self,
        center: Vector2,
        radius: f32,
        colors: &TonePalette,
        alpha: f32,
    ) {
        let shape = D2D1_ELLIPSE {
            point: center,
            radiusX: radius,
            radiusY: radius,
        };
        let gradient = linear_gradient(
            &self.target,
            Vector2::new(center.X - radius, center.Y - radius),
            Vector2::new(center.X + radius, center.Y + radius),
            with_alpha(colors.from, alpha),
            with_alpha(colors.to, alpha),
        );
        unsafe {
            if let Ok(brush) = gradient {
                self.target.FillEllipse(&shape, &brush);
            } else {
                let solid = self.brushes.alpha_brush(colors.accent, alpha);
                self.target.FillEllipse(&shape, &solid);
            }
        }
    }

    pub(crate) fn fill_ambient_glow(&self, center: Vector2, color: D2D1_COLOR_F, clip: D2D_RECT_F) {
        let radius = scale(AMBIENT_RADIUS_PT, self.dpi) as f32;
        let Ok(fade) = radial_fade(&self.target, center, radius, color) else {
            return;
        };
        unsafe {
            self.target
                .PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            self.target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: center,
                    radiusX: radius,
                    radiusY: radius,
                },
                &fade,
            );
            self.target.PopAxisAlignedClip();
        }
    }

    pub(crate) fn draw_progress_lip(
        &self,
        lip: otto_core::Rect,
        filled_fraction: f32,
        palette: &TonePalette,
        glow: f32,
        alpha: f32,
    ) {
        if lip.is_empty() {
            return;
        }
        let radius = lip.height as f32 * 0.5;
        let track = self.brushes.alpha_brush(FOREGROUND, 0.10 * alpha);
        self.draw_rounded_box(lip, radius, &track);
        let filled = (lip.width as f32 * filled_fraction.clamp(0.0, 1.0)).round();
        if filled < lip.height as f32 {
            return;
        }
        let rect = D2D_RECT_F {
            left: lip.x as f32,
            top: lip.y as f32,
            right: lip.x as f32 + filled,
            bottom: lip.bottom() as f32,
        };
        self.draw_lip_fill(rect, radius, palette, glow, alpha);
    }

    pub(crate) fn draw_drifting_lip(
        &self,
        lip: otto_core::Rect,
        phase: f32,
        palette: &TonePalette,
        glow: f32,
        alpha: f32,
    ) {
        if lip.is_empty() {
            return;
        }
        let radius = lip.height as f32 * 0.5;
        let track = self.brushes.alpha_brush(FOREGROUND, 0.10 * alpha);
        self.draw_rounded_box(lip, radius, &track);
        let span = lip.width as f32 * SOFT_SEGMENT_SHARE;
        let centre = lip.x as f32 + span * 0.5 + (lip.width as f32 - span) * phase.clamp(0.0, 1.0);
        let top = lip.y as f32;
        let bottom = lip.bottom() as f32;
        let halves = [
            (centre - span * 0.5, centre, palette.accent, true),
            (centre, centre + span * 0.5, palette.accent, false),
        ];
        for (left, right, colour, rising) in halves {
            let (start, end) = if rising {
                (with_alpha(colour, 0.0), with_alpha(colour, alpha))
            } else {
                (with_alpha(colour, alpha), with_alpha(colour, 0.0))
            };
            if let Ok(brush) = linear_gradient(
                &self.target,
                Vector2::new(left, top),
                Vector2::new(right, top),
                start,
                end,
            ) {
                unsafe {
                    self.target.FillRectangle(
                        &D2D_RECT_F {
                            left,
                            top,
                            right,
                            bottom,
                        },
                        &brush,
                    );
                }
            }
        }
        let _ = glow;
    }

    fn draw_lip_fill(
        &self,
        rect: D2D_RECT_F,
        radius: f32,
        palette: &TonePalette,
        glow: f32,
        alpha: f32,
    ) {
        if glow > 0.01 {
            let halo = scale(LIP_HALO_POINTS, self.dpi) as f32;
            let soft = self
                .brushes
                .alpha_brush(palette.accent, LIP_HALO_ALPHA * glow * alpha);
            unsafe {
                self.target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: rect.left - halo,
                            top: rect.top - halo,
                            right: rect.right + halo,
                            bottom: rect.bottom + halo,
                        },
                        radiusX: radius + halo,
                        radiusY: radius + halo,
                    },
                    &soft,
                );
            }
        }
        self.fill_gradient_rounded(rect, radius, palette, alpha);
    }
}
