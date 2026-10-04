<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Landblock records (prefix 0x80): heightmap data, landblock info, block maps, landblock properties

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` (landblock data, landblock info, block-map and descriptor decoders), `crates/world` (landblock IDs, bundles, block-map resolution, minimap tiles), `crates/terrain` (height grid, triangulation, surface layers), `apps/explorer` (landblock view), `fuzz/` (one target per record kind) |
| **Companion specs** | `coordinates.md` (frames, units, map coordinates, rotations). `entity-descriptor.md` (the static entity descriptors inside landblock info; normative for their layout). `environment.md` §3 (region descriptor 0x0F, which names the block-map descriptor) and §4 (per-region tables 0x11–0x19). `records.md` §1 (VLE, TSize, Pascal string), §2.2 (landblock DID shape), §5 (render surfaces). `property-system.md` §4–§5 (values and embedded property sets), §4.6 (Position). `mesh.md` §10 (collision tree block and its `build_byte` rule). `dat-container.md` §5.2 (directory `stamp`), routing. `wstate.md` (0x70 objects named by `Area_DID` and weenies). |

---

<a id="s0"></a>
## 0. Purpose

The outdoor world is split into **regions**, each a grid of **landblocks** of 160 × 160
world units (`coordinates.md`). Every landblock is described by up to five records whose
DIDs share the shape `0x80KRXXYY` (`records.md` §2.2). This spec gives an implementer what is
needed to decode **every** landblock record of an install and to build a landblock bundle
for the world walker:

- which records exist, where they live and how regions map to archives (§2);
- the **landblock data** record `0x800RXXYY`: height grid, height scale, terrain
  triangulation, ceiling grid, surface layers, collision trees, all decoded to the last byte
  (§3);
- the **landblock info** record `0x802RXXYY`: cells, static entities, links, weenies (§4);
- **block maps** `0x801RXXYY` and their per-region descriptors (prefix 0x18): area, music,
  ambient sound, sky and other per-landblock keys (§5);
- **landblock properties** `0x804RXXYY`: minimap tiles and baked light maps (§6);
- the framing of the unidentified kind `0x805RXXYY` (§7);
- the bundle-loading algorithm, invariants, statistics, failure categories and what remains
  unknown (§8–§12).

All counts are observations of one install on one date and change with patches. Tests
assert the invariants, not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install on 2026-10-04 by throwaway probes in the
  dirty-room scratchpad, over **all** records of the kind unless a sample is stated.
- **Probable**: supported by a statistical test on the data, but not by an exact rule.
- **Reported**: comes only from third-party material and was not, or could not be, checked.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam). Every record of `client_cell_{1,2,3,4,5,14,15}.dat` and `client_map_{1,2,3,4,5,14,15}.dat` was read: 390,150 landblock-data, 390,150 block-map, 49,636 landblock-info, 60,450 landblock-properties and 33,159 kind-5 records. Every landblock-data, landblock-info, block-map and landblock-properties record decoded **to the last byte** with the layouts below. Directory `stamp` values were read for all of them. All five 0x18 descriptors and all seven 0x0F region descriptors were decoded. 120 minimap tiles and 43 layer masks were opened to check formats and orientation. |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution; decompiled classes `HeightMapDataLoader`, `HeightMap`, `MinimapLoader`, `GeoLoader`, `PositionDecoder`) and `lotro-tools` (readable source: `LandblockInfoLoader`, `BlockMapLoader`, `PropertyDescriptorsManager`, `LandblockLoader`). Facts only; no code taken. |
| JT | jtauber/lotro `cell.py` (no licence; facts only): header shape of landblock data. |
| DS | `research/notes/data-semantics.md` §6.2–§6.6 (summary of LC and JT). |
| ER | `research/notes/engine-client-re.md` §3 (cache type names, 3 × 3 frill source groups, inverted terrain ceiling for caves). |
| ENV | `environment.md` §3 (region descriptor layout; written in parallel from the same install). |
| ED | `entity-descriptor.md` (static entity descriptors; written in parallel from the same install). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| L1 | Landblock-family DIDs and archive routing (§2) | DS, LC; OBS | Confirmed |
| L2 | Landblock data header: DID, landblock-info DID or 0, u32 0, u32 1089, then 33 × 33 u16 heights (§3.2) | LC, JT; OBS (390,150) | Confirmed |
| L3 | Heights are stored **X-major**: sample (i, j) at index 33·i + j, i along X (§3.3) | LC (index formula); OBS (seam test, entity contact test) | Confirmed |
| L4 | Height in world units = stored value / 32.768 (32,768 = 1,000 units) (§3.4) | LC (`value / 32768`, a factor 1,000 off); OBS (3.25 M placed entities; round-number sea levels) | Confirmed |
| L5 | The bytes after the height grid: optional ceiling grid and its collision tree, vertex-flag grid, 17 × 17 grid, surface-layer chunks, terrain collision tree, box list (§3.1) | OBS (all 390,150 records, zero leftover bytes) | Confirmed (layout). No published source describes them. |
| L6 | Bit 7 of the vertex-flag grid selects the split diagonal of each grid quad (§3.6) | OBS (65.5 M sampled layer triangles, 100 %) | Confirmed |
| L7 | Surface layers: material id, 0x41 alpha-mask DID, triangle range into a shared index list over grid vertices (§3.8) | OBS | Confirmed (layout); meaning of the material id OPEN |
| L8 | Index-list vertex numbers are **Y-major** (33·y + x), the transpose of the height array (§3.8.3) | OBS (two independent tests) | Confirmed |
| L9 | The ceiling grid is present exactly when the block map says `Landblock_UseCeiling` = 1 (§3.5) | ER (caves use an inverted terrain ceiling); OBS (6,867 of 6,867) | Confirmed |
| L10 | Landblock info layout (§4) | LC; OBS (49,636 records, zero leftover bytes) | Confirmed |
| L11 | Landblock info ends with a u32 only when the record's `stamp` ≥ 31 (§4.10) | OBS (perfect split) | Confirmed |
| L12 | Cell geometry DIDs are 0x01 records (§4.5) | OBS (1,033 distinct, all resolve) | Confirmed. Answers `setup.md` §2.2 OPEN. |
| L13 | Block map = list of (key, u16 index) into the region's 0x18 descriptor (§5) | LC; OBS (390,150 records) | Confirmed |
| L14 | The region's 0x18 descriptor is named by its 0x0F region descriptor (§5.1) | ENV; LC (hard-coded table that agrees); OBS | Confirmed |
| L15 | Landblock properties = DID + property set; minimap tile is a 200 × 200 JPEG at 1.25 px per unit, north up (§6) | LC (property names, tiling); OBS (60,450 records; 120 tiles) | Confirmed |
| L16 | Kind 5 exists only at XX, YY divisible by 3 and holds 99 × 99 five-byte samples (§7) | ER (3 × 3 groups); OBS (33,159) | Confirmed (framing); meaning OPEN |

---

<a id="s2"></a>
## 2. Records, archives and regions

<a id="s2-1"></a>
### 2.1 DID shape (Confirmed)

DID = `0x80` · 2²⁴ + `K` · 2²⁰ + `R` · 2¹⁶ + `XX` · 2⁸ + `YY` (`records.md` §2.2).

| Field | Bits | Meaning |
|---|---|---|
| `K` | 20–23 | Record kind (table below) |
| `R` | 16–19 | Region number. Observed 1, 2, 3, 4, 5 and 14 (written 0xE). |
| `XX` | 8–15 | Landblock X, 0–254 |
| `YY` | 0–7 | Landblock Y, 0–254 |

| `K` | Record | Archive | §  |
|---|---|---|---|
| 0 | Landblock data (heights, terrain surface) | `client_cell_R.dat` | §3 |
| 1 | Block map (per-landblock keys) | `client_map_R.dat` | §5 |
| 2 | Landblock info (cells, static entities, links, weenies) | `client_cell_R.dat` | §4 |
| 4 | Landblock properties (minimap, light maps) | `client_cell_R.dat` | §6 |
| 5 | Unidentified 3 × 3-group record | `client_cell_R.dat` | §7 |

No other `K` occurs. `R` selects the archive pair: region *R* lives in `client_cell_R.dat`
and `client_map_R.dat` with *R* written in decimal (region 14 → `client_cell_14.dat`).

<a id="s2-2"></a>
### 2.2 Regions (Confirmed)

| Region *R* | Archives present | Landblock records | Region descriptor (ENV §3) |
|---|---|---|---|
| 1, 2, 3, 4, 5, 14 | cell_R, map_R | yes | 0x0F000000, 01, 02, 05, 08, 06 respectively |
| 6 | none | no | 0x0F000009 exists |
| 15 | cell_15, map_15 | **none**: only the 4 archive metadata records (0xFF…) | none (its DID-map target 0x0F000007 is absent) |

The region number ↔ region descriptor mapping is the REGION DID map 0x28000008 (key =
region number, value = 0x0F DID; `records.md` §8, ENV §3). A client reads it at run time.
Coordinates of different regions overlap (`coordinates.md` §4), so a landblock is always
addressed by the triple (R, X, Y).

Besides the 0x80 family, each `client_cell_R.dat` also holds about 100 records with prefix
0x27 (13 in region 14) and every archive holds 4–5 metadata records 0xFFFF000n. Neither is
part of a landblock bundle (`records.md` §2.1).

<a id="s2-3"></a>
### 2.3 Which landblocks exist (Confirmed)

| Region | K=0 data | K=1 block map | K=2 info | K=4 properties | K=5 | Data blocks with constant height | Non-constant data blocks |
|---|---|---|---|---|---|---|---|
| 1 | 65,025 | 65,025 | 9,701 | 13,905 | 6,141 | 31,716 | 33,309 |
| 2 | 65,025 | 65,025 | 11,508 | 16,377 | 5,913 | 35,665 | 29,360 |
| 3 | 65,025 | 65,025 | 8,846 | 7,367 | 6,673 | 31,431 | 33,594 |
| 4 | 65,025 | 65,025 | 11,479 | 12,939 | 6,823 | 28,378 | 36,647 |
| 5 | 65,025 | 65,025 | 5,837 | 8,347 | 7,068 | 12,453 | 52,572 |
| 14 | 65,025 | 65,025 | 2,265 | 1,515 | 541 | 62,529 | 2,496 |
| **Total** | 390,150 | 390,150 | 49,636 | 60,450 | 33,159 | | |

Rules:

- **LB-E1**: kinds 0 and 1 are **dense**: every (X, Y) with 0 ≤ X, Y ≤ 254 has one, in
  every landblock region (255 × 255 = 65,025). X or Y = 255 never occurs.
- **LB-E2**: kinds 2, 4 and 5 are **sparse**. A missing record means "nothing of this
  kind", not an error.
- **LB-E3**: a landblock-info record exists **exactly** when the landblock-data header names
  it (§3.2, 49,636 of 49,636).
- **LB-E4**: kind 5 exists only where X and Y are both multiples of 3 (33,159 of 33,159).
- "Constant height" blocks have all 1,089 samples equal. Most are 0 (void or ocean); others
  hold a flat level such as stored 10,485 (≈ 320 units) or 4,915 (≈ 150 units). Between
  97 and 1,242 landblock-info records per region (1 – 11 %) sit on constant-height blocks
  (dungeon and instance blocks).

A world walker treats the extent of non-constant blocks plus the existence of kind 2 and
kind 4 as "the playable area". There is no explicit per-region bounds record in this family.

<a id="s2-4"></a>
### 2.4 Directory stamps (Confirmed)

The directory entry `stamp` (`dat-container.md` §5.2) changes two layouts (§3.9, §4.10).

| Kind | Stamps observed (record count) |
|---|---|
| 0 | 20 (43,962), 21 (104), 22 (74,070), 27 (11,914), 38 (130,050), 39 (130,050) |
| 1 | 20 (122,297), 21 (133), 22 (6,284), 27 (1,336), 38 (130,050), 39 (130,050) |
| 2 | 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29 (7,106 together); 31, 32, 33, 38, 39 (42,530 together) |
| 4 | 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29, 31, 32, 33, 38, 39 |
| 5 | 38 (24,434), 39 (8,725) |

A decoder therefore takes the record's `stamp` as an input next to the payload, as for
meshes. §3.9 and §4.10 give fallbacks for payloads without a stamp.

---

<a id="s3"></a>
## 3. Landblock data `0x800RXXYY` (Confirmed, 390,150 of 390,150)

<a id="s3-1"></a>
### 3.1 Top-level order

All fields are little-endian and packed. "VLE" is `records.md` §1.2.

| Order | Part | Present when | § |
|---|---|---|---|
| 1 | Header (16 bytes) | always | §3.2 |
| 2 | Height grid: 1,089 × u16 | always | §3.3 |
| 3 | `has_ceiling` u8 (0 or 1) | always | §3.5 |
| 4 | Ceiling grid: u32 count (1,089), count × u16 | `has_ceiling` = 1 | §3.5 |
| 5 | `has_ceiling_tree` u8 (0 or 1) | `has_ceiling` = 1 | §3.5 |
| 6 | Ceiling collision tree block | `has_ceiling_tree` = 1 | §3.9 |
| 7 | Vertex-flag grid: u32 count (1,089), count × u8 | always | §3.6 |
| 8 | Coarse grid: u32 count (289), count × u32 | always | §3.7 |
| 9 | Terrain surface chunk | always | §3.8 |
| 10 | Ceiling surface chunk | `has_ceiling` = 1 | §3.8 |
| 11 | `has_terrain_tree` u8 (0 or 1) | always | §3.9 |
| 12 | Terrain collision tree block | `has_terrain_tree` = 1 | §3.9 |
| 13 | `has_boxes` u8 (0 or 1) | always | §3.10 |
| 14 | Box list: u32 count *b*, *b* × 24 bytes | `has_boxes` = 1 | §3.10 |
| 15 | *(end)* | | zero leftover bytes in every record |

The smallest record (no ceiling, no layers, no trees, no boxes) is 4,456 bytes:
16 + 2,178 + 1 + 4 + 1,089 + 4 + 1,156 + 8. Readers that need only heights may stop after
part 2, but a validating decoder walks to the end.

<a id="s3-2"></a>
### 3.2 Header (Confirmed)

| Offset | Type | Meaning | Observed |
|---|---|---|---|
| 0 | DID | Own DID | always |
| 4 | DID | The landblock-info DID of this landblock (own DID + 0x00200000), or 0 when there is no landblock info | 49,636 non-zero, all equal to own + 0x00200000 and all existing |
| 8 | u32 | Reserved | always 0 |
| 12 | u32 | Height-sample count | always 1,089 (33 × 33) |

A reader rejects a count other than 1,089 as malformed (no other value exists; the grid
geometry below assumes 33 × 33).

<a id="s3-3"></a>
### 3.3 Height grid (Confirmed)

1,089 u16 values. Sample (i, j), with 0 ≤ i, j ≤ 32, is at array index **33 · i + j**, where
**i runs along X** (east) and **j along Y** (north). Sample (i, j) sits at block-local
position (5 · i, 5 · j): the spacing is 5 units and the grid covers the closed square
0 ≤ x, y ≤ 160.

Evidence for the axis assignment:

- **Seams.** For every pair of horizontally adjacent non-constant blocks, samples
  (32, j) of block (X, Y) equal samples (0, j) of block (X + 1, Y) for all j (180,329
  pairs, 100 %); samples (i, 32) of (X, Y) equal (i, 0) of (X, Y + 1) (179,620 pairs, 100 %).
  The transposed reading fails on 98.8 % of pairs.
- **Ground contact.** Placed entities (ED) rest on the terrain far more often under this
  reading: 491,515 of 2,479,912 in-block entities lie within 1 cm of the triangulated
  surface (§3.11) against 79,615 with X and Y swapped.

So **edge samples are shared** with the neighbour: a landblock and its east (north)
neighbour store identical values on their common edge. Terrain built from each block's own
33 × 33 samples is crack-free at boundaries without stitching heights; normals at an edge
still need the neighbour's second row (`crates/terrain`).

<a id="s3-4"></a>
### 3.4 Height scale (Confirmed)

**Height in world units = stored value / 32.768** (equivalently, stored value · 1,000 /
32,768). Stored 0 is height 0; 32,768 is 1,000 units; the u16 maximum 65,535 is 1,999.97
units.

Evidence:

- The median ratio of placed-entity *z* (absolute height, ED §4.4) to the bilinear terrain
  sample under the entity is 0.030518 in every region (1 / 32.768 = 0.0305176), over
  2,479,912 entities.
- With the scale and the triangulation of §3.11, 491,515 entities (19.8 %) sit within 1 cm
  and 1,221,684 (49.3 %; 42.5 % in region 4 to 60.5 % in region 1) within 25 cm of the
  terrain; the rest are on buildings, bridges, in water or deliberately sunk.
- Constant-height blocks use round heights under this scale: stored 4,915 → 149.99,
  10,485 → 319.98, 32,765 → 999.9.
- LC divides by 32,768, which gives the same shape in thousands of units; jtauber's
  logarithmic formula is a visualisation, not a scale.

Observed ranges of stored values: region 1: 0–65,482; 2: 0–65,502; 3 and 4: 0–49,481;
5: 0–54,521; 14: 0–65,481. Heights are never negative.

> **OPEN:** The roadmap asks for validation against in-game `/loc` heights at known spots.
> The scale is fixed by the data above; a table of in-game test points (recorded as
> region, landblock, local x, y and the `oz` value shown by the original client) should still
> be added here by whoever can observe the original client. Expected agreement: within a few
> centimetres on open ground.

<a id="s3-5"></a>
### 3.5 Ceiling grid (Confirmed layout; meaning Confirmed by correlation)

Present when `has_ceiling` = 1 (6,867 records: region 1: 971, 2: 4,303, 3: 99, 4: 826,
5: 305, 14: 363).

- `count` is always 1,089, and the u16 values use the **same index order and scale** as the
  height grid (§3.3, §3.4).
- The ceiling is present **exactly** in the landblocks whose block map (§5) gives
  `Landblock_UseCeiling` = 1 (6,867 of 6,867; the 383,283 others have value 0). This is the
  "inverted terrain ceiling" of cave regions (ER §3).
- Value 0 means "no ceiling at this vertex" (1,885,095 of 7,478,163 ceiling samples,
  25 %). Non-zero values lie above the floor height at 39 % of those vertices and at or below
  it at 61 % (where the ceiling is hidden inside the ground).
- Many ceiling grids are flat: of the 5,276 in regions 1 and 2, 1,571 have a single
  non-zero value and 1,030 are all zero.

`has_ceiling_tree` = 1 in 1,095 records; a collision tree block (§3.9) follows. A ceiling is
rendered with the ceiling surface chunk (§3.8) and faces downwards.

> **OPEN:** Winding and facing of ceiling triangles, and the exact rule for "ceiling below
> floor" vertices (cull, clamp or render). Not needed for an outdoor walker.

> **OPEN (implementation census, 2026-10-04):** Tessera's decoder census
> (`crates/formats/tests/landblock_real_data.rs`, every ceiling of the install) found that
> of 5,310,848 ceiling-layer triangles **none** is counter-clockwise seen from above: every
> stored ceiling triangle is clockwise from above, i.e. it faces down, as expected. But only
> 4,999,408 (94.1 %) use the diagonal that bit 7 of the vertex-flag grid (§3.6) gives for
> their quad; 311,440 use the other diagonal (in 986 records). So HM6 (§3.12) holds for
> terrain chunks only. Which rule picks a ceiling quad's diagonal (a separate flag, the
> ceiling heights, or only the stored triangles) is open; renderers mesh ceilings from the
> stored triangles, not from bit 7. Every ceiling triangle does lie in one grid quad.

<a id="s3-6"></a>
### 3.6 Vertex-flag grid (Confirmed layout)

u32 count (always 1,089), then one byte per height sample, in the **same order as the
height grid** (index 33 · i + j, i along X).

**Bit 7 (0x80): split diagonal of the quad whose south-west corner is this sample.** For
the quad with corners (i, j) and (i + 1, j + 1), 0 ≤ i, j ≤ 31:

| Bit 7 of byte (i, j) | Diagonal | Triangles (corner samples) |
|---|---|---|
| set | from (i, j) to (i + 1, j + 1) (south-west to north-east) | (i, j), (i + 1, j), (i + 1, j + 1) and (i, j), (i + 1, j + 1), (i, j + 1) |
| clear | from (i + 1, j) to (i, j + 1) (south-east to north-west) | (i, j), (i + 1, j), (i, j + 1) and (i + 1, j), (i + 1, j + 1), (i, j + 1) |

Triangles are listed counter-clockwise seen from above (+Z), matching the stored layer
triangles (§3.8.3). Evidence: every triangle of every surface layer (65,471,762 in a 1-in-7
sample) uses exactly the diagonal this bit predicts. Bytes in row i = 32 or column j = 32
carry no quad.

**Bits 0–6:** observed values 0, 1, 126 and 127 only. Per record, the set of low values is
{127} in most records with stamp 20–27, {0} in most with stamp 38–39, and mixed in a few
hundred; 126 occurs only together with a ceiling. Whole bytes observed: 0, 1, 126, 127,
128, 129, 254, 255.

> **OPEN:** Meaning of bits 0–6. Hypotheses: a per-vertex lighting or blend value, or an
> editor flag. A renderer ignores them.

<a id="s3-7"></a>
### 3.7 Coarse grid (Confirmed layout; meaning OPEN)

u32 count (always 289 = 17 × 17), then 289 × u32. The grid has one entry per 10 units
(every second height sample), presumably in the height grid's order.

| Observation (all 390,150 records) | Value |
|---|---|
| Bits 16–31 | always 0 |
| Byte 0 (bits 0–7) | 0–94; distinct sets per region (region 1 uses up to 94) |
| Byte 1 (bits 8–15) | 0–72 |
| Records whose 289 entries are all 0 | region 1: 43,976; 2: 52,988; 3: 53,071; 4: 47,402; 5: 50,675; 14: 63,924 |
| Distinct values per record | 1 in most; up to 16 |

> **OPEN:** Meaning (terrain type and scenery/frill type per 10-unit vertex are the leading
> hypotheses; ER §3 describes grass "frills" as landscape-driven), the table the two bytes
> index (candidates: the per-region 0x16 and 0x19 tables, ENV §4), and the index order.
> Not needed for geometry or for texturing with §3.8.

<a id="s3-8"></a>
### 3.8 Surface chunks (terrain and ceiling) (Confirmed layout)

A **surface chunk** lists the texture layers of one surface (the terrain, or the ceiling)
and the triangles each layer covers.

<a id="s3-8-1"></a>
#### 3.8.1 Chunk layout

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `layer_count` *n* | u32 | 0 – 80 observed |
| 2 | layers | *n* × 17 bytes | §3.8.2 |
| 3 | `index_count` *m* | VLE | Number of u16 vertex numbers that follow |
| 4 | indices | *m* × u16 | Triangle list over the grid vertices (§3.8.3) |
| 5 | `quad_count` *q* | VLE | 0 or 1,024 |
| 6 | quad grid | *q* × u16 | §3.8.4 |

Facts (terrain chunks / ceiling chunks):

- *n* = 0 in 303,424 / 4,512 chunks; then *m* = 0 and *q* = 0.
- *q* = 1,024 **exactly when** *n* > 0 (86,726 / 2,355 chunks).
- *m* = 3 × (sum of `prims` over the layers) in every chunk.
- *n* for terrain chunks with layers: 1 – 80, most often 2 – 20.

<a id="s3-8-2"></a>
#### 3.8.2 Layer (17 bytes)

| Offset | Field | Type | Meaning |
|---|---|---|---|
| 0 | `material` | u32 | Material/layer id. 0 – 1,420 in terrain chunks (1,409 distinct values); 390 distinct values in ceiling chunks. 0 is the most common single value. |
| 4 | `mask` | DID | A render surface (prefix 0x41) holding the layer's **alpha mask**, or 0 |
| 8 | `whole` | u8 | 0 or 1 (§ below) |
| 9 | `start` | u16 | First index of this layer's triangles in the chunk's index list |
| 11 | `prims` | u16 | Number of triangles |
| 13 | `min_vertex` | u16 | Smallest vertex number used by those triangles |
| 15 | `vertex_span` | u16 | Number of vertex numbers spanned, from `min_vertex` |

The last four fields have the shape of an indexed draw call (start index, primitive count,
minimum vertex, vertex count). Checked in every layer with triangles (696,749 terrain,
10,427 ceiling): the triangles `start` … `start` + 3·`prims` − 1 are inside the index list,
and every vertex number they use is in [`min_vertex`, `min_vertex` + `vertex_span`).

Layer kinds (terrain counts; ceiling layers follow the same pattern):

| `whole` | `prims` | `mask` | Count | Reading |
|---|---|---|---|---|
| 0 | > 0 | non-zero | 629,340 | Draw these triangles with this mask. **Every such mask exists** in the surface archives. |
| 0 | > 0 | 0 | 67,409 | Draw these triangles fully opaque (no mask). |
| 0 | 0 | non-zero | 142,091 | No geometry. **None of these masks exists** in the install. Skip. |
| 0 | 0 | 0 | 55,944 | No geometry, no mask. Skip. |
| 1 | 0 | non-zero (resolves) or 0 | 8,084 + 9,170 | No index range; probably covers the whole block (a base layer). |

Masks sampled (43): A8 format, 128 × 128 (42) or 256 × 256 (1) (`records.md` §5).

> **OPEN:** (1) What `material` indexes. It is not the coarse-grid byte (§3.7, range 0–94).
> The per-region 0x19 table (ENV §4: 317–1,422 named terrain layers with texture lists) is
> the leading candidate: its size matches the observed maximum of 1,420. (2) The draw order
> and blend rule between layers (stored order is the natural guess). (3) The orientation of
> the alpha mask over the block (which texel row is north). (4) The exact meaning of
> `whole` = 1. Until (1) is settled, `crates/terrain` shades by height and slope as the
> roadmap allows.

<a id="s3-8-3"></a>
#### 3.8.3 Index list and its vertex numbering

Each u16 is a **vertex number** *v* of the 33 × 33 grid, 0 ≤ *v* < 1,089 (all values in
range). Three consecutive numbers form one triangle.

**Vertex numbers are Y-major: v = 33 · y_index + x_index**, the transpose of the height
array's order. Vertex *v* therefore has x_index = *v* mod 33, y_index = ⌊*v* / 33⌋, and
its height is the height-grid element 33 · (*v* mod 33) + ⌊*v* / 33⌋.

Evidence (two independent tests):

- With this reading, the diagonal used by every stored triangle agrees with bit 7 of the
  vertex-flag grid read in height-grid order (65,471,762 of 65,471,762 sampled triangles).
  Reading *v* in height-grid order instead agrees for only 96.6 %.
- Ceiling-chunk triangles should use vertices that have a ceiling. Of 15.87 M ceiling
  triangle vertices on non-flat ceilings, 217,960 have a ceiling sample only under this
  reading and 21,535 only under the other (the rest agree under both).

Under this reading every stored triangle is **counter-clockwise seen from above**
(458,061,498 terrain triangles, all of them). Each triangle covers half of one 5 × 5 quad.

<a id="s3-8-4"></a>
#### 3.8.4 Quad grid

1,024 u16 values, one per quad (32 × 32). Every value is the `material` of one of the
chunk's layers (86,726 of 86,726 chunks).

Probable order: quad (column qx, row qy) at index 32 · qy + qx, the same Y-major order as the
index list (a layer's own triangles land on quads holding that layer's `material` 31.5 % of
the time under this order, 23.5 % under the transpose).

> **OPEN:** Which layer the quad grid records (topmost, dominant, or the one for footstep
> sounds and physics surfaces) and a firm orientation.

<a id="s3-9"></a>
### 3.9 Collision tree blocks (Confirmed layout)

Both the ceiling tree (part 6) and the terrain tree (part 12) have the layout of the
mesh collision tree block **without** its leading `has_code` byte (that role is played by
`has_ceiling_tree` / `has_terrain_tree`):

| Field | Type |
|---|---|
| `origin` | 3 × f32 |
| `scale` | f32 |
| `code_length` *n* | u32 |
| `code` | *n* bytes, opaque |
| `build_byte` | u8, value 1. Present **iff the record's `stamp` ≥ 38** (`mesh.md` §10.4) |

Observed: `has_terrain_tree` = 1 in 58,546 records (49,919 of them with stamp ≥ 38 and a
`build_byte`); `has_ceiling_tree` = 1 in 1,095. The stamp rule held in every record. The
`origin` is about −0.05 on the axes where the block starts at 0, and `code` starts with
byte 0x28 in the samples looked at, as for meshes (`mesh.md` §10.2).

Fallback without a stamp: after the code, a ceiling tree is followed by the vertex-flag
count u32 1,089 (bytes 41 04 00 00); if the next four bytes are not that, a `build_byte`
is present. For the terrain tree, a `build_byte` is present exactly when consuming it lets
the remaining bytes parse as parts 13–14 with nothing left over.

Tessera does not interpret the code (`mesh.md` §10.2). Physics may build its own structure
from the heights.

<a id="s3-10"></a>
### 3.10 Box list (Confirmed layout; meaning OPEN)

`has_boxes` = 1 in 2,256 records, **all of them with a ceiling**. Then u32 count (1 – 38;
2 in 1,237 records) and that many entries of 6 × f32. In the inspected entries the six
floats read as (min x, min y, min z, max x, max y, max z) in block-local units: thin strips
such as x 0–160 by y 40–45, z often 0. 7,850 of 8,869 entries have some value outside
0–160.

> **OPEN:** Meaning (cave-mouth or ceiling-cut volumes are guesses).

<a id="s3-11"></a>
### 3.11 Terrain height at a point (algorithm in words)

To get the terrain height at block-local (x, y), 0 ≤ x, y ≤ 160:

1. i = ⌊x / 5⌋ and j = ⌊y / 5⌋, each clamped to at most 31. fx = x / 5 − i, fy = y / 5 − j.
2. Read the four corner heights h(i, j), h(i + 1, j), h(i, j + 1), h(i + 1, j + 1) from
   the height grid (index 33 · i + j), converted with §3.4.
3. Read bit 7 of the vertex-flag byte at index 33 · i + j.
4. If set (south-west/north-east diagonal): when fx ≥ fy use the plane through corners
   (i, j), (i + 1, j), (i + 1, j + 1); otherwise the plane through (i, j), (i + 1, j + 1),
   (i, j + 1).
5. If clear (south-east/north-west diagonal): when fx + fy ≤ 1 use the plane through
   (i, j), (i + 1, j), (i, j + 1); otherwise the plane through (i + 1, j), (i + 1, j + 1),
   (i, j + 1).
6. Evaluate the chosen plane at (x, y).

The triangulated surface fits placed entities somewhat better than bilinear interpolation
(within 1 cm: 19.8 % against 16.1 %; region 5 is the exception at 15.6 % against 16.0 %).
The triangulation itself is certain from the stored layer triangles (§3.8.3). The same triangulation is what
a renderer meshes: two triangles per quad, diagonal per bit 7, counter-clockwise from above.

<a id="s3-12"></a>
### 3.12 Invariants (Confirmed, every record)

- **HM1**: the payload is consumed exactly by §3.1.
- **HM2**: header offset 8 is 0, offset 12 is 1,089; the header's info DID is 0 or own DID
  + 0x00200000.
- **HM3**: ceiling-grid and vertex-flag counts are 1,089; coarse-grid count is 289.
- **HM4**: all flag bytes (`has_*`, `whole`) are 0 or 1.
- **HM5**: per chunk: *q* ∈ {0, 1,024}; *q* = 1,024 ⇔ *n* > 0; *m* = 3 · Σ `prims`; every
  index < 1,089; every layer's triangles lie within its `start`/`prims` window and its
  vertex window.
- **HM6**: every triangle of a layer index list lies in one grid quad. In **terrain** chunks
  it also uses the diagonal given by §3.6 and is counter-clockwise from above; ceiling
  triangles are clockwise from above and do not always follow bit 7 (§3.5 census).
- **HM7**: edge samples equal the neighbour's (§3.3).

<a id="s3-13"></a>
### 3.13 What remains unknown in landblock data

| Part | Status |
|---|---|
| Heights, scale, axis order, triangulation | Known |
| Ceiling grid and when it exists | Known; facing OPEN |
| Vertex flags bit 7 | Known |
| Vertex flags bits 0–6 | OPEN (§3.6) |
| Coarse 17 × 17 grid | Layout known; meaning and order OPEN (§3.7) |
| Surface layers | Layout and geometry known; `material` table, blend order, mask orientation, `whole` OPEN (§3.8) |
| Collision trees | Layout known; code opaque (as for meshes) |
| Box list | Layout known; meaning OPEN (§3.10) |

---

<a id="s4"></a>
## 4. Landblock info `0x802RXXYY` (Confirmed, 49,636 of 49,636)

<a id="s4-1"></a>
### 4.1 Layout

| Order | Field | Type | Present when | § |
|---|---|---|---|---|
| 1 | did | DID | always | own DID |
| 2 | `flags` | u32 | always | bit 0 cells, bit 1 cross links, bit 2 header indices. Observed 1 (39,252), 3 (9,704), 5 (636), 7 (44): **bit 0 is always set**. |
| 3 | `size_hint` | u32 | always | Below the payload length in 49,625 records, above it in 11. Meaning OPEN; ignore. |
| 4 | cross links | TSize count, entries | `flags` bit 1 | §4.2 |
| 5 | header indices | TSize count, entries | `flags` bit 2 | §4.3 |
| 6 | cells | TSize count, entries | `flags` bit 0 | §4.4 |
| 7 | reserved | u32 | always | 0 in every record |
| 8 | static entities | u32 count, entity descriptors | always | §4.5 |
| 9 | links | u32 count, entries | always | §4.6 |
| 10 | properties | embedded property set | always | §4.7 |
| 11 | reserved | u32 | always | 0 in every record |
| 12 | weenies | TSize count, entries | always | §4.8 |
| 13 | `trailer` | u32 | `stamp` ≥ 31 | §4.9 |
| 14 | *(end)* | | | zero leftover bytes |

Totals: 104,271 cells, 3,253,862 static entities, 1,324,469 links, 3,768,840 weenies.

<a id="s4-2"></a>
### 4.2 Cross links (24 bytes each)

| Field | Type | Meaning (LC names) |
|---|---|---|
| `from_index` | u32 | |
| `from_info` | DID | A landblock-info DID |
| `to_info` | DID | A landblock-info DID |
| `link_type` | u32 | Same code space as §4.6 |
| `to_index` | u32 | |
| `mask` | u32 | Large varying values; meaning OPEN |

Present in 9,748 records.

<a id="s4-3"></a>
### 4.3 Header indices

Each entry: u16 `index`, then u32 count *k*, then *k* × u16. All listed u16 values are cell
indices of the same record (1,288 entries); the leading `index` is a cell index in 608 of
them. Present in 680 records. Meaning OPEN (a visibility or grouping list over cells is
likely).

<a id="s4-4"></a>
### 4.4 Cells

Cells are interior spaces (rooms, caves, dungeon pieces) placed in the landblock.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `index` | u16 | Cell number. 82,996 below 0x100, 21,275 at or above. |
| 2 | `position` | Position (`property-system.md` §4.6) | Always carries region, landblock (= this record's) and cell (= `index`). x, y, z are present in all cells **but 2** (flag byte 0x4B: region, landblock, cell and the payload-less 0x40 bit only; cell 0x2 of 0x8021FAF9 and of 0x8021FAFA). Orientation, (w, x, y, z) (`coordinates.md` §11.1), in 93,172 cells. Flag bytes seen: 0x3B (54,917), 0x7B (38,255), 0x1B (5,906), 0x5B (5,191), 0x4B (2). |
| 3 | `cell_flags` | u32 | Bit 2 (0x04) always set; bit 5 (0x20) in 92,133; bit 3 (0x08) in 87,512; bit 4 (0x10) in 9,493. Other bits never set. |
| 4 | `geometry` | DID | Always a **0x01** record in `client_general.dat` (1,033 distinct, all exist) |
| 5 | `neighbour_count` *k* | u32 | 0 – 10 |
| 6 | neighbours | *k* × 15 bytes | u32 ordinal (always equal to its position 0…*k*−1), u16 neighbour cell (always a cell of this record), u16 back-reference, 3 × u16 (always 0), u8 flag (1 in 392,318, 0 in 1,524) |
| 7a | `index_echo` | u16 | If `cell_flags` bit 5: equals `index` (all) |
| 7b | list A | u32 count, count × u16 | always; every value is a cell index of this record |
| 7c | list B | u32 count, count × u16 | always; values are not always local cell indices (67,836 cells) |
| 7d | `extra` | u16 | If `cell_flags` bit 5: 1 – 11 or 0xFFFF |
| 8 | properties | embedded property set | If `cell_flags` bit 2 (always). Holds `Render_AdjustableScale` and `Physics_AdjustableScale` in every cell, often `Appearance_InstanceList`, `Dungeon_DID`, interior fog and light values, `Ambient_*` |

Cell positions use the landblock frame but are not confined to 0–160 (`coordinates.md`
§6). Rendering cells needs the 0x01 geometry format, which is not specified yet.

> **OPEN:** layout of the 0x01 cell geometry records (portals, PVS); meaning of
> `cell_flags` bits 3, 4, 5, the neighbour back-reference and flag, lists A and B and
> `extra`. Interiors are after M3 in the roadmap.

> **OPEN:** how a cell whose `position` has no x, y, z (flag 0x4B, 2 cells) is placed:
> at the landblock origin, at the position of a neighbour or portal, or from its geometry.
> A loader must not assume the offset is present; until this is known it should skip
> such a cell (or place it from another source) rather than put it at (0, 0, 0).

<a id="s4-5"></a>
### 4.5 Static entities

A u32 count, then that many **static entity descriptors**. Their layout, flags, frames,
rotation and scale are normative in `entity-descriptor.md`. A landblock-info decoder must
decode them fully (their length depends on their flags and embedded property sets) to reach
the links.

<a id="s4-6"></a>
### 4.6 Links

u32 count, then entries:

| Order | Field | Type |
|---|---|---|
| 1 | `name` | Pascal string (often empty) |
| 2 | `iid` | u64 |
| 3 | `to_iid` | u64 |
| 4 | `from_iid` | u64 |
| 5 | `link_type` | u32 |
| 6 | properties | embedded property set |
| 7 | `cross` | u8, 0 or 1 |
| 8 | coda | 4 × u32 (index, info DID, info DID, index) if `cross` = 1 |

`cross` = 1 in 79,147 links; both coda DIDs are landblock-info DIDs in all, and one of them
is this record's in all but 6.

`link_type` codes observed, with LC's labels (Reported) and counts: 7 Generator_PositionSet
(418,989), 16 Path (266,194), 8 Generator (237,228), 12 Script (194,932), 9 Segment (62,398),
0x1000000C House_Decoration (32,627), 0x10000013 House_PropertyDriver (32,302), 2 Patrol
(28,717), 0x10000019 Phasing (23,376), 19 AIRemoteDetector (18,352), 1 PatrolOnce (6,723),
0x1000000B House_Permission (1,044), 0x1000000D House_Landmark (790), 11 FlyingPatrol (251),
0x1000000F House_ForSaleSign (248), 0x10000010–12 House_Storage1–3 (220, 56, 16),
23 SegmentBridge (6). A world walker without gameplay ignores links.

<a id="s4-7"></a>
### 4.7 Landblock property set

An embedded property set (`property-system.md` §5). Empty in 48,093 records. Otherwise it
holds dungeon and interior settings such as `Dungeon_DID`, `InteriorAmbientLight`,
`InteriorFog*`, `Environment_LocalSky`, `Ambient_SoundEnum_BlockMapOverride`.

<a id="s4-8"></a>
### 4.8 Weenies

TSize count, then entries of u64 `iid` followed by an embedded property set. These are
server-side object templates (spawn generators and the like; LC follows
`Generator_ProfileDID`). An offline walker skips them.

<a id="s4-9"></a>
### 4.9 Trailer (Confirmed rule)

A final u32 is present **iff the directory `stamp` ≥ 31**:

| Stamps | Records | Trailer |
|---|---|---|
| 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29 | 7,106 | absent |
| 31, 32, 33, 38, 39 | 42,530 | present; values 0 – 1,028 |

Stamp 30 was not observed. Fallback without a stamp: the trailer is present exactly when 4
bytes remain after the weenies. Its value is not a count of cells, entities, links or
weenies in most records. Meaning OPEN.

<a id="s4-10"></a>
### 4.10 Invariants (Confirmed)

- **LI1**: own DID first; zero leftover bytes after §4.1.
- **LI2**: both reserved u32 are 0.
- **LI3**: each cell's `position` names this landblock and its `index`; each neighbour's
  ordinal equals its position in the list; neighbour cells and list-A values are cells of
  this record; the three neighbour u16 are 0; `index_echo` = `index`.
- **LI4**: every cell `geometry` DID has prefix 0x01 and exists.

---

<a id="s5"></a>
## 5. Block maps `0x801RXXYY` and descriptors (prefix 0x18)

A block map assigns each landblock a value for about 24 per-landblock properties (area,
music, sky, level cap, …) by index into a per-region **descriptor** table.

<a id="s5-1"></a>
### 5.1 Finding the descriptor (Confirmed)

Region number → REGION DID map 0x28000008 → region descriptor 0x0F… → the DID in table
slot 7 (offset 102, ENV §3.1) → descriptor 0x18…. In this install:

| Region | Region descriptor | Block-map descriptor |
|---|---|---|
| 1 | 0x0F000000 | 0x18000000 |
| 2 | 0x0F000001 | 0x18000014 |
| 3 | 0x0F000002 | 0x18000015 |
| 4 | 0x0F000005 | 0x1800001A |
| 5 | 0x0F000008 | 0x1800001A |
| 14 | 0x0F000006 | 0x18000014 |
| (6) | 0x0F000009 | 0x1800001E |

This agrees with LC's hard-coded table. Regions may share a descriptor. Never hard-code it.

<a id="s5-2"></a>
### 5.2 Descriptor record layout (Confirmed, 5 of 5, zero leftover bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | Own DID |
| 2 | reserved | u32 | 0 |
| 3 | count *n* | TSize | 24 or 25 entries (hint 1) |
| 4 | entries | *n* × entry | |

Entry:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `pid` | u32 | Property ID of the values |
| 2 | `pid_echo` | u32 | Equals `pid` (all) |
| 3 | `key` | u32 | The block-map key that selects this entry |
| 4 | `flag` | u8 | 1, except 0 for the one float-valued property (`Encounter-Density`). Meaning OPEN. |
| 5 | `value_count` *c* | u32 | 1 – 281 |
| 6 | values | *c* × (u32 PID, value) | Array-element form (`property-system.md` §5.2): PID (always = `pid`), then the value encoded by that PID's type |

The value list is a **palette**: block maps store an index into it.

<a id="s5-3"></a>
### 5.3 Block-map record layout (Confirmed, 390,150 of 390,150)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | Own DID |
| 2 | count *n* | TSize | Hint 1; *n* = 28 (region 1), 29 (regions 2, 3, 14), 30 (regions 4, 5) in **every** record |
| 3 | entries | *n* × (u32 `key`, u16 `index`) | In TSize bucket order by `key` (all records) |

Records therefore have a fixed size per region: 174, 180 and 186 bytes. The index is
**u16** in this install (LC reports a u8 variant for another server build; not present).

<a id="s5-4"></a>
### 5.4 Resolving a landblock's properties (algorithm in words)

1. Find the region's descriptor (§5.1) and build a map `key` → (`pid`, palette).
2. Read the block map of (R, X, Y).
3. For each (`key`, `index`): if the descriptor has the key and `index` < palette length,
   the landblock's value of `pid` is palette[`index`]. If the key is unknown or the index is
   out of range, the property has no value for this landblock (not an error).

Out-of-range indices observed: region 1: 759 (`Ambient_SoundEnum_BlockMapOverride`); regions
3, 4, 5: 6 each (`Ambient_MusicRegion`); region 2: 1 (`Environment_LocalSky`). Every
described key appears in every record.

<a id="s5-5"></a>
### 5.5 Keys and properties (Confirmed)

| Key | Property (internal name) | Palette size (region 1 descriptor) |
|---|---|---|
| 0x00000001 | `BlockAllocationRegion` | 281 |
| 0x00000002 | `Passability` | 2 |
| 0x00000005 | `Ambient_SoundEnum_BlockMapOverride` | 4 |
| 0x00000008 | `Encounter-Density` | 16 |
| 0x00000009 | `Landblock_UseCeiling` | 2 (§3.5) |
| 0x10000001 | `Death_MPDefeatInfo` | 8 |
| 0x10000002 | `Death_DefeatInfo` | 113 |
| 0x10000003, 04, 06 | `Death_Flippable_MPDefeatInfo_1`, `_2`, `_3` | 4 each |
| 0x10000005 | `MonsterLevel_DID` | 3 |
| 0x10000007 | `PVP_PVPPermissionStatus` | 4 |
| 0x10000008, 09, 0A | `Death_Flippable_PlayerDefeatInfo_1`, `_2`, `_3` | 4 each |
| 0x1000000B | `Death_FlippableDefeatInfo` | 4 |
| 0x1000000C | `Ambient_MusicType` | 67 |
| 0x1000000D | **`Area_DID`** (0x70 object, `wstate.md`) | 271 |
| 0x1000000E | `Mood_Landblock_Hope` | 11 |
| 0x1000000F | `Environment_LocalSky` (`environment.md`) | 250 |
| 0x10000010 | `Ambient_MusicRegion` | 15 |
| 0x10000011 | `Landblock_Temperature` | 7 |
| 0x10000014 | `MountedCombat_Landblock_Area` | 2 |
| 0x10000015 | `LoE_Landblock_Shadow` (descriptors 0x1800001A, 0x1800001E only) | 31 |
| 0x10000016 | `Player_LevelCap` | 16 |

For the world walker the relevant ones are `Area_DID` (on-screen area name via
`wstate.md`), `Ambient_MusicType` / `Ambient_MusicRegion` / `Ambient_SoundEnum_BlockMapOverride`
(audio), `Environment_LocalSky` (sky), and `Landblock_UseCeiling`.

<a id="s5-6"></a>
### 5.6 Keys without a descriptor entry

Every block map also carries keys 0x00000003, 0x00000004, 0x00000006, 0x00000007, and in
regions 2, 3, 4, 5 and 14 key 0x10000012, which no 0x18 descriptor lists. Their indices
vary: key 3 is always 0; key 4 takes up to 182 distinct values per region; key 6 up to 8;
key 7 up to 32; key 0x10000012 0 or 1.

> **OPEN:** Where keys 3, 4, 6, 7 and 0x10000012 are resolved. Candidates are the other
> per-region tables named by the region descriptor (ENV §4: 0x11, 0x12, 0x14, 0x16, 0x17,
> 0x19); key 4's range suggests a terrain or environment zone. Readers keep the raw
> (key, index) pairs.

<a id="s5-7"></a>
### 5.7 Invariants (Confirmed)

- **BM1**: block map = own DID + TSize + 6-byte entries, nothing left over.
- **BM2**: the key set is identical in all records of a region.
- **BM3**: descriptor entries have `pid_echo` = `pid` and every value's PID = `pid`.

---

<a id="s6"></a>
## 6. Landblock properties `0x804RXXYY` (Confirmed, 60,450 of 60,450)

<a id="s6-1"></a>
### 6.1 Layout

Own DID, then one embedded property set (`property-system.md` §5), nothing left over.

<a id="s6-2"></a>
### 6.2 Properties observed

| Property | Type | Records | Content |
|---|---|---|---|
| `UI_Map_GameMap` | struct | 59,831 | The minimap tile (§6.3) |
| ↳ `UI_Map_MapImage` | DID | 59,831 | 0x41 render surface (all exist) |
| ↳ `UI_Map_BlockOffsetX`, `UI_Map_BlockOffsetY` | int | 59,831 | Always this record's X and Y |
| ↳ `UI_Map_PixelOffsetX`, `UI_Map_PixelOffsetY` | int | 59,831 | Always (0, 200) |
| ↳ `UI_Map_Scale` | float | 59,831 | Always 1.25 (1,404 store 1.2500001) |
| ↳ `UI_Map_FogOfWar` | int | 59,831 | Always 0 |
| ↳ `UI_Map_FogOfWar_Color` | colour | 58,412 | (0, 0, 0, 216) or (0, 0, 0, 255) |
| ↳ `UI_Map_MapImageArray` | array of `UI_Map_MapImageStruct` | 2,018 | Alternative tiles: `UI_Map_ContentLayer` (int) + `UI_Map_MapImage` (DID); 4,618 elements, all images exist |
| `Landblock_LightMap` | DID | 4,692 | 0x41 surface, all exist |
| `Landblock_CeilingLightMap` | DID | 749 | 0x41 surface, all exist; 748 of 749 on blocks with a ceiling |

619 records have no `UI_Map_GameMap` (light maps only).

<a id="s6-3"></a>
### 6.3 Minimap tiles

- Every tile image checked (120) is a JPEG (format code 500, `records.md` §5) of **200 × 200
  pixels**: 160 units × `UI_Map_Scale` 1.25 pixels per unit.
- Orientation: **image columns run east (+X) and rows run south (−Y)**: the tile of (X + 1, Y)
  continues the right edge of (X, Y), and the tile of (X, Y + 1) continues its top edge
  (40 of 40 sampled pairs each, by edge-pixel continuity). The top-left pixel is the
  block's north-west corner. `coordinates.md` §9 gives pixel ↔ world formulas from the
  `UI_Map_*` values.
- `UI_Map_ContentLayer` selects a variant tile when a content layer (phasing state) is active
  (value 1 in 1,873 elements, otherwise 0x100… content-layer IDs; ED §4.5.1). The base tile
  applies when no listed layer is active.

<a id="s6-4"></a>
### 6.4 Light maps

The surfaces sampled are DXT1, 128 × 128 (1.25 units per texel): baked landscape lighting
and static-object shadows (ER §3 notes that object shadows are baked into the landscape).
The ceiling variant lights the ceiling surface.

> **OPEN:** Light-map texel orientation and how it combines with the layer textures.

---

<a id="s7"></a>
## 7. Kind 5 `0x805RXXYY` (Confirmed framing; meaning OPEN)

- Exists only where X and Y are multiples of 3 (33,159 records), and only with stamps 38 or
  39. One record per 3 × 3-landblock group; ER §3 reports 3 × 3 "source groups" for grass
  (frills), and a cache type named for navigation landblocks; either may be this record.
- Layout: own DID; u32 *n* = 9,801 (= 99 × 99) in every record; *n* × 5 bytes; then a
  trailing section that starts with u32 0 in every record and whose layout is not decoded.
- In each 5-byte sample, bytes 0–1 read as a u16 on the terrain height scale and bytes 2–4
  as three unsigned bytes near (127, 255, 127) (an up-pointing vector in a biased encoding
  is the guess). The u16 values do **not** equal the terrain heights at the obvious
  corresponding points (median difference 15 units).

> **OPEN:** meaning, sample spacing and the trailing section. Not required by M3.

---

<a id="s8"></a>
## 8. Loading a landblock bundle (algorithm in words)

For landblock (R, X, Y):

1. Data: read `0x800RXXYY` from `client_cell_R.dat` (always present for 0 ≤ X, Y ≤ 254).
   Decode §3 with the record's directory stamp. Keep heights, flags (bit 7), ceiling,
   surface chunks; keep the other parts raw.
2. Info: if the header's info DID is non-zero, read it (same archive) and decode §4 with the
   record's stamp.
3. Block map: read `0x801RXXYY` from `client_map_R.dat` and resolve it (§5.4) with the
   region's descriptor (cached per region).
4. Properties: read `0x804RXXYY` if present (§6).
5. For seamless normals, also request the heights of the east, north and north-east
   neighbours (their edge samples equal this block's; §3.3).

<a id="s8-1"></a>
### 8.1 Implementation notes (`tessera-world`, 2026-10-04)

These record decisions of the clean-room implementation, not facts about the data.

- **One job per landblock.** The bundle loader reads the data record, the info record
  (only when the data header names it, LB-E3), the block map and the properties record
  with their directory stamps in one asset job, then decodes them on the same worker.
  Only the data record is required; a missing, unreadable (I/O or archive error) or
  undecodable block map, info or properties record is reported as a bundle problem (with
  its §9 category when it did not decode) and the bundle is still delivered, so terrain
  shows. Only a missing or unreadable data record fails the job.
- **Links and weenies** (§4.6, §4.8) are dropped from the resident bundle unless asked
  for; an offline walker ignores them.
- **Neighbour heights** (step 5) are not copied into a bundle. Consumers read the
  neighbours' resident bundles (`WorldStreamer::seam_neighbours`). Residency admission
  provides them: under the budget a near- or mid-ring landblock is admitted only together
  with its *seam closure* — its desired east, north and north-east neighbours, and theirs
  in turn while they are near or mid — so once streaming has settled every resident
  near- or mid-ring landblock admitted under the budget has those neighbours resident.
  The always-admitted near ring (default) brings the direct east, north and north-east
  neighbours of each of its landblocks beyond the budget; a mid-ring landblock resident
  only as such a neighbour gets its own neighbours when the budget allows, so with a
  binding budget it can lack them. Neighbours that are not desired (past the grid edge,
  or beyond the far radius when mid = far) are absent, and while streaming a neighbour
  may arrive a frame or more after the block, so consumers still handle a missing
  neighbour (one-sided edge normals, remesh on arrival).
- **Residency admission.** Desired landblocks are admitted nearest first while the
  cumulative charge fits the residency budget; a resident bundle is charged its real
  size, an evicted one the size it had, one never loaded the running mean. The first
  closure that does not fit ends admission, so eviction is strictly farthest first, and
  re-admitting an evicted bundle needs 1/64 of the budget free, so a bundle on the
  boundary does not flip at a fixed focus.
- **Residency charge.** A bundle is charged its decoded grids and surface lists plus four
  times the stored bytes of its other records (an estimate). On the user's install the
  process RSS grew by about the charged amount plus the asset cache (see §8.2).

> **OPEN:** Ring radii (near 480, mid 960, far 1,600 units, hysteresis 160) and the
> 512 MiB residency budget are Tessera defaults. The original's draw distances per detail
> setting have not been observed; they should replace these defaults once recorded.

<a id="s8-2"></a>
### 8.2 Streaming measurement (`tessera-world`, 2026-10-04)

`cargo test --release -p tessera-world --test real_data -- --nocapture` on the user's
install (24 hardware threads, 22 asset workers): a focus moving in a straight line across
the whole region (G_x 80 → 40,720) along the row with the most landblock-info records,
paced at 60 frames per second, default rings and budget. "Update" is the main-thread
time of one streamer update (ring diff, admission, integration).

| Region | Row Y | Speed (units/s) | Bundles loaded | Failed | Mean job | Longest job | Update p99 | Longest update | Peak residency | Near-ring holes |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 120 | 1,000 | 5,355 | 0 | 0.27 ms | 11.3 ms | 0.25 ms | 0.50 ms | 87.5 MiB (386 blocks) | 0 frames |
| 1 | 120 | 4,000, budget 24 MiB | 4,751 (2,197 evicted) | 0 | 0.29 ms | 11.7 ms | 0.26 ms | 0.51 ms | 27.1 MiB | 0 frames |
| 2 | 252 | 3,000 | 3,315 | 0 | 0.25 ms | 10.9 ms | 0.12 ms | 0.60 ms | 99.7 MiB | 0 frames |
| 3 | 236 | 3,000 | 5,355 | 0 | 0.20 ms | 11.2 ms | 0.16 ms | 0.57 ms | 61.0 MiB | 0 frames |
| 4 | 49 | 3,000 | 5,355 | 0 | 0.21 ms | 15.7 ms | 0.15 ms | 1.60 ms | 48.2 MiB | 0 frames |
| 5 | 126 | 3,000 | 5,355 | 0 | 0.22 ms | 11.4 ms | 0.14 ms | 0.46 ms | 67.3 MiB | 0 frames |
| 14 | 250 | 3,000 | 3,825 | 0 | 0.16 ms | 11.6 ms | 0.11 ms | 0.36 ms | 13.7 MiB | 0 frames |

Every bundle became resident one frame after it was requested (request-to-resident
mean 16.6 ms at 60 frames per second). The region 1 rows were re-measured after the
admission rework of §8.1 (seam closures, real sizes of evicted bundles); the other rows
predate it. With the 24 MiB budget the residency stayed
within the budget plus the always-admitted near ring. The longest main-thread update
(1.6 ms) is far below the 50 ms of ROADMAP M3 exit criterion 2; terrain meshing and GPU
uploads are not part of this measurement.

<a id="s9"></a>
## 9. Failure categories (exit criterion 1)

No record of any kind failed to decode on this install. Implementations still report
failures under these categories, so a later install can be compared:

| Category | Meaning |
|---|---|
| `Truncated` | Payload ended inside a field |
| `TrailingBytes` | Bytes left after the layout |
| `BadCount` | Height, ceiling or flag count ≠ 1,089; coarse count ≠ 289; quad count ∉ {0, 1,024} |
| `BadFlag` | A `has_*`, `whole` or `cross` byte not 0/1 |
| `IndexOutOfRange` | A layer window outside the index list, or a vertex ≥ 1,089 |
| `DidMismatch` | Own DID or header info DID inconsistent |
| `UnknownStampRule` | Stamp 30 (info) or 34–37 (tree `build_byte`) where the rules have no observation; use the fallbacks |
| `PropertySet` | An embedded property set failed (`property-system.md` §5.3) |

Tessera's decoders (`crates/formats::landblock`, `FailureCategory`) add three categories
for conditions this section did not list: `UnsupportedFlags` (a landblock-info flag bit
other than 0–2, a cell flag bit outside 0x3C, a static-entity flag bit ≥ 0x800),
`Limit` (static-entity count above 65,536, the decoder's memory budget or nesting depth)
and `Malformed` (non-finite, out-of-bound or zero-length static-entity transform).
`UnknownStampRule` is not a failure in Tessera: the decoder falls back and reports the
record as unusual.

Classification is structural, so the same fault always lands in the same category: any
failure inside an embedded property set (SED, cell, link, landblock, weenie and kind-4
sets, and block-map descriptor palette values) is `PropertySet`, whether the set was
truncated, malformed or named an unknown PID; a resource limit is `Limit` wherever it is
hit, inside a set or not; everything else is classified by its innermost error.

<a id="s9-1"></a>
### 9.1 Decode rates of the Tessera implementation (2026-10-04)

`cargo test --release -p tessera-formats --test landblock_real_data` with this install,
every record, each decoded with its own directory stamp:

| Record | Records | Decoded | Failed | Unusual | Cross-record checks |
|---|---|---|---|---|---|
| Landblock data (K 0) | 390,150 | 390,150 (100 %) | 0 | 0 | Re-encodes byte for byte; stampless fallback (§3.9) agrees on all; LB-E1 dense; LB-E3 holds; HM7 0 of 359,949 neighbour pairs differ |
| Block map (K 1) | 390,150 | 390,150 (100 %) | 0 | 0 | Re-encodes byte for byte; BM2 one key set per region (28, 29, 29, 30, 30, 29 keys); L9 0 disagreements with `has_ceiling`; out-of-range indices 759, 1, 6, 6, 6, 0 (regions 1, 2, 3, 4, 5, 14) |
| Landblock info (K 2) | 49,636 | 49,636 (100 %) | 0 | 0 | LI2–LI4, ED1, ED3–ED6; stampless trailer fallback agrees on all; 104,271 cells, 3,253,862 static entities, 1,324,469 links, 3,768,840 weenies; 1,033 cell geometry DIDs, all exist |
| Landblock properties (K 4) | 60,450 | 60,450 (100 %) | 0 | 0 | 59,831 minimap tiles (4,618 variants), all with block offset = own X, Y, pixel offset (0, 200), scale 1.25; 5,441 light maps |
| Kind 5 | 33,159 | 33,159 framed | 0 | 0 | 9,801 samples, tail starts with u32 0, X and Y multiples of 3 |
| Block-map descriptors (0x18) | 5 | 5 | 0 | 0 | BM3 |
| Region descriptors (0x0F) | 7 | 7 | 0 | 0 | Block-map descriptor in slot 7 in all |

No failure category occurred, so none needs a new OPEN item. The census also confirms
the §3.5 ceiling finding above and, on a 1-in-7 sample, that 38.1 % of outdoor Physics
entities in landblocks without cells lie within 25 cm of the §3.11 surface.

> **OPEN (decoder policy):** Tessera rejects landblock-info flag bits other than 0–2 and
> cell flag bits outside 0x3C as `UnsupportedFlags`, because their payloads (if any) are
> unknown and the rest of the record would be misframed. None is set in this install; if a
> later install sets one, its layout belongs in §4.1 / §4.4.

> **OPEN (decoder policy):** A null element (PID 0) in a 0x18 descriptor palette (the
> Reported null-entry rule of `property-system.md` §5) is assumed to keep its slot, so the
> indices of later elements do not shift. Never observed.

> **OPEN (decoder policy):** Region descriptors (prefix 0x0F) are accepted only at exactly
> 114 bytes (`environment.md` §3.1, every observed record). The block-map descriptor is
> looked up as the first table DID with prefix 0x18 rather than by slot number.

<a id="s10"></a>
## 10. Hostile-input notes

- Bound every count by the bytes remaining: a layer is 17 bytes, an index 2, a quad entry
  2, a box 24, a cross link 24, a neighbour 15, a block-map entry 6.
- `code_length` of a tree is bounded by the remaining bytes (`mesh.md` §10.3).
- Validate layer windows before slicing the index list.
- Cap the cell, entity, link and weenie counts by remaining bytes as well.
- Keep checks and lookups linear in the record size: membership tests over cell indices
  (LI3) and block-map key lookups (§5.4) need an index (cell indices are u16, so a
  65,536-bit set suffices), or a hostile record makes them quadratic.

<a id="s11"></a>
## 11. Open questions (summary)

1. Bits 0–6 of the vertex-flag grid (§3.6).
2. The 17 × 17 coarse grid (§3.7).
3. Surface-layer `material` table, blend order, mask orientation, `whole` (§3.8.2); quad-grid
   meaning and order (§3.8.4).
4. Ceiling diagonal rule (§3.5; facing is now known: down); box list (§3.10).
5. Landblock-info `size_hint`, trailer, cross-link `mask`, header indices, cell flags and
   lists, 0x01 cell geometry, placement of the 2 cells without an offset (§4).
6. Block-map keys without descriptor entries (§5.6); descriptor `flag` byte (§5.2).
7. Light-map orientation (§6.4).
8. Kind 5 (§7).
9. In-game height test points (§3.4).
