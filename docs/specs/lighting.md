<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Lighting: passes, light sources, the lighting equation, shadows and fog

| | |
|---|---|
| **Status** | Accepted (2026-10-08, seventh revision, see §16) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Seventh revision: separate Claude review session (spec-reviewer role), 2026-10-08, **passed** (Accepted). Sixth revision: Claude review session (spec-writer agent), 2026-10-07, **passed** (Accepted). Earlier rounds and detailed findings: dirty-room review log (incident 0018). |
| **Implements into** | `crates/render` (lit and unlit pipelines, light uniforms, shadow pass, fog), `crates/material` (per-surface lighting inputs derived from parameters), `crates/world` (light selection per object, sun and ambient from the environment, light records from Light entities) |
| **Companion specs** | [`material.md`](material.md) (parameters, texture roles, feature flags, permutation records, texture transforms §8.4, float curves §5.3). [`environment.md`](environment.md) (sky description, day description keyframes, game clock: the source of sun, ambient and fog values). [`entity-descriptor.md`](entity-descriptor.md) §4.5 (light properties on placed entities). [`mesh.md`](mesh.md) §5 (normals, tangent frame, vertex colours). [`terrain-texturing.md`](terrain-texturing.md) §9 (terrain layers' use of these inputs). [`water.md`](water.md) §8 (water shading; uses the fake-light inputs of §4.2). [`property-system.md`](property-system.md) §4.7 (property colour values). |

---

<a id="s0"></a>
## 0. Purpose

This spec lets an implementer reproduce how the original client lights a surface:

- which **passes** a lit surface is drawn in and which inputs each pass reads (§2, §3, §12);
- the **lighting equation** in words: ambient with a hemispheric term, one main directional
  light (the sun or moon) with shadows, nearby point lights, specular highlights, emissive and
  glow terms, environment reflection, dye, alpha and distance fog (§5, §6);
- where the **light values** come from: the time-of-day keyframes of `environment.md` for the
  sun, ambient and fog; placed light entities for point lights; which lights an object
  receives (§8);
- the **sun shadow** model in enough detail for a simple implementation (§9);
- the colour encodings used by records and properties (§4.4).

Tessera writes its own shaders. Nothing here asks an implementer to run, translate or read
the compiled programs stored in shader records (`material.md` §7). The equations below are
functional descriptions of what the original computes, so that Tessera's own shaders can
match its look. Rules marked **Tessera Decision** are this project's choices where the
original's behaviour is not known; tests must not treat them as the original's behaviour.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-05 by throwaway probes in the dirty-room scratch area: all 510 shader records (0x2B) and their 31,519 permutations parsed with `material.md` §7; the reflection tables of every compiled program read (constant names, sizes and offsets; sampler names; vertex input semantics); all 0x30 materials and 0x31 surface records resolved with `material.md` §9. |
| BIN-S | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`lotroclient64.exe`, `lotro_live`). The compiled programs inside the shader records of that build were disassembled with a public shader disassembler (Direct3D 9 shader model 2/3 and Direct3D 10/11 shader model 4 bytecode; the OpenGL back end stores GLSL text) and read for representative permutations of every pass code (§3.2). **Question:** what each pass computes from its inputs (the lighting equation, attenuation, shadow filtering, fog, alpha test, normal-map convention). **Why M1–M3 were not enough:** the data stores only parameter defaults and program bytecode; the equations exist only as program code; no frame capture of the original is available to this session (M3) and no published source describes them (M2). Third pass 2026-10-05: every Direct3D 10/11 permutation, following each sampler's texture coordinate back to the vertex inputs (§4.3), and the colour programs of the unlit blended records of `material.md` §20.1. **Question:** which UV set feeds which texture; what reaches the output alpha. Same reasons. |
| BIN-C | **M4 (decision 0011).** Same client build, client executable disassembled around the day-description keyframe blending and the shader-constant registration table. **Question:** how the sun direction, ambient and fog values are produced from data, and which engine constants exist. **Why M1–M3 were not enough:** the data gives keyframe values but not how they are combined or converted into light parameters. |
| BIN-L | **M4 (decision 0011), second pass 2026-10-05.** Same build (`x64/lotroclient64.exe`, SHA-256 prefix d22363eb9aff0fb5), plus the graphics back-end libraries shipped beside it (`x64/D3D11GraphicsCore.dll`, `x64/OpenGLGraphicsCore.dll`), disassembled with objdump; constants read from the files. **Questions:** (a) how the frame's ambient and hemispheric inputs are formed from the environment; (b) how the environment's light values are carried and cross-faded; (c) what the per-material setup uploads (material times ambient, per-object terms, specular rule); (d) which lights an object receives and in which slot; (e) how a light's record (colour, specular, range, attenuation factor, type) is built from its fields; (f) the field labels of the light class; (g) the "fake" light inputs; (h) the texture and frame-buffer formats of the back ends (sRGB); (i) the clock behind the programs' time input; third pass 2026-10-05: (j) the depth comparison of the passes (the back ends' depth-state translation, `D3D11GraphicsCore.dll` and `OpenGLGraphicsCore.dll`); (k) the engine values of the sun-shadow inputs (the client's registration of those inputs); fourth pass 2026-10-06 (same build, read with the Ghidra 12.1.3 decompiler in the dirty room): (l) which objects a placed light reaches: the selection radius, the distance test, the order of an object's light list, the treatment of landscape chunks and the parent flags; (m) the light class's field registration (each field's offset, label and property id); sixth pass 2026-10-07 (same build, own copy of the Ghidra project, decompiler and listings): (n) which objects are candidates of the light-list walk and how the parent switches behave for a light without a parent; (o) the client's second path for `Light_BakedLandscapeLight` lights (drawing them into the baked-lighting texture of terrain and ceiling chunks): when it runs, its inputs, how it composites; (p) the graphics options and device conditions that gate that path; seventh pass 2026-10-08 (same build, own copy of the Ghidra project, decompiler and listings in the dirty scratch area): (q) how the world box that the light-list walk tests (§8.5 step 2) is formed; (r) whether the walk treats directional lights differently from point lights. No network, login, launcher or anti-tamper code was examined. **Why M1–M3 were not enough:** these are code paths; the data stores the inputs only; no frame capture is available (M3); nothing is published (M2). |
| ENV | [`environment.md`](environment.md) (this session): day-description layout and keyframe blending. |
| MAT | [`material.md`](material.md): parameter ids, texture roles, permutation layout, texture transforms, float curves. |
| TT | [`terrain-texturing.md`](terrain-texturing.md) §3.3, §9 (record colours, terrain lighting). |

Marks: **Confirmed** = checked against all records of the kind in the install (layouts,
censuses) or, for BIN-S facts, checked in every permutation of the pass code named; **BIN** =
rests on disassembly only (decision 0011 requires saying so); **Inferred** = a reading that
fits the evidence but was not proven; **Reported** = third-party only (none here); **Tessera
Decision** = this project's rule where the original is unknown.

| # | Fact | Sources | Mark |
|---|---|---|---|
| L1 | Program blob kinds: 0 = Direct3D 9 shader model 1.1, 1 = Direct3D 9 shader model 2.0, 2 = shader model 3.0, 3 = OpenGL (GLSL text with a constant table), 4 = Direct3D 10/11 shader model 4.0 (§2.2) | OBS (magic of every blob: 27 + 6,918 + 7 + 7,041 + 16,985) | Confirmed |
| L2 | Permutation key word 3 is the **pass code**; table §3.2 | OBS (census of constants, samplers and render state per pass code over all permutations); BIN-S | Confirmed census; meanings Inferred from the inputs each pass reads |
| L3 | Key words 1 and 2 are "required" and "excluded" feature masks; bit meanings §3.3 | OBS (correlation of each bit with program inputs over 16,985 + 6,918 permutations) | Inferred |
| L4 | Engine-supplied inputs and their meaning (§4.2, E1–E21) | OBS (the programs' reflection data, used in the dirty room to tell the inputs apart; 160 registered inputs, BIN-C); BIN-S (how each is used); BIN-L (a), (c), (g), (i) (how several are filled) | Set of inputs Confirmed; meanings BIN. The spec describes the inputs without the programs' names (revised 2026-10-05). |
| L5 | Lit base-pass equation (§5) | BIN-S (passes 0x04, 0x05, 0x3E, 0x3F, 0x40 of the most used lit shader records, all back ends) | BIN |
| L6 | Normal maps: tangent-space vector = 2 × texel − 1, axes = stored tangent, **negated** stored binormal, normal (§5.2.4) | BIN-S (every vertex program that passes a tangent frame) | BIN |
| L7 | Lit output = 2 × albedo × (sum of light terms) before specular, emissive and fog (§5.7) | BIN-S | BIN |
| L8 | Alpha test discards when opacity ≤ threshold (§5.12) | BIN-S | BIN |
| L9 | Point-light attenuation 1 / (1 + k·d²) inside the squared range, with a smooth fade over the last quarter of the squared range in the newer back end (§8.3) | BIN-S | BIN |
| L10 | Sun shadow: cascaded shadow map with a split-map lookup, 1 or 5 comparison taps, tinted by a shadow colour and faded with distance (§9) | BIN-S | BIN |
| L11 | Linear distance fog per vertex: f = clamp((distance − near) / range, 0, 1); additive passes are scaled by (1 − f) (§10) | BIN-S | BIN |
| L12 | Sun direction from the day-description heading and pitch (§8.1) | BIN-C; ENV | BIN |
| L13 | Vertex colours are read only by the 116 shader records whose programs take a colour input (§5.13) | OBS (input signatures of every Direct3D 11 vertex program) | Confirmed |
| L14 | A second render target receives linear view depth (§11) | BIN-S | BIN |
| L15 | Light SED property census: colours, intensity curves, ranges, falloffs (§8.3) | OBS (all 73,711 Light SEDs of every landblock-info record, 2026-10-05) | Confirmed census |
| L16 | Frame ambient = ½(X + Y) and frame hemispheric = ½(X − Y), with X and Y the two ambient colours of the scene, each = ambient colour × ambient brightness (§8.1) | BIN-L (a), (b) | BIN |
| L17 | Per-material ambient inputs: *A₀* = *M* × (frame ambient + a per-object ambient term), *H₀* = *M* × frame hemispheric (§4.2, §8.1) | BIN-L (c) | BIN |
| L18 | A change of a record's sky key cross-fades the lighting values with a sine-shaped factor, field by field (the light direction as a normalised vector); a new day description under the same key does not (§8.1) | BIN-L (b); `environment.md` E27 (the trigger, corrected 2026-10-08) | BIN |
| L19 | Light record: colour × clamp(intensity curve × scale, 0, 1); specular defaults to the diffuse colour; range *R*; *k* = 10 × falloff / *R*²; point or directional by the light's mode (§8.3) | BIN-L (e), (f), (m) | BIN |
| L20 | Light selection: the main light first, then the object's lights in list order; key lights are per-pixel candidates (the first becomes light 0), others go to the per-vertex lists; at most 8 (§8.5) | BIN-L (d) | BIN |
| L21 | Specular colour uploaded black when the specular power is below 0.5 (§5.8) | BIN-L (c) | BIN |
| L22 | No back end uses sRGB texture or frame-buffer formats (§5.1, §15) | BIN-L (h) | BIN |
| L23 | Record colours (packed u32) are 0xAARRGGBB stored as bytes B, G, R, A (§4.4) | BIN-C; BIN (terrain-texturing.md T24) | BIN |
| L24 | Fake directional and point-light inputs come from the main light (§4.2) | BIN-L (g) | BIN |
| L25 | The programs' time input (E20) = the client's global clock in seconds, reduced modulo 3,600 above 3,600 (§4.2) | BIN-L (i) | BIN |
| L26 | Fog distances are limited: far ≤ 10,000 and ≥ 20; near ≤ far − 20 and ≥ −10,000 (§10) | BIN-L (b) | BIN |
| L27 | UV set per texture role: base layers read set 0; the second colour layer reads set 0 in 72 records and set 1 in 3; the baked light mask reads set 1; coordinate inputs 3 and 4 are read only by the terrain layer records (§4.3) | OBS (input signatures of every Direct3D 11 vertex program); BIN-S (third pass, data flow per sampler) | Signatures Confirmed; per-sampler sets BIN |
| L28 | The depth comparison is not part of the record's render state; the back ends take a Direct3D comparison code (1–8) from the engine with each depth state (§6) | BIN-L (j); OBS (`material.md` §7.6) | BIN; the engine's value per pass not traced |
| L29 | Output alpha of unlit blended records by record family (`material.md` §20.1, MA34) | BIN-S (third pass) | BIN |
| L30 | Lights with `Light_BakedLandscapeLight` set are never added to the light lists of terrain or ceiling chunks (§8.3). In normal play their light on the ground comes from the stored light maps only; the client's light-map bake (L35) is the one other path (§8.6) | BIN-L (l), (o) | Light-list rule BIN; "normal play" Inferred (§8.6) |
| L31 | A light reaches an object when the squared distance from the light's position to the object's world bounding box is below ρ², with ρ = Range × (1 − 0.15 × min(Falloff, 1)), or ρ = Range for a `Light_GlobalLight` light (§8.5) | BIN-L (l) | BIN |
| L32 | An object's light list: lights with KeyLight on are put at the front, the others appended, in the order the scene's lights are processed; `Light_OnlyInfluenceParent` and `Light_NeverInfluenceParent` restrict a light to or from the object it is attached to (§8.5) | BIN-L (l) | BIN; the scene's light order not traced |
| L33 | Each light field is registered with the property id of a placed-entity property; table in §8.3 | BIN-L (m); OBS (the property names of those ids in the install's property registry) | BIN |
| L34 | Candidates of the light-list walk are only objects drawn in the current frame and without any of three scene flags; a light with `Light_OnlyInfluenceParent` set and no parent reaches no object (§8.5) | BIN-L (n) | BIN |
| L35 | The client contains a light-map bake: it draws every `Light_BakedLandscapeLight` light that reaches a terrain or ceiling chunk with pass code 0x41 into an off-screen target of at most 1,024 × 1,024 and adds the results into the chunk's baked-lighting texture, which it first clears (§8.6) | BIN-L (o), (p) | BIN |
| L36 | The bake runs only after a landscape renderer flag requests it; that flag is raised only while a renderer switch is on, and the switch is set only in a special client mode; the per-layer pass-0x41 draw items exist only in that mode too (§8.6) | BIN-L (o) | BIN; "not in normal play" Inferred |
| L37 | Indoors (camera inside, outdoor-visible clear) the frame's ambient, fog and main light come from the indoor record of `landblock.md` §4.13 (§8.1) | `landblock.md` §4.13 (BIN2 there) | BIN (by reference) |
| L38 | The world box of the light-list walk is the axis-aligned box around the eight corners of **one** local box of the object, carried through the object's full transform (rotation, scale and translation); the box is not built from the object's parts or vertices. Each object kind supplies its own local box (§8.5 step 2) | BIN-L (q) | BIN; which local box each kind supplies (in particular skinned and instanced objects) not traced |
| L39 | Steps 1–4 of the light-list walk do not test the light's kind: a directional light that is among the walked lights is tested with its own world position and ρ like a point light and is placed in the object's list by the key-light rule of step 4; only the submission routes it by kind (§8.5) | BIN-L (r) | BIN |
| L40 | The switch between the outdoor and the indoor record has no blend of its own; it is smoothed only by the 5 s sun-change fade of `environment.md` §11.6, so it fades exactly when it changes "sun present" (§8.1) | `environment.md` E37 (BIN-E2 there) | BIN (by reference) |

---

<a id="s2"></a>
## 2. Rendering model

<a id="s2-1"></a>
### 2.1 Multi-pass forward lighting (BIN)

The original draws a lit surface in a **base pass** and, for some lights, extra **additive
passes**:

1. The base pass writes depth and colour. It always contains ambient light and distance fog.
   Depending on the permutation it also contains the main directional light (with shadow),
   up to eight point lights evaluated per vertex or per pixel, and one point light evaluated
   per pixel (§3.2).
2. Each additive pass adds one more light (directional, point, or projected) on top. These
   passes do not write depth, use blend factors source-alpha / one, and are scaled by
   (1 − fog) so that fogged distance does not brighten (§6).

Render-state blocks confirm the split (`material.md` §7.6, OBS): every block of pass codes
0x04, 0x05, 0x0A, 0x3E, 0x3F and 0x40 has `depth_write` = 1; every block of codes 0x08,
0x09 and 0x41 has 0; in codes 0x01, 0x02 and 0x03 every block with the additive factors
SRCALPHA / ONE (8,940 blocks) has 0, and the 64 blocks with SRCALPHA / INVSRCALPHA have 1.

Tessera may implement the same model with one base pass that loops over all lights; the
equations of §5 and §6 then sum.

<a id="s2-2"></a>
### 2.2 Back ends (Confirmed)

Every shader record carries the same effect for several graphics back ends; the
`program_kind` of a permutation (`material.md` §7.4) names the back end:

| `program_kind` | Program format | Permutations |
|---|---|---|
| 0 | Direct3D 9, shader model 1.1 | 27 |
| 1 | Direct3D 9, shader model 2.0 | 6,918 |
| 2 | Direct3D 9, shader model 3.0 | 7 |
| 3 | OpenGL: GLSL source text preceded by a table of constant and sampler names | 7,041 |
| 4 | Direct3D 10/11, shader model 4.0 | 16,985 |

The shader model 4.0 programs (kind 4) are the most complete (shadows, per-pixel point
lights, smooth light fade). Where this spec says "the newer back end" it means kind 4; "the
older back ends" are kinds 0–3. Tessera reproduces the kind 4 behaviour. The Direct3D 10/11
and OpenGL back ends are separate libraries next to the executable (BIN-L (h)).

---

<a id="s3"></a>
## 3. Permutation key

<a id="s3-1"></a>
### 3.1 Words (Inferred from census)

The 16-byte permutation key (`material.md` §7.4) is four little-endian u32 words:

| Word | Meaning |
|---|---|
| 0 | Effect-level bits (§3.3) |
| 1 | Feature bits the permutation **requires** |
| 2 | Feature bits the permutation **excludes** |
| 3 | Pass code (§3.2) |

Words 1 and 2 use the same bit numbering: bit 3 is set in word 1 in 6,860 permutations, and
nearly all of their programs take bone matrices (the review census of 2026-10-06, an
independent decoder, found 1 of the 6,860 without a bone-index vertex input and 3 without a
bone constant; not re-run in this revision); bit 3 is set in word 2 in 9,995 permutations and
none of their programs take bone matrices. A
renderer that selects a permutation would take the one whose pass code matches, whose
required bits are all active and whose excluded bits are all inactive. Tessera does not need
to select permutations; it reads the render state of permutations of the right pass code
(`material.md` §11).

<a id="s3-2"></a>
### 3.2 Pass codes (Confirmed census, meanings Inferred from inputs)

Counts are Direct3D 11 permutations (kind 4). "Light 0" is the one light evaluated per pixel
in that pass. "X lights" are the per-vertex (or, with feature bit 19, per-pixel) light lists
of §8.

| Code | Permutations | Render state | Inputs (beyond transforms) | Reading |
|---|---|---|---|---|
| 0x00 | 457 | over all back ends: SRCALPHA / INVSRCALPHA (1,764 blocks), SRCALPHA / ONE (220), ONE / ZERO (97); depth write in most | material colour, alpha-test threshold, often ambient and fog, texture matrix | Single unlit or simply lit pass (effects, unlit records) |
| 0x01 | 604 | SRCALPHA / ONE, no depth write (a few blocks SRCALPHA / INVSRCALPHA) | light 0 directional with shadow map, fog distances | Additive pass: one directional light |
| 0x02 | 2,392 | SRCALPHA / ONE, no depth write (a few blocks SRCALPHA / INVSRCALPHA) | light 0 position and attenuation, cube shadow textures, fog distances | Additive pass: one point light |
| 0x03 | 2,384 | SRCALPHA / ONE, no depth write | light 0 point plus a projector texture and matrix | Additive pass: one projected (cookie) light |
| 0x04 | 596 | SRCALPHA / INVSRCALPHA, depth write | ambient, hemispheric ambient, light 0 directional with shadow, fog | Base pass with the main directional light |
| 0x05 | 2,384 | as 0x04 | ambient, hemispheric ambient, light 0 point, fog | Base pass with one point light |
| 0x08 | 185 | no depth write | glow mask texture, glow colour, alpha test | Glow pass (bloom source) |
| 0x09 | 298 | no depth write | material colour, opacity textures, often bones | Depth-only or occlusion pass |
| 0x0A | 490 | ONE / ZERO, depth write | alpha-test threshold, opacity textures, often bones | Shadow-map depth pass |
| 0x3E | 999 | as 0x04 | ambient, hemispheric ambient, X directional lights, X point lights, fog; **no light 0** | Base pass, all lights per vertex |
| 0x3F | 1,905 | as 0x04 | as 0x3E plus light 0 directional with shadow map | Base pass, sun per pixel |
| 0x40 | 3,801 | as 0x04 | as 0x3E plus light 0 point with optional cube shadow | Base pass, one point light per pixel |
| 0x41 | 192 | no depth write | light 0 point, material colour, opacity | One point light drawn on its own. The client's light-map bake draws terrain chunks with this pass code (§8.6, BIN); other uses not traced |
| 0x42 | 298 | ONE / ZERO | view-space transform, normal map, alpha test | Writes view-space normals (ambient-occlusion input) |

Code 0x07 occurs in 38 permutations of other kinds only.

<a id="s3-3"></a>
### 3.3 Feature bits (Inferred)

| Word / bit | Correlated input (share with bit set / without) | Reading |
|---|---|---|
| word 0 bit 7 | distance fog 0.63 / 0.08; hemispheric ambient 0.50 / 0.00 | Lit or fogged effect |
| word 0 bit 14 | wind constants 1.00 / 0.00 | Wind sway (`material.md` §8.7) |
| word 1 bit 3 | bone matrices 1.00 / 0.00 | Skinned |
| word 1 bit 4 | specular power and light-0 specular colour 1.00 / 0.10 | Specular enabled |
| word 1 bit 15 | point-light cube shadow textures 1.00 / 0.00 | Point-light shadows |
| word 1 bit 19 | point lights evaluated in the pixel program instead of the vertex program | Per-pixel point lights (higher quality) |
| word 1 bit 1 (older back ends) | time constant 0.88 / 0.04 | Animated by time |

> **OPEN:** Bits of word 0 other than 7 and 14 (bit 8 is set in 8,967 permutations; no
> input correlates), and word 1 bits 1, 13 and 19 in the newer back end beyond the reading
> above. Not needed by Tessera.

---

<a id="s4"></a>
## 4. Inputs

<a id="s4-1"></a>
### 4.1 Per-surface inputs (from material parameters)

The material parameters of `material.md` §8 reach the programs as follows (binding
`semantic` numbers of `material.md` §7.3; meanings BIN-S + OBS):

| Parameter (label in enum mapper 0x230001A6) | Semantic | Program input |
|---|---|---|
| 2000 DiffuseColor | 0 | Material colour *M* (RGBA), after the per-object tint of `material.md` §8.3. Pre-multiplied into every light colour and the ambient terms; its alpha is the output alpha factor (§5.12) |
| 2001 SpecularColor | 1 | Material specular colour *Ms*. Pre-multiplied into light specular colours; uploaded black when the specular power is below 0.5 (§5.8) |
| 1000 SpecularPower | 2 | Specular exponent *p* |
| 2014 GlowColor | 3 | Glow and emissive tint *G* |
| 2003 DyeColor, 2013 second dye | 4 | Dye colours *D*, *D2* |
| 4003 DoubleSided | 5 | Cull mode (`material.md` §11.4) |
| 4001 DepthTest | 6 | Depth test on/off |
| 4002 DepthWrite | 7 | Depth write on/off |
| 4004 AlphaTest | 8 | Selects alpha-tested programs |
| 1043 AlphaTestRef | 9 | Alpha-test threshold *τ* |
| 3000–3079 textures | 10 | Sampler `slot` of the permutation |
| 4005, 4006 DiffuseMapWrapU/V | 11, 12 | Address mode U/V of the sampler named by the binding `slot` (1 = wrap, 0 = clamp) |
| 1017/1018, 1033, 1001/1002, 1070/1071 (and the "2", "3" sets) | 38/39, 40, 41/42, 43/44 | Texture matrix whose index is the binding `slot` (§5.2.1; composition `material.md` §8.4) |
| ids whose label starts with `c_` | none, except 2013 (one semantic-4 binding in 7 of its 11 records, naming a permutation without programs, `material.md` §7.3) | Bound by name to the program constant of the same name (`material.md` §8.6); this is how every program receives them, 2013 included |

<a id="s4-2"></a>
### 4.2 Engine-supplied inputs (meanings BIN)

These are the values the engine supplies to the programs in addition to the material
parameters of §4.1. The programs identify each one by a name in their reflection tables; the
dirty room used those names to tell the inputs apart, but Tessera never loads the original
programs, so no interoperability need requires them and this spec describes the inputs
instead (CLEAN_ROOM §5.1). The input names below (E1–E21) are this spec's own. Names that are
quoted elsewhere are material-parameter labels from enum mapper 0x230001A6 (`material.md`
§8.0, §8.6), which are data. "×M" means the engine multiplies the value by the material
colour *M* before upload.

| # | Input | Value | Used for |
|---|---|---|---|
| E1 | Ambient × material | (frame ambient + per-object ambient term) × *M*; alpha = *M*.alpha (BIN-L (c)) | Ambient base term *A₀* (§5.3) and output alpha |
| E2 | Hemispheric × material | frame hemispheric × *M* (BIN-L (c)) | Hemispheric term *H₀* (§5.3) |
| E3 | Ambient, hemispheric (without *M*) | frame ambient + per-object ambient term; frame hemispheric | §5.3, in programs that apply *M* themselves |
| E4 | Up vector | world up (+Z) in model space | §5.3 |
| E5 | Light 0 colour and range | light 0 colour ×M; fourth component = squared range *R²* (point lights) | §5.4, §8.3 |
| E6 | Light 0 specular colour | light 0 specular colour × *Ms* | §5.8 |
| E7 | Light 0 direction | unit vector toward a directional light 0 (the negated travel direction), in model space | §5.4 |
| E8 | Light 0 position and attenuation | point light 0 position (model or world space); fourth component = attenuation factor *k* | §8.3 |
| E9 | Point-light lists | arrays of 8 point lights: colour ×M with *R²*, world position with *k*, specular colour | §5.6 |
| E10 | Point-light count | number of valid entries in the arrays (0–8) | §5.6 |
| E11 | Extra directional light | colour ×M, world direction, count 0 or 1 | §5.6, §8.2 |
| E12 | Distance fog | fog colour, start distance, (end − start) | §10 |
| E13 | Sun shadow | world-to-light clip matrix, shadow quality, fade parameters, landscape shadow colour, shadow colour range | §9 |
| E14 | Texture matrices 0–3 | per-surface texture transforms | §5.2.1 |
| E15 | Eye position | camera position in world and model space | specular, fog |
| E16 | Material values | the §4.1 values: specular power, specular colour, glow colour, dye colours, alpha-test threshold | §5 |
| E17 | Terrain blend maximum | material parameter 1203 (label `c_AlphaBlendMax`); in the terrain layer records the weight of the squared mask (`terrain-texturing.md` §7) | terrain opacity |
| E18 | Fake directional light | direction, colour and specular colour: the main light's travel direction, colour and specular colour when the main light is directional; otherwise a default direction, a grey of 0.6 and an engine specular colour (BIN-L (g)) | unlit effects and water (`water.md` §8.4) |
| E19 | Fake point light | position with *k*, colour with *R²*, specular colour: the main light's values when the main light is a point light; otherwise zero position and colour and an engine specular colour (BIN-L (g)) | unlit effects |
| E20 | Time | the global clock of `material.md` §5.3.1 in seconds, as a float, reduced modulo 3,600 once it exceeds 3,600 (BIN-L (i)) | animated effects |
| E21 | Other effect inputs | a metallic-specular amount (44 shader records), opacity-fog properties, fake ambient terms | OPEN (§15) |

The per-object ambient term and a per-object tint of *M* are supplied by the drawn object
(`material.md` §8.3); for ordinary world objects they are zero and white (Inferred).
**Tessera Decision:** zero and white.

<a id="s4-3"></a>
### 4.3 Vertex inputs (signatures Confirmed; data flow BIN)

Input signatures of the Direct3D 11 vertex programs (OBS): position, normal, one UV set,
tangent and binormal in 7,003 permutations; plus bone indices and weights in 4,507; a
**colour** input in 1,673 + 1,673 + smaller groups. 116 shader records (2,624 surface records)
take a colour input; no other record reads vertex colours (§5.13). UV sets: 12,904 Direct3D 11
vertex programs read UV set 0 only and 792 read none.

**Which UV set feeds which texture** (L27; OBS 2026-10-05 over every kind-4 permutation,
following each sampler's coordinate back through the pixel and vertex programs, BIN-S third
pass):

| Texture role (sampler name) | UV set read | Records (surface records) |
|---|---|---|
| Base colour and normal map (`Color…`, `ColorAnd…`, `TSNormal…`, `DiffuseTexture`, …) | set 0 in every record used by surface records (transformed by texture matrix 0 where the permutation has one, §5.2.1) | all |
| Second colour layer (`Color2`, `ColorAndOpacity2`, …) | set 0 in 72 records (664 surface records); **set 1, unchanged**, in 3 records (0x2B0006F7, 0x2B000734, 0x2B0007E6; 82 surface records); no UV input in 3 records (1 surface record) | 78 |
| `DirLightMaskAndAmbOcc` (baked light mask and ambient occlusion) | set 1 | 2 (0x2B00070E, 0x2B00078E; 6 surface records) |
| `BakedLight` | coordinate input 3 | 96 terrain layer records only |
| `BakedShadow` | coordinate input 4 | 192 terrain layer records only |
| Projector and shadow lookups of one effect record (0x2B0007A9) | sets 1 and 2 together with the position | 1 (3 surface records) |
| Terrain layer textures | input 0, 1 or 2 by projection | 192 terrain layer records (`terrain-texturing.md` §6.1) |

Consequences:

- **Coordinate inputs 3 and 4 are read only by the terrain layer records** (input 3 by 144,
  input 4 by all 192), where they are the mask/light-map set and the baked-shadow set
  (`terrain-texturing.md` §6.1). No shader record used by a surface record reads them, so on
  meshes UV sets 3 and above have no use in drawing.
- On meshes, UV set 1 is read only by the five records named above (88 surface records in
  all: a second colour layer in three, the baked light mask in two) and by 0x2B0007A9, and UV
  set 2 only by 0x2B0007A9 (records used by surface records). The mesh UV sets 1 … 6 of `mesh.md` §5.5 are otherwise unused by the programs.
- Example of the common two-layer form: 0x2B0006E8 (`ColorAndOpacity` / `ColorAndOpacity2`)
  reads UV set 0 for both layers and differs only in the texture matrix (§5.2.1). The water
  shader 0x2B0000A8 does the same, but only in its one Direct3D 11 permutation with `Color` /
  `Color2` samplers (permutation 8 of 13; its OpenGL twin is permutation 9); its eight main
  water permutations read only the position (`water.md` §8.2–§8.3).

> **OPEN:** What 0x2B0007A9 (3 surface records) does with UV sets 1 and 2 (they enter its
> projector and shadow-split lookups); and the meaning of UV sets 3 … 6 stored on 444 mesh
> submeshes (`mesh.md` §5.5), which no program used by a surface record reads.

<a id="s4-4"></a>
### 4.4 Colour encodings (the single statement other specs refer to)

| Where | Stored form | Channel order | Mark |
|---|---|---|---|
| Packed 4-byte colours inside records: day-description keyframes and sky objects (`environment.md` §5.3–§5.6), terrain type entries (`terrain-texturing.md` §3.3) | u32 little-endian | value 0xAARRGGBB, so the bytes are **B, G, R, A**; each channel / 255 | BIN (one client routine serves both; BIN-C and `terrain-texturing.md` T24) |
| Material and shader colour parameters (group 2000) | four f32 | R, G, B, A | Confirmed (`material.md` §5.2) |
| Property values of type colour (code 16), for example `LightDiffuse`, `InteriorFogColor` | 4 × u8 (`property-system.md` §4.7) | not traced in the client | Inferred **B, G, R, A** |

Evidence for the last row: read as B, G, R, A, the two most common stored `LightDiffuse`
values (33, 136, 220, 255) and (0, 140, 255, 255) are warm orange (red 220 and 255) — torch
and lamp light — while the other order would make most lights blue. **Tessera Decision:** read
property colours as B, G, R, A until `property-system.md` §4.7 settles it.

The client uses all colours as stored: no back end converts textures or colours from sRGB
(§5.1, §15).

---

<a id="s5"></a>
## 5. Lit base pass (BIN)

<a id="s5-1"></a>
### 5.1 Notation

All vectors are unit length unless stated. *N* is the shading normal (§5.2.4), *L* the unit
vector from the surface toward a light, *V* the unit vector from the surface toward the eye,
*U* the world up direction. "sat" clamps to [0, 1]. Colours are RGB triples combined
component-wise. Texture values are used as sampled, and the original samples them as stored:
neither the Direct3D 10/11 nor the OpenGL back end creates sRGB textures or an sRGB frame
buffer (BIN-L (h)), so all arithmetic happens on the stored values (gamma space).

<a id="s5-2"></a>
### 5.2 Surface terms

1. **Texture coordinates.** <a id="s5-2-1"></a>The common programs read UV set 0 (§4.3). A
   permutation with texture transforms computes each layer's coordinates as an affine 2-D
   transform of (u, v, 1): the first two rows of a 4 × 4 matrix, using its first, second and
   fourth columns (u′ = m00·u + m01·v + m03, v′ = m10·u + m11·v + m13). The base colour uses
   matrix 0, the second colour layer matrix 1; the normal map uses the base colour's
   coordinates in the common permutations. Permutations without transforms pass (u, v)
   unchanged. Which UV set each texture reads is tabled in §4.3 (a few records read the
   second colour layer and the baked light mask from UV set 1).
   The engine builds the matrix from the early translation, scale, rotation (counter-clockwise,
   degrees) and translation in that order (`material.md` §8.4, BIN).
2. **Albedo** *C*: RGB of the base colour texture (slot 3000). With a second colour layer
   (slot 3001) the programs combine the layers differently per record: the
   `ColorAndOpacity` / `ColorAndOpacity2` record 0x2B0006E8 multiplies the RGB values and
   the alphas (the product is the opacity); the `ColorAndDye` / `ColorAndDye2` record
   0x2B0006E4 adds the RGB values. The combine is coded in each record's programs; no data
   field selects it (§15). **Tessera Decision:** multiply for `…Opacity2` roles, add for
   `…Dye2` roles, multiply otherwise.
3. **Dye.** With a dye mask *a* (the alpha of the role carrying `Dye`, `material.md` §12.2)
   and dye colour *D*: *C* ← *a* · *C* + (1 − *a*) · (*C* × *D*). This is `material.md` §12.3's
   smooth form, applied to the sampled values before lighting. A second dye colour *D2*
   (2013) uses a second mask *a2* (the `DyeAlpha` texture's alpha in record 0x2B00072C, the
   second layer's alpha in 0x2B0006E4) and is applied to the **undyed** albedo *C₀*:
   *C* ← *a2* · (result of the first dye) + (1 − *a2*) · (*C₀* × *D2*).
4. **Normal.** <a id="s5-2-4"></a>With a normal map, the texel (r, g, b) gives
   *n* = (2r − 1, 2g − 1, 2b − 1), normalised. The shading normal is
   *N* = *n*.x · **T** − *n*.y · **B** + *n*.z · **N₀**, where **T** and **B** are the stored
   tangent and binormal and **N₀** the stored vertex normal (`mesh.md` §5.6; tangent = +U,
   binormal = +V). Because V grows down the image, positive green tilts the normal toward
   the top of the texture image. Without a normal map *N* = **N₀** (normalised after
   transformation). Terrain layers build **T** and **B** per vertex and rotate *n* by the
   texture transform first (`terrain-texturing.md` §6.4).
5. **Gloss** *g*: the alpha of the role that carries `Gloss` (`material.md` §10.2), else 1
   for shaders that compute specular, else unused.

<a id="s5-3"></a>
### 5.3 Ambient

*A* = sat(*A₀* + (*N* · *U*) × *H₀*), where *A₀* is the ambient input × *M* and *H₀* the
hemispheric input × *M* (§4.2, §8.1). Surfaces facing up receive *A₀* + *H₀*, surfaces
facing down *A₀* − *H₀* (each clamped). Shaders without the hemispheric input use *A₀* alone.

<a id="s5-4"></a>
### 5.4 Main directional light (sun or moon)

*D₀* = sat(*N* · *L₀*) × *S* × *E₀*, where *L₀* is the unit vector toward the light, *E₀* the
light colour × *M* and *S* the shadow factor of §9 (1 without shadows).

<a id="s5-5"></a>
### 5.5 Point light 0

In pass codes 0x05 and 0x40: *P₀* = sat(*N* · *L*) × *a*(*d*) × *E₀*, with the attenuation
*a* of §8.3 and the optional cube shadow (OPEN).

<a id="s5-6"></a>
### 5.6 Light lists

The vertex program (or, with feature bit 19, the pixel program) sums
*X* = Σ sat(*N* · *Lᵢ*) × *aᵢ*(*dᵢ*) × *Eᵢ* over the first *n* entries of the 8-entry
point-light arrays (*n* from the count input), plus sat(*N* · *L_d*) × *E_d* for the extra
directional light when its count is 1. In the per-vertex form *N* is the vertex normal and
the sum is interpolated; the per-vertex form uses a hard range cut-off (§8.3). Which lights
fill the arrays is §8.5.

**Tessera Decision:** Tessera may evaluate every listed light per pixel, with the smooth fade
of §8.3 step 3 and the clamped specular of §5.8, instead of the per-vertex form (a quality
choice; the original's per-vertex lists have the hard cut-off and no specular). Tests must not
treat the per-pixel result as the original's.

<a id="s5-7"></a>
### 5.7 Combination

Light sum *Σ* = *A* + *X* + *D₀* (or + *P₀*). The lit colour is **2 × *C* × *Σ***. The factor 2
is applied to every lit pass (base and additive) in every back end; light colours of about 0.5
therefore reproduce the albedo.

<a id="s5-8"></a>
### 5.8 Specular

When the permutation has specular (feature bit 4), for each per-pixel light *j* with
unit half-vector *Hⱼ* = normalise(*V* + *Lⱼ*):

- term = *g* × sat(*N* · *Hⱼ*)^*p* × *Fⱼ*, where *Fⱼ* is the light's specular colour × *Ms*;
- for light 0 the term is multiplied by the shadow factor *S* (it is not multiplied by
  *N* · *L*); for point lights it is multiplied by the attenuation *a*; in the per-pixel
  point-light list the product *g* × *a* × sat(*N* · *H*)^*p* is clamped to [0, 1] before
  the colour multiply, in the additive point pass it is not.

The specular terms are **added after** the factor 2 of §5.7. The power is applied as
exp2(*p* × log2(x)). The programs do not special-case *p* = 0, but the client uploads a black
specular colour *Ms* whenever the effective power is below 0.5 (BIN-L (c), `material.md` §8.3),
so such surfaces have no highlight.

<a id="s5-9"></a>
### 5.9 Emissive and glow

With an emissive texture *Em* (slot 3018): colour ← 2 × *C* × *Σ* + *Em*.rgb × *G*, before fog.
The glow pass (code 0x08) draws the glow mask (slot 3013) times *G* into a separate target for
bloom; its composition is OPEN. (Tessera's interim rule, adding the glow term like the emissive
one, is a Decision of `render.md` §4.1, not a fact of this spec.)

<a id="s5-10"></a>
### 5.10 Environment reflection

Records with an `Environment` role (slot 3015, sampled as a 2-D sphere-style map): reflect the
view vector about *N* in the tangent frame to get *R*; sample at
(*R*.x, *R*.y) / (2.5 × |*R*|) + (0.5, 0.5); add the sample × *g* to the albedo *C* **before**
lighting. Cube-map variants (`EnvironmentCube`) occur in a few permutations; their lookup is OPEN.

<a id="s5-11"></a>
### 5.11 Fog

Final RGB = lerp(colour, fog colour, *f*) with the per-vertex fog factor *f* of §10.

<a id="s5-12"></a>
### 5.12 Alpha

- **Alpha test**, when the program is the alpha-tested variant: the opacity texel *α* (the
  §11.2 source of `material.md`; with two layers the product of both alphas) is compared
  with *τ*; the fragment is **discarded when *α* ≤ *τ*** (kept only when *α* > *τ*). The test
  uses the texture opacity only, before any vertex-colour alpha (§5.13).
- **Output alpha** = *α* × *M*.alpha when the base role carries opacity, else *M*.alpha.
  With the base pass blend factors SRCALPHA / INVSRCALPHA (`material.md` §7.6) this blends:
  texels just above the threshold are slightly translucent, and a material colour with
  alpha 0 makes the surface invisible.
- Additive passes output *M*.alpha as alpha.

<a id="s5-13"></a>
### 5.13 Vertex colours (Confirmed census, use BIN)

Only shader records whose programs take a colour input read vertex colours (116 records,
2,624 surface records, §4.3). In those, the vertex colour multiplies *M*: the lit RGB is
multiplied by the vertex RGB, and the output alpha by the vertex alpha. The alpha test
(§5.12) is performed before that multiply. Every other shader record ignores vertex colours.

---

<a id="s6"></a>
## 6. Additive light passes (BIN)

Codes 0x01 (directional), 0x02 (point), 0x03 (projected). Each pass computes, for its one
light:

- diffuse = sat(*N* · *L*) × *a* × *E* (× *S* for 0x01, × projector texture for 0x03);
- specular as §5.8 when present;
- colour = (2 × *C* × diffuse + specular) × (1 − *f*);
- alpha = *M*.alpha; blended SRCALPHA / ONE without depth write.

Depth comparison (BIN-L (j)): it is not stored in the render-state block (`material.md` §7.6).
The graphics back ends receive it from the engine with each depth state: the Direct3D 10/11
back end takes a Direct3D comparison code (1–8) and passes it on one to one, and the OpenGL
back end applies the comparison stored in its own depth-state object. Which code the engine
sets for the base and additive passes was not traced.

> **OPEN:** The depth comparison the engine uses for the additive passes (and for terrain
> layers drawn on top of each other, `terrain-texturing.md` §8 step 5). **Tessera Decision:**
> less-or-equal, so that an additive pass lands on the base pass's pixels.

The projector texture is sampled with the light's projector matrix applied to the world
position; its exact projection is OPEN.

---

<a id="s7"></a>
## 7. Other passes

| Code | Behaviour |
|---|---|
| 0x00 | Unlit or single-pass effect: base texture × material colour (and vertex colour when present), optional ambient, fog, alpha test. Blend as `material.md` §11.3 |
| 0x09, 0x0A | Depth only (0x0A for the sun shadow map, with alpha test against *τ* so cut-outs cast cut-out shadows) |
| 0x42 | Writes the view-space normal (from the normal map when present) with alpha test; input to screen-space ambient occlusion, which Tessera may skip |
| 0x08 | Glow, §5.9 |

---

<a id="s8"></a>
## 8. Light sources

<a id="s8-1"></a>
### 8.1 Sun, ambient and fog from the environment (BIN)

**Keyframe values.** The day description of the current sky (`environment.md` §5.2,
keyframes §5.3) gives, after keyframe blending at the current time of day
(`environment.md` §11.3):

- heading *h* and pitch *e* in degrees; the unit vector **toward** the light is
  (sin *h* · cos *e*, cos *h* · cos *e*, sin *e*) in world space (Z up, `coordinates.md`);
  the client stores its negation as the light's travel direction;
- directional colour *Cd*, brightness *bd* and specular colour *Cs*;
- ambient brightness *ba* and two ambient colours *Ca1*, *Ca2*;
- fog near, far and colour.

With no day description the client uses: ambient brightness 0.7, ambient colours white,
directional brightness 0.1, colours white and travel direction (1, 0, −0.4).

**Cross-fade (BIN-L (b), L18).** When a record's sky key changes (a new sky), the client
builds the lighting values of the old and the new state and interpolates them with the factor
*f* = ½ + ½ · sin(π · (min(*t* / *T*, 1) − ½)), where *t* is the time since the change on the
client's raw timer and *T* the transition time (`environment.md` §11.6 steps 1–4). Numbers and
colour channels blend linearly; the light direction blends as a unit vector and is normalised,
and the sun, fog and `effect` follow the rules of `environment.md` §11.6 step 4. Below
*f* = 0.0002 the old values are used unchanged, above 0.9998 the new ones. A new day
description under the same sky key (a new game day or day type) is **not** cross-faded
(`environment.md` E27; the first text of this paragraph said it was, corrected 2026-10-08).

**Ambient (BIN-L (a), (b), (c); L16, L17).** The client forms two ambient colours

- *X* = *Ca1* × *ba*, *Y* = *Ca2* × *ba* (RGB),

and from them the frame inputs

- frame ambient = ½ (*X* + *Y*), frame hemispheric = ½ (*X* − *Y*),

so that an up-facing surface receives *X* and a down-facing one *Y* (§5.3). Per material,
*A₀* = *M* × (frame ambient + the per-object ambient term of §4.2) and *H₀* = *M* × frame
hemispheric. Two engine options can override *X* and *Y*: debug lighting modes set both to 0.5
or to 1, and an adjustment option adds engine offsets and clamps each channel to at most 1;
neither is active in normal play (Inferred). An override held by the environment can replace
*X* and *Y* when the scene record is built (BIN-L (b)).

**Indoors (L37).** The client builds two environment records, outdoor and indoor, and uses
the indoor one while the camera is inside (`landblock.md` §4.13 gives the rules and the
property lookup). For lighting this means: in the indoor record *X* = `InteriorUpperAmbientLight`
+ `InteriorAmbientLight` and *Y* = `InteriorLowerAmbientLight` + `InteriorAmbientLight`
(an absent colour counts as black; every channel limited to at most 1; the keyframe brightness
*ba* is not applied) when any of the three properties is present, otherwise the keyframe
values of the indoor sky key; the frame inputs are then formed from *X* and *Y* as above. The
indoor record has **no main directional light** (no sun or moon, so no sun shadow either), and
its fog follows the interior overrides and the limits of §10.

**Switching between the records (L40).** "Immediate" means that the switch has no transition
of its own: on every environment update the client takes the record selected by the camera
(indoor or outdoor), without blending the two records and without starting the sky-key
cross-fade above, which belongs to each record separately. The record taken then passes
through the client's **sun-change fade** (`environment.md` §11.6, E34, E37), and that fade is
what the frame sees:

- **The switch changes "sun present"** (the outdoor record has a main light; the indoor record
  never has one): the lighting values (*X*, *Y*, *Cd*, *bd*, *Cs*, the light direction and the
  fog) fade from the values shown before the switch to the new record's over 5 seconds of the
  raw timer, with the weight shape of the cross-fade above. During those 5 s the main light
  stays on, its colour fading toward black, so stepping indoors by day dims the sun over 5 s
  rather than cutting it.
- **The switch does not change "sun present"** (the outdoor record has no main light, see
  **Sun** below): the new record's values are used from the same update on, a hard cut, unless
  a sun-change fade is still running, which then continues toward the new record.
- **An update marked immediate** (`environment.md` §11.6 step 2; Inferred there: entering the
  world or a teleport): the new record's values at once in every case.

`landblock.md` §4.13 says the same in one word ("changing from one record to the other is
immediate"); this paragraph is the full statement.

**Sun.** The environment's lighting record carries *Cd*, *bd*, *Cs* and the travel direction
separately (BIN-L (b)); the main light's colour *E₀* (before ×*M*) and specular colour are its
light record (§8.3). The outdoor record has **no** main light (no sun light, no sun shadow)
when the keyframe light colour *Cd* is opaque black within 0.0002 per channel, when *bd* is
below 0.0002, or while a keyframe `effect` is active (`landblock.md` §4.13 and
`environment.md` §11.7 step 9, "sun present"); the indoor record never has one.

> **OPEN:** How *Cd* and *bd* combine into the main light's colour, and the main light's
> specular colour. **Tessera Decision:** *E₀* = *Cd* × *bd* and specular = *Cs*. Support
> (BIN-C): for keyframes from data versions without *Cs*, the client derives *Cs* as a grey
> clamp(2 × *bd* × mean(*Cd*), 0, 1), which is the strength of *Cd* × *bd* after the lit
> pass's factor 2.

<a id="s8-2"></a>
### 8.2 Extra directional light (BIN-L (d); use Inferred)

The per-vertex directional slot (§5.6, count 0 or 1) receives a directional light that is
submitted to the per-vertex lists (§8.5) rather than as light 0. The main directional light
goes to light 0 when the pass has one; in pass 0x3E, where no light is evaluated per pixel,
Tessera can treat the per-vertex directional light as the sun at lower quality.

<a id="s8-3"></a>
### 8.3 Point lights, light records and attenuation

**Attenuation (BIN-S).** For a light at position *Q* with squared range *R²* and attenuation
factor *k*, at distance *d* = |*Q* − *P*|:

1. If *d²* ≥ *R²* the light contributes nothing.
2. Otherwise *a* = 1 / (1 + *k* · *d²*).
3. Newer back end, light 0 and per-pixel lists only: when *d²* > 0.75 *R²*, multiply *a* by
   the smoothstep *s*²(3 − 2*s*) with *s* = (*R²* − *d²*) / (0.25 *R²*), so the light fades to
   zero at the range. The per-vertex lists use step 1–2 only.

**Light fields (BIN-L (f), (m); L33).** The client's light class registers each field with
the id of the placed-entity property that fills it. The fields the rules of this spec use, with
the property names those ids carry in the install's property registry (names quoted for
interoperability: a reader finds the stored properties by these names; the ids are this
install's and are read from the registry at run time):

| Light field | Property (id in this install) | Used in |
|---|---|---|
| Range | `LightRange` (0x3FE) | light record, selection radius (§8.5) |
| Diffuse | `LightDiffuse` (0x3FD) | light record |
| DiffuseIntensity (float curve, `material.md` §5.3) | `LightDiffuseIntensity` (0x4F0) | light record |
| Specular | `LightSpecular` (0x4EF) | light record |
| Falloff | `LightFalloff` (0xF5) | light record, selection radius |
| KeyLight | `KeyLight` (0x455) | §8.5 |
| BakedLandscapeLight | `Light_BakedLandscapeLight` (0x7C9) | left out of the light lists of terrain and ceiling chunks (below); input of the light-map bake (§8.6) |
| OnlyInfluenceParent | `Light_OnlyInfluenceParent` (0x824) | §8.5 |
| NeverInfluenceParent | `Light_NeverInfluenceParent` (0x823) | §8.5 |
| GlobalLight | `Light_GlobalLight` (0x84A) | selection radius (§8.5) |
| DrivenByDayCycle | `Light_DrivenByDayCycle` (0x707) | day and night variants (below) |

Direction has no property. The other fields (projector, corona, shadow-casting, baked-shadow
and level-of-detail settings) are not used by this spec. A new light starts with: point mode, Range 25, Diffuse white,
DiffuseIntensity a constant 0.5, Specular white, Falloff 1.0, KeyLight off, a run-time scale
*σ* = 1 (BIN-L (e)).

**Light record (BIN-L (e), L19).** Every frame the client builds a light record from the
fields, with *t* the global clock (`material.md` §5.3.1):

| Record value | Rule |
|---|---|
| type | point unless the light's mode says directional (the main sun or moon light) |
| colour (RGB) | Diffuse × clamp(DiffuseIntensity(*t*) × *σ*, 0, 1) |
| specular (RGB) | Specular × clamp(2 × *σ* × DiffuseIntensity(*t*), 0, 1); when KeyLight is off and Specular equals white (within 0.0002 per channel), the Diffuse colour is used instead of Specular |
| range *R* | max(Range, 0.0002); uploaded squared as *R²* |
| attenuation *k* | 10 × Falloff / *R²* |
| position | the entity's world position plus an offset |
| direction | the Direction field (directional lights) |

So a light reaches 1 / (1 + 10 × Falloff) of its strength at the range before the fade of step
3; for Falloff 0.5 it is at half strength at *d* = *R* / √5 ≈ 0.45 *R*.

*σ* is a run-time factor set by other code (for example lights driven by the day cycle);
where it is set was not traced. **Tessera Decision:** *σ* = 1, and lights with
`Light_DrivenByDayCycle` use the day or night property variants by the time of day
(`Light.Script@Entity.Day.*`, `…Night.*`).

Census of the 73,711 Light SEDs in all landblock-info records (OBS, 2026-10-05):

| Property | Present on | Values |
|---|---|---|
| `LightDiffuse` (colour) | 72,465 | 4,525 distinct; stored bytes such as (33, 136, 220, 255) and (0, 140, 255, 255) are the most common: warm orange in the B, G, R, A reading of §4.4 |
| `LightDiffuseIntensity` (float curve, `material.md` §5.3) | 67,022 | constant 1.0 (14,152), 0.5, 0.4, 0.7, 0.3, 0.8, …; mode 7 noise around 0.8 ± 0.2 at frequency 2 (6,476, a flicker), mode 8 fractal noise (8,095 in all), mode 4 sine (271). Lights without it use the default 0.5. |
| `LightRange` (f32) | 71,617 | metres: 10 (9,742), 15, 7, 20, 12, 8, 25, 5, 8.5, 30, 40, … (701 distinct) |
| `LightFalloff` (f32) | 60,616 | 0.5 (14,773), 0.3 (14,710), 0.2, 0.1, 0.4, 1.0, … in [0, 1] |
| `LightSpecular` (colour) | 3,383 | white (1,015) and others |
| `KeyLight` (bool) | 11,865 | true 5,090 |
| `Light_DrivenByDayCycle` | 868 | true 250 |
| `Light_BakedLandscapeLight` | 23,765 | true 23,324 |
| `Light_GlobalLight` | 527 | true 360 |

**Baked landscape lights (BIN-L (l), L30; answered 2026-10-07).** When the client builds the
light lists (§8.5), a light whose `Light_BakedLandscapeLight` field is set is skipped for every
terrain chunk and every ceiling chunk (`terrain-texturing.md` §4.8), whether or not the block
has a light map; it still reaches every other object in range. In normal play its light on the
ground comes only from the landblock light maps (`terrain-texturing.md` §10); a block without
a light map shows no light from it on the ground. The client also contains a light-map bake
that draws such lights into the chunks' baked-lighting textures, but it is tied to a special
client mode and is not part of normal play (§8.6; the "normal play" part is Inferred). (The
former Tessera Decision excluded such lights only from chunks with a light map; the client
excludes them from all terrain and ceiling chunks.)

> **OPEN (for implementers, 2026-10-07):** the walker's terrain light selection
> (`walker.md` §14.3: baked lights left out only for chunks with a light map, range sphere
> against bounding sphere) and the walker's frame setup (no selection radius ρ, key-light or
> baked-landscape inputs passed to the renderer's automatic selection, `render.md` §4.3) still
> follow the former Tessera Decisions. Under the rules of this section and §8.5, a
> `Light_BakedLandscapeLight` light reaches no terrain or ceiling chunk, light map or not.
> Implementers align that code with §8.3 and §8.5; until then its behaviour is a known
> difference from the original, not a Tessera Decision.

> **OPEN (implementer, 2026-10-06, `tessera-world` lighting module):** two inputs of the
> light record are not specified. (a) The "offset" added to the entity's world position:
> its source was not traced; `tessera-world` uses the SED's placement position (offset 0).
> (b) The day and night variants: the names quoted in `environment.md` §7.3 are
> `Light.Script@Entity.Night.m_fLightRange` and `…Day.m_bLightActivated` "and similar";
> the full list of varying fields and which value applies when a variant is absent are not
> given. `tessera-world` reads only `…Day.` / `…Night.m_fLightRange` and
> `…m_bLightActivated` (absent: the light's own range, and on), and only for lights with
> `Light_DrivenByDayCycle`; night is the rule of `sound.md` §10.3.5.

<a id="s8-4"></a>
### 8.4 Projector lights

Pass 0x03 adds a light whose colour is modulated by a projected texture (`LightProjector`
sampler and a projector-matrix input; the light fields ProjectorEnabled, ProjectorTexture and the
yaw, pitch and roll curves). The projection is OPEN.

<a id="s8-5"></a>
### 8.5 Which lights an object receives (BIN-L (d), L20)

For each drawn object the client submits lights in this order, at most 8 to the per-vertex
lists and at most 8 in all (the main light included):

1. the **main light** of the scene (the sun or moon, or a point light standing in for it):
   to light 0 when it is directional; to the per-vertex point list when it is a point light;
2. the lights in the **object's light list**, in list order:
   - with the client's per-pixel lighting option on, a light with KeyLight on (or every light
     when an engine option forces it) is a per-pixel candidate: the first such light becomes
     light 0 of its kind (a pass has one directional and one point light-0 slot), later ones
     go to the per-vertex lists;
   - every other light goes to the per-vertex lists (point lights to the point arrays, a
     directional light to the extra directional slot, §8.2);
   - with the per-pixel option off, every light goes to the per-vertex lists.

Each submitted light's colour is multiplied by a scale argument, 1 in all submissions found.
When there is no main light, one submission path adds a default directional light whose
shipped colour is black (an engine value that may change at run time).

**Building an object's light list (BIN-L (l), (n); L31, L32, L34; answered 2026-10-07 except the
scene order).** Once per frame the client walks the scene's placed lights and, for each light,
the drawn objects near it:

1. **Selection radius.** ρ = Range × (1 − (1 − 0.85) × min(Falloff, 1)), so a light with
   Falloff 1 or more reaches 0.85 of its Range and one with Falloff 0 its full Range; for a light
   with `Light_GlobalLight` set, ρ = Range. (0.85 is a client constant, decision 0012.) Note
   that ρ ≤ *R*, the range of the light record above, so the record's fade near *R* is often
   outside the selection radius.
2. **Test.** The candidates are the objects near the light that were **drawn in the current
   frame** (for the view being built) and that carry **none of three scene flags** of the
   object (L34; what the three flags mean was not traced). A candidate is reached when the
   squared distance from the light's world position to the object's world-space axis-aligned
   bounding box (0 inside the box) is less than ρ². That box is the axis-aligned box around
   the eight corners of one local box of the object after the object's full transform
   (rotation, scale and translation), not a box of its parts or vertices (L38); each object
   kind supplies its local box, and which box a skinned or instanced object supplies was not
   traced. The light's kind plays no part in steps 1–4: a directional light that is among the
   walked lights is tested with its own world position and ρ like a point light (L39).
3. **Exclusions.** A light with `Light_BakedLandscapeLight` set skips terrain and ceiling
   chunks (above). The parent rule (L34): for the object the light is attached to (its
   parent), the light is added only when `Light_NeverInfluenceParent` is off; for **every
   other object** it is added only when `Light_OnlyInfluenceParent` is off. The second test is
   made whether or not the light has a parent, so a light with `Light_OnlyInfluenceParent` set
   and no parent (for example a Light SED of landblock info with that property true) reaches
   no object at all. One special light record of the client has its parent looked up through
   another client object instead of its own parent field (not traced; it is not a placed
   light).
4. **Order.** A light with KeyLight on is inserted at the **front** of the object's list, any
   other light is appended at the end. So the list holds the key lights first (the one
   processed last at the very front), then the others in processing order. The submission of
   this section takes lights from the list in that order, which decides which lights fill the
   eight slots.

> **OPEN:** The order in which the client walks the scene's lights (step 4), which decides
> the order inside each group. **Tessera Decision:** order each group (key lights, then the
> others) by distance from the object, nearest first. The choice matters only for objects
> reached by more than eight lights.

> **OPEN (implementer, 2026-10-06, `tessera-world` lighting module):** whether the default
> directional light added when there is no main light occupies the directional light-0
> slot, and whether it counts toward the 8 lights in all. Tessera (Decision): it stands in
> for the main light: it holds the directional light-0 slot unless a per-pixel directional
> light from the object's list takes that slot (then no default light is added), and it
> counts toward the 8.

> **OPEN (implementer, 2026-10-07, `tessera-render` automatic selection):** three inputs of
> the steps above are not specified. (a) Whether the parent switches of step 3 restrict a
> light that is attached to no object (for example a Light SED of landblock info with
> `Light_OnlyInfluenceParent` set). (b) How the object's world-space box of step 2 is formed
> for a rotated or scaled object (from the model-space box, from the parts, or from the
> vertices) and which box a skinned or instanced object uses. (c) Where an extra directional
> light (§8.2) of the light list stands relative to the point lights, since it has no
> position for the distance test.
>
> **Answered (spec writer, 2026-10-07, sixth revision):** (a) by BIN-L (n), step 3 above: the
> only-parent switch is applied to every object other than the parent, so an unattached light
> with `Light_OnlyInfluenceParent` set reaches nothing. The former Tessera Decision (a) of
> `render.md` §4.3 ("the switches apply only to attached lights") differs from the original
> and is withdrawn; implementers align the renderer's automatic selection. (b) and (c) stay
> open.
>
> **Answered in part (spec writer, 2026-10-08, seventh revision, BIN-L (q), (r)):**
> (b) The client forms the box as the Tessera Decision below does for a rigid object: the
> axis-aligned box around the eight corners of the object's local box after its full
> transform, rotation and scale included (L38); it never builds the box from parts or
> vertices. Which local box a skinned (animated) object or an instanced object supplies, and
> whether the box follows the animated pose, were not traced; for those the Decision stays.
> The client builds a list for each object against that object's own box (L32), so a
> per-instance selection, which `render.md` §4.3 now makes, is the client's behaviour for
> instances that are separate objects. (c) A directional light has a world position like any
> placed light, and the walk tests it by the same box distance and ρ as a point light (L39);
> it stands in the object's list where step 4 puts it (at the front when KeyLight is on,
> otherwise in walk order with the other lights), not after the point lights. The submission
> then routes it by kind (per-pixel candidate if key, else the extra directional slot, §8.2).
> **Still open:** which directional light the submission keeps when an object's list holds
> more than one non-key directional light (there is one extra directional slot), and whether
> a second key directional light can take the directional light-0 slot when the pass has no
> main light (`render.md` §4.3 implementer OPENs (c), (g), (n)); the submission routine
> itself was not re-traced in this pass.
>
> **Tessera Decision (`render.md` §4.3), for (b) and (c):** (b) the box around the eight
> corners of the model-space box after the object's transform (bind pose for skinned meshes;
> an instance batch is one object enclosing its instances; `render.md` §4.3 now selects per
> instance); (c) directional lights reach every object and follow the point lights. Part (c) of
> this Decision now **departs from the client** (L39: the client tests a directional light by
> distance and orders it by step 4); implementers align it, giving a directional light its
> world position and ρ. A light for which the caller supplies no selection data
> uses ρ = *R* and no switches (`render.md` §4.3); that default is Tessera's.

<a id="s8-6"></a>
### 8.6 The client's light-map bake (BIN-L (o), (p); L35, L36)

Besides the light lists of §8.5, the client has a second path for lights with
`Light_BakedLandscapeLight` set: it renders them into the baked-lighting texture of terrain
and ceiling chunks (the texture bound as material parameter 3019, `terrain-texturing.md` §10).
The steps:

1. **Trigger.** The landscape renderer keeps a "re-bake" flag. In the landscape update, when
   the multi-pass-lighting graphics option is on, the landscape-lighting-quality graphics
   option is above its lowest value (0) and the flag is set, the client runs the bake once and
   clears the flag.
2. **Device condition.** The bake does nothing unless the graphics device reports a capability
   flag and a device class above 2 (engine values of the back end).
3. **Lights.** Every placed light of the scene with `Light_BakedLandscapeLight` set.
4. **Chunks.** Every terrain and ceiling chunk of the scene (the landscape-chunk object kind
   of §8.5) that holds a baked-lighting texture. A light reaches a chunk by the box test of
   §8.5 step 2 (squared distance from the light to the chunk's world box below ρ²); the
   drawn-this-frame, scene-flag, parent and key-light rules of §8.5 are **not** applied here.
5. **Per chunk.** The client first resets the chunk's baked-lighting texture (cleared, or
   reloaded from the stored light map). Then, for each light that reaches the chunk, it draws
   the chunk lit by that light with **pass code 0x41** into an off-screen colour target of
   min(1,024, *w*) × min(1,024, *h*) texels, with *w* and *h* two size values of the graphics device (Inferred: the back-buffer size), and composites that target
   into the chunk's texture: the first composite clears the texture to (0, 0, 0, 1) and then
   adds the target; every later one adds the target (source factor one, destination factor
   one; the composite's blend codes read as the Direct3D factors of `material.md` §7.6). So after the bake the texture holds the **sum of the reaching lights' pass-0x41
   images**, and the stored light map is replaced, not added to.

**When this happens (L36).** The re-bake flag is raised after landscape changes (blocks
loaded, a new landscape position) only while a second renderer switch is on. The only routine
found that turns that switch on sets it from a comparison of a client-wide mode value with
one particular constant; the only assignment of that mode value found in this build stores a
different value, and the other routines that test it treat the matching mode as special (for
example they keep landscape resources that normal play releases). In addition, the
per-layer draw items with pass code 0x41 that the bake draws are created only in that same
mode (`terrain-texturing.md` §8.1). **Inferred:** the bake belongs to a light-map baking mode
of the client's tools (the stored light maps are its output) and does not run in normal play;
in normal play the stored light maps are the only source of these lights on the ground.

> **OPEN:** What sets the client-wide mode value to the matching constant (if anything does in
> the shipped client), and what the pass-0x41 programs of the terrain records compute
> (attenuation, mapping of the chunk into the light-map texture). **Tessera Decision:** no
> bake; terrain and ceiling chunks use the stored light map as it is (`terrain-texturing.md`
> §10).

---

<a id="s9"></a>
## 9. Sun shadows (BIN)

1. The sun's depth is rendered into a shadow map in pass 0x0A (cut-outs alpha-tested).
2. In passes 0x01, 0x04 and 0x3F the vertex program transforms the world position by the
   light's clip matrix and maps x, y to [0, 1] texture space (x′ = 0.5 x + 0.5,
   y′ = 0.5 − 0.5 y); z is the comparison depth.
3. The pixel program samples a **split map** at that position; its texel gives a scale
   (z, w) and offset (x, y) that select the cascade: shadow-map coordinates = position ×
   (z, w) + (x, y).
4. Visibility *s* ∈ [0, 1] is a hardware depth comparison: one tap when the quality input is
   1, otherwise five taps (centre weight 0.36; one texel left, right, up and down, weight
   0.16 each).
5. Shadow colour = landscape shadow colour + shadow colour range × *s* (per channel).
6. Fade *φ* = clamp(quality, 0, 1) × clamp(1 − (view depth − 0.1 − fade start) × fade scale,
   0, 1), from the fade-parameter input. When *φ* = 0 no sample is taken and the factor is 1.
7. Shadow factor *S* = 1 + *φ* × (shadow colour − 1), per channel. Terrain layers use the
   baked-shadow texel in place of 1 (`terrain-texturing.md` §9.5).

The values of the landscape shadow colour and colour range, the cascade layout (number of
cascades, split distances, map size) and the fade parameters are engine values uploaded per
frame (E13); they are not in the data. The client registers these inputs (BIN-L (k)), but
where it fills them was not found.

> **OPEN:** The engine's values for the landscape shadow colour and colour range (step 5), the
> cascade count, split distances and shadow-map size, and the fade start and scale (step 6),
> and how a graphics setting changes them. **Tessera Decision (simple form):** a single
> cascade, *s* from one tap, landscape shadow colour 0 and colour range 1 (so shadow colour =
> *s*), and *φ* fading linearly to 0 at the shadow distance. Variants that stay within this
> Decision: the five-tap filter of step 4 instead of one tap; a fade that starts later than
> the camera (for example over the last fifth of the shadow distance); a single map fitted
> around a point ahead of the camera; a depth bias; points outside the map count as lit; no
> sun shadow while the sun is low (a minimum elevation) or indoors (§8.1, no main light). The
> radius, distance, map size, bias and elevation are Tessera's values (`render.md` §4.4,
> `walker.md`), not the original's.

> **OPEN:** Point-light cube shadows (code 0x02/0x05/0x40 with feature bit 15) and the
> `LightShadowCubeIndex1/2` textures; the split-target output input of the shadow passes.

---

<a id="s10"></a>
## 10. Distance fog (BIN)

Per vertex: *f* = clamp((|eye − *P*| − near) / range, 0, 1), with range = far − near from the
keyframe's fog near and far (`environment.md` §5.3), interpolated across the triangle. The
base pass blends toward the fog colour by *f*; additive passes are multiplied by (1 − *f*).
Indoors, cells may override fog with `InteriorFogNear/Far/Color` and `InteriorFogType`
(`environment.md` §7.2; "Linear" or "None").

Limits (BIN-L (b), L26): the client caps fog far at 10,000 before applying overrides, then
raises it to at least 20, and keeps near between −10,000 and far − 20. When fog is off the
client uploads large fixed distances (near 10,000 and far 20,000 in the main scene path), so
*f* stays 0 within the drawn range.

Fog near values can be negative in the data (−100 in 894 keyframes): fog then starts before
the eye.

---

<a id="s11"></a>
## 11. Second render target (BIN)

Lit and many unlit programs write a second output: the view-space depth minus 0.1 (the clip
w of the vertex, interpolated). It feeds depth-based post-processing (soft particles,
ambient occlusion). Tessera may ignore it.

---

<a id="s12"></a>
## 12. Inputs per permutation (summary)

For a lit shader record, the passes and what each needs from Tessera:

| Pass | Needs |
|---|---|
| Base, all per vertex (0x3E) | *C*, dye, *N*, *A₀*, *H₀*, up vector, X lights, fog |
| Base, sun per pixel (0x3F) | as 0x3E + sun direction and colour, shadow map, specular inputs if specular |
| Base, point per pixel (0x40) | as 0x3E + one point light, optional cube shadow |
| Base without lists (0x04, 0x05) | *A₀*, *H₀*, one directional (0x04) or point (0x05) light, fog |
| Additive (0x01, 0x02, 0x03) | one light, fog factor |
| Depth (0x09, 0x0A) | transforms, opacity and *τ* |
| Glow (0x08) | glow mask, *G* |
| Normals (0x42) | normal map, *τ* |

Unlit records: `material.md` §11.3 blend, texture × *M* (× vertex colour when the record
reads it), fog when their programs read the fog inputs.

---

<a id="s13"></a>
## 13. Invariants

| # | Invariant | Status |
|---|---|---|
| LT1 | Every permutation with `program_kind` 4 starts both blobs with the DXBC container magic; kind 1 blobs start with version tokens 0xFFFE0200 / 0xFFFF0200; kind 3 blobs start with u32 2 | Confirmed, all |
| LT2 | Pass codes are in {0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x07, 0x08, 0x09, 0x0A, 0x3E, 0x3F, 0x40, 0x41, 0x42} | Confirmed, all 31,519 permutations |
| LT3 | `depth_write` = 0 in every render-state block of codes 0x01, 0x02, 0x03 whose factors are SRCALPHA / ONE | Confirmed |
| LT4 | The point-light arrays hold 8 entries (128 bytes of 16-byte vectors) wherever present | Confirmed |
| LT5 | No program outside the 116 colour-reading records declares a colour vertex input | Confirmed |

---

<a id="s14"></a>
## 14. Worked examples (synthetic values)

**Sun-lit texel.** Albedo *C* = (0.5, 0.4, 0.3); no dye; *N* = up; *U* = up; *A₀* =
(0.10, 0.10, 0.12); *H₀* = (0.05, 0.05, 0.05); sun toward-vector at pitch 30°, so *N* · *L₀* =
0.5; *E₀* = (0.5, 0.5, 0.45); no shadow (*S* = 1); no lists; no specular; fog *f* = 0.25 toward
(0.6, 0.6, 0.7).
*A* = sat((0.10, 0.10, 0.12) + 1 × (0.05, 0.05, 0.05)) = (0.15, 0.15, 0.17).
*D₀* = 0.5 × (0.5, 0.5, 0.45) = (0.25, 0.25, 0.225). *Σ* = (0.40, 0.40, 0.395).
2 × *C* × *Σ* = (0.40, 0.32, 0.237). Fogged: 0.75 × that + 0.25 × fog = (0.45, 0.39, 0.353).

**Point light.** *R²* = 100, *k* = 0.04, *d²* = 80: step 2 gives 1 / (1 + 3.2) = 0.238;
80 > 75, so *s* = (100 − 80) / 25 = 0.8, smoothstep = 0.896; *a* = 0.213.

**Light record.** Range 10, Falloff 0.5, Diffuse (1.0, 0.6, 0.2), DiffuseIntensity constant
1.0, *σ* = 1: *R²* = 100, *k* = 10 × 0.5 / 100 = 0.05, colour (1.0, 0.6, 0.2). At *d²* = 50:
*a* = 1 / (1 + 2.5) = 0.286 (no fade, 50 < 75). At *d²* = 80: 1 / (1 + 4) = 0.2, times the
smoothstep 0.896 = 0.179.

**Ambient.** *ba* = 0.2, *Ca1* = (1.0, 0.9, 0.8), *Ca2* = (0.4, 0.4, 0.5): *X* = (0.2, 0.18,
0.16), *Y* = (0.08, 0.08, 0.1); frame ambient (0.14, 0.13, 0.13), frame hemispheric (0.06,
0.05, 0.03). With *M* = white an up-facing surface gets *A* = (0.2, 0.18, 0.16) and a
down-facing one (0.08, 0.08, 0.1).

**Cross-fade.** *T* = 8 s, *t* = 2 s: *f* = ½ + ½ sin(π × (0.25 − 0.5)) = ½ − ½ × 0.7071 =
0.146.

**Alpha test.** *τ* = 0.8; texel alpha 0.8 is discarded, 0.81 is kept and output with alpha
0.81 × *M*.alpha.

---

<a id="s15"></a>
## 15. Open questions

Answered 2026-10-08 (seventh revision, BIN-L (q), (r)): how the client forms the world box
of the light-list walk (§8.5 step 2, L38; implementer OPEN part (b), except skinned and
instanced objects); where a directional light stands in an object's list (§8.5, L39; part
(c)); what the "immediate" switch between the outdoor and indoor records means (§8.1, L40, by
reference to `environment.md` E37).

Answered 2026-10-07 (sixth revision, BIN-L (n)–(p)): the candidates of the light-list walk
and the parent rule for lights without a parent (§8.5, L34; the implementer OPEN part (a));
the client's second path for baked-landscape lights, its light-map bake (§8.6, L35, L36);
the indoor lighting record (§8.1, L37, by reference to `landblock.md` §4.13).

Answered 2026-10-07 (fourth BIN-L pass): baked-landscape lights on terrain (§8.3, L30); how an
object's light list is built apart from the scene order (§8.5, L31, L32); the mapping of light
fields to properties (§8.3, L33).

Answered 2026-10-05 (second M4 pass): sRGB (no back end uses sRGB formats, §5.1); the ambient
and hemispheric mapping (§8.1); the light-property mapping (§8.3); the order and slots of
lights (§8.5, except how the object's list is built); texture-matrix composition
(`material.md` §8.4); specular power 0 (§5.8); the terrain blend maximum (§4.2, E17); the time
input (§4.2, E20). Answered 2026-10-05 (third pass): which UV set each texture reads, including
the baked-lighting textures, and what coordinate inputs 3 and 4 are (§4.3, L27).

> **OPEN:** The depth comparison of the additive passes and of stacked terrain layers (§6,
> L28). Tessera Decision there.

> **OPEN:** The engine values of the sun-shadow inputs: landscape shadow colour and colour
> range, cascade layout, fade parameters (§9). Tessera Decision there.

> **OPEN:** UV sets 1 and 2 of 0x2B0007A9 and mesh UV sets 3 … 6 (§4.3).

> **OPEN:** How the sun colour and brightness combine into the main light, and its specular
> colour (§8.1). Tessera Decision there.

> **OPEN:** The order in which the scene's lights are walked when an object's light list is
> built (§8.5); the run-time light scale *σ* (§8.3). Tessera Decisions there.

> **OPEN:** Projector lights (§8.4); point-light shadows (§9).

> **OPEN:** What sets the client mode that enables the light-map bake, and what the
> terrain records' pass-0x41 programs compute (§8.6). Tessera Decision there (no bake).

> **OPEN:** The meaning of the three scene flags that exclude an object from the light-list
> walk (§8.5 step 2); which local box a skinned or instanced object supplies to the walk; which
> directional light the submission keeps when an object's list holds more than one, and
> whether a key directional light takes the light-0 slot of a pass without a main light (§8.5,
> implementer OPEN (b), (c) remainder). Tessera Decisions there.

> **OPEN (for implementers):** align the walker's terrain light selection and the renderer's
> handling of unattached lights with §8.3 and §8.5 (callouts there).

> **OPEN:** How a record combines its two colour layers (multiply or add, §5.2 item 2): the
> combine is coded per record in its programs and no data field or role name selects it;
> Tessera Decision there.

> **OPEN:** Glow composition (bloom); the other effect inputs of §4.2 E21 (metallic-specular
> amount, opacity-fog properties, fake ambient terms); cube environment maps.

> **OPEN:** The channel order of property colour values (`property-system.md` §4.7); Tessera
> Decision B, G, R, A (§4.4).

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version (Draft): passes, permutation keys, inputs, lit base pass, additive passes, light sources, sun shadow, fog, invariants, examples. |
| 2026-10-05 | Revision (spec writer, second M4 pass, BIN-L). §1: tag BIN-L, rows L16–L26; Tessera Decision mark. §2.2: back-end libraries. §4.1, §4.2: how the ambient inputs, fake lights, the terrain blend maximum (parameter 1203) and the time input (E20) are filled. New §4.4: colour encodings (single statement; record colours B, G, R, A by BIN; property colours Inferred). §5.1: no sRGB in any back end (OPEN closed). §5.2: transform composition reference; two-layer combine labelled Tessera Decision; terrain normal frame reference. §5.8: black specular below power 0.5. §8.1 rewritten: cross-fade, ambient formation (former working hypothesis now BIN), sun colour labelled Tessera Decision. §8.2: per-vertex directional slot. §8.3: light fields, defaults, record rules (colour, specular, range, *k* = 10 × falloff / *R²*), census note; former working hypothesis replaced. New §8.5: light submission order and slots. §9 step 7: terrain baked shadow. §10: fog limits. §14: light-record, ambient and cross-fade examples. §15 updated; this change log added. |
| 2026-10-05 | Third revision after review (spec writer; M1, BIN-S third pass, BIN-L (j)–(k)). Status Draft → Review. §1: BIN-S and BIN-L log the third pass; rows L27–L29; L4 and L25 no longer quote program names. §4.2: the engine-supplied inputs are described without the programs' reflection names (no interoperability need; CLEAN_ROOM §5.1), numbered E1–E21; only material-parameter labels from the enum mapper remain. §4.3 rewritten: the claim about 0x2B0000A8 limited to its `Color`/`Color2` permutation (its main water permutations read only the position); new table of the UV set each texture role reads (second colour layer from set 1 in three records; baked light mask from set 1; inputs 3 and 4 only in terrain records, so unused on meshes); OPEN for 0x2B0007A9 and mesh sets 3 … 6. §5.2 item 1: points to §4.3. §6: depth comparison stated as BIN (not in the record; set by the engine) with an OPEN callout and a Tessera Decision. §8.4, §9: program names replaced by descriptions. §9: OPEN callout for the engine's shadow values, Tessera Decision made explicit. §15: answered and new OPEN items. |
| 2026-10-06 | Fourth revision after the re-review of 2026-10-05 (spec writer; M1 only). §4.1, last row: 2013 is the one `c_` id with a binding (semantic 4, in 7 of its 11 records, naming a permutation without programs; aligned with `material.md` §7.3, §8.6); it still reaches the programs by name. §16: the second-revision row no longer quotes a program constant name; it names the time input E20 and the terrain blend maximum (parameter 1203) instead. |
| 2026-10-07 | Fifth revision (spec writer; M4 under decision 0011, BIN-L fourth pass with the Ghidra decompiler). Status Accepted → Revised. §1: BIN-L questions (l), (m); rows L30–L33; L19 cites (m). §8.3: light fields mapped to properties by the ids the client registers (replaces the Inferred mapping by name); the `Light_BakedLandscapeLight` OPEN answered: such lights never reach terrain or ceiling chunks (the former Tessera Decision limited this to chunks with a light map). §8.5: the object-light-list OPEN answered (selection radius ρ with the constant 0.85, box distance test, exclusions, key lights first); a narrower OPEN on the scene's light order with a Tessera Decision. §15 updated. Implementers: `walker.md` §14.3 (terrain light table) and `render.md` §4.3 (automatic selection) differ from these rules. |
| 2026-10-07 | Sixth revision (spec writer; M4 under decision 0011, BIN-L sixth pass in an own copy of the Ghidra project; incident 0018 follow-up). Status Revised → Review. Header: the long review records moved to the dirty-room review log; a short record remains. §1: BIN-L questions (n)–(p); row L30 narrowed (light lists BIN, "normal play" Inferred); new rows L34–L37. §3.1: the bone-matrix share stated (review census). §3.2: pass 0x41 reading names the light-map bake. §5.6: Tessera Decision for per-pixel evaluation of the listed lights (what `tessera-render` does). §8.1: the indoor record (by reference to `landblock.md` §4.13) replaces the Inferred indoor sentence. §8.3: field-table row and the baked-landscape paragraph aligned with §8.6; implementer OPEN on the walker's terrain light selection. §8.5: step 2 states the candidates (drawn this frame, no scene flags); step 3 the parent rule for lights without a parent (BIN; withdraws the render.md Decision (a)); implementer OPEN of 2026-10-07 part (a) answered, (b) and (c) kept with their Decisions. New §8.6: the client's light-map bake (trigger, inputs, composite; Inferred not in normal play; OPEN and Tessera Decision "no bake"). §9: the shadow Tessera Decision covers the variants the renderer and walker use. §15 updated. Change-log rows put in date order. Implementers: `render.md` §4.3 Decision (a) and the walker's terrain light selection differ from §8.3, §8.5. |
| 2026-10-08 | Seventh revision (spec writer; M4 under decision 0011, BIN-L seventh pass in an own copy of the Ghidra project, questions (q), (r)). Status Accepted → Revised, waiting for review. No section number or anchor changed. §8.1: the "immediate" switch between the outdoor and indoor records is explained (no transition of its own; the 5 s sun-change fade of `environment.md` §11.6 smooths it exactly when it changes "sun present"; immediate updates cut), row L40; the cross-fade paragraph corrected to follow `environment.md` §11.6 and E27 (a sky-key change starts it, not a new day description; raw timer; direction blended as a vector); the **Sun** paragraph points to the outdoor no-main-light rule (review note 3 of the sixth revision). §5.9: pointer to the glow Decision of `render.md` §4.1 (review note 4). §8.5 step 2 and the implementer OPEN of 2026-10-07: (b) answered for rigid objects (L38: eight corners of one local box under the full transform), skinned and instanced boxes stay open; (c) answered (L39: the walk does not test the light's kind; a directional light is tested by distance and placed by step 4), so Decision (c) of `render.md` §4.3 now departs from the client; the remainder (more than one directional light; key directional light without a main light) stays open. §1: BIN-L questions (q), (r); rows L38–L40. §15 updated. Not done in this revision: review notes 1 and 2 of the sixth revision (§8.6 step 5 reset state; moving the §8.6 evidence narrative). |
