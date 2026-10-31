//! Optional, explicit embedded-ICC -> rendered sRGB pixel transform.
//!
//! No ICC profile is reinterpreted as sRGB. The caller must explicitly opt
//! into using the decoder's **rendered output ICC** signal. The transform
//! operates on borrowed input rows and engine-budgeted f32 scratch buffers.
//! The codec's own image allocations remain independently opaque.

use std::sync::Arc;

use moxcms::{ColorProfile, Layout, TransformF32Executor, TransformOptions};

use crate::color::AlphaMode;
use crate::decoder::{
    ChannelLayout, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::Rect;
use crate::memory::MemoryBudget;

pub(crate) struct IccSrgbSource<'a> {
    source: Box<dyn DecodeSource + 'a>,
    output: DecoderInfo,
    transformer: Arc<TransformF32Executor>,
}

impl<'a> IccSrgbSource<'a> {
    pub(crate) fn new(source: Box<dyn DecodeSource + 'a>) -> Result<Self, Error> {
        let info = source.info();
        if info.layout != ChannelLayout::Rgb || info.alpha != AlphaMode::Opaque {
            return Err(Error::UnsupportedCodecLayout);
        }
        let bytes = info
            .rendered_icc
            .as_deref()
            .ok_or(Error::UnsupportedColorEncoding)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::UnsupportedColorEncoding);
        }
        let src =
            ColorProfile::new_from_slice(bytes).map_err(|_| Error::UnsupportedColorEncoding)?;
        let dst = ColorProfile::new_srgb();
        let transformer = src
            .create_transform_f32(Layout::Rgb, &dst, Layout::Rgb, TransformOptions::default())
            .map_err(|_| Error::UnsupportedColorEncoding)?;
        let mut output = info.clone();
        output.sample = SampleType::F32;
        output.rendered_icc = None;
        output.rendered_cicp = Some([1, 13, 0, 1]);
        Ok(Self {
            source,
            output,
            transformer,
        })
    }
}
impl DecodeSource for IccSrgbSource<'_> {
    fn info(&self) -> &DecoderInfo {
        &self.output
    }
    fn read_region(
        &mut self,
        region: Rect,
        budget: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error> {
        let len = usize::try_from(region.width)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(3)
            .ok_or(Error::GeometryOverflow)?;
        let mut input = budget.allocate_typed::<f32>(len)?;
        let mut output = budget.allocate_typed::<f32>(len)?;
        let expected_sample = self.source.info().sample;
        let mut report = self.source.read_region(region, budget, &mut |y, row| {
            if row.len() != len || row.sample_type() != expected_sample {
                return Err(Error::IncompleteDecoderOutput);
            }
            match row {
                DecodedRow::U8(values) => {
                    for (dst, src) in input.as_mut_slice().iter_mut().zip(values) {
                        *dst = f32::from(*src) / 255.0;
                    }
                }
                DecodedRow::F32(values) => input.as_mut_slice().copy_from_slice(values),
            }
            self.transformer
                .transform(input.as_slice(), output.as_mut_slice())
                .map_err(|_| Error::UnsupportedColorEncoding)?;
            sink(y, DecodedRow::F32(output.as_slice()))
        })?;
        report.output_sample_bytes = report
            .rows
            .checked_mul(u64::from(region.width))
            .and_then(|n| n.checked_mul(12))
            .ok_or(Error::GeometryOverflow)?;
        report.peak_engine_managed_bytes = budget.snapshot().peak;
        Ok(report)
    }
}
