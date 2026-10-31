# Codec candidate audit (31 October 2026)

This document records adapter feasibility, not production-hardening, a benchmark or a blanket safety guarantee.

| Input | Candidate | Licensing | Verified interface | Decision |
|---|---|---|---|---|
| JPEG | [zune-jpeg 0.5.15](https://docs.rs/zune-jpeg/0.5.15/zune_jpeg/) | MIT OR Apache-2.0 OR Zlib | Header probe, RGB output, decode into caller-provided full-frame buffer | Optional input adapter; explicit materialization, no JPEG output optimization |
| JPEG XL | [jxl-oxide 0.12.6](https://docs.rs/jxl-oxide/0.12.6/jxl_oxide/) | MIT OR Apache-2.0 | Cropped render request, f32 output stream, ICC/CICP metadata | Optional ROI input adapter; no claim that requested ROI guarantees reduced codec work |
| JPEG XL alternative | [jxl-rs](https://github.com/libjxl/jxl-rs) | BSD-3-Clause | Modern Rust decoder | Compare ROI interface, buffer ownership, performance and maintenance before choosing a permanent backend |
| AVIF | [zenavif](https://github.com/imazen/zenavif) | AGPL or commercial | Rust decode, HDR/YUV metadata | Excluded for now; not automatically compatible with intended permissive distribution |
| AV1 core | [rav1d-safe](https://github.com/imazen/rav1d-safe) | Requires dependency-level review | AV1 frame decoding, candidate planar output | Investigate AVIF container assembly and multi-item support separately |

## Validation backlog

- Stress unusual dimensions, malformed inputs and decompression bombs.
- Review dependencies including transitive licenses and any native/unsafe code.
- Compare color correctness for ICC, CICP, HDR, alpha and chroma.
- Make orientation, animation and progressive output semantics explicit.
- Instrument realistic decoder RSS, scratch allocations and actual ROI work.
- Consider output streaming and encoder constraints only after source contracts are sound.
