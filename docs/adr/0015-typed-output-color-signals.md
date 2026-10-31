# ADR-0015: Inspect rendered color signals before selecting a color operation

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0005](0005-codec-capabilities-and-pixel-contracts.md), [0012](0012-codec-source-contract.md), [0013](0013-sample-domain-resize.md), [0014](0014-linear-srgb-straight-alpha.md)

## Context

The decoder contract carries raw `rendered_icc` and `rendered_cicp` values and the graph preserves both. The first linear-light reference is restricted to explicit sRGB CICP, but a byte comparison alone cannot tell the planner why Display P3, Rec.2020 PQ/HLG, ICC or conflicting metadata need different work. Worse, a decoder may already have converted its output colors, so using original-file metadata in place of the profile of **emitted samples** can silently corrupt results.

ICC profile parsing and ICC color transformation are not equivalent. A profile with a plausible 128-byte header is not automatically safe, complete, or equivalent to a named color space.

## Decision

Introduce a small, dependency-free **inspection layer**, separate from pixel conversion, the semantic image graph and codec storage.

1. Parse H.273/CICP as typed primary, transfer, matrix and Boolean range fields. Preserve unknown code points, including unsupported matrices or limited-range sRGB-like values; do not "normalize" them to sRGB.
2. Classify the exact signaling combinations currently relevant to our pipeline: RGB full-range sRGB `[1,13,0,1]`, Display P3 `[12,13,0,1]`, and BT.2100 PQ/HLG with recognized RGB or BT.2020 YCbCr matrices. These are *descriptors*, not implementations.
3. Inspect ICC data with checked lengths, the `acsp` signature, declared size and tag-table boundary; extract only header fields. **Do not claim this validates a whole ICC profile or allows any ICC conversion.** Full parsing and transform correctness belong to a future color management engine.
4. When both ICC and CICP are present, preserve both. No default precedence or equivalence is assumed; the selected decoder and its output-profile semantics must define that policy.
5. Return a typed *linear-sRGB readiness/requirement* decision: reference ready, gamut conversion needed, HDR handling needed, ICC engine needed, dual-signal resolution needed, missing metadata, unsupported signal, or malformed metadata.
6. Use that decision as the **single source of truth** for the existing strict `LinearSrgb` graph operation and executor guard. All non-ready cases remain rejected. Existing `EncodedSamples` execution is unchanged and still does not imply color fidelity.

`DecoderInfo` must describe *rendered output pixel values*, not merely original encoded tags. `DecoderInfo::rendered_color_signal` avoids copying profile payloads when inspecting. For example, if a JPEG XL adapter requests a converted output profile through a CMS, the resulting emitted pixel metadata must match that output profile.

This is an **experimental internal contract**, not a supported API, and does not bring a CMM, tone mapper, ICC transform, SIMD/GPU implementation, or new dependency.

## Consequences

+ Establishes actionable, observable reasons for color conversions without prematurely choosing a CMM or backend.
+ Prevents silent gamut/transfer errors and preserves metadata across cropped regions.
+ Distinguishes code-point classification from actual decoder work and from color-managed pixel transformation.
+ ICC inspection is bounded and does not materialize or convert pixels.
- Header-level ICC inspection cannot check all tags, profile validity, rendering intent or color-equivalence.
- The current source/graph still clone ICC payload into `Arc` storage, which must be capped and accounted for in a future resource-safety review.
- ICC/CICP precedence, arbitrary ICC, HDR reference white, out-of-gamut handling, and alpha in linear working space remain unsolved.

## Alternatives considered

- **Assume sRGB if tags are absent:** rejected; not grounded in rendered pixel semantics.
- **Treat ICC and CICP as automatically equivalent:** rejected; profile precedence depends on format and decoder configuration.
- **Implement arbitrary ICC parsing or GPU shaders now:** rejected; unrelated complexity without correctness or licensing validation.
- **Normalize all decoder output to RGBA8:** rejected; loses high-bit-depth/HDR information and increases memory traffic.

## Validation

- Characterize sRGB, P3, PQ, HLG, limited-range and unsupported matrix code points independently.
- Reject malformed CICP range flags, truncated ICC headers, invalid signatures, impossible declared sizes, and tag-table overflow.
- Verify that ICC-only and ICC+CICP cannot qualify for the scalar sRGB reference merely because their bytes appear to match a tag.
- Verify color evidence is preserved over crop and that the graph refuses unsupported gamut conversion before invoking any codec.
- Preserve tests for the sRGB linear-light reference, planar YUV, JPEG/JPEG XL source lifetimes and encoded-sample resize.

## Review when

A real decoder exposes a usable output-profile transform API; an ICC parser/CMM is selected; a reference-white and output-intent model for PQ/HLG is defined; metadata sizes cause large allocations; or evidence demonstrates that an explicitly supported profile precedence policy can be standardized per adapter.

## Amendment log

- 2026-10-31 — Added explicit, non-transforming color evidence classification.
