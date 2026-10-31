# ADR-0005: Codec capability negotiation and explicit pixel semantics

- **Status:** Proposed
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0001](0001-project-thesis.md), [0002](0002-semantic-graph-and-planner.md), [0003](0003-demand-driven-regions.md), [0011](0011-borrowed-planar-surface-contract.md)

## Context

Modern codecs expose different ways to avoid work. JPEG XL, AVIF and WebP also differ in native layout, bit depth, alpha, progressive behavior and accessible decode granularity. Requiring a single RGBA8 buffer between every stage adds conversions and can destroy precision. Equally, operating directly on YCbCr can change quality unless chroma position, color range and transfer semantics are respected.

## Decision

Explore **versioned, capability-reporting adapters**. The planner may request a region, target resolution, representation or progressive quality only when the specific decoder API advertises it; adapters must accurately declare fallback costs and granularity. Encoders similarly declare acceptable inputs, buffering requirements and streaming properties.

The core's pixel contract must account for:

- dimensions, coordinate spaces, orientation and plane stride;
- sample representation, bit depth, range and channel/plane layout;
- color primaries, transfer function, color matrix, chroma subsampling and siting;
- alpha semantics and precision/quality requirements;
- residency and whether a view is borrowed, owned or device-resident.

Treat layout negotiation as an **optimization under correctness constraints**. Do not assume YUV-to-YUV resizing is automatically equivalent to a chosen RGB/linear-light reference. Where quality requires a conversion, perform it and record it in the plan. Prioritize modern format families; historical JPEG-specific optimization is not a launch requirement.

## Consequences

+ Supports native-layout fast paths and reduces format-specific assumptions in the core.
+ Makes fidelity requirements visible to planner optimizations.
- More metadata and color-space test cases than a fixed RGBA8 interface.
- Capability flags alone may be too coarse; actual cost and quality can be contextual.

## Alternatives considered

- **All RGBA8:** easy operations; expensive conversions and poor HDR/high-precision support.
- **Expose internal codec objects to every operation:** ties the engine to specific implementations.
- **One universal decoder trait with unconditional ROI guarantees:** not implementable for many real codec APIs.

## Validation

For each adapter record decoder version/license/safety, output layout, region granularity, full-frame fallback, progressive quality semantics and opaque memory. Differential tests should include 8/10/12/16-bit inputs as supported, ICC/color profiles, chroma siting, alpha, HDR, rotations and cropped edges. No optimization is accepted solely because it is faster at visibly different quality.

## Partial experiment (2026-10-31)

[ADR-0011](0011-borrowed-planar-surface-contract.md) introduces borrowed, byte-strided YUV420P10 planes and explicit color metadata into a crop-only execution path. This validates one storage/lifetime contract, **not** generic codec capability negotiation, color conversion, or chroma-aware resampling. This broader ADR remains Proposed.

## Review when

Real adapters require substantially different contracts or the model forces unnecessary color conversions in common modern-format workflows.

## Amendment log

- 2026-10-31 — Initial proposal.
- 2026-10-31 — Linked the experimental planar-source probe; codec negotiation remains open.

## 2026-10-31: experimental progress (not acceptance)

[ADR-0013](0013-sample-domain-resize.md) now carries exact rendered ICC/CICP bytes and uncertain alpha status through graph nodes and tests a deliberately *encoded-sample-domain* bilinear resizer. These facts **do not** resolve ICC/CICP precedence, transfer-function linearization, HDR fidelity or premultiplied alpha. This ADR remains Proposed.

## 2026-10-31: reference color-fidelity boundary

[ADR-0014](0014-linear-srgb-straight-alpha.md) demonstrates an opt-in linear-light bilinear path for **unambiguous CICP sRGB RGB8 and straight RGBA8**. No other metadata can be implicitly converted, and the existing more general color-management decision remains Proposed. The experiment establishes a baseline for alpha-safe and transfer-aware processing, not complete ICC, HDR, or native YUV pipeline negotiation.

### Channel identity as part of sample layout

The sRGB-alpha experiment exposed a concrete architectural ambiguity: `InterleavedU8(4)` cannot distinguish RGBA from CMYK. The core now retains the `ChannelLayout` identity in each interleaved `PixelFormat`, and the explicit color reference only accepts `Rgb`/`Rgba`. This does not yet resolve wider color profile conversion or planar sample meaning; this ADR remains Proposed.

## 2026-10-31: typed output color evidence

[ADR-0015](0015-typed-output-color-signals.md) adds code point classification and no-allocation ICC header inspection to distinguish sRGB, Display P3, BT.2100 PQ/HLG and untrusted/dual profile signals. Its planning result only describes **requirements**, not a working CMM. Decoder adapters must describe emitted samples after any codec-level color conversion. The decision about a full working-space, ICC precedence and HDR reference luminance remains Proposed.
