//! Checked, half-open geometry for image coordinates.

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub(crate) fn new(width: u32, height: u32) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidDimensions);
        }
        Ok(Self { width, height })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub(crate) fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, Error> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidDimensions);
        }
        x.checked_add(width).ok_or(Error::GeometryOverflow)?;
        y.checked_add(height).ok_or(Error::GeometryOverflow)?;
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    pub(crate) fn right(self) -> u32 {
        self.x + self.width
    }

    pub(crate) fn bottom(self) -> u32 {
        self.y + self.height
    }

    pub(crate) fn fits(self, size: Size) -> bool {
        self.right() <= size.width && self.bottom() <= size.height
    }

    pub(crate) fn translate(self, dx: u32, dy: u32) -> Result<Self, Error> {
        let x = self.x.checked_add(dx).ok_or(Error::GeometryOverflow)?;
        let y = self.y.checked_add(dy).ok_or(Error::GeometryOverflow)?;
        Self::new(x, y, self.width, self.height)
    }

    pub(crate) fn byte_len(self) -> Result<usize, Error> {
        let w = usize::try_from(self.width).map_err(|_| Error::GeometryOverflow)?;
        let h = usize::try_from(self.height).map_err(|_| Error::GeometryOverflow)?;
        w.checked_mul(h).ok_or(Error::GeometryOverflow)
    }
}
