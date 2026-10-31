# Working on Ferrixel

Ferrixel is in an intentionally **design-first** phase. Do not turn a proposed architectural idea into a public API or dependency merely because it is described in an ADR.

## Non-negotiable conventions

- **US English** for source code, comments, documentation, ADRs, tests, commit messages, and issues.
- **Conventional Commits** for commit messages.
- Read [README.md](README.md), [docs/architecture.md](docs/architecture.md), and [docs/adr/README.md](docs/adr/README.md) before implementation.
- Keep architecture decisions **living**: update the ADR and its evidence and review date when design changes. Record replacements and supersessions explicitly; do not silently rewrite historical rationale.
- Prefer narrow abstractions supported by a realistic end-to-end use case to speculative extensibility.
- No `unsafe` in the engine crate. If a future codec or platform adapter needs `unsafe` or FFI, isolate and audit that boundary; do not imply that the entire dependency graph is memory-safe.
- No unmeasured performance claims. Establish correctness and a scalar/reference path before hand-written SIMD or GPU specialization.
- Respect [ADR-0010](docs/adr/0010-dual-licensing.md): Ferrixel is `MIT OR Apache-2.0`. Do not add incompatible third-party code or dependencies without a documented licensing review.
- Never optimize only for historical JPEG compatibility at the expense of modern formats; do not claim partial decoding is supported unless the *specific decoder implementation* exposes it.
- Keep output semantics explicit: crop coordinates, orientation, bit depth, color space, transfer function, chroma siting, alpha handling, precision and filtering all matter.
- The plan must make materializations and CPU/GPU transfers observable.

## Local checks

```sh
cargo fmt --all --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

## Before expanding scope

Add or amend an ADR with its problem, alternatives, evidence, trade-offs and re-evaluation trigger. For any optimization, include a reproducible benchmark, peak-memory observation, and image-quality/correctness checks. Update the ADR index and architecture sketch accordingly.