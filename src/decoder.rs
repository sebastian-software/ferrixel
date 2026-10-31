//! A private, capability-reporting boundary between graph planning and codecs.
//! A requested ROI is NOT proof that the underlying codec avoided other work.
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::graph::{Graph, NodeId, NodeKind};
use crate::memory::MemoryBudget;
use crate::plan::plan_region;
use crate::surface::PixelFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChannelLayout {
    Gray,
    GrayAlpha,
    Rgb,
    Rgba,
    Cmyk,
    Cmyka,
}
impl ChannelLayout {
    pub(crate) const fn channels(self) -> u8 {
        match self {
            Self::Gray => 1,
            Self::GrayAlpha => 2,
            Self::Rgb => 3,
            Self::Rgba | Self::Cmyk => 4,
            Self::Cmyka => 5,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SampleType {
    U8,
    F32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodeAccess {
    /// Complete decoded frame required even if the sink needs only a crop.
    FullFrameFallback,
    /// Region requested through the codec API; actual skipped work is unknown.
    CodecRegionRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DecoderInfo {
    pub size: Size,
    pub layout: ChannelLayout,
    pub sample: SampleType,
    pub access: DecodeAccess,
    /// Describes *emitted* samples. None means unknown, not assumed sRGB.
    pub rendered_icc: Option<Vec<u8>>,
    pub rendered_cicp: Option<[u8; 4]>,
}
impl DecoderInfo {
    pub(crate) fn pixel_format(&self) -> PixelFormat {
        match self.sample {
            SampleType::U8 => PixelFormat::InterleavedU8(self.layout.channels()),
            SampleType::F32 => PixelFormat::InterleavedF32(self.layout.channels()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum DecodedRow<'a> {
    U8(&'a [u8]),
    F32(&'a [f32]),
}
impl DecodedRow<'_> {
    fn len(self) -> usize {
        match self {
            Self::U8(r) => r.len(),
            Self::F32(r) => r.len(),
        }
    }
    fn sample_type(self) -> SampleType {
        match self {
            Self::U8(_) => SampleType::U8,
            Self::F32(_) => SampleType::F32,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DecodeReport {
    pub requested: Rect,
    pub access: DecodeAccess,
    pub rows: u64,
    pub output_sample_bytes: u64,
    /// Explicitly known, adapter-allocated full output frames.
    pub known_full_frame_buffers: u32,
    pub peak_engine_managed_bytes: usize,
    /// Codec-managed compressed data, render cache, and scratch are not capped.
    pub opaque_codec_memory_unbounded: bool,
}

pub(crate) trait DecodeSource {
    fn info(&self) -> &DecoderInfo;
    /// Borrows each output row only for the duration of the sink callback.
    fn read_region(
        &mut self,
        region: Rect,
        budget: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error>;
}

/// Crop-only graph execution through the existing backwards region planner.
/// A separate transform executor is required before resizing these layouts.
pub(crate) fn execute_decoded(
    graph: &Graph,
    target: NodeId,
    source_node: NodeId,
    source: &mut dyn DecodeSource,
    output: Rect,
    budget: &MemoryBudget,
    mut sink: impl FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
) -> Result<DecodeReport, Error> {
    let steps = plan_region(graph, target, output)?;
    let first = steps.first().ok_or(Error::UnknownNode)?;
    let node = graph.node(first.node)?;
    let info = source.info();
    if first.node != source_node
        || node.kind != NodeKind::DecoderSource
        || node.size != info.size
        || node.format != info.pixel_format()
    {
        return Err(Error::SourceBindingMismatch);
    }
    if steps.iter().skip(1).any(
        |step| !matches!(graph.node(step.node), Ok(n) if matches!(n.kind, NodeKind::Crop { .. })),
    ) {
        return Err(Error::UnsupportedPixelOperation);
    }
    let expected_samples = usize::try_from(first.region.width)
        .map_err(|_| Error::GeometryOverflow)?
        .checked_mul(usize::from(info.layout.channels()))
        .ok_or(Error::GeometryOverflow)?;
    let expected_type = info.sample;
    let mut expected_y = first.region.y;
    let mut delivered = 0_u32;
    let report = source.read_region(first.region, budget, &mut |y, row| {
        if y != expected_y || row.len() != expected_samples || row.sample_type() != expected_type {
            return Err(Error::IncompleteDecoderOutput);
        }
        let output_y = output
            .y
            .checked_add(delivered)
            .ok_or(Error::GeometryOverflow)?;
        sink(output_y, row)?;
        delivered += 1;
        expected_y += 1;
        Ok(())
    })?;
    if delivered != output.height || report.rows != u64::from(delivered) {
        return Err(Error::IncompleteDecoderOutput);
    }
    Ok(report)
}
