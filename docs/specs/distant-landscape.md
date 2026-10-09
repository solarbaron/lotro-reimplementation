<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Distant-landscape sectors (landblock kind 5, DB type 0x41): far terrain, ceilings, water, sprites and far entities

| | |
|---|---|
| **Status** | Accepted (re-review of the 2026-10-08 revision passed, 2026-10-08) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec reviewer), not the author session |
| **Review record** | First review 2026-10-07 (separate session): **passed**, Accepted. Revision 2026-10-08: re-reviewed 2026-10-08 by a separate spec-reviewer session (not the author): **passed**, Accepted. Details in the dirty-room review log (decision 0018). |
| **Implements into** | `crates/formats` (sector record decoder, one fuzz target), `crates/world` (sector keys, loaded-sector set, per-landblock LOD and near-field flags, distant water positional read), `crates/terrain` or `crates/render` (horizon mesh: vertices, generated indices, LOD stitching, ceiling mesh), `crates/render` (distant sprites from imposter atlases), `apps/explorer` (region overview from sector samples), `apps/client` (horizon beyond the full-detail rings) |
| **Companion specs** | [`landblock.md`](landblock.md) §2.1 (DID shape, kind 5), §3.3–§3.6 (height grid, height scale, ceiling grid, vertex-flag grid), §3.14 (terrain vertex normals), §7 (kind 5 framing, superseded in detail by this spec). [`water.md`](water.md) §6.2 (landblock water rule), §8.1 (sector water quads), §9. [`records.md`](records.md) §1.2 (VLE), §2.1 (DB types), §5 (render surfaces). [`texture-list.md`](texture-list.md) §2 (texture lists), §6.3.1 (the 96 × 96 sector tiles). [`misc-records.md`](misc-records.md) §5 (imposter tables, prefix 0x27). [`entity-descriptor.md`](entity-descriptor.md) §4.1 (entity types), §4.2 (instance IDs), §9.3 (view distances). [`property-system.md`](property-system.md) §4.6 (Position), §4.11 and §5.2 (list-form property, arrays). [`material.md`](material.md) §8.0 (parameter ids 3000, 3001, 3075, 4005, 4006). [`terrain-texturing.md`](terrain-texturing.md) §12 (distance bands, sector overlay). [`coordinates.md`](coordinates.md) (frames and units). |

---

<a id="s0"></a>
## 0. Purpose

Every 3 × 3 group of landblocks whose corner landblock has X and Y divisible by 3 may have a
**distant-landscape sector** record, DID `0x805RXXYY`. It is the client's low-detail copy of
that group: terrain samples with normals, an optional ceiling surface for cave landblocks,
the group's landscape water, ground-hugging billboard sprites drawn from the imposter
atlases, and a list of far-visible entity placements. It is the largest block of data in the
landblock archives (33,159 records, 799 MB stored, 1,788 MB unpacked in this install).

This spec lets an implementer:

- decode every sector record of an install to the last byte (§3);
- know what each part means and how it relates to other records (§4);
- reproduce how the original client loads sectors around the viewer, picks a level of detail
  per landblock, builds the far terrain, ceiling and water meshes and places the sprites
  (§5, BIN where marked);
- build a horizon, a cheap region overview and per-sector height and water summaries (§6);
- check a decoder against testable invariants and synthetic examples (§7, §8).

All counts are observations of one install on one date; tests assert invariants, not counts.

---

<a id="s1"></a>
## 1. Provenance

Marks: **Confirmed** = checked on every record of the kind in this install on the date
given (for a fact with a BIN source: the data agrees on every record); **BIN** = rests on the
disassembly only; **Probable** = supported by a statistical test or by BIN and data together
but not proven for every case; **Inferred** = a conclusion drawn from Confirmed or BIN facts
that no probe or listing showed directly; **Reported** = published sources only; **Tessera
Decision** = Tessera's own rule where the original's is not known (never a statement about
the original).

| Tag | Source |
|---|---|
| OBS | Observed in the user's install (Steam) on 2026-10-07 by throwaway read-only probes in the dirty-room scratchpad (a stream reader built on the project's `tessera-dat` crate plus independent parsers written from the hypotheses of this spec). Read: **all 33,159** kind-5 records of `client_cell_{1,2,3,4,5,14}.dat` (every record decoded to the last byte with the layout of §3); all 390,150 landblock-data records (kind 0: height grids, ceiling grids, vertex-flag grids) and all 390,150 block maps (kind 1) of the same regions; all 507 imposter tables (prefix 0x27); the headers of every texture list (0x40) and render surface (0x41) a sector names (36,098 lists, 34,100 surfaces); the water tables 0x16000000, 0x16000004, 0x16000005, 0x1600000A through `tessera-dattool show`. |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5, re-checked 2026-10-07), read on 2026-10-07 with a headless Ghidra session on an own copy of the shared project (deleted afterwards) plus a byte scan of the executable for constants. **Questions:** (a) the grammar of the sector record's trailing section: the reader of DB type 0x41, its sub-object reader and its versioned lists; (b) how the client turns the 5-byte samples into vertices (positions, height scale, normal decoding, texture coordinates) and which index layout it draws; (c) which sectors and levels of detail the client chooses around the viewer, and how it stitches different levels; (d) what the mode byte, the ceiling mask, the 23-byte entries, the entity list and the six trailing DIDs are used for; (e) how distant water is built from the record. **Why M1–M3 were not enough:** the trailing section has variable-length parts whose sizes the data cannot reveal without the grammar (only 660-byte records are self-evident); the sample-to-vertex conversion, the index layout and the LOD and stitching rules are code with no trace in any record; no published source describes kind 5; no running client is available for M3. **Read:** the sector object's type registration, constructor, dependency collector and serialiser with its sub-object, chunk, water-list, sprite-entry and entity-entry readers; the far-terrain vertex and index builders and the LOD stitching table builder; the sector manager's update (sector list, per-landblock LOD grid, per-sector mesh build, sprite placement) and its option handler; the distant water chunk builder. Nothing about network, login, keys, launcher, patcher or anti-tamper (decision 0004). Output is prose facts only; listings stay in the dirty scratch area. |
| OBS2 | Observed in the same install on 2026-10-08 by a throwaway read-only probe in the dirty-room scratchpad: the stored normal bytes of **every** sample of all 33,159 terrain meshes (all nine blocks, interior, edge and corner samples) and all 941 ceiling meshes, compared with the rule of §4.3 computed from the kind-0 height, ceiling and vertex-flag grids of the sample's landblock and its eight neighbours (all 390,150 landblock-data records of regions 1–5 and 14). |
| BIN2 | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md)**, second session. Same client build (file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-08 with headless Ghidra on an own copy of the shared project (deleted afterwards). **Questions:** (f) what the client does with `ceiling_mask` bits above bit 8 (implementer OPEN of 2026-10-08); (g) how the sprite builder sets each sprite's fade parameter and mirror flag and what the sprite's last byte is; (h) what the client does with the entity list; (i) how the per-landblock near flag reaches the far mesh. **Read:** the sector reader's mode and mask branch, every reader of the stored mask, the sprite-entry reader, the sprite part of the sector manager's update with its constants and helper functions, the object draw-distance factor function, the camera field-of-view setter, the far-entity registration and its object constructor, the chunk mesh builder. **Why M1–M3 were not enough:** no record has mask bits above 8; the fade parameter, the mirror flag and the entity registration are code with no trace in the data; no running client is available for M3. Nothing about network, login, keys, launcher, patcher or anti-tamper. |
| SPEC | Other Tessera specs, cited by section (`landblock.md`, `water.md`, `texture-list.md`, `misc-records.md`, `entity-descriptor.md`, `property-system.md`, `material.md`, `terrain-texturing.md`, `terrain.md`, `mesh.md`, `walker.md`). |

No published third-party source describes kind-5 records (the research clones were searched;
one tool skips the DID range without decoding it).

| # | Fact | Sources | Mark |
|---|---|---|---|
| DL1 | DID shape `0x805RXXYY`, DB type 0x41, X and Y multiples of 3, one record per 3 × 3 group, sparse (§2) | `landblock.md` L16, L18; OBS | Confirmed |
| DL2 | Top-level layout: own DID, terrain mesh block, mode byte, optional ceiling mask and ceiling mesh block, `block_size` f32, water list, sprite list, entity list, six DIDs (§3.1) | BIN (a); OBS (33,159 of 33,159 records decode with zero bytes left) | Confirmed |
| DL3 | Mesh block: u32 sample count, samples × 5 bytes, u32 index count, indices × u16, nine 66-byte chunk entries, two bounds vectors (§3.2, §3.4) | BIN (a); OBS | Confirmed |
| DL4 | Sample = u16 height on the landblock height scale + three normal bytes; samples are nine 33 × 33 blocks, block *k* = 3·*x* + *y*, sample index inside a block 33·*j* + *i* (*i* along X) (§3.3, §4.1) | BIN (b); OBS (all 298,431 terrain blocks equal the kind-0 height grid of their landblock, sample for sample) | Confirmed |
| DL5 | Vertex position (*x*, *y*) = landblock offset + 5 × sample index; height converted exactly as `landblock.md` §3.4 (§4.2) | BIN (b) (constants 1/32, 160.0 from the record, 2,000.0 and the 32-bit 1/65,535) | BIN; heights Confirmed (DL4) |
| DL6 | Normal byte *b* decodes to *b* × 2/255 − 1; the three bytes are world X, world Z (up), world Y (§4.3) | BIN (b) (constant 0.0078431377 = 2/255 and 1.0); OBS (fit against terrain normals) | Confirmed (decode); order Confirmed |
| DL7 | Stored terrain normals are the normalised mean of `landblock.md` §3.14's triangle normals, encoded as ⌊(*c* + 1) × 127.5⌋ (§4.3) | OBS: 285,731,621 of 285,739,896 interior samples (all landblocks except the 1,095 with a ceiling collision tree) match exactly, every sample within one step; OBS2: the same for every sample of every terrain block, edges and corners included (§4.3) | Confirmed (within one step), 2026-10-08 |
| DL23 | At block edges and corners the stored normals use the neighbouring landblocks' samples and diagonals (any landblock of the region, not only those of the sector), the quads east and north of the block with the neighbour's own flag byte; ceiling normals follow the mirrored ceiling rule of `landblock.md` §3.14, normalised (§4.3) | OBS2 | Confirmed (within one step), 2026-10-08 |
| DL24 | `ceiling_mask` bits 9–31 are stored but never read: the client queries only bits 0–8, and the ceiling sample count is the mesh block's own count (§3.5) | BIN2 (f) | BIN |
| DL25 | Sprite fade parameter: the stored `distance` for sprites within the 3 × 3 landblocks around the viewer's landblock, otherwise a value computed from the sprite size, the screen and the object draw-distance factor (§5.6) | BIN2 (g); its use in the sprite program: `misc-records.md` §18 (BIN there) | BIN |
| DL26 | The sprite's last byte is a heading *b* × 360/256 degrees; the client mirrors the sprite when that heading points away from a fixed point near the viewer (§4.8, §5.6) | BIN2 (g); mirror rendering `misc-records.md` §5.4 | BIN |
| DL27 | Sprites are built only while the distant-imposter graphics option is on | BIN2 (g) (option identified by its storage next to the two draw-distance options) | BIN (rule); option identity Probable |
| DL28 | The far entities become placement objects keyed by instance ID; an instance ID already known is skipped (§4.9) | BIN2 (h) | BIN |
| DL29 | Near radius *N* = 1 landblock; sector radius *F* = ⌊*R* / 3⌋ + 1 from the landscape draw-distance option; near landblocks go to a second, flagged batch of the same far mesh (§5.1, §5.2) | `terrain.md` TR20, TR21 (BIN-T there); BIN2 (i) | BIN |
| DL8 | Chunk entry *c* = *x* + 3·*y* describes landblock (X + *x*, Y + *y*): vertex start, vertex count, bounds, water flag and water index, four LOD index ranges (§3.4, §4.5) | BIN (a), (d), (e); OBS (all chunk fields follow the rules of §7 in every record) | Confirmed |
| DL9 | Mode byte 0 = no ceiling, 1 = ceiling in all nine landblocks, 2 = a u32 mask follows; mask bit *x* + 3·*y* = landblock (X + *x*, Y + *y*) has a ceiling (§3.5) | BIN (a), (d); OBS (mask equals the landblocks' `has_ceiling`, 33,159 of 33,159; mode 1 exactly when all nine) | Confirmed |
| DL10 | Ceiling mesh: same block layout; samples only for ceiling landblocks, packed in vertex-block order; heights equal the ceiling grid; absent chunks all zero (§4.4) | BIN (a), (d); OBS (6,780 ceiling blocks equal their ceiling grid exactly; 1,689 absent chunks all zero) | Confirmed |
| DL11 | `block_size` = 160.0 in every record; the vertex builder takes landblock size and sample spacing (`block_size` / 32) from it (§3.1, §4.2) | BIN (b); OBS (33,159 of 33,159) | Confirmed |
| DL12 | Water list: VLE count, then (u8 water type, f32 height) pairs; chunk water flag and index name the pair; the flags and pairs equal `water.md` §6.2; list in order of first use in vertex-block order (§3.6, §4.6) | BIN (a), (e); OBS (all 298,431 chunk flags and all 89,913 wet pairs agree; order holds in all 1,188 records with two or more pairs) | Confirmed |
| DL13 | Sprite entry (23 bytes): u16 imposter name key, (*x*, height, *y*) sector-local position, f32 scale, f32 distance, u8 (§3.7) | BIN (a), (d); OBS (3,549,236 entries in 8,159 records) | Confirmed (layout) |
| DL14 | The client looks a sprite's key up in the sector's own imposter table and skips sprites whose key is absent; size comes from the imposter entry times the sprite's scale, with the roles of `misc-records.md` §5.2 (I5) (§4.8, §5.6) | BIN (d); `misc-records.md` I5 (BIN there); OBS (3,495,690 of 3,549,236 keys present in the sector's table) | BIN (rule); census Confirmed |
| DL15 | Sprite positions lie on the terrain: second component = absolute height (§4.8) | OBS (region 1 sample of 29,577 sprites: median 0.24, 99th percentile 2.96 units from the nearest terrain sample; swapped axes give 26 / 452) | Probable |
| DL16 | Entity entry: u32 entity type, u32 object DID, u64 instance ID, three f32 scale, Position, one list-form property, f32 squared view distance (§3.8) | BIN (a); OBS (304,450 entries in 4,306 records) | Confirmed |
| DL17 | Entity entries are placements of the sector's own landblocks (instance ID and position name a landblock of the sector, outdoors) with view distance ≥ 250 (§4.9) | OBS (304,450 of 304,450) | Confirmed |
| DL18 | Six trailing DIDs: terrain colour texture list, ceiling colour texture list, terrain tile surface, ceiling tile surface, distant-water flow map, imposter table (§3.9, §4.7) | BIN (a), (d), (e); OBS (types and presence rules of §3.9 in every record) | Confirmed (layout and types); roles BIN, partly OPEN |
| DL19 | Format version: every record of this install is read by the client's newest grammar (version 10 or later); older versions differ as in §3.10 | BIN (a); OBS (every record decodes with the newest grammar) | BIN (older grammars); Confirmed (this install) |
| DL20 | Index layout the client generates: per chunk and LOD, quads of 2^*l* samples, diagonal south-west to north-east, terrain counter-clockwise from above, ceiling reversed; stored LOD ranges match it (§5.3) | BIN (b); OBS (stored ranges in 33,159 terrain and 941 ceiling meshes; stored index count 0 everywhere) | BIN (indices); ranges Confirmed |
| DL21 | Sector loading, per-landblock LOD by squared landblock distance (≤ 3, ≤ 24, ≤ 99, beyond), near-field flag, edge stitching (§5.1–§5.3) | BIN (c) | BIN |
| DL22 | Distant water: one mesh per water-list pair, one quad per landblock that names it, at the pair's height; the water type's surface with the record's flow map as parameter 3075 (§5.5) | BIN (e); `water.md` §8.1 | BIN |

---

<a id="s2"></a>
## 2. Records and where they live

<a id="s2-1"></a>
### 2.1 DID and archive (Confirmed, DL1)

DID = `0x80500000` + *R* · 2¹⁶ + *X* · 2⁸ + *Y* (`landblock.md` §2.1, kind *K* = 5), with
*R* the region, and *X*, *Y* the landblock coordinates of the group's **south-west corner**
landblock, both multiples of 3 (0, 3, …, 252). The record covers the nine landblocks
(*X* + *x*, *Y* + *y*), 0 ≤ *x*, *y* ≤ 2. A **sector key** (*sx*, *sy*) = (*X* / 3, *Y* / 3)
ranges over 0–84. The record lives in `client_cell_R.dat` with the other landblock records
of region *R*. The client registers the DID range for DB object type 0x41
(DISTANT_LANDSCAPE_SECTOR, `landblock.md` L18).

Directory stamps are 38 (24,434 records) and 39 (8,725); the layout does not depend on the
stamp in this install (DL19).

<a id="s2-2"></a>
### 2.2 Which sectors exist (Confirmed)

| Region | Possible (85 × 85) | Present | With ceiling | With water | With sprites | With entities |
|---|---|---|---|---|---|---|
| 1 | 7,225 | 6,141 | 142 | 1,027 | 1,590 | 1,047 |
| 2 | 7,225 | 5,913 | 541 | 966 | 1,556 | 942 |
| 3 | 7,225 | 6,673 | 20 | 3,931 | 1,397 | 672 |
| 4 | 7,225 | 6,823 | 139 | 3,542 | 1,845 | 844 |
| 5 | 7,225 | 7,068 | 40 | 3,379 | 1,610 | 676 |
| 14 | 7,225 | 541 | 59 | 80 | 161 | 125 |
| **Total** | 43,350 | **33,159** | **941** | **12,925** | **8,159** | **4,306** |

(Ceiling: records with `mode` ≠ 0. Water: records with at least one water pair.)

Rules:

- **DL-E1:** a missing sector means "no distant data for this group". It is not an error.
  Presence does not follow from the terrain alone: 6,325 sectors over nine all-zero height
  grids exist and 1,208 groups with non-flat terrain have no sector. A reader asks for the record and
  handles its absence.
- **DL-E2:** 87 landblocks with a ceiling grid lie in groups without a sector record; they get
  no distant ceiling.

<a id="s2-3"></a>
### 2.3 References to and from other records (Confirmed)

| From | To | Rule |
|---|---|---|
| Sector | Landblock data (kind 0) of its nine landblocks | Not named; same (*R*, *X* + *x*, *Y* + *y*). The sector's samples copy their height and ceiling grids (§4.2, §4.4). |
| Sector | Block maps (kind 1) and the region's water table (`water.md` §3) | Not named; the water list is the result of `water.md` §6.2 for the nine landblocks (§4.6). |
| Sector | Texture lists (0x40), render surfaces (0x41) | Named in the DID tail (§3.9); every named DID exists. |
| Sector | Imposter table (0x27) | Named in the DID tail: always the table of the 27 × 27 square that contains the sector (`misc-records.md` §5.1), 33,159 of 33,159. No other record type is known to name imposter tables (`misc-records.md` §5.1). |
| Sector | Entity descriptors (0x47), game objects (0x70) | Named by the entity entries (§3.8). |
| Imposter table | Sector | No back reference. |

---

<a id="s3"></a>
## 3. Record layout (Confirmed, 33,159 of 33,159, zero bytes left)

All integers and floats little-endian, packed, no alignment. VLE is `records.md` §1.2. "Mesh
block" is §3.2.

<a id="s3-1"></a>
### 3.1 Top-level order

| Order | Field | Type | Present | Meaning / observed |
|---|---|---|---|---|
| 1 | `did` | DID | always | Own DID |
| 2 | `terrain` | mesh block (§3.2) | always | Terrain samples of all nine landblocks; sample count 9,801 in every record |
| 3 | `mode` | u8 | always | 0 = no ceiling mesh; 1 = ceiling mesh covering all nine landblocks; 2 = ceiling mesh covering the landblocks of `ceiling_mask` (§3.5). Observed 0 (32,218), 1 (608), 2 (333). |
| 4 | `ceiling_mask` | u32 | `mode` = 2 | Bit *x* + 3·*y* set = landblock (*X* + *x*, *Y* + *y*) has a ceiling. Observed with 1–8 bits set. |
| 5 | `ceiling` | mesh block (§3.2) | `mode` ≠ 0 | Ceiling samples (§4.4) |
| 6 | `block_size` | f32 | always | Landblock edge length in units; 160.0 in every record (DL11) |
| 7 | water count *w* | VLE | always | 0–5 observed |
| 8 | `water` | *w* × (u8 `type`, f32 `height`) | always | Distinct (water type, water height) pairs (§3.6, §4.6) |
| 9 | sprite count *s* | u32 | always | 0–5,194 observed |
| 10 | `sprites` | *s* × 23-byte entry (§3.7) | always | Distant billboard sprites (§4.8) |
| 11 | entity count *e* | u32 | always | 0–2,074 observed |
| 12 | `entities` | *e* × entity entry (§3.8) | always | Far-visible entity placements (§4.9) |
| 13 | `colour_list` | DID | always | §3.9 |
| 14 | `ceiling_colour_list` | DID | always | §3.9 |
| 15 | `tile` | DID | always | §3.9 |
| 16 | `ceiling_tile` | DID | always | §3.9 |
| 17 | `flow_map` | DID | always | §3.9 |
| 18 | `imposter_table` | DID | always | §3.9 |
| — | *(end)* | | | No bytes remain in any record |

Record size = 8 + 49,005 + the trailing section; trailing sections (from field 2's index
count to the end) are 660–224,444 bytes. The smallest record (no ceiling, water, sprites or
entities) is 49,673 bytes (§8.1).

<a id="s3-2"></a>
### 3.2 Mesh block

| Order | Field | Type | Meaning / observed |
|---|---|---|---|
| 1 | sample count *n* | u32 | Terrain: 9,801 in every record. Ceiling: 1,089 × the number of ceiling landblocks. |
| 2 | `samples` | *n* × 5 bytes (§3.3) | Sample array |
| 3 | index count *m* | u32 | 0 in every mesh block of this install (33,159 terrain, 941 ceiling) |
| 4 | `indices` | *m* × u16 | Stored index list. The client keeps it only under a stream mode that was not traced and otherwise skips the bytes; with no stored indices it generates its own (§5.3, BIN). Empty in this install. **Tessera rule:** skip the *m* × 2 bytes and always use the generated list of §5.3 (the client's ordinary loading path). |
| 5 | `chunks` | 9 × chunk entry (66 bytes, §3.4) | Entry *c* describes landblock (*X* + (*c* mod 3), *Y* + ⌊*c* / 3⌋) |
| 6 | `bounds_min` | 3 × f32 | (*x*, *y*, *z*) minimum over the chunks present (§4.5) |
| 7 | `bounds_max` | 3 × f32 | (*x*, *y*, *z*) maximum |

<a id="s3-3"></a>
### 3.3 Sample (5 bytes)

| Offset | Size | Type | Meaning | Units |
|---|---|---|---|---|
| 0 | 2 | u16 | Height on the landblock height scale (`landblock.md` §3.4) | stored units (65,535 = 2,000 world units) |
| 2 | 1 | u8 | Normal, world X component, biased (§4.3) | *b* × 2/255 − 1 |
| 3 | 1 | u8 | Normal, world Z (up) component, biased | same |
| 4 | 1 | u8 | Normal, world Y component, biased | same |

Order of samples: §4.1.

<a id="s3-4"></a>
### 3.4 Chunk entry (66 bytes)

| Offset | Size | Type | Field | Meaning (§4.5) |
|---|---|---|---|---|
| 0 | 4 | u32 | `vertex_start` | Index of the chunk's first sample in the mesh block's sample array |
| 4 | 4 | u32 | `vertex_count` | 1,089, or 0 for a ceiling chunk of a landblock without ceiling |
| 8 | 12 | 3 × f32 | `min` | (*x*, *y*, *z*) of the chunk's south-west bottom corner, sector-local units |
| 20 | 12 | 3 × f32 | `max` | (*x*, *y*, *z*) of the north-east top corner |
| 32 | 1 | u8 | `wet` | 0 or 1: the landblock has landscape water. The client fails the record for a value above 1 (BIN). Always 0 in a ceiling mesh. |
| 33 | 1 | u8 | `water_index` | Index into the water list when `wet` = 1; 0 when `wet` = 0 (all 208,518 dry terrain chunks) |
| 34 | 32 | 4 × (u32, u32) | `lods` | For LOD *l* = 0…3: (first index, triangle count) into the mesh's index list (§5.3) |

<a id="s3-5"></a>
### 3.5 Mode and ceiling mask (Confirmed, DL9)

- `mode` 0: no `ceiling_mask` and no ceiling mesh block follow; the mask is 0.
- `mode` 1: no `ceiling_mask` follows; the mask is 0x1FF (all nine landblocks); a ceiling
  mesh block follows.
- `mode` 2: a u32 `ceiling_mask` follows, then a ceiling mesh block.
- Observed: `mode` 1 exactly when all nine landblocks have a ceiling; `mode` 2 exactly when
  1–8 of them do (28, 36, 130, 19, 9, 90, 11, 10 records for 1–8 bits); `mode` 0 exactly when
  none does. Bit *x* + 3·*y* is set exactly when landblock (*X* + *x*, *Y* + *y*) has
  `has_ceiling` = 1 in its landblock data (`landblock.md` §3.5), in all 33,159 records.
- The client reads no mask for a mode above 2 and still reads a ceiling mesh block (BIN).
  No such mode occurs; **Tessera rule:** reject the record as malformed.
- **Mask bits 9–31 (BIN2 (f), DL24).** The client stores the u32 of `mode` 2 unchanged and
  never reads those bits: every reader of the mask asks for bit *x* + 3·*y* with *x* and *y*
  each 0–2, so only bits 0–8 matter. The mask does not size the ceiling mesh: the ceiling
  mesh block carries its own sample count (§3.2), and each chunk entry its own
  `vertex_start` and `vertex_count`. A record with high bits set therefore loads in the
  client and behaves as if they were clear. No record of this install has such a bit
  (DL-I7). Tessera's rule is §3.11.

<a id="s3-6"></a>
### 3.6 Water list

A VLE count *w*, then *w* pairs:

| Order | Size | Type | Meaning |
|---|---|---|---|
| 1 | 1 | u8 | Water type: the block-map type value of `water.md` §6.1 (largest observed 47) |
| 2 | 4 | f32 | Water height *h* in world units, bit-identical to `water.md` §6.2 step 6 |

The client fails the record when *w* exceeds the remaining bytes (BIN). Pair order and use:
§4.6. This answers the positional-read question of `landblock.md` §7 (§11).

<a id="s3-7"></a>
### 3.7 Sprite entry (23 bytes)

| Offset | Size | Type | Field | Meaning (§4.8) | Observed |
|---|---|---|---|---|---|
| 0 | 2 | u16 | `name_key` | Imposter name key, looked up in the sector's imposter table (`misc-records.md` §5.2 `name_key`) | 818 distinct values, 1–983 |
| 2 | 4 | f32 | `east` | Sector-local *x* (east), units | 0.03–479.996 |
| 6 | 4 | f32 | `height` | Absolute height, units | −3.3 to 1,765.2 |
| 10 | 4 | f32 | `north` | Sector-local *y* (north), units | 0.0–479.999 |
| 14 | 4 | f32 | `scale` | Size factor | 0.1–10.0 (median 0.96) |
| 18 | 4 | f32 | `distance` | Fade distance, units (§4.8, §5.6) | 45, 80, 140, 180, 200, 300 or 1,000 |
| 22 | 1 | u8 | `byte` | Heading, `byte` × 360/256 degrees (§4.8, §5.6) | 0–255, roughly uniform (0 in 72,738, others about 13,800 each) |

The sprite count is a plain u32; the client allocates *s* × 23 bytes without first comparing
*s* with the remaining bytes (BIN). **Tessera rule:** reject *s* > remaining / 23.

<a id="s3-8"></a>
### 3.8 Entity entry (variable length, at least 37 bytes)

| Order | Size | Type | Field | Meaning (§4.9) |
|---|---|---|---|---|
| 1 | 4 | u32 | `type` | Entity type, the enumeration of `entity-descriptor.md` §4.1. Observed 7 Physics (302,878), 6 Weenie (1,335), 9 Light (233), 23 PatrolPoint (4). |
| 2 | 4 | DID | `object` | A 0x47 entity descriptor (303,115) or, for the Weenie entries, a 0x70 object (1,335) |
| 3 | 8 | u64 | `iid` | Instance ID, layout of `entity-descriptor.md` §4.2 (kind byte 1 in 302,533 entries, 4 in 1,917) |
| 4 | 12 | 3 × f32 | `scale` | Scale per axis; the three values are equal in every entry (0.075–34.0) |
| 5 | var | Position | `position` | Stored Position of `property-system.md` §4.6 (flag byte, then the fields its payload bits select). Observed flag bytes 0x8B, 0x9B, 0xBB, 0xDB, 0xFB; cell 0 (outdoors) in every entry. |
| 6 | ≥ 4 | property, list form | `property` | One property in the list form of `property-system.md` §5.2: a u32 PID, then its value, or a PID of 0 alone (null). Observed: null in 285,696 entries; PID 0x4CE `Entity_ContentLayers` (an array of `Entity_ContentLayer` values, `entity-descriptor.md` §4.5.1) in 18,754 entries of 474 records. |
| 7 | 4 | f32 | `view_distance_sq` | The square of a view distance in units: 62,500 – 64,000,000 (distances 250–8,000) |

The entity count is a plain u32 that the client does not compare with the remaining bytes
(BIN). **Tessera rule:** reject *e* > remaining / 37 (37 = the smallest entry: Position flag
byte 0 and a null property). Decoding the property needs the property dictionary
(`property-system.md` §5.3); a decoder that meets an unknown PID fails the record, as the
client's reader does.

<a id="s3-9"></a>
### 3.9 DID tail (24 bytes; Confirmed types, roles §4.7)

| Order | Field | Type | Observed |
|---|---|---|---|
| 1 | `colour_list` | DID | A texture list (0x40) in all 33,159: kind 2, 7 levels, level 0 a 256 × 256 DXT1 surface; each named by exactly one sector |
| 2 | `ceiling_colour_list` | DID | 0 when `mode` = 0; otherwise a texture list of the same shape (941), each named once |
| 3 | `tile` | DID | A render surface (0x41), 96 × 96 DXT3, category 0, in all 33,159; each named once (`texture-list.md` T24) |
| 4 | `ceiling_tile` | DID | 0 when `mode` = 0; otherwise a 96 × 96 DXT3 surface (941; `texture-list.md` T25) |
| 5 | `flow_map` | DID | 0 (31,161) or a texture list (1,998) of one level, a 256 × 256 A8R8G8B8 surface; each named once |
| 6 | `imposter_table` | DID | The imposter table (0x27) of the 27 × 27 square holding the sector (33,159 of 33,159); its last two bytes are therefore the region number and 0x27, which is what `texture-list.md` §6.3.1 observed at the record's end |

`ceiling_colour_list` and `ceiling_tile` are non-zero exactly when `mode` ≠ 0.

<a id="s3-10"></a>
### 3.10 Older format versions (BIN only; none in this install)

The client asks its stream for a format version of this record type and branches on it
(DL19). Every record of this install decodes with the newest grammar above; a version
number itself is not stored in the record (how the client obtains it is OPEN, §10). The
differences, for completeness:

| Version | Difference from §3.1–§3.8 |
|---|---|
| below 2 | Nothing is read; the record counts as unreadable (§5.8). |
| 2–6 | A different top level: own DID; the terrain mesh block (same layout as §3.2); no mode byte and no ceiling; then `block_size`, the water list, (from version 3) the sprite count and list, then `colour_list` and (from version 5) `tile`; nothing else. |
| below 6 | Sprite entry: u8 key (instead of u16), position, u8 `byte`, f32 `scale`, three ignored bytes; no `distance`. |
| 6–7 | Sprite entry: u16 key, position, u8 `byte`, f32 `scale`, three ignored bytes; no `distance`. |
| 7–9 | No entity count and no entity list. |
| 7 | No `imposter_table`. |
| 7–8 | No `flow_map`. |

**Tessera rule:** decode the newest grammar only and report anything else as malformed
input. A record that leaves bytes over or runs short under the newest grammar is malformed.

<a id="s3-11"></a>
### 3.11 Decoder rules for hostile input

- Every count is bounded by the remaining bytes before anything is allocated: sample count
  ≤ remaining / 5, index count ≤ remaining / 2, water count ≤ remaining / 5, sprite count ≤
  remaining / 23, entity count ≤ remaining / 37.
- A chunk entry is usable only when `vertex_count` is 0 or 1,089 and `vertex_start` +
  `vertex_count` ≤ the mesh's sample count; the client's vertex builder does not check this
  (BIN). Tessera rejects the record otherwise.
- `wet` above 1 fails the record (as in the client). `water_index` must be below *w* when
  `wet` = 1; the client does not check it (BIN); Tessera rejects the record.
- `mode` above 2: §3.5.
- The terrain sample count must be 9,801 for the grid meaning of §4.1; Tessera rejects any
  other value (the client's grammar would accept it).
- A ceiling mesh's sample count must equal 1,089 × the number of mask bits (DL10).
- **`ceiling_mask` bits 9–31 (Tessera Decision, confirmed 2026-10-08).** A `mode` 2 mask with
  any of bits 9–31 set is rejected as malformed. The client ignores those bits instead
  (§3.5, BIN2); Tessera keeps the stricter rule, like the other rules of this list that are
  stricter than the client, because no record of this install has such a bit and DL-I7 then
  fails. With bits 9–31 rejected, "the number of mask bits" above is the number of set bits
  among bits 0–8.
- **Encoder (Tessera Decision, confirmed 2026-10-08).** The format needs no encoder. An
  encoder that Tessera uses for synthetic fixtures and fuzzing may write every entity entry
  with a null property (a u32 PID of 0, §3.8); such records are valid and decode to the same
  entries. The client offers no rule here: it writes the property it holds.

Answered 2026-10-08 (implementer OPEN of 2026-10-08 on mask bits above bit 8 and on the
encoder's null properties): the two rules above; the client's behaviour is §3.5.

> **OPEN:** (implementer note, `crates/formats/src/landblock/sector.rs`, 2026-10-09;
> engineering choices, for review) (1) Under the Tessera rule of §3.2 the decoder keeps only
> the stored index count *m*, not the index bytes; the fixture encoder writes *m* zero u16
> values back, so re-encoding is byte-exact only for records with *m* = 0 (all records of
> this install). (2) The checker of the §4.3 normal rule (DL-I14) computes in 64-bit floats;
> the census finds the counts of §4.3 exactly (every sample within one step). Are these
> acceptable?

---

<a id="s4"></a>
## 4. Meaning of the parts

<a id="s4-1"></a>
### 4.1 Sample order and the sector frame (Confirmed, DL4)

**Sector frame:** origin at the south-west corner of landblock (*X*, *Y*), *x* east, *y*
north, *z* absolute height, units as `coordinates.md`. Landblock (*X* + *x*, *Y* + *y*)
covers 160·*x* ≤ sector *x* ≤ 160·(*x* + 1) and the same in *y*.

The terrain sample array holds **nine blocks of 1,089 samples**:

- Block *k* (samples 1,089·*k* … 1,089·*k* + 1,088) belongs to landblock (*X* + *x*,
  *Y* + *y*) with ***k* = 3·*x* + *y*** ("vertex-block order": X outer, Y inner).
- Inside a block, sample (*i*, *j*), 0 ≤ *i*, *j* ≤ 32, *i* along X and *j* along Y, is at
  offset **33·*j* + *i*** ("Y-major", the transpose of the kind-0 height array of
  `landblock.md` §3.3, which is 33·*i* + *j*).
- Sample (*i*, *j*) of block (*x*, *y*) sits at sector-local (160·*x* + 5·*i*,
  160·*y* + 5·*j*).

Edge samples are duplicated between neighbouring blocks (a block holds its own 33 × 33
grid with both edges), exactly as the kind-0 grids do; so 99 × 99 = 9,801 samples cover a
97 × 97 point lattice with shared rows and columns repeated.

Chunk entries use a **different order**: chunk *c* = *x* + 3·*y* (X inner). Chunk *c*'s
`vertex_start` is 1,089 × (3·*x* + *y*) in the terrain mesh (§4.5).

<a id="s4-2"></a>
### 4.2 Heights (Confirmed, DL4, DL5)

The u16 of every terrain sample equals the stored height of the same point in the kind-0
height grid of its landblock, in all 298,431 blocks of all 33,159 records. Convert with
`landblock.md` §3.4: convert the u16 to a 32-bit float, multiply by 2,000.0, multiply by the
32-bit float nearest 1/65,535, rounding each product to 32 bits (the client's far-terrain
vertex builder uses exactly these constants, BIN). The "median difference 15 units" noted
earlier in `landblock.md` §7 came from assuming a uniform 99 × 99 lattice; with the block
order of §4.1 there is no difference.

Consequences: a reader that has the kind-0 records needs the sector heights only as a
compact copy; a reader that has only the sectors (for example a map tool) gets the full
terrain heights from them.

<a id="s4-3"></a>
### 4.3 Normals (decode Confirmed, DL6; rule Confirmed within one step, DL7, DL23)

Each of the three normal bytes decodes to *b* × 2/255 − 1 (the client multiplies by the
32-bit float 0.0078431377 and subtracts 1.0, BIN). Byte 2 is the world X component, byte 3
the world Z (up) component and byte 4 the world Y component. A flat terrain sample stores
(127, 255, 127), which decodes to (−0.0039, 1.0, −0.0039) in (X, Z, Y) order; the client
does not renormalise.

The client does not recompute the normals; a reader decodes the stored bytes. The rule
below says how they were made, for a tool that writes or checks sector records (OBS, OBS2;
DL7, DL23; revised 2026-10-08, it covered interior terrain samples only before):

1. Compute the vertex normal of `landblock.md` §3.14 for the sample: for the terrain mesh
   with *s* = +1 (terrain heights, bit-7 diagonals), for the ceiling mesh with *s* = −1
   (the stored ceiling grid including its zero samples, bit-0 diagonals, X offsets mirrored,
   normals facing down).
2. Across a block edge, corners at index −1 or 33 come from the neighbouring landblock in
   that direction (the diagonal neighbour at a corner), at its sample 31 or 1. The neighbour
   is any landblock-data record of the region, whether or not it belongs to the same sector.
3. A quad at index −1 in an axis reads its diagonal bit from the neighbour's flag byte at
   index 31 in that axis, and a quad at index **32** from the neighbour's flag byte at index
   0 (the quad east or north of the block belongs to the neighbour). This differs from
   `landblock.md` §3.14 step 4, which reads the block's own byte at index 32 for the
   client's run-time normals; for the stored sector normals only the neighbour's byte
   matches at every corner sample.
4. A triangle with a corner in a landblock that has no landblock-data record (beyond the
   region border, `terrain.md` TR18), or, for the ceiling, in a landblock without a ceiling
   grid, is left out of the mean. A quad's bit counts as set only when its landblock has no
   record; a neighbour without a ceiling grid still supplies its bit-0 flags (with mirrored
   X offsets, a quad outside the block can contribute a triangle whose corners all lie
   inside it).
5. Normalise the mean to unit length and encode each component *c* as ⌊(*c* + 1) × 127.5⌋,
   in the byte order X, Z (up), Y of §3.3.

Observed (OBS2, every sample of every record): the encoded rule equals the stored bytes in
324,981,787 of 324,991,359 terrain samples (all 33,159 records × 9,801) and 7,383,411 of 7,383,420 ceiling samples (all 6,780 ceiling blocks), and every
other sample differs by one step in one or more components (rounding at a step boundary,
Inferred). In every ceiling sample the middle (up) byte is at most 126: ceiling normals
never point up.

<a id="s4-4"></a>
### 4.4 Ceiling mesh (Confirmed, DL10)

When `mode` ≠ 0 the ceiling mesh block describes the inverted-terrain ceilings of the
landblocks in the mask (`landblock.md` §3.5):

- its sample count is 1,089 × (number of mask bits);
- the ceiling landblocks' blocks are **packed** in vertex-block order: the *r*-th ceiling
  landblock in the order *k* = 3·*x* + *y* has `vertex_start` 1,089·*r*;
- each block's u16 values equal that landblock's ceiling grid, sample for sample, in the
  order of §4.1 (6,780 of 6,780 blocks), including the value 0 where the ceiling grid has
  "no ceiling here";
- the chunk entries of landblocks outside the mask are entirely zero (all 66 bytes; 1,689 of
  1,689);
- the LOD ranges of present chunks are packed by the same rank *r* (§4.5);
- `bounds_min` and `bounds_max` span the present chunks only.

<a id="s4-5"></a>
### 4.5 Chunk bounds and LOD ranges (Confirmed, DL8)

For a present chunk of landblock (*x*, *y*):

- `min` = (160·*x*, 160·*y*, lowest converted height of the block's samples), `max` =
  (160·(*x* + 1), 160·(*y* + 1), highest converted height), the heights converted as §4.2
  (bit-identical in every chunk of both meshes);
- `lods` = (8,160·*r* + 0, 2,048), (8,160·*r* + 6,144, 512), (8,160·*r* + 7,680, 128),
  (8,160·*r* + 8,064, 32), where *r* = 3·*x* + *y* for the terrain mesh and the packed rank
  of §4.4 for the ceiling mesh. 2,048, 512, 128 and 32 triangles are (32 / 2^*l*)² × 2;
  8,160 = 3 × (2,048 + 512 + 128 + 32) is one chunk's share of the generated index list
  (§5.3).

The mesh block's `bounds_min` and `bounds_max` are the union of its present chunks' bounds
(33,159 terrain and 941 ceiling meshes).

<a id="s4-6"></a>
### 4.6 Water per landblock (Confirmed, DL12)

For terrain chunk *c* of landblock *L*:

- `wet` = 1 exactly when *L* has water by `water.md` §6.2 (all 298,431 chunks);
- then `water_index` names the pair (*t*, *h*) of the water list with *t* = *L*'s water type
  (low 8 bits of the block-map value, `water.md` §9 OPEN) and *h* = its water height, bit
  for bit (89,913 of 89,913 wet landblocks);
- `wet` = 0 means no landscape water, and then `water_index` is 0.

The water list holds each distinct pair once, and every pair is used by at least one chunk.
Its order is the order of **first use** when the nine landblocks are visited in vertex-block
order (*k* = 3·*x* + *y*), Confirmed in all 1,188 records with two or more pairs (chunk
order *c* = *x* + 3·*y* fails in 226 of them).

**Reading distant water positionally** (answers `landblock.md` §7 OPEN of 2026-10-06): walk
the record with §3.1; the water list is field 8; per landblock use chunk *c* = *x* + 3·*y*
of the terrain mesh block. No byte search is needed.

<a id="s4-7"></a>
### 4.7 Textures (types Confirmed; use BIN, partly OPEN)

- `colour_list` / `ceiling_colour_list`: the colour texture of the terrain / ceiling mesh
  (BIN: the client binds it as the sector material's parameter 3000, `material.md` §8.0, and
  accepts either a render surface or a texture list there). It spans the whole sector with
  the texture coordinates of §5.4. Level 0 is 256 × 256 DXT1: about 1.9 units per texel.
- `tile` / `ceiling_tile`: 96 × 96 DXT3 surfaces (32 texels per landblock). The client loads
  them as dependencies of the sector (BIN), but no reader of them was found in the far-terrain,
  water or sprite code (OPEN, §10).
- `flow_map`: a single-level 256 × 256 A8R8G8B8 texture list that the distant water
  material receives as parameter 3075 (`material.md`: flow map; §5.5, BIN).
- `imposter_table`: the sprites' atlas and size source (§4.8, §5.6).

<a id="s4-8"></a>
### 4.8 Sprites (layout Confirmed; use BIN)

Each sprite entry places one billboard drawn from the imposter atlases of the sector's
imposter table:

- `name_key` is looked up in the sector's own `imposter_table` (BIN; DL14). In this install
  3,495,690 of 3,549,236 entries find their key there; 53,464 name a key that only other
  squares' tables hold and 82 name one of two keys (296, 596) that no table holds. The client
  **skips** a sprite whose key is not in the table (BIN).
- Position: sector frame (*x* = `east`, *y* = `north`), *z* = `height` absolute. Sprites
  stand on the terrain (DL15). All 3,549,236 lie inside the sector square.
- Size: the client multiplies the imposter entry's two stored floats (`misc-records.md` §5.2
  `value_a`, `value_b`) by `scale` (BIN). *S* = `value_a` × `scale` is the width and height
  of the billboard quad and *B* = `value_b` × `scale` the height of its lower edge above the
  sprite position (`misc-records.md` I5, BIN there; corner table in its §5.2). Revised
  2026-10-08: this was Inferred here before.
- `distance`: the sprite's **fade distance**. It becomes the sprite's fade parameter when the
  sprite lies near the viewer, and is replaced by a computed value otherwise (§5.6, BIN2;
  DL25). The sprite program fades the sprite in, beyond a start distance derived from that
  parameter (`misc-records.md` §18, BIN there).
- `byte`: the sprite's **heading** *θ* = `byte` × 360 / 256 degrees (0 ≤ *θ* < 360; the
  client computes it as (`byte` × 1/256 × 360) modulo 360 in 32-bit floats). The only use
  found is the decision whether the billboard is drawn mirrored (§5.6, BIN2; DL26). Its uniform
  spread in the data fits a random heading per sprite.
- Atlas cell: the imposter entry's (`column`, `row`) and the table's `cell_u`, `cell_v`
  (`misc-records.md` §5.4); the client takes the table's two texture lists as the sprite
  batch's two textures (BIN).

These are the "distant imposters" of the far band (`terrain-texturing.md` §12, ER): trees and
similar scenery shown as flat sprites beyond the full-detail scenery.

<a id="s4-9"></a>
### 4.9 Far entities (layout Confirmed; registration BIN; drawing OPEN)

Each entity entry repeats a placement from the sector's own landblocks (DL17): in all
304,450 entries the instance ID's region and landblock (`entity-descriptor.md` §4.2) name a
landblock of the sector, the Position names the same landblock with cell 0 (outdoors), and
the view distance √`view_distance_sq` is at least 250.0. The set resembles the long-range
nodes of `entity-descriptor.md` §9.3 (*v* ≥ 250) but is not the same set: it holds 302,878
Physics placements (§9.3 counts 314,957 long-range Physics nodes in the shared world) and all
1,335 Weenie placements of the 29 distant-battle unit groups (§9.3 counts 546 long-range
Weenie nodes). Whether every entry matches a stored SED field by field was not checked (OPEN).

The client hands the list to its world-object layer on every sector update (BIN). There
(BIN2 (h), DL28), for each entry:

1. if the layer's table of far placements already holds the entry's instance ID, the entry is
   skipped (so a sector that is loaded again, or an entry repeated in two records, adds
   nothing);
2. otherwise the client builds a placement object from the entry, copying the entity type,
   object DID, instance ID, the three scale values, the Position, the property and the squared
   view distance (the object's own default for that field, before the copy, is 1,000,000,
   that is a view distance of 1,000 units), and adds it to the table under the instance ID.

The placement object is of the same kind the world-object layer builds elsewhere (one object
type, two builders), which suggests the far entries are handled like ordinary placements
(Inferred). When those objects are created as entities, drawn and removed was not traced
(OPEN, §10).

---

<a id="s5"></a>
## 5. Client behaviour (BIN unless marked)

<a id="s5-1"></a>
### 5.1 Which sectors are loaded

1. The viewer's sector key is (⌊viewer landblock X / 3⌋, ⌊viewer landblock Y / 3⌋), when
   the viewer's region is valid and 3·key < 255 on both axes.
2. With a far radius *F* in sectors, the client lists
   every key (*sx*, *sy*) with |*sx* − viewer *sx*| ≤ *F* and |*sy* − viewer *sy*| ≤ *F*
   whose corner landblock is inside 0–254 on both axes. It asks for each listed sector's
   record and skips those that do not exist. *F* = ⌊*R* / 3⌋ + 1, where *R* is the
   landscape far radius in landblocks that the landscape draw-distance option gives (2, 4,
   6, 9 or 12 for levels 0–4; `terrain.md` TR20, `walker.md` §11.3; BIN there). *R* is a
   user setting: read it from Tessera's own settings, do not assume a level.
3. The client re-evaluates the list when the viewer's region, landblock or instance changes or
   a radius setting changes. When the viewer changes region or moves by more than one
   landblock on either axis, it first discards every loaded sector.

<a id="s5-2"></a>
### 5.2 Level of detail and near field per landblock

For every landblock of a loaded sector the client computes, from the landblock offsets
*dx*, *dy* to the viewer's landblock:

- LOD = 0 when *dx*² + *dy*² ≤ 3; 1 when ≤ 24; 2 when ≤ 99; 3 otherwise. LOD *l* uses every
  2^*l*-th sample (§5.3).
- **near** when max(|*dx*|, |*dy*|) ≤ the near radius *N* = 1 landblock: the viewer's
  landblock and its eight neighbours (`terrain.md` TR21, BIN there; DL29).

Landblocks outside the near field go into the sector's main draw batch; landblocks inside it
go into a second batch of the **same** far mesh (BIN2 (i)): the near landblocks are not left
out of the distant mesh. The client sets a flag on the second batch's draw item and clears
it on the first. What that flag changes in the renderer was not traced (OPEN, §10).
**Inferred** (`terrain.md` §9): the layered near terrain covers the same 3 × 3 landblocks,
so the flagged batch is the part of the far mesh that lies under the full-detail terrain.
The ceiling mesh contributes only landblocks whose mask bit is set.

The client also tracks, over the landblocks on the outer ring of the loaded area, the lowest
of (water height when `wet`, else the chunk's `min` *z*). Its use was not traced.

<a id="s5-3"></a>
### 5.3 Mesh construction

**Vertices.** For a batch whose finest LOD is *l*, the client builds one vertex per sample
(*i*, *j*) with *i* and *j* multiples of 2^*l* (33, 17, 9 or 5 per side and landblock), for
each landblock in the order X outer, Y inner. Each vertex has:

- position: sector-local *x* = 160·*x* + *i* × `block_size` / 32, *y* = 160·*y* +
  *j* × `block_size` / 32, *z* = converted height (§4.2), placed at the sector's world origin;
- normal: decoded as §4.3;
- texture coordinates (*u*, *v*) = ((*x* + *i*/32) / 3, 1 − (*y* + *j*/32) / 3) with *x*,
  *y* the landblock offsets 0–2: *u* = sector *x* / 480, *v* = 1 − sector *y* / 480.

**Indices.** The stored index count is 0 in this install; the client then uses an index
list it generates once (one for terrain meshes, one for ceiling meshes):

1. For each chunk position *r* = 0…8 (base vertex 1,089·*r*) and, inside it, for each LOD
   *l* = 0…3: with step *s* = 2^*l* and *q* = 32 / *s* quads per side, for each quad row
   *j* = 0, *s*, …, 32 − *s* and, inside it, each quad column *i* = 0, *s*, …, 32 − *s*:
   append two triangles over the samples SW = (*i*, *j*), SE = (*i* + *s*, *j*),
   NW = (*i*, *j* + *s*), NE = (*i* + *s*, *j* + *s*), each sample index 33·row + column plus
   the base vertex.
2. Terrain: triangles (NW, SW, NE) and (NE, SW, SE): the diagonal runs south-west to
   north-east in **every** quad (the far mesh ignores the vertex-flag diagonals of
   `landblock.md` §3.6), counter-clockwise seen from above.
3. Ceiling: (NE, SW, NW) and (SE, SW, NE): the same diagonal, clockwise from above (facing
   down).

The stored `lods` ranges (§4.5) point into this list. For LOD *l* > 0 the client remaps each
full-resolution sample index to its index in the LOD vertex set.

**Stitching between LODs.** For each landblock edge (west, south, east, north) the client
adds stitching triangles when the neighbouring landblock across that edge is covered by a
loaded sector, has a different LOD, and is either in the near field or has the lower (finer)
LOD. It then adds, for every segment of the coarser side's edge (2^*l* samples long), one
triangle joining the segment's two end samples with the edge sample at the segment's middle,
oriented like the mesh. These are vertical slivers that close the crack between the coarse
edge and the finer neighbour's edge. With the thresholds of §5.2, two landblocks that share
an edge never differ by more than one LOD (checked for every offset up to 60 landblocks), so
the finer edge has exactly one extra sample in the middle of each coarse segment and one
triangle per segment closes it.

<a id="s5-4"></a>
### 5.4 Material

Each sector mesh gets a copy of the client's distant-terrain material with parameter 3000 =
the mesh's colour list (§4.7), 4005 = 0 and 4006 = 0 (texture wrap off in U and V,
`material.md` labels), and 3001 = a texture held by the sector manager (not traced). The base
material is switched by a far-landscape normal-map option; which texture supplies the normal
map was not traced (OPEN, possibly `tile`).

**Inferred:** this sector material is the "first material of the sector object" that
`terrain-texturing.md` §12 says the near terrain's sector overlay uses; the overlay's
1/480 scale on *u* and −1/480 on *v* match §5.3's coordinates. The client passes it to the
near-terrain builder only while its near/far seam-blending option is on and the normal-map
material is active (BIN).

<a id="s5-5"></a>
### 5.5 Distant water

For each pair index *w* of the water list the client builds one mesh with a 160 × 160 quad
for every terrain chunk whose `wet` = 1 and `water_index` = *w*, at the pair's height, normal
straight up, texture coordinates (*x*/3, 1 − *y*/3) at the quad corners, drawn with the
surface the region's water table gives for the pair's type (`water.md` §8.1). When
`flow_map` ≠ 0 the client uses a copy of that material with parameter 3075 set to `flow_map`.
This is the builder `water.md` §8.1 describes; this spec adds where its inputs are stored.

<a id="s5-6"></a>
### 5.6 Sprites

When a sector is drawn and its imposter table has entries, the client walks the sprite
list: it looks each `name_key` up in that table, skips misses, and adds a billboard with the
position of §4.8 offset by the sector's world origin, the size of §4.8, the table entry's
atlas cell, a fade parameter and a mirror flag. The billboards of a sector are batched with
its imposter table's two atlases. The client builds the billboards when it builds the
sector's draw data (§5.1 re-evaluation), not every frame. Details (BIN2 (g) unless marked):

1. **Condition.** Billboards are built only while the distant-imposter graphics option is
   on (DL27; the option the preferences call `DistantImposters`, `walker.md` §11.2, built-in
   value on; its identification is Probable) and only for a sector whose imposter table has
   at least one entry.
2. **Viewer frame.** Let (*x*, *y*) be the sprite's horizontal position relative to the
   south-west corner of the **viewer's landblock** (the landblock of §5.1 step 1): its
   sector-local `east`, `north` plus 160 × (sector corner landblock − viewer landblock) on
   each axis. (When the sector lies in another region than the viewer the client uses an
   offset of −2,000 on each axis; such sectors are not listed by §5.1.)
3. **Fade parameter** *P* (DL25).
   - When −160 ≤ *x* ≤ 320 and −160 ≤ *y* ≤ 320 (the sprite lies on the viewer's
     landblock or one of its eight neighbours), *P* = the sprite's stored `distance`.
   - Otherwise *P* = *S* × *K* / *M_d*, with *S* the billboard size of §4.8, *M_d* the
     object draw-distance multiplier (0.6, 0.8, 1.0, 1.4 or 2.0 for the object draw-distance
     levels 0–4, `mesh.md` §8.7, `walker.md` §11.2), and *K* = (*H* / *φ*) / 248, where *H*
     is the height of the render target in pixels and *φ* the camera's field of view in
     radians (the client converts the stored field of view in degrees with π / 180).
     Identifying *H* and *φ* with those two quantities is Inferred: the client reads them
     from the render device and the camera, whose fields were not mapped one by one.
   *P* is handed to the sprite program, which fades the sprite in over 10 units beyond a
   start distance computed from *P* (`misc-records.md` §18).
4. **Mirror flag** (DL26). With *θ* the heading of §4.8 converted to radians, the billboard is
   drawn mirrored exactly when (*x* − 160) × sin *θ* + (*y* − 160) × cos *θ* > 0 (strictly).
   The point (160, 160) is the north-east corner of the viewer's landblock in the frame of
   step 2. How a mirrored billboard is drawn: `misc-records.md` §5.4 (horizontal flip of
   the atlas cell, normal R axis negated).
5. The atlas cell is the table entry's (`column`, `row`) (`misc-records.md` §5.4).

The billboard corners, atlas texture coordinates and the sprite program are specified in
`misc-records.md` §5.2–§5.4.

<a id="s5-7"></a>
### 5.7 Entities

See §4.9.

<a id="s5-8"></a>
### 5.8 Failures

A record that fails to read, or whose version is below 2, leaves the sector marked
unreadable; the client then logs it and shows nothing for that sector (BIN). A sector whose
colour list is missing is still built, without texture. **Tessera rule:** the same: skip the
sector, log it once, keep going.

---

<a id="s6"></a>
## 6. Using the records in Tessera (Tessera Decision unless stated)

<a id="s6-1"></a>
### 6.1 Horizon in the walker

1. Load the sectors of §5.1 with a far radius chosen by Tessera (suggested: 8 sectors, about
   3.8 km; the original uses *F* = ⌊*R* / 3⌋ + 1, at most 5 sectors, §5.1) and skip the
   landblocks the full-detail terrain already draws (the near field of §5.2 with *N* = the
   full-detail ring radius).
2. Build the far mesh per §5.3 (positions, normals, texture coordinates and the generated
   index layout). Use the LOD rule of §5.2. Either reproduce the stitching triangles of §5.3
   or hang vertical skirts below every chunk edge; both close cracks.
3. Texture with `colour_list` (§5.4); draw the ceiling mesh only where the walker draws
   ceilings for the near field.
4. Draw distant water per §5.5 (equivalent to `water.md` §9's block-map rule, now readable
   from the record).
5. Sprites (§5.6) are optional for M5-level fidelity; when drawn, use the size rule of §4.8.

<a id="s6-2"></a>
### 6.2 Explorer region overview

Per region, read every sector record and draw each block's 33 × 33 heights and normals at
the sector's place (85 × 85 sectors of 97 × 97 distinct points, neighbours sharing their
edges: 8,161 × 8,161 points per region). Shade with the stored normals. Missing sectors are empty. This needs 33,159 records
instead of 390,150 kind-0 records and gives the same heights (§4.2).

<a id="s6-3"></a>
### 6.3 Coarse summaries for tools and an own server

Per sector: height range from the terrain mesh's bounds; per landblock: height range from the
chunk bounds, water flag and water height from the chunk and the water list, ceiling flag
from the mask. These are exact copies of facts the landblock records also hold (§4.2, §4.4,
§4.6).

---

<a id="s7"></a>
## 7. Invariants (Confirmed on all 33,159 records, 2026-10-07, unless marked)

| # | Invariant |
|---|---|
| DL-I1 | The record decodes with §3.1 and no bytes remain. |
| DL-I2 | The DID's kind is 5 and its X and Y are multiples of 3 below 255. |
| DL-I3 | The terrain mesh's sample count is 9,801; its index count is 0; the ceiling mesh's index count is 0. |
| DL-I4 | Terrain chunk *c* (landblock *x* = *c* mod 3, *y* = ⌊*c*/3⌋): `vertex_start` = 1,089·(3·*x* + *y*), `vertex_count` = 1,089, `min`/`max` *x*, *y* = the landblock square, `lods` as §4.5. |
| DL-I5 | `min` *z* and `max` *z* of every present chunk equal the lowest and highest converted heights of its 1,089 samples; the mesh bounds are the union of its chunks'. |
| DL-I6 | Terrain block *k* = 3·*x* + *y*, read Y-major, equals the kind-0 height grid of landblock (*X* + *x*, *Y* + *y*) (all 298,431). |
| DL-I7 | `mode` ∈ {0, 1, 2}; the effective mask equals the set of the nine landblocks with `has_ceiling` = 1; `mode` = 1 exactly when the mask is 0x1FF. |
| DL-I8 | When `mode` ≠ 0: the ceiling sample count is 1,089 × popcount(mask); present ceiling chunks are packed by rank in vertex-block order; each present block equals its landblock's ceiling grid; absent ceiling chunks are 66 zero bytes. |
| DL-I9 | `block_size` = 160.0. |
| DL-I10 | The water pairs are distinct, each is used by a chunk, and they appear in first-use order over vertex-block order; `wet` and the named pair equal `water.md` §6.2 for every landblock; dry chunks have `water_index` 0; ceiling chunks are never wet. |
| DL-I11 | Every sprite lies inside 0 ≤ `east`, `north` ≤ 480; `scale` > 0. |
| DL-I12 | Every entity's instance ID and Position name a landblock of the sector, cell 0; the three scale values are equal; `view_distance_sq` ≥ 62,500. |
| DL-I13 | `imposter_table` is the 0x27 table of the square containing the sector; `colour_list` and `tile` are non-zero and exist; `ceiling_colour_list` and `ceiling_tile` are non-zero exactly when `mode` ≠ 0. |
| DL-I14 | Every stored terrain and ceiling normal is within one encoding step, per component, of the rule of §4.3 (all samples, edges and corners included; Confirmed 2026-10-08, DL7, DL23). The middle byte of every ceiling sample is at most 126. |
| DL-I15 | In a `mode` 2 record `ceiling_mask` has no bit above bit 8 (Confirmed; the client would ignore such bits, §3.5). |

---

<a id="s8"></a>
## 8. Worked examples (synthetic; not taken from the install)

<a id="s8-1"></a>
### 8.1 The smallest record (49,673 bytes)

A sector with no ceiling, water, sprites or entities.

| Offset | Size | Content |
|---|---|---|
| 0 | 4 | DID, e.g. `0x80510009` (region 1, X 0, Y 9) |
| 4 | 4 | 9,801 (`49 26 00 00`) |
| 8 | 49,005 | 9,801 samples |
| 49,013 | 4 | Index count 0 |
| 49,017 | 594 | Nine chunk entries (§8.2) |
| 49,611 | 24 | `bounds_min`, `bounds_max` |
| 49,635 | 1 | `mode` = 0 |
| 49,636 | 4 | `block_size` 160.0 (`00 00 20 43`) |
| 49,640 | 1 | Water count 0 (VLE) |
| 49,641 | 4 | Sprite count 0 |
| 49,645 | 4 | Entity count 0 |
| 49,649 | 24 | Six DIDs: colour list, 0, tile, 0, 0, imposter table `0x27010000` (`00 00 01 27`) |
| 49,673 | | End |

<a id="s8-2"></a>
### 8.2 One chunk entry

Chunk *c* = 1 (landblock *x* = 1, *y* = 0), lowest stored height 3,277 and highest 4,915
(converted by §4.2 to 100.00763 and 149.99619; revised 2026-10-08, the earlier 100.0 and
150.0 are not reachable by that conversion), dry:

`C3 0C 00 00` (vertex start 3,267) · `41 04 00 00` (1,089) · `00 00 20 43` `00 00 00 00`
`E8 03 C8 42` (min 160, 0, 100.00763) · `00 00 A0 43` `00 00 20 43` `06 FF 15 43` (max 320,
160, 149.99619) · `00` (`wet`) · `00` (`water_index`) · `A0 5F 00 00` `00 08 00 00` (24,480, 2,048) ·
`A0 77 00 00` `00 02 00 00` (30,624, 512) · `A0 7D 00 00` `80 00 00 00` (32,160, 128) ·
`20 7F 00 00` `20 00 00 00` (32,544, 32). 66 bytes.

<a id="s8-3"></a>
### 8.3 Water

Landblocks (*x*, *y*) = (0, 0) and (1, 0) have water type 3 at level 38, landblock (0, 2)
type 5 at level 86, the rest are dry. Visiting in vertex-block order (0,0), (0,1), (0,2),
(1,0), …: first use gives pair 0 = (3, 149.01962) and pair 1 = (5, 337.25491).

- Water list: `02` · `03 06 05 15 43` · `05 A1 A0 A8 43` (the heights are the 32-bit results
  of `water.md` §6.2 step 6).
- Chunk 0 (0,0): `wet` 1, `water_index` 0. Chunk 1 (1,0): 1, 0. Chunk 6 (0,2): 1, 1. All
  other chunks: 0, 0.

<a id="s8-4"></a>
### 8.4 A sprite and an entity

Sprite: key 600 at (100, 250, 300) with scale 1, distance 140, byte 0x40:
`58 02` · `00 00 C8 42` · `00 00 7A 43` · `00 00 96 43` · `00 00 80 3F` · `00 00 0C 43` ·
`40` (23 bytes). It stands at sector-local (100, 300), height 250.

Entity: a Physics placement (type 7) of object `0x47001234`, instance ID kind 1, region 1,
landblock (3, 0xF0), serial 5, scale 1, at block-local (10, 20, 300) without orientation,
no property, view distance 1,000:
`07 00 00 00` · `34 12 00 47` · `05 00 00 00 00 3F 10 01` · `00 00 80 3F` × 3 ·
`DB 01 03 F0 00 00 00 00 20 41 00 00 A0 41 00 00 96 43` (Position: flags 0xDB, region 1,
landblock 3/0xF0, cell 0, *x*, *y*, *z*) · `00 00 00 00` (null property) · `00 24 74 49`
(1,000,000). 54 bytes. Its sector is `0x805103F0`.

---

<a id="s9"></a>
## 9. Census (OBS, 2026-10-07)

| Quantity | Value |
|---|---|
| Records | 33,159 (stamps 38: 24,434; 39: 8,725) |
| Stored / unpacked bytes | 799,063,698 / 1,787,523,554 |
| Trailing section | 660 – 224,444 bytes (162,301,487 in all) |
| `mode` | 0: 32,218; 1: 608; 2: 333 |
| Ceiling blocks | 6,780 |
| Water pairs per record | 0: 20,234; 1: 11,737; 2: 960; 3: 174; 4: 48; 5: 6 |
| Wet terrain chunks | 89,913 (water index 0: 86,850; 1: 2,636; 2: 347; 3: 73; 4: 7) |
| Sprites | 3,549,236 in 8,159 records (at most 5,194 in one record) |
| Sprite `distance` values | 140 (2,460,251), 80 (664,194), 180 (206,938), 200 (159,382), 1,000 (42,498), 300 (15,190), 45 (783) |
| Entities | 304,450 in 4,306 records (at most 2,074 in one record) |
| Entity Position flags | 0xBB 227,406; 0xFB 50,052; 0x9B 19,647; 0xDB 7,338; 0x8B 7 |
| `flow_map` present | 1,998 records |

---

<a id="s10"></a>
## 10. Open questions

> **OPEN:** How the client obtains the format version of a sector record (DL19). Every record
> of this install decodes with the newest grammar; the version is not stored in the record.
> The reader asks the stream's version source for the version of this record type (BIN2);
> that source (directory stamp, iteration or an archive-level table) was not traced.

> **OPEN:** What the flag on the near-field batch of §5.2 changes in the renderer (for
> example drawing only where full-detail terrain is not resident, a depth offset, or a
> separate pass). The radii are answered: *N* = 1, *F* = ⌊*R* / 3⌋ + 1 (§5.1, §5.2).

> **OPEN:** The use of `tile` and `ceiling_tile` (96 × 96 DXT3, alpha 0 everywhere,
> `texture-list.md` §6.3.1): loaded as dependencies, no reader found in the far-terrain,
> water or sprite code. A normal map for the far-landscape normal-map option is one
> possibility.

> **OPEN:** Texture parameter 3001 of the sector material (a texture held by the sector
> manager) and the normal-map source of the far-landscape normal-map option (§5.4).

> **OPEN:** Sprites: the value of the sprite program constant that scales the fade parameter
> *P* into the fade start distance (`misc-records.md` §18 gives the program side; the value
> the client loads into that constant was not found). Answered 2026-10-08: the role of
> `distance` and of `byte`, the mirror flag and its angle, the size rule (§4.8, §5.6).

> **OPEN:** Far entities (§4.9): whether every entry equals a stored SED of its landblock
> field by field, and when the client creates, draws and removes entities from the
> placement objects it registers (registration answered 2026-10-08, §4.9).

Answered 2026-10-08 (were OPEN): the rule of the stored ceiling normals and of the terrain
normals at block edges and corners (§4.3, DL23).

---

<a id="s11"></a>
## 11. Answers to OPEN items in other specs (for their owners to adopt)

This spec does not edit other specs. Their owners may adopt:

| Spec | Item | Answer here |
|---|---|---|
| `landblock.md` §7 | OPEN on sample spacing and meaning, and the trailing-section grammar | §3, §4.1–§4.5 |
| `landblock.md` §7 | Implementer OPEN of 2026-10-06 (positional read of the water list, per-landblock part) | §3.6, §4.6: the list is field 8 of §3.1; the per-landblock part is the chunk's `wet` byte and `water_index` byte, chunk *c* = *x* + 3·*y*; the nine entries come before the list (inside the terrain mesh block) |
| `landblock.md` §7 | "The u16 values do not equal the terrain heights" | Corrected by §4.2: they are equal under the block order of §4.1 |
| `water.md` §9 | "The rest of the record's layout is OPEN"; Tessera rule to derive from block maps | §3, §4.6, §5.5 (both routes give the same pairs) |
| `texture-list.md` §6.3.1 | OPEN (1): role of the two 96 × 96 tiles | Positions explained (§3.9: `tile`, `ceiling_tile`); role still OPEN (§10) |
| `misc-records.md` §5 | "Nothing in the data names these DIDs" | Every sector names its square's imposter table (§2.3); sprites use `value_a`, `value_b` as size and lower-edge height (§4.8; BIN, `misc-records.md` I5) |
| `terrain-texturing.md` §12 | OPEN on the sector overlay | §5.4 (Inferred identification of the sector material; conditions BIN) |
| `walker.md` §11.9 | OPEN on `DistantImposters` | The option gates the sector sprites of §4.8 and §5.6 (DL27); how far they show is the fade rule of §5.6 step 3 with `misc-records.md` §18 |
| `terrain.md` §9 | OPEN (a) on which landblocks the far mesh draws | Partly: the far mesh also holds the 3 × 3 near landblocks, in a second, flagged batch (§5.2, BIN2 (i)); what the flag does is still OPEN |
| `misc-records.md` §18 | Proposed answers for this spec (size roles, fade, mirror flag) | Adopted in §4.8 and §5.6 (rev. 2026-10-08) |

---

<a id="s12"></a>
## 12. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review): full layout of kind-5 records over all 33,159 records (BIN + OBS); sample order and heights, normals, ceiling mesh, chunk entries, water list and per-landblock water, sprite and entity lists, DID tail; client behaviour for loading, LOD, mesh and index generation, stitching, material, distant water and sprites (BIN); invariants, examples, census, OPENs. |
| 2026-10-07 | Review passed (separate reviewer session); status Accepted. No content change. |
| 2026-10-08 | Spec writer (dirty room; M1 probe OBS2, M4 BIN2 under decision 0011). Status Accepted → Revised. Implementer OPEN of 2026-10-08 answered: client ignores `ceiling_mask` bits 9–31 (§3.5, DL24); Tessera keeps rejecting them and the null-property encoder is a Tessera Decision (§3.11); new invariant DL-I15. §3.2: Tessera rule for stored indices (review note 3). §4.3: normal rule for every sample, edges, corners and ceilings (DL7 now Confirmed, DL23; DL-I14 widened; review note 1). §4.8, §5.6: sprite size roles BIN (from `misc-records.md`), `distance` = fade parameter with the near/far selection rule, `byte` = heading, mirror-flag rule, option gate (DL25–DL27). §4.9: far-entity registration by instance ID (DL28). §5.1, §5.2: radii *N* = 1 and *F* = ⌊*R*/3⌋ + 1 adopted from `terrain.md`, near batch belongs to the same far mesh (DL29). §5.3: at most one LOD step between neighbours (review note 2). §6.1 step 1: the original's radius noted. §8.2: example heights now reachable (review note 4). §10: OPENs narrowed or closed; §11: three rows. |
