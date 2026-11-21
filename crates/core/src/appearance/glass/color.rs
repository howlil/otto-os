use super::compose::SRGB_BYTE_TO_LINEAR_LUT;
use super::params::Rgb;

const fn build_byte_to_f32_lut() -> [f32; 256] {
    let mut lut = [0.0f32; 256];
    let mut i = 0;
    while i < 256 {
        lut[i] = i as f32 / 255.0;
        i += 1;
    }
    lut
}

pub(super) const BYTE_TO_F32_LUT: [f32; 256] = build_byte_to_f32_lut();

pub(super) const fn srgb_byte_to_linear(byte: u8) -> f32 {
    SRGB_BYTE_TO_LINEAR_LUT[byte as usize]
}

pub(super) fn srgb_to_linear_fast(value: f32) -> f32 {
    if (0.0..=1.0).contains(&value) {
        let byte = (value * 255.0 + 0.5) as u8;
        if (value - BYTE_TO_F32_LUT[byte as usize]) == 0.0 {
            return srgb_byte_to_linear(byte);
        }
    }
    srgb_to_linear(value)
}

pub(super) fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub(super) fn luminance(rgb: Rgb) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

pub(super) fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    [
        from[0] + (to[0] - from[0]) * amount,
        from[1] + (to[1] - from[1]) * amount,
        from[2] + (to[2] - from[2]) * amount,
    ]
}

pub(super) fn scale_rgb(rgb: Rgb, factor: f32) -> Rgb {
    [rgb[0] * factor, rgb[1] * factor, rgb[2] * factor]
}

pub(super) fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
