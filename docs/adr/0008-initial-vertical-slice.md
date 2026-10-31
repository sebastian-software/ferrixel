# ADR-0008: Validate the architecture with a minimal vertical slice

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0003](0003-demand-driven-regions.md), [0004](0004-buffer-ownership-and-budget.md)

## Context

The project could lose years building generic extensibility, GPU frameworks or codec-specific optimizations before demonstrating that the basic demand-driven ownership model works. The safest way to test the architecture is end to end with the fewest real moving parts.

## Decision

Start with a **synthetic, codec-free source → crop → reference resize → sink** pipeline. A synthetic source enables precise accounting of region requests and pixel work. Provide an executor-owned budget and plan trace. The first real adapter should be selected based on decoder API quality and licensing, with a preference for modern JPEG XL, AVIF or WebP paths rather than JPEG benchmark parity.

Do not add a public fluent API, Node.js bindings, CLI, GPU device abstraction, decoder registry or multi-format benchmark suite as initial prerequisites. Start as a single Rust crate and split crates only after a demonstrable dependency or build boundary emerges.

## Consequences

+ Tests the most important contracts before introducing codec and FFI complexity.
+ Gives a baseline for measuring region pruning and allocations.
- Early demos will not process arbitrary image files and should not be presented as feature-complete.

## Alternatives considered

- **Build a polished CLI first:** pushes interface work ahead of execution fundamentals.
- **Integrate every modern codec first:** turns the project into an adapter exercise.
- **GPU and SIMD from day one:** masks algorithmic and scheduling flaws.

## Validation

Acceptance test: a small cropped output of a very tall synthetic image only requests its mathematically necessary source footprint; crop and resize agree with a trusted full-frame reference; full-frame intermediate allocations remain zero; cancellation and over-budget behavior are safe. Make the full request graph and execution trace inspectable.

## Review when

Synthetic-source tests prove misleading for a real codec's granularity or the chosen first adapter exposes an unmodeled dependency class.

## Experiment scope (2026-10-31)

The first internal implementation is intentionally **Gray8-only**, uses a scalar pixel-center bilinear reference resize, and supports a single chain of `synthetic → crop → resize → sink` with optional additional crops/resizes. There is no public API or codec. A per-row planner and non-retaining sink callback avoid output-sized allocations; RAII-tracked input/output region buffers enforce a shared managed-pixel-memory ceiling.

Tests include an independent eager resize oracle, chained resize + downstream crop, 100-million-row synthetic input without source materialization, severe vertical downscale without expanding intermediate row gaps, invalid geometry, memory pressure, sink cancellation, and atomic reservation safety. These test the architectural contract, **not** real decoder behavior or image quality across color spaces.

The immediate next research question is whether codec adapters can expose region-granular decoded planes without breaking this ownership model. Before a public API or a stable region scheduler, test noncontiguous inputs, encoded output backpressure, format-native planes, and two consumers of a shared node.

## Amendment log

- 2026-10-31 — Initial proposal.
- 2026-10-31 — Added a bounded scalar vertical-slice experiment; unresolved contracts remain explicitly provisional.
