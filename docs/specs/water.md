<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Water: water tables, fog-zone tables, landblock water, water volumes and drawing water

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), review 2026-10-05: changes requested (first round; addressed). Re-review 2026-10-05: changes requested (§8.4 step 6: the specular sentence is broken by the inserted *L* text, so "plus a sparkle term" dangles and it is not stated where the sparkle term is added; §8.2 permutation table: "position only (one vertex input)" is true of the programs' use, but the Direct3D 11 water permutations declare position, normal and UV in their input signature and read only position). Spot checks passed: W2, W3, W16 (55 users of 0x2B0000A8, 43 water; 13 permutations: 8 water, 4 Direct3D 11 with depth and 2 with ripple, ONE/ZERO without depth, SRCALPHA/INVSRCALPHA with depth, cull NONE; 5 low-detail with Color/Color2, cull CW), the 43/21/5/5/1/1 shader split. Second round addressed 2026-10-06 (change log). Third review 2026-10-06, Claude review session (spec-writer agent): **passed, Accepted.** All §5.3 items hold. §8.4 step 6 was checked against the OpenGL water pixel program and all four Direct3D 11 water pixel programs of 0x2B0000A8: the per-component clamp of the normalised tangent-space normal minus the threshold, the factor 4, the sum added after the exponent, the half vector normalize(*v* − *L*) and the toward-eye *v*. §8.2 and §8.3 were checked too: the four Direct3D 11 water vertex programs declare position, normal and UV in their signature but have only the position as a declared input register. Five Confirmed facts were spot-checked on freshly read data and all passed. W2: 5 of 5 tables, zero bytes left over, 125 entries (47/19/23/31/5), keys 4 and 7, volume entities 120/3/2, `value` 24.0, 76 surfaces. W3: 137 and 1,244 bytes, zero left over, zone key 3, `active` 0 in the defaults and 1 in all 9 zones. W15 and WA4: keys 3, 4 and 7 are in all 390,150 block maps, and key 3 is 0 in all of them. W8 and the §6.1 and §6.4 tables: 89,913 wet landblocks, with every per-region count equal. W16: 13 permutations with their render-state bytes. Advisory note for a later revision, not blocking: the Direct3D 11 depth permutations tint the refraction by a depth difference instead of the eye-distance shoreline blend of step 3, and they scale the output alpha by depth. This falls under the existing OPEN on depth permutations, and the Tessera rule avoids it. |
| **Implements into** | `crates/formats` (decoders for the water table, prefix 0x16, and the fog-zone table, prefix 0x17; one fuzz target each), `crates/world` (per-landblock water state, water volumes, "is under water" query), `crates/terrain` or `crates/render` (water quads per landblock), `crates/render` (water pipeline), the movement controller of `collision.md` §12 (water states), `apps/explorer` (water table and per-landblock water view) |
| **Companion specs** | [`environment.md`](environment.md) §3 (region descriptor: slots 5 and 6 name the two tables). [`landblock.md`](landblock.md) §3.4 (height conversion), §5 (block maps; keys 3, 4 and 7 are read here), §7 (distant-landscape sector records). [`material.md`](material.md) §4 (surface records 0x31), §5–§8 (shader records 0x2B, parameter entries, labels; §8.6: `c_`-labelled parameters feed the program constant of the same name). [`collision.md`](collision.md) §9–§12 (volumes, movement states, controller). [`scenery.md`](scenery.md) §7.4–§7.5 (height modes and water ranges that read the water plane). [`entity-descriptor.md`](entity-descriptor.md) (placed static entities). [`property-system.md`](property-system.md) §5 (0x78 property sets), §7 (enum mappers). [`records.md`](records.md) §1 (VLE), §2.1 (prefix → DB type table). |

---

<a id="s0"></a>
## 0. Purpose

This spec is the single place that describes water. With it an implementer can:

- decode the region's **water table** (prefix 0x16) and **fog-zone table** (prefix 0x17)
  to the last byte (§4, §5);
- decide, for every outdoor landblock, whether it has **landblock water**, its water type and
  its surface height, exactly as the original client does (§6);
- place the **water volumes** that turn a mover into a wading or swimming one (§7);
- draw the water surface with the original's material chain, geometry and shading inputs
  (§8), including distant water (§9);
- answer the questions other systems ask about water: scenery height checks, the encounter
  pass, "is this point under water" (§10).

It also records which statements in other specs about water were wrong (§13).

All counts are observations of one install on one date and change with patches. Tests assert
the invariants (§11), not the counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install (Steam) on 2026-10-05 by throwaway probes in the dirty-room scratchpad: all 5 records of prefix 0x16 and all 5 of prefix 0x17 decoded to the last byte; all 7 region descriptors; all 390,150 block maps (every key and value) and the minimum and maximum of all 390,150 height grids of regions 1–5 and 14; all 33,159 distant-landscape sector records (kind 5) searched for their water pairs; all 3,244,086 placed static entities of all 49,636 landblock-info records (object DID, position, world-building label); all 40,643 entity descriptors (0x47) and all 1,616 stand-alone property sets (0x78) checked for liquid-volume properties; the 76 surface records (0x31) named by the water tables and their shader records (0x2B). |
| BIN | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read with objdump and capstone in the dirty room on 2026-10-05. **Questions:** (a) which DB object type the client registers for each DID range (to settle what prefixes 0x16 and 0x17 are); (b) the deserialisers of the water table, the fog-zone table and the region descriptor; (c) how a landblock decides it has water: which block-map keys, how a stored level becomes a height, the presence rule, the plane; (d) who consumes that state: the water-volume entity, the "under water" test, the scenery and encounter passes; (e) how water geometry is built for drawing; (f) how the fog-zone table is looked up. **Why M1–M3 were not enough:** block-map keys 3, 4, 6 and 7 have no descriptor entry in the data (`landblock.md` §5.6), so nothing in the data says which table reads them or how a stored value becomes a height; the earlier specs disagreed about which slot is water (§13); no published source describes these records; no running client is available to this session for M3. Only offline world code was read: nothing about network, login, keys, launcher or anti-tamper. |
| SH | The water shader records (prefix 0x2B) of the same install carry the OpenGL back end's programs as text. Those of shader record 0x2B0000A8 (used by 43 of the 76 water surfaces) were read in the dirty room (method M1, data files) and reduced to the words of §8.3–§8.4; the sampler lists of the other five water shader records were read. No program text is reproduced. |
| LC | LotroCompanion `lotro-tools` (`RegionLoader`, readable source; facts only): names region slot 5 "water" and slot 6 "fog". Reported only; confirmed here by BIN. |
| ENV, LB, COL, SCN, MAT | `environment.md`, `landblock.md`, `collision.md`, `scenery.md`, `material.md` (same install). |

Marks: **Confirmed** = checked on every record of the kind in this install on the date given
(for BIN facts: the data agrees on every record); **BIN** = rests on the disassembly only;
**Probable** = supported by BIN and data together but not proven for every case; **Reported**
= published sources only; **Inferred** = a conclusion drawn from Confirmed or BIN facts that
no probe or listing has shown directly (the reasoning is given beside it; implementers treat
it like a Tessera Decision until it is confirmed); **Tessera Decision** = Tessera's own rule
where the original's is not known (never a statement about the original).

| # | Fact | Sources | Mark |
|---|---|---|---|
| W1 | Region slot 5 names the water table (prefix 0x16, DB type 0x17 WATER_DESC); slot 6 names the fog-zone table (prefix 0x17, DB type 0x16 FOG_DESC) (§3) | BIN (a), (b), (c), (f); LC; OBS | Confirmed |
| W2 | Water-table layout (§4.1–§4.2) | BIN (b); OBS (5 of 5, zero leftover bytes, 125 entries) | Confirmed |
| W3 | Fog-zone-table layout (§5.1–§5.2) | BIN (b); OBS (5 of 5, zero leftover bytes) | Confirmed |
| W4 | Roles of the water-entry fields: surface record, volume entity; four further DIDs and a float unused by this install (§4.2) | BIN (c), (d), (e); OBS | Confirmed for surface and entity; the rest OPEN |
| W5 | The water table names the block-map keys it reads: level key at record offset 8, type key at offset 12; the block map's raw 16-bit value is used, no 0x18 palette (§6.1) | BIN (c); OBS (keys 4 and 7 in every block map; no 0x18 descriptor lists them) | Confirmed |
| W6 | Water height = level × 1000 / 255 world units, computed in 32-bit floats (§6.2) | BIN (c); OBS (§6.5, §9) | Confirmed |
| W7 | A landblock has water exactly when its type selects an entry that is not "dry" and the height is strictly above the block's lowest terrain sample (§6.2) | BIN (c); OBS (the distant-sector water tables of all 12,925 sectors with wet landblocks contain every pair this rule predicts, §9) | Confirmed |
| W8 | 89,913 of 390,150 landblocks have water (§6.4) | OBS (every block map and height grid) | Confirmed |
| W9 | A point is under landblock water when its height is more than 0.0002 below the surface (§6.3) | BIN (d) | BIN |
| W10 | Each wet landblock creates the water type's volume entity at block-local (0, 0, *h*); its three monitor boxes define surface, wading and deep zones (§7.1) | BIN (d) (placement); OBS (properties of the three volume entities) | Placement BIN; boxes Confirmed |
| W11 | 3,895 placed static entities (37 distinct 0x47 records) carry liquid volumes of their own (§7.3) | OBS | Confirmed |
| W12 | Landscape water is drawn as one horizontal 160 × 160 quad per wet landblock at its water height, gathered per distant-landscape sector and water type (§8.1, §9) | BIN (e) | BIN; the only landscape-water geometry builder found (Probable that it serves all distances) |
| W13 | Water texture coordinates come from the world position, block units (1/160), per-layer scale and scroll speed; three normal-map layers; vertex waves; Fresnel mix of refraction and reflection (§8.3–§8.4) | SH (program text of 0x2B0000A8); MAT §8.0 (labels) | Confirmed for 0x2B0000A8 (formula in data); other water shaders Probable |
| W14 | Fog-zone values are looked up at the landblock and its east, north and north-east neighbours; only entries flagged active are used (§5.3) | BIN (f) | BIN |
| W15 | Block-map key 3 is 0 in every landblock, so no fog zone is active anywhere in this install (§5.4) | OBS | Confirmed |
| W16 | Shader record 0x2B0000A8 is named by 55 surface records (0x31), 43 of them water surfaces of the tables; its water permutations take positions only and composite the frame-buffer copy, its low-detail permutations sample two colour layers and are lit per vertex by ambient, hemispheric ambient and the per-vertex lights (§8.2) | OBS (all surface records, 2026-10-05); SH (sampler lists, render-state bytes and constant lists of every permutation of 0x2B0000A8) | Confirmed |
| W17 | The specular direction of §8.4 step 6 is the main light's travel direction when the main light is directional, else an engine default direction (§8.4) | `lighting.md` §4.2, row L24 (BIN-L (g)) | BIN |

---

<a id="s2"></a>
## 2. Where water comes from

| Source | Data | Drawn by | Volumes (swimming) | § |
|---|---|---|---|---|
| **Landblock water** (seas, large lakes and rivers at a flat level per landblock) | Region water table (0x16) + block-map keys 7 (type) and 4 (level) + the height grid's minimum | One quad per wet landblock (§8.1) with the type's surface record (0x31) | The type's volume entity, placed per wet landblock (§7.1) | §4, §6–§8 |
| **Placed water** (ponds, pools, water in interiors, fountains, falls) | Static entities in landblock info whose 0x47 chain carries a water mesh and, for many, liquid-volume monitors | Like any static entity (`entity-descriptor.md`, `material.md`); the water shaders of §8.2 | The entity's own monitor boxes (§7.3) | §7.3, §8.5 |
| **Distant water** | The water pairs inside distant-landscape sector records (kind 5) | The same quads, built per 3 × 3-landblock sector (§9) | none | §9 |

Not water: the **fog-zone table** (0x17, slot 6). It is documented here (§5) because earlier
specs took it for the water table.

There is **no** water level in the height grid, in landblock properties or in any stored
property: the dictionary properties `WaterHeight` and `WaterType` have no stored value
anywhere (`environment.md` §7.4), and `Volume_WaterEntityOverRide` (a string the client
queries before creating the volume entity, BIN (d)) is stored in no record either (raw search
of every cell archive and of `client_gamelogic.dat`, OBS).

---

<a id="s3"></a>
## 3. Finding the tables (Confirmed)

The region descriptor (`environment.md` §3) holds one DID map per table kind. The client asks
each map for key 0 only (BIN (b)). Slots 5 and 6:

| Slot | Prefix | DB type (code, label) | Kind | Region 1 | Regions 2, 14 | Region 3 | Regions 4, 5 | Region 6 |
|---|---|---|---|---|---|---|---|---|
| 5 | 0x16 | 0x17 WATER_DESC | Water table (§4) | 0x16000000 | 0x16000004 | 0x16000005 | 0x1600000A | 0x1600000E |
| 6 | 0x17 | 0x16 FOG_DESC | Fog-zone table (§5) | 0x17000000 | 0x17000001 | 0x17000002 | 0x17000007 | 0x1700000A |

Note that the DID prefix and the DB type code are crossed: prefix 0x16 holds DB type 0x17 and
prefix 0x17 holds DB type 0x16 (BIN: the client registers DID range 0x16000000–0x16FFFFFF
for type 0x17 and 0x17000000–0x17FFFFFF for type 0x16; the water code loads the slot-5 DID
as type 0x17 and the fog code loads the slot-6 DID as type 0x16). The type labels are data
(enum mapper "DBOType", `property-system.md` §9). The full prefix → type table is in
`records.md` §2.1.

The DIDs above are observations; a reader takes them from the region descriptor at run time.
All ten records are in `client_general.dat` and start with their own DID.

---

<a id="s4"></a>
## 4. Water table (prefix 0x16, DB type 0x17)

<a id="s4-1"></a>
### 4.1 Record layout (Confirmed, 5 of 5, zero leftover bytes)

All integers little-endian, packed. VLE is `records.md` §1.2.

| Offset | Size | Type | Field | Meaning | Observed |
|---|---|---|---|---|---|
| 0 | 4 | DID | `did` | Own DID (0x16…) | |
| 4 | 4 | u32 | `reserved` | Read into the table object; no reader of it was found (BIN) | 0 in all 5 |
| 8 | 4 | u32 | `level_key` | Block-map key whose value is the **water level** (§6.1) | 4 in all 5 |
| 12 | 4 | u32 | `type_key` | Block-map key whose value is the **water type** (§6.1) | 7 in all 5 |
| 16 | 1 | bool8 | `has_default` | A default entry follows | 1 in all 5 |
| 17 | 33 or 5 | entry | `default` | Water-type entry (§4.2), used when the type matches no listed entry | key 0, `dry` = 1 in all 5 (5 bytes) |
| … | 1–5 | VLE | `count` *n* | Number of listed entries | 47, 19, 23, 31, 5 |
| … | | entry × *n* | `entries` | §4.2, back to back | keys 1 … *n* in stored order in all 5 |

<a id="s4-2"></a>
### 4.2 Water-type entry (Confirmed layout; roles per field)

| Order | Size | Type | Field | Meaning | Observed (125 listed entries) |
|---|---|---|---|---|---|
| 1 | 4 | u32 | `key` | Water-type number; compared with the block map's type value (§6.2) | 1 … *n* |
| 2 | 1 | bool8 | `dry` | 1 = this type means "no water" and **nothing else follows**; 0 = fields 3–5 follow. A reader rejects other values (BIN). | 0 in all 125; 1 in the 5 default entries |
| 3 | 4 | DID | `surface` | Surface record (prefix 0x31, `material.md` §4) of the water surface (§8.2) | 76 distinct, all exist |
| 4 | 16 | 4 × DID | `extra` | Four further DIDs. The client preloads them with the others and copies them into the landblock's water state, but no reader of them was found (BIN). | 0 in all 125 |
| 5 | 4 | DID | `volume_entity` | Entity descriptor (prefix 0x47) of the water-volume entity (§7.1) | 0x47000754 (120), 0x470025A2 (2), 0x47008188 (3) |
| 6 | 4 | f32 | `value` | Copied into the landblock's water state; no reader found (BIN) | 24.0 in all 125 |

Entry size: 5 bytes when `dry` = 1, 33 bytes when `dry` = 0. When the client creates an entry
before reading it, it fills the DIDs with its "no DID" value, `value` with 24.0 and `dry` with 0
(BIN); a reader never needs these defaults because every field it uses is stored.

> **OPEN:** Meaning of the four `extra` DIDs and of `value` (24.0 everywhere). Neither
> affects anything in this install.

<a id="s4-3"></a>
### 4.3 Census (Confirmed)

| Record | Regions | Size (bytes) | Entries | Lava entries (`volume_entity` 0x47008188) | Entries with 0x470025A2 |
|---|---|---|---|---|---|
| 0x16000000 | 1 | 1,574 | 47 | – | keys 23, 43 |
| 0x16000004 | 2, 14 | 650 | 19 | – | – |
| 0x16000005 | 3 | 782 | 23 | key 21 | – |
| 0x1600000A | 4, 5 | 1,046 | 31 | key 21 | – |
| 0x1600000E | 6 | 188 | 5 | key 5 | – |

The 76 distinct surface records use six shader records (§8.2).

<a id="s4-4"></a>
### 4.4 Decoder rules and hostile input

- Consume exactly: DID, three u32, `has_default`, optional default entry, VLE count, entries.
- `has_default` and `dry` are bool8 (0 or 1); reject other values.
- Bound *n* by the bytes left (an entry is at least 5 bytes).
- Keys need not be sorted or dense; build a key → entry index. When two listed entries share a
  key, the client uses the **first** in stored order (BIN: its look-up walks the list).

---

<a id="s5"></a>
## 5. Fog-zone table (prefix 0x17, DB type 0x16)

<a id="s5-1"></a>
### 5.1 Record layout (Confirmed, 5 of 5, zero leftover bytes)

| Offset | Size | Type | Field | Meaning | Observed |
|---|---|---|---|---|---|
| 0 | 4 | DID | `did` | Own DID (0x17…) | |
| 4 | 4 | u32 | `reserved` | | 0 in all 5 |
| 8 | 4 | u32 | `zone_key` | Block-map key whose value selects the fog zone (§5.3) | 3 in all 5 |
| 12 | 1 | bool8 | `has_default` | A default entry follows | 1 in all 5 |
| 13 | varies | entry | `default` | §5.2 | key 0 |
| … | 1–5 | VLE | `count` *n* | | 9 (0x17000000), 0 (the other four) |
| … | | entry × *n* | `entries` | §5.2 | keys 1 … 9 |

Records are 137 bytes (default only) or 1,244 bytes (0x17000000).

<a id="s5-2"></a>
### 5.2 Fog-zone entry (Confirmed layout; meanings OPEN unless stated)

A "colour" is a u32 0xAARRGGBB stored little-endian (`environment.md` §5.6). An "override
triple" is 9 bytes: u8 `mode`, u32, u32.

| Order | Size | Type | Field | Observed (10 entries) |
|---|---|---|---|---|
| 1 | 4 | u32 | `key` | 0 (defaults), 1–9 |
| 2 | 63 | 7 × override triple | `overrides` | modes 3 in all default entries; 3, 4 or 6 in the others; both u32 always 0 |
| 3 | 4 | f32 | `near` | 9,000 (region 1 default), 500 (other defaults), −50 … 50 (zones) |
| 4 | 4 | f32 | `far` | 10,000 (defaults), 50 … 1,600 (zones) |
| 5 | 4 | f32 | `amount` | 0 (defaults), 0 … 0.9 (zones) |
| 6 | 4 | f32 | `value_60` | 1.0 in all |
| 7 | 1 | u8 | `active` | 0 in the defaults, 1 in every zone. The client uses a zone only when this is non-zero (BIN). |
| 8 | 4 | colour | `colour` | white in the defaults; nine different opaque colours in the zones |
| 9 | 4 | f32 | `light_a` | 1.0 (0.7 in one zone) |
| 10 | 4 | colour | `colour_a1` | |
| 11 | 4 | colour | `colour_a2` | Present when the archive's version for this type is ≥ 4; otherwise a copy of `colour_a1` (BIN). Present in this install. |
| 12 | 4 | f32 | `light_b` | 1.0 (0.8 in one zone) |
| 13 | 4 | colour | `colour_b` | |
| 14 | 4 | colour | `colour_c` | Version ≥ 2 (present) |
| 15 | 1 | bool8 | `flag_c` | Version ≥ 2 (present); 0 in all |
| 16 | 4 | colour | `colour_d` | |
| 17 | 4 | DID | `did_e` | Version ≥ 3 (present); 0 in all |
| 18 | 0 or 4 | u32 | `value_e` | Present only when `did_e` ≠ 0; absent in all |
| 19 | 1 | bool8 | `flag_f` | 0 in all |
| 20 | 1 | u8 | `value_g` | 0 in all |

Every entry of this install is 123 bytes: the 4-byte key and 119 bytes of fields 2–20 (the
version-dependent fields are all present; `value_e` is absent). Field names are Tessera's.
The shape of fields 3–16 (a fog near and far distance, a fog amount, a fog colour, then light
brightness and colour pairs) parallels the day-description keyframe of `environment.md` §5.3;
the seven override modes plausibly say, per channel, whether the zone's value replaces,
scales or leaves the sky's value (Inferred).

> **OPEN:** Meaning of the override modes (3, 4, 6), of fields 9–20, and of the archive
> version that selects the optional fields (the client asks the archive for a per-type
> version; a reader that assumes the newest layout reads every record of this install).

<a id="s5-3"></a>
### 5.3 How the client uses the fog-zone table (BIN)

For an outdoor landblock (X, Y): read the block-map value of `zone_key` at the landblock
itself, at (X, Y + 1), at (X + 1, Y + 1) and at (X + 1, Y). For each value, take the entry
with that key (first match), else the default. If **none** of the four entries has `active`
≠ 0, the landblock has no fog zone and the sky's fog applies unchanged (`environment.md`
§11.3). Otherwise the client copies the four entries as the landblock's four corner values
and notes whether all four values are equal; how it blends the corners across the block and
with the sky's fog is OPEN (bilinear across the block is the natural reading).

<a id="s5-4"></a>
### 5.4 Census (Confirmed)

Block-map key 3 is 0 in all 390,150 block maps, so every landblock selects the default
entry, whose `active` is 0: **no fog zone is active anywhere in this install**. The nine
zones of 0x17000000 are unused data.

**Tessera rule (Decision):** decode the table and ignore it while key 3 is 0 everywhere.

---

<a id="s6"></a>
## 6. Landblock water

<a id="s6-1"></a>
### 6.1 The block-map keys (Confirmed)

The water table's `type_key` (7) and `level_key` (4) are **block-map keys** (`landblock.md`
§5). The value used is the raw u16 stored in the block map for that key; no 0x18 descriptor
palette is involved (no descriptor lists keys 4 or 7). Observed values:

| Region | Water table | Type values (key 7) | Level values (key 4) |
|---|---|---|---|
| 1 | 0x16000000 | 0–47 (0 in 57,027 blocks) | 0–255, 111 distinct |
| 2 | 0x16000004 | 0–19 and 37 (0 in 40,124) | 0–255, 182 distinct |
| 3 | 0x16000005 | 0–22 (0 in 1,975) | 0–151, 98 distinct |
| 4 | 0x1600000A | 0–24 (0 in 4,250) | 0–170, 119 distinct |
| 5 | 0x1600000A | 0–31 (0 in 6) | 0–163, 107 distinct |
| 14 | 0x16000004 | 0–11 (0 in 64,376) | 0–211, 40 distinct |

Type value 37 in region 2 (one block) names no entry of 0x16000004 and falls to the dry
default.

<a id="s6-2"></a>
### 6.2 Deciding a landblock's water (algorithm in words; BIN, Confirmed by data)

Inputs: the landblock (R, X, Y); its region's water table *W* (§3); its block map; its height
grid.

1. If the region has no water table, the landblock has no water.
2. Read the block-map value *t* of *W*.`type_key`. If the block map lacks that key, the
   landblock has no water.
3. Choose the entry: the first listed entry whose `key` = *t*; if none, the default entry; if
   there is no default either, the landblock has no water.
4. If the chosen entry's `dry` = 1, the landblock has no water. (Type 0 always lands here in
   this install: no table lists key 0, and every default is dry.)
5. Read the block-map value *L* of *W*.`level_key`. If the block map lacks that key, the
   landblock has no water.
6. Water height *h*, in world units of the landblock frame: convert *L* to a 32-bit float,
   multiply by 1000.0, then multiply by the 32-bit float nearest 1/255
   (0.003921568859368563), rounding each product to 32 bits. So *h* = *L* × 1000 / 255 within
   one unit in the last place: level 38 → 149.0196, level 86 → 337.2549, level 255 → 1,000.0.
7. Let *m* be the lowest of the 1,089 height samples of the block, converted with
   `landblock.md` §3.4. The landblock **has water** exactly when *h* > *m* (strictly). The
   highest sample plays no part.
8. If it has water, its water state is: the water type *t*, the entry's `surface`, `extra`,
   `volume_entity` and `value`, and the **water plane** z = *h* over the whole block
   (normal +Z), §6.3.

The client evaluates this once per landblock when it loads the landblock data (BIN). A
ceiling grid (`landblock.md` §3.5) does not enter the rule.

<a id="s6-3"></a>
### 6.3 The water plane and "under water" (BIN)

The client stores the plane as a normal (0, 0, 1) and a distance −*h*, so the signed distance
of a landblock-frame point *p* is *p*.z − *h*.

- **Under landblock water:** *p*.z − *h* < −0.0002. Points within 0.0002 of the surface count
  as not under water. The encounter pass (§10) and the client's "is this position under
  water" query use this test; when the test fails, the query goes on to a physics-world query
  (Inferred: for the liquid volumes of §7, so placed water counts too).
- The plane covers the whole 160 × 160 block, also where the terrain rises above it; a
  consumer that needs "water at this point" compares *h* with the terrain height there
  (scenery does, `scenery.md` §7.5).

> **OPEN (implementer, 2026-10-06, `tessera-world` water module):** with the Inferred
> second step (liquid volumes), a point within 0.0002 below *h*, or exactly at *h*, is
> "under water" after all: the volume entity's surface box (§7.1) reaches up to *h*
> inclusive. Whether the client's physics-world query uses inclusive box bounds, and so
> whether the 0.0002 tolerance of the plane test has any effect on the full query, is not
> known. `tessera-world` keeps both steps as stated (plane test, then boxes with inclusive
> bounds, the containment rule of `tessera-physics`).

<a id="s6-4"></a>
### 6.4 Census: where this install has water (Confirmed, 2026-10-05)

| Region | Landblocks | Dry type (entry dry) | Typed, level not above the lowest sample | **Wet** | Wet, level at or above the highest sample (fully submerged) | Wet with a ceiling |
|---|---|---|---|---|---|---|
| 1 | 65,025 | 57,027 | 3,225 | **4,773** | 1,634 | 14 |
| 2 | 65,025 | 40,125 | 20,885 | **4,015** | 848 | 398 |
| 3 | 65,025 | 1,975 | 32,794 | **30,256** | 25,328 | 8 |
| 4 | 65,025 | 4,250 | 35,611 | **25,164** | 18,860 | 411 |
| 5 | 65,025 | 6 | 39,570 | **25,449** | 20,505 | 152 |
| 14 | 65,025 | 64,376 | 393 | **256** | 0 | 17 |
| **All** | 390,150 | | | **89,913** | 67,175 | 1,000 |

Largest groups (type, level → height; landblock range; two example DIDs of the landblock
data records):

| Region | Type | Level → *h* | Wet blocks | X range | Y range | Examples |
|---|---|---|---|---|---|---|
| 1 | 29 | 86 → 337.255 | 1,353 | 42–121 | 170–197 | 0x80012AAB, 0x80012AAC |
| 1 | 37 | 86 → 337.255 | 899 | 47–224 | 65–199 | 0x80012F68, 0x80012F69 |
| 1 | 1 | 21 → 82.353 | 583 | 0–64 | 0–64 | 0x80010001, 0x80010002 |
| 2 | 11 | 96 → 376.471 | 880 | 52–179 | 14–222 | 0x800234D7, 0x800235D7 |
| 3 | 11 | 38 → 149.020 | 19,084 | 0–254 | 61–254 | 0x80030043, 0x80030044 |
| 4 | 11 | 38 → 149.020 | 16,330 | 0–254 | 0–254 | 0x80040005, 0x80040006 |
| 5 | 23 | 38 → 149.020 | 23,747 | 0–254 | 0–254 | 0x80050000, 0x80050001 |
| 14 | 11 | 96 → 376.471 | 37 | 3–21 | 170–184 | 0x800E03B8, 0x800E04B8 |

Level 38 (149.02 units) is the sea level of regions 3–5; land blocks there stand at 150
units or more and so stay dry although their type is set. Lava (§7.2) covers 121 wet
landblocks in region 3 and 535 in region 4 (type 21).

These are observations; an implementation computes water with §6.2 from the data and never
hard-codes a table of wet landblocks.

<a id="s6-5"></a>
### 6.5 Validation against placed objects (Confirmed)

Placed static entities whose world-building label names floating water plants: of the 1,067
that stand in wet landblocks, the median of *z* − *h* is −0.007 units; 543 lie within 5 cm of
*h* and 917 within 0.5 units. (The other 1,326 stand in landblocks without landblock water;
placed water, §7.3, is the Probable reason.)
The distant-sector water pairs (§9) give an independent check of §6.2 on every wet landblock.

---

<a id="s7"></a>
## 7. Water volumes

<a id="s7-1"></a>
### 7.1 The landblock water-volume entity (placement BIN; properties Confirmed)

For every wet landblock, when its static entities are created, the client creates one
client-side entity from the water type's `volume_entity` at block-local position (0, 0, *h*)
(BIN (d); the code sets no rotation or scale, Inferred: identity and 1). The entity is not
drawn as water (its setup holds a single-triangle mesh); its purpose is its **monitor
volumes**: its 0x47 record carries
`Entity_Monitors`, an array of `Entity_Monitor` structs, each an axis-aligned box in the
entity's frame (`Monitor_BoxMin`, `Monitor_BoxMax`, `Monitor_ScaleWithEntity` true) that
applies a property set (`Monitor_VolumePropertiesToApply`, a 0x78 DID) to objects inside it.
`Monitor_Name` uses enum mapper 0x23000096 (1 WaterSurfaceVolume, 2 WaterVolume, 3
WaterWadeVolume; labels are data).

| Entity | Monitor | Box min (x, y, z) | Box max (x, y, z) | Property set |
|---|---|---|---|---|
| 0x47000754 (water) | surface (1) | (0, 0, −0.5) | (160, 160, 0) | 0x78000075 |
| | wading (3) | (0, 0, −1) | (160, 160, −0.5) | 0x78000074 |
| | deep (2) | (0, 0, −1,600) | (160, 160, −1) | 0x78000073 |
| 0x470025A2 (water, deep only) | deep (2) | (0, 0, −1,600) | (160, 160, 0) | 0x78000073 |
| 0x47008188 (lava) | surface (1) | (0, 0, −0.5) | (160, 160, 0) | 0x78005CDF |
| | wading (3) | (0, 0, −1) | (160, 160, −0.5) | 0x78005CE0 |
| | deep (2) | (0, 0, −1,600) | (160, 160, −1) | 0x78005CE1 |

Relative to the water surface *h*: the surface zone is the top 0.5 units, the wading zone
0.5–1 unit below the surface, the deep zone from 1 unit down to 1,600 units below.

> **OPEN (implementer, 2026-10-06, `tessera-world` water module):** three monitor fields
> are not covered here: (a) `Monitor_Offset` (vec3, on 382 of 464 monitors per `sound.md`
> §10.3.4): whether and how it moves the box. Tessera (Decision): the box is
> (min + offset, max + offset) in the entity frame, before the entity's scale, rotation and
> position. With this rule the boxes of all three landblock volume entities of this install
> still cover the whole 160 × 160 block from *h* down (checked by the `tessera-world` test
> `volume_entities_cover_their_landblock`). (b) A monitor without
> `Monitor_ScaleWithEntity`: Tessera treats it as true (scales with the entity).
> (c) Whether `Entity_Monitors` is inherited along the 0x47 parent chain: Tessera reads it
> from the nearest level of the chain like other object properties (`scenery.md` §9.2); in
> this install no placed volume entity inherits its monitors.

<a id="s7-2"></a>
### 7.2 Volume property sets (Confirmed)

| Set | Properties (all boolean true) |
|---|---|
| 0x78000073 | `Volume_IsLiquid`, `Volume_WaterDeep`, `PathMap_IsWater` |
| 0x78000074 | `Volume_IsLiquid`, `Volume_WaterWading` |
| 0x78000075 | `Volume_IsLiquid`, `Volume_WaterSurface` |
| 0x78005CE1 | `Volume_IsLiquid`, `Volume_IsLava`, `Volume_LavaDeep`, `PathMap_IsWater` |
| 0x78005CE0 | `Volume_IsLiquid`, `Volume_IsLava`, `Volume_LavaWading` |
| 0x78005CDF | `Volume_IsLiquid`, `Volume_IsLava`, `Volume_LavaSurface` |

Three further sets carry `Volume_IsCoin` with coin surface, wading and deep flags (one each);
no landblock water type uses them.

These are the sets `collision.md` §10.4 names for the water movement states.

<a id="s7-3"></a>
### 7.3 Placed water volumes (Confirmed census)

48 entity descriptors (0x47) carry monitors with the water or lava sets above: the three of
§7.1 and 45 others. Placed static entities reference 37 of those (3,895 entities; the most used
two are referenced 1,017 and 569 times; 0x470025A2 is also placed twice by hand). Their boxes
are entity-local and smaller (for example ±15 × ±15 units with the same three depth bands, or
±8 × ±8 with bands near the surface). Apart from the three landblock volume entities, all of
them have a real mesh (4 to 12,880 vertices; regular grids such as 7 × 7 and 9 × 13 vertices
are common), drawn with the entity (`Translucent` true and render phase 2 on the most used
ones). They are placed by their static-entity frame (`entity-descriptor.md`) and scaled with
the entity when `Monitor_ScaleWithEntity` is true.

> **OPEN (implementer, 2026-10-06, `tessera-world` real-data census):** the three coin sets
> of §7.2 also carry `Volume_IsLiquid` (their flags read as liquid without a water or lava
> depth flag). Two placed 0x47 objects use them (19 placements). Counting every placed
> Physics entity whose object has monitors applying a `Volume_IsLiquid` set gives 3,914
> entities and 39 objects; without the coin sets it gives the 3,895 and 37 above. Whether a
> coin volume counts as water for movement (§7.4) and for the "under water" query is not
> known. Tessera (Decision): every `Volume_IsLiquid` box is liquid, as the rule of §7.4
> states.

<a id="s7-4"></a>
### 7.4 From volumes to movement states (partly OPEN)

`collision.md` §10.4 lists the mover states Water_ControlledSurface (1), Water_NoSurface (2) and
Water_UncontrolledSurface (3). The volumes above say *where* water is and how deep; which point
of the mover is tested against the boxes and how the client switches between the states is not
traced (`collision.md` Q9). **Tessera rule (Decision):** a mover is in water when its feet are
inside any liquid box; it swims (state 2) when the water surface is more than its float height
above its feet (`collision.md` §11.2), otherwise it wades (state 1, or 3 on steep support).

> **OPEN (implementer, 2026-10-06, `tessera-physics` controller):** the rule above does not
> say which state a mover in water **without** support takes when the surface is less than
> its float height above its feet (jumping or stepping off a ledge into shallow water).
> Tessera chooses state 2 (swimming) whenever there is no support. The feet are the
> mover's position (the bottom of its capsule, `collision.md` §10.6). While swimming, the
> controller keeps the feet from rising above the surface minus the float height and lets a
> swimmer above that line sink to it (with gravity −20 the swimming layer would otherwise
> lift the mover out of the water); the surface is the water plane's level where the liquid
> boxes end there, else the top of the liquid boxes above the feet.

> **OPEN (implementer, 2026-10-06, controller review):** `collision.md` §11.1 step 9 applies
> the Swimming layer (gravity −20) in states 1, 2 and 3. How its upward gravity interacts
> with support in states 1 and 3 is not traced (`collision.md` Q9). Tessera (Decision): in
> state 1 the mover stands on walkable support, so its vertical speed is 0. In states 2 and 3
> the controller uses a **float line**, the surface minus the float height. Below the line
> or on it, the layer's gravity applies and the feet stop at the line, with the upward speed
> zeroed. Above the line, the magnitude of the gravity (at least 1 m/s²) pulls the mover down,
> and the feet stop at the line with the speed zeroed, so a swimmer does not overshoot it.
> As a result, a mover dropping or stepping into deep water settles at the line without
> sinking below it, and a mover on steep support in water shallower than its float height
> slides down instead of being lifted. The original's buoyancy and its integration are not
> known.


> **OPEN (implementer, 2026-10-06, walker walk mode):** whether a mover inside an interior
> cell (`collision.md` §5.8.1) can be in landscape water is not stated. The landblock volume
> entity's boxes reach far below the water plane (§7.1), so a dungeon cell under a lake lies
> inside them: walk IN1 of `m4-5-visual.md` (a cell 660 m below a lake's plane) swam up out
> of its cell. Tessera (Decision, `walker.md` §13.5): no landscape water while the cell
> lookup at the feet + 0.05 m finds a cell. Please state the client's rule (BIN), and
> whether placed volumes inside cells (§7.3) still apply there.

> **OPEN (M4.5 exit verification, clean room, 2026-10-06):** how the original slows a mover
> that enters deep water at speed is not stated. Tessera (Decision, `collision.md` §12.3 OPEN
> (p)): below the float line the downward speed is cancelled at once, so a mover sliding
> into a lake at 15 m/s stops within one step instead of diving about 4 m before the lift
> turns it.
---

<a id="s8"></a>
## 8. Drawing water

<a id="s8-1"></a>
### 8.1 Landscape water geometry (BIN; Tessera rule for the near field)

The only code found that turns a water table's `surface` into geometry is the distant-landscape
sector builder (BIN (e)). For a sector of 3 × 3 landblocks (§9) and for each entry of the
sector's water list, it builds one mesh holding a quad for every landblock of the sector that
has water of that entry:

- corners at block-local (0, 0), (160, 0), (0, 160), (160, 160) offset by the landblock's
  position in the sector (160 units per landblock), all at height *h*;
- vertex normal straight up;
- texture coordinates (*i* / 3, 1 − *j* / 3) over the sector, *i* and *j* = 0 … 3 the corner's
  landblock-edge index along X and Y;
- two triangles per quad, drawn with the entry's surface record.

No separate near-field water mesh builder was found (OPEN whether the original uses these
sector quads at every distance; it is the Probable reading). **Tessera rule (Decision):** at
every distance draw, for each wet landblock of §6, one 160 × 160 quad at *h* with the water
type's surface record, texture coordinates as above taken from the landblock's place in its
3 × 3 sector (sector origin at landblock X − X mod 3, Y − Y mod 3); subdivide it into a grid
(for example 32 × 32 quads) only if vertex waves (§8.3) are wanted. Terrain above the plane
hides the water through the depth test; no shoreline clipping is needed.

<a id="s8-2"></a>
### 8.2 Material chain (Confirmed)

Water surface record (0x31, `material.md` §4) → shader record (0x2B) + materials (0x30) →
effective parameter set (`material.md` §9). The 76 water surfaces use six shader records:

| Shader record | Water surfaces | Samplers of its water programs |
|---|---|---|
| 0x2B0000A8 | 43 (of the 55 surface records that name it) | normal map; frame-buffer copy (refraction); reflection; depth (some permutations); an interactive ripple normal map (some) |
| 0x2B0007FD | 21 | as above plus flow map (3075), foam map (3079) and a noise texture |
| 0x2B0000CE | 5 | normal map; frame-buffer copy; depth (some); ripple map (some) |
| 0x2B0005EC | 5 | normal map, flow map, noise, frame-buffer copy, reflection, depth |
| 0x2B000559 | 1 | normal map, frame-buffer copy, reflection, environment cube map, depth |
| 0x2B000810 | 1 | colour-and-opacity layers, emissive, normal, flow, foam, noise, frame-buffer copy |

Every one also has a low-detail permutation that samples two colour textures (3000, 3001)
with the vertex texture coordinates, lights them per vertex and fogs them.

**Which permutation a statement covers (0x2B0000A8, Confirmed, W16).** Statements about
0x2B0000A8 elsewhere in the spec set describe different permutations of the same record:

| Permutations | Inputs and samplers | Lighting | Output and render state |
|---|---|---|---|
| Water permutations: 8 of the record's 13 (four Direct3D 11, all with a depth sampler, two of them also with the ripple map; two shader-model-2 and two OpenGL, without depth, one of each pair with a reflection sampler) | read only the position (the four Direct3D 11 vertex programs also declare a normal and one UV set in their input signature, unused; the program's only declared input register is the position); normal map, frame-buffer copy, optionally reflection, depth, ripple map | none from the light slots; only the fake directional light of §8.4 step 6 | §8.4; without a depth sampler source ONE, destination ZERO; with one, source alpha and inverse source alpha; cull byte NONE (both faces) |
| Low-detail permutations: the other 5 (one Direct3D 11 and one OpenGL with programs and the two colour samplers 3000, 3001; three without programs whose extra state lists describe two texture stages, `material.md` §7.4, §7.7) | position, normal and one UV set (three vertex inputs); the two colour layers through texture matrices 0 and 1 | ambient, hemispheric ambient, the per-vertex point-light array and the extra directional light (constants read by the OpenGL program; `lighting.md` §5, §7) | output alpha the material colour's alpha (2000); source alpha and inverse source alpha; cull byte CW (back faces culled) |

So "a two-layer, `Color`-role record lit per vertex and blended by 2000 alpha" (`material.md`
§20.1, `lighting.md` §4.3) is true of the low-detail permutations only, and "positions only,
frame-buffer composite" (§8.3–§8.4) of the water permutations only. Of the 55 surface records
that name 0x2B0000A8, 43 are water surfaces of the water tables and 12 are not; the latter are
drawn through whichever permutation the renderer selects (OPEN, §8.4).

The water parameters are the `c_Water…` labels of enum mapper 0x230001A6 (`material.md` §8.0;
flag-1 parameters feed the program constant of the same name, `material.md` §8.6). Those the
programs of 0x2B0000A8 read:

| Id | Label (interoperability) | Use (§8.3–§8.4) | Shader default in 0x2B0000A8 |
|---|---|---|---|
| 1044, 1045, 1046 | `c_WaterWaveSpeed`, `…WaveFrequency`, `…WaveAmplitude` | vertex waves | 1.0, 0.2, 0.0 |
| 1057–1062 | `c_WaterNormal0SpeedU` … `c_WaterNormal2SpeedV` | scroll of the three normal layers | 0.005, 0.0075, 0.015, 0.02, −0.01, −0.0125 |
| 1063–1065 | `c_WaterNormal0Scale` … `2Scale` | tiling of the three normal layers | 2, 4, 8 |
| 1047 | `c_WaterReflectionBumpDistortionAmount` | reflection offset | 0.1 |
| 1048 | `c_WaterRefractionBumpDistortionAmount` | refraction offset | 0.04 |
| 1050, 1051 | `c_WaterFresnelBias`, `…Exponent` | Fresnel | 0.1, 2.0 |
| 1054 | `c_WaterSpecularPower` | specular exponent | 80 |
| 1066 | `c_WaterOverbrightAmount` | colour scale | 1.0 |
| 1067 | `c_WaterFakeSpecularThreshold` | sparkle threshold | 0.25 |
| 1068 | `c_WaterFogAmount` | under-water tint of the refraction | 1.0 |
| 2006 | `c_WaterSpecularColor` | specular tint | white |
| 2009 | `c_WaterColor` | colour multiplier | white |
| 2010 | `c_WaterFogColor` | under-water tint colour | black |
| 2011 | `c_WaterShorelineColor` | near-camera refraction tint | white |
| 3011 | NormalMap | the normal texture of all three layers | |

Materials of individual surfaces override these as usual. Other declared water parameters
(1170–1173, 1182–1190, 2004, 2005, 2007, 2008) are read by the flow, foam and depth permutations; their
formulas were not traced (OPEN).

> **OPEN:** Three of the six water shader records do not declare every parameter of the
> table above (clean-room M4.5 real-data run, 2026-10-06): 0x2B0005EC and 0x2B000810 lack
> 1044–1046 (waves), 1059–1062 and 1064, 1065 (normal layers 1 and 2); 0x2B0007FD lacks
> 1044–1046. `tessera-material` (`WaterMaterial`) fills an undeclared id with the
> 0x2B0000A8 default of this table and lists it as undeclared. Whether those records' programs
> use a single normal layer and no vertex waves (so a renderer following §8.3–§8.4 should
> skip layers 1 and 2 and the waves for them) is not stated.

<a id="s8-3"></a>
### 8.3 Vertex stage (SH, shader record 0x2B0000A8)

The water programs (all but the low-detail permutations) read **only the vertex position** (the Direct3D 11 ones also declare a normal and one UV set in their input signature but never read them, §8.2; a vertex layout that supplies position, normal and UV satisfies every permutation). With *P*
the world position (in the program's frame the vertical axis is the second component; below,
x and y are horizontal and z vertical as everywhere in Tessera), *T* the effect time in
seconds, and *B* = ((*P*.x, *P*.y) − *O*) / 160, where *O* is an engine-supplied horizontal
offset (the program constant `c_ContiguousWorldBlockOffset`; Inferred: the origin of the
renderer's current block grid), so *B* advances by one per landblock:

- **Normal-layer coordinates:** layer *k* (0, 1, 2) samples the normal map at
  *B* × `Scale`ₖ + (`SpeedU`ₖ, `SpeedV`ₖ) × *T*.
- **Waves:** the vertex height gets the sum over *k* = 0 and 1 of
  *A*ₖ · sin *s*ₖ, with *A*ₖ = `WaveAmplitude` × (1 − 0.5 *k*) and
  *s*ₖ = ((−1 + 0.3 *k*) · *x* + 0.7 *k* · *y*) · `WaveFrequency` · (*k* + 1)
  + `WaveSpeed` · *T* · (0.5 + 0.8 *k*) (the program reduces *s*ₖ to [−π, π) before taking
  the sine), using the vertex's **model** x and y. The surface normal is tilted by the
  matching derivatives. With the default amplitude 0 there are no waves; on a single quad
  per landblock they are not visible either.
- **Distance fog factor:** clamp((distance to the eye − fog near) / fog range, 0, 1), with the
  frame's distance-fog values (`lighting.md` §10).

**Tessera rule (Decision):** take *O* = (0, 0) in the region frame, so *B* is the region
position in landblock units and the normal layers continue seamlessly across landblocks.

<a id="s8-4"></a>
### 8.4 Pixel colour (SH, shader record 0x2B0000A8)

1. **Normal:** sample the normal map at the three layer coordinates, map each sample to
   2 · (rgb − 0.5), add the three, normalise (*n*, tangent space), and rotate *n* into world
   space with the surface frame. Keep the first two components of the normalised
   tangent-space *n* (before the rotation) for the sparkle term of step 6.
2. **Screen coordinates** *S* of the pixel in [0, 1]².
3. **Refraction:** sample the frame-buffer copy (the scene drawn so far) at *S* plus the world
   normal's two horizontal components × `RefractionBumpDistortionAmount`. Call it *F*.
   Tint it: *U* = *F* + `FogAmount` · (`FogColor` − *F*). Near the camera blend toward the
   shoreline tint: with *w* = clamp((eye distance − 5) × 0.1, 0, 1),
   refraction colour = *F* · `ShorelineColor` + *w* · (*U* − *F* · `ShorelineColor`).
4. **Reflection:** sample the reflection texture at *S* plus the horizontal normal components
   × `ReflectionBumpDistortionAmount`, plus 0.003 on the vertical screen axis. In the
   permutations without a reflection texture the unmodified refraction sample *F* is used
   instead.
5. **Fresnel:** with *v* the unit vector toward the eye, *f* = `FresnelBias` + (1 −
   `FresnelBias`) · (1 − |*v* · *n*|)^`FresnelExponent`; colour = refraction + *f* ·
   (reflection − refraction).
6. **Specular:** *s* = clamp(*n* · normalize(*v* − *L*), 0, 1)^`SpecularPower` +
   4 · (clamp(*g*ₓ − `FakeSpecularThreshold`, 0, 1) + clamp(*g*ᵧ − `FakeSpecularThreshold`,
   0, 1)). The second summand is the sparkle term; it is added **after** the exponent and is
   not itself clamped, so *s* can exceed 1 (at most 9). Here *n* is the world-space normal of
   step 1, *v* the unit vector toward the eye, and *g* the first two (tangent-space,
   horizontal) components of the normalised normal-map sum of step 1, before its rotation into
   world space.
   *L* is the engine's directional "fake light" direction (program constant
   `c_FakeDirLight_WorldSpaceDirection`): the main light's **travel** direction (pointing away
   from the light, so −*L* points toward it) when the main light is directional, and an engine
   default direction otherwise (`lighting.md` §4.2, BIN-L (g); W17). For an outdoor scene
   whose main light is the sun, −*L* is the toward-sun vector of `lighting.md` §8.1.
7. **Output:** colour × `WaterColor` × `OverbrightAmount` + *s* × the fake light's specular
   colour × `SpecularColor`; then mixed toward the distance-fog colour by the fog factor of
   §8.3. Output alpha = the alpha of the material colour (2000).

Render state of these permutations (`material.md` §7.6): the ones without a depth texture write
with source ONE and destination ZERO (they composite the frame-buffer copy themselves); the
ones with a depth texture and the low-detail ones blend with source alpha and inverse source
alpha; all write depth; all but the low-detail ones draw both faces.

> **OPEN:** How the client chooses the permutation (the user's water-reflection,
> shoreline-effect and interactive-water settings are the likely switches; the permutation
> keys are opaque, `material.md` §7.4), how the reflection texture is rendered (a mirrored
> view about the plane z = *h*, presumably), what the depth-texture permutations compute
> (soft shorelines are the likely use; parameters 1170–1173), the flow and foam formulas of
> 0x2B0007FD and 0x2B0005EC, and the draw order of water relative to other translucent
> objects.

> **OPEN (implementer, 2026-10-06, `tessera-render` water pipeline):** step 1 rotates the
> normal-map sum "into world space with the surface frame" without saying which way the
> tangent-space axes point on the landscape plane, i.e. whether the world normal is
> *n*.x · **T** + *n*.y · **B** + *n*.z · **N** or uses the −*n*.y of `lighting.md` §5.2.4
> (**T** along +x, **B** along +y, the directions in which the layer coordinates of §8.3
> grow). Tessera uses the `lighting.md` §5.2.4 sign (`docs/specs/render.md` §5.3). The sign
> mirrors the ripples' shading and the refraction offset; it does not change the Fresnel or
> sparkle magnitudes.

> **OPEN (implementer, 2026-10-06):** step 6 needs the fake light's direction and specular
> colour when the main light is not directional; `lighting.md` E18 names "a default
> direction" and "an engine specular colour" without values. Tessera uses straight down with
> a black specular colour (no highlight) in that case (`docs/specs/render.md` §5.3).

**Tessera rule (Decision), until those are settled:** draw water after opaque geometry with a
copy of the colour buffer as the refraction source, implement steps 1–7 with the reflection
replaced by the refraction sample (the original's no-reflection path), and use alpha 1.

<a id="s8-5"></a>
### 8.5 Placed water meshes

Placed water (lakes above landblock level, pools, rivers in dungeons, falls) is ordinary
static geometry whose surface records use the water shaders of §8.2; it is drawn with
`entity-descriptor.md` and the material pipeline, and the same shading applies. Its meshes
carry their own texture coordinates and may be tessellated, so vertex waves show there.

---

<a id="s9"></a>
## 9. Distant-landscape sectors and their water (kind 5)

The landblock-family records `0x805RXXYY` (`landblock.md` §7) are DB type 0x41
DISTANT_LANDSCAPE_SECTOR (BIN (a)); one exists per 3 × 3 group of landblocks, at X and Y
divisible by 3. Among other things the client reads from each (BIN (e)):

- a **sector water list**: a VLE count, then that many pairs of (u8 water type, f32 height);
- for each of the sector's nine landblocks, a "has water" byte and the index of its pair in
  the list.

The pairs are exactly the (type, *h*) values of §6.2: in every one of the 12,925 sector
records whose 3 × 3 landblocks include a wet one, every distinct (type, *h*) of those
landblocks occurs as a byte pair in the record's trailing section, with *h* bit-identical to
the 32-bit result of §6.2 step 6 (OBS). The rest of the record's layout is OPEN
(`landblock.md` §7).

**Tessera rule (Decision):** derive distant water from the block maps with §6.2, which gives
the same pairs, instead of decoding kind 5.

> **OPEN (implementer, 2026-10-06):** the sector pair stores the water type as one byte,
> while the block-map type value is a u16. How the client narrows a type value above 255 is
> not stated (none occurs: the largest observed type is 47). `tessera-world` keeps the low
> 8 bits.

---

<a id="s10"></a>
## 10. Other consumers of landblock water (BIN)

| Consumer | What it reads | Spec |
|---|---|---|
| Procedural scenery | The plane and the "has water" flag: `height_mode` 1 snaps to *h*; the water-depth and above/below-water ranges use *w* = *h* | `scenery.md` §7.4 step 11, §7.5, §11 (which points here) |
| Encounter pass | Whether the cell centre is under water (§6.3) and the water type *t* | `scenery.md` §7.7 (out of scope offline) |
| "Under water" query (camera, effects) | §6.3, then liquid volumes in the physics world | §7 |
| Movement | The volumes of §7 | `collision.md` §10.4, §12.3 step 7 |

---

<a id="s11"></a>
## 11. Invariants

| # | Invariant | Status |
|---|---|---|
| WA1 | Every 0x16 record parses exactly with §4.1–§4.2; `has_default` and `dry` are 0 or 1 | Confirmed, 5 of 5 |
| WA2 | Every 0x17 record parses exactly with §5.1–§5.2 using the newest layout | Confirmed, 5 of 5 |
| WA3 | Every non-dry entry's `surface` is an existing 0x31 record and `volume_entity` an existing 0x47 record | Confirmed (125 of 125) |
| WA4 | Every block map has the water table's `type_key` and `level_key` and the fog table's `zone_key` | Confirmed (390,150) |
| WA5 | For every wet landblock, the (type, *h*) pair occurs in the distant-sector record of its 3 × 3 group | Confirmed (12,925 sectors) |
| WA6 | §6.2 is a pure function of the water table, the block map and the height grid (deterministic) | BIN |

Hostile input: bound every count by the bytes left; reject flag bytes other than 0 and 1;
treat a non-finite `value` or fog float as malformed; never index the entry list by key
without checking it exists.

> **OPEN:** Which fog-zone floats "fog float" covers, and whether the client itself rejects
> non-finite values (no BIN statement). The `tessera-formats` decoder (M4.5) rejects a
> non-finite value in every f32 of a fog-zone entry (`near`, `far`, `amount`, `value_60`,
> `light_a`, `light_b`) and in the water entry's `value`; all 10 entries and 125 water
> entries of this install are finite. It keeps `active` (§5.2 field 7, typed u8, not bool8)
> as a raw byte and treats any non-zero value as active, and keeps the override `mode`
> bytes raw.

---

<a id="s12"></a>
## 12. Worked examples (synthetic)

**Water table.** A table with `level_key` 4, `type_key` 7, a dry default and two entries
(key 1: surface 0x31000001, volume entity 0x47000001, value 24.0; key 2: dry) is
4 + 4 + 4 + 4 + 1 + 5 + 1 + 33 + 5 = 61 bytes: DID, 0, 4, 7, byte 1, (key 0, byte 1),
VLE 2, (key 1, byte 0, 0x31000001, four zero DIDs, 0x47000001, 24.0), (key 2, byte 1).

**Landblock water.** Block map: key 7 → 1, key 4 → 38. Height grid minimum stored value
3,000 → 3,000 × 2,000 / 65,535 = 91.55 units. *h* = 38 × 1000 / 255 = 149.0196 > 91.55, so the
block has water of type 1 at 149.0196. A point at block-local z = 148.5 is under water
(148.5 − 149.0196 = −0.5196 < −0.0002) and inside the wading box of a 0x47000754-like volume
entity (−1 ≤ −0.5196 < −0.5 relative to the entity at *h*). With key 4 → 23 instead,
*h* = 90.196 < 91.55: no water although the type is set.

**Fog zones.** A zone table with `zone_key` 3 whose entries are all inactive, or block maps
whose key 3 is 0, leaves the sky's fog unchanged.

---

<a id="s13"></a>
## 13. Corrections and notes for other specs

| Spec | Statement | Correction |
|---|---|---|
| `environment.md` §4, §8 (before 2026-10-05 revision) | 0x16 "terrain type → surface palette", 0x17 "water normal map or FOG_DESC" with a texture list 0x400043FA / 0x4000460C | 0x16 is the water table (§4), 0x17 the fog-zone table (§5). The "texture lists" were four bytes straddling the f32 values 500.0 (or 9,000.0) and 10,000.0 of the fog entry; neither record holds a texture list. Revised in `environment.md` the same day. |
| `scenery.md` §11, S14, §15 item 11 | Water from the slot-6 table (0x17) and block-map key 3; "no landblock has a water surface"; `height_mode` 1 never places anything | Water comes from the slot-5 table (0x16) and keys 7 (type) and 4 (level); key 3 is the fog-zone key. 89,913 landblocks have water (§6.4), so `height_mode` 1 nodes and the water ranges of `scenery.md` §7.5 apply there. The height formula (level × 1000 / 255) and the presence rule there are right. **Done:** `scenery.md` §11, S14 and §15 item 11 now point to §6.2–§6.3 (scenery revision of 2026-10-05). |
| `terrain-texturing.md` §11, §15 (before its 2026-10-05 revision) | Slot 5 (0x16) is the water table naming a water-shader surface and a 0x47 physics object; "no terrain record holds a water level" | Right about the table. Water levels are block-map values (keys 4 and 7), §6; the 0x47 object is the volume entity of §7.1. **Done:** `terrain-texturing.md` §11 now only points to this spec. |
| `material.md` §20.1 (rows "§18 unlit blended records" and "§11.3 unlit") | 0x2B0000A8 described as a `Color`-role record blended by 2000 alpha and lit by ambient, hemispheric and per-vertex lights in pass 0x00; "442 surfaces of the 0x2B0000A8 kind" | Only 55 surface records name 0x2B0000A8 (43 water). The description fits its low-detail permutations only; its water permutations take positions only, composite the frame-buffer copy themselves and (without a depth texture) write with ONE/ZERO (§8.2 table, §8.4). If "of the 0x2B0000A8 kind" means a family of similar records, the count should name that family. For the material owner. |
| `lighting.md` §4.3 (last sentence) | 0x2B0000A8 is a "two-layer record" whose colour layers read UV set 0 and differ only in the texture matrix | True of its low-detail permutations only; its water permutations read no UV set (positions only, §8.3). For the lighting owner. |
| `landblock.md` §5.6 | Keys 3, 4, 6, 7, 0x10000012 unresolved | Resolved there (revised the same day): 3 fog zone, 4 water level, 7 water type (this spec), 6 encounter zone. |
| `collision.md` §10.4 OPEN, Q8; §12.3 step 7 | Where water volumes are placed and how deep water is | §6.2 (landblock water), §7.1 (volume boxes), §7.3 (placed volumes). Switching between water states stays OPEN (§7.4). |
| `terrain.md` §9 OPEN (water) | No water level to shade the shore against | §6 gives the level per landblock. |
| `walker.md` §9 OPEN | Water not drawn | §8 gives the geometry and shading. |
| `material.md` §8.7 OPEN | Water effect-family parameters | §8.2–§8.4 for the parameters the 0x2B0000A8 programs read; the flow, foam and depth parameters stay OPEN. |

---

<a id="s14"></a>
## 14. Open questions

> **OPEN:** The four `extra` DIDs and the float `value` of a water entry (§4.2).

> **OPEN:** Fog-zone override modes, the remaining fog-zone fields, the corner blending, and
> the archive version that selects optional fog fields (§5). Moot in this install (§5.4).

> **OPEN:** Whether the original draws near water with the sector quads of §8.1 or with
> another builder; the reflection pass; the permutation choice; the depth, flow and foam
> formulas; water draw order (§8.1, §8.4).

> **OPEN:** How a mover is tested against the liquid boxes and how the water states switch
> (§7.4, `collision.md` Q9).

> **OPEN:** The full layout of the distant-landscape sector record (§9, `landblock.md` §7).

---

<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version. Settles what prefixes 0x16 and 0x17 are (BIN); full layouts with censuses; landblock water rule and census (89,913 wet landblocks); water volumes; drawing (geometry BIN, shading SH); distant water pairs; corrections to `environment.md`, `scenery.md`, `terrain-texturing.md`, `landblock.md`, `collision.md`, `terrain.md`, `walker.md`, `material.md`. |
| 2026-10-05 | Revision after review (changes requested). Legend: **Inferred** defined. New rows W16 (0x2B0000A8 users and permutations, OBS + SH) and W17 (fake-light direction, from `lighting.md` §4.2). §8.2: 0x2B0000A8 user count (55 surface records, 43 water) and a permutation table saying which statements cover which permutation; the "other declared water parameters" list no longer includes 2006, which the table already covers. §8.4 step 6: *L* is the main light's travel direction (or an engine default), adopted from `lighting.md` §4.2. §10: scenery row updated. §13: scenery and terrain-texturing rows marked done; correction rows added for `material.md` §20.1 and `lighting.md` §4.3. |
| 2026-10-06 | Second revision after re-review (changes requested). §8.4 step 6 restated as one formula: the sparkle term 4 · (clamp(*g*ₓ − threshold) + clamp(*g*ᵧ − threshold)) is added to the specular term after the exponent, outside the clamp (SH: the OpenGL water program and all four Direct3D 11 water pixel programs of 0x2B0000A8 agree); *L* is defined in a separate sentence. §8.4 step 1: the sparkle input is the normalised tangent-space normal (was wrongly "unnormalised sum"). §8.2 permutation table and §8.3: the water programs read only the position; the four Direct3D 11 vertex programs also declare a normal and one UV set in their input signature, unused (SH, all four checked). |
