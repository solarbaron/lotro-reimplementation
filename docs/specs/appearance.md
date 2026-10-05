<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Appearance assembly: playable bodies, worn and held items, sliders, NPCs

| | |
|---|---|
| **Status** | Review |
| **Last updated** | 2026-10-05 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | none yet |
| **Implements into** | `crates/formats` (appearance-list structs, bone-slider definition structs; the 0x20 decoder already exists), `crates/anim` (bone-slider application, attach placement), `crates/assets` or a new assembly module (slot table, priority resolution, material and dye selection), `apps/explorer` (character viewer: species, sex, worn items, dyes, sliders), `apps/client` and the sandbox backend (animated avatar, static NPCs) |
| **Companion specs** | [`setup.md`](setup.md) (normative for the 0x47, 0x1F, 0x04 and 0x20 layouts, the item paths §3, the slot field §8.3 and §12.2, held items §13). [`skeleton-anim.md`](skeleton-anim.md) (skeletons, the race-table walk §2.2, attach points §8, the multi-part rule §9). [`material.md`](material.md) §9.3 (material groups), §12 (dyes). [`entity-descriptor.md`](entity-descriptor.md) (SEDs, instance IDs). [`landblock.md`](landblock.md) §4.8 (the landblock weenie list). [`property-system.md`](property-system.md) §5–§8 (property sets, enum mappers, DID maps). `script-table.md` (which clip plays; idle animations of NPCs). |

---

<a id="s0"></a>
## 0. Purpose

`setup.md` says how to decode the records that make a character and how an item reaches its
meshes. This spec says how the pieces are **put together** into one dressed figure:

- which playable species and sexes exist, how each reaches its body, and the list that
  M4 exit criterion 2 must cover (§2);
- the default body, the head, hair, beard and ear styles, and the character-creation
  controls (§3);
- worn items: equipment slots, wardrobe records per body, the **part priority** that decides
  which item shows in each body slot, layering cases, dyes (§4);
- held items: slot and category statistics, attach location, wielded versus stowed (§5);
- whole-body scale, the height controls and the facial bone sliders (§6);
- NPCs and creatures: what the 775 weenie objects of the landblocks really are, where
  NPC-like figures come from, and how to dress them (§7);
- the whole algorithm in words, invariants, statistics, failure categories, corrections
  and synthetic examples (§8–§13).

All counts are observations of one install on one date. Tests assert the invariants (§9),
not the counts. DIDs quoted are observations for tests and checklists; implementations
find records at run time through the DID maps and properties named here.

Enum labels (`MeshTypeID`, `UpperBody`, `Human_Male` …) appear because they are internal
identifiers stored in the data. No game text appears; appearance keys and animation
labels whose identifiers read like asset names are given by value only.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Confirmed** = decoded or measured on the user's own install (Steam, data current
on 2026-10-05) by throwaway probes in the dirty-room scratchpad, using the repository's own
decoders as tools; a Confirmed fact states what was measured and how much of it.
**Reported** = third-party material or analogy only. **Decision** = a Tessera choice where
the data is silent; it is marked as such and has an OPEN next to it.

| Tag | Source |
|---|---|
| OBS | Observed 2026-10-05: **all 4,201** 0x20 records (every entry, block, group and part); **all 383,332** item property sets (all 634,655 worn-appearance map structs of the 42,086 worn items; slot, class, category, dye and parenting properties of all items); **all 40,643** 0x47 records (class and instance appearance lists of all entities); all 29,134 0x1F and 28,555 0x04 records; vertex counts, bounds and slot flags of **all 47,563** meshes; the race table, all 14 player-avatar entities and their character-creation controls; the four bone-slider definition records and two preset records; **all 49,636** landblock-info records (3,253,862 SEDs, 3,768,840 landblock weenie property sets); the 775 weenie objects; the relevant enum mappers. |
| SET | `setup.md` (layouts; item paths §3.1–§3.2; slot field §8.3, §12.2; character setups §12.3; held items §13). |
| SA | `skeleton-anim.md` (race-table walk §2.2; attach maps and holding locations §8; multi-part rule §9). |
| MAT | `material.md` §9.3, §12. |
| ED, LB | `entity-descriptor.md` §4.1–§4.3; `landblock.md` §4.8. |
| MV | Mirarkitty `lotro_mesh_viewer` docs `wardrobe.md`, `hair-face.md`, `outfit-composer.md`, `weapons.md` (MIT docs). Facts only. |
| DS | `research/notes/data-semantics.md` §7.1 (chargen chain, live appearance properties, species codes). |
| BIN | The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035, compiled 2026-09-27), disassembled in the dirty room for facts only on 2026-10-05 (method M4, standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md)). Questions: how the height controls act (§6.2); how bone sliders are parsed, combined and applied, in which frame, and how scaled bones affect children and feet (§6.3–§6.4). Why M1–M3 were not enough: the data name the properties, nodes and limits but no rule (the earlier Decisions were guesses that made River-hobbit feet float 9 cm); no published source covers them; observing the client (M3) can confirm a look but not a rule. Only offline animation and mesh-deformation code was read. |
| ST, SA2 | `script-table.md` §6.7–§6.9 (held clips, layer 110, deformer nodes) and `skeleton-anim.md` §5.4, §6.6 (scale composition, layer combination), written from the same BIN session. |
| RH | Dirty-room probe of 2026-10-05: the rules of §6.2–§6.4 run on the real River-hobbit, DwarfStoutAxe, Dwarf and Human setups, clips and every vertex of their default parts; lowest vertex, lowest bone and top vertex measured (§6.5). |

| # | Fact | Sources | Mark |
|---|---|---|---|
| A1 | 14 playable (species, sex) combinations reach 11 setups through the race table; `Item_SpeciesOfWearer` codes 117, 120, 125 are `HighElf`, `DwarfStoutAxe`, `RiverHobbit` (§2) | SA §2.2; MV, DS (codes 117–125 "probable"); OBS (`SpeciesType` mapper) | Confirmed. Corrects MV and DS. |
| A2 | Worn items use one wardrobe record per (slot family, wardrobe body); seven wardrobe bodies serve the 14 combinations (§2.3, §4.2) | MV (shared wardrobe per body); OBS (634,655 map structs) | Confirmed |
| A3 | A character setup's part list is the default body, one part per `MeshTypeID` slot, some slots holding three-vertex placeholders (§3.1) | SET §12.2; OBS (11 player setups, 4 NPC rigs) | Confirmed |
| A4 | Head, hair, beard and ears come from appearance lists (`Appearance_ClassList`, `Appearance_InstanceList`) of the avatar entity: key + 0x20 record + modifier; the modifier selects the block (style) (§3.2–§3.3) | MV, DS (chargen chain); OBS (14 avatars) | Confirmed |
| A5 | An appearance part's leading float is a **priority**; per body slot the part with the highest priority is shown; a three-vertex part at a higher priority hides the slot (§4.4) | MV (stub "blanking"); OBS (all worn records; mesh extents) | Confirmed by consistent census and geometry; tie rule OPEN |
| A6 | The float before each material group is that group's priority; the float after the last group is 0 in every block; group field `a` is a `MeshTypeID` slot or 2 (`All`) (§4.3) | OBS (84,261 groups, 74,839 blocks) | Confirmed. Corrects the field attribution of SET §8.3 and MAT §9.3. |
| A7 | The 0x20 key table value of an entry equals the number of its blocks whose `variant` is below 0.5 (§4.8) | OBS (49,383 of 49,383 entries) | Confirmed. Resolves SET §8.4. |
| A8 | An item's default dye code is `Item_ClothingColor`; it selects a block of the `ClothingColor` entry of the same wardrobe record (§4.9) | MAT §12; OBS (21,843 of 21,999 items) | Confirmed. Answers MAT §12.4 OPEN. |
| A9 | Whole-body scale of a playable body: template `vec3` and entity `Render_AdjustableScale`; River-hobbit entities carry 0.7 (§6.1) | SET §5.2; OBS (14 avatars) | Confirmed values; composition OPEN |
| A10 | Bone sliders: definition records name bone indices, transform channels and limits; the character stores one byte per slider (§6.3) | DS (byte string, 0x80 default); OBS (4 definition records, 2 preset records) | Confirmed layout; value mapping and application: A16, A17 |
| A11 | The 775 weenie objects placed by SEDs are housing hooks (744), distant-battle units (29), one door and one monster; none is an NPC (§7.1) | ED §4.1 ("NPC, interactive object"); OBS (775 of 775) | Confirmed. Corrects ED §4.1. |
| A12 | The landblock weenie list holds per-instance property sets keyed by IID: one for every SED IID plus 514,987 for IIDs without a SED (§7.2) | LB §4.8 ("server-side templates"); OBS (3,768,840 entries) | Confirmed. Corrects LB §4.8. |
| A13 | Static NPC-like figures are Physics SEDs whose object resolves to a humanoid rig and whose instance property set adds appearance lists, a random-appearance type, an idle animation and held items (§7.2–§7.4) | OBS | Confirmed |
| A14 | No client record maps `Appearance_RandomAppearanceType` to an outfit (§7.4) | OBS (property sets of all prefixes holding property sets; raw search of the PID) | Confirmed absence in the scanned records; OPEN |
| A15 | `AC_Avatar_Height` is the held time (as a fraction) of the River-hobbit layer-110 `scale_blend` clip; `AC_BoneScale_Test` is the held fraction of every avatar's body-shape deformer clip and of the DwarfStoutAxe layer-110 clip (§6.2) | OBS (the 7 driven nodes, the 14 species tables); BIN (§6.7 of ST) | Confirmed 2026-10-05 |
| A16 | Bone sliders: one value per slider *t* = byte / 255 in definition order, 0.5 for sliders past the end of the string; channel = min + *t* × (max − min), negated per reversed bone; later sliders overwrite earlier ones per bone and channel (§6.3) | BIN; OBS (definition records, node strings) | Confirmed 2026-10-05 (BIN only for the rule). Replaces the §6.3 value Decision. |
| A17 | Sliders and body-shape deformers are not applied to the animated pose: they deform the skinned meshes once in bind space, using the scale composition of SA2 (§6.4) | BIN; RH (feet within 1 cm of the ground for River-hobbit M and F at every height value) | Confirmed 2026-10-05. Replaces the §6.3 application Decision. |
| A18 | Expected standing heights with all mechanisms applied (§6.5) | RH | Confirmed 2026-10-05 as a computation on real data; not yet compared with the original client by eye |

---

<a id="s2"></a>
## 2. Playable species and bodies

<a id="s2-1"></a>
### 2.1 From the race table to a body (Confirmed)

The walk is normative in `skeleton-anim.md` §2.2: DID-map label `RaceTableDirectory` →
`RaceTable_RaceTableList` (one game object per species) → `RaceTable_Race` (a `SpeciesType`
key) and `RaceTable_GenderList` → per gender `RaceTable_GenderType` (0x1000 male, 0x2000
female) and `RaceTable_Gender_PlayerAvatar` (a game object) → its `PhysObj` (0x47) → 0x1F →
0x04. The same gender struct also holds:

| Property | Meaning for assembly |
|---|---|
| `RaceTable_RaceSelect_Entity` | A 0x47 used by the race-selection screen; not the play body (the River-hobbit entries name the Hobbit selection entities 0x47000604/0x47000605 although they play on the human-sized setups). Ignore for M4. |
| `RaceTable_GenderNullAppearance` | 0x20000004 for every entry: a record with one entry (key 0x1000003A `Worn`), one block, one group and **no parts**. It is the "nothing to show" appearance (also the value of `ExternalAppearanceID` on 138,469 items). |
| `RaceTable_Gender_AdminAvatar` | Another avatar object; ignore. |
| `RaceTable_IsTallRace` (on the species object) | true for Human, Elf, Beorning, HighElf; false for Hobbit, Dwarf, DwarfStoutAxe, RiverHobbit. Not an appearance input. |

The **player-avatar game object** itself carries no appearance data apart from `PhysObj`. Its
0x47 entity (the "avatar entity") carries everything else: the appearance list of §3.2, the
default colours and texture indices `AC_*`, `Sex`, `Render_AdjustableScale`,
`AC_BoneScale_Test`, `AC_BoneSlider_Value_Array` and the character-creation controls
`AppearanceUI_Controls` (§3.4).

<a id="s2-2"></a>
### 2.2 The list for exit criterion 2 (Confirmed)

`SpeciesType` keys are from the enum mapper that `RaceTable_Race` and `Item_SpeciesOfWearer`
name (0x2300000B here). There is no body-type choice beyond sex in the data (no property,
control or record selects a body shape); the River-hobbit "body type" differences are fixed
sliders (§6.3). **Fourteen** combinations must be checked; DwarfStoutAxe has two gender
entries that name the same avatar object (0x7005339D), so it is one body.

| # | Species key | Label | Sex | Avatar entity | Template | Setup | Bones | Wardrobe body (§2.3) | Template `vec3` | Entity scale | Bind height (m) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P1 | 0x51 | Hobbit | M | 0x47000606 | 0x1F000586 | 0x04000551 | 62 | Hobbit M | – | 1.0 | 1.16 |
| P2 | 0x51 | Hobbit | F | 0x47000607 | 0x1F000587 | 0x04000552 | 62 | Hobbit F | – | 1.0 | 1.14 |
| P3 | 0x17 | Human | M | 0x4700060A | 0x1F000588 | 0x04000553 | 89 | Human M | – | 1.0 | 1.81 |
| P4 | 0x17 | Human | F | 0x4700060B | 0x1F000589 | 0x04000554 | 89 | Human F | – | 1.0 | 1.77 |
| P5 | 0x41 | Elf | M | 0x4700060E | 0x1F00058A | 0x04000555 | 89 | Elf M | 1.03 | 1.0 | 1.80 |
| P6 | 0x41 | Elf | F | 0x4700060F | 0x1F00058B | 0x04000556 | 89 | Elf F | – | 1.02 | 1.76 |
| P7 | 0x49 | Dwarf | M | 0x47000611 | 0x1F00058C | 0x04000557 | 62 | Dwarf M | – | 1.0 | 1.36 |
| P8 | 0x72 | Beorning | M | 0x47007004 | 0x1F000588 | 0x04000553 | 89 | Human M | – | 1.03 | 1.81 |
| P9 | 0x72 | Beorning | F | 0x47007005 | 0x1F000589 | 0x04000554 | 89 | Human F | – | 1.03 | 1.77 |
| P10 | 0x75 | HighElf | M | 0x4700806C | 0x1F006111 | 0x04005EC4 | 89 | Elf M | 1.05 | 1.0 | 1.81 |
| P11 | 0x75 | HighElf | F | 0x47008071 | 0x1F006110 | 0x04005EC5 | 89 | Elf F | 1.03 | 1.02 | 1.75 |
| P12 | 0x78 | DwarfStoutAxe | (both entries) | 0x4700979A | 0x1F00058C | 0x04000557 | 62 | Dwarf M | – | 1.0 | 1.36 |
| P13 | 0x7D | RiverHobbit | M | 0x4700CEA3 | 0x1F00846F | 0x0400772B | 89 | Human M | – | **0.7** | 1.82 |
| P14 | 0x7D | RiverHobbit | F | 0x4700CEA4 | 0x1F008470 | 0x0400772A | 89 | Human F | – | **0.7** | 1.77 |

"Entity scale" is `Render_AdjustableScale` (equal to `Physics_AdjustableScale` in all 14).
"Bind height" is the top of the default body's geometry (hair included) in the setup frame,
before any scale (§6.1).

Out of scope for M4: the Beorning's animal form (a shapeshift entity named by item and trait
properties, `setup.md` §3.3), mounts, and the race-selection entities.

<a id="s2-3"></a>
### 2.3 Wardrobe bodies (Confirmed)

Every worn item lists one `Item_WornAppearanceMapStruct` per (species, sex) it supports
(`setup.md` §3.2). Over all 42,086 worn items the struct for a given (species, sex) and
item slot names the **same** wardrobe record (apart from 2 chest/shoulder cross-overs per
body, §4.2). The species collapse onto seven wardrobe bodies:

| Wardrobe body | Used by | Setup skeleton |
|---|---|---|
| Human M | Human M, Beorning M, RiverHobbit M | 89-bone male |
| Human F | Human F, Beorning F, RiverHobbit F | 89-bone female |
| Elf M | Elf M, HighElf M | 89-bone male |
| Elf F | Elf F, HighElf F | 89-bone female |
| Dwarf M | Dwarf M, DwarfStoutAxe (both sexes) | 62-bone dwarf |
| Hobbit M | Hobbit M | 62-bone hobbit male |
| Hobbit F | Hobbit F | 62-bone hobbit female |

Map structs per worn item: 15 (38,700 items), 16 (3,379 items: the extra struct is
Dwarf F) and 13 (7 items without Beorning). The Dwarf F structs point at the null record
0x20000004 in 2,339 cases and at the Dwarf M records otherwise; no playable Dwarf female
exists, so a resolver never asks for them.

> **Decision:** if an item has no struct for the wearer's exact species, use the struct of
> the species that shares the wardrobe body in the table above (Beorning → Human,
> RiverHobbit → Human, HighElf → Elf, DwarfStoutAxe → Dwarf), same sex. Needed for the 7
> items without Beorning structs.

---

<a id="s3"></a>
## 3. Base body

<a id="s3-1"></a>
### 3.1 Default parts from the setup (Confirmed)

A character setup's part list is its default body (`setup.md` §12.2). Each part's slot is
its mesh's `flags` word, a `MeshTypeID` value (0x23000089 here). The playable setups
(vertex counts; "stub" = a three-vertex single-triangle mesh, §4.4; "–" = no part):

| Setup | UpperBody | LowerBody | Hands | Feet | Head | Ears | Crest | Beard | Shoulder | Helmet | Cape | Satchel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Hobbit M 0x04000551 | 814 | 629 | 794 | 566 | 730 | – | 605 | – | stub | stub | stub | stub |
| Hobbit F 0x04000552 | 995 | 833 | 690 | 604 | 647 | – | 272 | – | stub | stub | stub | stub |
| Human M 0x04000553 | 1,119 | 810 | 944 | 298 | 1,125 | 184 | 22,535 | stub | stub | stub | stub | stub |
| Human F 0x04000554 | 2,170 | 1,882 | 885 | 294 | 1,216 | 190 | 11,459 | stub | stub | stub | stub | stub |
| Elf M 0x04000555 | 1,116 | 935 | 944 | 298 | 1,155 | 258 | 10,175 | – | stub | stub | stub | stub |
| Elf F 0x04000556 | 708 | 628 | 397 | 138 | 1,225 | 258 | 21,022 | – | stub | stub | stub | stub |
| Dwarf M 0x04000557 | 930 | 571 | 890 | 234 | 649 | – | 199 | 658 | stub | stub | stub | stub |
| HighElf M 0x04005EC4 | 1,116 | 935 | 944 | 298 | 1,155 | 258 | 12,277 | – | stub | stub | stub | stub |
| HighElf F 0x04005EC5 | 708 | 628 | 397 | 138 | 1,195 | 258 | 16,836 | – | stub | stub | stub | stub |
| RiverHobbit M 0x0400772B | 1,119 | 810 | 944 | 2,061 | 1,125 | 184 | 24,813 | stub | stub | stub | stub | stub |
| RiverHobbit F 0x0400772A | 2,170 | 1,882 | 885 | 3,045 | 1,213 | 190 | 33,231 | stub | stub | stub | stub | stub |

`Crest` is the hair slot, `Head` the head and face, `Ears` the ears (separate only on the
human-sized bodies), `Beard` the beard. The default Head, Crest and Beard parts are blocks
of the avatar's style records (§3.3): for example on Human M the setup's hair mesh is block
0.00 of the hair style record, its head mesh block 0.01 of the head style record and its
beard stub block 0.00 (the "none" beard) of the beard style record.

The bare body parts (UpperBody, LowerBody, Hands, Feet) are the body without clothes; every
garment replaces them per slot (§4.4).

<a id="s3-2"></a>
### 3.2 Appearance lists (Confirmed)

Entities and instances carry **appearance lists**: arrays of `Appearance_KeyInfo` structs.
Two array properties use the same struct:

| Property | Where | Non-empty in |
|---|---|---|
| `Appearance_ClassList` | 0x47 entity property set (class default) | 9,225 entities |
| `Appearance_InstanceList` | 0x47 entity property set, or an instance property set (a landblock weenie entry, §7.2) | 550 entities; 421,791 landblock instance sets |

`Appearance_KeyInfo` fields:

| Field | Type | Meaning |
|---|---|---|
| `Appearance_Key` | enum (`AppearanceKey` mapper, 0x2300008A) | Which entry of the record to use |
| `Appearance_AprFile` | DID 0x20 | The appearance record |
| `Appearance_Modifier` | float | Selects the block of the entry (§3.2.1) |
| `Appearance_AprToggleFile` | DID 0x20, optional | Alternative record (§3.3) |
| `Appearance_ToggleProperty` | property ID, optional | The property that switches to the toggle record (§3.3) |

<a id="s3-2-1"></a>
#### 3.2.1 Choosing the block (Confirmed sources, rule partly Decision)

Within the entry whose `key` equals `Appearance_Key`:

1. If a block's `variant` equals the modifier (within 0.0005), take it. This covers style
   lists (modifier = style index / 100: 0.00, 0.01, …), worn pieces on NPCs (modifier 1.0 =
   the base block) and dye tables (modifier = dye code).
2. Otherwise, for a `ClothingColor` or `ClothingColorSecondary` entry, use the dye rule of
   `material.md` §12.4.
3. Otherwise (Decision): the block with `variant` 1.0 if there is one, else the first block.

Census over all appearance lists of all 0x47 records (12,199 elements): 11,823 find a block
by rule 1, 295 need rule 2 or 3, 67 name a key the record lacks, 14 name the null DID.

<a id="s3-3"></a>
### 3.3 Head, hair, beard and ears (Confirmed)

The avatar entity's list (class or instance list) names one style record per feature. Each
style record has **one entry** whose blocks are the styles; each block has one part.

| `AppearanceKey` (value, label) | Slot the parts fill | Part priority (§4.4) | Notes |
|---|---|---|---|
| 0x10000009 `FaceTexture` | `Head` | 1.0 | The head and face mesh. The label is historical; the parts are meshes. Driven by the character property `AC_Mesh_Head` (the control's `AppearanceUI_ControlScriptPropertyName`). |
| 0x10000032 `HeadMesh` | `Crest` | 1.0 | The **hair**. One block is a stub ("bald"). |
| 0x10000018 `BeardMesh` | `Beard` | 2.0; the "none" block 0.0 | Human, Dwarf and DwarfStoutAxe male lists, Human F (a second file with eight blocks). |
| 0x1000002F `HeadSpecial` | `Ears` | 1.0 | Ear styles of Elf and HighElf; only in the character-creation controls, not in the entity's list (the default ears are the setup part). |
| 0x10000034 `ClothingColor` | none (material only) | – | DwarfStoutAxe's list adds a one-block colour entry. |

Per avatar (record DID and block count; "toggle" = `Appearance_AprToggleFile` with
`Appearance_ToggleProperty` = `GOP_AvatarUpdateVisible`):

| Avatar | List | Head (`FaceTexture`) | Hair (`HeadMesh`) | Beard |
|---|---|---|---|---|
| Hobbit M | class | 0x20002AE3 [20], toggle 0x20001250 [20] | 0x20002B03 [23], toggle 0x20001251 [23] | – |
| Hobbit F | class | 0x20002AD0 [20], toggle 0x20001253 [20] | 0x20002AB2 [23], toggle 0x20001252 [23] | – |
| Human M | instance | 0x20002AD4 [38], toggle 0x20001255 [38] | 0x20002AD2 [58], toggle 0x20001254 [61] | 0x20002B12 [9], toggle 0x20001256 [9] |
| Human F | instance | 0x20002AB6 [35], toggle 0x20001258 [35] | 0x20002AEF [55], toggle 0x20001257 [59] | 0x200032D1 [8], toggle 0x2000327E [8] |
| Elf M | class | 0x20002AE6 [18], toggle 0x20001259 [26] | 0x20002AB1 [22], toggle 0x2000125A [37] | – |
| Elf F | class | 0x20002AB5 [20], toggle 0x2000125C [30] | 0x20002AAF [22], toggle 0x2000125B [37] | – |
| Dwarf M | class | 0x20002B08 [12], toggle 0x2000125D [12] | 0x20002AF8 [22], toggle 0x2000125E [22] | 0x20002B17 [14], toggle 0x2000125F [14] |
| Beorning M | class | 0x200029B8 [8] | 0x200029BA [14] | – |
| Beorning F | class | 0x200029B9 [9] | 0x200029C0 [16] | – |
| HighElf M | class | – (setup default) | 0x20002B3D [37] | – |
| HighElf F | class | – (setup default) | 0x20002B4A [38] | – |
| DwarfStoutAxe | class | 0x2000306F [8] | 0x20003044 [12] | 0x20003043 [21] |
| RiverHobbit M | instance | 0x20003303 [15] | 0x20003329 [37] | – |
| RiverHobbit F | instance | 0x20003302 [15] | 0x2000332A [37] | – |

All list modifiers are 0 (style 0). The character-creation controls (§3.4) of some avatars
name other files for the same keys (Beorning, HighElf, the toggle files); the entity's list
is the in-world default.

**Toggle files.** The property named by `Appearance_ToggleProperty` is
`GOP_AvatarUpdateVisible`, a boolean that the avatar entity itself sets to true. The toggle
file holds the same styles as the plain file plus later ones (Human M hair: 57 meshes in
common, 4 only in the toggle file, 1 only in the plain file; heads and beards identical
sets).

> **Decision:** use `Appearance_AprToggleFile` when the toggle property is true in the
> entity's property set (it is, for all avatars that have toggles), else `Appearance_AprFile`.
> **OPEN** (owner observation O11): what the client's own option controls and which file it
> uses by default.

**Face compositing is out of scope for M4.** Character-creation controls of type
`AppearanceUI_CompositorControl` name compositor records (0x33000002, 0x33000003) and layer
labels (`Avatar_Face_Base`, `_Eyes`, `_Eyebrows`, `_Mouth`, `_Scar`, `_Scar2`, `_Wrinkles`,
`_Stubble` …) driven by the integer properties `AC_Texture_*`. Fallback: draw the head mesh
with its own surface (its base face texture is complete, MV). Skin, hair, eye and lip colours
(`AC_Color_Skin`, `AC_Color_Hair`, `AC_Color_Eyes`, `AC_Color_Lips`, `AC_Color_WarPaint`,
RGBA) are not applied by M4.

> **OPEN** (frame capture, owner observation O15): how the colour properties tint skin, hair
> and eyes (MV reports a dye-like tint of grey hair textures), and the 0x33 compositor layout.

<a id="s3-4"></a>
### 3.4 Character-creation controls (Confirmed layout, UI only)

`AppearanceUI_Controls` (11 to 20 controls per avatar) is an array of `AppearanceUI_Control`
structs, each holding `AppearanceUI_Availability` (bits `CharGen`, `AppearanceMod`),
optionally `AppearanceUI_Group` (`Skin`, `Head`, `Face`, `Eyes`, `Hair`, `Mouth`) and exactly
one control struct:

| Control struct | Drives | Content |
|---|---|---|
| `AppearanceUI_APRControl` | A style choice | `AppearanceUI_APRFile`, `AppearanceUI_APRKey`, optional `AppearanceUI_ControlScriptPropertyName` (`AC_Mesh_Head` for heads), `AppearanceUI_EndOldOptionsControlValue` and `AppearanceUI_OldOptionsWeightValue` (UI ordering of older styles) |
| `AppearanceUI_CompositorControl` | A face texture layer | §3.3 |
| `AppearanceUI_DyeMapControl` | A colour property | `AppearanceUI_ControlPropertyName` (`AC_Color_*`), `AppearanceUI_DyeMapImage` (a palette image) |
| `AppearanceUI_BoneScalingControl` | `AC_BoneScale_Test`, and on RiverHobbit also `AC_Avatar_Height` | `AppearanceUI_MinFloatControlValue`, `AppearanceUI_MaxFloatControlValue` (§6.2) |
| `AppearanceUI_BoneSliderControl` | `AC_BoneSlider_Value_Array` | `AppearanceUI_BoneSliderDefinitionsFile` (§6.3) |
| `AppearanceUI_PresetsControl` | Whole-appearance presets | `AppearanceUI_PresetsFile`: a 0x78 property set with `Chargen_AvatarPresetsArray` of `Character_AppearanceInfo` structs (appearance list, `AC_*` values, slider bytes) |

The controls matter to Tessera only as the catalogue of choices for the explorer's character
viewer. A character's actual choices are character data (server side in the original, M7 in
Tessera); the avatar entity's own values are the defaults.

---

<a id="s4"></a>
## 4. Worn items

<a id="s4-1"></a>
### 4.1 Equipment slots (Confirmed)

The item's slot is `Inventory_DefaultSlot`, a `ContainerSlot` bit field (mapper 0x230000EE;
`Inventory_CompatibleSlot` adds `Backpack` and `Overflow`). The 42,086 items with an
`Item_WornAppearanceMapList`:

| Slot (bit) | Items | `Item_Class` values | Own body slot (§4.4) |
|---|---|---|---|
| `Head` (0x2) | 5,989 | `Head`, `Cosmetic_Head` | `Helmet` |
| `Chest` (0x4) | 7,655 | `Chest`, `Cosmetic_Chest` | `UpperBody` |
| `Legs` (0x8) | 5,816 | `Legs`, `Cosmetic_Legs` | `LowerBody` |
| `Gloves` (0x10) | 6,448 | `Hands`, `Cosmetic_Hands` | `Hands` |
| `Boots` (0x20) | 6,462 | `Feet`, `Cosmetic_Feet` | `Feet` |
| `Shoulder` (0x40) | 6,511 | `Shoulders`, `Cosmetic_Shoulders` | `Shoulder` |
| `Back` (0x80) | 2,885 | `Back`, `Cosmetic_Back` | `Cape` |
| `Weapon_Primary` (0x10000) | 200 | `Weapon_BattleGauntlets` | `Hands` |
| `Class` (0x100000) | 120 | `Item_Runekeeper` (104), `Cosmetic_Class` (16) | `Satchel` |

`WeenieType` of these items: `Clothing` 41,871, `Weapon` 200, `Item` 15. Jewellery slots
(rings, bracelets, earrings, necklace, pocket) have no worn appearance and are invisible.
Every worn item also has a `PhysObj` (its look on the ground); a worn item is drawn through
its worn appearance, never through `PhysObj` (`setup.md` §13.3).

<a id="s4-2"></a>
### 4.2 Wardrobe records (Confirmed)

The 634,655 map structs name 73 distinct 0x20 records: per wardrobe body one record per
slot family, plus the null record. DIDs on this install:

| Wardrobe body | Head | Shoulder | Chest | Back (a) | Back (b) | Gloves | Legs | Boots | Class | Weapon_Primary |
|---|---|---|---|---|---|---|---|---|---|---|
| Human M | 0x20001E46 | 0x20001E62 | 0x20001E54 | 0x200024D9 | 0x20001EA2 | 0x20001E69 | 0x20001E4D | 0x20001E5B | 0x20002133 | 0x200031F2 |
| Human F | 0x20001E47 | 0x20001E63 | 0x20001E55 | 0x200024D8 | 0x20001EA3 | 0x20001E6A | 0x20001E4E | 0x20001E5C | 0x20002128 | 0x200031F4 |
| Elf M | 0x20001E4B | 0x20001E67 | 0x20001E59 | 0x200024D5 | 0x20001EA7 | 0x20001E6E | 0x20001E52 | 0x20001E60 | 0x20002107 | 0x200031F1 |
| Elf F | 0x20001E4C | 0x20001E68 | 0x20001E5A | 0x200024D4 | 0x20001EA8 | 0x20001E6F | 0x20001E53 | 0x20001E61 | 0x200020FC | 0x200031EE |
| Dwarf M | 0x20001E4A | 0x20001E66 | 0x20001E58 | 0x200024D3 | 0x20001EA6 | 0x20001E6D | 0x20001E51 | 0x20001E5F | 0x200020F1 | 0x200031EF |
| Hobbit M | 0x20001E48 | 0x20001E64 | 0x20001E56 | 0x200024D7 | 0x20001EA4 | 0x20001E6B | 0x20001E4F | 0x20001E5D | 0x2000211D | 0x200031F3 |
| Hobbit F | 0x20001E49 | 0x20001E65 | 0x20001E57 | 0x200024D6 | 0x20001EA5 | 0x20001E6C | 0x20001E50 | 0x20001E5E | 0x20002112 | 0x200031F0 |

Back (a) serves 2,768 cloaks, Back (b) 116; the Class records also serve one cloak each.
Two chest items name the Shoulder record and two shoulder items the Chest record of each
body (the families share keys). The order of bodies within a family differs between
families, so nothing may be derived from DID arithmetic.

Each wardrobe record has one entry per item appearance key (single block, `variant` 1.0)
plus two **dye tables**: entries keyed `ClothingColor` (0x10000034) and
`ClothingColorSecondary` (0x10000022) with 53 material-only blocks each (§4.9). Entry counts
per record (dye tables included): Head 736–738, Shoulder 661–667, Chest 1,085–1,088, Back (a)
1,022, Back (b) 115, Gloves 645–647, Legs 349–355, Boots 653–655; Human M Class 18,
Weapon_Primary 26.

`section` byte by family: Head, Shoulder, Gloves, Boots 5; Chest, Back (a) 6; Legs 4;
Back (b) 3; Weapon_Primary 1; Class and the null record 0. Mount records use 2, 3 and 4 as
well, and style records use 0. The byte does not change decoding.

> **OPEN:** meaning of `section` (a record category; nothing in assembly depends on it).

<a id="s4-3"></a>
### 4.3 Block fields, corrected attribution (Confirmed)

The block framing of `setup.md` §8.3 stands. Its float attribution in the group list is
corrected here (the framing is identical either way, so decoders do not change):

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | `variant` | f32 | Block selector (§3.2.1) |
| 2 | `group_count` *g* | u32 | |
| 3 | groups | *g* × (f32 `priority`, u32 `shader`, u32 `material`, u32 `slot`, u32 `binding_key`) | The float **before** each group is that group's priority. `slot` is a `MeshTypeID` value: 2 (`All`) or a body slot. |
| 4 | `groups_end` | f32 | 0.0 in every block |
| 5 | `part_count` *p* | u32 | |
| 6 | parts | *p* × (f32 `priority`, u32 `slot`, u32 `mesh`) | `setup.md` §8.3 (`part_param` = priority) |
| 7 | extras, `end` | | `setup.md` §8.3 |

In `setup.md` §8.3 terms: `group_param` is the first group's priority, each group's `g` is
the next group's priority, and the last group's `g` is `groups_end`. Evidence: the last
group's trailing float is 0.0 in all 67,166 blocks that have groups, and `group_param` is 0.0
in all 7,673 blocks without groups, while the per-group values pair with the groups' own
slots (for example a dye block's groups: `All` 5, `All` 5, `All` 7, `UpperBody` 0). The
`slot` field (`setup.md`'s `a`) holds a `MeshTypeID` key in all 84,261 groups (2 = `All` in
the large majority).

Group priorities observed: 0 (34,598), 1 (26,876), 5 (9,409), 10 (3,765), 3 (3,310), 7, 2,
11, 4, 50, 6, 20, 25, 30, 15, 12, 9, 100. Part priorities: 1 (43,821), 10 (17,736), 15
(6,281), 4 (5,230), 2 (2,733), 5 (1,902), 0 (1,196), 3 (1,060), then 50, 11, 30, 20, 7, 9, 6,
25, 17, 100, 8, 12. All are whole numbers.

<a id="s4-4"></a>
### 4.4 The replacement rule (Confirmed census; normative for Tessera)

A figure is a **slot table**: for every `MeshTypeID` slot, at most one mesh. Sources write
into it in order (§8); a part replaces the slot's current mesh when its priority is
**greater than** the current one's.

1. The setup's default parts enter first with the lowest possible priority.
2. Every appearance part (style records, NPC pieces, worn items) is a candidate for its own
   `slot` field (not the mesh's `flags`; they differ for 3 parts of the worn records).
3. Per slot the candidate with the **highest priority** wins. A winning **stub** mesh
   (three vertices, one triangle: 258 mesh records; the smallest real part has 4 vertices)
   leaves the slot visually empty: the slot is "hidden". Whether the original draws the
   stub's tiny triangle is O4; it never matters for what is hidden.
4. A slot with no candidate keeps the default part.

The rule needs no flag or property anywhere: hiding is expressed only by stub parts at a
higher priority (MV found no hide or suppress property; this census confirms).

**Why "highest wins" (evidence).** Every worn item carries its own slot's part at a low
priority (1 for chest, legs, gloves, shoulders, cloaks; 2 or 5 for boots; 3 or 4 for
helmets) and parts for other slots at higher ones (10, 15). Geometry fits only this reading:

- Chest entries with a `LowerBody` stub at 10: 686 entries on Human M, all of whose
  `UpperBody` meshes reach the ankles (median lowest point 0.12 m). The 375 chest entries
  without a `LowerBody` part have `UpperBody` meshes ending at the waist (1.03 m, the height
  where the leg meshes start). Full-length robes must hide the leggings (priority 1).
- Chest entries with a real `Hands` part at 10 (117) or 5 (37) use exactly the body's bare
  hands mesh (0x0600D4B0 on Human M): sleeves that would clip through gloves force bare
  hands, beating gloves at 1 (641 of 645 glove entries) but not the four gloves at 6 when the
  sleeve's priority is 5.
- Boots at 2 are tall (median top 0.49 m; 463 on Human M), boots at 5 are low shoes (top
  0.13 m; 189). Long robes with their own shoes (`Feet` real at 3, 62 entries) beat tall
  boots, whose shafts would clip through the hem, and lose to low shoes.
- Helmets hide hair with a `Crest` stub at 10 (above the hair styles' 1) or replace it with a
  single fitted "hair under a hat" mesh at 10; hooded cloaks hide hair and helmet with stubs
  at 15 (above helmets' 3 or 4); ten very large helmets at 20 stay visible over a hood and
  hide the face with a `Head` stub at 15.

<a id="s4-5"></a>
### 4.5 Census of worn parts (Confirmed, all 632,056 resolved structs with parts)

Own-slot part (priority: structs) and parts for other slots (slot, kind, priority: structs):

| Family | Own slot part | Other-slot parts (most common) |
|---|---|---|
| Head | `Helmet` real 4: 80,786; 3: 7,600; 20: 1,620 | `Crest` stub 10: 60,075; `Ears` real 10: 47,346; `Crest` real 10: 24,888; `Beard` stub 10: 4,245; `Head` stub 15: 1,620; `Crest` real 9: 133 |
| Shoulder | `Shoulder` real 1: 97,562; real 10: 102; stub 10: 60; 20, 50 | rare full-outfit entries (`UpperBody`, `Hands`, `LowerBody`, `Feet`, `Helmet`, `Cape`, `Crest`, `Satchel` at 5–10; under 200 structs each) |
| Chest | `UpperBody` real 1: 114,892; 11: 45; 10: 6; none: 30 | `LowerBody` stub 10: 42,942; `Feet` stub 10: 2,380; `Hands` real 10: 1,660; `Feet` real 3: 740; `Hands` stub 10: 650; `Hands` real 5: 558; `LowerBody` real 10: 474; `Hands` real 0: 126 |
| Back | `Cape` real 1: 43,277; 10: 45; none: 15 | `Crest` stub 15: 12,196; `Helmet` stub 15: 12,142; `Ears` real 10: 4,430; `Helmet` real 9, 17, 20: 60 |
| Gloves | `Hands` real 1: 96,754; 6: 120 | none |
| Legs | `LowerBody` real 1: 87,237 | `Feet` real 7: 30; 15: 15 |
| Boots | `Feet` real 2: 38,440; 1: 32,473; 5: 26,178 | none |
| Class | `Satchel` real 3: 1,795 | none |
| Weapon_Primary | `Hands` real 5: 2,985; 1: 15 | none |

Per wardrobe body the pattern is the same with body-specific differences: hobbit and dwarf
boots all use priority 1; hobbit and dwarf records have no `Ears` parts (no ears slot) and
put a `Hands` stub where the human-sized records put bare hands (the hobbit and dwarf robe
meshes model the hands themselves); dwarf chest records have 36 `Hands` real parts at 0.

Resolution of the map structs: 632,056 find an entry with parts, 2,339 find a material-only
entry (all Dwarf F → null record), 260 name a key their record lacks (legs records of
several bodies, 9 to 15 structs each). Every entry found has a `variant` 1.0 block
(634,395 of 634,395); worn items always use that block.

<a id="s4-6"></a>
### 4.6 Layering cases in words (Confirmed by §4.4–§4.5)

| Case | What decides | Result |
|---|---|---|
| Cloak over chest | Different slots (`Cape` vs `UpperBody`) | Both drawn; the cape mesh is modelled to sit over the torso. No ordering is needed: parts are opaque skinned meshes on one skeleton. |
| Helmet versus hair | Helmet's `Crest` part at 10 vs hair style at 1 | Closed helmets hide hair (stub); hats with a fitted hair mesh replace the style; open headgear (no `Crest` part, 94 entries on Human M) leaves the chosen hair. Beard stubs at 10 hide beards (50 entries on Human M). |
| Helmet versus ears | `Ears` real 10 | Most human-sized helmets substitute fitted ears (0x0600E55A on Human M) for the body's ears. |
| Full helm versus face | `Head` stub 15 | Hides the head mesh (10 entries per body). |
| Hood versus helmet and hair | Cloak's `Helmet` and `Crest` stubs at 15 | Hooded cloaks (440–444 per body) hide both; helmets at 20 stay. |
| Gloves versus sleeves | Chest `Hands` real (bare) at 5 or 10, or stub at 10 | Long-sleeved chests force bare hands (hide gloves at 1, or at 6 when the chest uses 5); battle gauntlets (5) tie with a chest at 5 (§4.7). |
| Boots versus legs | Legs' `Feet` real at 7 or 15 (10 entries per human-sized body; hobbit and dwarf records have 6 `Feet` stubs at 10 and one real at 15) | Those leggings include footwear, or cover the feet, and beat any boots; ordinary leggings have no `Feet` part. |
| Boots versus robes | Chest `Feet` stub 10 or real 3 | Floor-length robes hide boots, or show their own shoes unless the boots are low shoes (5). |
| Robe versus legs | Chest `LowerBody` stub 10 | Leggings hidden under floor-length robes. Chests with their own legs (`LowerBody` real 10, 22–24 per body) replace the leggings. |
| Satchel | `Satchel` real 3 (Class slot) | Drawn in its own slot; one cloak entry also supplies a satchel at 4. |

<a id="s4-7"></a>
### 4.7 Ties (Confirmed occurrence; rule OPEN)

Equal priorities with different meshes occur between families: `Hands` 5 (chest 7 entries
per body vs battle gauntlets 147), `Hands` 1 (gloves vs 6 gauntlet entries), `Ears` 10
(helmet vs cloak; same fitted-ears mesh in practice), `Feet` 1 (boots vs 7 cloak entries),
`Helmet` 9, 10 and 20, `Crest` 10, `LowerBody`, `UpperBody`, `Shoulder`, `Cape` 10 (rare
cross-slot entries of the shoulder family), `Satchel` 4 (identical meshes). In NPC
appearance lists (§7.4) ties at priority 1 are common: 1,176 slot conflicts in the static
NPC lists, 311 of them between a stub and a real part (the real part comes later in the
list in 265).

> **Decision:** on equal priority the later source wins, with worn items applied in
> ascending `ContainerSlot` bit order (`Head`, `Chest`, `Legs`, `Gloves`, `Boots`,
> `Shoulder`, `Back`, `Weapon_Primary`, `Class`) after the appearance lists, and list
> elements in stored order. This lets battle gauntlets show over a chest's bare hands and an
> NPC's explicitly listed hair show over a piece's stub.
>
> **OPEN** (owner observation O10): the original's tie rule.

<a id="s4-8"></a>
### 4.8 Key table (Confirmed; resolves `setup.md` §8.4)

The value stored for each key equals the **number of blocks of that entry whose `variant`
is below 0.5**, in all 49,383 entries of all 4,201 records: 0 for a single base block
(`variant` 1.0), the block count for style lists, and for example 46 for a 53-block dye
table (dye codes 0.00 to 0.45 below 0.5, seven special codes up to 0.80 above). It is a
derived count (probably the number of ordinary choices); a decoder may check it as an
invariant and assembly does not need it.

<a id="s4-9"></a>
### 4.9 Dyes on worn items (Confirmed; answers `material.md` §12.4 OPEN)

The dye code of a worn item comes from the item property `Item_ClothingColor` (float, 22,257
items; 0.0 in 7,712) and, for the second channel, `Item_ClothingColorSecondary` (2,785
items). The code selects a block of the `ClothingColor` (or `ClothingColorSecondary`) entry
**of the same wardrobe record** as the item's parts, by the block rule of `material.md`
§12.4: 21,843 of the 21,999 items with maps and a dye code find a block with exactly that
`variant`; 156 do not (108 have code 1.0, the rest codes between 0.49 and 0.97 in records
whose tables stop earlier). The dye block's groups layer onto the item's parts as
`material.md` §12.3 says. A character's chosen dye replaces the item's default code.

<a id="s4-10"></a>
### 4.10 Material groups (Confirmed slots; priority use Reported by analogy)

The block's material groups apply to the block's own parts (`material.md` §9.3): a group
with `slot` 2 (`All`) to every part of the block, a group with a body slot to the block's
part in that slot, each to the submeshes whose surface binding key matches. In worn records
groups target `All` (41,806), a slot of one of the block's own parts (5,759) or, in
material-only dye blocks, a slot with no part (1,485); never a slot that another item fills.

> **OPEN:** how group priorities combine when several layers (surface materials, the item's
> group, a dye block's group) set the same parameter; `material.md` §9.2 layers them in
> order, which Tessera keeps. Group priority is probably the analogue of part priority.

<a id="s4-11"></a>
### 4.11 Blocks for entries without a base block (Confirmed; answers `setup.md` §10.1 OPEN)

The 3,102 entries with several blocks and no `variant` 1.0 are all **selection tables**:
dye tables (1,871), NPC and style records whose blocks are styles or variants (793 with
parts, 373 material-only), and the head, hair, beard and ear style records (65). The block is
always chosen by a modifier (§3.2.1) or a dye code (§4.9); a viewer without one shows the
first block. Worn item structs never land on such entries. Material-only entries (the
`setup.md` §10.2 OPEN) are the dye tables of the same record or selection tables of NPC
pieces; they apply only together with the record's parts as above.

---

<a id="s5"></a>
## 5. Held items

<a id="s5-1"></a>
### 5.1 Chain and worn-versus-held (Confirmed)

An item that is held resolves through `PhysObj` → 0x47 → 0x1F → 0x04 (`setup.md` §3.1) and
is placed at a holding location of the wearer (`setup.md` §13.1, `skeleton-anim.md` §8.4).
An item with an `Item_WornAppearanceMapList` is worn instead (`setup.md` §13.3): this
concerns the 200 battle gauntlets (`Weapon_Primary`, drawn as `Hands` parts) and the 120
class satchels (`Class`, drawn in the `Satchel` slot). All held weapons' setups have one part
and a one- or two-bone skeleton.

<a id="s5-2"></a>
### 5.2 Slots and categories (Confirmed census of items with `PhysObj` and no worn appearance)

| Default slot | Items | `Item_EquipmentCategory` (items) | `Inventory_ParentingLocation` |
|---|---|---|---|
| `Weapon_Primary` | 19,776 | `Orb` 1,929, `1HSword` 2,243, `Spear` 1,417, `Dagger` 1,844, `1HAxe` 1,653, `1HMace` 1,674, `1HHammer` 1,493, `1HClub` 1,648, `2HSword` 1,069, `2HStaff` 1,045, `2HAxe` 1,043, `2HClub` 926, `2HHammer` 881, `Halberd` 838, `Cosmetic` 58, `Implement1H` 12, `Implement2H` 3 | none |
| `Weapon_Secondary` | 2,973 | `Shield` 1,157, `HeavyShield` 874, `MediumShield` 870, one-handed weapons 71, `Bauble` 1 | `Wpn_Forearm` (shields; `Forearm` on 2) |
| `Weapon_Ranged` | 5,272 | `Instrument` 1,894, `Bow` 1,383, `Javelin` 939, `Crossbow` 893, `Class` 163 | `Wpn_ROffhand` (javelins, crossbows) |
| `Class` | 825 | `Class` 579, `Instrument` 246 | none |
| `Weapon_Primary_FX` | 36 | `Weapon_FX` | none |

One-handed weapons are compatible with both weapon slots (`Inventory_CompatibleSlot`
`Weapon_Primary` | `Weapon_Secondary`); two-handed weapons, staves, spears, halberds and orbs
only with `Weapon_Primary`. `Item_EquipmentCategory` labels come from mapper 0x2300004C.

<a id="s5-3"></a>
### 5.3 Attach location (Confirmed sources, rule partly OPEN)

The sources and the viewer rule are in `skeleton-anim.md` §8.5: `Weapon_Primary` →
`Wpn_RHand`, `Weapon_Secondary` → `Wpn_LOffHand`, `Weapon_Ranged` → `Sheath_Back_Bow` (the
inventory-control object's paperdoll list), replaced by the item's
`Inventory_ParentingLocation` in the world. On the player setups these resolve to:

| Location | Human M 0x04000553 bone (`BoneType`) | Hobbit M 0x04000551 | Dwarf M 0x04000557 |
|---|---|---|---|
| `Wpn_RHand`, `Wpn_ROffhand`, `RHand` | 79 `Rweaponholder` | 41 | 41 |
| `Wpn_LOffHand`, `Wpn_LHand`, `LHand` | 65 `Lweaponholder` | 27 | 29 |
| `Wpn_Forearm` | 54 `Lelbow` | 17 `Lulna` | 21 `Lulna` |
| `Forearm` | 54 `Lelbow` | 16 `Lelbow` | 20 `Lelbow` |
| `Sheath_Back_*` | 16 `Thorax` | 5 `Thorax` | 5 `Thorax` |
| `Sheath_LHip`, `Sheath_RHip` | 3 (hip girdle) | 51 | 51 |

`Class`-slot held items and instruments have no location in the data (the paperdoll list
names only the three weapon slots). Their `PhysObj` is mostly one generic entity
(0x470001AE) shared by 579 class items and 246 instruments.

> **Decision:** do not draw `Class`-slot held items and `Weapon_Primary_FX` items in the
> world for M4. **OPEN** (owner observation O13): where they appear.

<a id="s5-4"></a>
### 5.4 Wielded versus stowed (Confirmed data, rule OPEN)

Every player setup carries stowing locations (`Sheath_LHip`, `Sheath_RHip`,
`Sheath_Back_2H`, `Sheath_Back_2H_Sword`, `Sheath_Back_Bow`, `Sheath_Back_Crossbow`,
`Sheath_Back_Shield`; `skeleton-anim.md` §8.3), but no item or inventory property selects
one; the enum `Inventory_Cosmetic_IsSheathed` (`Sheathed`, `Unsheathed`, `Combat_Defined`,
`Sheathed_Staff`, `Combat_Defined_Staff`) shows that the state exists, and the combat-stance
enum `Inventory_EntityMode_*` (`combat_1h`, `combat_2h`, `combat_bow`, `combat_xbow`,
`combat_1hshield`, `combat_dualwield` …) belongs to the script tables.

> **Decision** (until `script-table.md` or owner observation settles it): out of combat,
> stow by category: one-handed in the primary slot → `Sheath_LHip`, in the secondary slot →
> `Sheath_RHip`; `2HSword` → `Sheath_Back_2H_Sword`; other two-handed weapons, staves,
> spears and halberds → `Sheath_Back_2H`; `Bow` → `Sheath_Back_Bow`; `Crossbow` →
> `Sheath_Back_Crossbow`; shields → `Sheath_Back_Shield`; orbs and javelins stay in hand.
> In combat use §5.3. **OPEN** (owner observation O13).

<a id="s5-5"></a>
### 5.5 NPC held items (Confirmed)

NPC-like figures name held props directly by entity: `NPC_Cosmetic_Inventory_Item1` … `4`
(a 0x47 DID) with `NPC_Cosmetic_Inventory_HoldingLocation1` … `4` (`HoldingLocation`; the
small keys 1 `RHand`, 2 `LHand`, 18 `RHand2` and 0x1000003B `Wpn_RHand` are the common
values) and `NPC_Cosmetic_Inventory_Visible`. In the landblock instance sets: 30,419 sets
have item 1 (28,807 non-null), 15,295 item 2, 201 item 3. The prop resolves and attaches like
any held item (§5.1) at that location of the NPC's own setup.

---

<a id="s6"></a>
## 6. Scale, height and bone sliders

<a id="s6-1"></a>
### 6.1 Whole-body scale (Confirmed values; composition OPEN)

Two values scale a whole body uniformly:

- the template's `vec3` (`setup.md` §5.2): 1.03 on Elf M and HighElf F, 1.05 on HighElf M;
- the entity's `Render_AdjustableScale` (equal to `Physics_AdjustableScale` everywhere): 1.02
  on Elf F and HighElf F, 1.03 on both Beorning entities, **0.7** on both RiverHobbit
  entities, 1.0 elsewhere.

The River-hobbit value is corroborated by `Physics_AvatarFloatHeight`: 0.985 (M) and 0.97
(F) against 1.42 and 1.40 on the Human entities that share the setups (0.69 and 0.69). With
the product of both values, standing heights (bind geometry top × scale) become: Hobbit
1.16/1.14 m, Human 1.81/1.77, Elf 1.85/1.80, Dwarf and DwarfStoutAxe 1.36, Beorning
1.86/1.82, HighElf 1.90/1.84, RiverHobbit 1.27/1.24 (M/F). This answers the
`skeleton-anim.md` §2.2 River-hobbit question as far as the data can: the setup is drawn at
0.7.

> **Decision:** model scale = template `vec3` (1 if absent) × entity `Render_AdjustableScale`
> × instance `Render_AdjustableScale` (landblock instance set or SED, §7.2), applied
> uniformly at the object's origin before the world placement.
> **OPEN** (owner observation O8): whether template and entity scales multiply (HighElf F
> would be 1.0506) or one replaces the other.

The template floats `value_a` and `value_b` (`setup.md` §5.2) are not scales (the hobbit
setups are already hobbit-sized); assembly ignores them.

<a id="s6-2"></a>
### 6.2 Height controls (Confirmed storage and effect; A15)

| Property | Range in the control | Default on the entity | Present on | What it drives |
|---|---|---|---|---|
| `AC_BoneScale_Test` (float) | 0 to 1 (`AppearanceUI_BoneScalingControl`); RiverHobbit M: 0 to 0.6 | 0.5 | all 14 avatar entities; presets use 0.5 and 0.85 | the held fraction of the avatar's **body-shape deformer** clip (§6.4), and on DwarfStoutAxe also of its layer-110 `scale_blend` clip |
| `AC_Avatar_Height` (float) | RiverHobbit M 0 to 0.95, F 0 to 0.6 | 0 | RiverHobbit entities only | the held fraction of the River-hobbit layer-110 `scale_blend` clip |

Both act through script nodes of the avatar's **species table** (`script-table.md` §6.8,
§6.9), whose fields are driven by these properties (`script-table.md` §4.2):

- **`AC_Avatar_Height`** sets `ManualTime` of the held layer-110 node (River-hobbit M clip
  0x05008E76, 1.666 s; F clip 0x05008E75, 0.416 s). The clip is held at
  fraction = value (looping node: the fractional part, so 1.0 would wrap to 0; the ranges stop
  short of 1), never played over time, and blended by the layer rule
  (`skeleton-anim.md` §6.6). At the default 0 the pelvis is about 0.098 m lower and the legs
  as much shorter than at fraction 1 (before the entity scale 0.7); higher values make the
  character taller; the feet stay on the ground at every value. **The default 0 is the
  shortest River-hobbit.**
- **`AC_BoneScale_Test`** sets the time fraction (limited to [0, 1], no wrap) of the
  species table's MeshDeformer node, whose clip reshapes the body in bind space (§6.4). On
  the Hobbit, Human, Elf, HighElf, Dwarf and Beorning bodies the clips change girth (x and y
  scales of the torso, hips and limbs) and leave the height unchanged at every value; on the
  River-hobbit bodies the clip also gives the hobbit head, face and feet (§6.4) at every
  value. On DwarfStoutAxe it also sets `ManualTime` of the held, non-looping layer-110 clip
  0x050079DF, which makes the body about 0.10 m taller on longer legs at every value and
  widens the shoulders more as the value rises.

No script node of any table or scriptlet reads `Character_TestScale` or
`GRSEvent_AppearanceMod_BoneScale` (Confirmed, all records); Tessera ignores them.

> **OPEN** (owner observation O9, confirmation only): the shortest and tallest character at
> the two ends of each control, compared with §6.5.

<a id="s6-3"></a>
### 6.3 Bone sliders (Confirmed layout and limits; value rule BIN, A16)

**Where.** The character-creation UI finds the definitions through the avatar entity's
`AppearanceUI_BoneSliderControl` (`AppearanceUI_BoneSliderDefinitionsFile`): Human M/F
0x7800A9F1; Elf M/F and HighElf M/F 0x7800BEAB; RiverHobbit M 0x7800A9F3, F 0x7800A9F4. The
**figure** gets its sliders from the SliderMeshDeformer node (kind 81) in the `Startup` script
of its species table (`script-table.md` §6.9): the node names its own definitions record
(Human M, Elf M and HighElf M 0x7800A9F1; Human F 0x7800A9F2; Elf F and HighElf F
0x7800BEAB; River-hobbit as above; 0x7800A9F1, 0x7800A9F2 and 0x7800BEAB have identical
content) and its value string is driven by the character's `AC_BoneSlider_Value_Array`. Use
the node's record. Only the Human, Elf, HighElf and River-hobbit avatars have sliders; Hobbit, Dwarf, DwarfStoutAxe and
Beorning avatars have none.

**Definition record.** `BoneSliderDeformer_SliderDefinitionArray` of
`BoneSlider_DefinitionStruct`, in slider order:

| Field | Type | Meaning |
|---|---|---|
| `BoneSlider_Name` | enum (mapper 0x23000437) | Slider identity (`Head_Width`, `Nose_Scale`, `Eye_Separation`, …) |
| `BoneSlider_SliderGroup` | enum (0x2300048C) | `Head`, `Nose`, `Jaw`, `Mouth`, `Eyes`, `Body` |
| `BoneSlider_BoneTranformMaskArray` | array of bit fields (0x2300043D) | Channels driven: `TranslateX/Y/Z`, `RotateX/Y/Z`, `ScaleX/Y/Z` (key = bit + 1) |
| `BoneSlider_LimitMin`, `BoneSlider_LimitMax` | vec3 | Channel values at the two ends; only the masked components are used |
| `BoneSlider_BoneIndexArray` | array of `BoneSlider_BoneIndexStruct` (`BoneSlider_BoneIndex` int, `BoneSlider_BoneReverse` int 0/1) | Bones driven; reverse = mirror the value for that bone |
| `BoneSlider_UI_Visible` | bool, optional | false on hidden fixed sliders |

**The 35 visible sliders** are identical in all four records: head width, brow inner and
outer height and scale, nose bridge, position, tip and nostril height and scale, mouth
position and scale, upper-lip centre and outer, lower-lip centre, cheek snarl, scale and
bone, eye height, separation, rotation, scale and lid, jaw height and scale, chin height,
width and depth, ear height, separation, rotation and scale. Channel kinds and limits:

| Kind | Sliders | Limits |
|---|---|---|
| Translate Z (up) | 14 | ±0.002 to ±0.005 m |
| Translate X (sideways), mirrored | 2 (eyes, ears) | ±0.0015, ±0.003 m |
| Rotate X or Y, mirrored or not | 2 (eyes ±0.1, ears ±0.15) | radians (Reported unit; ±0.1 rad ≈ ±5.7°) |
| Scale XYZ (uniform limits, except the upper-lip centre at 0.8–1.2 in Z) | 13 | 0.7–1.3 (nose bridge) down to 0.95–1.05 |
| Single-axis scale | 4 (head width, jaw, chin width X; chin depth Y) | 0.91–1.09 at most |

Bone indices are indices into the **skeleton of the setup** (the 89-bone male or female
skeleton). They match the setup's `attach_map` labels (`skeleton-anim.md` §8.1): 18 `head`,
19/22 `Face_Eye_Rotation_Scale_Left/Right`, 21/24 eye lids, 25 cheek bone, 28 cheek, 30 jaw,
31 chin, 33/34 brows, 37–40 nose, 41–47 mouth, 50/51 ears (index 26, the cheek snarl bone,
has no `BoneType`). `BoneSlider_BoneReverse` is 1 on the left bone of the two mirrored
sideways sliders and the eye rotation.

**RiverHobbit additions.** 0x7800A9F3 and 0x7800A9F4 append two hidden sliders
(`BoneSlider_UI_Visible` false, group `Body`, both named `Femur_Scale`) whose minimum equals
their maximum: Scale Z 0.8 on bones 4 and 8 (`Lfemur`, `Rfemur`) and Scale Z 1.15 on bones 5
and 9 (`Lknee`, `Rknee`). They change the River-hobbit leg proportions regardless of any
value: in the bind-space deformation of §6.4 the thigh vertices shrink to 0.8 of their
height about the hip joints and the shin and foot vertices to 0.8 × 1.15 = 0.92 about the
knee and ankle joints, while no joint moves (`skeleton-anim.md` §5.4).

**Values (BIN, A16).** The value string holds hexadecimal byte pairs. The client reads
it pair by pair from the start (each pair read as a base-16 number, so a pair that is not
hexadecimal reads 0) and gives the *k*-th pair to the *k*-th slider **of the definition
array, hidden ones included**, as *t* = byte / 255; it stops when every slider has a value or
the string ends; every slider left without a value gets *t* = **0.5** exactly. With the
35-pair strings of this install, the hidden River-hobbit sliders (definitions 36 and 37) get
0.5, which changes nothing because their minimum equals their maximum. The avatar entities'
strings are 35 × `80` (*t* = 128 / 255 = 0.50196, so for example a 0.7–1.3 scale slider gives
1.00118, not exactly 1). The SliderMeshDeformer node's own stored string is 34 × `80` with
`00` at slider 24 (`Eye_Rotation`).

> **OPEN** (driven value of an absent property): the Elf and HighElf avatar entities carry no
> `AC_BoneSlider_Value_Array`. Whether the driven field then keeps the node's stored string
> (eye rotation at its minimum, −0.1) or receives the property's dictionary default was not
> traced. **Decision** (until known): use 35 × `80`, the value every other slider avatar
> carries.

**Per-bone entries (BIN).** The sliders are turned into one entry per bone, in definition
order, starting from translation (0, 0, 0), angles (0, 0, 0), scale (1, 1, 1) and an empty
mask:

1. For slider *i* with value *t*: channel vector *c* = `BoneSlider_LimitMin` + *t* ×
   (`BoneSlider_LimitMax` − `BoneSlider_LimitMin`), per component.
2. For every bone of its `BoneSlider_BoneIndexArray`, with sign *σ* = −1 when
   `BoneSlider_BoneReverse` is 1, else +1: for every channel bit in the slider's mask, **write**
   (do not add or multiply) *σ* × the matching component of *c* into that bone's entry:
   `TranslateX/Y/Z` → translation x/y/z, `RotateX/Y/Z` → angle x/y/z (radians), `ScaleX/Y/Z`
   → scale x/y/z. The sign applies to every channel kind (no reversed scale slider exists).
   Add the slider's bits to the entry's mask.
3. A later slider that writes the same channel of the same bone replaces the earlier value.

**Applying an entry to a local transform (BIN).** For each bone with an entry, on the
deformation pose of §6.4 (never on the animated pose):

- if the mask has any translate bit: local translation += the entry's translation;
- if it has any rotate bit: local rotation = local rotation · *q*, where *q* is the
  rotation **about local +Y by angle x, then about +Z by angle y, then about +X by angle z**
  composed as *R*_Y(angle x) · *R*_Z(angle y) · *R*_X(angle z) (Hamilton products, active,
  quaternions as `skeleton-anim.md` §5.2). Note the axis shift: the `RotateX` channel turns
  about Y, `RotateY` about Z, `RotateZ` about X;
- if it has any scale bit: local scale ×= the entry's scale, per component.

Unmasked components hold their start values (0, or 1 for scale), so they change nothing.

<a id="s6-4"></a>
### 6.4 Bind-space deformation: body shape and sliders (BIN, A17)

The original applies neither the body-shape deformer nor the bone sliders to the animated
pose. It deforms the skinned meshes **once, in bind space**, and animates the deformed
meshes with the ordinary skeleton. Inputs, all from the avatar's species table
(`script-table.md` §6.9): the MeshDeformer node(s) (clip, time fraction from
`AC_BoneScale_Test`, `UseBoneWeights`) and, on Human, Elf, HighElf and River-hobbit bodies, the SliderMeshDeformer node
(entries of §6.3).

Algorithm (run at assembly and again whenever a deformer time or the slider string
changes):

1. **Deformation local pose.** With at least one MeshDeformer: sample each deformer clip at
   time = (fraction limited to [0, 1]) × its duration; its track weights are
   ⌊255 × clip weight⌋ / 255 when `UseBoneWeights` is 1 and the clip has a weight array,
   else 1; combine all deformer clips per bone by steps 4–5 of `skeleton-anim.md` §6.6
   (normalised weighted average, reference pose filling a total below 0.1). Without one:
   the skeleton's reference pose. No animation clip takes part, whatever is playing.
2. **Sliders.** Apply every bone entry of §6.3 to this local pose.
3. **Deformation transforms.** Compose model triples and form each bone's skinning
   transform *D_b* by `skeleton-anim.md` §5.4 (against the ordinary reference pose).
4. **Deform the vertices.** For every skinned part drawn on the figure's skeleton (body parts
   and worn parts alike), replace each bind-space vertex *v* by Σ *w_k* · *D*_{j_k} · *v* over
   its own influences (`skeleton-anim.md` §4.7). Normals: the same blended transform without
   translation, renormalised (Reported by analogy; the client's normal handling was not
   traced).
5. **Draw.** Each frame, skin the deformed vertices with the animated palette
   (`skeleton-anim.md` §10) exactly as undeformed ones; the reference pose and inverse
   reference stay those of the skeleton.

With the defaults (`AC_BoneScale_Test` 0.5, slider strings of 0x80) the Human, Elf, HighElf,
Hobbit, Dwarf, Beorning and DwarfStoutAxe deformations leave the top and bottom of the
default body within 1 mm of the undeformed mesh. The River-hobbit deformation is the
hobbit look: its body-shape clip at every value scales the head 1.25 in height and 0.96 in
width, moves the face bones 1.2 times further from the head joint, scales the eye-offset bones
0.8, scales the feet 1.21 in length and 1.4 in height and lifts the foot joints 1.2 cm (which
keeps the larger feet on the ground), plus girth changes that vary with the value; the
fixed leg sliders shorten thigh and shin vertices about their joints (§6.3).

Why this matters (RH): applied per frame to the animated pose with matrix composition, the
same two leg sliders lift the River-hobbit feet 8.5 cm (M) and 8.8 cm (F) off the ground,
the float the M4 viewer showed; by this algorithm the lowest vertex stays within 1 cm of the
ground (§6.5).

<a id="s6-5"></a>
### 6.5 Expected standing heights (Confirmed computation, A18)

Computed by RH on the real data with §6.1–§6.4, `script-table.md` §6.7–§6.8 and
`skeleton-anim.md` §5.4, §6.6: setup default parts only (no appearance-list head or hair, no
worn items), idle clip of §8.3 sampled at time 0, body-shape value 0.5, slider strings
35 × `80`, unmounted, model scale of §6.1. Heights in metres at model scale.

| Body | Height control value | Top of the mesh | Lowest vertex | Lowest bone |
|---|---|---|---|---|
| River-hobbit M (scale 0.7) | `AC_Avatar_Height` 0 (default) | 1.231 | −0.005 | +0.011 |
| River-hobbit M | 0.5 | 1.266 | −0.003 | +0.013 |
| River-hobbit M | 0.95 (maximum) | 1.300 | −0.001 | +0.016 |
| River-hobbit F (scale 0.7) | 0 (default) | 1.200 | −0.008 | +0.009 |
| River-hobbit F | 0.3 | 1.215 | −0.008 | +0.009 |
| River-hobbit F | 0.6 (maximum) | 1.245 | −0.007 | +0.010 |
| DwarfStoutAxe | `AC_BoneScale_Test` 0.5 (default) | 1.434 | +0.007 | +0.037 |
| Dwarf M (comparison) | – | 1.345 | +0.000 | +0.030 |
| Human M (comparison) | – | 1.806 | −0.001 | +0.022 |

Invariant for tests: on every playable body, with every height value in its control's range,
the lowest skinned vertex of the idle pose at time 0 lies within 0.01 m of the ground, and
the River-hobbit top rises monotonically with `AC_Avatar_Height`.

> **OPEN** (owner observation, implementation feedback 2026-10-05, moving): §6.5 covers the
> idle pose only. With §6.2–§6.4 as implemented (default parts, `AC_Avatar_Height` 0,
> model scale 0.7, two cycles sampled every 0.05 s), the lowest skinned vertex while
> **walking** reaches −0.055 m on River-hobbit M against about −0.02 m on Human M, Hobbit M
> and River-hobbit F; running, River-hobbit M and Human M both reach about −0.037 m. Taking
> the River-hobbit M walk apart: −0.037 m without the held layer-110 clip, −0.033 m without
> the bind-space deformation, −0.016 m without both (the held clip shortens the legs along
> the leg, so an angled stance leg sinks; the body-shape clip enlarges the feet). Do
> River-hobbit toes visibly dip into flat ground at toe-off while walking in the original
> client? If not, which input differs (held-clip blending while moving, foot scaling,
> skinning form of `skeleton-anim.md` §15)?

---

<a id="s7"></a>
## 7. NPCs and creatures

<a id="s7-1"></a>
### 7.1 The 775 weenie objects placed by SEDs (Confirmed; corrects `entity-descriptor.md` §4.1)

`entity-descriptor.md` §4.3: 33,676 SEDs of type `Weenie` place 775 distinct 0x70 objects.
Opening all 775 property sets:

| `WeenieType` | Objects | SEDs | What they are | `PhysObj` |
|---|---|---|---|---|
| `Entity` (housing hooks) | 744 | 32,338 | `HousingHook_ID`, `HousingHook_HookCategory` (furniture, wall, floor, yard … hook categories), `HousingHook_IsPropertyHook`; class index 1989 | small marker setups (one part) |
| `DistantBattleUnit` | 29 | 1,335 (up to 229 per object) | `DistantBattleUnit_Formation` groups, each naming a unit game object (`DistantBattleUnit_Formation_Unit`, `Monster` objects with their own creature or humanoid `PhysObj`), spawn rates, widths; class index 3725 | one marker entity 0x47006915 |
| `Door` | 1 | 1 | a door | 0x4700C408 |
| `Monster` | 1 | 2 (landblock 0x80253E07) | one humanoid monster type (random-appearance type 0xDB) | 0x4700005B → setup 0x0400003E |

None of them is an NPC with species, worn items or held items. An offline walker draws the
housing hooks' markers only in a debug view, the door through its `PhysObj`, and treats the
distant-battle units as OPEN (they spawn crowds of their unit types; M7 or later).

<a id="s7-2"></a>
### 7.2 Where NPC-like figures come from (Confirmed)

**The landblock weenie list is an instance-property table** (corrects `landblock.md` §4.8):
its 3,768,840 entries are (IID, property set) pairs. 3,253,853 of them carry the IID of a SED
of the same landblock, which is exactly the number of distinct SED IIDs, so every SED has
one instance property set (at least `Render_AdjustableScale` and `Physics_AdjustableScale`;
all 3,768,840 have both). The other 514,987 belong to IIDs that no SED places (kind byte 4 in
411,154): server-spawned objects and generators (`Generator_ProfileDID` in 33,902 of them).

Among the instance sets of **Physics SEDs** (`entity-descriptor.md` §4.1 type 7):

| Instance content | Physics SEDs |
|---|---|
| `Appearance_InstanceList` | 379,660 (mostly static props: banners, coloured props) |
| `NPC_StartingIdleAnimation` (with `NPC_PlayingStartingIdle`) | 51,113 |
| `NPC_Cosmetic_Inventory_Item1` | 18,009 |
| `Appearance_RandomAppearanceType` | about 43,000 |

So the client places **static NPC-like figures** itself: a Physics SED names a 0x47 whose
setup is a humanoid rig (§7.3), and its instance set makes it a dressed, animated figure.
Of the 44,661 such SEDs with appearance, idle or held-item properties: random-appearance
type only 42,981; explicit appearance list 1,411; neither 269. Held items are present on
17,983, an idle animation on 43,309.

**Server NPCs** (not placed by the landblocks): `WeenieType` `RealNPC` 16,621 objects and
`Monster` 20,161 objects among the 0x79 property sets. Their own visual data is only
`PhysObj` (and `Agent_Species`, which is `Placeholder` on almost every `RealNPC`):

| Kind (by setup, §7.3) | `RealNPC` | `Monster` |
|---|---|---|
| Humanoid rig, `Appearance_RandomAppearanceType` not `Undef` | 2,735 | 3,900 |
| Humanoid rig, random type `Undef` or absent | 13,174 | 14,915 |
| Creature or animated rig | 281 | 1,168 |
| Static setup | 431 | 178 |

Their outfits are not in their client records; in the original they come from the server at
run time. For Tessera they are M7 data (Tessera's own server sends appearance lists).

<a id="s7-3"></a>
### 7.3 Humanoid rigs versus creatures (Confirmed)

**Rule:** a setup is a **humanoid rig** when one of its parts is in slot `UpperBody`; it is a
**creature or animated rig** when it has skinned parts but no `UpperBody` part; otherwise it
is static. Census over all setups: 319 humanoid, 717 creature or animated, 27,519 static.
Humanoid rigs take appearance lists and worn pieces by slot; creatures are drawn with their
setup's parts (and any appearance list, which on creatures mostly recolours: `ClothingColor`
keys) and take held items only where their list A has the location.

The NPC humanoid rigs used by the static figures (default parts: real slots, others stubs):

| Setup | Default real parts | Height (m) | Used by |
|---|---|---|---|
| 0x0400003E | `Head`, `UpperBody` (full body), `Crest` | 1.79 | 5,575 `RealNPC`; most static male figures |
| 0x04000006 | `Head`, `UpperBody`, `Crest` | 1.80 | 2,169 `RealNPC` |
| 0x04000004 | `Head`, `UpperBody`, `Crest` | 1.17 | 1,061 `RealNPC` |
| 0x0400011F | `UpperBody`, `Crest`, `Head` | 1.14 | 611 `RealNPC` |
| 0x04000152 | `Head`, `Crest`, `UpperBody` | 1.35 | 2,398 `RealNPC` |
| 0x04000170, 0x0400016F | `Head`, `UpperBody` | 1.79, 1.76 | 855, 704 `RealNPC` |

Their `UpperBody` default is a complete clothed body, so a rig drawn with no appearance list
is already a plausible figure. These rigs carry `attach_map` and list A holding locations
(0x0400003E: 22 attach bones, 32 locations including `RHand`, `LHand`, `Wpn_RHand`,
`Wpn_LOffHand`, `Wpn_Forearm`, `Sheath_Back_Shield`).

Species and sex of an NPC are not separate inputs: the setup is the body. The entity's `Sex`
property, `Av_Avatar_Race_Gender` (3 entities; values from 0x230003E4 `Hobbit_Female` …
`RiverHobbit_Female`, `Invisible`) and the voice-race property describe it but select no
geometry. NPCs on player bodies exist: 128 `RealNPC` objects resolve to player setups
(0x04000553: 27, 0x04000557: 26, 0x04000551: 19, 0x04000552: 16, 0x04000554: 15, 0x04000555:
15, 0x04000556: 10), and three 0x47 entities with `Av_Avatar_Race_Gender`
dress on the Human M template with an `Appearance_ClassList` that names the **player
wardrobe records** (Human M head, shoulder, chest, legs, gloves and boots records) with item
appearance keys and modifier 1.0: they assemble exactly like a player (§4).

<a id="s7-4"></a>
### 7.4 Dressing an NPC (Confirmed sources; random type OPEN)

Sources, applied in this order (§8):

1. The setup's default parts (§3.1).
2. The template's appearance section (`setup.md` §5.4; 577 templates): one 0x20 record with
   a modifier map whose keys are `AppearanceKey` values (`ClothingColor` in 575 sections,
   `Worn` 3) and whose values are modifiers. It is a one-record appearance list
   (key → modifier); mostly a dye of the parent template's geometry. 60 of them sit on
   skinned setups.
3. The entity's `Appearance_ClassList`.
4. The instance's `Appearance_InstanceList` if the instance set has one, otherwise the
   entity's own `Appearance_InstanceList` (Decision: an instance property replaces the
   entity property of the same name, as for every instance property).
5. Held props (§5.5).

NPC appearance records are small: an entry keyed `Worn` (0x1000003A) with one block at
`variant` 1.0 holding the piece's parts (all at priority 1), plus `ClothingColor` and
`ClothingColorSecondary` dye tables; NPC lists pair each `Worn` element (modifier 1.0) with a
`ClothingColor` element of the same or another record (modifier = dye code, for example
0.06 or 0.32). Some NPC lists use rig-specific keys (labels such as `NPC_Upperbody_Mesh`,
`NPC_Hair_Mesh`, `NPC_Cloak_Mesh`, `NPC_Head_Mesh`, `NPC_Helmet_Mesh` and their `_Dye`
partners) with style modifiers 0.00, 0.01, …

**Random appearance.** `Appearance_RandomAppearanceType` (enum 0x2300031D, 440 labels:
regional villager, guard, brigand and creature types) is set on 36,685 server objects and on
about 43,000 static figures. No client record maps a type to pieces: the property occurs only
in property sets of 0x79 (36,685), 0x47 (165) and landblock instance sets, in 72 script tables
(0x07) and 8 scriptlets (0x0C), and no array, struct or DID map is keyed by it.

> **Decision:** draw a random-appearance figure with its setup's default parts and its
> instance's held props. **OPEN** (owner observation O12): what such figures look like in the
> original (fixed or varying per visit), and whether the client builds them from a table not
> found here.

---

<a id="s8"></a>
## 8. Algorithm in words

Input: an object to draw (a player avatar entity, an NPC entity, or a Physics SED with its
instance property set), plus for a player the worn items with dyes, the held items and the
character's appearance values.

1. **Body.** Resolve the 0x47 through the entity chain to the template(s) and setup
   (`setup.md` §3.1). Load the skeleton (`skeleton-anim.md` §2).
2. **Slot table.** For every setup part, slot = mesh `flags` (`MeshTypeID`); enter it with
   the lowest priority.
3. **Appearance lists**, in the order of §7.4 (template section, class list, instance list).
   For each element: pick the record (toggle rule, §3.3), the entry (`key`), the block
   (§3.2.1; for a player's head, hair and beard the character's chosen style value replaces
   the modifier). Offer each part to its slot (§4.4); remember the block's material groups
   with the parts they belong to; a dye element (`ClothingColor` keys) selects a dye block of
   its record for the parts that record contributed.
4. **Worn items** (players and player-bodied NPCs), in ascending `ContainerSlot` order: pick
   the map struct for the wearer's species and sex (§2.3 Decision for gaps), the entry for
   `Item_AppearanceKey`, its `variant` 1.0 block; offer the parts; take the dye block for the
   item's dye code (§4.9).
5. **Resolve** every slot: highest priority wins, later source on ties (§4.7). Record which
   slots hold stubs.
6. **Materials.** For each winning part: surface materials, then the groups of its own block
   that target `All` or its slot with a matching binding key, then its dye block
   (`material.md` §9.3, §12).
7. **Scale, shape and sliders.** Model scale per §6.1. Once per change of the height
   controls or slider string (not per frame), deform the skinned parts in bind space by the
   body-shape deformer and the sliders (§6.4). The species table also starts the held
   proportion clip of River-hobbit and DwarfStoutAxe bodies (`script-table.md` §6.8), which
   the per-frame animation blends like any other layer.
8. **Draw** every non-stub winning part with identity part transform in the setup frame,
   skinned to the skeleton (`skeleton-anim.md` §9.1); stubs per the O4 policy (draw or skip;
   no effect on hiding).
9. **Held items.** For each held item or NPC prop: resolve its setup, choose the location
   (§5.3–§5.5), look it up in the wearer's list A, place it there each frame
   (`skeleton-anim.md` §8.4). Items whose location the wearer lacks are not drawn.

A figure is rebuilt (steps 2–6) only when its equipment, appearance values or instance set
change; the deformation of step 7 when a height control or the slider string changes; the
animation of the figure and steps 8–9 run per frame.

---

<a id="s9"></a>
## 9. Invariants (Confirmed over all records)

| ID | Invariant |
|---|---|
| **AI1** | Every `Item_WornAppearance` DID is an existing 0x20 record; every entry an item struct finds has a block with `variant` 1.0. |
| **AI2** | 0x20 key table value = number of the entry's blocks with `variant` < 0.5 (49,383 of 49,383). |
| **AI3** | In every block with groups, the float after the last group is 0.0; in every block without groups, the float after the group count is 0.0. |
| **AI4** | Every group's slot field and every part's slot field is a key of the `MeshTypeID` mapper (or its base). |
| **AI5** | Part and group priorities are finite whole numbers between 0 and 100. |
| **AI6** | For a given (species, sex, item slot) all worn items name one wardrobe record, except the chest/shoulder cross-overs (2 per body) and the null record for Dwarf F. |
| **AI7** | Bone-slider bone indices are below the bone count of every setup whose avatar names the definition record; `BoneSlider_BoneReverse` ∈ {0, 1}; each mask element has one bit set; the value string has two hex digits per visible slider. |
| **AI8** | Every appearance-list element names an existing 0x20 record or the null DID. |

A loader reports AI1, AI4, AI7 and AI8 violations as data-quality issues and continues
(dropping the offending element); AI2 and AI3 are decoder checks.

---

<a id="s10"></a>
## 10. Statistics and failure categories

| Item | Value |
|---|---|
| Worn items / map structs / distinct wardrobe records | 42,086 / 634,655 / 73 |
| Structs resolving to parts / material only / key missing | 632,056 / 2,339 / 260 |
| Appearance-list elements in 0x47 records (class + instance) | 12,199 |
| Landblock instance sets / with appearance lists | 3,768,840 / 421,791 |
| Static NPC-like Physics SEDs (humanoid rig with NPC properties) | 44,661 |
| Server `RealNPC` / `Monster` objects | 16,621 / 20,161 |
| Setups: humanoid / creature or animated / static | 319 / 717 / 27,519 |
| Stub meshes (three vertices) | 258 (234 skinned) |

Failure categories an assembler distinguishes:

| Category | Handling |
|---|---|
| No map struct for the wearer (after the §2.3 fallback) | Item not drawn; body part stays |
| Key missing in the record (260 structs) | Item not drawn for that body |
| Material-only entry where parts were expected (null record) | Nothing to draw |
| Block for the modifier not found | §3.2.1 rule 3 |
| Part mesh missing or undecodable | Slot keeps its previous winner; reported |
| Holding location missing on the wearer | Held item not drawn (debug: at origin) |
| Random-appearance type | Default parts (§7.4 Decision) |
| Chain, record or skeleton failures | As `setup.md` §10 and `skeleton-anim.md` |

---

<a id="s11"></a>
## 11. Corrections to earlier notes

| Earlier statement | Where | Finding |
|---|---|---|
| Species codes 117, 120, 125 "probably" High Elf, Stout-axe, unknown | MV `wardrobe.md`, DS §7.1 | `SpeciesType` labels: `HighElf`, `DwarfStoutAxe`, `RiverHobbit` (§2.2). |
| "Part tags are per record family, not global"; hats use tag 0x10000005 | MV `wardrobe.md`, `hair-face.md` | Slot values are global `MeshTypeID` keys; 0x10000005 is `Shoulder`, helmets use `Helmet` 0x1000000A (§4.4). |
| Stub parts recognised by file size under about 2 KB | MV | Stubs are the three-vertex meshes; and hiding follows from priority, not from recognising stubs (§4.4). |
| Block `q` = 0 or 1.0 is the base look | MV | 1.0 is the base of item entries; in style and dye tables the variant is the style index or dye code (§3.2.1). |
| Group fields `a`, `g` and `group_param` OPEN; float after the group | `setup.md` §8.3, `material.md` §9.3 | Float before each group = group priority; float after the last group = 0; `a` = `MeshTypeID` slot or `All` (§4.3). `part_param` = part priority (§4.4). |
| Key-table values OPEN | `setup.md` §8.4 | Count of blocks with `variant` < 0.5 (§4.8). |
| Default dye code source OPEN | `material.md` §12.4 | `Item_ClothingColor` / `Item_ClothingColorSecondary` (§4.9). |
| Weenie SEDs place NPCs and interactive objects | `entity-descriptor.md` §4.1 | Housing hooks, distant-battle units, one door, one monster (§7.1). |
| Landblock weenies are server-side templates | `landblock.md` §4.8 | Per-IID instance property sets for every SED plus server objects (§7.2). |
| River-hobbit drawn at setup height or scaled: unknown | `skeleton-anim.md` §2.2 | Entity scale 0.7, the layer-110 `scale_blend` clip held by `AC_Avatar_Height`, the body-shape deformer and the fixed thigh and knee sliders in bind space (§6.1–§6.5); the composition with template scale is OPEN. |
| Height controls neutral; sliders applied per frame to the local pose after sampling, matrix composition | this spec §6.2–§6.3 (Decisions before 2026-10-05) | Height controls hold deformer and `scale_blend` clips (§6.2); sliders and body shape deform the meshes once in bind space with triple composition (§6.3–§6.4). |
| Slider value *t* = byte / 255; "centred mapping equally plausible"; absent string means 0x80 | this spec §6.3, §14 | *t* = byte / 255 confirmed; sliders without a pair get 0.5; the absent-string case stays OPEN (§6.3). |
| Appearance keys head 0x10000009, hair 0x10000032, beard 0x10000018 | DS §7.1 | Confirmed; their labels (`FaceTexture`, `HeadMesh`) are misleading (§3.3). |

---

<a id="s12"></a>
## 12. Synthetic worked examples (for tests)

All values are invented for this spec.

<a id="s12-1"></a>
### 12.1 Slot resolution

Default body: `UpperBody` A, `LowerBody` B, `Hands` C, `Feet` D, `Crest` E, `Helmet` S0 (a
stub). Hair style block: `Crest` F at 1. Worn, in slot order: head item {`Helmet` G at 4,
`Crest` S1 (stub) at 10}; chest item {`UpperBody` H at 1, `LowerBody` S2 (stub) at 10,
`Hands` C at 10}; legs item {`LowerBody` J at 1}; gloves item {`Hands` K at 1}; boots item
{`Feet` L at 2}; back item {`Cape` M at 1, `Crest` S3 (stub) at 15, `Helmet` S4 (stub) at 15}.

| Slot | Winner | Visible |
|---|---|---|
| `UpperBody` | H (1 > default) | H |
| `LowerBody` | S2 (10 > J's 1) | nothing |
| `Hands` | C from the chest (10 > K's 1) | bare hands C |
| `Feet` | L | L |
| `Crest` | S3 (15 > S1's 10 > F's 1) | nothing |
| `Helmet` | S4 (15 > G's 4) | nothing |
| `Cape` | M | M |

Remove the back item: `Crest` → S1 (hidden), `Helmet` → G. Remove the head item as well:
`Crest` → F, `Helmet` → S0.

<a id="s12-2"></a>
### 12.2 Block choice

An entry with blocks of `variant` 0.00, 0.01, 0.02, 0.45, 0.80: modifier 0.02 → third block;
modifier 0.0199995 → third block (within 0.0005); modifier 1.0 → no 1.0 block, so the first
block (Decision); its key-table value is 4.

<a id="s12-3"></a>
### 12.3 Bone slider

A slider with mask `ScaleX|ScaleY|ScaleZ`, min (0.9, 0.9, 0.9), max (1.1, 1.1, 1.1), bone 7;
byte 0xFF → *t* = 1 → bone 7's local scale is multiplied by (1.1, 1.1, 1.1); byte 0x00 → 0.9;
byte 0x80 → 0.9 + 0.2 × 128 / 255 = 1.00039; no pair for it in the string → *t* = 0.5 → 1.0.
A mirrored `TranslateX` slider with limits ±0.003 on bones 12 (reverse 1) and 13 (reverse 0)
at byte 0xFF moves bone 12 by −0.003 and bone 13 by +0.003 along their local x.

<a id="s12-4"></a>
### 12.4 Slider entries: overwrite and rotation axes

- Two sliders, in this definition order, on bone 5: A with mask `ScaleX`, limits 1.0–1.2,
  byte 0xFF; B with mask `ScaleX|ScaleZ`, limits 0.9–0.9 (fixed). Entry of bone 5: scale
  (0.9, 1, 0.9) (B overwrites A's x; no product 1.08), mask `ScaleX|ScaleZ`.
- A slider with mask `RotateX`, limits −0.1 to 0.1 radians, byte 0xFF, bone 19 (reverse 1)
  and bone 22 (reverse 0): angles (−0.1, 0, 0) on bone 19 and (0.1, 0, 0) on bone 22; bone
  19's local rotation is post-multiplied by a turn of −0.1 rad about its local **+Y**
  axis, bone 22's by +0.1 rad about +Y.
- Angles (a, b, c) on one bone give *R*_Y(a) · *R*_Z(b) · *R*_X(c). For a = 0.3, b = −0.7,
  c = 1.1 rad the quaternion (x, y, z, w) is (0.44180, −0.05754, −0.36242, 0.81863).

<a id="s12-5"></a>
### 12.5 Bind-space deformation of a leg (§6.4)

Skeleton (reference rotations identity, scales 1): hip bone 0 at z 1.0; knee bone 1 (child)
at local (0, 0, −0.45); ankle bone 2 (child of 1) at local (0, 0, −0.41). No body-shape
deformer; two fixed sliders: bone 0 `ScaleZ` 0.8, bone 1 `ScaleZ` 1.15. Deformation pose =
reference pose with those scales. Model scales z: 0.8, 0.92, 0.92; joints stay at 1.0, 0.55,
0.14.

| Bind vertex (z), bone | Deformed z |
|---|---|
| 0.775, bone 0 | 1.0 + 0.8 × (−0.225) = 0.82 |
| 0.345, bone 1 | 0.55 + 0.92 × (−0.205) = 0.3614 |
| 0.0 (sole), bone 2 | 0.14 + 0.92 × (−0.14) = 0.0112 |

Animating with the reference pose leaves these values unchanged; the sole stays 1.1 cm above
the ground. Applying the same sliders to the animated pose with matrix composition instead
would put the sole at 0.134 (`skeleton-anim.md` §14.7) — the floating-feet error.

---

<a id="s13"></a>
## 13. Open questions

> **OPEN** (owner observation O8): composition of template `vec3` and entity
> `Render_AdjustableScale` (§6.1).

> **Resolved** (2026-10-05, BIN): the height controls' effect (§6.2), the bone-slider byte
> mapping, combination, rotation axes and frame (§6.3–§6.4). **OPEN** (owner observation
> O9, confirmation only): compare the shortest and tallest River-hobbit with §6.5; the
> driven value of an absent slider string (§6.3).

> **OPEN** (owner observation O10): equal-priority ties between worn items and in NPC lists
> (§4.7); confirmation of the layering cases of §4.6.

> **OPEN** (owner observation O11): the updated-avatar toggle and which style files the
> client uses by default (§3.3).

> **OPEN** (owner observation O12): random-appearance figures (§7.4).

> **OPEN** (owner observation O13): stowed weapon locations by category, `Class`-slot held
> items (§5.3, §5.4).

> **OPEN** (frame capture, O15): skin, hair and eye colour tinting; face compositing (§3.3).

> **OPEN:** `section` byte of 0x20 records (§4.2); how material group priorities combine
> (§4.10); distant-battle units (§7.1).

> **OPEN** (owner observation O4, existing): whether stub triangles are drawn (§4.4).

None of these blocks assembling and animating the 14 playable bodies with worn and held
items; they affect fidelity of scale, sliders, ties and NPC looks.

---

<a id="s14"></a>
## 14. Implementation feedback (clean room, 2026-10-05)

`tessera-formats` (`appearance`, `avatar`, `item` modules) was implemented from this spec
alone. Its real-data test (`crates/formats/tests/appearance_real_data.rs`) reproduces on the
same install: AI2 and AI3 in all 4,201 records (49,383 entries, 3,102 without a base block,
74,839 blocks), the part and group priority histograms of §4.3, the §3.2.1 census of the
12,199 appearance-list elements (11,823 rule 1, 295 rules 2–3, 67 key missing, 14 null DID),
the worn census of §4.5 and §10 (42,086 items, 634,655 structs, 73 records; 632,056 parts,
2,339 material only, 260 key missing), the dye census of §4.9 (21,999 items, 21,843 exact,
156 not) and the 14 bodies of §2.2 with their setups, template scales (1.03, 1.05, 1.03) and
entity scales (1.02, 1.03, 0.7). Points for the spec owner:

> **OPEN:** §2.2 and §6.1 give `Render_AdjustableScale` as a number (1.02, 0.7 …); in the
> data it is a **vector3** with three equal components (for example (0.7, 0.7, 0.7) on the
> River-hobbit entities). The implementation accepts a float or a uniform vector and
> ignores a non-uniform one. Please state the type, and what a non-uniform value would mean.

> **Answered in part** (2026-10-05, spec writer; was: absent `AC_BoneSlider_Value_Array` on
> the Elf and High-elf entities). Correct: only the Human and River-hobbit entities carry the
> string; §6.3 now says so. The client's slider input is the SliderMeshDeformer node of the
> species table, whose own stored string is 34 × `80` with `00` at slider 24; whether an
> absent property leaves that string or yields the dictionary default was not traced
> (§6.3 OPEN). Keep 0x80 for every slider (§6.3 Decision). Two related corrections: sliders
> for which the string has no pair get *t* = 0.5 exactly, not 0x80; and the definitions
> record is the node's (Elf M and HighElf M: 0x7800A9F1, identical in content to 0x7800BEAB).

> **OPEN:** §5.3 (and `skeleton-anim.md` §8.5) take the paperdoll location per equipment
> slot from the inventory-control object's `InventoryControl_SlotToPaperdollLocation_MappingList`,
> but the field names of its element struct are not given, so a clean-room reader cannot
> extract the pairs. The implementation's `HoldingRule` takes (slot bits, location) pairs from
> the caller; its test builds them from the `ContainerSlot` and `HoldingLocation` labels of
> §5.3 (bit-to-key rule, `property-system.md` §7.3). Please name the struct's fields.

> **OPEN** (§2.3, §10): an item that has wardrobe structs for other bodies but none for the
> wearer (after the fallback) and also a `PhysObj` is treated as **not drawn**, as §10's
> first failure category says, never as a held item. Please confirm that the `PhysObj` of
> a worn item is never shown on a body it does not fit.

> **OPEN** (§5.5, review 2026-10-05): §5.5 names `NPC_Cosmetic_Inventory_Visible` without
> giving its meaning. A review probe found that of 28,807 landblock instance sets with a non-null
> `NPC_Cosmetic_Inventory_Item1`, the flag is false in 633, true in 12,980 and absent in
> 15,194 (dictionary type Bool, default true). The implementation no longer hides a
> figure's props when it is false: `npc_held_items` returns them all and
> `npc_cosmetic_inventory_visible` reports the flag for the caller to decide. Please state
> whether false hides the props (always, out of combat, or only in some UI views).

> **Correction applied** (§5.3, review): the holding rule now follows `skeleton-anim.md`
> §8.5 exactly: in a paperdoll view the item's `Inventory_PaperdollParentingLocation`, else
> the slot's paperdoll location; the in-world `Inventory_ParentingLocation` is never used
> there (a review probe found it had leaked into the paperdoll view for 940 `Weapon_Ranged` items, javelins and
> some bows, which got `Wpn_ROffhand` instead of `Sheath_Back_Bow`).

> **Answered** (2026-10-05, spec writer, BIN; was: rotation order and several sliders on one
> bone). Neither the x-y-z order nor applying sliders one after another is the client's
> rule. Sliders first build one entry per bone, a later slider **overwriting** the channels
> it writes (§6.3 "Per-bone entries"); the entry's three angles then form one rotation
> *R*_Y(angle x) · *R*_Z(angle y) · *R*_X(angle z), which post-multiplies the local rotation
> (§6.3, worked example §12.4). And the entries are applied to the bind-space deformation
> pose of §6.4, not to the animated pose: `BoneSliders` in `tessera-anim` needs to move from
> the per-frame path to a mesh-deformation step.

> **OPEN** (header "Implements into", review): the slot table (`SlotTable`), the holding
> rule (`HoldingRule`) and the wardrobe fallbacks (`WardrobeFallbacks`) were implemented in
> `crates/formats` (`appearance`, `item` modules) for M4, although the header names
> `crates/assets` or a new assembly module. They are small and data-only, but the placement
> needs a decision record, and then this header and the `CLAUDE.md` layout line updated.

---

<a id="s15"></a>
## 15. Implementation feedback: assembly (clean room, 2026-10-05)

`tessera-mesh` (`assembly` module: `CharacterAssembler`, ARCHITECTURE §11.1) implements §8
on top of the `tessera-formats` decoders. Its real-data test
(`crates/mesh/tests/assembly_real_data.rs`) on the reference install assembles the 14 bodies
of §2.2 (setups, entities and scales as the table, with the scale Decision of §6.1), every
W, L and H row of `docs/checklists/m4-visual.md` (48 distinct items; all expected winners,
priorities, stubs, holding locations and bones as the checklist states, including the
Hobbit and Dwarf notes), the six N rows (setups, props at their locations, idle values) and
all 775 weenie objects of §7.1 (775 assembled, no body failures; one random-appearance
figure, type 0xDB). Choices the spec leaves open:

> **OPEN** (§3.2.1, §7.4, dye pairing): a `ClothingColor` list element is applied to the
> parts contributed by elements naming the **same record**. When that record contributed no
> parts, the implementation applies the dye to the setup's default parts that are still
> shown (Decision), and reports `unpaired-dye`. On the reference install this happens for
> N3 (element with record 0x2000080A, which has only a `ClothingColor` entry; the list's
> `Worn` element for it names a key the record lacks) and N4 (dye elements with 0x2000088E
> and 0x20000834, records whose `Worn` parts the list does not use), and for DwarfStoutAxe's
> one-block colour entry (§3.3). Template appearance sections (§7.4 step 2) are applied the
> same way, without a report. Please state how a dye element pairs with pieces of another
> record (by position after its `Worn` element, by record, or onto the rig).

> **OPEN** (§7.3, rig rule): the distant-battle marker entity 0x47006915 (29 weenie objects,
> §7.1) resolves to setup 0x040002DE, whose single part is in slot `UpperBody`. The rule of
> §7.3 therefore calls it a humanoid rig, while §7.1 describes it as a marker. Nothing in
> assembly depends on the classification (the part is drawn either way); please confirm the
> rule or refine it (for example, require a skinned part).

> **OPEN** (§7.4 step 4, weenie objects): a weenie object (0x70) has its own property set
> besides its entity's. The implementation lets an `Appearance_InstanceList` in the
> object's set replace the entity's, and the instance set's list replace both; NPC props,
> `NPC_StartingIdleAnimation` and `Appearance_RandomAppearanceType` are read with the same
> precedence (instance, object, entity). On the reference install all 775 weenie objects
> have an instance set, but neither it nor the object's set carries an instance list (all
> 775 use the entity's), so the object-set step is not exercised by the data.

> **OPEN** (§4.4, stubs): a part is a stub when its mesh has at most three vertices in all
> submeshes and levels together. Please confirm that no stub has more than one submesh.

> **OPEN** (§3.1, §4.4, static setups): static setups list several parts with the same
> `flags` (`Default`). The implementation keeps every default part, in order, and an offered
> part for a slot replaces **all** default parts of that slot (drawn once, at the first one's
> position). No observed appearance list offers a `Default` part to a static setup with
> several parts, so this is untested by the data.

> **OPEN** (§5.4 Decision, categories it does not name): `Implement1H`, `Implement2H`,
> `Cosmetic`, `Bauble`, `Instrument`, `Class` and `Weapon_FX` items stay at their in-hand
> (wielded) location when stowed. Please extend the Decision if they have stowing places.

Review follow-up (clean room, 2026-10-05):

> **OPEN** (§5.2, §5.4, `Item_EquipmentCategory` storage): a real-data review probe reports
> that the property registry types `Item_EquipmentCategory` as a 64-bit bitfield and that
> items store a bit mask (bit *k* − 1 for enum key *k*, `property-system.md` §7.3; for
> example `2HSword` items store 0x4, `Bow` items 0x80, `Shield` items 0x10000).
> `tessera-formats` now reads `Item_EquipmentCategory` as the full 64-bit mask, and
> `tessera-mesh` matches the §5.4 stowing rule against those bits, including `MediumShield`
> and `Orb`, which fall on bits above 31 (H21, a medium shield, now stows at
> `Sheath_Back_Shield` like the other shields). The only open point is the reading itself:
> please confirm the bitfield reading in §5.2.

> **OPEN** (§4.11, material-only list elements under other keys): a list element whose
> chosen block has material groups but no parts, under a key other than `ClothingColor` /
> `ClothingColorSecondary` (NPC selection tables and the `_Dye` partners of §7.4), is now
> applied like a dye element: its groups go onto the parts that list elements naming the
> same record contributed, and when that record contributed none, onto the default parts
> (the unpaired-dye Decision above, reported as `unpaired-dye`). A review scan of the
> landblock instance lists counts about 10,000 such unpaired elements on static rigs.
> Please confirm that unpaired material-only elements recolour the default parts, or say
> that they are dropped ("they apply only together with the record's parts").

> **OPEN** (§7.4 random appearance versus explicit lists): the implementation follows the
> Decision literally: a figure with a random-appearance type is drawn with its setup's
> default parts and its held props, and its template sections, class list and instance
> list are skipped. A review scan reports that 616 of 43,722 random-appearance figures carry
> lists that would contribute parts (for example entity 0x47006369 in all 82 of its
> placements). Please state whether explicit lists take precedence over the random type.

> **OPEN** (§3.2.1 rule 1 versus `material.md` §12.4, dye code 0.0 in lists): a
> `ClothingColor` list element with modifier 0.0 layers the 0.00 block (rule 1: "dye tables
> (modifier = dye code)"), while a worn item with code 0.0 (default or chosen) layers no dye
> (`material.md` §12.4: "Code 0.0 or 'no dye' means: do not layer a dye material"). Please
> state which reading applies to list elements.

> **Implementation note** (§4.9, §8 step 3, dye pairing): list dye and material-only
> elements pair only with parts contributed by appearance-list elements, never with a worn
> item's parts on the same wardrobe record, so a list dye cannot replace an item's own or
> chosen dye. The holding location (§5.3), the stowing location (§5.4) and the worn-item
> order (§4.7) use the slot an item is **equipped** in when the caller gives one (dual
> wielding), else its `Inventory_DefaultSlot`.

> **OPEN** (implementation, River-hobbit height fix, §6.3 values): "each pair read as a
> base-16 number, so a pair that is not hexadecimal reads 0" leaves two cases open: a pair
> whose first character is a hex digit and whose second is not (`8z`; a number parser that
> stops at the first non-digit would read 8), and a trailing single character of an
> odd-length string. Tessera Decision: a pair reads its value only when both characters are
> hex digits, else 0; a trailing single character is not a pair. Please confirm.

> **OPEN** (implementation, §6.4 step 1): a MeshDeformer time that is not finite (NaN or
> infinite after the driven substitution) is read as fraction 0 by Tessera (Decision); the
> client's handling was not stated. Step 1's "⌊255 × clip weight⌋ / 255" is implemented with
> the same 10⁻⁴-of-a-step tolerance as the layer weights (`skeleton-anim.md` §6.6 OPEN).

> **OPEN** (implementation, confirmation only, §6.4 step 4 and §6.5): the expected heights of §6.5 were
> reproduced to the millimetre by the implementation (top 1.231 / 1.266 / 1.300 m and
> lowest vertex −0.005 / −0.003 / −0.001 m for River-hobbit M at 0, 0.5, 0.95; F 1.200 /
> 1.215 / 1.245 m; DwarfStoutAxe 1.434 m; Dwarf 1.345 m; Human 1.806 m) with the skinning
> transform taken as the matrix product of `skeleton-anim.md` §5.4 OPEN below. Only the
> lowest bone of River-hobbit M at 0.95 differs (0.015 against 0.016, rounding).

---

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-05 | §1 (BIN, ST/SA2, RH, A10, A15–A18), §6.2, §6.3, new §6.4 and §6.5, §8 step 7, §11, §12.3, new §12.4–§12.5, §13, §14 | Height controls, bone-slider values and application, and bind-space deformation from the client (BIN, decision 0011), checked on real data (RH); the earlier Decisions of §6.2 and §6.3 withdrawn; expected standing heights added; implementation OPENs on absent slider strings and rotation order answered (the first in part). | Spec writer (dirty room) |
