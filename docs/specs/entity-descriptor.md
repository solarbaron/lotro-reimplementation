<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Static entity descriptors in landblock info (placement, rotation, scale, references)

| | |
|---|---|
| **Status** | **Accepted** (re-review of 2026-10-07 passed; covers the OPEN-closing revision with a third BIN use, its repair revision with a fourth and the second repair revision with a fifth, §15). Previously Accepted 2026-10-05. Sections, anchors and numbers of that Accepted text are unchanged; new material is in new subsections (§4.9, §9.3, §9.4) and in the rows the change log names. |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Re-review of the second repair revision, 2026-10-07 (a session separate from the writer's): **passed**, Accepted. Details in the review log (decision 0018). |
| **Implements into** | `crates/formats` (static entity descriptor decoder, inside the landblock-info decoder), `crates/world` (placement transform, streaming of static entities, content-layer and draw filters), `crates/net` sandbox backend ("serve static world entities"), `apps/client` (world walker), `apps/explorer` (landblock view) |
| **Companion specs** | [`landblock.md`](landblock.md) (normative for the landblock-info record that contains the descriptors, for cells, block maps and the terrain height scale). [`coordinates.md`](coordinates.md) (frames, map coordinates, `/loc`, the heading rule §11.3–§11.4). `setup.md` §3–§7 (the 0x47 physics-object record and its chain to meshes). [`appearance.md`](appearance.md) §7 (what the weenie objects are; NPC-like figures). `property-system.md` §4.6 (Position), §5 (embedded property sets), §8 (DID maps, `CONTENT_LAYER`). `records.md` §1 (VLE, TSize, Pascal string), §2.2 (landblock DIDs). `mesh.md` §11 (axes, units, handedness). `wstate.md` (0x70 objects named by weenie entities). [`lighting.md`](lighting.md) (light entities). |

---

<a id="s0"></a>
## 0. Purpose and terminology

Every landblock that holds placed scenery has a **landblock-info** record (DID
`0x802RXXYY`, `records.md` §2.2). Inside it is a list of **static entity descriptors**: one
per placed object (a house, a tree, a fence segment, a lamp, a spawn point, a patrol point …).
This spec gives:

- the descriptor's byte layout, decoded over **all** records, with the values of absent
  fields (§3);
- what each field means: entity type, instance ID, the object it names, position, rotation,
  scale, properties, content layers, name, flags (§4);
- the placement transform from object space to world space, in words (§5), with the evidence
  for the rotation order and direction (§6) and for scale (§7);
- a validation of placements against terrain heights (§8);
- which descriptors a world walker shows, including the content-layer rule (§9);
- the decoding algorithm, invariants and statistics (§10–§12).

**Terminology.** `setup.md` calls the 0x47 records in `client_gamelogic.dat` "entity
descriptors". To avoid a clash, this spec says **static entity descriptor** (SED) for the
structure inside landblock info, and **physics object** (0x47 record) for what `setup.md` §4
describes. A SED names a physics object; the physics object leads to a setup and meshes.

---

<a id="s1"></a>
## 1. Provenance

Legend as in `mesh.md` §1: **Confirmed** = decoded on the user's own install (2026-10-04, and
the re-checks of 2026-10-05 named below) by throwaway probes in the dirty-room scratchpad;
**BIN** = read from the client executable (method M4, [decision
0011](../decisions/0011-standing-m4-authorisation.md)); facts that rest on BIN alone say so;
**Reported** = third-party material only.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04. **All 49,636** landblock-info records of regions 1–5 and 14 decoded to the last byte; **all 3,253,862** SEDs read. All 27,962 distinct 0x47 records they name resolved through `setup.md` §3.1 to setups and mesh bounds. All 775 distinct 0x70 objects they name opened. Terrain heights read from the 0x800RXXYY records for the validation in §8. Re-checked 2026-10-05 with the repository's decoders as tools: the `Entity_ContentLayers` list of every SED and of every landblock instance property set (3,768,840), the IID bits 24–35 of all of them, the 27 SEDs without a frame and the mesh bounds of their objects, the CONTENT_LAYER DID map 0x28000012 (all 1,661 entries). Repair re-check 2026-10-05 (all SEDs again): SEDs and records with SEDs per region; framed SEDs by cell presence of their landblock and by the interior flag; first and last rotation components against ±1 at tolerances 0, 10⁻⁶, 10⁻⁵ and 10⁻⁴; distinct scale values under several rounding methods; and for every Physics SED the three draw properties on the SED, on its instance property set, on every level of its 0x47 chain (27,571 objects, 28,151 chain levels) and in every 0x78 extra set those levels name (17,484, all parsed), plus `Render_ServerDraw` on all of them. **Re-check 2026-10-07** (all SEDs of types Decal, Weenie, Physics, Light and PatrolPoint, 2,795,543): the effective `Entity_CreationType` and `Entity_ContentLayers` found by the client's lookup order of §4.9 (the SED's own set, then the named object's chain, for weenies the 0x70 property set and then its `PhysObj` chain), the creation-type adjustment for 0x70 objects, the content-layer test of §4.5.1 for (*L*, *I*) = (1, 0), (0, 0), (1, 5) and (0, 5), the interior flag, and the instance property sets carrying `NPC_StartingIdleAnimation` or `Appearance_RandomAppearanceType`; the frames of the N1–N7 SEDs of §9.4. **Repair re-check 2026-10-07** (all Decal, Weenie, Physics, Light and PatrolPoint SEDs the client creates, by the rules of §4.9): the view distance of §9.3 step 3 from `Render_FarViewDistance` and `Render_LODClass` (both by the lookup of §4.9) and the LOD-class table 0x32000000 (all 32 entries), the resulting set (long-range, main, far-view) by type, layer-test result and interior flag, and `Render_AllowDistanceBasedCreation` on all 49,636 landblock-info property sets. **Second repair re-check 2026-10-07** (all 2,380,699 Physics SEDs again): the far-view nodes among the created Physics SEDs split by view distance (*v* = 0, 0 < *v* < 25, 25 ≤ *v* ≤ 160), and every count of E28 and ED11 recomputed. |
| BIN | The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035, compiled 2026-09-27), disassembled in the dirty room on 2026-10-05 for facts only. Questions: (1) which content layer the world shows and how a SED's layer list is tested (owner item O3); (2) the heading of a placed frame as `/loc` reports it (O2); (3) the SED reader: order of the rarely used fields, the values of absent fields, the meaning of bits 0x01, 0x20, 0x200 and 0x400; (4) how instance IDs carry an instance number. Why M1–M3 were not enough: the data hold the layer lists and the DID map labels but not the rule that combines them; the reported fields (bits 0x01, 0x20, 0x200) never occur in this install, so their place and meaning cannot be observed; the owner observations O2 and O3 are pending and could only sample one spot each. Only the SED reader and writer, the landblock entity enumeration, the content-layer test, the instance-ID helpers and the position formatting were read; no network, login, launcher or anti-tamper code. **Second use, 2026-10-05 (repair session, same build, same method, address ranges only):** (5) which properties decide whether a render node is hidden, and in which order (§9); (6) on which path, and with which *L* and *I*, the client evaluates the content-layer test for Physics SEDs (§4.5.1). **Why M1–M3 were not enough:** the data hold the property values but not the rule that combines them, and no published source gives one; on this install the SED-versus-object order changes the result for 12 SEDs only, too few and too scattered for an owner observation (M3) to establish a rule; the content-layer path leaves no trace in data. Read: the render node's property-change handler and its visibility test, the generic property getter it calls (only far enough to see that the lookup is dispatched to the owning object), the landblock loader's pass over Weenie, Physics and PatrolPoint SEDs, the clearance-volume builder it calls, and the content-layer test with its membership routine. **Third use, 2026-10-07 (OPEN-closing session, same build, same method, address ranges only):** (7) whether, and on which path, the drawn object of a Physics SED is subject to the content-layer test, and what a failed test does (§4.5.1 OPEN; owner items O3 and O17); (8) which SEDs the client creates entities for at all, and from which property (§4.9); (9) in which order an entity built from a SED answers a property lookup: its per-instance set, the SED, the physics object (§9.2 OPEN); (10) where a created entity takes its position, rotation, cell and scale from (§4.4, §4.8, §7; O17 facing); (11) how the render node stores `Render_DrawMeshes` and `Render_NeverAddToScene` (§9.2 OPEN). **Why M1–M3 were not enough for (7)–(11):** the data hold the layer lists, creation types and property values but not the rules that combine them; the drawn-object side of the layer test leaves no trace in data; the owner observations O3 and O17 could check a handful of spots but not establish a rule for 2.3 million objects. Read: the SED property lookup and its object-chain fallback, the creation-type reader and its validity check, the landblock loader's placement records and the creation list built from them, the per-landblock streaming index and its update (the second user of the content-layer membership routine), the entity factory's creation of a streamed entity, the entity property component's lookup order, the per-IID property-set lookup, the cell constants' static initialisers, and the two render-node cases for `Render_DrawMeshes` and `Render_NeverAddToScene`. No network, login, launcher or anti-tamper code. **Fourth use, 2026-10-07 (repair session after the review of the same date; same build, same method, address ranges only):** (12) how the streaming index sorts a landblock's nodes by view distance, which constants decide it, and how the long-range and far-view sets are created and removed (§9.3); (13) what triggers the per-landblock pass and which landblocks are in range (§4.5.1, §9.3); (14) the order in which the creation list is built and what its two IID fields hold (§9.3). **Why M1–M3 were not enough for (12)–(14):** the data hold `Render_FarViewDistance`, `Render_LODClass` and the LOD-class table, but not the thresholds, the set rules or the update triggers; these are code paths and constants with no trace in any record; observing the running client (M3) could show that an object appears late or stays, but not the threshold, the trigger or the rule. Read: the streaming index's node builder and its view-distance routine, the LOD-class table's reader (to see which stored float the routine uses), the per-landblock pass, the manager's position update and its constructor (initial range and switches), the range test and the landblock enumeration around the viewer, the switch that turns the far-view mode on and off, and again the creation-list builder. No network, login, launcher or anti-tamper code. **Fifth use, 2026-10-07 (second repair session after the re-review of the same date; same build, the analysed copy's file hash equal to the installed executable's; same method, address ranges only):** (15) what the creation list holds after the SED records and whether the streaming index treats those records like the SED ones (§9.3 steps 2–3); (16) which landblocks the enumeration around the viewer yields when the viewer's position is invalid or its cell is the "no cell" value (§9.3 step 4). **Why M1–M3 were not enough for (15)–(16):** both are code paths with no trace in any record; the data cannot show the order or the content of an in-memory list, and observing the client could not separate an invalid viewer from an empty neighbourhood. Read: the creation-list builder, the streaming index's node builder, the landblock enumeration around the viewer, the manager's position update and the cell constants' start-up initialiser. No network, login, launcher or anti-tamper code. |
| AP | [`appearance.md`](appearance.md) §7.1–§7.4 (same install, 2026-10-05): what the 775 weenie objects are, the landblock weenie list as per-instance property sets, NPC-like figures on Physics SEDs. Folded in here as corrections (§14). |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution; decompiled class `EntityDescLoader`, data class `EntityDescriptor`) and `lotro-tools` (`LandblockInfoLoader`, readable source). Field order and flag bits. Facts only; no code taken. |
| DS | `research/notes/data-semantics.md` §6.1 (positions, 160 m landblocks), §6.3 (landblock info), §6.4 (entity descriptor bits). |
| ER | `research/notes/engine-client-re.md` §4 (coordinate systems, `/loc` output) and §3 (cache type names). |
| SU | `setup.md` (0x47 → 0x1F → 0x04 → 0x06 chain; Z-up object space; (w, x, y, z) frames). |
| PS | `property-system.md` (property types, embedded sets, DID maps). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| E1 | SEDs are a u32-counted list inside landblock info, after the cells (§2) | LC, DS; OBS (49,636 records, zero trailing bytes) | Confirmed |
| E2 | Layout: u32 flags, then optional fields in a fixed order; the property set precedes the 0x200 list, which precedes the 0x40 word; the name is last (§3) | LC, DS; OBS (3,253,862 descriptors); BIN (place of 0x200) | Confirmed for the observed bits; BIN for 0x200 |
| E3 | Flag 0x400 carries no payload and marks interior entities (§3.3, §4.8) | OBS; BIN (stored as a boolean) | Confirmed (layout); meaning Confirmed statistically |
| E4 | Bit 0x001 adds a DID, bit 0x020 a legacy scale vector, bit 0x200 a VLE-counted list of u32 (§3.1, §3.4, §7) | LC; BIN | BIN (never set in this install) |
| E5 | Entity type is an enum value labelled by enum mapper 0x23000091 (§4.1) | LC; OBS | Confirmed |
| E6 | The 64-bit instance ID encodes region and landblock; bits 24–35 receive the instance number of an instanced copy (§4.2) | OBS (100 %); BIN | Confirmed (layout); BIN (bits 24–35) |
| E7 | The physics-object field names a 0x47 record, or a 0x70 object for weenies (§4.3) | LC; OBS | Confirmed |
| E8 | Position is landblock-local x, y in metres from the landblock's south-west corner, z absolute height (§4.4) | DS, ER; OBS (§8) | Confirmed |
| E9 | Rotation is a unit quaternion stored (w, x, y, z), applied as an active rotation; the client reads it into the same frame type as every position (§4.4, §6) | LC (field names); OBS (§6); BIN | Confirmed |
| E10 | Scale is the uniform vector property `Render_AdjustableScale` (equal to `Physics_AdjustableScale`) on every SED (§7) | OBS | Confirmed |
| E11 | Census of the draw properties on SEDs, instance sets, 0x47 chains and their 0x78 extra sets; no object holds any of them at more than one chain level or in an extra set; SED and object disagree for 12 Physics SEDs only (§9.1) | OBS | Confirmed |
| E12 | A SED without bit 0x010 is placed at the owning landblock's origin with the identity rotation (§3.4, §4.4) | BIN; OBS (27 SEDs; mesh bounds) | BIN, consistent with data |
| E13 | The client's writer sets each flag bit exactly when its field differs from the default (§3.4) | BIN | BIN |
| E14 | Content layers: labels 0 `Undef`, 1 `InstanceZero`, 2 `All`; membership rule; the shared world shows lists naming 1 or 2 (§4.5.1) | OBS (DID map 0x28000012, census); BIN (rule; its use for decals, lights, weenies, Physics clearance volumes and, since 2026-10-07, the created entities of Weenie, Physics and PatrolPoint SEDs, §9.3) | BIN, census Confirmed. Revised 2026-10-07: the OPEN on drawn Physics objects is closed (E25) |
| E15 | The `/loc` heading of a SED's frame is h = (360° − ψ) mod 360°, the bearing of its model +Y axis for a pure turn (§4.4) | BIN (`coordinates.md` §11.3–§11.4) | BIN |
| E16 | The 775 weenie objects are housing hooks (744), distant-battle units (29), one door and one monster; none is an NPC (§4.1, §4.3) | AP; OBS | Confirmed |
| E17 | The landblock weenie list holds one instance property set per SED IID (plus server objects); a SED's layer list is repeated there unchanged (§4.5.2) | AP; OBS | Confirmed |
| E18 | Static NPC-like figures are Physics SEDs whose object is a humanoid rig and whose instance set dresses and animates them (§4.1, §9) | AP | Confirmed |
| E19 | The client turns Decal and Light SEDs into decal and light objects after the content-layer test (§4.1, §9) | BIN | BIN |
| E20 | A render node is hidden when its effective draw value is false: `Render_ServerDraw` if found, else `Render_Draw` if found, else drawn; both re-evaluated when either property changes (§9.2) | BIN (question 5); OBS (dictionary defaults: both true) | BIN |
| E21 | The landblock loader evaluates the content-layer test for Weenie, Physics and PatrolPoint SEDs with *I* = the instance number of the landblock copy and *L* handed in by the loader's caller, and stores the result in a per-SED placement record together with the SED's creation type, a copy of the SED and its clearance volumes; the record is built whether the test passes or not (§4.5.1, §9.3) | BIN (questions 6 and 7) | BIN. Revised 2026-10-07: the render consequence is E25 |
| E22 | The draw rule of §9.2: which properties suppress, and the lookup order (per-instance set, then the 0x47 chain, first value found) | BIN (E26) for the order and for `Render_Draw` / `Render_ServerDraw`; Tessera for treating `Render_DrawMeshes` = false and `Render_NeverAddToScene` = true as "not drawn" | Order BIN since 2026-10-07; the two suppressing properties remain a **Decision** (§9.2) |
| E23 | `Entity_CreationType` (enum mapper 0x23000174) decides whether the client creates an entity from a Weenie, Physics or PatrolPoint SED: values 7 and 8 (class Dynamic) are never created by the client from the data, value 0 (or no value) makes the SED an "entity with errors" that is skipped; 0x70 objects are forced to a unique class (§4.9) | BIN (question 8); OBS (census 2026-10-07) | BIN; census Confirmed |
| E24 | The client's property lookup on a SED: the SED's own set, then the named object: a 0x47 object and its parents (first value found wins), or a 0x70 object's property set and then its `PhysObj` 0x47 chain (§4.9) | BIN (question 8) | BIN; no 0x47 or 0x70 object in this install carries `Entity_ContentLayers` (OBS 2026-10-07) |
| E25 | The client's streaming index creates the entities of a landblock's **main-set** nodes (§9.3 step 3) only when their content-layer test passes with the current *L* and the copy's *I* at a pass of that landblock, removes them when it fails at a pass, and creates interior-flag nodes only for a viewer in an interior cell, the others only for a viewer outdoors; long-range and far-view nodes follow other rules (E30) (§9.3) | BIN (questions 7, 12, 13) | BIN. Revised by the repair of 2026-10-07: limited to the main set; the passes are triggered as E31 says, not at every update |
| E26 | An entity built from a SED answers a property lookup from its per-instance set first (the landblock's instance property set for its IID if there is one, otherwise the SED's own set), then from its physics object's properties (§9.2) | BIN (question 9) | BIN |
| E27 | A created entity takes the SED's position and quaternion unchanged, the owning landblock as its block, cell 1 when the interior flag is set and 0 otherwise, the instance number in its IID, and `Physics_AdjustableScale` found by E24 (default (1, 1, 1)) as its scale (§4.4, §4.8, §7, §9.3) | BIN (question 10) | BIN |
| E28 | Census under E23–E25 and E30 in the shared world (*L* = 1, *I* = 0): 2,318,176 of 2,380,699 Physics SEDs are created (1,497,742 without and 820,434 with the interior flag); 62,522 fail the layer test and 1 has creation type 0. Of the created ones 314,957 are long-range nodes (19,471 with the interior flag), 63,377 are main-set nodes that are also far-view nodes, and 2,003,219 are main-set nodes (1,202,256 without and 800,963 with the interior flag, the far-view ones included) (§4.9, §9.3) | OBS (2026-10-07, repair re-check); BIN (rules) | Confirmed |
| E29 | The render node keeps `Render_NeverAddToScene` = true and `Render_DrawMeshes` = false as two flag bits of its own (§9.2) | BIN (question 11) | BIN; what the renderer does with the bits OPEN |
| E30 | The streaming index gives every node a view distance *v*: the larger of the seventh stored float (`d6`) of the node's LOD-class entry (−1 when there is no class or no entry) and `Render_FarViewDistance` (−1 when absent). A Weenie, Physics or PatrolPoint node with *v* ≥ 250.0 goes into the long-range set only; every other node goes into the main set, and also into the far-view set when 0 ≤ *v* ≤ 160.0 (a non-zero *v* below 25.0 then becomes 25.0). Decal and Light nodes ignore the 250 threshold. The long-range set is created after the layer test without the viewer test and is never removed by the pass; the far-view set is created and removed by viewer distance, without the layer test, only while the far-view mode is on (§9.3) | BIN (question 12); OBS (LOD-class table: `d6` = −1 in 31 of 32 entries, 50 in class 0x1000001C) | BIN; census Confirmed |
| E31 | The per-landblock pass runs when the landblock's index is built, on a forced update (viewer moved more than 9 units since the last forced update, or changed region, landblock or instance, or no valid previous position), on revisits after a forced update (only for a viewer kind not yet created there), and when the far-view mode changes; no trigger on a change of *L* alone. Range: the landblocks within *R* of the viewer's block in X and in Y, *R* initialised to 1; a viewer whose position is invalid (region 0, X or Y 0xFF, or cell 0xFFFF) enumerates no landblock (§9.3) | BIN (questions 13, 16) | BIN |
| E32 | The creation list is built from the placement records from last to first; both IID fields of a creation record hold the instance-stamped IID; the same list then receives the landblock's terrain-generated scenery objects as kind-4 records, also from last to first, and the streaming index builds nodes for them by the same rules (§9.3) | BIN (questions 14, 15) | BIN |

---

<a id="s2"></a>
## 2. Where SEDs live

<a id="s2-1"></a>
### 2.1 Landblock-info record (summary; `landblock.md` is normative)

The landblock-info record is decoded in this order (Confirmed: all 49,636 records end exactly
after the last field). Only the parts needed to reach the SED list are summarised here.

| Order | Field | Type | Note |
|---|---|---|---|
| 1 | `did` | DID | Own DID, `0x802RXXYY` |
| 2 | `flags` | u32 | Observed 1 (39,252 records), 3 (9,704), 5 (636), 7 (44). Value 0x1 (bit 0) = cells present, 0x2 (bit 1) = cross-links present, 0x4 (bit 2) = header indices present (`landblock.md` §4.1). |
| 3 | `hint` | u32 | See `landblock.md` |
| 4 | cross-links | TSize count, 24-byte elements | Only when `flags` & 2 |
| 5 | header indices | TSize count, elements u16 + u32-counted u16 list | Only when `flags` & 4 |
| 6 | cells | TSize count, cell elements | Only when `flags` & 1 (bit set in every record; count may be 0) |
| 7 | `reserved` | u32 | 0 in every record |
| 8 | **SED count** *n* | **u32** (not TSize) | 0 in 16,629 records; at most 7,801 (0x8023C57A) |
| 9 | **SEDs** | *n* × §3 | This spec |
| 10 | links | u32 count, link elements | `landblock.md` |
| 11 | properties | embedded property set | `property-system.md` §5 |
| 12 | `reserved` | u32 | 0 in every record |
| 13 | weenies | TSize count, elements u64 IID + embedded property set | **Per-instance property sets** keyed by IID: one for every SED IID plus 514,987 for server objects (§4.5.2; AP). `landblock.md` |
| 14 | trailer | u32, optional | Present in 42,530 records, absent in 7,106 |

<a id="s2-2"></a>
### 2.2 Population (Confirmed)

| Region | Landblock-info records | Of them with ≥ 1 SED | SEDs |
|---|---|---|---|
| 1 | 9,701 | 8,655 | 805,097 |
| 2 | 11,508 | 7,874 | 903,981 |
| 3 | 8,846 | 4,674 | 515,800 |
| 4 | 11,479 | 5,998 | 561,529 |
| 5 | 5,837 | 5,058 | 430,541 |
| 14 | 2,265 | 748 | 36,914 |
| **All** | **49,636** | **33,007** | **3,253,862** |

Counted by the region digit of the landblock-info DID (`0x802RXXYY`); re-checked 2026-10-05.

No other record type was found to embed SEDs. LC also decodes the same structure from
network messages (a "from network" variant whose embedded property set uses the network
form, `property-system.md` §10); that is M7 material.

---

<a id="s3"></a>
## 3. SED layout

<a id="s3-1"></a>
### 3.1 Fields in stream order (Confirmed; BIN for the bits never set)

| Order | Bit | Field | Type | Present in (of 3,253,862) |
|---|---|---|---|---|
| 1 | — | `flags` | u32 | all |
| 2 | 0x001 | `did` | DID (§3.4) | 0 |
| 3 | 0x002 | `type` | u32 enum (§4.1) | all |
| 4 | 0x004 | `iid` | u64 (§4.2) | all |
| 5 | 0x008 | `object` | DID (§4.3) | 3,244,086 |
| 6 | 0x010 | `position` | f32 × 3 (§4.4) | 3,253,835 |
| 6 | 0x010 | `rotation` | f32 × 4, (w, x, y, z) (§4.4) | (with `position`) |
| 7 | 0x020 | `vector` | f32 × 3, a legacy scale (§7) | 0 |
| 8 | 0x080 | `properties` | embedded property set (`property-system.md` §5: TSize count, entries with PID echo) | all |
| 9 | 0x200 | `dids` | VLE count *k* (`records.md` §1.2), then *k* × u32 (DIDs per LC) | 0 |
| 10 | 0x040 | `word` | u32 (§4.6) | 94,335 |
| 11 | 0x100 | `name` | Pascal string (`records.md` §1.4) | 3,242,259 |
| — | 0x400 | *(no payload)* | flag only (§4.8) | 963,922 |

**Order is not bit order** for bits 0x040, 0x080 and 0x200: the property set (bit 0x080)
comes first, then the 0x200 list, then the 0x040 word, then the name. Confirmed for 0x080
before 0x040: every descriptor with bit 0x040 also has bit 0x080, reading the word first
breaks the decode of every such record, and the order above decodes all records exactly.
The place of the 0x200 list is BIN (the client's reader and writer); it was given after the
name in the earlier version of this spec, which would misframe a record that uses it.

<a id="s3-2"></a>
### 3.2 Observed `flags` values (Confirmed census)

| Value | SEDs | Fields beyond flags/type/iid/properties |
|---|---|---|
| 0x0000019E | 2,221,954 | object, position+rotation, name |
| 0x0000059E | 916,198 | object, position+rotation, name, interior flag |
| 0x000001DE | 56,945 | object, position+rotation, word, name |
| 0x000005DE | 37,359 | object, position+rotation, word, name, interior flag |
| 0x0000049E | 7,208 | object, position+rotation, interior flag (no name) |
| 0x00000196 | 6,628 | position+rotation, name (no object) |
| 0x0000009E | 4,364 | object, position+rotation (no name) |
| 0x00000596 | 3,148 | position+rotation, name, interior flag (no object) |
| 0x000000DE | 31 | object, position+rotation, word (no name) |
| 0x0000018E | 18 | object, name (no frame: placed at the origin, §4.4) |
| 0x0000058E | 9 | object, name, interior flag (no frame) |

Bits 0x002, 0x004 and 0x080 are set in every SED. Bits 0x001, 0x020, 0x200 and every bit
≥ 0x800 are never set.

<a id="s3-3"></a>
### 3.3 Unsupported bits

A decoder supports bits 0x001, 0x020 and 0x200 with the sizes and order of §3.1 (they cost
nothing and keep network and older data readable), but rejects any bit ≥ 0x800 as
unsupported: an unknown bit may carry a payload of unknown size, so continuing would
misframe the rest of the landblock-info record. The client's own reader knows no bit above
0x400 (BIN).

<a id="s3-4"></a>
### 3.4 Absent fields and how the flags are chosen (BIN)

The client's writer derives every flag bit from the field's content, and its reader leaves a
field at its default when the bit is clear. A decoder therefore gives an absent field the
default below, and an encoder that wants the client's byte stream sets a bit exactly in the
case in the last column.

| Bit | Field | Value when the bit is clear | The client's writer sets the bit when |
|---|---|---|---|
| 0x001 | `did` | none (invalid DID) | the descriptor's own DID is valid |
| 0x002 | `type` | 0 (`Invalid`) | type ≠ 0 |
| 0x004 | `iid` | none (invalid IID) | the IID is valid |
| 0x008 | `object` | none (invalid DID) | the object DID is valid |
| 0x010 | `position`, `rotation` | position (0, 0, 0), rotation (1, 0, 0, 0) | any of the seven components differs from that default by more than 0.0002 |
| 0x020 | `vector` | (1, 1, 1) | never: the writer does not emit it (§7) |
| 0x080 | `properties` | an empty set | the set has at least one entry |
| 0x200 | `dids` | an empty list | the list is not empty |
| 0x040 | `word` | 0 | word ≠ 0 |
| 0x100 | `name` | empty | the name is not empty |
| 0x400 | interior flag | false | the entity is an interior entity |

Bit 0x001's DID is read into the field every client database object carries for its own
DID; nothing in the SED handling was seen to use it. The 0x200 list holds u32 values; LC
reports them as DIDs; their use was not traced.

---

<a id="s4"></a>
## 4. Field semantics

<a id="s4-1"></a>
### 4.1 `type` (Confirmed values; labels from enum mapper 0x23000091)

The labels are internal identifiers read at run time from enum mapper 0x23000091
(`records.md` §7). The "object" column says what the `object` field names for that type.

| Value | Label | SEDs | `object` | Role |
|---|---|---|---|---|
| 7 | Physics | 2,380,699 | 0x47 (27,571 distinct) | Placed scenery and props: the only type with real geometry to draw (§9). Also the **static NPC-like figures**: a 0x47 whose setup is a humanoid rig, dressed, animated and given held props by the SED's instance property set (§4.5.2; `appearance.md` §7.2–§7.4). |
| 3 | GeneratorPoint | 415,238 | 0x47 (239 distinct; 1 m editor markers) | Spawn point |
| 23 | PatrolPoint | 268,896 | 0x47 (8 distinct markers) | AI patrol waypoint |
| 9 | Light | 73,711 | 0x47 (2 distinct markers) in 72,880; none in 831 | Light source; parameters in properties (`LightDiffuse`, `LightRange`, `LightFalloff`, day/night variants). The client creates a light object from it (BIN, §9; `lighting.md`). |
| 19 | SegmentNode | 55,645 | 0x47 (1 marker) | Path segment node |
| 6 | Weenie | 33,676 | **0x70** (775 distinct) | Server-side objects, **not NPCs**: 744 housing hooks (32,338 SEDs), 29 distant-battle unit groups (1,335 SEDs), one door (1 SED) and one monster (2 SEDs) (AP; §4.3) |
| 2 | Decal | 16,511 | 0x47 (140 distinct small markers) | Projected decal; parameters in properties (`DecalVolume_*`). The client creates a decal object from it (BIN, §9). |
| 16 | TeleportDestination | 8,945 | none | Arrival point |
| 31 | LBIOnly | 541 | 0x47 (1 marker) | Unknown |

The mapper also defines labels that no SED uses (Invalid 0, PathBlock 1, LinkDesc 4,
PathHint 5, SceneFile 8, UIScene 10, Link 11, Scriptlet 12, Script 13, EntityGroup 15,
Landblock 18, KynMapSeedPoint 21, Cell 22, Export 24, Particle 25, DBObj 26, Camera 27,
WorldBuilderOnly 32). A decoder keeps unknown values as numbers.

<a id="s4-2"></a>
### 4.2 `iid`: instance ID (Confirmed, 100 % of SEDs; bits 24–35 BIN)

The 64-bit instance ID is structured:

| Bits | Content |
|---|---|
| 56–63 | Kind byte: 1 (3,217,157 SEDs) or 4 (36,705; 33,605 of them Weenies, the rest Physics 1,918, GeneratorPoint 657, PatrolPoint 344, SegmentNode 140, TeleportDestination 24, Light 16, Decal 1) |
| 52–55 | Region *R* of the owning landblock |
| 44–51 | Landblock X of the owning landblock |
| 36–43 | Landblock Y of the owning landblock |
| 24–35 | **Instance number** of an instanced copy of the landblock (BIN); 0 in every SED and every landblock instance set (3,253,862 and 3,768,840) |
| 0–23 | Serial within the landblock (starts at 1; usually contiguous) |

Bits 36–55 equal the owning landblock-info DID's region, X and Y in every SED. When the
client builds the entities of an instanced copy of a landblock (instance number *I*, 1 ≤ *I*
< 4,096), it writes *I* into bits 24–35 of the IIDs it builds for that copy (entities and
the references between them), so the copies do not collide (BIN; it does so for kind bytes 1, 4, 5, 9, 10, 0x82 and 0x83,
refuses *I* ≥ 4,096 and leaves other kinds unchanged). The stored data always describe the
shared copy (*I* = 0).

3,253,853 distinct IIDs; 9 IIDs occur twice (in different landblocks). Links and weenies
refer to entities by IID (`landblock.md`), so a world model indexes SEDs by IID and must
tolerate a duplicate (keep both; resolve a link to the one in the same landblock first).

Weenie SEDs are kind 4 except 71 (kind 1). The instance property sets (§4.5.2) of IIDs that
no SED places are kind 1 (103,833) or kind 4 (411,154).

> **OPEN:** Meaning of the kind byte (1 versus 4). The client treats both alike for
> instancing (above); no other use was traced.

<a id="s4-3"></a>
### 4.3 `object`: what is placed (Confirmed)

- **0x47 physics object** (3,210,410 SEDs; 27,962 distinct DIDs, every one exists in
  `client_gamelogic.dat`). Resolve it with `setup.md` §3.1: 0x47 → (0x47)* → 0x1F →
  (0x1F)* → 0x04 setup → 0x06 parts. Observed chains: 0x47 → 0x1F → 0x04 for 26,512
  distinct objects, one extra hop (0x47 → 0x47 or 0x1F → 0x1F) for 1,450. Two objects
  (0x47001031, 0x47003E76; 20 SEDs) end at setups without parts.
- **0x70 game object** (33,676 SEDs, all of type Weenie; 775 distinct DIDs, all exist). The
  object's property set (DID + 0x09000000, `property-system.md` §6.2) carries `PhysObj`, a
  0x47 DID, in all 775; resolve that as above. What they are (AP, from their `WeenieType`):
  744 housing hooks (`Entity`, class index 1989, small one-part marker setups), 29
  distant-battle unit groups (`DistantBattleUnit`, class index 3725, one marker entity
  0x47006915), one door (`Door`, 0x4700C408) and one monster (`Monster`, a humanoid rig with a
  random-appearance type; landblock 0x80253E07). None is an NPC with species, worn or held
  items; `appearance.md` §7.1 gives the details and `walker.md` §10.2 what the walker draws.
- **Absent** (9,776 SEDs: all 8,945 TeleportDestination and 831 Light). Nothing to resolve.

Meshes are never named directly by a SED; geometry always comes through the physics object.

<a id="s4-4"></a>
### 4.4 `position` and `rotation` (Confirmed)

**Frame.** The position is in the frame of the **owning landblock** (the one whose
landblock-info record holds the SED):

- *x* grows towards landblock X + 1 (east), *y* towards landblock Y + 1 (north), *z* is up
  (`mesh.md` §11: right-handed, Z-up, metres).
- *x* = 0, *y* = 0 is the landblock's corner with the smallest X and Y (south-west).
- *z* is the **absolute height** in metres on the same scale as terrain (§8), not an
  offset from the ground.

So the global position inside the region is (160 · X + *x*, 160 · Y + *y*, *z*). §8 confirms
this against terrain.

**Range.** 2,479,964 SEDs (76.2 %) have 0 ≤ *x*, *y* ≤ 160. The other 773,871 lie outside
the landblock square, between six landblock widths below and five above (about −960 to
+960 m). 772,963 of those (99.9 %) carry the interior flag (§4.8): they are interior and
dungeon contents placed in the owning landblock's frame, not outdoor objects spilling into a
neighbour (only about 190 of them sit within 2 m of the neighbour's terrain). A loader must
not clamp or re-home such positions. By landblock: in landblocks whose landblock-info record
has cells, 768,911 of 1,001,648 framed SEDs are out of the square and all of them carry the
interior flag; in landblocks without cells, 4,960 of 2,252,187 are, 4,052 with the flag and
908 without. The 908 out-of-square SEDs without the flag (886 Physics, 22 Light) are all in
landblocks without cells (`coordinates.md` §7, CO2).

**Rotation.** Four f32 forming a unit quaternion (all 3,253,835 within 0.001 of length 1),
stored **w first**: (w, x, y, z), the same order as `setup.md` §6.7. It rotates object space
into the landblock frame as an **active** rotation (§6 gives the evidence and §5 the
transform). The client reads the position and the four values, in stored order, into the
same frame type it uses for every position; its identity is (1, 0, 0, 0) (BIN). When it
creates the SED's entity it copies the three position and four rotation values unchanged
into the entity's position (E27, §9.3): no snapping of *z* to the ground and no extra turn,
so a placed figure stands at the stored height and faces along its rotated model +Y axis.

- 3,115,694 rotations (95.8 %) are pure turns about +Z (x = y = 0 within 1e-4). The rest
  tilt the object (rocks, debris, leaning props).
- For a pure turn the yaw angle ψ = 2 · atan2(z, w) is counter-clockwise seen from above.
  An object's local +Y axis (the forward direction of characters, `mesh.md` §11.1) then
  points at compass bearing (360° − ψ) mod 360°, measured clockwise from north.
- **This is the client's heading rule** (BIN, answers owner item O2): the heading the
  client prints for a frame (`/loc` `h`) is minus its yaw, in degrees, reduced to
  0 ≤ h < 360 (`coordinates.md` §11.3–§11.4). An avatar facing exactly along a SED's model
  +Y axis therefore reports the SED's bearing. For a tilted rotation the client's yaw is the
  z-y-x yaw of `coordinates.md` §11.3, which can differ from the bearing of the projected +Y
  axis.
- The identity is (1, 0, 0, 0). Counts of SEDs whose first component w, or whose last
  component z, is ±1, by the tolerance allowed on |component| − 1:

  | Tolerance | w = ±1 | z = ±1 (genuine 180° turns) |
  |---|---|---|
  | exact | 824,440 | 170,728 |
  | 10⁻⁶ | 828,104 | 173,093 |
  | 10⁻⁵ | 833,084 | 178,174 |
  | 10⁻⁴ | 849,744 | 192,157 |

**No frame** (27 SEDs; flags 0x18E and 0x58E: 24 Physics, 2 Light, 1 PatrolPoint). The
bit is clear because the frame equals the default: the SED stands at the owning landblock's
**origin** (0, 0, 0) with the **identity rotation** (BIN, §3.4). A renderer places it there;
it does not skip it. Five of the 24 Physics SEDs (landblocks 0x8024D05B, 0x8024D05C,
0x8024D15C) name meshes whose vertices already lie in and just around the block square (x
and y between about −19 and 171 m) at heights of 447–769 m: geometry authored in the
landblock frame, which only the origin placement puts in the right spot. None of the 24 is draw-suppressed (§9).

<a id="s4-5"></a>
### 4.5 `properties` (Confirmed census)

An embedded property set (`property-system.md` §5.1, without the leading DID), decoded with
the master dictionary. Every SED has one, with 2 to 26 entries (2: 1,171,086 SEDs; 3: 819,478;
4: 604,741; …). 275 distinct PIDs occur. The most common:

| Property | Type | SEDs | Notes |
|---|---|---|---|
| `Render_AdjustableScale` | vector | 3,253,862 | Scale (§7) |
| `Physics_AdjustableScale` | vector | 3,253,862 | Equal to the render scale in every SED (§7) |
| `Entity_Clearance_BoxList` | array | 516,726 | Clearance volumes |
| `Render_LODClass` | enum | 455,455 | Detail-level class |
| `Render_CastLandscapeShadows` | boolean | 415,917 | |
| `Render_FarViewDistance` | float | 384,845 | Per-instance draw distance |
| `Appearance_InstanceList` | array | 380,072 | Per-instance appearance (material variants; `setup.md` part 2) |
| `Entity_ContentLayers` | array of `Entity_ContentLayer` | 354,497 | Content (phasing) layers, §4.5.1 |
| `AI_PhysicallyWanders`, `AI_WanderingRange` | boolean, float | 279,745, 266,521 | Generator/AI data |
| `Render_NeverOcclude` | boolean | 218,531 | |
| `Render_CastBakedStencilShadows` | boolean | 198,617 | |
| `Entity_Clearance_CylinderList` | array | 140,995 | |
| `Physics_NoPhysics` | boolean | 96,446 | No collision for this instance |
| `Render_Color` | colour | 89,735 | Instance tint |
| `LightDiffuse`, `LightRange`, `LightDiffuseIntensity`, `LightFalloff` | colour, float, …, float | 62,048–74,167 | Light parameters |
| `TimeOfDay@Entity.m_eTimeOverride` | enum 0x23000161 | 42,429 | Time-of-day visibility (`environment.md`) |
| `WindSpeed` | enum 0x23000362 | 18,334 | Wind animation class |
| `Render_Draw`, `Render_DrawMeshes` | boolean | 467, 328 | Per-instance draw override (§9) |

<a id="s4-5-1"></a>
#### 4.5.1 Content layers (Confirmed values; rule BIN; answers owner item O3)

`Entity_ContentLayers` is an array whose elements are `Entity_ContentLayer` enum values. The
enum's data field names DID map 0x28000012, the `CONTENT_LAYER` map of `property-system.md`
§8.1 (root key 9). Its keys are the layer values; their labels, read at run time, are
interoperability identifiers:

| Key | Label | Meaning (BIN) |
|---|---|---|
| 0 | `Undef` | no layer |
| 1 | `InstanceZero` | the shared, non-instanced world |
| 2 | `All` | every layer and every instance |
| 0x1000xxxx (1,658 keys) | named layers | phases and private encounters, made active by the server |

**Census** (all 3,253,862 SEDs, their own property set):

| List | SEDs | Of them Physics |
|---|---|---|
| no `Entity_ContentLayers` | 2,899,365 | 2,253,509 |
| empty list | 859 | 818 |
| names 2 (`All`) | 111,047 | 3,894 |
| names 1 (`InstanceZero`), not 2 | 166,263 | 59,956 |
| names 0 | 0 | 0 |
| names only 0x1000xxxx layers | 76,328 | 62,522 |

List lengths: 0 (859), 1 (320,090), 2 (28,764), 3 (2,762), 4 (2,018), 5 (4). 5,234 lists
combine 1 with named layers (for example [1, named] 2,279 and [named, 1] 1,635); one list
combines 2 with a named layer. 966 distinct named layers occur; the most common are
0x100002A8 (21,103 SEDs) and 0x100002A7 (20,875). By type, the lists naming 2 are mostly on
PatrolPoint SEDs (106,898). The SED's instance property set (§4.5.2) carries the same list
in 354,496 of the 354,497 SEDs that have one.

**The test** (BIN). Inputs: the SED's property set, the active content layer *L* (one
value) and the instance number *I* of the copy being built. The SED **exists** when

1. the property is absent, or its value is not an array, or the array is empty; or
2. some element *e* of the array satisfies one of:
   - *e* = 2;
   - *e* = 1 and (*I* = 0 or *L* = 1);
   - *e* = *L* (this also covers *e* = 0 when *L* = 0, and a named layer when it is active).

Where *L* and *I* come from in the client: *L* is the world's current content layer, which
the client stores together with the player's position whenever that position is set (in
the original an input from the server; 0 until it is first set); *I* is the instance number
of the landblock copy or of the player's position (0 for the shared world). Decal and light
objects are tested with the landblock copy's own instance number. When the client
enumerates a landblock's weenie SEDs for a given layer, it treats layer 1 as the shared
world and every other layer as an instanced copy (*I* ≠ 0), so a list naming only 1 then
fails unless *L* = 1.

Physics SEDs (E21, BIN): when the client loads a landblock copy, one pass handles the
Weenie, Physics and PatrolPoint SEDs. For each one that passes the loader's own validity
checks (SEDs that fail them are reported as entities with errors; §4.9) it builds a
**placement record** holding the SED's creation type (§4.9), the result of the test, a copy
of the SED and its clearance volumes (from `Entity_Clearance_BoxList` and
`Entity_Clearance_CylinderList`). The test uses *I* = the instance number of the landblock
copy being loaded and *L* = a value handed in by the loader's caller (a virtual method of
the owning object on three of the four call paths, a value read from the global world
object on the fourth; it is *L* because the test uses it as *L*, its source was not traced
further). The record is built whether the test passes or not. The layer list the test reads
is found by the lookup order of §4.9 (the SED's own set first); no physics object or game
object in this install carries `Entity_ContentLayers`, so in practice it is the SED's own
list (OBS 2026-10-07).

**The drawn objects** (E25, E30, E31, BIN; closes the former OPEN of this section). The
entities a player sees are created from these placement records by the streaming index of
§9.3, and the test governs them in two places. First, a record whose test failed at load
time never enters the creation list. Second, at each **pass** of a landblock (§9.3 step 4;
a pass runs when the landblock's index is built, on a forced update of the viewer position
and on the revisits that follow it, not at every update and not when *L* alone changes) the
client re-runs the test for the landblock's nodes with its current *L* and the landblock
copy's *I*:

- a **main-set** node whose test passes is created if it does not exist (subject to the
  interior rule of §4.8), and one whose test fails is removed if it exists;
- a **long-range** node (view distance 250 or more, §9.3 step 3) whose test passes is
  created if it does not exist; a failing test does **not** remove it;
- **far-view** nodes are not tested at all; they are created and removed by distance, and
  only while the client's far-view mode is on (§9.3 step 4, off by default).

Decal and Light SEDs are indexed the same way and tested at the same passes (they are not
filtered at load time; they are never long-range). So, for Physics SEDs as for the others:

- (a) in the shared world (*L* = 1 or 0, *I* = 0) the 62,522 Physics SEDs whose lists name
  only named layers are **absent** (they never enter the creation list);
- (b) in an instanced copy (*I* ≠ 0) a list naming only 1 passes only while *L* = 1, and a
  list naming 2 always passes;
- (c) under an active named layer *L* a list naming *L* passes. When *L* later changes to a
  value the list does not name, a main-set entity disappears at the next pass of its
  landblock (which a change of *L* alone does not trigger, §9.3 step 4); a long-range
  entity stays.

Census of the test for Physics SEDs (2026-10-07, layer lists found by §4.9): with (*L*, *I*)
= (1, 0) or (0, 0), 2,318,177 pass and 62,522 fail; in an instanced copy with *L* = 1
(*I* = 5 taken as an example) the same 2,318,177 pass; with *L* = 0 and *I* = 5, a further
59,956 (the lists naming 1 but not 2) fail.

**What the open world shows** (answers O3). In the shared world with the shared-world layer
(*L* = 1, *I* = 0) the client shows SEDs with no list, with an empty list, and with a list
naming 1 **or** 2, and hides lists that name only other layers; this now holds for every
traced kind, including the drawn objects of Physics SEDs (above). Layers 1 and 2 are
therefore not alternatives: both sets are present together. A list naming 2 is present in
every layer and every instance; a list naming only 1 disappears in instanced copies unless
*L* = 1 (and, for weenies, whenever another layer is active); named layers appear only when
the server makes one of them active, which is what depends on a character's progress.

**Tessera default** (offline, no server): *L* = 1, *I* = 0, which reproduces the shared
world. An option that makes named layers active for inspection applies the test once per
active layer and shows the union (Tessera convention).

> **Correction:** the earlier working assumption "draw SEDs without layers plus those that
> list layer 1" hid the lists naming only 2: 111,047 SEDs, 3,894 of them Physics (3,256
> not draw-suppressed). The original shows them everywhere. Empty lists: confirmed to count
> as "no list" (shown), as `tessera-world` already does.

**Confirmation spot** (optional, for O3): region 1, landblock 0xA15B (info 0x8021A15B),
map coordinates about 51.4S, 17.7W. SED #41 is a large group of trees (about 20 × 21 ×
28 m; physics object 0x4700379C) at (55.07, 32.04, 346.13) and SED #36 a ruined wall corner
at (75.55, 39.42, 348.57); both list only layer 2 and are present for every character.
SEDs #32–#35, #37, #38 and #43 nearby (rubble and rock outcrops, for example #37 at
(86.90, 42.07, 345.36)) list only the named layer 0x1000008D and are absent unless the
server activates that layer.

<a id="s4-5-2"></a>
#### 4.5.2 Per-instance property sets (Confirmed; folded in from `appearance.md` §7.2)

The landblock-info record's weenie list (§2.1 row 13) is an **instance-property table**:
3,768,840 (IID, property set) pairs. 3,253,853 carry the IID of a SED of the same landblock
(every distinct SED IID once), so every SED has one instance property set; it holds at least
`Render_AdjustableScale` and `Physics_AdjustableScale` (all 3,768,840 do) and repeats the
SED's `Entity_ContentLayers`. The other 514,987 belong to IIDs no SED places (server-spawned
objects and generators; `Generator_ProfileDID` in 33,902).

On Physics SEDs the instance sets carry what makes a static figure: `Appearance_InstanceList`
(379,660), `NPC_StartingIdleAnimation` with `NPC_PlayingStartingIdle` (51,113),
`NPC_Cosmetic_Inventory_Item1` (18,009) and `Appearance_RandomAppearanceType` (about
43,000); `appearance.md` §7.2–§7.4 says how to dress them. Precedence (E26, BIN 2026-10-07):
an entity built from a SED answers a property lookup from its per-instance set first and
from its physics object's properties second, so an instance property wins over the
property of the same name on the SED's object (this confirms `appearance.md` §7.4 step 4,
which was a Decision). The instance set **replaces** the SED's own property set as the
entity's per-instance layer: when the landblock has an instance set for the entity's IID,
the SED's own set is not consulted at all, and the two are never merged; only when there is
no instance set does the SED's own set take its place (§9.2).

<a id="s4-6"></a>
### 4.6 `word` (bit 0x040; Confirmed values, meaning OPEN)

Physics SEDs: value 1 in all 60,659 that have it. Weenie SEDs (all 33,676 have it): 1
(26,810), 2 (5,066), 3 (1,145), 4 (459), 5 (105), 6 (65), 7 (22), 9 (2), 8 (1), 11 (1).
An absent word is 0 (the client's writer omits a zero word, §3.4; BIN). The value shows no
relation to the number of links that name the SED (census of 2026-10-05).

> **OPEN:** Meaning (a spawn count or a version are candidates for weenies). The client
> stores it as a plain u32; its users were not traced.

<a id="s4-7"></a>
### 4.7 `name` (Confirmed)

A Pascal string (VLE length, Latin-1). Lengths 0–116 bytes; all but 5 are printable ASCII.
These are world-building labels, not player-visible text: 2,787,252 of them end in the
suffix `.entity` (Physics, GeneratorPoint, Decal, SegmentNode, PatrolPoint, LBIOnly); the
others are short tool labels. Tools may display them; the client must not depend on them.

<a id="s4-8"></a>
### 4.8 Interior flag (bit 0x400; no payload)

Bit 0x400 has no payload (§3.1). It marks entities that belong to interior cells rather
than the outdoor landscape. Evidence (Confirmed statistically): each cell of landblock
info has a position and a 0x01 cell-geometry record (`landblock.md`); its bounding box,
moved into the landblock frame, contains 642,918 SED positions with the bit set and 5
without. Of SEDs with the bit, only 51 lie within 2 m of the terrain surface. 963,922 SEDs
(29.6 %) have the bit; 958,925 of those with a position are in landblocks with cells.

The client keeps the bit as a boolean (BIN). When it builds decal and light objects from
SEDs (§9), and when it builds the creation records of Weenie, Physics and PatrolPoint SEDs
(§9.3, E27), it gives them the cell number 1, the first interior cell value, when the bit is
set and 0 (outdoors) otherwise; the SED itself names no particular cell. The two values are
the client's start-up constants of `coordinates.md` §7 (outdoors = 0xFFFF + 1 in 16 bits,
interior = outdoors + 1; read from the static initialisers 2026-10-07).

What the bit changes (BIN, §9.3): for a **main-set** node, the streaming index creates an
interior-flag entity only in a pass for a viewer position that lies in an interior cell
(cell neither 0 nor 0xFFFF), and an entity without the flag only in a pass for a viewer
position that is outdoors (cell 0, or 0xFFFF or invalid in a forced pass). Entities already
created stay until their landblock leaves the range or a later pass finds their layer test
failing. The bit does **not** restrict **long-range** nodes (view distance 250 or more,
§9.3 step 3): they are created in any pass whose layer test passes, whatever the viewer's
cell. In the shared world 820,434 of the 2,318,176 created Physics SEDs carry the flag
(OBS 2026-10-07); 314,957 of the created ones are long-range, 19,471 of those with the flag.
So, counted over the whole world before the range rule of §9.3, passes for an outdoor
viewer create at most 1,517,213 Physics entities (the 1,202,256 main-set ones without the
flag plus all 314,957 long-range ones) and passes for an interior viewer at most 1,115,920
(800,963 plus 314,957); entities that earlier passes created for the other viewer kind stay
until their landblock leaves the range.

The client's general position normaliser (`coordinates.md` §4) treats the value 1 as "inside,
cell not yet known": for a position with cell 1 it asks a cell-lookup service, with the
position, for the cell that contains it, and stores the answer as the position's cell
(BIN 2026-10-07; the lookup service itself was not traced).

> **OPEN:** Which concrete cell a SED's entity is attached to, and when: the creation record
> carries cell 1 and the normaliser can resolve it by a containment lookup (above), but the
> lookup's rule (volume test, tolerance, which cells are searched) and whether the streamed
> entity's position is normalised at creation were not traced. 4,994 SEDs with the bit live
> in landblocks without cells: 6 without a frame, 936 with a frame inside the square and
> 4,052 outside it (`coordinates.md` §7); which cells, if any, they belong to is not known.

<a id="s4-9"></a>
### 4.9 `Entity_CreationType`: which SEDs the client creates (BIN; census Confirmed 2026-10-07)

**Lookup order** (E24). Whenever the client reads a property "of a SED" (the creation type,
the layer list, the scale, the far-view distance), it searches:

1. the SED's own property set;
2. then, if the SED names a 0x47 physics object, that object's property set, then its parent
   0x47 objects in turn (the template link of `setup.md` §4), stopping at the first value
   found and at the first level that is not a 0x47 object;
3. or, if the SED names a 0x70 game object (Weenie SEDs): the game object's property set
   (DID + 0x09000000, `property-system.md` §6.2), then the 0x47 object named by its
   `PhysObj` and that object's parents as in step 2.

The SED's instance property set (§4.5.2) is **not** part of this lookup; it serves the
created entity instead (§9.2, E26).

**The property.** `Entity_CreationType` is an enum property whose labels come from enum
mapper 0x23000174 (read at run time; interoperability identifiers). The client groups its
values into three classes, whose labels come from enum mapper 0x23000175 (1 `Dynamic`,
2 `Static`, 3 `ClientOnly`), and separately marks some values as "unique":

| Value | Label (0x23000174) | Class | Unique |
|---|---|---|---|
| 0 | `Invalid` | Static (2) | no |
| 1 | `UniqueClientOnlyMobile` | ClientOnly (3) | yes |
| 2 | `SharedClientOnlyRigid` | ClientOnly (3) | no |
| 3 | `SharedStaticImmobile` | Static (2) | no |
| 4 | `UniqueClientOnlyRigid` | ClientOnly (3) | yes |
| 5 | `SharedClientOnlyImmobile` | ClientOnly (3) | no |
| 6 | `UniqueClientOnlyImmobile` | ClientOnly (3) | yes |
| 7 | `UniqueDynamicMobile` | Dynamic (1) | yes |
| 8 | `UniqueDynamicImmobile` | Dynamic (1) | yes |
| 9 | `UniqueStaticImmobile` | Static (2) | yes |
| any other | — | Static (2) | no |

**Reading it** (BIN). For a SED of type Weenie, Physics or PatrolPoint (the client accepts
the types 0, 6, 7, 19, 23 and 31 here and gives every other type the value 0):

1. Look the property up by the order above. A value that is not an integer or enum counts
   as 0.
2. If the SED's object is a 0x70 game object: when the value is not unique, replace it by 6
   if its class is ClientOnly and by 8 otherwise; then, when the class of the (possibly
   replaced) value is Static, replace it by 8. Game objects therefore always end in a unique
   class, ClientOnly (6) or Dynamic (7, 8).

**Validity** (BIN). The landblock loader's pass (§4.5.1) rejects a Weenie, Physics or
PatrolPoint SED, reporting it as an entity with errors and building no placement record,
when its object DID is invalid, when the creation type is 0, or when a Weenie SED's final
class is Static (impossible after step 2).

**Consequence** (E23, §9.3). Placement records whose class is Dynamic never enter the
creation list: the client does not create those entities from the data (in the original
they come from the server). Static and ClientOnly records do enter it and are created by the
client itself, subject to the content-layer test.

**Census** (all SEDs, 2026-10-07; value after step 2):

| Type | Creation type: SEDs | Not created in the shared world (and why) |
|---|---|---|
| Physics (2,380,699) | 0: 1; 1: 903; 3: 977,753; 5: 492,656; 6: 118,238; 9: 791,148 | 62,523: 62,522 fail the layer test, 1 has creation type 0 (#173 of 0x802505EC, whose own set says 0 while its object says 3; the SED's value wins) |
| PatrolPoint (268,896) | 3: 268,768; 7: 1; 9: 127 | 10,520: 10,519 fail the layer test, 1 is Dynamic (#458 of 0x80231799) |
| Weenie (33,676) | 6: 33,676 (33,673 store 6, 3 store 5 and are raised to 6) | 792, all failing the layer test |

No Physics SED is Dynamic, so every static NPC-like figure is created by the client itself
when its layer test passes (§9.4). Physics SEDs by class: Static 1,768,902 (values 0, 3 and
9), ClientOnly 611,797 (1, 5 and 6).

---

<a id="s5"></a>
## 5. Placement transform (Confirmed)

To place one part vertex *v* (object space, `setup.md` §7) of a SED owned by landblock
(*R*, *X*, *Y*):

1. Multiply *v* by the uniform scale *s* (§7).
2. Rotate the result by the SED quaternion *q* = (w, x, y, z) as an active rotation
   (rotation of a vector by *q* followed by the conjugate of *q*).
3. Add the SED position (*x*, *y*, *z*); a SED without a frame uses (0, 0, 0) and the
   identity (§4.4).
4. Add the landblock origin (160 · *X*, 160 · *Y*, 0) to obtain region coordinates. A
   renderer subtracts a camera-relative origin before converting to f32 (ER §4: rendering
   is camera-relative), because region coordinates reach about 40,800 m.

Parts of a multi-part setup share the setup's frame (`setup.md` §7), so they all use the same
transform. Object-space normals take the rotation only (uniform scale needs no inverse
transpose).

> **OPEN:** The 0x1F template fields that look like scale (`setup.md` §5.2: `vec3`,
> `value_a`, `value_b`) and the 0x47 `block` (`setup.md` §4.4). 27 of the 27,962
> referenced physics objects reach a template with one of them. Whether they multiply into
> *s* is not established; the validation in §6–§8 ignored them.

---

<a id="s6"></a>
## 6. Rotation order and direction (Confirmed)

Test: fences, walls and similar segments are placed end to end. For every pair of SEDs in
the same landblock naming the same physics object whose part bounds are elongated (one
horizontal extent at least four times the other and at least 2 m), take each SED's nearest
neighbour of the same object. When their distance is within 10 % of the scaled long extent,
the offset between them must be parallel to the object's long axis after rotation. Pure
turns that are multiples of 90° (±10°) were excluded because they cannot tell readings
apart (32,118 pairs).

| Reading of the four floats | Pairs aligned within 3° (of 23,240) |
|---|---|
| (w, x, y, z), active rotation | **13,601** |
| (w, x, y, z), inverse rotation | 256 |
| (x, y, z, w), active | 405 |
| no rotation | 405 |

The remaining pairs are neighbours that are not end-to-end (corners, parallel rows). Only the
first reading explains the chains, so the order is (w, x, y, z) and the rotation is active
(object space to landblock frame). The client's reader stores the values in this order and
its identity is (1, 0, 0, 0) (BIN, §4.4).

---

<a id="s7"></a>
## 7. Scale (Confirmed)

- Every SED carries both `Render_AdjustableScale` and `Physics_AdjustableScale` (vector
  properties). In all 3,253,862 SEDs they are equal and uniform (x = y = z).
- Values: from 0.01 to 100. 6,458 distinct f32 values; 1,925 distinct after rounding to 3
  decimals, where each f32 is widened to f64, multiplied by 1,000 and rounded half away
  from zero to an integer (formatting each value with three decimals gives the same 1,925).
  Doing the multiplication in f32 instead gives 1,935, because some values lie within f32
  rounding of a half-thousandth. 1.0 exactly in 2,275,093 SEDs (69.9 %), and 1.000 to 3
  decimals in 2,292,193 (70.4 %); then 1.5, 0.5, 0.85, 0.75, 2.0, 0.8 ….
- Evidence that it applies: in the chain test of §6 restricted to SEDs with *s* ≠ 1, aligned
  nearest neighbours sit at the scaled extent *L*·*s* in 6,013 cases and at the unscaled *L*
  in 1,198 (the rest are not end-to-end).
- Some 0x47 property sets also contain `Render_AdjustableScale` (`setup.md` §4.4: 1,309
  objects). The SED always has its own value, so the SED value is used.

A decoder reads the scale from the SED property set; if it is ever missing, it uses 1. If
the three components ever differ, it applies them per axis before the rotation (never
observed); normals then take the inverse transpose of the scale (divide by it per axis,
rotate, renormalise) rather than the rotation alone. The physics scale is for collision
and is applied the same way.

**Legacy scale vector (bit 0x020; BIN, never set in this install).** Its three f32 default
to (1, 1, 1). After reading a SED, when the vector was present and any component differs
from 1 by more than 0.0002, the client stores it as **both** `Physics_AdjustableScale` and
`Render_AdjustableScale` in the SED's property set, replacing values already there. The
client's writer never emits the bit, so re-encoded data carries the scale only as
properties.

**Which value the client uses** (BIN 2026-10-07; closes the former OPEN "replaces or
multiplies"). The scale of a created entity is a single lookup of `Physics_AdjustableScale`
by the order of §4.9 (the SED's own set first, then the physics object's chain), with
(1, 1, 1) when nothing is found (E27). The SED's value therefore **replaces** a value on the
physics object; no product of the two was seen on the creation path. On this install every
SED has its own value, so the object's value is never used.

---

<a id="s8"></a>
## 8. Validation against terrain (Confirmed)

Terrain height used here (normative statement belongs to `landblock.md`): the landblock data
record 0x800RXXYY holds a 33 × 33 grid of u16 samples at 5 m spacing, sample (*i*, *j*) at
index 33 · *i* + *j*, where *i* steps along *x* and *j* along *y*; the height in metres is
**raw / 32.768** (raw × 1000 / 32768). Evidence: 1,385 SEDs placed exactly on a grid vertex
and within 0.5 m of that height have a median residual of 0.004 m and a fitted scale of
1 / 32.7665. With the grid transposed (index 33 · *j* + *i*) the ground-cover test of §8.1
drops from 82.8 % to 18.5 % within 0.5 m (median error 4.7 m instead of 0.11 m).

<a id="s8-1"></a>
### 8.1 Ground cover

192,661 outdoor grass and ground-cover SEDs (selected in the dirty room by their labels),
compared with the bilinearly interpolated terrain at their (*x*, *y*):

| | Value |
|---|---|
| Within 0.1 m | 48.5 % |
| Within 0.5 m | 82.8 % |
| Median absolute difference | 0.108 m |
| Least-squares fit *z* = a · raw + b (inliers) | a = 1 / 32.7677, b = 0.013 m |

The residue sits on stairs, bridges, terraces and other placed geometry. Bilinear and the two
triangle splits of a grid cell fit equally well (within 0.004 m of median error); ground
cover cannot decide the terrain interpolation rule (`landblock.md`).

<a id="s8-2"></a>
### 8.2 Buildings

Population: 6,838 Physics SEDs whose labels denote buildings, whose part bounds are at least
6 m × 6 m × 4 m after scaling, in the 3,779 "open-country" landblocks (at least 20 outdoor
Physics SEDs, of which ≥ 80 % lie within 2 m of terrain). For each, the four bottom corners
of the part bounds were transformed by §5 and the terrain sampled under them.

| Check | Result |
|---|---|
| Origin *z* within 1 m of terrain under the origin | 62.5 % |
| Origin *z* within 2 m | 73.7 % |
| Origin *z* − terrain, percentiles 5 / 25 / 50 / 75 / 95 | −10.3 / −1.67 / −0.16 / +0.01 / +1.12 m |
| Mesh bottom at or below terrain (+0.25 m) under all four corners | 89.0 % |

Buildings are sunk slightly into the ground with their floor near terrain level, as
expected. The tail below −2 m is buildings set into slopes, cellars and ruins.

<a id="s8-3"></a>
### 8.3 Sample (20 buildings, random draw from §8.2)

Heights in metres. "Terrain at corners" is the minimum and maximum terrain height under the
four rotated bottom corners. Yaw is ψ (§4.4).

| Landblock info | SED index | Physics object | Scale | Position *x*, *y*, *z* | Yaw ψ | Terrain at origin | Mesh bottom | Terrain at corners |
|---|---|---|---|---|---|---|---|---|
| 0x80215479 | 149 | 0x47000D7A | 1.00 | 159.92, 133.67, 377.10 | 101.0 | 376.99 | 376.38 | 376.99 – 377.08 |
| 0x80215971 | 2 | 0x47000E23 | 1.00 | 17.94, 99.54, 409.25 | 210.0 | 410.79 | 407.83 | 408.68 – 416.23 |
| 0x80215E72 | 27 | 0x47000DD9 | 1.00 | 98.89, 29.61, 407.71 | 300.0 | 408.04 | 407.30 | 407.22 – 408.14 |
| 0x80216478 | 27 | 0x47000D91 | 1.00 | 61.96, 121.38, 380.76 | 243.8 | 380.95 | 379.14 | 379.93 – 383.91 |
| 0x80216574 | 67 | 0x47001013 | 1.00 | 140.17, 107.02, 366.10 | 180.0 | 368.58 | 365.25 | 364.47 – 370.45 |
| 0x80217871 | 21 | 0x470012A1 | 0.85 | 92.59, 154.70, 387.36 | 90.0 | 387.33 | 387.19 | 387.16 – 387.93 |
| 0x80217F5C | 140 | 0x4700BFB4 | 0.85 | 53.09, 61.51, 361.30 | 180.0 | 363.34 | 360.96 | 362.78 – 363.53 |
| 0x8022565B | 33 | 0x47005834 | 1.00 | 37.78, 140.58, 313.39 | 275.7 | 313.39 | 310.62 | 313.39 – 313.39 |
| 0x8022614B | 2 | 0x47006848 | 1.00 | 66.46, 34.49, 398.80 | 270.0 | 389.98 | 398.49 | 389.98 – 389.98 |
| 0x80226F5E | 23 | 0x470060B0 | 1.00 | 84.47, 12.23, 317.73 | 180.0 | 317.53 | 316.31 | 316.89 – 318.61 |
| 0x8022A5BB | 1 | 0x47003417 | 0.85 | 40.69, 8.76, 669.73 | 0.0 | 666.61 | 669.01 | 665.36 – 672.45 |
| 0x80236D4B | 1 | 0x47006E3C | 1.00 | 91.35, 143.23, 330.80 | 225.0 | 330.78 | 330.41 | 330.47 – 330.95 |
| 0x802375DE | 95 | 0x4700701D | 1.95 | 9.37, 85.27, 163.54 | 63.7 | 165.65 | 161.64 | 165.65 – 165.65 |
| 0x802376B2 | 20 | 0x4700701D | 1.20 | 99.22, 123.56, 208.79 | 226.1 | 208.86 | 207.62 | 208.84 – 208.92 |
| 0x802378E9 | 105 | 0x4700701D | 2.15 | 53.06, 47.76, 167.95 | 349.5 | 170.70 | 165.86 | 170.44 – 170.99 |
| 0x8023A5B2 | 52 | 0x4700701D | 1.20 | 4.85, 151.27, 202.94 | 226.1 | 202.83 | 201.77 | 202.80 – 203.28 |
| 0x8023A7B3 | 63 | 0x47004B64 | 1.75 | 126.50, 148.30, 150.38 | 316.7 | 155.29 | 143.71 | 151.49 – 154.89 |
| 0x8023D5E7 | 187 | 0x4700701D | 2.60 | 99.28, 148.39, 150.18 | 347.0 | 153.37 | 147.64 | 152.37 – 154.82 |
| 0x8024AF4D | 20 | 0x47007848 | 1.00 | 15.94, 122.06, 165.80 | 38.4 | 165.85 | 161.99 | 165.80 – 166.16 |
| 0x80258B62 | 26 | 0x4700F208 | 2.45 | 10.25, 38.69, 212.45 | 86.1 | 212.43 | 210.54 | 212.44 – 215.14 |

17 of 20 have their mesh bottom at or below the terrain (+0.25 m) under every corner.
0x8022614B #2 floats 8.5 m above a perfectly flat terrain patch (raised on other geometry,
or a flattened heightmap under a placed platform); 0x8022A5BB #1 and 0x80216574 #67 overhang
a slope at one corner (by 3.6 m and 0.8 m). These rows can serve as the M3 reference points for object placement (ROADMAP M3 exit 4)
once the same spots are checked in the original client.

---

<a id="s9"></a>
## 9. What a world walker shows

Static geometry comes from **Physics** SEDs only. The other types name editor marker
objects (one-metre boxes, light bulbs, arrows; §4.1) that the original does not show in play.
The original does, however, **realise Decal and Light SEDs**: after the content-layer test
it builds a decal object, or a light object, at the SED's frame, with the cell chosen by the
interior flag (§4.8) (BIN). Their markers are not drawn. Lights are specified in
`lighting.md`; decals by a later spec.

<a id="s9-1"></a>
### 9.1 Draw properties: census (Confirmed)

Three boolean properties look like draw switches by name. Their definitions in the property
dictionary (`property-system.md` §2.2) carry the defaults `Render_Draw` true,
`Render_DrawMeshes` true and `Render_NeverAddToScene` false (and `Render_ServerDraw`, §9.2,
true). On the physics objects named
by SEDs (looked up along the 0x47 chain, `setup.md` §3.1):

| Property | Value meaning "do not draw" (§9.2) | SEDs naming such an object (all types) |
|---|---|---|
| `Render_Draw` | 0 | 854,615 |
| `Render_DrawMeshes` | 0 | 186,921 |
| `Render_NeverAddToScene` | 1 | 740,551 |
| any of the three (579 of the 27,962 named objects) | | 1,027,172 |

By type, the SEDs naming such an object are: all GeneratorPoint, PatrolPoint, SegmentNode
and LBIOnly SEDs, and 286,852 of the 2,380,699 Physics SEDs (invisible walls, ground-cover
removal volumes, entity-group markers, sound emitters, occluders). Light (9) and Decal (2)
markers are not flagged but are markers (§4.1).

Where the values sit, for the 27,571 objects named by Physics SEDs:

- **Along the chain:** no object has any of the three properties on more than one level of
  its 0x47 chain (28,151 levels in all), and none of the 17,484 0x78 extra sets those
  levels name (`setup.md` §4.4) contains any of them. So the order in which a chain is
  searched cannot change any result on this install.
- **On the SED:** `Render_Draw` occurs on 467 Physics SEDs (0: 329, 1: 138) and
  `Render_DrawMeshes` on 328 (0: 245, 1: 83); `Render_NeverAddToScene` on none. The SED's
  instance property set (§4.5.2) carries the same property with the same value in every
  one of these cases, and nowhere else.
- **SED against object:** the two disagree for **12** Physics SEDs only: the SED says
  `Render_Draw` = 1 while its object says 0. In the other 455 SEDs with `Render_Draw` the
  object has no value (448) or the same value (7); every SED with `Render_DrawMeshes` names
  an object without it.
- `Render_ServerDraw` (§9.2) occurs on no SED, no instance property set and no object named
  by a Physics SED.

<a id="s9-2"></a>
### 9.2 Draw rule

**What the client is known to do (E20, BIN).** A render node keeps a "hidden" flag that it
re-evaluates whenever `Render_Draw` or `Render_ServerDraw` changes on it. The node is drawn
unless the effective value is false, where the effective value is `Render_ServerDraw` if the
node's property lookup finds it, otherwise `Render_Draw` if found, otherwise true. The
lookup is dispatched to the object that owns the node.

**Lookup order of the owning entity (E26, BIN 2026-10-07; closes the former precedence
OPEN).** Every entity the client builds from a SED (§9.3) carries a property component.
Apart from values stored on the entity itself at run time, which are searched first (none
come from the data at creation), it has two sources, searched in this order, the first value
found winning:

1. its **per-instance set**: the landblock's instance property set for the entity's IID
   (§4.5.2) if the landblock has one, otherwise the SED's own property set (the two are not
   merged: when an instance set exists, the SED's own set is not consulted);
2. its **physics object's** properties (the 0x47 object and its parents, first value found).

So a value on the SED (or its instance set) wins over the object's, and along the chain the
level nearest the named object wins. On this install every SED IID has an instance set
(§4.5.2) and it carries the same draw values as the SED (§9.1), so the 12 Physics SEDs on
which SED and object disagree (`Render_Draw` = 1 on the SED, 0 on the object) are
**drawn**: SEDs #1594, #1598, #1600, #1785, #1786, #1791, #1793, #1796, #1799 and #1802 of
landblock info 0x8023FCDB (all naming 0x47000DD1), #1695 of 0x8021FCF3 (0x47006D6D) and
#649 of 0x8022FAD2 (0x47000C82; its layer list fails the shared-world test, so it is not
created there anyway).

**`Render_DrawMeshes` and `Render_NeverAddToScene` (E29, BIN).** The render node's
property-change handler keeps each as a flag bit of its own, separate from the hidden flag
above: `Render_NeverAddToScene` = true sets one bit (false clears it), `Render_DrawMeshes`
= false sets another (true clears it). Neither changes the hidden flag.

> **OPEN:** what the renderer does with those two node bits (skip the node, skip its meshes
> but keep children or effects, keep it out of a scene list only). The consumers of the
> bits were not found. Tessera treats both as "not drawn" (step 3 below, Decision); on this
> install 186,921 SEDs name an object with `Render_DrawMeshes` = 0 and 740,551 one with
> `Render_NeverAddToScene` = 1 (§9.1), almost all of them markers.

**Tessera's rule (E22).** The lookup order is the client's (E26); treating the two flags of
E29 as suppressing is a project decision:

1. For each of `Render_Draw`, `Render_DrawMeshes` and `Render_NeverAddToScene`, the
   effective value is the first one found in this order: the per-instance set (the SED's
   instance property set if there is one, otherwise the SED's own property set), then the
   0x47 chain from the named object towards its parents (first value found wins), then the
   dictionary default (§9.1). (BIN, E26.)
2. `Render_ServerDraw` is looked up the same way; if found, it replaces the effective
   `Render_Draw` (as in the client's node test above). It occurs nowhere on this install.
   (BIN, E20.)
3. A SED is **suppressed** when the effective `Render_Draw` is false, the effective
   `Render_DrawMeshes` is false, or the effective `Render_NeverAddToScene` is true.
   (Decision for the last two, OPEN above.)

Draw a SED if its type is Physics, it passes the content-layer test of §4.5.1 (Tessera
default *L* = 1, *I* = 0), the client would create it (§4.9: creation type not 0, class
not Dynamic), and it is not suppressed. A SED without a frame is drawn at the landblock
origin with the identity rotation (§4.4). Interior-flag SEDs follow the viewer rule of §9.3.

The creation-type condition removes exactly one SED from the drawn count below: #173 of
0x802505EC (creation type 0, interior flag set, not suppressed), so the client-faithful
shared-world count is **2,041,836**.

Counts under this rule without the creation-type condition (Confirmed census): 287,392 of the 2,380,699 Physics SEDs are
suppressed (the 286,852 that name a suppressing object, minus the 12 the SED un-suppresses,
plus 552 that only the SED's own value suppresses). 2,041,837 Physics SEDs are drawn in the
shared world (2,093,307 if content layers were ignored). Compared with the M3 rule (lists had
to name 1; frameless SEDs skipped) this adds 3,256 Physics SEDs whose lists name 2 and 24
frameless Physics SEDs, none of them suppressed: 2,041,837 instead of 2,038,557 (the "about
2,093,800" of the first version counted without the layer filter). The two alternatives
counted earlier are now excluded by E26 (object value winning: 2,041,826; SED value ignored:
2,042,327).

Static NPC-like figures are among these Physics SEDs: their object resolves to a humanoid rig
and their instance property set dresses and animates them (§4.5.2; `appearance.md` §7.2–§7.4).
Weenie SEDs are housing hooks, distant-battle units, a door and a monster (§4.3); what
Tessera shows for them is `walker.md` §10.2.

> **OPEN (roadmap, not format):** when Tessera realises decals and lights from their
> properties (the original does, above). The light parameters are in `lighting.md`; decal
> volumes (`DecalVolume_*`) have no spec yet.

<a id="s9-3"></a>
### 9.3 How the client creates a landblock's entities (BIN 2026-10-07)

This is the path from a SED to an entity in the world, as the client runs it in normal play
(E21, E23, E25, E27). It is given so that a walker can reproduce **which** placed objects
exist at a given moment; drawing them is §9.2 and `walker.md`.

**1. Placement records** (at landblock load, §4.5.1). One record per Weenie, Physics and
PatrolPoint SED that passes the validity check of §4.9: creation type, content-layer result
(load-time *L*, the copy's *I*), a copy of the SED, clearance volumes.

**2. Creation list** (E32). The client walks the placement records **from the last to the
first** and keeps those whose creation class is not Dynamic **and** whose load-time layer
test passed. For each it appends a creation record, so the list holds the kept records in
reverse record order:

| Field | Value |
|---|---|
| kind | 2 ("from a SED"; terrain-generated scenery uses kind 4, `scenery.md`) |
| IID, second IID | both the SED's IID with the copy's instance number written into bits 24–35 (§4.2); the unstamped IID is not kept |
| object | the SED's object DID |
| block | the owning landblock (region, X, Y) and instance |
| cell | 1 if the interior flag is set, else 0 (§4.8) |
| position, rotation | the SED's stored values, unchanged (§4.4) |
| scale | `Physics_AdjustableScale` by the lookup of §4.9, default (1, 1, 1) (§7) |
| creation type | the value of §4.9 |

After the last SED record the same list receives the landblock's terrain-generated scenery
objects (`scenery.md` §7.6), also from the last to the first, as records of **kind 4** with
an IID synthesized from the landblock and the object's place in the generated list. They are
not filtered by creation class here. The streaming index of step 3 builds a node for each of
them by the same rules as for the SED records; their IIDs name no SED, so the layer list and
the view distance come from the named object's own properties. `scenery.md` owns which
objects these are and their frames.

**3. Streaming index** (E30). For each landblock the client builds an index node per
creation record, in list order, and then one per Decal and Light SED of the landblock, in
SED order (these are not filtered at load time). A node holds the record's data, its
content-layer list and a **view distance** *v*, both looked up by the order of §4.9 on the
SED (found by its IID with the instance bits cleared, so every copy uses the shared
landblock's SED; when no SED is found, on the named object's own properties):

1. *v* starts at −1.0.
2. If the lookup finds `Render_LODClass` (an enum or integer value) and the LOD-class table
   (`mesh.md` §8.5 step 2, layout `environment.md` §9.1) has an entry for it, *v* becomes the
   entry's **seventh** stored float, `d6` in `environment.md`'s naming. (The client keeps
   that float in the last slot of its in-memory copy of the entry, after `d7`.) In this
   install `d6` is −1 in 31 of the 32 entries and 50 in class 0x1000001C; read it at run
   time.
3. If the lookup finds `Render_FarViewDistance` as a float and that value is larger than
   *v*, *v* becomes that value; any other type, or no value, counts as −1.
4. The node is **long-range** when *v* ≥ 250.0. It is **far-view** when 0 ≤ *v* ≤ 160.0;
   a far-view node with 0 < *v* < 25.0 gets *v* = 25.0 (a *v* of exactly 0 stays 0). The
   node stores *v*².

Which sets a node joins:

- a Weenie, Physics or PatrolPoint node that is long-range joins the **long-range set**
  only (not the main set, not the far-view set);
- every other node of these types joins the **main set**, and also the **far-view set**
  when it is far-view;
- a Decal or Light node joins the main set always (the 250 threshold does not apply to
  them), and the far-view set when it is far-view.

Each set is keyed by the instance-stamped IID; a node whose IID is already in that set
replaces the earlier node there. With the list order of step 2, when two Weenie, Physics or
PatrolPoint SEDs of a landblock share an IID and join the same set, the one earlier in
record order is the one kept (a Decal or Light node, indexed after them, replaces either).

Census for the shared world (OBS, repair re-check 2026-10-07; created Physics SEDs, layer
test passed): long-range 314,957 (19,471 with the interior flag; every one of them because
of `Render_FarViewDistance` ≥ 250, none because of an LOD class); main set 2,003,219, of
them 63,377 also far-view (2,454 with *v* = 0 and 60,923 with 25 ≤ *v* ≤ 160; no *v*
between 0 and 25 occurs, so the raise to 25 never applies here). 149 created Physics SEDs
have the class 0x1000001C (`d6` = 50). Weenie SEDs: 546 long-range; PatrolPoint SEDs: 4
long-range. No Decal or Light SED is far-view.

**4. Update** (E25, E30, E31). The streaming manager keeps the viewer position it was last
given (which object supplies it is OPEN below) and a range *R* in landblocks.

*Range* (BIN). *R* is initialised to 1 when the manager is built; no other writer of it was
found in the manager's code. When the manager enumerates landblocks around the viewer, it
takes, for a viewer whose cell is 0 (outdoors) and whose instance lies outside 0x800–0xFFF,
every valid landblock (X + *i*, Y + *j*) with \|*i*\| ≤ *R* and \|*j*\| ≤ *R* in the viewer's
region and instance (3 × 3 landblocks for *R* = 1); for any other viewer (an interior cell,
or an instance in 0x800–0xFFF) it takes the viewer's own landblock only. A viewer whose
position is invalid (region 0, X or Y = 0xFF, or cell 0xFFFF, the "no valid cell" value;
`coordinates.md` §4 and §7, rows C16–C17) enumerates **no** landblock, so a forced update for such a viewer runs the forced pass on no
landblock and requests none; the rest of trigger 2 below is unchanged. A landblock already
indexed counts as **in range** (after a validity check) when its instance equals the
viewer's and its X and Y each differ from the viewer's by at most *R* (for a landblock whose instance lies in 0x800–0xFFF:
only when it is the viewer's own landblock and instance).

*Triggers* (E31). The per-landblock update below runs:

1. when a landblock's data has loaded and its index has just been built, with the current
   viewer position (an ordinary update);
2. on a **forced update**: when the manager is given a viewer position and, compared with
   the position stored at the previous forced update, the region, the landblock X or Y or
   the instance differs, or the squared distance between the two exceeds 81.0 (more than 9
   units), or there is no valid previous position. The manager then runs a forced update on
   every enumerated landblock whose index is ready, requests the missing ones (they arrive
   by trigger 1), queues every indexed landblock for a revisit and stores the position;
3. on **revisits**: at every position update the manager works through the revisit queue
   within a time budget of 2,000 µs per update (2,000 times a factor that is 1.0 by
   default; the unit is read from the timer arithmetic), each as an ordinary update; a
   landblock found out of range is scheduled to be discarded 19 s later;
4. when the far-view mode is switched on or off: a forced update of every indexed
   landblock.

There is no trigger on a change of the content layer *L* alone, nor on a change of the
viewer's cell without one of the changes of trigger 2.

*The per-landblock update.*

1. **Out of range.** If the landblock is not in range, remove the entities of its main-set
   nodes for whichever viewer kinds were created (below) and the entities of all its
   far-view nodes, clear both "created" marks, and stop. Long-range entities are not removed
   here.
2. If the landblock's index is not ready, stop.
3. **Viewer kind and the set pass.** The viewer is **interior** when its position is valid
   and its cell is neither 0 nor 0xFFFF, otherwise **outdoor**. The set pass (steps 4–6)
   runs in a forced update; in an ordinary update it runs only when the viewer is interior
   and the landblock's interior mark is not set, or when the viewer's cell is exactly 0 and
   the outdoor mark is not set. Otherwise go to step 7.
4. **Long-range nodes.** For each long-range node: evaluate the content-layer test of §4.5.1
   with the node's list, the client's current *L* and the landblock copy's *I*; when it
   passes and the node's entity does not exist, create it. No viewer-kind condition; a
   failing test does nothing.
5. **Main-set nodes.** For each main-set node: evaluate the same test; if it fails, remove
   the node's entity if it exists; if it passes and the node's entity does not exist,
   create it when the node's cell matches the viewer kind (interior viewer: node cell valid
   and neither 0 nor 0xFFFF, that is cell 1; outdoor viewer: node cell 0, 0xFFFF or
   invalid), except that while step 7 is enabled for the landblock, a node that is also in
   the far-view set is left to step 7.
6. Set the mark of the viewer kind (interior or outdoor) for this landblock.
7. **Far-view nodes.** Only while the far-view mode is on and either the landblock-info
   property `Render_AllowDistanceBasedCreation` is true or a second manager switch is on:
   for each far-view node, with *d*² the squared distance from the viewer position to the
   node's position (across landblock boundaries), remove the node's entity when
   *v*² × *s* ≤ *d*² (*s* a manager factor, 1.0 at construction), and otherwise create it if
   it does not exist. Neither the layer test nor the viewer kind applies here. This step runs
   at every per-landblock update of an in-range, ready landblock, whether or not step 3 ran
   the set pass.

The far-view mode and the second switch are both off when the manager is built; what
switches them on was not traced (OPEN below). `Render_AllowDistanceBasedCreation` is absent
from all 49,636 landblock-info property sets of this install (OBS 2026-10-07). With the mode
off, step 7 never runs and far-view nodes are ordinary main-set nodes.

A node of kind Decal or Light becomes a decal or light object (§9, `lighting.md`); any other
node becomes an entity built by the entity factory from the record's object DID, IIDs and
scale, at the record's position (E27), with the property component of §9.2 (E26).

**Consequences a walker reproduces:** (a) a Weenie, Physics or PatrolPoint SED failing the
load-time layer test never exists, in any form; (b) Dynamic SEDs never exist without a
server; (c) a main-set interior-flag SED is created only in a pass for a viewer inside an
interior cell, but a long-range one (view distance 250 or more) is created whatever the
viewer's cell, so an outdoor viewer can have interior-flag entities (19,471 Physics SEDs in
the shared world); (d) placed objects take their stored frame unchanged; (e) a change of
*L* takes effect for a landblock's main-set nodes only at its next set pass, and never
removes long-range entities; (f) with the far-view mode off (the default), the view
distance only decides which nodes are long-range.

> **OPEN:** what removes long-range entities: the per-landblock update does not, neither on
> a failed layer test nor when the landblock leaves range; discarding the landblock's index
> (scheduled 19 s after it leaves range) was not traced. **Tessera default:** remove them
> with their landblock, as for main-set entities.

> **OPEN:** what switches the far-view mode and the second manager switch on (a preference,
> the world loader, or a per-region setting), and the source of the factor *s*; what the
> manager's separate global list (a second distance-driven set, possibly the far-view entity
> list 0x0E100001 of `records.md` §2.1) holds. **Tessera default:** far-view mode off.

> **OPEN:** whether anything outside the manager's own code changes *R* in play, and which
> object supplies the viewer position (the avatar's or the camera's). Tessera keeps the
> walker's ranges (`walker.md` §3–§4) and uses the camera cell of `landblock.md` §4.12
> (Decision there); a client-faithful mode uses *R* = 1 with the rules above.

<a id="s9-4"></a>
### 9.4 Static NPC figures: presence, position and facing (answers owner item O17)

Static NPC-like figures are Physics SEDs (or, for N6, the one monster Weenie SED) whose
object resolves to a character rig (§4.1, `appearance.md` §7). Under §4.9 and §9.3 the client
creates them like any other placed object: there is no separate NPC path for them, no
ground snapping, no extra turn. Their presence is the content-layer test plus the interior
rule; their position and facing are the stored frame.

Census (2026-10-07): 52,267 Physics SEDs have an instance property set carrying
`NPC_StartingIdleAnimation` or `Appearance_RandomAppearanceType`; in the shared world 48,177
of them are created (class Static 41,200, ClientOnly 6,977) and 4,090 are not, all because
their layer lists name only named layers; 9,392 of the created ones carry the interior flag.
By §9.3 step 3, 2,955 of the 48,177 are long-range (6 of them with the interior flag) and
45,222 are main-set nodes (3,405 of those also far-view); so 9,386 interior-flag figures are
created only by a pass for a viewer inside an interior cell, and the 6 long-range ones
whatever the viewer's cell (OBS, repair re-check 2026-10-07). (Two Weenie SEDs, the N6 monster, also
match and are not created in the shared world.)

**The N spots of `docs/checklists/m4-visual.md`** (rows N1–N7; values read from the install
2026-10-07; heading *h* is the compass bearing of the figure's model +Y axis, clockwise from
north, by `coordinates.md` §11.3–§11.4; ψ is the z-y-x yaw):

| Row | Landblock info, IID | Creation type (class) | Layer list | Interior | Present in the shared world | ψ | Facing *h* |
|---|---|---|---|---|---|---|---|
| N1 | 0x80211B8F, 0x0111B8F00000002C | 9 (Static) | none | no | yes | 131.39° | 228.6° |
| N2 | 0x8021678A, 0x011678A0000000A5 | 9 (Static) | none | no | yes | 136.93° | 223.1° |
| N3 | 0x80213FAA, 0x0113FAA0000000C8 | 9 (Static) | none | no | yes | −90.00° | 90.0° |
| N4 | 0x80213B85, 0x0113B85000000019 | 9 (Static) | [0x10000019] | no | **no**: only while the server makes layer 0x10000019 active | 105.00° | 255.0° |
| N5 | 0x80214F79, 0x0114F79000000076 | 9 (Static) | [1] | no | yes (absent in an instanced copy unless *L* = 1) | −125.28° (tilted rotation) | 125.3° (projected +Y axis: 125.8°) |
| N6 | 0x80253E07, 0x0453E0700000001B (and …1A) | 6 (ClientOnly; stored 5, raised by §4.9 step 2) | [0x10000536] | no | **no**: only while layer 0x10000536 is active | 153.26° (…1A: 143.50°) | 206.7° (…1A: 216.5°) |
| N7 | 0x802235F6, 0x04235F600000001D | 6 (ClientOnly) | none | no | yes (a hidden distant-battle marker, `walker.md` §10.2) | 178.61° | 181.4° |

Each figure stands at its stored position (block-local *x*, *y*, *z* of `m4-visual.md`; *z*
is its foot height, not snapped to the terrain).

What still needs the owner (O17): whether the server activates layers 0x10000019 and
0x10000536 for a given character (that is server state, not client code); and the look of
each figure (pose, dress, held props), which `appearance.md` and the animation specs
govern, not this one.

---

<a id="s10"></a>
## 10. Decoding algorithm (in words)

For each SED, with the landblock-info cursor positioned after the SED count:

1. Read the u32 `flags`. If any bit ≥ 0x800 is set, stop with "unsupported SED flags" for the
   whole record.
2. If bit 0x001: read a DID. If bit 0x002: read the u32 type. If bit 0x004: read the u64
   IID. If bit 0x008: read the object DID.
3. If bit 0x010: read three f32 (position) and four f32 (rotation, w first); otherwise the
   position is (0, 0, 0) and the rotation (1, 0, 0, 0).
4. If bit 0x020: read three f32 (legacy scale, §7).
5. If bit 0x080: read an embedded property set (`property-system.md` §5.3), with the
   dictionary.
6. If bit 0x200: read a VLE count *k*, then *k* u32 values.
7. If bit 0x040: read a u32 (otherwise the word is 0).
8. If bit 0x100: read a Pascal string.
9. Bit 0x400: nothing to read; record it.
10. If step 4 read a vector that differs from (1, 1, 1) by more than 0.0002 in any
    component, store it as both scale properties (§7).

Defensive limits: bound the SED count by the remaining bytes divided by 4 (a SED is at
least its flags word), and apply an absolute cap (the
largest observed count is 7,801; a cap of 65,536 is ample). Bound the name length by the
remaining bytes (observed ≤ 116) and the list count by remaining / 4. Reject a
non-finite position or rotation component as malformed; renormalise a rotation whose length
differs from 1 by more than 0.01 and report it (never observed).

> **OPEN (decoder policy):** Tessera additionally (a) rejects a position component of
> magnitude above 10⁶ units as malformed (observed positions stay within about ±960 in x, y
> and at terrain heights in z, §4.4), so hostile values cannot overflow world transforms;
> (b) normalises **every** rotation, not only those more than 0.01 off, so the placement
> never scales geometry, while keeping the stored value for re-encoding and reporting a
> rotation more than 0.001 off unit length as an ED5 deviation; (c) ignores a scale vector
> (§7) with any component outside [10⁻⁴, 10⁴] (zero, negative, non-finite or absurd) and
> uses 1. None of these occurs in this install.

---

<a id="s11"></a>
## 11. Invariants (Confirmed)

| ID | Invariant |
|---|---|
| **ED1** | `flags` ⊆ {0x002, 0x004, 0x008, 0x010, 0x040, 0x080, 0x100, 0x400}, with 0x002, 0x004 and 0x080 always set (plus the BIN-defined 0x001, 0x020, 0x200, never observed). |
| **ED2** | Every landblock-info record decodes to its last byte with the SED layout of §3 (49,636 / 49,636). |
| **ED3** | Bits 36–55 of every IID equal the owning landblock's region, X and Y; bits 24–35 are 0 (every SED and every instance property set). |
| **ED4** | Type 6 (Weenie) SEDs name 0x70 objects; every other SED with an object names a 0x47 record. Every named record exists. |
| **ED5** | Rotations have unit length (±0.001). |
| **ED6** | `Render_AdjustableScale` and `Physics_AdjustableScale` are present, uniform and equal, with values in [0.01, 100]. |
| **ED7** | An SED whose position lies outside the 160 m square has the interior flag 0x400 (772,963 of 773,871; the exceptions are a census item, not an error). Every exception is in a landblock whose landblock-info record has no cells (908 of 908); in landblocks with cells the rule has no exception (768,911 of 768,911). |
| **ED8** | No `Entity_ContentLayers` list names layer 0; every SED IID has an instance property set in the same landblock-info record, whose layer list (when present) equals the SED's (354,496 of 354,497). |
| **ED9** | A SED without bit 0x010 decodes to position (0, 0, 0) and rotation (1, 0, 0, 0) (27 SEDs). |
| **ED10** | With the lookup order of §4.9, every Weenie, Physics and PatrolPoint SED finds an `Entity_CreationType`; after step 2 of §4.9 the per-type counts of the §4.9 census hold; no Physics SED is Dynamic, every Weenie SED ends at 6; no 0x47 or 0x70 object carries `Entity_ContentLayers` (OBS 2026-10-07). |
| **ED11** | In the shared world (*L* = 1, *I* = 0) the creation rule of §9.3 creates 2,318,176 Physics SEDs, 1,497,742 of them without the interior flag (OBS 2026-10-07). By the view distance of §9.3 step 3, 314,957 of them are long-range (19,471 with the interior flag) and 2,003,219 are main-set nodes (1,202,256 without the flag), 63,377 of which are also far-view (OBS, repair re-check 2026-10-07). |
| **ED12** | In the LOD-class table 0x32000000 the seventh stored float (`d6`) of every entry is −1 except one (class 0x1000001C, 50); no created SED becomes long-range through its class alone; no created SED has a view distance strictly between 0 and 25; `Render_AllowDistanceBasedCreation` occurs in no landblock-info property set (OBS, repair re-check 2026-10-07). |

---

<a id="s12"></a>
## 12. Statistics and failure categories

| Measure | Value |
|---|---|
| Landblock-info records decoded / total | 49,636 / 49,636 (0 trailing bytes) |
| SEDs | 3,253,862 |
| Distinct 0x47 objects named / resolved to meshes | 27,962 / 27,960 (2 part-less setups) |
| Distinct 0x70 objects named / with `PhysObj` | 775 / 775 |
| SEDs with a content-layer list / of them shown in the shared world (§4.5.1) | 354,497 / 278,169 |
| Physics SEDs drawn by the §9.2 rule (shared world) | 2,041,836 with the creation-type condition (2,041,837 without it; M3 rule: 2,038,557; content layers ignored: 2,093,307) |
| Physics SEDs the client creates in the shared world (§9.3) | 2,318,176 of 2,380,699; long-range 314,957 (19,471 interior-flag), main set 2,003,219 (1,202,256 without the flag; 63,377 also far-view); at most 1,517,213 created by passes for an outdoor viewer and 1,115,920 for an interior one |

Failure categories a decoder distinguishes: truncated SED, unsupported flag bit, property-set
failure (unknown PID or bad echo, `property-system.md` §5.3), count over limit, non-finite
transform; and for the resolver: missing physics object, unreadable record, chain too long or
cyclic (`setup.md` §3.1). None occur in this install.

Tessera's decoder (`crates/formats::landblock::StaticEntity`, inside `LandblockInfo`)
reproduces the census on this install: 49,636 of 49,636 records and all 3,253,862 SEDs
decode, with the type counts of §4.1; ED4 and ED6 hold for every SED; 773,871 SEDs lie
outside the 160 m square, 908 of them without the interior flag (ED7). The decoder
renormalises a rotation more than 0.01 from unit length and flags the SED as unusual
(never happens here). See `landblock.md` §9.1.

---

<a id="s13"></a>
## 13. Open questions

> **OPEN:** IID kind byte 1 versus 4 (§4.2).

> **OPEN:** Meaning of the 0x40 word (§4.6).

> **OPEN:** Which concrete cell an interior SED's entity is attached to (the creation record
> carries the generic interior cell 1; the client's normaliser can resolve it by a
> containment lookup whose rule was not traced); interior SEDs in landblocks without cells
> (§4.8).

> **OPEN:** Template-level scale fields and the 0x47 `block` versus the SED scale (§5). (The
> SED-versus-object part of §7 is answered: the SED value replaces the object's.)

> **OPEN (roadmap):** when Tessera realises decals and lights (§9).

> **OPEN:** The use of the `did` (0x001) and `dids` (0x200) fields, never set in this
> install (§3.4).

> **OPEN:** What the renderer does with the node bits set by `Render_DrawMeshes` = false and
> `Render_NeverAddToScene` = true (§9.2; Tessera treats both as "not drawn", Decision).

> **OPEN:** Whether anything outside the manager's own code changes the range *R*
> (initialised to 1), and whether its viewer position is the avatar's or the camera's (§9.3;
> the range rule and the rule for an interior viewer are now given there).

> **OPEN:** What removes long-range entities, and what switches the far-view mode on (§9.3).
> Tessera defaults: remove them with their landblock; far-view mode off.

Answered in the 2026-10-05 revisions: the bearing of placed objects against the `/loc`
heading (§4.4, owner item O2); the content-layer defaults and the empty-list case (§4.5.1,
owner item O3); the meaning of the `vector` field (§7); SEDs without a frame (§4.4).

Answered in the repair revision of 2026-10-07 (BIN fourth use): the view-distance threshold
of the streaming index (250) and the long-range and far-view sets (§9.3 step 3); the triggers
of the per-landblock pass (no trigger on a change of *L* alone) and the range rule (§9.3
step 4); the creation list's order and IID fields (§9.3 step 2).

Answered in the 2026-10-07 revision (BIN third use): the content-layer test governs the
drawn objects of Physics SEDs in every case (shared world, instanced copies, named layers;
§4.5.1, §9.3; former OPEN, owner items O3 and O17); which SEDs the client creates at all
(`Entity_CreationType`, §4.9); the precedence of the SED's (instance set's) value over the
physics object's for the draw properties, so the 12 disagreeing SEDs are drawn (§9.2; former
OPEN); that the SED scale replaces the object's (§7; former OPEN); that interior-flag
entities of the main set are created only for a viewer in an interior cell (§4.8, §9.3;
long-range ones are exempt, repair revision); the frame, cell and scale
of created entities and the presence and facing of the N1–N7 figures (§4.4, §9.3, §9.4;
owner item O17).

---

<a id="s14"></a>
## 14. Corrections to other specs (not edited here)

| Spec | Statement there | Finding (this spec) |
|---|---|---|
| `walker.md` §1 (`--content-layers`), §10.2 table N4/N6 note; `tessera-world` layer filter | Active layers default `[1]`; a SED passes when its list names an active layer | Use the test of §4.5.1: lists naming 2 always pass, lists naming 1 pass in the shared world (*I* = 0), named layers only when active. The current filter rejects all 111,047 SEDs whose lists name 2; 3,256 of them are drawable Physics SEDs that the original shows (most of the rest are markers). |
| `walker.md` §4 (static objects); `tessera-world` placement | SEDs without a frame are skipped (from the earlier §4.4) | Place them at the landblock origin with the identity rotation (§4.4; 24 drawable Physics SEDs). |
| `landblock.md` §4.8 | Landblock weenies are server-side templates | Per-instance property sets, one per SED IID plus server objects (already corrected by `appearance.md` §7.2; §4.5.2 here); they repeat the SED's content-layer list. |
| `landblock.md` §6.3 (`UI_Map_ContentLayer`) | Value 1 in 1,873 elements, otherwise named layers | Value 1 is the `InstanceZero` layer label of §4.5.1 (labels only; how the minimap chooses a variant tile was not traced). |
| `lighting.md` (point lights from Light SEDs) | Light SEDs as placed | The client creates a light object per Light SED only when the SED passes the content-layer test of §4.5.1 (Light SEDs: 73,143 without a list, 16 naming 1, 5 naming 2, 547 named only); the light's cell is 1 (interior) or 0 by the interior flag (§4.8). |
| `docs/checklists/owner-observations.md` O2, O3, O17 | Observations needed | O2 is answered by BIN (§4.4); a reading is a confirmation only. O3 is answered by BIN for every kind, including the drawn objects of Physics SEDs (§4.5.1, §9.3, revision of 2026-10-07); the confirmation spot of §4.5.1 is optional. O17 is answered by BIN for presence, position and facing (§9.4: N1–N3, N5 and N7 present, N4 and N6 absent unless the server activates their layers; facings 228.6°, 223.1°, 90.0°, 255.0°, 125.3°, 206.7°, 181.4°); the owner is still needed for the server's layer state per character and for the figures' look. |
| `walker.md` §4, §10.2; `tessera-world` drawable set | Physics SEDs passing the layer test and not suppressed are drawn | Also leave out SEDs the client does not create (§4.9): Physics #173 of 0x802505EC (creation type 0) and PatrolPoint #458 of 0x80231799 (Dynamic). Shared-world Physics count 2,041,836 instead of 2,041,837. |
| `walker.md` §4 (`--interiors`), OPEN of 2026-10-07 item (c)–(d) | Tessera choices: interior entities loaded only while the camera is in a cell of their landblock; from inside, no outdoor object; interior lights only from inside | The client's rule (§9.3, BIN): main-set interior-flag entities are created only in a pass for a viewer in an interior cell, the others only for a viewer outdoors; long-range entities (view distance 250 or more; 19,471 interior-flag Physics SEDs in the shared world) are created whatever the viewer's cell; already-created entities stay until their landblock leaves range or a later pass finds their layer test failing. Decal and Light SEDs follow the main-set rule. For an interior viewer the client enumerates only the viewer's own landblock on a forced update, and *R* = 1 otherwise (§9.3 step 4). The walker's choices agree on creation for the main set; "no outdoor object from inside" is not the client's rule: outdoor entities created earlier stay while their landblock is in range. |
| `lighting.md` (point lights from Light SEDs) | Light SEDs tested once | The layer test of a Light SED is re-run at each set pass of its landblock (§9.3 step 4: when the index is built, on forced updates and their revisits; not on a change of *L* alone) with the current *L*; a light whose test fails at such a pass is removed. Light SEDs are never long-range. |
| `walker.md` §11 OPEN on `Render_FarViewDistance` | The client combines the property with a value of the LOD-class entry and compares the result with 250 and 160 m; not traced further | Traced for creation (§9.3 step 3, E30): *v* = the larger of the class entry's `d6` (−1 without a class) and `Render_FarViewDistance` (−1 without it); *v* ≥ 250 makes a Weenie, Physics or PatrolPoint node long-range (created without the viewer-kind test, not removed by the per-landblock update); 0 ≤ *v* ≤ 160 makes a node far-view (created and removed by distance only while the far-view mode is on, off by default). Whether *v* also changes how far a created entity is drawn was not traced here (`mesh.md` §8 governs drawing). |
| `appearance.md` §7.1–§7.2 | Corrections to this spec's §4.1 and to `landblock.md` §4.8 | Folded in (§4.1, §4.3, §4.5.2, §9). |

---

<a id="s15"></a>
## 15. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-04 | all | First version (Review). | Spec writer (dirty room) |
| 2026-10-05 | header, §0, §1 (BIN and AP sources, E2–E4, E6, E9, E12–E19), §2.1, §3.1–§3.2, new §3.4, §4.1–§4.6, new §4.5.2, §4.8, §5, §6, §7, §9, §10, §11 (ED1, ED3, new ED8–ED9), §12, §13, new §14–§15 | Content-layer labels and test from the client (BIN): layer 2 `All` always shown, layer 1 `InstanceZero` shown in the shared world, named layers only when active (answers O3). Heading rule of placed frames (answers O2). SED reader: place of the 0x200 list (after the property set), defaults of absent fields, SEDs without a frame at the origin, the legacy scale vector, IID bits 24–35 as the instance number. Folded in `appearance.md`'s corrections: what the weenie objects are, per-instance property sets, NPC-like figures. | Spec writer (dirty room) |
| 2026-10-05 | header, §1 (OBS re-check, BIN second use with questions 5–6, E11 split, new E20–E22), §2.2, §4.4, §4.5.1, §4.8, §7, §9 (new §9.1–§9.2), §11 (ED7), §12, §13 | Repair after review. The draw-suppression precedence, formerly "Reported by analogy", is now a labelled Tessera Decision (E22, §9.2) with an OPEN for the client's rule; the census behind it is extended (no chain or extra-set conflicts; SED and object disagree for 12 SEDs, which are listed) and the drawn counts under the alternative rules are given. New BIN facts: the render node's visibility test (`Render_ServerDraw`, then `Render_Draw`, then drawn; E20) and the content-layer path for Weenie, Physics and PatrolPoint SEDs during landblock loading (E21), with an OPEN for the render side of Physics SEDs and a Decision until then. §2.2 per-region SED counts and records with SEDs filled in. §7 states how the 1,925 distinct scales are counted (f64 rounding; f32 rounding gives 1,935). §4.4 gives the ±1 counts per tolerance (833,084 and 178,174 at 10⁻⁵) and the out-of-square split by cell presence; ED7 states that all 908 exceptions are in landblocks without cells. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (OBS re-check, BIN third use with questions 7–11, E14, E21, E22 revised, new E23–E29), §4.4, §4.5.1, §4.8, new §4.9, §7, §9.2, new §9.3–§9.4, §11 (new ED10–ED11), §12, §13, §14 | OPEN-closing revision for owner items O3 and O17 (BIN, decision 0011). The content-layer test governs the created entities of Physics SEDs in every case: failing records never enter the creation list, and the streaming index re-tests each node with the current layer and removes it on failure (former §4.5.1 OPEN closed). New §4.9: the client's property lookup on a SED (own set, then the object chain; for weenies the 0x70 set, then `PhysObj`) and `Entity_CreationType` with its classes, the weenie adjustment and the validity check; Dynamic SEDs are not created by the client (census: 1 PatrolPoint; 1 Physics SED has creation type 0). New §9.3: placement records, creation list, streaming index and update, including the interior-flag rule (interior entities only for a viewer in an interior cell). §9.2: the entity's lookup order (instance set or SED, then object) is the client's, so the 12 disagreeing SEDs are drawn; `Render_DrawMeshes` and `Render_NeverAddToScene` are node flag bits whose consumers stay OPEN. §7: the SED scale replaces the object's. §4.8: cell 1 for interior entities and the normaliser's cell lookup; attachment OPEN narrowed. New §9.4: presence, position and facing of the N1–N7 figures (answers O17 except server layer state and looks). | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (OBS repair re-check, BIN fourth use with questions 12–14, E25 and E28 revised, new E30–E32), §4.5.1, §4.5.2, §4.8, §9.3, §9.4, §11 (ED11, new ED12), §12, §13, §14 | Repair after the review of 2026-10-07 (changes requested, four items). (1) §9.3 step 3: the view distance (the LOD-class entry's `d6`, −1 by default, against `Render_FarViewDistance`), the long-range threshold 250.0, the far-view band 0–160.0 with the raise to 25.0, and the three sets; step 4: long-range nodes are created after the layer test without the viewer-kind test and are not removed by the per-landblock update; far-view nodes are created and removed by distance without the layer test, only while the far-view mode (off by default) is on; E25, §4.8, ED11 and §12 corrected (314,957 long-range Physics SEDs, 19,471 of them interior-flag; an outdoor viewer's passes create at most 1,517,213, not 1,497,742); OPENs for the removal of long-range entities and the far-view switch. (2) §4.5.1 and §9.3 step 4: the pass runs when a landblock's index is built, on forced updates (moved more than 9 units, changed region, landblock or instance, or no previous position) and their revisits, and when the far-view mode changes; never on a change of *L* alone; consequence (c) corrected. The range rule (*R* = 1 at construction; 3 × 3 landblocks outdoors, only the own landblock for an interior viewer) narrows the range OPEN. (3) §4.5.2: precedence traced (E26); the instance set replaces the SED's own set, never merged. (4) §9.3 step 2: the placement records are walked from last to first; both IID fields hold the instance-stamped IID. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (OBS second repair re-check, BIN fifth use with questions 15–16, E31 and E32 revised), §9.3 | Second repair after the re-review of 2026-10-07 (changes requested, one item, and two suggestions). (1) §9.3 step 3 census: the far-view split counted every Physics SED, not only the created ones; for the 63,377 created far-view Physics SEDs it is 2,454 with *v* = 0 and 60,923 with 25 ≤ *v* ≤ 160 (OBS, recomputed over all Physics SEDs; every other count of E28 and ED11 reproduced unchanged). (2) §9.3 step 2: the creation list also receives the landblock's terrain-generated scenery objects as kind-4 records after the SED records, last to first, and the streaming index builds their nodes by the same rules, with properties from the object (cross-reference to `scenery.md` §7.6; E32). (3) §9.3 step 4, *Range*: a viewer with an invalid position or cell 0xFFFF enumerates no landblock (E31). No section number or anchor changed. | Spec writer (dirty room) |
