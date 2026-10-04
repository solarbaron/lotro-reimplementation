<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Materials: surfaces, materials, shader records, texture slots, feature flags and dyes

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-04 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` or `crates/material` (decoders for prefixes 0x31, 0x30 and 0x2B), `crates/material` (effective parameter set, texture roles, feature flags, dye), `crates/render` (pipeline selection from feature flags), explorer material inspector and dye preview |
| **Companion specs** | `texture-list.md` (a texture slot names a 0x40 texture list; level selection and highres fallback). `records.md` §1 (primitives), §5 (0x41 surfaces). `dat-container.md` (routing). `mesh.md` (mesh record layout; only the surface list is used here). A future `appearance.md` (the 0x20 record grammar; only the parts needed for material binding are given here, §9.3). |

---

<a id="s0"></a>
## 0. Purpose

A mesh does not name a texture. Each submesh names a **render surface descriptor**
(prefix 0x31, called a *surface record* below to keep it apart from the 0x41 render
surfaces of `records.md` §5). A surface record names one **shader record** (0x2B) and an
ordered list of **materials** (0x30). The shader record declares a set of named
**parameters** with defaults; materials override some of them. Texture parameters hold
**texture lists** (0x40), which `texture-list.md` resolves to images.

This spec defines:

- the byte layout of surface records, materials and shader records, with a census over
  **all** records of each type (§3–§7);
- the parameter vocabulary: which slot is the diffuse, normal map, specular, emissive,
  opacity or dye mask, and which parameters follow (§8);
- how a submesh's effective parameter set is built, including appearance overrides (§9);
- how a texture slot's channel packing (what its alpha channel means) is derived from the
  shader record's own tables (§10);
- feature flags (alpha test with threshold, alpha blend, additive, two-sided, dyeable,
  normal-mapped, glow, ...) derived **without interpreting shader program bytecode** (§11);
- dye masks and per-item dye colour tables (§12).

Tessera never executes, translates or inspects the compiled shader programs stored in
shader records. It skips them by their size fields. Every fact used at run time comes from
the structured parts of the records (parameter declarations, binding tables, sampler
tables and render-state blocks). This settles `ARCHITECTURE.md` §12 OPEN and §25 item 9
(§11.1).

All counts are observations from one install on one date. Tests assert the invariants
(§14), not the counts.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded on the user's own install on 2026-10-04 by throwaway probes in
  the dirty-room scratchpad. A Confirmed fact states what was decoded and how much of it.
- **Inferred**: a meaning derived by the spec writer from consistent statistics in the
  data (co-occurrence, value ranges, names stored in the records). The layout is
  Confirmed; the meaning may still be wrong.
- **Reported**: from third-party material only.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). Every 0x31 (13,781), 0x30 (24,094) and 0x2B (510) record decoded to the last byte. Header and surface list of every mesh record (47,563) read. Every appearance record (prefix 0x20, 4,201) parsed with the grammar of DS §7.2 (4,199 clean). A raw scan of every other record of `client_general.dat` for DIDs that resolve to 0x30, 0x31 or 0x2B records. Four meshes rendered by a throwaway software rasteriser using the resolution rules of §9–§12 and inspected by eye (§13). |
| MV | Mirarkitty `lotro_mesh_viewer` docs (`textures.md`, `shaders.md`, `dyes.md`, `mesh-format.md`; MIT for docs, repository GPL-3.0). Facts only. |
| DS | `research/notes/data-semantics.md` §7.2–§7.5. |
| KC | gl_kilves `lotro-mesh-converter` (MIT): per-submesh vertex-format bytes in the mesh header. Facts only. |
| D3D | Public Direct3D 9 enumerations (`D3DBLEND`, `D3DBLENDOP`, `D3DCMPFUNC`, `D3DCULL`, `D3DTEXTUREADDRESS`, `D3DTEXTUREFILTERTYPE`). General references used to name observed small integers. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| MA1 | Surface record layout: DID, shader DID, binding key, material count, material DIDs, u16 (§4) | MV (single-material form only); OBS (13,781 of 13,781, zero leftover bytes) | Confirmed |
| MA2 | Every mesh lists exactly one surface record per submesh; submesh *i* uses entry *i* (§9.1) | KC (count); OBS (47,563 meshes; 112,910 references, all resolve; 4 meshes rendered) | Confirmed |
| MA3 | The mesh header does **not** list materials. MV's "T × 0x30 material DIDs" is a misreading of the first submesh's vertex-format bytes (§15) | OBS | Confirmed |
| MA4 | Parameter entry grammar shared by materials and shader records (§5) | OBS (99,935 material entries, 9,832 shader declarations) | Confirmed |
| MA5 | Material layout: DID, count, parameter entries (§6) | MV (slot list, "type 4" texture entries); OBS (24,094 of 24,094, zero leftover bytes) | Confirmed |
| MA6 | Shader record layout: DID, reserved, parameter declarations with binding tables, permutation list (§7) | OBS (510 of 510, zero leftover bytes) | Confirmed. Program blobs are opaque. |
| MA7 | Parameter id groups: 1000s float, 2000s colour, 3000s texture, 4000s flag (§8.1) | OBS (all entries) | Confirmed |
| MA8 | Texture-slot roles come from the sampler names that the shader record binds each slot to (§10) | OBS (78,326 texture bindings) | Confirmed (mechanism); role meanings Inferred |
| MA9 | Slot 3000 is the base colour (diffuse) texture | MV ("first 0x40 slot"); OBS (bound to `Color…`/`Diffuse…` samplers in every shader) | Confirmed. "First texture entry" is only a heuristic (§15). |
| MA10 | Effective parameters = shader defaults, then each material in order, later entries win (§9.2) | OBS (layering of 72 two-material surfaces; dye layers render correctly, §13) | Confirmed for layering; order of the few variant-like lists is OPEN |
| MA11 | Appearance material groups bind to submeshes by the surface record's binding key (§9.3) | DS §7.2 (group fields); OBS (46,885 of 47,895 groups match a key of the part meshes) | Confirmed |
| MA12 | Alpha test is enabled by flag 4004 and its threshold is float 1043 (§11.2) | MV (cutout shaders exist); OBS (always declared together, 307 shaders; 3,386 surfaces enabled; visual check) | Confirmed |
| MA13 | Dye colour is slot 2003 (and 2013 for a second dye channel) of a one-entry material layered by the appearance record's dye block (§12) | DS §7.5 (dye code, mask); OBS (2,180 dye entries; visual check) | Confirmed |
| MA14 | Dye colours are per appearance, not a global palette keyed by dye code (§12.4) | OBS (modal colour per code agrees in only 9,060 of 16,945 dye blocks) | Confirmed. Contradicts MV's single-palette model. |
| MA15 | Dye mask is the alpha channel of the texture whose sampler name carries "Dye" (`ColorAndDye`, `TSNormalAndDye`, `DyeAlpha`); low alpha = dyeable (§12.2) | DS §7.5, MV (diffuse alpha, low = dyeable); OBS (role table; visual check) | Confirmed (mask location, polarity). Exact blend is OPEN. |
| MA16 | Render-state block per permutation: blend factors, blend op, alpha compare, depth write, cull, alpha reference (§7.6) | OBS (31,519 blocks); D3D (enum values) | Layout Confirmed; meanings Inferred |
| MA17 | Shader records are compiled HLSL with no name field; classification by counting constant names in the blob | MV | Reported. Not used by Tessera (§11.1). |

---

<a id="s2"></a>
## 2. The chain at a glance

```
mesh 0x06 ─ surface list ─► surface record 0x31 (one per submesh, positional)
                               ├─ shader record 0x2B ── parameter declarations (defaults,
                               │                        bindings) + permutations (sampler
                               │                        tables, render-state blocks)
                               └─ materials 0x30 × n ── parameter overrides
                                       │
             appearance 0x20 group ────┘ (optional: replaces the shader, layers one more
             and dye block               material, then a dye material; matched by the
                                         surface record's binding key)

effective parameter set ── texture slots (0x40 lists) ──► texture-list.md ──► 0x41
                        ├─ colours, floats, flags
                        └─ + shader record tables ──► texture roles, feature flags, dye
```

A renderer needs, per submesh: the effective parameter set (§9), the role of each texture
slot (§10) and the feature flags (§11).

---

<a id="s3"></a>
## 3. Record classification by prefix

<a id="s3-1"></a>
### 3.1 Prefixes involved (Confirmed)

| Prefix | Name here | Archive (count) | Size (bytes) | Storage | Spec |
|---|---|---|---|---|---|
| 0x31 | surface record | general (13,781) | 18–30 | all uncompressed | §4 |
| 0x30 | material | general (24,091), local (3) | 8–1,058 | all uncompressed | §6 |
| 0x2B | shader record | general (510) | 1,155–1,567,860 (220.7 MB in all) | all uncompressed | §7 |
| 0x40 | texture list | general (81,135), local (1) | | | `texture-list.md` |
| 0x41 | render surface (image) | surface, highres families, local | | | `records.md` §5 |
| 0x06 | mesh | mesh (37,484), mesh_aux_1 (10,079) | | | `mesh.md` |
| 0x20 | appearance | general (4,201) | | | future `appearance.md`; §9.3 here |

The three local materials (`client_local_<Locale>.dat`) have DIDs that do not also exist in
general. Look in general first, then local (`dat-container.md` routing).

<a id="s3-2"></a>
### 3.2 Who references what (Confirmed)

| Target | Referenced by | Coverage |
|---|---|---|
| 0x31 surface records | Mesh surface lists (112,910 references, 12,028 distinct). Also found by a raw DID scan in setup records 0x01, and in prefixes 0x66, 0x39, 0x0E, 0x20 and others (2,226 distinct). | 353 surface records are referenced by neither (they may be referenced from cell or landblock archives, which were not scanned). |
| 0x30 materials | Surface records (13,861 references, 13,608 distinct). Appearance groups (83,984 references that resolve, 14,500 distinct). | Every one of the 24,094 materials is referenced by a surface record or an appearance group. |
| 0x2B shader records | Surface records (241 distinct). Appearance groups (136 distinct). Raw scan finds the rest in other record types. | All 510 are referenced by something in general. |
| 0x40 texture lists | Material texture entries (44,804 non-zero values, all resolve). Shader texture defaults (1,549, all resolve). | |

The raw-scan rows (a u32 that equals an existing DID of the target type) can include chance
matches. They show where else these records are used. They are not layouts.

---

<a id="s4"></a>
## 4. Surface record (prefix 0x31)

<a id="s4-1"></a>
### 4.1 Layout (Confirmed, 13,781 records, zero leftover bytes)

All integers are little-endian (`records.md` §1).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 4 | DID | `did` | The record's own DID (0x31…). Equals the DID looked up. |
| 4 | 4 | DID | `shader` | A shader record (0x2B…). Never zero. |
| 8 | 4 | u32 | `binding_key` | 1, 2, or a value of the form 0x1000xxxx. Matches appearance groups (§9.3). |
| 12 | 4 | u32 | `material_count` *n* | 0–3 observed |
| 16 | 4·*n* | DID × *n* | `materials` | Material DIDs (0x30…), applied in order (§9.2) |
| 16+4*n* | 2 | u16 | `tail` | 1 in 13,711 records, 0 in 69, 256 in 1 |

Total size = 18 + 4·*n*. A reader checks this exactly.

<a id="s4-2"></a>
### 4.2 Census (Confirmed)

| Field | Values (records) |
|---|---|
| `material_count` | 0 (6), 1 (13,691), 2 (82), 3 (2) |
| `binding_key` | 1 (6,976), 2 (8), 0x1000xxxx (6,797; 203 distinct values) |
| `shader` | 241 distinct; all resolve |
| `materials` | 13,861 references, all resolve; 13,608 distinct materials |
| `tail` | see above |

Most-used shader records by surface count: 0x2B0006EB (2,526), 0x2B0007A0 (1,794),
0x2B0007D1 (1,079), 0x2B0007D5 (960), 0x2B000700 (743).

Surface records with `material_count` = 0 use the shader's defaults only (§9.2).

> **OPEN:** The meaning of `tail`. It does not correlate with `material_count` or
> `binding_key` in an obvious way. Readers keep it and ignore it.

> **OPEN:** What `binding_key` 1 and 2 mean. Static world meshes use 1. Values 0x1000xxxx
> are shared with appearance keys (§9.3).

---

<a id="s5"></a>
## 5. Parameter entry (shared by materials and shader records)

Materials (§6) and shader record declarations (§7.2) use one grammar for a parameter.

<a id="s5-1"></a>
### 5.1 Layout (Confirmed)

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 4 | u32 | `id` | Parameter id, for example 3000. See §8. |
| 2 | 2 | u16 | `group` | Always ⌊`id` / 1000⌋ × 1000 (1000, 2000, 3000 or 4000) |
| 3 | 4 | u32 | `value_type` | 4, 0x10 or 0x40 (§5.2). Fixed per group. |
| 4 | varies | | `value` | By `value_type` (§5.2) |
| 5 | 4 | u32 | `binding_count` *b* | Always 0 in materials. In shader records, the binding table size (§7.3). |
| 6 | 1 | u8 | `flag` | 0 or 1, fixed per `id` (§8.6) |
| 7 | 12·*b* | | `bindings` | *b* binding entries (§7.3) |

<a id="s5-2"></a>
### 5.2 Value types (Confirmed)

| `value_type` | Group | Value layout | Size |
|---|---|---|---|
| 4 | 3000 (texture) | u32 texture-list DID (0x40…), or 0 for none | 4 |
| 4 | 4000 (flag) | u32, 0 or 1 in every observed entry | 4 |
| 0x10 | 2000 (colour) | four f32: R, G, B, A | 16 |
| 0x40 | 1000 (float) | a **float curve** (§5.3) | 8, 44 or 53 + 8·*k* |

No other `value_type` occurs. A reader rejects the record on an unknown `value_type`,
because the entry length is then unknown.

<a id="s5-3"></a>
### 5.3 Float curve (Confirmed layout, meaning partly OPEN)

| Field | Size | Meaning |
|---|---|---|
| `mode` | u32 | 1–10 observed |
| if `mode` = 1 | f32 | constant value |
| if 2 ≤ `mode` ≤ 9 | 10 × f32 | waveform parameters `w[0..9]` |
| if `mode` = 10 | 11 × f32, u8, u32 *k*, *k* × (f32, f32) | waveform parameters `w[0..10]`, a flag byte, then *k* keyframe pairs |

Material census: mode 1 (24,587), 2 (1,592), 3 (6), 4 (542), 5 (3), 6 (19), 7 (205),
8 (27), 9 (63), 10 (38). Shader defaults use modes 1 (4,123), 2 (48) and 4 (8).

Observations for modes 2–9: only `w[0]`, `w[2]`, `w[4]`, `w[6]` are commonly non-zero, and
`w[1]`, `w[3]`, `w[5]` rarely. Mode 2 is the most common form on texture-offset
parameters (§8.4), which suggests scrolling. For mode 10, `w[10]` ranges 0.25–18 (a period
in seconds is plausible), *k* is 2–10, and in 24 of 38 curves the second number of the
pairs runs from 0 to 100 (a percentage of the period is plausible).

Tessera rule for M2 (Decision): evaluate a mode-1 curve as its constant. Evaluate any other
mode as the constant `w[0]` (no animation).

> **OPEN:** The waveform shapes for modes 2–10, the meaning of `w[0..10]`, the mode-10 flag
> byte and the pair order. Needed only for animated materials (scrolling water, pulsing
> glow). A frame capture over time, or careful observation of a scrolling surface, can
> settle it.

---

<a id="s6"></a>
## 6. Material (prefix 0x30)

<a id="s6-1"></a>
### 6.1 Layout (Confirmed, 24,094 records, zero leftover bytes)

| Offset | Size | Type | Field |
|---|---|---|---|
| 0 | 4 | DID | `did` (0x30…), equals the DID looked up |
| 4 | 4 | u32 | `entry_count` *m* |
| 8 | varies | | *m* parameter entries (§5), each with `binding_count` = 0 |

A material is a **partial** parameter set: it lists only the parameters it changes.

<a id="s6-2"></a>
### 6.2 Census (Confirmed)

- `entry_count`: 0 (4 records of 8 bytes), 1 (4,353), 2 (2,638), 3 (2,554), 4 (5,856),
  5 (4,406), 6 (1,781), 7 (736), … up to 38.
- 99,935 entries: group 3000 (44,808), 1000 (27,082), 2000 (21,682), 4000 (6,363).
- No material repeats an `id`.
- Entries are **not** sorted by `id` (17,611 materials are out of order). Readers keep
  the stored order but must not depend on it: lookup is by `id`.
- Texture values: 44,804 resolve to 0x40 lists; 4 are 0.
- Every material is referenced by a surface record or an appearance group (§3.2).

---

<a id="s7"></a>
## 7. Shader record (prefix 0x2B)

A shader record is an effect: a parameter interface plus a list of compiled
**permutations** for several graphics back ends. Tessera reads the interface, the binding
tables, the sampler tables and the render-state blocks, and skips the program blobs.

<a id="s7-1"></a>
### 7.1 Top level (Confirmed, 510 records, zero leftover bytes)

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 4 | DID | `did` | The record's own DID |
| 2 | 4 | u32 | `reserved` | 0 in all 510 |
| 3 | 4 | u32 | `param_count` *p* | 0–62 |
| 4 | varies | | *p* parameter declarations | §7.2 |
| 5 | 4 | u32 | `permutation_count` *q* | 1–192 |
| 6 | varies | | *q* permutations | §7.4 |

<a id="s7-2"></a>
### 7.2 Parameter declarations (Confirmed)

Each declaration is a parameter entry (§5) whose `value` is the **default** and whose
binding table (§7.3) says which permutations use the parameter.

- 9,832 declarations: group 1000 (4,179), 4000 (3,066), 3000 (1,549), 2000 (1,038).
- 29 shader records declare no parameters.
- All 1,549 texture defaults are non-zero and resolve. Common defaults: 0x40000007
  (on slot 3000 in 438 records; its first non-highres level is a white image with a thin
  dark border), 0x4000000D (on 3011 in 224; a flat tangent-space normal), 0x40000008
  (white), 0x40001BF4 (black).

<a id="s7-3"></a>
### 7.3 Binding entry (Confirmed layout)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 2 | u16 | `semantic` | Small integer, see below |
| 2 | 2 | u16 | `group` | Always equals the declaration's `group` |
| 4 | 4 | u32 | `permutation` | Index into the permutation list; always < *q* (Confirmed, all) |
| 8 | 4 | u32 | `slot` | For texture parameters: index into that permutation's sampler table (§7.5); always in range (78,326 of 78,326). For other groups: 0–7, meaning OPEN. |

`semantic` is 10 for every texture binding. For other parameters it is fixed per `id` in
almost all records (for example 2000 → 0, 2001 → 1, 1000 → 2, 2014 → 3, 2003 and
2013 → 4, 4003 → 5, 4001 → 6, 4002 → 7, 4004 → 8, 1043 → 9, 4005 → 11, 4006 → 12; the
texture-transform floats use 33–44). Tessera does not need `semantic`.

Parameters with no binding entries exist: every flag-1 parameter (§8.6), parameter 4007 in
all 442 records that declare it, and a few others in a few records. They are still part of
the interface (they have defaults and materials set them).

> **OPEN:** The meaning of `slot` for non-texture bindings (0/1 for flags 4005/4006; 0–7
> for floats), and how unbound parameters reach the programs. Not needed by Tessera.

<a id="s7-4"></a>
### 7.4 Permutation (Confirmed layout)

| Order | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 1 | 16 | 4 × u32 | `key` | Permutation key, an opaque bit pattern (back end, light type, skinning, …) |
| 2 | 4 | u32 | `has_programs` | 0 or 1 |
| 3 | 4 | u32 | `program_kind` | Present only if `has_programs` = 1 |
| 4 | 4 + *a* | u32 *a*, *a* bytes | `program_a` | Present only if `has_programs` = 1. Opaque. |
| 5 | 4 + *b* | u32 *b*, *b* bytes | `program_b` | Present only if `has_programs` = 1. Opaque. |
| 6 | varies | | sampler table | §7.5 |
| 7 | varies | | extra state list | §7.7 |
| 8 | 39 | | render-state block | §7.6 |

Census over 31,519 permutations: `has_programs` = 0 in 541. `program_kind` 0 (27), 1
(6,918), 2 (7), 3 (7,041), 4 (16,985). (Inferred: the kinds are program back ends, kind 4
the newest. A record usually carries each pass once per back end, which is why
permutation counts come in multiples.) Tessera skips both blobs by their sizes and never
looks inside them.

<a id="s7-5"></a>
### 7.5 Sampler table (Confirmed layout)

| Field | Size | Meaning |
|---|---|---|
| `sampler_count` | u32 | 0–10 observed |
| per sampler: `name_length` | u8 | 0–28 observed |
| per sampler: `name` | `name_length` bytes, ASCII | Sampler name, may be empty |
| per sampler: `state` | 40 bytes | Sampler state (below) |

105,119 sampler entries. Sampler `state` bytes, with their observed values (Inferred names
from D3D):

| Byte | Values (entries) | Inferred meaning |
|---|---|---|
| 8 | 1 (69,351), 3 (32,648), 4 (3,120) | Address mode U: 1 wrap, 3 clamp, 4 border |
| 9 | 1 (69,323), 3 (32,676), 4 (3,120) | Address mode V, same codes |
| 10 | 2 (90,956), 1 (14,114), 3 (49) | Magnification filter: 1 point, 2 linear, 3 anisotropic |
| 11 | same as byte 10 | Minification filter |
| 12 | 2 (82,254), 1 (22,786), 0 (79) | Mip filter: 0 none, 1 point, 2 linear |
| 4 | 0 mostly; 2, 5, 9, 10, 11 on engine-supplied samplers | OPEN |
| others | almost constant | OPEN |

For a given shader record and texture slot, every permutation that binds the slot uses the
same address and filter bytes (1,548 of 1,549 slots).

<a id="s7-6"></a>
### 7.6 Render-state block (Confirmed layout, Inferred meanings)

39 bytes. Census over all 31,519 blocks:

| Bytes | Field | Values (blocks) | Inferred meaning |
|---|---|---|---|
| 0 | `src_blend` | 5 (29,976), 2 (1,502), 1 (21), 9 (18), 7 (2) | D3DBLEND: 1 ZERO, 2 ONE, 5 SRCALPHA, 7 DESTALPHA, 9 DESTCOLOR |
| 1 | `dst_blend` | 6 (19,895), 2 (10,125), 1 (1,484), 3 (7), 4 (4), 5 (4) | D3DBLEND: 6 INVSRCALPHA, 3 SRCCOLOR, 4 INVSRCCOLOR |
| 2 | `blend_op` | 1 (31,516), 3 (3) | D3DBLENDOP: 1 ADD, 3 REVSUBTRACT |
| 3 | `alpha_cmp` | 7 (31,331), 8 (188) | D3DCMPFUNC: 7 GREATEREQUAL, 8 ALWAYS |
| 4 | `depth_write` | 1 (20,303), 0 (11,216) | Depth write enable (see below) |
| 5 | `cull` | 2 (31,169), 1 (350) | D3DCULL: 2 CW (back faces culled), 1 NONE (two-sided) |
| 6 | `b6` | 0 (30,993), 1 (526) | OPEN |
| 7–10 | u32 | 1 in all | OPEN |
| 11–14 | f32 `alpha_ref` | 0.9 (16,614), 0.8 (14,828), 0.1 (50), 0.6 (20), 0.0 (7) | Default alpha-test reference |
| 15–22 | 8 bytes | 0xFF except in 3 blocks | OPEN (mask-like) |
| 23–26 | u32 | 1 in all | OPEN |
| 27–33 | 7 bytes | 0 in all | |
| 34–38 | 5 bytes | 0xFF in all | |

Why byte 4 is read as depth write and not as blend enable: the permutations that are
clearly additive light passes (src SRCALPHA, dst ONE) have byte 4 = 0, which fits "do not
write depth" but not "blending off". The unlit additive shaders (src SRCALPHA, dst ONE)
have byte 4 = 1, which would make them opaque if byte 4 were "blending off".

> **OPEN (frame capture):** Confirm the meanings of bytes 4, 5 and 6, and whether blending
> is ever disabled independently of the factors. A capture of one lit object, one additive
> effect and one alpha-blended effect settles it.

<a id="s7-7"></a>
### 7.7 Extra state list (Confirmed layout)

| Field | Size | Meaning |
|---|---|---|
| `item_count` | u32 | 0 (18,633 permutations), 1 (8,322), 2 (4,526), 3 (5), 4 (1), 6 (29), 8 (3) |
| per item: `item_type` | u8 | 1, 2, 3 or 5 |
| per item: `stage` | u32 | small integer |
| per item: arrays | *c*(`item_type`) × (u32 *n*, *n* × 4 bytes) | *c*(1) = 2, *c*(2) = 1, *c*(3) = 2, *c*(5) = 7 |

Every one of the 121,965 arrays has *n* = 1. Item type 5 (17,370 items) holds seven arrays
of a 1 and a 0. Type 3 items hold values 1.0.

A reader must reject an unknown `item_type`, because its length is unknown.

> **OPEN:** The meaning of the extra state items. Tessera skips them.

---

<a id="s8"></a>
## 8. Parameter vocabulary

<a id="s8-1"></a>
### 8.1 Groups (Confirmed)

| Group | Ids observed | Value | Purpose |
|---|---|---|---|
| 1000 | 1000–1207 | float curve | Scalars: specular power, alpha-test threshold, texture transforms, effect parameters |
| 2000 | 2000–2014 | RGBA | Colours: diffuse tint, specular, dye, glow, water |
| 3000 | 3000–3079 | texture-list DID | Texture slots |
| 4000 | 4001–4007 | 0/1 | Feature flags |

The meaning of an id is the same in every shader record that declares it (§8.2–§8.5 list
the evidence). Implementations key everything on the `id`, never on entry order.

<a id="s8-2"></a>
### 8.2 Texture slots

Role names are the sampler names that the shader records bind the slot to (§10). "Shaders"
counts records declaring the slot; "surfaces" counts surface records whose shader declares
it.

| Id | Role (sampler names seen, most common first) | Shaders | Surfaces | Mark |
|---|---|---|---|---|
| 3000 | **Base colour (diffuse).** `ColorAndOpacity`, `ColorAndGloss`, `Color`, `ColorAndDye`; older back ends also use `DiffuseTexture`, `DiffuseMap` | 467 | 13,760 | Confirmed |
| 3001 | Second base-colour layer (`ColorAndOpacity2`, `ColorAndGloss2`, `Color2`, `ColorAndDye2`), or a baked shadow map (`BakedShadow`) | 277 | 770 | Confirmed |
| 3002 | Third colour layer (`DiffuseTexture3`) | 1 | 0 | Confirmed |
| 3011 | **Normal map**, tangent space (`TSNormal`, `TSNormalAndGloss`, `TSNormalAndDye`, `NormalTexture`) | 252 | 9,296 | Confirmed |
| 3013 | Glow mask (`Glow`) | 142 | 572 | Confirmed |
| 3015 | Environment map (`Environment`, `EnvironmentCube`) | 20 | 504 | Confirmed |
| 3017 | Vertical palette (`VerticalPaletteTexture`) | 1 | 3 | Confirmed |
| 3018 | **Emissive** (`Emissive`, `EmissiveAndOpacity`) | 139 | 541 | Confirmed |
| 3019 | Baked lighting (`BakedLight`, `DirLightMaskAndAmbOcc`) | 98 | 6 | Confirmed |
| 3020 | Separate opacity mask (`OpacityAlpha`) | 106 | 133 | Confirmed |
| 3021 | Separate gloss mask (`GlossAlpha`) | 4 | 4 | Confirmed |
| 3024 | Separate dye mask (`DyeAlpha`) | 22 | 829 | Confirmed |
| 3026 | Distortion (`Distortion`) | 1 | 4 | Confirmed |
| 3075 | Flow map (`FlowTexture`) | 3 | 27 | Confirmed |
| 3076 | Tie-dye colour ramp (`TieDyeColor`) | 2 | 21 | Confirmed |
| 3077 | Sphere map (`SphereMap`) | 3 | 6 | Confirmed |
| 3078 | Second normal map (`TSNormal2`, `TSNormalAndGloss2`) | 9 | 53 | Confirmed |
| 3079 | Foam (`FoamTexture`) | 2 | 22 | Confirmed |

There is no separate specular-colour texture slot. Specular strength comes from the alpha
of a `…AndGloss` texture or from `GlossAlpha` (§10.2), scaled by colour 2001 and power
1000 (§8.3).

<a id="s8-3"></a>
### 8.3 Colours and common scalars (Inferred)

The evidence is: which shader records declare the id, compared with which records contain
a named program constant (§1, OBS); the binding `semantic`; and value ranges in materials.

| Id | Meaning | Evidence | Typical values |
|---|---|---|---|
| 2000 | Material diffuse colour (tint) | Declared in 477 records; tracks the diffuse-colour constant | white |
| 2001 | Specular colour | Always declared together with 1000 (311 records) | greys |
| 1000 | Specular power | Tracks the specular-power constant (Jaccard 0.94); values 25, 30, 10, 15, 20, 45 | 10–45 |
| 2003 | **Dye colour** | Tracks the dye-colour constant (41 of 42 records) | RGBA, A = 1 in 6,493 of 6,553 |
| 2013 | **Second dye colour** | Tracks the second dye-colour constant (10 of 11) | RGBA |
| 2014 | Glow colour | Tracks the glow-colour constant (151 of 151) | |
| 1043 | **Alpha-test threshold** | Always declared with 4004 (307 = 307); tracks the alpha-test constant | 0.8 (most), 0.65, 0.5, 0.7, 0.6 |
| 1174 | Soft-particle depth fade factor | Declared by 31 unlit effect shaders only | |

<a id="s8-4"></a>
### 8.4 Texture-coordinate transforms (Inferred)

Floats that form a texture matrix for texture-coordinate set 0, 1 or 2. Grouping is
Inferred from the binding `semantic` values and from co-declaration with numbered
texture-matrix constants.

| Set 0 | Set 1 | Set 2 | Meaning | Typical values |
|---|---|---|---|---|
| 1001, 1002 | 1003, 1004 | 1005, 1006 | Scale (U, V) | 1, 2, 3, 0.25 |
| 1017, 1018 | 1019, 1020 | 1021, 1022 | Offset (U, V); often an animated curve | mode 2 |
| 1033 | 1034 | 1035 | Rotation, degrees | 0, 30, 90, 180, −30 |
| 1070, 1071 | 1072, 1073 | 1074, 1075 | Rotation centre (U, V) | −0.5 |

> **OPEN:** Which of each pair is U and which is V, the composition order (scale, rotate,
> offset), and which texture slots use which coordinate set (likely set 0 for slot 3000
> and set 1 for slot 3001). Needs a capture or a careful visual comparison with the
> original client.

<a id="s8-5"></a>
### 8.5 Flags (group 4000)

| Id | Meaning | Evidence | Mark |
|---|---|---|---|
| 4004 | **Alpha test enable** | Declared exactly together with 1043. Effective value 1 on 3,059 of 3,212 surface records whose shader is lit (§11.3) with a `ColorAndOpacity` diffuse; 0 on 1,546 of 1,578 unlit ones (those blend instead) | Confirmed (by co-declaration and visual check, §13) |
| 4007 | Never bound to any permutation (442 records declare it). Effective value 1 on 590 of 3,212 lit `ColorAndOpacity` surface records (cut-out foliage, grilles, cloth edges), but only on 19 of 2,457 `ColorAndDye`, 10 of 4,322 `ColorAndGloss` and 2 of 1,647 `Color` ones | Candidate for **two-sided** | OPEN |
| 4002 | Bound only in base passes, never in additive light passes; tracks the fog-colour constant (Jaccard 0.79) | Candidate for "receives distance fog" | OPEN |
| 4001, 4003, 4005, 4006 | Declared by almost every lit shader | none conclusive | OPEN |

Values are 0 or 1. The defaults come from the shader record.

<a id="s8-6"></a>
### 8.6 The per-entry `flag` byte (Confirmed values, meaning OPEN)

`flag` is fixed per `id`. It is 1 for the effect-specific parameters (water ids
1044–1069, 1170–1190, 2004–2011; mirror 1100–1106; window 1120–1127; distortion
1140–1160; depth fade 1174; tie-dye 1175–1177; wind 1178–1180; vertex displacement
1191–1199; snow and triplanar 1200–1202; 1203; anisotropic specular 1204–1207; second dye
colour 2013) and 0 for every other id. None of the flag-1 parameters has binding entries.

> **OPEN:** Meaning of `flag` (plausibly "effect-specific parameter, bound by name").

<a id="s8-7"></a>
### 8.7 Effect-specific parameters

Ids from 1044 up (except 1070–1075) and 2004–2011 belong to special effect families:
water, mirrors, windows, heat distortion, wind sway, vertex displacement, snow blending,
anisotropic hair specular. They occur on few surfaces (water: 98 surfaces). M2 renders
these surfaces with the generic pipeline chosen by §11 and ignores the extra parameters.

> **OPEN:** Per-parameter meanings of the effect families. Needed for M3 (water, wind) and
> later.

---

<a id="s9"></a>
## 9. Resolving a submesh's material

<a id="s9-1"></a>
### 9.1 Submesh → surface record (Confirmed)

The mesh record (`mesh.md`) starts with its DID, a u32 flags word, a u32 count *S*, *S*
surface-record DIDs, and then a u32 submesh count *N*. In all 47,563 meshes *S* = *N*, and
**submesh *i* uses surface record *i***. A mesh may name the same surface record for several
submeshes (13,398 meshes do).

This was checked visually on a building mesh with 8 submeshes and 8 different surface
records (§13): each submesh got the expected texture.

<a id="s9-2"></a>
### 9.2 Effective parameter set (normative)

Given a shader record *H* and an ordered list of materials *L*:

1. Start with *H*'s declarations: for each declared `id`, the value is the declared
   default.
2. For each material in *L*, in order: for each entry whose `id` is declared by *H*, set the
   value to the entry's value. A later material replaces an earlier one's value.
3. Entries whose `id` *H* does not declare are ignored (they have no effect on rendering).
   They are kept for the inspector.

For a plain submesh, *H* = the surface record's `shader` and *L* = its `materials`.

Evidence: in 13,215 of the 13,861 (surface, material) pairs, every material `id` is declared
by the surface's shader; the rest carry a few extra ids (3011, 3001, 2003, …). The value
types of declared ids and material entries never disagree (0 mismatches).

Of the 84 surface records with several materials, 72 start with the same generic material
(alpha test on, threshold 0.8, four flags) followed by a specific one that sets textures and
sometimes a different threshold. Last-wins gives the specific threshold, which is the
natural reading. Four records instead list two or three materials that each set a
different slot-3000 texture.

> **OPEN:** For the few surface records whose materials set conflicting textures, whether
> the client uses the last, the first, or selects one by some state (season, damage,
> variation). Tessera uses last-wins.

<a id="s9-3"></a>
### 9.3 Appearance overrides (Confirmed)

Worn items, mounts and character parts are assembled from appearance records (0x20). Their
grammar belongs to a future `appearance.md` (Reported in DS §7.2; this probe parsed 4,199 of
4,201 records with it). The part relevant here: a **block** carries a list of **material
groups**, and each group has five fields:

| Field | Type | Meaning |
|---|---|---|
| `shader_override` | DID or 0 | 0 in 62,110 groups; a 0x2B DID in 22,151 |
| `material` | DID | A 0x30 material (83,984 of 84,261 resolve) |
| `a` | u32 | 2 or a 0x1000xxxx key. OPEN. |
| `binding_key` | u32 | Matched against surface records |
| `g` | f32 | OPEN |

Binding rule: a group applies to every submesh, among the block's part meshes, whose surface
record has `binding_key` equal to the group's `binding_key`. That holds for 46,885 of the
47,895 groups that come with part meshes (97.9%). For such a submesh:

- *H* = `shader_override` if non-zero, else the surface record's shader;
- *L* = the surface record's materials, then the group's material, then any dye material
  (§12.3).

The surface record's own materials are the mesh's default look. An appearance group
layered on top gives the item's look.

> **OPEN:** The 1,010 groups whose key matches no surface of the block's parts (they may
> target parts added by another entry), and the fields `a` and `g`.

---

<a id="s10"></a>
## 10. Texture roles and channel packing

<a id="s10-1"></a>
### 10.1 Deriving a slot's role (normative)

For a shader record *H* and a texture `id` that *H* declares:

1. Collect the names of the samplers that *H*'s binding entries for that `id` point to:
   for each binding, take permutation `permutation`, sampler table entry `slot`, and its
   `name`. Ignore empty names.
2. The **role name** is chosen from that collection in this order: the most frequent name
   that begins with `ColorAnd`, `TSNormalAnd` or `EmissiveAnd`; otherwise the most frequent
   name that is exactly one of `Color`, `TSNormal`, `Emissive`, `OpacityAlpha`, `DyeAlpha`,
   `GlossAlpha`, `Glow`, `Environment`; otherwise the most frequent name; otherwise none.

Step 2 is needed because older back ends use generic names (`DiffuseTexture`, `DiffuseMap`)
for the same slot that newer back ends name by packing (`ColorAndOpacity`).

Eight declared texture slots (in eight records) have no named binding. Their role is
"unknown"; they are sampled as plain colour.

<a id="s10-2"></a>
### 10.2 What the channels mean (Inferred from the names; alpha meanings checked visually)

| Role name | RGB | Alpha |
|---|---|---|
| `Color`, `Color2`, `DiffuseTexture`, `DiffuseMap` (as the chosen role) | base colour | not used |
| `ColorAndOpacity`, `ColorAndOpacity2` | base colour | **opacity** (cut-out or blend, §11) |
| `ColorAndGloss`, `ColorAndGloss2` | base colour | **gloss** (specular mask) |
| `ColorAndDye`, `ColorAndDye2` | base colour, dyeable area stored near grey | **dye mask**, low = dyeable (§12) |
| `TSNormal`, `TSNormal2`, `NormalTexture` | tangent-space normal | not used |
| `TSNormalAndGloss`, `TSNormalAndGloss2` | tangent-space normal | gloss |
| `TSNormalAndDye` | tangent-space normal | dye mask |
| `Emissive` | emissive colour | not used |
| `EmissiveAndOpacity` | emissive colour | opacity |
| `OpacityAlpha` | not used | opacity |
| `GlossAlpha` | not used | gloss |
| `DyeAlpha` | not used | dye mask |
| `Glow` | glow mask | |
| `Environment`, `EnvironmentCube`, `SphereMap` | reflection | |
| `BakedShadow`, `BakedLight`, `DirLightMaskAndAmbOcc` | precomputed lighting | |

Diffuse role census over surface records (slot 3000): `ColorAndOpacity` 4,790 (211
shaders), `ColorAndGloss` 4,322 (83), `ColorAndDye` 2,457 (29), `Color` 2,131 (124),
other names 60. Normal maps (3011): `TSNormal` 4,616, `TSNormalAndGloss` 4,493,
`NormalTexture` 138, `TSNormalAndDye` 37.

Textures sample with the address mode and filter of §7.5. UVs are used as stored, with no
V flip (confirmed visually: MV agrees).

> **OPEN:** Whether normal maps are stored with a flipped green channel, and the normal-map
> swizzle in general. A capture or a close comparison of lighting on a relief texture.

---

<a id="s11"></a>
## 11. Feature flags

<a id="s11-1"></a>
### 11.1 Method (Decision)

Feature flags come from three structured sources only:

1. the effective parameter set (§9.2),
2. the texture roles (§10),
3. the render-state blocks of the shader record's permutations (§7.6).

Tessera does **not** count constant names inside program blobs (MV's method). That method
is not needed: every feature it detects (alpha test, dye, metallic/specular) is visible in
the parameter interface (declared ids 4004/1043, 2003, 1000/2001) and the sampler names.
In this dirty room, the constant names were used once, only as evidence for the meanings in
§8.3. Implementations never read them.

<a id="s11-2"></a>
### 11.2 Alpha test (Confirmed)

- **Enabled** when *H* declares 4004 and the effective 4004 = 1, **and** the slot roles
  provide an opacity source (below).
- **Threshold** = the effective 1043 (§5.3 evaluation). A fragment is kept when opacity ≥
  threshold. (`alpha_cmp` = GREATEREQUAL in 31,331 of 31,519 render-state blocks.) The
  render-state `alpha_ref` (0.8 or 0.9) is the shader's built-in default and is overridden
  by 1043.
- **Opacity source**, first match: alpha of slot 3000 if its role contains `Opacity`;
  else alpha of slot 3020 (`OpacityAlpha`); else alpha of slot 3018 if its role is
  `EmissiveAndOpacity`.

Census over surface records (their own materials only): the shader declares 4004 for
5,213; alpha test is on for 3,386 and off for 1,827. Thresholds of enabled surfaces: 0.8
(2,446), 0.65 (187), 0.5 (177), 0.7 (111), 0.6 (106), 0.4 (67), 0.3 (59), 0.9 (42), others.
Opacity source: slot 3000 (3,091), slot 3020 (130), slot 3018 only (2), none (163).

142 of the 163 "no source" surfaces use one shader record whose diffuse is `ColorAndDye`
(MV classifies that shader as opaque). Testing the dye mask as opacity would punch holes in
the dyeable cloth.

Rule (Decision): with no opacity source, alpha test is off.

> **OPEN (frame capture):** What the original does with alpha test enabled but no opacity
> texture: off, or testing some other channel. Also whether the client applies alpha to
> coverage or a softened edge.

<a id="s11-3"></a>
### 11.3 Blend mode (layout Confirmed, rule Inferred)

Classify the shader record first:

- **Lit**: at least one permutation's render state has `src_blend` = 5, `dst_blend` = 2
  and `depth_write` = 0 (additive per-light passes). 158 records, 11,639 surface records.
- **Unlit**: all others. 352 records, 2,142 surface records.

| Family | (src, dst) pairs in its render-state blocks | Tessera blend mode | Records (surface records) |
|---|---|---|---|
| Lit | base passes SRCALPHA / INVSRCALPHA with depth write on, plus the light passes | **Opaque**, or **cut-out** when alpha test is on (§11.2) | 158 (11,639) |
| Unlit | only ONE / ZERO | Opaque | 17 (2) |
| Unlit | only SRCALPHA / INVSRCALPHA | **Alpha blend**: colour × opacity + destination × (1 − opacity) | 79 (1,266) |
| Unlit | only SRCALPHA / ONE | **Additive**: colour × opacity + destination | 31 (719) |
| Unlit | ONE / ZERO and SRCALPHA / INVSRCALPHA mixed (water and many records used only outside surface records) | the most common pair | 204 (98) |
| Unlit | other pairs: DESTCOLOR / ONE, ZERO / SRCCOLOR, ONE / ONE, ZERO / ONE with SRCALPHA / INVSRCALPHA, … | the most common pair | 21 (57) |

Rule for unlit records (Decision): take the most common (src, dst, op) triple among the
record's render-state blocks and use it as the fixed-function blend state, mapping the D3D
factor codes of §7.6 to the equivalent wgpu blend factors. "Opacity" in a factor is the
opacity source of §11.2, or 1 if there is none. Depth write follows `depth_write` of the
same block. Blended surfaces are drawn in the transparent pass, sorted back to front
(`ARCHITECTURE.md` §13).

Many shader records are used only through other record types (terrain, effects,
appearances), which is why some rows have few surface records.

Why lit base passes are not treated as blended: in lit shaders the alpha of the diffuse is
often a dye mask or gloss (§10.2), and those surfaces are opaque in the original (MV,
visual checks §13). The SRCALPHA / INVSRCALPHA factors there therefore cannot be blending
by texture alpha. The likely reading is that the programs output an alpha of 1 except at
cut-out edges.

> **OPEN (frame capture):** Confirm that lit objects are drawn without blending (or with
> output alpha 1), and whether any lit surface is translucent (glass, ghosts). 153 surface
> records with a lit `ColorAndOpacity` shader have alpha test off; Tessera draws them opaque,
> but their look in the original is unknown.

<a id="s11-4"></a>
### 11.4 Two-sided (partly OPEN)

- If the most common `cull` value among *H*'s render-state blocks is 1 (NONE), the surface
  is two-sided. That holds for 38 records (120 surface records, mostly water and unlit
  effects); 350 blocks in all have `cull` = 1.
- Otherwise back faces are culled. Front faces are counter-clockwise in a right-handed
  frame: the triangle normal (p1 − p0) × (p2 − p0) agrees with the stored vertex normals for
  99.4% of 361,885 sampled triangles (cross-check with `mesh.md`).

Lit shaders always have `cull` = 2 in their base passes, yet foliage cards need two sides.
Flag 4007 is the leading candidate (§8.5).

Rule (Decision for M2, revisitable): effective 4007 = 1 also makes a surface two-sided.

> **OPEN (frame capture):** Whether 4007 is the two-sided flag. A capture of a leaf card
> seen from behind settles it. The data cannot: meshes contain no back-facing duplicate
> triangles either way.

<a id="s11-5"></a>
### 11.5 Other flags (Confirmed from the interface)

| Flag | Rule | Scale (shader records declaring the ids; surface records using them) |
|---|---|---|
| `normal_mapped` | effective 3011 non-zero and role is a normal role | 252; 9,296 |
| `specular` | *H* declares 1000; gloss from the role table (§10.2), colour 2001, power 1000 | 311 |
| `dyeable` | role of 3000 or 3011 contains `Dye`, or *H* declares 3024 (`DyeAlpha`); and *H* declares 2003 | §12 |
| `dyeable_2` | *H* declares 2013 | 11 |
| `emissive` | effective 3018 non-zero | 139; 541 |
| `glow` | *H* declares 2014 (with 3013) | 151 |
| `environment` | effective 3015 non-zero | 20; 504 |
| `second_layer` | effective 3001 non-zero and its role is a colour role | 277; 770 |
| `baked_lighting` | 3019 or a `BakedShadow` role on 3001 | rare on surface records |
| `uv_transform` | any of the §8.4 ids declared | |
| `animated` | any effective float curve with `mode` ≠ 1 | |

"Metallic" in MV's sense (a separate specular-metallic constant) has no material
parameter. Tessera treats it as specular with an environment map when 3015 is present.

> **OPEN:** How the original combines gloss, specular colour 2001 and power 1000, and how
> it uses the environment map. M2 uses a plain Blinn-Phong term: gloss × 2001 ×
> (N·H)^1000.

---

<a id="s12"></a>
## 12. Dyes

<a id="s12-1"></a>
### 12.1 Model (Confirmed)

A dye is not a colour stored in the item. It is a **dye code** (a float, 0.01 steps, DS
§7.5) that selects a **block** of the item's appearance record. That block layers a
one-entry material whose only parameter is the dye colour 2003 (or 2013 for the second
channel). The shader multiplies the dyeable area of the base texture by that colour.

<a id="s12-2"></a>
### 12.2 Dye mask (Confirmed location and polarity)

The mask is the alpha channel of the texture whose role name contains `Dye`:

| Role | Mask | Shader records (surfaces) |
|---|---|---|
| `ColorAndDye` on 3000 | diffuse alpha | 29 (2,457) |
| `TSNormalAndDye` on 3011 | normal-map alpha (the diffuse alpha then carries opacity) | 5 (37) |
| `DyeAlpha` on 3024 | separate texture | 22 (829) |
| `ColorAndDye2` on 3001 | second layer alpha | 1 |

Low alpha marks dyeable texels (DS §7.5, MV, and this probe's render §13). Over 25 random
`ColorAndDye` diffuse textures, alpha is strongly bimodal: 23% of texels in the lowest
eighth and 62% in the highest. Some textures have a sizeable share of mid values (up to
72%), so the mask is not purely binary.

<a id="s12-3"></a>
### 12.3 Applying a dye (normative for Tessera)

Given the dye mask value *a* (0–1), base colour *c* and dye colour *d* = effective 2003 RGB:

- dye weight *w* = 1 − *a*;
- dyed colour = *c* × (1 − *w*) + (*c* × *d*) × *w*.

For a second channel (2013), the same rule applies with the mask from the second dye-mask
source.

> **OPEN (frame capture):** The exact operation. DS §7.5 reports a hard threshold (alpha <
> 128 ⇒ multiply). Tessera uses the smooth form above, which equals the threshold form on
> bimodal masks. Also unknown: the role of the dye colour's alpha (1.0 in 6,493 of 6,553
> values; 0.45 in 49), whether the multiply is in linear or gamma space, and any brightness
> boost of the grey base.

<a id="s12-4"></a>
### 12.4 Dye colour tables are per appearance (Confirmed)

Appearance entries that carry no part meshes but only material groups hold the dye
variants. Each block's `q` is the dye code; blocks are sorted by ascending `q` in every
entry.

| Entry content | Entries |
|---|---|
| Every block's material sets only 2003 | 447 |
| … only 2013 | 36 |
| … only 2003 and 2013 | 67 |
| Material-only entries that set other parameters (texture swaps per dye, specular, …) | 2,624 |

The dye colour for a given code differs between items: for each code, the most common
colour accounts for only 9,060 of 16,945 dye blocks (53%). A global "dye code → RGB" palette
is therefore wrong. The colour must come from the item's own blocks. (MV's palette, taken
from wiki swatches, is not used.)

Selecting a block for a dye code *x* (Decision): the block whose `q` is within 0.005 of *x*;
otherwise the block with the largest `q` ≤ *x*; otherwise the first block. Code 0.0 or "no
dye" means: do not layer a dye material, so the shader default 2003 (white) applies.

> **OPEN:** The client's rule when a code has no block, and where an item's default dye
> code comes from (DS mentions an item property naming the default dye). Belongs with
> `appearance.md`.

<a id="s12-5"></a>
### 12.5 Tie-dye

Two shader records bind a `TieDyeColor` ramp (3076) with parameters 1175–1177 (21 surface
records). M2 treats them as `ColorAndDye` with the ramp ignored.

> **OPEN:** How the tie-dye ramp is indexed.

---

<a id="s13"></a>
## 13. Visual verification performed (dirty room, images not kept in the repo)

A throwaway software rasteriser drew meshes using only the rules of §9–§12: raw V, Z-up,
nearest sampling, Lambert shading, alpha test with the effective threshold, dye by the
threshold form.

| Mesh | What it exercises | Result |
|---|---|---|
| 0x0600220D | one surface record over 3 submeshes, `ColorAndGloss` | pass: stone texture and trim aligned |
| 0x06000C37 | `ColorAndOpacity`, alpha test 0.8 (tree) | pass: twigs cut out cleanly |
| 0x06011305 | 8 submeshes, 8 surface records, two diffuse roles, grilles with threshold 0.4 | pass: every part textured correctly, grilles see-through |
| 0x06002E60 + appearance 0x20002727 | appearance shader override, layered material, dye layers at three codes | pass: only the coat changes colour; tack and saddle keep theirs |
| culling test | wedge mesh drawn with each winding culled | counter-clockwise (right-handed) is front |

---

<a id="s14"></a>
## 14. Invariants

Real-data tests assert these. Failures are reported per record and do not stop a census.

| # | Invariant | Status |
|---|---|---|
| MT1 | 0x31 size = 18 + 4·`material_count`; `shader` has prefix 0x2B and resolves; every material resolves | Confirmed, all |
| MT2 | 0x30: entries parse to exactly the record end; `group` = ⌊id/1000⌋×1000; `value_type` fixed per group (4, 0x10, 0x40, 4); `binding_count` = 0; no repeated id | Confirmed, all |
| MT3 | Float curve `mode` ∈ 1–10 | Confirmed, all (reject other modes: unknown length) |
| MT4 | 0x2B: `reserved` = 0; parse to exactly the record end; every binding's `permutation` < count; every texture binding's `slot` < that permutation's sampler count | Confirmed, all |
| MT5 | Texture values are 0 or resolve to 0x40 | Confirmed, all (44,804 + 4 zeros; 1,549 defaults) |
| MT6 | Mesh surface count = submesh count | Confirmed, all 47,563 |
| MT7 | A material's value type for an id equals the declaring shader's | Confirmed, all pairs |
| MT8 | 4004 and 1043 are declared together | Confirmed, 307 of 307 records |
| MT9 | `has_programs` ∈ {0, 1}; extra item types ∈ {1, 2, 3, 5} | Confirmed, all (reject others: unknown length) |

Hostile-input rules: every count is checked against the bytes left before allocating;
string lengths are a u8; program blob sizes must fit in the record.

---

<a id="s15"></a>
## 15. Discrepancies with the research notes and third-party docs

| Claim | Source | Finding |
|---|---|---|
| Mesh header has `u32 T` and *T* × 0x30 material DIDs after the surface list | DS §7.3, MV | No. The u32 after the surface list is the submesh count. The next four bytes are the first submesh's vertex-format bytes, whose last byte is 0x30 in 47,562 meshes. Read as a u32 they look like a 0x30 DID. Materials come only from surface records and appearance groups. |
| Surface record is 22 bytes with one material and key 0x10000050 | DS §7.4, MV | 22 bytes is the 1-material case (13,691). Lengths 18–30 occur, and the key varies (§4). |
| "The first 0x40 slot of a material is the diffuse" | DS §7.4, MV, `texture-list.md` T12 | The diffuse is slot **id 3000**. The first texture entry is 3000 in 22,502 materials, but not in 297 (261 have no 3000 at all, for example only a normal map or dye; 36 list 3000 later). Use the id. |
| Material entry `{slotId, slotId2, type = 4, DID, params…}` | DS §7.4 | Entry is id (u32), group (u16), type, value, u32 binding count (0), u8 flag (§5). |
| UV at vertex offset 24 for every stride | MV | Only without the vertex-colour bit. With it (first format byte 0x52 instead of 0x12) a 4-byte colour precedes the UV, which is then at 28 (seen while rendering; belongs in `mesh.md`). |
| Shaders classified by counting constant names in the blob | MV | Not needed. The parameter interface and sampler names carry the same information (§11.1). |
| Dye colours: one palette keyed by dye code | MV | Per-item colours via dye materials (§12.4). |

---

<a id="s16"></a>
## 16. Synthetic fixtures for implementers

- Surface records with 0, 1 and 3 materials, and with the length off by one (reject).
- Materials with one entry of each value type, including curves of modes 1, 4 and 10
  (*k* = 3); one with an unknown `value_type` (reject); one with `group` ≠ ⌊id/1000⌋×1000
  (warn).
- A shader record with two declarations (3000 with two texture bindings, 4004 with none),
  two permutations: one without programs, one with two small opaque blobs, a sampler table
  naming `ColorAndOpacity` and `TSNormal`, one extra item of type 5, and a render-state
  block for SRCALPHA / INVSRCALPHA with `cull` = 2.
- A binding whose `permutation` is out of range, and a texture binding whose `slot` is out
  of range (reject).
- Resolution cases: default only; one material; two materials overriding the same id
  (last wins); an appearance group with a shader override; a dye layer.
- Role selection: one slot named `DiffuseTexture` in one permutation and `ColorAndDye` in
  another (role is `ColorAndDye`).

---

<a id="s17"></a>
## 17. Open questions

Settled only by a frame capture of the original client (not planned now):

1. Render-state bytes 4–6 and the blend-enable question (§7.6, §11.3): does a lit base pass
   blend, and is any lit surface translucent?
2. Whether flag 4007 is two-sided (§11.4).
3. Alpha test with no opacity texture (§11.2); alpha-to-coverage or edge softening.
4. Exact dye blend: threshold or smooth mask, linear or gamma space, role of the dye
   colour's alpha (§12.3).
5. The lighting model: how gloss, 2001 and 1000 combine; environment-map use; normal-map
   channel conventions (§10.2, §11.5).
6. Texture-matrix composition, U/V assignment and coordinate-set per slot (§8.4).
7. Waveform shapes of animated float curves (§5.3).
8. Meanings of flags 4001, 4002, 4003, 4005, 4006 (§8.5).

Answerable from data with more probing:

9. The surface-record `tail` u16 and `binding_key` values 1 and 2 (§4.2).
10. Surface records listing alternative materials (§9.2).
11. Appearance group fields `a` and `g`, and the 1,010 unmatched groups (§9.3); the dye-block
    fallback rule and the default dye property (§12.4). Belongs with `appearance.md`.
12. The permutation `key` bits, `slot` of non-texture bindings, extra state items, sampler
    state bytes other than address and filter (§7.3–§7.7). Tessera does not need them.
13. Which record types reference the 353 unreferenced surface records (§3.2).
14. Effect-family parameters (water, wind, snow, …) (§8.7). Needed for M3.

---

<a id="s18"></a>
## 18. Open items raised by the `material` implementation (2026-10-04)

These came up while implementing §4–§12 in `crates/material` (`records`, `interface`,
`resolve`, `dye`, `MaterialLibrary`) from this spec alone. The whole-install run of
`crates/material/tests/material_real_data.rs` decodes all 510 shader records, 24,094
materials and 13,781 surface records with no MT1, MT2, MT4, MT5, MT7 or MT8 finding,
resolves every surface record, and resolves the 1,199 submeshes of 500 random meshes
(MT6 holds; every diffuse texture decodes; 298 dye previews). None of the choices below
changes a result on current data unless stated.

> **OPEN:** Ties. §10.1 (role name) and §11.3/§11.4 ("most common" triple, cull value and
> `alpha_ref`) do not say how ties are broken. The implementation takes the candidate met
> first (binding order; permutation order).

> **OPEN:** §11.3 says depth write "follows `depth_write` of the same block", but several
> blocks may carry the chosen triple with different `depth_write`. The implementation takes
> the majority among those blocks, ties to "write".

> **OPEN:** Alpha-test threshold when the shader declares 4004 but not 1043 (MT8 says this
> never happens). The implementation falls back to the most common render-state
> `alpha_ref`, then 0.5. A non-finite 1043 is treated the same way.

> **OPEN:** §11.2 picks the opacity source by role only. When the effective texture of that
> slot is 0 (4 material entries set a texture to 0), the implementation still enables alpha
> test with that slot as source; a renderer has no texture to test and should treat opacity
> as 1. Whether the original then disables the test is unknown.

> **OPEN:** §11.3 maps unlit records to a fixed blend but does not say whether alpha test
> (§11.2) also applies to blended surfaces. The implementation reports both independently:
> an unlit ONE / ZERO record with alpha test on becomes cut-out; a blended record keeps its
> blend mode and also carries the alpha test.

> **OPEN:** §12.3 says the second dye channel (2013) uses "the mask from the second dye-mask
> source" without ordering the sources. The implementation orders them: 3000 with a `Dye`
> role, 3011 with a `Dye` role, 3024 (declared), 3001 with a `Dye` role. The primary channel
> uses the first; the second channel uses the second, or the first when there is only one.
> On current data 243 surface records declare 2013.

> **OPEN:** Dye preview details not covered by §12.3: the output alpha is the diffuse alpha
> only when the diffuse role's alpha is opacity, otherwise 255 (the alpha is a mask or
> gloss); a mask texture of another size than the diffuse is sampled nearest-neighbour; the
> blend is applied to the stored 8-bit values (gamma vs linear is already OPEN).

> **OPEN:** A shader record that declares the same id twice (never observed) keeps the first
> position and the last value, like a material layered on itself. Value type 4 is read as a
> texture DID when the *id* is in the 3000 group and as a flag word otherwise, whatever the
> stored `group` field says (a mismatch is reported as unusual).

> **OPEN:** Blend-factor and blend-op codes that §7.6 does not name (for example 8, 10, 11
> and op 2) are reported as "other" with their raw code; the renderer needs a fallback. None
> occurs in the most common triple of any record whose surfaces were resolved.

> **OPEN:** Material resolution policy for broken references: a material that is missing or
> does not decode is left out of *L* (and reported); a missing or undecodable shader record
> fails the whole submesh. The original's behaviour is unknown; current data has no such
> reference.

> **OPEN:** `MaterialLibrary::texture` shows a cube map as its first stored face (face
> order is OPEN in `texture-list.md` §3.2). `MaterialLibrary::cube_faces` returns all six
> faces of a single-level cube in stored order; a cube with more than one level (never
> observed, array order OPEN) is not split.

> **OPEN:** Slot-3000 role names outside the §10.2 table occur on current data:
> `ColorTexture` (4 surface records), `LiquidDiffuseTexture` (12) and
> `DecalDynamicLightTexture` (1); 21 more have no named binding. The implementation gives
> names outside the table "other" RGB and "unknown" alpha (sampled as plain colour, §10.1),
> except names starting with `Diffuse`, which it reads as base colour with alpha unused.
> Whether `ColorTexture` and `LiquidDiffuseTexture` carry opacity in alpha is unknown.

Added after review of the implementation (2026-10-04):

> **OPEN (Decision, frame capture to confirm):** Opacity of unlit blended records whose
> diffuse role has no opacity in alpha. §11.3 takes "source alpha" from the §11.2 opacity
> source, or 1 without one. 522 surface records are unlit, transparent, use SRCALPHA or
> INVSRCALPHA, and have no §11.2 source: their slot-3000 role is `Color` (463),
> `DiffuseTexture` (43), `LiquidDiffuseTexture` (12) or `ColorTexture` (4). Their
> textures often carry the effect's shape only in alpha (78 of their 368 distinct diffuse
> lists are at least a quarter below alpha 128; others are DXT1 with alpha 255), and
> with opacity 1 they would draw as solid cards. Tessera's rule: for an unlit record whose
> chosen blend triple has a SRCALPHA or INVSRCALPHA factor and no §11.2 source, the
> blend's source alpha is the alpha of slot 3000 when that slot's alpha (§10.2) is "not
> used" or unknown (`Color`, `DiffuseTexture`, `Diffuse…`, unnamed, names outside the
> table); a gloss or dye-mask alpha is never used. Alpha test (§11.2) is unchanged. On
> current data this leaves no transparent source-alpha surface record whose diffuse is
> mostly transparent without an opacity source; 15 such records declare no slot 3000 at
> all (their only slot is 3011 or 3017, or none) and blend with opacity 1. Exposed as
> `BlendState::opacity_slot`.

> **OPEN (Decision):** Bounds on resolution, for hostile input (observed data is far
> below them). A material listed more than once in *L* is applied at its last position
> only; with last-wins layering the effective values are identical. More than 64 distinct
> materials fail the resolution. At most 1,024 material entries without effect are kept
> for the inspector; further ones are only counted.

> **OPEN (Decision):** Texture level selection in `MaterialLibrary` (`texture-list.md`
> §5.2): the configured largest texture side acts as a resolution cap on top of the
> caller's (step 5), so a smaller stored level is used instead of failing; it is an error
> only when even the last level is larger. A level's size is its header size, or for a
> JPEG level (header 0×0) its stream's frame size; a level of zero width or height, or a
> JPEG level whose frame header cannot be read, does not resolve (step 3).

<a id="s19"></a>
## 19. Open items raised by the explorer mesh viewer (clean room, 2026-10-04)

The explorer's 3D view (`apps/explorer`, `scene` module) maps resolved materials onto the
`tessera-render` mesh pipelines. A real-data run (`apps/explorer/tests/mesh_real_data.rs`,
`TESSERA_EXPLORER_FULL=1`) built every one of the 47,563 mesh records into a drawable scene:
72,839 level-0 draws, every surface record resolved, 0 failures. Choices the spec leaves
open:

> **OPEN:** Specular power 0. Several common shader records declare 1000 (so §11.5 sets
> `specular`) with default 0, and materials that want a highlight override it (25, 30).
> In a sample of 764 level-0 draws (every 97th mesh record), 737 materials are specular
> and 138 of them have effective power 0. With §11.5's `(N·H)^1000` that is a constant,
> view-independent highlight over the whole surface. The viewer treats power ≤ 0 as "no
> highlight" (and loads no gloss mask then). Whether 0 means "off" in the original is
> unknown.

> **OPEN:** Diffuse colour 2000 (§8.3, Inferred). Census of the install's 24,094
> materials: 1,055 set 2000; 340 are white, 8 are black (RGB 0; alpha 0, 0.75–0.86 or 1),
> and 707 are other tints; alpha is 1 in 791 and spread over 0–0.98 in the rest. A black
> 2000 multiplied into the base colour draws the surface solid black (for example the
> one-triangle part of setup 0x04000001, whose material sets (0, 0, 0, 0)), so "tint" may
> be wrong or need a condition (alpha 0 = unset?). The viewer therefore applies 2000 only
> on request ("Diffuse colour", `--mesh-diffuse-tint`; read as sRGB-encoded like the
> textures) and notes non-white values in the material inspector.

> **OPEN:** Vertex colours (`mesh.md` §5.4). The viewer passes them only on request
> ("Vertex colours", `--mesh-vertex-colors`), RGB only, read as sRGB-encoded bytes, with
> alpha always 1. Reason (explorer review, every 7th mesh record, 6,795 meshes): 117
> alpha-tested and 20 blended draws have most vertices with alpha below the 0.8 threshold,
> and 180 opaque draws have mostly dark RGB; multiplied in, they delete foliage cards and
> tent canopies and blacken snow (meshes 0x06007d43, 0x060070ad, 0x06002990), which look
> right without them. Whether the original multiplies them into the diffuse, uses them for
> something else (wind weights, blend masks, baked occlusion), or ignores them per shader
> is unknown.

> **OPEN (Decision):** Specular colour 2001 is read as sRGB-encoded and converted to
> linear like 2000 and the dye colour, so the three material colours share one convention
> in the viewer (the renderer's specular colour is linear). With the stored greys (§8.3)
> this gives a highlight about 3–4 times weaker than passing the stored value as linear.
> Which space the original uses is unknown.

> **OPEN (Decision):** Opacity for the renderer. The viewer composes one base-colour
> texture whose alpha is the §11.2 opacity source (slot 3000's own alpha, or the alpha of
> 3020/3018 resampled nearest to 3000's size), or opaque when there is none, because the
> renderer takes coverage from the base texture's alpha. Dye masks (§12.2) are moved into
> R as `255 − alpha` (the renderer's mask convention, `render.md` §2.3) and gloss masks
> (§10.2) replicated into RGB; the dye colour is converted from sRGB to linear before the
> renderer's linear multiply, which equals §12.3's multiply of stored values for a pure
> power curve.

> **OPEN (Decision):** Two dye channels. The renderer takes one dye per draw, so the viewer
> draws one channel at a time: the first, unless only the second has a colour other than
> white, or the second while a "dye 2" colour is previewed. A "dye 2" preview leaves
> materials without a second channel on their own first dye: §12.3 applies 2013 only
> through the second dye-mask source. Drawing both channels needs a second dye slot in
> the renderer.

> **OPEN:** Material-only appearance entries (`setup.md` §3.2: entries with dye blocks and
> no parts, for example the 7-block coat dyes of a mount) name no geometry. The viewer
> lists their blocks' 2003/2013 colours and lets the user preview one on any dyeable
> material, but cannot tell which geometry a block layers onto (the same item's worn
> entry? the wearer's body?). The rule that pairs them is needed for a faithful dye
> preview of worn items.

