<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Terrain texturing: terrain type tables, surface layers, alpha masks and layer blending

| | |
|---|---|
| **Status** | Accepted (2026-10-07, sixth revision, see §17) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Sixth revision: Claude review session (spec-writer agent), 2026-10-07, **passed** (Accepted). Earlier rounds and detailed findings: dirty-room review log (incident 0018). |
| **Implements into** | `crates/formats` (terrain type table decoder, prefix 0x19), `crates/terrain` (layer resolution, per-vertex texture-coordinate sets and tangent frame, splat inputs), `crates/render` (layer pipeline: feature selection, draw order, opacity rule, blend state, lighting inputs), `apps/explorer` (terrain type table view), `fuzz/` (one target for the 0x19 decoder) |
| **Companion specs** | [`landblock.md`](landblock.md) §3.4 (height conversion), §3.5 (ceiling grid), §3.7 (per-vertex scenery word), §3.8 (surface chunks and layers), §6.4 (light maps). [`environment.md`](environment.md) §3 (region descriptor, slot 8), §5.6 (colour encoding). [`material.md`](material.md) §5.3 (float curve, reused here), §8.0 (parameter labels), §8.4 (texture transforms; same rule as §6.3 here). [`lighting.md`](lighting.md) §4 (engine inputs), §5 (lighting equation), §8 (light sources), §9 (sun shadow), §15 (colour space). [`water.md`](water.md) (everything about water). [`texture-list.md`](texture-list.md) (0x40 texture lists). [`records.md`](records.md) §1 (primitives, VLE), §5 (render surfaces, format 28 = A8), §8 (DID maps). [`property-system.md`](property-system.md) §5 (0x78 property sets), §8 (DID maps). [`scenery.md`](scenery.md) §4.4 (the same 0x19 table; its procedural flag gates scenery). [`collision.md`](collision.md) §6 (terrain collision). [`terrain.md`](terrain.md) (Tessera's meshing; §6, §7 and §9 there point here). |

---

<a id="s0"></a>
## 0. Purpose

Every landblock's terrain surface is drawn as a stack of **layers** (`landblock.md` §3.8).
This spec gives an implementer everything needed to draw those layers the way the original
client does:

- what a layer's `material` number means: a key into the region's **terrain type table**
  (prefix 0x19), decoded here to the last byte (§3);
- the meaning of layer order, of `whole`, of unmasked layers and of the quad grid (§4);
- the alpha masks: format, size, orientation, sampling (§5);
- how the client generates texture coordinates for a layer: three projection planes, a
  per-type scale, rotation and offset, and the tangent frame for normal maps (§6);
- the per-pixel opacity rule and the blend state, i.e. the splatting algorithm (§7, §8);
- which shading features the client enables for a layer and which values it feeds them
  (§8.1, §8.2);
- how colour, normal maps, specular, the baked light map and the baked shadow enter the
  pixel (§9, §10);
- what is known about roads and distance behaviour (§11, §12); water is in `water.md`;
- the height conversion (§13, now adopted by `landblock.md` §3.4);
- what the client draws for a chunk without layers (§4.9) and how it computes the vertex
  normals of the terrain and ceiling grids (§4.10).

It settles the texture-splatting OPEN of `terrain.md` §9 and the OPENs (1)–(4) of
`landblock.md` §3.8.2.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install (Steam) on 2026-10-05 by throwaway probes in the dirty-room scratchpad. All 5 records of prefix 0x19 decoded to the last byte (4,688 entries). All 390,150 landblock-data records of regions 1–5 and 14 read (86,726 terrain and 2,355 ceiling chunks with layers, 925,884 layers). The headers of all 646,073 distinct masks named by a drawn layer read. The 1,279 texture lists named by the tables decoded. A 1-in-40 sample (9,750 blocks) decoded in full (indices, vertex flags, masks); 630 minimap tiles compared with the composited layers (§4.1); light maps of adjacent blocks compared in a seam test (4,323 pairs, §10). (Corrected 2026-10-05: an earlier version of this row said "5,449 light maps"; that figure counted every 128 × 128 image in the sampled dump, not light maps. The install has 5,441 light-map properties, OBS3.) |
| OBS3 | Observed in the same install on 2026-10-05 (third pass): all 60,450 landblock-property records (prefix 0x804, `landblock.md` §6) of regions 1–5 and 14 scanned for the two light-map properties: 4,692 `Landblock_LightMap` and 749 `Landblock_CeilingLightMap` values (5,441 in all, 5,422 distinct surfaces; 19 surfaces are named by two blocks); the header of every one of the 5,422 surfaces read. The default of parameters 3000 and 3020 in all 192 terrain shader records read. |
| OBS5 | Observed in the same install on 2026-10-06 (M4.5 follow-up) by throwaway probes: all 390,150 landblock-data records read again; for every terrain and ceiling chunk without layers, whether the block has a ceiling and whether the stored terrain height is below the stored ceiling height at any sample (*i*, *j*) with 0 ≤ *i*, *j* ≤ 31; the smallest `draw_order` of all five tables; the diffuse-texture alpha of the 1,073 entries whose specular switches take the gloss from the diffuse alpha (227 distinct textures, one mip level of at most 32 × 32 texels each); the light maps of the `m4-5-visual.md` rows L2, L7, T15 and T16. |
| OBS6 | Observed in the same install on 2026-10-07 by a throwaway probe (sixth revision): all 390,150 landblock-data records of regions 1–5 and 14; for every interior grid sample (1 ≤ *i*, *j* ≤ 31) the number of grid triangles that have the sample as a corner, enumerated from the quad split flags (terrain: bit 7 and the triangulation of `landblock.md` §3.6; ceiling: bit 0 with the mirrored rule of §4.10); for every pair of blocks adjacent east–west or north–south, bit 7 and bit 0 of the vertex flags at index 32 compared with the neighbour's index 0. |
| OBS4 | Observed in the same install on 2026-10-06 by a throwaway probe: all 390,150 landblock-data records of regions 1–5 and 14 read again; every terrain-chunk and ceiling-chunk layer whose key names an entry without a diffuse texture in its region's table counted, split into drawn and skipped (§8 step 2) layers. |
| COL | [`collision.md`](collision.md) §6.5 and rows C22 / CO11 there: census of terrain triangles covered by no layer over all 390,150 landblock-data records, compared with the ceiling grid, and the client's terrain collision mesh (BIN). |
| OBS2 | Observed in the same install on 2026-10-05 (second pass): census of the switch combinations of all 4,688 entries (§3.6); the parameter declarations of all 192 terrain shader records of §8.1 (defaults of the transform, colour, threshold and texture parameters); the labels of DID map 0x28000002 (UDB below). |
| UDB | The "UNIQUEDB" DID map (root key 0xF, 0x28000002 in this install, `property-system.md` §8.1), read in the install: its entries pair numeric keys with labels and 0x2B shader-record DIDs. 192 labels name the terrain layer shader records; the label grammar is in §8.1. Labels are data and are quoted for interoperability. |
| SH | The OpenGL back end's shader text stored in the terrain shader records (prefix 0x2B) of the same install, read in the dirty room (method M1, data files) and reduced to the formulas of §7 and §9. No shader text is reproduced. The opacity formula was checked in the programs of passes 0x3E, 0x3F and 0x40 of all 192 records. |
| BIN-S | **M4 (decision 0011).** The Direct3D 10/11 programs (kind 4, `lighting.md` §2.2) of the terrain shader records disassembled with a public shader disassembler: records 0x2B0006F4, 0x2B0009F8, 0x2B00073F and 0x2B000A5E for every pass code, and the constant and sampler names of all 192 records. **Question:** what the newer back end computes for a terrain layer (opacity, lighting, normal map, specular, baked light, baked shadow, emissive). **Why M1–M3 were not enough:** the kind-4 programs are bytecode only; the OpenGL text describes an older variant that differs (§9.6); no frame capture is available (M3). Second pass 2026-10-07 (fifth revision, same build and method): records 0x2B0007E3 and 0x2B0007B0 (pass 0x3F, layers without a normal map) and 0x2B00076A and 0x2B00079D (pass 0x3F, permutations with feature bit 4). **Question:** how the programs use the stored vertex normal (normalised or not, per term) and how the specular term is formed (§4.10 last paragraph, §9.4). Same reasons; not re-run in the sixth revision. |
| BIN | **M4 (decision 0011).** Client build: `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5), disassembled with objdump in the dirty room; small routines were run in a CPU emulator with synthetic inputs. Questions asked: (a) the field list and order of a terrain type entry; (b) the per-block layer draw loop (order, which layers are skipped, how `whole` layers are drawn, which layer is drawn opaque); (c) the blend and sampler state of a layer; (d) the per-vertex texture-coordinate sets of the terrain mesh; (e) how scale, rotation and offset compose into the texture matrix; (f) the height conversion constant met in the same code; (g) how the client chooses the shader record of a layer; (h) which material parameters the client sets for a layer and from which entry fields; (i) the client's own field labels of an entry and the serialiser of each field; (j) the colour serialiser; (k) the tangent frame written by the terrain vertex builder; (l) the source of the baked-shadow texture; (m) the texture stage setup (coordinate set, address and filter modes); third pass 2026-10-05: (n) how the client loads a layer's mask and what it does with a mask that is not A8; (o) what the layer setup does when the entry's diffuse texture is 0 or does not load; (p) where the per-block texture offset of the vertex builder comes from (§6.2); (q) what the per-chunk layer routine does when a layer's key is missing from the table or its setup fails; fifth pass 2026-10-06 (the same build, read with the Ghidra 12.1.3 decompiler in the dirty room; no network, login, launcher or anti-tamper code examined): (r) what the per-chunk routine draws for a chunk that has no layers, and which entry it uses; (s) how the per-vertex normals of the terrain and ceiling grids are computed; (t) whether placed lights reach the terrain and ceiling chunks (`lighting.md` L30); sixth pass 2026-10-07 (same build, own copy of the Ghidra project, decompiler and listings): (u) the client's light-map bake into the chunks' baked-lighting textures and when it runs (`lighting.md` §8.6); (v) the graphics options and device conditions that gate the normal-map switch and the extra full-grid draws of the per-chunk routine; (w) what those extra draws use. Why M1–M3 were not enough: draw-time state (blend modes, shader choice, texture-coordinate generation, transform order, tangent frames, parameter wiring) is not stored in the data; no published source describes it; a frame capture of the running client (M3) is not available to this session. |
| LC | LotroCompanion `lotro-tools` (`RegionLoader`, readable source; facts only): names the eight table slots of the region descriptor. |
| LB | [`landblock.md`](landblock.md) §3.8 (chunk and layer layout), §6.3–§6.4 (minimap tiles, light maps). |
| ENV | [`environment.md`](environment.md) §3–§4 (region descriptor, per-region tables), §5.6 (colour encoding). |
| MAT | [`material.md`](material.md) §5.3 (float curve grammar), §8.0 (parameter labels), §8.4 (texture-coordinate transforms). |
| LT | [`lighting.md`](lighting.md) §4–§9 (engine inputs, lighting equation, lights, sun shadow). |
| WAT | [`water.md`](water.md) (water tables, landblock water, drawing water). |
| ER | `research/notes/engine-client-re.md` §3 (landscape draw bands, far-landscape normal maps, near/far seam blending). |

Marks: **Confirmed** = checked on every record of the kind in the install on the date given;
**Confirmed (sample)** = checked on the stated sample; **Probable** = supported by a statistical
test, not an exact rule; **BIN** = rests on disassembly (method M4) only; **Inferred** = a
reading of BIN or SH facts that the data supports but does not prove; **Reported** = published
sources only. **Tessera Decision** marks a rule chosen by this project where the original's
behaviour is unknown; tests must not treat it as the original's behaviour.

| # | Fact | Sources | Mark |
|---|---|---|---|
| T1 | Region descriptor slot 8 names the region's terrain type table (prefix 0x19) (§3.1) | ENV; LC (slot names); OBS | Confirmed |
| T2 | 0x19 record = DID, u32 count, count × entry; entry = u32 key, label, 90-byte body with two float curves (§3.2, §3.3) | OBS (5 of 5 records, zero leftover bytes) | Confirmed. No published source. |
| T3 | Meaning of the 26 entry fields; the client's labels of the fields, registered in exactly the stored order (§3.3) | BIN (a), (i); OBS (value census); SH (constants named after some fields) | BIN for names and roles; layout Confirmed |
| T4 | Keys of a table are exactly 0 … count − 1; entries are stored in hash order, not key order (§3.4) | OBS (5 of 5) | Confirmed |
| T5 | A layer's `material` is a key of the region's terrain type table (§4.1) | OBS (925,884 layers, none out of range; regions 1 and 5 use their table's last key); OBS (minimap test, §4.1) | Confirmed |
| T6 | Within every chunk, layers are stored in non-decreasing order of the entry's draw-order value (§4.2) | OBS (86,726 + 2,355 chunks, no exception; 3 terrain chunks hold both entries of an equal-value pair) | Confirmed |
| T7 | Stored order is the draw order, bottom to top (§4.2, §8) | BIN (b); OBS (occlusion test, minimap test) | Confirmed |
| T8 | Layers with no triangles and `whole` = 0 are skipped; `whole` = 1 draws all 2,048 triangles of the block (§4.3) | BIN (b); OBS | Confirmed |
| T9 | The first drawn layer of a chunk gets no opacity source, no alpha test and an opaque output; later layers are blended "over" (§7, §8) | BIN (b), (c), (g), (h); BIN-S | BIN |
| T10 | Lower layers have no triangles where a later layer is fully opaque (export-time culling) (§4.4) | OBS (sample: 99.0 % of mask texels outside a layer's triangles lie under a later fully opaque layer, 0.07 % under an earlier one) | Confirmed (sample) |
| T11 | Grid triangles covered by no layer are not drawn. They occur in 2,368 landblocks (2,750,134 triangles), every one of which has a ceiling; none lies where the ceiling is above the terrain: 2,336,479 have the ceiling at or below the terrain (buried under the ceiling surface); 413,655 have all three ceiling samples 0 (what covers them, if anything, is OPEN, §4.5). The client keeps them in terrain collision (§4.5) | COL (all 390,150 records; collision.md C22, CO11); BIN (b) (not drawn); COL (collision mesh, BIN there) | Confirmed (census); collision BIN. Revised 2026-10-05: the former row called them cave mouths on a 49-block sample. |
| T12 | The quad grid holds the dominant visible terrain type of each 5 × 5 quad, at index 32·qy + qx (§4.6) | OBS (sample: 94–95 % agreement; 51–56 % for the transposed order) | Probable (meaning); Confirmed (order) |
| T13 | The 17 × 17 coarse grid is the per-vertex scenery word and plays no part in texturing (§4.7) | `landblock.md` §3.7, `scenery.md` §3; OBS | Confirmed |
| T14 | Every mask of a drawn layer exists, is format 28 (A8) and square 128, 256 or 512 texels (§5.1) | OBS (646,073 of 646,073) | Confirmed |
| T15 | Mask texel row 0 lies on the block's south edge, column 0 on its west edge (§5.2) | OBS (triangle-footprint test, 8 orientations); BIN (d) | Confirmed |
| T16 | The terrain mesh carries texture-coordinate sets (x, y), (x, height), (y, height) in world units, (x, y)/160 over the block, and an optional fifth set for the baked-shadow texture (§6.1) | BIN (d), (l); BIN-S (vertex input signatures) | BIN |
| T17 | The projection value 3/1/2 selects set (x, y)/(x, height)/(y, height) (§6.1) | BIN (c), (g), (m); BIN-S; UDB (record labels "XYProj", "XZProj", "YZProj"); OBS (paired side projections in every table) | BIN; consistent with data |
| T18 | Texture coordinates = early translation, then scale, then counter-clockwise rotation in degrees, then translation; the terrain layer sets the scale, rotation and translation only, the early translation stays 0 (§6.3) | BIN (e) (composition routine run in an emulator, error ≤ 1.2 × 10⁻⁶ over 20 random parameter sets), BIN (h); OBS2 (defaults of the 192 records) | BIN |
| T19 | Layer opacity by record family: mask and diffuse alpha combined with the squared-mask term only when the entry's opacity flag is set and the layer has a mask; otherwise the mask alone, the diffuse alpha alone, or 1; discard at or below the threshold (§7) | SH (192 records); BIN-S (192 records); BIN (g), (h) | Confirmed in the programs of all 192 records; selection BIN. Revised 2026-10-05 (the former rule applied the squared-mask term also without the opacity flag). |
| T20 | Blend state: source alpha / inverse source alpha (§8) | BIN (c); OBS (render-state blocks of the 192 records) | Confirmed |
| T21 | Light maps share the mask orientation (§10) | OBS (seam test on 4,323 adjacent block pairs, 8 orientations) | Confirmed |
| T22 | Light maps are added to the lighting sum (§10) | BIN-S; SH; BIN (h) (bound as parameter 3019); OBS (79 % of texels are 0) | BIN |
| T23 | The client converts a stored height h to world units as h × 2,000 / 65,535 (§13) | BIN (f) | BIN; adopted by `landblock.md` §3.4 (L4) |
| T24 | The three 4-byte colours of an entry are u32 values 0xAARRGGBB stored little-endian (bytes B, G, R, A), each channel / 255 (§3.3) | BIN (i), (j): the entry's colour fields use the same serialiser as the day-description colours of `environment.md` §5.6 | BIN |
| T25 | The label is a records.md Pascal string: VLE length, then the bytes (§3.3) | BIN (i) (the label serialiser writes and reads a records.md §1.2 VLE count) | BIN; every label in this install is shorter than 128 bytes |
| T26 | Shader-record choice per layer from the projection and seven switches (§8.1) | BIN (g); UDB (label grammar of 192 records) | BIN |
| T27 | Parameters the client sets for a layer, and the first-layer rules (§8.2) | BIN (h); OBS2 | BIN |
| T28 | Specular needs `use_specular` and `use_gloss_specular`; gloss comes from the normal map's alpha or from the diffuse alpha (§8.1, §9.4) | BIN (g), (h); BIN-S | BIN |
| T29 | Tangent frame per vertex (east axis projected onto the surface; second axis = normal × tangent) and the rotation of the mapped normal by the texture matrix (§6.4) | BIN (k); BIN-S; SH | BIN |
| T30 | The baked-shadow texture is a render target the client generates (512, 1,024 or 2,048 texels by a setting), not data (§9.5) | BIN (l) | BIN |
| T31 | Texture stage setup: coordinate set per texture, wrap or clamp, linear filtering (§5.2, §6.3, §10) | BIN (m) | BIN |
| T32 | Lighting of a layer pixel in the newer back end (§9) | BIN-S | BIN |
| T33 | Ceiling chunks use the ceiling height grid for positions and for the height component of sets 1 and 2, and a separate ceiling normal array (§4.8) | BIN (d) | BIN |
| T34 | The client loads a layer mask only if its format is A8; otherwise it logs an error and binds no mask, so the record's default for parameter 3020 stays (§5.1) | BIN (n); OBS3 (default 0x40000007 in all 96 overlay records) | BIN; never happens in this install (T14) |
| T35 | A layer whose entry has no diffuse texture (or whose texture does not load) is still drawn; parameter 3000 keeps the record default (§8) | BIN (o); OBS3 (default 0x40000007 in all 192 records) | BIN |
| T36 | The per-block texture offset is the block's position relative to the landscape renderer's reference landblock (160 units per block) plus an accumulated shift that the renderer resets when it reaches 2,048 units or when the reference jumps by 480 units or more (§6.2) | BIN (p) | BIN; the choice of the reference landblock not traced |
| T37 | Every light map is a DXT1 surface; 5,421 of the 5,422 distinct surfaces are 128 × 128 texels, one is 64 × 64 (§10) | OBS3 (5,422 of 5,422) | Confirmed |
| T38 | When a layer's key is not in the table, or its setup fails, the per-chunk routine stops and reports failure; the remaining layers of the chunk are not drawn by it (§8) | BIN (q) | BIN; what the caller does then was not traced |
| T39 | The 15 entries without a diffuse texture (all in table 0x19000000) are named by 182 layers in 29 landblocks of region 1: 96 in terrain chunks (60 drawn, 36 skipped) and 86 in ceiling chunks (43 drawn, 43 skipped) (§8) | OBS4 (all 390,150 landblock-data records) | Confirmed 2026-10-06 |
| T40 | A chunk with no layers is drawn with one opaque layer over its full grid, using the table entry with the smallest `draw_order`, when the block has no ceiling or its stored terrain height is below its stored ceiling height at one of the samples 0 ≤ *i*, *j* ≤ 31; otherwise the chunk draws nothing and is not added to the scene (§4.9) | BIN (r) | BIN |
| T41 | Chunks without layers: 303,424 terrain chunks (300,187 drawn by T40, 3,237 not) and 4,512 ceiling chunks (944 drawn, 3,568 not); the smallest `draw_order` of every table belongs to key 0 alone (§4.9) | OBS5 (all 390,150 records, all 5 tables) | Confirmed 2026-10-06 |
| T42 | Vertex normal = the plain average of the unit normals of the grid triangles around the vertex (not renormalised); neighbouring blocks supply the triangles across an edge; ceiling normals point down and read the quad splits mirrored east–west (§4.10) | BIN (s) | BIN |
| T43 | Lights with `Light_BakedLandscapeLight` set are never put in the light lists of terrain or ceiling chunks, whether or not the block has a light map; in normal play their light on the ground is the stored light map (§10) | BIN (t), (u); `lighting.md` L30, L36 | Light lists BIN; "normal play" Inferred |
| T44 | In 89 % of the 1,073 entries that take the gloss from the diffuse alpha, that alpha averages below 0.25 (median 0.049; per-texture maximum median 0.133) (§9.4) | OBS5 | Confirmed 2026-10-06 (one mip level per texture) |
| T45 | The per-chunk routine draws every layer over the full grid instead of its triangle range when a switch held by the landscape renderer is off; the renderer starts with it on (§4.3) | BIN (r) | BIN; what turns it off was not traced |
| T46 | A grid sample has 4 to 8 triangles around it (each of its four quads gives two when its diagonal runs through the sample); census of interior samples: terrain 4: 9,201, 5: 353,168, 6: 374,206,540, 7: 358,859, 8: 6,382; ceiling (6,867 blocks) 58 / 4,260 / 6,590,660 / 4,038 / 171 (§4.10) | OBS6 | Confirmed 2026-10-07 |
| T47 | At every shared edge of two adjacent blocks, bit 7 and bit 0 of the vertex flag at index 32 equal those of the neighbour's flag at index 0 (25,648,920 of 25,648,920 samples each) (§4.10) | OBS6 | Confirmed 2026-10-07 |
| T48 | The normal-map switch needs the multi-pass-lighting option on, the landscape-lighting-quality option at 2 or more and a device class above 2 (§8.1) | BIN (v) | BIN |
| T49 | The client's light-map bake draws baked-landscape lights into the chunks' baked-lighting textures with pass code 0x41 and replaces the texture's content; the per-layer pass-0x41 draw items exist only in a special client mode (§10; `lighting.md` §8.6) | BIN (u) | BIN; "not in normal play" Inferred |
| T50 | After its layers, the per-chunk routine can draw the full grid once more with the material of a 3 × 3-block sector object of the landscape renderer, with texture scale 1/480 (u) and −1/480 (v) and a translation from the block's place in the sector, when the multi-pass-lighting option is on, the landscape-lighting quality is above 0 and the device class is above 2 (§12) | BIN (v), (w) | BIN; what the sector object is Inferred |
| T51 | The programs' columns *c*₀, *c*₁ of §6.4 hold scale and rotation only; the translations sit in the fourth column and do not enter the normal rotation (§6.4) | `material.md` §8.4 (BIN-T); `lighting.md` §5.2.1 (BIN-S) | Inferred from those BIN facts |
| T52 | The §4.10 last paragraph (stored normal unnormalised in the hemispheric term and the sun's *N* · *L₀*, normalised in the per-vertex point-light sum) and the §9.4 specular form (not × *N* · *L*, albedo or 2) | BIN-S (second pass, records named in the tag) | BIN |

---

<a id="s2"></a>
## 2. The chain at a glance

```
landblock data 0x800RXXYY ─ terrain chunk ─ layer i: material k, mask DID, whole, triangle range
        │                                          │            │
        │ region R                                 │            └─► 0x41 A8 alpha mask (§5)
        ▼                                          ▼
region descriptor 0x0F (slot 8) ──► terrain type table 0x19 ── entry with key k (§3)
                                                       ├─ diffuse texture list 0x40 (§6, §9)
                                                       ├─ normal / emissive / glow lists 0x40
                                                       ├─ scale, rotation, offsets, projection (§6)
                                                       ├─ switches, threshold, blend maximum (§7, §8.1)
                                                       └─ physics material 0x78 property set
landblock properties 0x804RXXYY ── light map 0x41 (§10)
UNIQUEDB DID map ── terrain shader records 0x2B (§8.1; parameters §8.2)
```

A renderer resolves every layer of a block to one entry, decides the layer's features
(§8.1), draws the layers in stored order (§8), and computes each pixel with the opacity rule
of §7 and the colour terms of §9.

---

<a id="s3"></a>
## 3. Terrain type table (prefix 0x19)

<a id="s3-1"></a>
### 3.1 Finding the table (Confirmed)

The region descriptor of region *R* (`environment.md` §3.1) holds eight table DIDs; the
eighth (offset 110) is the region's terrain type table. In this install:

| Table | Regions | Size (bytes) | Entries |
|---|---|---|---|
| 0x19000000 | 1 | 130,889 | 1,095 |
| 0x19000001 | 2, 14 | 149,647 | 1,236 |
| 0x19000002 | 3 | 75,259 | 620 |
| 0x19000007 | 4, 5 | 173,939 | 1,421 |
| 0x1900000A | 6 (no cell archive in this install) | 40,637 | 316 |

These DIDs are observations. A reader takes the DID from the region descriptor at run time
and never hard-codes it. All records are in `client_general.dat`, uncompressed.

LC names the eight slots, in order: scene table (0x11), terrain description (0x12),
encounter table (0x14), sky table (0x15), water table (0x16), fog table (0x17), block-map
property descriptor (0x18), terrain type table (0x19). `water.md` §3 confirms slots 5 and 6
by BIN (0x16 is the water table, 0x17 the fog-zone table).

<a id="s3-2"></a>
### 3.2 Record layout (Confirmed, 5 of 5, zero leftover bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0 | 4 | DID | `did` | Own DID (0x19…) |
| 4 | 4 | u32 | `count` *n* | Number of entries |
| 8 | varies | entry × *n* | `entries` | §3.3, back to back, no padding |

<a id="s3-3"></a>
### 3.3 Entry layout (Confirmed layout, 4,688 of 4,688; field roles BIN)

Offsets are relative to the start of the entry. *V* is the size of the label's length prefix
(1 byte for every label in this install) and *L* the label length; *c1* and *c2* are the sizes
of the two float curves (8 bytes for curve mode 1, 44 for modes 2–9, `material.md` §5.3).
With *V* = 1 and both curves in mode 1 the body after the label is exactly 90 bytes.

The client keeps a field table for this entry type with one label per field, registered in
exactly the stored order below, and the record is that table serialised field by field (BIN
(i)). The same table gives each field a named reader and writer for the client's named-field
text form of the entry. The labels are quoted in the "Client label" column for
interoperability with that form and with `scenery.md` §4.4, which names the fields the same
way; the Tessera names are this spec's.

| Offset | Size | Type | Field (Tessera name) | Client label | Meaning | Units / values |
|---|---|---|---|---|---|---|
| 0 | 4 | u32 | `key` | (hash key) | The terrain type number that layers refer to (§4.1) | 0 … *n* − 1 |
| 4 | *V* + *L* | Pascal string | `label` | Name | Internal identifier of the type (an editor name). VLE length (`records.md` §1.2), then *L* bytes. Not displayed text; do not depend on its content. | *L* = 6 – 48 observed |
| 4+*V*+*L* | 4 | u32 | `draw_order` | DepthIndex | Draw-order value; layers of a chunk are stored in non-decreasing `draw_order` (§4.2) | 2 – 3,086; unique within a table except one pair in each of three tables |
| 8+*V*+*L* | 4 | DID | `diffuse` | DiffuseTexture | 0x40 texture list of the colour texture, or 0 (15 entries, all in table 0x19000000) | |
| 12+*V*+*L* | 4 | f32 | `u_scale` | TextureUScale | Texture repeats per world unit along the first projected axis (§6.3) | −0.5 – 1.0; most often 0.1, 0.2, 0.3, 0.05 |
| 16+*V*+*L* | 4 | f32 | `v_scale` | TextureVScale | Same for the second axis | equal to `u_scale` in 4,543 entries |
| 20+*V*+*L* | 4 | f32 | `rotation` | TextureRotation | Texture rotation (§6.3) | degrees, −90 – 321; non-zero in 530 entries |
| 24+*V*+*L* | *c1* | float curve | `u_offset` | TextureUOffset | Texture translation along u (§6.3); `material.md` §5.3 | mode 1 in all 4,688; non-zero constant in 18 |
| 24+*V*+*L*+*c1* | *c2* | float curve | `v_offset` | TextureVOffset | Same along v | mode 1 in 4,660; mode 2 (scrolling) in 28, all in table 0x1900000A |
| *b* = 24+*V*+*L*+*c1*+*c2* | 4 | u32 | `projection` | TextureProjectionAxis | Projection plane (§6.1): 1 = XZ, 2 = YZ, 3 = XY | 1: 519, 2: 520, 3: 3,649 |
| *b*+4 | 1 | bool8 | `use_opacity_mask` | UseOpacityMask | The diffuse texture's alpha is an opacity (§7, §8.1) | 1 in 1,069 |
| *b*+5 | 1 | bool8 | `use_specular` | UseSpecular | First of the two specular switches (§8.1) | 1 in 1,566 |
| *b*+6 | 1 | bool8 | `use_gloss_specular` | UseGlossSpecular | Second specular switch; specular needs both (§8.1) | 1 in 1,696 |
| *b*+7 | 1 | bool8 | `specular_from_normal` | UseSpecularFromNormalmap | Gloss is taken from the normal texture's alpha instead of the diffuse alpha (§8.1) | 1 in 364 |
| *b*+8 | 1 | bool8 | `slope_orientation` | UseSlopeOrientation | Not read by the layer setup (BIN (h)); no use found (§16) | 1 in 147 |
| *b*+9 | 4 | f32 | `alpha_test_threshold` | AlphaTestThreshold | Pixels whose opacity is at or below this value are discarded (§7) | 0 in 4,550; up to 1.0 |
| *b*+13 | 4 | f32 | `alpha_blend_max` | AlphaBlendMax | Weight of the squared mask in the opacity of masked layers with `use_opacity_mask` (§7); fed to material parameter 1203, labelled `c_AlphaBlendMax` (`material.md` §8.0) | 0 in 4,406; 0 – 5.0 |
| *b*+17 | 4 | u32 | `mask_resolution` | TerrainMaskResolution | Authoring resolution of this type's masks; not read by the layer setup | 128 (4,458), 256 (194), 512 (36). Read the size from the mask (§5.1). |
| *b*+21 | 4 | DID | `physics_material` | PhysicsMaterial | A 0x78 property set describing the ground for physics and footsteps, or 0 | 25 distinct, all resolve; 0 in 203 |
| *b*+25 | 1 | bool8 | `procedural` | AllowProceduralContent | Procedural scenery (grass and similar) may be placed on this type (`scenery.md` §4.4: 29 bytes before the end of the entry) | 1 in 4,080 |
| *b*+26 | 4 | colour | `diffuse_scale` | DiffuseColorScale | Multiplies the diffuse colour (§8.2, §9); its alpha byte is ignored (§8.2) | alpha byte 255 in all |
| *b*+30 | 4 | DID | `normal` | NormalTexture | 0x40 list of a tangent-space normal texture, or 0 | 0 in 834 |
| *b*+34 | 4 | DID | `emissive` | EmissiveTexture | 0x40 list, or 0 | non-zero in 105 (table 0x1900000A only) |
| *b*+38 | 4 | DID | `glow` | GlowTexture | 0x40 list, or 0 | non-zero in 106 (table 0x1900000A only) |
| *b*+42 | 4 | colour | `glow_colour` | GlowColor | Colour of the emissive and glow terms (§9.5) | |
| *b*+46 | 4 | colour | `specular_colour` | SpecularColorScale | Specular colour (§9.4) | |
| *b*+50 | 4 | f32 | `specular_power` | SpecularPower | Specular exponent (§9.4) | 0 – 2,556; 8.0 in most |
| *b*+54 | | | | | End of entry | |

**Colours (BIN, T24).** Each colour is a u32 whose value is 0xAARRGGBB, stored little-endian,
so the four bytes in the record are blue, green, red, alpha; each channel divided by 255 gives
a value in [0, 1]. This is the same encoding, read by the same client routine, as the colours
of `environment.md` §5.6; `lighting.md` §4.4 lists every colour encoding in one place.
Example: the bytes 0x20, 0x40, 0x80, 0xFF are red 0.502, green 0.251, blue 0.125, alpha 1.0.

**Booleans.** All five bool8 fields and `procedural` hold only 0 or 1 in the data; the client's
boolean reader rejects any other byte (BIN (i)).

Every non-zero texture DID (796 distinct diffuse and 482 distinct normal lists, 1,279 lists in
all) resolves to a 0x40 mip chain (`texture-list.md`).

Former Tessera names (before 2026-10-05): `opacity_from_diffuse` = `use_opacity_mask`,
`specular` = `use_specular`, `gloss_specular` = `use_gloss_specular`, `alpha_cutoff` =
`alpha_test_threshold`, `mask_boost` = `alpha_blend_max`. The names were aligned with the
client's labels because `material.md` refers to the same values by the material parameters
1043 (label `AlphaTestRef`) and 1203 (label `c_AlphaBlendMax`), §8.2.

<a id="s3-4"></a>
### 3.4 Keys and storage order (Confirmed)

- The keys of a table are exactly the integers 0 … *n* − 1, each once.
- Entries are **not** stored in key order (the first entries of 0x1900000A have keys 191, 0,
  192, 1, 193, 2, …: a hash-table order). A reader builds a key → entry index and never uses
  the stored position.
- Key 0 has the smallest `draw_order` of its table in all five tables.

<a id="s3-5"></a>
### 3.5 Invariants for a decoder

1. The record is consumed exactly: DID, count, *n* entries, nothing left.
2. Keys are unique and form the range 0 … *n* − 1.
3. Curve modes are 1–10 (`material.md` §5.3); any other mode makes the entry length unknown
   and the record is rejected.
4. bool8 fields are 0 or 1 (reject otherwise, as `records.md` §1.1 does).
5. `projection` is 1, 2 or 3.
6. The label length is a VLE (`records.md` §1.2) that fits in the remaining bytes.

<a id="s3-6"></a>
### 3.6 Census of the switches (Confirmed, OBS2, 4,688 entries)

| Combination | Entries | Effect (§8.1) |
|---|---|---|
| `use_specular` = 1 and `use_gloss_specular` = 1 | 1,390 | specular possible |
| … of which `specular_from_normal` = 1 | 312 | gloss from the normal texture |
| … of which `use_opacity_mask` = 1 and `specular_from_normal` = 0 | 2 | no specular (the diffuse alpha is taken by the opacity) |
| `use_specular` = 1 only | 176 | no specular |
| `use_gloss_specular` = 1 only | 306 | no specular |
| `specular_from_normal` = 1 without both specular switches | 52 | no effect |
| `alpha_blend_max` ≠ 0 | 282 | |
| … with `use_opacity_mask` = 0 | 58 | no effect (§7) |
| `alpha_test_threshold` > 0.0002 | 138 | alpha test on later layers (§8.2) |
| … with `use_opacity_mask` = 0 | 71 | the threshold applies to the mask alone (§7) |

---

<a id="s4"></a>
## 4. Surface layers: meaning (adds to and corrects `landblock.md` §3.8)

<a id="s4-1"></a>
### 4.1 `material` is a terrain type key (Confirmed)

A layer's `material` (u32 at layer offset 0) is the `key` of an entry of the terrain type table
of the block's region (§3.1). The same holds for ceiling chunks.

Evidence: none of 925,884 layers (terrain and ceiling, regions 1–5 and 14) is out of range;
region 1 uses key 1,094 and region 5 key 1,420, the last keys of their tables; the per-chunk
order rule of §4.2 holds only under this reading. A rendering test agrees: compositing the mean
colours of the layers' diffuse textures through their masks (§8) correlates with the
minimap tile of the same block (`landblock.md` §6.3) at 0.27 on average over 630 sampled
blocks, against 0.10–0.12 for the reversed order and about 0 for a wrong table index.

<a id="s4-2"></a>
### 4.2 Order (Confirmed)

- In every chunk the layers are stored in non-decreasing `draw_order` of their entries (86,726
  of 86,726 terrain chunks and 2,355 of 2,355 ceiling chunks). Equal values occur (review census
  of 2026-10-06): 3 terrain chunks (region 2) hold both entries of an equal-`draw_order` pair of their table (§3.3), in
  either stored order, so a test for strictly increasing values fails there; the stored order
  decides. No chunk names the same key twice.
- The stored order is the **draw order**, bottom layer first (BIN (b); §8). A renderer uses
  the stored order and does not re-sort.
- The first layer of every chunk with layers is key 0 (86,681 of 86,726 terrain chunks; all
  ceiling chunks), often with no triangles.

<a id="s4-3"></a>
### 4.3 Layer kinds (answers `landblock.md` §3.8.2 OPEN (4))

| `whole` | `prims` | `mask` | What the client draws (BIN (b), (g)) |
|---|---|---|---|
| 0 | 0 | any | Nothing. The layer is skipped and does not count as "first drawn" (§8). Its mask, if named, does not exist. |
| 0 | > 0 | 0 | The layer's triangles without a mask (§7: opacity from the diffuse alpha or 1) |
| 0 | > 0 | DID | The layer's triangles through the mask |
| 1 | 0 | 0 | **All 2,048 triangles of the block**, without a mask. In 9,035 of 9,170 such layers no earlier layer of the chunk has triangles. |
| 1 | 0 | DID | All 2,048 triangles through the mask |

For `whole` = 1 the client ignores `start`, `prims`, `min_vertex` and `vertex_span` (all 0 in
the data) and uses one shared index list covering the full grid: the level-0 triangulation of
`landblock.md` §3.6 (each quad split by bit 7 of its vertex flag). For a ceiling chunk the
same full-grid list follows the ceiling triangulation instead: each quad split by bit 0 of its
vertex flag, triangles wound to face down (BIN (r); `collision.md` §6 uses the same split).

**Full-grid switch (BIN (r), T45).** The table above holds while a switch kept by the
client's landscape renderer is on. With it off, the per-chunk routine draws **every** layer,
including those with `prims` = 0, over the full grid, and every layer after the first gets an
overlay record (§8.1) whether or not it names a mask. The renderer creates the switch on; two
renderer events clear it, and which events those are was not traced.

> **OPEN:** When the client's landscape renderer turns the full-grid switch off (T45). The
> stored triangle ranges and the export-time culling of §4.4 only make sense with it on.
> **Tessera Decision:** treat it as always on (the table above).

<a id="s4-4"></a>
### 4.4 Why lower layers have gaps (Confirmed on sample)

Masks are painted over large areas, but a layer's stored triangles cover only part of its
mask: 23 % of non-zero mask texels lie outside the layer's triangles. 99.0 % of those texels
lie under a **later** layer that is fully opaque there (mask 255 or no mask) and covers that
place with its own triangles; 0.07 % lie under an earlier one. So the exporter removed a
layer's triangles where later layers hide it completely. Consequences:

- Drawing only the stored triangles, bottom to top, reproduces the intended picture; a
  renderer must not "fill in" a layer over its whole mask.
- At every covered triangle the lowest layer that has the triangle is opaque there, so the
  bottom of the stack never shows the background.

<a id="s4-5"></a>
### 4.5 Holes (Confirmed census; collision BIN)

The original draws only layer triangles (§8, BIN (b)), so a grid triangle that no layer of the
terrain chunk covers is not drawn. Facts (COL, census of all 390,150 landblock-data records,
counted in the 86,726 landblocks whose terrain chunk has layers; a `whole` layer covers every
triangle):

- 2,368 landblocks have such triangles, 2,750,134 triangles in all. **Every** one of these
  landblocks has a ceiling (`landblock.md` §3.5); no landblock without a ceiling has any.
- No such triangle lies under an open cave. Comparing the mean of a triangle's three
  ceiling samples with the mean of its three terrain heights: 2,336,479 have the ceiling at
  or below the terrain (the terrain there is buried inside the rock between the cave floor and
  the ceiling surface), and 413,655 have all three ceiling samples 0 (no ceiling there).
- The client's terrain collision mesh contains every grid triangle, these included
  (`collision.md` §6.1, §6.5, BIN there).

So these triangles are not openings in the ground ("cave mouths", as an earlier version of
this section said): collision keeps them. For the 2,336,479 with the ceiling at or below the terrain, the terrain is hidden in
the rock under the ceiling surface. For the 413,655 whose three ceiling samples are all 0,
nothing in the data examined here shows what, if anything, covers them or whether a walker
can reach them (OPEN below; the same OPEN is kept in `collision.md` §6.5). Either way the
original does not draw them. A renderer that draws only
layer triangles reproduces the original without special handling; a renderer that draws the
full grid (for example `terrain.md` §6's fallback shading) shows terrain the original does
not draw.
Collision keeps all 2,048 triangles of every landblock (`collision.md` §6.5).

> **OPEN:** What, if anything, lies over the 413,655 undrawn terrain triangles whose three
> ceiling samples are all 0, and whether a walker can reach them. Not checked: cells
> (`collision.md` §5), static entities, or neighbouring geometry over those triangles. This is
> the OPEN of `collision.md` §6.5. Drawing does not depend on the answer (the original does not
> draw them, BIN (b)); only what the player sees there does.

> **Answered (spec writer, 2026-10-07, fifth revision):** what the original draws for a
> terrain chunk with no layers is now §4.9 (BIN (r), T40, T41): one opaque layer of the
> table's lowest-`draw_order` entry over the full grid, unless the block has a ceiling that is
> nowhere above its terrain. The 13 × 13 landblocks around `m4-5-visual.md` row F1 (region 1,
> 0x4C66) have no ceiling, so each is drawn with key 0 of 0x19000000. The height-shaded
> fallback of `walker.md` §14.4 is no longer needed for such chunks. The original question
> follows.
>
> **OPEN (clean-room implementer, 2026-10-06, walker review):** what the original draws for
> a **terrain chunk with no layers at all**. The census above counts only the 86,726
> landblocks whose terrain chunk has layers. A review probe found many landblock-data
> records whose terrain chunk has none while the heights are not constant: 17,020 in
> region 1 (703 of them with a landblock info), 11,359 in region 2 and 42,807 in region 5,
> among them the 13 × 13 landblocks around `m4-5-visual.md` row F1 (region 1, 0x4C66). Is
> such a chunk drawn at all, drawn by the distant-landscape path (§12), or in some other
> way? Tessera Decision meanwhile (`walker.md` §14.4): the walker draws it like a chunk
> that does not resolve (height-shaded), not as nothing.

<a id="s4-6"></a>
### 4.6 Quad grid (answers `landblock.md` §3.8.4 OPEN; Probable)

The 1,024 values hold, for each 5 × 5 quad, the terrain type key that is most visible in that
quad after compositing (§8). Index **32 · qy + qx**, qx along X (east), qy along Y (north),
the same order as the index list. Agreement with a recomputation from masks and triangles at
128 × 128 resolution: 93.6–94.9 % per region (transposed order: 51–56 %). Its use is not
rendering; the entry's `physics_material` suggests ground-type lookups (footsteps, physics).

> **OPEN:** The exact weighting the exporter used (mask area, centre sample, or opacity
> including the diffuse alpha), which would explain the remaining 5 %.

<a id="s4-7"></a>
### 4.7 Coarse grid (Confirmed)

The 17 × 17 grid of `landblock.md` §3.7 is the per-vertex **scenery word** (terrain-type id
for scenery, scene slot, encounter slot), specified in `landblock.md` §3.7 and `scenery.md`
§3. It does not select textures: texturing uses only the surface layers. (Measured here as a
cross-check: its low byte predicts the quad grid's terrain type for only 29 % of quads.)

<a id="s4-8"></a>
### 4.8 Ceiling chunks (BIN (d); data Confirmed)

A landblock with a ceiling has two terrain render objects, one for the terrain chunk and one
for the ceiling chunk, built by the same routine. When the vertex builder works on the
ceiling object it takes:

- the **ceiling height grid** (`landblock.md` §3.5) for the vertex heights, so the height
  component of coordinate sets 1 and 2 (§6.1) is the ceiling height;
- a **separate per-block array of ceiling normals** for the vertex normals (and the tangent
  frame of §6.4 is built from those normals).

Everything else (layers, masks, the light map named `Landblock_CeilingLightMap`, the opacity
and blend rules) works as for the terrain chunk. Ceiling triangles face downwards
(`landblock.md` §3.5).

The ceiling normal array is computed from the ceiling heights by the rule of §4.10 (BIN (s),
answered 2026-10-07): the normals point **down**, so a ceiling receives the downward ambient
*A₀* − *H₀* of §9 (`lighting.md` §5.3), plus its own light map and the placed lights that
reach it (§10). The former Tessera Decision (terrain rule, negated) gives the same direction;
§4.10 gives the exact triangles.

---

<a id="s4-9"></a>
### 4.9 Chunks without layers (BIN (r); census Confirmed)

A terrain or ceiling chunk whose layer count is 0 is not simply skipped. The client decides
per block, with one flag that serves both chunks of the block:

1. **The block's open flag.** It is set when the block has no ceiling. When the block has a
   ceiling, it is set only if, at some grid sample (*i*, *j*) with 0 ≤ *i* ≤ 31 and
   0 ≤ *j* ≤ 31 (the last row and column, 32, are not tested), the stored terrain height is
   strictly lower than the stored ceiling height at the same sample (`landblock.md` §3.4,
   §3.5; the comparison is the same on the raw u16 values or on the converted heights).
2. **Flag set:** the chunk is drawn as if it had exactly one layer:
   - the entry is the one with the **smallest `draw_order`** in the region's table (§3.1). The
     client finds it by scanning the whole table; if several entries shared the smallest value
     the one it meets last in its own storage order would win, but in every table of this
     install the smallest value belongs to one entry only, key 0 (T41);
   - the layer is the chunk's first drawn layer (§7, §8): no mask, no opacity input, no alpha
     test, output alpha 1;
   - its geometry is the chunk's full grid (§4.3: bit 7 split for the terrain chunk, bit 0 and
     downward winding for the ceiling chunk);
   - everything else is the normal layer setup: the shader-record switches of §8.1 (normal map,
     specular, the chunk's light map of §10 as baked light, emissive and glow), the parameters of
     §8.2, the texture coordinates of §6 and the lighting of §9.
3. **Flag clear:** the chunk draws nothing. A chunk with nothing to draw is not added to the
   client's scene at all, so it neither receives lights nor is drawn by any other path.

Census (OBS5, T41; all 390,150 landblock-data records of regions 1–5 and 14):

| Chunk | Without layers | Drawn by rule 2 | Not drawn (rule 3) |
|---|---|---|---|
| Terrain, block without a ceiling | 299,369 | 299,369 | 0 |
| Terrain, block with a ceiling | 4,055 | 818 | 3,237 |
| Ceiling | 4,512 (of 6,867) | 944 | 3,568 |

Of the 6,867 blocks with a ceiling, 3,075 have the open flag set and 3,792 do not. Many of the
terrain chunks drawn by rule 2 lie outside the playable areas (142,651 of them have every
height sample 0), but not all: the 13 × 13 blocks around `m4-5-visual.md` row F1 (region 1,
0x4C66) have varying heights, no ceiling and no layers, and are drawn with key 0 of
0x19000000. In every table key 0 is a top-down (`projection` 3) ground type.

These DIDs and counts are observations of this install; a reader takes the entry with the
smallest `draw_order` from the table at run time.

<a id="s4-10"></a>
### 4.10 Vertex normals of the terrain and ceiling grids (BIN (s))

The client computes one normal per grid sample (33 × 33 per chunk) once per block and stores
it with the block; the vertex builder copies it unchanged into the vertex (§4.8, §6.4).
Positions for this computation are (5 · *i*, 5 · *j*, *h*) with *i* the east index, *j* the
north index, 5 the grid spacing (a client constant) and *h* the height of §13 (the ceiling
height for a ceiling chunk).

**Terrain chunk.** The normal at sample (*i*, *j*) is the plain average of the unit normals of
the grid triangles that have that sample as a corner, taken from the four quads around it
(south-west, south-east, north-west and north-east of the sample). Each quad is split by bit 7
of its own vertex flag exactly as the terrain triangulation is (`landblock.md` §3.6): a quad
whose split runs through the sample contributes two triangles, otherwise one. The four split
flags are independent, so a sample has **four to eight** triangles: two each from the
south-west and north-east quads when their flag is set, and from the south-east and north-west
quads when their flag is clear. Census of the interior samples (1 ≤ *i*, *j* ≤ 31) of all
390,150 landblock-data records (OBS6, T46): 4 triangles at 9,201 samples, 5 at 353,168, 6 at
374,206,540, 7 at 358,859 and 8 at 6,382; ceiling samples of the 6,867 blocks with a ceiling
(mirrored rule below, which gives the same count from the bit-0 flags): 58, 4,260, 6,590,660,
4,038 and 171. Each triangle's normal is the normalised cross product of its edges
taken counter-clockwise seen from above, so it points up. The average is the sum divided by
the number of triangles; it is **not** normalised again, so its length is at most 1.

**Ceiling chunk.** The same procedure with three differences:

1. heights are the ceiling heights, and quads are split by bit 0 of the vertex flag (the
   ceiling split, `collision.md` §6);
2. east and west are exchanged in the triangle shapes, but not in the split flags: the quad
   south-east of the sample is split by the flag of the quad south-west of it, the quad
   south-west by the flag of the quad south-east, the quad north-east by the flag of the quad
   north-west and the quad north-west by the flag of the quad north-east; and a set flag then
   means the diagonal from the quad's south-east corner to its north-west corner (the mirror
   image of its meaning in the triangulation);
3. the triangle corners are taken in the mirrored order, so every triangle normal is the
   **downward** unit normal of its triangle, and the average points down (about (0, 0, −1)
   under a flat ceiling).

So the triangles averaged for a ceiling sample are in general not the drawn ceiling triangles
around it (§4.3); under a flat ceiling the result is the same either way.

**Block edges.** A triangle that needs a sample outside the block uses the neighbouring
block's data: index −1 is index 31 of the block to the west (or south), index 33 is index 1 of
the block to the east (or north) (index 32 and the neighbour's 0 are the same samples). A
**corner** sample needs the block **diagonally** adjacent as well: when both indices are out
of range at once, both are mapped, so for example (−1, −1) is sample (31, 31) of the block to
the south-west. The split flag of a quad outside the block is read from the neighbour (or the
diagonal neighbour) in the same way. The client reads quads with indices −1 … 32 only; for a
quad at index 32 it uses the block's **own** vertex flag 32, not the neighbour's flag 0 (BIN (s),
as re-read by the fifth-revision review; not re-traced in the sixth revision). That
gives the same result, because flag 32 equals the neighbour's flag 0 in both bit 7 and bit 0
at every shared edge sample of this install (invariant T47: 25,648,920 of 25,648,920 for each
bit). When the needed neighbour record does not exist, or (for a ceiling) the neighbour has no
ceiling, the triangle is left out of the average. Normals along an edge therefore agree
between blocks when both neighbours exist.

How the programs of layers without a normal map use the stored normal (BIN-S second pass,
T52; records 0x2B0007E3 and 0x2B0007B0, pass 0x3F): the per-vertex hemispheric term of §9.1 and the sun's
**N** · *L₀* (interpolated, evaluated per pixel) take it as stored, without normalising; only
the per-vertex point-light sum normalises it first. Layers with a normal map use the frame of
§6.4.

<a id="s5"></a>
## 5. Alpha masks

<a id="s5-1"></a>
### 5.1 Format (Confirmed, 646,073 of 646,073)

Every mask named by a drawn layer exists as a 0x41 render surface (`records.md` §5) with format
code 28 (A8, one byte per texel), square, 128 × 128 (623,092), 256 × 256 (20,624) or
512 × 512 (2,357). A mask value *m* = byte / 255. The size does not always match the entry's
`mask_resolution`; read it from the surface.

Format check (BIN (n), T34): the client checks the surface's format code when it loads a
mask. For any code other than 28 it logs an error naming the surface and its format and binds
no mask; the layer is then drawn with the shader record's own default for parameter 3020
(0x40000007 in all 96 overlay records of this install, a texture list whose base level is a
white image with a thin dark border, `material.md` §7.2). This never happens in this install
(T14). **Tessera Decision:** reject a non-A8 mask as malformed data and draw the layer as if
it had no mask (§7: opacity from the diffuse alpha or 1), with a logged warning.

<a id="s5-2"></a>
### 5.2 Placement over the block (Confirmed)

A mask of *W* × *W* texels covers the whole 160 × 160 block:

- texture coordinate (s, t) = (x / 160, y / 160) with x, y the block-local position (x east,
  y north, `coordinates.md` §3); this is coordinate set 3 of §6.1;
- texel column *c* covers x ∈ [160·c/*W*, 160·(c+1)/*W*), and stored **row *r* covers
  y ∈ [160·r/*W*, 160·(r+1)/*W*)**: row 0 is the **south** edge, column 0 the west edge;
- sampled with bilinear filtering and clamp-to-edge addressing (BIN (c), (m)); the mask is
  the layer's `OpacityAlpha` texture, material parameter 3020 (§8.2).

Note the difference from the minimap tiles, whose row 0 is the north edge
(`landblock.md` §6.3), and from `terrain.md` §6's block UVs (v = 1 − y/160). A renderer that
uploads the mask rows in stored order samples it at t = y / 160.

Evidence: the non-zero texels of each mask fall inside the layer's triangles or under a later
opaque layer (§4.4) for 99.93 % of texels in this orientation, against 74–82 % for each of the
other orientations tested (both flips, the transpose and the half turn). The client's vertex
data (§6.1, set 3) gives the same mapping.

---

<a id="s6"></a>
## 6. Texture coordinates of a layer

<a id="s6-1"></a>
### 6.1 Coordinate sets on the terrain vertices (BIN)

The client builds one vertex per grid sample (33 × 33) and stores these 2-component texture
coordinate sets on it. With x, y the sample's block-local position (5·i, 5·j), *h* its
height in world units (§13), and (*o*ₓ, *o*ᵧ) a per-block offset (§6.2):

| Set | Components | Used for |
|---|---|---|
| 0 | (x + *o*ₓ, y + *o*ᵧ) | Layers with `projection` = 3 (XY: top-down) |
| 1 | (x + *o*ₓ, *h*) | Layers with `projection` = 1 (XZ: faces looking north or south) |
| 2 | (y + *o*ᵧ, *h*) | Layers with `projection` = 2 (YZ: faces looking east or west) |
| 3 | (i / 32, j / 32) = (x / 160, y / 160) | Masks (§5.2) and the light map (§10) |
| 4 | present only when the client's landscape-shadow feature is on | The baked-shadow texture (§9.5) |

The diffuse and normal textures of a layer use the set its `projection` selects, transformed
by §6.3. The layer's shader record reads that set as its first texture coordinate input
(set 0 for the XY records, set 1 for XZ, set 2 for YZ; BIN-S input signatures), and the
fixed-function path selects the same set by index (projection 1 → set 1, 2 → set 2, 3 → set 0;
BIN (m)). Cliff materials come as groups of entries with projections 1, 2 and 3 and separate
masks, which together give a three-plane (triplanar-style) cliff texture; the layer masks do
the plane blending.

<a id="s6-2"></a>
### 6.2 The per-block offset

The vertex builder places the vertices block-local (x, y from 0 to 160) and adds
(*o*ₓ, *o*ᵧ) to sets 0–2 only. The per-block build passes (*o*ₓ, *o*ᵧ) as the sum of two
pairs of floats (BIN (d), (p)):

1. **The block's place relative to a reference landblock.** The landscape renderer holds a
   current *reference landblock* (X_ref, Y_ref) of the same region. For a block (X, Y) the
   first pair is (160 · (X − X_ref), 160 · (Y − Y_ref)), computed when the block's terrain
   mesh is built. (Between different regions the client's helper returns a fixed
   far-away value; within one region the rule above holds.)
2. **An accumulated shift** (*a*ₓ, *a*ᵧ) held by the landscape renderer, initially (0, 0).
   Whenever the reference landblock changes from (X₀, Y₀) to (X₁, Y₁) in the same region, the
   renderer computes the step (*d*ₓ, *d*ᵧ) = (160 · (X₁ − X₀), 160 · (Y₁ − Y₀)):
   - if |*d*ₓ| < 480 and |*d*ᵧ| < 480 (a move of at most two blocks on each axis), it adds the
     step to (*a*ₓ, *a*ᵧ); if afterwards |*a*ₓ| ≥ 2,048 or |*a*ᵧ| ≥ 2,048, it resets
     (*a*ₓ, *a*ᵧ) to (0, 0);
   - otherwise (a jump of three or more blocks, a change of region, or no valid previous
     reference) it resets (*a*ₓ, *a*ᵧ) to (0, 0).

So (*o*ₓ, *o*ᵧ) = 160 · (X − X_ref) + *a*ₓ, and likewise for y. Between two resets the sum
does not change when the reference moves: it equals 160 · (X − X_a) with (X_a, Y_a) the
reference landblock at the last reset (the *anchor*). Layer textures therefore form one
continuous frame anchored near the viewer, and that frame jumps by a whole number of blocks
(160 units per block) when the accumulator resets. With the common scales (0.05, 0.1, 0.2,
0.25, 0.3, 0.5) 160 · scale is a whole number of texture repeats and a jump is invisible; with
scales such as 0.055 or 0.035 the texture shifts at a reset. The constants 160, 480 and 2,048
are from the client (decision 0012).

> **OPEN:** Which landblock the landscape renderer uses as its reference (the camera's, the
> player's, or another rule), when it updates it, and whether blocks already built are rebuilt
> after a reset. Without these, the anchor of the original's texture frame at a given moment
> cannot be reproduced exactly.
> **Tessera Decision:** take the landblock that contains the camera as the reference and
> apply the accumulator rule above, so sets 0–2 stay small numbers near the viewer and
> textures continue across block edges. Using fixed region coordinates (anchor (0, 0), offset
> (160·X, 160·Y)) is an acceptable simplification: it differs from the original only by whole
> blocks, which is invisible for the common scales.

<a id="s6-3"></a>
### 6.3 Texture transform (BIN)

The client builds the layer's texture matrix with the same routine as every material texture
transform (`material.md` §8.4). From the selected pair (p, q) of §6.1 the texture
coordinates (u, v) are, in this order:

1. **early translation:** p₁ = p + *e*ᵤ, q₁ = q + *e*ᵥ, with (*e*ᵤ, *e*ᵥ) the material
   parameters 1070/1071 (`UTranslateEarly`, `VTranslateEarly`). The terrain layer does not set
   them and every one of the 192 terrain shader records declares them with default 0
   (OBS2), so for terrain *e*ᵤ = *e*ᵥ = 0;
2. **scale:** p₂ = `u_scale` · p₁, q₂ = `v_scale` · q₁;
3. **rotate** by θ = `rotation` · π / 180, counter-clockwise in the (u, v) plane:
   u₃ = cos θ · p₂ − sin θ · q₂, v₃ = sin θ · p₂ + cos θ · q₂;
4. **translate:** u = u₃ + `u_offset`(t), v = v₃ + `v_offset`(t), where the offsets are float
   curves (`material.md` §5.3) evaluated at the material clock *t* (`material.md` §5.3.1). A
   mode-1 curve is its constant; the 28 mode-2 curves scroll the texture with
   `material.md` §5.3.1's linear ramp (units per second = `w[6]`).

The scale, rotation and translation are fed through material parameters 1001, 1002, 1033,
1017 and 1018 (§8.2). The client evaluates the scale and rotation as constants (they are
stored as plain floats) and the translations as the stored curves.

The matrix applies only to the layer textures (diffuse, normal, emissive, glow). Masks, the
light map and the baked-shadow texture use their sets unchanged. The layer textures repeat
(wrap addressing) and are filtered bilinearly with mipmaps (BIN (c), (m); anisotropic filtering
is a client setting). A negative scale mirrors the texture.

<a id="s6-4"></a>
### 6.4 Normal-map frame (BIN (k); BIN-S; SH)

**Per-vertex tangent frame.** When normal maps are in use the vertex builder also writes a
tangent **T** and a binormal **B** per vertex, from the vertex normal **N** (unit, Tessera
axes x east, y north, z up):

- **T** = the east axis with its component along **N** removed, normalised:
  **T** = normalise(**E** − (**E** · **N**) **N**) with **E** = (1, 0, 0);
- **B** = **N** × **T**;
- when the length before normalising is at most 10⁻⁶ (a normal pointing exactly east or west),
  **T** and **B** are both zero; the normal map then has no effect at that vertex.

For flat ground **T** = east, **B** = north. The frame is the same for all three projections:
the client does not build a separate frame for the side projections.

**Rotation of the mapped normal.** In the pixel program the tangent-space vector *n* =
2 · texel.rgb − 1 is first turned by the texture transform: with *c*₀ and *c*₁ the first two
columns of the texture matrix, each normalised over its four components (§6.3: *c*₀ ∝
(`u_scale` cos θ, `u_scale` sin θ), *c*₁ ∝ (−`v_scale` sin θ, `v_scale` cos θ)),

- *n*′.x = *n*.x · *c*₀.x + *n*.y · *c*₁.x,
- *n*′.y = *n*.x · *c*₀.y + *n*.y · *c*₁.y,
- *n*′.z = *n*.z,

then *n*′ is normalised. For positive scales this is the rotation of (*n*.x, *n*.y) by θ; a
negative scale flips the matching axis. *n*′ is used in the frame (**T**, −**B**, **N**) of
`lighting.md` §5.2.4: light, view and up vectors are taken into that frame per vertex and
interpolated, and the lighting of §9.2 is evaluated with *n*′.

> **OPEN:** (implementer, 2026-10-06) "each normalised over its four components" leaves
> open which components the client's 4 × 4 texture matrix holds in those two columns. If
> the matrix convention puts the translation (the `u_offset`/`v_offset` curves) into the
> same columns, a non-zero or scrolling offset would change *n*′. Tessera follows the
> proportionality stated above (*c*₀, *c*₁ from scale and rotation only; translation not
> included). Please confirm, or give the four components.
>
> **Answered (spec writer, 2026-10-07, sixth revision; T51, Inferred from BIN facts):** the
> programs read the matrix as u′ = m00·u + m01·v + m03 and v′ = m10·u + m11·v + m13
> (`lighting.md` §5.2.1), so the translations (`u_offset`, `v_offset` and the early
> translation) sit in the **fourth** column (m03, m13). The matrix is a 2-D transform appended
> to the identity (`material.md` §8.4), so its first two columns are
> *c*₀ = (`u_scale` cos θ, `u_scale` sin θ, 0, 0) and *c*₁ = (−`v_scale` sin θ,
> `v_scale` cos θ, 0, 0). A constant or scrolling offset therefore does not change *n*′; the
> proportionality stated above is exact.

---

<a id="s7"></a>
## 7. Layer opacity per pixel (SH and BIN-S, all 192 records; selection BIN)

For a pixel of a layer with entry *E*, let *m* be the mask value at the pixel (§5) and *a* the
alpha of the diffuse texture at the pixel's (u, v). The client chooses one of four record
families for the layer (§8.1), and the family fixes the opacity α:

| Layer | `use_opacity_mask` | Opacity α |
|---|---|---|
| first drawn layer of the chunk | any | 1 (no opacity input; output alpha 1) |
| later layer with a mask | 1 | α = clamp(*a* · *m* + *m*² · `alpha_blend_max`, 0, 1) |
| later layer with a mask | 0 | α = *m* |
| later layer without a mask | 1 | α = *a* |
| later layer without a mask | 0 | α = 1 |

Then:

1. **Discard.** The pixel is discarded when α ≤ τ, where τ = `alpha_test_threshold` if the
   layer is not the first drawn one and `alpha_test_threshold` > 0.0002, and τ = 0 otherwise
   (§8.2). With τ = 0 only fully transparent pixels are dropped.
2. **Output alpha** = α × the alpha of material colour 2000, which the client sets to 1 for
   terrain (§8.2), so α.

`alpha_blend_max` therefore matters only for masked later layers whose entry sets
`use_opacity_mask` (224 entries); in the other 58 entries with a non-zero value it has no
effect (§3.6). A positive value lets strong mask values reach full opacity even where the
diffuse alpha is low, which sharpens transitions painted with textures that carry an opacity
pattern.

---

<a id="s8"></a>
## 8. Blending algorithm in words

For one chunk (terrain or ceiling) of one block:

1. Resolve the region's terrain type table (§3.1) and, for every layer, its entry by key.
2. Walk the layers in stored order. Skip a layer with `whole` = 0 and `prims` = 0. A chunk
   with no layers at all is handled by §4.9 (one full-grid layer of the lowest-`draw_order`
   entry, or nothing).
3. Its geometry is its triangle range (`landblock.md` §3.8.3) or, for `whole` = 1, the full
   grid triangulation (§4.3).
4. The **first layer that is drawn** gets no opacity source and no alpha test (§7, §8.2), so it
   covers what is below it. Every later drawn layer is blended over what is already there with
   source factor α and destination factor 1 − α (standard "over"), after the discard test of
   §7. All 192 terrain shader records carry the blend factors SRCALPHA / INVSRCALPHA with depth
   write on in their lit passes (OBS2, `material.md` §7.6); the first layer is opaque because
   its output alpha is 1.
5. Depth: all layers lie on the same triangles; the client draws them in sequence on the same
   geometry. The client's depth comparison for them was not traced (§16; `lighting.md` §6
   explains why it is not in the data). **Tessera Decision:** "less or equal" (or later layers
   with depth writes off).
6. The colour of each pixel is computed per §9 before blending, so lighting and fog are
   applied per layer and then blended.

The result at a point covered by layers 0 … k (bottom to top) is the usual repeated "over":
C = C_k · α_k + (C_{k−1} · α_{k−1} + (… C_0 …) · (1 − α_{k−1})) · (1 − α_k), with layer 0
fully opaque.

Equivalent single-pass splatting (Tessera option): gather the drawn layers of a block (up to
80 observed, typically 2–20) and evaluate the same "over" chain in one shader, per triangle
only over the layers that contain it. The result is identical as long as the per-layer colour
is the same; it is Tessera's choice how to batch.

**Entries without a diffuse texture** (15, all in table 0x19000000; BIN (o), T35, T39). They are
named by 182 layers in 29 landblocks of region 1: 96 terrain-chunk layers (60 drawn, 36 skipped
because `whole` = 0 and `prims` = 0) and 86 ceiling-chunk layers (43 drawn, 43 skipped).
The client does not skip such a layer. When the entry's diffuse DID is 0 or its texture
does not load, the layer setup builds and discards a diagnostic string (no visible effect),
leaves material parameter 3000 unset and draws the layer with everything else as usual
(record choice §8.1, mask, opacity, lighting). Parameter 3000 then keeps the shader record's
default, 0x40000007 in all 192 terrain records of this install (OBS3): a texture list whose base
level is a white image with a thin dark border (`material.md` §7.2). Such a layer therefore
covers its triangles with that near-white texture, tinted and lit like any layer.
**Tessera Decision:** draw such a layer with the record default read from the data at run time
(the parameter-3000 default that the terrain shader records declare), or with plain white when
the shader records are not read; never skip it.

**Failure inside the layer walk** (BIN (q), T38). The client looks up every layer's key in the
table before testing whether the layer has triangles. If a key is missing (never in this
install, T5), or the per-layer setup fails (for example the shader record of §8.1 is missing),
the client's per-chunk routine stops at that layer and reports failure, so the later layers of
the chunk are not drawn by it. **Tessera Decision:** skip only the offending layer, log it, and
draw the rest.

<a id="s8-1"></a>
### 8.1 Features of a layer: choosing the shader record (BIN (g), (h); UDB)

The client draws each layer with one of 192 terrain shader records. It chooses the record by
a lookup key made of the projection and seven switches, each decided per layer as follows:

| Switch | On when |
|---|---|
| **overlay** | the layer is not the first drawn layer of the chunk **and** it names a mask |
| **opacity** | the layer is not the first drawn layer **and** `use_opacity_mask` = 1 |
| **normal map** | the entry names a normal texture, it loads, and the graphics settings allow it: the multi-pass-lighting option is on, the landscape-lighting-quality option is 2 or more, and the device class of the graphics back end is above 2 (BIN (v), T48; the texture is not even requested otherwise) |
| **gloss from normal** | `use_specular` = 1, `use_gloss_specular` = 1, `specular_from_normal` = 1 and the normal-map switch is on |
| **specular** | `use_specular` = 1, `use_gloss_specular` = 1, and either the gloss-from-normal switch is on or the opacity switch is off |
| **baked light** | the block has a light map (§10) and the client has a texture stage free for it |
| **emissive/glow** | the entry's emissive or glow texture loads |

The colour texture's alpha has one role per record: opacity (opacity switch on), gloss
(specular on without gloss from normal) or unused. Gloss from the normal texture uses that
texture's alpha.

The records are listed in the UNIQUEDB DID map (`records.md` §8; key 0xF of the root map,
`property-system.md` §8.1) with labels that spell out the same switches. A label is
`Render_Mat_Terrain_`, then `Overlay_` for overlay records, then `XYProj_`, `XZProj_` or
`YZProj_` (projection 3, 1, 2), then the colour role `ColorTexSRT`, `ColorAndOpacityTexSRT` or
`ColorAndGlossTexSRT`, then optionally `_TSNormalTexSRT` or `_TSNormalAndGlossTexSRT`, then
`_OpacityAlphaTex` for overlay records, then optionally `_BakedLightTex`, then optionally
`_EmissiveGlow`. The 192 labels are exactly the combinations the rule above can produce:
2 (overlay) × 3 (projection) × 8 (colour role, normal kind and specular) × 2 (baked light) ×
2 (emissive/glow). The client itself looks the records up by numeric keys of that map; a reader
that wants the original records finds them by their labels at run time (they are data; never
hard-code the DIDs or keys).

Tessera writes its own shaders and needs the switches, not the records. **Tessera Decision:**
treat the graphics conditions of the normal-map switch as met (the original's look at its
higher landscape-lighting settings), and treat a texture stage as always free.

**Pass-0x41 draw items (BIN (u), T49).** In the special client mode of `lighting.md` §8.6 the
layer setup adds, for each layer, a second draw item with pass code 0x41, used by the
light-map bake. Outside that mode no such item exists. Tessera draws none (`lighting.md` §8.6
Tessera Decision).

<a id="s8-2"></a>
### 8.2 Parameters the client sets for a layer (BIN (h); OBS2)

Before drawing a layer the client sets these material parameters (ids and labels of
`material.md` §8.0) on the layer's record; parameters a record does not declare are ignored:

| Parameter | Value |
|---|---|
| 3000 DiffuseMap | the entry's `diffuse` list |
| 3011 NormalMap | the entry's `normal` list (normal-map switch on) |
| 3018 EmissiveMap, 3013 GlowMap | the entry's `emissive` and `glow` lists (emissive/glow switch on) |
| 3020 OpacityAlphaMap | the layer's mask (overlay records) |
| 3019 BakedLightingMap | the block's light map (§10; baked-light switch on) |
| 3001 (sampler `BakedShadow`) | the client's landscape-shadow render target when that feature is on (§9.5); otherwise the record default 0x40000008, a white texture (`material.md` §7.2) |
| 2000 DiffuseColor | `diffuse_scale` with its alpha replaced by 1 |
| 2001 SpecularColor | `specular_colour` when `use_specular` = 1, else white |
| 1000 SpecularPower | `specular_power` when `use_specular` = 1, else 0 |
| 2014 GlowColor | `glow_colour` when the emissive/glow switch is on, else (0, 0, 0, 1) |
| 4004 AlphaTest | 1 when the layer is not the first drawn one and `alpha_test_threshold` > 0.0002, else 0 |
| 1043 AlphaTestRef | `alpha_test_threshold` when 4004 = 1, else 0 |
| 1203 `c_AlphaBlendMax` | `alpha_blend_max` for layers that are not the first drawn, else 0 |
| 1001, 1002 | `u_scale`, `v_scale` (as constants) |
| 1033 | `rotation` (as a constant, degrees) |
| 1017, 1018 | the `u_offset` and `v_offset` curves |
| 1070, 1071 | not set (record default 0, §6.3) |

`slope_orientation` and `mask_resolution` are not read by the layer setup.

---

<a id="s9"></a>
## 9. Colour of a layer pixel (BIN-S; inputs `lighting.md`)

The terrain programs follow the lighting model of `lighting.md` §5, with the material colour
*M* = `diffuse_scale` (§8.2) pre-multiplied into every light and ambient term
(`lighting.md` §4.1) and these terrain-specific inputs. *C* is the diffuse texel RGB; *A₀*,
*H₀*, *E₀*, *F₀* and the point-light sums are the engine inputs of `lighting.md` §4.2 and §8.

<a id="s9-1"></a>
### 9.1 Layer without a normal map

Per vertex: the ambient term *V*ₐ = clamp(*A₀* + (**N** · **U**) *H₀*, 0, 1) with the vertex
normal **N** and up vector **U**, plus the per-vertex point-light sum of `lighting.md` §5.6;
and the sun's **N** · *L₀* with the vertex normal. Both are interpolated.

Per pixel:

1. *B*ₗ = the light-map texel RGB (baked-light switch on, §10), else 0;
2. *S* = the sun's shadow factor of §9.5;
3. lighting Σ = *B*ₗ + *V*ₐ + clamp(**N** · *L₀*, 0, 1) × *S* × *E₀* (in the passes that carry
   the sun as light 0; `lighting.md` §3.2 and §5.5 for a point light 0);
4. colour = 2 × *C* × Σ, plus the emissive term of §9.5, then distance fog (`lighting.md`
   §10).

**Tessera Decisions (what `tessera-render` does; not the original's behaviour):** the ambient
term may be evaluated per pixel with the interpolated stored normal instead of per vertex and
then interpolated (the two differ only where the clamp acts inside a triangle); the placed
lights of the per-vertex sum may be evaluated per pixel with the smooth fade of `lighting.md`
§8.3 step 3, diffuse only (`lighting.md` §5.6 Decision). Without a normal map the stored
normal is used as stored (not normalised) for the ambient and the sun's diffuse term, and the
unit normal for the placed lights and the highlight, as above; this reproduces the original
only when the vertex normals are those of §4.10.

<a id="s9-2"></a>
### 9.2 Layer with a normal map

As §9.1 with the mapped normal *n*′ of §6.4 per pixel:

- ambient = clamp(*A₀* + (*n*′ · **U**ₜ) *H₀*, 0, 1) + the per-vertex point-light sum, where
  **U**ₜ is the up vector taken into the tangent frame per vertex;
- sun = clamp(*n*′ · *L*ₜ, 0, 1) × *S* × *E₀*, with *L*ₜ the light vector in the tangent frame.

<a id="s9-3"></a>
### 9.3 Fog and alpha

Fog is the per-vertex linear fog of `lighting.md` §10, applied per layer before blending.
Output alpha is α of §7.

<a id="s9-4"></a>
### 9.4 Specular (specular switch on, §8.1)

With *g* the gloss (the normal texture's alpha when the gloss-from-normal switch is on,
otherwise the diffuse alpha), *V*ₜ the view vector in the tangent frame and **H** =
normalise(*V*ₜ + *L*ₜ):

- term = *g* × clamp(*n* · **H**, 0, 1)^*p* × *F₀* × *S*, where *n* is *n*′ with a normal map
  and the vertex normal (0, 0, 1 in the tangent frame) without one, *p* = material 1000
  (`specular_power`) and *F₀* = light 0's specular colour × material 2001
  (`specular_colour`);
- the term is added after the factor 2 of §9.1 step 4, and it is scaled by the same shadow
  factor *S* as the sun's diffuse term.
- the specular power rule of `lighting.md` §5.8 applies: the client uploads a black specular
  colour when the effective power is below 0.5. Terrain layers set parameter 1000 (power) and
  2001 (specular colour) like any material (§8.2), and those bind with the same semantics as in
  every record, so the rule reaches them too (Inferred from `lighting.md` L21 and §8.2). With
  `use_specular` = 0 the power is 0 (§8.2), so such a layer has no highlight.

The term is not multiplied by **N** · *L*, by the albedo or by the factor 2, so its strength
is set by the gloss *g* and the light's specular colour (BIN-S second pass, T52; records
0x2B00076A and 0x2B00079D, pass 0x3F, permutations with feature bit 4). **Data note (OBS5, T44):** where the
gloss is the diffuse alpha, that alpha is small in most entries: over the 1,073 entries with
both specular switches on, gloss not from the normal texture and a diffuse texture (227
distinct textures; 1,024 entries use DXT3, 33 DXT1, 16 DXT5), the mean alpha is below 0.25 in
89 % and below 0.5 in 95 % (median 0.049); only 3.4 % have an alpha of 1 throughout. Such
layers show a faint sheen, not a broad highlight. Example: the diffuse texture of key 67 of
0x19000002 (`m4-5-visual.md` rows T6 and L5) has alpha between 0 and 0.20 (mean 0.09), so with
a white sun specular colour, white `specular_colour` and *S* = 1 the term adds at most about
0.2 (typically 0.09) to each channel at the centre of the reflection, against a diffuse
term 2 × *C* × Σ of about 0.3–0.4 there (texture mean albedo about (0.42, 0.37, 0.29); the
keyframe of 0x1A000000 at *t* = 0.5 has ambient brightness 0.2, sun brightness 0.55 and sun
specular white; sun colour per the Tessera Decision of `lighting.md` §8.1).

<a id="s9-5"></a>
### 9.5 Emissive, glow and baked shadow

- **Emissive** (emissive/glow switch on): colour ← 2 × *C* × Σ + *Em*.rgb × *G* before fog,
  with *Em* the emissive texel at the layer's (u, v) and *G* = material 2014 (`glow_colour`).
- **Glow** pass (code 0x08): output RGB = the glow texel × *G* × *M*, output alpha = the
  layer's α of §7, after the same discard test (into the bloom target, `lighting.md` §5.9).
- **Baked shadow** (BIN (l), BIN-S): every terrain record samples a `BakedShadow` texture
  (parameter 3001) at coordinate set 4. The client binds a render target it generates at run
  time, 512, 1,024 or 2,048 texels square by a graphics setting, when its landscape-shadow
  feature is on; otherwise the record default, a white texture. The sun's shadow factor is
  *S* = the baked-shadow texel RGB where the dynamic sun shadow of `lighting.md` §9 is faded
  out (fade φ = 0), and *S* = lerp(baked-shadow texel, landscape shadow colour, φ) otherwise
  (`lighting.md` §9 steps 5–7). With the white default, *S* is exactly the factor of
  `lighting.md` §9.

> **OPEN:** What the client renders into the landscape-shadow target, over which area, and
> how coordinate set 4 maps the landblock onto it. **Tessera Decision:** treat the feature as
> off: the baked-shadow texel is white (1, 1, 1).

<a id="s9-6"></a>
### 9.6 Older back ends (SH; for reference)

The OpenGL text of the same records differs from §9.1: the baked-shadow texel multiplies the
albedo *C* (all light terms), the sun term has no shadow map, and the sun's **N** · *L₀* is
taken per vertex. Tessera reproduces the newer back end (`lighting.md` §2.2).

---

<a id="s10"></a>
## 10. The landblock light map (answers `landblock.md` §6.4 OPEN)

- `Landblock_LightMap` (on 4,692 blocks) and `Landblock_CeilingLightMap` (on 749) name DXT1
  surfaces (OBS3, T37: all 5,422 distinct surfaces are DXT1; 5,421 are 128 × 128 texels and
  one is 64 × 64). Read the size from the surface; the mapping below is by texture coordinate,
  so it holds for any size.
- **Orientation (Confirmed):** the same as masks (§5.2): row 0 on the south edge, column 0 on
  the west edge, coordinates (x / 160, y / 160) from set 3 of §6.1. Seam test on adjacent
  blocks (4,323 pairs): mean edge difference 2.2–2.6 (E–W) and 2.1–2.2 (N–S) grey levels in
  this orientation, 5.5–17.6 for the other seven.
- **Use (BIN):** bound as material parameter 3019 (`BakedLightingMap`) with clamp addressing
  and bilinear filtering, and **added** to the lighting sum (§9.1 step 1), not multiplied.
  Content: baked light from placed light sources; 79 % of all texels are 0 (black), the rest
  are dim, warm colours (OBS).
- The ceiling light map applies to the ceiling chunk in the same way.
- **Placed lights on terrain (BIN (t), T43).** Lights whose `Light_BakedLandscapeLight` flag
  is set are never put in the light lists of terrain or ceiling chunks (both are landscape
  objects to the client), whether the block has a light map or not. In normal play their light
  on the ground is the stored light map, and a block without a light map shows none of it.
  Lights without the flag reach the chunks like any object (`lighting.md` §8.3, §8.5).
- **The light-map bake (BIN (u), T49; `lighting.md` §8.6).** The client can also draw the
  flagged lights that reach a chunk into the chunk's baked-lighting texture (pass code 0x41,
  composited additively after a clear, replacing the stored content). Only a chunk that holds a
  baked-lighting texture takes part, and in play a chunk holds one only when its block names
  `Landblock_LightMap` (terrain chunk) or `Landblock_CeilingLightMap` (ceiling chunk) (BIN (u),
  as traced by the fifth-revision review). The bake and its per-layer pass-0x41 draw items are
  tied to a special client mode; **Inferred:** it is the light-map baking of the client's tools
  and does not run in normal play, at any graphics setting. **Tessera Decision:** no bake; the
  stored light map is used as it is. Example (OBS5): under each of the five orange lights of
  `m4-5-visual.md` row L2 the block's light map 0x41221577 holds texels of about (0.29, 0.11,
  0.0), so the pools are in the map; the light map of row L7, 0x4116792D, is black (largest
  channel value 0.010 over all texels).

---

<a id="s11"></a>
## 11. Roads, river beds and editor layers

- **Roads and paths** are ordinary terrain types (about 10 % of each table's labels name roads
  or paths). They are drawn as layers like any other; there is no separate road record.
- **River and lake beds** are terrain types too, drawn as ordinary layers. **Water surfaces
  are not terrain layers**: everything about water (the water table 0x16, landblock water
  levels, water volumes, drawing water) is in [`water.md`](water.md).
- A few entries are editor planning markers (2–5 per table, used by 47–816 layers per region).
  They have textures and are drawn like other layers; the client applies no special rule that
  was found.

---

<a id="s12"></a>
## 12. Distance behaviour (Reported; OPEN)

ER reports three landscape bands (full detail to about 14 blocks, medium to about 24 blocks,
then distant imposters), a setting for normal maps on far landscape and a near/far seam
blend. The layer data above describe the full-detail band. The far bands use other records
(the DB-object vocabulary names distant landscape sectors, `environment.md` §2;
`landblock.md` §7).

> **OPEN:** How far layers are drawn, what replaces them beyond that distance, and how the
> seam between near and far terrain is blended. Tessera may draw full layers at every
> distance it meshes (`terrain.md` §3) until this is known.

**Extra full-grid draws after the layers (BIN (v), (w); T50).** The per-chunk routine of §8
can draw the chunk's full grid (§4.3) again after its layers:

1. **Sector overlay.** The landscape renderer keeps objects for sectors of 3 × 3 landblocks
   (480 units); the block build looks up the sector that holds the block and passes that
   sector object's first material to the per-chunk routine when the renderer has one (and a
   renderer switch is on; not traced further). When such a material is passed, the
   multi-pass-lighting option is on, the landscape-lighting-quality option is above 0 and the
   device class is above 2, the routine draws the full grid once more with a copy of that
   material whose texture transform is set so that the sector image spans the sector: scale
   1/480 on u and −1/480 on v (parameters 1001, 1002), translation from the block's place in
   the sector (160 units per block) minus two offset values that the block build passes in
   (Inferred: the per-block offset of §6.2), scaled by the same factors (1017, 1018; the
   unscaled values go to 1019, 1020), and parameter 4006 (wrap V) set to 1.
   **Inferred:** the sector objects are the distant-landscape sectors (ER; `environment.md` §2),
   and this draw blends the near terrain toward the distant image. The material's blend state
   and its effect on the picture were not traced.
2. **Default layer again.** A further full-grid draw of the lowest-`draw_order` entry (§4.9)
   runs only in the special client mode of `lighting.md` §8.6 or under a client debug switch;
   it is not part of normal play (Inferred).

> **OPEN:** What the sector overlay contributes (its shader record, blend and fade) and when
> the renderer holds a sector material for a block. **Tessera Decision:** neither extra draw
> is made; a chunk shows its layers only.

---

<a id="s13"></a>
## 13. Height scale seen in the client (BIN; adopted)

Every place in the client that converts a stored height sample (the terrain vertex builder,
height queries, physics) computes height = stored × 2,000 × (1 / 65,535) in 32-bit floats,
i.e. stored / 32.7675, so 65,535 is 2,000 units. `landblock.md` §3.4 adopted this rule on
2026-10-05 (row L4); the former stored / 32.768 differs by a factor of 1.0000153, at most
0.031 units. The heights *h* of §6.1 use this rule.

---

<a id="s14"></a>
## 14. Worked example (synthetic)

A table with two entries (bytes shown as values, not as a dump):

| Field | Entry key 0 | Entry key 1 |
|---|---|---|
| label | 6 bytes (length byte 6) | 6 bytes |
| `draw_order` | 10 | 20 |
| `diffuse` | 0x40000001 | 0x40000002 |
| `u_scale`, `v_scale` | 0.1, 0.1 | 0.2, 0.2 |
| `rotation` | 0 | 90 |
| `u_offset`, `v_offset` | mode 1, 0.0 | mode 1, 0.0 |
| `projection` | 3 | 3 |
| `use_opacity_mask` | 0 | 1 |
| `alpha_test_threshold`, `alpha_blend_max` | 0, 0 | 0.1, 0.25 |
| `diffuse_scale` bytes | 0xFF, 0xFF, 0xFF, 0xFF | 0x20, 0x40, 0x80, 0xFF |

Each entry is 4 + 1 + 6 + 90 = 101 bytes; the record is 8 + 2 · 101 = 210 bytes. Entry 1's
`diffuse_scale` decodes to red 0.502, green 0.251, blue 0.125 (§3.3).

A block at landblock (X, Y) = (3, 7) has a chunk with two layers: key 0, `whole` = 1, no mask;
key 1, 10 triangles, a 128 × 128 mask.

At the block-local point (12.5, 3.0) inside one of key 1's triangles:

- Offset (§6.2) with the anchor at landblock (0, 0), i.e. the fixed-region-coordinate
  simplification: (480 + 12.5, 1,120 + 3.0) = (492.5, 1,123.0). (With the anchor at landblock
  (2, 7) instead, the point is (172.5, 3.0); for key 0's scale 0.1 that changes u by 32, a
  whole number of repeats, so the colour is the same.)
- Key 0: first drawn layer, record XY, colour role `Color`, no overlay: (u, v) = (49.25,
  112.3); opaque: colour C₀.
- Key 1: overlay (has a mask), opacity on (`use_opacity_mask` = 1), alpha test on (0.1 >
  0.0002). Early translation 0; p₂ = 98.5, q₂ = 224.6; θ = 90°: (u, v) = (−224.6, 98.5).
  Mask texel: column ⌊12.5 / 160 · 128⌋ = 10, row ⌊3.0 / 160 · 128⌋ = 2 (row 2 counted from the
  south edge). Suppose *m* = 0.8 and the diffuse alpha there is 0.5:
  α = clamp(0.5 · 0.8 + 0.64 · 0.25) = 0.56 > 0.1, kept.
- Result: 0.56 · C₁ + 0.44 · C₀.
- Had key 1 `use_opacity_mask` = 0, α would be *m* = 0.8 and `alpha_blend_max` would have no
  effect (§7).

---

<a id="s15"></a>
## 15. Corrections and notes for other specs

| Spec | Item | Status |
|---|---|---|
| `landblock.md` §3.8.2 | OPEN (1)–(4) | Adopted there 2026-10-05: (1) §4.1; (2) §4.2, §8; (3) §5.2; (4) §4.3 |
| `landblock.md` §3.8.4 | Quad grid OPEN | Adopted there: §4.6 (order 32·qy + qx Confirmed; meaning Probable) |
| `landblock.md` §3.4 | Height scale | Adopted there (L4): §13 |
| `landblock.md` §6.4 | Light map OPEN | Adopted there: §10 |
| `landblock.md` §6.4 | "The surfaces are DXT1, 128 × 128" | All DXT1, but one of the 5,422 distinct surfaces is 64 × 64 (§10, T37); readers take the size from the surface. For the landblock owner. |
| `collision.md` §6.5, §15 | Holes are not cave mouths; collision keeps them | Adopted here 2026-10-05 (§4.5, T11) |
| `environment.md` §4 | Kinds of 0x16 and 0x17 | Settled by `water.md` §3 (0x16 water table, 0x17 fog-zone table) |
| `material.md` §8.4 | Transform composition | Adopted there 2026-10-05: one rule, §6.3 here and `material.md` §8.4 |
| `scenery.md` §4.4 | "full body grammar OPEN"; "readers keep stored order" | Adopted there 2026-10-05: §4.4 defers the layout to §3.3 here and looks entries up by key (§3.4). |
| `terrain.md` §9 | Holes "at cave mouths"; ceiling diagonal OPEN | Revised there 2026-10-05 (§4.5 here; `collision.md` §6.4–§6.5) |
| `property-system.md` §4.7 | Colour channel order of property values (code 16) | Not settled by this spec: the 0x19 colours use the record colour routine (§3.3), not the property-value reader. |
| `lighting.md` §4.2 | `c_AlphaBlendMax` meaning OPEN | Adopted there: it is `alpha_blend_max` (§7) |
| `walker.md` §14.3, §14.4 | Terrain light selection keeps baked-landscape lights on chunks without a light map; chunks without layers drawn height-shaded | Differ from §10 (T43) and §4.9 (T40). For the implementers of the walker: align with those sections; the walker's terrain normals (`terrain.md` §5, central differences) also differ from §4.10, which the renderer's stored-normal shading assumes |
| `render.md` §4.3, §5.2 | Unattached lights with `Light_OnlyInfluenceParent`; the L30 conclusion cited in §5.2 | `lighting.md` §8.5 step 3 (BIN) withdraws the Decision for unattached lights; §5.2 stays right for the light lists, and the light-map bake is not part of normal play (§10, `lighting.md` §8.6). For the renderer's implementers |
| `terrain.md` §5 | Central-difference vertex normals | The original's rule is §4.10 (T42); for the terrain implementers |

---

<a id="s16"></a>
## 16. Open questions

> **OPEN:** Per-block texture offset (§6.2): the rule is now known (BIN (p)); open are the
> choice of the reference landblock, when it changes, and whether built blocks are rebuilt
> after a reset (Tessera Decision there).

> **OPEN:** Content and mapping of the landscape-shadow render target (§9.5). Tessera treats
> it as white (Tessera Decision).

> **OPEN:** When the landscape renderer's full-grid switch turns off (§4.3, T45). Tessera
> Decision there: always on.

> **OPEN:** `slope_orientation` (147 entries): not read by the layer setup; any other use was
> not found. Tessera ignores it.

> **OPEN:** Quad-grid weighting (§4.6).

> **OPEN:** Distance bands and near/far blending (§12).

> **OPEN:** What lies over the 413,655 undrawn terrain triangles whose ceiling samples are
> all 0, and whether a walker can reach them (§4.5, T11). Carried from `collision.md` §6.5;
> it affects neither drawing (they are not drawn) nor collision (they collide).

> **OPEN:** Client depth comparison for layers (§8 step 5). The graphics back ends receive the
> comparison from the engine with each depth state (`lighting.md` §6); the value the engine
> sets for terrain layers was not traced. Tessera Decision "less or equal".

> **OPEN:** What sets the client mode that enables the light-map bake and its pass-0x41 draw
> items (§10, `lighting.md` §8.6). Tessera Decision: no bake.

> **OPEN:** What the sector overlay draw contributes and when a sector material is present
> (§12, T50). Tessera Decision: not drawn.

Answered in the sixth revision (2026-10-07): the client's second path for baked-landscape lights
(§10, T43, T49: a light-map bake, Inferred not in normal play); the extra full-grid draws of the
per-chunk routine (§12, T50); the conditions of the normal-map switch (§8.1, T48); the triangle
count around a sample and the block-edge rule (§4.10, T46, T47); the columns of the texture
matrix used for the normal rotation (§6.4, T51; the implementer OPEN of 2026-10-06).

Answered in the fifth revision (2026-10-07): what is drawn for a chunk without layers (§4.9,
T40, T41; the implementer's OPEN of §4.5); ceiling normals and the client's vertex-normal rule
(§4.8, §4.10, T42); whether lights flagged as baked reach the terrain (§10, T43).

Answered in the third revision (2026-10-05): the client's handling of entries without a
diffuse texture (§8, T35; formerly unstated), the handling of a non-A8 mask (§5.1, T34), and
the per-block offset rule (§6.2, T36).

---

<a id="s17"></a>
## 17. Change log

| Date | Change |
|---|---|
| 2026-10-05 | First version. Terrain type table decoded; layer meaning, order, masks, texture coordinates, opacity and blending specified; landblock light-map orientation and the quad grid settled; height-scale note. |
| 2026-10-05 | Revision (spec writer, M4 under decision 0011). §1: new tags OBS2, UDB, BIN-S, LT, WAT; BIN questions (g)–(m); rows T9, T13, T16–T19, T22, T23 revised, T24–T33 added. §3.3: client labels of every field; fields renamed to the client's terms (`use_opacity_mask`, `use_specular`, `use_gloss_specular`, `alpha_test_threshold`, `alpha_blend_max`); colour byte order B, G, R, A (BIN); label length is a VLE (OPEN closed); §3.5 invariant 6; new §3.6 census. §4.7: the coarse grid is the scenery word (stale text removed). New §4.8 (ceiling chunks). §5.2, §6.1: coordinate set 4; set selection by record. §6.2: offset sources. §6.3: complete transform rule (early translation, rotation direction, curve clock, which textures it applies to); stale "mode 2 OPEN" removed. New §6.4 (tangent frame, rotation of the mapped normal). §7 rewritten: opacity per record family (T19 corrected). §8 step 4 and new §8.1 (feature switches and record choice), §8.2 (parameters). §9 rewritten with the newer back end's terrain lighting, specular, emissive, glow and baked shadow (§9.1–§9.6). §10: use BIN. §11: water replaced by a pointer to `water.md`. §13: adopted. §14: example updated. §15, §16 updated. |
| 2026-10-05 | Third revision (spec writer, after review; M1 and M4 under decision 0011). §1: OBS row corrected (the "5,449 light maps" figure counted every 128 × 128 image of a sample dump; the install has 5,441 light-map properties); new tags OBS3 and COL; BIN questions (n)–(q); T11 rewritten from the full census of `collision.md` §6.5 (2,368 landblocks, all with a ceiling; collision keeps the triangles); new rows T34–T38. §3.3: the former-names note cites the real parameter labels (1043 `AlphaTestRef`, 1203 `c_AlphaBlendMax`). §4.5 rewritten: holes are terrain hidden under the ceiling surface or where there is no ceiling, not cave mouths; collision keeps them. §5.1: the A8 check is BIN (n) with a provenance row (T34) and a Tessera Decision. §6.2: the per-block offset rule (reference landblock, accumulated shift, reset at 2,048 units or a jump of 480 units or more); the OPEN narrowed to the choice of reference; Tessera Decision revised (camera block as reference; fixed region coordinates acceptable). §8: depth comparison labelled Tessera Decision; the client draws entries without a diffuse texture with the record default (formerly unstated, only a Decision); behaviour when a layer key is missing or a setup fails. §10: all light maps DXT1, one of 5,422 is 64 × 64. §14: offset wording follows §6.2. §15: `scenery.md` §4.4 row marked adopted; rows for `landblock.md` §6.4, `collision.md` §6.5 and `terrain.md` §9. §16 updated. |
| 2026-10-06 | Fourth revision (spec writer, after re-review; M1). §4.5: the last paragraph no longer calls all undrawn triangles hidden terrain; only the 2,336,479 with the ceiling at or below the terrain are shown to be hidden, and what covers the 413,655 with all ceiling samples 0 is an OPEN callout carried from `collision.md` §6.5; T11 says the same. §8: the 15 entries without a diffuse texture are named by 182 layers (96 terrain-chunk layers, 60 drawn and 36 skipped; 86 ceiling-chunk layers, 43 drawn and 43 skipped), replacing "used by 60 layers"; new tag OBS4 and row T39. §16: the 413,655-triangle OPEN added. |
| 2026-10-07 | Fifth revision (spec writer; M1 and M4 under decision 0011, Ghidra decompiler on the same build). Status Accepted → Revised. §0: new bullet. §1: tag OBS5, BIN questions (r)–(t), rows T40–T45. §4.3: full-grid list of ceiling chunks (bit 0, downward winding); the landscape renderer's full-grid switch (T45) with an OPEN and a Tessera Decision. §4.5: the implementer's OPEN on chunks without layers answered (pointer to §4.9). §4.8: ceiling-normal OPEN answered (normals point down; §4.10). New §4.9 (chunks without layers: the block's open flag, one full-grid layer of the lowest-`draw_order` entry, census). New §4.10 (vertex normals: average of unit triangle normals, ceiling rule with mirrored splits, block edges, how the programs use the stored normal). §8 step 2: pointer to §4.9. §9.4: the specular term's strength and the gloss census (T44), with the L5 example. §10: baked lights never reach terrain or ceiling chunks (T43); light maps of rows L2 and L7. §16: ceiling-normal OPEN removed; full-grid switch OPEN added; answered list. Implementers: `walker.md` §14.4's height-shaded drawing of chunks without layers and its Decision on baked lights for chunks without a light map, and `terrain.md` §5's central-difference normals, differ from the original as now specified. |
| 2026-10-07 | Sixth revision (spec writer; M1 and M4 under decision 0011, own copy of the Ghidra project; incident 0018 follow-up). Status Revised → Review. Header: long review records moved to the dirty-room review log, short record kept. §1: tag OBS6; BIN-S tag extended with its second pass (records and question of the fifth revision's §4.10 and §9.4 statements); BIN questions (u)–(w); T6 says non-decreasing; T43 narrowed ("normal play" Inferred); T39 moved before T40; new rows T46–T52. §3.3, §4.2: "ascending" → non-decreasing, with the equal-value chunks. §4.10: four to eight triangles per sample with the census (T46); corner samples use the diagonal neighbour; quads −1 … 32 and flag 32 with the invariant T47; BIN-S references. §6.4: implementer OPEN answered (T51). §8.1: exact conditions of the normal-map switch (T48); the pass-0x41 draw items; Tessera Decision reworded. §9.1: Tessera Decisions for per-pixel ambient and placed lights (what `tessera-render` does). §9.4: the specular-power rule applies to terrain (Inferred). §10: placed lights in normal play and the light-map bake (T49) with Tessera Decision. §12: the extra full-grid draws (T50) with an OPEN and Tessera Decision. §15: rows for `walker.md`, `render.md`, `terrain.md`. §16 updated. Change-log rows put in date order. |
