<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Renderer integration notes (`crates/render`)

| | |
|---|---|
| **Status** | §1–§2: **M2 design** (meshes, materials and shared buffers), §2.4 **M4 skinning**, implemented. §4: **M4.5 lit pipelines, shared frame uniform and mip chains**, §5: **M4.5 render-world pipelines** (terrain layers, water, sky, instanced scenery and frills), implemented; §4–§5 and §7 **Accepted** (fourth revision of 2026-10-08: body brought in line with the current code, implementer OPENs folded in, implementer choices labelled as Decisions; re-review passed 2026-10-08). §6: **M5 UI pass**, **Accepted** (2026-10-08), then **Revised** (2026-10-09: the §6.10 implementer OPEN on fixed blends in premultiplied passes answered; review 2026-10-09: **changes requested**; second revision 2026-10-09 answering it; review of the second revision 2026-10-09: **passed**, **Accepted**; status and review record in §6); an implementer OPEN on the R12 implementation (§6.17) was added after that acceptance and awaits the reviewer. §8: change log |
| **Last updated** | 2026-10-09 (merge of main into m5-followups: the §6.10 fixed-blend rule R10–R12 and the §6.17 implementer OPEN combined with remediation rounds 2–3 (fourth revision of §4–§5 and §7); §4.3 `Fixed` row points to §6.10 "Fixed blends" for premultiplied passes; see §8. Earlier, 2026-10-09: spec reviewer: §6 review of the second revision passed, §6 Accepted, §6 status paragraph and header rows; spec writer, dirty room, second revision of §6 after its review: §6 status and intro, §6.1 R10–R11, §6.10 "Client portrait path" OPENs (a)–(c) and D3D9 scope, §6.16 external-content OPEN; first revision the same day: §6.10 alpha of fixed blends in premultiplied passes, §6.1 rows R10–R12, §6 status Revised; merge of main into remediation-2: the M5 UI pass (§6) and the change log's move to §8 combined with the fourth revision of §4–§5 and §7; earlier, 2026-10-08: spec writer, dirty room: fourth revision of §4.1–§4.5, §4.8, §5.2–§5.5, §5.7 and §7; merge of the M5 branch: §6 UI pass added, change log moved from §6 to §8; earlier, 2026-10-07: spec writer, dirty room: §3, §4.1, §4.3, §4.4, §4.6, §5.1–§5.5, §5.7 revised; new §7 client facts and provenance; second revision of §4.3, §5.5 and §7 after the re-review; third, wording-only revision of §4.4, §5.3 and §7 after the acceptance) |
| **Author role** | §1–§5 and the change log (§8, formerly §6): Clean-room implementer of `tessera-render`; §4–§5 revised 2026-10-07 and 2026-10-08 by a spec writer (dirty room), who also wrote §7. §6 (UI pass, M5): Spec writer (dirty room), 2026-10-06, Tessera design from decisions 0014 and 0015; §6.10 revision, rows R10–R12 and the second revision of §6: spec writer (dirty room), 2026-10-09; §6.17 implementation notes: clean-room implementer |
| **Reviewer** | §4–§5, §7: Claude review sessions (spec-writer agent, M4.5 criterion 9), independent of the writer. §6: Claude review session (spec-writer agent, fresh session, not the writer) |
| **Review record** | §4–§5: 2026-10-07 first review and re-review: changes requested; 2026-10-07 re-review of the spec writer's revision (§4–§5, §7): **changes requested**; 2026-10-07 re-review of the second revision (Claude review session, spec-writer agent): **passed, Accepted**; non-blocking notes applied in a third revision (Revised); 2026-10-07 re-review of the third revision (Claude review session, spec-writer agent): **passed, Accepted**; 2026-10-07 re-review of the text with the implementer's conformance OPENs (Claude review session, spec-writer agent): **changes requested**; 2026-10-08 re-review of the fourth revision (Claude review session, spec-writer agent): **passed, Accepted**. §6: 2026-10-08: **passed**, Accepted (record in §6); revision of 2026-10-09 (§6.10, §6.1 R10–R12): reviewed 2026-10-09 (Claude review session, spec-writer agent, not the writer): **changes requested** (three blocking items, record in §6); second revision 2026-10-09 answering them: reviewed 2026-10-09 (Claude review session, spec-writer agent, fresh session, not the writer): **passed, Accepted** (two non-blocking notes, record in §6); the §6.17 implementer OPEN added afterwards is not yet reviewed. 2026-10-09: merges of main into remediation-2 and into m5-followups, no fact changed; §6, its revisions and the fourth revision of §4–§5 and §7 were each reviewed apart. Details in the dirty-room review log (decision 0018). |

The clean-room implementer of `tessera-render` wrote this file. It holds Tessera's own design
decisions and the open questions that need spec facts. The client facts that §4–§5 rely on
come from the specs they cite; §7, written by a spec writer in the dirty room, restates them
with their provenance and lists every place where the renderer departs from the client. §6,
the M5 UI pass, is Tessera design from decisions 0014 and 0015, written by a spec writer. Spec
writers may replace or extend any section.

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

> **Answered (M4.5, §5.5):** per-draw cost and instancing: repeated static objects and
> frills draw as instance batches (one `draw_indexed` per batch and submesh). The original
> question: each `MeshDraw` takes its own 256-byte dynamic-offset uniform and one
> `draw_indexed` per submesh and face side, with no instancing; ARCH §13 calls for
> instancing of repeated static objects.

> **OPEN:** Whether the renderer should accept BC1/BC2/BC3 (DXT1/3/5, records.md §5) blocks
> directly when the adapter supports `TEXTURE_COMPRESSION_BC`, instead of the formats crate decoding
> them to RGBA8. This affects memory: 4096² surfaces exist. The plan is to add a compressed path with
> RGBA8 as the fallback. The renderer needs to know whether any surface needs CPU-side processing
> before upload, such as the BC1 1-bit alpha question in records.md §5.3.

> **Answered (M4.5, §4.6, `texture-list.md` §4.2–§5.5):** mip chains: one mip per stored
> level, level 0 the largest, a level whose stored size differs from its slot copied clipped.
> The original question: is level 0 always the largest, and does each level halve exactly?

> **OPEN:** The colour space of each surface format and category: which are sRGB-encoded colour
> data and which are linear data (normal maps, masks). This decides `ColorSpace::Srgb` vs
> `ColorSpace::Linear` per surface category (records.md §5.4).

> **OPEN:** Default third-person camera parameters (distance limits, pitch limits, pivot height,
> FOV) for a familiar feel. These must come from user settings or from public, observable
> behaviour described in a spec. They must never be read out of the client binary.

> **OPEN:** Triangle winding of mesh records (which side is the front). The renderer takes
> counter-clockwise front faces; the mesh spec must state the source convention so the decoder
> knows whether to call `flip_winding`.

> **Answered for lit passes (M4.5, §4.1, `lighting.md` §5.2.4):** three channels, no Z
> reconstruction, positive green tilts the normal toward the top of the image
> (*N* = *n*.x **T** − *n*.y **B** + *n*.z **N₀**). The M2 pipelines keep their own
> convention (`normal_green_up`). The original question: green-channel convention of game
> normal maps and whether they use two channels with Z reconstructed.

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
| Light passes | **One base pass loops over all lights** (`lighting.md` §2.1 allows it): ambient + hemispheric, the sun with its shadow, and table lights, all per pixel: at most 8 lights per draw **in all, light 0 included** (`lighting.md` §8.5). Light 0 is the main light, or, in a pass without one, the default directional light that stands in for it (the Tessera Decision of `lighting.md` §8.5: it holds the light-0 slot and counts toward the 8), so a draw gets at most 7 table lights; only when a pass without a main light gives a draw a directional table light with `KeyLight` (a per-pixel candidate, `lighting.md` §8.5 step 2) does that light take the light-0 slot in place of the default light, and the draw gets 8 table lights (§4.3). The additive light passes 0x01–0x03 are folded in; since the base pass is lerp(Σ, fog, *f*) and each additive pass adds (…) × (1 − *f*), the sum is the same. Deviation: on translucent texels the folded light is weighted by the base pass's output alpha (§4.3), where the original's additive passes use *M*.alpha; the two differ only for base-texture opacity. |
| **Decision**: per-pixel lists | Every table light uses the newer back end's per-pixel form: smooth fade over the last quarter of *R²* (`lighting.md` §8.3 step 3) and the clamped specular of `lighting.md` §5.8. This departs from the original, which sums the lights of the per-vertex lists (every light but light 0) per vertex with the hard cut-off of `lighting.md` §8.3 steps 1–2 and no specular (`lighting.md` §5.6). Extra directional lights (`lighting.md` §8.2) get diffuse and specular too (per-vertex in the original). |
| Colour space | Lit shaders compute on **stored values** (§5.1: no back end decodes sRGB). A material texture uploaded with `ColorSpace::Srgb` is re-encoded after sampling (per-texture flag in the draw uniform), so both upload kinds give the stored value (filtering then happens on decoded values, a rounding-level difference). Material, light, fog and vertex colours are taken as given. On an sRGB target the final colour is decoded before output so the stored pixel equals the computed value; blending on sRGB targets therefore happens on decoded values (Decision). |
| Unlit pass | `Material::unlit` in a lit pass: texture × *M* (× vertex colour when `LitSurface::vertex_colors`), fogged only when `LitSurface::fog` is set (`lighting.md` §7 code 0x00: an unlit program is fogged when it reads the fog inputs; the caller supplies that switch). A lit surface is always fogged (`lighting.md` §2.1: the base pass always contains distance fog), whatever `LitSurface::fog` says. |
| Emissive and glow | *Em*.rgb × *G* added before fog (§5.9). Glow mask (slot 3013) × *G* is added the same way (**Decision**: the bloom composition is OPEN in §5.9). |
| Two-sided | Back faces keep the front normal (`material.md` §11.4: the programs read no front-face input). The M2 pipelines keep their flip. |
| Vertex colours | Multiply *M* (RGB and alpha) only when `LitSurface::vertex_colors` (§5.13); the alpha test uses the texture opacity before that. |
| Specular | Black (no term) when `Specular::shininess` < 0.5 (§5.8); power capped at 4096. `Specular::texture` is not used by lit passes; gloss comes from `LitSurface::gloss`. |
| Dye | The renderer's mask convention (§2.3, 1 = dyed); callers convert the data's polarity (`material.md` §12.3: low alpha = dyed). A dye masked by the base alpha (`DyeMask::BaseAlpha`) makes that alpha a dye mask only: `Opacity::BaseAlpha` then acts as `Opacity::One` in the alpha test, the output alpha and the shadow pass (`material.md` §11.2: testing the dye mask would punch holes in dyeable cloth). Second dye, second colour layer, environment maps and texture transforms are not part of the lit pipeline yet (not in the M4.5 deliverables). |
| Normal maps | *N* = *n*.x · **T** − *n*.y · **B** + *n*.z · **N₀** with **B** = +V (`lighting.md` §5.2.4): positive green tilts the normal toward the top of the image. Lit passes always use this sign; `Material::normal_green_up` serves only the M2 pipelines. This answers the green-channel OPEN of §3 for lit passes (three channels, no Z reconstruction). Mirrored transforms flip the bitangent sign so the mapping follows the mirrored texture. |
| **Decision** (departs from `lighting.md` §5.2.4): tangent frame | **T** is the stored tangent after the transform, normalised but not made orthogonal to the normal (as `lighting.md` §5.2.4 uses it). `MeshData` has no binormal stream (§2.1), so **B** is built per pixel as *w* × cross(**N₀**, **T**), normalised, from the interpolated normal and tangent and the tangent's sign *w*. The client uses the **stored** binormal, which mesh records carry (`mesh.md` §4.5, §5.6: the +V direction, not orthogonalised against the normal, with a handedness that varies within a mesh), so the stored binormal and the cross product differ wherever the stored frame is not orthogonal; the normal map's tilt along **B** then differs (§7 departure R-D10). |

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
| 192 | `fog` | near, 1 / (far − near), far, far − near, after the **lower** limits of `lighting.md` §10 (`Fog::lower_limits`: far at least 20, a non-finite far 10,000; near within −10,000 … far − 20, a non-finite near −10,000; the far is **not** capped). The 10,000 cap that the client applies to the keyframe's fog end **before** an interior override (§7 row R-F1) is the caller's: `Fog::keyframe_capped` (constant `FOG_FAR_CAP`) caps a keyframe's end, and `Fog::limited` (the cap, then the lower limits) serves a fog without an override. An overriding end above 10,000 is therefore drawn as given (`walker.md` §14.3 "Fog limits") |
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
| `Additive` (new) | `src·a + dst`, no depth write, sorted; *a* is the output alpha below (opacity × *M*.alpha, so a caller can fade the surface by *M*.alpha); fog scales it by (1 − *f*) (**Decision**: `lighting.md` §6 and §10 state the (1 − *f*) rule for the additive light passes and §7 does not say how unlit SRCALPHA / ONE effects are fogged; lerping an additive effect toward the fog colour would brighten fogged distance). A shorthand for the blend of `Fixed` below with SRCALPHA / ONE, ADD and no depth write; it differs from that `Fixed` only in its *a* when *M*.alpha < 1. An additive record whose render state writes depth (`material.md` §7.6) uses `Fixed` with depth write | as `Blend` |
| `Fixed(FixedBlend)` (new) | the fixed-function blend of an unlit record (`material.md` §11.3, rule for unlit records): colour = op(source × *s*, destination × *d*) with the factors ZERO, ONE, SRCCOLOR, INVSRCCOLOR, SRCALPHA, INVSRCALPHA, DESTALPHA, INVDESTALPHA, DESTCOLOR, INVDESTCOLOR (the caller maps the D3D codes of `material.md` §7.6) and the operations ADD, SUBTRACT, REVSUBTRACT, MIN, MAX (MIN and MAX ignore the factors, as the D3D operations do), depth write from the record's render-state block. The alpha that SRCALPHA and INVSRCALPHA read is the opacity source of `material.md` §11.2 alone, or 1 without one (the §11.3 rule says so); *M*.alpha (the instance tint included) and the vertex alpha do not enter (Decision; whether they enter in the client is OPEN in `material.md`). **Decisions** (the specs are silent): drawn with the blended draws, sorted back to front; no shadow; the target's alpha channel is kept (alpha factors ZERO / ONE; in a pass with `Pass::premultiplied_alpha` the alpha component follows §6.10 "Fixed blends", R12); fog as in the fog row below. **Departs** from `material.md` §11.2: no alpha test (§7 departure R-D11) | as `Blend` |

**Decision (fog of `Fixed`, extending the `Additive` Decision; the client's rule is
OPEN):** fog fades a fixed blend's contribution toward what leaves the destination
unchanged: with destination factor ONE and ADD or REVSUBTRACT, and with MAX, the colour is
scaled by (1 − *f*); with ZERO / SRCCOLOR or DESTCOLOR / ZERO and ADD, and with MIN, the
colour is lerped toward white by *f*; every other blend lerps toward the fog colour.

> **OPEN (spec writers of `lighting.md` and `material.md`):** how the client fogs an unlit
> effect of each blend pair (`lighting.md` §7 code 0x00 says only whether the program reads
> the fog inputs), and whether the vertex alpha (`lighting.md` §5.13) and the per-object
> tint alpha (`material.md` §8.3) enter the opacity a fixed-function factor reads.

Output alpha (`lighting.md` §5.12, `material.md` §11.3) = opacity × *M*.alpha when the opacity
source is the base texture (`Opacity::BaseAlpha`), else *M*.alpha; × vertex alpha where
`vertex_colors` is set; for `Fixed` the opacity source alone (above). The opacity of a
separate texture (`Opacity::Texture`, slot 3020) or of the emissive alpha
(`Opacity::EmissiveAlpha`, slot 3018) drives only the alpha test (and the shadow pass's
cut-outs). `LitSurface`: glow colour *G*, emissive texture, glow mask, gloss source (`One`,
`BaseAlpha`, `NormalAlpha`, `Texture`), opacity source (`BaseAlpha` default, `One`,
`EmissiveAlpha`, `Texture`), `vertex_colors`, `fog` (unlit surfaces only, §4.1),
`cast_shadow`, `lights` (`Auto`, `None`, `List`).

**Light selection (`lighting.md` §8.5, rows L30–L32, L34):** `LightSelection::Auto` takes the
point lights whose squared distance to the draw's world box is below ρ² (strict), where ρ is
the light's selection radius; it leaves out lights flagged as baked into the landscape on
terrain and ceiling chunks (§5.2); it applies the parent switches as the client does: for
the light's parent the never-parent switch, for **every other** draw the only-parent switch,
whether or not the light has a parent, so an unattached light with
`Light_OnlyInfluenceParent` reaches nothing, and a draw without an object handle is never a
parent (§7 row R-L2); it orders key lights first, then the others, each group nearest first
by the squared box distance (the Tessera Decision of `lighting.md` §8.5; ties by table
index), at most 7 point lights; then the directional table lights by the "Directional table
lights" row below. The reach of each table light comes from `Pass::light_reach` (a
`LightReach` per `FrameLighting::lights` entry: ρ, `KeyLight`, `Light_BakedLandscapeLight`,
the parent's object handle and the two parent switches; `LightReach::from_fields` computes
ρ = Range × (1 − 0.15 × min(Falloff, 1)), or Range for `Light_GlobalLight`). A draw's object
handle is `MeshDraw::light_object` or, for an instance, `Instance::light_object` (default
none); terrain chunks have none. **Decision:** a light without an entry has ρ = *R* (its
record's range), is not a key light, reaches terrain and ceiling chunks and has no parent
switch (the client always has the light's fields; an entry-less light comes from a caller
that supplies none). The world crate can pass its own choice with `LightSelection::List`:
**Decision**, the list is kept as given, in its order and with baked lights included (the
caller owns the rule); indices past the table are dropped, and the cap is 7, or 8 when the
list holds a key directional light that takes light 0 (below). The renderer builds the
selection data once per pass (directional indices; point lights sorted along x, so a draw
tests only the lights whose x range can reach it) and selects without allocating. The client
considers only objects drawn in the current frame and skips objects that carry any of three
scene flags (§7 row R-L3); the renderer selects only for the draws it is given.

| Topic | Choice |
|---|---|
| **Decision**: world box (`lighting.md` §8.5 OPEN (b), how an object's box is formed) | A mesh draw's box is the box around the eight corners of the mesh's model-space box (bind pose for skinned meshes) after the draw's transform; an instance's is the box around the eight corners of the model's box after the instance's transform; a terrain chunk's is the box of its vertices after its transform. A box that is not finite receives no point light. |
| Per-instance lists (`lighting.md` §8.5, row L32; §7 row R-L6) | With `Auto`, each instance drawn in colour gets its **own** selection against its own box (above), with the parent switches by its `Instance::light_object`, as the client builds one list per object. The lists travel in the instance data (§5.5). `None` and `List` selections, and `Auto` in a pass without usable point lights, cannot differ between instances and are made once per batch. Instances drawn only into the shadow map carry no list. |
| **Decision**: directional table lights (`lighting.md` §8.5 OPEN (c), §8.2, §5.6) | Directional table lights have no position; they reach every draw and follow the point lights. In a pass **without** a main light, the first directional table light in table order whose `LightReach::key_light` is set takes the light-0 slot in place of the default directional light and does not count toward the 7 (a per-pixel candidate, `lighting.md` §8.5 step 2 and the Decision there). After the point lights, **at most one** further directional table light, the first in table order other than that light 0, fills the single extra directional slot of `lighting.md` §5.6 and §8.2 ("count 0 or 1"), when fewer than 7 point lights were selected. Which directional light the client keeps when an object's list holds more than one was not traced; no placed light of the data is directional (`lighting.md` §8.3: a light record is a point light unless its mode says directional, which is the main light's, and the direction has no property), so callers that build the table from placed lights never supply one. |
| **Decision**: malformed light fields (§7 row R-E3) | `reference::light_record` and `reference::selection_radius` (hence `LightReach::from_fields`) sanitise by one rule: a non-finite field takes the default of a new light (`lighting.md` §8.3: Range 25, Falloff 1.0, intensity 0.5, *σ* 1, Diffuse and Specular white; a colour with any non-finite channel is replaced whole) and a negative Falloff counts as 0, so ρ ≤ *R* always and a light whose record contributes nothing (Range ≤ 0) is never selected. Input of this kind has no client counterpart in the data the specs describe. |

> **OPEN (implementer, 2026-10-08, remediation pass):** `lighting.md` §8.5 (seventh
> revision, L39) says Decision (c) above departs from the client and asks implementers to
> give a directional light its world position and ρ. The renderer now does so when the
> caller supplies them: `LightReach::position` (with `LightReach::radius`) makes a
> directional table light a walked light, tested like a point light (box distance below
> ρ², parent and baked switches) and ranked by step 4 (key first, then nearest, then table
> index) for the light-0 slot (no main light) and the single extra directional slot; a
> directional light without a position keeps the Decision (reaches every draw, table
> order, after the walked ones). No caller supplies a position (no placed light is
> directional). Please fold this into the row above. The `Fixed` alpha test of §4.8
> (R-D11) is implemented: `FixedBlend::alpha_test` (an `AlphaThreshold`, the record's
> 1043 when it tests an opacity source) discards fragments with opacity at most the
> threshold in the lit, instanced and sky stages; the walker passes it for unlit records
> (`walker.md` §14.5). The binormal stream of R-D10 is not done (it changes `MeshData`
> for every caller, in crates outside this pass).

Draw uniform: the M2 `DrawUniform` (224 bytes, §2) grows by two vectors to 256 bytes, still one
256-byte dynamic-offset slot: `lit` (offset 224: rgb *G*, w specular power *p*) and
`lit_lights` (offset 240: x, y = eight light indices as bytes, z = count, w = flags: vertex
colours 1, fog 2, emissive 4, glow 8, specular 16, gloss source bits 5–6, opacity source
bits 7–8, sRGB re-encode bits 9–15 per material texture, bit 16 "output alpha is the opacity
source alone" for `Fixed`). Group 2 holds seven textures (base, normal, dye mask, gloss,
emissive, glow, opacity; samplers at binding + 7); an unset or unresolved texture binds the
white texture.

<a id="s4-4"></a>
### 4.4 Sun shadow (`lighting.md` §9; within the Tessera Decision "simple form" there, using its listed variants)

The equations are those of `lighting.md` §9 (steps 2, 4–7). The engine values are OPEN there
and its Tessera Decision (simple form) is one cascade, one tap, shadow colour 0, colour range
1 and *φ* fading linearly to 0 at the shadow distance; that Decision lists as variants that
stay within it the five-tap filter, a fade that starts later than the camera, a single map
fitted around a point ahead of the camera, a depth bias, points outside the map counted as
lit, and no sun shadow while the sun is low or indoors. **Decision:** the renderer keeps one
cascade, colour 0 and range 1 and uses these variants: five taps (quality 2), a linear fade
over the last fifth of the shadow distance, one map fitted around a sphere chosen by the
caller, points outside the map lit; the depth bias is 0.006 × the fitted sphere's radius
when `SunShadow::fit` builds the shadow, else the field default 0.0015. The radius,
distance, map size and bias are Tessera's values, not the original's; the minimum sun
elevation is the caller's (`walker.md` §14.3).

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
`Additive`, `Fixed`) back to front by bounding-sphere centre (§2.3). Group 0
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
mip-chain OPEN of §3 (`texture-list.md` §4.2–§5.5). The walker uploads the chains of its
object and terrain textures (`walker.md` §14.1).

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

> **OPEN (spec writer, 2026-10-08, for the `tessera-render` and `tessera-mesh`
> implementers):** give `MeshData` a binormal stream, fill it from the mesh records' stored
> binormal (`mesh.md` §4.5, §5.6), and use it as **B** in the lit and instanced pipelines
> (`lighting.md` §5.2.4), keeping *w* × cross(**N₀**, **T**) only for meshes without one
> (§4.1 tangent-frame Decision, departure R-D10).

> **OPEN (implementer, `tessera-render` / `tessera-mesh`, 2026-10-08, remediation pass):**
> the binormal stream above is implemented; please fold it into §2.1, the §4.1
> tangent-frame Decision, §5.5 "Lighting" and §7 row R-D10. `MeshData::binormals`
> (`Option<Vec<[f32; 3]>>`, one per vertex, accepted only together with `tangents`:
> binormals without tangents or of the wrong length are `InvalidMesh`; non-finite or zero
> entries pack as zero; `compute_tangents` drops them; `MeshInfo::has_binormals`). The GPU
> vertex grows from 40 to 48 bytes (binormal snorm16×4 at offset 40, w = 0, shader
> location 13 in the lit and instanced pipelines). The lit and instanced pipelines use the
> stored binormal, transformed like the tangent (model matrix, skinned like the tangent,
> rotated for frills; a mirroring transform mirrors it, so no sign flip), normalised per
> pixel, as **B**; where it is absent or its interpolated length is below 10⁻³ of the
> tangent's they keep *w* × cross(**N₀**, **T**) (`frame_binormal` in
> `frame_shading.wgsl`). The M2 pipelines keep the derived bitangent. `tessera-mesh`
> `LevelGeometry::binormals` carries the stored stream as stored (zero for parts without
> a frame); the explorer passes it (and deforms it like the tangents); the M2/M4 golden
> images are unchanged. Engineering choices taken (Decisions, please confirm): the
> rejection of binormals without tangents, the 10⁻³ fallback threshold, the per-pixel
> fallback for degenerate stored binormals (`mesh.md` §5.6: 0.12 % are zero or short).
> **Not done, outside this pass:** the walker's `level_mesh_data` passes no tangent frame
> at all (`tangents: None`), so walker objects and characters are drawn without normal
> maps (the §5.5 normal-map switch needs tangents); passing `LevelGeometry::tangents` and
> `binormals` there is the walker implementer's change (`walker.md` §14.5 lists the normal
> map 3011).

> **OPEN (spec writer, 2026-10-08, for the `tessera-render` implementer):** apply the alpha
> test of `material.md` §11.2 (threshold 1043 when 4004 is on and an opacity source exists)
> to `Fixed` surfaces too (departure R-D11).

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
| Stages (**Decision**) | `Shadow`: terrain chunks and opaque or alpha-tested instanced surfaces of batches that cast. `Opaque` (after the opaque and alpha-tested meshes): terrain layers, opaque and alpha-tested instanced surfaces, then the sky. Then the scene copy (§5.3). `Translucent` (after overlays, before the sorted blended meshes): **blended instanced surfaces, then water**, in the hook order of the row above. Hook draws are not sorted with blended meshes. Consequences: the water's refraction (the scene copy) does not contain blended instances; and water, which writes with source ONE, destination ZERO under the depth test (§5.3), replaces a blended instanced surface that lies in front of the water but wrote no depth (`AlphaMode::Blend`); one that wrote depth (`TestBlend`, the non-opaque frill groups of §5.5) is kept. The original's order of water among translucent surfaces is not specified. |
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
| Bindings | Group 1: draw uniform. Group 2, one retained bind group per layer: layer uniform, diffuse, normal, emissive, glow, mask, light map, a **wrap** trilinear sampler for the layer textures (§6.3) and a **clamp** bilinear sampler for the mask and light map (§5.2, §10): linear within a mip level, the nearest mip level (**Decision**). Masks and light maps are single 0x41 render surfaces with one image level (`terrain-texturing.md` §5.1, §10; `records.md` §5), so with data from the install the sampler only ever reads level 0 and the mip setting has no effect. Missing textures bind white with their switch off. |
| Missing mask | A later layer whose mask does not resolve (unknown, destroyed, a render target, or rejected upstream as non-A8) is drawn as if it had no mask (§5.1 Tessera Decision): `MaskedDiffuse` → `Diffuse` (α = *a*), `Mask` → `One` (α = 1), logged. |
| Switches (§8.1) | Normal map: the normal texture resolved; it perturbs the normal only with a tangent frame (a zero frame leaves *N*, §6.4). Gloss from the normal: requested, and the normal texture resolved (the frame plays no part). Specular: requested with power ≥ 0.5, **and** gloss comes from the normal or the opacity switch is off (the opacity rule does not read the diffuse alpha: `Opaque`, `Mask`, `One`); otherwise no highlight. Opacity, specular and gloss switches are derived after the missing-mask rule above. |
| Mask channel | The mask value is the texture's **alpha** (an A8 surface uploaded as `tessera-formats` decodes it, (255, 255, 255, a), stored row 0 first, sampled at set 3 so row 0 is the south edge, §5.2). A missing mask reads 1. (Unlike the M2 dye masks of §2.3, which read red.) |
| Draw order | Chunk by chunk, layers in stored order, one `draw_indexed` per layer, one pipeline for all layers. |
| Blend and depth | Source α, destination 1 − α ("over", §8 step 4) on every layer; the first drawn layer outputs α = 1. Depth test `GreaterEqual` under reversed depth, i.e. "less or equal" (§8 step 5 Tessera Decision), depth write on for every layer; the clip position is `@invariant` so every layer of a triangle produces the same depth. Back faces culled (terrain triangles are counter-clockwise from above, ceiling triangles from below). |
| Opacity and discard | §7 per pixel; discarded when α ≤ τ except on the first drawn layer. |
| Colour (§9) | Σ = light map RGB (baked-light switch) + `frame_ambient(N, M)` + sat(*N* · *L₀*) *S* *E₀* *M* + the table lights' diffuse × *M*; colour = 2 *C* Σ + *g* sat(*N* · *H*)^*p* *S* × sun specular × *Ms* + *Em* × *G* (emissive) + glow × *G* × *M* (glow), then the per-vertex fog. The highlight is not multiplied by *N* · *L*, the albedo or 2 (§9.4 as revised), so a gloss from a diffuse alpha of about 0.09 (row L5) gives a sheen of about 0.09, not a broad highlight. |
| Normal map | *n*′ = the texel turned by *c*₀, *c*₁ (§6.4), normalised, used in the frame (**T**, −**B**, **N**): *N* = *n*′.x **T** − *n*′.y **B** + *n*′.z **N**. A zero frame leaves *N* = **N**. |
| **Decision**: table lights | Per pixel with the smooth fade (as §4.1; the original sums them per vertex), diffuse only: §9.4 gives a specular term for light 0 alone. Selected once per chunk by its world box (`LightSelection`, §4.3); with `Auto`, lights whose `LightReach::baked_landscape` is set never reach a chunk, whether or not it has a light map (`lighting.md` §8.3, L30; §7 row R-L4). |
| Baked-landscape lights on the ground (`lighting.md` §8.6, Tessera Decision there) | The baked-lighting texture is the stored light map as given; the renderer has no light-map bake. The client's bake (pass code 0x41 into the chunk's baked-lighting texture, §7 row R-L5) runs only in a special client mode, Inferred not in normal play (`lighting.md` L36). |
| Stored normal (`terrain-texturing.md` §4.10) | Layers without a normal map use the interpolated vertex normal **as stored, not normalised**, for the ambient (hemispheric) term and the sun's *N* · *L₀*; the table lights and the highlight use the unit normal. A ceiling chunk's downward normals therefore receive *A₀* − *H₀* and no sun from above. The chunk's normal matrix (inverse transpose) is scaled to determinant ±1 so a uniform scale keeps the stored length. Layers with a normal map use the mapped unit normal everywhere. The row reproduces the client only when the caller supplies the normals of `terrain-texturing.md` §4.10 (averages of unit triangle normals, shorter than 1 at creases); the renderer takes the normals it is given. The walker currently supplies unit normals (`walker.md` §14.4 Decision and OPEN), so with its chunks the length effect does not occur. |
| **Decision**: glow | Added like emissive (the bloom target is OPEN, `lighting.md` §5.9), with the §9.5 glow-pass colour glow × *G* × *M*. |
| **Decision**: baked shadow | White (the landscape-shadow feature is off, §9.5 Tessera Decision): *S* is the shadow factor of `lighting.md` §9. |
| Layer offsets | `TerrainDraw::layer_offsets` replaces the translations of the layers (scrolling `u_offset`/`v_offset` curves evaluated by the caller at the material clock, §6.3): one entry per layer in stored order (a list of another length applies to the layers it covers, logged once per pass). When the frame is built, only the layers whose rows changed are written, with one buffer write per chunk covering them; the last value written in a frame applies to all its passes. |
| Limits | One 128-byte (aligned) draw slot per queued `TerrainDraw`, checked against the device's buffer limit before anything is built (`LimitExceeded`, about two million draws at 128-byte alignment). |
| Shadows | Chunks with `cast_shadow` draw every covered triangle **once** into the sun's shadow map, no culling, the lit meshes' slope bias: when a triangle repeats across the layers' lists (the same three vertices in any order), the chunk's distinct triangles are appended after the layers' lists at upload and the shadow draw uses that range; otherwise it uses the layers' lists (**Decision**; the depth image is the same as drawing every layer's triangles). |
| Culling | A chunk whose bounding sphere (× transform) is outside the frustum is not drawn in colour (`terrain_chunks_culled`) but still casts. |

<a id="s5-3"></a>
### 5.3 Water (`water.md` §8–§9)

| Topic | Choice |
|---|---|
| Inputs | `WaterDraw`: quads (`WaterQuad { origin, size, height }`; `WaterQuad::landblock` makes the 160 × 160 quad of a wet landblock, §8.1 Tessera rule), the surface's normal map, `WaterParams` (the `c_Water…` parameters of §8.2 with the 0x2B0000A8 shader defaults; `normal_layers` 1–3 for records that lack layers 1 and 2, §8.2 OPEN), grid cells per quad side (1–64) and the block offset *O* (default (0, 0), §8.3 Tessera rule). |
| Geometry | No retained mesh: one instance per quad (origin, size, height; 32 bytes) and `6 n²` generated vertices per instance (`n` cells per side, two counter-clockwise triangles per cell), so any number of quads of one surface is one draw. Hostile quads (non-finite, size ≤ 0) are dropped; at most 65,536 quads per pass (`LimitExceeded`). **Per-pass vertex budget:** all draws of a pass together generate at most 2²⁴ vertices: in queue order each draw's grid is halved until it fits in what the earlier draws left, minus one cell per quad for every later draw (one cell per quad always fits). |
| GPU layout | Per pass: one buffer of all quads, one uniform buffer with a 176-byte slot per draw at a dynamic offset, one bind group per distinct normal map (with the scene copy). One `draw` per `WaterDraw`. 4,096 single-quad draws at the largest grid cost 3.2 ms of `end_frame` CPU time on the reference machine (release, `water_golden.rs` `one_draw_per_quad_stays_bounded`, other builds running). |
| Distant water (§9) | The caller derives the wet landblocks of distant 3 × 3 sectors from the block maps (§9 Tessera rule) and draws them as more landblock quads (with one cell each); the pipeline treats near and distant quads alike. The sector texture coordinates of §8.1 feed only the low-detail permutations, which Tessera does not draw (the water permutations read only the position, §8.3). |
| Vertex stage | `water.md` §8.3: waves Σ *A*ₖ sin *s*ₖ with *s*ₖ reduced to [−π, π); the surface tilted by the wave derivatives; normal-layer coordinates *B* · Scaleₖ + Speedₖ · *T* with *B* = (*P*.xy − *O*) / 160; per-vertex fog. *T* is the frame's time input (`FrameLighting::time`, E20). **Decision (departs from `water.md` §8.3):** the waves take the x, y of the vertex in the pass's world frame (the quads are generated in that frame), where the client uses the vertex's model x, y. With a caller whose world frame is relative to a moving anchor (the walker, `walker.md` §2), the wave phase therefore jumps when the anchor moves; the normal layers do not, because the caller passes the anchor's origin as *O* (`walker.md` §14.6). |
| Scene copy | Water refracts the colour target as it is before the pass's translucent stage: `end_frame` ends the pass's render pass after the overlays, copies the target into a renderer-owned texture of the same size and format (`scene_copies` counts it), and continues in a new render pass that loads colour and depth. Surfaces are configured with `COPY_SRC` where the surface offers it; a target that cannot be copied draws the refraction as the water fog colour (logged once). An sRGB copy is re-encoded after sampling so the shader sees stored values. |
| Pixel stage | §8.4 steps 1–7: the three normal-map samples (re-encoded to the stored texels when the map was uploaded `ColorSpace::Srgb`, §5.1) mapped to 2(rgb − 0.5), summed and normalised; screen coordinates; refraction *F* of the scene copy offset by the world normal's x, y × `RefractionBumpDistortionAmount`, the under-water tint *U*, the shoreline blend with *w* = clamp((distance − 5) × 0.1, 0, 1); reflection = the unmodified *F* (§8.4 Tessera rule: no reflection texture); Fresnel; specular sat(*n* · normalize(*v* − *L*))^power + the sparkle term; colour × `WaterColor` × overbright + *s* × fake specular × `SpecularColor`; fog. **Decision** (malformed values; `water.md` gives no range): `FresnelExponent` and `SpecularPower` are clamped to 0–4,096 (a non-finite value takes the shader default) and both powers use clamp(*x*, 0, 1)^*e* with *x*⁰ = 1 for every *x*, so no value from data produces an infinity or NaN; the record defaults are 2.0 and 80 (`water.md` §8.2). The sparkle term is added only when the normal map resolved (the "Missing normal map" row); `FakeSpecularThreshold` is not clamped. |
| Render state | Source ONE, destination ZERO, alpha 1, depth test and write, both faces (§8.4 Tessera rule). |
| **Decision**: normal frame | The tangent-space normal is turned into world space with **T** = +x, **B** = +y (tilted by the waves) and the sign convention of `lighting.md` §5.2.4: *N* = *n*.x **T** − *n*.y **B** + *n*.z **N**. (`water.md` §8.4 step 1 says "the surface frame" without the signs; OPEN there.) |
| **Decision**: fake light | *L* = the main light's travel direction (−`sun_dir`) and its specular colour when the pass has a main light; otherwise straight down with a black specular colour (the engine's default direction and specular colour are not given, `lighting.md` E18; OPEN in `water.md`). Whether the pass has a main light is the caller's choice (`FrameLighting::sun`): the client has none while the sun-present flag of `environment.md` §11.7 step 9 is false (blended keyframe `effect` not null, sun colour opaque black, or sun brightness below 0.0002, and always indoors). The walker passes no main light indoors or for a black or dim sun, but makes no test on the blended `effect` (a departure labelled in `walker.md` §14.3); the renderer needs no change for it. |
| Missing normal map | A flat tangent-space normal (0, 0, 1): no ripples, no sparkle. |

> **OPEN:** (implementer, 2026-10-09; engineering choice) `WaterDraw` gained an optional
> far-draw fade (`fade`: start *s*, inverse range *r*; default none) for the sector water of
> the far view (`far-view.md` §8 OPEN): after fog, the colour is blended toward the
> undistorted scene copy by *a* = 1 − sat((*d* − *s*) × *r*), *d* the distance to the eye,
> and the fragment is discarded where *a* < 0.001 (without a scene copy: where *a* < 0.5).
> The uniform slot grew from 176 to 192 bytes. The owner of this spec should fold this into
> the table above.

<a id="s5-4"></a>
### 5.4 Sky (`environment.md` §11.4, §12)

| Topic | Choice |
|---|---|
| Inputs | `SkyDraw { objects }`: each `SkyObject` is a `WorldModel` (the geometry of the object's 0x47 record, uploaded by the caller with its surfaces), angles *A* and *B* in degrees, the keyframe tint (alpha = opacity) and a uniform scale. Visibility, angles and tints come from the caller's environment blending (§11.3–§11.4). At most 4,096 objects per pass. |
| Placement (`environment.md` §11.4 step 5; §7 row R-S1) | A model vertex *v* is drawn at eye + R_Y(−*A*) · R_Z(−*B*) · (scale · *v*): first a turn about +Z (up) by −*B*, then about +Y (north) by −*A*, as the client does. *B* is wrapped into [0, 360) first (the rotation is the same; the wrap keeps large angles precise), *A* is not; a non-finite angle counts as 0. With *A* = *B* = 0 the model keeps its axes (x east, y north, z up). `SkyObject::rotation` returns the matrix; the `environment.md` §14 worked example is a unit test. |
| Depth | Drawn at the far plane (clip z = 0 under reversed depth) with `GreaterEqual` and no depth write, so the sky covers only pixels nothing else has drawn, whether it is queued in the same pass as the world (it draws after the opaque geometry, saving fill) or in an earlier pass. |
| Shading (**Decision** of this file) | Unlit and unfogged: base texture × *M* × tint (× vertex colour when the surface reads it); output alpha = (base alpha unless the surface is opaque) × *M*.alpha × tint alpha. `environment.md` §12 leaves the lighting of sky objects unstated and their fog OPEN; it gives one fog fact, which is the caller's to apply: when fog is on and `fog_extra` > 0.99 the sky objects are not drawn (`environment.md` §11.7 step 10). Alpha blending for every surface (additive for `Additive` surfaces, the surface's own blend for `Fixed` surfaces, with depth write off as for every sky surface, and the alpha their factors read scaled by the keyframe opacity), the alpha test for `Mask`/`TestBlend` surfaces, both faces. Objects are drawn in the given order (later over earlier); the caller passes the day description's stored order, and that this order is back to front is assumed (`environment.md` §12: whether lower draw numbers draw first is OPEN). An object with opacity ≤ 0 is skipped. |

<a id="s5-5"></a>
### 5.5 Instanced scenery and frills (`scenery.md` §8.3, §8.5, §9)

| Topic | Choice |
|---|---|
| Models | `Renderer::upload_world_model(&MeshData, &[WorldSurface])`, or `upload_world_model_with(&MeshData, &[WorldSurface], &[WorldSurfaceOptions])`: the mesh is validated under the renderer's own `MeshLimits` (`RendererConfig::meshes`) and packed as for `upload_mesh` (§2.1, the 40-byte vertex, no wireframe edges) into buffers of the model's own; submesh *i* uses surface min(*i*, len − 1). `WorldSurfaceOptions::fog` (default on; `upload_world_model` uses the defaults) is the fog switch of an **unlit** world surface (`lighting.md` §7 code 0x00: fogged when the record's program reads the fog inputs); a lit world surface is always fogged (§4.1). **Opacity (Decision; `WorldSurface` has no opacity-source field):** the base alpha is the opacity (alpha test, output alpha, shadow cut-outs) unless the surface takes its gloss from the base alpha; then it has no opacity source (opacity 1), as `material.md` §11.2 ("no source, no alpha test") and §11.3 ("a gloss alpha never reaches the output") give; a caller whose base alpha means something else (a dye mask) must not pass a non-opaque `AlphaMode` for it. `WorldSurface`: *M*, base and normal textures (the upload's own samplers), `AlphaMode` (as for lit meshes, §4.3), two-sided, specular (colour, power), gloss from the base alpha or from the normal texture's alpha (a normal-and-gloss texture, when it resolves; added in the M4.5 verification: without it such surfaces took gloss 1 and frills of row F2 drew near-white), vertex colours, unlit. At most 4,096 surfaces. A texture that does not resolve (unknown, destroyed, a render target) is absent: the base binds white, and the normal-map switch is set only for a normal texture that resolved (and a mesh with tangents). |
| Batches | `InstanceBatch`: a model, `Arc<[Instance]>` (`Instance`: affine transform, tint multiplying *M* and the specular colour *Ms*, as the client's per-object tint does, `material.md` §8.3; the terrain colour beneath; `light_object`, the instance's handle for the parent switches), the light selection (`Auto` selects **per instance**, §4.3 "Per-instance lists"), shadow casting, an optional per-instance distance limit (`scenery.md` §9.1 Decision: `d0` of the LOD class) and optional `FrillParams`. |
| GPU layout | Per instance 96 bytes (three rows of the affine matrix, tint, terrain colour with a sway phase from the instance origin, and the instance's light list as one `vec4<u32>`: x, y = eight light-table indices as bytes, z = count) in one vertex buffer per pass and frame; per batch a 48-byte uniform (flags in its first vector, whose other three words are unused; *D*, *F*, animation and terrain-colour intensities; the model's z_min, sway height and fade-in) at a dynamic offset; group 2 the model surface's retained bind group. Each batch's colour instances are ordered upright first, then mirrored (negative determinant). **One `draw_indexed` per batch and submesh**, two for a one-sided surface when the batch has both upright and mirrored instances (the mirrored range drawn with the other face culled, **Decision**, as §2.3 does for mirrored transforms), and one more per casting submesh in the shadow stage. At most 1,048,576 instances per pass (`MAX_INSTANCES_PER_PASS`, lower if the device's buffer limit is): instances beyond it are **dropped** in queue order, counted in `instances_culled` and logged once per pass, so a dense view cannot fail the frame. The batch uniforms are bounded per queued batch (§5.1 Per-pass buffers). |
| Culling | Per instance: non-finite transforms dropped, the distance limit (sphere start beyond it), and the model's bounding sphere × transform against the view frustum. **Decision** (conservative culling, no visual effect): for a frill group the culling sphere is the model sphere with its radius grown by the largest sway displacement (intensity × *h* × √1.25 when the group sways) and at least the distance from the model origin to the sphere's centre (the distance scale shrinks vertices toward the origin); the distance limit still uses the plain sphere. A frill group's box (rules 2 and 4) is built only from instances with a finite transform and a finite placed sphere; a group with none is culled whole. Instances outside the view are not drawn in colour (`instances_culled`), but in a batch that casts they still cast when their sphere meets the shadow map's volume (the frame's shadow `view_proj`): they follow the batch's colour instances in the instance buffer, and the shadow stage draws both (§4.4: casters outside the view still cast into it; the same rule as terrain chunks, §5.2). |
| Lighting | As the lit mesh pipeline (§4.1): ambient and hemispheric, sun with shadow, up to the instance's table lights, specular, fog (unlit surfaces by `WorldSurfaceOptions::fog`), alpha test, the same alpha modes. Normals: the inverse transpose of the instance matrix (× the sign of its determinant) for scenery. The tangent frame is the §4.1 Decision; an instance with a negative determinant flips the tangent's sign *w* in the scenery and the frill branch alike (§2.3). |
| Frill rules (§8.3, §8.5) | Rule 2–3: a group whose bounding box is at least *D* from the camera is skipped. Rule 4: `frill_reduced_count` draws the first *g* × max(1, trunc((1 − *f*) *n* / *g*)) instances. Rule 5: `frill_fade_in(age)`. Rules 1–3 helpers: `frill_radius_sectors`, `frill_distances` (*D*, *F*). §8.5 step 5: the sway weight *w* = clamp((*v*.z − z_min) / *h*, 0, 1) from the vertex block's stored bounding box, `FrillParams::sway_z_range` (z_min, z_max), or the box computed from the model's vertices when it is `None` (equal on all observed data, `mesh.md` M7); no sway when the box is at most 0.01 high; normals rotated, not scaled (the instance matrix's normalised columns). **Decision:** non-opaque groups (`Render_Frill_IsOpaque` false) draw their opaque and alpha-tested surfaces blended with depth write in the translucent stage (`scenery.md` §8.3 rule 6 gives only the shader variant). **Decision:** frill batches cast no shadow by default (not specified for the original). `FrillParams::default()` is the engine default: level 2 = 10 sectors, *D* = 97, *F* = 30. Frill instances must carry the terrain colour beneath them (`Instance::new` gives white). |
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
  fogged ground (unchanged by the 2026-10-07 change to the client's rotation: the test's sun card lies on +x with angles that point it at the same direction). Pixel checks: the placement rule (*A* raises +x toward the zenith, *B* turns clockwise seen from above),
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

> **OPEN (updated 2026-10-08):** the sky objects' scale and fog (§5.4; `environment.md` §12
> OPEN). Their rotation follows the client (§5.4, `environment.md` §11.4 step 5).

> **OPEN:** Terrain layers beyond the full-detail band and the near/far seam
> (`terrain-texturing.md` §12): every chunk is drawn with all its layers at any distance (the
> walker's far band, `walker.md` §14.4, is the caller's).

> **OPEN:** Water reflection, the depth-texture (soft shoreline), flow and foam permutations
> and the low-detail permutation are not drawn (`water.md` §8.2–§8.4 OPEN). The draw order of
> water relative to other translucent surfaces is Tessera's (§5.1 Stages, Decision): in the
> translucent stage blended instances draw first, then water, then the sorted blended
> meshes; water is not sorted with them.

> **OPEN (spec writer, 2026-10-07, for the `tessera-render` implementer):** decide whether
> water should draw before the blended instanced surfaces, so that a `Blend`-mode instance in
> front of water (frills over a shore, scenery with blended parts) is not replaced by the
> water's ONE/ZERO write (§5.1 Stages). Either order is a Tessera choice; the original's is
> not specified.

> **OPEN (for the implementer and the callers):** `WorldSurface` has no explicit opacity
> source (§5.5 Models Decision); add one, together with the callers' values, when the callers
> can supply it.

<a id="s6"></a>
## 6. UI pass (M5)

Status: **Accepted** (§6 only). Review of the second revision: Claude review session
(spec-writer agent, fresh session, neither the writer nor the reviewer of the first revision),
2026-10-09: **passed; §6 moved Revised → Accepted.** The three blocking items of the review of
the first revision are resolved: (1) the §6.16 OPEN now states the decided rule (R12) and keeps
open only the original's clear, composition and non-D3D9 paths; (2) "What this section is"
names R11 (OBS) and R10 (BIN, D3D9 path only); (3) "Client portrait path" is scoped to the D3D9
path, its unknowns are `> **OPEN:**` callouts (a)–(c) tied to the `ui-widgets.md` §12 OPEN, and
the former "Consequence" bullet is an expected result conditional on OPEN (a) and (b); "Why
this rule", the answered implementer callout and the effects table say D3D9. The non-blocking
notes are applied. Every `CLEAN_ROOM.md` §5.3 item holds: functional facts and Tessera rules
only, no code or listing structure, no copied prose, labels are interoperability identifiers,
no game text, keys or hostnames, each row marked (R10 BIN with build, question and M1–M3
reason under decision 0011; R11 Confirmed; R12 DEC), testable consequences concrete, no closed
gate. Arithmetic re-checked: the "Fixed blends" exceptions, the empty-pixel rows (additive
exact; modulate, DESTCOLOR / ONE, MIN, REVSUBTRACT black; MAX and SUBTRACT add their source
term) and the additive consequence min(1, *B* + *a*·*s*). BIN spot check (private Ghidra copy,
deleted afterwards): of 197 direct render-state calls through the D3D9 device, three set the
colour-write state, one through a cached wrapper, two to red, green and blue (a reset path that
also caches it), and none sets the separate alpha-blend states, matching R10's resting mask and
the "Client portrait path" bullet; the mask draw itself was not re-derived. Non-blocking: in
the effects table, the "Partly covered" row's "for destination factor ONE with ADD the result
is again exact" holds only when the source factor does not read the destination (as the
"Empty" row says; DESTCOLOR / ONE is not exact); the first 2026-10-09 change-log row still calls
the background colour inside the mask a "known difference", which the second revision made
conditional (history, superseded by the next row). Before that: **Revised**.
Second revision, 2026-10-09 (spec writer, dirty room), answering
the three blocking items of the review below: the §6.16 OPEN on additive and multiply blends is
rewritten (the alpha rule is decided; what stays open is the original's composition of a
portrait); the "What this section is" paragraph names R10 (BIN) and R11 (OBS); the unknowns of
"Client portrait path" (§6.10) are `> **OPEN:**` callouts, and its client claims, the "Why this
rule" paragraph and the effects table are limited to the client's D3D9 path. The non-blocking
notes are applied too (R11 counts six properties and says the background colour is set
explicitly; exception 2 states that ZERO / ZERO draws opaque black; an M3 frame capture is
named as the check of R10). No Tessera rule changed. Review of the second revision: above.
First revision, 2026-10-09: the implementer OPEN of §6.10 (how fixed blend
states write alpha in a premultiplied pass) is answered by a Tessera Decision resting on client
facts found with method M4 (rows R10–R12; change log §8). Review of
the first revision: Claude review session (spec-writer agent, fresh session, not the writer),
2026-10-09: **changes requested; §6 stays Revised.** Passed: no code or listing structure, no
copied prose, labels are interoperability identifiers, no game text, keys or hostnames, R10 has
the build, question and M1–M3 reason that decision 0011 requires, no closed gate; the "Fixed
blends" arithmetic and its effects table check out (empty, opaque and partly covered pixels;
both exceptions reproduce source-over and opaque results), and the testable consequences are
concrete. Spot checks matched: the executable's version resource reads 4906.0071.1794.4035; in
the raw 0x22 records each of the six viewport PIDs of R11 (0x0520, 0x0B93, 0x0176, 0x00C9,
0x0652, 0x05AF) occurs exactly twice, both in 0x22000033, index 1 with mask 0x40000E4C inside
`OpponentPanel` and index 0 with mask 0x40000E4B inside `AvatarPanel`, size 62, background
0xFF000000, `RenderInParent` 1. The BIN behaviour of R10 was not re-derived. Blocking:
(1) §6.16 still carries the OPEN "what other blend modes (additive, multiply) must write to
alpha in such a pass is not decided", which the new rule answers; close or rewrite it (the
change log says no other section changed). (2) The "What this section is" paragraph still says
that the only data fact re-checked is R7 and that nothing here comes from disassembly; R10 is
BIN and R11 OBS now. (3) "Client portrait path" leaves unknowns as prose instead of
`> **OPEN:**` callouts: the clear of the target (not traced), how the parent surface takes the
pixels (not traced), and the D3D10, D3D11 and OpenGL paths (not examined, yet the "Why this
rule" paragraph and the "Consequence" bullet state "the original" in general); the
"Consequence" bullet presents as fact a visible result that depends on the two untraced steps.
Mark these OPEN (or point to the `ui-widgets.md` §12 OPEN) and scope the claims to the D3D9
path. Non-blocking: R11 says "all five properties" but names six (index, `RenderInParent`,
width, height, background colour, mask); say the background colour is set explicitly although
it equals the dictionary default; exception 2 includes ZERO / ZERO, which writes opaque black,
say so if intended; an M3 frame capture should confirm R10 when one is available. Before that:
**Accepted**. Review record: Claude review session (spec-writer agent,
fresh session, not the writer), 2026-10-08: **passed; §6 moved Review → Accepted.** Every
`CLEAN_ROOM.md` §5.3 item holds: Tessera rules (DEC) and data facts only, no code or listing
structure, no copied prose, element and property labels are interoperability identifiers, no
game text, keys or hostnames, R1–R9 cover the section, invariants and T1–T20 are testable,
unknowns are `> **OPEN:**`, no closed gate. Consistent with decision 0015 §8 and Amendment 1.
Spot checks matched: R7/R8 on layout 0x22000012 (18 non-root moded elements; radar, marker and
rim mode 1; rim 7 columns left and 6 right of the 224-wide root), the wgpu 30 downlevel flags and
limit, and the rotation convention. Non-blocking: say in §6.5 that a near-zero rotation is kept
as identity (as §6.17 does) so §6.8 space indices never shift; §6.14 omits the root-level drag
bar and title surfaces from its 18. Earlier: Review (spec writer, 2026-10-06). §4 and §5
are the M4.5 sections (`render.md` of the M4.5 branch); this section follows them and does
not change them.

What this section is. Decision 0015 (UI rendering, "UR" below) §8 asks for a "UI pass"
section in this file, and decision 0014 (UI input bridge, "IB") fixes the frame steps and the
pointer lock that it depends on. The section turns UR §8, its P1 checks and the amendments
that UR's open review items ask for into the renderer's contract. Almost all of it is **Tessera design** taken from those Accepted
decisions. Most facts about the game data are cited by spec section only (`game-panels.md`
G39–G41, `ui-layout.md` U46); two were observed in the install for this section: row R7
(re-checked) and row R11 (the portrait census, OBS). One group of client facts comes from
disassembly (method M4 under decision 0011): row R10, **BIN**, the client's portrait path on its
built-in D3D9 path only, recorded in §6.10 "Client portrait path" as the background of the
Tessera Decision R12. Nothing else here comes from disassembly. Names of types and functions are
suggestions for the implementer; the behaviour is the contract.

Abbreviations: UR = `docs/decisions/0015-ui-rendering.md`, IB =
`docs/decisions/0014-ui-input-bridge.md`, UL = `ui-layout.md`, UB = `ui-behaviour.md`, UW =
`ui-widgets.md`, LPA = `lua-plugin-api.md`, GP = `game-panels.md`, FO = `fonts.md`.

<a id="s6-1"></a>
### 6.1 Provenance

| # | Statement | Source | Mark |
|---|-----------|--------|------|
| R1 | Surfaces are 32-bit 0xAARRGGBB canvases with straight alpha, little-endian, uploaded unconverted to `Bgra8Unorm`; texture = tile box of the sparse canvas; front/back textures, GPU copies, scissored clears; UI upload budget apart from the asset budget | UR Decision 1.1, 1.2, 1.5, §6 | DEC (Accepted 2026-10-06) |
| R2 | One quad per entry; displayed box, composition scale, transform chain, clip chain, opacity, source flag, filter, alpha threshold, repeat addressing; order of the screen-space passes | UR Decision 1.6, §2.1–§2.3, §8 | DEC |
| R3 | Premultiplied filtering in the shader, bilinear (default) or nearest, no mips | UR §2.2; FO Q3 OPEN | DEC (Tessera default) |
| R4 | The alpha threshold compares the texel alpha **before** opacity | UW §4.5 Tessera rule; UR open review "Alpha-threshold order" | DEC (amends UR §8) |
| R5 | The clip chain is built from the enclosing **windows**, including mode-0 child windows drawn into an ancestor's canvas | UR open review must-fix 1; LPA §13.18 OPEN (clip chain) Tessera rule, invariant 60 | DEC (amends UR Decision 1.6/1.7) |
| R6 | Software cursor, pointer lock, OS cursor default | UR §2.5; IB D7 | DEC |
| R7 | In P1 (layout 0x22000012) `GamePlay_Radar`, `MinimapPlayerMarker` and `Minimap_Frame` carry object mode 1; 18 non-root P1 elements carry a non-zero mode | GP G39, G40; UL U46. Re-checked for this section: all 302 0x22 layouts re-read read-only from the install's directory index on 2026-10-06 and the P1 tree resolved again | OBS, **Confirmed** |
| R8 | The rim is in its own canvas; the marker's canvas above is empty at every heading (corrections to UR's P1 checks 2 and 4) | GP §8.7.2 "How P1 splits into surfaces", GP17, GP §14 | DEC on data G40, G41 |
| R9 | Tile-box texture allocated at the job's full size; screenshots leave out the software cursor; the full-canvas size for a cropped canvas | UR open review items ("banded first paint", "software cursor", "cropped canvas") | Tessera rule (this section) |
| R10 | How the original makes a viewport's image on its built-in D3D9 path (§6.10 "Client portrait path"; the D3D10, D3D11 and OpenGL paths are OPEN there): a viewport without a render-target index draws its scene straight into the frame inside the element's box; one with an index draws into that render target, then writes the alpha of its opacity-mask image into the target's alpha channel alone, replacing it; with `RenderInParent` the result goes to the parent's surface. The D3D9 path's resting colour-write mask is red, green and blue (alpha off) | **BIN** (M4, decision 0011). Client executable `x64/lotroclient64.exe`, build 4906.0071.1794.4035 (64-bit; the viewport code and the built-in D3D9 path; the separate D3D10, D3D11 and OpenGL libraries were not examined), analysed 2026-10-09. **Question:** how the alpha of a portrait's image is produced and which channels its scene draws write (the §6.10 OPEN). **Why M1–M3 were not enough:** the data holds only the viewport properties; no published source or research clone describes the path (searched 2026-10-09); a frame capture of the running client (M3) was not available to this session. **Check still wanted:** an M3 frame capture of a portrait (render-state and blend of the mask draw, the target's clear, the parent's composition) | BIN |
| R11 | Of the 36 viewport instances in the instantiated 0x22 layouts, only the two portraits set a render-target index: `PlayerPortrait` (P4) index 0 and `OpponentPortrait` (P5) index 1. Both set `RenderInParent` true, a 62 × 62 target, background colour 0xFF000000 (opaque black; set explicitly in both records, although it equals the dictionary default, `ui-widgets.md` §12) and an opacity mask (`UICore_Viewport_RenderToTextureOpacityMask`, PID 0x05AF; DID 0x40000E4B for P4 and 0x40000E4C for P5). The other 34 leave all six of these properties (index, `RenderInParent`, width, height, background colour, mask) unset, so their index is the default −1 | OBS: census of all instantiated layout elements (layout records read read-only from the install on 2026-10-06, counted again 2026-10-09) | **Confirmed** |
| R12 | Fixed blend states keep the destination alpha in a premultiplied pass, with two named exceptions (§6.10 "Fixed blends") | Tessera Decision on R10 | DEC (this revision) |

<a id="s6-2"></a>
### 6.2 UI textures

| Topic | Choice |
|-------|--------|
| Format | `Bgra8Unorm`, **linear**: no sRGB decode on read. A canvas pixel 0xAARRGGBB is stored little-endian as the bytes B, G, R, A, so canvas rows upload with no conversion (UR Decision 1.1). The colour is **straight** (not premultiplied) alpha; only external content (§6.10) is premultiplied. |
| Usages | `TEXTURE_BINDING` (read with `textureLoad`, no sampler), `COPY_DST` (sub-rectangle uploads and the destination of GPU copies), `COPY_SRC` (the source of the front-to-back copy and of the copy when a tile box grows or shrinks, UR Decision 1.2, 1.5), `RENDER_ATTACHMENT` (the scissored clear of §6.3). |
| Type | A separate creation call, for example `Renderer::create_ui_texture(width, height, label) → Result<TextureId>`, makes a **UI texture**: one level, the format and usages above, content transparent black (0, 0, 0, 0). It is not a `TextureUpload` with a format field: a UI texture starts empty, has no sampler and must carry `RENDER_ATTACHMENT`. `destroy_texture` releases it like any texture. |
| Size | Each axis 1 ..= min(device `max_texture_dimension_2d`, 16,384) (UR §7). A larger request fails with `RenderError::InvalidTexture`. |
| Coverage | A UI texture holds the **tile box** of its surface: the tile-aligned bounding box of the surface's allocated 256 × 256 tiles, not the whole canvas (UR Decision 1.2). The entry gives the position of the texture's texel (0, 0) inside the canvas (§6.5, `tex_offset`). Texels of the canvas outside the texture read as transparent black. |
| Memory | `Renderer::ui_texture_bytes()` and `FrameStats::ui_texture_bytes` give the sum of width × height × 4 over live UI textures, so the client can keep U5's GPU budget (front and back textures ≤ 256 MiB, UR §6). |

<a id="s6-3"></a>
### 6.3 Sub-rectangle updates, GPU copies and clears

| Call | Contract |
|------|----------|
| `update_texture_rect(id, x, y, w, h, bytes, row_stride) → Result<()>` | Writes a *w* × *h* rectangle at (*x*, *y*) of a single-level texture with 4 bytes per texel. UI textures take B, G, R, A bytes, and RGBA8 textures from `upload_texture` take R, G, B, A. Checks, each failing with `RenderError::InvalidTexture` and never panicking: the texture exists and belongs to this renderer; it is not a registered target texture; it has one level; *x* + *w* ≤ width and *y* + *h* ≤ height, computed without overflow; `row_stride` ≥ 4*w*; `bytes.len()` ≥ `row_stride` × (*h* − 1) + 4*w*. A rectangle with *w* = 0 or *h* = 0 is accepted and does nothing. The bytes are copied at the call, so the caller may reuse its buffer at once. |
| `copy_texture_rect(src, sx, sy, dst, dx, dy, w, h) → Result<()>` | GPU copy between two **different** UI textures (front → back, and growing or shrinking a tile box). Both rectangles are bounds-checked as above. `src == dst` is refused, because overlapping copies within one texture are not allowed. |
| `clear_texture_rect(id, x, y, w, h) → Result<()>` | Sets a rectangle of a UI texture to transparent black (a tile the job freed, UR Decision 1.5). It is a **draw** under a scissor equal to the rectangle (blending off, writing (0, 0, 0, 0)), on an attachment loaded with `LoadOp::Load`. A `LoadOp::Clear` is not used, because it ignores the scissor and would clear the whole texture. |
| Order | All three kinds of operation on UI textures run on the GPU **in the order of the calls**, and before any pass of the next submitted frame reads the textures. A queued write must not overtake an earlier copy or clear: copy the front into a back texture, then upload the damage into it, and the upload must win. `Queue::write_texture` alone runs before every command buffer of its submission, so it would lose; the implementation must avoid that, for example by staging uploads in a buffer and copying them in the same encoder as the copies and clears. |
| Statistics | `FrameStats` gains `ui_upload_bytes` (4 × *w* × *h* per update; the stride padding is not counted), `ui_upload_rects`, `ui_gpu_copies`, `ui_copy_bytes` and `ui_clears`. They count only these calls, are reported **apart from** any asset-upload counter, and are never counted in the asset budget of ARCHITECTURE §20.3 step 5 (24 MiB and 4 ms, `walker.md` §2). U3's per-frame UI cap (8 MiB) is the client's to keep (UR §6); the renderer only counts. |

<a id="s6-4"></a>
### 6.4 Blending on stored values: the non-sRGB view

The original composes panels on stored 8-bit values, with no sRGB decoding (UL §9.6.1, whose
blend is OPEN). The UI pass does the same.

| Topic | Choice |
|-------|--------|
| Swap chain | When the surface format is an sRGB format, the renderer adds its non-sRGB variant (`Bgra8UnormSrgb` → `Bgra8Unorm`, `Rgba8UnormSrgb` → `Rgba8Unorm`) to `SurfaceConfiguration::view_formats`. The UI pass, and **only** the UI pass and the software cursor, render into a view of the frame's texture created with that variant. A surface whose format is already non-sRGB is used as it is. |
| Offscreen targets | Targets created with `OffscreenFormat::Rgba8Srgb` list `Rgba8Unorm` in their `view_formats`, so a capture (§6.11) composes the UI exactly as the window does. |
| Effect | The shader's output is stored without encoding, and the blend unit reads the stored bytes, so blending works on stored values. `textureLoad` on the `Unorm` UI textures returns stored byte ÷ 255. |
| Other passes | Unchanged. World passes, the walker overlays and the developer overlay keep the sRGB view. Under the M4.5 Decision of §4.1 the lit passes blend decoded values on sRGB targets. The last OPEN of §4.8 (blending on sRGB targets) suggests rendering into a `Unorm` target for an exact match; this section adopts that fix **for the UI pass only**, and §4.1 and §4.8 stay as they are. |
| Availability | Needs `DownlevelFlags::SURFACE_VIEW_FORMATS` for the swap chain and `DownlevelFlags::VIEW_FORMATS` for offscreen textures (wgpu 30). GL/GLES backends and Vulkan on Android lack them. |
| Fallback | Where a target cannot get the view, the UI pass draws into its sRGB view, and the shader converts its premultiplied output to linear light before the blend. Divide out alpha, decode each colour channel with the sRGB transfer function, and multiply alpha back in; alpha is not converted, and alpha 0 gives 0. The hardware re-encodes on store. Pixels whose output alpha is 1 are exact, because decode followed by encode must return every 8-bit value unchanged, and a test checks all 256. Blended pixels are blended in linear light, which is a recorded known difference. It is logged once (`tracing` info), and `Renderer::ui_view_mode()` reports `Stored` or `LinearFallback`. |

<a id="s6-5"></a>
### 6.5 The composition entry

The client records one UI pass per frame (for example `Frame::ui_pass()`), holding an ordered
list of **entries** that the client builds from `tessera-ui`'s composition list (UR Decision
1.6). The renderer knows nothing of elements, windows or generations. It draws what the entry
says.

| Field | Meaning | Validation |
|-------|---------|------------|
| `texture` | A UI texture (§6.2) or a texture from `register_target_texture` (external content, §6.10) | exists, belongs to this renderer; an sRGB target is refused (§6.10) |
| `source` | `Straight` (UI textures) or `Premultiplied` (the **external-content flag**: the shader skips its premultiply step) | — |
| `canvas` | (*cw*, *ch*): the size of the **full canvas**, ≥ 1 on each axis. For a Fit surface it is the element's layout box; for a Tile surface it is the canvas that repeats (UR §2.1) | 1 ..= 16,384 per axis |
| `tex_offset` | (*ox*, *oy*): the canvas position of the texture's texel (0, 0). It is the tile-box offset (UR Decision 1.2) plus, for a canvas cropped by UR §7, its canvas origin offset | `i32`, any value |
| `dest` | (*x*, *y*, *w*, *h*): the **displayed box** in integer physical pixels (§6.6) | *w*, *h* ≥ 1 |
| `addressing` | `Fit` (scaled onto `dest`) or `Tile { node_scale: (sx, sy) }` (repeat addressing, UR Decision 1.6) | `node_scale` finite and > 0 |
| `filter` | `Linear` (default) or `Nearest`, from the Tessera setting `ui_scale_filter` (UR §2.2) | — |
| `opacity` | The element's `UICore_Element_ObjectOpacity`, clamped to 0–1 when drawn (LPA §13.19 Tessera rule); a stand-in surface passes 1 (UR Decision 1.1) | NaN → 0 |
| `alpha_threshold` | *t* in 0–1; 0 drops nothing. Only alpha-test meters set it, to 1 − *p* (UW §4.5) | NaN → 0, clamped to 0–1 |
| `rotations` | At most 9 rotations, nearest first: the one held by the entry's **own** surface (if any), then those held by each surfaced ancestor's surface up to the root. Each has a centre in physical pixels (the centre of that surface's displayed box) and three angles in degrees (§6.6) | ≤ 9; a non-finite angle → 0; a rotation with all three angles within 0.0002 of 0 is dropped |
| `clips` | At most 8 items, each an integer rectangle with a **space index** *k* (§6.8) | ≤ 8; 0 ≤ *k* ≤ number of rotations |

> **OPEN:** (clean-room implementer, 2026-10-08; the review record's non-blocking note) the
> `rotations` row says a near-zero rotation "is dropped", but the clip items' space indices
> *k* (§6.8) count positions in the list as the builder gave it, so removing an item would
> shift every *k* after it. The implementation keeps such a rotation in the list as the
> identity (§6.17 "Rotations"), which draws the same pixels as dropping it while every *k*
> keeps its meaning, and it is the list length before this step that bounds *k*. Should
> §6.5 say "kept as the identity" instead of "dropped"?

**Full canvas and stored part** (Tessera rule, R9). The composition scale always uses the full
canvas: (*w* ÷ *cw*, *h* ÷ *ch*). The texture holds only the **stored part**, which is the
tile box, or for a canvas cropped by UR §7 the on-screen part, and `tex_offset` places it. So
a crop never changes the scale (UR open review item "cropped canvas": "use the size of the
layout box and treat the crop as an offset").

**Limits** (UR §7). There are at most 8,192 entries per frame, counting nested entries,
canvases above and external content; the software cursor is not an entry. The client's
builder keeps that cap. The renderer enforces it again as a backstop: it drops the entries past
8,192, and it refuses any entry that fails a check above. Each refusal adds 1 to
`FrameStats::ui_entries_refused` and gives one `tracing` warning per kind of failure and
renderer; it never panics. `FrameStats::ui_quads` counts the entries drawn, and `ui_surfaces`
the distinct textures they read.

**Cap counters of UR §7.** The caps on surfaces per apartment and in all, on depth, on memory,
and the Tile-parent fallback are applied by `tessera-ui` and the client, not by the renderer.
So the counters reach `FrameStats` in one place, the client hands them to the UI pass (for
example `UiPass::cap_counters(UiCapCounters)`), and `end_frame` copies them unchanged into
`FrameStats::ui_caps`.

<a id="s6-6"></a>
### 6.6 Placement and the transform chain

| Topic | Choice |
|-------|--------|
| Destination | The element's displayed box of UB §9.2.1 in integer physical pixels. The transition rule of UR Decision 1.5 is applied by the client: while a group's generation runs, every entry of the group gets the box, rotation centres and clip rectangles of the **last swapped generation**, and the root's position follows UR's rule. Opacity and angles always take their current values. |
| Pixel rule | Screen pixel (*X*, *Y*) has its centre at *p* = (*X* + ½, *Y* + ½). An unrotated entry covers exactly the pixels whose centres lie in the box: *X* in [*x*, *x* + *w*), *Y* in [*y*, *y* + *h*). |
| Local position | *q* is *p* taken back through the entry's transform (below), so *q* is in the root's unrotated space. In practice the vertex shader passes each corner's unrotated position, and the interpolation is exact because every map here is affine. |
| Canvas position | `Fit`: *c* = (*q* − (*x*, *y*)) ⊘ (*w* ÷ *cw*, *h* ÷ *ch*), componentwise. `Tile`: *c* = ((*q* − (*x*, *y*)) ⊘ (*sx*, *sy*)) mod (*cw*, *ch*), with a Euclidean modulo (UR Decision 1.6). |
| 1 : 1 path | An entry is at 1 : 1 when it has no rotation and either `Fit` with *w* = *cw* and *h* = *ch*, or `Tile` with node scale exactly (1, 1). The texel is then computed in integers: (*X* − *x*, *Y* − *y*), wrapped modulo (*cw*, *ch*) for Tile. A 1 : 1 copy is therefore exact, with no rounding. |
| Rotation of one surface | Angles are reduced modulo 360 and converted to radians (LPA §13.20 step 4). The rotation acts on 3-D points (*x*, *y*, *z*) relative to its centre, with *x* to the right, *y* down and *z* = 0 on the screen plane. It applies *x*, then *y*, then *z*. The *x* rotation by *a* maps (*x*, *y*, *z*) to (*x*, *y* cos *a* − *z* sin *a*, *y* sin *a* + *z* cos *a*). The *y* rotation by *b* maps it to (*x* cos *b* + *z* sin *b*, *y*, −*x* sin *b* + *z* cos *b*). The *z* rotation by *c* maps it to (*x* cos *c* + *y* sin *c*, −*x* sin *c* + *y* cos *c*, *z*). So a **positive *z* turns counter-clockwise on screen**: a point right of the centre moves up. Tessera rule (LPA §13.20 OPEN: direction, order and projection; UR §2.3). |
| Chain | The entry's transform applies its rotations in list order (own first, root last), each about its own centre in 3-D, and projects **once at the end**, orthographically: it drops *z*. Tessera rule: a child lies in the plane of its tilted parent, as a card on a card (LPA §13.20 OPEN). With only *z* rotations this is the plain 2-D composition. A projected quad of zero area (a tilt of exactly ±90°) draws nothing. Rotated quads are drawn with **no culling**, because a tilt past 90° flips the winding. |
| Who sets the angles | The angles are those held by the surfaces (UR §2.3), never `GetRotation()` values. Entries derived from a root (its nested entries and canvases above) carry the root's rotation as their last item, so they turn with it. The minimap marker's *z* is chosen by the client so that the east-pointing arrow turns clockwise on screen by *h* − 90° (GP §8.6.3), so *z* = 90 − *h* under the convention above. |

<a id="s6-7"></a>
### 6.7 The UI pipeline

Per fragment of an entry's quad, in this order:

1. **Clip.** Run the test of §6.8 and discard the fragment outside it.
2. **Texel read.** Use only `textureLoad`. Canvas texel (*i*, *j*) is texture texel (*i* − *ox*,
   *j* − *oy*); outside the texture's extent it reads as transparent black (0, 0, 0, 0).
3. **Sample.**
   - At 1 : 1 (§6.6): read the one texel.
   - `Linear` (scaled or rotated): *u* = *c* − (½, ½); *i*₀ = ⌊*u*⌋, *f* = *u* − *i*₀. Read the
     four texels (*i*₀ + {0, 1}, *j*₀ + {0, 1}). For `Fit`, each coordinate is first clamped to
     the **full canvas**, 0 ..= *cw* − 1 and 0 ..= *ch* − 1 (clamp-to-edge, UR §2.2). For
     `Tile`, each is wrapped modulo the canvas size, and the tile-box lookup of step 2 comes
     after the wrap (UR Decision 1.6). Each texel is premultiplied, (*r*·*a*, *g*·*a*, *b*·*a*,
     *a*), and the four are weighted bilinearly by *f*. The result is premultiplied, so a
     transparent-black neighbour does not darken an edge.
   - `Nearest`: read the texel ⌊*c*⌋, clamped or wrapped the same way, and premultiply it.
   - With the external-content flag (`Premultiplied`), the texels are used as read, with no
     premultiply step.
4. **Alpha threshold** (R4). Discard the fragment when the sample's alpha is **less than** *t*.
   This compares the texel alpha (the filtered alpha when scaled), **before** opacity, so an
   alpha-test meter drops the same pixels at every opacity, including during a fade (UW §4.5
   Tessera rule). This replaces the order in UR §8's "UI pipeline" row, which applied the
   threshold after opacity.
5. **Opacity.** Multiply all four channels of the premultiplied sample by the clamped opacity.
6. **Fallback only** (§6.4): convert to linear light as described there.
7. **Blend**, as named constants (one place to change when a frame capture settles UL §9.6.1):
   `UI_BLEND_COLOR` = source factor `One`, destination factor `OneMinusSrcAlpha`, operation
   `Add`; `UI_BLEND_ALPHA` = the same. This is premultiplied source-over. Colour writes are on
   for all channels. There is no depth attachment, no depth test, no culling and one sample.

**Exactness.** At 1 : 1, with opacity 1 and threshold 0, over any destination:
- where the canvas alpha is 255, the stored pixel equals the canvas colour **exactly**;
- where it is 0, the destination is **unchanged exactly**;
- otherwise each channel is within ±1 of the straight source-over value
  round(*c*·*α* + *d*·(1 − *α*)).

<a id="s6-8"></a>
### 6.8 The clip chain

**What the builder puts in it** (contract for `tessera-ui` and the client; UR Decision 1.6 and
1.7 "Clip", amended by R5).
1. The **enclosing windows** of an entry *E* are every root panel strictly above *E*'s element
   (a plugin `Window`, a native root panel), from the nearest one up to the root of *E*'s
   group. A window counts whichever way it is drawn:
   - through its own surface;
   - through a stand-in surface (UR Decision 1.1);
   - with **no surface**, as a mode-0 child window drawn into the canvas of the nearest surface
     above it (UR Decision 1.1).
   Surfaces of other elements never clip: a plugin control with a stretch mode, the minimap
   marker, `GamePlay_Radar` (LPA §13.18 Tessera rule, "clipped to its window only", and its
   extension to the windows above).
2. Each enclosing window *W* contributes its displayed box (in the geometry of §6.6, including
   the transition rule), tested in the space of the **surface that draws it**: its own or
   stand-in surface, or for a mode-0 window the nearest surface above it. Its space index *k*
   is the number of *E*'s rotations that come **before** that surface's own rotation in *E*'s
   list. When that surface holds no rotation, *k* counts up to where its rotation would stand.
   *k* = the length of *E*'s list means screen space.
3. Consecutive items with the same *k* are intersected into one rectangle. If an intersection
   is empty, *E* draws nothing and the builder may drop it.
4. A root entry has an empty chain. A canvas above takes the chain of its surface's entry.
5. **At most 8 items.** Every *k* is the position of a surface strictly above *E* (a mode-0
   window's drawing surface is above the window, so it is above *E* too), and an entry has at
   most 8 surfaced ancestors under UR §7's depth cap. Items with equal *k* are consecutive,
   because the windows are listed nearest first.

**The test** (renderer). For an item (rectangle [*x*₀, *x*₁) × [*y*₀, *y*₁), *k*), take *E*'s
local position *q* (§6.6) as a 3-D point with *z* = 0, apply the first *k* rotations of *E*'s
list, and keep *x* and *y*. The fragment is inside when *x*₀ ≤ *x* < *x*₁ and *y*₀ ≤ *y* < *y*₁
(half-open, on pixel centres). Outside any item, it is discarded. An item with *k* = the length
of the list is a screen rectangle, and the half-open centre test then selects exactly the
pixels of the integer rectangle. So when every item is of that kind, the renderer may replace
the test by one scissor (their intersection) with an identical result. The test must work on
the renderer's downlevel adapters: no storage buffers in the fragment stage, and no more inter-stage variables than the
downlevel limits allow (`max_inter_stage_shader_variables` = 15), for example the 8 positions
packed two per `vec4`.

**Examples.**
- P1: the nearest enclosing window of every nested entry of `GamePlay_Radar` (rim, marker,
  buttons, labels) and of the radar itself is P1's root `GamePlay_RadarBox`, so each chain is
  that one unrotated box, which is a scissor (GP §8.7.2 "Clip").
- Mode-0 child window (R5): window *P* has a surface; its child window *C* is in stretch mode
  0, so it is drawn into *P*'s canvas; *C* holds a control *K* in stretch mode 1.
  - *K*'s chain is [*C*'s box at *k* = 0 (the space of *P*'s surface, before *P*'s rotation),
    *P*'s box at *k* = 0], merged into one rectangle. So *K* is cut at *C*'s edge.
  - With *P* rotated by *z* = 30, *K*'s list is [*P*'s rotation] and the merged item still has
    *k* = 0: *K* is cut along *C*'s edge as it turns with *P*.
  - Under UR as written (surfaces only), *K* would have been cut at *P*'s edge only.

<a id="s6-9"></a>
### 6.9 Order of the screen-space passes

After the world passes (opaque, alpha-tested, transparent), the frame draws (UR Decision 1.6
"Order", §8; ARCHITECTURE §13 as amended by UR):
1. the **walker overlays** when shown: the HUD text and the walker minimap (`walker.md` §6).
   With a game UI attached they are under the UI, the walker minimap is off by default
   (`walker_minimap = auto`), and the HUD follows `walker_hud`, which is off in checklist runs;
2. the **UI pass**: the entries in the given order, with no depth, no sorting and no batching
   that reorders;
3. the **developer overlay** (egui frame statistics, ARCHITECTURE §13 "Diagnostics"; planned
   and off by default);
4. the **software cursor** (§6.11), when used.

The renderer draws passes in the order the client records them, as today, and the client
records them in this order. The software cursor is the one exception: it is drawn after every
pass of the frame, whenever it was set. Without a game UI there is no UI pass, and the walker
overlays draw on top as in M3 and M4.

<a id="s6-10"></a>
### 6.10 External content

Portrait viewports (P4 and P5, optional in M5 by UW §12's Tessera rule) are offscreen targets
composed as entries (UR Decision 1.7 "External content").

| Topic | Choice |
|-------|--------|
| Target | `OffscreenFormat::Rgba8Unorm`, sized to the element's layout box (the entry's full canvas; `tex_offset` (0, 0)), and registered with `register_target_texture`. The entry carries the external-content flag. |
| Colour | Stored values: the lit passes compute stored values (§4.1), and the `Unorm` target keeps them, so `textureLoad` returns them unconverted to the non-sRGB UI view. |
| Alpha | **Premultiplied.** The target is cleared to (0, 0, 0, 0) in each frame it is drawn. Opaque draws write alpha 1. Source-over draws keep their colour blend and blend alpha with `One`/`OneMinusSrcAlpha`, so colour over the cleared target is premultiplied and empty pixels are transparent. Fixed blend states (additive, modulate, min, max) keep the destination alpha, with two exceptions ("Fixed blends" below, R12). The renderer offers this as an option of a pass drawn into such a target (for example `Pass::premultiplied_alpha()`), which sets the clear and the alpha component of every blend state. |
| sRGB refused | An entry whose texture comes from an `Rgba8Srgb` target is refused (§6.5): it would return decoded values into the non-sRGB view and darken the portrait. The client also refuses to register such a target as UI content, as a categorised failure: the portrait draws nothing. |
| Timing | The target must be drawn earlier in the same frame, or in an earlier submission, than the UI pass that reads it; otherwise the entry shows its previous content. An entry that reads the frame's own colour target is refused (no feedback loop). |
| M5 scope | The `UICore_Viewport_RenderToTexture…` properties are recorded but not used; Browser and Movie items stay empty (UW §12). |

> **Answered 2026-10-09 (spec writer, dirty room; was OPEN):** (implementer, merge of M5 with
> M4.5, 2026-10-08) The M4.5 fixed blend states (`FixedBlend`, used by the lit, instanced and
> sky pipelines for additive, modulate, min and max surfaces) keep the destination alpha, so
> they do not blend alpha with `One`/`OneMinusSrcAlpha` as the "Alpha" row required, and
> `Pass::premultiplied_alpha` does not change them. Question: in a premultiplied pass, should
> every fixed blend state blend alpha with `One`/`OneMinusSrcAlpha`, or should additive and
> modulate blends keep the destination alpha, and what do min and max blends write?
> **Answer:** they keep the destination alpha, except the two cases named in "Fixed blends"
> below (R12). On its D3D9 path the original gives no alpha behaviour to copy, because none of
> its scene draws decides a portrait's alpha ("Client portrait path", R10; the other graphics
> paths are OPEN there). The renderer's guard test no longer
> needs `FixedBlend` as an exception; it checks the rule below instead.

**Fixed blends** (Tessera Decision, R12). In a pass with the premultiplied option, a fixed blend
state keeps its colour factors and colour operation, and its alpha component keeps the
destination alpha: source factor `Zero`, destination factor `One`, operation `Add`. This holds
for every colour operation, `Min` and `Max` included; the alpha component has its own operation,
so the colour's `Min` or `Max` does not apply to alpha. Two fixed blends are not "fixed" in
effect and follow the rows above instead:
1. colour factors SRCALPHA / INVSRCALPHA with operation ADD is source-over: alpha with
   `One`/`OneMinusSrcAlpha`;
2. destination factor ZERO with a source factor that does not read the destination (ZERO, ONE,
   SRCALPHA or INVSRCALPHA), operation ADD, draws opaque: it writes alpha 1, as opaque draws do.
   This includes ZERO / ZERO, which therefore draws opaque black, as the same blend does in an
   opaque target; this is intended.

Outside premultiplied passes nothing changes (fixed blends already keep the destination alpha
there).

What the rule gives when the UI pass composes the target over what lies behind it, *B*
(premultiplied source-over, §6.7): the stored colour *C* and alpha *A* show as
*C* + (1 − *A*) · *B*, where *C* is what the fixed blend left in the target.

| Pixel of the target before the draw | Result after composition |
|---|---|
| Covered opaquely by the figure (*A* = 1) | The same colour as the blend gives in an opaque target; *B* does not show. |
| Empty, (0, 0, 0, 0) | The blend's result over black is **added** to *B*. With destination factor ONE, operation ADD and a source factor that does not read the destination (SRCALPHA / ONE, ONE / ONE, ZERO / ONE), this equals drawing the surface straight onto *B*, which is what a client viewport without a render target does on the D3D9 path (R10). |
| Empty, other blends | Modulate (ZERO / SRCCOLOR, DESTCOLOR / ZERO), DESTCOLOR / ONE, MIN and REVSUBTRACT give black over black and leave *B* unchanged; MAX adds the source colour, and SUBTRACT the source term, to *B*. Drawn straight onto *B* they would act on *B* (darken it, take the minimum or maximum, subtract). This is a known difference of the Tessera rule; it affects only pixels the figure does not cover. |
| Partly covered (0 < *A* < 1) | The colour blends with the stored premultiplied colour and *A* is unchanged; for destination factor ONE with ADD the result is again exact, for the other blends it lies between the two rows above. |

Why this rule: under R10, on the client's D3D9 path, a scene draw never decides a portrait's
alpha (its draws write red, green and blue only, and the opacity mask replaces the alpha
afterwards), so coverage in Tessera comes only from opaque and source-over draws, and additive
light, the common fixed blend, composes exactly. The alternative (`One`/`OneMinusSrcAlpha` for
every fixed blend) would make an additive draw over an empty pixel cover what lies behind it by
its source alpha, which neither of the two D3D9 viewport paths of R10 does. The rule is a
Tessera Decision and stands whatever the OPENs of "Client portrait path" turn out to be.

Testable consequences:
- In a premultiplied pass, a fixed blend draw (other than the two exceptions) leaves the alpha
  of every target pixel unchanged.
- An additive draw (SRCALPHA / ONE, ADD) of colour *s* and alpha *a* over an empty target,
  composed by the UI pass at 1 : 1 over a destination *B*, stores min(1, *B* + *a*·*s*) per
  channel, within ±1/255.
- Over a pixel an opaque draw covered, any fixed blend gives the same stored colour as the same
  draw into an opaque target, and alpha stays 1.

**Client portrait path** (BIN, R10; data R11). Recorded so a later revision of `ui-widgets.md`
§12 can use it; the Tessera rules of this section do not depend on it beyond R12. **Scope:**
every statement below describes the client's viewport code as it drives the built-in D3D9
path of the build in R10. The other graphics paths are OPEN (c) below.
- A viewport whose `UICore_Viewport_RenderToTextureIndex` is −1 (34 of the 36 instances) draws
  its scene straight into the frame being composed, with the device viewport set to the
  element's box, over what is already there. No alpha is involved.
- A viewport with index 0 or 1 (only the two portraits, R11) draws its scene into that render
  target, in a viewport of the property size (62 × 62 here), with the background colour
  (0xFF000000 here) passed to the scene draw as its clear colour. Whether and how the target is
  then cleared with it is OPEN (a). When the viewport has an opacity mask, the client then
  switches the colour-write mask to **alpha only**, draws the mask image over the whole target
  with blend factors ONE / ZERO (a copy), and switches the mask back to red, green and blue. So
  the target's alpha is the mask's alpha, whatever the scene draws did. A viewport with a mask
  is drawn only when the device reports that it can mask colour writes.
- With `UICore_Viewport_RenderInParent` true (both portraits) the target is copied to a
  readable texture and the element's parent surface is marked for repaint over the element's
  box, so the portrait reaches the screen through the parent's surface. How that surface takes
  the pixels is OPEN (b).
- The D3D9 path resets the colour-write mask to red, green and blue (alpha off), and none of
  its render-state call sites found sets the separate alpha-blend states. Every call site that
  enables alpha writes through the graphics device (all four channels, or alpha alone) switches
  back to red, green and blue afterwards.

> **OPEN (a):** the clear of a portrait's render target. The background colour reaches the
> scene draw as its clear colour, but the clear call was not traced: whether the target is
> cleared every frame it is drawn, whether the clear covers all four channels or the colour
> channels only, and whether a frame in which the figure draws nothing still clears it. Part of
> the `ui-widgets.md` §12 OPEN (the render-to-texture path). An M3 frame capture of a portrait
> settles it.

> **OPEN (b):** how the parent surface takes a portrait's pixels when
> `UICore_Viewport_RenderInParent` is true: the blend of that step, whether the colour is
> treated as straight or premultiplied, and whether the mask's alpha acts as coverage there.
> Not traced. This is the "how the result is composed into the panel surface" part of the
> `ui-widgets.md` §12 OPEN; the same M3 frame capture settles it.

> **OPEN (c):** the D3D10, D3D11 and OpenGL paths of the client (separate libraries) were not
> examined. Whether they keep alpha writes off by default and draw the opacity mask the same
> way is unknown; R10 and every statement of this block hold for the D3D9 path only.

- Expected visible result (depends on OPEN (a) and (b); not a confirmed fact): if, on the D3D9
  path, the target is cleared to the background colour in all channels and the parent surface
  uses the mask's alpha as coverage, a portrait shows its background colour (opaque black on
  this install) wherever the mask is opaque and the figure draws nothing, and its edge follows
  the mask. Tessera's portrait (cleared to transparent black, panel art showing where nothing
  is drawn, the properties recorded but not used; `ui-widgets.md` §12 Tessera rule) would
  differ there. That difference, if confirmed, belongs to the M5 scope of `ui-widgets.md` §12,
  not to the renderer's contract in this file.

<a id="s6-11"></a>
### 6.11 Cursor and screenshots

- **OS cursor (default).** The cursor is set through the windowing layer (UR §2.5), so the
  renderer does nothing. Visibility and grab are client work (IB D7).
- **Software cursor** (`ui_cursor = software`, or where custom cursors are not available).
  - **What is drawn.** One quad set with, for example, `Frame::software_cursor(texture,
    position)`. It is a UI texture of trunc(32 × `UIScale_Cursor`) pixels square, built and
    resampled once by the client (UB §9.3, UR §2.5). It is drawn at 1 : 1 at the pointer
    position minus the scaled hot spot, in integer physical pixels, through the UI pipeline
    with opacity 1, no clip and no threshold, into the non-sRGB view.
  - **When.** It is the last draw of the frame (§6.9). It is not a composition entry and is
    not counted in `ui_quads` or in the entry cap.
  - **Not drawn** while the pointer lock is on (IB D7; UR §2.5 item 1; not at *F* either),
    while the pointer is outside the window (after a cursor-left), or with `ui_cursor = os`.
    After the lock ends it is drawn again from the next frame at the mapper's position.
  - While it is in use, the client keeps the OS cursor hidden.
- **Screenshots** (Tessera rule, R9; answers UR's open review item). A screenshot holds the
  world passes, the walker overlays as shown, and the UI pass. It **leaves out the software
  cursor** and the developer overlay. Reasons:
  - an OS cursor is never in a capture, so both cursor modes give the same picture;
  - the owner's comparison captures (`docs/checklists/m5-visual.md`) judge panels, not the
    pointer;
  - the developer overlay is off in checklist runs anyway.

  This covers the walker's screenshot action and `--screenshot`, which render the frame into
  an offscreen target (§6.4 gives that target the same non-sRGB UI view). Whether the
  original's own screenshot feature records the cursor is not known; Tessera's rule does not
  depend on it.

<a id="s6-12"></a>
### 6.12 Tile-box textures over a paint job

Tessera rule, R9, amending UR Decision 1.2 for jobs (UR open review item "banded first
paint"). Under Decision 1.2 alone, every band of a banded first paint could grow the tile box
and re-create the texture with a GPU copy: up to 33 times for a 3840 × 2160 surface
(31.6 MiB).

- **Size once.** When a job starts, the client sizes the texture it will write into **once**,
  at the job's full size: the tile-aligned box of the tiles allocated at the start, joined with
  the job's damage box, both tile-aligned and clipped to the stored part of the canvas.
  - The texture is a back texture for a content job that needs one and for a generation that
    changes the tile box, and the front texture for a first paint.
  - A first paint's damage is the whole canvas, so its texture covers the whole stored part.
  - Bands upload into that texture. No band re-creates it.
- **Shrink once.** When the job completes, the shrink rule of Decision 1.2 is applied once: if
  the box of the allocated tiles is at most half the texture on an axis, the texture is
  re-created at that box with one GPU copy, before the swap.
- **Memory.** The texture counts toward U5 at its allocated size. If it does not fit, the
  rule of UR §7 applies: no back texture is created, the job waits, and a root's first paint
  draws nothing; nothing shown disappears.

> **OPEN:** (implementation review of `tessera-ui::paint`, M5) Three memory cases are not
> specified, and the implementation chose conservatively: (1) a back texture that could not
> fit U5 even with every other texture released is refused instead of waited for, so the
> surface's textures are dropped (it draws nothing, counted) and its generation does not stall;
> (2) a back texture that could fit later still waits with no time bound, so a generation can
> wait as long as other surfaces hold the memory; (3) canvas tiles past the canvas budget while
> painting are not allocated, so their pixels are dropped (counted once per job), not moved to
> the canvas below. Admission (UR §7, "would push past U5") takes the caller's estimate of the
> new surface's bytes, because a sparse canvas's size is known only after painting.

<a id="s6-13"></a>
### 6.13 Tests

**Renderer tests** (`crates/render/tests`, golden images or read-back; lavapipe and llvmpipe
GL like the other render tests, with the fallback path of §6.4 on GL):

| # | Test |
|---|------|
| T1 | **Golden 1 : 1** at opacity 1 over an opaque background: screen pixel = canvas pixel where canvas alpha is 255, unchanged where it is 0, ±1 of straight source-over otherwise. |
| T2 | **Sub-rectangle update:** stride larger than 4*w*; every bounds error returns `Err`; a zero-size rectangle is a no-op; in one frame, copy front → back, then update a rectangle, then clear another: the result shows all three in call order. |
| T3 | **Clear:** a scissored clear changes no pixel outside its rectangle. |
| T4 | **Tile box:** texels outside the texture read as transparent black; growing and shrinking by GPU copy keep the content; `tex_offset` places the texture. |
| T5 | **Scaled quad, both filters,** with a transparent-black border: no darkened edge (premultiplied filtering); nearest reads ⌊*c*⌋. |
| T6 | **Alpha threshold:** the same pixels are dropped at opacity 1 and 0.5 (threshold before opacity, R4); *t* = 0 drops nothing. |
| T7 | **Tile:** a 32 × 32 canvas on a 200 × 100 box repeats 1 : 1 from the box's top-left (screen (*x*, *y*) = canvas (*x* mod 32, *y* mod 32)); with node scale 1.5 each repeat covers 48 × 48 pixels with no seam darkening. |
| T8 | **Clip:** a nested entry extending past its root is cut at the root's edge, unrotated and with the root at *z* = 30 (cut along the rotated edge, turning with the root). |
| T9 | **Clip chain:** (a) a stretch-mode control inside a child window (own surface) that extends past the child window is cut at the child window's edge, not the outer window's; (b) with the child window extending past its parent window, both are cut at the parent's edge; (c) with the child window rotated by *z* = 30, the control is cut along the child window's rotated edge; (d) **mode-0 child window** (R5): the same control inside a child window in stretch mode 0, drawn into its parent's canvas, is cut at the child window's edge; with the parent rotated by *z* = 30 it is cut along the child window's edge turned with the parent; with the child window extending past the parent it is also cut at the parent's edge. |
| T10 | **External content:** a premultiplied `Rgba8Unorm` target with a half-covered edge texel composes without a dark fringe; at 1 : 1 its stored bytes reach the frame unconverted; an `Rgba8Srgb` target is refused. |
| T11 | **Round minimap** (GP §8.7.1): a synthetic surface with GP4's disc alpha over an opaque background: the four corners show the background unchanged, the centre shows the surface. |
| T12 | **Non-sRGB view:** stored bytes reach the frame unconverted; in the fallback, opaque pixels round-trip all 256 values exactly. |
| T13 | **Order:** overlapping solid quads in a walker overlay, the UI pass, the developer overlay and the software cursor stack in that order, even when the cursor is set before the passes are recorded. |
| T14 | **Rotation convention:** *z* = 90 turns a right-pointing arrow to point up; an *x* tilt of 60° halves the drawn height; a tilt of 90° draws nothing. |
| T15 | **Hostile input:** 8,193 entries leave 1 refused; more than 8 clips, more than 9 rotations, NaN angles (drawn unrotated), NaN opacity (nothing drawn), a zero-size box and a foreign texture are refused or sanitised without a panic and are counted. |

**Client and `tessera-ui` tests** (they drive the composition list; UR Decision 1.5):

| # | Test |
|---|------|
| T16 | **Back-texture swap:** no frame shows a partly uploaded surface, and the entry's box changes in the frame of the swap. |
| T17 | **Group transition:** during a root's resize generation, a nested entry and its canvas above keep their old place relative to the old root box, and all three change in the frame of the swap. |
| T18 | **Generation hold:** a root job that moves content between the canvas below and the canvas above for a moved child completes while the child's first paint is withheld; no frame shows the root's new split before the child's new place; the front texture is unchanged until the swap. |
| T19 | **Job-size texture** (§6.12): a banded first paint of a 3840 × 2160 surface creates its texture once, plus at most one shrink at the end. |
| T20 | **Screenshot** (§6.11): with `ui_cursor = software`, a capture holds the UI pass and no cursor pixels. |

<a id="s6-14"></a>
### 6.14 The P1 composition checks

UR §8 lists four checks that the composition path must pass for P1 (the radar, layout
0x22000012). They are added to the `tessera-ui` and client tests and to the M5 checklist run
(`m5-visual.md` P1-A). Check 2 is **rewritten** here and check 4 is **corrected**, both from
GP §8.7.2 "How P1 splits into surfaces" (R7, R8). UR as written treats the marker as P1's only
nested entry. The data give P1 18 non-root surfaces in two levels:
- `GamePlay_Radar` (mode 1) is a nested entry of the root's surface;
- `Minimap_Frame` (the rim), `MinimapPlayerMarker` and the other mode-1 children are nested
  entries of the **radar's** surface.

1. **Round, not square.** The composition uses the surface's alpha (GP §8.7.1). The disc
   (draw mode 2) and the map (draw mode 5) are painted into `GamePlay_Radar`'s canvas, so the
   minimap is round. Covered by T11 and by P1 on screen.
2. **The rim is in its own canvas** (rewritten). `Minimap_Frame` has object mode 1 (GP G39,
   UL U46), so it owns a surface. It is a nested entry of `GamePlay_Radar`'s surface, and its
   pixels are in **its own nested canvas**, not in the root canvas and not in the radar's.
   - **Clip.** The rim is painted without the viewport's clip, because the viewport is not a
     window and does not clip a nested entry. It is clipped only by its nearest enclosing
     window, P1's root (GP §8.7.2 Tessera rule, §6.8).
   - **Test.** A painter-and-composition test on a synthetic P1-shaped tree, with the rim art
     at its layout rectangle relative to the viewport, checks four things:
     - the rim's canvas holds the rim;
     - the composed frame shows rim pixels **outside the 170 × 170 viewport box**;
     - no rim pixel lies outside the root's box (at the design layout the root cuts 7
       columns on the left and 6 on the right, and no row);
     - the radar's canvas holds the disc and the map pixels with the disc's alpha (GP17).
3. **The marker points right.** The marker is a nested entry of the radar's surface with its
   own *z* rotation (§6.6). On screen it points up for *h* = 0 (north) and right for *h* = 90.
4. **No damage while turning** (corrected). While walking, P1's damage is the viewport box
   only: 170 × 170 in the radar's canvas (UR Decision 1.4). While turning there is no P1
   damage at all, because the marker's coverage region in the radar's canvas is the same
   40 × 40 square at every heading (UR Decision 1.7). A `tessera-ui` test turns the marker
   through 360° in 1° steps and finds no damage after the first rotation. Correction: the
   marker's **canvas above is empty at every heading**, not "the same rim pixels", because
   everything after the marker in the radar's drawing order owns a surface (GP G40, GP17).

<a id="s6-15"></a>
### 6.15 Invariants

1. For any entry at 1 : 1 with opacity 1 and threshold 0, pixels where the canvas alpha is 255
   equal the canvas colour exactly, and pixels where it is 0 are unchanged (§6.7).
2. Changing only an entry's opacity never changes which fragments its threshold drops (§6.7
   step 4).
3. An entry's clip chain has at most 8 items after merging, and with no rotations it is
   equivalent to one integer scissor (§6.8).
4. A canvas texel outside the stored part never contributes colour (§6.7 step 2).
5. Within a frame, operations on UI textures take effect in call order (§6.3).
6. No input to the UI pass panics the renderer. Each refusal is counted in
   `FrameStats::ui_entries_refused` (§6.5).
7. The software cursor is never in a screenshot and never drawn during the pointer lock
   (§6.11).

<a id="s6-16"></a>
### 6.16 Open items and indicative measurements

> **OPEN:** The composition blend and the premultiplication (UL §9.6.1 OPEN). `UI_BLEND_COLOR`
> and `UI_BLEND_ALPHA` are named constants; owner observation 3 of UR (a frame capture of a
> semi-transparent panel edge) settles them.

> **OPEN:** The scale filter and mips (FO Q3; UR §2.2). Bilinear on premultiplied texels with
> no mips is the Tessera default, switchable with `ui_scale_filter`. Owner observation 1 of UR
> (the chat window at UI scale 1.0, 1.5 and 0.5) settles it; a premultiplied mip chain per
> reduced surface is the change if it shows smoothing.

> **OPEN:** Rotation direction, order and projection (LPA §13.20 OPEN), including how the
> rotations of nested surfaces combine (§6.6 chooses 3-D composition with one projection at
> the end), and the marker resampling (GP §8.6.3). The owner's rotation test plugin of LPA
> §13.20 and owner observation 7 of UR settle them.

> **OPEN:** Nested-surface composition (LPA §13.18 OPEN (1), §13.19 OPEN): opacity "own only",
> the clip by the windows above the nearest one (UR spec question 12), and the clip by mode-0
> child windows (R5). R5 is applied here as a Tessera rule, following LPA §13.18's interim rule
> and invariant 60. Decision 0015 should record it at its next revision, together with the
> corrected P1 checks 2 and 4 (§6.14) and the threshold order (R4). Owner observation 4 of UR
> (the nested-surface test plugin) settles the behaviour.

> **OPEN:** The original's external-content format and alpha convention (UW §12 OPEN). Tessera's
> side is decided: a portrait pass is premultiplied (§6.10 "Alpha"), source-over draws blend
> alpha with `One`/`OneMinusSrcAlpha`, and every other fixed blend (additive, multiply and the
> rest) keeps the destination alpha except the two cases of §6.10 "Fixed blends" (R12, answered
> 2026-10-09; this sentence replaced the earlier open question on additive and multiply
> blends). What stays open is how the original clears a portrait's target and composes it into
> its parent surface, and its non-D3D9 paths: §6.10 "Client portrait path" OPEN (a) to (c).
> None of them changes the renderer's contract.

> **OPEN (not renderer work, listed so it is not lost):** what decision 0015 leaves to M5
> outside this file:
> - the coverage region of an entry with its own rotation whose nested entries turn with it
>   (UR must-fix 2, `tessera-ui`);
> - the order in which jobs share the 4 ms paint slice and the 8 MiB upload cap, and the
>   bounds derived from them;
> - a way out when a back texture waits on U5 indefinitely;
> - group visibility while a root's first paint is withheld;
> - in-place switches between stand-in and real surfaces;
> - how tooltip shows count in U1.
>
> None of these changes the renderer's contract above.

> **OPEN:** U2 (UI pass GPU time ≤ 0.3 ms at 3840 × 2160) has no derivation (UR open review).
> The scripted run's screen-sized plugin window with a full-size background is about 8.3 Mpx
> of blended quad, roughly 100 MB of memory traffic per frame, which alone approaches the
> budget on the RTX 2070 Super Max-Q. `FrameStats::ui_gpu_ns` reports the UI pass's GPU time
> from timestamp queries. It is reported for the most recent frame whose queries have
> resolved, so it can lag by a few frames, and it is `None` where the adapter lacks
> `TIMESTAMP_QUERY`, in which case U2 is not a gate (UR §6). One permitted optimisation is
> exact: a `Fit` entry's quad may be shrunk to its stored part, grown by one canvas texel for
> filtered entries, because the other fragments add premultiplied zero.

**Indicative measurements.** None yet: the UI pass does not exist on this branch. U2 and the
`FrameStats` UI counters are measured by UR §6's scripted run once it does. The results are
recorded as indicative, to be measured again on an idle reference machine
(`docs/perf/reference-machine.md`, decision 0010).

<a id="s6-17"></a>
### 6.17 Implementation notes (M5, clean-room implementer)

Status of `crates/render/src/ui/` against this section. These are Tessera choices made while
implementing; they add no facts about the game.

| Topic | As implemented | Where |
|-------|----------------|-------|
| API | `Renderer::create_ui_texture`, `update_texture_rect`, `copy_texture_rect`, `clear_texture_rect`, `flush_ui_ops`, `ui_texture_bytes`, `is_ui_texture`, `ui_view_mode`, `set_ui_linear_fallback` (forces the §6.4 fallback, for tests); `Frame::ui_pass` → `UiPass::entry(&UiEntry)`, `UiPass::cap_counters`; `Frame::software_cursor`; `Pass::premultiplied_alpha` (§6.10: clears to transparent black; every built-in blend state already blends alpha with `One`/`OneMinusSrcAlpha`, and a unit test in `ui/api.rs` fails when a pipeline with any other blend state is added, so the option must be extended first); `ui_pending_ops` and `ui_staging_bytes` (queue length and retained staging memory). The older `update_texture` refuses UI textures (they take B, G, R, A through `update_texture_rect`) | `ui/api.rs`, `ui/mod.rs`, `renderer.rs` |
| Composition input | `UiEntry` borrows its rotations and clip items as slices; `UiPass` copies them into flat arrays, so a frame does not allocate per entry. `tessera-render` does not depend on `tessera-ui` | `ui/mod.rs` |
| Statistics | The counters of §6.3 and §6.5 are fields of one struct, `FrameStats::ui` (`UiStats`: `upload_bytes`, `upload_rects`, `gpu_copies`, `copy_bytes`, `clears`, `quads`, `surfaces`, `entries_refused`, `entries_sanitised`, `texture_bytes`, `caps`, `gpu_ns`, `cursor_drawn`). Upload, copy and clear counters cover the calls since the previous `end_frame`. `entries_sanitised` counts entries accepted after replacing a NaN opacity or threshold or a non-finite angle, drawn or not: a NaN opacity becomes 0, so that entry is counted but draws nothing (T15 "sanitised … and counted") | `ui/mod.rs` |
| Ordering of UI texture operations | Uploads are staged (rows padded to 256 bytes) and recorded with copies and clears in one encoder, submitted before the frame's own command buffer by `end_frame`, by `flush_ui_ops`, before `destroy_texture`, before `update_texture` (its `write_texture` then lands in a later submission), when 32 MiB are staged (a larger rectangle is split into row bands) and when 4,096 operations are pending (a caller painting while no frame is ended cannot grow the queue without bound). Consecutive clears of one texture share one render pass. Staging memory above 8 MiB (U3's per-frame cap) is released after each submission, on the CPU and the GPU; allocations run in error scopes, so a failed one is retried rather than kept | `ui/ops.rs`, `renderer.rs` |
| Shader | Entry data in a dynamic-offset uniform block (496 bytes); five user inter-stage variables (the box position and the eight clip positions, two per `vec4`); clip maps, corners and the 1 : 1 decision computed on the CPU in `f64`. An entry with no rotation uses the exact pixel centre instead of the interpolated position; its `Nearest` texel is computed exactly, ⌊(2(*X* − *x*) + 1)·*cw* ÷ 2*w*⌋ in integers for `Fit` and with a one-step correction of the `f32` estimate for `Tile`, so a *c* that lands on an integer (2 : 1, UI scale 1.5 or 0.75) never reads the previous texel on any driver | `shaders/ui.wgsl`, `ui/geometry.rs`, `ui/state.rs` |
| Rotations | A rotation whose three angles are within 0.0002 of 0 (after a non-finite angle became 0) is kept in the list as the identity, so the space indices of the clip items do not shift; it counts as "no rotation" for the 1 : 1 path | `ui/geometry.rs` |
| Scissor | Clip items whose space index is followed only by identity rotations test the screen position and are folded into one scissor, which is also cut to the projected quad's bounding box and the target (exact: those fragments add nothing) | `ui/geometry.rs`, `ui/state.rs` |
| Skipped draws | Entries with zero projected area, an empty clip intersection, an empty scissor or opacity 0 are valid but not drawn and not counted in `quads` | `ui/state.rs` |
| Hostile-input bounds (Tessera) | Displayed-box sides and coordinates at most 2^24 (`MAX_UI_COORD`), Tile node scale within 1e−4 … 1e4 (`UI_NODE_SCALE_RANGE`), rotation centres finite and at most 2^24; entries outside are refused as `Dest`, `NodeScale` or `Rotations`. The frame-wide entry cap counts the entries of the frame's earlier UI passes when an entry is recorded, so an entry past it is never stored | `ui/geometry.rs`, `ui/mod.rs` |
| First pass | `Frame::ui_pass` on a frame with no pass yet records an empty screen pass first, so the frame still clears to the renderer's clear colour before the UI is drawn | `ui/api.rs` |
| Software cursor | Drawn whenever `Frame::software_cursor` was called, after every pass, if its texture is a UI texture (a registered target would be premultiplied twice and is refused as not UI content); a cursor whose texture is refused is not drawn and gives a once-per-kind warning, but is not counted in `entries_refused` (it is not an entry) | `ui/state.rs` |
| Tests | T1–T15 and the renderer side of T16–T18 and of the P1 checks (§6.14 checks 1–3, and check 4's "turning uploads nothing") as read-back tests in `tests/ui_pass.rs`; golden images in `tests/ui_golden.rs` (Tile, clip, clip chain with a mode-0 child window, external content, round minimap, a P1-shaped radar, filters/threshold/opacity/rotations), rendered into `Rgba8Unorm` targets so that they are the same with and without the non-sRGB view; real surfaces of the install through invariant 1 in `tests/ui_real_data.rs`. Verified on NVIDIA Vulkan, Mesa lavapipe (stored view) and Mesa llvmpipe GL (the linear fallback: T12 round-trips all 256 values; T1 and T10 then check only alpha 0 and 255 on sRGB targets) | `crates/render/tests/ui_*.rs` |

> **OPEN:** `UiStats::gpu_ns` (U2's timestamp queries) is always `None`: the device is
> created with no optional features (`context.rs` requests `Features::empty()`), so
> `TIMESTAMP_QUERY` is never enabled even where the adapter has it. Requesting it when the
> adapter offers it, and resolving one query pair around the UI passes, is a small
> follow-up in `context.rs` (not owned by the UI implementation).

> **OPEN:** §6.5 does not say what a non-finite or out-of-range **rotation centre** does.
> Tessera refuses the entry (`UiRefusal::Rotations`); a spec rule (for example "treated as
> the displayed box's centre") would replace this.

> **OPEN:** §6.5's limits name no bound on the displayed box or the Tile node scale beyond
> "≥ 1" and "finite and > 0". The Tessera bounds in the table above keep every position
> exact in `f32` and every reciprocal finite; a spec bound would replace them.

> **OPEN (implementer, 2026-10-09, §6.10 "Fixed blends", R12; engineering choice for the
> reviewer to confirm or replace):** R12 is implemented in `crates/render`
> (`FixedBlend::alpha_rule` and `FixedBlend::pipeline` in `material.rs`; the lit, instanced
> and sky pipelines key on the result). `Pass::premultiplied_alpha` now marks the pass as
> well as clearing it. Exception 1 blends alpha with `wgpu::BlendComponent::OVER`. Exception
> 2 cannot write alpha 1 through a blend state alone (every alpha factor scales the
> fragment's alpha or the destination's, which may both be 0), and such a blend never reads
> the destination, so its pipeline draws **without blending**: the fragment writes alpha 1
> and the colour times the source factor (0, 1, *a* or 1 − *a*), selected by the WGSL
> pipeline-overridable constant `FIXED_OPAQUE` in `frame_shading.wgsl` (default 0, no
> change). Before it applies the factor, the fragment clamps its colour and alpha to
> [0, 1], as the blend unit of a `Unorm` target clamps the source before applying blend
> factors (lit colour can exceed 1 and is not clamped on `Rgba8Unorm`). Without that clamp
> the first version of this path (commit 820391d) stored *a* × colour where the blend
> stores *a* × clamp(colour), for SRCALPHA / ZERO and INVSRCALPHA / ZERO (corrected
> 2026-10-09 after a clean verifier's finding). With the clamp the stored colour equals the
> blended one up to rounding, also for over-bright colour. Outside premultiplied passes
> the pipelines are unchanged. The guard test in `ui/api.rs` no longer exempts
> `FixedBlend`: it evaluates every fixed blend (all factor and operation pairs) with and
> without the option over sampled source and destination values (source colour and alpha
> above 1 included) against R12 and the blend's colour, and still fails on any other blend state. The "API" row of the table above
> predates R12 in its description of `Pass::premultiplied_alpha`.

**Indicative measurements (2026-10-07, development machine under load from other builds,
load average about 9–11; not the idle reference machine).** `tests/ui_perf.rs` (release,
`--ignored`): a 3840 × 2160 `Rgba8Srgb` target, 46 entries (one screen-sized 1 : 1
background, P10-, P7- and P1-sized panels, a rotated window with a clipped stretched child,
a Tile strip), 60 frames each waited for on the GPU. NVIDIA RTX 2070 Super Max-Q (Vulkan):
wall-clock time per frame 0.07–0.11 ms without the UI pass and 0.55–0.75 ms with it, so
**0.47–0.68 ms** as an upper bound of the UI pass (it includes submission and wait latency
and the GPU's low clocks between waited frames); `end_frame` CPU time 0.03 ms → 0.10 ms.
This is above U2's 0.3 ms but is not a GPU-time figure (no timestamp queries, OPEN above);
the memory-traffic estimate of the U2 OPEN above (about 100 MB per frame for the
screen-sized background alone) is the expected floor. To be measured again with timestamp
queries on the idle reference machine (decision 0010).

<a id="s7"></a>
## 7. Client facts §4–§5 rely on, and their provenance (spec writer, dirty room, 2026-10-07)

Every fact about the original client that the code of §4–§5 relies on, restated with its
source. Marks: **BIN** = read from the client (decision 0011); **Confirmed** = observed in
the user's install; **Spec** = taken from the cited spec, whose own provenance applies.
"Re-checked" means this spec writer verified it again on 2026-10-07 in a private copy of the
Ghidra project of the installed client (`x64/lotroclient64.exe`, SHA-256 prefix
d22363eb9aff0fb5, the build of `walker.md` §11.1), or with an own read-only probe over all
49,636 landblock-info records. **BIN questions** (the same session as `walker.md` §14.13):
(k) where the fog limits stand relative to the interior override; (l) how the light-list
builder applies the parent switches to a light without a parent, which objects it considers,
and how it treats baked-landscape lights; (m) whether baked-landscape lights are also drawn
into a chunk's baked-lighting texture, with which pass code and target size. **Why M1–M3 were
not enough:** these are run-time rules with no trace in the data; no running client or frame
capture was available; and the specs under revision disagreed on (k) and (l). Only offline
rendering and environment code was read; no network, login, launcher or anti-tamper code.

| Row | Fact | Used in | Source | Mark |
|---|---|---|---|---|
| R-E1 | Lighting equations of the lit base pass: ambient and hemispheric terms, main light with *N* · *L₀*, normal-map sign *N* = *n*.x **T** − *n*.y **B** + *n*.z **N₀**, no specular below power 0.5, emissive added before fog, output alpha and the alpha test before vertex alpha, time input modulo 3,600 | §4.1–§4.3 | `lighting.md` §4–§5, `material.md` §5.3.1, §11 | Spec (BIN) |
| R-E2 | At most 8 lights per object in all, the main light included | §4.2, §4.3 | `lighting.md` §8.5 | Spec (BIN) |
| R-E3 | Light record of a placed light: colour = Diffuse × clamp(DiffuseIntensity(*t*) × *σ*, 0, 1); specular = Specular × clamp(2 × *σ* × DiffuseIntensity(*t*), 0, 1), with the Diffuse colour in place of a white Specular (within 0.0002 per channel) when KeyLight is off; range *R* = max(Range, 0.0002), uploaded as *R²*; attenuation *k* = 10 × Falloff / *R²*; the field defaults (Range 25, Diffuse and Specular white, intensity 0.5, Falloff 1, *σ* 1); attenuation 1 / (1 + *k* *d²*) inside *R*, nothing at or beyond it, with the smoothstep fade above 0.75 *R²* for the newer back end's light 0 and per-pixel lists | §4.2 (the light table: the record built from a placed light's fields, the range floor of every uploaded point light; the CPU reference of the frame functions) | `lighting.md` §8.3 (attenuation, light record, L19) | Spec (BIN) |
| R-E4 | The scene has no main light while the environment state's sun-present flag is false: on the indoor record always, on the outdoor record while the blended keyframe `effect` is not null, the sun colour is opaque black or the sun brightness is below 0.0002; then one submission path adds the default directional light of `lighting.md` §8.5 (its slot a Tessera Decision there), and the fake light takes its default (E18) | §5.3 (fake light; the caller decides) | `environment.md` §11.7 step 9; `lighting.md` §8.1, §8.5, E18; `walker.md` §14.13 rows W-E10, W-E11 | Spec (BIN-E, Accepted); re-checked 2026-10-07 for `walker.md` |
| R-L1 | Selection radius ρ = Range × (1 − (1 − 0.85) × min(Falloff, 1)), Range for `Light_GlobalLight` (0.85 a client constant, decision 0012) | §4.3 | `lighting.md` §8.5 step 1 | Spec (BIN) |
| R-L2 | A light reaches an object when the squared distance from the light's position to the object's world-space axis-aligned box (0 inside) is below ρ², strict; key lights are inserted at the front, the others appended; the never-parent switch is tested for the light's parent, the only-parent switch for every other object whether or not the light has a parent (one special light record's parent is looked up separately, not traced). No Light SED of this install's landblock infos sets `Light_OnlyInfluenceParent` true (136 store false) | §4.3 | `lighting.md` §8.5 steps 2–4, row L34; census own probe | BIN, re-checked 2026-10-07; census Confirmed 2026-10-07 |
| R-L3 | Only objects drawn in the current frame are candidates; objects with any of three scene flags are skipped | §4.3 | `lighting.md` §8.5 step 2, row L34 | BIN, re-checked 2026-10-07 |
| R-L6 | The client builds a light list **for each** drawn object, against that object's own world box; no list is shared between objects | §4.3, §5.5 (per-instance lists) | `lighting.md` §8.5, row L32 | Spec (BIN) |
| R-L4 | A `Light_BakedLandscapeLight` light is skipped for the object kind that terrain and ceiling chunks are created as, light map or not (L30) | §4.3, §5.2 | `lighting.md` §8.3, §8.5 step 3 | BIN, re-checked 2026-10-07 |
| R-L5 | The client's light-map bake draws each baked-landscape light that reaches a chunk with pass code 0x41 into an off-screen target of at most 1,024 × 1,024 (smaller when the device's limit is) and sums the results into the chunk's baked-lighting texture; it runs only after a re-bake flag that a special client mode raises (Inferred: not in normal play) | §5.2 | `lighting.md` §8.6, rows L35, L36 | BIN; the 0x41 draw and the target size re-checked 2026-10-07, the trigger as `lighting.md` reports |
| R-F1 | Fog: per-vertex factor, base pass lerps to the fog colour, additive passes × (1 − *f*); limits: the keyframe's end capped at 10,000 first, an interior override of the end not capped, then end ≥ 20, start in −10,000 … end − 20 | §4.2 (the renderer's lower limits; the cap is the caller's), §4.3 | `lighting.md` §10, `landblock.md` §4.13 | Spec (BIN); the order of cap and override re-checked 2026-10-07 |
| R-F2 | The interior fog ends of this install lie in 0–7,000 (775 cells, 540 landblock-info sets) | §4.2 (former departure R-D2) | own probe | Confirmed 2026-10-07 |
| R-SH | Sun shadow: light clip matrix, split map, one or five taps (0.36 / 0.16), shadow colour = landscape colour + range × *s*, fade *φ*, factor 1 + *φ* (colour − 1); engine values OPEN | §4.4 | `lighting.md` §9 | Spec (BIN) |
| R-T1 | Terrain layers: opacity rules, discard at τ except the first layer, "over" blending, the §8.1 switches, mask row 0 in the south, light map added to Σ, highlight not × *N* · *L*, the albedo or 2 | §5.2 | `terrain-texturing.md` §5, §7–§10 (§9.4, T44) | Spec (BIN) |
| R-T2 | Layers without a normal map take the stored vertex normal unnormalised for the hemispheric term and the sun's *N* · *L₀*, normalised for the point-light sum; the stored normal is the average of the unit triangle normals (shorter than 1 at creases), downward for ceilings | §5.2 | `terrain-texturing.md` §4.8, §4.10 | Spec (BIN (s), BIN-S) |
| R-W1 | Water: waves Σ *A*ₖ sin *s*ₖ on the vertex's **model** x, y; normal layers; refraction, shoreline weight, Fresnel, specular and sparkle; no table light, only the fake light | §5.3 | `water.md` §8.2–§8.4, §9 | Spec (BIN) |
| R-S1 | Sky object rotation: first about +Z by −*B*, then about +Y (north) by −*A* | §5.4 (placement) | `environment.md` §11.4 step 5 | Spec (BIN-E, Accepted) |
| R-S2 | With fog on and `fog_extra` > 0.99 the sky objects are not drawn; each object's colour and opacity are `Render_Color` and `Render_Opacity` from the keyframe tint; whether lower draw numbers draw first is OPEN | §5.4 | `environment.md` §11.4, §11.7 step 10, §12 | Spec (BIN-E) |
| R-FR | Frill groups: radius, *D* and *F* (level 2: 10 sectors, 97, 30), reduction *g* × max(1, trunc((1 − *f*) *n* / *g*)), fade-in, the sway weight from the stored box | §5.5 | `scenery.md` §8.3–§8.5 | Spec (BIN) |
| R-M1 | Mip chains: one level per stored level, level 0 largest, a level of the wrong size copied clipped | §4.6 | `texture-list.md` §4.2–§5.5 | Spec |
| R-M2 | Mesh records store a binormal (the +V direction) beside the tangent, not orthogonalised against the normal and with a handedness that varies within a mesh; the lit programs use the stored tangent and binormal as **T** and **B** | §4.1 (tangent-frame Decision, departure R-D10) | `mesh.md` §4.5, §5.6 (rows M4, M5, M16); `lighting.md` §5.2.4 (L6) | Spec (Confirmed; BIN) |
| R-M3 | Terrain masks and light maps are single 0x41 render surfaces, each one image level | §5.2 (mask and light-map sampler) | `terrain-texturing.md` §5.1, §10; `records.md` §5 | Spec (Confirmed) |
| R-A1 | Alpha test of a surface: on when the shader declares 4004, its value is 1 and an opacity source exists; threshold 1043; discard when opacity ≤ threshold. The opacity a fixed-function factor of an unlit record reads is the §11.2 opacity source, or 1 without one | §4.3 (`Fixed`, departure R-D11) | `material.md` §11.2, §11.3 | Spec (Confirmed; the factor rule part of a Tessera Decision there) |
| R-A2 | The client multiplies the material colour 2000 and the specular colour 2001 by the same per-object tint before upload | §5.5 (instance tint × *M* and *Ms*) | `material.md` §8.3 (BIN-T (f), MA32) | Spec (BIN) |
| R-L7 | A placed light's record is a point light unless its mode says directional (the main light's); the direction has no property. The per-vertex lists hold one extra directional light (count 0 or 1); a per-pixel key light becomes light 0 of its kind | §4.3 (directional table lights) | `lighting.md` §5.6, §8.2, §8.3, §8.5 step 2 | Spec (BIN) |

**Departures, in one place** (each is labelled where it occurs; state of the code on
2026-10-08):

| Row | Where | The renderer | The client | Resolution |
|---|---|---|---|---|
| R-D1 | §4.3 unattached lights | (withdrawn 2026-10-08) the only-parent switch is now tested for every non-parent draw, attached or not | the same (R-L2) | No longer a departure (row kept for stable numbering) |
| R-D2 | §4.2 fog | (withdrawn 2026-10-08) only the lower limits; the 10,000 cap of the keyframe's end is the caller's | caps only the keyframe's end, before the override (R-F1) | No longer a departure (row kept); the walker applies the cap in the client's order (`walker.md` §14.3) |
| R-D3 | §4.1 per-pixel lists | every table light per pixel with the smooth fade and specular | per-vertex lists with the hard cut-off, no specular | Decision |
| R-D4 | §4.4 shadow | five taps, fade over the last fifth, fitted map, depth bias | engine values OPEN; `lighting.md` §9 simple form (one tap, fade to 0) with the variants it lists, these among them | No longer a departure: within the `lighting.md` §9 Decision as that spec now words it (row kept so the numbers stay stable) |
| R-D5 | §5.3 waves | world x, y of the pass | model x, y | Decision |
| R-D6 | §5.4 sky placement | (withdrawn 2026-10-08) R_Y(−*A*) · R_Z(−*B*) | the same (R-S1) | No longer a departure (row kept) |
| R-D7 | §5.4 sky shading | unlit, unfogged | lighting unstated, fog OPEN; `fog_extra` > 0.99 hides the sky (caller) | Decision |
| R-D8 | §5.1 translucent order | blended instances, then water | not specified | Decision; OPEN for the implementer (§5.7) |
| R-D9 | §4.3, §5.5 light lists of instance batches | (withdrawn 2026-10-08) one selection per instance against its own box | one list per object (R-L6) | No longer a departure (row kept) |
| R-D10 | §4.1 tangent frame | **B** = *w* × cross(**N₀**, **T**), normalised per pixel | the stored binormal (R-M2) | OPEN for the render and mesh implementers (§4.8) |
| R-D11 | §4.3 `Fixed` blends | no alpha test | alpha test by `material.md` §11.2 when the record has it on (R-A1) | OPEN for the implementer (§4.8) |

<a id="s8"></a>
## 8. Change log

> This change log was §6 (anchor `s6`) until the merge of 2026-10-08; it moved to §8 because
> §6 was already the UI pass, which code and decision 0015 cite. Its rows are unchanged.

| Date | Change |
|---|---|
| 2026-10-04 | §1–§3: M2 design (meshes, materials, shared buffers, pipelines) and open questions (clean-room implementer). (Row added 2026-10-07.) |
| 2026-10-05 | §2.4: M4 skinned meshes (clean-room implementer). (Row added 2026-10-07.) |
| 2026-10-06 | §4: lit pipelines on one frame uniform (one base pass looping over the lights, light selection, sun shadow, fog, the alpha modes `TestBlend` and `Additive`, mip-chain upload); §5: render-world pipelines (terrain layers, water with the scene copy, sky objects, instanced scenery and frills, their tests) (clean-room implementer, M4.5). (Row added 2026-10-07.) |
| 2026-10-06 | M4.5 exit verification (clean-room implementer): `WorldSurface` takes the gloss from a normal-and-gloss texture's alpha (`gloss_from_normal_alpha`, §5.5), which removed row F2's near-white frills. (Row added 2026-10-07.) |
| 2026-10-07 | §4.3, §5.2 (implementer, `tessera-render`): automatic light selection follows the revised `lighting.md` §8.5 (selection radius ρ, squared distance to the world box, key lights first, parent switches; `Pass::light_reach`, `LightReach`, `MeshDraw::light_object`, `TerrainChunkInfo::aabb`) and L30 (baked-landscape lights never reach terrain or ceiling chunks under `Auto`); terrain layers without a normal map shade the ambient and sun terms with the stored, unnormalised vertex normal (`terrain-texturing.md` §4.10), and the terrain normal matrix keeps the normal's length under a uniform scale. New tests: `stored_normals_shade_as_they_are`, `specular_is_a_sheen_scaled_by_the_gloss`, `baked_lights_skip_terrain_chunks` (GPU) and the selector's unit tests. No golden image changed. Earlier §4–§5 additions (M4.5) are listed in the ROADMAP M4.5 status. |
| 2026-10-07 | Spec writer (dirty room), answering the first review and the re-review of §4–§5 and incident 0018 (decision 0018, owner decision A): header review record shortened (details moved to the dirty-room review log); §3: the mip-chain, green-channel and per-draw-cost OPENs marked answered; §4.1 per-pixel lists labelled a Decision that departs from `lighting.md` §5.6, §8.3; §4.2 fog row names the limits; §4.3: the entry-less light and the `List` behaviour labelled Decisions, the unattached-light Decision marked as departing from the client (BIN, row R-L2) with an OPEN, the world-box and directional Decisions tied to the `lighting.md` §8.5 OPEN; §4.4 heading and text: departs from the simple-form Decision (five taps, fade over the last fifth, depth bias); §4.6 stale sentence replaced; §5.1 Stages corrected to the built order (blended instances, then water) with its consequences, labelled a Decision; §5.2: the baked-landscape light row (no bake, `lighting.md` §8.6 Decision; row R-L5) and the stored-normal row's assumption about its input; §5.3 wave frame labelled a Decision that departs from `water.md` §8.3; §5.4 placement marked as departing from `environment.md` §11.4 step 5 (BIN-E), shading labelled this file's Decision with the `fog_extra` fact, draw order stated as assumed; §5.5 frill Decisions labelled; §5.7 OPENs updated and two new implementer OPENs (sky rotation, water order); new §7 (client facts with provenance; table of departures). Status of §4–§5: Review. |
| 2026-10-07 | Spec writer (dirty room), second revision answering the re-review of §4–§5 and §7 (changes requested): §4.3 the world-box Decision split in two: the box rule stays tied to `lighting.md` §8.5 OPEN (b); treating an instance batch as one light-list object is a new Decision labelled as departing from the client's per-object lists (L32), with its effect and an implementer OPEN; §5.5 Batches row points to it; §7: new rows R-E3 (the light record and attenuation of `lighting.md` §8.3: range floor 0.0002, colour, specular and attenuation rules, used by the light-record builder, the range floor of uploaded point lights and the CPU reference) and R-L6 (per-object lists), new departure R-D9. Status of §4–§5 and §7: Review. |
| 2026-10-07 | Spec writer (dirty room), third revision after the acceptance, applying the two non-blocking notes of that review; no fact about the renderer's behaviour changed: §4.4 heading and text say the renderer's shadow lies within the `lighting.md` §9 simple-form Decision, using the variants that Decision now lists (five taps, later fade, fitted map, depth bias, outside lit), and which values are Tessera's; row R-D4 marked as no longer a departure (kept for stable numbering); §5.3 fake-light row: whether a pass has a main light is the caller's, with the client's sun-present rule and a pointer to the walker's departure (`walker.md` §14.3); new row R-E4 for that rule. Status of §4–§5 and §7: Accepted → Revised. |
| 2026-10-08 | Spec writer (dirty room), fourth revision of §4–§5 and §7 answering the re-review of 2026-10-07 (changes requested: R1 body describing former code, R2 implementer choices only in OPENs; with `walker.md` B2, the fog contradiction), checked against the code of the remediation worktree: §4.1 the 7-or-8 light count with light 0 restated (default directional light, key directional light), lit surfaces always fogged and `LitSurface::fog` for unlit ones only, new tangent-frame Decision departing from the stored binormal of `lighting.md` §5.2.4 (R-D10; mesh records store one, `mesh.md` §5.6); §4.2 fog row: the renderer applies only the lower limits, the 10,000 keyframe cap is the caller's (former departure R-D2 withdrawn, consistent with `walker.md` §14.3); §4.3: new `Fixed` alpha mode (factors, operations, opacity rule, Decisions, fog Decision, no alpha test as departure R-D11), `Additive` as its shorthand, the parent switches as the client (R-D1 withdrawn), per-instance light lists (R-D9 withdrawn), directional table lights and malformed light fields as Decisions (no placed light is directional, row R-L7), draw-uniform bit 16; §4.5 `Fixed` among the blended draws; §4.8 the fog OPENs replaced by OPENs for the binormal stream and the `Fixed` alpha test; §5.2 mask and light-map sampler (nearest mip, no effect: single-level surfaces, R-M3) and terrain shadows drawn once per triangle as Decisions; §5.3 water exponent clamps and the sparkle rule as a Decision; §5.4 placement by the client's rotation (R-D6 withdrawn), `Fixed` sky surfaces; §5.5 world-model limits, fog option and opacity Decision, instance tint × *Ms* (R-A2), per-instance lists and the 96-byte instance, mirrored instances, the grown frill culling sphere; §5.6 sky test note; §5.7 answered and folded implementer OPENs removed; §7 rows R-M2, R-M3, R-A1, R-A2, R-L7, "Used in" columns updated, departures R-D1, R-D2, R-D6, R-D9 withdrawn (rows kept), R-D10, R-D11 added. No section number or anchor changed. Status of §4–§5 and §7: Revised (Accepted text changed). |
| 2026-10-08 | Merge of the M5 branch (spec writer, dirty room): new §6, UI pass (M5; Tessera design from the accepted decisions 0014 and 0015, written 2026-10-06, reviewed and **Accepted** 2026-10-08, record in its status paragraph), placed after §5 and before §7; it does not change §4, §5 or §7. The change log, numbered §6 on the M4.5 side, becomes §8 (anchor `s8`) because the two sides had both used §6; no row or other section changed. Header rows extended for §6. |
| 2026-10-09 | Merge of main into remediation-2: main's §6 (UI pass, M5, Accepted 2026-10-08) and the change log's move to §8 combined with the fourth revision of §4–§5 and §7 (2026-10-08, Accepted); header combined; §6.4 "second OPEN of §4.8" now names the last OPEN of §4.8 (blending on sRGB targets), whose position changed in the fourth revision. No fact changed. |
| 2026-10-09 | Spec writer (dirty room): §6.10 implementer OPEN (alpha of fixed blend states in premultiplied passes) answered. New Tessera Decision "Fixed blends" (R12): fixed blends keep the destination alpha in premultiplied passes, except SRCALPHA / INVSRCALPHA ADD (source-over rule) and destination ZERO with a non-destination source factor, ADD (opaque, alpha 1); effects table and testable consequences added; the guard test's `FixedBlend` exception is replaced by that rule. New "Client portrait path" facts (R10, BIN, M4 under decision 0011: build 4906.0071.1794.4035) and the portrait data census (R11, OBS, Confirmed), including the known difference that the original shows the background colour inside the opacity mask. "Alpha" row of §6.10 points to the rule. §6 status Accepted → Revised; review pending. No other section changed. |
| 2026-10-09 | Spec writer (dirty room), second revision of §6 answering the review of the first (changes requested; three blocking items). (1) §6.16: the OPEN on what additive and multiply blends write to alpha is replaced by the decided rule (§6.10 "Fixed blends", R12) and an OPEN limited to the original's clear, composition and non-D3D9 paths. (2) §6 "What this section is": names R11 (OBS) and R10 (BIN, D3D9 path only) instead of saying that only R7 was re-checked and nothing comes from disassembly. (3) §6.10 "Client portrait path": scoped to the client's D3D9 path; the untraced clear, the parent surface's composition and the D3D10/D3D11/OpenGL paths are now `> **OPEN:**` callouts (a)–(c), pointing to the `ui-widgets.md` §12 OPEN; the "Consequence" bullet becomes an expected result that depends on OPEN (a) and (b); the answered implementer callout, the effects table and "Why this rule" say D3D9. Non-blocking: R11 counts six properties and notes the explicit background colour; exception 2 states ZERO / ZERO draws opaque black (intended); R10 names an M3 frame capture as the check still wanted. §6 status paragraph and header rows updated. No Tessera rule and no client fact changed. §6 stays Revised; review pending. |
| 2026-10-09 | Merge of main into m5-followups: the §6.10 fixed-blend rule (R10–R12, §6 Accepted 2026-10-09) and the §6.17 implementer OPEN combined with remediation rounds 2–3 (fourth revision of §4–§5 and §7, Accepted 2026-10-08: binormal stream of §4.8, 48-byte vertex, unlit fog switch); header combined; the §4.3 `Fixed` row now points to §6.10 "Fixed blends" for the alpha component in premultiplied passes (wording only). No fact changed. |
