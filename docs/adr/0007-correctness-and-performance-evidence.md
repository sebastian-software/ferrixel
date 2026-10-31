# ADR-0007: Evidence before optimization

- **Status:** Accepted
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0001](0001-project-thesis.md), [0006](0006-cpu-first-gpu-capable-execution.md)

## Context

Performance work can optimize a narrow benchmark while silently regressing correctness, memory, concurrency or other formats. Modern image codecs also expose output quality and speed trade-offs that can make raw elapsed-time comparisons misleading. Ferromark and Ferroni reinforce the value of architecture-led optimization and measurement before SIMD specialization.

## Decision

Adopt **correctness-first, reproducible evidence** for all substantial optimizations. Maintain a simple/reference implementation where practical. For performance comparisons, distinguish codec time from transform time; always include representative end-to-end benchmarks as well. State the exact input, output semantics, codec versions, quality settings, CPU/GPU, threads, warmup and repetitions.

Track at least:

- latency distribution, throughput and scaling under concurrency;
- peak live engine-managed memory and total process RSS where available;
- allocated bytes, copy volume and number/size of materialized intermediates;
- amount of pixel work actually done (including decoded/filtered pixels);
- output correctness and perceptual quality, with explicit tolerances for lossy comparisons.

Only add hand-written SIMD or special GPU kernels when measurements demonstrate a worthwhile win for realistic workloads. No unqualified claims such as 'fastest' or 'zero-copy' without evidence and scope.

## Consequences

+ Prevents optimization from becoming a disconnected collection of microbenchmarks.
+ Makes regressions diagnosable and supports long-lived architectural decisions.
- Measurements require maintenance, representative fixtures and independent oracles.

## Alternatives considered

- **Throughput only:** hides latency, allocations and output differences.
- **Microbenchmarks only:** ignores decoder/encoder and data-movement costs.
- **Optimize first, validate later:** unsuitable for image fidelity and memory-safety goals.

## Validation

Create a benchmark protocol and correctness corpus before the first optimized kernel. CI should verify reference equivalence and fail on semantic regressions. Performance assertions remain documented measurements rather than brittle cross-machine CI thresholds until a stable regression setup exists.

## Review when

Benchmarks become too expensive to sustain or a new quality metric demonstrates that the current oracle misses user-visible defects.

## Amendment log

- 2026-10-31 — Accepted initial measurement and correctness policy.
