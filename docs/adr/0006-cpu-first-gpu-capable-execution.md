# ADR-0006: CPU-first scheduling with a conditional GPU path

- **Status:** Proposed
- **Date:** 2026-10-31
- **Last reviewed:** 2026-10-31
- **Supersedes:** None
- **Related:** [0002](0002-semantic-graph-and-planner.md), [0004](0004-buffer-ownership-and-budget.md), [0007](0007-correctness-and-performance-evidence.md)

## Context

Modern CPUs offer efficient cache hierarchies and SIMD; GPUs provide throughput on large and repeated computations but incur setup, transfer, synchronization and residency costs. Unconditionally allocating GPU textures or assigning multiple threads to every image can reduce performance. A common semantic graph should not require identical CPU and GPU execution mechanisms.

## Decision

Begin with a **bounded CPU executor** that uses demand-driven regions, work units suited to locality, and measured thread concurrency. Prefer a scalar correctness baseline; add architecture-specific SIMD implementations only with repeatable wins and equivalent output. Do not embed threadpool decisions into individual graph operators.

Make the execution plan represent **data residency, transfers and synchronization** so a future GPU executor can be added without changing graph semantics. Treat a GPU backend as a later measured experiment, not a version-one dependency. Avoid designing a universal CPU/GPU shader language or committing now to a particular GPU abstraction library.

When backend transitions become relevant, evaluate whole-pipeline cost and seek long contiguous segments on the selected backend rather than bouncing buffers between CPU and GPU.

## Consequences

+ CPU performance and safety remain the baseline on all platforms.
+ Allows GPU acceleration without making every operation GPU-aware.
- Some operations will require separate CPU and GPU kernels.
- GPU scheduling may expose constraints that demand a future revision of the planner.

## Alternatives considered

- **GPU-first:** large compatibility and deployment surface before proving benefit.
- **Hardware-independent vector IR from day one:** speculative complexity for limited current evidence.
- **Always maximize per-image threads:** risks oversubscription and memory spikes under throughput workloads.

## Validation

Separate one-image latency from concurrent throughput. Compare scalar to SIMD with output equivalence. For GPU prototypes, include transfer, submission, synchronization and readback in end-to-end time, not just kernel time. Record device, driver, workload sizes and measured break-even point.

## Review when

A real GPU-resident use case dominates the target market, or CPU-only region execution cannot represent an unavoidable backend constraint.

## Amendment log

- 2026-10-31 — Initial proposal.
