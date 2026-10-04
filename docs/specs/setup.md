<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setups and part assembly (part 1: static objects and the paths to geometry)

| | |
|---|---|
| **Status** | Review (part 1 of 2) |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` (entity descriptor 0x47, visual template 0x1F, setup 0x04, appearance 0x20 decoders), `crates/mesh` / `crates/assets` (resolving an object to meshes), `apps/explorer` |
| **Companion specs** | `mesh.md` (the 0x06 parts). This spec holds the full 0x20 grammar (§8) that `material.md` §9.3 refers to as a future appearance spec. `property-system.md` §5 (property sets embedded in 0x47; item property sets 0x79). `records.md` §1 (VLE, TSize). `material.md` (0x30 materials, 0x2B shaders named by appearance records). |
| **Part 2 (later, M4)** | Skeleton decoding inside setups, attachment of held items to bones, character appearance assembly. |

---

<a id="s0"></a>
## 0. Purpose

A mesh record (0x06) is one part. Whole objects (a barrel, a tree, a weapon, an NPC) are
assembled by a short chain of records. This part of the spec covers:

- which DID prefixes take part and what each one is (§2);
- the chains from an item or object to its meshes, with resolution statistics (§3);
- the **entity descriptor** (0x47), **visual template** (0x1F) and **setup** (0x04) record
  layouts, decoded byte-exactly over all records (§4–§6);
- how a static object's parts are placed (§7);
- the worn-appearance record (0x20), the second path from items to meshes (§8);
- invariants and open questions (§9–§10).

Skeleton contents (the Havok data inside a setup) and animation are part 2.

---

<a id="s1"></a>
## 1. Provenance

Legend as in `mesh.md` §1. Confirmed facts were decoded on the user's install on 2026-10-04
by throwaway probes in the dirty-room scratchpad.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04. **All** 40,643 records 0x47 (gamelogic), 29,134 records 0x1F, 28,555 records 0x04 and 4,201 records 0x20 (general) decoded to the last byte. All 383,332 item property sets (0x79) scanned for references to these prefixes, using the repository's own property-set decoder. Every chain resolved to its mesh records. Three multi-part setups rendered and inspected. |
| MV | Mirarkitty `lotro_mesh_viewer` docs: `weapons.md` (held-item chain 0x47 → 0x1F → 0x04 → 0x06), `wardrobe.md` (0x20 grammar), `properties.md`. Facts only. |
| DS | `research/notes/data-semantics.md` §6.4 (entity descriptors in landblocks), §7.1 (item → geometry chains), §7.2 (0x20 grammar), §8 (Havok tagfile magic). |
| PS | `property-system.md` (embedded property-set form, TSize). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| S1 | Prefix roles: 0x47 entity descriptor, 0x1F visual template, 0x04 setup (Havok tagfile + body + part list), 0x06 part mesh (§2) | MV, DS; OBS | Confirmed |
| S2 | 0x01 records are **not** setups; they are stand-alone collision geometry (§2.2) | OBS | Confirmed (correction of `records.md` §2.1 and DS §1) |
| S3 | Item `PhysObj` property → 0x47 → (0x47)* → 0x1F → (0x1F)* → 0x04 → 0x06 parts (§3.1) | MV; OBS (195,930 of 195,930 item references resolve) | Confirmed |
| S4 | 0x47 layout: flags, DID, a u32, template DID, optional 28-byte block, embedded property set, optional 0x78 list (§4) | MV (first four words); PS; OBS (40,643 of 40,643) | Confirmed |
| S5 | 0x1F layout: flags-driven optional fields, appearance section, setup section (§5) | MV ("last u32 is a 0x04"); OBS (29,134 of 29,134) | Confirmed |
| S6 | 0x04 layout: DID, tagfile length, Havok binary tagfile, body with attachment lists, collision volumes, physics bounds and the part list (§6) | MV (trailer is a counted 0x06 list); DS §8 (magic); OBS (28,555 of 28,555) | Confirmed |
| S7 | Parts of a setup share the setup's frame; the part list carries no transforms (§7) | OBS (3 multi-part renders; bounds) | Confirmed for static setups |
| S8 | Worn items: `Item_WornAppearanceMapList` → (0x20, appearance key) → entry → blocks → parts (§8) | MV, DS; OBS (4,201 of 4,201 records; 634,655 map entries) | Confirmed. The part layout differs from MV (§8.3). |

---

<a id="s2"></a>
## 2. Prefixes involved

<a id="s2-1"></a>
### 2.1 Table (Confirmed counts, 2026-10-04)

| Prefix | Archive (count) | Role in this spec | Starts with own DID |
|---|---|---|---|
| 0x47 | gamelogic (40,643) | Entity descriptor: per-object class data, render/physics properties, template link (§4) | yes |
| 0x1F | general (29,134) | Visual template: scale, appearance link, setup link (§5) | **no** (flags word first, DID second) |
| 0x04 | general (28,555) | Setup: skeleton, attachment frames, collision volumes, part list (§6) | yes |
| 0x06 | mesh, mesh_aux_1 (47,563) | Part mesh (`mesh.md`) | yes |
| 0x66 | general (16,819) | Collision mesh named by setups (§6.5). Not decoded here. | **no** (begins with a small count) |
| 0x20 | general (4,201) | Worn-appearance (wardrobe) record (§8) | yes |
| 0x78 | gamelogic (1,616), local (2) | Stand-alone property sets named by some 0x47 records (§4.4) | yes (`property-system.md`) |
| 0x01 | general (1,057) | Stand-alone collision geometry (§2.2). Not part of the object chain. | yes |

<a id="s2-2"></a>
### 2.2 0x01 records are not setups (Confirmed)

Earlier notes and `records.md` §2.1 call 0x01 "setup / model assembly". The data disagrees.
The 0x01 records inspected (a handful, not all 1,057) start with their DID, a byte 1, a VLE
vertex count and that many 3 × f32 positions, followed by u16 triangle indices and a
collision-tree code block shaped like the one in `mesh.md` §10. None of them has a part
list. No 0x1F or 0x04 field can hold a 0x01 DID (§5, §6), and none of the 383,332 item
property sets or the 40,643 entity property sets contains a 0x01 DID value. Setups are the 0x04 records (a DID-map label
ending in "_Setup" also points into 0x04, `records.md` §2.1).

> **OPEN:** Which records reference 0x01 (landblock or cell data is the likely user, M3), and
> their exact layout. `records.md` §2.1 should be updated by its owner: 0x01 = collision
> geometry, 0x04 = setups.

---

<a id="s3"></a>
## 3. From an object to its meshes

<a id="s3-1"></a>
### 3.1 Entity path (held items, props, NPCs, world objects) (Confirmed)

1. Start at a 0x47 entity descriptor. Items name it in the property `PhysObj` of their
   property set (0x79…, `property-system.md` §6). Landblock entity descriptors name it in a
   physics-object field (DS §6.4, Reported; M3).
2. Read the 0x47 record's `template` field (§4.1). If it is another 0x47 DID, repeat with
   that record (a parent entity). Observed hops: 0 (39,145 records), 1 (1,472), 2 (12),
   3 (6), 4 (8).
3. At a 0x1F template: if it has a setup section (§5.3), take its 0x04 DID. Otherwise follow
   its `parent` 0x1F (§5.2); one hop suffices in every observed case (557 chains).
4. At the 0x04 setup, the part list (§6.6) names the 0x06 meshes.

Resolution over the whole install:

| Start | Count | Resolved to ≥ 1 mesh | Setup without parts | Broken link |
|---|---|---|---|---|
| Item `PhysObj` references | 195,930 | 195,930 | 0 | 0 |
| 0x47 records | 40,643 | 40,641 | 2 | 0 |
| 0x1F records | 29,134 | 29,132 | 2 | 0 |
| Part DIDs in all setups | 28,992 | 28,992 exist (28,826 distinct) | | 0 |

A resolver must bound every chain (a limit of 16 hops is ample) and detect cycles.

<a id="s3-2"></a>
### 3.2 Worn path (garments and body parts) (Confirmed)

1. The item property set holds `Item_WornAppearanceMapList`, an array of
   `Item_WornAppearanceMapStruct`. Each struct has `Item_WornAppearance` (a 0x20 DID),
   `Item_AppearanceKey` (an enum key 0x1000xxxx), `Item_SpeciesOfWearer` and
   `Item_SexOfWearer` (0x1000 male, 0x2000 female).
2. Choose the struct matching the wearer's species and sex.
3. In that 0x20 record, find the entry whose `key` equals `Item_AppearanceKey` (§8.2).
4. Its blocks give material bindings and parts (0x06 meshes) (§8.3).

Over all 634,655 map structs: 632,056 find an entry with at least one part, 2,339 find an
entry that only changes materials (no parts), and 260 name a key that the 0x20 record does
not contain. Species values seen: 23, 65, 73, 81, 114, 117, 120, 125.

<a id="s3-3"></a>
### 3.3 Other references found in item property sets (Confirmed census)

| Property (value prefix) | Count | Notes |
|---|---|---|
| `Item_WornAppearance` (0x20), inside the map list | 634,655 | §3.2 |
| `PhysObj` (0x47) | 195,930 | §3.1 |
| `ExternalAppearanceID` (0x20) | 138,503 | Not geometry: MV found these to be material-only appearance records |
| `Appearance_AprFile` (0x20), nested | 67,099 | Appearance instances; character customisation (part 2) |
| 16 other properties naming 0x47 (cosmetic inventory slots, mounted and channelling cosmetics, collection display entities, cosmetic weapon forwards, race-select and shapeshift entities, NPC cosmetic items) | 8,075 in total | Alternative entities; resolve as §3.1 |
| `Trait_Appearance_APR`, `Trait_Appearance_ColorAPR` (0x20) | 3,068 | Part 2 |
| `CloakingAppearanceID`, `MeshFXScript_AppearanceID`, `RaceTable_GenderNullAppearance` (0x20) | 115 | Part 2 |
| `Physics_AlternativeDetectionRep` (0x04) | 26 | A setup named directly |

---

<a id="s4"></a>
## 4. Entity descriptor (prefix 0x47, `client_gamelogic.dat`)

<a id="s4-1"></a>
### 4.1 Layout (Confirmed, 40,643 of 40,643, zero trailing bytes)

| Order | Field | Type | Present | Meaning |
|---|---|---|---|---|
| 1 | `flags` | u32 | always | §4.2 |
| 2 | `did` | DID | always | Own DID (equals the looked-up DID in every record) |
| 3 | `kind` | u32 | always | §4.3 |
| 4 | `template` | DID | always | 0x1F visual template (39,145) or parent 0x47 (1,498) |
| 5 | `block` | f32 × 7 | flags & 0x10 | §4.4 |
| 6 | `properties` | embedded property set | always | TSize count + entries with PID echo (`property-system.md` §5.1–§5.3) |
| 7 | `extra_sets` | u8 *n*, then *n* × DID | flags & 0x200 | 0x78 property-set DIDs (§4.4) |

<a id="s4-2"></a>
### 4.2 `flags` (Confirmed values)

| Value | Records |
|---|---|
| 0x0000008B | 20,344 |
| 0x0000028B | 20,274 |
| 0x0000009B | 25 |

Bits 0x01, 0x02, 0x08 and 0x80 are set in every record; they line up with the fields that are
always present (DID, `kind`, `template`, property set), but that reading is unproven. Bit 0x10
adds `block`; bit 0x200 adds `extra_sets`. A decoder rejects other bit patterns as
unsupported (none occur).

<a id="s4-3"></a>
### 4.3 `kind` (Confirmed values, meaning OPEN)

7 (40,229 records), 3 (240), 2 (141), 9 (18), 23 (8), 1 (2), 5, 19, 21, 31 (1 each). It
looks like a bit set.

> **OPEN:** Meaning of `kind`.

<a id="s4-4"></a>
### 4.4 Optional parts (Confirmed layout, meaning partly OPEN)

- **`block`** (25 records): seven f32. The last four are always (1, 0, 0, 0), an identity
  rotation in (w, x, y, z) order (§6.7). The first three are values such as (0.4, 0.4, 2.7),
  (1, 1, 1), (3, 3, 3), (0, 0, −3), (0, −1.25, 0).

  > **OPEN:** Whether the first three floats are an offset or a scale. With values like
  > (3, 3, 3) and (0, 0, −3) both readings occur; a viewer ignores the block.

- **`extra_sets`**: 20,274 records; *n* = 1 (20,108), 2 (141), 3 (16), 4 (9). 20,474 DIDs,
  all with prefix 0x78; 20,411 of them exist in gamelogic.

  > **OPEN:** Where the 63 missing 0x78 DIDs live (possibly the localized archive), and how
  > the extra sets combine with the embedded set (probably as defaults).

- **Property set.** Common PIDs, by records containing them: `Entity_CreationType` (40,091),
  `Render_LODClass` (33,018), `Render_CastLandscapeShadows` (25,867),
  `Entity_Clearance_BoxList` (21,211), `Entity_ClassScriptTable` (11,334),
  `Appearance_ClassList` (9,336), `Render_NeverSelectable` (6,363), `Translucent` (2,823),
  `Render_Opacity` (1,563), `Render_LODCrossFade` (1,365), `Render_AdjustableScale` (1,309),
  `Render_ImposterName` (1,072). None of them names a mesh. Rendering-relevant semantics are
  OPEN (`material.md` and the M2 renderer decide which ones to honour).

---

<a id="s5"></a>
## 5. Visual template (prefix 0x1F, `client_general.dat`)

<a id="s5-1"></a>
### 5.1 Layout (Confirmed, 29,134 of 29,134, zero trailing bytes)

The record does **not** start with its DID: the first u32 is a flag word.

| Order | Field | Type | Present | Meaning |
|---|---|---|---|---|
| 1 | `flags` | u32 | always | §5.2 |
| 2 | `did` | DID | always | Own DID |
| 3 | `parent` | DID | flags & 0x002 | Parent 0x1F template |
| 4 | `vec3` | f32 × 3 | flags & 0x020 | §5.2 |
| 5 | `value_a` | f32 | flags & 0x040 | §5.2 |
| 6 | `value_b` | f32 | flags & 0x400 | §5.2 |
| 7 | appearance section | | flags & 0x100 | §5.4 |
| 8 | setup section | | flags & 0x200 | §5.3 |

<a id="s5-2"></a>
### 5.2 Flags (Confirmed census)

| Flags | Records | Fields present |
|---|---|---|
| 0x201 | 28,425 | setup |
| 0x103 | 538 | parent, appearance |
| 0x601 | 64 | `value_b`, setup |
| 0x301 | 39 | appearance, setup |
| 0x641 | 34 | `value_a`, `value_b`, setup |
| 0x003 | 13 | parent |
| 0x241 | 12 | `value_a`, setup |
| 0x221 | 7 | `vec3`, setup |
| 0x261 | 2 | `vec3`, `value_a`, setup |

Bit 0x001 is always set. Every record has a setup section or a parent; templates with a parent
never have their own setup section.

`vec3` values are uniform triples such as (0.3, 0.3, 0.3) or (1.03, 1.03, 1.03); `value_a`
and `value_b` take values such as 0.6, 0.9, 1.5, 2.0, 2.5, 3.0.

> **OPEN:** Meaning of `vec3`, `value_a` and `value_b` (scale factors are likely). Their
> stream order when both 0x040 and 0x400 are set (34 records) is assumed to follow bit order;
> the two orders cannot be told apart from the data.

<a id="s5-3"></a>
### 5.3 Setup section (Confirmed)

A TSize (`records.md` §1.3) map with *n* = 1 in every record, whose element is:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `key` | u32 | Always 1 |
| 2 | `type` | u32 | Always 9 (a DID list) |
| 3 | `count` | u32 | Always 1 |
| 4 | DIDs | DID × `count` | 0x04 setup DIDs |

28,583 setup sections name 28,554 distinct setups; every named setup exists. A decoder reads
`type` and `count` generally (rejecting types other than 9 as unsupported) and uses the
first DID.

<a id="s5-4"></a>
### 5.4 Appearance section (Confirmed layout, meaning OPEN)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | three u32 | u32 × 3 | Always 33, 2, 1 |
| 2 | `appearance` | DID | 0x20 record (577 of 577 exist) |
| 3 | modifiers | TSize map, element = u32 key, f32 value | *n* = 1 (571), 2 (4), 46 (2). Key 0x10000034 in 575 sections. |

These templates attach an appearance (`Appearance_…` style key/value pairs) to a parent
template's geometry. For static viewing they are ignored; part 2 covers appearance
modifiers.

---

<a id="s6"></a>
## 6. Setup (prefix 0x04, `client_general.dat`)

<a id="s6-1"></a>
### 6.1 Outer layout (Confirmed, 28,555 of 28,555, zero trailing bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | `tagfile_length` *t* | u32 | Length of the next field |
| 3 | `tagfile` | *t* bytes | Havok binary tagfile; begins with the 8-byte magic `1E 0D B0 CA CE FA 11 D0` in every record. Holds the skeleton (part 2). Common lengths 358 (16,376 records), 402, 351, 349. |
| 4 | `attach_map` | TSize map {u32 → u32} | §6.2 |
| 5 | `bone_frames` | frame list A | §6.3 |
| 6 | `placements` | frame list B | §6.3 |
| 7 | `named_frames` | frame list C | §6.3 |
| 8 | `boxes` | u8 count, × 64 bytes | §6.4 |
| 9 | `spheres` | u8 count, × 16 bytes | §6.4 |
| 10 | `capsules` | u8 count, × 20 bytes | §6.4 |
| 11 | `collision_mesh` | DID | 0x66 DID or 0 (§6.5) |
| 12 | `reserved` | u8 | 0 in every record |
| 13 | `physics_bounds` | f32 × 8 | §6.5 |
| 14 | `part_count` *p* | u8 | Number of parts |
| 15 | `parts` | DID × *p* | 0x06 mesh DIDs (§6.6) |

A decoder that needs only the parts must still parse fields 4–13 (their sizes vary); reading
the part list backwards from the end is not reliable.

<a id="s6-2"></a>
### 6.2 `attach_map` (Confirmed layout, meaning OPEN)

TSize map, element = u32 key, u32 value. Empty in 28,177 setups. Keys are enum values
0x1000xxxx (5,812 elements) or small integers (559); values are below 256 (bone-index sized).

> **OPEN:** Meaning (a slot → bone table for attachments is the likely reading; part 2).

<a id="s6-3"></a>
### 6.3 Frame lists (Confirmed layout)

All three lists start with a u32 count *n*. When *n* = 0 nothing else follows. Otherwise:

| List | Header after *n* | Element size | Element layout |
|---|---|---|---|
| A `bone_frames` | TSize whose count equals *n* | 52 | u32 `key`, u32 `key2`, i32 `bone`, frame (40 bytes) |
| B `placements` | u32 equal to *n* | 44 | u32 `id`, frame (40 bytes) |
| C `named_frames` | TSize whose count equals *n* | 48 | u32 `key`, u32 `key2`, frame (40 bytes) |

**Frame** (40 bytes): position f32 × 3, scale f32 × 3, rotation quaternion f32 × 4 in
**(w, x, y, z)** order (§6.7).

Observed: list A in 2,737 setups (25,118 elements; `key2` = `key` always; `bone` ≥ −1,
−1 probably meaning "not bound to a bone"; scale (1, 1, 1) in 24,190 elements); list B in 42 setups
(50 elements, `id` = 0); list C in 50 setups (54 elements, `key2` = `key`). No setup has more
than one of the three lists.

> **OPEN:** What list B and list C place (they hold 1–3 frames in setups with one part, so
> they are not per-part transforms). List A is probably the attachment-point table (its keys are
> 0x1000xxxx enum values and its elements name bones); its use is part 2.

<a id="s6-4"></a>
### 6.4 Collision volumes (Confirmed)

| Volume | Size | Layout | Setups with any |
|---|---|---|---|
| Box | 64 | local min corner f32 × 3, local max corner f32 × 3, then a frame (§6.3: position, scale, rotation) | 997 |
| Sphere | 16 | centre f32 × 3, radius f32 | 1,911 |
| Capsule | 20 | centre of the lower cap f32 × 3, length between cap centres f32, radius f32 | 719 |

Semantics were confirmed against `physics_bounds` (§6.5): for all 1,910 sphere-only and all
715 capsule-only setups the bounds equal the union of the volumes computed this way (a
capsule spans z from centre − r to centre + length + r). For box-only setups, the corners
moved by the frame position match the bounds in 865 of the 874 setups whose boxes are
unrotated; the 108 setups with rotated boxes need the rotation applied (not checked), and 9
unrotated ones differ (OPEN).

<a id="s6-5"></a>
### 6.5 Collision mesh and physics bounds (Confirmed)

- `collision_mesh` is a 0x66 DID in 16,822 setups (all exist; 16,819 distinct) and 0 in the
  rest. The 0x66 layout is out of scope for M2 (a counted vertex list followed by
  triangles; not decoded here).
- Exactly one kind of collision is typical (all 28,555 setups): mesh only 16,812, sphere
  only 1,910, box only 990, capsule only 715, none 8,117; mixtures 11 (mesh + box 6,
  mesh + capsule 4, box + sphere 1).
- `physics_bounds`: an AABB (min x, y, z, max x, y, z), then the **height** (max z − min z;
  Confirmed on all 20,438 setups that have collision) and the **horizontal radius** (largest
  distance of the collision from the vertical axis through the origin; Confirmed for
  spheres, capsules and unrotated boxes). All eight are 0 when the setup has no collision
  (8,117 setups).

These bounds describe collision, not the visible geometry. Render bounds come from the part
meshes (`mesh.md` §6.1).

<a id="s6-6"></a>
### 6.6 Part list (Confirmed)

`part_count` (u8) then that many 0x06 DIDs.

| Parts | Setups |
|---|---|
| 0 | 2 |
| 1 | 28,407 |
| 2 – 12 | 146 |

Of the 146 multi-part setups, 116 have only skinned parts (bodies split into head, torso,
legs …), 27 only static parts, and 3 a mix. All 28,992 part DIDs exist in the mesh archives.

<a id="s6-7"></a>
### 6.7 Rotation order (Confirmed statistically)

In every frame list and in boxes, the identity rotation is written (1, 0, 0, 0): of all unit
quaternions that have one component equal to ±1, that component is the first in 21,080 of
22,853 list-A elements, 50 of 50 list-B, 39 of 40 list-C and 1,040 of 1,042 boxes (the rest
are genuine 180° turns). Rotations are therefore stored **w first**: (w, x, y, z).

---

<a id="s7"></a>
## 7. Assembling a static object (Confirmed for static setups)

1. Resolve the object to a setup (§3.1).
2. For each DID in the part list, in order, load the mesh (`mesh.md`) and draw its level 0
   **in the setup's own frame**: the part list holds no transforms, and each mesh is
   authored in object space (Z-up, origin at the object's foot point, `mesh.md` §11).
3. The object's placement in the world comes from outside the setup (a landblock entity
   frame, M3; or a bone attachment for held items, part 2).

Evidence: three multi-part static setups rendered with every part at identity show coherent
objects (a rock pile with a separate top part resting on it, a pillar with small separate
decorations around it, and a creature-like prop); the second part's bounding box lies exactly
where it belongs relative to the first.

Scale and the 0x47 `block` (§4.4) or 0x1F scale-like fields (§5.2) are OPEN; a viewer draws
unscaled.

---

<a id="s8"></a>
## 8. Worn-appearance record (prefix 0x20)

<a id="s8-1"></a>
### 8.1 Record layout (Confirmed, 4,201 of 4,201, zero trailing bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | `reserved` | u32 | 0 in every record |
| 3 | `section` | u8 | 0 (4,111), 1 (9), 2 (6), 3 (16), 4 (10), 5 (32), 6 (15). Meaning OPEN. |
| 4 | `entry_count` *n* | VLE | `records.md` §1.2 |
| 5 | entries | × *n* | §8.2 |
| 6 | `table_tag` | u8 | 1 in every record |
| 7 | `table_count` | VLE | Equals *n* in every record |
| 8 | table | × `table_count` | u32 key, u32 value (§8.4) |

<a id="s8-2"></a>
### 8.2 Entry (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `key` | u32 | Appearance key (0x1000xxxx). Unique within a record (no duplicates in 4,201 records). |
| 2 | `block_count` | u32 | 1 (46,215 entries), up to 53 |
| 3 | blocks | × `block_count` | §8.3 |

49,373 entries, 74,839 blocks in total.

<a id="s8-3"></a>
### 8.3 Block (Confirmed framing; field attribution partly OPEN)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `variant` | f32 | Block selector. 1.0 (45,126 blocks) is the base look; 0.00, 0.01, 0.02 … are dye variants (DS §7.5). Ascending within every entry. |
| 2 | `group_count` | u32 | |
| 3 | `group_param` | f32 | 0 (41,884), 1, 5, 3, 10 … Meaning OPEN. |
| 4 | groups | × `group_count`, 20 bytes each | u32 `shader` (0x2B DID or 0), u32 `material` (0x30 DID or 0), u32 `a`, u32 `b` (binding key matched against the parts' surface records, `material.md` §9.3), f32 `g` |
| 5 | `part_count` | u32 | |
| 6 | parts | × `part_count`, 12 bytes each | f32 `part_param`, u32 `slot` (0x1000xxxx), u32 `mesh` (0x06 DID) |
| 7 | `extra_count` | u32 | 0 in 74,837 blocks, 1 in 2 |
| 8 | extras | × `extra_count`, 12 bytes each | u32 `type` (9), u32 `slot`, u32 DID (0x04 setup) |
| 9 | `end` | u32 | 0 in every block |

Statistics: 84,261 groups (`material` is a 0x30 DID in 83,984 and 0 in 277; `shader` is a
0x2B DID in 22,151 and 0 in 62,110; the most common (`a`, `b`) pairs are (2, 0x10000050),
(2, 0x10000018), (2, 0x10000055)); 81,088 parts, every `mesh` an existing 0x06 record.
`part_param` is 1.0 (43,821), 10 (17,736), 15 (6,281), 4 (5,230) …; common slots are
0x1000000C, 0x1000000E, 0x1000000A, 0x10000003, 0x1000000B, 0x10000001.

**Difference from MV's grammar.** MV reads the part list as `u32 count, f32, count × (u32 slot,
u32 mesh, f32 lod)`, ending with the last part's float, and reports 11 unparsed records out of
4,192 in an older install. On this install that framing parses all but two records. Two
blocks in 0x200028ED and 0x20002900 settle it: the u32 after the parts is a count, followed by one (9, slot, 0x04 DID) element, and the
float belongs **before** each part's slot. A decoder must use the layout above.

> **OPEN:** Meaning of `part_param` (a level-of-detail or draw-priority value is likely), of
> `group_param`, `a` and `g`, and the split of floats between groups: like the part
> list, the per-group float might belong to the start of the next group rather than the end
> of the current one; the framing is the same either way.

<a id="s8-4"></a>
### 8.4 Key table (Confirmed layout, meaning OPEN)

Same keys as the entries (same order in 3,835 records, same set in the other 366). Values:
0 (45,088), 1, 2, 3, 4, other.

> **OPEN:** Meaning of the table values.

<a id="s8-5"></a>
### 8.5 Choosing parts for display (Reported, MV)

For a base look take the block with `variant` = 1.0 (or the only block). Draw every part of
that block; each part's mesh uses the material bound by the group whose slot matches (MV:
selection by family-specific slot keys; very small meshes are placeholder stubs). Dye blocks
replace materials only. The full material and dye rules belong to `material.md` and part 2.

> **OPEN:** (found by the M2 visual checklist, 2026-10-04, `docs/checklists/m2-visual.md`
> row C6.) Whether, and by what rule, the original skips one-triangle placeholder parts.
> Census of the install (clean-room decoder, counts only): 259 of 47,563 mesh records draw
> exactly one triangle at level 0, none of them degenerate (bound diagonals 0.02–0.55 m);
> 151 of them reference surface 0x310000E5 (a plain white diffuse, diffuse colour 2000 =
> (1, 1, 1, 0)), the rest 15 other surfaces. They appear as extra parts of creature setups
> (for example setup 0x04000110, parts 1 and 2: meshes 0x06003A8E and 0x060068B9, mesh
> `flags` 0x10000005 and 0x1000000A), and Tessera's viewer draws the larger ones as a white
> triangle near the feet. Candidate readings to check against the original (frame capture,
> `material.md` §19): attachment or reference markers that are never drawn (the low `flags`
> bits of `mesh.md` §3.3 might then be a bone or slot number), or drawn with a material
> that is invisible in practice (diffuse colour alpha 0). Until resolved, viewers draw every
> part.

---

<a id="s9"></a>
## 9. Invariants (Confirmed)

| ID | Invariant |
|---|---|
| **SI1** | 0x47: `did` equals the record DID; `template` prefix is 0x1F or 0x47; the embedded property set decodes with the dictionary and the record ends exactly after the optional 0x78 list. |
| **SI2** | 0x1F: flags ⊆ {0x001, 0x002, 0x020, 0x040, 0x100, 0x200, 0x400}; bit 0x001 set; every record has either a `parent` or a setup section, never both; setup section *n* = 1, key 1, type 9, count 1. |
| **SI3** | 0x04: tagfile magic at offset 8; frame-list headers repeat their count; `reserved` = 0; the record ends exactly after the part list. |
| **SI4** | Every part DID is an existing 0x06 record; every `collision_mesh` DID an existing 0x66 record. |
| **SI5** | 0x20: `reserved` = 0; `table_tag` = 1; `table_count` = `entry_count`; block `end` = 0; entry keys unique. |
| **SI6** | Chains terminate (no cycles; at most 4 0x47 hops and 1 0x1F hop observed). |

---

<a id="s10"></a>
## 10. Statistics and open questions

| Record type | Decoded / total | Trailing bytes |
|---|---|---|
| 0x47 entity descriptor | 40,643 / 40,643 | 0 |
| 0x1F visual template | 29,134 / 29,134 | 0 |
| 0x04 setup | 28,555 / 28,555 | 0 |
| 0x20 worn appearance | 4,201 / 4,201 | 0 |

Failure categories a decoder should distinguish: truncated, unsupported flag bits or section
types, count mismatch (frame-list header, key table), dangling reference (resolver only),
trailing bytes. None occur in this install.

> **OPEN:** 0x47 `kind`, `block`, the 63 missing 0x78 sets (§4.3, §4.4).

> **OPEN:** 0x1F scale-like fields and their order (§5.2); the appearance section's three
> constants (§5.4).

> **OPEN:** 0x04 `attach_map`, frame lists B and C (§6.2, §6.3); the 0x66 collision-mesh
> layout (§6.5); the Havok tagfile contents (part 2).

> **OPEN:** 0x20 `section`, group fields, `part_param`, key-table values (§8).

> **OPEN:** One-triangle placeholder parts: skipped or drawn by the original (§8.5).

> **OPEN:** Who references 0x01 collision geometry (§2.2).

<a id="s10-1"></a>
### 10.1 Implementation feedback (clean room, 2026-10-04)

`tessera-formats` (`setup`, `appearance` modules) decodes all four record types on the same
install with the counts of §10 (0 failures, 0 trailing bytes, SI1–SI5 hold), resolves all
40,643 0x47 and 29,134 0x1F chains (2 + 2 end at the part-less setups), re-encodes every
record of the four types byte for byte, and reproduces the §6.4 bounds check (2,625 sphere-
or capsule-only setups, 0 differ). Points for the spec owner:

> **OPEN:** Box bounds (§6.4, §6.5, §6.7). Rotating a box's eight local corners by its frame
> rotation as the active rotation `v' = q v q*`, with `q` read as (w, x, y, z), then adding
> the frame position, gives the stored `physics_bounds` AABB for **all 990** box-only setups
> (825 with unrotated boxes, 165 with rotated ones; within 2e-3). This confirms the (w, x, y,
> z) order of §6.7 directly, not only statistically, and fixes the rotation's direction.
> The §6.4 counts ("865 of 874 unrotated match, 9 differ; 108 rotated") do not reproduce:
> the implementation finds 0 differing unrotated box-only setups. Please recount and close
> the "9 differ" OPEN. The horizontal `radius` of all 1,910 sphere-only setups equals the
> largest `hypot(cx, cy) + r` (§6.5). No observed box has a scale other than (1, 1, 1), so
> how a frame's scale combines with its rotation stays open; `CollisionBox::aabb` refuses
> scaled boxes instead of guessing.

> **OPEN:** SI6 says "at most 4 0x47 hops and 1 0x1F hop". Counting every link (entity →
> entity, entity → template, template → template, template → setup), real 0x47 chains are
> 2–6 links long (2: 38,588; 3: 2,029; 4: 12; 5: 6; 6: 8) and 0x1F chains 1–2. Suggest
> stating the bound in those terms.

> **OPEN:** §10's failure categories have no entry for "the record could not be read"
> (I/O, inflate or block corruption) as distinct from "dangling reference" (the record does
> not exist). The implementation's resolver keeps them apart (`ResolveError::Fetch` vs
> `DanglingReference`); please add the category.

> **OPEN:** §4.3 lists `kind` values summing to 40,642. The implementation's census also
> finds `kind` = 32 in one record.

> **OPEN:** §8.2/§8.3 give 49,373 entries and 81,088 parts; the implementation counts 49,383
> entries and 81,098 parts (blocks agree at 74,839, groups at 84,261). Either a counting
> difference or a data update; please recount.

> **OPEN:** 3,102 entries have several blocks and none with `variant` = 1.0, so §8.5's base
> look ("the block with variant 1.0, or the only block") is undefined for them.
> `AppearanceEntry::base_block` returns nothing there; which block a viewer should draw is
> open.

> **OPEN:** Constants the implementation treats as layout-relevant and rejects as
> unsupported when they differ (conservative, since a different value might be a count or
> a tag): the 0x47 flag bits 0x8B being set (§4.2), the 0x1F appearance-section words
> (33, 2, 1) (§5.4), the 0x04 `reserved` byte (§6.1), and in 0x20 the `reserved` word, the
> `table_tag`, the block `end` word and the extra `type` 9 (§8). Please confirm or say
> which of them are plain values. Values that do not affect framing (tagfile magic, key
> echoes in frame lists, link prefixes, key-table key set) are reported as "unusual"
> instead and still decode.

<a id="s10-2"></a>
### 10.2 Implementation feedback (explorer mesh viewer, clean room, 2026-10-04)

The explorer's 3D view resolves 0x47, 0x1F, 0x04 and 0x20 records and objects (0x70–0x7F)
to scenes with this spec. On samples of the owner's install (every 211th setup, 293rd
template, 401st entity, 41st appearance record and 3,001st object property set) every
setup, template and entity chain resolved to drawable parts. Points for the spec owner:

> **OPEN:** Objects name geometry in more than one way. The viewer offers every top-level
> property whose value is a 0x47, 0x1F or 0x04 DID (`PhysObj` and the alternatives of
> §3.3) plus every `Item_WornAppearanceMapList` element as a choice. Which one the client
> shows for an item lying on the ground, held, or worn is not stated; the viewer shows the
> first (property order). 61 of 128 sampled 0x79 sets have neither (no geometry).

> **OPEN:** A setup's parts are drawn with identity transforms (§7), and the 0x1F
> scale-like fields (§5.2) and the 0x47 `block` (§4.4) are ignored. Multi-part skinned
> setups (bodies split into head, torso, …) then show in bind pose, which looks right; for
> static multi-part setups the viewer adds an "explode" slider of its own to separate
> parts, which is not a format fact.

> **OPEN:** Appearance entries whose first entry is material-only: 63 of 103 sampled
> 0x20 records have no part meshes in entry 0 (the viewer lists every entry as a choice).
> When a record mixes geometry entries and material-only entries, which material-only
> entry applies to which geometry entry is not stated (see `material.md` §19).

