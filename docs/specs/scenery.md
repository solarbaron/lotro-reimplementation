<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Scenery, trees and grass: per-vertex scene selection, scene records, procedural placement and frills

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Claude review session (spec-writer agent), 2026-10-05: changes requested (CLEAN_ROOM.md §5.3; issues listed in the review reply). Re-review 2026-10-05: changes requested (repairs of the first round verified; new issues on §8.1–§8.3 frill records of non-frill objects and inline OPENs in §4.2, §5.1, §8.3 listed in the review reply). Third review 2026-10-06: changes requested. The repairs of the re-review are verified, and S4, S7, S26, S30 (the 452 column) and §5.4 reproduce exactly on the install. Open issue: the property counts of §8.3 (1,515 frill-layer objects) and §9 (2,746 objects) cover only each 0x47 record's own property set. Resolved along the 0x47 parent chain (`entity-descriptor.md` E22 order), they become `Entity_CreationType` 1,515 and 2,746, `Render_Frill_AnimationIntensity` 974, and `Render_LODClass` 1,042 and 2,441. The spec should label which counts it gives, and state (BIN, or OPEN with a Tessera Decision) whether the per-object reads of §8.3 rules 6–7 and the class-2 default of §9 follow that chain. Minor: cite `mesh.md` §6.1 for the bounding box in §8.5 step 5, say which UV sets are copied, and replace a quoted client phrase in §8.5 step 1 with plain words. Repaired 2026-10-06 (change log). Fourth review 2026-10-06: changes requested. The repairs of the third review are verified. §9.2 rules 1–6, its Tessera Decision, S31–S35, SC14, SC15, §8.5 steps 1 and 5 and the own-set/resolved labelling are complete and implementable. An independent probe using the repository's crates read all 4,261 objects, their parents and every 0x78 extra set. It reproduces every own and resolved count of §8.3, §9, §9.2 and §10 (1,515: 82 with a parent, 45/6, 68/0, 54/2, 881 animated; 2,746: 115 with a parent, 65/12, 61/13, `Render_CastLandscapeShadows` 2,147/2,241, clearance lists 1,257/1,325 and 296/354, `Render_ImposterName` 840/844; the 452 have no parent; 1,043 records name one or two extra sets, none with a `Render_*` or `Entity_*` property). Open issues: (1) §9.2 intro and the S1 OBS row say the 197 objects with a parent have "28 distinct parents". They have **156** distinct parents. 128 of them are scene objects themselves and 28 are not, so the 28 is the count of extra records (4,289 = 4,261 + 28). None of the 156 has a parent of its own, so the chain-depth conclusion holds. Correct both sentences. (2) Minor: the §9 `Render_LODClass` value list reads as descending but skips two classes of 121 / 121 each (0x10000010 and `SmallDecor`) before `Bush` 107. List them, or say that the list is a selection. Repaired 2026-10-06 (change log). Fifth review, Claude review session (spec-writer agent), 2026-10-06: **passed; status Review → Accepted.** Both issues of the fourth review are repaired and verified by an independent read-only probe (the repository's crates, all records): the 197 objects with a parent have 156 distinct parents, 128 of them scene objects and 28 not, 4,289 records in all, none of the 156 with a parent, 1,043 records naming extra sets; the eleven `Render_LODClass` values of §9 and their own/resolved counts match exactly (sums 2,376 and 2,441) and the labels match enum mapper 0x2300008D. Further spot checks reproduced exactly: (1) S7/§5.1: 3,402 scene records, zero leftover bytes, `group_count` 1/0/2 in 3,398/3/1, frill layer in 2,334, 180 empty, all with frequency 1.0, 2,320 with zero displacement, 8,031 frill-layer children all scene references; (2) §5.2: 62,922 object children, 11,033 scene references, 66,191 nodes (63,846 in groups); (3) §5.3 and SC10: `frequency` ≥ 1 in 25,582 (1.0 in 25,450), `clearance` 0 in 49,405 (all with the SC10 tuple), 0.5 in 15,798, `clear_aspect_tol` 360 in all, `height_mode` 0/1 in 64,789/1,402, `align_to_slope` in 38,992; (4) S5/SC2: 5 records 0x11 with zero leftover bytes, 91 – 1,054 entries, 3,320 choices all with flag 1, 2,472 named scenes; (5) §5.4/S24: 4,261 objects, 2,981 through groups, 1,515 through frill layers (1,512 from named scenes only), 235 in both; and every own/resolved count of §8.3, §9 and §9.2 (82 and 115 with a parent; 45/6, 68/0, 54/2, 65/12, 61/13; `SmallDecor` 746/786; `SharedClientOnlyImmobile` 1,176/1,230; `Render_LODCrossFade` true in 14; `Render_FarViewDistance` 1,000 – 2,000). §5.3 checklist satisfied: functional facts in prose and tables only, no code or pseudo-code, no game text, keys or hostnames, names only for interoperability, provenance marks on every fact, BIN rows (a)–(n) with questions and the M1–M3 justification, unknowns as OPEN callouts with Tessera Decisions, no closed gate. |
| **Implements into** | `crates/formats` (decoders for the region scene tables 0x11, 0x12 and scene records 0x02; the terrain-type table 0x19 decoder of `terrain-texturing.md` §3 and the LOD-class record 0x32 decoder of `environment.md` §9.1, which own those layouts; fuzz targets for each), `crates/world` (per-landblock scenery generation, §7), a frill (grass) layer with its own ring (§8, `crates/world` + `crates/render`), `crates/render` (instanced drawing of scenery and frill objects), `apps/explorer` (scene and scenery views), `apps/client` |
| **Companion specs** | [`landblock.md`](landblock.md) §3.3–§3.8 (height grid, coarse 17 × 17 grid, surface layers and quad grid), §4.5 (static entities), §5 (block maps). [`terrain-texturing.md`](terrain-texturing.md) §3 (terrain-type table 0x19, full entry layout), §13 (height constant). [`collision.md`](collision.md) §6.1–§6.2 (terrain collision mesh and triangle keys), §6.6 (terrain triangles near a point), §9.4 (collision of generated scenery). [`water.md`](water.md) §6.2–§6.4 (per-landblock water surface). [`environment.md`](environment.md) §3 (region descriptor slots), §7.4 (`WindSpeed`), §9.1 (LOD-class record 0x32, owner of its layout). [`entity-descriptor.md`](entity-descriptor.md) (hand-placed static entities). [`setup.md`](setup.md) (0x47 → 0x1F → 0x04 → 0x06 chain; §3.1 parent 0x47 records, §4.4 extra sets, used by §9.2). [`mesh.md`](mesh.md) §6.1 (stored vertex-block bounding box, §8.5). [`coordinates.md`](coordinates.md) §11 (axes, quaternion convention, compass heading). [`property-system.md`](property-system.md) (property sets in 0x47 records). [`records.md`](records.md) §1 (VLE, TSize). |

---

<a id="s0"></a>
## 0. Purpose

Trees, bushes, rocks, flowers and grass in the outdoor world come from three sources:

1. **Hand-placed static entities** in each landblock-info record (`entity-descriptor.md`).
   These are stored with their exact position and need no generation.
2. **Procedural scenery**: for every 10-unit terrain vertex, the landblock data stores a
   terrain-type id and a scene slot (`landblock.md` §3.7). Through two per-region tables
   they select a **scene** record (prefix 0x02), whose placement nodes scatter objects
   (0x47 entity descriptors) around the vertex with seeded pseudo-random displacement,
   heading, scale and frequency, subject to slope, height, terrain-type and spacing checks.
3. **Frills** (grass and small ground cover): the optional *frill layer* of the same scene
   records, generated by a separate frill engine in 10 × 10-unit sectors around the camera,
   thinned by a density option and drawn with distance fade.

This spec gives the record layouts (§3–§5), the hash functions (§6), the generation
algorithm in words with its orientation, terrain, clearance and spacing rules (§7), the
frill system with its draw distances and fades (§8), what is known about the objects, the
LOD-class record and wind (§9–§10), the census of this install (§12) and the invariants
(§13). With it an implementer can place the same scenery objects at the same positions and
orientations as the original client, apart from the OPEN items listed in §15.

Where this spec describes a rule of the original client it says so and gives the source
(Confirmed or BIN). Where Tessera may or must choose for itself, the text says
**Tessera Decision**.

All counts are observations of one install on one date. Tests assert the invariants, not the
counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-05 by throwaway probes in the dirty-room scratchpad: **all** 5 records of each of the prefixes 0x11, 0x12, 0x17 and 0x19, **all** 3,402 records 0x02, the coarse grid, heights and surface quad grid of **all** 390,150 landblock-data records, and **all** 4,261 distinct 0x47 records referenced by scenes (properties read with the repository's own property-set decoder; meshes resolved with its setup resolver and mesh summary). Second pass, same date: the record 0x32000000 (§9.1) and the clearance fields of all 66,191 placement nodes (§5.3, SC10). Third pass, same date (answering the review): the default entry of all 5 records 0x12 (§4.2), the empty scenes and frill-layer children of all 3,402 records 0x02 (§5.4), the `frequency` census (§5.3), the objects reachable through frill layers (§5.4, §8.2). Fourth pass, 2026-10-06 (answering the re-review): the group objects that the frill pass meets after a frill layer (§8.1), the part, detail-level and draw-entry census of all 1,967 objects that can become frill records (meshes read with the repository's mesh decoder; §8.3), and the properties of the 452 objects among them that no frill layer reaches (§8.3). Fifth pass, 2026-10-06 (answering the third review): the properties of all 4,261 objects resolved along their 0x47 parent chains (197 objects have a parent; 156 distinct parents, 128 of them scene objects themselves and 28 not, none of which has a parent of its own), and the 0x78 extra sets named by all 4,289 records involved, the 4,261 objects and the 28 parents that are not scene objects (1,043 of these records name one or two; every set exists and none holds a `Render_*` or `Entity_*` property) (§9.2). |
| BIN | The client executable of the user's install, `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, compiled 2026-09-27), disassembled in the dirty room under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md). Questions: (a) which record kinds and fields the per-vertex grid indexes and how a scene is chosen; (b) the scene-record grammar and the meaning of each placement-node field; (c) the placement algorithm (seeds, hash, displacement, rotation, scale, height, slope, terrain-type, spacing and bounds checks); (d) how frills are produced and the frill-engine defaults. Second pass, same build, 2026-10-05: (e) the orientation conventions (sign of the heading, axes of the second and third angles, the frame built by `align_to_slope`); (f) which terrain triangle supplies the plane, what the "physics clearance" step queries and tests, and which already-placed objects the spacing step compares against; (g) the frill draw distance per level, fades, reduction and fade-in; (h) which client record type the DID range 0x32… belongs to. Third pass, same build, 2026-10-05: (i) the comparison that picks the terrain triangle when a point lies on a quad's diagonal, and whether the client's general terrain height and terrain plane queries at a point use the same rule; (j) which frill layers the frill pass processes and how the "under a frill layer" state is kept. Fourth pass, same build, 2026-10-06: (k) what the frill engine does with a frill record: whether it filters records by object property, creation type or part count, which geometry of the object it draws, and how it stores the vertices of a sector; (l) which graphics back ends force the frill terrain-colour intensity to 0. Fifth pass, same build, 2026-10-06: (m) how the client resolves a property of an object along its 0x47 chain, which reads of `Entity_CreationType`, `Render_LODClass` and the frill per-object properties go through that resolution, and what an object without `Render_LODClass` gets; (n) which texture-coordinate set the frill bake copies. Why M1–M3 were not enough: procedural placements are **not stored** anywhere in the data (only the parameters are), so neither file observation nor published sources can give the generation rule; no public decoder of these records exists; frame captures (M3) would show results but not the seeds or rules, and none is available to this session. No network, login, launcher or anti-tamper code was examined. |
| LB | [`landblock.md`](landblock.md) (height grid, coarse grid layout, surface chunks, block maps). |
| TT | [`terrain-texturing.md`](terrain-texturing.md) §3 (terrain-type table 0x19 decoded to the last byte), §13 (height constant). |
| COL | [`collision.md`](collision.md) §6.1–§6.2 (terrain collision mesh, triangle keys), §6.6 (terrain triangles near a point), §9.4. |
| WAT | `water.md` §6.2–§6.4 (per-landblock water: presence rule, height, plane, census). |
| ENV | [`environment.md`](environment.md) §3 (region descriptor slot table), §7.4 (`WindSpeed` census). |
| ED | [`entity-descriptor.md`](entity-descriptor.md) (static entities, `WindSpeed` on 18,334 of them). |
| SET | [`setup.md`](setup.md) (entity descriptor → visual template → setup → mesh chain). |
| ER | `research/notes/engine-client-re.md` §3 (a separate frill engine with its own ring, 3 × 3 landblock source groups, frame budget). |

Marks: **Confirmed** = checked over every record of the kind on 2026-10-05 (OBS), or a BIN
fact that the data agrees with on every record; **BIN** = rests on the disassembly only
(cannot be observed in data); **Probable** = supported by a statistical test or by BIN and
data together but not proven for every case; **Reported** = third-party only.

| # | Fact | Sources | Mark |
|---|---|---|---|
| S1 | Coarse-grid word: bits 0–7 terrain-type id, bits 8–14 scene slot, bits 15–19 encounter slot (§3) | BIN (bit extraction); OBS (bits 15–31 are 0 in all 390,150 records) | Confirmed (layout); encounter use BIN |
| S2 | Coarse grid is X-major (index 17·i + j, i along X) with shared edges (§3.2) | BIN (loop and position); OBS (east-west seam equality 31,500 of 31,500 pairs in regions 1–2; Y-major reading 16 %) | Confirmed |
| S3 | Region slot 1 = scene-choice table 0x11, slot 2 = terrain-scene table 0x12, slot 8 = terrain-type table 0x19 (§4.1) | ENV (slot table); BIN (record kinds); OBS (all keys resolve, §13) | Confirmed |
| S4 | Terrain-scene table 0x12 layout; the default entry has the same layout as every other entry (§4.2) | OBS (5 of 5, zero leftover bytes, default entry included; corrected 2026-10-05: an earlier draft gave the default entry one extra byte); BIN (entry field order, lookup rules) | Confirmed |
| S5 | Scene-choice table 0x11 layout and weighted pick (§4.3, §7.2) | OBS (5 of 5, zero leftover); BIN (pick rule) | Confirmed (layout); pick rule BIN |
| S6 | Terrain-type table 0x19: keys are the surface-layer `material` ids; the "procedural content allowed" flag is the entry's `procedural` field (§4.4) | OBS (every layer `material` and every quad-grid value of all 390,150 records is a key; flag 0 on cliffs, ice, tilled soil and paved-road layers); TT §3.3 (entry layout, 4,688 of 4,688 entries); BIN (flag gates placement) | Keys and layout Confirmed; gating BIN |
| S7 | Scene record 0x02 grammar (§5) | BIN (field order); OBS (3,402 of 3,402, zero leftover bytes) | Confirmed |
| S8 | Placement-node field meanings (§5.3) | BIN; OBS (value statistics) | BIN, per field as marked |
| S9 | Hash functions and seeds (§6); constants allowed by decision 0012 | BIN | BIN |
| S10 | Generation algorithm (§7) | BIN | BIN |
| S11 | Frill layer = the scene's optional root node, processed only by the frill engine (§8) | BIN; OBS (2,334 of 3,402 scenes have it; its children are scene references only; 180 have no children) | BIN + Confirmed structure |
| S12 | Frill engine options and defaults (§8.4) | BIN; ER | BIN |
| S13 | Scenery and frill objects are 0x47 records; properties and mesh sizes (§9) | OBS (4,261 of 4,261 resolve to a setup and meshes) | Confirmed |
| S14 | The height modes, water ranges and scale modes 5–7 read a per-landblock water surface (a flag and a plane); its source and height are specified in `water.md` §6.2–§6.3 (§11) | BIN (what the generator reads); WAT | BIN |
| S15 | Wind and frill animation properties (§10) | OBS; ED; ENV | Confirmed (presence); algorithm OPEN |
| S16 | Orientation: the heading turns clockwise seen from above; second angle about X, third about Y; the `align_to_slope` frame; alignment replaces the parent orientation (§7.4 step 10) | BIN (e) | BIN |
| S17 | The terrain plane comes from the client's terrain collision mesh; triangle choice including the band rule for quads with bit 7 clear and the tie on the diagonal for quads with bit 7 set (§7.4 step 8) | BIN (f), (i); COL §6.2 (key numbering Confirmed) | BIN |
| S18 | The clearance step is a flatness test over the terrain triangles near the candidate; meaning of node offsets 143, 147, 151 and 155 (§7.4 step 13, §5.3) | BIN (f); OBS (field census over 66,191 nodes) | BIN; values Confirmed |
| S19 | Spacing compares against the clearance volumes of the landblock-info static entities and against the footprints of already placed nodes, not against generated objects (§7.4 step 14) | BIN (f) | BIN |
| S20 | Node offsets 163 and 174 are not read by the placement code; `max_objects` rule (§5.3, §7.4 step 15) | BIN | BIN |
| S21 | LOD-class record 0x32: layout owned by `environment.md` §9.1 (E15); keys are `Render_LODClass` values (§9.1) | ENV; OBS (1 of 1: payload 1,160 bytes = 8 + 32 × 36, 32 distinct keys); BIN (h): the client maps DIDs 0x32000000–0x32FFFFFF to its LOD-class record type | Layout Confirmed (in `environment.md`); meaning of the values OPEN |
| S22 | Frill radius per draw-distance level, far distance, fade, reduction and fade-in (§8.3) | BIN (g) | BIN |
| S23 | Frill option defaults, including terrain-colour blending on and the dial-back halving (§8.4) | BIN (g) | BIN |
| S24 | The frill pass processes the frill layer of every scene it instantiates, including scenes reached through scene references; the "under a frill layer" state lasts to the end of the vertex; the frill pass uses the landblock's generated list (§8.2) | BIN (j); OBS (1,515 objects reachable this way, 1,512 through the frill layers of the 2,472 scenes named by 0x11 tables alone) | BIN; counts Confirmed |
| S25 | The client's general terrain height and terrain plane queries at a point use the triangle choice of §7.4 step 8, not the geometric triangle of `landblock.md` §3.11 (§7.4 step 8) | BIN (i) | BIN |
| S26 | Frill records come from objects inside a frill layer (1,515 distinct) and from group objects that the frill pass meets after a frill layer in the same vertex (474 distinct in 84 named scenes, 452 of them not among the 1,515); 1,967 objects in all (§8.1) | BIN (j) (rule); OBS fourth pass (counts) | Rule BIN; counts Confirmed |
| S27 | The frill engine filters no frill record by property, creation type or part count; per object it creates one source entity and draws only the first draw entry of detail level 0 of the model's first part (§8.5 steps 1–3) | BIN (k) | BIN |
| S28 | The frill engine bakes every record of a sector into one vertex and one index buffer with 16-bit indices, offset by a running vertex count kept in 16 bits; per-vertex sway weight from the vertex block's bounding box (§8.5 steps 5–6) | BIN (k) | BIN |
| S29 | The frill terrain-colour intensity is 0 under the Direct3D 9 back end built into the executable and under the OpenGL back end (§8.3 rule 7) | BIN (l) | BIN |
| S30 | Mesh and property census of the 1,967 objects that can become frill records (§8.3); property counts over each record's own set (the resolved counts are S33) | OBS fourth pass | Confirmed |
| S31 | Property lookup on a descriptor: the record's own set (its 0x78 extra sets in list order, the embedded set on top), then the parent 0x47 record by the same rule, recursively; the nearest level that has the property wins. Building an entity's property set from a descriptor merges the chain with the same precedence (§9.2 rules 1–3) | BIN (m) | BIN |
| S32 | The client reads `Entity_CreationType` for generated objects and `Render_LODClass` with the lookup of S31; the frill engine reads its per-object values through the client's general property lookup on the source entity (§9.2 rules 4–5) | BIN (m); that the frill source entity holds the merged set of S31: Probable | BIN; Probable for the frill source entity |
| S33 | Property counts of §8.3, §9 and §10, over each record's own set and resolved along the chain (§9.2) | OBS fifth pass | Confirmed |
| S34 | In the one traced reader of `Render_LODClass`, an object with no class anywhere on its chain gets no class value; the registry default (2) is not applied there (§9.2) | BIN (m) | BIN |
| S35 | The frill bake copies only texture-coordinate set 0 of each vertex (§8.5 step 5) | BIN (n) | BIN |

---

<a id="s2"></a>
## 2. Overview of the data chain

For the landblock (R, X, Y) and its 17 × 17 coarse vertices (i, j):

1. The coarse-grid word of vertex (i, j) gives a **terrain-type id** *t* and a **scene slot**
   *s* (§3).
2. The region's **terrain-scene table** (prefix 0x12) maps *t* to a terrain entry, and the
   entry maps *s* to a **scene key** *k* (§4.2).
3. The region's **scene-choice table** (prefix 0x11) maps *k* to a weighted list of
   **scene** DIDs (prefix 0x02); one is chosen with a per-vertex hash (§4.3, §6.1).
4. The **scene** (§5) holds a group of **placement nodes**. Each node, with its own
   probability, places its children (objects, nested nodes or other scenes) around the vertex
   (§7.4).
5. Objects are **0x47 entity descriptors**, drawn through the usual chain to meshes
   (`setup.md`).
6. The scene's optional **frill layer** (root node) is evaluated only by the frill engine
   (§8) and yields grass and ground cover.

The region's **terrain-type table** (prefix 0x19), indexed by the surface `material` ids
of `landblock.md` §3.8, says which surface layers forbid procedural content (§4.4).

---

<a id="s3"></a>
## 3. The per-vertex scenery word (landblock data, coarse grid)

<a id="s3-1"></a>
### 3.1 Bit layout (Confirmed)

The coarse grid of `landblock.md` §3.7 holds 289 u32 words. Each word:

| Bits | Field | Range observed | Meaning |
|---|---|---|---|
| 0–7 | `terrain` | 0–94 | Terrain-type id; looked up by **id** in the terrain-scene table (§4.2). 0 never matches an entry and therefore selects the table's default entry. |
| 8–14 | `slot` | 0–72 | Scene slot within the terrain entry (§4.2), compared with the entry's slot numbers. |
| 15–19 | `encounter` | always 0 | Encounter slot, used only by a separate encounter-placement pass (BIN; §7.7). |
| 20–31 | — | always 0 | |

<a id="s3-2"></a>
### 3.2 Order and position (Confirmed)

Word (i, j), 0 ≤ i, j ≤ 16, is at index **17 · i + j**, where **i runs along X** (east) and
**j along Y** (north), the same order as the height grid. Vertex (i, j) sits at block-local
(10 · i, 10 · j) and coincides with height sample (2i, 2j). Edge words are shared with the
neighbour: word (16, j) of block (X, Y) equals word (0, j) of block (X + 1, Y) in every
adjacent pair tested (regions 1 and 2, 31,500 pairs).

A block whose 289 words are all 0 has no procedural scenery and no frills (312,036 of
390,150 records).

---

<a id="s4"></a>
## 4. Region scene tables

<a id="s4-1"></a>
### 4.1 Finding the tables (Confirmed)

The region descriptor (prefix 0x0F, `environment.md` §3.2) names one table per slot. For
scenery:

| Slot | Prefix | Table | Region 1 | Regions 2, 14 | Region 3 | Regions 4, 5 | Region 6 |
|---|---|---|---|---|---|---|---|
| 1 | 0x11 | Scene-choice table (§4.3) | 0x11000000 | 0x11000001 | 0x11000002 | 0x11000007 | 0x1100000A |
| 2 | 0x12 | Terrain-scene table (§4.2) | 0x12000000 | 0x12000001 | 0x12000002 | 0x12000007 | 0x1200000A |
| 8 | 0x19 | Terrain-type table (§4.4) | 0x19000000 | 0x19000001 | 0x19000002 | 0x19000007 | 0x1900000A |

A client reads the slots at run time; it never hard-codes the DIDs. Slots 1 and 2 of one
region always have the same serial, and the scene keys of a slot-2 table index the slot-1
table with that serial.

<a id="s4-2"></a>
### 4.2 Terrain-scene table, prefix 0x12 (Confirmed, 5 of 5, zero leftover bytes)

All fields little-endian, packed. A **name** is a u8 length *n* followed by *n* Latin-1
bytes; *n* is below 128 in every name of the 5 records. A **pair** is 8 bytes: u32
`scene_key`, u32 `slot`.

> **OPEN:** the encoding of a name whose first length byte is 0x80 or more. The client's
> reader accepts a two-byte length form (BIN, not decoded); the likely form is the VLE length
> of `records.md` §1 (Pascal string), but this is not verified. Readers reject a first length
> byte of 0x80 or more as unsupported; no record contains one.

Record:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | DID | Own DID |
| 2 | reserved | u32 | 0 in all 5 |
| 3 | `has_default` | u8 | 1 in all 5 |
| 4 | default entry | terrain entry (below) | Present when `has_default` = 1 |
| 5 | `count` | u32 | 3 – 99 terrain entries |
| 6 | entries | `count` × terrain entry | |

> **OPEN:** whether the presence bytes `has_default` (record) and `has_default_pair`
> (terrain entry) are bool8 (`records.md` §1.1), so that a reader rejects 2 or more, or a u8
> where any non-zero value means present. Every observed value is 0 or 1. Tessera's decoder
> currently reads both as bool8 and rejects other values (implementer's choice, not a
> Confirmed fact; `crates/formats/src/scenery.rs`).

Terrain entry (the default entry and every entry of the list have this one layout):

| Offset | Field | Type | Meaning |
|---|---|---|---|
| 0 | `name` | name | Internal label of a scene zone or ground type (1 + n bytes) |
| n + 1 | `kind` | u32 | 1 in the default entry; in the list 2 in most entries, 1 in region 6, 3 – 50 in region 1's ground-type entries. Not used by placement; meaning OPEN (callout below the default-entry paragraph) |
| n + 5 | `id` | u32 | **Terrain-type id** matched against bits 0–7 of the coarse word. 0 in the default entry; ids in the list 1 – 204, each occurring once within a record. |
| n + 9 | `unknown_f` | f32 | 0.7071 (the f32 nearest √½) in every entry, the default entry included (the client's default is 0.7). Not used by placement; meaning OPEN (callout below the default-entry paragraph) |
| n + 13 | `has_default_pair` | u8 | 1 in the default entry, 0 in every entry of the list |
| n + 14 | default pair | pair | Present only when `has_default_pair` = 1 |
| … | `pair_count` | u32 | 1 – 73 |
| … | pairs | `pair_count` × pair | (`scene_key`, `slot`). In every entry of the list the first pair is (2, 0). Slots are distinct within an entry; in regions 2, 3, 6 they are 0, 1, 2, … in order, elsewhere a few entries skip numbers. |

The **default entry** is, in all 5 records: a 7-byte name, `kind` 1, `id` 0, `unknown_f`
0.7071, `has_default_pair` 1, default pair (1, 0), `pair_count` 1, pair (1, 0): 1 + 7 + 4 +
4 + 4 + 1 + 8 + 4 + 8 = 41 bytes from its length byte to the end of its last pair, so
`count` follows at record offset 50. Its scene key 1
selects an empty scene (§5.4), so terrain ids that are not in the table get no scenery.
Read with this one entry layout, all 5 records decode to their last byte with zero leftover
bytes, the first pair of every entry of the list is (2, 0), and ids and slots are distinct as
stated (Confirmed, 5 of 5).

> **OPEN:** the meaning of a terrain entry's `kind` (offset n + 1) and `unknown_f` (offset
> n + 9). Neither the lookup below nor the placement code of §7 and §8 reads them (BIN); no
> other reader in the client was traced. Observed values: `kind` 1 in the default entry of
> all 5 records; in the list 2 in most entries, 1 in every entry of region 6, 3 – 50 in the
> ground-type entries of region 1; `unknown_f` 0.7071 in every entry. **Tessera Decision:**
> decode both, keep them for tools, ignore them for placement, and accept any value.

Lookup (BIN):

1. Find the entry whose `id` equals the word's `terrain`. If none, use the default entry.
2. In that entry, find the pair whose `slot` equals the word's `slot` (by value, not by
   position). If none, use the entry's default pair if it has one; otherwise the vertex has
   **no scene** (the client logs a diagnostic).
3. The pair's `scene_key` is looked up in the scene-choice table (§4.3). A key of 0 means no
   scene.

<a id="s4-3"></a>
### 4.3 Scene-choice table, prefix 0x11 (Confirmed, 5 of 5, zero leftover bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | reserved | u32 | 0 (this prefix starts with a u32 before its DID, `environment.md` §4) |
| 2 | did | DID | Own DID |
| 3 | `count` | u32 | 91 – 1,054 entries |
| 4 | entries | `count` × entry | |

Entry:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `key` | u32 | Scene key (1 – 2,755); unique within the record; not sorted |
| 2 | `n` | u32 | 1 – 22 choices |
| 3 | choices | `n` × 9 bytes | DID `scene` (prefix 0x02), f32 `weight`, u8 `flag` |

Observed: `flag` = 1 in all 3,320 choices; `weight` = 1.0 when `n` = 1; with several choices
the weights are arbitrary positive numbers (entry sums 1.0 – 14.6). The total weight of an entry is
the sum of its weights.

> **OPEN:** meaning of the choice `flag` (always 1).

<a id="s4-4"></a>
### 4.4 Terrain-type table, prefix 0x19 (Confirmed; layout in `terrain-texturing.md` §3)

The record is decoded to the last byte in `terrain-texturing.md` §3.2–§3.3 (record and
entry layout) and §3.4 (keys). Scenery uses only two things from it:

- **Lookup by key.** An entry is found by its `key`, never by its stored position: entries
  are stored in a hash order, and a reader builds a key → entry index
  (`terrain-texturing.md` §3.4). The keys are exactly 0 … *n* − 1.
- **The `procedural` field** (bool8, `terrain-texturing.md` §3.3, offset *b* + 25 of the
  entry; 29 bytes before the end of an entry whose curves are both in mode 1):
  **procedural content allowed.** 0 forbids scenery and frills on surface quads of this
  terrain type (§7.4 step 7). 0 in 38 – 179 entries per table (129, 179, 107, 155, 38):
  cliff faces, ice, tilled fields, paved roads and an explicit "scene disabler" layer.

`key` is the surface-layer **`material`** id of `landblock.md` §3.8.2: every layer
`material` (912,038 terrain-chunk layers) and every quad-grid value (88,807,424 quads) of
every landblock-data record is a key of its region's slot-8 table (Confirmed). In this
install: region 1 → 0x19000000, regions 2 and 14 → 0x19000001, region 3 → 0x19000002,
regions 4 and 5 → 0x19000007 (read the slot at run time).

---

<a id="s5"></a>
## 5. Scene records, prefix 0x02 (`client_general.dat`; Confirmed, 3,402 of 3,402)

<a id="s5-1"></a>
### 5.1 Record

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | reserved | u32 | 0 |
| 2 | did | DID | Own DID |
| 3 | `flag_a` | bool8 | 1 in all records. Meaning OPEN (callout below) |
| 4 | `has_frill_layer` | bool8 | 1 in 2,334 records, 0 in 1,068 |
| 5 | frill layer | placement node (§5.3) | Present when `has_frill_layer` = 1 |
| 6 | `group_count` | u32 | 1 (3,398 records), 0 (3), 2 (1) |
| 7 | groups | `group_count` × group | |

> **OPEN:** the meaning of `flag_a` (1 in all 3,402 records). Scene instantiation in the
> static pass (§7.3) and in the frill pass (§8.2) does not read it (BIN); no other reader in
> the client was traced. **Tessera Decision:** decode it, ignore it, and accept 0 as well
> as 1.

Group:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `group_id` | u32 | Always 1. The generator uses the **first** group whose id is 1. |
| 2 | `n` | u32 | Number of children |
| 3 | children | `n` × child (§5.2) | In all records every group child is a placement node (63,846). |

<a id="s5-2"></a>
### 5.2 Child

A u8 `type`, then:

| `type` | Payload | Meaning | Count (all children everywhere) |
|---|---|---|---|
| 1 | DID (4 bytes) | **Object**: a 0x47 entity descriptor | 62,922 |
| 2 | placement node (§5.3) | **Nested node**, placed relative to its parent | 63,846 in groups, 11 nested |
| 3 | DID (4 bytes) | **Scene reference**: another 0x02 scene, instantiated at the parent's placement | 11,033 (8,031 of them in frill layers) |
| 4 | u32, string, u32 | Known to the client's reader; never occurs. Readers reject it as unsupported. | 0 |

Any other type is malformed.

<a id="s5-3"></a>
### 5.3 Placement node (fixed part 187 bytes, then children, then 4 bytes)

Offsets are relative to the first byte after the child `type` byte (or to the start of the
frill layer). "Range (a, b)" fields are pairs of f32. Unless stated, values in the
Observed column are over all 66,191 nodes.

| Offset | Size | Type | Field | Meaning (BIN unless stated) | Observed |
|---|---|---|---|---|---|
| 0 | 12 | 3 × f32 | `disp_min` | Minimum displacement (x, y, z) from the parent position, units | (−5, −5, 0) in 57,108 |
| 12 | 12 | 3 × f32 | `disp_max` | Maximum displacement | (5, 5, 0) in 57,824 |
| 24 | 8 | 2 × f32 | `heading` | Heading range in degrees (min, max); clockwise seen from above (§7.4 step 10) | (0, 359) in 63,425; (0, 0) in 2,572 |
| 32 | 8 | 2 × f32 | `angle2` | Second rotation range, degrees, about the model X axis | always (0, 0) |
| 40 | 8 | 2 × f32 | `angle3` | Third rotation range, degrees, about the model Y axis | always (0, 0) |
| 48 | 4 | f32 | `frequency` | Probability that the node is placed at a given vertex; ≥ 1 means always | 0.0005 – 25; ≥ 1 in 25,582, of which exactly 1.0 in 25,450 |
| 52 | 12 | 3 × f32 | `scale_min` | Minimum scale per axis | (1, 1, 1) in 29,696; (0.8, …) in 16,025 |
| 64 | 12 | 3 × f32 | `scale_max` | Maximum scale per axis | (1, 1, 1) in 21,098; (1.2, …) in 11,427 |
| 76 | 4 | f32 | `stretch_ref` | Divisor for the stretch scale modes 5–7; unused by mode 4 | always 1.0 |
| 80 | 4 | f32 | `slope_max_cos` | Upper bound on the terrain normal's *z* (1 = flat) | 1.0 in 63,980; 0.9397 (20°) in 1,902 |
| 84 | 4 | f32 | `slope_min_cos` | Lower bound on the normal's *z* | 0.766 (40°) in 42,495; ≈ 0 (90°) in 3,512; 0.9397, 0.9848, 0.9063, 0.866 … |
| 88 | 1 | bool8 | `align_to_slope` | Tilt the object so that its up axis is the terrain normal (§7.4 step 10) | 1 in 38,992 |
| 89 | 1 | bool8 | `face_direction` | Add the compass bearing of the terrain normal's horizontal part plus 180° to the heading (§7.4 step 10) | always 0 |
| 90 | 8 | 2 × f32 | `above_ground` | Height-above-ground range (§7.5) | (−0.0002, 0.0002) mostly |
| 98 | 8 | 2 × f32 | `above_water` | Height-above-water range | (−0.0002, 0.0002) mostly |
| 106 | 8 | 2 × f32 | `below_water` | Depth-below-water range | (−0.0002, 0.0002) mostly |
| 114 | 8 | 2 × f32 | `water_depth` | Water-depth range (water surface minus ground) | (−0.0002, 0.0002) in 53,350; also (0, 50), (0.2, 0.7) … |
| 122 | 4 | f32 | `below_ground_min` | Minimum of the depth-below-ground range (the maximum is the node's last field) | always −0.0002 |
| 126 | 8 | 2 × f32 | `z_range` | Absolute height range (min, max) | always (−FLT_MAX, +FLT_MAX) |
| 134 | 4 | f32 | `spacing` | Spacing radius (§7.4 steps 5 and 14), units | 0.1 (25,651), 0.05, 1.0, 3.0, 0 … |
| 138 | 4 | f32 | `clearance` | Radius of the terrain clearance test (§7.4 step 13); 0 disables the test | 0 in 49,405; 0.5 in 15,798 |
| 142 | 1 | bool8 | `clearance_scaled` | Multiply `clearance` by max(scale x, scale y) | always 1 |
| 143 | 4 | f32 | `clear_max_cos` | Upper bound on the normal *z* of every terrain triangle in the clearance area | 1.0 (66,106), cosines of small angles |
| 147 | 4 | f32 | `clear_min_cos` | Lower bound on those normals' *z* | ≈ 0 (49,750), 0.766 (14,779) |
| 151 | 4 | f32 | `clear_min_dot` | Smallest allowed dot product between any two of those normals; −0.9998 or less disables | −1.0 (49,405), 0.9848 (16,405) |
| 155 | 4 | f32 | `clear_aspect_tol` | Aspect tolerance in degrees; the aspect test runs only when below 180 | always 360.0 |
| 159 | 4 | u32 | `max_objects` | Stop processing this node's children once it has created this many objects (§7.4 step 15); 0 = no limit | always 0 |
| 163 | 4 | f32 | `unknown_163` | Not read by the placement code (BIN); the client's default is 1.0 | always 1.0 |
| 167 | 1 | bool8 | `disp_scaled` | Multiply the displacement (and z offset) by the parent scale | always 1 |
| 168 | 1 | bool8 | `inherit_scale` | Start from the parent scale; 0 = start from 1 | 1 except 42 |
| 169 | 1 | bool8 | `disp_relative` | Rotate the displacement by the parent orientation | always 1 |
| 170 | 1 | bool8 | `orient_relative` | Compose the node's rotation with the parent orientation; 0 = absolute | always 1 |
| 171 | 1 | bool8 | `spacing_scaled` | Multiply `spacing` by the largest scale component | always 1 |
| 172 | 1 | bool8 | `no_checks` | Skip this node's terrain-type, height-range and spacing checks | 1 in 1,624 |
| 173 | 1 | bool8 | `spacing_3d` | Spacing radius uses max of all three scale components and tests in 3D | always 0 |
| 174 | 1 | bool8 | `unknown_174` | Not read by the placement code (BIN) | always 0 |
| 175 | 1 | bool8 | `no_spacing` | Use spacing radius 0 and skip the test against the footprints of placed nodes (also for descendants) | 1 in 1,944 |
| 176 | 1 | u8 | `height_mode` | 0 = snap to the terrain plane; 1 = snap to the water surface (reject where there is none); 9 = no snap; 2–8 = base height 0; above 9 = no snap (BIN only for 2–9) | 0 (64,789), 1 (1,402) |
| 177 | 1 | u8 | `scale_mode` | 4 = uniform random scale (§7.4 step 3); 0–3, 5–8 other modes (BIN only) | always 4 |
| 178 | 1 | u8 | `above_ground_mode` | Range mode for `above_ground` (§7.5) | 1 (66,083), 0, 2 |
| 179 | 1 | u8 | `above_water_mode` | Range mode for `above_water` | 1 (65,582), 0, 2 |
| 180 | 1 | u8 | `below_water_mode` | Range mode for `below_water` | 1 (35,963), 0 (29,408), 2 |
| 181 | 1 | u8 | `water_depth_mode` | Range mode for `water_depth` | 1 (29,358), 0 (23,992), 2 (12,841) |
| 182 | 1 | u8 | `below_ground_mode` | Range mode for the depth-below-ground range | 2 (62,115), 1 (4,076) |
| 183 | 4 | u32 | `child_count` | | 1 in 62,669; up to dozens |
| 187 | … | child × `child_count` | children | §5.2 | |
| end | 4 | f32 | `below_ground_max` | Maximum of the depth-below-ground range | 0.0002 in 65,356 |

Where a meaning is marked BIN only, no record exercises it; implementers may reject the
unobserved enum values as unsupported.

<a id="s5-4"></a>
### 5.4 Observations

- 2,472 of the 3,402 scene records are named by scene-choice tables; the rest are reached
  only through scene references. Every referenced scene exists (0 dangling); the reference
  graph has no cycles and is at most 5 scenes deep; nodes nest at most 2 deep.
- Every **frill layer** (root node) has frequency 1.0 and, in 2,320 of 2,334, zero
  displacement. Its children, where it has any, are **only scene references** (8,031 in
  2,154 layers); 180 of the 2,334 frill layers have no children at all and produce nothing.
  The objects of a frill layer are therefore the group objects of the referenced scenes.
- Three scenes of 22 bytes contain one empty group and nothing else: 0x02000000 and
  0x020000CA, the "no scenery" scenes named by scene keys 1 and 2 of every 0x11 table, and
  0x020000CB, which no 0x11 table names and which is reached only through 6 scene
  references in other scenes.
- Objects: 4,261 distinct 0x47 DIDs. 2,981 are reachable through the groups of the 2,472
  scenes named by 0x11 tables and the scenes those groups reference; 1,515 through frill
  layers, counting the frill layer of every scene that the frill pass can instantiate
  (§8.2: those named by 0x11 tables and those reached from them through scene references);
  235 through both. Counting only the frill layers of the 2,472 named scenes gives 1,512
  (`collision.md` §9.4 used this narrower count before its 2026-10-05 revision). A further
  452 objects, reached through groups only, can also become frill records, because the frill
  pass meets them after a frill layer (§8.1); 1,967 objects in all.

---

<a id="s6"></a>
## 6. Hash functions and seeds (BIN)

All arithmetic is on unsigned 32-bit integers, wrapping modulo 2³². "*v* ≫ 16" is a
logical right shift. A hash value *v* is turned into a fraction **u = (v ≫ 16) / 65,536**,
0 ≤ u < 1.

> **Constants:** the multipliers and offsets below are constants of the client's placement
> hash, taken from the binary (BIN). They are numbers that define how the client computes
> from the user's data, not keys or tables, and [decision
> 0012](../decisions/0012-numeric-constants.md) allows specs to state them and Tessera's
> code to contain them, citing this section. The same holds for the thresholds of §7 and §8
> (0.0002 tolerances, the band rule of §7.4 step 8, 0.3 sectors, 1.5 s).

<a id="s6-1"></a>
### 6.1 Vertex scene pick

For landblock (X, Y) and coarse vertex (i, j): gx = 16·X + i, gy = 16·Y + j (global vertex
coordinates in 10-unit steps).

v = ((gx² · gy · 0x019A266F − 0x4A6506DB) · gy − gx · 0x6B3C5F11 − 0x39B625FB) mod 2³²,
and u = (v ≫ 16) / 65,536.

<a id="s6-2"></a>
### 6.2 Node random function R(A, B, s)

R(A, B, s) = u of the value
v = ((0x62433D7D − 0x2BD38901 · s · A² · B²) · s − 0x6B3C5F11 · A − 0x4A6506DB · B) mod 2³².

A and B are fixed for one vertex (§6.3); *s* is the node seed C plus a salt per use:

| Use | Salt added to C |
|---|---|
| Frequency test | 0x01191E67 |
| Displacement x, and the height offset of §7.4 step 11 | 0x28D0642D |
| Displacement y | 0x062ADE4F |
| Displacement z (only when `height_mode` = 9) | 0x0071D36D |
| Heading | 0x03D60BCB |
| Second angle | 0x02986B4B |
| Third angle | 0x0090B46B |
| Uniform scale (`scale_mode` 4) | 0x00515E71 |

> **OPEN (not needed by this install):** the salts of the per-axis scale modes 0–3, 5, 6
> (0x00515E71, 0x0DE876C7 and 0x17E0784F appear, with sign variants of the A and B terms) are
> not specified; no node uses them.

<a id="s6-3"></a>
### 6.3 Seeds

- **A** (per landblock) = X + Y + R + 5, where R is the region number (1–5, 14) and X, Y
  the landblock coordinates. (The client adds the 16-bit cell field of the landblock's
  identifier, which is 0 for outdoor landblocks. It uses A = 1 instead when a list held by
  its landblock-info object is non-empty; that list is not filled by any part of the
  landblock-info record as decoded in `landblock.md` §4, and what fills it was not found.
  Tessera Decision: use X + Y + R + 5.)

  > **OPEN:** what fills the landblock-info list whose being non-empty switches seed A to 1
  > (§15 item 7). Until it is found, A = X + Y + R + 5 for every outdoor landblock.

- **B** (per vertex) = 17·i + j + 0x003556F7.
- **C** (per node): when a scene is instantiated with an incoming seed *c*, its scene seed
  is S = *c* · 0x000923B1. The k-th child of its group (k = 0, 1, …) is processed with
  C = S + k · 0x2D41BF19. Inside a placement node with seed C, its k-th child is processed
  with C + k · 0x2D41BF19 (no multiplication), whether that child is a nested node, a scene
  reference (which then multiplies, as above) or an object. The frill layer of a scene with
  scene seed S is processed with C = S + 0x0380430A. The top-level incoming seed for a
  vertex is *c* = 0x000923B1.

---

<a id="s7"></a>
## 7. Generating the scenery of a landblock (BIN)

<a id="s7-1"></a>
### 7.1 When

The client generates a landblock's scenery once, when it builds the landblock's static
entities (BIN). Before generation it has:

1. built the landblock's **terrain collision mesh** (`collision.md` §6.1; 2,048 triangles
   numbered as in `collision.md` §6.2). Generation reads terrain planes from this mesh
   (§7.4 steps 8 and 13), not from a separate height query;
2. built the landblock's **static list** from the landblock-info record (§7.4 step 14a).

It then runs the vertex pass (§7.2–§7.3) and the encounter pass (§7.7). Generation reads
nothing else from the world: no collision shapes of objects, cells or other scenery. The
generated objects become entities afterwards (§7.6). Generation is deterministic: the
same data gives the same objects every time.

<a id="s7-2"></a>
### 7.2 Choosing the scene of each vertex

For every coarse vertex (i, j), i = 0 … 16 outer, j = 0 … 16 inner:

1. Read the word (§3). Resolve terrain entry and scene key (§4.2). No key → no scene.
2. In the scene-choice table find the entry with that key. Missing → no scene.
3. Compute u (§6.1) and the threshold T = u × (total weight of the entry).
4. Walk the choices in stored order, adding each weight to a running sum; pick the first
   choice whose running sum is **strictly greater** than T. If none of the first *n* − 1
   qualifies, pick the last. (A single choice is always picked.)

<a id="s7-3"></a>
### 7.3 Instantiating the vertex scene

For every vertex in the same order (outer i, inner j), with a chosen scene:

1. Base frame: position (i · (10 − 0.0002), j · (10 − 0.0002), h) where h is the height
   sample (2i, 2j) converted as stored × 2,000 / 65,535 (the client's rule,
   `landblock.md` §3.4 and `terrain-texturing.md` §13); orientation identity; scale
   (1, 1, 1).
   (The 10 is the region's scenery vertex spacing; every region uses 10 in this build. The
   0.0002 keeps vertex 16 inside the block.)
2. Seeds A, B and incoming *c* = 0x000923B1 (§6.3).
3. Instantiate the scene (below). The order of vertices and children matters, because
   objects placed earlier take part in the spacing test of later ones.

**Instantiating a scene** with frame F, scale σ, seeds A, B and incoming *c*:

1. S = *c* · 0x000923B1.
2. Take the first group with id 1. For each child k in order, with C = S + k · 0x2D41BF19:
   - placement node → process it (§7.4) with parent frame F and parent scale σ;
   - scene reference → instantiate that scene with frame F, scale σ and incoming *c* = C;
   - object → create it at F with scale σ (§7.6).
3. The frill layer is ignored here; only the frill engine evaluates it (§8).

<a id="s7-4"></a>
### 7.4 Processing a placement node

Inputs: node N, parent frame P (position, orientation), parent scale σ, seeds A, B, C, and
the inherited switches `no_spacing*` and `no_checks*` (both initially false). Every step
below is the original client's rule (BIN) unless it says Tessera Decision.

1. **Frequency.** If R(A, B, C + 0x01191E67) is **not less than** `frequency`, skip the node
   (nothing is placed, not an error).
2. **Switches.** `no_spacing*` ← `no_spacing*` or N.`no_spacing`, and `no_checks*` ←
   `no_checks*` or N.`no_checks`; both are handed to all descendants. The checks below
   consult only the node's **own** `no_checks` and the combined `no_spacing*`: the client
   passes `no_checks*` down the tree but never reads it.

   > **OPEN (raised by the `world` implementation, 2026-10-06):** whether "all descendants"
   > includes the nodes of scenes reached through a scene reference (a child of a node, or
   > the frill layer's references), that is, whether instantiating a scene (§7.3) carries
   > the incoming `no_spacing*` into its group's nodes or starts it again at false. How
   > many observed nodes set `no_spacing` and have a scene reference below them was not
   > counted. **Tessera Decision** until traced: the switches pass through scene
   > references unchanged (`tessera_world::scenery`).
3. **Scale.** Start from σ if `inherit_scale`, else (1, 1, 1). If `scale_min` and
   `scale_max` differ by more than 0.0002 on some axis, draw t = R(A, B, C + 0x00515E71)
   and multiply each axis *a* by `scale_min`[a] + t · (`scale_max`[a] − `scale_min`[a])
   (one t for all three axes); otherwise multiply by `scale_min`. Call the result σ′.
4. **Position.** For x and y draw r = R with the salts of §6.2 and take
   d = `disp_min` + r · (`disp_max` − `disp_min`), multiplied per axis by σ if
   `disp_scaled`. (dz is computed only for `height_mode` 9; otherwise 0.) If
   `disp_relative`, rotate (dx, dy, dz) by P's orientation. Candidate position
   p = P.position + d.
5. **Spacing radius.** ρ = `spacing` × (largest of σ′x, σ′y, or of all three if
   `spacing_3d`) when `spacing_scaled`, else `spacing`. If `no_spacing*`, ρ = 0. The
   node's own footprint radius ρ_N (step 15) is the same value **without** the
   `no_spacing*` zeroing.
6. **Inside the block.** Reject unless ρ ≤ p.x < 160 − ρ and ρ ≤ p.y < 160 − ρ
   (block-local units).
7. **Terrain type.** Unless N.`no_checks`: convert p and ρ to quad units (q = coordinate ×
   32 / 160, radius ρq = ρ × 32 / 160). Visit every surface quad (qx, qy) with qx from
   q.x − ρq − 1 to q.x + ρq + 1 and qy likewise (each bound truncated toward zero, then
   clamped to 0 … 31). With dx = |qx + 0.5 − q.x| and dy = |qy + 0.5 − q.y|, a quad counts
   when dx < 0.5, or dy < 0.5, or (dx − 0.5)² + (dy − 0.5)² ≤ ρq². For each counted quad
   read the quad-grid material at index **32 · qy + qx** (`landblock.md` §3.8.4: Y-major)
   and look it up **by key** in the terrain-type table (§4.4); if its `procedural` field is
   0, reject. The test passes at once when the block has no quad grid, or when the visited
   range lies wholly outside 0 … 32 (q.x − ρq − 1 > 32, q.y − ρq − 1 > 32,
   q.x + ρq + 1 < 0 or q.y + ρq + 1 < 0). A quad value that is not a key of the table is
   ignored.
8. **Terrain plane.** The plane is that of one triangle of the landblock's terrain
   collision mesh (`collision.md` §6.1; key = 2 · (32 · j + i) + side, numbering of
   `collision.md` §6.2). With i = ⌊p.x / 5⌋ and j = ⌊p.y / 5⌋ (32 becomes 31; an i or j
   outside 0 … 32 leaves the plane at n = (0, 0, 0), d = 0, which step 6 rules out),
   fx = p.x − 5i and fy = p.y − 5j (units, 0 ≤ fx, fy < 5):
   - bit 7 of the vertex-flag byte of sample (i, j) set: side = 1 when **fx > fy**
     (strictly), else 0. A point on the diagonal (fx = fy) therefore gets side 0, the
     north-west triangle (key *base*, `collision.md` §6.2);
   - bit 7 clear: side = 1 when **fy > 4 − fx** (strictly), else 0.

   Both comparisons are the original client's (BIN (i)), and so is the use of the result:
   the plane of the chosen triangle gives the height of step 11 and the normal of steps 9–10.
   The second rule is not the geometric split of the quad (that is fx + fy > 5). For points
   with 4 < fx + fy ≤ 5 in a quad with bit 7 clear, the client takes the plane of the
   north-east triangle although the point lies in the south-west one. On the diagonal of a
   quad with bit 7 set both triangles contain the point and give the same height (up to
   rounding), but their normals differ, so the slope test of step 9 and the tilt of
   step 10 depend on the tie rule.

   The client's general terrain height query and terrain plane query at a point (used
   outside scenery placement) choose the triangle by these same two rules (BIN (i)).
   `landblock.md` §3.11 describes the geometric triangulation instead: it takes the
   south-east triangle when fx ≥ fy (bit 7 set) and the north-east one only when
   fx + fy > 5 (bit 7 clear). That section is therefore Tessera's height rule, not the
   client's point query; the two agree everywhere except on the diagonals of bit-7-set quads
   (same height, other normal) and in the band 4 < fx + fy ≤ 5 of bit-7-clear quads (other
   height and normal). **Tessera Decision:** scenery placement uses the client's rules of
   this step, so that generated objects match the original; other height queries
   (walking, camera) use `landblock.md` §3.11 and `collision.md`. The plane passes through the triangle's
   three corners (heights stored × 2,000 / 65,535); n is its unit normal, n.z > 0, and d
   its offset (n · v + d = 0 on the plane). When the landblock has no terrain mesh the
   plane is n = (0, 0, 0), d = 0.
9. **Slope.** Reject unless `slope_min_cos` ≤ n.z ≤ `slope_max_cos`.
10. **Orientation.** Draw three angles in degrees: h = `heading`.min +
    R(A, B, C + 0x03D60BCB) · (`heading`.max − `heading`.min), a2 from `angle2` with salt
    0x02986B4B and a3 from `angle3` with salt 0x0090B46B (both 0 in this install). Write
    Rx(θ), Ry(θ), Rz(θ) for right-handed rotations by θ about the +X, +Y, +Z axes in the
    quaternion convention of `coordinates.md` §11.1–§11.2 ((w, x, y, z), active, Hamilton
    product). The node's local rotation is **Q_local = Rz(−h) · Ry(a3) · Rx(a2)**; a2 and
    a3 are applied only when non-zero. This is the client's own yaw-bank-pitch composition
    (`coordinates.md` §11.3) with yaw −h, bank a3 and pitch a2. In words: the heading turns
    the object **clockwise seen from above** by h degrees, so model +Y (forward) points to
    compass bearing h (`coordinates.md` §11.4; east at 90°); a2 then tilts about model X and
    a3 about model Y, applied before the heading. The final orientation Q:
    - if `face_direction` (never set here): h ← h + 180° + atan2(−n.x, n.y) in degrees (the
      atan2 term is 0 when √(n.x² + n.y²) ≤ 10⁻⁶) and a2, a3 are not applied; if
      `align_to_slope` is also set, Q = Q_align(n) · Rz(−h) as in the next rule, otherwise
      Q = Rz(−h) (absolute, whatever `orient_relative` says);
    - else if `align_to_slope`: **Q = Q_align(n) · Rz(−h)**, with a2 and a3 not applied and
      P's orientation **not** composed (the alignment replaces the parent orientation).
      Q_align(n) is the rotation whose columns, the images of model X, Y and Z, are
      X′ = (L, −n.x·n.y / L, −n.x·n.z / L), Y′ = (0, n.z / L, −n.y / L) and Z′ = n, with
      L = √(n.y² + n.z²): model Z goes to the normal, and model X to world +X projected
      onto the slope. Special cases: when |n.x| < 0.0002 and |n.y| < 0.0002, Q_align is
      the identity; when |n.z| ≤ 0.0002 (vertical plane), X′ = (−n.y, n.x, 0) / L′,
      Y′ = (0, 0, 1) and Z′ = (n.x, n.y, 0) / L′ with L′ = √(n.x² + n.y²);
    - else if `orient_relative`: Q = P.orientation · Q_local;
    - else Q = Q_local.

    The position is not changed by this step.
11. **Height.** Depending on `height_mode`:
    - 0 → p.z = the step-8 plane's height at (p.x, p.y), −(n.x·p.x + n.y·p.y + d) / n.z,
      when |n.z| > 0.0002 (else p.z is kept);
    - 1 → if the landblock has a water surface (§11), p.z = its plane's height at
      (p.x, p.y) in the same way; otherwise reject;
    - 2 – 8 → p.z = 0;
    - 9 → nothing at all in this step (the z displacement of step 4 is used instead);
    - any other value → p.z is kept.

    Except for mode 9, then add the height offset `disp_min`.z + r · (`disp_max`.z −
    `disp_min`.z), r = R(A, B, C + 0x28D0642D), multiplied by the **parent** scale σ.z if
    `disp_scaled` (this is how objects are sunk into the ground; the range is (0, 0) for
    most nodes and as deep as (−5, −5) for some). The scale modes 5 – 9 (unused here)
    would now adjust σ′ from the water depth or the slope.
12. **Height ranges** (§7.5). Unless N.`no_checks`.
13. **Terrain clearance.** Only when `clearance` > 0. r = `clearance` × max(σ′x, σ′y)
    when `clearance_scaled`, else `clearance`. Collect the terrain triangles of the
    landblock near p with the radius query of `collision.md` §6.6 (horizontal circle of
    radius r; terrain triangles only, never objects, cells or other scenery). Reject the
    candidate when any of these fails:
    a. every collected triangle has `clear_min_cos` ≤ n.z ≤ `clear_max_cos`;
    b. when `clear_aspect_tol` < 180: every collected triangle that is not level
       (|n.x| > 0.0002 or |n.y| > 0.0002) has an aspect within `clear_aspect_tol` of the
       node's compass heading (that of Q by `coordinates.md` §11.3–§11.4, h for an
       untilted Q; the difference must be strictly less than the tolerance), where the
       aspect is
       (450° − atan2(n.y, n.x) in degrees) mod 360 (the compass bearing of the normal's
       horizontal part) and the difference is wrapped to 0 … 180°; never exercised here
       (the tolerance is 360 in every node);
    c. when `clear_min_dot` > −0.9998: every pair of collected triangles has
       n₁ · n₂ ≥ `clear_min_dot`.

    In this install every node with `clearance` 0 carries (`clear_max_cos`,
    `clear_min_cos`, `clear_min_dot`, `clear_aspect_tol`) = (1, ≈ 0, −1, 360) (SC10), and
    most nodes with a clearance carry (1, 0.766, 0.9848, 360): the ground within the radius
    must be no steeper than 40° and no two triangles may differ by more than 10°.
14. **Spacing.** Unless N.`no_checks`, reject when the candidate overlaps any of:

    a. **The static list:** the static entities of the landblock-info record
       (`landblock.md` §4.5, `entity-descriptor.md` §4.1) of type Weenie (6), Physics (7)
       or PatrolPoint (23), in record order, that the client keeps after its static-entity
       filter (see below). Each carries the volumes of its `Entity_Clearance_BoxList` and
       `Entity_Clearance_CylinderList` properties (property ids 0x207 and 0x4CF; the SED's
       own value, else its object's). The test maps p into the entity's frame (subtract the
       SED position, rotate by the inverse SED rotation; **no** scale) and checks each
       volume:
       - box (min, max): skipped when all three extents max − min are at most 0.0002;
         otherwise the squared distance from the mapped point to the box (per axis 0 inside
         the range, else the distance to the nearer face; z only when `spacing_3d`) is
         compared: overlap when it is at most ρ²;
       - cylinder (built from `Entity_Clearance_LowPoint`, `Entity_Clearance_Radius` and
         `Entity_Clearance_Height`): skipped when its radius is at most 0; otherwise,
         unless `spacing_3d`, overlap when (x − cx)² + (y − cy)² ≤ (ρ + radius)², with
         (cx, cy) the x and y of the low point; with `spacing_3d` a sphere-against-cylinder
         test whose exact form was not traced (unused here).

       > **OPEN:** the sphere-against-cylinder form of the cylinder test when `spacing_3d`
       > is set (§15 item 5). No node sets `spacing_3d`; readers may treat it as
       > unsupported.

       The client has a second list tested the same way, meant for other objects already
       registered in the landblock; in this build nothing ever fills the registry that
       feeds it, so the list is always empty (BIN).

       The static-entity filter: the client drops entities that fail its validity check
       and, in one of its run modes, entities that are the target of a Generator link
       (`landblock.md` §4.6, type 8) and one object named by a DID map. Tessera Decision:
       keep every entity of the three types; Generator targets are spawn points and rarely
       carry clearance volumes.

       > **OPEN:** which run mode of the static-entity filter an offline client is in, and so
       > whether Generator targets are dropped from the static list (§15 item 6).

    b. **Footprints of placed nodes**, unless `no_spacing*`: every *closed* footprint
       marker in the landblock's generated list (step 15; markers of the current node and
       its ancestors are still open and are skipped). Overlap when
       (ρ + ρ_m)² ≥ |p − m|², with m the marker position and ρ_m its radius, measured
       horizontally unless both the candidate's and the marker's `spacing_3d` are set.
       Markers with ρ_m ≤ 0 never overlap. Generated **objects** carry no volume and never
       cause a rejection.

       > **OPEN (M4.5 exit verification, clean room, 2026-10-06):** the expected generated
       > lists of `m4-5-visual.md` §4.2 (rows S1–S11) and walk GN1 contradict this rule as
       > `tessera_world::scenery` implements it. With step 14b as written, 6 of the 22
       > listed objects are rejected by a closed footprint marker (S1 both, S5, S6, S8, S10)
       > and every row's landblock total is lower than the row's (for example S2 315 against
       > 344, S10 80 against 102). With a probe that ignores every marker created by a node
       > processed directly in a vertex scene (scene depth 1), all 22 objects match to the
       > listed tolerances and the totals of S1–S8, S10 and S11 match exactly (36, 344, 173,
       > 364, 252, 176, 471, 361, 102, 201); S9 (nested scene references) gives 249–272
       > against 362 under every variant tried (also: no footprint test at all 1,532; markers
       > closed at the end of the vertex or of the scene instance: S1, S5, S6, S8 still
       > fail). In S10 all 22 rejections are between sibling nodes of one vertex scene
       > (for example 0x47007AA2's node at 1.961 m from a sibling's marker with ρ + ρ_m =
       > 2.098). Which markers the client really tests (and whether the checklist's
       > generator matches this section) needs a BIN re-check; the code keeps the rule as
       > written until then.
15. **Children.** The node is placed at frame (p, Q) with scale σ′. Unless the node is
    processed while the frill pass's "under a frill layer" state is set (§8.2) or by the
    encounter pass (§7.7), the client first appends
    the node's **footprint marker** to the landblock's generated list: position p,
    radius ρ_N (step 5), the node's `spacing_3d`, state *open*. It then processes the
    children in order with C_k = C + k · 0x2D41BF19 (§6.3): nested node → §7.4 with this
    frame as parent and the switches of step 2; scene reference → instantiate the scene
    (§7.3) with this frame, scale σ′ and incoming *c* = C_k; object → create it (§7.6) with
    this frame and σ′. If `max_objects` > 0, the remaining children are skipped once the
    number of objects created directly by the node, or directly in the group of a scene it
    references, reaches `max_objects` (unused here). After the last child the marker
    becomes *closed*.

A rejected node places nothing and its children are not visited; the remaining siblings
continue. Instantiating a scene (§7.3) also appends a marker, with radius 0; it never
overlaps and an implementation may omit it.

<a id="s7-5"></a>
### 7.5 Height-range checks

Let z = p.z, g = the height of the step-8 terrain plane at (p.x, p.y) (z itself when
|n.z| ≤ 0.0002), and, if the landblock has a water surface (§11), w = the height of its
plane there (z itself when the plane's |n.z| ≤ 0.0002). A range check with mode M and range
(a, b) on a value q passes when: M = 0 → never; M = 1 → q ≥ a; M = 2 → a ≤ q ≤ b; any
other value → always.

1. Reject unless `z_range`.min ≤ z ≤ `z_range`.max.
2. If there is a water surface and w − g > 0.0002 (water over ground):
   - check `water_depth_mode` / `water_depth` on q = w − g (mode 2 also rejects q < a);
   - then if z − w ≥ −0.0002 check `above_water_mode` / `above_water` on q = z − w, else
     check `below_water_mode` / `below_water` on q = w − z.
3. Otherwise: if z − g ≥ −0.0002 check `above_ground_mode` / `above_ground` on q = z − g,
   else check `below_ground_mode` / (`below_ground_min`, `below_ground_max`) on q = g − z.

With the observed values, most nodes allow any height above ground and at most 0.0002
below it (`below_ground_mode` 2); 4,076 nodes allow any depth below ground; 23,992 nodes
(`water_depth_mode` 0) refuse water-covered ground.

<a id="s7-6"></a>
### 7.6 Creating an object

An object child names a 0x47 entity descriptor. The client appends a record to the
landblock's generated list (BIN): the object DID, the frame (position and orientation Q of
the node, or the incoming frame for an object directly in a scene group), the scale (the
node's σ′, or the incoming σ; uniform in this install) and the object's
`Entity_CreationType`, looked up along the object's 0x47 chain (§9.2 rule 2; 0 when no
level has it, which no scene object in this install lacks). Object records carry no
clearance volume and do not take part in the
spacing test (§7.4 step 14). Objects of the encounter pass go to a separate list.

After generation the client turns every object record into a static-entity creation
request of the same form as those for landblock-info statics (request kind 4 instead of 2),
with an instance ID synthesized from the landblock and the record's position in the list;
rendering, collision (`collision.md` §9.4) and LOD then follow the object's properties,
resolved along its 0x47 chain (§9, §9.2, `setup.md`). Footprint markers (§7.4 step 15) create nothing.

<a id="s7-7"></a>
### 7.7 Encounter pass (out of scope)

A second pass visits the 16 × 16 cell centres ((2i + 1) · (5 − 0.0002),
(2j + 1) · (5 − 0.0002)), reads bits 15–19 of the coarse word and a per-region encounter
table, and places scenes with seeds A = 5, B = 0x003556F7 + 17·i + j, C = 0x2D41BF19 into a
separate list (no footprint markers). It runs only when the landblock data lists encounter
entries; bits 15–19 are 0 everywhere in this install, so a viewer can omit it.

---

<a id="s8"></a>
## 8. Frills (grass and ground cover)

<a id="s8-1"></a>
### 8.1 What a frill is (BIN; structure Confirmed)

A **frill record** (object DID, landblock, frame, scale) is what the frill pass of §8.2
produces for an object instead of an entity. Frill records are not entities: they have no
collision, no selection and no LOD class; the frill engine draws them in bulk (§8.3, §8.5).
The frill pass makes a frill record of **every** object it meets while its per-vertex state
*under a frill layer* is set (§8.2), with the frame and scale computed by the full §7.4
algorithm. The frill layer (§5.1) is a placement node whose children are scene references.
Objects reach the frill pass with the state set in two ways (census over the 2,472 scenes
named by 0x11 tables and everything they reference, before frequency and other tests; OBS,
2026-10-06):

| Source | How the frill pass meets the object | Distinct objects |
|---|---|---|
| (a) **Frill-layer objects** | While a frill layer is being processed: the group objects of the scenes that a frill layer references, at any depth (§5.4) | 1,515 |
| (b) **Late group objects** | While no frill layer is being processed, but after one has finished in the same vertex: group objects that come, in visit order, after a scene whose frill layer has been processed (§8.2 item 4) | 474, in 84 of the 2,472 named scenes; 22 of them are also in (a) |
| All | | 1,967 (1,515 + 452) |

The 452 objects of (b) that are not in (a) are ordinary scenery (census in §8.3): the static
pass of §7 makes them entities as well. The client treats both sources alike (§8.5);
Tessera does not draw the records of source (b) (§8.6).

<a id="s8-2"></a>
### 8.2 Generation per frill sector

- A **frill sector** is one 10 × 10-unit coarse cell; a landblock has 16 × 16 sectors.
  Sector (sx, sy) of a block is generated from coarse vertex (sx, sy); sectors in the last
  column also generate vertex (16, sy), the last row vertex (sx, 16), and the corner sector
  vertex (16, 16).
- For a vertex, the client chooses the scene exactly as in §7.2, builds the base frame of
  §7.3 and instantiates the scene in the **frill pass** (BIN (j)). The pass keeps one
  per-vertex state, *under a frill layer*, which is clear when the vertex starts:
  1. Instantiating a scene in the frill pass walks its group as in §7.3–§7.4 (nodes are
     tested and placed, scene references are instantiated in the frill pass, recursively);
     an object met while the state is clear is skipped, one met while it is set becomes a
     frill record.
  2. After the group, if the scene has a frill layer, the state is **set** and the frill
     layer is processed as a placement node (§7.4) with C = S + 0x0380430A (§6.3); every
     object reached under it, through its scene references, becomes a frill record instead
     of an entity.
  3. This holds for **every** scene instantiated in the frill pass, not only the vertex's
     own: a scene reached through a scene reference (in a group, in a placement node or in a
     frill layer) has its frill layer processed after its group too. Of the 2,334 scenes with
     a frill layer, 41 are not named by any 0x11 table; 40 of them are referenced only from
     placement nodes of groups (479 references) and one only from a frill layer (OBS).
  4. The state is never cleared before the vertex ends. Objects met later in the same
     vertex, also in a group after a referenced scene's frill layer, become frill records
     (BIN). In 84 of the 2,472 scenes named by 0x11 tables, the visit order puts group
     objects (474 distinct DIDs) after such a frill layer (OBS, before frequency and other
     tests), so this matters in the data; such objects are also entities of the static
     pass, at the same frame when the same nodes are placed. These are source (b) of §8.1.
     The client draws their frill records like any other (§8.5); Tessera discards them
     (§8.6).
  5. Footprint markers (§7.4 step 15): nodes processed while the state is clear append
     markers, nodes processed while it is set do not. The frill pass runs on the same
     per-landblock generator, and appends to the same generated list, as the static pass
     of §7.1–§7.6 (BIN).
- A frill record is kept only if its position lies in the same landblock as the vertex, and
  then only if a uniform random number in [0, 1) drawn from a **global, unseeded** generator
  is at most the frill **density** option (§8.4). With the default density 1.0 every frill
  is kept; lower densities thin frills randomly, differently each session.
- Sectors are processed in a spiral around the centre sector, which lies between the camera
  and the player by the player-position-blend option (Probable).

> **OPEN (raised by the `world` implementation, 2026-10-06):** the shape of the set of
> sectors the frill engine keeps for a radius of *n* sectors (square, circle, or the
> spiral's own extent) and when a sector leaves it. §8.3 gives the radius only.
> **Tessera Decision** until traced: every sector whose centre is within
> *n* + `FrillEngine.ExtraPreCacheRadiusLevels` sectors (centre to centre) of the centre sector
> is generated, nearest first ring by ring; a sector is dropped once it is more than one
> further sector away (`tessera_world::frills::FrillRing`). Drawing still stops at the
> far distance D of §8.3 rule 2.

> **OPEN:** whether the static pass's closed footprint markers are still in the generated
> list when the frill pass runs, and whether the frill pass's own markers are removed after
> a vertex or a sector. The list is emptied when the landblock's generator is rebuilt (BIN);
> no emptying between the static pass and a frill pass was found. If the static markers
> remain, every group node that the static pass placed, with a spacing radius above 0 and
> neither `no_checks` nor `no_spacing*`, overlaps its own static-pass marker (same position)
> and is rejected in the frill pass, so only the
> frill layers of the vertex scene and of scenes referenced directly in its group (not
> inside placement nodes) produce frills. Most frill layers skip the marker test themselves
> (`no_spacing` or `no_checks` set in 2,224 of the 2,334 layers).
> **Tessera Decision** until this is traced: the frill pass of a vertex tests spacing only
> against the markers appended during the frill pass of that same vertex, discards them when
> the vertex ends, and applies items 1–5 above as stated.

<a id="s8-3"></a>
### 8.3 Drawing (BIN; shader formulas OPEN)

- **Mesh census** of the objects that can become frill records (OBS, 2026-10-06, all 1,967
  of §8.1). "Drawn" means the geometry the frill engine uses: the first draw entry of
  detail level 0 of the first part (§8.5 step 3).

  | | (a) Frill-layer objects | (b) Late group objects not in (a) |
  |---|---|---|
  | Objects | 1,515 | 452 |
  | Parts per setup | 1 in all | 1 in 440; 8 in 6; 9 in 6 (in these 12, the first three parts are real meshes and the rest one-triangle placeholders, `setup.md` §8.5) |
  | Only part is a one-triangle placeholder | 0 | 8 |
  | First part has more than one detail level | 24 | 348 |
  | Level 0 of the first part has more than one draw entry | 1 | 140 |
  | Triangles drawn: median / 95th percentile / maximum | 36 / 280 / 2,052 | 1,511 / 9,896 / 11,266 |
  | Vertices drawn: median / maximum | 48 / 2,134 | 1,291 / 9,654 |
  | Objects whose level-0 geometry over all parts exceeds what is drawn | 1 | 152 |
  | Triangles over all parts and all detail levels: median / maximum | 36 / 4,270 | 3,329 / 29,462 |

  The frill-layer objects are small card or clump meshes, suited to instancing; the late
  group objects have the size of ordinary scenery (§9).
- Per-object properties of the late group objects (OBS, 452): `Entity_CreationType` on all
  (`SharedStaticImmobile` 310, `SharedClientOnlyImmobile` 107, `UniqueStaticImmobile` 33,
  `UniqueClientOnlyImmobile` 2); `Render_LODClass` 441; appearance properties
  (`Appearance_*`) 222; `Render_Frill_AnimationIntensity` 18 (0.02 or 0.15);
  `Render_Frill_IsOpaque` 1; `Render_Frill_TerrainColorIntensity` none, so rule 7 gives
  them the default intensity 1.0. None of the 452 has a parent 0x47 record, so these
  counts are the same over the own sets and resolved along the chain (§9.2).
- Per-object properties of the 1,515 frill-layer objects (OBS). Each count is given twice:
  first over each record's **own** set, then **resolved** along the record's 0x47 chain
  (§9.2; 82 of the 1,515 have a parent):

  | Property | Type | Own set | Resolved | Values |
  |---|---|---|---|---|
  | `Render_Frill_AnimationIntensity` | f32 | 929 | 974 | 0.15, 0.2, 0.1, 0.0 …; above 0.0002 (animated) in 836 own, 881 resolved |
  | `Render_Frill_TerrainColorIntensity` | f32 | 48 | 48 | 0.25 or 0 |
  | `Render_Frill_IsOpaque` | bool | 5 | 5 | true |
  | `Render_Frill_AllowDistanceReduction` | bool | 0 | 0 | defined in the registry (default true); no stored value |
  | `Render_LODClass` | enum | 974 | 1,042 | mostly `SmallDecor` (746 own, 786 resolved) |
  | `Entity_CreationType` | enum | 1,461 | 1,515 | mostly `SharedClientOnlyImmobile` (1,176 own, 1,230 resolved), i.e. one shared instance source per object kind |

  So 45 frill kinds take their sway intensity only from their parent. Six frill-layer
  objects carry an intensity of their own that differs from their parent's; their own
  value wins (§9.2 rule 2).

The frill engine's rules (BIN), with S = 10 units the frill sector size (160 / 16):

1. **Radius.** The draw-distance level L of `Render.FrillDistance` gives a radius of 0, 8,
   10, 12, 14 or 16 sectors for L = 0 … 5 (any other level: 0, no frills). When
   `Render.DialBackGraphics` is on the radius is halved (integer division). A renderer
   capability level of the graphics back end below 2 forces the radius to 0 (the level's
   meaning is OPEN; Tessera Decision: frills enabled). `FrillEngine.ExtraPreCacheRadiusLevels`
   rings are generated beyond the radius ahead of need.
2. **Far distance.** D = (radius − 0.3) × S units when `FrillEngine.UseDepthTransitions` is
   on, else D = 1,000 units. With the default level 2: D = 97 units.
3. **Fade.** Fade length F = min(D, `FrillEngine.DepthFadeDistance` × S) (30 units by
   default) and fade start D − F (67 units by default) are handed to the frill shaders,
   which fade or scale frills out between the two (the per-vertex formula is in the frill
   shader programs, OPEN). Frill instances are drawn in groups; a group whose bounding box
   lies at a squared distance of D² or more from the camera is not drawn, and a group at a
   distance of at least D − F uses the fading shader variant.
4. **Reduction.** For a group of an object kind that allows distance reduction
   (`Render_Frill_AllowDistanceReduction`), when `FrillEngine.EnableReduction` is on and
   2 · Rd ≤ D, with Rd = `FrillEngine.ReductionDistance` × S (40 units by default): with
   d the distance from the camera to the group's bounding box, n the group's instance
   count and g its instance granule (n must be a multiple of g, else no reduction),
   f = clamp((d² − Rd²) / (D² − Rd²), 0, 1) and the client draws
   g × max(1, trunc((1 − f) × n / g)) instances.
5. **Fade-in.** A group that has just appeared fades in over 1.5 s with opacity 3t² − 2t³,
   t = age / 1.5 s.
6. **Per-object values.** For each frill object kind the engine reads
   `Render_Frill_AllowDistanceReduction` (default true), `Render_Frill_IsOpaque` (default
   true) and `Render_Frill_AnimationIntensity` (default 0, clamped to 0 … 1). It reads them
   from the kind's source entity (§8.5 step 1) through the client's general property lookup
   (BIN (m)). The defaults apply only when the lookup finds nothing. Whether that lookup
   follows the 0x47 chain is Probable, not traced (§9.2 rule 5). **Tessera Decision:**
   resolve these values, and the one of rule 7, along the chain as in §9.2 rule 2. The shader
   variant depends on whether the object is opaque, whether its animation intensity
   exceeds 0.0002 (animated sway) and on one further flag (OPEN).
7. **Terrain colour.** When `Render.FrillTerrainColor` is on (default) the engine reads
   `Render_Frill_TerrainColorIntensity` with a default of **1.0** (clamped to 0 … 1), in the
   same way as rule 6, so frills without the property blend fully toward the terrain colour
   beneath them. (The chain does not change this property's counts here: 48 own and 48
   resolved.) The
   intensity is 0 when the option is off, and also, whatever the option says, when the
   client runs on its Direct3D 9 back end (the one built into the executable) or on its
   OpenGL back end (BIN (l)); on the Direct3D 10 and Direct3D 11 back ends the property
   applies. The blend formula is in the frill shaders (OPEN, callout below).
   **Tessera Decision:** Tessera's renderer behaves like the Direct3D 10/11 back ends.

> **OPEN:** the shader-side fade or scale formula, the terrain-colour blend and the sway
> formula (frill shader programs, prefix 0x2B); the third variant flag; the renderer
> capability level of rule 1; the engine's frame-time budget (ER reports 0.0005, about 8 ms
> per 16 ms frame).

<a id="s8-4"></a>
### 8.4 Engine options and defaults (BIN)

| Option (engine name, for compatibility with the user's preferences) | Type | Default | Meaning |
|---|---|---|---|
| `Render.FrillDistance` | int | 2 | Frill draw distance level; 0 … 5 → radius 0, 8, 10, 12, 14, 16 sectors (§8.3) |
| `Render.FrillDensity` | float | 1.0 | Fraction of frill records kept (§8.2) |
| `Render.FrillTerrainColor` | bool | true | Terrain-colour blending of frills (§8.3 rule 7) |
| `Render.DialBackGraphics` | bool | false | Among other things, halves the frill radius (§8.3 rule 1) |
| `WorldRender.DrawFrills` | bool | true (Probable) | Master switch |
| `FrillEngine.DepthFadeDistance` | float | 3.0 | Fade/scale distance, in frill sectors |
| `FrillEngine.EnableReduction` | bool | true | Distance-based reduction on |
| `FrillEngine.ReductionDistance` | float | 4.0 | Start of reduction, in frill sectors from the camera |
| `FrillEngine.UseDepthTransitions` | bool | true | Depth transitions on |
| `FrillEngine.ExtraPreCacheRadiusLevels` | int | 1 | Extra rings generated ahead of need |
| `FrillEngine.PlayerPositionBlend` | float | 1.0 | Blend of player position into the ring centre |

Tessera's own settings may use these defaults; the names are listed only so that an
implementation can read a user's existing preferences if it chooses to.

<a id="s8-5"></a>
### 8.5 What the frill engine draws for a frill record (BIN)

The frill engine builds the geometry of a frill sector from all the frill records of that
sector (§8.2):

1. **One source per object.** The engine keeps one *source* per object DID: it looks the
   DID up first and creates a source only when none exists. A new source asks the client's
   entity system for an entity of the object (a creation request of its own kind, not one of
   the kinds of §7.6), with the object's properties and appearance. Those properties are the ones
   resolved along the object's 0x47 chain. This is Probable for the source entity and BIN
   for the descriptor lookup (§9.2 rule 5). If a sector's records name a source that has not finished loading, the sector is
   not built yet: the build stops without producing geometry and tells its caller that the
   sector is incomplete (BIN). That the engine then tries again later is Probable; the retry
   was not traced. **Tessera Decision:** build a sector as soon as all of its sources are loaded. If a source is a shared entity (`Entity_CreationType` shared)
   with an appearance change, the client completes the appearance change at once and logs a
   diagnostic; the geometry of step 3 is then taken from the changed model.
2. **No filter.** Every frill record is used. The engine tests neither
   `Entity_CreationType`, nor any other property, nor the part count, nor whether the
   object was reached through a frill layer. It reads only the per-object values of §8.3
   rules 6 and 7.
3. **Geometry.** From the source's model the engine takes the **first entry of its part
   list**, that part's mesh, the mesh's **detail level 0** (the first kind-1 level,
   `mesh.md` §8.3) and, of that level, **only the first draw entry**: its vertex block,
   index buffer and surface. Nothing else is used: no other part, no other draw entry of
   level 0 and no other detail level. Frills therefore have no distance LOD; rule 4 of §8.3
   thins instances instead. The one-triangle rule of `setup.md` §8.5 plays no part: the
   first part is used as it is. In this install it is a one-triangle placeholder only for
   the 8 objects whose only part is one (§8.3).
4. **Groups.** Records are grouped by the surface state of the draw entry, the opaque flag,
   the reduction flag and the third flag of §8.3 rule 6. Records of different objects that
   agree in all four share a group.
5. **Baking.** For every record, with position p, orientation Q and per-axis scale s, and
   for every vertex of the vertex block (model space, position v, normal n), the engine
   writes one world-space vertex:
   - position p + Q(s ⊙ v): the scale multiplies each model axis before the rotation;
   - normal Q(n): rotated, not scaled;
   - texture coordinates: only **set 0** of the vertex is copied, unchanged; any further
     texture-coordinate sets of the vertex block are dropped (BIN (n));
   - a **sway weight** w = clamp((v.z − z_min) / h, 0, 1). Here z_min and z_max are the z
     components of the minimum and maximum of the vertex block's stored bounding box
     (`mesh.md` §6.1), not recomputed from the vertices. h = max(0.0002, z_max − z_min), and
     w = 0 when h ≤ 0.01 (flat blocks do not sway). v.z is the unscaled model-space z;
   - per-record terrain-colour and animation inputs of the frill shaders (§8.3 rules 6–7;
     formulas OPEN, §8.3 callout).

   The block's indices are copied, each increased by the number of vertices already written
   for the sector.
6. **Buffer limits.** All frill vertices of one sector go into one vertex buffer and one
   index buffer. The indices are 16-bit, and the running vertex count added to them is kept
   in 16 bits. A sector with more than 65,536 frill vertices therefore gets wrapped, wrong
   indices; the client does not split the buffer. Frill-layer objects have at most 2,134
   vertices each, so this needs dozens of instances in one 10 × 10-unit sector. A late group
   object has up to 9,654, and 13 of them have 6,554 or more, so ten instances in one sector
   are enough. **Tessera Decision:** use 32-bit indices, or split the sector's geometry every
   65,536 vertices. Never reproduce the wrap.

> **OPEN:** whether the order of the part list that the frill engine reads is always the
> order of the setup's part list (`setup.md` §6.6). The engine reads the same list as the
> client's part draw list (`setup.md` §8.5). That the two orders agree is Probable. It
> matters only for the 12 multi-part objects of §8.3; in all 12 the stored first part is a
> real mesh. **Tessera Decision:** use the first part of the setup's part list.

<a id="s8-6"></a>
### 8.6 Frill records of ordinary scenery objects (client behaviour BIN; Tessera Decision)

**What the client does.** A late group object (§8.1 source (b)) that passes the frill pass's
tests becomes a frill record. Within the frill radius and after density thinning (§8.2,
§8.3), the frill engine draws it like any frill, as §8.5 describes:
- only the first draw entry of level 0 of its first part is drawn;
- it is drawn with the frill shaders;
- the terrain-colour intensity is 1.0, because none of the 452 objects carries
  `Render_Frill_TerrainColorIntensity`;
- the sway weight follows §8.5, and the animation intensity comes from the property
  (default 0).

Nothing compares frill records with the entities of the static pass. When the static pass
placed the same object at the same frame (same seeds and node, and both passes accepted it),
the client draws it twice in the same place: once as an entity (all parts, detail levels,
ordinary shaders) and once as frill geometry. Whether this is visible in the original's
frames (coincident surfaces, terrain tint) could not be checked without a frame capture.

> **OPEN:** how the original client's frames look where a late group object is drawn twice.
> Answering this needs an M3 frame capture at a vertex whose scene is one of the 84 scenes
> of §8.1 source (b). It does not affect the Tessera Decision below.

**Tessera Decision:**
- The frill pass notes for each frill record whether it was created **inside** a frill
  layer or **after** one. Inside means that at least one frill layer is being processed, at
  any depth: the layer node, its nested nodes, and every scene it references, recursively,
  including the frill layers of those scenes. After means that the state is set but no frill
  layer is being processed.
- Records created after a frill layer are discarded. They are not given to the frill engine
  and not drawn.
- Everything else stays as §8.2 describes it:
  - the state still becomes set and stays set until the vertex ends;
  - footprint markers are appended or not as before;
  - the landblock and density tests are unchanged. The density generator is unseeded, so
    discarding a record changes no other result.
- Result: exactly the frill-layer objects (source (a)) reach the frill engine. Every late
  group object appears once, as the entity that the static pass made of it.

---

<a id="s9"></a>
## 9. Scenery objects, meshes and LOD (Confirmed census; distances OPEN)

- Every object DID in a scene is a 0x47 entity descriptor that resolves through
  `setup.md` to a setup and meshes (4,261 of 4,261). Non-frill scenery objects: 2,715 of
  2,746 single-part; median 742 triangles (5 % ≤ 50, 95 % ≤ 15,024). The 2,746 include the
  452 late group objects of §8.1 source (b), which the client also feeds to the frill
  engine (§8.6).
- Properties on the non-frill scenery objects (2,746; 115 of them have a parent 0x47
  record). Each count is given over each record's **own** set and **resolved** along the
  0x47 chain (§9.2):

  | Property | Own set | Resolved | Values (own / resolved) |
  |---|---|---|---|
  | `Entity_CreationType` | 2,685 | 2,746 | `SharedStaticImmobile` 1,575 / 1,587, `UniqueStaticImmobile` 634 / 660, `SharedClientOnlyImmobile` 437 / 457, `UniqueClientOnlyImmobile` 39 / 42 |
  | `Render_LODClass` | 2,376 | 2,441 | `Tree` 907 / 941, `SmallStructure` 351 / 367, `LargeDecor` 300 / 315, `LargeStructure` 235 / 235, `MediumDecor` 203 / 203, `Landmark` (0x10000010) 121 / 121, `SmallDecor` (0x10000012) 121 / 121, `Bush` 107 / 107, `Creature` (0x10000008) 19 / 19, `FarView500` (0x1000000F) 8 / 8, `Equipment` (0x10000013) 4 / 4 (all eleven classes present; their sums are the two counts) |
  | `Render_CastLandscapeShadows` | 2,147 | 2,241 | |
  | `Entity_Clearance_BoxList` (boxes with `Entity_Clearance_Box_Min`/`Max`) | 1,257 | 1,325 | the clearance volumes of §7.4 step 14 |
  | `Entity_Clearance_CylinderList` (`Radius`, `Height`, `LowPoint`) | 296 | 354 | as above |
  | `Render_ImposterName` | 840 | 844 | distant tree imposters |
  | `Render_LODCrossFade` | 341 | 341 | true in 14 |
  | `Render_FarViewDistance` | 8 | 8 | 1,000 – 2,000 units |

  65 objects take their LOD class only from their parent, and 12 carry a class of their own
  that differs from their parent's (their own wins, §9.2 rule 2). 61 take their creation
  type only from their parent, and 13 override their parent's.
- The LOD class labels come from enum mapper 0x2300008D (read at run time). The registry
  default of `Render_LODClass` is 2. The client's traced reader of the property does not
  apply that default (§9.2 rule 6). **Tessera Decision:** an object with no
  `Render_LODClass` on any level of its chain uses class 2 for Tessera's draw distance
  (§9.1). In this install that is 305 non-frill objects resolved along the chain (370 by
  own set alone).

<a id="s9-1"></a>
### 9.1 The LOD-class record, prefix 0x32 (layout in `environment.md` §9.1; values OPEN)

`environment.md` §9.1 owns the layout of this record (provenance E15 there; `records.md`
§2.1 names that spec). In short: the client's table of DID ranges assigns 0x32000000 –
0x32FFFFFF to its LOD-class record type (BIN), and the install holds one record,
0x32000000 in `client_general.dat`, whose 1,160-byte payload is its DID, a u32 `count`
(32) and `count` entries of 36 bytes (u32 `class`, then eight f32 `d0` … `d7`), with no
other field and zero leftover bytes (OBS). Field names here follow `environment.md`.

What scenery needs from it:

- `class` is a `Render_LODClass` value (§9): the keys are 1, 2 and 3 (no label in the
  mapper) and the 29 labelled values 0x10000001 – 0x1000001D, each once (OBS). Objects
  with no `Render_LODClass` on their chain use class 2 by Tessera Decision (§9, §9.2
  rule 6).
- `d0` ranges from 25 (`SmallDecor`) to 4,000; `Tree` has 140, structures 180 – 300,
  landmark classes 1,000 – 4,000 (OBS; full value census in `environment.md` §9.1). A
  reader reads the record at run time and never hard-codes the values.

> **OPEN:** what the eight values mean (draw distance, detail-level steps, fade lengths,
> shadow and imposter distances are the candidates; the record's own reader was not
> found; one consumer of one value is described in §9.2 rule 6), and how imposters are
> chosen and drawn. **Tessera Decision** until then: `d0` is the class's maximum draw
> distance in units (scaled by Tessera's own draw-distance setting), and the other values
> are ignored.

<a id="s9-2"></a>
### 9.2 Resolving an object's properties along its 0x47 chain (BIN; counts Confirmed)

A 0x47 record can name another 0x47 record as its template, its **parent** (`setup.md`
§3.1 step 2). Of the 4,261 scene objects, 197 have a parent. They have 156 distinct
parents: 128 are scene objects themselves and 28 are not, so the chain covers 4,289
records. None of the 156 has a parent of its own (OBS). A 0x47 record can also name
0x78 **extra sets** (`setup.md` §4.4). What the client does (BIN (m)):

1. **Own set.** The client combines a record's extra sets in list order, each later set
   overriding the earlier ones, and then puts the record's embedded property set on top. The
   embedded value of a property wins over an extra set's value. In this install no extra
   set named by a scene object or its parent holds a `Render_*` or `Entity_*` property
   (OBS), so this step changes no count of this spec.
2. **Lookup.** To read a property of a descriptor, the client looks in the record's own set.
   If the property is not there and the record's template is a 0x47 record, it looks in
   the parent in the same way, recursively. The first level that has the property gives
   the value, so a record's own value hides its parent's. If no level has it, the lookup
   finds nothing. The lookup itself applies no registry default; each reader applies its
   own default, as stated where the reader is described.
3. **Merge.** When the client builds an entity's property set from a descriptor, it merges
   the chain: the parent-most level first, then each nearer level, overriding values already
   present, and the record's own set last. The result gives the same value for every
   property as rule 2. The client builds the property sets of landblock-info static entities
   this way, with the static entity's own set as the nearest level and the named 0x47 chain
   behind it. That is the order `entity-descriptor.md` E22 chose for the draw properties.
4. **Readers that use rule 2.** These readers use the lookup of rule 2 (BIN (m)):
   - the `Entity_CreationType` stored with every generated object (§7.6);
   - `Render_LODClass` and `Render_FarViewDistance` in the reader of rule 6.
5. **The frill engine.** The frill engine reads its per-object values (§8.3 rules 6–7)
   through the client's general property lookup on the frill source entity (§8.5 step 1),
   not through rule 2 directly. The source entity is created from the object DID by a
   creation request. The descriptor load path that turns a descriptor DID into an entity's
   property set uses the merge of rule 3. That the frill source's request takes this path
   is **Probable**: the link between the frill engine's request and that load path was not
   traced end to end.
6. **A missing LOD class.** One reader of `Render_LODClass` was traced (BIN (m)). It
   computes a per-object distance as the larger of two values. The first is a value from
   the object's entry in the LOD-class record (§9.1), or −1 when the chain has no
   `Render_LODClass` or the record has no entry for it. The second is
   `Render_FarViewDistance`, or −1 when absent. The value taken from the entry sits at the
   position of `d6` if the client keeps the record's field order. That is Probable: `d6` is
   −1 in 31 of the 32 classes (`environment.md` §9.1). The reader then compares the result
   with 250 and with 160 units. A result in 0 … 160 that is not 0 is raised to at least 25.
   What these outputs control is OPEN. The point for this spec: an object without a class
   on its chain gets −1 here, **not** the value of class 2. Whether other consumers, such as
   the renderer's detail-level choice, apply the registry default 2 was not traced.

**Tessera Decision.** Every per-object property read in this spec (§7.6, §8.3 rules 6–7,
§8.5, §9, §10) resolves the value by rules 1–2: the own set, then the parent, recursively.
A default (the registry default, or the default stated in the rule that reads the
property) applies only when no level has the property. An object with no `Render_LODClass`
on its chain uses class 2 (§9).

Effect of the chain on this install (OBS, fifth pass; full counts in §8.3 and §9):

| Objects | Property | Own set | Resolved | Objects whose value comes only from the parent | Own value differs from the parent's |
|---|---|---|---|---|---|
| 1,515 frill-layer | `Render_Frill_AnimationIntensity` | 929 | 974 | 45 | 6 |
| 1,515 frill-layer | `Render_LODClass` | 974 | 1,042 | 68 | 0 |
| 1,515 frill-layer | `Entity_CreationType` | 1,461 | 1,515 | 54 | 2 |
| 1,515 frill-layer | `Render_Frill_TerrainColorIntensity`, `Render_Frill_IsOpaque` | 48, 5 | 48, 5 | 0 | 0 |
| 2,746 non-frill | `Render_LODClass` | 2,376 | 2,441 | 65 | 12 |
| 2,746 non-frill | `Entity_CreationType` | 2,685 | 2,746 | 61 | 13 |
| 2,746 non-frill | `Render_Frill_AnimationIntensity` | 57 | 57 | 0 | 0 |
| 452 late group (in the 2,746) | all of §8.3 | same | same | 0 | 0 |

> **OPEN:** (a) whether the frill source entity's property set is the merged chain of
> rule 3 (Probable; rule 5); (b) what the distance and the two comparisons of rule 6
> control, and whether any other reader of `Render_LODClass` applies the registry default
> 2 (§15 item 16). The Tessera Decision above stands until these are traced.

---

<a id="s10"></a>
## 10. Wind and foliage animation (properties Confirmed; algorithm OPEN)

- Hand-placed static entities carry `WindSpeed` (enum mapper 0x23000362: Undef 0,
  Medium 1, Low 2, High 3, Interior 5, LowMed) on 18,334 entities (`environment.md` §7.4,
  ED). None of the 4,261 scene objects carries it.
- Frill objects carry `Render_Frill_AnimationIntensity` (§8.3: 929 own set, 974 resolved
  along the chain, §9.2); 57 non-frill scene objects carry it too (57 either way).
- The shader programs (prefix 0x2B) contain wind constants (current and previous wind
  value, phase, height factor, maximum displacement, vertical motion) in 3 programs, a
  "reactive frill" configuration in 4 programs, and generic per-axis vertex-displacement
  constants (amplitude, frequency, speed for x, y, z) in 7 programs.

> **OPEN:** how the client turns `WindSpeed` and the animation intensity into those shader
> constants, and the displacement formula in the vertex shader. **Tessera Decision** until
> then: sway vertices by height above the object origin with Tessera's own formula, scaled
> by the class (`WindSpeed`) or intensity.

---

<a id="s11"></a>
## 11. Water surface used by the checks (see `water.md`)

The height modes and water ranges of §7.4–§7.5 and the scale modes 5–7 read a
**per-landblock water surface** that the client keeps with the landblock's data (BIN): a
flag saying whether the landblock has water, and a plane (unit normal and offset, the same
form as the terrain plane of §7.4 step 8). Where the flag and the plane come from is
specified in `water.md` §6.2 (which per-region table and block-map keys, the height
*h* = level × 1000 / 255 and the presence rule) and §6.3 (the plane: normal (0, 0, 1),
offset −*h*, over the whole block). This spec no longer gives its own reading; its earlier
statement that no landblock has water is withdrawn (`water.md` §6.4 counts 89,913 landblocks
with water in this install). Generation uses the surface as follows:

- no water: `height_mode` 1 rejects every candidate, §7.5 step 3 applies, and the scale
  modes 5–7 reject;
- with water: the water height at (x, y) is −(n.x·x + n.y·y + d) / n.z of its plane when
  |n.z| > 0.0002, else the candidate's own z; with the plane of `water.md` §6.3 this is
  *h* everywhere in the block. `height_mode` 1 snaps to *h*, and the water-depth and
  above/below-water ranges of §7.5 use w = *h* (step 2 applies only where *h* − g > 0.0002).

---

<a id="s12"></a>
## 12. Census of this install (Confirmed, 2026-10-05)

Over the landblock-data records whose coarse grid is not all zero (vertices counted per
record, shared edges counted twice):

| Region | Blocks | Vertices with terrain 0 | Vertices with an unknown terrain id | Distinct (terrain, slot) | Vertices with a group scene | … in blocks | Vertices with a frill layer | … in blocks | Terrain ids used / in table |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 21,049 | 661,281 | 0 | 751 | 1,897,884 | 12,259 | 3,306,724 | 13,608 | 78 / 99 |
| 2 | 12,037 | 1,092,264 | 13,380 | 672 | 2,108,222 | 11,689 | 1,959,322 | 10,508 | 37 / 38 |
| 3 | 11,954 | 1,395,657 | 34 | 478 | 2,002,790 | 11,269 | 1,512,593 | 9,357 | 28 / 29 |
| 4 | 17,623 | 1,977,907 | 3,870 | 610 | 2,974,234 | 16,505 | 2,432,274 | 14,294 | 31 / 47 |
| 5 | 14,350 | 1,227,929 | 58 | 1,057 | 2,737,116 | 13,707 | 2,487,842 | 12,626 | 44 / 47 |
| 14 | 1,101 | 105,533 | 194 | 127 | 200,778 | 1,078 | 156,199 | 908 | 16 / 38 |
| **Total** | 78,114 | 6,460,571 | 17,536 | | 11,921,024 | 66,507 | 11,854,954 | 61,301 | |

- Vertices whose slot is not in their terrain entry (no scene): 7,295 of 22,574,946
  (0.03 %; region 5: 6,814). Looking slots up by position instead of by value would give
  14,343 misses and 956 vertices with a different scene key, so the by-value rule of §4.2 is
  the one the data fits.
- Scene keys missing from the paired scene-choice table: 2 pairs (terrain id 200 slot 1 in
  0x12000000, terrain id 34 slot 8 in 0x12000001).
- "Group scene" counts vertices whose chosen entry offers at least one scene with a
  non-empty group; the frequency test then decides how many objects appear.

---

<a id="s13"></a>
## 13. Invariants (Confirmed on every record unless marked)

- **SC1**: every 0x11, 0x12 and 0x02 record decodes with §4.2, §4.3 and §5 to its last
  byte.
- **SC2**: every scene DID in a 0x11 record exists (2,472 of 2,472); every scene reference
  in a 0x02 record exists (0 dangling); the scene-reference graph is acyclic.
- **SC3**: every object DID in a scene is an existing 0x47 record (4,261 of 4,261).
- **SC4**: group ids are 1; frill-layer children are scene references only; every 0x11
  choice `flag` is 1; `scale_mode` is 4 in every node; `height_mode` ∈ {0, 1}; child type 4
  never occurs.
- **SC5**: within every 0x12 entry, slots are distinct; the first pair of every entry of the
  list is (2, 0) and the default entry is the 41-byte entry of §4.2; within a record,
  terrain ids are distinct.
- **SC6**: coarse-grid bits 15–31 are 0; edge words equal the neighbour's (§3.2).
- **SC7**: every surface-layer `material` and quad-grid value of a landblock is a key of
  its region's 0x19 table.
- **SC8** (determinism, BIN): generating the same landblock twice yields identical
  objects, frames and scales (frill thinning excepted, §8.2).
- **SC9**: the 0x32 record decodes with `environment.md` §9.1 to its last byte:
  8 + `count` × 36 equals the payload size (1,160 with `count` 32); the `class` keys are
  distinct (32 of 32).
- **SC10**: every node with `clearance` 0 has (`clear_max_cos`, `clear_min_cos`,
  `clear_min_dot`, `clear_aspect_tol`) = (1, 6.1 × 10⁻¹⁷ (cos 90° as an f32), −1, 360)
  (49,405 of 49,405); every node has `clear_aspect_tol` 360.
- **SC11** (BIN, testable on synthetic fixtures): every generated orientation is a unit
  quaternion; with `align_to_slope` the rotated model +Z equals the step-8 normal; without
  it and with a2 = a3 = 0 the rotated model +Y has compass bearing h (`coordinates.md`
  §11.4).
- **SC12** (Tessera Decision of §8.6, synthetic fixture): a vertex scene whose group holds a
  scene reference to a scene with a frill layer, followed by an object X, yields an entity
  for X in the static pass and **no** frill record for X in the frill pass. The objects of
  the referenced scene's frill layer still yield frill records.
- **SC13** (§8.5 BIN; the last sentence tests the Tessera Decision of §8.5 step 6;
  synthetic fixture): a frill record of a two-part object whose first part's
  level 0 has two draw entries contributes exactly the vertices and indices of the first
  draw entry of the first part. Its baked positions equal p + Q(s ⊙ v), and its baked
  texture coordinates equal set 0 of the source vertices when the vertex block has two
  sets. A sector holding more than 65,536 frill vertices still draws every triangle with
  its original vertices.
- **SC14** (§9.2 rules 1–2 and the Tessera Decision there; synthetic fixture): a frill-layer
  object A whose 0x47 record has no `Render_Frill_AnimationIntensity`, with parent P that
  has 0.2, gets intensity 0.2 (animated sway). If A's embedded set has 0.1, A gets 0.1. If
  A names an extra set with 0.3 and its embedded set has 0.1, A gets 0.1. With the extra set
  alone, A gets 0.3. With no value on any level, A gets the default 0. A non-frill object
  with no `Render_LODClass` on itself or its parent uses class 2 (Tessera Decision); one
  whose parent has `Tree` uses `Tree`.
- **SC15** (OBS): resolved along the 0x47 chain, every one of the 4,261 scene objects has
  `Entity_CreationType` (1,515 + 2,746); every parent named by a scene object exists and
  is a 0x47 record.

<a id="s13-1"></a>
### 13.1 Hostile-input notes

- Bound every count by the remaining bytes (`count`, `pair_count`, `n`, `group_count`,
  `child_count`); reject a child type outside 1–3.
- Scene references can form cycles in hostile data: track the scenes on the current
  instantiation path and refuse a repeat; cap the depth (5 is the observed maximum; Tessera
  Decision: a cap of 16) and the total objects per vertex.
- Treat NaN or infinite floats in node fields as malformed except the ±FLT_MAX `z_range`.

> **OPEN (raised by the `formats` implementation, 2026-10-06):** ±FLT_MAX is finite, so the
> exception above does not say whether a `z_range` bound may be ±infinity. No record has one
> (always ±FLT_MAX). The decoder (`tessera_formats::scenery`) accepts ±infinity in
> `z_range`, which reads as "no bound" under §7.5 step 1, and rejects NaN there and NaN or
> ±infinity in every other node float, `below_ground_max` included. Floats of the 0x11
> choices (`weight`) and of 0x12 entries (`unknown_f`) are not checked; §4.3 says nothing
> about non-positive or non-finite weights. As implemented, the pick of §7.2 takes the last
> choice whenever no running sum compares greater than T (for example when a weight is NaN).

> **OPEN (raised by the `world` implementation, 2026-10-06):** hostile values the spec does
> not cover. (1) A negative `spacing` (any finite value decodes; every record has
> `spacing` ≥ 0) makes ρ negative, and the bound of §7.4 step 6, ρ ≤ p.x < 160 − ρ, then
> accepts points outside the block; step 8's "step 6 rules out an i or j outside 0 … 32"
> no longer holds. The client's behaviour is not traced. **Tessera Decision:** step 6
> also requires 0 ≤ p.x < 160 and 0 ≤ p.y < 160, which changes nothing for ρ ≥ 0;
> negative ρ is otherwise used as the steps state (ρ² in step 14a, ρ + ρ_m in step 14b).
> (2) A static entity whose rotation (§7.4 step 14a) is not finite, has zero length, or is
> far from unit length: the inverse rotation is the conjugate only for a unit quaternion.
> **Tessera Decision:** such an entity is left out of the static list when its position
> or rotation is not finite or the rotation has zero length; a finite rotation further
> than 0.001 from unit length is normalised first; a mapped point that is not finite
> overlaps nothing. (3) A landblock-info record may name tens of thousands of static
> entities with up to 256 volumes each. **Tessera Decision:** the static list keeps at
> most 16,384 volumes per landblock and each step-14a scan is charged to the generation
> work budget (`tessera_world::scenery`).

---

<a id="s14"></a>
## 14. Worked synthetic example

Synthetic inputs (not taken from any record): region R = 2, landblock X = 48, Y = 80,
coarse vertex (i, j) = (3, 7).

- Vertex index 17·3 + 7 = 58; B = 58 + 0x003556F7 = 0x00355731. A = 48 + 80 + 2 + 5 = 135.
- Scene pick (§6.1): gx = 771, gy = 1,287; v = 0x7237E00C, u = 0x7237 / 65,536 = 0.44615.
  With choices weighted (1.0, 1.0, 2.0), T = 0.44615 × 4 = 1.7846: running sums 1.0, 2.0 →
  the second choice is picked (2.0 > 1.7846).
- Scene seed S = 0x000923B1 · 0x000923B1 = 0x876BE061. The first group child (k = 0) has
  C = 0x876BE061.
- Frequency: s = C + 0x01191E67 = 0x8884FEC8; v = 0xEC949086; R = 0.924133. A node with
  `frequency` 0.9 is skipped; one with 1.0 is placed.
- Displacement: R(x) = 0.611084, R(y) = 0.134033; with (−5, −5)…(5, 5):
  d = (1.11084, −3.65967).
- Heading: R = 0.233948; with (0, 359): 83.99°.
- Uniform scale: R = 0.306381; with 0.8 … 1.2: 0.92255.
- The second group child has C = 0x876BE061 + 0x2D41BF19 = 0xB4AD9F7A; its frequency draw
  is 0.752548. The frill layer uses C = S + 0x0380430A = 0x8AEC236B.

A test fixture can assert these numbers.

**Orientation (§7.4 step 10).** With the heading draw above, h = 0.233948 × 359 = 83.987°.
Without `align_to_slope`, Q = Rz(−83.987°) = (w, x, y, z) = (0.74322, 0, 0, −0.66905);
model +Y goes to (0.99450, 0.10475, 0), compass bearing 83.987°. With `align_to_slope` and
a synthetic normal n = (0.2, −0.3, 0.93274): L = 0.97980, X′ = (0.97980, 0.06124,
−0.19039), Y′ = (0, 0.95197, 0.30619), Q_align = (0.98292, 0.15418, 0.09929, 0.01558),
and Q = Q_align · Rz(−83.987°) = (0.74094, 0.04816, 0.17695, −0.64604); Q maps model +Z
to n.

**Terrain plane band (§7.4 step 8).** Quad (i, j) = (3, 7) with bit 7 clear, point
(17.5, 37.0): fx = 2.5, fy = 2.0. Geometrically the point is in the south-west triangle
(fx + fy = 4.5 < 5, key 2 · (32 · 7 + 3) = 454), but fy = 2.0 > 4 − 2.5 = 1.5, so the
client uses key 455 (the north-east triangle). At (16.0, 36.0) (fx + fy = 2) both rules
give 454; at (19.0, 38.5) (fx + fy = 7.5) both give 455. **Tie on the diagonal:** the same
quad with bit 7 set and the point (17.5, 37.5) has fx = fy = 2.5; the client uses key 454
(the north-west triangle), whereas `landblock.md` §3.11 would take the south-east one
(key 455).

**Clearance query (§7.4 step 13, `collision.md` §6.6).** p = (12.0, 13.0), r = 0.5: only
cell (2, 2) is in range; p lies 0.7071 from the cell's south-west/north-east diagonal, more
than r, and on its north-west side, the other side from the south-east corner, so the
query collects key 2 · (32 · 2 + 2) + 1 = 133 alone.

---

<a id="s15"></a>
## 15. Open questions (summary)

1. (Closed: decision 0012 allows the binary-sourced constants of §6–§8 in specs and code.)
2. `kind` and `unknown_f` of terrain entries (§4.2 callout) and the two-byte name length
   form (§4.2); the 0x11 choice `flag` (§4.3). (The default entry's "extra byte" of an
   earlier draft does not exist; §4.2 corrected.)
3. (Closed: the 0x19 entry is decoded in `terrain-texturing.md` §3.3.)
4. `flag_a` of the scene record (§5.1 callout). (Offsets 143–155 are now named; 163 and 174 are not
   read by the placement code.)
5. (Closed: rotation conventions, §7.4 step 10.) Remaining: the sphere-against-cylinder
   variant of the spacing test with `spacing_3d` (§7.4 step 14a; unused here).
6. (Closed: the clearance step and the spacing lists, §7.4 steps 13–14.) Remaining: which
   run mode of the client's static-entity filter applies offline (§7.4 step 14a).
7. Seed A: what fills the landblock-info list that switches A to 1 (§6.3).
8. Frills: the shader-side fade or scale formula, the terrain-colour blend and the sway
   formula (frill shader programs); the third shader-variant flag and the renderer
   capability level (§8.3). (Closed: the two back ends without terrain colour, §8.3 rule 7.)
9. Meaning of the eight values of the LOD-class record and how imposters are chosen
   (§9.1; layout owned by `environment.md` §9.1).
10. Wind: mapping from `WindSpeed` and animation intensity to shader constants (§10).
11. (Closed: the per-landblock water surface is specified in `water.md` §6.2–§6.3; §11.)
12. Whether the 3 × 3-landblock frill "source groups" reported by ER, and the kind-5 records
    `0x805RXXYY` that exist once per 3 × 3 group (`landblock.md` §7), feed the frill engine
    (for example as terrain colour for frill blending). Not needed to place frills (§8.2).
13. Frill pass: whether the static pass's footprint markers are still in the generated list
    when the frill pass runs, and when the frill pass's own markers are removed (§8.2).
14. Frill engine: whether the part-list order it reads always equals the setup's stored order
    (§8.5 callout; Probable, matters for 12 objects).
15. How the original's frames look where a late group object is drawn both as an entity and
    as frill geometry (§8.6; needs an M3 frame capture; Tessera does not draw the frill copy).
16. Property resolution (§9.2 callout): whether the frill source entity's property set is
    the merged 0x47 chain (Probable). What the distance and the 250 and 160-unit
    comparisons of the traced `Render_LODClass` reader control, and whether any other
    reader applies the registry default class 2. Until then, Tessera resolves every
    per-object property along the chain and uses class 2 when no level has a class.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-05 | First draft: per-vertex scenery word, region scene tables 0x11/0x12, terrain-type flag in 0x19, scene record 0x02 grammar, placement algorithm and hashes (BIN), frills, census. |
| 2026-10-05 | Second pass (BIN and OBS), status Draft → Review. §4.4 now defers the 0x19 layout to `terrain-texturing.md` §3.3 and looks entries up by key (the stale "body grammar OPEN" and "keep stored order" removed). §5.3 names offsets 143–155 (clearance flatness test) and marks 163 and 174 as unread. §7.1 states what exists before generation. §7.3 uses the client's height constant. §7.4: terrain plane from the client's terrain mesh with its band rule (step 8); heading sign, Euler axes and the `align_to_slope` frame (step 10); height modes in full (step 11); the clearance step is a terrain flatness test using `collision.md` §6.6 (step 13); the spacing lists are the landblock-info statics with their clearance volumes and the footprint markers of placed nodes (step 14); markers and `max_objects` (step 15). §7.6 creation path. §8.3–§8.4 frill radius per level, far distance, fade, reduction, fade-in, per-object defaults, terrain-colour default on. §9.1 the LOD-class record 0x32. §11 now points to `water.md` §6.2–§6.3 instead of its own 0x17 reading, and the earlier "no landblock has water" statement is withdrawn (89,913 landblocks have water, `water.md` §6.4). New invariants SC9–SC11, worked examples for orientation, the band rule and the clearance query, §15 updated; the note on hash constants and §15 item 1 now cite decision 0012; step 10 names the composition after `coordinates.md` §11.3. The default 0x12 entry's label is described by length only. |
| 2026-10-05 | Revision answering the spec review (third pass, OBS and BIN (i), (j)). §4.2: the 0x12 default entry has the same layout as every other entry (the "extra zero byte" of the earlier text does not exist; all 5 records decode with zero leftover bytes); entry offsets corrected for the name's length byte (`kind` at n + 1, not n); the two-byte name length is a proper OPEN; S4 corrected. §4.1: stale correction note for `environment.md` removed. §5.3: `frequency` ≥ 1 in 25,582, exactly 1.0 in 25,450. §5.4: third empty scene 0x020000CB; 180 frill layers without children; the objects reachable through frill layers (1,515) are defined, with the narrower 1,512. §7.3 step 1: stale height-constant remark removed (`landblock.md` §3.4 now gives the client's rule). §7.4 step 8: the tie on the diagonal of bit-7-set quads (strict fx > fy, so the north-west triangle) is the client's (BIN); the client's general terrain height and plane queries use the same rules; `landblock.md` §3.11 is the geometric rule and is Tessera's, not the client's point query (S25); step 15 wording. §8.2: the frill pass processes the frill layer of every scene it instantiates; the per-vertex "under a frill layer" state and its persistence; markers and the shared generated list (S24); new OPEN on the frill pass's markers with a Tessera Decision. §9.1: the 0x32 layout is owned by `environment.md` §9.1 (no `length` or DID echo; payload 1,160 bytes); field names follow that spec; S21 and the Implements-into row updated. SC5, SC9 restated; worked example of the diagonal tie (§14); §15 items 2, 9 and new 13. |
| 2026-10-06 | Revision answering the re-review (fourth pass, OBS and BIN (k), (l)). §8.1 rewritten: a frill record is made of every object that the frill pass meets with its state set. There are two sources: frill-layer objects (1,515) and late group objects met after a frill layer (474 in 84 named scenes, 452 not among the 1,515), 1,967 objects in all (S26). §8.2 item 4 points to §8.1 (b), §8.5 and §8.6. §8.3: mesh census of both sources (parts, placeholders, detail levels, draw entries, drawn triangles and vertices) and properties of the 452 (S30), replacing the statement that all frill objects are single-part. Rule 7: the "two render modes" are the Direct3D 9 back end built into the executable and the OpenGL back end (S29), with a Tessera Decision. New §8.5: what the frill engine draws (one source entity per object, no filter, first draw entry of detail level 0 of the first part, baking with sway weight, 16-bit sector indices with a Tessera Decision against the wrap; S27, S28; OPEN on part-list order). New §8.6: the client draws late group objects a second time as frill geometry; Tessera Decision to discard frill records created after a frill layer; OPEN on the original's visual result. §4.2: OPEN callout for `kind` and `unknown_f`, and the quoted client log text replaced by plain words. §5.1: OPEN callout for `flag_a`. §5.4, §9 mention the 452. New invariants SC12, SC13; §15 items 2, 4, 8 updated, new 14, 15. |
| 2026-10-06 | Revision answering the third review (fifth pass, OBS and BIN (m), (n)). New §9.2: how the client resolves an object's properties along its 0x47 chain. The own set is the extra sets with the embedded set on top. The lookup tries the own set, then the parent, recursively, and the nearest level wins. Building an entity merges the chain with the same precedence. Generated objects' `Entity_CreationType` and the traced `Render_LODClass` reader use this lookup. For the frill source entity it is Probable. In the traced reader, a missing class gives −1, not class 2. A Tessera Decision resolves every per-object read along the chain and keeps class 2 as Tessera's default; OPEN callout and §15 item 16 (S31–S34). The counts in §8.3 (1,515 frill-layer objects), §9 (2,746) and §10 are now labelled as own-set counts, and the counts resolved along the chain are added: `Entity_CreationType` 1,515 and 2,746, `Render_Frill_AnimationIntensity` 974, `Render_LODClass` 1,042 and 2,441, and others (S33). §8.3 rules 6–7 state where the per-object values come from. §7.6 and §8.5 step 1 no longer say "own properties". §8.5 step 1 says in plain words that an incomplete sector is not built. §8.5 step 5 cites `mesh.md` §6.1 for the stored bounding box and says that only texture-coordinate set 0 is copied (S35). §9 and §9.1: the class-2 default is a Tessera Decision. New invariants SC14 and SC15; SC13 extended. Companion specs updated. |
| 2026-10-06 | Revision answering the fourth review (sixth pass, OBS; every count re-measured over all 4,261 objects, their parents and the 2,746 non-frill objects). §9.2 intro and the OBS provenance row: the 197 objects with a parent have **156** distinct parents (128 are scene objects themselves, 28 are not), not 28; 28 is the number of parent records outside the 4,261, which makes 4,289 records in all. None of the 156 has a parent of its own, so the chain depth of one is unchanged. §9: the `Render_LODClass` value list now names all eleven classes present, adding `Landmark` and `SmallDecor` (121 / 121 each), `Creature` 19 / 19, `FarView500` 8 / 8 and `Equipment` 4 / 4; the own and resolved values sum to 2,376 and 2,441. The other counts of §9 (creation-type values, `Render_CastLandscapeShadows`, clearance lists, `Render_ImposterName`, `Render_LODCrossFade` true in 14, `Render_FarViewDistance` 1,000 – 2,000, 2,715 single-part, triangle median and percentiles, 305 / 370 without a class, 65 / 12 and 61 / 13) were re-measured and are unchanged. |
