<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Mounts and riding: mount effects, mount entities, rider presentation, mounted movement, war-steed stats and landblock riders

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review 2026-10-23 by spec-review:spec-gameplay:2 (independent session), a session that wrote no part of the 2026-10-23 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23 revision of §14.3 and §14.7: walking and running back clips per family, the `Equipment_Hide` targets and the dismount timer confirmed, rows MT52–MT53; no implemented behaviour changes; awaiting independent review). Before: **Accepted** (2026-10-14 revision of §14.6 — crossing-step Decision, gait 3 census, jumping while mounted, the narrowed MQ9 OPEN, provenance keys OBS4, SCR4, BIN3 and rows MT48–MT51 — reviewed and accepted 2026-10-14 by an independent spec reviewer, fresh session, not the writer; CLEAN_ROOM.md §5.3, all items). Before: **Revised** (2026-10-14, spec writer, dirty room, awaiting review). Before: **Accepted** (P5 revision — new §14 on mounted presentation and gaits, MQ4/MQ5/MQ9 answered for Tessera — reviewed and accepted 2026-10-13 by an independent spec reviewer, fresh session, not the writer; CLEAN_ROOM.md §5.3, all items). Before: **Revised** (2026-10-11, P5 revision, awaiting review); **Accepted** (2026-10-09; the same-day revision of §4.4 and §5.3 re-reviewed and accepted 2026-10-09); §1–§13 unchanged in substance except the pointers named in §15 |
| **Last updated** | 2026-10-23 (spec writer, dirty room: §14.3 back clips per style, §14.7 confirmations for `walker.md` §16.12). Before: 2026-10-14 (spec writer, dirty room: §14.6 follow-ups of the P5 implementer OPENs). Before: 2026-10-11 (spec writer, dirty room: P5 revision, §14) |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session), not the author |
| **Review record** | **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-gameplay:2 (independent session)): **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; MT52 (about ten rows of clip DIDs, labelled observations, read at run time by the walker) judged not a bulk data table under the CLAUDE.md content rules; MT53 consistent with §14.7. Earlier: 2026-10-09, spec reviewer (fresh session): passed (CLEAN_ROOM.md §5.3; 7 OBS/BIN facts re-checked independently). Non-blocking notes in the review log. 2026-10-09, revision review by an independent spec reviewer (fresh session, not the writer): **passed** (CLEAN_ROOM.md §5.3, all items). Changes reviewed: §4.4 per-axis scale and the Answered note, §5.3 strafe pairing and the Answered note, §11.4 example, §13 row for `collision.md` §11.6, rows SCR2, BIN2, MT31, MT32, change log. Spot checks, read-only: MT31 re-counted over every decoded 0x07 and 0x0C record (10,349 MeshFX nodes; 38 with `Mount_MeshFX_Entity`, all with equal components, 18 driving `Scale`; 875 others unequal; both properties vector3 in the dictionary); MT32 re-read on a private copy of the analysed project (deleted after use): the field registration pairs `StrafeLeftMoveSpeed` and `StrafeRightMoveSpeed` with the last two of the four consecutive speed floats, and the war-steed block copies *S*1 and *S*2 from them in that order. The 397 stored slot-scale values were not re-counted. Non-blocking notes for the next revision: (N1) MT31 counts 38 mount MeshFX nodes while §4.4, MT11 and MI9 count the 35 nodes driven by `Mounted_Player_Cosmetic_Slot_Entity`; say in §4.4 that 3 more nodes write `Mount_MeshFX_Entity` with a stored, undriven entity. (N2) The §13 row for `collision.md` says "the third and fourth float after `ControlAcceleration`"; other fields lie between, so say "the third and fourth of the four consecutive speed floats", or drop the object layout from MT32 and §13 altogether (implementers do not need it; the pairing by property ID is the fact). (N3) The short names `StrafeLeftMoveSpeed` etc. omit the dictionary prefix `GenericPhysicsController_`, as `collision.md` §11.6 does; a one-line note in §1 would help readers look them up. 2026-10-13, P5 revision review by an independent spec reviewer (fresh session, not the writer; reviewed `git diff 8e9670d..f18ddd7` of this file): **passed** (CLEAN_ROOM.md §5.3, all items). Changes reviewed: new §14 (§14.1–§14.13), rows OBS3, SCR3, MT33–MT47, invariants MI14–MI18, questions MQ4/MQ5/MQ9 (Answered notes), MQ13–MQ15, the pointers in §4.1/§4.4/§4.9/§5.8 and §13, and the §15 change-log row. The P5 revision adds no `BIN` row and used no method M4 (SCR3: "No method M4 was used for this revision"); BIN and BIN2 were accepted in the 2026-10-09 rounds, so no private Ghidra copy was needed for this review. Spot checks, read-only (M1, prebuilt `tessera-dattool cat` against the user install, in place): 19 representative DIDs named in §14 all resolve to records of the stated families — mount effects 0x7002D53C, 0x70031E9B, 0x70066B26, 0x7002D53D, war-steed startup-with-entity 0x7002EDDF and 0x7003230E, MountDirectory 0x79048B29, entities 0x47000361 (horse) and 0x47005D62 (medium war-steed), clips 0x05001447/0x0500522F/0x05005237/0x05005231/0x05005236, mount table 0x070007A8, locomotion template 0x0C000FFE, closure table 0x070003A3, player locomotion 0x0700001C, and the no-entity startup record 0x70033562. Internal arithmetic re-checked: the §14.11 census sums to 722 (normal 679 = 590 + 72 + 9 + 6 + 2; + Clydesdale 4 + war-steeds 25 + 2 startup + 3 horse-with-bit + 8 hobby-horse + 1 no-entity); §14.2 count 10,830 = 722 × 15, 10,815 = 10,830 − 15; §14.7 induction 688 = 660 + 28; the §14.13 gear and placement worked examples are self-consistent. Cross-references verified present and consistent: §5.1 Decision (speed factor), §4.6 item 4 (Clydesdale gait rule), §2.6 fields (`ForwardSource_Movement_MountSpeedMultiplier`, `Movement_MountSpeedBonus`), `Mount_Name_Initial` (§2.3). No code, no pseudo-code, no copied prose, no game text, no key/hostname/anti-cheat fact. Non-blocking notes in the review log. 2026-10-14, review of the §14.6 revision of commit e5ce2c6 by an independent spec reviewer (fresh session, not the writer; reviewed `git show e5ce2c6 -- docs/specs/mounts.md`): **passed** (CLEAN_ROOM.md §5.3, all items). Changes reviewed: §14.6 item 4 (crossing step), "Gait 3", "Jumping while mounted", `Mount_Movement_Jump_Modifier`, the narrowed MQ9 OPEN, §14 intro, rows OBS4, SCR4, BIN3, MT48–MT51, change log. Spot checks with the reviewer's own read-only probes (M1, repository decoders, install in place): MT51 re-run as a raw 4-byte search of every record of the three archives (845,460, 261,483 and 25,400 records, none unreadable): hits only in the dictionary, one enum mapper and the two tables 0x0700001C and 0x07002B86 (four each, the eight driven pairs); MT49 re-counted over all 7,524 decodable tables (2 failures) and 28,992 scriptlets: 36 SetProperty nodes on `MountMovement_CurrentSpeed`, gait 3 written only in the `Mounted_Channeling_Loop` scripts of those two tables, eight 0 (server-only), ten 7 (`Cbt_Mounted_Charge`), the extra 2/4/6 and the one 2 in a third table as stated; one When pair Equal 2 / NotEqual 2 in each of the two `Loc_Jumping` copies; MT50 re-read from the full `Loc_Jumping` tree of 0x0700001C (root WhenRunning, Running port to the Switch on `Mount_CurrentMountType`, case Equal 0x8/0x10/0x20, gait cases 7, 6, 4, 5 with the driven 0.285/15 impulse and an empty case 3, Default branch with `Slider_Propel` greater than 0.5, `Slider_Strafe` greater than 0.25 / less than −0.25 empty and its Default setting `Movement_IsJumping`, `Emote_Idle1` on the Child target, the undriven 0.285/15 impulse), 0x07002B86 with both branches and 0x070017E4 with the Running port holding only the Default form; the 14 avatar class tables of `script-table.md` §8.2 all include 0x0700001C directly; port-type and script-key labels re-read from their mappers; MT48 re-derived per race-table entry (15 of 15: Tall 347 = 323 Normal + 18 war-steed + 2 Clydesdale + 4 hobby-horse, Short 329 = 322 + 1 + 2 + 4, lists of 350 and 332, first Normal entry 18 or 1, type Horse); arithmetic 7.275 = (0.285 + 0.2) × 15 and 16/60 ≈ 0.27 m/s re-checked. BIN3 re-read on a private copy of the shared analysed project (Ghidra 12 headless, deleted after use; executable SHA-256 prefix d22363eb9aff0fb5 re-verified): the routine that applies a node's driven pairs at start reads the source through the entity property read into an empty holder and writes the field only when that read reports success; unlike the Switch's read, the holder is not pre-filled with the dictionary default. Offline client code only; nothing about network, login, keys, the launcher or anti-tamper. No code, no pseudo-code, no copied prose, no game text, no key/hostname/anti-cheat fact; the node-tree prose describes the install's script data, not a decompiler listing; BIN3 carries no listing, symbol or address. Non-blocking notes for the next revision: (N1) MT49 "413–473 Switch cases per gait value" counts single-value cases only; six cases hold several values ([0, 2, 1], [6, 5], [6, 7], two each), so each value is named by 415–475 case entries; say which count is meant. (N2) §14.6 "Jumping while mounted" could say that the war-steed branch's gait-1 and gait-3 presses set `Movement_IsJumping` and run `Jump` with no impulse, so the walker needs a rule for when that flag clears without a take-off (see `walker.md` §16 review note). |
| **Implements into** | `crates/props` (typed views of the mount effect, §2, and of the template mount and war-steed properties, §6); `crates/physics` (the war-steed controller, §5.3–§5.8, next to the controller of `collision.md` §11); `crates/scriptgraph` and `apps/client` (rider presentation through the script tables, §4; the walker's summon and dismiss, §4.9); `apps/explorer` (mount card and mount viewer, §10); `apps/client` walker mounts (P5: clip choice, gait rule, sequence, placement, camera, §14; the walker's side in `walker.md` §16); Tessera's own server (summoning, durability, war-steed stats, landblock riders, §2.4, §6, §7) |
| **Companion specs** | [`combat-data.md`](combat-data.md) §3 (two halves), §5 (modifier entries), §6 (effects; class 2459 is listed there as "Mount"), §7 (skills); [`game-controls.md`](game-controls.md) §5.1 (MountDirectory), §5.2 (MountAppearanceTraits), §5.3 (MountedCombatControl, gait multipliers); [`collision.md`](collision.md) §10–§11 (movement parameters, layer choice, the player controller, the movement input, turning, the jump); [`script-table.md`](script-table.md) §4–§8 (nodes, driven properties, targets, MeshFX layout); [`script-runtime.md`](script-runtime.md) (how node graphs run); [`setup.md`](setup.md) and [`skeleton-anim.md`](skeleton-anim.md) §8 (holding locations); [`appearance.md`](appearance.md) §7 (instance sets, NPC figures); [`creatures.md`](creatures.md) §2, §3.2.1, §6 (templates, mirror properties, spawning); [`landblock.md`](landblock.md) §4.8; [`misc-records.md`](misc-records.md) §13.1 (PropertyMetaData); [`property-system.md`](property-system.md) §7.3 (bit-to-key rule), §8 (DID maps); [`wlib-bytecode.md`](wlib-bytecode.md) (the class library's code); [`walker.md`](walker.md) §16 (mounts in the walker, P5); [`combat-data.md`](combat-data.md) §7.9 (induction records) |

---

<a id="purpose"></a>
## Purpose

Riding is a core client feature, and before this spec no specification decoded the mount
records. `combat-data.md` lists the 722 effects of root class 2459 only by role ("Mount");
`game-controls.md` §5 covers the global mount controls (the directory of mount skills, the
appearance traits, the gait multipliers). This spec adds:

- the **mount effect** (class 2459): every field, its census over all 722 records, its
  compiled half, and what each field is for (§2);
- the **mount entity** a mount effect names and how its setup is reached (§3);
- how the client **presents a mounted rider**: which entity is drawn where, which scripts
  run on the rider and on the mount, and which rider properties drive them (§4). This is
  carried by the script tables and the class library in the user's install, so most of it is
  Confirmed from data;
- the **mounted movement model**: the normal mount on the player controller of
  `collision.md`, and the war-steed controller with its acceleration, turn rate, gait and
  speed limits, read from the client (§5);
- the **mount and war-steed stats** on creature and avatar templates, their forwarding pairs
  and the records that modify them (§6);
- the **NPC riders** of the landblocks: the per-instance mount, rider-pose and banner
  properties and how the data consumes them (§7);
- the reference graph, invariants, worked examples and the open questions (§8, §9, §11,
  §12);
- (P5) the **mounted presentation in the walker**: the clips rider and mount play for each
  motion and gait, the gait rules, the mount and dismount sequence, placement, seat and camera,
  the mounts each playable body rides and a census of all mount effects (§14).

Game text is never reproduced. Records are named by DID, fields by property label, values by
enum-mapper label; those labels are interoperability identifiers (they are how a client finds
the records and fields at run time) and are quoted for that reason only. Every count and DID
is an observation of this install and is read at run time.

The network protocol, login, keys, the launcher and anti-tamper are out of scope (decision
0004). Where the original's behaviour depends on server messages, this
spec says so and stops.

---

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam, client data current on 2026-10-08) on 2026-10-08 and 2026-10-09 with throwaway probes in the dirty-room scratch area built on the project's decoders (`tessera-dat`, `tessera-formats`, `tessera-props`, `tessera-wstate`, `tessera-wvm`), read-only. Read **in full**: every object record (0x70) and every property set (0x78, 0x79) of `client_gamelogic.dat` (383,332 object sets, 1,616 bare sets); every entity descriptor (0x47, 40,643, all decoded); every landblock-info record of `client_cell_1/2/3/4/5/14/15` (49,636, all decoded: static-entity sets, weenie instance sets, link sets, landblock sets, cell sets); every setup (0x04, 28,555, all decoded); the property dictionary (27,546 definitions) and the enum mappers named below. A property is "mount-family" when its label contains `Mount` or `Dismount` (767 dictionary definitions); every occurrence of every mount-family property, at any nesting depth, was counted per holder class, with value types and value distributions, and every property-ID value naming a mount-family property was counted by holder. |
| SCR | The script tables (0x07, 7,524 decoded, 2 failures as in `script-table.md` §10.3) and scriptlets (0x0C, 28,992) of the install, read on 2026-10-09 with the same decoders: every node whose driven pairs (`script-table.md` §4.2) name a mount-family property, every Switch and When node watching one, every Run and WhenRunning node naming the mount script keys of §4.3, and the full node trees of the scripts quoted in §4. The MeshFX data (kind 41, opaque in the decoder) was read by the layout of `script-table.md` §5.2. This is data in the user's install (method M1). |
| BC | The class library's bytecode (record 0x56000000, chunk type 1), read on 2026-10-09 with the project's disassembler (`tessera-wvm`, following `wlib-bytecode.md`): the methods of class 2459 that are not inherited from class 734 (7 bodies), the mount-info class 2464 (9 bodies), the mount-system classes 1358 and 2469 (all bodies with a mount-family constant), and a scan of the whole code block for constants equal to the PIDs of every mount-family and cosmetic-slot property. Data in the install (method M1). |
| BIN | **M4 (decision 0011).** Client `x64/lotroclient64.exe`, build 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (verified on 2026-10-09), read on 2026-10-09 with Ghidra (headless, on a private copy of the shared analysed project, deleted after use) and a byte scan of the read-only file. **Questions:** (m1) does native code read the mount effect's rider fields or the cosmetic-slot fields by constant property ID; (m2) what native code does with `Mount_Movement_*_Cached`, `MountMovement_CurrentSpeed` and `MountedCombat_Current_LockedGear` (the war-steed controller); (m3) the source of the second forward-speed limit left open by `game-controls.md` Q8; (m4) what the property natives called by the mount-system bytecode do (package 169, functions 3 and 45), located with the registration pairing of `wlib-bytecode.md`'s BIN row; (m5) which bits of `Mount_CurrentMountType` native code tests. **Method:** an instruction scan for immediates equal to 230 mount-family PIDs; a byte scan of the whole file for the 4-byte PIDs of the 12 rider fields of §2.2, the 57 cosmetic-slot PIDs and the 9 mount script keys; decompilation of the functions found. **Why M1–M3 were not enough:** the movement controller and the native property functions exist only in the executable; the data say which properties exist, not which side reads them or how they combine; no published source describes them; no running client is available to this session (M3). **Not read:** network handling, login, keys, the launcher, anti-tamper. The war-steed controller blends its state toward server updates (the controller is server-authoritative, §5.9); that blending was seen at its entry and not followed (decision 0004). No listings, symbol names or addresses are carried into this spec. |
| SCR2 | The same script tables and scriptlets, read again on 2026-10-09 for this revision with a new throwaway probe: the `Scale` field, its driven pair and `MeshIIDTargetProperty` of **every** MeshFX node (10,349 nodes in all decoded 0x07 and 0x0C records); the property dictionary entries of `MeshFXScript_Scale` and `Mounted_Player_Cosmetic_Slot_Scale`; every value of the latter in all 0x78 and 0x79 sets (nested values included) and all 3,768,840 landblock instance sets. |
| BIN2 | **M4 (decision 0011)**, same build, 2026-10-09, Ghidra 12 on a private copy of the shared project (deleted after use). **Question (m6):** which property names the two strafe slots of the war-steed parameter block carry. **Why M1–M3 were not enough:** every observed movement layer has equal strafe speeds, so no data and no observation of the running client can tell the slots apart. **Read:** the field registration of the layered movement-parameter object (property ID and accessor of each field), the accessors of `ForwardMoveSpeed`, `BackwardMoveSpeed`, `StrafeLeftMoveSpeed`, `StrafeRightMoveSpeed` and `ControlAcceleration`, and again the war-steed parameter-block builder and the sideways target of §5.4. |
| OBS3 | Observed in the same install (client build 4906.0071.1794.4035; archives unchanged since 2026-10-04) on 2026-10-11 with throwaway probes in the dirty-room scratch area built on the repository's decoders, the clip resolver of `tessera-formats` (`script_table::resolve`, `script-table.md` §11.3) and `tessera-anim`, read-only. **Rider census:** every one of the 722 mount effects on each of the 15 race-table entries (10,830 evaluations of the idle, walk and run states with the rider state of §14.2); full motion matrices (12 motion states, gaits 1–7 for war-steed types) for ten representative effects (one per mount type) on all 15 entries, the airborne states with the `Jump` script (key 1) running, and the hobby-horse with `Emote_Holding_HobbyHorse` running instead of the channelling loop; script runs of the summon, mounting and dismount scripts. **Mount census:** all 605 (mount entity, mount type) pairs that the 721 effects with an entity reach (601 entities), in the states idle, walk, run, back, turn, strafe, airborne and, for war-steed types, trot and canter; the resolver does not follow targets, so the probe gave the mount the rider's values of the properties its graph reads through `MeshFXScriptOutput_Source` (§14.1) and marked `Mounted_Channeling_Loop` running. **Skeletons:** 222 distinct (setup, clip) pairs of riders and mounts bound to their skeletons; 527 (setup, clip) pairs of mounted rider clips sampled at 16 times each for bone 0 and the bone height range. **Records:** the MountDirectory's two skill lists against every skill that toggles a mount effect; `Skill_MountRacialConversionAnalog` of all 688 mount skills; MountedCombatControl's `Mount_Control_Tree_To_Skill_Hash`; the five induction records (class 1195) the mount skills name; the player-avatar objects' mount template values; dictionary entries of the properties named in §14. |
| SCR3 | The script tables and scriptlets of the install, read on 2026-10-11 with the repository's decoder: every node of all 7,524 decodable tables and 28,992 scriptlets with a Property target, tallied by the property it reads and by whether the same record reads it untargeted; a byte scan of every 0x07 and 0x0C record for the property IDs of `MountMovement_LastSpeedChangeTime`, `MountedCombat_Current_LockedGear`, `GOP_MountedMovement_ReleasedKeys_LoseMomentum` and `MountMovement_CurrentSpeed`; full node trees of `Mounted_Channeling_Start`, `…_Mounting`, `…_End`, `…_Failure`, `Mounted_Player_Cosmetic_Start` and the summon scripts, the mounted branch of the locomotion template 0x0C000FFE and the mount locomotion graph of 0x070007A8. No method M4 was used for this revision. |
| OBS4 | Observed in the same install (client build 4906.0071.1794.4035; archives unchanged since 2026-10-04) on 2026-10-14 with throwaway probes in the dirty-room scratch area built on the repository's decoders and the mount model of `tessera-gamedata` (as a tool), read-only: the offered list (§14.10 order) of each of the 15 race-table entries with the walker category (§14.11) of every entry; the MountDirectory's two groups by category; a raw search of **every** record of `client_gamelogic.dat` (845,460 records), `client_general.dat` (261,483) and `client_anim.dat` (25,400) for the 4-byte property ID of `Mount_Movement_Jump_Modifier`; the labels of the script keys, port types and holding location named in §14.6. |
| SCR4 | The script tables (7,524 decoded, 2 failures) and scriptlets (28,992) of the install, read on 2026-10-14 with the repository's decoder: every SetProperty, Switch and When node on `MountMovement_CurrentSpeed` and every PhysicsImpulse node with `IsJump`, each with the path from its script root; the full trees of `Loc_Jumping` in the three tables that define it; the include lists of all decoded tables (the direct includers of those three). |
| BIN3 | **M4 (decision 0011).** Client `x64/lotroclient64.exe`, build 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (verified on 2026-10-14), read on 2026-10-14 with Ghidra 12.1.3 (headless, decompiler, on a private copy of the shared analysed project, deleted after use). **Question (m7):** when a script node with a driven pair starts and the entity has no value for the source property, does the field take the property's dictionary default or keep its stored value? **Why M1–M3 were not enough:** no record of the install stores `Mount_Movement_Jump_Modifier` (OBS4), so the data cannot show the effect; `script-runtime.md` §5.2 says "has a value" without saying whether a dictionary default counts; no published source describes it; the property is set only at run time from outside the data, and no running client is available to this session (M3). **Read:** the routine that applies a node's driven pairs at start and the entity property read it calls (the same read that `script-runtime.md` BIN-R found for the Switch). **Not read:** network handling, login, keys, the launcher, anti-tamper. No listings, symbol names or addresses are carried into this spec. |
| SPEC | Facts stated, with their own provenance, in the companion specs; cross-references only. |
| DEC | Tessera's own choices where the data, the client library and the executable say nothing. Marked **Decision**; not claims about the original. |

Marks: **Confirmed** = observed in the install over every record of the kind named (dates
above); **Confirmed (script data)** = read from the install's script tables (SCR);
**Confirmed (bytecode)** = read from the class library's code (BC), every observed record
agreeing where a census applies; **BIN** = read from the executable; **Inferred** = a reading
consistent with the data and code that was not traced end to end; **Decision** = DEC.

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| MT1 | Class 2459 (parent 734 ← 698): 722 object records, 939,897 bytes of object state and 786,865 bytes of property sets; all decode on both halves (§2.1) | OBS (722 of 722) | Confirmed |
| MT2 | Field census of the mount effect: rider fields, identity and durability fields, war-steed fields, common effect fields with their fixed values (§2.2–§2.5) | OBS (722 of 722, full recursive walk) | Confirmed |
| MT3 | Modifier entries of the mount effects: 719 arrays, 4,441 entries; the speed entry with its modifier list; the camera and combat-flag entries; the war-steed stat entries (§2.6) | OBS | Confirmed |
| MT4 | Record groups: 691 durability mounts, 31 war-steed records, 8 hobby-horse records within the 691, 3 monster/NPC startup records within the 31 (§2.1) | OBS | Confirmed |
| MT5 | Compiled half: every root object holds, in slot 29, an object of class 2464 with a rider property set (the 9–12 rider fields) and a mount property set (durability, name, pip type, mount type) (§2.7) | OBS (722 of 722); BC (class 2464 attribute list, its getters) | Confirmed |
| MT6 | `Mount_Type` and `Mount_CurrentMountType` are 32-bit bitfields of mapper 0x23000280; key *k* is bit *k* − 1; census by key (§2.8) | OBS; SPEC (`property-system.md` §7.3) | Confirmed |
| MT7 | A mount type is a war-steed type exactly when it has a bit of 0x38 (keys 4, 5, 6); the client library and native code test that mask (§2.8, §4.7) | BC (mount-system bodies); BIN (m5); SCR (Switch cases 0x8, 0x10, 0x20) | Confirmed (bytecode) + BIN |
| MT8 | Mount entities: 607 distinct 0x47 records named by `Mounted_Player_Cosmetic_Slot_Entity` (601 by effects, 19 by templates, 13 shared), all resolving to 13 setups with skeletons; only one of them (named by templates only) carries `Mount_Connection`; the entity's own `Mount_CurrentMountType` agrees with the effect's `Mount_Type` in 676 of 721 records (§3) | OBS | Confirmed |
| MT9 | Rider setups: 34 setups carry the holding location `Mount_Connection`; 33 on bone 0 with identity rotation and a downward offset of 0.51–2.66 m (§3.3) | OBS (28,555 setups) | Confirmed |
| MT10 | Skill → effect: each of the 676 MountDirectory skills toggles exactly one mount effect through a one-element `Skill_Toggle_Effect_List`; 12 more skills do the same; 685 effects are toggled by skills (§4.1) | OBS | Confirmed |
| MT11 | The rider's mount mesh is drawn by MeshFX script nodes (kind 41) of the script `Mounted_Player_Cosmetic_Mesh`, whose fields are driven by the rider's `Mounted_Player_Cosmetic_Slot_*` properties, and the spawned mesh entity's IID is written to `Mount_MeshFX_Entity` (§4.4) | SCR (35 nodes in 5 tables) | Confirmed (script data) |
| MT12 | The rider's mount scripts are Run nodes driven by `Mounted_StartScriptID`, `Mounted_AnimationScriptID`, `Dismounted_ByPlayer_Script` and `Dismounted_ByOther_Script` in `Mounted_Player_Cosmetic_Start`; their order and the dismount choice by `Mount_CurrentMountStatus` (§4.3) | SCR | Confirmed (script data) |
| MT13 | Native code reads none of the 12 rider fields by constant property ID; the client library copies them only into the class-2464 object (§4.8) | BIN (m1, byte scan of the whole file: 0 occurrences for 11 PIDs, 1 for the roll field as a user-interface element ID); BC | BIN; Confirmed (bytecode) |
| MT14 | Mount scripts on the mount entity read the rider through the property target `MeshFXScriptOutput_Source`; the rider's scripts reach the mount through the Child target at `Mount_Connection` (§4.5) | SCR | Confirmed (script data) |
| MT15 | `Mounted_Channeling_Loop` on the player sets `Mount_Is_Mounted` true and `Mount_CurrentMountStatus` 5 (server-only) when it starts, and resets them when it stops; it mirrors the gait to the mount for war-steed types (§4.6) | SCR (table 0x0700001C; a variant in 0x07002B86) | Confirmed (script data) |
| MT16 | The mount system of the client library: on a change of `Mount_CurrentMountType`, it sets `Mount_Movement_Use_RampUp_Controller` and `Movement_ServerAuthoritative_Controller` to "the type has a war-steed bit" (§4.7) | BC; BIN (m4: package 169 function 45 sets a boolean property) | Confirmed (bytecode) + BIN |
| MT17 | The mount effect's applicability check: target is a player or an AI; the target's own entity, scale, bone offset and mount type replace the effect's; not in water; the area's, dungeon's and volume's allowed mount types (§4.7) | BC; OBS (`Area_Allowed_Mount_Types` on all 675 class-690 objects, `Dungeon_Allowed_Mount_Types` on all 1,303 class-691 objects) | Confirmed (bytecode) |
| MT18 | Normal mounts move with the player controller and the Mounted layer of `collision.md` §11.1; their speed bonus raises `Movement_Effect_MountSpeedMultiplier` through the forwarding pair; no client code combines it into the controller's speed multiplier (§5.1) | OBS (PropertyMetaData); BIN (m1, m2); BC (scan) | Confirmed; combination OPEN |
| MT19 | War-steed controller parameters: built from the layered movement parameters, then forward speed, forward acceleration, turn rate and gear-change rate replaced by the actor's `_Cached` values when present; defaults 150 and 2.0; speed and acceleration multipliers (§5.3) | BIN (m2) | BIN |
| MT20 | War-steed target velocity, integration, forward limits (gait, locked gear, world limit) (§5.4–§5.5) | BIN (m2, m3) | BIN |
| MT21 | The second forward-speed limit is the world value of the property named by the RIPCORDS entry `MountedCombat_MaxSpeed` (key 0x83 here): `World_MountedCombat_MaxSpeed`, dictionary default 16.0 (§5.5; answers `game-controls.md` Q8) | BIN (m3); BC (the script function that native code calls); OBS (RIPCORDS map, the ripcord record, the dictionary) | BIN + Confirmed |
| MT22 | War-steed turning: (gait turn multiplier + add-mod) × the parameter turn rate × update time × turn input; `Movement_FreezeRotation` stops it (§5.6) | BIN (m2); SPEC (`game-controls.md` §5.3.2) | BIN |
| MT23 | War-steed published sliders: strafe and propel as fractions of the current limits (§5.7) | BIN (m2) | BIN |
| MT24 | Mount and war-steed stats on templates: the properties, the classes that carry them and their class-wide values (§6.1) | OBS (all 38,651 creature-subtree and avatar templates) | Confirmed |
| MT25 | Forwarding pairs of the mount stats (PropertyMetaData) (§6.2) | OBS (5,211 of 5,211 metadata objects) | Confirmed |
| MT26 | Records that modify mount stats (effects, traits, relics, class-1346 objects, items, progressions) (§6.3) | OBS (every modifier entry and property-ID value) | Confirmed |
| MT27 | NPC and monster riders: 280 templates carry `Mounted_Player_Cosmetic_Slot_Entity`, all of them with `Mount_Type` and the startup effect 0x70033562; 418 templates start a mount effect (§6.6) | OBS | Confirmed |
| MT31 | `MeshFXScript_Scale` and `Mounted_Player_Cosmetic_Slot_Scale` are vector3 properties (dictionary). All 38 MeshFX nodes whose `MeshIIDTargetProperty` is `Mount_MeshFX_Entity` store three equal components (18 of them drive `Scale`); 875 of the other 10,311 MeshFX nodes store unequal components, for example (0.7, 0.7, 1.0); all 397 stored `Mounted_Player_Cosmetic_Slot_Scale` values are uniform (§4.4) | SCR2 | Confirmed (script data) |
| MT32 | The layered movement-parameter object holds `ForwardMoveSpeed`, `BackwardMoveSpeed`, `StrafeLeftMoveSpeed`, `StrafeRightMoveSpeed` as four consecutive floats and `ControlAcceleration` as a vector3 before them; the war-steed block takes *S*1 from the `StrafeLeftMoveSpeed` field and *S*2 from the `StrafeRightMoveSpeed` field, *F*, *B* and *a* from the forward, backward and acceleration fields (§5.3) | BIN2 (m6) | BIN |
| MT28 | Landblock rider instance sets: 1,549 instance sets carry at least one of the seven rider keys; census, link roles, enum mappers (§7.1–§7.3) | OBS (all 3,768,840 instance sets) | Confirmed |
| MT29 | How the data consumes the rider keys: rider-pose scriptlets by `Mount_RiderType`, Appearance nodes by `Mount_MountAppearance`, banner scripts by the banner keys (§7.4) | SCR | Confirmed (script data) |
| MT30 | Durability: 691 records with three change events (event IDs 22, 17, 31 in that order), base maxima 60–300, regeneration 1.0 or 1.5 (§2.4) | OBS | Confirmed (data); rules server-side |
| MT33 | Mount scripts read the rider through Property targets on `MeshFXScriptOutput_Source` (55 records; sliders, style, gait, `Mount_Is_Mounted`, element, `CharGen_Stage`, type; 87 WhenRunning and 34 Run nodes); untargeted nodes read the mount (§14.1) | SCR3 | Confirmed (script data) |
| MT34 | The rider's mounted clips come from the WhenRunning (`Mounted_Channeling_Loop`) branch of the locomotion template; layers 13, 20, 51, 54, 60, 65, 75; every effect × entry finds idle and walk clips, and run clips with the Clydesdale gait (§14.2) | OBS3 (10,830 of 10,830); SCR3 | Confirmed |
| MT35 | Every (mount entity, type) pair finds idle, walk, run, back, turn, strafe and airborne clips except the 4 hobby-horse entities (§14.3) | OBS3 (605 pairs) | Confirmed |
| MT36 | War-steed forward clips of rider and mount follow `MountMovement_CurrentSpeed`; rider and mount clip lengths match per gait (§14.3, §14.4) | OBS3 | Confirmed |
| MT37 | Normal mount clips do not depend on the gait; the Clydesdale rider uses the war-steed riding set (§14.4) | OBS3 | Confirmed |
| MT38 | Bone 0 stays at its reference position in all 504 sampled mounted rider clips of layers 13, 20, 51, 54, 60, 65; the 11 layer-75 offset clips (23 pairs) hold it at one fixed offset each (§14.8) | OBS3 | Confirmed |
| MT39 | Each race's war-steed effect stores a `BoneOffset` that cancels its male riders' layer-75 bone-0 rise to within 0.001 m (12 effects; females within 0.04 m) (§14.8) | OBS3 | Confirmed (data); the reading "translation in the location frame" Inferred |
| MT40 | `Mount_Control_Tree_To_Skill_Hash`: 24 entries, 12 skills, one per tree and race group, each toggling one war-steed mount effect and the same three class-718 effects (§14.10) | OBS3 | Confirmed |
| MT41 | Tall/Short directory groups: 330 symmetric analog pairs, 328 Tall–Short; 324 of 329 Short skills' effects store a slot scale below 1, 332 of 347 Tall skills' effects none (§14.10) | OBS3 | Confirmed |
| MT42 | Mount skills name five induction records: 2.0 s with `Mounted_Summon_Start` (660 skills), 3.0 s with `Mounted_Warsteed_Summon_Start` (28); none allowed while moving (§14.7) | OBS3 | Confirmed |
| MT43 | The mounting clip is started by `Mounted_Channeling_Mounting` on layer 54; no dismount script starts a rider clip; the mount bows at rest (§14.7) | OBS3; SCR3 | Confirmed (script data) |
| MT44 | Step 2 of `Mounted_Player_Cosmetic_Start` is a persistent Run; read with `script-runtime.md` §12.4 step 3 the graph never reaches step 4; `Mounted_Channeling_End` has the same shape (§14.7) | SCR3; SPEC | Confirmed (script data); consequence Inferred |
| MT45 | Mounted clips are driven by `Movement_MountAnimationSpeedMultiplier` (default 1.0) except the 89-bone walk clips and war-steed summon clips (`Movement_SpeedMultiplier`); mount clips carry no displacement (§14.2, §14.3) | OBS3 | Confirmed |
| MT46 | No script table or scriptlet names `MountMovement_LastSpeedChangeTime`, `MountedCombat_Current_LockedGear` or `GOP_MountedMovement_ReleasedKeys_LoseMomentum`; 62 name `MountMovement_CurrentSpeed` (§14.6) | SCR3 (byte scan) | Confirmed |
| MT47 | 711 effects `Set` the camera look-at offsets 0.75 (*z*) and 0.2 (*y*); the dictionary holds `Entity_Camera_LookAt_OffsetX`/`Y`/`Z` (float, default 0.0); readers not traced (§14.9) | OBS3; OBS | Confirmed (data) |
| MT48 | Offered lists (§14.10 order) of the 15 race-table entries: Tall group 347 skills (323 Normal, 18 war-steed, 2 Clydesdale, 4 hobby-horse; the first 18 are the war-steed skills), Short group 329 (322 Normal, 1 war-steed first, 2 Clydesdale, 4 hobby-horse); lists of 350 (Tall) and 332 (Short) entries with the race's three war-steeds appended, not de-duplicated (a war-steed effect can appear twice); the first Normal entry is index 18 (Tall) or 1 (Short), a Horse-type effect, for every entry (`walker.md` §16.2) | OBS4 (15 of 15) | Confirmed |
| MT49 | `MountMovement_CurrentSpeed` in the scripts: 36 SetProperty nodes (gait 3 written only by the two mirror copies of `Mounted_Channeling_Loop`, one node per gait 1–7 in each); besides the mirrors: 0 (8, server-only), 2/4/6 (the Clydesdale rule, and one 2 elsewhere), 7 (10, the mounted charge script); 413–473 Switch cases per gait value 0–7; one When pair (Equal 2 / NotEqual 2) in each of two `Loc_Jumping` copies (§14.6) | SCR4 (every decodable record) | Confirmed (script data) |
| MT50 | The mounted branch of `Loc_Jumping`: war-steed branch for types Equal 0x8, 0x10 or 0x20 (gait not 2; impulse at gaits 4–7, empty case for 3), Default branch for every other type (`Slider_Propel` > 0.5 and `Slider_Strafe` within ±0.25); `AirTime` 0.285, `ExpectedGravity` 15; `Emote_Idle1` on the mount when the gate fails; in 0x0700001C and 0x07002B86 (both branches) and 0x070017E4 (Default only); all 14 avatar class tables include 0x0700001C directly (§14.6) | SCR4; OBS4 (labels) | Confirmed (script data) |
| MT51 | `Mount_Movement_Jump_Modifier` (float, default 0.0) is stored or modified by no record: its ID occurs only in the dictionary, one enum mapper and two script tables (the 8 driven pairs); a driven field whose entity source has no value keeps its stored value (the dictionary default is not used) (§14.6) | OBS4 (raw search of every record of three archives); BIN3 (m7) | Confirmed; BIN |
| MT52 | Back clips per movement style: over every entity descriptor whose closure holds one of the eight mount locomotion tables (630 entities), the layer-54 clip for walking back (style `Walking`, propel −1) and running back (style `Running`, propel −1) under each family's mount type, with `Horsepants_Startup` started and `Mounted_Channeling_Loop` running; every entity of one table and type gives the same pair (§14.3) | OBS (the repository's clip resolver as a tool, 2026-10-23) | Confirmed 2026-10-23 |
| MT53 | `Mounted_Channeling_Start` (tables 0x0700001C and 0x07002B86) runs `Equipment_Hide` on the Child target at `Wpn_LOffHand` when `Inventory_ImplementTypeSecondary` is `Cosmetic` (0x10000), at `Wpn_Forearm` when it is `Shield` (8), and at `Wpn_RHand` when `Inventory_ImplementTypePrimary` is `Cosmetic` (mapper 0x23000004); its longest Timer is 0.4 s; `Equipment_Hide` is defined in four tables, in 0x07000035 as the stop of `Activate_fx` and `Cbt_Bow_Mode` and a persistent opacity entity effect (`EntityVFXScript_Type` `OpacityModify`, ramps 0.15 s); `Mounted_Channeling_End` holds two runs of `Emote_Mounted_Bow` on a Child target and one 1.5 s Timer (§14.7) | OBS (every script table, 2026-10-23) | Confirmed 2026-10-23 |

---

<a id="s1"></a>
## 1. Conventions and terms

| Term | Meaning |
|---|---|
| **Mount effect** | An effect object (prefix 0x70) of root class 2459 here, with its property set at DID + 0x09000000 (`combat-data.md` §3). Found by class at run time; a reader must not hard-code the class index (it is build-specific). |
| **Rider** | The entity that rides: the player's character, an NPC or a monster. The rider is the entity that moves; the mount is drawn as a child of it (§4). |
| **Mount entity** | The 0x47 entity descriptor named by `Mounted_Player_Cosmetic_Slot_Entity` (§3). |
| **Rider fields** | The 12 properties of §2.2 that the rider's scripts read. |
| **War-steed type** | A mount type with a bit of 0x38 (§2.8). |
| **Gait** | A key of enum mapper `MountedMovementSpeed` (0x2300033B here; `game-controls.md` §5.3.2): 1 Backwards, 2 Neutral, 3 Neutral_Forwards, 4 Walk, 5 Trot, 6 Canter, 7 Gallop. |
| **Instance set** | One entry of a landblock-info weenie list (`creatures.md` §1a). |

Property types are those of `property-system.md` §3. "PID" is a property-ID value. The short
movement-field names `ForwardMoveSpeed`, `BackwardMoveSpeed`, `StrafeLeftMoveSpeed`,
`StrafeRightMoveSpeed` and `ControlAcceleration` stand for the dictionary labels with the prefix
`GenericPhysicsController_`, as in `collision.md` §11.6. Enum and
bitfield values are given as keys with their mapper labels where a rule depends on the label;
bitfields follow the bit-to-key rule of `property-system.md` §7.3 (bit *i* stands for key
*i* + 1). Holding locations are keys of mapper 0x23000068 (`skeleton-anim.md` §8.2); the one
used here is `Mount_Connection` (0x10000027 here). Script keys are keys of mapper 0x23000097.

---

<a id="s2"></a>
## 2. The mount effect (class 2459)

<a id="s2-1"></a>
### 2.1 Census and groups (Confirmed, MT1, MT4)

722 records. Ancestry: 2459 → 734 (property-modification effect) → 698 → 486 → 89 → 485 →
484. Both halves decode completely.

| Group | Rule (data) | Records |
|---|---|---|
| Durability mounts | `Mount_DurabilityChangeEventsArray` present | 691 |
| … of which hobby-horse | `Mounted_AnimationScriptID` = `Emote_Holding_HobbyHorse` (every one has `Mount_Type` key 11 `Cosmetic_Hobbyhorse`) | 8 |
| War-steed records | `Mount_ShouldUseAppearances` present (always true) | 31 |
| … of which startup records | no `Mod_Array`, `Effect_RemoveOnDefeat` false, `Effect_EquivalenceClass` `Mounted_Monster_Startup` (key 1756 of mapper 0x23000005) | 3 |

The two top groups are disjoint and cover all 722.

<a id="s2-2"></a>
### 2.2 Rider fields (Confirmed, MT2)

These are the fields the rider's script tables read (§4). All are top-level properties.

| Property | Type | Records | Observed values | Read by (§4) |
|---|---|---|---|---|
| `Mounted_Player_Cosmetic_Slot_Entity` | DID (0x47) | 721 (all but one startup record) | 601 distinct mount entities | MeshFX `Entity` |
| `Mounted_Player_Cosmetic_Slot_HoldingLocation` | enum, mapper 0x23000068 | 722 | `Mount_Connection` in all | MeshFX `HoldingLocation` |
| `Mounted_Player_Cosmetic_Slot_OffsetRotationYaw`, `…Pitch`, `…Roll` | float | 722 each | 0.0 in all | MeshFX `OffsetRotation*` |
| `Mounted_Player_Cosmetic_Slot_BoneOffset` | vector3 | 21 | (0, 0, *z*) with *z* from −0.5 to −0.135 in 19, (0, 0.05, −0.135) and (0, 0.05, 0) in one each; all 21 are war-steed records | MeshFX `BoneOffset` |
| `Mounted_Player_Cosmetic_Slot_Scale` | vector3 | 362 | uniform: 0.925 in 328, then 0.95 (11), 1.1 (7), 1.2 (5), 0.9 (4) and seven single values from 1.02 to 1.25 | MeshFX `Scale` (only where the node drives it, §4.4) |
| `Mounted_Player_Cosmetic_Slot_Behavior` | enum, mapper 0x23000097 | 722 | `Horsepants_Startup` in all | MeshFX `Behavior` (the script started on the spawned mount) |
| `Mounted_StartScriptID` | enum, mapper 0x23000097 | 722 | `Mounted_Channeling_Start` 714, `Emote_Pickup` 8 (hobby-horse) | Run in `Mounted_Player_Cosmetic_Start` |
| `Mounted_AnimationScriptID` | enum, mapper 0x23000097 | 722 | `Mounted_Channeling_Loop` 714, `Emote_Holding_HobbyHorse` 8 | Run in `Mounted_Player_Cosmetic_Start` |
| `Dismounted_ByPlayer_Script` | enum, mapper 0x23000097 | 722 | `Mounted_Channeling_End` 714, `Emote_Putdown` 8 | Run on dismount, status 7 |
| `Dismounted_ByOther_Script` | enum, mapper 0x23000097 | 722 | `Mounted_Channeling_Failure` 714, `Emote_Putdown` 8 | Run on dismount, any other status |

<a id="s2-3"></a>
### 2.3 Identity fields (Confirmed, MT2)

| Property | Type | Records | Observed |
|---|---|---|---|
| `Mount_Name_Initial` | StringInfo | 722 | the mount's display name (a string-table reference) |
| `Mount_Type` | bitfield32, mapper 0x23000280 | 721 | §2.8 |
| `Effect_Icon` | DID (0x41) | 722 | one and the same icon in all 722 |
| `UI_Examination_Tooltip_0Click_Layout`, `…_1Click_Layout` / `…_Element` | DID (0x22) / enum | 722 each | one layout and one element in all |

<a id="s2-4"></a>
### 2.4 Durability fields (Confirmed, MT30; rules server-side)

| Property | Type | Records | Observed |
|---|---|---|---|
| `Mount_Durability_Base_Max` | int | 722 | 250 (331), 200 (250), 100 (98), 300 (31: exactly the war-steed records), 150 (8), 80 (2), 60 (2) |
| `Mount_Durability_Base_RegenRate` | float | 722 | 1.0 (691), 1.5 (31: the war-steed records) |
| `Mount_DurabilityChangeEventsArray` | array of 3 `Mount_DurabilityChangeStructure` | 691 | each struct holds `Mount_EventID` (enum, mapper 0x23000154) and `Mount_DurabilityChange` (int) |

In every one of the 691 arrays the three structs carry the event IDs 22
(`Combat_OnTargetOfHit`), 17 (`Combat_OnTargetOfCriticalHit`) and 31
(`Combat_OnTargetOf_SuperCriticalHit`), in that order. Changes are negative:

| Event | Values (records) |
|---|---|
| 22 | −30 (668), −20 (13), −12 (4), −6, −2, −40 (2 each) |
| 17 | −90 (537), −1,000 (128), −60 (20), −100 (6) |
| 31 | −180 (519), −1,000 (164), −150 (6), −120 (2) |

Commonest triple (22, 17, 31): (−30, −90, −180) in 501 records; (−30, −1,000, −1,000) in 117.

Reading (consistent with the labels; the rule is not in the data): the mount's current
durability starts at the base maximum, loses the listed amount when its rider is the target
of a hit, critical hit or super-critical hit, regenerates at the base rate, and the rider is
dismounted at 0. The actor properties `Mount_Durability_Current_Value`,
`Mount_Durability_Current_Max` (and its `_Cached` mirror) and
`Mount_Durability_Current_RegenRate` hold the live values (§6.1); the client library only
has getter functions for them (BC). **Decision (Tessera server):** apply the reading above, with
regeneration in points per second.

> **OPEN:** the original's durability rules (time unit of the regeneration, whether a change
> applies once per event, what happens at 0, whether durability applies to war-steeds, which
> have no events).

<a id="s2-5"></a>
### 2.5 War-steed fields and common effect fields (Confirmed, MT2)

War-steed only (31 records unless stated):

| Property | Type | Observed |
|---|---|---|
| `Mount_ShouldUseAppearances` | bool | true in all 31 |
| `Mount_Pip_Type` | enum, mapper 0x23000071 | 6 (`Momentum`) in all 31 |
| `Mount_MountGear_to_PipMaxModifier_Hash` | array of 7 `Mount_MountGear_to_PipMaxModifier_Pair` {`Mount_MountGear` (enum, gait mapper), `Mount_PipMaxModifier` (int)} | in all 31 the gaits 1–7 once each, with modifiers −1,000 (gaits 1, 2, 3), −750, −500, −250, 0 (gaits 4, 5, 6, 7) |
| `MeshFXScript_Appearances` | array of 6 `MeshFXScript_Appearance` {`MeshFXScript_AppearanceID` (DID 0x20), `…Key` (enum 0x2300008A), `…Modifier` (float)} | 31 records, 186 structs; 155 structs empty (DID 0), 25 name one appearance record, 6 others |
| `Effect_ForceHotspotUpdate` | bool | true in 13 |

Reading of the gear hash (consistent with the labels): the maximum of the war-steed's
momentum pips (`combat-data.md` §7.10) is lowered by the modifier of the current gait.

Fields common to all effects (`combat-data.md` §6.2) hold one value in all 722 records unless
stated: `Effect_Duration_Permanent` true, `Effect_Instantaneous` false (stored), `Effect_Harmful`,
`Effect_Debuff`, `Effect_Duration_CombatOnly`, `Effect_RemoveOnAwaken`,
`Effect_RemoveOnPulseResist`, `Effect_UIVisible` false; `Effect_SentToClient`,
`Effect_ForceRemoveOnLogout`, `Effect_IsCurable`, `Effect_ApplicationInterested` true;
`Effect_RemoveOnDefeat` true (719; false in the 3 startup records);
`Effect_EquivalenceClass` 0 (719; `Mounted_Monster_Startup` in the 3 startup records);
`Effect_ClassPriority` 1; `Effect_ConstantApplicationProbability` 1.0; resist fields empty
(category 0, level 0, chance modifier 0, three null PIDs).

<a id="s2-6"></a>
### 2.6 Modifier entries (Confirmed, MT3)

719 records have a `Mod_Array` (not the 3 startup records); 4,441 entries, all of the inline or
progression kinds of `combat-data.md` §5.2 (none empty). Operators are keys of mapper
0x23000046 (`combat-data.md` §5.3: 5 `Set`, 7 `Add`, 8 `Multiply`).

| Modified property | Op | Entries | Value | Notes |
|---|---|---|---|---|
| `ForwardSource_Movement_MountSpeedMultiplier` | Add | 719 | 0.62 (355), 0.68 (296), 0.315 (12), 0.32 (8), 0.12, 0.33, 0.45, 0.6 (4 each), 0.11, 0.13 (2 each) in durability mounts; in war-steed records 0.69, 0.77, 0.85 (4 each), 1.8, 2.0, 2.2 (5 each), 1.2 (1) | 667 entries carry `Mod_ModifierList` = [`Movement_MountSpeedBonus`] with `Mod_ModifierOp` `Set`: when the actor holds `Movement_MountSpeedBonus`, its value replaces the entry's value (`combat-data.md` §5.4) |
| `ForwardSource_Entity_Camera_LookAt_OffsetZ` | Set | 711 | 0.75 | camera look-at offset while mounted |
| `ForwardSource_Entity_Camera_LookAt_OffsetY` | Set | 711 | 0.2 | |
| `Combat_CanEvade`, `Combat_CanParry`, `Combat_CanBlock` | Set | 691 each | false | durability mounts only |
| `Mount_Movement_Max_Speed` | Add | 24 | 1.6, 1.7, 1.8 (4 each), 0.0 (12) | war-steed |
| `Mount_Movement_Acceleration` | Add | 25 | 8, 9, 10 (4 each), 16 (12), 2 (1) | war-steed |
| `Mount_Movement_Turn_Rate` | Add | 13 | 15, 20, 25 (4 each), 190 (1) | war-steed |
| `MountEndurance_MaxLevel`, `MountPower_MaxLevel` | Add (progression, 12) / Multiply 5.25 (12) / Add 3,500 or 4,000 (1) | 25 each | | war-steed vitals (§6.4) |
| `Vital_MountEndurance…RegenAddMod`, `Vital_MountPower…RegenAddMod` (peace, combat) | Add (progression, 12, or 5–12, 1) | 13 each | | |
| `Stat_MountStrength`, `Stat_MountAgility` | Add (progression, 12; 500 or 300, 12; 500, 1) | 25 each | int | |
| `Combat_Agent_MountArmor_Value_Float` | Add (progression or 300) | 13 | | |

Progression-driven entries name class-3918 progressions (`combat-data.md` §4.4), 27 distinct.

**Groups of war-steed records by their entries:** 12 records with 15 entries (speed +0.69 to
+0.85, acceleration +8 to +10, turn rate +15 to +25, maximum speed +1.6 to +1.8, nine
progression-driven stats and vitals, two camera entries); 12 records with 9 entries (speed
+1.8 to +2.2, maximum speed +0, acceleration +16, endurance and power maxima × 5.25,
strength +500, agility +300, two camera entries); 1 record with 14 entries; 3 records with the
speed and camera entries only; 3 startup records without entries.

<a id="s2-7"></a>
### 2.7 The compiled half (Confirmed, MT5)

The 722 object states hold 23,094 items in all (script objects, native values, string
references). The class library gives class 2459 one attribute more than class 734 (30 slots
instead of 29). In all
722 records that attribute (slot 29) references one object of class 2464, which holds:

| Slot | Content (all 722 unless stated) |
|---|---|
| 5 | a property set with exactly the rider fields the record has: the four script IDs, the behaviour, the holding location, the three rotation offsets, and the entity, scale and bone offset where present |
| 0 | a property set with `Mount_Durability_Base_Max`, `Mount_Durability_Base_RegenRate`, `Mount_Name_Initial`, and `Mount_Type` and `Mount_Pip_Type` where present |
| 1 | a native object (all 722) |
| 2 | an integer: 1 in the 31 war-steed records, 0 otherwise |
| 3, 4 | an integer and a native object set only in the 31 war-steed records (meaning OPEN) |

So the compiled half separates what the **rider** receives (slot 5) from what describes the
**mount** (slot 0). The class library reads `Mount_Type`, the durability base values and the
name from slot 0 by constant PID (BC). A reader never needs the compiled half to obtain a
value; the property half carries every field.

<a id="s2-8"></a>
### 2.8 Mount types (Confirmed, MT6, MT7)

`Mount_Type` (effects, templates, instance sets), `Mount_CurrentMountType` (actors, mount
entities, instance sets), `Mount_LastUsedMountType`, `Mount_Volume_Allowed_Mount_Types`,
`Area_Allowed_Mount_Types` and `Dungeon_Allowed_Mount_Types` are bitfields of the same mapper
(`MountType`, 0x23000280 here). Key *k* ≥ 1 is bit *k* − 1, so its value is 2^(*k*−1).

| Key | Label | Value | Mount effects with exactly this type |
|---|---|---|---|
| 1 | `Horse` | 0x1 | 590 |
| 2 | `Caveclaw` | 0x2 | 0 |
| 3 | `Goat` | 0x4 | 72 |
| 4 | `WarsteedHeavy` | 0x8 | 8 |
| 5 | `WarsteedLight` | 0x10 | 8 |
| 6 | `WarsteedMedium` | 0x20 | 11 |
| 7 | `Warg` | 0x40 | 0 |
| 8, 9, 10 | `Warg_B`, `Warg_Mordor`, `Warg_Whitehand` | 0x80, 0x100, 0x200 | 0 |
| 11 | `Cosmetic_Hobbyhorse` | 0x400 | 8 |
| 12 | `Mearh` | 0x800 | 0 |
| 13 | `Elk` | 0x1000 | 0 |
| 14 | `Donkey` | 0x2000 | 0 |
| 15 | `Boar` | 0x4000 | 9 |
| 16 | `Clydesdale` | 0x8000 | 4 |
| 17 | `Oryx` | 0x10000 | 2 |
| 18 | `Camel` | 0x20000 | 6 |

Three records combine two bits (0x9, 0x11, 0x21: Horse with one war-steed bit); one record
has no `Mount_Type`. Key 0 `Undef` is the empty bitfield.

**War-steed rule** (MT7). A mount type is a war-steed type when it has any bit of **0x38**
(keys 4, 5, 6). The client library's mount system (§4.7) and native code (BIN m5) test this
mask; the rider scripts switch on the cases 0x8, 0x10, 0x20 (some Switches add 0x40).

---

<a id="s3"></a>
## 3. Mount entities

<a id="s3-1"></a>
### 3.1 Resolution (Confirmed, MT8)

`Mounted_Player_Cosmetic_Slot_Entity` names a 0x47 entity descriptor (`setup.md` §4). 607
distinct entities are named (601 by mount effects, 19 by templates, 13 by both). Every one
exists, decodes and resolves to a setup through the chain of `setup.md` (0x47 → parent 0x47
or 0x1F template → 0x04 setup). They resolve to **13 setups**; two of them carry 509 of the
607 entities (the entities differ by their appearance lists, `appearance.md` §3.2). Every one of
the 13 setups has a skeleton (a Havok tagfile). Their parts: 10 (509 entities), 2 (61), 7
(27), and 1, 3 or 5 for the rest. Only one of the 13 has the holding location
`Mount_Connection`: setup 0x0400016C, reached only through entity 0x47005DA8, which only
templates name (101 monster and 2 NPC templates; reading: a combined rider rig). The common
horse setups carry effect, body and sheath locations only.

<a id="s3-2"></a>
### 3.2 Mount-family properties of entities (Confirmed)

| Property | Entities | Observed |
|---|---|---|
| `Mount_CurrentMountType` | 611 | key values as in §2.8: 0x1 (484), 0x4 (52), 0x1000 (22), 0x10 (19), 0x8 (8), 0x20 (6), 0x8000 (6), 0x4000 (5), 0x40 (4), 0x400 (4), 0x2000 (1) |
| `Mount_RiderType` | 83 | 2 `Avatar` (65), 3 `Npc_Facing` (16), 1 `Npc_Angled` (2) (§7.2) |
| `Mount_MountAppearance`, `Mount_Type` | 18 each | |
| `Mount_CurrentMountStatus` | 14 | 5 `Mounted` (12), 0 (2) |
| `Mount_Is_Mounted`, `MountMovement_CurrentSpeed`, `Mount_IsWarPony`, `Movement_MountAnimationSpeedMultiplier`, `Mount_MountBannerAppearance`, `Mount_Jester_Jingle` | 6, 6, 3, 3, 2, 2 | |

Over the 721 naming effects, the entity's `Mount_CurrentMountType` is found on the entity
itself for 681, on a parent 0x47 along the chain for 31, and nowhere for 9. Compared with the
naming effect's `Mount_Type`: equal in 676 of 721 effects; different in 45 (22 effects say `Horse` where the
entity says `Elk`; the others mix war-steed bits or lack a value).

> **OPEN:** which of the two values the original copies to the rider's
> `Mount_CurrentMountType` when it mounts (the rider scripts of §4 switch on the rider's
> value). Reading: the effect's `Mount_Type`, because the compiled half hands it to the
> mount system (§2.7); not confirmed.

<a id="s3-3"></a>
### 3.3 The rider's `Mount_Connection` (Confirmed, MT9)

34 setups carry the holding location `Mount_Connection`. In 33 it is on **bone 0** with
identity rotation, unit scale and a position of (0, *y*, *z*) with *y* between −0.033 and
+0.353 m and *z* between −0.508 and −2.661 m (the player setups 0x04000551–0x04000557,
0x04005EC4/5 and 0x0400772A/B among them: *z* −0.508 to −1.059). One setup has it on bone 11
with a rotation. On the player skeletons the location cancels bone 0's reference height, so in
the reference pose it composes to the model origin (`skeleton-anim.md` §5.1).

---

<a id="s4"></a>
## 4. How the client presents a mounted rider

The presentation is **data**: script nodes in the rider's and the mount's script tables,
driven by properties of the rider. An implementation that runs the script tables
(`script-runtime.md`) and implements the MeshFX node kind gets the original's behaviour
from the data alone. This section gives the chain and the facts the data fixes.

<a id="s4-1"></a>
### 4.1 From skill to rider properties (Confirmed, MT10; transfer OPEN)

1. **Skill.** Each of the 676 skills of the MountDirectory (`game-controls.md` §5.1) holds
   `Skill_Toggle_Effect_List` with exactly one `Skill_Toggle_Effect_Data`, whose
   `Skill_Toggle_Effect` names one mount effect. 12 further skills do the same (9 name
   war-steed records). 685 distinct effects are reached this way (3 effects by two skills
   each). Every mount skill has `Skill_AllowedWhileMounted` false; 660 of the 688 carry
   `Skill_MountRacialConversionAnalog` (a skill DID; reading: the counterpart for the other
   body size, `game-controls.md` §5.1).
2. **Other referrers.** 28 items reach 22 mount effects through
   `EffectGenerator_UsageEffectList`; 418 templates start one of two mount effects through
   their startup-effect arrays (§6.6); one dispel effect, one hotspot and one landblock drama
   name a mount effect. 16 mount effects are named by no property set of
   `client_gamelogic.dat`.
3. **Application.** Toggling the skill applies the effect to the user. The effect carries the
   rider fields (§2.2); the rider's scripts read them **as properties of the rider**
   (§4.2). How they reach the rider is not in the client: native code reads none of them by
   constant ID (MT13), and the client library copies them only into the class-2464 object
   (§2.7). Reading: the server sets them on the rider while the effect is applied, together
   with `Mount_CurrentMountType` and `Mount_Cache_MountEffectDID`/`…ID` (actor properties the
   mount system reads, BC).

> **OPEN:** how the original transfers the rider fields to the rider and starts
> `Mounted_Player_Cosmetic_Start` (§4.3) for a normal mount. No node of the player's tables
> runs it except the hobby-horse branch of `Horsepants_Startup`; no constant equal to its key
> exists in the executable (BIN m1) or the class library (BC). It is likely a server-driven
> script start (decision 0004). **Decision (Tessera):** when a mount effect is applied to a
> rider, the client sets the rider fields, `Mount_CurrentMountType` = the effect's
> `Mount_Type` and `Mount_CurrentMountStatus` 0 on the rider, then starts
> `Mounted_Player_Cosmetic_Start` on it; when the effect is removed it sets
> `Mount_CurrentMountStatus` to 7 (dismissed by the player) or 6 (any other cause) and stops
> that script (its OnStop branch then runs the dismount script, §4.3).

> **Answered for Tessera (2026-10-11, P5; MQ4):** the original's path stays server-side, but
> the step it leaves out is now known: the start script is a persistent Run in step 2 of the
> Sequence, which by `script-runtime.md` §12.4 never completes (MT44), so the walker itself
> starts step 4's scripts after the start script's graph has completed. The whole timeline (summon, mounting, mounted,
> dismount) is the Decision of §14.7.

<a id="s4-2"></a>
### 4.2 Rider properties read by script nodes (Confirmed (script data), MT11, MT12)

Census over all script tables and scriptlets of the install:

| Node kind | Field | Driven by (rider property) | Nodes |
|---|---|---|---|
| MeshFX (41) | `MeshFXScript_Entity` | `Mounted_Player_Cosmetic_Slot_Entity` | 35 |
| MeshFX | `MeshFXScript_Behavior` | `Mounted_Player_Cosmetic_Slot_Behavior` | 35 |
| MeshFX | `MeshFXScript_HoldingLocation` | `Mounted_Player_Cosmetic_Slot_HoldingLocation` | 34 |
| MeshFX | `MeshFXScript_OffsetRotationPitch`, `…Roll`, `…Yaw` | the matching `Mounted_Player_Cosmetic_Slot_OffsetRotation*` | 34, 34, 33 |
| MeshFX | `MeshFXScript_Scale` | `Mounted_Player_Cosmetic_Slot_Scale` | 18 |
| MeshFX | `MeshFXScript_BoneOffset` | `Mounted_Player_Cosmetic_Slot_BoneOffset` | 10 |
| MeshFX | `MeshFXScript_Appearances` | `Mount_Appearances` (war-steed appearance list, §4.4) | 10 |
| MeshFX | `MeshFXScript_FadeInTime` | `Mount_FadeIn_Override` | 10 |
| Run (5) | `RunScript_ID` | `Mounted_AnimationScriptID` / `Mounted_StartScriptID` / `Dismounted_ByPlayer_Script` / `Dismounted_ByOther_Script` | 10 / 5 / 8 / 8 |
| Animation (36) | `AnimationScript_Speed` | `Movement_MountAnimationSpeedMultiplier` | 633 |
| PhysicsImpulse (9) | `PhysicsImpulseScript_AirTime` | `Mount_Movement_Jump_Modifier` | 8 (`collision.md` §11.10) |
| SetProperty (40) | the property itself | `Mount_CurrentMountType`, `MountMovement_CurrentSpeed`, `Mount_MountBannerAppearance` (1 each) | mirrors to another entity (§4.5) |

Switch and When nodes watching mount-family properties: `Mount_CurrentMountStatus` 2,697,
`Mount_CurrentMountType` 1,567, `Mount_Type` 655, `MountMovement_CurrentSpeed` 479 + 3 When,
`ScriptSystem_MountedCombat_WarsteedState` 243, `Mount_Is_Mounted` 203, `Mount_IsWarPony` 69,
`Mount_RiderType` 46, `Script_ViewMountMeshFX` 25, `Script_IsMounted` 22,
`Mount_MountBannerAppearance` 11, `Mount_LastUsedMountType` 10, `Mount_MountAppearance` 6,
`Mount_Movement_Use_RampUp_Controller` 4, `Mount_MountBannerAnimation` 2, and single ones; 19
When nodes test `Mounted_Player_Cosmetic_Slot_Entity` against the null DID.

<a id="s4-3"></a>
### 4.3 The rider's mount scripts (Confirmed (script data), MT12, MT15)

Script keys (mapper 0x23000097; labels are interoperability identifiers) and the tables that
define them in this install:

| Script | Defined in | Role |
|---|---|---|
| `Mounted_Player_Cosmetic_Start` | 0x0700001C, 0x07002B86 (the player locomotion tables of `collision.md` §11.10) | entry point: starts the mount scripts and the mesh (below) |
| `Mounted_Player_Cosmetic_Mesh` | 7 tables (0x07000024, 0x07001693, 0x070018C7, 0x070018D1, 0x070018D2, 0x070019CB, 0x070021E2) | spawns the mount mesh (§4.4) |
| `Mounted_Channeling_Start`, `…_Loop`, `…_End`, `…_Failure` | the two player tables; `…_Loop` also in 2 more tables, `…_End` in 11 | the values of the rider fields (§2.2) |
| `Horsepants_Startup` | 91 tables (player, avatar-class, NPC and mount tables) | the MeshFX `Behavior` value: started on the spawned mount; also a startup branch in rider tables |
| `Mounted_Player_Cosmetic_End` | none | run by the start script's stop branch; no definition exists, so the run does nothing |

**`Mounted_Player_Cosmetic_Start`** (table 0x0700001C; the graph in 0x07002B86 is identical):

1. A Switch on `Script_RunInViewport`: when true (the character is shown in a user-interface
   viewport), run `Mounted_Player_Cosmetic_Mesh`, `Mounted_Warsteed_Manager` and
   `Mounted_Channeling_Loop` and nothing else.
2. Otherwise a Switch on `WeenieType`: for the labels `Player` and `Admin` a Collection
   starts, together:
   - a Sequence: step 2 runs the script named by `Mounted_StartScriptID` (persistent);
     step 4, when step 2 has finished, runs the script named by `Mounted_AnimationScriptID`
     and `Mounted_Player_Cosmetic_Mesh`;
   - an OnStop branch that, when the start script stops, runs in parallel:
     `Mounted_Player_Cosmetic_End`; a stop of the script named by
     `Mounted_AnimationScriptID`; a game callback; and a Switch on `Mount_CurrentMountStatus`
     that runs `Dismounted_ByPlayer_Script` for 7 (`Dismounted_ByPlayer`) and
     `Dismounted_ByOther_Script` for any other value; plus, out of combat, a Switch on the
     movement keys that runs one of two scripts on the child entity (Child target).
3. For other weenie types the same Sequence and OnStop branch run without the combat and
   emote parts.

The labels of `Mount_CurrentMountStatus` (mapper 0x230002C9): 0 `Undef`, 5 `Mounted`,
6 `Dismounted_ByOther`, 7 `Dismounted_ByPlayer`, 8 `Dismounted_ByQuest`.

**`Horsepants_Startup` in the two player tables (identical):** a Switch on `Mount_CurrentMountType`: for
0x400 (hobby-horse) run `Emote_Holding_HobbyHorse` and `Mounted_Player_Cosmetic_Start`
(both persistent); for any other type run `Mounted_Channeling_Loop` (persistent). In
parallel a Sequence sets `Script_IsMounted` true, then runs `Visibility_Controller`.

<a id="s4-4"></a>
### 4.4 The mount mesh: MeshFX nodes (Confirmed (script data), MT11)

`Mounted_Player_Cosmetic_Mesh` in table 0x07000024 is one Switch on the rider's
`Mount_CurrentMountType` with cases 0x8/0x10/0x20 (war-steeds), 0x400 (hobby-horse), 0x4
(goat), 0x4000 (boar), 0x8000 (Clydesdale), 0x20000 (camel) and a default. Each case holds a
When node that continues only when `Mounted_Player_Cosmetic_Slot_Entity` is not the null DID,
then (except for the hobby-horse) a Switch on `Agent_Species` (and once on `Sex`) whose ports
each hold **one MeshFX node** (23 nodes). Four other tables (0x07001693, 0x070018C7,
0x070018D1, 0x070018D2) hold three nodes each, for the cases war-steed, 0x40 (warg) and
default, without species branches (35 nodes in all); the two remaining tables that define the
script hold no entity-driven MeshFX node. Three further MeshFX nodes write `Mount_MeshFX_Entity`
with a stored, undriven entity (38 in all, MT31).

MeshFX layout: `script-table.md` §5.2 (field labels `MeshFXScript_*`). The stored values of
the mount nodes:

| Field | Stored value in the mount nodes | Driven? |
|---|---|---|
| `Entity` | 0 | always driven by the rider's entity field |
| `AttachToParent` | 1 | no |
| `AllowPhysics` | 0 | no |
| `FadeInTime`, `LifeTime`, `FadeOutTime`, `DestroyTime` | 0.25 s each | fade-in by `Mount_FadeIn_Override` in 10 nodes |
| `HoldingLocation` | 0, except `Wpn_LHand` in the hobby-horse node | driven by the rider's holding location in 34 nodes; the hobby-horse node is not driven, so the hobby-horse is held in the left hand |
| `BoneOffset` | (0, 0, 0), except (0, 0, −0.3), (0, 0, −0.32) and (0, 0, −0.5) in the Clydesdale nodes | driven in the 10 nodes of the war-steed and warg (0x40) cases |
| `OffsetRotationYaw`, `…Pitch`, `…Roll`, `RemoveRotation` | 0 | rotations driven in 33–34 nodes |
| `BoneType` | 0 | no |
| `Behavior` | 0 | always driven (`Horsepants_Startup`) |
| `Persistent`, `OnWater`, `UseParentScale`, `UseParticleScale`, `IgnoreChildScale` | `Persistent` 1 and `OnWater` 0 in all 35; the last three (1, 0, 1) in 14 nodes, (1, 1, 0) in 16, (1, 1, 1) in 4, (0, 1, 1) in 1 | no |
| `Scale` | per node: 1.0 in the default branches; by species in the goat, boar, Clydesdale and camel branches (from 0.7 to 1.2) | driven in 18 nodes only |
| `Appearances` | empty | driven by `Mount_Appearances` in the same 10 nodes |
| `MeshIIDTargetProperty` | `Mount_MeshFX_Entity` in all | |

Consequences an implementation must keep:

- Where a node does **not** drive `Scale`, the effect's `Mounted_Player_Cosmetic_Slot_Scale`
  is not used: the node's stored scale applies. Which node runs depends on the rider's
  `Mount_CurrentMountType`, `Agent_Species` and `Sex`, so the same mount effect can be drawn at
  different scales for different species (worked example §11.4).
- The spawned mount entity's instance ID is written to the rider's `Mount_MeshFX_Entity`
  (`MeshIIDTargetProperty`).
- `Behavior` (`Horsepants_Startup`) is the script started on the spawned entity, looked up
  through the **mount entity's** tables.

> **OPEN:** the MeshFX node's run-time behaviour is not specified anywhere
> (`script-runtime.md` OPEN-17 lists MeshFX as "not traced"): how `BoneOffset`, the three
> rotation offsets (unit and order), `RemoveRotation`, `Scale` with `UseParentScale` and
> `IgnoreChildScale`, and the fade times combine with the holding-location frame. For the
> mount nodes the rotation offsets are 0 in all data, so only the bone offset and the scale
> matter. **Decision (Tessera):** the mount's model transform is the rider's model transform ×
> the rider's bone transform of the location × the location frame (`skeleton-anim.md` §8.2, as
> for held items) × a translation by `BoneOffset` (metres, in the location's frame) × a
> **per-axis** scale by the node's `Scale` vector, component by component (*x*, *y*, *z* of
> the mount's model space), times the rider's final model scale, also per axis
> (`appearance.md` §6.1), when `UseParentScale` is 1. (Revised 2026-10-09: was "a uniform
> scale".)

Why per axis (MT31, Confirmed (script data)): `MeshFXScript_Scale` and the property that drives
it, `Mounted_Player_Cosmetic_Slot_Scale`, are both vector3 properties in the dictionary;
875 MeshFX nodes outside the mounts store unequal components, which only a per-axis scale can
use; and the object render scale they end up in is per axis (`appearance.md` §6.1, own scale
×  holder's final scale, component by component). For the mounts the question has no effect in
this install: all 38 MeshFX nodes that write `Mount_MeshFX_Entity` store equal components,
and all 397 `Mounted_Player_Cosmetic_Slot_Scale` values in the property sets of
`client_gamelogic.dat` (effects and templates, §2.2, §6.6; none in a landblock instance set) are
uniform.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-09):** no component is chosen: the
> three components scale the three axes (Tessera's choice is the Decision above). How the
> original applies the node's scale at run time is still part of the MeshFX OPEN above (MQ5).

> **Partly answered (2026-10-11, P5; MQ5):** the translation part of the Decision now rests on
> data: every race's war-steed effect stores a `BoneOffset` that cancels, along the location's
> *z*, the rise of bone 0 in its riders' layer-75 offset clips (MT39, §14.8). The rotation
> offsets, `RemoveRotation` and the fade semantics stay open; the walker's per-frame placement
> and fade are §14.8.

<a id="s4-5"></a>
### 4.5 Rider and mount talk through targets (Confirmed (script data), MT14)

- Nodes in the **rider's** scripts act on the mount through the Child target (target type
  14, `script-table.md` §4.4) with `ChildTarget_HoldingLocation` = `Mount_Connection`: the
  entity attached to the rider at that location. In `Mounted_Channeling_Loop` (0x0700001C)
  seven such nodes set `MountMovement_CurrentSpeed` on the mount (one per gait) and two run
  the mount's war-steed locomotion transition.
- Nodes in the **mount's** scripts read the rider through the Property target (type 3) with
  `PropertyTarget_PropertyName` = `MeshFXScriptOutput_Source` (an instance-ID property of
  the spawned entity naming its source): for example the mount table 0x070003A4 watches the
  rider's `Mount_MountAppearance` and applies Appearance nodes (kind 58: appearance record,
  key, modifier) to itself (§7.4), and the mount tables choose rider-pose scriptlets by the
  rider's `Mount_RiderType`.

<a id="s4-6"></a>
### 4.6 State set by the scripts (Confirmed (script data), MT15)

`Mounted_Channeling_Loop` in table 0x0700001C (the variant in 0x07002B86 resets the status
locally, without the server-only flag, and lacks the fidget and Clydesdale branches):

1. When it starts: a Sequence whose first step is a Timer (duration 0) and whose second step
   sets `Mount_Is_Mounted` true and `Mount_CurrentMountStatus` 5 (`Mounted`; with
   `SetPropertyScript_ServerOnly`, so the client does not set it locally,
   `script-table.md` §7.7); and `Script_BlockRandomEmotes` true.
2. When it stops (OnStop): the same Timer-then-set Sequence sets `Mount_Is_Mounted` false and
   `Mount_CurrentMountStatus` 0 (server only); `Script_IsMounted` false (then
   `Visibility_Controller` runs), `Script_BlockRandomEmotes` false; it stops
   `Mounted_Warsteed_Manager` and `Equipment_Hide` (Child target).
3. For war-steed types (0x8, 0x10, 0x20, 0x40): runs `Mounted_Warsteed_Manager`; mirrors each
   change of the rider's `MountMovement_CurrentSpeed` to the mount (Child target); on the
   transitions Neutral → Walk and Undef → Walk runs the idle-to-walk transition on rider and
   mount; sets `Script_AnimationLayerOverride` to 52 for 0x8/0x10/0x20 and to 30 for 0x40.
4. For the Clydesdale (0x8000): sets `MountMovement_CurrentSpeed` to 4 (`Walk`) while
   `Movement_MovementStyle` is 1 (`Walking`), else to 6 (`Canter`) while `Slider_Propel` is
   not 0 and to 2 (`Neutral`) when it is 0.
5. A WhenRunning on itself runs `Mounted_Channeling_Resync` every 2 s for players and
   administrators (with extra resyncs 0.05 s after a landing or a change of
   `Entity_Element` while moving, for non-war-steed types), and at random intervals of 4, 5,
   9 or 12 s for monsters.

`Mount_Is_Mounted` is what `collision.md` §11.1 step 3 tests to choose the Mounted layer, and
`Mounted_Channeling_Loop` is what the jump script tests to choose the mounted branches
(`collision.md` §11.10).

<a id="s4-7"></a>
### 4.7 The client library's mount system (Confirmed (bytecode) + BIN, MT16, MT17)

Two classes of the class library (2469 derived from 1358 here) hold the mount system.

**Controller flags.** When the actor's `Mount_CurrentMountType` is set or changes, the mount
system sets the boolean properties `Mount_Movement_Use_RampUp_Controller` and
`Movement_ServerAuthoritative_Controller` on the actor, both to "the type has a bit of 0x38"
(BIN m4: the native function used sets a boolean property to "value ≠ 0"). So every war-steed
type selects the Warsteed layer and the Mounted controller kind of `collision.md` §11.1, and
makes the controller server-authoritative; every other type clears both. On removal of the
property a third handler calls a property native on the same two properties (reading:
removes them; not traced).

**Applicability of a mount effect** (the effect's application check, class 2459):

1. The inherited effect check runs first; a failure is returned as is.
2. The target must be a player or an AI agent (two predicates of the base class); otherwise
   the effect is refused.
3. **The target's own mount fields win.** Before the type is read, the effect's
   mount-information object (§2.7) takes the target's own values of
   `Mounted_Player_Cosmetic_Slot_Entity`, `…_Scale` and `…_BoneOffset` (into the rider set) and
   of `Mount_Type` (into the mount set), each only when the target holds that property; a
   property the target lacks leaves the effect's value. The mount type *T* is then the
   `Mount_Type` of the mount set (0 when absent). This is how the generic startup effect of
   NPC and monster riders, which has neither entity nor type, obtains them from the rider's
   template (§6.6).
4. The target is refused (result A) when its `Entity_Element` is 1, 2 or 3 (in water,
   `collision.md` §10.4).
5. Two cases, chosen by a native query on the target's position (package 161, not read):
   - **outdoors:** the area is the target's `Area_CurrentArea` (refused, A, when 0);
   - **in a cell:** the area is the cell's `Area_DID` (refused, A, when absent), checked
     first; then, when the cell has a `Dungeon_DID`, the dungeon object's
     `Dungeon_Allowed_Mount_Types` is checked too (refused, A, when the dungeon lacks the
     property).
   The area check reads the area object's `Area_Allowed_Mount_Types` (refused, A, when the area
   lacks it).
6. **Mask check** for an allowed mask *A* and the type *T*: refused (A) when *T* is 0;
   refused (result B) when *A* AND *T* is 0; allowed otherwise.

A second check, a method of the mount system whose caller was not traced, first refuses an
actor that is already mounted (with its own code), then a shapeshifted actor
(`Shapeshifting_Target_Entity` non-zero; another code), then applies the mask check to the
actor's `Mount_Volume_Allowed_Mount_Types` when it has that property (a volume restriction;
no restriction when absent), then steps 4–6. Results A and B are two distinct non-zero codes
of the code block.

Data: all 675 area objects (class 690) hold `Area_Allowed_Mount_Types`: 0x3C47D in 518, 0 in
82 (no mount type allowed), 0x4004 in 44, six other masks in 31. All 1,303 dungeon objects
(class 691) hold `Dungeon_Allowed_Mount_Types`: 0 in 1,298 (no mount in those dungeons),
other masks in 5.

<a id="s4-8"></a>
### 4.8 What native code does not read (BIN, MT13)

The 4-byte values of the PIDs of `Mounted_Player_Cosmetic_Slot_Entity`, `…_Scale`,
`…_BoneOffset`, `…_HoldingLocation`, `…_Behavior`, `…_OffsetRotationYaw`, `…_OffsetRotationPitch`,
`Mounted_AnimationScriptID`, `Mounted_StartScriptID`, `Dismounted_ByPlayer_Script` and
`Dismounted_ByOther_Script` occur nowhere in the executable; the one occurrence of the roll
field's PID is a user-interface element ID in a panel function. Nor do the keys of
`Mounted_Player_Cosmetic_Start`, `Mounted_Player_Cosmetic_Mesh` and `Mounted_Channeling_Failure`.
The presentation of §4.3–§4.6 is therefore entirely data plus the generic script machinery.

<a id="s4-9"></a>
### 4.9 Walker procedure (Decision, built on the facts above)

To mount the player's character in the offline walker:

1. Pick a mount effect (from a MountDirectory skill of the character's body size,
   `game-controls.md` §5.1, or directly) and apply the Decision of §4.1.
2. Run the script machinery: `Mounted_Player_Cosmetic_Start` starts the channelling scripts
   and `Mounted_Player_Cosmetic_Mesh`, whose MeshFX node spawns the mount entity at the rider's
   `Mount_Connection` (§4.4 Decision), starts `Horsepants_Startup` on it, and records its IID
   in `Mount_MeshFX_Entity`.
3. `Mounted_Channeling_Loop` sets `Mount_Is_Mounted` (and, because Tessera is its own server,
   Tessera also applies the server-only status 5), which selects the Mounted layer
   (`collision.md` §11.1).
4. Movement as in §5. To dismount, apply the removal Decision of §4.1.

Without a MeshFX implementation, a walker may draw the mount entity directly with the §4.4
Decision transform and the stored scale of the node that the Switch would select.

The P5 revision refines this procedure: clips of rider and mount §14.2–§14.5, gait §14.6,
sequence and timing §14.7, placement, seat and fade §14.8, camera §14.9.

---

<a id="s5"></a>
## 5. Mounted movement

<a id="s5-1"></a>
### 5.1 Normal mounts (Confirmed; combination OPEN; MT18)

A mount type without a war-steed bit leaves `Mount_Movement_Use_RampUp_Controller` false
(§4.7), so the rider keeps the **Generic** controller of `collision.md` §11.6 with the
**Mounted** layer (or MountedWalking) of `collision.md` §11.1 and §10.3, and turns at the
on-foot turning speed (`collision.md` §11.9). Nothing in the client reads a mount-specific
speed for this controller: its only speed factor is the mover's `Movement_SpeedMultiplier`
(`collision.md` §11.6).

The mount effect's speed entry raises `ForwardSource_Movement_MountSpeedMultiplier` (§2.6);
its PropertyMetaData forwards it to `Movement_Effect_MountSpeedMultiplier` (§6.2), whose
template value is 1.0. With the commonest entry the forwarded value is 1.0 + 0.62 = 1.62. No
client code combines it with `Movement_SpeedMultiplier`: the client library does not name it
(BC), and the three native readers build a user-interface speed label (BIN).

> **OPEN:** how the original turns `Movement_Effect_MountSpeedMultiplier` into the rider's
> movement speed (reading: the server includes it in `Movement_SpeedMultiplier`).
> **Decision (Tessera):** while a rider is mounted on a type without a war-steed bit, the
> controller's *m* of `collision.md` §11.6 is `Movement_SpeedMultiplier` ×
> `Movement_Effect_MountSpeedMultiplier` (each 1 when absent). Worked example §11.5.

<a id="s5-2"></a>
### 5.2 The war-steed controller: when it applies (BIN, SPEC)

For a war-steed type, `Mount_Movement_Use_RampUp_Controller` is true (§4.7), so
`collision.md` §11.1 selects the Warsteed layer and the controller kind Mounted
(PHYSICSCONTROLLER key 0x10000001). That controller is a separate class in the client
(§5.3–§5.8). It uses the same input pair (*x*, *y*) as the player controller
(`collision.md` §11.8), the same 0.0002 thresholds, and the gait multipliers of
`game-controls.md` §5.3.2.

<a id="s5-3"></a>
### 5.3 Parameter block (BIN, MT19)

At each use the controller builds a parameter block from the layered movement parameters of
`collision.md` §11.1 and the actor's properties:

| Slot | Initial value (layered parameters) | Replaced by (when the actor has it as a float) | Then multiplied by |
|---|---|---|---|
| *S*1, *S*2 | the two strafe speeds of the layered parameters (*S*2 is used for *x* > 0, *S*1 otherwise) | — | — |
| *F* (forward) | `ForwardMoveSpeed` | `Mount_Movement_Max_Speed_Cached` | `Movement_SpeedMultiplier` (1 when absent) |
| *B* (backward) | `BackwardMoveSpeed` | — | `Movement_SpeedMultiplier` |
| *a*x, *a*y, *a*z | `ControlAcceleration` (sideways, forward, vertical) | *a*y by `Mount_Movement_Acceleration_Cached` | `Movement_AccelerationMultiplier` (1 when absent), all three |
| *R* (turn rate) | **150** | `Mount_Movement_Turn_Rate_Cached`; a negative value becomes 0 | — |
| *G* (gear-change rate) | **2.0** | `Mount_Movement_Gear_Change_Rate_Cached` | — |

The 150 and 2.0 are constants of the client (decision 0012). The replacements and
multipliers are read from the code. The pairing of the block's slots with the property names
of the layered parameters is BIN (MT32, revised 2026-10-09): the client registers each field of
the layered movement-parameter object with its property ID and an accessor, and the war-steed
block copies *S*1 from the field registered as `StrafeLeftMoveSpeed`, *S*2 from the field
registered as `StrafeRightMoveSpeed`, *F* from `ForwardMoveSpeed`, *B* from
`BackwardMoveSpeed` and the three *a* values from `ControlAcceleration`. So a positive sideways
input *x* (strafing right, `collision.md` §11.8 item 5) targets `StrafeRightMoveSpeed`, a
negative or zero one `StrafeLeftMoveSpeed`, and the sideways limits of §5.4 are
−`StrafeLeftMoveSpeed` … +`StrafeRightMoveSpeed`. Every observed layer has equal strafe speeds,
so the pairing has no effect in this install.

With the template values of §6.1 (`Mount_Movement_Max_Speed` 6.5, `…_Acceleration` 0,
`…_Turn_Rate` 100, `…_Gear_Change_Rate` 2.0, mirrored in their `_Cached` forms) a rider without
war-steed bonuses has *F* = 6.5, *a*y = 0 and *R* = 100. The war-steed effects add to the base
properties (§2.6); the `_Cached` forms are the forwarded results (§6.2).

> **Answered (2026-10-09; was OPEN, implementer 2026-10-09):** *S*1 is the layered
> `StrafeLeftMoveSpeed` and *S*2 the layered `StrafeRightMoveSpeed` (BIN2, m6); Tessera's
> choice matches the client.

<a id="s5-4"></a>
### 5.4 Target velocity and integration (BIN, MT20)

Let *g* be the actor's `MountMovement_CurrentSpeed` (2 `Neutral` when absent), and *L* the
forward limit of §5.5 (without the locked gear).

**Target** (horizontal, in the mover's frame; zero when the actor has no physics object):

1. Sideways: when *g* is 2 (Neutral), *x* × *S*2 for *x* > 0 and *x* × *S*1 otherwise; for any
   other gait, 0 (a moving war-steed does not strafe).
2. Forward, when |*y*| > 0.0002: for *y* > 0 the target is *L* itself, whatever the size of
   *y*; for *y* < 0 it is *y* × *B*.
3. Forward, when |*y*| ≤ 0.0002: if the actor's boolean
   `GOP_MountedMovement_ReleasedKeys_LoseMomentum` is not true and *g* > 3 (a forward gait),
   the target keeps the current forward control velocity (momentum); otherwise 0.

**Integration** for an update of *t* seconds, with *v* the stored control velocity:

1. On each horizontal axis, *v* moves toward the target by at most *a*x·*t* sideways and
   *a*y·*t* forward (the change Δ is limited to ±that amount); the vertical part is 0.
2. The update's mean control velocity is the old *v* plus Δ/2; the stored *v* becomes old *v*
   plus Δ.
3. Both the mean and the stored velocity are then limited: sideways to −*S*1 … *S*2, forward
   to −*B* … *L*′, where *L*′ is *L* of §5.5, or, when the actor has
   `MountedCombat_Current_LockedGear`, the speed multiplier of that gear
   (`game-controls.md` §5.3.2 step 1) × *F*.

The displacement, physics velocity and contacts then follow the player controller's steps
(`collision.md` §11.6 items 3–7) (Inferred: the shared base class was not followed in full).

<a id="s5-5"></a>
### 5.5 Forward limit (BIN + Confirmed, MT20, MT21)

*L* = min(*W*, *M*Gallop × *F*), where *M*Gallop is the speed multiplier of gait 7
(`game-controls.md` §5.3.2 step 1, 1.0 when the control lacks it) and *W* is the **world
mounted-combat speed limit**:

1. The client looks up the DID map `RIPCORDS` (root key 0x10000002 here,
   `property-system.md` §8) at the entry labelled `MountedCombat_MaxSpeed` (key 0x83 here; the
   client passes the key 0x83 through a class-library function).
2. That entry names a ripcord object (0x7002F504 here) whose `RipCord_WorldProperty` names the
   world property `World_MountedCombat_MaxSpeed` (float; dictionary default **16.0**).
3. *W* is the world's current value of that property. The client-library function returns 0.0
   when the lookup fails.

This answers `game-controls.md` Q8 (the second forward-speed limit). With a locked gear the
limit of §5.4 step 3 does not include *W*.

> **OPEN:** what the original uses for *W* when the world has no value (the dictionary
> default 16.0, or 0, which would stop the steed). **Decision (Tessera):** the dictionary
> default.

<a id="s5-6"></a>
### 5.6 Turning (BIN, MT22)

The war-steed's turn rate is (*T*g + *A*g) × *R* degrees per second, where *T*g + *A*g is the
gait's turn multiplier plus add-mod of `game-controls.md` §5.3.2 step 2 (gait from
`MountMovement_CurrentSpeed`). While its `Movement_FreezeRotation` is not true and the
character is not steered with the mouse (the mouse condition is Inferred from the code's
structure), each update turns it by that rate × *t*, scaled by the turn input (turn right −
turn left, `collision.md` §11.8 item 2), when |turn input| > 0.0002; the turn input is
published to `Slider_Turn` through the ramped setter. While steered with the mouse, the steed
turns toward the steering heading by at most the same rate × *t* per update.

This refines `collision.md` §11.9, which states a fixed 150: 150 is only the default of *R*;
an actor with `Mount_Movement_Turn_Rate_Cached` turns at that value (100 on every template,
§6.1, plus war-steed bonuses).

> **OPEN:** the thresholds of the mouse-steering branch (two constants of the client were seen
> and not resolved).

<a id="s5-7"></a>
### 5.7 Published sliders (BIN, MT23)

At the end of an update the controller writes, through the ramped property setter with the
controller's ramp time (`collision.md` §11.6 item 8):

| Property | Value |
|---|---|
| `Slider_Strafe` | *v*x / *S*2 when *v*x > 0, else *v*x / *S*1 (0 when the divisor's magnitude is at most 0.0002) |
| `Slider_Propel` | *v*y / *L* when *v*y > 0, else *v*y / *B* (same rule) |
| `Slider_Fly` | 0 |
| `Slider_ForwardSpeed`, `Slider_StrafeSpeed`, `Slider_FlySpeed` | the sums of control and physics velocity, as on foot |

`Slider_Strafe` and `Slider_Propel` are 0 while `Movement_ActiveMovementFrozen` or
`Movement_PlayerMovementFrozen` is true. Unlike on foot (where `Slider_Propel` is the input),
the war-steed's propel slider is its **speed as a fraction of the current limit**.

<a id="s5-8"></a>
### 5.8 Gaits (Confirmed (script data); BIN partial)

`MountMovement_CurrentSpeed` selects the speed and turn multipliers. For the Clydesdale the
rider's script sets it from the walk style and `Slider_Propel` (§4.6 item 4). For war-steeds
the rider's script mirrors it to the mount (§4.6 item 3); what changes it on the rider is not
in the client's controller code that was read, nor in the class library.

> **OPEN:** who changes the war-steed's gait (`MountMovement_CurrentSpeed`) and how the
> gear-change rate *G* and `MountMovement_LastSpeedChangeTime` enter (reading: the server
> shifts the gait toward the input at *G* gears per second). No reader of *G* was found among
> the controller's methods.

> **Answered for Tessera (2026-10-11, P5; MQ9):** the walker shifts a selected gear at *G* gears
> per second from the input, keeps it in `MountedCombat_Current_LockedGear` (so §5.4 step 3
> limits the speed) and derives the gait from the forward speed (§14.6). The original's rule
> stays server-side (OPEN in §14.6).

<a id="s5-9"></a>
### 5.9 Server authority (BIN; not followed)

For war-steed types the client sets `Movement_ServerAuthoritative_Controller` (§4.7). The
war-steed controller keeps a ring of 20 server updates and blends its state toward them; that
part belongs to network handling and was not read (decision 0004). Tessera's own server owns
the mounted state; the walker runs §5.3–§5.7 locally.

---

<a id="s6"></a>
## 6. Mount and war-steed stats on templates

<a id="s6-1"></a>
### 6.1 Template properties and class-wide values (Confirmed, MT24)

Templates are the creature-subtree and avatar records of `creatures.md` §2.1. Every set of
the classes named carries the property with the single value shown (the `_Cached`,
`_ForwardSource` and `ForwardSource_` mirrors of `creatures.md` §3.2.1 carry the same value):

| Property | Type | Classes (sets) | Value |
|---|---|---|---|
| `Mount_Movement_Max_Speed` | float | 1723, 1724, 1653, 1654, 4026, 5785, 1725, 3074, 3075 (all 36,929) | 6.5 |
| `Mount_Movement_Acceleration` | float | same | 0.0 |
| `Mount_Movement_Turn_Rate` | float | same | 100.0 |
| `Mount_Movement_Gear_Change_Rate` | float | same | 2.0 |
| `Mount_Control_Turn_Rate_AddMod_Walk`, `…_Trot`, `…_Cantor` (spelled so), `…_Gallop` | float | same | 0.0 |
| `Mount_Combat_EvadePoints_Modifier` | float | same | 0.0 |
| `Mount_Durability_Current_Max` | int | same | 0 |
| `MountEndurance_MaxLevel`, `MountPower_MaxLevel` | float | same | 0.0 |
| `Vital_MountEnduranceThresholdPolicySingleton`, `Vital_MountPowerThresholdPolicySingleton` | DID | same | 0 |
| `MountedCombat_RiderToMount_DamageEnabled` | bool | same | true |
| `ForwardSource_Movement_MountSpeedMultiplier`, `Movement_Effect_MountSpeedMultiplier` | float | same | 1.0 |
| `Hotspot_OverrideMountedCombat_Allow`, `…_Disallow` | bool | same | false |
| `Mount_Momentum_CashoutPoints_Modifier` | float | 1653, 1654 | 0.0 |
| `Mount_SteedXP_Multiplier` | float | 1653, 1654 | 1.0 |
| `MountedCombat_Charge_MoveToSkills`, `MountedCombat_SteedCollisionEnabled`, `…_CanHitMountedUnits` | bool | 1653, 1654 | false |
| `MountedCombat_SteedCollisionEnabled_EffectOverride` | DID | 1653, 1654 | 0 |
| `Vital_MountEnduranceCombatBaseRegen`, `Vital_MountPowerCombatBaseRegen` | float | 1653, 1654 | 0.0 |
| `Vital_MountEndurancePeaceBaseRegen`, `Vital_MountPowerPeaceBaseRegen` | float | 1653, 1654 | 50.0 |
| `Combat_MountedCombo1` … `6` | enum | 1653, 1654 | 40 |
| `Combat_Mounted_SkillCombo` | bitfield32 | 1653, 1654 | 0 |

Authored (not class-wide) mount properties on templates:

| Property | Class 1723 | Class 1724 | Observed |
|---|---|---|---|
| `Mounted_Player_Cosmetic_Slot_Entity` | 228 (15 entities) | 52 (6 entities) | mount entities (§3) |
| `Mount_Type` | 229 | 62 | 1723: 0x20 (171), 0x8 (53), 0x1 (3), 0x10, 0x800; 1724: 0x20 (34), 0x1 (15), 0x800 (10), 0x40 (2), 0x4 |
| `Mounted_Player_Cosmetic_Slot_Scale` | 34 | 1 | 1.18 (33), 1.2, 0.9 |
| `Mounted_Player_Cosmetic_Slot_BoneOffset` | 64 | 2 | (0, 0, 0.2) in all |
| `Mount_MeshFX_Appearances` | 144 | 24 | a struct holding an `Appearance_InstanceList` (1–8 `Appearance_KeyInfo` of appearance record, key, modifier; `appearance.md` §3.2) |
| `MountedCombat_CollisionDetectionAllowed` | 159 | 0 | false |
| `Mounted_Combat_IsAMountedPet` | 3 | 0 | true |
| `Movement_MountAnimationSpeedMultiplier` | 1 | 1 | 1.5, 1.45 |
| `Mount_MountAppearance`, `Mount_RiderType`, `Mount_CurrentMountStatus`, `Mount_CurrentMountType` | 0 | 7, 1, 1, 1 | |

<a id="s6-2"></a>
### 6.2 Forwarding pairs (Confirmed, MT25)

PropertyMetaData objects (`misc-records.md` §13.1) name, for each mount stat, the property
that receives its value (`PropertyMetaData_ForwardedValueTarget`) and the weenie type that
must hold it (`…_RequiredWeenieType`, mapper 0x230000C9):

| Property | Receives its value | Required type |
|---|---|---|
| `ForwardSource_Movement_MountSpeedMultiplier` | `Movement_Effect_MountSpeedMultiplier` | `Agent` |
| `Mount_Movement_Max_Speed`, `…_Acceleration`, `…_Turn_Rate`, `…_Gear_Change_Rate` | the `_Cached` form | `Agent` |
| `MountEndurance_MaxLevel`, `MountPower_MaxLevel` | the `_Cached` form | `Agent` |
| `Mount_Durability_Current_Max`, `Mount_Combat_EvadePoints_Modifier` | the `_Cached` form | `Agent` |
| `Vital_MountEnduranceThresholdPolicySingleton`, `Vital_MountPowerThresholdPolicySingleton` | the `_Cached` form | `Agent` |
| `Mount_Control_Turn_Rate_AddMod_<gait>` | `…_<gait>_ForwardSource` | `Agent` |
| `MountedCombat_RiderToMount_DamageEnabled` | `…_ForwardSource` | `Agent` |
| `Mount_SteedXP_Multiplier`, `Mount_Momentum_CashoutPoints_Modifier` | the `_Cached` form | `Player` |
| `MountedCombat_SteedCollisionEnabled`, `…_CanHitMountedUnits`, `…_EffectOverride`, `MountedCombat_Charge_MoveToSkills` | `…_ForwardSource` | `Player` |
| `Hotspot_OverrideMountedCombat_Allow`, `…_Disallow` | `…_ForwardSource` | `Agent` |

So a client reads the **targets** (the controller reads the `_Cached` movement values, §5.3;
the gait add-mods are read through the `_ForwardSource` names that MountedCombatControl
names, `game-controls.md` §5.3.1). Modifier entries change the **sources** (§6.3).

<a id="s6-3"></a>
### 6.3 Records that modify mount stats (Confirmed, MT26)

Every modifier entry whose `Mod_Modified` names a mount stat, by holder class (`combat-data.md`
§2; class roles there):

| Stat | Mount effects (2459) | Other effects (734, 713, 780) | Traits (3438, 1477) | Relics (2219) | Class 1346 (38 objects) | Items (796) |
|---|---|---|---|---|---|---|
| `Mount_Movement_Max_Speed` | 24 | 55, 1, 1 | – | – | 38 | – |
| `Mount_Movement_Acceleration` | 25 | 32, 1 | – | 12 | 38 | – |
| `Mount_Movement_Turn_Rate` | 13 | 22 | – | 11 | 38 | 3 |
| `Mount_Movement_Gear_Change_Rate` | – | – | – | – | 38 | – |
| `MountEndurance_MaxLevel` | 25 | 15 | 14, 5 | 22 | 38 | 3 |
| `MountPower_MaxLevel` | 25 | 6 | 3, 8 | 22 | 38 | 3 |
| `Stat_MountStrength`, `Stat_MountAgility` | 25 each | 6, 19 | 13 + 5, 3 + 5 | 22, 21 | – | 3 each |
| `Vital_Mount*RegenAddMod` (4 properties) | 13 each | 2 for two of them | 4–7 | 10 for two | – | 3 for two |
| `Mount_Combat_EvadePoints_Modifier` | – | 8 | 3 | 10 | – | – |
| `Mount_SteedXP_Multiplier` | – | 16 | – | – | – | – |
| `ForwardSource_Movement_MountSpeedMultiplier` | 719 | 61 | – | – | – | – |
| `Movement_MountSpeedBonus` | (modifier list, 667) | 4 | – | – | – | – |

Progressions represent mount stats too: class-3918 progressions with
`LinearInterpolatingProgression_ValueProperty` = `MountEndurance_MaxLevel` or
`MountPower_MaxLevel` (14 each), `Stat_MountStrength`/`…Agility` (12 each), the regeneration
add-mods (3–6); class-3810 progressions with the endurance, power, strength and agility
properties as inner property (3 each); 7 class-1385 progressions of `Mount_SteedXP_Multiplier`.
The class library has its own stat classes for `Stat_MountAgility` (3434),
`Stat_MountStrength` (3435) and the two mount vitals (3431, 3432; BC).

How the server combines base, modifiers and progressions is `combat-data.md` §5 and §13 (server
rule). **Decision (Tessera server):** combine as `combat-data.md` §5.3's Decision D2 and
write the result to the forwarding target of §6.2.

<a id="s6-4"></a>
### 6.4 Mount vitals (Confirmed data; rules server-side)

Two vitals belong to a war-steed: **endurance** (`MountEndurance_*`) and **power**
(`MountPower_*`), each with `…_CurrentLevel`, `…_MaxLevel` (and `_Cached`),
`…_Starting_Level`, `…_Temporary`, `…_Temporary_Max`, `…_UpdateThresholdPercent`, and the
regeneration family `Vital_Mount<Vital><Peace|Combat><BaseRegen|CurrentRegen|RegenAddMod>`,
`Vital_Mount<Vital>LastRegenTimestamp`, `Vital_Mount<Vital>Percentage` and the threshold
policy singleton. Only the template values of §6.1 and the modifier entries of §6.3 hold
values in the data; the others are run-time actor properties. Vital-change effects name the
mount vitals as keys 3 and 4 of the vital-type mapper (`combat-data.md` §6.4).

<a id="s6-5"></a>
### 6.5 Mounted-combat flags (Confirmed data)

`MountedCombat_SteedCollisionEnabled` (with `…_Radius`, `…_CanHitMountedUnits`,
`…_EffectOverride`), `MountedCombat_Charge_MoveToSkills`,
`MountedCombat_RiderToMount_DamageEnabled`, `MountedCombat_CollisionDetectionAllowed`,
`MountedCombat_CurrentlyFollowing`, `MountedCombat_Current_LockedGear` (gait mapper, read by the
controller, §5.4) and `MountedCombat_Landblock_Area` (landblock block-map key 0x10000014,
`landblock.md` §5.5; read by the client library) are run-time flags. Effects set
`MountedCombat_Charge_MoveToSkills`, `…_RiderToMount_DamageEnabled` and
`…_SteedCollisionEnabled` (one effect each). 130 effects require a war-steed trait tree through
`Effect_RequiresProperty` = `Mount_WarSteed_ActiveTree` (mapper 0x23000369: key 1
`MountedCombat_Heavy` in 45, 2 `MountedCombat_Light` in 43, 3 `MountedCombat_Medium` in 42). The rules are server rules (`game-controls.md` §5.3.1 Q7).

<a id="s6-6"></a>
### 6.6 NPC and monster riders on templates (Confirmed, MT27)

- 418 templates start a mount effect through their startup arrays
  (`Effect_MonsterInitEffect_Array` in 286 class-1723 templates,
  `Effect_MonsterStartupEffect_Array` in 132 class-1724 templates, struct
  `Effect_MonsterStartupEffect_Struct`, field `Effect_StartupEffectID`). Only two effects are
  used: 0x70033562 (413 templates) and 0x7002EDDF (5). Both are startup records of §2.1;
  0x70033562 has no entity and no `Mount_Type`.
- All 280 templates that carry `Mounted_Player_Cosmetic_Slot_Entity` also carry `Mount_Type` and
  start 0x70033562. So for these riders the **template** supplies the entity, the type, and
  usually the scale and bone offset, and the startup effect supplies the scripts: the
  effect's application check copies the rider's own values into the effect's mount
  information (§4.7 step 3).
- 138 templates start a mount effect without an entity of their own: 5 with 0x7002EDDF (which
  names its own entity), 11 with `Mount_Type` 0x800 and no entity, 122 with neither.

> **OPEN:** how the 133 riders without any entity are drawn (reading: through instance sets or
> server data, §7; not confirmed).

---

<a id="s7"></a>
## 7. Riders and banners in the landblocks

<a id="s7-1"></a>
### 7.1 Census (Confirmed, MT28)

Seven keys mark riders in instance sets: `Mount_MountAppearance`, `Mount_RiderType`,
`Mount_Is_Mounted`, `Mount_CurrentMountType`, `Mount_Type`, `Mount_MountBannerAppearance`,
`Mount_MountBannerAnimation`. 1,549 instance sets carry at least one: 1,361 belong to instance
IDs without a static entity (spawned objects, `creatures.md` §6.2) and 188 to Physics static
entities (banners, §7.5).

| Property (sets without a static entity) | Type, mapper | Sets | Values |
|---|---|---|---|
| `Mount_MountAppearance` | enum, 0x23000338 (94 keys) | 1,301 | 25 distinct keys; key 71 in 805, 5 in 82, 1 in 61, 63 in 59, 43 in 51; 0 in 5 |
| `Mount_RiderType` | enum, 0x23000336 | 1,053 | 6 (739), 2 (161), 3 (85), 4 (25), 7 (22), 0 (15), 5 (4), 1 (2) |
| `Mount_Is_Mounted` | bool | 859 | true 844, false 15 |
| `Mount_CurrentMountType` | bitfield32, 0x23000280 | 636 | 0 (474), 0x1 (127), 0x20 (16), 0x800 (11), 0x8 (5), 0x10 (3) |
| `Mount_CurrentMountStatus` | enum, 0x230002C9 | 507 | 5 (503), 0 (3), 6 (1) |
| `Mount_Type` | bitfield32, 0x23000280 | 504 | 0x20 (461), 0x1 (22), 0x8 (9), 0 (8), 0x800 (3), 0x10 (1) |
| `Mount_MountBannerAppearance` | enum, 0x230003C7 (77 keys) | 139 | 16 distinct; 0 in 48 |
| `Mount_MountBannerAnimation` | enum, 0x230003C8 | 60 | 2 (29), 5 (17), 0 (6), 4 (6), 3 (2) |
| `Movement_MountAnimationSpeedMultiplier` | float | 5 | 1.5 |
| `ScriptSystem_MountedCombat_WarsteedState` | bool | 14 | true |
| `Script_ViewMountMeshFX` | bool | 461 | true (`instance-scripts.md`: server-only) |

These sets also carry the usual per-instance overrides of spawned figures: `Entity_ContentLayers`
(1,029), `Appearance_RandomAppearanceType` (930), `NPC_Cosmetic_Inventory_Item1` (816) and
`…Item2` (479) with their holding locations, `Physics_NoPhysics` (657), `Drama_DramaList`
(260), `NPC_StartingIdleAnimation` (197) and others (`appearance.md` §7.2).

<a id="s7-2"></a>
### 7.2 Enum labels used by rules (Confirmed)

`Mount_RiderType` (0x23000336; labels are interoperability identifiers): 0 `Undef`,
1 `Npc_Angled`, 2 `Avatar`, 3 `Npc_Facing`, 4 `Npc_Angled_Active`, 5 `Npc_Facing_Random_A`,
6 `Npc_Facing_Random_A_Banner`, 7 `Npc_Facing_Random_A_BannerVertical`, 8 `Npc_Upright`
(unused in instance sets).

`Mount_MountBannerAnimation` (0x230003C8): 0 `Undef`, 1 `NoWind`, 2 `LowWind`,
3 `HighWind`, 4 `MediumWind`, 5 `MovementBased`.

`Mount_MountAppearance` (0x23000338, 94 keys) and `Mount_MountBannerAppearance`
(0x230003C7, 77 keys) name horse liveries and banner devices; a reader shows their labels
from the mapper at run time.

<a id="s7-3"></a>
### 7.3 Where the rider sets sit in the spawn graph (Confirmed, MT28)

Of the 1,361 sets without a static entity, 794 have IID kind byte 1 and 567 kind byte 4
(`entity-descriptor.md` §4.2). By their links in the same landblock (`creatures.md` §6.3):
577 are sources of type-8 links (spawned object → generator), 418 sources of patrol links
(types 1 and 2), 191 targets of phasing links (0x10000019); 722 have no link at all in their
landblock. None holds `Generator_ProfileDID`. So a rider set is a **per-instance override**
of an object that a generator (or the server) spawns from a template (`creatures.md` §6.1
step 5): the template supplies the figure and usually the mount (§6.6), the set chooses the
livery, the rider pose and the banner.

> **OPEN:** how the 722 rider sets without any link in their landblock are spawned (by
> cross-landblock links, dramas or server lists).

<a id="s7-4"></a>
### 7.4 How the data consumes the rider keys (Confirmed (script data), MT29)

- **Rider pose.** Mount tables (for example 0x070007A8) switch on the rider's
  `Mount_RiderType` (read through `MeshFXScriptOutput_Source`, §4.5), gated by `CharGen_Stage`,
  and run a rider-pose scriptlet per type: types 1 and 4 have their own scriptlets, type 3 uses
  the general one, and type 2 and the default choose between a war-steed scriptlet and the
  general one by the rider's war-steed bits. 46 Switch nodes on `Mount_RiderType` exist in 22
  tables.
- **Livery.** Mount tables (6 Switch nodes in 5 tables, for example 0x070003A4) switch on the
  rider's `Mount_MountAppearance` and apply Appearance nodes (kind 58) to the mount: one or
  more (appearance record 0x20, appearance key, modifier) triples per key, some through a
  Random node (so a key can stand for a random choice among liveries).
- **Banners.** Switch nodes on `Mount_MountBannerAppearance` (11) and
  `Mount_MountBannerAnimation` (2) in banner tables choose the banner's device and wind
  animation; one SetProperty node is driven by `Mount_MountBannerAppearance`.

To draw an NPC rider mounted, an implementation therefore needs: the template's mount fields
(§6.6) on the spawned figure, the instance set's overrides applied to it (`appearance.md`
§7.4 order), and the script machinery of §4.

<a id="s7-5"></a>
### 7.5 Banner props (Confirmed)

188 Physics static entities carry `Mount_MountBannerAppearance` (20 distinct keys) and 158 of
them `Mount_MountBannerAnimation` (1 `NoWind` 139, 2 `LowWind` 19); the same counts occur in
the static entities' own property sets and in their instance sets. They place three 0x47 objects (0x4700573B 169 times,
0x47004E6F 18, 0x4700573A 1): free-standing banners whose device and wind come from the same
keys as a rider's banner.

---

<a id="s8"></a>
## 8. Reference graph (Confirmed)

| From | Field | To | Count |
|---|---|---|---|
| MountDirectory (`game-controls.md` §5.1) | `Mount_SkillToGrantTall` / `…Short` | skill (827) | 676 |
| skill (827) | `Skill_Toggle_Effect_List` / `Skill_Toggle_Effect_Data` / `Skill_Toggle_Effect` | mount effect (2459) | 688 (685 effects) |
| skill (827) | `Skill_MountRacialConversionAnalog` | skill (827) | 660 |
| item (796) | `EffectGenerator_UsageEffectList` / `EffectGenerator_EffectStruct` / `EffectGenerator_EffectID` | mount effect | 28 (22 effects) |
| template (1723, 1724) | startup-effect arrays (§6.6) | mount effect | 418 (2 effects) |
| dispel effect (715), hotspot (1170), landblock drama | effect lists | mount effect | 1 each |
| mount effect, template | `Mounted_Player_Cosmetic_Slot_Entity` | entity (0x47) | 721 + 280 (607 entities) |
| entity (0x47) | template chain | setup (0x04) | 607 → 13 setups |
| mount effect | `Mod_Array` / `Mod_Entry` / `Mod_Progression` | progression (3918) | 108 entries, 27 progressions |
| mount effect (war-steed) | `MeshFXScript_Appearance` / `MeshFXScript_AppearanceID` | appearance record (0x20) | 31 non-null |
| template | `Mount_MeshFX_Appearances` / … / `Appearance_AprFile` | appearance record (0x20) | 322 |
| RIPCORDS map | `MountedCombat_MaxSpeed` | ripcord object (0x7002F504) → world property | 1 |
| script tables (0x07) | driven pairs and Switches (§4.2) | rider properties | §4.2 |

---

<a id="s9"></a>
## 9. Invariants (Confirmed on this install unless marked)

| # | Invariant |
|---|---|
| MI1 | Every mount effect has a property set and an object state that decode completely; every root object holds, in the attribute added by class 2459, an object of class 2464 with a rider property set and a mount property set (§2.7). |
| MI2 | In every mount effect, `Mounted_Player_Cosmetic_Slot_HoldingLocation` is `Mount_Connection` and the three rotation offsets are 0.0. |
| MI3 | A mount effect is a war-steed record iff it has `Mount_ShouldUseAppearances`; iff its durability base maximum is 300 and its regeneration rate 1.5; and it then has no durability events (31 of 31). |
| MI4 | Every durability-event array has exactly three structs with event IDs 22, 17, 31 in that order (691 of 691), and every change is negative. |
| MI5 | Every MountDirectory skill has a `Skill_Toggle_Effect_List` with exactly one element, naming a mount effect (676 of 676). |
| MI6 | Every `Mounted_Player_Cosmetic_Slot_Entity` value resolves to a setup with a skeleton (607 of 607); exactly one of the 13 setups reached has `Mount_Connection`, and only template-named entities reach it. |
| MI7 | Every setup with `Mount_Connection` other than one has it on bone 0 with identity rotation and *x* = 0 (33 of 34). |
| MI8 | Every template with `Mounted_Player_Cosmetic_Slot_Entity` has `Mount_Type` and starts mount effect 0x70033562 (280 of 280). |
| MI9 | Every MeshFX node driven by `Mounted_Player_Cosmetic_Slot_Entity` has `AttachToParent` 1, `AllowPhysics` 0 and `MeshIIDTargetProperty` = `Mount_MeshFX_Entity` (35 of 35). |
| MI10 | A mount type's war-steed test is "AND 0x38 ≠ 0"; for every value in the data the test agrees with the label of its keys (keys 4–6). |
| MI11 | `Mount_RiderType` in instance sets is between 0 and 7 (1,053 of 1,053); `Mount_MountAppearance` values are keys of mapper 0x23000338 (1,301 of 1,301). |
| MI12 | The class-wide template values of §6.1 hold in every set of the classes named. |
| MI13 (Decision) | Tessera's war-steed controller never produces a forward control velocity above the forward limit of §5.4 step 3 nor a sideways one outside −*S*1 … *S*2 after an update. |
| MI14 | In every mounted rider clip of layers 13, 20, 51, 54, 60 and 65, bone 0 stays at its reference position (504 of 504 sampled pairs, within 0.3 mm). |
| MI15 | Every (mount entity, mount type) pair reached by a mount effect, hobby-horse entities excepted, has idle, walk, run, back, turn, strafe and airborne clips that bind to its skeleton (601 of 601). |
| MI16 | For each tree and race group, the war-steed effect of `Mount_Control_Tree_To_Skill_Hash` has a `BoneOffset` *z* equal, within 0.001 m, to minus the bone-0 rise of the male rider's layer-75 offset clip (12 of 12; 0 where there is no offset clip). |
| MI17 | Every mount skill names exactly one induction record, with `Induction_AllowedWhileMoving` false (688 of 688). |
| MI18 (Decision) | The walker's gait is 1 only while the forward control velocity is below −0.0002, 2 only while its magnitude is at most 0.0002, and never 3 (§14.6). |

---

<a id="s10"></a>
## 10. Explorer: mount card and viewer (Decision)

**Mount card** (for a class-2459 record): name; mount type (labels of the set bits; "war-steed"
when a bit of 0x38 is set); group of §2.1; entity DID and its setup; scale, bone offset; the
four script IDs by label; durability base, regeneration and the three event changes; the
modifier entries in the form of `combat-data.md` §15; referrers (skills, items, templates)
from §8.

**Mount viewer:** the rider (a player body of `appearance.md` §2–§4, or an NPC template's figure)
and the mount entity of the effect (or template) assembled as in §4.4's Decision, at the
rider's `Mount_Connection`; choose the MeshFX branch by the rider's mount type, species and
sex to obtain the scale; play `Mounted_Channeling_Loop` on the rider and `Horsepants_Startup`
on the mount through the script machinery, or, without it, the rider's mounted idle of
`script-table.md` §8.4 (the WhenRunning branch on `Mounted_Channeling_Loop`) and the mount's
idle. A gait selector sets `MountMovement_CurrentSpeed` on both.

---

<a id="s11"></a>
## 11. Worked synthetic examples

<a id="s11-1"></a>
### 11.1 Mount-type bits

A synthetic `Mount_Type` value 0x4004 has bits 2 and 14 set: keys 3 (`Goat`) and 15
(`Boar`). 0x4004 AND 0x38 = 0: not a war-steed type, so the mount system clears
`Mount_Movement_Use_RampUp_Controller`. An area whose `Area_Allowed_Mount_Types` is 0x1
refuses it with result B (0x1 AND 0x4004 = 0); an area with 0x4004 allows it; an area with 0
refuses it with result B; a mount with type 0 is refused with result A everywhere.

<a id="s11-2"></a>
### 11.2 War-steed parameters and forward limit

Layered parameters: forward 6.5, backward 2, strafe 3 / 3, control acceleration (10, 10, 0)
(the Warsteed layer over BasePlayer, `collision.md` §10.3). Actor: `Mount_Movement_Max_Speed_Cached`
8.3, `Mount_Movement_Acceleration_Cached` 16, `Mount_Movement_Turn_Rate_Cached` 120, no
`Mount_Movement_Gear_Change_Rate_Cached`, `Movement_SpeedMultiplier` 1.2, no acceleration
multiplier. Control: Gallop speed multiplier 1.0. World limit 16.

- *F* = 8.3 × 1.2 = 9.96; *B* = 2 × 1.2 = 2.4; (*a*x, *a*y, *a*z) = (10, 16, 0); *R* = 120;
  *G* = 2.0.
- *L* = min(16, 1.0 × 9.96) = 9.96 m/s.
- With `MountedCombat_Current_LockedGear` = 5 (Trot) and a Trot multiplier 0.7, the limit is
  0.7 × 9.96 = 6.972 m/s (the world limit is not applied).

<a id="s11-3"></a>
### 11.3 One update of the war-steed controller

Same block, no locked gear, *g* = 7 (Gallop), input (*x*, *y*) = (0.57, 1), stored *v* =
(0, 5), *t* = 0.1 s.

- Target: sideways 0 (gait not Neutral); forward *L* = 9.96 (because *y* > 0).
- Δ forward = min(9.96 − 5, 16 × 0.1) = 1.6; Δ sideways = 0.
- Mean = (0, 5.8); stored *v* = (0, 6.6); both within −2.4 … 9.96.
- `Slider_Propel` target value 6.6 / 9.96 ≈ 0.663.
- Turn: with Gallop turn multiplier 0.47 and add-mod 0 (`game-controls.md` §5.3.1), the rate
  is 0.47 × 120 = 56.4 degrees per second; a full right turn input turns 5.64 degrees in this
  update.

<a id="s11-4"></a>
### 11.4 Which scale a mount is drawn at

A synthetic table holds `Mounted_Player_Cosmetic_Mesh` as a Switch on `Mount_CurrentMountType`
with a case 0x4 (goat) whose species Switch has a branch for species A with a MeshFX node of
stored scale 1.05 that does **not** drive `Scale`, and a default branch whose node drives
`Scale`. The mount effect has `Mounted_Player_Cosmetic_Slot_Scale` (0.925, 0.925, 0.925).

- Rider of species A on the goat: drawn at (1.05, 1.05, 1.05); the effect's value is ignored.
- Rider of another species on the goat: the default node drives `Scale`: (0.925, 0.925, 0.925).
- Same rider, with a synthetic effect value (0.9, 1.0, 1.1) (none occurs in the install): the
  mount is scaled by 0.9 along its model *x*, 1.0 along *y* and 1.1 along *z*; with
  `UseParentScale` 1 and a rider final scale of (0.7, 0.7, 0.7) (a River-hobbit,
  `appearance.md` §6.1), (0.63, 0.7, 0.77).
- Rider without `Mounted_Player_Cosmetic_Slot_Entity` (null DID): the When node stops the
  branch; nothing is drawn.

<a id="s11-5"></a>
### 11.5 Normal-mount speed (Decision of §5.1)

Mount effect entry: `ForwardSource_Movement_MountSpeedMultiplier` Add 0.62 with modifier list
[`Movement_MountSpeedBonus`] and modifier operator Set. Rider without `Movement_MountSpeedBonus`:
the forwarded `Movement_Effect_MountSpeedMultiplier` is 1.0 + 0.62 = 1.62. Rider with
`Movement_MountSpeedBonus` 0.75: the entry's value becomes 0.75, so 1.75. With
`Movement_SpeedMultiplier` 1 and the BasePlayer forward speed 6.5 (the Mounted layer does not
change it), the Decision gives a target of 6.5 × 1.62 = 10.53 m/s, or 11.375 m/s with the bonus.

<a id="s11-6"></a>
### 11.6 Durability (Decision of §2.4)

A mount with base maximum 200, regeneration 1.0 and events (22: −30, 17: −90, 31: −180): its
rider takes a normal hit (200 → 170), then a critical hit (170 → 80), then 30 s pass (80 →
110), then a super-critical hit (110 → 0): Tessera's server dismounts the rider with
`Mount_CurrentMountStatus` 6.

---

<a id="s12"></a>
## 12. Open questions

| # | Question | Section |
|---|---|---|
| MQ1 | The original's durability rules (time unit, per-event application, zero, war-steeds). Server rule. | §2.4 |
| MQ2 | The meaning of the two class-2464 attributes set only in war-steed records. | §2.7 |
| MQ3 | Which mount type value (effect or entity) the rider's `Mount_CurrentMountType` takes. | §3.2 |
| MQ4 | How the rider fields reach the rider and what starts `Mounted_Player_Cosmetic_Start` for a normal mount (likely server-driven). **Answered for Tessera** (§14.7: the walker performs the server's part; step 4 is not reached by the graph, MT44); the original's path stays OPEN. | §4.1, §14.7 |
| MQ5 | The MeshFX node's run-time composition of holding location, bone offset, rotation offsets, scale flags and fade times (`script-runtime.md` OPEN-17). **Partly answered:** the bone offset is a translation in the location frame (MT39); rotations, `RemoveRotation` and fades stay OPEN (Decisions in §14.8). | §4.4, §14.8 |
| MQ6 | How `Movement_Effect_MountSpeedMultiplier` enters a normal mount's speed. | §5.1 |
| MQ7 | The world limit's value when the world has none. | §5.5 |
| MQ8 | The mouse-steering thresholds of the war-steed's turning. | §5.6 |
| MQ9 | Who shifts the war-steed's gait and how the gear-change rate and `MountMovement_LastSpeedChangeTime` are used. **Answered for Tessera** (§14.6, gear rule); the original's rule, and when it uses gait 3, stay OPEN (server). | §5.8, §14.6 |
| MQ10 | How the 133 riders whose templates name no mount entity are drawn. | §6.6 |
| MQ11 | How the 722 linkless rider instance sets are spawned. | §7.3 |
| MQ12 | `game-controls.md` Q23 (how trait appearances reach the mount), narrowed: the war-steed MeshFX nodes take their `Appearances` from the rider's `Mount_Appearances`; the class-library functions that build that array from `Mount_Appearances_TraitDID`, `…_TraitsOverride` and `…_Slot_Colourization` (classes 3439 and 3470) were not read. | §4.2 |
| MQ13 | What the original does when a mounted character enters deep water (Tessera: dismount by other). Server. | §14.12 |
| MQ14 | How the original's camera uses `Entity_Camera_LookAt_Offset*` (Tessera: raise and advance the walker's target). | §14.9 |
| MQ15 | Whether an area that forbids the mount type dismounts a mounted character (Tessera: no). Server. | §14.12 |

---

<a id="s13"></a>
## 13. Corrections and additions to other specs (proposals; not edited here)

| Spec | Proposal |
|---|---|
| `collision.md` §11.9 and row C62 | "a fixed 150" → 150 is the default; the war-steed controller uses the actor's `Mount_Movement_Turn_Rate_Cached` when present (mounts.md §5.3, §5.6). The controller described there is the war-steed (ramp-up) controller; normal mounts keep on-foot turning. |
| `game-controls.md` Q8 | Answered by mounts.md §5.5 (the RIPCORDS world limit). Q23 narrowed by mounts.md §4.2 and MQ12. |
| `combat-data.md` §6.1, row 2459 | Point to mounts.md §2 for the fields and groups. |
| `script-runtime.md` OPEN-17 | The mount MeshFX nodes are persistent (`Persistent` 1) and name their mesh-IID property; mounts.md §4.4 needs the MeshFX kind implemented. |
| `creatures.md` §6.1 step 5 | Rider overrides of spawned objects (mounts.md §7). |
| `collision.md` §11.6, OPEN (BIN 36) | The names of the two strafe fields need not rest on the registration order: the field registration gives each field's property ID with an accessor, and the accessors read `StrafeLeftMoveSpeed` and `StrafeRightMoveSpeed` from the third and fourth of the four consecutive speed floats (mounts.md MT32, BIN2). Which of the two the player controller's item 1 reads for *x* > 0 should be re-checked against these accessors. (Wording revised 2026-10-11, review note N2.) |
| `script-table.md` §8.4 step 2 | Name the mounted branch's parameter families and layers (mounts.md §14.1–§14.2) and that the rider's airborne clips need the `Jump` script running while the mount's do not (§14.3). |
| `game-controls.md` §5.1 Q6 | Data supporting the Tall/Short reading: mounts.md §14.10, MT41 (Short skills' effects carry a slot scale below 1). The war-steed skills per race come from `Mount_Control_Tree_To_Skill_Hash` (§5.3.1), MT40. |
| `script-runtime.md` §6.2, §12.4 | A note that targets are per node (the mount graph's untargeted children of targeted nodes read the mount, mounts.md §14.1). An OPEN on §12.4 step 3: `Mounted_Player_Cosmetic_Start` and `Mounted_Channeling_End` put a persistent Run in an early Sequence step and needed work in later steps (mounts.md MT44, §14.7), which suggests a completion path when the started script ends; re-check with the client. |
| `keymap.md` §8 | The walker's `toggle_mount` action (`walker.md` §16.4) belongs in the built-in table. |
| `walker.md` | New §16 (mounts in the walker), written with this revision. |

---

<a id="s14"></a>
## 14. Mounted presentation and gaits (P5)

This section gives what the walker needs to show a mounted player character with the
install's mounted gaits (decision 0024, P5 exit criterion 2): which clips the rider and the
mount play for each motion and gait, how they are found, how the gait follows the input, the
mount and dismount sequence, the mount's placement and scale without a full MeshFX runtime,
the rider's seat, the camera, which mounts each playable body rides, and a census of all 722
mount effects. Facts are from OBS3 and SCR3 (rows MT33–MT47) and, for the 2026-10-14
additions to §14.6, OBS4, SCR4 and BIN3 (rows MT48–MT51); everything marked
**Decision** is Tessera's choice for the offline walker. The walker side (options, command,
key, HUD and logs) is `walker.md` §16.

<a id="s14-1"></a>
### 14.1 Two entities, one evaluation model (Confirmed (script data), MT33)

The rider is the entity that moves and runs the input; the mount is the entity spawned by the
MeshFX node of §4.4 and attached to the rider at `Mount_Connection`. Both choose their clips
through their own script tables, the same way as any figure (`script-table.md` §8, §11.3):

- **Rider.** The rider's closure is the body's avatar closure (`script-table.md` §8.3). Its
  locomotion template 0x0C000FFE has a WhenRunning node on `Mounted_Channeling_Loop` as the
  first test of the game tree (`script-table.md` §8.4 step 2): while that script runs, its
  Running port leads to the mounted branch, whose Default ports are named (as stored; the
  names are interoperability identifiers) `riding_idle`, `riding_walk`, `riding_run`,
  `riding_run2`, `riding_walk_back`, `riding_run_back`, `riding_*_strafe_*`, `riding_*_turn_*`,
  `riding_turn_*`, `riding_falling`, `riding_land_run`, the `camel_riding_*` and
  `oryx_riding_*` families, the `mounted_combat_riding_*` family and
  `riding_warsteed_heavy`, `…_light`, `…_medium`, plus the `mounted_combat_*_idle` stance
  idles. The mounted branch switches on the rider's `Mount_CurrentMountType` (54 Switch
  nodes), `Movement_MovementStyle`, `Mount_CurrentMountStatus` (10 Switches, case 5
  `Mounted`, in front of some turn ports), `ScriptSystem_MeshPantsSync` (dictionary default true; false selects the
  `riding_run2` variants), `Entity_Element` and `Agent_Class`, and blends on `Slider_Propel`,
  `Slider_Strafe`, `Slider_Turn` and `ScriptSystem_Slider_Land`.
- **Mount.** The mount's closure is its 0x47 entity's (for the common horse entity
  0x47000361: class table 0x070003A3 and 14 includes, among them the player locomotion table
  0x0700001C and the mount table 0x070007A8); its start-up adds the MeshFX node's `Behavior`
  (`Horsepants_Startup`, §4.4). The mount's locomotion graph (the `Horsepants_Startup` of
  0x070007A8) is a WhenRunning on `Mounted_Channeling_Loop`
  **of the rider**, whose Running port holds a Switch on the rider's `Mount_CurrentMountType`
  and below it the blends on the rider's sliders and switches on the rider's
  `Movement_MovementStyle`, `MountMovement_CurrentSpeed`, `Entity_Element` and
  `ScriptSystem_MeshPantsSync`.

**The mount reads the rider through targets.** Every node of the mount's graph that reads a
rider property carries a Property target (type 3, `script-table.md` §4.4) whose
`PropertyTarget_PropertyName` is `MeshFXScriptOutput_Source`, the instance-ID property the
MeshFX node sets on the spawned entity. Census over every table and scriptlet (SCR3): 55
records hold such nodes; they read, through the target, `Slider_Turn` (218 ScalarBlends),
`Slider_Strafe` (139), `Slider_Propel` (65 ScalarBlends, 9 Switches),
`ScriptSystem_Slider_Land` (19), `MountMovement_CurrentSpeed` (72 Switches),
`Movement_MovementStyle` (27), `Mount_Is_Mounted` (19), `ScriptSystem_MeshPantsSync` (19),
`Entity_Element` (14), `CharGen_Stage` (11), `Mount_CurrentMountType` (9), and others with at
most 22 nodes each; 87 WhenRunning nodes and 34 Run nodes are targeted the same way. A node
**without** a target reads the running entity, here the mount itself (`script-runtime.md`
§6.2): for example the `Mount_RiderType` Switches nested inside the targeted `CharGen_Stage`
Switch of 0x070007A8 carry no target and read the mount's own `Mount_RiderType` (65 mount
entities hold `Avatar`, §3.2). A few records read one property both with and without the
target (13 records: `Shapeshifting_Target_Entity` in 4 scriptlets, `Mount_CurrentMountType` in
the mount tables 0x070007A8 and 0x070007F7, the three sliders, `Movement_MovementStyle` and
`Mount_CurrentMountStatus` in scriptlet 0x0C007D14, and detection, equipment, banner, combat and
sex properties in 6 other tables).

**Evaluating the mount (Decision).** Tessera's static evaluation (`script-table.md` §11.3)
evaluates a graph against one entity's property state. The walker evaluates the mount with two
property states: nodes whose Property target names `MeshFXScriptOutput_Source` read the
**rider's** state, every other node reads the **mount's** state; WhenRunning nodes with that
target see the rider's running scripts. Where the resolver cannot take a second state, the
walker may use one merged state: the mount's own properties, overlaid with the rider's values
of exactly the properties that the mount's closure reads through that target, and the rider's
`Mounted_Channeling_Loop` marked running. OBS3 used the merged form. The two forms differ only
where a record reads one property both with and without the target: in the mount table
0x070007A8 (and 0x070007F7) `Mount_CurrentMountType` is read through the target by the
locomotion Switch and without it by the rider-pose Switches, so the merged form gives the
rider-pose Switches the rider's type, which differs from the mount entity's own type for the 45
effects of §3.2; the two-state form is the rule.

<a id="s14-2"></a>
### 14.2 Rider clips by motion (Confirmed census, MT34, MT38)

Rider state for the evaluation: the rider fields and `Mount_CurrentMountType` of the Decision
of §4.1, `Mount_CurrentMountStatus` 5, `Mount_Is_Mounted` true, `Mounted_Channeling_Loop`
running and started, `Mounted_Player_Cosmetic_Mesh` started, the movement inputs of
`script-table.md` §8.4 (style, sliders, element) and, for war-steed types and the
Clydesdale, the gait as `MountMovement_CurrentSpeed`. Layers are keys of the layer mapper
(`script-table.md` §6.2) with their labels: 13 `mounted_idle`, 20 `movement`,
51 `mounted_warsteed_idle`, 52 `mounted_idle_emote`, 54 `mounted_movement`,
60 `mounted_jumping`, 65 `mounted_falling`, 75 `mounted_locomotion_offset`.

**Normal mounts** (example: body P3 Human M of `appearance.md` §2.2 on any horse effect;
clip DIDs are this install's and are read at run time; durations in seconds):

| Motion | Inputs | Rider clip (layer) |
|---|---|---|
| idle | sliders 0 | 0x05001447 (13), looping, 1.983 |
| walk | `Walking`, propel +1 | 0x05009359 (20), 1.983 |
| run (gallop) | `Running`, propel +1 | 0x05009358 (54), 0.583, displacement 4.38 m |
| walk back / run back | propel −1 | 0x05003666 (54), 1.983 / 0x0500144F (54), 0.983 |
| strafe left / right | strafe −1 / +1 | 0x05001452 / 0x05001454 (54), 0.783 |
| turn left / right | turn −1 / +1 | 0x05001456 / 0x05001458 (54), 1.000 |
| jump, airborne | `Entity_Element` 4 and the `Jump` script (key 1) running | 0x05001440 (60, one-shot, 0.250), then 0x05001463 (65, looping, 0.366) |
| falling without a jump | `Entity_Element` 4 | no airborne clip: the idle and locomotion layers continue |
| in water | `Entity_Element` 1, 2 or 3 | no swim clip in the mounted branch: the idle continues; with propel +1 the run clip |

Body P1 Hobbit M on the same horse: idle 0x05000E56 (13), walk 0x050036A5 (54), run 0x05000E57
(54, displacement 4.38 m), run back 0x05000E5F, strafes 0x05000E62 / 0x05000E64, turns
0x05000E66 / 0x05000E68, jump 0x05000E48 (60) then 0x05000E71 (65). The walk clip lies on
layer 20 for the 89-bone bodies (Human, Elf, Beorning, HighElf, River-hobbit) and on layer 54
for the 62-bone bodies (Hobbit, Dwarf, DwarfStoutAxe). Locomotion clips of the mounted branch
carry `AnimationScript_Speed` driven by `Movement_MountAnimationSpeedMultiplier` (dictionary
default 1.0; float, minimum 0.0), except the layer-20 walk clips of the 89-bone bodies and the
war-steed summon clips, which are driven by `Movement_SpeedMultiplier`.

**War-steeds and the Clydesdale** use the war-steed riding set: idle on layer 51 (Human M
0x0500522F, looping, 10.666) with a looping **offset clip** on layer 75 (§14.8), and a forward
clip chosen by the gait (§14.4). The Clydesdale (type 0x8000) rider uses this set too (its idle
0x0500522F and offset clip 0x050054F6 for Human M), with the gait set by the rule of §4.6 item 4.

**Census (OBS3).** For each of the 722 mount effects and each of the 15 race-table entries
(the 14 bodies of `appearance.md` §2.2; DwarfStoutAxe has two entries), the evaluation above
in the idle, walk and run states: 10,815 of 10,830 rows find the mount mesh (all but the 15
rows of the startup record 0x70033562, which has no entity, §6.6); every row finds an idle
clip (layer 13, or 51 for war-steed and Clydesdale types) and a walk clip (layer 20 or 54);
every row finds a run clip (war-steed types evaluated at gait 7) except the 60 Clydesdale
rows, whose run clip needs the gait 6 (`Canter`) that the Clydesdale rule sets. Every clip found binds to the body's skeleton (222
distinct (setup, clip) pairs of riders and mounts, all with the skeleton's track count).

<a id="s14-3"></a>
### 14.3 Mount clips by motion (Confirmed census, MT35)

Mount state for the evaluation: §14.1 (the rider's values through the target), the mount's own
properties, `Horsepants_Startup` started. Over all 605 (mount entity, mount type) pairs that
the 721 effects with an entity reach (601 entities), every pair finds an idle, walk, run, walk
back, turn, strafe and airborne clip, except the 4 hobby-horse entities, which find none (they
are held props, §14.11). The families:

| Family (type) | Entities | Idle | Walk | Run | Back | Airborne (60 / 65) |
|---|---|---|---|---|---|---|
| Horse A (0x1) | 256 | 0x05000D13 (13) | 0x05003055 | 0x05000D20 | 0x0500365F | 0x05000D23 / 0x05000D12 |
| Horse B (0x1) | 253 | 0x05000D13 | 0x05003055 | 0x05000D2A | 0x0500365F | – / 0x05000D32 |
| Goat (0x4) | 30 + 29 | 0x05003A35 or 0x050046C8 | 0x05003A41 | 0x05003A37 or 0x050046CA | 0x05003A39 | – / 0x05003A2F or 0x050046D2 |
| War-steed (0x8, 0x10, 0x20) | 6 + 6 + 8 | 0x05005234 (10) and 0x05000D13 (13) | by gait, §14.4 | by gait | 0x05000D17 | 0x05005F67 / 0x05005F6D |
| Boar (0x4000) | 4 | 0x05007D7F | 0x05007D8B | 0x05007D72 | 0x05007D88 | – / 0x05007D78 |
| Camel (0x20000) | 3 | 0x050098A7 | 0x050098A8 | 0x050098C4 | 0x050098B3 | – / 0x050098BB |
| Oryx (0x10000) | 1 | 0x05009370 | 0x05009382 | 0x05009381 | 0x05009384 | – / 0x05009364 |
| Clydesdale (0x8000) | 2 | 0x05008DF3 (10) | 0x05008DF5 (20) | 0x05008D38 | 0x05008DF4 | – / 0x05008D43 |
| Horse with a war-steed bit (0x9, 0x11, 0x21) | 3 pairs | as Horse A | 0x05003055 | 0x05000D20 at every gait | 0x0500365F | as Horse A |
| Hobby-horse (0x400) | 4 | none | none | none | none | none |

**Back clips per style (MT52; revision 2026-10-23).** The "Back" column above holds the
**walking** back clip (style `Walking`). Backwards at a run (style `Running`) the mount plays:

| Family (type) | Walking back | Running back |
|---|---|---|
| Horse A (0x1), and Horse A with a war-steed bit (0x9, 0x11, 0x21) | 0x0500365F | 0x05000D17 |
| Horse B (0x1) | 0x0500365F | 0x05000D37 |
| Goat (0x4), first table | 0x05003A39 | 0x05003A45 |
| Goat (0x4), second table | 0x05003A39 | 0x050046CC |
| War-steed (0x8, 0x10, 0x20) | 0x05000D17 | 0x05000D17 |
| Boar (0x4000) | 0x05007D88 | 0x05007D88 |
| Camel (0x20000) | 0x050098B3 | 0x050098B0 |
| Oryx (0x10000) | 0x05009384 | 0x0500937B |
| Clydesdale (0x8000) | 0x05008DF4 | 0x05008DF4 |
| Hobby-horse (0x400) | none | none |

Clip DIDs are observations of this install; the walker takes every clip from the resolver at
run time (`walker.md` §16.7), so this table is for tests and for recognising the data.

Walk, run and back are on layer 54 unless stated. Strafe and turn clips are on layer 54 too
(Horse A: strafes 0x05000D19 / 0x05000D1A, turns 0x05000D1B / 0x05000D1C). "–" means only the
looping layer-65 clip was listed for the airborne state. The mount's airborne clips follow
the rider's `Entity_Element` (4 or 6, through the target): a Sequence plays a one-shot jump-start
scriptlet and then the falling loop; unlike the rider's, they do not need the `Jump` script.
The mount's clips carry no `LocomotionDisplacement` (|displacement| ≤ 0.002 m in all sampled
mount clips) and are driven by `Movement_MountAnimationSpeedMultiplier`.

**Matching lengths.** For one motion the rider's and the mount's clips have the same length
(Human M on Horse A and on the medium war-steed: idle 1.983, walk 1.983, gallop 0.583, strafe
0.783, turn 1.000; war-steed walk 1.983, trot 0.800, canter 0.583, gallop 0.500, war-steed idle
10.666); the exceptions are the back clip (rider 0.983, mount 1.967, an exact ratio of 2) and
the airborne clips (rider 0.250 then 0.366, mount 0.750 then 2.000).

<a id="s14-4"></a>
### 14.4 War-steed gaits (Confirmed census, MT36, MT37)

Forward input (propel +1, style `Running`) with the gait set as `MountMovement_CurrentSpeed`
on the rider (and so read by the mount through the target); Human M on the medium war-steed
(effect 0x7002D53C, entity 0x47005D62, setup 0x040044D6):

| Gait (key, label) | Rider forward clip (54) | Mount forward clip (54) | Length |
|---|---|---|---|
| 1 `Backwards` | none | none | – |
| 2 `Neutral` | none | 0x05005237 | – |
| 3 `Neutral_Forwards` | 0x05005232 | 0x05005237 | 1.983 |
| 4 `Walk` | 0x05005232 | 0x05005237 | 1.983 |
| 5 `Trot` | 0x05005231 | 0x05005236 | 0.800 |
| 6 `Canter` | 0x0500522E | 0x05005233 | 0.583 |
| 7 `Gallop` | 0x05005230 | 0x05005235 | 0.500 |
| unset | none (the dictionary default of the gait is 2) | 0x05005237 | – |

With style `Walking` the rider plays 0x05005232 at every gait. Back input plays the rider's
0x0500144F (`Running`) or 0x05007083 (`Walking`) and the mount's 0x05000D17 at every gait.
Strafe input plays strafe clips at every gait (rider 0x0500536A / 0x0500536B at gaits 2–5,
0x05005368 / 0x05005369 at 6, 0x0500536C / 0x0500536D at 7), but a moving war-steed has no
sideways velocity (§5.4 step 1), so its published `Slider_Strafe` is 0 and these clips show
only at gait 2. The idle (layers 51 and 75 on the rider, 10 and 13 on the mount) does not
depend on the gait.

**Normal mounts do not read the gait.** The horse, goat, boar, camel and oryx families give the
same clips for gaits 2, 4, 5, 6 and 7 (OBS3, Horse A checked at every gait): their clips follow
`Movement_MovementStyle` and the sliders only. The Clydesdale's **rider** reads the gait (the
war-steed riding set); the Clydesdale **mount** does not.

<a id="s14-5"></a>
### 14.5 Finding the clips in the walker (Decision)

1. **Rider set-up.** Apply the Decision of §4.1 to the walker's avatar (rider fields,
   `Mount_CurrentMountType` = the effect's `Mount_Type`), then set
   `Mount_CurrentMountStatus` 5 and `Mount_Is_Mounted` true when the sequence of §14.7
   reaches "mounted", and mark `Mounted_Channeling_Loop` running.
2. **Mount mesh.** Choose the MeshFX node as `MountMesh` does (the node that the evaluation of
   `Mounted_Player_Cosmetic_Mesh` starts last, §4.4): entity, holding location, bone offset,
   scale and its `UseParentScale` flag. The node depends on the rider's mount type, species and
   sex only, so it is chosen once per (body, effect).
3. **State key.** Clips are chosen per state key: (style, sign of `Slider_Propel`, sign of
   `Slider_Strafe`, sign of `Slider_Turn` with a dead band of 0.05, element class ground / air /
   water, gait, whether the `Jump` script runs). Each slider enters the evaluation as −1, 0 or
   +1, as `MotionInputs` does; the controller's fractional slider values are logged but not used
   as blend weights. A key change starts one evaluation of the rider and one of the mount on
   the asset workers; results are cached per (body, effect, key).
4. **Layers.** The rider shows the most likely looping candidate of each of its idle layers (13,
   or 51 and 75), the most likely candidate of layer 54 (else 20) while moving, and 60 then 65
   while airborne; the mount likewise with its layers 10/13, 54/20 and 60/65. Ramps are the
   nodes' `RampIn` / `RampOut` (0.1 to 0.25 s in this data), not the on-foot fades of
   `walker.md` §10.5.
5. **Rate.** Mounted clips play at the client's rate: driven `Speed` × `SpeedMultiplier`
   (`script-table.md` §8.5); there is no speed blend and no rate matching to the ground speed,
   for rider and mount alike.
6. **Phase.** When the rider's and the mount's clips of one key start in the same frame, both
   start at phase 0 and are sampled at the same clip time (their lengths match, §14.3; for the
   back clips the mount's phase is the rider's halved).

<a id="s14-6"></a>
### 14.6 How the gait follows the input

**Normal mounts** (no war-steed bit; Confirmed rules of other specs, Decision where marked).
The Generic controller of `collision.md` §11.6 runs with the Mounted layer, or MountedWalking
in walking style (`collision.md` §11.1 step 4, §10.3: forward 6.5 and 3, backward 2 and 3,
strafe 5 and 3, ramp 0.25 s), the speed factor of the Decision of §5.1 and the on-foot
turning of `collision.md` §11.9. The "gait" is the walking flag of `collision.md` §11.8
item 6: `Walking` plays the walk clips, `Running` the gallop clips; the sliders follow the
input as on foot (`script-table.md` §8.4 slider rule, ramp time of the active layer).
`MountMovement_CurrentSpeed` stays unset (Neutral) except for the Clydesdale, where the rider's
script sets it (§4.6 item 4; **Decision:** the walker applies that rule itself: 4 while
walking, 6 while `Slider_Propel` is not 0, else 2).

**War-steeds** (a bit of 0x38; Tessera rule answering MQ9 for the walker). The controller of
§5.3–§5.7 runs with the parameter block of §5.3: the actor values are the player avatar
object's template values (`Mount_Movement_Max_Speed` 6.5, `…_Acceleration` 0, `…_Turn_Rate` 100,
`…_Gear_Change_Rate` 2.0 here, §6.1) plus the effect's constant `Add` entries on those four
properties (§2.6), written to the `_Cached` forms (Decision, as the forwarding of §6.2 would).
**Decision:** when the summed acceleration is not above 0, the layered `ControlAcceleration`
is kept (as if the actor lacked the value), so a steed whose effect has no acceleration entry
still moves. The gear and the gait then follow this rule, once per controller step:

1. **Selected gear** *s* (Tessera keeps it in `MountedCombat_Current_LockedGear`, so the forward
   limit of §5.4 step 3 is the gear's speed multiplier × *F*). On mounting *s* is unset (no
   limit beyond §5.5) and the gait is 2.
2. **Shifting up.** While forward input is held (*y* > 0.0002): if *s* is unset or below 4, *s*
   becomes 4 at once; afterwards *s* rises by one gear each time 1/*G* seconds of held forward
   input have passed since the last change (*G* from the parameter block, 2.0 by default), up
   to 7, or up to 4 while the style is `Walking`. Switching to `Walking` lowers *s* to 4 at once.
3. **Shifting down.** While backward input is held (*y* < −0.0002) and the steed still moves
   forward, *s* falls by one gear per 1/*G* seconds, down to 4. Releasing the keys keeps *s*.
4. **Gait from speed.** With *v* the forward control velocity after the update and *F* the
   block's forward speed: *v* < −0.0002 gives gait 1 (`Backwards`); |*v*| ≤ 0.0002 gives 2
   (`Neutral`); otherwise the smallest gait *k* from 4 to *s* (to 7 when *s* is unset) whose
   speed multiplier (`game-controls.md` §5.3.2 step 1) × *F* + 0.25 m/s is at least *v*, else
   *s* (or 7). **Crossing step (Decision, 2026-10-14):** a step whose *v* is below −0.0002 while
   the previous step's *v* was above 0.0002 gives 2 instead of 1, so braking from a forward gait
   always shows one step of Neutral (7, 6, 5, 4, 2, 1), even when a step of *a*y × 1/60 s jumps
   over the ±0.0002 band (0.27 m/s per step for *a*y 16). Tessera never produces gait 3
   (`Neutral_Forwards`) (Decision; see "Gait 3" below). The gait is written
   to `MountMovement_CurrentSpeed` on the rider, so the rider's and the mount's clips and the
   turn rate of §5.6 follow it.
5. **Momentum.** With the keys released the controller keeps the forward velocity while the
   gait is above 3 (§5.4 step 3), because the walker never sets
   `GOP_MountedMovement_ReleasedKeys_LoseMomentum` (dictionary default false). Backward input
   brakes at *a*y (target *y* × *B*, §5.4 step 2) and then reverses.
6. **Published sliders** are those of §5.7 (`Slider_Propel` = speed as a fraction of the
   current limit); for the state key of §14.5 only their signs count.

`MountMovement_LastSpeedChangeTime`, `MountedCombat_Current_LockedGear` and
`GOP_MountedMovement_ReleasedKeys_LoseMomentum` are named by no script table or scriptlet of
the install (MT46), so the rule above changes no script result except through the gait.

**Gait 3 (Confirmed (script data), MT49).** Nothing in the install's data produces gait 3 on
the rider. Over every table and scriptlet, the SetProperty nodes on `MountMovement_CurrentSpeed`
write 3 only in the two copies of `Mounted_Channeling_Loop` (tables 0x0700001C and 0x07002B86
here), where one node per gait value 1–7 copies the rider's current gait to the mount (§4.6
item 3); the other writers set 0 (server-only), 2, 4 or 6 (the Clydesdale rule of §4.6 item 4
and one node elsewhere) or 7 (the mounted charge script). No reader of *G* or writer of the
gait was found in the client's controller code or in the class library (§5.8). So gait 3 can
reach the rider only from the server. The data read it in two places: the clip Switches give
it the Walk clips (§14.4), and the mounted jump has an explicit empty case for it (no impulse,
below).

**Jumping while mounted (Confirmed (script data), MT50; BIN3).** The jump key path is that of
`collision.md` §11.10 (`Movement_ActionKey`, `Loc_JumpKeyPressed`, `Loc_Jumping`); its gates 1–4
apply mounted as on foot, and gate 5 is replaced by the gates below. In `Loc_Jumping` the root
WhenRunning on `Mounted_Channeling_Loop` takes its Running port while the loop runs; there a
Switch on the rider's `Mount_CurrentMountType` chooses between two branches (labels and script
keys are interoperability identifiers; table DIDs are this install's, read at run time):

1. **War-steed branch** (case Equal with one of the values 0x8, 0x10, 0x20; so a type with
   another bit as well, such as 0x9, takes the other branch). A When on
   `MountMovement_CurrentSpeed` (an unset gait reads as its dictionary default 2,
   `script-runtime.md` R27):
   - gait **Equal 2** (`Neutral`): no jump; the mount (Child target at `Mount_Connection`) runs
     `Emote_Idle1`; `Movement_IsJumping` is not set.
   - gait **not 2**: a Sequence sets `Movement_IsJumping` true, then (under the While on that
     property) runs `Jump` on the rider and on the mount and, when `Movement_CannotJump` is
     false, a Switch on the gait: cases 4, 5, 6 and 7 each hold a PhysicsImpulse node with
     `IsJump`, `AirTime` 0.285 (driven by `Mount_Movement_Jump_Modifier`) and
     `ExpectedGravity` 15; case 3 is an empty Collection; gait 1 matches no case. Then a 0.2 s
     Timer and the clear of `Movement_IsJumping`, as on foot.
2. **Every other type** (the Switch's Default port; normal mounts, the Clydesdale and the
   mixed types): a Collection whose ordinal 5 is a When on `Slider_Propel`: **greater than
   0.5** leads to a When on `Slider_Strafe` whose tests "greater than 0.25" and "less than
   −0.25" lead to nothing and whose Default sets `Movement_IsJumping` true; propel at most 0.5
   runs `Emote_Idle1` on the mount. Ordinal 6 is the While on `Movement_IsJumping` with the same
   Sequence as above, but with one undriven impulse node (`AirTime` 0.285, `ExpectedGravity` 15)
   under the `Movement_CannotJump` case false and no gait Switch.

So a normal mount jumps only while moving forward with `Slider_Propel` above 0.5 and
`Slider_Strafe` within ±0.25, a war-steed only at gaits 4–7 (gaits 1 and 3 play the `Jump`
scripts without an impulse). With the stored `AirTime` the vertical impulse is
(0.285 + 0.2) × 15 = **7.275 m/s** (`collision.md` §11.10 item 2).

The table 0x0700001C holds both branches; 0x07002B86 has the same mounted branches; the third
table that defines `Loc_Jumping`, 0x070017E4, has only the Default branch. All 14 avatar class
tables include 0x0700001C directly (MT50).

**`Mount_Movement_Jump_Modifier`** (float, dictionary default 0.0) is stored and modified by no
record of the install; its property ID occurs only in the dictionary, one enum mapper and the
eight driven pairs of the two tables (MT51). When a node with a driven pair starts and the
entity has no value for the source, the client leaves the field at its stored value: the value
holder is not filled with the dictionary default (unlike a Switch's read, `script-runtime.md`
R27), and the field is written only when the entity's read reports a value (BIN3). So offline,
where nothing sets the property, the war-steed nodes keep `AirTime` 0.285. This states for
driven pairs what `script-runtime.md` §5.2 calls "has a value": a dictionary default does not
count (proposal for that spec's next revision).

> **OPEN:** (MQ9, narrowed 2026-10-14) the original's gear and gait rules, which the server
> applies (§5.9): when the server sends gait 3 (`Neutral_Forwards`) (the data never produce it,
> treat it like Walk for clips and give it no jump impulse; above), how it shifts gears, and
> what value it gives `Mount_Movement_Jump_Modifier`. Tessera's rules are the Decisions above.

<a id="s14-7"></a>
### 14.7 The mount and dismount sequence (Confirmed data; timeline Decision; MQ4)

**Summoning (the skill's induction; Confirmed, MT42).** Every mount skill names one induction
record (class 1195, `combat-data.md` §7.9) in `Skill_InductionAction`. The 688 mount skills use
five records:

| Records | Skills | `Induction_Duration` | `Induction_AnimationScriptID` | Allowed in combat | Duration modifiers |
|---|---|---|---|---|---|
| 1 (0x7000A03E here) | 660 (every non-war-steed mount skill, hobby-horse included) | 2.0 s | `Mounted_Summon_Start` | false | none |
| 4 | 28 (war-steed skills) | 3.0 s | `Mounted_Warsteed_Summon_Start` | true | `MTX_Mount_SummonTimeReduction` (multiplier list, 4), `Trait_Mount_SummonTime` (additive list, 1) |

In all five `Induction_AllowedWhileMoving` is false and `Induction_Interruptable` true; they
list set-back events (hit 22; the war-steed ones also 17, 11, 6, 23). The summon scripts are
defined in nine species tables (`Mounted_Summon_Start`: 0x07000428, 0x07000457, 0x07000460,
0x0700046A, 0x0700047C, 0x07000484, 0x0700048C, 0x0700204F, 0x0700205D; the war-steed one in
the same and 0x070018F6). Each is a Sequence of two scriptlets: a one-shot clip on layer 40
(`attack`; ramp-in 0.2 s; Human M 0x050032BE, 0.916 s) followed by a looping clip on layer 40
(Hobbit M 0x050032BF). The war-steed version's first clip is driven by
`Movement_SpeedMultiplier`.

**Mounting (Confirmed script data, MT43).** `Mounted_Channeling_Start` runs
`Mounted_Channeling_Mounting` (which sets `Script_IsMounted` true, runs `Visibility_Controller`
and, for war-steed types, `Mounted_Warsteed_Manager`), a 0.4 s Timer that blocks random emotes,
a game callback, and `Equipment_Hide` on the Child targets of held implements of two types.
The rider's evaluation of that run starts one clip: the **mounting clip** on layer 54
(Human M 0x05001450, 2.000 s, from the standing pose to the seated pose: lowest bone 0.02 m →
0.82 m, highest 1.68 m → 2.41 m; Hobbit M 0x05000E5D). In `Mounted_Player_Cosmetic_Start` the
start script is step 2 of a Sequence and the animation script and the mesh are step 4; step 2 is
a Run with `Persistent` 1. Read with `script-runtime.md` §12.4 step 3 (a persistent Run stays
running until it is stopped; no path that completes it when its script ends was found), step 4
is never reached (MT44). The same shape occurs in `Mounted_Channeling_End` (below: a persistent
Run of the unsummon effect in an early step, the loop's stop in a later one). Either such Runs
do complete when the started script ends, or other code supplies the later steps; which one is
OPEN (§13 proposal for `script-runtime.md`). The walker's timeline below gives the same
presentation in both cases.

**Held implements (MT53; revision 2026-10-23).** The "held implements of two types" are the
implement types `Shield` (8) and `Cosmetic` (0x10000) of mapper 0x23000004: `Equipment_Hide`
runs on the held item at `Wpn_LOffHand` when the rider's `Inventory_ImplementTypeSecondary` is
`Cosmetic`, at `Wpn_Forearm` when it is `Shield`, and at `Wpn_RHand` when
`Inventory_ImplementTypePrimary` is `Cosmetic`. The script is the held item's own: it is
defined in four tables; in 0x07000035 it stops `Activate_fx` and `Cbt_Bow_Mode` on the item and
starts a persistent opacity effect (`OpacityModify`, 0.15 s ramps) on it, so the item fades
(which tables the held items use, and the effect's target opacity, were not traced). The
walker's Decision to hide none (`walker.md` §16.12) is a departure from this data and stays
until a later round. The 0.4 s Timer is the longest child of `Mounted_Channeling_Start`, as the
timeline below uses.

**Dismounting (Confirmed script data).** The OnStop branch runs `Dismounted_ByPlayer_Script` for
status 7 and `Dismounted_ByOther_Script` otherwise (§4.3). For the 714 channelling records
these are `Mounted_Channeling_End` and `Mounted_Channeling_Failure`. In table 0x0700001C:

- `Mounted_Channeling_End`, player, not a war-steed state, **no movement key held**: in parallel,
  the mount plays `Emote_Mounted_Bow` (Child target; on the mount a one-shot clip on layer 52,
  0x05004D50 on the horse and war-steed entities checked) and `Emote_Idle10`; the rider's
  `Script_IsMounted` becomes false; after a **1.5 s** Timer `Visibility_Controller` runs and
  `Mounted_Channeling_Loop` is stopped.
- The same with **a movement key held**: `Mounted_Player_Cosmetic_Mesh` is stopped at once; a
  Sequence then runs a game callback, the mount's `vfxMounted_natk_Unsummoned` (a persistent
  Run), the stop of `Mounted_Channeling_Loop`, and `Script_IsMounted` false with
  `Visibility_Controller`; no Timer delays it.
- With the war-steed state set: the same bow and idle emotes on the mount, the loop stopped,
  then `Mounted_Warsteed_Manager` stopped.
- `Mounted_Channeling_Failure`: out of combat, the mesh, the loop and the mounting scripts are
  stopped at once; in combat the mount first runs `vfxMounted_horse_death`.

No dismount script starts a clip on the rider: when the loop stops, the rider's template
leaves the mounted branch (WhenRunning NotRunning) and the on-foot idle of
`script-table.md` §8.3 starts with its own ramp.

**Timeline in the walker (Decision; answers MQ4 for Tessera).** The walker is its own server
(decision 0024: the client half, offline), so it performs what the original's server does:

| Phase | Starts | Presentation | Ends |
|---|---|---|---|
| Summoning | the request (§16 of `walker.md`) | the induction script of the skill on the rider (layer 40); a request without a skill uses the 2.0 s / 3.0 s record of a skill toggling the effect, else no summoning | after `Induction_Duration` (modifier lists ignored offline); any movement input, a jump, a fall or water cancels it (no mount) |
| Mounting | the end of summoning | the Decision of §4.1; `Mounted_Player_Cosmetic_Start`; the start script and its mounting clip | when the start script's own graph has completed (0.4 s for `Mounted_Channeling_Start`, its longest child; for the hobby-horse `Emote_Pickup`'s run) the walker starts, as step 4 would, the animation script and `Mounted_Player_Cosmetic_Mesh`: the mount appears with its fade-in (§14.8) and `Mounted_Channeling_Loop` sets `Mount_Is_Mounted` |
| Mounted | `Mount_Is_Mounted` true | §14.2–§14.6; the controller's Mounted or Warsteed layer | a dismount request |
| Dismounting | status 7 (player) or 6 (other) | the dismount script; mount bow at rest; fade-out | at rest by the player: 1.5 s later the loop stops (`Mount_Is_Mounted` false, on-foot layers), and the mount fades out; moving, or by other: at once |

Movement during "mounting" is allowed (the induction has ended); the mounting clip plays on
layer 54 and yields to locomotion clips by its ramps.

<a id="s14-8"></a>
### 14.8 Placement, seat and scale (MQ5, partly answered)

**The seat (Confirmed, MT38).** In every mounted rider clip on layers 13, 20, 51, 54, 60 and 65
(504 (setup, clip) pairs, 16 samples each), bone 0 stays at its reference position (within
0.3 mm). The rider's `Mount_Connection` is on bone 0 and cancels bone 0's reference height
(§3.3), so the mount's origin is the rider's model origin: the controller's feet. The mounted
clips raise the rider's body into the saddle around a fixed bone 0 (Human M: lowest bone 0.02
→ 0.80 m, highest 1.69 → 2.42 m; Hobbit M: 0.02 → 0.51 m and 1.07 → 1.88 m). Only the layer-75
**offset clips** move bone 0: 11 distinct clips in 23 sampled (setup, clip) pairs, each holding
bone 0 at one fixed offset over the whole clip (equal in all 16 samples) (Human M medium 0x050054F5: 0.135 m up, 0.065 m forward of its reference position;
heavy 0x050054F6: 0.300 m up, 0.192 m forward).

**The bone offset is a translation in the location's frame (Confirmed by data, MT39).** The
war-steed effects come per race and tree (§14.10), and each effect's
`Mounted_Player_Cosmetic_Slot_BoneOffset` cancels the height its riders' layer-75 clip adds to
bone 0:

| Tree | Bodies (species) | Effect (here) | BoneOffset *z* | Rider offset clip (male) | Bone 0 rise | Net |
|---|---|---|---|---|---|---|
| Medium | Human, Elf, Beorning, HighElf | 0x7002D53C | −0.135 | 0x050054F5 | +0.1347 | −0.0003 |
| Medium | Hobbit | 0x7002D540 | −0.16 | 0x0500598D | +0.1603 | +0.0003 |
| Medium | Dwarf, DwarfStoutAxe | 0x70031E9B | −0.37 | 0x05005962 | +0.3703 | +0.0003 |
| Medium | River-hobbit | 0x70066B26 | −0.135 (and +0.05 *y*) | 0x050054F5 | +0.1347 | −0.0003 |
| Heavy | Human, Elf, Beorning, HighElf | 0x7002D53D | −0.30 | 0x050054F6 | +0.2997 | −0.0003 |
| Heavy | Hobbit | 0x7002D53B | −0.32 | 0x05005992 | +0.3203 | +0.0003 |
| Heavy | Dwarf, DwarfStoutAxe | 0x70031E9A | −0.50 | 0x0500595F | +0.5003 | +0.0003 |
| Heavy | River-hobbit | 0x70066B23 | −0.30 | 0x050054F6 | +0.2997 | −0.0003 |
| Light | Human, Elf, Beorning, HighElf | 0x7002D544 | none | none | 0 | 0 |
| Light | Hobbit | 0x7002D53F | none | none | 0 | 0 |
| Light | Dwarf, DwarfStoutAxe | 0x70031E99 | −0.23 | 0x05005C79 | +0.2303 | +0.0003 |
| Light | River-hobbit | 0x70066B27 | none (+0.05 *y*) | none | 0 | 0 |

Female bodies use their own offset clips: Human, Elf, Beorning, HighElf and River-hobbit F
0x05005990 (medium) and 0x05005994 (heavy), which leave a net of +0.025 and +0.020 m; Hobbit F
0x0500598F and 0x05005993, which leave −0.0003 and −0.040 m. So the node's
`BoneOffset` is applied in the holding location's frame, after it, in metres, with *z* up, as
the Decision of §4.4 states; the Decision now rests on the data (MT39) for the translation part.
The rotation offsets (0 in every mount record), `RemoveRotation` and the fade semantics remain
open (MQ5).

**Scale (Confirmed census).** The node chosen for a (body, effect) drives `Scale` from the
effect's `Mounted_Player_Cosmetic_Slot_Scale` for most bodies; the HighElf and River-hobbit
branches of the horse, oryx and combined cases, the River-hobbit branch of the goat case (1.05)
and every camel, boar and Clydesdale node use their stored scale (camel: 0.70 Hobbit, 0.80 Dwarf, 0.83 DwarfStoutAxe, 0.97 River-hobbit, 1.0 others;
boar: 0.78 Hobbit, 0.95 Dwarf, 1.0 DwarfStoutAxe, 1.08 female, 1.15 male others; horse for
HighElf 1.03 with `UseParentScale` 0; horse for River-hobbit 1.0 with `UseParentScale` 1, so
the River-hobbit's final scale 0.7 (`appearance.md` §6.1) scales its horse to 0.7).

**Placement without a MeshFX runtime (Decision; refines §4.9).** Each frame, after the rider's
pose is sampled:

1. The **attachment point** is *P* = *M*rider × *B*0 × *H* × T(`BoneOffset`), where *M*rider is
   the rider's model transform (feet, heading and final model scale), *B*0 the rider's posed
   model matrix of the location's bone (bone 0 for every player setup), *H* the location frame
   (`skeleton-anim.md` §8.2); so the location and the bone offset scale with the rider. The
   mount's model transform is the position and rotation of *P* followed by S(`Scale` ∘ *k*),
   where *k* is the rider's final model scale per axis when `UseParentScale` is 1, else
   (1, 1, 1): the rider's scale reaches the mount's size only through *k* (Decision; the
   composition of §4.4).
2. The mount's own pose is sampled from its clips (§14.5) and skinned with its own skeleton.
3. **Fade (Decision):** the mount's opacity rises linearly from 0 to 1 over `FadeInTime`
   (0.25 s; `Mount_FadeIn_Override` when the rider holds it) after it appears and falls from 1
   to 0 over `FadeOutTime` (0.25 s) when it is removed; it is drawn and animated during both.
4. **Collision:** the mount has none (`AllowPhysics` 0 in every mount node, MI9); the rider's
   own capsule (`collision.md` §10.6) is the only collision shape while mounted.
5. **Visibility:** the mount is drawn, culled and faded as part of the rider's figure (the
   rider's character distance and level of detail, `walker.md` §11.4.2, §14.5).

<a id="s14-9"></a>
### 14.9 The camera while mounted (Confirmed data, MT47; Decision)

711 mount effects (all but the 3 startup records and the 8 hobby-horse records) hold two
modifier entries with operator `Set`: `ForwardSource_Entity_Camera_LookAt_OffsetZ` 0.75 and
`ForwardSource_Entity_Camera_LookAt_OffsetY` 0.2 (§2.6). The dictionary defines
`Entity_Camera_LookAt_OffsetX`, `…Y`, `…Z` (float, default 0.0) and the three `ForwardSource_`
forms; which client code reads them was not traced. The offset matches the seat: the Human M
rider's highest bone rises by 0.73 m and the Hobbit M's by 0.81 m when mounted (§14.8).

**Decision (walker).** While mounted, the walker's follow camera (`walker.md` §5) aims at its
on-foot target point raised by the effect's `…OffsetZ` and moved forward along the rider's
heading by `…OffsetY` (0.75 m and 0.2 m here); without the entries (hobby-horse) nothing
changes. Offsets change over the camera's smoothing like any target move.

> **OPEN:** how the original's third-person camera (`walker.md` §11.7) uses
> `Entity_Camera_LookAt_Offset*` (axis frame, smoothing).

<a id="s14-10"></a>
### 14.10 Which mounts a playable body rides (Confirmed data, MT40, MT41; Decision)

**Normal mounts.** The MountDirectory (0x79048B29 here, `game-controls.md` §5.1) lists 347
`Mount_SkillToGrantTall` and 329 `Mount_SkillToGrantShort` skills. 660 mount skills name a
`Skill_MountRacialConversionAnalog`; the relation is symmetric (each names the other), and
328 of the pairs join one Tall and one Short skill. In 324 of those 328 pairs the Short
skill's effect stores `Mounted_Player_Cosmetic_Slot_Scale` (0.925 in 314, 0.95 in 6, 0.9 in 4)
and the Tall skill's effect stores none (1.0 by the node's stored value); 309 of these 324
pairs also name different entities. Over the directory: 324 of 329 Short skills toggle an effect
with a scale below 1; 332 of 347 Tall skills toggle one without a scale. This supports the
reading of `game-controls.md` Q6 (tall bodies use the Tall group).

**War-steeds.** MountedCombatControl's `Mount_Control_Tree_To_Skill_Hash` maps each tree and
`Mount_Control_Player_Race` to one skill (24 entries, 12 skills): Human, Beorning, Elf and
HighElf share one skill per tree; Hobbit, River-hobbit and Dwarf/DwarfStoutAxe have their own.
Each of the 12 skills toggles its war-steed mount effect and three class-718 effects (the same
three in all 12). The 12 effects are those of the table in §14.8, and their bone offsets fit
their riders' offset clips.

**Decision (walker).** The mounts offered for a body are: the effects of the skills of its
directory group (Tall when the species' `RaceTable_IsTallRace` is true, else Short), in the
directory's stored order, then the three war-steed effects of its race in the order of the tree keys (1 `Heavy`,
2 `Light`, 3 `Medium` here). Any other mount effect may still be chosen by DID; the walker
then notes "not this body's group" on the HUD and draws it (the MeshFX node's species branch
gives its scale).

<a id="s14-11"></a>
### 14.11 Census of all mount effects for the walker (Confirmed, OBS3)

| Category | Effects | Drawable in the walker | Gaits |
|---|---|---|---|
| Normal mounts: Horse 0x1 (590), Goat 0x4 (72), Boar 0x4000 (9), Camel 0x20000 (6), Oryx 0x10000 (2) | 679 | yes, every body | walk / run style, back, strafe, turn, jump (§14.2, §14.3) |
| Clydesdale 0x8000 | 4 | yes | war-steed riding set on the rider with the §4.6 item 4 gait; normal controller |
| War-steeds 0x8 (8), 0x10 (8), 0x20 (9) | 25 | yes | gaits 1–7 (§14.4) on the war-steed controller |
| War-steed startup records with an entity (0x7002EDDF, 0x7003230E; type 0x20) | 2 | yes (by DID; not offered, §14.10) | as war-steeds |
| Horse with a war-steed bit (0x9, 0x11, 0x21) | 3 | yes (by DID; reached by no skill of the directory or the tree hash) | war-steed controller; horse clips at every gait (§14.3) |
| Hobby-horse 0x400 | 8 | as a **held prop** at the node's stored holding location (`Wpn_LHand`, not driven), not as a mount: `Horsepants_Startup` runs `Emote_Holding_HobbyHorse` and `Mounted_Player_Cosmetic_Start` instead of the channelling loop, so `Mount_Is_Mounted` is never set and the rider keeps the on-foot controller and clips | on foot |
| No entity, no type (0x70033562, the NPC startup record of §6.6) | 1 | no: the rider fields come from NPC templates (§4.7 step 3); a player has none | – |

Sum: 722. Reasons a walker may still fail to draw a choice: a later install that changes these
records; a body whose setup lacks `Mount_Connection` (none of the 14 bodies; `skeleton-anim.md`
§8.4 rule: nothing is drawn); an evaluation that finds no mesh (the When node of §4.4 stops a
null entity). The walker reports these on the HUD (`walker.md` §16).

<a id="s14-12"></a>
### 14.12 Water, air, areas and refusals (Confirmed data; Decision)

- **Water.** The application check refuses a target whose `Entity_Element` is 1, 2 or 3
  (§4.7 step 4). The rider's mounted branch has no swim clips and the mount's graph has no water
  case; `collision.md` §11.1 step 9 gives a mounted rider in water the Swimming layer.
  **Decision:** the walker refuses to start summoning in water, and a mounted rider whose
  `Entity_Element` becomes 1, 2 or 3 is dismounted by "other" (status 6, no wait).
  > **OPEN:** what the original does when a mounted character enters deep water (server).
- **Air.** Summoning is cancelled by a jump or a fall (`Induction_AllowedWhileMoving` false);
  mounted jumps follow `collision.md` §11.10 (mounted branch: `AirTime` 0.285, 7.275 m/s) with
  the airborne clips of §14.2–§14.3.
- **Areas.** **Decision:** before summoning, the walker applies the area, dungeon and mask
  checks of §4.7 steps 5–6 to the feet's area (outdoors `Area_CurrentArea` from the walker's
  area lookup; in a cell its `Area_DID` and dungeon); a refusal ends the request with a message.
  An area entered while mounted does not dismount (OPEN: the original's rule is server-side).
- **Shapeshift, combat.** The walker's avatar is never shapeshifted and never in combat
  (`walker.md` §10.6), so those refusals do not arise.

<a id="s14-13"></a>
### 14.13 Worked examples (synthetic)

**Gear rule (§14.6).** *F* = 8.2 m/s, *a*y = 9 m/s², *G* = 2.0, speed multipliers Walk 0.5, Trot
0.7, Canter 0.8, Gallop 1.0, world limit 16, steps of 1/60 s. Forward is held from rest at
*t* = 0: *s* = 4 at once (limit 4.1 m/s); *s* = 5 at *t* = 0.5 s (limit 5.74), 6 at 1.0 s
(6.56), 7 at 1.5 s (8.2). Speed rises at 9 m/s² to each limit: 4.1 m/s at *t* ≈ 0.456 s. Gaits:
4 from the first step with *v* > 0.0002; at *t* = 0.6 s, *v* = 4.1 + 9 × 0.1 = 5.0 ≤ 0.7 × 8.2 +
0.25 = 5.99, but 5.0 > 0.5 × 8.2 + 0.25 = 4.35, so gait 5 (`Trot`); at *t* = 1.7 s, *v* = 8.2,
gait 7. Releasing the key keeps 8.2 m/s and gait 7. Holding back: the target is −*B*, the speed
falls by 9 m/s² (0 after about 0.91 s), the gait follows it down (6 below 6.81, 5 below 5.99, 4
below 4.35, then 2 at |*v*| ≤ 0.0002 and 1 when *v* turns negative), and *s* falls one gear per
0.5 s to 4.

**Placement (§14.8).** A rider with model scale 0.7 (River-hobbit), bone 0 at its reference
height 0.9953 m, `Mount_Connection` (0, −0.0075, −0.9953) on bone 0, node `BoneOffset`
(0, 0, 0), stored scale 1.0, `UseParentScale` 1: the mount's origin is at the rider's feet and
the mount is scaled 0.7 on every axis. The same rider on a medium war-steed node with
`BoneOffset` (0, 0.05, −0.135) and offset clip 0x050054F5 (bone 0 0.135 m up and 0.065 m
forward of its reference position, no rotation): the mount's origin lies 0.0003 m below the
feet and 0.115 m forward of them in the rider's model frame (0.065 + 0.05), which the rider's
0.7 model scale turns into 0.081 m in the world.

---

<a id="s15"></a>
## 15. Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (spec writer). Status Review. Sources OBS, SCR, BC, BIN (questions m1–m5). |
| 2026-10-09 | Revision (spec writer), answering the two implementer OPENs of 2026-10-09. §4.4: the MeshFX `Scale` is applied per axis, with the rider's final model scale per axis (`appearance.md` §6.1); census of every MeshFX node's scale (SCR2, MT31); §11.4 example per axis. §5.3: strafe slots paired with `StrafeLeftMoveSpeed` (*S*1) and `StrafeRightMoveSpeed` (*S*2) from the field registration and accessors (BIN2 question m6, MT32). §13: proposal for `collision.md` §11.6. Status Revised. |
| 2026-10-11 | P5 revision (spec writer, dirty room). New §14 (mounted presentation and gaits): the two-entity evaluation with the mount reading the rider through `MeshFXScriptOutput_Source` targets (§14.1), rider and mount clips by motion with a census over all 722 effects × 15 race-table entries and all 605 (entity, type) pairs (§14.2–§14.3), war-steed gaits (§14.4), how the walker finds the clips (§14.5), the gait rule for normal mounts and Tessera's gear rule for war-steeds (§14.6, MQ9), the summon, mounting and dismount sequence with the induction records and the persistent-Run finding (§14.7, MQ4), seat, bone offset, scale and per-frame placement (§14.8, MQ5 partly), camera (§14.9), mounts per body (§14.10), census by category (§14.11), water, air and areas (§14.12), examples (§14.13). Sources OBS3, SCR3 (no M4); rows MT33–MT47; invariants MI14–MI18; MQ13–MQ15; pointers in §4.1, §4.4, §4.9, §5.8; §13 rows; review notes N1 (§4.4), N2 (§13), N3 (§1) of 2026-10-09 applied. The change log moves to §15. Status Accepted → Revised. |
| 2026-10-13 | P5 revision reviewed by an independent spec reviewer (fresh session, not the writer): **passed** against CLEAN_ROOM.md §5.3 (all items), with an M1 spot-check (19 §14 DIDs resolve in the install) and the §14.11/§14.2/§14.7 arithmetic re-checked. No content change; status Revised → Accepted. See the review record. |
| 2026-10-14 | Revision (spec writer, dirty room), answering the P5 implementer OPENs carried in `walker.md` §16.12. §14.6 item 4: the crossing step (forward to backward within one step) gives Neutral, a Decision that the walker on main already applies (no behaviour change). §14.6 new "Gait 3": no data writer of gait 3 except the mirror to the mount (MT49), so it can come only from the server. §14.6 new "Jumping while mounted": the gates and nodes of the mounted branch of `Loc_Jumping` (war-steed branch at gaits 4–7, Default branch with `Slider_Propel` > 0.5 and `Slider_Strafe` within ±0.25, 7.275 m/s; MT50), `Mount_Movement_Jump_Modifier` stored nowhere and a driven field keeping its stored value when the source has none (MT51, BIN3 (m7)); this **changes behaviour implemented on main** through `walker.md` §16.6 (the walker had no mounted jump). The MQ9 OPEN is narrowed (server's gait-3, gear and jump-modifier rules). Provenance: OBS4, SCR4, BIN3 (M4, decision 0011), MT48 (offered lists, for `walker.md` §16.2), MT49–MT51. No section number or anchor changed. Status Accepted → Revised. |
| 2026-10-14 | §14.6 revision of commit e5ce2c6 reviewed by an independent spec reviewer (fresh session, not the writer): **passed** against CLEAN_ROOM.md §5.3 (all items), with M1 re-counts of MT48–MT51 and a re-read of BIN3 on a private copy of the analysed project. No content change; status Revised → Accepted. The behaviour change for main stated in the previous row (the mounted jump through `walker.md` §16.6) stands. See the review record (two non-blocking notes). |
| 2026-10-23 | Revision (spec writer, dirty room; own M1 probes of the install with the repository's clip resolver and script decoder): §14.3 gains the walking and running back clips per family (MT52; the "Back" column is the walking one), answering the observation carried in `walker.md` §16.12; §14.7 states the held-implement types, hand locations and the content of `Equipment_Hide` (MT53) and confirms the 0.4 s start Timer and the dismount's bow runs and 1.5 s Timer the walker relies on. No other fact changed. Status Accepted → Revised, awaiting independent review. **No implemented behaviour changes** (the walker takes clips from the resolver; its hide-none Decision is unchanged). |
| 2026-10-23 | Independent review (spec-review:spec-gameplay:2 (independent session)) of the 2026-10-23 revision: **passed**, CLEAN_ROOM.md §5.3 every item; status Revised → Accepted. Header status and review record only; no other text changed; no implemented behaviour changes beyond those the revision row states. |
