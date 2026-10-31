//! Feature-gated AVIF materialization boundary. ravif currently consumes a
//! complete packed input frame and returns an encoded Vec; neither is hidden
//! behind a false streaming-memory claim. The upstream source may stream
//! into this bounded *engine-owned output surface* exactly once.

use crate::error::Error;
use crate::geometry::Size;
use crate::memory::{ManagedBuffer, MemoryBudget};

#[derive(Debug, Clone, Copy)]
pub(crate) struct EncodeSettings {
    pub quality: f32,
    pub speed: u8,
}

impl EncodeSettings {
    pub(crate) fn validate(self) -> Result<(), Error> {
        if !(1.0..=100.0).contains(&self.quality)
            || !self.quality.is_finite()
            || !(1..=10).contains(&self.speed)
        {
            return Err(Error::InvalidEncodeOptions);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct EncodedAvif {
    pub bytes: Vec<u8>,
    pub frame_materialization_bytes: usize,
    pub encoded_bytes: usize,
    /// The rav1e encoder may allocate additional opaque scratch memory.
    pub opaque_encoder_memory_unbounded: bool,
}

pub(crate) struct AvifSink {
    size: Size,
    next_y: u32,
    pixels: ManagedBuffer<ravif::RGBA8>,
}

impl AvifSink {
    pub(crate) fn new(size: Size, budget: &MemoryBudget) -> Result<Self, Error> {
        let pixels = usize::try_from(size.width)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(usize::try_from(size.height).map_err(|_| Error::GeometryOverflow)?)
            .ok_or(Error::GeometryOverflow)?;
        Ok(Self {
            size,
            next_y: 0,
            pixels: budget.allocate_typed::<ravif::RGBA8>(pixels)?,
        })
    }

    /// Rows are output-space straight RGBA8; the input is copied into the
    /// encoder-required surface exactly once, with no extra eager intermediate.
    pub(crate) fn write_row(&mut self, y: u32, bytes: &[u8]) -> Result<(), Error> {
        let width = usize::try_from(self.size.width).map_err(|_| Error::GeometryOverflow)?;
        if y != self.next_y || bytes.len() != width.checked_mul(4).ok_or(Error::GeometryOverflow)? {
            return Err(Error::IncompleteOutput);
        }
        let start = usize::try_from(y)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(width)
            .ok_or(Error::GeometryOverflow)?;
        for (pixel, sample) in self.pixels.as_mut_slice()[start..start + width]
            .iter_mut()
            .zip(bytes.chunks_exact(4))
        {
            *pixel = ravif::RGBA8::new(sample[0], sample[1], sample[2], sample[3]);
        }
        self.next_y += 1;
        Ok(())
    }

    pub(crate) fn finish(self, settings: EncodeSettings) -> Result<EncodedAvif, Error> {
        settings.validate()?;
        if self.next_y != self.size.height {
            return Err(Error::IncompleteOutput);
        }
        let size = self.pixels.as_slice().len()
            .checked_mul(std::mem::size_of::<ravif::RGBA8>())
            .ok_or(Error::GeometryOverflow)?;
        let image = ravif::Img::new(
            self.pixels.as_slice(),
            usize::try_from(self.size.width).map_err(|_| Error::GeometryOverflow)?,
            usize::try_from(self.size.height).map_err(|_| Error::GeometryOverflow)?,
        );
        let encoded = ravif::Encoder::new()
            .with_quality(settings.quality)
            .with_speed(settings.speed)
            .with_num_threads(Some(1))
            .encode_rgba(image)
            .map_err(|_| Error::EncodeFailed)?;
        let encoded_bytes = encoded.avif_file.len();
        Ok(EncodedAvif {
            bytes: encoded.avif_file,
            frame_materialization_bytes: size,
            encoded_bytes,
            opaque_encoder_memory_unbounded: true,
        })
    }
}
