# ADR-0003: Model demand as regions with explicit dependencies

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0004](0004-buffer-ownership-and-budget.md)

## Context

Decoders and operations expose different natural units: scanlines, rectangular regions, group- or cell-aligned blocks, and sometimes full frames. A fixed tile size or universal scanline abstraction can accidentally force wasted work. Resampling and blur also require input footprints larger than their requested output regions.

## Decision

Investigate output-driven **rectangular regions** with operators able to map an output request to source footprints, including halos, alignment, orientation and filter boundary behavior. Use logical regions in the graph; choose **physical** work units (scanline windows, codec groups, larger CPU regions or device workloads) in the planner/executor.

Important distinctions:

- *Cropped output* is not synonymous with *partial decode*.
- *Progressive preview* is not synonymous with *final-quality downsample*.
- A format's theoretical region support is not the same as the decoder API's usable capability.
- Some transforms or encoders require full-image state. Expose an **explicit materialization/fallback** instead of claiming universal streaming.

Cache only when reuse justifies its cost. Respect an overall work/memory budget to avoid multiplying halos or redundant decode work under concurrent consumers.

## Consequences

+ Enables codec-specific granularity while retaining a codec-neutral semantic contract.
+ Avoids unnecessary pixel reconstruction when an adapter can honor region requests.
- Requires careful source-coordinate math, edge policies and dependency tracking.
- Overlapping regions can increase work without appropriate scheduling or reuse.

## Alternatives considered

- **Fixed 256x256 tiles:** convenient but binds the engine to one physical geometry.
- **Only forward scanlines:** ideal for simple operations but obstructs general ROI and repeated consumers.
- **Always materialize:** greatly simplifies dependencies at prohibitive memory-traffic costs for some workloads.

## Validation

Synthetic tests for noninteger scaling, filter halos, edge handling, orientation and intersecting output requests. Track requested vs. actually decoded pixels, copies and cached regions. Use a decoder whose API demonstrably supports ROI to validate avoided work rather than just cropped output.

## Review when

Dependency mapping or overlap bookkeeping consistently dominates simple pipelines, or decoder APIs make region selection impractical across targeted formats.

## Experiment: scanline-shaped work units (2026-10-31)

The current scheduler requests **one output row at a time**, then performs backward region mapping for that row. A crop translates coordinates without copying. Bilinear resize computes the exact bounding rectangle of its two horizontal and vertical sample neighbors, with clamping at source edges. A separate upstream footprint is calculated for each row. This prevents an extreme vertical downscale from inadvertently requesting all intervening source rows, which would happen if an entire output frame were mapped to one source bounding rectangle.

This is deliberately **not** a fixed scanline architecture contract: the logical primitive remains a rectangle, and future workers may choose larger work units or multiple disjoint input spans. Horizontally sparse accesses are currently represented by one conservative bounding rectangle. Codec-native group alignment, rotation, filters beyond bilinear, repeated-ROI caching, and full-frame fallbacks are not implemented.

**New tests:** very tall synthetic input and a severe vertical downscale request at most two source rows per output row for a single bilinear resize.

## Amendment log

- 2026-10-31 — Initial proposal.
- 2026-10-31 — Added a bounded scalar vertical-slice experiment; unresolved contracts remain explicitly provisional.
