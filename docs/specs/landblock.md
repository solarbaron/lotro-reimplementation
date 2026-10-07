<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Landblock records (prefix 0x80): heightmap data, landblock info, block maps, landblock properties

| | |
|---|---|
| **Status** | Accepted (2026-10-07). Revisions of 2026-10-07: interiors, visibility and ceiling normals (§3.14, §4.3, §4.4, §4.11–§4.14); then the review corrections and the facts the released walker code relies on (§3.14, §3.15, §4.2, §4.4, §4.10–§4.15, §8.1). |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review sessions (spec-writer agent), each a different session from the writer |
| **Review record** | 2026-10-05 review: changes requested (addressed). 2026-10-05 re-review: changes requested (addressed). 2026-10-06 third review: changes requested (addressed). 2026-10-06 re-review: passed, Accepted. 2026-10-07 re-review of the interiors revision: changes requested. 2026-10-07 second re-review: changes requested (same corrections; addressed in the second 2026-10-07 revision, change log). 2026-10-07 third re-review (Claude review session, spec-writer agent; a different session from the writer): passed, Accepted. Detailed findings are kept in the dirty-room review log. |
| **Implements into** | `crates/formats` (landblock data, landblock info, block-map and descriptor decoders), `crates/world` (landblock IDs, bundles, block-map resolution, minimap tiles), `crates/terrain` (height grid, triangulation, surface layers), `apps/explorer` (landblock view), `apps/client` (walker interiors and landscape drawing, §3.15, §4.11–§4.15), `fuzz/` (one target per record kind) |
| **Companion specs** | `coordinates.md` (frames, units, map coordinates, rotations). `entity-descriptor.md` (the static entity descriptors inside landblock info; normative for their layout). `environment.md` §3 (region descriptor 0x0F, which names the block-map descriptor and the per-region tables). `records.md` §1 (VLE, TSize, Pascal string), §2.1–§2.2 (prefix and landblock DID shape, DB types), §5 (render surfaces). `property-system.md` §4–§5 (values and embedded property sets), §4.6 (Position). `mesh.md` §10 (collision tree block and its `build_byte` rule). `dat-container.md` §5.2 (directory `stamp`), routing. `wstate.md` (0x70 objects named by `Area_DID` and weenies). [`environment.md`](environment.md) §7, §11 and [`lighting.md`](lighting.md) §8.1, §10 (the indoor environment record of §4.13 feeds them). [`appearance.md`](appearance.md) §3.2 (appearance lists, §4.14). **Meaning of the parts decoded here:** [`terrain-texturing.md`](terrain-texturing.md) (surface layers, masks, quad grid, light maps), [`collision.md`](collision.md) (tree code, terrain and ceiling collision, 0x01 cell geometry, portals), [`scenery.md`](scenery.md) (coarse grid), [`water.md`](water.md) (water and fog-zone keys of the block map, water per landblock, kind 5 water). |

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
- how the client draws **interior cells**: visibility cells and portals, when cells are drawn
  from inside and from outside, the indoor environment (ambient, fog, no sun) and what mesh a
  cell draws (§4.11–§4.14); how terrain and **ceiling** vertex normals are computed and what
  that means for cave-ceiling lighting (§3.14); which chunks without layers are drawn
  (§3.15); what the released walker does where these sections are silent (§4.15);
- **block maps** `0x801RXXYY` and their per-region descriptors (prefix 0x18): area, music,
  ambient sound, sky and other per-landblock keys (§5);
- **landblock properties** `0x804RXXYY`: minimap tiles and baked light maps (§6);
- the framing of kind 5 `0x805RXXYY`, the distant-landscape sector record (§7);
- the bundle-loading algorithm, invariants, statistics, failure categories and what remains
  unknown (§8–§12).

All counts are observations of one install on one date and change with patches. Tests
assert the invariants, not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install (2026-10-04, or 2026-10-05, 2026-10-06 or 2026-10-07 where
  the row says so) by throwaway probes in the dirty-room scratchpad, over **all** records of the kind
  unless a sample is stated. For a fact with a BIN source: the data agrees on every record.
- **BIN**: rests on the disassembly of the client (tags BIN and BIN2 below, or the BIN tags of
  the companion specs named in the row) and has not been checked against every record.
- **Inferred**: follows from BIN facts by reasoning, not traced directly.
- **Probable**: supported by a statistical test on the data, but not by an exact rule.
- **Reported**: comes only from third-party material and was not, or could not be, checked.
- **Tessera Decision**: Tessera's own rule where the original's is not known; never a
  statement about the original.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam). Every record of `client_cell_{1,2,3,4,5,14,15}.dat` and `client_map_{1,2,3,4,5,14,15}.dat` was read: 390,150 landblock-data, 390,150 block-map, 49,636 landblock-info, 60,450 landblock-properties and 33,159 kind-5 records. Every landblock-data, landblock-info, block-map and landblock-properties record decoded **to the last byte** with the layouts below. Directory `stamp` values were read for all of them. All five 0x18 descriptors and all seven 0x0F region descriptors were decoded. 120 minimap tiles and 43 layer masks were opened to check formats and orientation. |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution; decompiled classes `HeightMapDataLoader`, `HeightMap`, `MinimapLoader`, `GeoLoader`, `PositionDecoder`) and `lotro-tools` (readable source: `LandblockInfoLoader`, `BlockMapLoader`, `PropertyDescriptorsManager`, `LandblockLoader`). Facts only; no code taken. |
| JT | jtauber/lotro `cell.py` (no licence; facts only): header shape of landblock data. |
| DS | `research/notes/data-semantics.md` §6.2–§6.6 (summary of LC and JT). |
| ER | `research/notes/engine-client-re.md` §3 (cache type names, 3 × 3 frill source groups, inverted terrain ceiling for caves). |
| ENV | `environment.md` §3 (region descriptor layout; written in parallel from the same install). |
| ED | `entity-descriptor.md` (static entity descriptors; written in parallel from the same install). |
| TT, COL, SCN, WAT | `terrain-texturing.md`, `collision.md`, `scenery.md`, `water.md` (same install, 2026-10-05): each decodes the meaning of parts whose layout is given here; their correction tables (`terrain-texturing.md` §15, `collision.md` §15, `water.md` §13) are adopted in this revision. |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5), read in the dirty room on 2026-10-05 (objdump, capstone). **Questions:** (a) how the client converts a stored height sample to world units (every converter found: terrain vertex builder, height queries, physics, the landblock minimum and maximum, `terrain-texturing.md` §13 and `water.md` BIN (c)); (b) which DB object type the client registers for each landblock kind; (c) which records read block-map keys 3, 4, 6 and 7. **Why M1–M3 were not enough:** the data fits stored / 32.768 and stored × 2,000 / 65,535 equally well (they differ by at most 0.031 units); the record kinds and the keys without a 0x18 descriptor entry are named nowhere in the data or in published sources; no running client is available for M3. |
| BIN2 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md)**, second use. The same client build as BIN (`x64/lotroclient64.exe` 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), read in the dirty room on 2026-10-07 with a headless decompiler session. Nothing outside rendering, cell loading and the environment was examined (no network, login or anti-tamper code). **Questions:** (d) which interior cells, portals and meshes the client draws, and when cells are drawn with the camera inside or outside them (row Y5 of `docs/checklists/m4-5-visual.md`: interior cell meshes not drawn); (e) how the client reads the cell flags, lists A and B, `extra`, the cell groups and the neighbour entries of §4.3–§4.4; (f) which environment values (ambient, fog, sun, sky key) apply indoors and where the client reads them; (g) how the vertex normals of terrain and ceiling chunks are computed (dark cave ceilings, rows T5, T15 and T16 of the same checklist). **Why M1–M3 were not enough:** the data stores the lists, flags and properties but not how they are used or which source wins; normals are not stored at all; no running client is available for M3 frame captures in this session. Output is prose facts only. |
| OBS2 | Observed in user install 2026-10-07 (Steam) by a throwaway read-only probe: all 49,636 landblock-info records of regions 1–5 and 14 decoded again; all 104,271 cells, their neighbour entries, lists A and B, `extra`, flags and property sets, and all 1,288 cell groups (§4.3) dumped and cross-checked; the portal joins of the landblocks with scaled cells recomputed from the 0x01 geometry; the landblock-data and landblock-properties records of the blocks of checklist rows T5, T15 and T16 read. |
| BIN3 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md)**, third use. The same client build (`x64/lotroclient64.exe` 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5, re-checked on 2026-10-07), read in the dirty room on 2026-10-07 with a headless decompiler session on an own copy of the shared project (deleted afterwards). Nothing outside cell loading, the visibility set-up and the scene renderer's landscape switches was examined. **Questions:** (h) which portal polygon of the 0x01 geometry the client takes for a neighbour entry: the one named by the entry's `portal` field or the one at the entry's position; (i) whether a neighbour, a group member or a cell of a PVS can belong to another landblock; (j) what the outdoor-visible flag switches besides terrain, ceilings and landscape water (for example frills); (k) re-check of the review findings on the scene-flag pass (§4.12) and the PVS walk. **Why M1–M3 were not enough:** `portal` equals the entry position in every entry, so the data cannot tell the two readings apart; which objects a flag switches is not stored anywhere; no running client is available for M3. Output is prose facts only. |
| OBS3 | Observed in user install 2026-10-07 (Steam) by a throwaway read-only probe: all 390,150 landblock-data records (open flag of §3.15, chunks without layers, triangles per interior grid sample) and all 49,636 landblock-info records (cell numbers, positions without offset or orientation, neighbour `portal` against the entry position, every neighbour, list-A and list-B value against the record's cells); the portal joins of the two cells without an offset recomputed from their 0x01 geometry. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| L1 | Landblock-family DIDs and archive routing (§2) | DS, LC; OBS | Confirmed |
| L2 | Landblock data header: DID, landblock-info DID or 0, u32 0, u32 1089, then 33 × 33 u16 heights (§3.2) | LC, JT; OBS (390,150) | Confirmed |
| L3 | Heights are stored **X-major**: sample (i, j) at index 33·i + j, i along X (§3.3) | LC (index formula); OBS (seam test, entity contact test) | Confirmed |
| L4 | Height in world units = stored value × 2,000 / 65,535, computed in 32-bit floats (65,535 = 2,000 units) (§3.4) | BIN (a) (the rule); OBS (3.25 M placed entities fix the scale to within 0.031 units; the water heights of `water.md` §6.5); LC (`value / 32768`, a factor 1,000 off and 1.0000153 off) | Rule BIN; scale Confirmed. Revised 2026-10-05 (was stored / 32.768). |
| L5 | The bytes after the height grid: optional ceiling grid and its collision tree, vertex-flag grid, 17 × 17 grid, surface-layer chunks, terrain collision tree, box list (§3.1) | OBS (all 390,150 records, zero leftover bytes) | Confirmed (layout). No published source describes them. |
| L6 | Bit 7 of the vertex-flag grid selects the split diagonal of each **terrain** grid quad (§3.6) | OBS (65.5 M sampled terrain-layer triangles, 100 %) | Confirmed |
| L6a | Bit 0 of the vertex-flag grid selects the split diagonal of each **ceiling** grid quad, with or without stored triangles; ceiling triangles are clockwise from above (§3.5, §3.6) | COL §6.4 (BIN: the client's mesh builder tests bit 0 for the ceiling, bit 7 for the terrain; OBS: all 5,310,848 stored ceiling triangles follow bit 0, bit 7 only 94.1 %); review probe 2026-10-05 (all 486,882 stored ceiling triangles of region 1: clockwise and bit 0 without exception, bit 7 95.8 %) | Confirmed |
| L6b | Terrain grid triangles that no layer covers are kept by the client's terrain collision; they lie where the terrain is buried under the ceiling surface or where there is no ceiling (§3.5) | COL §6.5 (OBS census of all records; BIN for the collision mesh) | Confirmed (census); collision rule BIN |
| L6c | Vertex-flag bytes: the low seven bits take only 0, 1, 126 and 127; whole bytes only 0, 1, 126, 127, 128, 129, 254, 255; low value 126 occurs in 365 records (region 1: 21, region 2: 271, region 14: 73), 357 of them with a ceiling, 8 without (0x80020A90, 0x800220F6, 0x80027590, 0x800E06C7, 0x800E07CD, 0x800E08A6, 0x800E09A5, 0x800E09C8) (§3.6) | OBS (probe 2026-10-06 over all 390,150 landblock-data records of regions 1–5 and 14; 6,867 with a ceiling) | Confirmed (2026-10-06) |
| L7 | Surface layers: terrain-type key (`material`), 0x41 alpha-mask DID, `whole` flag, triangle range into a shared index list over grid vertices (§3.8) | OBS; TT §4 (meaning: key into the region's 0x19 terrain type table, draw order, masks, `whole`) | Confirmed (layout and meaning) |
| L8 | Index-list vertex numbers are **Y-major** (33·y + x), the transpose of the height array (§3.8.3) | OBS (two independent tests) | Confirmed |
| L9 | The ceiling grid is present exactly when the block map says `Landblock_UseCeiling` = 1 (§3.5) | ER (caves use an inverted terrain ceiling); OBS (6,867 of 6,867) | Confirmed |
| L10 | Landblock info layout (§4); field and part names are Tessera's own, chosen for this spec (§4.1–§4.8) | LC (field order); OBS (49,636 records, zero leftover bytes) | Confirmed |
| L11 | Landblock info ends with a u32 only when the record's `stamp` ≥ 31 (§4.9) | OBS (perfect split) | Confirmed |
| L12 | Cell geometry DIDs are 0x01 records (§4.4) | OBS (1,033 distinct, all resolve) | Confirmed. Answers `setup.md` §2.2 OPEN. |
| L13 | Block map = list of (key, u16 index) into the region's 0x18 descriptor (§5) | LC; OBS (390,150 records) | Confirmed |
| L14 | The region's 0x18 descriptor is named by its 0x0F region descriptor (§5.1) | ENV; LC (hard-coded table that agrees); OBS | Confirmed |
| L15 | Landblock properties = DID + property set; minimap tile is a 200 × 200 JPEG at 1.25 px per unit, north up (§6) | LC (property names, tiling); OBS (60,450 records; 120 tiles) | Confirmed |
| L16 | Kind 5 exists only at XX, YY divisible by 3 and holds 99 × 99 five-byte samples; it is the distant-landscape sector record (DB type 0x41) and carries the sector's water pairs (§7) | ER (3 × 3 groups); OBS (33,159); BIN (b); WAT §9 | Confirmed (framing, water pairs); rest of the layout OPEN |
| L17 | Coarse grid = per-vertex scenery word: bits 0–7 terrain-type id, 8–14 scene slot, 15–19 encounter slot; X-major order with shared edges (§3.7) | OBS (all 390,150 records; seam test 2026-10-05); BIN (client `x64/lotroclient64.exe` 4906.0071.1794.4035: how the client reads the word; method M4 under decision 0011, because no data or published source says which table the bytes index); `scenery.md` | Confirmed |
| L18 | DB object types of the landblock kinds: K 0 = 0x11, 1 = 0x2F, 2 = 0x2E, 4 = 0x29, 5 = 0x41 (§2.1) | BIN (b); labels from the DBOType enum mapper (OBS) | BIN |
| L19 | Block-map keys without a 0x18 descriptor entry are read by other region tables: key 3 by the fog-zone table, keys 4 and 7 by the water table, key 6 by the encounter table (§5.6) | BIN (c); OBS (the tables store the key numbers) | Confirmed (3, 4, 7); Probable (6) |
| L20 | The tree `code` of terrain and ceiling trees is the quantised bounding-volume program of `collision.md` §3; terrain keys number the grid triangles (§3.9) | COL §3, §6 | Confirmed |
| L21 | Quad grid: index 32 · qy + qx; the dominant visible terrain type of each quad (§3.8.4) | TT §4.6 | Confirmed (order); Probable (meaning) |
| L22 | Light maps: row 0 on the south edge, column 0 on the west edge; bound as material parameter 3019 and **added** to the lighting sum; content is baked light (§6.4) | TT §10, T22 (BIN there) | Confirmed (orientation); BIN (additive use); Probable (content) |
| L23 | A point enters a cell from the outdoor world by containment in the cell's phantom and cell volume, not by crossing a portal (§4.4) | COL §5.8.1 | BIN |
| L24 | `link_type` (§4.2, §4.6) takes codes of the install's `EntityLinkType` enumeration: the enum list 0x28000001 names it at key 0x0D and points to enum mapper 0x230000BE (codes 0x10000001 and up), whose base mapper 0x230001A9 holds codes 0x00–0x1B. Every code observed in §4.6 has a label there; the labels are quoted as interoperability names from the data | OBS (2026-10-06, both mappers read in full) | Confirmed |
| L25 | Cell groups (§4.3) are **visibility groups**: `lead` is a group key, not a cell; a cell's `extra` names its group; in cells with `cell_flags` bit 5, list B holds group keys (its own group always among them); in cells without bit 5, list B lists the cell's own group as cell numbers (§4.3, §4.4) | BIN2 (e); OBS2: 78,069 cells have an `extra` that is a group key of their record and are members of that group, 14,064 have 0xFFFF and are in no group, 12,138 have no `extra`; 129,684 of 129,684 list-B values of bit-5 cells are group keys, the own group is among them in 78,069 of 78,069; list B of the 9,288 non-empty bit-5-clear cells always contains the cell | Confirmed (2026-10-07) |
| L26 | Neighbour entry grammar as the client reads it: u32 portal number, u16 neighbour cell, i32 back-reference, u32-counted list of u16, u8 flag (§4.4); the former "u16 back-reference + 3 × u16 zero" reading is the same 15 bytes when the list is empty | BIN2 (e); OBS2 (list count 0 and back-reference high half 0 in all 393,842 entries) | Confirmed (2026-10-07) |
| L27 | `cell_flags`: bit 0 makes a cell an entry of the outdoor visibility walk (never set), bit 2 = property set present, bit 4 = the sky is visible from the cell, bit 5 = the tail format with `index_echo`, group keys in list B and `extra`; bit 3 is kept by the client but its use was not traced (§4.4) | BIN2 (e) (reader and writer of the flags); OBS2 (bit 0: 0 cells; bit 4: 9,493 cells in 348 records) | BIN (meaning); counts Confirmed |
| L28 | Visibility cells: one for the whole outdoor world, one per cell group, one per cell in no group; 104,271 cells form 18,390 visibility cells (§4.11) | BIN2 (d); OBS2 (counts) | BIN; counts Confirmed (2026-10-07) |
| L29 | Visibility portals: one per neighbour entry whose neighbour is outside the cell's own group, built from the geometry's portal polygon; 39,692 of 393,842 neighbour entries make a portal, 354,150 lie inside a group (§4.11) | BIN2 (d); OBS2 | BIN; counts Confirmed (2026-10-07) |
| L30 | Interior cells are never drawn while the camera is outdoors, and the landscape (terrain, ceilings, landscape water) is never drawn while the camera is in an interior cell: no portal leads between the outdoor visibility cell and a cell, and no cell has bit 0 (§4.12) | BIN2 (d); OBS2 (no neighbour entry names cell 0 or 0xFFFF; cell numbers run 2–1,569) | BIN; data Confirmed (2026-10-07) |
| L31 | Potentially visible cells of a cell: the cell, list A, and the members of every group named in list B (bit-5 cells); every neighbour of a cell is among them (§4.12) | BIN2 (d); OBS2 (neighbours inside the set in 104,271 of 104,271 cells) | BIN; invariant Confirmed (2026-10-07) |
| L32 | Environment values at a position are looked up in the cell's property set, then the landblock-info property set (§4.7), then the block map (§5.4); two environment records are kept, an outdoor one (sky key `Environment_LocalSky`, default 2) and an indoor one (sky key `Environment_IndoorAmbiance`, default 1), and the indoor one is used while the camera is in a cell (§4.13) | BIN2 (f) | BIN |
| L33 | Indoor record: the main directional light (sun or moon) is off; up- and down-facing ambient come from `InteriorUpperAmbientLight` / `InteriorLowerAmbientLight` plus `InteriorAmbientLight`, each channel limited to 1; `InteriorFogType` 1 turns fog on, any other stored value turns it off; `InteriorFogColor`, `InteriorFogNear`, `InteriorFogFar` replace the keyframe values; far ≥ 20, near ≤ far − 20 (§4.13) | BIN2 (f); OBS2 (property counts) | BIN |
| L34 | A cell draws the render mesh embedded in its 0x01 record, placed with the cell's frame; when the cell has appearance data the client draws a private copy with the appearance applied (§4.14) | BIN2 (d); COL §5.7 | BIN |
| L35 | The cell scale properties are not part of the placement: applying `Render_AdjustableScale` / `Physics_AdjustableScale` (51 cells not 1.0) breaks all 144 portal joins of the two landblocks that have scaled cells, which match exactly without it (§4.14) | OBS2 | Confirmed (placement, 2026-10-07); render use OPEN |
| L36 | Vertex normals of terrain and ceiling chunks: the mean of the unit normals of the grid triangles around the sample, from the neighbouring landblocks at the edges; ceiling normals face down (§3.14) | BIN2 (g) | BIN |
| L37 | Terrain and ceiling chunks are drawn only while the outdoor scene is visible (§4.12); a ceiling with down-facing normals receives the down-facing ambient, its light map and point lights, but no sun (§3.14) | BIN2 (d), (g); `terrain-texturing.md` §9.1 | BIN (draw rule); Inferred (lighting consequence) |
| L38 | Visibility never leaves a landblock: the client forms the key of every neighbour, group member and PVS cell from the region, landblock and instance of the cell that names it, so portals, shared visibility cells and the scene-flag walk stay inside one landblock-info record (§4.11, §4.12) | BIN3 (i); OBS3 (every neighbour, every list-A value and every list-B value of a cell without bit 5 is a cell of the same record, 0 exceptions) | BIN (rule); data Confirmed (2026-10-07) |
| L39 | A visibility portal's polygon is the 0x01 geometry's portal named by the entry's `portal` field; the client drops the portal when the neighbour has no visibility cell (§4.11) | BIN3 (h); OBS3 (`portal` = entry position in all 393,842 entries) | BIN; data Confirmed (2026-10-07) |
| L40 | Scene-flag pass in detail: the outdoor branch only for region ≠ 0, landblock X and Y ≠ 0xFF and cell 0; any other position is looked up as a cell and, when none is found, both flags stay clear; each visited cell first clears both flags when it has bit 0, then sets sky-visible when it has bit 4; the PVS walk skips `index_echo` and 0xFFFF and does not expand the PVS of a PVS cell (§4.12) | BIN2 (d), re-checked by both 2026-10-07 reviews; BIN3 (k) | BIN |
| L41 | Outdoor-visible clear (or a landscape position that is not valid) means the client sets up no landscape pass at all: no terrain, no ceiling, no landscape water, no frills, and a fourth landscape part whose meaning was not traced; the sky depends on sky-visible only (§4.12) | BIN3 (j) | BIN |
| L42 | The two cells whose `position` has no x, y, z (flag 0x4B) lie at the landblock origin with the identity orientation: an absent offset is (0, 0, 0) and an absent orientation the identity (`property-system.md` §4.6, BIN there), and placed so, all 5 of their portal joins with their neighbours match exactly; 11,099 cells have no orientation (§4.4) | OBS3; `property-system.md` §4.6 | Confirmed (2026-10-07) |
| L43 | Cell numbers are unique within a landblock-info record (104,271 cells, no repeat) (§4.4, LI9) | OBS3 | Confirmed (2026-10-07) |
| L44 | The open flag of a block (§3.15) and the chunks without layers: 6,867 blocks with a ceiling, 3,075 open and 3,792 not; terrain chunks without layers 299,369 (no ceiling) + 818 (open) + 3,237 (not open); ceiling chunks without layers 944 (open) + 3,568 (not open); the raw u16 test and the converted-height test agree in every block; including row and column 32 in the test would change it in 140 blocks | OBS3; `terrain-texturing.md` §4.9, T40 (BIN (r) there, re-checked by both of its 2026-10-07 reviews), T41 | Confirmed (2026-10-07; rule BIN) |
| L45 | A grid sample's vertex normal averages 4 to 8 triangles (one per neighbouring quad, two when the quad's diagonal runs through the sample); interior terrain samples of all 390,150 records: 4 triangles at 9,201, 5 at 353,168, 6 at 374,206,540, 7 at 358,859, 8 at 6,382 (§3.14) | BIN2 (g); OBS3 | BIN (rule); census Confirmed (2026-10-07). Corrects "up to six" of the first 2026-10-07 revision. |

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

| `K` | Record | DB type (BIN, L18; label from the DBOType mapper) | Archive | §  |
|---|---|---|---|---|
| 0 | Landblock data (heights, terrain surface) | 0x11 LANDBLOCKDATA | `client_cell_R.dat` | §3 |
| 1 | Block map (per-landblock keys) | 0x2F LANDBLOCKBLOCKMAP | `client_map_R.dat` | §5 |
| 2 | Landblock info (cells, static entities, links, weenies) | 0x2E LANDBLOCKINFO | `client_cell_R.dat` | §4 |
| 4 | Landblock properties (minimap, light maps) | 0x29 LANDBLOCKPROPERTIES | `client_cell_R.dat` | §6 |
| 5 | Distant-landscape sector, one per 3 × 3 group | 0x41 DISTANT_LANDSCAPE_SECTOR | `client_cell_R.dat` | §7 |

No other `K` occurs. (The client also registers `K` = 6 for DB type 0x63, labelled
KYNLANDBLOCK, a navigation record; no such record exists in this install.) `R` selects the
archive pair: region *R* lives in `client_cell_R.dat` and `client_map_R.dat` with *R* written
in decimal (region 14 → `client_cell_14.dat`).

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

The directory entry `stamp` (`dat-container.md` §5.2) changes two layouts (§3.9, §4.9).

| Kind | Stamps observed (record count) |
|---|---|
| 0 | 20 (43,962), 21 (104), 22 (74,070), 27 (11,914), 38 (130,050), 39 (130,050) |
| 1 | 20 (122,297), 21 (133), 22 (6,284), 27 (1,336), 38 (130,050), 39 (130,050) |
| 2 | 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29 (7,106 together); 31, 32, 33, 38, 39 (42,530 together) |
| 4 | 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29, 31, 32, 33, 38, 39 |
| 5 | 38 (24,434), 39 (8,725) |

A decoder therefore takes the record's `stamp` as an input next to the payload, as for
meshes. §3.9 and §4.9 give fallbacks for payloads without a stamp.

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
### 3.4 Height scale (rule BIN; scale Confirmed; revised 2026-10-05)

**Height in world units = stored value × 2,000 / 65,535.** The original client computes it
in 32-bit floats as: convert the stored u16 to a float, multiply by 2,000.0, multiply by the
32-bit float nearest 1 / 65,535 (1.5259022 × 10⁻⁵), rounding each product to 32 bits (BIN (a):
every converter in the client, `terrain-texturing.md` §13). Stored 0 is height 0; 65,535 is
exactly 2,000 units; 32,768 is 1,000.0153 units. The 32-bit result differs from the exact
quotient by at most 6.2 × 10⁻⁵ units.

The earlier rule of this spec, stored / 32.768 (32,768 = 1,000 units), is **superseded**. The
two differ by a factor 1.0000153: the largest difference over all stored values is
**0.0305 units** (3 cm, near the top of the range), and less than 0.016 units below stored
32,768. No data test separates them; the client's rule is adopted so that heights, water
levels (`water.md` §6.2) and scenery placement (`scenery.md` §7) match the original bit for
bit. Specs that cite this section (`scenery.md` §7.3, `collision.md` §6.1, `terrain.md` §1,
`water.md` §6.2) use this rule.

Evidence for the scale (Confirmed):

- The median ratio of placed-entity *z* (absolute height, ED §4.4) to the bilinear terrain
  sample under the entity is 0.030518 in every region (2,000 / 65,535 = 0.0305180;
  1 / 32.768 = 0.0305176), over 2,479,912 entities.
- With the scale and the triangulation of §3.11, 491,515 entities (19.8 %) sit within 1 cm
  and 1,221,684 (49.3 %; 42.5 % in region 4 to 60.5 % in region 1) within 25 cm of the
  terrain; the rest are on buildings, bridges, in water or deliberately sunk.
- Constant-height blocks use round heights: stored 4,915 → 149.996, 10,485 → 319.982,
  32,765 → 999.924.
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

**Ceiling triangulation (Confirmed, L6a).** The diagonal of every ceiling quad (i, j) is
given by **bit 0** of the vertex-flag byte of sample (i, j) (§3.6), whether or not the quad
has stored ceiling-chunk triangles: set → south-west/north-east, clear →
south-east/north-west. Ceiling triangles are **clockwise seen from above** (they face down):
the same two triangles per quad as the terrain split for that diagonal, with the corners of
each triangle in reverse order. Every stored ceiling-chunk triangle (5,310,848, all records)
follows this rule; bit 7 agrees for only 4,999,408 of them (94.1 %), so bit 7 must not be
used for ceilings (`collision.md` §6.4; the 2026-10-04 implementation census that raised the
question is answered by it). A renderer draws the ceiling's layers from their stored
triangles (§3.8); any ceiling mesh built from the grid (a `whole` ceiling layer, the
collision mesh) uses the bit-0 diagonal. The client's ceiling collision mesh holds all
2,048 grid triangles, including those over zero samples (at height 0, normally far below the
terrain); its collision tree uses the terrain numbering (`collision.md` §6.2, §6.4).

**Terrain holes (Confirmed census, client rule BIN; L6b).** Terrain grid triangles that no
terrain layer covers are not drawn (`terrain-texturing.md` §4.5). They occur only in
landblocks with a ceiling, and they are **not** cave mouths: no such triangle lies under an
open cave; they lie where the terrain is buried under the ceiling surface (ceiling at or
below the terrain) or where the ceiling samples are all 0. The client's terrain collision
keeps every one of them (all 2,048 grid triangles, `collision.md` §6.5), so a walker
collides with the full grid.

> **OPEN:** How the original renders ceiling vertices that lie at or below the floor (61 %
> of non-zero ceiling samples): culled, clamped or simply drawn and hidden by the depth test.
> Not needed for an outdoor walker. **Tessera Decision** until then: draw the stored ceiling
> triangles unchanged and let the depth test hide what lies inside the ground.

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

Triangles are listed counter-clockwise seen from above (+Z), matching the stored terrain
layer triangles (§3.8.3). Evidence: every triangle of every terrain surface layer
(65,471,762 in a 1-in-7 sample) uses exactly the diagonal this bit predicts. Bytes in row i = 32 or column j = 32
carry no quad.

**Bit 0 (0x01): split diagonal of the ceiling quad whose south-west corner is this sample**
(§3.5, L6a). Same table as above with bit 0 in place of bit 7, and with each triangle's
corners in reverse order (clockwise from above). A renderer or collision builder that
meshes a ceiling must read bit 0; for terrain it reads bit 7.

**Bits 1–6:** the seven low bits take only the values 0, 1, 126 and 127, so bits 1–6 are
either all set (0x7E) or all clear in every byte. Per record, the set of low values is
{127} in most records with stamp 20–27, {0} in most with stamp 38–39, and mixed in a few
hundred. The low value 126 occurs in 365 records, 357 of them with a ceiling and 8 without
(3 in region 2, 5 in region 14; L6c), so it does not imply a ceiling. Whole bytes
observed: 0, 1, 126, 127, 128, 129, 254, 255.

> **OPEN:** Meaning of bits 1–6 (`collision.md` §15). The client's terrain and ceiling mesh
> builders test only bits 7 and 0 (`collision.md` §6.4). Tessera ignores bits 1–6.

<a id="s3-7"></a>
### 3.7 Coarse grid: per-vertex scenery word (Confirmed layout and order; meaning in `scenery.md`)

u32 count (always 289 = 17 × 17), then 289 × u32. The grid has one word per 10 units
(every second height sample). Word (i, j), 0 ≤ i, j ≤ 16, is at index **17 · i + j** with
**i along X**, the height grid's order: word (i, j) belongs to height sample (2i, 2j), and
word (16, j) of block (X, Y) equals word (0, j) of block (X + 1, Y) (31,500 of 31,500
adjacent pairs in regions 1 and 2; the transposed reading matches 16 %).

| Bits | Field | Observation (all 390,150 records) |
|---|---|---|
| 0–7 | Terrain-type id for scenery (`scenery.md` §3) | 0–94; distinct sets per region |
| 8–14 | Scene slot (`scenery.md` §3) | 0–72 |
| 15–19 | Encounter slot (`scenery.md` §7.7) | always 0 |
| 20–31 | — | always 0 |

| Observation (all 390,150 records) | Value |
|---|---|
| Records whose 289 words are all 0 | region 1: 43,976; 2: 52,988; 3: 53,071; 4: 47,402; 5: 50,675; 14: 63,924 |
| Distinct values per record | 1 in most; up to 16 |

The grid selects **procedural scenery and frills**, not terrain textures: bits 0–7 are
looked up by id in the region's terrain-scene table (prefix 0x12) and bits 8–14 select a
scene slot of that entry, which leads through the scene-choice table (0x11) to a scene
record (0x02). `scenery.md` §3–§7 specifies the tables and the generation. Texturing uses
only §3.8. A block whose words are all 0 has no procedural scenery.

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
| 0 | `material` | u32 | **Terrain-type key**: the `key` of an entry of the region's terrain type table, prefix 0x19 (`terrain-texturing.md` §3, §4.1). 0 – 1,420 in terrain chunks (1,409 distinct values); 390 distinct values in ceiling chunks. 0 is the most common single value. |
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

| `whole` | `prims` | `mask` | Count | Reading (what the client draws: `terrain-texturing.md` §4.3, BIN there) |
|---|---|---|---|---|
| 0 | > 0 | non-zero | 629,340 | Draw these triangles through this mask. **Every such mask exists** in the surface archives. |
| 0 | > 0 | 0 | 67,409 | Draw these triangles with mask value 1 everywhere. |
| 0 | 0 | non-zero | 142,091 | Nothing (skipped; does not count as the first drawn layer). **None of these masks exists** in the install. |
| 0 | 0 | 0 | 55,944 | Nothing (skipped). |
| 1 | 0 | non-zero (resolves) or 0 | 8,084 + 9,170 | **All 2,048 triangles of the block** (the full-grid triangulation of §3.6: bit 7 in terrain chunks, bit 0 and clockwise in ceiling chunks, §3.5), through the mask or with mask value 1. `start`, `prims`, `min_vertex`, `vertex_span` are 0 and ignored. |

Masks (`terrain-texturing.md` §5, all 646,073 masks of drawn layers): format 28 (A8),
square, 128, 256 or 512 texels; read the size from the surface, not from the type entry.

Meaning of the layers, settled in `terrain-texturing.md` (the former OPENs (1)–(4) of this
section):

1. `material` is a key of the region's terrain type table (0x19, region-descriptor slot 8):
   that spec §4.1 (none of 925,884 layers out of range).
2. Order and blending: layers are stored in ascending `draw_order` of their entries; stored
   order is the draw order, bottom first; the first drawn layer is opaque and every later
   drawn layer is blended "over" with its per-pixel opacity: that spec §4.2, §7, §8.
3. Mask orientation: texel row 0 lies on the block's **south** edge, column 0 on its west
   edge, coordinates (x / 160, y / 160): that spec §5.2 (unlike the minimap tiles of §6.3).
4. `whole` = 1: the table above, that spec §4.3.

<a id="s3-8-3"></a>
#### 3.8.3 Index list and its vertex numbering

Each u16 is a **vertex number** *v* of the 33 × 33 grid, 0 ≤ *v* < 1,089 (all values in
range). Three consecutive numbers form one triangle.

**Vertex numbers are Y-major: v = 33 · y_index + x_index**, the transpose of the height
array's order. Vertex *v* therefore has x_index = *v* mod 33, y_index = ⌊*v* / 33⌋, and
its height is the height-grid element 33 · (*v* mod 33) + ⌊*v* / 33⌋.

Evidence (two independent tests):

- With this reading, the diagonal used by every stored terrain-chunk triangle agrees with
  bit 7 of the vertex-flag grid read in height-grid order (65,471,762 of 65,471,762 sampled
  triangles); every ceiling-chunk triangle agrees with bit 0 (§3.5).
  Reading *v* in height-grid order instead agrees for only 96.6 %.
- Ceiling-chunk triangles should use vertices that have a ceiling. Of 15.87 M ceiling
  triangle vertices on non-flat ceilings, 217,960 have a ceiling sample only under this
  reading and 21,535 only under the other (the rest agree under both).

Under this reading every stored triangle is **counter-clockwise seen from above**
(458,061,498 terrain triangles, all of them). Each triangle covers half of one 5 × 5 quad.

<a id="s3-8-4"></a>
#### 3.8.4 Quad grid

1,024 u16 values, one per quad (32 × 32). Every value is the `material` (terrain-type key)
of one of the chunk's layers (86,726 of 86,726 chunks).

**Order (Confirmed):** quad (column qx along X, row qy along Y) at index **32 · qy + qx**, the
same Y-major order as the index list (`terrain-texturing.md` §4.6: 93.6–94.9 % agreement with
a recomputation per region, 51–56 % for the transposed order).

**Meaning (Probable):** the terrain type most visible in that quad after the layers are
composited (`terrain-texturing.md` §4.6). It is not used for drawing. Procedural scenery reads
it to test each type's "procedural content allowed" flag (`scenery.md` §7.4 step 7); the
type entry's physics material suggests footstep and ground-type use as well.

> **OPEN:** The exact weighting the exporter used for "most visible" (`terrain-texturing.md`
> §4.6).

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
| `code` | *n* bytes: the quantised bounding-volume program of `collision.md` §3 |
| `build_byte` | u8, value 1. Present **iff the record's `stamp` ≥ 38** (`mesh.md` §10.4) |

Observed: `has_terrain_tree` = 1 in 58,546 records (49,919 of them with stamp ≥ 38 and a
`build_byte`); `has_ceiling_tree` = 1 in 1,095. The stamp rule held in every record. The
`origin` is about −0.05 on the axes where the block starts at 0, and `code` starts with
byte 0x28 in the samples looked at, as for meshes (`mesh.md` §10.2).

Fallback without a stamp: after the code, a ceiling tree is followed by the vertex-flag
count u32 1,089 (bytes 41 04 00 00); if the next four bytes are not that, a `build_byte`
is present. For the terrain tree, a `build_byte` is present exactly when consuming it lets
the remaining bytes parse as parts 13–14 with nothing left over.

The code is specified in `collision.md` §3 (opcodes, integer coordinates, queries). For
terrain trees its keys are grid triangle numbers 2 · (32 · j + i) + side, and every terrain
code reports exactly the 2,048 grid triangles of §3.11's triangulation (`collision.md` §6.1,
§6.2); ceiling trees use the same numbering over the ceiling grid (`collision.md` §6.4).
331,604 records have no terrain tree, so the tree is not needed to collide with terrain. A
decoder parses past the code by its length; Tessera builds its own collision structure from
the triangles (`collision.md` §3.8, a Tessera Decision).

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
  it uses the diagonal given by bit 7 (§3.6) and is counter-clockwise from above; in
  **ceiling** chunks it uses the diagonal given by bit 0 (§3.6) and is clockwise from above
  (§3.5).
- **HM7**: edge samples equal the neighbour's (§3.3).

<a id="s3-13"></a>
### 3.13 What remains unknown in landblock data

| Part | Status |
|---|---|
| Heights, axis order, triangulation | Known |
| Height scale | Known: the client's rule stored × 2,000 / 65,535 (§3.4, BIN) |
| Ceiling grid and when it exists | Known; faces down; diagonal from bit 0 for every quad (§3.5); rendering of ceiling vertices below the floor OPEN |
| Vertex flags bits 7 and 0 | Known: terrain and ceiling diagonals (§3.6) |
| Vertex flags bits 1–6 | OPEN (§3.6); always all set or all clear together |
| Terrain triangles in no layer | Known: not drawn, kept in collision, not cave mouths (§3.5) |
| Coarse 17 × 17 grid | Known: per-vertex scenery word (§3.7, `scenery.md`) |
| Surface layers | Known: terrain-type keys, draw order, masks, `whole` (§3.8, `terrain-texturing.md`) |
| Quad grid | Order known; meaning Probable (§3.8.4) |
| Collision trees | Known: `collision.md` §3, §6 (§3.9) |
| Box list | Layout known; meaning OPEN (§3.10) |
| Water | Not stored here: per-landblock water comes from block-map keys 4 and 7 and the height-grid minimum (`water.md` §6) |
| Vertex normals | Not stored; computed by the client (§3.14) |
| Chunks without layers | Known: drawn or not by the block's open flag (§3.15, `terrain-texturing.md` §4.9) |

<a id="s3-14"></a>
### 3.14 Vertex normals of terrain and ceiling chunks (BIN2 (g); L36, L37)

The client computes one normal per grid sample, separately for the terrain chunk (terrain
heights, bit-7 diagonals) and for the ceiling chunk (ceiling heights, bit-0 diagonals, only
in landblocks with a ceiling), and writes it unchanged into the chunk's vertices: the vertex
builder neither negates nor renormalises it. These are the "separate per-block ceiling
normals" of `terrain-texturing.md` §4.8, whose OPEN this section answers.

**Algorithm in words**, for sample (*i*, *j*), 0 ≤ *i*, *j* ≤ 32, *i* along X:

1. Let *s* = +1 for the terrain chunk and *s* = −1 for the ceiling chunk.
2. A corner (*a*, *b*) is the point (5*a*, 5*b*, *h*) in units, with *h* the stored value of
   sample (*a*, *b*) of the chunk's grid converted with §3.4. When *a* or *b* is −1 or 33 the
   corner comes from the neighbouring landblock in that direction (X − 1 or X + 1, Y − 1 or
   Y + 1, both for a diagonal neighbour), at its sample 31 (for −1) or 1 (for 33). A triangle
   with a corner that cannot be read is skipped: the neighbouring landblock's data is not
   loaded, or, for the ceiling, that landblock has no ceiling grid.
3. Name the corners around the sample, with the X offsets multiplied by *s*:
   O = (*i*, *j*); S = (*i*, *j* − 1); N = (*i*, *j* + 1); W = (*i* − *s*, *j*);
   E = (*i* + *s*, *j*); SW = (*i* − *s*, *j* − 1); SE = (*i* + *s*, *j* − 1);
   NW = (*i* − *s*, *j* + 1); NE = (*i* + *s*, *j* + 1).
4. Four quads are tested **without** the factor *s*: (*i* − 1, *j* − 1), (*i*, *j* − 1),
   (*i* − 1, *j*), (*i*, *j*). A quad's bit is bit 7 (terrain) or bit 0 (ceiling) of the
   vertex-flag byte of sample (*qi*, *qj*) of this landblock (§3.6); when *qi* or *qj* is −1,
   of sample 31 in that axis of the neighbouring landblock; when that landblock is not
   loaded, the bit counts as set.
5. The triangles that contribute (corner order matters):

   | Quad tested | Bit set | Bit clear |
   |---|---|---|
   | (*i* − 1, *j* − 1) | (O, W, SW) and (SW, S, O) | (S, O, W) |
   | (*i*, *j* − 1) | (E, O, S) | (SE, E, O) and (O, S, SE) |
   | (*i* − 1, *j*) | (W, O, N) | (NW, W, O) and (O, N, NW) |
   | (*i*, *j*) | (O, E, NE) and (NE, N, O) | (N, O, E) |

6. The normal of a triangle (A, B, C) is (B − A) × (C − A), normalised to length 1.
7. The vertex normal is the sum of the normals of the triangles that were not skipped,
   divided by their number. It is **not** normalised again, so on a crease it is shorter
   than 1.

For the terrain (*s* = +1) these are the triangles of the four quads that touch the sample,
with the quads' own diagonals: each quad gives one triangle, or two when its diagonal runs
through the sample, so a sample averages **four to eight** triangles (corrected 2026-10-07;
the first 2026-10-07 revision said "up to six"). All five counts occur (L45: interior terrain
samples of all records, 4 at 9,201 samples, 8 at 6,382). Every terrain normal points up
(counter-clockwise from above). For the ceiling (*s* = −1) every X offset is mirrored while the diagonal tests are
not, and the corner order makes every normal point **down**. On a plane the ceiling result is
exactly the negated up-normal of that plane; on a curved ceiling it can differ from "the
terrain rule applied to the ceiling heights, negated" where the mirrored triangles use another
diagonal. (The `terrain-texturing.md` §4.8 Tessera Decision, "negate", is therefore right on
planes and close elsewhere; reproduce the table above for an exact match.)

**What this means for cave ceilings (Inferred from L36 and `terrain-texturing.md` §9.1).**
With a down-facing normal the per-vertex ambient clamp(*A₀* + (**N** · **U**) *H₀*) of a
flat ceiling is *A₀* − *H₀*, the down-facing ambient colour *Y* of `lighting.md` §8.1, and the
sun term clamp(**N** · *L₀*) is 0 whenever the sun is above the horizon. A ceiling is
therefore lit only by *Y* of the **outdoor** environment record (a cave under a landscape
ceiling is cell 0, outdoors, §4.12), by its `Landblock_CeilingLightMap` where the block has
one (§6.4), and by point lights. Where *Y* is dark and the block has no ceiling light map, a
cave ceiling is close to black in the original as well. Of the checklist rows
(`m4-5-visual.md`), T15's block (landblock-properties record 0x8041E13A) has a ceiling light map (0x4111307C), so its
ceiling shows those texels added to *Y*; T16's block (0x8044C9AE) has only a terrain light
map (0x411AF104) and no ceiling light map, so its ceiling gets *Y* and point lights only.
Row T5's block (landblock data 0x80039FE8) has no ceiling grid (`has_ceiling` 0) and only a terrain light
map (0x4116792D); how dark it looks is not a ceiling matter (OBS2, landblock-data and
landblock-properties records of the three blocks).

**Drawing (BIN2 (d), BIN3 (j)).** The terrain chunk, the ceiling chunk, landscape water and
frills are drawn only while the outdoor scene is visible, that is while the camera is
outdoors (§4.12, L41). The
ceiling chunk is drawn from the same layers as the terrain (§3.8, `terrain-texturing.md`
§4.8) with its own index order (clockwise from above, §3.5).

> **OPEN:** The client loads a chunk's light map (`Landblock_LightMap` for the terrain chunk,
> `Landblock_CeilingLightMap` for the ceiling chunk) only when a condition on graphics
> settings holds; two settings and a lighting-quality level take part and were not
> identified. **Tessera Decision:** always bind the light map (the setting combination of a
> default install is unknown).

> **OPEN:** Whether the terrain shaders renormalise the interpolated normal (the vertex
> normal of step 7 is shorter than 1 on creases). Not traced. **Tessera Decision:**
> normalise per pixel, as `terrain-texturing.md` §6.4 does for the tangent frame.

<a id="s3-15"></a>
### 3.15 Open blocks and chunks without layers (rule BIN in `terrain-texturing.md` §4.9; census Confirmed; L44)

What is drawn for a surface chunk (§3.8) whose `layer_count` is 0 is normative in
`terrain-texturing.md` §4.9. It depends on one per-block value computed from this record, the
**open flag**, which this section states so that a reader of landblock data can compute it:

1. A block without a ceiling grid (`has_ceiling` = 0, §3.5) is open.
2. A block with a ceiling grid is open exactly when at least one sample (*i*, *j*) with
   0 ≤ *i* ≤ 31 and 0 ≤ *j* ≤ 31 has a stored terrain height (§3.3) **strictly below** the
   stored ceiling height at the same index (§3.5). Row 32 and column 32 are not tested (it
   matters: testing them would change the flag in 140 blocks). A ceiling sample of 0 is
   compared like any other value, so it never makes a block open. Comparing the raw u16
   values or the heights converted by §3.4 gives the same flag (both scales are the same
   positive factor; checked in every block).
3. The flag serves both chunks of the block. A chunk that stores layers is drawn from its
   layers whatever the flag. A chunk without layers is drawn as one opaque full-grid layer of
   the region's lowest-`draw_order` terrain type when the flag is set, and is not added to the
   scene at all when it is clear (`terrain-texturing.md` §4.9 rules 2 and 3; the full grid is
   §3.6's bit-7 triangulation for the terrain chunk and the bit-0, clockwise one for the
   ceiling chunk, §3.5).

Census (OBS3, all 390,150 records): 6,867 blocks have a ceiling grid, 3,075 of them open and
3,792 not. Terrain chunks without layers: 299,369 in blocks without a ceiling (all drawn),
818 in open blocks with a ceiling (drawn), 3,237 in blocks that are not open (not drawn).
Ceiling chunks without layers: 944 drawn, 3,568 not drawn.

The terrain **collision** mesh does not depend on the flag: it holds all 2,048 grid triangles
in every block (§3.5, `collision.md` §6.5).

- **Invariant HM8** (Confirmed 2026-10-07): the open flag computed on raw values equals the
  flag computed on converted heights in every block.

---

<a id="s4"></a>
## 4. Landblock info `0x802RXXYY` (Confirmed, 49,636 of 49,636)

<a id="s4-1"></a>
### 4.1 Layout

| Order | Field | Type | Present when | § |
|---|---|---|---|---|
| 1 | did | DID | always | own DID |
| 2 | `flags` | u32 | always | bit 0 cells, bit 1 block-pair records, bit 2 cell groups. Observed 1 (39,252), 3 (9,704), 5 (636), 7 (44): **bit 0 is always set**. |
| 3 | `aux_word` | u32 | always | Below the payload length in 49,625 records, above it in 11. Meaning OPEN; ignore. |
| 4 | block-pair records | TSize count, entries | `flags` bit 1 | §4.2 |
| 5 | cell groups | TSize count, entries | `flags` bit 2 | §4.3 |
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
### 4.2 Block-pair records (24 bytes each)

Each record names two DIDs of landblock-info shape (`0x802RXXYY`). A named record need not
exist: of the 158,294 DIDs in these records, one (0x8025474E, named once) has no record in the
install (OBS3, 2026-10-07). A reader must not treat a missing record as an error. The field
names are Tessera's own and say nothing about direction; the order below is the byte order.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `number_a` | u32 | Meaning OPEN |
| 2 | `info_a` | DID | A landblock-info DID |
| 3 | `info_b` | DID | A landblock-info DID |
| 4 | `link_type` | u32 | Same code space as §4.6 |
| 5 | `number_b` | u32 | Meaning OPEN |
| 6 | `pair_word` | u32 | Large varying values; meaning OPEN |

Present in 9,748 records.

<a id="s4-3"></a>
### 4.3 Cell groups

"Cell group" is Tessera's name for this part.
Each entry: u16 `lead`, then u32 count *k*, then *k* × u16 `members`. All `members` values
are cell indices of the same record (1,288 entries, 2 – 902 members); `lead` is a cell index
in 608 of them. Present in 680 records.

**Meaning (BIN2 (d), (e); L25, Confirmed 2026-10-07):** each entry is a **visibility group**.
`lead` is the group's **key**, not a cell (that it equals a cell number in 608 entries is a
coincidence of small numbers); `members` are the cells of the group. A cell names its own
group with `extra` (§4.4 part 7d), and other cells name the group in their list B to make
its members potentially visible (§4.4 part 7c, §4.12). All members of a group share one
visibility cell, and no visibility portal is made between two cells of the same group
(§4.11). A key is unique within its record.

Records whose cells lack `cell_flags` bit 5 (142 records) have no groups part; there each
cell's list B is its own group, given as cell numbers, and the client gives that list a
fresh key of its own (§4.11).

<a id="s4-4"></a>
### 4.4 Cells

Cells are interior spaces (rooms, caves, dungeon pieces) placed in the landblock.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `index` | u16 | Cell number. 82,996 below 0x100, 21,275 at or above. Unique within the record (LI9, L43). |
| 2 | `position` | Position (`property-system.md` §4.6) | Always carries region, landblock (= this record's) and cell (= `index`). x, y, z are present in all cells **but 2** (flag byte 0x4B: region, landblock, cell and the payload-less 0x40 bit only; cell 0x2 of 0x8021FAF9 and of 0x8021FAFA). Orientation, (w, x, y, z) (`coordinates.md` §11.1), in 93,172 cells. Flag bytes seen: 0x3B (54,917), 0x7B (38,255), 0x1B (5,906), 0x5B (5,191), 0x4B (2). |
| 3 | `cell_flags` | u32 | Bit 2 (0x04) always set; bit 5 (0x20) in 92,133; bit 3 (0x08) in 87,512; bit 4 (0x10) in 9,493. Other bits never set. Meaning (BIN2 (e), L27): bit 0 (0x01) outdoor-walk entry (§4.12; never set); bit 2 property set present (part 8); bit 3 kept by the client, use not traced; bit 4 the sky is visible from this cell (§4.12); bit 5 the tail format of parts 7a–7d. Every record has either all cells with bit 5 (1,291 records) or none (142 records). |
| 4 | `geometry` | DID | Always a **0x01** record in `client_general.dat` (1,033 distinct, all exist) |
| 5 | `neighbour_count` *k* | u32 | 0 – 6 in 104,268 cells, 24 in 2, 54 in 1 (corrected 2026-10-07, OBS2; the former "0 – 10" was wrong). Equals the geometry's portal count (`collision.md` §5.8). |
| 6 | neighbours | *k* entries, each 15 bytes in this install | See the neighbour-entry table below (revised 2026-10-07, L26). The former reading "u32 ordinal, u16 neighbour cell, u16 back-reference, 3 × u16 (always 0), u8 flag" covers the same bytes while the list of field 4 is empty, which it is in every entry. |
| 7a | `index_echo` | u16 | If `cell_flags` bit 5: equals `index` (all). The client skips this value when it walks the cell's potentially visible cells (§4.12). |
| 7b | list A | u32 count, count × u16 | always; every value is a cell index of this record, never the cell itself. **Potentially visible cells** of this cell (BIN2 (d), L31). |
| 7c | list B | u32 count, count × u16 | always. With bit 5: **group keys** (§4.3) whose members are also potentially visible; all 129,684 values are keys of this record's groups, and the cell's own group (`extra`) is always among them when it has one (L25). Without bit 5: the cell's **own visibility group** as cell numbers; it contains the cell itself in all 9,288 non-empty lists (2,850 are empty) (L25). |
| 7d | `extra` | u16 | If `cell_flags` bit 5: the key of the cell's own group (§4.3), 1 – 11 in this install, or 0xFFFF = in no group. Every cell with a key is a member of that group (78,069 cells); no cell with 0xFFFF is a member of any group (14,064 cells) (L25). |
| 8 | properties | embedded property set | If `cell_flags` bit 2 (always). Holds `Render_AdjustableScale` and `Physics_AdjustableScale` in every cell, often `Appearance_InstanceList`, `Dungeon_DID`, interior fog and light values, `Ambient_*` |

Cell positions use the landblock frame but are not confined to 0–160 (`coordinates.md`
§6). The 0x01 geometry records are decoded in `collision.md` §5 (physics mesh, cell-volume
planes, portals, wall polygons, hull, properties and an embedded render mesh, §5.7, which is
what a renderer draws). A cell places its geometry by `position` and orientation as an active
rotation (`collision.md` §5.8). **Neighbour entry *k* describes portal *k* of the cell's
geometry:** the neighbour cell is the cell seen through that portal and the back-reference is
the index of the same opening among the neighbour's portals (`collision.md` §5.8; reciprocal
in all 393,842 entries).

A point enters a cell from the outdoor world by **containment**, not through a portal: the
client puts a phantom shaped like each cell's hull into the landblock's physics world and
takes the first cell whose phantom overlaps a small cube around the point and whose cell
volume contains it (`collision.md` §5.8.1, BIN; L23). No portal leads outside.

**Neighbour entry (BIN2 (e), L26; Confirmed 2026-10-07 on all 393,842 entries; field names are Tessera's own):**

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `portal` | u32 | Number of the portal in the cell's geometry (`collision.md` §5.5); always equal to the entry's position 0 … *k* − 1. The client selects the portal polygon by this field, not by the position (L39) |
| 2 | `neighbour` | u16 | Cell seen through the portal; always a cell of this record, never 0 or 0xFFFF (§4.12) |
| 3 | `back_reference` | i32 | Number of the same opening among the neighbour's portals; 0 – 53 in this install (high half 0 in all entries). The client makes no visibility portal for a negative value (§4.11; none occurs) |
| 4 | `extra_cells` | u32 count *c*, *c* × u16 | Tessera's name; *c* = 0 in every entry, meaning OPEN |
| 5 | `flag` | u8 | 0 or 1 (the client rejects other values); 1 in 392,318 entries, 0 in 1,524; not used by the visibility code of §4.11 |

An entry is 15 + 2*c* bytes. A decoder must read field 4 as a counted list (bounded by the
remaining bytes, §10), not as fixed zeros.

> **OPEN:** Meaning of `cell_flags` bit 3, of the neighbour `flag` and `extra_cells`
> (`collision.md` Q3); how often the client re-evaluates a moving character's cell (after
> every step, or only when its phantom starts or stops overlapping a cell phantom;
> `collision.md` Q3, Tessera Decision there). *(Bits 0, 2, 4 and 5, lists A and B and `extra`
> answered 2026-10-07, L25–L27.)*

**Cells without an offset or an orientation (L42, Confirmed 2026-10-07; answers the former
OPEN of this section).** A `position` field that is absent has the value that made the writer
leave it out (`property-system.md` §4.6): an absent x, y, z is (0, 0, 0) and an absent
orientation is the identity (1, 0, 0, 0). A cell is placed with these values like any other:

- the 2 cells with flag byte 0x4B (cell 0x2 of 0x8021FAF9 and of 0x8021FAFA) lie at the
  landblock origin with the identity orientation. Placed so, all 5 of their portal joins
  (§4.4 neighbour entries, `collision.md` §5.8) match their neighbours' portals exactly
  (0 units at every corner), so this is the placement the data was built for;
- the 11,099 cells without an orientation use the identity rotation.

A loader therefore never skips a cell for a missing offset (the former advice to skip such a
cell is withdrawn).

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
| 2 | `link_id` | u64 |
| 3 | `end_id_1` | u64 |
| 4 | `end_id_2` | u64 |
| 5 | `link_type` | u32 (L24) |
| 6 | properties | embedded property set |
| 7 | `has_pair` | u8, 0 or 1 |
| 8 | block pair | 4 × u32 (number, info DID, info DID, number); present only if `has_pair` = 1 |

The names of fields 2–4, 7 and 8 are Tessera's own.

> **OPEN:** what the three u64 values identify (the link itself and its two ends are
> likely, but not checked against the static entities of §4.5), which end is which, and the
> meaning of the two numbers of the block pair.

`has_pair` = 1 in 79,147 links; both DIDs of the block pair have the landblock-info shape in
all, and one of them is this record's in all but 6. One of the 158,294 DIDs, 0x8025474E, names
no existing record (OBS3, 2026-10-07; the same DID as in §4.2); a missing record is not an
error.

`link_type` codes observed, with their labels from the install's `EntityLinkType` enum
mappers (L24; interoperability names from the data) and counts: 7 Generator_PositionSet
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

TSize count, then entries of u64 `object_id` (Tessera's name) followed by an embedded
property set. These are server-side object templates (spawn generators and the like; a
published loader resolves their `Generator_ProfileDID` property, Reported). An offline
walker skips them.

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
  this record; the high half of each neighbour `back_reference` is 0 and its `extra_cells`
  list is empty (§4.4; the former wording "the three neighbour u16 are 0" says the same for
  this install); `index_echo` = `index`.
- **LI4**: every cell `geometry` DID has prefix 0x01 and exists.
- **LI5** (Confirmed 2026-10-07, OBS2): in a record either every cell has `cell_flags` bit 5
  or none has; group keys (§4.3) are unique within a record; every cell is a member of at
  most one group, and it is a member exactly when its `extra` is a key of the record, and
  then of that group.
- **LI6** (Confirmed 2026-10-07): every list-B value of a bit-5 cell is a group key of its
  record, and the cell's own key (`extra` ≠ 0xFFFF) is among them; a non-empty list B of a
  cell without bit 5 contains the cell.
- **LI7** (Confirmed 2026-10-07): list A never contains the cell itself; every neighbour of a
  cell is in the cell's potentially visible set (§4.12).
- **LI8** (Confirmed 2026-10-07): no cell number and no neighbour cell is 0 or 0xFFFF (cell
  numbers run 2 – 1,569); no cell has `cell_flags` bit 0.
- **LI9** (Confirmed 2026-10-07, OBS3): cell numbers are unique within a record.
- **LI10** (Confirmed 2026-10-07, OBS3): every neighbour's `portal` equals its position in
  the list; every neighbour, every list-A value and every list-B value of a cell without
  bit 5 is a cell of the same record (so nothing a cell names lies in another landblock,
  L38).

<a id="s4-11"></a>
### 4.11 Visibility cells and portals (BIN2 (d); L28, L29)

The client draws interiors through a portal visibility system (a third-party library for
portal and occlusion culling). This section gives the partition into visibility cells and the
portals the client builds from §4.3–§4.4 and the 0x01 geometry (`collision.md` §5); any
portal-culling scheme can reproduce them.

**Visibility-cell key.** A cell is identified by region, landblock X and Y, instance and cell
number, the fields of its `position` (`property-system.md` §4.6; instance 0 for the stored
landblocks). A position whose cell number is 0 (outdoors) or 0xFFFF (none), or whose region
is 0 or landblock X or Y is 0xFF (no landblock), maps to the single **outdoor visibility
cell**, which exists from start-up and holds the whole landscape: terrain and ceiling chunks
(§3.8), landscape water and every outdoor object. (The scene-flag pass of §4.12 treats these
positions differently: there only cell 0 counts as outdoors.)

**Within one landblock (L38).** Whenever the client turns a cell number named by a cell (a
group member, a neighbour, a cell of its PVS) into a key, it takes the region, landblock and
instance of the naming cell. So a visibility cell, a portal and a PVS never join cells of
different landblock-info records, and a portal walk that starts in a cell of one landblock
reaches only cells of that landblock (LI10: every number a cell names is a cell of its own
record).

**Group list of a cell.** With `cell_flags` bit 5: the members of the group whose key is the
cell's `extra` (§4.3), or nothing when `extra` is 0xFFFF or a key the record does not have
(BIN3 (k); never the case in this install, LI5). Without bit 5: the cell's list B.

**Visibility cells**, made when a landblock's cells are loaded, for each cell in turn:

1. If the cell already has a visibility cell, nothing is done.
2. Otherwise its group list is searched in order, the cell itself skipped; the first listed
   cell that already has a visibility cell shares it with this cell.
3. If none has one, a new visibility cell is made; its frame is this cell's placement
   (`collision.md` §5.8).

For cells with bit 5 every member of a group has the same group list, so each group becomes
exactly one visibility cell in any order, and each cell in no group becomes a visibility cell
of its own. For cells without bit 5 two members' lists can differ and the result depends on
the order. **Tessera Decision:** take the cells in their stored order (§4.4). Census (OBS2):
104,271 cells form 18,390 visibility cells besides the outdoor one.

**Portals**, made after every cell of the landblock has its visibility cell, for each cell
and each of its neighbour entries (§4.4):

1. Skip the entry when its `neighbour` is in the cell's group list (both share one visibility
   cell; the opening between them needs no portal) or when its `back_reference` is negative.
2. Otherwise the portal's shape is portal number `portal` of the cell's 0x01 geometry
   (`collision.md` §5.5: four corners in the cell frame), selected by the entry's `portal`
   field (L39; it equals the entry's position in every entry), placed with the cell's frame
   (`collision.md` §5.8). The client splits the polygon into a fan of triangles: corners
   (1, *k* − 1, *k*) for *k* = 3 … corner count. If the neighbour has no visibility cell
   (its key, formed as in "Within one landblock", is not known), no portal is made.
3. The portal belongs to the visibility cell of the cell and leads into the visibility cell
   of `neighbour`. A portal is one-way; the way back is the neighbour's own entry, which
   exists for every entry (the entries are reciprocal, `collision.md` §5.8).

Census (OBS2): of the 393,842 neighbour entries, 39,692 make a portal and 354,150 lie inside
a group.

**Objects.** Every drawn object is entered into the visibility cell of the cell it belongs
to; an object that belongs to several cells is entered once per distinct visibility cell. A
cell's own render mesh (§4.14) is in the cell's visibility cell. Which cell an interior static
entity belongs to is OPEN (`entity-descriptor.md` §4.8). **Tessera Decision** (walker,
§4.15): an interior entity belongs to the cell of its landblock whose cell volume contains its
origin, tried at the origin and 0.05 units above and below it (the containment test of
`collision.md` §5.8.1); one that no cell contains is drawn whenever the camera is in any cell
of its landblock. Moving entities (characters) are, by the rule above, in the visibility cell
of the cell they stand in (Inferred; the walker does not follow this yet, §4.15).

**Each frame** the cell of the camera's position (§4.12) selects the root visibility cell. The
library returns what is visible from the root: the objects of the root and of every visibility
cell reached through a chain of portals, each seen through the ones before it (the view
narrowed at every portal), optionally with occlusion culling. **Tessera Decision:** a
recursive portal traversal that clips the view frustum to each portal's screen bounds and
recurses into the portal's target (at most 32 portals deep; a visibility cell may be entered
again through another portal); occlusion culling optional; portals treated as two-sided until
their facing is known. The walker (2026-10-07, §4.15) adds these Tessera rules, none of which
is a statement about the original:

- the view is a rectangle in normalised device coordinates, the whole screen at the root;
- a portal polygon is clipped against the plane just in front of the eye (clip-space
  *w* ≥ 10⁻⁴) before its screen bounds are taken; a portal wholly behind that plane is not
  entered; otherwise the next view is the overlap of the current rectangle and the portal's
  bounds, and an empty overlap is not entered;
- an eye within 0.5 units of a portal's plane and of its bounding box (grown by 0.5 units)
  passes the current view on unchanged (an edge-on doorway the eye stands in has no screen
  area but is looked through);
- a portal whose polygon is not known passes the current view on unchanged;
- at most 4,096 portals are crossed in one frame, as a bound on cyclic portal graphs.

**Example (IN1 of `m4-5-visual.md`, landblock info 0x802191A6):** group 1 holds cells 6 – 23
and is one visibility cell, whose only two portals lead from cell 6 into cell 5 and from cell
8 into cell 24. The doorway the IN1 walk crosses (cell 9 into cell 8) lies inside the group:
both cells are one visibility cell and no portal separates them.

> **OPEN:** Together with a portal's triangles the client passes the visibility library a
> per-portal choice derived from a field of the portal in the 0x01 geometry (possibly the
> leading byte of `collision.md` §5.5, which is 1 for every portal); it most likely sets the
> triangles' facing. Portal winding and facing therefore remain OPEN there and here.

<a id="s4-12"></a>
### 4.12 Inside versus outside: the camera's cell (BIN2 (d); L30, L31)

**Potentially visible set (PVS) of a cell:** the cell itself, every cell of its list A, and
for a cell with bit 5 the members of every group whose key is in its list B (a key the
record does not have adds nothing; BIN3 (k), never the case in this install, LI6); the cell's own
number (`index_echo`) is skipped when these lists are walked, and so is the value 0xFFFF.
For a cell without bit 5 it is the cell and list A (its list B is its own group, which shares
its visibility cell). All of these numbers are cells of the same landblock (L38). Every
neighbour of a cell lies in its PVS (LI7). Example: the PVS of cell 9 of 0x802191A6 has 25
cells (2 – 25 and 31).

**Scene flags.** Whenever the camera's region, landblock, cell or instance changes, the
client recomputes two flags, **outdoor-visible** and **sky-visible**, and a set of
visibility-cell keys:

1. **Outdoor branch.** It is taken only when the camera's position has region ≠ 0, landblock
   X ≠ 0xFF, landblock Y ≠ 0xFF and cell **0** (L40). Both flags are set. Then, for every
   loaded landblock, each of its cells with `cell_flags` bit 0 is visited **with** its PVS
   (rule 3). No cell has bit 0 in this install (LI8), so outdoors both flags stay set and no
   cell is visited.
2. **Every other position**, including cell 0xFFFF and a position without region or
   landblock: both flags start cleared and the client looks up the camera's cell (region,
   landblock, instance and cell of the position). If no such cell is loaded, nothing is
   visited and **both flags stay clear**: no landscape, the indoor environment record and no
   sky (L40). This differs from §4.11, where cell 0xFFFF maps to the outdoor visibility cell:
   an implementation must not treat 0xFFFF as outdoors when it sets the flags. If the cell is
   found, it is visited with its PVS (rule 3).
3. **Visiting a cell.** Each cell is visited at most once per recomputation. A visited cell
   with `cell_flags` bit 0 clears both flags; then, if it has bit 4, it sets sky-visible
   (so a cell with both bits leaves sky-visible set). Visiting "with its PVS" means the cell
   itself first, then each cell of its PVS in the order list A, then the members of the
   list-B groups in list order (`index_echo` and 0xFFFF skipped); the PVS cells are visited
   **without** their own PVS (one level only). The result therefore depends on the order
   only when bit 0 occurs, which it does not in this install: there, inside a cell,
   outdoor-visible is clear and sky-visible is set exactly when the camera's cell or a cell of
   its PVS has bit 4.
4. The keys of the visited cells' visibility cells, plus the outdoor one when outdoor-visible
   is set, form the set handed to the scene renderer. **Tessera Decision:** its use there was
   not traced; use it for nothing but the flags and let the portal query of §4.11 decide what
   is drawn.

**What the flags switch:**

- Outdoor-visible set: the landscape is submitted (terrain chunks, ceiling chunks, landscape
  water and frills, plus one further landscape part whose meaning was not traced) and the
  **outdoor** environment record is used (§4.13). Clear: the client sets up no landscape pass
  at all, so none of these is drawn, frills included, and the **indoor** record is used (L41).
  The landscape pass also needs a valid landscape position (region ≠ 0, landblock X and Y ≠
  0xFF, cell ≠ 0xFFFF), which always holds when the outdoor branch of step 1 was taken.
- Sky-visible set: the sky is drawn; clear: it is not. *(Inferred: indoors the sky drawn is
  that of the indoor record's sky key; OPEN.)*
- Static and moving entities and cell meshes are not switched by the flags; the portal query
  culls them.

**Consequences:**

- **From outside**, interior cells and everything in their visibility cells are never drawn:
  the outdoor visibility cell has no portal into any cell (no neighbour is 0 or 0xFFFF,
  LI8) and no cell has bit 0. This includes the cells of 0x8021FAF9 and 0x8021FAFA that
  overlap the terrain (`m4-5-visual.md` §1.1).
- **From inside**, no terrain, ceiling chunk, landscape water or frill is drawn, and the sky
  only when the camera's cell or a cell of its PVS has bit 4 (9,493 cells in 348 records).
  No outdoor object (static, generated scenery or moving) is drawn either: the portal walk
  starts in the camera's visibility cell and no portal leads to the outdoor one (LI8), nor to
  a cell of another landblock (L38). IN1: none of the 25 cells of cell 9's PVS has bit 4, so
  neither landscape nor sky is drawn there.
- The change between the two is immediate when the camera's cell changes between 0 and an
  interior cell (no blend of the environment records, §4.13).

> **OPEN:** How the client gives the camera position its cell (the camera follows the
> character and walls push it). Not traced. **Tessera Decision:** the cell lookup of
> `collision.md` §5.8.1 at the eye (the eye as placed by the camera controller, before any
> terrain clamp); when it finds no cell and the character is walking with its feet in a
> cell, the character's cell; otherwise outdoors (cell 0). While the camera is in a cell the
> walker does not keep the eye above the terrain (the terrain is not drawn then).

<a id="s4-13"></a>
### 4.13 The indoor environment: sky key, ambient, fog, sun (BIN2 (f); L32, L33)

**Property lookup at a position.** For a property the client asks, in this order, and takes
the first source that has it:

1. the property set of the cell the position is in, when it is in a cell (§4.4 part 8);
2. the landblock-info property set of the position's landblock (§4.7);
3. the block-map properties of that landblock (§5.4).

**Two environment records.** On every environment update the client builds two records from
sky keyframes (`environment.md` §11):

- the **outdoor record** from sky key A: a value held by the client's session object (Inferred:
  the local player's) in `Environment_LocalSky_HousingHook`, `Environment_PlayerLocalSkyOverride`
  or `Environment_LocalSky`, the first present in that order (set by gameplay; an offline
  walker has none); else `Environment_LocalSky` looked up at the camera position; else the
  value 2;
- the **indoor record** from sky key B, with the interior overrides below: the session
  object's `Environment_PlayerIndoorAmbianceOverride` or `Environment_IndoorAmbiance`, the
  first present; else `Environment_IndoorAmbiance` looked up at the camera position; else the
  value 1.

The indoor record is used while outdoor-visible is clear (§4.12), or when the caller forces
it; the outdoor record otherwise. Each record blends to new keyframe values on its own
(`environment.md` §11.2); changing from one record to the other is immediate.

**Interior overrides** of the indoor record, read with the lookup above at the camera
position; re-read when the camera's region, landblock or instance changes, when forced, and
otherwise at most every 2.5 s:

| Property (data label) | Effect on the indoor record |
|---|---|
| `InteriorUpperAmbientLight` (colour) | Up-facing ambient *X* (`lighting.md` §8.1) = this colour + `InteriorAmbientLight`; black (0, 0, 0, 1) when absent |
| `InteriorLowerAmbientLight` (colour) | Down-facing ambient *Y* = this colour + `InteriorAmbientLight`; black when absent |
| `InteriorAmbientLight` (colour) | Added to both, channel by channel; then every channel of *X* and *Y* is limited to at most 1 (no lower limit) |
| *(none of the three present)* | *X* and *Y* stay those of key B's keyframes (*Ca1* × *ba*, *Ca2* × *ba*, `lighting.md` §8.1). When any of the three is present, *X* and *Y* are replaced as above, without the keyframe intensity *ba*. |
| `InteriorFogType` (enum 0x2300016A) | Present: fog is on exactly when the distance-fog switch is on and the value is 1 (Linear); 0 (Invalid) and 2 (None) turn fog **off**. Absent: the keyframe's fog on or off. |
| `InteriorFogColor` (colour) | Replaces the fog colour |
| `InteriorFogNear` (f32, units) | Replaces the fog start |
| `InteriorFogFar` (f32, units) | Replaces the fog end |

- **Main directional light:** off in the indoor record; sun and moon give no light indoors.
  In the outdoor record it is on unless the keyframe light's colour is (0, 0, 0, 1) within
  0.0002 per channel (inclusive: a difference of exactly 0.0002 still counts as that colour)
  or its intensity is **below** 0.0002 (strict: an intensity of exactly 0.0002 keeps the light
  on; corrected 2026-10-07, the first 2026-10-07 revision said "at most"). A further keyframe
  condition was not traced.
- **Fog limits (both records):** the keyframe's fog end is first limited to at most 10,000;
  after the interior override the end is raised to at least 20; the start is limited to at
  most end − 20 and at least −10,000. The 10,000 limit is applied **before** the override,
  so an `InteriorFogFar` above 10,000 is kept as stored (none is stored in this install, but
  a reader must not cap it).
- Colour properties are 4 bytes in the order B, G, R, A (`property-system.md` §4.7,
  `lighting.md` §4.4). The conversion to the record's floats was not traced here.

> **OPEN (clean-room implementer, 2026-10-07):** for a camera in a cell, which landblock is
> "the position's landblock" in steps 2 and 3 of the property lookup: the landblock whose
> landblock info owns the cell (the landblock its `position` names, LI3), or the landblock
> under the camera's x, y? They differ for cells placed away from their owner (the
> implementer measured cell origins up to 6 landblocks from it, `walker.md` §13.3).
> **Tessera Decision** (`walker.md` §14.11 interiors note): the owner's landblock info and
> block map.
>
> **Answered in part (spec writer, 2026-10-07; Inferred, L38):** a position in a cell names
> the cell's **owner** landblock: cells are keyed by region, landblock, instance and cell
> number (§4.11), a cell number means nothing without the landblock whose landblock info
> holds it, and every cell's `position` names its owner (LI3). So steps 2 and 3 read the
> owner's landblock info and block map, as the Tessera Decision does; the landblock under the
> camera's x, y plays no part. Not traced directly in the client, hence Inferred.

These facts answer two implementer OPENs of `environment.md`: §7.1 (which sky key wins:
the order above) and §12 (`InteriorFogType` None and Invalid switch fog off rather than
leaving the outdoor fog in force). That spec should adopt them.

**Row Y5 worked through (camera in cell 9 of 0x802191A6, IN1 start).** Cell 9 has none of
the interior properties; the landblock-info set has `InteriorAmbientLight` stored (8, 25, 8,
255), `InteriorFogType` 1, `InteriorFogNear` 0.059, `InteriorFogFar` 90 and
`InteriorFogColor` stored (2, 14, 11, 255) (OBS2). Indoor record: *X* = *Y* = the ambient
colour (R 8, G 25, B 8); fog on (with the distance-fog switch on), linear from 0.059 to 90 units (inside the
limits) in the fog colour R 11, G 14, B 2; no sun or moon. Key B: no `Environment_IndoorAmbiance` in
the cell or the landblock-info set, and it is no block-map key, so the value 1.

<a id="s4-14"></a>
### 4.14 What a cell draws (BIN2 (d); L34, L35)

- **The mesh:** the render mesh embedded in the cell's 0x01 record (`collision.md` §5.7 part
  11, decoded as `mesh.md` describes), placed with the cell's frame (`collision.md` §5.8) and
  entered into the cell's visibility cell (§4.11). Nothing else of the 0x01 record is drawn:
  the physics mesh, cell volume, portals, wall polygons and hull serve collision and
  visibility only.
- **Appearance:** when the cell has appearance data, the client draws a private copy of the
  mesh with the appearance changes applied (BIN2; the rule was not traced). 80,825 cells
  carry `Appearance_InstanceList` (OBS2). **Tessera Decision:** apply the cell's
  `Appearance_InstanceList` to its mesh as `appearance.md` §3.2 applies an instance list to
  an entity.
- **Scale:** `Render_AdjustableScale` and `Physics_AdjustableScale` are equal in every cell:
  (1, 1, 1) in 104,220 cells, 0.3 in 48, 0.384 in 2, 2.0 in 1. The placement does not use
  them: with them, all 144 portal joins of the two landblocks with scaled cells (0x80239A60,
  0x8024FCB6) break by up to 21 units; without them, all join exactly (L35).
- **Lighting:** the indoor environment record (§4.13) while the camera is inside, point
  lights of interior Light entities (`lighting.md` §8); no landblock light map applies to
  cells. Which point lights the client selects while the camera is inside (only those in the
  visibility cells the portal walk reaches, every interior light of the loaded landblocks, or
  another rule) was not traced. **Tessera Decision** (walker, §4.15): while the camera is in a
  cell, only interior Light entities enter the light table; while it is outdoors, only
  outdoor ones.
- **Interior entities** (`entity-descriptor.md` §4.8) are drawn through the visibility cell
  of the cell they belong to (§4.11; attachment OPEN there).

> **OPEN:** Whether the render mesh of a scaled cell is scaled (about the cell origin) when
> drawn. **Tessera Decision:** not scaled, so the mesh stays on its portals and physics mesh.

> **OPEN:** Detail level and draw distance of cell meshes (`walker.md` §11.9), and what the
> cell properties `Render_CastDynamicShadows` (12 cells), `Render_CastLandscapeShadows` (12)
> and `Render_CastBakedStencilShadows` (2,638) change. **Tessera Decision** (walker, §4.15):
> a cell mesh is drawn at its finest detail level out to the global draw limit and the
> distance cap (no per-cell distance), and casts shadows within the render component's
> default shadow distance; the visibility rules of §4.11–§4.12 decide whether it is drawn at
> all.

<a id="s4-15"></a>
### 4.15 The walker's interiors and landscape switches: Tessera rules and known differences (2026-10-07)

This section records what the released walker code (`apps/client` walker: interiors,
terrain, objects; `crates/world` bundles) does where §3.15 and §4.11–§4.14 are silent, and
where it differs from them. It is not a statement about the original. Implementers check the
differences against the sections named and change the code, or raise an OPEN here.

**Tessera rules (where the client's rule is unknown or the choice is Tessera's own):**

| Item | Rule | Section |
|---|---|---|
| Camera cell | The cell lookup at the unclamped eye, else the walking character's cell, else outdoors | §4.12 OPEN |
| Portal walk | Screen rectangles, eye-plane clipping, eye-in-portal rule, unknown polygons, 32 deep, 4,096 portals per frame | §4.11 |
| Interior entities | Attached by containment of the origin (±0.05 units); unattached ones drawn whenever the camera is in a cell of their landblock | §4.11 |
| Loading | Interior objects (cell meshes and interior entities) are loaded only while the camera is in a cell of their landblock; a debugging switch loads and draws every interior whatever the camera's cell | — |
| Cell meshes | Finest level, global draw limit, default shadow distance | §4.14 OPEN |
| Point lights | Inside: interior lights only; outside: outdoor lights only | §4.14 |
| Indoor record | Overrides read at the camera's cell from the owner landblock (now Inferred to be the client's rule) and re-read on an owner-landblock change or after 2.5 s | §4.13 |
| Landscape debug views | The walker leaves out a terrain chunk that §3.15 does not draw only in its normal view (layers and lighting on); with layers or lighting switched off it draws that chunk's M3 height-shaded mesh | §3.15 |

**Known differences between the released code and this spec (OPEN for the implementer):**

> **OPEN (implementer):** cells whose `position` has no x, y, z are skipped by the walker
> (and by the collision cell placement it uses). §4.4 now places them at the landblock origin
> with the identity orientation (L42); they should be loaded, drawn and collided with there.

> **OPEN (implementer):** the walker draws a cell's render mesh without its appearance data.
> §4.14 (Tessera Decision there) applies the cell's `Appearance_InstanceList` as
> `appearance.md` §3.2 does for entities.

> **OPEN (implementer):** characters and other moving entities are drawn whatever the
> camera's cell. By §4.11 (Inferred for moving entities) they belong to the visibility cell
> of the cell they stand in, so from inside, a character standing outdoors is not drawn, and
> from outside, one standing in a cell is not drawn.

> **OPEN (implementer):** the walker's scene flags take a simplified form of §4.12: outdoors
> it clears both flags when any loaded landblock has a cell with bit 0 (instead of visiting
> those cells with their PVS, rule 3, where a later cell with bit 4 sets sky-visible again);
> inside it ignores bit 0 of the visited cells. Both give the §4.12 result on every record
> of this install (no cell has bit 0, LI8). Bring the code in line with rule 3, or keep it
> and add a test that fails if a cell with bit 0 appears.

> **OPEN (implementer):** the walker limits the overriding interior fog end to 10,000 (it
> takes the renderer's limits of `lighting.md` §10). §4.13 limits only the keyframe end, before
> the override. No stored `InteriorFogFar` exceeds 10,000 in this install.

> **OPEN (implementer):** the landblock-info decoder (`crates/formats`) still reads the last
> six bytes of the fixed neighbour-entry part as three u16 values, which equals §4.4's layout
> only while `extra_cells` is empty (every entry of this install). §4.4 requires the
> back-reference as an i32 and `extra_cells` as a counted list; the walker's "negative
> back-reference" test (bit 15 of the high half) is the sign of that i32. The decoder's
> field names for the block-pair records, cell groups and links (§4.2, §4.3, §4.6) also
> still predate the 2026-10-06 renaming.

> **OPEN (implementer):** the walker builds a cell's portal polygon from the neighbour
> entry's position in the list; the client uses the entry's `portal` field (L39). They are
> equal in every entry of this install (LI10); use the field, or check the equality when
> decoding.

---

<a id="s5"></a>
## 5. Block maps `0x801RXXYY` and descriptors (prefix 0x18)

A block map assigns each landblock a value for about 24 per-landblock properties (area,
music, sky, level cap, …) by index into a per-region **descriptor** table.

<a id="s5-1"></a>
### 5.1 Finding the descriptor (Confirmed)

Region number → REGION DID map 0x28000008 → region descriptor 0x0F… → the key-0 entry of its
seventh DID map (`environment.md` §3.1; offset 102 in every record of this install) →
descriptor 0x18…. In this install:

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
regions 2, 3, 4, 5 and 14 key 0x10000012, which no 0x18 descriptor lists. Their values are
**not** palette indices: the client uses the raw u16 (BIN (c)). Another region table names
each key in its own header (the u32 at offset 8, or 12 for the water type), and the client
reads that field and looks the key up in the landblock's block map (BIN (c)):

| Key | Named by (region-descriptor slot) | Meaning | Values in this install | Spec |
|---|---|---|---|---|
| 3 | Fog-zone table, prefix 0x17 (slot 6), offset 8 | Fog-zone number, looked up at the landblock and its east, north and north-east neighbours | 0 in all 390,150 (no active zone) | `water.md` §5 |
| 4 | Water table, prefix 0x16 (slot 5), offset 8 | Water level *L*; height *L* × 1000 / 255 | 0–255; up to 182 distinct per region | `water.md` §6 |
| 6 | Encounter table, prefix 0x14 (slot 3), offset 8 | Encounter zone (Probable: the client reads that table's key field the same way; the encounter pass is out of scope offline) | region 1 only: 0, 1, 4, 6, 12–15; 0 elsewhere | `scenery.md` §7.7 |
| 7 | Water table, offset 12 | Water type (an entry key of the water table) | 0–47 | `water.md` §6 |
| 0x10000012 | not found | OPEN | 1 in 5 blocks of region 2, else 0 | |

> **OPEN:** Which record or code reads key 0x10000012. Readers keep the raw (key, value)
> pairs of every key.

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

The surfaces are DXT1, 128 × 128 (1.25 units per texel). Settled in
`terrain-texturing.md` §10 (the former OPEN of this section):

- **Orientation (Confirmed):** the same as the layer masks, row 0 on the block's south edge,
  column 0 on its west edge, sampled at (x / 160, y / 160); seam test on 4,323 adjacent block
  pairs. This is the opposite row order to the minimap tiles of §6.3.
- **Use (BIN, `terrain-texturing.md` §10 and T22):** bound as material parameter 3019 with
  clamp addressing and bilinear filtering, and **added** to the lighting sum of every layer,
  not multiplied.
- **Content (Probable):** baked light from placed light sources; 79 % of texels are black
  (OBS).
- The ceiling variant (`Landblock_CeilingLightMap`) applies to the ceiling chunk in the same
  way.

---

<a id="s7"></a>
## 7. Kind 5 `0x805RXXYY`: distant-landscape sector (Confirmed framing; DB type BIN; most of the layout OPEN)

- **What it is (BIN (b)):** the client registers DID range 0x80500000–0x805FFFFF for DB type
  0x41, labelled DISTANT_LANDSCAPE_SECTOR: the low-detail landscape for a 3 × 3 group of
  landblocks, drawn beyond the full-detail rings (`terrain-texturing.md` §12). It is not the
  navigation record (that is `K` = 6, absent). Whether the frill engine's 3 × 3 "source
  groups" (ER §3) read it is not known (`scenery.md` §15 item 12).
- Exists only where X and Y are multiples of 3 (33,159 records), and only with stamps 38 or
  39. One record per 3 × 3-landblock group.
- Layout: own DID; u32 *n* = 9,801 (= 99 × 99) in every record; *n* × 5 bytes; then a
  trailing section that starts with u32 0 in every record.
- In each 5-byte sample, bytes 0–1 read as a u16 on the terrain height scale and bytes 2–4
  as three unsigned bytes near (127, 255, 127) (an up-pointing vector in a biased encoding
  is the guess). The u16 values do **not** equal the terrain heights at the obvious
  corresponding points (median difference 15 units).
- **Water (Confirmed):** the trailing section carries the sector's water list, a VLE count
  and that many pairs of (u8 water type, f32 height), and per landblock a "has water" byte and
  the index of its pair (BIN). In all 12,925 records whose landblocks include a wet one, every
  (type, height) pair predicted by `water.md` §6.2 occurs, bit-identical. The client builds
  its landscape water quads from these pairs (`water.md` §8.1, §9); a reader can derive the
  same pairs from the block maps instead.

> **OPEN:** Sample spacing and meaning, and the rest of the trailing section (the client's
> reader has several versioned sub-objects: a mode byte, a sub-object present when the mode is
> non-zero, a u32, the water list, a list of 23-byte entries and, for newer versions, further
> counted lists). Not required by M3 or M4.

> **OPEN (implementer, 2026-10-06):** The water list cannot be read positionally from this
> section: its byte offset in the trailing section (equivalently, the grammar of the mode
> byte, the mode-dependent sub-object and the u32 before it) is not given, nor is the encoding
> of the per-landblock part (width of the "has water" byte's companion index, whether the nine
> entries come before or after the list, and their landblock order, X-major or Y-major).
> `tessera-formats` therefore keeps the section raw and offers only a byte search for a given
> (type, height) pair (the WA5 check), which on this install finds 89,913 of the 89,914 pairs
> predicted from the block maps by `water.md` §6.2 steps 5–7 when every non-zero type is
> taken as wet (the one miss is consistent with the dry type 37 of `water.md` §6.1). Distant
> water follows the Tessera rule of `water.md` §9 (derived from the block maps) until the
> offsets are specified.

---

<a id="s8"></a>
## 8. Loading a landblock bundle (algorithm in words)

For landblock (R, X, Y):

1. Data: read `0x800RXXYY` from `client_cell_R.dat` (always present for 0 ≤ X, Y ≤ 254).
   Decode §3 with the record's directory stamp. Keep the heights, the whole vertex-flag
   byte of every sample (bit 7 meshes the terrain, bit 0 meshes the ceiling; §3.5, §3.6),
   the ceiling grid and the surface chunks; keep the other parts raw.
2. Info: if the header's info DID is non-zero, read it (same archive) and decode §4 with the
   record's stamp.
3. Block map: read `0x801RXXYY` from `client_map_R.dat` and resolve it (§5.4) with the
   region's descriptor (cached per region).
4. Properties: read `0x804RXXYY` if present (§6).
5. For seamless normals, also request the heights of the east, north and north-east
   neighbours (their edge samples equal this block's; §3.3).
6. Water: decide the landblock's water from the block map (keys 7 and 4) and the height
   grid's minimum with `water.md` §6.2; the fog-zone key 3 needs the east, north and
   north-east neighbours' block maps (`water.md` §5.3).
7. Cells (2026-10-07): when the info record has cells, place every cell (§4.4, including
   cells without an offset or orientation), give each cell its visibility cell and then make
   the portals (§4.11); the camera's cell decides inside or outside (§4.12), the indoor
   environment (§4.13) and what is drawn (§4.14). Normals of the terrain and ceiling chunks
   need the eight neighbouring landblocks' data at the edges (§3.14), and whether a chunk
   without layers is drawn needs the block's open flag (§3.15).

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
- **World parts (2026-10-07).** The bundle job also builds the landblock's world systems in
  the order of `collision.md` §9.4 (terrain collision mesh and water decision, scenery
  generation, water, collision set); each part's heap bytes are added to the charge,
  including the static pass's footprint markers that scenery keeps for frill spacing
  (`scenery.md` §8.2 item 5). These parts follow their own specs; nothing in them changes
  the records of §3–§6.
- **Stage times (2026-10-07, diagnostics only).** A bundle job that takes 250 ms or more logs
  the wall time of each of its stages at debug level. This has no effect on the bundle.

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
| `BadFlag` | A `has_*` byte (including the link's `has_pair`, §4.6) or `whole` byte not 0/1 |
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
>
> **Answered (2026-10-05, spec writer):** `environment.md` §3.1 now gives the region
> descriptor's full grammar (BIN): fixed fields, nine DID maps (each a TSize count and
> (u32 key, DID) entries) and a trailing DID. Every record of this install has one entry in
> maps 1–7 and none in maps 8–9, hence 114 bytes, but a decoder should parse the grammar
> rather than require the size. The block-map descriptor is the key-0 entry of map 7 (the
> client asks every map for key 0 only); the prefix rule gives the same DID in this
> install.
>
> **Implemented (2026-10-06, implementer):** `tessera-formats` now parses that grammar (no
> size check) and takes every table as the key-0 entry of its map (slot 8: the trailing DID).
> Decoder limit (Tessera policy, not a fact about the data): at most 8 entries per DID map
> (every observed map holds 0 or 1); a larger map is rejected as `Limit`.

<a id="s10"></a>
## 10. Hostile-input notes

- Bound every count by the bytes remaining: a layer is 17 bytes, an index 2, a quad entry
  2, a box 24, a block-pair record 24, a neighbour at least 15 (15 + 2 × its `extra_cells`
  count, §4.4), a block-map entry 6.
- `code_length` of a tree is bounded by the remaining bytes (`mesh.md` §10.3).
- Validate layer windows before slicing the index list.
- Cap the cell, entity, link and weenie counts by remaining bytes as well.
- Keep checks and lookups linear in the record size: membership tests over cell indices
  (LI3) and block-map key lookups (§5.4) need an index (cell indices are u16, so a
  65,536-bit set suffices), or a hostile record makes them quadratic.

<a id="s11"></a>
## 11. Open questions (summary)

1. Bits 1–6 of the vertex-flag grid (§3.6). *(Bit 0 answered: the ceiling diagonal,
   `collision.md` §6.4.)*
2. *(Answered: the 17 × 17 coarse grid is the per-vertex scenery word, §3.7 and `scenery.md`.)*
3. *(Answered 2026-10-05: surface-layer `material`, order, blending, mask orientation and
   `whole`, §3.8.2 and `terrain-texturing.md` §4–§8; quad-grid order, §3.8.4.)* Remaining:
   the quad grid's exact weighting (§3.8.4).
4. Rendering of "ceiling below floor" vertices (§3.5); box list (§3.10). *(Ceiling diagonal
   answered: bit 0 for every quad, `collision.md` §6.4, Q5 answered; hole triangles are kept
   in collision, `collision.md` §6.5.)*
5. Landblock-info `aux_word`, trailer, the block-pair `number_a`, `number_b` and
   `pair_word`, the link ids, `cell_flags` bit 3, the neighbour `flag` and `extra_cells` (§4).
   *(Placement of the 2 cells without an offset answered 2026-10-07: the landblock origin,
   §4.4, L42. The 0x01 cell geometry, the neighbour
   meaning and entering a cell from outdoors are answered by `collision.md` §5, §5.8.1; only
   the re-evaluation frequency stays open, `collision.md` Q3. Cell groups, cell flags 0, 2,
   4 and 5, lists A and B and `extra` answered 2026-10-07, §4.3, §4.4, §4.11, §4.12.)*
6. Block-map key 0x10000012 (§5.6; keys 3, 4, 6 and 7 answered); descriptor `flag` byte
   (§5.2).
7. *(Answered 2026-10-05: light-map orientation and use, §6.4 and `terrain-texturing.md`
   §10.)*
8. Kind 5: sample meaning and the rest of the trailing section (§7; the record kind and its
   water pairs are known).
9. In-game height test points (§3.4).
10. Interiors (§4.11–§4.14): portal facing; how the camera position gets its cell; which sky
    is drawn indoors when a visible cell has bit 4; the use of the visible-cell set in the
    scene renderer; which point lights are used indoors; render scale, detail level, draw
    distance and shadow properties of cell meshes; the meaning of the fourth landscape part
    switched by outdoor-visible (§4.12). *(Answered 2026-10-07: the portal polygon is chosen
    by `portal`, visibility stays inside one landblock, frills are switched by
    outdoor-visible, the landblock of the indoor property lookup is the owner's (Inferred).)*
11. Ceilings and terrain (§3.14): the graphics-setting condition under which light maps are
    bound; whether the shaders renormalise the vertex normal.
12. Implementation differences of the released walker (§4.15): seven implementer OPENs.

<a id="s12"></a>
## 12. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First version (§0–§11). |
| 2026-10-05 | Sweep adopting the correction tables of `terrain-texturing.md` §15, `collision.md` §15 and `water.md` §13, plus new BIN facts. §1: tags TT, COL, SCN, WAT and BIN; L4 revised; L7 and L16 updated; new L18–L22. §2.1: DB type of every kind; `K` = 6 registered but absent. §3.4: height rule changed to the client's stored × 2,000 / 65,535 (BIN), largest difference to the former stored / 32.768 is 0.0305 units. §3.5: ceiling facing, collision numbering and terrain holes cross-referenced; OPEN narrowed. §3.8.2: `material` is a terrain-type key; the layer-kinds table states what the client draws; mask census; former OPENs (1)–(4) answered. §3.8.4: order Confirmed, meaning Probable. §3.9: tree code specified by `collision.md` §3. §3.13: summary updated. §4.4: 0x01 geometry and neighbour meaning answered by `collision.md` §5; OPEN narrowed. §5.6: keys 3, 4, 6, 7 resolved (fog zone, water level, encounter zone, water type); only 0x10000012 OPEN. §6.4: light-map orientation and use answered. §7: kind 5 is the distant-landscape sector record and carries water pairs. §8: step 6 (water). §11 updated; this change log added. |
| 2026-10-05 | Revision after review (changes requested): the `collision.md` §15 rows for §3.5 and §3.6, not adopted by the sweep above, are now adopted. §1: legend adds BIN and Tessera Decision and covers 2026-10-05 facts; L6 limited to terrain; new L6a (bit 0 = ceiling diagonal), L6b (holes kept in collision, not cave mouths), L23 (entering a cell by containment); L22 marks the additive use BIN. §3.5: ceiling triangulation by bit 0 for every quad (replaces "from the stored triangles" and the implementation-census OPEN); holes described per `collision.md` §6.5 (not cave mouths); only the "ceiling below floor" rendering question stays OPEN, with a Tessera Decision. §3.6: bit 0 specified; bits 1–6 always set or clear together, OPEN. §3.8.3: evidence sentence limited to terrain chunks. §3.12 HM6: ceiling triangles follow bit 0. §3.13: rows updated. §4.4: entering a cell from outdoors answered (`collision.md` §5.8.1); OPEN narrowed to the re-evaluation frequency. §6.4: additive use BIN, content Probable. §11 items 1, 4 and 5 updated. |
| 2026-10-06 | Revision after re-review (changes requested). §1: new row L6c (vertex-flag value census, Confirmed 2026-10-06 over all 390,150 records). §3.6: the false sentence "126 occurs only together with a ceiling" replaced by the counts (365 records with low value 126, 357 with a ceiling, 8 without). §8 step 1: the loader keeps the whole vertex-flag byte (bit 7 for the terrain, bit 0 for the ceiling) instead of bit 7 only. |
| 2026-10-06 | Revision after third review (changes requested, §5.3 item 3). Landblock-info field and part names taken from a published loader replaced by Tessera's own (old names deliberately not repeated here): §4.1 field 3 is now `aux_word`, part 4 "block-pair records" (§4.2, retitled) and part 5 "cell groups" (§4.3, retitled), also in the `flags` bit labels; §4.2 fields 1–6 are now `number_a`, `info_a`, `info_b`, `link_type`, `number_b`, `pair_word`, with an Order column and a plain Meaning column; §4.3 entry fields `lead` and `members`; §4.6 fields 2–4 are now `link_id`, `end_id_1`, `end_id_2`, field 7 `has_pair` and field 8 "block pair", with a new OPEN on the u64 ids; §4.8 entry id is now `object_id`; §9 (`BadFlag`), §10 and §11 item 5 follow. No layout, count or rule changed. New row L24: `link_type` labels now cited from the install's `EntityLinkType` enum mappers 0x230000BE and 0x230001A9 (OBS, Confirmed 2026-10-06) instead of a published loader. L10 notes that the names are Tessera's own. Wrong cross-references fixed: L11 and §2.4 (twice) now point to §4.9 for the trailer rule; L12 points to §4.4 (cells), not §4.5. |
| 2026-10-07 | Interiors, visibility and ceiling normals (spec writer; M1 and M4 under decision 0011, tags BIN2 and OBS2; answers checklist row Y5 and the dark-ceiling rows T5, T15, T16 of `m4-5-visual.md`). §1: legend adds Inferred; new tags BIN2, OBS2; new rows L25–L37. §0: purpose lists the new sections. §3.13: normals row. New §3.14 (vertex normals of terrain and ceiling chunks; ceiling normals face down; what lights a cave ceiling; landscape drawn only outdoors; OPENs on light-map binding and renormalisation). §4.3: cell groups are visibility groups, `lead` is a key. §4.4: `cell_flags` meanings (bits 0, 2, 4, 5); **correction**: `neighbour_count` is 0 – 6, 24 or 54, not 0 – 10; neighbour entry re-read as u32, u16, i32 back-reference, u32-counted u16 list, u8 (same 15 bytes when the list is empty); lists A and B and `extra` explained; OPEN narrowed. §4.10: LI3 reworded, new LI5–LI8. New §4.11 (visibility cells and portals), §4.12 (inside versus outside; scene flags), §4.13 (indoor environment record, property lookup order, interior overrides, fog limits, no sun indoors; row Y5 worked through), §4.14 (what a cell draws; scale not used for placement). §8: step 7. §10: neighbour size. §11: items 5, 10, 11. For other specs (not edited here): `terrain-texturing.md` §4.8 OPEN (ceiling normals) is answered by §3.14; `environment.md` §7.1 and §12 implementer OPENs are answered by §4.13; `collision.md` §5.8 neighbour layout follows §4.4. |
| 2026-10-07 | Second 2026-10-07 revision (spec writer; incident 0018 follow-up: every client fact the released walker code relies on is now in the body; M1 and M4 under decision 0011, tags BIN3 and OBS3). Header: long review records moved to the dirty-room review log, short records kept; status Revised, ready for re-review. §1: tags BIN3, OBS3; rows L38–L45. **Review corrections:** §3.14, a sample averages four to eight triangles, not "up to six" (L45, census); §4.13, the main light is off when the intensity is **below** 0.0002 (strict), the colour test inclusive; §4.12, the outdoor branch only for region ≠ 0, landblock X and Y ≠ 0xFF and cell 0, any other position (cell 0xFFFF included) looked up as a cell, both flags clear when none is found (L40); §4.11 key paragraph, region 0 or landblock X or Y 0xFF give the outdoor key; §4.2 and §4.6, a block-pair DID need not name an existing record (0x8025474E). **New facts:** §3.15 (new) the block's open flag and chunks without layers (pointer to `terrain-texturing.md` §4.9, census L44, invariant HM8); §3.13 row; §4.4 cell numbers unique, `portal` selects the polygon, the two cells without an offset lie at the landblock origin with the identity orientation (answers the former OPEN; the advice to skip them is withdrawn); §4.10 LI9, LI10; §4.11 visibility stays in one landblock (L38), a portal is dropped when the neighbour has no visibility cell (L39), the walker's portal-walk rules and interior-entity attachment recorded as Tessera Decisions; §4.12 the visiting rule (bit 0 then bit 4, one level of PVS, 0xFFFF skipped), frills and the whole landscape pass switched by outdoor-visible (L41), no outdoor object drawn from inside; camera-cell Decision stated precisely; §4.13 the fog-end limit precedes the override; the implementer OPEN on the lookup landblock answered in part (owner landblock, Inferred); §4.14 Tessera Decisions for indoor point lights and cell-mesh detail; §4.15 (new) the walker's Tessera rules and seven implementer OPENs where the released code differs; §8 step 7 and §8.1 (world parts, stage-time diagnostics); §11 items 5, 10, 12. No layout changed. |
