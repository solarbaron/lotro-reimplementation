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

<a id="s6"></a>
## 6. UI pass (M5)

Status: **Accepted** (§6 only). Review record: Claude review session (spec-writer agent,
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
decisions. The few facts about the game data are cited by spec section only (`game-panels.md`
G39–G41, `ui-layout.md` U46), and one was re-checked for this section (row R7). Nothing here
comes from disassembly. Names of types and functions are suggestions for the implementer; the
behaviour is the contract.

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
| Other passes | Unchanged. World passes, the walker overlays and the developer overlay keep the sRGB view. Under the M4.5 Decision of §4.1 the lit passes blend decoded values on sRGB targets. The second OPEN of §4.8 suggests rendering into a `Unorm` target for an exact match; this section adopts that fix **for the UI pass only**, and §4.1 and §4.8 stay as they are. |
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
| Alpha | **Premultiplied.** The target is cleared to (0, 0, 0, 0) in each frame it is drawn. Opaque draws write alpha 1. Source-over draws keep their colour blend and blend alpha with `One`/`OneMinusSrcAlpha`, so colour over the cleared target is premultiplied and empty pixels are transparent. The renderer offers this as an option of a pass drawn into such a target (for example `Pass::premultiplied_alpha()`), which sets the clear and the alpha component of every blend state. |
| sRGB refused | An entry whose texture comes from an `Rgba8Srgb` target is refused (§6.5): it would return decoded values into the non-sRGB view and darken the portrait. The client also refuses to register such a target as UI content, as a categorised failure: the portrait draws nothing. |
| Timing | The target must be drawn earlier in the same frame, or in an earlier submission, than the UI pass that reads it; otherwise the entry shows its previous content. An entry that reads the frame's own colour target is refused (no feedback loop). |
| M5 scope | The `UICore_Viewport_RenderToTexture…` properties are recorded but not used; Browser and Movie items stay empty (UW §12). |

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

> **OPEN:** The external-content format and alpha convention (UW §12 OPEN). This section
> requires source-over draws in a portrait pass to blend alpha with `One`/`OneMinusSrcAlpha`.
> What other blend modes (additive, multiply) must write to alpha in such a pass is not
> decided, and no M5 portrait is known to use one.

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
