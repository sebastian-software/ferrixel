//! Bounded deterministic malformed-input smoke corpus. These are not a
//! replacement for libFuzzer/ASan or a full security review.

#[cfg(any(feature = "jpeg-read", feature = "jpeg-xl-read", feature = "avif-read"))]
use std::panic::{AssertUnwindSafe, catch_unwind};

#[cfg(feature = "jpeg-read")]
#[test]
fn truncated_and_mutated_jpeg_headers_never_panic_in_adapter_setup() {
    use crate::codecs::jpeg::JpegSource;

    let complete = include_bytes!("../tests/fixtures/sample.jpg");
    for length in [0, 1, 2, 4, 8, 16, complete.len() / 2] {
        let data = &complete[..length.min(complete.len())];
        let result = catch_unwind(AssertUnwindSafe(|| JpegSource::new(data)));
        assert!(
            result.is_ok(),
            "JPEG header parser panicked at length {length}"
        );
    }
    let mut invalid = complete.to_vec();
    invalid[0] = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| JpegSource::new(&invalid))).is_ok());
}

#[cfg(feature = "jpeg-xl-read")]
#[test]
fn truncated_and_mutated_jxl_headers_never_panic_in_adapter_setup() {
    use crate::codecs::jxl::JxlSource;

    let complete = include_bytes!("../tests/fixtures/sample.jxl");
    for length in [0, 1, 2, 4, 8, 16, complete.len() / 2] {
        let data = &complete[..length.min(complete.len())];
        let result = catch_unwind(AssertUnwindSafe(|| JxlSource::new(data)));
        assert!(
            result.is_ok(),
            "JXL header parser panicked at length {length}"
        );
    }
    let mut invalid = complete.to_vec();
    invalid[0] ^= 0xff;
    assert!(catch_unwind(AssertUnwindSafe(|| JxlSource::new(&invalid))).is_ok());
}

#[cfg(all(feature = "avif-read", feature = "avif-write"))]
#[test]
fn avif_decoder_rejects_truncated_container_without_panicking() {
    use crate::avif_sink::{AvifSink, EncodeSettings};
    use crate::codecs::avif::AvifSource;
    use crate::geometry::Size;
    use crate::memory::MemoryBudget;

    let budget = MemoryBudget::new(4 * 4 * 4);
    let mut sink = AvifSink::new(Size::new(4, 4).unwrap(), &budget).unwrap();
    for row in 0..4 {
        sink.write_row(row, &[40, 60, 100, 255].repeat(4)).unwrap();
    }
    let file = sink
        .finish(EncodeSettings {
            quality: 80.0,
            speed: 10,
        })
        .unwrap()
        .bytes;
    for length in [0, 1, 2, 4, 8, 16, file.len() / 2] {
        let truncated = &file[..length.min(file.len())];
        let result = catch_unwind(AssertUnwindSafe(|| AvifSource::new(truncated)));
        assert!(result.is_ok(), "AVIF adapter panicked at length {length}");
        assert!(
            result.unwrap().is_err(),
            "truncated container unexpectedly accepted"
        );
    }
    assert_eq!(budget.snapshot().live, 0);

    assert!(AvifSource::new_bounded(&file, 1).is_err());
    assert!(AvifSource::new_bounded(&file, 16).is_ok());
}

#[cfg(all(feature = "jpeg-read", feature = "avif-write"))]
#[test]
fn api_rejects_invalid_quality_and_mismatched_input_format() {
    use crate::pipeline::{ImageFormat, InputProfilePolicy, TransformOptions, transcode};
    let jpeg = include_bytes!("../tests/fixtures/sample.jpg");
    let options = TransformOptions {
        output_width: 2,
        output_height: 2,
        profile_policy: InputProfilePolicy::AssumeSrgbIfMissing,
        ..TransformOptions::default()
    };
    assert!(transcode(jpeg, ImageFormat::Avif, options).is_err());
    assert!(transcode(&[0; 32], ImageFormat::Jpeg, options).is_err());
    for quality in [f32::NAN, f32::INFINITY, 0.0, 101.0] {
        assert!(
            transcode(
                jpeg,
                ImageFormat::Jpeg,
                TransformOptions { quality, ..options },
            )
            .is_err()
        );
    }
    assert!(
        transcode(
            jpeg,
            ImageFormat::Jpeg,
            TransformOptions {
                output_width: 0,
                ..options
            },
        )
        .is_err()
    );
    assert!(
        transcode(
            jpeg,
            ImageFormat::Jpeg,
            TransformOptions {
                max_managed_bytes: 1,
                ..options
            },
        )
        .is_err()
    );
}
