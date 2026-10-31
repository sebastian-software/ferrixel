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
    /// The planner owns color classification; a profile is never discarded
    /// merely because a second CICP hint resembles sRGB.
    pub(crate) fn linear_srgb_readiness(&self) -> crate::color_contract::LinearSrgbReadiness {
        crate::color_contract::plan_to_linear_srgb(self)
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
