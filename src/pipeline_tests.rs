#![cfg(all(feature = "jpeg-read", feature = "avif-write"))]
use crate::pipeline::{ImageFormat, InputProfilePolicy, ResizeFilter, TransformOptions, transcode};

#[test]
fn real_jpeg_reaches_valid_avif_bytes_through_m1_graph() {
    let encoded = include_bytes!("../tests/fixtures/sample.jpg");
    let options = TransformOptions {
        output_width: 3,
        output_height: 3,
        filter: ResizeFilter::Area,
        profile_policy: InputProfilePolicy::AssumeSrgbIfMissing,
        quality: 85.0,
        speed: 9,
        max_managed_bytes: 128 * 1024,
        ..TransformOptions::default()
    };
    let image = transcode(encoded, ImageFormat::Jpeg, options).unwrap();
    assert!(image.avif_bytes.len() > 40);
    assert_eq!(&image.avif_bytes[4..8], b"ftyp");
    assert_eq!(image.report.decoder_requests, 1);
    assert_eq!(image.report.decoder_full_frame_buffers, 1);
    assert_eq!(image.report.engine_full_frame_intermediates, 0);
    assert_eq!(image.report.encoder_output_frame_bytes, 3 * 3 * 4);
}

#[test]
fn missing_color_information_requires_an_explicit_override() {
    let encoded = include_bytes!("../tests/fixtures/sample.jpg");
    let options = TransformOptions {
        output_width: 2,
        output_height: 2,
        profile_policy: InputProfilePolicy::Strict,
        ..TransformOptions::default()
    };
    assert!(transcode(encoded, ImageFormat::Jpeg, options).is_err());
}

#[cfg(feature = "avif-read")]
#[test]
fn produced_avif_is_accepted_by_an_independent_decoder() {
    use crate::codecs::avif::AvifSource;
    use crate::decoder::DecodeSource;

    let options = TransformOptions {
        output_width: 4,
        output_height: 3,
        profile_policy: InputProfilePolicy::AssumeSrgbIfMissing,
        speed: 9,
        ..TransformOptions::default()
    };
    let encoded = transcode(
        include_bytes!("../tests/fixtures/sample.jpg"),
        ImageFormat::Jpeg,
        options,
    )
    .unwrap();
    let decoded = AvifSource::new(&encoded.avif_bytes).unwrap();
    assert_eq!(
        decoded.info().size,
        crate::geometry::Size::new(4, 3).unwrap()
    );
    assert_eq!(decoded.info().layout.channels(), 3);
}


#[cfg(all(feature = "jpeg-xl-read", feature = "icc", feature = "avif-write"))]
#[test]
fn jpeg_xl_icc_can_be_explicitly_transformed_and_encoded() {
    let opts = TransformOptions {
        output_width: 3,
        output_height: 2,
        profile_policy: InputProfilePolicy::UseRenderedIcc,
        speed: 9,
        max_managed_bytes: 128 * 1024,
        ..TransformOptions::default()
    };
    let image = transcode(
        include_bytes!("../tests/fixtures/sample.jxl"),
        ImageFormat::JpegXl,
        opts,
    ).unwrap();
    assert_eq!(&image.avif_bytes[4..8], b"ftyp");
    assert_eq!(image.report.decoder_requests, 1);
    assert_eq!(image.report.engine_full_frame_intermediates, 0);
}
