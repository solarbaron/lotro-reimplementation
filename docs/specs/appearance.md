<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Appearance assembly: playable bodies, worn and held items, sliders, NPCs

| | |
|---|---|
| **Status** | Revised, awaiting re-review. Revision of 2026-10-09 (review answer): the two blocking items of the review of 2026-10-09 are fixed (§5.7 step 2: the attached-path composition is a Tessera Decision with a narrow OPEN, A35 qualified; §5.7 step 3: OPEN on the holder-factor bits, §6.1 aligned with BIN6, A36), and three non-blocking notes taken (A37 fishing-pole and `…_Cosmetic_Hide` gating, §5.5 `_Behavior` type, A39). Earlier revision of 2026-10-09: the five implementation OPENs of 2026-10-09 in §15 answered from the data (OBS4) and the client (BIN6); §5.3, §5.5, §5.6 corrected and new §5.7. Previously Accepted 2026-10-09 (full review of the spec state after the merge of main into specs-5; includes the owner-observation change of 2026-10-08, §7.4 O12 OPEN and §17 O12 row) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec-reviewer, a different session from the writer), 2026-10-07: first review and re-review; 2026-10-08: review of the BIN4 and BIN5 revisions (another fresh session); 2026-10-08: narrow re-review of §3.3 (a further fresh session) |
| **Review record** | Review 2026-10-09 of the BIN6/OBS4 revision (Claude review session, spec-reviewer, a fresh session that wrote no part of this spec): **changes requested**. Scope: `git diff main -- docs/specs/appearance.md` on branch specs-11 (commit 5484cc8: OBS4, BIN6, A35–A40, §3.2.1, §3.3, §4.4, §4.7, §5.3–§5.7, §6.1, §7.4, §8, §15, §17). Holds: functional facts only; no code (the §5.7 quaternion is a mathematical identity, checked by hand to equal *R*_Z(yaw) · *R*_Y(roll) · *R*_X(pitch), and consistent with `coordinates.md` §11.3); no copied prose; names only for interoperability; no game text, keys or hostnames; new facts carry A35–A40 with OBS4 and BIN6 rows (BIN6 under decision 0011, questions and limits stated); Decisions are labelled; stable section numbers (new §5.7 appended); change-log row present. Spot checks (read-only, own probe on the repository decoders): dictionary types and defaults of `Script_AdjustableScale_Secondary_Vector` and `Inventory_Cosmetic_Slot<n>_Scale` (vector3, (1, 1, 1)), `…_OffsetRotationRoll` (float 0), `…_Hide` (bool false), `Inventory_Cosmetic_IsSheathed` labels 0–5; script 0x100007C3 of table 0x07000090 (death and `MonsterPlayer` branches, the sheathed condition, the four fixed `Sheath_Back_Shield` placements, location 0 empty, sheathed nodes drive only `Entity`, `Scale`, `Behavior`, the default and unsheathed nodes drive all seven fields including roll); script 0x10000684 (gauntlet effect 0x4700A619 at `Wpn_RHand` yaw −90 and `Wpn_LHand` yaw +90 in the stowed script; 54 cosmetic in-hand DIDs; the four-DID fishing-pole case at `Wpn_RHand` under `Script_Item_Primary_Cosmetic_Hide`); script 0x10000593 (the same 54 DIDs at `Wpn_LOffHand`; the shield `Wpn_Forearm` nodes only under a non-null `Script_Item_PhysObj_Secondary_Cosmetic`; nothing while ranged and mounted); script 0x10000527 (GameCallback nodes naming 0x70000469). All match. **Blocking:** (1) §5.7 step 2 and A35: the composition for attached nodes (`AttachToParent` 1, every placement of §5.5 and §5.6) was not read, yet the text states it as the rule with only "(Tessera uses the same order)"; mark it as a Tessera Decision and add a narrow `> **OPEN:**` for the attached path, and qualify A35 accordingly (`CLEAN_ROOM.md` §5.3, unknowns marked OPEN, not guessed). (2) §5.7 step 3: "whether these are the two holder-factor bits of §6.1 was not checked" is an unknown without an OPEN callout; add one, and bring the §6.1 "What was not traced" paragraph (which still says no link between a MeshFX field and either bit was seen) in line with BIN6 (the node copies `UseParticleScale` and `IgnoreChildScale` into two flag bits of the spawned object), with a cross-reference. Note for this item: the sheathed nodes of script 0x100007C3 and the starting-idle sheath nodes of 0x10000684 store `UseParticleScale` 1, unlike the §5.6 rows, so the answer matters for those props. Non-blocking: let A37 (or a new row) also cover the fishing-pole case and the `…_Cosmetic_Hide` gating of §5.6; the §5.5 table gives `_Behavior` as "script key", the dictionary types it as an enum over the script-key mapper (say so); the Human M lute row of §5.6 and the 0x10002BA0 node counts were not spot-checked. Previous record: Full review 2026-10-09 (Claude review session, spec-reviewer, a fresh session that wrote no part of this spec and did none of the earlier reviews): **passed**, Accepted. Scope: the whole spec, with `git diff f9eb8db` (BIN5, OBS3, the O12 change, the §14 and §15 answers, the merge header). Every `CLEAN_ROOM.md` §5.3 item holds. Every earlier blocking item is resolved: the three of the first review of 2026-10-07 and the one of the re-review of 2026-10-07, repeated on 2026-10-08 (§3.3 toggle counts and the OPEN's conclusion; fixed by OBS3 and A34, passed by the narrow re-review of 2026-10-08). Spot checks (read-only): A34 over all 40,643 0x47 entities with an own probe on the repository decoders (11 name the property, 7 of them carry it; 19 carry it, all true and at top level; no listed 0x78 set carries or names it), matches; the O12 fact (`script-runtime.md` R40) in a private copy of the client analysis, deleted afterwards (the Random pick draws from one global combined generator with a shuffle table, seeded at start-up from the clock and by one other routine; nothing in the pick path seeds per object), matches. Non-blocking notes: name the 0x78 counts of §3.3 in A34; give the O12 fact its own provenance row; refresh the §13 O12 line; answer the last §15 confirmation OPEN. Earlier: first review and re-review 2026-10-07, changes requested; review 2026-10-08 of BIN4 and BIN5, changes requested; narrow re-review 2026-10-08 of §3.3, passed. Full records in the review log (decision 0018). |
| **Implements into** | `crates/formats` (appearance-list structs, bone-slider definition structs; the 0x20 decoder already exists), `crates/anim` (bone-slider application, attach placement), `crates/assets` or a new assembly module (slot table, priority resolution, material and dye selection), `apps/explorer` (character viewer: species, sex, worn items, dyes, sliders), `apps/client` and the sandbox backend (animated avatar, static NPCs) |
| **Companion specs** | [`setup.md`](setup.md) (normative for the 0x47, 0x1F, 0x04 and 0x20 layouts, the item paths §3, the slot field §8.3 and §12.2, held items §13). [`skeleton-anim.md`](skeleton-anim.md) (skeletons, the race-table walk §2.2, attach points §8, the multi-part rule §9). [`material.md`](material.md) §9.3 (material groups), §12 (dyes). [`entity-descriptor.md`](entity-descriptor.md) (SEDs, instance IDs). [`landblock.md`](landblock.md) §4.8 (the landblock weenie list). [`property-system.md`](property-system.md) §5–§8 (property sets, enum mappers, DID maps). `script-table.md` (which clip plays; idle animations of NPCs; §5 node layouts of the Appearance, MeshFX, EditMaterial and compositor nodes used by §3.5, §5.6 and §7.4; §8 how a figure finds its tables). [`misc-records.md`](misc-records.md) §3 (face compositors and their composition). |

---

<a id="s0"></a>
## 0. Purpose

`setup.md` says how to decode the records that make a character and how an item reaches its
meshes. This spec says how the pieces are **put together** into one dressed figure:

- which playable species and sexes exist, how each reaches its body, and the list that
  M4 exit criterion 2 must cover (§2);
- the default body, the head, hair, beard and ear styles, the character-creation
  controls, face compositing and colour tints (§3);
- worn items: equipment slots, wardrobe records per body, the **part priority** that decides
  which item shows in each body slot, layering cases, dyes (§4);
- held items: slot and category statistics, attach location, stowed and wielded
  placements (§5);
- whole-body scale, the height controls and the facial bone sliders (§6);
- NPCs and creatures: what the 775 weenie objects of the landblocks really are, where
  NPC-like figures come from, and how to dress them (§7);
- the whole algorithm in words, invariants, statistics, failure categories, corrections
  and synthetic examples (§8–§13), and what the client settles of the owner observations
  (§17).

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
| OBS2 | Observed 2026-10-07 (same Steam install, data current that day), read-only, with throwaway probes on the repository's own decoders: **all 7,524** decodable script tables (0x07; 2 of 7,526 do not decode with the current decoder and were skipped) and **all** scriptlets (0x0C), every node walked (632,847 nodes), with the enclosing Switch, When and port context of each Appearance (58), MeshFX (41), EditMaterial (60), ModifyCompositorColor (33), ModifyCompositorLayer (44), TextureCompositor (53), Run (5) and SetProperty (40) node and its driven fields; the include graph of all tables; all 27,546 property definitions; the enum mappers `Item_EquipmentCategory`, `HoldingLocation`, `Combat_SkillMode`, `TextureCompositorNames`, `ImageCompositorColorNames`, `MeshTypeID`, `MaterialTypeID`, the script-key mapper 0x23000097 and the DID map 0x28000007; the inventory-control object 0x70000230; all 0x47 property sets for scale and location properties; all 383,343 game-object property sets for location properties; all 49,636 landblock-info records for `Appearance_RandomAppearanceType`; all 12,199 appearance-list elements of the 0x47 records against all 4,201 0x20 records. |
| BIN2 | The client executable of the user's install (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035), read in the dirty room on 2026-10-07 with Ghidra (headless, read-only copy of the shared project, deleted afterwards), method M4 under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md). **Code read:** the routine that builds a figure's appearance from `Appearance_ClassList` and `Appearance_InstanceList`; the toggle-file test; the visual-template object (0x1F) merge, flag computation and parent flattening; the appearance-record (0x20) block choice; the per-record key table and the record order of a figure's appearance; the block application (material groups, parts); the Appearance script node's apply routine; the render component's scale computation and its property-change handler; the registration of the two avatar graphics options and their character-creation and options-panel bindings; the client's object property lookup. Searches by value and by name for the PIDs of the worn-item map structs, the random-appearance properties, the cosmetic-inventory properties and the held-item script properties. **Questions:** O8 (does the template scale combine with the entity scale), O10 (equal-priority ties and source order), O11 (the updated-avatar option and its default), O12 (does the client turn a random-appearance type into an outfit), O13 (does client code choose stowed and wielded locations), O15 (does client code pair colours with face layers); and the implementation OPENs of §14–§15 on block choice, dye pairing, scale type and cosmetic-inventory visibility. **Why M1–M3 were not enough:** the data names the properties, records and nodes but not the rules that combine them (tie order, block choice for a modifier that matches no block, whether a property is read by code at all); no published source covers them; an owner observation (M3) shows one outcome but cannot give a rule. **Not read:** anything that handles network traffic, login, keys, the launcher or anti-tamper. |
| BIN3 | Same executable and build (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035), read again in the dirty room on 2026-10-07 after the review, with Ghidra (headless, read-only copy of the shared project, deleted afterwards), method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). **Code read:** the render component's scale computation again (threshold comparison, the holder factor and its two flag bits, the holder's final scale); the property lookup call that reads `Model_ParticleScale`; the block-applying routine of a 0x20 record; the 0x20 record reader (the fields after the DID, the `section` byte and the entry table). **Questions:** review items 1, 5 and 9 (is the 0.0002 limit inclusive; which holder factors enter the render scale; what the list applied before a record's own entry is and where it is stored), and the `section` OPEN of §4.2. **Why M1–M3 were not enough:** the u32 after the DID is 0 and the `section` byte has no visible effect in every record, so the data cannot show their use; the limit and the holder factors only change the result for values no record carries (components near 0.0002, attached figures with flags whose source is not in the data); no published source covers them; observing the client (M3) cannot separate them. **Not read:** anything that handles network traffic, login, keys, the launcher or anti-tamper. |
| BIN4 | Same executable and build (file version 4906.0071.1794.4035), read again in the dirty room on 2026-10-07 with Ghidra (headless, read-only fresh copy of the shared project), method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), to settle edge cases that a clean-room audit of the River-hobbit height code found unstated. **Questions:** how the slider value string is cut and read when a piece is not a clean hexadecimal pair or the string is short or of odd length (§6.3); what happens to a slider entry for a bone outside the skeleton and to limits that are not finite (§6.3); what a body-shape deformer with a missing clip or a time that is not finite does, how many deformers and slider sets a figure may hold, and how track weights are converted (§6.4). **Why M1–M3 were not enough:** every value string of the install is 70 hexadecimal digits, every bone index is in range and every deformer names a clip, so the data cannot show these rules; no published source covers them; observing the running client (M3) cannot produce such inputs. **Code read:** the slider node's string parser and per-bone entry builder, the animation component's slider-set and deformer registration, the deformation-pose builder (deformer clips and slider application) and the string class's append routine (to read its length field). Nothing of the network, login, launcher or anti-tamper code. |
| BIN5 | Same executable and build (file version 4906.0071.1794.4035), read again in the dirty room on 2026-10-08 with Ghidra (headless, read-only fresh copy of the shared project, deleted afterwards), method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), in the same session as `script-table.md` BIN6. **Questions:** the implementation confirmation points of §14 and §15 (2026-10-07 and 2026-10-08): how the client computes a slider's channel value and what it does with a value that is not finite; whether "the most recently started" slider node is the last one in evaluation order. **Code read:** the slider channel-value routine (instruction level), the slider entry builder and applier; the script start order (`script-table.md` BIN6). **Why M1–M3 were not enough:** no slider of the install has limits or values that overflow, and no closure has two slider nodes, so the data cannot show either rule; observing the client (M3) cannot produce those inputs. **Not read:** anything that handles network traffic, login, keys, the launcher or anti-tamper. |
| OBS3 | Observed 2026-10-08 (same Steam install, data current that day), read-only, with a throwaway probe on the repository's own decoders: **all 40,643** 0x47 entities (none failed to decode), every value of each entity's own property set walked at every depth, and every stand-alone 0x78 property set that an entity lists; the 14 avatar entities found through the race table (`skeleton-anim.md` §2.2). Question: which entities name `Appearance_ToggleProperty` in an appearance list and which carry `GOP_AvatarUpdateVisible` (§3.3, answering the blocking item of the 2026-10-07 re-review). |
| OBS4 | Observed 2026-10-09 (same Steam install, data current that day), read-only, with throwaway probes on the repository's own decoders: **all 10,349** MeshFX nodes (41) of all script tables and scriptlets (1,118 with two or more non-zero stored rotation angles, 89 with a driven rotation); the six placement scripts, the visibility controller 0x10000527 and the cosmetic-inventory script 0x100007C3 of table 0x07000090; the sheathed-instrument script 0x10002BA0 in the 12 avatar binding tables that define it; **all 367** Random nodes (17) with a single Default port; **all 29,134** visual templates (setup sections and parents); **all 47,563** meshes (index buffers and the draw entries of every level); **all 4,201** 0x20 records (74,839 blocks), **all 12,199** list elements of the 0x47 records and **all 421,791** list elements of the landblock instance sets, for finite values; the properties `Inventory_Cosmetic_Slot*` and `NPC_Cosmetic_Inventory_*` over all 0x47 entities, all 0x78 and 0x79 property sets and all landblock instance sets; the dictionary definitions (type, default) of the properties named in §5.5–§5.7; the items, instance sets and lists of the H, N and B34 rows of `docs/checklists/m4-visual.md`. |
| BIN6 | Same executable and build (`x64/lotroclient64.exe`, file version 4906.0071.1794.4035), read in the dirty room on 2026-10-09 with Ghidra (headless, private read-only copy of the shared project, deleted afterwards), method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). **Code read:** the MeshFX node's field registration (to find where each field is held), the routine that places the entity a MeshFX node spawns (angle conversion, the placement frame, the attached and unattached paths, the scale computation), the routine that turns an angle triple into a rotation, the routine that turns a rotation into axes, and the frame-composition routine of the unattached path. **Questions:** the implementation OPENs of 2026-10-09 on the axes and order of `OffsetRotationYaw`, `…Pitch` and `…Roll` and on the placement scale (§15). **Why M1–M3 were not enough:** the data hold the angles and scales but no rule for axes, order or composition; every placement row of §5.6 turns about one axis, so a look at the original (M3) cannot separate the orders; no published source states them. **Not read:** anything that handles network traffic, login, keys, the launcher or anti-tamper. |

| # | Fact | Sources | Mark |
|---|---|---|---|
| A1 | 14 playable (species, sex) combinations reach 11 setups through the race table; `Item_SpeciesOfWearer` codes 117, 120, 125 are `HighElf`, `DwarfStoutAxe`, `RiverHobbit` (§2) | SA §2.2; MV, DS (codes 117–125 "probable"); OBS (`SpeciesType` mapper) | Confirmed. Corrects MV and DS. |
| A2 | Worn items use one wardrobe record per (slot family, wardrobe body); seven wardrobe bodies serve the 14 combinations (§2.3, §4.2) | MV (shared wardrobe per body); OBS (634,655 map structs) | Confirmed |
| A3 | A character setup's part list is the default body, one part per `MeshTypeID` slot, some slots holding three-vertex placeholders (§3.1) | SET §12.2; OBS (11 player setups, 4 NPC rigs) | Confirmed |
| A4 | Head, hair, beard and ears come from appearance lists (`Appearance_ClassList`, `Appearance_InstanceList`) of the avatar entity: key + 0x20 record + modifier; the modifier selects the block (style) (§3.2–§3.3) | MV, DS (chargen chain); OBS (14 avatars) | Confirmed |
| A5 | An appearance part's leading float is a **priority**; per body slot the part with the highest priority is shown; a three-vertex part at a higher priority hides the slot (§4.4) | MV (stub "blanking"); OBS (all worn records; mesh extents) | Confirmed by consistent census and geometry; tie rule A22 |
| A6 | The float before each material group is that group's priority; the float after the last group is 0 in every block; group field `a` is a `MeshTypeID` slot or 2 (`All`) (§4.3) | OBS (84,261 groups, 74,839 blocks) | Confirmed. Corrects the field attribution of SET §8.3 and MAT §9.3. |
| A7 | The 0x20 key table value of an entry equals the number of its blocks whose `variant` is below 0.5 (§4.8) | OBS (49,383 of 49,383 entries) | Confirmed. Resolves SET §8.4. |
| A8 | An item's default dye code is `Item_ClothingColor`; it selects a block of the `ClothingColor` entry of the same wardrobe record (§4.9) | MAT §12; OBS (21,843 of 21,999 items) | Confirmed. Answers MAT §12.4 OPEN. |
| A9 | Whole-body scale of a playable body: template `vec3` and entity `Render_AdjustableScale`; River-hobbit entities carry 0.7 (§6.1) | SET §5.2; OBS (14 avatars) | Confirmed values; composition A27 (partly BIN2), OPEN for the template factor |
| A10 | Bone sliders: definition records name bone indices, transform channels and limits; the character stores one byte per slider (§6.3) | DS (byte string, 0x80 default); OBS (4 definition records, 2 preset records) | Confirmed layout; value mapping and application: A16, A17 |
| A11 | The 775 weenie objects placed by SEDs are housing hooks (744), distant-battle units (29), one door and one monster; none is an NPC (§7.1) | ED §4.1 ("NPC, interactive object"); OBS (775 of 775) | Confirmed. Corrects ED §4.1. |
| A12 | The landblock weenie list holds per-instance property sets keyed by IID: one for every SED IID plus 514,987 for IIDs without a SED (§7.2) | LB §4.8 ("server-side templates"); OBS (3,768,840 entries) | Confirmed. Corrects LB §4.8. |
| A13 | Static NPC-like figures are Physics SEDs whose object resolves to a humanoid rig and whose instance property set adds appearance lists, a random-appearance type, an idle animation and held items (§7.2–§7.4) | OBS | Confirmed |
| A14 | No **property set** maps `Appearance_RandomAppearanceType` to an outfit (§7.4) | OBS (property sets of all prefixes holding property sets; raw search of the PID) | Confirmed for property sets; superseded by A26 (the mapping is in script tables) |
| A15 | `AC_Avatar_Height` is the held time (as a fraction) of the River-hobbit layer-110 `scale_blend` clip; `AC_BoneScale_Test` is the held fraction of every avatar's body-shape deformer clip and of the DwarfStoutAxe layer-110 clip (§6.2) | OBS (the 7 driven nodes, the 14 species tables); BIN (§6.7 of ST) | Confirmed 2026-10-05 |
| A16 | Bone sliders: one value per slider *t* = byte / 255 in definition order, 0.5 for sliders past the end of the string; channel = min + *t* × (max − min), negated per reversed bone; later sliders overwrite earlier ones per bone and channel (§6.3) | BIN; OBS (definition records, node strings) | Confirmed 2026-10-05 (BIN only for the rule). Replaces the §6.3 value Decision. Parse details A29. |
| A17 | Sliders and body-shape deformers are not applied to the animated pose: they deform the skinned meshes once in bind space, using the scale composition of SA2 (§6.4) | BIN; RH (feet within 1 cm of the ground for River-hobbit M and F at every height value) | Confirmed 2026-10-05. Replaces the §6.3 application Decision. |
| A18 | Expected standing heights with all mechanisms applied (§6.5) | RH | Confirmed 2026-10-05 as a computation on real data; not yet compared with the original client by eye |
| A19 | Block choice: the block with the greatest `variant` not above the modifier; none if the modifier is below every variant; a modifier of −1.0 means "no block" (§3.2.1) | BIN2; OBS2 (12,199 elements: 11,774 exact, 265 a lower block, 79 none, 67 key missing, 14 null) | Confirmed (rule BIN only) |
| A20 | Toggle files: the figure reads the toggle property through its own property lookup and uses `Appearance_AprToggleFile` when it reads true; the property is a graphics option, default on, and a change rebuilds the figure (§3.3) | BIN2 | BIN |
| A21 | Face compositing and colour tints are driven by script nodes of each avatar's species table: compositor attach (53), layer choice (44) from `AC_Texture_*`, colour (33) from `AC_Color_*`, and dye-colour edits (60) of hair, beard and skin surfaces from `AC_Color_Hair` and `AC_Color_Skin` (§3.5) | OBS2 (all compositor nodes: 218 of kind 33, 199 of kind 44, 39 of kind 53; all 297 EditMaterial nodes driven by `AC_Color_Hair` or `AC_Color_Skin`) | Confirmed |
| A22 | Equal part priority: the part applied later replaces the earlier one; records are applied in the order of their first appearance in the figure's appearance, keys of one record in the order of the client's key table (§4.7) | BIN2 | BIN |
| A23 | Material groups of all applied blocks go into one figure-wide list ordered by ascending group priority; a group keeps no link to the record or block it came from (§4.10) | BIN2 | BIN |
| A24 | Stowed and wielded placements of weapons and shields are data: MeshFX nodes of the shared avatar script table attach the equipped item's entity at a location chosen by equipment category, species and skill mode (§5.6) | OBS2 (all MeshFX nodes of the six placement scripts; all avatar closures) | Confirmed |
| A25 | No client routine references the worn-item map structs, the random-appearance properties, the cosmetic-inventory visibility or `Script_Item_PhysObj_Primary` / `…_Secondary`; they are read by data (script nodes) or supplied with the object (§4.7, §5.5, §5.6, §7.4) | BIN2 (searches by value and by name) | BIN |
| A26 | Random-appearance types select outfits through Appearance nodes under Switches on `Appearance_RandomAppearanceType` in figure script tables, with a random choice per piece group (§7.4) | OBS2 (21,780 nodes under explicit cases in key 5, 830 (table, case) pairs in 61 tables; 21,841 in all scripts and ports, 62 tables; 433 case values; 316 of the 318 non-zero types placed by landblocks covered; counting rule in §7.4) | Confirmed |
| A27 | Render scale of an object, per axis: `Render_AdjustableScale_Checkpointed` or else `Render_AdjustableScale`, × one uniform holder factor, × `Model_Scale`, × a vector of a related object, × the holder's final render scale when attached; a result with a non-finite component or a component **strictly below** 0.0002 becomes (1, 1, 1). Template `vec3` values multiply along the template parent chain (§6.1) | BIN2, BIN3; OBS2 (types and census) | BIN (rule); Confirmed (types) |
| A28 | The u32 after the DID of a 0x20 record is the count of a list of further 0x20 DIDs that follow it; the client applies those records, recursively and with the same key and modifier, before the record's own entry. The count is 0 in all 4,201 records. The `section` byte is the size class of the client's in-memory entry lookup (an index 0–22), with no appearance meaning (§4.2, §4.7) | BIN3; OBS (4,201 of 4,201 records, zero trailing bytes) | BIN (meaning); Confirmed (all counts 0) |
| A29 | Slider value string: cut into two-character pieces from the start; an odd last character is a piece of its own; after an even number of characters one more, empty, piece follows; each piece is read like C `strtol` in base 16, the result taken as an unsigned 32-bit number and divided by 255; sliders beyond the pieces get 0.5 (§6.3) | BIN4 | BIN (rule); Confirmed (strings of the install, OBS) |
| A30 | Slider entries are applied by walking the skeleton's bones 0 to bone count − 1, so an entry for a bone outside that range is never used and the slider's other bones still apply; a figure holds one slider set, the most recently started slider node's (§6.3, §6.4) | BIN4 | BIN |
| A31 | Body-shape deformers: a clip that cannot be loaded or whose track count differs from the bone count contributes nothing; any number of deformers; time = duration × fraction limited to [0, duration]; track weights 255 × weight converted to an integer by truncation, no tolerance (§6.4) | BIN4 | BIN; NaN fraction and non-finite slider limits: Tessera Decisions |
| A32 | A slider's channel value is computed per component in single precision as (`BoneSlider_LimitMax` − `BoneSlider_LimitMin`) × *t* + `BoneSlider_LimitMin`, each operation rounded on its own (no fused multiply-add), then multiplied by the sign; nothing checks the result, so a value that is not finite enters the bone entry (§6.3) | BIN5 | BIN; the Tessera handling of such sliders is a Decision |
| A33 | "The most recently started" slider node is the later one in the client's start order of `script-table.md` §8.2 step 3 and §7.5 (kind-1 definitions before kind-2, roots and port children in stored order, Collection ports by ascending ordinal) (§6.4) | BIN5 (`script-table.md` BIN6, T41, T42) | BIN; no closure has two slider nodes (`script-table.md` SI12) |
| A34 | `Appearance_ToggleProperty` is named (always as `GOP_AvatarUpdateVisible`) by 7 avatar entities and 4 non-avatar entities; `GOP_AvatarUpdateVisible` is carried, always true and always at the top level of the entity's own set, by 19 entities: the same 7 avatar entities, 3 avatar entities that do not name it (DwarfStoutAxe, RiverHobbit M and F) and 9 non-avatar entities; the 4 non-avatar naming entities do not carry it; no 0x78 set carries or names it (§3.3). Corrects the 2026-10-07 text ("10 entities" naming it) | OBS3 | Confirmed 2026-10-08 (all 40,643 entities) |
| A35 | A MeshFX node places its entity in the frame of its holding location (or bone): first a translation by `BoneOffset` (metres, location axes), then a rotation built from `OffsetRotationYaw`, `…Pitch`, `…Roll` (degrees): yaw about the location's +Z, pitch about +X, roll about +Y, composed as *R*_Z(yaw) · *R*_Y(roll) · *R*_X(pitch), right-handed (§5.7). The axes, the order and the angle units hold for every node; the composition location frame × placement frame was read only on the path for unattached nodes (`AttachToParent` 0). For attached nodes (`AttachToParent` 1, every placement of §5.5 and §5.6) the same composition is a Tessera **Decision** with an OPEN (§5.7 step 2) | BIN6; OBS4 (1,118 nodes with two or more angles) | BIN (axes, order, units; composition on the unattached path); Decision (composition on the attached path, OPEN) |
| A36 | The MeshFX `Scale` is a vector applied per axis: the spawned entity's `Render_AdjustableScale` becomes its own value (default (1, 1, 1)) × `Scale`, divided per axis by the holder's `Render_AdjustableScale` when `UseParentScale` is 0; a driven `Scale` whose source property is absent takes the dictionary default; `Script_AdjustableScale_Secondary_Vector` is a vector3 with default (1, 1, 1) (§5.7). The node copies `UseParticleScale` and `IgnoreChildScale` into two flag bits of the spawned object (values 0x4000 and 0x40000000); whether these are the two holder-factor bits of §6.1 was not read (OPEN, §5.7 step 3) | BIN6; OBS4 | BIN (rule, flag copy); Confirmed (types, defaults); link to §6.1 OPEN |
| A37 | Placement-script details: the stowed shield node of the non-ranged branch runs only while a cosmetic override is set; the gauntlet effect entity (stored in the node) is attached in the stowed family only (its wielded collection has `Probability` 0); the cosmetic in-hand rows test the override against entity DIDs stored in the Switch cases; the primary and secondary stowed scripts run only while `Script_Item_Primary_Cosmetic_Hide` (or `…_Secondary_…`) is false, and while the primary flag is true the primary script attaches the item at `Wpn_RHand` only when `Script_Item_PhysObj_Primary_Cosmetic` is one of the four entity DIDs of its fishing-pole case (§5.6) | OBS4 | Confirmed (corrects §5.6 of 2026-10-07) |
| A38 | A ranged-slot instrument is drawn while stowed: script 0x10002BA0 of the avatar's binding table attaches it at a location chosen by `Music_Cached_Instrument_Type`, while the playing script 0x10000324 is not running (§5.3, §5.6) | OBS4 (12 binding tables) | Confirmed (corrects the §5.3 Decision for instruments) |
| A39 | Creature cosmetic slots `Inventory_Cosmetic_Slot1` … `4` (eight properties each) occur only in 0x79 property sets (3,523 sets), never in 0x47 entities, 0x78 sets or landblock instance sets; script 0x100007C3 attaches them; the property dictionary types `…_Behavior` of all four slots as an enum over the script-key mapper 0x23000097, default 0 (§5.5) | OBS4; property dictionary of the install (all four slots) | Confirmed |
| A40 | No 0x20 `variant` and no list modifier is non-finite; no visual template that names a setup has a parent; every index buffer of every mesh is drawn exactly once (258 one-triangle meshes by either count); no single-port Random node has weight 0 (§3.2.1, §4.4, §6.1, §7.4) | OBS4 | Confirmed 2026-10-09 (all records) |

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
#### 3.2.1 Choosing the block (Confirmed rule, BIN2, A19)

Within the entry whose `key` equals `Appearance_Key` (the blocks of every entry are stored in
ascending `variant` order, `setup.md` §8.3):

1. A modifier of exactly −1.0 selects **no block**: the element contributes nothing (the
   client uses −1.0 as its "removed" marker).
2. Otherwise take the block with the **greatest `variant` that is not above the modifier**
   (compared as single-precision floats, no tolerance). Equivalently: walk the blocks in
   stored order and stop before the first block whose `variant` is greater than the
   modifier; the last block passed is the one.
3. If the modifier is below the first block's `variant`, the element contributes nothing.

There is no separate dye rule and no fallback to the `variant` 1.0 block or to the first
block: `ClothingColor` elements, style lists and worn pieces all use rules 1–3. Examples: a
style list 0.00 … 0.37 with modifier 1.0 gives the 0.37 block; a dye table 0.00 … 0.45 with
code 0.49 gives the 0.45 block; an entry whose only block is 1.0 with modifier 0.0 gives
nothing; a dye element with modifier 0.0 gives the 0.00 block (it is layered like any other
dye block).

**Values that are not numbers (Tessera Decision, 2026-10-09).** A NaN modifier selects no
block (it is above no `variant`), and a block whose `variant` is NaN is passed over by the
walk of rule 2 (it is never "greater than the modifier"). The client's handling was not
traced; no `variant` of any 0x20 record and no list modifier of the install is NaN or
infinite (A40), so no figure depends on it.

Census over all appearance lists of all 0x47 records (12,199 elements, OBS2): 11,774 match a
block's `variant` exactly, 265 take a lower block by rule 2, 79 take nothing (13 of them
have modifier exactly −1.0, rule 1; 66 have a modifier below the first block's `variant`,
rule 3, for example modifier 0.0 on an entry whose only block is 1.0), 67 name a key the
record lacks, 14 name the null DID. No block of any 0x20 record has a negative `variant`,
so the 13 elements at −1.0 would take nothing by rule 3 as well. Compared with the rules this section stated before 2026-10-07 (match
within 0.0005, else the 1.0 block or the first block; the old dye rule not modelled), 153
elements get a different result.

When the same record is named twice with the same key (for example by a template section and
by a list), the later modifier replaces the earlier one; when it is named with different
keys, each key selects its own block (§4.7 gives the order).

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
`GOP_AvatarUpdateVisible`, a boolean graphics option (below). Which entities name it and
which carry a value for it are two different sets (OBS3, all 40,643 0x47 entities, A34):

| Entities | Name it in a list | Carry it in their own property set | Count |
|---|---|---|---|
| Avatar entities Hobbit M and F, Human M and F, Elf M and F, Dwarf M (0x47000606, 0x47000607, 0x4700060A, 0x4700060B, 0x4700060E, 0x4700060F, 0x47000611) | yes | yes, true | 7 |
| Avatar entities DwarfStoutAxe (0x4700979A) and RiverHobbit M and F (0x4700CEA3, 0x4700CEA4) | no | yes, true | 3 |
| Avatar entities Beorning M and F, HighElf M and F | no | no | 4 |
| Non-avatar entities 0x4700A4A4, 0x4700A4A8, 0x4700A4AA, 0x4700A4AF (class lists, every naming element with a non-null toggle file) | yes | **no** | 4 |
| Other non-avatar entities | no | yes, true | 9 |

So 7 avatar entities name the property, 10 avatar entities carry it, and 19 entities carry
it in all, every value true. No entity carries it nested inside another value, and none of
the stand-alone 0x78 property sets that 20,274 entities list carries or names it (the four
non-avatar naming entities list one such set each, the 7 naming avatar entities two each).
Every naming element names this property and no other. The toggle file holds the same styles as the plain file plus later ones (Human M hair: 57 meshes in
common, 4 only in the toggle file, 1 only in the plain file; heads and beards identical
sets).

**Toggle rule (BIN2, A20).** A list element is ignored when its `Appearance_AprFile` is null
or its `Appearance_Key` is 0; a missing `Appearance_Modifier` counts as 0.0. The toggle file
replaces `Appearance_AprFile` when the element names both a toggle property and a non-null
`Appearance_AprToggleFile` and that property, looked up **through the figure's own property
lookup**, is a boolean true. The figure watches every toggle property its elements name and
rebuilds its appearance when one changes.

`GOP_AvatarUpdateVisible` is a **graphics option**: the client registers it at start-up with
the default **on**, the options panel binds a check box to it, and the character-creation
screen shows a check box for it only while the species is Dwarf (0x49) or Hobbit (0x51)
(BIN2). The same property also chooses the face compositor (§3.5): on, the newer one; off,
the older one or, for some head choices, the newer one. So by default the client shows the
toggle files, which agrees with the own value of the 7 avatar entities that name the property
(true; the property's dictionary default is false, and 4 non-avatar entities name it without
carrying it, see the OPEN below).

> **Rule (replaces the 2026-10-05 Decision):** use `Appearance_AprToggleFile` when the toggle
> property reads true for the figure, else `Appearance_AprFile`. Tessera keeps the property
> in its own settings (default true) and lets the figure's lookup find it there before the
> entity's value; changing it rebuilds every figure that met it. Any other toggle property
> (none in this install: every naming element names `GOP_AvatarUpdateVisible`, A34) is read
> through the figure's own sets (instance, game object, entity) and, when absent, as its
> dictionary default, or false when the dictionary has none (`script-table.md` T28;
> confirmed 2026-10-09 for the implementation reading of §15).
>
> **OPEN (narrow):** the client's property lookup chooses where to read a property (the
> object's own set, another component, or a client-wide store) by a per-property attribute
> of its in-memory definition (BIN2). Which stored definition field that attribute comes from
> was not traced; all 248 `GOP_*` properties, and 15 option-description properties, share
> `field_a` = 0x18 (`property-system.md` §2.2; OBS2), which fits an option group. That the
> option store, not the entity, answers for `GOP_AvatarUpdateVisible` is therefore inferred,
> not traced. The property's dictionary default (the optional default of its definition,
> `property-system.md` §2.2) is **false** (OBS2); it answers when the chosen source holds no
> value. The two candidate answers agree by default for the 7 avatar entities that name the
> toggle property: the graphics option registered at start-up is on (BIN2), and each of the
> 7 carries true in its own property set (OBS3, A34). They **disagree for the 4 non-avatar
> entities** 0x4700A4A4, 0x4700A4A8, 0x4700A4AA and 0x4700A4AF, which name the property but
> do not carry it: if the option store answers, they show the toggle files while the option
> is on; if the entity's own set answers, the dictionary default false applies and they
> show the plain files even with the option on. For the 7 avatar entities the result
> differs between the two sources only when the option is turned off. (The 3 avatar
> entities that carry the property without naming it, and the Beorning and High-elf
> entities, which neither name nor carry it, have no toggle files, so the question does not
> arise for them.) The Tessera rule above (settings first, default true) gives the toggle
> files for all 11 naming entities by default.

**Face compositing and colours** are in §3.5.

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

<a id="s3-5"></a>
### 3.5 Face compositing and colour tints (Confirmed data, A21; answers owner item O15)

None of this is client code that pairs properties with layers: the pairing is in the
`Startup` and two named scripts of each avatar's species table (`script-table.md` §8.1 for
how a figure finds its tables), as script nodes whose fields are **driven** by the
character's properties (`script-table.md` §4.2). Counts are over all tables and scriptlets
(OBS2).

**Face texture (composited at run time).** The species tables attach a face compositor with
a TextureCompositor node (53) in one of two scripts: key 0x10002ACD (`Compositor`), under the
compositor name 0x10000001 (the newer face), and key 0x10002ACE (`Compositor_Old`), under
0x1000000A (the older face). Each attaches one 0x33 record (21 face compositors in all) to
mesh slot `Head` (0x10000002), material type 0x1000006C (the face-skin binding key). The
`Startup` script runs one of them: in the tables that define both (0x07002DAD, 0x07002DB0,
0x07002F08, 0x07002F09 directly; 0x0700041F, 0x07000425, 0x07000491, 0x07001D83 and
0x07001D85 through scriptlet 0x0C0091D3),
the newer one runs and the older one is stopped when `GOP_AvatarUpdateVisible` is true, and a
test on `AC_Mesh_Head` picks one when it is false; the tables 0x0700247C, 0x07003012,
0x07003013, 0x0700301A and 0x0700301B define only the newer one. Before either runs, a
scriptlet (0x0C0084DA) checks `GOP_AvatarFaceCompositing`: when it is false neither
compositor is attached and the head keeps the texture of its own surface.
`GOP_AvatarFaceCompositing` is a second graphics option, default **on** (BIN2). The
composition itself (layers, masks, colour multiply, placement) is `misc-records.md` §3.6.

Layer choices (node 44, `ImageNumber` driven) and colours (node 33, `Color` driven), per
compositor name; colour and layer keys are those of `misc-records.md` §3.3:

| Property | Node | Feeds (colour or layer key) |
|---|---|---|
| `AC_Color_Skin` | 33 | face skin 0x1000000B, eye-area skin 0x1000000D, eyebrow-area skin 0x1000000E (newer face only), mouth skin 0x10000003, scar skin 0x10000005, wrinkle skin 0x10000019 |
| `AC_Color_Eyes` | 33 | eye colour 0x1000000A |
| `AC_Color_Hair` | 33 | eyebrow colour 0x10000007, scar hair colour 0x10000018 |
| `AC_Color_Lips` | 33 | lip colour 0x10000017 |
| `AC_Color_WarPaint` | 33 | war-paint colour 0x10000013 |
| `AC_Texture_Mouth`, `_Eyebrows`, `_Eyes`, `_Scar`, `_Scar2`, `_Facepaint`, `_Wrinkles`, `_Complexion`, `_Eyeshadow`, `_Jewelry`, `_Stubble` | 44 | layers 0x10000001, 0x10000003, 0x10000004, 0x10000006, 0x1000000C, 0x10000012, 0x10000017, 0x1000000B, 0x1000001B, 0x1000001A, 0x10000018 |

The Beorning bear face (compositor name 0x10000005, attached to slot `UpperBody` 0x1000000C,
material type 0x10000062) takes colour key 0x10000014 from `AC_Color_Hair` (the fur), key
0x10000015 from `AC_Color_WarPaint` and layer 0x10000014 from `AC_Texture_Scar`.

The stored `Color` of the 33 nodes is white (war paint: one fixed colour) and is replaced by
the driven property at run time; colours are the RGBA property bytes in `property-system.md`
order (B, G, R, A).

**Hair, beard and skin surfaces (dye-colour edits).** 297 EditMaterial nodes (60) have
`ColorValue` driven by `AC_Color_Hair` (255) or `AC_Color_Skin` (42); all set material
parameter 2003 (the **dye colour**, `material.md` §12) with priority 5 (a few 20). Three
further EditMaterial nodes are driven by other properties (`AC_Color_WarPaint`,
`Character_SkinColor` and `Appearance_RandomDyeColorValue`, one each) and are not part of
avatar tinting:

| Property | Mesh slot (`MeshType`) | Material types (binding keys) |
|---|---|---|
| `AC_Color_Hair` | `Crest` 0x1000000E | 0x10000024, 0x1000002F, 0x10000036, 0x10000041, 0x10000047, 0x1000004B, 0x10000051, 0x10000056 (the hair materials) |
| `AC_Color_Hair` | `Beard` 0x10000007 | 0x1000000B, 0x10000010, 0x10000017, 0x1000001D, 0x10000023, 0x1000002E, 0x10000035, 0x10000046 (the beard materials) |
| `AC_Color_Hair` | `All` (2) | a few tables: 0x10000024, 0x1000002F, 0x10000062, 0x10000073, 0x1000000B |
| `AC_Color_Skin` | `All` (2) | 0x1000006C (face skin), 0x10000073 (body skin), 0x1000008F |

So hair and beard are **tinted, not baked**: the chosen colour becomes the dye colour of the
hair and beard surfaces, and the shader mixes it in through the surface's dye mask
(`material.md` §12.3). Skin is tinted twice: in the composited face texture and as the dye
colour of the skin surfaces.

> **OPEN (narrow):** into which texture slot of the face-skin surface the composed image goes
> (the TextureCompositor node names the mesh slot and the material type only), and how an
> EditMaterial node's priority ranks against the surface's own material and dye blocks
> (`material.md` §9.2). Owner frame capture O15 can settle the first.

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

**What the two header fields are (BIN3, A28).** The client's reader of a 0x20 record treats
the fields of `setup.md` §8.1 as follows:

- The u32 after the DID (`reserved` there) is a **count** *m*, and *m* u32 DIDs of further
  0x20 records follow it, before the `section` byte. In this install *m* is 0 in all 4,201
  records, so no record carries such a list and the framing of `setup.md` §8.1 is exact
  (every record decodes with zero trailing bytes). A reader should nevertheless accept
  *m* > 0 by reading the *m* DIDs; how they are applied is §4.7 rule 4.
- The `section` byte is the **size class** of the client's in-memory lookup table for the
  entries: an index from 0 to 22 into the client's list of table sizes. The client rejects
  a record whose byte is above 22. It has no appearance meaning, and its census by family
  above reflects the number of entries. Tessera may ignore it apart from that limit.

> **Resolved** (2026-10-07, BIN3): the meaning of `section` (was OPEN). `setup.md` §8.1
> rows 2 and 3 should be revised by that spec's owner to name the count and the size class.

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
3. Per slot the candidate with the **highest priority** wins; on equal priority the one
   applied later (§4.7). A winning **stub** mesh (three vertices, one triangle: 258 mesh
   records; the smallest real part has 4 vertices) leaves the slot visually empty: the slot
   is "hidden". The client never draws a stub part of a multi-part figure (`setup.md` §8.5,
   answers O4). The client counts a mesh's triangles over the draw entries of its detail
   levels (`mesh.md` §8.10); in this install every index buffer of every mesh is drawn
   exactly once (A40), so the triangles of the index buffers give the same total and the
   same 258 stubs.
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
### 4.7 Ties and source order (Confirmed occurrence; client rule BIN2, A22)

Equal priorities with different meshes occur between families: `Hands` 5 (chest 7 entries
per body vs battle gauntlets 147), `Hands` 1 (gloves vs 6 gauntlet entries), `Ears` 10
(helmet vs cloak; same fitted-ears mesh in practice), `Feet` 1 (boots vs 7 cloak entries),
`Helmet` 9, 10 and 20, `Crest` 10, `LowerBody`, `UpperBody`, `Shoulder`, `Cape` 10 (rare
cross-slot entries of the shoulder family), `Satchel` 4 (identical meshes). In NPC
appearance lists (§7.4) ties at priority 1 are common: 1,176 slot conflicts in the static
NPC lists, 311 of them between a stub and a real part (the real part comes later in the
list in 265).

**The client's rule (BIN2).** A figure's appearance is a set of **records**, each with a set
of (key, modifier) pairs, and blocks are applied one after another to a part table:

1. When a block offers a part for a slot that already holds a part, the new part replaces
   the old one when the old priority is **lower than or equal to** the new one. **On equal
   priority the part applied later wins.** A block part for a slot with no part yet is
   added.
2. Records are applied in the order in which each record **first** entered the figure's
   appearance. Sources enter in this order: the template chain's appearance sections (root
   template first, `setup.md` §5.4), then the elements of `Appearance_ClassList` followed by
   those of `Appearance_InstanceList` (one concatenated list, in stored order), then
   Appearance script nodes as they run (§7.4). A record named again later keeps its first
   position; the later (key, modifier) is merged into it (same key: the modifier is
   replaced; new key: added).
3. Within one record the keys are applied in the order of the client's key table: ascending
   (key mod 23), and among keys with the same remainder the one added last first. (The key
   table has 23 buckets, the value of `records.md` §1.3 *B*[1], and never grows. This order
   matters only when two keys of one record offer equal-priority parts for one slot.)
4. **Linked records (BIN3, A28).** When the client applies one (key, modifier) pair of a
   record, it first applies the same key and modifier to each record of that record's list
   of further DIDs (§4.2), in stored order, each in the same way (so their own lists first,
   recursively), and only then chooses and applies the block of the record's own entry for
   the key. A linked DID that does not resolve is skipped (the client logs it). A linked
   record lacking the key contributes nothing. Every list in this install is empty, so this
   rule changes no result today. The client sets no depth or block limit here; an
   implementation's own bounds (for example 16 levels of linked records and 4,096 applied
   blocks per figure) are Tessera limits that no figure of this install reaches.

> **OPEN** (narrow, BIN3): the block-applying routine has no guard against a record that
> lists itself, directly or through others; what the client does with such data was not
> traced further. Tessera stops at a record
> already being applied on the current path and reports a data-quality issue (Decision; no
> record in this install has a list).

**Worn items.** No client routine reads the per-species worn-item map structs (no reference
by value to `Item_WornAppearanceMapStruct`, `Item_SpeciesOfWearer` or `Item_SexOfWearer`, and
no string naming the worn-appearance properties, BIN2, A25): in the original a character's
worn pieces reach the client as appearance elements supplied with the character, so the
order between worn items is the supplier's (the server's), not the client's. Tessera is its
own supplier (M7):

> **Decision (unchanged in effect, now grounded):** Tessera builds a player figure's
> appearance as the client would apply a list: the avatar entity's lists, then the worn items
> in ascending `ContainerSlot` bit order (`Head`, `Chest`, `Legs`, `Gloves`, `Boots`,
> `Shoulder`, `Back`, `Weapon_Primary`, `Class`), each item contributing its wardrobe record
> with key `Item_AppearanceKey` and modifier 1.0 and its dye element (§4.9); then rule 1
> above. This lets battle gauntlets show over a chest's bare hands and an NPC's explicitly
> listed hair show over a piece's stub. Note rule 2: two items sharing one wardrobe record
> (the chest/shoulder cross-overs of §4.2) are applied at that record's first position, and
> their dye elements name the same (record, `ClothingColor`) pair, so the later item's dye
> code replaces the earlier one there (§3.2.1, last paragraph; confirmed 2026-10-09).
>
> **OPEN** (owner observation O10, still wanted): which piece the original shows in the
> equal-priority cases between items (battle gauntlets against a chest at 5, gloves against
> gauntlets at 1), which reflects the order in which the original's server lists the pieces.

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
**of the same wardrobe record** as the item's parts: 21,843 of the 21,999 items with maps
and a dye code find a block with exactly that `variant`; 156 do not (108 have code 1.0, the
rest codes between 0.49 and 0.97 in records whose tables stop earlier). A character's chosen
dye replaces the item's default code.

In the original the item's dye reaches the figure as an appearance element (§4.7), so the
client chooses the dye block by §3.2.1 (BIN2): the block with the largest `variant` not
above the code, no tolerance, nothing below the first block. This is the client rule that
`material.md` §12.4 asks for; for the 156 codes without an exact block it gives the last
block at or below the code (for code 1.0, the table's last block). The dye block's groups
join the figure-wide group list (§4.10) and reach the item's parts through their slot and
binding key (`material.md` §12.3).

> **Decision:** Tessera emits no dye element for an item code of 0.0 (`material.md` §12.4:
> "no dye"); whether the original's server sends one is not visible in the client.

<a id="s4-10"></a>
### 4.10 Material groups (Confirmed slots; ordering BIN2, A23)

In worn records groups target `All` (41,806), a slot of one of the block's own parts (5,759)
or, in material-only dye blocks, a slot with no part (1,485); never a slot that another item
fills.

**The client's handling (BIN2).** When a block is applied (§4.7), each of its groups is
inserted into **one figure-wide group list**, kept in ascending order of group priority; a
group with the same priority as groups already in the list goes after them (application
order). The list element keeps only (priority, shader, material, slot, binding key): it has
no link to the record, block or parts it came from. A group therefore applies to **every
drawn part of the figure** whose slot matches the group's `slot` (`All` matches every part)
and whose surface binding key matches (`material.md` §9.3), whichever record supplied the
part, including the setup's default parts. A dye element (`ClothingColor` key) and any other
material-only element are not paired with "their" record's parts; they join this list like
any other block.

> **Rule for Tessera:** collect the groups of all applied blocks into one list ordered by
> ascending group priority (ties: application order of §4.7) and layer them onto every
> matching part in that order, so that a higher-priority group is applied later
> (`material.md` §9.2 layering).
>
> **OPEN (narrow):** the consumer of the group list was not traced; that it layers in list
> order (higher priority last, so it wins) is the reading that matches `material.md` §9.2.

<a id="s4-11"></a>
### 4.11 Blocks for entries without a base block (Confirmed; answers `setup.md` §10.1 OPEN)

The 3,102 entries with several blocks and no `variant` 1.0 are all **selection tables**:
dye tables (1,871), NPC and style records whose blocks are styles or variants (793 with
parts, 373 material-only), and the head, hair, beard and ear style records (65). The block is
always chosen by a modifier (§3.2.1) or a dye code (§4.9); a viewer without one shows the
first block. Worn item structs never land on such entries. Material-only entries (the
`setup.md` §10.2 OPEN) are the dye tables of the same record or selection tables of NPC
pieces; their groups join the figure-wide group list and apply to every matching part
(§4.10).

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
### 5.3 Attach location (Confirmed sources; in-world rule §5.6)

The paperdoll (UI viewport) rule is in `skeleton-anim.md` §8.5: `Weapon_Primary` →
`Wpn_RHand`, `Weapon_Secondary` → `Wpn_LOffHand`, `Weapon_Ranged` → `Sheath_Back_Bow` (the
inventory-control object's paperdoll list), replaced by the item's
`Inventory_PaperdollParentingLocation`. **In the world** the placement comes from the
placement scripts of §5.6 (stowed or wielded), not from `Inventory_ParentingLocation`, which
no script node reads (OBS2) and no client routine references by value (BIN2). On the player setups the locations resolve to:

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

> **Decision (now supported by the data, §5.6; revised 2026-10-09, A38):** do not draw
> `Class`-slot held items (the class items and the 246 `Class`-slot instruments) and
> `Weapon_Primary_FX` items in the world. The placement scripts of §5.6 cover only the three
> weapon slots (there is no held-item script property for the `Class` slot). An instrument
> equipped in the **ranged** slot is drawn while stowed, by the sheathed-instrument script of
> §5.6, and while it is played by the music scripts; while wielded it is not drawn (the
> earlier text said instruments were never drawn when not played; corrected). **OPEN**
> (owner observation O13, narrowed): whether class items are ever visible on the figure.

<a id="s5-4"></a>
### 5.4 Wielded versus stowed (Confirmed data, §5.6)

Every player setup carries stowing locations (`Sheath_LHip`, `Sheath_RHip`,
`Sheath_Back_2H`, `Sheath_Back_2H_Sword`, `Sheath_Back_Bow`, `Sheath_Back_Crossbow`,
`Sheath_Back_Shield`; `skeleton-anim.md` §8.3). No item property selects one: the choice is
made by script data, by equipment category, species and skill mode (§5.6). The
`Inventory_ParentingLocation` and `Inventory_PaperdollParentingLocation` item properties
take no part in it (no script node reads them, OBS2); §5.3's paperdoll rule stays for the
paperdoll view only.

> **Rule (replaces the 2026-10-05 Decision):** out of combat, place each equipped weapon,
> shield or ranged item at its **stowed** placement of §5.6; in combat at its **wielded**
> placement. The walker (no combat) uses the stowed placements. Differences from the old
> Decision: shields stay on the forearm (`Wpn_Forearm`) when stowed, except while the
> figure's skill mode is ranged (a Tessera Decision for the case without a cosmetic
> override, §5.6 "Shields out of combat"); javelins are stowed at `Sheath_Back_Bow`; staves
> are stowed at `Sheath_Back_2H` with an offset and a half turn; an orb has no stowed
> placement (it is not drawn while stowed); a ranged-slot instrument is stowed by its
> species' sheathed-instrument script (§5.6, revised 2026-10-09).

<a id="s5-5"></a>
### 5.5 NPC held items (Confirmed)

NPC-like figures name held props directly by entity: `NPC_Cosmetic_Inventory_Item1` … `4`
(a 0x47 DID) with `NPC_Cosmetic_Inventory_HoldingLocation1` … `4` (`HoldingLocation`; the
small keys 1 `RHand`, 2 `LHand`, 18 `RHand2` and 0x1000003B `Wpn_RHand` are the common
values) and `NPC_Cosmetic_Inventory_Visible`. In the landblock instance sets: 30,419 sets
have item 1 (28,807 non-null), 15,295 item 2, 201 item 3. The prop resolves and attaches like
any held item (§5.1) at that location of the NPC's own setup.

**Visibility (OBS2, BIN2).** No client routine reads `NPC_Cosmetic_Inventory_Visible` (A25);
script data does. Every MeshFX node that attaches `NPC_Cosmetic_Inventory_Item1` or `…2`
lies under a Switch case `NPC_Cosmetic_Inventory_Visible` = true (or under the sheath
branch of §5.6 for a figure playing its starting idle), and 41 SetProperty nodes set the
flag to false (21) or true (20) during scripted actions. The same holds for the creature
cosmetic slots below, attached by the script key 0x100007C3 of table 0x07000090 while the
flag is true and the slot's `_Hide` is false.

**Creature cosmetic slots (OBS4, A39; answers the implementation OPEN of 2026-10-09).** For
slot *n* = 1 … 4 the properties are (full names, given for interoperability):

| Property | Type | Dictionary default | Role |
|---|---|---|---|
| `Inventory_Cosmetic_Slot<n>_Entity` | DID (0x47 entity) | 0 | The prop; 0 means the slot is empty |
| `Inventory_Cosmetic_Slot<n>_HoldingLocation` | `HoldingLocation` enum | 0 | Where it is held unsheathed |
| `Inventory_Cosmetic_Slot<n>_Scale` | vector3 | (1, 1, 1) | Drives the MeshFX `Scale` (§5.7) |
| `Inventory_Cosmetic_Slot<n>_OffsetRotationYaw`, `…Pitch`, `…Roll` | float, degrees | 0 | Drive the MeshFX rotations (§5.7) |
| `Inventory_Cosmetic_Slot<n>_Behavior` | enum over the script-key mapper 0x23000097 (its value is a script key) | 0 | Drives the MeshFX `Behavior` |
| `Inventory_Cosmetic_Slot<n>_Hide` | bool | false | True: the slot is not drawn |
| `Inventory_Cosmetic_IsSheathed` (one per figure) | enum: 0 `Undef`, 1 `Sheathed`, 2 `Unsheathed`, 3 `Combat_Defined`, 4 `Sheathed_Staff`, 5 `Combat_Defined_Staff` | 0 | Selects the sheathed placements |

**Carriers.** Only 0x79 property sets carry them: 3,523 sets (slot 1 entity and location in
3,515, slot 2 in 1,970, slot 3 in 62, slot 4 in none; scales in 307, 166 and 0; behaviours in
196, 87 and 1; a rotation in at most 18 sets per slot and angle). These are server-side
objects (creature templates and the cosmetic loadouts of `interaction.md` §6). No 0x47
entity, no 0x78 set and no landblock instance set carries any of them, so no placed static
figure has a creature cosmetic slot; they reach a figure only with a server object.

**Placement by script 0x100007C3** (table 0x07000090). Nothing is drawn while `LifeState` is
`Dead` or `Incapacitated`; figures whose `Agent_ClassificationFilter` is `MonsterPlayer`
take another branch (the player-controlled monster's weapons, not covered here). Otherwise,
while `NPC_Cosmetic_Inventory_Visible` is true, each slot whose `_Entity` is non-null and
whose `_Hide` is false is attached by one MeshFX node (`AttachToParent` 1, `UseParentScale`
1), applied by §5.7:

- **Sheathed** (`Inventory_Cosmetic_IsSheathed` 1, or 3 while `Combat_Violent` is false): the
  node is chosen by the slot's `_HoldingLocation`, and the prop goes to `Sheath_Back_Shield`
  with fixed values: from `Wpn_LHand` offset 0 and rotation (yaw −30, pitch 0, roll 90); from
  `Wpn_LOffHand` offset (−0.15, 0.2, 0) m and yaw 210; from `Wpn_RHand` offset (0.15, 0.2, 0)
  m and yaw −210; from `Wpn_Forearm` offset 0 and no rotation; location 0: not drawn. Only
  `Entity`, `Scale` and `Behavior` are driven by the slot; its rotations are not used. Any
  other location takes the unsheathed node below.
- **Unsheathed** (every other value, and 3 while `Combat_Violent` is true): the node's
  `Entity`, `HoldingLocation`, `Scale`, `OffsetRotationYaw`, `…Pitch`, `…Roll` and
  `Behavior` are all driven by the slot; offset 0.

> **Rule:** implement creature cosmetic slots as above when server objects supply them
> (M7 and later). Offline, no figure has one.

> **Rule:** draw a figure's cosmetic props only while `NPC_Cosmetic_Inventory_Visible` is
> true (its dictionary default is true); skip a creature cosmetic slot whose `_Hide` is true.
>
> **OPEN (narrow):** the probe found the attaching MeshFX nodes for props 1 and 2 only in
> spawn scripts (table 0x0700012C) and in the sheath script of §5.6; the node that keeps a
> static figure's tool in its hand during its idle was not identified (it may be bound
> through scriptlet parameters, `script-table.md` §7.8). Until it is, attach every non-null
> prop at its holding location while the flag is true.

<a id="s5-6"></a>
### 5.6 Stowed and wielded placements (Confirmed data, A24; answers owner item O13)

**Where it lives.** Table 0x07000090, which is in the include closure of every avatar class
table (`script-table.md` §8.2), defines six scripts that attach the equipped item's entity
with a MeshFX node (41) whose `Entity` is driven by an avatar property:

| Script key (label) | Entity property | Role |
|---|---|---|
| 0x10000684 (`Inventory_Sheath_Primary`) | `Script_Item_PhysObj_Primary` | stowed, primary weapon slot |
| 0x10000593 (`Inventory_Sheath_Secondary`) | `Script_Item_PhysObj_Secondary` | stowed, secondary slot |
| 0x10000135 (`Inventory_Sheath_Bow`) | `Script_Item_PhysObj_Ranged` | stowed, ranged slot |
| 0x10002982 (`Inventory_Equipped_Primary`) | `Script_Item_PhysObj_Primary` | wielded, primary |
| 0x10002981 (`Inventory_Equipped_Secondary`) | `Script_Item_PhysObj_Secondary` | wielded, secondary |
| 0x10002983 (`Inventory_Equipped_Bow`) | `Script_Item_PhysObj_Ranged` | wielded, ranged |

Inputs the scripts switch on, all avatar properties: `Inventory_PrimaryEquipmentCategory`,
`Inventory_SecondaryEquipmentCategory`, `Inventory_RangedEquipmentCategory` (64-bit
bit fields with the `Item_EquipmentCategory` bits of the equipped item; the inventory-control
object's `InventoryControl_EquipmentCategoryCachedSlotList` names which slot fills which of
the three), `Agent_Species`, `Combat_SkillMode`, `Combat_Violent`, the `…_Cosmetic`
override DIDs and the `…_Cosmetic_Hide` flags. No client routine references
`Script_Item_PhysObj_Primary` or `…_Secondary` by value or by name (A25; four routines
reference `…_Ranged` and were not read): the client does not compute them from the
inventory, so in the original they arrive with the character. Tessera sets each to the 0x47
entity of the item in that slot (its `PhysObj`, or the cosmetic override when one is set),
leaves the `…_Cosmetic` override DIDs null unless the user chooses an override, and sets
`Music_Cached_Instrument_Type` to the ranged item's `Item_Music_Instrument_Type` (Decision,
2026-10-09: how the original sets the cached type was not traced).

Which of the two families runs is decided by the script `Visibility_Controller` (0x10000527)
of the same table and by the combat scripts: out of combat (`Combat_Violent` false) the
stowed scripts run and the wielded ones are stopped; in combat the reverse; mounts, stealth,
a ranged skill mode and cosmetic locomotion styles add exceptions that a full script
evaluator reproduces.

**Stowed placements** (offset in metres in the location's frame, rotations in degrees as
`OffsetRotationYaw`, `…Pitch`, `…Roll`, scale is the node's `Scale`, a vector3 applied per axis by §5.7 (equal components in every stored value); categories are
`Item_EquipmentCategory` labels):

| Slot | Categories | Species | Location | Offset | Yaw, pitch, roll | Scale |
|---|---|---|---|---|---|---|
| Primary | `1HSword`, `1HAxe`, `1HMace`, `1HClub`, `1HHammer`, `Dagger`, `Implement1H`, `Cosmetic` | all | `Sheath_LHip` | 0 | 0 | 1 |
| Primary | same, while `Script_Item_PhysObj_Primary_Cosmetic` equals one of the entity DIDs stored as the values of the Switch case on that property, under the one-handed case of script 0x10000684 (cosmetic in-hand items; 54 distinct DIDs in this install, read from the node) | all | `Wpn_RHand` | 0 | 0 | 1 |
| Primary | `2HSword` | all | `Sheath_Back_2H_Sword` | 0 | 0 | 1 |
| Primary | `2HMace`, `2HAxe`, `2HClub`, `2HHammer`, `Spear`, `Halberd`, `Implement2H` | all | `Sheath_Back_2H` | 0 | 0 | 1 |
| Primary | `2HStaff` | Dwarf, DwarfStoutAxe | `Sheath_Back_2H` | (0, 0.35, 0) | (0, 180, 0) | 1 |
| Primary | `2HStaff` | Hobbit | `Sheath_Back_2H` | (0, 0.2, 0) | (0, 180, 0) | 1 |
| Primary | `2HStaff` | others | `Sheath_Back_2H` | (0, 0.4, 0) | (0, 180, 0) | 1 |
| Primary | `Orb` | all | none (not drawn) | | | |
| Secondary | one-handed list as above | all | `Sheath_RHip` | 0 | 0 | 1 |
| Secondary | same, while `Script_Item_PhysObj_Secondary_Cosmetic` equals one of the entity DIDs of the corresponding Switch case of script 0x10000593 (the same list in this install) | all | `Wpn_LOffHand` | 0 | 0 | 1 |
| Secondary | `Shield`, `HeavyShield`, `MediumShield` | all | `Sheath_Back_Shield` while the skill mode is `Ranged` (nothing while also mounted); otherwise `Wpn_Forearm` **only while `Script_Item_PhysObj_Secondary_Cosmetic` is non-null**; with no override this script attaches nothing (see "Shields out of combat") | 0 | 0 | `Script_AdjustableScale_Secondary_Vector` |
| Ranged | `Bow` | Dwarf, DwarfStoutAxe / Hobbit, RiverHobbit / others | `Sheath_Back_Bow` | 0 | 0 | 0.9 / 0.8 / 1 |
| Ranged | `Crossbow` | all | `Sheath_Back_Crossbow` | 0 | 0 | 1 |
| Ranged | `Javelin` | Dwarf, DwarfStoutAxe | `Sheath_Back_Bow` | 0 | (0, 0, 90) | 1 |
| Ranged | `Javelin` | Hobbit | `Sheath_Back_Bow` | (0.03, 0, 0) | (0, 0, 90) | 1 |
| Ranged | `Javelin` | others | `Sheath_Back_Bow` | 0 | 0 | 1 |
| Ranged | `Instrument` | per species | script 0x10002BA0 of the avatar's binding table, by `Music_Cached_Instrument_Type`; for example Human M with `Lute`: `Sheath_Back_2H_Sword` | (0, −0.1, 0.02) | (180, 20, 0) | 1 |

The primary and secondary stowed scripts run only while `Script_Item_Primary_Cosmetic_Hide`
(or `…_Secondary_…`) is false. While the primary flag is true, the primary script attaches
the item only when `Script_Item_PhysObj_Primary_Cosmetic` is one of the four entity DIDs of
its fishing-pole case, at `Wpn_RHand`.

**Stowed instruments (OBS4, A38).** For an `Instrument` in the ranged slot the stowed ranged
script starts script 0x10002BA0 while script 0x10000324 (the playing script) is not running,
and stops it while that script runs; the wielded ranged script stops it. Script 0x10002BA0
is defined in each avatar's binding table (`script-table.md` §8.2; 12 tables), and attaches
`Script_Item_PhysObj_Ranged` with one MeshFX node per value of `Music_Cached_Instrument_Type`
(some values add a second node with a fixed entity stored in it, for example for the
fiddles, the handknells, the drum and the cowbells); offsets, rotations, scales and
locations differ per species and instrument and are read from the nodes. Nothing is drawn while `Shapeshifting_Target_Entity` is set or the type
is `Undef`.

**Shields out of combat (OBS4).** The visibility controller 0x10000527 chooses between the
two secondary scripts for a shield with a game-logic callback (a GameCallback node,
`script-table.md` §5.2, naming the package 0x70000469). Out of combat and not mounted, a true
result runs the wielded script (shield at `Wpn_Forearm`) and stops the stowed one; a false
result does the reverse, and the stowed script then attaches nothing in a non-ranged skill
mode unless a cosmetic override is set. What the callback returns is game logic and was not
traced.

> **Decision (2026-10-09):** out of combat the walker draws an equipped shield at
> `Wpn_Forearm` (the result of a true callback, and of the stowed script with an override),
> and at `Sheath_Back_Shield` while the skill mode is `Ranged`.
>
> **OPEN** (owner observation, added to O13): whether the original shows an equipped
> shield on the forearm out of combat with the weapons sheathed, or hides it.

**Wielded placements:**

| Slot | Categories | Location | Notes |
|---|---|---|---|
| Primary | one-handed list, `2HSword`, the two-handed list, `2HStaff` | `Wpn_RHand` | |
| Primary | `Orb` | `Wpn_RHand` and `Wpn_LHand` | two MeshFX nodes in the same branch |
| Secondary | one-handed list | `Wpn_LOffHand` | |
| Secondary | shields | `Wpn_Forearm` | all species |
| Ranged | `Bow` | `Wpn_LHand` | scale 0.9 Dwarf and DwarfStoutAxe, 0.8 Hobbit and River-hobbit |
| Ranged | `Crossbow` | `Wpn_RHand` | |
| Ranged | `Javelin` | `Wpn_ROffhand` | |

`BattleGauntlets` (primary): in the **stowed** family the primary script attaches the
effect entity named in the node itself (its stored `Entity`, 0x4700A619 in this install) at
`Wpn_RHand` (yaw −90) and `Wpn_LHand` (yaw +90). In the wielded family the same pair sits in
a Collection whose `Probability` is 0, which never runs (`script-table.md` §4.2, T4); the
wielded branch instead runs script 0x10002F3F (cosmetic weapon-effect pieces) and sets
`Combat_DefaultStrikingImplementType_Melee`. The gauntlets themselves are worn (§5.1).
(Corrected 2026-10-09: the earlier text said "in both families".)

Each node also names a `Behavior` (a script key run on the attached item, for example its
draw or sheathe effect), `AttachToParent` 1, `Persistent` 1 and `UseParentScale` 1 in all
of the placements above. How offset, rotations and scale apply is §5.7.

**What is normative (answers the implementation OPEN of 2026-10-09 on rows not modelled).**
The tables of this section summarise the MeshFX nodes of the scripts named here for the
categories and species of this install; the scripts (found by table and script key, with
each node's fields, driven pairs and enclosing Switch and When cases) are the source. An
implementation that keeps the tables must read the DIDs the scripts store from the nodes at
run time: the cosmetic in-hand lists (the case values of the Switch on
`Script_Item_PhysObj_Primary_Cosmetic` or `…_Secondary_Cosmetic`), the gauntlet effect entity
(the stored `Entity` of the two nodes under the `BattleGauntlets` case of script 0x10000684),
the fishing-pole list, and the fixed entities of the instrument and cosmetic-locomotion
branches. Evaluating the MeshFX nodes of these scripts directly, with the Switch, When,
Collection and Probability rules of `script-table.md` §7, gives the same placements, also
covers the exceptions (mounts, stealth, cosmetic locomotion, skill modes) and is preferred.

> **OPEN (narrow, owner observation O13):** whether a long weapon held in the hand points
> along the line of sight or along the forearm (`skeleton-anim.md` §8.3 note); the
> transitions between the two families (draw and sheathe animations, scripts 0x10000683 and
> 0x100008A5) are not covered here. (The order of yaw, pitch and roll is answered by §5.7.)


<a id="s5-7"></a>
### 5.7 How a MeshFX placement is applied (BIN6, A35, A36; answers the implementation OPENs of 2026-10-09)

This applies to every MeshFX node (41) that attaches an entity at a holding location or bone:
the placements of §5.6, the NPC and creature props of §5.5, and mount and channelling props of
other specs.

1. **Placement frame.** The node defines a frame relative to the holding location's frame
   (`skeleton-anim.md` §8.2; or the bone's frame when the node names a `BoneType` instead of
   a location): its origin is `BoneOffset` (metres, along the location's axes) and its
   rotation is built from the three angles, which are **degrees**:
   - yaw (`OffsetRotationYaw`) turns about the location's **+Z** axis, pitch
     (`OffsetRotationPitch`) about **+X**, roll (`OffsetRotationRoll`) about **+Y**; each
     positive angle turns counter-clockwise when looking from the positive end of its axis
     towards the origin (right-handed);
   - composed as *R* = *R*_Z(yaw) · *R*_Y(roll) · *R*_X(pitch) for column vectors: a point of
     the item is turned first by pitch about X, then by roll about Y, then by yaw about Z, all
     about the location's fixed axes. As a unit quaternion (w, x, y, z) with half angles
     *a* = yaw/2, *b* = pitch/2, *c* = roll/2: w = cos *c* cos *a* cos *b* + sin *c* sin *a*
     sin *b*; x = cos *c* cos *a* sin *b* − sin *c* sin *a* cos *b*; y = sin *c* cos *a* cos *b*
     + cos *c* sin *a* sin *b*; z = cos *c* sin *a* cos *b* − sin *c* cos *a* sin *b*.
   These are the axes and the order of `coordinates.md` §11.3 for object frames.
2. **Composition.** Item to world = the holder's world transform × the location frame on its
   bone (`skeleton-anim.md` §8.4) × translation by `BoneOffset` × *R* × the item's scale
   (step 3). So the offset is measured along the location's axes, and the item turns about
   its own origin placed at the offset.
   - **Unattached nodes** (`AttachToParent` 0): this is the client's rule (BIN6). The client
     multiplies the location's current world frame by the placement frame of step 1 once,
     when it places the spawned entity.
   - **Attached nodes** (`AttachToParent` 1; every placement of §5.5 and §5.6): the client
     hands the same placement frame, the location or bone and the `RemoveRotation` flag to
     the holder's attachment, and the attachment's per-frame composition was not read. For
     these nodes the composition above is a Tessera Decision (below).
   `RemoveRotation` is 0 in every placement of §5.5 and §5.6.

   > **Decision (Tessera, attached nodes):** an attached item is composed every frame as
   > holder world transform × location frame on its bone × translation by `BoneOffset` × *R*
   > × the item's scale, the same order as the unattached path, with `RemoveRotation` 0. A
   > node with `RemoveRotation` 1 is composed the same way until the OPEN below is answered.

   > **OPEN:** (attached path only) the per-frame composition the holder's attachment applies
   > to the frame a MeshFX node hands it: whether it is location frame × placement frame
   > (as on the unattached path) or another order, and what `RemoveRotation` 1 removes (the
   > location's rotation, the holder's, or the placement's). The axes, order and units of
   > *R* (step 1) are not part of this question.
3. **Scale.** When the entity is spawned the client sets its `Render_AdjustableScale`, per
   axis, to the entity's own `Render_AdjustableScale` ((1, 1, 1) when it has none), divided
   per axis by the holder's `Render_AdjustableScale` when `UseParentScale` is 0 and the
   holder has the property, and multiplied by the node's `Scale`. `Scale` is a **vector3**
   used per axis (a uniform value is the special case of equal components). A driven `Scale`
   takes the current value of its source property; an absent source reads as the property's
   dictionary default (`script-table.md` T28): `Script_AdjustableScale_Secondary_Vector` and
   `Inventory_Cosmetic_Slot<n>_Scale` are vector3 properties with the default (1, 1, 1), so
   a figure without them leaves the stored `Scale` replaced by (1, 1, 1). The render scale of
   §6.1 then applies to the item, including the holder's final scale for an attached item.
   The node also copies `UseParticleScale` and `IgnoreChildScale` into two flag bits of the
   spawned object (values 0x4000 and 0x40000000, BIN6). Whether these are the two flag bits
   of the render component that override the holder factor *s* in §6.1 (one forces *s* = 1,
   the other makes *s* the holder's `Model_ParticleScale`) was not read; see the OPEN below.
   Until it is answered, Tessera keeps the §6.1 Decision (*s* = 1 for attached items) and
   ignores both node fields.

   > **OPEN:** (holder-factor bits) are the bits 0x4000 and 0x40000000 that a MeshFX node sets
   > from `UseParticleScale` and `IgnoreChildScale` the same bits that §6.1 says override *s*,
   > and if so, which node field forces *s* = 1 and which selects the holder's
   > `Model_ParticleScale`? The answer changes the drawn size only of items attached by
   > nodes that store `UseParticleScale` 1 or `IgnoreChildScale` 1 on holders whose reported
   > scale or `Model_ParticleScale` is not 1. The review of 2026-10-09 notes such nodes among
   > the sheathed nodes of script 0x100007C3 and the starting-idle sheath nodes of script
   > 0x10000684 (§5.5); a count of both fields over all MeshFX nodes is part of this question.

Example: the Hobbit javelin row of §5.6 (offset (0.03, 0, 0), roll 90) moves the javelin
3 cm along the `Sheath_Back_Bow` location's X axis and turns it a quarter turn about that
location's Y axis; the staff rows (offset (0, *d*, 0), pitch 180) move the staff *d* along
Y and turn it a half turn about X.

---

<a id="s6"></a>
## 6. Scale, height and bone sliders

<a id="s6-1"></a>
### 6.1 Whole-body scale (Confirmed values; composition partly BIN2 and BIN3, A27)

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

**What the client does (BIN2).**

- **Template chain.** When a visual template (0x1F) is resolved, its parent chain is
  flattened root first, and the `vec3`, `value_a` and `value_b` of each template are
  **multiplied**, per component, into the accumulated values (a template without the field
  contributes 1). So a template's effective `vec3` is the product over its chain. The chain
  runs to the root; in this install no template that names a setup has a parent (A40), so
  the root is the template that names the setup and the product over the whole chain equals
  the product up to that template. Template appearance sections are applied root first
  (§4.7 rule 2).
- **Render scale of an object (BIN2, BIN3).** The render component computes a per-axis
  scale in two stages.
  1. **Own scale**, per axis, the product of:
     - *A*: `Render_AdjustableScale_Checkpointed` if the object has it, else
       `Render_AdjustableScale` (default (1, 1, 1));
     - *s*: one **uniform** scalar from the holder. When the object is attached to a holder
       (another object, for example a held item's wearer), *s* is a scale the holder reports
       (how the holder computes it was not traced); when it is not attached, *s* = 1. For an
       attached object two flag bits of the object's render component can override it: one
       forces *s* = 1; the other, which wins if both are set, makes *s* the holder's
       `Model_ParticleScale` (property 0x5A0, a float, read through the holder's property
       lookup; 1 when absent or not a float). Where these flag bits come from was not traced
       (candidate: the two bits a MeshFX node sets, §5.7 step 3 and its OPEN); for an
       unattached object they have no effect;
     - *M*: `Model_Scale` (property 0x3C3, a vector, default (1, 1, 1));
     - *V*: a vector held by a related object of the figure (default (1, 1, 1); see below).
  2. **Final scale** = own scale × the holder's **final** render scale, per axis, when the
     object is attached to a holder; otherwise the own scale.

  The checks run on the final scale only: if any component is not finite, the final scale
  becomes (1, 1, 1); then, if any component is **strictly below** 0.0002 (so zero and
  negative values too, but not 0.0002 itself), the client logs it and uses (1, 1, 1).
  `Physics_AdjustableScale` is not part of the render scale.
- **Types (Confirmed).** `Render_AdjustableScale` is a **vector3** (registry type Vector3;
  answers the §14 OPEN). Its components are used per axis; a non-uniform value scales the
  axes separately. One entity in this install has one: 0x4700DAD2, (0.4, 1, 1). No 0x47 entity
  has `Model_Scale` or `Render_AdjustableScale_Checkpointed`.

What was not traced: what the holder's reported scale *s* is; whether the two flag bits are
the ones a MeshFX node sets when it spawns an attached item (BIN6 shows that the node copies
its `UseParticleScale` and `IgnoreChildScale` fields into two flag bits of the spawned object,
0x4000 and 0x40000000, but not that the render component tests these bits; §5.7 step 3 and its
OPEN; `UseParentScale` acts only on the spawn-time `Render_AdjustableScale`, §5.7 step 3); and whether the vector *V* "held by a related object" is the
template chain's `vec3`. The data suggests it: High-elf templates carry larger `vec3` values than the Elf ones
(1.05 against 1.03 for males) while the entities differ only for females, which a replace
rule would partly cancel.

> **Decision (unchanged apart from the limit):** model scale = template chain `vec3`
> (product, 1 if absent) × entity `Render_AdjustableScale` × instance
> `Render_AdjustableScale` (landblock instance set or SED, §7.2), per axis, applied at the
> object's origin before the world placement; for an attached object (a held item), also
> × the holder's final model scale, with *s* = 1 (Decision: the holder factor *s* and its
> flag bits are not modelled until their sources are known; §5.7 step 3 OPEN). A result with a non-finite
> component, or with a component strictly below 0.0002, is replaced by (1, 1, 1).
> **OPEN** (owner observation O8, narrowed): whether the template `vec3` enters the render
> scale. With it a High-elf female stands about 1.84 m (scale 1.0506), without it about
> 1.82 m (1.02); a High-elf male 1.90 m against 1.81 m.

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

**Values (BIN, A16; parse BIN4, A29).** The value string holds hexadecimal byte pairs. The
client cuts it into **pieces** from the start, two characters at a time. When the string has
an odd number of characters, its last character is a piece of its own. When it has an even
number of characters (none included), one more piece follows the last pair, and that piece
is **empty**. The client gives the *k*-th piece to the *k*-th slider **of the definition
array, hidden ones included**, and stops when every slider has a value or the pieces run
out; every slider left without a value gets *t* = **0.5** exactly.

A piece is read the way the C library function `strtol` reads text in base 16 (named only to
pin the rule down): optional leading white space, an optional `+` or `-`, an optional `0x`
or `0X`, then hexadecimal digits of either case up to the first other character; a piece
with no digit reads 0. So `80` reads 128, `8z` reads 8, `z8` reads 0, a lone `8` reads 8 and
the empty piece reads 0. The result is taken as an unsigned 32-bit number (a negative result
becomes 2³² minus its magnitude), converted to single precision and divided by 255: that is
*t*. Consequences:

- A string of 2*m* characters with fewer pairs than sliders (*m* below the slider count)
  gives slider *m* + 1 the value *t* = 0 (from the empty piece) and the sliders after it 0.5.
  An empty string gives the first slider 0 and every other slider 0.5.
- A piece with a minus sign gives a *t* far outside [0, 1] (for `-1`, (2³² − 1) / 255 in
  single precision). No string of the install has one.

With the 35-pair (70-character) strings of this install, the Human, Elf and HighElf records
(35 sliders) take one pair per slider. The River-hobbit records (37 sliders) give the first
hidden slider (definition 36) *t* = 0 from the empty piece and the second (definition 37)
*t* = 0.5. Neither changes anything, because each has its minimum equal to its maximum. The avatar entities'
strings are 35 × `80` (*t* = 128 / 255 = 0.50196, so for example a 0.7–1.3 scale slider gives
1.00118, not exactly 1). The SliderMeshDeformer node's own stored string is 34 × `80` with
`00` at slider 24 (`Eye_Rotation`).

> **Answered** (2026-10-07, BIN4; was the implementation OPEN on `8z` and a trailing single
> character): `8z` reads 8, not 0 (the client stops at the first non-digit), and a trailing
> single character is a piece of its own (a lone `8` reads 8 for the next slider). Also note
> the empty piece after an even-length string that is shorter than the slider list. The
> implementation's strict pair rule and its dropping of the last character differ from the
> client; follow the paragraph above.

> **OPEN** (driven value of an absent property): the Elf and HighElf avatar entities carry no
> `AC_BoneSlider_Value_Array`. Whether the driven field then keeps the node's stored string
> (eye rotation at its minimum, −0.1) or receives the property's dictionary default was not
> traced. The dictionary default of `AC_BoneSlider_Value_Array` is itself 35 × `80`
> (Confirmed, OBS2, the property definition's default value), so the Decision equals the
> dictionary-default reading. **Decision** (until known): use 35 × `80`.

**Per-bone entries (BIN).** The sliders are turned into one entry per bone, in definition
order, starting from translation (0, 0, 0), angles (0, 0, 0), scale (1, 1, 1) and an empty
mask:

1. For slider *i* with value *t*: channel vector *c* = `BoneSlider_LimitMin` + *t* ×
   (`BoneSlider_LimitMax` − `BoneSlider_LimitMin`), per component. The client works in
   single precision and rounds each step on its own: first the difference, then the
   product with *t*, then the sum with the minimum (no fused multiply-add) (BIN5, A32).
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

**Bones outside the skeleton and limits that are not finite (BIN4, A30).** The client
applies the entries by walking the skeleton's bones from 0 to its bone count − 1 and taking
each bone's entry. An entry for a bone index outside that range, negative ones included, is
therefore never used, and the slider's other bones still get theirs. The client does not
check limits or values for being finite. **Tessera Decision:** a slider with a limit that is
not finite contributes no entry (OPEN: no instance, and the client's result would not be
finite). A slider with no channel bit writes no channel, so it changes nothing.

**Channel values that are not finite (BIN5, A32).** With finite limits and a finite *t*, *c*
can still overflow: *t* reaches about 1.68 × 10⁷ for a piece with a minus sign, and the
difference of two finite limits near the single-precision range can itself overflow. The
client writes such a component into the bone entry unchecked; applied to the deformation
pose (§6.4 step 2), it makes that bone's deformation transform, and through the composition
of step 3 those of the bones below it, not finite, so every vertex they influence is not
finite either. **Tessera Decision** (extends the one above, accepted from the
implementation): a slider whose channel value *c* has a component that is not finite
contributes no entry, and the case is logged. It differs from the client only for data the
client cannot draw. No slider of this install comes near it (the largest limit magnitude is
1.3 and every *t* lies in [0, 1]).

> **Answered** (2026-10-07, BIN4; was the implementation OPEN on bones outside the skeleton
> and limits that are not finite): an index outside the skeleton loses only that bone's
> entry; dropping the whole slider is not the client's rule. Keep the slider's other bones
> (AI7's "dropping the offending element" means that bone index). The handling of limits
> that are not finite is the Decision above. Skipping a slider with no channel bit is
> equivalent to the client.

<a id="s6-4"></a>
### 6.4 Bind-space deformation: body shape and sliders (BIN, A17)

The original applies neither the body-shape deformer nor the bone sliders to the animated
pose. It deforms the skinned meshes **once, in bind space**, and animates the deformed
meshes with the ordinary skeleton. Inputs, all from the avatar's species table
(`script-table.md` §6.9): the MeshDeformer node(s) (clip, time fraction from
`AC_BoneScale_Test`, `UseBoneWeights`) and, on Human, Elf, HighElf and River-hobbit bodies, the SliderMeshDeformer node
(entries of §6.3). A figure holds one slider set: with more than one SliderMeshDeformer the
most recently started one's set is used, and stopping any of them removes the set
(`script-table.md` §6.9; each species table has one). "Most recently started" follows the
client's start order (BIN5, A33): nodes started in the same script run count in the order
of `script-table.md` §8.2 step 3 and §7.5 (kind-1 definitions before kind-2 ones, roots and
the children of one port in stored order, a Collection's ports by ascending ordinal, each
node with its immediate starts before the next); a node started by a later run, a later
Timer or Sequence step or a later Switch branch counts as later.

Algorithm (run at assembly and again whenever a deformer time or the slider string
changes):

1. **Deformation local pose.** With at least one MeshDeformer: sample each deformer clip at
   time = (fraction limited to [0, 1]) × its duration; its track weights are
   ⌊255 × clip weight⌋ / 255 when `UseBoneWeights` is 1 and the clip has a weight array,
   else 1; combine all deformer clips per bone by steps 4–5 of `skeleton-anim.md` §6.6
   (normalised weighted average, reference pose filling a total below 0.1). Without one:
   the skeleton's reference pose. No animation clip takes part, whatever is playing.
   Details (BIN4, A31): the track weight is 255 × weight converted to an integer by
   truncation, with no tolerance added. A deformer whose clip cannot be loaded (the null DID
   included) or whose track count differs from the bone count contributes nothing (the
   second is reported as an error); with no contributing deformer the pose is the reference
   pose. The client keeps any number of deformers. The time is limited to [0, duration]
   after the multiplication, so a fraction of +∞ gives the end and −∞ gives 0. **Tessera
   Decision:** a NaN fraction reads as 0 (OPEN: no instance; the client then samples at an
   undefined time).
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

> **Answered** (2026-10-07, BIN4; was the implementation OPEN on a deformer time that is
> not finite, the 16-clip bound and vertices without a usable influence): the time rule is
> now in step 1. +∞ gives the **end** of the clip in the client, not 0; −∞ gives 0; NaN is
> the Decision of step 1 (0). The client keeps any number of deformers; 16 is an
> implementation limit, not a client rule, and the install has one deformer per figure
> (`script-table.md` SI12), so log any deformer it drops. Leaving a vertex unchanged when
> none of its influences is usable is accepted as a **Tessera rule** for malformed meshes
> (the client's handling of such vertices was not traced). The truncation without tolerance
> and the use of the K26 transform in step 3 are right.

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
### 7.4 Dressing an NPC (Confirmed sources; random type Confirmed, A26)

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
about 43,000 static figures. No client routine reads it, `Appearance_RandomAppearanceColor`,
`Appearance_RandomDyeColorSet` or `Appearance_RandomDyeColorValue` (A25), and no record
outside the script tables is keyed by it. The outfits are in the **script tables** (OBS2):

- Counting rule: an Appearance node (kind 58, `script-table.md` §5.1: a 0x20 record, an
  appearance key, a modifier, `Persistent`) is counted for a case when the **nearest**
  enclosing Switch on `Appearance_RandomAppearanceType` reaches it through an explicit case
  port (not the default port). By that rule 21,780 Appearance nodes lie under such cases in
  the `Startup` script (key 5) of 61 tables, in 830 (table, case) pairs, for 433 distinct
  type values. A further 38 lie under default ports of those Switches in key 5 (21,818), and
  23 more in two other scripts of one table (21,841 in all scripts and ports, 62 tables;
  AI10 counts this total).
  Those tables are reached through the include closures of the NPC rigs' class tables (for
  example rig entity 0x4700005B → table 0x07000004 → 0x07001469, the largest, 6,088 nodes).
- A case is typically a Collection (2) of piece groups (median 3, at most 11), and each group is a
  Random node (17) over Appearance nodes (median 23 alternatives per case, at most 103),
  usually with weight 1 each; so each group contributes **one** randomly chosen piece or
  dye: for example a group of shirt colours (one record and key, modifiers 0.01 … 0.10)
  next to a group of hair styles. 66 nodes have `Probability` 0.25 or about 0.22 (an optional
  extra piece).
- Coverage: of the 318 non-zero types on landblock instance sets (60,211 sets), 316 have a
  case (60,117 sets); types 149 (88 sets) and 148 (6 sets) have none.

**Applying an Appearance node (BIN2).** When the node runs, the client appends one element
(record, key, modifier × the node's run-time intensity) to the figure's appearance, through
the same path as a list element (§4.7, rule 2: after the lists); the block is chosen by
§3.2.1. Every Appearance node in this install has `IntensityMultiplier` 1.0, variance 0 and
`UseParentIntensity` 1 (`script-table.md` §4.2); Tessera uses the stored modifier (factor 1;
the intensity of enclosing nodes was not checked). 18 nodes in other contexts
take their record, key or modifier from driven properties (quest-state appearances).
Appearance nodes of one script run append their elements in the client's start order
(BIN5, A33; `script-table.md` §8.2 step 3 and §7.5): in particular a Collection of piece
groups starts its ports by **ascending ordinal**, usually the reverse of their stored
order, and ports with equal ordinals in an order that was not traced (Tessera Decision:
stored order). The order matters only through §4.7 rules 1 and 2 (a later element wins an
equal-priority tie, a record keeps its first position).

> **Rule (replaces the 2026-10-05 Decision):** dress a figure with its setup's default
> parts, its template sections, its class and instance lists (§7.4 steps 1–4), its held
> props, and then the Appearance nodes its `Startup` scripts run (`script-table.md` §11.3),
> choosing at each Random node by its weights (`script-table.md` §7.4: a node with exactly
> one port always picks it, even when its weight is 0; none of the 367 single-port Random
> nodes of the install has weight 0, A40). Explicit lists and the random type both
> apply; the script pieces come later and win equal-priority ties (§4.7). The 616 figures
> that carry both keep both.
>
> **OPEN** (owner observation O12, narrowed again 2026-10-08): the data makes the pieces random
> each time the script runs. The client draws them from its single global generator, seeded
> once from the wall-clock time at start-up, with no per-object seed in the Random node's
> path (`script-runtime.md` §8, R40, BIN). So a new look per visit is expected (Inferred: a
> game-logic reseed before the figure's start-up script was not ruled out). Tessera seeds the
> Random nodes of a placed figure from its instance ID so that a figure keeps its look
> (Decision). Decision details (2026-10-09): a figure without an instance ID (a player body,
> the explorer's figure) uses seed 0; one generator serves one resolution and is drawn only
> on the branches the picks take, in the start order of `script-table.md` §8.2.

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
3. **Appearance elements**, building the ordered record list of §4.7 rule 2: the template
   chain's sections (root first), the class list then the instance list (§7.4), then for a
   player the worn items in ascending `ContainerSlot` order (§4.7 Decision: per item its
   wardrobe record from the map struct for the wearer's species and sex, §2.3, with key
   `Item_AppearanceKey` and modifier 1.0, and its dye element, §4.9), then the Appearance
   nodes the figure's `Startup` scripts run (§7.4). For each element pick the record (toggle
   rule, §3.3); a record already in the list keeps its position and takes the new key and
   modifier. For a player's head, hair and beard the character's chosen style value replaces
   the modifier.
4. **Apply** the records in list order and, within a record, its keys in key-table order
   (§4.7 rule 3): first apply the same key and modifier to the record's linked records
   (§4.7 rule 4; none in this install), then choose the block by §3.2.1 (none for −1.0 or a
   modifier below every variant); offer each part to its slot, replacing the current part when its priority is
   lower or equal (§4.7 rule 1); insert each material group into the figure-wide group list
   by ascending group priority (§4.10).
5. **Resolve:** the slot table now holds the winners. Record which slots hold stubs.
6. **Materials.** For each winning part: its surface materials, then every group of the
   figure-wide list whose slot is `All` or the part's slot and whose binding key matches, in
   list order (`material.md` §9.3, §12); then the colour edits of §3.5 (hair, beard and skin
   dye colours) and the composited face texture.
7. **Scale, shape and sliders.** Model scale per §6.1. Once per change of the height
   controls or slider string (not per frame), deform the skinned parts in bind space by the
   body-shape deformer and the sliders (§6.4). The species table also starts the held
   proportion clip of River-hobbit and DwarfStoutAxe bodies (`script-table.md` §6.8), which
   the per-frame animation blends like any other layer.
8. **Draw** every non-stub winning part with identity part transform in the setup frame,
   skinned to the skeleton (`skeleton-anim.md` §9.1); stub parts of a multi-part figure are
   never drawn (`setup.md` §8.5).
9. **Held items.** For each equipped weapon, shield or ranged item: its stowed placement out
   of combat, its wielded placement in combat (§5.4, §5.6); for NPC props, their holding
   locations while `NPC_Cosmetic_Inventory_Visible` is true (§5.5). Resolve the entity's
   setup, look the location up in the wearer's list A, apply the placement's offset,
   rotation and scale by §5.7, and place it there each frame (`skeleton-anim.md` §8.4).
   Items whose location the wearer lacks are not drawn. A ranged-slot instrument is stowed
   by its species' sheathed-instrument script (§5.6).

A figure is rebuilt (steps 2–6) only when its equipment, appearance values, a watched toggle
property (§3.3) or its instance set change; the deformation of step 7 when a height control
or the slider string changes; the animation of the figure and steps 8–9 run per frame.

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
| **AI9** | In every 0x20 entry the block `variant` values are strictly ascending in stored order (49,383 of 49,383 entries), so the block choice of §3.2.1 is well defined. |
| **AI10** | Every Appearance script node (58) names an existing 0x20 record or the null DID (3 nodes). Of the 21,841 under Switches on the random-appearance type or colour: 21,216 match a block exactly, 292 take a lower block, 42 take none, 291 name a key the record lacks. |
| **AI11** | Table 0x07000090 is in the include closure of the class table of all 14 player-avatar entities and defines the six placement scripts of §5.6. |
| **AI12** | Block choice over the 12,199 appearance-list elements of the 0x47 records: 11,774 exact, 265 lower block, 79 none (13 by rule 1, 66 by rule 3), 67 key missing, 14 null DID (§3.2.1). |
| **AI13** | In every 0x20 record the linked-record count after the DID is 0 and the `section` byte is at most 22 (4,201 of 4,201; §4.2). |

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
| No block at or below the modifier, or modifier −1.0 | The element contributes nothing (§3.2.1) |
| Part mesh missing or undecodable | Slot keeps its previous winner; reported |
| Holding location missing on the wearer | Held item not drawn (debug: at origin) |
| Equipment category with no placement (orb stowed, class items, instruments) | Not drawn (§5.6) |
| Random-appearance type | Pieces from the `Startup` script's Appearance nodes (§7.4); with no case for the type, the default parts |
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
| Block choice: exact match within 0.0005, then a dye rule, then the 1.0 block or the first block | this spec §3.2.1 (2026-10-05) | The greatest `variant` not above the modifier; nothing below the first block or for −1.0 (§3.2.1, BIN2). |
| Material groups apply to their own block's parts; a dye element applies to the parts its record contributed | this spec §4.10, §15 implementation; `material.md` §9.3 | Groups join one figure-wide list by group priority and apply to every matching part (§4.10, BIN2). `material.md` §9.3's "own block" restriction does not hold for figures. |
| Equal-priority ties: later source wins, worn items after lists (Decision) | this spec §4.7 (2026-10-05) | Client rule confirmed (later applied wins), with the record order of §4.7 rule 2 (BIN2); the worn-item order is the supplier's. |
| Shields stowed on the back; javelins and orbs stay in hand (Decision) | this spec §5.4 (2026-10-05) | Shields stay on the forearm unless the skill mode is ranged; javelins stow at `Sheath_Back_Bow`; orbs are not drawn when stowed (§5.6, data). |
| No client record maps a random-appearance type to an outfit | this spec §7.4, A14 (2026-10-05) | The mapping is in script tables: Appearance nodes under Switches on the type (§7.4, A26). The scan of 2026-10-05 covered property sets only. |
| 0x20 `reserved` u32 is 0; `section` byte meaning OPEN | `setup.md` §8.1 | The u32 counts a list of further 0x20 DIDs applied before the record's own entry; `section` is the size class (0–22) of the client's entry lookup table (§4.2, §4.7 rule 4, BIN3). |
| `Inventory_ParentingLocation` gives the in-world location of a held item | `skeleton-anim.md` §8.5; this spec §5.3 | No script node reads it; in-world placement comes from the placement scripts (§5.6). It keeps no known role (OPEN for `skeleton-anim.md`). |

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

An entry with blocks of `variant` 0.00, 0.01, 0.02, 0.45, 0.80 (single-precision values):
modifier 0.02 → third block; modifier 0.0199995 → **second** block (it is below the stored
0.02, and there is no tolerance); modifier 0.30 → third block; modifier 1.0 → fifth block
(0.80); modifier −0.5 → no block; modifier −1.0 → no block. Its key-table value is 4. An
entry whose only block is 1.0: modifier 1.0 or 2.0 → that block; modifier 0.0 → no block.

Record order (§4.7): template section {record R1, key K1}, class list {R2 K2, R1 K3}, then a
worn item {R3 K4}. Records apply in the order R1 (keys K1 and K3, in key-table order), R2,
R3. If R1's K3 block and R2's K2 block both offer a `Hands` part at priority 5, R2's part
wins (applied later); if R3 also offers one at 5, R3's wins.

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

> **OPEN** (owner observation O8, narrowed 2026-10-07): whether the template chain's `vec3`
> enters the render scale; the multiplication along the template chain and the render-scale
> product are BIN (§6.1).

> **Resolved** (2026-10-05, BIN): the height controls' effect (§6.2), the bone-slider byte
> mapping, combination, rotation axes and frame (§6.3–§6.4). **OPEN** (owner observation
> O9, confirmation only): compare the shortest and tallest River-hobbit with §6.5; the
> driven value of an absent slider string (§6.3; the Decision equals the dictionary default).

> **Resolved** (2026-10-07, BIN2): the client's tie rule and record order (§4.7). **OPEN**
> (owner observation O10, narrowed): the original's order between worn items, which its
> server supplies; confirmation of the layering cases of §4.6.

> **Resolved** (2026-10-07, BIN2): the updated-avatar option (graphics option, default on)
> and the toggle rule (§3.3). Narrow OPEN on the property lookup's source (§3.3).

> **Resolved** (2026-10-07, OBS2): where random-appearance outfits come from (§7.4).
> **OPEN** (owner observation O12, narrowed): same look on every visit or not.

> **Resolved** (2026-10-07, OBS2): stowed and wielded placements of weapons, shields and
> ranged items (§5.6); rotation order and placement composition (2026-10-09, BIN6, §5.7);
> stowed ranged-slot instruments (2026-10-09, OBS4). **OPEN** (owner observation O13,
> narrowed): in-hand orientation of long weapons, class items, an equipped shield out of
> combat (§5.3, §5.6). **OPEN** (narrow, 2026-10-09): the per-frame composition of attached
> MeshFX placements and the meaning of `RemoveRotation` 1 (§5.7 step 2, Decision); whether
> the MeshFX `UseParticleScale` and `IgnoreChildScale` bits are the holder-factor bits of
> §6.1 (§5.7 step 3).

> **Resolved** (2026-10-07, OBS2): which colour and texture properties drive which face
> layers and surfaces (§3.5). **OPEN** (frame capture O15, narrowed): the face-skin texture
> slot that receives the composed image; the rank of colour edits against dye blocks.

> **Resolved** (2026-10-07, BIN3): the `section` byte and the u32 after the DID of 0x20
> records (§4.2, §4.7 rule 4). **OPEN** (narrow): a self-referencing linked-record list
> (§4.7).

> **OPEN:** the consumer of the figure-wide group
> list (§4.10, layering direction); distant-battle units (§7.1); the node that keeps a
> static figure's prop in hand (§5.5).

> **Resolved** (`setup.md` §8.5, BIN): stub parts of multi-part figures are never drawn (O4).

None of these blocks assembling and animating the 14 playable bodies with worn and held
items; they affect fidelity of scale, ties and NPC looks.

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

> **Answered** (2026-10-07, spec writer, OBS2 and BIN2): the type is **Vector3** (registry
> type 15, `property-system.md` §3). The client uses it per axis: a non-uniform value scales
> the three axes separately, and any component that is not finite or is strictly below
> 0.0002 makes the whole render scale (1, 1, 1); exactly 0.0002 is kept (§6.1). One entity has a non-uniform value (0x4700DAD2,
> (0.4, 1, 1)). Accept any vector; stop ignoring non-uniform ones.

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

> **Answered** (2026-10-07, spec writer, OBS2): both lists
> (`InventoryControl_SlotToPaperdollLocation_MappingList` and
> `InventoryControl_SlotToLocation_MappingList`) hold `InventoryControl_SlotToLocation_Mapping`
> structs with two fields: `InventoryControl_SlotToLocation_InventorySlot` (bit field, a
> `ContainerSlot` value) and `InventoryControl_SlotToLocation_HoldingLocationID` (enum, a
> `HoldingLocation` key). The same object's `InventoryControl_EquipmentCategoryCachedSlotList`
> holds `InventoryControl_EquipmentCategoryCachedSlotListEntry` structs
> (`InventoryControl_EquipmentCategoryCachedSlot`, `InventoryControl_EquipmentCategoryCachedProperty`)
> that name the avatar property caching each weapon slot's category (§5.6).

> **OPEN** (§2.3, §10): an item that has wardrobe structs for other bodies but none for the
> wearer (after the fallback) and also a `PhysObj` is treated as **not drawn**, as §10's
> first failure category says, never as a held item. Please confirm that the `PhysObj` of
> a worn item is never shown on a body it does not fit.

> **Answered** (2026-10-07, spec writer, BIN2 and OBS2): confirmed. A worn item reaches a
> figure only as appearance elements (§4.7); the only data path that attaches an item's
> entity to a figure is the placement scripts of §5.6, which read the three weapon-slot
> properties only. A worn item without a wardrobe struct for the wearer is not drawn.

> **OPEN** (§5.5, review 2026-10-05): §5.5 names `NPC_Cosmetic_Inventory_Visible` without
> giving its meaning. A review probe found that of 28,807 landblock instance sets with a non-null
> `NPC_Cosmetic_Inventory_Item1`, the flag is false in 633, true in 12,980 and absent in
> 15,194 (dictionary type Bool, default true). The implementation no longer hides a
> figure's props when it is false: `npc_held_items` returns them all and
> `npc_cosmetic_inventory_visible` reports the flag for the caller to decide. Please state
> whether false hides the props (always, out of combat, or only in some UI views).

> **Answered** (2026-10-07, spec writer, OBS2 and BIN2): false hides the props, always: they
> are attached by script nodes that run only while the flag is true, and scripts switch the
> flag during scripted actions (§5.5 Rule).

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

> **OPEN** (§6.3 "limits that are not finite", implementer, 2026-10-08): the Tessera
> Decision covers only a slider with a limit that is not finite. `BoneSliders::add_slider`
> in `tessera-anim` also gives no entry for a slider whose position *t* or channel value
> *c* = `BoneSlider_LimitMin` + *t* × (`BoneSlider_LimitMax` − `BoneSlider_LimitMin`) is not
> finite. *t* from the value parse is always finite, but *c* can overflow to ±∞ (or NaN)
> when a piece carries a minus sign (*t* about 1.7 × 10⁷) and the limits are very large;
> the client would then write a non-finite entry (§6.3: it checks nothing). No such slider
> exists in this install. Conservative choice until the spec decides: treat it like the
> limits Decision (the slider contributes no entry, logged). Please confirm or state a rule.

> **Answered** (2026-10-08, spec writer, BIN5, A32): confirmed as a Tessera Decision. The
> client computes *c* in single precision as (max − min) × *t* + min, each step rounded on
> its own, and writes the result unchecked, so a component that is not finite makes the
> bone's deformation and those of the bones below it non-finite (§6.3 "Channel values that
> are not finite"). Dropping the slider and logging it is the Decision now stated there.

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

> **Answered** (2026-10-07, spec writer, BIN2): none of these. A dye element's material
> groups join the figure-wide group list and apply to every drawn part whose slot and binding
> key match, whichever record supplied the part, default parts included (§4.10). There is no
> pairing and so no "unpaired" case; drop the `unpaired-dye` report.

> **OPEN** (§7.3, rig rule): the distant-battle marker entity 0x47006915 (29 weenie objects,
> §7.1) resolves to setup 0x040002DE, whose single part is in slot `UpperBody`. The rule of
> §7.3 therefore calls it a humanoid rig, while §7.1 describes it as a marker. Nothing in
> assembly depends on the classification (the part is drawn either way); please confirm the
> rule or refine it (for example, require a skinned part).

> **Answered** (2026-10-08, spec writer): keep the rule as stated. The classification of §7.3
> is this spec's census vocabulary, not a client concept: the client applies appearance
> elements to every figure the same way, by slot (§4.7, BIN2), and nothing else in assembly
> reads the class. The marker counts as a humanoid rig by the rule, which changes nothing.

> **OPEN** (§7.4 step 4, weenie objects): a weenie object (0x70) has its own property set
> besides its entity's. The implementation lets an `Appearance_InstanceList` in the
> object's set replace the entity's, and the instance set's list replace both; NPC props,
> `NPC_StartingIdleAnimation` and `Appearance_RandomAppearanceType` are read with the same
> precedence (instance, object, entity). On the reference install all 775 weenie objects
> have an instance set, but neither it nor the object's set carries an instance list (all
> 775 use the entity's), so the object-set step is not exercised by the data.

> **Answered** (2026-10-08, spec writer): accepted as a **Tessera Decision** (instance, then
> object, then entity), the same precedence as the property-set order Decision of
> `script-table.md` §16. The client's merge of an object's and its entity's sets for these
> properties was not traced, and no weenie object of this install exercises it.

> **OPEN** (§4.4, stubs): a part is a stub when its mesh has at most three vertices in all
> submeshes and levels together. Please confirm that no stub has more than one submesh.

> **Answered** (2026-10-07, spec writer): the client flags a mesh whose detail levels hold
> exactly **one triangle in total** (`setup.md` §8.5, `mesh.md` §8.10); that selects the same
> 258 records as the three-vertex test. A mesh with one triangle in its first level but more
> in others (0x0600C113) is not a stub.

> **OPEN** (§3.1, §4.4, static setups): static setups list several parts with the same
> `flags` (`Default`). The implementation keeps every default part, in order, and an offered
> part for a slot replaces **all** default parts of that slot (drawn once, at the first one's
> position). No observed appearance list offers a `Default` part to a static setup with
> several parts, so this is untested by the data.

> **Answered** (2026-10-08, spec writer): accepted as a **Tessera Decision**. How the client's
> part table holds several default parts of one slot was not traced (§4.7 rule 1 describes
> one part per slot), and no appearance list of this install offers a part to such a
> setup, so no result depends on it.

> **OPEN** (§5.4 Decision, categories it does not name): `Implement1H`, `Implement2H`,
> `Cosmetic`, `Bauble`, `Instrument`, `Class` and `Weapon_FX` items stay at their in-hand
> (wielded) location when stowed. Please extend the Decision if they have stowing places.

> **Answered** (2026-10-07, spec writer, OBS2): §5.6. `Implement1H` and `Cosmetic` stow with
> the one-handed weapons (`Sheath_LHip`, or `Sheath_RHip` in the secondary slot);
> `Implement2H` with the two-handed weapons (`Sheath_Back_2H`); `Orb` has no stowed placement
> (not drawn while stowed); `Bauble`, `Instrument`, `Class` and `Weapon_FX` items have no
> placement script and are not drawn (§5.3 Decision).

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

> **Answered** (2026-10-07, spec writer, OBS2): confirmed. `Item_EquipmentCategory` is
> registry type Bitfield64 with the enum mapper 0x2300004C (key *k* = bit *k* − 1), and the
> avatar properties `Inventory_PrimaryEquipmentCategory`, `…Secondary…`, `…Ranged…` that the
> placement scripts switch on are Bitfield64 too; the scripts' cases compare single bits
> (for example 0x4 `2HSword`, bit 38 `Orb`).

> **OPEN** (§4.11, material-only list elements under other keys): a list element whose
> chosen block has material groups but no parts, under a key other than `ClothingColor` /
> `ClothingColorSecondary` (NPC selection tables and the `_Dye` partners of §7.4), is now
> applied like a dye element: its groups go onto the parts that list elements naming the
> same record contributed, and when that record contributed none, onto the default parts
> (the unpaired-dye Decision above, reported as `unpaired-dye`). A review scan of the
> landblock instance lists counts about 10,000 such unpaired elements on static rigs.
> Please confirm that unpaired material-only elements recolour the default parts, or say
> that they are dropped ("they apply only together with the record's parts").

> **Answered** (2026-10-07, spec writer, BIN2): they are not dropped and not paired: their
> groups join the figure-wide list like every other block's groups (§4.10). "They apply only
> together with the record's parts" in §4.11 is withdrawn.

> **OPEN** (§7.4 random appearance versus explicit lists): the implementation follows the
> Decision literally: a figure with a random-appearance type is drawn with its setup's
> default parts and its held props, and its template sections, class list and instance
> list are skipped. A review scan reports that 616 of 43,722 random-appearance figures carry
> lists that would contribute parts (for example entity 0x47006369 in all 82 of its
> placements). Please state whether explicit lists take precedence over the random type.

> **Answered** (2026-10-07, spec writer, OBS2 and BIN2): both apply. The random type is not
> a replacement for the lists: it selects Appearance nodes in the figure's `Startup` scripts,
> which run after the lists are applied and win equal-priority ties (§7.4 Rule).

> **OPEN** (§3.2.1 rule 1 versus `material.md` §12.4, dye code 0.0 in lists): a
> `ClothingColor` list element with modifier 0.0 layers the 0.00 block (rule 1: "dye tables
> (modifier = dye code)"), while a worn item with code 0.0 (default or chosen) layers no dye
> (`material.md` §12.4: "Code 0.0 or 'no dye' means: do not layer a dye material"). Please
> state which reading applies to list elements.

> **Answered** (2026-10-07, spec writer, BIN2): for list elements (and Appearance nodes) the
> client applies the block chosen by §3.2.1, so modifier 0.0 layers the 0.00 block of the
> dye table. The "code 0.0 means no dye" rule of `material.md` §12.4 concerns an item's own
> dye code, which in the original is turned into an element by the server; Tessera's
> worn-item builder (§4.7 Decision) keeps that rule by not emitting a dye element for code
> 0.0.

> **Implementation note** (§4.9, §8 step 3, dye pairing): list dye and material-only
> elements pair only with parts contributed by appearance-list elements, never with a worn
> item's parts on the same wardrobe record, so a list dye cannot replace an item's own or
> chosen dye. The holding location (§5.3), the stowing location (§5.4) and the worn-item
> order (§4.7) use the slot an item is **equipped** in when the caller gives one (dual
> wielding), else its `Inventory_DefaultSlot`.

> **Superseded** (2026-10-07, spec writer, BIN2): there is no pairing at all (§4.10); every
> group, from lists, worn items or script nodes, applies figure-wide by slot and binding key
> in group-priority order. A list dye therefore can affect a worn item's parts when slot and
> binding key match, as in the original. Using the equipped slot for placement stays right:
> the placement scripts of §5.6 switch on the categories cached per weapon slot.

> **OPEN** (implementation, River-hobbit height fix, §6.3 values): "each pair read as a
> base-16 number, so a pair that is not hexadecimal reads 0" leaves two cases open: a pair
> whose first character is a hex digit and whose second is not (`8z`; a number parser that
> stops at the first non-digit would read 8), and a trailing single character of an
> odd-length string. Tessera Decision: a pair reads its value only when both characters are
> hex digits, else 0; a trailing single character is not a pair. Please confirm.

> **Answered** (2026-10-07, BIN4; pointer added 2026-10-08): not confirmed; the client reads
> `8z` as 8 and a trailing single character as a piece of its own (§6.3 "Values", A29). The
> implementation has since followed the piece rule (the last note of this section).

> **OPEN** (implementation, §6.4 step 1): a MeshDeformer time that is not finite (NaN or
> infinite after the driven substitution) is read as fraction 0 by Tessera (Decision); the
> client's handling was not stated. Step 1's "⌊255 × clip weight⌋ / 255" is implemented with
> the same 10⁻⁴-of-a-step tolerance as the layer weights (`skeleton-anim.md` §6.6 OPEN).

> **Answered** (2026-10-07, BIN4; pointer added 2026-10-08): §6.4 step 1. +∞ gives the end
> of the clip, −∞ gives 0, NaN is the Tessera Decision (0); the track weight is truncated
> without any tolerance (A31, `skeleton-anim.md` §15).

> **OPEN** (implementation, confirmation only, §6.4 step 4 and §6.5): the expected heights of §6.5 were
> reproduced to the millimetre by the implementation (top 1.231 / 1.266 / 1.300 m and
> lowest vertex −0.005 / −0.003 / −0.001 m for River-hobbit M at 0, 0.5, 0.95; F 1.200 /
> 1.215 / 1.245 m; DwarfStoutAxe 1.434 m; Dwarf 1.345 m; Human 1.806 m) with the skinning
> transform taken as the matrix product of `skeleton-anim.md` §5.4 OPEN below. Only the
> lowest bone of River-hobbit M at 0.95 differs (0.015 against 0.016, rounding).

> **Answered** (2026-10-08, spec writer): confirmed. The skinning transform is the client's
> T·R·S of the skinning triple with the inverse reference triple of `skeleton-anim.md` §5.4
> (K26, resolved 2026-10-07); on the player skeletons (identity reference rotations, unit
> reference scales) it equals the matrix product used here, so the reproduction stands. The
> §6.5 values are rounded to the millimetre; compare them with a tolerance of 0.001 m,
> which covers the lowest-bone difference.

> **OPEN** (implementation, 2026-10-07, after the BIN4 answers; engineering choices,
> confirmation only): (1) §6.3 values now follow the piece rule (two characters at a time,
> a trailing one-character piece, the empty piece after an even-length string, `strtol`
> reading, *t* not limited to [0, 1]); a bone index outside the skeleton loses only its own
> entry. (2) Extension of the §6.3 Tessera Decision: a slider whose limits and *t* are finite
> but whose channel value *c* = min + *t* × (max − min) is not (overflow, possible only with
> limits near the single-precision range or a huge *t* from a minus sign) also contributes
> no entry. Please say if the client's result differs in a way that matters. (3) §6.4 step 1:
> Tessera combines at most 16 deformer clips per figure (an implementation limit; the rest
> are dropped and logged); a clip whose track count differs from the bone count is skipped
> and logged as an error, the others still contribute. (4) §6.4, one slider set per figure:
> the resolver evaluates a `Startup` script without time, so "the most recently started"
> SliderMeshDeformer is taken to be the **last one in evaluation order**. Please confirm that
> the client starts the nodes of one script in that order (no instance: SI12).

> **Answered** (2026-10-08, spec writer, BIN5 and `script-table.md` BIN6):
> (1) Confirmed: the piece rule, the `strtol` reading, *t* not limited to [0, 1] and the
> loss of only the out-of-range bone's entry are the client's (A29, A30).
> (2) Confirmed as a Tessera Decision. The client computes *c* = (max − min) × *t* + min in
> single precision, step by step, and writes it unchecked; a component that is not finite
> makes the bone's deformation, and those of the bones below it, non-finite. The difference
> therefore matters only for data the client cannot draw; keep dropping and logging such a
> slider (§6.3 "Channel values that are not finite", A32).
> (3) Confirmed: the client keeps any number of deformers, so 16 is an implementation
> limit (log what it drops, as §6.4 says); a clip whose track count differs from the bone
> count is reported as an error and skipped while the others still contribute, as
> implemented (A31).
> (4) Confirmed with a precision: "last in evaluation order" is right **if** the evaluation
> order is the client's start order (A33). For one script run the client starts the roots
> of the kind-1 definitions before those of the kind-2 definitions, the roots of one
> definition and the children of one port in stored order, and each node with its
> immediate starts before the next (`script-table.md` §8.2 step 3, T41). One exception to
> "stored order": a Collection starts its ports by **ascending ordinal**, which in most
> Collections is the reverse of the stored order (`script-table.md` §7.5, T42). A resolver
> that walks Collection ports in stored order picks the wrong "last" node there. No
> species table has two slider nodes, so no figure of this install changes; the same order
> decides equal-priority ties between Appearance nodes of one run (§4.7 rule 2, §7.4).

> **OPEN** (implementation, 2026-10-08, A33 implemented; confirmation only): the
> resolver lists deformers in the client's start order (`script-table.md` §8.2 step 3,
> §7.5, T41, T42; Sequence steps and Timer ports by step and time, §7.6), and the figure
> uses the set of the **last** SliderMeshDeformer in that list. A resolution evaluates every
> Random port and lists only started nodes, so a slider node under a Random port counts as
> started (static analysis), and stops of a SliderMeshDeformer are not applied. No figure
> of the install has two slider nodes (SI12), so nothing visible changes.

Implementation feedback (clean room, 2026-10-09, conformance with the Accepted text):

> **OPEN** (implementation, §5.6 rotations, axes): the implementation applies a placement
> as the location's frame followed by the offset (in the location's frame), then the
> rotation, then the node's scale, and builds the rotation as yaw about the location's
> +Z, pitch about +X and roll about +Y, composed *R*_Z(yaw) · *R*_Y(roll) · *R*_X(pitch),
> the axes and order of `coordinates.md` §11.3 for object frames. §5.6 gives the angles but
> not the axes of the location frame they turn about (the O13 OPEN covers the order, which
> no row of §5.6 exercises: every row turns about one axis). Please state which local axis
> each of `OffsetRotationYaw`, `…Pitch` and `…Roll` turns about.

> **Answered** (2026-10-09, spec writer, BIN6, A35): §5.7 steps 1–2. Yaw turns about the
> location's +Z axis, pitch about +X, roll about +Y, in degrees, right-handed, composed
> *R*_Z(yaw) · *R*_Y(roll) · *R*_X(pitch); the frame is the location's frame, then the
> offset along the location's axes, then the rotation, then the scale. The implementation's
> reading is the client's for the axes, the order and the units. (Qualified 2026-10-09 after
> the review: the composition was read only for unattached nodes; for attached nodes, which
> include every row of §5.6, it is a Tessera Decision with an OPEN, §5.7 step 2. The
> implementation's reading matches the Decision, so no code change follows.) 1,118 MeshFX nodes of the install have two or more non-zero
> angles (emotes, effects), so the order matters beyond §5.6.

> **OPEN** (implementation, §5.6 shield scale): §5.6 calls the placement scale "the node's
> uniform `Scale`", while `script-table.md` §5.2 lists the MeshFX `Scale` as a vec3, and the
> stowed-shield rows drive it from the vector property
> `Script_AdjustableScale_Secondary_Vector`. The implementation uses the property per axis
> when the figure's property lookup finds a vector (or a float for all axes), and
> (1, 1, 1) when it finds none (Tessera choice: the property's dictionary default is not
> stated here). Please confirm the vec3 reading and state what an absent property gives.

> **Answered** (2026-10-09, spec writer, BIN6 and OBS4, A36): §5.7 step 3. `Scale` is a
> vector3 applied per axis ("uniform" in §5.6 is withdrawn); it multiplies the spawned
> item's own `Render_AdjustableScale` per axis. `Script_AdjustableScale_Secondary_Vector` is
> a vector3 property whose dictionary default is (1, 1, 1), and an absent driven source reads
> as that default, so (1, 1, 1) is the client's value, not only a Tessera choice. A float
> value for a vector3 property does not occur (the property is typed vector3).

> **OPEN** (implementation, §5.6 rows not modelled): the implementation reproduces §5.6 as a
> table, not by running the six placement scripts. Two rows need data the spec does not
> carry: the cosmetic in-hand row of the primary slot (it applies when
> `Script_Item_PhysObj_Primary_Cosmetic` is one of the entity DIDs listed in the script's
> case) and the battle-gauntlet effect entity at `Wpn_RHand` / `Wpn_LHand` (yaw −90, +90),
> whose DID the script names. Both are left out (cosmetic overrides are not an input yet;
> gauntlets show only as worn parts). Should the spec name how a reader finds those DIDs
> (script key and node), or should a later implementation evaluate the MeshFX nodes of the
> placement scripts directly?

> **Answered** (2026-10-09, spec writer, OBS4, A37): both are now in §5.6. The cosmetic
> in-hand DIDs are the values of the Switch case on `Script_Item_PhysObj_Primary_Cosmetic`
> under the one-handed case of script 0x10000684 (54 distinct DIDs; the secondary script
> 0x10000593 holds the same list and places such items at `Wpn_LOffHand`); the gauntlet
> effect entity is the stored `Entity` of the two nodes under the `BattleGauntlets` case of
> script 0x10000684, and it is attached in the **stowed** family only (corrects "both
> families": the wielded pair has `Probability` 0). Evaluating the MeshFX nodes of the
> scripts directly is preferred ("What is normative" in §5.6); a table must read those DIDs
> from the nodes. Checking the tables against the scripts also corrected two rows:
> ranged-slot instruments are stowed by a per-species script (§5.3, §5.6, A38), and the
> stowed shield node of a non-ranged skill mode needs a cosmetic override (§5.6 "Shields
> out of combat", with a Decision that keeps the forearm placement).

> **OPEN** (implementation, §5.5 creature cosmetic slots): the implementation attaches the
> four `NPC_Cosmetic_Inventory_Item<n>` props while `NPC_Cosmetic_Inventory_Visible` is true,
> but not the creature cosmetic slots `Inventory_Cosmetic_Slot1` … `4`. Their property names
> are given as a stem and suffixes (`_Entity`, `_HoldingLocation`, `_Scale`,
> `_OffsetRotationYaw`, `_OffsetRotationPitch`, `_Behavior`, `_Hide`); please state the full
> names (for example whether the slot's entity is `Inventory_Cosmetic_Slot1_Entity`), which
> figures carry them (placed static figures or only server objects), and how `_Scale` and
> the two rotations apply to the attachment (as the MeshFX fields of §5.6?).

> **Answered** (2026-10-09, spec writer, OBS4, A39): §5.5 "Creature cosmetic slots". The
> names are `Inventory_Cosmetic_Slot<n>_Entity`, `…_HoldingLocation`, `…_Scale`,
> `…_OffsetRotationYaw`, `…_OffsetRotationPitch`, `…_OffsetRotationRoll`, `…_Behavior` and
> `…_Hide` for *n* = 1 … 4 (roll was missing from the earlier list). Only server objects
> carry them (3,523 0x79 property sets; no 0x47 entity, 0x78 set or landblock instance set),
> so no placed static figure has one and the offline walker needs none. They drive the
> MeshFX fields of the same names, applied by §5.7; while sheathed the script uses fixed
> placements at `Sheath_Back_Shield` instead of the slot's location and rotations.

> **OPEN** (implementation, confirmation only, §3.2.1, §3.3, §4.7, §6.1, §7.4, §15):
> engineering readings taken where the text is silent: (1) §3.2.1: a NaN modifier selects
> no block (it is above no block); a NaN `variant` is passed by the walk. (2) §3.3: the
> option store answers only `GOP_AvatarUpdateVisible` (Tessera default true); any other
> toggle property is read through the figure's own sets (instance, game object, entity) and
> reads false when absent. (3) §4.7 Decision: an item's dye element names the item's
> wardrobe record, so two items sharing one record (the chest/shoulder cross-overs of §4.2)
> share one dye key there and the later item's code replaces the earlier one (rule 2 taken
> literally). (4) §4.7 rule 4: linked records recurse at most 16 deep and a figure applies
> at most 4,096 blocks (Tessera bounds; every list of the install is empty). (5) §6.1: the
> template `vec3` product runs over the template parent chain up to the root, also above the
> template that names the setup, where the entity chain of `setup.md` §3.1 stops; template
> sections are applied root first. (6) §7.4 Decision: the walker seeds a placed figure's
> picks with its instance ID; figures without one (player bodies, the explorer) use seed 0;
> one generator per resolution, drawing only on branches the picks take, in the client's
> start order; a Random node whose only port has weight 0 picks nothing (the resolver skips
> weight-0 ports; §7.4 rule 1 would pick it). (7) §4.4 and the §15 stub answer: the mesh
> summary counts the triangles of the index buffers, not of the detail levels' draws (it does
> not read the levels); the two agree when each index buffer is drawn once. Please say if
> any reading differs from the client.

> **Answered** (2026-10-09, spec writer, OBS4, A40; folded into the sections named):
> (1) Accepted as a Tessera Decision (§3.2.1): the client's handling of NaN was not traced,
> and no modifier or `variant` of the install is NaN or infinite. (2) Confirmed, with one
> precision (§3.3 Rule): an absent toggle property reads as its dictionary default, or false
> when there is none (`script-table.md` T28); no element of the install names any property
> but `GOP_AvatarUpdateVisible`. (3) Confirmed (§3.2.1, §4.7 Decision): the later item's code
> replaces the earlier one for the shared (record, key) pair. (4) Confirmed as Tessera
> bounds (§4.7 rule 4); the client sets none, and every list is empty. (5) Confirmed
> (§6.1): the product runs to the root; no template that names a setup has a parent, so it
> equals the product up to the setup template; sections root first. (6) Seeds and the
> generator per resolution: accepted as Decision details (§7.4). The weight-0 reading
> **differs** from the client: a Random node with exactly one port always picks it, even at
> weight 0 (`script-table.md` §7.4 rule 1, BIN3); none of the 367 single-port Random nodes of
> the install has weight 0, so nothing visible changes, but follow rule 1. (7) Confirmed
> (§4.4): every index buffer of all 47,563 meshes is drawn exactly once, and both counts
> flag the same 258 meshes.

---

<a id="s16"></a>
## 16. Change log

| Date | Sections | Change | Role |
|---|---|---|---|
| 2026-10-05 | §1 (BIN, ST/SA2, RH, A10, A15–A18), §6.2, §6.3, new §6.4 and §6.5, §8 step 7, §11, §12.3, new §12.4–§12.5, §13, §14 | Height controls, bone-slider values and application, and bind-space deformation from the client (BIN, decision 0011), checked on real data (RH); the earlier Decisions of §6.2 and §6.3 withdrawn; expected standing heights added; implementation OPENs on absent slider strings and rotation order answered (the first in part). | Spec writer (dirty room) |
| 2026-10-07 | header; §1 (OBS2, BIN2, A14, A19–A27); §3.2.1; §3.3; new §3.5; §4.7; §4.9; §4.10; §4.11; §5.3; §5.4; §5.5; new §5.6; §6.1; §6.3; §7.4; §8; §11; §12.2; §13; §14; §15; new §17 | Owner items O8, O10, O11, O12, O13, O15 answered from the client where the client can settle them (BIN2, decision 0011) and from script-table data (OBS2): block choice (largest variant not above the modifier) replaces the old rule and Decision; tie rule and record order; figure-wide material-group list (dye pairing withdrawn); updated-avatar and face-compositing options; face layers and colour tints from species-table script nodes; stowed and wielded weapon placements from the shared avatar table (old stowing Decision replaced); random-appearance outfits from script tables (old Decision replaced); render-scale composition and the vector3 type; eleven implementation OPENs answered. | Spec writer (dirty room) |
| 2026-10-07 | header; §1 (OBS2, A21, A26, A27, new A28, new BIN3); §3.2.1; §3.3; §3.5; §4.2; §4.7 (new rule 4 and OPEN); §6.1; §7.4; §8 step 4; §9 (AI12, new AI13); §11; §13; §14; §17 (new O9 and O17 rows, O8 row) | Answers the review of 2026-10-07. Blocking: the render-scale limit is **strictly below** 0.0002 (A27, §6.1 twice including the Decision, §14); the §3.2.1 census splits the 79 "none" into 13 by rule 1 and 66 by rule 3; §17 gains the O9 row. Non-blocking: §7.4 states its counting rule and the corrected counts (21,780 nodes, 830 cases, 61 tables in key 5; 21,841 and 62 tables over all scripts); §6.1 states the holder factors (uniform holder scalar with its two flag bits and `Model_ParticleScale`, then the holder's final scale, BIN3); §3.3 names the two sources and the dictionary default (false); §3.5 and A21 attribute the 297 nodes and the compositor counts by kind; OBS2 node total 632,847. From BIN3: the u32 after a 0x20 record's DID counts a linked-record list applied first (§4.2, §4.7 rule 4), and the `section` byte is a lookup-table size class (its OPEN resolved). | Spec writer (dirty room) |
| 2026-10-07 (BIN4) | header; §1 (new BIN4, A16 note, new A29–A31); §6.3 (values: piece rule and reading; River-hobbit hidden slider 36 gets *t* = 0; new paragraph on bones outside the skeleton and limits that are not finite; two implementation OPENs answered); §6.4 (one slider set per figure; step 1 details; implementation OPEN answered) | Edge cases from a clean-room audit of the River-hobbit height code, settled from the client (BIN4, decision 0011). §6.3: the value string is cut into two-character pieces, an odd last character is a piece of its own, and an even-length string gets one more empty piece; each piece is read as base-16 text up to the first non-digit (`8z` reads 8), taken as unsigned and divided by 255. Correction: with 35-pair strings, the first hidden River-hobbit slider gets *t* = 0, not 0.5 (no visible change, its limits are equal). An entry for a bone outside the skeleton is not used, and the slider's other bones still apply (the implementation drops the whole slider; that differs). Limits that are not finite: Tessera Decision. §6.4: one slider set per figure; track weights truncated without tolerance; a deformer with a missing or non-fitting clip contributes nothing; no limit on deformers; time limited to [0, duration] (+∞ gives the end); NaN: Tessera Decision. The fact the audit found contradicted by the explorer (River-hobbit M body-shape control range 0 to 0.6, §6.2, Confirmed 2026-10-05 from the character-creation control data; not re-measured here) stands, as do the step 1 quantisation (re-read, no tolerance) and the §6.4 step 3 reference to the K26 transform. | Spec writer (dirty room) |
| 2026-10-08 (BIN5) | header; §1 (new BIN5, A32, A33); §6.3 (step 1 precision; new paragraph "Channel values that are not finite" with an extended Tessera Decision); §6.4 (meaning of "most recently started"); §7.4 (order of Appearance nodes in one run); §14 (OPEN on non-finite channel values answered); §15 (eight implementation OPENs answered: the four confirmation points of 2026-10-07, the §6.5 skinning confirmation, pointers for the `8z` and deformer-time OPENs, the rig rule, weenie-object precedence and static-setup default parts) | Implementation confirmation points settled from the client (BIN5, decision 0011, with `script-table.md` BIN6): the channel value is (max − min) × *t* + min in single precision, unchecked, so a non-finite value poisons the bone and those below it (Tessera Decision: drop and log the slider, as implemented); "most recently started" follows the client's start order, in which a Collection starts its ports by ascending ordinal; the piece rule, the deformer limits and the §6.5 reproduction confirmed; the weenie-object precedence and the static-setup default parts accepted as Tessera Decisions (not traced, no instance). | Spec writer (dirty room) |
| 2026-10-08 (OBS3) | header; §1 (new OBS3, new A34); §3.3 (toggle-files paragraph, the default-agreement sentence and the narrow OPEN) | Answers the blocking item of the 2026-10-07 re-review, repeated by the review of 2026-10-08. A census of all 40,643 0x47 entities (OBS3) gives: 7 avatar entities name the toggle property and carry it true; 3 more avatar entities carry it true without naming it; 19 entities carry it in all, every value true; 4 non-avatar entities (0x4700A4A4, 0x4700A4A8, 0x4700A4AA, 0x4700A4AF) name it but do not carry it. The earlier "10 entities" and "10 of the 14" are replaced by a table. The OPEN's conclusion is corrected: for those 4 entities the two candidate sources disagree even with the option on (the entity's own set would fall to the dictionary default false and show the plain files); for the 7 naming avatar entities they still differ only when the option is off. The Tessera rule is unchanged. | Spec writer (dirty room) |
| 2026-10-08 | §7.4 OPEN (O12), §17 O12 row | Owner-observation pass: the random picks of the figures' start-up scripts come from the client's global generator, seeded from the clock at start-up (`script-runtime.md` §8, R40); a new look per visit is expected. No rule of this spec changed. | Spec writer (dirty room) |
| 2026-10-09 (merge) | header | Merge of main into the specs-5 branch: the Accepted text of 2026-10-08 (BIN5, OBS3, narrow re-review) and the branch's owner-observation row above are both kept; status Revised until the O12 change is reviewed. No fact changed. | Spec writer (dirty room) |
| 2026-10-09 (BIN6, OBS4) | header; §1 (new OBS4, BIN6, A35–A40); §3.2.1 (NaN Decision); §3.3 (absent toggle property); §4.4 (triangle count); §4.7 (rule 4 bounds, shared dye key); §5.3 (Decision revised for instruments); §5.4 (Rule); §5.5 (creature cosmetic slots: names, carriers, placement); §5.6 (cosmetic rows, shield row, instrument row and paragraph, shields out of combat with Decision and OPEN, gauntlets stowed only, what is normative, O13 OPEN narrowed); new §5.7; §6.1 (template chain); §7.4 (single-port Random, seed details); §8 step 9; §15 (five implementation OPENs of 2026-10-09 answered); §17 O13 row | Answers the implementation OPENs filed with the appearance conformance pass of 2026-10-09. From the client (BIN6, decision 0011): MeshFX rotation axes (yaw +Z, pitch +X, roll +Y), order *R*_Z · *R*_Y · *R*_X, composition location × offset × rotation × scale, and the per-axis scale written into the spawned item's `Render_AdjustableScale`. From the data (OBS4): full names and carriers of the creature cosmetic slots (server objects only) and their placements; how a reader finds the cosmetic and gauntlet DIDs; corrections to Accepted text: ranged-slot instruments are drawn while stowed, the gauntlet effect is stowed-only, and the stowed shield node needs a cosmetic override (Decision keeps the forearm). Confirmations of the engineering readings (one differs: weight-0 single-port Random nodes are picked). Status Accepted → Revised. | Spec writer (dirty room) |
| 2026-10-09 (review answer) | header; §1 (A35, A36, A37, A39); §5.5 (`_Behavior` type); §5.7 steps 2 and 3 (Decision and two OPENs); §6.1 (flag-bit bullet, "What was not traced", Decision cross-reference); §13 (narrow OPENs); §15 (answer on axes qualified) | Answers the review of 2026-10-09 of the BIN6/OBS4 revision. Blocking 1: the composition location frame × placement frame was read only for unattached MeshFX nodes; for attached nodes (every placement of §5.5 and §5.6) it is now a labelled Tessera Decision with an OPEN on the attachment's per-frame composition and on `RemoveRotation` 1; A35 says so. Blocking 2: whether the two flag bits a MeshFX node sets from `UseParticleScale` and `IgnoreChildScale` are the holder-factor bits of §6.1 is now an OPEN; the §6.1 text no longer says that no link was seen and points to §5.7 step 3; the §6.1 Decision (*s* = 1) stays. Non-blocking: A37 covers the fishing-pole case and the `…_Cosmetic_Hide` gating; §5.5 and A39 give `…_Behavior` as an enum over the script-key mapper (property dictionary). No rule an implementation follows changed. | Spec writer (dirty room) |

---

<a id="s17"></a>
## 17. Owner observations: what the client settles (2026-10-07)

Answers to the owner items of `docs/checklists/owner-observations.md` in this spec's scope
(O8–O13, O15, and O17 in part), from BIN (2026-10-05), BIN2, BIN3, RH and OBS2.
"Answered" means the item no longer needs the owner for Tessera to match the original; "partly" means a narrower question remains.

| Item | Result | What the client and data settle | What still needs the owner, and why |
|---|---|---|---|
| O8 standing heights | **partly** | Template `vec3` values multiply along the template chain; the render scale is a per-axis product of `Render_AdjustableScale` (or its checkpointed copy), a uniform holder factor, `Model_Scale` and a vector of a related object, times the holder's final scale for an attached object, reset to (1, 1, 1) when a component is not finite or strictly below 0.0002 (§6.1). | Whether the template `vec3` is the vector of the related object was not traced. One screenshot pair settles it: a High-elf female at about 1.84 m (multiply) or 1.82 m (no template factor) beside a Human female (1.77 m). |
| O9 height and facial sliders | **partly** (rules answered by BIN on 2026-10-05) | The height controls hold clip fractions: `AC_BoneScale_Test` the body-shape deformer (girth only, except DwarfStoutAxe and the River-hobbit look) and `AC_Avatar_Height` the River-hobbit layer-110 clip, higher values taller (§6.2). A slider byte gives *t* = byte / 255 and the channel value `BoneSlider_LimitMin` + *t* × (max − min), so byte 0 (the left end) gives the stored minimum (the smaller scale, for example 0.7 on the nose bridge) and the default 0x80 gives *t* = 0.50196, a hair off the centre value; mirrored sliders negate the value on the reversed bone, so eye separation moves both eyes symmetrically; sliders and body shape deform the meshes once in bind space, about each bone's own joint, never the animated pose (§6.3–§6.4). On real data the computation keeps River-hobbit feet within 1 cm of the ground at every height value in idle (§6.5). | Confirmation only: the standing heights of §6.5 (shortest and tallest River-hobbit, Human ends) against the original's character-creation screen, and that River-hobbit feet rest on the ground at the default height in idle, walk and run (the walking toe-dip OPEN of §6.5). The client code read cannot show the rendered result. |
| O10 layering ties | **partly** | Equal priority: the later-applied part wins; the order of records and keys (§4.7); the layering cases of §4.6 follow from the priorities. | Worn pieces reach the original client from its server, whose order between items is not in the client; items 5 (gauntlets against a long-sleeved chest) and 1–4, 6–8 remain useful as confirmation. |
| O11 updated-avatar option | **answered** | A graphics option, default on, bound to `GOP_AvatarUpdateVisible`; with it on, the toggle (newer) style files and the newer face compositor are used; turning it off rebuilds figures with the older files; character creation offers it for Hobbit and Dwarf only (§3.3, §3.5). | Nothing required; a look at the options screen would confirm the default. |
| O12 random-appearance NPCs | **partly** | Outfits come from Appearance nodes under Switches on the type in the figures' `Startup` scripts, one random piece per group (§7.4); 316 of the 318 placed types have outfits. The picks come from the client's global generator, seeded from the clock at start-up, with no per-object seed (`script-runtime.md` R40, 2026-10-08). | Confirmation that a figure's look changes between visits (expected). Two visits to one figure settle it. |
| O13 held-item rest positions | **answered** for weapons, shields and ranged items; **partly** overall | Stowed and wielded locations, offsets, rotations and per-species scales by equipment category (§5.6); how offset, rotation and scale apply, including the yaw/pitch/roll order (§5.7, BIN6, 2026-10-09); ranged-slot instruments are stowed by a per-species script (§5.6, A38); class items have no placement script (§5.3). | The in-hand direction of long weapons (one screenshot of a drawn sword from the side), whether class items are ever visible, and whether an equipped shield shows on the forearm out of combat with weapons sheathed (§5.6 "Shields out of combat"). |
| O15 skin, hair and eye tints, face layers | **answered** for the pairing and the mechanism; **partly** for rendering detail | The face is one texture composited at run time from the face compositor (`misc-records.md` §3.6) with layer choices from `AC_Texture_*` and colours from `AC_Color_*`; hair and beard (and skin surfaces) are tinted through the dye colour 2003 set from `AC_Color_Hair` and `AC_Color_Skin` (§3.5); a second graphics option, default on, turns face compositing off. | Which texture slot of the face-skin surface receives the composed image (one frame capture of a face, as the checklist asks). |
| O17 static NPCs at the N spots (dress part only) | **needs the owner** | What a figure wears and holds is settled by §7.2–§7.4 and §5.5 (data and BIN2); positions and headings come from the SED data (`entity-descriptor.md`), which the client places as stored. | Presence, position and facing at each spot depend on the original's server and content layers, not on client code; only the in-game `/loc` readings and a look can settle them. |
