# ADR-0011: Experiment with borrowed, strided planar 10-bit surfaces

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0003](0003-demand-driven-regions.md), [0004](0004-buffer-ownership-and-budget.md), [0005](0005-codec-capabilities-and-pixel-contracts.md), [experiment notes](../design/planar-surface-experiment.md)

## Context

The Gray8 vertical slice demonstrates region demand, but assumes a contiguous 8-bit source and a single implicit color representation. JPEG XL, AVIF, and WebP decoders may provide planar, subsampled, high-bit-depth, stride-padded or borrowed data; an RGBA8-centric graph would prematurely freeze an expensive conversion boundary. Conversely, a generic all-formats surface hierarchy before any real adapter would be speculative complexity.

We need a narrow, inspectable experiment that exercises real Rust lifetimes and subsampled geometry without committing to an API or inventing a codec.

## Decision

Experiment with a semantic `Planar10Source` node carrying dimensions and explicit color metadata; **do not embed source storage in the graph**. Bind an external `Yuv420P10<'a>` view only during execution. Its Y/Cb/Cr planes borrow non-contiguous byte-strided slices containing unpacked little-endian 10-bit samples (`u16` containers). Use checked arithmetic to validate plane footprints, including odd dimensions.

For this first experiment, only even-origin crop requests may forward planar data unchanged; this preserves 4:2:0 chroma phase. The existing region planner translates crop coordinates and a separate planar executor forwards borrowed scanlines without allocating pixel intermediates. **Do not implement YUV resize yet**: independent chroma plane filtering and HDR transfer semantics require separate fidelity decisions. Keep `Graph`, pixel storage, and the implementation of execution separate. Treat color metadata as meaning, not as proof that two layouts are interchangeable.

This is a limited proof-of-contract, **not** an accepted universal surface representation or a codec capability guarantee. No crate dependency or public API is added.

## Consequences

+ Establishes a working Rust ownership boundary between immutable graph state and decoder/caller-owned pixels.
+ Verifies byte stride and planar 4:2:0 geometry without unsafe casts or RGB copies.
+ Keeps color and 10-bit representation visible to future planning decisions.
- An extra type-specific execution function exists until a real adapter motivates a general executor contract.
- Odd-origin crop and planar resizing are rejected rather than approximated.
- Only one unpacked LE format and CPU-resident byte slices are represented.

## Alternatives considered

- **RGBA8 everywhere:** simple implementation but throws away bit depth, chroma layout, and potentially expensive conversions.
- **Graph owns borrowed planes:** couples execution data lifetime to a permanent semantic graph; makes decoder reuse difficult.
- **A universal dynamic surface trait now:** more abstraction than this one-source experiment can justify.
- **Immediately enable per-plane resize:** would imply image-quality semantics that have not been specified or tested.

## Validation

Run differential scalar sample tests, zero-copy row identity checks, non-contiguous stride/odd-luma geometry tests, graph color propagation, invalid memory bounds and source-binding rejection, explicit chroma-phase errors, sink cancellation and the existing Gray8 regressions. The plan should have no new managed pixel allocations for coordinate-only crops.

## Review when

The first real decoder requires packed/tiled data or a shorter borrowing lifetime; multiple sources require a binding registry; phase-aware crops or resampling expose different semantic geometry; or the type-specific source executor starts duplicating significant planning logic.

## Amendment log

- 2026-10-31 — Introduced a narrow borrowed YUV420P10 source and crop-only experiment. Retain broader codec and color negotiation decisions as Proposed under ADR-0005.
