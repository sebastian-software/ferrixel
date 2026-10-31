# Contributing to Ferrixel

Thanks for helping shape Ferrixel. The project is currently exploring architectural contracts, not accumulating features.

## Working agreements

- Write **US English** throughout the project.
- Use **Conventional Commits**, such as `docs(adr): clarify region dependencies` or `feat(core): introduce surface metadata`.
- Prefer small, independently reviewable changes with explicit correctness criteria.
- Keep core logic in safe Rust. Treat decoder and GPU adapter safety boundaries independently and document them.
- Never add a dependency solely because it is popular or appears in a benchmark. Explain why the interface is needed and what can be substituted later.

## Living ADRs

Before material changes to graph semantics, memory ownership, decoder contracts, scheduling, pixel layouts, or backend selection:

1. Open the related ADR or create a new one from [the template](docs/adr/TEMPLATE.md).
2. Describe the actual problem and evidence; separate observations from assumptions.
3. Mark the decision `Proposed`, `Accepted`, `Experimental`, `Superseded`, or `Rejected` as appropriate.
4. Update `Last reviewed` and any `Review when` triggers; document alternatives and consequences.
5. Preserve a short amendment log for substantive changes and cross-link superseding ADRs.

These documents are **living** but not context-free: do not replace a decision's reasoning without explaining why it changed. A proposed implementation is not an accepted commitment.

## Validation

```sh
cargo fmt --all --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Benchmarks, once introduced, should report the operation, input dimensions and format, hardware, decoder/encoder versions, quality settings, warmup, repetitions, latency distribution, throughput, total allocated bytes, and peak resident memory where possible. Compare equivalent image quality; log planner decisions and intermediate materializations. Performance comparisons must not exclude decoding or encoding unless explicitly labeled as *transform-only*.

## Project boundaries

Do not introduce release automation, external APIs, bindings, C FFI, a dedicated GPU framework, or public compatibility guarantees during the design scaffold phase without an accepted ADR. First publication still requires an explicit decision; project dual-licensing is accepted in [ADR-0010](docs/adr/0010-dual-licensing.md).
## License of contributions

Unless explicitly stated otherwise, contributions intentionally submitted to Ferrixel are licensed under **MIT OR Apache-2.0**, at the recipient's option. Only submit code and assets you have the right to license on those terms. Third-party libraries and copied code must undergo a separate license, attribution, and patent-obligation review before inclusion; the project's license does not relicense its dependencies.
