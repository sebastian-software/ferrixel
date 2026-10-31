//! Reference SDR color arithmetic for the future color-transform graph node.
//!
//! This module is deliberately **not** an eager frame or a public transform
//! API. It operates on one RGB triple at a time, preserving out-of-gamut
//! linear values and leaving rendering intent, clipping, ICC and HDR to future
//! explicit stages.
//!
//! The exact D65 matrices are the rational reference coefficients from
//! W3C CSS Color 4 §19 (sRGB / Display P3 -> XYZ D65 and back):
//! https://www.w3.org/TR/css-color-4/#color-conversion-code
//! Both color spaces use the sRGB transfer function, but *not* the same
//! primaries. No chromatic adaptation is needed because both use D65.

use crate::color::ColorEncoding;
use crate::error::Error;

/// The finite reference output is linear-light sRGB, not sRGB-coded samples.
/// Components can legitimately be negative or greater than one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LinearSrgb {
    pub rgb: [f64; 3],
}

/// RGB has already been decoded to linear sRGB and premultiplied by opacity.
/// This representation can later be bilinearly filtered without transparent
/// neighbor colors bleeding into the visible result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PremultipliedLinearSrgb {
    pub rgb: [f64; 3],
    pub alpha: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SdrSource {
    Srgb,
    DisplayP3,
}

/// Fail closed unless the *rendered sample profile*, with no conflicting ICC,
/// is specifically full-range RGB sRGB or Display P3 with sRGB transfer.
fn source_from_color(color: &ColorEncoding) -> Result<SdrSource, Error> {
    match color {
        ColorEncoding::Cicp([1, 13, 0, 1]) => Ok(SdrSource::Srgb),
        ColorEncoding::Cicp([12, 13, 0, 1]) => Ok(SdrSource::DisplayP3),
        _ => Err(Error::UnsupportedColorEncoding),
    }
}

/// Extended sRGB electro-optical transfer function. Preserve the sign of
/// extended-range encoded samples rather than clipping them to [0, 1].
fn decode_srgb(encoded: f64) -> f64 {
    let magnitude = encoded.abs();
    if magnitude <= 0.040_45 {
        encoded / 12.92
    } else {
        encoded.signum() * ((magnitude + 0.055) / 1.055).powf(2.4)
    }
}

/// Matrix-vector multiplication with no quantization, clipping or allocation.
fn transform(matrix: [[f64; 3]; 3], rgb: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0].mul_add(rgb[0], row[1].mul_add(rgb[1], row[2] * rgb[2])))
}

/// The W3C CSS Color 4 Display P3 (D65) to CIE XYZ D65 reference matrix.
const P3_TO_XYZ_D65: [[f64; 3]; 3] = [
    [
        608311.0 / 1250200.0,
        189793.0 / 714400.0,
        198249.0 / 1000160.0,
    ],
    [
        35783.0 / 156275.0,
        247089.0 / 357200.0,
        198249.0 / 2500400.0,
    ],
    [0.0, 32229.0 / 714400.0, 5220557.0 / 5000800.0],
];

/// The W3C CSS Color 4 CIE XYZ D65 to linear sRGB reference matrix.
const XYZ_D65_TO_SRGB: [[f64; 3]; 3] = [
    [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
    [
        -851781.0 / 878810.0,
        1648619.0 / 878810.0,
        36519.0 / 878810.0,
    ],
    [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
];

/// Decode a *trusted* SDR color signal into unclipped, linear-light sRGB.
/// Input is a floating-point encoded color triple, not inherently u8/RGBA.
/// The result is a tiny reference value, not a materialized image surface.
pub(crate) fn decode_to_linear_srgb(
    color: &ColorEncoding,
    encoded_rgb: [f64; 3],
) -> Result<LinearSrgb, Error> {
    let source = source_from_color(color)?;
    if encoded_rgb.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::NonFinitePixelSample);
    }
    let linear_rgb = encoded_rgb.map(decode_srgb);
    let rgb = match source {
        SdrSource::Srgb => linear_rgb,
        SdrSource::DisplayP3 => {
            let xyz = transform(P3_TO_XYZ_D65, linear_rgb);
            transform(XYZ_D65_TO_SRGB, xyz)
        }
    };
    if rgb.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::NonFinitePixelSample);
    }
    Ok(LinearSrgb { rgb })
}

/// Convert explicitly straight alpha to **linear-light premultiplied sRGB**
/// after the color transformation, never in gamma-coded RGB. This is a small
/// scalar reference for a future region kernel, not an output encoder.
pub(crate) fn decode_straight_to_premultiplied_linear_srgb(
    color: &ColorEncoding,
    encoded_rgb: [f64; 3],
    alpha: f64,
) -> Result<PremultipliedLinearSrgb, Error> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(Error::InvalidAlphaValue);
    }
    let linear = decode_to_linear_srgb(color, encoded_rgb)?;
    Ok(PremultipliedLinearSrgb {
        rgb: linear.rgb.map(|sample| sample * alpha),
        alpha,
    })
}
