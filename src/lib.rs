//! Ferrixel — an experimental, pipeline-first image processing engine.
//!
//! This crate currently implements only an **internal** codec-free vertical
//! slice, used to validate region dependencies, ownership and a bounded CPU
//! pixel-buffer model. There is intentionally no stable public processing API.
//!
//! See `docs/architecture.md` and `docs/adr/` in the repository.

#![forbid(unsafe_code)]
// The experiment is compiled as part of the crate but deliberately has no
// public API. Remove this allowance when the first supported API lands.
#![cfg_attr(not(test), allow(dead_code))]

mod error;
mod execute;
mod geometry;
mod graph;
mod memory;
mod plan;
mod planar;
mod surface;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod planar_tests;
