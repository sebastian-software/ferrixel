//! Independent contracts for the first Display P3 / sRGB SDR reference.
//! The P3 red fixture is derived from W3C CSS Color 4's reference matrices.

use crate::color::ColorEncoding;
use crate::error::Error;
use crate::working_color::{decode_straight_to_premultiplied_linear_srgb, decode_to_linear_srgb};

fn near(actual: f64, expected: f64, epsilon: f64) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "expected {expected:?}, got {actual:?} (epsilon {epsilon:?})"
    );
}

#[test]
fn srgb_midpoint_is_decoded_before_any_resampling() {
    let color = ColorEncoding::Cicp([1, 13, 0, 1]);
    let linear = decode_to_linear_srgb(&color, [0.5, 0.5, 0.5]).unwrap();
    for sample in linear.rgb {
        near(sample, 0.214_041_140_482_232_55, 1.0e-12);
    }
}

#[test]
fn d65_white_and_neutral_gray_are_invariant_between_display_p3_and_srgb() {
    let p3 = ColorEncoding::Cicp([12, 13, 0, 1]);
    for encoded in [0.0, 0.5, 1.0] {
        let srgb =
            decode_to_linear_srgb(&ColorEncoding::Cicp([1, 13, 0, 1]), [encoded; 3]).unwrap();
        let transformed = decode_to_linear_srgb(&p3, [encoded; 3]).unwrap();
        for channel in 0..3 {
            near(transformed.rgb[channel], srgb.rgb[channel], 2.0e-12);
        }
    }
}

#[test]
fn display_p3_red_is_out_of_srgb_gamut_and_must_not_be_clipped() {
    let red = decode_to_linear_srgb(&ColorEncoding::Cicp([12, 13, 0, 1]), [1.0, 0.0, 0.0]).unwrap();
    near(red.rgb[0], 1.224_940_176_280_559_8, 1.0e-10);
    near(red.rgb[1], -0.042_056_954_709_688_3, 1.0e-10);
    near(red.rgb[2], -0.019_637_554_590_334, 1.0e-10);
    assert!(red.rgb[0] > 1.0 && red.rgb[1] < 0.0 && red.rgb[2] < 0.0);
}

#[test]
fn straight_alpha_is_applied_after_linearization_and_gamut_conversion() {
    let p3 = ColorEncoding::Cicp([12, 13, 0, 1]);
    let straight = decode_to_linear_srgb(&p3, [1.0, 0.0, 0.0]).unwrap();
    let premultiplied =
        decode_straight_to_premultiplied_linear_srgb(&p3, [1.0, 0.0, 0.0], 0.25).unwrap();
    near(premultiplied.alpha, 0.25, 0.0);
    for channel in 0..3 {
        near(
            premultiplied.rgb[channel],
            straight.rgb[channel] * 0.25,
            1.0e-14,
        );
    }
    let invisible =
        decode_straight_to_premultiplied_linear_srgb(&p3, [1.0, 0.0, 0.0], 0.0).unwrap();
    assert_eq!(invisible.rgb, [0.0; 3]);
    assert_eq!(invisible.alpha, 0.0);
}

#[test]
fn unknown_icc_dual_signalling_or_hdr_never_implicitly_becomes_srgb() {
    let unsupported = [
        ColorEncoding::Unspecified,
        ColorEncoding::Icc(vec![0; 132].into()),
        ColorEncoding::IccAndCicp {
            icc: vec![0; 132].into(),
            cicp: [1, 13, 0, 1],
        },
        ColorEncoding::Cicp([9, 16, 0, 1]),
        ColorEncoding::Cicp([9, 18, 0, 1]),
        ColorEncoding::Cicp([1, 13, 0, 0]),
        ColorEncoding::Cicp([1, 13, 1, 1]),
    ];
    for color in unsupported {
        assert_eq!(
            decode_to_linear_srgb(&color, [0.5; 3]),
            Err(Error::UnsupportedColorEncoding)
        );
    }
}

#[test]
fn nonfinite_samples_and_invalid_alpha_are_rejected() {
    let srgb = ColorEncoding::Cicp([1, 13, 0, 1]);
    for sample in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            decode_to_linear_srgb(&srgb, [sample, 0.5, 0.5]),
            Err(Error::NonFinitePixelSample)
        );
    }
    for alpha in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
        assert_eq!(
            decode_straight_to_premultiplied_linear_srgb(&srgb, [0.3; 3], alpha),
            Err(Error::InvalidAlphaValue)
        );
    }
}

#[test]
fn extended_negative_srgb_transfer_is_sign_preserving() {
    let srgb = ColorEncoding::Cicp([1, 13, 0, 1]);
    let transformed = decode_to_linear_srgb(&srgb, [-0.5, 0.0, 0.5]).unwrap();
    near(transformed.rgb[0], -0.214_041_140_482_232_55, 1.0e-12);
    near(transformed.rgb[1], 0.0, 0.0);
    near(transformed.rgb[2], 0.214_041_140_482_232_55, 1.0e-12);
}
