# Linear-light sRGB and straight-alpha experiment

**Status: experimental; not a general color-managed pipeline.** See [ADR-0014](../adr/0014-linear-srgb-straight-alpha.md).

## Why two resize domains?

Ferrixel's previous `EncodedSamples` bilinear reference filters the numbers delivered by the decoder. This is useful as a controlled baseline but produces a dark midpoint between black and white in gamma-encoded sRGB. A 50/50 average between sRGB 0 and 255 gives 128 in encoded space; decoded to linear light, averaged and encoded again, it gives **188**.

`LinearSrgb` is a distinct semantic graph operation. It uses the same region- and row-based executor but requires exact input color and alpha semantics before a source can be bound. Its refusal of unsupported color is intentional and testable.

## Pixel operations

For each of the four bilinear neighbors:

1. Decode RGB components with the standard piecewise sRGB transfer function.
2. If an alpha channel exists, normalize the straight alpha to [0, 1] and multiply the *linear* RGB components by it.
3. Bilinearly mix premultiplied colors and alpha using the same spatial weights.
4. When output alpha is positive, divide the mixed RGB components by output alpha; otherwise set them to zero.
5. Apply the sRGB transfer function to RGB, quantize RGB and alpha independently, and emit straight U8 samples.

This avoids transparent-pixel color halos; alpha is a linear opacity, not an sRGB color. The reference uses F64 intermediates and explicit quantization. No new full-frame image is allocated. The source decoder may nevertheless allocate one (e.g. the JPEG fallback), and decoder-internal allocations remain opaque.

## Scope and fail-closed behavior

Only U8 interleaved RGB (opaque) and RGBA (explicitly straight alpha) with sole full-range RGB/sRGB CICP `[1,13,0,1]` are accepted. The experimental path rejects unknown, ICC-only, ICC plus CICP, PQ, HLG, CMYK, YUV420, F32 and unestablished alpha association. Decoder-provided metadata refers to *emitted* samples; original-file metadata alone is insufficient evidence.

Current JPEG may have no applicable CICP and JPEG XL currently emits F32, so this experiment is primarily validated by synthetic decoder sources. Do not claim that these codecs are color managed through `LinearSrgb` until a proper conversion adapter or explicit policy is implemented and tested.

## Quality and engineering limitations

- Bilinear is a reference filter; it is not antialiasing-optimized for strong downscales and is not a Lanczos/area resampler.
- Only sRGB transfer and alpha association are implemented. No ICC parsing, perceptual intent, chromatic adaptation, wide-gamut transform, PQ/HLG transfer, tone mapping, pixel-orientation correction or HDR reference-white model exists yet.
- `EncodedSamples` remains separately callable, with its existing semantics; no silent automatic plan optimization may exchange it for `LinearSrgb`.
- Benchmarks should compare equal-quality modes and count decoder/encoder work separately.
- Future managed color conversion should be a composable graph operation that can be fused by the planner without requiring a full-frame materialization.

## Tests

The `linear_srgb_tests` module covers encoded vs linear black/white mixing, invisible-color isolation, all-transparent pixels, 2D mixing, profile conflicts and unsupported transfers, alpha association requirements, memory budget failure, and cancellation. The existing optional JPEG and JPEG XL tests continue to exercise the sample-domain path.

## Pixel format identity

A failed contract test revealed that `InterleavedU8(4)` did not distinguish RGBA from CMYK. The pixel format now includes the semantic `ChannelLayout` rather than a bare channel count. A four-channel CMYK surface is rejected even if its caller incorrectly claims straight alpha and an sRGB CICP tag. This is why the reference tests check semantic compatibility, not only data length.
