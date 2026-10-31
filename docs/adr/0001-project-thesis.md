# ADR-0001: A pipeline-first engine for modern image processing

- **Status:** Accepted
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0005](0005-codec-capabilities-and-pixel-contracts.md), [0007](0007-correctness-and-performance-evidence.md)

## Context

Image libraries often expose individual operations or universal pixel buffers as their fundamental abstraction. libvips demonstrates that demand-driven, region-oriented pipelines can avoid expensive intermediate images and improve locality. A Rust implementation can revisit these principles without inheriting historical compatibility requirements. Modern formats and heterogeneous hardware make the shape of the dataflow at least as important as the speed of a filter.

The project's purpose is sustainable architecture over a roughly five-year horizon, not winning a particular short-lived benchmark.

## Decision

Build **Ferrixel** as a safe-Rust, pipeline-first image processing engine. Treat the **pipeline** as the product: a semantic graph describes requests, a planner minimizes work and memory movement, and backends execute operations. Defaults should avoid materializing complete intermediate images when demand-driven regions suffice.

Prioritize realistic paths for **JPEG XL, AVIF and WebP** and the capabilities of their actual decoder/encoder implementations. Optimize the core for modern CPUs and allow a later GPU backend when justified by measurement. Treat SIMD as a measured operation-level implementation strategy, not a defining public abstraction.

Explicitly prioritize architectural clarity, correctness and maintainability over the last few milliseconds. Do not define success as replacing all libvips or Sharp APIs or matching JPEG-specific benchmarks. No promise of universal zero-copy, universal streaming, or end-to-end memory safety is implied for unreviewed third-party codec implementations.

## Consequences

+ An architecture can survive codec and hardware changes.
+ Fewer compatibility constraints; a small initial scope is legitimate.
+ Optimization choices become explainable at the pipeline level.
- Requires an explicit planner, dependency semantics and careful color/memory contracts.
- Best-case performance may initially trail mature C/C++ libraries or dedicated codecs.

## Alternatives considered

- **Port libvips:** inherits decades of API and internal choices; useful as prior art, not the objective.
- **Build a codec library first:** optimizes the wrong boundary for our goals.
- **Build a generic eager image-buffer utility:** simpler initially but locks in unnecessary intermediate materialization.

## Validation

The initial vertical slice must compute a region of a transformed image without creating full-frame intermediates; report allocations and planner choices; match a trusted correctness reference. Evaluate the design with at least two format families before stabilizing a public API.

## Review when

User-facing workloads demonstrate that eager full-frame processing is consistently simpler *and* measurably superior, or the chosen abstraction makes basic format adapters unreasonably complex.

## Amendment log

- 2026-10-31 — Accepted the initial project direction.
