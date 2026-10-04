<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Static entity descriptors in landblock info (placement, rotation, scale, references)

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` (static entity descriptor decoder, inside the landblock-info decoder), `crates/world` (placement transform, streaming of static entities, draw filter), `crates/net` sandbox backend ("serve static world entities"), `apps/client` (world walker), `apps/explorer` (landblock view) |
| **Companion specs** | [`landblock.md`](landblock.md) (normative for the landblock-info record that contains the descriptors, for cells, block maps and the terrain height scale). [`coordinates.md`](coordinates.md) (map coordinates). `setup.md` §3–§7 (the 0x47 physics-object record and its chain to meshes). `property-system.md` §4.6 (Position), §5 (embedded property sets), §8 (DID maps, `CONTENT_LAYER`). `records.md` §1 (VLE, TSize, Pascal string), §2.2 (landblock DIDs). `mesh.md` §11 (axes, units, handedness). `wstate.md` (0x70 objects named by weenie entities). |

---

<a id="s0"></a>
## 0. Purpose and terminology

Every landblock that holds placed scenery has a **landblock-info** record (DID
`0x802RXXYY`, `records.md` §2.2). Inside it is a list of **static entity descriptors**: one
per placed object (a house, a tree, a fence segment, a lamp, a spawn point, a patrol point …).
This spec gives:

- the descriptor's byte layout, decoded over **all** records (§3);
- what each field means: entity type, instance ID, the object it names, position, rotation,
  scale, properties, name, flags (§4);
- the placement transform from object space to world space, in words (§5), with the evidence
  for the rotation order and direction (§6) and for scale (§7);
- a validation of placements against terrain heights (§8);
- which descriptors a world walker should draw (§9);
- the decoding algorithm, invariants and statistics (§10–§12).

**Terminology.** `setup.md` calls the 0x47 records in `client_gamelogic.dat` "entity
descriptors". To avoid a clash, this spec says **static entity descriptor** (SED) for the
structure inside landblock info, and **physics object** (0x47 record) for what `setup.md` §4
describes. A SED names a physics object; the physics object leads to a setup and meshes.

---

<a id="s1"></a>
## 1. Provenance

Legend as in `mesh.md` §1: **Confirmed** = decoded on the user's own install on 2026-10-04 by
throwaway probes in the dirty-room scratchpad; **Reported** = third-party material only.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04. **All 49,636** landblock-info records of regions 1–5 and 14 decoded to the last byte; **all 3,253,862** SEDs read. All 27,962 distinct 0x47 records they name resolved through `setup.md` §3.1 to setups and mesh bounds. All 775 distinct 0x70 objects they name opened. Terrain heights read from the 0x800RXXYY records for the validation in §8. |
| LC | LotroCompanion `delta-lotro-dat-utils` (Apache-2.0 distribution; decompiled class `EntityDescLoader`, data class `EntityDescriptor`) and `lotro-tools` (`LandblockInfoLoader`, readable source). Field order and flag bits. Facts only; no code taken. |
| DS | `research/notes/data-semantics.md` §6.1 (positions, 160 m landblocks), §6.3 (landblock info), §6.4 (entity descriptor bits). |
| ER | `research/notes/engine-client-re.md` §4 (coordinate systems, `/loc` output) and §3 (cache type names). |
| SU | `setup.md` (0x47 → 0x1F → 0x04 → 0x06 chain; Z-up object space; (w, x, y, z) frames). |
| PS | `property-system.md` (property types, embedded sets, DID maps). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| E1 | SEDs are a u32-counted list inside landblock info, after the cells (§2) | LC, DS; OBS (49,636 records, zero trailing bytes) | Confirmed |
| E2 | Layout: u32 flags, then optional fields in a fixed order; the property set precedes the 0x40 word (§3) | LC, DS; OBS (3,253,862 descriptors) | Confirmed |
| E3 | Flag 0x400 carries no payload and marks interior entities (§3.3, §4.8) | OBS | Confirmed (layout); meaning Confirmed statistically |
| E4 | Flag bits 0x01, 0x20, 0x200 add a DID, a vector and a DID list | LC | Reported (never set in this install) |
| E5 | Entity type is an enum value labelled by enum mapper 0x23000091 (§4.1) | LC; OBS | Confirmed |
| E6 | The 64-bit instance ID encodes region and landblock (§4.2) | OBS (100 %) | Confirmed |
| E7 | The physics-object field names a 0x47 record, or a 0x70 object for weenies (§4.3) | LC; OBS | Confirmed |
| E8 | Position is landblock-local x, y in metres from the landblock's south-west corner, z absolute height (§4.4) | DS, ER; OBS (§8) | Confirmed |
| E9 | Rotation is a unit quaternion stored (w, x, y, z), applied as an active rotation (§4.4, §6) | LC (field names); OBS (§6) | Confirmed |
| E10 | Scale is the uniform vector property `Render_AdjustableScale` (equal to `Physics_AdjustableScale`) on every SED (§7) | OBS | Confirmed |
| E11 | Draw suppression comes from the physics object's properties (§9) | OBS | Confirmed census; client behaviour Reported by analogy |

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
| 13 | weenies | TSize count, elements u64 IID + embedded property set | `landblock.md` |
| 14 | trailer | u32, optional | Present in 42,530 records, absent in 7,106 |

<a id="s2-2"></a>
### 2.2 Population (Confirmed)

| Region | Landblock-info records | SEDs |
|---|---|---|
| 1 | 9,701 | |
| 2 | 11,508 | |
| 3 | 8,846 | |
| 4 | 11,479 | |
| 5 | 5,837 | |
| 14 | 2,265 | |
| **All** | **49,636** (33,007 with ≥ 1 SED) | **3,253,862** |

No other record type was found to embed SEDs. LC also decodes the same structure from
network messages (a "from network" variant whose embedded property set uses the network
form, `property-system.md` §10); that is M7 material.

---

<a id="s3"></a>
## 3. SED layout

<a id="s3-1"></a>
### 3.1 Fields in stream order (Confirmed)

| Order | Bit | Field | Type | Present in (of 3,253,862) |
|---|---|---|---|---|
| 1 | — | `flags` | u32 | all |
| 2 | 0x001 | `did` | DID | 0 (Reported, LC) |
| 3 | 0x002 | `type` | u32 enum (§4.1) | all |
| 4 | 0x004 | `iid` | u64 (§4.2) | all |
| 5 | 0x008 | `object` | DID (§4.3) | 3,244,086 |
| 6 | 0x010 | `position` | f32 × 3 (§4.4) | 3,253,835 |
| 6 | 0x010 | `rotation` | f32 × 4, (w, x, y, z) (§4.4) | (with `position`) |
| 7 | 0x020 | `vector` | f32 × 3 | 0 (Reported, LC; meaning unknown) |
| 8 | 0x080 | `properties` | embedded property set (`property-system.md` §5: TSize count, entries with PID echo) | all |
| 9 | 0x040 | `word` | u32 (§4.6) | 94,335 |
| 10 | 0x100 | `name` | Pascal string (`records.md` §1.4) | 3,242,259 |
| 11 | 0x200 | `dids` | VLE count, then that many DIDs | 0 (Reported, LC) |
| — | 0x400 | *(no payload)* | flag only (§4.8) | 963,922 |

**Order is not bit order** for bits 0x40 and 0x80: the property set (bit 0x80) comes
**before** the 0x40 word. Confirmed: every descriptor with bit 0x40 also has bit 0x80, and
reading the word first breaks the decode of every such record, while the order above decodes
all records exactly.

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
| 0x0000018E | 18 | object, name (no position) |
| 0x0000058E | 9 | object, name, interior flag (no position) |

Bits 0x002, 0x004 and 0x080 are set in every SED. Bits 0x001, 0x020, 0x200 and every bit
≥ 0x800 are never set.

<a id="s3-3"></a>
### 3.3 Unsupported bits

A decoder supports the Reported bits 0x001, 0x020 and 0x200 with the sizes in §3.1 (they
cost nothing and keep network and older data readable), but rejects any bit ≥ 0x800 as
unsupported: an unknown bit may carry a payload of unknown size, so continuing would
misframe the rest of the landblock-info record.

---

<a id="s4"></a>
## 4. Field semantics

<a id="s4-1"></a>
### 4.1 `type` (Confirmed values; labels from enum mapper 0x23000091)

The labels are internal identifiers read at run time from enum mapper 0x23000091
(`records.md` §7). The "object" column says what the `object` field names for that type.

| Value | Label | SEDs | `object` | Role |
|---|---|---|---|---|
| 7 | Physics | 2,380,699 | 0x47 (27,571 distinct) | Placed scenery and props. The only type with real geometry to draw (§9). |
| 3 | GeneratorPoint | 415,238 | 0x47 (239 distinct; 1 m editor markers) | Spawn point |
| 23 | PatrolPoint | 268,896 | 0x47 (8 distinct markers) | AI patrol waypoint |
| 9 | Light | 73,711 | 0x47 (2 distinct markers) in 72,880; none in 831 | Light source; parameters in properties (`LightDiffuse`, `LightRange`, `LightFalloff`, day/night variants) |
| 19 | SegmentNode | 55,645 | 0x47 (1 marker) | Path segment node |
| 6 | Weenie | 33,676 | **0x70** (775 distinct) | Server-side object placement (NPC, interactive object) |
| 2 | Decal | 16,511 | 0x47 (140 distinct small markers) | Projected decal; parameters in properties (`DecalVolume_*`) |
| 16 | TeleportDestination | 8,945 | none | Arrival point |
| 31 | LBIOnly | 541 | 0x47 (1 marker) | Unknown |

The mapper also defines labels that no SED uses (Invalid 0, PathBlock 1, LinkDesc 4,
PathHint 5, SceneFile 8, UIScene 10, Link 11, Scriptlet 12, Script 13, EntityGroup 15,
Landblock 18, KynMapSeedPoint 21, Cell 22, Export 24, Particle 25, DBObj 26, Camera 27,
WorldBuilderOnly 32). A decoder keeps unknown values as numbers.

<a id="s4-2"></a>
### 4.2 `iid`: instance ID (Confirmed, 100 % of SEDs)

The 64-bit instance ID is structured:

| Bits | Content |
|---|---|
| 56–63 | Kind byte: 1 (3,217,157 SEDs) or 4 (36,705; 33,605 of them Weenies, the rest Physics 1,918, GeneratorPoint 657, PatrolPoint 344, SegmentNode 140, TeleportDestination 24, Light 16, Decal 1) |
| 52–55 | Region *R* of the owning landblock |
| 44–51 | Landblock X of the owning landblock |
| 36–43 | Landblock Y of the owning landblock |
| 32–35 | 0 in every SED |
| 0–31 | Serial within the landblock (starts at 1; usually contiguous) |

Bits 32–55 equal the owning landblock-info DID's region, X and Y in every SED. 3,253,853
distinct IIDs; 9 IIDs occur twice (in different landblocks). Links and weenies refer to
entities by IID (`landblock.md`), so a world model indexes SEDs by IID and must tolerate a
duplicate (keep both; resolve a link to the one in the same landblock first).

> **OPEN:** Meaning of the kind byte (1 versus 4).

<a id="s4-3"></a>
### 4.3 `object`: what is placed (Confirmed)

- **0x47 physics object** (3,210,410 SEDs; 27,962 distinct DIDs, every one exists in
  `client_gamelogic.dat`). Resolve it with `setup.md` §3.1: 0x47 → (0x47)* → 0x1F →
  (0x1F)* → 0x04 setup → 0x06 parts. Observed chains: 0x47 → 0x1F → 0x04 for 26,512
  distinct objects, one extra hop (0x47 → 0x47 or 0x1F → 0x1F) for 1,450. Two objects
  (0x47001031, 0x47003E76; 20 SEDs) end at setups without parts.
- **0x70 game object** (33,676 SEDs, all of type Weenie; 775 distinct DIDs, all exist). The
  object's property set (DID + 0x09000000, `property-system.md` §6.2) carries `PhysObj`, a
  0x47 DID, in all 775; resolve that as above. Object classes (`wstate.md`): class index
  1989 (744 objects), 3725 (29), 1722 (1), 1723 (1).
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
neighbour (only about 190 of them sit within 2 m of the neighbour's terrain). A loader must not clamp or
re-home such positions.

**Rotation.** Four f32 forming a unit quaternion (all 3,253,835 within 0.001 of length 1),
stored **w first**: (w, x, y, z), the same order as `setup.md` §6.7. It rotates object space
into the landblock frame as an **active** rotation (§6 gives the evidence and §5 the
transform).

- 3,115,694 rotations (95.8 %) are pure turns about +Z (x = y = 0 within 1e-4). The rest
  tilt the object (rocks, debris, leaning props).
- For a pure turn the yaw angle ψ = 2 · atan2(z, w) is counter-clockwise seen from above.
  An object's local +Y axis (the forward direction of characters, `mesh.md` §11.1) then
  points at compass bearing (360° − ψ) mod 360°, measured clockwise from north.
- The identity is (1, 0, 0, 0) (first component ±1 in 833,084 SEDs; last component ±1 in
  178,174, which are genuine 180° turns).

**No position** (27 SEDs; flags 0x18E and 0x58E: 24 Physics, 2 Light, 1 PatrolPoint). A
renderer skips them.

> **OPEN:** The bearing formula above is derived, not yet checked against the original's
> `/loc` heading `h` (ER §4) for a placed object. One observation at a known statue or door
> would close it.

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
#### 4.5.1 Content layers (Confirmed values, semantics OPEN)

`Entity_ContentLayers` is an array whose elements are `Entity_ContentLayer` enum values. The
enum's data field names DID map 0x28000012, the `CONTENT_LAYER` map of `property-system.md`
§8.1, whose entries point at 0x78 property sets. Most common element lists: [1] (161,029
SEDs), [2] (111,046), then lists of 0x1000xxxx layer keys (single keys and lists of up to
four). 859 SEDs carry an empty list.

> **OPEN:** Which layers are active in an offline walker by default. Working assumption:
> SEDs without the property always exist; SEDs with layer lists exist only when one of their
> layers is active; layers 1 and 2 are the two most common alternatives and need
> observation before a default is chosen. Until then the walker draws SEDs without layers
> plus those that list layer 1, and offers a debug toggle.

> **OPEN (implementation choice):** `tessera-world` treats an **empty**
> `Entity_ContentLayers` list (859 SEDs) like an absent one, i.e. the SED is drawn. Whether
> the original hides such SEDs is unobserved. The active layer list is a bundle option
> (default `[1]`).

<a id="s4-6"></a>
### 4.6 `word` (bit 0x40; Confirmed values, meaning OPEN)

Physics SEDs: value 1 in all 60,659 that have it. Weenie SEDs (all 33,676 have it): 1
(26,810), 2 (5,066), 3 (1,145), 4 (459), 5 (105), 6 (65), 7 (22), 9 (2), 8 (1), 11 (1).

> **OPEN:** Meaning (a spawn count or a version are candidates for weenies).

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

> **OPEN:** How a SED is attached to a particular cell (no cell index is stored in the SED;
> containment in the cell volume works as a fallback). 4,988 SEDs with the bit live in
> landblocks without cells.

---

<a id="s5"></a>
## 5. Placement transform (Confirmed)

To place one part vertex *v* (object space, `setup.md` §7) of a SED owned by landblock
(*R*, *X*, *Y*):

1. Multiply *v* by the uniform scale *s* (§7).
2. Rotate the result by the SED quaternion *q* = (w, x, y, z) as an active rotation
   (rotation of a vector by *q* followed by the conjugate of *q*).
3. Add the SED position (*x*, *y*, *z*).
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
(object space to landblock frame).

---

<a id="s7"></a>
## 7. Scale (Confirmed)

- Every SED carries both `Render_AdjustableScale` and `Physics_AdjustableScale` (vector
  properties). In all 3,253,862 SEDs they are equal and uniform (x = y = z).
- Values: 1,925 distinct (to 3 decimals) from 0.01 to 100; 1.0 in 2,292,193 SEDs (70.4 %);
  then 1.5, 0.5, 0.85, 0.75, 2.0, 0.8 ….
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

> **OPEN:** Whether the SED scale replaces or multiplies a scale stored on the physics object
> (indistinguishable here, since every SED has its own).

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
## 9. What a world walker draws

Static geometry comes from **Physics** SEDs only. The other types name editor marker
objects (one-metre boxes, light bulbs, arrows; §4.1) that the original does not show in play.

Within Physics SEDs, the physics object's own property set can suppress drawing. Resolve the
0x47 chain (`setup.md` §3.1) and look up these booleans, the first value found along the
chain wins (Confirmed census; precedence along the chain Reported by analogy with property
inheritance):

| Property | Value meaning "do not draw" | SEDs naming such an object (all types) |
|---|---|---|
| `Render_Draw` | 0 | 854,615 |
| `Render_DrawMeshes` | 0 | 186,921 |
| `Render_NeverAddToScene` | 1 | 740,551 |
| any of the three (579 of the 27,962 named objects) | | 1,027,172 |

By type, the suppressed SEDs are: all GeneratorPoint, PatrolPoint, SegmentNode and LBIOnly
SEDs, and 286,852 of 2,380,699 Physics SEDs (invisible walls, ground-cover removal volumes,
entity-group markers, sound emitters, occluders). Light (9) and Decal (2) markers are not
flagged but are markers (§4.1). The SED's own properties may override: `Render_Draw` occurs
on 467 Physics SEDs (0: 329, 1: 138) and `Render_DrawMeshes` on 328 (0: 245, 1: 83); the SED
value wins over the object's.

Draw rule for M3: draw a SED if its type is Physics, it has a position, it passes the
content-layer filter (§4.5.1), and the effective `Render_Draw`, `Render_DrawMeshes` and
`Render_NeverAddToScene` do not suppress it. That leaves about 2,093,800 SEDs.

> **OPEN:** Whether lights and decals (types 9 and 2) should be realised from their
> properties in M3 (light parameters, decal volumes); whether weenies (type 6) are shown by
> the sandbox as static stand-ins (their `PhysObj` resolves to geometry, §4.3).

---

<a id="s10"></a>
## 10. Decoding algorithm (in words)

For each SED, with the landblock-info cursor positioned after the SED count:

1. Read the u32 `flags`. If any bit ≥ 0x800 is set, stop with "unsupported SED flags" for the
   whole record.
2. If bit 0x001: read a DID. If bit 0x002: read the u32 type. If bit 0x004: read the u64
   IID. If bit 0x008: read the object DID.
3. If bit 0x010: read three f32 (position) and four f32 (rotation, w first).
4. If bit 0x020: read three f32 (vector).
5. If bit 0x080: read an embedded property set (`property-system.md` §5.3), with the
   dictionary.
6. If bit 0x040: read a u32.
7. If bit 0x100: read a Pascal string.
8. If bit 0x200: read a VLE count *k*, then *k* DIDs.
9. Bit 0x400: nothing to read; record it.

Defensive limits: bound the SED count by the remaining bytes divided by 4 (a SED is at
least its flags word), and apply an absolute cap (the
largest observed count is 7,801; a cap of 65,536 is ample). Bound the name length by the
remaining bytes (observed ≤ 116) and the DID-list count by remaining / 4. Reject a
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
| **ED1** | `flags` ⊆ {0x002, 0x004, 0x008, 0x010, 0x040, 0x080, 0x100, 0x400}, with 0x002, 0x004 and 0x080 always set (plus the Reported 0x001, 0x020, 0x200, never observed). |
| **ED2** | Every landblock-info record decodes to its last byte with the SED layout of §3 (49,636 / 49,636). |
| **ED3** | Bits 32–55 of every IID equal the owning landblock's region, X and Y; bits 32–35 are 0. |
| **ED4** | Type 6 (Weenie) SEDs name 0x70 objects; every other SED with an object names a 0x47 record. Every named record exists. |
| **ED5** | Rotations have unit length (±0.001). |
| **ED6** | `Render_AdjustableScale` and `Physics_AdjustableScale` are present, uniform and equal, with values in [0.01, 100]. |
| **ED7** | An SED whose position lies outside the 160 m square has the interior flag 0x400 (772,963 of 773,871; the exceptions are a census item, not an error). |

---

<a id="s12"></a>
## 12. Statistics and failure categories

| Measure | Value |
|---|---|
| Landblock-info records decoded / total | 49,636 / 49,636 (0 trailing bytes) |
| SEDs | 3,253,862 |
| Distinct 0x47 objects named / resolved to meshes | 27,962 / 27,960 (2 part-less setups) |
| Distinct 0x70 objects named / with `PhysObj` | 775 / 775 |
| SEDs drawn by the §9 rule (approx.) | 2,093,800 |

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

> **OPEN:** Bearing of objects versus the original's `/loc` heading (§4.4).

> **OPEN:** IID kind byte 1 versus 4 (§4.2).

> **OPEN:** Content-layer defaults for an offline walker (§4.5.1).

> **OPEN:** Meaning of the 0x40 word (§4.6).

> **OPEN:** Cell attachment of interior SEDs; interior SEDs in landblocks without cells
> (§4.8).

> **OPEN:** Template-level scale fields and the 0x47 `block` versus the SED scale (§5, §7).

> **OPEN:** Realising lights, decals and weenies (§9).

> **OPEN:** The meaning of the `vector` (0x020), `did` (0x001) and `dids` (0x200) fields,
> Reported only.
