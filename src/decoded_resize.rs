//! First real-codec resize seam: a scalar, encoded-sample-domain reference.
//!
//! A single decoder region request streams rows past a one-row input cache and
//! one-row output buffer. JPEG's full-frame fallback is decoded once, not
//! once per output scanline. The bounding ROI can still overfetch with JPEG XL.
//! This is not color-managed or alpha-aware resampling.

use crate::decoder::{ChannelLayout, DecodeReport, DecodeSource, DecodedRow, SampleType};
use crate::error::Error;
use crate::geometry::Rect;
use crate::graph::{Graph, NodeId, NodeKind, ResizeDomain};
use crate::memory::{ManagedBuffer, MemoryBudget};
use crate::plan::{Sample, plan_region, sample_at};
use crate::surface::PixelFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResizeReport {
    pub decoder: DecodeReport,
    pub output_rows: u64,
    pub output_sample_bytes: u64,
    pub peak_engine_managed_bytes: usize,
    pub full_frame_intermediates: u64,
    pub decoder_region_requests: u32,
}

enum PreviousRow {
    U8(ManagedBuffer<u8>),
    F32(ManagedBuffer<f32>),
}

impl PreviousRow {
    fn allocate(sample: SampleType, len: usize, budget: &MemoryBudget) -> Result<Self, Error> {
        Ok(match sample {
            SampleType::U8 => Self::U8(budget.allocate_typed::<u8>(len)?),
            SampleType::F32 => Self::F32(budget.allocate_typed::<f32>(len)?),
        })
    }

    fn sample(&self, index: usize) -> f64 {
        match self {
            Self::U8(data) => f64::from(data.as_slice()[index]),
            Self::F32(data) => f64::from(data.as_slice()[index]),
        }
    }

    fn remember(&mut self, row: DecodedRow<'_>) -> Result<(), Error> {
        match (self, row) {
            (Self::U8(buffer), DecodedRow::U8(samples)) => {
                buffer.as_mut_slice().copy_from_slice(samples);
                Ok(())
            }
            (Self::F32(buffer), DecodedRow::F32(samples)) => {
                buffer.as_mut_slice().copy_from_slice(samples);
                Ok(())
            }
            _ => Err(Error::IncompleteDecoderOutput),
        }
    }
}

enum OutputRow {
    U8(ManagedBuffer<u8>),
    F32(ManagedBuffer<f32>),
}

impl OutputRow {
    fn allocate(sample: SampleType, len: usize, budget: &MemoryBudget) -> Result<Self, Error> {
        Ok(match sample {
            SampleType::U8 => Self::U8(budget.allocate_typed::<u8>(len)?),
            SampleType::F32 => Self::F32(budget.allocate_typed::<f32>(len)?),
        })
    }

    fn write(&mut self, index: usize, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::NonFinitePixelSample);
        }
        match self {
            Self::U8(buffer) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let sample = value.clamp(0.0, 255.0).round() as u8;
                buffer.as_mut_slice()[index] = sample;
            }
            Self::F32(buffer) => {
                #[allow(clippy::cast_possible_truncation)]
                let sample = value as f32;
                if !sample.is_finite() {
                    return Err(Error::NonFinitePixelSample);
                }
                buffer.as_mut_slice()[index] = sample;
            }
        }
        Ok(())
    }

    fn as_row(&self) -> DecodedRow<'_> {
        match self {
            Self::U8(buffer) => DecodedRow::U8(buffer.as_slice()),
            Self::F32(buffer) => DecodedRow::F32(buffer.as_slice()),
        }
    }
}

fn sample(row: DecodedRow<'_>, index: usize) -> f64 {
    match row {
        DecodedRow::U8(data) => f64::from(data[index]),
        DecodedRow::F32(data) => f64::from(data[index]),
    }
}

fn horizontal(
    row: DecodedRow<'_>,
    previous: &PreviousRow,
    from_previous: bool,
    x: Sample,
    left: u32,
    channels: usize,
    channel: usize,
) -> Result<f64, Error> {
    let lower = usize::try_from(x.lower.checked_sub(left).ok_or(Error::OutOfBounds)?)
        .map_err(|_| Error::GeometryOverflow)?;
    let upper = usize::try_from(x.upper.checked_sub(left).ok_or(Error::OutOfBounds)?)
        .map_err(|_| Error::GeometryOverflow)?;
    let left_index = lower
        .checked_mul(channels)
        .and_then(|v| v.checked_add(channel))
        .ok_or(Error::GeometryOverflow)?;
    let right_index = upper
        .checked_mul(channels)
        .and_then(|v| v.checked_add(channel))
        .ok_or(Error::GeometryOverflow)?;
    let (left_sample, right_sample) = if from_previous {
        (previous.sample(left_index), previous.sample(right_index))
    } else {
        (sample(row, left_index), sample(row, right_index))
    };
    Ok(left_sample + (right_sample - left_sample) * x.upper_weight)
}

#[derive(Clone, Copy)]
struct PreparedResize {
    source_region: Rect,
    resize_region: Rect,
    input_footprint: Rect,
    input_size: crate::geometry::Size,
    resized_size: crate::geometry::Size,
    channels: usize,
    domain: ResizeDomain,
}

impl PreparedResize {
    fn render_row(
        self,
        current: DecodedRow<'_>,
        previous: &PreviousRow,
        result: &mut OutputRow,
        vertical: Sample,
        use_previous: bool,
    ) -> Result<(), Error> {
        match self.domain {
            ResizeDomain::EncodedSamples => {
                self.render_encoded_row(current, previous, result, vertical, use_previous)
            }
            ResizeDomain::LinearSrgb => {
                self.render_linear_srgb_row(current, previous, result, vertical, use_previous)
            }
        }
    }

    fn render_encoded_row(
        self,
        current: DecodedRow<'_>,
        previous: &PreviousRow,
        result: &mut OutputRow,
        vertical: Sample,
        use_previous: bool,
    ) -> Result<(), Error> {
        for column in 0..self.resize_region.width {
            let output_x = self.resize_region.x + column;
            let x = sample_at(output_x, self.input_size.width, self.resized_size.width);
            for channel in 0..self.channels {
                let top = horizontal(
                    current,
                    previous,
                    use_previous,
                    x,
                    self.input_footprint.x,
                    self.channels,
                    channel,
                )?;
                let bottom = horizontal(
                    current,
                    previous,
                    false,
                    x,
                    self.input_footprint.x,
                    self.channels,
                    channel,
                )?;
                let value = top + (bottom - top) * vertical.upper_weight;
                let position = usize::try_from(column)
                    .map_err(|_| Error::GeometryOverflow)?
                    .checked_mul(self.channels)
                    .and_then(|v| v.checked_add(channel))
                    .ok_or(Error::GeometryOverflow)?;
                result.write(position, value)?;
            }
        }
        Ok(())
    }

    fn render_linear_srgb_row(
        self,
        current: DecodedRow<'_>,
        previous: &PreviousRow,
        result: &mut OutputRow,
        vertical: Sample,
        use_previous: bool,
    ) -> Result<(), Error> {
        let DecodedRow::U8(current_pixels) = current else {
            return Err(Error::UnsupportedPixelOperation);
        };
        let PreviousRow::U8(previous_pixels) = previous else {
            return Err(Error::UnsupportedPixelOperation);
        };
        let top = if use_previous {
            previous_pixels.as_slice()
        } else {
            current_pixels
        };

        for column in 0..self.resize_region.width {
            let output_x = self.resize_region.x + column;
            let x = sample_at(output_x, self.input_size.width, self.resized_size.width);
            let lower = usize::try_from(
                x.lower
                    .checked_sub(self.input_footprint.x)
                    .ok_or(Error::OutOfBounds)?,
            )
            .map_err(|_| Error::GeometryOverflow)?;
            let upper = usize::try_from(
                x.upper
                    .checked_sub(self.input_footprint.x)
                    .ok_or(Error::OutOfBounds)?,
            )
            .map_err(|_| Error::GeometryOverflow)?;
            let lower_offset = lower
                .checked_mul(self.channels)
                .ok_or(Error::GeometryOverflow)?;
            let upper_offset = upper
                .checked_mul(self.channels)
                .ok_or(Error::GeometryOverflow)?;
            let encoded = crate::linear_srgb::bilinear(
                top,
                current_pixels,
                lower_offset,
                upper_offset,
                self.channels,
                x.upper_weight,
                vertical.upper_weight,
            );
            let destination = usize::try_from(column)
                .map_err(|_| Error::GeometryOverflow)?
                .checked_mul(self.channels)
                .ok_or(Error::GeometryOverflow)?;
            for (channel, sample) in encoded.iter().take(self.channels).enumerate() {
                result.write(destination + channel, f64::from(*sample))?;
            }
        }
        Ok(())
    }

    fn upper_source_y(self, output_row: u32) -> Result<(Sample, u32), Error> {
        let output_y = self
            .resize_region
            .y
            .checked_add(output_row)
            .ok_or(Error::GeometryOverflow)?;
        let vertical = sample_at(output_y, self.input_size.height, self.resized_size.height);
        let upper_y = self
            .source_region
            .y
            .checked_add(
                vertical
                    .upper
                    .checked_sub(self.input_footprint.y)
                    .ok_or(Error::OutOfBounds)?,
            )
            .ok_or(Error::GeometryOverflow)?;
        Ok((vertical, upper_y))
    }
}

fn prepare_resize(
    graph: &Graph,
    target: NodeId,
    bound_source: NodeId,
    info: &crate::decoder::DecoderInfo,
    output: Rect,
) -> Result<PreparedResize, Error> {
    let steps = plan_region(graph, target, output)?;
    let first = steps.first().ok_or(Error::UnknownNode)?;
    let source_node = graph.node(first.node)?;
    if first.node != bound_source
        || source_node.kind != NodeKind::DecoderSource
        || source_node.size != info.size
        || source_node.format != info.pixel_format()
        || source_node.semantics != info.semantics()
    {
        return Err(Error::SourceBindingMismatch);
    }
    let mut resize_index = None;
    for (index, step) in steps.iter().enumerate().skip(1) {
        match graph.node(step.node)?.kind {
            NodeKind::Crop { .. } => {}
            NodeKind::Resize { .. } if resize_index.is_none() => resize_index = Some(index),
            _ => return Err(Error::UnsupportedPixelOperation),
        }
    }
    let index = resize_index.ok_or(Error::UnsupportedPixelOperation)?;
    let resize_step = steps[index];
    let input_step = steps[index - 1];
    let resize = graph.node(resize_step.node)?;
    let input = graph.node(input_step.node)?;
    if resize.format != info.pixel_format() || resize.semantics != info.semantics() {
        return Err(Error::PixelFormatMismatch);
    }
    let NodeKind::Resize { domain, .. } = resize.kind else {
        return Err(Error::UnsupportedPixelOperation);
    };
    match domain {
        ResizeDomain::EncodedSamples => {
            if !matches!(info.layout, ChannelLayout::Gray | ChannelLayout::Rgb)
                || !matches!(
                    resize.format,
                    PixelFormat::InterleavedU8(1 | 3) | PixelFormat::InterleavedF32(1 | 3)
                )
            {
                return Err(Error::UnsupportedPixelOperation);
            }
        }
        ResizeDomain::LinearSrgb => {
            if !matches!(info.layout, ChannelLayout::Rgb | ChannelLayout::Rgba)
                || info.sample != SampleType::U8
            {
                return Err(Error::UnsupportedPixelOperation);
            }
            let expected_alpha = if info.layout == ChannelLayout::Rgba {
                crate::color::AlphaMode::Straight
            } else {
                crate::color::AlphaMode::Opaque
            };
            if info.alpha != expected_alpha {
                return Err(Error::UnsupportedAlphaMode);
            }
            if !info.semantics().color.is_unambiguous_srgb() {
                return Err(Error::UnsupportedColorEncoding);
            }
        }
    }
    Ok(PreparedResize {
        source_region: first.region,
        resize_region: resize_step.region,
        input_footprint: input_step.region,
        input_size: input.size,
        resized_size: resize.size,
        channels: usize::from(info.layout.channels()),
        domain,
    })
}

/// Execute exactly one interleaved resize, with optional crops on either side.
///
/// The graph explicitly opts into encoded-domain interpolation. This does not
/// perform ICC transformations or linearize transfer functions.
pub(crate) fn execute_decoded_resize(
    graph: &Graph,
    target: NodeId,
    bound_source: NodeId,
    source: &mut dyn DecodeSource,
    output: Rect,
    budget: &MemoryBudget,
    mut sink: impl FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
) -> Result<ResizeReport, Error> {
    let info = source.info().clone();
    let plan = prepare_resize(graph, target, bound_source, &info, output)?;
    let row_samples = usize::try_from(plan.source_region.width)
        .map_err(|_| Error::GeometryOverflow)?
        .checked_mul(plan.channels)
        .ok_or(Error::GeometryOverflow)?;
    let output_samples = usize::try_from(output.width)
        .map_err(|_| Error::GeometryOverflow)?
        .checked_mul(plan.channels)
        .ok_or(Error::GeometryOverflow)?;
    let mut previous = PreviousRow::allocate(info.sample, row_samples, budget)?;
    let mut result = OutputRow::allocate(info.sample, output_samples, budget)?;
    let mut next_y = plan.source_region.y;
    let mut output_rows = 0_u32;
    let mut decoder_rows = 0_u64;
    let decode_report = source.read_region(plan.source_region, budget, &mut |source_y, row| {
        if source_y != next_y || row.sample_type() != info.sample || row.len() != row_samples {
            return Err(Error::IncompleteDecoderOutput);
        }
        while output_rows < output.height {
            let (vertical, upper_source_y) = plan.upper_source_y(output_rows)?;
            if upper_source_y != source_y {
                break;
            }
            let use_previous = vertical.lower != vertical.upper;
            if use_previous && source_y == plan.source_region.y {
                return Err(Error::IncompleteDecoderOutput);
            }
            plan.render_row(row, &previous, &mut result, vertical, use_previous)?;
            let result_y = output
                .y
                .checked_add(output_rows)
                .ok_or(Error::GeometryOverflow)?;
            sink(result_y, result.as_row())?;
            output_rows += 1;
        }
        previous.remember(row)?;
        decoder_rows += 1;
        next_y = next_y.checked_add(1).ok_or(Error::GeometryOverflow)?;
        Ok(())
    })?;
    if decoder_rows != u64::from(plan.source_region.height)
        || decode_report.rows != decoder_rows
        || output_rows != output.height
    {
        return Err(Error::IncompleteDecoderOutput);
    }
    let sample_bytes = match info.sample {
        SampleType::U8 => 1_u64,
        SampleType::F32 => 4_u64,
    };
    let output_sample_bytes = u64::from(output.width)
        .checked_mul(u64::from(output.height))
        .and_then(|len| len.checked_mul(u64::try_from(plan.channels).ok()?))
        .and_then(|len| len.checked_mul(sample_bytes))
        .ok_or(Error::GeometryOverflow)?;
    Ok(ResizeReport {
        decoder: decode_report,
        output_rows: u64::from(output_rows),
        output_sample_bytes,
        peak_engine_managed_bytes: budget.snapshot().peak,
        full_frame_intermediates: 0,
        decoder_region_requests: 1,
    })
}
