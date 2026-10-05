<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setups and part assembly: static objects, characters, skeletons and attachments

| | |
|---|---|
| **Status** | Review. Complete (parts 1 and 2) apart from the OPENs listed in §10 and §14 |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` (entity descriptor 0x47, visual template 0x1F, setup 0x04, appearance 0x20 decoders), `crates/mesh` / `crates/assets` (resolving an object to meshes), `crates/anim` (attach tables, §11–§13), `apps/explorer`, `apps/client` |
| **Companion specs** | `mesh.md` (the 0x06 parts). This spec holds the full 0x20 grammar (§8) that `material.md` §9.3 refers to as a future appearance spec. `property-system.md` §5 (property sets embedded in 0x47; item property sets 0x79), §8 (DID maps). `records.md` §1 (VLE, TSize). `material.md` (0x30 materials, 0x2B shaders named by appearance records). `havok-data.md` (the skeleton tagfile of field 3, §6.1). `skeleton-anim.md` (bone mapping, skinning, conventions, attach-point semantics; part 2 here refers to it). `appearance.md` (M4: species and body assembly, worn-part replacement rules, bone sliders). |
| **Part 2 (M4)** | §11 skeleton inside setups, §12 character setups versus static ones, §13 held items on bones, §14 part 2 invariants and open questions. Part 2 also settles §6.2 and §6.3. |

---

<a id="s0"></a>
## 0. Purpose

A mesh record (0x06) is one part. Whole objects (a barrel, a tree, a weapon, an NPC) are
assembled by a short chain of records. Part 1 of this spec covers:

- which DID prefixes take part and what each one is (§2);
- the chains from an item or object to its meshes, with resolution statistics (§3);
- the **entity descriptor** (0x47), **visual template** (0x1F) and **setup** (0x04) record
  layouts, decoded byte-exactly over all records (§4–§6);
- how a static object's parts are placed (§7);
- the worn-appearance record (0x20), the second path from items to meshes (§8);
- invariants and open questions (§9–§10).

Part 2 (M4) adds:

- the skeleton inside a setup, by reference to `havok-data.md` (§11);
- the meaning of the attach map and frame lists (§6.2, §6.3) and how character setups differ
  from static ones (§12);
- how held items attach to bones (§13);
- part 2 invariants, statistics and open questions (§14).

Animation semantics (track binding, skinning, sampling, root motion) are in
`skeleton-anim.md`.

---

<a id="s1"></a>
## 1. Provenance

Legend as in `mesh.md` §1. Confirmed facts were decoded on the user's install on 2026-10-04
by throwaway probes in the dirty-room scratchpad.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04. **All** 40,643 records 0x47 (gamelogic), 29,134 records 0x1F, 28,555 records 0x04 and 4,201 records 0x20 (general) decoded to the last byte. All 383,332 item property sets (0x79) scanned for references to these prefixes, using the repository's own property-set decoder. Every chain resolved to its mesh records. Three multi-part setups rendered and inspected. |
| OBS2 | Part 2, observed 2026-10-05: every setup's skeleton, attach map and frame lists decoded and checked against the skeleton; attach keys matched against all 735 enum mappers; the race table, player-avatar objects, inventory-control object and the holding-location properties of all item property sets read; every skinned submesh of all 47,563 meshes read (`skeleton-anim.md` §1). |
| SA | `skeleton-anim.md` (part 2 relies on its §2, §4, §5, §8 and §9). |
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
| S9 | The setup's tagfile is the object's only skeleton; nothing shares a skeleton by reference (§11) | OBS2, SA §2 | Confirmed |
| S10 | `attach_map` = `BoneType` enum key → bone index (§6.2) | OBS2 (378 setups) | Confirmed (part 2) |
| S11 | Frame list A = `HoldingLocation` key → (bone or −1, bone-local frame); list C keyed by `UIViewport_Type` (§6.3) | OBS2 (2,737 and 50 setups) | Confirmed (part 2); list B meaning OPEN |
| S12 | Parts of a skinned setup are drawn with identity part transforms and skinned to the setup's skeleton; part meshes carry their `MeshTypeID` slot in their `flags` (§12) | OBS2, SA §4, §9 | Confirmed |
| S13 | Held items: the item's setup origin is placed at a holding-location frame of the wearer's list A (§13) | MV (bones); OBS2 | Confirmed for the geometry; location choice partly OPEN |

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

7 (40,229 records), 3 (240), 2 (141), 9 (18), 23 (8), 1 (2), 5, 19, 21, 31, 32 (1 each;
32 recounted 2026-10-05 after implementation feedback, §10.1). It looks like a bit set.

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

Part 2 evidence (2026-10-05), the player-avatar templates of §12.3: `vec3` = (1.03, 1.03,
1.03) on the Elf male and High-elf female templates and (1.05, 1.05, 1.05) on the High-elf
male template, whose setups carry the same 89-bone skeleton and human-height bodies as the
Human ones (a uniform model scale fits); `value_a` = 0.9 on the Human, Elf, High-elf and
River-hobbit female templates and the Dwarf template, 0.65 on both Hobbit templates;
`value_b` = 0.85 on both Hobbit templates only. The Hobbit setups already have small
skeletons and bodies (1.16 m), so `value_a` and `value_b` are not plain geometry scales.

> **OPEN:** Meaning of `vec3`, `value_a` and `value_b` (`vec3` as a uniform model scale is
> likely; owner observation: an Elf male stands about 3 % taller than a Human male). Their
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
| 3 | `tagfile` | *t* bytes | Havok binary tagfile; begins with the 8-byte magic `1E 0D B0 CA CE FA 11 D0` in every record. Holds the object's skeleton (§11; decoded by `havok-data.md` §3–§4.2). Common lengths 358 (16,376 records), 402, 351, 349. |
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
### 6.2 `attach_map` (Confirmed layout and meaning)

TSize map, element = u32 key, u32 value. Empty in 28,177 setups. Keys are enum values
0x1000xxxx (5,812 elements) or small integers (559); values are below 256.

**Meaning (part 2, Confirmed on all 378 non-empty maps):** `key` is a value of the enum
mapper labelled **`BoneType`** in the EMAPPER DID map (keys 0x1000xxxx; the small keys 1 =
`physics` and 3 = `Root` come from its base mapper), and `value` is a **bone index** into the
setup's skeleton (§11). Keys are unique and values are unique within every map, and every
value is below the skeleton's bone count. It is the semantic bone table that effect and
script properties use to find "the head", "the right weapon holder", a tail or a face-slider
bone on any skeleton (`skeleton-anim.md` §8.1, label list and statistics).

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

Observed: list A in 2,737 setups (25,118 elements; `key2` = `key` always; `bone` ≥ −1;
scale (1, 1, 1) in 24,190 elements); list B in 42 setups (50 elements, `id` = 0 in 48 and
0x10000001 in 2); list C in 50 setups (54 elements, `key2` = `key`). No setup has more than
one of the three lists.

**Meaning (part 2, Confirmed):**

- **List A is the holding-location table.** `key` is a value of the enum mapper labelled
  **`HoldingLocation`** (all 115 keys in use resolve, with the small keys 1 `RHand`, 2 `LHand`,
  3 `LOffHand`, 4, 6, 17, 18, 19 from its base mapper); keys are unique per setup. `bone` is a
  bone index of the setup's skeleton (below the bone count in all 2,737 setups) or −1, meaning
  the frame is relative to the setup origin (2,338 setups, almost all one-bone props and
  weapons). The frame is expressed in the bone's frame: location = bone model transform ·
  translate(position) · rotate(rotation) · scale(scale) (`skeleton-anim.md` §5.3, §8.2). It
  holds every hand, off-hand, forearm, sheath, mount, target, effect and weapon-trail point
  (`skeleton-anim.md` §8.2–§8.6).
- **List C** is keyed by `UIViewport_Type` values (the enum mapper that the property
  `UIViewport_Type` names; it has no EMAPPER label; labels `Paperdoll`, `Opponent`, `Self`,
  `AppearanceMod`, `MountedPaperdoll`, `EmotesPaperdoll`; observed `Opponent` 31,
  `MountedPaperdoll` 19, `EmotesPaperdoll` 3, `AppearanceMod` 1). They look like object
  offsets for the UI's 3D viewports; world rendering ignores them.
- **List B**: unscaled, unrotated positions on large single-part props, such as (0, 0, 2),
  (0, 0, 6.1), (0, −6.7, 0).

> **OPEN:** What list B places (a seat, a spawn or an effect point are candidates). World
> rendering ignores it.

<a id="s6-4"></a>
### 6.4 Collision volumes (Confirmed)

| Volume | Size | Layout | Setups with any |
|---|---|---|---|
| Box | 64 | local min corner f32 × 3, local max corner f32 × 3, then a frame (§6.3: position, scale, rotation) | 997 |
| Sphere | 16 | centre f32 × 3, radius f32 | 1,911 |
| Capsule | 20 | centre of the lower cap f32 × 3, length between cap centres f32, radius f32 | 719 |

Semantics were confirmed against `physics_bounds` (§6.5): for all 1,910 sphere-only and all
715 capsule-only setups the bounds equal the union of the volumes computed this way (a
capsule spans z from centre − r to centre + length + r). For all 990 box-only setups, the
eight local corners rotated by the box frame's rotation (active rotation, (w, x, y, z) order,
§6.7) and moved by its position give the bounds within 2 × 10⁻³ (recounted 2026-10-05: 864
setups whose boxes are all unrotated and 126 with a rotated box; the earlier "9 differ" was a
counting error, §10.1). No box has a scale other than (1, 1, 1).

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

Confirmed directly as well (2026-10-05): with this order and an active rotation, box frames
reproduce the physics bounds of all 990 box-only setups (§6.4), and the sheath frames of list A
hang weapons blade-down (`skeleton-anim.md` §5.2, §8.3). Note that the skeleton's own
quaternions (inside the tagfile) are stored in the other order, (x, y, z, w)
(`havok-data.md` §4.2).

---

<a id="s7"></a>
## 7. Assembling an object (Confirmed)

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

The same holds for setups with skinned parts (bodies split into head, torso, legs …): every
part is drawn in the setup's frame with an identity part transform, and its skinned submeshes
are skinned against the setup's skeleton (§12). At the skeleton's reference pose skinning is
the identity, so the bind-pose display with identity transforms is exact (`skeleton-anim.md`
§4.4, §9.1). An unskinned part never follows a bone: no setup mixes skinned and unskinned
parts (§12.1).

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

49,383 entries, 74,839 blocks in total (entry count recounted 2026-10-05, §10.1).

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
(2, 0x10000018), (2, 0x10000055)); 81,098 parts (recounted 2026-10-05), every `mesh` an
existing 0x06 record.
`part_param` is 1.0 (43,821), 10 (17,736), 15 (6,281), 4 (5,230) …; common slots are
0x1000000C, 0x1000000E, 0x1000000A, 0x10000003, 0x1000000B, 0x10000001.

**`slot` is a `MeshTypeID` value** (part 2, Confirmed): the enum mapper labelled `MeshTypeID`
labels every slot value in use (`UpperBody` 0x1000000C, `Crest` 0x1000000E, `Helmet`
0x1000000A, `LowerBody` 0x10000003, `Cape` 0x1000000B, `Hands` 0x10000001, `Feet`, `Shoulder`,
`Ears`, `Head`, `Beard`, `Satchel`, `Accessory1`, mount, war-machine and fence slots, and 1
`Default`, 2 `All` from its base mapper). The part mesh's own `flags` word equals its slot for
80,974 of the 81,098 parts (§12.2).

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
>
> Part 2 evidence (2026-10-05): the mesh `flags` low bits are not a bone number but the
> part's `MeshTypeID` slot (§12.2). 242 three-vertex skinned stub meshes carry body or
> equipment slots (`Accessory1` 32, `Crest` 28, `Shoulder`, `Head`, `UpperBody`, `Helmet` 17
> each, `Cape` 16 …); the two examples above are `Shoulder` and `Helmet`. That fits **slot
> placeholders** (a slot that is present but empty until a worn item fills it, like the
> "none" style of MV's character-creation records) better than visible geometry. Whether the
> original draws them stays an owner observation; `appearance.md` owns the slot rules.

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
| **SI6** | Chains terminate (no cycles). Counting every link (entity → entity, entity → template, template → template, template → setup), 0x47 chains are 2 – 6 links long (2: 38,588; 3: 2,029; 4: 12; 5: 6; 6: 8) and 0x1F chains 1 – 2. |

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
types, count mismatch (frame-list header, key table), dangling reference (the referenced
record does not exist; resolver only), unreadable record (the record exists but could not be
read: I/O, inflate or block corruption; resolver only), trailing bytes. None occur in this
install.

> **OPEN:** 0x47 `kind`, `block`, the 63 missing 0x78 sets (§4.3, §4.4).

> **OPEN:** 0x1F scale-like fields and their order (§5.2); the appearance section's three
> constants (§5.4).

> **OPEN:** 0x04 frame list B (§6.3); the 0x66 collision-mesh layout (§6.5). (`attach_map`,
> lists A and C and the tagfile contents are resolved by part 2: §6.2, §6.3, §11.)

> **OPEN:** 0x20 `section`, group fields, `part_param`, key-table values (§8).

> **OPEN** (owner observation): one-triangle placeholder parts: skipped or drawn by the
> original (§8.5; part 2 evidence says slot placeholders).

> **OPEN:** Who references 0x01 collision geometry (§2.2).

<a id="s10-1"></a>
### 10.1 Implementation feedback (clean room, 2026-10-04)

`tessera-formats` (`setup`, `appearance` modules) decodes all four record types on the same
install with the counts of §10 (0 failures, 0 trailing bytes, SI1–SI5 hold), resolves all
40,643 0x47 and 29,134 0x1F chains (2 + 2 end at the part-less setups), re-encodes every
record of the four types byte for byte, and reproduces the §6.4 bounds check (2,625 sphere-
or capsule-only setups, 0 differ). Points for the spec owner:

> **Resolved (2026-10-05, spec owner):** recount agrees: all 990 box-only setups match (§6.4
> updated); the "9 differ" was a counting error.
> Original note: Box bounds (§6.4, §6.5, §6.7). Rotating a box's eight local corners by its frame
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

> **Resolved (2026-10-05):** SI6 now states the bound in links.
> Original note: SI6 says "at most 4 0x47 hops and 1 0x1F hop". Counting every link (entity →
> entity, entity → template, template → template, template → setup), real 0x47 chains are
> 2–6 links long (2: 38,588; 3: 2,029; 4: 12; 5: 6; 6: 8) and 0x1F chains 1–2. Suggest
> stating the bound in those terms.

> **Resolved (2026-10-05):** "unreadable record" added to §10.
> Original note: §10's failure categories have no entry for "the record could not be read"
> (I/O, inflate or block corruption) as distinct from "dangling reference" (the record does
> not exist). The implementation's resolver keeps them apart (`ResolveError::Fetch` vs
> `DanglingReference`); please add the category.

> **Resolved (2026-10-05):** `kind` = 32 (1 record) added to §4.3.
> Original note: §4.3 lists `kind` values summing to 40,642. The implementation's census also
> finds `kind` = 32 in one record.

> **Resolved (2026-10-05):** recount gives 49,383 entries and 81,098 parts, as the implementation
> found; §8.2 and §8.3 are corrected.
> Original note: §8.2/§8.3 give 49,373 entries and 81,088 parts; the implementation counts 49,383
> entries and 81,098 parts (blocks agree at 74,839, groups at 84,261). Either a counting
> difference or a data update; please recount.

> **OPEN** (owned by `appearance.md`): 3,102 entries have several blocks and none with `variant` = 1.0, so §8.5's base
> look ("the block with variant 1.0, or the only block") is undefined for them.
> `AppearanceEntry::base_block` returns nothing there; which block a viewer should draw is
> open.

> **OPEN** (answered in part, 2026-10-05): the data cannot tell a plain value from a tag or
> count when only one value ever occurs, so the conservative rejection stays. The tagfile
> magic is a format constant (`havok-data.md` §3.1).
> Original note: Constants the implementation treats as layout-relevant and rejects as
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

> **OPEN** (answered in part, 2026-10-05): held items use `PhysObj` unless the item has a worn
> appearance for the wearer, in which case it is worn (§13.3); the ground and display cases
> stay open.
> Original note: Objects name geometry in more than one way. The viewer offers every top-level
> property whose value is a 0x47, 0x1F or 0x04 DID (`PhysObj` and the alternatives of
> §3.3) plus every `Item_WornAppearanceMapList` element as a choice. Which one the client
> shows for an item lying on the ground, held, or worn is not stated; the viewer shows the
> first (property order). 61 of 128 sampled 0x79 sets have neither (no geometry).

> **Resolved (2026-10-05, part 2):** identity part transforms are exact for static and skinned
> setups (§7, §12.1; `skeleton-anim.md` §9.1). The 0x1F and 0x47 scale fields stay open
> (§5.2, §4.4).
> Original note: A setup's parts are drawn with identity transforms (§7), and the 0x1F
> scale-like fields (§5.2) and the 0x47 `block` (§4.4) are ignored. Multi-part skinned
> setups (bodies split into head, torso, …) then show in bind pose, which looks right; for
> static multi-part setups the viewer adds an "explode" slider of its own to separate
> parts, which is not a format fact.

> **OPEN** (owned by `appearance.md`): Appearance entries whose first entry is material-only: 63 of 103 sampled
> 0x20 records have no part meshes in entry 0 (the viewer lists every entry as a choice).
> When a record mixes geometry entries and material-only entries, which material-only
> entry applies to which geometry entry is not stated (see `material.md` §19).

---

<a id="s11"></a>
## 11. Part 2: the skeleton inside a setup

<a id="s11-1"></a>
### 11.1 Decoding (Confirmed, by reference)

Field 3 of every setup (§6.1) is a Havok binary tagfile holding exactly one `hkaSkeleton`:
parent indices, bones (optional names, optional translation-lock flags) and a reference pose
of translation, rotation (x, y, z, w) and scale per bone. Its grammar, members, defaults,
invariants and failure categories are normative in `havok-data.md` §3–§4.2 and §9.1; this
spec does not repeat them. All 28,555 skeletons decode (`havok-data.md` §8.2).

Everything else in the setup that names a bone uses an index into this skeleton's bone
array: `attach_map` values (§6.2), list A `bone` (§6.3), and the vertex bone indices of
skinned parts (`mesh.md` §5.7; `skeleton-anim.md` §4.1).

<a id="s11-2"></a>
### 11.2 One skeleton per setup (Confirmed)

The tagfile is the object's only skeleton. No setup, template, entity or clip names another
setup's skeleton, and a skeleton has no name of its own (`skeleton-anim.md` §2.1). Equal
skeletons are stored again in every setup that uses them: 923 distinct skeletons among the
28,555 setups (191 distinct among the 293 with 30 or more bones). A loader may share decoded
skeletons by content.

<a id="s11-3"></a>
### 11.3 Skeletons of static setups (Confirmed)

| Bones | Setups | Reference pose all identity (zero translation, identity rotation, unit scale) |
|---|---|---|
| 1 | 22,434 | 22,185 |
| 2 | 5,344 | 5,021 |

A static setup's skeleton is not needed to draw it (its parts are unskinned, §12.1), but it
still anchors list A frames with a non-negative `bone` and fits clips with 1 or 2 tracks (55
and 346 clips). A viewer that only draws static objects may skip decoding the tagfile; it
must still skip its bytes using `tagfile_length`.

---

<a id="s12"></a>
## 12. Part 2: character setups and static setups

<a id="s12-1"></a>
### 12.1 What differs (Confirmed, all 28,555 setups)

| Bones | Setups | All parts skinned | No part skinned | No parts |
|---|---|---|---|---|
| 1 | 22,434 | 19 | 22,413 | 2 |
| 2 | 5,344 | 161 | 5,183 | 0 |
| 3 – 29 | 484 | 480 | 4 | 0 |
| 30 – 248 | 293 | 293 | 0 | 0 |

- **Static setups** (almost all one- and two-bone setups): unskinned parts, trivial skeleton,
  usually no attach map; drawn as §7 describes.
- **Skinned setups** (characters, creatures, animated props; 953 setups): every part is
  skinned against the setup's skeleton; parts are drawn with identity part transforms in the
  setup frame (§7) and deformed by the palette of `skeleton-anim.md` §4.3. They usually have
  an `attach_map` (229 of the 293 with 30 or more bones) and a list A of holding locations.
- No setup mixes skinned and unskinned parts. A part that must follow a bone is skinned, if
  need be with one influence per vertex.

<a id="s12-2"></a>
### 12.2 Parts carry their slot (Confirmed)

Each part mesh's `flags` word (`mesh.md` §3.3) is a value of the enum mapper labelled
`MeshTypeID`: 1 `Default` on the 27,960 static meshes, 2 `All` on 20, and a slot value
0x1000_00nn on all 19,583 others (`Helmet` 4,014, `UpperBody` 2,960, `Shoulder` 2,603,
`Crest` 1,503, `Hands` 1,191, `Feet` 1,113 …; `skeleton-anim.md` §9.2). In worn-appearance
records (§8.3) the part's `slot` equals its mesh's `flags` for 80,974 of 81,098 parts.

A character setup's part list is its **default body**: one part per slot. The 89-bone male
avatar setups, for example, list one mesh each for `UpperBody`, `LowerBody`, `Hands`, `Feet`,
`Head`, `Ears`, `Crest`, `Beard`, `Shoulder`, `Helmet`, `Cape` and `Satchel` (several of them
three-vertex stubs, §8.5). A worn item's parts replace the body part of the same slot; the
rules for which slots an item hides or keeps belong to `appearance.md`.

<a id="s12-3"></a>
### 12.3 Playable characters (Confirmed)

A playable species and sex reaches its setup through the race table: DID-map label
`RaceTableDirectory` → species objects → `RaceTable_GenderList` → `RaceTable_Gender_PlayerAvatar`
→ `PhysObj` → 0x47 → 0x1F → 0x04 (`skeleton-anim.md` §2.2, with the full table). Observed:

| Setups | Skeleton | Used by |
|---|---|---|
| 0x04000551, 0x04000552 | 62 bones each, own skeletons | Hobbit male, female |
| 0x04000557 | 62 bones | Dwarf male; Stout-axe dwarf (both entries) |
| 0x04000553, 0x04000555, 0x04005EC4, 0x0400772B | one 89-bone male skeleton | Human, Elf, High-elf, River-hobbit males; Beorning male uses 0x04000553 |
| 0x04000554, 0x04000556, 0x04005EC5, 0x0400772A | one 89-bone female skeleton | the same species' females; Beorning female uses 0x04000554 |

These setups carry the full set of holding locations (35 to 39 list A elements) and an
`attach_map` of 25 (hobbit), 29 (dwarf) or 46 (89-bone) bones, including 21 face-slider bones on the
89-bone ones. NPCs and creatures reach their setups through ordinary entity chains (§3.1).

---

<a id="s13"></a>
## 13. Part 2: held items on bones

<a id="s13-1"></a>
### 13.1 Rule (Confirmed geometry)

1. The held item resolves to its own setup through `PhysObj` (§3.1).
2. A **holding location** is chosen (§13.2) and looked up in the **wearer's** list A (§6.3):
   it gives a bone of the wearer's skeleton (or −1 for the wearer's origin) and a frame in
   that bone's space.
3. The item's setup origin is placed at that frame, following the bone through the animation:
   item model transform = wearer bone model transform · frame transform; world placement =
   wearer placement · item model transform. The item's parts are then drawn as §7 says, in its
   own frame.

Weapons are modelled with the grip at their origin and the long axis along +Y, so in-hand
locations are identity frames on the weapon-holder bones and the item needs no offset of its
own (`skeleton-anim.md` §8.3–§8.4, with the per-location table).

<a id="s13-2"></a>
### 13.2 Which location (Confirmed sources, rule partly OPEN)

The data gives a paperdoll location per equipment slot (primary weapon `Wpn_RHand`, secondary
`Wpn_LOffHand`, ranged `Sheath_Back_Bow`, from the inventory-control object), per-item
overrides (`Inventory_ParentingLocation`: shields `Wpn_Forearm`, crossbows, javelins and some
bows `Wpn_ROffhand`; `Inventory_PaperdollParentingLocation`) and explicit locations for
cosmetic props. The full table and a viewer rule are in `skeleton-anim.md` §8.5.

> **OPEN** (for `script-table.md`, and owner observation): the in-world default location per
> equipment slot, and when and where weapons are stowed (the sheath locations of §12.3).

<a id="s13-3"></a>
### 13.3 Worn versus held (Reported, MV)

Some class items (satchels, instruments) carry an `Item_WornAppearanceMapList` as well as a
`PhysObj`; for them the worn appearance (§3.2) is what the wearer shows, and the `PhysObj`
chain leads to a generic ground model. Rule: an item with a worn-appearance entry for the
wearer's species and sex is worn; otherwise it is held.

---

<a id="s14"></a>
## 14. Part 2: invariants, statistics and open questions

| ID | Invariant (Confirmed over all records) |
|---|---|
| **SI7** | `attach_map`: keys unique; values unique; every value < the skeleton's bone count. |
| **SI8** | List A: `key2` = `key`; keys unique per setup; `bone` = −1 or < bone count. List C: `key2` = `key`. |
| **SI9** | No setup has both skinned and unskinned parts. |
| **SI10** | Every skinned part's largest vertex bone index < the setup's bone count. |
| **SI11** | Every part mesh's `flags` is a `MeshTypeID` value. |

| Statistic | Value |
|---|---|
| Setups with `attach_map` / list A / list B / list C | 378 / 2,737 / 42 / 50 |
| `attach_map` keys in use / list A keys in use | 72 (`BoneType`) / 115 (`HoldingLocation`) |
| Setups with skinned parts | 953 |
| Distinct skeletons | 923 (191 with 30 or more bones) |

A decoder reports SI7, SI8 and SI10 violations as data-quality issues; none occur.

> **OPEN:** frame list B (§6.3).

> **OPEN** (for `script-table.md`, and owner observation): holding-location choice in the
> world, and stowing (§13.2).

> **OPEN** (owner observation): one-triangle slot placeholder parts (§8.5).

> **OPEN:** the 0x1F scale-like fields (§5.2), with the part 2 evidence there.


<a id="s14-1"></a>
### 14.1 Implementation feedback, part 2 (clean room, 2026-10-05)

`tessera-formats` exposes the part-2 meanings as typed views over the unchanged decoders:
`Setup::bone_for` and `Setup::attachments` (§6.2), `Setup::holding_location` with an
`Anchor` of origin (−1) or bone (§6.3 list A), `Setup::viewport_frame` (list C), and
`TemplateAppearance::elements` (§5.4 as a one-record appearance list). Its real-data test
reproduces the §14 statistics (378 attach maps, list A in 2,737 setups, list B in 42, list C
in 50) and finds the skeleton-independent parts of SI7 and SI8 (unique keys and values, list
A bones ≥ −1) in every setup; the largest bone index any setup names is 200.

> **OPEN:** SI7, SI8 and SI10 compare indices with the skeleton's bone count, which lives in
> the tagfile (`havok-data.md`). The formats crate keeps the tagfile opaque, so it checks
> them only when a caller passes the bone count (`Setup::bone_issue`); the full check
> belongs to a test that also decodes skeletons.
