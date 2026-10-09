<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Mounts and riding: mount effects, mount entities, rider presentation, mounted movement, war-steed stats and landblock riders

| | |
|---|---|
| **Status** | **Revised** (2026-10-09; was Accepted 2026-10-09). Revised the same day to answer the two implementer OPENs of 2026-10-09 (§4.4 scale per axis, §5.3 strafe slots); needs a fresh review of the changed sections. |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session), not the author |
| **Review record** | 2026-10-09, spec reviewer (fresh session): passed (CLEAN_ROOM.md §5.3; 7 OBS/BIN facts re-checked independently). Non-blocking notes in the review log. |
| **Implements into** | `crates/props` (typed views of the mount effect, §2, and of the template mount and war-steed properties, §6); `crates/physics` (the war-steed controller, §5.3–§5.8, next to the controller of `collision.md` §11); `crates/scriptgraph` and `apps/client` (rider presentation through the script tables, §4; the walker's summon and dismiss, §4.9); `apps/explorer` (mount card and mount viewer, §10); Tessera's own server (summoning, durability, war-steed stats, landblock riders, §2.4, §6, §7) |
| **Companion specs** | [`combat-data.md`](combat-data.md) §3 (two halves), §5 (modifier entries), §6 (effects; class 2459 is listed there as "Mount"), §7 (skills); [`game-controls.md`](game-controls.md) §5.1 (MountDirectory), §5.2 (MountAppearanceTraits), §5.3 (MountedCombatControl, gait multipliers); [`collision.md`](collision.md) §10–§11 (movement parameters, layer choice, the player controller, the movement input, turning, the jump); [`script-table.md`](script-table.md) §4–§8 (nodes, driven properties, targets, MeshFX layout); [`script-runtime.md`](script-runtime.md) (how node graphs run); [`setup.md`](setup.md) and [`skeleton-anim.md`](skeleton-anim.md) §8 (holding locations); [`appearance.md`](appearance.md) §7 (instance sets, NPC figures); [`creatures.md`](creatures.md) §2, §3.2.1, §6 (templates, mirror properties, spawning); [`landblock.md`](landblock.md) §4.8; [`misc-records.md`](misc-records.md) §13.1 (PropertyMetaData); [`property-system.md`](property-system.md) §7.3 (bit-to-key rule), §8 (DID maps); [`wlib-bytecode.md`](wlib-bytecode.md) (the class library's code) |

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
  §12).

Game text is never reproduced. Records are named by DID, fields by property label, values by
enum-mapper label; those labels are interoperability identifiers (they are how a client finds
the records and fields at run time) and are quoted for that reason only. Every count and DID
is an observation of this install and is read at run time.

The network protocol, login, keys, the launcher and anti-tamper are out of scope (decision
0004, `docs/LEGAL.md` §9.3). Where the original's behaviour depends on server messages, this
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

Property types are those of `property-system.md` §3. "PID" is a property-ID value. Enum and
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
script hold no entity-driven MeshFX node.

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
| MQ4 | How the rider fields reach the rider and what starts `Mounted_Player_Cosmetic_Start` for a normal mount (likely server-driven). | §4.1 |
| MQ5 | The MeshFX node's run-time composition of holding location, bone offset, rotation offsets, scale flags and fade times (`script-runtime.md` OPEN-17). | §4.4 |
| MQ6 | How `Movement_Effect_MountSpeedMultiplier` enters a normal mount's speed. | §5.1 |
| MQ7 | The world limit's value when the world has none. | §5.5 |
| MQ8 | The mouse-steering thresholds of the war-steed's turning. | §5.6 |
| MQ9 | Who shifts the war-steed's gait and how the gear-change rate and `MountMovement_LastSpeedChangeTime` are used. | §5.8 |
| MQ10 | How the 133 riders whose templates name no mount entity are drawn. | §6.6 |
| MQ11 | How the 722 linkless rider instance sets are spawned. | §7.3 |
| MQ12 | `game-controls.md` Q23 (how trait appearances reach the mount), narrowed: the war-steed MeshFX nodes take their `Appearances` from the rider's `Mount_Appearances`; the class-library functions that build that array from `Mount_Appearances_TraitDID`, `…_TraitsOverride` and `…_Slot_Colourization` (classes 3439 and 3470) were not read. | §4.2 |

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
| `collision.md` §11.6, OPEN (BIN 36) | The names of the two strafe fields need not rest on the registration order: the field registration gives each field's property ID with an accessor, and the accessors read `StrafeLeftMoveSpeed` and `StrafeRightMoveSpeed` from the third and fourth float after `ControlAcceleration`, directly after `ForwardMoveSpeed` and `BackwardMoveSpeed` (mounts.md MT32, BIN2). Which of the two the player controller's item 1 reads for *x* > 0 should be re-checked against these accessors. |

---

<a id="s14"></a>
## 14. Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (spec writer). Status Review. Sources OBS, SCR, BC, BIN (questions m1–m5). |
| 2026-10-09 | Revision (spec writer), answering the two implementer OPENs of 2026-10-09. §4.4: the MeshFX `Scale` is applied per axis, with the rider's final model scale per axis (`appearance.md` §6.1); census of every MeshFX node's scale (SCR2, MT31); §11.4 example per axis. §5.3: strafe slots paired with `StrafeLeftMoveSpeed` (*S*1) and `StrafeRightMoveSpeed` (*S*2) from the field registration and accessors (BIN2 question m6, MT32). §13: proposal for `collision.md` §11.6. Status Revised. |
