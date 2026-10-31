# Fuzzing and hostile-input policy

**Status:** Fuzz harness prepared; sustained fuzz campaign **not yet run**.

Ferrixel's public processing facade accepts untrusted JPEG, JPEG XL, and AVIF bytes. The core forbids unsafe Rust; optional decoder, assembler, SIMD and encoder dependencies do **not** inherit that guarantee. In particular, the current `avif-decode` adapter may decode a complete image and allocate opaque buffers **before** Ferrixel has a chance to reject over-budget pixels.

## Reproducible fuzz target

Install `cargo-fuzz` and run the target from the repository's `fuzz` directory or through `cargo +nightly fuzz` as appropriate for the current platform:

```sh
cargo install cargo-fuzz --locked
cd fuzz
cargo +nightly fuzz run transcode -- -max_len=524288 -timeout=10 -rss_limit_mb=1024
```

The target exercises all three input codec selectors using short, deliberately bounded output requests and an explicit no-guess color policy. It does not assert that a 2 MiB *engine-managed* memory limit constrains external codecs. Run fuzzing in a sandbox with process-level limits and sanitizer support; preserve and minimize crashes in `fuzz/artifacts/` or the upstream codec test corpus.

The default CI also runs **deterministic malformed-header smoke tests** for truncated JPEG/JXL/AVIF files and invalid API options, so regressions surface without an expensive nightly fuzz dependency.

## M1 exit evidence still needed

- [ ] Record fuzz duration, seed corpus, architecture/toolchain, sanitizer configuration, coverage and any unique crashes.
- [ ] Audit decompression bombs and allocation ceilings at the **decoder preflight** stage before an untrusted AVIF frame is fully decoded.
- [ ] Verify JPEG EXIF orientation, JPEG XL orientation/region coordinate mapping and AVIF orientation are not silently lost.
- [ ] Verify ICC parsing and output-profile contradictions with hostile profiles, not only short headers.
- [ ] Recheck memory limits with actual process RSS, not just `MemoryBudget` snapshots.

The existence of a fuzz target alone does not satisfy the M1 hardening acceptance criteria.

## CI bounded smoke

The separate [M1 Decoder Fuzz Smoke](../.github/workflows/m1-fuzz-smoke.yml) installs a pinned cargo-fuzz driver on a nightly Rust runner, seeds it with two checked-in real JPEG/JXL images, and runs **3,000 bounded attempts** with a 16 KiB maximum input and a 1 GiB process RSS cap. This is useful to catch shallow codec/protocol panics and integration bugs. It does **not** constitute a sustained, coverage-reviewed fuzz campaign and is not proof of process-wide memory safety; actual release acceptance still requires broader corpora, sanitizers and target-specific time budgets.

## Early input limits

The experimental public facade now rejects compressed input exceeding a configurable byte cap (default **64 MiB**) before opening any decoder and rejects image dimensions above a configured **100-megapixel** default. The AVIF adapter additionally parses the AV1 primary-item sequence header and checks its maximum coded frame dimensions **before full-frame decode**. This mitigates common decompression-bomb cases; it does **not** bound memory used by the compressed AVIF parser, rav1d, JPEG XL or encoders. The preflight intentionally requires a second AVIF container parse and should eventually be folded into a retained decoder-header object so no work is repeated. Always retain process RSS and sandbox limits for hostile inputs.
