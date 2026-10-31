#![cfg(feature = "avif-write")]
use crate::avif_sink::{AvifSink, EncodeSettings};
use crate::error::Error;
use crate::geometry::Size;
use crate::memory::MemoryBudget;

#[test]
fn sink_requires_complete_rows_and_releases_managed_frame_after_encoding() {
    let budget = MemoryBudget::new(4 * 4 * 4);
    let size = Size::new(4, 4).unwrap();
    let mut sink = AvifSink::new(size, &budget).unwrap();
    assert_eq!(sink.write_row(1, &[0; 16]), Err(Error::IncompleteOutput));
    for y in 0..4 {
        sink.write_row(
            y,
            &[
                80, 120, 200, 255, 80, 120, 200, 255, 80, 120, 200, 255, 80, 120, 200, 255,
            ],
        )
        .unwrap();
    }
    let encoded = sink
        .finish(EncodeSettings {
            quality: 75.0,
            speed: 9,
        })
        .unwrap();
    assert!(encoded.bytes.len() > 32);
    assert_eq!(&encoded.bytes[4..8], b"ftyp");
    assert_eq!(encoded.frame_materialization_bytes, 64);
    assert_eq!(encoded.encoded_bytes, encoded.bytes.len());
    assert!(encoded.opaque_encoder_memory_unbounded);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn invalid_options_and_incomplete_output_fail_closed() {
    let budget = MemoryBudget::new(64);
    let size = Size::new(4, 4).unwrap();
    assert!(matches!(
        AvifSink::new(size, &MemoryBudget::new(63)),
        Err(Error::BudgetExceeded { .. })
    ));
    let sink = AvifSink::new(size, &budget).unwrap();
    assert_eq!(
        sink.finish(EncodeSettings {
            quality: 75.0,
            speed: 9
        })
        .unwrap_err(),
        Error::IncompleteOutput
    );
    assert_eq!(budget.snapshot().live, 0);
    assert_eq!(
        (EncodeSettings {
            quality: f32::NAN,
            speed: 9
        })
        .validate(),
        Err(Error::InvalidEncodeOptions)
    );
    assert_eq!(
        (EncodeSettings {
            quality: 50.0,
            speed: 11
        })
        .validate(),
        Err(Error::InvalidEncodeOptions)
    );
}
