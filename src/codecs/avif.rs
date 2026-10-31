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
    /// An upper bound on coded frame dimensions is checked *before* the
    /// decoder allocates a full pixel surface. This is not an RSS limit:
    /// parsing and decoding can still allocate opaque codec memory.
    pub(crate) fn new_bounded(bytes: &[u8], max_pixels: u64) -> Result<Self, Error> {
        let mut reader = bytes;
        let parsed = avif_parse::AvifData::from_reader(&mut reader)
            .map_err(|_| Error::DecodeFailed)?;
        let metadata = parsed.primary_item_metadata().map_err(|_| Error::DecodeFailed)?;
        let coded_pixels = u64::from(metadata.max_frame_width.get())
            .checked_mul(u64::from(metadata.max_frame_height.get()))
            .ok_or(Error::GeometryOverflow)?;
        if coded_pixels > max_pixels {
            return Err(Error::InputPixelLimitExceeded {
                pixels: coded_pixels,
                limit: max_pixels,
            });
        }
        // Parsing above holds compressed AV1 data temporarily; drop it before
        // decoding. The downstream decoder currently parses the AVIF again.
        drop(parsed);
        let image = Decoder::from_avif(bytes)
            .map_err(|_| Error::DecodeFailed)?
            .to_image()
            .map_err(|_| Error::DecodeFailed)?;
        let (width, height, layout) = match &image {
            Image::Rgb8(frame) => (frame.width(), frame.height(), ChannelLayout::Rgb),
            Image::Rgba8(frame) => (frame.width(), frame.height(), ChannelLayout::Rgba),
            Image::Rgb16(frame) => (frame.width(), frame.height(), ChannelLayout::Rgb),
            Image::Rgba16(frame) => (frame.width(), frame.height(), ChannelLayout::Rgba),
            _ => return Err(Error::UnsupportedCodecLayout),
        };
        let size = Size::new(
            u32::try_from(width).map_err(|_| Error::GeometryOverflow)?,
            u32::try_from(height).map_err(|_| Error::GeometryOverflow)?,
        )?;
        let rendered_pixels = u64::from(size.width) * u64::from(size.height);
        if rendered_pixels > max_pixels {
            return Err(Error::InputPixelLimitExceeded {
                pixels: rendered_pixels,
                limit: max_pixels,
            });
        }
        let info = DecoderInfo {
            size,
            layout,
            sample: if matches!(image, Image::Rgb16(_) | Image::Rgba16(_)) {
                SampleType::F32
            } else {
                SampleType::U8
            },
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

    pub(crate) fn new(bytes: &[u8]) -> Result<Self, Error> {
        Self::new_bounded(bytes, 100_000_000)
    }
}

/// Convert the decoder-owned full frame into borrowed interleaved output
/// rows. The temporary row buffers stay under Ferrixel's managed budget.
fn emit_decoded_rows(
    image: &Image,
    region: Rect,
    width: usize,
    scratch_u8: &mut Option<crate::memory::ManagedBuffer<u8>>,
    scratch_f32: &mut Option<crate::memory::ManagedBuffer<f32>>,
    sink: &mut dyn FnMut(u32, DecodedRow<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    let left = usize::try_from(region.x).map_err(|_| Error::GeometryOverflow)?;
    for y in region.y..region.bottom() {
        let row_index = usize::try_from(y).map_err(|_| Error::GeometryOverflow)?;
        match image {
            Image::Rgb8(frame) => {
                let pixels = &frame[row_index][left..left + width];
                let scratch = scratch_u8.as_mut().ok_or(Error::UnsupportedCodecLayout)?;
                for (dst, pixel) in scratch
                    .as_mut_slice()
                    .as_chunks_mut::<3>()
                    .0
                    .iter_mut()
                    .zip(pixels)
                {
                    dst.copy_from_slice(&[pixel.r, pixel.g, pixel.b]);
                }
            }
            Image::Rgba8(frame) => {
                let pixels = &frame[row_index][left..left + width];
                let scratch = scratch_u8.as_mut().ok_or(Error::UnsupportedCodecLayout)?;
                for (dst, pixel) in scratch
                    .as_mut_slice()
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(pixels)
                {
                    dst.copy_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
                }
            }
            Image::Rgb16(frame) => {
                let pixels = &frame[row_index][left..left + width];
                let scratch = scratch_f32.as_mut().ok_or(Error::UnsupportedCodecLayout)?;
                for (dst, pixel) in scratch
                    .as_mut_slice()
                    .as_chunks_mut::<3>()
                    .0
                    .iter_mut()
                    .zip(pixels)
                {
                    dst.copy_from_slice(&[
                        f32::from(pixel.r) / 65_535.0,
                        f32::from(pixel.g) / 65_535.0,
                        f32::from(pixel.b) / 65_535.0,
                    ]);
                }
            }
            Image::Rgba16(frame) => {
                let pixels = &frame[row_index][left..left + width];
                let scratch = scratch_f32.as_mut().ok_or(Error::UnsupportedCodecLayout)?;
                for (dst, pixel) in scratch
                    .as_mut_slice()
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(pixels)
                {
                    dst.copy_from_slice(&[
                        f32::from(pixel.r) / 65_535.0,
                        f32::from(pixel.g) / 65_535.0,
                        f32::from(pixel.b) / 65_535.0,
                        f32::from(pixel.a) / 65_535.0,
                    ]);
                }
            }
            _ => return Err(Error::UnsupportedCodecLayout),
        }
        if let Some(scratch) = &scratch_u8 {
            sink(y, DecodedRow::U8(scratch.as_slice()))?;
        } else if let Some(scratch) = &scratch_f32 {
            sink(y, DecodedRow::F32(scratch.as_slice()))?;
        } else {
            return Err(Error::UnsupportedCodecLayout);
        }
    }
    Ok(())
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
        let components = width.checked_mul(channels).ok_or(Error::GeometryOverflow)?;
        let mut scratch_u8 = if self.info.sample == SampleType::U8 {
            Some(budget.allocate(components)?)
        } else {
            None
        };
        let mut scratch_f32 = if self.info.sample == SampleType::F32 {
            Some(budget.allocate_typed::<f32>(components)?)
        } else {
            None
        };
        emit_decoded_rows(
            &self.image,
            region,
            width,
            &mut scratch_u8,
            &mut scratch_f32,
            sink,
        )?;
        Ok(DecodeReport {
            requested: region,
            access: DecodeAccess::FullFrameFallback,
            rows: u64::from(region.height),
            output_sample_bytes: u64::from(region.width)
                * u64::from(region.height)
                * channels as u64
                * if self.info.sample == SampleType::F32 {
                    4
                } else {
                    1
                },
            known_full_frame_buffers: 1,
            peak_engine_managed_bytes: budget.snapshot().peak,
            opaque_codec_memory_unbounded: true,
        })
    }
}
