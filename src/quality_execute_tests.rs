//! End-to-end streaming reference regression tests. No codec required.

use crate::color::AlphaMode;
use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::Graph;
use crate::memory::MemoryBudget;
use crate::quality::Filter;
use crate::quality_execute::execute_quality_srgb8;

struct RgbSource {
    info: DecoderInfo,
    bytes: Vec<u8>,
    calls: usize,
    regions: Vec<Rect>,
}
impl RgbSource {
    fn new(size: Size, layout: ChannelLayout, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            size.width as usize * size.height as usize * usize::from(layout.channels())
        );
        Self {
            info: DecoderInfo {
                size,
                layout,
                sample: SampleType::U8,
                access: DecodeAccess::CodecRegionRequest,
                alpha: if layout == ChannelLayout::Rgba {
                    AlphaMode::Straight
                } else {
                    AlphaMode::Opaque
                },
                rendered_icc: None,
                rendered_cicp: Some([1, 13, 0, 1]),
            },
            bytes: pixels,
            calls: 0,
            regions: Vec::new(),
        }
    }
}
impl DecodeSource for RgbSource {
    fn info(&self) -> &DecoderInfo {
        &self.info
    }
    fn read_region(
        &mut self,
        region: Rect,
        _budget: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error> {
        self.calls += 1;
        self.regions.push(region);
        let channel = usize::from(self.info.layout.channels());
        let stride = self.info.size.width as usize * channel;
        let width = region.width as usize * channel;
        for y in region.y..region.bottom() {
            let start = y as usize * stride + region.x as usize * channel;
            sink(y, DecodedRow::U8(&self.bytes[start..start + width]))?;
        }
        Ok(DecodeReport {
            requested: region,
            access: DecodeAccess::CodecRegionRequest,
            rows: u64::from(region.height),
            output_sample_bytes: u64::from(region.width)
                * u64::from(region.height)
                * channel as u64,
            known_full_frame_buffers: 0,
            peak_engine_managed_bytes: 0,
            opaque_codec_memory_unbounded: false,
        })
    }
}

fn make_graph(
    source: &RgbSource,
    output: Size,
    filter: Filter,
) -> (Graph, crate::graph::NodeId, crate::graph::NodeId) {
    let mut graph = Graph::new();
    let id = graph
        .decoded_source(
            source.info.size,
            source.info.pixel_format(),
            source.info.semantics(),
        )
        .unwrap();
    let linear = graph.add_linear_srgb(id).unwrap();
    let resized = graph.resize_working(linear, output, filter).unwrap();
    let finished = graph.add_srgb8(resized).unwrap();
    (graph, id, finished)
}

#[test]
fn transparent_edges_are_filtered_after_linear_premultiplication() {
    let mut source = RgbSource::new(
        Size::new(2, 1).unwrap(),
        ChannelLayout::Rgba,
        vec![255, 0, 0, 255, 0, 0, 255, 0],
    );
    let (graph, id, output) = make_graph(&source, Size::new(1, 1).unwrap(), Filter::Area);
    let budget = MemoryBudget::new(128);
    let mut pixels = Vec::new();
    let report = execute_quality_srgb8(
        &graph,
        output,
        id,
        &mut source,
        Rect::new(0, 0, 1, 1).unwrap(),
        &budget,
        |_, line| {
            pixels.extend_from_slice(line);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(pixels, [255, 0, 0, 128]);
    assert_eq!(source.calls, 1);
    assert_eq!(report.decoder_requests, 1);
    assert_eq!(report.explicit_full_frame_intermediates, 0);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn area_downscale_black_white_checkerboard_is_linear_mid_gray() {
    let mut rgb = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            let value = if (x + y) % 2 == 0 { 0 } else { 255 };
            rgb.extend([value; 3]);
        }
    }
    let mut source = RgbSource::new(Size::new(4, 4).unwrap(), ChannelLayout::Rgb, rgb);
    let (graph, id, out) = make_graph(&source, Size::new(1, 1).unwrap(), Filter::Area);
    let budget = MemoryBudget::new(256);
    let mut result = Vec::new();
    let report = execute_quality_srgb8(
        &graph,
        out,
        id,
        &mut source,
        Rect::new(0, 0, 1, 1).unwrap(),
        &budget,
        |_, data| {
            result.extend_from_slice(data);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result, [188, 188, 188, 255]);
    assert_eq!(report.output_rows, 1);
    assert_eq!(source.calls, 1);
    assert_eq!(budget.snapshot().live, 0);
    assert!(report.peak_engine_managed_bytes <= 256);
}

#[test]
fn failure_and_sink_cancellation_release_cached_rows() {
    let mut source = RgbSource::new(Size::new(3, 3).unwrap(), ChannelLayout::Rgb, vec![200; 27]);
    let (graph, id, out) = make_graph(&source, Size::new(2, 2).unwrap(), Filter::Lanczos3);
    let small = MemoryBudget::new(5);
    assert!(matches!(
        execute_quality_srgb8(
            &graph,
            out,
            id,
            &mut source,
            Rect::new(0, 0, 2, 2).unwrap(),
            &small,
            |_, _| Ok(())
        ),
        Err(Error::BudgetExceeded { .. })
    ));
    assert_eq!(small.snapshot().live, 0);
    let big = MemoryBudget::new(2048);
    let mut called = 0;
    assert_eq!(
        execute_quality_srgb8(
            &graph,
            out,
            id,
            &mut source,
            Rect::new(0, 0, 2, 2).unwrap(),
            &big,
            |_, _| {
                called += 1;
                Err(Error::Cancelled)
            }
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(called, 1);
    assert_eq!(big.snapshot().live, 0);
}

#[test]
fn explicit_color_nodes_preserve_correct_metadata() {
    let source = RgbSource::new(Size::new(4, 4).unwrap(), ChannelLayout::Rgb, vec![80; 48]);
    let (graph, id, output) = make_graph(&source, Size::new(2, 2).unwrap(), Filter::Lanczos3);
    assert_eq!(
        graph.node(id).unwrap().semantics.color,
        crate::color::ColorEncoding::Cicp([1, 13, 0, 1])
    );
    assert_eq!(
        graph.node(output).unwrap().semantics.color,
        crate::color::ColorEncoding::Cicp([1, 13, 0, 1])
    );
    assert_ne!(
        graph.node(id).unwrap().size,
        graph.node(output).unwrap().size
    );
}

#[test]
fn two_dimensional_area_reference_handles_mixed_opacity() {
    // Independent expected values obtained from area coverage, the IEC
    // 61966-2-1 transfer curve and linear-light premultiplied averaging.
    // Transparent blue cannot introduce a blue fringe.
    let mut source = RgbSource::new(
        Size::new(2, 2).unwrap(),
        ChannelLayout::Rgba,
        vec![
            255, 0, 0, 255, 0, 0, 255, 0,
            0, 255, 0, 128, 255, 255, 255, 64,
        ],
    );
    let (graph, input, output) = make_graph(&source, Size::new(1, 1).unwrap(), Filter::Area);
    let mut actual = Vec::new();
    let budget = MemoryBudget::new(512);
    execute_quality_srgb8(
        &graph,
        output,
        input,
        &mut source,
        Rect::new(0, 0, 1, 1).unwrap(),
        &budget,
        |_, row| {
            actual.extend_from_slice(row);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(actual, [220, 175, 106, 112]);
    assert_eq!(source.regions, [Rect::new(0, 0, 2, 2).unwrap()]);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn downstream_output_region_uses_crop_coordinates_not_source_origin() {
    // The requested output is the *right half* of a 3x2 input crop.
    // A direct 1x1 source request would not suffice; the correct Area
    // footprint includes two source columns and both input rows.
    let mut pixels = vec![0_u8; 5 * 4 * 3];
    for y in 1..3_usize {
        for (x, gray) in [(1_usize, 0_u8), (2, 128), (3, 255)] {
            let index = (y * 5 + x) * 3;
            pixels[index..index + 3].fill(gray);
        }
    }
    let mut source = RgbSource::new(Size::new(5, 4).unwrap(), ChannelLayout::Rgb, pixels);
    let mut graph = Graph::new();
    let input = graph
        .decoded_source(source.info.size, source.info.pixel_format(), source.info.semantics())
        .unwrap();
    let crop = graph.crop(input, Rect::new(1, 1, 3, 2).unwrap()).unwrap();
    let linear = graph.add_linear_srgb(crop).unwrap();
    let resized = graph.resize_working(linear, Size::new(2, 1).unwrap(), Filter::Area).unwrap();
    let output = graph.add_srgb8(resized).unwrap();
    let mut pixels = Vec::new();
    let budget = MemoryBudget::new(512);
    execute_quality_srgb8(
        &graph,
        output,
        input,
        &mut source,
        Rect::new(1, 0, 1, 1).unwrap(),
        &budget,
        |y, row| {
            assert_eq!(y, 0);
            pixels.extend_from_slice(row);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(pixels, [223, 223, 223, 255]);
    assert_eq!(source.regions, [Rect::new(2, 1, 2, 2).unwrap()]);
    assert_eq!(source.calls, 1);
    assert_eq!(budget.snapshot().live, 0);
}

#[test]
fn deterministic_resampler_property_matrix_preserves_flat_srgb() {
    for filter in [Filter::Area, Filter::Lanczos3] {
        for (width, height, new_width, new_height) in [
            (1, 1, 1, 1),
            (2, 3, 5, 7),
            (17, 5, 3, 2),
            (7, 19, 1, 1),
            (4, 4, 11, 3),
            (13, 2, 2, 12),
        ] {
            let constant = [78_u8, 159, 232];
            let image: Vec<u8> = constant.iter().copied().cycle()
                .take(width * height * 3).collect();
            let mut source = RgbSource::new(
                Size::new(width as u32, height as u32).unwrap(),
                ChannelLayout::Rgb,
                image,
            );
            let (graph, input, output) = make_graph(
                &source,
                Size::new(new_width as u32, new_height as u32).unwrap(),
                filter,
            );
            let budget = MemoryBudget::new(65_536);
            let mut delivered = 0_u32;
            let report = execute_quality_srgb8(
                &graph,
                output,
                input,
                &mut source,
                Rect::new(0, 0, new_width as u32, new_height as u32).unwrap(),
                &budget,
                |_, row| {
                    delivered += 1;
                    for pixel in row.as_chunks::<4>().0 {
                        for channel in 0..3 {
                            assert!(pixel[channel].abs_diff(constant[channel]) <= 1);
                        }
                        assert_eq!(pixel[3], 255);
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(delivered, new_height as u32);
            assert_eq!(source.calls, 1);
            assert_eq!(report.output_rows, u64::from(delivered));
            assert_eq!(budget.snapshot().live, 0);
        }
    }
}

#[test]
fn p3_reference_preserves_gamut_until_explicit_output_quantization() {
    let mut source = RgbSource::new(
        Size::new(1, 1).unwrap(),
        ChannelLayout::Rgb,
        vec![255, 0, 0],
    );
    source.info.rendered_cicp = Some([12, 13, 0, 1]);
    let (graph, id, output) = make_graph(&source, Size::new(1, 1).unwrap(), Filter::Area);
    let mut result = Vec::new();
    let budget = MemoryBudget::new(256);
    execute_quality_srgb8(
        &graph,
        output,
        id,
        &mut source,
        Rect::new(0, 0, 1, 1).unwrap(),
        &budget,
        |_, row| {
            result.extend_from_slice(row);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result, [255, 0, 0, 255]);
    assert_eq!(budget.snapshot().live, 0);
}
