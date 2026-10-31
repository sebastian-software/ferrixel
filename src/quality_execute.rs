//! M1 reference executor: decoder -> color transform -> antialiased
//! separable resampling -> explicit SDR output quantization.
//! A single source region is decoded only once, even for JPEG's full-frame
//! fallback. The reusable output buffer and cached *horizontal* source rows
//! are charged to the shared managed-memory budget. The final encoder may
//! separately require an entire output frame; this executor does not.

use std::collections::VecDeque;

use crate::color::{AlphaMode, ColorEncoding};
use crate::decoder::{ChannelLayout, DecodeReport, DecodeSource, DecodedRow, SampleType};
use crate::error::Error;
use crate::geometry::Rect;
use crate::graph::{Graph, NodeId, NodeKind, ResizeDomain};
use crate::memory::{ManagedBuffer, MemoryBudget};
use crate::quality::{Filter, Tap, taps};
use crate::surface::PixelFormat;
use crate::working_color::{
    decode_straight_to_premultiplied_linear_srgb, decode_to_linear_srgb, encode_linear_srgb8,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QualityReport {
    pub decoder: DecodeReport,
    pub output_rows: u64,
    pub decoder_requests: u32,
    pub peak_engine_managed_bytes: usize,
    pub explicit_full_frame_intermediates: u32,
}

struct CachedRow {
    y: u32,
    samples: ManagedBuffer<f64>,
}

#[derive(Clone, Copy)]
struct Prepared {
    source: Rect,
    footprint: Rect,
    resize: Rect,
    input: crate::geometry::Size,
    output: crate::geometry::Size,
    filter: Filter,
}

fn prepare(
    graph: &Graph,
    target: NodeId,
    source_node: NodeId,
    decoder: &dyn DecodeSource,
    request: Rect,
) -> Result<Prepared, Error> {
    let stages = crate::plan::plan_region(graph, target, request)?;
    let first = stages.first().ok_or(Error::UnknownNode)?;
    let source = graph.node(first.node)?;
    let info = decoder.info();
    if first.node != source_node
        || source.kind != NodeKind::DecoderSource
        || source.size != info.size
        || source.format != info.pixel_format()
        || source.semantics != info.semantics()
    {
        return Err(Error::SourceBindingMismatch);
    }
    if !matches!(info.layout, ChannelLayout::Rgb | ChannelLayout::Rgba) {
        return Err(Error::UnsupportedPixelOperation);
    }
    if !matches!(info.sample, SampleType::U8 | SampleType::F32) {
        return Err(Error::UnsupportedPixelOperation);
    }

    let mut linearized = false;
    let mut resized = None;
    let mut quantized = false;
    for (index, step) in stages.iter().enumerate().skip(1) {
        match graph.node(step.node)?.kind {
            NodeKind::Crop { .. } => {}
            NodeKind::ToLinearSrgb { .. } if !linearized && resized.is_none() => {
                linearized = true;
            }
            NodeKind::Resize { domain: ResizeDomain::WorkingLinear(filter), .. }
                if linearized && resized.is_none() =>
            {
                resized = Some((index, filter));
            }
            NodeKind::ToSrgb8 { .. } if resized.is_some() && !quantized => {
                quantized = true;
            }
            _ => return Err(Error::UnsupportedPixelOperation),
        }
    }
    let (index, filter) = resized.ok_or(Error::UnsupportedPixelOperation)?;
    if !linearized || !quantized {
        return Err(Error::UnsupportedPixelOperation);
    }
    if graph.node(target)?.format != PixelFormat::InterleavedU8(info.layout)
        || graph.node(target)?.semantics.color != ColorEncoding::Cicp([1, 13, 0, 1])
    {
        return Err(Error::PixelFormatMismatch);
    }
    let resize = stages[index];
    let foot = stages[index - 1];
    Ok(Prepared {
        source: first.region,
        footprint: foot.region,
        resize: resize.region,
        input: graph.node(foot.node)?.size,
        output: graph.node(resize.node)?.size,
        filter,
    })
}

fn sample_rgba(
    source: DecodedRow<'_>,
    pixel: usize,
    channels: usize,
    color: &ColorEncoding,
) -> Result<[f64; 4], Error> {
    let index = pixel.checked_mul(channels).ok_or(Error::GeometryOverflow)?;
    let component = |channel: usize| match source {
        DecodedRow::U8(row) => f64::from(row[index + channel]) / 255.0,
        DecodedRow::F32(row) => f64::from(row[index + channel]),
    };
    let rgb = [component(0), component(1), component(2)];
    if channels == 4 {
        let premultiplied =
            decode_straight_to_premultiplied_linear_srgb(color, rgb, component(3))?;
        Ok([
            premultiplied.rgb[0],
            premultiplied.rgb[1],
            premultiplied.rgb[2],
            premultiplied.alpha,
        ])
    } else {
        let linear = decode_to_linear_srgb(color, rgb)?;
        Ok([linear.rgb[0], linear.rgb[1], linear.rgb[2], 1.0])
    }
}

fn final_rgba(acc: [f64; 4], opaque: bool) -> Result<[u8; 4], Error> {
    if acc.iter().any(|sample| !sample.is_finite()) {
        return Err(Error::NonFinitePixelSample);
    }
    let alpha = if opaque { 1.0 } else { acc[3].clamp(0.0, 1.0) };
    let mut rgba = [0_u8; 4];
    if alpha > 1.0e-12 {
        for channel in 0..3 {
            rgba[channel] = encode_linear_srgb8(acc[channel] / alpha)?;
        }
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        rgba[3] = (alpha * 255.0).round() as u8;
    }
    Ok(rgba)
}

/// A scanline sink is borrowed and non-retaining. Cancelling the sink
/// propagates out immediately and releases all managed row reservations.
pub(crate) fn execute_quality_srgb8(
    graph: &Graph,
    target: NodeId,
    bound_source: NodeId,
    source: &mut dyn DecodeSource,
    requested: Rect,
    budget: &MemoryBudget,
    mut sink: impl FnMut(u32, &[u8]) -> Result<(), Error>,
) -> Result<QualityReport, Error> {
    let info = source.info().clone();
    let plan = prepare(graph, target, bound_source, source, requested)?;
    if info.layout == ChannelLayout::Rgba && info.alpha != AlphaMode::Straight {
        return Err(Error::UnsupportedAlphaMode);
    }
    if info.layout == ChannelLayout::Rgb && info.alpha != AlphaMode::Opaque {
        return Err(Error::UnsupportedAlphaMode);
    }
    let channels = usize::from(info.layout.channels());
    let output_width = usize::try_from(requested.width).map_err(|_| Error::GeometryOverflow)?;
    let width_components = output_width.checked_mul(4).ok_or(Error::GeometryOverflow)?;
    let source_width = usize::try_from(plan.source.width).map_err(|_| Error::GeometryOverflow)?;
    let source_components = source_width.checked_mul(channels).ok_or(Error::GeometryOverflow)?;

    let mut columns = Vec::new();
    for offset in 0..requested.width {
        let x = plan.resize.x.checked_add(offset).ok_or(Error::GeometryOverflow)?;
        columns.push(taps(plan.input.width, plan.output.width, x, plan.filter)?);
    }
    let mut lines = Vec::new();
    for offset in 0..requested.height {
        let y = plan.resize.y.checked_add(offset).ok_or(Error::GeometryOverflow)?;
        lines.push(taps(plan.input.height, plan.output.height, y, plan.filter)?);
    }
    let mut needed: Vec<u32> = lines.iter().flat_map(|row| row.iter().map(|tap| tap.index)).collect();
    needed.sort_unstable();
    needed.dedup();

    let mut output = budget.allocate_typed::<u8>(width_components)?;
    let mut cache: VecDeque<CachedRow> = VecDeque::new();
    let mut output_rows = 0_usize;
    let mut decoded_rows = 0_u32;
    let mut expected_y = plan.source.y;
    let report = source.read_region(plan.source, budget, &mut |decoder_y, row| {
        if decoder_y != expected_y || row.len() != source_components
            || row.sample_type() != info.sample
        {
            return Err(Error::IncompleteDecoderOutput);
        }
        decoded_rows += 1;
        expected_y = expected_y.checked_add(1).ok_or(Error::GeometryOverflow)?;
        let semantic_y = decoder_y
            .checked_sub(plan.source.y)
            .and_then(|n| n.checked_add(plan.footprint.y))
            .ok_or(Error::GeometryOverflow)?;
        if needed.binary_search(&semantic_y).is_ok() {
            let mut horizontal = budget.allocate_typed::<f64>(width_components)?;
            for (col, weights) in columns.iter().enumerate() {
                for tap in weights {
                    let source_x = usize::try_from(
                        tap.index.checked_sub(plan.footprint.x).ok_or(Error::OutOfBounds)?,
                    )
                    .map_err(|_| Error::GeometryOverflow)?;
                    let pixel = sample_rgba(row, source_x, channels, &info.semantics().color)?;
                    for channel in 0..4 {
                        horizontal.as_mut_slice()[col * 4 + channel] += pixel[channel] * tap.weight;
                    }
                }
            }
            cache.push_back(CachedRow {
                y: semantic_y,
                samples: horizontal,
            });
        }

        while output_rows < lines.len()
            && lines[output_rows].last().is_some_and(|tap| tap.index <= semantic_y)
        {
            let taps_y = &lines[output_rows];
            for col in 0..output_width {
                let mut acc = [0.0_f64; 4];
                for tap in taps_y {
                    let data = cache.iter()
                        .find(|row| row.y == tap.index)
                        .ok_or(Error::IncompleteDecoderOutput)?;
                    for channel in 0..4 {
                        acc[channel] += data.samples.as_slice()[col * 4 + channel] * tap.weight;
                    }
                }
                let pixel = final_rgba(acc, info.layout == ChannelLayout::Rgb)?;
                output.as_mut_slice()[col * 4..col * 4 + 4].copy_from_slice(&pixel);
            }
            let out_y = requested.y + u32::try_from(output_rows).map_err(|_| Error::GeometryOverflow)?;
            sink(out_y, output.as_slice())?;
            output_rows += 1;
            if let Some(next) = lines.get(output_rows).and_then(|line| line.first()) {
                while cache.front().is_some_and(|row| row.y < next.index) {
                    cache.pop_front();
                }
            } else {
                cache.clear();
            }
        }
        Ok(())
    })?;

    if decoded_rows != plan.source.height || output_rows != lines.len()
        || report.rows != u64::from(decoded_rows)
    {
        return Err(Error::IncompleteDecoderOutput);
    }
    Ok(QualityReport {
        decoder: report,
        output_rows: u64::try_from(output_rows).map_err(|_| Error::GeometryOverflow)?,
        decoder_requests: 1,
        peak_engine_managed_bytes: budget.snapshot().peak,
        explicit_full_frame_intermediates: 0,
    })
}
