# Architecture Decision Records

Ferrixel keeps **living ADRs**: decisions are durable and reviewable, not immutable historical monuments. A decision can change when evidence changes, but its previous rationale and the reason for revision must remain discoverable.

## Lifecycle

- **Proposed:** hypothesis/design under review, not binding implementation policy.
- **Experimental:** temporarily implemented or benchmarked to collect evidence.
- **Accepted:** current architectural policy; amendments require recorded rationale.
- **Superseded:** replaced by a linked ADR.
- **Rejected:** considered and declined, with an explanation.

Every ADR contains **Status**, **Date**, **Last reviewed**, **Context**, **Decision**, **Consequences**, **Alternatives**, **Validation**, **Review when**, and an **Amendment log**. Update this index whenever status or topic changes. Separate measured facts from hypotheses; avoid declaring architecture decisions from attractive diagrams alone. Use [TEMPLATE.md](TEMPLATE.md) for new ADRs.

## Current decisions

| ADR | Topic | Status |
| --- | --- | --- |
| [0001](0001-project-thesis.md) | Pipeline-first project principles and modern-format scope | Accepted |
| [0002](0002-semantic-graph-and-planner.md) | Semantic graph separated from capability-aware planner | Experimental |
| [0003](0003-demand-driven-regions.md) | Demand propagation, region footprints and fallback | Experimental |
| [0004](0004-buffer-ownership-and-budget.md) | Ownership, allocation and realistic memory budgets | Experimental |
| [0005](0005-codec-capabilities-and-pixel-contracts.md) | Modern codec adapters, color and layout contracts | Proposed |
| [0006](0006-cpu-first-gpu-capable-execution.md) | Execution scheduling, SIMD and conditional GPU | Proposed |
| [0007](0007-correctness-and-performance-evidence.md) | Correctness and benchmark evidence before optimization | Accepted |
| [0008](0008-initial-vertical-slice.md) | Minimal architecture-validating implementation | Experimental |
| [0009](0009-living-adr-process.md) | Decision lifecycle and amendment policy | Accepted |
| [0010](0010-dual-licensing.md) | Dual-license Ferrixel under MIT OR Apache-2.0 | Accepted |
| [0013](0013-sample-domain-resize.md) | Preserve rendered metadata and validate sample-domain codec resize | Experimental |
| [0012](0012-codec-source-contract.md) | Real decoder sources: explicit ROI capabilities and JPEG fallback | Experimental |
| [0011](0011-borrowed-planar-surface-contract.md) | Borrowed, byte-strided YUV420P10 source contract | Experimental |

## Changing an ADR

Edit the original if refining or correcting the same decision; update **Last reviewed** and append the key change to the amendment log. Write a new ADR when fundamentally replacing a decision or choosing a materially different direction, and cross-link `Supersedes`/`Superseded by`. Significant changes should include a small experiment or empirical motivation.
