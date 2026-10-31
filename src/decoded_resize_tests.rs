//! Differential and contract tests for encoded-sample-domain resize.

use crate::color::{AlphaMode, ColorEncoding};
use crate::decoded_resize::execute_decoded_resize;
use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::{Graph, NodeId};
use crate::memory::MemoryBudget;
use crate::plan::sample_at;

fn size(w: u32, h: u32) -> Size {
    Size::new(w, h).unwrap()
}
fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn sample_u8(x: u32, y: u32, channel: u32) -> u8 {
    u8::try_from((x * 23 + y * 19 + channel * 43) % 256).unwrap()
}
fn sample_f32(x: u32, y: u32, channel: u32) -> f32 {
    let x = f32::from(u16::try_from(x).unwrap());
    let y = f32::from(u16::try_from(y).unwrap());
    let channel = f32::from(u16::try_from(channel).unwrap());
    x.mul_add(0.3, y.mul_add(0.2, channel.mul_add(0.1, -1.5)))
}
struct PatternSource {
    info: DecoderInfo,
    requests: Vec<Rect>,
}
impl PatternSource {
    fn new(size: Size, sample: SampleType, layout: ChannelLayout) -> Self {
        Self {
            info: DecoderInfo {
                size,
                sample,
                layout,
                access: DecodeAccess::CodecRegionRequest,
                alpha: match layout {
                    ChannelLayout::GrayAlpha | ChannelLayout::Rgba | ChannelLayout::Cmyka => {
                        AlphaMode::Unspecified
                    }
                    ChannelLayout::Gray | ChannelLayout::Rgb | ChannelLayout::Cmyk => {
                        AlphaMode::Opaque
                    }
                },
                rendered_icc: None,
                rendered_cicp: None,
            },
            requests: Vec::new(),
        }
    }
}
impl DecodeSource for PatternSource {
    fn info(&self) -> &DecoderInfo {
        &self.info
    }
    fn read_region(
        &mut self,
        region: Rect,
        _: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error> {
        assert!(region.fits(self.info.size));
        self.requests.push(region);
        let channels = usize::from(self.info.layout.channels());
        for y in region.y..region.bottom() {
            match self.info.sample {
                SampleType::U8 => {
                    let mut row = Vec::new();
                    for x in region.x..region.right() {
                        for channel in 0..channels {
                            row.push(sample_u8(x, y, u32::try_from(channel).unwrap()));
                        }
                    }
                    sink(y, DecodedRow::U8(&row))?;
                }
                SampleType::F32 => {
                    let mut row = Vec::new();
                    for x in region.x..region.right() {
                        for channel in 0..channels {
                            row.push(sample_f32(x, y, u32::try_from(channel).unwrap()));
                        }
                    }
                    sink(y, DecodedRow::F32(&row))?;
                }
            }
        }
        let sample_bytes = match self.info.sample {
            SampleType::U8 => 1,
            SampleType::F32 => 4,
        };
        Ok(DecodeReport {
            requested: region,
            access: self.info.access,
            rows: u64::from(region.height),
            output_sample_bytes: u64::from(region.width)
                * u64::from(region.height)
                * u64::try_from(channels * sample_bytes).unwrap(),
            known_full_frame_buffers: 0,
            peak_engine_managed_bytes: 0,
            opaque_codec_memory_unbounded: false,
        })
    }
}
fn bind(graph: &mut Graph, source: &dyn DecodeSource) -> NodeId {
    let info = source.info();
    graph
        .decoded_source(info.size, info.pixel_format(), info.semantics())
        .unwrap()
}

#[test]
fn rgb8_crop_resize_crop_matches_independent_reference() {
    let mut source = PatternSource::new(size(16, 12), SampleType::U8, ChannelLayout::Rgb);
    source.info.rendered_icc = Some(vec![3, 1, 4, 1, 5]);
    source.info.rendered_cicp = Some([1, 13, 0, 1]);
    let mut graph = Graph::new();
    let id = bind(&mut graph, &source);
    let crop = graph.crop(id, rect(2, 1, 10, 8)).unwrap();
    let resized = graph.resize_encoded_samples(crop, size(7, 6)).unwrap();
    let crop = graph.crop(resized, rect(1, 2, 5, 3)).unwrap();
    assert!(matches!(
        graph.node(crop).unwrap().semantics.color,
        ColorEncoding::IccAndCicp { .. }
    ));
    let budget = MemoryBudget::new(250);
    let mut output = Vec::new();
    let report = execute_decoded_resize(
        &graph,
        crop,
        id,
        &mut source,
        rect(1, 0, 3, 3),
        &budget,
        |_, row| {
            let DecodedRow::U8(samples) = row else {
                panic!("expected RGB8");
            };
            output.extend_from_slice(samples);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(source.requests.len(), 1);
    assert_eq!(report.decoder_region_requests, 1);
    assert_eq!(report.full_frame_intermediates, 0);
    assert_eq!(report.output_rows, 3);
    assert_eq!(report.output_sample_bytes, 27);
    assert_eq!(budget.snapshot().live, 0);
    assert!(report.peak_engine_managed_bytes <= 250);
    for y in 0..3_u32 {
        let sy = sample_at(y + 2, 8, 6);
        for x in 0..3_u32 {
            let sx = sample_at(x + 2, 10, 7);
            for channel in 0..3_u32 {
                let tl = f64::from(sample_u8(sx.lower + 2, sy.lower + 1, channel));
                let tr = f64::from(sample_u8(sx.upper + 2, sy.lower + 1, channel));
                let bl = f64::from(sample_u8(sx.lower + 2, sy.upper + 1, channel));
                let br = f64::from(sample_u8(sx.upper + 2, sy.upper + 1, channel));
                let top = tl + (tr - tl) * sx.upper_weight;
                let bottom = bl + (br - bl) * sx.upper_weight;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let expected = (top + (bottom - top) * sy.upper_weight).round() as u8;
                let idx = usize::try_from((y * 3 + x) * 3 + channel).unwrap();
                assert_eq!(output[idx], expected);
            }
        }
    }
}

#[test]
fn f32_resize_preserves_negative_and_hdr_sample_values() {
    let mut source = PatternSource::new(size(7, 5), SampleType::F32, ChannelLayout::Rgb);
    source.info.rendered_cicp = Some([9, 16, 9, 1]);
    let mut graph = Graph::new();
    let id = bind(&mut graph, &source);
    let resized = graph.resize_encoded_samples(id, size(4, 9)).unwrap();
    let mut actual = Vec::new();
    let report = execute_decoded_resize(
        &graph,
        resized,
        id,
        &mut source,
        rect(0, 0, 4, 9),
        &MemoryBudget::new(300),
        |_, row| {
            let DecodedRow::F32(data) = row else {
                panic!("expected f32");
            };
            actual.extend_from_slice(data);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(report.output_rows, 9);
    assert_eq!(source.requests.len(), 1);
    assert_eq!(
        graph.node(resized).unwrap().semantics.color,
        ColorEncoding::Cicp([9, 16, 9, 1])
    );
    for y in 0..9 {
        let sy = sample_at(y, 5, 9);
        for x in 0..4 {
            let sx = sample_at(x, 7, 4);
            for channel in 0..3 {
                let tl = f64::from(sample_f32(sx.lower, sy.lower, channel));
                let tr = f64::from(sample_f32(sx.upper, sy.lower, channel));
                let bl = f64::from(sample_f32(sx.lower, sy.upper, channel));
                let br = f64::from(sample_f32(sx.upper, sy.upper, channel));
                let top = tl + (tr - tl) * sx.upper_weight;
                let bottom = bl + (br - bl) * sx.upper_weight;
                let expected = top + (bottom - top) * sy.upper_weight;
                let idx = usize::try_from((y * 4 + x) * 3 + channel).unwrap();
                assert!((f64::from(actual[idx]) - expected).abs() < 1.0e-5);
            }
        }
    }
    assert!(actual.iter().any(|v| *v < 0.0));
    assert!(actual.iter().any(|v| *v > 1.0));
}

#[test]
fn metadata_mismatch_is_detected_before_decode() {
    let mut source = PatternSource::new(size(8, 6), SampleType::U8, ChannelLayout::Rgb);
    source.info.rendered_icc = Some(vec![1, 2, 3]);
    let mut graph = Graph::new();
    let id = bind(&mut graph, &source);
    let resized = graph.resize_encoded_samples(id, size(4, 3)).unwrap();
    source.info.rendered_icc = Some(vec![4, 5, 6]);
    assert_eq!(
        execute_decoded_resize(
            &graph,
            resized,
            id,
            &mut source,
            rect(0, 0, 4, 3),
            &MemoryBudget::new(300),
            |_, _| Ok(())
        ),
        Err(Error::SourceBindingMismatch)
    );
    assert_eq!(source.requests.len(), 0);
}

#[test]
fn alpha_and_cmyk_are_not_accidentally_resized() {
    let mut graph = Graph::new();
    for layout in [
        ChannelLayout::Rgba,
        ChannelLayout::GrayAlpha,
        ChannelLayout::Cmyk,
    ] {
        let source = PatternSource::new(size(4, 4), SampleType::U8, layout);
        let id = bind(&mut graph, &source);
        assert_eq!(
            graph.resize_encoded_samples(id, size(2, 2)),
            Err(Error::UnsupportedPixelOperation)
        );
        if layout == ChannelLayout::Rgba {
            assert_eq!(
                graph.node(id).unwrap().semantics.alpha,
                AlphaMode::Unspecified
            );
        }
    }
}

#[test]
fn memory_budget_and_cancellation_are_checked() {
    let mut source = PatternSource::new(size(9, 7), SampleType::U8, ChannelLayout::Rgb);
    let mut graph = Graph::new();
    let id = bind(&mut graph, &source);
    let resized = graph.resize_encoded_samples(id, size(4, 3)).unwrap();
    let small = MemoryBudget::new(33);
    assert!(matches!(
        execute_decoded_resize(
            &graph,
            resized,
            id,
            &mut source,
            rect(0, 0, 4, 3),
            &small,
            |_, _| Ok(())
        ),
        Err(Error::BudgetExceeded { .. })
    ));
    assert_eq!(small.snapshot().live, 0);
    assert_eq!(source.requests.len(), 0);
    let budget = MemoryBudget::new(60);
    let mut calls = 0;
    assert_eq!(
        execute_decoded_resize(
            &graph,
            resized,
            id,
            &mut source,
            rect(0, 0, 4, 3),
            &budget,
            |_, _| {
                calls += 1;
                Err(Error::Cancelled)
            }
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(calls, 1);
    assert_eq!(budget.snapshot().live, 0);
}

#[cfg(feature = "jpeg-read")]
#[test]
fn real_jpeg_full_frame_fallback_is_decoded_once_for_resize() {
    use crate::codecs::jpeg::JpegSource;
    let mut source = JpegSource::new(include_bytes!("../tests/fixtures/sample.jpg")).unwrap();
    let mut graph = Graph::new();
    let id = bind(&mut graph, &source);
    let resized = graph.resize_encoded_samples(id, size(3, 3)).unwrap();
    let budget = MemoryBudget::new(180);
    let mut pixels = Vec::new();
    let report = execute_decoded_resize(
        &graph,
        resized,
        id,
        &mut source,
        rect(0, 0, 3, 3),
        &budget,
        |_, row| {
            let DecodedRow::U8(data) = row else {
                panic!("expected RGB8");
            };
            pixels.extend_from_slice(data);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(report.decoder.access, DecodeAccess::FullFrameFallback);
    assert_eq!(report.decoder.known_full_frame_buffers, 1);
    assert_eq!(report.decoder_region_requests, 1);
    assert_eq!(pixels.len(), 27);
    assert_eq!(budget.snapshot().live, 0);
}

#[cfg(feature = "jpeg-xl-read")]
#[test]
fn real_jxl_float_roi_flows_through_resize() {
    use crate::codecs::jxl::JxlSource;
    let mut source = JxlSource::new(include_bytes!("../tests/fixtures/sample.jxl")).unwrap();
    let mut graph = Graph::new();
    let id = bind(&mut graph, &source);
    let resized = graph.resize_encoded_samples(id, size(4, 3)).unwrap();
    let mut samples = 0;
    let report = execute_decoded_resize(
        &graph,
        resized,
        id,
        &mut source,
        rect(1, 0, 2, 3),
        &MemoryBudget::new(200),
        |_, row| {
            let DecodedRow::F32(data) = row else {
                panic!("expected f32");
            };
            samples += data.len();
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(report.decoder.access, DecodeAccess::CodecRegionRequest);
    assert_eq!(report.decoder_region_requests, 1);
    assert_eq!(samples, 18);
}
