# ADR-0002: Separate the semantic graph from the execution planner

- **Status:** Proposed
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0001](0001-project-thesis.md), [0003](0003-demand-driven-regions.md), [0005](0005-codec-capabilities-and-pixel-contracts.md)

## Context

A pipeline must describe what image is required independently of scheduling and codec-specific optimization. Eagerly allocating output for each operation hides cross-stage opportunities; embedding execution policy inside each operator makes resource control inconsistent.

## Decision

Investigate an **immutable semantic graph** with nodes that describe output shape, pixel/color semantics, and backward dependencies. A separate planner receives demanded outputs, source/codec capabilities, constraints and a resource budget, and produces an **explainable execution plan**. Executor-specific decisions (CPU threads, region shape, copy/fusion points, optional GPU work) are not part of graph semantics.

Keep the graph **minimal**. A graph node is not necessarily a public trait and a graph need not be user-serializable. Preserve the option of single-chain fast paths within the same semantics. No mandatory generic compiler IR or cost-based optimizer in the first version.

The planner may reorder or fuse operations **only if output semantics are preserved**, including orientation, edge behavior, resampling quality, color transfer and alpha.

## Consequences

+ Optimizations can evolve independently from the user-facing API.
+ A plan can explain why a resize decoded an entire image, retained a layout, or materialized an intermediate.
- Introduces metadata and planner complexity; overgeneralization is a real risk.

## Alternatives considered

- **Each node owns execution and buffers:** simple for one operator; difficult for cross-operation optimizations.
- **A universal compute graph/DSL:** powerful but speculative and unrelated to the first use case.
- **Pure scanline-only pipeline:** attractive for simple resizes, too restrictive as the sole semantic abstraction for arbitrary crops, halos and format-specific access.

## Validation

Run the same crop + resize graph with synthetic scanline and region sources. Verify identical semantics, different valid execution plans, and no forced full-frame materialization. The graph representation must remain smaller than the executor code needed to exercise it.

## Review when

Graph-node metadata grows faster than supported behavior, or repeated experiments show the planner cannot meaningfully improve a fixed pipeline.

## Amendment log

- 2026-10-31 — Initial proposal.
