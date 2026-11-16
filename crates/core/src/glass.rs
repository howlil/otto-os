use crate::layout::Rect;
use crate::pixels::{PixelBufferError, PixelLimits, validate_bgra_buffer};
use crate::theme::ResolvedTheme;

pub type Rgb = [f32; 3];

const LIGHT_DIRECTION: (f32, f32) = (-0.6, -0.8);
const BASE_DPI: f32 = 96.0;
const WHITE: Rgb = [1.0, 1.0, 1.0];
const BLACK: Rgb = [0.0, 0.0, 0.0];
const BLUR_PASSES: usize = 3;
const OUTLINE_REACH: f32 = 1.0;

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
        saturation: 1.48,
        sheen_alpha: 0.07,
        rim_base: 0.04,
        rim_lit: 0.94,
        rim_bounce: 0.44,
        rim_width: 2.2,
        inner_glow: 0.26,
        inner_glow_depth: 13.0,
        outline_alpha: 0.03,
        shadow_strength: 0.04,
        shadow_depth: 8.0,
        lens_band: 22.0,
        lens_shift: 18.0,
        lens_mix: 0.92,
        lens_tint_scale: 0.25,
        dispersion: 3.0,
    };

    pub const LIGHT: Self = Self {
        tint: [0.97, 0.98, 1.0],
        tint_amount: 0.07,
        luma_floor: 0.52,
        luma_ceiling: 1.0,
        saturation: 1.42,
        sheen_alpha: 0.10,
        rim_base: 0.06,
        rim_lit: 0.96,
        rim_bounce: 0.46,
        rim_width: 2.2,
        inner_glow: 0.28,
        inner_glow_depth: 13.0,
        outline_alpha: 0.04,
        shadow_strength: 0.02,
        shadow_depth: 6.0,
        lens_band: 22.0,
        lens_shift: 18.0,
        lens_mix: 0.92,
        lens_tint_scale: 0.25,
        dispersion: 2.6,
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

#[derive(Debug, Clone, PartialEq)]
pub struct RgbBuffer {
    width: usize,
    height: usize,
    data: Vec<f32>,
}

impl RgbBuffer {
    pub fn from_bgra(width: u32, height: u32, pixels: &[u8]) -> Result<Self, PixelBufferError> {
        let layout = validate_bgra_buffer(width, height, pixels.len(), PixelLimits::BACKDROP)?;
        let data = pixels[..layout.byte_len]
            .chunks_exact(4)
            .flat_map(|bgra| {
                [
                    f32::from(bgra[2]) / 255.0,
                    f32::from(bgra[1]) / 255.0,
                    f32::from(bgra[0]) / 255.0,
                ]
            })
            .collect();
        Ok(Self {
            width: width as usize,
            height: height as usize,
            data,
        })
    }

    #[must_use]
    pub fn filled(width: usize, height: usize, color: Rgb) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            data: color
                .iter()
                .copied()
                .cycle()
                .take(width * height * 3)
                .collect(),
        }
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Rgb {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        let at = (y * self.width + x) * 3;
        [self.data[at], self.data[at + 1], self.data[at + 2]]
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            let at = (y * self.width + x) * 3;
            self.data[at..at + 3].copy_from_slice(&color);
        }
    }

    #[must_use]
    pub fn downsample(&self, factor: usize) -> Self {
        let factor = factor.max(1);
        let width = self.width.div_ceil(factor);
        let height = self.height.div_ceil(factor);
        let mut sums = vec![0.0f32; width * height * 3];
        let mut counts = vec![0.0f32; width * height];
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = (y / factor) * width + x / factor;
                let source = (y * self.width + x) * 3;
                for channel in 0..3 {
                    sums[cell * 3 + channel] += self.data[source + channel];
                }
                counts[cell] += 1.0;
            }
        }
        for (cell, count) in counts.iter().enumerate() {
            for channel in 0..3 {
                sums[cell * 3 + channel] /= count;
            }
        }
        Self {
            width,
            height,
            data: sums,
        }
    }
    #[must_use]
    pub fn linearized(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            data: self
                .data
                .iter()
                .map(|channel| srgb_to_linear(*channel))
                .collect(),
        }
    }

    #[must_use]
    pub fn gamma_encoded(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            data: self
                .data
                .iter()
                .map(|channel| linear_to_srgb(*channel))
                .collect(),
        }
    }

    #[must_use]
    pub fn box_blurred(&self, radius: usize) -> Self {
        let mut current = self.clone();
        let mut scratch = vec![0.0f32; self.data.len()];
        if radius == 0 {
            return current;
        }
        let (width, height) = (self.width, self.height);
        for _ in 0..BLUR_PASSES {
            blur_lines(&current.data, &mut scratch, width, height, radius, true);
            blur_lines(&scratch, &mut current.data, width, height, radius, false);
        }
        current
    }

    #[must_use]
    pub fn graded(mut self, params: &GlassParams) -> Self {
        for texel in self.data.chunks_exact_mut(3) {
            let graded = params.grade([texel[0], texel[1], texel[2]], 1.0);
            texel.copy_from_slice(&graded);
        }
        self
    }

    #[must_use]
    pub fn sample(&self, x: f32, y: f32) -> Rgb {
        let x = (x - 0.5).clamp(0.0, self.width as f32 - 1.0);
        let y = (y - 0.5).clamp(0.0, self.height as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (fx, fy) = (smoothstep(x - x0 as f32), smoothstep(y - y0 as f32));
        let top = mix(self.pixel(x0, y0), self.pixel(x0 + 1, y0), fx);
        let bottom = mix(self.pixel(x0, y0 + 1), self.pixel(x0 + 1, y0 + 1), fx);
        mix(top, bottom, fy)
    }

    #[must_use]
    pub fn average_luminance(&self) -> f32 {
        let count = (self.width * self.height).max(1);
        let sum: f32 = self
            .data
            .chunks_exact(3)
            .map(|rgb| luminance([rgb[0], rgb[1], rgb[2]]))
            .sum();
        (sum / count as f32).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn to_rgba_f32(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for rgb in self.data.chunks_exact(3) {
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 1.0]);
        }
        out
    }
}

fn blur_lines(
    source: &[f32],
    target: &mut [f32],
    width: usize,
    height: usize,
    radius: usize,
    horizontal: bool,
) {
    let (length, lines, step, line_step) = if horizontal {
        (width, height, 3, width * 3)
    } else {
        (height, width, width * 3, 3)
    };
    let reach = radius as isize;
    let window = (2 * radius + 1) as f32;
    for line in 0..lines {
        for channel in 0..3 {
            let base = line * line_step + channel;
            let at =
                |index: isize| source[base + index.clamp(0, length as isize - 1) as usize * step];
            let mut sum: f32 = (-reach..=reach).map(at).sum();
            for index in 0..length {
                target[base + index * step] = sum / window;
                let next = index as isize;
                sum += at(next + reach + 1) - at(next - reach);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassShape {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub radius: f32,
}

impl GlassShape {
    #[must_use]
    pub fn from_rect(rect: Rect, radius: f32) -> Self {
        let half_short_side = rect.width.min(rect.height).max(0) as f32 * 0.5;
        Self {
            left: rect.x as f32,
            top: rect.y as f32,
            right: rect.right() as f32,
            bottom: rect.bottom() as f32,
            radius: radius.clamp(0.0, half_short_side),
        }
    }

    #[must_use]
    pub fn distance(&self, x: f32, y: f32) -> f32 {
        let center_x = (self.left + self.right) * 0.5;
        let center_y = (self.top + self.bottom) * 0.5;
        let reach_x = (self.right - self.left) * 0.5 - self.radius;
        let reach_y = (self.bottom - self.top) * 0.5 - self.radius;
        let offset_x = (x - center_x).abs() - reach_x;
        let offset_y = (y - center_y).abs() - reach_y;
        let ox = offset_x.max(0.0);
        let oy = offset_y.max(0.0);
        let outside = if ox > 0.0 && oy > 0.0 {
            ox.hypot(oy)
        } else {
            ox + oy
        };
        outside + offset_x.max(offset_y).min(0.0) - self.radius
    }

    #[must_use]
    pub fn normal(&self, x: f32, y: f32) -> (f32, f32) {
        let gradient_x = self.distance(x + 1.0, y) - self.distance(x - 1.0, y);
        let gradient_y = self.distance(x, y + 1.0) - self.distance(x, y - 1.0);
        let length = gradient_x.hypot(gradient_y);
        if length < 1e-4 {
            (0.0, 0.0)
        } else {
            (gradient_x / length, gradient_y / length)
        }
    }

    #[must_use]
    pub fn coverage(&self, x: f32, y: f32) -> f32 {
        (0.5 - self.distance(x, y)).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn pixel_bounds(&self) -> Rect {
        let left = self.left.floor() as i32 - 1;
        let top = self.top.floor() as i32 - 1;
        let right = self.right.ceil() as i32 + 1;
        let bottom = self.bottom.ceil() as i32 + 1;
        Rect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlassBackdrop<'a> {
    pub sharp: &'a RgbBuffer,
    pub soft: &'a RgbBuffer,
    pub soft_factor: usize,
    pub origin: (f32, f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlassBitmap {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[must_use]
pub fn soften_backdrop(
    capture: &RgbBuffer,
    params: &GlassParams,
    factor: usize,
    blur_radius: usize,
) -> RgbBuffer {
    capture
        .linearized()
        .downsample(factor)
        .box_blurred(blur_radius)
        .gamma_encoded()
        .graded(params)
}

#[must_use]
pub fn light_direction_for_pointer(shape: &GlassShape, pointer: Option<(f32, f32)>) -> (f32, f32) {
    let Some((px, py)) = pointer else {
        return LIGHT_DIRECTION;
    };
    let cx = (shape.left + shape.right) * 0.5;
    let cy = (shape.top + shape.bottom) * 0.5;
    let dx = px - cx;
    let dy = py - cy;
    let dist = dx.hypot(dy);
    if dist < 1.0 {
        return LIGHT_DIRECTION;
    }
    let bx = LIGHT_DIRECTION.0 * 0.60 + (dx / dist) * 0.40;
    let by = LIGHT_DIRECTION.1 * 0.60 + (dy / dist) * 0.40;
    let len = bx.hypot(by);
    if len < 1e-4 {
        LIGHT_DIRECTION
    } else {
        (bx / len, by / len)
    }
}

#[must_use]
pub fn compose_glass(
    params: &GlassParams,
    shape: &GlassShape,
    backdrop: &GlassBackdrop<'_>,
) -> GlassBitmap {
    compose_glass_lit(params, shape, backdrop, LIGHT_DIRECTION)
}

#[must_use]
pub fn compose_glass_lit(
    params: &GlassParams,
    shape: &GlassShape,
    backdrop: &GlassBackdrop<'_>,
    light_dir: (f32, f32),
) -> GlassBitmap {
    let bounds = shape.pixel_bounds();
    let width = bounds.width.max(1) as u32;
    let height = bounds.height.max(1) as u32;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for row in 0..height {
        for column in 0..width {
            let x = (bounds.x + column as i32) as f32 + 0.5;
            let y = (bounds.y + row as i32) as f32 + 0.5;
            let coverage = shape.coverage(x, y);
            if coverage <= 0.0 {
                continue;
            }
            let color = glass_color_lit(params, shape, backdrop, x, y, light_dir);
            let at = (row as usize * width as usize + column as usize) * 4;
            pixels[at] = to_byte(color[2] * coverage);
            pixels[at + 1] = to_byte(color[1] * coverage);
            pixels[at + 2] = to_byte(color[0] * coverage);
            pixels[at + 3] = to_byte(coverage);
        }
    }
    GlassBitmap {
        x: bounds.x,
        y: bounds.y,
        width,
        height,
        pixels,
    }
}

#[must_use]
pub fn glass_color(
    params: &GlassParams,
    shape: &GlassShape,
    backdrop: &GlassBackdrop<'_>,
    x: f32,
    y: f32,
) -> Rgb {
    glass_color_lit(params, shape, backdrop, x, y, LIGHT_DIRECTION)
}

#[must_use]
pub fn glass_color_lit(
    params: &GlassParams,
    shape: &GlassShape,
    backdrop: &GlassBackdrop<'_>,
    x: f32,
    y: f32,
    light_dir: (f32, f32),
) -> Rgb {
    const IOR_INV: f32 = 1.0 / 1.50;
    const BEZEL_SLOPE_GAIN: f32 = 1.25;
    const BEZEL_SHORT_RATIO: f32 = 0.44;

    let distance = shape.distance(x, y);
    let depth = (-distance).max(0.0);
    let factor = backdrop.soft_factor.max(1) as f32;
    let local_x = x - backdrop.origin.0;
    let local_y = y - backdrop.origin.1;

    let half_short = 0.5
        * (shape.right - shape.left)
            .min(shape.bottom - shape.top)
            .max(2.0);
    let bezel = if params.lens_mix <= 0.0 || params.lens_band <= 0.0 {
        0.0
    } else {
        params
            .lens_band
            .min(half_short * BEZEL_SHORT_RATIO)
            .max(1.0)
    };
    let u = if bezel > 0.0 {
        (1.0 - depth / bezel).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let need_normal =
        u > 0.0 || depth < params.rim_width + params.shadow_depth.max(params.inner_glow_depth);
    let (normal_x, normal_y) = if need_normal {
        shape.normal(x, y)
    } else {
        (0.0, 0.0)
    };

    let (sample_x, sample_y, nz3, meniscus) = if u > 0.0 {
        let u4 = u * u * u * u;
        let height = (1.0 - u4).max(0.002).powf(0.25);
        let slope = ((u * u * u) / (height * height * height)).min(3.2);
        let gx = normal_x * slope * BEZEL_SLOPE_GAIN;
        let gy = normal_y * slope * BEZEL_SLOPE_GAIN;
        let inv_len = 1.0 / (gx * gx + gy * gy + 1.0).sqrt();
        let nx3 = gx * inv_len;
        let ny3 = gy * inv_len;
        let nz3 = inv_len;
        let radicand = (1.0 - IOR_INV * IOR_INV * (1.0 - nz3 * nz3)).max(0.0);
        let mu = IOR_INV * nz3 - radicand.sqrt();
        let tx = mu * nx3;
        let ty = mu * ny3;
        let tz = (-IOR_INV + mu * nz3).abs().max(0.25);
        let refract_mag = params.lens_shift * 1.65 * (0.35 + 0.65 * u);
        (
            local_x + (tx / tz) * refract_mag,
            local_y + (ty / tz) * refract_mag,
            nz3,
            smoothstep(u) * u,
        )
    } else {
        (local_x, local_y, 1.0, 0.0)
    };

    let mut color = backdrop.soft.sample(sample_x / factor, sample_y / factor);

    if meniscus > 0.0 {
        let disp = params.dispersion * u * u;
        let refracted = if disp > 1e-3 {
            [
                backdrop
                    .sharp
                    .sample(sample_x - normal_x * disp, sample_y - normal_y * disp)[0],
                backdrop.sharp.sample(sample_x, sample_y)[1],
                backdrop
                    .sharp
                    .sample(sample_x + normal_x * disp, sample_y + normal_y * disp)[2],
            ]
        } else {
            backdrop.sharp.sample(sample_x, sample_y)
        };
        let graded = params.grade(refracted, params.lens_tint_scale);
        color = mix(color, graded, params.lens_mix * 0.55 * meniscus);
    }

    let span = (shape.bottom - shape.top).max(1.0);
    let upper = (1.0 - (y - shape.top) / span).clamp(0.0, 1.0);
    color = mix(color, WHITE, params.sheen_alpha * upper * upper);

    let lit = normal_x * light_dir.0 + normal_y * light_dir.1;
    let fresnel = if u > 0.0 {
        (0.04 + 0.96 * (1.0 - nz3).clamp(0.0, 1.0).powf(2.2)) * smoothstep(u)
    } else {
        0.0
    };
    let inner_band = smoothstep((1.0 - depth / params.inner_glow_depth.max(1e-3)).clamp(0.0, 1.0));
    let inner_sheen =
        inner_band.max(fresnel * 0.85) * params.inner_glow * (0.28 + 0.72 * lit.max(0.0));
    color = mix(color, WHITE, inner_sheen.clamp(0.0, 1.0));

    let shadow_reach = ((depth - params.rim_width) / params.shadow_depth.max(1e-3)).clamp(0.0, 1.0);
    let shadow = (1.0 - shadow_reach).powi(2) * (-lit).max(0.0) * params.shadow_strength;
    color = scale_rgb(color, 1.0 - shadow);

    let rim_band = smoothstep((1.0 - depth / params.rim_width.max(1e-3)).clamp(0.0, 1.0));
    let primary = lit.max(0.0).powi(2);
    let bounce = (-lit).max(0.0).powi(2);
    let rim = rim_band * (params.rim_base + params.rim_lit * primary + params.rim_bounce * bounce);
    color = mix(color, WHITE, rim.clamp(0.0, 1.0));

    let outline = smoothstep((OUTLINE_REACH - depth).clamp(0.0, 1.0)) * params.outline_alpha;
    mix(color, BLACK, outline)
}

fn smoothstep(t: f32) -> f32 {
    let clamped = t.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}
fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn luminance(rgb: Rgb) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    [
        from[0] + (to[0] - from[0]) * amount,
        from[1] + (to[1] - from[1]) * amount,
        from[2] + (to[2] - from[2]) * amount,
    ]
}

fn scale_rgb(rgb: Rgb, factor: f32) -> Rgb {
    [rgb[0] * factor, rgb[1] * factor, rgb[2] * factor]
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::{
        GlassBackdrop, GlassParams, GlassShape, RgbBuffer, compose_glass, glass_color,
        light_direction_for_pointer, mix, soften_backdrop,
    };
    use crate::layout::Rect;
    use crate::material::Appearance;
    use crate::pixels::PixelBufferError;
    use crate::theme::{ResolvedTheme, Rgba};

    const THEMES: [ResolvedTheme; 2] = [ResolvedTheme::Dark, ResolvedTheme::Light];

    fn rgb_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
        left.iter()
            .zip(right)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max)
    }

    fn opaque(rgb: [f32; 3]) -> Rgba {
        Rgba::new(rgb[0], rgb[1], rgb[2], 1.0)
    }

    fn pill() -> GlassShape {
        GlassShape::from_rect(
            Rect {
                x: 40,
                y: 30,
                width: 240,
                height: 60,
            },
            30.0,
        )
    }

    fn stripes(width: usize, height: usize) -> RgbBuffer {
        let mut buffer = RgbBuffer::filled(width, height, [0.0; 3]);
        for y in 0..height {
            for x in 0..width {
                let value = if (x / 6) % 2 == 0 { 1.0 } else { 0.0 };
                buffer.set_pixel(x, y, [value, 1.0 - value, 0.5]);
            }
        }
        buffer
    }

    fn backdrop_for<'a>(
        sharp: &'a RgbBuffer,
        soft: &'a RgbBuffer,
        factor: usize,
    ) -> GlassBackdrop<'a> {
        GlassBackdrop {
            sharp,
            soft,
            soft_factor: factor,
            origin: (0.0, 0.0),
        }
    }

    #[test]
    fn bgra_import_reorders_channels_and_rejects_bad_buffers() {
        let pixels = [10u8, 20, 30, 255, 40, 50, 60, 255];
        let buffer = RgbBuffer::from_bgra(2, 1, &pixels).unwrap();
        assert!(
            rgb_distance(
                buffer.pixel(0, 0),
                [30.0 / 255.0, 20.0 / 255.0, 10.0 / 255.0]
            ) < 1e-6
        );
        assert_eq!(
            RgbBuffer::from_bgra(0, 1, &pixels),
            Err(PixelBufferError::EmptyDimension)
        );
        assert_eq!(
            RgbBuffer::from_bgra(2, 2, &pixels),
            Err(PixelBufferError::BufferTooShort)
        );
    }

    #[test]
    fn downsample_averages_blocks_including_partial_edges() {
        let mut buffer = RgbBuffer::filled(5, 2, [0.0; 3]);
        for x in 0..5 {
            buffer.set_pixel(x, 0, [1.0; 3]);
        }
        let small = buffer.downsample(2);
        assert_eq!((small.width(), small.height()), (3, 1));
        assert!((small.pixel(0, 0)[0] - 0.5).abs() < 1e-6);
        assert!((small.pixel(2, 0)[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn blur_keeps_flat_colors_and_smooths_edges() {
        let flat = RgbBuffer::filled(16, 16, [0.3, 0.5, 0.7]);
        let blurred = flat.box_blurred(3);
        for channel in 0..3 {
            assert!((blurred.pixel(8, 8)[channel] - flat.pixel(8, 8)[channel]).abs() < 1e-5);
        }
        let mut stepped = RgbBuffer::filled(32, 4, [0.0; 3]);
        for y in 0..4 {
            for x in 16..32 {
                stepped.set_pixel(x, y, [1.0; 3]);
            }
        }
        let smooth = stepped.box_blurred(3);
        let across = smooth.pixel(15, 1)[0];
        assert!(across > 0.05 && across < 0.95);
        assert!(smooth.pixel(0, 1)[0] < 0.01);
        assert!(smooth.pixel(31, 1)[0] > 0.99);
    }

    #[test]
    fn zero_radius_blur_is_the_identity() {
        let buffer = stripes(12, 4);
        assert_eq!(buffer.box_blurred(0), buffer);
    }
    #[test]
    fn linear_roundtrip_preserves_flat_colors() {
        for color in [[0.0; 3], [1.0; 3], [0.2, 0.5, 0.9]] {
            let buffer = RgbBuffer::filled(4, 4, color);
            let roundtrip = buffer.linearized().gamma_encoded();
            for channel in 0..3 {
                assert!(
                    (roundtrip.pixel(0, 0)[channel] - buffer.pixel(0, 0)[channel]).abs() < 1e-6
                );
            }
        }
    }

    #[test]
    fn linear_space_blur_keeps_midtones_brighter() {
        let mut stepped = RgbBuffer::filled(64, 1, [0.0; 3]);
        for x in 32..64 {
            stepped.set_pixel(x, 0, [1.0; 3]);
        }
        let srgb_blur = stepped.box_blurred(8);
        let linear_blur = stepped.linearized().box_blurred(8).gamma_encoded();
        let srgb_at = srgb_blur.pixel(31, 0)[0];
        let linear_at = linear_blur.pixel(31, 0)[0];
        assert!(srgb_at > 0.45 && srgb_at < 0.50);
        assert!(linear_at > srgb_at + 0.1);
    }

    #[test]
    fn sample_interpolates_between_pixel_centers() {
        let mut buffer = RgbBuffer::filled(2, 1, [0.0; 3]);
        buffer.set_pixel(1, 0, [1.0; 3]);
        assert!((buffer.sample(1.0, 0.5)[0] - 0.5).abs() < 1e-6);
        assert!((buffer.sample(0.5, 0.5)[0]).abs() < 1e-6);
        assert!((buffer.sample(99.0, 99.0)[0] - 1.0).abs() < 1e-6);
        assert!((buffer.sample(0.75, 0.5)[0] - 0.156_25).abs() < 1e-5);
        assert!((buffer.sample(1.25, 0.5)[0] - 0.843_75).abs() < 1e-5);
    }

    #[test]
    fn lens_refracts_the_bevel_and_tapers_smoothly_toward_interior() {
        let sharp = stripes(320, 120);
        let soft = soften_backdrop(&sharp, &GlassParams::DARK, 2, 9);
        let backdrop = backdrop_for(&sharp, &soft, 2);
        let shape = pill();
        let flat = GlassParams::DARK.without_lens();
        let outer_bevel = (160.0, 32.0);
        let inner_bevel = (160.0, 44.0);
        let outer_diff = rgb_distance(
            glass_color(
                &GlassParams::DARK,
                &shape,
                &backdrop,
                outer_bevel.0,
                outer_bevel.1,
            ),
            glass_color(&flat, &shape, &backdrop, outer_bevel.0, outer_bevel.1),
        );
        let inner_diff = rgb_distance(
            glass_color(
                &GlassParams::DARK,
                &shape,
                &backdrop,
                inner_bevel.0,
                inner_bevel.1,
            ),
            glass_color(&flat, &shape, &backdrop, inner_bevel.0, inner_bevel.1),
        );
        assert!(outer_diff > inner_diff);
        assert!(outer_diff > 0.05);
    }

    #[test]
    fn bevel_disperses_rgb_channels_across_a_white_edge() {
        let mut sharp = RgbBuffer::filled(320, 120, [0.0; 3]);
        for y in 0..45 {
            for x in 0..320 {
                sharp.set_pixel(x, y, [1.0; 3]);
            }
        }
        let soft = soften_backdrop(&sharp, &GlassParams::DARK, 2, 5);
        let backdrop = backdrop_for(&sharp, &soft, 2);
        let shape = pill();
        let no_disp = GlassParams {
            dispersion: 0.0,
            ..GlassParams::DARK
        };
        let mut strongest = ([0.0; 3], 0.0f32);
        for y in 30..=52 {
            let with_disp = glass_color(&GlassParams::DARK, &shape, &backdrop, 160.0, y as f32);
            let without_disp = glass_color(&no_disp, &shape, &backdrop, 160.0, y as f32);
            let distance = rgb_distance(with_disp, without_disp);
            if distance > strongest.1 {
                strongest = (with_disp, distance);
            }
        }
        assert!(strongest.1 > 0.01);
        let with_disp = strongest.0;
        let chroma_spread = (with_disp[0] - with_disp[2])
            .abs()
            .max((with_disp[1] - with_disp[2]).abs());
        assert!(chroma_spread > 0.01);
    }

    #[test]
    fn grading_pulls_toward_the_theme_tint_and_boosts_saturation() {
        let dark = GlassParams::DARK;
        let neutral = dark.grade([0.5; 3], 1.0);
        assert!(neutral.iter().all(|c| *c < 0.5));
        let red = dark.grade([0.8, 0.2, 0.2], 1.0);
        assert!(red[0] > red[1] && red[0] - red[1] > 0.25);
        let light = GlassParams::LIGHT.grade([0.0; 3], 1.0);
        assert!(light.iter().all(|c| *c > 0.5));
        let untinted = dark.grade([0.5; 3], 0.0);
        assert!(untinted[0] > neutral[0]);
    }

    #[test]
    fn shape_distance_is_negative_inside_and_normals_point_outward() {
        let shape = pill();
        assert!(shape.distance(160.0, 60.0) < -29.0);
        assert!(shape.distance(10.0, 60.0) > 29.0);
        assert!(shape.distance(40.0, 60.0).abs() < 1e-4);
        let (nx, ny) = shape.normal(160.0, 30.5);
        assert!(nx.abs() < 1e-3 && ny < -0.99);
        let (nx, _) = shape.normal(279.5, 60.0);
        assert!(nx > 0.99);
        assert!((shape.coverage(160.0, 60.0) - 1.0).abs() < 1e-6);
        assert!(shape.coverage(5.0, 5.0).abs() < 1e-6);
    }

    #[test]
    fn rectangular_bars_use_a_zero_radius() {
        let bar = GlassShape::from_rect(
            Rect {
                x: 0,
                y: 0,
                width: 400,
                height: 28,
            },
            0.0,
        );
        assert!(bar.distance(0.0, 14.0).abs() < 1e-4);
        assert!(bar.distance(200.0, 14.0) < -13.9);
        let oversized = GlassShape::from_rect(
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 20,
            },
            90.0,
        );
        assert!((oversized.radius - 10.0).abs() < 1e-6);
    }

    #[test]
    fn composed_bitmap_is_premultiplied_and_clipped_to_the_shape() {
        let sharp = stripes(320, 120);
        let soft = soften_backdrop(&sharp, &GlassParams::DARK, 4, 3);
        let backdrop = backdrop_for(&sharp, &soft, 4);
        let bitmap = compose_glass(&GlassParams::DARK, &pill(), &backdrop);
        assert_eq!(
            bitmap.pixels.len(),
            bitmap.width as usize * bitmap.height as usize * 4
        );
        let at = |x: i32, y: i32| {
            let index =
                ((y - bitmap.y) as usize * bitmap.width as usize + (x - bitmap.x) as usize) * 4;
            &bitmap.pixels[index..index + 4]
        };
        assert_eq!(at(41, 31)[3], 0);
        assert_eq!(at(160, 60)[3], 255);
        for pixel in bitmap.pixels.chunks_exact(4) {
            assert!(pixel[0] <= pixel[3] && pixel[1] <= pixel[3] && pixel[2] <= pixel[3]);
        }
    }

    #[test]
    fn lens_bends_the_edge_toward_sharp_content_and_leaves_the_interior_soft() {
        let sharp = stripes(320, 120);
        let soft = soften_backdrop(&sharp, &GlassParams::DARK, 4, 3);
        let backdrop = backdrop_for(&sharp, &soft, 4);
        let shape = pill();
        let flat = GlassParams::DARK.without_lens();
        let interior = (160.0, 60.0);
        let lensed = glass_color(
            &GlassParams::DARK,
            &shape,
            &backdrop,
            interior.0,
            interior.1,
        );
        let unlensed = glass_color(&flat, &shape, &backdrop, interior.0, interior.1);
        assert!(rgb_distance(lensed, unlensed) < 1e-6);
        let near_edge = (160.0, 33.0);
        let bent = glass_color(
            &GlassParams::DARK,
            &shape,
            &backdrop,
            near_edge.0,
            near_edge.1,
        );
        let straight = glass_color(&flat, &shape, &backdrop, near_edge.0, near_edge.1);
        assert!(rgb_distance(bent, straight) > 1e-3);
    }

    #[test]
    fn rim_is_brighter_on_the_lit_side_than_the_shaded_side() {
        let sharp = RgbBuffer::filled(320, 120, [0.5; 3]);
        let soft = soften_backdrop(&sharp, &GlassParams::LIGHT, 4, 3);
        let backdrop = backdrop_for(&sharp, &soft, 4);
        let params = GlassParams::LIGHT.without_lens();
        let shape = GlassShape::from_rect(
            Rect {
                x: 40,
                y: 30,
                width: 240,
                height: 60,
            },
            0.0,
        );
        let lit_side = glass_color(&params, &shape, &backdrop, 41.0, 31.0);
        let shaded_side = glass_color(&params, &shape, &backdrop, 278.0, 88.0);
        let luma = |c: [f32; 3]| c[0] + c[1] + c[2];
        assert!(luma(lit_side) > luma(shaded_side));
    }

    #[test]
    fn dpi_scaling_only_changes_pixel_measures() {
        let base = GlassParams::DARK;
        let scaled = base.scaled(192);
        assert!((scaled.lens_band - base.lens_band * 2.0).abs() < 1e-5);
        assert!((scaled.rim_width - base.rim_width * 2.0).abs() < 1e-5);
        assert!(rgb_distance(scaled.tint, base.tint) < 1e-9);
        assert!((scaled.tint_amount - base.tint_amount).abs() < f32::EPSILON);
        assert_eq!(base.scaled(96), base);
    }

    #[test]
    fn pointer_shifts_specular_light_toward_hovered_side() {
        let shape = pill();
        let default_light = light_direction_for_pointer(&shape, None);
        let right_hover = light_direction_for_pointer(&shape, Some((270.0, 60.0)));
        let left_hover = light_direction_for_pointer(&shape, Some((50.0, 60.0)));
        assert!(right_hover.0 > default_light.0);
        assert!(left_hover.0 < default_light.0);
    }

    #[test]
    fn primary_text_meets_wcag_aa_over_the_brightest_and_darkest_backdrops() {
        for theme in THEMES {
            let params = GlassParams::for_theme(theme);
            let colors = Appearance::glass(theme).colors();
            let (lightest, darkest) = params.interior_extremes();
            let worst = match theme {
                ResolvedTheme::Dark => lightest,
                ResolvedTheme::Light => darkest,
            };
            let ratio = colors.text_primary.contrast_ratio(opaque(worst));
            assert!(ratio >= 4.5, "{theme:?} primary text {ratio:.2}");
        }
    }

    #[test]
    fn muted_text_meets_aa_large_over_midtone_backdrops() {
        for theme in THEMES {
            let params = GlassParams::for_theme(theme);
            let colors = Appearance::glass(theme).colors();
            for level in [0.25f32, 0.5, 0.75] {
                let graded = params.grade([level; 3], 1.0);
                let surface = mix(graded, [1.0; 3], params.sheen_alpha);
                let ratio = colors.text_muted.contrast_ratio(opaque(surface));
                assert!(ratio >= 3.0, "{theme:?} muted over {level}: {ratio:.2}");
            }
        }
    }
}
