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
