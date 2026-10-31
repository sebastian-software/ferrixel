//! JPEG XL adapter: codec-level ROI request, float output scanlines.
//! ROI is a capability, not a guarantee of skipped internal codec work.
use jxl_oxide::{CropInfo, JxlImage, PixelFormat};
use crate::color::AlphaMode;
use std::io::Cursor;

use crate::decoder::{
    ChannelLayout, DecodeAccess, DecodeReport, DecodeSource, DecodedRow, DecoderInfo, SampleType,
};
use crate::error::Error;
use crate::geometry::{Rect, Size};
use crate::memory::MemoryBudget;

pub(crate) struct JxlSource {
    image: JxlImage,
    info: DecoderInfo,
}
impl JxlSource {
    pub(crate) fn new(bytes: &[u8]) -> Result<Self, Error> {
        let image = JxlImage::builder()
            .read(Cursor::new(bytes))
            .map_err(|_| Error::DecodeFailed)?;
        let size = Size::new(image.width(), image.height())?;
        let layout = match image.pixel_format() {
            PixelFormat::Gray => ChannelLayout::Gray,
            PixelFormat::Graya => ChannelLayout::GrayAlpha,
            PixelFormat::Rgb => ChannelLayout::Rgb,
            PixelFormat::Rgba => ChannelLayout::Rgba,
            PixelFormat::Cmyk => ChannelLayout::Cmyk,
            PixelFormat::Cmyka => ChannelLayout::Cmyka,
        };
        let info = DecoderInfo {
            size,
            layout,
            sample: SampleType::F32,
            access: DecodeAccess::CodecRegionRequest,
            alpha: match layout {
                ChannelLayout::Graya | ChannelLayout::Rgba | ChannelLayout::Cmyka => AlphaMode::Unspecified,
                ChannelLayout::Gray | ChannelLayout::Rgb | ChannelLayout::Cmyk => AlphaMode::Opaque,
            },
            rendered_icc: Some(image.rendered_icc()),
            rendered_cicp: image.rendered_cicp(),
        };
        Ok(Self { image, info })
    }
}
impl DecodeSource for JxlSource {
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
        self.image.set_image_region(CropInfo {
            left: region.x,
            top: region.y,
            width: region.width,
            height: region.height,
        });
        let rendered = self
            .image
            .render_frame_cropped(0)
            .map_err(|_| Error::DecodeFailed)?;
        // Orientation/ROI coordinate mapping is not validated yet.
        if rendered.orientation() != 1 {
            return Err(Error::UnsupportedPixelOperation);
        }
        let mut stream = rendered.stream();
        if stream.width() != region.width
            || stream.height() != region.height
            || stream.channels() != u32::from(self.info.layout.channels())
        {
            return Err(Error::IncompleteDecoderOutput);
        }
        let row_samples = usize::try_from(region.width)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(usize::from(self.info.layout.channels()))
            .ok_or(Error::GeometryOverflow)?;
        let mut scratch = budget.allocate_typed::<f32>(row_samples)?;
        for y in region.y..region.bottom() {
            if stream.write_to_buffer(scratch.as_mut_slice()) != row_samples {
                return Err(Error::IncompleteDecoderOutput);
            }
            sink(y, DecodedRow::F32(scratch.as_slice()))?;
        }
        let output_bytes = u64::try_from(row_samples)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(u64::from(region.height))
            .and_then(|n| n.checked_mul(4))
            .ok_or(Error::GeometryOverflow)?;
        Ok(DecodeReport {
            requested: region,
            access: DecodeAccess::CodecRegionRequest,
            rows: u64::from(region.height),
            output_sample_bytes: output_bytes,
            known_full_frame_buffers: 0,
            peak_engine_managed_bytes: budget.snapshot().peak,
            opaque_codec_memory_unbounded: true,
        })
    }
}
