//! Ferrixel — an experimental, pipeline-first image processing engine.
//!
//! Ferrixel currently contains a private experimental pipeline and optional
//! read-only JPEG/JPEG XL decoders for validating codec capability contracts,
//! region planning, and bounded engine-managed memory. There is intentionally no stable public processing API.
//!
//! See `docs/architecture.md` and `docs/adr/` in the repository.

#![forbid(unsafe_code)]
// The experiment is compiled as part of the crate but deliberately has no
// public API. Remove this allowance when the first supported API lands.
#![cfg_attr(not(test), allow(dead_code))]

mod codecs;
mod color;
mod color_contract;
mod decoded_resize;
mod decoder;
mod error;
mod execute;
mod geometry;
mod graph;
mod linear_srgb;
mod memory;
mod plan;
mod planar;
mod quality;
mod quality_execute;
mod surface;
mod working_color;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod planar_tests;

#[cfg(test)]
mod quality_tests;

#[cfg(test)]
mod quality_execute_tests;

#[cfg(test)]
mod decoder_tests;

#[cfg(test)]
mod decoded_resize_tests;

#[cfg(test)]
mod linear_srgb_tests;

#[cfg(test)]
mod working_color_tests;

#[cfg(test)]
mod color_contract_tests;
