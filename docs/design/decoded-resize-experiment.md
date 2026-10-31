# Real decoder resize: first bounded sample-domain experiment

**Status:** experimental internal code, not a stable API or a general-purpose color-managed resize. See [ADR-0013](../adr/0013-sample-domain-resize.md).

## Execution path

1. Decoder reports sample type, channels, requested region behavior and rendered ICC/CICP.
2. Graph carries immutable image semantics independently of decoder-owned pixel storage. Crop nodes keep the semantics unchanged.
3. The caller explicitly chooses **EncodedSamples** bilinear interpolation. Only opaque Gray/RGB U8 and F32 are supported.
4. The existing backward planner calculates the source footprint, including crops before and after the resize.
5. One source request provides borrowed rows. A budgeted previous input row and one output row allow sequential scalar interpolation and sink callbacks.
6. The sink receives borrowed, non-retained rows. Memory reservations are released on normal completion, errors or cancellation.

JPEG still decodes a full RGB frame **inside the adapter** and reports it; the resize makes **one** decode request rather than one per output scanline. JPEG XL makes an ROI request, but that alone does not prove that the codec skipped internal work. The current planner uses a bounding source region, which may include many unused intervening scanlines on strong downscales.

## Color fidelity boundary

The operation interpolates numbers in their **current encoded representation**, not linear-light colors. The original ICC and/or CICP metadata is propagated unchanged; nothing is converted. Missing metadata remains explicitly unspecified. There is no gamma correction, gamut mapping, chroma resampling, tone mapping, alpha premultiplication, CMYK conversion, or YUV plane resizing. F32 values are not clamped to [0,1]. Do not advertise this path as quality-optimal for HDR or color-critical workloads.

## Memory and testing

The engine budgets two rows (previous input + current output), not a full-frame transform result. Adapter-owned full decoded JPEG frames and opaque codec allocations are reported separately and are **not** covered by a universal process memory cap.

Tests use an independent bilinear oracle, explicit metadata mismatch, alpha rejection, cancellation, budget exhaustion, float HDR sample ranges, and feature-gated JPEG/JPEG XL fixtures. The next planner experiment should compare decoder-native scaling, sparse scanline requests and one-pass bounding requests under measured workloads before adding more codecs.
