<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Materials: surfaces, materials, shader records, texture slots, feature flags and dyes

| | |
|---|---|
| **Status** | Accepted (2026-10-08; fifth revision of 2026-10-07, see §20.2) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Third review 2026-10-06: **passed**, Accepted. Full record moved to the review log (decision 0018). Fourth review (fifth revision) 2026-10-08, separate review session: **passed**, Accepted; details in the review log. |
| **Implements into** | `crates/formats` or `crates/material` (decoders for prefixes 0x31, 0x30 and 0x2B), `crates/material` (effective parameter set, texture roles, feature flags, dye), `crates/render` (pipeline selection from feature flags), explorer material inspector and dye preview |
| **Companion specs** | `texture-list.md` (a texture slot names a 0x40 texture list; level selection and highres fallback). `records.md` §1 (primitives), §5 (0x41 surfaces). `dat-container.md` (routing). `mesh.md` (mesh record layout; only the surface list is used here). [`setup.md`](setup.md) §8 (normative for the 0x20 worn-appearance layout) and [`appearance.md`](appearance.md) §4.9–§4.10 (dyes and material groups of worn items); only the parts needed for material binding are repeated here, §9.3. [`lighting.md`](lighting.md) (passes, lighting equation and engine inputs that use these parameters; §4.4 colour encodings). [`terrain-texturing.md`](terrain-texturing.md) (terrain layers use the same transform and parameters, §6.3, §8.2). [`water.md`](water.md) §8 (water parameters). |

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
- **BIN**: rests on disassembly of the client executable or of the compiled programs stored in
  the shader records (method M4, decision 0011; tags BIN-S, BIN-C, BIN-T below), not on data
  observation. Decision 0011 requires the mark; tests may rely on BIN facts as the original's
  behaviour, but they were not checked against a running client.
- **Decision** (also written **Tessera Decision**): a rule chosen by this project where the
  original's behaviour is unknown or not needed; tests must not treat it as the original's
  behaviour.

Facts first observed on 2026-10-05 (EM, BIN-T and the census figures added that day) carry
that date; the rest of the Confirmed facts date from 2026-10-04.

| Tag | Source |
|---|---|
| OBS | Observed in user install 2026-10-04 (Steam, data current on that date). Every 0x31 (13,781), 0x30 (24,094) and 0x2B (510) record decoded to the last byte. Header and surface list of every mesh record (47,563) read. Every appearance record (prefix 0x20, 4,201) parsed with the grammar of DS §7.2 (4,199 clean). A raw scan of every other record of `client_general.dat` for DIDs that resolve to 0x30, 0x31 or 0x2B records. Four meshes rendered by a throwaway software rasteriser using the resolution rules of §9–§12 and inspected by eye (§13). |
| MV | Mirarkitty `lotro_mesh_viewer` docs (`textures.md`, `shaders.md`, `dyes.md`, `mesh-format.md`; MIT for docs, repository GPL-3.0). Facts only. |
| DS | `research/notes/data-semantics.md` §7.2–§7.5. |
| KC | gl_kilves `lotro-mesh-converter` (MIT): per-submesh vertex-format bytes in the mesh header. Facts only. |
| D3D | Public Direct3D 9 enumerations (`D3DBLEND`, `D3DBLENDOP`, `D3DCMPFUNC`, `D3DCULL`, `D3DTEXTUREADDRESS`, `D3DTEXTUREFILTERTYPE`). General references used to name observed small integers. |
| EM | Enum mapper 0x230001A6 in the user's install (observed 2026-10-05): it labels every material parameter id (197 labelled ids; the 148 ids that occur in materials or shader records are all among them, §8.0). Labels are data and are quoted for interoperability. |
| BIN-S | **M4 (decision 0011).** Client build 4906.0071.1794.4035: the compiled programs stored in the shader records were disassembled with a public shader disassembler and read for representative permutations of every pass code. **Question:** what the programs do with each parameter (alpha-test comparison, dye blend, normal-map axes, specular, output alpha, vertex colours, texture coordinates). **Why M1–M3 were not enough:** the data stores only defaults and bytecode; no frame capture (M3) is available; nothing is published (M2). Details in [`lighting.md`](lighting.md). Third pass 2026-10-05: the Direct3D 10/11 colour programs (pass 0x00) of every shader record behind the 537 unlit blended surface records without an opacity source, with the programs' constant reflection tables used to name each input. **Question:** what reaches the output alpha (§20.1). Same reasons. |
| BIN-C | **M4 (decision 0011).** Same build, client executable: the float-curve deserialiser and evaluator. **Question:** waveform shapes of float-curve modes 2–10 (§5.3) and the mode-10 pair order. **Why M1–M3 were not enough:** the shapes are code; the data only holds parameters. |
| BIN-T | **M4 (decision 0011), second pass 2026-10-05.** Same build (`x64/lotroclient64.exe` 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), objdump; the texture-matrix routine was also run in a CPU emulator with synthetic inputs. **Questions:** (a) how the seven transform values of a texture matrix compose (§8.4); (b) which binding `semantic` writes which transform value (§7.3); (c) the comparison the mode-10 key search uses at exact key positions (§5.3.1); (d) the noise and random generators of modes 3, 7 and 8 (§5.3.1); (e) the clock passed to curves and to the programs' time input (§5.3.1); (f) what the per-material setup does with the specular power, colour 2000 and the ambient inputs (§8.3, §11.5); (g) the graphics back ends' texture formats (sRGB, §12.3). **Why M1–M3 were not enough:** all of these are code paths; the data holds only parameters; no frame capture (M3) is available; nothing is published (M2). |
| BIN-R | **M4 (decision 0011), fourth pass 2026-10-07.** Same build (`x64/lotroclient64.exe` 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5), decompiled with a headless disassembler on a copy of the analysis project. **Questions:** (h) what the two bytes after a surface record's material list mean (§4); (i) how appearance groups are matched to parts and surfaces (§9.3); (j) how the client reads and applies parameter bindings, render-state blocks, sampler states and extra state items (§7.3, §7.5–§7.7); (k) what 4007 AlphaBlendPass does (§8.5); (l) how the alpha-test reference is handled per draw, and whether alpha-to-coverage is used (§11.2); (m) how materials are layered and what happens on broken references (§9.2); (n) whether mode 10 gets the clock reduction (§5.3.1). **Why M1–M3 were not enough:** the data holds bytes whose use is code; their values barely vary (most fields are constant in all records), so statistics cannot name them; no frame capture (M3) is available; nothing is published (M2). |
| BIN-S4 | **M4 (decision 0011), fourth shader pass 2026-10-07.** Same build; the compiled programs stored in the shader records, disassembled with the same public shader disassembler. **Questions:** output alpha of the `ColorTexture` record and of the lit base passes whose opacity comes from 3020 or 3018 (§18); which texture matrix the emissive and glow samplers use (§8.4); how the tie-dye ramp is indexed (§12.5); what the `DecalDynamicLightTexture` record outputs (§18). Same reasons as BIN-S. |
| WAT | [`water.md`](water.md) §8.2–§8.4 (water surface parameters). |
| TT | [`terrain-texturing.md`](terrain-texturing.md) §6.3, §8.2 (the terrain layer's use of the same transform and parameters). |

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
| MA10 | Effective parameters = shader defaults, then each material in order, later entries win (§9.2) | OBS (layering of 72 two-material surfaces; dye layers render correctly, §13); BIN-R (m) (the client applies every material in order, 2026-10-07) | Confirmed for layering; variant-like lists: last wins (BIN, MA45) |
| MA11 | Appearance material groups bind to submeshes by the surface record's binding key (§9.3) | DS §7.2 (group fields); OBS (46,885 of 47,895 groups match a key of the part meshes) | Confirmed |
| MA12 | Alpha test is enabled by flag 4004 and its threshold is float 1043 (§11.2) | MV (cutout shaders exist); OBS (always declared together, 307 shaders; 3,386 surfaces enabled; visual check) | Confirmed |
| MA13 | Dye colour is slot 2003 (and 2013 for a second dye channel) of a one-entry material layered by the appearance record's dye block (§12) | DS §7.5 (dye code, mask); OBS (2,180 dye entries; visual check) | Confirmed |
| MA14 | Dye colours are per appearance, not a global palette keyed by dye code (§12.4) | OBS (modal colour per code agrees in only 9,060 of 16,945 dye blocks) | Confirmed. Contradicts MV's single-palette model. |
| MA15 | Dye mask is the alpha channel of the texture whose sampler name carries "Dye" (`ColorAndDye`, `TSNormalAndDye`, `DyeAlpha`); low alpha = dyeable (§12.2) | DS §7.5, MV (diffuse alpha, low = dyeable); OBS (role table; visual check) | Confirmed (mask location, polarity). Exact blend: BIN (MA24, §12.3). |
| MA16 | Render-state block per permutation: blend factors, blend op, depth comparison (byte 3, read before 2026-10-07 as an alpha comparison), depth write, cull, alpha-test switch, alpha reference, default colours and specular power (§7.6) | OBS (31,519 blocks); D3D (enum values); BIN-R (j) | Layout Confirmed; meanings BIN (MA40) |
| MA17 | Shader records are compiled HLSL with no name field; classification by counting constant names in the blob | MV | Reported. Not used by Tessera (§11.1). |
| MA18 | Parameter labels (enum mapper 0x230001A6): 4001 DepthTest, 4002 DepthWrite, 4003 DoubleSided, 4004 AlphaTest, 4005/4006 DiffuseMapWrapU/V, 4007 AlphaBlendPass; the full list §8.0 | EM | Confirmed (labels) |
| MA19 | The per-entry `flag` byte is 1 exactly for the ids whose label starts with `c_` (95 ids) and 0 for the other 53 (§8.6). Flag-1 parameters reach the programs by name; the only one with binding entries is 2013 (one semantic-4 binding in 7 of 11 records, naming a permutation without programs, §7.3) | EM; OBS (all declarations and material entries; 2013 bindings re-checked 2026-10-06) | Confirmed |
| MA20 | Two-sided is flag 4003, not 4007 (§11.4) | EM; OBS (4003 is bound to permutations with binding semantic 5) | Confirmed label; rendering effect BIN |
| MA21 | Float-curve waveforms (§5.3) | BIN-C | BIN |
| MA22 | Alpha test discards when opacity ≤ threshold; lit base passes blend with output alpha = opacity × material alpha (§11.2, §11.3) | BIN-S | BIN |
| MA23 | Normal map axes: tangent, negated binormal, normal (§10.2) | BIN-S | BIN |
| MA24 | Dye blend: smooth form of §12.3 on sampled values | BIN-S | BIN |
| MA25 | Program blob kinds 0–4 (§7.4) | OBS (magic of every blob) | Confirmed |
| MA26 | Vertex colours are read only by shader records whose programs take a colour input (116 records) | OBS | Confirmed |
| MA27 | Texture matrix = early translation, scale, counter-clockwise rotation in degrees, translation, applied after the matrix the slot already holds (§8.4) | BIN-T (a) (emulated: error ≤ 1.2 × 10⁻⁶ over 20 random parameter sets) | BIN |
| MA28 | Semantics 38–44 write translate U/V, rotate, scale U/V and early translate U/V of a type-5 item; semantics 33–37 write the curves of type-1, -2 and -3 items: 33 and 34 translate U and V, 35 rotation, 36 and 37 scale U and V (§7.3, §7.7). **Corrected 2026-10-07:** the earlier reading "33, 35, 36 write early translate U, 34 and 37 early translate V" took the item to be of type 5 | BIN-T (b), BIN-R (j) (the switch writes the first or second curve of the item the binding names); OBS (all 363 bindings with semantics 33–37 name an item of the matching type) | BIN |
| MA29 | Mode-10 search uses strict comparisons: a key exactly at the current position is used neither as lower nor as upper key (§5.3.1) | BIN-T (c) | BIN |
| MA30 | Mode 3 draws from the client's global random generator; modes 7 and 8 use a 1-D gradient noise whose two tables are defined by a fixed pseudo-random sequence (a linear congruential generator started from state 0), mode 8 sums 6 octaves (§5.3.1) | BIN-T (d) | BIN |
| MA31 | Material curves and the programs' time input use the client's global clock: seconds since the client's timer started; the time input is that value modulo 3,600 above 3,600 (§5.3.1) | BIN-T (e) | BIN for the time input and the frame code; per-draw path Inferred |
| MA32 | The specular colour is replaced by black when the effective specular power is below 0.5 (§8.3, §11.5) | BIN-T (f) | BIN |
| MA33 | No back end samples textures through sRGB formats; colours are used as stored (§12.3) | BIN-T (g) (Direct3D 10/11 and OpenGL back ends) | BIN |
| MA34 | Output alpha of the unlit blended surface records without an opacity source, by role and record (§20.1) | OBS (census, 2026-10-05); BIN-S (third pass) | Census Confirmed; program behaviour BIN |
| MA35 | Flag-1 ids in the data: 1044–1069, 1100–1106, 1120–1127, 1140–1149, 1160, 1170, 1171, 1173–1180, 1182–1207, 2004–2006, 2009–2011, 2013 (95 ids); 197 labelled ids, 148 of them used (§8.0, §8.6) | EM; OBS (all declarations and material entries, 2026-10-05) | Confirmed |
| MA36 | Light inputs read by the unlit records used by surface records: 11 records (99 surface records) read some, 94 (2,043) none; per record and permutation as in §20.1 | OBS (constant tables of every program permutation of all four back ends, 2026-10-06); meanings of the inputs BIN-S (`lighting.md` §4.2) | Confirmed (census); input meanings BIN |
| MA37 | The two bytes after a surface record's material list are two flags (each 0 or 1): shadow receiver and not pickable (§4.1) | BIN-R (h); OBS (census of all 13,781 records, 2026-10-07) | BIN (meaning); values Confirmed |
| MA38 | Appearance group matching: slot key `a` against the part, binding key `b` against the surface record; 2 matches everything, `a` = 0 nothing; groups act on every part of the drawn entity (§9.3) | BIN-R (i); OBS (census of all 47,895 groups that come with parts, 2026-10-07) | BIN (rule); census Confirmed |
| MA39 | Bindings apply per permutation: semantics 0–9 set that permutation's pass state, 10–12 its sampler `slot`, 33–44 its extra state item `slot` (§7.3) | BIN-R (j); OBS (all `slot` values in range) | BIN |
| MA40 | Full render-state block: byte 3 is the depth comparison for a reversed depth range, byte 6 the alpha-test switch, bytes 7–38 the default alpha reference, colours and specular power as float curves and packed colours (§7.6) | BIN-R (j); OBS (all 31,519 blocks, 2026-10-07) | BIN (meaning); values Confirmed |
| MA41 | Full sampler state; bytes 10 and 11 are minification and magnification in that order (§7.5) | BIN-R (j); OBS (all 105,119 sampler entries); sampler-source names from co-occurrence with sampler names (OBS) | BIN (meaning); values Confirmed |
| MA42 | The extra state list is the permutation's list of texture-coordinate transforms; `stage` is the texture matrix it changes (§7.7, §8.4) | BIN-R (j); OBS (all 17,591 items) | BIN |
| MA43 | Effective 4007 AlphaBlendPass makes the client draw the object a second time, without alpha test and depth write, with a strict depth test (§8.5) | BIN-R (k) | BIN |
| MA44 | Per-draw alpha-test handling: reference quantised to 1/255, scaled by the material alpha or test dropped, reference 0 for SRCALPHA passes, optional alpha-to-coverage (§11.2) | BIN-R (l) | BIN |
| MA45 | Layering in the client: materials in stored order, first matching declaration, unknown ids skipped, a missing material left out, a missing shader record leaves the surface without effect (§9.2) | BIN-R (m) | BIN |
| MA46 | Output alpha of the lit base passes is the 3020 or 3018 alpha × the material alpha when the 3000 role carries no opacity; `ColorTexture` outputs 2 × both layer alphas × the 2000 alpha (§18, §20.1) | BIN-S4 (every lit base permutation of the 7 records concerned; both back ends of 0x2B0000B7) | BIN |
| MA47 | Emissive, `EmissiveAndOpacity` and glow samplers use texture matrix 0 or 1 depending on the record (§8.4) | BIN-S4 (Direct3D 10/11 programs of all 152 records with such samplers) | BIN |
| MA48 | Tie-dye: the ramp is indexed by a power of N·V plus an offset, then lerped toward white (§12.5) | BIN-S4 (0x2B00072F, 0x2B00076E) | BIN |
| MA49 | Mode 10 receives the same modulo-3,600 clock reduction as the other modes (§5.3.1) | BIN-R (n) | BIN |

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
| 0x20 | appearance | general (4,201) | | | `setup.md` §8 (layout); `appearance.md` (use); §9.3 here |

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
| 16+4*n* | 1 | u8 | `shadow_receiver` | 0 or 1 (BIN-R (h), MA37). 1 in 13,711 records, 0 in 70. |
| 17+4*n* | 1 | u8 | `not_pickable` | 0 or 1 (BIN-R (h), MA37). 0 in 13,780 records, 1 in 1. |

Total size = 18 + 4·*n*. A reader checks this exactly. The client rejects a record in which
either flag byte is 2 or more (BIN-R (h)); a reader does the same.

The two bytes were read before 2026-10-07 as one u16 `tail` (1 in 13,711 records, 0 in 69, 256
in 1). Read as bytes: (`shadow_receiver`, `not_pickable`) = (1, 0) in 13,711 records, (0, 0) in
69 and (0, 1) in 1 (0x3100000D). Of the 69 (0, 0) records, 38 use lit shader records and 31 unlit
ones (effects, light projectors, depth-tested overlays).

Meaning (BIN-R (h)):

- **`shadow_receiver`**: the client gathers the triangles of every submesh whose surface record
  has `shadow_receiver` = 1 and `not_pickable` = 0 (and of submeshes without a surface record)
  as receiver geometry for its projected-shadow objects (the scene objects the client classes
  as shadows, onto which an object's simple shadow is projected). Submeshes of other surface
  records receive no projected shadow.
- **`not_pickable`**: the client's ray tests against a mesh (finding the object under a ray,
  for example the mouse cursor) skip submeshes whose surface record has `not_pickable` = 1, and
  their triangles are left out of the pick acceleration structure. Such submeshes are not
  gathered as shadow receivers either.

Neither flag changes how the submesh is drawn. Tessera reads both and may ignore them until it
implements projected shadows and picking.

<a id="s4-2"></a>
### 4.2 Census (Confirmed)

| Field | Values (records) |
|---|---|
| `material_count` | 0 (6), 1 (13,691), 2 (82), 3 (2) |
| `binding_key` | 1 (6,976), 2 (8), 0x1000xxxx (6,797; 203 distinct values) |
| `shader` | 241 distinct; all resolve |
| `materials` | 13,861 references, all resolve; 13,608 distinct materials |
| `shadow_receiver`, `not_pickable` | see §4.1 |

Most-used shader records by surface count: 0x2B0006EB (2,526), 0x2B0007A0 (1,794),
0x2B0007D1 (1,079), 0x2B0007D5 (960), 0x2B000700 (743).

Surface records with `material_count` = 0 use the shader's defaults only (§9.2).

> **Answered 2026-10-07 (§4.1, BIN-R (h); was OPEN):** the former `tail` u16 is the two flag
> bytes `shadow_receiver` and `not_pickable`.

`binding_key` (answers the former OPEN, BIN-R (i), MA38): the client uses it only to match
appearance material groups (§9.3). No surface-record value is special: a group matches a
surface record whose `binding_key` equals the group's binding key, and a group whose binding
key is 2 matches every surface record. Value 1 is the key of static world meshes (6,976
records); 894 appearance groups use binding key 1 and so act on such surfaces. The 8 surface
records whose own key is 2 are matched by groups with key 2 like every other surface record.

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
### 5.3 Float curve (Confirmed layout; evaluation BIN)

| Field | Size | Meaning |
|---|---|---|
| `mode` | u32 | 1–10 observed |
| if `mode` = 1 | f32 | constant value `w[0]` |
| if 2 ≤ `mode` ≤ 9 | 10 × f32 | waveform parameters `w[0..9]` |
| if `mode` = 10 | 10 × f32, f32, u8, u32 *k*, *k* × (f32, f32) | `w[0..9]` (unused by this mode), `period` (seconds), `loop` (0 or 1; a reader rejects ≥ 2), then *k* keyframe pairs (**value**, **position**) where position is a percentage 0–100 of the period |

Material census: mode 1 (24,587), 2 (1,592), 3 (6), 4 (542), 5 (3), 6 (19), 7 (205),
8 (27), 9 (63), 10 (38). Shader defaults use modes 1 (4,123), 2 (48) and 4 (8). The same
grammar occurs in day-description keyframes (`environment.md` §5.3).

<a id="s5-3-1"></a>
#### 5.3.1 Evaluation (BIN)

Input: a time *t* in seconds and a phase offset *s* (0 at every call site seen). Mode 1
returns `w[0]`. For the other modes:

**Clock (BIN-T (e)).** The client's frame code passes its **global clock**: seconds since
the client's timer started (it is reset to 0 when the timer starts), held as a double and
shared by all objects. The same clock drives the light records (`lighting.md` §8.3) and the
time input of the shader programs (that value converted to a float and, once it exceeds 3,600,
reduced modulo 3,600; `lighting.md` §4.2). The per-material draw receives its time as an argument from its caller; that
it is the same clock is Inferred. No per-object time offset was found.

1. If |*t*| > 3,600, replace *t* by the floating-point remainder of *t* / 3,600 (it keeps
   the sign of *t*). Mode 10 applies the same reduction to the time it compares with its start
   time (BIN-R (n), MA49).
2. Compute, with *x* = *t*:
   - base *B* = `w[0]` + `w[1]` · *x*;
   - amplitude *A* = `w[2]` + `w[3]` · *x*;
   - frequency *F* = `w[6]` + `w[7]` · *x* (radians per second for the periodic modes);
   - phase *P* = π · (*s* + `w[4]`) + `w[5]` · *x*;
   - *G* = `w[8]` + `w[9]` · *x* (mode 8 only).
3. Result by mode, with *θ* = *F* · *x* + *P*:

| Mode | Result | Shape |
|---|---|---|
| 2 | *B* + *θ* | Linear ramp (scrolling): `w[6]` is units per second |
| 3 | *B* + *A* · *r*, *r* a uniform random value in [−1, 1] drawn from the client's global random generator at each evaluation (BIN-T (d)) | Random flicker |
| 4 | *B* + *A* · sin *θ* | Sine |
| 5 | *B* + *A* when sin *θ* ≥ 0, else *B* − *A* | Square wave |
| 6 | *B* + *A* · abs(sin *θ*) | Rectified sine |
| 7 | *B* + *A* · *N₁*(*θ*) | Smooth one-dimensional noise of *θ*; *N₁* below (BIN-T (d)) |
| 8 | *B* + *A* · *N₂*(*θ*), with *N₂*(*z*) = the sum over *i* = 0 … 5 of *N₁*(*z* · 2^*i*) · 2^(−*H* · *i*) and *H* = 1 − *G* | Fractal noise: 6 octaves, frequency ratio 2, weights 2^(−*H*·*i*), summed in double precision (BIN-T (d); checked by running the client routine in an emulator against this formula, difference ≤ 3 × 10⁻⁸) |
| 9 | see below | Staircase (flip-book frames) |
| 10 | see below | Keyframed, linear between keys |

**Noise *N₁* (modes 7 and 8; BIN-T (d)).** *N₁* uses two fixed tables of 256 entries, a
gradient table Γ and a permutation Π of 0 … 255. Both are defined by one sequence of
pseudo-random integers *r*₁, *r*₂, *r*₃, … (each in 0 … 32,767):

- the sequence is the output of the linear congruential generator with state *S*₀ = 0,
  *S*ₖ = (*S*ₖ₋₁ × 214,013 + 2,531,011) mod 2³² and output *r*ₖ = ⌊*S*ₖ / 65,536⌋ mod 32,768
  (this is the generator of the Microsoft C runtime seeded with 0, general knowledge of that
  runtime; the constants are functional facts, decision 0012);
- **gradients:** Γ[*i*] = ((*r*₍ᵢ₊₁₎ mod 512) − 256) / 256 for *i* = 0 … 255, i.e. the first 256
  outputs, in order;
- **permutation:** start from the identity (Π[*i*] = *i*); then for *m* = 1 … 255 take the
  output *r*₍₂₅₆₊ₘ₎ and exchange the entries at positions *i* = 256 − *m* and
  *j* = *r*₍₂₅₆₊ₘ₎ mod 256 (so position 255 is exchanged first and position 1 last; *j* may
  equal *i*).

The tables are built once, the first time either mode is evaluated, from a generator state
set to 0 immediately before, so they are the same in every run. (The client also keeps
a repeated copy of both tables; the lookups below use indices 0–255 only.) To evaluate
*N₁*(*z*) in 32-bit floats:

- *y* = *z* + 10,000; *k* = the integer part of *y*; *b₀* = *k* mod 256 and
  *b₁* = (*k* + 1) mod 256; *ρ* = *y* − *k*;
- *g₀* = *ρ* × Γ[Π[*b₀*]] and *g₁* = (*ρ* − 1) × Γ[Π[*b₁*]];
- *σ* = *ρ*² × (3 − 2*ρ*); *N₁* = *g₀* + *σ* × (*g₁* − *g₀*).

*N₁* is 0 whenever *y* is a whole number and lies in [−1, 1]. (Checked: the client routine run
in an emulator and this description agree exactly on 400 random inputs.)

**Mode 9.** Let *n* = integer part of *A* (steps) and *m* = integer part of *F* (steps per
second). If either is ≤ 0 the result is *B*. Otherwise the cycle length is *c* = *n* / *m*
seconds; *q* = *x* − *c* · (integer part of *x* / *c*); step index *j* = integer part of
*q* · *n* / *c*; result = *B* + *j* · *P*. Example: `w` = (−0.002, 0, 32, 0, 0.01, 0, 16, 0, 0, 0)
gives 32 steps at 16 per second (a 2-second cycle), each adding π × 0.01 ≈ 0.0314.

**Mode 10.** The curve keeps a start time (initially unset) and its last value.

1. If the start time is set and *t* − start ≥ `period`: when `loop` = 0, return the last
   value (the curve holds); when `loop` = 1, set start = *t*. If the start time is unset,
   set start = *t*.
2. If *t* − start < −0.1 (the clock went backwards), set start = *t*.
3. Position *p* = (*t* − start) / `period` × 100; if *p* ≤ 0 use *p* = 0.1.
4. Take the pair with the largest position **strictly** below *p* (lower) and the pair with
   the smallest position **strictly** above *p* (upper). A pair whose position equals *p* is
   used as neither (BIN-T (c), MA29); among pairs with equal positions the first in stored
   order is taken. A missing lower pair counts as (value 0, position 0); a missing upper pair
   as (value 0, position 100).
5. Result = lower value + (*p* − lower position) / (upper position − lower position) ×
   (upper value − lower value); it becomes the last value.

Example: pairs (1.0, 0), (0.5, 45), (0.5, 80), (1.0, 100), `period` 18 s, `loop` 0: at 4.5 s
after start *p* = 25, lower (1.0, 0), upper (0.5, 45), result 1.0 − 25/45 × 0.5 = 0.722; after
18 s the value stays at the last one computed. When *p* is exactly 45 (8.1 s after start), the
pair at 45 is skipped, lower (1.0, 0), upper (0.5, 80), result 1.0 − 45/80 × 0.5 = 0.719 (not
0.5). Just below and just above 45 the result is close to 0.5, so the curve has a one-sample
spike at an exact key position; this is the original's behaviour.

Tessera rule (Decision, revised 2026-10-05): evaluate every mode as above with *s* = 0 and *t*
from Tessera's own monotonic clock in seconds since start (the original's clock up to its
epoch). Modes 7 and 8 use *N₁* exactly as above. Mode 3 may use any uniform random source:
the original draws from a global generator shared with unrelated code, so its sequence cannot
be reproduced.

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
| 8 | 4 | u32 | `slot` | By `semantic` (BIN-R (j), MA39): for 10, 11 and 12 an index into that permutation's sampler table (§7.5); for 33–44 an index into that permutation's extra state list (§7.7); unused for 0–9. Always in range (78,326 texture bindings; every transform binding). |

`semantic` is 10 for every texture binding. For other parameters it is fixed per `id` in
almost all records (for example 2000 → 0, 2001 → 1, 1000 → 2, 2014 → 3, 2003 and
2013 → 4, 4003 → 5, 4001 → 6, 4002 → 7, 4004 → 8, 1043 → 9, 4005 → 11, 4006 → 12; the
texture-transform floats use 33–44). Tessera does not need `semantic`.

Parameters with no binding entries exist: every flag-1 parameter except 2013 (§8.6),
parameter 4007 in all 442 records that declare it, and a few others in a few records. 2013
has exactly one binding (semantic 4, group 2000, `slot` 0) in 7 of the 11 records that declare
it (0x2B00072C, 0x2B0007AC, 0x2B0009C0, 0x2B000A17, 0x2B000A18, 0x2B000A1F, 0x2B000A20) and
none in the other 4 (0x2B0006E4, 0x2B0007B7, 0x2B00080E, 0x2B0009DE). In all 7 that binding
names a permutation without programs (`has_programs` = 0, §7.4; permutation 1 or 2, key
words 22, 0, 8200, 0), so it reaches no program; the programs that read the second dye colour
get it by name (§8.6) like every other flag-1 parameter (OBS, all records, 2026-10-06). They are still part of
the interface (they have defaults and materials set them).

`semantic` is an engine-side role code (OBS census; meanings from the program inputs, BIN-S;
the full table is `lighting.md` §4.1): 0 material colour, 1 specular colour, 2 specular power,
3 glow colour, 4 dye colour, 5 double-sided, 6 depth test, 7 depth write, 8 alpha test,
9 alpha-test threshold, 10 texture, 11/12 wrap U/V, 33–37 and 38–44 texture transforms
(38/39 translate U/V, 40 rotate, 41/42 scale U/V, 43/44 early translate U/V in 12,804
records; 33–37 in a few older ones).

<a id="s7-3-1"></a>
#### 7.3.1 How the client applies a binding (BIN-R (j), MA39)

A value reaches the renderer only through the bindings of the declaration it belongs to. For a
value *x* (the declaration's default, or a material entry's value, §9.2) and each binding
(`semantic`, `group`, `permutation` *p*, `slot`) of that declaration, the client changes
**permutation *p* only**:

| Semantic | What it sets in permutation *p* |
|---|---|
| 0 | material colour (default: render-state bytes 15–18, §7.6) := *x* |
| 1 | specular colour (default bytes 19–22) := *x* |
| 2 | specular power curve (default bytes 23–30) := *x* |
| 3 | glow colour (default bytes 31–34) := *x* |
| 4 | dye colour (default bytes 35–38) := *x* |
| 5 | cull := 1 (NONE) when *x* ≠ 0, else 2 (CW) |
| 6 | depth comparison := 7 when *x* ≠ 0, else 8 (ALWAYS, no depth test) |
| 7 | depth write := (*x* ≠ 0) |
| 8 | alpha-test switch := (*x* ≠ 0) |
| 9 | alpha-test reference curve := *x* |
| 10 | texture of sampler `slot` := the texture list *x* (0 clears it) |
| 11, 12 | address mode U (11) or V (12) of sampler `slot` := 1 (wrap) when *x* ≠ 0, else 3 (clamp) |
| 33 to 44 | one float curve of extra state item `slot` := *x* (table below) |

Other semantic values set nothing. Each binding overrides the block or sampler value of its own
permutation in both directions (for example an effective 4002 = 1 turns depth write on in a
permutation whose block byte 4 is 0, and 4003 = 0 forces culling where the block says NONE).
Permutations that no binding of a declaration names keep their block, sampler or item values.

Transform semantics (BIN-T (b), BIN-R (j), MA28). The binding's `slot` names an item of the
permutation's extra state list (§7.7); the semantic chooses which of that item's curves it sets:

| Semantic | Item type it names (census) | Curve set | Parameters bound with it (census) |
|---|---|---|---|
| 38 | 5 | translation U | 1017, 1019, 1021 (UTranslate…) |
| 39 | 5 | translation V | 1018, 1020, 1022 |
| 40 | 5 | rotation | 1033, 1034, 1035 (UVRotate…) |
| 41 | 5 | scale U | 1001, 1003, 1005 (UScale…) |
| 42 | 5 | scale V | 1002, 1004, 1006 |
| 43 | 5 | early translation U | 1070, 1072, 1074 (UTranslateEarly…) |
| 44 | 5 | early translation V | 1071, 1073, 1075 |
| 33 | 1 | translation U of a translation item | UTranslate and UTranslateEarly ids in older records |
| 34 | 1 | translation V of a translation item | VTranslate and VTranslateEarly ids |
| 35 | 2 | the rotation item's angle | UVRotate ids |
| 36 | 3 | scale U of a scale item | UScale ids |
| 37 | 3 | scale V of a scale item | VScale ids |

In the client, semantics 33, 35, 36 and 43 all write an item's first curve and 34, 37 and 44 its
second; the item types make them mean the values above (all 363 bindings with semantics 33–37
name an item of the type listed; all 121,576 with 38–44 name a type-5 item). Every item of type
5 that any of semantics 38–44 names has all seven: 407 (record, `slot`) pairs, 29 of which also
carry an older semantic. Semantics 33–37 occur in 363 bindings: 335 in permutations without
programs, 14 in OpenGL and 14 in Direct3D 10/11 permutations (for example 0x2B0000B7, whose
transforms are built from type-1, -3 and -2 items only, §7.7). **Tessera Decision (revised
2026-10-07):** evaluate both kinds through the item model of §7.7; the former rule "use 38–44,
ignore 33–37" loses the transforms of the records built from items of types 1–3.

Unbound parameters: ids whose `flag` is 1 are bound by name to the program constant of the
same name (§8.6), whether or not they also have binding entries (only 2013 has any, above); 4007 has no binding and no program reads it; the client reads it directly (§8.5).

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
| 8 | 39 (observed) | | render-state block | §7.6 (contains two float curves; 39 bytes when both are mode 1) |

Census over 31,519 permutations: `has_programs` = 0 in 541. `program_kind` 0 (27), 1
(6,918), 2 (7), 3 (7,041), 4 (16,985). The kinds are program back ends (Confirmed by the
blob contents, MA25): 0 Direct3D 9 shader model 1.1, 1 shader model 2.0, 2 shader model 3.0,
3 OpenGL (GLSL text after a name table), 4 Direct3D 10/11 shader model 4.0. `program_a` is the
vertex program, `program_b` the pixel program. A record carries each pass once per back end,
which is why permutation counts come in multiples. The `key` words and the pass code in word 3
are described in [`lighting.md`](lighting.md) §3. Tessera skips both blobs by their sizes and
never looks inside them.

The client reads `has_programs` as a **count** of program sets (BIN-R (j)): each set is
`program_kind`, `program_a` and `program_b`, followed by a third blob of the same form (u32 size,
bytes) when bit 10 (0x400) of key word 0 is set. On this install the count is 0 or 1 and bit 10
is clear in every permutation, which is why the layout above holds; a reader keeps rejecting
other counts (MT9) and a set bit 10 with programs, because their use is unknown.

<a id="s7-5"></a>
### 7.5 Sampler table (Confirmed layout)

| Field | Size | Meaning |
|---|---|---|
| `sampler_count` | u32 | 0–10 observed |
| per sampler: `name_length` | u8 | 0–28 observed |
| per sampler: `name` | `name_length` bytes, ASCII | Sampler name, may be empty |
| per sampler: `state` | 40 bytes | Sampler state (below) |

105,119 sampler entries. Sampler `state` (40 bytes; layout and meanings BIN-R (j), MA41; values
OBS, all entries; enumeration names from D3D):

| Offset | Size | Type | Field | Values (entries) | Meaning |
|---|---|---|---|---|---|
| 0 | 4 | DID | `default_texture` | 0 (104,961); 0x40000007 (126), 0x400163D4 (23), 0x40000E49 (9) | Texture list the sampler holds until a texture binding (semantic 10, §7.3.1) sets one; 0 = none |
| 4 | 4 | u32 | `source` | 0 (78,589), 2 (8,584), 10 (5,472), 11 (5,472), 5 (3,576), 9 (3,112), 8 (179), 1 (99), 3 (28), 12 (8) | 0: a material texture (every texture binding names a source-0 sampler; 263 source-0 samplers have no binding). Others are engine-supplied images; the sampler names they occur with (OBS) are 1 frame buffer, 2 light shadow depth, 3 reflection, 5 light projector, 8 depth-stencil, 9 shadow split map, 10 and 11 shadow cube index textures, 12 interactive water normals |
| 8 | 1 | u8 | `address_u` | 1 (69,351), 3 (32,648), 4 (3,120) | D3DTEXTUREADDRESS: 1 wrap, 3 clamp, 4 border |
| 9 | 1 | u8 | `address_v` | 1 (69,323), 3 (32,676), 4 (3,120) | same codes |
| 10 | 1 | u8 | `min_filter` | 2 (90,956), 1 (14,114), 3 (49) | **Minification** filter, D3DTEXTUREFILTERTYPE: 1 point, 2 linear, 3 anisotropic (corrected 2026-10-07: byte 10 is minification and byte 11 magnification; they are equal in every entry) |
| 11 | 1 | u8 | `mag_filter` | same as byte 10 | Magnification filter |
| 12 | 1 | u8 | `mip_filter` | 2 (82,254), 1 (22,786), 0 (79) | 0 none, 1 point, 2 linear |
| 13 | 4 | u32 | `max_anisotropy` | 0 in all | 0: the client's configured anisotropy |
| 17 | 1 | u8 | `color_op` | 3 (104,502), 4 (312), 5 (238), 7 (50), 2 (10), 8 (3), 13 (3), 6 (1) | Fixed-function texture-stage colour operation (D3DTEXTUREOP) |
| 18 | 4 | u32 | `color_arg1` | 2 (105,010), 1 (101), 0 (7), 3 (1) | D3DTA argument |
| 22 | 4 | u32 | `color_arg2` | 0 (104,603), 3 (360), 1 (156) | D3DTA argument |
| 26 | 1 | u8 | `alpha_op` | 3 (104,674), 4 (386), 2 (55), 7 (3), 5 (1) | Fixed-function alpha operation |
| 27 | 4 | u32 | `alpha_arg1` | 2 (104,998), 1 (105), 18 (11), 0 (4), 3 (1) | D3DTA argument |
| 31 | 4 | u32 | `alpha_arg2` | 0 (104,649), 3 (313), 1 (157) | D3DTA argument |
| 35 | 4 | u32 | `texcoord` | 0 (88,422), 3 (4,944), 4 (4,128), 1 (3,965), 2 (3,660) | Fixed-function texture-coordinate index (low byte) and coordinate-generation mode (bits 8–15, 0 in all) |
| 39 | 1 | u8 | `projected` | 0 in all | Fixed-function projected-coordinates flag, 0 or 1; the client rejects 2 or more |

Bytes 17–39 are texture-stage settings of the client's Direct3D 9 fixed-function path and byte
13 the anisotropy its Direct3D 9 path sets (BIN-R (j)); Tessera reads and ignores them. That
path also changes the stored filters by the client's texture-filtering option: linear minification and magnification become
anisotropic at the highest setting (when the device supports it), and a linear mip filter
becomes point at the lowest (BIN-R (j)).

For a given shader record and texture slot, every permutation that binds the slot uses the
same address and filter bytes (1,548 of 1,549 slots).

<a id="s7-6"></a>
### 7.6 Render-state block (Confirmed layout; meanings BIN-R (j), MA40)

The block is a sequence of fields; with both float curves in mode 1 (every block of this
install) it is 39 bytes. Census over all 31,519 blocks:

| Offset | Size | Type | Field | Values (blocks) | Meaning |
|---|---|---|---|---|---|
| 0 | 1 | u8 | `src_blend` | 5 (29,976), 2 (1,502), 1 (21), 9 (18), 7 (2) | Source blend factor, D3DBLEND: 1 ZERO, 2 ONE, 5 SRCALPHA, 7 DESTALPHA, 9 DESTCOLOR |
| 1 | 1 | u8 | `dst_blend` | 6 (19,895), 2 (10,125), 1 (1,484), 3 (7), 4 (4), 5 (4) | Destination factor: 6 INVSRCALPHA, 3 SRCCOLOR, 4 INVSRCCOLOR |
| 2 | 1 | u8 | `blend_op` | 1 (31,516), 3 (3) | D3DBLENDOP: 1 ADD, 3 REVSUBTRACT |
| 3 | 1 | u8 | `depth_func` | 7 (31,331), 8 (188) | **Depth comparison** (formerly read as an alpha comparison), D3DCMPFUNC for a **reversed** depth range: 7 GREATEREQUAL = "nearer or equal passes", 8 ALWAYS = no depth test. The client swaps 2 ↔ 5 and 4 ↔ 7 under one of its device modes (BIN-R (j)); reading that mode as "conventional depth range" is Inferred. A renderer with a conventional depth range does the same swap |
| 4 | 1 | u8 | `depth_write` | 1 (20,303), 0 (11,216) | Depth write, 0 or 1; the client rejects 2 or more |
| 5 | 1 | u8 | `cull` | 2 (31,169), 1 (350) | D3DCULL: 2 CW (back faces culled), 1 NONE (two-sided) |
| 6 | 1 | u8 | `alpha_test` | 0 (30,993), 1 (526) | Alpha-test switch, 0 or 1; the client rejects 2 or more |
| 7 | 8 | float curve (§5.3) | `alpha_ref` | mode 1 in all; value 0.9 (16,614), 0.8 (14,828), 0.1 (50), 0.6 (20), 0.0 (7) | Alpha-test reference (u32 mode 1 at 7–10, f32 at 11–14) |
| 15 | 4 | u32 colour | `material_color` | 0xFFFFFFFF in all but 6 blocks | Default material colour, packed 0xAARRGGBB (stored bytes B, G, R, A; each channel / 255) |
| 19 | 4 | u32 colour | `specular_color` | 0xFFFFFFFF in all | Default specular colour |
| 23 | 8 | float curve | `specular_power` | mode 1, 0.0 in all | Default specular power |
| 31 | 4 | u32 colour | `glow_color` | 0xFF000000 in all | Default glow colour (black, alpha 1) |
| 35 | 4 | u32 colour | `dye_color` | 0xFFFFFFFF in all | Default dye colour (white) |

The six blocks with another `material_color` belong to two shader records that declare no
parameters and are used by no surface record: 0x2B00053C (0x19FFFFFF, white with alpha 25/255)
and 0x2B00053D (0xFF000000, black), three blocks each.

These are the per-permutation values the bindings of §7.3.1 override: semantics 0–4 replace the
colours and the specular power, 5–9 the cull, depth comparison, depth write, alpha-test switch
and reference. So a shader record that declares no 2000 draws with `material_color` (white on
this install), and one that declares no 1000 with specular power 0, hence no highlight (§8.3).
A reader parses the two curves with the grammar of §5.3 (a curve of another mode makes the
block longer) and rejects the record if a flag byte is 2 or more.

Why byte 4 is read as depth write and not as blend enable: the permutations that are
clearly additive light passes (src SRCALPHA, dst ONE) have byte 4 = 0, which fits "do not
write depth" but not "blending off". The unlit additive shaders (src SRCALPHA, dst ONE)
have byte 4 = 1, which would make them opaque if byte 4 were "blending off". The client's
reader confirms it (BIN-R (j)).

Byte 4 by pass code (OBS, all blocks): 1 in every block of the base and shadow-depth passes
(codes 0x04, 0x05, 0x0A, 0x3E, 0x3F, 0x40), 0 in every SRCALPHA / ONE block of the additive
light passes (0x01, 0x02, 0x03; their 64 SRCALPHA / INVSRCALPHA blocks have 1), and 0 in the
glow pass (0x08) and the depth pass 0x09; this confirms "depth write" (`lighting.md` §2.1). Blending is not disabled by any byte: the lit base passes do
blend by their output alpha (§11.3).

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** byte 6 is the alpha-test switch, bytes 7–38 are
> the default alpha reference, colours and specular power above, and the depth comparison is
> byte 3.

<a id="s7-7"></a>
### 7.7 Extra state list: texture-coordinate transforms (Confirmed layout; meaning BIN-R (j), MA42)

| Field | Size | Meaning |
|---|---|---|
| `item_count` | u32 | 0 (18,633 permutations), 1 (8,322), 2 (4,526), 3 (5), 4 (1), 6 (29), 8 (3) |
| per item: `item_type` | u8 | 1–5 (below); 1, 2, 3 and 5 observed |
| per item: `stage` | u32 | The texture matrix the item changes: 0 (12,843 type-5 items, and items of types 1–3), 1 (4,524 type-5 items, and items of types 1–3), 2 (3 type-5 items) |
| per item: body | by type | float curves (§5.3 grammar) or floats, below |

| `item_type` | Body | Transform | Items |
|---|---|---|---|
| 1 | 2 curves: *t*ᵤ, *t*ᵥ | translation: u += *t*ᵤ, v += *t*ᵥ | 80 |
| 2 | 1 curve: θ in degrees | rotation by θ, counter-clockwise as in §8.4 step 3 | 67 |
| 3 | 2 curves: *s*ᵤ, *s*ᵥ | scale: u ×= *s*ᵤ, v ×= *s*ᵥ | 74 |
| 4 | 15 × f32 | the first 15 elements of a 4 × 4 matrix whose last element is 1 (the client's reader; never observed) | 0 |
| 5 | 7 curves, in this order: early translation U, early translation V, scale U, scale V, rotation (degrees), translation U, translation V | the full transform of §8.4 steps 1–4 | 17,370 |

The stored curves are all mode 1 (constant): 0.0 in every curve of types 1, 2 and 5 except 4
type-5 curves of 1.0, and 1.0 in every type-3 curve. (The earlier reading "*c* × (u32 *n*, *n*
× 4 bytes) with *n* = 1" was a mode-1 curve read as an array.) The bindings of §7.3.1 replace
these values: a type-5 item has no useful transform until its parameters are bound, since its
stored scale is 0 (§8.4).

**Composing a permutation's texture matrices (BIN-R (j)).** For each `stage` *K*, the client
starts from the identity at the first item of stage *K* and applies the items of stage *K* in
list order, each after the ones before it, every curve evaluated at the clock of §5.3.1. The
result is the permutation's **texture matrix *K***, the one the programs read (§8.4). Item
types 1, 2 and 3 in the order translation, scale, rotation, translation, which is how the older
records (for example 0x2B0000B7) build their transforms, give the same matrix as one type-5
item (Inferred: the type-5 item applies the same four steps in that order). A matrix that no item of the permutation changes is not reset by the client: it keeps the
value of the previous draw (BIN-R (j)). **Tessera Decision:** use the identity for such a matrix.
In the client's Direct3D 9 fixed-function path the same composition is applied per texture
stage, with `stage` naming the sampler index.

A reader must reject an unknown `item_type`, because its length is unknown.

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** the extra state items are the texture-coordinate
> transforms above.

---

<a id="s8"></a>
## 8. Parameter vocabulary

<a id="s8-0"></a>
### 8.0 Parameter labels (Confirmed, enum mapper 0x230001A6)

The install labels every parameter id in enum mapper 0x230001A6 (197 labelled ids plus 0
"Invalid"; 148 of them occur in materials or shader records, and no id used by a material or
shader record lacks a label; census corrected 2026-10-05). The labels are data;
implementations read them at run time for display and may key on the ids. Summary:

| Ids | Labels |
|---|---|
| 1000 | SpecularPower |
| 1001–1016 | UScale, VScale, UScale2, VScale2, … UScale8, VScale8 |
| 1017–1032 | UTranslate, VTranslate, UTranslate2, VTranslate2, … |
| 1033–1040 | UVRotate, UVRotate2, … UVRotate8 |
| 1041, 1042 | DetailMapUScale, DetailMapVScale |
| 1043 | AlphaTestRef |
| 1070–1085 | UTranslateEarly, VTranslateEarly, UTranslateEarly2, … |
| 1044–1069, 1100–1207 (except 1070–1085) | effect constants, each labelled with the program constant it feeds (`c_Water…`, `c_Mirror…`, `c_Window…`, `c_AEDistortion…`, `c_HeatDistortionAmount`, `c_DepthBlendFactor`, `c_TieDye_…`, `c_Wind_…`, `c_VtxDisp_…`, `c_TriplanarTextureScale`, `c_SnowBlending…`, `c_AlphaBlendMax`, `c_SpecularPower_AnisoLobe…`) |
| 2000 – 2003 | DiffuseColor, SpecularColor, ReflectionColor, DyeColor |
| 2004 – 2011 | `c_WaterDeepColor`, `c_WaterShallowColor`, `c_WaterSpecularColor`, `c_WaterNearColor`, `c_WaterFarColor`, `c_WaterColor`, `c_WaterFogColor`, `c_WaterShorelineColor` |
| 2013, 2014 | `c_MaterialDyeColor2`, GlowColor |
| 3000 – 3007 | DiffuseMap, DiffuseMap2 … DiffuseMap8 |
| 3010, 3011, 3013, 3014, 3015, 3016, 3017, 3018, 3019, 3020, 3021, 3022, 3024, 3026 | DetailMap, NormalMap, GlowMap, DetailNormalMap, EnvironmentMap, FakeSpecularEnvMap, VerticalPaletteMap, EmissiveMap, BakedLightingMap, OpacityAlphaMap, GlossAlphaMap, TerrainMaskTexture, DyeAlphaMap, DistortionMap |
| 3075 – 3079 | WaterFlowMap, TieDyeColorMap, SphereMap, NormalMap2, WaterFoamMap |
| 4000 – 4007 | Obsolete0, DepthTest, DepthWrite, DoubleSided, AlphaTest, DiffuseMapWrapU, DiffuseMapWrapV, AlphaBlendPass |

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
### 8.3 Colours and common scalars (labels Confirmed, §8.0; use in the programs BIN, `lighting.md` §4.1)

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
| 1174 | Soft-particle depth fade factor (label `c_DepthBlendFactor`) | Declared by 31 unlit effect shaders only | |

2000 multiplies every light colour and the ambient terms (so it tints the lit result) and its
alpha scales the output alpha (`lighting.md` §5.12). A material colour (0, 0, 0, 0) therefore
makes a lit surface black and fully transparent. 2002 ReflectionColor is declared by no shader
record of this install.

**What the client does before upload (BIN-T (f), MA32).** When it sets up a material for
drawing, the client:

- multiplies 2000 by a per-object tint (four floats supplied by the drawn object; white for
  ordinary objects, Inferred) and uses the product as *M* (`lighting.md` §4.2);
- evaluates the specular power 1000 at the clock of §5.3.1 and multiplies it by a global
  specular-power scale (an engine setting, Inferred to be 1);
- uses the specular colour 2001 times the same per-object tint, **except when the effective
  specular power is below 0.5**: then it uses black (0, 0, 0) as the specular colour, so the
  surface has no highlight;
- forms the ambient inputs as in `lighting.md` §8.1.

Where these values come from (BIN-R (j), MA40): the colours and the specular power are the
values of the drawn permutation, that is the render-state block's defaults (§7.6) replaced by
whatever 2000, 2001, 2014, 2003 and 1000 the bindings of §7.3.1 set. A shader record that does
not declare 2001 therefore uses the block's specular colour (white in every block of this
install) times the tint; one that does not declare 1000 has power 0 and no highlight. The curves
(1000 here, 1043 in §11.2) are evaluated at every draw.

All colours of group 2000 are stored as four f32 R, G, B, A (§5.2), so their channel order
needs no rule. Packed 4-byte colours in other records (the terrain type table, day
descriptions) use 0xAARRGGBB stored as bytes B, G, R, A (`terrain-texturing.md` §3.3,
`environment.md` §5.6; `lighting.md` §4.4).

<a id="s8-4"></a>
### 8.4 Texture-coordinate transforms (BIN; grouping Inferred from the census)

Floats that form a texture matrix for texture-coordinate set 0, 1 or 2. Grouping is
Inferred from the binding `semantic` values and from co-declaration with numbered
texture-matrix constants.

| Set 0 | Set 1 | Set 2 | Label (§8.0) | Typical values |
|---|---|---|---|---|
| 1001, 1002 | 1003, 1004 | 1005, 1006 | UScale, VScale | 1, 2, 3, 0.25 |
| 1017, 1018 | 1019, 1020 | 1021, 1022 | UTranslate, VTranslate; often an animated curve | mode 2 |
| 1033 | 1034 | 1035 | UVRotate, degrees | 0, 30, 90, 180, −30 |
| 1070, 1071 | 1072, 1073 | 1074, 1075 | UTranslateEarly, VTranslateEarly | −0.5 |

The first id of each pair is U and the second V (labels). Each set feeds one texture matrix
through the extra state item its bindings name (§7.3.1, §7.7): the item's `stage` is the
matrix number. Set 0 lands on matrix 0 and set 1 on matrix 1 in 89,821 and 31,782 bindings;
273 bindings put set 1 on matrix 0 and 42 put set 0 on matrix 1 (OBS; for example in the
glow pass of 0x2B000765 set 1 drives matrix 0, which its glow texture reads). **The matrix is
therefore chosen per permutation** (BIN-R (j), MA42): a renderer that follows a record's
transforms evaluates, for each permutation it draws, the items of that permutation.

Which matrix a sampler reads is fixed by the programs (BIN-S4, MA47; Direct3D 10/11 programs of
every record with such samplers):

- the base-colour layer reads matrix 0 and the second colour layer matrix 1, both applied to
  the same vertex UV pair; the normal map uses the base-colour coordinates (`lighting.md`
  §5.2.1, BIN);
- `Emissive` (3018) and `Glow` (3013) read matrix 0, except: `Emissive` reads matrix 1 in the
  colour passes of 0x2B0009CE and 0x2B0009CF and in the pass-0x00 program of 0x2B00070D;
  `Glow` reads matrix 1 in the glow pass (0x08) of 0x2B000739, 0x2B000812, 0x2B000813,
  0x2B0008A8 and 0x2B0009E0;
- `EmissiveAndOpacity` (3018) reads matrix 1 in every pass, except in 0x2B000772, whose glow,
  depth, shadow-depth and view-normal passes (0x08, 0x09, 0x0A, 0x42) read matrix 0.

> **Answered 2026-10-07 (BIN-R (j), BIN-S4; was OPEN):** the matrix used by the glow and
> emissive samplers is the list above; the per-permutation binding of the second set to matrix 0
> is the client's behaviour (§7.7).

**Composition (BIN-T (a), MA27).** The client builds each texture matrix from its seven
values (§7.3), each a float curve evaluated at the clock of §5.3.1. From the input
coordinates (u, v), in this order:

1. **early translation:** u₁ = u + *e*ᵤ, v₁ = v + *e*ᵥ (UTranslateEarly, VTranslateEarly);
2. **scale:** u₂ = *s*ᵤ · u₁, v₂ = *s*ᵥ · v₁ (UScale, VScale);
3. **rotation** by θ = UVRotate · π / 180, counter-clockwise in the (u, v) plane:
   u₃ = cos θ · u₂ − sin θ · v₂, v₃ = sin θ · u₂ + cos θ · v₂;
4. **translation:** u′ = u₃ + *t*ᵤ, v′ = v₃ + *t*ᵥ (UTranslate, VTranslate).

As one affine map: u′ = cos θ · *s*ᵤ · (u + *e*ᵤ) − sin θ · *s*ᵥ · (v + *e*ᵥ) + *t*ᵤ and
v′ = sin θ · *s*ᵤ · (u + *e*ᵤ) + cos θ · *s*ᵥ · (v + *e*ᵥ) + *t*ᵥ. The programs read it as
u′ = m00·u + m01·v + m03 and v′ = m10·u + m11·v + m13 (`lighting.md` §5.2.1), with m00 =
*s*ᵤ cos θ, m01 = −*s*ᵥ sin θ, m10 = *s*ᵤ sin θ, m11 = *s*ᵥ cos θ, m03 and m13 the
translation terms above. The client appends this transform to whatever the matrix already
holds (identity for a fresh matrix); it keeps the translation in the third row for the
fixed-function path and in the fourth for the programs, with the same result.

A matrix value that its record does not declare keeps the stored curve of its item (§7.7):
0 for every type-5 value on this install except four curves of 1.0, so an undeclared scale would
collapse the coordinates; every type-5 item that semantics 38–44 name has all seven declared
(§7.3.1).

Rotation about the texture centre: in 136 materials an early translation of −0.5 is paired
with a translation of +0.5 on the same axis of the same set, which rotates and scales the
texture about (0.5, 0.5). Counted as pairs of material entries (constant curves, OBS, census
of 2026-10-06 over all 24,091 materials): 264 pairs, 205 on U (set 0: 1070/1017, 92; set 1:
1072/1019, 113) and 59 on V (set 0: 1071/1018, 25; set 1: 1073/1020, 34); none on set 2.
The earlier figure "205 material entries" counted the U pairs only. The terrain layers use the same routine with the early translation 0
(`terrain-texturing.md` §6.3).

<a id="s8-5"></a>
### 8.5 Flags (group 4000)

| Id | Label (§8.0) | Rule | Census (surface records, effective value) | Mark |
|---|---|---|---|---|
| 4001 | DepthTest | In each permutation its bindings name: 1 = depth comparison 7, 0 = 8 (no depth test) (§7.3.1) | 1: 13,554; 0: 203 | Label Confirmed; rule BIN-R (j) |
| 4002 | DepthWrite | In each permutation its bindings name: depth write := value, both ways (§7.3.1) | 1: 11,699; 0: 2,058 | Label Confirmed; rule BIN-R (j) |
| 4003 | **DoubleSided** | In each permutation its bindings name: 1 = cull NONE, 0 = cull CW (§7.3.1, §11.4) | 1: 2,119; 0: 11,633 | Label Confirmed; rule BIN-R (j) |
| 4004 | **AlphaTest** | In each permutation its bindings name: alpha-test switch := value (§7.3.1, §11.2) | declared together with 1043 in all 307 records | Confirmed; rule BIN-R (j) |
| 4005, 4006 | DiffuseMapWrapU, DiffuseMapWrapV | 1 = wrap (1), 0 = clamp (3), for the address mode U / V of the sampler each binding names by (`permutation`, `slot`), over the stored byte whatever it is (§7.3.1) | U 1: 12,258, 0: 1,455; V 1: 12,163, 0: 1,550 | Label Confirmed; rule BIN-R (j) |
| 4007 | AlphaBlendPass | no binding and no program input; the client reads it directly: 1 = draw the object a second time to soften cut-out edges (below) | 1: 654; 0: 12,952 | Label Confirmed; rule BIN-R (k) |

Values are 0 or 1. The defaults come from the shader record. The sampler tables' stored
address bytes (§7.5) of slot 3000 are wrap in 453 of the 457 records that declare 4005 and
3000, including 36 of the 40 whose default 4005 is 0, so the flags, not the stored bytes,
decide the wrap mode (BIN-R (j): the flag's value replaces the sampler's address mode).

**4007 AlphaBlendPass (BIN-R (k), MA43).** It is 1 on 654 surface records, 573 of them lit
`ColorAndOpacity` surfaces with alpha test on (foliage cards, grilles). The client reads 4007
directly, without bindings: an effective 4007 = 1 sets a flag of the surface's effect, and an
object with any such surface is put on a separate list when the scene is culled. After the main scene draw, while a
render-configuration flag of the client is set, the client draws every object of that list a
second time with a device mode in which, for every permutation of every surface of the object:

- alpha test is off;
- depth write is off;
- the depth comparison is 5 (GREATER on the reversed depth range of §7.6, that is strictly
  nearer than what is stored);
- the blend state, programs and parameters are those of the normal draw (Inferred: the device mode changes only the three states above).

The texels the alpha test kept in the first draw wrote their depth and fail the strict test;
the texels it removed are drawn now and blended by their output alpha (source-alpha blend,
§11.3) over what lies behind. Inferred effect: soft, semi-transparent edges around cut-out
foliage instead of hard ones. **Tessera rule (Decision):** a renderer that wants the original's edges draws
these surfaces a second time in the transparent phase with exactly these three changes.

> **OPEN:** Which user setting sets the render-configuration flag that enables the second
> draw (a member of the client's scene-render configuration object; the setting that writes it
> was not identified). Owner observation can settle it: compare foliage edges in the original
> with each graphics option on and off.

<a id="s8-6"></a>
### 8.6 The per-entry `flag` byte (Confirmed values and meaning)

`flag` is fixed per `id`. It is 1 for the effect-specific parameters that occur in the data
(water 1044–1069, 1170, 1171, 1173, 1182–1190, 2004–2006 and 2009–2011, consistent with
`water.md` §8.2; mirror 1100–1106; window 1120–1127; distortion 1140–1149; heat distortion
1160; depth fade 1174; tie-dye 1175–1177; wind 1178–1180; vertex displacement 1191–1199; snow
and triplanar 1200–1202; 1203 (`c_AlphaBlendMax`, terrain layers); anisotropic specular 1204–1207; second
dye colour 2013) and 0 for every other id (Confirmed, all declarations and material entries,
re-checked 2026-10-05; the earlier ranges "1170–1190" and "1140–1160" overlapped other
families). Ids 1172, 1181, 1150–1159 have no label; 2007 and 2008 are labelled water colours
that no record uses. No flag-1 parameter has binding entries except 2013, which has one
semantic-4 binding in 7 of its 11 records; that binding names a permutation without programs
(§7.3), so for every program the flag-1 parameters, 2013 included, are reached by name only.

Meaning (Confirmed, MA19): `flag` = 1 exactly for the 95 ids whose label (§8.0) starts
with `c_`; such a parameter feeds the program constant whose name equals its label, so it
needs no binding entries to reach a program (the one exception that has any, 2013, is
described in §7.3). `flag` = 0 for the other 53 ids.

How the client does it (BIN-R (j)): for every entry whose `flag` is 1 it turns the id into its
label through the parameter-label enumeration and looks that name up among the constants of
the shader record's programs; when it is found, a value of group 1000 (a float curve) or 2000 (a
colour) is written to that constant. Entries of groups 3000 and 4000 are never passed by name
(none has `flag` = 1).

<a id="s8-7"></a>
### 8.7 Effect-specific parameters

Ids from 1044 up (except the early translations 1070–1085, §8.4) and 2004–2011 belong to special effect families:
water, mirrors, windows, heat distortion, wind sway, vertex displacement, snow blending,
anisotropic hair specular. They occur on few surfaces (water: 98 surfaces). M2 renders
these surfaces with the generic pipeline chosen by §11 and ignores the extra parameters.

Water: the parameters the water programs read and how they combine are in `water.md`
§8.2–§8.4. The terrain layers use parameter 1203 (`c_AlphaBlendMax`) as the weight of the
squared mask (`terrain-texturing.md` §7, §8.2).

Tie-dye (1175–1177): answered in §12.5.

> **OPEN:** Per-parameter meanings of the other effect families: mirrors (1100–1106), windows
> (1120–1127), distortion (1140–1149), heat distortion (1160), wind sway (1178–1180), vertex
> displacement (1191–1199), snow and triplanar blending (1200–1202) and anisotropic specular
> (1204–1207), and the water flow, foam and depth parameters left open by `water.md`. Each is
> answerable from the programs of the few records that declare it (BIN-S); none is needed by
> the generic pipeline of §11.

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

How the client layers (BIN-R (m), MA45): it loads the surface record's shader record and its
materials in stored order, builds the effect from the shader record's defaults, then applies
every entry of every material in order: it looks for the first declaration with the entry's
`id` (an `id` the shader does not declare is skipped) and applies the value through that
declaration's bindings (§7.3.1); entries with `flag` = 1 are also passed by name (§8.6). A later
entry that sets the same state therefore wins. No material is chosen by any state.

> **Answered 2026-10-07 (BIN-R (m); was OPEN):** surface records whose materials set
> conflicting textures use the last one (last-wins); the client does not select a material by
> season, damage or variation.

Broken references (BIN-R (m)): a material DID that does not load is reported by the client and
left out of the list, the others keep their order; a shader record that does not load is
reported and the surface gets no effect, so the client has nothing to draw it with.
**Tessera rule (Decision, unchanged):** leave a missing material out of *L*; fail the submesh
when its shader record is missing.

<a id="s9-3"></a>
### 9.3 Appearance overrides (Confirmed)

Worn items, mounts and character parts are assembled from appearance records (0x20). Their
layout is normative in `setup.md` §8 (blocks §8.3, with the field attribution of
`appearance.md` §4.3); this probe parsed 4,199 of 4,201 records with the grammar of DS §7.2. The part relevant here: a **block** carries a list of **material
groups**, and each group has five fields:

| Field | Type | Meaning |
|---|---|---|
| `shader_override` | DID or 0 | 0 in 62,110 groups; a 0x2B DID in 22,151 |
| `material` | DID | A 0x30 material (83,984 of 84,261 resolve) |
| `a` | u32 | The target slot: a `MeshTypeID` slot key, or 2 = `All` (`setup.md` §8.3, `appearance.md` §4.10) |
| `binding_key` | u32 | Matched against surface records |
| `g` | f32 | Priority of the **next** group of the block (`setup.md` §8.3) |

Binding rule (BIN-R (i), MA38): the client applies a group to **every part of the drawn entity**
(every mesh of the assembled figure or object, not only the block's own parts) whose part key
equals the group's `a`, and on such a part to every surface record whose `binding_key` equals
the group's `binding_key`:

- `a` = 2 matches every part; `a` = 0 matches none (the group does nothing);
- `binding_key` = 2 matches every surface record of the part;
- the part key is the part's `MeshTypeID` slot (Inferred: it is the key the client returns for a
  picked part and the census below fits it).

Census (OBS, 2026-10-07; groups that come with part meshes): of 47,895 groups, 46,801 match a
surface record of their own block's parts under this rule and 87 more do so only through
`binding_key` = 2; 84 groups whose key alone matches a surface of their own block name in `a` a
slot that the matching part does not have, so they act only on other parts of the entity; 923
match nothing in their own block and likewise act only on other parts (the wearer's body or
other worn items). The former count "46,885 of 47,895 (97.9%)" compared keys only. Over all
84,261 groups, `binding_key` is 1 in 894, 2 in 406 and 0 in 1. For a matched submesh:

- *H* = `shader_override` if non-zero, else the surface record's shader;
- *L* = the surface record's materials, then the group's material, then any dye material
  (§12.3).

The surface record's own materials are the mesh's default look. An appearance group
layered on top gives the item's look.

How a matched group changes the surface (BIN-R (i)): a non-zero `shader_override` replaces the
surface's shader record, and the group's material is appended after the materials the surface
already has (its own, then those appended earlier), so the layering of §9.2 applies.

> **Answered 2026-10-07 (BIN-R (i); was OPEN):** the 1,010 groups whose key matches no surface
> of their block's parts are matched against all parts of the entity (rule above). The fields
> `a` and `g` are answered by `setup.md` §8.3; how group priorities combine stays OPEN in
> `appearance.md` §4.10.

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

Normal maps (BIN, MA23): the tangent-space vector is (2r − 1, 2g − 1, 2b − 1), normalised;
it is expressed in the frame (stored tangent, **negated** stored binormal, vertex normal), so
the shading normal is x·T − y·B + z·N (`mesh.md` §5.6: T = +U, B = +V). Positive green tilts
the normal toward the top of the image (decreasing V). The alpha channel is gloss or dye mask
by role, as tabled above.

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
- **Threshold** = the effective 1043 (§5.3 evaluation). The programs discard a fragment when
  opacity ≤ threshold, so a fragment is kept only when opacity **>** threshold (BIN, MA22).
  Render-state byte 3 is the depth comparison, not an alpha comparison (§7.6); the block's
  `alpha_ref` (0.8 or 0.9) is the reference of permutations that 1043 does not bind (corrected
  2026-10-07, BIN-R (j)). Every record that declares 4004 also declares 1043 (MT8), so on this
  install 1043 decides; the client adjusts it per draw (below).
- With two colour layers (`ColorAndOpacity` and `ColorAndOpacity2`) the opacity is the
  product of both alphas. Vertex-colour alpha (where read, `lighting.md` §5.13) is applied
  after the test, so it never removes texels.
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

Rule: with no opacity source, alpha test is off. Confirmed by the programs (BIN): the shader
record behind the 142 surfaces (0x2B0009DA) and the next one (0x2B000A22) read the
threshold only in their view-space-normal pass (0x42), never in a colour pass.

**What the client does per draw (BIN-R (l), MA44).** For each permutation it draws, the client
takes that permutation's alpha-test switch and reference (render-state bytes 6 and 7–14,
replaced by 4004 and 1043 through their bindings, §7.3.1), evaluates the reference curve at the
clock, and then, with *A* = the material colour's alpha × the per-object tint's alpha (§8.3):

1. If alpha test is on and the client's alpha-to-coverage option is on (whether or not the
   frame is multisampled), it keeps the reference as it is and skips step 2.
2. Otherwise, if alpha test is on and the permutation writes depth, the reference becomes
   reference × *A*; if alpha test is on and the permutation does not write depth, alpha test is
   switched off for the draw when *A* < 0.9.
3. The reference handed to the programs is ⌊reference × 255⌋ / 255 when alpha test is on
   (for example 0.65 becomes 165 / 255 ≈ 0.647; 0.8 stays 204 / 255 = 0.8). When alpha test is
   off but the permutation's `src_blend` is 5 (SRCALPHA), the reference is 0, so fragments with
   alpha 0 are discarded in blended passes; otherwise it is −1 (nothing is discarded). The
   programs discard when the tested value is ≤ the reference (MA22).
4. **Alpha-to-coverage:** when the client's alpha-to-coverage graphics option is on, the frame
   is multisampled and the permutation alpha-tests, the client also enables alpha-to-coverage
   for the draw.

The Direct3D 9 fixed-function path uses the same reference as an 8-bit alpha-test value with
the comparison "greater" (BIN-R (l)). **Edge softening:** the original softens cut-out edges
with the second draw of 4007 AlphaBlendPass (§8.5) and, when the user enables it with
multisampling, with alpha-to-coverage.

> **Answered 2026-10-07 (BIN-R (k), (l); was OPEN):** the client uses both: alpha-to-coverage
> (an option, multisampling only) and the 4007 second draw (§8.5).

> **OPEN:** Two per-draw conditions of the client were not identified. (1) A mode supplied by
> the caller of the per-draw setup: with it, alpha test and depth write are switched off when
> the client's material-detail option is at its lowest setting or alpha test is already off
> (only for permutations whose `dst_blend` is not ONE), and an alpha-tested reference becomes
> reference + (1 − reference) × (1 − *A*) with *A* then taken as 1. (2) A device state bit
> under which a blended pass without alpha test gets reference −1 instead of 0. Both need
> further disassembly of the callers (method M4); owner observation of fading objects in the
> original could tell whether (1) is the fade-out path.

<a id="s11-3"></a>
### 11.3 Blend mode (layout Confirmed, rule Inferred)

Classify the shader record first:

- **Lit**: at least one permutation's render state has `src_blend` = 5, `dst_blend` = 2
  and `depth_write` = 0 (additive per-light passes). 158 records, 11,639 surface records.
- **Unlit**: all others. 352 records, 2,142 surface records.

| Family | (src, dst) pairs in its render-state blocks | Tessera blend mode | Records (surface records) |
|---|---|---|---|
| Lit | base passes SRCALPHA / INVSRCALPHA with depth write on, plus the light passes | **Source-alpha blend** with the output alpha of the lit base pass; opaque pass when that alpha is 1 by construction, transparent pass otherwise; alpha test as §11.2 (rule at the end of this section) | 158 (11,639) |
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

Lit base passes (BIN, MA22): the programs output alpha = (opacity texel when the base role
carries opacity) × the alpha of material colour 2000; a gloss or dye-mask alpha never reaches
the output. With the SRCALPHA / INVSRCALPHA factors the base pass therefore blends:
- opaque roles (`Color`, `ColorAndGloss`, `ColorAndDye`) with 2000 alpha 1 are opaque;
- alpha-tested `ColorAndOpacity` surfaces keep texels with opacity > threshold and blend them
  by that opacity (slightly translucent just above the threshold);
- `ColorAndOpacity` surfaces with alpha test off are translucent by their texture alpha
  (153 lit surface records);
- a 2000 alpha below 1 makes any lit surface translucent (0 makes it invisible).

Rule (Decision, revised 2026-10-05; the table row above states the same rule): draw lit
surfaces with the source-alpha blend and the output alpha above, in the opaque pass when the
output alpha is 1 by construction (opaque role and 2000 alpha 1) and in the transparent pass
otherwise. Alpha-tested `ColorAndOpacity` surfaces therefore belong to the transparent pass:
the test removes texels at or below the threshold and the kept texels blend by their opacity,
as in the original. (The former table entry "opaque, or cut-out when alpha test is
on" was the M2 rule and is withdrawn.)

<a id="s11-4"></a>
### 11.4 Two-sided

- **Effective 4003 DoubleSided = 1 makes the surface two-sided** (label Confirmed, §8.5;
  4003 is bound to the colour passes with binding semantic 5). 2,119 surface records: 1,104
  unlit `ColorAndOpacity` effects, 616 lit alpha-tested `ColorAndOpacity` surfaces (foliage),
  235 unlit `Color`, and others.
- Also, if the most common `cull` value among *H*'s render-state blocks is 1 (NONE), the
  surface is two-sided. That holds for 38 records (120 surface records, mostly water and
  unlit effects); 350 blocks in all have `cull` = 1.
- Otherwise back faces are culled. Front faces are counter-clockwise in a right-handed
  frame: the triangle normal (p1 − p0) × (p2 − p0) agrees with the stored vertex normals for
  99.4% of 361,885 sampled triangles (cross-check with `mesh.md`).

What the client does (BIN-R (j)): each permutation has its own cull (block byte 5), which an
effective 4003 replaces in the permutations its bindings name: 1 → NONE, 0 → CW, both ways
(§7.3.1). Compared with the rule below over the colour permutations (pass codes 0x00, 0x04,
0x05, 0x3E–0x40) of the Direct3D 10/11 back end: 11,651 surface records are one-sided and 2,111
two-sided in every such permutation, as the rule says, and 19 that the rule calls two-sided are
two-sided in only some of them (OBS, 2026-10-07).

Rule (revised 2026-10-05; kept 2026-10-07 as the record-level approximation): two-sided =
effective 4003 = 1, or the record's most common cull is NONE. A renderer that keeps per-pass
state uses the per-permutation cull instead. 4007 is **not** the two-sided flag (it is AlphaBlendPass, §8.5); the earlier M2
decision is withdrawn. The programs do not flip the normal for back faces (no front-face
input is read), so a back face is lit with the front face's normal (BIN).

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

Specular (BIN): Blinn-Phong per light, gloss × sat(N·H)^1000 × (light specular colour × 2001),
added after the lit colour's factor 2; the sun's term is scaled by the shadow factor, point
lights' by their attenuation (`lighting.md` §5.8). The client uploads a black specular colour
when the effective power 1000 is below 0.5 (§8.3, BIN-T (f)), so a `specular` surface with
power 0 shows no highlight in the original. Environment map (slot 3015, role
`Environment`): sampled with the reflected view vector, multiplied by gloss and **added to the
base colour before lighting** (`lighting.md` §5.10).

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

The smooth form is what the programs compute (BIN, MA24), on the sampled texture values,
before lighting; the dye colour's alpha is not used and there is no brightness boost.

Colour space (BIN-T (g), MA33; answers the former OPEN): no graphics back end creates sRGB
textures or an sRGB frame buffer, so the original applies this blend, and all lighting, to the
stored 8-bit values as they are (`lighting.md` §5.1, §15). An implementation that samples
through sRGB views must convert to reproduce the original.

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

> **OPEN:** The client's rule when a dye code has no block of its own (the Tessera rule above is
> a Decision). It lies in the client's appearance code that picks a block by its `variant`
> (`setup.md` §8.3), which this pass did not reach; answerable by disassembly (M4) or by owner
> observation of an item dyed with a code its appearance record lacks. (Where an item's default
> dye code comes from is answered in `appearance.md` §4.9: the item's clothing-colour
> properties.)

<a id="s12-5"></a>
### 12.5 Tie-dye

Two shader records bind a `TieDyeColor` ramp (3076) with parameters 1175–1177 (21 surface
records): 0x2B00072F and 0x2B00076E. Neither declares 2003: the ramp replaces the dye colour.
M2 treated them as `ColorAndDye` with the ramp ignored.

What their programs compute in the colour passes (BIN-S4, MA48), per fragment, before lighting:

1. *N* = the normal-map vector (2 × texel − 1 per channel, **not** normalised at this point);
   *V* = the unit vector from the vertex toward the eye, expressed per vertex in the frame
   (tangent, −binormal, normal) of §10.2 and interpolated without renormalising.
2. *f* = min((saturate(*N* · *V*))^*P*, 1) with *P* = 1175 (`c_TieDye_Power`; default 1, material
   values such as 0.8).
3. Ramp colour *R* = the RGB of slot 3076 sampled at (*f* + *O*, 0.5) with *O* = 1176
   (`c_TieDye_Offset`), using the sampler's address mode.
4. *w* = saturate(*f*² × *W*) with *W* = 1177 (`c_TieDye_Whiteness`); dye colour *D* = *R* +
   *w* × (1 − *R*) (toward white where the surface faces the viewer).
5. Dyed colour as in §12.3 with *d* = *D* and the mask *a* = the `ColorAndDye` alpha:
   *a* · *c* + (1 − *a*) · (*c* × *D*).

The ramp is therefore indexed by a view-dependent term: the colour shifts as the surface turns
(an iridescent cloth). Names in backticks are the labels of §8.0, given for interoperability.

> **Answered 2026-10-07 (BIN-S4; was OPEN):** how the tie-dye ramp is indexed (steps above).

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
| MT9 | `has_programs` ∈ {0, 1}; extra item types ∈ {1, 2, 3, 5} | Confirmed, all (reject others: unknown length; type 4 has a known length, §7.7, but never occurs) |
| MT10 | 0x31: `shadow_receiver` and `not_pickable` ∈ {0, 1} | Confirmed, all 13,781 (2026-10-07) |
| MT11 | Render-state block: `depth_write` and `alpha_test` ∈ {0, 1}; both curves mode 1, so 39 bytes | Confirmed, all 31,519 (2026-10-07) |
| MT12 | Every binding with semantic 33 or 34 names an item of type 1, 35 of type 2, 36 or 37 of type 3, 38–44 of type 5 (§7.3.1) | Confirmed, all 121,939 (2026-10-07) |
| MT13 | Every texture binding (semantic 10) names a sampler whose `source` is 0 | Confirmed, all 78,326 (2026-10-07) |

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

Status of the former frame-capture questions after the M4 pass of 2026-10-05 (BIN-S, BIN-C,
EM):

1. Render-state byte 4 (depth write) and blending: answered (§7.6, §11.3: lit base passes
   blend by output alpha). Byte 6 answered 2026-10-07: alpha-test switch (§7.6).
2. Two-sided: answered, flag 4003 DoubleSided (§11.4); 4007 is AlphaBlendPass, answered
   2026-10-07: a second draw that softens cut-out edges (§8.5); the setting that enables it is
   OPEN.
3. Alpha test without an opacity texture: answered, off (§11.2). Edge softening answered
   2026-10-07: 4007 second draw and optional alpha-to-coverage (§11.2).
4. Dye blend: answered, smooth form on sampled values, dye alpha unused (§12.3). Colour
   space answered 2026-10-05: stored values, no sRGB (§12.3, BIN-T (g)).
5. Lighting model, specular, environment map, normal-map convention: answered
   (`lighting.md` §5; §10.2, §11.5). Specular power below 0.5: no highlight (§8.3).
6. Texture matrices: U/V assignment and matrix per set answered (§8.4); composition answered
   2026-10-05 (§8.4, BIN-T (a)).
7. Float-curve waveforms: answered (§5.3.1); noise generators and the clock answered
   2026-10-05 (BIN-T (d), (e)); mode 3's random sequence cannot be reproduced (Decision).
8. Flags 4001–4006: answered by their labels (§8.5).

Answerable from data with more probing:

9. The surface-record `tail` u16 and `binding_key` values 1 and 2: answered 2026-10-07
   (§4.1, §4.2).
10. Surface records listing alternative materials: answered 2026-10-07, last wins (§9.2).
11. The 1,010 unmatched appearance groups: answered 2026-10-07 (§9.3). The dye-block fallback
    rule (§12.4) stays OPEN. Fields `a` and `g`: `setup.md` §8.3; the default dye property:
    `appearance.md` §4.9.
12. `slot` of non-texture bindings, extra state items, sampler state bytes: answered 2026-10-07
    (§7.3.1, §7.5, §7.7). The permutation `key` bits are described in `lighting.md` §3.
13. Which record types reference the 353 unreferenced surface records (§3.2).
14. Effect-family parameters (wind, snow, …) (§8.7). Water: `water.md` §8.2–§8.4. Tie-dye:
    answered 2026-10-07 (§12.5).

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

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** Ties. §10.1 (role name) and §11.3/§11.4 ("most common" triple, cull value and
> `alpha_ref`) do not say how ties are broken. The implementation takes the candidate met
> first (binding order; permutation order).
> Answer: the original has no "most common" choice: every permutation is drawn with its own block (§7.6) and bindings (§7.3.1). The record-level choices of §10.1, §11.3 and §11.4 are Tessera approximations; the implementation's tie-break (first met in binding order, then permutation order) is adopted as the Decision.

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** §11.3 says depth write "follows `depth_write` of the same block", but several
> blocks may carry the chosen triple with different `depth_write`. The implementation takes
> the majority among those blocks, ties to "write".
> Answer: in the original each permutation writes depth according to its own byte 4 (and 4002 where bound). For the record-level approximation the implementation's majority rule, ties to "write", is adopted as the Decision.

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** Alpha-test threshold when the shader declares 4004 but not 1043 (MT8 says this
> never happens). The implementation falls back to the most common render-state
> `alpha_ref`, then 0.5. A non-finite 1043 is treated the same way.
> Answer: a permutation that 1043 does not bind uses its block's `alpha_ref` (bytes 7–14, §7.6), so the fall-back to the block value is the original's rule; take it from the permutation drawn (or, record-level, the most common value). The final 0.5 fall-back and the treatment of a non-finite 1043 stay Decisions.

> **OPEN (narrowed 2026-10-07):** §11.2 picks the opacity source by role only. When the
> effective texture of that slot is 0 (4 material entries set a texture to 0), the
> implementation still enables alpha test with that slot as source; a renderer has no texture
> to test and should treat opacity as 1. Known (BIN-R (j), (l)): a texture value 0 clears the
> sampler's texture, and the per-draw alpha-test setup does not look at textures, so the client
> does not switch the test off. Not known: what the back ends bind and sample for a sampler
> without a texture (an empty binding would read 0 and discard every fragment; a default white
> image would keep them). Answerable by disassembly of the back ends' texture binding (M4) or
> by owner observation of the 4 affected surfaces.

> **Answered 2026-10-05 (§20.1, row 1; was OPEN):** §11.3 maps unlit records to a fixed blend but does not say whether alpha test
> (§11.2) also applies to blended surfaces. The implementation reports both independently:
> an unlit ONE / ZERO record with alpha test on becomes cut-out; a blended record keeps its
> blend mode and also carries the alpha test.

> **Answered 2026-10-05 (§20.1, last row; was OPEN):** §12.3 says the second dye channel (2013) uses "the mask from the second dye-mask
> source" without ordering the sources. The implementation orders them: 3000 with a `Dye`
> role, 3011 with a `Dye` role, 3024 (declared), 3001 with a `Dye` role. The primary channel
> uses the first; the second channel uses the second, or the first when there is only one.
> On current data 243 surface records declare 2013.

> **Decision (implementation), colour-space note answered 2026-10-05:** Dye preview details
> not covered by §12.3: the output alpha is the diffuse alpha only when the diffuse role's
> alpha is opacity, otherwise 255 (the alpha is a mask or gloss); a mask texture of another
> size than the diffuse is sampled nearest-neighbour; the blend is applied to the stored 8-bit
> values. The former "gamma vs linear is already OPEN" is answered: the original uses stored
> values with no sRGB conversion (§12.3, MA33), which is what this preview does.

> **Answered 2026-10-07 (BIN-R (m); was OPEN):** A shader record that declares the same id twice (never observed) keeps the first
> position and the last value, like a material layered on itself. Value type 4 is read as a
> texture DID when the *id* is in the 3000 group and as a flag word otherwise, whatever the
> stored `group` field says (a mismatch is reported as unusual).
> Answer: the client applies a material entry through the **first** declaration with that id (it scans the declarations in stored order and stops at the first match, §9.2); material values reach only the bindings of that first declaration. Reading value type 4 by the id's group is the Decision.

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** Blend-factor and blend-op codes that §7.6 does not name (for example 8, 10, 11
> and op 2) are reported as "other" with their raw code; the renderer needs a fallback. None
> occurs in the most common triple of any record whose surfaces were resolved.
> Answer: the codes are the Direct3D 9 enumerations; the client's Direct3D 9 back end passes them to the device unchanged. Factors: 8 INVDESTALPHA, 10 INVDESTCOLOR, 11 SRCALPHASAT (12–15, the BOTH… and BLENDFACTOR codes, occur in no block). Operations: 2 SUBTRACT, 4 MIN, 5 MAX. A renderer maps them to its equivalents and treats anything else as invalid input.

> **Answered 2026-10-07 (BIN-R (m); was OPEN):** Material resolution policy for broken references: a material that is missing or
> does not decode is left out of *L* (and reported); a missing or undecodable shader record
> fails the whole submesh. The original's behaviour is unknown; current data has no such
> reference.
> Answer: the policy matches the client: a missing material is reported and left out, a missing shader record leaves the surface without an effect (§9.2).

> **OPEN:** `MaterialLibrary::texture` shows a cube map as its first stored face (face
> order is OPEN in `texture-list.md` §3.2). `MaterialLibrary::cube_faces` returns all six
> faces of a single-level cube in stored order; a cube with more than one level (never
> observed, array order OPEN) is not split. (The face order is owned by `texture-list.md` §3.2;
> this callout only records the implementation's choice until that spec answers.)

> **Answered 2026-10-07 (BIN-S, BIN-S4; was OPEN):** Slot-3000 role names outside the §10.2 table occur on current data:
> `ColorTexture` (4 surface records), `LiquidDiffuseTexture` (12) and
> `DecalDynamicLightTexture` (1); 21 more have no named binding. The implementation gives
> names outside the table "other" RGB and "unknown" alpha (sampled as plain colour, §10.1),
> except names starting with `Diffuse`, which it reads as base colour with alpha unused.
> Whether `ColorTexture` and `LiquidDiffuseTexture` carry opacity in alpha is unknown.
> Answer: `ColorTexture` and `LiquidDiffuseTexture`: their alpha reaches the output alpha (§20.1); `ColorTexture` outputs 2 × the product of both layers' alphas and the 2000 alpha. `DecalDynamicLightTexture` (0x2B000544): the program outputs texel × vertex colour (RGBA) and the record blends DESTALPHA / ONE, so the texel alpha does not act as opacity. Samplers without a name occur only in permutations without programs (465 entries, OBS); the implementation's "other" / "unknown" treatment is adopted as the Decision.

Added after review of the implementation (2026-10-04):

> **Revised 2026-10-05 (§20.1, row 2; was OPEN, Decision):** Opacity of unlit blended records
> whose diffuse role has no opacity in alpha. §11.3 takes "source alpha" from the §11.2 opacity
> source, or 1 without one. 522 surface records that declare slot 3000 (and 15 that do not)
> are unlit, transparent, use SRCALPHA or INVSRCALPHA, and have no §11.2 source: their slot-3000 role is `Color` (463),
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

> **Decision (adopted 2026-10-07; was OPEN):** Bounds on resolution, for hostile input (observed data is far
> below them). A material listed more than once in *L* is applied at its last position
> only; with last-wins layering the effective values are identical. More than 64 distinct
> materials fail the resolution. At most 1,024 material entries without effect are kept
> for the inspector; further ones are only counted.
> Adopted: these are hostile-input limits with no counterpart in the original; observed data stays far below them.

> **Decision (adopted 2026-10-07; was OPEN):** Texture level selection in `MaterialLibrary` (`texture-list.md`
> §5.2): the configured largest texture side acts as a resolution cap on top of the
> caller's (step 5), so a smaller stored level is used instead of failing; it is an error
> only when even the last level is larger. A level's size is its header size, or for a
> JPEG level (header 0×0) its stream's frame size; a level of zero width or height, or a
> JPEG level whose frame header cannot be read, does not resolve (step 3).
> Adopted as Tessera's level-selection policy (an engineering choice; `texture-list.md` §5.2 stays normative for the levels themselves).

Added by the clean-room implementation of the 2026-10-05 revisions (`crates/material`,
2026-10-06). Each is an engineering choice where the revised text is silent:

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** 4002 DepthWrite (§8.5) is applied as "0 turns depth write off": the
> effective depth write is the render-state byte (unlit) or on (lit base passes) AND
> 4002 ≠ 0. Whether an effective 4002 = 1 also turns depth write **on** for a record whose
> chosen render-state byte 4 is 0 is not stated; the implementation leaves the byte's choice.
> 4001 DepthTest is exposed as `ResolvedMaterial::depth_test` (on when undeclared).
> Answer: an effective 4002 sets depth write in the permutations its bindings name **both ways**: 1 turns it on even where the block's byte 4 is 0 (§7.3.1). Likewise 4001 sets the depth comparison to 7 (on) or 8 (off) in its permutations. Revise the implementation accordingly.

> **Answered 2026-10-07 (BIN-S4, MA46; was OPEN):** §20.1 describes the `ColorTexture` record (0x2B0000B7) as outputting "twice the
> product of the two layers' alphas and the 2000 alpha", but the Rule in the same row lists
> only the slot-3000 alpha. The implementation multiplies the 2000 alpha, the slot-3000
> alpha and the second layer's alpha (3001, when its effective texture is non-zero) and
> does **not** apply the factor 2. Is the factor 2 part of the rule, and is the second layer
> read whatever its role?
> Answer: the factor 2 is part of the rule (both the Direct3D 10/11 and the OpenGL program compute 2 × alpha(3000) × alpha(2000) × alpha(3001); the output is clamped by the 8-bit target). The second layer is the 3001 texture (sampler `ColorTexture2`, read with texture matrix 1) whatever its role; only its alpha is used, the RGB is 3000 × 2000. §20.1's Rule is corrected accordingly.

> **Answered 2026-10-07 (BIN-S4, MA46; was OPEN):** Lit blend classification (§11.3) treats any §11.2 opacity source as
> "the base role carries opacity", including 3020 `OpacityAlpha` and 3018
> `EmissiveAndOpacity`, and also when that slot's effective texture is 0 (§18, opacity
> source without a texture). Such a surface goes to the transparent pass. "Opaque by
> construction" is: no §11.2 source and effective 2000 alpha ≥ 1 (a non-finite alpha counts
> as below 1). Undeclared 2000 counts as alpha 1.
>
> **Answered 2026-10-07 (BIN-S4, MA46; was OPEN):** does the 3020 `OpacityAlpha` or 3018
> `EmissiveAndOpacity` alpha reach the lit base pass's output alpha (`lighting.md` §5.12)
> when the 3000 role carries no opacity? §11.3's literal text puts a lit surface with an
> opaque 3000 role (`Color`, `ColorAndGloss`, `ColorAndDye`) and 2000 alpha 1 in the opaque
> pass, the cut-out coming from the alpha test alone; the Decision above sends it to the
> sorted transparent pass. A reviewer census (not re-checked by the implementer) counts
> 134 such surface records: 130 with 3020 and base `ColorAndDye` (0x2B00072C 60,
> 0x2B000754 24, 0x2B000A1F 19, 0x2B000A17 13, 0x2B00076E 9, 0x2B00080E 5), all
> alpha-tested, and 4 with 3018 and base `ColorAndGloss` (0x2B000772), 2 of them without
> alpha test. The code keeps the Decision until §20.1 answers.
> Answer (both items): yes, and the Decision matches the programs. In all lit base permutations (pass codes 0x04, 0x05, 0x3E, 0x3F, 0x40) of 0x2B00072C, 0x2B000754, 0x2B000A1F, 0x2B000A17, 0x2B00076E and 0x2B00080E the output alpha is the `OpacityAlpha` (3020) alpha × the material alpha, and in 0x2B000772 the `EmissiveAndOpacity` (3018) alpha × the material alpha; the 3000 alpha (dye mask or gloss) does not reach it. These surfaces blend by that alpha and belong to the transparent pass, as the Decision says.

> **Decision (adopted 2026-10-07; was OPEN):** Two colour layers (§11.2): the second opacity slot is 3001 when the
> opacity (or the blend's source alpha) comes from slot 3000, the 3000 role contains
> `Opacity` and the 3001 role contains `Opacity`, and 3001's effective texture is non-zero
> (as for §11.5 `second_layer`). Exposed as `ResolvedMaterial::second_opacity_slot`.
> Adopted: consistent with §11.2 (opacity is the product of both colour-layer alphas, BIN).

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** Wrap flags (§8.5): a texture slot's address mode U (V) is set from
> the effective 4005 (4006) when any of the slot's texture bindings names the same
> (permutation, sampler index) as a binding of the 4005 (4006) declaration, whatever that
> binding's `semantic`; 1 → wrap (code 1), 0 → clamp (code 3), including over a stored
> border (4). Slots whose samplers no flag binding names keep their stored bytes.
> Answer: this is the client's rule: semantics 11 and 12 set the address mode of the sampler their binding names, 1 → wrap (1), 0 → clamp (3), over whatever the stored byte holds, border included (§7.3.1); samplers no flag binding names keep their stored bytes. Matching by (permutation, sampler index) whatever the semantic is equivalent on this install, where every 4005/4006 binding has semantic 11/12.

> **Answered in part 2026-10-07 (BIN-R (l), (n); was OPEN):** Float curves (§5.3.1). Static consumers (the 1043 threshold, the
> 1000 power and every `ParameterSet::float`) use the value at *t* = 0 with a fresh state
> (for mode 10, the value at its start, *p* = 0.1); animated use evaluates over time with
> per-curve state. Mode 3 uses a xorshift32 generator. A mode-10 position that is NaN (a
> zero or non-finite `period`) is treated like *p* ≤ 0 (*p* = 0.1). The |*t*| > 3,600
> reduction of step 1 is applied to mode 10 as well, as the section's structure says
> ("for the other modes").
> Answer: the original evaluates the alpha reference and the specular power at the clock at every draw (§8.3, §11.2), so the t = 0 value for static consumers is an approximation (Decision). The modulo-3,600 reduction applies to mode 10 in the client too (§5.3.1, MA49). The xorshift generator for mode 3 and the NaN-position rule stay Decisions.

Added by the clean-room implementation of M4.5 (`crates/material` modules `transform`,
`inputs`, `water`, 2026-10-06). Each is an engineering choice where the text is silent:

> **Answered 2026-10-07 (BIN-R (j), BIN-S4; was OPEN):** The texture-matrix index of §7.3 (the binding `slot`) is per permutation, but
> §8.4 and `lighting.md` §5.2.1 describe one matrix layout per record. On the reference
> install four records (0x2B000765, 0x2B000772, 0x2B0009CE, 0x2B0009CF) have permutations
> (one without programs whose only sampler is a glow texture, and others) that bind the
> second set's parameters (1003, 1004, 1019, 1020, 1034, 1072, 1073) to their matrix 0,
> while their colour passes bind the first set there: 35 (matrix, value) pairs have two
> candidates. Permutations whose own table differs from the record-wide one: 0x2B000765 7
> (1, 19, 39, 85, 131, 149, 169), 0x2B000772 20 (pass codes 0x08 glow, 0x09 depth, 0x0A
> shadow depth, 0x42 view normal; in 0x42 set 0 is also bound to matrix 1), 0x2B0009CE 6,
> 0x2B0009CF 6. The implementation (revised after review, 2026-10-06) keeps one table per
> permutation (`TextureMatrixLayout::for_permutation` / `evaluate_for`, and
> `MaterialLibrary::permutation_texture_matrices`), so a pass can use the transform its own
> permutation binds. It also keeps a record-wide table for callers that do not track
> permutations: per (matrix, value), the parameter named by the most bindings (ties to the
> one met first in stored order), which gives the first set on matrix 0 in all four (the
> colour passes). Open: whether the per-permutation reading is the client's (it follows
> §7.3, where `slot` belongs to one permutation), and which matrix each non-colour sampler
> uses **in the colour passes**: §8.4 states only base colour → matrix 0 and second colour
> layer → matrix 1; 3013 `Glow` (`lighting.md` §5.9) and 3018 `Emissive` /
> `EmissiveAndOpacity` are not covered. In 9 of the 10 surface records that use these four
> shader records, set 0 and set 1 differ (for example 0x3100422A: set 0 identity, set 1
> scale 0.5 with sine-animated 1019/1020), so the answer changes what is drawn. 407
> matrices carry semantics 38–44 (as §7.3 says), 393 of them in permutations with
> programs.
> Answer: the per-permutation reading is the client's: `slot` names an item of that permutation's extra state list and the item's `stage` is the texture matrix (§7.3.1, §7.7). Which matrix each non-colour sampler reads in the colour passes is listed in §8.4; for 0x3100422A (shader 0x2B000772) the `EmissiveAndOpacity` texture reads matrix 1 (set 1) in the colour and light passes.

> **Decision (adopted 2026-10-07; was OPEN):** Values handed to a renderer are finite. A non-finite clock leaves a
> mode-10 curve's state untouched (a running curve returns its last value, a fresh one the
> value at its start), so one bad tick neither restarts nor freezes it and a start time
> cannot become NaN; the other modes are evaluated at 0 (revised after review,
> 2026-10-06: reading the clock as 0 made a running loop-0 curve look like a clock going
> backwards and froze it at its start value). An animated 1043 whose value is non-finite
> falls back like a non-finite constant one (§18, alpha-test threshold), not to 0. A NaN
> curve result becomes 0
> and an infinite one the largest finite f32 of its sign; colours are treated the same
> way. §5.3.1 lets mode 8 overflow on finite data: with `w[9]` ≠ 0, *G* grows with time
> and the weights 2^(−*H*·*i*) reach infinity (an amplitude of 0 then gives NaN). What the
> original does then is not stated; on the reference install no effective curve of a
> surface record is non-finite at 0, 1.5, 37.25 or 3,700 s.
> Adopted: the original hands non-finite values to the device unchanged; Tessera keeps them finite as described (a renderer safety rule, not the original's behaviour).

> **Answered 2026-10-07 (BIN-R (j); was OPEN):** Specular colour when a shader record declares 1000 but not 2001
> (never observed, §8.3 says they are always declared together): white × the per-object
> tint, then black below power 0.5 as usual. The program time of `lighting.md` §4.2 E20
> is 0 for a negative or non-finite clock.
> Answer: the client uses the block's `specular_color` (bytes 19–22, §7.6; white in every block of this install) × the per-object tint, then black below power 0.5 (§8.3). The implementation's white × tint equals it on this install; reading the block value is exact.

<a id="s19"></a>
## 19. Open items raised by the explorer mesh viewer (clean room, 2026-10-04)

The explorer's 3D view (`apps/explorer`, `scene` module) maps resolved materials onto the
`tessera-render` mesh pipelines. A real-data run (`apps/explorer/tests/mesh_real_data.rs`,
`TESSERA_EXPLORER_FULL=1`) built every one of the 47,563 mesh records into a drawable scene:
72,839 level-0 draws, every surface record resolved, 0 failures. Choices the spec leaves
open:

> **Answered 2026-10-05 (§20.1; was OPEN):** Specular power 0. Several common shader records declare 1000 (so §11.5 sets
> `specular`) with default 0, and materials that want a highlight override it (25, 30).
> In a sample of 764 level-0 draws (every 97th mesh record), 737 materials are specular
> and 138 of them have effective power 0. With §11.5's `(N·H)^1000` that is a constant,
> view-independent highlight over the whole surface. The viewer treats power ≤ 0 as "no
> highlight" (and loads no gloss mask then). Whether 0 means "off" in the original is
> unknown.

> **Answered 2026-10-05 (§20.1; was OPEN):** Diffuse colour 2000 (§8.3, Inferred). Census of the install's 24,094
> materials: 1,055 set 2000; 340 are white, 8 are black (RGB 0; alpha 0, 0.75–0.86 or 1),
> and 707 are other tints; alpha is 1 in 791 and spread over 0–0.98 in the rest. A black
> 2000 multiplied into the base colour draws the surface solid black (for example the
> one-triangle part of setup 0x04000001, whose material sets (0, 0, 0, 0)), so "tint" may
> be wrong or need a condition (alpha 0 = unset?). The viewer therefore applies 2000 only
> on request ("Diffuse colour", `--mesh-diffuse-tint`; read as sRGB-encoded like the
> textures) and notes non-white values in the material inspector.

> **Answered 2026-10-05 (§20.1; was OPEN):** Vertex colours (`mesh.md` §5.4). The viewer passes them only on request
> ("Vertex colours", `--mesh-vertex-colors`), RGB only, read as sRGB-encoded bytes, with
> alpha always 1. Reason (explorer review, every 7th mesh record, 6,795 meshes): 117
> alpha-tested and 20 blended draws have most vertices with alpha below the 0.8 threshold,
> and 180 opaque draws have mostly dark RGB; multiplied in, they delete foliage cards and
> tent canopies and blacken snow (meshes 0x06007d43, 0x060070ad, 0x06002990), which look
> right without them. Whether the original multiplies them into the diffuse, uses them for
> something else (wind weights, blend masks, baked occlusion), or ignores them per shader
> is unknown.

> **Answered 2026-10-05 (§20.1; was OPEN, Decision):** Specular colour 2001 is read as sRGB-encoded and converted to
> linear like 2000 and the dye colour, so the three material colours share one convention
> in the viewer (the renderer's specular colour is linear). With the stored greys (§8.3)
> this gives a highlight about 3–4 times weaker than passing the stored value as linear.
> Which space the original uses is unknown.

> **Decision (adopted 2026-10-07; was OPEN):** Opacity for the renderer. The viewer composes one base-colour
> texture whose alpha is the §11.2 opacity source (slot 3000's own alpha, or the alpha of
> 3020/3018 resampled nearest to 3000's size), or opaque when there is none, because the
> renderer takes coverage from the base texture's alpha. Dye masks (§12.2) are moved into
> R as `255 − alpha` (the renderer's mask convention, `render.md` §2.3) and gloss masks
> (§10.2) replicated into RGB; the dye colour is converted from sRGB to linear before the
> renderer's linear multiply, which equals §12.3's multiply of stored values for a pure
> power curve.
> Adopted for the viewer: an engineering mapping onto the renderer's texture convention; it reproduces §11.2 and §12.3 on stored values except for the sRGB note of §12.3.

> **Decision (viewer); the original's rule answered 2026-10-05 (§20.1, last row):** Two dye channels. The renderer takes one dye per draw, so the viewer
> draws one channel at a time: the first, unless only the second has a colour other than
> white, or the second while a "dye 2" colour is previewed. A "dye 2" preview leaves
> materials without a second channel on their own first dye: §12.3 applies 2013 only
> through the second dye-mask source. Drawing both channels needs a second dye slot in
> the renderer.

> **Answered 2026-10-05 (`appearance.md` §4.11; was OPEN):** Material-only appearance entries (`setup.md` §3.2: entries with dye blocks and
> no parts, for example the 7-block coat dyes of a mount) name no geometry. The viewer
> lists their blocks' 2003/2013 colours and lets the user preview one on any dyeable
> material, but cannot tell which geometry a block layers onto (the same item's worn
> entry? the wearer's body?). The rule that pairs them is needed for a faithful dye
> preview of worn items.


---

<a id="s20"></a>
## 20. Answers to §18–§19 items, and change log

<a id="s20-1"></a>
### 20.1 Answers (2026-10-05, from BIN-S, BIN-C and EM)

| Item | Answer |
|---|---|
| §18 alpha test on blended records | The programs discard by the alpha test and still output their blend alpha: test and blend are independent, as implemented. |
| §18 unlit blended records without an opacity source (Decision `BlendState::opacity_slot`) | **Revise** (census and programs re-checked 2026-10-05; this row replaces an earlier count of 530 that used a different selection). The set is the 522 surface records that declare slot 3000, are unlit, choose a blend triple with a SRCALPHA or INVSRCALPHA factor and have no §11.2 source (463 `Color`, 43 `DiffuseTexture`, 12 `LiquidDiffuseTexture`, 4 `ColorTexture`), plus 15 that declare no slot 3000 (they output the 2000 alpha, times the vertex alpha in one record). What the kind-4 colour programs of their records output as alpha (BIN-S): **`Color` (463), the slot-3000 alpha never reaches the output.** 239 surface records (9 records, the largest 0x2B00078B with 158) output the 2000 alpha; 131 (4 records, the largest the additive 0x2B0007F2 with 122) output vertex-colour alpha × 2000 alpha, times a soft-particle depth fade in permutations that have one (parameter 1174); 93 (6 water records: 0x2B0000A8, 0x2B0000CE, 0x2B0005EC, 0x2B0007FD, 0x2B000559, 0x2B000A8D) output an alpha computed by the water programs (`water.md` §8), except in their simple `Color`/`Color2` permutations, which output the 2000 alpha. **`DiffuseTexture` (43): the slot-3000 alpha reaches the output**, multiplied by effect factors: vertex alpha (heat distortion, 0x2B000084), 2000 alpha and a Fresnel term (mirror 0x2B0000AA, window 0x2B000077 and the window family 0x2B0009E1–0x2B0009E8, §8.7). **`LiquidDiffuseTexture` (12):** slot-3000 alpha × 2000 alpha. **`ColorTexture` (4, 0x2B0000B7):** twice the product of the two layers' alphas and the 2000 alpha. Rule: source alpha = 2000 alpha × (vertex alpha where the record reads vertex colours, `lighting.md` §5.13) × (slot-3000 alpha only for roles `DiffuseTexture`, `LiquidDiffuseTexture`, `ColorTexture`, `ColorTexture2`) × (for `ColorTexture` also 2 × the slot-3001 alpha, corrected 2026-10-07, BIN-S4); the water, mirror, window and heat-distortion factors stay OPEN (§8.7). |
| §11.3 "unlit" | "Unlit" in §11.3 means "no additive light passes", not "no lighting". **Revised 2026-10-06** (census of every permutation of every back end; the former row counted only Direct3D 10/11 programs and was wrong). Of the 105 unlit records used by surface records (2,142 surface records), 11 records (99 surface records) read light inputs (`lighting.md` §4.2) in their single pass and 94 records (2,043 surface records) read none. The 11: **(a) ten water-family records** — the six water records above (93 surface records) and 0x2B0000A9 (2), 0x2B0000DC, 0x2B00084B, 0x2B00089A (1 each). In their water permutations they read the fake directional light (E18; 0x2B0007FD also the sun-shadow inputs E13 in its Direct3D 10/11 water permutations). In their simple `Color`/`Color2` permutation they read ambient × material (E1), hemispheric × material (E2) and the per-vertex point and directional lights with their counts (E9–E11). That permutation exists for both Direct3D 10/11 and OpenGL in 0x2B0000A8, 0x2B0000CE, 0x2B000559, 0x2B0005EC and 0x2B0007FD, and for OpenGL only in 0x2B0000A9, 0x2B0000DC, 0x2B00084B, 0x2B00089A and 0x2B000A8D. **(b) 0x2B000A8C** (1 surface record; samplers `ColorAndOpacity`/`ColorAndOpacity2`, SRCALPHA / ONE with depth write): every one of its four program permutations (Direct3D 9 shader model 2.0, two Direct3D 10/11, OpenGL) reads ambient and hemispheric without the material colour (E3) and the light-0 colour and range (E5). Of the 94 records that read no light input, 10 (23 surface records: 0x2B000442, 0x2B0004B5, 0x2B00063E, 0x2B000810 and the window family 0x2B0009E1–0x2B0009E5, 0x2B0009E8) read water parameters and 84 (2,020 surface records) read neither. The 192 terrain layer records are also "unlit" in this sense and are lit in their own passes (`terrain-texturing.md` §9). |
| §19 specular power 0 | **Superseded by the "second pass" row below.** First-pass answer: the programs compute x⁰ = 1 (for x > 0), a view-independent highlight; whether the engine avoids it was then open. |
| §19 diffuse colour 2000 | Confirmed as a tint of the lit result (it multiplies every light and ambient colour) and as the output alpha factor (`lighting.md` §5.12). A (0, 0, 0, 0) material is black and fully transparent, i.e. invisible: the one-triangle part of setup 0x04000001 is not visible in the original. |
| §19 vertex colours | Only the 116 shader records whose programs take a colour input read them (2,624 surface records; `lighting.md` §5.13). There they multiply 2000: RGB scales the lit colour, alpha scales the output alpha, **after** the alpha test. All other records ignore vertex colours, which is why multiplying them in everywhere deleted foliage and darkened snow. |
| §19 specular colour space, dye colour space | **Superseded by the "second pass" row below.** First-pass answer: the programs use sampled and constant values as given; the texture formats were then open. |
| §19 specular power 0 (second pass, BIN-T (f)) | Answered: the client uploads a black specular colour whenever the effective power is below 0.5 (§8.3), so there is no highlight. The viewer's "power ≤ 0 = no highlight" matches the original for 0; use the 0.5 threshold to match it exactly. |
| §19 specular and dye colour space (second pass, BIN-T (g)) | Answered: no back end uses sRGB textures or frame buffers; every colour and texel is used as stored (§12.3, `lighting.md` §15). The viewer's conversion of 2000, 2001 and 2003 from sRGB to linear does not match the original's arithmetic; matching it means doing the arithmetic on stored values. |
| §19 two dye channels, §18 second dye mask order | The second dye colour (2013) is applied to the undyed base colour where its own mask is low, after the first dye: result = *a2* · (first-dyed colour) + (1 − *a2*) · (base × 2013) (`lighting.md` §5.2). Its mask is the `DyeAlpha` texture (slot 3024) in records that also have a `ColorAndDye` diffuse (for example 0x2B00072C), and the second colour layer's alpha in records with `ColorAndDye2` (0x2B0006E4). The implementation's order (3000, 3011, 3024, 3001 for the first source; the next one for the second) matches both observed forms. |

Answers of 2026-10-07 (BIN-R, BIN-S4):

| Item | Answer |
|---|---|
| §18 `ColorTexture` factor 2 | Part of the rule: 2 × alpha(3000) × alpha(2000) × alpha(3001) in both back ends; colour 3000 × 2000 (MA46). |
| §18 3020 / 3018 in the lit base output | Yes, × the material alpha, in every lit base permutation of the 7 records concerned (MA46). |
| §18 texture-matrix index per permutation | The client's reading; `slot` → extra state item → `stage` = matrix (§7.3.1, §7.7, MA42); emissive and glow matrices §8.4 (MA47). |
| §18 4002 and 4001 | Both ways, per bound permutation (§7.3.1). |
| §18 4004 without 1043 | The block's `alpha_ref` (§7.6). |
| §18 wrap flags | The implementation's rule is the client's (§7.3.1). |
| §18 blend codes, duplicate ids, broken references | §7.6 (Direct3D 9 codes), §9.2. |
| §18 alternative materials (§9.2) | Last wins; no state selection. |
| §8.5 4007 | A second, edge-softening draw (§8.5); the enabling setting is OPEN. |
| §11.2 edge softening | 4007 and the alpha-to-coverage option (§11.2). |

<a id="s20-2"></a>
### 20.2 Change log

| Date | Sections | Change |
|---|---|---|
| 2026-10-04 | all | First complete version (Review). |
| 2026-10-05 | header, §1 | Revised by the spec writer with M4 under decision 0011 (BIN-S, BIN-C) and the parameter-label enum mapper (EM); provenance rows MA18–MA26. |
| 2026-10-05 | §5.3 | Float-curve evaluation for every mode; mode 10 tail is (period, loop flag, pairs of value and position %); Decision revised. |
| 2026-10-05 | §7.3, §7.4, §7.6 | Binding semantics and `slot` meanings; program kinds named; byte 4 confirmed as depth write per pass code. |
| 2026-10-05 | §8.0 (new), §8.3–§8.6 | Parameter labels; flags 4001–4007 named (4003 DoubleSided, 4007 AlphaBlendPass); U/V of the transform pairs; `flag` byte = label starts with `c_`. |
| 2026-10-05 | §10.2, §11.2–§11.5, §12.3 | Normal-map axes; alpha test keeps opacity > threshold; lit base passes blend; two-sided = 4003 (the M2 decision on 4007 is withdrawn); specular and environment map; dye blend confirmed. |
| 2026-10-05 | §17, §20 | Status of the former frame-capture questions; answers to §18–§19. |
| 2026-10-05 | §1, §5.3.1, §7.3, §8.3, §8.4, §8.7, §11.5, §12.3, §17, §20.1 | Second M4 pass (BIN-T), provenance rows MA27–MA33. §5.3.1: the clock (global, seconds since the client's timer started); mode 3 = global random generator; modes 7 and 8 specified (1-D gradient noise *N₁* from `srand(0)`/`rand`, 6-octave sum for mode 8); mode-10 search confirmed strict at exact key positions, with an example. §7.3: semantic → transform value table, including the older codes 33–37 (Tessera ignores them). §8.3: what the client does before upload (per-object tint, specular power scale, black specular below power 0.5); colour storage note. §8.4: composition rule (early translation, scale, counter-clockwise rotation, translation), matrix layout, reset value, rotation about the centre; OPEN closed. §8.7: water pointer to `water.md`. §11.5: specular below 0.5. §12.3: colour space answered (no sRGB). §17, §20.1 updated. |
| 2026-10-05 | header, §1, §3.1, §5.3.1, §8.0, §8.6, §8.7, §9.3, §11.3, §12.4, §17, §18, §19, §20.1 | Third revision after review (spec writer; M1 and BIN-S third pass). §1: legend defines the marks BIN and Decision and the dates; BIN-S logs the third pass; rows MA34–MA35; MA31 and §5.3.1 name the programs' time input without a program-constant label. §5.3.1: noise *N₁* restated as a definition of the two tables in terms of the generator's output sequence (constants kept, decision 0012). §8.0: 197 labelled ids, 148 used (was "148 labelled"). §8.6: flag-1 water ids are 1044–1069, 1170, 1171, 1173, 1182–1190, 2004–2006, 2009–2011 (the former "1170–1190" overlapped depth fade, tie-dye and wind; distortion is 1140–1149 plus heat 1160). §8.7: the exclusion is 1070–1085, as in §8.0. §11.3: the lit table row now states the revised Decision (source-alpha blend, transparent pass unless the output alpha is 1 by construction). §3.1, §9.3, §12.4, §17, header: the stale "future `appearance.md`" replaced by `setup.md` §8 and `appearance.md` §4.9–§4.11; fields `a` (slot) and `g` (next group's priority) answered from `setup.md` §8.3. §18–§19: callouts answered by §20.1 marked as answered or revised. §20.1: the unlit-blended row recounted (522 + 15, not 530) with the output alpha per record family; 0x2B0000A8 identified as the water shader (its texture-free alpha holds only for its `Color`/`Color2` permutation); the "unlit" row corrected (only the six water records apply lights in their single pass); first-pass rows on specular power 0 and colour space marked superseded. |
| 2026-10-06 | §1, §7.3, §8.4, §8.6, §20.1 | Fourth revision after the re-review of 2026-10-05 (spec writer; M1 only). §7.3, §8.6, MA19: 2013 is the one flag-1 parameter with binding entries (one semantic-4 binding in 7 of its 11 records, all naming a permutation without programs); the statements that no flag-1 parameter has bindings are corrected, and flag-1 parameters still reach every program by name. §8.4: the centre-rotation census is 264 early-translation/translation pairs in 136 materials (205 on U, 59 on V); "205 material entries" counted U only. §20.1 "unlit" row rewritten from a census of all back ends: 11 records (99 surface records) read light inputs (ten water-family records, E18 in water permutations and E1, E2, E9–E11 in their `Color`/`Color2` permutation, some OpenGL only; 0x2B000A8C reads E3 and E5 in all four permutations), 94 (2,043) read none, 84 of them (2,020) no water parameter either. New row MA36. |
| 2026-10-07 | header, §1, §4.1, §4.2, §5.3.1, §7.3, §7.3.1 (new), §7.4, §7.5, §7.6, §7.7, §8.3, §8.4, §8.5, §8.6, §8.7, §9.2, §9.3, §11.2, §11.4, §12.4, §12.5, §14, §17, §18, §19, §20.1 | Fifth revision (spec writer; M4 under decision 0011: BIN-R client-executable pass and BIN-S4 shader pass; M1 censuses). Status Revised. New provenance rows MA37–MA49; MA28 corrected (semantics 33–37 set the curves of type-1, -2 and -3 items, not early translations). §4.1: the u16 `tail` is two flag bytes, `shadow_receiver` and `not_pickable`; §4.2: `binding_key` has no special surface value, 2 is the group-side wildcard. §7.3.1: how each binding sets its own permutation's state, sampler or item. §7.4: `has_programs` is a count, third blob with key bit 10. §7.5: full sampler state; bytes 10 and 11 are minification then magnification (swapped). §7.6: full block; byte 3 is the depth comparison for a reversed depth range, byte 6 the alpha-test switch, bytes 7–38 default reference, colours and specular power as curves and packed colours. §7.7: the extra state list is the texture-transform list; `stage` = texture matrix; composition per permutation. §8.4: matrix of emissive and glow samplers per record. §8.5: 4001–4006 rules per bound permutation; 4007 = second edge-softening draw. §8.6: by-name passing for groups 1000/2000 only. §9.2: client layering, last wins, broken references. §9.3: group matching (`a`, `b`, 2 = all, `a` 0 = none) over all parts of the entity; census. §11.2: per-draw alpha-test handling (quantised reference, material-alpha scaling, reference 0 in SRCALPHA passes, alpha-to-coverage option). §11.4: per-permutation cull. §12.5: tie-dye ramp. §14: MT10–MT13. §18–§19: callouts answered, adopted as Decisions or narrowed. Of the 34 earlier OPEN callouts 30 are closed (25 answered from the client and the data, 5 implementation choices adopted as Decisions) and 4 remain; 2 new, narrower ones were added. Remaining: the setting that enables the 4007 draw (§8.5), effect families (§8.7), two unidentified per-draw alpha-test conditions (§11.2), the dye-block fallback (§12.4), a sampler without a texture (§18), cube-map face order (`texture-list.md`). |
| 2026-10-08 | header | Fifth revision reviewed (separate review session): passed, status Accepted. No content change. |
