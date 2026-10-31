//! Optional read-only codec adapters; no codec depends on the graph itself.
#[cfg(feature = "jpeg-read")]
pub(crate) mod jpeg;
#[cfg(feature = "jpeg-xl-read")]
pub(crate) mod jxl;

#[cfg(feature = "avif-read")]
pub(crate) mod avif;
