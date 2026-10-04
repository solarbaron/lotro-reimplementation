<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Terrain meshing, LOD, normals and shading (`crates/terrain`)

| | |
|---|---|
| **Status** | M3 design, implemented |
| **Last updated** | 2026-10-04 |
| **Author role** | Clean-room implementer of `tessera-terrain` |
| **Implements into** | `crates/terrain`, `fuzz/fuzz_targets/terrain_mesh.rs` |
| **Format facts from** | `landblock.md` §3 (height grid, scale, split diagonal, surface layers, §3.11 height rule), `coordinates.md` §3, §4, §12 |

This file holds **Tessera's own design decisions** for terrain: how the landblock height grid
becomes meshes at several levels of detail, how seams stay closed, how normals and colours are
made, and what the walker's height queries return. It contains no format facts beyond those
cited from `landblock.md` and `coordinates.md`, and nothing from dirty-room material. The open
format questions that block further work are collected in §9.

---

<a id="s1"></a>
## 1. Input and frames

- One `TerrainBlock` per landblock, built from the decoded height grid (33 × 33 u16, X-major,
  `landblock.md` §3.3) and the vertex flags (bit 7 = split diagonal of each quad, §3.6). The
  ceiling grid of cave blocks is not terrain and is not meshed here (§9).
- Heights stay as the stored u16 values and are converted with `stored / 32.768` (§3.4) only
  when a vertex or a query result is produced. Because neighbouring blocks store equal edge
  samples (HM7), shared vertices come out bit-identical.
- **One conversion.** The scale is the decoder's constant (`HEIGHT_STEPS_PER_UNIT`, an f32);
  queries interpolate in f64 and divide by that constant widened to f64, then round once to
  f32. For a stored integer this equals `height_from_raw` bit for bit, so a query at a sample,
  the block's min/max height and the mesh vertex z agree exactly, and an interpolated query
  (clamped to the samples' range) never leaves the block's height bounds.
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

- Per grid sample, from central differences of stored heights over two cells:
  `dz/dx = (h(i + 1, j) − h(i − 1, j)) / (32.768 · 10)`, likewise for y, normal =
  normalise(−dz/dx, −dz/dy, 1).
- On a block edge the sample beyond the edge comes from the edge neighbour's second row or
  column (only the four edge neighbours are needed; the corner sample uses the east/west
  neighbour for x and the north/south neighbour for y). The differences are taken on stored
  integers before any floating-point step, so both blocks compute bit-identical normals for
  every shared vertex.
- Without a neighbour the difference is one-sided (scaled to the same span) and the side is
  reported **provisional** (`VertexNormals::is_provisional`); a streamer recomputes the normals
  (and the mesh's normal and colour streams) when the neighbour arrives.
- Normals are computed on the full grid and shared by every level, so they are also identical
  across LOD transitions.

<a id="s6"></a>
## 6. Shading until splatting: height and slope

Until the surface-layer material table is known (§9), terrain is coloured per vertex from an
original palette (`HeightSlopeShading::default`):

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

<a id="s7"></a>
## 7. Splatting inputs and mesh streams

- `splat_layers(chunk)` turns a terrain surface chunk (`landblock.md` §3.8) into drawable
  layers in stored order: material id, mask DID (`None` for 0), kind (masked, opaque, whole)
  and the layer's triangles as grid vertex numbers (empty for `whole`). Empty layers are
  skipped. Hostile chunks are rejected: windows outside the index list, vertex numbers
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
  > **OPEN:** ARCH §10 still states the shared-index-buffer plan; its owner should replace it
  > with the per-block layout above.

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
- Real data (`crates/terrain/tests/real_data.rs`, release build, 2026-10-04, this install):
  all 390,150 landblock-data records of regions 1–5 and 14; 6,242,400 height queries agree
  with the decoder's rule within 1.3 × 10⁻³ units; all 458,061,498 stored layer triangles are
  triangles of the level-0 mesh; every block meshed at a pseudo-random level stitched to its
  neighbours' levels is a valid cover; all 777,240 east/north neighbour pairs share their edge
  samples, and on the 382,536 pairs with final normals on both sides the two meshes have
  bit-identical edge vertices, normals and segments. On every eighth block (48,768 blocks)
  the level error bounds equal the maximum of |level surface − level-0 surface| over the
  2.5-unit raster (all refinement vertices) within 10⁻³ units. Per block on one thread: block
  setup 1.2 µs, level error bounds (lazy) 36 µs, normals 7 µs, level-0 mesh 28 µs.
- Fuzzing: `terrain_mesh` (meshing, normals, shading, queries, selection, tiles, splat
  conversion on arbitrary inputs).

<a id="s9"></a>
## 9. Open questions

> **OPEN:** Texture splatting. What a surface layer's `material` id indexes (the per-region
> 0x19 table is the leading candidate), the blend order between layers, the orientation of the
> alpha masks over the block and the meaning of `whole` = 1 (`landblock.md` §3.8.2 OPEN (1)–(4)).
> Until then `tessera-terrain` exposes the validated layers (`splat_layers`) but shades by
> height and slope (§6).

> **OPEN:** The 17 × 17 coarse grid (`landblock.md` §3.7) may carry terrain or scenery types;
> it is not used. If it turns out to be a terrain-type map, it could drive a cheaper per-vertex
> splat than the layer masks.

> **OPEN:** Baked light maps (`landblock.md` §6.4): texel orientation and how they combine with
> the layer textures. The mesh UVs (§6) are ready for a per-block texture once the orientation
> is known.

> **OPEN:** Cave ceilings (`landblock.md` §3.5): ceiling triangles face down, but the diagonal
> rule of ceiling quads and the handling of ceiling-below-floor vertices are open. Ceilings are
> not meshed by `tessera-terrain`; a renderer would draw the stored ceiling-layer triangles.

> **OPEN:** Draw distance and LOD behaviour of the original client (how far terrain is drawn,
> how coarse distant terrain gets) are unknown; the selector of §3 is Tessera's own policy. The
> same holds for the shading model: whether the original uses smoothed vertex normals at all
> (it has baked light maps) is unknown.

> **OPEN:** Water. Sea and lake surfaces are not part of the height grid (`environment.md` §8);
> a terrain renderer has no water level to shade the shore against yet.

> **OPEN:** Region edges. Blocks at X or Y = 0 or 254 have no neighbour on that side; their
> edge normals stay one-sided (provisional). No data says how the original treats the edge of
> the world.
