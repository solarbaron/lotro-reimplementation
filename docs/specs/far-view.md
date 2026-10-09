<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Far view: how the client uses distant-landscape sectors, imposters and far entities at run time

| | |
|---|---|
| **Status** | Accepted (2026-10-09, after review of the 2026-10-09 revision: the implementer notes of §12 answered; Tessera Decisions FD10–FD15 and two data facts FV23–FV24 added). Before: Revised (2026-10-09), Accepted (2026-10-08) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate session, not the author) |
| **Review record** | 2026-10-08, spec reviewer: passed (CLEAN_ROOM.md §5.3), Accepted. Details in the dirty-room review log (decision 0018). 2026-10-09, spec reviewer (separate session, not the author): the 2026-10-09 revision (§5.2, §6.3, §9.3, §12, §16, provenance OBS-F2, FV23, FV24, FD10–FD15) passed CLEAN_ROOM.md §5.3, Accepted. FV23 and FV24 spot-checked read-only with an independent parser on 2026-10-09 against all five named shader records (all 5 + 11 + 3 + 3 + 4 render-state blocks): declarations, defaults, cull, alpha test, reference 0.1 and blend reproduced. Non-blocking notes: (1) two of the 5 and two of the 11 far-terrain blocks are ONE / ZERO, not SRCALPHA / INVSRCALPHA, which the unchanged §6.3 render-state sentence does not mention (FV23 claims only cull, which holds in all); (2) the sliver corner order that FD10 makes moot is still given only as "oriented like the mesh" (`distant-landscape.md` §5.3). |
| **Implements into** | `crates/world` (radii, sector set, per-landblock grid, far-entity list), `crates/terrain` or `crates/render` (far-terrain batches, stitching, seam overlay, horizon fill), `crates/render` (far-terrain and sprite programs), `apps/client` (walker horizon, settings) |
| **Companion specs** | [`distant-landscape.md`](distant-landscape.md) (record layout §3, meaning §4, mesh and index construction §5.3, material §5.4, water §5.5, sprite list §5.6; this spec answers several of its §10 OPENs). [`walker.md`](walker.md) §11.2 (graphics preferences, *M_d*), §11.3 (extent *E*, global limit *G*), §11.4 (object draw distances), §14.4 (Tessera's own far band). [`entity-descriptor.md`](entity-descriptor.md) §9.3 (entity streaming manager, long-range set, factor *s*). [`misc-records.md`](misc-records.md) §5 (imposter tables, sprite quad, atlases, mirrored sprites). [`terrain-texturing.md`](terrain-texturing.md) §12 (sector overlay on near terrain). [`water.md`](water.md) §8.1 (landscape water geometry). [`lighting.md`](lighting.md) §2–§4, §10 (passes, engine inputs, fog). [`material.md`](material.md) §7.6 (render-state block), §8.4 (texture transforms). [`property-system.md`](property-system.md) §8 and [`records.md`](records.md) §8 (DID maps). [`mesh.md`](mesh.md) (render meshes). [`coordinates.md`](coordinates.md) (frames, units). |

---

<a id="s0"></a>
## 0. Purpose

`distant-landscape.md` decodes the distant-landscape sector records and describes how the
client turns one record into meshes. This spec describes how the client **uses** those
records while the viewer moves: which sectors and landblocks are resident, which part of the
landscape is drawn at full detail and which from the sectors, how the far terrain is
textured, lit, fogged and faded, how the near and far terrain meet, how the sector water
and the distant sprites (imposters) are drawn and faded against the real objects, how the
sectors' entity lists bring long-range objects into the world, and what fills the view
beyond the last sector.

With it an implementer can reproduce the original's far view at the original's settings,
and knows which parts are Tessera's own choices.

All counts and DIDs quoted are observations of one install on one date; values that come
from data are read at run time. Numeric constants from the client's code are allowed by
[decision 0012](../decisions/0012-numeric-constants.md) and are marked BIN.

---

<a id="s1"></a>
## 1. Provenance

Marks: **Confirmed** = observed in the user's install on the date given, for every record
of the kind; **BIN** = rests on the client's code only (method M4); **OBS** = read from data
records of the install (for shader programs: the programs stored in the shader records,
disassembled with a generic DXBC disassembler; these are data, not the executable);
**Inferred** = a conclusion from BIN or OBS facts that no listing or probe showed directly;
**Tessera Decision** = Tessera's own rule where the original's is not known.

| Tag | Source |
|---|---|
| BIN-F | **M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5, the build of `distant-landscape.md`'s BIN row), read on 2026-10-08 with a headless Ghidra session on an own copy of the shared analysis project (deleted afterwards), plus the listings the `distant-landscape.md` session left in the dirty scratch area. **Questions:** (a) the values of the near radius and the far radius of the sector manager and how the graphics preferences set them; (b) which landblocks are drawn at full detail and which from the sectors; what the near-field batch flag does; (c) when sectors are loaded, kept and discarded; (d) the far-terrain material: which material records, which textures (parameter 3001, normal maps) and under which options; (e) how the landscape extent, fog and draw distances interact, and what is drawn beyond the last sector; (f) when distant sprites are drawn, their fade distances, the per-sprite byte and flag; (g) what the client does with the sectors' entity lists. **Why M1–M3 were not enough:** the radii, the batch rules, the material choice, the fade rules and the entity handling are code with no trace in the records; the shader records hold the per-pixel arithmetic but not the inputs the client feeds them; no published source describes the far view; no running client or frame capture is available (M3). **Read:** the landscape manager's update, sector-list, option, radius and per-frame functions; the sector mesh draw and readiness functions; the near-landscape block builder's option path; the preference registration table and the reader of `LandscapeDrawDistance`; the scene-description builder and its initialiser; the frame state copy (draw-distance scale); the sprite batch builder and sprite painter; the entity streaming manager's distant set; the sky-gradient placement. Nothing about network, login, keys, launcher, patcher or anti-tamper (decision 0004). Output is prose facts only; listings and addresses stay in the dirty scratch area and the review log. |
| OBS-F | **M1**, user install (Steam), 2026-10-08, read-only: the DID map 0x28000002 (UNIQUEDB, `property-system.md` §8) decoded for keys 0x39, 0xB6, 0x7A and 0x95; the shader records 0x2B0005ED and 0x2B000461 decoded with the layout of `material.md` §7 (parameter declarations and defaults, permutations, render-state blocks) and their DXBC programs disassembled; the texture lists 0x40000002, 0x40000007 and 0x40017690 and the surfaces they name (headers); the render mesh 0x06000000 (presence); **all 33,159** sector records streamed and parsed (sprite `distance` per imposter key; entity view distances). Repository tools (`tessera-dattool`) and throwaway probes were used. |
| OBS-F2 | **M1**, user install (Steam), 2026-10-09, read-only, with throwaway probes in the dirty-room scratch area and the layout of `material.md` §7: the parameter declarations, defaults and every render-state block of the two far-terrain templates 0x2B0005ED and 0x2B000461 and of the three sprite shader records 0x2B00000E, 0x2B0003B9 and 0x2B0006E5 (`misc-records.md` §5). |
| SPEC | `distant-landscape.md` (DL rows), `walker.md` (W rows), `entity-descriptor.md` §9.3, `misc-records.md` §5, `terrain-texturing.md` §12, `water.md` §8.1, `lighting.md`, `material.md`. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| FV1 | `LandscapeDrawDistance` level → *R* = 2, 4, 6, 9, 12 landblocks (any other stored level gives 6); far radius *F* = ⌊*R* / 3⌋ + 1 sectors; *R* itself is kept but not read by the sector or near-landscape code read (§3.1) | BIN-F (a); `walker.md` W3 | BIN |
| FV2 | Near radius *N* = 1 landblock, set when the landscape settings are initialised; only a debug override replaces it (§3.1) | BIN-F (a); `walker.md` W4 | BIN |
| FV3 | Full-detail terrain (landblock data, block maps, landblock properties) is built only for the landblocks within Chebyshev distance *N* of the viewer's landblock: 3 × 3 landblocks (§2, §3.2) | BIN-F (b) | BIN |
| FV4 | Sector list: every existing sector within ±*F* sector keys of the viewer's sector (§3.3) | BIN-F (a); DL21 | BIN |
| FV5 | Update triggers; discard of every sector on a region change or a move of more than one landblock; sectors leaving the list are detached and kept; the update is spread over frames under a time budget (§3.4–§3.6) | BIN-F (c) | BIN |
| FV6 | Per-landblock grid: uncovered, near (Chebyshev ≤ *N*) or far, with the LOD of DL21; LOD and near status are relative to the viewer's landblock only (§4) | BIN-F (b); DL21 | BIN |
| FV7 | The near batch of a sector mesh is drawn only under a scene switch that the world view's scene description leaves off: in the world view, sector terrain is drawn only outside the near field (§5.1) | BIN-F (b) | BIN |
| FV8 | Far landblocks next to the near field stitch to its full-resolution edge; with *N* = 1 no far landblock has LOD 0; adjacent landblocks differ by at most one LOD (§5.2) | BIN-F (b); DL20, DL21; arithmetic | BIN; bounds Inferred (arithmetic) |
| FV9 | Far-terrain material templates come from UNIQUEDB keys 0x39 (plain) and 0xB6 (normal-mapped); the detail texture from key 0x7A; the normal-mapped one is used when `FarLandscapeNormalMaps` is on and the graphics core is 2 or 5 (§6.1) | BIN-F (d) (keys, rule); OBS-F (labels, targets, types) | BIN; targets Confirmed 2026-10-08 |
| FV10 | Per-sector material copy: parameter 3000 = colour list, 3001 = detail texture, 4005 = 4006 = 0 (§6.2) | BIN-F (d); DL row BIN of `distant-landscape.md` §5.4 | BIN |
| FV11 | Far-terrain shading: texture combination, per-vertex or per-pixel lighting, ×2, fog, far-draw fade, alpha blending (§6.3, §6.4) | OBS-F (programs, defaults, render states) | Confirmed (programs) 2026-10-08 |
| FV12 | Normal-mapped template: normal-map strength ramp over horizontal distance 80–240, near-view mask, seam-overlay alpha (§6.4, §6.5) | OBS-F (programs); BIN-F (d) (conditions) | Confirmed (programs); meaning of the near-view origin Inferred |
| FV13 | Landscape extent *E* = 160 × (3*F* + *N*) units; the global object limit *G* = *E* when a landscape is drawn (§7) | BIN-F (e); `walker.md` W5 | BIN |
| FV14 | Horizon fill: a render mesh from UNIQUEDB key 0x95 drawn around the eye and stretched down to the lowest terrain near the edge of the landscape (§7.3) | BIN-F (e); OBS-F (key → existing render mesh) | BIN |
| FV15 | Sector water meshes are built once per sector record and drawn whenever the sector is listed, at every distance, the near field included (§8) | BIN-F (c), (e) | BIN |
| FV16 | Distant sprites exist only when `DistantImposters` is on and the device class is at least 3; every listed sector contributes its sprites (§9.1, §9.2) | BIN-F (f) | BIN |
| FV17 | Sprite fade-in distance: the record's `distance` inside the 3 × 3 window around the viewer's landblock, a screen-size value outside it; fade formula (§9.3) | BIN-F (f); OBS-F (sprite vertex program, as `misc-records.md` §5) | BIN + Confirmed (program) |
| FV18 | The sprite distance scale is the object draw-distance scale: crowd factor × *M_d* (§9.3) | BIN-F (f) | BIN |
| FV19 | Sprite `distance` per imposter key: 801 of 818 keys use one value, 17 use two; the seven values used are LOD-class `d0` values (§9.4) | OBS-F (all 3,549,236 sprites) | Confirmed 2026-10-08; meaning Inferred |
| FV20 | The sprite's last byte is a heading in steps of 360/256 degrees; it decides the mirror flag (§9.5) | BIN-F (f) | BIN |
| FV21 | Sector entity lists feed the entity streaming manager's distant set: created within their view distance, removed beyond it or when the sector leaves the list (§10) | BIN-F (g) | BIN |
| FV22 | Detail texture: a nine-level texture list, top level 1,024 × 1,024 DXT1; template defaults of the texture transforms (§6.2, §6.3) | OBS-F | Confirmed 2026-10-08 |
| FV23 | Both far-terrain templates declare material colour 2000 with default (1, 1, 1, 1) and declare neither 4003 nor 4004; every render-state block of both has cull mode 2 (back faces culled); the per-sector copy sets only 3000, 3001, 4005 and 4006 (FV10), so far terrain, stitching triangles included, is drawn with back-face culling and *M* = the template's 2000 (§5.2, §6.3, §12) | OBS-F2; BIN-F (d) via FV10 | Confirmed 2026-10-09 |
| FV24 | The three sprite shader records declare only textures (3000; 3011 in two of them) and 4007 = 0, no 1043, 4004, 4002, 4003 or 2000; every render-state block of all three is SRCALPHA / INVSRCALPHA, ADD, depth comparison "nearer or equal", depth write on, cull none (two-sided), alpha test on with reference 0.1, material colour white (§9.3, §12) | OBS-F2 | Confirmed 2026-10-09 |

---

<a id="s2"></a>
## 2. Overview: three zones around the viewer

The client splits the landscape around the viewer's landblock *V* = (*X*, *Y*) into three
zones (FV1–FV3, FV7, FV13, FV14):

| Zone | Landblocks | What draws it |
|---|---|---|
| **Near field** | Chebyshev distance ≤ *N* = 1 from *V*: 3 × 3 landblocks, 480 × 480 units | Full-detail terrain: landblock data heights, texture layers, light maps (`terrain-texturing.md`), ceilings; static objects of these landblocks; the sector water quads; distant sprites whose fade-in has begun (§9) |
| **Far field** | Every landblock of a listed sector outside the near field | Sector terrain meshes at LOD 1–3 (§5), sector ceiling meshes, sector water (§8), distant sprites (§9), long-range entities from the sector lists (§10) |
| **Beyond** | Outside the listed sectors | Nothing of the landscape; the horizon fill mesh (§7.3) and the sky |

The far field reaches the edge of a square of (2*F* + 1)² sectors around the viewer's
sector, so its extent depends on where *V* lies inside its sector: between 3*F* and 3*F* + 2
whole landblocks beyond *V* in each direction. The client summarises this as the landscape
extent *E* = 160 × (3*F* + *N*) units (§7).

| `LandscapeDrawDistance` | *R* | *F* (sectors) | Listed sector square | *E* (units) |
|---|---|---|---|---|
| VeryLow (0) | 2 | 1 | 3 × 3 sectors (9 × 9 landblocks) | 640 |
| Low (1) | 4 | 2 | 5 × 5 (15 × 15) | 1,120 |
| Medium (2) | 6 | 3 | 7 × 7 (21 × 21) | 1,600 |
| High (3, built-in) | 9 | 4 | 9 × 9 (27 × 27) | 2,080 |
| VeryHigh (4) | 12 | 5 | 11 × 11 (33 × 33) | 2,560 |

The near field does not grow with any setting. Tessera's walker currently draws full-detail
terrain over many rings (`walker.md` §3, §14.4); that is Tessera's choice, not the
original's (§16).

---

<a id="s3"></a>
## 3. Radii and the resident set

<a id="s3-1"></a>
### 3.1 Settings (BIN, FV1, FV2)

1. *R* comes from the `LandscapeDrawDistance` preference (`walker.md` §11.2): stored level
   0 → 2, 1 → 4, 2 → 6, 3 → 9, 4 → 12 landblocks; any other level gives 6. A debug switch of
   the client can replace *R* by a fixed value (Tessera ignores it).
2. The far radius is *F* = ⌊*R* / 3⌋ + 1, in sectors (3 landblocks).
3. The near radius is *N* = 1 landblock. The client writes 1 when it initialises its
   landscape settings and changes it only through a debug override.
4. *R* is stored with *F* and *N*, but none of the far-view or near-landscape code read uses
   it; only *F* and *N* reach the landscape manager.
5. When *F* or *N* changes (a preference change), the manager re-runs its update (§3.4).

<a id="s3-2"></a>
### 3.2 Near field (BIN, FV3)

The landscape manager lists the landblocks (*X* + *i*, *Y* + *j*) with |*i*|, |*j*| ≤ *N*
that are valid in the viewer's region, requests their landblock data (DB type 0x11), block
maps (0x2F) and landblock properties (0x29), and builds full-detail terrain and ceiling
chunks for them (`terrain-texturing.md`). These are the only landblocks with full-detail
terrain. The entity streaming manager uses the same range (`entity-descriptor.md` §9.3, *R*
= 1 there), so static objects exist for the same 3 × 3 landblocks, plus the long-range
objects of §10.

<a id="s3-3"></a>
### 3.3 Sector list (BIN, FV4)

The viewer's sector key is (⌊*X* / 3⌋, ⌊*Y* / 3⌋) (`distant-landscape.md` §5.1). The list
holds every key (*sx*, *sy*) with |*sx* − *vx*| ≤ *F* and |*sy* − *vy*| ≤ *F* whose corner
landblock (3*sx*, 3*sy*) lies in 0–254 on both axes and in a valid region. Keys are visited
column by column (X outer, Y inner). Each listed key's record is requested; a key without a
record is skipped (DL-E1), and a record that fails to read is logged once and skipped
(`distant-landscape.md` §5.8).

<a id="s3-4"></a>
### 3.4 When the list is rebuilt (BIN, FV5)

The manager compares the new viewer position with the last one:

| Change | Effect |
|---|---|
| Region differs (both positions valid), or the landblock moved by more than 1 on X or on Y, or a reset was requested | **Discard every sector** (meshes, water, sprites and entity entries), then rebuild |
| Landblock differs by at most 1 on both axes, same region | Rebuild the list; sectors that stay keep their meshes |
| Only the instance differs | No rebuild; the attached meshes are re-tagged with the new instance |
| Only the cell or the position inside the landblock changes | Nothing |
| *F* or *N* changes | Rebuild |
| `FarLandscapeNormalMaps` or the graphics core changes the active template (§6.1) | Discard every sector, then rebuild |
| `NearFarSeamBlending` changes (§6.5) | The near-field terrain is rebuilt; sectors are kept |

A position counts as invalid when its region is 0, its X or Y is 0xFF or its cell is the
"no valid cell" value (`coordinates.md`); a viewer with an invalid position lists no
sectors.

So LOD, near status and sprite windows change only when the viewer crosses a landblock
boundary, never inside a landblock.

<a id="s3-5"></a>
### 3.5 Sectors that leave the list (BIN, FV5; eviction OPEN)

At the start of every rebuild every cached sector is marked "not listed" and stamped with
the current time; listing it again clears the mark. A sector that stays unlisted after the
rebuild has its terrain, ceiling and water meshes removed from the scene and its entity
entries removed from the entity manager (§10), but its decoded record and built meshes
stay cached and are re-attached if it is listed again. The rule that finally frees cached
sectors was not found (§15). A discard (§3.4) frees everything.

<a id="s3-6"></a>
### 3.6 Order of work (BIN, FV5)

A rebuild runs as a sequence of stages that may be spread over several frames: each call is
given a time budget and returns early when it is spent, continuing at the next call. The
stages, in order:

1. build the sector list (§3.3);
2. mark cached sectors unlisted (§3.5);
3. for each listed key: re-mark it listed if cached, otherwise read the record, check the
   colour list and ceiling colour list (a missing one is logged, the sector is still built),
   build its terrain, ceiling and water meshes (`distant-landscape.md` §5.3–§5.5) and cache
   it;
4. compute the per-landblock grid (§4) over the bounding rectangle of the listed sectors;
5. for each cached sector: rebuild its batches if its per-landblock descriptors changed
   (§5.1), attach or detach its meshes, add or remove its entity entries (§10), and add its
   sprites to the sprite batches (§9);
6. let the entity manager process its distant set (§10);
7. rebuild the sprite batches' vertex data (§9).

The budget's value comes from the caller and was not traced. **Tessera Decision:** do the
same work on background workers and swap the result in atomically per sector; never stall a
frame.

---

<a id="s4"></a>
## 4. The per-landblock grid (BIN, FV6)

For every landblock *L* in the bounding rectangle of the listed sectors the client stores one
code:

| Code | Meaning |
|---|---|
| uncovered | No listed, readable sector holds *L* (initial value of every cell) |
| far, LOD *l* | *L* is in a listed sector and max(\|*dx*\|, \|*dy*\|) > *N* |
| near, LOD *l* | *L* is in a listed sector and max(\|*dx*\|, \|*dy*\|) ≤ *N* |

with (*dx*, *dy*) = *L* − *V* in landblocks, and *l* from *d*² = *dx*² + *dy*²: 0 when
*d*² ≤ 3, 1 when *d*² ≤ 24, 2 when *d*² ≤ 99, 3 otherwise (DL21).

From the grid, each sector builds nine descriptors (one per landblock, `distant-landscape.md`
§5.2): present (covered), outside the near field, ceiling bit, LOD, and per edge (west,
south, east, north) a stitch flag and the neighbour's LOD. An edge gets a stitch flag when
the neighbour across it is covered, has a different LOD, and is either near or finer
(DL20).

Consequences with *N* = 1 (FV8, arithmetic):

- every landblock with *d*² ≤ 3 is in the near field, so **no far landblock has LOD 0**:
  the far field starts at LOD 1 (10-unit sample spacing);
- the near field's 3 × 3 landblocks are all LOD 0;
- two edge-adjacent landblocks never differ by more than one LOD (between neighbours *d*²
  changes by 2|*dx*| + 1 at most, which is at most 9 while *d*² ≤ 24 and at most 21 while
  *d*² ≤ 99, while skipping a level would need a jump of at least 76).

---

<a id="s5"></a>
## 5. Far-terrain mesh at run time

<a id="s5-1"></a>
### 5.1 Two batches; the near batch is not drawn (BIN, FV7)

Each sector's terrain mesh (and ceiling mesh) holds up to two batches built by
`distant-landscape.md` §5.3: the **far batch** with the present landblocks outside the near
field, and the **near batch** with the present landblocks inside it. The client rebuilds the
batches only when the sector's nine descriptors differ from the previous build.

When drawing, the client draws the far batch always and the near batch only when a switch of
the current scene description is on. The world view's scene description starts with that
switch off and the world renderer never sets it (no other writer was found in the world
renderer). So in the world view **the sector terrain covers exactly the far field**, and the
near field is drawn only by the full-detail terrain. The ceiling mesh follows the same rule.

Two debug switches of the client (on by default) can hide the sector terrain and ceiling
meshes; Tessera ignores them.

<a id="s5-2"></a>
### 5.2 Meeting the near field and LOD seams (BIN, FV8)

There are no skirts. Cracks are closed by the stitching triangles of
`distant-landscape.md` §5.3:

- A far landblock next to the near field always stitches toward it (the neighbour is near
  and has LOD 0 ≠ its LOD 1). Its edge triangles join each 2-sample segment of its own edge
  with the middle sample of that segment, which is a vertex of the near landblock's
  full-resolution edge. The sector samples are the landblock-data heights
  (`distant-landscape.md` §4.2), so both sides meet at the same points: the seam is closed
  without T-junctions on the far side.
- Between far landblocks of LOD *l* and *l* + 1 the coarser one stitches; its segment
  middles are vertices of the finer edge.
- Toward an uncovered neighbour (a missing sector or the map edge) no stitching is made; the
  edge stays open.

The stitching triangles are drawn with the far batch and its material, whose render state
culls back faces (FV23), so each is seen from one side only, the side set by its stored corner
order (`distant-landscape.md` §5.3); Tessera's Decision is FD10.

The far mesh ignores the vertex-flag diagonals of the near terrain (DL20). Inside the near
field nothing is shared, so the two triangulations differ only inside each landblock.

<a id="s5-3"></a>
### 5.3 Normals

The far mesh uses the stored sample normals (`distant-landscape.md` §4.3) without
recomputing them. The near terrain uses its own normals (`terrain-texturing.md` §4.10).
Lighting differences at the seam are hidden by the seam overlay (§6.5) when it is active.

---

<a id="s6"></a>
## 6. Far-terrain texturing, lighting and fog

<a id="s6-1"></a>
### 6.1 Which material (BIN + OBS, FV9)

The landscape manager looks three entries up in the UNIQUEDB DID map (root key 0xF,
`property-system.md` §8) by key number (the labels are tool data, quoted for
interoperability):

| Key (BIN) | Label (OBS) | Requested as | Observed target | Role |
|---|---|---|---|---|
| 0x39 | `Render_DistantLandscapeMaterialTemplate` | shader record | 0x2B0005ED | plain template |
| 0xB6 | `Render_DistantLandscapeMaterialTemplate_NormalMapped` | shader record | 0x2B000461 | normal-mapped template |
| 0x7A | `Render_DistantLandscapeDetailTexture` | texture | 0x40000002 | detail texture (parameter 3001) |

The **active template** is the normal-mapped one when the preference
`FarLandscapeNormalMaps` is on **and** the effective graphics-core setting is 2 or 5
(meaning of those values not traced); otherwise the plain one. A change of the active
template discards every sector and rebuilds them (§3.4). Read the DIDs at run time.

<a id="s6-2"></a>
### 6.2 Per-sector material (BIN, FV10, FV22)

Each terrain (ceiling) mesh gets a copy of the active template with parameter 3000 = the
sector's `colour_list` (`ceiling_colour_list`), 3001 = the detail texture of §6.1, and 4005
= 4006 = 0 (no wrap in U and V for the colour texture). Every other parameter keeps the
template's default (OBS):

| Parameter | Plain template | Normal-mapped template |
|---|---|---|
| 3000 colour, 3001 detail | as set above | as set above |
| 3011 normal texture | (none) | template default, a nine-level texture list (0x40017690 here) |
| Texture transform set 0 (`material.md` §8.4: 1001/1002 scale, 1017/1018 translation, 1033 rotation, 1070/1071 early translation) | scale 4, 4; others 0 | scale 1, 1; others 0 |
| Texture transform set 1 (1003/1004, 1019/1020, 1034, 1072/1073) | (none) | scale 4, 4; others 0 |

The detail texture of this install is a nine-level list whose top level is 1,024 × 1,024
DXT1. The sector's `tile` and `ceiling_tile` surfaces are not used by the far-terrain
material (OPEN, §15).

With the sector texture coordinates (*u*, *v*) = (sector *x* / 480, 1 − sector *y* / 480)
(`distant-landscape.md` §5.3), the plain template samples the colour list at (*u*, *v*) and
the detail texture at transform set 0, that is (4*u*, 4*v*): four detail repeats per sector,
one per 120 units. The normal-mapped template samples the colour list at set 0 (*u*, *v*) and
both the detail texture and the normal texture at set 1, (4*u*, 4*v*).

<a id="s6-3"></a>
### 6.3 Plain template: one per-vertex pass (OBS, FV11)

The plain template has one Direct3D 11 permutation, the per-vertex base pass 0x3E
(`lighting.md` §3.2). In words:

Per vertex, with *P* the world position, *n* the decoded sample normal (not renormalised),
*n*_up its vertical component and *d* = |eye − *P*|:

1. Light *Σ* = sat(*A* + *n*_up × *H*) + *k* × sat(*n* · (−*D*)) × *C*_D, where *A* and *H*
   are the material-scaled ambient and hemispheric ambient, *D* and *C*_D the direction and
   colour of the per-vertex directional light, and *k* the per-vertex directional-light count
   (0 or 1) (`lighting.md` §4.2).
2. Fog *f* = sat((*d* − fog near) / fog range) (`lighting.md` §10).
3. Far-draw fade *a* = 1 − sat((*d* − *s*) × *r*), with *s*, *r* the engine input
   `c_FarDrawFadeParams` (values OPEN, §7.2).

Per pixel:

4. Discard the pixel when *a* < 0.1.
5. *c* = (colour texel + detail texel − 0.5) × *Σ* (RGB).
6. Output RGB = (1 − *f*) × 2*c* + *f* × fog colour; output alpha = *a*.

Render state (OBS): source factor SRCALPHA, destination INVSRCALPHA, depth test "nearer or
equal", depth write on, back faces culled (`material.md` §7.6). No shadows and no point
lights reach the far terrain through this template. The material colour that scales *A* and
*H* in step 1 is the template's 2000, (1, 1, 1, 1) in both templates, which the per-sector copy
leaves unchanged (FV23).

<a id="s6-4"></a>
### 6.4 Normal-mapped template (OBS, FV11, FV12)

The normal-mapped template has Direct3D 11 permutations for the passes 0x3E (all per
vertex), 0x3F (sun per pixel) and 0x40 (point light per pixel); which pass the client draws
follows `lighting.md` §2. All three vertex programs compute, besides fog and the far-draw
fade *a* of §6.3:

- the horizontal distance *h* from the eye to *P* (world X and Y only);
- the **normal strength** *t* = sat((*h* − 80) / 160): 0 within 80 units, 1 beyond 240;
- the **near-view mask** *m*: with (*p*, *q*) = the horizontal position of *P* minus the
  engine input `c_NearViewOrigin`, *m* = sat(max(−*p* − 159, −*q* − 159, *p* − 319,
  *q* − 319)); *m* = 0 inside the square −159 ≤ *p*, *q* ≤ 319 and reaches 1 one unit outside
  −160 ≤ *p*, *q* ≤ 320;
- the **coverage** *w* = min(1, *m* + sat((*h* − 80) / 80) × min(1, *n*_up²)).

**Inferred:** `c_NearViewOrigin` is the south-west corner of the viewer's landblock, so the
mask square is the near field (§2). Far-batch vertices lie outside it (*m* = 1, *w* = 1).

Per pixel (all three passes): discard when *w* × *a* < 0.001; output alpha = *w* × *a*; RGB as
§6.3 steps 5–6, with the light of the pass. In pass 0x3F the light is computed per pixel:

1. Tangent-space normal *n′* = *t* × (2*R* − 1, 2*G* − 1, 2*B* − 2) + (0, 0, 1) from the
   normal texel (*R*, *G*, *B* in 0–1): the flat normal (0, 0, 1) near the viewer, the full
   map from 240 units on.
2. With the light vector *L* and the up vector *U* in the vertex's tangent frame (built per
   vertex from the vertex normal alone; no stored tangents): *e* = sat(*n′* · *L*),
   *e′* = sat(2*e* − 0.9), *μ* = the largest component of *L*,
   *e″* = *e* + *μ* × (*e′* − *e*).
3. *Σ* = sat(*A* + (*n′* · *U*) × *H*) + *e″* × sun colour.

In passes 0x3E and 0x40 the light is computed per vertex as in §6.3 (no normal map).

<a id="s6-5"></a>
### 6.5 Seam blending on the near terrain (BIN conditions, OBS formula, FV12)

`terrain-texturing.md` §12 describes an extra full-grid draw of each near-terrain chunk with
a copy of the sector's material ("sector overlay"). The landscape manager hands the sector
material to the near-terrain builder only while the preference `NearFarSeamBlending` is on
**and** the active template is the normal-mapped one (§6.1); `terrain-texturing.md` §12 adds
the lighting conditions. Toggling the preference rebuilds the near terrain.

Because the overlay draws near-field geometry (*m* = 0), its coverage is
*w* = sat((*h* − 80) / 80) × min(1, *n*_up²): invisible within 80 units horizontally, then
fading in to full at 160 units on level ground and less on slopes. Blended with SRCALPHA /
INVSRCALPHA over the layered near terrain, it turns the near terrain into the far image
before the near-field border, which lies at least 160 units away horizontally from any
viewer position. With the plain template (or the option off) there is no overlay and the
change from layered near terrain to the colour image is a hard edge at the near-field
border.

---

<a id="s7"></a>
## 7. Extent, fog and draw distances

<a id="s7-1"></a>
### 7.1 Landscape extent and the object limit (BIN, FV13)

The client computes *E* = 160 × (3 × *F* + *N*) units from the manager's radii. When a
landscape is drawn, the global object limit *G* of `walker.md` §11.3 starts as *E*, may be
lowered to the fog far distance, and is raised to at least 60 units. *E* is a fixed
summary; the real far-field edge lies between 160 × 3*F* and 160 × (3*F* + 2) units from the
viewer's landblock edge (§2).

<a id="s7-2"></a>
### 7.2 Fog and the far-draw fade (OBS formulas; values OPEN)

Far terrain, ceilings and distant sprites are fogged per vertex with the frame fog
(`lighting.md` §10), like other lit surfaces; nothing far-view specific changes the fog. They
are also multiplied by the far-draw fade *a* = 1 − sat((*d* − *s*) × *r*). The values the
client gives `c_FarDrawFadeParams` (*s*, *r*) were not traced (§15).

**Tessera Decision:** *s* = *E* − 160, *r* = 1/160 (the far terrain fades out over the
last landblock before *E*); with fog on and fog far < *s*, the fade is invisible anyway.

<a id="s7-3"></a>
### 7.3 Horizon fill (BIN, FV14)

Beyond the listed sectors the client draws no terrain. To hide the gap below the horizon it
draws a render mesh from UNIQUEDB key 0x95 (label `SkyGradientMesh`, DB type 0x38 render
mesh; 0x06000000 here) centred on the eye, during the frame's fog and sky set-up, whenever
the landscape manager has the mesh loaded. Its placement is recomputed each frame while a
landscape is drawn:

1. Floor *z*_f = min(*z*_ring, *z*_near) + 15, where *z*_ring is the lowest of, over the far
   landblocks on the outer boundary of the listed area (first and last row and column of the
   grid of §4), the water height when the chunk is wet and the chunk's `min` *z* otherwise,
   and *z*_near is the lowest bound of the near-field terrain chunks.
2. *E′* = 1.25 × *E*.
3. From the mesh's bounds: *r* = the larger of its maximum X and maximum Y, *H* = its height
   (max *z* − min *z*).
4. *δ* = max(0, eye *z* − *z*_f); *k* = *r* × *δ* / *E′*.
5. Place the mesh at the eye's horizontal position, shifted down by *k*, with its vertical
   scale (*H* + *k*) / *H*.

So the mesh reaches down to where the floor would appear at distance 1.25 *E*. Its own
surfaces and their fogging are `mesh.md`'s and `lighting.md`'s.

---

<a id="s8"></a>
## 8. Landscape water from the sectors (BIN, FV15)

A sector's water meshes (one per water-list pair, `distant-landscape.md` §5.5) are built once
when the record is first read, independently of the viewer, and are attached to the scene
whenever the sector is listed; they are detached with the rest of the sector (§3.5). There
is no near/far split and no LOD for water: **the sector quads are the landscape water at
every distance, the near field included.** This settles the OPEN of `water.md` §8.1 in
favour of its Tessera rule.

---

<a id="s9"></a>
## 9. Distant sprites (imposters)

<a id="s9-1"></a>
### 9.1 When sprites exist (BIN, FV16)

- The sprite painter exists only when the graphics device's class is at least 3 (device
  classes as in `terrain-texturing.md` §12); otherwise no sprites are ever drawn.
- Sprites are built only while the preference `DistantImposters` is on (built-in default
  on, `walker.md` §11.2); only then does the client load the sectors' imposter tables.
- The world renderer draws them only when two further conditions of the frame hold that
  were not identified (Inferred: not in secondary views such as reflections), and a batch
  only once its atlases are loaded.

<a id="s9-2"></a>
### 9.2 Which sprites (BIN)

At each rebuild (§3.6 step 5) the client takes every listed sector, the near field's sectors
included, and every sprite of it whose `name_key` is found in the sector's own imposter table
(`distant-landscape.md` §4.8, §5.6). Each sector with at least one such sprite forms one batch
with that table's two atlases (`misc-records.md` §5.3). Positions are kept relative to the
south-west corner of the viewer's landblock. Within a batch the client orders the sprites by
squared horizontal distance from a fixed point and keeps the batches ordered by distance
(exact order OPEN, §15).

<a id="s9-3"></a>
### 9.3 Fade-in distance (BIN + OBS, FV17, FV18)

Each sprite carries a distance parameter *D*. With (*x*, *y*) its horizontal position
relative to the south-west corner of the viewer's landblock:

- **Near window** (−160 ≤ *x* ≤ 320 and −160 ≤ *y* ≤ 320, the near field): *D* = the
  record's `distance`.
- **Elsewhere:** *D* = *S* × *P* / (*φ* × 248) / *M_d*, where *S* = `value_a` × `scale` is
  the quad size (`misc-records.md` §5.2), *P* the render target's height in pixels
  (Inferred), *φ* the camera's field of view in radians (the client converts its degree
  value with π/180; Inferred: the vertical field of view, `walker.md` §11.7), and *M_d* the
  `ObjectDrawDistance` multiplier of `walker.md` §11.2.

The sprite vertex program (`misc-records.md` §5) multiplies *D* by the second component *c*
of `c_DistantSpriteConfig`, which the client sets to the object draw-distance scale: *c* =
crowd factor × *M_d*. The crowd factor is 1.0 unless many players are near (it follows the
player count and `PlayerCrowdQuality`; details OPEN in `walker.md` §11.9). With *d* the
distance from the eye to the sprite's anchor, the sprite's opacity factor is

  sat((*d* − (*D* × *c* − 25)) / 10) × (1 − sat((*d* − *s*) × *r*)),

the second factor being the far-draw fade of §7.2. It multiplies the atlas coverage before
the alpha test (`misc-records.md` §5.3).

The sprite material's state (FV24): alpha test on with the block's reference 0.1 (the sprite
records declare no 1043), blended SRCALPHA / INVSRCALPHA, depth test "nearer or equal", depth
write on, drawn two-sided. The per-draw rules of `material.md` §11.2 hand the programs the
reference ⌊0.1 × 255⌋ / 255 = 25 / 255 (with depth write on, scaled by the material and tint
alpha, which are 1 for sprites), so a sprite texel is kept when its faded coverage exceeds
about 0.099 (the program's 0.001 margin included).

Consequences:

- In the near field the sprite starts to appear 25 units before *D* × *c* and is complete
  15 units before it. The real object of LOD class `d0` is drawn up to min(*G*, `d0` × *M_d*)
  and fades over 1.5 s when crossing it (`walker.md` §11.4). Where *D* = `d0` (§9.4) the
  sprite fades in shortly before the object disappears: a cross-fade by distance.
- Outside the near field *M_d* cancels: a sprite appears where its projected height drops
  below about 248 pixels (25 units earlier). Since every point outside the near window is at
  least 160 units from the viewer, nearly every far sprite is fully visible.

<a id="s9-4"></a>
### 9.4 What the sprites stand for (OBS, FV19; Inferred meaning)

Over all 3,549,236 sprites, 801 of the 818 imposter keys used always carry the same
`distance`; 17 keys carry two values. The seven values used (45, 80, 140, 180, 200, 300 and
1,000) are all `d0` values of LOD classes in use (`walker.md` §11.4.1: MediumDecor 45,
LargeDecor and Bush 80, Tree 140, SmallStructure 180, the no-class value 200,
LargeStructure 300, Landmark 1,000). **Inferred:** each sprite stands for one placed or
generated object of the near-field data, and its `distance` is that object's draw distance.
Which objects receive sprites is decided by the data, not by the client.

<a id="s9-5"></a>
### 9.5 Orientation and mirror (BIN, FV20)

The sprite's last byte *b* (`distant-landscape.md` §3.7, "meaning OPEN") is a heading:
*θ* = *b* × 360 / 256 degrees. At each rebuild the client sets the sprite's mirror flag
(`misc-records.md` §5.4) when

  (*x* − 160) × sin *θ* + (*y* − 160) × cos *θ* > 0,

with (*x*, *y*) as in §9.3. The flag is not recomputed while the viewer stays in its
landblock. The quad always faces the camera (`misc-records.md` §5.2); the heading only picks
the mirrored or unmirrored atlas image.

<a id="s9-6"></a>
### 9.6 Lighting and fog

As `misc-records.md` §5.3: atlas albedo and tangent-space normals lit by the fake
directional light, the ambient and the hemispheric ambient, then the distance fog. The
sprite colour is doubled before fogging like the terrain (§6.3 step 6).

---

<a id="s10"></a>
## 10. Far entities from the sector lists (BIN, FV21)

The entity streaming manager of `entity-descriptor.md` §9.3 creates objects only for the 3 ×
3 near landblocks. The sector entity lists extend that range for long-range objects:

1. At each rebuild, for each listed sector, every entity entry (`distant-landscape.md` §3.8)
   is added by instance ID to the manager's **distant set** (an entry already present is
   kept). For a sector that is no longer listed, its entries are removed and their entities
   destroyed. (The client skips this step under one combination of global client-mode flags
   that was not identified; normal play is not affected, Inferred.)
2. After the rebuild the manager walks the distant set. If the viewer's instance changed
   since the last walk, it first removes every distant entity.
3. For each entry, with *d*² the squared 3D distance from the viewer position to the entry's
   Position (across landblocks): when `view_distance_sq` × *s* ≤ *d*² the entity is removed
   (*s* the manager factor of `entity-descriptor.md` §9.3, 1.0 at construction); otherwise,
   if no entity with that instance ID exists through the main or long-range sets, the
   entity is created from the entry (object DID, instance ID, scale, Position, property);
   if one exists, the existing one is kept.

The entries are the sectors' copies of long-range placements (`distant-landscape.md` §4.9,
view distances from 250 to 8,000 units; the most frequent are 1,000, 2,000, 3,000, 2,800 and
4,000, OBS). How an entity created this way is drawn is the normal path (`walker.md` §11.4:
draw distance min(*G*, `d0` × *M_d*)), so a long-range landmark appears when it is both
within its view distance and within its LOD-class draw distance.

---

<a id="s11"></a>
## 11. Transitions (summary)

| Transition | What happens in the original | Where |
|---|---|---|
| Viewer crosses a landblock boundary | Near field shifts by one landblock (new full-detail landblocks are built, old ones dropped); LOD grid and stitching recomputed; sprite windows and mirror flags recomputed; distant entities re-checked | §3.4, §4, §9 |
| Near terrain → far terrain | At the near-field border: shared edge heights, stitched far edge; colour change hidden by the seam overlay (normal-mapped template and `NearFarSeamBlending` only) | §5.2, §6.5 |
| LOD change of a far landblock | Instant (no morphing) when the viewer crosses a landblock boundary | §4 |
| Normal-map detail | Fades in from 80 to 240 units horizontally | §6.4 |
| Real object → sprite | Sprite fades in over 10 units ending 15 units before *D* × *c*; object fades out over 1.5 s at its draw distance | §9.3 |
| Long-range entity | Created inside its view distance, removed outside it or when its sector leaves the list | §10 |
| Edge of the landscape | Far-draw fade to the far edge (values OPEN); horizon fill mesh below the horizon; fog | §7 |
| Teleport or region change | Every sector discarded and rebuilt | §3.4 |

---

<a id="s12"></a>
## 12. Tessera Decisions

| # | Decision | Reason |
|---|---|---|
| FD1 | Far-draw fade *s* = *E* − 160, *r* = 1/160 (§7.2) | Original values not traced |
| FD2 | Keep unlisted sectors cached while they lie within *F* + 1 sector keys of the viewer's sector; free them beyond that (§3.5) | The client's eviction rule was not found |
| FD3 | Build sectors, batches and sprite data on background workers; swap per sector (§3.6) | The client time-slices on its main thread; Tessera's frame budget forbids stalls |
| FD4 | Draw sprite batches far to near; inside a batch, no sort is needed for the alpha-tested parts (§9.2) | The client's sort reference was not identified |
| FD5 | Tangent frame for the far normal map: tangent = world east made orthogonal to the vertex normal, bitangent = normal × tangent (§6.4) | The exact frame of the program was not reduced to words; the normal map is a tiled detail map |
| FD6 | *P* and *φ* of §9.3 are the walker camera's render-target height and vertical field of view | Identity of the two client values Inferred |
| FD7 | Apply the content-layer test of `entity-descriptor.md` §4.5.1 to distant-set entries, using the entry's own `Entity_ContentLayers` (§10) | Whether the client tests layers for these entries was not traced |
| FD8 | Never draw the near batch in the world view; use the near batch only for tools (§5.1) | Matches the original's world view |
| FD9 | The walker may keep its own far band (`walker.md` §14.4) until this spec is implemented; when implemented, the near field is 3 × 3 landblocks as here, unless the owner chooses a larger full-detail ring as an enhancement (then the far mesh must skip those landblocks and stitch to them by the same rule) | Fidelity versus look; owner choice |
| FD10 | Draw the stitching triangles with face culling off (§5.2) | The client culls back faces (FV23); a sliver seen from its back then shows a gap in the original. Two-sided slivers close every crack and cost almost nothing |
| FD11 | With a full-detail ring larger than *N* = 1 (FD9 enhancement): the far mesh treats a near neighbour as LOD 0 and fans each 2-sample segment over every neighbour edge sample inside it (the client's single triangle when the neighbour is one LOD finer); the walker meshes every full-detail landblock that borders a far landblock, or one about to become far, at its finest level and leaves the far neighbour out of its own seam matching, so the edge carries every sample the fan joins | One triangle per segment would leave T-junctions against a coarser full-detail edge; the case cannot arise with *N* = 1 (§4) |
| FD12 | With an explicit near set (the walker's), the landblocks of the set count as near inside the grid's rectangle even when their sector has no record, and their far neighbours stitch toward them; the radius form (tools, tests) keeps the §4 rule | §4 calls a landblock near only when a listed, readable sector covers it; the walker's full-detail terrain exists without a sector record, and an open edge there would show a crack |
| FD13 | Hand-off per landblock: the full-detail terrain (and ceiling) and the landblock water skip exactly the landblocks that the far batches on the GPU cover, and only those landblocks get sector water (§8) from the far view; a near-field landblock whose full-detail mesh is not on the GPU yet is left out of the near set, so the far batch draws it until the mesh arrives; a sector's terrain batch is drawn only once its colour list is loaded or has failed | FD3 swaps work per sector; this keeps every landblock drawn exactly once while the near field changes |
| FD14 | Draw order in the world pass: terrain layers, instanced scenery, sky, far view, water. The far view comes after the sky so that the far-draw fade (§6.3 step 3, §7.2) and the far edge blend with the sky, not with the clear colour | The client's order was not traced; `render.md` §5.1 does not list the far view yet (note for its owner) |
| FD15 | Walker with `--far-view on`: without `--view-blocks` the full-detail terrain, landblock water and static objects are streamed within 2 landblocks (320 units, which holds the 3 × 3 from any position in the viewer's landblock) and the terrain is drawn for the 3 × 3 near field only; `--view-blocks` keeps a larger ring (FD9, FD11). The landscape extent for the global object limit and the fog (§7.1) is *E* either way, and the far clip plane covers *E*. The walker's viewer instance is always 0, so the instance rules of §3.4 and §10 step 2 never fire | Walker settings, not client facts |

**Implementer notes of 2026-10-08, answered 2026-10-09 (spec writer).**

> **Answered (was OPEN, implementer):** which side of a stitching triangle faces the viewer and
> whether the client culls them. The far-terrain material culls back faces (FV23), so the
> client draws each sliver from one side only, set by its stored corner order; Tessera's
> two-sided slivers are Decision FD10.

> **Answered (was OPEN, implementer, corrected 2026-10-09):** stitching to a larger
> full-detail ring: Decision FD11 (the fan and the walker's finest-level edge).

> **Answered (was OPEN, implementer):** far landblocks next to a full-detail landblock whose
> sector has no record: Decision FD12 (explicit near set). Whether the original ever has such a
> pair inside its 3 × 3 was not checked; with the §4 rule its edge would stay open.

> **Answered (was OPEN, implementer):** what is drawn while the near field changes: Decision
> FD13 (hand-off per landblock).

> **Answered (was OPEN, implementer):** the draw order against the sky: Decision FD14.

> **Answered (was OPEN, implementer):** walker settings with `--far-view on`: Decision FD15.

> **Answered (was OPEN, implementer):** the material colour that scales *A* and *H* (§6.3
> step 1) is the template's 2000, (1, 1, 1, 1) in both templates (FV23); Tessera's 1 is the
> client's value.

> **Answered (was OPEN, implementer):** the distant-sprite material's alpha-test threshold and
> blend and depth state are data (FV24, §9.3): reference 0.1 (handed as 25 / 255), SRCALPHA /
> INVSRCALPHA, depth write on, two-sided. Tessera's threshold 0.5 departs from it and is
> replaced by 0.1; batches stay far to near (FD4).

**Implementation status (2026-10-09; gaps, not questions).** Not implemented yet, recorded so
that they stay visible: the normal-mapped template (§6.4; the walker always uses the plain one
and does not read `FarLandscapeNormalMaps` or the graphics core); the seam overlay (§6.5, which
needs that template); the flow-map material copy of the sector water (`distant-landscape.md`
§5.5, parameter 3075; the far water uses the water type's surface as it is); drawing the far
entities of §10 (the distant set is kept and walked, nothing is created). The plain template's
detail repeats are read from its declared defaults of 1001 and 1002 (§6.2), 4 × 4 when the
record is missing.

> **OPEN:** whether the ceiling mesh uses the terrain descriptors' stitch flags (neighbours
> without a ceiling included) or flags of its own. Not traced. **Tessera Decision:** the same
> nine descriptors serve both meshes.

> **OPEN:** the fake directional light's "default direction" when the main light is not
> directional (`lighting.md` E18). Not traced. **Tessera Decision:** sprites are lit from
> straight above with grey 0.6.

> **OPEN:** §7.3 step 5 places the horizon mesh "at the eye's horizontal position, shifted
> down by *k*" without saying from which height or whether it writes depth. Not traced.
> **Tessera Decision:** from the eye's height; the walker draws it only when its horizontal
> radius is at least 1.25 *E*, so that a depth-tested mesh never hides listed terrain.

---

<a id="s13"></a>
## 13. Invariants

| # | Invariant |
|---|---|
| FV-I1 | For the five `LandscapeDrawDistance` levels, *F* = 1, 2, 3, 4, 5 and (with *N* = 1) *E* = 640, 1,120, 1,600, 2,080, 2,560. |
| FV-I2 | The sector list holds at most (2*F* + 1)² keys, every key in 0–84 on both axes, and every listed key's corner landblock lies within 3*F* + 2 landblocks of the viewer's landblock on each axis. |
| FV-I3 | Every covered landblock with max(\|*dx*\|, \|*dy*\|) ≤ 1 is near; with *N* = 1 every far landblock has LOD ≥ 1. |
| FV-I4 | Edge-adjacent covered landblocks differ by at most one LOD. |
| FV-I5 | The far batches of all listed sectors and the full-detail near field cover the union of the covered landblocks and the near field, with no landblock drawn by both and none missing (holes only where no sector exists). |
| FV-I6 | A far landblock of LOD *l* adjacent to a finer or near neighbour adds 32 / 2^*l* stitching triangles on that edge; adjacent to a coarser far neighbour, none. |
| FV-I7 | The near-view mask *m* is 0 for every point with −159 ≤ *p*, *q* ≤ 319 and 1 for every point with *p* or *q* outside −160…320. |
| FV-I8 | Seam-overlay coverage is 0 for *h* ≤ 80, equals min(1, *n*_up²) for *h* ≥ 160, and never exceeds 1. |
| FV-I9 | The normal strength *t* is 0 for *h* ≤ 80 and 1 for *h* ≥ 240. |
| FV-I10 | A near-window sprite's opacity factor is 0 for *d* ≤ *D* × *c* − 25 and (before the far-draw fade) 1 for *d* ≥ *D* × *c* − 15. |
| FV-I11 | The mirror flag depends only on the byte, the sprite position and the viewer's landblock; it does not change while the viewer stays in one landblock. |
| FV-I12 | Every distant entity exists only while its sector is listed and *d*² < `view_distance_sq` × *s*. |
| FV-I13 | Data (Confirmed 2026-10-08): UNIQUEDB keys 0x39 and 0xB6 name existing shader records, key 0x7A an existing texture list, key 0x95 an existing render mesh; the plain template declares parameters 3000, 3001, 4005, 4006 and transform set 0 with scale 4; the normal-mapped one also declares 3011 and transform set 1 with scale 4. |

---

<a id="s14"></a>
## 14. Worked examples (synthetic)

<a id="s14-1"></a>
### 14.1 Resident set at the built-in setting

`LandscapeDrawDistance` High: *R* = 9, *F* = 4, *N* = 1, *E* = 2,080. Viewer in region 1,
landblock (100, 50), so sector key (33, 16).

- Listed keys: *sx* 29–37, *sy* 12–20 (81 keys, those with records); landblocks X 87–113,
  Y 36–62.
- Near field: X 99–101, Y 49–51 (full detail).
- From the viewer's landblock the far field reaches 13 landblocks west (to X 87), 13 east
  (to X 113), 14 south (to Y 36) and 12 north (to Y 62); *E* corresponds to 13.

<a id="s14-2"></a>
### 14.2 LOD and stitching

| (*dx*, *dy*) | *d*² | Zone | LOD | Note |
|---|---|---|---|---|
| (0, 0), (1, 1) | 0, 2 | near | 0 | full-detail terrain, not drawn from the sector |
| (2, 0) | 4 | far | 1 | stitches its west edge to (1, 0): 16 triangles |
| (4, 2) | 20 | far | 1 | |
| (5, 0) | 25 | far | 2 | stitches its west edge to (4, 0), LOD 1: 8 triangles |
| (9, 4) | 97 | far | 2 | |
| (10, 0) | 100 | far | 3 | stitches its west edge to (9, 0) (*d*² 81, LOD 2): 4 triangles |

<a id="s14-3"></a>
### 14.3 A far-terrain texel (plain template)

Colour texel (0.40, 0.35, 0.30), detail texel (0.5, 0.5, 0.5), light *Σ* = (0.6, 0.6, 0.6),
fog *f* = 0.25 toward (0.70, 0.75, 0.80), *a* = 1:
*c* = (0.40, 0.35, 0.30) × 0.6 = (0.24, 0.21, 0.18); 2*c* = (0.48, 0.42, 0.36); output =
0.75 × (0.48, 0.42, 0.36) + 0.25 × (0.70, 0.75, 0.80) = (0.535, 0.5025, 0.47), alpha 1.

<a id="s14-4"></a>
### 14.4 Normal strength and seam overlay

A near-terrain vertex at *h* = 120 units with *n*_up = 0.9: *t* = sat(40 / 160) = 0.25;
overlay coverage = sat(40 / 80) × min(1, 0.81) = 0.5 × 0.81 = 0.405. At *h* = 200 on the
same slope: coverage 0.81, *t* = 0.75.

<a id="s14-5"></a>
### 14.5 Sprite fade

Tree sprite in the near window, `distance` 140, *M_d* = 1, crowd factor 1: invisible
closer than 115 units, complete from 125 units; the tree object (class `d0` 140) is drawn to
140 units. With `ObjectDrawDistance` UltraHigh (*M_d* = 2): 255 to 265 units, object to 280.

Far sprite: *S* = 12 units, *P* = 1,080 pixels, *φ* = 45° = 0.7853982 rad: *D* × *c* =
12 × 1,080 / (0.7853982 × 248) = 66.54 units, so it starts at 41.54 units and is complete
at 51.54: always fully visible at the 160 units or more where far sprites lie.

<a id="s14-6"></a>
### 14.6 Mirror

Byte 64: *θ* = 90°. A sprite at (200, 40) relative to the viewer landblock's south-west
corner: (200 − 160) × 1 + (40 − 160) × 0 = 40 > 0, mirrored. Byte 0: *θ* = 0°, value
(40 − 160) × 1 = −120, not mirrored.

<a id="s14-7"></a>
### 14.7 Horizon fill

*E* = 2,080, so *E′* = 2,600. Outer-ring floor 120, near minimum 140: *z*_f = 135. Eye at
*z* = 185: *δ* = 50. Mesh with *r* = 1,000 and *H* = 500: *k* = 1,000 × 50 / 2,600 =
19.23; the mesh is shifted down by 19.23 and scaled vertically by 519.23 / 500 = 1.0385.

---

<a id="s15"></a>
## 15. Open questions

> **OPEN:** The values the client gives `c_FarDrawFadeParams` (start *s* and inverse range
> *r* of the far-draw fade, §7.2). Tessera Decision FD1 until known. A frame capture of the
> original (M3, owner) would settle it.

> **OPEN:** The rule that frees cached, unlisted sectors (§3.5). Tessera Decision FD2.

> **OPEN:** The time budget per update call (§3.6). Tessera Decision FD3.

> **OPEN:** The use of the sector's `tile` and `ceiling_tile` surfaces (still not read by the
> far-terrain material, the water or the sprites; `distant-landscape.md` §10).

> **OPEN:** The exact sprite sort order (the fixed reference point of the in-batch sort and
> the batch order, §9.2). Tessera Decision FD4.

> **OPEN:** Whether the client applies a content-layer test to distant-set entries, and
> what sets the manager factor *s* other than its construction (§10). Tessera Decision FD7.

> **OPEN:** The identity of `c_NearViewOrigin` (Inferred: the viewer landblock's south-west
> corner), of *P* and *φ* in §9.3 (Inferred: render-target height and vertical field of
> view), and of graphics-core values 2 and 5 (§6.1).

> **OPEN:** Whether any renderer other than the world view (for example reflections or a map
> view) turns on the near-batch switch (§5.1).

> **OPEN:** Which objects of the data the sprites stand for, record by record (§9.4); a probe
> matching sprite positions with near-field placements and generated scenery
> (`scenery.md`) would settle it.

> **OPEN:** The crowd factor of §9.3 (player counts, `PlayerCrowdQuality`), owned by
> `walker.md` §11.9.

---

<a id="s16"></a>
## 16. Answers to OPEN items in other specs (for their owners to adopt)

This spec edits no other spec except one cross-reference line in `distant-landscape.md`.

| Spec | Item | Answer here |
|---|---|---|
| `distant-landscape.md` §10 | Values of *N* and *F*; what the near-batch flag changes | §3.1 (*N* = 1, *F* = ⌊*R* / 3⌋ + 1), §5.1 (near batch drawn only under a scene switch that the world view leaves off) |
| `distant-landscape.md` §10 | Parameter 3001 and the normal-map source | §6.1, §6.2: 3001 is the detail texture of UNIQUEDB key 0x7A; the normal map is the normal-mapped template's own default 3011, not `tile` |
| `distant-landscape.md` §10 | Sprites: `distance`, `byte`, per-sprite flag | §9.3 (fade-in distance), §9.5 (byte = heading; flag = mirror) |
| `distant-landscape.md` §10 | Far entities: what the client does with the list | §10 |
| `distant-landscape.md` §5.2 | Use of the outer-ring minimum height | §7.3 (horizon fill floor) |
| `walker.md` §11.3 | OPEN: near ring meshing, which landblocks' static objects load | §2, §3.2: full detail only for 3 × 3 landblocks, everything else from sectors at LOD 1–3; static objects for the same 3 × 3 plus long-range entities from the sector lists (§10) |
| `walker.md` §11.9 | OPEN: `DistantImposters` | §9 |
| `walker.md` §14.11 | OPEN: the original's far landscape and seam | §5, §6, §11 |
| `terrain-texturing.md` §12 | OPEN: what the sector overlay contributes, and when the renderer holds a sector material | §6.5 (coverage formula, conditions); "Reported" bands of 14 and 24 blocks are the freecam tool's overrides (`walker.md` W19), not the client's |
| `water.md` §8.1 | OPEN: whether the sector quads are used at every distance | §8: yes |
| `render.md` §5.1 | The world pass's draw order does not list the far view | FD14: terrain layers, instanced scenery, sky, far view, water |
| `misc-records.md` §18 | Proposed answer on sprite `distance` and the flag | Confirmed and completed by §9.3, §9.5 |

---

<a id="s17"></a>
## 17. Change log

| Date | Change |
|---|---|
| 2026-10-08 | First version (Review): radii and settings, near field, sector list and its rebuild rules, per-landblock grid, far batches and the near-batch rule, stitching against the near field, far-terrain materials (templates, detail texture, normal maps, shading, fade, near-view mask), seam overlay, extent and horizon fill, sector water at every distance, distant sprites (gates, fade-in distances, cross-fade, heading and mirror), far entities, transitions, Tessera Decisions, invariants, worked examples, OPENs (BIN-F, OBS-F). |
| 2026-10-09 | Spec writer (dirty room); status Accepted → Revised, awaiting review. §12: the implementer notes of 2026-10-08 answered; new Tessera Decisions FD10 (two-sided stitching triangles), FD11 (stitching to a larger full-detail ring), FD12 (explicit near set), FD13 (hand-off per landblock), FD14 (far view after the sky), FD15 (walker settings); the gaps recorded as an implementation status; three notes stay OPEN with Decisions (ceiling stitch flags, fake light default direction, horizon mesh height and depth). New data facts (OBS-F2): FV23 (templates: 2000 = (1, 1, 1, 1), back-face culling, no 4003 or 4004), FV24 (sprite records: alpha test on, reference 0.1, alpha blend, depth write, two-sided). §5.2, §6.3 and §9.3 cite them; §16 gains a row for `render.md` §5.1. |
