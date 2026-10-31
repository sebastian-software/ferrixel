//! A conservative scalar sRGB reference: decode the exact sRGB transfer
//! function, premultiply straight-alpha pixels in **linear light**, bilinearly
//! filter colors and alpha, then unpremultiply and encode straight sRGB.
//!
//! This is not ICC conversion, tone mapping, or a general HDR resampler.

#[derive(Debug, Clone, Copy)]
struct PremultipliedPixel {
    color: [f64; 3],
    alpha: f64,
}

fn decode_channel(code: u8) -> f64 {
    let value = f64::from(code) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_channel(linear: f64) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let encoded = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    quantize_unit(encoded)
}

// Values passed to this routine have been explicitly bounded to [0, 1].
// The cast is isolated here, not permitted across the interpolation kernel.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn quantize_unit(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn load_pixel(row: &[u8], offset: usize, channels: usize) -> PremultipliedPixel {
    debug_assert!(channels == 3 || channels == 4);
    let alpha = if channels == 4 {
        f64::from(row[offset + 3]) / 255.0
    } else {
        1.0
    };
    let mut color = [0.0; 3];
    for (channel, value) in color.iter_mut().enumerate() {
        *value = decode_channel(row[offset + channel]) * alpha;
    }
    PremultipliedPixel { color, alpha }
}

fn lerp(left: f64, right: f64, weight: f64) -> f64 {
    left + (right - left) * weight
}

fn interpolate(
    left: PremultipliedPixel,
    right: PremultipliedPixel,
    weight: f64,
) -> PremultipliedPixel {
    let mut color = [0.0; 3];
    for (channel, value) in color.iter_mut().enumerate() {
        *value = lerp(left.color[channel], right.color[channel], weight);
    }
    PremultipliedPixel {
        color,
        alpha: lerp(left.alpha, right.alpha, weight),
    }
}

/// Returns RGB or straight RGBA code values. The fourth element is ignored
/// for RGB. Input alpha is *not* transfer-encoded; only the color components
/// follow the sRGB OETF/EOTF.
pub(crate) fn bilinear(
    top: &[u8],
    bottom: &[u8],
    lower_offset: usize,
    upper_offset: usize,
    channels: usize,
    weight_x: f64,
    weight_y: f64,
) -> [u8; 4] {
    let top_left = load_pixel(top, lower_offset, channels);
    let top_right = load_pixel(top, upper_offset, channels);
    let bottom_left = load_pixel(bottom, lower_offset, channels);
    let bottom_right = load_pixel(bottom, upper_offset, channels);
    let upper = interpolate(top_left, top_right, weight_x);
    let lower = interpolate(bottom_left, bottom_right, weight_x);
    let mixed = interpolate(upper, lower, weight_y);

    let mut output = [0_u8; 4];
    if mixed.alpha > 0.0 {
        for (channel, value) in output.iter_mut().take(3).enumerate() {
            *value = encode_channel(mixed.color[channel] / mixed.alpha);
        }
    }
    output[3] = quantize_unit(mixed.alpha);
    output
}
