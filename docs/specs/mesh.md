<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Mesh records (prefix 0x06): geometry, vertex formats, detail levels, collision

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/mesh` (decoder, engine vertex streams), `crates/render` (pipelines), `apps/explorer` (mesh viewer), `fuzz/` (mesh target) |
| **Companion specs** | `dat-container.md` (archive set, routing, the directory entry `stamp` field this spec needs in §10.4). `records.md` §1 (primitive encodings). `material.md` (surface 0x31 → material 0x30 → texture list; not repeated here). `texture-list.md` (0x40 → 0x41). `setup.md` (which records reference meshes, and how parts are assembled). |

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
- detail levels and draw lists (§8), and the collision data stored behind them (§9–§10);
- coordinate system, units, handedness, winding and UV origin (§11);
- how a mesh names its surfaces (§12);
- the decoding algorithm in words, invariants, statistics and failure categories (§13–§15).

All counts are observations of one install on one date. They change with patches. Tests
assert the invariants (§14), not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install on 2026-10-04 by throwaway probes in the
  dirty-room scratchpad. A Confirmed fact states what was decoded and how much of it.
- **Reported**: comes only from third-party material or analogy and was not, or could not
  be, checked against data.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). **All 47,563** mesh records (37,484 in `client_mesh.dat`, 10,079 in `client_mesh_aux_1.datx`) decoded to the last byte; every vertex of every submesh read and checked (89,593,149 vertices, 95,366,468 triangles). About 25 meshes and 3 multi-part assemblies were rendered with a scratch software rasteriser (textured, Z-up, no V flip) and inspected by eye: crates, walls, rocks, a tower, a statue, a cottage, two NPC bodies in bind pose, armour pieces, a tree, a gate, detail-level proxies. Images were kept in the scratchpad only. |
| MV | Mirarkitty `lotro_mesh_viewer`, `docs/mesh-format.md` and `docs/animation.md` (repository carries GPL-3.0 and MIT licence files). Facts only. |
| KC | gitlab `kilves/lotro-mesh-converter` (GPL-3.0), written against older client data. Studied for facts only; no code taken. |
| DS | `research/notes/data-semantics.md` §7.3 (summary of MV). |
| D3D | Public Direct3D 9 documentation: flexible-vertex-format (FVF) bit layout, `D3DPRIMITIVETYPE`, `D3DCOLOR`. General references, used as analogies. |
| AN | Analogy only, not verified: the client licenses Havok Physics (research notes §8); the collision tree in §10 has the shape of a Havok "MOPP" code block. |
| DC | `dat-container.md` §5.2 (directory entry `stamp`). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| M1 | Mesh records live in `client_mesh.dat` and `client_mesh_aux_1.datx`; the two DID sets are disjoint (§2) | DS; OBS | Confirmed |
| M2 | Header: DID, flags, surface-DID list, submesh count equal to the surface count (§3.1) | MV, KC (partly); OBS (47,563 of 47,563) | Confirmed. MV misread the submesh count and format words as a material list (§16). |
| M3 | Each submesh starts with a **vertex format word**; the stride and attribute layout follow from it deterministically (§4) | KC (UV count, influence count, tangent flag, 5 bytes per influence); D3D (bit layout); OBS (112,910 of 112,910 submeshes; bounding box after every vertex block matches bit-exactly) | Confirmed |
| M4 | Attribute order: position, normal, point-size scalar, colour, UV sets, tangent, binormal, bone indices, bone weights (§4.3) | D3D (FVF order), DS (skin-tail offsets); OBS | Confirmed |
| M5 | Normals are unit length; tangent is +U, binormal is +V (§5) | OBS | Confirmed |
| M6 | Bone indices are u8, one per influence, indexing the skeleton bone array directly; weights are f32 and sum to 1 (§5.7) | MV, DS; OBS (weights all submeshes; index meaning by bone names on 2 NPC setups) | Confirmed |
| M7 | Per-submesh AABB follows the vertex block and equals the min/max of the positions bit-exactly (§6.1) | MV; OBS (112,910 of 112,910) | Confirmed |
| M8 | The per-submesh u32 list after the AABB has one entry per non-zero weight (§6.2) | OBS (43,059 of 43,059 skinned submeshes; 0 entries on all 69,851 others) | Confirmed (length). Meaning OPEN. |
| M9 | Index buffers come after all vertex blocks, one per submesh; u16 submesh-local triangle lists (§7) | MV, KC; OBS (all) | Confirmed |
| M10 | Detail levels with draw lists, and per-level collision meshes, follow the index buffers (§8–§9) | OBS (47,563 of 47,563, zero trailing bytes) | Confirmed. No published source describes this part. |
| M11 | The collision tree block ends with one extra byte only in records whose directory `stamp` ≥ 38 (§10.4) | DC; OBS (71,167 blocks, perfect split by stamp) | Confirmed |
| M12 | Z-up, right-handed, metres; characters face +Y; front faces counter-clockwise (§11) | MV, DS (Z-up); OBS (bone names, winding census, renders) | Confirmed |
| M13 | UV (0,0) is the first stored texel row/column; no V flip (§11.4) | MV; OBS (renders) | Confirmed |
| M14 | Surface list entries are 0x31 surface DIDs; draw entries select them by index (§12) | MV; OBS (112,910 references, all resolve) | Confirmed |

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
### 3.3 The `flags` word (values Confirmed, meaning partly OPEN)

| Value | Records | Notes |
|---|---|---|
| 0x00000001 | 27,960 | Static meshes |
| 0x00000002 | 20 | Static meshes (one or two submeshes, tangent formats); no structural difference found |
| 0x1000_00*nn* (bit 28 set), *nn* = 0x01 … 0x43 | 19,583 | Meshes meant for skinning |

Bit 28 does **not** decide the vertex layout. 1,317 records with bit 28 set have no submesh
with bone influences, and 783 records with flags = 1 have submeshes with influences. A
decoder takes the layout of each submesh from its own `vfmt` only, and a renderer treats a
submesh as skinned exactly when its `vfmt` influence count (§4.1) is non-zero.

> **OPEN:** The meaning of the low 16 bits when bit 28 is set (observed 1 … 0x43; they match
> neither the submesh count, the level count nor the highest bone index), and of value 2.
> Readers must not reject other values.

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
| 24–31 | 0xFF000000 | TANGENTS | 0x30 = tangent and binormal present; 0x00 = absent. No other value observed. | 24 if 0x30 |

Bits 0, 2, 3, 12–15 have never been observed set.

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

> **OPEN:** Meaning of the scalar (a wind-sway weight or height above ground would fit).
> Renderers ignore it until resolved.

<a id="s5-4"></a>
### 5.4 Colour (Confirmed layout, channel order Reported)

4 bytes. Byte 3 is alpha (Confirmed: across the 32,719 submeshes with COLOUR, 27,776 have
alpha 255 on every vertex). Bytes 0–2 are blue, green, red, as in a little-endian
`D3DCOLOR` (D3D, Reported). 7,968 coloured submeshes have one colour on every vertex.

> **OPEN:** How the original uses vertex colour (tint, baked occlusion, a wind or blend
> mask). `material.md` owns this question. A viewer may modulate the diffuse colour with it.

<a id="s5-5"></a>
### 5.5 UV sets (Confirmed)

Each set is 2 × f32 (u, v). Set 0 is the texture coordinate for the diffuse map.

- Values outside [0, 1] are normal and mean wrap (repeat) addressing: 104,300 of 112,910
  submeshes keep set 0 within [−16, 16], but 338 submeshes exceed ±100 (up to ±8 × 10⁷).
  Readers must not reject or clamp them.
- Origin and direction: §11.4.
- Sets 1 … 6 exist in 444 submeshes. In the samples inspected, set 1 was constant (0, 1).

> **OPEN:** What the second and later UV sets are used for (light map, detail map, dye
> mask). `material.md` decides which set each texture slot samples.

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
union of the AABBs of the submeshes drawn by its level 0 (§8). (Setups carry their own
physics bounds, `setup.md`.)

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
| 1 | `kind` | u32 | 1 = detail level, 2 = proxy level (§8.3). No other value. |
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
### 8.3 Level kinds (Confirmed shapes, use partly OPEN)

**Kind 1, detail levels.** Level 0 is the full-detail geometry; a viewer draws level 0 only.
Every kind-1 level carries a collision mesh (71,765 of 71,765). In 10,750 of the 12,787
records with two or more kind-1 levels, triangle counts never increase from one level to the
next. In the other 2,037, the last kind-1 level repeats the size of level 1 (for example
772, 324, 128, 324 triangles).

**Kind 2, proxy levels.** 4,173 levels in 2,644 records. They always come after all kind-1
levels. Their submeshes use the formats without tangents (0x112 or 0x152), mostly have 10–19
triangles (3,455 of 4,173 levels), and form simple boxes around the detail geometry (Confirmed visually on two wall
pieces). They never carry collision data. Their surfaces (106 distinct DIDs, most often
0x310003B4) also occur on ordinary submeshes.

> **OPEN:** At what distances the original switches between kind-1 levels (the record holds
> no switch distances; an entity property such as a render-LOD class probably selects them),
> what the repeated last kind-1 level is for (shadow casting is a candidate), and what
> proxy levels are used for (occlusion culling is a candidate: the client ships an
> occlusion-culling library). Renderers draw level 0 and ignore proxy levels until resolved.

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
| 6 | `build_byte` | u8 | Present if `has_code` = 1 **and** the record's directory `stamp` ≥ 38 (§10.4). Always 1. |

<a id="s10-2"></a>
### 10.2 What the block is (Reported analogy, partly Confirmed)

The block has the shape of a Havok "MOPP" bounding-volume code (AN): a quantisation origin
and scale followed by a byte program. Confirmed on 395 sampled blocks: `origin` is the
minimum corner of the level's collision vertices minus about 0.05 per axis, and 2²⁴ / `scale`
is 0.96 – 1.53 times the largest extent of those vertices. The first code byte is 0x28 in
95 % of blocks (then 0x27, 0x26). Tessera does not interpret the code; physics, if ever
needed, rebuilds its own acceleration structure from §9.

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

**Normative rule:** `build_byte` is present if and only if `stamp` ≥ 38.

**Fallback when no stamp is available** (fuzzing, payloads from other sources). This rule
agrees with the normative one on all 47,563 records:

1. If this is the last level, `build_byte` is present exactly when one byte remains in the
   record.
2. Otherwise, look at the next 8 bytes as two u32. If the first is 1 or 2 and the second is
   ≥ 1, they are the next level's `kind` and `draw_count`, and `build_byte` is absent.
   Otherwise `build_byte` is present.

(With the byte present, the next u32 would start with byte 0x01 followed by the 0x01 of
`kind`, giving 0x101 or larger, which is never a valid `kind`.)

> **OPEN:** Stamps 34–37 were not observed on meshes; the threshold between 33 and 38 is
> assumed. A reader meeting such a stamp should use the fallback.

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
Evidence: on two NPC setups whose skeletons name their bones, vertices dominated by bones
named `rwrist`, `rshoulder`, `rfoot` average x = +0.10 … +0.76, and the matching `l…` bones
average the same magnitude at negative x. A renderer whose world is right-handed uses
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
offset to the indices of `index_buffer`, and bind surface `surfaces[surface]`.

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
| **MI6** | Influence-list length = number of non-zero weights (0 when *K* = 0). |
| **MI7** | Index count multiple of 3; indices in [0, *V*−1]; min 0; max *V*−1; no degenerate triangles; every vertex used. |
| **MI8** | Draw entries: primitive 4; the three indices < *N* (and equal in all observed data); every submesh drawn exactly once over all levels. |
| **MI9** | Level kind ∈ {1, 2}; kind 2 levels come last; kind 1 ⇔ `has_collision_mesh` = 1. |
| **MI10** | Collision indices in range; vertex list strictly ascending; fA < fB when fB ≠ 0xFFFF. |
| **MI11** | `build_byte` presence follows §10.4. |
| **MI12** | The payload ends exactly after the last level (zero trailing bytes). |

MI10 does not hold literally in the 117 collision levels with more than 65,536 edges
(§9.2 OPEN): their u16 edge and face indices cannot address every element.

MI4, MI5, MI7 and MI10 are data-quality checks: a viewer may log a violation and still draw.
MI1–MI3, MI8, MI9 (structure), MI11 and MI12 are structural: violating them means the
decoder and the data disagree.

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
drawn as a white triangle (`setup.md` §8.5 OPEN), not a decoding error.

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

---

<a id="s17"></a>
## 17. Open questions

> **OPEN:** Low 16 bits of `flags` on skinned meshes, and flags value 2 (§3.3).

> **OPEN:** Meaning of the per-vertex scalar (§5.3).

> **OPEN:** Colour channel use; uses of UV sets 1 … 6 (§5.4, §5.5) — owned by `material.md`.

> **Resolved (2026-10-05):** the influence list is redundant: per vertex in reverse order, its
> slot-0 bone index once per non-zero weight (§6.2).

> **OPEN:** Level switch distances, the repeated last detail level, and the use of proxy
> levels (§8.3).

> **OPEN:** The `build_byte` threshold for stamps 34–37 (§10.4).

> **OPEN:** COLOUR2 (format bit 0x80) and tangent byte values other than 0x30 have never been
> seen; their sizes are not confirmed, so decoders reject them (§4.2).

> **OPEN:** Collision meshes with more than 65,536 triangles or edges: how u16 indices
> address elements beyond 65,535 (§9.2).

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

