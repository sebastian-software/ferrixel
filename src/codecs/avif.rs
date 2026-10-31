//! Read-only AVIF bridge via `avif-decode` (BSD-3-Clause).
//!
//! This codec currently performs a full decode in its constructor. Its
//! allocation occurs inside the codec and is NOT constrained by Ferrixel's
//! managed-memory budget. The emitted RGB8/RGBA8 color profile is unknown;
//! callers must supply an explicit policy before color-managed resampling.
//! HDR/high-bit-depth/gray input currently fails closed.

use avif_decode::{Decoder, Image};

use crate::color::AlphaMode;
use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::memory::MemoryBudget;

pub(crate) struct AvifSource {
    image: Image,
    info: DecoderInfo,
}
impl AvifSource {
    pub(crate) fn new(bytes: &[u8]) -> Result<Self, Error> {
        let image = Decoder::from_avif(bytes)
            .map_err(|_| Error::DecodeFailed)?
            .to_image()
            .map_err(|_| Error::DecodeFailed)?;
        let (width, height, layout) = match &image {
            Image::Rgb8(frame) => (frame.width(), frame.height(), ChannelLayout::Rgb),
            Image::Rgba8(frame) => (frame.width(), frame.height(), ChannelLayout::Rgba),
            _ => return Err(Error::UnsupportedCodecLayout),
        };
        let size = Size::new(
            u32::try_from(width).map_err(|_| Error::GeometryOverflow)?,
            u32::try_from(height).map_err(|_| Error::GeometryOverflow)?,
        )?;
        let info = DecoderInfo {
            size,
            layout,
            sample: SampleType::U8,
            alpha: if layout == ChannelLayout::Rgba {
                // avif-decode unpremultiplies images signaled as premultiplied.
                AlphaMode::Straight
            } else {
                AlphaMode::Opaque
            },
            access: DecodeAccess::FullFrameFallback,
            rendered_icc: None,
            // The API does not expose trustworthy converted-output CICP.
            rendered_cicp: None,
        };
        Ok(Self { image, info })
    }
}

impl DecodeSource for AvifSource {
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
        let channels = usize::from(self.info.layout.channels());
        let width = usize::try_from(region.width).map_err(|_| Error::GeometryOverflow)?;
        let mut scratch =
            budget.allocate(width.checked_mul(channels).ok_or(Error::GeometryOverflow)?)?;
        let left = usize::try_from(region.x).map_err(|_| Error::GeometryOverflow)?;
        for y in region.y..region.bottom() {
            let row_index = usize::try_from(y).map_err(|_| Error::GeometryOverflow)?;
            match &self.image {
                Image::Rgb8(frame) => {
                    let pixels = &frame[row_index][left..left + width];
                    for (dst, pixel) in scratch.as_mut_slice().chunks_exact_mut(3).zip(pixels) {
                        dst.copy_from_slice(&[pixel.r, pixel.g, pixel.b]);
                    }
                }
                Image::Rgba8(frame) => {
                    let pixels = &frame[row_index][left..left + width];
                    for (dst, pixel) in scratch.as_mut_slice().chunks_exact_mut(4).zip(pixels) {
                        dst.copy_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
                    }
                }
                _ => return Err(Error::UnsupportedCodecLayout),
            }
            sink(y, DecodedRow::U8(scratch.as_slice()))?;
        }
        Ok(DecodeReport {
            requested: region,
            access: DecodeAccess::FullFrameFallback,
            rows: u64::from(region.height),
            output_sample_bytes: u64::from(region.width)
                * u64::from(region.height)
                * channels as u64,
            known_full_frame_buffers: 1,
            peak_engine_managed_bytes: budget.snapshot().peak,
            opaque_codec_memory_unbounded: true,
        })
    }
}
