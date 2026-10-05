<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Renderer integration notes (`crates/render`)

Status: **M2 design** (meshes, materials and shared buffers in §2), **M4 skinning** (§2.4). The clean-room implementer of
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

### 2.4 Skinned meshes (M4)

Tessera's own design on top of the format facts of `skeleton-anim.md` §4 (vertex bone indices
address the skeleton's bone array directly; at most 4 influences; at most 248 bones, largest
index 247; linear blend skinning; bind pose = reference pose) and `mesh.md` §5.7 (weights sum
to 1 within 2 × 10⁻⁴; unused slots weight 0, index 0).

| Topic | Choice | Where |
|-------|--------|-------|
| Input | A skinned mesh is a `MeshData` plus a `MeshSkin` side stream: four `u8` bone indices and four `f32` weights per vertex, and `palette_size` (an upper bound on the bone indices checked at upload, normally the skeleton's bone count, `1..=256`; draws do not compare palettes against it, see Draw; `MeshSkin::default()` uses 256 so struct-update syntax validates). A side stream rather than a new `MeshData` field, so existing struct literals in other crates keep compiling. `Renderer::upload_skinned_mesh`, or `MeshData::prepare_skinned` on a worker thread + `upload_prepared` | `skin.rs`, `mesh.rs` |
| Validation (`MeshSkin::validate`, error `RenderError::InvalidSkin` naming the vertex) | both streams have one entry per position; `palette_size` in `1..=256`; every weight finite and ≥ 0; the four weights sum to 1 within **10⁻³** (`WEIGHT_SUM_TOLERANCE`, five times the observed deviation); every bone index **with a non-zero weight** is below `palette_size`. A zero-weight slot may hold any index (it is packed as bone 0). A vertex with all weights 0 is rejected (unskinned parts are drawn as static meshes, `skeleton-anim.md` §9.1) | `skin.rs` |
| Packing | second vertex stream, 12 bytes per vertex: bone indices `uint8x4`, weights `unorm16x4`. Weights are renormalised by their sum and quantised so the four values sum to exactly 65535 (floor, then the remaining units to the largest fractional parts, ties to the lower slot); a slot whose weight quantises to 0 gets bone 0. `MeshSkin::drawn_weights` returns these quantised weights as `f32` so CPU code can match the drawn positions | `GpuSkin` |
| Arena | the skin stream lives in a **skin buffer parallel to the page's vertex buffer** (12 bytes per vertex slot), so the rebased page indices address both streams and draws bind it once per page. It is created when the first skinned mesh lands on a page, counted in `max_total_bytes`, and released with the page's last skinned mesh, except on the warm page 0 (default size), which keeps it like the page itself so a character streaming in and out does not reallocate it; an idle skin buffer is released when an upload would otherwise exceed `max_total_bytes`. Skinned uploads prefer pages that already have one and never attach one to an oversized page. `ArenaStats::skinned_meshes` / `skin_bytes_used` | `arena.rs` |
| Palette | per frame: `Frame::skin_palette(&[Mat4])` (or `skin_palette_rows(&[[f32; 12]])`, the layout `tessera-anim` writes) appends one palette (1..=256 matrices; only the top three rows, the affine part, are used; a non-finite matrix becomes the identity) to the frame's palette data and returns a `SkinPalette` handle. Each palette starts at the device's dynamic uniform offset alignment. `end_frame` uploads all palettes of the frame with **one** `write_buffer` into a growable uniform buffer (power-of-two growth; the draw bind group is rebuilt only when it grows). Budget: 64 MiB of palette data per frame, or the device's `max_buffer_size` if smaller (`LimitExceeded` from `skin_palette` beyond). The buffer is sized to one full 12 KiB binding from the last palette's start, the same quantity `skin_palette` checks against the budget, so an accepted palette never fails `end_frame`. The draw-uniform and palette buffers are both reserved before either error is returned, and the bind group is rebuilt if either was replaced | `skin.rs`, `frame.rs`, `renderer.rs` |
| Binding | group 1 holds the draw uniform (binding 0) and the palette (binding 1), both **dynamic-offset uniform** bindings in the one draw bind group; the palette binding covers 256 × 48 B = 12 KiB, below the 16 KiB uniform binding size every adapter offers, so no storage buffers in vertex shaders are needed (downlevel/GL adapters work). A skinned draw sets `[draw offset, palette offset]`; static draws pass offset 0. No buffer or bind group is created per palette or per draw | `mesh_pipelines.rs` |
| Draw | `MeshDraw::with_skin(palette)`; parts of one character share one palette. A skinned mesh without a palette draws its bind pose through the static pipelines; a palette from another frame, from a frame of another renderer (palettes carry the renderer id and the per-renderer frame serial), or shorter than `max_joint + 1` (`SkinInfo::required_palette_len`; `palette_size` is not checked here) is ignored with a warn-once and the bind pose is drawn; a palette on a static mesh is ignored. `transform` is applied after the palette (palette: bind-pose model space → posed model space) | `renderer.rs` |
| Pipelines | every surface kind (opaque, alpha test, alpha blend incl. two-sided back/front, dye, normal map, specular, unlit) and every debug view (base colour, normals, UV checker, wireframe, normal lines) has a skinned variant (`MeshKey::skinned`) with the same fragment code; only the vertex stage differs. Surfaces sort by (alpha-tested, skinned, cull, material, page) | `mesh_pipelines.rs`, `mesh.wgsl` |
| Shader formula | linear blend skinning written as **v + Σₖ wₖ (Mₖ v − v)** for positions and, without translation, for normals and tangents (renormalised in the fragment stage; the tangent's bitangent sign is kept). With weights summing to 1 this equals Σₖ wₖ Mₖ v; with identity matrices every term is exactly 0, so the **bind pose reproduces the static path bit for bit** (tested for every material mode and debug view on lavapipe and llvmpipe GL, and on 24 real meshes). Zero-weight slots are skipped | `mesh.wgsl` |
| Statistics | `FrameStats::skinned_draws`, `palettes`, `palette_bytes` | `renderer.rs` |
| CPU cost | `cargo bench -p tessera-render --bench skinned`: 50 characters × 30 skinned parts (1,500 draws, one 89-bone palette per character and frame) cost 1.27 ms of CPU per frame against 1.15 ms for the same draws unskinned (RTX 2070 Vulkan; lavapipe 1.67 / 1.70 ms), most of it the per-draw work shared with the static path | `benches/skinned.rs` |

Real data (`crates/render/tests/skinned_real_data.rs`, 2026-10-05): all 19,049 skinned level-0
meshes of the owner's install (22.5 M vertices; 1 to 4 influences, largest bone index 247)
pass `MeshSkin::validate` with a 256-entry palette; 24 of them drawn with an identity palette
equal the static path pixel for pixel. Golden images: `crates/render/tests/skinned_golden.rs`.

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

> **Resolved (M4, 2026-10-05):** Skinned meshes: bone indices and weights are a `MeshSkin` side
> stream of `MeshData` with a GPU skinning path (§2.4), from the facts of `skeleton-anim.md` §4
> and `mesh.md` §5.7.

> **OPEN:** Weight renormalisation. The renderer renormalises each vertex's weights to sum
> exactly 1 when packing (§2.4), which removes the |1 − Σw| · |v| bind-pose offset that
> `skeleton-anim.md` §15 reports for unnormalised weights (up to 4.4 mm on large models).
> `mesh.md` §5.7 says no renormalisation is needed; whether the original client renormalises
> is open, as is whether 10⁻³ is a safe acceptance tolerance (all real data passes it).
> The CPU and GPU skinners currently disagree: `tessera-anim`'s CPU reference skinner
> (`skeleton-anim.md` §15) blends the stored weights as they are, so its positions differ from
> the drawn ones by up to |1 − Σw| · |v| (up to 4.4 mm; a review's GPU-vs-CPU probe on three
> real characters saw at most 15 differing pixels in a 320 × 480 frame). The renderer keeps
> renormalising: its shader form v + Σₖ wₖ (Mₖ v − v) needs Σw = 1 to equal Σₖ wₖ Mₖ v at all (with unnormalised weights it gives
> Σₖ wₖ Mₖ v + (1 − Σw) v, which matches neither convention), and renormalised weights keep
> the bind pose exact. Proposed single convention: CPU skinners renormalise too, or blend with
> `MeshSkin::drawn_weights`, which agrees with the GPU to quantisation error. Needs the owner
> of `tessera-anim` / `skeleton-anim.md` §15 to adopt it.

> **OPEN:** Mixed levels. `tessera-mesh` gives unskinned parts of a skinned level weight 0 on
> every slot; `MeshSkin::validate` rejects such vertices, so a caller must draw those parts as
> static meshes. None occurs in level 0 of the owner's install (`skeleton-anim.md` §2.4); if
> the spec confirms they are rigid in model space (§9.1), the renderer could accept all-zero
> weights as "unskinned" (the shader formula already leaves such a vertex unchanged).

> **OPEN:** Mirroring skinning matrices. A palette matrix with a negative determinant (a
> mirrored bone, e.g. the half-turn face bone of `skeleton-anim.md` §5.1 composed with a
> reflection) would flip triangle winding and the bitangent; the renderer only accounts for
> mirroring in the draw transform. No reflecting skinning matrix is known in the data.
