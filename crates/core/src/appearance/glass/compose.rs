use super::buffer::RgbBuffer;
use super::color::mix;
use super::color::scale_rgb;
use super::color::to_byte;
use super::params::BLACK;
use super::params::GlassParams;
use super::params::LIGHT_DIRECTION;
use super::params::OUTLINE_REACH;
use super::params::Rgb;
use super::params::WHITE;
use super::shape::GlassBackdrop;
use super::shape::GlassBitmap;
use super::shape::GlassShape;

#[must_use]
pub fn soften_backdrop(
    capture: &RgbBuffer,
    params: &GlassParams,
    factor: usize,
    blur_radius: usize,
) -> RgbBuffer {
    let mut soft = capture.downsample_linearized(factor);
    soft.box_blur_in_place(blur_radius);
    soft.gamma_encode_and_grade_in_place(params);
    soft
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
    const SHARP_WEIGHT: f32 = 0.55;

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
        color = mix(color, graded, params.lens_mix * SHARP_WEIGHT * meniscus);
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

pub(super) fn smoothstep(t: f32) -> f32 {
    let clamped = t.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}
pub(super) const SRGB_BYTE_TO_LINEAR_LUT: [f32; 256] = [
    0.0,
    0.000_303_527,
    0.000_607_054,
    0.000_910_581,
    0.001_214_108,
    0.001_517_635,
    0.001_821_162,
    0.002_124_688_8,
    0.002_428_216,
    0.002_731_743,
    0.003_035_27,
    0.003_346_535_8,
    0.003_676_507_4,
    0.004_024_717,
    0.004_391_442,
    0.004_776_953_7,
    0.005_181_517,
    0.005_605_392,
    0.006_048_833_5,
    0.006_512_091,
    0.006_995_410_7,
    0.007_499_032,
    0.008_023_193,
    0.008_568_126,
    0.009_134_059,
    0.009_721_218,
    0.010_329_823,
    0.010_960_095,
    0.011_612_245,
    0.012_286_489,
    0.012_983_032_5,
    0.013_702_083,
    0.014_443_845,
    0.015_208_516,
    0.015_996_294,
    0.016_807_377,
    0.017_641_956,
    0.018_500_222,
    0.019_382_363,
    0.020_288_564,
    0.021_219_011,
    0.022_173_885,
    0.023_153_368,
    0.024_157_634,
    0.025_186_861,
    0.026_241_222,
    0.027_320_893,
    0.028_426_04,
    0.029_556_835,
    0.030_713_445,
    0.031_896_032,
    0.033_104_766,
    0.034_339_808,
    0.035_601_314,
    0.036_889_452,
    0.038_204_372,
    0.039_546_236,
    0.040_915_2,
    0.042_311_41,
    0.043_735_03,
    0.045_186_203,
    0.046_665_087,
    0.048_171_826,
    0.049_706_567,
    0.051_269_464,
    0.052_860_65,
    0.054_480_28,
    0.056_128_494,
    0.057_805_438,
    0.059_511_244,
    0.061_246_06,
    0.063_010_02,
    0.064_803_27,
    0.066_625_945,
    0.068_478_175,
    0.070_360_1,
    0.072_271_85,
    0.074_213_57,
    0.076_185_39,
    0.078_187_43,
    0.080_219_83,
    0.082_282_715,
    0.084_376_216,
    0.086_500_466,
    0.088_655_59,
    0.090_841_72,
    0.093_058_966,
    0.095_307_47,
    0.097_587_354,
    0.099_898_73,
    0.102_241_74,
    0.104_616_49,
    0.107_023_105,
    0.109_461_72,
    0.111_932_434,
    0.114_435_38,
    0.116_970_67,
    0.119_538_434,
    0.122_138_776,
    0.124_771_82,
    0.127_437_68,
    0.130_136_47,
    0.132_868_32,
    0.135_633_33,
    0.138_431_62,
    0.141_263_29,
    0.144_128_47,
    0.147_027_27,
    0.149_959_79,
    0.152_926_16,
    0.155_926_47,
    0.158_960_83,
    0.162_029_39,
    0.165_132_2,
    0.168_269_4,
    0.171_441_11,
    0.174_647_4,
    0.177_888_42,
    0.181_164_25,
    0.184_474_99,
    0.187_820_78,
    0.191_201_69,
    0.194_617_84,
    0.198_069_32,
    0.201_556_25,
    0.205_078_74,
    0.208_636_87,
    0.212_230_76,
    0.215_860_53,
    0.219_526_23,
    0.223_227_98,
    0.226_965_9,
    0.230_740_07,
    0.234_550_61,
    0.238_397_6,
    0.242_281_15,
    0.246_201_35,
    0.250_158_3,
    0.254_152_12,
    0.258_182_88,
    0.262_250_7,
    0.266_355_63,
    0.270_497_83,
    0.274_677_34,
    0.278_894_3,
    0.283_148_77,
    0.287_440_87,
    0.291_770_67,
    0.296_138_3,
    0.300_543_8,
    0.304_987_34,
    0.309_468_95,
    0.313_988_75,
    0.318_546_8,
    0.323_143_24,
    0.327_778_13,
    0.332_451_55,
    0.337_163_66,
    0.341_914_45,
    0.346_704_1,
    0.351_532_64,
    0.356_400_16,
    0.361_306_82,
    0.366_252_63,
    0.371_237_7,
    0.376_262_16,
    0.381_326_05,
    0.386_429_46,
    0.391_572_5,
    0.396_755_25,
    0.401_977_8,
    0.407_240_24,
    0.412_542_64,
    0.417_885_1,
    0.423_267_7,
    0.428_690_52,
    0.434_153_68,
    0.439_657_2,
    0.445_201_22,
    0.450_785_82,
    0.456_411_03,
    0.462_077_02,
    0.467_783_8,
    0.473_531_5,
    0.479_320_2,
    0.485_149_95,
    0.491_020_9,
    0.496_933,
    0.502_886_5,
    0.508_881_33,
    0.514_917_7,
    0.520_995_6,
    0.527_115_17,
    0.533_276_44,
    0.539_479_5,
    0.545_724_5,
    0.552_011_43,
    0.558_340_43,
    0.564_711_5,
    0.571_124_85,
    0.577_580_45,
    0.584_078_43,
    0.590_618_85,
    0.597_201_8,
    0.603_827_36,
    0.610_495_57,
    0.617_206_6,
    0.623_960_44,
    0.630_757_15,
    0.637_596_9,
    0.644_479_7,
    0.651_405_63,
    0.658_374_85,
    0.665_387_33,
    0.672_443_15,
    0.679_542_5,
    0.686_685_3,
    0.693_871_8,
    0.701_101_9,
    0.708_375_8,
    0.715_693_53,
    0.723_055_1,
    0.730_460_76,
    0.737_910_45,
    0.745_404_24,
    0.752_942_2,
    0.760_524_5,
    0.768_151_16,
    0.775_822_2,
    0.783_537_8,
    0.791_298,
    0.799_102_7,
    0.806_952_3,
    0.814_846_6,
    0.822_785_74,
    0.830_769_9,
    0.838_799,
    0.846_873_2,
    0.854_992_6,
    0.863_157_2,
    0.871_367_1,
    0.879_622_4,
    0.887_923_1,
    0.896_269_4,
    0.904_661_2,
    0.913_098_63,
    0.921_581_86,
    0.930_110_9,
    0.938_685_7,
    0.947_306_5,
    0.955_973_3,
    0.964_686_3,
    0.973_445_3,
    0.982_250_6,
    0.991_102_1,
    1.0,
];
