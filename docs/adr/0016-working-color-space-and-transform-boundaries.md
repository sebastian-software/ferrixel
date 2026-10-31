# ADR-0016: Model a composable working-color transform boundary

- **Status:** Proposed
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0004](0004-buffer-ownership-and-budget.md), [0005](0005-codec-capabilities-and-pixel-contracts.md), [0014](0014-linear-srgb-straight-alpha.md), [0015](0015-typed-output-color-signals.md), [M1](../milestones/M1-complete-image-pipeline.md)

## Context

The current scalar `LinearSrgb` reference handles only explicitly tagged SDR sRGB samples and straight alpha. The typed color-signalling contract now distinguishes P3, PQ/HLG and ICC evidence but deliberately cannot execute conversions. The next several milestones need a coherent place for profile transforms, higher-quality resampling and encoder input negotiation.

Neither always-converting to RGBA8 nor making every codec emit a full-frame floating-point RGB buffer is consistent with Ferrixel's pipeline-first, memory-conscious design.

## Proposed decision

Investigate a **composable, semantic color-transform stage**, in which:

1. Each image edge carries independent descriptors of **pixel layout**, **color encoding or linear-light domain**, **alpha association**, **precision**, **residency** and **ownership**.
2. A transform node declares its **source and destination color meaning** (including profile/gamut, transfer, white point, and intent where relevant). It is not a side-effect or unchecked metadata rewrite.
3. The planner can choose small CPU working regions (e.g., `f32` or high-precision reference scalar pixels) while preserving the option of codec-native planar work and future GPU-resident kernels. A universal eager `RGBAf32` frame is **not** mandatory.
4. Resampling on color channels follows the declared working domain; when alpha is straight, it is made premultiplied **in linear light before filtering**. The planner makes linearization, association and final quantization explicit.
5. Out-of-gamut linear samples are **not silently clipped** in intermediate transforms. Gamut mapping/clamping and target output encoding are separate, visible decisions with tests and output metadata.
6. ICC and CICP conflicts do not get silently prioritized. Decoder adapters must declare the profile of their **rendered output**, and a future CMS backend must be supplied only after its license, safety and precision are audited.
7. PQ/HLG and HDR tone mapping require distinct reference-white/scene-display decisions, and stay unsupported by default through the SDR M1 milestone.

Initial implementation should start with a scalar SDR Display P3 (D65, sRGB transfer) to linear-sRGB reference, independently verified color matrices, and a minimal region-based transform contract. The first native ICC backend can then be evaluated (e.g. `moxcms`), but its API must not dictate graph semantics.

This ADR is **not Accepted** until the reference and at least one real codec's output profile are validated.

## Consequences

+ A sound foundation for multiple codecs and output color spaces without recreating eager full-frame buffers.
+ Color management and resampling become composable stages, leaving room for fusion without exposing backend details in the graph.
+ The planner can explain exact transforms and output profile changes.
- More explicit metadata and test fixtures; source metadata from an arbitrary codec may be inadequate.
- Floating-point working colors require precision, gamut and quantization policy.
- A conservative initial implementation will reject many real-world HDR and ICC workflows rather than guessing.

## Alternatives considered

- **Normalize all input to full-frame RGBA8:** simplest API, but discards precision and adds expensive copies.
- **Make `f32`/linear RGB the only internal pixel format:** elegant kernels but unnecessary conversions for format-native or copy-free steps.
- **Let the encoder/decoder decide all color handling:** prevents the planner from reasoning about transfer, work and quality.
- **Add ICC transforms as an invisible effect of `resize`:** hides semantic changes and cannot be reliably tested.
- **GPU-first universal shader IR:** speculative and unnecessary for the first working SDR pipeline.

## Validation

- Reference D65 Display P3-to-linear-sRGB matrix tests, gray/white invariance, out-of-gamut preservation and near-reversible transforms.
- Alpha association tests through conversion, filtering and encoding, including fully transparent pixels.
- Working-row/region memory and copy accounting, cancellation, and explicit output color metadata.
- Cross-check with an independent ICC/CMS oracle where profiles are supported.
- Exercise real JPEG and JPEG XL emitted data without ungrounded assumptions about sRGB.
- Reject missing/conflicting ICC/CICP and PQ/HLG when no explicit policy is supplied.

## Review when

Real codec and encoder dataflow makes the proposed format contract too expensive, if a CMS exposes a better typed interface, or when HDR and GPU residency require distinct working-space semantics.

## Amendment log

- 2026-10-31 — Proposed an explicit, region-capable working-color transform boundary for M1.

## 2026-10-31: narrow reference experiment (not acceptance)

A dependency-free scalar `src/working_color.rs` proof now decodes tagged sRGB or Display P3 (D65, sRGB transfer) samples into unclipped linear sRGB triples using the W3C CSS Color 4 rational reference matrices. A separate helper converts explicitly straight alpha into linear-light premultiplied samples. These values are not tied to a graph, decoder or full-frame buffer; ICC and PQ/HLG still fail closed. Independent unit fixtures verify D65 neutral preservation, a known out-of-gamut P3 red, extended signed transfer, alpha and profile rejection. **This does not yet implement the proposed composable graph transform or an encoder-ready color-managed path.** The ADR remains Proposed until real source and sink integration is proven.
