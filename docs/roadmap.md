# Implementation roadmap

**Current milestone:** [M1 — First complete image pipeline](milestones/M1-complete-image-pipeline.md) (planned). It turns the current architecture probes into a usable SDR JPEG/JPEG XL → AVIF tool through ten gated work packages. Its acceptance criteria and dependency ordering supersede the earlier broad phase summaries below; these remain as research/progress history, not a contradictory second task list.

[Encoder/CMS candidates and licensing](research/2026-10-31-encoder-and-cms-candidates.md) identify a permissively licensed AVIF path, a candidate ICC backend, and a JPEG XL encoder license gate. See [ADR-0016](adr/0016-working-color-space-and-transform-boundaries.md) (Proposed) for the working-color design contract.

*Sequence, not a deadline or a commitment to a fixed API.* Progress depends on tests and architectural evidence, not feature count.

## 0. The smallest correct pipeline — experimental implementation

- Typed geometry: rectangles, bounds, coordinate transforms, input footprints and filter halos.
- Minimal image graph with synthetic sources (no codecs required).
- A region-requesting consumer and a simple CPU executor.
- Safe view/borrow contracts and an observable allocation counter.
- Golden tests for crop and one reference resize algorithm; no throughput claims.
- A readable plan report showing actual work, materializations, peak engine-managed memory, and unsupported fast paths.

**Progress (2026-10-31):** The scalar Gray8 synthetic → crop → bilinear resize → borrowed scanline sink implementation validates region planning and bounded managed pixel buffers. A subsequent [borrowed planar source probe](design/planar-surface-experiment.md) integrates 10-bit YUV 4:2:0 planes with byte strides, explicit color metadata and even-origin zero-copy crop forwarding. Neither experiment validates real decoder APIs or phase-aware/HDR resizing. The next review should test a real codec lifetime and output-format contract before treating the designs as accepted architecture.

**Exit criterion:** a synthetic pipeline requests only necessary upstream regions and passes strict semantic/ownership tests without allocating full-frame intermediate images.

## 1. Real format-informed vertical slice

- Evaluate **one** modern decoder and **one** output sink from JPEG XL, AVIF and WebP according to usable APIs, licensing, correctness and accessible partial-decode capabilities.
- Measure full decode versus limited decode where the implementation genuinely supports it.
- Validate pixel contracts, orientation, alpha and color handling.
- Connect bounded scheduling and backpressure to concrete codec behavior.

**Exit criterion:** a real input-to-output pipeline with trustworthy resource accounting and no unjustified copies; graceful fallback where codec-specific optimizations are unavailable.

## 2. Pipeline optimization by evidence

- Native-layout negotiation and conversion avoidance where quality is maintained.
- Correct, reproducible resampling benchmarks on several image classes and sizes.
- Limited SIMD variants only for proven hotspots with a reference implementation.
- Task scheduling and buffer reuse tuned with both latency and multi-image throughput.

**Exit criterion:** measured improvement without quality or memory regressions, with planner explanations for every fast path.

## 3. Conditional GPU experiments

- Model device residency, upload/readback cost and synchronization explicitly.
- Prototype one large, repeated operation (e.g., resampling plus color transform).
- Compare end-to-end time and resource use against CPU on representative hardware.
- Preserve the CPU-only configuration as the baseline.

**Exit criterion:** an independently measured workload where device execution wins enough to justify maintenance and portability costs.

## Not prioritized initially

Historical JPEG output tuning, compatibility with the complete libvips or Sharp surface, hundreds of image operations, generalized image editing, custom codecs, FFI-heavy dependencies, and a universal compute DSL.

## Decoder adapter experiment

Read-only JPEG and JPEG XL source adapters are introduced behind Cargo features. They validate cropped source-to-sink execution and managed vs. opaque memory boundaries; they do **not** complete phase 1's color, orientation, malformed-input and full encoder requirements. See [ADR-0012](adr/0012-codec-source-contract.md).

## 2026-10-31: Real decoder resize experiment

An explicitly selected sample-domain bilinear `JPEG/JPEG XL decoder → crop* → resize → crop* → borrowed row sink` path now probes real codec output and end-to-end row lifetimes. Rendered ICC/CICP and alpha association are tracked through the graph and verified on binding. No color-managed filtering, alpha-aware compositing or HDR conversion is claimed. The next work should evaluate quality-correct linear-light and premultiplied-alpha paths and measure region overfetch rather than adding codec support for its own sake. See [ADR-0013](adr/0013-sample-domain-resize.md).

## Color fidelity experiment (2026-10-31)

The [first opt-in linear-light sRGB path](design/color-correctness-experiment.md) introduces a correct transfer-function and straight-alpha bilinear reference for strictly tagged RGB8/RGBA8. No ICC parser, gamut conversion, HDR transfer processing or encoder path is available. The next color milestone is an explicit trusted **decoder output profile / color transform** contract, not treating untagged JPEG or JPEG XL float samples as sRGB. Correctness fixtures should precede SIMD and antialiasing optimizations. See [ADR-0014](adr/0014-linear-srgb-straight-alpha.md).

## Explicit color evidence (2026-10-31)

[ADR-0015](adr/0015-typed-output-color-signals.md) records the next experiment: classify **rendered** decoder color signals without changing pixels. The shared contract identifies sRGB, Display P3, BT.2100 PQ and HLG, preserves unsupported code points, inspects only ICC header/tag-table bounds, and refuses to guess precedence when ICC and CICP are both present. The linear-sRGB path delegates eligibility to that contract; all other conversion capabilities remain unavailable. Before admitting ICC or HDR transforms, specify the CMM's supported profile classes, pixel and reference-white domains, output metadata, black-point/intent policy, memory accounting, licensing and differential color fixtures.
