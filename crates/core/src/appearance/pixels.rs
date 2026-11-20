const BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelLimits {
    pub max_dimension: u32,
    pub max_bytes: usize,
}

impl PixelLimits {
    pub const ICON: Self = Self {
        max_dimension: 1_024,
        max_bytes: 4 * 1024 * 1024,
    };

    pub const BACKDROP: Self = Self {
        max_dimension: 8_192,
        max_bytes: 16 * 1024 * 1024,
    };

    pub const WINDOW_SNAPSHOT: Self = Self {
        max_dimension: 8_192,
        max_bytes: 40 * 1024 * 1024,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelBufferError {
    EmptyDimension,
    DimensionTooLarge,
    ByteBudgetExceeded,
    BufferTooShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BgraLayout {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub byte_len: usize,
}

impl BgraLayout {
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.width != 0
            && self.height != 0
            && self.width.checked_mul(BYTES_PER_PIXEL as u32) == Some(self.stride)
            && (self.stride as usize).checked_mul(self.height as usize) == Some(self.byte_len)
    }
}

pub fn bgra_layout(
    width: u32,
    height: u32,
    limits: PixelLimits,
) -> Result<BgraLayout, PixelBufferError> {
    if width == 0 || height == 0 {
        return Err(PixelBufferError::EmptyDimension);
    }
    if width > limits.max_dimension || height > limits.max_dimension {
        return Err(PixelBufferError::DimensionTooLarge);
    }
    let stride = width
        .checked_mul(BYTES_PER_PIXEL as u32)
        .ok_or(PixelBufferError::DimensionTooLarge)?;
    let byte_len = (stride as usize)
        .checked_mul(height as usize)
        .ok_or(PixelBufferError::ByteBudgetExceeded)?;
    if byte_len > limits.max_bytes {
        return Err(PixelBufferError::ByteBudgetExceeded);
    }
    Ok(BgraLayout {
        width,
        height,
        stride,
        byte_len,
    })
}

pub fn validate_bgra_buffer(
    width: u32,
    height: u32,
    buffer_len: usize,
    limits: PixelLimits,
) -> Result<BgraLayout, PixelBufferError> {
    let layout = bgra_layout(width, height, limits)?;
    if buffer_len < layout.byte_len {
        return Err(PixelBufferError::BufferTooShort);
    }
    Ok(layout)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[must_use]
pub fn crop_bgra(source: &[u8], source_layout: BgraLayout, region: CropRegion) -> Option<Vec<u8>> {
    if !source_layout.is_consistent()
        || source.len() < source_layout.byte_len
        || region.width == 0
        || region.height == 0
    {
        return None;
    }
    let right = region.x.checked_add(region.width)?;
    let bottom = region.y.checked_add(region.height)?;
    if right > source_layout.width || bottom > source_layout.height {
        return None;
    }
    let row_bytes = region.width as usize * BYTES_PER_PIXEL;
    let mut cropped = Vec::with_capacity(row_bytes * region.height as usize);
    for row in region.y..bottom {
        let start =
            row as usize * source_layout.stride as usize + region.x as usize * BYTES_PER_PIXEL;
        cropped.extend_from_slice(&source[start..start + row_bytes]);
    }
    Some(cropped)
}

#[must_use]
pub fn scale_dimension(length: u32, from_dpi: u32, to_dpi: u32) -> u32 {
    if from_dpi == 0 || to_dpi == 0 {
        return length;
    }
    let scaled =
        (u64::from(length) * u64::from(from_dpi) + u64::from(to_dpi) / 2) / u64::from(to_dpi);
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

#[must_use]
pub fn resize_bgra(
    source: &[u8],
    source_layout: BgraLayout,
    target_layout: BgraLayout,
) -> Option<Vec<u8>> {
    if !source_layout.is_consistent()
        || !target_layout.is_consistent()
        || source.len() < source_layout.byte_len
    {
        return None;
    }
    if source_layout.width == target_layout.width && source_layout.height == target_layout.height {
        return Some(source[..source_layout.byte_len].to_vec());
    }
    let mut target = vec![0u8; target_layout.byte_len];
    let x_ratio = f64::from(source_layout.width) / f64::from(target_layout.width);
    let y_ratio = f64::from(source_layout.height) / f64::from(target_layout.height);
    let max_x = source_layout.width - 1;
    let max_y = source_layout.height - 1;
    let source_stride = source_layout.stride as usize;
    let target_stride = target_layout.stride as usize;
    let at = |x: u32, y: u32| y as usize * source_stride + x as usize * BYTES_PER_PIXEL;
    for row in 0..target_layout.height {
        let sy = ((f64::from(row) + 0.5) * y_ratio - 0.5).max(0.0);
        let y0 = (sy.floor() as u32).min(max_y);
        let y1 = (y0 + 1).min(max_y);
        let fy = sy - f64::from(y0);
        for column in 0..target_layout.width {
            let sx = ((f64::from(column) + 0.5) * x_ratio - 0.5).max(0.0);
            let x0 = (sx.floor() as u32).min(max_x);
            let x1 = (x0 + 1).min(max_x);
            let fx = sx - f64::from(x0);
            let out = row as usize * target_stride + column as usize * BYTES_PER_PIXEL;
            for channel in 0..BYTES_PER_PIXEL {
                let top = f64::from(source[at(x0, y0) + channel]) * (1.0 - fx)
                    + f64::from(source[at(x1, y0) + channel]) * fx;
                let bottom = f64::from(source[at(x0, y1) + channel]) * (1.0 - fx)
                    + f64::from(source[at(x1, y1) + channel]) * fx;
                target[out + channel] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
        }
    }
    Some(target)
}

#[must_use]
pub fn downsample_bgra_half(source: &[u8], width: u32, height: u32) -> Option<(u32, u32, Vec<u8>)> {
    let layout = bgra_layout(width, height, PixelLimits::WINDOW_SNAPSHOT).ok()?;
    if source.len() < layout.byte_len {
        return None;
    }
    if width <= 2 && height <= 2 {
        return Some((width, height, source[..layout.byte_len].to_vec()));
    }
    let out_width = (width / 2).max(1);
    let out_height = (height / 2).max(1);
    let src_stride = layout.stride as usize;
    let out_stride = out_width as usize * BYTES_PER_PIXEL;
    let mut output = vec![0u8; out_stride * out_height as usize];
    for row in 0..out_height {
        let sy0 = (row * 2) as usize;
        let sy1 = (sy0 + 1).min(height as usize - 1);
        let row0 = sy0 * src_stride;
        let row1 = sy1 * src_stride;
        let dst_row = row as usize * out_stride;
        for col in 0..out_width {
            let sx0 = (col * 2) as usize * BYTES_PER_PIXEL;
            let sx1 = (col * 2 + 1).min(width - 1) as usize * BYTES_PER_PIXEL;
            let dst = dst_row + col as usize * BYTES_PER_PIXEL;
            for ch in 0..BYTES_PER_PIXEL {
                let sum = u32::from(source[row0 + sx0 + ch])
                    + u32::from(source[row0 + sx1 + ch])
                    + u32::from(source[row1 + sx0 + ch])
                    + u32::from(source[row1 + sx1 + ch]);
                output[dst + ch] = ((sum + 2) >> 2) as u8;
            }
        }
    }
    Some((out_width, out_height, output))
}

#[must_use]
pub fn downsample_bgra_to_fit(
    source: &[u8],
    width: u32,
    height: u32,
    max_dimension: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    let layout = bgra_layout(width, height, PixelLimits::WINDOW_SNAPSHOT).ok()?;
    if source.len() < layout.byte_len || max_dimension == 0 {
        return None;
    }
    if width.max(height) <= max_dimension {
        return Some((width, height, source[..layout.byte_len].to_vec()));
    }
    let (mut cur_w, mut cur_h, mut cur_buf) = downsample_bgra_half(source, width, height)?;
    while cur_w.max(cur_h) > max_dimension && (cur_w > 2 || cur_h > 2) {
        let (next_w, next_h, next_buf) = downsample_bgra_half(&cur_buf, cur_w, cur_h)?;
        cur_w = next_w;
        cur_h = next_h;
        cur_buf = next_buf;
    }
    Some((cur_w, cur_h, cur_buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANY: PixelLimits = PixelLimits {
        max_dimension: 64,
        max_bytes: 64 * 64 * 4,
    };

    #[test]
    fn dimensions_scale_between_dpi_and_never_reach_zero() {
        assert_eq!(scale_dimension(1080, 96, 144), 720);
        assert_eq!(scale_dimension(720, 144, 96), 1080);
        assert_eq!(scale_dimension(500, 144, 144), 500);
        assert_eq!(scale_dimension(1, 96, 192), 1);
        assert_eq!(scale_dimension(77, 0, 144), 77);
    }

    #[test]
    fn resize_keeps_identical_sizes_untouched() {
        let layout = bgra_layout(2, 2, ANY).unwrap();
        let pixels: Vec<u8> = (0..16).collect();
        assert_eq!(resize_bgra(&pixels, layout, layout), Some(pixels));
    }

    #[test]
    fn resize_up_preserves_solid_colour_and_fills_every_pixel() {
        let small = bgra_layout(2, 2, ANY).unwrap();
        let large = bgra_layout(3, 3, ANY).unwrap();
        let white = vec![255u8; small.byte_len];
        let scaled = resize_bgra(&white, small, large).unwrap();
        assert_eq!(scaled.len(), large.byte_len);
        assert!(scaled.iter().all(|value| *value == 255));
    }

    #[test]
    fn resize_interpolates_between_neighbours() {
        let source = bgra_layout(2, 1, ANY).unwrap();
        let target = bgra_layout(4, 1, ANY).unwrap();
        let pixels = [0, 0, 0, 255, 200, 200, 200, 255];
        let scaled = resize_bgra(&pixels, source, target).unwrap();
        let blues: Vec<u8> = scaled.chunks_exact(4).map(|px| px[0]).collect();
        assert_eq!(blues.first(), Some(&0));
        assert_eq!(blues.last(), Some(&200));
        assert!(blues.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn resize_rejects_a_short_source_buffer() {
        let source = bgra_layout(2, 2, ANY).unwrap();
        let target = bgra_layout(4, 4, ANY).unwrap();
        assert!(resize_bgra(&[0; 4], source, target).is_none());
    }

    const SMALL: PixelLimits = PixelLimits {
        max_dimension: 16,
        max_bytes: 256,
    };

    #[test]
    fn accepts_a_buffer_that_covers_every_row() {
        let layout = validate_bgra_buffer(4, 2, 32, SMALL).unwrap();
        assert_eq!(layout.stride, 16);
        assert_eq!(layout.byte_len, 32);
    }

    #[test]
    fn rejects_empty_dimensions_before_any_native_call() {
        assert_eq!(
            validate_bgra_buffer(0, 4, 64, SMALL),
            Err(PixelBufferError::EmptyDimension)
        );
        assert_eq!(
            validate_bgra_buffer(4, 0, 64, SMALL),
            Err(PixelBufferError::EmptyDimension)
        );
    }

    #[test]
    fn rejects_a_buffer_shorter_than_its_declared_size() {
        assert_eq!(
            validate_bgra_buffer(4, 2, 31, SMALL),
            Err(PixelBufferError::BufferTooShort)
        );
    }

    #[test]
    fn rejects_dimensions_and_byte_totals_over_budget() {
        assert_eq!(
            validate_bgra_buffer(17, 1, 1024, SMALL),
            Err(PixelBufferError::DimensionTooLarge)
        );
        assert_eq!(
            validate_bgra_buffer(16, 16, 4096, SMALL),
            Err(PixelBufferError::ByteBudgetExceeded)
        );
    }

    #[test]
    fn stride_arithmetic_never_wraps_for_extreme_widths() {
        let huge = PixelLimits {
            max_dimension: u32::MAX,
            max_bytes: usize::MAX,
        };
        assert_eq!(
            bgra_layout(u32::MAX, 1, huge),
            Err(PixelBufferError::DimensionTooLarge)
        );
    }

    #[test]
    fn snapshot_budget_admits_a_4k_window_and_rejects_an_8k_one() {
        assert!(bgra_layout(3840, 2160, PixelLimits::WINDOW_SNAPSHOT).is_ok());
        assert_eq!(
            bgra_layout(7680, 4320, PixelLimits::WINDOW_SNAPSHOT),
            Err(PixelBufferError::ByteBudgetExceeded)
        );
    }

    #[test]
    fn crop_copies_the_requested_rows_and_columns() {
        let layout = bgra_layout(3, 2, SMALL).unwrap();
        let source: Vec<u8> = (0..24).collect();
        let cropped = crop_bgra(
            &source,
            layout,
            CropRegion {
                x: 1,
                y: 1,
                width: 2,
                height: 1,
            },
        )
        .unwrap();
        assert_eq!(cropped, vec![16, 17, 18, 19, 20, 21, 22, 23]);
    }

    #[test]
    fn crop_rejects_regions_outside_the_source() {
        let layout = bgra_layout(3, 2, SMALL).unwrap();
        let source = vec![0u8; 24];
        let outside = CropRegion {
            x: 2,
            y: 0,
            width: 2,
            height: 1,
        };
        assert!(crop_bgra(&source, layout, outside).is_none());
        let short = vec![0u8; 8];
        let inside = CropRegion {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        assert!(crop_bgra(&short, layout, inside).is_none());
    }

    fn hand_built(width: u32, height: u32, stride: u32, byte_len: usize) -> BgraLayout {
        BgraLayout {
            width,
            height,
            stride,
            byte_len,
        }
    }

    #[test]
    fn only_layouts_whose_stride_and_byte_length_follow_from_their_size_are_consistent() {
        assert!(bgra_layout(3, 2, SMALL).unwrap().is_consistent());
        assert!(!hand_built(4, 4, 4, 16).is_consistent());
        assert!(!hand_built(4, 4, 16, 63).is_consistent());
        assert!(!hand_built(0, 4, 0, 0).is_consistent());
        assert!(!hand_built(4, 0, 16, 0).is_consistent());
    }

    #[test]
    fn crop_refuses_a_hand_built_layout_whose_stride_contradicts_its_width() {
        let lying = hand_built(4, 4, 4, 16);
        let source = vec![0u8; 16];
        let whole = CropRegion {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        assert!(crop_bgra(&source, lying, whole).is_none());
    }

    #[test]
    fn resize_refuses_hand_built_layouts_that_contradict_their_size() {
        let target = bgra_layout(2, 2, ANY).unwrap();
        let lying_source = hand_built(4, 4, 4, 16);
        assert!(resize_bgra(&[0; 16], lying_source, target).is_none());

        let source = bgra_layout(2, 2, ANY).unwrap();
        let lying_target = hand_built(4, 4, 4, 16);
        assert!(resize_bgra(&[0; 16], source, lying_target).is_none());
    }

    #[test]
    fn downsample_half_averages_2x2_blocks_and_shrinks_by_four_bytes() {
        let pixels = vec![
            10, 20, 30, 255, 30, 40, 50, 255, 0, 0, 0, 255, 0, 0, 0, 255, 50, 60, 70, 255, 70, 80,
            90, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 40, 80, 120, 255, 40,
            80, 120, 255, 0, 0, 0, 255, 0, 0, 0, 255, 40, 80, 120, 255, 40, 80, 120, 255,
        ];
        let (w, h, out) = downsample_bgra_half(&pixels, 4, 4).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(out.len(), 16);
        assert_eq!(&out[0..4], &[40, 50, 60, 255]);
        assert_eq!(&out[12..16], &[40, 80, 120, 255]);
    }

    #[test]
    fn downsample_to_fit_caps_large_buffers_and_keeps_small_ones() {
        let small = vec![120u8; 4 * 4 * 4];
        let (sw, sh, sout) = downsample_bgra_to_fit(&small, 4, 4, 8).unwrap();
        assert_eq!((sw, sh), (4, 4));
        assert_eq!(sout.len(), small.len());

        let large = vec![200u8; 16 * 8 * 4];
        let (lw, lh, lout) = downsample_bgra_to_fit(&large, 16, 8, 4).unwrap();
        assert_eq!((lw, lh), (4, 2));
        assert_eq!(lout.len(), 4 * 2 * 4);
    }
}
