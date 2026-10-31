//! Codec-to-planner contract tests with tiny committed real bitstreams.
use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
    execute_decoded,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::Graph;
use crate::memory::MemoryBudget;

fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}

struct Probe {
    info: DecoderInfo,
    requested: Option<Rect>,
}
impl Probe {
    fn new() -> Self {
        Self {
            info: DecoderInfo {
                size: Size::new(10, 8).unwrap(),
                layout: ChannelLayout::Rgb,
                sample: SampleType::U8,
                access: DecodeAccess::CodecRegionRequest,
                rendered_icc: None,
                rendered_cicp: None,
            },
            requested: None,
        }
    }
}
impl DecodeSource for Probe {
    fn info(&self) -> &DecoderInfo {
        &self.info
    }
    fn read_region(
        &mut self,
        region: Rect,
        _: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error> {
        self.requested = Some(region);
        let row = vec![123_u8; region.width as usize * 3];
        for y in region.y..region.bottom() {
            sink(y, DecodedRow::U8(&row))?;
        }
        Ok(DecodeReport {
            requested: region,
            access: DecodeAccess::CodecRegionRequest,
            rows: u64::from(region.height),
            output_sample_bytes: u64::from(region.width) * u64::from(region.height) * 3,
            known_full_frame_buffers: 0,
            peak_engine_managed_bytes: 0,
            opaque_codec_memory_unbounded: true,
        })
    }
}

#[test]
fn nested_crops_request_only_needed_source_pixels() {
    let mut source = Probe::new();
    let mut graph = Graph::new();
    let id = graph
        .decoded_source(source.info.size, source.info.pixel_format())
        .unwrap();
    let cropped = graph.crop(id, rect(2, 1, 7, 6)).unwrap();
    let cropped = graph.crop(cropped, rect(1, 2, 4, 3)).unwrap();
    let mut ys = Vec::new();
    let report = execute_decoded(
        &graph,
        cropped,
        id,
        &mut source,
        rect(1, 1, 2, 2),
        &MemoryBudget::new(0),
        |y, row| {
            ys.push(y);
            assert!(matches!(row, DecodedRow::U8(data) if data == [123; 6]));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(source.requested, Some(rect(4, 4, 2, 2)));
    assert_eq!(ys, vec![1, 2]);
    assert_eq!(report.known_full_frame_buffers, 0);
}

#[test]
fn source_binding_and_cancellation_are_checked() {
    let mut source = Probe::new();
    let mut graph = Graph::new();
    let id = graph
        .decoded_source(source.info.size, source.info.pixel_format())
        .unwrap();
    let wrong = graph
        .decoded_source(Size::new(2, 2).unwrap(), source.info.pixel_format())
        .unwrap();
    assert_eq!(
        execute_decoded(
            &graph,
            id,
            wrong,
            &mut source,
            rect(0, 0, 1, 1),
            &MemoryBudget::new(0),
            |_, _| Ok(())
        ),
        Err(Error::SourceBindingMismatch)
    );
    let mut called = 0;
    assert_eq!(
        execute_decoded(
            &graph,
            id,
            id,
            &mut source,
            rect(0, 0, 1, 2),
            &MemoryBudget::new(0),
            |_, _| {
                called += 1;
                Err(Error::Cancelled)
            }
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(called, 1);
}

#[cfg(feature = "jpeg-read")]
#[test]
fn jpeg_read_only_fallback_is_explicit_and_budgeted() {
    use crate::codecs::jpeg::JpegSource;
    let bytes = include_bytes!("../tests/fixtures/sample.jpg");
    let mut source = JpegSource::new(bytes).unwrap();
    assert_eq!(source.info().size, Size::new(7, 5).unwrap());
    assert_eq!(source.info().layout, ChannelLayout::Rgb);
    assert_eq!(source.info().access, DecodeAccess::FullFrameFallback);
    let mut graph = Graph::new();
    let id = graph
        .decoded_source(source.info().size, source.info().pixel_format())
        .unwrap();
    let crop = graph.crop(id, rect(2, 1, 4, 3)).unwrap();
    let mut ys = Vec::new();
    let budget = MemoryBudget::new(105);
    let report = execute_decoded(
        &graph,
        crop,
        id,
        &mut source,
        rect(1, 1, 2, 2),
        &budget,
        |y, pixels| {
            assert!(matches!(pixels, DecodedRow::U8(row) if row.len() == 6));
            ys.push(y);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(ys, vec![1, 2]);
    assert_eq!(report.requested, rect(3, 2, 2, 2));
    assert_eq!(report.known_full_frame_buffers, 1);
    assert_eq!(report.output_sample_bytes, 12);
    assert_eq!(budget.snapshot().live, 0);
    assert_eq!(budget.snapshot().peak, 105);
    assert_eq!(
        source.read_region(
            rect(3, 2, 2, 2),
            &MemoryBudget::new(104),
            &mut |_, _| Ok(())
        ),
        Err(Error::BudgetExceeded {
            requested: 105,
            available: 104
        })
    );
    assert!(matches!(
        JpegSource::new(b"invalid"),
        Err(Error::DecodeFailed)
    ));
}

#[cfg(feature = "jpeg-xl-read")]
#[test]
fn jpeg_xl_requests_codec_region_and_delivers_float_scanlines() {
    use crate::codecs::jxl::JxlSource;
    let bytes = include_bytes!("../tests/fixtures/sample.jxl");
    let mut source = JxlSource::new(bytes).unwrap();
    assert_eq!(source.info().size, Size::new(7, 5).unwrap());
    assert_eq!(source.info().layout, ChannelLayout::Rgb);
    assert_eq!(source.info().sample, SampleType::F32);
    assert_eq!(source.info().access, DecodeAccess::CodecRegionRequest);
    let mut graph = Graph::new();
    let id = graph
        .decoded_source(source.info().size, source.info().pixel_format())
        .unwrap();
    let crop = graph.crop(id, rect(1, 1, 5, 3)).unwrap();
    let budget = MemoryBudget::new(24);
    let mut ys = Vec::new();
    let report = execute_decoded(
        &graph,
        crop,
        id,
        &mut source,
        rect(1, 1, 2, 2),
        &budget,
        |y, row| {
            assert!(matches!(row, DecodedRow::F32(samples) if samples.len() == 6));
            ys.push(y);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(ys, vec![1, 2]);
    assert_eq!(report.requested, rect(2, 2, 2, 2));
    assert_eq!(report.known_full_frame_buffers, 0);
    assert_eq!(report.output_sample_bytes, 48);
    assert_eq!(budget.snapshot().live, 0);
    assert_eq!(budget.snapshot().peak, 24);
    assert!(matches!(
        JxlSource::new(b"invalid"),
        Err(Error::DecodeFailed)
    ));
}
