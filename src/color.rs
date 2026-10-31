//! Codec-independent description of the samples emitted by a decoder.
//!
//! This records provenance and alpha semantics, not an implicit color-space
//! conversion. Unknown metadata must never silently become sRGB. ICC bytes are
//! reference counted so a graph can share the exact profile across nodes.

use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ColorEncoding {
    Unspecified,
    Icc(Arc<[u8]>),
    Cicp([u8; 4]),
    IccAndCicp { icc: Arc<[u8]>, cicp: [u8; 4] },
}

impl ColorEncoding {
    /// Only an unambiguous full-range, RGB-matrix sRGB CICP descriptor is
    /// eligible for the first linear-light reference. ICC presence always
    /// requires an explicit color-management step, even with matching CICP.
    pub(crate) fn is_unambiguous_srgb(&self) -> bool {
        matches!(self, Self::Cicp([1, 13, 0, 1]))
    }

    pub(crate) fn from_metadata(icc: Option<&[u8]>, cicp: Option<[u8; 4]>) -> Self {
        match (icc, cicp) {
            (Some(icc), Some(cicp)) => Self::IccAndCicp {
                icc: Arc::from(icc),
                cicp,
            },
            (Some(icc), None) => Self::Icc(Arc::from(icc)),
            (None, Some(cicp)) => Self::Cicp(cicp),
            (None, None) => Self::Unspecified,
        }
    }
}

/// Association modes reserved for future alpha-capable codec adapters.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlphaMode {
    Opaque,
    /// An alpha channel exists, but its association has not been established.
    Unspecified,
    Straight,
    Premultiplied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageSemantics {
    pub color: ColorEncoding,
    pub alpha: AlphaMode,
}

impl ImageSemantics {
    pub(crate) fn unspecified_opaque() -> Self {
        Self {
            color: ColorEncoding::Unspecified,
            alpha: AlphaMode::Opaque,
        }
    }
}
