<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Mesh records (prefix 0x06): geometry, vertex formats, detail levels, collision

| | |
|---|---|
| **Status** | Accepted (revision of 2026-10-07 and its review changes re-reviewed and passed, 2026-10-07; implementers re-check code built on the earlier `d6`/`d7` naming, §20) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-reviewer, a different session from the writer) |
| **Review record** | Re-review of the review changes, 2026-10-07: **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/mesh` (decoder, engine vertex streams), `crates/render` (pipelines, detail-level choice §8.4–§8.8), `crates/world` (draw distances, occluders §8.9), `apps/explorer` (mesh viewer), `fuzz/` (mesh target) |
| **Companion specs** | `dat-container.md` (archive set, routing, the directory entry `stamp` field this spec needs in §10.4). `records.md` §1 (primitive encodings). `material.md` (surface 0x31 → material 0x30 → texture list; not repeated here). `texture-list.md` (0x40 → 0x41). `setup.md` (which records reference meshes, and how parts are assembled; §8.5 one-triangle parts). `environment.md` §9.1 (layout of the LOD-class record 0x32 that §8.5 uses). `collision.md` §3, §5, §7 (the tree code of §10, cell geometry). `skeleton-anim.md` §4.2, §9.2 (influence list, part slots). `lighting.md` §4.3, §5.12–§5.13 (vertex inputs the programs read). `property-system.md` §8 (DID maps). |

---

<a id="s0"></a>
## 0. Purpose

A mesh record (DID prefix 0x06, the "GfxObj" family) holds the triangle geometry of one
renderable part: a building section, a prop, a weapon, a body or garment. This spec gives an
implementer everything needed to decode **every** mesh record of the install into vertex and
index streams, without heuristics:

- the record layout (§2–§3);
- the per-submesh **vertex format word** and the deterministic stride and attribute layout
  derived from it (§4), proven over all records;
- every vertex attribute encoding, including bone indices and weights (§5);
- per-submesh bounds and the influence list (§6), index buffers (§7);
- detail levels and draw lists (§8), how the client chooses a detail level by distance and
  what the second level kind is for (§8.3–§8.10), and the collision data stored behind them
  (§9–§10);
- coordinate system, units, handedness, winding and UV origin (§11);
- how a mesh names its surfaces (§12);
- the decoding algorithm in words, invariants, statistics and failure categories (§13–§15).

All counts are observations of one install on one date. They change with patches. Tests
assert the invariants (§14), not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install on 2026-10-04 (the CNS census on
  2026-10-05) by throwaway probes in the dirty-room scratchpad. A Confirmed fact states what
  was decoded and how much of it.
- **Reported**: comes only from third-party material or analogy and was not, or could not
  be, checked against data.
- **BIN**: rests on the client's code alone (method M4, decision 0011); the data cannot show
  it. Facts marked BIN are removed if that authorisation is withdrawn.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). **All 47,563** mesh records (37,484 in `client_mesh.dat`, 10,079 in `client_mesh_aux_1.datx`) decoded to the last byte; every vertex of every submesh read and checked (89,593,149 vertices, 95,366,468 triangles). About 25 meshes and 3 multi-part assemblies were rendered with a scratch software rasteriser (textured, Z-up, no V flip) and inspected by eye: crates, walls, rocks, a tower, a statue, a cottage, two NPC bodies in bind pose, armour pieces, a tree, a gate, detail-level proxies. Images were kept in the scratchpad only. |
| MV | Mirarkitty `lotro_mesh_viewer`, `docs/mesh-format.md` and `docs/animation.md` (repository carries GPL-3.0 and MIT licence files). Facts only. |
| KC | gitlab `kilves/lotro-mesh-converter` (GPL-3.0), written against older client data. Studied for facts only; no code taken. |
| DS | `research/notes/data-semantics.md` §7.3 (summary of MV). |
| D3D | Public Direct3D 9 documentation: flexible-vertex-format (FVF) bit layout, `D3DPRIMITIVETYPE`, `D3DCOLOR`. General references, used as analogies. |
| AN | Analogy only, not verified: the client licenses Havok Physics (research notes §8); the collision tree in §10 has the shape of a Havok "MOPP" code block. |
| DC | `dat-container.md` §5.2 (directory entry `stamp`). |
| BIN | **M4 ([decision 0011](../decisions/0011-standing-m4-authorisation.md)).** The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035), disassembled in the dirty room for facts only on 2026-10-05. **Questions:** (1) how the client reads the `flags` word and the vertex format word; (2) how it chooses among kind-1 levels, from which distances, and what kind-2 levels are for; (3) whether it draws one-triangle meshes (with `setup.md` §8.5, owner item O4). **Why M1–M3 were not enough:** the records hold no switch distances, no marker of what a level is for and no meaning for `flags`; no published source describes them; observing the running client (M3) can show that a model changes with distance, but not the rule, the distances or where they come from. Only offline rendering code was read: the mesh reader, the scene graph's per-frame distance, visibility and level update, the occlusion-culling setup and the part list of drawn objects. Nothing of the network, login, launcher or anti-tamper code. **Second session, same build, 2026-10-05 (address ranges only):** **questions** (4) where the global limit *G* of §8.7 comes from; (5) which models use a combined mesh (`d3`) and which skip distance smoothing; (6) what turns on the last-level shadow caster of §8.8; (7) the constants of §8.6–§8.8 (smoothing factor, jump threshold, fade time, cross-fade band, preference multipliers, component defaults). **Why M1–M3 were not enough:** none of these values or conditions is stored in any record; the preference multipliers and the per-frame render state exist only in code; no frame capture or running client was available (M3), and a running client could show only that something fades or switches, not the condition or the constant. Code read: the per-frame render-state setup, the entity scene node's distance, visibility, fade and level routines, the model component's part scan, the mesh preparation step that derives runtime flags, the render component's constructor and property handler. **Third session, same build, 2026-10-07** (Ghidra, read-only copy of the project, by address): **questions** (8) which LOD class an entity without `Render_LODClass` gets, and the render component's initial values of the other properties §8.5–§8.8 use; (9) what resets the smoothed distance; (10) the use of `d4` … `d7` and of the `d5`/`d6` state, and which initial value belongs to `d6` and `d7`; (11) the imposter variant of the `d0` test; (12) what gates the combined mesh, including the `MeshCombining` preference; (13) how a cross-fade is composited; (14) where a forced level comes from; (15) the use of a kind-4 level; (16) the `build_byte` threshold for directory stamps 34–37 (§10.4); (17) how the client stores collision indices (§9.2); (18) whether the per-vertex scalar reaches the GPU (§5.3). **Why M1–M3 were not enough:** these are code paths and constants with no trace in the records (no LOD class is stored for 7,625 descriptors, no record has a kind-4 level, no mesh has a stamp from 34 to 37); no published source states them; no running client or frame capture was available, and a capture could show a fade or a pop but not its condition. Code read: the render component (constructor, property handler, property-change wrappers, LOD-class setter), the scene node's constructor, distance pass, visibility and fade, per-part level and shadow routines, the part draw routine, the model's combined-mesh and attachment routines, the mesh level classification, the collision-mesh and tree readers, the client's table of data-version steps, the GPU vertex-format builder, and the registration of the `MeshCombining` preference. Nothing of the network, login, launcher or anti-tamper code. **Fourth session, same build, 2026-10-07** (review changes; Ghidra, read-only copy of the project, by address): **questions** (19) into which slot of the render component the LOD-class table reader puts each float of a record entry, and which slots the attachment hysteresis reads; (20) what the player's ×3 does when the class has no table entry; (21) every factor of the opacity that the shadow test of §8.8 reads. **Why M1–M3 were not enough:** the record gives the eight floats but not which code slot each one fills; the ×3 path and the opacity product exist only in code; no running client or capture was available, and neither would show a slot assignment. Code read: the LOD-class table reader, the render component's constructor and LOD-class setter, the scene node's per-frame model update (attachment hysteresis, opacity product) and the per-object tint refresh. Nothing of the network, login, launcher or anti-tamper code. |
| WLK | `walker.md` §11.3 (BIN-W question (c), same build): the landscape extent *E* and the global limit *G*. Re-read independently in the second BIN session above; the two readings agree. |
| PDEF | Property dictionary of the same install, read 2026-10-05 with the repository's `tessera-props` as a tool: dictionary defaults of `Render_UseMeshCombining` (false), `Render_ImposterName` (0, `None`), `Render_LODCrossFade` (true), `Render_CastFakeShadows` (false), `Render_LODClass` (2). Census of these properties over the own property sets of **all 40,643** entity descriptors (0x47) from a dirty-room dump of 2026-10-05 whose `Render_LODClass` count (33,018) matches §8.5. |
| CNS | Census of 2026-10-05 by throwaway probes (repository decoders used as tools): all 47,563 mesh records (levels per kind, triangle counts per level, draw entries per kind-2 level, the one-triangle census), all 28,555 setups (stub parts), the surfaces and materials of every stub mesh, the LOD-class record 0x32000000, and the DID maps and enum mappers named in §8.5. |
| ENV, COL, SA, LGT | `environment.md` §9.1 (LOD-class record layout); `collision.md` §3, §5, §7, §15 (tree code, cell geometry, corrections to §10 here); `skeleton-anim.md` §4.2, §9.2 (influence list, part slots); `lighting.md` §4.3, §5.12–§5.13 (vertex inputs, alpha). Same install. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| M1 | Mesh records live in `client_mesh.dat` and `client_mesh_aux_1.datx`; the two DID sets are disjoint (§2) | DS; OBS | Confirmed |
| M2 | Header: DID, flags, surface-DID list, submesh count equal to the surface count (§3.1) | MV, KC (partly); OBS (47,563 of 47,563) | Confirmed. MV misread the submesh count and format words as a material list (§16). |
| M3 | Each submesh starts with a **vertex format word**; the stride and attribute layout follow from it deterministically (§4) | KC (UV count, influence count, tangent flag, 5 bytes per influence); D3D (bit layout); OBS (112,910 of 112,910 submeshes; bounding box after every vertex block matches bit-exactly) | Confirmed |
| M4 | Attribute order: position, normal, point-size scalar, colour, UV sets, tangent, binormal, bone indices, bone weights (§4.3) | D3D (FVF order), DS (skin-tail offsets); OBS | Confirmed |
| M5 | Normals are unit length; tangent is +U, binormal is +V (§5) | OBS | Confirmed |
| M6 | Bone indices are u8, one per influence, indexing the skeleton bone array directly; weights are f32 and sum to 1 (§5.7) | MV, DS; OBS (weights all submeshes; index meaning by bone names on 2 NPC setups) | Confirmed |
| M7 | Per-submesh AABB follows the vertex block and equals the min/max of the positions bit-exactly (§6.1) | MV; OBS (112,910 of 112,910) | Confirmed |
| M8 | The per-submesh u32 list after the AABB is redundant: for each vertex from the last to the first, the vertex's slot-0 bone index once per non-zero weight (§6.2) | OBS (43,059 of 43,059 skinned submeshes, exact; 0 entries on all 69,851 others); SA §4.2 | Confirmed (length and content, 2026-10-05) |
| M9 | Index buffers come after all vertex blocks, one per submesh; u16 submesh-local triangle lists (§7) | MV, KC; OBS (all) | Confirmed |
| M10 | Detail levels with draw lists, and per-level collision meshes, follow the index buffers (§8–§9) | OBS (47,563 of 47,563, zero trailing bytes) | Confirmed. No published source describes this part. |
| M11 | The collision tree block ends with one extra byte only in records whose directory `stamp` ≥ 38 among the observed stamps; the client's threshold is 35 (§10.4, row M41) | DC; OBS (71,167 blocks, perfect split by stamp) | Confirmed (observed split) |
| M12 | Z-up, right-handed, metres; characters face +Y; front faces counter-clockwise (§11) | MV, DS (Z-up); OBS (bone names, winding census, renders) | Confirmed |
| M13 | UV (0,0) is the first stored texel row/column; no V flip (§11.4) | MV; OBS (renders) | Confirmed |
| M14 | Surface list entries are 0x31 surface DIDs; draw entries select them by index (§12) | MV; OBS (112,910 references, all resolve) | Confirmed |
| M15 | `flags` is a value of the enum mapper labelled `MeshTypeID`: the part's body or equipment slot (§3.3) | SA §9.2 (47,563 of 47,563); BIN (the reader treats the word as a `MeshTypeID` value) | Confirmed |
| M16 | The vertex layout follows from the format word by the client's own routine; tangent and binormal are separate bits; three further bits are runtime layout options (§4.5) | BIN; OBS (§4.2) | Confirmed for the observed words; BIN for the bits that never occur |
| M17 | Kind-1 levels are the detail levels in stored order; kind-2 levels are occluders and never drawn; the client also recognises kind 4 (§8.3) | BIN; CNS | BIN |
| M18 | Detail-level choice: factor 1 − d/R, level ⌊(n − 1)(1 − f²)⌋, last level only at d ≥ R (§8.4) | BIN | BIN |
| M19 | R = the object's LOD-class detail range × the `ModelDetail` multiplier; 200 m × the multiplier for interior cell geometry; d is the distance to the object's bounding box (§8.5–§8.6) | BIN; ENV (record layout); CNS (table content) | BIN (rule); Confirmed (table, 2026-10-05) |
| M20 | Shadow passes may use the last detail level; neighbouring levels cross-fade for entities that ask for it (§8.8; details in M29–M31) | BIN | BIN |
| M21 | Each kind-2 level becomes one occluder of the occlusion-culling system, built from its single draw entry (§8.9) | BIN; CNS (4,173 of 4,173 kind-2 levels have one draw entry) | BIN |
| M22 | A mesh with exactly one triangle over all its detail levels is flagged and is not drawn as one of several parts of an object (§8.10) | BIN; CNS (258 records) | BIN (rule); Confirmed (census) |
| M23 | A mesh's bound is the union of the stored boxes of the vertex blocks drawn by detail level 0 (§6.1) | BIN; OBS | Confirmed |
| M24 | Distance smoothing for entity-drawn objects: *d_s* ← *d_s* + min(1, 3 / (*F* + 0.0002)) × (*d* − *d_s*); *d_s* = *d* in the first frame of the object's per-view state; no smoothing for models with a skinned part; the 25 m camera jump resets the fade, not the smoothing (§8.6) | BIN (questions 5, 7) | BIN. Corrects the previous §8.6 (§16). |
| M25 | Draw distance min(*G*, `d0` × *M_d*), tested against the unsmoothed distance; *M_d* = 0.6, 0.8, 1.0, 1.4, 2.0 for `ObjectDrawDistance` 0 … 4; fade in or out over 1.5 s; immediate after a camera jump of more than 25 m or a view reset; no distance culling while distance-based levels are off (§8.7) | BIN (questions 4, 7) | BIN |
| M26 | *G*: unbounded, else the landscape extent when the view has a landscape, clipped to the fog far distance under a condition, at least 60 m (§8.7) | BIN (question 4); WLK | BIN |
| M27 | Shadow distance min(*G*, `d2` × *M_d* × *M_s*), *M_s* = 0.6, 0.8, 1.0, 1.4, 2.0 for `StencilShadows` 1 … 5, tested against the unsmoothed distance; no limit while distance-based levels are off (§8.7) | BIN (question 7) | BIN |
| M28 | Combined mesh: only models with a skinned part and entities whose `Render_UseMeshCombining` is true (54 descriptors); used beyond `d3` × min(1, *M_c*); `d5`/`d7` are a hysteresis pair for the same models (§8.7; corrected 2026-10-07, row M35) | BIN (question 5); PDEF (default, census) | BIN (rule); Confirmed (default and census, 2026-10-05) |
| M29 | Last-level shadow caster: a switch of the frame's render state (off in the render state's defaults) and the entity's shadow flag (§8.8) | BIN (question 6); PDEF (`Render_CastFakeShadows` default false; set by 1,906 descriptors, true in 1,786) | BIN (rule); Confirmed (census) |
| M30 | Cross-fade: band 0.2 × the factor gap; requires `Render_LODCrossFade`, the main view, multi-pass lighting and a model **without** a skinned part (§8.8) | BIN (question 7); PDEF (default true; set by 1,365 descriptors, false in 1,350) | BIN. Corrects the previous §8.8 (§16). |
| M31 | Forced level: a per-entity index in the render component, default "none", clamped to *n* − 1 (§8.8) | BIN (question 7) | BIN; source OPEN |
| M32 | Vertex colour is read by 116 shader records only and multiplies the material colour; UV set 0 is the common texture coordinate, sets 1 and 2 serve baked lighting and effects (§5.4, §5.5) | LGT (`lighting.md` §4.3, §5.13, its BIN of the shader programs) | BIN (in `lighting.md`); not re-checked here |
| M33 | An entity none of whose property layers holds `Render_LODClass` keeps the render component's initial values (`d0` … `d7` = 200, 60, 16, 80, 1000, 1000, −1, 1100; order corrected 2026-10-07, row M35); the dictionary default (class 2) reaches the component only when a held value is removed at run time (§8.5) | BIN (question 8); WLK (`walker.md` W20, an independent reading of the same build) | BIN. Closes the §8.5 OPEN. |
| M34 | The render component's own initial values of `Render_LODCrossFade` (true), `Render_CastFakeShadows` (false) and the imposter flag (off) equal the dictionary defaults' effect (§8.5, §8.7, §8.8) | BIN (question 8); PDEF | BIN |
| M35 | In record terms (`d0` … `d7` = the entry's floats in stream order): `d6` starts at −1 and `d7` at 1,100. The client's table reader stores the 7th float (`d6`) in the slot the component initialises to −1 and the 8th float (`d7`) in the slot initialised to 1,100; the copy into the component is slot for slot. The `d5`/`d7` state switches a model's per-model attachments (records of DB type 0x48) on and off; no reader of `d4` or `d6` was found (§8.7). **Corrected 2026-10-07:** the revision of the same day named `d6` and `d7` the wrong way round. The data agree with the corrected naming: class 0x10000014 has `d5` 12, `d6` −1, `d7` 20 (on below 12 m, off above 20 m) | BIN (question 10; re-read for question 19); OBS (0x32000000: all 32 entries, 1,160 bytes; `d6` −1 in 31 entries and 50 in one, `d7` 1,100 in 31 and 20 in one, 2026-10-07) | BIN (slots); Confirmed (values) |
| M36 | The smoothed distance restarts only when the object's scene node is created (the object is added to the scene); the scene's distance pass reruns when the camera has moved 1.5 m, after 1 s, or when the view missed a frame (§8.6) | BIN (question 9) | BIN |
| M37 | Entities with an imposter name fade out over the 15 m before *D*, measured at the box centre, and their draw test limit becomes *D* + (*d_c* − *d*) (§8.7) | BIN (question 11); PDEF (1,072 descriptors set `Render_ImposterName`) | BIN |
| M38 | The combined mesh is built only while the `MeshCombining` preference is on, the entity's `Render_UseMeshCombining` is true, the model has a skinned part and its distance asked for it; otherwise it is released (§8.7) | BIN (question 12) | BIN |
| M39 | A cross-fading object is drawn through the translucent path; each part draws its level and then the neighbouring level again with alpha = object opacity × the part's weight (§8.8) | BIN (question 13) | BIN (the second of two composite modes not traced) |
| M40 | An object casts shadows only while its opacity is above 0.8 and its distance is strictly below the shadow distance (§8.7, §8.8) | BIN (question 13) | BIN |
| M41 | `build_byte` is present exactly when the record's data version gives the Havok-data tag a version of at least 1, which is directory `stamp` ≥ 35 (§10.4) | BIN (question 16; the stamp-to-step link inferred, §10.4); OBS (all 71,167 tree blocks agree) | BIN (threshold, partly inferred); Confirmed (observed stamps) |
| M42 | The client reads collision-mesh triangle and edge indices as 16-bit values with no high part (§9.2) | BIN (question 17) | BIN |
| M43 | The client keeps the per-vertex scalar in the vertex stream it uploads to the GPU (§5.3) | BIN (question 18) | BIN |
| M44 | No store into the forced-level field after construction was found in the code of the render component, the model and the scene, and no read of the kind-4 position besides the level classification in the mesh code (searched by field offset) (§8.3, §8.8) | BIN (questions 14, 15; negative searches) | BIN (negative result) |
| M45 | The ×3 for the player's own character (§8.5 step 4) runs whenever a class value is applied and the LOD-class record exists, also when the class has no entry; then it multiplies the values the component already holds, so each further application (a new value or a forced re-application of the same one) multiplies them again. Every `Render_LODClass` value held by an entity descriptor's own property set in this install has an entry (25 distinct values, 33,018 descriptors) | BIN (question 20); OBS (census of all 40,643 descriptors, 2026-10-07) | BIN (rule); Confirmed (census) |
| M46 | The opacity that the shadow test reads (§8.8) has a fourth factor: the alpha of a per-object tint that the scene node refreshes at most every 20 s; it starts at 1 and every path traced sets it to 1 | BIN (question 21) | BIN |

---

<a id="s2"></a>
## 2. Where mesh records live (Confirmed)

| Archive | Records | Smallest | Largest |
|---|---|---|---|
| `client_mesh.dat` | 37,484 | 278 B | 17,984,776 B |
| `client_mesh_aux_1.datx` | 10,079 | 350 B | 13,036,865 B |

- No 0x06 DID occurs in both archives. Routing (`dat-container.md`) tries `client_mesh.dat`
  first; a reader must fall back to the aux archive.
- Every 0x06 record decodes as described here. No other prefix occurs in these archives
  apart from the 0xFF metadata records.
- Every record begins with its own DID (`records.md` §1.6). Sizes are of the inflated
  payload.

---

<a id="s3"></a>
## 3. Record overview

All integers are little-endian, all floats IEEE-754 binary32 (`records.md` §1). Every field
is packed: **nothing is aligned**. Vertex blocks have odd strides (61, 65, 71 …), so every
later field may start at any byte offset.

A record is five consecutive regions, in this order:

| Region | Content | Section |
|---|---|---|
| A | Header and surface list | §3.1 |
| B | *N* submesh blocks (vertex format word, vertices, bounds, influence list) | §3.2, §4–§6 |
| C | *N* index buffers | §7 |
| D | Detail levels: draw lists, each followed by optional collision data | §8–§10 |
| (end) | Nothing follows. 47,563 of 47,563 records end exactly after region D. | |

<a id="s3-1"></a>
### 3.1 Region A: header (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `did` | DID | Own DID (0x06…) |
| 2 | `flags` | u32 | §3.3 |
| 3 | `surface_count` *S* | u32 | Number of surface DIDs |
| 4 | `surfaces` | DID × *S* | 0x31 surface DIDs (§12) |
| 5 | `submesh_count` *N* | u32 | Number of submesh blocks. *N* = *S* in every record. |

<a id="s3-2"></a>
### 3.2 Region B: submesh block (Confirmed)

Repeated *N* times, in submesh order 0 … *N*−1:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `vfmt` | u32 | Vertex format word (§4) |
| 2 | `vertex_count` *V* | u32 | Number of vertices. Observed 3 … 57,603; never 0. |
| 3 | `vertices` | *V* × stride bytes | Interleaved vertices, layout per §4 |
| 4 | `aabb` | f32 × 6 | min x, min y, min z, max x, max y, max z (§6.1) |
| 5 | `influence_count` *I* | u32 | §6.2 |
| 6 | `influences` | u32 × *I* | §6.2 |

<a id="s3-3"></a>
### 3.3 The `flags` word: the part's slot (Confirmed)

`flags` is a key of the enum mapper labelled **`MeshTypeID`** in the EMAPPER DID map
(`property-system.md` §8; 0x23000089 here, base mapper 0x23000170 with 0 `Invalid`,
1 `Default`, 2 `All`). It names the body or equipment slot that the mesh fills when it is a
part of a figure (`setup.md` §12.2, `appearance.md` §4.4). The client's own mesh reader treats
the word as a value of that mapper (BIN), and every record of the install carries a key of it
(SA §9.2):

| Value | Records | Label (from the mapper) |
|---|---|---|
| 0x00000001 | 27,960 | `Default` (static meshes) |
| 0x00000002 | 20 | `All` |
| 0x1000_00*nn*, *nn* = 0x01 … 0x43 | 19,583 | body, equipment, mount, war-machine and fence slots (`Helmet` 4,014, `UpperBody` 2,960, `Shoulder` 2,603, `Crest` 1,503, …; `skeleton-anim.md` §9.2) |

Bit 28 (0x10000000) is set exactly on the mapper's own keys; it does **not** decide the
vertex layout. 1,317 records with a slot value have no submesh with bone influences, and 783
records with `Default` have submeshes with influences. A decoder takes the layout of each
submesh from its own `vfmt` only, and a renderer treats a submesh as skinned exactly when its
`vfmt` influence count (§4.1) is non-zero. Readers keep the value as a u32 and must not reject
values outside the mapper (the client's reader consults the mapper only for the text form of
a record; the binary form is the plain u32).

---

<a id="s4"></a>
## 4. Vertex format word and the stride rule

<a id="s4-1"></a>
### 4.1 Bit fields (Confirmed)

The `vfmt` word is laid out like a Direct3D flexible vertex format (D3D) in its low 12 bits,
with two extra bytes on top:

| Bits | Mask | Name | Meaning | Bytes per vertex |
|---|---|---|---|---|
| 1 | 0x00000002 | POSITION | Position, 3 × f32. Set in every observed word. | 12 |
| 4 | 0x00000010 | NORMAL | Normal, 3 × f32. Set in every observed word. | 12 |
| 5 | 0x00000020 | SCALAR | One f32 per vertex (D3D's "point size" slot) | 4 |
| 6 | 0x00000040 | COLOUR | One 4-byte colour (§5.4) | 4 |
| 7 | 0x00000080 | COLOUR2 | Second 4-byte colour (D3D "specular"). **Never observed.** | 4 (Reported) |
| 8–11 | 0x00000F00 | UV_COUNT *T* | Number of UV sets, each 2 × f32. Observed 1, 2, 3, 7. | 8·*T* |
| 16–23 | 0x00FF0000 | INFLUENCES *K* | Bone influences per vertex. Observed 0 … 4. | 5·*K* |
| 24–31 | 0xFF000000 | TANGENTS | 0x30 = tangent and binormal present; 0x00 = absent. No other value observed. Bit 28 is the tangent and bit 29 the binormal (§4.5). | 24 if 0x30 |

Bits 0, 2, 3, 12–15 have never been observed set. §4.5 gives what the client does with the bits
that never occur.

<a id="s4-2"></a>
### 4.2 Stride rule (Confirmed, 112,910 of 112,910 submeshes)

stride = 12 (position)
       + 12 if NORMAL
       + 4 if SCALAR
       + 4 if COLOUR
       + 4 if COLOUR2
       + 8 × *T*
       + 24 if TANGENTS = 0x30
       + 5 × *K*

Proof obligations, all met on every submesh of the install:

1. Reading *V* vertices at this stride lands exactly on the AABB, and the AABB equals the
   per-axis minimum and maximum of the decoded positions **bit for bit** (§6.1).
2. Continuing with this stride for every submesh, then decoding regions C and D, consumes the
   record to its last byte.
3. Normals at the derived offset are unit length (§5.2), and bone weights at the derived
   offset sum to 1 (§5.7).

A brute-force search for any other stride that also lands on a matching AABB found an
alternative only for a single degenerate 3-vertex stub whose three positions coincide, where
every stride "matches". The rule is therefore the only workable reading. No unit-normal
scanning or other heuristic is needed or allowed.

**Unsupported words.** A decoder must refuse (as "unsupported vertex format", not as
corrupt data) a word that has POSITION clear, any of bits 0, 2, 3, 12–15 set, COLOUR2 set,
or a TANGENTS byte other than 0x00 and 0x30. None occur in this install.

<a id="s4-3"></a>
### 4.3 Attribute order (Confirmed)

Within one vertex, present attributes appear in this order, packed with no padding:

| # | Attribute | Present when | Size |
|---|---|---|---|
| 1 | position | always | 12 |
| 2 | normal | NORMAL | 12 |
| 3 | scalar | SCALAR | 4 |
| 4 | colour | COLOUR | 4 |
| 5 | (colour 2) | COLOUR2 (never observed; position Reported from D3D order) | 4 |
| 6 | UV set 0 … *T*−1 | *T* ≥ 1 | 8 each |
| 7 | tangent | TANGENTS = 0x30 | 12 |
| 8 | binormal | TANGENTS = 0x30 | 12 |
| 9 | bone indices | *K* ≥ 1 | *K* × u8 |
| 10 | bone weights | *K* ≥ 1 | *K* × f32 |

Consequences, against older notes:

- The first UV set is at offset 24 **only** when neither SCALAR nor COLOUR is set. With
  COLOUR it is at 28, with SCALAR and COLOUR at 32.
- Bone indices are grouped (all *K* index bytes, then all *K* weights), not interleaved.
  The skin-tail table in DS (strides 61 … 84) agrees with this rule row for row.

<a id="s4-4"></a>
### 4.4 Census of every format word (Confirmed)

| `vfmt` | Stride | Submeshes | | `vfmt` | Stride | Submeshes |
|---|---|---|---|---|---|---|
| 0x00000112 | 32 | 4,075 | | 0x30020112 | 66 | 6,241 |
| 0x00000152 | 36 | 99 | | 0x30020152 | 70 | 275 |
| 0x30000112 | 56 | 35,061 | | 0x30020212 | 74 | 20 |
| 0x30000152 | 60 | 30,497 | | 0x30020252 | 78 | 25 |
| 0x30000212 | 64 | 9 | | 0x30030112 | 71 | 14,079 |
| 0x30000252 | 68 | 103 | | 0x30030152 | 75 | 490 |
| 0x30000372 | 80 | 7 | | 0x30030212 | 79 | 44 |
| 0x30010112 | 61 | 11,402 | | 0x30030252 | 83 | 96 |
| 0x30010152 | 65 | 697 | | 0x30030352 | 91 | 1 |
| 0x30010212 | 69 | 3 | | 0x30030712 | 119 | 1 |
| 0x30010252 | 73 | 39 | | 0x30040112 | 76 | 9,238 |
| 0x30010352 | 81 | 2 | | 0x30040152 | 80 | 312 |
| 0x30010712 | 109 | 1 | | 0x30040212 | 84 | 17 |
| | | | | 0x30040252 | 88 | 76 |

27 distinct words, 112,910 submeshes. The two words without tangents (0x112, 0x152) are used
almost only by proxy levels (§8.3): 4,173 of their 4,174 submeshes.

Worked example (synthetic): `vfmt` = 0x30020152 means POSITION, NORMAL, COLOUR, one UV set,
two influences, tangent frame. Stride = 12 + 12 + 4 + 8 + 24 + 10 = 70. Offsets: position 0,
normal 12, colour 24, UV 28, tangent 36, binormal 48, indices 60–61, weights 62–69.

<a id="s4-5"></a>
### 4.5 The client's layout routine (BIN)

The client's mesh reader reads `vfmt` and the vertex count, derives the layout from `vfmt`
with one routine, and then reads vertex count × stride bytes. For every word of §4.4 the
routine gives exactly the stride and offsets of §4.2–§4.3. It also defines the bits that
never occur in this install:

| Bits | Client's reading | Bytes per vertex |
|---|---|---|
| 1–3 (0x0000000E) | Position present when any of them is set | 12 |
| 30 (0x40000000) | Packed position, used only when bits 1–3 are all clear | 4 |
| 4, 5, 6, 7 | Normal, scalar, colour, colour 2, in this order after the position | 12, 4, 4, 4 |
| 8–11 | UV count: values 1 … 8 give that many sets; 0 and 9 … 15 give **none** | 8 per set |
| 28 (0x10000000) | Tangent, after the UV sets | 12 |
| 29 (0x20000000) | Binormal, after the tangent | 12 |
| 16–23 | Influence count *K* (no upper limit in the routine) | indices then weights, below |
| 15 (0x00008000) | Packed skin: 4 bytes of indices and 4 one-byte weights, whatever *K* is | 8 instead of 5*K* |
| 24 (0x01000000) | Padded stride: the smallest of 32, 64, 96, 128, 160, 192, 224 that is **greater** than the unpadded stride (so 32 becomes 64); an unpadded stride of 224 or more becomes the larger of itself and 256 | padding |

The renderer also builds vertex buffers for the graphics card from the stored word, keeping
the position, normal, scalar and colour bits, the UV count, and bits 28–30 (on capable
hardware), and setting bit 15 for skinned vertices when skinning runs on the graphics card.
Bits 15, 24 and 30 are therefore runtime layout options; a stored word never carries them.

Decoders keep refusing the words that §4.2 lists as unsupported (none occur). An
implementation that wants to accept COLOUR2 alone, or a lone tangent (bit 28) or binormal
(bit 29), may use the sizes above; they are the client's.

---

<a id="s5"></a>
## 5. Vertex attribute encodings

<a id="s5-1"></a>
### 5.1 Position (Confirmed)

3 × f32, object space, metres (§11). All 89,593,149 positions are finite.

<a id="s5-2"></a>
### 5.2 Normal (Confirmed)

3 × f32. Every normal of every vertex has length 1 within 0.001 (89,593,149 of 89,593,149),
including the tiny 3-vertex stub meshes. A reader may still renormalise.

<a id="s5-3"></a>
### 5.3 Scalar (Confirmed presence, meaning OPEN)

One f32. Present in 7 submeshes of 7 records (all trees, `vfmt` 0x30000372). Values range
from 3.77 to 25.75 and vary per vertex.

The client keeps the scalar in the vertex stream it uploads to the GPU (BIN, row M43): the
GPU vertex format it derives from `vfmt` keeps the POSITION, NORMAL, SCALAR, COLOUR and
COLOUR2 bits and the UV count unchanged (the tangent frame and the skin data only on capable
renderer paths).

> **OPEN:** Meaning of the scalar (a wind-sway weight or height above ground would fit).
> The client's layout routine places it after the normal (§4.5) and uploads it (above), but
> which shader input receives it was not traced; the shader programs read two vertex inputs
> labelled as texture coordinates 3 and 4 whose meaning is also open (`lighting.md` §4.3).
> Renderers ignore it until resolved.

<a id="s5-4"></a>
### 5.4 Colour (Confirmed layout, channel order Reported)

4 bytes. Byte 3 is alpha (Confirmed: across the 32,719 submeshes with COLOUR, 27,776 have
alpha 255 on every vertex). Bytes 0–2 are blue, green, red, as in a little-endian
`D3DCOLOR` (D3D, Reported). 7,968 coloured submeshes have one colour on every vertex.

Use (provenance row M32: `lighting.md` §4.3 and §5.13, BIN of the shader programs there; not
re-checked in this spec): only 116 shader
records (2,624 surface records) read vertex colour. There it multiplies the material colour:
RGB scales the lit colour and alpha scales the output alpha, after the alpha test. Every other
surface ignores the vertex colour, even when the mesh stores one.

> **OPEN** (`lighting.md` §15): whether the colour bytes are sRGB-encoded or linear.

<a id="s5-5"></a>
### 5.5 UV sets (Confirmed)

Each set is 2 × f32 (u, v). Set 0 is the texture coordinate for the diffuse map.

- Values outside [0, 1] are normal and mean wrap (repeat) addressing: 104,300 of 112,910
  submeshes keep set 0 within [−16, 16], but 338 submeshes exceed ±100 (up to ±8 × 10⁷).
  Readers must not reject or clamp them.
- Origin and direction: §11.4.
- Sets 1 … 6 exist in 444 submeshes. In the samples inspected, set 1 was constant (0, 1).

Use (provenance row M32: `lighting.md` §4.3, BIN there): the common shader programs read UV
set 0 only; sets 1 and 2 are
read by records with baked-lighting roles (`BakedShadow`, `BakedLight`,
`DirLightMaskAndAmbOcc`) and by some effect records.

> **OPEN** (`lighting.md` §4.3): which set a baked-lighting record reads for which slot, and
> sets 3 … 6.

<a id="s5-6"></a>
### 5.6 Tangent frame (Confirmed)

Tangent then binormal, 3 × f32 each.

- **Tangent is the +U direction, binormal the +V direction.** Over sampled submeshes the
  mean cosine between the stored tangent and the per-triangle ∂P/∂u was 0.90 – 0.99, and
  between the stored binormal and ∂P/∂v 0.87 – 0.96; the cross terms were about 0.
- Both are unit length on 99.88 % of the 89,481,871 vertices that carry them; the rest are
  zero or short (degenerate UV mapping). Renderers must handle zero vectors.
- The frame is not orthogonalised against the normal, and its handedness varies within a
  mesh (the sign of (tangent × binormal) · normal is positive on some vertices and negative
  on others, because UVs are mirrored). A renderer must not assume one bitangent sign; it can
  use the stored binormal directly or derive a per-vertex sign from it.

<a id="s5-7"></a>
### 5.7 Bone indices and weights (Confirmed)

*K* bytes of bone index, then *K* f32 weights (*K* = influence count, 1 … 4).

| Property | Observation (all 43,059 submeshes with *K* ≥ 1) |
|---|---|
| Index meaning | Index into the bone array of the skeleton used with the mesh. Confirmed on two NPC setups whose skeletons carry bone names: vertices dominated by bones named `l…` lie at −X and `r…` at +X, head-bone vertices at head height. Skeleton decoding belongs to the future animation spec. |
| Index range | 0 … 247 (u8). |
| Weight sum | 1 within 2 × 10⁻⁴ on every vertex. |
| Negative weights | none |
| First weight | always > 0 |
| Unused slots | weight 0 and index 0 (no counter-example) |
| Order | not sorted: 29,894 submeshes have some vertex whose weights are not descending |

A vertex therefore needs no renormalisation. A GPU path that wants 4 influences pads with
index 0, weight 0.

---

<a id="s6"></a>
## 6. Per-submesh trailer

<a id="s6-1"></a>
### 6.1 Bounding box (Confirmed)

Six f32: minimum x, y, z, then maximum x, y, z of the submesh positions. In 112,910 of
112,910 submeshes these equal the computed minimum and maximum **bit for bit**. A decoder
may verify this as a structural check (it costs one pass over the positions) and should
report a mismatch as malformed data.

There is no bounding sphere and no whole-mesh bound in the record. A mesh's bound is the
union of the stored AABBs of the vertex blocks drawn by its detail level 0 (§8; BIN: the
client computes it this way when it loads a mesh, also for the one-triangle meshes of §8.10).
A mesh without detail levels has no bound of its own. (Setups carry their own physics
bounds, `setup.md`.)

<a id="s6-2"></a>
### 6.2 Influence list (Confirmed: redundant, 43,059 of 43,059)

`influence_count` *I* and *I* u32 values.

- *I* = 0 for every submesh with *K* = 0 (69,851 of 69,851).
- For every submesh with *K* ≥ 1 the list is fully determined by the vertex data
  (2026-10-05, all 43,059 skinned submeshes, exact): walk the vertices from the **last to the
  first** (*V*−1 down to 0); for each vertex, write its **first** bone index (slot 0, §5.7)
  once for every non-zero weight of that vertex. So *I* equals the number of non-zero weights
  of the submesh, and every value is a bone index the submesh uses.
- It is **not** a palette (it has one entry per influence, not per distinct bone) and **not**
  a submesh-local → skeleton remap: vertex bone indices already address the skeleton's bone
  array directly (§5.7; `skeleton-anim.md` §4.1).

Rendering ignores the list; skinning uses the per-vertex data of §5.7 and the skeleton's
palette (`skeleton-anim.md` §4.3). Decoders keep it as a u32 array for lossless re-encoding
and may recompute it from the vertices to validate a record (a mismatch is a data-quality
issue, never observed).

Synthetic example: three vertices with two influence slots, bones (4, 7) weights
(0.75, 0.25); bones (2, 0) weights (1, 0); bones (9, 4) weights (0.5, 0.5). The list is
(9, 9, 2, 4, 4).

---

<a id="s7"></a>
## 7. Region C: index buffers (Confirmed)

Directly after the last submesh block:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `index_buffer_count` | u32 | Equals *N* in every record |
| 2 | buffers | × `index_buffer_count` | Each: u32 `index_count`, then `index_count` × u16 |

Index buffer *i* belongs to submesh *i* in practice (every draw entry of §8 pairs vertex
block *i* with index buffer *i*). Observed on all 112,910 buffers:

- **Triangle list.** `index_count` is a non-zero multiple of 3; triangle *t* is indices
  3*t*, 3*t*+1, 3*t*+2. (The draw entry's primitive byte is 4, the D3D value for a triangle
  list; no other value occurs.)
- **Submesh-local.** Indices count from 0 within the submesh's own vertex block.
- **Tight.** The smallest index is 0, the largest is *V*−1, every vertex is referenced, and
  no triangle repeats a vertex (no degenerate triangles).
- Largest buffer: 193,119 indices. Since *V* ≤ 57,603 < 65,536, u16 always suffices.

To build one flat buffer for a whole level, add the running vertex offset of each submesh to
its indices.

---

<a id="s8"></a>
## 8. Region D: detail levels and draw lists

<a id="s8-1"></a>
### 8.1 Layout (Confirmed, 47,563 records, zero trailing bytes)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `level_count` *L* | u32 | ≥ 1. Distribution: 1 (33,443 records), 2 (4,209), 3 (6,498), 4 (2,968), 5–17 (445). |
| 2 | levels | × *L* | Level record, below |

Level record:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `kind` | u32 | 1 = detail level, 2 = proxy (occluder) level (§8.3). No other value occurs; the client also recognises 4 (§8.3). |
| 2 | `draw_count` *D* | u32 | ≥ 1 |
| 3 | draws | × *D* | Draw entry, 13 bytes each, below |
| 4 | `has_collision_mesh` | u8 | 0 or 1 (§9) |
| 5 | collision mesh | | Present only if `has_collision_mesh` = 1 (§9.1) |
| 6 | `has_tree` | u8 | 0 or 1 (§10) |
| 7 | tree block | | Present only if `has_tree` = 1 (§10.1) |

Draw entry:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `primitive` | u8 | Always 4 (triangle list) |
| 2 | `vertex_block` | u32 | Submesh index for vertices |
| 3 | `index_buffer` | u32 | Index-buffer index |
| 4 | `surface` | u32 | Index into the header surface list (§12) |

In all 112,910 draw entries the three indices are equal. Decoders must still honour each
field separately and bounds-check it against *N*.

<a id="s8-2"></a>
### 8.2 Draw-list rules (Confirmed)

- Across all levels of a record, every submesh index 0 … *N*−1 appears in exactly one draw
  entry (112,910 entries for 112,910 submeshes). Levels therefore partition the submeshes.
- In 47,377 records the draw entries, read level by level, list submeshes in storage order.
  In the other 186, the order inside a level differs (for example level 0 draws 1 then 0).
  **The draw order is the list order**, which matters for alpha-blended surfaces.
- The total number of triangles in a level is the sum over its draws.

<a id="s8-3"></a>
### 8.3 Level kinds (shapes Confirmed; use BIN)

When it loads a mesh the client sorts the levels by `kind` into two lists, each in stored
order (BIN):

- **Kind 1, detail levels.** Position *k* in the list of kind-1 levels is **detail level
  *k***; detail level 0 is the full-detail geometry. The choice among them is §8.4–§8.8.
  Every kind-1 level carries a collision mesh (71,765 of 71,765).
- **Kind 2, proxy (occluder) levels.** They are never drawn and never take part in the
  level choice; they become occluders of the occlusion-culling system (§8.9).
- **Kind 4.** The client also remembers the position of one level of kind 4 (the last one
  wins). No record of this install has one, and its use was not traced. All other `kind`
  values are ignored by the client.

> **OPEN:** the use of a kind-4 level. Narrowed on 2026-10-07 (row M44): besides the level
> classification, no code of the mesh module reads the remembered kind-4 position; a user
> elsewhere (for example an effect or a picking path) was not searched. **Tessera default:**
> decode it, never draw it.

Census of the detail levels per record (CNS): 1 (34,776 records), 2 (3,849), 3 (6,463),
4 (2,474), 6 (1). Triangle counts never increase from one detail level to the next in 10,750
of the 12,787 records with two or more. Of the other 2,037: in 1,976 only the last step rises,
and the last level then has exactly the triangle count of detail level 1 (for example 772,
324, 128, 324 triangles); in 35 only the last step rises, to another size; 22 rise elsewhere;
4 have two levels with more triangles in the second. §8.4 and §8.8 say when the last level is
drawn.

**Kind-2 shapes.** 4,173 levels in 2,644 records (1 per record in 1,902, up to 16). They
always come after all kind-1 levels and have exactly one draw entry each (4,173 of 4,173).
Their submeshes use the formats without tangents (0x112 or 0x152), mostly have 10–19
triangles (3,455 of 4,173 levels), and form simple boxes around the detail geometry
(Confirmed visually on two wall pieces). They never carry collision data. Their surfaces (106
distinct DIDs, most often 0x310003B4) also occur on ordinary submeshes; since kind-2 levels
are not drawn, the surfaces do not matter.

<a id="s8-4"></a>
### 8.4 Choosing a detail level (BIN)

Inputs: *n*, the number of detail levels of the mesh (≥ 1); *R*, the object's detail range in
metres (§8.5); *d*, the object's distance from the camera in metres (§8.6).

1. If distance-based levels are disabled, use *f* = 1. Distance-based levels are **enabled by
   default**: the client disables them only when an internal setting that allows them (true
   by default; not a key of the preference file, `install-layout.md` §8.3) is turned off, or
   for special views that ask for full detail. While they are disabled, objects keep full
   detail, are never culled or faded by distance (§8.7) and cast shadows at any distance.
2. Otherwise *f* = 1 − *d* / *R*; when *f* < 0, use *f* = 0.
3. *x* = 1 − *f*².
4. If *x* ≥ 1 (that is, *f* = 0), the level is *k* = *n* − 1. Otherwise *k* = ⌊(*n* − 1) ×
   *x*⌋, the integer part (rounding toward zero).
5. Draw detail level *k* (§8.3).

Consequences:

- One detail level: always level 0.
- Detail level *k* with 1 ≤ *k* ≤ *n* − 2 starts at the distance *d_k* = *R* × (1 −
  √(1 − *k* / (*n* − 1))).
- The **last** detail level (*k* = *n* − 1) is drawn only from *d* ≥ *R* on, where *f* = 0.

| *n* | Level 1 from | Level 2 from | Level 3 from | Level 4 from | Level 5 from |
|---|---|---|---|---|---|
| 2 | *R* | | | | |
| 3 | 0.2929 *R* | *R* | | | |
| 4 | 0.1835 *R* | 0.4226 *R* | *R* | | |
| 6 | 0.1056 *R* | 0.2254 *R* | 0.3675 *R* | 0.5528 *R* | *R* |

Worked example (synthetic): *n* = 3, *R* = 60 m. At *d* = 10 m, *f* = 0.8333, *x* = 0.3056,
2*x* = 0.611, level 0. At *d* = 20 m, *f* = 0.6667, *x* = 0.5556, 2*x* = 1.111, level 1. At
*d* = 59 m, *f* = 0.0167, *x* = 0.9997, level 1. At *d* = 60 m or more, level 2.

The client computes the inverse as well, for cross-fading (§8.8): the factor at which detail
level *k* begins is 1 for *k* = 0, √(1 − *k* / (*n* − 1)) for 0 < *k* < *n* − 1, and 0 for
the last level.

<a id="s8-5"></a>
### 8.5 The detail range *R* (BIN; table content Confirmed)

**Objects drawn through an entity** (everything that resolves through a 0x47 entity
descriptor: placed objects, NPCs, creatures, avatars, held items; `setup.md` §3): every part
of the object uses the object's range.

1. Read the entity's property `Render_LODClass` (an enum; mapper labelled `RenderLODClass` in
   the second group of the EMAPPER DID map, 0x2300008D here). 33,018 of the 40,643 entity
   descriptors set it in their own property set (`setup.md` §4.4).
2. Find the LOD-class record through the root DID map's `UNIQUEDB` sub-map
   (`property-system.md` §8.1): the entry with key 0x3F, labelled `RenderLODClasses` (the
   client looks it up by the key; the label identifies it for readers). Its value is a 0x32
   record (0x32000000 here; layout `environment.md` §9.1). Look the class up in its entries.
3. Of the entry's eight floats `d0` … `d7`, `d1` is the **detail range**. The other fields of
   known use are in §8.7. The client copies the entry when the property value reaches the
   object's render component; until then, and when the class has no entry, the component
   keeps its own values: `d0` 200, `d1` 60, `d2` 16, `d3` 80 metres (all eight in §8.7).
   **A value reaches the component only from a property layer that holds it** (row M33): an
   entity none of whose property layers holds `Render_LODClass` (its own descriptor, its
   parent descriptors, its placement's property sets; order in `walker.md` §11.4.3) keeps
   the component's initial values for its whole life. The property dictionary's default
   (class 2) is handed to the component only when a value that a layer held is removed at run
   time. A class value without an entry in the table leaves the values unchanged, except for
   the multiplication of step 4.
4. For one entity, the one the client holds as its current player object (the player's own
   character, by inference from how the reference is used), the client multiplies `d0` …
   `d5` by 3 each time a class value is applied and the LOD-class record of step 2 exists
   (row M45). When the class has an entry, the entry is copied first and then multiplied, so
   the result is always 3 × the entry. When the class has **no** entry, the multiplication is
   still applied, to the values the component already holds; each further application (a
   different value without an entry, or the client re-applying the current value, which it
   can force; the same value arriving again without that force changes nothing) multiplies
   them by 3 again. Every class value that the entity descriptors of
   this install hold has an entry (row M45), so the second case arises only for a value from
   elsewhere.
5. *R* = `d1` × *M*, where *M* is the multiplier of the user's `ModelDetail` preference
   (`install-layout.md` §8.3) times a factor of the frame's render state that is 1 in the
   render state's defaults (Tessera: 1 for the main view):

| `ModelDetail` | Low | Medium | High | VeryHigh | UltraHigh |
|---|---|---|---|---|---|
| Stored value | 0 | 1 | 2 | 3 | 4 |
| *M* | 0.4 | 0.7 | 1.0 | 1.4 | 2.0 |

A second preference, `DialBackGraphics` (boolean), shifts the stored value down by two before
this table is applied; Low and Medium then fall outside it and the client uses 2.0. Tessera
may ignore that switch.

The table of this install has 32 classes (keys 1, 2, 3 from the base mapper and 0x10000001 …
0x1000001D); `d1` ranges from 2 to 200 m (CNS). Like every data value it is read at run time.

> **Resolved (2026-10-07, BIN, row M33; closes the OPEN of 2026-10-05 and converges with
> `walker.md` §11.4 step 4 and W20):** an entity without `Render_LODClass` in any of its
> property layers is drawn with the component's initial values (`d0` 200 m, `d1` 60 m, `d2`
> 16 m, `d3` 80 m, …), not with class 2. Step 4's ×3 for the player's own character runs only
> when a class value arrives, so it is not applied to these initial values. The same
> mechanism decides the other properties of §8.7–§8.8, and for them the component's own
> initial values agree with the dictionary defaults (row M34): `Render_LODCrossFade` starts
> true, `Render_CastFakeShadows` false, and the imposter flag of §8.7 off. The earlier
> Tessera decision (class 2) is withdrawn; `tessera-world` already follows the rule above
> (implementer note of 2026-10-06).

**Interior cell geometry** (the render meshes inside the cell records 0x01, `collision.md`
§5.7, drawn by the client's environment-cell objects; the identification rests on the code's
context): *R* = 200 m × *M*, with *d* unsmoothed (§8.6). These meshes have no LOD class.

<a id="s8-6"></a>
### 8.6 The distance *d* (BIN)

*d* is the distance from the camera position to the **nearest point of the object's
world-space axis-aligned bounding box** (0 when the camera is inside the box). The client
computes it per view; it also keeps, per view, the distance *d_c* from the camera to the
**centre** of that box (used only by the imposter variant of §8.7) and a third distance that
none of these rules uses.

**When the distances are recomputed (BIN, row M36).** The scene computes them for its objects
in one pass per view. A view's pass runs only when, since its previous pass, the camera has
moved 1.5 m or more, or more than 1 s has passed, or the view was not processed in the
previous frame; otherwise every object keeps the distances of the previous pass. Whether other
paths refresh the distances of moving objects between passes was not traced. A renderer may
recompute every frame; the difference is at most 1.5 m of camera movement.

**Smoothing (provenance row M24).** For objects drawn through an entity, the distance that
enters the detail factor of §8.4 is a smoothed copy *d_s*, kept per object and view:

1. In the first frame in which the object is processed in a view (its per-view state is new),
   *d_s* = *d*.
2. If the object's model has a **skinned part** (defined below), *d_s* = *d* in every frame:
   such models are not smoothed.
3. Otherwise, each frame, *d_s* ← *d_s* + *k* × (*d* − *d_s*) with *k* = min(1, 3 / (*F* +
   0.0002)), where *F* is the client's running frame rate in frames per second (0.0002 only
   guards against a zero rate). At 60 frames per second *k* = 0.05, so *d_s* lags fast
   camera moves by about a third of a second.

*d_s* serves **only** the detail factor of §8.4. The draw, fade and shadow tests of §8.7 and
the combined-mesh choice use the unsmoothed *d*. Interior cell geometry always uses *d*
(§8.5). A camera jump of more than 25 m (§8.7) does **not** reset *d_s*; it resets the fade.

**Skinned part.** When the client prepares a mesh, it marks the mesh if any draw entry of any
detail level (kind 1, §8.3) uses a vertex block whose influence count *K* (§4.2) is greater
than 0. A model has a skinned part when the mesh of at least one of its parts is marked. In
practice these are the figures that carry a skeleton (characters, creatures; §5.7); a model
made only of rigid meshes has no skinned part. The same property gates the combined mesh (§8.7)
and the cross-fade (§8.8).

> **Resolved (2026-10-07, BIN, row M36):** the per-view state is part of the object's scene
> node. The node is created when the object is added to the scene and destroyed when it is
> removed; an object that leaves the scene and comes back (out of residency, a rebuilt scene
> after a teleport) gets a new node, so its *d_s* starts again at *d*. Nothing else resets
> *d_s*: the camera-jump and view-reset handling of §8.7 sets the reset marker and clears it
> again within the fade step, after the smoothing of that frame has run. This is the earlier
> Tessera default.

<a id="s8-7"></a>
### 8.7 The other LOD-class distances (BIN)

The fields of the LOD-class entry (§8.5) whose use was traced. All tests below use the
unsmoothed distance *d* of §8.6. Multipliers (stored values as in §8.5's table; the
`DialBackGraphics` shift of §8.5 applies to all of them):

| Preference | Stored value → multiplier | Symbol |
|---|---|---|
| `ObjectDrawDistance` | 0 → 0.6, 1 → 0.8, 2 → 1.0, 3 → 1.4, 4 → 2.0 | *M_d* |
| `StencilShadows` | 1 → 0.6, 2 → 0.8, 3 → 1.0, 4 → 1.4, 5 → 2.0 (0 is `Disabled`; the multiplier is then 1 and no stencil shadow is drawn) | *M_s* |
| `ModelDetail` | as *M* in §8.5, times a second factor of the frame's render state (1 in the render state's defaults) | *M_c* |

**Global limit *G* (provenance rows M26; owner `walker.md` §11.3).** Each frame, before
objects are tested:

1. *G* starts unbounded.
2. When the view has a landscape, *G* is the landscape extent *E* of `walker.md` §11.3
   (160 m × (3 *g* + *n*); 2,080 m at the built-in `LandscapeDrawDistance`).
3. When distance fog is active and *G* is greater than the frame's fog far distance, *G*
   becomes the fog far distance under the condition stated in `walker.md` §11.3 step 3 (its
   switch *S* is OPEN there).
4. If *G* < 60 m, *G* = 60 m.

> **OPEN** (owned by `walker.md` §11.3): the switch *S* of step 3 and which fog value the
> 0.99 test reads. **Tessera default** (the same as `walker.md`): clip *G* to the fog far
> distance whenever fog is active; with no landscape and no fog, *G* is unbounded.

| Field | Use |
|---|---|
| `d0` | **Draw distance** *D* = min(*G*, `d0` × *M_d*) (row M25). The object is drawn while *d* ≤ *D*. Crossing *D* starts a fade in or out that changes the object's opacity linearly over 1.5 s. In the first frame of the object's per-view state, and in a frame in which the camera has moved more than 25 m since the previous frame (or the render state asks for a reset), the object appears or disappears at once. While distance-based levels are disabled (§8.4 step 1) no object is hidden or faded by distance. |
| `d1` | Detail range *R* (§8.5). |
| `d2` | **Shadow distance** (rows M27, M40): the object casts shadows while *d* < min(*G*, `d2` × *M_d* × *M_s*) (strictly below), at any distance while distance-based levels are disabled, and in both cases only while its opacity (§8.8) is above 0.8. |
| `d3` | **Combined-mesh distance** (rows M28, M38): for a model with a skinned part (§8.6), the client switches to one combined mesh instead of the separate parts while *d* > `d3` × min(1, *M_c*) (*d* counts as 0 while distance-based levels are disabled; an internal per-object flag can also force the combined form). The switch asks for the combined mesh; the client builds it only while **all** of these hold: the user's `MeshCombining` preference (`install-layout.md` §8.3) is on, the entity's `Render_UseMeshCombining` is true (dictionary default false; 54 descriptors set it to true), the model has a skinned part, the switch has asked for it, and two capability switches of the renderer are on. When any of them fails, a combined mesh that exists is released. The combined mesh is drawn only once it has been built; otherwise the separate parts are drawn. Its detail level follows §8.4 with the object's factor. |
| `d4` | No use found (row M35: no reader in the render component, model or scene code). |
| `d5`, `d7` | For a model with a skinned part, a hysteresis pair (row M35): a per-model state switches on when *d* < `d5` × *M_c* and off when *d* > `d7` × *M_c*. The state governs the model's **attachments**: per-model objects that the client loads as records of DB type 0x48 and keeps by DID. While the state is on they are updated each frame; while it is off, each attachment's run-time resource is released; every switch marks the set for an update. What the 0x48 records are was not identified. Models without attachments are not affected. |
| `d6` | No use found (row M35). |

The render component's own values, kept when no LOD-class entry reaches it (§8.5 step 3),
are `d0` … `d7` = 200, 60, 16, 80, 1000, 1000, −1, 1100, in record order: `d6` starts at −1
and `d7` at 1,100, the same as the table's common values (row M35). The client's table reader
does not keep the record's order for the last two floats: it stores the 7th float (`d6`) in
the slot the component initialises to −1 and the 8th float (`d7`) in the slot initialised to
1,100, and the copy into the component is then slot for slot. An implementation that keeps
the record's order throughout gets the same result. **Corrected 2026-10-07:** the revision of
the same day named `d6` and `d7` the wrong way round (initial values, the hysteresis row and
the field without a reader); code built on that naming must be re-checked. Check with the
data: class 0x10000014 has `d5` 12, `d6` −1, `d7` 20, so its attachments switch on below
12 m × *M_c* and off above 20 m × *M_c*. The component's ninth value, the
forced level of §8.8, starts at "none".

Worked example (synthetic). An entry with `d0` 80, `d2` 16, `d3` 80; `ObjectDrawDistance` 3
(*M_d* 1.4), `StencilShadows` 3 (*M_s* 1.0), `ModelDetail` 3 (*M_c* 1.4); landscape extent
2,080 m, no fog, so *G* = 2,080 m. Draw distance *D* = min(2,080, 112) = 112 m; shadows to
min(2,080, 22.4) = 22.4 m; combined mesh (skinned model with `Render_UseMeshCombining`) from
80 × min(1, 1.4) = 80 m on. With fog active and a fog far distance of 90 m that the *G* rule
applies, *G* = 90 m and *D* = 90 m. With fog far 40 m, *G* = 60 m (the floor), *D* = 60 m.
Smoothing at *F* = 60 with *d_s* = 100 m and *d* = 40 m: *k* = 3 / 60.0002 = 0.0499998, so
the next *d_s* = 100 − 0.0499998 × 60 = 97.0 m (rigid model; a skinned model uses 40 m at
once).

> **OPEN** (narrowed 2026-10-07; the gate is in the `d3` row): how the client builds a
> combined mesh from the parts (merged geometry, textures, skinning), and what the two
> renderer capability switches are. **Tessera default:** never combine; draw the separate
> parts at every distance (the result differs only in cost).

> **OPEN** (narrowed 2026-10-07): what the DB type 0x48 attachments of the `d5`/`d7` row are
> (the record kind and prefix were not identified; no DID range is registered for that type
> in the client's range table); whether any code outside the render component, the model and
> the scene reads `d4` or `d6`. **Tessera default:** ignore these fields.

**Imposter variant (BIN, row M37; answers the OPEN of 2026-10-05 for the model side).** The
render component sets an **imposter flag** when an `Render_ImposterName` value other than
`None` reaches it (dictionary default `None`; 1,072 descriptors set the property, 1,050 of
them to another value); nothing clears it. For an entity with the flag, while distance-based
levels are enabled, each frame and view:

1. *D* = min(*G*, `d0` × *M_d*) as above; *d* and *d_c* as in §8.6.
2. *w* = 1 − min(1, max(0, (*d_c* − (*D* − 15)) / 15)): 1 while the box centre is nearer than
   *D* − 15 m, falling linearly to 0 at *d_c* = *D*.
3. *w* becomes the target opacity of the model in that view. The 1.5 s fade of the `d0` row
   scales towards it (a fade in runs from 0 to *w*, a fade out from the current opacity to 0)
   instead of towards 1. If the object is not visible and 0 < *w* < 0.1, it is marked visible
   (so it fades in at the edge of the band).
4. The draw test of the `d0` row uses the limit *D* + (*d_c* − *d*) instead of *D*.

Worked example (synthetic): *D* = 80 m, box with *d* = 70 m and *d_c* = 72 m: *w* = 1 − (72
− 65) / 15 = 0.5333; the draw limit is 82 m, so the model is drawn at about half opacity.

> **OPEN:** what the client draws in place of the faded model (the dynamic-imposter system
> named by the `DistantImposters` preference: capture, texture and placement of the imposter
> sprite). **Tessera default:** apply steps 1–4 only if an imposter is drawn in its place;
> otherwise test these entities like all others.

<a id="s8-8"></a>
### 8.8 Shadow passes, cross-fades and forced levels (BIN)

- **Shadow passes (row M29).** The client keeps a second level per part for its
  shadow-casting draws (the draws limited by `d2`, §8.7). It is the level of §8.4, except
  when **both** of these hold; then it is the **last** detail level, at every distance:
  1. a switch of the frame's render state is on. It is off in the render state's defaults;
     the client's render paths set the render state per render call, and which of them turn
     it on was not identified;
  2. the entity casts "fake" shadows: its boolean property `Render_CastFakeShadows` is true
     (dictionary default false; 1,906 descriptors set it, 1,786 to true). The client also
     accepts the flag of a second scene object the entity's node refers to (by inference the
     object it is attached to, so that a held item follows its holder; not confirmed).

  The 1,976 meshes whose last level repeats detail level 1 (§8.3) fit this use: a
  medium-detail shadow caster that the main pass draws only beyond *R*.

  An object casts shadows only while its **opacity** is above 0.8 (row M40), so an object
  fading in or out (§8.7) stops casting shadows before it disappears. Opacity here is the
  product of the view's fade opacity (§8.7) and two factors of the object's render
  component, one of which follows `Render_Opacity` (dictionary default 1; both start at 1);
  when the entity's `Render_InheritOpacity` is true, it is also multiplied by the opacity of
  the object it is attached to, if that object was processed in the same frame. The result is
  then multiplied by a fourth factor (row M46): the alpha of a tint colour that the object's
  scene node keeps and refreshes at most every 20 s (the refresh also derives the colour's
  other three components from a lookup at the object's position, which this spec does not
  use). The alpha starts at 1, and every path traced sets it to 1, so Tessera uses 1.
- **Cross-fade (row M30).** A part close to a level threshold is drawn with both
  neighbouring levels when **all** of these hold: the entity's boolean property
  `Render_LODCrossFade` is true (its dictionary default is true, §8.5; 1,365 entity
  descriptors set it, 1,350 to false; the component also starts with true, row M34, so an
  entity that never names the property cross-fades); the view is the main one; the renderer's
  multi-pass lighting is active (`MultiPassLighting` preference on capable hardware); the
  object's model has **no skinned part** (§8.6); no forced level is set; the mesh has at least
  two detail levels; and an internal per-object flag is clear (it is set while one of the
  objects attached to this one, such as a held item, has a particular render state; not
  traced further). The weight of the part, from the factor *f* of §8.4 and the begin factors
  *t_k* of its detail level *k* and its neighbours:
  1. If a finer level *k* − 1 exists: *g* = 0.2 × (*t_k*₋₁ − *t_k*). If the level chosen at
     min(1, *f* + *g*) is not *k*, the second level is that finer level and its weight is
     1 − (*t_k* − *f*) / *g*.
  2. Otherwise, if a coarser level *k* + 1 exists: *g′* = 0.2 × (*t_k* − *t_k*₊₁). If the
     level chosen at max(0, *f* − *g′*) is not *k*, the second level is that coarser level and
     its weight is (*t_k*₊₁ − (*f* − *g′*)) / *g′*.
  3. The weight is limited to 0 … 1; with weight 0 nothing extra is drawn.

  **Composition (row M39).** When any part of the object has a weight above 0, the object's
  opacity is lowered to 253/255 if it is 254/255 or more, which sends the object through the
  translucent (alpha-blended) path, and the object is drawn in one of two composite
  modes. In the mode traced, each part first draws its own level as usual and then draws the
  second level again with alpha = the object's opacity × the part's weight, under a separate
  blend state for "finer" and for "coarser" second levels. The other mode is chosen when two
  switches of the frame's render state are on; it was not traced.
- **Forced level (row M31).** The render component holds a level index that is "none" by
  default. When it is set, the main level of every part (and of a combined mesh) is that
  index, limited to *n* − 1, and no cross-fade happens. The shadow level (first item above)
  is not affected.

> **OPEN:** what turns on the render-state switch of the shadow rule. **Tessera default:**
> off; shadow casters use the level of §8.4.

> **OPEN** (narrowed 2026-10-07): the second composite mode of a cross-fade, chosen by two
> switches of the frame's render state (whose meaning was not traced), and the two blend
> states. **Tessera default:** switch levels without a fade, or, if a fade is wanted, draw the
> second level alpha-blended with alpha = opacity × weight as in the traced mode.

> **OPEN:** where a forced level comes from. Narrowed on 2026-10-07 (row M44): no property
> handler sets it, and no store into the field after construction was found in the render
> component, model or scene code; a debugging or tool path elsewhere remains possible.
> **Tessera default:** never force a level.

<a id="s8-9"></a>
### 8.9 Kind-2 levels: occluders (BIN)

For each mesh of an entity part or of interior cell geometry, the client turns every kind-2
level into one **occluder model** of its occlusion-culling library: the vertex positions of
the vertex block of the level's first draw entry, and all triangles of that draw's index
buffer. The object's own bounding box is the volume that is tested for visibility; the
occluders are what can hide other objects behind it. Occluders are built only while the
client's occlusion culling is active.

A renderer may ignore kind-2 levels without any visible difference, or use them as cheap
occluders of its own.

<a id="s8-10"></a>
### 8.10 One-triangle meshes (BIN; census Confirmed)

When the client loads a mesh it adds up the triangles of every draw entry of every detail
level (index count / 3). A mesh whose total is exactly **one** triangle is flagged. When the
client builds the draw list of an object whose model has **more than one part**, it leaves
flagged parts out; a model whose only part is a flagged mesh still draws it.

In this install exactly the 258 meshes with three vertices in total are flagged (one detail
level, one triangle); record 0x0600C113, whose detail level 0 has one triangle but whose two
other levels have 24 each, is not. `setup.md` §8.5 gives what this means for the stub parts of
figures (owner item O4).

---

<a id="s9"></a>
## 9. Collision mesh

<a id="s9-1"></a>
### 9.1 Layout (Confirmed)

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `vertex_count` *cv* | u32 | May be 0 (1,761 levels) |
| 2 | vertices | *cv* × 3 bytes | u8 `draw`, u16 `vertex` |
| 3 | `triangle_count` *ct* | u32 | |
| 4 | triangles | *ct* × 12 bytes | u16 v0, u16 e0, u16 v1, u16 e1, u16 v2, u16 e2 |
| 5 | `edge_count` *ce* | u32 | |
| 6 | edges | *ce* × 8 bytes | u16 vA, u16 fA, u16 vB, u16 fB |

<a id="s9-2"></a>
### 9.2 Meaning (Confirmed)

- **Vertices** are references into the render geometry of **this level**: `draw` is a
  position in this level's draw list (not a global submesh index), and `vertex` indexes the
  vertex block of that draw entry. The collision position is that render vertex's position.
  Checked on 70,004 levels: always in range under this reading; reading `draw` as a global
  submesh index fails on 790 levels.
- The vertex list is strictly ascending by (`draw`, `vertex`), so each render vertex appears
  at most once. Render vertices that share a position are usually referenced once (the list
  is welded).
- **Triangles** give three collision-vertex indices v0, v1, v2 and three edge indices:
  e*k* is the edge from v*k* to v(*k*+1 mod 3). All indices are in range in every level.
- **Edges** name two collision vertices and up to two triangles. 0xFFFF in fB means "no
  second triangle" (an open border); 14.2 million edges are borders. When both triangles are
  present, fA < fB (no counter-example). An edge stored as (vA → vB) is traversed in that
  direction by triangle fA and in the opposite direction by fB (99.4 % of triangle edges in a
  1-in-25 sample; the rest belong to edges shared by more than two triangles).

The collision mesh is for physics and picking. Rendering does not need it. A decoder that
only renders must still parse it to find the next level (its size follows from the three
counts).

> **OPEN:** (found by the clean-room implementation, 2026-10-04, census of all 47,563
> records) Collision triangle and edge indices are u16, but 117 kind-1 levels in 105
> records have more than 65,536 edges (largest 148,395) and 19 of those also have more than
> 65,536 triangles (largest 95,904); collision vertex counts stay below 65,536 (largest
> 54,401). In these levels a triangle's edge indices and an edge's face indices cannot
> address every element, so "all indices are in range" above holds only because the values
> are 16-bit. The rule "fA < fB" fails in exactly the 19 levels with more than 65,536
> triangles (6,483 edges), which suggests face indices wrap modulo 65,536. How the original
> resolves elements beyond index 65,535 (wrap-around, an implicit high part, or not at all)
> is unknown. `tessera-mesh` reports such levels as `CollisionIndexOverflow` and skips the
> fA < fB check there; physics must not trust their edge and face indices.
>
> **Answered in part (2026-10-07, BIN, row M42):** the client's reader stores every
> triangle-edge index and edge-face index as the 16-bit value read, with no high part taken
> from anywhere else; in those 117 levels the client therefore holds the same truncated
> values. Whether any client code uses the edge and face indices of a collision mesh (the
> collision queries of `collision.md` use the tree code and the render triangles) was not
> traced; that part stays OPEN.

---

<a id="s10"></a>
## 10. Collision tree block

<a id="s10-1"></a>
### 10.1 Layout (Confirmed)

Present when `has_tree` = 1 (71,167 levels; 598 kind-1 levels and all proxy levels have 0).

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `has_code` | u8 | 0 or 1. 0 in 211 blocks (207 on one-triangle stub meshes, 2 with an empty collision mesh, 2 on ordinary meshes); then the block ends here. |
| 2 | `origin` | f32 × 3 | Present if `has_code` = 1 |
| 3 | `scale` | f32 | Present if `has_code` = 1 |
| 4 | `code_length` *n* | u32 | Present if `has_code` = 1 |
| 5 | `code` | *n* bytes | Opaque |
| 6 | `build_byte` | u8 | Present if `has_code` = 1 **and** the record's directory `stamp` ≥ 35 (§10.4; no observed stamp lies between 34 and 37). Always 1. |

<a id="s10-2"></a>
### 10.2 What the block is (Reported analogy, partly Confirmed)

The block has the shape of a Havok "MOPP" bounding-volume code (AN): a quantisation origin
and scale followed by a byte program. The first code byte is 0x28 in 95 % of blocks (then
0x27, 0x26).

**Corrected by `collision.md` §3 and §7.1 (2026-10-05):** the code indexes the level's
**render** triangles (key = position of the draw entry in the level · 2²⁰ + triangle index in
that draw's index buffer), not the triangles of the §9 collision mesh; `origin` and `scale`
follow from the box of the render triangles in the code, with 2²⁴ / `scale` exactly
(*L* + 0.1) · 256 / 254 of its largest extent *L*. The instruction set and queries are
specified there. (The earlier reading, "minimum corner of the collision vertices minus about
0.05; 2²⁴ / `scale` 0.96 – 1.53 times their extent", was an approximation from 395 samples.)
A renderer does not need the code.

<a id="s10-3"></a>
### 10.3 Hostile-input notes

`code_length` is bounded by the bytes remaining in the record. Collision data is large: in
the static meshes inspected while writing this spec, region D was roughly ten times the size
of region C, so a decoder must not assume region D is small.

<a id="s10-4"></a>
### 10.4 The `build_byte` rule (Confirmed, every block)

Whether `build_byte` exists cannot be read from the block itself. It depends on the
directory entry's `stamp` (`dat-container.md` §5.2), a per-record data-generation number:

| Directory `stamp` of the mesh record | Tree blocks with code | `build_byte` |
|---|---|---|
| 7, 8, 11, 12, 14, 20, 21, 22, 27, 28, 29, 31, 32, 33 | 25,987 | absent |
| 38, 39 | 44,969 | present (value 1) |

The split is perfect: no stamp value has both forms. A mesh decoder therefore takes the
record's `stamp` as an input next to the payload bytes.

**Normative rule:** `build_byte` is present if and only if `stamp` ≥ 35 (revised 2026-10-07;
the former "≥ 38" gives the same result on every record of the install, because no mesh has
a stamp from 34 to 37).

**Why (BIN, row M41).** The client's tree reader asks the record's input stream for the
version of its Havok-data tag and reads `build_byte` when that version is 1 or more. The
client holds a fixed history of 39 data-version steps, each raising the version of one or
more tags; the Havok-data tag is 0 before step 35, becomes 1 at step 35 and 2 at step 37. A
record's tag versions are those of the first *s* steps, where *s* is its `stamp`
(`dat-container.md` §5.2, the record's data version). That last link was not read in the
stream code; it is inferred from three agreements: the history has exactly 39 steps and 39 is
the largest stamp in the install; the archive's version-stamp record requires `stamp` 1 and
step 1 sets the core tag to 1; 0xFFFF0001 carries its trailing list only at `stamp` 2
(`dat-container.md` P57), the step that raises the core tag to 2. All 71,167 tree blocks
agree with the resulting threshold.

**Fallback when no stamp is available** (fuzzing, payloads from other sources). This rule
agrees with the normative one on all 47,563 records:

1. If this is the last level, `build_byte` is present exactly when one byte remains in the
   record.
2. Otherwise, look at the next 8 bytes as two u32. If the first is 1 or 2 and the second is
   ≥ 1, they are the next level's `kind` and `draw_count`, and `build_byte` is absent.
   Otherwise `build_byte` is present.

(With the byte present, the next u32 would start with byte 0x01 followed by the 0x01 of
`kind`, giving 0x101 or larger, which is never a valid `kind`.)

> **Resolved (2026-10-07, BIN, row M41):** the threshold is 35 (stamps 34 and below: absent;
> 35 and above: present). Stamps 34–37 still do not occur on meshes; the fallback stays for
> payloads without a stamp.

---

<a id="s11"></a>
## 11. Coordinate system, units, handedness, winding, UV origin

<a id="s11-1"></a>
### 11.1 Axes and units (Confirmed)

- **Z is up.** Characters and buildings stand on the XY plane; character feet are at z = 0
  (NPC bodies span z = 0 … 1.79 and 0 … 1.82).
- **Units are metres.** Human-sized NPC bodies are 1.8 units tall; terrain uses 160-unit
  landblocks (research notes).
- **Characters face +Y.** Rendering two NPC bodies from the +Y side shows their faces, and
  their toes extend towards +Y.
- The origin of a character or prop is on the ground at its foot point, not at its centre.

<a id="s11-2"></a>
### 11.2 Handedness (Confirmed)

The data is **right-handed**: with up = +Z and forward = +Y, the character's right is +X.
Evidence: on two NPC setups whose skeletons name their bones, vertices dominated by the
right wrist, right shoulder and right foot bones (by their names, which carry a side prefix)
average x = +0.10 … +0.76, and the matching left-side bones average the same magnitude at
negative x. A renderer whose world is right-handed uses
positions unchanged. A left-handed renderer must mirror one axis and flip winding.

<a id="s11-3"></a>
### 11.3 Winding (Confirmed)

Front faces are **counter-clockwise** when seen from the side the normal points to, in the
right-handed frame above. Over all 95,366,468 triangles, the geometric normal
(p1 − p0) × (p2 − p0) agrees in direction with the sum of the three vertex normals for
94,698,787 triangles (99.30 %) and disagrees for 665,819 (not investigated; two-sided cards
whose normals point one way are a likely cause). Renderers cull back faces by this rule, but materials flagged two-sided
(`material.md`) must disable culling.

<a id="s11-4"></a>
### 11.4 UV origin (Confirmed)

UV (0, 0) maps to the first texel of the first stored row of the surface image
(`records.md` §5.3), u grows along the row and v grows down the rows. **Do not flip V.**
All textured renders made for this spec (a crate stack, walls, a tower, a statue, a cottage
roof, two NPC bodies with faces and belts) were correct with raw V. Re-rendering the crate
stack and one NPC body with V flipped put the wrong texture regions on every face. In a graphics API whose texture origin is the first
uploaded row (wgpu, Direct3D, Vulkan), pass UVs through unchanged.

---

<a id="s12"></a>
## 12. Surface references (Confirmed)

- The header lists *S* = *N* surface DIDs. All 112,910 references resolve to existing 0x31
  records in `client_general.dat` (12,028 distinct DIDs out of 13,781 surface records).
- A draw entry's `surface` field selects the surface for that draw. It always equals the
  submesh index, so in practice submesh *i* uses `surfaces[i]`.
- 13,398 records list the same surface DID more than once (each detail level repeats the
  surfaces of level 0 in the same order in 23,772 of 24,202 cases).
- The mesh record holds no material, texture or shader DID itself. Surface → material →
  texture list → surface image is specified in `material.md` and `texture-list.md`.

---

<a id="s13"></a>
## 13. Decoding algorithm (in words)

Inputs: the inflated payload and the directory entry's `stamp` (optional, §10.4).

1. Read the DID, `flags`, *S* and the *S* surface DIDs, then *N*. If *N* ≠ *S*, report
   malformed (never observed).
2. For each of the *N* submeshes:
   1. Read `vfmt` and *V*. Reject unsupported format words (§4.2). Compute the stride.
   2. Check that *V* × stride + 28 bytes remain; otherwise report truncation. Take the vertex
      bytes as one slice; do not copy per vertex unless converting.
   3. Read the AABB and the influence list (bound *I* by the remaining bytes / 4).
3. Read the index-buffer count. If it is not *N*, report malformed. For each buffer read the
   count and the u16 indices (bound by remaining bytes / 2). Check count % 3 = 0 and every
   index < *V* of its submesh.
4. Read *L*. For each level read `kind`, *D* and *D* draw entries. Check every index field
   < *N* and `primitive` = 4.
5. Read `has_collision_mesh`; if 1, read the three counted arrays of §9.1 (bound each count by
   the remaining bytes divided by the element size). Read `has_tree`; if 1, read `has_code`;
   if 1, read origin, scale, the code length (bounded) and skip the code; then apply §10.4 for
   `build_byte`.
6. After the last level the payload must be exhausted. Leftover bytes mean malformed data or
   an unknown format generation; report them, do not ignore them.

For rendering a level: for each draw entry in order, take vertex block `vertex_block`,
decode the attributes the renderer needs at the offsets of §4.3, add the running vertex
offset to the indices of `index_buffer`, and bind surface `surfaces[surface]`. Which detail
level to draw follows §8.4–§8.8; kind-2 levels are never drawn (§8.9); a one-triangle mesh
that is one of several parts is not drawn (§8.10).

Allocation bounds (hostile input): vertex count ≤ 65,536 (u16 indices cannot address more;
observed maximum 57,603), *N* ≤ remaining / 36 (a submesh block needs at least 36 bytes),
*L* ≤ remaining / 23 (a level needs at least 23 bytes), draw count ≤ remaining / 13.

---

<a id="s14"></a>
## 14. Invariants (Confirmed over all 47,563 records)

| ID | Invariant |
|---|---|
| **MI1** | The record starts with its own DID. |
| **MI2** | *S* = *N* = index-buffer count. |
| **MI3** | Every `vfmt` is supported (§4.2); stride per §4.2. |
| **MI4** | AABB = bit-exact min/max of positions, per submesh. |
| **MI5** | Normals unit length; weights sum to 1 (± 2 × 10⁻⁴); first weight > 0; zero weights have index 0. |
| **MI6** | Influence list = for each vertex from the last to the first, its slot-0 bone index written once per non-zero weight of that vertex (§6.2); so its length is the number of non-zero weights, 0 when *K* = 0. |
| **MI7** | Index count multiple of 3; indices in [0, *V*−1]; min 0; max *V*−1; no degenerate triangles; every vertex used. |
| **MI8** | Draw entries: primitive 4; the three indices < *N* (and equal in all observed data); every submesh drawn exactly once over all levels. |
| **MI9** | Level kind ∈ {1, 2}; kind 2 levels come last; kind 1 ⇔ `has_collision_mesh` = 1. |
| **MI10** | Collision indices in range; vertex list strictly ascending; fA < fB when fB ≠ 0xFFFF. |
| **MI11** | `build_byte` presence follows §10.4. |
| **MI12** | The payload ends exactly after the last level (zero trailing bytes). |
| **MI13** | Every kind-2 level has exactly one draw entry (4,173 of 4,173). |
| **MI14** | The meshes whose detail levels hold exactly one triangle in total are exactly the meshes with three vertices in total (258 of 258). |

MI10 does not hold literally in the 117 collision levels with more than 65,536 edges
(§9.2 OPEN): their u16 edge and face indices cannot address every element.

MI4, MI5, MI6, MI7, MI10, MI13 and MI14 are data-quality checks: a viewer may log a violation
and still draw (the client would use only the first draw entry of a kind-2 level, §8.9).
MI1–MI3, MI8, MI9 (structure), MI11 and MI12 are structural: violating them means the decoder
and the data disagree. (MI9 is stricter than the client, which also accepts kind 4 and ignores
other kinds, §8.3; a reader meeting another kind should report it, not crash.)

---

<a id="s15"></a>
## 15. Statistics and failure categories (Confirmed)

| Measure | Value |
|---|---|
| Records decoded / total | **47,563 / 47,563 (100 %)** |
| Trailing bytes after decoding | 0 in every record |
| Failure categories | none observed. A decoder should still distinguish: truncated, unsupported vertex format, index out of range, structural mismatch (MI2, MI8, MI9), trailing bytes. |
| Submeshes | 112,910 (1 … 78 per record; 24,734 records have one) |
| Vertices / triangles | 89,593,149 / 95,366,468 |
| Submeshes with bone influences | 43,059 |
| Detail levels / proxy levels | 71,765 / 4,173 |
| Collision meshes / tree blocks / tree blocks with code | 71,765 / 71,167 / 70,956 |

**Visual checks done for this spec** (scratch renders, not committed): 8 random static meshes
of 2,000–15,000 triangles, 4 random skinned meshes in bind pose, a tree with the scalar
attribute and three UV sets, a gate and a strip-shaped prop with three or more UV sets, two NPC bodies from
front and back, two proxy levels in wireframe, and three multi-part setups. All showed
correct shapes, textures on the right faces, and no stray triangles.

**About the "sliver" check of MV.** The heuristic "an edge longer than half the bounding-box
diagonal is a decode error" flags 404,931 triangles in 13,527 records of this install. Every
flagged mesh that was rendered (a log stack, a cottage, a tower, a gate) was correct: long
edges are normal on low-polygon architecture. With the deterministic stride rule the check
is not needed; the M2 visual checklist should use renders instead.

**M2 visual checklist** (2026-10-04, renders by `tessera-explorer`, recorded as text in
[`docs/checklists/m2-visual.md`](../checklists/m2-visual.md)): 74 checks on weapons,
armour (with dye previews), creatures and NPCs in bind pose, vegetation, buildings and
props; 73 pass, no slivers or UV faults. The one failure is a one-triangle placeholder part
drawn as a white triangle, not a decoding error: the original leaves such parts out of
multi-part objects (§8.10, `setup.md` §8.5).

---

<a id="s16"></a>
## 16. Corrections to earlier notes

| Earlier claim | Source | What the data shows |
|---|---|---|
| "u32 T × u32 material (0x30)" follows the surface list | MV, DS | The u32 after the surface list is the submesh count *N*; the "0x30…" values were the submesh format words (top byte 0x30 = tangent flag) read as DIDs. Meshes hold no material DIDs. |
| "There is no stride field; detect it from unit normals" | MV, DS | The stride follows from the format word (§4). |
| "UV is always at offset 24" | MV | Only without SCALAR and COLOUR (§4.3). |
| "u32 nBone × u32 boneData" after the bbox | DS | An influence list whose length is the number of non-zero weights (§6.2). |
| "3-vertex stubs have zero-length normals" | MV | All normals in the install are unit length. |
| Static = flags 1, skinned = 0x1000____ | MV | Bit 28 does not decide skinning; use the per-submesh format word (§3.3). |
| A float read before the index region | KC | It is the u32 index-buffer count. |
| Everything after the index region | (none) | Detail levels, draw lists and collision data (§8–§10), previously undocumented. |
| Low 16 bits of skinned-mesh `flags` and value 2 unexplained | this spec §3.3 (before 2026-10-05) | The whole word is a `MeshTypeID` value; 2 is `All` (§3.3). |
| "In the other 2,037, the last kind-1 level repeats the size of level 1" | this spec §8.3 (before 2026-10-05) | 1,976 of them; 35 others rise in the last step to another size, 22 rise elsewhere, 4 two-level records rise (§8.3). |
| Switch distances and proxy-level use OPEN; "a viewer draws level 0 only" | this spec §8.3 (before 2026-10-05) | Distance rule and ranges §8.4–§8.7; kind-2 levels are occluders (§8.9). |
| Influence-list meaning OPEN (provenance row M8) | this spec §1 (before 2026-10-05) | Redundant, fully determined by the vertices (§6.2). |
| Tree origin and scale relate to the collision vertices | this spec §10.2 (before 2026-10-05) | They relate to the render triangles in the code (`collision.md` §3.2, §7.1). |
| "A reset [of the smoothed distance] happens when the camera has moved more than 25 m since the previous frame (or the view was reset), and the smoothing is skipped for models that switch it off" | this spec §8.6 (first 2026-10-05 version) | The 25 m jump resets only the fade of §8.7. *d_s* starts at *d* when the object's per-view state is created; models with a skinned part are never smoothed (§8.6, row M24). |
| Cross-fade requires that "the parts are not drawn as a combined mesh" | this spec §8.8 (first 2026-10-05 version) | It requires that the model has **no skinned part** (§8.6); combined meshes exist only for models with a skinned part, so the old wording was too weak (§8.8, row M30). |
| `d0` and `d2` are tested against "its distance (§8.6)" | this spec §8.7 (first 2026-10-05 version) | Against the **unsmoothed** distance *d* (§8.7). |

---

<a id="s17"></a>
## 17. Open questions

> **Resolved (2026-10-05, BIN):** `flags` is the part's `MeshTypeID` slot; 2 is `All` (§3.3).

> **OPEN:** Meaning of the per-vertex scalar (§5.3).

> **Resolved in part (2026-10-05, `lighting.md`):** vertex colour is read by 116 shader
> records only (§5.4); UV sets 1 and 2 serve baked lighting and effects (§5.5). **OPEN:** which
> set each baked-lighting slot reads, sets 3 … 6.

> **Resolved (2026-10-05):** the influence list is redundant: per vertex in reverse order, its
> slot-0 bone index once per non-zero weight (§6.2).

> **Resolved (2026-10-05, BIN):** the detail-level choice, its ranges and the LOD-class
> distances (§8.4–§8.7); the last detail level (main pass beyond *R*, shadow casters, §8.8);
> kind-2 levels are occluders (§8.9); one-triangle meshes (§8.10).

> **Resolved (2026-10-05, second BIN session):** the source of *G* (§8.7, with `walker.md`
> §11.3); which models use the combined mesh of `d3` and which are not smoothed (models with
> a skinned part, §8.6–§8.7); the shadow rule's two conditions (§8.8); `d5` and a later
> field are a hysteresis pair (§8.7; the second field is `d7`, corrected 2026-10-07).

> **Resolved (2026-10-07, third BIN session):** the class of an entity without
> `Render_LODClass` (component initial values, as `walker.md` §11.4; §8.5); what resets the
> smoothed distance (a new scene node only; §8.6); the imposter variant on the model side
> (§8.7); the combined-mesh gate including `MeshCombining` (§8.7); the `d5`/`d7` state
> (per-model attachments) and the initial `d6` −1 and `d7` 1,100 (§8.7; both corrected
> 2026-10-07, row M35); the traced cross-fade
> composite mode and the shadow opacity limit (§8.8); the `build_byte` threshold (§10.4).

> **OPEN** (each with a Tessera default in its section):
> - the switch *S* and the fog value of the *G* rule (§8.7, owned by `walker.md` §11.3;
>   default: clip to fog far whenever fog is on);
> - how the combined mesh is built and the two renderer capability switches (§8.7; default:
>   never combine);
> - what the DB type 0x48 attachments are; any use of `d4`, `d6` outside the code searched
>   (§8.7; default: ignore);
> - what is drawn in place of an imposter entity's faded model (§8.7);
> - what turns on the render-state switch of the last-level shadow caster (§8.8; default:
>   off);
> - the second cross-fade composite mode (§8.8; default: no fade) and where a forced level
>   comes from (§8.8; default: none);
> - the use of kind 4 (§8.3).

> **Resolved (2026-10-07, BIN, row M41):** the `build_byte` threshold is `stamp` ≥ 35 (§10.4).

> **Answered in part (2026-10-05, BIN):** the client's sizes for COLOUR2 (4 bytes), a lone
> tangent (bit 28, 12 bytes) and a lone binormal (bit 29, 12 bytes) are in §4.5. None occur;
> decoders may keep rejecting them (§4.2).

> **OPEN** (answered in part 2026-10-07, row M42): collision meshes with more than 65,536
> triangles or edges. The client keeps the 16-bit values as read, with no high part; whether
> any client code uses edge and face indices is open (§9.2).

> **OPEN:** Influence counts *K* > 4 and formats with NORMAL clear were never observed. The
> stride rule defines their layout, so `tessera-mesh` accepts them (up to *K* = 255, 15 UV
> sets): a GPU path keeps the four largest weights renormalised to their sum, and missing
> normals are generated from faces. Whether the original accepts them is unknown.

> **OPEN:** Whether vertex colour bytes are sRGB-encoded or linear (§5.4; `material.md`).
> `tessera-mesh` hands out the raw bytes in R, G, B, A order.

---

<a id="s18"></a>
## 18. Implementation status (`crates/mesh`, clean room)

Census by the gated real-data test `crates/mesh/tests/real_data.rs` on the owner's install,
2026-10-04 (counts only):

| Measure | Result |
|---|---|
| Records decoded (stamp rule of §10.4) | 47,563 / 47,563 (100 %) |
| Failure categories | none (the decoder distinguishes truncated, unsupported vertex format, index out of range, structure, trailing bytes, limit exceeded, invalid (strict mode), source) |
| Lossless re-encoding (decoded record → bytes identical to the payload) | 47,563 / 47,563 |
| §10.4 fallback rule agrees with the stamp rule | 47,563 / 47,563 |
| Every level flattens into shared streams within the default limits | 47,563 / 47,563 records |
| Largest flattened level | 99,478 vertices, 287,712 indices, 28 draws, about 6 MiB of streams |
| Largest ratio of flattened stream bytes to the stored vertex and index bytes of a level | 1.16 |
| Largest record | 78 submeshes, 17 levels |
| Strict decoding (structural issues only) | 47,563 / 47,563 |
| Submeshes / vertices / triangles / skinned submeshes | 112,910 / 89,593,149 / 95,366,468 / 43,059 (as §15) |
| Detail / proxy levels; collision meshes; tree blocks (with code, with `build_byte`) | 71,765 / 4,173; 71,765; 71,167 (70,956, 44,969) |
| Distinct vertex format words | 27 (as §4.4) |
| §14 invariant failures | only `CollisionIndexOverflow` (117 levels in 105 records, §9.2 OPEN); no other invariant failed |
| §6.2 influence list equal to its recomputation from the vertices (`InfluenceListContent`, added 2026-10-05) | every skinned submesh (no issue in 47,563 records) |

Decisions the spec left to the implementation:

- A level count of 0 and a draw count of 0 are rejected as structural (§8.1 says ≥ 1; the
  allocation bound of §13 assumes it).
- A draw that pairs a vertex block with a different index buffer is accepted when the
  buffer's largest index fits the block, and rejected as index-out-of-range otherwise.
- Bounds: the AABB of the finite positions drawn by level 0 and a sphere around its centre;
  the stored per-submesh AABB is kept and compared by value (−0 = +0) in validation.
- Flattening a level is bounded because a small record can draw one vertex block many
  times, and streams that one part has are filled for every vertex. Defaults: 1 Mi
  vertices, 4 Mi indices, 1,024 draws, 128 MiB of streams, and flattened stream bytes at
  most 64 × the vertex and index bytes the level stores (each block counted once). Skin
  reduction to four influences is done once per vertex block.
- A record may have at most 4,096 submeshes and 4,096 levels (hostile-input bounds on the
  fixed per-element overhead of a decoded mesh, far above the observed 78 and 17). A
  decoded mesh then holds at most about 1.4 × the record bytes plus a few MB.
- Strict decoding fails only on structural invariants (§14: the decoder and the data
  disagree); quality violations, including the MI10 overflow above, still decode.
- When a format has tangents but no normals, the bitangent sign is taken against the
  generated face normals.
- M4 (2026-10-05): `MeshSummary` reads regions A–C without decoding vertices (slot and stub
  detection for assembly, `appearance.md` §4.4; a stub is a mesh with at most three vertices
  in all). `Mesh::skinned_level` flattens a level with four influences per vertex and the
  bones each part uses, checked against a skeleton's bone count (`setup.md` SI10); the joint
  remap is the identity (`skeleton-anim.md` §4.1), and `CompactPalette` is the optional
  per-part palette of `skeleton-anim.md` §4.3.


Note for the implementation (2026-10-05, spec writer): the stub test of `MeshSummary` (at most
three vertices in all) and the client's one-triangle flag (§8.10) pick the same 258 records on
this install (MI14). The client's rule is the triangle count over the detail levels; prefer it
where the two could differ.

---

<a id="s19"></a>
## 19. Corrections to other specs

Recorded here instead of editing specs owned by other writers.

| Spec | Statement | Correction |
|---|---|---|
| `environment.md` §9.1 | Meaning of the LOD-class fields `d0` … `d7` and how the client applies them OPEN ("BIN not traced") | Traced for `d0` … `d3` (BIN): `d0` draw distance × `ObjectDrawDistance` multiplier, `d1` detail range × `ModelDetail` multiplier, `d2` shadow distance, `d3` mesh-combining distance; class from `Render_LODClass`; record found through `UNIQUEDB` key 0x3F; defaults 200, 60, 16, 80; × 3 for the player's own character (§8.5–§8.7). `d4` … `d7` stay OPEN. |
| `scenery.md` §9 | The per-class distance table that controls drawing is OPEN | It is the LOD-class record; the rules are §8.5–§8.7 here. |
| `walker.md` §11.4 item 1 | "The object is drawn while its smoothed distance (`mesh.md` §8.6 …) is at most *D*" | The draw test uses the **unsmoothed** distance *d*; the smoothed copy feeds only the detail factor (§8.6, §8.7). Models with a skinned part (characters, creatures) are never smoothed. |
| `walker.md` §11.4 item 4 and §11.4.1 row "(no class)" | Entities without `Render_LODClass` use 200 m and 60 m | **Agreed (2026-10-07, row M33):** `walker.md` is right; this spec's former decision (class 2) is withdrawn (§8.5). |
| `walker.md` §11.4 step 4 and W20 | "the last two fields are 1,100 and −1, and which of them corresponds to the record's `d6` and which to `d7` was not traced" | In record order (`d0` … `d7` = the entry's floats in stream order, as in `environment.md` §9.1): `d6` = −1 and `d7` = 1,100 (row M35, §8.7; corrected 2026-10-07, the revision of the same day had them the wrong way round). |
| `environment.md` §9.1 (addition to the row above) | `d4` … `d7` OPEN | `d5` and `d7` are a hysteresis pair for models with a skinned part that switches the model's per-model attachments: on below `d5` × *M_c*, off above `d7` × *M_c* (§8.7); no reader of `d4` or `d6` was found. Component values when no entry applies: 1000, 1000, −1, 1100 for `d4` … `d7` (the table's common values). Corrected 2026-10-07: the revision of the same day named `d6` and `d7` the wrong way round. |
| `collision.md` §3 (field table, §3 note), §5 (row 6), invariant list | `build_byte` present iff `stamp` ≥ 38; "an unobserved stamp 34–37" | The client's threshold is `stamp` ≥ 35 (§10.4, row M41). Same result on every record of the install. |

---

<a id="s20"></a>
## 20. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-04 | all | First complete version (Review). | Spec writer (dirty room) |
| 2026-10-05 | §6.2, §14 MI6, §18 | Influence list found redundant and its rule stated. | Spec writer (dirty room) |
| 2026-10-05 | header, §1 (legend, BIN, CNS, ENV/COL/SA/LGT; M8 updated; M15–M23), §3.3, §4.1, new §4.5, §5.3–§5.5, §6.1, §8.1, §8.3 rewritten, new §8.4–§8.10, §10.2, §13, §14 (MI6 restated, MI13, MI14), §15, §16, §17, §18 (note), new §19, §20 | M4 (decision 0011): `flags` is the part's `MeshTypeID` slot (OPEN closed); the client's vertex-layout routine; the detail-level choice by distance with LOD-class ranges, `ModelDetail` multipliers and bounding-box distance; shadow, cross-fade and forced levels; kind-2 levels are occluders; one-triangle meshes are left out of multi-part objects (owner item O4); the 2,037-record census refined; collision-tree corrections of `collision.md` folded in; vertex-colour and UV-set use from `lighting.md`. | Spec writer (dirty room) |
| 2026-10-05 | header, §1 (BIN questions 4–7, WLK, PDEF; M20; new M24–M32), §5.4, §5.5, §8.4 step 1, §8.5 (defaults, OPEN), §8.6–§8.8 rewritten, §11.2, §16, §17, §19 | Review changes: provenance rows for §8.6–§8.8 and for the `lighting.md` facts of §5.4–§5.5; every unknown of §8.6–§8.8 as an OPEN callout with a Tessera default; §8.4 step 1 reworded (distance-based levels enabled by default); bone names in §11.2 described, not quoted. Second M4 session (decision 0011): *G* traced (agrees with `walker.md` §11.3); "skinned part" defined and found to gate distance smoothing, the combined mesh and the cross-fade; `Render_UseMeshCombining` gates the combined mesh; `d5`/`d6` hysteresis; shadow-rule switch is off in the render state's defaults; draw and shadow tests use the unsmoothed distance; corrected the smoothing reset and the cross-fade condition (§16). | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN third session, questions 8–18; M11; new M33–M44), §5.3, §8.3, §8.4 step 1 (wording, review note), §8.5 (step 3, OPEN resolved), §8.6 (distances, distance pass, OPEN resolved), §8.7 (`d2`–`d7` rows, component values, imposter variant, OPENs narrowed), §8.8 (shadow opacity, cross-fade conditions, weights and composition, forced level), §9.2, §10.1 row 6, §10.4 (rule ≥ 35, OPEN resolved), §17, §19, §20 | M4 (decision 0011), third session: an entity without `Render_LODClass` keeps the component's initial values (converges with `walker.md` W20; former class-2 decision withdrawn); component initial values of the other render properties; smoothing restarts only with a new scene node; the distance pass reruns after 1.5 m, 1 s or a missed frame; `d6` 1,100 and `d7` −1; the `d5`/`d6` state switches per-model attachments; the imposter variant (15 m centre-distance band, shifted draw limit); the combined mesh needs the `MeshCombining` preference; cross-fade weights and the traced composite mode; shadows only above opacity 0.8, strictly inside the shadow distance; `build_byte` threshold `stamp` ≥ 35 from the client's data-version history; collision indices stay 16-bit in the client; the scalar is uploaded to the GPU; negative searches for the forced level and kind 4. Status Accepted → Revised. | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (BIN fourth session, questions 19–21; M28, M33, M35 corrected; new M45, M46), §8.5 steps 3–4, §8.7 (`d5`–`d7` rows, component values, OPEN), §8.8 (opacity), §17, §19, §20 | Review changes of 2026-10-07. **Blocking:** `d6` and `d7` were named the wrong way round. In record order (`d0` … `d7` = the entry's floats in stream order) the component starts with `d6` −1 and `d7` 1,100; the attachment hysteresis switches off above `d7` × *M_c* (not `d6`); the field with no reader is `d6` (not `d7`). Re-read on the same build (table reader, LOD-class setter, per-frame model update) and checked on all 32 entries of 0x32000000 (class 0x10000014: `d5` 12, `d6` −1, `d7` 20). **Implementers who built on the earlier naming must re-check** (initial values, hysteresis, ignored field). Non-blocking: the player's ×3 is applied even when the class has no entry, and compounds on each further application (§8.5 step 4, row M45; every class value held by a descriptor in this install has an entry); the opacity of §8.8 has a fourth factor, the alpha of a per-object tint, 1 in every path traced (row M46). Status stays Revised, awaiting re-review. | Spec writer (dirty room) |
