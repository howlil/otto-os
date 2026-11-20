#[derive(Debug, Clone, PartialEq)]
pub struct CoverStyle {
    pub rows: Vec<[u8; 3]>,
    pub top_right_radius: f32,
    pub top_border_inset: bool,
    pub right_border_inset: bool,
    pub feather_width: f32,
    pub is_dark: bool,
}

impl CoverStyle {
    #[must_use]
    pub fn solid(rgb: [u8; 3], top_right_radius: f32, border_inset: bool) -> Self {
        Self {
            rows: vec![rgb],
            top_right_radius,
            top_border_inset: border_inset,
            right_border_inset: border_inset,
            feather_width: 0.0,
            is_dark: rgb_is_dark(rgb),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverBand {
    pub top: i32,
    pub bottom: i32,
    pub rgb: [u8; 3],
}

pub(super) const fn rgb_luma(rgb: [u8; 3]) -> u32 {
    299 * rgb[0] as u32 + 587 * rgb[1] as u32 + 114 * rgb[2] as u32
}

pub(super) const fn rgb_is_dark(rgb: [u8; 3]) -> bool {
    rgb_luma(rgb) < 128_000
}

#[must_use]
pub fn cover_is_dark(rows: &[[u8; 3]]) -> bool {
    if rows.is_empty() {
        return true;
    }
    let slice = if rows.len() > 2 {
        &rows[1..rows.len() - 1]
    } else {
        rows
    };
    let mut lumas: Vec<u32> = slice.iter().copied().map(rgb_luma).collect();
    lumas.sort_unstable();
    lumas[lumas.len() / 2] < 128_000
}

pub(super) fn channel_median(values: &mut [u8]) -> u8 {
    values.sort_unstable();
    values[values.len() / 2]
}

pub(super) fn max_channel_diff(a: [u8; 3], b: [u8; 3]) -> u8 {
    a[0].abs_diff(b[0])
        .max(a[1].abs_diff(b[1]))
        .max(a[2].abs_diff(b[2]))
}

pub(super) fn row_pixel_rgb(row: &[u8], x: usize) -> [u8; 3] {
    let pixel = &row[x * 4..x * 4 + 4];
    [pixel[2], pixel[1], pixel[0]]
}

pub(super) fn is_horizontally_smooth(row: &[u8], x: usize, width: usize) -> bool {
    let cur = row_pixel_rgb(row, x);
    let left_smooth = x > 0 && max_channel_diff(cur, row_pixel_rgb(row, x - 1)) <= 12;
    let right_smooth = x + 1 < width && max_channel_diff(cur, row_pixel_rgb(row, x + 1)) <= 12;
    left_smooth || right_smooth
}

pub(super) fn dominant_background_rgb(
    bgra: &[u8],
    width: usize,
    height: usize,
    row_bytes: usize,
) -> [u8; 3] {
    let row_range = if height > 2 { 1..height - 1 } else { 0..height };
    let mut sum_r = vec![0_u64; 4096];
    let mut sum_g = vec![0_u64; 4096];
    let mut sum_b = vec![0_u64; 4096];
    let mut counts = vec![0_u32; 4096];
    let mut saw_smooth = false;
    for y in row_range.clone() {
        let row = &bgra[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..width {
            if is_horizontally_smooth(row, x, width) {
                saw_smooth = true;
                let rgb = row_pixel_rgb(row, x);
                let idx = (usize::from(rgb[0] >> 4) << 8)
                    | (usize::from(rgb[1] >> 4) << 4)
                    | usize::from(rgb[2] >> 4);
                sum_r[idx] += u64::from(rgb[0]);
                sum_g[idx] += u64::from(rgb[1]);
                sum_b[idx] += u64::from(rgb[2]);
                counts[idx] += 1;
            }
        }
    }
    if !saw_smooth {
        for y in row_range {
            let row = &bgra[y * row_bytes..(y + 1) * row_bytes];
            for x in 0..width {
                let rgb = row_pixel_rgb(row, x);
                let idx = (usize::from(rgb[0] >> 4) << 8)
                    | (usize::from(rgb[1] >> 4) << 4)
                    | usize::from(rgb[2] >> 4);
                sum_r[idx] += u64::from(rgb[0]);
                sum_g[idx] += u64::from(rgb[1]);
                sum_b[idx] += u64::from(rgb[2]);
                counts[idx] += 1;
            }
        }
    }
    let mut bins: Vec<([u8; 3], u32)> = Vec::new();
    for idx in 0..4096 {
        let count = counts[idx];
        if count > 0 {
            let denom = u64::from(count);
            bins.push((
                [
                    (sum_r[idx] / denom) as u8,
                    (sum_g[idx] / denom) as u8,
                    (sum_b[idx] / denom) as u8,
                ],
                count,
            ));
        }
    }
    bins.sort_unstable_by_key(|bin| std::cmp::Reverse(bin.1));
    bins.truncate(64);
    let Some(&(best_seed, _)) = bins.iter().max_by_key(|&&(seed_rgb, seed_count)| {
        let cluster_count: u64 = bins
            .iter()
            .filter(|&&(other_rgb, _)| max_channel_diff(seed_rgb, other_rgb) <= 48)
            .map(|&(_, other_count)| u64::from(other_count))
            .sum();
        (cluster_count, seed_count)
    }) else {
        return [0, 0, 0];
    };
    let mut total_r = 0_u64;
    let mut total_g = 0_u64;
    let mut total_b = 0_u64;
    let mut total_count = 0_u64;
    for &(bin_rgb, bin_count) in &bins {
        if max_channel_diff(best_seed, bin_rgb) <= 48 {
            let w = u64::from(bin_count);
            total_r += u64::from(bin_rgb[0]) * w;
            total_g += u64::from(bin_rgb[1]) * w;
            total_b += u64::from(bin_rgb[2]) * w;
            total_count += w;
        }
    }
    if total_count == 0 {
        return best_seed;
    }
    [
        (total_r / total_count) as u8,
        (total_g / total_count) as u8,
        (total_b / total_count) as u8,
    ]
}

pub(super) fn median_rgb_from_indices(
    row: &[u8],
    indices: &[usize],
    reds: &mut Vec<u8>,
    greens: &mut Vec<u8>,
    blues: &mut Vec<u8>,
) -> [u8; 3] {
    reds.clear();
    greens.clear();
    blues.clear();
    for &x in indices {
        let rgb = row_pixel_rgb(row, x);
        reds.push(rgb[0]);
        greens.push(rgb[1]);
        blues.push(rgb[2]);
    }
    [
        channel_median(reds),
        channel_median(greens),
        channel_median(blues),
    ]
}

#[must_use]
pub fn extract_cover_rows(bgra: &[u8], width: usize, height: usize) -> Option<Vec<[u8; 3]>> {
    if width == 0 || height == 0 {
        return None;
    }
    let row_bytes = width.checked_mul(4)?;
    let total_bytes = row_bytes.checked_mul(height)?;
    if bgra.len() < total_bytes {
        return None;
    }
    let base_rgb = dominant_background_rgb(bgra, width, height, row_bytes);
    let min_cluster_matches = (width / 4).max(usize::from(width >= 2));
    let mut rows = Vec::with_capacity(height);
    let mut cluster_smooth = Vec::with_capacity(width);
    let mut smooth_only = Vec::with_capacity(width);
    let mut cluster_any = Vec::with_capacity(width);
    let mut all_indices = Vec::with_capacity(width);
    let mut reds = Vec::with_capacity(width);
    let mut greens = Vec::with_capacity(width);
    let mut blues = Vec::with_capacity(width);
    for y in 0..height {
        let row = &bgra[y * row_bytes..(y + 1) * row_bytes];
        cluster_smooth.clear();
        smooth_only.clear();
        cluster_any.clear();
        all_indices.clear();
        for x in 0..width {
            all_indices.push(x);
            let rgb = row_pixel_rgb(row, x);
            let in_cluster = max_channel_diff(rgb, base_rgb) <= 48;
            if in_cluster {
                cluster_any.push(x);
            }
            if is_horizontally_smooth(row, x, width) {
                smooth_only.push(x);
                if in_cluster {
                    cluster_smooth.push(x);
                }
            }
        }
        let chosen = if cluster_smooth.len() >= min_cluster_matches && !cluster_smooth.is_empty() {
            &cluster_smooth
        } else if smooth_only.len() >= (width / 2).max(2) {
            &smooth_only
        } else if !cluster_any.is_empty() {
            &cluster_any
        } else {
            &all_indices
        };
        rows.push(median_rgb_from_indices(
            row,
            chosen,
            &mut reds,
            &mut greens,
            &mut blues,
        ));
    }
    Some(rows)
}

#[must_use]
pub fn cover_row_bands(rows: &[[u8; 3]], height: usize, border_inset: bool) -> Vec<CoverBand> {
    let start_row = usize::from(border_inset);
    if height <= start_row || rows.is_empty() {
        return Vec::new();
    }
    let fallback = *rows.last().unwrap_or(&[0, 0, 0]);
    let mut bands: Vec<CoverBand> = Vec::new();
    for y in start_row..height {
        let rgb = rows.get(y).copied().unwrap_or(fallback);
        let row_top = y as i32;
        let row_bottom = (y + 1) as i32;
        if let Some(last) = bands.last_mut()
            && last.rgb == rgb
            && last.bottom == row_top
        {
            last.bottom = row_bottom;
        } else {
            bands.push(CoverBand {
                top: row_top,
                bottom: row_bottom,
                rgb,
            });
        }
    }
    bands
}

pub const NATIVE_CAPTION_BUTTON_DIP: i32 = 46;
pub const NATIVE_CAPTION_HEIGHT_DIP: i32 = 30;
pub const NATIVE_CORNER_RADIUS_DIP: i32 = 8;
pub const COVER_RIGHT_MARGIN_DIP: i32 = 14;
pub const COVER_FEATHER_DIP: i32 = 6;
pub const COVER_SAMPLE_GAP_DIP: i32 = 2;
pub const COVER_SAMPLE_WIDTH_DIP: i32 = 24;
pub const RESIZE_BORDER_DIP: i32 = 4;
