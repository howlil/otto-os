use super::params::BLUR_PASSES;
use crate::pixels::{PixelBufferError, PixelLimits, validate_bgra_buffer};

use super::color::BYTE_TO_F32_LUT;
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
        let pixel_count = width as usize * height as usize;
        let mut data = Vec::with_capacity(pixel_count * 3);
        for bgra in pixels[..layout.byte_len].chunks_exact(4) {
            data.push(BYTE_TO_F32_LUT[bgra[2] as usize]);
            data.push(BYTE_TO_F32_LUT[bgra[1] as usize]);
            data.push(BYTE_TO_F32_LUT[bgra[0] as usize]);
        }
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
        let pixel_count = width * height;
        let mut data = Vec::with_capacity(pixel_count * 3);
        for _ in 0..pixel_count {
            data.extend_from_slice(&color);
        }
        Self {
            width,
            height,
            data,
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
            let dy = (y / factor) * width;
            let src_row = y * self.width * 3;
            for x in 0..self.width {
                let cell = dy + x / factor;
                let source = src_row + x * 3;
                let dst = cell * 3;
                sums[dst] += self.data[source];
                sums[dst + 1] += self.data[source + 1];
                sums[dst + 2] += self.data[source + 2];
                counts[cell] += 1.0;
            }
        }
        for (cell, &count) in counts.iter().enumerate() {
            let inv = 1.0 / count;
            let dst = cell * 3;
            sums[dst] *= inv;
            sums[dst + 1] *= inv;
            sums[dst + 2] *= inv;
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
        let row_stride = self.width * 3;
        for dy in 0..height {
            let y_start = dy * factor;
            let y_end = (y_start + factor).min(self.height);
            let block_rows = y_end - y_start;
            for dx in 0..width {
                let x_start = dx * factor;
                let x_end = (x_start + factor).min(self.width);
                let inv_count = 1.0 / (block_rows * (x_end - x_start)) as f32;
                let mut sum_r = 0.0f32;
                let mut sum_g = 0.0f32;
                let mut sum_b = 0.0f32;
                let x_start_3 = x_start * 3;
                let x_end_3 = x_end * 3;
                for y in y_start..y_end {
                    let row_start = y * row_stride;
                    for rgb in self.data[row_start + x_start_3..row_start + x_end_3].chunks_exact(3)
                    {
                        sum_r += srgb_to_linear_fast(rgb[0]);
                        sum_g += srgb_to_linear_fast(rgb[1]);
                        sum_b += srgb_to_linear_fast(rgb[2]);
                    }
                }
                data.push(sum_r * inv_count);
                data.push(sum_g * inv_count);
                data.push(sum_b * inv_count);
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
        if radius == 0 || self.width == 0 || self.height == 0 {
            return;
        }
        let mut scratch = vec![0.0f32; self.data.len()];
        let mut row_sums = vec![0.0f32; self.width * 3];
        let (width, height) = (self.width, self.height);
        for _ in 0..BLUR_PASSES {
            blur_horizontal(&self.data, &mut scratch, width, height, radius);
            blur_vertical(
                &scratch,
                &mut self.data,
                &mut row_sums,
                width,
                height,
                radius,
            );
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

    pub fn write_rgba_u8(&self, out: &mut Vec<u8>) {
        out.clear();
        let needed = self.width * self.height * 4;
        if out.capacity() < needed {
            out.reserve(needed - out.capacity());
        }
        for rgb in self.data.chunks_exact(3) {
            out.push(to_byte(rgb[0]));
            out.push(to_byte(rgb[1]));
            out.push(to_byte(rgb[2]));
            out.push(255);
        }
    }

    #[must_use]
    pub fn to_rgba_u8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        self.write_rgba_u8(&mut out);
        out
    }
}

fn blur_horizontal(source: &[f32], target: &mut [f32], width: usize, height: usize, radius: usize) {
    let row_len = width * 3;
    let inv_window = 1.0 / (2 * radius + 1) as f32;
    let max_x = width - 1;
    let max_x_isize = max_x as isize;
    let reach = radius as isize;
    let has_interior = width > 2 * radius + 1;

    for y in 0..height {
        let row_start = y * row_len;
        let src = &source[row_start..row_start + row_len];
        let dst = &mut target[row_start..row_start + row_len];

        let mut sr = 0.0f32;
        let mut sg = 0.0f32;
        let mut sb = 0.0f32;
        for offset in -reach..=reach {
            let idx = offset.clamp(0, max_x_isize) as usize * 3;
            sr += src[idx];
            sg += src[idx + 1];
            sb += src[idx + 2];
        }

        if has_interior {
            let left_first = [src[0], src[1], src[2]];
            for x in 0..radius {
                let d = x * 3;
                dst[d] = sr * inv_window;
                dst[d + 1] = sg * inv_window;
                dst[d + 2] = sb * inv_window;
                let add = (x + radius + 1) * 3;
                sr += src[add] - left_first[0];
                sg += src[add + 1] - left_first[1];
                sb += src[add + 2] - left_first[2];
            }
            let interior_end = width - radius - 1;
            for x in radius..interior_end {
                let d = x * 3;
                dst[d] = sr * inv_window;
                dst[d + 1] = sg * inv_window;
                dst[d + 2] = sb * inv_window;
                let add = (x + radius + 1) * 3;
                let sub = (x - radius) * 3;
                sr += src[add] - src[sub];
                sg += src[add + 1] - src[sub + 1];
                sb += src[add + 2] - src[sub + 2];
            }
            let right_last_i = max_x * 3;
            let right_last = [
                src[right_last_i],
                src[right_last_i + 1],
                src[right_last_i + 2],
            ];
            for x in interior_end..width {
                let d = x * 3;
                dst[d] = sr * inv_window;
                dst[d + 1] = sg * inv_window;
                dst[d + 2] = sb * inv_window;
                let sub = (x - radius) * 3;
                sr += right_last[0] - src[sub];
                sg += right_last[1] - src[sub + 1];
                sb += right_last[2] - src[sub + 2];
            }
        } else {
            for x in 0..width {
                let d = x * 3;
                dst[d] = sr * inv_window;
                dst[d + 1] = sg * inv_window;
                dst[d + 2] = sb * inv_window;
                let next = x as isize;
                let add = (next + reach + 1).clamp(0, max_x_isize) as usize * 3;
                let sub = (next - reach).clamp(0, max_x_isize) as usize * 3;
                sr += src[add] - src[sub];
                sg += src[add + 1] - src[sub + 1];
                sb += src[add + 2] - src[sub + 2];
            }
        }
    }
}

fn blur_vertical(
    source: &[f32],
    target: &mut [f32],
    row_sums: &mut [f32],
    width: usize,
    height: usize,
    radius: usize,
) {
    let row_len = width * 3;
    let inv_window = 1.0 / (2 * radius + 1) as f32;
    let max_y = height - 1;
    let max_y_isize = max_y as isize;
    let reach = radius as isize;

    row_sums.fill(0.0);
    for offset in -reach..=reach {
        let row = offset.clamp(0, max_y_isize) as usize * row_len;
        let src_row = &source[row..row + row_len];
        for (acc, &val) in row_sums.iter_mut().zip(src_row) {
            *acc += val;
        }
    }

    for y in 0..height {
        let dst_start = y * row_len;
        let dst_row = &mut target[dst_start..dst_start + row_len];
        for (out, &acc) in dst_row.iter_mut().zip(row_sums.iter()) {
            *out = acc * inv_window;
        }
        let add_y = (y + radius + 1).min(max_y) * row_len;
        let sub_y = y.saturating_sub(radius) * row_len;
        let add_row = &source[add_y..add_y + row_len];
        let sub_row = &source[sub_y..sub_y + row_len];
        for ((acc, &add), &sub) in row_sums.iter_mut().zip(add_row).zip(sub_row) {
            *acc += add - sub;
        }
    }
}
