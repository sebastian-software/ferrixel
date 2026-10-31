# Borrowed planar 10-bit surface experiment

**Status:** Experimental; not a supported decoder, color-processing engine, resize implementation, or public API. See [ADR-0011](../adr/0011-borrowed-planar-surface-contract.md).

## Question

Can the existing semantic graph and backward region planner handle format-native, non-contiguous, externally owned pixel planes **without** demanding an RGBA8 materialization or embedding buffer ownership in graph nodes?

## Concrete test contract

- `PixelFormat::Gray8` and `PixelFormat::Yuv420P10(ColorMetadata)` are separate, explicit internal semantic types. The first scalar resize remains Gray8-only.
- A `Planar10Source` graph node contains dimensions and *color semantics only*. Its bytes are supplied at execution time by a `Yuv420P10<'a>` borrowing three planes. This separates graph lifetime from decoder/buffer lifetime.
- Each plane borrows an unaligned `&[u8]` and declares a **byte stride**. Samples are *unpacked 16-bit little-endian* values with 10 meaningful low bits. No reinterpret cast, alignment assumption, or allocation is needed for a borrowed plane view.
- Luma uses full dimensions. Cb and Cr each use `ceil(width/2) × ceil(height/2)`, including images with odd dimensions. `PlaneView::new` validates active-row width, last accessible byte, and stride multiplication with checked arithmetic; padding after the final active row is not required.
- `ColorMetadata` records color primaries, transfer characteristics, matrix coefficients, full/limited range, and chroma siting. The prototype **preserves** this metadata through crops, but does not perform color transforms or interpret ICC profiles.
- Only crop chains that preserve chroma phase are executable: **even x and y** at every crop and for the requested output region. Odd widths and heights remain valid. An odd crop origin requires an explicit phase-aware resampling operation and currently returns an error, rather than producing silently incorrect color.
- `execute_planar10` receives one source binding, uses the existing backward region planner, and calls a borrowed-row sink in Y, Cb, Cr order. It sends only active samples (no stride padding) and has **zero managed pixel-buffer allocations**, because the caller owns the input planes and sink. Graph/plan Vec allocations and the caller's source/sink are not included in this claim.
- All planar resizing, even independent per-plane resizing, is **intentionally unsupported**. Correctness requires defining a chroma-aware filter and color quality policy. An 8-bit Gray executor rejects the YUV source rather than treating bytes as grayscale.

## Observed invariants tested

1. Non-contiguous and even odd-byte-strided planes are read and cropped without copying or unaligned `u16` pointer casts.
2. Pixel values up to 1023 roundtrip in an explicit little-endian byte representation; stride padding never enters the sink.
3. The exact source region is determined by the existing crop planner and remains within the input bounds.
4. Slice pointer identity proves forwarded sink rows borrow the original source storage, not an intermediate buffer.
5. Byte size, plane coordinates, 4:2:0 subsampling, and ceil dimensions for odd luma sizes are correct.
6. Color metadata survives the graph; mismatched source sizes, color bindings, and node identities are rejected *before* the first sink callback.
7. Unaligned chroma crops, accidental color-blind resizing, insufficient storage, and a Gray8/YUV sink mismatch produce explicit errors.
8. Sink cancellation stops forwarding and leaves the external source buffers unchanged.

## What this does *not* prove

- Arbitrary decoder-native buffers can be represented as-is. Some deliver big-endian, packed 10-bit, higher-bit-depth, tiled, GPU-resident or opaque storage, or permit borrowing only for a bounded callback scope.
- Crops with odd x/y positions are impossible; they simply need a correct chroma-phase model before we allow them.
- YUV-native resize has the same output as a linear-light RGB reference. It generally does not.
- The overall process is allocation-free or memory-bounded. The caller owns the planes, and the Rust planner still allocates metadata.
- Multiple asynchronous consumers can borrow the same frame without pinning/retaining decoder storage. That needs separate experiments.

## Follow-ups before accepting the design

1. Integrate a real, appropriately licensed JPEG XL / AVIF / WebP decoder and inventory the actual output layouts, stride requirements, and borrowing lifetimes.
2. Specify chroma sample coordinates (siting, pixel centers, odd origins), format-aware footprint mapping, and a high-quality reference for planar-to-planar and planar-to-RGB resizing.
3. Evaluate a multiformat `Surface`/`Plane` representation only after at least two structurally different decoder adapters; avoid a premature universal hierarchy.
4. Add explicit content/quality policies for values outside the nominal 10-bit range, HDR transfer, alpha, ICC/CICP conflict, and device residency.
5. Revisit whether a region sink should expose borrowed **plane views** rather than separately forwarding each scanline.
