# Real decoder source experiment

**Status: experimental and private.** No stable public processing API, no output codec or performance claim.

## Source to sink

```text
immutable graph: decoder source → crop → crop
                         |
               shared region planner
                         |
                  requested ROI
                         |
          +--------------+---------------+
          |                              |
      JPEG / zune-jpeg                JPEG XL / jxl-oxide
      full RGB8 frame                cropped render request
      managed framebuffer           f32 output-row scratch
          |                              |
      borrowed u8 rows               borrowed f32 rows
          +--------------+---------------+
                         |
                    callback sink
```

## What the experiment measures

| Property | JPEG | JPEG XL |
|---|---|---|
| Read-only input | Yes | Yes |
| Access | Full-frame fallback | Codec ROI request |
| Output | RGB8 | Float interleaved, 1–5 channels |
| Known full-frame buffer | One engine-managed | None *created by Ferrixel* |
| Engine-managed scratch | Complete RGB8 output | One f32 output row |
| Internal decoder memory cap | No | No |
| Profile | Embedded ICC if present | Rendered ICC / CICP |
| Orientation | Currently not applied | Non-default currently rejected |
| Resize, encode | Not implemented | Not implemented |

A JPEG XL region request does **not** establish that codec reconstruction outside the region was avoided. In the future, add profiling of actual groups/bytes decoded if the codec exposes it.

## Architectural questions retained

1. Put complete color/profile semantics into a graph-level image contract without making every node own expensive metadata.
2. Understand the intersection of decoder granularity, sink row order and logical region footprints.
3. Decide how codec-managed allocations are estimated, limited or reported, independently of engine-managed memory.
4. Support JPEG XL orientations and real metadata before exposing a user-facing API.
5. Build a format-independent reference resize/color path with explicit precision and alpha semantics.
6. Consider AVIF by composing permissively licensed container demux + AV1 decode if practical, without licensing shortcuts.

The first goal remains **five-year architectural quality**, not matching a JPEG throughput benchmark.
