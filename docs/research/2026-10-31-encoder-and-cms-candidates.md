# Encoder and color-management candidates — 2026-10-31

**Status:** Research notes; no library dependencies chosen or installed. Confirm the exact version, its complete dependency graph, source permissions, APIs and reproducible behavior before integrating anything.

## First modern output: AVIF

### `ravif` (`rav1e`-based)

- Documentation: https://docs.rs/ravif/0.13.0/ravif/
- Manifest: https://docs.rs/crate/ravif/0.13.0/source/Cargo.toml.orig
- **License:** BSD-3-Clause; permissive, subject to NOTICE/attribution and full transitive license audit.
- Supports RGB/RGBA input; exposes raw-plane iterator APIs for 8-bit and 10-bit planes, but an iterator API **does not prove** end-to-end streaming or a bounded encoder working set.
- The encoder returns an encoded AVIF buffer. Benchmark and report any input materialization, output buffering and rav1e's opaque internal memory separately.
- **Candidate for M1's first output adapter**, after license and output-profile validation. No assumption that 10-bit internal AV1 encoding makes 8-bit input HDR-capable.

### AVIF decoding

A production-quality decoder choice is still an open research and adapter decision. Evaluate format coverage (grid/alpha, 8/10/12-bit, animation scope), native plane layout, actual ROI support, licensed dependencies, safety, malformed-input behavior, color metadata, and the memory contract. Do not equate a format capability with a particular decoder API's capabilities.

## JPEG XL output: explicit license and memory gate

### `jxl-encoder` (pure Rust)

- Documentation: https://docs.rs/crate/jxl-encoder/0.3.4
- Repository: https://github.com/imazen/jxl-encoder
- **License: AGPL-3.0 or commercial**, rather than MIT/Apache/BSD as a blanket dependency. Its libjxl-derived upstream algorithms are separately BSD-3-Clause; that does **not** relicense its implementation.
- Features include lossless and lossy JPEG XL and a `Write`-oriented output API. A `Write` output interface alone is **not evidence of bounded source/scratch memory**; validate the current frame-input behavior, especially on very large images.
- **Not approved for the default MIT OR Apache-2.0 Ferrixel dependency tree.** A compatible implementation, explicit commercial license, or a separate optional backend would require its own decision.

### `libjxl` (reference C++)

- https://github.com/libjxl/libjxl
- **BSD-3-Clause**, reference-quality and actively developed.
- Potential optional FFI adapter, but introduces a native toolchain and an unsafe boundary: conflicts with a pure-safe-Rust preference, not inherently with the license. Defer until a concrete need warrants it.
- A separate pure-Rust permissive encoder can be investigated; do not assert such an option without verifying quality, coverage, license and memory behavior.

### Existing JPEG XL decode

- `jxl-oxide`: https://github.com/tirr-c/jxl-oxide — already integrated as an optional decoder with a codec ROI-request capability. Internal avoided work remains unproven by output cropping alone.
- `jxl-rs`: https://github.com/libjxl/jxl-rs — separately evaluate before changing decoder backends; improving SIMD/progressiveness does not settle the executor's color or memory contracts.

## ICC color management

### `moxcms`

- Manifest: https://docs.rs/crate/moxcms/0.9.1/source/Cargo.toml.orig
- Repository: https://github.com/awxkee/moxcms
- **License: BSD-3-Clause OR Apache-2.0**; attractive for Ferrixel's default permissive policy subject to transitive audit.
- Pure-Rust profile parsing and transforms; scanline-based RGB/CMYK transform examples exist. Verify out-of-gamut, linear-light precision, alpha and profile/intent handling using known ICC profiles and independent test oracles.
- A usable ICC-transform API does **not** answer how to choose a working color space, how to treat PQ/HLG, or how to resolve encoder-versus-decoder precedence. Those remain separate graph/planner contracts.

## Minimum adapter review checklist

Before adding any codec or CMS as a Cargo dependency:

1. Exact crate and transitive licenses, attribution and MSRV.
2. Native format and sample layout, color and alpha semantics of **emitted/consumed** pixels.
3. Row/region API, true partial decode, full-frame fallback, and immutable vs mutable buffers.
4. Engine-managed allocations, codec-internal memory, peak RSS, cancellation and failure behavior.
5. Independent format conformance/quality tests; mismatched default quality settings are not comparative benchmarks.
6. Exposure of format metadata to the planner; zero silent color-profile guessing.

The preferred output decision for M1 is **AVIF first**, with **JPEG XL output as an explicit follow-up gate**.
