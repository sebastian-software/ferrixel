# ADR-0012: Bind real codecs through a capability-reporting source contract

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0003](0003-demand-driven-regions.md), [0004](0004-buffer-ownership-and-budget.md), [0005](0005-codec-capabilities-and-pixel-contracts.md), [0010](0010-dual-licensing.md)

## Context

Synthetic sources prove region propagation but not the integration boundary with real codecs. Codecs differ in supported image layouts, metadata, buffer lifetimes, memory use and their ability to honor region requests. A crop API alone does not imply work was saved.

## Decision

Experiment with a **private** `DecodeSource` contract, independent of the graph and executor. Its reported capabilities include image dimensions, channel layout, emitted sample type, rendered color metadata, and an explicit access class:

- **FullFrameFallback:** the adapter must reconstruct the entire image before exposing the requested region.
- **CodecRegionRequest:** the adapter passes an ROI to the codec API; this makes **no guarantee** about skipped entropy decode, coefficient reconstruction, decoder allocations, or compute cost.

The existing graph planner maps requested output rectangles backwards through crop nodes. A codec source node stores no encoded bytes, decoder handles, or borrowed pixel memory. The source is bound at execution time and sends ephemeral borrowed scanlines through a callback sink; callback errors propagate as cancellation/backpressure.

Two initial **read-only** adapters validate the contract:

1. **JPEG** via optional `zune-jpeg` 0.5.15: header probe, RGB8 output, optional embedded ICC. Full RGB output is decoded directly into an engine-budgeted buffer; requested crop rows borrow from that buffer. Disable architecture-specific unsafe-intrinsic features for this first experiment. This does **not** create a JPEG encoder.
2. **JPEG XL** via optional `jxl-oxide` 0.12.6: request a crop using the codec's ROI API; stream f32 samples via a single budgeted row buffer. Retain rendered ICC/CICP metadata. For now reject non-default orientation rather than risk incorrect coordinates.

The graph currently carries sample layout, but **not** a complete graph-owned color/profile contract. Only crop and sink are permitted for these sources until output semantics for resize, color and alpha are properly designed.

## Memory and safety

The engine's RAII `MemoryBudget` covers allocations it creates, including the RGB8 JPEG frame and f32 JPEG XL row scratch. It **does not cap or measure** codec-internal scratch, retained compressed data, metadata vectors, or whole-process RSS. A report with zero *known* full-frame buffers is not evidence that a codec created none internally.

The Ferrixel crate continues to forbid `unsafe` locally. Do not equate that with a guarantee that all dependencies avoid unsafe code, or that arbitrary malicious inputs have been hardened and fuzzed.

Both selected adapters are permissively licensed. Do not integrate AGPL/commercial AVIF libraries into the MIT OR Apache-2.0 package without an explicit license decision and dependency audit.

## Consequences

+ The graph stays codec-independent while the planner can observe known fallback costs.
+ JPEG read support is practical without optimizing the entire engine around it.
+ JPEG XL's float and region capabilities can be tested without forced RGB8 quantization.
- JPEG currently decodes a full frame for every independently requested crop.
- JPEG XL may perform opaque allocations or whole-image work despite the ROI API.
- Color metadata is source-side rather than a full semantic graph contract.
- No encoder, frame reuse, animation, or arbitrary untrusted-image hardening yet.

## Alternatives

- **Universal RGBA8 decoder:** straightforward but forces color conversions and loses HDR precision.
- **Treat every cropped output as partial decode:** misleading and impossible for many real files.
- **Codec-specific graph nodes:** couples graph semantics to arbitrary backend APIs.
- **Add every codec first:** makes it difficult to isolate the architectural problems.

## Validation

Exercise real small JPEG and JPEG XL fixtures, invalid input, region geometry, row typing, source binding, early cancellation and budget failures in feature-gated tests. Compare cropped pixels against full-frame decodes, with a documented numerical tolerance for floating-point output. CI must test the no-codec configuration and both optional codec adapters. Inspect codec and dependency licenses separately.

Before promotion to Accepted: validate actual codec work avoided, orientation/ICC/CMYK/HDR handling, adversarial dimensions and malformed files, memory usage inside decoders, output fidelity and a format-independent transform path.

## Review when

A source requires native planar output, push/pull access rather than callback rows, precise codec allocation accounting, or more complete color semantics to avoid incorrect transformations.

## Amendment log

- 2026-10-31 — Initial experimental contract with JPEG read-only fallback and JPEG XL region requests.
