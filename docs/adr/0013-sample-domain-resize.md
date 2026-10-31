# ADR-0013: Carry rendered color semantics and validate bounded decoded resize

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0012

## Context

The first real decoders returned interleaved U8 or F32 rows, but the image graph only retained their sample layout, not the exact rendered ICC or CICP metadata. Adding a generic resize without declaring its color and alpha semantics could silently change visual results or imply HDR fidelity where none was implemented. JPEG also requires an explicitly full-frame fallback; making a decoder call per output row would repeat this expensive work.

## Decision

Introduce an internal image-semantics descriptor on graph nodes, separate from sample layout, storage, and codec lifetimes. Carry exact decoder-provided ICC bytes and/or CICP identifiers through crops and resizes without interpreting or normalizing them. Share ICC bytes through Arc-backed storage. Missing color metadata remains Unspecified, never implicitly sRGB. A decoder with alpha channels but unspecified association must be marked as such, not assumed premultiplied or straight. Check the descriptor as part of source binding before producing pixels.

Support an **explicitly requested encoded-sample-domain bilinear reference** for interleaved Gray/RGB U8 and F32 with no alpha. The implementation interpolates numeric samples as emitted; it does not transform ICC profiles, linearize the transfer function, tone-map HDR, or handle alpha. F32 preserves finite negative and greater-than-one samples; non-finite values are rejected. U8 uses explicit rounding and clamping.

Use the existing region planner to request a single bounding source rectangle for the output ROI. One decoder call streams through a single previous input row and a single output row in engine-budgeted memory. That ensures JPEG's full-frame fallback is decoded once per output region. For JPEG XL the bounding rectangle may overfetch unrelated rows; avoid claiming avoided codec-internal work without measurement.

Keep planar YUV420P10, CMYK, alpha-bearing layouts, arbitrary orientation and multiple successive resizes unsupported at this stage. No new external dependency, public API, SIMD implementation or GPU backend is added.

## Consequences

- The graph carries meaningful, unmodified color provenance across operations, independent of decoder-owned storage.
- Real JPEG and JPEG XL can validate a bounded transform path rather than just crop forwarding.
- The scalar reference provides a correctness baseline for future kernel optimization.
- This is **not** a general color-managed resizer; encoded-domain interpolation may differ perceptually and numerically from linear-light resampling.
- One bounding ROI can read large intervening areas during heavy downscale; smarter scheduling remains an open problem.
- Engine memory accounting cannot enforce a hard bound on opaque decoder-internal allocations.

## Alternatives considered

- Assume sRGB and silently resize everything: rejected because absent metadata is not proof of sRGB.
- Full ICC and HDR processing first: deferred until the execution/ownership contract is verified.
- Request each output scanline from every codec: rejected as a JPEG decode-regression trap.
- Eagerly materialize a complete intermediate image: rejected as the default because it hides pipeline costs.
- Immediately support alpha, CMYK, YUV and GPU: deferred pending explicit quality semantics.

## Validation

Differential tests compare nested crop and resize chains to an independent scalar reference; cover float values outside [0,1], metadata preservation/mismatch rejection, alpha refusal, budget exhaustion, cancellation and both real read-only adapters. Report decoder accesses, known full-frame fallback buffers, output rows and managed memory separately.

## Review when

Codec-native scaling or regional execution offers a better tradeoff; linear-light filtering, ICC/CICP precedence, or premultiplied alpha become implemented; a real decoder exposes a new sample layout; or severe downscales reveal that the bounding-region strategy is too expensive.

## Amendment log

- 2026-10-31 — Initial bounded, codec-backed sample-domain resize experiment.
