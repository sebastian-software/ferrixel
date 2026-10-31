# Modern image format capability checklist

*Research notes, not guarantees. The concrete decoder and its public API determine which optimizations are usable.*

This document drives adapter experiments and should be revised with exact decoder versions, source links, and benchmarks as they become available.

| Question | JPEG XL | AVIF | WebP |
| --- | --- | --- | --- |
| Native decoded representation | VarDCT / Modular internally; exposed pixel format depends on decoder | Often planar YCbCr for AV1-based images | Lossy is VP8-derived YUV; lossless has a different internal representation |
| Useful structure to investigate | Groups, progressive reconstruction, reduced-resolution representations | AV1 blocks/tiles, grid images, optional layered representations | Incremental decoding; library-specific crop/scale interfaces |
| Region extraction without full reconstruction | **Investigate** group granularity and decoder interface | **Investigate** grid-cell and tile behavior and API exposure | **Investigate** decoded-area semantics, not just output cropping |
| Low-resolution decode | Investigate progressive / downsampled modes and quality implications | Decoder- and bitstream-dependent; not guaranteed | libwebp has integrated scaling; verify work actually saved |
| Color/bit-depth path | High precision, color profiles and transfer characteristics | YCbCr, chroma sampling, high bit depth, HDR variants | YUV and RGBA paths vary by lossless/lossy mode |
| Output buffering constraints | Verify encoder streaming/granularity | Verify encoder requirements and opaque working memory | Verify encoder requirements and output mode |

## Required experiment for each adapter

Document the format variant, decoder crate/version, enabled features, pixel layout, color semantics, supported output granularity, whole-frame fallback behavior, memory accounting, and licensing/safety audit. Explicitly distinguish:

1. **Format capability**: what the bitstream could permit.
2. **Decoder capability**: what the actual library can expose.
3. **Pipeline exploitation**: what the planner can use without changing output semantics.

For progressive decoding, distinguish fast previews from final quality. For cropping, distinguish *not emitting* pixels from *not decoding* pixels. For YUV-native processing, verify chroma siting, alpha handling, color ranges and transfer functions before calling an operation equivalent to an RGB path.

## Architectural anti-patterns

- A forced `RGBA8` intermediate between every stage.
- A hard-coded tile size based on one codec.
- Assuming that random access or partial decode is available merely because the file format supports it.
- Comparing codecs at unequal quality settings and calling it a pipeline speedup.
- Treating an encoder's opaque memory use as if the engine's allocator could enforce it.
