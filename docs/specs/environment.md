<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Environment: regions, sky, time of day, fog, weather and water (first look)

| | |
|---|---|
| **Status** | Draft (first look; mostly OPEN). Not required for the M3 exit criteria. |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | Later: `crates/formats` (region descriptor and environment records), `crates/world` (per-landblock sky selection), `crates/render` (sky, fog, time-of-day lighting, water). For M3 only §8's "what to do meanwhile" applies. |
| **Companion specs** | `records.md` §2.1 (prefix table this spec refines), §7 (enum mappers). `property-system.md` §8 (DID maps), §9 (DB-object-type codes). [`landblock.md`](landblock.md) (block maps, which carry the per-landblock sky key). `entity-descriptor.md` (static entities: lights, time-of-day overrides, water meshes). `material.md` §8.7 (water and other effect parameters). |

---

<a id="s0"></a>
## 0. Purpose and status

The roadmap asks for a first look at sky, environment, weather and water records (M3 RE
task, "feeds an environment.md with OPEN items"). This spec records:

- the vocabulary of environment-related record kinds that the data itself names (§2);
- the **region descriptor** (prefix 0x0F) and the per-region tables it points at (§3, §4);
- the shared **sky table** 0x15000007 and the 963 **environment descriptors** (0x1A) it
  selects (§5);
- a game-time record (0x35) (§6);
- the properties through which landblocks, cells and entities choose a sky, fog, ambience
  and time-of-day behaviour (§7);
- what is known about water (§8);
- everything else as OPEN (§9).

Layouts here are **Observed** first looks (byte positions in a handful of records, counts over
all records of a prefix), not full decodes. Nothing in this spec is Confirmed to the
standard of `entity-descriptor.md` unless marked so.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 by throwaway probes in the dirty-room scratchpad: every record of prefixes 0x0F, 0x11, 0x12, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x32, 0x35 read; DID-like values located by scanning; block-map descriptors 0x18 fully decoded; property sets of all landblock-info records, cells, SEDs, 0x804 landblock properties and all 0x78/0x79 property sets scanned for the properties of §7. |
| PS | `property-system.md` §8–§9 (DID maps; DB-object-type mapper "DBOType"). |
| DS | `research/notes/data-semantics.md` §6.5 (block maps), §13 (environment, sky, weather and water listed as unknown; no public decoder). |
| ER | `research/notes/engine-client-re.md` §3 (renderer settings such as atmospherics, precipitation, interactive water, landscape shore effects), §4 (map coordinates). |

| Mark | Meaning |
|---|---|
| Confirmed | Checked over all records of the kind |
| Observed | Seen in the records named; layout not decoded end to end |
| Hypothesis | A reading that fits the observations but is not proven |

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

The code is **not** the DID prefix (for example RENDERSURFACE is code 0x05 but prefix 0x41),
and no record found so far maps codes to prefixes. The prefix ↔ kind pairings in §3–§6 are
hypotheses drawn from content.

---

<a id="s3"></a>
## 3. Region descriptor (prefix 0x0F, `client_general.dat`; Observed, 7 of 7 records)

The REGION DID map (root key 0x11, sub-map 0x28000008, `property-system.md` §8.1) labels
eight 0x0F DIDs; seven exist (0x0F000000, 01, 02, 05, 06, 08, 09; the eighth target,
0x0F000007, does not). Every record is 114 bytes and starts with its own DID.

<a id="s3-1"></a>
### 3.1 Layout (Observed in all 7)

| Offset | Type | Observed values | Reading |
|---|---|---|---|
| 0 | DID | own DID | |
| 4 | u32 | 1, 2, 3, 4, 14, 5, 6 | **Landblock region number** *R* (`records.md` §2.2). 0x0F000006 → 14; 0x0F000009 → 6 (no `client_cell_6.dat` exists in this install). |
| 8 | f32 | 8.0 (region 1), 8.85 (others) | OPEN |
| 12 | u32 | 2 | OPEN |
| 16 | u32 | 1468 | Map-coordinate origin X in 20 m units (1468 × 20 = 29,360 m; ER §4, `coordinates.md`) (Hypothesis, strong) |
| 20 | u32 | 1244 | Map-coordinate origin Y in 20 m units (1244 × 20 = 24,880 m) (Hypothesis, strong) |
| 24 | u32 × 2 | 0, 0 | |
| 32 | f32 | 0.005 | Map-coordinate scale per metre (ER §4: one map unit per 200 m) (Hypothesis, strong) |
| 36 | 7 slots of 10 bytes | u8 1, u8 1, u32 0, DID | Table DIDs at offsets 42, 52, 62, 72, 82, 92, 102 |
| 106 | 4 bytes, then DID | 01 00 01 00, DID | Eighth table DID at offset 110 |

If the hypotheses for offsets 16–32 hold, the map coordinates of `coordinates.md` should be
read from here at run time rather than hard-coded.

<a id="s3-2"></a>
### 3.2 Table DIDs per region (Observed)

| Region descriptor | *R* | Slot 1 (0x11) | Slot 2 (0x12) | Slot 3 (0x14) | Slot 4 (0x15) | Slot 5 (0x16) | Slot 6 (0x17) | Slot 7 (0x18) | Slot 8 (0x19) |
|---|---|---|---|---|---|---|---|---|---|
| 0x0F000000 | 1 | 0x11000000 | 0x12000000 | 0x14000000 | 0x15000007 | 0x16000000 | 0x17000000 | 0x18000000 | 0x19000000 |
| 0x0F000001 | 2 | 0x11000001 | 0x12000001 | 0x14000001 | 0x15000007 | 0x16000004 | 0x17000001 | 0x18000014 | 0x19000001 |
| 0x0F000002 | 3 | 0x11000002 | 0x12000002 | 0x14000002 | 0x15000007 | 0x16000005 | 0x17000002 | 0x18000015 | 0x19000002 |
| 0x0F000005 | 4 | 0x11000007 | 0x12000007 | 0x14000007 | 0x15000007 | 0x1600000A | 0x17000007 | 0x1800001A | 0x19000007 |
| 0x0F000006 | 14 | as region 2 | | | | | | | |
| 0x0F000008 | 5 | as region 4 | | | | | | | |
| 0x0F000009 | 6 | 0x1100000A | 0x1200000A | 0x1400000A | 0x15000007 | 0x1600000E | 0x1700000A | 0x1800001E | 0x1900000A |

Slot 7 is the block-map property descriptor (DS §6.5, `landblock.md`); its values match the
per-region descriptors reported in DS. So the region descriptor is how a client finds a
region's block-map descriptor and the other per-region tables, and the region-number ↔
descriptor mapping is data, not code.

---

<a id="s4"></a>
## 4. Per-region tables (Observed; kinds are hypotheses)

Five records exist for each of the prefixes 0x11, 0x12, 0x14, 0x16, 0x17, 0x19 (serials 0, 1,
2, 7, 10 except 0x16: 0, 4, 5, 10, 14), one per distinct region set of §3.2.

| Prefix | Sizes (bytes) | Starts with | What is in it | Hypothesis |
|---|---|---|---|---|
| 0x11 | 1,802 – 19,613 | u32 0, then own DID (**not** DID first) | Mostly small integers and f32 1.0; no DID references found | OPEN (LAND_BLOCK_TERRAIN_LIST or SCENE_DESC) |
| 0x12 | 882 – 10,934 | own DID | A list of named entries (4 – 98 ASCII labels per record; the first is the label "Default"), each followed by integer lists; some u32 values coincide with sky keys (§7.1), not proven to be references | Named environment or terrain zones |
| 0x14 | 58 (four records), 1,687 (region 1) | own DID | An ASCII label for "no encounter", then small integer lists | ENCOUNTER_DESC (Hypothesis, strong) |
| 0x16 | 188 – 1,574 | own DID | Header u32 0, 4, 7, 1, then 33-byte elements each naming a 0x31 surface descriptor (5 – 47 per record, all distinct) | Terrain type → surface palette (TERRAIN_DESC or TERRAIN_TYPE_TABLE) |
| 0x17 | 137 (four records), 1,244 (region 1) | own DID | Small integers and one 0x40 texture list (0x400043FA in four records, 0x4000460C in region 1's) | WATER_DESC (a water normal-map texture) or FOG_DESC |
| 0x19 | 40,637 – 173,939 | own DID | 317 – 1,422 ASCII layer labels (terrain material layers such as grass, scree, cliff faces) and 820 – 2,681 references to 0x40 texture lists (194 – 637 distinct) | Terrain texture layers for splatting (TERRAIN_DESC) |

> **OPEN:** Full layouts of all six. 0x16 and 0x19 are the most valuable for M3 terrain
> splatting (`landblock.md`: the bytes after the height grid are the per-vertex terrain
> types these tables would index).

> **Correction for `records.md` §1.6:** not every record starts with its own DID. Prefixes
> 0x11, 0x15, 0x1A and 0x02 start with a u32 (0, or 1 for 0x15) followed by the DID.
> `records.md` §2.1 should mark 0x0F as the region descriptor with this layout and 0x11,
> 0x12, 0x14, 0x16, 0x17, 0x19 as per-region tables named by it.

---

<a id="s5"></a>
## 5. Sky selection table and environment descriptors

<a id="s5-1"></a>
### 5.1 Shared sky table 0x15000007 (Observed)

The only 0x15 record (30,184 bytes) is named by slot 4 of every region descriptor. It starts
with u32 1 and its DID, then four f64 (1.0, 1.0, 180.0, 180.0), then the DID 0x35000000
(§6), then a long sequence of entries that pair **sky keys** (values of the
`Environment_LocalSky` enum, §7.1) with **0x1A environment descriptors**, each followed by an
f32 1.0 (a weight is likely). The entries are not fixed-size (gaps between consecutive
references are 8 and 13 bytes in most places).

Coverage (Confirmed by scanning): it mentions 610 of the 647 sky keys of the 0x1000xxxx form
and **all 963** 0x1A records.

Hypothesis: for each sky key, a weighted list of environment descriptors; the weights
select a weather variant (clear, cloudy, rainy …) for that sky.

> **OPEN:** Exact entry grammar, the meaning of the four doubles (a latitude/longitude-like
> 180.0 suggests sun-path parameters), and how weights and time of day pick a descriptor.

<a id="s5-2"></a>
### 5.2 Environment descriptors (prefix 0x1A, 963 records; Observed)

DIDs 0x1A000000 – 0x1A000413 (sparse); 136 – 9,700 bytes.

| Offset | Type | Observed | Reading |
|---|---|---|---|
| 0 | u32 | 0 in all 963 | |
| 4 | DID | own DID in all 963 | |
| 8 | u32 | 1 (335 records), 11 – 13 (486), others up to 21 | A count; keyframes per day is likely (the time-of-day enum of §7.3 has 14 named values) |
| 12 | u32 | 0 | |
| 16 | f32 | e.g. 200, 50, 1000, 100, 250 | Fog start distance (Hypothesis) |
| 20 | f32 | e.g. 1200, 2500, 350, 512 | Fog end distance (Hypothesis): greater than the value at 16 in 962 of 963 records |
| 24 … | mixed | Small integers, f32 such as 0.55, 15.0, 150.0, 0.2, and packed RGBA colours (bytes ending in 0xFF) | Per-keyframe colours (sky, fog, ambient, sun) and intensities (Hypothesis) |

These are the best candidates for the DAY_DESC / SKY_DESC / FOG_DESC vocabulary of §2.

> **OPEN:** Complete layout; which colours are sky gradient, fog, ambient and sun light; how
> keyframes map to times of day; which textures or meshes draw the sky dome, clouds, sun,
> moon and stars (no reference to a 0x06 mesh or 0x40 texture list was found inside 0x1A
> records).

---

<a id="s6"></a>
## 6. Game-time record 0x35000000 (Observed)

180 bytes, own DID first, named by the sky table (§5.1). Readable values: f32 11,160.0 at
offset 4, u32 360 at offset 8, f64 3,600.0 at offset 12, then u32 34, f32 0.8, f32 0.25,
u32 4, and a list of (u32, u32) pairs whose second members step by 30 (30, 60, 90 … 300).

Hypothesis: game-time constants. 11,160 s (3 h 6 min) as the length of a game day and 360
as days per year are the natural readings; the 30-step list looks like a calendar.

> **OPEN:** Verify against the original's `/time` output over a real-time interval
> (ER §4: the game timestamp equals `Turbine.Engine.GetGameTime()`), then specify.

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
- Path to visuals (Hypothesis): sky key → 0x15000007 entries → 0x1A descriptor(s).

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
| `WaterHeight`, `WaterType`, `DistanceFogType`, `ForceSunlight`, `GlobalCamera_UnderWater` | defined in the dictionary; **no stored value** in landblock info, cells, entities, landblock properties or 0x78/0x79 sets | Runtime-only or unused |

---

<a id="s8"></a>
## 8. Water (Observed; mostly OPEN)

- No water level is stored in the terrain records examined so far, and the water properties
  of §7.4 carry no stored values.
- Water is drawn by **placed geometry**: about 40,000 Physics static entities carry
  world-building labels naming water features (lakes, rivers, ponds, ocean; about 15,800 of
  them waterfalls), resolving through at least 432 distinct physics objects. Their meshes use water
  surfaces (`material.md` §8.7 lists the water effect parameters; 98 surface records).
- Each region's slot-6 table (0x17, §4) names one texture list, a candidate for a shared
  water normal map.

What to do meanwhile (M3): draw water entities like any other Physics entity, with the
material pipeline's fallback for unsupported effect families (`material.md`). Use a fixed
sky colour and distance fog; the per-landblock sky key (§7.1) can already be shown in a
debug overlay.

> **OPEN:** Water rendering parameters (`material.md` §8.7 effect family), shorelines
> ("landscape shore effects" setting, ER §2), underwater detection, and whether any terrain
> record holds water levels (the 0x805RXXYY records, `records.md` §2.2, are unexplored).

---

<a id="s9"></a>
## 9. Other unknown prefixes seen while looking

| Prefix | Records | Observed | Hypothesis |
|---|---|---|---|
| 0x02 | 3,402 (general) | u32 0, then own DID; 22 – 2,600 bytes; floats and packed colours | OPEN (MATERIALMODIFIER or a render-state kind) |
| 0x08 | 11 (general) | First u32 is a 0x47 DID-like value, then floats (0.15, 0.1, 2000.0) | OPEN |
| 0x2D | 3 (general) | Contain a DXBC shader container | COMPUTESHADER (Hypothesis, strong) |
| 0x32 | 1 (general) | Names 0x0F000000 and two texture lists; floats 500, 100, 80, 1000 | OPEN (distant landscape / imposter settings) |
| 0x44 | 254 (general) | ASCII texture file names and particle template labels | FORKPSDESC particle systems (Hypothesis, strong) |
| 0x39 | 11,155 (general) | (`property-system.md` §9: particle-system descriptor) | PSDESC |

---

<a id="s10"></a>
## 10. Open questions (summary)

> **OPEN:** Region descriptor fields at offsets 8 and 12 and the slot flag bytes (§3.1);
> region number 6 without a cell archive.

> **OPEN:** Layouts of the per-region tables 0x11, 0x12, 0x14, 0x16, 0x17, 0x19 (§4); 0x16
> and 0x19 first, for terrain splatting.

> **OPEN:** Sky table entry grammar and weather selection (§5.1); 0x1A descriptor layout and
> keyframe timing (§5.2); sky dome, cloud, sun, moon and star assets.

> **OPEN:** Game-time constants (§6), checked against the original's clock.

> **OPEN:** Water levels and water rendering (§8).

> **OPEN:** Prefixes 0x02, 0x08, 0x32 (§9).
