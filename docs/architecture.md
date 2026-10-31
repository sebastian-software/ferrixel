# Architecture sketch

*Status: exploratory. This page summarizes design intent, not an implementation contract.*

## Core separation

```text
                       Declarative image graph
            source ─── transform ─── transform ─── sink
                                   |
                                   v
                      Capability-aware planner
             format  ·  color  ·  geometry  ·  cost
                                   |
                                   v
                       Executable region plan
               task DAG  ·  ownership  ·  memory budget
                          /                 \
                         v                   v
                CPU execution           Future GPU execution
               scalar / SIMD          device tasks / textures
                          \                 /
                           v               v
                            Output consumer
```

The architecture has four conceptual layers:

1. **Graph:** immutable semantic image operations, independent of particular hardware and codec implementations. Nodes describe *what image they produce* rather than a buffer they own.
2. **Planner:** combines requested outputs with operation dependencies, codec capabilities, color semantics, and resource constraints. Produces an explainable execution plan; an optimization is valid only if it respects observable semantics.
3. **Executor:** executes work on regions/scanlines or device workloads under explicit lifetime, concurrency, and memory constraints. The CPU backend comes first; GPU execution is conditional on end-to-end benefit.
4. **Adapters:** decoders, encoders, sources, output sinks and device interfaces. Adapters publish real capabilities. The format specification alone is not proof that an implementation supports a fast path.

## Demand-driven example

For a request to crop an AVIF image, resize it, and encode a JPEG XL thumbnail:

```text
required output region
  <- resampling source footprint, plus filter halo
  <- crop coordinate mapping (respecting orientation semantics)
  <- decoder region, rounded to decoder's supported granularity
  <- encoded source
```

The engine must distinguish a decoder that can *avoid reconstructing* irrelevant regions from a decoder that only hides them after full reconstruction. A plan may need full-frame decode for some images. This is an explicit cost, not an invisible violation of the contract.

## A first-class pixel contract

A surface needs more than width and height:

- dimensions and coordinate space, pixel/plane strides and alignment;
- planar or interleaved channel layout and sample representation;
- bit depth and sample range, including float where required;
- color primaries, transfer characteristics, color matrix, chroma subsampling/siting;
- alpha mode (straight/premultiplied) and orientation/metadata semantics;
- residency (CPU / device), ownership, lifetime and synchronization requirements.

These are semantic requirements, not an invitation to implement every possible layout in the first version. Add representations as a real codec-to-operation path requires them.

## Memory and scheduling

- **No full-frame intermediate by default.** Region views and borrowed input should be normal; explicit materialization is permitted when unavoidable or measurably beneficial.
- Use a **memory budget** for engine-managed allocations, including concurrent in-flight tasks. Estimate or constrain opaque decoder/encoder memory separately; never claim total-process bounds that cannot be enforced.
- Represent filter **halos**, alignment and access patterns in dependencies; no single physical tile size is mandated.
- Make cache and intermediate reuse evidence-driven. A region cache is helpful for overlapping consumers, but can hurt one-shot streaming workflows.
- CPU/GPU transfers are explicit plan edges with costs. Avoid backend switches that erase gains through copying and synchronization.

## Open questions

1. What is the minimum *semantic* node contract that supports region dependencies without committing to a giant public DAG API?
2. How do we represent decoder capability granularity and quality (exact ROI versus preview versus full-resolution)?
3. How do we preserve color correctness while deciding between planar native fast paths and RGB/linear-light paths?
4. Can executor-owned buffers be borrowed safely across asynchronous tasks without overconstraining scheduling?
5. When does the additional complexity of a GPU path pay for itself over a well-planned CPU path?

These questions are tracked in [the ADRs](adr/README.md), not decided by this sketch.

## First executable experiment (not a stable contract)

The internal Rust implementation in `src/` tests the four layers using a **single-chain Gray8** graph with synthetic random-access source, crop, scalar bilinear resize, and a borrowed scanline sink:

```text
caller's requested output rectangle
    → schedule one output row
    → backward region footprint through resize and crop
    → synthetic source generates only that footprint
    → crop rebases the region without copying
    → resize allocates just its required output row
    → sink borrows that row; both input/output reservations are released
    → repeat with the next output row
```

**Why one row?** With extreme downsampling, two consecutive target rows can sample source rows millions of pixels apart. Taking their union as one source rectangle materializes unnecessary intervening scanlines. Planning each requested row avoids that specific trap; future planners may merge adjacent rows only when doing so is worthwhile.

This experiment has no codec integration, advanced color model, multi-input branching, parallel worker scheduler, or GPU support. The budget currently tracks explicitly allocated **pixel bytes only**, not metadata, codec internals, or sink-owned buffers. This limitation must remain visible when comparing real workloads. See [the experiment's contracts](design/region-experiment.md) and ADRs [0002](adr/0002-semantic-graph-and-planner.md), [0003](adr/0003-demand-driven-regions.md), and [0004](adr/0004-buffer-ownership-and-budget.md).
