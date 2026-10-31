# Ferrixel

**A pipeline-first image processing engine for modern formats and hardware, written in Rust.**

> **Status: architecture exploration.** Ferrixel is not yet an image processor, has no stable API, and makes no performance claims. This repository establishes its technical direction and records open decisions before implementation.

Ferrixel explores what an image engine should look like when designed today for a five-year horizon rather than for compatibility with historical image-processing APIs.

## Thesis

The pipeline is the product. Codecs provide capabilities; the planner minimizes work; execution backends perform the remaining operations. **Performance should emerge from architecture before micro-optimization.**

Ferrixel's primary goals are:

- **Demand-driven execution:** ask upstream stages for only the data actually required downstream.
- **No default full-frame intermediates:** represent regions and borrowed views wherever semantics and dependencies permit; materialize explicitly only when required.
- **Codec-aware, codec-independent core:** exploit capabilities of JPEG XL, AVIF, WebP, and future formats without binding the graph to a specific codec.
- **Modern pixel representations:** retain appropriate planar, interleaved, high-bit-depth, alpha, and color metadata rather than forcing every operation through RGBA8.
- **CPU-first, GPU-capable design:** excellent CPU behavior with optional measured SIMD paths; permit GPU execution when transfers and workload make it worthwhile.
- **Memory safety by construction:** safe Rust in the core, bounded engine-managed allocations, explicit buffer ownership, and honest accounting for limits imposed by codecs.
- **Correctness and observability:** measurable output fidelity, explainable execution plans, and reproducible benchmarks; no unmeasured performance claims.

### Deliberate non-goals

Ferrixel is **not** initially a libvips/Sharp API clone, a universal image editor, a replacement for every codec, a JPEG benchmark project, or a cross-platform GPU abstraction framework. JPEG can be supported later as an input compatibility adapter without steering the architecture.

## Conceptual pipeline

```text
Image source / encoded input
           |
      Semantic graph         (what the caller wants)
           |
     Capability planner      (what can actually be avoided)
           |
    Execution plan           (regions, layouts, buffers, tasks)
           |
    CPU backend  /  future GPU backend
           |
   Encoded sink / raw surface / stream
```

A graph node describes the image it produces, not an eagerly allocated pixel array. A consumer's required region propagates backward through operations, including filter halos, decoder alignment, and unavoidable whole-image dependencies. The planner may select reduced-resolution decode, reuse a compatible native layout, fuse operations, or choose an explicit materialization point. All such choices must preserve declared output semantics.

## Read the design

- **[M1 — First complete image pipeline](docs/milestones/M1-complete-image-pipeline.md)**: 10 ordered work packages, acceptance gates, and a usable SDR JPEG/JXL → AVIF goal

- [Architecture sketch](docs/architecture.md)
- [Architecture Decision Records](docs/adr/README.md) — **living**, reviewable decisions with explicit status and reconsideration triggers
- [Research hypotheses and codec capability checklist](docs/research/format-capabilities.md)
- [First vertical slice](docs/roadmap.md)
- [Color-signalling contract](docs/design/color-management-contract.md) and [ADR-0015](docs/adr/0015-typed-output-color-signals.md)
- [Linear-light sRGB and alpha experiment](docs/design/color-correctness-experiment.md) and [ADR-0014](docs/adr/0014-linear-srgb-straight-alpha.md)
- [Real decoded resize experiment](docs/design/decoded-resize-experiment.md) and [ADR-0013](docs/adr/0013-sample-domain-resize.md)
- [Real decoder experiment](docs/design/real-decoder-experiment.md) and [codec candidate audit](docs/research/codec-adapter-audit.md), [the Gray8 experiment](docs/design/region-experiment.md), and [the borrowed planar experiment](docs/design/planar-surface-experiment.md)
- [Contributor guide](CONTRIBUTING.md) and [agent guide](AGENTS.md)

## Development

The repository contains **two internal, experimental, codec-free** slices: (1) `synthetic Gray8 source → crop → bilinear resize → borrowed scanline sink`, exercising backward region planning and bounded managed pixel buffers; (2) externally borrowed, byte-strided **10-bit YUV 4:2:0 planes → even-origin crop → borrowed scanline sink**, preserving color metadata without RGB conversion or pixel allocation. Chroma-phase-changing crops and planar resizing are deliberately not implemented. Read-only JPEG and JPEG XL are **optional, private experimental decoder adapters**, and a narrow **encoded-sample-domain bilinear resize** now exercises both. Decoder ICC/CICP and alpha-association metadata propagate through the graph. An opt-in **linear-sRGB bilinear reference** now supports explicitly tagged sRGB RGB8 and straight RGBA8 with premultiplied filtering; unknown/ICC/HDR color and unspecified alpha are rejected, not guessed. The earlier encoded-sample resize remains a distinct baseline and is **not** color-managed. ICC/CICP evidence is now inspected through a typed, non-transforming color contract: sRGB, Display P3, BT.2100 PQ/HLG, ICC and dual-signalled output are distinguishable, with unsafe or unsupported cases explicitly rejected. There is **still no general ICC/HDR conversion or cross-codec color management**. Encoder, supported public processing API, SIMD and GPU backends remain unimplemented. JPEG uses an explicit full-frame fallback; JPEG XL requests a codec-level ROI without claiming internal work savings.

```sh
cargo fmt --all --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings

# Optional codecs: read-only JPEG and JPEG XL
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

All project materials, code, comments, ADRs and commits use **US English**. We follow Conventional Commits.

## Status and licensing

Design exploration; unpublished, unstable, and **not ready for production**.

Ferrixel is dual-licensed under **[MIT](LICENSE-MIT) OR [Apache License 2.0](LICENSE-APACHE)**, at your option. See [ADR-0010](docs/adr/0010-dual-licensing.md) for the rationale. Contributions are submitted under the same dual-license terms unless explicitly stated otherwise. Third-party codecs and dependencies retain their own licenses and require individual review before integration.

Licensing does not imply a stable API or a published crate; first publication remains a separate decision.
