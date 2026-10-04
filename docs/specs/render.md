# Renderer integration notes (`crates/render`)

Status: **stub**. The clean-room implementer of `tessera-render` wrote this file. It holds Tessera's
own design decisions and the open questions that need spec facts. It contains no facts taken from
dirty-room material. Spec writers may replace or extend any section.

## 1. Current design (Tessera's own choices, not format facts)

| Topic | Choice | Where |
|-------|--------|-------|
| Texture input | Tightly packed RGBA8, top row first, one level, sRGB or linear | `TextureUpload` |
| Clip space | wgpu convention: right-handed view space, depth `0..1`, depth format `Depth32Float`, compare `LessEqual` | `camera.rs`, `pipelines.rs` |
| World up axis | Configurable (`UpAxis::Y` default, `UpAxis::Z`) | `OrbitConfig::up` |
| Orbit camera tuning | Values in `OrbitConfig::default()` are Tessera's own tuning, not measurements of any client | `camera.rs` |
| Screen space | Pixels, origin at the top-left, +Y down | `PassSpace::Screen` |
| UV origin | `v = 0` is the top edge of a quad | `quad.wgsl` |

## 2. Open questions

> **OPEN:** World coordinate convention in LOTRO data files: which axis is up, the handedness, the
> units (metres?), and how landblock-local coordinates map to world space. The renderer currently
> defaults to Y-up and leaves the choice to the caller (`UpAxis`).

> **OPEN:** Whether the renderer should accept BC1/BC2/BC3 (DXT1/3/5, records.md §5) blocks
> directly when the adapter supports `TEXTURE_COMPRESSION_BC`, instead of the formats crate decoding
> them to RGBA8. This affects memory: 4096² surfaces exist. The plan is to add a compressed path with
> RGBA8 as the fallback. The renderer needs to know whether any surface needs CPU-side processing
> before upload, such as the BC1 1-bit alpha question in records.md §5.3.

> **OPEN:** Mip chains (0x40 records list separate 0x41 levels): is level 0 always the largest,
> and does each level halve exactly (rounding down, minimum 1, or minimum 4 for DXT)? The upload API
> takes a single level today. A `mips` field will be added once this is specified.

> **OPEN:** The colour space of each surface format and category: which are sRGB-encoded colour
> data and which are linear data (normal maps, masks). This decides `ColorSpace::Srgb` vs
> `ColorSpace::Linear` per surface category (records.md §5.4).

> **OPEN:** Default third-person camera parameters (distance limits, pitch limits, pivot height,
> FOV) for a familiar feel. These must come from user settings or from public, observable
> behaviour described in a spec. They must never be read out of the client binary.
