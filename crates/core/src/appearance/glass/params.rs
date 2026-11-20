use crate::theme::ResolvedTheme;

use super::color::luminance;
use super::color::mix;
use super::color::scale_rgb;
use super::compose::smoothstep;

pub type Rgb = [f32; 3];

pub const LIGHT_DIRECTION: (f32, f32) = (-0.6, -0.8);
pub(super) const BASE_DPI: f32 = 96.0;
pub(super) const WHITE: Rgb = [1.0, 1.0, 1.0];
pub(super) const BLACK: Rgb = [0.0, 0.0, 0.0];
pub(super) const BLUR_PASSES: usize = 3;
pub(super) const OUTLINE_REACH: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassParams {
    pub tint: Rgb,
    pub tint_amount: f32,
    pub luma_floor: f32,
    pub luma_ceiling: f32,
    pub saturation: f32,
    pub sheen_alpha: f32,
    pub rim_base: f32,
    pub rim_lit: f32,
    pub rim_bounce: f32,
    pub rim_width: f32,
    pub inner_glow: f32,
    pub inner_glow_depth: f32,
    pub outline_alpha: f32,
    pub shadow_strength: f32,
    pub shadow_depth: f32,
    pub lens_band: f32,
    pub lens_shift: f32,
    pub lens_mix: f32,
    pub lens_tint_scale: f32,
    pub dispersion: f32,
}

impl GlassParams {
    pub const DARK: Self = Self {
        tint: [0.12, 0.14, 0.20],
        tint_amount: 0.05,
        luma_floor: 0.01,
        luma_ceiling: 0.66,
        saturation: 1.30,
        sheen_alpha: 0.02,
        rim_base: 0.02,
        rim_lit: 0.55,
        rim_bounce: 0.20,
        rim_width: 1.2,
        inner_glow: 0.05,
        inner_glow_depth: 8.0,
        outline_alpha: 0.03,
        shadow_strength: 0.04,
        shadow_depth: 8.0,
        lens_band: 10.0,
        lens_shift: 6.0,
        lens_mix: 0.92,
        lens_tint_scale: 0.25,
        dispersion: 0.5,
    };

    pub const LIGHT: Self = Self {
        tint: [0.97, 0.98, 1.0],
        tint_amount: 0.07,
        luma_floor: 0.52,
        luma_ceiling: 1.0,
        saturation: 1.30,
        sheen_alpha: 0.03,
        rim_base: 0.03,
        rim_lit: 0.45,
        rim_bounce: 0.16,
        rim_width: 1.2,
        inner_glow: 0.06,
        inner_glow_depth: 8.0,
        outline_alpha: 0.04,
        shadow_strength: 0.02,
        shadow_depth: 6.0,
        lens_band: 10.0,
        lens_shift: 6.0,
        lens_mix: 0.92,
        lens_tint_scale: 0.25,
        dispersion: 0.5,
    };

    #[must_use]
    pub const fn for_theme(theme: ResolvedTheme) -> Self {
        match theme {
            ResolvedTheme::Dark => Self::DARK,
            ResolvedTheme::Light => Self::LIGHT,
        }
    }

    #[must_use]
    pub fn scaled(self, dpi: u32) -> Self {
        let factor = dpi as f32 / BASE_DPI;
        Self {
            rim_width: self.rim_width * factor,
            inner_glow_depth: self.inner_glow_depth * factor,
            shadow_depth: self.shadow_depth * factor,
            lens_band: self.lens_band * factor,
            lens_shift: self.lens_shift * factor,
            dispersion: self.dispersion * factor,
            ..self
        }
    }

    #[must_use]
    pub const fn without_lens(self) -> Self {
        Self {
            lens_mix: 0.0,
            dispersion: 0.0,
            ..self
        }
    }

    #[must_use]
    pub fn grade(&self, rgb: Rgb, tint_scale: f32) -> Rgb {
        let luma = luminance(rgb);
        let chroma = rgb
            .iter()
            .map(|channel| (channel - luma).abs())
            .fold(0.0f32, f32::max);
        let compressed_luma = if self.luma_ceiling < 0.999 {
            let shoulder = luma / (1.0 + luma / self.luma_ceiling.max(1e-3));
            let roll = smoothstep(luma * 1.15);
            luma + (shoulder - luma) * roll
        } else {
            luma
        };
        let target_luma = self.luma_floor + (1.0 - self.luma_floor) * compressed_luma;
        let luma_scale = if luma > 1e-4 { target_luma / luma } else { 1.0 };
        let chroma_scale = if luma > 1e-4 {
            0.40 + 0.60 * luma_scale
        } else {
            1.0
        };
        let chroma_relief = 1.0 - 0.30 * smoothstep(chroma * 2.0);
        let amount = (self.tint_amount * tint_scale * chroma_relief).clamp(0.0, 1.0);
        let mut graded = [0.0; 3];
        for ((out, channel), tint) in graded.iter_mut().zip(rgb).zip(self.tint) {
            let base = if luma > 1e-4 {
                target_luma + (channel - luma) * self.saturation * chroma_scale
            } else {
                target_luma
            };
            *out = (base.clamp(0.0, 1.0) * (1.0 - amount) + tint * amount).clamp(0.0, 1.0);
        }
        graded
    }

    #[must_use]
    pub fn interior_extremes(&self) -> (Rgb, Rgb) {
        let lightest = mix(self.grade(WHITE, 1.0), WHITE, self.sheen_alpha);
        let darkest = scale_rgb(self.grade(BLACK, 1.0), 1.0 - self.shadow_strength);
        (lightest, darkest)
    }
}
