//! Read-only JPEG fallback: full output materialization is explicit.
//! No JPEG encoder or JPEG-first core assumptions.
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};

use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::memory::MemoryBudget;

pub(crate) struct JpegSource<'a> {
    bytes: &'a [u8],
    info: DecoderInfo,
}
fn decoder(bytes: &[u8]) -> JpegDecoder<ZCursor<&[u8]>> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    JpegDecoder::new_with_options(ZCursor::new(bytes), options)
}
impl<'a> JpegSource<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let mut decoder = decoder(bytes);
        decoder.decode_headers().map_err(|_| Error::DecodeFailed)?;
        let (width, height) = decoder.dimensions().ok_or(Error::DecodeFailed)?;
        let size = Size::new(
            u32::try_from(width).map_err(|_| Error::GeometryOverflow)?,
            u32::try_from(height).map_err(|_| Error::GeometryOverflow)?,
        )?;
        if decoder.output_colorspace() != Some(ColorSpace::RGB) {
            return Err(Error::UnsupportedCodecLayout);
        }
        Ok(Self {
            bytes,
            info: DecoderInfo {
                size,
                layout: ChannelLayout::Rgb,
                sample: SampleType::U8,
                access: DecodeAccess::FullFrameFallback,
                rendered_icc: decoder.icc_profile(),
                rendered_cicp: None,
            },
        })
    }
}
impl DecodeSource for JpegSource<'_> {
    fn info(&self) -> &DecoderInfo {
        &self.info
    }

    fn read_region(
        &mut self,
        region: Rect,
        budget: &MemoryBudget,
        sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
    ) -> Result<DecodeReport, Error> {
        if !region.fits(self.info.size) {
            return Err(Error::OutOfBounds);
        }
        let mut decoder = decoder(self.bytes);
        decoder.decode_headers().map_err(|_| Error::DecodeFailed)?;
        let width = usize::try_from(self.info.size.width).map_err(|_| Error::GeometryOverflow)?;
        let height = usize::try_from(self.info.size.height).map_err(|_| Error::GeometryOverflow)?;
        let frame_bytes = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(3))
            .ok_or(Error::GeometryOverflow)?;
        if decoder.output_buffer_size() != Some(frame_bytes) {
            return Err(Error::UnsupportedCodecLayout);
        }
        let mut frame = budget.allocate(frame_bytes)?;
        decoder
            .decode_into(frame.as_mut_slice())
            .map_err(|_| Error::DecodeFailed)?;
        let stride = width.checked_mul(3).ok_or(Error::GeometryOverflow)?;
        let x_bytes = usize::try_from(region.x)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(3)
            .ok_or(Error::GeometryOverflow)?;
        let row_bytes = usize::try_from(region.width)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(3)
            .ok_or(Error::GeometryOverflow)?;
        let mut rows = 0_u64;
        for y in region.y..region.bottom() {
            let offset = usize::try_from(y)
                .map_err(|_| Error::GeometryOverflow)?
                .checked_mul(stride)
                .and_then(|i| i.checked_add(x_bytes))
                .ok_or(Error::GeometryOverflow)?;
            sink(
                y,
                DecodedRow::U8(&frame.as_slice()[offset..offset + row_bytes]),
            )?;
            rows += 1;
        }
        Ok(DecodeReport {
            requested: region,
            access: DecodeAccess::FullFrameFallback,
            rows,
            output_sample_bytes: rows
                .checked_mul(u64::try_from(row_bytes).map_err(|_| Error::GeometryOverflow)?)
                .ok_or(Error::GeometryOverflow)?,
            known_full_frame_buffers: 1,
            peak_engine_managed_bytes: budget.snapshot().peak,
            opaque_codec_memory_unbounded: true,
        })
    }
}
