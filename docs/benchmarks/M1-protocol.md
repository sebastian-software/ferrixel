# M1 quality and performance baseline protocol

**Status: provisional harness, not a completed comparative benchmark.** [M1 milestone](../milestones/M1-complete-image-pipeline.md).

## What is measured

The engine's own reported peak *managed* allocation does not cover decoder/encoder scratch or operating system buffers. Measure **process peak RSS** and wall time externally in addition to the internal plan report. Compare peak allocations, output size, dimensions, image content and results across equal output quality.

The benchmark script `scripts/bench-m1.sh` builds a `--release` CLI and runs a real encode repeatedly. It prints CSV `iteration,elapsed_seconds,peak_rss_kib,output_bytes` and records the compiler/host in stderr. This is a first single-image measurement harness, **not a completed assessment against libvips/Sharp**.

```sh
FERRIXEL_PROFILE=assume-srgb FERRIXEL_FILTER=area \
  scripts/bench-m1.sh photos/untagged.jpg 1600 900 5 > m1-ferrixel.csv
# For ICC-tagged RGB input, use FERRIXEL_PROFILE=rendered-icc.
```

Collect multiple realistic images: small photos (hundreds of pixels), 12/24/48-Mpx images, transparent overlays, odd dimensions, P3, ICC-tagged JPEG, JPEG XL, AVIF, and adversarial extreme downscales. Record orientation. The tiny committed `sample.jpg` is a correctness fixture, **not** representative of throughput.

## Comparative fairness requirements

1. Verify **the same crop, resize geometry and orientation**, including border modes and antialiasing footprint.
2. Use consistent SDR output profiles, alpha association and an independent quality oracle. For lossy AVIF, **do not compare elapsed time at different quality** simply because both libraries use a value labeled `80`.
3. Tune codecs to comparable SSIMULACRA2/Butteraugli and document actual output bytes and metadata. JPEG XL input should preserve the same source frame/color semantics.
4. Report setup/cold vs warm latency, p50/p95, peak RSS, memory budget failures, total throughput under multiple requests, and materialization count (codec-owned versus engine-owned).
5. Execute `vips`/Sharp separately with explicit, documented equivalent settings, then independently decode outputs and compare. The script **does not** claim those comparisons until verified.
6. Record CPU model, core count, architecture, OS, rustc version, native assembler availability, codec crate versions, full command line and number of repetitions.

## M1 scorecard

| Check | Status |
| --- | --- |
| Scalar color/resampling reference tests | In progress |
| Real JPEG → AVIF end-to-end roundtrip | In progress |
| JPEG XL → AVIF with opt-in rendered ICC | In progress |
| AVIF → AVIF and high-bit-depth paths | In progress |
| Peak RSS across a realistic corpus | Not measured |
| Quality-matched Sharp/libvips comparison | Not measured |
| Multiple concurrent images and backpressure | Not measured |
| Robustness/fuzz corpus | In progress |

Do not mark M1 release-ready based only on a small set of passing unit tests.

## CI synthetic feasibility comparison (not parity)

The separate [M1 Synthetic Performance Smoke](../../.github/workflows/m1-performance-smoke.yml) generates a deterministic 1280×853 JPEG, transcodes it to 640×427 AVIF with Ferrixel, and tries an analogous `vipsthumbnail --linear` libvips operation. It records two wall-time/peak-RSS/output-size observations per backend and captures environment details in downloadable workflow artifacts.

**Interpretation:** output `Q=80` is not equal fidelity between `ravif` and libvips' AV1 encoder; libvips may use a different filter, chroma mode and internal parallelism. Results prove a runnable tool and provide resource *smoke data*, not a speedup/quality claim. The mandatory fair Sharp/libvips corpus comparison, matching image quality through SSIMULACRA2/Butteraugli, remains an open M1-RC gate.

## 2026-10-31 first measured smoke, not quality parity

GitHub Actions [run 41234019887](https://github.com/sebastian-software/ferrixel/actions/runs/41234019887) measured a fixed synthetic 1280×853 JPEG to 640×427 AVIF (quality label 80, speed 10) on a hosted Linux runner. In two repetitions, Ferrixel's end-to-end wall time was **0.69 seconds** each; observed peak RSS was **24,908 KiB** and **24,680 KiB**, with **159,086 output bytes**. These values are narrow *smoke observations* on transient CI hardware, not portable throughput claims. The runner's libvips/libheif reported **`heifsave: Unsupported compression`**, so no end-to-end libvips AVIF comparator was obtained. A subsequent workflow revision captures an explicitly labeled PNG resize-only fallback; it still cannot establish AVIF or quality parity.

## Sharp AVIF feasibility baseline

The smoke workflow optionally installs a **pinned Sharp 0.34.5** only on the GitHub runner and records its `JPEG → Lanczos3 resize → AVIF` end-to-end wall time, peak process RSS and encoded file size. It has an independent embedded libvips/libheif build, which may provide AVIF even when the OS libvips package does not. Values are labeled `comparable_quality=no`: Sharp's `quality=80` does **not** represent the same compression/distortion as `ravif quality=80`, and Sharp's default transfer, resampler and threading behavior differs. The method is a **feasibility baseline only**. Do not infer performance leadership until output fidelity has been matched with independent decoders and a perceptual quality metric.

## Sharp AVIF observations (smoke only)

GitHub Actions [run 41235323253](https://github.com/sebastian-software/ferrixel/actions/runs/41235323253) extended the reproducible synthetic 1280×853 JPEG → 640×427 experiment to pinned **Sharp 0.34.5** with its own libvips binary. Two repeats on that run recorded:

| Backend | Time (s) | Peak RSS (KiB) | Output (bytes) |
| --- | ---: | ---: | ---: |
| Ferrixel, AVIF (ravif) | 0.69, 0.69 | 24,916; 24,868 | 159,086 |
| Sharp 0.34.5, AVIF | 0.11, 0.11 | 85,940; 85,732 | 254,757 |
| OS libvips, PNG resize-only | 0.06, 0.06 | 62,428; 62,224 | 795,223 |

Both AVIF paths used a nominal quality setting of 80 but **not an independently verified equivalent perceptual quality**, encoder speed, chroma mode, transfer curve, threading policy or output profile. In this run Sharp was faster, while Ferrixel's reported process RSS was lower; **neither observation establishes a quality-matched advantage**. The OS libvips package still could not encode AVIF, so that PNG result is not an AVIF competitor.

The benchmark remains a runner-specific feasibility smoke. Its reference input is synthetic and cannot replace a real-image corpus or SSIMULACRA2/Butteraugli quality matching. Artifacts contain the CSV and environment for reproducibility.
