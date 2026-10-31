# M1 — First complete image pipeline

- **Status:** Implementation under validation; release-candidate gates open
- **Created:** 2026-10-31
- **Audience:** Ferrixel maintainers and contributors
- **Horizon:** The next coherent product milestone, not a date-driven release commitment
- **Scope:** Correct, usable **SDR** image transformation ending in **AVIF**
- **Companion research:** [Encoder and color-management candidates](../research/2026-10-31-encoder-and-cms-candidates.md)
- **Architecture direction:** [ADR-0016](../adr/0016-working-color-space-and-transform-boundaries.md) (Proposed)

## Milestone statement

> Ferrixel can read a real JPEG or JPEG XL image, interpret its *rendered* color meaning under an explicit policy, crop and resize with an image-quality reference, and produce a valid AVIF file using the **Ferrixel pipeline** rather than an eager general-purpose image-library pipeline.

The engine and graph remain independent of concrete codec libraries. A small Rust example and CLI exercise the same core. Color, alpha, materializations, codec fallbacks, and engine-owned memory are observable.

**AVIF output is the first required encoder.** JPEG XL output is an important follow-up, but it is a **separate decision gate** because the most substantial pure-Rust encoder candidate has AGPL/commercial licensing. The M1 plan does not import it by default.

## Acceptance contract

M1 is done only when all of these conditions hold:

1. **A real round trip:** JPEG and JPEG XL inputs reach a valid AVIF output through the shared graph, planner, executor and encoder sink. An AVIF decoder is added when its safety, license and output contract have been validated. Supported/unsupported input cases are explicit.
2. **Correctness over apparent speed:** A documented default SDR output profile, genuine linear-light resizing with alpha handled in the declared association mode, and tests against independent color/resize oracles. Unknown ICC, contradictory ICC/CICP and PQ/HLG inputs are rejected (or require a separately specified explicit conversion policy). No silent gamut or tone-mapping guesses.
3. **Modern format capability awareness:** The plan reports exact codec ROI support versus a full-frame fallback, and cannot claim that a codec skipped internal work merely because it returned a crop.
4. **Bounded engine-owned intermediates:** No full-frame *transformation* buffer by default on the supported crop/resize path. Buffer peaks, copies, transfers, known unavoidable codec materializations, and opaque codec memory are reported separately. Do **not** assert a hard process-wide memory bound that external codecs cannot honor.
5. **Operationally useful behavior:** Cancellation, malformed/truncated input, geometry overflows, orientation, alpha, output profile signaling and memory-limit failures have reliable tests and errors; the caller can obtain a deterministic execution trace.
6. **Measured baseline:** Comparable output-quality comparisons against libvips/Sharp and at least one independent decoder/reference. Record image sizes, formats, color/quality settings, hardware, threads, latency and peak RSS. No claim of universal performance superiority.
7. **Release discipline:** The internal contracts and licensing have been reviewed; a documented, intentionally small API and a CLI example can be exercised on supported platforms. No stable 1.0 API is implied.

## Ten ordered work packages

The order is intentional: **semantic correctness → complete data flow → performance evidence**. A work package may contain more than one pull request, and its design must be validated before it becomes Accepted in an ADR.

| # | Work package | Exit evidence / acceptance test | Dependency |
| --- | --- | --- | --- |
| 1 | **Explicit working-color contract** | Represent encoded vs linear domains, source and destination primaries, output profile, alpha association and precision independently of buffer residency. A typed plan rejects ambiguous conversions. No global RGBA buffer is mandatory. | ADR-0015 and Proposed ADR-0016 |
| 2 | **SDR reference transforms and CMM decision** | Tested Display P3 ↔ sRGB matrix/transfer reference; evaluate a license-compatible ICC CMS (e.g. `moxcms`) with actual ICC test profiles; preserve out-of-gamut linear samples until an explicit mapping/quantization step. Do not equate an ICC header with a transform. | 1 |
| 3 | **Quality resampler** | Separable antialiased area/Lanczos3 reference, filter halos, edge policies, gamma-correct alpha compositing; compare with an independent oracle for upscaling, extreme downscaling, odd ROI and transparency. | 1–2 |
| 4 | **Execution planner and memory-aware scheduling** | Proper region/scanline work-unit scheduling, no repeated full JPEG decode per output line, measured overfetch/halo duplication and peak managed memory, backpressure and cancellation across stages. | 3 + existing region engine |
| 5 | **AVIF read adapter and modern decoder audit** | One permissively usable and safety-reviewed AVIF decoder behind a feature; correct YUV/10-bit/alpha and output-profile contracts, explicit full-frame/region capability reporting and opaque codec cost accounting. Existing JPEG/JXL decoders remain green. | 1–2 |
| 6 | **Encoder sink and materialization contract** | A typed sink describes accepted layouts/profiles, row vs frame access, latency and memory requirements. Planner can emit an explicit `Materialize` decision rather than pretending a codec streams. | 1 + 4 |
| 7 | **First AVIF output adapter** | License-reviewed pure Rust encoder candidate (start with `ravif`); a real decoded → transformed → AVIF file with verified dimensions, output metadata, alpha and independent roundtrip tests. | 2–3 + 6 |
| 8 | **Small public facade and CLI** | A minimal Rust API and CLI example (`input → crop → resize → AVIF`) sharing the same core, with explicit quality, color intent and resource options; no generic plugin framework. | 7 |
| 9 | **Reference corpus, profiling and benchmarks** | Repeatable quality and performance baselines including HDR/ICC rejection cases, JPEG/JXL/AVIF test inputs, many small images and very large dimensions; profile decoding, transforms and encoding separately and end to end. | 7–8 |
| 10 | **Hardening and M1 candidate** | Fuzzing/property tests, malformed ICC/AVIF/JXL corpus, allocator and cancellation audits, MSRV/lints/features matrix, license/NOTICE audit, documented platform support, release decision. | 1–9 |

### Checkpoints

- **M1-A (Correct core):** Items 1–4. A coherent reference transform and region scheduler, with synthetic + real decoder tests.
- **M1-B (Usable image tool):** Items 5–8. Real JPEG/JXL → AVIF output and a small practical interface.
- **M1-RC (Release candidate):** Items 9–10. Independent quality evidence and operational hardening.

## Architectural guardrails

- **The pipeline is the product.** Codecs and backends are replaceable, not hidden inside graph nodes.
- **Correctness and provenance precede optimization.** Never convert or drop color information unless the semantic plan explicitly permits it.
- **No universal eager intermediate.** An explicit materialization is legitimate where a codec requires it; silent or avoidable frame copies are not.
- **All codec memory claims are honest.** Engine-owned memory can be budgeted; foreign codec memory may be opaque or only estimated.
- **CPU first, future GPU compatible.** Retain residency and transfer concepts, but do not introduce a GPU framework or bespoke SIMD language to satisfy M1.
- **Narrow dependencies.** The central crate stays small. Encoder and CMS adapters are feature-gated and must pass a license/security review.
- **ADRs remain living.** `Proposed` ≠ committed; use an experiment and an updated ADR before locking in interfaces.

## Risks and decision gates

| Risk | Gate / fallback |
| --- | --- |
| Untagged JPEG input has no provable rendered color meaning | Require explicit caller choice (not an implicit sRGB default) or decline color-managed operations. Keep read-only JPEG support. |
| JPEG XL decoder emits F32 with ICC/CICP semantics not yet normalized | Validate decoder-output profile and transform stage before routing through the SDR resampler. |
| ICC parsing/CMS expands scope or has unsafe/profile-size corner cases | Validate `moxcms` against independent ICC fixtures behind a feature. Retain explicit unsupported error paths. |
| AVIF encoder consumes full-frame RGB or reallocates internally | Report the buffer as an encoder-required materialization, measure peak RSS and leave room for future region-native alternatives. |
| JPEG XL encoding dependency is copyleft/commercial or its row API buffers a whole frame | No dependency in the default crate. Explore a permissive encoder, an optional FFI/backend with explicit limitations, or defer JXL output until after M1. |
| Strong downscale still performs redundant decoder work | Instrument requested vs. generated pixels and tune regions based on the real codec capabilities. |

## After M1: separate goals

- **M2:** Multi-output DAGs, shared intermediates, better adaptive scheduling, measured SIMD specializations, conditional GPU execution, and portable/embedded deployments.
- **M3:** Broader HDR/10-bit fidelity (PQ/HLG, tone mapping, scene/display reference white), more formats and animations, additional encoders including JPEG XL where licensing and capabilities permit.

Neither phase is a prerequisite for the first actually useful image pipeline.

## Progress note — first working-color reference

A small, dependency-free `src/working_color.rs` reference experiment covers SDR sRGB and Display P3 transfer/gamut arithmetic and premultiplied straight alpha. It validates the math for work package 1, **not** the entire graph/region transform, ICC conversion or M1 exit criteria. See [ADR-0016](../adr/0016-working-color-space-and-transform-boundaries.md). Work package 1 stays open until the color-domain contract is expressible and executable through the actual planner.

## M1 implementation evidence

The experimental pipeline has landed on the feature branch in individually themed commits. See [M1 evidence and remaining gates](M1-evidence-and-gates.md) for the current check results, known architecture risks, reproducibility and the distinction between a passing CI matrix and M1 release acceptance. This plan is not a claim that its ten steps have all passed their quality exit criteria.
