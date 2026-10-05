<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Environment: regions, sky, time of day, fog and weather (water: `water.md`)

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested (first round; addressed). Re-review 2026-10-05: passed (CLEAN_ROOM.md §5.3, every item; spot checks: all 7 region descriptors 114 bytes with the §3.1/§3.2 values, 0x15000007 header, all 963 0x1A records, 0x35000000, 0x32000000 1,160 bytes and its d0–d7 ranges, the §14 hash values, LC region-loader slot order). |
| **Implements into** | `crates/formats` (region descriptor, sky description 0x15, day description 0x1A, game time 0x35), `crates/world` (per-landblock sky key, game clock, day-description selection, keyframe blending, sky objects), `crates/render` (sky objects, fog, sun and ambient inputs of [`lighting.md`](lighting.md)) |
| **Companion specs** | `records.md` §2.1 (prefix → DB type table), §7 (enum mappers). `property-system.md` §8 (DID maps), §9 (DB-object-type codes). [`landblock.md`](landblock.md) (block maps, which carry the per-landblock sky key). `entity-descriptor.md` (static entities: lights, time-of-day overrides; 0x47 objects). [`lighting.md`](lighting.md) (how the values below light a surface). `material.md` §5.3 (float curves, shared with §5.3 here). The per-region tables named by the region descriptor are decoded in [`scenery.md`](scenery.md) (0x11, 0x12, 0x02), [`terrain-texturing.md`](terrain-texturing.md) (0x19) and [`water.md`](water.md) (0x16 water, 0x17 fog zones; all water). `coordinates.md` (map coordinates, §3.1 here). |

---

<a id="s0"></a>
## 0. Purpose and status

This spec lets an implementer:

- decode a region's **region descriptor** to the last byte and find its per-region tables
  (§3), and know which spec decodes each of them (§4);
- decode the shared **sky description** 0x15000007 (§5.1) and the 963 **day descriptions**
  (prefix 0x1A, §5.2–§5.5) to the last byte;
- run the **game clock** from the game-time record 0x35000000 (§6, §11.1);
- choose the day description for a landblock's sky key and the current game day (§11.2),
  blend its **time-of-day keyframes** (§11.3) into sun, ambient and fog values for
  [`lighting.md`](lighting.md), and animate the **sky objects** (sun, moon, clouds, domes)
  (§11.4, §12);
- know which properties select skies, interior fog and time-of-day behaviour (§7). Water and
  the region's fog zones are specified in [`water.md`](water.md) (§8 here is a pointer).

Values quoted from the install are observations; implementations read them at run time.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 by throwaway probes in the dirty-room scratchpad: every record of prefixes 0x0F, 0x11, 0x12, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x32, 0x35 read; DID-like values located by scanning; block-map descriptors 0x18 fully decoded; property sets of all landblock-info records, cells, SEDs, 0x804 landblock properties and all 0x78/0x79 property sets scanned for the properties of §7. |
| OBS2 | Observed in user install 2026-10-05: 0x15000007 parsed to the last byte with the grammar of §5.1; all 963 day descriptions (0x1A) parsed to the last byte with §5.2–§5.4 (7,590 keyframes, 12,788 sky objects, 107,213 keyframe sky-object entries); 0x35000000 parsed to the last byte (§6); censuses of every field. Enum mappers dumped (labels quoted below come from them). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`lotroclient64.exe`, `lotro_live`), disassembled with a general-purpose disassembler around: the deserialisers of DB object types 0x3F (sky description), 0x3A (day description, its keyframes and sky objects) and 0x02 (game time); the keyframe and sky-object blending; the day-description selection; the game clock. **Question:** field boundaries that the data leaves ambiguous (variable-length parts inside keyframes and sky objects), the type of each record, and how keyframes are combined over time. **Why M1–M3 were not enough:** the keyframe tail and sky objects contain optional and variable parts whose boundaries cannot be found reliably from zero-heavy data; the blending, angle interpolation and selection rules exist only as code; no running client is available to this session for M3. Facts resting on BIN alone are marked **BIN**. |
| PS | `property-system.md` §8–§9 (DID maps; DB-object-type mapper "DBOType"). |
| DS | `research/notes/data-semantics.md` §6.5 (block maps), §13 (environment, sky, weather and water listed as unknown; no public decoder). |
| ER | `research/notes/engine-client-re.md` §3 (renderer settings), §4 (map coordinates). |
| AC | Asheron's Call lineage (published AC format descriptions in the research clones) used only as a naming analogy for "sky description", "day group", "sky object", "time-of-day keyframe". No AC layout is assumed: every layout below was checked against this install. |
| BIN-R | **M4 under decision 0011**, second pass on 2026-10-05, same client build (4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5). **Questions:** (a) which DB object type the client registers for each DID range (to replace the content-based kind hypotheses of §4 and §9); (b) the region descriptor's deserialiser and the order of its DID maps, and which code asks which map; (c) how the client turns a position into map coordinates with the region's fields; (d) the exact comparisons of the keyframe search of §11.3 at a keyframe's own time. **Why M1–M3 were not enough:** the region fields at offsets 8–32 and the empty maps cannot be told apart from content; the record kinds are named nowhere in the data except through these registrations; the boundary behaviour of §11.3 is code only; no running client is available for M3. |
| OBS3 | Observed in user install 2026-10-05: all 7 region descriptors parsed with §3.1 (zero leftover bytes); the offset-8 DIDs resolved; 0x32000000 parsed (§9); the record-prologue census of `records.md` §1.6. |
| LC | LotroCompanion `lotro-tools` (readable source, its region loader; facts only, no code, comments or identifiers taken): reads region-descriptor offset 8 as an image DID and takes slots 1–6 of §3.1 as the scene, terrain, encounter, sky, water and fog tables, in that order. Reported only; the layout here rests on BIN-R and OBS3. |

| Mark | Meaning |
|---|---|
| Confirmed | Checked over all records of the kind |
| Observed | Seen in the records named; layout not decoded end to end |
| BIN | Rests on disassembly of the client (decision 0011) |
| Inferred | A reading that fits the observations but is not proven |
| Tessera Decision | Tessera's own rule where the original's is not known; never a statement about the original |

| # | Fact | Sources | Mark |
|---|---|---|---|
| E1 | 0x15 is DB type 0x3F (sky description), 0x1A is type 0x3A (day description), 0x35 is type 0x02 (game time) (§2) | BIN (the loader requests these type codes for these DIDs) | BIN |
| E2 | Sky description layout (§5.1) | OBS2 (1 of 1 record, zero bytes left); BIN (field order, the two optional forms) | Confirmed |
| E3 | Day description layout: header, keyframes, sky objects, transition time (§5.2–§5.4) | OBS2 (963 of 963, zero bytes left); BIN (boundaries of optional parts) | Confirmed |
| E4 | Keyframe field meanings: time, fog, directional light, ambient (§5.3) | BIN (blending code routes them to the fog and light state; sun direction built from heading and pitch) | BIN |
| E5 | Colour encoding u32 0xAARRGGBB, stored little-endian (bytes B, G, R, A) (§5.6) | BIN | BIN |
| E6 | Game-time layout and clock (§6, §11.1) | OBS2; BIN | Layout Confirmed; clock BIN |
| E7 | Day-description choice: weighted pick seeded by the game day number (§11.2) | BIN | BIN |
| E8 | Keyframe blending and angle interpolation (§11.3) | BIN | BIN |
| E9 | Sky-object animation (§11.4) | BIN; OBS2 | BIN |
| E10 | Day types (enum mapper 0x230000E8) | OBS2 (labels) | Confirmed labels |
| E11 | Region descriptor layout: fixed fields, nine DID maps, trailing terrain-type-table DID (§3.1) | BIN-R (b); OBS3 (7 of 7, zero leftover bytes) | Confirmed |
| E12 | Kind of every per-region table and of prefixes 0x02, 0x08, 0x2D, 0x32, 0x39, 0x44 (§2, §4, §9) | BIN-R (a); labels from the DBOType mapper (OBS) | Confirmed (BIN registration; content agrees) |
| E13 | Region offset 8 is the DID of a 0x41 map image; offsets 16–32 are the map-coordinate origin (in 20-unit steps), two map offsets and the map scale (§3.1) | BIN-R (b), (c); OBS3 (both offset-8 DIDs exist: 512 × 512 user-interface images); LC (names) | BIN; image DIDs Confirmed |
| E14 | The keyframe search of §11.3 uses exactly the comparisons stated there, so a time equal to an interior keyframe's time blends its two neighbours (§11.3) | BIN-R (d) | BIN |
| E15 | Prefix 0x32 is the render LOD-class table: DID, count, entries of (class key, 8 × f32) (§9) | BIN-R (a); OBS3 (1 of 1, zero leftover bytes) | Confirmed layout; field meanings OPEN |

---

<a id="s2"></a>
## 2. Vocabulary from the data (Confirmed labels)

The DB-object-type enum mapper (labelled "DBOType" in the EMAPPER DID map; base mapper
0x230000F0 in this install, `property-system.md` §9) names the record kinds the engine
knows. The environment-related labels, with their codes:

| Code | Label | Code | Label |
|---|---|---|---|
| 0x02 | GAME_TIME | 0x31 | LAND_BLOCK_TERRAIN_LIST |
| 0x13 | ENCOUNTER_DESC | 0x35 | TERRAIN_TYPE_TABLE |
| 0x16 | FOG_DESC | 0x3A | DAY_DESC |
| 0x17 | WATER_DESC | 0x3F | SKY_DESC |
| 0x18 | FARVIEWENTITIES | 0x41 | DISTANT_LANDSCAPE_SECTOR |
| 0x1C | TERRAIN_DESC | 0x4A | SCENE_DESC |
| 0x1E | PSDESC | 0x4F | REGION |
| 0x30 | RENDER_IMPOSTER_TABLE | 0x56 | FORKPSDESC |

The code is **not** the DID prefix (for example RENDERSURFACE is code 0x05 but prefix 0x41).
The client registers one type code per DID range (BIN-R (a), E12); the complete table is in
`records.md` §2.1. The pairings that concern this spec:

| DID prefix | Type code | Label | Kind | Spec |
|---|---|---|---|---|
| 0x0F | 0x4F | REGION | Region descriptor | §3 |
| 0x11 | 0x4A | SCENE_DESC | Scene-choice table | `scenery.md` §4.3 |
| 0x12 | 0x1C | TERRAIN_DESC | Terrain-scene table | `scenery.md` §4.2 |
| 0x14 | 0x13 | ENCOUNTER_DESC | Encounter table | §4 (layout OPEN) |
| 0x15 | 0x3F | SKY_DESC | Sky description | §5.1 |
| 0x16 | 0x17 | WATER_DESC | Water table | `water.md` §4 |
| 0x17 | 0x16 | FOG_DESC | Fog-zone table | `water.md` §5 |
| 0x18 | 0x21 | PROPERTY_DESC | Block-map property descriptor | `landblock.md` §5.2 |
| 0x19 | 0x35 | TERRAIN_TYPE_TABLE | Terrain type table | `terrain-texturing.md` §3 |
| 0x1A | 0x3A | DAY_DESC | Day description | §5.2 |
| 0x35 | 0x02 | GAME_TIME | Game time | §6 |
| 0x02 | 0x3C | SCENE | Scene record | `scenery.md` §5 |
| 0x32 | 0x3B | RENDER_LOD_CLASSES | LOD-class table | §9 |
| 0x80 K 5 | 0x41 | DISTANT_LANDSCAPE_SECTOR | Distant-landscape sector | `landblock.md` §7 |

Note the crossed codes of 0x16 and 0x17: the DID prefix and the type code do not match
(`water.md` §3).

Related enum mappers (labels are data; read at run time):

| Mapper | Content |
|---|---|
| 0x230000E8 (base 0x23000185) | **Day types**, 13 values 0x10000001 – 0x1000000D. Labels name three weather words each (for example 0x10000001 "FairFairFair", 0x10000004 "FairCloudyFair", 0x10000008 "FairRainyFair", 0x1000000B "FairStormFair"). |
| 0x230000E9 (base 0x230001B4) | `Environment_LocalSky` sky keys (§7.1); base mapper value 2 is labelled "Default" |
| 0x230001E4 | Time-of-day names (§7.3) |

<a id="s3"></a>
## 3. Region descriptor (prefix 0x0F, `client_general.dat`; Confirmed, 7 of 7 records)

The REGION DID map (root key 0x11, sub-map 0x28000008, `property-system.md` §8.1) labels
eight 0x0F DIDs; seven exist (0x0F000000, 01, 02, 05, 06, 08, 09; the eighth target,
0x0F000007, does not). Every record is 114 bytes in this install; that size follows from the
grammar below (one entry in each of maps 1–7, none in maps 8 and 9) and is not a rule.

<a id="s3-1"></a>
### 3.1 Layout (Confirmed: BIN-R (b) and OBS3, 7 of 7, zero leftover bytes)

All fields little-endian. "Offset" is the offset in every record of this install.

| Order | Offset | Size | Type | Field | Observed | Meaning |
|---|---|---|---|---|---|---|
| 1 | 0 | 4 | DID | `did` | own DID | |
| 2 | 4 | 4 | u32 | `region` | 1, 2, 3, 4, 14, 5, 6 | **Landblock region number** *R* (`records.md` §2.2). 0x0F000006 → 14; 0x0F000009 → 6 (no `client_cell_6.dat` exists in this install). |
| 3 | 8 | 4 | DID | `map_image` | 0x4100000F (region 1), 0x410D9AE6 (the others) | A render surface (prefix 0x41): both are 512 × 512 user-interface images (category 15). The region's map image (Inferred; LC names it so). The first revision of this spec read these bytes as an f32 (8.0, 8.85). |
| 4 | 12 | 4 | u32 | `value_12` | 2 in all | OPEN |
| 5 | 16 | 4 | u32 | `origin_x` | 1,468 | Map-coordinate origin along X, in steps of 20 world units (BIN-R (c)) |
| 6 | 20 | 4 | u32 | `origin_y` | 1,244 | Map-coordinate origin along Y, in steps of 20 world units |
| 7 | 24 | 4 | f32 | `offset_x` | 0.0 | Added to the map X coordinate |
| 8 | 28 | 4 | f32 | `offset_y` | 0.0 | Added to the map Y coordinate |
| 9 | 32 | 4 | f32 | `map_scale` | 0.005 | Map units per world unit |
| 10 | 36 | 10 | DID map | slot 1 | 1 entry | Scene-choice table, prefix 0x11 (§4) |
| 11 | 46 | 10 | DID map | slot 2 | 1 entry | Terrain-scene table, 0x12 |
| 12 | 56 | 10 | DID map | slot 3 | 1 entry | Encounter table, 0x14 |
| 13 | 66 | 10 | DID map | slot 4 | 1 entry | Sky description, 0x15 (§5.1) |
| 14 | 76 | 10 | DID map | slot 5 | 1 entry | Water table, 0x16 (`water.md` §4) |
| 15 | 86 | 10 | DID map | slot 6 | 1 entry | Fog-zone table, 0x17 (`water.md` §5) |
| 16 | 96 | 10 | DID map | slot 7 | 1 entry | Block-map property descriptor, 0x18 (`landblock.md` §5) |
| 17 | 106 | 2 | DID map | map 8 | empty in all 7 | Kind OPEN |
| 18 | 108 | 2 | DID map | map 9 | empty in all 7 | Kind OPEN |
| 19 | 110 | 4 | DID | slot 8 | 0x19… | Terrain type table, 0x19 (`terrain-texturing.md` §3) |

A **DID map** is a TSize (`records.md` §1.3: hint byte, VLE count) followed by count ×
(u32 key, DID). Maps 1–7 hold exactly one entry here, with key 0: hint 1, count 1, key 0, DID
(10 bytes). Maps 8 and 9 are hint 1, count 0 (2 bytes). The client asks every map for key 0
only (BIN-R (b)); what other keys would select is OPEN. "Slot 1–8" is the numbering the other
specs use; slot 8 is the trailing DID, not a map.

**Versions (BIN):** the deserialiser asks the archive for a per-type version. Versions below
24 have three more u32 after field 9; versions below 4 have, in addition, one more u32 after
those and one more DID map between slots 2 and 3; versions below 2 have no trailing DID;
versions 3 and 4 end with one more u32. Every record of this install has the newest layout
above (24 or later), and a reader that knows only that layout rejects anything that does not
parse exactly.

**Map coordinates (BIN-R (c)).** For block-local (*x*, *y*) of landblock (X, Y):
map X = `offset_x` + `map_scale` × (160 · X + *x* − 20 · `origin_x`) and
map Y = `offset_y` + `map_scale` × (160 · Y + *y* − 20 · `origin_y`). The client displays
|map Y| with a north or south mark by its sign and |map X| with east or west. With this
install's values, map X = 0.005 × (160 · X + *x* − 29,360) and map Y = 0.005 × (160 · Y +
*y* − 24,880). `coordinates.md` owns map coordinates; these fields should be read at run time
rather than hard-coded.

<a id="s3-2"></a>
### 3.2 Table DIDs per region (Confirmed)

| Region descriptor | *R* | Slot 1 (0x11) | Slot 2 (0x12) | Slot 3 (0x14) | Slot 4 (0x15) | Slot 5 (0x16) | Slot 6 (0x17) | Slot 7 (0x18) | Slot 8 (0x19) |
|---|---|---|---|---|---|---|---|---|---|
| 0x0F000000 | 1 | 0x11000000 | 0x12000000 | 0x14000000 | 0x15000007 | 0x16000000 | 0x17000000 | 0x18000000 | 0x19000000 |
| 0x0F000001 | 2 | 0x11000001 | 0x12000001 | 0x14000001 | 0x15000007 | 0x16000004 | 0x17000001 | 0x18000014 | 0x19000001 |
| 0x0F000002 | 3 | 0x11000002 | 0x12000002 | 0x14000002 | 0x15000007 | 0x16000005 | 0x17000002 | 0x18000015 | 0x19000002 |
| 0x0F000005 | 4 | 0x11000007 | 0x12000007 | 0x14000007 | 0x15000007 | 0x1600000A | 0x17000007 | 0x1800001A | 0x19000007 |
| 0x0F000006 | 14 | as region 2 | | | | | | | |
| 0x0F000008 | 5 | as region 4 | | | | | | | |
| 0x0F000009 | 6 | 0x1100000A | 0x1200000A | 0x1400000A | 0x15000007 | 0x1600000E | 0x1700000A | 0x1800001E | 0x1900000A |

So the region descriptor is how a client finds a region's block-map descriptor and other
per-region tables, and the region-number ↔ descriptor mapping is data, not code. The DIDs are
observations; read them at run time.

---

<a id="s4"></a>
## 4. Per-region tables (kinds Confirmed, BIN-R (a))

Five records exist for each of the prefixes 0x11, 0x12, 0x14, 0x16, 0x17, 0x19 (serials 0, 1,
2, 7, 10 except 0x16: 0, 4, 5, 10, 14), one per distinct region set of §3.2, and one 0x15.
Each kind is decoded in the spec named here; this section replaces the content-based
hypotheses of the first revision (§15).

| Prefix | Slot | DB type | Kind | Sizes (bytes) | Decoded in |
|---|---|---|---|---|---|
| 0x11 | 1 | 0x4A SCENE_DESC | Scene-choice table: scene keys → weighted lists of scene records 0x02. Starts with u32 0, then its DID. | 1,802 – 19,613 | `scenery.md` §4.3 |
| 0x12 | 2 | 0x1C TERRAIN_DESC | Terrain-scene table: terrain-type ids of the coarse grid → scene keys | 882 – 10,934 | `scenery.md` §4.2 |
| 0x14 | 3 | 0x13 ENCOUNTER_DESC | Encounter table. Header: own DID, u32 0, u32 6 (the block-map key it reads, `landblock.md` §5.6), u32 16 or 1, then a named default entry and encounter lists. | 58 (four records), 1,687 (region 1) | Layout OPEN; the encounter pass is out of scope offline (`scenery.md` §7.7) |
| 0x15 | 4 | 0x3F SKY_DESC | Sky description (one record shared by all regions) | 30,184 | §5.1 |
| 0x16 | 5 | 0x17 WATER_DESC | Water table: water types (surface record, volume entity) and the block-map keys of water type and level | 188 – 1,574 | `water.md` §4, §6 |
| 0x17 | 6 | 0x16 FOG_DESC | Fog-zone table (block-map key 3); no zone is active in this install | 137, 1,244 | `water.md` §5 |
| 0x18 | 7 | 0x21 PROPERTY_DESC | Block-map property descriptor | | `landblock.md` §5.2 |
| 0x19 | 8 | 0x35 TERRAIN_TYPE_TABLE | Terrain type table: the surface-layer `material` keys | 40,637 – 173,939 | `terrain-texturing.md` §3; `scenery.md` §4.4 (its procedural flag) |

Corrections to the first revision: 0x11 does list DIDs (its entries name 0x02 scene records);
0x12 is the terrain-scene table, not environment zones; 0x16 is the water table and its 33-byte
elements are water types, not a terrain palette; 0x17 holds **no** texture list (the values
0x400043FA and 0x4000460C reported there are four bytes straddling the f32 fog distances 500.0
or 9,000.0 and 10,000.0 of `water.md` §5.2); 0x19 is TERRAIN_TYPE_TABLE, not TERRAIN_DESC. The
per-vertex grid after the height grid (`landblock.md` §3.7) selects scenery, not textures, and
the surface layers index the 0x19 table (`landblock.md` §3.8).

The record prologues (own DID first, or a u32 then the DID) are listed in `records.md` §1.6.

---

<a id="s5"></a>
## 5. Sky description and day descriptions

Conventions for §5 and §6: all integers little-endian; "f32" IEEE single; "f64" IEEE double;
"colour" a u32 described in §5.6; "weighted list" the structure of §5.1.3. Every count is
checked against the bytes left before allocating.

<a id="s5-1"></a>
### 5.1 Sky description 0x15000007 (DB type 0x3F; Confirmed, 1 of 1 record, 30,184 bytes)

The only 0x15 record. Slot 4 of every region descriptor names it (§3.2).

<a id="s5-1-1"></a>
#### 5.1.1 Top level

| Order | Size | Type | Field | Value in this install |
|---|---|---|---|---|
| 1 | 4 | u32 | `version` | 1. A reader rejects other values (BIN: the client refuses them). |
| 2 | 4 | DID | `did` | own DID |
| 3 | 8 | f64 | `light_interval_min` | 1.0 |
| 4 | 8 | f64 | `light_interval_max` | 1.0 |
| 5 | 8 | f64 | `sky_interval_min` | 180.0 |
| 6 | 8 | f64 | `sky_interval_max` | 180.0 |
| 7 | 4 | DID | `game_time` | 0x35000000 (§6) |
| 8 | 4 | u32 | `group_count` *g* | 4 |
| 9 | varies | | *g* day groups | §5.1.2 |
| 10 | varies | | named follow lists | §5.1.4 |
| 11 | varies | | named sky lists | §5.1.4 |
| 12 | 4 | u32 | `default_count` *m* | 540 |
| 13 | varies | | *m* sky entries (§5.1.3): the **local default** table | |

The two (min, max) pairs are update intervals in seconds of the game clock (BIN: the client
uses the midpoint of each pair: it recomputes the blended lighting every
½(`light_interval_min` + `light_interval_max`) seconds and re-checks the sky every
½(`sky_interval_min` + `sky_interval_max`) seconds, each divided by the clock's rate,
§11.1). Inferred names.

<a id="s5-1-2"></a>
#### 5.1.2 Day group

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 4 | u32 | `day_type` | A day-type value (enum mapper 0x230000E8, §2) |
| 2 | 4 | f32 | `chance` | Relative frequency of this day type (15, 2, 2, 1 in this install) |
| 3 | 1 | u8 | `follow_is_named` | 0 or 1 (a reader rejects ≥ 2) |
| 4 | 4 or varies | | follow | If `follow_is_named` = 1: u32 key of a named follow list (§5.1.4). If 0: an inline **follow list**: u32 `key` (0 here), then a weighted list body of (day type u32, weight f32) pairs (§5.1.3) |
| 5 | 4 | u32 | `entry_count` *n* | 208 – 236 |
| 6 | varies | | *n* sky entries | §5.1.3 |

The follow list gives the relative weights of the **next** day's type (all four groups list
the same four day types; for example after "FairFairFair": 15, 1, 1, 1). Inferred meaning
(BIN: the client links each group to its follow list and reports a missing one as an error).

<a id="s5-1-3"></a>
#### 5.1.3 Sky entry and weighted list

Sky entry:

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 4 | u32 | `sky_key` | An `Environment_LocalSky` value (§7.1); the local default table also has keys 1 ("DefaultIndoor") and 2 ("Default") of base mapper 0x230001B4 |
| 2 | 1 | u8 | `list_is_named` | 0 or 1 |
| 3 | varies | | list | If 1: u32 key of a named sky list (§5.1.4). If 0: an inline weighted list: u32 `key` (0 here) and a weighted list body of (DID u32, weight f32) pairs |

Weighted list body: u32 `count` *k*, then *k* pairs of (u32 value, f32 weight). In sky
entries the values are day-description DIDs (prefix 0x1A).

Census: every sky entry of the four day groups and of the local default table uses an
inline list (`list_is_named` = 0 everywhere); 539 of the 540 default entries and every group
entry hold exactly one DID; the default entry for key 2 holds two (weights 2.0 and 1.0). All other weights
are 1.0, except 10.0 on the group entries for key 2 and the default entry for key 1.

<a id="s5-1-4"></a>
#### 5.1.4 Named lists

Two tables follow the groups. Each starts with a count encoded as a **VLE** (`records.md`
§1.2; BIN-R: the client reads both counts with its VLE routine, the same one that reads the
counts of `water.md` §4) and then that many entries. The counts are 1 here (one byte).

| Table | Entry |
|---|---|
| Named follow lists | u32 `key`, weighted list body of (day type, weight). This install: one entry, key 0x10000003, pairs (0x10000007, 10.0), (0x10000002, 10.0) |
| Named sky lists | u32 `key`, weighted list body of (DID, weight). This install: one entry, key 0x10000001, pairs (0x1A00002B, 5.0), (0x1A000000, 5.0) |

No entry of this install refers to them by name (all `…_is_named` flags are 0).

<a id="s5-2"></a>
### 5.2 Day description (prefix 0x1A, DB type 0x3A; Confirmed, 963 of 963 records)

A day description is one weather variant of one sky: a list of **time-of-day keyframes**
and a list of **sky objects**.

| Order | Size | Type | Field | Census |
|---|---|---|---|---|
| 1 | 4 | u32 | `unknown0` | 0 in all 963 |
| 2 | 4 | DID | `did` | own DID |
| 3 | 4 | u32 | `keyframe_count` *n* | 1 (335 records), 2 – 21 (628); never 0 here |
| 4 | varies | | *n* keyframes | §5.3; times ascending in all 963 |
| 5 | 4 | u32 | `object_count` *m* | 0 – 21 (13 most common) |
| 6 | varies | | *m* sky objects | §5.4 |
| 7 | 4 | f32 | `transition_seconds` | 0.0 (803), 2.0 (70), 8.0 (54), 1.0 (18), 0.5 (18) |

`transition_seconds` (Inferred name, BIN use): when the active day description changes, the
client cross-fades from the old one to the new one over this many seconds if either value is
positive (§11.2 step 5).

<a id="s5-3"></a>
### 5.3 Keyframe (Confirmed layout; meanings BIN)

| Order | Size | Type | Field | Meaning | Census (7,590 keyframes) |
|---|---|---|---|---|---|
| 1 | 4 | f32 | `time` | Time of day, fraction 0 – 1 (§11.1) | 0.0 (652), 1.0 (627), 0.2, 0.31, 0.75, 0.275, 0.79, 0.225 most common |
| 2 | 4 | f32 | `fog_near` | Fog start distance from the eye, world units (`coordinates.md` §3: one world unit, by convention a metre; the same unit as every other distance in the spec set) | 200, 100, −100, 0, … (60 values) |
| 3 | 4 | f32 | `fog_far` | Fog end distance from the eye, world units | 2500, 1200, 1700, … (92 values) |
| 4 | 4 | f32 | `fog_extra` | A third fog value, 0 – 0.8 | 0.0 (5,082), 0.5 (884), … |
| 5 | 4 | colour | `fog_colour` | Fog colour | |
| 6 | 4 | f32 | `sun_brightness` | Directional light brightness | 0.55 (5,508), 0.0 (558), … |
| 7 | 4 | f32 | `sun_heading` | Degrees | 15 (5,128), 50 (1,571), 0, 90, … |
| 8 | 4 | f32 | `sun_pitch` | Degrees | 150 (5,030), 158 (1,568), 0, 50, … |
| 9 | 4 | colour | `sun_colour` | Directional light colour | |
| 10 | 4 | colour | `sun_specular` | Directional specular colour (grey in most keyframes) | |
| 11 | 4 | f32 | `ambient_brightness` | | 0.2 (5,539), 0.25, 0.3, 0.1, … |
| 12 | 4 | colour | `ambient_colour_1` | | |
| 13 | 4 | colour | `ambient_colour_2` | Equal to colour 1 in 5,510 keyframes; black in 318 | |
| 14 | 4 | colour | `colour_3` | Blended like the others; use OPEN | |
| 15 | 4 | u32 | `object_entry_count` *r* | | 0 – 21 |
| 16 | 20·*r* | | object entries | §5.3.1 | |
| 17 | varies | | four float curves | `material.md` §5.3 grammar; blended per curve; use OPEN | 7,589 / 7,590 / 7,520 / 7,548 of them mode 1 |
| 18 | 24 | 6 × f32 | `values_a` | OPEN | (0.033, 1.0, 2.0, 0.07, 0.6, 0.03) most common |
| 19 | 4 | f32 | `value_b` | OPEN; the client clamps the blended value to [0, 1] | 0.0 (7,231) |
| 20 | 4 | DID | `effect` | 0 (7,087) or a 0x47 object (503) | |
| 21 | 0 or 4 | u32 | `effect_extra` | Present only when `effect` ≠ 0; 0 in all 503 | |
| 22 | 4 | colour | `colour_4` | White (7,522), dark grey, pink | |

Field names are Tessera's. Meanings that rest on BIN: the client's keyframe blending routes
fields 2–5 into the fog state (near, far, extra, colour), fields 6–13 into the light state,
and builds the light direction from fields 7 and 8 (§11.3, `lighting.md` §8.1). Field 10:
for older data versions without it, the client derives a grey from the sun colour and
brightness, which supports the reading "specular colour".

`effect` (Inferred): a weather or precipitation effect object; it is taken from one of the
two blended keyframes, not interpolated (§11.3).

> **OPEN:** Meanings of `fog_extra`, `colour_3`, `colour_4`, the four curves, `values_a`,
> `value_b` and `effect`. Candidates from the renderer's settings vocabulary: sky glow,
> cloud cover and speed, precipitation density.

<a id="s5-3-1"></a>
#### 5.3.1 Keyframe object entry (20 bytes)

| Offset | Size | Type | Field | Census (107,213 entries) |
|---|---|---|---|---|
| 0 | 4 | u32 | `object_index` | a sky object's `index` (§5.4); 3,214 entries name an index that the record does not define and have no effect |
| 4 | 4 | DID | `replacement` | 0 in all |
| 8 | 4 | f32 | `angle_b_offset` | 0 in all |
| 12 | 4 | f32 | `angle_a_offset` | 0 in all |
| 16 | 4 | colour | `tint` | alpha 0 in 61,529 entries, 255 in 22,819, others between |

At the keyframe's time the object of that index is drawn with this tint (alpha = opacity:
stars and moon fade by alpha), with its angles offset and, if `replacement` ≠ 0, with that
object instead (BIN; offsets and replacements are unused in this install).

<a id="s5-4"></a>
### 5.4 Sky object (Confirmed layout; meanings BIN where stated)

| Order | Size | Type | Field | Census (12,788 objects) |
|---|---|---|---|---|
| 1 | 4 | u32 | `index` | 1 – 136, unique within a record |
| 2 | 4 | DID | `object` | 0x47 objects (12,781; 129 distinct), 0 (7) |
| 3 | 4 | f32 | `begin_time` | 0.0 in all |
| 4 | 4 | f32 | `end_time` | 1.0 in all |
| 5 | 4 | f32 | `angle_a_begin` | degrees: 0 (9,218), 90, −90, 345, … |
| 6 | 4 | f32 | `angle_a_end` | degrees: 0, 430, 270, 345, … |
| 7 | 4 | f32 | `angle_b_begin` | degrees: 0, 75, 120, −30, 137, … |
| 8 | 4 | f32 | `angle_b_end` | degrees: 0, 75, 120, 210, 137, … |
| 9 | 4 | u32 | `unknown_20` | 0 in all |
| 10 | 1 | u8 | `flag_25` | 0 in all |
| 11 | 1 | u8 | `flag_26` | 0 in all |
| 12 | 1 | u8 | `flag_30` | 0 in all |
| 13 | 4 | f32 | `value_28` | 0 in all (the client converts it to an integer) |
| 14 | 4 | f32 | `value_2c` | 0 in all (converted to an integer) |
| 15 | 1 | u8 | `reverse` | 0 in all; 1 = angles run downward (§11.4) |
| 16 | 4 | u32 | `layers_a` | 0x3F800000 (11,997) or 0x40C00000 (791); read as f32: 1.0 or 6.0 |
| 17 | 4 | u32 | `layers_b` | 1.0 (11,997) or 0.6 (791) as f32 |
| 18 | 4 | u32 | `layer_count` *q* | 0 (11,997) or 9 (791) |
| 19 | 12·*q* | | *q* layers of (f32, texture-list DID 0x40…, f32) | first floats −1.0 … 0.7 ascending |
| 20 | 1 | u8 | `layers_flag` | 1 (12,787), 0 (1) |
| 21 | 4 | f32 | `layers_value` | 0.06 in all |
| 22 | 1 | u8 | `flag_31` | 0 (11,974), 1 (814) |

All u8 flags are 0 or 1 (the client rejects other values). Fields 16–21 form one
sub-structure (BIN boundary); its meaning is OPEN (the 9-layer form, with one texture list per
layer keyed by a value from −1 to 1, fits a moon-phase or cloud-cover selection).

The `object` DIDs are physics-object records (prefix 0x47, `entity-descriptor.md` §4.3), the
same kind placed entities use; they resolve to the geometry drawn for the sky (domes, sun,
moon, cloud layers, stars).

<a id="s5-5"></a>
### 5.5 Coverage (Confirmed)

The day groups mention 238 sky keys and the local default table 540 (612 distinct keys in
all); together the tables name all 963 day descriptions. Of the 647 sky keys of the
0x1000xxxx form, 610 appear.

<a id="s5-6"></a>
### 5.6 Colours (BIN)

A colour is a u32 whose value is 0xAARRGGBB, stored little-endian, so the bytes in the
record are blue, green, red, alpha. Each channel divided by 255 gives a value in [0, 1].

---

<a id="s6"></a>
## 6. Game time 0x35000000 (DB type 0x02; Confirmed layout, 180 bytes)

| Order | Size | Type | Field | Value in this install |
|---|---|---|---|---|
| 1 | 4 | DID | `did` | own DID |
| 2 | 4 | f32 | `day_length` | 11,160.0 seconds (3 h 6 min) |
| 3 | 4 | u32 | `days_per_year` | 360 |
| 4 | 8 | f64 | `zero_offset` | 3,600.0 seconds |
| 5 | 4 | u32 | `zero_year` | 34 |
| 6 | 4 | f32 | `value_54` | 0.8 |
| 7 | 4 | f32 | `value_58` | 0.25 |
| 8 | 4 | u32 | `period_count` *a* | 4 |
| 9 | 8·*a* | | *a* × (f32 `start`, u32 `name`) | starts 0.0, 0.25, 0.5, 0.75 |
| 10 | 4 | u32 | `weekday_count` *b* | 1 |
| 11 | 4·*b* | | *b* × u32 `name` | |
| 12 | 4 | u32 | `month_count` *c* | 12 |
| 13 | 8·*c* | | *c* × (u32 `start_day`, u32 `name`) | start days 0, 30, 60, … 330 |
| 14 | 4 | u32 | `unknown_last` | |

`name` values are string tokens (resolution OPEN; they do not decode as DIDs). Field names are
Tessera's; fields 2–5 and the month table have BIN-confirmed roles (§11.1). The periods
(field 9) split the day into four named quarters by `start` (Inferred). `value_54` and
`value_58` are OPEN (a night boundary at 0.8 – 0.25 of the day is a candidate; the client
keeps a separate "is night" state).

---

<a id="s7"></a>
## 7. How the world chooses an environment (property hooks)

<a id="s7-1"></a>
### 7.1 Sky per landblock (Confirmed)

- Property `Environment_LocalSky` (enum mapper 0x230000E9, which inherits from base mapper
  0x230001B4; 650 labelled values, 647 of them of the form 0x1000xxxx). Labels are
  internal identifiers (area-and-mood names); read them at run time.
- **Block maps** carry it: every region's block-map descriptor (0x18) has a key 0x1000000F
  whose value list is `Environment_LocalSky` values (250 values for region 1, 194 for 2, 33
  for 3, 98 for 4/5, 6 for 6). So every landblock selects a sky through its block map
  (`landblock.md`).
- Overrides: 299 cell property sets, 193 landblock-info property sets and 147 stand-alone
  0x78 property sets (content-layer and similar sets) also carry `Environment_LocalSky`.
  Related enum properties: `Environment_PlayerLocalSkyOverride`,
  `Environment_LocalSky_HousingHook`.
- Path to visuals (Confirmed, §5.1, §11.2): sky key → sky entry of 0x15000007 (a day group's,
  else the local default table's) → weighted list → one 0x1A day description per game day.

<a id="s7-2"></a>
### 7.2 Interior fog and ambience (Confirmed census)

| Property | Type | Where (count) |
|---|---|---|
| `InteriorFogType` | enum 0x2300016A (Invalid 0, Linear 1, None 2) | cells (775), landblock info (541) |
| `InteriorFogNear`, `InteriorFogFar` | f32 | cells (775 each), landblock info (546, 540), 0x78 sets (7) |
| `InteriorFogColor` | colour | cells (775), landblock info (533), 0x78 sets (7) |
| `Environment_IndoorAmbiance` | enum 0x230000E9 (the sky enum) | cells (777), landblock info (40) |
| `EnvironmentType` | 64-bit bitfield | landblock info (3) |

Interiors therefore have linear fog with near/far distances and a colour, and choose an
"indoor ambiance" from the same enum as outdoor skies.

<a id="s7-3"></a>
### 7.3 Time of day (Confirmed values)

- Enum mapper 0x230001E4 (time-of-day values, 15 including Undef): Night, Dusk, Day, Dawn,
  Day_Mid, Night_End, Night_Early, Day_Late, Day_Early, Night_Late, Night_Start, and others.
- `TimeOfDay@Entity.m_eTimeOverride` (enum mapper 0x23000161: Night 1, Automatic 2,
  Never 3, Dusk 4, Always 5, Day 6, Dawn 7) on 42,429 static entities and 24 object property
  sets: entities that exist or light up only at certain times (lamps at night, for example).
- Lights carry day and night variants of their parameters
  (`Light.Script@Entity.Night.m_fLightRange`, `…Day.m_bLightActivated` and similar, on
  2,700 – 12,800 light entities each; `entity-descriptor.md` §4.5).
- `TimeOfDay_Control_*` properties (array of time structs, starting time) exist in the
  dictionary; no stored value was found in the places scanned.

<a id="s7-4"></a>
### 7.4 Other environment properties (Confirmed census)

| Property | Where (count) | Note |
|---|---|---|
| `WindSpeed` (enum 0x23000362: Undef, Medium, Low, High, Interior, LowMed) | static entities (18,334) | Foliage wind class |
| `Render_IsFogVolume` | static entities (173) | Local fog volumes |
| `Render_EtherealToPrecipitation` | 0x47 physics objects (e.g. invisible walls) | Precipitation passes through |
| `Landblock_UseCeiling` | block-map key 9 (two values per region) | Landblocks with a terrain ceiling (caves built from landscape, ER §3) |
| `Landblock_Temperature` | block-map key 0x10000011 (7 values per region) | OPEN (cold/heat gameplay or visuals) |
| `WaterHeight`, `WaterType`, `DistanceFogType`, `ForceSunlight`, `GlobalCamera_UnderWater` | defined in the dictionary; **no stored value** in landblock info, cells, entities, landblock properties or 0x78/0x79 sets | Runtime-only or unused. Water levels and types are block-map values (`water.md` §6). |

---

<a id="s8"></a>
## 8. Water (moved to `water.md`)

Water is specified in [`water.md`](water.md), which supersedes the first-look notes that
stood here. In short:

- Each region's **water table** (slot 5, prefix 0x16) lists water types; each landblock
  selects a type and a level through block-map keys 7 and 4, and has water when the level's
  height (level × 1000 / 255) is above its lowest terrain sample (`water.md` §6; 89,913
  landblocks of this install have water).
- Water surfaces are drawn as one quad per wet landblock with the type's surface record and
  the water shaders (`water.md` §8); placed water entities add ponds, pools and falls with
  their own meshes and liquid volumes (`water.md` §7.3).
- The slot-6 table (prefix 0x17) is the **fog-zone table**, read through block-map key 3; no
  zone is active in this install (`water.md` §5). It holds no texture list.

The properties `WaterHeight` and `WaterType` of §7.4 stay unused: water levels are block-map
values, not properties.

---

<a id="s9"></a>
## 9. Other prefixes seen while looking (kinds Confirmed, BIN-R (a))

| Prefix | Records | DB type | Observed | Spec |
|---|---|---|---|---|
| 0x02 | 3,402 (general) | 0x3C SCENE | u32 0, then own DID; placement nodes | `scenery.md` §5 (decoded) |
| 0x08 | 11 (general) | 0x0D CAMERASTATE | No own DID in the first 8 bytes; the first u32 is a 0x47-like DID, then floats (0.15, 0.1, 2000.0) | Layout OPEN (camera settings; not needed by the world) |
| 0x2D | 3 (general) | 0x62 COMPUTESHADER | A DXBC shader container | Not needed |
| 0x32 | 1 (general) | 0x3B RENDER_LOD_CLASSES | LOD-class distance table, decoded below | §9.1 |
| 0x39 | 11,155 (general) | 0x1E PSDESC | Particle-system descriptors; no own DID in the first 8 bytes | OPEN |
| 0x44 | 254 (general) | 0x56 FORKPSDESC | Texture file names and particle template labels; no own DID in the first 8 bytes | OPEN |

<a id="s9-1"></a>
### 9.1 LOD-class table 0x32000000 (Confirmed layout, 1 of 1, 1,160 bytes, zero leftover)

| Order | Size | Type | Field | Observed |
|---|---|---|---|---|
| 1 | 4 | DID | own DID | |
| 2 | 4 | u32 | `count` *n* | 32 |
| 3 | 36 · *n* | entry × *n* | `entries` | |

Entry: u32 `class` (a `Render_LODClass` value, enum mapper 0x2300008D), then eight f32
`d0` … `d7`. Observed: `d0` 25 – 4,000 (for example 500 for the class labelled
"FarView500", 1,000 for "Landmark", 25 for "SmallDecor"), `d1` 2 – 200, `d2` 2 – 100, `d3` 80
(26 once), `d4` 1,000, `d5` 1,000 (12 once), `d6` −1 (50 once), `d7` 1,100 (20 once).
Reading (Inferred): `d0` is the class's maximum draw distance in world units, the following
values distances of lower detail levels. The meaning of each field and how the client
applies it are OPEN (BIN not traced). `scenery.md` §9.1 uses this record for its LOD
classes and names the eight values `v1` … `v8` (= `d0` … `d7` here).

**Owner of the layout.** This section is the single statement of the 0x32 layout
(`records.md` §2.1 points here). The record's payload is **1,160 bytes**: own DID, `count`
32, and 32 entries of 36 bytes (4 + 4 + 32 × 36 = 1,160; OBS3, re-checked 2026-10-05).
There is no `length` field and no second copy of the DID in the payload.

> **Correction for `scenery.md` §9.1 and SC9 (for the scenery owner):** its table lists a
> `length` u32 (1,160) and a `did_echo` after the own DID and gives the record as 1,168
> bytes. Those 8 extra bytes are not in the record payload (they match a dump-file header
> of DID and length placed in front of it). The layout is the one above; SC9's check of
> `length` should be dropped, and the meaning of the values deferred to this section.

---

<a id="s10"></a>
## 10. Open questions (summary)

> **OPEN:** Region descriptor field 4 (offset 12, value 2), the kinds of maps 8 and 9 (empty
> here), and what keys other than 0 would select in the slot maps (§3.1); region number 6
> without a cell archive.

> **OPEN:** Layout of the encounter table, prefix 0x14 (§4). Not needed offline. (The other
> per-region tables are decoded in `scenery.md`, `terrain-texturing.md` and `water.md`.)

> **OPEN:** How the client sets the current **day type** (§11.2). It starts at 2, which
> matches no day group, so the local default table decides; whether a server message or a
> daily roll over the groups' `chance` and follow weights changes it is not known. Offline,
> Tessera uses the local default table, or optionally simulates day types from `chance` and
> the follow lists (Tessera Decision).

> **OPEN:** Keyframe fields `fog_extra`, `colour_3`, `colour_4`, the four curves, `values_a`,
> `value_b`, `effect` (§5.3); sky-object fields 9–14, 16–22 and the axes of the two angles
> (§5.4, §11.4); how two keyframes' tints and curves are blended exactly.

> **OPEN:** Game-time `value_54`, `value_58`, `unknown_last`, the string tokens of the
> period, weekday and month names, and the clock's epoch (§6, §11.1). The epoch decides
> which time of day the original shows at a given real time; it is set by the server
> connection, which is outside the offline client.

> **OPEN:** Sky rendering details: draw order, depth, scale and orientation of sky objects,
> the clear colour, and how fog applies to the sky (§12).

> **OPEN:** Layouts of 0x08, 0x39 and 0x44; the meaning of the LOD-class fields (§9).

---

<a id="s11"></a>
## 11. Algorithms (BIN unless stated)

<a id="s11-1"></a>
### 11.1 Game clock and time of day

State: a clock *G* in seconds (f64) and a rate *ρ* (1 by default; the client has a console
setting for it).

1. Each frame, *G* increases by the elapsed real time × *ρ*.
2. *T* = *G* + `zero_offset`.
3. Year length *Y* = `days_per_year` × `day_length`. Year index *i* = floor(*T* / *Y*); the
   year number is *i* + `zero_year`.
4. *r* = *T* − *i* · *Y*. Day of year *d* = floor(*r* / `day_length`).
5. **Time of day** *t* = (*r* − *d* · `day_length`) / `day_length`, a fraction in [0, 1).
   Keyframe times (§5.3) and sky-object times (§5.4) are on this scale.
6. **Game day number** *n* = `days_per_year` × (year number) + *d*.
7. Month index = the last month-table entry whose `start_day` ≤ *d*.

**Tessera Decision:** offline, Tessera chooses the epoch of *G* itself (for example from the
local clock, or a user-set time of day) because the original's epoch is OPEN.

<a id="s11-2"></a>
### 11.2 Choosing the day description

Inputs: the sky key *K* of the current landblock or cell (§7.1, including overrides), the
current day type *D* (OPEN; initially 2), the game day number *n*.

1. Look for a day group with `day_type` = *D*. If one exists and it has a sky entry with
   `sky_key` = *K* (first match in stored order), take that entry's weighted list.
2. Otherwise take the local default entry with `sky_key` = *K*.
3. If neither exists, there is no day description (the client then uses the defaults of
   §11.3 step 8, BIN); **Tessera Decision:** draw a fixed sky colour as well.
4. **Weighted pick.** Let *W* be the sum of the list's weights; if *W* ≤ 0 there is no pick.
   Compute *h* from *n* with 32-bit wrap-around arithmetic: multiply *n* by 0x11A28787,
   subtract 0x6B3C5F11, multiply by *n* again, subtract 0x1B87D114, and keep the top 16 bits
   of the 32-bit result. Let *x* = *h* / 65536 × *W*. Walk the pairs in stored order with a
   running sum of weights; pick the first pair whose running sum exceeds *x*; if none does,
   pick the last pair. (These constants are only needed to reproduce the original's choice of
   variant on a given game day; any deterministic per-day choice is acceptable otherwise.)
5. When the picked DID differs from the one in use, the client cross-fades from the old day
   description to the new one over the new description's `transition_seconds` if that is
   positive, else over the old one's. The cross-fade rule is OPEN; **Tessera Decision:**
   blend all §11.3 outputs linearly over that time.

The pick depends only on *n*, so a sky keeps its weather variant for a whole game day.

<a id="s11-3"></a>
### 11.3 Blending keyframes

Given the time of day *t* and a day description with keyframes *k*₀ … *k*ₙ₋₁ (ascending
`time`):

1. *a* = the first index *i* ≤ *n* − 2 with time(*kᵢ*) ≤ *t* ≤ time(*kᵢ₊₁*) (both comparisons
   inclusive); if none, *a* = *n* − 1.
2. *b* = the first index whose time is strictly greater than *t*; if none, *b* = 0.

   **Boundary behaviour (BIN-R (d), E14: these are the client's comparisons, confirmed
   2026-10-05).** The two searches are independent. When *t* equals the time of an interior
   keyframe *kⱼ* (0 < *j* < *n* − 1) and no earlier pair already contains *t*, step 1 stops at
   *a* = *j* − 1 (because time(*kⱼ*) ≤ *t* holds with equality) while step 2 skips *kⱼ* and
   finds *b* = *j* + 1. The client then blends *kⱼ₋₁* and *kⱼ₊₁* by
   (*t* − time(*kⱼ₋₁*)) / (time(*kⱼ₊₁*) − time(*kⱼ₋₁*)) and the values of *kⱼ* itself are not
   used at that exact instant; immediately before and after it the result is within rounding
   of *kⱼ*. An implementation that wants the original's output keeps these comparisons as
   they are; changing step 1 to a strict upper bound would differ from the original only at
   such instants (§14 has a worked example). Keyframe times that coincide exactly (several
   keyframes at 0.0, or at 1.0) are handled by the same rule.
3. Factor *f*, with *tₐ* = time(*kₐ*), *t_b* = time(*k_b*):
   - *tₐ* = *t_b*: *f* = 0;
   - *t_b* > *tₐ*: *f* = (*t* − *tₐ*) / (*t_b* − *tₐ*) when *tₐ* ≤ *t* ≤ *t_b*, else 0.5;
   - *t_b* < *tₐ* (wrapping past the end of the day): *u* = *t* − *tₐ*, plus 1 if negative;
     *s* = *t_b* − *tₐ* + 1; *f* = *u* / *s* (0 if *s* = 0).
4. Every scalar and every colour channel: value = *A* + *f* · (*B* − *A*), *A* from *kₐ*, *B*
   from *k_b*. The blended fog, sun, specular and ambient colours have alpha 1.
5. `sun_heading` and `sun_pitch` use **shortest-way angle interpolation** on a 360° circle:
   if going directly from *A* to *B* is shorter than going around, the result is
   *A* + *f* · (*B* − *A*) (not reduced); otherwise the client interpolates toward *B* ± 360
   and brings the result back into [0, 360).
6. The unit vector toward the sun is (sin *h* · cos *e*, cos *h* · cos *e*, sin *e*) with
   heading *h* and pitch *e* in degrees (world axes of `coordinates.md`, Z up). See
   `lighting.md` §8.1 for its use.
7. `effect` and `effect_extra` come from *kₐ* when `effect` ≠ 0 there, else from *k_b*.
8. With no day description the client uses the defaults of `lighting.md` §8.1, fog near 0,
   far 10,000, fog colour white.

<a id="s11-4"></a>
### 11.4 Animating sky objects

For each sky object at time of day *t*:

1. **Visibility.** Visible if `begin_time` = `end_time`; if `begin_time` < `end_time`, visible
   when `begin_time` ≤ *t* ≤ `end_time`; if `begin_time` > `end_time` (wrapping), visible when
   *t* < `end_time` or *t* > `begin_time`.
2. **Progress** *f*: as §11.3 step 3 with `begin_time` and `end_time` in place of the keyframe
   times. If `begin_time` = `end_time` the angles are the `…_begin` values.
3. **Angles A and B**, each from its (begin, end) pair: when `reverse` = 0 and begin > end,
   subtract 360 from begin; when `reverse` = 1 and begin < end, subtract 360 from end. Then
   angle = begin + *f* · (end − begin), plus 360 if negative. (In this install every sky
   object spans the whole day, 0 – 1, so angle A of, for example, −90 → 270 turns once per
   day.)
4. **Keyframe entries.** The entries for the object's `index` in keyframes *kₐ* and *k_b*
   (§11.3) add their angle offsets, may replace the object, and give its tint; the tint is
   blended between the two keyframes (Inferred: by the keyframe factor *f*). The tint's alpha
   is the object's opacity. An object without an entry in the current keyframes keeps its
   untinted look (Inferred).
5. Draw the object's geometry (§5.4 `object`) rotated by the two angles (axes OPEN, §12).

---

<a id="s12"></a>
## 12. Drawing the sky (Tessera Decision unless marked as a fact)

- Per frame: run §11.1, pick the day description with §11.2 when the sky key or the day
  changes (or every `sky_interval` seconds), and blend §11.3 every `light_interval` seconds
  (§5.1.1; 1 s in this install).
- Feed the blended sun, ambient and fog values to [`lighting.md`](lighting.md) §8.1 and §10.
- Draw the visible sky objects (§11.4) around the camera before the world, without writing
  depth, each tinted by its keyframe tint (alpha = opacity). Their geometry comes from the
  0x47 objects like any physics object (`entity-descriptor.md` §4.3). Which axes angles A and
  B rotate about, the scale and whether fog applies are OPEN; **Tessera Decision:** B is the
  azimuth of the object's arc and A the rotation along it.
- Interiors: cells with `InteriorFogType` "Linear" use their own fog values (§7.2) instead of
  the keyframe fog.

---

<a id="s13"></a>
## 13. Invariants

| # | Invariant | Status |
|---|---|---|
| EV1 | 0x15000007: `version` = 1; parses to the last byte with §5.1 | Confirmed |
| EV2 | Every 0x1A record parses to the last byte with §5.2–§5.4; keyframe times ascending; every u8 flag is 0 or 1; sky-object indices unique within a record | Confirmed, 963 of 963 |
| EV3 | Every DID in a sky-description weighted list resolves to a 0x1A record | Confirmed |
| EV4 | `effect_extra` is present exactly when `effect` ≠ 0 | Confirmed (by full parse) |
| EV5 | 0x35000000 parses to the last byte with §6; period starts ascending in [0, 1); month start days ascending | Confirmed |
| EV6 | Sky-object DIDs are 0 or prefix 0x47; keyframe `effect` DIDs are 0 or prefix 0x47 | Confirmed |

Hostile-input rules: reject unknown float-curve modes (their length is unknown), flags ≥ 2,
counts larger than the bytes left, and a sky-description `version` other than 1.

---

<a id="s14"></a>
## 14. Worked examples (synthetic or computed from the formulas)

**Clock.** *G* = 0 with this install's game time: *T* = 3,600; *Y* = 360 × 11,160 =
4,017,600; *i* = 0, year 34; *d* = 0; *t* = 3,600 / 11,160 = 0.3226; *n* = 360 × 34 + 0 =
12,240.

**Weighted pick.** For *n* = 12,240 the hash gives *h* = 39,996, so *h* / 65536 = 0.6103. A
list with weights (2, 1) has *W* = 3 and *x* = 1.831; the running sums are 2 and 3; 2 > 1.831,
so the first DID is picked. For *n* = 12,241, *h* = 39,042, *x* = 1.787: again the first.

**Keyframe factor.** Keyframes at 0.2 (fog far 1,200) and 0.31 (fog far 2,500), *t* = 0.25:
*f* = 0.05 / 0.11 = 0.4545; fog far = 1,200 + 0.4545 × 1,300 = 1,790.9.

**Keyframe boundary (§11.3, the client's behaviour).** Keyframes at times 0, 0.2, 0.31 and 1.0
with fog far 1,000, 1,200, 2,500 and 1,000:

| *t* | *a* | *b* | *f* | fog far |
|---|---|---|---|---|
| 0.19999 | 0 | 1 | 0.99995 | 1,199.99 |
| 0.2 exactly | 0 (0 ≤ 0.2 ≤ 0.2) | 2 (first time > 0.2) | 0.2 / 0.31 = 0.6452 | 1,000 + 0.6452 × 1,500 = 1,967.7 |
| 0.20001 | 1 | 2 | 0.00009 | 1,200.12 |

**Angle.** Heading 350 → 10 with *f* = 0.5: the direct way (340°) is longer than the wrap
(20°), so the result is 350 + 0.5 × 20 = 360, reduced to 0.

**Sun direction.** Heading 15, pitch 150 (the most common keyframe values): toward-sun vector
(sin 15° · cos 150°, cos 15° · cos 150°, sin 150°) = (−0.224, −0.837, 0.5).

**Sky object.** Angle A from −90 to 270, `reverse` 0, *t* = 0.25 over 0 – 1: *f* = 0.25,
A = −90 + 0.25 × 360 = 0.

---

<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-04 | First look (§0–§10). |
| 2026-10-05 | §1 provenance extended (OBS2, BIN). §2: prefix ↔ type pairings for 0x15, 0x1A, 0x35; day-type and sky-key mappers. §5 rewritten: sky description (§5.1), day description (§5.2), keyframe (§5.3), sky object (§5.4), coverage (§5.5), colours (§5.6), all Confirmed to the last byte; the first-look readings of §5.2 (keyframe count at offset 8, fog distances at 16 and 20) are confirmed and extended. §6 rewritten with the full game-time layout. §10 updated. New §11–§15. §3, §4, §7, §8, §9 unchanged. |
| 2026-10-05 (second revision) | Sweep against `scenery.md`, `terrain-texturing.md` and the new `water.md`, with new BIN facts (BIN-R, OBS3, E11–E15). §2: complete prefix ↔ DB type table for this spec's records (from the client's DID-range registration; full table in `records.md` §2.1). §3 rewritten: the region descriptor's full grammar (fixed fields, nine DID maps, trailing terrain-type-table DID), offset 8 is a map-image DID (not an f32), offsets 16–32 are the map-coordinate parameters with the client's formula, version rules. §4 rewritten: every per-region table's kind and the spec that decodes it; the first-look hypotheses for 0x11, 0x12, 0x16, 0x17 and 0x19 corrected (0x17 holds no texture list). §5.1.4: the named-list counts are VLEs (OPEN closed). §7.4: water note. §8 replaced by a pointer to `water.md`. §9: kinds of 0x02, 0x08, 0x2D, 0x32, 0x39, 0x44; new §9.1 LOD-class table. §10 updated. §11.3: the keyframe-search comparisons confirmed by BIN, boundary behaviour stated; §14 boundary example. Mark "Tessera Decision" added to the legend. |
| 2026-10-05 (third revision, after review) | Status Draft → Review. §1: source tag LC defined (cited by E13 and §3.1 field 3). §5.3: `fog_near` and `fog_far` in world units (one unit is by convention a metre, `coordinates.md` §3). §9.1: stale pointer to `scenery.md` §9 replaced by a cross-reference; this section named the single owner of the 0x32 layout (payload 1,160 bytes: DID, count, 32 × 36 bytes; re-checked on the install); correction note for `scenery.md` §9.1 and SC9 (`length`, `did_echo` and the 1,168 figure to be removed). |
