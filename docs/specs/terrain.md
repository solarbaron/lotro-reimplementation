<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Terrain meshing, LOD, normals and shading (`crates/terrain`)

| | |
|---|---|
| **Status** | Accepted (review 2026-10-08 of the 2026-10-08 revision; changes in §0, §1, §5, §8, §9, see §10) |
| **Last updated** | 2026-10-08 |
| **Author role** | §1–§5, §8 and the Tessera Decisions of §6, §7 and §9: Implementer (clean room) of `tessera-terrain`; everything there is a **Tessera Decision**, not a description of the original client. §0, the facts about the original cited in §1 (height rule), §5 (normals), §6 (mask and light-map orientation), §7 (meaning of the splat layers, splatting inputs) and §9: Spec writer (dirty room), revisions of 2026-10-05 and 2026-10-08; each such fact has a row in §0.2. |
| **Reviewer** | Claude review session (spec-writer agent); 2026-10-08 revision: Claude spec-reviewer session (not its writer) |
| **Review record** | Re-review 2026-10-05: **passed**, Accepted. Review 2026-10-08 (2026-10-08 revision): **passed**, Accepted. Full records in the review log (decision 0018). |
| **Implements into** | `crates/terrain`, `fuzz/fuzz_targets/terrain_mesh.rs` |
| **Format facts from** | `landblock.md` §3 (height grid, scale, split diagonal, surface layers, §3.11 height rule), `coordinates.md` §3, §4, §12, `terrain-texturing.md` (layer meaning, masks, texture coordinates, tangent frame, blending, lighting inputs), `collision.md` §6 (terrain and ceiling triangles, holes), `water.md` (water per landblock) |
| **Revised** | 2026-10-05 (three revisions by a spec writer, dirty room); 2026-10-08 (spec writer, dirty room); see §10. **Implementers re-check `crates/terrain` against §1 (interpolated queries, wording only) and §5 (vertex normals now follow the original's rule).** |

This file has two kinds of content, kept apart:

- **Tessera's own design** for terrain (§1–§5, §8, and the rules marked **Tessera Decision**
  in §6, §7 and §9): how the landblock height grid becomes meshes at several levels of
  detail, how seams stay closed, how normals and fallback colours are made, and what the
  walker's height queries return. None of it describes the original client.
- **Facts about the data and the original client** that Tessera's design depends on. They are
  not stated here first: each is cited from the spec that owns it (`landblock.md`,
  `coordinates.md`, `terrain-texturing.md`, `collision.md`, `water.md`), and §0.2 lists them
  with their provenance marks. Some of them come from dirty-room work in those specs,
  including disassembly of the client under decision 0011 (BIN). The 2026-10-08 revision
  adds research of its own (draw distances, the reference landblock, region borders); its
  sources are listed in §0.3.

The open questions are collected in §9.

<a id="s0"></a>
## 0. Provenance

<a id="s0-1"></a>
### 0.1 Marks

**Confirmed** = checked on every record of the kind in the install (date and source in the
owning spec); **BIN** = rests on disassembly of the client (method M4, decision 0011) in the
owning spec; **Tessera Decision** = this project's choice; tests must not treat it as the
original's behaviour. Values observed in one install are read at run time, never hard-coded.

<a id="s0-2"></a>
### 0.2 Facts used, by source

| # | Fact | Used in | Source (owning spec) | Mark |
|---|---|---|---|---|
| TR1 | Height grid 33 × 33 u16, X-major; vertex flags, bit 7 = split diagonal of the quad | §1, §2 | `landblock.md` §3.3, §3.6 | Confirmed |
| TR2 | Height in world units = stored × 2,000 / 65,535, computed in 32-bit floats | §1, §5 | `landblock.md` §3.4 (L4), `terrain-texturing.md` §13 (T23) | BIN |
| TR3 | The rendered and collided surface is the level-0 grid triangulation with the stored diagonals; triangles counter-clockwise from above | §2, §3 | `landblock.md` §3.11; `collision.md` §6.1 (BIN for the client's mesh); verified on real data here (§8) | Confirmed |
| TR4 | Surface-layer index lists use vertex number 33 · j + i | §1, §7 | `landblock.md` §3.8.3; real-data test §8 (458,061,498 triangles) | Confirmed |
| TR5 | Neighbouring blocks store equal edge samples | §1, §5 | `landblock.md` (HM7) | Confirmed |
| TR6 | Layer `material` = key of the region's terrain type table; stored order = draw order; first drawn layer opaque, later ones "over"; `whole` = all 2,048 triangles; opacity per record family | §7, §9 | `terrain-texturing.md` §4, §7, §8 (T5–T9, T19) | Confirmed / BIN as marked there |
| TR7 | Masks and light maps have stored row 0 on the south edge, sampled at (x / 160, y / 160) | §6, §9 | `terrain-texturing.md` §5.2, §10 (T15, T21) | Confirmed |
| TR8 | The original's layer texture coordinates: sets (x, y), (x, h), (y, h) plus a per-block offset relative to a reference landblock, and the mask set (x / 160, y / 160) | §7, §9 | `terrain-texturing.md` §6.1–§6.2 (T16, T36) | BIN |
| TR9 | The original's tangent frame: east axis projected onto the surface, binormal = normal × tangent | §7 | `terrain-texturing.md` §6.4 (T29) | BIN |
| TR10 | The original builds the ceiling chunk with the same vertex builder from the ceiling height grid and a separate ceiling normal array | §9 | `terrain-texturing.md` §4.8 (T33) | BIN |
| TR11 | Ceiling quads split by bit 0 of the vertex-flag byte; ceiling triangles face down; zero ceiling samples are part of the ceiling mesh | §9 | `collision.md` §6.4 (C21, CO10) | Confirmed (stored triangles); BIN (mesh builder) |
| TR12 | Grid triangles in no layer occur only in landblocks with a ceiling and never under an open cave; the original does not draw them but keeps them in collision | §7, §9 | `collision.md` §6.5 (C22, CO11); `terrain-texturing.md` §4.5 (T11) | Confirmed (census); collision BIN |
| TR13 | The original's terrain lighting: ambient, hemispheric and point lights per vertex (hemispheric per pixel on normal-mapped layers), sun per pixel with a shadow factor, light map added | §9 | `terrain-texturing.md` §9–§10 (T22, T32) | BIN |
| TR14 | The 17 × 17 coarse grid is the per-vertex scenery word, not a texture selector | §9 | `landblock.md` §3.7, `scenery.md` §3 (T13) | Confirmed |
| TR15 | Water level of a landblock and water drawing | §9 | `water.md` §6, §8 | as marked there |
| TR16 | The original's vertex normal of a terrain or ceiling sample: the plain mean of the unit normals of the four to eight grid triangles around the sample (stored diagonals), not renormalised; ceiling rule mirrored, normals pointing down | §5, §9 | `landblock.md` §3.14 (L36), `terrain-texturing.md` §4.10 (T42) | BIN |
| TR17 | Across a block edge those triangles use the neighbouring blocks' samples and split flags, the diagonal neighbour at a corner; a triangle with a sample in a block that does not exist (or, for a ceiling, has no ceiling) is left out of the mean | §5, §9 | `landblock.md` §3.14 steps 2 and 4, `terrain-texturing.md` §4.10 "Block edges" | BIN |
| TR18 | Every region with landblock data (1–5 and 14) has a landblock-data record for every (X, Y) with 0 ≤ X, Y ≤ 254 (65,025 per region) and none with X or Y = 255; the only blocks missing an edge or diagonal neighbour are the 1,016 on the region border (X or Y = 0 or 254) | §9 | OBS-T (§0.3) | Confirmed 2026-10-08 (all 390,150 records) |
| TR19 | Distant-landscape sectors exist only for corner landblocks inside 0–254, so nothing of a region is drawn beyond its border | §9 | `distant-landscape.md` §5.1 | BIN |
| TR20 | Far radius *R* (landblocks) from the landscape draw-distance graphics option: levels 0, 1, 2, 3, 4 give 2, 4, 6, 9, 12; any other level gives 6; a console override (off by default, value 6) replaces it. Sector radius *F* = ⌊*R* / 3⌋ + 1. The option's built-in initial level is 3; the six overall graphics presets set levels 0, 1, 2, 3, 4, 4 | §9 | BIN-T (a) (§0.3) | BIN |
| TR21 | The landscape's near radius is 1 landblock (a 3 × 3 grid around the viewer's landblock); it is the near radius *N* of `distant-landscape.md` §5.2 | §9 | BIN-T (b) | BIN |
| TR22 | The reference landblock of the per-block texture offset and its accumulator are held by the distant-landscape sector manager: the reference is the landblock of the viewer position that drives sector loading and the far LOD (`distant-landscape.md` §5.1–§5.2), replaced whenever that position's landblock or region changes | §9 | BIN-T (c); `terrain-texturing.md` §6.2 | BIN |
| TR23 | The world manager's load position each frame is the player character's position by default (a console switch, on by default); with the switch off it is a position supplied from outside (the switch's own description says: the camera's). That this position is the one of TR22 is not traced | §9 | BIN-T (c) | BIN; link to TR22 Inferred |

Everything else in §1–§8 (level-of-detail scheme, error bounds, selector and its defaults,
stitching, the summation order and provisional sides of §5, the fallback palette and block
UVs, memory layout, buffer layout, hostile-input limits) is a **Tessera Decision**.

<a id="s0-3"></a>
### 0.3 Research of the 2026-10-08 revision

| Tag | Source |
|---|---|
| OBS-T | **M1.** Throwaway probe in the dirty scratch area reading the directory of every `client_cell_*.dat` archive of the user's install (read-only, in place) on 2026-10-08: the set of landblock-data DIDs 0x800RXXYY per region, their X and Y ranges and, for every block, which of its eight neighbours has a record. |
| BIN-T | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, SHA-256 prefix d22363eb9aff0fb5 (the build of `terrain-texturing.md` BIN), read on 2026-10-08 with headless Ghidra 12.1.3 on an own copy of the shared project (deleted afterwards). **Questions:** (a) how the client chooses how far terrain is drawn (far radius) from its graphics options; (b) the near radius of the landscape and the size of its landblock grid; (c) which landblock the landscape uses as the reference of the per-block texture offset (`terrain-texturing.md` §6.2 OPEN) and which position drives it. **Read:** the graphics-option registration and the six presets, the far-radius setter of the world manager, the landscape object's initialiser, the sector manager's position update (reference, accumulator, sector discard) and its per-landblock LOD pass, the world manager's per-frame position update and its option registration. **Why M1–M3 were not enough:** radii, the reference choice and the position source are code with no trace in the data; no published source gives them; no running client is available to this session for M3. Nothing about network, login, keys, launcher, patcher or anti-tamper was examined. |

---

<a id="s1"></a>
## 1. Input and frames

- One `TerrainBlock` per landblock, built from the decoded height grid (33 × 33 u16, X-major,
  `landblock.md` §3.3) and the vertex flags (bit 7 = split diagonal of each quad, §3.6). The
  ceiling grid of cave blocks (`landblock.md` §3.5) gets a surface of its own (bit-0
  diagonals, clockwise from above), its normals (§5) and its layer vertex streams (§7), but
  no levels of detail: ceilings are drawn at level 0 from their stored triangles (§7,
  Decision (3)).
- Heights stay as the stored u16 values and are converted with the rule of `landblock.md`
  §3.4 only when a vertex or a query result is produced: height = stored × 2,000 / 65,535,
  computed as the original client does in 32-bit floats (stored as f32, times 2,000.0, times
  the f32 nearest 1 / 65,535, each product rounded to f32). Because neighbouring blocks store
  equal edge samples (HM7), shared vertices come out bit-identical. (Revised 2026-10-05; the
  former rule stored / 32.768 differs by at most 0.031 units.)
- **One conversion** (Tessera Decision; reworded 2026-10-08, answering the implementer's
  OPEN of 2026-10-06). The decoder's conversion of a stored integer (`landblock.md` §3.4) is
  the reference: a query at a sample, the block's min/max height and the mesh vertex z equal
  it bit for bit. An interpolated query works in two rounding steps, like the decoder's
  rule: (1) the interpolated height in stored steps, computed in f64 and clamped to the range
  of the samples it uses, is multiplied by 2,000 in f64 and rounded to f32; (2) that value is
  multiplied in f32 by the f32 nearest 1 / 65,535. At a sample this is exactly the decoder's
  rule (a single rounding of stored × 2,000 / 65,535 would differ from it by 1 ulp for some
  stored values). Both steps are monotone, so an interpolated query never leaves the block's
  height bounds. Slopes, normals (§5) and the level error bounds (§3) use the f64 factor
  2,000 / 65,535. (`landblock.md` §3.11 converts the corner samples first and then evaluates
  the plane; the two orders differ only by floating-point rounding.)

- **Memory.** A block keeps 2,178 bytes of heights and a 128-byte bit set of split diagonals
  (about 2.3 KiB). The per-level error bounds (§3) are computed on first use, so height-only
  users (ground clamping, a server) never pay for them.
- Frame: Z-up, +X east, +Y north (`coordinates.md` §3). Vertex positions are block-local,
  x, y ∈ [0, 160] from the south-west corner, z absolute; the renderer translates a block by
  (160 · (X − X_anchor), 160 · (Y − Y_anchor), 0) (`coordinates.md` §12).
- **Vertex order**: every per-vertex array uses vertex number **33 · j + i** (i along X,
  j along Y). This is the transpose of the stored height order and exactly the order of the
  vertex numbers in surface-layer index lists (`landblock.md` §3.8.3), so layer triangles index
  the full-grid vertex set without conversion.

<a id="s2"></a>
## 2. Height queries (for the walk controller)

| Query | Rule |
|---|---|
| `height_at(x, y)` | `landblock.md` §3.11: the plane of the full-grid triangle containing the point, quad split by bit 7. Coordinates clamped to [0, 160]; NaN gives none. Evaluated in f64. |
| `normal_at(x, y)` | Unit normal of that triangle (facet normal). |
| `height_bilinear(x, y)` | Bilinear interpolation of the four quad corners (smoother; not the rendered surface). |
| `lod_height_at(lod, x, y)` | The regular (unstitched) surface of a coarser level. |
| `TerrainTiles::height_at(G_x, G_y)` | Region-global: locate the landblock (`coordinates.md` §4; X = ⌊G_x / 160⌋ clamped to 254), and on a shared edge fall back to the resident neighbour that also contains the point. |

The level-0 mesh **is** the `height_at` surface: same vertices, same diagonals, same winding.
Verified on every landblock of the install (§8).

<a id="s3"></a>
## 3. Levels of detail (geomipmapping)

- Six levels. Level *l* uses every 2^*l*-th sample: quads of 5, 10, 20, 40, 80 and 160 units
  (32 down to 1 quad per side). All vertices of all levels are grid samples, so a vertex has
  one position, normal and colour whatever the level.
- Level 0 splits each quad by the stored diagonal. A coarser quad has no stored diagonal; it
  takes the diagonal whose midpoint height is closer to the sample at the quad's centre (ties:
  south-west to north-east), decided on stored integers so it is deterministic.
- **Geometric error** of a level: the largest vertical distance between the level's regular
  surface and the level-0 surface (§2) anywhere on the block, made non-decreasing over the
  levels and rounded up to the next f32. Both surfaces are piecewise linear, so the difference
  peaks at a vertex of their common refinement: a grid sample, or the centre of a fine quad
  whose stored diagonal crosses the coarse quad's diagonal. Both kinds of point are evaluated,
  so the bound is exact (checking samples alone underestimated it by up to 1.5× on real data,
  and to 0 on a synthetic fold). Stitch strips (§4) are not part of the regular surface and
  are not covered. Computed lazily on first use (§8 gives the cost); a streamer computes it on
  the worker that builds the block.
- **Selection** (`LodSelector`): the coarsest level whose error, projected at the block's
  distance (`error · viewport_height / (2 · tan(fov_y / 2)) / distance`), stays within a pixel
  tolerance (default 1.5 px at 1080 px and 60°). Blocks within `full_detail_radius` (default
  240 units) are always level 0, so the near ring's rendered surface equals the collision
  surface. Both thresholds have hysteresis (default 0.15): a block only moves to a coarser
  level once that level is below `tolerance / (1 + hysteresis)`, and a block at level 0 only
  leaves it at `full_detail_radius · (1 + hysteresis)`, so blocks on a threshold do not flicker
  (each flip would also re-stitch the four neighbours). All defaults are Tessera's own tuning.
- Degenerate distances never trigger a re-mesh of the ring: an infinite distance selects
  level 5, a NaN distance (corrupt camera) keeps the block's current level (level 5 without
  one). An invalid selector (NaN or negative tolerance, non-positive or non-finite projection
  scale) selects level 0 consistently.

<a id="s4"></a>
## 4. Crack-free edges: stitching

- Each side of a block is meshed at its **edge level**: the coarser of the block's level and
  the neighbour's (`EdgeLods::effective`; a missing neighbour counts as the block's own level).
  Both blocks sharing an edge therefore use the same edge vertices.
- A side whose edge level is coarser than the block's is **stitched**: the row of quads along
  that side is replaced by a strip between the coarse edge vertices (outer chain) and the
  block's own vertices one quad step inside (inner chain). The strip is triangulated by zipping
  the two chains, always advancing the chain whose next segment has the smaller midpoint; every
  triangle is counter-clockwise and none is degenerate.
- Where two stitched sides meet, both strips end on the diagonal from the block corner to the
  inner corner; where a stitched side meets an unstitched one, the stitched strip takes the
  corner quad and the inner chain runs to the block edge.
- Result, checked exhaustively in tests for every level and every combination of four
  neighbour levels: the triangles cover the 160 × 160 square exactly once, every interior edge
  is shared by exactly two triangles with opposite directions, and the boundary on each side
  consists of exactly the segments between consecutive multiples of that side's edge step. Two
  neighbours thus mesh their shared edge with identical segments: no cracks and no
  T-junctions, at any level difference (not just one).
- Consequence: a level-0 block next to a coarser block deviates from the §2 surface inside the
  5-unit strip along that edge. With the default selector this never happens near the camera.
- No skirts are used.

<a id="s5"></a>
## 5. Normals

Revised 2026-10-08: the original's rule is now known (TR16, TR17) and Tessera follows it.
**Implementers re-check `crates/terrain`**: until it follows, the code uses the superseded
rule at the end of this section.

- **Rule** (the original's, `landblock.md` §3.14, normative there; `terrain-texturing.md`
  §4.10 states the same). Per grid sample (*i*, *j*) of the terrain surface: take the grid
  triangles of the four quads that have the sample as a corner, each quad split by its own
  stored diagonal (bit 7), so a quad contributes one triangle, or two when its diagonal runs
  through the sample (four to eight triangles in all). Positions are (5 · *a*, 5 · *b*, *h*)
  with *h* converted by `landblock.md` §3.4. Each triangle's normal is its unit normal
  facing up; the vertex normal is the plain mean of those unit normals, **not** normalised
  again (shorter than 1 on a crease).
- **Ceiling surface**: the mirrored rule of `landblock.md` §3.14 (ceiling heights, bit-0
  flags, X offsets mirrored while the flag tests are not, normals facing down). It equals
  the negated terrain rule on a plane and can differ elsewhere.
- **Block edges.** A triangle that needs a sample beyond the block uses the neighbouring
  block's data: index −1 is the neighbour's 31, index 33 its 1, and at a corner both indices
  are mapped, which needs the **diagonal** neighbour as well (eight neighbours in all, not
  four). Split flags of quads beyond the block come from the neighbour in the same way. A
  triangle with a sample in a block that does not exist, or (for a ceiling) in a neighbour
  without a ceiling, is left out of the mean (TR17).
- **Bit identity at seams** (Tessera Decision): compute each triangle normal from corner
  positions taken relative to the sample (exact multiples of 5 units across, height
  differences of converted heights) and add the triangles in a fixed order (quads south-west,
  south-east, north-west, north-east; within a quad, the order of the `landblock.md` §3.14
  table). Both blocks that share an edge sample then use the same triangles in the same
  order and compute bit-identical normals.
- **Provisional sides** (Tessera Decision). When a needed neighbour block exists in the
  archive but is not resident yet, its triangles are left out as above and the side is
  reported **provisional** (`VertexNormals::is_provisional`); a streamer recomputes the
  normals (and the mesh's normal and colour streams) when the neighbour arrives. Whether the
  original recomputes in that case was not traced. A side whose neighbour does not exist in
  the archive (the region border, TR18) is final, not provisional: the original leaves those
  triangles out for good.
- Normals are computed on the full grid and shared by every level, so they are also identical
  across LOD transitions.
- **Superseded rule** (Tessera Decision of 2026-10-04, revised 2026-10-05, retired
  2026-10-08; kept only because the code still uses it): central differences of stored
  heights over two cells, dz/dx = (h(i + 1, j) − h(i − 1, j)) × *s* / 10 with *s* = 2,000 /
  65,535, likewise for y, normal = normalise(−dz/dx, −dz/dy, 1); the four edge neighbours
  only; one-sided differences without a neighbour; ceilings negated.

<a id="s6"></a>
## 6. Shading until splatting: height and slope

Texture splatting is now specified in `terrain-texturing.md` (§8 there gives the algorithm).
The height/slope palette below stays as Tessera's **fallback**: for blocks whose layers cannot
be resolved (no terrain type table for the region, a hostile chunk rejected by §7, missing
textures) and as a debug view. Before `terrain-texturing.md` existed, terrain was coloured per
vertex from Tessera's own palette (`HeightSlopeShading::default`; **Tessera Decision**, not taken
from any data):

| Height (units) | Colour (sRGB) |
|---|---|
| 0 | 186, 172, 128 (shore sand) |
| 30 | 128, 142, 86 (grass) |
| 140 | 86, 120, 60 (green) |
| 420 | 70, 98, 52 (dark green) |
| 820 | 118, 110, 82 (heath) |
| 1,200 | 126, 122, 116 (rock) |
| 1,550 | 232, 234, 238 (snow) |

Linear interpolation between bands (in linear RGB), then a smoothstep blend towards a cliff
colour (112, 104, 94) between 32° and 48° of slope (slope from the §5 normal). Palettes are
sanitised on use (non-finite stops dropped, stops sorted, colours clamped, at most 64 stops).
The colours go into the mesh's linear vertex colours; draw with a white lit material.

Block UVs: u = x / 160 eastwards, v = 1 − y / 160 (v = 0 at the north edge, matching the
top-row-first convention of `tessera-render` and the north-up minimap tiles of
`landblock.md` §6.3).

**These block UVs are for minimap-style images only** (Tessera Decision). Layer alpha masks
and the landblock light map have their first stored row on the **south** edge (TR7,
`terrain-texturing.md` §5.2, §10): sample them at (x / 160, y / 160), or flip v, when they are uploaded in stored row order.
Layer textures use the world-space coordinate sets of `terrain-texturing.md` §6, not these UVs.

<a id="s7"></a>
## 7. Splatting inputs and mesh streams

- `splat_layers(chunk)` turns a terrain surface chunk (`landblock.md` §3.8) into drawable
  layers in stored order: material id, mask DID (`None` for 0), kind (masked, opaque, whole)
  and the layer's triangles as grid vertex numbers (empty for `whole`). Empty layers are
  skipped. Meaning (TR6, `terrain-texturing.md` §4, §7, §8): the material id is a key of the
  region's terrain type table; stored order is the draw order, bottom first; the first drawn
  layer has no opacity and covers what is below, later ones blend "over" with the opacity of
  `terrain-texturing.md` §7 (which depends on whether the layer has a mask and on the entry's
  `use_opacity_mask`); the shading features of a layer are chosen by
  `terrain-texturing.md` §8.1; a `whole` layer covers all 2,048 grid triangles (the level-0
  triangulation of §1); grid triangles in no layer are not drawn (TR12: they lie under the
  ceiling surface or where there is no ceiling; collision keeps them, `collision.md` §6.5).
  Hostile chunks are rejected (Tessera Decision): windows outside the index list, vertex numbers
  ≥ 1,089, more than 256 layers, or windows that together copy more indices than the list
  holds (real chunks partition their list, `m = 3 · Σ prims`). The `tessera-formats` decoder
  already enforces the first two, but not the last two, so a corrupt record that decodes can
  still be rejected here; callers skip splatting for that block (height/slope shading stays)
  rather than treating the error as impossible. No real record is rejected (§8).
- `TerrainMesh` mirrors `tessera_render::MeshData` (positions, normals, uvs, linear colours,
  u16 indices) without depending on the renderer (ARCH §3: terrain is L3, render L4). It keeps
  only the vertices its triangles use, in grid order, and records each one's grid vertex number
  (`grid_vertices`), so `remap_grid_indices` maps layer triangles onto any mesh that has their
  vertices (always the unstitched level-0 mesh, whose indices equal grid numbers).
- **No shared index buffers (deviation from ARCH §10).** ARCH §10 plans "shared index buffers
  per LOD and stitch pattern". That does not fit this terrain: level 0 uses each block's
  stored diagonals and coarser levels pick their diagonals from each block's heights, so the
  triangle lists differ from block to block, and `TerrainMesh` compacts its vertices per mesh.
  Every block and level therefore has its own vertex and index buffer (at most 1,089 vertices
  and 6,144 + stitch indices). A cheaper GPU layout the API already supports: one full-grid
  vertex buffer per block (1,089 vertices in grid order, never re-uploaded on a level change)
  plus a per-level index list from `triangulate`, whose indices are grid vertex numbers.
  > **OPEN:** (documentation, not a fact question; for the owner of `docs/ARCHITECTURE.md`)
  > ARCH §10 still says "shared index buffers per LOD and stitch pattern" for the terrain
  > output (as of 2026-10-08); replace it with the per-block layout above. Nothing in the
  > data or the client bears on it.
- **Splatting inputs per vertex.** The layer texture coordinates of `terrain-texturing.md`
  §6.1 (TR8) are functions of the vertex position (block-local x, y plus the block's offset of
  `terrain-texturing.md` §6.2, and the height) and the mask coordinates are (x / 160, y / 160),
  so a layer shader can derive sets 0–3 from the position stream plus a per-block offset
  uniform (whose anchor is the Tessera Decision of `terrain-texturing.md` §6.2). Normal-mapped
  layers also need the per-vertex tangent frame of `terrain-texturing.md` §6.4 (TR9); it is a
  function of the vertex normal alone (tangent = the
  east axis projected onto the surface, binormal = normal × tangent), so it can be derived
  from the normal stream; no extra vertex stream is required. A `whole` layer is drawn with
  the level-0 index list (§1), which already equals the client's full-grid list.
- **Layer resolution (implemented 2026-10-06, M4.5; design is Tessera's, facts are
  `terrain-texturing.md`'s).** `resolve_chunk` / `resolve_landblock` turn a terrain or
  ceiling chunk into draw records in stored order (`ChunkLayers`, `DrawLayer`): the entry
  looked up by key, the first drawn layer marked opaque, `whole` layers expanded to the
  surface's full level-0 triangulation (bit-7 diagonal and counter-clockwise for the
  terrain, bit-0 and clockwise for the ceiling), the exact grid-triangle coverage of every
  layer (collision-tree keys) and the holes, the feature switches (§8.1 there), material
  parameters (§8.2), opacity rule and alpha test (§7), textures by DID, and the light map.
  `LayerVertices` holds the full-grid vertex streams (1,089 vertices in grid order:
  positions, normals, tangent frame, coordinate sets 0–3); `TextureFrame` applies the
  per-block offset rule of `terrain-texturing.md` §6.2 with the camera's landblock as the
  reference. The height/slope shading of §6 is only a debug fallback. **Tessera Decisions**
  beyond those in `terrain-texturing.md`: (1) after a layer skipped for a missing key, the
  first layer actually drawn is the opaque one; (2) coarser levels and stitched level-0
  meshes draw a layer on every triangle of the level's list that is a level-0 grid
  triangle the layer covers, or that is a coarser or stitch triangle whose quad bounding
  box contains a quad the layer touches (`ChunkLayers::layer_indices_for`); the original's
  distance bands are OPEN (`terrain-texturing.md` §12); (3) ceilings are drawn at level 0
  from their stored triangles only; (4) non-finite stored floats of a terrain type entry
  count as 0; (5) rotations by multiples of 90° use exact sines and cosines. Added
  2026-10-06 after the M4.5 code review: (6) a stored layer triangle that is not a level-0
  grid triangle wound as its surface is (counter-clockwise for the terrain, clockwise for
  the ceiling), or that repeats a triangle of the same layer, is not drawn and is counted
  (`ChunkLayers::rejected_triangles`, one warning per chunk), so what is drawn always
  equals the coverage, the holes and the CPU reference (none in the install, §8); (7) the
  lists of `layer_indices_for` come back in the surface's winding (a ceiling's face down),
  although ceilings are drawn from their stored triangles only (3); (8) all `whole` layers of
  a chunk share one full-grid list, and missing keys and repeated keys are logged once per
  chunk; (9) a rejected mask is dropped with a logged warning (`DrawLayer::check_mask`,
  `terrain-texturing.md` §5.1); (10) the layer texture transform (`LayerTransform`) returns
  finite values only, as `material.md` §8.4's does.

<a id="s8"></a>
## 8. Verification

- Synthetic unit and property tests: axis order, the §3.11 rule against the
  `tessera-formats` decoder, exhaustive stitch-pattern covers, east/north seam identity
  (positions, normals, colours, segments) for all 36 level pairs, vertex identity across
  levels, normals bit-identical at seams and corners, hostile palettes and splat chunks,
  exact level error bounds against a dense raster (including a fold block whose samples all
  lie on the coarse surface), one height conversion for all 65,536 stored values, selector
  hysteresis at the full-detail radius and degenerate distances.
- Golden images (`crates/terrain/tests/golden/`): a generated 3 × 3-block patch at mixed
  levels, lit, and top-down with its wireframe; checked on hardware Vulkan, llvmpipe (GL) and
  lavapipe (Vulkan).
- Real data (`crates/terrain/tests/real_data.rs`, release build, this install; figures of
  the implementer's run of 2026-10-06 with the height rule of `landblock.md` §3.4 as
  revised, normals by the superseded §5 rule): all 390,150 landblock-data records of regions
  1–5 and 14; height queries agree with the decoder's rule within 1.22 × 10⁻³ units; all
  458,061,498 stored layer triangles are triangles of the level-0 mesh; every block meshed at
  a pseudo-random level stitched to its neighbours' levels is a valid cover; all 777,240
  east/north neighbour pairs share their edge samples, and on the pairs with final normals on
  both sides, 382,536 east and 382,536 north seams (765,072), the two meshes have
  bit-identical edge vertices, normals and segments (0 mismatched pairs). The run of
  2026-10-04 compared east seams only (its residency window never completed a north
  neighbour's own neighbours), so its "382,536 seams" were all east pairs; the test now
  keeps rows y − 1 to y + 2 resident and counts both directions. On every eighth block (48,768 blocks)
  the level error bounds equal the maximum of |level surface − level-0 surface| over the
  2.5-unit raster (all refinement vertices) within 10⁻³ units. Per block on one thread: block
  setup 1.2 µs, level error bounds (lazy) 36 µs, normals 7 µs, level-0 mesh 28 µs.
- Layers (`crates/terrain/tests/layers_real_data.rs`, release build, 2026-10-06, this
  install): all 5 terrain type tables (4,688 entries) hold `terrain-texturing.md` §3.5
  items 1–6 and re-encode byte for byte; every layer key and quad-grid value of all
  390,150 landblock-data records is in its region's table (SC7): 86,726 terrain and 2,355
  ceiling chunks with layers, 925,884 layers, 0 unresolved, draw order never decreasing,
  no key twice in a chunk, 0 rejected layer triangles (off the level-0 grid, wound against
  the surface or repeated; re-run 2026-10-06 after the review fixes); 2,750,134 hole
  triangles in 2,368 landblocks, all with a ceiling (515 of them have layers but none
  drawn); 60 + 43 drawn layers without a diffuse texture (T39); all 646,073 masks of drawn
  layers are A8 and square (623,092 × 128, 20,624 × 256, 2,357 × 512); the 5,441
  light-map values name 5,422 DXT1 surfaces (5,421 × 128², 1 × 64²). On every 7th block
  (12,748 chunks) the level-0 lists of `layer_indices_for` equal the stored triangles (both
  surfaces, winding included) and
  the surface's vertex streams are finite with downward ceiling normals. The worked
  example of `terrain-texturing.md` §14 is reproduced exactly
  (`crates/terrain/tests/worked_example.rs`).
- Fuzzing: `terrain_layers` (layer resolution, draw records, level-of-detail lists, vertex
  streams, texture frame and masks on hostile tables and chunks) and `terrain_mesh`
  (meshing, normals, shading, queries, selection, tiles, splat conversion on arbitrary
  inputs).

- After the §5 revision of 2026-10-08 the seam comparison needs the diagonal neighbours
  resident as well (corner samples); the invariant is unchanged: on every pair with final
  normals on both sides, edge vertices, normals and segments are bit-identical. Normals on
  the region border (TR18) are final with the outside triangles left out.

<a id="s9"></a>
## 9. Open questions

**Answered 2026-10-05 (was OPEN): texture splatting.** The `material` id is a key of the
region's terrain type table 0x19 (`terrain-texturing.md` §3, §4.1); layers are drawn in stored
order, first drawn layer opaque, later ones blended "over" with the opacity rule of §7 there;
masks have row 0 on the south edge (§5.2 there); `whole` = 1 covers all 2,048 grid triangles
(§4.3 there). Algorithm: `terrain-texturing.md` §8.

**Answered 2026-10-05 (was OPEN): coarse grid.** It does not select textures; it is the
per-vertex scenery word (`landblock.md` §3.7, `scenery.md` §3, `terrain-texturing.md` §4.7).
Splatting uses the layers only.

**Answered 2026-10-05 (was OPEN): light maps.** Same orientation as the masks (row 0 south);
added to the lighting sum (`terrain-texturing.md` §10, §9.1; BIN). Use (x / 160, y / 160), not
the §6 block UVs.

**Answered 2026-10-05 (was OPEN): height scale.** `landblock.md` §3.4 adopted the client's
rule stored × 2,000 / 65,535 (BIN); §1 and §5 use it, and the implementation has followed
since 2026-10-06 (the former stored / 32.768 differed by at most 0.031 units).

**Answered 2026-10-05 (was OPEN): water.** The water level of a landblock comes from its block
map (keys 7 and 4) and the region's water table: `water.md` §6 gives the level and the "under
water" test, §8 the water geometry and shading. Water surfaces are not terrain layers; river
and lake beds are ordinary terrain types (`terrain-texturing.md` §11).

**Answered 2026-10-05 (was OPEN): cave ceilings, diagonal and zero samples.** A ceiling quad
is split by **bit 0** of the vertex-flag byte of its south-west sample (bit 7 is the terrain
diagonal); stored ceiling triangles face down (clockwise from above); the original's ceiling
mesh contains all 2,048 triangles, also those over zero ceiling samples, which lie at height 0
(TR11, `collision.md` §6.4: Confirmed on all 5,310,848 stored ceiling triangles, BIN for the
mesh builder). The original builds the ceiling chunk's render mesh with the terrain vertex
builder from the ceiling height grid and a separate ceiling normal array (TR10,
`terrain-texturing.md` §4.8). `tessera-terrain` builds the ceiling surface (bit-0
diagonals), its normals (§5) and its layer vertex streams, and expands `whole` ceiling
layers to the clockwise bit-0 grid, but makes no levels of detail for it: ceilings are drawn
at level 0 from their stored ceiling-layer triangles (§7, Decision (3); Tessera Decision).

**Answered 2026-10-08 (was OPEN, implementer 2026-10-06): stale statements.** (a) §1 and the
paragraph above now describe the ceiling surfaces the crate builds; (b) the height-scale
answer records that the implementation follows the revised rule; (c) the header carries the
state Revised and this date; the §7 and §8 additions of 2026-10-06 and this revision wait for
review.

**Answered 2026-10-05 (was described wrongly): terrain triangles in no layer.** In landblocks
with a ceiling, some terrain grid triangles belong to no layer and the original does not draw
them. They are **not** openings at cave mouths: every one lies where the terrain is buried
under the ceiling surface or where the ceiling samples are 0 (TR12, `collision.md` §6.5,
census of all landblocks), and the original keeps them in its terrain collision. The level-0
mesh of §3 still contains them (Tessera Decision); a layer renderer that draws only layer
triangles reproduces the original, the fallback-shaded mesh of §6 shows terrain the original
hides; collision uses the full grid.

**Answered 2026-10-05 (was part of the draw-distance OPEN): the original's shading model.**
The original's terrain vertices carry normals and, with normal maps on, a tangent frame;
ambient, hemispheric and point lights are evaluated per vertex (the hemispheric term per pixel
on normal-mapped layers), the sun per pixel with the shadow factor, and the light map is added
(TR13, `terrain-texturing.md` §9–§10, BIN).

**Partly answered 2026-10-08: per-block texture origin** (`terrain-texturing.md` §6.2). The
rule is known (TR8: the block's place relative to a reference landblock plus an accumulated
shift that resets at 2,048 units). The reference landblock and the accumulator belong to the
distant-landscape sector manager: the reference is the landblock of the viewer position that
also selects the loaded sectors and the far LOD (`distant-landscape.md` §5.1–§5.2), and it
is replaced whenever that position's landblock or region changes (TR22, BIN). The world
manager's per-frame load position is the player character's position by default and an
outside position (the camera's, by the switch's own description) when a console switch is
turned off (TR23, BIN); that this is the position the sector manager receives is Inferred,
the call path between them was not traced. **Tessera Decision** (unchanged,
`terrain-texturing.md` §6.2): the camera's landblock as the reference, or fixed region
coordinates; both differ from the original only by whole blocks.

> **OPEN:** (a) Confirm the call path from the world manager's load position to the sector
> manager's reference (TR23 → TR22), so that "the player's landblock, not the camera's" can
> be stated as BIN. (b) Whether full-detail blocks already built are rebuilt, or keep their
> old offset, when the accumulator resets (a reset that left built blocks unchanged would
> show as a texture jump at block edges with scales such as 0.055); answerable by M4 on the
> per-block build or by the owner watching such a texture while walking 2,048 units in one
> direction in the original.

**Answered 2026-10-08 (was OPEN): ceiling normals.** The original's separate ceiling normal
array follows the mirrored rule of `landblock.md` §3.14 (bit-0 flags, X offsets mirrored,
normals facing down, not renormalised; TR16, BIN). §5 now uses it.

**Answered 2026-10-08 (was OPEN): the original's terrain vertex normals.** The plain mean of
the unit normals of the four to eight grid triangles around the sample, with neighbours
(diagonal ones included) across block edges and no renormalisation (`landblock.md` §3.14,
`terrain-texturing.md` §4.10; TR16, TR17, BIN). §5 now uses it.

**Answered 2026-10-08 (was OPEN): region edges.** Every region with landblock data is a
complete 255 × 255 grid of landblock-data records (X, Y from 0 to 254; none at 255), so the
only blocks without a neighbour are the 1,016 on the region border (TR18, Confirmed). The
original leaves a normal's triangles that need a sample beyond the border out of the mean
(TR17), so the border normals average the triangles inside the block, and nothing lies beyond
the border: no landblock records and no distant-landscape sectors (TR19). §5 reproduces this
and marks those sides final, not provisional.

**Partly answered 2026-10-08: draw distance and LOD of the original** (BIN unless marked).

- **How far.** The landscape draw-distance graphics option has five levels (0–4). They give a
  far radius *R* of 2, 4, 6, 9 or 12 landblocks (any other level: 6); a console override,
  off by default, replaces *R* by its own value (default 6). The built-in initial level is 3
  (*R* = 9) until the user's preferences set it; the six overall graphics presets, lowest to
  highest, set levels 0, 1, 2, 3, 4 and 4 (TR20). Values are a user setting: read Tessera's
  own setting, do not assume a level.
- **Distant terrain.** The sector radius of `distant-landscape.md` §5.1 is
  *F* = ⌊*R* / 3⌋ + 1 sectors around the viewer's sector (TR20); within the loaded sectors each
  landblock gets far LOD 0–3 by its squared landblock distance to the viewer (≤ 3, ≤ 24,
  ≤ 99, beyond; `distant-landscape.md` §5.2), each LOD halving the sample spacing of the
  33 × 33 grid.
- **Near field.** The landscape's near radius is 1 landblock: its landblock grid is 3 × 3
  around the viewer's landblock, and the sector manager flags exactly the landblocks with
  max(|*dx*|, |*dy*|) ≤ 1 as near (*N* = 1 in `distant-landscape.md` §5.2; TR21). **Inferred:**
  the layered terrain of `terrain-texturing.md` covers those 3 × 3 landblocks and the
  distant-landscape mesh draws the rest.
- The selector of §3 (Tessera's own geomipmapping of landblock records at every distance) is
  a Tessera Decision and does not reproduce this.

> **OPEN:** (a) ER (`terrain-texturing.md` §12) reports full-detail terrain to about 14
> landblocks; TR21 gives a 3 × 3 landblock near field. Which landblocks the original draws
> with the full layer stack and which with the distant-landscape mesh needs the owner's
> observation of the original (for example a frame capture, or the layer detail visible on
> landblocks two and three blocks from the player at each draw-distance level). (b) How near
> and far terrain are blended at the seam (the sector overlay of `terrain-texturing.md` §12
> and the near/far seam-blending graphics option): not traced.

<a id="s10"></a>
## 10. Change log

| Date | Change |
|---|---|
| 2026-10-04 | M3 design (clean-room implementer). |
| 2026-10-05 | Spec writer (dirty room): §6 height/slope shading becomes the fallback and the block-UV note warns that masks and light maps are south-row-first; §7 states the meaning of the splat layers and how a layer shader derives its coordinates; §9 answers the splatting, coarse-grid and light-map OPENs from `terrain-texturing.md`, adds the height-scale and texture-origin OPENs and notes on water and cave holes. |
| 2026-10-05 | Spec writer (dirty room), second revision: §1 and §5 use the height rule adopted by `landblock.md` §3.4 (stored × 2,000 / 65,535; implementation to follow). §7: layer meaning follows `terrain-texturing.md` §7–§8.1 (opacity per record family, feature switches); the tangent frame for normal-mapped layers derives from the normal stream. §9: coarse-grid note corrected (it is the scenery word); height-scale and water OPENs answered (`landblock.md` §3.4, `water.md` §6, §8); texture origin marked as a Tessera Decision; ceilings and shading model updated from `terrain-texturing.md` §4.8 and §9. |
| 2026-10-05 | Spec writer (dirty room), third revision after review. Header: Status "M3 design, implemented" replaced by the CLEAN_ROOM §5.2 state Review; per-section author roles; "Format facts from" adds `collision.md` §6; the introduction no longer claims the file has no facts from dirty-room material and separates Tessera's design from cited facts. New §0 (marks, provenance table TR1–TR15 with the owning spec and mark of every fact about the data or the original). §5, §6, §7: Tessera Decisions labelled; §7 holes text and per-vertex splatting inputs follow `terrain-texturing.md` §4.5 and §6.2. §9: ceiling diagonal and zero samples answered (bit 0, `collision.md` §6.4); "holes at cave mouths" corrected (`collision.md` §6.5); the answered shading model split out of the draw-distance OPEN; separate OPENs for the texture origin (rule now known), ceiling normals, the original's vertex normals and draw distance. |
| 2026-10-06 | Implementer (clean room), M4.5: §7 layer resolution, vertex streams and texture frame implemented, with five Tessera Decisions (missing keys, level-of-detail lists, ceilings at level 0, non-finite floats, exact right-angle rotations); §8 layer real-data results and the `terrain_layers` fuzz target. Not yet reviewed. |
| 2026-10-06 | Implementer (clean room), M4.5 code-review fixes: §7 Tessera Decisions (6)–(10) (rejected stored triangles, level-of-detail lists in the surface's winding, shared `whole` lists and per-chunk warnings, logged mask rejection, finite layer transform); §9 OPEN on the stale ceiling and height-scale sentences and the header. Not yet reviewed. |
| 2026-10-08 | Spec writer (dirty room; M1 probe OBS-T and M4 BIN-T under decision 0011, §0.3). Status Accepted → Revised. §0: rows TR16–TR23, new §0.3. §1: ceiling bullet matches §7; the "one conversion" OPEN answered by stating the implemented two-step rounding as the Tessera Decision. §5: **normals now follow the original's rule** (`landblock.md` §3.14: mean of unit triangle normals, diagonal neighbours at corners, missing-block triangles left out, no renormalisation, mirrored ceiling rule); fixed summation order for bit identity; region-border sides final; the central-difference rule kept as superseded until the code follows. §8: seam figures replaced by the implementer's 2026-10-06 run (east and north seams), OPEN closed; note on diagonal neighbours. §9: OPENs answered — stale statements, ceiling normals, terrain normals, region edges; texture origin and draw distance partly answered (far radius by graphics level, sector radius, near radius 1, reference landblock held by the sector manager, player position as load position) with narrowed OPENs. The M3 criterion-3 evidence in `docs/ROADMAP.md` (east seams only) is for that file's owner to update. |
| 2026-10-08 | Spec reviewer (not the writer of the 2026-10-08 revision): review passed; Status Revised → Accepted. Record in the review log. |
