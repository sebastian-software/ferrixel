//! Errors returned by the experimental, codec-free execution path.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Error {
    InvalidDimensions,
    GeometryOverflow,
    OutOfBounds,
    UnknownNode,
    AllocationFailed,
    BudgetExceeded {
        requested: usize,
        available: usize,
    },
    Cancelled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions => write!(f, "image dimensions must be nonzero"),
            Self::GeometryOverflow => write!(f, "image geometry or byte count overflow"),
            Self::OutOfBounds => write!(f, "requested region is outside image bounds"),
            Self::UnknownNode => write!(f, "graph node does not exist"),
            Self::AllocationFailed => write!(f, "could not allocate the requested buffer"),
            Self::BudgetExceeded {
                requested,
                available,
            } => write!(
                f,
                "managed buffer needs {requested} bytes with {available} bytes available"
            ),
            Self::Cancelled => write!(f, "processing was cancelled by the output sink"),
        }
    }
}

impl std::error::Error for Error {}
