use super::params::BLUR_PASSES;
use crate::pixels::{PixelBufferError, PixelLimits, validate_bgra_buffer};

use super::color::linear_to_srgb;
use super::color::luminance;
use super::color::mix;
use super::color::srgb_to_linear;
use super::color::srgb_to_linear_fast;
use super::color::to_byte;
use super::compose::smoothstep;
use super::params::GlassParams;
use super::params::Rgb;

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
    pub fn downsample_linearized(&self, factor: usize) -> Self {
        let factor = factor.max(1);
        let width = self.width.div_ceil(factor);
        let height = self.height.div_ceil(factor);
        let mut data = Vec::with_capacity(width * height * 3);
        for dy in 0..height {
            let y_start = dy * factor;
            let y_end = (y_start + factor).min(self.height);
            let block_rows = y_end - y_start;
            for dx in 0..width {
                let x_start = dx * factor;
                let x_end = (x_start + factor).min(self.width);
                let count = (block_rows * (x_end - x_start)) as f32;
                let mut sum_r = 0.0f32;
                let mut sum_g = 0.0f32;
                let mut sum_b = 0.0f32;
                for y in y_start..y_end {
                    let row_start = y * self.width * 3;
                    for rgb in
                        self.data[row_start + x_start * 3..row_start + x_end * 3].chunks_exact(3)
                    {
                        sum_r += srgb_to_linear_fast(rgb[0]);
                        sum_g += srgb_to_linear_fast(rgb[1]);
                        sum_b += srgb_to_linear_fast(rgb[2]);
                    }
                }
                data.extend_from_slice(&[sum_r / count, sum_g / count, sum_b / count]);
            }
        }
        Self {
            width,
            height,
            data,
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

    pub fn box_blur_in_place(&mut self, radius: usize) {
        if radius == 0 {
            return;
        }
        let mut scratch = vec![0.0f32; self.data.len()];
        let (width, height) = (self.width, self.height);
        for _ in 0..BLUR_PASSES {
            blur_lines(&self.data, &mut scratch, width, height, radius, true);
            blur_lines(&scratch, &mut self.data, width, height, radius, false);
        }
    }

    #[must_use]
    pub fn box_blurred(&self, radius: usize) -> Self {
        let mut current = self.clone();
        current.box_blur_in_place(radius);
        current
    }

    pub fn gamma_encode_and_grade_in_place(&mut self, params: &GlassParams) {
        for texel in self.data.chunks_exact_mut(3) {
            let srgb = [
                linear_to_srgb(texel[0]),
                linear_to_srgb(texel[1]),
                linear_to_srgb(texel[2]),
            ];
            let graded = params.grade(srgb, 1.0);
            texel.copy_from_slice(&graded);
        }
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

    #[must_use]
    pub fn to_rgba_u8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for rgb in self.data.chunks_exact(3) {
            out.extend_from_slice(&[to_byte(rgb[0]), to_byte(rgb[1]), to_byte(rgb[2]), 255]);
        }
        out
    }
}

pub(super) fn blur_lines(
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
