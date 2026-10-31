# Implementation roadmap

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
