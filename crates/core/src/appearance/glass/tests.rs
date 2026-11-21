use super::buffer::*;
use super::color::*;
use super::compose::*;
use super::params::*;
use super::shape::*;
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

fn backdrop_for<'a>(sharp: &'a RgbBuffer, soft: &'a RgbBuffer, factor: usize) -> GlassBackdrop<'a> {
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
            assert!((roundtrip.pixel(0, 0)[channel] - buffer.pixel(0, 0)[channel]).abs() < 1e-6);
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
    for y in 0..36 {
        for x in 0..320 {
            sharp.set_pixel(x, y, [1.0; 3]);
        }
    }
    let soft = soften_backdrop(&sharp, &GlassParams::DARK, 2, 5);
    let backdrop = backdrop_for(&sharp, &soft, 2);
    let shape = pill();
    let dispersive = GlassParams {
        dispersion: 3.0,
        ..GlassParams::DARK
    };
    let no_disp = GlassParams {
        dispersion: 0.0,
        ..GlassParams::DARK
    };
    let mut strongest = ([0.0; 3], 0.0f32);
    for y in 30..=42 {
        let with_disp = glass_color(&dispersive, &shape, &backdrop, 160.0, y as f32);
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
    let bitmap = compose_glass_lit(&GlassParams::DARK, &pill(), &backdrop, LIGHT_DIRECTION);
    assert_eq!(
        bitmap.pixels.len(),
        bitmap.width as usize * bitmap.height as usize * 4
    );
    let at = |x: i32, y: i32| {
        let index = ((y - bitmap.y) as usize * bitmap.width as usize + (x - bitmap.x) as usize) * 4;
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

fn bgra_pattern(width: usize, height: usize) -> RgbBuffer {
    let mut bytes = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let b = ((x * 37 + y * 19) % 256) as u8;
            let g = ((x * 13 + y * 53 + 7) % 256) as u8;
            let r = ((x * 71 + y * 11 + 29) % 256) as u8;
            bytes.extend_from_slice(&[b, g, r, 255]);
        }
    }
    RgbBuffer::from_bgra(width as u32, height as u32, &bytes).unwrap()
}

#[test]
fn srgb_byte_lut_matches_srgb_to_linear_for_all_bytes() {
    for byte in 0..=255u8 {
        let norm = f32::from(byte) / 255.0;
        let expected = srgb_to_linear(norm);
        assert!(
            (SRGB_BYTE_TO_LINEAR_LUT[byte as usize] - expected).abs() < 1e-6,
            "lut mismatch at byte {byte}"
        );
        assert!(
            (srgb_byte_to_linear(byte) - expected).abs() < 1e-6,
            "helper mismatch at byte {byte}"
        );
        assert!(
            (srgb_to_linear_fast(norm) - expected).abs() < 1e-6,
            "fast mismatch at byte {byte}"
        );
    }
    for arbitrary in [-0.1f32, 0.02, 0.3, 0.5, 0.7, 1.2] {
        assert!((srgb_to_linear_fast(arbitrary) - srgb_to_linear(arbitrary)).abs() < 1e-6);
    }
}

#[test]
fn downsample_linearized_matches_reference_across_odd_and_even_dimensions() {
    for (width, height) in [(16usize, 12usize), (17, 13), (5, 3), (1, 1)] {
        let bgra = bgra_pattern(width, height);
        let mut mixed = RgbBuffer::filled(width, height, [0.3, 0.5, 0.7]);
        mixed.set_pixel(0, 0, [0.123, 0.456, 0.789]);
        for capture in [&bgra, &mixed] {
            for factor in [1usize, 2, 3, 4] {
                let fast = capture.downsample_linearized(factor);
                let reference = capture.linearized().downsample(factor);
                assert_eq!(
                    (fast.width(), fast.height()),
                    (reference.width(), reference.height())
                );
                for y in 0..reference.height() {
                    for x in 0..reference.width() {
                        assert!(
                            rgb_distance(fast.pixel(x, y), reference.pixel(x, y)) < 1e-6,
                            "mismatch at ({x},{y}) for {width}x{height} factor {factor}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn soften_backdrop_matches_full_res_reference_across_odd_and_even_dimensions() {
    for (width, height) in [(32usize, 24usize), (33, 25), (19, 11)] {
        let bgra = bgra_pattern(width, height);
        let gradient = stripes(width, height);
        for capture in [&bgra, &gradient] {
            for params in [GlassParams::DARK, GlassParams::LIGHT] {
                for factor in [1usize, 2, 3, 4] {
                    for radius in [0usize, 1, 3, 5] {
                        let actual = soften_backdrop(capture, &params, factor, radius);
                        let reference = capture
                            .linearized()
                            .downsample(factor)
                            .box_blurred(radius)
                            .gamma_encoded()
                            .graded(&params);
                        assert_eq!(
                            (actual.width(), actual.height()),
                            (reference.width(), reference.height())
                        );
                        for y in 0..reference.height() {
                            for x in 0..reference.width() {
                                assert!(
                                    rgb_distance(actual.pixel(x, y), reference.pixel(x, y)) < 1e-5,
                                    "soften mismatch at ({x},{y}) for {width}x{height} factor {factor} radius {radius}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn to_rgba_u8_packs_clamped_bytes_with_opaque_alpha() {
    let mut buffer = RgbBuffer::filled(2, 2, [0.0, 0.5, 1.0]);
    buffer.set_pixel(1, 0, [-0.25, 1.5, 128.0 / 255.0]);
    buffer.set_pixel(0, 1, [0.2, 0.4, 0.8]);
    buffer.set_pixel(1, 1, [1.0, 0.0, 0.25]);
    let rgba = buffer.to_rgba_u8();
    assert_eq!(rgba.len(), 2 * 2 * 4);
    let expected = [
        to_byte(0.0),
        to_byte(0.5),
        to_byte(1.0),
        255,
        0,
        255,
        128,
        255,
        to_byte(0.2),
        to_byte(0.4),
        to_byte(0.8),
        255,
        255,
        0,
        to_byte(0.25),
        255,
    ];
    assert_eq!(rgba, expected);
    let mut scratch = Vec::with_capacity(64);
    scratch.extend_from_slice(&[9u8; 20]);
    buffer.write_rgba_u8(&mut scratch);
    assert_eq!(scratch, expected);
    assert_eq!(scratch.capacity(), 64);
}

#[test]
fn separable_box_blur_matches_reference_across_narrow_and_wide_dimensions() {
    fn reference_pass(buf: &RgbBuffer, radius: usize) -> RgbBuffer {
        let w = buf.width();
        let h = buf.height();
        let reach = radius as isize;
        let window = (2 * radius + 1) as f32;
        let mut horiz = RgbBuffer::filled(w, h, [0.0; 3]);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0f32; 3];
                for offset in -reach..=reach {
                    let sx = (x as isize + offset).clamp(0, w as isize - 1) as usize;
                    let p = buf.pixel(sx, y);
                    for c in 0..3 {
                        sum[c] += p[c];
                    }
                }
                horiz.set_pixel(x, y, [sum[0] / window, sum[1] / window, sum[2] / window]);
            }
        }
        let mut out = RgbBuffer::filled(w, h, [0.0; 3]);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0f32; 3];
                for offset in -reach..=reach {
                    let sy = (y as isize + offset).clamp(0, h as isize - 1) as usize;
                    let p = horiz.pixel(x, sy);
                    for c in 0..3 {
                        sum[c] += p[c];
                    }
                }
                out.set_pixel(x, y, [sum[0] / window, sum[1] / window, sum[2] / window]);
            }
        }
        out
    }

    for (width, height, radius) in [(1, 1, 3), (3, 5, 3), (7, 4, 2), (25, 17, 4), (40, 21, 6)] {
        let input = bgra_pattern(width, height);
        let fast = input.box_blurred(radius);
        let mut expected = input.clone();
        for _ in 0..3 {
            expected = reference_pass(&expected, radius);
        }
        for y in 0..height {
            for x in 0..width {
                assert!(
                    rgb_distance(fast.pixel(x, y), expected.pixel(x, y)) < 1e-4,
                    "blur mismatch at ({x},{y}) for {width}x{height} r={radius}"
                );
            }
        }
    }
}
