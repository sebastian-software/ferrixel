# Region-planning experiment: executable contract

**Status:** Internal architecture experiment, 2026-10-31. Not a public API, not a codec adapter, not a throughput claim.

## Goal

Verify that a small immutable image graph can be planned backward from output demand and executed safely with bounded *engine-managed pixel buffers* without materializing the full source or every intermediate image. This validates the basic shape of [ADR-0002](../adr/0002-semantic-graph-and-planner.md), [ADR-0003](../adr/0003-demand-driven-regions.md) and [ADR-0004](../adr/0004-buffer-ownership-and-budget.md). All three remain **Experimental**, not Accepted.

## What is implemented

- `Graph`: append-only node construction, with source, crop and bilinear resize. Parent indexes must refer to already-added nodes, keeping the graph acyclic. Dimension and crop bounds validation is checked.
- `Rect`: half-open, integer image-coordinate regions. A requested crop translates by its top-left offset. No physical tile size is part of the graph semantics.
- `plan_region`: walks requested output regions backward to the source, using exact pixel-center bilinear sample footprints and edge clamping. The resulting steps are a **temporary** plan for a region; no persistent IR is required yet.
- `execute`: requests each output scanline separately. It then executes only that scanline's dependency plan. The output sink receives a borrowed row, allowing the caller to stream without allocating a final image in Ferrixel.
- `MemoryBudget`: RAII-reserved pixel byte buffers with an atomic shared cap, high-water mark, allocation count, and safe release on failure/cancellation.
- Scalar Gray8 bilinear reference operation. Pixel coordinate convention is `(output + 0.5) * source / destination - 0.5`, with edge clamping; this is a **defined test baseline**, not yet a quality recommendation for real photographs or HDR.

## Intentional boundaries and limitations

- **No codec integration**: real partial decoding, native planes, chroma and alpha remain unvalidated.
- **No stable public API**: the experimental graph, plan, pixel buffers and sink are crate-private. No new crates or dependencies.
- **No strict process memory cap**: only allocations made via `MemoryBudget` count. Graph/planner metadata, source/decoder internals, sink-retained output, allocator overhead and OS/GPU allocations are excluded.
- **No universal zero-copy**: crop simply rebases the owned region; resize allocates an output region and reads from the current input. Where full materialization is necessary, the future planner must make it visible.
- **No multi-consumer caching**: a graph chain is executed sequentially. Repeated source footprints are regenerated, not cached. A future scheduler must measure caching versus redundant work.
- **No sparse horizontal region set**: horizontal sampling footprints are bounded by one rectangle, potentially generating more pixels than a sparse sampler would need.
- **No global quality guarantees**: the scalar Gray8 baseline is distinct from high-quality, color-managed resizing and should not be silently promoted to a production resampler.
- **No GPU, SIMD, threading or async scheduling**: adding these must not change the semantic graph contract.

## Test invariants

1. A requested ROI is validated before work; erroneous geometry returns a typed error.
2. A crop produces the correct coordinate-rebased values without a crop-owned pixel buffer.
3. Resizes, including chained resizes, match an independently implemented eager reference.
4. A very tall synthetic source does not require height-proportional buffers for a small output ROI.
5. Aggressive vertical shrink does not turn disjoint sampled source rows into a full-height buffer.
6. A failed allocation or cancelled output sink releases **all** outstanding managed pixel-buffer reservations.
7. A shared budget cannot be overcommitted by independent concurrent holders.
8. The report identifies pixel work and allocation counts; it does **not** pretend to be whole-process memory telemetry.

## Next design probes

- Add a synthetic planar 10-bit source with meaningful strides/borrowed planes to test the actual pixel representation contract before selecting a codec.
- Test a branching graph: two output consumers requesting overlapping parts of a shared source; compare recomputation against a bounded shared cache.
- Integrate exactly one capable modern codec (JPEG XL, AVIF or WebP) behind a capability-reporting adapter, verifying actual API support for requested regions and native data layout.
- Explore alternative work-unit shapes based on measured allocations and dependency inflation before stabilizing a scheduler.
