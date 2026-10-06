<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Renderer integration notes (`crates/render`)

Status: **M2 design** (meshes, materials and shared buffers in §2), **M4 skinning** (§2.4),
**M4.5 lit pipelines, shared frame uniform and mip chains** (§4). The clean-room implementer of
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

<a id="s4"></a>
## 4. Lit pipelines and the shared frame uniform (M4.5)

Tessera's own design for drawing with the lighting of [`lighting.md`](lighting.md) §4–§10.
The equations are the spec's; the structure (one pass per light set, uniform layout,
bindings, draw order) and every row marked **Decision** are this crate's choices. Code:
`crates/render/src/lighting.rs` (types, uniform, CPU reference), `lit_pipelines.rs`,
`pass_hook.rs` (the render-world extension point, §4.2), `shaders/frame.wgsl`,
`shaders/frame_shading.wgsl`, `shaders/lit*.wgsl`, `shaders/shadow.wgsl`.

<a id="s4-1"></a>
### 4.1 Model

| Topic | Choice |
|-------|--------|
| Lit pass | A world pass becomes lit with `Pass::frame_lighting(FrameLighting)`. Its mesh draws with `MeshShading::Lit` go through the lit pipeline; debug shadings keep the M2 pipelines and the studio `Lighting` (§2.3). Passes without frame lighting draw exactly as in M2 (the M2 and M4 golden images are unchanged). |
| Light passes | **One base pass loops over all lights** (`lighting.md` §2.1 allows it): ambient + hemispheric, the sun with its shadow, and table lights, all per pixel: at most 8 lights per draw **in all, the main light included** (`lighting.md` §8.5), so 7 table lights when the pass has a sun and 8 without. The additive light passes 0x01–0x03 are folded in; since the base pass is lerp(Σ, fog, *f*) and each additive pass adds (…) × (1 − *f*), the sum is the same. Deviation: on translucent texels the folded light is weighted by the base pass's output alpha (§4.3), where the original's additive passes use *M*.alpha; the two differ only for base-texture opacity. |
| Per-pixel lists | Every table light uses the newer back end's per-pixel form: smooth fade over the last quarter of *R²* (§8.3 step 3) and the clamped specular of §5.8. Extra directional lights (§8.2) get diffuse and specular too (Decision; per-vertex in the original). |
| Colour space | Lit shaders compute on **stored values** (§5.1: no back end decodes sRGB). A material texture uploaded with `ColorSpace::Srgb` is re-encoded after sampling (per-texture flag in the draw uniform), so both upload kinds give the stored value (filtering then happens on decoded values, a rounding-level difference). Material, light, fog and vertex colours are taken as given. On an sRGB target the final colour is decoded before output so the stored pixel equals the computed value; blending on sRGB targets therefore happens on decoded values (Decision). |
| Unlit pass | `Material::unlit` in a lit pass: texture × *M* (× vertex colour when `LitSurface::vertex_colors`), fog when `LitSurface::fog` (§7 code 0x00). |
| Emissive and glow | *Em*.rgb × *G* added before fog (§5.9). Glow mask (slot 3013) × *G* is added the same way (**Decision**: the bloom composition is OPEN in §5.9). |
| Two-sided | Back faces keep the front normal (`material.md` §11.4: the programs read no front-face input). The M2 pipelines keep their flip. |
| Vertex colours | Multiply *M* (RGB and alpha) only when `LitSurface::vertex_colors` (§5.13); the alpha test uses the texture opacity before that. |
| Specular | Black (no term) when `Specular::shininess` < 0.5 (§5.8); power capped at 4096. `Specular::texture` is not used by lit passes; gloss comes from `LitSurface::gloss`. |
| Dye | The renderer's mask convention (§2.3, 1 = dyed); callers convert the data's polarity (`material.md` §12.3: low alpha = dyed). A dye masked by the base alpha (`DyeMask::BaseAlpha`) makes that alpha a dye mask only: `Opacity::BaseAlpha` then acts as `Opacity::One` in the alpha test, the output alpha and the shadow pass (`material.md` §11.2: testing the dye mask would punch holes in dyeable cloth). Second dye, second colour layer, environment maps and texture transforms are not part of the lit pipeline yet (not in the M4.5 deliverables). |
| Normal maps | *N* = *n*.x · **T** − *n*.y · **B** + *n*.z · **N₀** with **B** = +V (`lighting.md` §5.2.4): positive green tilts the normal toward the top of the image. Lit passes always use this sign; `Material::normal_green_up` serves only the M2 pipelines. This answers the green-channel OPEN of §3 for lit passes (three channels, no Z reconstruction). Mirrored transforms flip the bitangent sign so the mapping follows the mirrored texture. |

<a id="s4-2"></a>
### 4.2 Group 0 of every lit pipeline: the frame uniform

Every pipeline that draws into a lit pass (meshes here; terrain layers, water, sky and
instanced scenery in the render-world modules) uses **the same group 0**, so one bind group
per frame serves them all:

| Binding | Resource | Stages |
|---------|----------|--------|
| 0 | `FrameUniform` (12,624 bytes), uniform buffer with **dynamic offset** (one slot per lit pass, stride = size rounded up to `min_uniform_buffer_offset_alignment`) | vertex, fragment |
| 1 | sun shadow map, `texture_depth_2d` (`Depth32Float`) | fragment |
| 2 | comparison sampler (`LessEqual`, linear, clamp to edge) | fragment |

Shaders get the declarations by prepending `tessera_render::FRAME_WGSL` (`Frame`,
`FrameLight`, `@group(0) @binding(0) var<uniform> frame`) and, for the shading functions and
bindings 1–2, `FRAME_SHADING_WGSL`. A shadow-casting pipeline uses a group 0 with binding 0
only (the shadow map is its attachment).

**Extension point for the render-world modules** (`crates/render/src/pass_hook.rs`,
crate-internal). A module implements the `PassHook` trait and adds an `Arc<dyn PassHook>` to a
world pass with `Pass::hook` (typically from a public `impl Pass { pub fn water(..) }` in its
own file). `Renderer::end_frame` calls

- `PassHook::prepare(&HookPrepare)` once per pass in the gather phase, after the pass's frame
  uniform is known. `HookPrepare` carries the device and queue, the target format, the
  group-0 layouts of lit colour passes (`frame_layout`) and of shadow passes
  (`shadow_frame_layout`), the pass's `view_proj` and eye, its `FrameUniform` when lit, and
  its mesh shading. An error aborts `end_frame`.
- `PassHook::record(rp, &HookRecord, &mut FrameStats)` in three stages: `Shadow` (inside the
  sun's shadow-map pass, only for lit passes with a shadow, after the mesh casters),
  `Opaque` (after the opaque and alpha-tested mesh surfaces, before quads and lines) and
  `Translucent` (after the overlays, before the back-to-front blended mesh draws; hook draws
  are not sorted with them). `HookRecord::group0` is the bind group for that stage with the
  pass's dynamic offset: the frame bind group in the colour stages, the shadow-pass frame
  group in `Shadow`; `None` in passes without frame lighting.

A hook sets its own pipeline and every bind group it uses; the renderer re-binds its own
state afterwards. `Renderer::frame_layout()`, `shadow_frame_layout()` and
`frame_bind_group()` return the same objects. The time input (`eye.w`, E20) is reduced
modulo 3,600 once it exceeds 3,600 before upload, so callers pass the raw global clock.

`Frame` / `FrameUniform` layout (all `vec4` unless noted; offsets in bytes):

| Offset | Field | Contents |
|-------:|-------|----------|
| 0 | `view_proj` (`mat4x4`) | world → clip, reversed depth (§1) |
| 64 | `eye` | xyz camera position; w = time input in seconds (E20) |
| 80 | `up` | xyz world up *U* (unit; default +Z) |
| 96 | `sun_dir` | xyz unit vector **towards** the main light; w = 1 present, 0 none |
| 112 | `sun_color` | rgb *E₀* before ×*M* |
| 128 | `sun_specular` | rgb main-light specular before ×*Ms* |
| 144 | `ambient` | rgb frame ambient ½(*X* + *Y*) (§8.1) |
| 160 | `hemispheric` | rgb frame hemispheric ½(*X* − *Y*) |
| 176 | `fog_color` | rgb fog colour; w = 1 fog on, 0 off |
| 192 | `fog` | near, 1 / (far − near), far, far − near, after the §10 limits |
| 208 | `shadow_view_proj` (`mat4x4`) | world → shadow clip (orthographic, depth 0..1, 0 nearest the light) |
| 272 | `shadow` | quality (0 none, 1 one tap, 2 five taps), fade start, fade scale, texel size (1 / map size) |
| 288 | `shadow_color` | rgb landscape shadow colour; w = depth bias subtracted from the comparison depth |
| 304 | `shadow_range` | rgb shadow colour range |
| 320 | `params` (`vec4<u32>`) | light count, flags (bit 0: sRGB target), 0, 0 |
| 336 | `lights` (`array<FrameLight, 256>`) | 48 bytes each: `position_k` (point: position, *k*; directional: direction towards the light, 0), `color_r2` (colour, *R²*), `specular_kind` (specular colour, kind 0 point / 1 directional) |

Functions in `FRAME_SHADING_WGSL` (all prefixed `frame_`): `frame_ambient(n, m)` (§5.3),
`frame_sun(n, v, gloss, power, s)` (§5.4, §5.8), `frame_light(i, p, n, v, gloss, power)`
(§5.6, §8.3), `frame_attenuation`, `frame_spec_power`, `frame_shadow_coords(world)` (vertex
stage, §9 step 2), `frame_shadow_factor(coords, view_depth)` (§9 steps 3–7),
`frame_fog_factor(world)` (per vertex, §10), `frame_apply_fog`, `frame_output(c)` (decode
for sRGB targets), `frame_normalize`, `frame_srgb_to_linear`, `frame_linear_to_srgb`. Light
and ambient terms come back **before** the material colour; the caller multiplies by *M*
(and specular by *Ms*) as §4.2 says the engine does. `crate::lighting::reference` is the CPU
twin of these functions.

The light table holds at most 256 entries (`MAX_FRAME_LIGHTS`, so the uniform fits the
16 KiB uniform binding every adapter offers). Each draw receives at most 8 lights in all, the
sun included (`MAX_DRAW_LIGHTS`, `lighting.md` §8.5). The `FrameUniform` and `GpuLight` Rust
types are crate-internal; this table is the contract for WGSL authors.

<a id="s4-3"></a>
### 4.3 Per-draw inputs of the lit mesh pipeline

The surface state comes from the renderer's own `Material` (alpha, blend, two-sided as in
§2.3, extended); the inputs that only lit passes use are a `LitSurface` on the `MeshDraw`
(`MeshDraw::with_lit`).

| `AlphaMode` | Lit pass | Outside lit passes |
|-------------|----------|--------------------|
| `Opaque` | no blend, depth write, alpha 1 | as M2 |
| `Mask { cutoff }` | discard when opacity ≤ cutoff (§5.12), opaque output, depth write | as M2 (`alpha < cutoff`) |
| `TestBlend { cutoff }` (new) | discard when opacity ≤ cutoff, then **source-alpha blend with depth write** (lit `ColorAndOpacity`, `material.md` §11.3), drawn with the blended draws. A negative cutoff keeps every texel: `AlphaMode::BLEND_DEPTH_WRITE` (= `TestBlend { cutoff: -1 }`) is the lit translucent surface with alpha test off, whose base pass writes depth (`lighting.md` §2.1) | as `Mask` |
| `Blend` | source-alpha blend, **no** depth write, sorted (unlit blend records whose render state has depth write off; lit surfaces use `TestBlend` or `BLEND_DEPTH_WRITE`) | as M2 |
| `Additive` (new) | `src·a + dst`, no depth write, sorted; fog scales it by (1 − *f*) (**Decision**: `lighting.md` §6 and §10 state the (1 − *f*) rule for the additive light passes and §7 does not say how unlit SRCALPHA / ONE effects are fogged; lerping an additive effect toward the fog colour would brighten fogged distance) | as `Blend` |

Output alpha (`lighting.md` §5.12, `material.md` §11.3) = opacity × *M*.alpha when the opacity
source is the base texture (`Opacity::BaseAlpha`), else *M*.alpha; × vertex alpha where
`vertex_colors` is set. The opacity of a separate texture (`Opacity::Texture`, slot 3020) or of
the emissive alpha (`Opacity::EmissiveAlpha`, slot 3018) drives only the alpha test (and the
shadow pass's cut-outs). `LitSurface`: glow colour *G*,
emissive texture, glow mask, gloss source (`One`, `BaseAlpha`, `NormalAlpha`, `Texture`),
opacity source (`BaseAlpha` default, `One`, `EmissiveAlpha`, `Texture`), `vertex_colors`,
`fog`, `cast_shadow`, `lights` (`Auto`, `None`, `List`).

**Light selection (Decision of `lighting.md` §8.5):** `LightSelection::Auto` takes the point
lights whose range sphere touches the draw's bounding sphere (mesh sphere × transform),
nearest centre first (ties by table index), then the directional table lights in order, at
most 8 lights in all with the sun counted (7 table lights when the pass has a sun). The world
crate can pass its own choice with `LightSelection::List` (the same cap applies). The renderer
builds the selection data once per pass (directional indices; point lights sorted along x, so
a draw tests only the lights whose x range can reach it) and selects without allocating.

Draw uniform: the M2 `DrawUniform` (224 bytes, §2) grows by two vectors to 256 bytes, still one
256-byte dynamic-offset slot: `lit` (offset 224: rgb *G*, w specular power *p*) and
`lit_lights` (offset 240: x, y = eight light indices as bytes, z = count, w = flags: vertex
colours 1, fog 2, emissive 4, glow 8, specular 16, gloss source bits 5–6, opacity source
bits 7–8, sRGB re-encode bits 9–15 per material texture). Group 2 holds seven textures
(base, normal, dye mask, gloss, emissive, glow, opacity; samplers at binding + 7); an unset
or unresolved texture binds the white texture.

<a id="s4-4"></a>
### 4.4 Sun shadow (`lighting.md` §9, Tessera Decision "simple form")

| Topic | Choice |
|-------|--------|
| Cascades | One. `SunShadow::fit(toward_light, center, radius, distance, size)` builds an orthographic light view around a sphere, snapped to whole texels so the shadow does not shimmer, with five-tap quality, shadow colour 0, colour range 1, and a linear fade over the last fifth of `distance` (view depth). Callers may fill `SunShadow` themselves (cascade split, fade, bias). |
| Fit | `SunShadow::fit` keeps casters up to `SunShadow::DEFAULT_CASTER_DISTANCE` (1,000 world units, **Decision**) beyond the sphere toward the light, so tall terrain, buildings and trees outside the view sphere still cast; `fit_with_casters` takes the distance. The depth bias is 0.006 × radius in world units whatever that distance. |
| Map | `Depth32Float`, standard depth (cleared to 1, `LessEqual`); one map per renderer whose size is the largest `map_size` any lit pass of the frame asks for (64–4096, device limit); recreated when that changes. Only passes with `MeshShading::Lit` ask (debug shadings sample no map; their frame uniform gets quality 0). The map is created in a GPU error scope: when the allocation fails, the previous map stays (shadows keep working at its size if it is at least 64 texels, otherwise the frame draws without shadows) and the failed size is not retried until the request changes. After `Renderer::SHADOW_IDLE_FRAMES` (60) frames without a request the map is freed (1 × 1). `FrameStats::shadow_map_size` reports the size. |
| Pass | For each lit-shaded pass with a shadow, a depth-only pass renders the pass's casters (and the `Shadow` stage of its hooks, §4.2) before the pass itself (the map is cleared even with no casters). Casters: lit draws with `LitSurface::cast_shadow`, not unlit, alpha `Opaque` (depth only) or `Mask`/`TestBlend` (cut-outs discard by the opacity, §7 code 0x0A). Blended and additive draws cast no shadow. No culling; slope-scaled raster bias 1.5 plus `SunShadow::depth_bias` (default 0.0015) in the comparison. |
| Lookup | Five taps when quality ≥ 2 (weights 0.36 / 0.16), one otherwise; each tap is a hardware comparison with linear filtering (2 × 2 PCF on most adapters, Decision). Outside the map: lit. Shadow colour and fade as §9 steps 5–7. |

<a id="s4-5"></a>
### 4.5 Draw order of a lit pass

Shadow-map pass; then in the pass: opaque and alpha-tested surfaces grouped by (masked, lit,
skinned, cull, material, page); the hooks' `Opaque` stage; quads and lines; overlays; the
hooks' `Translucent` stage; then blended surfaces (M2 `Blend`, lit `TestBlend`, `Blend`,
`Additive`) back to front by bounding-sphere centre (§2.3). Group 0
switches between the M2 globals and the frame bind group as the pipelines change.

<a id="s4-6"></a>
### 4.6 Mip chains (`texture-list.md` §5.5)

`Renderer::upload_texture_mips(&MipChainUpload)` creates one texture from the active chain:
size of level 0, **one mip per stored level** (no generated levels; sampling below the last
level stays on it), level *i* in mip *i*. A level whose stored size differs from its slot
max(1, size ≫ *i*) is copied clipped to the smaller width and height, as the original does
(the §4.2 exception). Every level must carry exactly width × height × 4 bytes; levels beyond a
full pyramid are ignored. Samplers filter between mips (`Filter::Linear` is trilinear,
`Nearest` picks the nearest mip); single-level textures sample as before. Mip textures
cannot be updated in place. This supersedes the "one level" row of §1 and answers the
mip-chain OPEN of §3 (`texture-list.md` §4.2–§5.5); the walker still has to upload its
chains to close the aliasing OPEN of `walker.md` §9.

<a id="s4-7"></a>
### 4.7 Tests

- `crates/render/tests/lighting_examples.rs`: every worked example of `lighting.md` §14 on
  the CPU reference (sun-lit texel, point light, light record, ambient, cross-fade, alpha
  test), plus lit pixels read back from the GPU against the same reference.
- `crates/render/tests/lit_golden.rs`, golden image `lit_mesh.png`: a lit scene with a point
  light, the sun's shadow and fog; `lit_surfaces.png`: unlit, additive, test-blend,
  emissive and glow, vertex colours, two-sided cards, specular with gloss sources.
- `crates/render/tests/lit.rs`: hostile frame lighting (non-finite values, 306 lights, a
  shadow map request of `u32::MAX`), skinned lit draws and casters, overlays and debug views
  in lit passes, several lit passes with different shadow sizes next to an M2 pass, the
  idle shadow map freed after `SHADOW_IDLE_FRAMES` and no map for debug shadings.
- `crates/render/tests/lit_features.rs`: GPU pixels against `lighting.md` for the output
  alpha of each opacity source, the normal-map green sign (also mirrored), dye from a mask
  texture and from the base alpha (kept by the alpha test, casting a full shadow),
  directional table lights with `Auto`, `None` and `List`, the 8-lights-in-all cap, blending
  with depth write, casters beyond the fitted shadow sphere, and the M2 fallback of
  `TestBlend` (as `Mask`) and `Additive` (as `Blend`).
- `pass_hook.rs` unit test: a hook gets group 0 at each pass's own offset in every stage.
- `crates/render/tests/mip_chain.rs`: mip selection by on-screen size, a short chain, the
  clipped copy of a non-halving level, hostile chains.
- `crates/render/tests/mip_chain_real_data.rs` (`TESSERA_GAME_DIR`): 200 texture lists of
  the install resolved to their active chain at detail 3 without highres and uploaded
  (2026-10-06: 196 with several levels, 1,064 levels, every mip count as expected).

All of them pass on the reference GPU (RTX 2070 Super Max-Q, Vulkan), on Mesa lavapipe 24.2
and on llvmpipe GL (Mesa 26.1.8), and the M2 and M4 golden images are unchanged.

<a id="s4-8"></a>
### 4.8 Open items

> **OPEN:** The engine's shadow values (cascades, colour, fade) and the depth comparison of
> the additive passes are OPEN in `lighting.md` §6 and §9; the renderer uses the Tessera
> Decisions there and makes every value a `SunShadow` field.

> **OPEN:** Blending on sRGB targets happens on decoded values, so translucent and additive
> surfaces differ slightly from the original, which blends stored values. Rendering lit passes
> into an `Rgba8Unorm` target and presenting it unconverted would match exactly; not done
> because the M2 overlays and the surface format are sRGB.

<a id="s5"></a>
## 5. Render-world pipelines (M4.5): terrain layers, water, sky, instanced scenery

Tessera's own design for drawing the world data of [`terrain-texturing.md`](terrain-texturing.md)
§5–§10, [`water.md`](water.md) §8–§9, [`environment.md`](environment.md) §11.4–§12 and
[`scenery.md`](scenery.md) §8.3, §8.5, §9. The formulas are the specs'; the structure
(retained objects, uniform layouts, bindings, draw order) and every row marked **Decision**
are this crate's choices. Code: `crates/render/src/terrain_layers.rs`, `water.rs`, `sky.rs`,
`instanced.rs` and `shaders/{terrain_layers,water,sky,instanced,instanced_common}.wgsl`.
`tessera-render` does not depend on the terrain, world or material crates: they convert their
records into the render-owned input types below.

<a id="s5-1"></a>
### 5.1 Model

| Topic | Choice |
|---|---|
| Queuing | Per pass: `Pass::terrain(TerrainDraw)`, `Pass::water(WaterDraw)`, `Pass::sky(SkyDraw)`, `Pass::instanced(InstanceBatch)`. The draws are plain data held by the pass (`WorldDraws` in `frame.rs`). |
| Lit passes only | The four pipelines draw only in passes with `Pass::frame_lighting` (they need the shared group 0 of §4.2). In other passes their draws are skipped with one warning per frame; screen passes ignore them. |
| Hooks | `Renderer::end_frame` turns each pass's world draws into `PassHook`s (§4.2) during the gather phase, with renderer access (textures, the scene copy, pipeline caches), and inserts them before any other hook of the pass. Order in each stage: terrain layers, instanced scenery, sky, water. |
| Stages | `Shadow`: terrain chunks and opaque or alpha-tested instanced surfaces of batches that cast. `Opaque` (after the opaque and alpha-tested meshes): terrain layers, opaque and alpha-tested instanced surfaces, then the sky. `Translucent` (after overlays, before the sorted blended meshes): water, then blended instanced surfaces. Hook draws are not sorted with blended meshes. |
| Group 0 | The pass's frame uniform (§4.2) with its dynamic offset; the shadow stage uses the shadow-pass frame group. Every world shader prepends `FRAME_WGSL` and `FRAME_SHADING_WGSL`, so lighting, fog, shadows and the sRGB output rule (§4.1) are those of the lit mesh pipeline. |
| Retained vs per frame | Terrain chunks (`Renderer::upload_terrain_chunk` → `TerrainChunk`) and world models (`Renderer::upload_world_model` → `WorldModel`) own their GPU buffers and bind groups; handles are cheap `Arc` clones. A texture destroyed (`Renderer::destroy_texture`) while a retained object still uses it is dropped from that object when the object is next drawn: the affected bind group is rebuilt as if the texture had never resolved (white, its switch off), because a bind group holding a destroyed texture makes the GPU reject the whole frame's submission. Water quads, instances, sky objects and per-draw transforms and light lists are per-frame data packed into a few buffers per pass and frame. |
| Textures | Resolved when a retained object is created (terrain, models) or when the frame is built (water). Unknown or destroyed ids are treated as absent and logged once; registered render targets are never bound by retained objects (a later frame could render into them) and not by the frame that renders into them. Textures uploaded `ColorSpace::Srgb` are re-encoded after sampling, as in §4.1. |
| Foreign handles | Chunks and models carry their renderer's id; draws of another renderer's objects are skipped with a warning. |
| Pipelines | Built lazily per colour format and kept for the renderer's life (`TerrainState`, `WaterState`, `SkyState`, `InstancedState`). Feature switches are uniform flags in one shader per pipeline family, not pipeline permutations. |
| Statistics | `FrameStats::terrain_layers`, `terrain_chunks_culled`, `water_quads`, `sky_objects`, `instances` (each instance drawn in colour once, whatever the number of submeshes), `instances_culled`, `scene_copies`; their draws count in `draw_calls` (and `shadow_draws`). |
| Per-pass buffers | Every per-pass uniform buffer is bounded by the device (`max_buffer_size`, and dynamic offsets below 2³²). Terrain and instanced batches check this against the **queued** draws (one slot each), so a pass either always or never fails for the same input, whatever the view: `LimitExceeded`. |

<a id="s5-2"></a>
### 5.2 Terrain layers (`terrain-texturing.md` §5–§10)

**Inputs.** `TerrainChunkData`: the chunk's vertices (`TerrainVertices`: positions, normals,
tangents **T** and binormals **B** of §6.4, coordinate sets 0–3 of §6.1, one entry per vertex;
empty tangent and binormal streams mean a zero frame), its drawn layers in stored order and the
block's light map. A `TerrainLayer` holds its triangles (vertex indices), the projection
(`Xy`/`Xz`/`Yz` → set 0/1/2), the texture transform (`u_scale`, `v_scale`, `rotation` in
degrees, the evaluated `u_offset`/`v_offset`), the opacity rule of §7 (`TerrainOpacity`:
`Opaque` marks the first drawn layer, then `MaskedDiffuse { alpha_blend_max }`, `Mask`,
`Diffuse`, `One`), τ, `diffuse_scale` (*M*), the optional specular term (colour, power, gloss
from the diffuse or the normal alpha), `glow_colour` and the textures (diffuse, normal,
emissive, glow, mask). The terrain crate's `DrawLayer`, `LayerVertices`, `LayerFeatures` and
`LayerParams` map onto these one to one.

| Topic | Choice |
|---|---|
| Validation | `TerrainChunkData::validate` (no GPU): 1–65,536 vertices (16-bit indices), at most 256 layers and 524,288 triangles, every non-empty stream one entry per vertex, normals required, tangents and binormals both or neither, finite positions, every index in range. Errors: `InvalidMesh`, `LimitExceeded`. Non-finite normals, frames, sets and material values are sanitised (normal +Z, others 0 or their default). |
| GPU layout | Vertex 80 bytes: position, normal, tangent, binormal (`f32×3` each), sets 0–3 (`f32×2` each). One vertex and one 16-bit index buffer per chunk (all layers' triangles back to back). Layer uniform 112 bytes (*M* with `alpha_blend_max`, *Ms* with *p*, *G* with τ, the two rows of the §6.3 matrix, the normalised columns *c*₀, *c*₁ of §6.4, coordinate set, opacity rule, flags, sRGB bits), one slot per layer in a per-chunk uniform buffer. Per-frame draw uniform 128 bytes (model, normal matrix, light list), dynamic offset. |
| Bindings | Group 1: draw uniform. Group 2, one retained bind group per layer: layer uniform, diffuse, normal, emissive, glow, mask, light map, a **wrap** trilinear sampler for the layer textures (§6.3) and a **clamp** bilinear sampler for the mask and light map (§5.2, §10). Missing textures bind white with their switch off. |
| Missing mask | A later layer whose mask does not resolve (unknown, destroyed, a render target, or rejected upstream as non-A8) is drawn as if it had no mask (§5.1 Tessera Decision): `MaskedDiffuse` → `Diffuse` (α = *a*), `Mask` → `One` (α = 1), logged. |
| Switches (§8.1) | Normal map: the normal texture resolved; it perturbs the normal only with a tangent frame (a zero frame leaves *N*, §6.4). Gloss from the normal: requested, and the normal texture resolved (the frame plays no part). Specular: requested with power ≥ 0.5, **and** gloss comes from the normal or the opacity switch is off (the opacity rule does not read the diffuse alpha: `Opaque`, `Mask`, `One`); otherwise no highlight. Opacity, specular and gloss switches are derived after the missing-mask rule above. |
| Mask channel | The mask value is the texture's **alpha** (an A8 surface uploaded as `tessera-formats` decodes it, (255, 255, 255, a), stored row 0 first, sampled at set 3 so row 0 is the south edge, §5.2). A missing mask reads 1. (Unlike the M2 dye masks of §2.3, which read red.) |
| Draw order | Chunk by chunk, layers in stored order, one `draw_indexed` per layer, one pipeline for all layers. |
| Blend and depth | Source α, destination 1 − α ("over", §8 step 4) on every layer; the first drawn layer outputs α = 1. Depth test `GreaterEqual` under reversed depth, i.e. "less or equal" (§8 step 5 Tessera Decision), depth write on for every layer; the clip position is `@invariant` so every layer of a triangle produces the same depth. Back faces culled (terrain triangles are counter-clockwise from above, ceiling triangles from below). |
| Opacity and discard | §7 per pixel; discarded when α ≤ τ except on the first drawn layer. |
| Colour (§9) | Σ = light map RGB (baked-light switch) + `frame_ambient(N, M)` + sat(*N* · *L₀*) *S* *E₀* *M* + the table lights' diffuse × *M*; colour = 2 *C* Σ + *g* sat(*N* · *H*)^*p* *S* × sun specular × *Ms* + *Em* × *G* (emissive) + glow × *G* × *M* (glow), then the per-vertex fog. |
| Normal map | *n*′ = the texel turned by *c*₀, *c*₁ (§6.4), normalised, used in the frame (**T**, −**B**, **N**): *N* = *n*′.x **T** − *n*′.y **B** + *n*′.z **N**. A zero frame leaves *N* = **N**. |
| **Decision**: table lights | Per pixel with the smooth fade (as §4.1; the original sums them per vertex), diffuse only: §9.4 gives a specular term for light 0 alone. Selected once per chunk by its bounding sphere (`LightSelection`, §4.3). |
| **Decision**: glow | Added like emissive (the bloom target is OPEN, `lighting.md` §5.9), with the §9.5 glow-pass colour glow × *G* × *M*. |
| **Decision**: baked shadow | White (the landscape-shadow feature is off, §9.5 Tessera Decision): *S* is the shadow factor of `lighting.md` §9. |
| Layer offsets | `TerrainDraw::layer_offsets` replaces the translations of the layers (scrolling `u_offset`/`v_offset` curves evaluated by the caller at the material clock, §6.3): one entry per layer in stored order (a list of another length applies to the layers it covers, logged once per pass). When the frame is built, only the layers whose rows changed are written, with one buffer write per chunk covering them; the last value written in a frame applies to all its passes. |
| Limits | One 128-byte (aligned) draw slot per queued `TerrainDraw`, checked against the device's buffer limit before anything is built (`LimitExceeded`, about two million draws at 128-byte alignment). |
| Shadows | Chunks with `cast_shadow` draw their whole index buffer (every layer's triangles: exactly the covered triangles) into the sun's shadow map, no culling, the lit meshes' slope bias. |
| Culling | A chunk whose bounding sphere (× transform) is outside the frustum is not drawn in colour (`terrain_chunks_culled`) but still casts. |

<a id="s5-3"></a>
### 5.3 Water (`water.md` §8–§9)

| Topic | Choice |
|---|---|
| Inputs | `WaterDraw`: quads (`WaterQuad { origin, size, height }`; `WaterQuad::landblock` makes the 160 × 160 quad of a wet landblock, §8.1 Tessera rule), the surface's normal map, `WaterParams` (the `c_Water…` parameters of §8.2 with the 0x2B0000A8 shader defaults; `normal_layers` 1–3 for records that lack layers 1 and 2, §8.2 OPEN), grid cells per quad side (1–64) and the block offset *O* (default (0, 0), §8.3 Tessera rule). |
| Geometry | No retained mesh: one instance per quad (origin, size, height; 32 bytes) and `6 n²` generated vertices per instance (`n` cells per side, two counter-clockwise triangles per cell), so any number of quads of one surface is one draw. Hostile quads (non-finite, size ≤ 0) are dropped; at most 65,536 quads per pass (`LimitExceeded`). **Per-pass vertex budget:** all draws of a pass together generate at most 2²⁴ vertices: in queue order each draw's grid is halved until it fits in what the earlier draws left, minus one cell per quad for every later draw (one cell per quad always fits). |
| GPU layout | Per pass: one buffer of all quads, one uniform buffer with a 176-byte slot per draw at a dynamic offset, one bind group per distinct normal map (with the scene copy). One `draw` per `WaterDraw`. 4,096 single-quad draws at the largest grid cost 3.2 ms of `end_frame` CPU time on the reference machine (release, `water_golden.rs` `one_draw_per_quad_stays_bounded`, other builds running). |
| Distant water (§9) | The caller derives the wet landblocks of distant 3 × 3 sectors from the block maps (§9 Tessera rule) and draws them as more landblock quads (with one cell each); the pipeline treats near and distant quads alike. The sector texture coordinates of §8.1 feed only the low-detail permutations, which Tessera does not draw (the water permutations read only the position, §8.3). |
| Vertex stage | §8.3: waves Σ *A*ₖ sin *s*ₖ with *s*ₖ reduced to [−π, π), using the world x, y (the quads are generated in world space); the surface tilted by the wave derivatives; normal-layer coordinates *B* · Scaleₖ + Speedₖ · *T* with *B* = (*P*.xy − *O*) / 160; per-vertex fog. *T* is the frame's time input (`FrameLighting::time`, E20). |
| Scene copy | Water refracts the colour target as it is before the pass's translucent stage: `end_frame` ends the pass's render pass after the overlays, copies the target into a renderer-owned texture of the same size and format (`scene_copies` counts it), and continues in a new render pass that loads colour and depth. Surfaces are configured with `COPY_SRC` where the surface offers it; a target that cannot be copied draws the refraction as the water fog colour (logged once). An sRGB copy is re-encoded after sampling so the shader sees stored values. |
| Pixel stage | §8.4 steps 1–7: the three normal-map samples (re-encoded to the stored texels when the map was uploaded `ColorSpace::Srgb`, §5.1) mapped to 2(rgb − 0.5), summed and normalised; screen coordinates; refraction *F* of the scene copy offset by the world normal's x, y × `RefractionBumpDistortionAmount`, the under-water tint *U*, the shoreline blend with *w* = clamp((distance − 5) × 0.1, 0, 1); reflection = the unmodified *F* (§8.4 Tessera rule: no reflection texture); Fresnel; specular sat(*n* · normalize(*v* − *L*))^power + the sparkle term; colour × `WaterColor` × overbright + *s* × fake specular × `SpecularColor`; fog. |
| Render state | Source ONE, destination ZERO, alpha 1, depth test and write, both faces (§8.4 Tessera rule). |
| **Decision**: normal frame | The tangent-space normal is turned into world space with **T** = +x, **B** = +y (tilted by the waves) and the sign convention of `lighting.md` §5.2.4: *N* = *n*.x **T** − *n*.y **B** + *n*.z **N**. (`water.md` §8.4 step 1 says "the surface frame" without the signs; OPEN there.) |
| **Decision**: fake light | *L* = the main light's travel direction (−`sun_dir`) and its specular colour when the pass has a main light; otherwise straight down with a black specular colour (the engine's default direction and specular colour are not given, `lighting.md` E18; OPEN in `water.md`). |
| Missing normal map | A flat tangent-space normal (0, 0, 1): no ripples, no sparkle. |

<a id="s5-4"></a>
### 5.4 Sky (`environment.md` §11.4, §12)

| Topic | Choice |
|---|---|
| Inputs | `SkyDraw { objects }`: each `SkyObject` is a `WorldModel` (the geometry of the object's 0x47 record, uploaded by the caller with its surfaces), angles *A* and *B* in degrees, the keyframe tint (alpha = opacity) and a uniform scale. Visibility, angles and tints come from the caller's environment blending (§11.3–§11.4). At most 4,096 objects per pass. |
| **Decision**: placement | A model vertex *v* is drawn at eye + R_z(−*B*) · R_x(*A*) · (scale · *v*): *A* turns about the east axis (+y toward +z), *B* then turns clockwise seen from above (north toward east), like the heading of §11.3 step 6. With *A* = *B* = 0 the model keeps its axes (x east, y north, z up). `SkyObject::rotation` returns the matrix. (Which axes the original turns about is OPEN, §12; this realises the Tessera Decision "B is the azimuth of the arc, A the rotation along it".) |
| Depth | Drawn at the far plane (clip z = 0 under reversed depth) with `GreaterEqual` and no depth write, so the sky covers only pixels nothing else has drawn, whether it is queued in the same pass as the world (it draws after the opaque geometry, saving fill) or in an earlier pass. |
| Shading | Unlit and unfogged (Tessera Decision of §12): base texture × *M* × tint (× vertex colour when the surface reads it); output alpha = (base alpha unless the surface is opaque) × *M*.alpha × tint alpha. Alpha blending for every surface (additive for `Additive` surfaces), the alpha test for `Mask`/`TestBlend` surfaces, both faces. Objects are drawn in the given order (back to front); an object with opacity ≤ 0 is skipped. |

<a id="s5-5"></a>
### 5.5 Instanced scenery and frills (`scenery.md` §8.3, §8.5, §9)

| Topic | Choice |
|---|---|
| Models | `Renderer::upload_world_model(&MeshData, &[WorldSurface])`: the mesh is validated and packed as for `upload_mesh` (§2.1, the 40-byte vertex, no wireframe edges) into buffers of the model's own; submesh *i* uses surface min(*i*, len − 1). `WorldSurface`: *M*, base and normal textures (the upload's own samplers), `AlphaMode` (as for lit meshes, §4.3), two-sided, specular (colour, power), gloss from the base alpha or from the normal texture's alpha (a normal-and-gloss texture, when it resolves; added in the M4.5 verification: without it such surfaces took gloss 1 and frills of row F2 drew near-white), vertex colours, unlit. At most 4,096 surfaces. A texture that does not resolve (unknown, destroyed, a render target) is absent: the base binds white, and the normal-map switch is set only for a normal texture that resolved (and a mesh with tangents). |
| Batches | `InstanceBatch`: a model, `Arc<[Instance]>` (`Instance`: affine transform, tint multiplying *M*, the terrain colour beneath), the light selection (once per batch, by the box around its drawn instances), shadow casting, an optional per-instance distance limit (`scenery.md` §9.1 Decision: `d0` of the LOD class) and optional `FrillParams`. |
| GPU layout | Per instance 80 bytes (three rows of the affine matrix, tint, terrain colour with a sway phase from the instance origin) in one vertex buffer per pass and frame; per batch a 48-byte uniform (light list, flags, *D*, *F*, animation and terrain-colour intensities, the model's z_min and sway height, fade-in) at a dynamic offset; group 2 the model surface's retained bind group. **One `draw_indexed` per batch and submesh** (and one more per casting submesh in the shadow stage). At most 1,048,576 instances per pass (`MAX_INSTANCES_PER_PASS`, lower if the device's buffer limit is): instances beyond it are **dropped** in queue order, counted in `instances_culled` and logged once per pass, so a dense view cannot fail the frame. The batch uniforms are bounded per queued batch (§5.1 Per-pass buffers). |
| Culling | Per instance: non-finite transforms dropped, the distance limit (sphere start beyond it), and the model's bounding sphere × transform against the view frustum. Instances outside the view are not drawn in colour (`instances_culled`), but in a batch that casts they still cast when their sphere meets the shadow map's volume (the frame's shadow `view_proj`): they follow the batch's colour instances in the instance buffer, and the shadow stage draws both (§4.4: casters outside the view still cast into it; the same rule as terrain chunks, §5.2). |
| Lighting | As the lit mesh pipeline (§4.1): ambient and hemispheric, sun with shadow, up to the table lights of the batch, specular, fog, alpha test, the same five alpha modes. Normals: the inverse transpose of the instance matrix (× the sign of its determinant) for scenery. |
| Frill rules (§8.3, §8.5) | Rule 2–3: a group whose bounding box is at least *D* from the camera is skipped. Rule 4: `frill_reduced_count` draws the first *g* × max(1, trunc((1 − *f*) *n* / *g*)) instances. Rule 5: `frill_fade_in(age)`. Rules 1–3 helpers: `frill_radius_sectors`, `frill_distances` (*D*, *F*). §8.5 step 5: the sway weight *w* = clamp((*v*.z − z_min) / *h*, 0, 1) from the vertex block's stored bounding box, `FrillParams::sway_z_range` (z_min, z_max), or the box computed from the model's vertices when it is `None` (equal on all observed data, `mesh.md` M7); no sway when the box is at most 0.01 high; normals rotated, not scaled (the instance matrix's normalised columns). Non-opaque groups (`Render_Frill_IsOpaque` false) draw their opaque and alpha-tested surfaces blended with depth write in the translucent stage. Frill batches cast no shadow by default. `FrillParams::default()` is the engine default: level 2 = 10 sectors, *D* = 97, *F* = 30. Frill instances must carry the terrain colour beneath them (`Instance::new` gives white). |
| **Decision**: distance fade | Frills **scale** toward their origin between *D* − *F* (full size) and *D* (nothing), per vertex from the instance origin's distance, times the fade-in factor (the shader formula is OPEN, §8.3). Scaling needs no blending or sorting of alpha-tested cards. |
| **Decision**: sway | Animated when the intensity exceeds 0.0002: the model-space vertex moves by *a* · (sin(φ + 1.7 *t*), ½ sin(1.3 φ + 1.1 *t*), 0) with *a* = intensity × *h* × *w*², φ the instance's phase and *t* the frame time (`scenery.md` §10 Decision: Tessera's own formula). |
| **Decision**: terrain colour | The albedo blends toward the terrain colour beneath the instance by intensity × (1 − *w*): the base of a clump takes the ground's colour, the tips keep their own (§8.3 rule 7, formula OPEN). |
| CPU cost | `instanced_golden.rs` `thousands_of_instances_per_frame` (release): 40,550 drawn instances (64 frill groups of 625 clumps and 2,000 scenery rocks; 1,450 culled) in 66 draw calls cost 3.3–4.2 ms of CPU per frame on the reference machine (RTX 2070 Super Max-Q, Vulkan; queuing, culling, packing, recording and submission; measured while other builds were running, so an upper bound). |

<a id="s5-6"></a>
### 5.6 Tests

Golden images (each passes on the reference GPU, RTX 2070 Super Max-Q with NVIDIA 615.71
Vulkan, on Mesa lavapipe 24.2 and on llvmpipe GL, Mesa 26.1.8, 2026-10-06; the whole
`tessera-render` suite, earlier golden images included, passes unchanged on the reference GPU
and on lavapipe):

- `crates/render/tests/terrain_layers_golden.rs`, `terrain_layers.png`: a 3 × 3 patch of
  synthetic 33 × 33 blocks with an opaque base layer, a rock layer (masks, masked-diffuse
  opacity, normal map with gloss from its alpha, specular, a rotated and offset transform), a
  sand path (mask rule, alpha test) and a light map on the centre block, under a sun with its
  shadow and fog. Pixel checks: the four opacity rules and the discard test against §7, the
  light map added to Σ, mask and light-map row 0 in the south, the normal-map rotation of
  §6.4, unknown textures and hostile floats, an unlit pass, a foreign chunk; an unresolved
  mask (drawn as no mask), the §8.1 specular and gloss switches without the normal texture
  and without a tangent frame; textures destroyed while a chunk is drawn; scrolling layer
  offsets persisting across frames and lists of the wrong length.
- `water_golden.rs`, `water.png`: a terrain basin with a striped pillar, flooded by a flat
  and a subdivided waving quad with three scrolling normal layers, refraction, Fresnel,
  specular and sparkle. Pixel checks: the refraction composite (water colour, overbright, the
  under-water tint and the shoreline weight at three eye distances) against §8.4; 900 distant
  quads in one draw; hostile quads and parameters; the per-pass quad limit; an sRGB-uploaded
  normal map shading as its linear upload; 4,096 single-quad draws at the largest grid in one
  pass.
- `sky_golden.rs`, `sky_morning.png`, `sky_evening.png`: a vertex-coloured dome, an additive
  sun disc and a blended cloud layer at two times of day (different angles and tints) over
  fogged ground. Pixel checks: the placement rule (*A* to the zenith, *B* to the east),
  opacity 0 and ½, the sky never covering geometry (same pass and earlier pass), hostile
  values, the object limit.
- `instanced_golden.rs`, `frill_sector.png`: a 10 × 10-unit frill sector of 625 alpha-tested
  clumps (sway, terrain colour, distance scale) beside three instanced rocks casting shadows.
  Checks: group culling at *D*, distance reduction, the per-instance distance limit and
  frustum culling, 40,550 instances in 66 draws, hostile instances and limits; a caster
  outside the view darkening the visible ground; an unknown normal map shading as none;
  instances counted once per batch of a two-submesh model; the per-pass instance limit
  dropping instead of failing; a destroyed texture of a model and a sky object (the frame
  succeeds, the surface draws white).
- Unit tests in each module: shader validation, layout sizes, the §6.3 transform and §6.4
  columns (worked example of `terrain-texturing.md` §14), the §7 opacity rules, chunk
  validation, the wave formula and its derivatives (central differences in x and y), the
  per-pass water grid budget, Fresnel, the sky rotation (through the function the hook
  uses, with non-finite angles), the frill rules 1–5 and `FrillParams::default()`, the sway
  bounds from a stored box, frustum culling.

<a id="s5-7"></a>
### 5.7 Open items

> **OPEN:** The water normal frame and the fake light's default direction and specular colour
> (§5.3 Decisions; `water.md` §8.4 OPEN notes).

> **OPEN:** The frill shaders' fade, sway and terrain-colour formulas (§5.5 Decisions;
> `scenery.md` §8.3 OPEN).

> **OPEN:** The sky objects' rotation axes, scale and fog (§5.4 Decision; `environment.md` §12
> OPEN).

> **OPEN:** Terrain layers beyond the full-detail band and the near/far seam
> (`terrain-texturing.md` §12): every chunk is drawn with all its layers at any distance.

> **OPEN:** Water reflection, the depth-texture (soft shoreline), flow and foam permutations
> and the low-detail permutation are not drawn (`water.md` §8.2–§8.4 OPEN). The draw order of
> water relative to other translucent surfaces is Tessera's (§5.1): water is the first
> translucent draw of its pass, before blended instances and the sorted blended meshes, and
> it is not sorted with them.
