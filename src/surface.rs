//! An intentionally narrow, **borrowed** 10-bit planar surface contract.
//!
//! A surface is an external set of planes, not an eagerly materialized image.
//! Plane storage is borrowed as bytes with byte strides, so neither the graph
//! nor this module needs to own, align, or transmute decoder memory. This first
//! experiment supports *unpacked little-endian u16* samples containing 10 bits.
//! Other layouts will be introduced only with an adapter that requires them.

use crate::error::Error;
use crate::geometry::{Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorPrimaries {
    Bt709,
    Bt2020,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferFunction {
    Srgb,
    Pq,
    Hlg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatrixCoefficients {
    Bt709,
    Bt2020NonConstantLuminance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorRange {
    Full,
    Limited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChromaSiting {
    Centered,
    Cosited,
}

/// Metadata is carried unchanged through lossless/coordinate-only operations.
/// It does **not** assert that resizing YUV planes independently is colorimetrically
/// correct, nor does it express a complete ICC or CICP implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColorMetadata {
    pub primaries: ColorPrimaries,
    pub transfer: TransferFunction,
    pub matrix: MatrixCoefficients,
    pub range: ColorRange,
    pub chroma_siting: ChromaSiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PixelFormat {
    Gray8,
    /// 4:2:0 planar YCbCr; each sample is a LE u16 containing 10 valid bits.
    Yuv420P10(ColorMetadata),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaneId {
    Y,
    Cb,
    Cr,
}

impl PlaneId {
    pub(crate) const ALL: [Self; 3] = [Self::Y, Self::Cb, Self::Cr];

    const fn index(self) -> usize {
        match self {
            Self::Y => 0,
            Self::Cb => 1,
            Self::Cr => 2,
        }
    }
}

/// Borrowed byte-strided plane of little-endian 16-bit samples.
///
/// Padding between rows remains owned by the caller and is never read as
/// image samples. The final row needs only its active pixels to be present;
/// padding after the final row is optional.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlaneView<'a> {
    bytes: &'a [u8],
    size: Size,
    stride_bytes: usize,
}

impl<'a> PlaneView<'a> {
    pub(crate) fn new(bytes: &'a [u8], size: Size, stride_bytes: usize) -> Result<Self, Error> {
        let width_bytes = row_bytes(size.width)?;
        if stride_bytes < width_bytes {
            return Err(Error::InvalidPlaneStride);
        }
        let last_row = usize::try_from(size.height - 1)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(stride_bytes)
            .ok_or(Error::GeometryOverflow)?;
        let end = last_row
            .checked_add(width_bytes)
            .ok_or(Error::GeometryOverflow)?;
        if end > bytes.len() {
            return Err(Error::InsufficientPlaneStorage);
        }
        Ok(Self {
            bytes,
            size,
            stride_bytes,
        })
    }

    pub(crate) fn size(self) -> Size {
        self.size
    }

    pub(crate) fn stride_bytes(self) -> usize {
        self.stride_bytes
    }

    /// Returns an unaligned, zero-copy subview of the plane.
    pub(crate) fn crop(self, rect: Rect) -> Result<Self, Error> {
        if !rect.fits(self.size) {
            return Err(Error::OutOfBounds);
        }
        let start = self.offset(rect.x, rect.y)?;
        Self::new(
            &self.bytes[start..],
            Size::new(rect.width, rect.height)?,
            self.stride_bytes,
        )
    }

    /// A borrowed contiguous slice of a row, excluding all padding.
    pub(crate) fn row(self, y: u32, x: u32, width: u32) -> Result<&'a [u8], Error> {
        let rect = Rect::new(x, y, width, 1)?;
        if !rect.fits(self.size) {
            return Err(Error::OutOfBounds);
        }
        let start = self.offset(x, y)?;
        let end = start
            .checked_add(row_bytes(width)?)
            .ok_or(Error::GeometryOverflow)?;
        Ok(&self.bytes[start..end])
    }

    /// Scalar little-endian read used only for test oracles, not SIMD kernels.
    pub(crate) fn sample(self, x: u32, y: u32) -> Result<u16, Error> {
        let row = self.row(y, x, 1)?;
        Ok(u16::from_le_bytes([row[0], row[1]]))
    }

    fn offset(self, x: u32, y: u32) -> Result<usize, Error> {
        usize::try_from(y)
            .map_err(|_| Error::GeometryOverflow)?
            .checked_mul(self.stride_bytes)
            .and_then(|base| base.checked_add(row_bytes(x).ok()?))
            .ok_or(Error::GeometryOverflow)
    }
}

fn row_bytes(width: u32) -> Result<usize, Error> {
    usize::try_from(width)
        .map_err(|_| Error::GeometryOverflow)?
        .checked_mul(2)
        .ok_or(Error::GeometryOverflow)
}

/// A borrowed, three-plane YCbCr 4:2:0 surface. The input buffers outlive the
/// execution call; this view owns no memory. Chroma dimensions use ceil(width/2)
/// and ceil(height/2), including for odd luma dimensions.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Yuv420P10<'a> {
    size: Size,
    color: ColorMetadata,
    planes: [PlaneView<'a>; 3],
}

impl<'a> Yuv420P10<'a> {
    pub(crate) fn new(
        size: Size,
        color: ColorMetadata,
        y: PlaneView<'a>,
        cb: PlaneView<'a>,
        cr: PlaneView<'a>,
    ) -> Result<Self, Error> {
        let chroma = chroma_size(size)?;
        if y.size() != size || cb.size() != chroma || cr.size() != chroma {
            return Err(Error::InvalidPlaneDimensions);
        }
        Ok(Self {
            size,
            color,
            planes: [y, cb, cr],
        })
    }

    pub(crate) fn size(self) -> Size {
        self.size
    }

    pub(crate) fn color(self) -> ColorMetadata {
        self.color
    }

    pub(crate) fn plane(self, id: PlaneId) -> PlaneView<'a> {
        self.planes[id.index()]
    }
}

pub(crate) fn chroma_size(size: Size) -> Result<Size, Error> {
    Size::new(size.width.div_ceil(2), size.height.div_ceil(2))
}

/// Map an even-origin luma region to its corresponding 4:2:0 plane rectangle.
/// Odd offsets are rejected: they change chroma phase and cannot be expressed
/// as a simple byte-offset view. This is an experiment, not general crop support.
pub(crate) fn chroma_rect(luma: Rect) -> Result<Rect, Error> {
    if luma.x % 2 != 0 || luma.y % 2 != 0 {
        return Err(Error::ChromaPhaseRequiresResampling);
    }
    Rect::new(
        luma.x / 2,
        luma.y / 2,
        luma.width.div_ceil(2),
        luma.height.div_ceil(2),
    )
}
