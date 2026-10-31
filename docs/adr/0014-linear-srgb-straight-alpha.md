# ADR-0014: Explicit linear-light sRGB filtering with straight-alpha input

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0005](0005-codec-capabilities-and-pixel-contracts.md), [0011](0011-borrowed-planar-surface-contract.md), [0012](0012-codec-source-contract.md), [0013](0013-sample-domain-resize.md)

## Context

Interpolating gamma-encoded RGB samples is not the same as interpolating linear-light colors. Likewise, interpolating straight-alpha RGB and alpha independently can produce color fringes from invisible pixels. An unqualified "color-correct" path would be misleading while ICC transforms, HDR transfer functions, decoder output profiles and chroma semantics remain unresolved.

The first codec-backed resize contract preserves ICC/CICP metadata and explicitly labels encoded-sample interpolation. We need a narrower, genuinely validated linear-light path without accidentally treating untagged JPEGs, arbitrary ICC profiles or JPEG XL float output as sRGB.

## Decision

Provide a separate, opt-in **LinearSrgb** resize domain in the semantic graph, alongside the previous **EncodedSamples** experiment. Both use the same region planner and bounded row-oriented execution; the new mode has its own scalar pixel arithmetic.

For the initial reference path, accept only:

- Interleaved **RGB8 + Opaque** or **RGBA8 + explicitly Straight** alpha.
- A sole, unambiguous CICP descriptor **[1, 13, 0, 1]** (BT.709/sRGB primaries, IEC 61966-2-1 transfer, RGB matrix, full range).
- No competing ICC profile. Presence of ICC is **not** proof of sRGB, even when a CICP hint also exists.

The resampler performs the standard piecewise sRGB EOTF to linearize RGB; for RGBA it multiplies linear RGB by straight alpha before filtering. It bilinearly interpolates premultiplied linear RGB **and** linearly encoded alpha, divides by output alpha when positive, then applies the sRGB OETF to RGB. Fully transparent output uses canonical zero RGB. Alpha is not transfer-decoded or sRGB-encoded. Output remains straight RGBA8 or RGB8, with the same color metadata. The scalar kernel is a correctness baseline, not a production speed claim.

This is deliberately **not a general ICC-managed pipeline**. Reject missing color metadata, ICC-only or ICC/CICP combination, conflicting or non-sRGB CICP, PQ/HLG, F32, unknown/premultiplied alpha, CMYK, planar YUV, and unsupported layouts. An explicit caller-supplied *assume sRGB* policy for untagged input may be considered later in a separate ADR, but no implicit assignment exists today.

The existing encoded-sample-domain path remains an explicitly requested legacy/reference experiment; it is not automatically replaced.

## Consequences

+ Correct sRGB light mixing; avoids gamma-darkening of resized gradients.
+ Straight-alpha filtering prevents transparent RGB from producing colored fringes.
+ A distinct graph semantic allows future ICC, linear HDR and premultiplied-input paths without overloading a generic "resize" switch.
+ Decoder lifetimes, codec features and memory budgets are unchanged; source requests remain a single ROI per output.
- RGB transfer conversion is currently performed per sample in scalar arithmetic and may cost more than encoded-sample interpolation.
- Only strict CICP-sRGB-tagged U8 input qualifies. JPEG lacking trustworthy tags and JPEG XL F32 are not silently reinterpreted.
- Carrying metadata is not equivalent to transforming it. Profiles, output gamut, ICC precedence and true HDR remain future work.

## Alternatives considered

- **Treat all untagged bytes as sRGB:** rejected; missing metadata does not establish colorimetry.
- **Interpolate straight alpha in encoded RGB:** rejected; fails transparent edge tests.
- **Allow unknown or premultiplied alpha association:** rejected until association and transfer domain are explicit.
- **Implement full ICC/CICP conversion immediately:** too much external and semantic complexity before a known-good reference.
- **Normalize every frame into RGBA F32:** forces extra materialization and memory movement contrary to the project's pipeline-first design.

## Validation

Unit/integration tests must establish: the black/white midpoint is sRGB 188, not encoded-domain 128; transparent blue does not contaminate an opaque red sample when resized; zero-alpha output is transparent black; 2D interpolation is symmetric and correct; exact color/alpha restrictions fail **before** decoder work; graph color metadata is preserved; output and temporary memory are released on budget exhaustion and cancellation. Preserve existing encoded-sample, JPEG, JPEG XL and planar tests.

## Review when

An ICC color-management engine becomes available; real codecs can provide trusted sRGB output with richer metadata; HDR or F32/resampling is needed; a new alpha association is demonstrated; or measured per-pixel transfer overhead motivates LUT/SIMD specialization. Only enable fast paths with differential reference tests and reproducible measurements.

## Amendment log

- 2026-10-31 — Introduced strict, opt-in linear-sRGB bilinear reference with correct straight-alpha handling.
