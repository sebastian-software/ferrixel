# ADR-0004: Make buffer ownership and memory budgets explicit

- **Status:** Experimental
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0003](0003-demand-driven-regions.md), [0006](0006-cpu-first-gpu-capable-execution.md)

## Context

Avoiding full-frame intermediate images is primarily about memory movement and allocation, not literally using zero memory. Pipelines still need decoder state, filter halos, queues, output buffers and sometimes unavoidable materialization. Concurrency can multiply these costs. Rust makes ownership explicit but borrowing across asynchronous execution requires care.

## Decision

Investigate **owner-controlled allocations** and **borrowed plane/region views** for CPU-local work, with clearly defined strides, lifetimes and exclusive write access. Separate a logical image from a particular buffer or device allocation. Prefer transfer/reuse of ownership over copies, but do not promise zero-copy across incompatible layouts, device boundaries or external codecs.

The executor should operate under a declared **budget for engine-managed memory** and bound the number of in-flight jobs. Account for work buffers, caches, task queues and requested output. Report materialization points, copies and peak managed memory. Separately report estimated or observed opaque decoder/encoder allocations; a total-process hard cap is **not** guaranteed unless the adapter actually enforces one.

Avoid defining a global allocator, reference-counting strategy, or mandatory ringbuffer layout until a real vertical slice demonstrates the required ownership pattern.

## Consequences

+ Makes resource use visible and allows controlled degradation/fallback.
+ Reduces pressure to introduce `unsafe` for convenient borrowing tricks.
- Ownership, pinned work and cross-thread lifetimes can constrain scheduling designs.
- Opaque codec allocations remain outside strict engine control.

## Alternatives considered

- **Shared, mutable frame objects:** simple integration but encourages implicit copies and locks.
- **Single global memory pool:** may improve reuse but can obscure ownership and contention.
- **Hard promise of O(width) total memory:** invalid for some formats, filters, encoders and decoder internals.

## Validation

Instrument managed buffer allocations, copied bytes and peak live bytes. Run large-height images at fixed width to check expected intermediate-space scaling for streaming-compatible operations; test multi-image backpressure. Test decoder failures, cancellation and out-of-budget behavior without leaks or deadlocks.

## Review when

Allocator instrumentation reveals excessive borrowing/ownership overhead, or a targeted codec's memory requirements make the model misleading.

## Experiment: synchronous ownership and budget (2026-10-31)

The first executor holds at most its required input region and output region for a scalar transform. The crop operation changes the tile's logical coordinate origin **without copying bytes**. An output sink receives a borrowed row slice and cannot retain it through the callback's type. There is no forced destination image allocation inside the engine.

An `Arc`-shared, atomically reserved byte budget tracks the **engine's owned pixel buffers**, using RAII to return reservations on success, cancellation, or allocation failure. The counter enforces a limit across concurrent holders and records a reservation high-water mark. Graph metadata, planner vectors, callback-owned data, allocator overhead and external codecs are **not** included; this is not a total process/RSS bound. Until these other allocations are explicitly modeled, claims must say *managed pixel-buffer budget*, not *strict engine memory limit*.

**New tests:** allocation failure returns an error without leaked reservations, early sink cancellation releases buffers, and a shared budget prevents two simultaneous reservations from exceeding the limit.

## Amendment log

- 2026-10-31 — Initial proposal.
- 2026-10-31 — Added a bounded scalar vertical-slice experiment; unresolved contracts remain explicitly provisional.
