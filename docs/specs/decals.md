<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Placed decal volumes: `DecalVolume_*` parameters, projection, receiving surfaces and shading

| | |
|---|---|
| **Status** | Revised (2026-10-09, spec writer: the two non-blocking notes of the first review taken up, §2.1 (DC3 wording, stored order counted) and §6.5 (total 52,028); no rule changed). Second review 2026-10-09: **changes requested** (one blocking item). Revised again 2026-10-09 (spec writer): blocking item (1) fixed, §10 invariant 2 now states the comparison as in DC3, independent of stored order; awaiting re-review. Before: Accepted (2026-10-09) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (independent dirty-room spec reviewer, not the author) |
| **Review record** | First review, 2026-10-09: **passed** (CLEAN_ROOM.md §5.3; 14 data and BIN facts spot-checked independently, all match). Non-blocking fixes for the next revision: §6.5 total of `Render_DepthBias_Type` is 52,028 (the per-value counts are right), and the instance-set equality of DC3 holds for the entries, with a different order in 1,671 SEDs. Full record in the review log (decision 0018). | Second review, 2026-10-09 (revision of the same day; diff against main and the sections it touches: header, DC3, §2.1, §6.5, change log): **changes requested**. Checked: CLEAN_ROOM.md §5.3 items on the changed text (functional facts only, no code or pseudo-code, no copied prose, no game text, provenance row DC3 updated with source and date, no closed gate); the §6.5 total recomputed from the unchanged per-value counts (sum 52,028); the DC3 split re-counted independently and read-only over all 49,636 landblock-info records (16,511 Decal SEDs, every one with an instance set; same entries, values and header in all; same stored order 14,840, another order 1,671, no other difference): matches. **Blocking:** (1) §10 invariant 2 still says the per-instance set "equals" the SED's own set; after the DC3 revision this is false for 1,671 SEDs when equality is read as ordered (the natural test). State it as in DC3: the same entries (PIDs and values) and the same header, independent of stored order. No other findings.
| **Implements into** | `crates/world` (realising Decal SEDs as decal objects, §2.3; parameter lookup, §3), a decal builder in `crates/render` or `crates/world` (projection volume, receiving triangles, clipping, §4–§5), `crates/render` (decal pass: vertex colour, blend, depth bias, phase, §6), `crates/world` streaming and draw distance (§7), `apps/client` (world walker), `apps/explorer` (decal preview, §9.3) |
| **Companion specs** | [`entity-descriptor.md`](entity-descriptor.md) (SED layout §3, frame §4.4, content layers §4.5.1, interior flag §4.8, lookup order §4.9 and §9.2, creation of Decal nodes §9.3; rows E19, E24–E27, E30–E32). [`property-system.md`](property-system.md) (dictionary defaults §2, colour encoding §4.7, DID maps and `UNIQUEDB` §8). [`material.md`](material.md) (surface records 0x31 §4, effective parameter set §9, blend §11.3). [`lighting.md`](lighting.md) (light records and attenuation §8.3, light lists §8.5, vertex colours §5.13). [`mesh.md`](mesh.md) (distance *d* §8.6, draw distance and fade §8.7, skinned part §8.6). [`environment.md`](environment.md) §9.1 (LOD-class table). [`landblock.md`](landblock.md) (terrain grid and diagonals §3.3, §3.6, terrain vertex normals §3.14, cells §4.4). [`terrain.md`](terrain.md) §1 (the full-resolution terrain triangles). [`water.md`](water.md) §6.3 (water plane). [`coordinates.md`](coordinates.md) (frames, Z up, cells). [`script-table.md`](script-table.md) §5.3 (the script Decal node, kind 1). |

---

<a id="s0"></a>
## 0. Purpose and terminology

The world contains projected **decals**: road and path markings, scorch and blood marks,
ground patterns, signs painted on walls. In the data each one is a **Decal SED** (static
entity descriptor of type 2, `entity-descriptor.md` §4.1) whose properties carry the
`DecalVolume_*` parameters. The client turns each such SED into a **decal object**: an
oriented box in the world (the **projection volume**) into which it clips the triangles of
nearby terrain, interior cell geometry, placed objects and (optionally) water, and draws the
clipped triangles with the decal's material.

This spec lets an implementer:

- find every decal and its 20 parameters, with the value used when a parameter is absent
  (§2, §3);
- build the projection volume and the decal mesh: frame, box, receiving triangles, back-face
  test, clipping, texture coordinates (§4);
- decide which surfaces receive decals (`Render_AllowDecals`, `Render_ForceDecals`, §5);
- shade a decal: material, per-decal colour with the seven lighting-blend modes, the decal's
  light colour, opacity modifiers, depth bias (`DecalVolume_DepthBias`, and the shared
  depth-bias table that `Render_DepthBias_Type` also indexes), render phase (§6);
- decide when a decal is drawn (LOD class and draw distance, §7);
- relate the placed decal to the script Decal node (§8).

**Terms.** *Decal SED*: a SED of type 2. *Marker object*: the 0x47 physics object a Decal SED
names (a small editor marker; never drawn). *Decal object*: the client's run-time object built
from a Decal SED (or from a script Decal node, §8). *Receiver*: a surface whose triangles a
decal is clipped against. *Local axes* X′, Y′, Z′: the SED's rotation applied to the unit
axes (§4.1). Distances are in metres (world units, `coordinates.md`).

---

<a id="s1"></a>
## 1. Provenance

Marks: **Confirmed** = checked against **every** record of the kind in the user's install, on
the date given; **BIN** = read from the client executable (method M4, [decision
0011](../decisions/0011-standing-m4-authorisation.md)); facts that rest on BIN alone say so;
**Inferred** = a reading that fits the evidence but was not proven; **Reported** = third-party
material only (none here); **Tessera Decision** = this project's rule where the original's
behaviour is not known or not reproduced.

| Tag | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-08 by throwaway probes in the dirty-room scratch area, with the repository's decoders used as tools. **All** 49,636 landblock-info records of regions 1–5 and 14 decoded; **all** 3,253,862 SEDs and **all** 3,768,840 per-instance property sets read; the **16,511** Decal SEDs examined field by field (flags, IID kind, object, frame, properties, instance set, content layers); **all** 40,643 0x47 physics objects of `client_gamelogic.dat` decoded, with their parent chains; **all** 0x78–0x7F stand-alone property sets searched for the depth-bias and decal properties; the master property dictionary (0x34000000) read for every property whose name contains `Decal` or `DepthBias`; the enum mappers 0x23000297 (with base 0x23000298), 0x230002A7, 0x230000E5 and 0x2300008D (with base 0x23000165); the `UNIQUEDB` DID map 0x28000002; the LOD-class table 0x32000000; the 140 surface records named by decals resolved with `material.md` §9 (shader record, blend family, depth write); the terrain heights 0x800RXXYY under every outdoor Decal SED whose local +Y points down (§4.2 check). |
| BIN | The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-08 with the Ghidra 12 decompiler and listings on a read-only copy of the shared analysed project (deleted afterwards), for facts only, by address. **Questions:** (1) which class a Decal SED becomes, which fields it has, from which properties they are filled and with which initial values; (2) where the decal object's parameters come from (SED, instance set, marker object), and what happens to the SED's scale; (3) how the projection volume is placed relative to the SED frame, and how its size enters; (4) which surfaces are collected, under which conditions, and how they are clipped, culled and textured; (5) what `Render_AllowDecals` and `Render_ForceDecals` do on the receiving side; (6) how the decal's colour, light and opacity are computed and what `DecalVolume_LightingBlend` does; (7) how `DecalVolume_DepthBias` and `Render_DepthBias_Type` reach a draw, and where the depth-bias definitions come from; (8) how `DecalVolume_LODClass` sets the draw distance; (9) when the geometry is built and rebuilt; (10) how the script Decal node maps onto the same object. **Why M1–M3 were not enough:** the data hold only parameter values and the marker objects; the box placement, the clipping, the receiver rules, the colour arithmetic, the depth-bias wiring and the draw-distance rule are code paths with no trace in any record; no published source describes them (M2); no frame capture or running client was available to this session (M3), and a capture would show the result but not the rules or the constants. **Code read:** the decal-volume class registration, constructor, field setters, serialisation and LOD-class lookup; the landblock streaming index's decal creation; the SED property collector and its object-chain recursion; the decal manager (registration, batching, per-frame update, vertex writer, batch draw); the decal geometry builder, its polygon clipper and its terrain collector; the scene's decal node (bounds, distance range) and the shared distance-fade routine; the entity and cell scene nodes' decal acceptance tests and triangle collectors; the render component's property handler (allow, force, depth-bias type); the renderer's start-up loading of the depth-bias definitions; the script Decal node's field registration and run routine. No network, login, launcher or anti-tamper code. |
| ED | [`entity-descriptor.md`](entity-descriptor.md) (Accepted): SED layout, lookup order, content-layer test, creation of Decal nodes (rows E14, E19, E24–E27, E30–E32). |
| MAT, LGT, MESH, ENV, PS | [`material.md`](material.md), [`lighting.md`](lighting.md), [`mesh.md`](mesh.md), [`environment.md`](environment.md), [`property-system.md`](property-system.md): facts reused, cited by section. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| DC1 | 16,511 Decal SEDs; per region, flags, IID kind, objects, frames as §2.1 | OBS (all SEDs, 2026-10-08) | Confirmed |
| DC2 | Every Decal SED names one of 140 marker objects (0x47); chains 0x47 → 0x1F (29) or 0x47 → 0x47 → 0x1F (111); none has 0x78 extra sets; `DecalVolume_Material` sits on the first chain level of all 140 (§2.2) | OBS (all 140 objects) | Confirmed |
| DC3 | Every Decal SED IID has a per-instance property set, and it holds the same entries as the SED's own set (same PIDs and values, same header); the stored order is the same in 14,840 and differs in 1,671 (§2.1) | OBS (16,511 of 16,511); order re-counted 2026-10-09 over all 49,636 landblock-info records | Confirmed |
| DC4 | The client builds a decal object for a Decal node of the streaming index (`entity-descriptor.md` §9.3), at the node's position and rotation and cell, without any scale (§2.3) | BIN (questions 1, 2); ED E19, E27 | BIN |
| DC5 | The decal object's parameters come from a property collection filled from the marker object's chain (root parent first, nearer levels overwriting) and then the SED's own property set (overwriting); the per-instance set is not read on this path (§3.2) | BIN (question 2) | BIN; no difference on this install (DC3) |
| DC6 | The decal class has exactly 20 parameters, one per `DecalVolume_*` property, with the initial values of §3.1 (they equal the dictionary defaults, except the material and LOD distance, which come from `UNIQUEDB` and the LOD table) | BIN (question 1); OBS (dictionary) | BIN; defaults Confirmed |
| DC7 | Census of the effective parameters over all Decal SEDs (§3.3) | OBS | Confirmed |
| DC8 | Frame, box and scene bounds: centre = SED position + (raw `OrthoScale`.y / 2) × Y′; extents = `OrthoScale` with every component below 0.1 replaced by 1.0; scene bounds local x ∈ [−sx/2, sx/2], y ∈ [0, sy], z ∈ [−sz/2, sz/2] (§4.2) | BIN (question 3) | BIN |
| DC9 | Outdoor Decal SEDs whose local +Y points down sit 0 … `OrthoScale`.y above the terrain in 3,664 of 4,213 cases (§4.2 check) | OBS | Confirmed (supports DC8) |
| DC10 | Receivers and their collection order: entity and cell triangles (by the stick flags and §5), then terrain triangles (only when sticking to the landscape and the decal is outdoors), then the water rule (§4.3) | BIN (question 4) | BIN |
| DC11 | Back-face test against −Y′ with cos(`BackfaceCullingAngle`), stored or geometric normal, landscape triangles only with `UseLandscapeBackfaceCulling` (§4.4) | BIN (question 4) | BIN |
| DC12 | Clipping against the six box planes, fan triangulation, texture coordinates u along X′, v along −Z′, lift of 0.008 m along the vertex normal; warning above 8,000 triangles (§4.5–§4.7) | BIN (question 4) | BIN |
| DC13 | Receiving rules for entities (allow, force, two internal node states, skinned parts, visibility) and for cell geometry (cell's own `Render_AllowDecals`) (§5) | BIN (question 5) | BIN |
| DC14 | Census of `Render_AllowDecals` / `Render_ForceDecals` on SEDs, instance sets, objects and cells (§5.4) | OBS | Confirmed |
| DC15 | Per-decal vertex colour from `ColorAndOpacity`, the light colour and the seven `LightingBlend` modes; 8-bit packing by truncation (§6.2) | BIN (question 6) | BIN |
| DC16 | The decal's light colour: ambient plus the main directional light and the node's lights at the decal's mean normal and mesh centre, times a per-node tint, clamped (§6.3) | BIN (question 6) | BIN; two inputs OPEN |
| DC17 | Height-based and depth-based opacity rules; neither is enabled by any decal of this install (§6.4) | BIN (question 6); OBS | BIN; census Confirmed |
| DC18 | Depth-bias table of 16 entries filled at renderer start from the stand-alone property sets behind `UNIQUEDB` keys 0x163 and 0x164; decals select an entry with `DecalVolume_DepthBias` (§6.5) | BIN (question 7); OBS (DID map, both sets) | BIN; data Confirmed |
| DC19 | 138 of the 140 decal surface records use unlit shader records; blend families and depth write as §6.1 | OBS (all 140) | Confirmed |
| DC20 | Draw distance D = min(G, `d0` of the decal's LOD class × *M_d*), 30 m without a table entry; distance and 1.5 s fade as for entities (§7) | BIN (question 8); MESH §8.6–§8.7 | BIN |
| DC21 | The geometry is built at the first update of the decal's scene node and rebuilt whenever a parameter or the frame changes (§4.8) | BIN (question 9) | BIN |
| DC22 | The script Decal node builds the same decal class; field correspondence of §8 | BIN (question 10); ST §5.3 | BIN |

---

<a id="s2"></a>
## 2. Where decals come from

<a id="s2-1"></a>
### 2.1 Decal SEDs (Confirmed, DC1, DC3)

| Item | Census (all 16,511 Decal SEDs) |
|---|---|
| Per region (digit of the landblock-info DID) | 1: 6,125; 2: 6,977; 3: 1,161; 4: 1,233; 5: 895; 14: 120 |
| `flags` (`entity-descriptor.md` §3.2) | 0x19E (object, frame, properties, name): 5,047; 0x59E (the same plus the interior flag 0x400): 11,464 |
| IID kind byte | 1: 16,510; 4: 1 |
| Frame | present on all (none at the landblock origin) |
| Instance property set (ED §4.5.2) | one per Decal SED IID; the same entries as the SED's own set (same PIDs, values and header) in all 16,511, in the same stored order in 14,840 and in another order in 1,671 |
| Content-layer test in the shared world (*L* = 1, *I* = 0; ED §4.5.1) | passes 15,502 (15,126 without a list, 376 with a list naming 1 or 2); fails 1,009 |
| Properties on the SED other than `DecalVolume_*` | `Render_AdjustableScale` and `Physics_AdjustableScale` (all; value 1 on 16,259, 1.4 on 28, 2 on 224), `Entity_ContentLayers` (1,386), `Dungeon_DID` (82), and on 28 SEDs `Appearance_InstanceList`, `Render_CastLandscapeShadows`, `Render_FarViewDistance`, `Render_LODClass`. None of these is a decal parameter (§2.3, §3). |

`DecalVolume_*` properties occur nowhere else among the SEDs except one Light SED (type 9)
that carries `DecalVolume_OrthoScale`; a light object ignores it. In the per-instance property
sets they occur exactly where the Decal SEDs have them (each occurrence duplicated, DC3).

<a id="s2-2"></a>
### 2.2 Marker objects (Confirmed, DC2)

The 140 distinct 0x47 objects named by Decal SEDs resolve to small marker setups (as all
editor markers, `entity-descriptor.md` §4.1). The client does not draw them for a decal: a
Decal SED becomes only a decal object (§2.3). Their property sets carry the decal's default
look:

| Property on the marker object (first chain level) | Decal SEDs whose object carries it |
|---|---|
| `DecalVolume_Material` | 16,511 (all) |
| `DecalVolume_LightingBlend` | 11,288 |
| `DecalVolume_LODClass` | 8,861 |
| `DecalVolume_ColorAndOpacity` | 8,148 |
| `DecalVolume_OrthoScale` | 5,116 |
| `DecalVolume_UseHeightBasedOpacity` (false) | 543 |
| `DecalVolume_UseDepthBasedOpacity` (false) | 524 |
| `DecalVolume_StickToWorldEntities` (false) | 19 |

The second chain level of the 111 two-level objects carries no decal parameter.

<a id="s2-3"></a>
### 2.3 Creating the decal object (BIN, DC4)

Which Decal SEDs exist at a given moment is fixed by `entity-descriptor.md` §9.3: Decal SEDs
are indexed after the creation records, always join the main set (never long-range), are
subject to the content-layer test at every pass of their landblock, and their interior flag
restricts them to passes for an interior viewer (ED §4.8). This spec starts where §9.3
ends ("a node of kind Decal … becomes a decal object"):

1. The client creates a decal object with the initial values of §3.1.
2. It gives the object the node's frame: the region, landblock and instance of the owning
   landblock copy, the cell (1 when the SED has the interior flag, else 0; ED §4.8), and the
   SED's stored position and rotation unchanged. The frame is then normalised like any
   position (`coordinates.md` §4: cell 1 is resolved to the containing interior cell).
3. It finds the SED in the owning landblock by IID and collects its properties (§3.2), then
   assigns every collected property whose id is one of the 20 decal parameters (§3.1) to the
   matching parameter. Other collected properties are ignored.
4. It registers the object with the scene as a decal node (§7). The object starts marked
   for a build; at the first update of its scene node it registers with the decal manager
   (§6.6) and builds its mesh (§4.8).
5. It keeps the object in a per-manager table keyed by the node's IID.

**Scale is not used.** The decal class has no scale field and the creation passes none:
`Render_AdjustableScale` / `Physics_AdjustableScale` of a Decal SED (1.4 on 28 SEDs, 2 on 224)
do not change the projection volume; only `DecalVolume_OrthoScale` sizes it (BIN).

> **OPEN (low priority):** the client contains a second routine that walks a landblock's SED
> list and builds decal and light objects for every Decal and Light SED that passes the
> content-layer test, owned by the object that also owns the interior cells' render geometry.
> What triggers it was not traced (it is gated by a state value of that object and a switch
> of the data service). The streaming-index path above is the one `entity-descriptor.md`
> §9.3 traces for normal play. The resulting decal objects are the same either way.

---

<a id="s3"></a>
## 3. Parameters

<a id="s3-1"></a>
### 3.1 The 20 parameters (BIN, DC6; dictionary defaults Confirmed)

Property names are quoted for interoperability: they are the names under which the values
are stored, and a reader finds them by name in the install's property dictionary at run time
(the ids below are this install's). Types are property-value types (`property-system.md` §4).
"Initial value" is what the decal object holds when no collected property sets the
parameter.

| Property (id here) | Type | Initial value | Meaning | § |
|---|---|---|---|---|
| `DecalVolume_Material` (0x1AD) | DID (surface record, prefix 0x31) | the DID in the `UNIQUEDB` map under key 0x29 (label `Render_DefaultDecalMaterial`; 0x31000008 here, equal to the dictionary default) | Surface record the decal is drawn with | §6.1 |
| `DecalVolume_LODClass` (0x111) | enum (mapper 0x2300008D, the `Render_LODClass` vocabulary) | 1 (`DefaultDecal`) | Selects the draw distance | §7 |
| `DecalVolume_ColorAndOpacity` (0xB4) | colour | (1, 1, 1, 1) | Decal colour and opacity | §6.2 |
| `DecalVolume_LightingBlend` (0x839) | enum (mapper 0x230002A7) | 0 (`None`) | 0 = unlit; otherwise how the light colour combines with the decal colour | §6.2, §6.3 |
| `DecalVolume_UseBackfaceCulling` (0x110) | bool | true | Back-face test on | §4.4 |
| `DecalVolume_UsePreciseBackfaceCulling` (0x10F) | bool | false | Geometric instead of stored normal | §4.4 |
| `DecalVolume_UseLandscapeBackfaceCulling` (0x70F) | bool | true | Back-face test also for terrain triangles | §4.4 |
| `DecalVolume_BackfaceCullingAngle` (0xB2) | float, degrees | 85.0 (dictionary minimum 0, maximum 180) | Limit angle of the back-face test | §4.4 |
| `DecalVolume_UseHeightBasedOpacity` (0x81D) | bool | false | Height fade on | §6.4 |
| `DecalVolume_HeightBasedOpacityRange` (0x821) | float, metres | 0.0 | Height fade range | §6.4 |
| `DecalVolume_UseDepthBasedOpacity` (0x83F) | bool | false | Camera-distance fade on | §6.4 |
| `DecalVolume_DepthBasedOpacityMin` (0x841) | float, metres | 1.0 | Distance fade start | §6.4 |
| `DecalVolume_DepthBasedOpacityRange` (0x840) | float, metres | 1.0 | Distance fade length | §6.4 |
| `DecalVolume_StickToWorldEntities` (0x10E) | bool | true | Placed objects receive | §4.3 |
| `DecalVolume_StickToCells` (0x10D) | bool | true | Interior cell geometry receives | §4.3 |
| `DecalVolume_StickToLandscape` (0x242) | bool | true | Terrain receives (outdoor decals only) | §4.3 |
| `DecalVolume_StickToWater` (0x851) | bool | false | Water rule | §4.3 |
| `DecalVolume_RenderPhase` (0x68D) | enum (mapper 0x230000E5) | 1 (`Default`) | When in the frame the decal is drawn | §6.6 |
| `DecalVolume_OrthoScale` (0x339) | vector (x, y, z), metres | (1, 1, 1) | Size of the projection volume along X′, Y′, Z′ | §4.2 |
| `DecalVolume_DepthBias` (0x7F7) | enum (mapper 0x23000297) | 1 (`SlopeScale_NegativeOne`) | Depth-bias entry for the draw | §6.5 |

The client also keeps a **LOD distance** derived from the LOD class (§7; initial 30.0 m,
replaced at construction by the entry of class 1) and an opacity factor that starts at 1.0
and is not set from data.

Enum labels (internal identifiers from the named mappers, read at run time; quoted to
identify values):

| Mapper | Values |
|---|---|
| 0x230002A7 (lighting blend) | 0 `None`, 1 `AlphaMultiply`, 2 `AlphaSubtract`, 3 `Subtract`, 4 `Add`, 5 `Multiply`, 6 `AlphaAdd` |
| 0x230000E5 (render phase) | 1 `Default`, 2 `AfterOpaqueObjects_BeforeSpritesAndFrills`, 3 `Invalid`, 4 `AfterFrills_BeforeTranslucentNodes`, 5 `DynamicDecalLights` |
| 0x23000297 (depth bias), base 0x23000298 | 0 `None`, 1 `SlopeScale_NegativeOne`, 2 `SlopeScale_One`, 3–7 `BiasForward1` … `BiasForward5`, 8–12 `BiasBackward1` … `BiasBackward5`, 13 `SlopeScale_NegativeNudge`, 14 `SlopeScale_Nudge`, 15 `SlopeScale_NegativeLarge` |
| 0x2300008D (LOD class), base 0x23000165 | base: 1 `DefaultDecal`, 2 `Unspecified`, 3 `DefaultLight`; the derived mapper adds the 0x100000*nn* classes of `mesh.md` §8.5 |

Other decal-named properties in the dictionary are not parameters of placed decals:
`DecalScript_*` (§8), `WB_DecalVolume_Entity` and `ShowDecalVolumes` (world-building tool
values, absent from all SEDs), `QuestState_*Decal*`, `Render_SelectionDecal_*`,
`Skill_GroundTargetPositionDecalSize`, `Combat_ClickPositionDecalOrigin`,
`ScriptParameter_DecalSize` (game-system decals outside this spec).

<a id="s3-2"></a>
### 3.2 Where a parameter's value comes from (BIN, DC5)

For a Decal SED the client fills one property collection, in this order, a later value of the
same property replacing an earlier one:

1. the marker object's 0x47 chain, **root parent first**: the chain is walked from the
   named object to its last 0x47 ancestor, and the levels are added from that ancestor back
   down to the named object (so the level nearest the named object wins);
2. the SED's own property set (wins over the object).

The per-instance property set of the landblock (ED §4.5.2) is not read on this path (it is
for entities, ED §9.2). On this install it makes no difference (DC3). A parameter absent from
the collection keeps its initial value (§3.1). Values of the wrong type are not assigned
(the client's typed field setters).

<a id="s3-3"></a>
### 3.3 Census of effective values (Confirmed, DC7)

Effective value per Decal SED, by the rule of §3.2 ("SED" = from the SED's set, "object" =
from the marker chain, "initial" = not set):

| Parameter | Source counts | Effective values |
|---|---|---|
| `Material` | object 16,511 | 140 distinct surface records (all prefix 0x31) |
| `OrthoScale` | SED 14,584; object 1,141; initial 786 | 1,098 distinct SED values; components 0.1–20 for x, 0–20 for y, 0.2–20 for z; y below 0.1 (all exactly 0) on 94 decals |
| `ColorAndOpacity` | SED 3,184; object 6,099; initial 7,228 | 226 distinct SED values |
| `LightingBlend` | SED 1,858; object 10,744; initial 3,909 | 0: 5,626 (with initial); 1: 287; 2: 39; 3: 3; 4: 228; 5: 9,679; 6: 649 |
| `LODClass` | SED 512; object 8,723; initial 7,276 | class 1: 7,711; 0x10000003: 8,265; 0x10000008: 19; 0x1000000B: 303; 0x1000000C: 87; 0x10000010: 126 |
| `DepthBias` | SED 936; initial 15,575 | 1: 15,763 (with initial); 0: 1; 3: 164; 4: 87; 5: 319; 6: 19; 7: 157; 13: 1 |
| `StickToWorldEntities` | SED 1,295; object 19 | false 1,138; true otherwise |
| `StickToCells` | SED 423 | false 305; true otherwise |
| `StickToLandscape` | SED 204 | false 25; true otherwise |
| `StickToWater` | SED 14 | true 14 (all outdoor); false otherwise |
| `UseBackfaceCulling` | SED 64 | false 19; true otherwise |
| `BackfaceCullingAngle` | SED 15 | 65.5814 (13), 45 (1), 30 (1); 85 otherwise |
| `UsePreciseBackfaceCulling` | SED 5 | false everywhere |
| `UseLandscapeBackfaceCulling` | SED 4 | true everywhere |
| `UseHeightBasedOpacity` | object 543 | false everywhere |
| `HeightBasedOpacityRange` | SED 27 | 0.0 |
| `UseDepthBasedOpacity` | SED 64; object 524 | false everywhere |
| `DepthBasedOpacityMin`, `…Range` | SED 42 each | 1.0, 1.0 |
| `RenderPhase` | SED 149 | 1 everywhere |

Combinations of the stick flags (entities, cells, landscape, water): outdoor decals (5,047):
(T, T, T, F) 4,555; (F, T, T, F) 442; (F, F, T, F) 16; (T, F, T, F) 14; (T, T, T, T) 14;
(T, T, F, F) 6. Interior decals (11,464): (T, T, T, F) 10,493; (F, T, T, F) 677;
(T, F, T, F) 275; (T, T, F, F) 16; (F, T, F, F) 3.

All values are observations of this install; an implementation reads them at run time.

---

<a id="s4"></a>
## 4. Projection volume and decal mesh (BIN unless marked)

<a id="s4-1"></a>
### 4.1 Frame and local axes

The decal's frame is the SED's (§2.3): position *p* in landblock coordinates (`coordinates.md`;
x and y in metres from the landblock's south-west corner, z absolute height), and the unit
quaternion *q* stored (w, x, y, z), applied as an active rotation (ED §4.4, §6). The local
axes are X′ = *q*·(1, 0, 0), Y′ = *q*·(0, 1, 0), Z′ = *q*·(0, 0, 1), in the landblock frame
(Z up).

- **Projection direction** is +Y′: the volume extends from *p* along +Y′, and the surfaces
  it marks face back along −Y′.
- **Texture axes**: u grows along +X′, v grows along −Z′ (§4.5).

<a id="s4-2"></a>
### 4.2 The box (DC8, DC9)

Let *s* = (*s_x*, *s_y*, *s_z*) be the effective `OrthoScale` (§3), unchanged.

1. **Centre** *c* = *p* + (*s_y* / 2) × Y′, using the raw *s_y*.
2. **Extents** *e* = *s*, except that every component strictly below 0.1 becomes 1.0.
3. The **box** is the set of points *P* with |(*P* − *c*) · X′| ≤ *e_x* / 2,
   |(*P* − *c*) · Y′| ≤ *e_y* / 2 and |(*P* − *c*) · Z′| ≤ *e_z* / 2.

So for *s_y* ≥ 0.1 the box runs from the SED's position (local y = 0) to local y = *s_y*, and
the SED's position lies on the centre of the box face from which the projection starts. For
*s_y* < 0.1 (the 94 decals with *s_y* = 0) the box is 1 m deep and centred on *p*.

**Scene bounds.** For visibility and distance (§7) the client uses the box around *p* with
the **raw** *s*: local x ∈ [−*s_x*/2, *s_x*/2], y ∈ [0, *s_y*], z ∈ [−*s_z*/2, *s_z*/2],
transformed by the frame.

**Check against the data (Confirmed, DC9).** Of the 16,511 Decal SEDs, local +Y points down
(Y′.z < −0.9) in 11,253 (4,213 outdoor, 7,040 interior), lies within 17° of horizontal
(|Y′.z| < 0.3) in 3,603 (wall decals), points obliquely down in 1,087, obliquely up in 162
and up in 406. For the 4,213 outdoor down-pointing decals, the SED's height above the terrain
(the heightmap bilinearly interpolated at *p*) lies between −0.5 m and *s_y* + 0.5 m in
3,664; 524 are higher (decals over bridges, platforms and other placed geometry) and 25 more
than 0.5 m below the terrain. The ground the box is meant to reach is therefore inside it.

<a id="s4-3"></a>
### 4.3 Collecting receiving triangles (DC10)

The builder collects candidate triangles, each with three vertices of position and normal, in
the landblock frame of the decal. The order matters for §4.4.

1. **Placed objects and cell geometry** (only when `StickToWorldEntities` or `StickToCells`
   is true). The client queries its scene for objects whose bounds overlap the world-space
   axis-aligned box around the eight corners of the box. Each candidate contributes when all
   of these hold:
   - it is an **entity** object and `StickToWorldEntities` is true, or it is **cell
     geometry** (the render meshes of an interior cell, `landblock.md` §4.4) and
     `StickToCells` is true; other scene object kinds are not traced (OPEN below);
   - it is not the decal's owner (only script decals have one, §8);
   - it accepts decals (§5);
   - for an entity: its model has no skinned part (`mesh.md` §8.6), it is not attached to
     another object (an internal link of the scene node; Inferred), and at the moment of
     collection it is drawn in the current view (inside its draw distance of `mesh.md` §8.7)
     with an opacity above 0.0002, and not in further internal node states (OPEN Q4).

   An entity contributes the triangles of its parts' meshes, transformed by its scale,
   rotation and position into the decal's frame, with its vertex normals rotated likewise; a
   cell contributes its render meshes the same way.
2. **Terrain** (only when `StickToLandscape` is true **and** the decal's normalised frame is
   valid and outdoors: cell = the outdoor value, `coordinates.md` §7). Candidates are the
   triangles of the full-resolution terrain mesh (`terrain.md` §1; each 5 m quad split by its
   stored diagonal, `landblock.md` §3.6) of every quad of every loaded landblock that meets the
   horizontal square of half-width |*e*| (the length of the extents vector) around *c*, with
   the terrain vertex normals of `landblock.md` §3.14. Terrain triangles are appended **after**
   all triangles of step 1. A landblock whose terrain is not loaded contributes nothing.
3. **Water** (only when `StickToWater` is true). If the client finds a water surface at the
   decal (a cast against the water collision at *c*; `water.md` §6.3), let *h* be its height
   and *w* = min(*h*, *c_z* + *e_z*). Every collected vertex whose height is below *w* is
   raised to *w*. The decal then lies on the water surface where the receivers are under
   water. (14 outdoor decals use this.)

Any superset of candidates gives the same mesh, since §4.5 clips to the box; the square of
step 2 only bounds the search.

> **OPEN:** (a) the scene object kinds other than entities and cell geometry (the client's
> object-kind tests let some other kinds through; which kinds are in the queried structure
> was not traced); (b) which detail level of an entity's part meshes is collected (the
> collector asks the part for level 0; whether that is the most detailed level as in
> `mesh.md` §8.3 was not confirmed); (c) the cast that finds the water surface (start,
> length, which water bodies) — `water.md` §6.3 gives the water plane; Tessera uses the
> landblock water plane at *c* when *c* is within its extent (Tessera Decision).

<a id="s4-4"></a>
### 4.4 Back-face test (DC11)

When `UseBackfaceCulling` is true, the test applies to every triangle collected in step 1 of
§4.3, and to terrain triangles (step 2) only when `UseLandscapeBackfaceCulling` is also true.
For a tested triangle with vertices *v₀*, *v₁*, *v₂* (in collection order):

1. Its normal *n* is the stored normal of *v₀* (as collected, not renormalised), or, when
   `UsePreciseBackfaceCulling` is true, the unit vector of (*v₁* − *v₀*) × (*v₂* − *v₀*); a
   cross product of length at most 10⁻⁶ gives *n* = 0.
2. The triangle is **kept** when *n* · (−Y′) ≥ cos(`BackfaceCullingAngle` × π / 180), and
   discarded otherwise.

Untested triangles are always kept. With the default 85° a ground decal keeps surfaces
tilted up to 85° from facing the projector and drops vertical walls (*n* · (−Y′) = 0 <
cos 85° ≈ 0.0872).

<a id="s4-5"></a>
### 4.5 Clipping, triangulation, texture coordinates and lift (DC12)

For each kept triangle:

1. **Clip** the triangle as a polygon successively against the six planes of the box (two
   per local axis). A vertex is inside a plane when its signed distance to it is ≥ 0. Where
   an edge crosses a plane, the new vertex's position **and normal** are interpolated
   linearly with the same weight (the normal is not renormalised). A polygon left with fewer
   than three vertices produces nothing.
2. **Triangulate** the clipped polygon as a fan from its first vertex: for *k* vertices,
   triangles (0, *i*, *i* + 1) for *i* = 1 … *k* − 2. The source triangle's winding is kept.
3. **Texture coordinates** of each output vertex *P* (before step 4):
   u = ((*P* − *c*) · X′) / *e_x* + 0.5, v = 0.5 − ((*P* − *c*) · Z′) / *e_z*.
   Inside the box both lie in [0, 1].
4. **Lift**: *P* ← *P* + 0.008 × *N*, with *N* the vertex's (interpolated) normal.

Each output vertex keeps position, normal and (u, v).

<a id="s4-6"></a>
### 4.6 Mesh centre and mean normal

After building, when `LightingBlend` ≠ 0, the client derives two values used by §6.3:

- the **mesh centre** *m*: the centre of the axis-aligned bounding box of all output vertices;
- the **mean normal** n̄: the sum of the normals of all output vertices divided by their
  number, minus 4 × Y′, normalised (zero when its length is at most 10⁻⁶). The −4 Y′ term
  pulls it strongly towards the direction facing the projector.

<a id="s4-7"></a>
### 4.7 Limits

When a decal produces more than 8,000 triangles the client only logs a performance warning;
it keeps and draws all of them. There is no other limit (BIN).

<a id="s4-8"></a>
### 4.8 When the mesh is built (DC21)

- The object starts marked; the mesh is first built at the first update of its scene node
  (§2.3 step 4, §7), that is, the first frame in which the scene processes the node.
- Every parameter setter and the frame setter compare the new value with the stored one
  (floats and frame components with a tolerance of 0.0002, integers and booleans exactly);
  a change marks the object; at its next update (§7, while its scene node is processed) a
  marked object's mesh is discarded and rebuilt from the current parameters.
- Nothing else was found to rebuild a placed decal: receivers that load, unload, move or
  become visible later do not by themselves trigger a rebuild (BIN, as far as traced).
- A decal whose build produced no triangle stays registered and draws nothing.

> **OPEN (behaviour to confirm by owner observation):** because §4.3 step 1 takes only
> entities that are drawn in the view at the moment of the build, and §4.8 does not rebuild
> when receivers appear, a decal created in the same pass as the objects it should lie on
> may miss them in the original. Whether this is visible in play (missing decals on objects
> after a teleport, for example) was not checked. **Tessera Decision:** build a decal once
> its landblock's terrain and the resident objects in its box are available, and rebuild it
> when an object or terrain block overlapping its box enters or leaves residency; tests
> must not treat this as the original's timing.

---

<a id="s5"></a>
## 5. Receiving surfaces (BIN, DC13)

<a id="s5-1"></a>
### 5.1 Placed objects (entities)

An entity's render component keeps two flags from its properties, found by the entity lookup
order of `entity-descriptor.md` §9.2 (per-instance set, then the 0x47 chain, first value
found; dictionary defaults otherwise):

- **no-decals**, set when `Render_AllowDecals` (dictionary default true) is false;
- **force-decals**, set when `Render_ForceDecals` (dictionary default false) is true.

The entity **accepts** decals when force-decals is set, or when no-decals is clear **and**
its scene node is in neither of two internal states that some scene-node kinds set on
themselves (not traced, OPEN below). In addition, §4.3 step 1 requires a model without
skinned parts, no attachment to a parent, and visibility at build time; `Render_ForceDecals`
does not lift those conditions.

<a id="s5-2"></a>
### 5.2 Cell geometry

An interior cell's render geometry accepts decals exactly when the cell's own
`Render_AllowDecals` is not false (cell property set in landblock info, `landblock.md` §4.4;
default true). `Render_ForceDecals` is not consulted for cells.

<a id="s5-3"></a>
### 5.3 Terrain and water

Terrain has no receiving property: it receives every decal that sticks to the landscape and
is outdoors (§4.3 step 2). Water is not a receiver; `StickToWater` only flattens the
collected triangles onto the water surface (§4.3 step 3).

<a id="s5-4"></a>
### 5.4 Census (Confirmed, DC14)

Effective values by the entity lookup order, over all SEDs:

| SED type | `Render_AllowDecals` | `Render_ForceDecals` | Accepts by §5.1 (node states aside) |
|---|---|---|---|
| Physics (7) | false: 53,311 from the instance set, 312,939 from the object chain; true: 738 (instance set), 5,596 (object chain); absent otherwise | true: 1,624 (instance set), 6,398 (object chain); false: 135 (instance set) | refuses 366,243; accepts 2,014,456 (8,022 of them with force-decals, 7 of those with `Render_AllowDecals` false) |
| PatrolPoint (23) | false on 9 (instance set) | — | markers, not drawn |
| all other types | absent | absent | — |

Elsewhere: on the 0x47 objects' own sets `Render_AllowDecals` false 1,690, true 22,
`Render_ForceDecals` true 40; in landblock instance sets of IIDs that no SED places
(server-spawned objects) `Render_AllowDecals` 2,979 times; in interior cells 10 times (false
once); in one 0x79 property set. For the Physics SEDs the counts taken from the SEDs' own
sets equal those taken from the instance sets, value by value.

> **OPEN:** the two internal scene-node states of §5.1 (flag bits of the scene node that one
> traced node kind sets for itself unconditionally; ordinary entity nodes were not seen to
> set them). **Tessera Decision:** treat ordinary placed objects as not in those states, so
> acceptance is `Render_ForceDecals` or `Render_AllowDecals`.

---

<a id="s6"></a>
## 6. Shading and drawing (BIN unless marked)

<a id="s6-1"></a>
### 6.1 Material (DC19)

`DecalVolume_Material` names a **surface record** (prefix 0x31, `material.md` §4). The decal
mesh is drawn with that surface record's shader record and materials, resolved by
`material.md` §9.2 (effective parameter set, texture slots, blend state, depth test and depth
write of the record). When the record cannot be found the client draws nothing for the batch
(§6.6).

Census of the 140 surface records used by decals (Confirmed; families of `material.md`
§11.3):

| Shader family | Blend | Surface records | Depth write |
|---|---|---|---|
| unlit | alpha blend (SRCALPHA, INVSRCALPHA) | 118 (9 shader records) | off in 114, on in 4 (shader record 0x2B000755) |
| unlit | additive (SRCALPHA, ONE) | 8 (3 shader records) | off |
| unlit | DESTCOLOR, ONE | 11 | off |
| unlit | ZERO, SRCCOLOR (multiply) | 1 | off |
| lit | alpha blend | 2 (shader record 0x2B0007A6) | on |

Alpha test is on in 5 of them (records of 0x2B000755, 0x2B000760 and 0x2B0007A6). 13 of the 15 shader records carry a colour input in their
program signatures (both back ends), so the decal's vertex colour (§6.2) multiplies their
output as `lighting.md` §5.13 describes (Inferred from the signatures, OBS 2026-10-08); the
records 0x2B000755 and 0x2B0007A6 (6 surface records) show no colour input, so for them the
vertex colour has no effect.

<a id="s6-2"></a>
### 6.2 Vertex format and per-decal colour (DC15)

The client writes the decal's triangles each frame into a dynamic vertex buffer with this
layout (36 bytes per vertex):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 12 | f32 × 3 | position (relative to the renderer's origin) |
| 12 | 12 | f32 × 3 | normal |
| 24 | 4 | u8 × 4 | colour, packed (byte order follows the back end) |
| 28 | 8 | f32 × 2 | u, v (§4.5) |

All vertices of a decal get the same colour (except under the height fade of §6.4). It is
computed per frame from:

- *C* = (*C_r*, *C_g*, *C_b*, *C_a*), the effective `ColorAndOpacity` as floats: stored bytes
  B, G, R, A, each divided by 255 (`property-system.md` §4.7);
- *L* = (*L_r*, *L_g*, *L_b*, *L_a*), the light colour: (1, 1, 1, 1) when `LightingBlend` is
  0, otherwise §6.3;
- *α₀* = *C_a* × *o*, with *o* the opacity of §6.4;
- *ℓ* = (*L_r* + *L_g* + *L_b*) / 3.

| `LightingBlend` | Colour (RGB) | Alpha before the final clamp |
|---|---|---|
| 0 `None` (and any value not listed) | *C* | *α₀* |
| 1 `AlphaMultiply` | *C* | *α₀* × *ℓ* |
| 2 `AlphaSubtract` | *C* | max(0, *α₀* − *ℓ*) |
| 3 `Subtract` | max(0, *C* − *L*) per channel | max(0, *α₀*) |
| 4 `Add` | min(1, *C* + *L*) per channel | min(1, *α₀*) |
| 5 `Multiply` | *C* × *L* per channel | *α₀* × *L_a* |
| 6 `AlphaAdd` | *C* | min(1, *α₀* + *ℓ*) |

The alpha is then clamped to [0, 1]. Each channel is packed to a byte by multiplying by 255
and truncating towards zero.

So `LightingBlend` decides whether a decal is lit and how: mode 5 (the most common, 9,679
decals) tints the decal by the light like an ordinary lit surface; mode 1 fades it in the
dark; modes 2 and 6 make it fade out or in with brightness; modes 3 and 4
darken or brighten by the light; mode 0 (5,626 decals) is unlit.

<a id="s6-3"></a>
### 6.3 The decal's light colour (DC16)

Only for `LightingBlend` ≠ 0, each frame the decal is processed:

1. Start with *L* = (*A_r*, *A_g*, *A_b*, 1), where *A* is the mean of two colours of the
   frame's light environment (OPEN below).
2. For the frame's main directional light (sun or moon, `lighting.md` §8.1), when present,
   and then each light in the decal's light list (`lighting.md` §8.5; the decal is a scene
   node like other objects):
   - direction *ℓ̂*: for a directional light, its direction vector (pointing from the light
     into the scene); for a point light, the unit vector from the light's position to the
     mesh centre *m* (§4.6), with *d* the distance between them;
   - factor *f* = max(0, −*ℓ̂* · n̄) × *I*, with n̄ the mean normal (§4.6) and *I* =
     clamp(DiffuseIntensity(*t*) × *σ*, 0, 1) × *a*, using the light's fields of
     `lighting.md` §8.3; *a* = 1 / (1 + 10 × Falloff × *d*² / Range²) for a point light
     (no cut-off at the range on this path) and 1 for any other light;
   - add the light's colour × *f* to *L_r*, *L_g*, *L_b*.

   The client reads these from the light object's own fields: a float curve and a run-time
   scale for *I*, a colour for "the light's colour"; that they are DiffuseIntensity, *σ* and
   Diffuse of `lighting.md` §8.3 is Inferred from the field use (the constant 10 and the
   squared range match that section's attenuation).
3. Multiply *L* channel by channel (alpha included) by the decal node's tint *T* (OPEN
   below), then clamp every channel to [0, 1].

> **OPEN:** (a) the two colours averaged into *A* (two four-component colours of the frame's
> render state; if they are the up- and down-facing ambient of `lighting.md` §5.3, *A₀* + *H₀*
> and *A₀* − *H₀*, their mean is *A₀* — Inferred). **Tessera default:** *A* = *A₀*.
> (b) the tint *T*: a per-node value of four floats that the client refreshes every 20 s;
> for outdoor positions it blends towards a value the client samples at the decal's position
> from a frame-level lighting object (Inferred: baked landscape lighting, `lighting.md` §8.6) by
> a factor of at most 0.85, and it is (1, 1, 1, 1) indoors (not traced
> further). **Tessera default:** *T* = (1, 1, 1, 1).

<a id="s6-4"></a>
### 6.4 Opacity (DC17)

The opacity *o* of §6.2 is the product of:

- the decal node's **fade** value for the view (§7; 1 while fully visible);
- the opacity factor of the decal object (1.0, not set by data);
- when `UseDepthBasedOpacity` is true: clamp(1 − (`DepthBasedOpacityMin` − *d_v*) / *r*, 0, 1),
  with *r* = `DepthBasedOpacityRange` when positive, else 1, and *d_v* the node's per-view
  camera distance (the distance of §7). The decal is fully opaque from `…Min` outwards and
  fades to nothing at `…Min` − *r* towards the camera.

When `UseHeightBasedOpacity` is true, each vertex's alpha is further replaced by
max(0, (1 − |*z_v* − *z_c*| / *r_h*) × *α*), where *α* is the decal's clamped alpha, *z_v* the
vertex height after the lift, *z_c* the height of the box centre *c*, and *r_h* =
`HeightBasedOpacityRange` when positive, else 1.

No decal of this install enables either fade (§3.3), so a faithful renderer may omit both;
they are given for completeness.

<a id="s6-5"></a>
### 6.5 Depth bias (DC18)

**The table.** At start-up the renderer fills a table of 16 depth-bias entries, each three
floats (absolute bias, slope-scale bias, maximum bias), all zero to begin with. It reads two
stand-alone property sets (prefix 0x78) found through the `UNIQUEDB` DID map
(`property-system.md` §8): key **0x163** (label `EngineDepthBiasDefs`, 0x780001F5 here)
first, then key **0x164** (`GameDepthBiasDefs`, 0x780001F6 here). In each it reads the array
`Render_DepthBiasDefList`; every element is a `Render_DepthBiasDef` struct with
`Render_DepthBias_Type` (the entry index, mapper 0x23000297), and optional
`Render_DepthBias_AbsoluteDepthBias`, `Render_DepthBias_SlopeScaleDepthBias` and
`Render_DepthBias_MaxDepthBias` (each 0 when absent). An index of 16 or more is reported and
skipped; a later definition of an index replaces an earlier one. For one of the graphics back
ends (the one with internal back-end code 0; which one was not identified) the slope-scale
value is negated when stored.

On this install the engine set defines indices 0, 1, 2, 13, 14 and 15 and the game set
indices 3–12; every definition gives only a slope-scale value (absolute and maximum are
absent, so 0); index 0 has no value (no bias). The values carry the sign the labels suggest:
negative for `BiasForward*`, `SlopeScale_Negative*`, positive for `BiasBackward*` and the
positive `SlopeScale_*` labels, with magnitudes from 0.4 to 10. Read them at run time.

**Decals.** A decal batch's draw sets the device's depth-bias state to the entry
`DecalVolume_DepthBias` selects and resets it to entry 0 afterwards (BIN). That the three
floats become the constant bias, the slope-scale bias and the bias clamp of the back end's
rasteriser state is Inferred from their names; the back-end code was not read. The default 1 (`SlopeScale_NegativeOne`) applies the
engine's slope-scale value of that entry.

**Objects.** `Render_DepthBias_Type` selects an entry of the same table for an entity: the
render component stores the value from the entity's properties (BIN). On Physics SEDs (by the
entity lookup order) it occurs 52,028 times: 0: 2,567; 1: 64; 2: 42; 3: 20,290; 4: 2,417;
5: 1,188; 6: 272; 7: 1,936; 8: 18,073; 9: 2,080; 10: 1,174; 11: 423; 12: 1,468; 13: 30;
15: 4 (Confirmed). How the object draws apply it was not traced (OPEN below).

**Tessera Decision (sign).** With a depth buffer whose values grow with distance, apply the
stored slope-scale value unchanged (negative pulls the surface towards the camera, matching
the `Forward` labels); with a reversed depth buffer, negate it. Tests must not treat the sign
choice for a given back end as the original's.

> **OPEN:** (a) which back end has internal code 0 (the negated slope); (b) how a back end
> scales the absolute bias (unused by this install's definitions); (c) whether and in which
> passes entity draws apply `Render_DepthBias_Type` (Tessera default: apply it to the
> entity's draws like the decal's, so coplanar overlay meshes do not flicker).

<a id="s6-6"></a>
### 6.6 Batching and render phase

The decal manager groups decal objects into **batches** that share the render phase, the
depth-bias index and the material. Each frame, for each phase it draws, the renderer draws
every batch of that phase as one triangle list: the triangles of every decal of the batch
whose node was processed in this frame, in batch order (not sorted by depth), with the
batch's material, its depth bias (§6.5) and no other per-decal state. Decals are drawn only
while a decal switch of the frame's render state is on (its source was not traced; OPEN).

All placed decals of this install use phase 1 (`Default`). The client draws phase-1 batches
in the main view right after its scene-object draw call and phase-4 batches later in the
frame (BIN, call order); that the preceding call draws the opaque objects is Inferred from
the phase labels, and phases 2 and 5 were not traced.
**Tessera:** draw decals after the opaque pass, with depth test on, depth write as the
material says (mostly off), the material's blend, and the depth bias of §6.5.

---

<a id="s7"></a>
## 7. Draw distance and LOD class (BIN, DC20)

1. **LOD distance** *L*: when the decal object is built and whenever `DecalVolume_LODClass`
   changes, the client looks the class up in the LOD-class table (`UNIQUEDB` key 0x3F,
   `environment.md` §9.1) and takes the entry's **first** float `d0`. When the table or the
   entry is missing, *L* keeps its previous value (initially 30.0 m). `Render_LODClass` is not
   used for this (it feeds the streaming view distance of ED §9.3).
2. **Draw range** *D* = min(*G*, *L* × *M_d*), with *G* the global limit and *M_d* the
   `ObjectDrawDistance` multiplier of `mesh.md` §8.7. (BIN: the decal node takes the smaller
   of a frame value and *L* × a second frame value; the entity draw-distance routine reads
   the same two frame values, which is why they are identified with *G* and *M_d*:
   Inferred.)
3. The decal's scene node is visible while its distance *d* (from the camera to the nearest
   point of the world-space axis-aligned box around the scene bounds of §4.2; `mesh.md` §8.6)
   is at most *D*. Crossing *D* fades the node in or out over 1.5 s, with the same rules as
   the `d0` row of `mesh.md` §8.7 (the client runs the same routine for both); the fade value
   is the *o* factor of §6.4.
4. A decal is drawn in a frame only if its node was processed (visible or fading) in that
   frame (§6.6).

On this install the classes used by decals have `d0` = 40 m (class 1, 7,711 decals), 80 m
(0x10000003 and 0x10000008, 8,284), 180 m (0x1000000B, 303), 300 m (0x1000000C, 87) and
1,000 m (0x10000010, 126) (observations; read the table at run time).

---

<a id="s8"></a>
## 8. Relation to the script Decal node (BIN, DC22)

The script Decal node (kind 1, `DecalScript_*` layout in `script-table.md` §5.3) uses the
same decal class when it is `Managed`, and the decal manager directly (a temporary decal with
lifespan and ramps) when it is neither `Managed` nor `Persistent`; `Persistent` without
`Managed` is an error. Its fields map onto the parameters of §3.1:

| Script field | Decal parameter |
|---|---|
| `Material` | `Material` |
| `Color` | `ColorAndOpacity` |
| `Size` (float curve, value at the start) × `Size3D` (per component) | `OrthoScale` |
| `LightingBlend`, `UseBackfaceCulling`, `UsePreciseBackfaceCulling`, `UseLandscapeBackfaceCulling`, `BackfaceCullingAngle`, `UseHeightBasedOpacity`, `HeightBasedOpacityRange`, `UseDepthBasedOpacity`, `DepthBasedOpacityMin`, `DepthBasedOpacityRange`, `StickToWorldEntities`, `StickToCells`, `StickToLandscape`, `StickToWater`, `RenderPhase`, `DepthBias`, `LODClass` | the parameter of the same name |
| `StickToParentEntity` false | the parent entity becomes the decal's owner and does not receive it (§4.3) |

`Origin`, `Rotation`, `Radius`, `Lifespan`, `RampInTime`, `RampOutTime`, `HoldingLocation`,
`BoneType`, `BoneOffset`, `RemoveBoneRotation`, `AttachToParent` and `ScaleWithParent` place
and time the script decal; they belong to the script runtime.

> **OPEN (for the owner of `script-runtime.md`):** the frame of a script decal (origin,
> rotation in degrees, random offset within `Radius`, holding location and bone), the
> lifespan and ramp behaviour of temporary decals, and the difference between the two
> `Managed` registrations. Placed decals do not depend on these.

---

<a id="s9"></a>
## 9. Guidance for Tessera

<a id="s9-1"></a>
### 9.1 World walker

Realise a decal for every Decal node that `entity-descriptor.md` §9.3 creates (layer test,
interior viewer rule), with the parameters of §3 and the mesh of §4. Remove it when the node's
entity is removed. Draw it per §6–§7. The SED's marker object is never drawn.

<a id="s9-2"></a>
### 9.2 Order of work (Tessera Decision)

A walker that does not yet light decals may draw every decal with *L* = (1, 1, 1, 1): modes
0, 1, 5 then show the decal colour unchanged, which is the original's look in bright light.

<a id="s9-3"></a>
### 9.3 Explorer preview (Tessera Decision)

For a selected Decal SED the explorer shows the box of §4.2 as a wireframe with the local
axes, and a preview mesh built by §4.3–§4.5 from a synthetic receiver: a flat square facing
−Y′ through the box centre (normal −Y′), larger than the box, drawn with the decal's material
and the colour of §6.2 for *L* = (1, 1, 1, 1). With the landblock loaded it may instead build
the real mesh against the loaded terrain and objects.

---

<a id="s10"></a>
## 10. Invariants

Data (checkable on the install; Confirmed 2026-10-08 unless noted):

1. Every Decal SED names a 0x47 object whose chain yields `DecalVolume_Material`, and the
   value is the DID of an existing surface record (prefix 0x31).
2. Every Decal SED's per-instance property set holds the same entries as its own property
   set (the same PIDs with the same values) and has the same header; the stored order of the
   entries may differ and is not part of the comparison (DC3; differs in 1,671 of 16,511).
3. `DecalVolume_*` properties occur on Decal SEDs, their instance sets and the 140 marker
   objects only, apart from one Light SED.
4. `UseHeightBasedOpacity` and `UseDepthBasedOpacity` are false (or absent) for every decal;
   `RenderPhase` is 1 for every decal.
5. Every effective `DepthBias` value is below 16, and every effective `LODClass` value has an
   entry in the LOD-class table.

Builder (synthetic fixtures):

6. Every output vertex, before the lift, lies inside the box (§4.2) within 10⁻⁴ m, and its
   (u, v) lies in [0, 1] within 10⁻⁵.
7. One source triangle yields at most 7 output triangles (a triangle clipped by six planes
   has at most 9 vertices).
8. A source triangle entirely inside the box and kept by §4.4 yields exactly itself (same
   vertices, lifted by 0.008 m along the normals).
9. An `OrthoScale` component below 0.1 gives an extent of 1.0; a component of exactly 0.1
   gives 0.1.
10. With `LightingBlend` 0, the packed colour of every vertex equals the packed
    `ColorAndOpacity` with alpha × *o*, whatever the lights.
11. Swapping receivers' collection order between entities and terrain changes the result
    only through the back-face test (§4.4: terrain triangles follow
    `UseLandscapeBackfaceCulling`).

---

<a id="s11"></a>
## 11. Worked examples (synthetic values)

**Example 1: a ground decal on flat terrain.** SED position *p* = (80, 80, 105), rotation
*q* = (0.70710678, −0.70710678, 0, 0) (a turn of −90° about X). Then X′ = (1, 0, 0),
Y′ = (0, 0, −1), Z′ = (0, 1, 0): the decal projects straight down. `OrthoScale` (4, 2, 6).
Centre *c* = *p* + 1 × Y′ = (80, 80, 104); extents (4, 2, 6); the box is x ∈ [78, 82],
z ∈ [103, 105], y ∈ [77, 83]. Flat terrain at height 104 with normal (0, 0, 1): *n* · (−Y′) =
1 ≥ cos 85°, so terrain triangles are kept; clipping leaves the 4 m × 6 m rectangle. Corner
(82, 83, 104): u = 2/4 + 0.5 = 1.0, v = 0.5 − 3/6 = 0.0; corner (78, 77, 104): u = 0.0,
v = 1.0. After the lift the corners are at height 104.008. A wall triangle in the box with
normal (1, 0, 0) gives *n* · (−Y′) = 0 < 0.0872 and is dropped.

**Example 2: zero depth.** Same frame with `OrthoScale` (4, 0, 6): *c* = *p* = (80, 80, 105)
(raw *s_y* = 0), extents (4, 1, 6), box z ∈ [104.5, 105.5]. Terrain at 104 is outside: the
decal produces no triangle. The scene bounds are local y ∈ [0, 0].

**Example 3: colour.** `ColorAndOpacity` stored bytes (B, G, R, A) = (0, 128, 255, 204):
*C* = (1.0, 0.50196, 0.0, 0.8). Opacity *o* = 1, light *L* = (0.6, 0.6, 0.5, 1.0),
*ℓ* = 0.56667.
- Mode 0: RGBA = (1.0, 0.50196, 0.0, 0.8) → bytes (255, 128, 0, 204).
- Mode 5: (0.6, 0.30118, 0.0, 0.8) → (153, 76, 0, 204) (76.8 truncates to 76).
- Mode 1: colour unchanged, alpha 0.8 × 0.56667 = 0.45333 → 115.
- Mode 2: alpha max(0, 0.8 − 0.56667) = 0.23333 → 59.
- Mode 4: (min(1, 1.6), min(1, 1.10196), 0.5, 0.8) → (255, 255, 127, 204).

**Example 4: decal light.** Mean normal n̄ = (0, 0, 1), mesh centre *m* = (80, 80, 104).
Ambient mean *A* = (0.2, 0.2, 0.25), no directional light, one point light at (80, 84, 107)
with Diffuse (1.0, 0.8, 0.5), DiffuseIntensity 0.9, *σ* = 1, Range 10, Falloff 0.5, tint
*T* = (1, 1, 1, 1). Vector light → *m*: (0, −4, −3), *d* = 5, *ℓ̂* = (0, −0.8, −0.6);
−*ℓ̂* · n̄ = 0.6; *a* = 1 / (1 + 10 × 0.5 × 25 / 100) = 0.44444; *I* = 0.9 × 0.44444 =
0.4; *f* = 0.24. *L* = (0.2 + 0.24, 0.2 + 0.192, 0.25 + 0.12, 1) = (0.44, 0.392, 0.37, 1).

**Example 5: draw distance.** Class entry `d0` = 50 m, *M_d* = 1.4, *G* = 2,080 m:
*D* = 70 m. A class without an entry and no earlier value: *D* = min(2,080, 30 × 1.4) = 42 m.

**Example 6: depth-bias table.** Engine set entries {type 1, slope −1.0} and {type 13,
slope −0.4}; game set entry {type 3, slope −0.8} and a second {type 1, slope −2.0}. The
table: index 1 = (0, −2.0, 0) (the game set replaces the engine value), 3 = (0, −0.8, 0),
13 = (0, −0.4, 0), every other index (0, 0, 0). On the back end with code 0 the slopes are
stored as +2.0, +0.8, +0.4.

---

<a id="s12"></a>
## 12. Open questions (summary)

| # | Question | Section | Tessera default meanwhile |
|---|---|---|---|
| Q1 | Trigger of the second decal-creation routine | §2.3 | streaming-index path only |
| Q2 | Other scene object kinds as receivers; entity detail level collected; the water cast | §4.3 | entities and cell geometry; most detailed level; landblock water plane |
| Q3 | Visible effect of build-time receiver selection; other rebuild triggers | §4.8 | rebuild on residency changes (Decision) |
| Q4 | The two internal scene-node states of §5.1 | §5.1 | ordinary objects not in them |
| Q5 | The two ambient colours averaged into *A*; the tint *T* | §6.3 | *A* = *A₀*; *T* = 1 |
| Q6 | Back end with code 0; absolute-bias scaling; entity use of `Render_DepthBias_Type` | §6.5 | sign by depth convention; apply to entities |
| Q7 | Source of the frame's decal switch; draw positions of phases 2, 4, 5 | §6.6 | decals always on; all phases after the opaque pass |
| Q8 | Script decal frame, lifespan and ramps | §8 | owned by `script-runtime.md` |

---

<a id="s13"></a>
## 13. Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (Review). New spec for placed decal volumes, answering `entity-descriptor.md` §9's "decal volumes have no spec yet" (its owner may point the OPEN there to this spec). Census of all Decal SEDs, marker objects, instance sets, receiving properties and depth-bias definitions (OBS 2026-10-08); client behaviour from one BIN session (2026-10-08, questions 1–10). |
| 2026-10-09 | Non-blocking notes of the first review taken up (spec writer): §6.5, the total of `Render_DepthBias_Type` on Physics SEDs is 52,028 (the per-value counts were right; their sum was misprinted as 51,028); DC3 and the §2.1 table now say that the instance set holds the same entries as the SED's own set, in another stored order in 1,671 of 16,511 (re-counted over all landblock-info records). No rule changed. Status Revised, awaiting review. |
| 2026-10-09 | Blocking item (1) of the second review fixed (spec writer): §10 invariant 2 no longer says the per-instance set "equals" the SED's own set; it now requires the same entries (PIDs and values) and the same header, independent of stored order, as DC3 states. No rule or count changed. Status Revised, awaiting re-review. |
