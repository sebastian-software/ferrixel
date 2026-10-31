//! End-to-end scalar reference tests for explicitly tagged linear sRGB.
//! No codec is needed: fixtures are raw output samples of a synthetic decoder.

use crate::color::AlphaMode;
use crate::decoded_resize::execute_decoded_resize;
use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::Graph;
use crate::memory::MemoryBudget;

const SRGB: [u8; 4] = [1, 13, 0, 1];
const PQ_BT2020: [u8; 4] = [9, 16, 0, 1];

fn size(width: u32, height: u32) -> Size {
    Size::new(width, height).unwrap()
}
fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
    Rect::new(x, y, width, height).unwrap()
}

struct RawSource {
    info: DecoderInfo,
    pixels: Vec<u8>,
    regions: Vec<Rect>,
}
impl RawSource {
    fn new(dimensions: Size, layout: ChannelLayout, alpha: AlphaMode, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            dimensions.width as usize * dimensions.height as usize * usize::from(layout.channels())
        );
        Self {
            info: DecoderInfo {
                size: dimensions,
                layout,
                alpha,
                sample: SampleType::U8,
                access: DecodeAccess::CodecRegionRequest,
                rendered_icc: None,
                rendered_cicp: Some(SRGB),
            },
            pixels,
            regions: Vec::new(),
        }
    }

    fn bind(&self, graph: &mut Graph) -> crate::graph::NodeId {
        graph
            .decoded_source(
                self.info.size,
                self.info.pixel_format(),
                self.info.semantics(),
            )
            .unwrap()
    }
}
impl DecodeSource for RawSource {
    fn info(&self) -> &DecoderInfo {
        &self.info
    }

    fn read_region(
        &mut self,
        region: Rect,
        _: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error> {
        self.regions.push(region);
        let channels = usize::from(self.info.layout.channels());
        let stride = self.info.size.width as usize * channels;
        let row_len = region.width as usize * channels;
        for y in region.y..region.bottom() {
            let start = y as usize * stride + region.x as usize * channels;
            sink(y, DecodedRow::U8(&self.pixels[start..start + row_len]))?;
        }
        Ok(DecodeReport {
            requested: region,
            access: self.info.access,
            rows: u64::from(region.height),
            output_sample_bytes: u64::from(region.width)
                * u64::from(region.height)
                * u64::from(self.info.layout.channels()),
            known_full_frame_buffers: 0,
            peak_engine_managed_bytes: 0,
            opaque_codec_memory_unbounded: false,
        })
    }
}

#[test]
fn black_white_midpoint_filters_in_linear_light() {
    let mut source = RawSource::new(
        size(2, 1),
        ChannelLayout::Rgb,
        AlphaMode::Opaque,
        vec![0, 0, 0, 255, 255, 255],
    );
    let mut graph = Graph::new();
    let input = source.bind(&mut graph);
    let linear = graph.resize_linear_srgb(input, size(1, 1)).unwrap();
    let legacy = graph.resize_encoded_samples(input, size(1, 1)).unwrap();
    let budget = MemoryBudget::new(12);
    let mut linear_pixels = Vec::new();
    let report = execute_decoded_resize(
        &graph,
        linear,
        input,
        &mut source,
        rect(0, 0, 1, 1),
        &budget,
        |_, row| {
            let DecodedRow::U8(bytes) = row else {
                panic!("RGB8 output required");
            };
            linear_pixels.extend_from_slice(bytes);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(linear_pixels, [188, 188, 188]);
    assert_eq!(source.regions, [rect(0, 0, 2, 1)]);
    assert_eq!(report.decoder_region_requests, 1);
    assert_eq!(report.full_frame_intermediates, 0);
    assert_eq!(budget.snapshot().live, 0);
    assert!(report.peak_engine_managed_bytes <= 12);

    let mut encoded_pixels = Vec::new();
    execute_decoded_resize(
        &graph,
        legacy,
        input,
        &mut source,
        rect(0, 0, 1, 1),
        &budget,
        |_, row| {
            let DecodedRow::U8(bytes) = row else {
                panic!("RGB8 output required");
            };
            encoded_pixels.extend_from_slice(bytes);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(encoded_pixels, [128, 128, 128]);
}

#[test]
fn straight_alpha_is_premultiplied_in_linear_light_before_filtering() {
    let mut source = RawSource::new(
        size(2, 1),
        ChannelLayout::Rgba,
        AlphaMode::Straight,
        vec![255, 0, 0, 255, 0, 0, 255, 0],
    );
    let mut graph = Graph::new();
    let input = source.bind(&mut graph);
    let resized = graph.resize_linear_srgb(input, size(1, 1)).unwrap();
    let mut out = Vec::new();
    execute_decoded_resize(
        &graph,
        resized,
        input,
        &mut source,
        rect(0, 0, 1, 1),
        &MemoryBudget::new(12),
        |_, row| {
            let DecodedRow::U8(data) = row else {
                panic!("RGBA8 output required");
            };
            out.extend_from_slice(data);
            Ok(())
        },
    )
    .unwrap();
    // The invisible blue sample must not bleed into the opaque red.
    // Alpha itself is linear and must not be sRGB transfer-encoded.
    assert_eq!(out, [255, 0, 0, 128]);
}

#[test]
fn all_transparent_pixels_yield_canonical_transparent_black() {
    let mut source = RawSource::new(
        size(2, 1),
        ChannelLayout::Rgba,
        AlphaMode::Straight,
        vec![255, 255, 0, 0, 0, 255, 255, 0],
    );
    let mut graph = Graph::new();
    let input = source.bind(&mut graph);
    let resized = graph.resize_linear_srgb(input, size(1, 1)).unwrap();
    let mut output = Vec::new();
    execute_decoded_resize(
        &graph,
        resized,
        input,
        &mut source,
        rect(0, 0, 1, 1),
        &MemoryBudget::new(12),
        |_, row| {
            let DecodedRow::U8(data) = row else {
                panic!("RGBA8 output required");
            };
            output.extend_from_slice(data);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(output, [0, 0, 0, 0]);
}

#[test]
fn crops_and_vertical_interpolation_preserve_alpha_and_color_semantics() {
    let mut source = RawSource::new(
        size(2, 2),
        ChannelLayout::Rgba,
        AlphaMode::Straight,
        vec![0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0],
    );
    let mut graph = Graph::new();
    let input = source.bind(&mut graph);
    let crop = graph.crop(input, rect(0, 0, 2, 2)).unwrap();
    let resized = graph.resize_linear_srgb(crop, size(1, 1)).unwrap();
    assert_eq!(
        graph.node(resized).unwrap().semantics,
        graph.node(input).unwrap().semantics
    );
    let mut output = Vec::new();
    let budget = MemoryBudget::new(20);
    let report = execute_decoded_resize(
        &graph,
        resized,
        input,
        &mut source,
        rect(0, 0, 1, 1),
        &budget,
        |_, row| {
            let DecodedRow::U8(data) = row else {
                panic!("RGBA8 output required");
            };
            output.extend_from_slice(data);
            Ok(())
        },
    )
    .unwrap();
    // Only the two opaque top pixels contribute: black and white, in
    // linear sRGB. The bottom (invisible) pixels do not contaminate RGB.
    assert_eq!(output, [188, 188, 188, 128]);
    assert_eq!(report.decoder_region_requests, 1);
    assert_eq!(budget.snapshot().live, 0);
    assert_eq!(source.regions, [rect(0, 0, 2, 2)]);
}

#[test]
fn missing_or_conflicting_profile_and_hdr_transfers_fail_closed() {
    let mut source = RawSource::new(
        size(2, 1),
        ChannelLayout::Rgb,
        AlphaMode::Opaque,
        vec![0, 0, 0, 255, 255, 255],
    );
    for (icc, cicp) in [
        (None, None),
        (Some(vec![1, 2, 3]), None),
        (Some(vec![1, 2, 3]), Some(SRGB)),
        (None, Some(PQ_BT2020)),
        (None, Some([1, 1, 0, 1])),
        (None, Some([1, 13, 1, 1])),
        (None, Some([1, 13, 0, 0])),
    ] {
        source.info.rendered_icc = icc;
        source.info.rendered_cicp = cicp;
        let mut graph = Graph::new();
        let input = source.bind(&mut graph);
        assert_eq!(
            graph.resize_linear_srgb(input, size(1, 1)),
            Err(Error::UnsupportedColorEncoding)
        );
    }
    assert!(source.regions.is_empty());
}

#[test]
fn alpha_must_have_explicit_straight_association() {
    for alpha in [
        AlphaMode::Unspecified,
        AlphaMode::Premultiplied,
        AlphaMode::Opaque,
    ] {
        let source = RawSource::new(size(2, 1), ChannelLayout::Rgba, alpha, vec![0; 8]);
        let mut graph = Graph::new();
        let input = source.bind(&mut graph);
        assert_eq!(
            graph.resize_linear_srgb(input, size(1, 1)),
            Err(Error::UnsupportedAlphaMode)
        );
    }
    let source = RawSource::new(
        size(2, 1),
        ChannelLayout::Rgb,
        AlphaMode::Straight,
        vec![0; 6],
    );
    let mut graph = Graph::new();
    let input = source.bind(&mut graph);
    assert_eq!(
        graph.resize_linear_srgb(input, size(1, 1)),
        Err(Error::UnsupportedAlphaMode)
    );
}

#[test]
fn unsupported_layouts_and_precision_are_not_silently_converted() {
    for layout in [
        ChannelLayout::Gray,
        ChannelLayout::GrayAlpha,
        ChannelLayout::Cmyk,
    ] {
        let source = RawSource::new(
            size(2, 1),
            layout,
            AlphaMode::Opaque,
            vec![0; usize::from(layout.channels()) * 2],
        );
        let mut graph = Graph::new();
        let input = source.bind(&mut graph);
        assert_eq!(
            graph.resize_linear_srgb(input, size(1, 1)),
            Err(Error::UnsupportedPixelOperation)
        );
    }
    // Even a forged alpha declaration and a nominal sRGB tag cannot make
    // CMYK's four channels semantically equivalent to RGBA.
    let source = RawSource::new(
        size(2, 1),
        ChannelLayout::Cmyk,
        AlphaMode::Straight,
        vec![0; 8],
    );
    let mut graph = Graph::new();
    let cmyk = source.bind(&mut graph);
    assert_eq!(
        graph.resize_linear_srgb(cmyk, size(1, 1)),
        Err(Error::UnsupportedPixelOperation)
    );

    let mut graph = Graph::new();
    let rgb = graph
        .decoded_source(
            size(2, 1),
            crate::surface::PixelFormat::InterleavedF32(ChannelLayout::Rgb),
            crate::color::ImageSemantics {
                color: crate::color::ColorEncoding::Cicp(SRGB),
                alpha: AlphaMode::Opaque,
            },
        )
        .unwrap();
    assert_eq!(
        graph.resize_linear_srgb(rgb, size(1, 1)),
        Err(Error::UnsupportedPixelOperation)
    );
}

#[test]
fn cancellation_and_budgets_release_all_managed_buffers() {
    let mut source = RawSource::new(
        size(2, 2),
        ChannelLayout::Rgba,
        AlphaMode::Straight,
        vec![255; 16],
    );
    let mut graph = Graph::new();
    let input = source.bind(&mut graph);
    let resized = graph.resize_linear_srgb(input, size(1, 1)).unwrap();
    let too_small = MemoryBudget::new(11);
    assert!(matches!(
        execute_decoded_resize(
            &graph,
            resized,
            input,
            &mut source,
            rect(0, 0, 1, 1),
            &too_small,
            |_, _| Ok(())
        ),
        Err(Error::BudgetExceeded { .. })
    ));
    assert_eq!(too_small.snapshot().live, 0);
    assert!(source.regions.is_empty());

    let budget = MemoryBudget::new(12);
    let mut called = 0;
    assert_eq!(
        execute_decoded_resize(
            &graph,
            resized,
            input,
            &mut source,
            rect(0, 0, 1, 1),
            &budget,
            |_, _| {
                called += 1;
                Err(Error::Cancelled)
            }
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(called, 1);
    assert_eq!(budget.snapshot().live, 0);
}
