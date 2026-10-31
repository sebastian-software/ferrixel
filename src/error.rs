//! Errors returned by the experimental engine and decoder execution paths.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Error {
    InvalidDimensions,
    GeometryOverflow,
    OutOfBounds,
    UnknownNode,
    AllocationFailed,
    InvalidPlaneStride,
    InvalidPlaneDimensions,
    InsufficientPlaneStorage,
    ChromaPhaseRequiresResampling,
    PixelFormatMismatch,
    SourceBindingMismatch,
    UnsupportedPixelOperation,
    BudgetExceeded {
        requested: usize,
        available: usize,
    },
    Cancelled,
    // Constructed only when an optional real codec is enabled.
    #[allow(dead_code)]
    DecodeFailed,
    IncompleteDecoderOutput,
    NonFinitePixelSample,
    InvalidAlphaValue,
    #[cfg(feature = "avif-read")]
    InputPixelLimitExceeded { pixels: u64, limit: u64 },
    #[cfg(feature = "avif-write")]
    InvalidEncodeOptions,
    #[cfg(feature = "avif-write")]
    IncompleteOutput,
    #[cfg(feature = "avif-write")]
    EncodeFailed,
    UnsupportedColorEncoding,
    UnsupportedAlphaMode,
    #[allow(dead_code)]
    UnsupportedCodecLayout,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => write!(f, "image dimensions must be nonzero"),
            Self::GeometryOverflow => write!(f, "image geometry or byte count overflow"),
            Self::OutOfBounds => write!(f, "requested region is outside image bounds"),
            Self::UnknownNode => write!(f, "graph node does not exist"),
            Self::AllocationFailed => write!(f, "could not allocate the requested buffer"),
            Self::InvalidPlaneStride => write!(f, "plane stride is smaller than its active row"),
            Self::InvalidPlaneDimensions => {
                write!(f, "plane dimensions do not match sampling geometry")
            }
            Self::InsufficientPlaneStorage => {
                write!(f, "plane storage is shorter than its declared extent")
            }
            Self::ChromaPhaseRequiresResampling => {
                write!(f, "unaligned 4:2:0 crop requires chroma phase handling")
            }
            Self::PixelFormatMismatch => write!(
                f,
                "the requested executor does not support this pixel format"
            ),
            Self::SourceBindingMismatch => {
                write!(f, "the borrowed source does not match graph metadata")
            }
            Self::UnsupportedPixelOperation => write!(
                f,
                "the operation has no validated implementation for this pixel format"
            ),
            Self::BudgetExceeded {
                requested,
                available,
            } => write!(
                f,
                "managed buffer needs {requested} bytes with {available} bytes available"
            ),
            Self::Cancelled => write!(f, "processing was cancelled by the output sink"),
            Self::DecodeFailed => write!(f, "decoder rejected input or could not render it"),
            Self::NonFinitePixelSample => write!(f, "non-finite sample during resampling"),
            #[cfg(feature = "avif-write")]
            Self::InvalidEncodeOptions => {
                write!(f, "encoder quality/speed or output dimensions are invalid")
            }
            #[cfg(feature = "avif-write")]
            Self::IncompleteOutput => write!(f, "output sink has not received every scanline"),
            #[cfg(feature = "avif-write")]
            Self::EncodeFailed => write!(f, "AVIF encoder failed"),
            #[cfg(feature = "avif-read")]
            Self::InputPixelLimitExceeded { pixels, limit } => {
                write!(f, "input has {pixels} pixels, exceeding the configured {limit} pixel limit")
            }
            Self::InvalidAlphaValue => {
                write!(f, "straight alpha must be finite and between zero and one")
            }
            Self::UnsupportedColorEncoding => write!(
                f,
                "linear-sRGB resize requires unambiguous full-range RGB sRGB CICP; ICC/unknown/HDR need a managed transform"
            ),
            Self::UnsupportedAlphaMode => write!(
                f,
                "the channel layout requires an explicit, compatible alpha-association mode"
            ),
            Self::IncompleteDecoderOutput => {
                write!(f, "decoder did not provide the complete output region")
            }
            Self::UnsupportedCodecLayout => {
                write!(f, "decoder pixel or color layout is not supported")
            }
        }
    }
}

impl std::error::Error for Error {}
