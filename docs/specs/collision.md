<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Collision geometry, cells and portals, and character movement

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), 2026-10-05: changes requested (first round). Re-review 2026-10-05: passed (CLEAN_ROOM.md §5.3, every item; M1 spot checks over all records: terrain/ceiling tree origin bits 0xBD4CCCCB in 58,546 / 1,095 trees, 0x66 closure counts under both tests, 0x01 hull-vertex distances, last-plane and plane-touch counts, hull-vertex plane incidence, §9.4 frill census 1,515) |
| **Implements into** | `crates/formats` (0x66 and 0x01 decoders, quantised-tree reader), `crates/world` (cells, portals, point-in-cell, collision residency), `crates/terrain` (terrain collision triangles), a collision/movement module (new crate or `crates/world`; Tessera's own acceleration structure, sweeps and the character controller of §12), `apps/client` (walk mode on collision instead of terrain heights), `apps/dattool` / `apps/explorer` (inspection), `fuzz/` (one target per record kind and one for the tree reader) |
| **Companion specs** | `mesh.md` §8–§10 (detail levels, per-level collision mesh, tree block; **corrected** here in §7 and §14). `landblock.md` §3.1, §3.6, §3.9, §3.11 (height grid, split diagonal, collision tree blocks; §4.4 cells; **extended** here in §6 and §5.8). `setup.md` §2.2, §6.4, §6.5 (0x01 records, collision volumes, 0x66 DID and physics bounds; **completed** here in §4, §5 and §8). `entity-descriptor.md` §4.5, §9 (SED properties, draw rule; collision rule added here in §9). `scenery.md` §7 (procedural scenery; its clearance step uses §6.6, its objects collide per §9.4). `terrain-texturing.md` §4.5 (terrain holes; corrected here in §6.5). [`water.md`](water.md) §6–§7 (landblock water, water volumes; answers the water OPENs of §10.4). `property-system.md` §4–§8 (values, embedded sets, DID maps). `records.md` §1.2 (VLE), §2.1 (prefix table). `coordinates.md` §11.1 (rotations). `appearance.md` §6.1 (model scale). `wstate.md` (0x70 objects). |

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
  from the data, and the rules found in the client for walkable slopes and parameter
  selection (§10–§11);
- a character controller for Tessera built from those facts, marked as a decision where the
  original's rule is still unknown (§12);
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

- **Confirmed**: observed in the user's install on 2026-10-05 by throwaway probes in the
  dirty-room scratchpad, over **all** records of the kind unless a sample is stated.
- **BIN**: read from the client executable under decision 0011 (method M4); see the BIN row.
  "Confirmed (BIN)" means the fact rests on the executable only.
- **Probable**: supported by data statistics or names, not by an exact rule.
- **Reported**: from published material only.

| Tag | Source |
|---|---|
| OBS | User install, Steam, data current on 2026-10-05. Read in full: all 1,057 records with prefix 0x01 and all 16,819 with prefix 0x66 (`client_general.dat`); all 28,555 setups (0x04); all 40,643 entity descriptors (0x47), all 1,616 stand-alone property sets (0x78) and all 383,332 object property sets (0x79) (`client_gamelogic.dat`); all 47,563 mesh records (`client_mesh.dat`, `client_mesh_aux_1.datx`); all 390,150 landblock-data and 49,636 landblock-info records of `client_cell_{1,2,3,4,5,14,15}.dat`. Every quantised code block of every 0x66, 0x01 and landblock record and of every mesh level was walked; every triangle that a code block contains was looked up through the code at its centroid and at its three corners. Second pass, same date: the surface layers, ceiling grids and stored ceiling triangles of all 390,150 landblock-data records (6,867 with a ceiling) for §6.4–§6.5, and the 0x47 chains, setups and physics properties of the 4,261 objects named by scene records for §9.4. Third pass, same date (answering the review): the stored x and y origin of all 58,546 terrain trees and the x origin of all 1,095 ceiling trees (§3.2); edge pairing and signed volume of all 16,819 0x66 meshes under both closure tests (§4.2); last-plane normals and hull-vertex distances of all 1,057 0x01 records (§5.4); the frill-object census recounted with the reach rule of `scenery.md` §8.2 (§9.4). |
| BIN | Client executable `x64/lotroclient64.exe`, build 4906.0071.1794.4035 (changelist 711794), read with a disassembler (capstone) on 2026-10-05. **Questions:** (1) which rule turns the per-state movement records into the active parameters; (2) the walkable-slope limit; (3) how the per-avatar float height reaches the controller; (4) what the 16 movement properties are as a unit; (5) what `Physics_NoPhysics` and the shape of a moving entity mean to the client. Second pass, same build and date: (6) how the client builds terrain and ceiling collision (holes, ceiling diagonals, landblocks without a stored tree); (7) how it decides which cell contains a point; (8) what collision it gives to procedurally generated scenery and in which order the pieces are built; (9) the terrain-triangle query used by scenery placement; (10) the instance-ID test for player characters. **Why M1–M3 were not enough:** the data holds the parameter records but not the rule that picks and layers them, and holds no slope limit at all; which triangles the client puts into its collision shapes, how it assigns points to cells and how generated scenery becomes physical are run-time behaviour that no record stores; the original client cannot be observed here (M3) and no published source describes these rules (M2). Only the offline world and movement code was read; nothing about network, login, keys or anti-tamper. |
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
| C17 | Support classification: walkable if the surface normal's up component exceeds cos 45° (player characters) or cos 70° (other movers); sliding support up to cos 85° (§11.3) | BIN | Confirmed (BIN) |
| C18 | Player collision capsule from the setup, scaled by the model scale (§10.6) | SET; OBS (all playable setups) | Confirmed (shape); use by the controller Probable |
| C19 | Ethereality bitfields decide which movers pass through an object (§9.2) | OBS (census); names | Probable |
| C20 | The client builds a terrain collision mesh of all 2,048 grid triangles for every landblock, numbered as in §6.2; no triangle is left out for holes (§6.1, §6.5) | BIN (6) | BIN |
| C21 | Ceiling collision: same numbering, diagonal from **bit 0** of the vertex-flag byte, triangles clockwise from above, built from all ceiling samples (§6.4) | BIN (6); OBS (5,310,848 of 5,310,848 stored ceiling triangles follow bit 0) | Confirmed |
| C22 | Terrain triangles covered by no layer occur only in landblocks with a ceiling, and never where the ceiling lies above the terrain (§6.5) | OBS (all 390,150 records; 2,750,134 such triangles) | Confirmed |
| C23 | Each cell is registered as a solid body (its physics mesh) and a phantom (hull of its hull vertices); a point is in a cell when every volume plane gives at least −0.0002 (§5.4, §5.8.1) | BIN (7) | BIN |
| C24 | Terrain triangles near a point: the client's radius query (§6.6) | BIN (9) | BIN |
| C25 | Generated scenery becomes static entities through the same creation request as landblock-info statics; frills never become entities; build order (§9.4) | BIN (8); OBS (census) | BIN (path, order); Confirmed (census) |
| C26 | A mover is a player character for the slope rule when its instance ID's top byte is 2 (§11.3) | BIN (10); PUB2 (meaning of the byte) | BIN (test); Reported (meaning) |

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
counter-clockwise from above (the key-*base* triangle of a quad with the
south-west/north-east diagonal has its corners in the order (i, j + 1), (i, j),
(i + 1, j + 1)). The mesh is built from the heights alone: the surface layers are not
consulted and no triangle is ever left out (§6.5).

The client's terrain height and terrain plane queries at a point pick one triangle of this
mesh by the rules of `scenery.md` §7.4 step 8 (BIN there, S25), which are not always the
triangle under the point: on the diagonal of a bit-7-set quad they take the north-west
triangle, and in the band 4 < fx + fy ≤ 5 (units) of a bit-7-clear quad the north-east one.
Collision itself uses the triangles, so this matters only to code that reproduces the
client's point queries (scenery placement); Tessera's controller (§12) collides with the
triangles.

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
tree. When the record stores a tree, the client uses it as the mesh's bounding-volume code;
when it does not, the client can build one from the same mesh at load time. Either way the
triangles are those of §6.1.

> **OPEN (Q4):** the condition under which the client builds a tree at load time for a
> landblock without a stored one, and what it uses for landblocks whose quads all share the
> south-west/north-east diagonal (both traced only in part). Tessera does not need the
> answer: it builds its own acceleration structure over the triangles of §6.1 (§3.8).

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
for a given diagonal, with the corners of each triangle in reverse order.

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

---

<a id="s8"></a>
## 8. Collision volumes of setups

`setup.md` §6.4 gives the box, sphere and capsule layouts and §6.5 the collision mesh DID
and the physics bounds; §4 above completes the 0x66 record. Facts added here (Confirmed):

- Volume census over 28,555 setups: mesh only 16,812, sphere only 1,910, box only 990,
  capsule only 715, none 8,117, mixtures 11 (`setup.md` §6.5).
- The capsule is vertical: it runs from its lower cap centre straight up by `length`
  (`setup.md` §6.4); every playable character setup has exactly one capsule and nothing
  else (§10.6).
- Moving entities use **one** convex shape (a single box, capsule or sphere) (BIN: the client
  rejects mobile entities with any other collision as a configuration error). Static objects
  may use the mesh.

---

<a id="s9"></a>
## 9. Per-instance collision flags

<a id="s9-1"></a>
### 9.1 Properties (Confirmed census)

Resolved like the draw flags of `entity-descriptor.md` §9: the SED's own value wins, then
the 0x47 chain from the named object upward.

| Property | Type | Where (count) | Meaning |
|---|---|---|---|
| `Physics_NoPhysics` | boolean | SEDs: true 76,508, false 19,938; 0x47: true 777, false 21 | true: the instance has no physics shape (no collision). BIN: a mover with it set has no shape. |
| `Physics_AdjustableScale` | vector | every SED (equal to the render scale, `entity-descriptor.md` §7) | Scales the collision shape |
| `Physics_EtherealityType` | bitfield, mapper 0x230000BA | SEDs 7,276; 0x47 87; 0x79 43,321 | The categories this object belongs to (§9.2) |
| `Physics_EtherealToType` | bitfield, mapper 0x230000BA | SEDs 26,095; 0x47 2,639; 0x79 43,370 | The categories of movers that pass through this object (§9.2) |
| `Physics_PlacementEtherealToType` | bitfield, same mapper | SEDs 3,292; 0x79 43,384 | As above, for placement tests |
| `Physics_AlwaysUncontrolledSurface` | boolean (default false) | – | The object never counts as walkable ground (§11.3) |
| `Physics_SurfacePropertiesToApply` | DID (0x78) | 0x47 52; 0x01 655 | Surface material set: `Base_Surface_Type`, `Content_IAmMadeOf`, `SND_Footstep_Surfaces` |
| `Physics_CanLedgeClimb` | boolean (default true) | 0x47 28; 0x01 53 | Ledge climbing allowed |
| `Entity_Clearance_*` boxes and cylinders | struct arrays | SEDs 510,356 boxes, 144,071 cylinders | Placement and path clearance; **not** movement collision (Probable) |

The ethereality mapper's labels (keys 1–30): Default, Camera, Cosmetic, LineOfSight,
DarkSidePlayer, Corpse, OpenDoor, Missile, ClosedDoor, Creature, LightSidePlayer, NPC,
PlayerMadeHotspot, Admin, PlayerImpassable, Player (keys 0x11–0x1E for the last 14, keys 1–2
for the first two; bit *k* − 1 stands for key *k*, `property-system.md` §7.3). Examples: the
player entities of all species carry EtherealTo = {Camera}; 2,302 creature and NPC entities
carry {Default, Camera, Corpse, OpenDoor, Missile, ClosedDoor, Creature, NPC, Admin, Player}.

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

<a id="s9-3"></a>
### 9.3 Which placed objects block a walker (Confirmed census)

Physics SEDs (type 7) with a position, classified by the resolved chain (§9.1) and the
setup's collision (§8):

| Setup collision | NoPhysics | Drawn (`entity-descriptor.md` §9) | Ethereal to Player | SEDs |
|---|---|---|---|---|
| mesh | false | yes | no | 1,409,434 |
| none | false | yes | no | 400,661 |
| none | false | no | no | 177,375 |
| volumes | false | yes | no | 143,857 |
| **mesh** | **false** | **no** | **no** | **82,139** (invisible walls and blockers) |
| mesh | true | yes | no | 68,074 |
| volumes | true | yes | either | 44,144 |
| volumes | false | no | no | 15,549 |
| volumes | false | yes | yes | 15,004 |
| other combinations | | | | 24,462 |

**Collision rule (for Tessera):** a SED blocks a walker when its type is Physics, it has a
position, it passes the content-layer filter (`entity-descriptor.md` §4.5.1), its effective
`Physics_NoPhysics` is not true, its setup has a 0x66 mesh or volumes, and its effective
`Physics_EtherealToType` does not contain Player. The draw flags play **no** part: 97,688
undrawn SEDs block movement. This rule is a Tessera Decision resting on the census above
and on the BIN fact that a mover with `Physics_NoPhysics` has no shape (§9.1).
Procedurally generated scenery: §9.4.

<a id="s9-4"></a>
### 9.4 Procedurally generated scenery

Objects placed by `scenery.md` §7 are not SEDs. BIN (C25):

- After generation the client turns every generated object into the same static-entity
  creation request that it uses for landblock-info statics (request kind 4 instead of 2),
  carrying the object's 0x47 DID, position, rotation, scale and `Entity_CreationType`, with
  an instance ID synthesized from the landblock and the object's index in the generated
  list (`scenery.md` §7.6). From there, generated objects and statics take the same path.
- Landblock-info statics pass two filters on that path that generated objects do not: the
  client drops statics whose creation type is UniqueDynamicMobile or UniqueDynamicImmobile
  (`Entity_CreationType` 7 or 8; the server creates those) and statics whose
  `Entity_ContentLayers` its content-layer check rejects.
- Frill objects (`scenery.md` §8) are never entities and never collide.
- **Order of building** for a landblock: (1) the terrain and ceiling collision meshes
  (§6.1, §6.4); (2) the static list of landblock-info entities; (3) scenery generation,
  which reads only the terrain mesh (plane and radius queries, §6.6) and the statics'
  clearance volumes, never the collision of objects, cells or other scenery; (4) the
  creation of all statics, generated objects included, with their collision.

**Collision rule for generated objects (Tessera Decision following §9.3):** a generated
object blocks a walker when its effective `Physics_NoPhysics` (resolved along its 0x47
chain; there is no SED-level value) is not true, its setup has a 0x66 mesh or volumes, and
its effective `Physics_EtherealToType` does not contain Player. Its shape is the setup's,
at the generated position and rotation, scaled by the generated scale (uniform in this
install). No content-layer or position filter applies.

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
> creation types (SharedClientOnlyImmobile and similar). Tessera Decision: it does.

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
| `GenericPhysicsController_VelocityDamper` | vector | Per-axis damping of velocity (OPEN: factor per second or per step) |
| `GenericPhysicsController_RampTime` | float | Seconds to reach the target speed (Probable) |
| `GenericPhysicsController_Gravity` | float | Downward acceleration, m/s² (negative = upward, used for buoyancy) (Probable) |
| `GenericPhysicsController_PhysicsTerminalVelocity` | float | Speed limit, m/s (Probable) |
| `GenericPhysicsController_JumpAcceleration` | float | Upward acceleration of a jump (OPEN: duration of its application) |
| `GenericPhysicsController_StepUpHeight` | float | Highest step climbed without jumping, m (Probable) |
| `GenericPhysicsController_FloatHeight` | float | Depth at which a mover floats in water, m (§10.5; Probable) |
| `GenericPhysicsController_ForceStrafe` | boolean | OPEN |
| `GenericPhysicsController_AllowContactLoss` | boolean | OPEN (leaving the ground on bumps?) |

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
water (§6.2–§6.3), the landblock water-volume entity (§7.1) and placed volumes (§7.3). How
the client switches between the water states stays OPEN there (`water.md` §7.4); its Tessera
rule is what §12.3 step 7 uses.

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

The capsule is multiplied by the model scale of `appearance.md` §6.1 (template `vec3` ×
entity `Render_AdjustableScale`, and the instance scale where one is set; the composition is
recorded there as a Tessera Decision). With that rule every playable species ends up with nearly the same
capsule: radius 0.35 m and height 1.80–1.81 m for Hobbit, Human, Dwarf, Stout-axe dwarf,
River-hobbit (setup × 0.7) and High-elf (setup × 1.05, female × 1.03 × 1.02); radius 0.36 m
and height 1.85–1.86 m for Elf and Beorning (scale 1.02–1.03). So the original does **not**
size the collision capsule by species height; Hobbits and Dwarves differ from Men only in
float height (§10.5). Whether the controller uses this capsule directly is Probable (BIN
shows a support query radius of at least 0.3 m built from the mover's shape radius, §11.3).

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

For each contact found around a mover, the client takes the unit contact normal; when the
contacted triangle's own face normal is steeper than the walkable limit and less upward than
the contact normal, it uses the face normal instead. With *u* the up (z) component of the
chosen normal:

| Condition | Contact counts as |
|---|---|
| contacted object has `Physics_AlwaysUncontrolledSurface` true | ignored as support |
| *u* < 0 | ignored as support (overhang) |
| *u* > cos *α*, *α* = **45°** for player characters, **70°** for other movers | walkable ("controlled") support |
| otherwise, *u* > cos 85° | steep ("uncontrolled") support: the mover slides (state 6) |
| otherwise | wall, not support |

A mover counts as a player character when the top byte of its 64-bit instance ID is 2: the
client's test is exactly (instance ID ≫ 56) = 2 (BIN). That this byte marks player
characters is Reported (PUB2); landblock-info entities use 1 and 4
(`entity-descriptor.md` §4.2). An offline client has no server-assigned IDs; Tessera
Decision: the local avatar counts as a player character. The client keeps the list of
contact normals, merging normals that differ by at most 0.035 in every component (the planes
a plane-based solver slides along). The support search uses a radius of at least 0.3 m (the
larger of 0.3 and the mover's shape radius times its scale).

The three angles, the 0.035 merge tolerance and the 0.3 m radius are constants of the
client (BIN), not data. They are physics limits and tolerances, which [decision
0012](../decisions/0012-numeric-constants.md) allows specs to state and Tessera's code to
contain; the same holds for the tolerances of §5.4, §5.8.1 and §6.6.

> **OPEN (RE tasks, §16):** how jump acceleration, ramp time, control acceleration and the
> damper are integrated per step; the step-up procedure; the solver that resolves several
> contact planes; how `Entity_Element` switches between water states; any fall damage (no
> client property names one; probably decided by the server). The second BIN pass looked for
> the controller's integration routine without locating it; §12.3 therefore stays a Tessera
> Decision.

---

<a id="s12"></a>
## 12. Tessera's controller (Decision, built from §3–§11)

This section is a design for Tessera, not a description of the original. Where it fills a
gap left OPEN above it says so; it must be revised when the gap closes.

<a id="s12-1"></a>
### 12.1 World representation

- **Static triangle sets per landblock:** all 2,048 terrain triangles (§6.1, holes
  included, §6.5); the ceiling triangles of landblocks with a ceiling (§6.4); for every SED
  that blocks (§9.3) and every generated scenery object that blocks (§9.4) the 0x66
  triangles or the volumes of its setup, transformed by its placement and scale; for every
  cell the physics mesh (§5.3) in the landblock frame (§5.8).
- **Cells:** per landblock the cell volumes and hulls for the cell lookup of §5.8.1.
- **Order:** terrain and ceiling meshes first, then scenery generation (`scenery.md` §7,
  which needs only the terrain mesh and the statics' clearance volumes), then the statics'
  and generated objects' shapes (§9.4). This is the original's order (BIN); the
  acceleration structure is Tessera's own.
- **Acceleration:** one bounding-volume hierarchy per landblock over those triangles plus a
  list of primitive volumes, built off the main thread when a landblock becomes resident. All
  triangles are two-sided (§4.2).

<a id="s12-2"></a>
### 12.2 Mover

- Shape: the capsule of the avatar's setup times the model scale (§10.6).
- Parameters: §11.1 layering over the records of §10.3, re-evaluated when any condition
  changes; float height §11.2.
- State: `Entity_Element` values of §10.4, decided each step from contacts (§11.3) and water.

<a id="s12-3"></a>
### 12.3 Step (Decision where the original is OPEN)

1. **Desired velocity:** from the input direction in the avatar's frame and the target speeds
   (forward, backward, strafe). Move the horizontal velocity toward it at the control
   acceleration, never faster than reaching it within the ramp time (Decision).
2. **Vertical:** on walkable support, vertical speed is 0 unless a jump starts. A jump applies
   the jump acceleration for one ramp time (0.13 s for players), giving 5.2 m/s and, under the
   falling gravity of 15 m/s², an apex of about 0.9 m and 0.7 s in the air (Decision: the
   original's integration is OPEN). In the air or on steep support add the layer's gravity;
   clamp speed to the terminal velocity.
3. **Damping:** multiply each velocity component by its damper factor raised to (step / 1 s)
   (Decision).
4. **Sweep and slide:** sweep the capsule along velocity × step; on contact, keep the motion
   up to the contact, remove the velocity component into the contact plane, and repeat with
   the remainder, at most 4 times per step.
5. **Step-up:** when a horizontal sweep is blocked, retry from the position raised by the
   step-up height, then sweep down by up to step-up height plus 0.05 m; accept when the new
   support is walkable (§11.3).
6. **Ground snap and state:** sweep down by 0.05 m (plus step-up height when the mover was on
   walkable support and moving down a slope); classify the support with §11.3 to choose state
   4, 5 or 6.
7. **Water:** with the water surface and volumes of `water.md` §6.3 and §7, where the water
   surface lies more than the float height above the feet use state 2, where the feet are in
   water but less deep state 1 or 3 (`water.md` §7.4, Decision).
8. **Cell:** after the step, find the mover's cell with the lookup of §5.8.1 at its position
   (outdoors when no cell contains it). The lookup itself is the original's (BIN); running
   it every step is a Decision (§5.8.1 OPEN).

Fixed step 1/60 s with at most 5 steps per frame.

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

Quad *i* = 3, *j* = 7: *base* = 2 · (32 · 7 + 3) = 454. With bit 7 set at sample (3, 7):
key 454 = corners (3, 7), (4, 8), (3, 8); key 455 = (3, 7), (4, 7), (4, 8). With bit 7 clear:
key 454 = (3, 7), (4, 7), (3, 8); key 455 = (4, 7), (4, 8), (3, 8).

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

---

<a id="s16"></a>
## 16. Open questions

| # | Question | Where | Suggested method |
|---|---|---|---|
| Q1 | Surface property that removes a draw from a mesh-level code | §3.5 | M1 (surface/material flags of 0x310003B4) |
| Q2 | Meaning of the two plane-list terminators, the portal lead byte, portal corner winding | §5.4, §5.5 | M1 |
| Q3 | Neighbour flag byte; non-coincident portal pairs. (Entering a cell from outdoors: answered, §5.8.1.) Remaining: whether a moving character's cell is re-evaluated every step or on phantom overlap | §5.8, §5.8.1 | M4 (movement code), M1 (cross links, door entities) |
| Q4 | Partly answered (§6.3): the client always builds the full mesh and uses the stored tree when there is one. Remaining: when it builds a tree at load time and what it uses for landblocks whose quads all share the south-west/north-east diagonal | §6.3 | M4 |
| Q5 | Answered: ceiling diagonal from bit 0; zero ceiling samples are part of the ceiling mesh (§6.4) | §6.4 | — |
| Q6 | Use of per-level collision meshes and trees | §7 | M4 |
| Q7 | Exact ethereality test and default categories | §9.2 | M4 |
| Q8 | Placement of water volumes and depth query: owned by `water.md` | §10.4 | — |
| Q9 | Integration of jump, ramp, control acceleration and damper; step-up procedure; plane solver; water-state switching | §11.3 | M4 (routine not located in the second pass), then M3 (owner measurements: jump height and airtime, walk and run speeds, highest climbable step and slope) |
| Q10 | Fall damage | §11.3 | probably server-side; nothing in the client data |
| Q11 | `ForceStrafe`, `AllowContactLoss`, `Entity_FinalElement` | §10.1, §10.4 | M4 |
| Q12 | Physics bodies for generated objects of client-only creation types | §9.4 | M4 |

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version: quantised tree, 0x66 and 0x01 layouts, cells and portals, terrain and ceiling trees, per-level mesh trees, setup volumes, per-instance flags, movement data and the BIN rules for parameter layering, float height and slopes; Tessera's controller. |
| 2026-10-05 | Second pass (BIN and OBS). §6.1: the client builds the full 2,048-triangle terrain mesh for every landblock. §6.3: answers what is used without a stored tree, in part. §6.4: the ceiling diagonal is bit 0 of the vertex-flag byte (all 5,310,848 stored ceiling triangles), ceilings face down, zero samples are in the mesh. New §6.5 (terrain holes do not need cutting: they occur only under ceilings and never under an open cave; the client keeps them in its collision) and §6.6 (the client's terrain-triangle radius query used by scenery placement). §5.3: the cell physics mesh is the cell's solid body. §5.4: the client's point-in-volume tolerance −0.0002. New §5.8.1: cell phantoms and the client's lookup of the cell containing a point (entering a cell from outdoors needs no portal). New §9.4: collision of procedurally generated scenery, the creation path, the build order and a census. §10.4 and §12.3 step 7 point to `water.md` for water volumes. §11.3: the instance-ID test (BIN) with a published source for the meaning of byte 2 (PUB2); the binary-sourced constants are recorded under decision 0012. §12.1 and §12.3 updated (holes, generated scenery, order, cell step). Invariants CO10–CO11, worked examples §14.7–§14.8, corrections for `landblock.md` §3.5–§3.6 and `terrain-texturing.md` §4.5, open questions Q3–Q5, Q8, Q9 and Q12 updated. §10.6 cites the model-scale rule of `appearance.md` §6.1 as the Decision it is there. |
| 2026-10-05 | Revision answering the spec review (third pass, OBS). §3.2: terrain-tree x and y origins are stored as 0xBD4CCCCB (−0.04999999), not the f32 nearest −0.05; C4 extended. §4.2: the closure test behind the counts is stated (each directed edge exactly once, its reverse exactly once); inside-out count corrected from 50 to 51; the looser test's counts added. §5.4: hull vertices within 5.2 mm (one record at 5.17 mm), not 5 mm; the last plane faces up (z > 0.999) in 947 records and is the most upward-facing in 960, the old "floor in 960" was the latter; §5.8 tolerance remark adjusted. §6.3: Q4 is a proper OPEN callout. §6.5: the unverified "presumably under other geometry" is now an OPEN callout. §6.1: note on the client's terrain point queries (`scenery.md` S25). §9.4: frill-object census recounted as 1,515 (the reach rule of `scenery.md` §8.2); the earlier 1,512 counted only the frill layers of named scenes. |
