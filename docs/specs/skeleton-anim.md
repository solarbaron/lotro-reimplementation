<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Skeletons in use: bone mapping, skinning, conventions, sampling, root motion, attach points

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/anim` (pose evaluation, track-to-bone binding, blending, root motion, attach transforms, skinning palettes), `crates/render` (skinned pipeline, palette size), `crates/formats` (setup attach tables, `setup.md` §6.2–§6.3), `apps/explorer` (character and animation viewer), `apps/client` (animated avatar) |
| **Companion specs** | [`havok-data.md`](havok-data.md) (normative for the skeleton and clip containers, their decoding and the curve evaluation of §7 there; this spec starts from decoded skeletons and decoded clip samples). [`setup.md`](setup.md) (the 0x04 setup that carries the skeleton, its attach tables §6.2–§6.3, and part 2 §11–§14 on character setups). [`mesh.md`](mesh.md) §5.7 and §6.2 (vertex bone indices and the per-submesh influence list), §11 (axes). [`property-system.md`](property-system.md) §7–§8 (enum mappers, DID maps). `appearance.md` (M4, species and body assembly, bone sliders, worn parts). `script-table.md` (M4, which clip an entity plays when). |

---

<a id="s0"></a>
## 0. Purpose

`havok-data.md` says how to decode a skeleton and how to sample a clip into per-track
(translation, rotation, scale) values. This spec says what those numbers **mean** once an
object is on screen:

- which skeleton an object or a playable character uses (§2);
- which bone each clip track drives, and which clips fit which skeleton (§3);
- what mesh bone indices address, the per-submesh influence list, palettes and inverse-bind
  matrices (§4);
- spaces, axes, quaternion order and direction, and the order in which bone transforms
  compose (§5);
- frame rate, looping, interpolation and blending of clips, and how partial clips are
  marked (§6);
- root motion: which bone moves, in-place clips, extracted motion, locomotion speed (§7);
- attach points: the semantic bone table, the held-item frames, and how a held item is placed
  (§8);
- how the parts of a multi-part character bind to one skeleton (§9);
- the evaluation pipeline in words, invariants, census, corrections, synthetic examples and
  open questions (§10–§15).

All counts are observations of one install on one date. They change with patches. Tests
assert the invariants (§11), not the counts. DIDs quoted below are observations for tests
and censuses; implementations find records at run time through the DID maps and properties
named here, never through hard-coded DIDs.

Enum labels (`HoldingLocation`, `BoneType`, `Wpn_RHand` …) and a few bone-name tokens appear
because they are internal identifiers stored in the data (enum mappers, skeleton bone
arrays) and are what an implementation would display for debugging. No game text appears.

---

<a id="s1"></a>
## 1. Provenance

Legend:

- **Confirmed**: decoded or measured on the user's own install (Steam, data current on
  2026-10-04/05) by throwaway probes in the dirty-room scratchpad. A Confirmed fact states
  what was measured and how much of it.
- **Reported**: comes only from third-party material or analogy and was not, or could not
  be, checked against data.

| Tag | Source |
|---|---|
| OBS | Observed in the user install, 2026-10-05. **All 28,555** 0x04 setups (skeleton, attach map, three frame lists, parts) and **all 25,395** 0x05 clips decoded; the frame-0 and last-frame pose of every clip computed (every spline block parsed); **all 43,059** skinned submeshes of all 47,563 mesh records read (28,635,289 vertices); all 735 enum mappers and 24 DID maps; the race table, player-avatar objects, inventory-control object and all 383,332 item property sets scanned with the repository's own property decoder (used as a tool). Forward kinematics and linear-blend skinning computed with scratch code. |
| HD | `havok-data.md` (skeleton members, reference-pose layout, clip envelope, track weights, reference frames, sampling algorithms). |
| SET | `setup.md` §6 (setup layout, frame layout with (w, x, y, z) rotations). |
| MV | Mirarkitty `lotro_mesh_viewer`, `docs/animation.md` and `docs/weapons.md`: positional binding by track count, player rig bone counts, attach bones and hand-tuned held-item offsets, gait-based identification of walk clips. Facts only. |
| ARCH | `docs/ARCHITECTURE.md` §11.2 ("tracks map to bones by position, Reported"). |
| PS | `property-system.md` §7 (enum labels), §8 (DID maps), §9 (DB-object types). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| K1 | An object's skeleton is the tagfile inside its own 0x04 setup; nothing names or shares a skeleton by reference (§2.1) | HD; OBS (28,555 setups) | Confirmed |
| K2 | A playable character's setup is found through the race table, the player-avatar object, `PhysObj` and the entity chain (§2.2) | OBS (all 15 gender entries of the 8 species) | Confirmed. MV's provisional Man/Elf assignment is confirmed. |
| K3 | Clip track *j* drives skeleton bone *j*; a clip fits a skeleton exactly when its track count equals the bone count (§3) | MV, ARCH, HD H16; OBS (frame-0 pose agreement on 24,839 clips, control test, 1,908 name-matched clips) | Confirmed (was Reported) |
| K4 | Mesh vertex bone indices address the bone array of the skeleton the mesh is drawn with; no remap exists (§4.1) | MV; OBS (2,625 setup submeshes, 116,768 worn-part checks) | Confirmed |
| K5 | The per-submesh influence list is redundant: per vertex in reverse order, the vertex's first bone index, once per non-zero weight (§4.2) | OBS (43,059 of 43,059) | Confirmed. Corrects MV ("palette that does not parse reliably"). |
| K6 | Inverse-bind matrices come from the skeleton reference pose; mesh vertices are stored in the reference-pose model space (§4.4–§4.5) | MV; OBS (692 setups, rigid-vertex proximity with control; skinning tests) | Confirmed |
| K7 | Skeleton space = mesh space: Z up, metres, right-handed, characters face +Y; no axis conversion (§5.1) | OBS | Confirmed |
| K8 | Reference-pose and clip quaternions are stored (x, y, z, w) and rotate actively; local = translate · rotate · scale; world = parent world · local (§5.2–§5.3) | HD (layout); OBS (arm/foot test, skinning tests, box bounds of `setup.md`) | Confirmed. Scale inheritance OPEN (§5.4). |
| K9 | Clips carry no loop flag; 56 % of clips end on their first pose (§6.3) | OBS | Confirmed (statistic); looping choice belongs to `script-table.md` |
| K10 | The envelope weight array marks partial (layered) clips, typically upper-body only; no additive clips exist (§6.6) | HD (array); OBS (pattern census) | Confirmed by pattern, meaning partly Reported |
| K11 | Bone tracks are in place in 84 % of clips; locomotion cycles are treadmill clips; 112 clips carry extracted motion whose w is a yaw angle (§7) | HD; OBS | Confirmed |
| K12 | `attach_map` maps `BoneType` enum keys to bone indices (§8.1) | OBS (378 setups, 5,812 + 559 elements) | Confirmed. Resolves `setup.md` §6.2 OPEN. |
| K13 | Frame list A maps `HoldingLocation` enum keys to (bone, bone-local frame); it holds every held-item, sheath, effect and target point (§8.2) | OBS (2,737 setups, 25,118 elements) | Confirmed. Corrects MV ("attach transforms are not in the data"). |
| K14 | Frame list C is keyed by `UIViewport_Type` (§8.7) | OBS (54 elements) | Confirmed (key); use Reported |
| K15 | Held items are placed with their setup origin at the holding-location frame; weapons are modelled grip at origin, long axis +Y (§8.4) | MV; OBS (1,591 weapon setups) | Confirmed |
| K16 | Mesh `flags` (mesh.md §3.3) is a `MeshTypeID` enum value: the body or equipment slot of the part (§9.2) | OBS (47,563 of 47,563 records; 80,974 of 81,098 worn parts) | Confirmed |

---

<a id="s2"></a>
## 2. Which skeleton an object uses

<a id="s2-1"></a>
### 2.1 Rule (Confirmed)

**An object's skeleton is the Havok tagfile embedded in the 0x04 setup the object resolves
to** (`setup.md` §3, §6.1 field 3; decoded by `havok-data.md` §4.2). There is no other
source:

- no skeleton record type exists apart from the tagfile inside each setup (`havok-data.md`
  §2.1);
- `hkaSkeleton.name` is absent in all 28,555 skeletons, no clip names a skeleton, and no
  binding, skeleton-mapper or container object exists (`havok-data.md` §4.6);
- no property, template or setup field names "the skeleton of another setup".

Identical skeletons are **stored repeatedly**, once per setup. Grouping skeletons by parent
list and reference pose (to 1e-4) gives 923 distinct skeletons among 28,555 setups, and 191
distinct ones among the 293 setups with 30 or more bones (154 occur once, the rest 2 to 20
times). For example the four male 89-bone player setups of §2.2 carry bit-identical
reference poses. An implementation may deduplicate skeletons by content (a hash of the
tagfile bytes or of the decoded arrays) to share decoded clips and palettes, but that is an
optimisation, not a format rule.

<a id="s2-2"></a>
### 2.2 Playable characters (Confirmed)

The path from a playable species and sex to its setup, resolved at run time:

1. The root DID map's `WEENIECONTENT` map (`property-system.md` §8) has the label
   `RaceTableDirectory`; its value is a game object (0x7000020F here) whose property set
   (+0x09000000 rule) holds `RaceTable_RaceTableList`: one game object per species.
2. Each species object has `RaceTable_Race` (a `SpeciesType` enum key) and
   `RaceTable_GenderList`, an array of `RaceTable_GenderStruct` with `RaceTable_GenderType`
   (0x1000 male, 0x2000 female) and `RaceTable_Gender_PlayerAvatar` (a game object).
3. The player-avatar object's property `PhysObj` names a 0x47 entity; the entity chain of
   `setup.md` §3.1 leads to a 0x1F template and then to the 0x04 setup.

Observed resolution (all 15 gender entries of the 8 species; the Dwarf species has one):

| Species key | `SpeciesType` label | Sex | Entity | Template | Setup | Bones |
|---|---|---|---|---|---|---|
| 0x51 | Hobbit | male | 0x47000606 | 0x1F000586 | 0x04000551 | 62 |
| 0x51 | Hobbit | female | 0x47000607 | 0x1F000587 | 0x04000552 | 62 |
| 0x17 | Human | male | 0x4700060A | 0x1F000588 | 0x04000553 | 89 |
| 0x17 | Human | female | 0x4700060B | 0x1F000589 | 0x04000554 | 89 |
| 0x41 | Elf | male | 0x4700060E | 0x1F00058A | 0x04000555 | 89 |
| 0x41 | Elf | female | 0x4700060F | 0x1F00058B | 0x04000556 | 89 |
| 0x49 | Dwarf | male only | 0x47000611 | 0x1F00058C | 0x04000557 | 62 |
| 0x72 | Beorning | male | 0x47007004 | 0x1F000588 | 0x04000553 | 89 |
| 0x72 | Beorning | female | 0x47007005 | 0x1F000589 | 0x04000554 | 89 |
| 0x75 | HighElf | male | 0x4700806C | 0x1F006111 | 0x04005EC4 | 89 |
| 0x75 | HighElf | female | 0x47008071 | 0x1F006110 | 0x04005EC5 | 89 |
| 0x78 | DwarfStoutAxe | both entries | 0x4700979A | 0x1F00058C | 0x04000557 | 62 |
| 0x7D | RiverHobbit | male | 0x4700CEA3 | 0x1F00846F | 0x0400772B | 89 |
| 0x7D | RiverHobbit | female | 0x4700CEA4 | 0x1F008470 | 0x0400772A | 89 |

Observations:

- **Two skeletons serve all eight 89-bone avatar setups**: the male setups 0x04000553, 0x04000555,
  0x04005EC4 and 0x0400772B carry one identical skeleton; the female setups 0x04000554,
  0x04000556, 0x04005EC5 and 0x0400772A another. Hobbit male, hobbit female and dwarf male
  each have their own 62-bone skeleton. (Ten 89-bone setups use this skeleton's naming
  without a rig prefix: the eight avatar setups and two others; one further 89-bone setup
  is an NPC rig.)
- The Beorning species reuses the human setups; its other forms are not in the race table
  (`appearance.md`).
- The River-hobbit setups use the human skeleton and share 8 of their 12 body parts with the
  human setups, all at human height (bind-pose geometry 1.82 m tall). Any size difference
  in the original must come from outside the setup (template fields, `setup.md` §5.2, or
  appearance scaling, `appearance.md`).

> **OPEN** (owner observation): whether a River-hobbit avatar in the original is drawn at
> the bind-pose height of its setup (about 1.8 m) or scaled down, and by which factor. A
> `/loc`-free check: stand a River-hobbit and a Human character side by side and compare
> heights in a screenshot.

<a id="s2-3"></a>
### 2.3 NPCs, creatures and objects (Confirmed)

Every other object resolves to its setup through the entity chain of `setup.md` §3.1
(landblock entities, `entity-descriptor.md`; items, `PhysObj`). Its skeleton is that
setup's tagfile.

<a id="s2-4"></a>
### 2.4 Census of setups by skeleton size and parts (Confirmed, all 28,555)

"Skinned" means the part mesh has at least one submesh with bone influences (`mesh.md` §4.1).

| Bones | Setups | All parts skinned | No part skinned | No parts |
|---|---|---|---|---|
| 1 | 22,434 | 19 | 22,413 | 2 |
| 2 | 5,344 | 161 | 5,183 | 0 |
| 3 – 29 | 484 | 480 | 4 | 0 |
| 30 – 248 | 293 | 293 | 0 | 0 |

No setup mixes skinned and unskinned parts. Geometry that must follow a bone is always
skinned, if need be with one influence per vertex (12,144 submeshes have exactly one
influence slot). An unskinned part never follows a bone (§9.1).

---

<a id="s3"></a>
## 3. Binding clips to skeletons

<a id="s3-1"></a>
### 3.1 No binding data (Confirmed)

A clip record holds no skeleton DID, no bone names it must match and no track-to-bone
index array (`havok-data.md` §4.6). The only clip-side data that mention bones are the
optional annotation track names (5,602 clips, `havok-data.md` §4.4).

<a id="s3-2"></a>
### 3.2 Rule: positional binding (Confirmed)

- **Track *j* of a clip drives bone *j* of the skeleton** (index into `bones`,
  `parentIndices` and `referencePose`, `havok-data.md` §4.2).
- **A clip is compatible with a skeleton exactly when `numberOfTransformTracks` equals the
  skeleton's bone count.**
- Bone names are never needed for binding. Annotation track names, where present, may be
  used as a diagnostic (§3.5) but not to reorder tracks.

This confirms ARCH §11.2 and `havok-data.md` H16, which marked the rule Reported.

<a id="s3-3"></a>
### 3.3 Evidence (Confirmed)

**Frame-0 translations.** Bone translations are rarely animated (a bone's length is fixed),
so a clip's frame-0 translation for track *j* should equal the reference-pose translation
of bone *j*. For every clip and every skeleton with the same bone count, the fraction of
tracks whose frame-0 translation lies within 2 mm of the reference translation was computed,
keeping the best skeleton:

| Best agreement | Clips |
|---|---|
| 100 % of tracks | 1,223 |
| 90 – 99 % | 18,708 |
| 50 – 89 % | 4,908 |
| below 50 % | 554 |
| no skeleton with that bone count | 2 |

Control: the same test with the track order reversed (sample of 5,005 clips with at least
three tracks) gives below 10 % agreement for 4,796 of them and 50 % or more for only 31.

**Rotations.** On skeletons whose reference pose has non-identity rotations, a clip's
frame-0 rotation equals the reference rotation of the same bone index for 9,999 of 17,492
such bone tracks (the rest are animated), in the same (x, y, z, w) component order; 286 of
the 9,999 store the negated quaternion (the same rotation, §5.5).

**Names.** 1,908 of the 5,602 annotated clips have a skeleton whose bone names match the
track names position by position (rule of §3.5). Exactly one annotated clip (0x050011A9, 62
tracks) carries the bone names of a skeleton in a different order; neither positional nor
by-name binding makes its frame-0 translations fit that skeleton (1 of 62 tracks either
way), so it is one of the unexplained clips below, not evidence for name binding.

**Skinning.** Posing the player meshes with frame-0 poses of 150 random compatible clips per
player skeleton keeps triangle edges near their rest length; a shuffled track order tears
the meshes (§4.5).

The 554 clips below 50 % agreement match no skeleton of the install well. Testing
uniformly scaled versions of the candidate skeletons explains 59 of them; the rest stay
unexplained (target rigs absent from this install, or clips that animate bone
translations from frame 0). The two clips without a same-count skeleton have 55 and 99
tracks.

<a id="s3-4"></a>
### 3.4 How a clip reaches an entity

Clips are chosen for an entity by its script and animation tables (`script-table.md`), not
by anything in the clip or the setup. The one direct reference found in entity property
sets is `Render_MeshCombiningPoseAnim` (51 entities, a 0x05 DID), whose use is open.
Whatever table selects a clip, `tessera-anim` applies the rule of §3.2:

- if the track count equals the bone count of the entity's skeleton, bind by position;
- otherwise the clip is **incompatible**: do not play it, and report it (category
  "track count mismatch"). Never bind the first min(tracks, bones) tracks, and never bind by
  name. No data in this install needs another rule.

<a id="s3-5"></a>
### 3.5 Annotation names as a diagnostic (Confirmed patterns)

Track names (`havok-data.md` §4.4) and bone names differ in spelling; a name check must
normalise both:

- a track name may be a path: segments joined by `|` (844 clips); use the last segment;
- compare case-insensitively;
- a track name matches a bone name when, after the rig-family prefix (the part up to the
  first `_`) is removed from the bone name, the track name equals it or ends with `_`
  followed by it (track names may carry an extra rig-variant prefix such as a weapon-style
  word).

Bones of 7,110 skeletons are named by their own index ("0", "1" …) and 4,658 skeletons have
no names; clips aimed at those skeletons can only be checked by track count and pose (§3.3).

<a id="s3-6"></a>
### 3.6 Clips per player skeleton (Confirmed)

Clips whose frame-0 translations agree on at least half of their tracks with the skeleton:

| Skeleton (setups) | Bones | Compatible clips |
|---|---|---|
| 89-bone male (0x04000553, 0x04000555, 0x04005EC4, 0x0400772B) | 89 | 2,421 |
| 89-bone female (0x04000554, 0x04000556, 0x04005EC5, 0x0400772A) | 89 | 2,436 |
| Dwarf male (0x04000557) | 62 | 2,195 |
| Hobbit male (0x04000551) | 62 | 1,754 |
| Hobbit female (0x04000552) | 62 | 1,753 |

The two hobbit skeletons and the dwarf skeleton have the same bone count, so a 62-track
clip is compatible with all three by the rule of §3.2; which one a clip was authored for is
visible only in its pose agreement (and its annotation names, where present). Clip
selection by script tables decides what plays (`script-table.md`).

---

<a id="s4"></a>
## 4. Skinning

<a id="s4-1"></a>
### 4.1 What vertex bone indices address (Confirmed)

The bone indices of a skinned vertex (`mesh.md` §5.7) index the bone array of the skeleton
the mesh is drawn with, directly: the setup's own skeleton for a setup part (§9), and the
wearer's skeleton for a worn part (`setup.md` §8, `appearance.md`). There is no
submesh-local palette and no remap table.

- For all 2,625 skinned submeshes reachable from setup part lists, the largest bone index
  used (by a non-zero weight) is smaller than the setup's bone count.
- For every worn-appearance use in the item property sets (an `Item_WornAppearanceMapStruct`
  with its 0x20 record, key, species and sex), every skinned part of the selected entry was
  checked against the bone count of that species and sex's skeleton (§2.2): 116,768 of
  116,768 checks pass. (175 uses name a key the 0x20 record lacks, `setup.md` §3.2.)
- Spatial check: vertices bound to a bone lie next to that bone in the reference pose (§4.5).

<a id="s4-2"></a>
### 4.2 The influence list is redundant (Confirmed, 43,059 of 43,059)

The per-submesh u32 list of `mesh.md` §6.2 is, exactly: for each vertex from the **last to
the first**, the vertex's **first** bone index (slot 0), written once for each non-zero
weight of that vertex. It carries no information beyond the vertex data, so it is not a
palette and not a remap. A decoder may recompute it to validate a record; renderers ignore
it. `mesh.md` §6.2 states this as the normative description.

<a id="s4-3"></a>
### 4.3 Palette (Confirmed sizes)

Because vertex indices address the skeleton directly, **the palette of a skinned draw is the
skeleton's skinning-matrix array, indexed by bone index**. No per-submesh palette needs to be
built.

| Measure | Value |
|---|---|
| Bones per skeleton, maximum | 248 (`havok-data.md` §8.2) |
| Largest vertex bone index | 247 (558 submeshes use an index of 128 or more) |
| Distinct bones used by one submesh | max 228, 99th percentile 45, median 5; 191 submeshes use more than 64, 10 more than 128 |
| Distinct bones used by one mesh record | max 246, 99th percentile 64 |

A GPU palette of 256 matrices per skeleton instance covers every observed case. A renderer
that wants smaller per-draw palettes may build a compact palette per submesh (the sorted
distinct bone indices of the submesh, with a remap applied to the vertex indices at upload
time), but this is an implementation choice and needs room for 228 entries in the worst case.

<a id="s4-4"></a>
### 4.4 Inverse-bind matrices (Confirmed)

Mesh records hold no bind matrices (`mesh.md` §3–§10; the influence list is redundant,
§4.2). The bind pose is the skeleton's **reference pose**: for bone *b*, let **B_b** be its
model-space reference transform (the reference pose composed from the root, §5.3); the
inverse-bind matrix is the inverse of **B_b**. The skinning matrix of bone *b* for a pose
with model-space bone transforms **P_b** is **P_b** · **B_b**⁻¹.

Skinning at the reference pose is therefore the identity: recomposing and applying the
matrices reproduces the stored vertices of all player setups' skinned parts within
5.3 × 10⁻⁵ m (floating-point error only).

<a id="s4-5"></a>
### 4.5 The mesh bind pose is the skeleton reference pose (Confirmed)

Mesh vertices are stored in the model space of the skeleton's reference pose. Evidence:

- **Proximity.** For every setup with at least three bones and at least 50 rigidly bound
  vertices (weight 1 on slot 0) in its level-0 skinned submeshes, the median distance of a
  bone's rigid vertices to that bone's segment (from its joint to each child joint, or the
  joint itself for a leaf), divided by the model's largest extent:

  | Setups | Median relative distance | Same with bone indices shuffled (control) | Better than control |
  |---|---|---|---|
  | 512 with identity reference rotations | 0.072 | 0.365 | 451 of 508 |
  | 180 with rotated reference bones | 0.044 | 0.376 | 169 of 177 |

  On the player setups, rigid vertices lie a few centimetres from their bone (eye vertices
  1.1–1.4 cm, wrist 2.2 cm, knee 4–6 cm, head 8 cm across the head).
- **Posed skinning.** Skinning the parts of four player setups (4,143 to 27,030 vertices)
  with frame-0 poses of 150 random compatible clips each: triangle edges keep their rest
  length (median change 0.0 – 0.5 %, 99th percentile 34 – 88 %, more than 50 % change on
  0.5 – 3.2 % of edges, the stretched ones lying across joints and cloth chains). With the
  clip tracks shuffled the 99th percentile is 784 – 1,135 % and 7.5 – 26 % of edges stretch
  by more than half.

<a id="s4-6"></a>
### 4.6 Influences per vertex (Confirmed)

At most four (`mesh.md` §4.1). Over all 28,635,289 skinned vertices, the number of non-zero
weights is 1 for 16,358,424, 2 for 9,444,485, 3 for 2,538,639 and 4 for 293,741. Influence
slots per submesh: 1 (12,144 submeshes), 2 (6,561), 3 (14,711), 4 (9,643). Weights already
sum to 1 (`mesh.md` §5.7).

<a id="s4-7"></a>
### 4.7 Skinning formula

Linear blend skinning: a vertex position *v* (as a point) becomes
Σₖ *wₖ* · (**P**_{jₖ} · **B**_{jₖ}⁻¹) · *v* over its influences (bone index *jₖ*, weight
*wₖ*); normals, tangents and binormals use the same blended matrix without translation and
are renormalised. The data gives no hint of dual-quaternion skinning; linear blending
reproduces plausible poses in the tests of §4.5 (Reported analogy for the original's exact
method).

---

<a id="s5"></a>
## 5. Conventions

<a id="s5-1"></a>
### 5.1 Space, axes, units (Confirmed)

Skeletons live in the same space as meshes (`mesh.md` §11): **Z up, metres, right-handed,
characters face +Y, their right is +X, model origin on the ground at the foot point**. No
axis conversion between Havok data and mesh data is needed.

Evidence on the player skeletons (reference pose composed to model space): left/right bone
pairs mirror across x = 0 within 0.6 mm; left-side joints have negative x (left wrist of the
male 89-bone skeleton at x = −0.73); the head joint is at z = 1.60 (89-bone male), 1.58
(female), 1.00 (hobbit male), 1.20 (dwarf); feet are at z = 0.01 – 0.02 with toes towards +Y;
the attach point `Mount_Connection` (§8.2) composes to exactly (0, 0, 0).

The reference rotations of all bones of the player skeletons are identity, except one
face-offset bone stored as a half turn about x (a mirrored partner). So in the bind pose
every player bone frame is axis-aligned with the model (MV observed the same for the hand
bones).

<a id="s5-2"></a>
### 5.2 Quaternions (Confirmed)

| Where | Component order | Identity written as |
|---|---|---|
| Skeleton reference pose and clip samples (`havok-data.md` §4.2, §5.6, §6) | (x, y, z, w) | (0, 0, 0, 1) |
| Setup frames: lists A, B, C, collision boxes (`setup.md` §6.3, §6.7) | (w, x, y, z) | (1, 0, 0, 0) |

A quaternion *q* = (x, y, z, w) rotates a vector *v* **actively**: *v′* = *q* · *v* · *q*\*,
the usual right-handed rotation by angle 2·acos(w) about axis (x, y, z) (counter-clockwise
when the axis points at the viewer). Evidence:

- Arms and feet. Posing the player skeletons with the frame-0 pose of every compatible clip
  (2,301 clips for the 89-bone male skeleton, 1,631 for the hobbit male): with this
  convention the wrists are below the shoulders in 92.4 % and 94.1 % of the clips (idle and
  locomotion poses lower the arms from the T-pose), and a foot is within 5 cm of the ground
  in 74.9 % and 72.8 %. With the conjugate (inverse) rotation the wrists are below the
  shoulders in only 14.6 % and 17.0 %.
- Collision boxes: the same active rotation, with the (w, x, y, z) order, reproduces the
  physics bounds of all 990 box-only setups (`setup.md` §6.4).
- Held-item frames: with this convention, sheathed weapons hang blade-down behind the back
  or at the hips (§8.3); read in the other component order they would lie horizontally.

<a id="s5-3"></a>
### 5.3 Composition order (Confirmed)

- **Local transform** of a bone (relative to its parent) from (translation *t*, rotation
  *q*, scale *s*): scale first, then rotate, then translate: **L** = T(*t*) · R(*q*) · S(*s*),
  as 4 × 4 matrices acting on column vectors.
- **Model transform**: **W**_b = **W**_{parent(b)} · **L**_b; a root bone (parent −1) has
  **W**_b = **L**_b. Parents precede children in the arrays (`havok-data.md` §4.2), so one
  forward pass suffices.
- Composing in the other order (**L**_b · **W**_{parent}) breaks the mirror symmetry of the
  89-bone skeletons (13 cm error) and tears posed meshes as badly as a shuffled track order
  (99th-percentile edge stretch 25 – 43 times the rest length).
- The model transform of the object (its world placement, `entity-descriptor.md` §5) is
  applied after the skeleton: world = object placement · **W**_b.

<a id="s5-4"></a>
### 5.4 Scale (Confirmed occurrence, inheritance OPEN)

Non-unit scale is rare: 224 reference-pose bones in 182 setups (137 uniform), and at frame 0
non-unit scale on some track of 1,217 clips (1,047 of them non-uniform); in 296 of those clips
a scaled bone has a child with a non-zero translation, so inheritance matters there. Tests
on 86 such clips with skinned meshes could not tell full matrix inheritance (§5.3, the
parent's scale also scales the child's translation and shape) from a "rigid parent, own
scale only" rule: the edge-stretch medians are equal (17 clips favour one, 18 the other,
51 tie).

> **OPEN** (owner observation): whether a parent bone's scale propagates to its children.
> Until resolved, `tessera-anim` uses the full matrix composition of §5.3, which is exact for
> all observed reference poses. A visible check: a creature clip that inflates or shrinks a
> limb (non-unit scale on a bone with children) shows whether the children grow with it.

<a id="s5-5"></a>
### 5.5 Quaternion sign

*q* and −*q* are the same rotation, and the data uses both: 2.9 % of frame-0 rotations that
equal a reference rotation are stored negated (§3.3), and neighbouring spline control
points flip sign 3,743 times (`havok-data.md` §7.6). Every comparison, blend or
interpolation must first align signs (§6.5).

---

<a id="s6"></a>
## 6. Sampling semantics

<a id="s6-1"></a>
### 6.1 Frame rate and duration (Confirmed)

| Frame rate ((frames − 1) / duration) | Clips |
|---|---|
| 60 per second | 19,279 |
| 120 | 4,762 |
| 30 | 439 |
| other (including 2-frame holds) | 915 |

Frame *f* lies at time *f* · duration / (frames − 1); the first frame is at 0, the last at
`duration` (`havok-data.md` §5.1, §6). Sampling uses the tagfile's duration
(`havok-data.md` §2.3); 21 envelopes disagree with it (`havok-data.md` §8.4, OPEN there).

<a id="s6-2"></a>
### 6.2 Interpolation (normative reference)

Between frames, spline clips are evaluated as B-spline curves and interleaved clips by
linear and normalised-linear interpolation, exactly as `havok-data.md` §7.4–§7.8 states.
`tessera-anim` must call those routines with times inside [0, duration]; this section adds
only what happens outside that range.

<a id="s6-3"></a>
### 6.3 Looping and wrap (Confirmed statistic, choice by caller)

A clip carries no loop flag, no loop range and no events (`havok-data.md` §4.4). Whether a
clip loops is decided by whoever plays it (`script-table.md`).

- 13,915 of the 24,994 clips with at least three tracks **end on their first pose**: for every
  track except the first three (root bones, §7.1), the last frame equals the first within
  1 mm and 0.5°. 15,443 do so within 1 cm and 2°.
- So a looping clip duplicates its first frame at the end. **Wrap rule**: for a looping clip
  at play time τ ≥ 0 use *t* = τ mod `duration` (the pose at `duration` equals the pose at 0).
  Do not add an interpolation segment from the last frame back to the first.
- A one-shot clip clamps: *t* = min(τ, `duration`), holding the last pose.
- A clip of duration 0 (none observed; the shortest is 0.016 s) is a constant pose.

<a id="s6-4"></a>
### 6.4 Playback rate

Clip time advances at real time × playback rate. The data holds no per-clip rate; rates other
than 1 come from the playing system (`script-table.md`, locomotion matching §7.4).

<a id="s6-5"></a>
### 6.5 Normalisation and shortest path

- Every sampled rotation is normalised (spline evaluation already does this,
  `havok-data.md` §7.6).
- **Blending two poses** (cross-fade between clips, or a partial clip over a base clip):
  per track, translation and scale blend linearly; rotation blends by normalised linear
  interpolation after negating the second quaternion when its dot product with the first is
  negative (shortest path). Spherical interpolation is an acceptable alternative.

> **OPEN** (owner observation): the original's blend method and cross-fade times between
> clips. Recommendation above; cross-fade durations belong to `script-table.md` if the data
> has them.

<a id="s6-6"></a>
### 6.6 Partial and additive clips (Confirmed patterns)

- **Partial clips are marked by the envelope's per-track weight array** (`havok-data.md`
  §2.3; 6,935 clips). An absent array means weight 1 on every track. On the player
  skeletons the arrays follow one dominant pattern, **upper body only**: legs, hips and the
  animated root at 0.01 (sometimes 0), the spine rising through 0.5, arms, head and face at
  1.0, the unanimated top bones at 1.0.

  | Skeleton | Clips with weights | Upper-body pattern (legs < 0.05, arms = 1) | Lower-body pattern | Other |
  |---|---|---|---|---|
  | 89-bone male | 1,171 | 1,071 | 6 | 94 |
  | Hobbit male | 735 | 643 | 4 | 88 |
  | Dwarf male | 1,116 | 934 | 6 | 176 |

  Value census over all arrays: 1.0 (325,452 values), 0.01 (51,257), 0.0 (49,748), 0.5
  (12,878), then 0.05, 0.8, 0.3 and others.
- **Recommended use** (Reported analogy, the original's exact blend is OPEN): a weighted
  clip layered over a base pose blends each track as base + *w_j* · (layer − base), with the
  rotation blend of §6.5 and *w_j* the track's weight times the layer's overall weight. A
  weighted clip played alone uses the full clip pose.
- **Additive clips do not exist**: no clip has near-zero translations and identity rotations
  on its tracks (3 clips come close, and they bind to skeletons whose own reference
  translations are zero), and no blend-mode datum exists (`havok-data.md` §4.6). Every clip
  is an absolute pose clip.

> **OPEN** (owner observation): confirm that weighted clips are layered over a base
> (for example an upper-body attack over legs that keep walking). Visible check: attack while
> running.

---

<a id="s7"></a>
## 7. Root motion

<a id="s7-1"></a>
### 7.1 The top of the hierarchy (Confirmed on the player skeletons)

The player skeletons start with three bones (one root):

| Index | Role (`BoneType` key, §8.1) | Parent | Reference translation | Animated in clips |
|---|---|---|---|---|
| 0 | top bone (no `BoneType`) | −1 | (0, small y, pelvis height): 0.995 m (89-bone male), 0.627 (dwarf), 0.508 (hobbit male) | never: static and equal to the reference in all 2,421 / 2,195 / 1,754 compatible clips |
| 1 | `physics` (key 1) | 0 | (0, 0, 0) | static in all but 1 clip |
| 2 | `Root` (key 3) | 0 | (0, 0, 0) | yes: the pelvis offset; everything else of the body hangs below it |

So the model origin stays on the ground under the character, bone 0 lifts the frame to the
pelvis, and bone 2 carries the body's own motion relative to that.

<a id="s7-2"></a>
### 7.2 In-place clips (Confirmed)

Over the 24,484 clips that agree with a skeleton of at least three bones (§3.3), the
horizontal displacement from first to last frame of the first four tracks:

| Class | Clips |
|---|---|
| In place (every one of those tracks returns within 10 cm) | 20,471 (84 %) |
| Some track moves more than 10 cm | 4,013: track 2 in 3,210, track 1 in 543, others few |

Speeds of the displacing clips (horizontal distance / duration): median 0.28 m/s, 95th
percentile 2.6 m/s; 555 exceed 1 m/s. On the player skeletons, track 2 moves more than 5 cm
first-to-last in 22 % of clips (median 0.16 m/s). These are emotes, lunges, knock-backs
and transitions, not locomotion.

**Locomotion cycles are treadmill clips**: the body stays over the origin and the planted foot
slides backwards (§7.4). The world movement of a walking or running character comes from the
movement system, not from the clip.

<a id="s7-3"></a>
### 7.3 Extracted motion (Confirmed)

112 clips carry an extracted-motion reference frame (`havok-data.md` §4.5). Its samples are
the motion removed from the bones:

- **w is an accumulated yaw angle in radians about `up`**. 49 of the 112 first reference
  frames change w over the clip; 24 end at ±0.785 (±π/4) with zero translation: paired
  turn-in-place clips (for example 0x0500012B
  +π/4 and 0x0500012C −π/4 on one creature rig). In those clips the bone tracks end exactly
  where they start (root yaw change 0.0°), so the turn exists only in the reference frame.
- x, y, z are an accumulated translation; 22 frames move more than 10 cm, mostly along ±y
  (for example 0.93 m forward over 3.3 s, 5.06 m backward over 2.5 s); speeds 0.08 – 26 m/s.
- The `forward` member is (1, 0, 0) in 207 of 214 frames even though characters face +Y; it
  does not describe the character's facing and is not needed.
- **Use**: at clip time *t*, the entity's placement is moved by the change of the sampled
  reference frame since the previous update: rotate the entity about the world up axis by
  Δw and translate it by Δ(x, y, z) expressed in the entity's frame at the clip's start.
  Bones are sampled as usual. When the reference frame has fewer samples than the clip has
  frames (3 cases), hold its last sample. When both a leading and an embedded copy exist
  they are identical (`havok-data.md` §4.5); use either.

> **OPEN** (owner observation): the sign of w. A positive w is a counter-clockwise turn seen
> from above under the active-rotation convention of §5.2 (a left turn for a character
> facing +Y). Check on a creature that turns in place (which way it ends up facing).

> **OPEN** (implementation, M4 character viewer): does the extracted translation scale with
> the entity's model scale (`appearance.md` §6.1)? The motion is removed from bones that are
> drawn scaled, so the explorer scales Δ(x, y, z) by the model scale (otherwise the feet of
> a River-hobbit, scale 0.7, slide by 0.3 × 3.17 m per cycle of 0x05008E8E). Please confirm.

<a id="s7-4"></a>
### 7.4 Locomotion speed of treadmill clips (Confirmed estimates, matching OPEN)

A clip's natural ground speed can be measured from its planted foot: while a foot is within
a quarter of its vertical range of its lowest point, its horizontal velocity is the
treadmill speed. Over clips compatible with the player skeletons whose feet both lift at
least 8 cm and stride at least 25 cm along y:

| Skeleton | Forward-locomotion clips (foot slides towards −y) | Speed clusters (m/s) | Backward clips (median m/s) |
|---|---|---|---|
| 89-bone male | 107 | median 2.1 of the 47 below 4 m/s (20 of them at 2 – 2.5); median 6.6 of the 60 at 4 m/s or more (52 at 6 – 7) | 28 (2.7) |
| Dwarf male | 54 | median 1.9 below 4 m/s; median 5.7 at 4 m/s or more | 13 (2.2) |
| Hobbit male | 38 | median 1.9 below 4 m/s; median 5.8 at 4 m/s or more | 9 (3.0) |

So a typical walk cycle covers about 2 m/s and a run about 6 m/s at playback rate 1. These
are estimates from a heuristic contact test, not stored values.

> **OPEN** (owner observation): how the original matches clip rate to movement speed (scale
> the playback rate by actual speed / clip speed, or play at rate 1 and let feet slide), and
> the avatar's walk and run speeds. Check: time a character running a known distance between
> two `/loc` readings, and watch whether the feet slide.

---

<a id="s8"></a>
## 8. Attach points

<a id="s8-1"></a>
### 8.1 `attach_map`: semantic bone table (Confirmed, resolves `setup.md` §6.2)

The setup's `attach_map` (`setup.md` §6.2) maps a **`BoneType`** enum key to a **bone index**
of the setup's skeleton. The mapper is the one labelled `BoneType` in the EMAPPER DID map
(0x23000086 here, base mapper 0x2300016E with the small keys).

- Present in 378 setups (15 with 1 bone, 30 with 2, 104 with 3 – 29, 229 with 30 or more);
  1 to 46 elements; keys unique and values unique within each setup; every value is below
  the bone count (378 of 378).
- Where bones are named, the bone at the value carries the label's body part in its name in
  1,553 of 2,490 elements; the others differ only in spelling (left/right abbreviations, the
  face-slider bones of §2.2 named "…_offset", effect rigs with generic labels).
- Labels in use (70 enum keys + 2 small keys): body bones (`head`, `neck`, `Thorax`,
  `midback`, `back`, `Lowerback`, `Lumbar`, `L/RShoulder`, `L/Rcollar`, `L/Relbow`, `L/Rulna`,
  `L/Rwrist`, `L/Rfemur`, `L/Rknee`, `L/Rankle`, `eyes`), weapon holders (`Lweaponholder`,
  `Rweaponholder`), tails (`TailBase`, `Tail1` … `Tail6`), generic and whip bones (`Generic1`
  … `Generic12`, `Whip1`, `Whip2`, `Extra3`, `Extra4`), 21 face-slider bones (`Face_…`,
  present on ten 89-bone setups: the eight avatar setups and two others), and the small
  keys 1 = `physics` and 3 = `Root`. Unused labels: `Extra1/2/5/6`, `ExtraBase`, `WhipBase`, `Whip3` … `Whip6`, two
  `Face_…` separations, and `Camera_Origin`/`Camera_Target` (2, 4).

**Use**: effect and script properties name bones by `BoneType` (for example
`ParticleScript_BoneType`, `BeamScript_TargetBoneType`, `MeshFXScript_BoneType`,
`CinematicCameraScript_BoneType`, `Combat_Beam_SourceBoneType`, all enum properties labelled
by this mapper). To find the bone, look the key up in the target setup's `attach_map`; a key
that is absent means the skeleton has no such bone (fall back to the object origin). Face
sliders (`appearance.md`) find their bones the same way.

<a id="s8-2"></a>
### 8.2 Frame list A: holding locations (Confirmed, resolves `setup.md` §6.3 for list A)

List A elements (`setup.md` §6.3: `key`, `key2`, `bone`, frame) are **holding locations**:

- `key` is a **`HoldingLocation`** enum key (mapper labelled `HoldingLocation`, 0x23000068
  here, base mapper 0x230000FE with the small keys 1 = `RHand`, 2 = `LHand`, 3 = `LOffHand`,
  17 = `LHand2`, 18 = `RHand2`, 19 = `LHand3`; 4, 5, 6 = camera and origin labels). All 115
  keys in use resolve. `key2` always equals `key`. Keys are unique within a setup (28,555 of
  28,555).
- `bone` is an index into the setup's skeleton (always below the bone count, 2,737 of
  2,737 setups), or −1: the frame is relative to the **setup origin** (object space). −1
  occurs in 2,338 setups, almost all one-bone props and weapons.
- The frame (position, scale, rotation (w, x, y, z)) is expressed **in the bone's frame**:
  the location's model transform is **W**_bone · T(position) · R(rotation) · S(scale)
  (§5.3), or the frame alone for bone −1. Scale is (1, 1, 1) on every held-item, sheath and
  body location; non-unit scales occur only on weapon and effect points of props.

Evidence for bone-local frames (player setups, reference pose): `Mount_Connection` on bone 0
composes to exactly (0, 0, 0) on all three player skeletons tested although its stored
position differs per skeleton (it cancels bone 0's pelvis height); `Eye_Left` and
`Eye_Right` on the head bone land within 1 – 2 cm of the eye bones, on the correct sides;
`Particle_OverHead` and `Particle_OverHead2` land 0.6 – 0.7 m above the head joint.

Families of locations:

| Family | Keys (labels) | Where |
|---|---|---|
| Held items | `Wpn_RHand`, `Wpn_LHand`, `Wpn_LOffHand`, `Wpn_ROffhand`, `Wpn_Forearm`, `Forearm`, small keys `RHand`, `LHand`, `LOffHand`, `RHand2`, `Arrow`, `Arrow_Offhand` | characters, NPCs, creatures (§8.3) |
| Stowed items | `Sheath_LHip`, `Sheath_RHip`, `Sheath_Back_2H`, `Sheath_Back_2H_Sword`, `Sheath_Back_Bow`, `Sheath_Back_Crossbow`, `Sheath_Back_Shield` | characters and humanoid NPCs |
| Mount | `Mount_Connection` (rider's seat reference on the rider, 34 setups) | riders |
| Body points | `Head`, `Eye_Left`, `Eye_Right`, `Eye_Left2`, `Eye_Right2`, `Mouth`, `Nostril_Left/Right`, `Shoulder_Left/Right`, `Elbow_Left/Right`, `Foot_Left/Right`, `Foot_Left2/Right2` | effects |
| Targets | `Target_Head`, `Target_Thorax1/2/3`, `Target_Back`, `Target_LShoulder`, `Target_RShoulder`, `Target_LThigh`, `Target_RThigh` | projectile and effect targets |
| Effect anchors | `Particle_A` … `Particle_Z`, `Particle_OverHead`, `Particle_OverHead2`, `Holding_Loc_Entity_00` … `_10`, `Cosmetic`, `CharGen_New_1` … `_8`, `CharGen_Existing_1` … `_5` | props, effects, character creation |
| Weapon points | `Weapon_Handle_A/B/C`, `Weapon_Mid_A/B/C`, `Weapon_End_A/B/C`, `Weapon_Tip_A/B/C`, `Weapon_Edge` | on the weapon's own setup (§8.6) |

The `Target_…` and some body points store the rotation (0, 0, 0, 1) in (w, x, y, z) order, a
half turn about z; where only the position matters this is harmless.

<a id="s8-3"></a>
### 8.3 Held-item and sheath locations on characters (Confirmed)

On the five player skeletons (and the humanoid NPC rigs that share the pattern):

| Location | Bone (`BoneType` of that bone) | Frame | Weapon long axis (+Y of the item) in the bind pose |
|---|---|---|---|
| `Wpn_RHand`, `RHand`, `RHand2`, `Wpn_ROffhand` | right weapon holder (`Rweaponholder`) | identity | forward (+Y) |
| `Wpn_LHand`, `LHand` | left weapon holder (`Lweaponholder`) | identity | forward |
| `Wpn_LOffHand`, `LOffHand` | left weapon holder | half turn about y | forward, mirrored about its own long axis |
| `Wpn_Forearm`, `Forearm` | left elbow or left ulna | small offset, small rotation (15 – 20°) | forward (a shield on the forearm) |
| `Arrow` | right weapon holder | 0.45 – 0.59 m along x, quarter turn about z | along +x (nocked arrow) |
| `Sheath_LHip`, `Sheath_RHip` | hip girdle | ±0.15 – 0.24 m sideways | down and backwards (blade below the hip) |
| `Sheath_Back_2H`, `Sheath_Back_2H_Sword`, `Sheath_Back_Bow`, `Sheath_Back_Crossbow` | thorax | 0.15 – 0.27 m behind | down and diagonally across the back |
| `Sheath_Back_Shield` | thorax (some NPC rigs: mid back) | 0.13 – 0.17 m behind | up (shield's top up) |

Census over all setups: `RHand` 188, `LHand` 185, `Wpn_RHand` 162, `Wpn_LHand` 157,
`Wpn_LOffHand` 151, `Wpn_ROffhand` 151, `Wpn_Forearm` 137, `LOffHand` 129,
`Sheath_Back_Shield` 37, `Sheath_LHip` 30, `Sheath_RHip` 26, `Sheath_Back_2H` 26,
`Sheath_Back_Bow` 26, `Sheath_Back_2H_Sword` 12, `Sheath_Back_Crossbow` 12. On creature rigs
the hand locations sit on whatever bone the rig uses (index-named bones, paws, wings, wrists).

<a id="s8-4"></a>
### 8.4 Placing a held item (Confirmed geometry, Reported grip convention)

1. Resolve the item to its own setup through `PhysObj` (`setup.md` §3.1), unless the item
   has a worn appearance for the wearer, in which case it is worn, not held (MV; `setup.md`
   §13.3).
2. Choose the holding location (§8.5) and look it up in the **wearer's** setup list A. If
   absent, do not draw the item (or draw it at the wearer's origin in a debug view).
3. The item's model transform is the location's model transform of §8.2 at the current pose,
   times the object's world placement. The item's own setup parts are drawn in its own
   object space (`setup.md` §7); the item's skeleton, if it has more than one bone, is posed
   independently (identity pose unless a clip plays on it).

Weapons are modelled with the **grip at the origin and the long axis along +Y**: over the
1,591 weapon setups with both `Weapon_End_A` and `Weapon_Tip_A`, the direction from end to
tip is +Y in 975, +X in 492 (bow-like and transverse shapes), −X in 103, other 15; median
positions are `Weapon_Handle_C` y = −0.22, `Weapon_Handle_B` −0.01, `Weapon_Handle_A` 0.16,
`Weapon_End_A` 0.33, `Weapon_Mid_A` 0.59, `Weapon_Tip_A` 0.76 m. No weapon setup carries a
grip location of its own (only one one-bone setup has a `Wpn_RHand` entry), so no extra
item-side offset applies.

<a id="s8-5"></a>
### 8.5 Choosing the holding location (Confirmed sources, rule partly OPEN)

Sources found in the data:

| Source | Content |
|---|---|
| Inventory-control object (DID-map label `InventoryControl` in `WEENIECONTENT`; 0x70000230 here), `InventoryControl_SlotToPaperdollLocation_MappingList` | Paperdoll (UI viewport) location per equipment slot: `Weapon_Primary` → `Wpn_RHand`, `Weapon_Secondary` → `Wpn_LOffHand`, `Weapon_Ranged` → `Sheath_Back_Bow`. Slots are `ContainerSlot` bit-field values (0x10000, 0x20000, 0x40000). |
| Same object, `InventoryControl_MutuallyExclusiveLocation_MappingList` | One group of mutually exclusive locations: `Wpn_RHand`, `Wpn_ROffhand`, `Wpn_LHand`, `Wpn_LOffHand` |
| Item property `Inventory_ParentingLocation` | Per-item override of the in-world location: `Wpn_Forearm` (2,900 items, all shields), `Wpn_ROffhand` (893 crossbows, 892 javelins, 47 bows), `Forearm` (2 shields) |
| Item property `Inventory_PaperdollParentingLocation` | Per-item paperdoll override: `Wpn_Forearm` (2,759 shields), `Sheath_Back_Crossbow` (892 crossbows), `Forearm` (143 shields) |
| Cosmetic and NPC properties (`Inventory_Cosmetic_Slot1…3_HoldingLocation`, `NPC_Cosmetic_Inventory_HoldingLocation1…4`, `Channeling_Player_Cosmetic_Slot_HoldingLocation`, `Mounted_Player_Cosmetic_Slot_HoldingLocation`, `QuestState_…_HoldingLocation`) | Explicit locations for cosmetic props: slot 1 mostly `Wpn_RHand` (3,253 of 3,515), slot 2 mostly `Wpn_LHand` (1,030 of 1,970), mounted cosmetics `Mount_Connection` (722) |

Rule for a viewer, until the script tables are specified: primary weapon `Wpn_RHand`,
secondary item `Wpn_LOffHand`, ranged weapon `Sheath_Back_Bow`, each replaced by the item's
`Inventory_ParentingLocation` when present (in the world) or
`Inventory_PaperdollParentingLocation` (in a paperdoll view).

> **OPEN** (for `script-table.md`, and owner observation): the in-world default location per
> equipment slot (the data has only the paperdoll list), and when weapons move between the
> hands and the sheath locations (combat stance on and off) and which sheath a weapon class
> uses. The sheath locations exist on every player setup; nothing in the item or inventory
> data selects them. Check in the original: draw and sheathe a one-handed sword, a
> two-handed sword, a two-handed axe, a bow, a crossbow and a shield, and note where each
> rests.

<a id="s8-6"></a>
### 8.6 Weapon points (Confirmed layout, use Reported)

1,707 setups (1,694 of them with one bone) carry up to twelve
`Weapon_Handle/Mid/End/Tip_A/B/C` points with bone −1 (object space) along the weapon's long
axis (§8.4), plus `Weapon_Edge` once. They look like anchor sets for swing trails or impact
effects (three sets A, B, C). They are not used for placing the weapon in a hand.

<a id="s8-7"></a>
### 8.7 Frame lists B and C (Confirmed keys, use partly OPEN)

- **List C** (50 setups, 54 elements; setups with one or two bones) is keyed by
  **`UIViewport_Type`** values (the enum mapper that the property `UIViewport_Type` names as
  its label source, 0x23000194 here; it has no EMAPPER label; labels `Paperdoll`, `Opponent`,
  `Self`, `AppearanceMod`, `MountedPaperdoll`, `EmotesPaperdoll`): observed `Opponent` (31),
  `MountedPaperdoll` (19), `EmotesPaperdoll` (3), `AppearanceMod` (1). They look like object
  offsets for the UI's 3D viewports. World rendering ignores them.
- **List B** (42 setups, 50 elements, `id` 0 in 48 and 0x10000001 in 2; 34 of the setups
  have two bones) holds unscaled, unrotated positions such as (0, 0, 2), (0, 0, 6.1),
  (0, −6.7, 0) and (0, 2.82, 0.01) on large single-part props.

> **OPEN:** what list B places (a seat, a spawn or an effect point are candidates). World
> rendering ignores it.

---

<a id="s9"></a>
## 9. Multi-part characters on one skeleton

<a id="s9-1"></a>
### 9.1 Rule (Confirmed; resolves the identity-transform OPENs of `setup.md` §7 and §10.2)

All parts of a setup are drawn **in the setup's model space with an identity part
transform**; the part list holds no transforms. For a setup with skinned parts, every
skinned submesh of every part is skinned against the one skeleton of the setup with the
palette of §4.3. At the reference pose the result equals the stored geometry (§4.4), which is
why the bind-pose display with identity transforms looked right. An unskinned part (only in
setups whose parts are all unskinned, §2.4) stays fixed in the setup's model space and does
not follow any bone.

<a id="s9-2"></a>
### 9.2 Parts carry their slot (Confirmed)

The mesh record's `flags` word (`mesh.md` §3.3) is a value of the enum mapper labelled
**`MeshTypeID`** (0x23000089 here, base 0x23000170 with 0 `Invalid`, 1 `Default`, 2 `All`):

- every one of the 47,563 mesh records has a `flags` value present in that mapper: 1
  `Default` (27,960 static meshes), 2 `All` (20), and 0x1000_00nn values (19,583: `Helmet`
  4,014, `UpperBody` 2,960, `Shoulder` 2,603, `Crest` 1,503, `Hands` 1,191, `Feet` 1,113,
  `Cape` 888, `Accessory1` 803, `Head` 611, mount and war-machine slots, fence pieces …);
- in the worn-appearance records the part `slot` (`setup.md` §8.3) equals the part mesh's
  `flags` for 80,974 of 81,098 parts (the rest: 84 static meshes with `Default` and 40 others);
- the player setups' parts are one mesh per body slot (for example the 89-bone male:
  `UpperBody`, `LowerBody`, `Hands`, `Feet`, `Head`, `Ears`, `Crest`, `Beard`, `Shoulder`,
  `Helmet`, `Cape`, `Satchel`), and 242 three-vertex skinned stub meshes are tagged with slots
  (`Accessory1` 32, `Crest` 28, `Shoulder`, `Head`, `UpperBody`, `Helmet` 17 each …).

So a worn item's part replaces the body part of the same `MeshTypeID` slot; the assembly
rules (which slots hide which) belong to `appearance.md`. The stub meshes are slot
placeholders (`setup.md` §8.5).

---

<a id="s10"></a>
## 10. Evaluation pipeline (in words)

For one animated object per frame:

1. **Skeleton**: from the object's setup (§2). Precompute the reference model transforms
   **B**_b and their inverses (§4.4) once per distinct skeleton.
2. **Clips**: for each clip layer chosen by the playing system, check compatibility (§3.4),
   compute the clip time (§6.3, §6.4) and sample every track (`havok-data.md` §7.7–§7.8) into
   local (translation, rotation, scale) per bone.
3. **Blend** layers per bone (§6.5, §6.6). Bones no clip drives keep their reference local
   transform.
4. **Root motion**: if a clip has an extracted-motion reference frame, apply its change to the
   object's placement (§7.3); bone tracks are used as sampled.
5. **Model transforms**: one pass from root to leaves (§5.3).
6. **Palette**: for each bone, model transform times inverse reference transform (§4.4); upload
   up to 256 matrices (§4.3).
7. **Draw** every part in the setup's model space with the object's world placement; skinned
   submeshes use the palette, unskinned parts are rigid (§9.1).
8. **Attached items**: for each held item, the location's model transform (§8.2) times the
   object's world placement is the item's world placement (§8.4); its own parts are drawn
   in its object space.

Sampling (step 2) is independent per object and per layer and runs on worker threads; the
results are plain arrays.

---

<a id="s11"></a>
## 11. Invariants (Confirmed over all records)

| ID | Invariant |
|---|---|
| **KI1** | Every skinned submesh reachable from a setup uses bone indices below that setup's bone count; every skinned worn part is below the bone count of every species/sex skeleton it is mapped to. |
| **KI2** | Influence list = per vertex from last to first, its slot-0 bone index once per non-zero weight (`mesh.md` §6.2). |
| **KI3** | `attach_map`: keys unique, values unique, values < bone count. |
| **KI4** | List A: `key2` = `key`; keys unique per setup; `bone` = −1 or < bone count. |
| **KI5** | Every clip's track count equals the bone count of at least one setup, except 2 clips (55 and 99 tracks). |
| **KI6** | Bone 0 of the player skeletons is static and equal to its reference in every compatible clip. |
| **KI7** | Mesh `flags` ∈ `MeshTypeID` keys for every mesh record. |

KI1–KI4 and KI7 are checks a decoder or loader may run and report; KI5 and KI6 are census
facts that tests may assert on real data.

---

<a id="s12"></a>
## 12. Census summary (Confirmed, 2026-10-05)

| Item | Value |
|---|---|
| Setups / distinct skeletons | 28,555 / 923 (30+ bones: 293 / 191) |
| Setups with skinned parts | 953 (all 293 with 30+ bones) |
| Clips agreeing with a same-count skeleton (≥ 90 % / 50 – 89 % / < 50 % of tracks) | 19,931 / 4,908 / 554; 2 without a same-count skeleton |
| Clips ending on their first pose | 13,915 of 24,994 (≥ 3 tracks) |
| In-place clips | 20,471 of 24,484 |
| Clips with extracted motion / with track weights | 112 / 6,935 |
| Setups with `attach_map` / list A / list B / list C | 378 / 2,737 / 42 / 50 |
| Skinned submeshes / vertices | 43,059 / 28,635,289 |

---

<a id="s13"></a>
## 13. Corrections to earlier notes

| Earlier statement | Where | Finding |
|---|---|---|
| "Tracks map to bones by position (Reported)" | ARCH §11.2, `havok-data.md` H16 | Confirmed (§3.3). |
| The per-submesh bone palette "does not parse reliably" / meaning open | MV, `mesh.md` §6.2 | It parses on all 43,059 submeshes and is redundant (§4.2). |
| "Real attach transforms are not read from the dat files"; hand-tuned hip and back offsets | MV `weapons.md` | Every hand, sheath and back location is stored in setup list A as (bone, bone-local frame) (§8.2–§8.3). |
| Man-vs-Elf rig assignment "provisional" | MV | Confirmed from the race table (§2.2): Human 0x04000553/554, Elf 0x04000555/556; both share one skeleton per sex. |
| `attach_map` "probably slot → bone", list A "probably attachment points", lists B and C unknown | `setup.md` §6.2–§6.3 | `BoneType` → bone; `HoldingLocation` → (bone, frame); C keyed by `UIViewport_Type`; B still open (§8). |
| Low 16 bits of skinned mesh `flags` unexplained | `mesh.md` §3.3 | The whole `flags` word is a `MeshTypeID` slot value (§9.2). |
| Cape and skirt bones may need runtime cloth simulation | MV | Clips animate them: cape bone rotations are animated in 90 – 96 % of compatible clips, skirt bones in 48 – 61 % (whether the original adds simulation on top stays open). |

---

<a id="s14"></a>
## 14. Synthetic worked examples (for tests)

All values are invented for this spec.

<a id="s14-1"></a>
### 14.1 Composition

A skeleton of three bones: bone 0 (parent −1) T = (0, 0, 1), R identity, S 1; bone 1
(parent 0) T = (0, 0.5, 0), R = 90° about z, stored (x, y, z, w) = (0, 0, √½, √½), S 1;
bone 2 (parent 1) T = (0, 1, 0), R identity, S 1.

| Bone | Model position | Model rotation |
|---|---|---|
| 0 | (0, 0, 1) | identity |
| 1 | (0, 0.5, 1) | 90° about z |
| 2 | (−1, 0.5, 1) (bone 1's rotation turns the local +y offset into −x) | 90° about z |

A test that composes in the wrong order gets (0, 1.5, 1) for bone 2, and one that uses the
conjugate rotation gets (1, 0.5, 1).

<a id="s14-2"></a>
### 14.2 Skinning matrix

Take the skeleton of §14.1 as the reference pose, and a pose that differs only in bone 1's
local rotation, now identity. Posed model positions: bone 0 (0, 0, 1), bone 1 (0, 0.5, 1),
bone 2 (0, 1.5, 1), all with identity rotation.

| Vertex (model space, reference) | Weights | Skinned position |
|---|---|---|
| (−1.2, 0.5, 1) (0.2 m beyond bone 2's joint along bone 2's local +y) | bone 2: 1 | (0, 1.7, 1) |
| (−1.2, 0.5, 1) | bone 1: 1 | (0, 1.7, 1) (bones 1 and 2 turn together) |
| (−1.2, 0.5, 1) | bone 0: 0.5, bone 2: 0.5 | (−0.6, 1.1, 1), the average of (−1.2, 0.5, 1) and (0, 1.7, 1) |
| any vertex | any weights, reference pose | unchanged (§4.4) |

<a id="s14-3"></a>
### 14.3 Influence list

Three vertices with two influence slots: vertex 0 bones (4, 7) weights (0.75, 0.25); vertex 1
bones (2, 0) weights (1, 0); vertex 2 bones (9, 4) weights (0.5, 0.5). The list is
(9, 9, 2, 4, 4): vertex 2's slot-0 bone twice, vertex 1's once, vertex 0's twice.

<a id="s14-4"></a>
### 14.4 Attach transform

A bone whose model transform is translation (0.8, 0, 1.36) with identity rotation, and a
list-A frame on it with position (0, −0.2, 0.1), rotation (w, x, y, z) = (√½, −√½, 0, 0)
(−90° about x), scale 1. The location's model transform places the item origin at
(0.8, −0.2, 1.46) and turns the item's +Y axis to model −Z (pointing down) and its +Z axis to
model +Y.

<a id="s14-5"></a>
### 14.5 Looping

A clip of duration 1.0 s with 61 frames that ends on its first pose, played looping: play
time 2.25 s samples *t* = 0.25 (frame 15); 3.0 s samples *t* = 0 (or 1.0, the same pose).
Played once: 2.25 s samples *t* = 1.0.

<a id="s14-6"></a>
### 14.6 Partial clip

A 4-track base pose with track rotations all identity and a layer whose track 3 is 90° about
z and whose track 2 is also 90° about z, track weights (1, 1, 0.01, 0.5), layer weight 1:
track 3 blends to 45° about z (the normalised linear blend of identity and 90° gives exactly
45° at weight 0.5); track 2 blends to about 0.8° about z (normalised linear blend at weight
0.01); tracks 0 and 1 take the layer's values.

---

<a id="s15"></a>
## 15. Open questions

> **OPEN** (owner observation): River-hobbit avatar size relative to its setup (§2.2).

> **OPEN** (owner observation): scale inheritance from parent to child bones (§5.4).

> **OPEN** (owner observation): the original's pose blending method and cross-fade times
> (§6.5); confirmation that weighted clips layer over a base (§6.6).

> **OPEN** (owner observation): the sign of the extracted-motion yaw (§7.3).

> **OPEN** (owner observation): playback-rate matching for locomotion and the avatar's walk
> and run speeds (§7.4).

> **OPEN** (for `script-table.md` and owner observation): in-world default holding location
> per equipment slot, and drawn versus sheathed placement per weapon class (§8.5).

> **OPEN:** frame list B (§8.7); `Render_MeshCombiningPoseAnim` (§3.4); the 554 clips that
> fit no skeleton of the install by pose and the 2 clips without a same-count skeleton (§3.3).

> **OPEN** (owner observation): whether cape and skirt bones get runtime simulation on top
> of their clip animation (§13).

None of these blocks rendering an animated character; they affect fidelity of playback
choices in `tessera-anim`.

Raised by the clean-room implementation (`tessera-anim`, 2026-10-05). The real-data run
built all 28,555 skeletons (999 distinct by bit-exact content), bound and sampled 25,393 of
25,395 clips (the 2 without a same-count skeleton are 0x05002255 and 0x05004AD7, as KI5
says), reproduced the §3.3 agreement buckets within a few clips (1,222 / 18,737 / 4,883 /
551 with skeletons grouped bit-exactly instead of to 1e-4), found KI1 and KI2 true on all
2,625 setup submeshes, and found no non-finite posed matrix. The implementation's choices
are stated; the spec writer should confirm or correct.

> **OPEN:** Reference-pose skinning tolerance (§4.4). With the stored weights as they are,
> skinning at the reference pose moves vertices by up to 4.4 mm (setup 0x04002BE1; about
> 730 of 2.5 million setup-part vertices move more than 10⁻⁴ · (1 + |*v*| / 10 m) m), because a vertex's weights sum to 1
> only within 2 × 10⁻⁴ (`mesh.md` §5.7) and the skinned position is Σ*w* · *v* = (Σ*w*) · *v*
> at the reference pose, which is off by |1 − Σ*w*| · |*v*| on large models. With the
> weights renormalised to sum 1 the largest error is 3.1 × 10⁻⁵ m, matching the 5.3 × 10⁻⁵ m
> of §4.4. Should renderers renormalise weights at load (`mesh.md` §5.7 says no
> renormalisation is needed), and does the 5.3 × 10⁻⁵ m figure assume it? The
> implementation does not renormalise; its real-data test allows 10⁻⁴ m (scaled for large
> models) plus |1 − Σ*w*| · |*v*|.

> **OPEN:** Singular reference transforms (§4.4). The spec does not say what to do when a
> bone's model-space reference transform cannot be inverted (a zero scale); none occurs in
> this install. The implementation rejects such a skeleton (`skeleton.singular_bind`), and
> also rejects skeletons with more than 256 bones (§4.3), non-finite reference values or a
> rotation that cannot be normalised (`skeleton.bad_reference`; reference rotations are
> normalised otherwise).

> **OPEN:** Extracted motion over loops (§7.3). "Δ(x, y, z) expressed in the entity's frame
> at the clip's start" leaves open what "start" is for a looping clip. The implementation
> restarts the reference frame each cycle: within a cycle the entity relative to the
> cycle's start is T(*p*(*t*) − *p*(0)) · R_up(*w*(*t*) − *w*(0)), each whole cycle adds the
> full-cycle motion, and playing backwards gives the inverse motion. Following the "Use"
> paragraph, it rotates about the world up axis (+Z) whatever the frame's stored `up`;
> please confirm for the 7 leading frames with `up` = (0, 1, 0) (§4.5 of `havok-data.md`;
> 0x050039F1 changes w by 0.041 rad), where "about `up`" in the first bullet would instead
> turn about y. It uses the clip frame grid holding the last sample
> (`ReferenceFrame::sample_on_grid`).

> **OPEN:** Broken sample values (§7.7 of `havok-data.md`: "may treat them as broken").
> The implementation replaces any sampled track that is not finite or whose translation
> exceeds 10⁴ m in some component by the bone's reference transform; on this install that
> affects 2 clips (0x05002819 and 0x05009355). Please confirm the threshold.

> **OPEN:** Track weights on the lowest layer (§6.6, `script-table.md` §6.5). "A weighted
> clip played alone uses the full clip pose" is implemented as: track weights never apply
> to the lowest layer that contributes, only to layers above it, and a layer of two
> cross-fading clips uses the blend of their weights. The spec does not cover a partial
> clip on a higher layer whose lower layers have weight 0 (it then plays over the reference
> pose with its weights).

> **OPEN:** Play times outside a cycle (§6.3). Negative play times (negative playback
> rates) wrap backwards for looping clips (*t* = τ mod `duration` with the Euclidean
> remainder) and clamp to 0 for one-shot clips; non-finite play times sample *t* = 0. The
> spec only states τ ≥ 0.
