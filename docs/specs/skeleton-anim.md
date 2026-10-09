<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Skeletons in use: bone mapping, skinning, conventions, sampling, root motion, attach points

| | |
|---|---|
| **Status** | Revised (2026-10-08, §7.4 OPEN and §15.1 O14.4 answered from `collision.md` §11.8; awaiting review) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (dirty room, a different session from the writer) |
| **Review record** | Re-review 2026-10-07 of the revision after review: **passed**, Accepted. BIN5 revision of 2026-10-07: not yet reviewed. Full record in the review log (decision 0018). |
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
| BIN | The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035, compiled 2026-09-27), disassembled in the dirty room for facts only (method M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md)), 2026-10-05. Questions: (1) how a bone's scale reaches its children and the skinned vertices (§5.4); (2) how clips playing at the same time on different layers combine per bone (§6.6). Why M1–M3 were not enough: the data hold no composition or blend rule; the §5.4 test on 86 clips could not tell the candidate rules apart; no published source states them; and observing the running client (M3) can show that River-hobbit feet do not float, but not which rule makes them so. Only offline-client code was read (pose evaluation, animation playback, mesh deformation); nothing of the network, login or anti-tamper code. |
| RH | Dirty-room probe of 2026-10-05 that runs the BIN rules of §5.4, §6.6 and `appearance.md` §6.4 on the real setups, clips and meshes of the River-hobbit, Human, Dwarf and DwarfStoutAxe bodies (all vertices of the default parts) and compares lowest vertex, lowest bone and top vertex with the earlier matrix rule. |
| BIN3 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-07 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), offline animation and script code only (animation component and layers, the Havok animation runtime it calls, the skinning palette, the script nodes that start clips, the movement code that sets the movement sliders). Questions, all from owner observation O14: (3) the shape and duration of clip ramps and cross-fades (§6.5); (4) the duration of a `UseBoneOpacity` change (§6.6); (5) whether and how the client applies a clip's extracted motion, and the sign of its yaw (§7.3); (6) whether the playback rate is matched to ground speed, and where the avatar's ground speed comes from (§7.4); (7) the exact skinning transform and inverse reference transform for rotated or scaled reference bones (§5.4); (8) whether capes and skirts get runtime cloth or secondary motion (§13). Why M1–M3 were not enough: the data hold no ramp curve, no blend or skinning rule and no cloth data; ground speeds are not in the data files (a full scan of all records for the move-speed properties found no stored value, OBS2); no published source covers the client's animation layer; and observing the running client (M3) can show that a blend takes "about 0.2 s" or that feet slide, but not the curve, the formula or which code path is responsible. Nothing of the network, login, launcher or anti-tamper code was read. |
| OBS2 | Observed in the user install, 2026-10-07: a raw scan of every record of every archive for the property IDs of `Movement_RunForwardMoveSpeed`, `Movement_WalkForwardMoveSpeed`, `Movement_BackwardMoveSpeed`, `Physics_AvatarForwardMoveSpeed` and `Physics_AvatarBackwardMoveSpeed`, and a decode of all 0x47 entity sets and all 0x78–0x7F property sets for them as top-level properties (none set; the raw hits are coincidental byte patterns, enum values and script keys); the dictionary defaults of those properties; the controller property sets of all 127 PlayerPhysics script nodes (decoded with the repository's property decoder used as a tool). |
| BIN4 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-07 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), in a fresh copy of the analysis project, to answer the review of 2026-10-07: offline movement-controller code only (the player physics-control setup that builds the avatar's controller configuration, the controller's parameter block and its property bindings, the per-frame input-to-slider step, the per-direction speed step). Questions: (9) which slider time *T* the player's own input path uses and where it comes from (§7.4, §15.1 O14.1); (10) where the player's per-direction move speeds come from (§7.4, O14.4); (11) the order of the max and the doubling in the speed mix (§7.4). Why M1–M3 were not enough: the data hold the parameter structures (OBS3) but not which of them the client applies in which state, nor which field drives the slider time; no published source covers them; observing the running client (M3) gives an approximate blend time and a speed, not their source. Nothing of the network, login, launcher or anti-tamper code was read. |
| BIN5 | The same executable (file version 4906.0071.1794.4035), disassembled again on 2026-10-07 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), in a fresh copy of the analysis project, because a clean-room audit of the River-hobbit height code found code that contradicts §5.4 and §6.6. Questions: (12) whether the 1/255 weight quantisation adds a tolerance (§6.6 step 3); (13) whether a clip's overall weight is its own ramp value or a product along a cross-fade chain (§6.6 step 2); (14) the skinning palette (§5.4, K26); (15) whether a layer limits its number of clips, and whether starting a clip changes other clips' weights (§6.5). Why M1–M3 were not enough: these are rules of code that leave no trace in the data; observing the running client (M3) cannot separate a 10⁻⁴ tolerance or a chain product from the client's rule. Code read: the layer's per-bone weight pass and the clip-weight accessor, the layer's play routine, the skinning-palette builder and the deformation-pose builder. Nothing of the network, login, launcher or anti-tamper code. Result: §5.4 and §6.6 stand as written. |
| OBS3 | Observed in the user install, 2026-10-07, with the repository's decoders used as a tool: the root DID map's `WEENIECONTENT` map entry labelled `PlayerPhysicsControl` (key 0x10000162; game object 0x700304E2 here) and the complete property set of that object (0x790304E2 here, +0x09000000 rule), decoded to the last nested field; a scan of all 0x47 entity sets and all 0x78–0x7F property sets for `PlayerPhysicsControl_BasePlayerParameters` (one hit, that set) and for `GenericPhysicsController_RampTime`, `…_ForwardMoveSpeed`, `…_BackwardMoveSpeed`, `…_StrafeLeftMoveSpeed` as top-level properties (two hits, the sets named by the root map's `PHYSICSCONTROLLER` map); the dictionary default of `Movement_SpeedMultiplier`. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| K1 | An object's skeleton is the tagfile inside its own 0x04 setup; nothing names or shares a skeleton by reference (§2.1) | HD; OBS (28,555 setups) | Confirmed |
| K2 | A playable character's setup is found through the race table, the player-avatar object, `PhysObj` and the entity chain (§2.2) | OBS (all 15 gender entries of the 8 species) | Confirmed. MV's provisional Man/Elf assignment is confirmed. |
| K3 | Clip track *j* drives skeleton bone *j*; a clip fits a skeleton exactly when its track count equals the bone count (§3) | MV, ARCH, HD H16; OBS (frame-0 pose agreement on 24,839 clips, control test, 1,908 name-matched clips) | Confirmed (was Reported) |
| K4 | Mesh vertex bone indices address the bone array of the skeleton the mesh is drawn with; no remap exists (§4.1) | MV; OBS (2,625 setup submeshes, 116,768 worn-part checks) | Confirmed |
| K5 | The per-submesh influence list is redundant: per vertex in reverse order, the vertex's first bone index, once per non-zero weight (§4.2) | OBS (43,059 of 43,059) | Confirmed. Corrects MV ("palette that does not parse reliably"). |
| K6 | Inverse-bind matrices come from the skeleton reference pose; mesh vertices are stored in the reference-pose model space (§4.4–§4.5) | MV; OBS (692 setups, rigid-vertex proximity with control; skinning tests) | Confirmed |
| K7 | Skeleton space = mesh space: Z up, metres, right-handed, characters face +Y; no axis conversion (§5.1) | OBS | Confirmed |
| K8 | Reference-pose and clip quaternions are stored (x, y, z, w) and rotate actively; local = translate · rotate · scale; world = parent world · local (§5.2–§5.3) | HD (layout); OBS (arm/foot test, skinning tests, box bounds of `setup.md`) | Confirmed for unit scale. Scale: K17. |
| K9 | Clips carry no loop flag; 56 % of clips end on their first pose (§6.3) | OBS | Confirmed (statistic); looping choice belongs to `script-table.md` |
| K10 | The envelope weight array marks partial (layered) clips, typically upper-body only; no additive clips exist (§6.6) | HD (array); OBS (pattern census) | Confirmed by pattern, meaning partly Reported |
| K11 | Bone tracks are in place in 84 % of clips; locomotion cycles are treadmill clips; 112 clips carry extracted motion whose w is a yaw angle (§7) | HD; OBS | Confirmed |
| K12 | `attach_map` maps `BoneType` enum keys to bone indices (§8.1) | OBS (378 setups, 5,812 + 559 elements) | Confirmed. Resolves `setup.md` §6.2 OPEN. |
| K13 | Frame list A maps `HoldingLocation` enum keys to (bone, bone-local frame); it holds every held-item, sheath, effect and target point (§8.2) | OBS (2,737 setups, 25,118 elements) | Confirmed. Corrects MV ("attach transforms are not in the data"). |
| K14 | Frame list C is keyed by `UIViewport_Type` (§8.7) | OBS (54 elements) | Confirmed (key); use Reported |
| K15 | Held items are placed with their setup origin at the holding-location frame; weapons are modelled grip at origin, long axis +Y (§8.4) | MV; OBS (1,591 weapon setups) | Confirmed |
| K16 | Mesh `flags` (mesh.md §3.3) is a `MeshTypeID` enum value: the body or equipment slot of the part (§9.2) | OBS (47,563 of 47,563 records; 80,974 of 81,098 worn parts) | Confirmed |
| K17 | Bone transforms compose as (translation, rotation, scale) triples: a parent's scale never moves a child joint, scales multiply per axis down the chain, and the skinning transform scales a vertex's offset from its own joint (§5.4) | BIN (pose composition and skinning-palette code); RH (River-hobbit feet stay within 1 cm of the ground with it, float 8.5–8.8 cm with matrix composition) | Confirmed 2026-10-05 (BIN-derived rule, checked against real data by RH). Replaces the matrix Decision of §5.4. |
| K18 | Simultaneous clips combine per bone by layer priority with a remaining-weight budget, weights quantised to 1/255, then a normalised weighted average with a reference-pose fill below a total of 0.1 (§6.6) | BIN (animation player and pose-sampling code); BIN5 (re-read 2026-10-07: each clip enters with its own overall weight, and 255 × weight is converted to an integer by truncation with no tolerance) | Confirmed 2026-10-05 (BIN only); re-checked 2026-10-07 |
| K19 | Skinned parts of figures with body-shape deformers or bone sliders are drawn from vertices deformed once in bind space (`appearance.md` §6.4); the animated pose itself carries no slider | BIN; RH | Confirmed 2026-10-05 (BIN-derived, checked by RH) |
| K20 | The client binds clip track *j* to bone *j* with an identity track-to-bone table and refuses (with an error) a clip whose track count differs from the skeleton's bone count (§3.2, §3.4) | BIN3 | Confirmed 2026-10-07 (BIN; agrees with K3) |
| K21 | A clip's weight ramps **linearly in time**: from 0 to its target weight over `RampIn`, and from its current weight to 0 over `RampOut`; a `RampOut` below 0.0002 s removes the clip at once; a one-shot clip begins its ramp-out `RampOut` seconds before its end; clips started by an entity's start-up scripts skip their ramp-in (§6.5) | BIN3 | Confirmed 2026-10-07 (BIN only) |
| K22 | A change of `UseBoneOpacity` on a playing clip moves the mixing factor *m* linearly over **0.2 s**, a constant of the client (§6.6) | BIN3 | Confirmed 2026-10-07 (BIN only) |
| K23 | The reference-pose fill of §6.6 step 4 applies when the total weight is **at most** 0.1, and its rotation is sign-aligned with the weighted sum of the contributions so far (§6.6) | BIN3 | Confirmed 2026-10-07 (BIN only) |
| K24 | The client never applies a clip's extracted-motion reference frame to the entity; the frame's w is a rotation about the frame's stored `up` vector (right-handed) (§7.3) | BIN3 | Confirmed 2026-10-07 (BIN; "never applies" = no code path in the animation component or its Havok calls reads it) |
| K25 | Clip playback rate is the node's `Speed` × `SpeedMultiplier` (with `Speed` driven by a property); it is never rescaled to the measured ground speed. The avatar's ground speed is a per-direction move speed × `Movement_SpeedMultiplier`, mixed by the input direction (max(\|*x*\|, \|*y*\|) taken before \|*y*\| is doubled for the mix) (§7.4). The `Movement_…MoveSpeed` and `Physics_Avatar…MoveSpeed` properties are unset in the data; the speeds the player's controller uses come from the data by another path (K28) | BIN3; BIN4; OBS2 | Confirmed 2026-10-07 (rule BIN; unset properties OBS2). Revised 2026-10-07: the earlier "those move speeds are not in the data files" was wrong (K28). |
| K26 | Skinning matrix = T·R·S of the triple (*t* + *q*·(*s* ⊙ *X*ₜ), *q*·*X*_q, *s* ⊙ *X*ₛ), with the inverse reference triple *X* = (−(*q*_r⁻¹ applied to *t*_r), *q*_r⁻¹, 1/*s*_r): the inverse translation is **not** divided by the reference scale (§5.4) | BIN3; BIN5 (palette builder re-read 2026-10-07) | Confirmed 2026-10-07 (BIN only). Corrects the *X*_b translation of 2026-10-05. |
| K27 | The executable contains no runtime cloth classes (only serialization names of Havok cloth types), never creates a ragdoll instance, and its animation component has no secondary-motion step: capes and skirts move only with their clip tracks (§13) | BIN3 | Confirmed 2026-10-07 (BIN only) |
| K28 | The player's own movement controller is configured from the game object that the `WEENIECONTENT` DID map labels `PlayerPhysicsControl`: its `PlayerPhysicsControl_BasePlayerParameters`, overlaid in a fixed order by state-dependent parameter structures (troll, mounted, walking, slide, ice, falling, slope, swimming, frozen). The merged `GenericPhysicsController_…MoveSpeed` fields are the per-direction speeds, and the merged `GenericPhysicsController_RampTime` is the slider time *T* of the player's input path (§7.4) | BIN4; OBS3 | Confirmed 2026-10-07 (path, order and field use BIN; values OBS3, read at run time) |
| K29 | Starting or stopping a clip changes no other clip's overall weight *c*; a layer has no limit on its number of clips (at 1,000 the client logs a warning and still plays the new clip) (§6.5) | BIN5 | Confirmed 2026-10-07 (BIN only) |
| K30 | The keyboard's input vector per gait and the resulting walking speeds (§7.4, §15.1 O14.4) | `collision.md` §11.8 (BIN questions 60–61, C60–C61, 2026-10-08) | BIN |

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

> **OPEN** (owner observation, confirmation only): the data and BIN predict that a
> River-hobbit avatar is drawn at entity scale 0.7, further lowered by the `scale_blend`
> layer and reshaped by its body-shape deformer and fixed leg sliders, for a standing height
> of 1.23 m (male) and 1.20 m (female) at the default height value (`appearance.md` §6.5). A
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

The client does the same (K20, BIN3): when it plays a clip it builds a binding whose
track-to-bone table is the identity (track *j* → bone *j*), and a clip whose track count
differs from the bone count of the entity's setup is not played and is reported as an error.

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

The product **P_b** · **B_b**⁻¹ is exact while every scale is 1. With non-unit scales (a clip
track, a body-shape deformer or a bone slider) the client forms the skinning transform from
(translation, rotation, scale) triples instead, by the rule of §5.4 (K17, K26); the two
agree whenever all scales are 1. For the few bones whose reference scale is not 1 the
client's skinning at the reference pose is **not** the identity (§5.4, last paragraph).

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
  forward pass suffices. This matrix form is exact when every scale is 1; with scale the
  client composes triples as §5.4 states, which differs from the matrix product.
- Composing in the other order (**L**_b · **W**_{parent}) breaks the mirror symmetry of the
  89-bone skeletons (13 cm error) and tears posed meshes as badly as a shuffled track order
  (99th-percentile edge stretch 25 – 43 times the rest length).
- The model transform of the object (its world placement, `entity-descriptor.md` §5) is
  applied after the skeleton: world = object placement · **W**_b.

<a id="s5-4"></a>
### 5.4 Scale (Confirmed occurrence; composition rule BIN, K17)

Non-unit scale is rare: 224 reference-pose bones in 182 setups (137 uniform), and at frame 0
non-unit scale on some track of 1,217 clips (1,047 of them non-uniform); in 296 of those clips
a scaled bone has a child with a non-zero translation, so inheritance matters there. Tests
on 86 such clips with skinned meshes could not tell the candidate rules apart (edge-stretch
medians equal). The client's code settles it (BIN). Character bodies use scale heavily
through body-shape deformers and bone sliders (`appearance.md` §6.3–§6.4) and the
River-hobbit `scale_blend` layer (`script-table.md` §6.8).

**Composition rule.** Every local and model transform is a triple (translation *t*,
rotation *q*, per-axis scale *s*). For a bone *b* with parent *p*:

| Model component | Rule |
|---|---|
| translation | *t*_b = *t*_p + *q*_p applied to the local translation. The parent's scale is **not** applied: scaling a bone never moves its children's joints. |
| rotation | *q*_b = *q*_p · local rotation (as §5.2–§5.3). |
| scale | *s*_b = *s*_p × local scale, component by component (x with x, y with y, z with z), whatever the rotations in between. |

A root bone's model triple is its local triple. With all scales 1 this is exactly §5.3.

**Inverse reference triple** (revised 2026-10-07, K26). From the reference pose composed by
the same rule, bone *b*'s reference model triple (*t*_r, *q*_r, *s*_r) gives the triple
*X*_b = (translation −(*q*_r⁻¹ applied to *t*_r), rotation *q*_r⁻¹, scale 1/*s*_r, per axis).
The client computes it exactly so: the translation is **not** divided by the reference
scale, so *X*_b is the exact inverse only when *s*_r = (1, 1, 1). (The 2026-10-05 text
divided the translation by *s*_r; that was wrong.)

**Skinning transform** (revised 2026-10-07, K26). For a pose with model triple (*t*, *q*,
*s*) of bone *b*, the skinning triple is (translation *t* + *q* applied to (*s* ⊙ *X*_b's
translation), rotation *q* · *X*_b's rotation, scale *s* ⊙ *X*_b's scale), where ⊙ is the
per-axis product; the skinning matrix is T · R · S of that triple (§5.3 order), so a
bind-space vertex *v* lands at

  *t* − *q* applied to (*s* ⊙ (*q*_r⁻¹ applied to *t*_r)) + (*q* · *q*_r⁻¹) applied to
  ((*s* ⊙ (1/*s*_r)) ⊙ *v*).

The scale acts on *v* along the **model (bind) axes**, before the rotation *q* · *q*_r⁻¹.
For a bone whose reference rotation is the identity and whose reference scale is 1 (every
player-skeleton bone) this reduces to *t* + *q* applied to (*s* ⊙ (*v* − *t*_r)): the
vertex's offset from the bone's reference joint is scaled per axis by the bone's **model**
scale, then rotated and moved to the posed joint. So a scaled bone shrinks or grows the
vertices bound to it and to its descendants (whose model scale inherits the factor), around
each bone's own joint, while every joint stays where the translations and rotations put it.
The two readings that the 2026-10-05 text allowed differ only for a bone with a non-identity
reference rotation and a non-uniform model scale; the client uses the one above.

Consequence for bones whose **model reference scale is not 1** (the 224 bones in 182 setups
with a non-unit local reference scale, first paragraph of this section, and their
descendants): at the reference pose the skinning transform is not the identity. A vertex
bound to such a bone lands at *t*_r − *q*_r applied to (*s*_r ⊙ (*q*_r⁻¹ applied to
*t*_r)) + *v*; with a uniform reference scale σ that is *v* + (1 − σ) *t*_r. An
implementation that wants the client's look reproduces this offset; KI8 (unit scales) is
unaffected.

Evidence and consequences (RH, K17): with this rule the River-hobbit bodies, whose thighs and
knees carry fixed scales (`appearance.md` §6.3), keep their lowest vertex within 1 cm of
the ground at every height value; with matrix composition on the animated pose the feet float
8.5 cm (male) and 8.8 cm (female) at model scale 0.7, which is the float the M4 viewer showed
(`docs/checklists/m4-visual.md` note 1). Tessera's earlier choice of full matrix
composition is withdrawn.

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
than 1 come from the playing system: the node's `Speed` × `SpeedMultiplier`, with `Speed`
often driven by a property (`script-table.md` §6.1, §8.5). The client never adapts the rate
to the ground speed (§7.4, K25).

<a id="s6-5"></a>
### 6.5 Normalisation and shortest path

- Every sampled rotation is normalised (spline evaluation already does this,
  `havok-data.md` §7.6).
- **Blending poses**: the original never interpolates two poses directly; every clip that
  is playing contributes to a per-bone weighted average (§6.6, K18). Translations and scales
  average linearly; rotations are summed as weighted quaternions and normalised (a
  normalised linear blend), each quaternion first negated when it points away from the
  others (shortest path). For the reference-pose fill of §6.6 step 4 the test is against the
  weighted sum of the contributions accumulated so far (K23, BIN3); for the clip
  contributions themselves the same running-sum test is Reported (inside the Havok blending
  routine, not traced). Cross-fades are the same average while one clip's weight falls and
  the other's rises.

<a id="s6-5-1"></a>
**Ramps and cross-fades (K21, BIN3; answers the earlier OPEN on cross-fade durations).**
Every playing clip carries one overall weight *c* (step 2 of §6.6). It changes only in these
ways, and every change is **linear in time** (no ease curve):

| Event | What *c* does |
|---|---|
| Clip started | Rises from 0 to the target weight (the node's intensity, normally 1) over the node's `RampIn`. A `RampIn` of 0 starts at the target. Clips started while an entity runs a **start-up** script (`script-table.md` §8.1) skip the ramp and start at the target. |
| Clip stopped (its script branch stops, or a one-shot clip reaches its end) | Falls from its current value to 0 over the node's `RampOut`; then the clip is removed. A `RampOut` below 0.0002 s removes it at once. A one-shot clip's stop is scheduled `RampOut` seconds **before** its end, so the fade finishes exactly at the end of its played range (`script-table.md` §6.10). |
| The node's intensity changes (driven property, ScalarBlend weight, a Switch cross-fade of its branch) | Jumps to the new value at once, and a ramp-in still in progress is abandoned. |

Starting or stopping another clip, on the same layer or on another, is not one of these
events: it changes no other clip's overall weight *c* (lower layers still receive what a
higher layer's clips leave, §6.6), and each stopped clip falls from its own current
value over its own node's `RampOut` (BIN5, K29). A layer has no limit on its number of clips;
the client logs a warning when one holds 1,000 and still plays the new clip.

So the cross-fade durations that exist are: a clip's own `RampIn` / `RampOut`
(`script-table.md` §6.1), the `RampOut` of the Switch case being left, which fades the whole
outgoing branch against the incoming one (`script-table.md` §7.1; 0.2 s for walk ↔ run in
the locomotion templates), and the time the movement code takes to move a ScalarBlend
slider (`script-table.md` §8.4: a constant rate of 1/*T* per second; for the player's own
input *T* is the controller's `RampTime`, §7.4). No other blend time exists in the animation
layer.

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
- **Combination rule (BIN, K18).** Every playing clip *i* has an overall weight *c_i*
  (its linear ramp envelope of §6.5: rising from 0 to the node's intensity, normally 1,
  over `RampIn`, falling over `RampOut`; `script-table.md` §6.1) and a per-track **opacity** *o_ij*: the
  clip's track weight *j* when its animation node has `UseBoneOpacity` = 1 and the clip has a
  weight array, otherwise 1 (`script-table.md` §6.5). For each bone *j* separately:
  1. Set the remaining weight *r* = 1.
  2. Visit the layers (`script-table.md` §6.2) from the **highest number to the lowest**.
     Every clip *i* on the current layer gets *w_ij* = *c_i* × *r* × *o_ij*; all clips of one
     layer use the same *r*. After the layer, *r* = max(0, *r* − the sum of that layer's
     *w_ij*). Lower layers therefore only get what higher layers leave.
  3. Quantise every weight: *w_ij* becomes ⌊255 × *w_ij*⌋ / 255 (truncation; the
     unquantised values were used in step 2). The client multiplies the single-precision
     weight by 255 and converts to an integer toward zero, with no tolerance added
     (BIN3).
  4. If the bone's total *W* = Σ *w_ij* is **at most 0.1**, add the bone's reference
     transform with weight 0.1 − *W* (K23; at exactly 0.1 that weight is 0). Its rotation
     is negated first when its dot product with the weighted quaternion sum accumulated so
     far is negative.
  5. The bone's local transform is the weighted average of the contributions: Σ *w* ·
     translation / Σ *w*, the same for scale, and the normalised weighted quaternion sum
     (§6.5).

  Consequences: a full-body clip on a higher layer with *c* = 1 replaces lower layers
  completely; an upper-body clip (legs 0.01) over a walk leaves the legs walking, because the
  walk gets 0.99 of the leg weight; a partial clip with **nothing below it** is pulled
  towards the reference pose on its weak tracks (track weight 0.01 → weight 2/255 plus
  0.0922 of reference pose). A layered weight of 0.01 over a full base is effectively
  2 / (2 + 252) = 0.00787, not 0.01 (worked example §14.6, §14.8).
- **Additive clips do not exist**: no clip has near-zero translations and identity rotations
  on its tracks (3 clips come close, and they bind to skeletons whose own reference
  translations are zero), and no blend-mode datum exists (`havok-data.md` §4.6). Every clip
  is an absolute pose clip.

> **OPEN** (owner observation, confirmation only): the rule above predicts that an upper-body
> attack played while running leaves the legs running, and that the same attack played while
> standing moves the whole body (in 7,722 nodes `UseBoneOpacity` is driven by
> `ScriptSystem_CurrentlyMoving`, so the opacity applies only while moving). Visible check:
> attack while running, then while standing. Not needed to implement.

The `UseBoneOpacity` flag can change while a clip plays (it is driven by
`ScriptSystem_CurrentlyMoving` in 7,722 nodes): the player then moves a mixing factor *m*
between 0 (opacity used) and 1 (opacity ignored) and uses *o_ij* + *m* × (1 − *o_ij*) as the
track's opacity while *m* < 0.9998.

**Duration of the change (K22, BIN3; answers the earlier OPEN).** When the flag changes,
*m* moves **linearly from its current value to the new target** (0 when the flag becomes 1,
1 when it becomes 0) over **0.2 s**, a constant of the client, not a data field. A clip
without a weight array has nothing to mix and ignores the change. A clip starts with *m* = 0
when its node has `UseBoneOpacity` = 1 and the clip has weights, otherwise with *m* = 1.

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
- **Meaning of a sample** (BIN3): the client's evaluation of a reference frame at time *t*
  interpolates (x, y, z, w) linearly between the two neighbouring samples (first sample
  before 0, last sample at or after the frame's duration) and forms the transform
  "translate by (x, y, z), rotate by the angle w about the frame's stored `up` vector"
  (right-handed: a positive w turns counter-clockwise when `up` points at the viewer, so with
  `up` = +Z a positive w is a left turn for a character facing +Y). The stored `up` is used,
  not the world up axis (7 leading frames store (0, 1, 0), `havok-data.md` §4.5).
- **Use by the client** (K24, BIN3): **none.** The client's animation component never reads
  a clip's extracted motion: per playing clip it only advances the clip's time and samples
  its bone tracks, and none of its calls into the Havok runtime queries a reference frame.
  An entity's placement and heading come from the movement system (and, for locomotion
  clips, `LocomotionDisplacement`, `script-table.md` §6.3), never from these frames. The
  turn-in-place clips of the list above therefore show only their bone motion (which ends
  where it starts) while the movement system turns the entity.
- **Tessera rule**: the world view and the client must not move or turn entities by
  extracted motion. A viewer may offer it as a debug overlay; when it does, it applies the
  change of the sampled frame since the previous update (rotate about the stored `up` by Δw,
  translate by Δ(x, y, z) in the entity's frame at the cycle's start, restart the reference
  each loop), holds the last sample when the frame has fewer samples than the clip has
  frames (3 cases), and scales Δ(x, y, z) by the entity's model scale. These choices are
  Tessera's (no client behaviour to match). When both a leading and an embedded copy exist
  they are identical (`havok-data.md` §4.5); use either.

> **Answered** (2026-10-07, BIN3; was: the sign of w, owner observation). A positive w is a
> right-handed rotation about the stored `up` (counter-clockwise seen from the tip of `up`).
> The question no longer affects rendering, because the client never applies the frame.

> **Answered** (2026-10-07, BIN3; was: implementation OPEN on scaling the extracted
> translation by the model scale, and on loops and the `up` axis in the implementer notes of
> §15). The client applies no extracted motion, so the M4 viewer's choices (scale by model
> scale, restart per cycle, world up) are a debug display only; for that display use the
> stored `up` (rule above). In the world, do not apply it.

<a id="s7-4"></a>
### 7.4 Locomotion speed of treadmill clips (Confirmed estimates; rule and configured speeds BIN)

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

**Rate matching (K25, BIN3; answers the first half of the earlier OPEN).** The client does
**not** match the clip rate to the ground speed:

- A clip's playback rate is its node's `Speed` × `SpeedMultiplier` (`script-table.md` §6.1),
  where `Speed` is usually driven by `Movement_SpeedMultiplier` (`script-table.md` §8.5).
  Nothing in the animation layer reads the entity's measured velocity.
- The avatar's ground velocity comes from the movement code (BIN3, BIN4): for the input
  direction (strafe *x*, propel *y*, each in [−1, 1]) it takes the forward speed when
  *y* > 0, otherwise the backward speed, and the left strafe speed when *x* > 0, otherwise
  the right strafe speed, each multiplied by `Movement_SpeedMultiplier` (dictionary default
  1.0). It first takes *m* = max(\|*x*\|, \|*y*\|) from the undoubled values; then, only
  for the mix, when both \|*x*\| and \|*y*\| are at most 0.5 it doubles \|*y*\| (call the
  result *y*′). The speed is (\|*x*\| × strafe speed + *y*′ × forward-or-back speed) /
  (\|*x*\| + *y*′) × *m*, along the normalised direction (*x*, *y*); with *m* at most
  0.0002 the velocity is 0.
- **Where the player's speeds come from (K28, BIN4, OBS3).** The `Movement_…MoveSpeed` and
  `Physics_Avatar…MoveSpeed` properties are unset in the data (OBS2; `Physics_Avatar…` default
  to −1) and the client does not use them for this. Instead, the client builds the
  configuration of the player's own movement controller from the game object that the root
  DID map's `WEENIECONTENT` map labels `PlayerPhysicsControl`, using the current values of
  the properties named below; it repeats this as the player's state changes and applies the
  result only when it differs from the previous one (exactly when the step runs was not
  traced). It starts from that object's `PlayerPhysicsControl_BasePlayerParameters`
  and lays these structures over it, in this order, each replacing only the fields it
  contains:
  1. `PlayerPhysicsControl_TrollParameters` when `Agent_Species` is 47 (`Troll`);
  2. when `Mount_Is_Mounted` is true: `PlayerPhysicsControl_WarsteedParameters` if
     `Mount_Movement_Use_RampUp_Controller` is true, else `PlayerPhysicsControl_MountedParameters`;
  3. when `Movement_MovementStyle` is 1 (`Walking`): `PlayerPhysicsControl_MountedWalkingParameters`
     if mounted, else `PlayerPhysicsControl_WalkingParameters`;
  4. when `Physics_OnSlide` is true: `PlayerPhysicsControl_SlideParameters`, and steps 5 to 8
     are skipped;
  5. otherwise `PlayerPhysicsControl_IceParameters` when `Physics_OnIce` is true;
  6. `PlayerPhysicsControl_FallingParameters` (`…_StealthedFallingParameters` when
     `Stealth_IsStealthed` is true) when `Entity_Element` is 4 (no surface);
  7. `PlayerPhysicsControl_SlopeParameters` when `Entity_Element` is 6 (uncontrolled surface);
  8. `PlayerPhysicsControl_SwimmingParameters` (`…_StealthedSwimmingParameters` when stealthed)
     when `Entity_Element` is 1 to 3 (water);
  9. `PlayerPhysicsControl_FrozenParameters` when `Movement_ActiveMovementFrozen` is true.

  Each structure holds a `PlayerPhysicsControl_MovementConfiguration` structure, whose
  `GenericPhysicsController_…` fields are what the merge combines, and may hold a
  `PlayerPhysicsControl_MoveToConfiguration` (not used by this spec). The merged
  `GenericPhysicsController_ForwardMoveSpeed`, `…_BackwardMoveSpeed`, `…_StrafeLeftMoveSpeed`
  and `…_StrafeRightMoveSpeed` are the speeds of the rule above (metres per second), and the
  merged `GenericPhysicsController_RampTime` is the slider time *T* of `script-table.md` §8.4.
  The controller is of type 3 (`Generic`), or 0x10000001 (`Mounted`) when
  `Mount_Movement_Use_RampUp_Controller` is true; the slider and speed steps described here
  were traced for type 3 only. Observed values (this install; read them at run time):

  | Structure | Forward | Backward | Strafe left / right | `RampTime` (s) |
  |---|---|---|---|---|
  | Base | 6.5 | 3.25 | 5.0 / 5.0 | 0.13 |
  | Walking overlay | 5.0 | 4.5 | 4.5 / 4.5 | 0.25 |

  The other overlays set `RampTime` to 0.25 (mounted, warsteed, mounted walking, frozen),
  0.5 (slide, ice), 0.01 (falling), 0.1 (slope) and 0.4 (swimming); the troll overlay sets
  only the backward speed (6.0). So with a full forward input (*y* = 1, *x* = 0) and
  `Movement_SpeedMultiplier` 1, a running avatar on the ground moves at 6.5 m/s, close to the
  run clips' natural speed of about 6.6 m/s (89-bone male, table above), and backwards at
  3.25 m/s (clips: about 2.7 m/s). Which input vector the keyboard produces in run and in walk
  mode was not traced (OPEN below); a full-magnitude walk input would give 5.0 m/s, more than
  twice the walk clips' natural speed of about 2 m/s.
- Consequence: the feet slide whenever the ground speed differs from the clip's natural speed
  (\|`LocomotionDisplacement`\| / duration, `script-table.md` §6.3) × rate; nothing corrects
  it.
- Scripts can install their own movement controller: 127 PlayerPhysics nodes (kind 47,
  `script-table.md` §5.2, all of controller type 3) carry `GenericPhysicsController_…`
  properties, among them `ForwardMoveSpeed` (0 in 80 nodes; otherwise 2.0, 2.42, 3.0, 3.2,
  3.52, 5.0, 6.5 or 7.15), `BackwardMoveSpeed`, `StrafeLeftMoveSpeed`,
  `StrafeRightMoveSpeed` and `RampTime` (0.01 – 0.4 s) (OBS2, 2026-10-07). Such a controller
  is created fresh and takes the node's properties over the controller class's built-in
  defaults (BIN4; for type 3: each horizontal speed 4.5, `RampTime` 0.1 s). Which situations
  start these nodes was not traced. The root DID map also has a `PHYSICSCONTROLLER` map
  (controller type → property set; `Generic` → a set with forward 4.5, backward 2.5,
  `RampTime` 0.3, OBS3); neither path traced here reads it, and its use is OPEN.

The animation component also offers a **locomotion velocity** computed from its own clips
(BIN3): every layer that holds looping clips with a non-zero `LocomotionDisplacement`, a rate
above 0.0002 and no fade-out in progress contributes the weighted mean of |displacement| ×
rate / duration over those clips (weights = their current overall weights) along the
normalised weighted sum of their displacement directions, times the time step; the layers'
contributions add, and a warning is logged when more than one layer contributes. The displacement used is the node's value multiplied by the mean
of the entity's three scale components (`script-table.md` §6.3). Which movers use this
velocity was not traced (the avatar's controller of the list above does not).

> **Answered 2026-10-08** (owner observation O14.4; was OPEN): the input vector is traced
> (`collision.md` §11.8, BIN). The keyboard gives (0, ±1) or (±1, 0) when running; walk mode
> halves the command values, which the client quantises to (0, ±0.5) forward and backward
> and (±0.57, 0) sideways and which also select the Walking layer. Ground speeds with
> `Movement_SpeedMultiplier` 1 and this install's layers: running 6.5 m/s forward, 3.25 m/s
> backward, 5.0 m/s sideways; walking **2.5 m/s** forward, **2.25 m/s** backward, **2.565 m/s**
> sideways (the table in `collision.md` §11.8). The walking speed is thus close to the walk
> clips' natural speed of about 2 m/s, and feet slide by the difference (no rate matching,
> above). A `/loc` timing by the owner is now only a confirmation.

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
   **B**_b and the inverse reference triples *X*_b (§5.4) once per distinct skeleton.
2. **Clips**: for each clip layer chosen by the playing system, check compatibility (§3.4),
   compute the clip time (§6.3, §6.4; held clips take their time from a value instead,
   `script-table.md` §6.7) and sample every track (`havok-data.md` §7.7–§7.8) into local
   (translation, rotation, scale) per bone.
3. **Blend** all playing clips per bone by the layered-weight rule (§6.6), each clip with
   its current overall weight (linear ramps, §6.5). Bones with a total weight of at most 0.1
   are pulled to (or, with no weight, keep) their reference local transform. No bone slider
   is applied here (`appearance.md` §6.4).
4. **Root motion**: none from the clips. Extracted-motion reference frames are not applied
   (§7.3, K24); the object's placement comes from the movement system. Bone tracks are used
   as sampled.
5. **Model transforms**: one pass from root to leaves (§5.3, with scale by §5.4).
6. **Palette**: for each bone, the skinning transform of §5.4 (equal to model transform times
   inverse reference transform when all scales are 1, §4.4); upload up to 256 matrices
   (§4.3).
7. **Draw** every part in the setup's model space with the object's world placement; skinned
   submeshes use the palette, unskinned parts are rigid (§9.1). Figures with body-shape
   deformers or bone sliders draw their skinned parts from bind-space-deformed vertices,
   computed once when those inputs change (`appearance.md` §6.4).
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
| **KI8** | With every scale equal to 1, the triple composition and skinning of §5.4 give the same model transforms and skinned positions as the matrix form of §4.4 and §5.3 (a property test on synthetic skeletons; real reference poses differ only where a reference scale is not 1). |
| **KI9** | The root DID map's `WEENIECONTENT` map has an entry labelled `PlayerPhysicsControl` whose object's property set holds `PlayerPhysicsControl_BasePlayerParameters` with a `PlayerPhysicsControl_MovementConfiguration` that sets all four horizontal `GenericPhysicsController_…MoveSpeed` fields and `GenericPhysicsController_RampTime` (OBS3). |

KI1–KI4 and KI7 are checks a decoder or loader may run and report; KI5 and KI6 are census
facts that tests may assert on real data; KI8 is a property test; KI9 is a real-data check for the speed source of §7.4.

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
| Cape and skirt bones may need runtime cloth simulation | MV | Clips animate them: cape bone rotations are animated in 90 – 96 % of compatible clips, skirt bones in 48 – 61 %. The original adds nothing on top (K27, BIN3, 2026-10-07): the executable has no runtime cloth classes (only the serialization names of Havok cloth types), never creates a ragdoll instance, and its animation component goes from blended local pose to model pose to skinning palette with no secondary-motion step. |
| Decision: full matrix composition for scale until scale inheritance is observed | this spec §5.4 (before 2026-10-05) | The client composes (translation, rotation, scale) triples: parent scale never moves child joints; skinning scales a vertex's offset from its own joint (§5.4, BIN). |
| Recommended layering "base + w · (layer − base)"; a weighted clip alone plays its full pose | this spec §6.6 (before 2026-10-05) | Remaining-weight budget from the highest layer down, 1/255 truncation, normalised average, reference fill below 0.1 (§6.6, BIN). |
| Inverse reference translation −(1/*s*_r) × (*q*_r⁻¹ applied to *t*_r); two readings of the skinning transform | this spec §5.4 (2026-10-05) | The translation is not divided by the reference scale; the skinning matrix is T·R·S of the skinning triple, which scales along the bind axes (§5.4, K26, BIN3). |
| Extracted motion moves the entity (§7.3 "Use") | this spec §7.3 (2026-10-05) | The client never applies it (K24, BIN3); Tessera keeps it as a debug overlay only. |
| Reference fill "below 0.1" | this spec §6.6 (2026-10-05) | At most 0.1 (K23, BIN3). |

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

A 4-track base pose (layer 10, overall weight 1, no opacity) with every rotation identity,
and a layer-40 clip whose tracks 2 and 3 are 90° about z, with track weights
(1, 1, 0.01, 0.5), `UseBoneOpacity` 1, overall weight 1 (§6.6):

| Track | Layer weight → quantised | Remaining for the base → quantised | Result |
|---|---|---|---|
| 0, 1 | 1 → 255/255 | 0 → 0 | the layer's values |
| 2 | 0.01 → 2/255 | 0.99 → 252/255 | 0.639° about z (normalised sum of identity × 252 and 90° × 2) |
| 3 | 0.5 → 127/255 | 0.5 → 127/255 | exactly 45° about z (equal weights) |

The same layer clip with **no base playing**: track 2 gets 2/255 and the reference pose fills
0.1 − 2/255 = 0.0922, so its share is 0.0784 of the way to the layer's value.

<a id="s14-7"></a>
### 14.7 Scale composition and skinning (§5.4)

Three bones, reference rotations identity and scales 1: bone 0 (root) at (0, 0, 1); bone 1
(parent 0) local translation (0, 0, −0.45); bone 2 (parent 1) local (0, 0, −0.41). Reference
joints: 1.0, 0.55, 0.14 in z. Pose: reference translations, bone 0 local scale (1, 1, 0.8),
bone 1 local scale (1, 1, 1.15).

| | Rule of §5.4 | Matrix composition (wrong) |
|---|---|---|
| Joint heights (bones 0, 1, 2) | 1.0, 0.55, 0.14 (unchanged) | 1.0, 0.64, 0.2628 |
| Model scale z (bones 0, 1, 2) | 0.8, 0.92, 0.92 | — |
| Vertex at z 0.775 on bone 0 | 1.0 + 0.8 × (−0.225) = 0.82 | 0.82 |
| Vertex at z 0.345 on bone 1 | 0.55 + 0.92 × (−0.205) = 0.3614 | 0.4514 |
| Vertex at z 0.0 on bone 2 (the sole) | 0.14 + 0.92 × (−0.14) = 0.0112 | 0.1340 (floats 12 cm) |

<a id="s14-8"></a>
### 14.8 Three layers on one bone

One bone; clips on layers 110 (overall weight 1, opacity 0.01), 18 (overall 1, opacity 0)
and 10 (overall 1, opacity 1), sampled translations z = −10, 5 and 0. Layer 110: *w* = 0.01,
*r* becomes 0.99. Layer 18: *w* = 0, *r* stays 0.99. Layer 10: *w* = 0.99. Quantised: 2/255
and 252/255. Result z = (2 × −10 + 252 × 0) / 254 = −0.07874.

---

<a id="s14-9"></a>
### 14.9 Ramps and a Switch cross-fade (§6.5)

A clip started at time 0 with `RampIn` 0.2 and intensity 1 has overall weight 0.25 at 0.05 s,
0.5 at 0.1 s and 1 from 0.2 s on. Stopped at 3.0 s with `RampOut` 0.3, its weight is 0.5 at
3.15 s and it is removed at 3.3 s. A one-shot clip of duration 2.0 s, rate 1, `LoopCount` 1,
`RampOut` 0.25, started at 0, begins its ramp-out at 1.75 s and is gone at 2.0 s.

A Switch leaves a case whose `RampOut` is 0.2 at time 10.0 for a case with `Delay` 0. From
10.0 to 10.2 the outgoing branch's intensity is (10.2 − *t*) / 0.2 and the incoming branch's
is 1 minus that: at 10.05 they are 0.75 and 0.25. The clips of both branches take these
values as their weights at once (§6.5 table, third row). At 10.2 the outgoing branch is
stopped. If the property changes back to the first case at 10.05, the branch that was
coming in (now at 0.25) becomes the outgoing one: the new fade uses the `RampOut` of the case
now being left and is placed in time so that this branch continues from 0.25 down to 0; the
first case's old branch, still fading at 0.75, is stopped at once, and that case's branch is
started again and rises as the complement (`script-table.md` §7.1).

<a id="s14-10"></a>
### 14.10 Skinning with a reference scale (§5.4, K26)

One root bone with reference translation *t*_r = (0, 0, 1), identity rotation and uniform
reference scale σ = 0.5. Inverse reference triple *X* = ((0, 0, −1), identity, (2, 2, 2)).
At the reference pose (*t* = (0, 0, 1), *q* identity, *s* = (0.5, 0.5, 0.5)) the skinning
triple is ((0, 0, 1) + (0.5 ⊙ (0, 0, −1)), identity, (1, 1, 1)) = ((0, 0, 0.5), identity,
(1, 1, 1)): a vertex at (0, 0, 1) lands at (0, 0, 1.5) = *v* + (1 − σ) *t*_r. With the exact
inverse ((0, 0, −2), identity, (2, 2, 2)) it would stay at (0, 0, 1); a test that expects
that has the 2026-10-05 rule.

---

<a id="s15"></a>
## 15. Open questions

> **OPEN** (owner observation, confirmation only): River-hobbit avatar size relative to its
> setup (§2.2). The data and BIN now predict it (`appearance.md` §6.5); only a visual check
> remains.

> **Resolved** (2026-10-05, BIN): scale inheritance (§5.4) and the combination of layered
> clips (§6.6).

> **Resolved** (2026-10-07, BIN3): ramp shape and every cross-fade duration (§6.5, K21); the
> duration of a `UseBoneOpacity` change (§6.6, K22); the sign and the use of extracted-motion
> yaw (§7.3, K24: never applied); rate matching (§7.4, K25: none); cloth and secondary motion
> on capes and skirts (§13, K27: none); the exact skinning transform and inverse reference
> triple (§5.4, K26).

> **OPEN** (owner observation, confirmation only): an upper-body attack while running keeps
> the legs running, the same attack while standing moves the whole body (§6.6).

> **OPEN** (owner observation O14.4, narrowed 2026-10-07): the input vector the keyboard gives
> in run and walk mode, and so the walking ground speed; the running speed is predicted from
> the data (6.5 m/s forward, §7.4) and needs only a confirmation.

> **OPEN:** the slider and speed steps of the `Mounted` controller type (0x10000001), and the
> use of the root DID map's `PHYSICSCONTROLLER` map (§7.4).

> **OPEN** (for `script-table.md` and owner observation): in-world default holding location
> per equipment slot, and drawn versus sheathed placement per weapon class (§8.5).

> **OPEN:** frame list B (§8.7); `Render_MeshCombiningPoseAnim` (§3.4); the 554 clips that
> fit no skeleton of the install by pose and the 2 clips without a same-count skeleton (§3.3).

> **OPEN** (low priority): whether the client renormalises vertex weights before skinning
> (not traced; Tessera does not, see the answer below).

None of these blocks rendering an animated character; they affect fidelity of playback
choices in `tessera-anim`.

Raised by the clean-room implementation (`tessera-anim`, 2026-10-05). The real-data run
built all 28,555 skeletons (999 distinct by bit-exact content), bound and sampled 25,393 of
25,395 clips (the 2 without a same-count skeleton are 0x05002255 and 0x05004AD7, as KI5
says), reproduced the §3.3 agreement buckets within a few clips (1,222 / 18,737 / 4,883 /
551 with skeletons grouped bit-exactly instead of to 1e-4), found KI1 and KI2 true on all
2,625 setup submeshes, and found no non-finite posed matrix. Answers (spec writer,
2026-10-07):

> **Answered** (was: reference-pose skinning tolerance, §4.4). The 5.3 × 10⁻⁵ m of §4.4 was
> measured on the player setups only (models about 2 m tall), where |1 − Σ*w*| · |*v*| stays
> below it; it is not a bound for large models. **Tessera rule**: do not renormalise weights
> at load (`mesh.md` §5.7); tests allow 10⁻⁴ · (1 + |*v*| / 10 m) m plus |1 − Σ*w*| · |*v*|,
> as the implementation does. For bones whose reference scale is not 1, compare with the
> client's offset of §5.4 instead of the identity. Whether the client renormalises is the
> low-priority OPEN above.

> **Answered** (was: singular reference transforms, §4.4). Accepted as **Tessera rules**:
> reject a skeleton with a zero reference scale (`skeleton.singular_bind`), more than 256
> bones, non-finite reference values or a rotation that cannot be normalised
> (`skeleton.bad_reference`); normalise reference rotations otherwise. None occurs in the
> install. (The client takes the reciprocal of the reference scale without a check, BIN3, so
> it has no behaviour to match here.)

> **Answered** (was: extracted motion over loops and the `up` axis, §7.3). The client applies
> no extracted motion (K24). The implementation's per-cycle restart is fine for the debug
> overlay; rotate about the frame's **stored** `up`, which is what a sample means (§7.3).

> **Answered** (was: broken sample values, threshold 10⁴ m). Accepted as a **Tessera rule**
> (no client behaviour was found to match): a sampled track that is not finite or has a
> translation component beyond 10⁴ m takes the bone's reference transform (2 clips:
> 0x05002819, 0x05009355).

> **Answered** (was: bounds on scales and fallbacks). Accepted as **Tessera rules**: a
> sampled scale component beyond 10³ in magnitude marks the track broken; the blended local
> transform falls back per part to the reference beyond those bounds; a non-finite model
> triple takes its parent's (a root's: its reference model triple) and is counted; a
> non-finite skinning matrix is written as the identity.

> **Answered** (was: play times outside a cycle, §6.3). Accepted. This matches the client's
> time update (BIN3): a looping clip's time that goes below 0 wraps up by whole cycles (the
> Euclidean remainder), and a clip with a limited cycle count clamps at the start. Non-finite
> play times sampling *t* = 0 is a **Tessera rule**.

> **Answered** (was: which skinning form, §5.4; River-hobbit height fix). The client uses
> **T·R·S of the skinning triple** (the first statement; scale along the bind axes), and its
> inverse reference translation is −(*q*_r⁻¹ applied to *t*_r) **without** division by the
> reference scale (K26). The Tessera Decision (**P**_b · **B**_b⁻¹ with the exact inverse) is
> withdrawn: it differs from the client for bones with a rotated reference and a non-uniform
> model scale, and for bones whose reference scale is not 1 (worked example §14.10). On the
> player skeletons (identity reference rotations, unit reference scales) both give the same
> result, so the numbers of `appearance.md` §6.5 are unchanged.

> **Answered** (was: §6.6 Tessera Decisions). (1) The client adds no tolerance: it multiplies
> the single-precision weight by 255 and truncates (BIN3); compute *r* and *w* in single
> precision in the order of §6.6 step 2 and truncate the same way, with no 10⁻⁴ step. (2) The
> running-sum test is the client's for the reference fill (K23); for clip contributions it is
> Reported (same routine family, not traced); keep it. (3) There is no cross-fade chain in the
> client: each playing clip has one overall weight, set by its own linear ramp or, while a
> Switch cross-fades its branch, by the branch intensity (`script-table.md` §7.1); clips of
> one layer simply share that layer's remaining budget (§6.6 step 2). Replace the chain
> product by the clip's own current weight.

> **OPEN** (implementation, 2026-10-07; confirmation of engineering choices): the three
> answers above are implemented: no tolerance in the quantisation; skinning from model
> triples with the K26 inverse reference triple (worked example §14.10 is a test); one
> overall weight per clip. A fade that starts while clips of the layer are still fading
> stops every clip of the layer at the weight it has, and each falls linearly from there to
> 0 over the new fade while the new clip rises (the §6.5 table, rows 1 and 2, with one fade
> time for both). Tessera bounds no spec states: one layer plays at most 4 clips at a time;
> a fade that would exceed this drops its weakest stopped clips, and starting a fifth clip
> beside four others on one layer is refused. Please say if the client has a limit.

> **Answered** (2026-10-07, BIN5, K29): the three implemented answers match the client (re-read
> in BIN5: no tolerance, each clip's own overall weight, the K26 palette). On the fades:
> starting a clip changes no other clip's weight, and a clip falls only when its own node
> stops (or its one-shot end comes), from its own current weight over its own node's
> `RampOut` (§6.5). One shared fade time arises only when a Switch cross-fades its branches,
> over the left case's `RampOut` (`script-table.md` §7.1). If the implementation fades the
> other clips of a layer out when a new clip starts there, that differs from the client; if
> it does so only for a Switch branch change, it matches. Limits: the client has none (a
> warning at 1,000 clips per layer). Tessera's 4 clips per layer is an implementation limit,
> not a client rule; the install stays far below it, so a dropped or refused clip is a
> deviation and should be logged.

<a id="s15-1"></a>
### 15.1 Owner observation O14: status after BIN3 and BIN4 (2026-10-07)

The checklist items of `docs/checklists/owner-observations.md` O14, as far as the client's
code settles them. "Answered" means implementable without the owner; a visual check is still
welcome where noted.

| Item | Question | Status | Answer or what is still needed |
|---|---|---|---|
| O14.1 | Blend method and cross-fade time | **Answered** on foot (BIN, data), visual check welcome; mounted **Partly** | Weighted per-bone average with linear weight ramps (§6.5, §6.6). Walk ↔ run: 0.2 s linear cross-fade (the locomotion template's case `RampOut`, `script-table.md` §8.4). Idle ↔ walk and idle ↔ run: the locomotion clips' weights follow `Slider_Propel` / `Slider_Strafe`, which the player's own input path moves at a constant rate of 1/*T* per second, *T* being the merged `GenericPhysicsController_RampTime` of the player's controller (§7.4, K28): 0.13 s while running on the ground and 0.25 s while walking in this install (so 0 → 1 takes 0.13 s or 0.25 s, about 8 or 15 frames at 60 fps). Other entities move their sliders through the follower paths (0.2 s or 0.5 s, `script-table.md` §8.4). No pop, except for clips started by start-up scripts, which start at full weight. Still OPEN: the slider time of the `Mounted` controller type. |
| O14.2 | Upper-body clips over locomotion | **Answered** (BIN, K18), visual check welcome | Legs keep running while moving; a standing attack moves the whole body (§6.6). |
| O14.3 | Turn direction of turn-in-place clips | **Answered** (BIN, K24) | The client never applies the clips' extracted yaw; the heading change comes from the movement system. A positive stored w would be a left turn about +Z (§7.3). Nothing left to observe for rendering. |
| O14.4 | Walk, run, backward speeds; foot sliding | **Answered** (BIN, data; 2026-10-08), confirmation welcome | Rate rule answered (K25: no rate matching, feet slide when speeds differ). Speeds (K28, §7.4; input vector `collision.md` §11.8): running 6.5 m/s forward, 3.25 m/s backward, 5.0 m/s strafing; walking 2.5 m/s forward, 2.25 m/s backward, 2.565 m/s strafing, with `Movement_SpeedMultiplier` 1. |
| O14.5 | Scale inheritance from parent bones | **Answered** (BIN, K17) | Children inherit the scale (their vertices grow too) but their joints do not move: a hand on an inflating arm grows around its own joint and stays where the arm's unscaled joints put it (§5.4). |
| O14.6 | Runtime cloth on capes and skirts | **Answered** (BIN, K27) | None: capes and skirts move only with their clip tracks. |
| O14.7 | Turning in place | **Answered** (data + BIN), visual check welcome | While standing, a turn input sets `Slider_Turn`; the template then plays the species' turn-left or turn-right clips (stepping feet) on the locomotion layer, only while propel and strafe are 0, and the movement system rotates the body (`script-table.md` §8.4). |

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-05 | §1 (BIN, RH, K8, K17–K19), §4.4, §5.3, §5.4, §6.5, §6.6, §10, §11 (KI8), §13, §14.6–§14.8, §15, §2.2 OPEN | Scale composition and layer combination from the client (BIN, decision 0011): triple composition replaces the matrix Decision; the layered-weight rule replaces the "base + w · (layer − base)" recommendation; bind-space deformation noted in the pipeline; River-hobbit size OPEN reduced to a visual check; implementation OPEN on track weights of the lowest layer answered. | Spec writer (dirty room) |
| 2026-10-07 | §1 (BIN3, OBS2, K20–K27), §3.4, §4.4, §5.4, §6.5 (new ramp and cross-fade paragraph), §6.6, §7.3, §7.4, §10, §13, new §14.9–§14.10, §15 (rewritten list; implementation OPENs answered), new §15.1 | Owner observation O14 settled from the client where possible (BIN3, decision 0011): linear ramps, start-up clips without ramp-in, the 0.2 s `UseBoneOpacity` change, fill at a total of at most 0.1 aligned with the running sum, no extracted motion applied in the world, no rate matching and move speeds absent from the data, no cloth; §5.4 corrected (inverse reference translation not divided by the reference scale; the skinning matrix is T·R·S of the triple); the implementation's skinning-form and cross-fade-chain Decisions withdrawn, its other Decisions accepted as Tessera rules. | Spec writer (dirty room) |
| 2026-10-07 (revision after review) | §1 (new BIN4, OBS3, K25 revised, new K28), §6.5 (slider time), §7.4 (speed rule wording, new paragraph on the player's controller configuration, PlayerPhysics defaults, OPEN narrowed), §11 (new KI9), §15, §15.1 (O14.1, O14.4) | Review of 2026-10-07 answered. O14.1: the player's own slider time, which the review found untraced, is now traced (BIN4): it is the `RampTime` of the player's controller configuration, read from the `PlayerPhysicsControl` object of the data (0.13 s running, 0.25 s walking here); the row says Answered on foot and leaves the mounted controller OPEN. §7.4: max(\|*x*\|, \|*y*\|) is taken before \|*y*\| is doubled (optional review item); which strafe speed goes with which sign of *x*. Correction: K25's "the per-direction move speeds are not in the data files" was wrong; the player's speeds come from `PlayerPhysicsControl_…Parameters` (K28), so O14.4 is narrowed to the keyboard's input vector per gait. | Spec writer (dirty room) |
| 2026-10-07 (BIN5) | header; §1 (new BIN5, K18 and K26 sources, new K29); §6.5 (paragraph after the ramps table); §15 (implementation OPEN answered) | A clean-room audit of the River-hobbit height code found code contradicting §5.4 and §6.6 (a 10⁻⁴ quantisation tolerance, a cross-fade chain product, the exact matrix inverse for skinning). The client was re-read (BIN5, decision 0011): the spec's rules stand unchanged (truncation without tolerance, each clip's own overall weight, the K26 palette). Added: starting or stopping a clip changes no other clip's weight, and a layer has no clip limit (warning at 1,000). | Spec writer (dirty room) |
| 2026-10-08 | header; §1 (K30); §7.4 (OPEN answered); §15.1 (O14.4) | Owner-observation pass: the keyboard's input vector per gait is now traced in `collision.md` §11.8 (BIN questions 60–61, C60–C61), so the walking speeds are known (2.5 / 2.25 / 2.565 m/s forward / backward / sideways) and O14.4 needs only a confirmation. No other rule changed. | Spec writer (dirty room) |
