<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Collision geometry, cells and portals, and character movement

| | |
|---|---|
| **Status** | Accepted (2026-10-08) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec reviewer; separate from the writer's session) |
| **Review record** | Claude review session (spec reviewer), 2026-10-07: re-review of the second revision of 2026-10-07, **Accepted** (CLEAN_ROOM.md §5.3, every item; F1–F7 resolved). Re-review of the 2026-10-08 revision, 2026-10-08 (fresh reviewer session): **Accepted** (CLEAN_ROOM.md §5.3, every item; C28, C40–C43 spot-checked in the binary). Earlier rounds and detailed findings are in the dirty-room review log (decision 0018). |
| **Implements into** | `crates/formats` (0x66 and 0x01 decoders, quantised-tree reader), `crates/world` (cells, portals, point-in-cell, collision residency), `crates/terrain` (terrain collision triangles), `crates/physics` (decision 0013: collision sets, Tessera's own acceleration structure, sweeps and the character controller of §12), `apps/client` (walk mode on collision instead of terrain heights), `apps/dattool` / `apps/explorer` (inspection), `fuzz/` (one target per record kind and one for the tree reader) |
| **Companion specs** | `mesh.md` §8–§10 (detail levels, per-level collision mesh, tree block; **corrected** here in §7 and §14). `landblock.md` §3.1, §3.6, §3.9, §3.11 (height grid, split diagonal, collision tree blocks; §4.4 cells; **extended** here in §6 and §5.8). `setup.md` §2.2, §6.4, §6.5 (0x01 records, collision volumes, 0x66 DID and physics bounds; **completed** here in §4, §5 and §8). `entity-descriptor.md` §4.5, §9 (SED properties, draw rule; collision rule added here in §9). `scenery.md` §7 (procedural scenery; its clearance step uses §6.6, its objects collide per §9.4). `terrain-texturing.md` §4.5 (terrain holes; corrected here in §6.5). [`water.md`](water.md) §6–§7 (landblock water, water volumes; answers the water OPENs of §10.4). `property-system.md` §4–§8 (values, embedded sets, DID maps). `records.md` §1.2 (VLE), §2.1 (prefix table). `coordinates.md` §11.1 (rotations). `appearance.md` §6.1 (render scale and the avatar entities' scales; the physics scale is §8.1). `wstate.md` (0x70 objects). |

---

<a id="s0"></a>
## 0. Purpose

This spec gives an implementer what is needed to make a character **stand on, walk through and
bump into** the world the way the original client does, from the user's own data:

- every collision structure in the archives, decoded to the last byte, with censuses over
  **all** records: the quantised bounding-volume code block shared by meshes, terrain,
  ceilings, collision meshes and cells (§3); stand-alone collision meshes, prefix 0x66 (§4);
  cell geometry, prefix 0x01, with its cell volume, portals and wall polygons (§5); terrain
  and ceiling collision (§6); the per-level collision data of render meshes (§7); the
  collision volumes of setups (§8);
- how the client builds terrain and ceiling collision, why terrain holes do not need cutting,
  how it assigns a point to a cell, and the terrain-triangle query that scenery placement
  uses (§5.8.1, §6.4–§6.6);
- the per-instance flags that switch collision off or make an object pass-through for some
  kinds of movers, and the collision of procedurally generated scenery (§9);
- the movement parameters the original uses for player characters and other movers, read
  from the data, and the rules found in the client for parameter selection, walkable slopes,
  support, moves and step-ups (§10–§11);
- a character controller for Tessera built from those facts, marked as a decision where the
  original's rule is still unknown or where Tessera chooses differently (§12);
- invariants, synthetic worked examples, corrections to other specs and open questions
  (§13–§16).

All counts are observations of one install on one date and change with patches. Movement
parameters are **data** and must be read at run time (§10); tests assert invariants, not
counts.

Names of properties, enum labels and DID-map labels appear because they are stored in the
data and a reader looks values up by them (interoperability). Names of the third-party
physics middleware's classes are used only to identify a format (§3).

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: observed in the user's install on 2026-10-05 (later passes dated in the OBS
  row) by throwaway probes in the dirty-room scratchpad, over **all** records of the kind
  unless a sample is stated.
- **BIN**: read from the client executable under decision 0011 (method M4); see the BIN row.
  "Confirmed (BIN)" means the fact rests on the executable only.
- **Probable**: supported by data statistics or names, not by an exact rule.
- **Reported**: from published material only.

| Tag | Source |
|---|---|
| OBS | User install, Steam, data current on 2026-10-05. Read in full: all 1,057 records with prefix 0x01 and all 16,819 with prefix 0x66 (`client_general.dat`); all 28,555 setups (0x04); all 40,643 entity descriptors (0x47), all 1,616 stand-alone property sets (0x78) and all 383,332 object property sets (0x79) (`client_gamelogic.dat`); all 47,563 mesh records (`client_mesh.dat`, `client_mesh_aux_1.datx`); all 390,150 landblock-data and 49,636 landblock-info records of `client_cell_{1,2,3,4,5,14,15}.dat`. Every quantised code block of every 0x66, 0x01 and landblock record and of every mesh level was walked; every triangle that a code block contains was looked up through the code at its centroid and at its three corners. Second pass, same date: the surface layers, ceiling grids and stored ceiling triangles of all 390,150 landblock-data records (6,867 with a ceiling) for §6.4–§6.5, and the 0x47 chains, setups and physics properties of the 4,261 objects named by scene records for §9.4. Third pass, same date (answering the review): the stored x and y origin of all 58,546 terrain trees and the x origin of all 1,095 ceiling trees (§3.2); edge pairing and signed volume of all 16,819 0x66 meshes under both closure tests (§4.2); last-plane normals and hull-vertex distances of all 1,057 0x01 records (§5.4); the frill-object census recounted with the reach rule of `scenery.md` §8.2 (§9.4). Fourth pass, 2026-10-06: the per-SED census of §9.3 (all Physics SEDs of all landblock-info records; chain values from the 0x47 records' embedded sets). Fifth pass, 2026-10-07: the SED's own `Entity_CreationType` of all 2,380,699 Physics SEDs (§9.3); the §9.1 properties of all 40,643 0x47 records with and without their 0x78 extra sets (§9.1); the names of the 14 layer properties and the `MoveToController_*` properties in the property dictionary (§10.3); the §9.3 table recomputed over the 2,380,675 Physics SEDs that have a position. Sixth pass, 2026-10-07 (answering the review of that date): the 4,261 distinct 0x47 objects named by all 3,402 scene records (prefix 0x02), each checked for liquid monitors along its chain with the repository's water-volume decoder (`water.md` §7.3) as a tool (§9.4). Seventh pass, 2026-10-08: `Model_Scale` and `Physics_AdjustableScale` of all 3,253,862 landblock-info SEDs (§8.1); the dictionary types of the properties of §9.1, §9.3, §11.1 and `water.md` §7.2 (C41). |
| BIN | Client executable `x64/lotroclient64.exe`, build 4906.0071.1794.4035 (changelist 711794), read with a disassembler (capstone) on 2026-10-05. **Questions:** (1) which rule turns the per-state movement records into the active parameters; (2) the walkable-slope limit; (3) how the per-avatar float height reaches the controller; (4) what the 16 movement properties are as a unit; (5) what `Physics_NoPhysics` and the shape of a moving entity mean to the client. Second pass, same build and date: (6) how the client builds terrain and ceiling collision (holes, ceiling diagonals, landblocks without a stored tree); (7) how it decides which cell contains a point; (8) what collision it gives to procedurally generated scenery and in which order the pieces are built; (9) the terrain-triangle query used by scenery placement; (10) the instance-ID test for player characters. **Third pass, 2026-10-06 (strings and RTTI only):** (11) the loader's content-layer and creation-type handling for Physics SEDs; (12) which character controller the client uses. The executable carries run-time type information for the middleware's character construction-info types and character motion only, not for a character-proxy or character-rigid-body class, so it does not show which controller the client uses; the third pass's conclusion that the player is driven by the middleware's proxy is withdrawn (§11.3). **Fourth pass, 2026-10-07 (incident 0018 revision; same build; Ghidra headless on a copy of the analysed project):** (13) which `Entity_CreationType` value the static-creation path reads, and what happens to statics of types 7 and 8; (14) whether the content-layer result gates the creation of a static (and so its collision body); (15) whether the property lookup that serves the §9.1 flags includes a 0x47 record's 0x78 extra sets; (16) how a setup's collision mesh and volumes become one shape, and how an object's scale applies to each part; (17) the convex radius the client gives boxes and triangle meshes; (18) how the support contacts of a mover are classified; (19) the client's move procedure: casts, contact planes, step-up. **Why M1–M3 were not enough:** the data holds the parameter records but not the rule that picks and layers them, and holds no slope limit at all; which triangles the client puts into its collision shapes, how it assigns points to cells, how generated scenery becomes physical, which property value its creation filter reads, the shape radius it adds and how it moves a character are run-time behaviour that no record stores; the original client cannot be observed here (M3) and no published source describes these rules (M2). Only the offline world and movement code was read; nothing about network, login, keys, launcher or anti-tamper. The velocity integration (jump, ramp, control acceleration, damper) was looked for in the third and fourth passes and not located. **Fifth pass, 2026-10-07 (answering the review of that date; same build; Ghidra headless on a copy of the analysed project, deleted afterwards):** (20) which root-shape kind the face-normal flip of §11.3 tests, and whether the client builds bodies of that kind; (21) when the terrain and ceiling meshes get a tree at load time, and which of the three bodies of §6.3 the client makes; (22) the layout of a generated object's instance ID (§9.4); (23) what the creation-type reader returns when no value is found (§9.3). **Why M1–M3 were not enough:** the kind codes of run-time shapes, the loader's switches and the synthesized IDs are run-time state that no record stores and the running client does not show; no published source describes them. Read: the support collector's callback again, the constructors of the middleware's tree and height-field shapes and every instruction that writes the tree kind code, the terrain mesh builder, its callers and the shape cache it uses, the static-request builder and the two instance-ID helpers it calls, and the creation-type reader. Nothing about network, login, keys, launcher or anti-tamper. **Sixth pass, 2026-10-08 (answering the implementer OPENs of 2026-10-07; same build; Ghidra headless on a copy of the analysed project, deleted afterwards):** (24) whether a cell's solid body and its lookup phantom are registered independently, and whether making the body reads any property; (25) which builder and convex radius a cell's physics mesh gets; (26) which value types the client's readers accept for the §9.1 booleans and bitfields, the §11.1 conditions, the creation type and the scale; (27) how the physics component turns the scale properties into the scale of its shape; (28) what the content-layer test does with a value that is not a list; (29) the corner order of every terrain and ceiling triangle; (30) the routine that sets `Entity_Element`. **Why M1–M3 were not enough:** the dictionary gives each property one type, so the data never exercises a reader's handling of another type; the order of registrations, the shape scale and the corner order of run-time meshes are not stored in any record; the running client cannot be observed here and no published source describes them. Read: the cell registration routine and the two shape accessors of the cell geometry, the readers named in C41, the physics component's scale update and its set-up routine, the content-layer evaluator, the terrain mesh index builder, the support-flag accessors and the element routine. Nothing about network, login, keys, launcher or anti-tamper. |
| PUB | Published descriptions of the middleware's bounding-volume code, written for another game that uses the same middleware: the PyNifly project wiki page "MOPP Bytecode Format" and the PyFFI NIF library (BSD licence; read for facts only). Used as a starting hypothesis for §3.4; every opcode that occurs was then checked against data (OBS). |
| PUB2 | Mirarkitty `lotro_extractor` (GitHub; read for the fact only): its exporter recognises a character's instance ID by the top byte 0x02. |
| MESH, LB, SET, ED, SCN, TT, WAT | `mesh.md`, `landblock.md`, `setup.md`, `entity-descriptor.md`, `scenery.md`, `terrain-texturing.md`, `water.md` (same install). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| C1 | The tree block is a quantised bounding-volume program; opcode set, operand sizes, branch semantics (§3.4) | PUB; OBS (every code block of every host, 0 unknown opcodes) | Confirmed for every opcode that occurs; 0x29–0x2B Reported |
| C2 | Integer coordinates: floor((p − origin) × scale); the byte compared at a node is bits 16–23 (§3.3) | PUB; OBS (point queries, 0 misses on 0x66, 0x01 physics meshes and terrain) | Confirmed |
| C3 | Diagonal split coordinates of opcodes 0x13–0x1C, computed at full precision (§3.4.3) | OBS (fitted; PUB's table is wrong for 0x14 and silent on 0x1A–0x1C) | Confirmed |
| C4 | origin = minimum corner − 0.05; scale = 254 · 65,536 / (largest extent + 0.1) (§3.2); terrain and ceiling trees store the x origin (and terrain trees the y origin) as the f32 bits 0xBD4CCCCB | OBS (16,819 + 1,010 + every mesh level; terrain within 2 cm; stored bits in 58,546 of 58,546 terrain trees and 1,095 of 1,095 ceiling trees) | Confirmed |
| C5 | Leaf keys: triangle index (0x66, 0x01 physics mesh); grid triangle number (terrain, ceiling); draw position · 2²⁰ + triangle (mesh levels) (§3.5) | OBS | Confirmed (mesh: with the exception in §3.5) |
| C6 | 0x66 layout: VLE vertex count, vertices, VLE triangle count, u16 triangles, tree block (§4) | SET (start); OBS (16,819 of 16,819, zero leftover bytes) | Confirmed |
| C7 | Setup physics bounds equal the 0x66 vertex bounds (§4.3) | OBS (16,812 of 16,812 mesh-only setups) | Confirmed |
| C8 | 0x01 layout: physics mesh, cell-volume planes, portals, wall polygons, hull vertices, property set, embedded render mesh (§5) | OBS (1,057 of 1,057, zero leftover bytes) | Confirmed |
| C9 | A cell's neighbour list is its geometry's portal list, index for index; the back-reference is the portal index in the neighbour (§5.8) | OBS (104,271 cells; 393,842 neighbour entries, all reciprocal) | Confirmed |
| C10 | Cell placement: position + (w, x, y, z) active rotation in the landblock frame (§5.8) | OBS (392,980 of 393,842 portal pairs coincide within 1 cm) | Confirmed |
| C11 | Portals and wall polygons tile the faces of the cell volume (§5.5) | OBS (6,524 of 6,531 faces, area ratio 1.00) | Confirmed |
| C12 | Terrain tree triangle numbering 2 · (32 · j + i) + side (§6.2) | OBS (58,546 trees, 119.9 M triangles, 0 misses) | Confirmed |
| C13 | Movement properties form one 16-field unit (§10.1) | BIN; OBS (records carry exactly these fields) | Confirmed (BIN) |
| C14 | Player parameter records and their values (§10.3) | OBS (one 0x79 record) | Confirmed |
| C15 | Layering order and conditions of player parameter records (§11.1) | BIN | Confirmed (BIN) |
| C16 | Float height chain and the controller it is written to (§11.2) | BIN; OBS (values) | Confirmed (BIN) |
| C17 | Support classification: walkable if the surface normal's up component exceeds cos 45° (player characters) or cos 70° (other movers); sliding support up to cos 85°; the face-normal substitution with stored corner order and absolute values (§11.3) | BIN; BIN (18: details of §11.3.1) | Confirmed (BIN) |
| C18 | Player collision capsule from the setup, scaled by the physics scale of §8.1 (§10.6) | SET; OBS (all playable setups); BIN (27, scale) | Confirmed (shape); BIN (scale); use by the controller Probable |
| C19 | Ethereality bitfields decide which movers pass through an object (§9.2) | OBS (census); names | Probable |
| C20 | The client builds a terrain collision mesh of all 2,048 grid triangles for every landblock, numbered as in §6.2; no triangle is left out for holes (§6.1, §6.5); the loader builds no tree at load time; the physics body is the mesh with its stored tree, a height field (no tree, switch set) or the bare mesh (no tree, switch not set) (§6.3) | BIN (6, 21) | BIN |
| C21 | Ceiling collision: same numbering, diagonal from **bit 0** of the vertex-flag byte, triangles clockwise from above, built from all ceiling samples (§6.4) | BIN (6); OBS (5,310,848 of 5,310,848 stored ceiling triangles follow bit 0) | Confirmed |
| C22 | Terrain triangles covered by no layer occur only in landblocks with a ceiling, and never where the ceiling lies above the terrain (§6.5) | OBS (all 390,150 records; 2,750,134 such triangles) | Confirmed |
| C23 | Each cell is registered as a solid body (its physics mesh) and a phantom (hull of its hull vertices); a point is in a cell when every volume plane gives at least −0.0002 (§5.4, §5.8.1) | BIN (7) | BIN |
| C24 | Terrain triangles near a point: the client's radius query (§6.6) | BIN (9) | BIN |
| C25 | Generated scenery becomes static entities through the same creation request as landblock-info statics; frills never become entities; build order; the static-creation path takes a landblock-info static only when it passed the content-layer test and its creation type is not 7 or 8 (§9.3, §9.4) | BIN (8, 13, 14); OBS (census) | BIN (path, order, filters); Confirmed (census) |
| C26 | A mover is a player character for the slope rule when its instance ID's top byte is 2 (§11.3) | BIN (10); PUB2 (meaning of the byte) | BIN (test); Reported (meaning) |
| C27 | The §9.3 table is a data census over the 2,380,675 Physics SEDs with a position; the run-time set of collision bodies applies the content-layer test (offline *L* = 1, *I* = 0) and the creation-type filter; census 1,651,097 / 97,688 / 82,139, run-time 1,607,212 / 88,749 / 73,509 after the content-layer test and 1,607,211 / 88,749 / 73,509 after the creation-type filter (§9.3) | OBS (fourth and fifth passes); BIN (13, 14) | Confirmed (census and counts); BIN (filters) |
| C28 | Which surfaces the client calls "controlled" and "uncontrolled" support; `Entity_Element` is set from the support flags of §11.3.1 and the mover's `Volume_IsLiquid`: 5 / 6 / 4 out of liquid, 1 / 3 / 2 in liquid (§11.3.2) | BIN (18: the collector's flags and their debug labels; 30: the element routine); OBS (SL1–SL3 support faces 53.5°/61.4°/56.3°) | BIN (flags and element rule); Probable (that state 6 makes the mover slide) |
| C29 | The creation type that the static-creation filter reads is the SED's own `Entity_CreationType`, else the value found along the 0x47 chain (§9.3; agrees with `entity-descriptor.md` §4.9, E24); no value anywhere counts as 0 (rejected); no Physics SED of the install has an effective value 7 or 8, one has 0 | BIN (13, 23); ED (§4.9); OBS (fifth pass: 1,602 placed Physics SEDs have 7 or 8 on their chain, all carry their own value 3, 5, 6 or 9) | BIN (rule); Confirmed (census) |
| C30 | The §9.1 properties of a 0x47 level are read from its 0x78 extra sets and its embedded set by the lookup of `scenery.md` S31; counts with extra sets (§9.1) | BIN (15, the same descriptor lookup as `scenery.md` S31); OBS (fifth pass) | BIN (rule); Confirmed (counts) |
| C31 | A setup's collision is one compound of its mesh and all its volumes; per-part scale rules; scale treated as 1 within 0.0002 per axis (§8.1) | BIN (16) | BIN |
| C32 | Boxes, 0x66 meshes, terrain and ceiling collision and cell physics meshes carry a convex radius of 0.05 m (§8.2) | BIN (17, 25) | BIN |
| C33 | Support contacts: distance limit 0.08 m, exclusions, early-out after the first walkable contact, the crease rule (§11.3.1) | BIN (18) | BIN |
| C34 | The client's move: up to 10 casts per move, the contact planes of the plane solver, the extra vertical plane for steep contacts, push-out of penetrating contacts, the ground check, moves longer than 500 m refused (§11.4) | BIN (19) | BIN |
| C35 | The client's step-up procedure (§11.5) | BIN (19) | BIN |
| C36 | The names of the 14 layer properties `PlayerPhysicsControl_<Layer>Parameters` and of the `MoveToController_*` fields (§10.3) | OBS (fifth pass, property dictionary) | Confirmed |
| C37 | The face-normal flip of §11.3 tests the middleware's generic tree kind; only that kind's own constructor sets the code, every concrete tree kind replaces it, and no traced builder makes a body of that kind, so the flip is expected never to apply (§11.3) | BIN (20) | BIN (codes and constructors); Probable (never applies) |
| C38 | A generated object's instance ID: kind byte 5, the landblock's region, X, Y and instance number, and the 0-based list position in bits 0–23 (§9.4) | BIN (22); ED (§4.2 layout) | BIN |
| C39 | None of the 4,261 0x47 objects named by scene records carries a liquid monitor along its chain (§9.4) | OBS (sixth pass) | Confirmed |
| C40 | A cell's lookup phantom and its solid body are registered separately: the phantom always, the body only when the cell has a non-empty physics mesh; making the body reads no property of the cell; no size limit was found (§5.3) | BIN (24) | BIN |
| C41 | Value types the client's readers accept: Boolean (type 21, true when non-zero) for the §9.1 booleans, the §11.1 conditions and the volume flags of `water.md` §7.2; Bitfield32 (type 18, its 32 bits) for ethereality; Enum (8) or property ID (10) for `Entity_CreationType`; Vector (6) for the scale properties; Array (12) for `Entity_ContentLayers`. A value found with another type reads as false, empty, 0 or default; the lookup does not continue past it. The dictionary declares each of these properties with exactly the accepted type (§9.1, §9.3, §11.1) | BIN (26, 28); OBS (seventh pass, dictionary types) | BIN (readers); Confirmed (types) |
| C42 | The physics scale is uniform: *s* = z of `Physics_AdjustableScale` × z of `Model_Scale` (defaults 1); a non-uniform value is logged and its z used; a non-finite product or one below 0.0002 becomes 1; no landblock-info SED carries `Model_Scale`, and every SED's `Physics_AdjustableScale` is uniform within 0.0002 (§8.1, §10.6) | BIN (27); OBS (seventh pass) | BIN (rule); Confirmed (census) |
| C43 | Corner order of the two triangles of every terrain and ceiling quad, for both diagonals (§6.1, §6.4, §14.4) | BIN (29); OBS (agrees with C12 and C21) | BIN |

---

<a id="s2"></a>
## 2. Where collision comes from

| Source | Record | Geometry | Spatial code | Used for (status) | § |
|---|---|---|---|---|---|
| Outdoor ground | landblock data `0x800RXXYY` | the 33 × 33 height grid, two triangles per 5 × 5 quad; the client builds this mesh for every landblock, holes included | terrain tree in 58,546 of 390,150 records | standing and walking (Confirmed geometry; mesh building BIN) | §6 |
| Cave ceilings | landblock data, ceiling grid | the ceiling grid, diagonal from bit 0 | ceiling tree in 1,095 records | blocking from above (mesh building BIN) | §6.4 |
| Static and placed objects | setup 0x04 → stand-alone collision mesh 0x66, or the setup's boxes, spheres, capsules | triangle mesh or primitives | tree in every 0x66 | blocking movement (Confirmed shape; §9 rule) | §4, §8 |
| Procedural scenery (trees, rocks) | scene placement (`scenery.md` §7) → 0x47 → setup | as for placed objects | as for placed objects | blocking movement (§9.4) | §9.4 |
| Interiors (rooms, caves, dungeon pieces) | cell geometry 0x01, named by landblock-info cells | physics mesh of the cell (a solid body); cell volume and hull (a phantom); portals | tree on the physics mesh | blocking movement; which cell contains a point (§5.8.1); visibility through portals | §5 |
| Render meshes, per detail level | mesh 0x06, `mesh.md` §9–§10 | welded copy of the level's render triangles | tree over the level's render triangles | not for character movement (Probable: picking, camera, line of sight) | §7 |
| Moving entities (characters, creatures) | setup 0x04 volumes | one capsule, sphere or box | none | the mover's own shape | §8, §10.6 |

Frames: every structure is stored in the frame of the thing that places it: landblock-local
for terrain (`landblock.md` §3.3), cell-local for 0x01 (§5.8), setup-local for 0x66 and the
volumes (`setup.md` §7). All are Z-up, metres, right-handed (`mesh.md` §11).

---

<a id="s3"></a>
## 3. The quantised tree block

<a id="s3-1"></a>
### 3.1 Container fields (Confirmed)

Every host stores the same fields; only the presence byte differs.

| Field | Type | Meaning |
|---|---|---|
| (presence) | u8 | 0 or 1; see the host table below |
| `origin` | 3 × f32 | Minimum corner of the quantisation box, host frame, metres |
| `scale` | f32 | Integer units per metre (§3.3) |
| `code_length` *n* | u32 | Bytes of code |
| `code` | *n* bytes | The program (§3.4) |
| `build_byte` | u8, value 1 | Present **iff the record's directory `stamp` ≥ 38** (`mesh.md` §10.4) |

| Host | Presence | Trees | Leaves name | § |
|---|---|---|---|---|
| Mesh level | `has_tree` then `has_code` (`mesh.md` §10.1) | 71,167 blocks, 70,956 with code | render triangles of the level | §7 |
| Landblock terrain | `has_terrain_tree` (`landblock.md` §3.9; no `has_code`) | 58,546 | grid triangles | §6 |
| Landblock ceiling | `has_ceiling_tree` | 1,095 | ceiling grid triangles | §6.4 |
| 0x66 collision mesh | `has_code` (always 1) | 16,819 | the record's triangles | §4 |
| 0x01 physics mesh | `has_code` (always 1) | 1,010 | the record's physics triangles | §5.3 |
| 0x01 embedded mesh levels | as for mesh levels | 1,280 with code, 83 without | render triangles of the level | §5.7 |

The `build_byte` rule held in every host: in 0x01 records, all 214 physics-mesh trees with
`stamp` ≥ 38 carry it and the 796 others do not; the embedded mesh follows the mesh rule.

**Stamp-less fallback (Tessera Decision, 2026-10-07; raised by the implementation on
2026-10-06).** Without a stamp, or with an unobserved stamp 34–37, a reader decides
`build_byte` from the bytes: for 0x66, it is present exactly when one byte remains after the
code (the tree ends the record); for a 0x01 physics mesh, it is present unless the four bytes
after the code are one of the three plane tags of §5.4. On the owner's install (2026-10-06)
this agrees with the stamp rule on all 16,819 0x66 records and all 1,010 physics meshes. A
record with `has_code` = 0 (never observed; the layout without a tree is not known) is
rejected as unsupported.

<a id="s3-2"></a>
### 3.2 Origin and scale (Confirmed)

The builder quantises the axis-aligned box of the triangles it was given:

- `origin` = per-axis minimum of the triangle vertices − 0.05;
- `scale` = 254 · 65,536 / (*L* + 0.1), where *L* is the largest of the three extents
  (maximum − minimum) of those vertices.

So the box plus a 0.05 margin on each side spans integer coordinates 0 to about 254 · 2¹⁶
on its longest axis. Checked: all 16,819 0x66 trees and all 1,010 0x01 physics-mesh trees
(origin within 10⁻⁴, scale within 10⁻⁵ relative); every mesh level (§7). Terrain trees:
the x and y origin is −0.05 within 10⁻⁷ (stored as the f32 with bits 0xBD4CCCCB,
−0.04999999, two units in the last place from the f32 nearest −0.05, which is 0xBD4CCCCD;
the same bits in x and y of all 58,546 terrain trees and in x of all 1,095 ceiling trees),
so a reader that compares with the exact f32 −0.05 finds none; *L* = max(160, height
range); the z origin lies 0.00 –
0.02 below "minimum height − 0.05" (rounding in the builder). Readers never recompute these
values: they use the stored `origin` and `scale`.

<a id="s3-3"></a>
### 3.3 Integer coordinates

A point *p* in the host frame becomes three signed integers
*q*ₐ = floor((*p*ₐ − `origin`ₐ) · `scale`), computed in double precision. The code compares
**bytes**: at a node, the byte value of axis *a* is *q*ₐ shifted right by 16 bits
(arithmetic shift, so points below the origin give negative bytes and are rejected by the
root filters). Implementations must use at least 64-bit signed integers: the rescale opcodes
(§3.4) shift *q* left.

<a id="s3-4"></a>
### 3.4 Instruction set

The program is a byte stream walked from offset 0. Multi-byte operands are **big-endian**
(most significant byte first), unlike everything else in the archives. A walk carries a
program counter, the three integer coordinates (or coordinate ranges, §3.6) and an output
base *b* (initially 0). Offsets below are "from the end of the instruction" unless stated.

| Opcode | Size | Operands | Meaning | Observed |
|---|---|---|---|---|
| 0x00 | 1 | – | End of this branch; nothing is reported | yes (mesh, 0x01) |
| 0x01 – 0x04 | 4 | *X*, *Y*, *Z* (bytes) | Rescale: for each axis, *q* ← (*q* − operand · 2¹⁶) · 2ˢ with *s* = opcode (1–4) | yes |
| 0x05 | 2 | *c* (byte) | Jump forward *c* bytes | yes |
| 0x06 | 3 | *c* (2 bytes) | Jump forward *c* bytes | yes |
| 0x07 | 4 | *c* (3 bytes) | Jump forward *c* bytes | yes (0x66, 0x01, mesh levels) |
| 0x09 | 2 | *k* (byte) | *b* ← *b* + *k* | yes |
| 0x0A | 3 | *k* (2 bytes) | *b* ← *b* + *k* | yes |
| 0x0B | 5 | *k* (4 bytes) | *b* ← *k* | yes (0x01 embedded meshes, mesh levels) |
| 0x10 – 0x12 | 4 | *hi*, *lo* (bytes), *c* (byte) | Split on axis X, Y, Z (§3.4.1); right child at +*c* | yes |
| 0x13 – 0x1C | 4 | *hi*, *lo*, *c* | Split on a diagonal coordinate (§3.4.3) | yes |
| 0x20 – 0x22 | 3 | *t* (byte), *c* (byte) | Split on X, Y, Z with one threshold (§3.4.1); right child at +*c* | yes |
| 0x23 – 0x25 | 7 | *hi*, *lo* (bytes), *c*₁, *c*₂ (2 bytes each) | Split on X, Y, Z; left child at +*c*₁, right child at +*c*₂ | yes |
| 0x26 – 0x28 | 3 | *lo*, *hi* (bytes) | Filter on X, Y, Z (§3.4.2) | yes |
| 0x29 – 0x2B | 7 | *lo*, *hi* (3 bytes each) | Filter on X, Y, Z at 24-bit precision: compares *q* shifted right by 8 | no (Reported, PUB) |
| 0x30 – 0x4F | 1 | – | Report key *b* + (opcode − 0x30); end of branch | yes |
| 0x50 | 2 | *k* (byte) | Report *b* + *k*; end of branch | yes |
| 0x51 | 3 | *k* (2 bytes) | Report *b* + *k*; end of branch | yes |
| 0x52 | 4 | *k* (3 bytes) | Report *b* + *k*; end of branch | yes (0x01 embedded meshes, mesh levels) |
| 0x53 | 5 | *k* (4 bytes) | Report *b* + *k*; end of branch | yes (mesh levels only, 9,316) |
| any other | – | – | Malformed | never observed |

Jump and split targets always lie inside the code in every observed block; branches may
share code (two splits can lead to the same bytes), so a reader must not assume a tree
shape. Opcode totals over the 0x66 blocks: filters 4.4 M, axis splits 2.2 M, diagonal splits
1.5 M, one-threshold splits 19,501, wide splits 169,427, rescales 378,485, leaves 4.0 M.

#### 3.4.1 Axis splits (Confirmed)

For a split with operands *hi*, *lo* on axis *a* and byte value *v*:

- the **left** child (the next instruction, or +*c*₁ for 0x23–0x25) is taken when *v* ≤ *hi*;
- the **right** child is taken when *v* ≥ *lo*;
- both are taken when both hold (the children overlap when *lo* ≤ *hi*); none when *v* lies
  strictly between *hi* and *lo* (a gap with no triangles).

For a one-threshold split (0x20–0x22) with threshold *t*: left when *v* ≤ *t*, right when
*v* ≥ *t* (both when equal). A reading with "left when *v* < *t*" misses 0.3 % of the
triangles' own corner points and is wrong.

#### 3.4.2 Filters (Confirmed)

A filter with *lo*, *hi* on axis *a* ends the branch (nothing reported) unless
*lo* ≤ *v* ≤ *hi*. Code blocks start with filters on all three axes (first byte 0x28 in 95 %
of mesh blocks, `mesh.md` §10.2).

#### 3.4.3 Diagonal coordinates (Confirmed)

The value compared by opcodes 0x13–0x1C is computed from the **full-precision** integers
*x*, *y*, *z* (§3.3, after any rescale) and then shifted right by 16 bits. With
*F* = 2¹⁶ and integer division rounding down:

| Opcode | Value before the shift |
|---|---|
| 0x13 | (*y* + *z*) / 2 |
| 0x14 | (*y* − *z* + 255 · *F*) / 2 |
| 0x15 | (*x* + *z*) / 2 |
| 0x16 | (*x* − *z* + 255 · *F*) / 2 |
| 0x17 | (*x* + *y*) / 2 |
| 0x18 | (*x* − *y* + 255 · *F*) / 2 |
| 0x19 | (*x* + *y* + *z*) / 3 |
| 0x1A | (*x* + *y* − *z* + 255 · *F*) / 3 |
| 0x1B | (*x* − *y* + *z* + 255 · *F*) / 3 |
| 0x1C | (*x* − *y* − *z* + 510 · *F*) / 3 |

Evidence: with these formulas 71,264 of 71,264 diagonal decisions on the paths to sampled
triangles agree; computing them from the bytes instead fails 62 times; PUB's formula for
0x14 has the opposite sign.

<a id="s3-5"></a>
### 3.5 What a key means (Confirmed)

| Host | Key |
|---|---|
| 0x66, 0x01 physics mesh | Index of a triangle in the record's triangle list |
| Terrain, ceiling | Grid triangle number (§6.2), 0 – 2,047 |
| Mesh level, 0x01 embedded mesh level | *s* · 2²⁰ + *t*: *t* is the triangle index inside the index buffer of a draw entry, *s* the position of that draw among the level's draws |

Mesh keys exception: in 15 of the 72,236 coded levels (12 in 0x06 records, 3 in 0x01
embedded meshes) one draw takes no part in the code and the later draws are numbered one
lower: the code's subpart number counts only the draws it contains. In all 15 the skipped
draw has exactly **one** triangle (not degenerate: up to 5.7 m²), and it is never the last
draw. Every other level uses the plain draw position. A reader that maps keys back to
triangles must therefore number subparts over the draws that occur in the code (for example
by checking which subparts the code reports).

> **OPEN:** The rule that leaves such a draw out (its single triangle, or its surface: two of
> the 0x01 cases use surface 0x310003B4, the proxy surface of `mesh.md` §8.3).

<a id="s3-6"></a>
### 3.6 Coverage (Confirmed)

- **0x66:** 16,804 codes report every triangle; 15 omit 23 triangles in total, all with area
  below 0.01 m² (slivers).
- **0x01 physics mesh:** all 1,010 codes report every triangle (227,070).
- **Terrain:** every code reports exactly the 2,048 grid triangles.
- **Mesh levels:** small triangles are often omitted (details in §7.2), and so are the
  skipped draws of §3.5.
- Within one code no key is reported by two different leaf instructions, and no key outside
  the host's range occurs except through §3.5's renumbering.

So a reimplementation that builds its **own** acceleration structure from the triangles
loses nothing and gains the omitted slivers.

<a id="s3-7"></a>
### 3.7 Queries (algorithms in words)

The code answers "which triangles may touch this region"; exact tests follow on the
triangles themselves.

**Point query.** Convert the point to integers (§3.3). Walk from offset 0 with a stack of
pending branches. At a rescale, replace the integers. At a jump, move the counter. At an
output-base opcode, update *b* for this branch. At a filter, end the branch unless the byte
passes. At a split, compute the byte (or diagonal value), and continue with every child
whose condition holds, pushing the second when both hold. At a leaf, report the key and end
the branch. At 0x00 end the branch. Stop when the stack is empty. Checked: every triangle a
code contains is reported for its centroid and for each of its corners (0x66: 15.5 M
queries; 0x01 physics: 0.9 M; terrain: 479.6 M; no miss).

**Box query.** Carry a range [*q*ₐ⁻, *q*ₐ⁺] per axis instead of a value. Rescale both ends.
A filter passes when the ranges overlap. A split takes the left child when the range's
lower end satisfies the left condition, and the right child when its upper end satisfies the
right condition. For a diagonal split, compute the lowest and highest diagonal value over
the box: each term with a plus sign takes the axis minimum for the lowest value and the axis
maximum for the highest; each term with a minus sign the reverse. The result is a superset
of the triangles whose boxes overlap the query box.

**Ray or segment.** Use the box of the segment (or subdivide the segment into pieces and
query each piece's box), then intersect the ray with each reported triangle.

**Sphere or capsule sweep.** Query the box that encloses the shape at its start and end
positions, grown by the radius; then sweep the shape against each reported triangle.

**Limits for hostile input:** bound the number of executed instructions per query (for
example 64 × code length), refuse operand reads past the end of the code, refuse jumps
outside the code, and cap the branch stack (for example 4,096 entries). Treat an unknown
opcode as malformed.

**Overflow and end of code (Tessera Decision, 2026-10-07; raised by the implementation on
2026-10-06).** A key or base *b* + *k* (or *b* + the 0x09/0x0A operand) beyond 32 bits is
malformed. A jump or child offset that lands exactly at the end of the code is outside the
code, and a branch that runs off the end without an ending instruction is truncated.
Rescaled coordinates saturate at the 64-bit range instead of wrapping. No observed code
reaches any of these cases.

**Absolute budgets (Tessera Decision, 2026-10-07; raised by the implementation on
2026-10-06).** A budget proportional to the code length alone lets a short code followed by
unreachable padding buy work and memory in proportion to the padding (an exponential graph of
splits that share their children). Readers therefore also cap every query at 2²² executed
instructions and every structural walk (each (offset, *b*) state once) at 2²¹ states,
whatever the code length, and report each key once per walk. A query over the whole integer
range takes every branch; the largest such query in the observed data (a render-mesh level of
778,485 code bytes) executes 263,380 instructions, and the largest walk visits as many
states. Whether the original bounds its walks is not known and does not matter: Tessera
collides with its own structure (§3.8).

<a id="s3-8"></a>
### 3.8 What Tessera does with the code (Decision)

The code is **optional**. A reader must parse past it (its length is stored) and may keep
the bytes for tools. Collision queries in Tessera use a structure built from the triangles
(§12.1). The point and box queries above exist so that tools can display and verify the
original's code and so that a later version may use it.

---

<a id="s4"></a>
## 4. Stand-alone collision mesh (prefix 0x66, `client_general.dat`)

<a id="s4-1"></a>
### 4.1 Layout (Confirmed, 16,819 of 16,819, zero leftover bytes)

The record does **not** start with its DID.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `vertex_count` *V* | VLE (`records.md` §1.2) | 4 – 4,345 (median 34) |
| 2 | vertices | *V* × 3 × f32 | Positions, setup frame, metres |
| 3 | `triangle_count` *T* | VLE | 2 – 7,918 (median 60); 3,868,526 in all |
| 4 | triangles | *T* × 3 × u16 | Vertex indices |
| 5 | `has_code` | u8 | Always 1 |
| 6 | tree fields | §3.1 | `origin`, `scale`, *n*, code, then `build_byte` iff `stamp` ≥ 38 (10,002 records with, 6,817 without) |

Every index is below *V*; no triangle repeats a vertex. Stamps observed: 7 – 39 (39 and 38
are the most common).

<a id="s4-2"></a>
### 4.2 Shape (Confirmed)

A mesh is counted as **closed** here when every directed edge (*a*, *b*) of its triangles
occurs exactly once and its reverse (*b*, *a*) also occurs exactly once (a closed
two-manifold surface). By that definition 12,770 meshes are closed; 12,719 of them have
positive signed volume, i.e. triangles are **counter-clockwise seen from outside**, as in
`mesh.md` §11.3; 51 closed meshes are inside out; 4,049 are open. (The looser test "every
directed edge has at least one reverse" gives 12,873 closed, 12,822 positive and 51 inside
out, and 3,946 open.) Collision tests must therefore treat triangles as **two-sided**.

<a id="s4-3"></a>
### 4.3 Relation to setups (Confirmed)

Every 0x66 record is named by exactly the setups whose `collision_mesh` holds its DID
(`setup.md` §6.5; 16,822 setups, 16,819 distinct DIDs, all exist). For all 16,812 setups whose
only collision is the mesh, `physics_bounds` (`setup.md` §6.5) is the axis-aligned box of the
0x66 vertices (within 6 × 10⁻⁴), its height is max z − min z and its horizontal radius is the
largest distance of a vertex from the vertical axis through the origin. The mesh is therefore
in the **setup frame**, the same frame as the setup's parts; it moves with the placed object
(`entity-descriptor.md` §5) and scales with `Physics_AdjustableScale` (§9.1).

---

<a id="s5"></a>
## 5. Cell geometry (prefix 0x01, `client_general.dat`)

0x01 records describe interior spaces. `setup.md` §2.2 established that they are not
setups; `landblock.md` §4.4 that landblock-info cells name them. This section gives the
full layout.

<a id="s5-1"></a>
### 5.1 Layout (Confirmed, 1,057 of 1,057, zero leftover bytes)

| Order | Part | Type | § |
|---|---|---|---|
| 1 | `did` | DID | own DID |
| 2 | `has_physics_mesh` | u8 | 1 in 1,010 records, 0 in 47 |
| 3 | physics mesh | §5.3 | present iff part 2 is 1 |
| 4 | cell volume | plane list, §5.4 | always |
| 5 | portals | u32 count, entries, §5.5 | always |
| 6 | wall polygons | u32 count, entries, §5.5 | always |
| 7 | hull vertices | VLE count, count × 3 × f32, §5.6 | always |
| 8 | properties | embedded property set (`property-system.md` §5) | always |
| 9 | `mesh_count` | u32 | always 1 |
| 10 | `mesh_flag` | u8 | always 0 |
| 11 | embedded render mesh | a mesh record without its DID: from `flags` to the end of region D (`mesh.md` §3–§10) | always |

All coordinates are in the **cell frame** (§5.8). Record sizes 829 – 2,981,586 bytes
(median 45,218).

<a id="s5-2"></a>
### 5.2 Census (Confirmed)

| Item | Total | Per record |
|---|---|---|
| Physics-mesh vertices / triangles | 144,147 / 227,070 | 4 – 1,916 / 2 – 2,896 |
| Planes | 6,531 | 5 – 28 (6 in 1,006 records) |
| Portals | 3,361 | 0 – 54 (0 in 24 records) |
| Wall polygons | 6,523 (3,652 triangles, 2,863 quads, 8 with 5–8 corners) | 0 – 42 |
| Hull vertices | 10,368 | 8 – 56 (8 in 762) |
| Embedded mesh submeshes | 3,613 | levels: one kind-1 level (834), two (136), three (85), one kind-1 and two kind-2 (2) |
| Stamps | 7 – 39 | |

The 1,033 distinct DIDs named by the 104,271 cells of landblock info are all present; 24
records are named by no cell (4 of them without a physics mesh).

<a id="s5-3"></a>
### 5.3 Physics mesh (Confirmed; use BIN)

The same fields as a 0x66 record (§4.1): VLE vertex count, vertices, VLE triangle count,
u16 triangles, then `has_code` (always 1) and the tree fields (§3.1). It is a **simplified**
shell of the room (for example 43 triangles where the render geometry has 127), separate
from the render mesh of part 11. All 1,010 codes report every triangle (§3.6).

BIN: when a landblock is loaded, the client registers this mesh, placed with the cell's
frame (§5.8), as the cell's solid collision body. It is
what movers collide with inside a cell, in the same role as a setup's 0x66 mesh for
objects. A cell without a physics mesh (47 records) gets no body. The per-level collision
meshes of part 11 serve other queries (§7); the 47 cells without a physics mesh have empty
or tiny render collision too (33 have an empty level-0 collision mesh).

Flags of the cell body (Tessera Decision): the body carries the §9.1 properties found in the
record's own property set (part 8, §5.7): `Physics_SurfacePropertiesToApply` and
`Physics_CanLedgeClimb`. Neither changes whether the body blocks a mover. No 0x01 record holds
`Physics_AlwaysUncontrolledSurface` or an ethereality value (OBS: part 8 holds only the four
properties listed in §5.7), so a cell body always counts as support by the rules of §11.3.

**Registration and limits (BIN, C40; answers the two implementer OPENs of 2026-10-07).** The
client registers a cell's lookup phantom (§5.8.1) and its solid body as two separate objects.
The phantom is registered for every placed cell. The body is made only when the cell has a
non-empty physics mesh; making it reads no property of the cell, so `Physics_NoPhysics` or any
other §9.1 value in part 8 has no effect on it. No size limit on the mesh was found in the
client. Of the cell's properties, the list the client keeps for a cell
names `Physics_SurfacePropertiesToApply` and `Physics_CanLedgeClimb` explicitly and no other
§9.1 property.

Tessera's limits of §12.1 are its own (decision 0013), and the Tessera Decision stands as the
implementer chose it: a physics mesh refused at any limit (more triangles than one template
may hold, 2²⁰; a set-level limit of §12.1 on bodies, triangles, or spheres, capsules and
boxes; a placement that cannot be applied) gives no body and is counted as a body over a
limit, and the cell is still placed in the cell lookup. Both refusal paths give the same
lookup, which is the client's behaviour: its phantom never depends on the body. The cell
itself is refused only at the cell limit or the per-set plane and hull-vertex budgets of
§12.1. The body carries only the two properties named above; any other §9.1 value found in
part 8 (none is observed), `Physics_NoPhysics` included, is ignored.

<a id="s5-4"></a>
### 5.4 Cell volume (Confirmed)

A list of planes read until a terminator. Each entry is 20 bytes:

| Offset | Type | Meaning |
|---|---|---|
| 0 | u32 | Tag: 0x42502D2D ("BP--" read as big-endian characters) for every entry but the last; the last is 0x4250464C ("BPFL", 679 records) or 0x4C454146 ("LEAF", 378 records) |
| 4 | 3 × f32 | Unit normal *n* (length 1 within 10⁻³ in all 6,531 entries) |
| 16 | f32 | Distance *d* |

Read entries until one whose tag is not 0x42502D2D; that entry is the last and is still a
plane. The tags are named only for interoperability.

Meaning: the cell volume is the convex region where *n* · *p* + *d* ≥ 0 for every plane.
Every hull vertex (§5.6) satisfies all planes within 5.2 mm: 1,050 records within 1 mm,
1,056 within 5 mm, and one record (0x010001D8) at 5.17 mm. Every plane touches at least one
hull vertex. The last plane is the floor, with normal z above 0.999, in 947 records
(exactly (0, 0, 1) in 868; z above 0.9 in 959); it is the most upward-facing plane of its
list in 960 records. Readers must not assume a floor plane at any position.

BIN: the client stores the list as a chain of plane nodes, each entry's "inside" branch
leading to the next entry and its "outside" branch empty. Its point test walks the chain
and reports **inside** when every plane gives *n* · *p* + *d* ≥ −0.0002 (a tolerance of
2 × 10⁻⁴ units), outside at the first plane that gives less.

> **OPEN:** What distinguishes the two terminator tags (they follow data age: "LEAF" in 335 of
> the 360 stamp-7 records). Readers treat both alike.

<a id="s5-5"></a>
### 5.5 Portals and wall polygons (Confirmed)

Portal entry: u8 (always 1), u32 corner count (always 4), corners × 3 × f32.
Wall-polygon entry: u32 corner count (3 – 8), corners × 3 × f32.

Every portal (3,359 of 3,361) and every wall polygon (6,509 of 6,523) lies in one of the
volume's planes (within 5 mm). On each face of the volume, the portals and wall polygons on
that plane cover the face exactly: summed area / face area = 1.00 on 6,524 of 6,531 faces.
So the boundary of the cell volume is partitioned into **openings** (portals, leading to a
neighbouring cell, §5.8) and **closed wall** pieces. Portal normals are axis-aligned in
3,358 of 3,361 cases.

> **OPEN:** Meaning of the portal's leading byte (always 1) and the winding convention of
> portal and wall corners.

<a id="s5-6"></a>
### 5.6 Hull vertices (Confirmed)

A VLE count (every observed count is below 128, so a u8 reading gives the same bytes) and
that many points: the corners of the cell volume and of the pieces its faces are split into.
8,648 of 10,368 lie on three or more planes, 834 on two, 884 on one and 2 on none (within
2 mm).

<a id="s5-7"></a>
### 5.7 Properties and embedded render mesh (Confirmed)

Part 8 holds, by count of records: `Physics_SurfacePropertiesToApply` (655; a 0x78 set naming
the surface material, e.g. stone, for footsteps and physics), `Ambient_SoundEnum` (521),
`Physics_CanLedgeClimb` (53), `Render_CastDynamicShadows` (1).

Part 11 is a complete mesh body: `flags` (always 1), the surface list, submeshes, index
buffers and detail levels with their collision meshes and tree blocks, decoded exactly as
`mesh.md` §13 steps 1–6 describe from step 1's `flags` onward, with the record's `stamp`
deciding `build_byte` (§3.1). Vertex formats: 0x30000112 (3,509 submeshes), 0x30000152 (94),
0x30000212 (6), 0x00000112 (4). This is what a renderer draws for the cell.

<a id="s5-8"></a>
### 5.8 Cells, placement and portal links (Confirmed)

A landblock-info cell (`landblock.md` §4.4) places one 0x01 record:

- **Placement:** a cell-frame point *v* maps to the landblock frame as *R* · *v* + *t*, where
  *t* is the cell's `position` x, y, z and *R* its orientation (w, x, y, z), applied as an
  active rotation (`coordinates.md` §11.1; identity when absent). Checked by joining portals:
  for 392,980 of 393,842 neighbour entries the portal and its partner coincide within 1 cm
  in the landblock frame (862 pairs, mostly in one landblock, 0x8021FAE9, do not).
- **Neighbours = portals:** a cell's `neighbour_count` equals its geometry's portal count in
  all 104,271 cells. Neighbour entry *k* describes portal *k*: its neighbour cell is the cell
  seen through portal *k*, and its back-reference is the portal index **in the neighbour's
  geometry** that is the same opening. The link is reciprocal in all 393,842 entries.
- The neighbour's flag byte is 1 in 392,318 entries and 0 in 1,524.

> **OPEN:** Meaning of the neighbour flag byte (a closed or one-way portal?) and of the 862
> non-coincident portal pairs. (How a cell is entered from outdoors: §5.8.1.)

**Point in cell (algorithm in words).** To find the cell containing a landblock-frame point
*p*: for each candidate cell, map *p* into the cell frame with the inverse placement
(subtract *t*, rotate by the inverse of *R*) and test it against every plane of the cell
volume (§5.4); the point is inside when no plane gives less than −0.0002 (the client's
tolerance, BIN; a tolerance of 6 mm covers every hull vertex of the data, §5.4). The
client's own lookup is §5.8.1.

<a id="s5-8-1"></a>
#### 5.8.1 Which cell contains a point; entering a cell from outdoors (BIN)

When a landblock is loaded, the client puts two objects per cell into the landblock's
physics world:

- the solid body of §5.3 (only when the cell has a physics mesh);
- a **phantom** (a shape that detects overlaps but blocks nothing) whose shape is the
  convex hull of the cell's hull vertices (§5.6), placed with the cell's frame and tagged
  with the cell's number.

The client's lookup of the cell that contains a landblock-frame point *p*:

1. If |*p*|² > 7,593,750 (about 2,756 units from the landblock origin), no cell.
2. Ask the physics world's broad phase for the cell phantoms whose bounds overlap the cube
   [*p* − 0.05, *p* + 0.05] on all three axes.
3. Take the candidates one by one (last returned first). Map *p* into the candidate's cell
   frame and walk its cell volume (§5.4): inside when every plane gives at least −0.0002.
4. The first candidate that contains *p* names the cell. If none does, *p* is **outdoors**
   (no cell).

So a point enters a cell from outdoors by containment alone: no portal is crossed (no
portal leads outside, §5.8) and none is needed. Portals link cells to each other. When two
cells contain the same point the result depends on the broad phase's order; Tessera
Decision: take the lowest cell number.

**Tessera Decisions for the lookup** (2026-10-07; the first raised by the implementation on
2026-10-06):

- Two landblock-info cells carry no x, y, z offset (`landblock.md` §4.4 OPEN). They cannot be
  placed and are left out of the lookup and of collision (104,269 of 104,271 cells placed);
  the census runs above count all of them.
- Step 2's candidate test uses the axis-aligned box of the placed hull vertices in place of
  the phantom's own broad-phase bounds (the hull plus any shape radius and broad-phase
  margin, not traced). The candidate step only filters and step 3 decides, so the two can
  differ only for points within a few centimetres of a hull's boundary. A cell whose record
  has no hull vertex is never a candidate (no record has fewer than 8).
- The plane test is evaluated in double precision, so the 2 × 10⁻⁴ tolerance holds for cells
  placed far from the landblock origin.
- Of two cells with the same number in one record (never observed), the earlier one wins.
- Across landblocks: each landblock's cells are tested with the point in that landblock's
  frame, landblocks in ascending (x, y) order, and the first landblock that finds a cell
  answers. Which landblock (or landblocks) the client asks for a point near a landblock
  edge was not traced.

> **OPEN:** The client runs this lookup when it places entities (its placement routine).
> Whether a moving character's cell is re-evaluated with it after every step, or only when
> the character's own phantom starts or stops overlapping a cell phantom, was not traced.
> Tessera Decision: run the lookup on the mover's position after every step (§12.3 step 8).

---

<a id="s6"></a>
## 6. Terrain and ceiling collision

<a id="s6-1"></a>
### 6.1 Geometry (Confirmed)

Terrain collision triangles are **the render triangulation** of `landblock.md` §3.11: two
triangles per 5 × 5 quad, the diagonal chosen by bit 7 of the vertex-flag grid. Every point
query of every triangle of every terrain tree (58,546 codes, 119,902,208 triangles,
centroid and corners) reaches the triangle's key under this triangulation, with heights from
`landblock.md` §3.4.

BIN: for every landblock the client builds this mesh before anything else uses the
landblock's terrain: 1,089 vertices, vertex 33 · j + i at (5i, 5j, stored height × 2,000 /
65,535; `terrain-texturing.md` §13), and 2,048 triangles in the key order of §6.2, each
counter-clockwise from above, with the stored corner order of the table below. The mesh is
built from the heights alone: the surface layers are not consulted and no triangle is ever
left out (§6.5).

**Stored corner order (BIN, C43).** For the quad with south-west sample (*i*, *j*), corners
written as grid samples (vertex 33 · *j* + *i*). "Set" means the quad's diagonal bit is set:
bit 7 of the vertex-flag byte for terrain, bit 0 for the ceiling (§6.4); the diagonal then
runs south-west/north-east.

| Mesh | Diagonal bit | Key *base* | Key *base* + 1 |
|---|---|---|---|
| Terrain | set | (*i*, *j* + 1), (*i*, *j*), (*i* + 1, *j* + 1) | (*i* + 1, *j* + 1), (*i*, *j*), (*i* + 1, *j*) |
| Terrain | clear | (*i*, *j* + 1), (*i*, *j*), (*i* + 1, *j*) | (*i* + 1, *j*), (*i* + 1, *j* + 1), (*i*, *j* + 1) |
| Ceiling | set | (*i* + 1, *j* + 1), (*i*, *j*), (*i*, *j* + 1) | (*i* + 1, *j*), (*i*, *j*), (*i* + 1, *j* + 1) |
| Ceiling | clear | (*i* + 1, *j*), (*i*, *j*), (*i*, *j* + 1) | (*i*, *j* + 1), (*i* + 1, *j* + 1), (*i* + 1, *j*) |

Terrain triangles are counter-clockwise and ceiling triangles clockwise seen from above. Only
the winding matters to any rule in this spec: the face normal of §11.3 (the cross product of
the second and third corners minus the first) is the same for every rotation of a triangle's
corner list, so a reader may store the corners starting at any of them as long as the cyclic
order is kept.

The client's terrain height and terrain plane queries at a point pick one triangle of this
mesh by the rules of `scenery.md` §7.4 step 8 (BIN there, S25), which are not always the
triangle under the point: on the diagonal of a bit-7-set quad they take the north-west
triangle, and in the band 4 < fx + fy ≤ 5 (units) of a bit-7-clear quad the north-east one.
Collision itself uses the triangles, so this matters only to code that reproduces the
client's point queries (scenery placement); Tessera's controller (§12) collides with the
triangles. The edge cases (i or j of 32 taken as 31; i or j outside 0 … 32 gives no
triangle and the plane n = (0, 0, 0), d = 0) are those of `scenery.md` §7.4 step 8. Applying
the same point rule to a ceiling mesh, with bit 0 in place of bit 7, is a Tessera Decision for
tools; the client was not seen to query ceilings by point.

<a id="s6-2"></a>
### 6.2 Triangle numbering (Confirmed)

For the quad with south-west corner sample (*i*, *j*) (*i* along X, *j* along Y,
0 ≤ *i*, *j* ≤ 31), let *base* = 2 · (32 · *j* + *i*). The quad's two triangles have keys
*base* and *base* + 1:

- **key *base*** is the triangle on the **west** side of the quad's diagonal: with the
  south-west/north-east diagonal (bit 7 set) it is the triangle with corners (*i*, *j*),
  (*i* + 1, *j* + 1), (*i*, *j* + 1); with the south-east/north-west diagonal it is
  (*i*, *j*), (*i* + 1, *j*), (*i*, *j* + 1);
- **key *base* + 1** is the other triangle.

Note the order: quads are numbered **Y-major** (row *j* first) although the height grid is
stored X-major.

<a id="s6-3"></a>
### 6.3 Blocks without a terrain tree (Confirmed census; client use BIN, in part)

331,604 landblock-data records carry no terrain tree, including many with varied heights
(region 1: 29,479 without against 3,830 with). The tree is therefore not required to
collide with terrain. Tessera always collides with the triangulated height grid of §6.1.

BIN: the client builds the mesh of §6.1 for every landblock, with or without a stored
tree. When the record stores a tree, the client uses it as the mesh's bounding-volume code.
When it does not, the mesh-building routine builds a tree from the same mesh at load time
**only when its caller asks for it** (fifth pass): the landblock loader that builds the
world's terrain and ceiling meshes does not ask (for either), and a second load path asks
or not according to its own caller, which was not traced. So on the loader's path a
landblock without a stored tree has a mesh **without** a tree.

BIN (fourth pass, completed in the fifth): the client then makes the landblock's physics
body from that mesh by one of three cases.

| Mesh has a tree (stored, or built at load time) | Height-field switch | Physics body |
|---|---|---|
| yes | either | the mesh of §6.1 with that tree |
| no | set | a **sampled height field** of the 33 × 33 grid (terrain or ceiling samples), wrapped as a triangle collection with the 0.05 m convex radius of §8.2 and given a bounding-volume tree of its own |
| no | not set | the mesh of §6.1 with **no** tree (every triangle is a candidate in every query) |

The height-field switch is one global state value of the client compared with one
constant; its value at run time was not determined. The mesh's triangles are always those
of §6.1; how the height field splits its quads was not traced.

> **OPEN (Q4):** the value of the height-field switch at run time, the height field's quad
> split (expected to follow §6.1 and §6.4), and whether the second load path (which may
> build a tree at load time) ever serves the physics body. Tessera does not need the answer:
> it builds its own acceleration structure over the triangles of §6.1 (§3.8), which gives
> the same triangles in every case except, possibly, the height field's split.

<a id="s6-4"></a>
### 6.4 Ceiling trees and ceiling collision (Confirmed)

A ceiling tree uses the same numbering (§6.2) over the **ceiling grid** heights
(`landblock.md` §3.5), all 2,048 grid triangles included, also those whose samples are 0
("no ceiling"; such triangles sit at height 0).

**Diagonal (Confirmed).** The diagonal of a ceiling quad (i, j) is given by **bit 0** of
the vertex-flag byte of sample (i, j) (`landblock.md` §3.6): set → south-west/north-east,
clear → south-east/north-west. BIN: the client's mesh builder tests bit 0 for the ceiling
and bit 7 for the terrain. OBS: all 5,310,848 stored ceiling-chunk triangles (2,068 chunks)
use the bit-0 diagonal; bit 7 agrees for only 4,999,408. All of them are **clockwise** from
above (they face down); the client's ceiling mesh has the same quad split as the terrain
for a given diagonal, with the corners of each triangle in reverse order (stored order: §6.1
table, BIN, C43).

BIN: the client builds the ceiling collision mesh for every landblock with a ceiling
(`has_ceiling`, `landblock.md` §3.1 part 3), from all 1,089 ceiling samples, the zeros
included; triangles over zero samples therefore exist and lie at height 0, normally far
below the terrain. This answers the ceiling-diagonal OPEN of `landblock.md` §3.5.

<a id="s6-5"></a>
### 6.5 Terrain holes and collision (Confirmed census; client rule BIN)

Grid triangles that no surface layer covers are not drawn (`terrain-texturing.md` §4.5).
Facts:

- OBS (all 390,150 landblock-data records; holes counted, as in `terrain-texturing.md`
  §4.5, in the 86,726 landblocks whose terrain chunk has layers; a `whole` layer covers
  every triangle): 2,368 landblocks have such triangles, 2,750,134 triangles in all. **Every** one of these
  landblocks has a ceiling; no landblock without a ceiling has any. (Landblocks whose
  terrain chunk has no layers at all draw no terrain; they are not counted here.)
- OBS: **no** such triangle lies under an open cave. Comparing the mean of a triangle's
  three ceiling samples with the mean of its three terrain heights: 2,336,479 have the
  ceiling at or below the terrain (the terrain there is buried in the rock between the
  floor and the ceiling surface), and 413,655 have all three ceiling samples 0 (no ceiling
  at all; in 84 of the 2,368 landblocks no layer covers any triangle). Drawn triangles in
  the same landblocks, by contrast, have the ceiling above the terrain in 1,954,445 cases.
- BIN: the client's terrain collision mesh contains every grid triangle (§6.1), and every
  stored terrain tree reports all 2,048 keys (CO7).

So the original does **not** cut holes in terrain collision, and a hole is not an opening
a walker passes through: where a cave floor is drawn the terrain is solid; where the
terrain is hidden under a ceiling it lies inside rock (the ceiling surface comes down
through it). Tessera collides with all 2,048 triangles of every landblock, as the original
does. This corrects the "cave mouths" reading of `terrain-texturing.md` §4.5 (§15).

> **OPEN:** what, if anything, covers the 413,655 undrawn terrain triangles whose three
> ceiling samples are all 0 (no ceiling over them), and whether a walker can reach them.
> Not checked: whether cells (§5), static entities or neighbouring geometry lie over them.
> Tessera's collision does not depend on the answer (all triangles collide, as in the
> original); only what is drawn there does.

<a id="s6-6"></a>
### 6.6 Terrain triangles near a point (the client's radius query, BIN)

Scenery placement (`scenery.md` §7.4 step 13) collects the terrain triangles of one
landblock that lie near a point *p* = (x, y, z) (block-local units) within a horizontal
radius *r*. The client's rule, including a quirk that must be reproduced for identical
placement:

1. If x − r ≥ 160, y − r ≥ 160, x + r < 0 or y + r < 0, collect nothing.
2. Cell range: i from ⌊(x − r) / 5⌋ (0 when x − r ≤ 0) to ⌊(x + r) / 5⌋ (31 when
   x + r ≥ 159.99998), and j likewise with y.
3. For each cell (i, j) of the range (i outer, j inner), skip the cell when the squared
   horizontal distance from *p* to the square [5i, 5i + 5] × [5j, 5j + 5] exceeds r².
4. If the squared horizontal distance from *p* to the segment from (5i, 5j) to
   (5i + 5, 5j + 5) is at most r², collect both triangles of the cell, keys
   2 · (32 · j + i) and 2 · (32 · j + i) + 1.
5. Otherwise let s = ((y − 5j) − (x − 5i)) / √2 (the signed distance of *p* from that
   segment's line, positive on its north-west side). If s < −0.0002 (*p* on the
   south-east side), collect key 2 · (32 · j + i); otherwise collect key
   2 · (32 · j + i) + 1.

The collected triangles are those of §6.1 with these keys, each with its plane (unit
normal, n.z > 0). Steps 4–5 always use the south-west/north-east diagonal, whatever bit 7
says, and for a quad with that diagonal key 2 · (32 · j + i) is the north-west triangle
(§6.2): when the circle does not reach the diagonal, the client collects the triangle on
the **far** side of the diagonal from *p*, not the one under it. Implementations that must
match the original's placement reproduce this rule; elsewhere Tessera collects the
triangles that actually intersect the circle (Tessera Decision).

The keys come out in the order of step 3 (i outer, j inner), *base* before *base* + 1 in
step 4. A non-finite input or a negative radius collects nothing (Tessera Decision). Tessera
evaluates steps 1–5 in single precision, with 159.99998 and 0.0002 as single-precision
values (Tessera Decision; the precision of the client's arithmetic was not re-checked, and
only points within about 10⁻⁵ of a decision boundary could differ).

---

<a id="s7"></a>
## 7. Per-level collision data of render meshes

<a id="s7-1"></a>
### 7.1 What the tree indexes (Confirmed; corrects `mesh.md` §10.2)

The tree block of a mesh level indexes the level's **render triangles** (§3.5:
draw position · 2²⁰ + triangle index in that draw's index buffer), **not** the triangles of
the level's collision mesh (`mesh.md` §9). Evidence: keys exceed the collision-mesh
triangle count in 1,235 of 3,612 sampled levels, levels with an **empty** collision mesh
carry trees with hundreds of keys, and in 70,944 of the 70,956 coded 0x06 levels every key is
a valid (draw, triangle) pair of the level's render geometry (the other 12 are §3.5's
renumbered levels). The origin and scale follow §3.2 over the render triangles in the code
in 70,811 levels. Point queries at the centroid and corners of every coded render triangle
(330.6 million queries) report the triangle in all but 21,522 (0.007 %), which lie in the
levels whose keys or box are off.

<a id="s7-2"></a>
### 7.2 Coverage (Confirmed)

Small render triangles are often left out of the code. Level 0 of the 0x06 records: of
74,700,656 render triangles in coded levels, 9,628,796 are not in the code: 6,894,659 with
area below 10⁻⁴ m², 2,732,613 below 10⁻³, 277 below 10⁻² and 1,247 larger. Later levels:
1,949,651 of 19,530,019, of which 4,982 have 10⁻² m² or more. Duplicate triangles of
double-sided render geometry are kept.

> **OPEN:** What the original uses per-level collision meshes and trees for. Candidates:
> mouse picking, camera and line-of-sight tests against detailed geometry. Character
> movement uses the 0x66 mesh, setup volumes, cell physics meshes and terrain (§2).

Tessera Decision: when a per-level collision mesh is used at all, it is **detail** geometry
that only camera, picking and line-of-sight queries see; a mover never collides with it.

---

<a id="s8"></a>
## 8. Collision volumes of setups

`setup.md` §6.4 gives the box, sphere and capsule layouts and §6.5 the collision mesh DID
and the physics bounds; §4 above completes the 0x66 record. Facts added here (Confirmed):

- Volume census over 28,555 setups: mesh only 16,812, sphere only 1,910, box only 990,
  capsule only 715, none 8,117, mixtures 11 (`setup.md` §6.5). The setups that have both a
  0x66 mesh and volumes are named by 119 placed Physics SEDs (OBS, fourth pass).
- The capsule is vertical: it runs from its lower cap centre straight up by `length`
  (`setup.md` §6.4); every playable character setup has exactly one capsule and nothing
  else (§10.6).
- Moving entities use **one** convex shape (a single box, capsule or sphere) (BIN: the client
  rejects mobile entities with any other collision as a configuration error). Static objects
  may use the mesh.

<a id="s8-1"></a>
### 8.1 One shape per setup; scale (BIN, C31)

The client turns a setup's collision into **one** shape: the 0x66 mesh (when the setup names
one) and every box, sphere and capsule, each as a part of a compound shape (a single part is
used on its own). Every part collides; a setup with a mesh and volumes blocks with both.

**Physics scale (BIN, C42).** The physics component of an entity (a static, a generated object
or a mover) computes one **uniform** scale when it is set up and whenever the scale
properties change:

1. Read `Physics_AdjustableScale` and `Model_Scale` (property 0x3C3) of the entity, each used
   only when the value found is a vector (C41), else (1, 1, 1).
2. For each of the two vectors whose x differs from its y or its z by more than 0.0002, the
   client logs a warning and uses its z for every axis.
3. *s* = (z of `Physics_AdjustableScale`) × (z of `Model_Scale`). A product that is not finite
   becomes 1; a product below 0.0002 is logged and becomes 1. There is no upper limit.
4. The scale applied is (*s*, *s*, *s*). It replaces the previous scale (initially 1) only when
   it differs from it by more than 0.0002 in some component.

`Render_AdjustableScale` and the template factors of `appearance.md` §6.1 are not part of the
physics scale. OBS (seventh pass): no landblock-info SED carries `Model_Scale`
(`appearance.md` §6.1 records that no 0x47 record does either), and every SED's
`Physics_AdjustableScale` is uniform within 0.0002, so for placed objects *s* is the
`Physics_AdjustableScale` value.

That scale reaches each part as follows (*s*ₓ = *s*ᵧ = *s*_z = *s* by the rule above; the
per-axis entries describe the part builders, which never receive a non-uniform scale from
this path). A scale counts as 1 when every component lies within 0.0002 of 1; the client
then reuses one unscaled shape per setup.

| Part | Scaled shape |
|---|---|
| Box | Half extents (max − min) / 2 multiplied by *s* per axis, in the box's own frame; the box centre and the frame's translation multiplied by *s* per axis; the frame's rotation unchanged |
| Sphere | Centre multiplied by *s* per axis; radius multiplied by *s*_z |
| Capsule | Both cap centres multiplied by *s* per axis; radius multiplied by *s*_z |
| 0x66 mesh | Scaled **uniformly** by *s*ₓ rounded down to a multiple of 0.0002 (the client keeps one scaled mesh per record and rounded scale); *s*ᵧ and *s*_z are not used |

So in the client every part is multiplied by *s* (the mesh by *s* rounded down to a multiple of
0.0002).

<a id="s8-2"></a>
### 8.2 Convex radius (BIN, C32)

The physics middleware gives each box and each triangle of a triangle mesh a **convex
radius** of 0.05 m: the surface that a mover touches lies 0.05 m outside the stored box or
triangle, with rounded edges and corners. The client builds boxes, 0x66 meshes, the
landblock's terrain and ceiling collision (§6.1, §6.3, §6.4) and cell physics meshes (§5.3)
with this radius: a cell's physics mesh goes through the same mesh-shape builder as a 0x66
mesh, unscaled, with its stored tree (BIN, sixth pass, C32). The hull phantom of a cell
(§5.8.1) is built with the same radius; the point-in-cell test of §5.8.1 uses the planes,
not the phantom's surface. Spheres and capsules have none beyond their own radius.

A box is a solid convex shape: a mover whose shape overlaps it is pushed out of it, not held
by its inner faces.

<a id="s8-3"></a>
### 8.3 Tessera's representation (Decision)

- A setup's template holds the 0x66 triangles (when the mesh can be read), then 12 triangles
  per box (two per face), then its spheres and capsules. All parts collide (§8.1). When the
  0x66 record cannot be read, the volumes alone collide and the object is reported as
  incomplete.
- Boxes are solid (§8.2): a capsule whose core is inside a box gets one contact, for the face
  it leaves by most easily, with the penetration depth.
- Scale (changed 2026-10-08): Tessera uses the client's physics scale of §8.1, the uniform *s*
  from the z components with the fallbacks of steps 2–4 (the upper limit of 10⁴ of §9.3
  stays as a hostile-input bound, decision 0013). Every part is multiplied by *s*; Tessera
  does not round a mesh's scale down to a multiple of 0.0002 (a difference below 0.0002
  relative). The released per-axis reading gives the same result for every placed object of
  this install (all uniform, C42).
- No convex radius: Tessera collides with the bare triangles and boxes, with a contact offset
  of its own (§12.4 item (w)). Every surface a Tessera mover touches therefore lies 0.05 m
  closer to the stored geometry than in the original.

**Answer to the implementer OPEN of 2026-10-07 (2026-10-08).** No client fact is missing for
this section: the radius applies to every triangle and box body, cells included (§8.2), and
the client never applies a non-uniform scale (§8.1), so the non-uniform part rules have no
effect and Tessera follows the uniform scale (bullet above). Whether Tessera adopts the
0.05 m radius is a Tessera Decision, not a question about the client: the bullets above stand
(no convex radius, contact offset (w)) until the owner measurements of Q9 or a decision to
adopt it; adopting it would mean colliding the mover with triangles and boxes as a capsule
with the same axis and a radius 0.05 m larger, and leaving contacts with spheres and capsules
as they are.

> **OPEN (implementer, 2026-10-08, remediation pass):** `tessera-physics` now places statics
> with the physics scale of §8.1 (C42: z of `Physics_AdjustableScale` × z of `Model_Scale`,
> SED-then-chain lookup, vectors only, the fallbacks of steps 2–4 and Tessera's upper bound
> 10⁴) and scales the walker's capsule by the avatar entity's physics scale (§10.6). For
> **generated objects** (§9.4) Tessera still places the shape with the per-axis scale of
> the scenery transform (`scenery.md` §7.4, uniform in this install). §8.1 says the physics
> component of a generated object computes its scale from its `Physics_AdjustableScale` and
> `Model_Scale`, but not whether the scenery scale reaches the object as one of those
> properties (or as a third factor). Conservative choice until stated: the scenery
> transform's scale, as before; with uniform scales the result is the same.

---

<a id="s9"></a>
## 9. Per-instance collision flags

<a id="s9-1"></a>
### 9.1 Properties (Confirmed census)

Resolved like the draw flags of `entity-descriptor.md` §9: the SED's own value wins, then
the 0x47 chain from the named object upward. Each 0x47 level is read with the descriptor
lookup of `scenery.md` S31 (BIN, C30): its 0x78 extra sets in list order, each later set
overriding the earlier ones, and its embedded set on top. A SED has no extra sets.

| Property | Type | Where (count) | Meaning |
|---|---|---|---|
| `Physics_NoPhysics` | boolean | SEDs: true 76,508, false 19,938; 0x47: true 777, false 21 | true: the instance has no physics shape (no collision). BIN: a mover with it set has no shape. |
| `Physics_AdjustableScale` | vector | every SED (equal to the render scale, `entity-descriptor.md` §7) | Scales the collision shape |
| `Physics_EtherealityType` | bitfield, mapper 0x230000BA | SEDs 7,276; 0x47 87; 0x79 43,321 | The categories this object belongs to (§9.2) |
| `Physics_EtherealToType` | bitfield, mapper 0x230000BA | SEDs 26,095; 0x47 2,639; 0x79 43,370 | The categories of movers that pass through this object (§9.2) |
| `Physics_PlacementEtherealToType` | bitfield, same mapper | SEDs 3,292; 0x47 24; 0x79 43,384 | As above, for placement tests |
| `Physics_AlwaysUncontrolledSurface` | boolean (default false) | no 0x47 record (embedded or extra sets) | The object never counts as support (§11.3) |
| `Physics_SurfacePropertiesToApply` | DID (0x78) | 0x47 52 embedded, 20,216 with extra sets; 0x01 655 | Surface material set: `Base_Surface_Type`, `Content_IAmMadeOf`, `SND_Footstep_Surfaces` |
| `Physics_CanLedgeClimb` | boolean (default true) | 0x47 28; 0x01 53 | Ledge climbing allowed |
| `Entity_Clearance_*` boxes and cylinders | struct arrays | SEDs 510,356 boxes, 144,071 cylinders | Placement and path clearance; **not** movement collision (Probable) |

The 0x47 counts of the table are over each record's embedded set, and also with its extra
sets where that differs (OBS fifth pass, 2026-10-07: 20,274 of the 40,643 records name extra
sets; in 20,216 of them the extra sets carry a §9.1 property, always
`Physics_SurfacePropertiesToApply`; every other column is the same with or without extra sets:
`Physics_NoPhysics` 798, `Physics_AdjustableScale` 1,288, `Physics_EtherealityType` 87,
`Physics_EtherealToType` 2,639, `Physics_CanLedgeClimb` 28, `Entity_CreationType` 40,091).
This answers the implementation-review OPEN of 2026-10-06: the physics lookup includes extra
sets.

The ethereality mapper's labels (keys 1–30): Default, Camera, Cosmetic, LineOfSight,
DarkSidePlayer, Corpse, OpenDoor, Missile, ClosedDoor, Creature, LightSidePlayer, NPC,
PlayerMadeHotspot, Admin, PlayerImpassable, Player (keys 0x11–0x1E for the last 14, keys 1–2
for the first two; bit *k* − 1 stands for key *k*, `property-system.md` §7.3). Examples: the
player entities of all species carry EtherealTo = {Camera}; 2,302 creature and NPC entities
carry {Default, Camera, Corpse, OpenDoor, Missile, ClosedDoor, Creature, NPC, Admin, Player}.

**Value types (BIN, C41; answers the two implementer OPENs of 2026-10-07).** The property
dictionary gives every property one type, and every stored value of it is decoded with that
type (`property-system.md` §1, §5.3). In this install `Physics_NoPhysics`,
`Physics_AlwaysUncontrolledSurface` and `Physics_CanLedgeClimb` are Boolean (type 21) and the
three ethereality properties are Bitfield32 (type 18) (OBS). The client's readers:

- A boolean flag is true only when the value found is a Boolean and true (stored byte 1,
  `property-system.md` §4.5). A value of any other type reads as false (traced for `Physics_NoPhysics` and
  `Physics_AlwaysUncontrolledSurface`; the reader of `Physics_CanLedgeClimb` was not traced).
- An ethereality value is used only when it is a Bitfield32 (type 18), as its 32 bits. A value
  of any other type, the 64-bit and variable-width bitfields (types 14 and 22) included, reads
  as no categories.
- The lookup stops at the first level that holds the property, whatever the value's type; it
  does not go on along the chain.

Since all levels of one property share the dictionary's type, stopping and reading false (the
client) and treating the value as absent and going on (Tessera's Decision) give the same result
on any data decoded with the dictionary. **Tessera rule:** for the booleans the implementer's
Decision is confirmed (only a Boolean counts). For the ethereality properties Tessera follows
the client (changed 2026-10-08): only a Bitfield32 value counts, as its 32 bits; the released
reading of 64-bit and variable-width bitfields differs from the client only under a dictionary
that declares these properties with another type.

<a id="s9-2"></a>
### 9.2 Pass-through rule (Probable)

An object does not block a mover when the object's effective `Physics_EtherealToType`
contains a category the mover belongs to. A player character belongs to **Player**
(key 0x1E, bit 29); the camera to **Camera** (bit 1). This reading explains the data: player
entities are ethereal to the camera only, so the camera passes through players; creatures
and NPCs are ethereal to players and to each other, so nobody is blocked by them.

> **OPEN:** The exact test (only the object's EtherealTo against the mover's Ethereality, or
> also the reverse), the default categories of an entity without `Physics_EtherealityType`,
> and the meaning of `Default` (bit 0).

Tessera Decisions (2026-10-07): an object whose effective `Physics_EtherealToType` contains
Player still gets a collision body, so movers of other categories can collide with it; the
pass-through test is made per query with the mover's categories. The local avatar's
categories are {Player}; the camera's are {Camera}, and the camera also sees the detail
geometry of §7. An object without `Physics_EtherealToType` is ethereal to nothing.

<a id="s9-3"></a>
### 9.3 Which placed objects block a walker

**Census (OBS, C27).** The landblock-info records hold 2,380,699 Physics SEDs (type 7);
2,380,675 of them have a position. Classified by the resolved chain (§9.1), the setup's
collision (§8; "mesh" includes the setups with a mesh and volumes) and the plain
draw-suppression of `entity-descriptor.md` §9.2 (a SED is undrawn when the first value along
its chain — the SED's own set, then the 0x47 chain — of `Render_Draw` or `Render_DrawMeshes`
is false, or of `Render_NeverAddToScene` is true; not the SED-over-object precedence of E22):

| Setup collision | NoPhysics | Drawn | Ethereal to Player | SEDs |
|---|---|---|---|---|
| mesh | false | yes | no | 1,409,552 |
| none | false | yes | no | 400,661 |
| none | false | no | no | 177,375 |
| volumes | false | yes | no | 143,857 |
| **mesh** | **false** | **no** | **no** | **82,139** (invisible walls and blockers) |
| mesh | true | yes | no | 68,075 |
| volumes | true | yes | either | 44,144 |
| volumes | false | no | no | 15,549 |
| volumes | false | yes | yes | 15,004 |
| other combinations | | | | 24,319 |
| **all with a position** | | | | **2,380,675** |

This table is a **data census**: it applies neither the content-layer test nor the
creation-type filter of the rule below. (The revision of 2026-10-06 gave 1,409,434, 68,074
and 24,462 in three rows; it counted the 119 SEDs whose setup has a mesh and volumes outside
the "mesh" rows and summed to 2,380,699, the count with the 24 SEDs that have no position.)

**What becomes a collision body (BIN, C25, C29).** The client creates a landblock-info static,
and so its collision body, only on its static-creation path, and that path takes a Physics SED
only when:

1. it has a position (the census above);
2. it passed the content-layer test of `entity-descriptor.md` §4.5.1 when the landblock was
   loaded, with the world's active content layer *L* and the landblock copy's instance
   number *I* (offline default: the shared world, *L* = 1 and *I* = 0);
3. its creation type is not 0 (`Invalid`: the loader rejects the SED as an entity with
   errors) and not of the Dynamic class, UniqueDynamicMobile (7) or UniqueDynamicImmobile
   (8). The value tested is the SED's **own** `Entity_CreationType` when its property set has
   one, else the value found along its 0x47 chain: the lookup order and the classes are those
   of `entity-descriptor.md` §4.9 (E24). When neither the SED's own set nor any level of its
   chain has `Entity_CreationType`, or the value found is not an enum (type 8) or property-ID
   (type 10) value, the value is 0 and the SED is rejected exactly like an explicit 0 (BIN, fifth pass: the reader
   returns 0 when the lookup finds nothing; `entity-descriptor.md` §4.9 and E23). No Physics
   SED of this install reaches this case: every one has a value (`entity-descriptor.md`
   §4.9 census). Statics of types 7 and 8 that passed the content-layer test are handed to
   a separate list of dynamic entities, which the client places with its own rules;
   `entity-descriptor.md` §4.9 records that the client does not create them from the data
   (in the original they come from the server).

Item 2's layer list (`Entity_ContentLayers`) and the created body's scale
(`Physics_AdjustableScale`, §8.1) are read with the same lookup as the creation type: the
SED's own set first, then its 0x47 chain (`entity-descriptor.md` §4.9, E24). In this install
the chain never changes either value: no 0x47 object carries `Entity_ContentLayers` (E24),
and every Physics SED carries its own `Physics_AdjustableScale` (§9.1).

A created static then has collision when its effective `Physics_NoPhysics` is not true and
its setup has a mesh or volumes (§8). Its effective `Physics_EtherealToType` decides, per
mover, whether it blocks (§9.2): a static blocks a walker exactly when it has collision and
its `Physics_EtherealToType` does not contain Player. The draw flags play **no** part.

**Tessera rule.** Tessera applies items 1–3 and the collision test above; statics of types
7 and 8 get no body. In this install no Physics SED has an effective creation type of 7 or 8
(OBS, C29): 1,602 placed Physics SEDs have 7 or 8 on their 0x47 chain, and every one of them
carries its own value (5 in 883, 9 in 412, 6 in 262, 3 in 45). One placed Physics SED has an
effective creation type of 0 (its own set says 0 while its object says 3; `entity-descriptor.md`
§4.9): a drawn mesh blocker in the census, which the client does not create.

**Implementation status and reader details (answers the implementer OPENs of 2026-10-07;
BIN, C41, C42).** `tessera-physics` reads `Entity_CreationType`, `Entity_ContentLayers` and
`Physics_AdjustableScale` with the SED-then-chain lookup (0x47 extra sets included, as in
§9.1), refuses creation types 0, 7 and 8 and counts no value anywhere as 0; the earlier
chain-only reading is gone. The four points the implementer left open, against the client:

1. **Creation-type value types (changed 2026-10-08).** The client accepts only an enum
   (type 8) or property-ID (type 10) value and uses its 32-bit value as stored, so a value
   above the known types falls in the class the value gives by `entity-descriptor.md` §4.9.
   A 32-bit integer (type 15), a 64-bit integer (type 19), a bitfield or any other type
   counts as 0, so the SED is rejected. The implementer's reading of type 15 as a creation
   type is not the client's and must change. The dictionary declares `Entity_CreationType`
   as an enum (type 8) (OBS), so every stored value is accepted and the census does not
   change.
2. **Scale.** The client uses `Physics_AdjustableScale` only when the value found is a vector
   (type 6), else (1, 1, 1); it does not go on along the chain (confirmed). Its range rule is
   that of §8.1 steps 2–4, not 10⁻⁴ to 10⁴: a non-uniform vector uses its z, a non-finite
   result or one below 0.0002 becomes 1, and there is no upper limit. **Tessera rule
   (changed 2026-10-08):** §8.1 steps 1–4, plus Tessera's own upper bound: a result above 10⁴
   becomes 1 (hostile input, decision 0013; no SED comes near it, `entity-descriptor.md` ED6).
3. **Content layers (confirmed).** When the value found is not an array (type 12), the
   client's content-layer test passes, exactly as with no list; an empty array passes too
   (`entity-descriptor.md` §4.5.1 owns the rest of the test).
4. **No dictionary entry (confirmed).** Without a definition no value of `Entity_CreationType`
   can be decoded, the lookup finds nothing, the value is 0 and every landblock-info static is
   refused, as the implementer chose.

**Census vs. run-time count (OBS, 2,380,675 Physics SEDs with a position).**

| Set | Blockers | of them undrawn | of those, mesh |
|---|---|---|---|
| Data census (table above; no run-time filter) | 1,651,097 | 97,688 | 82,139 |
| Run-time, shared world (content-layer test, *L* = 1, *I* = 0) | 1,607,212 | 88,749 | 73,509 |
| ...and the creation-type filter (SED's own value first; 0, 7 and 8 dropped) | 1,607,211 | 88,749 | 73,509 |

So 8,939 undrawn blockers (8,630 of them mesh) name only inactive content layers and are
absent for a character in the shared world, and the creation-type filter removes the one
drawn blocker of creation type 0. (Read from the 0x47 chain alone and dropping only 7 and 8,
the filter would instead remove 901 blockers, 1 of them undrawn: 1,606,311 / 88,748 /
73,508. That reading is not the client's.)

<a id="s9-4"></a>
### 9.4 Procedurally generated scenery

Objects placed by `scenery.md` §7 are not SEDs. BIN (C25):

- After generation the client turns every generated object into the same static-entity
  creation request that it uses for landblock-info statics (request kind 4 instead of 2),
  carrying the object's 0x47 DID, position, rotation, scale and `Entity_CreationType`, with
  an instance ID synthesized from the landblock and the object's index in the generated
  list (`scenery.md` §7.6; the layout is below). From there, generated objects and statics
  take the same path.
- **Instance ID of a generated object (BIN, fifth pass, C38).** With *k* the entry's
  0-based position in the landblock's generated list (`scenery.md` §7.6; marker entries
  count, although they create nothing), the 64-bit instance ID has the layout of
  `entity-descriptor.md` §4.2 with these values:

  | Bits | Content |
  |---|---|
  | 56–63 | Kind byte **5** |
  | 52–55 | Region *R* of the landblock |
  | 44–51 | Landblock X |
  | 36–43 | Landblock Y |
  | 24–35 | Instance number *I* of the landblock copy (0 for the shared world) |
  | 0–23 | *k* |

  When *k* ≥ 2²⁴ the client uses its invalid instance ID instead (a run-time value; never
  reached: a generated list is far shorter). Kind 5 is among the kinds whose bits 24–35 the
  client rewrites for an instanced copy (`entity-descriptor.md` §4.2), and it does so here
  with the landblock's own instance number, so the copy's IDs carry *I*. Unlike SED serials
  (which start at 1), *k* starts at 0.
- Landblock-info statics pass two filters on that path that generated objects do not
  (§9.3 items 2–3): the content-layer test and the creation-type filter (types 7 and 8 go to
  the dynamic list instead). A generated object's request carries the creation type found
  along its 0x47 chain (`scenery.md` §9.2 rule 2), but no filter reads it: generated objects
  of every creation type are created (BIN; `scenery.md` S40).
- Frill objects (`scenery.md` §8) are never entities and never collide.
- **Order of building** for a landblock: (1) the terrain and ceiling collision meshes
  (§6.1, §6.4); (2) the static list of landblock-info entities; (3) scenery generation,
  which reads only the terrain mesh (plane and radius queries, §6.6) and the statics'
  clearance volumes, never the collision of objects, cells or other scenery; (4) the
  creation of all statics, generated objects included, with their collision.

**Collision rule for generated objects (BIN for the creation; Tessera Decision for the
body, following §9.3):** a generated object gets a collision body when its effective
`Physics_NoPhysics` (resolved along its 0x47 chain, extra sets included; there is no
SED-level value) is not true and its setup has a 0x66 mesh or volumes; it blocks a walker
when, in addition, its effective `Physics_EtherealToType` does not contain Player. Its shape
is the setup's (§8), at the generated position and rotation, scaled by the generated scale
(uniform in this install). No content-layer, creation-type or position filter applies.

Census (OBS, 2026-10-05; 0x47 records resolved with `setup.md` §3.1). The two populations
are those of `scenery.md` §5.4: **group objects** are reachable through the groups of the
2,472 scenes named by scene-choice tables and the scenes those groups reference; **frill
objects** are reachable through the frill layer of any scene the frill pass can instantiate
(`scenery.md` §8.2), which includes the frill layers of scenes reached through scene
references. An object can be in both (235 are).

| Setup collision | `Physics_NoPhysics` | Group objects (2,981) | Frill objects (1,515) |
|---|---|---|---|
| 0x66 mesh | not true | 1,997 | 74 |
| 0x66 mesh | true | 182 | 11 |
| volumes | not true | 253 | 0 |
| volumes | true | 49 | 6 |
| none | either | 500 | 1,424 |

The frill column was 1,512 in the earlier revision because it counted only the frill layers
of the 2,472 named scenes; the 3 objects added by the wider count (OBS) are one with a 0x66
mesh and `Physics_NoPhysics` true and two without collision. Since frill objects never
collide, the difference does not change any rule.

24 group objects carry `Physics_EtherealToType`; 22 of those sets contain Player. Creation
types of the 1,997 meshed, physical group objects: SharedStaticImmobile 1,437,
UniqueStaticImmobile 524, SharedClientOnlyImmobile 36.

> **OPEN:** whether the client gives a physics body to generated objects of the client-only
> creation types (SharedClientOnlyImmobile and similar). They are created like the others
> (above, BIN); whether their creation includes a physics body was not traced. Tessera
> Decision: it does.

> **OPEN (implementation, `tessera-world` streaming, 2026-10-06):** (a) the instance ID
> that the client synthesizes for a generated object "from the landblock and the object's
> index in the generated list" has no formula here or in `scenery.md` §7.6. Offline
> nothing reads it; Tessera gives generated objects no instance ID and identifies them by
> landblock and list index. (b) "From there, generated objects and statics take the same
> path" leaves open whether a generated object whose 0x47 chain carries liquid monitors
> creates liquid volumes like a placed static (`water.md` §7.3 speaks of placed statics
> only). **Tessera Decision:** it does; the landblock's liquid boxes are those of its
> statics followed by those of its generated objects, in generated-list order. Please state
> both (BIN) and, for (b), count the scene objects whose chains carry liquid monitors.
>
> **Answer (2026-10-07, review revision):** (a) answered by the instance-ID table above
> (BIN, C38). Tessera may keep identifying generated objects by landblock and list index;
> an implementation that gives them IDs uses that layout. (b) **Count (OBS, C39):** none.
> The 3,402 scene records (prefix 0x02) name 4,261 distinct 0x47 objects, and not one of
> them carries a liquid monitor anywhere along its chain (checked with the water-volume
> rule of `water.md` §7.3). So in this install no generated object has a liquid volume, and
> the Decision above changes nothing. Whether the client would create such a volume for a
> generated object was not traced (BIN); the Decision stands for other installs.

---

<a id="s10"></a>
## 10. Movement data

<a id="s10-1"></a>
### 10.1 The movement parameter unit (Confirmed (BIN) and OBS)

Sixteen properties form one unit, registered together in the client and always stored
together in movement records. Units are metres, seconds and degrees.

| Property (interoperability name) | Type | Meaning (status) |
|---|---|---|
| `GenericPhysicsController_ForwardMoveSpeed` | float | Target speed forward, m/s (Probable) |
| `GenericPhysicsController_BackwardMoveSpeed` | float | Target speed backward, m/s (Probable) |
| `GenericPhysicsController_StrafeLeftMoveSpeed`, `…_StrafeRightMoveSpeed` | float | Target sideways speeds, m/s (Probable) |
| `GenericPhysicsController_FlyUpMoveSpeed`, `…_FlyDownMoveSpeed` | float | Vertical speeds when flying (0 for players) |
| `GenericPhysicsController_ControlAcceleration` | vector | Acceleration toward the target velocity, per local axis (forward, side, up) (Probable) |
| `GenericPhysicsController_VelocityDamper` | vector | Per-axis damping of velocity, axes (forward, side, up) (OPEN: factor per second or per step; Tessera reads a factor per second, §12.4 (b)) |
| `GenericPhysicsController_RampTime` | float | Seconds to reach the target speed (Probable) |
| `GenericPhysicsController_Gravity` | float | Downward acceleration, m/s² (negative = upward, used for buoyancy) (Probable) |
| `GenericPhysicsController_PhysicsTerminalVelocity` | float | Speed limit, m/s (Probable) |
| `GenericPhysicsController_JumpAcceleration` | float | Upward acceleration of a jump (OPEN: duration of its application) |
| `GenericPhysicsController_StepUpHeight` | float | Highest step climbed without jumping, m (Probable) |
| `GenericPhysicsController_FloatHeight` | float | Depth at which a mover floats in water, m (§10.5; Probable) |
| `GenericPhysicsController_ForceStrafe` | boolean | OPEN (Tessera reads and layers it, and does not use it) |
| `GenericPhysicsController_AllowContactLoss` | boolean | OPEN (Tessera reads and layers it, and does not use it; the client's move has a ground check switched by a flag of the move request, §11.4, but the source of that flag was not traced) |

A parallel set `MoveToController_*` (forward, back, left, right, up, down speeds, turn rate,
gravity, terminal velocity, step-up and float height) drives scripted "move to" motion.

<a id="s10-2"></a>
### 10.2 Controller records (Confirmed)

DID map `PHYSICSCONTROLLER` (0x28000030, `property-system.md` §8) maps controller kinds to 0x78
sets: key 1 Ladder → 0x78000149, 2 Ledge → 0x78000148, 3 Generic → 0x78000019, 4 MoveTo →
0x78000310, 0x10000001 Mounted and 0x10000003 MountedAI → 0x780010B1. Observed values of the
Generic set (identical in 0x780010B1): forward 4.5, backward 2.5, strafe 4.5 and 4.5, fly 0
and 0, control acceleration (35, 25, 0), damper (0.4, 0.4, 0.4), ramp time 0.3, gravity 40,
terminal velocity 60, jump acceleration 230, step-up 0.4, float height 0, force-strafe and
allow-contact-loss false. Ladder and Ledge sets exist but are empty; MoveTo holds only
an ethereality value. 0x7800000E (MoveTo defaults: speeds 4.5, gravity 16, terminal 50,
step-up 0.6, float height 1.35, turn rate 450) and 0x7800000F (`Physics_AvatarFloatHeight` 0.35,
ethereal to Default and PlayerImpassable) are the defaults for creatures and NPCs (Probable).

<a id="s10-3"></a>
### 10.3 Player movement records (Confirmed)

One object property set, 0x790304E2 (object 0x700304E2, root class 3,569), is the only set in
the archives with `PlayerPhysicsControl_*` properties. Each `…Parameters` property is a struct
holding a `PlayerPhysicsControl_MovementConfiguration` struct (fields of §10.1) and a
`PlayerPhysicsControl_MoveToConfiguration` struct (fields of the MoveTo set). A struct lists
**only the fields it changes**. Observed movement fields (– = not listed):

| Layer | Fwd | Back | Strafe L/R | Ctrl accel | Damper | Ramp | Gravity | Terminal | Jump | Step-up | Other |
|---|---|---|---|---|---|---|---|---|---|---|---|
| BasePlayer | 6.5 | 3.25 | 5 / 5 | (65, 65, 0) | (0.2, 0.2, 1) | 0.13 | 0 | 50 | 40 | 0.5 | fly 0/0, force-strafe and contact-loss false |
| Troll | – | 6 | – | – | – | – | – | – | – | – | |
| Mounted | – | 2 | – | – | – | 0.25 | – | – | – | – | |
| Warsteed | – | 2 | 3 / 3 | (10, 10, 0) | – | 0.25 | – | – | – | – | |
| Walking | 5 | 4.5 | 4.5 / 4.5 | – | – | 0.25 | – | – | – | – | |
| MountedWalking | 3 | 3 | 3 / 3 | (10, 10, 0) | – | 0.25 | – | – | – | – | |
| Slide | 3.2 | 2 | 2 / 2 | (43, 25, 0) | (1, 1, 1) | 0.5 | 50 | 100 | 0 | – | |
| Ice | 3.2 | 2 | 2 / 2 | (43, 25, 0) | (1, 1, 1) | 0.5 | 25 | 50 | 0 | – | |
| Falling | 5 | 1 | – | (10, 10, 0) | (1, 1, 1) | 0.01 | 15 | – | 0 | – | |
| StealthedFalling | 2.42 | 1 | 2.42 / 2.42 | (10, 10, 0) | (1, 1, 1) | 0.01 | 15 | – | 0 | – | |
| Slope | 0 | 0 | 0 / 0 | (0, 0, 0) | (1, 1, 1) | 0.1 | 10 | – | 0 | – | |
| Swimming | 3.2 | 2 | 2 / 2 | – | (0.4, 0.4, 0.4) | 0.4 | −20 | – | 0 | – | |
| StealthedSwimming | 4.15 | 2.5 | 2.5 / 2.5 | (35, 25, 0) | (0.4, 0.4, 0.4) | 0.4 | −20 | – | 0 | – | |
| Frozen | 0 | 0 | 0 / 0 | (0, 0, 0) | – | 0.25 | – | – | – | – | |

MoveTo fields: BasePlayer turn rate 150, terminal 50, gravity 0; Falling and StealthedFalling
gravity 15; Slope 10; Swimming and StealthedSwimming −20; Slide 50 (turn rate 35, terminal 100);
Ice 25 (turn 35, terminal 50); Mounted and Warsteed turn rate 45; MountedWalking 150. The set
also holds `PlayerPhysicsControl_DefaultFloatHeight` = 1.35.

These values are **observations**: Tessera reads them from the record at run time (locate the
set by the presence of `PlayerPhysicsControl_BasePlayerParameters`, not by its DID).

**Property names (OBS, C36; needed for interoperability).** Each layer is the struct property
`PlayerPhysicsControl_<Layer>Parameters`, where `<Layer>` is the layer's name in the table
above written without spaces: BasePlayer, Troll, Mounted, Warsteed, Walking, MountedWalking,
Slide, Ice, Falling, StealthedFalling, Slope, Swimming, StealthedSwimming, Frozen (all 14
exist in the property dictionary). Inside it, `PlayerPhysicsControl_MovementConfiguration`
holds the §10.1 fields and `PlayerPhysicsControl_MoveToConfiguration` the MoveTo fields. The
MoveTo fields of §10.1's parallel set are `MoveToController_ForwardSpeed`, `…_BackSpeed`,
`…_LeftSpeed`, `…_RightSpeed`, `…_UpSpeed`, `…_DownSpeed`, `…_TurnRate`, `…_Gravity`,
`…_TerminalVelocity`, `…_StepUpHeight` and `…_FloatHeight`; the dictionary also holds
`MoveToController_CanFly`, `…_TurnToFrozen`, `…_MoveToFrozen`, `…_PerformingMoveTo` and
`…_EtherealToTypeDuringAction`, which no player layer lists.

**Hostile values (Tessera Decision, decision 0013).** A field whose value has the wrong type,
is not finite or lies outside a generous range counts as **not listed**, so the layer below
keeps its value: speeds and terminal velocity 0 – 120 m/s, control acceleration 0 – 1,000 per
axis, damper 0 – 1 per axis, ramp time 0 – 60 s, gravity −200 – 200 m/s², jump acceleration
0 – 2,000, step-up 0 – 2 m, float height 0 – 10 m (every observed value lies well inside).
A MoveTo struct keeps at most 64 fields, each a fixed-size value. When the archives hold no
player movement record, Tessera uses a built-in copy of the observed layers of the table
above (default float height 1.35); a field that no applied layer lists takes the BasePlayer
value of the table.

<a id="s10-4"></a>
### 10.4 Movement states (Confirmed values)

The entity property `Entity_Element` (enum mapper 0x230001B9) holds a mover's current medium
and support; `Entity_FinalElement` a related value:

| Key | Label | Reading |
|---|---|---|
| 1 | Water_ControlledSurface | in water, standing on walkable ground |
| 2 | Water_NoSurface | swimming |
| 3 | Water_UncontrolledSurface | in water, on a steep surface |
| 4 | Earth_NoSurface | in the air (jumping, falling) |
| 5 | Earth_ControlledSurface | on walkable ground |
| 6 | Earth_UncontrolledSurface | on a surface too steep to walk (sliding) |

Water volumes are surfaces whose 0x78 set carries `Volume_IsLiquid` with `Volume_WaterDeep`
(0x78000073), `Volume_WaterWading` (0x78000074) or `Volume_WaterSurface` (0x78000075).

Where water volumes and water surfaces are placed in the world, and how the depth of water
at a point is found, is specified in `water.md`, which owns the water records: landblock
water (§6.2–§6.3), the landblock water-volume entity (§7.1) and placed volumes (§7.3). The
client's state rule is §11.3.2 (BIN): liquid with walkable support 1, with steep support 3,
without support 2. Which point of the mover the volumes test, and how the client keeps a
swimmer at its float height, stay OPEN (`water.md` §7.4); Tessera's rule is what §12.3 step 7
uses.

<a id="s10-5"></a>
### 10.5 Float height per species (Confirmed values)

`Physics_AvatarFloatHeight` on the playable 0x47 entities (`skeleton-anim.md` §2.2 table):
Hobbit 0.92 (both sexes), Human, Elf, Beorning and High-elf 1.42 (male) and 1.40 (female),
Dwarf 1.05, Stout-axe dwarf 1.12, River-hobbit 0.985 (male) and 0.97 (female). The value is
handed to both controllers as their float height (§11.2). It scales with the body: the
River-hobbit's 0.985 ≈ 0.7 × 1.40. Reading (Probable): the water depth, measured from the
feet, at which the character stops wading and floats; swimming parameters (gravity −20)
lift it back to that depth.

<a id="s10-6"></a>
### 10.6 Character collision shape (Confirmed)

Every playable setup (`setup.md` §12.3) has one capsule and no other collision:

| Setups | Lower cap centre z | Length | Radius | Total height |
|---|---|---|---|---|
| Hobbit 0x04000551/552, Human 0x04000553/554, Elf 0x04000555/556, Dwarf 0x04000557 | 0.35 (0.350 – 0.353) | 1.10 – 1.11 | 0.35 | 1.80 – 1.81 |
| High-elf 0x04005EC4 / 0x04005EC5 | 0.335 / 0.332 | 1.057 / 1.051 | 0.333 / 0.331 | 1.73 / 1.71 |
| River-hobbit 0x0400772A/72B | 0.5 | 1.586 | 0.5 | 2.59 |

The capsule is multiplied by the entity's **physics scale** (§8.1, BIN, C42; changed
2026-10-08): the z of its `Physics_AdjustableScale` times the z of its `Model_Scale`. The 14
playable entities carry `Physics_AdjustableScale` equal to their `Render_AdjustableScale`
(`appearance.md` §6.1 table): 0.7 for River-hobbits, 1.02 for Elf and High-elf females, 1.03
for Beornings, 1 otherwise. None carries `Model_Scale`; whether the client sets one on a
player at run time was not traced. The template `vec3` of `appearance.md` §6.1 is not part of
the physics scale (earlier revisions multiplied the capsule by the render model scale, which
includes it). Resulting capsules: radius 0.35 m and height 1.80–1.81 m for Hobbit, Human,
Dwarf, Stout-axe dwarf, Elf male and River-hobbit (setup × 0.7); about 0.36 m and 1.84 m for
Elf females; 0.36 m and 1.85–1.86 m for Beornings; 0.333 m and 1.73 m for High-elf males;
0.338 m and 1.74 m for High-elf females. So the original does **not** size the collision
capsule by species height; Hobbits and Dwarves differ from Men only in float height (§10.5).
Whether the controller uses this capsule directly is Probable (BIN shows a support query
radius of at least 0.3 m built from the mover's shape radius, §11.3).

Tessera Decision: the mover's position is its **feet**; the capsule's lower cap centre lies
the setup's lower-cap height above it and the upper one `length` higher, all three values
times the physics scale of §8.1 (the client's scale; changed 2026-10-08 from the render model
scale). For most playable setups the lower-cap height equals the radius, so
the capsule's bottom is the feet (High-elf: about 2 mm above them). Without a setup Tessera
uses lower cap 0.35, length 1.10, radius 0.35. Accepted ranges (hostile input): radius
above 0 and at most 5 m, length and lower-cap height 0 – 20 m.

---

<a id="s11"></a>
## 11. Rules found in the client (BIN)

<a id="s11-1"></a>
### 11.1 Choosing the player's parameters (Confirmed (BIN))

The client builds the active parameter set by starting from an empty set and **overlaying**
the layers of §10.3 in this order; each listed field replaces the value so far:

1. BasePlayer, always.
2. Troll, when the entity's `Agent_Species` is 47.
3. Mounted, when `Mount_Is_Mounted` is true; Warsteed instead when, in addition,
   `Mount_Movement_Use_RampUp_Controller` is true.
4. Walking, when `Movement_MovementStyle` is 1; MountedWalking instead when mounted.
5. If `Physics_OnSlide` is true: Slide, and steps 6–9 are skipped.
6. Ice, when `Physics_OnIce` is true.
7. Falling (StealthedFalling when `Stealth_IsStealthed`), when `Entity_Element` is 4.
8. Slope, when `Entity_Element` is 6.
9. Swimming (StealthedSwimming when stealthed), when `Entity_Element` is 1, 2 or 3.
10. Float height (§11.2) is set last; Frozen is applied when movement is frozen
    (`Movement_ActiveMovementFrozen`).

The controller kind is Generic (PHYSICSCONTROLLER key 3), or Mounted (key 0x10000001) when
`Mount_Movement_Use_RampUp_Controller` is true. The same layering fills the MoveTo set from
each layer's MoveTo struct.

**Value types of the conditions (BIN, C41; answers the implementer OPEN of 2026-10-07).** Each
of `Mount_Is_Mounted`, `Mount_Movement_Use_RampUp_Controller`, `Physics_OnSlide`,
`Physics_OnIce`, `Stealth_IsStealthed` and `Movement_ActiveMovementFrozen` holds only when the
value found on the entity is a Boolean (type 21) and true; a value of any other type counts as
false. The dictionary declares all six as Boolean (OBS). `Entity_Element`,
`Movement_MovementStyle` and `Agent_Species` are read only as enum or property-ID values (types
8 and 10), else as 0; the float heights of §11.2 only as floats (type 9). The implementer's
Decision is the client's behaviour. The layering itself copies each listed field of a layer's
struct over the set by property, without looking at its type; the readers of
`GenericPhysicsController_ForceStrafe` and `…_AllowContactLoss` in the composed set were not
traced, so for them the rule of §10.3 (a value of another type counts as not listed) stays a
Tessera Decision. Both are Boolean in the dictionary.

Example: a player on foot in deep water uses Swimming over BasePlayer: forward 3.2, backward 2,
strafe 2, control acceleration (65, 65, 0) (from BasePlayer), damper 0.4, ramp 0.4, gravity
−20, terminal 50, jump 0, step-up 0.5.

<a id="s11-2"></a>
### 11.2 Float height (Confirmed (BIN))

The float height is the entity's `Physics_AvatarFloatHeight` if present, else
`PlayerPhysicsControl_DefaultFloatHeight` from the record of §10.3, else 1.35. It is written
into both `GenericPhysicsController_FloatHeight` and `MoveToController_FloatHeight`.

<a id="s11-3"></a>
### 11.3 Walkable, sliding and wall contacts (Confirmed (BIN))

For each contact found around a mover, the client takes the unit contact normal. When the
contact is with a single triangle of a triangle collection (a 0x66 mesh, the terrain or
ceiling mesh or height field, a cell's physics mesh), it also computes the triangle's face
normal: the cross product of (second corner − first corner) and (third corner − first
corner), corners in their stored order, normalised and rotated into the world by the body's
rotation. Boxes, spheres and capsules give no face normal. When the contacted body's root
shape is of one particular kind (see below) and the face normal points down (*z* < 0), the
face normal is negated. When |face *z*| is below the walkable cosine and also below
|contact *z*| (both strict), the face normal replaces the contact normal. With *u* the up
(*z*) component of the chosen normal:

| Condition | Contact counts as |
|---|---|
| contacted object has `Physics_AlwaysUncontrolledSurface` true | ignored entirely (§11.3.1) |
| *u* < 0 | ignored as support (overhang) |
| *u* > cos *α*, *α* = **45°** for player characters, **70°** for other movers | walkable ("controlled") support |
| otherwise, *u* > cos 85° | steep ("uncontrolled") support |
| otherwise | wall, not support |

A mover counts as a player character when the top byte of its 64-bit instance ID is 2: the
client's test is exactly (instance ID ≫ 56) = 2 (BIN). That this byte marks player
characters is Reported (PUB2); landblock-info entities use 1 and 4
(`entity-descriptor.md` §4.2). An offline client has no server-assigned IDs; Tessera
Decision: the local avatar counts as a player character. The support search uses a radius of
at least 0.3 m (the larger of 0.3 and the mover's shape radius times its scale).

**The root kind of the flip (BIN, fifth pass, C37).** The flip tests the kind code of the
contacted body's outermost shape. That code belongs to the middleware's generic
bounding-volume-tree kind. In this build only the generic tree's own constructor sets it,
and every concrete tree kind the client uses sets its own code over it: the tree over a
stored code block (0x66 meshes, terrain and ceiling meshes), the height-field tree (§6.3),
the compressed mesh and the static compound. A scan of the executable's code found no other
instruction that writes that code into a shape and no shape constructor called with it. The
client's builders traced in §6.3 and §8.1 therefore never produce a body of this kind, and
the flip is expected never to apply (Probable). In effect, the client's face normal is always
the stored-order normal: it points up for terrain triangles (counter-clockwise from above,
§6.1), down for ceiling triangles (clockwise, §6.4), and either way for 0x66 and cell meshes,
as their corner order gives.

> **OPEN (C37):** whether any body of the generic tree kind exists at run time (for
> example, built by a path not traced here), so that the flip applies to it. **Tessera
> Decision:** no Tessera body kind gets the client's flip, neither terrain, ceiling, 0x66
> meshes, the triangles of boxes, cell physics meshes nor detail geometry. All of them use
> Tessera's own capsule-side orientation of §12.4 (x) instead, which already turns every
> face normal toward the mover.

The three angles, the tolerances below and the 0.3 m radius are constants of the client
(BIN), not data. They are physics limits and tolerances, which [decision
0012](../decisions/0012-numeric-constants.md) allows specs to state and Tessera's code to
contain; the same holds for the tolerances of §5.4, §5.8.1, §6.6, §8 and §11.4–§11.5.

<a id="s11-3-1"></a>
#### 11.3.1 The support search (BIN, C33)

The client gathers a mover's support contacts into a list of normals and three flags (any
contact, support, walkable support):

1. A contact farther than 0.08 m from the mover's support shape is skipped. Once a walkable
   contact has been found, every later contact with a positive distance is skipped.
2. A contact with an object whose `Physics_AlwaysUncontrolledSurface` is true is skipped
   entirely: it neither gives support nor adds a normal.
3. The normal is chosen as above. A chosen normal with *u* < 0 is skipped (no flag, no
   normal).
4. *u* above the walkable cosine sets both flags; otherwise *u* above cos 85° sets the support
   flag only; a wall sets neither.
5. The normal is added to the list unless one already listed differs from it by at most
   0.035 in each of *x*, *y* and *z*.
6. **Crease rule.** When the search ends without a walkable contact and the list holds two
   or more normals, the support counts as walkable when, for some pair of listed normals,
   their sum, normalised, has *u* at least the walkable cosine, or when the sum of all listed
   normals, normalised, does. A sum whose three components all lie within 0.001 of zero is
   ignored.

The client's debug description of this state calls a mover with the walkable flag set on a
"controlled surface" and every other mover on an "uncontrolled surface".

**Reading (C28; replaces the "mechanism" paragraph of earlier revisions, which other documents
cite as "§11.3 mechanism").** The movement state follows these flags exactly (§11.3.2, BIN since
2026-10-08): `Entity_Element` 5 (Earth_ControlledSurface) is the walkable flag and 6
(Earth_UncontrolledSurface) a support without it. That state 6 makes the mover slide is
Probable (through the Slope layer of §11.1 step 8). The SL1–SL3 walks of `m4-5-visual.md`,
where a walker running onto a bank is expected to slide back, are consistent with this: along
their paths the face under the capsule reaches 53.5°, 61.4° and 56.3° (OBS), inside the steep
band for players. Tessera's controller follows it (§12.3 step 6).

<a id="s11-3-2"></a>
#### 11.3.2 The movement state (BIN, C28)

The client sets a mover's `Entity_Element` from three inputs: the support flag and the
walkable flag of the support search (§11.3.1, after the crease rule; the search is re-run when
its result is stale), and *liquid*, the mover's own `Volume_IsLiquid` read as a Boolean
(C41). Movers get `Volume_IsLiquid` from the liquid monitor volumes they are inside
(`water.md` §7.1–§7.3, which own those volumes):

| Support flag | Walkable flag | Not liquid | Liquid |
|---|---|---|---|
| not set | (any) | 4 Earth_NoSurface | 2 Water_NoSurface |
| set | not set | 6 Earth_UncontrolledSurface | 3 Water_UncontrolledSurface |
| set | set | 5 Earth_ControlledSurface | 1 Water_ControlledSurface |

The float height, the depth of the water and the water level play no part in this rule; a
mover standing on walkable ground inside a liquid volume is in state 1 however deep the water.
Every volume set that carries `Volume_IsLiquid` counts, the coin sets of `water.md` §7.2
included. The routine is called from the client's position-update paths; how often it runs
per step was not traced. Which point or shape of the mover
the monitor volumes test was not traced (`water.md` §7.4).

<a id="s11-4"></a>
### 11.4 The client's move procedure (BIN, C34)

The client moves a mover by a requested displacement (direction and length; requests longer
than 500 m are refused) with a cast-and-slide loop:

1. At most 10 casts. Each cast sweeps the mover's shape along the current direction by the
   remaining length.
2. Every contact of the cast becomes a plane for the middleware's plane solver. A
   penetrating contact is placed at distance 0 and moves along its normal at 10 per second
   times its depth, so an overlap is pushed out over the following moves rather than at once;
   a contact closer than 1.5 × 10⁻⁴ m is treated the same way with its small distance, which
   draws the mover onto it.
3. For every contact whose normal has 0.01 < *u* < the walkable cosine (a steep contact) and
   whose object is not itself a mover, the solver also gets a **vertical plane**: the same
   normal with its *z* removed and renormalised. The mover can therefore slide along or down a
   steep face but cannot climb it, whatever its state.
4. When the cast hits, the mover advances to the hit; otherwise it takes the solver's
   displacement. The solver's new direction and the length left become those of the next
   cast. The loop ends when the length left is below about 10⁻⁷ m, or after the first cast
   when the request does not allow sliding.
5. **Step-up trigger.** After a cast, if one of the contacts found by the cast itself (not
   one that was touching at its start) has *u* at most the walkable cosine and is not with a
   mover, and step-up is allowed for the request, the client tries the step-up of §11.5 with
   the request's step-up height, then, if that fails and the request carries a second height
   above 0.0002, with the second height. A successful step-up is taken at most once per move;
   its forward move replaces the cast's and the loop continues with the rest of the length.
6. **Ground check** (when the request asks for it, and only when ground was found under the
   start): after a cast, the client looks up the ground height under the new position (a
   ground query, not a collision query). The move stops before that cast when no ground is
   found there, or when the ground rose, relative to the mover's own vertical motion in that
   cast, by more than 0.0002 and by more than the horizontal distance moved (tan 45° = 1).

Where the requested displacement, gravity, jumps and the ramp come from (the velocity
integration) was not located; see the OPEN below.

<a id="s11-5"></a>
### 11.5 The client's step-up (BIN, C35)

With *d* the unit direction of the requested move, *L* the length still to go after the
blocked cast and *h* the step-up height:

1. Back off: cast 0.05 m along −*d* and move by what the cast allows.
2. Rise: cast up by *h* and move by what the cast allows (a ceiling limits it).
3. Forward: cast along *d* by *L* plus the distance backed off in step 1. If the cast covers
   less than 10 % of that, the step-up fails.
4. Descend: cast down by *h* and move by what the cast allows.
5. Re-run the support search (§11.3.1) at the new position. The step-up succeeds only on
   walkable support; otherwise the mover stays where it was before step 1.

> **OPEN (RE tasks, §16):** how jump acceleration, ramp time, control acceleration and the
> damper are integrated per step, and how the requested displacement of §11.4 is formed from
> them; where the support shape of §11.3.1 is placed relative to the mover; which request
> flags (step-up allowed, sliding allowed, ground check) a player's move sets, and whether
> `AllowContactLoss` or `ForceStrafe` feed them; which point of the mover the liquid volumes
> test and how buoyancy uses the float height (the state rule itself is §11.3.2); any fall damage (no client property names one; probably decided by the
> server). The integration routine was looked for in three BIN passes without being located;
> §12.3 therefore stays a Tessera Decision.

---

<a id="s12"></a>
## 12. Tessera's controller (Decision, built from §3–§11)

This section is a design for Tessera, not a description of the original. Where the original's
rule is known (§11.3–§11.5) and Tessera does something else, §12.5 says so. Every rule here is
a Tessera Decision unless it cites a BIN or OBS fact; it must be revised when an OPEN closes.

<a id="s12-1"></a>
### 12.1 World representation

- **One collision set per landblock**, in the landblock's frame (x, y from the south-west
  corner, z absolute; `landblock.md` §3.3). It holds:
  - the terrain body: all 2,048 terrain triangles (§6.1, holes included, §6.5), each
    identified by its grid key (§6.2); the ceiling body of a landblock with a ceiling (§6.4);
  - one body per created static with collision (§9.3) and per generated object with
    collision (§9.4): its setup's template (§8.3) placed by its position, rotation and
    `Physics_AdjustableScale`; one body per placed cell's physics mesh (§5.3) placed with the
    cell's frame (§5.8), with the flags of §5.3;
  - optional detail bodies (§7), which only camera, picking and line-of-sight queries see;
  - the placed cells and their lookup (§5.8.1);
  - the landblock's water plane and liquid boxes (`water.md` §6–§7, decision 0013).
- **Flags per body:** each body keeps the §9.1 values it was built with; the pass-through
  test of §9.2 is made per query with the mover's categories, and the support rules read
  `Physics_AlwaysUncontrolledSurface` of the body touched. An object whose effective
  `Physics_NoPhysics` is true gets no body.
- **Order:** terrain and ceiling meshes first, then scenery generation (`scenery.md` §7,
  which needs only the terrain mesh and the statics' clearance volumes), then the statics'
  and generated objects' shapes (§9.4). This is the original's order (BIN); within a set the
  order of statics, generated objects and cells does not matter.
- **Acceleration:** one bounding-volume hierarchy per set over its triangles, spheres,
  capsules and solid boxes, built off the main thread when a landblock becomes resident. All
  triangles are two-sided (§4.2); boxes are solid (§8.3).
- **Frames:** a mover lives in the region frame in double precision (landblock (x, y) at
  (160 · x, 160 · y), decision 0013). Queries run in single precision in a frame centred
  near the mover; the sets of the landblocks around it are placed by their landblock
  numbers, in ascending (x, y) order, so ties between sets go to the lower landblock.
- **Bounded work (hostile input, decision 0013):** a set holds at most 2²¹ triangles, 2¹⁸
  spheres, capsules and boxes, 2¹⁸ bodies, 2¹⁶ cells, 2¹⁸ cell planes, 2²⁰ cell hull
  vertices and 4,096 liquid boxes; one query visits at most 2¹⁵ items per set. Objects beyond a
  limit are refused and counted.

<a id="s12-2"></a>
### 12.2 Mover

- Shape: the capsule of the avatar's setup times the physics scale (§8.1), placed on the feet
  (§10.6). Categories {Player}; the walkable limit is the player's 45° (§11.3; the local
  avatar counts as a player character).
- Parameters: §11.1 layering over the records of §10.3, composed afresh for every step from
  the entity conditions and the state the previous step ended in; float height §11.2.
- State: `Entity_Element` values of §10.4, decided at the end of each step from the support
  (§11.3) and the water (`water.md` §7.4). A new mover starts in state 4 with zero velocity.

<a id="s12-3"></a>
### 12.3 Step

Every step lasts exactly 1/60 s. A frame runs a whole number of steps, at most 5; time beyond
them is dropped. Lettered items refer to §12.4.

0. **Push-out:** resolve overlaps with the geometry first (g).
1. **Desired velocity:** the input has a forward and a strafe axis, each clamped to −1 … 1,
   and the pair is scaled down to length 1 when longer. The heading is a yaw angle
   counter-clockwise from +X: forward is (cos *h*, sin *h*), right is (sin *h*, −cos *h*). The
   target forward speed is the input times the forward speed (backward speed when negative),
   the target side speed the input times the strafe-right speed (strafe-left when negative).
   Each local axis approaches its target as in (a) and (l).
2. **Vertical:** on walkable support (states 1 and 5), vertical speed is 0 unless a jump
   starts; a jump is taken only there, as in (c) and (i). Otherwise the layer's gravity is
   applied for the step; in water, the buoyancy of (p) replaces it.
3. **Damping and limits:** each local velocity component is multiplied by its damper factor
   raised to the power 1/60 (b), the vertical one included. The velocity's length is then
   limited to the terminal velocity (and to 120 m/s). The vertical displacement of the step
   is the mean of the vertical speeds before and after gravity, times 1/60, also limited, so
   a jump's apex is *v*² / 2*g* whatever the step.
4. **Sweep and slide:** sweep the capsule along the displacement; on contact keep the motion
   up to the contact, remove the motion and velocity into the contact plane, and repeat with
   the remainder, at most 4 sweeps per step. Which plane a contact gives depends on the
   mover's support and the contact's class (§11.3), as in (e), (n), (q), (r), (t), (u), (v).
   In the air or while swimming every contact keeps its own plane.
5. **Step-up:** when a sweep on walkable support was blocked by a contact that is not
   walkable, try a raised move as in (d), (k) and (o); take it when it lands on walkable
   support.
6. **Ground snap and support:** sweep down a short way to stay on the ground (f), (j); then
   classify the support underfoot as in (s): walkable support gives state 5, steep support
   state 6, none state 4. A walker that has ridden onto a steep face therefore enters state 6
   at the end of that step, whatever its state before (§14.9, CO13).
7. **Water:** with the water surface and volumes of `water.md` §6.3 and §7, where the water
   surface lies more than the float height above the feet use state 2, where the feet are in
   water but less deep state 1 or 3 (`water.md` §7.4, Decision); in water without any support
   the mover swims (state 2).
8. **Cell:** after the step, find the mover's cell with the lookup of §5.8.1 at its feet
   (outdoors when no cell contains it). The lookup itself is the original's (BIN); running it
   every step is a Decision (§5.8.1 OPEN).

**Checks.** A step whose result is not finite, or that would move the mover by more than
10 m, is undone and the velocity zeroed. A mover farther from its query frame's origin than
single precision allows does not move. The step is deterministic: the same start state,
inputs and sets give the same result bit for bit.

<a id="s12-4"></a>
### 12.4 Tessera readings (Decisions)

These items answer the implementer OPENs of 2026-10-06 and 2026-10-07 (their letters are kept;
citations of the form "§12.3 OPEN (x)" in other documents refer to item (x) here) and state the
rest of the released design. Each stays a Decision until the original's rule is
known (§11.4 OPEN) or owner measurements (Q9) settle it.

- **(a) Approach.** Per local axis the velocity moves toward its target by at most
  rate × 1/60 per step and never past it, where rate = min(control acceleration of the axis,
  the axis' larger target speed / ramp time) (BasePlayer forward: min(65, 6.5 / 0.13) =
  50 m/s²). The forward axis uses the larger of the forward and backward speeds, the side axis
  the larger of the two strafe speeds. With a ramp time of 0 the rate is the control
  acceleration.
- **(b) Damper.** Step 3 after step 1 makes the steady speed target × damper^(1/60), for
  example 6.5 × 0.2^(1/60) ≈ 6.33 m/s forward for players, not 6.5.
- **(c) Jump.** The jump's push is the jump acceleration applied for one ramp time, given at
  once in the jump's step (40 × 0.13 = 5.2 m/s for players), which gives an apex of about
  0.9 m and about 0.7 s in the air under the Falling gravity of 15 m/s². The jump uses the
  jump acceleration of the layers in effect when it starts; since a jump starts only on
  walkable support, the Falling layer (jump 0) never applies to it. No jump starts when that
  acceleration is 0, and a jump speed above 120 m/s is cut to 120.
- **(d) Raised move length.** The raised move of a step-up covers at least half the capsule
  radius: a capsule resting against a riser that rises by the step-up height and moves only
  one step's distance would otherwise land on the step's edge with its centre up to
  *r* − |*v*| · 1/60 from it, which §11.3 classifies as steep at walking speeds.
- **(e) Walls on walkable support.** On walkable support, a contact classified as wall,
  overhang or ignored (§11.3) is treated as a vertical wall: its plane is the vertical plane
  of its normal's horizontal part (the normal itself when it is nearly vertical). Walls and
  step risers therefore never lift the mover by the slide. Steep contacts are treated by (q).
- **(f) No snap; no support in a jump's step.** There is no ground snap in the step a jump
  starts, while rising in the air or while swimming (state 2 at the start of the step).
  "Rising in the air" means: the mover did not start the step on walkable support (or a jump
  starts in it), its vertical speed after the slide is above 0, and its state at the start
  of the step is not a water state (1, 2, 3). In the step a jump starts, and while rising in
  the air, the support probe of (s) is not run and the support is **none**, whatever lies
  under the mover and whether or not it is in water. The step's state then follows from no
  support (§12.3 steps 6–7): 4 out of water, 2 in water. So a jump from wading (state 1)
  ends its first step in state 2, and a jump from the ground (state 5) in state 4. While
  swimming the probe runs as usual.
- **(g) Push-out.** At the start of a step, the contacts within 0.01 m of the capsule are
  gathered; when one of them penetrates, the mover is moved by the shortest push that brings
  every contact closer than two skins (w) out to two skins, using at most the 32 deepest
  contacts and at most three planes at a time. Up to 4 such rounds run per step, at most
  0.5 m in all. Only contacts whose separation is finite and whose normal has unit length
  within 10⁻³ count; others are hostile values and are ignored.
- **(h) MoveTo fields.** The MoveTo structs are layered field by field (property by
  property); their names are now given in §10.3.
- **(i) Jump ramp time.** The ramp time of (c) is the BasePlayer layer's (0.13 s), whatever
  layer is active; with the composed layer's ramp time, the Walking and mounted layers (ramp
  0.25 s, jump acceleration 40 from BasePlayer) would launch at 10 m/s with an apex of about
  3.3–3.5 m.
- **(j) Snap reach.** The ground snap sweeps down by 0.05 m; on walkable support it reaches
  further by the drop of a walkable slope over the step's horizontal motion (horizontal
  distance × tan of the walkable limit, at most the step-up height). The snap moves the mover
  only onto support (walkable or steep, §11.3) that the sweep reaches after a positive
  distance. So the mover follows walkable downhill slopes and briefly falls off steeper drops,
  for example a 0.3 m step at run speed.
- **(k) Step-up acceptance.** The rise is swept up by the step-up height (less where something
  stops it; no step-up when the free rise is 0.001 m or less); the raised move is swept
  horizontally with every contact a vertical wall; the landing is swept down by the rise plus
  0.05 m and must be walkable both by the contact it lands on and by the support probe of (s).
  The step-up is taken only when it climbs more than 0.01 m and gets further along the move
  than the unraised slide did. When the raised move was longer than the move itself (the
  minimum of (d)), the same climb with the move's own length is preferred where it also lands
  walkable and climbs in proportion, so sliding along a wall is never sped up. "Climbs in
  proportion": its climb is at least the raised move's climb × (move length / raised length)
  − 0.001 m, where the raised length is the length the raised move actually taken covered (the
  first reach of (d), or the full capsule radius after the retry of (o)); the move's own length
  must also pass the acceptance test above, otherwise the longer raised move is taken. "Further
  along the move than the unraised slide" is strict, with no slack.
- **(l) Per-axis approach and walls.** A mover pushing diagonally into a wall slides along it
  slower than the wall-parallel part of its speed: the wall removes the velocity into it each
  step and the side axis then brakes the rest toward its target. At 0.5 rad from the wall
  normal a player settles near 0.5 m/s instead of 6.33 × sin 0.5 ≈ 3.0 m/s.
- **(m) Rising in water.** (f)'s "no support while rising" applies only out of water, because
  in water the swimming gravity of −20 lifts the mover.
- **(n) Creases.** On walkable support, when a sweep's contact normal (at an edge or corner of
  a triangle) does not oppose the motion but the triangle's face does, the motion slides along
  the face; otherwise the same hit would repeat at the same place every step and stop the
  mover dead on a slight terrain crease.
- **(o) Retry over chamfers.** When the raised move's landing is not walkable (the capsule's
  foot rests on a chamfered step edge, whose steep face decides the class), the raised move is
  retried once covering the full capsule radius.
- **(p) Buoyancy.** In water without walkable support (states 2 and 3) the float line is the
  water surface minus the float height. Above the line (by more than 0.001 m) the mover sinks
  with the layer's gravity in magnitude, at least 1 m/s², and stops at the line with zero
  vertical speed. At or below it the layer's gravity applies (−20 lifts a swimmer), the water
  takes up any downward speed and displacement at once, and the feet stop at the line with
  upward speed removed. The speed is limited again afterwards.
- **(q) Steep contacts on walkable support (geometric criterion).** Every sweep contact is
  classified by §11.3 from its normal, with Tessera's face-normal substitution (x). On walkable support
  a walkable contact keeps its normal, so walkable slopes are climbed by the slide itself. A
  **steep** contact (45°–85° from up for players, i.e. cos 85° < *u* ≤ cos 45°) is not a wall:
  the step's motion slides along its own plane, so the capsule rides onto the face. For a
  contact on the capsule's lower cap (radius *r*) this is the same as: the contact point lies
  below the lower cap's centre by more than *r* · cos 85° and by at most *r* · cos 45°.
  A steep contact still counts as blocking, so the step-up of step 5 is tried first and wins
  when it lands walkable (steps and chamfered edges keep climbing).
- **(r) Velocity on a steep face.** When the motion rides onto a steep face (q), the velocity
  loses only its part into the face's vertical plane (the normal's horizontal part), so riding
  onto a steep face gives the mover no upward speed.
- **(s) Support.** The support probe is a sphere of radius max(0.3 m, capsule radius) whose
  bottom is the capsule's bottom. Its contacts within 0.01 m are classified by §11.3; the
  support is walkable when any is walkable, else steep when any is steep, else none. A mover
  that has ridden onto a steep face has no walkable contact left within that margin, so it is
  in state 6 at the end of that step.
- **(t) Steep support.** While the mover starts a step on steep support (state 6, or 3 in
  water), every contact keeps its own plane, and a steep contact adds a second plane: the
  vertical plane of its normal's horizontal part. The mover can then slide down or along the
  face but not up it. With the Slope layer of §10.3 (no control acceleration, gravity 10), a
  walker held against a steep bank rides a few centimetres onto it, enters state 6, slides
  back to the ground and repeats.
- **(u) Merging planes.** The planes of one slide are merged by §11.3.1's tolerance (0.035
  per component); a new normal that merges with a listed one **replaces** it, because the
  latest contact is the one the remaining motion runs into (keeping the older normal stopped a
  mover at a shallow concave crease between two walkable triangles). A slide keeps at most 8
  planes. The motion is clipped by sliding along one plane when that keeps it out of all the
  others, else along the crease line of the last two planes listed, else stopped. The single
  planes are tried latest first and the first that works is used; a clipped motion *c* is out
  of a plane with normal *n* when *c* · *n* ≥ −10⁻⁵ · |*v*|, *v* being the motion before
  clipping (a rounding tolerance).
- **(v) Keeping the heading.** While a slide has met only walkable planes, the clipped motion
  and velocity are turned back to the horizontal direction of the original motion, keeping
  their horizontal length, and lifted onto the highest of those planes; a walker on a side
  slope then keeps its heading instead of drifting downhill. The rule is skipped when the
  clipped motion points backward or a plane is nearly vertical. It applies in every state (on
  walkable or steep support, in the air, swimming), but never to the step-up's raised move,
  whose planes are vertical walls (k). So a mover that lands on a walkable side slope keeps the
  horizontal heading of its fall in the landing step.
- **(w) Contact offset.** A sweep reports an obstacle when the gap to it would fall to
  0.001 m (one skin) and stops the capsule where the gap is still two skins (up to two and a
  half; less only when it started closer). The hit threshold is lowered to half the capsule's
  *clearance* (its smallest starting gap to anything the sweep tests) when that is smaller than
  one skin, so a capsule starting closer than one skin to a surface is not stopped at the
  shared edge of a coplanar neighbour; for any other start the threshold is exactly one skin.
  A starting gap below 10⁻⁶ m counts as touching, like an overlap. A sweep never resolves an
  overlap: a capsule that starts inside or touching an obstacle is stopped at once when it
  moves further in, and not at all otherwise. Ties between equally near obstacles go to the lowest body, then triangles before
  spheres and capsules, then the lowest feature.
- **(x) Face normal (Tessera's rule, differs from §11.3).** Every classification by §11.3 in
  Tessera (sweep contacts, the ground snap, the step-up landing, the support probe) uses
  this rule in place of the client's face-normal rule:
  1. A contact with a triangle (of any body: terrain, ceiling, 0x66 mesh, the 12 triangles
     of a box, a cell's physics mesh, detail geometry) has a face normal: the unit normal of
     the triangle's plane, turned to the **capsule's side**. In a sweep, that is the side the
     contact normal points to (the face normal is negated when its dot product with the
     contact normal is negative). In a contact query (the support probe of (s), the push-out
     of (g)), it is the side of the capsule's core centre (the midpoint of its axis segment), seen
     from the contact point on the triangle. The stored corner order plays no part, and no root-kind flip applies (§11.3
     OPEN C37).
  2. A capsule whose core is inside a solid box (§8.3) gets the outward normal of the face it
     leaves by as both contact and face normal, so (4) never changes it.
  3. Spheres, capsules and degenerate triangles have no face normal; a face normal that is
     not finite is ignored.
  4. The face normal replaces the contact normal when its *z* is **at most** the walkable
     cosine and **less than** the contact normal's *z* (signed values, not absolute ones).

  Since the face normal is turned toward the mover, a triangle under the mover never gives a
  downward face normal: a mover standing on the back face of a triangle (an inside-out 0x66
  mesh, §4.2, or a ceiling triangle seen from above) is supported as on its front face. The
  face normal also serves the crease rule (n). Differences from the client: §12.5.

- **(y) Speed after the slide.** After the slide and the step-up the velocity's length is
  limited again to the terminal velocity and 120 m/s, as (p) does after buoyancy, because (v)
  can lengthen it; the displacement already taken is not changed.

The solver details in (g), (k), (u), (v), (w) and (y) answer the implementer OPEN of
2026-10-07 (items 1–5); they are the released choices, confirmed as Tessera Decisions. None of
them has a counterpart in the client, whose move is the middleware's cast-and-plane-solver loop
of §11.4 (its own thresholds: 1.5 × 10⁻⁴ m near contacts, 10⁻⁷ m remaining length, at most 10
casts); §12.5 lists the differences.

<a id="s12-5"></a>
### 12.5 Where Tessera differs from the original's known rules

The original's support search, move and step-up (§11.3.1–§11.5, BIN) became known after the
controller above was built. Tessera differs from them as follows (all Tessera Decisions; see the answer below the
table and Q13):

| Topic | Original (BIN) | Tessera |
|---|---|---|
| Shape radius | 0.05 m convex radius on boxes and triangles (§8.2) | none; contact offset (w) |
| Support distance | contacts within 0.08 m of the support shape | within 0.01 m of the probe (s) |
| Support exclusions | `Physics_AlwaysUncontrolledSurface` and downward normals give no normal at all | they give no support; in a slide they act as walls (e) or keep their planes (t) |
| Crease rule | pairs or all normals summed may make the support walkable (§11.3.1 item 6) | none; creases handled in the slide (n), (u) |
| Steep faces | extra vertical plane for every steep contact with a non-mover, in every state | only on steep support (t); on walkable support the motion rides on (q) and the velocity meets the vertical plane (r) |
| Walls on walkable ground | real contact planes | vertical walls (e) |
| Casts | up to 10 per move, one move per update | up to 4 sweeps per 1/60 s step |
| Overlaps | penetrating contacts push out at 10 per second × depth | immediate push-out, at most 0.5 m per step (g) |
| Step-up | back off 0.05 m; rise; forward by the rest plus the back-off, at least 10 %; descend by the rise; walkable support; second height; once per move | no back-off; raised move of at least half the radius, retry with the full radius; descend by the rise plus 0.05 m; at least 0.01 m climbed and more progress than the plain slide (d), (k), (o) |
| Ground check | optional per move (§11.4 item 6) | none |
| Face normal | stored corner order; negated when pointing down only for a root kind that no traced builder produces (in effect never, C37); replaces the contact normal when \|face *z*\| < cos *α* and \|face *z*\| < \|contact *z*\|; boxes are convex shapes and give none | turned to the capsule's side for every triangle, the 12 triangles of each box included; replaces the contact normal when face *z* ≤ cos *α* and face *z* < contact *z* (signed) (x). Results differ for back faces (the client can choose a downward face normal and skip the contact as an overhang; Tessera never does for a contact from above), at the boundary face *z* = cos *α*, for contacts whose normal points down, and at box edges |

**Answer to the implementer OPEN of 2026-10-07 (2026-10-08).** The client's side of every row
is known (BIN), and Tessera's scale rule (§8.3) now follows the client. Keeping Tessera's own rule in
each remaining row is a Tessera Decision, not an open question about the client: the rows stay
as they are until the owner measurements of Q9 show a difference that matters, or until a
decision to adopt the client's rule. The velocity integration of the client is still unknown
(§11.5 OPEN), so the integration items (a)–(c), (i), (l) and (p) stay Decisions either way.

---

<a id="s13"></a>
## 13. Invariants (Confirmed over all records unless stated)

- **CO1** (0x66): the payload is consumed exactly by §4.1; `has_code` = 1; every triangle
  index is below the vertex count; `build_byte` present iff `stamp` ≥ 38.
- **CO2** (0x01): the payload is consumed exactly by §5.1; plane tags are 0x42502D2D except the
  last; the last is 0x4250464C or 0x4C454146; `mesh_count` = 1, `mesh_flag` = 0; portal lead
  byte 1 and corner count 4; every plane normal has unit length within 10⁻³.
- **CO3** (cells): for every landblock-info cell, `neighbour_count` = portal count of its
  geometry; neighbour *k*'s back-reference *r* satisfies: the neighbour's entry *r* names this
  cell with back-reference *k*.
- **CO4** (trees): every code block of every host walks without an unknown opcode or an
  out-of-range jump; every key is valid for its host (§3.5; 15 renumbered mesh levels).
- **CO5** (trees): `origin` and `scale` satisfy §3.2 for 0x66 and 0x01 physics meshes.
- **CO6** (trees): for 0x66, 0x01 physics meshes and terrain, the point query (§3.7) of every
  triangle's centroid and corners reports that triangle's key, when the code contains it.
- **CO7** (terrain): every terrain code reports exactly the keys 0 – 2,047.
- **CO8** (setups): for setups whose only collision is the 0x66 mesh, `physics_bounds` equals
  the mesh's vertex bounds within 10⁻³.
- **CO9** (movement record): exactly one 0x79 set holds `PlayerPhysicsControl_*` properties;
  every `MovementConfiguration` field is one of the 16 of §10.1.
- **CO10** (ceilings): every stored ceiling-chunk triangle lies in one grid quad, uses the
  diagonal given by bit 0 of the vertex-flag byte of the quad's south-west sample, and is
  clockwise seen from above (5,310,848 of 5,310,848).
- **CO11** (holes): in a landblock whose terrain chunk has layers, a grid triangle covered
  by no layer occurs only when the landblock has a ceiling, and the mean of its three
  ceiling samples is never above the mean of its three terrain heights (2,750,134 of
  2,750,134).
- **CO12** (blocker census, §9.3): over the Physics SEDs that have a position, classifying by
  the resolved chain and setup, with the plain draw-suppression of §9.3 and **no** run-time
  filter, gives 82,139 undrawn mesh blockers among 97,688 undrawn blocking SEDs and 1,651,097
  blockers in all. Applying the content-layer test (*L* = 1, *I* = 0) gives 1,607,212 /
  88,749 / 73,509, and additionally applying the creation-type filter with the SED's own
  value first (0, 7 and 8 dropped) gives 1,607,211 / 88,749 / 73,509.
- **CO13** (state 6, synthetic, §12.3 step 6, §12.4 (q), (s), (t)): a player walker
  (capsule radius 0.35, lower cap 0.35, length 1.10, BasePlayer layers of §10.3) that starts
  on flat ground 2 m before the foot of a planar bank and moves straight at it, with full
  forward input or with 0.3 of it, for 240 steps (4 s), when the bank's face rises at 46°,
  50°, 55°, 61° or 80° from the horizontal: is in state 6 in at least one step; its feet never
  rise 0.6 m or more above the flat ground; and it never gets 0.6 m or more past the bank's
  foot. On a 44° ramp the same walker is never in state 6 and climbs more than 3 m in the
  240 steps.
- **CO14** (creation types, §9.3, OBS): no Physics SED has an effective `Entity_CreationType`
  of 7 or 8 when the SED's own value is read first, and exactly one has 0; 1,602 placed
  Physics SEDs have 7 or 8 on their 0x47 chain, and each carries its own value.
- **CO15** (extra sets, §9.1, OBS): of the 40,643 0x47 records, 20,274 name 0x78 extra sets;
  the only §9.1 property those sets carry is `Physics_SurfacePropertiesToApply`, which they
  add to 20,164 records that lack it in their embedded set (52 embedded, 20,216 with extra
  sets).
- **CO16** (corner order, §6.1, synthetic): for every quad and both diagonal bits, the four
  stored corner lists of the §6.1 table give, as sets, the triangles of §6.2 with the same
  keys; the terrain lists are counter-clockwise and the ceiling lists clockwise seen from
  above; each ceiling list is the reverse of the terrain list for the same key and diagonal.
- **CO17** (scale census, §8.1, OBS): no landblock-info SED carries `Model_Scale`, and every
  SED's `Physics_AdjustableScale` has x and y within 0.0002 of z (3,253,862 SEDs), so the
  physics scale of a placed object equals its `Physics_AdjustableScale`.

---

<a id="s14"></a>
## 14. Synthetic worked examples (for tests)

<a id="s14-1"></a>
### 14.1 A 15-byte code

Container: `origin` (0, 0, 0), `scale` 65,536 (one byte step per metre). Code bytes:

`26 00 0A  27 00 0A  28 00 05  10 05 04 01  30  31`

Filters X and Y in 0 – 10 and Z in 0 – 5, then a split on X: left (the byte 0x30, key 0) when
x ≤ 5, right at +1 (the byte 0x31, key 1) when x ≥ 4.

| Point | Bytes | Reported keys |
|---|---|---|
| (2.0, 3.0, 1.0) | (2, 3, 1) | {0} |
| (4.5, 1.0, 1.0) | (4, 1, 1) | {0, 1} |
| (8.0, 1.0, 1.0) | (8, 1, 1) | {1} |
| (11.0, 1.0, 1.0) | (11, 1, 1) | {} (X filter) |
| (−0.5, 1.0, 1.0) | (−1, 1, 1) | {} (X filter) |

### 14.2 Rescale and base

Code `01 02 00 00  09 20  50 05` with `scale` 65,536: point (3.25, 0.5, 0.5) has *q* x =
212,992; after the rescale (shift 1, subtract 2 · 2¹⁶) *q* x = 163,840 (byte 2); the base
becomes 0x20 and the leaf reports 0x20 + 5 = 37.

### 14.3 Diagonal split

Code `17 90 8C 01 31 30` (split on (x + y)/2, *hi* 0x90 = 144, *lo* 0x8C = 140, right child
at +1), points on whole byte values: bytes (100, 150, 0) give 125 → left only → key 1;
(150, 150, 0) give 150 → right only → key 0; (140, 144, 0) give 142 → both → {0, 1}.

### 14.4 Terrain keys

Quad *i* = 3, *j* = 7: *base* = 2 · (32 · 7 + 3) = 454. Corners in the client's stored order
(§6.1 table). With bit 7 set at sample (3, 7): key 454 = (3, 8), (3, 7), (4, 8); key 455 =
(4, 8), (3, 7), (4, 7). With bit 7 clear: key 454 = (3, 8), (3, 7), (4, 7); key 455 = (4, 7),
(4, 8), (3, 8). Every list is counter-clockwise from above; a rotation of a list (for example
(3, 7), (4, 8), (3, 8) for key 454 with bit 7 set, as earlier revisions wrote it) is the same
triangle with the same face normal.

The example's corner lists follow the stored order of §6.1 since 2026-10-08 (this answers the
implementer OPEN of 2026-10-07: the earlier lists were rotations of the same triangles, meant
as winding, not as a start corner).

### 14.5 Minimal cell record

A 0x01 record of an 8 × 8 × 4 m box room with one opening on its east side and no physics
mesh: DID, byte 0; six planes, the first five tagged 0x42502D2D and the last 0x4250464C:
(0, 1, 0, 4), (−1, 0, 0, 4), (0, −1, 0, 4), (1, 0, 0, 4), (0, 0, −1, 4), (0, 0, 1, 0); portal
count 1: byte 1, count 4, corners (4, −4, 0), (4, 4, 0), (4, 4, 4), (4, −4, 4); wall count 5,
each a count 4 and the corners of one of the other five faces; hull count 8 with corners
(±4, ±4, 0 or 4); an empty property set (hint 0, count 0); u32 1; byte 0; then a mesh body.
Point (0, 0, 2) is inside (every plane gives ≥ 0); (5, 0, 2) is outside (the plane
(−1, 0, 0, 4) gives −1). A landblock-info cell placing it with `neighbour_count` 1 links its
portal 0 to the neighbour's portal named by the back-reference.

### 14.6 Parameter layering

BasePlayer then Falling (state 4, not stealthed): forward 5 (Falling), backward 1 (Falling),
strafe 5 (BasePlayer), control acceleration (10, 10, 0), damper (1, 1, 1), ramp 0.01,
gravity 15, terminal 50 (BasePlayer), jump 0, step-up 0.5 (BasePlayer).

<a id="s14-7"></a>
### 14.7 Terrain radius query (§6.6)

p = (12.0, 13.0), r = 0.5. Cell range i = ⌊11.5 / 5⌋ … ⌊12.5 / 5⌋ = 2 … 2, j = 2 … 2.
The point lies inside the square [10, 15]², so the cell is kept. Its distance to the
segment (10, 10)–(15, 15) is |13 − 12| / √2 = 0.7071 > r, so step 5 applies:
s = (3 − 2) / √2 = 0.7071 ≥ −0.0002, and the query collects key 2 · (32 · 2 + 2) + 1 = 133
only. With p = (12.0, 12.2) the distance is 0.1414 ≤ r and both keys 132 and 133 are
collected. With p = (13.0, 12.0) and r = 0.5, s = −0.7071 and key 132 alone is collected.

<a id="s14-8"></a>
### 14.8 Point in cell with tolerance (§5.4, §5.8.1)

The box room of §14.5 placed at position (100, 50, 20) with no rotation. The landblock
point (104.0001, 50, 22) maps to (4.0001, 0, 2); the plane (−1, 0, 0, 4) gives −0.0001 ≥
−0.0002, so the point is inside. The point (104.001, 50, 22) gives −0.001 and is outside;
if no other cell contains it, it is outdoors.

<a id="s14-9"></a>
### 14.9 Riding onto a steep bank (§12.3 steps 4–6, §12.4 (q), (r), (s))

A player walker (capsule radius *r* = 0.35, BasePlayer layers) runs along +X on flat ground
(state 5) at 6.33 m/s, so a step's motion is *d* = (0.1055, 0, 0). The capsule's lower cap
meets a planar bank whose face rises at 55°: its normal is *n* = (−sin 55°, 0, cos 55°) =
(−0.819, 0, 0.574).

1. Classification (§11.3): *u* = 0.574; cos 85° = 0.087 < 0.574 ≤ cos 45° = 0.707, so the
   contact is **steep**, not a wall.
2. Motion (q): the rest of the motion slides along the face's own plane:
   *d* · *n* = −0.0864, and *d* − (*d* · *n*) *n* = (0.0347, 0, 0.0496). The capsule rides
   0.0496 m up the face.
3. Velocity (r): the velocity loses its part into the face's vertical plane, whose normal is
   (−1, 0, 0): (6.33, 0, 0) becomes (0, 0, 0). No upward speed is gained.
4. Support (s): the probe's bottom is now about 0.05 m above the flat ground, outside the
   0.01 m margin, while the face touches the probe. The only support contact is steep, so the
   step ends in **state 6**.
5. Next step: the Slope layer (§10.3: no control acceleration, gravity 10 m/s²) applies, and
   on steep support the face adds its vertical plane (t): the walker slides back down the face
   to the flat ground and returns to state 5. With the input still held, steps 1–5 repeat:
   the walker stays at the foot of the bank (CO13).

With a 44° face instead, *u* = 0.719 > 0.707: the contact is walkable, keeps its plane, and the
walker climbs (state 5 throughout).

---

<a id="s15"></a>
## 15. Corrections and additions to other specs

| Spec | Statement | Correction |
|---|---|---|
| `mesh.md` §10.2 | The tree is "built from the level's collision vertices"; "Tessera does not interpret the code" | The code indexes the level's **render** triangles (draw position · 2²⁰ + triangle), not the §9 collision triangles; its box is that of the render triangles in it (§7.1). The code format is now specified (§3). |
| `mesh.md` §10.2 | 2²⁴ / `scale` is 0.96–1.53 times the collision mesh's extent | Exactly (*L* + 0.1) · 256/254 of the extent *L* of the triangles in the code (§3.2). |
| `landblock.md` §3.9 | `code` is opaque | Specified in §3; terrain keys in §6.2; ceiling in §6.4. |
| `landblock.md` §4.4 OPEN | Layout of 0x01 cell geometry (portals, PVS); neighbour meaning | §5: full layout; neighbour entries are portal links (§5.8). |
| `setup.md` §2.2 | 0x01: "byte 1, VLE vertex count, …, collision tree" | That is the optional physics mesh; the byte is a presence flag (0 in 47 records), followed by the cell volume, portals, walls, hull, properties and a render mesh (§5.1). |
| `setup.md` §6.5 | 0x66 "not decoded here" | §4. |
| `records.md` §2.1 | 0x66 not described | Stand-alone collision mesh (§4). |
| `landblock.md` §3.5 | OPEN: which rule picks a ceiling quad's diagonal (Q5); whether terrain collision keeps the hole triangles | Bit 0 of the vertex-flag byte for every ceiling quad, stored triangles or not (§6.4, CO10); ceiling triangles face down. The client's terrain collision keeps every hole triangle (§6.5). |
| `landblock.md` §3.6 | Bits 0–6 "a renderer ignores them" | Bit 0 is the ceiling diagonal (§6.4); bits 1–6 stay OPEN. |
| `terrain-texturing.md` §4.5 | Holes are "cave mouths"; collision "a separate question" | Holes lie where the terrain is buried under the ceiling surface or where there is no ceiling; the client's terrain collision keeps them (§6.5). |
| `scenery.md` §7.4 step 13 | (pointer) | The clearance step's terrain query is §6.6. |
| `scenery.md` §7.6 | Instance ID "synthesized from the landblock and the record's position in the list"; formula not traced | The layout is in §9.4 (kind byte 5, region, X, Y, instance number, 0-based list position; BIN, C38). |
| `entity-descriptor.md` §4.9 step 1 | "A value that is not an integer or enum counts" as no value | The client's creation-type readers accept an enum (type 8) or property-ID (type 10) value only; a 32-bit integer (type 15) counts as no value (§9.3 item 3, BIN, C41). The dictionary declares `Entity_CreationType` as an enum, so no stored value is affected. |

---

<a id="s16"></a>
## 16. Open questions

| # | Question | Where | Suggested method |
|---|---|---|---|
| Q1 | Surface property that removes a draw from a mesh-level code | §3.5 | M1 (surface/material flags of 0x310003B4) |
| Q2 | Meaning of the two plane-list terminators, the portal lead byte, portal corner winding | §5.4, §5.5 | M1 |
| Q3 | Neighbour flag byte; non-coincident portal pairs. (Entering a cell from outdoors: answered, §5.8.1.) Remaining: whether a moving character's cell is re-evaluated every step or on phantom overlap | §5.8, §5.8.1 | M4 (movement code), M1 (cross links, door entities) |
| Q4 | Partly answered (§6.3, BIN fourth and fifth passes): the client builds the full 2,048-triangle mesh for every landblock and uses the stored tree when there is one; the loader's path builds no tree at load time; without a tree the physics body is a sampled height field when a global switch is set, else the bare mesh. Remaining: the switch's run-time value, the height field's quad split, and whether the second load path (which may build a tree at load time) serves the physics body | §6.3 | M4 |
| Q5 | Answered: ceiling diagonal from bit 0; zero ceiling samples are part of the ceiling mesh (§6.4) | §6.4 | — |
| Q6 | Use of per-level collision meshes and trees | §7 | M4 |
| Q7 | Exact ethereality test and default categories | §9.2 | M4 |
| Q8 | Placement of water volumes and depth query: owned by `water.md` | §10.4 | — |
| Q9 | Integration of jump, ramp, control acceleration and damper; how the move request is formed. **Partly answered (BIN, 2026-10-07):** the support search (§11.3.1), the move's casts and plane solver (§11.4) and the step-up (§11.5); **2026-10-08:** the routine that sets `Entity_Element` (§11.3.2). The earlier "character proxy" reading is withdrawn (§11.3). **Still open:** the velocity integration and buoyancy, the request flags a player's move sets, where the support shape sits, which point of a mover the liquid volumes test | §11.3–§11.5, §12.3 | M4 (the integration routine was not located in three passes), then M3 (owner measurements: jump height and airtime, walk and run speeds, highest climbable step and slope) |
| Q10 | Fall damage | §11.3 | probably server-side; nothing in the client data |
| Q11 | `ForceStrafe`, `AllowContactLoss`, `Entity_FinalElement` | §10.1, §10.4 | M4 |
| Q12 | Physics bodies for generated objects of client-only creation types (they are created, BIN; the body was not traced) | §9.4 | M4 |
| Q13 | Whether Tessera adopts the original's 0.05 m convex radius (§8.2–§8.3) and its support, move, step-up and face-normal rules (§12.5). The scale rule is settled: the client's physics scale is uniform and Tessera follows it (§8.1, §8.3, 2026-10-08); no client fact is missing for this row | §8.3, §12.5 | Tessera Decision; owner (Q9 measurements) |
| Q14 | Whether an offline client instantiates the statics of creation types 7 and 8 that its loader hands to the dynamic list (none in this install has an effective 7 or 8) | §9.3 | M4 |
| Q15 | What covers the 413,655 undrawn terrain triangles with no ceiling over them, and whether a walker can reach them (collision does not depend on it) | §6.5 | M1 (cells, statics and neighbouring geometry over those triangles) |
| Q16 | Generated objects (implementation OPEN of 2026-10-06): (a) instance ID: answered (BIN, C38); (b) liquid monitors: no scene object carries one in this install (OBS, C39); still open whether the client would create the volume for a generated object (Tessera Decision: it does) | §9.4 | M4 (only if another install has such objects) |
| Q17 | Whether any run-time body has the root kind that the face-normal flip tests (expected never; Tessera Decision: no flip, capsule-side orientation) | §11.3, §12.4 (x) | M4 |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version: quantised tree, 0x66 and 0x01 layouts, cells and portals, terrain and ceiling trees, per-level mesh trees, setup volumes, per-instance flags, movement data and the BIN rules for parameter layering, float height and slopes; Tessera's controller. |
| 2026-10-05 | Second pass (BIN and OBS). §6.1: the client builds the full 2,048-triangle terrain mesh for every landblock. §6.3: answers what is used without a stored tree, in part. §6.4: the ceiling diagonal is bit 0 of the vertex-flag byte (all 5,310,848 stored ceiling triangles), ceilings face down, zero samples are in the mesh. New §6.5 (terrain holes do not need cutting: they occur only under ceilings and never under an open cave; the client keeps them in its collision) and §6.6 (the client's terrain-triangle radius query used by scenery placement). §5.3: the cell physics mesh is the cell's solid body. §5.4: the client's point-in-volume tolerance −0.0002. New §5.8.1: cell phantoms and the client's lookup of the cell containing a point (entering a cell from outdoors needs no portal). New §9.4: collision of procedurally generated scenery, the creation path, the build order and a census. §10.4 and §12.3 step 7 point to `water.md` for water volumes. §11.3: the instance-ID test (BIN) with a published source for the meaning of byte 2 (PUB2); the binary-sourced constants are recorded under decision 0012. §12.1 and §12.3 updated (holes, generated scenery, order, cell step). Invariants CO10–CO11, worked examples §14.7–§14.8, corrections for `landblock.md` §3.5–§3.6 and `terrain-texturing.md` §4.5, open questions Q3–Q5, Q8, Q9 and Q12 updated. §10.6 cites the model-scale rule of `appearance.md` §6.1 as the Decision it is there. |
| 2026-10-05 | Revision answering the spec review (third pass, OBS). §3.2: terrain-tree x and y origins are stored as 0xBD4CCCCB (−0.04999999), not the f32 nearest −0.05; C4 extended. §4.2: the closure test behind the counts is stated (each directed edge exactly once, its reverse exactly once); inside-out count corrected from 50 to 51; the looser test's counts added. §5.4: hull vertices within 5.2 mm (one record at 5.17 mm), not 5 mm; the last plane faces up (z > 0.999) in 947 records and is the most upward-facing in 960, the old "floor in 960" was the latter; §5.8 tolerance remark adjusted. §6.3: Q4 is a proper OPEN callout. §6.5: the unverified "presumably under other geometry" is now an OPEN callout. §6.1: note on the client's terrain point queries (`scenery.md` S25). §9.4: frill-object census recounted as 1,515 (the reach rule of `scenery.md` §8.2); the earlier 1,512 counted only the frill layers of named scenes. |
| 2026-10-06 | Movement revision for M4.5 criterion 4. §9.3: the census table is marked a data census (no run-time content-layer or creation-type filter; draw column uses the plain suppression, not E22); the "what blocks a walker" rule now applies the content-layer test (default *L* = 1, *I* = 0) and drops creation types 7 and 8; the two OPENs are resolved with the OBS counts (data 82,139 / 97,688; run-time 73,509 / 88,749; with 7/8 dropped 73,508 / 88,748; 1,651,097 → 1,607,212 → 1,606,311 total). New narrow OPEN: BIN did not trace that the collision **body** is gated on the content-layer test (Tessera gates it as a Decision). §11.3: new BIN mechanism paragraph (C28) — the mover uses the middleware's character **proxy**; the walkable/slide/wall decision is of the surface **underfoot**, a too-steep support is uncontrolled (state 6, slide) and a vertical plane blocks uphill motion; resolves SL1–SL3. §12.3 step 6 rewritten to classify the support underfoot and enter state 6 on a steep support regardless of the previous step; OPEN (e) and (f) revised so the frontal-wall treatment never suppresses a steep support. New facts C27, C28; invariant CO12; BIN row third pass (strings/RTTI only); Q9 partly answered. |
| 2026-10-07 | Revision for incident 0018 (decision 0018): the spec body now states every client fact the released `tessera-physics` relies on, and every place where it does something else is a Tessera Decision or an OPEN. Header: status Review; the long review records moved to the dirty-room review log, a short record kept. Provenance: BIN third pass corrected (no character-proxy class in the executable; the proxy conclusion withdrawn); BIN fourth pass (13)–(19); OBS fourth and fifth passes; C17, C25, C27, C28 restated; new C29–C36. §3.1, §3.7: the three implementation OPENs of 2026-10-06 become Tessera Decisions. §5.3: flags of the cell body. §5.8.1: lookup Decisions (cells without offset, candidate box, precision, duplicate numbers, across landblocks). §6.1: point-query edges; ceiling variant a Decision. §6.6: order and precision Decisions. §7: detail geometry Decision. §8 rewritten: one compound shape per setup and the per-part scale rules (§8.1, BIN), the 0.05 m convex radius (§8.2, BIN), Tessera's representation and its differences (§8.3), with an implementer OPEN; answers the two §8 OPENs of 2026-10-06. §9.1: lookup includes 0x78 extra sets (BIN, `scenery.md` S31) and the counts with them; answers the OPEN of 2026-10-06. §9.2: Tessera Decisions on bodies of ethereal objects and query categories. §9.3 rewritten: census recomputed over the 2,380,675 Physics SEDs with a position (the table of 2026-10-06 summed to 2,380,699 and counted the mixed setups apart); the content-layer test and creation-type filter are BIN (C25, C29); the filter reads the SED's own creation type first, so in this install it removes only the one Physics SED whose effective creation type is 0 (the chain-only reading would remove 901); answers both 2026-10-07 implementer OPENs and resolves the §9.3/§9.4 inconsistency; new implementer OPEN (the released code reads the chain only). §9.4: generated objects are created whatever their creation type (BIN). §10.1, §10.3: names of the layer and MoveTo properties (OBS), hostile-value and built-in-record Decisions. §10.6: capsule placement Decision. §11.3: face-normal rule with absolute values; the middleware-documentation paragraph and the class names removed; C28 restated as BIN flags plus a Probable reading; new §11.3.1 (support search and crease rule), §11.4 (move procedure), §11.5 (step-up), all BIN. §12 rewritten: world representation, mover and step restated; §12.4 lists the readings (a)–(w) as Decisions (the 2026-10-06 and 2026-10-07 implementer OPENs, plus (u), (v), (w) that the code cites), with the geometric criterion for steep contacts (q); §12.5 tabulates where Tessera differs from §11.3.1–§11.5, with an implementer OPEN. §13: CO12 revised, CO13 (state 6, synthetic), CO14, CO15. §14.9: worked example of riding onto a steep bank. §16: Q9, Q12 updated; Q13, Q14 new. Change log: the 2026-10-06 row's stray third cell removed and the rows put in date order. |
| 2026-10-07 | Second revision, answering the review of 2026-10-07 (F1–F7). **F1:** §11.3 states the client's face-normal rule in full (stored corner order, body rotation, triangles only, strict absolute compares). The root kind of the flip is identified (BIN fifth pass, C37) as the middleware's generic tree kind, which no traced builder produces, so the flip is expected never to apply; new OPEN with a Tessera Decision (no Tessera body gets the flip). New §12.4 (x) states Tessera's capsule-side face normal and signed compares as a Decision, (q) cites it, and §12.5 has a new "Face normal" row. **F2:** §6.3 rewritten: the loader builds no tree at load time, and a table gives the three physics bodies (mesh with tree; height field; bare mesh); Q4 updated. **F3:** the first 2026-10-07 row no longer says that the creation-type filter removes nothing (it removes the one creation-type-0 SED). **F4:** §9.3 item 3: no value anywhere counts as 0 and is rejected (BIN, fifth pass, question 23); C29 extended. **F5:** §9.3 states that the layer list and the scale use the SED-then-chain lookup, and the implementer OPEN notes that the released code reads the SED only. **F6:** §12.4 (f) gives the jump-step support rule (no support in the step a jump starts, also in water, so wading 1 → 2) and defines "rising in the air". **F7:** §16 rows Q15 (§6.5 OPEN), Q16 (§9.4 implementation OPEN) and Q17 (face-normal root kind). The §9.4 implementation OPEN is answered: (a) the generated-object instance ID layout (BIN fifth pass, C38; §15 row for `scenery.md` §7.6), (b) no scene object carries a liquid monitor (OBS sixth pass, C39). BIN row: fifth pass (20)–(23). OBS row: sixth pass. Q13 now covers the face-normal rule. |
| 2026-10-08 | Revision answering every implementer OPEN of 2026-10-07 from the binary (BIN sixth pass, questions 24–30; OBS seventh pass). Status Revised. **Confirmed as the client's behaviour:** §5.3 (cell lookup independent of the cell body, which reads no property; no client size limit; C40), §9.1 boolean value types, §9.3 points 3 and 4, §11.1 condition types (C41). **Changed (client behaviour stated):** §9.1 ethereality reads only Bitfield32 (type 18); §9.3 item 3 and point 1: the creation type is read only from an enum or property-ID value, a 32-bit integer counts as 0 (and §15 row for `entity-descriptor.md` §4.9); §9.3 point 2 and §8.1: the physics scale is uniform, z of `Physics_AdjustableScale` × z of `Model_Scale`, with the client's fallbacks (C42), and Tessera's scale bullet of §8.3 follows it; §10.6 and §12.2: the capsule is scaled by that physics scale, not the render model scale (Elf male and High-elf capsules become 3–5 % smaller; C18). **New client facts:** §8.2 cell physics meshes carry the 0.05 m radius (C32 now BIN); §6.1 table of the stored corner order of all terrain and ceiling triangles (C43), §6.4 pointer, §14.4 example aligned to it; new §11.3.2: `Entity_Element` from the support flags and the mover's `Volume_IsLiquid` (C28 now BIN), with §10.4 and the §11.5 OPEN narrowed. **Tessera Decisions recorded:** §12.4 (g), (k), (u), (v), (w) carry the solver details of the implementer OPEN, new (y) re-limits the speed after the slide; §8.3 and §12.5 OPENs answered (no client fact missing; adoption of the radius stays a Decision, Q13). New invariants CO16, CO17; Q9 and Q13 updated; editorial fixes in §12.4 (x) item 2 and before §14. |
