# Working on Ferrixel

Ferrixel is in an intentionally **design-first** phase. Do not turn a proposed architectural idea into a public API or dependency merely because it is described in an ADR.

## Non-negotiable conventions

- **US English** for source code, comments, documentation, ADRs, tests, commit messages, and issues.
- **Conventional Commits** for commit messages.
- Read [README.md](README.md), [docs/architecture.md](docs/architecture.md), and [docs/adr/README.md](docs/adr/README.md) before implementation.
- Keep architecture decisions **living**: update the ADR and its evidence and review date when design changes. Record replacements and supersessions explicitly; do not silently rewrite historical rationale.
- Prefer narrow abstractions supported by a realistic end-to-end use case to speculative extensibility.
- No `unsafe` in the engine crate. If a future codec or platform adapter needs `unsafe` or FFI, isolate and audit that boundary; do not imply that the entire dependency graph is memory-safe.
- Read [M1 milestone plan](docs/milestones/M1-complete-image-pipeline.md) before expanding pipeline scope; preserve its acceptance criteria and explicit codec-license gates.
- No unmeasured performance claims. Establish correctness and a scalar/reference path before hand-written SIMD or GPU specialization.
- Never optimize only for historical JPEG compatibility at the expense of modern formats; do not claim partial decoding is supported unless the *specific decoder implementation* exposes it.
- Keep output semantics explicit: crop coordinates, orientation, bit depth, color space, transfer function, chroma siting, alpha handling, precision and filtering all matter.
- Codec color metadata must describe the **emitted samples**. ICC header inspection alone is not a color transform; never auto-prioritize ICC versus CICP or reinterpret P3/PQ/HLG as sRGB. See ADR-0015. In particular, [ADR-0011](docs/adr/0011-borrowed-planar-surface-contract.md) permits only even-origin YUV420P10 crops and no planar resize until a chroma-aware quality policy exists.
- The plan must make materializations and CPU/GPU transfers observable.

## Local checks

```sh
cargo fmt --all --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

## Before expanding scope

Add or amend an ADR with its problem, alternatives, evidence, trade-offs and re-evaluation trigger. For any optimization, include a reproducible benchmark, peak-memory observation, and image-quality/correctness checks. Update the ADR index and architecture sketch accordingly.
