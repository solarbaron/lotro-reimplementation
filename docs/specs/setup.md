<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setups and part assembly: static objects, characters, skeletons and attachments

| | |
|---|---|
| **Status** | Accepted (revision of 2026-10-08 reviewed and passed, 2026-10-08) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-reviewer, a different session from the writer) |
| **Review record** | Review of the 2026-10-08 revision, 2026-10-08: Claude review session (spec-reviewer, a different session from the writer): **passed**, Accepted (reviewer added OPEN callouts in §5.5 and §8.3 for unknowns already named in the text; no facts changed). Earlier: re-review of 2026-10-07, **passed**. Full records in the review log (decision 0018). |
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
| APP, COL, MESH | `appearance.md` (§2.1, §4.1–§4.11, §5.1, §6.1, §7.4: slot priorities, block fields, key table, worn versus held, template scale, template appearance sections); `collision.md` (§4 the 0x66 layout, §5 the 0x01 cell geometry, §15 corrections to this spec); `mesh.md` §8.5–§8.10 (LOD classes, one-triangle meshes). Same install. |
| BIN | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)).** The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035), disassembled in the dirty room for facts only on 2026-10-05. **Question:** does the client draw the one-triangle placeholder parts of figures (owner item O4)? **Why M1–M3 were not enough:** the parts are ordinary mesh records with no marker; whether something is left out is decided by code; no published source describes it; a frame capture (M3) of the original was not available to this session. Only the mesh loader and the part list of drawn objects were read (with `mesh.md` BIN). **Second session, same build, 2026-10-07** (Ghidra, read-only copy of the project, by address): **questions** (2) how the client reads the 0x47 flag bits and fields, and what `kind` and `block` are; (3) how the 0x47 `extra_sets` combine with the embedded property set, and what happens when one is missing; (4) the stream order and defaults of the 0x1F `vec3`, `value_a` and `value_b`; (5) what frame list B is. **Why M1–M3 were not enough:** only one bit pattern and one order occur in the data, `block` occurs in 25 records with values that fit both an offset and a scale, 63 extra sets do not resolve, and the meaning of a field is not in the bytes; no published source states these; no running client was available. Code read: the entity-descriptor reader, reset routine, property lookup, property-merge routine and debug description; the visual-template reader, its flag computation for writing and its copy into the model component. List B's reader was not found. Nothing of the network, login, launcher or anti-tamper code. **Third session, same build, 2026-10-07** (review changes; Ghidra, read-only copy of the project, by address): **questions** (6) how the routine that gathers a descriptor's whole inherited property set treats each `kind` and each kind of `template` target, and how it differs from the single-property lookup of question 2; (7) for owner item O8, which inputs the render component combines into an entity's scale. **Why M1–M3 were not enough:** the data hold only well-formed chains (every chain ends at a 0x1F) and one `kind` 32 record, so the data cannot show how other targets or kinds are treated; the scale product exists only in code; no running client was available. Code read: the descriptor's whole-set gathering routine and its entry point, the property-set merge, the single-property lookup (again, for comparison), and the render component's scale routine. Nothing of the network, login, launcher or anti-tamper code. **Fourth session, same build, 2026-10-08** (Ghidra, read-only copy of the project, by address): **questions** (8) where the 0x1F `vec3`, `value_a` and `value_b` go after a template is copied into the model component, and which code reads them (owner item O8); (9) how the client reads a 0x04 record: frame list B, the `reserved` byte, the collision-volume and part counts, and boxes; (10) how it reads a 0x20 record: the `reserved` word, the `section` byte, the key table and the block body; (11) what the 0x1F appearance and setup sections are (the constant words 33, 2, 1 and 1, 9, 1); (12) what sets the two switches of the render scale's third input. **Why M1–M3 were not enough:** the data hold one value per constant and unlabelled scalars; what a value means, and whether a constant is a count, a flag word or a tag, is decided by code; no published source states it; no running client was available. Code read: the model component's set-up from a template and its getters, the template reader, its chain merge and its appearance-info reader, the property lookup of an entity and its provider registry, three property providers of the model component, the effects component's update of attached effects, the setup reader and the client setup's part-list reader, the appearance-record reader and its block reader, the stream's count reader, the routine that attaches a spawned object and sets the two switches, and the descriptor's debug description (for the `block` frame, §4.4). Nothing of the network, login, launcher or anti-tamper code. |
| OBS3 | Observed in user install 2026-10-08 by throwaway probes: all 20,474 `extra_sets` DIDs of the 0x47 records and their archives; the `section` byte and entry order of all 4,201 0x20 records; all 1,205 boxes and every volume and part count of the 28,555 setups; the list B headers of all 42 setups that have one; the `vec3`, `value_a` and `value_b` values of all 29,134 templates; the definitions of PIDs 0x112, 0x3C3 and 0x5A0 in the property dictionary. |
| CNS | Census of 2026-10-05 by throwaway probes (repository decoders used as tools): stub parts of all 28,555 setups; surfaces and materials of all 258 stub meshes. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| S1 | Prefix roles: 0x47 entity descriptor, 0x1F visual template, 0x04 setup (Havok tagfile + body + part list), 0x06 part mesh (§2) | MV, DS; OBS | Confirmed |
| S2 | 0x01 records are **not** setups; they are cell geometry (interior spaces) (§2.2) | OBS; COL §5 | Confirmed (correction of `records.md` §2.1 and DS §1; kind from COL) |
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
| S14 | One-triangle meshes are not drawn when they are one of several parts of an object; as the only part they are drawn (§8.5) | BIN; CNS (258 meshes; 97 multi-part setups, 52 single-part setups) | BIN (rule); Confirmed (census) |
| S15 | Worn-appearance block fields: the float before each group is the group's priority, part floats are part priorities, `a` is a `MeshTypeID` slot; the key-table value counts blocks with `variant` < 0.5; per slot the highest priority wins (§8.3–§8.5, §12.2) | APP §4.3, §4.4, §4.8 | Confirmed (by APP) |
| S16 | 0x01 records are cell geometry, named by landblock-info cells; 0x66 is decoded (§2.2, §6.5) | COL §4–§5 | Confirmed (by COL) |
| S17 | An item with a worn appearance for the wearer is worn, never drawn through `PhysObj` (§13.3) | MV; APP §4.1, §5.1 | Confirmed (by APP) |
| S18 | 0x47 flag bits as the client reads them: 0x01 DID, 0x02 `kind`, 0x04 an 8-byte value, 0x08 `template`, 0x10 `block`, 0x20 a legacy scale, 0x80 property set, 0x200 `extra_sets`, 0x40 a u32, 0x100 a string, 0x400 a flag carried in the word (§4.2) | BIN (question 2); OBS (the three observed words) | BIN (bits that never occur); Confirmed (observed order) |
| S19 | `kind` is the descriptor's type code; it decides which record kinds the `template` link may name when properties are inherited (§4.3) | BIN (question 2) | BIN (meaning of each value OPEN) |
| S23 | The client has two inheritance routines with different `kind` rules: the single-property lookup and the whole-set gathering (§4.3). In the gathering, `kind` 1, 2, 3, 5, 7, 9, 19, 21, 23, 31 follow a 0x47 parent (a 0x1F target ends; any other target fails), `kind` 6 follows a game object and then its associated 0x47, every other `kind` inherits nothing; nearer layers overwrite farther ones. In this install both routines give the same result for every descriptor: every `template` chain ends at a 0x1F, every 0x47 parent has `kind` 2, 3 or 7, and the `kind` 32 record names a 0x1F | BIN (question 6); OBS (all 40,643 descriptors and their chains, 2026-10-07) | BIN (rules); Confirmed (chains) |
| S24 | The render component's scale multiplies `Render_AdjustableScale` (or `Render_AdjustableScale_Checkpointed` when held) by `Model_Scale` and further factors; `Model_Scale` is the template `vec3` (S26, 2026-10-08); the fourth factor's source is not identified (§5.2) | BIN (questions 7, 8); OBS (property census, 2026-10-07) | BIN |
| S20 | `block` is a frame: position (3 × f32, metres) then rotation (w, x, y, z); identity by default (§4.4) | BIN (question 2); OBS (25 records, rotation always identity; 16 with `kind` 9 and 9 with `kind` 7, recounted 2026-10-07) | BIN (meaning); Confirmed (census) |
| S21 | The effective property set of a 0x47 is its `extra_sets` in list order, each overwriting earlier values, then its embedded set overwriting them all; a set that cannot be loaded is skipped (§4.4) | BIN (question 3) | BIN |
| S22 | 0x1F: `value_a` precedes `value_b` in the stream; `vec3`, `value_a` and `value_b` default to 1 and are written only when they differ from 1 by more than 0.0002 (§5.2) | BIN (question 4); OBS (no stored value within 0.0002 of 1) | BIN |
| S25 | The 63 `extra_sets` DIDs that are not in gamelogic all name one set, 0x78000151, which is in `client_local_English.dat`; with the routing of `property-system.md` §6 all 20,474 resolve (§4.4) | OBS3 | Confirmed |
| S26 | Template values along a `parent` chain multiply; the model component serves `vec3` as `Model_Scale` (PID 0x3C3), so it multiplies the entity's `Render_AdjustableScale`; `value_a` is the scale an entity gives to the entities attached to it (served to them as `Model_ChildScale`, PID 0x112); `value_b` is the entity's particle-effect scale (served as `Model_ParticleScale`, PID 0x5A0) (§5.2) | BIN (question 8); OBS3 (the three definitions carry the "provided" mark of S27) | BIN |
| S27 | The client answers a property lookup from a registered provider of the entity's components, instead of the stored property sets, when the definition's `field_d` byte 3 is 1 (§5.2, §15) | BIN (question 8); OBS3 | BIN |
| S28 | The render scale's third input is the attachment parent's `value_a`; one switch replaces it by 1 and the other by the parent's `Model_ParticleScale`; both are set per attachment by the routine that attaches a spawned object (§5.2) | BIN (questions 8, 12) | BIN |
| S29 | 0x1F appearance section and setup-section elements are one "appearance-info" structure: a flag word, then up to four u32 fields and an appearance list, each governed by one bit (§5.3, §5.4) | BIN (question 11); OBS (all 29,134 templates fit) | BIN (structure); Confirmed (values) |
| S30 | 0x04: the collision-volume counts and the part count are VLE; `reserved` is the presence flag of an optional 56-byte block (values above 1 fail the read); list B's first u32 only says whether the list exists and the second is the element count (§6.1, §6.3) | BIN (question 9); OBS3 (all counts below 128; list B header pairs equal) | BIN (rules); Confirmed (census) |
| S31 | The client keeps a box as half extents and a centre; when the box's min and max are symmetric about the origin within 0.0002 the half extents are the max corner times the frame scale (§6.4) | BIN (question 9); OBS3 (all 1,205 boxes symmetric) | BIN (rule); Confirmed (census) |
| S32 | 0x20: `reserved` is the count of a u32 list, `section` is the TSize hint of the entry table, `table_tag` the TSize hint of the key table; a block is `variant` and five counted lists (§8.1, §8.3) | BIN (question 10); OBS3 (entries in bucket order of `section` in all 4,201 records; only that hint fits in every record with `section` above 0) | BIN (structure); Confirmed (order) |
| S33 | `section` census corrected: 0 occurs in 4,113 records, not 4,111 (§8.1) | OBS3 | Confirmed |

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
| 0x66 | general (16,819) | Collision mesh named by setups (§6.5); decoded in `collision.md` §4 | **no** (begins with a small count) |
| 0x20 | general (4,201) | Worn-appearance (wardrobe) record (§8) | yes |
| 0x78 | gamelogic (1,616), local (2) | Stand-alone property sets named by some 0x47 records (§4.4) | yes (`property-system.md`) |
| 0x01 | general (1,057) | Cell geometry (§2.2; `collision.md` §5). Not part of the object chain. | yes |

<a id="s2-2"></a>
### 2.2 0x01 records are not setups (Confirmed)

Earlier notes and `records.md` §2.1 call 0x01 "setup / model assembly". The data disagrees.
The 0x01 records inspected (a handful, not all 1,057) start with their DID, a byte 1, a VLE
vertex count and that many 3 × f32 positions, followed by u16 triangle indices and a
collision-tree code block shaped like the one in `mesh.md` §10. None of them has a part
list. No 0x1F or 0x04 field can hold a 0x01 DID (§5, §6), and none of the 383,332 item
property sets or the 40,643 entity property sets contains a 0x01 DID value. Setups are the 0x04 records (a DID-map label
ending in "_Setup" also points into 0x04, `records.md` §2.1).

**Resolved (2026-10-05, COL):** 0x01 records are the client's **cell geometry** (record type
`CELLMESH`, `records.md` §2.1): interior spaces, each placed by a landblock-info cell
(`landblock.md` §4.4–§4.5, `collision.md` §5.8). The leading byte described above is the
presence flag of an optional physics mesh (0 in 47 records); the record goes on with the cell
volume, portals, wall polygons, hull vertices, a property set and an embedded render mesh
(`collision.md` §5.1). Their render meshes take part in the detail-level choice of
`mesh.md` §8.5 (interior cell geometry).

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
| `ExternalAppearanceID` (0x20) | 138,503 | Not geometry: 138,469 of them name the null appearance 0x20000004, a record with one entry, one block, one group and no parts (`appearance.md` §2.1); MV found the others to be material-only records |
| `Appearance_AprFile` (0x20), nested | 67,099 | Elements of appearance lists (`appearance.md` §3.2) |
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
always present (DID, `kind`, `template`, property set). Bit 0x10 adds `block`; bit 0x200 adds
`extra_sets`. A decoder rejects other bit patterns as unsupported (none occur).

**The client's reader (BIN, row S18).** Every field is optional and governed by one bit. The
reader reads them in this order, each only when its bit is set:

| Order | Bit | Field | Type | In this install |
|---|---|---|---|---|
| 1 | 0x001 | `did` | DID | always |
| 2 | 0x002 | `kind` | u32 | always |
| 3 | 0x004 | (unnamed) | 8 bytes, a 64-bit value | never |
| 4 | 0x008 | `template` | DID | always |
| 5 | 0x010 | `block` | frame, 28 bytes (§4.4) | 25 records |
| 6 | 0x020 | legacy scale | f32 × 3 | never |
| 7 | 0x080 | `properties` | embedded property set | always |
| 8 | 0x200 | `extra_sets` | count, then DIDs | 20,274 records |
| 9 | 0x040 | (unnamed) | u32 | never |
| 10 | 0x100 | (unnamed) | string | never |
| – | 0x400 | (a flag stored in the word itself; no field follows) | | never |

The order confirms the observed layout of §4.1 (the 0x200 list follows the property set). A
legacy scale (bit 0x20) that differs from (1, 1, 1) by more than 0.0002 in any component is
not kept as a field: the reader writes it into the descriptor's own property set as both
`Physics_AdjustableScale` and `Render_AdjustableScale`. The fields of bits 0x004, 0x040 and
0x100 were not traced further. A Tessera decoder keeps rejecting words with these bits (none
occur); one that wants to accept them can use the sizes above.

<a id="s4-3"></a>
### 4.3 `kind` (Confirmed values, meaning OPEN)

7 (40,229 records), 3 (240), 2 (141), 9 (18), 23 (8), 1 (2), 5, 19, 21, 31, 32 (1 each;
32 recounted 2026-10-05 after implementation feedback, §10.1). It looks like a bit set.

**What the client does with it (BIN, row S19).** `kind` is the descriptor's **type code**
(the client's debug description of a descriptor labels it as the type, next to its DID and
the `template` DID as the parent). It decides what the `template` link may name when
properties are inherited. The client has **two** inheritance routines, and they treat
`kind` differently (rows S19, S23). In both, a descriptor's own effective property set
(§4.4) comes first, each parent is treated by its **own** `kind`, and a null `template`
inherits nothing.

**(a) Single-property lookup** (asks for one PID):

- `kind` 1, 2, 3, 5, 7, 9, 19, 21, 23 or 31: the link is followed only when it names another
  0x47 descriptor, whose properties (and its own chain) are then consulted; any other target
  (a 0x1F visual template) ends the inheritance. A parent that cannot be loaded is reported
  and ends the lookup (the property is not found beyond it).
- `kind` 6 (not in this install): the link is followed only when it names a game object (DID
  range 0x70000000–0x77FFFFFF, `property-system.md` §9). Its properties are consulted; when
  it does not hold the property, the lookup continues with the 0x47 descriptor that the
  client associates with that game object.
- Any other value (in this install only 32, one record): the link may name either a 0x47
  descriptor (as in the first case) or a game object (as for 6).

**(b) Whole-set gathering** (builds a descriptor's complete inherited property set). It
first gathers the `template` chain, then merges the descriptor's own effective set over the
result; a property that a nearer layer holds replaces the value of a farther one. It reports
success or failure; the caller receives the set either way.

- `kind` 1, 2, 3, 5, 7, 9, 19, 21, 23 or 31: when the link names a 0x47 parent, the parent's
  own chain is gathered first (by the parent's `kind`), then the parent's own effective set is
  merged over it. When it names a 0x1F visual template, the chain ends (success). **Any other
  target, or a parent that cannot be loaded, is a failure.**
- `kind` 6: the link must name a game object, otherwise failure; a game object that cannot be
  loaded, or that has no property set, is reported and is a failure. The associated 0x47
  descriptor is the DID in the game object's property `PhysObj_Override` (PID 0xB55, a DID
  property; the dictionary name is given for interoperability) when the game object holds
  it, otherwise the descriptor that the client associates with the game object (the same rule
  as in (a)). When that descriptor exists, its whole set (chain, then its own set) is
  gathered first; when it does not exist it is skipped. Then the game object's own property
  set is merged over it, so the game object's values win.
- **Every other value** (32 in this install; also 0, 4, 8, 10–18, 20, 22, 24–30 and any value
  above 31): **inherits nothing** through `template` (success).
- A failure anywhere in the chain means that **no** parent's set is merged: each level
  merges its parent's set only after that parent's own gathering has succeeded, so the
  failure passes down to the descriptor. The descriptor's own set is still merged.

**Where the two differ.** For `kind` 32 and the other values of the last case, (a) may follow
a 0x47 parent or a game object while (b) inherits nothing; a `template` that names something
other than a 0x47, a 0x1F or (for `kind` 6) a game object ends (a) quietly but fails (b); an
unloadable parent hides the farther chain in (a) but drops the whole chain in (b). **In this
install the two give the same result for every descriptor** (row S23, all 40,643): every
`template` chain ends at a 0x1F record after at most four 0x47 parents, every parent loads
and has `kind` 2, 3 or 7, and the one `kind` 32 record names a 0x1F template, so neither
routine inherits anything for it. A Tessera implementation may therefore use either rule for
this install's data; for other data it should follow (b) wherever a complete property set is
built and (a) for single queries.

> **OPEN:** what each `kind` value stands for (no label for the values was identified);
> which client features read properties through (a) and which through (b) (each routine has
> many callers; they were not mapped).

<a id="s4-4"></a>
### 4.4 Optional parts (Confirmed layout, meaning partly OPEN)

- **`block`** (25 records; `kind` 9 in 16 of them, all (0.4, 0.4, 2.7), and `kind` 7 in 9;
  two of the 18 `kind` 9 records have no block; census corrected 2026-10-07):
  seven f32. **It is a frame** (BIN, row S20): the client reads a position (three f32,
  metres) and then a rotation quaternion (four f32, (w, x, y, z) order, §6.7) with its
  ordinary frame reader; when the bit is clear the descriptor holds the identity frame (zero
  position, identity rotation), and the client's debug description prints it as the
  descriptor's position. The first three floats are therefore an **offset**, not a scale. The
  rotation is the identity in all 25 records (re-checked 2026-10-07). Observed positions:
  (0.4, 0.4, 2.7) in the 16 `kind` 9 records; in the 9 `kind` 7 records (0, 0.6, 0) four
  times and (1, 1, 1), (3, 3, 3), (0, 0, −3), (0, −1.25, 0), (0, 0, 5) once each.

  > **OPEN:** how the frame is applied (relative to what, and by which code). Narrowed
  > 2026-10-08 (BIN, fourth session): the only reader found is the debug description,
  > which prints the frame as a position only when a 64-bit world-placement identifier that
  > the descriptor holds in memory (not a record field) resolves to a placement, combining the
  > frame with that placement's cell; this suggests a frame relative to a world placement, but
  > no code that applies it to an object was found. A viewer ignores the block.

- **`extra_sets`**: 20,274 records; *n* = 1 (20,108), 2 (141), 3 (16), 4 (9). 20,474 DIDs,
  all with prefix 0x78; 20,411 of them exist in gamelogic.

  **How they combine (BIN, row S21).** When a descriptor is loaded, the client builds its
  effective property set: it starts empty, merges each extra set in list order (a property
  that a later set holds replaces the value of an earlier one), and finally merges the
  embedded property set, whose values replace all of them. The embedded set therefore wins,
  and among the extra sets the last one wins. Each DID is loaded as a stand-alone property set
  (`property-system.md` §9, code 0x32); a DID that cannot be loaded is reported as an error
  and skipped, and the others still apply. Inheritance from the `template` chain (§4.3) is a
  separate, later step: a descriptor's effective set masks its parents'.

  **The 63 DIDs not in gamelogic** (OBS3, row S25): all 63 references (in 63 records) name the
  same set, 0x78000151, which is one of the two 0x78 records of `client_local_English.dat`.
  With the routing of `property-system.md` §6 (gamelogic first, then the language archive)
  all 20,474 DIDs resolve. A Tessera loader must route 0x78 DIDs that way; on an install whose
  language archive lacks the set, the client's rule above (skip it) applies.

- **Property set.** Common PIDs, by records containing them: `Entity_CreationType` (40,091),
  `Render_LODClass` (33,018), `Render_CastLandscapeShadows` (25,867),
  `Entity_Clearance_BoxList` (21,211), `Entity_ClassScriptTable` (11,334),
  `Appearance_ClassList` (9,336), `Render_NeverSelectable` (6,363), `Translucent` (2,823),
  `Render_Opacity` (1,563), `Render_LODCrossFade` (1,365), `Render_AdjustableScale` (1,309),
  `Render_ImposterName` (1,072). None of them names a mesh. Known semantics (2026-10-05):
  `Render_LODClass` selects the object's draw distance, detail range and shadow distance,
  and `Render_LODCrossFade` its level cross-fade (`mesh.md` §8.5–§8.8, BIN);
  `Render_AdjustableScale` is a whole-body scale (`appearance.md` §6.1). The others stay
  OPEN for the renderer.

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

`appearance.md` §6.1 (2026-10-05) treats `vec3` as a uniform whole-body scale that combines
with the entity's `Render_AdjustableScale`, and finds that `value_a` and `value_b` are not
scales of the body; assembly ignores them. What the client does with all three is now known
(below, rows S26–S28; §15 records the correction for `appearance.md`).

**The client's reader (BIN, row S22).** After `flags` and `did`, the client reads, each only
when its bit is set: `parent` (0x002), `vec3` (0x020), `value_a` (0x040), `value_b` (0x400),
a list of elements (0x080, never present; each element holds two u32, a 40-byte frame and two
further u32), the appearance section (0x100) and the setup section (0x200). So **`value_a`
precedes `value_b`** when both are present (34 records); the earlier assumption is confirmed.
All three values default to 1 when absent (`vec3` to (1, 1, 1)); the client's writer omits
each one whose value is within 0.0002 of 1 (in this install no stored value is within 0.0002
of 1, all 29,134 records, re-checked 2026-10-07). When a model is set up from a template the
client copies the whole template, these values included, into the model component.

**Along a `parent` chain the values multiply** (BIN, row S26). When the client resolves a
template it starts from the values 1, takes the farthest ancestor first and works down to the
template itself; at each level it multiplies `vec3` (per axis), `value_a` and `value_b` by that
level's values. In this install no template that has a `parent` stores any of the three
(flags 0x103 and 0x003, table above), so the effective values are those of the last template
of the chain.

**What the three values do** (BIN, rows S26–S28). The model component keeps the effective
values and serves them as properties of the entity. These three properties are marked in the
property dictionary as *provided* (`field_d` byte 3 = 1, row S27, §15): a lookup of such a PID
on an entity is answered by the component that registered it, not by the entity's stored
property sets, so a stored value of these PIDs has no effect on the lookup.

| Template value | Served as (dictionary name, for interoperability) | Effect |
|---|---|---|
| `vec3` | `Model_Scale` (PID 0x3C3, a vector) of the entity itself | Input 2 of the render scale below: it **multiplies** the entity's `Render_AdjustableScale` per axis. |
| `value_a` | `Model_ChildScale` (PID 0x112, a float) of the entities **attached to** this one | Input 3 of an attached entity's render scale: things attached to an entity are scaled by the entity's `value_a`. |
| `value_b` | `Model_ParticleScale` (PID 0x5A0, a float) of the entity itself | The effects component gives every visual effect attached to the entity a uniform scale equal to this value, next to the entity's final render scale (how an effect combines the two is for the particle specs); other effect code reads the same property. |

`Model_ChildScale` of an entity is its attachment parent's `value_a`, or 1 when the entity has
no attachment parent (or is its own). Seen from the data: the Hobbit templates carry
`value_a` 0.65 and `value_b` 0.85, the female Human, Elf, High-elf and River-hobbit templates
and the Dwarf template `value_a` 0.9; so things these figures carry are drawn at 0.65 or 0.9
of their own size, and the Hobbits' effects at 0.85. That a held item is attached in this
sense is an inference (the attachment path of held items was not traced, §13).

**The render component's scale (BIN, rows S24, S28; for owner item O8).** The client
computes an entity's render scale, per axis, as the product of:

1. `Render_AdjustableScale_Checkpointed` (PID 0xB57) when the entity holds it, otherwise
   `Render_AdjustableScale` (PID 0x33B); (1, 1, 1) when it holds neither;
2. `Model_Scale` (PID 0x3C3): the effective template `vec3`, (1, 1, 1) when absent;
3. a uniform factor: the attachment parent's `value_a` (its `Model_ChildScale`), 1 when the
   entity has no attachment parent. Two switches of the entity's render component replace it
   when the entity has a parent: one by 1, the other by the parent's `Model_ParticleScale`
   (its `value_b`, 1 when the lookup fails); when both are set, the particle scale wins. The routine
   that attaches a spawned object to a parent sets both switches from two flags of the data
   that drives that attachment (the record kind that holds those flags was not traced);
4. a vector kept by the render component, (1, 1, 1) when absent. It sits in a 40-byte object
   owned by the render component, at the place where a §6.3 frame holds its scale (bytes
   12–23); the code that fills it was not found.

When the entity has an attachment parent, the product is then multiplied per axis by the
parent's own final render scale; when the parent's render component is set up, the client
recomputes the scale of each entity attached to it. If any component of the result is not finite,
the client uses (1, 1, 1); if any component is below 0.0002, it reports this and uses
(1, 1, 1). (The property names are the dictionary's, given for interoperability.) In this
install no entity descriptor's own property set holds `Model_Scale` or
`Render_AdjustableScale_Checkpointed`, and 1,309 hold `Render_AdjustableScale` (census
2026-10-07).

> **OPEN:** (1) the source of the render scale's fourth factor (item 4 above); (2) which data
> record carries the two flags that set the third factor's switches, and for which attached
> objects they are set; (3) whether held and worn items reach the attachment hierarchy that
> item 3 uses (§13; owner observation O8 can confirm the visible result: a Hobbit's weapon at
> 0.65 of its size). Answered 2026-10-08 (BIN): `vec3` multiplies the entity scale; the
> meaning of `value_a` and `value_b`.

<a id="s5-3"></a>
### 5.3 Setup section (Confirmed values; structure BIN)

A TSize (`records.md` §1.3) map with *n* = 1 in every record. Each element is a u32 **map
key** followed by an **appearance-info** (§5.5). In every record the element is:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | map key | u32 | Always 1 |
| 2 | info `flags` | u32 | Always 9: bits 0x01 and 0x08 (§5.5) |
| 3 | info `key` | u32 | Always 1 (present because of bit 0x01) |
| 4 | info `setup` | DID | The 0x04 setup DID (present because of bit 0x08) |

The bytes are the same as in the earlier reading of this table (`key` 1, `type` 9, `count` 1,
one DID); that reading is withdrawn (BIN, row S29): the 9 is a flag word and the 1 before the
DID is the info's own key, not a count. 28,583 setup sections name 28,554 distinct setups;
every named setup exists. A decoder reads the element as §5.5 says; the setup of the template
is the `setup` field of its active info (§5.5).

<a id="s5-4"></a>
### 5.4 Appearance section (Confirmed values; structure BIN; meaning from `appearance.md`)

One appearance-info (§5.5), without a map key. In every record:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | info `flags` | u32 | Always 33 (0x21): bits 0x01 and 0x20 |
| 2 | info `key` | u32 | Always 2, the key of a template's **default** info (§5.5) |
| 3 | list count | u32 | Always 1: the appearance list of bit 0x20 holds one element |
| 4 | `appearance` | DID | 0x20 record (577 of 577 exist) |
| 5 | modifiers | TSize map, element = u32 key, f32 value | *n* = 1 (571), 2 (4), 46 (2). Key 0x10000034 in 575 sections. |

The former "three u32, always 33, 2, 1" are therefore the flag word, the key and the list count
(BIN, row S29). These templates attach an appearance to a parent template's geometry. The
section is a one-record appearance list (`appearance.md` §7.4 step 2): the modifier map's keys
are `AppearanceKey` values (`ClothingColor` 0x10000034 in 575 sections, `Worn` in 3) and its
values are the modifiers that select a block of that key's entry. Mostly it dyes the parent's
geometry; 60 such templates sit on skinned setups.

<a id="s5-5"></a>
### 5.5 Appearance-info structure (BIN; values Confirmed)

The client reads the same structure in three places: the 0x1F appearance section (§5.4), the
elements of the 0x1F setup section (§5.3) and the fourth list of a 0x20 block (§8.3). Each
field is present only when its bit is set, in this order:

| Order | Bit | Field | Type | Meaning |
|---|---|---|---|---|
| 1 | – | `flags` | u32 | Always present |
| 2 | 0x01 | `key` | u32 | The info's key. 2 marks a template's default info; in a 0x20 block it is a `MeshTypeID` slot. |
| 3 | 0x02 | `base` | u32 | Key of another info of the same template (2 = the default info) with which the client combines this info after reading (how was not traced); never present |
| 4 | 0x04 | (unnamed) | u32 | Not traced; never present |
| 5 | 0x08 | `setup` | DID | A 0x04 setup DID |
| 6 | 0x20 | appearance list | u32 count *m*, then *m* × (DID of a 0x20 record, TSize map of u32 key → f32 modifier) | A DID that is already in the list does not create a second element (what happens to its modifiers was not traced) |

Bits 0x10 and 0x40 and above add nothing. Observed flag words: 33 (577 appearance sections),
9 (all setup-section elements and both block extras of §8.3). A template's **active info** is
the one whose `setup` names the template's setup: with the single setup-section element of
every record in this install it is that element. A decoder that meets another flag word can
still frame the bytes with the table above.

> **OPEN:** how the client picks the active info when a setup section holds several
> elements (none does in this install), and what the field of bit 0x04 holds; how an info
> with a `base` (bit 0x02) is combined with the info it names, and what happens to the
> modifiers of an appearance-list DID that is already in the list (both never occur in this
> install).

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
| 8 | `boxes` | VLE count, × 64 bytes | §6.4 |
| 9 | `spheres` | VLE count, × 16 bytes | §6.4 |
| 10 | `capsules` | VLE count, × 20 bytes | §6.4 |
| 11 | `collision_mesh` | DID | 0x66 DID or 0 (§6.5) |
| 12 | `reserved` | u8, then 0 or 56 bytes | Presence flag of an optional block (below); 0 in every record |
| 13 | `physics_bounds` | f32 × 8 | §6.5 |
| 14 | `part_count` *p* | VLE | Number of parts |
| 15 | `parts` | DID × *p* | 0x06 mesh DIDs (§6.6) |

A decoder that needs only the parts must still parse fields 4–13 (their sizes vary); reading
the part list backwards from the end is not reliable.

**The client's reader (BIN, row S30).** The four counts of fields 8, 9, 10 and 14 are VLEs
(`records.md` §1.2), not bytes; every count in this install is below 128 (largest: 18 boxes,
10 spheres, 8 capsules, 12 parts), so each takes one byte and the earlier "u8" reading gives
the same result here. A count larger than the bytes left fails the read. Field 12 is a
presence flag: 0 means nothing follows; 1 means a block of 14 four-byte values (56 bytes)
follows, whose meaning was not traced; any other value fails the read. The client also checks
two stream version numbers that are not part of the record bytes. The part list (field 15) is
read by the client-side setup class after the shared fields.

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

**The client's reader (BIN, row S30).** The setup reader reads the lists in the order A, B, C.
For each, a first u32 of 0 means the list is absent. For list B the client then reads a
second u32 as the element count (a count larger than the bytes left fails the read) and that
many elements of a u32 `id` and a 40-byte frame, kept in stored order; the first u32 only
decides that the list exists. In all 42 setups with list B the two u32 are equal (1 in 36,
2 in 4, 3 in 2; OBS3). Lists A and C are read as hashed collections (TSize, `records.md`
§1.3) keyed by `key`. No code that reads list B after loading was found in this session.

> **OPEN:** What list B places (a seat, a spawn or an effect point are candidates) and which
> client code uses it (its reader is known, above; no user of the stored list was found).
> World rendering ignores it.

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

**How the client keeps a box (BIN, row S31).** On reading, the client turns a box into half
extents and a centre in the setup frame. When every component of the box's centre
((min + max) / 2) is within 0.0002 of zero, the half extents are the max corner multiplied per
axis by the frame scale and the centre is the frame position. Otherwise the half extents are
(max − min) / 2 multiplied by the scale, and the centre is the frame position plus the local
centre multiplied per axis by the scale (not rotated). All 1,205 boxes of this install are
symmetric about their origin within 0.0002 (OBS3), so only the first case occurs and the
rotation question of the second does not arise.

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

Whole-body scale: the render scale of §5.2 (entity `Render_AdjustableScale` × template
`vec3`, per axis, BIN 2026-10-08), applied at the object origin before the world placement
(`appearance.md` §6.1). An object attached to another one is further scaled by the parent's
`value_a` and the parent's final scale (§5.2). The 0x47 `block` is a frame whose use is OPEN
(§4.4); `value_b` scales the object's particle effects, not its parts (§5.2). A part
whose mesh holds a single triangle is left out when the setup has several parts (§8.5).

---

<a id="s8"></a>
## 8. Worn-appearance record (prefix 0x20)

<a id="s8-1"></a>
### 8.1 Record layout (Confirmed, 4,201 of 4,201, zero trailing bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `did` | DID | Own DID |
| 2 | `reserved` | u32 | Count *m* of a list of further 0x20 DIDs that follows at once (`appearance.md` §4.2, A28: applied before the record's own entry, §4.7 rule 4 there); 0 in every record, so nothing follows |
| 3 | `section` | u8 | The **TSize hint** (`records.md` §1.3) of the entry table: fields 3 and 4 together are the entry table's TSize. 0 (4,113), 1 (9), 2 (6), 3 (16), 4 (10), 5 (32), 6 (15). |
| 4 | `entry_count` *n* | VLE | `records.md` §1.2 |
| 5 | entries | × *n* | §8.2 |
| 6 | `table_tag` | u8 | The TSize hint of the key table (fields 6 and 7 are its TSize); 1 in every record |
| 7 | `table_count` | VLE | Equals *n* in every record |
| 8 | table | × `table_count` | u32 key, u32 value (§8.4) |

**The client's reader (BIN, row S32; agrees with `appearance.md` A28).** After the DID the
client reads a u32 count and that many 0x20 DIDs (none in this install), then the entries as a
hashed collection keyed by `key` (TSize: hint byte, VLE count), then the key table as a second
hashed collection of (u32 key, 4-byte value). So `section` is a hint byte, not a section
number: in all 4,201 records the entry keys taken modulo the bucket count of hint `section`
are in non-decreasing order, and in every record whose `section` is above 0 no other hint fits
that order (OBS3). The hint follows the rules of `records.md` §1.3 (it is the smallest fitting
hint in 4,098 records, larger in 103). A reader treats both hint bytes as `records.md` §1.3
says (accept 0–22, fail at 23 or more), and a byte-exact writer keeps them.

<a id="s8-2"></a>
### 8.2 Entry (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `key` | u32 | Appearance key (0x1000xxxx). Unique within a record (no duplicates in 4,201 records). |
| 2 | `block_count` | u32 | 1 (46,215 entries), up to 53 |
| 3 | blocks | × `block_count` | §8.3 |

49,383 entries, 74,839 blocks in total (entry count recounted 2026-10-05, §10.1).

<a id="s8-3"></a>
### 8.3 Block (Confirmed framing; field attribution corrected by `appearance.md` §4.3)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `variant` | f32 | Block selector. 1.0 (45,126 blocks) is the base look; 0.00, 0.01, 0.02 … are dye variants (DS §7.5). Ascending within every entry. |
| 2 | `group_count` | u32 | |
| 3 | `group_param` | f32 | 0 (41,884), 1, 5, 3, 10 … The **priority of the first group**; 0.0 in every block without groups (`appearance.md` §4.3). |
| 4 | groups | × `group_count`, 20 bytes each | u32 `shader` (0x2B DID or 0), u32 `material` (0x30 DID or 0), u32 `a` (a `MeshTypeID` slot, or 2 `All`), u32 `b` (binding key matched against the parts' surface records, `material.md` §9.3), f32 `g` (the **priority of the next group**; after the last group it is 0.0 in every block) |
| 5 | `part_count` | u32 | |
| 6 | parts | × `part_count`, 12 bytes each | f32 `part_param` (the part's **priority**, §8.5), u32 `slot` (0x1000xxxx), u32 `mesh` (0x06 DID) |
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

**Field attribution (resolved by `appearance.md` §4.3, 2026-10-05).** The framing above stays.
Read as `appearance.md` does, the float **before** each group is that group's priority
(`group_param` for the first group, the previous group's `g` for the others) and the float
after the last group is a terminator, 0.0 in all 67,166 blocks that have groups; `a` holds a
`MeshTypeID` key in all 84,261 groups (2 `All` in most); `part_param` is the part's priority.
Group priorities: 0 (34,598), 1 (26,876), 5 (9,409), 10, 3, … ; part priorities: 1 (43,821),
10 (17,736), 15 (6,281), 4 (5,230), … ; all whole numbers from 0 to 100.

**The client's framing (BIN, row S32).** The client reads a block as `variant` followed by
five lists, each a u32 count and that many elements. It reads the same bytes as the table
above, so a decoder built on that table stays correct; the five-list reading is the one to use
for new code and for writers:

| List | Elements | Element layout | In this install |
|---|---|---|---|
| 1 | groups, 20 bytes | f32 priority, u32 `shader`, u32 `material`, u32 `a`, u32 `b` | as above |
| 2 | 4 bytes | u32 (not traced) | count 0 in every block; this count is the "0.0" that the table reads as the last `g`, or as `group_param` when there are no groups |
| 3 | parts, 12 bytes | f32 priority, u32 `slot`, u32 `mesh` | as above |
| 4 | appearance-infos (§5.5), variable size | flag word and the fields it selects | the two extras: flags 9, `key` = the slot, `setup` = the 0x04 DID |
| 5 | 56 bytes | u32, u32, a 40-byte frame, u32, u32 (the element of the 0x1F 0x080 list, §5.2) | count 0 in every block; this count is `end` |

So each group's priority is part of that group, which is how `appearance.md` §4.3 already
reads it; the "terminator" after the last group is the count of list 2; the `type` 9 of an
extra is an appearance-info flag word; and `end` is the count of list 5. A reader should accept
non-zero counts in lists 2 and 5 and skip their elements by the sizes above.

> **OPEN:** what the elements of block lists 2 and 5 mean (both lists are empty in every
> block of this install; their element sizes are known, above).

> **OPEN** (owned by `appearance.md` §4.10): how group priorities combine when several layers
> set the same material parameter.

<a id="s8-4"></a>
### 8.4 Key table (Confirmed layout and meaning)

Same keys as the entries (same order in 3,835 records, same set in the other 366). Values:
0 (45,088), 1, 2, 3, 4, other.

**Meaning (resolved by `appearance.md` §4.8, 2026-10-05):** the value stored for a key is the
number of blocks of that entry whose `variant` is below 0.5, in all 49,383 entries: 0 for a
single base block (`variant` 1.0), the block count for style lists, 46 for a 53-block dye table.
It is derived data; a decoder may check it (`appearance.md` AI2), assembly does not need it.

<a id="s8-5"></a>
### 8.5 Choosing parts for display; one-triangle parts (rules Confirmed by APP; drawing BIN)

**Choosing parts** (normative in `appearance.md` §4.4 and §3.2.1). Worn items use the block
with `variant` 1.0 (every entry an item reaches has one); style lists and dye tables choose
their block by a modifier or dye code. Each part is a candidate for its own `slot`; per
`MeshTypeID` slot the candidate with the **highest priority** (`part_param`) wins over the
setup's default part and over other candidates. Hiding is expressed only by placeholder parts
(below) winning a slot; no flag or property hides anything. Material groups and dye blocks
change materials only (`material.md` §9.3, §12; `appearance.md` §4.9–§4.10).

**One-triangle placeholder parts.** 258 mesh records have three vertices and one triangle
(stubs; 234 of them skinned), none degenerate (bound diagonals 0.011 – 150 m). One further
record, 0x0600C113, has a single triangle in its detail level 0 but 24 in each of its two
other levels; it is no stub (`mesh.md` §8.10). 227 stubs carry a body, equipment, mount or
war-machine slot in their `flags` (`Accessory1` 33, `Crest` 28, `Helmet` and `Head` 17 each,
`Cape` and `Shoulder` 16 each, `LowerBody` 13, `UpperBody` and `WarMachine_Armor` 12 each,
`Hands` 10 …; §12.2) and 31 carry `Default`: they are **slot placeholders**, used as
default parts of character and creature setups for slots that stay empty until something
fills them, and in worn items to hide the slot they win (`appearance.md` §4.4).

**What the client draws (BIN, `mesh.md` §8.10; answers owner item O4):**

1. When the client loads a mesh it counts the triangles of all its detail levels; a mesh with
   exactly one triangle is flagged. On this install the flagged meshes are exactly the 258
   stubs.
2. When it builds the draw list of an object whose model has **more than one part**, flagged
   parts are left out. A placeholder part of a figure or a multi-part setup is therefore
   **never drawn**, whatever its material and whichever slot it fills.
3. A model whose **only** part is a flagged mesh draws it.

Census of the setups (CNS): 149 setups list stub parts (309 references to 192 distinct
stubs). In 97 of them the setup has several parts, so the stubs are not drawn; the example of
the M2 checklist (setup 0x04000110, parts 1 and 2: meshes 0x06003A8E and 0x060068B9, slots
`Shoulder` and `Helmet`) is one. The other 52 setups have a stub as their only part and draw
it: 22 of their 27 distinct stub meshes use surfaces whose material colour 2000 has alpha 0
(0x310000E5 and four others: `lighting.md` §5.12 makes such a surface invisible), and 5 use
ordinary opaque surfaces (single large triangles, 2 – 150 m across, flags `Default`).

Stub surfaces over all 258 stubs: 0x310000E5 (151; material colour (1, 1, 1, 0)), 0x31001E41
(47; alpha 0), 0x3100003D (7) and 0x3100000C (5) (alpha 0), 0x3100000D (2; (0, 0, 0, 0)),
0x31000482 (29), 0x3100008F (6), 0x31007D29 (3), 0x3100589E (2) and six surfaces used once (no
colour override, so opaque). Even where a placeholder would be drawn, the alpha-0 materials
keep 212 of the 258 stubs invisible; the rule of step 2 does not depend on them.

Renderers: leave flagged meshes out of multi-part objects (Tessera's `MeshSummary` stub test,
at most three vertices, selects the same 258 records; `mesh.md` MI14).

---

<a id="s9"></a>
## 9. Invariants (Confirmed)

| ID | Invariant |
|---|---|
| **SI1** | 0x47: `did` equals the record DID; `template` prefix is 0x1F or 0x47; the embedded property set decodes with the dictionary and the record ends exactly after the optional 0x78 list. |
| **SI2** | 0x1F: flags ⊆ {0x001, 0x002, 0x020, 0x040, 0x100, 0x200, 0x400}; bit 0x001 set; every record has either a `parent` or a setup section, never both; setup section *n* = 1, map key 1, info flags 9, info key 1 (§5.3); appearance section info flags 33, info key 2, list count 1 (§5.4). |
| **SI3** | 0x04: tagfile magic at offset 8; frame-list headers repeat their count; `reserved` = 0; every box symmetric about its origin within 0.0002 (§6.4); every volume and part count below 128; the record ends exactly after the part list. |
| **SI4** | Every part DID is an existing 0x06 record; every `collision_mesh` DID an existing 0x66 record. |
| **SI5** | 0x20: `reserved` = 0; `table_tag` = 1; `table_count` = `entry_count`; block `end` = 0; entry keys unique; entry keys taken modulo the bucket count of hint `section` (`records.md` §1.3) are non-decreasing (4,201 of 4,201, 2026-10-08). |
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

> **OPEN** (narrowed 2026-10-08): the meaning of each 0x47 `kind` value (its role in
> property inheritance is known, §4.3, in both inheritance routines); which client features
> use the single-property lookup and which the whole-set gathering (§4.3); how the `block`
> frame is applied (§4.4). Resolved: the flag bits and field order (§4.2), `block` is a
> position-and-rotation frame (§4.4), how `extra_sets` combine (§4.4), where the 63 sets not
> in gamelogic live (the language archive, §4.4, 2026-10-08).

> **OPEN** (narrowed 2026-10-08): the render scale's fourth factor and the data behind its
> third factor's switches (§5.2); the active-info choice for several setup-section elements
> and the field of appearance-info bit 0x04 (§5.5). Resolved 2026-10-08 (BIN): `vec3`
> multiplies the entity scale, `value_a` scales attached objects, `value_b` scales particle
> effects (§5.2); the appearance section's three constants are a flag word, a key and a list
> count (§5.4).

> **OPEN:** what 0x04 frame list B means and which code uses it (§6.3; its reader is known
> since 2026-10-08); what the optional 56-byte block of field 12 holds (§6.1; absent in every
> record). (`attach_map`, lists A and C and the tagfile contents are resolved by part 2:
> §6.2, §6.3, §11; the 0x66 layout by `collision.md` §4.)

> **Resolved (2026-10-08, BIN and OBS3):** 0x20 `section` is the entry table's TSize hint
> (§8.1); `reserved` is the count of a list of 0x20 DIDs, empty in every record (§8.1,
> `appearance.md` A28); the block is five counted lists (§8.3). Not traced, and never
> present: the meaning of the elements of block lists 2 and 5.
> (Group fields, `part_param` and the key-table values are resolved by `appearance.md` §4.3,
> §4.8: §8.3–§8.4.)

> **Resolved (2026-10-05, BIN; owner item O4):** one-triangle placeholder parts are not drawn
> as one of several parts of an object (§8.5, `mesh.md` §8.10).

> **Resolved (2026-10-05, COL):** 0x01 records are cell geometry placed by landblock-info cells
> (§2.2).

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

> **Resolved (2026-10-05, `appearance.md` §4.11):** the 3,102 entries are selection tables
> (dye tables, NPC and style records, head, hair, beard and ear styles). Their block is always
> chosen by a modifier (`appearance.md` §3.2.1) or a dye code (§4.9 there); a viewer without
> one shows the first block. Worn-item structs never land on them.
> Original note: 3,102 entries have several blocks and none with `variant` = 1.0, so §8.5's base
> look ("the block with variant 1.0, or the only block") is undefined for them.
> `AppearanceEntry::base_block` returns nothing there; which block a viewer should draw is
> open.

> **Resolved (2026-10-08, BIN):** every constant on the list is now explained by the client's
> readers, so a decoder can frame other values instead of rejecting them. 0x47: bits 0x01,
> 0x02, 0x08, 0x80 are presence bits and every other bit adds a field of known size (§4.2).
> 0x1F appearance section: 33, 2, 1 are an appearance-info flag word, its key and a list count
> (§5.4, §5.5). 0x04 `reserved`: a presence flag; 1 adds a 56-byte block, above 1 the client
> fails the read (§6.1). 0x20 `reserved`: the count of a list of 0x20 DIDs; `table_tag`: a TSize hint
> (0–22 valid); block `end`: the count of list 5 (56-byte elements); extra `type` 9: an
> appearance-info flag word (§8.1, §8.3, §5.5). Rejecting unseen values remains allowed as a
> conservative choice; accepting them with the sizes given is what the client does. The
> tagfile magic is a format constant (`havok-data.md` §3.1).
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
> appearance for the wearer, in which case it is worn (§13.3; confirmed by `appearance.md`
> §4.1, §5.1: every worn item also has a `PhysObj`, its look on the ground); the display case
> and items lying on the ground stay open.
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

> **Answered in part (2026-10-05, `appearance.md` §4.11):** material-only entries are the dye
> tables of the same record or selection tables of NPC pieces; they apply only together with
> parts that the same record contributes. How a dye element pairs with pieces of another
> record stays OPEN in `appearance.md` §15.
> Original note: Appearance entries whose first entry is material-only: 63 of 103 sampled
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
three-vertex stubs, §8.5). Worn items, style lists and NPC pieces offer parts for slots, and
per slot the part with the highest priority wins over the default part (`appearance.md` §4.4,
normative there); a winning stub leaves the slot empty, because one-triangle parts of a
multi-part object are not drawn (§8.5).

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
### 13.3 Worn versus held (Confirmed by `appearance.md` §4.1, §5.1)

Every worn item carries an `Item_WornAppearanceMapList` as well as a `PhysObj`; the worn
appearance (§3.2) is what the wearer shows, and the `PhysObj` chain leads to its look on the
ground. Rule: an item with a worn-appearance entry for the wearer's species and sex (after the
wardrobe fallbacks of `appearance.md` §2.3) is worn and never drawn through `PhysObj`;
otherwise it is held. Among the items with both, the `Weapon_Primary` battle gauntlets (200,
drawn as `Hands` parts) and the `Class` satchels (120, in the `Satchel` slot) are the ones that
might look like held items (`appearance.md` §5.1).

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

> **OPEN:** what frame list B means and which code uses it (§6.3); its reader is known
> (2026-10-08), no user of the stored list was found.

> **OPEN** (for `script-table.md`, and owner observation): holding-location choice in the
> world, and stowing (§13.2).

> **Resolved (2026-10-05, BIN):** one-triangle slot placeholder parts are not drawn when the
> object has several parts (§8.5; owner item O4).

> **Resolved (2026-10-08, BIN):** the 0x1F `value_a` (scale of attached objects), `value_b`
> (particle-effect scale) and the composition of `vec3` with the entity scale (it multiplies)
> (§5.2; `appearance.md` §6.1, owner item O8). Still OPEN in §5.2: the render scale's fourth
> factor, the data behind the switches of its third, and whether held items are attached in
> that sense (the owner can confirm the visible result, O8).


<a id="s14-1"></a>
### 14.1 Implementation feedback, part 2 (clean room, 2026-10-05)

`tessera-formats` exposes the part-2 meanings as typed views over the unchanged decoders:
`Setup::bone_for` and `Setup::attachments` (§6.2), `Setup::holding_location` with an
`Anchor` of origin (−1) or bone (§6.3 list A), `Setup::viewport_frame` (list C), and
`TemplateAppearance::elements` (§5.4 as a one-record appearance list). Its real-data test
reproduces the §14 statistics (378 attach maps, list A in 2,737 setups, list B in 42, list C
in 50) and finds the skeleton-independent parts of SI7 and SI8 (unique keys and values, list
A bones ≥ −1) in every setup; the largest bone index any setup names is 200.

> **Resolved (2026-10-08, spec owner):** SI7, SI8 and SI10 are defined against the bone count
> of the setup's own skeleton (`havok-data.md` §4.2, the reference pose's bone array, §11.1).
> A decoder that keeps the tagfile opaque checks them only when given that count, as
> `Setup::bone_issue` does; the full check belongs to a real-data test that also decodes the
> skeletons. No fact is missing. Original note: SI7, SI8 and SI10 compare indices with the
> skeleton's bone count, which lives in the tagfile (`havok-data.md`). The formats crate keeps
> the tagfile opaque, so it checks them only when a caller passes the bone count; the full
> check belongs to a test that also decodes skeletons.

---

<a id="s15"></a>
## 15. Corrections to other specs

Recorded here instead of editing specs owned by other writers.

| Spec | Statement | Correction |
|---|---|---|
| `appearance.md` §4.4 step 3, §8 step 8, §13 | Whether the original draws a winning stub's triangle is owner item O4; "stubs per the O4 policy (draw or skip)" | Answered by BIN (§8.5 here, `mesh.md` §8.10): one-triangle meshes are left out of the draw list of any object with more than one part, so a stub in a figure is never drawn. |
| `skeleton-anim.md` §9.2, `setup.md` §8.5 (before 2026-10-05) | "242 three-vertex skinned stub meshes … (`Accessory1` 32, `Crest` 28, `Shoulder`, `Head`, `UpperBody`, `Helmet` 17 each, `Cape` 16 …)" | 258 three-vertex stubs, 234 of them skinned; slots `Accessory1` 33, `Crest` 28, `Helmet` 17, `Head` 17, `Cape` 16, `Shoulder` 16, `LowerBody` 13, `UpperBody` 12, …, `Default` 31 (census of all mesh records, 2026-10-05; §8.5). |
| `docs/checklists/m2-visual.md` row C6, owner item O4 | Open: is the white triangle drawn by the original? | Not drawn (BIN, §8.5); the owner observation is no longer needed, a frame capture could still confirm it. |
| `appearance.md` §6.1 (Decision and OPEN O8) | Whether the template `vec3` enters the render scale is OPEN (owner O8); the holder factor *s* is taken as 1 "until its sources are known" | `vec3` enters: the model component serves the effective chain `vec3` as `Model_Scale` (PID 0x3C3), which is input 2 of the render scale, so it multiplies `Render_AdjustableScale` (BIN 2026-10-08, §5.2 here; a High-elf female at about 1.84 m). The holder factor is the holder's `value_a` (served as `Model_ChildScale`, PID 0x112), replaced by 1 or by the holder's `value_b` when the attachment's switches say so (§5.2). |
| `appearance.md` §6.1, last paragraph | "The template floats `value_a` and `value_b` are not scales …; assembly ignores them." | They are not scales of the figure itself, which is right for assembly; but `value_a` scales the objects attached to the figure and `value_b` its particle effects (served as `Model_ParticleScale`, PID 0x5A0) (§5.2). |
| `property-system.md` §2.2–§2.3 | `field_b` and `field_d` byte 3: meaning OPEN; "the client's readers and value display never use these fields" | When the client looks a property up on an entity (BIN 2026-10-08): `field_b` selects where the value comes from (4, the value of 23,331 definitions: the entity's own property store; 3 and 8: two other stores of the entity; 7 and 9: a global service; any other value: not found); and inside the entity's own store, a definition whose `field_d` byte 3 is 1 (18 definitions) is answered by a provider registered by one of the entity's components instead of the stored values. Three of the 18 are the model component's (§5.2). Neither field affects decoding. |
| `appearance.md` §4.2 (A28), §15 | "`setup.md` §8.1 rows 2 and 3 should be revised by that spec's owner" | Done 2026-10-08 (§8.1): row 2 is the DID-list count, row 3 the entry table's TSize hint, whose bucket order the entries follow in all 4,201 records. |

---

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-04 | §0–§10 | Part 1: object chains, 0x47, 0x1F, 0x04 and 0x20 layouts (Review). | Spec writer (dirty room) |
| 2026-10-05 | §6.2, §6.3, §7, §11–§14 | Part 2 (M4): skeleton, attach map, holding locations, character setups, held items. | Spec writer (dirty room) |
| 2026-10-05 | §1 (APP/COL/MESH, BIN, CNS; S2, S14–S17), §2.1, §2.2, §3.3, §4.4, §5.2, §5.4, §7, §8.3, §8.4, §8.5 rewritten, §10, §10.1, §10.2, §12.2, §13.3, §14, new §15, §16 | Owner item O4 answered by BIN: one-triangle parts are left out of multi-part objects (with the stub census of setups and surfaces); corrections of `appearance.md` folded in (group and part priorities, key-table values, slot rule, selection tables, worn versus held, template scale, template appearance sections, null appearance); corrections of `collision.md` folded in (0x01 cell geometry, 0x66 decoded); LOD-class semantics pointed to `mesh.md` §8.5–§8.8. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN second session, questions 2–5; S18–S22), §4.2, §4.3, §4.4, §5.2, §7, §10, §10.1, §14, §16 | M4 (decision 0011): the client's 0x47 reader (every flag bit, field order, the legacy scale turned into `Render_AdjustableScale` and `Physics_AdjustableScale`); `kind` is the descriptor's type code and decides what the `template` link may name in property inheritance; `block` is a position-and-rotation frame, not a scale (OPEN narrowed to its use); `extra_sets` merge in list order and the embedded set wins, missing sets are skipped; 0x1F `value_a` precedes `value_b`, all three template values default to 1 (re-checked: no stored value within 0.0002 of 1 in all 29,134 templates; the 25 blocks have identity rotations). List B's reader not found. Status Accepted → Revised. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN third session, questions 6–7; S20 corrected; new S23, S24), §4.3, §4.4, §5.2, §16 | Review changes of 2026-10-07. **(1)** `block` census corrected (recount of all 40,643 descriptors): 16 records with `kind` 9 (all (0.4, 0.4, 2.7)) and 9 with `kind` 7 (positions listed per kind); two of the 18 `kind` 9 records have no block. **(2)** §4.3 now gives both inheritance routines: the single-property lookup (as before) and the whole-set gathering, in which `kind` 1, 2, 3, 5, 7, 9, 19, 21, 23, 31 follow a 0x47 parent (a 0x1F ends, any other target fails), `kind` 6 follows a game object (`PhysObj_Override` first) and then its associated 0x47, and every other value inherits nothing; where they differ, and that they agree on every descriptor of this install (all chains end at a 0x1F, all parents `kind` 2, 3 or 7, the `kind` 32 record names a 0x1F). Which callers use which routine is OPEN. Owner item O8, partial: the render scale's inputs (§5.2); `vec3` not found among them. Status stays Revised, awaiting re-review. | Spec writer (dirty room) |
| 2026-10-08 | header, §1 (BIN fourth session, questions 8–12; OBS3; S24 updated; new S25–S33), §4.4, §5.2, §5.3, §5.4, new §5.5, §6.1, §6.3, §6.4, §7, §8.1, §8.3, §9 (SI2, SI3, SI5), §10, §10.1, §14, §14.1, §15, §16 | OPEN pass. **Closed:** the 63 unresolved `extra_sets` (all name 0x78000151 in the language archive); `vec3` multiplies the entity scale (served as `Model_Scale`), `value_a` scales attached objects (`Model_ChildScale`), `value_b` scales particle effects (`Model_ParticleScale`), template values multiply along a chain, the render scale's third factor and its two switches; the 0x1F appearance and setup sections are one appearance-info structure (constants 33, 2, 1 and 1, 9, 1 explained; the "type 9, count 1" reading of §5.3 withdrawn, same bytes); 0x04 volume and part counts are VLEs, `reserved` is a presence flag of a 56-byte block, list B's reader and header; how the client keeps boxes; 0x20 `reserved` is a DID-list count and `section` the entry table's TSize hint (census corrected to 4,113), the block is five counted lists (same bytes); the implementation-feedback OPENs on constants (§10.1) and on SI7/SI8/SI10 (§14.1). **Narrowed:** list B (reader known, user not found), O8 (fourth render-scale factor, the data behind the switches, whether held items are attached). New, precise: active-info choice for several setup-section elements, appearance-info bit 0x04, the 56-byte block of 0x04. Corrections for `appearance.md` §6.1, §4.2 and `property-system.md` §2.3 in §15. Status Accepted → Revised. | Spec writer (dirty room) |
| 2026-10-08 | header, §5.5, §8.3 | Review of the 2026-10-08 revision: passed. Two OPEN callouts added for unknowns the text already named as not traced (the `base` combination and duplicate appearance-list DIDs in §5.5; the elements of block lists 2 and 5 in §8.3). No facts changed. Status Revised → Accepted. | Spec reviewer |
