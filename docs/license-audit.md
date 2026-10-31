# Dependency licensing and release gates

Ferrixel itself is **MIT OR Apache-2.0** (see [ADR-0010](adr/0010-dual-licensing.md)). That license does **not** relicense embedded or transitive codec, compression, image, parser, assembler or CMS implementations.

## Automated metadata screening

```sh
python3 scripts/audit-licenses.py
```

The script invokes `cargo metadata --all-features`, traverses the resolved dependency graph and writes `target/m1-license-inventory.csv`. Ordinary CI **blocks strong-copyleft metadata** and reports missing/unknown/weak-copyleft terms as `REVIEW` without pretending they are approved. A strict pre-release run (`python3 scripts/audit-licenses.py --strict`) fails if *any* review item remains. This is a **metadata preflight, not a lawyer-reviewed dependency license audit**. `SPDX OR` grants a choice; metadata does not demonstrate how that choice must be documented or distributed.

When a crate is flagged, review its exact source license, license-file and notices, selected feature/dependency tree, patent terms, and bundled native code before allowing it. Do not expand the allowlist just to make CI green.

## Current direct codec/CMS choices

| Dependency | Cargo feature | Upstream metadata/license | Review focus |
| --- | --- | --- | --- |
| `zune-jpeg` | `jpeg-read` | Permissive; verify exact crate release | JPEG headers, ICC handling, decode scratch |
| `jxl-oxide` | `jpeg-xl-read` | Permissive; verify exact crate release | CMS output profile, color/alpha, crop/internal cache |
| `avif-decode` / `rav1d` | `avif-read` | BSD-3-Clause decoder; inspect native/build artifacts | Target-specific NASM assembly, malformed bitstreams, MSRV 1.98 |
| `ravif` / `rav1e` | `avif-write` | BSD-3-Clause encoder; audit transitive licenses | Encoded CICP, alpha, frame materialization and opaque scratch |
| `moxcms` | `icc` | BSD-3-Clause OR Apache-2.0 | ICC parsing, color fidelity, malformed profiles, SIMD/native code |

A future JPEG XL encoder, including the AGPL-3.0/commercial `jxl-encoder` candidate, is **not** an approved default dependency and requires a separate ADR.

## M1 release prerequisites

- [ ] Store an externally reviewable license inventory for an exact, reproducible version set (including NOTICE/license files).
- [ ] Review native code, assembler and unsafe boundaries in *all* dependencies, not only Ferrixel's `#![forbid(unsafe_code)]` core.
- [ ] Confirm whether a `Cargo.lock` should be committed for the CLI before first distribution and lock versions for reproducible measurements.
- [ ] Check that the package advertises its required Rust version accurately and that the target platforms actually compile.
- [ ] Record real AVIF output ICC/CICP and alpha behavior, HDR rejection, and total RSS on representative images.
- [ ] Perform an independent source and legal review of third-party license obligations.

Do **not** declare M1 release-ready merely because the metadata gate passes.

## Transitive review: `avif-parse` (MPL-2.0)

The `avif-decode` dependency currently incorporates `avif-parse` version 2.1.0, which declares **MPL-2.0** (confirmed in its upstream `Cargo.toml` and source headers). MPL-2.0 is **file-level copyleft**, not equivalent to a permissive MIT/Apache grant. A separately linked, unmodified MPL crate may be usable alongside an MIT/Apache project, but distributions must still honor MPL notices and source-availability obligations for covered files and assess any modifications. This review item must not be silently added to the permissive allowlist. Until the source/NOTICE redistribution requirements are explicitly reviewed, the full-dependency license gate remains **open**, even if code CI is green.

Reference: https://github.com/kornelski/avif-parse/blob/main/Cargo.toml
