<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Renderer integration notes (`crates/render`)

Status: **M2 design** (meshes, materials and shared buffers in §2). The clean-room implementer of
`tessera-render` wrote this file. It holds Tessera's own design decisions and the open questions that need spec facts. It contains no facts taken from
dirty-room material. Spec writers may replace or extend any section.

## 1. Current design (Tessera's own choices, not format facts)

| Topic | Choice | Where |
|-------|--------|-------|
| Texture input | Tightly packed RGBA8, top row first, one level, sRGB or linear | `TextureUpload` |
| Clip space | wgpu convention: right-handed view space, depth `0..1`, **reversed depth** (near plane → 1, far plane → 0; `Perspective::matrix` swaps the planes), depth format `Depth32Float`, cleared to `DEPTH_CLEAR` = 0, compare `DEPTH_COMPARE` = `GreaterEqual`. A float depth buffer then keeps roughly constant relative precision (≈ 1e-7 · distance) instead of the `distance² / near` error of standard depth, which would z-fight at landblock distances. Hand-made `PassSpace::World` matrices must use the same convention | `camera.rs`, `pipelines.rs`, `frame.rs` |
| World up axis | `+Z` up by default (`UpAxis::Z`, `Camera::default`), following the ARCH §11.1 decision that the engine's world space is Z-up; `UpAxis::Y` stays available. The procedural shapes (`MeshData::cube`, `uv_sphere`, `plane`, `tube`) are Y-up test placeholders | `OrbitConfig::up` |
| Orbit camera tuning | Values in `OrbitConfig::default()` are Tessera's own tuning, not measurements of any client | `camera.rs` |
| Screen space | Pixels, origin at the top-left, +Y down | `PassSpace::Screen` |
| UV origin | `v = 0` is the top edge of a quad | `quad.wgsl` |

## 2. Meshes and materials (M2)

All of this section is Tessera's own design. It fixes the renderer's input conventions so mesh
and material decoders know what to convert to; it contains no facts about the game's formats.

### 2.1 Mesh input (`MeshData`)

| Topic | Choice | Where |
|-------|--------|-------|
| Streams | positions (required), normals, tangents `[x, y, z, w]`, UVs, linear RGBA colours (optional, one per position), 16- or 32-bit triangle-list indices, submesh index ranges | `mesh.rs` |
| Winding | counter-clockwise front faces (wgpu default). Sources with clockwise front faces call `MeshData::flip_winding` | `MeshData` |
| Tangent frame | tangent = direction of increasing `u`; bitangent = `w * cross(normal, tangent)` = direction of increasing `v` (down the image, since `v = 0` is the top row) | `compute_tangents`, `mesh.wgsl` |
| Missing streams | normals: area-weighted face normals; UVs `(0, 0)`; colours opaque white; tangents: none, so no normal mapping (callers may run `compute_tangents`) | `prepare` |
| Validation | stream lengths agree; index count a positive multiple of 3; every index `< vertex count`; submesh ranges whole triangles inside the index list, and together covering **at most `index count` indices** (duplicated or overlapping ranges would multiply bounds/edge work and edge memory by up to the submesh count, so they are rejected unless the total stays within that bound); positions finite (rejected otherwise); other attributes sanitised (non-finite → default, colours clamped); `MeshLimits` (default 2 Mi vertices, 16 Mi indices, 4096 submeshes) checked before allocating. Work and memory are linear in the input; the edge list is at most `2 × index count` | `prepare_with` |
| Preparation | `MeshData::prepare(&MeshConfig)` does all CPU work (validation, packing, normals, edges, bounds) without a GPU and returns a `Send + Sync` `PreparedMesh`; `Renderer::upload_prepared` only re-checks its counts against the renderer's limits, drops edges when the renderer's `wireframe_edges` is off, and copies it. `upload_mesh` = `prepare` + `upload_prepared`. `validate(&MeshConfig)` returns exactly the `MeshInfo` an upload under that configuration reports | `mesh.rs`, `renderer.rs` |
| Bounds | AABB over all vertices, sphere centred on the AABB centre (radius = farthest vertex, computed in f64); per-submesh sphere over the vertices its triangles reference | `MeshInfo` |
| GPU vertex | 40 bytes: position f32×3, normal snorm16×4, tangent snorm16×4 (w = ±1), UV f32×2, colour unorm8×4 | `GpuVertex` |

### 2.2 Shared buffers

Meshes are sub-allocated from **pages** (one vertex + one index buffer each; default 16 MiB +
8 MiB) by a first-fit range allocator with coalescing. A mesh larger than a default page gets a
page of its own size. The total of all pages is capped (`MeshConfig::max_total_bytes`, default
1 GiB); uploads beyond the cap or beyond the device's buffer size fail with `LimitExceeded`.
Indices are stored rebased onto the page (no base-vertex draws, which some downlevel backends
lack). Optional wireframe edge lists (deduplicated triangle edges per submesh) share the
mesh's index allocation. Empty pages are released, except page 0 when it has the default size
(kept warm); a page sized for one large mesh is always released once empty, and an upload that
fails on the GPU releases the page created for it.

GPU errors of `upload_mesh`/`upload_prepared`, `upload_texture`, `create_offscreen_target`
and `read_pixels` are caught in wgpu error scopes and returned by that call (the failed
resource is not kept); other asynchronous errors go to the renderer-wide sink and are returned
by the next `end_frame`. `read_pixels` rejects a target whose padded readback buffer would
exceed the device's `max_buffer_size` with `LimitExceeded` before allocating anything.

### 2.3 Material pipelines

| Feature | Choice |
|---------|--------|
| Opaque | no blending, depth write, back-face culling |
| Alpha test | `discard` when `alpha < cutoff` (uniform per draw; non-finite → 0.5), output opaque |
| Alpha blend | `src·a + dst·(1−a)`, no depth write, drawn after all opaque/alpha-tested meshes, quads, lines and overlays of the pass, sorted back to front by the NDC depth of each submesh's bounding-sphere centre (stable) |
| Two-sided | no culling; back faces use the negated front normal (after normal mapping: the tangent frame is built from the unflipped normal, then the result is negated). Two-sided blended surfaces draw back faces then front faces |
| Mirrored transforms | negative determinant → culling and facing are swapped, bitangent sign flipped |
| Base colour | texture × factor × vertex colour |
| Dye | `rgb = base · mix(1, dye_color, mask)`, mask = R of a mask texture or the base texture's alpha (then coverage alpha comes from factor × vertex colour only) |
| Mask channels | single-channel masks are read from **R** (dye mask) or **RGB** (specular mask). A decoder that produces a mask from an alpha-only surface (A8 decodes to `(255, 255, 255, a)` in `tessera-formats`) must move the value into R (dye) or replicate it into RGB (specular) before upload; uploaded as decoded the mask is fully on everywhere |
| Normal map | tangent-space, RGB = XYZ·0.5+0.5, green along +v by default (`normal_green_up` flips) |
| Lighting | one directional light + ambient (per pass), Lambert diffuse, optional Blinn-Phong specular (colour, exponent 1..1024, optional RGB mask texture) |
| Draw order | opaque, then alpha-tested surfaces, each grouped by pipeline, material and page (stable among equal keys, so coplanar opaque draws of the same material keep submission order); then overlays; then blended surfaces back to front |
| Material cache | bind groups per distinct texture set are cached across frames; when the cache reaches 1024 entries (or twice the largest per-frame working set seen) entries unused in the current frame are evicted, so scenes with more texture sets than that do not re-create bind groups every frame |
| Debug views | base colour, world normals, UV checker (8×8 cells per UV unit), hidden surfaces; wireframe overlay; vertex-normal lines (instanced from the vertex buffer). Debug shadings draw blended materials opaque |
| Defaults | `Lighting::default()` is Tessera's neutral studio light, not a measurement |

Golden images for each pipeline: `crates/render/tests/mesh_golden.rs` (see the README next to
the PNGs).

## 3. Open questions

> **OPEN:** World coordinate convention in LOTRO data files: which axis is up, the handedness, the
> units (metres?), and how landblock-local coordinates map to world space. The renderer defaults
> to Z-up (ARCH §11.1 decision) and still lets the caller choose (`UpAxis`).

> **OPEN:** Whether mesh records can contain overlapping or duplicated submesh index ranges
> (e.g. one triangle range drawn with two materials). The renderer rejects meshes whose submesh
> ranges together cover more indices than the mesh has; a decoder meeting such data would have
> to duplicate the shared triangles into their own range.

> **OPEN:** Per-draw cost for M3 world rendering. Each `MeshDraw` takes its own 256-byte
> dynamic-offset uniform and one `draw_indexed` per submesh and face side, with no instancing.
> ARCH §13 calls for instancing of repeated static objects; the plan is an instanced path
> (per-instance transforms in a storage or vertex buffer, keyed by mesh, submesh and material,
> one `draw_indexed(.., 0..n)` per key) added with the M3 world renderer.

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

> **OPEN:** Triangle winding of mesh records (which side is the front). The renderer takes
> counter-clockwise front faces; the mesh spec must state the source convention so the decoder
> knows whether to call `flip_winding`.

> **OPEN:** Green-channel convention of game normal maps (along +v / "Y-down", or the opposite)
> and whether normal maps use two channels with Z reconstructed. Default here: three channels,
> green along +v.

> **OPEN:** The dye rule. The renderer implements a multiplicative tint by a mask
> (`material.md` is expected to state the real mask source, ARCH §12 reports "diffuse alpha",
> and the blend formula); until then `Dye` offers both mask sources.

> **OPEN:** Alpha-test thresholds and which materials blend versus alpha-test; whether blended
> surfaces in the original are sorted per object, per submesh or not at all. The renderer sorts
> per submesh by bounding-sphere centre.

> **OPEN:** Vertex colour encoding in mesh records (sRGB or linear, and whether alpha carries
> another meaning). `MeshData::colors` is linear RGBA.

> **OPEN:** Specular inputs (which texture channel is a specular mask, exponent source). The
> renderer accepts a colour, an exponent and an optional RGB mask. Likewise which channel the
> game's dye masks use and whether any mask is stored as A8; the material decoder has to produce
> the renderer's channel convention (§2.3 "Mask channels").

> **OPEN:** Skinned meshes: bone indices/weights streams are not part of `MeshData` yet; they
> will be added with the skeleton spec (M4, `havok-data.md`). Bind-pose meshes render as static meshes.
