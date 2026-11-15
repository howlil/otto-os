use crate::theme::Rgba;

const HUE_BINS: usize = 12;
const MAX_SAMPLES: usize = 1_024;
const MIN_ALPHA: u8 = 128;
const MIN_SATURATION: f32 = 0.15;
const MIN_VALUE: f32 = 0.15;
const MIN_COLOURFUL_SHARE: f32 = 0.04;
const ACCENT_MIN_SATURATION: f32 = 0.55;
const ACCENT_MIN_VALUE: f32 = 0.90;

#[derive(Clone, Copy, Default)]
struct HueBin {
    weight: f32,
    red: f32,
    green: f32,
    blue: f32,
}

#[must_use]
pub fn cover_accent(bgra: &[u8], width: u32, height: u32) -> Option<Rgba> {
    let pixels = (width as usize).checked_mul(height as usize)?;
    if pixels == 0 || bgra.len() < pixels.checked_mul(4)? {
        return None;
    }
    let step = pixels.div_ceil(MAX_SAMPLES).max(1);
    let mut bins = [HueBin::default(); HUE_BINS];
    let mut sampled = 0usize;
    for index in (0..pixels).step_by(step) {
        let at = index * 4;
        if bgra[at + 3] < MIN_ALPHA {
            continue;
        }
        sampled += 1;
        let (red, green, blue) = (
            f32::from(bgra[at + 2]) / 255.0,
            f32::from(bgra[at + 1]) / 255.0,
            f32::from(bgra[at]) / 255.0,
        );
        let (hue, saturation, value) = rgb_to_hsv(red, green, blue);
        if saturation < MIN_SATURATION || value < MIN_VALUE {
            continue;
        }
        let weight = saturation * value;
        let bin = &mut bins[((hue / 360.0) * HUE_BINS as f32) as usize % HUE_BINS];
        bin.weight += weight;
        bin.red += red * weight;
        bin.green += green * weight;
        bin.blue += blue * weight;
    }
    if sampled == 0 {
        return None;
    }
    let total: f32 = bins.iter().map(|bin| bin.weight).sum();
    let best = bins
        .iter()
        .copied()
        .max_by(|left, right| left.weight.total_cmp(&right.weight))?;
    if best.weight <= 0.0 || total / (sampled as f32) < MIN_COLOURFUL_SHARE {
        return None;
    }
    let (hue, saturation, value) = rgb_to_hsv(
        best.red / best.weight,
        best.green / best.weight,
        best.blue / best.weight,
    );
    let (red, green, blue) = hsv_to_rgb(
        hue,
        saturation.max(ACCENT_MIN_SATURATION),
        value.clamp(ACCENT_MIN_VALUE, 1.0),
    );
    Some(Rgba::new(red, green, blue, 1.0))
}

fn rgb_to_hsv(red: f32, green: f32, blue: f32) -> (f32, f32, f32) {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - red).abs() <= f32::EPSILON {
        60.0 * (((green - blue) / delta).rem_euclid(6.0))
    } else if (max - green).abs() <= f32::EPSILON {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue.rem_euclid(360.0), saturation, max)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (f32, f32, f32) {
    let chroma = value * saturation;
    let sector = hue.rem_euclid(360.0) / 60.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (red, green, blue) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let offset = value - chroma;
    (red + offset, green + offset, blue + offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(red: u8, green: u8, blue: u8, count: usize) -> Vec<u8> {
        (0..count).flat_map(|_| [blue, green, red, 255]).collect()
    }

    fn near(value: f32, expected: f32) -> bool {
        (value - expected).abs() < 0.02
    }

    #[test]
    fn pure_primaries_keep_their_hue() {
        let red = cover_accent(&solid(255, 0, 0, 64), 8, 8).expect("red");
        assert!(near(red.r, 1.0) && red.g < 0.1 && red.b < 0.1);
        let green = cover_accent(&solid(0, 255, 0, 64), 8, 8).expect("green");
        assert!(near(green.g, 1.0) && green.r < 0.1 && green.b < 0.1);
        let blue = cover_accent(&solid(0, 0, 255, 64), 8, 8).expect("blue");
        assert!(near(blue.b, 1.0) && blue.r < 0.1 && blue.g < 0.1);
    }

    #[test]
    fn grey_black_white_and_empty_covers_have_no_accent() {
        assert_eq!(cover_accent(&solid(128, 128, 128, 64), 8, 8), None);
        assert_eq!(cover_accent(&solid(0, 0, 0, 64), 8, 8), None);
        assert_eq!(cover_accent(&solid(255, 255, 255, 64), 8, 8), None);
        assert_eq!(cover_accent(&[], 0, 0), None);
        assert_eq!(cover_accent(&solid(255, 0, 0, 4), 8, 8), None);
    }

    #[test]
    fn transparent_pixels_are_ignored() {
        let mut pixels = solid(255, 0, 0, 64);
        for at in (3..pixels.len()).step_by(4) {
            pixels[at] = 0;
        }
        assert_eq!(cover_accent(&pixels, 8, 8), None);
    }

    #[test]
    fn the_larger_colour_wins_over_a_smaller_one() {
        let mut pixels = solid(0, 0, 255, 48);
        pixels.extend(solid(255, 0, 0, 16));
        let accent = cover_accent(&pixels, 8, 8).expect("accent");
        assert!(accent.b > accent.r);
    }

    #[test]
    fn dark_covers_are_lifted_to_a_readable_accent() {
        let accent = cover_accent(&solid(0, 0, 60, 64), 8, 8).expect("dark blue");
        assert!(accent.b >= 0.89);
        assert!(accent.r < 0.1 && accent.g < 0.1);
        let pale = cover_accent(&solid(255, 200, 200, 64), 8, 8).expect("pale red");
        let (_, saturation, value) = rgb_to_hsv(pale.r, pale.g, pale.b);
        assert!(saturation >= ACCENT_MIN_SATURATION - 0.01 && value >= ACCENT_MIN_VALUE);
    }

    #[test]
    fn large_covers_are_sampled_without_reading_every_pixel() {
        let accent = cover_accent(&solid(0, 255, 0, 128 * 128), 128, 128).expect("green");
        assert!(near(accent.g, 1.0));
    }

    #[test]
    fn hsv_round_trips() {
        for (red, green, blue) in [(0.9, 0.2, 0.4), (0.1, 0.8, 0.3), (0.3, 0.4, 0.95)] {
            let (hue, saturation, value) = rgb_to_hsv(red, green, blue);
            let (r, g, b) = hsv_to_rgb(hue, saturation, value);
            assert!(near(r, red) && near(g, green) && near(b, blue));
        }
    }
}
