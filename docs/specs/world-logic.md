<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# World logic: quest-state phasing, logic boxes, world-event wiring, travel hooks, property responses and version objects

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (fresh dirty-room session, not the writer) |
| **Review record** | 2026-10-09, spec reviewer: passed §5.3 checklist and implementability; 7 independent spot checks (DICT, OBS, BIN) agree; non-blocking notes in the dirty-room review log. Result: Accepted. |
| **Implements into** | `crates/formats` / `crates/props` (typed views of the property groups of §3), `crates/world` and `apps/client` (quest-state phasing of placed and spawned objects, §4; quest-state script tables, §5), `crates/net` sandbox and `apps/server` (quest journal input to §4–§5; the server-side logic objects of §6–§9 as data), `apps/explorer` (gating and wiring view of a landblock, §13) |
| **Companion specs** | [`landblock.md`](landblock.md) §4.5 (static entities), §4.6 (links and the link-type codes), §4.8 (the instance property sets, there called weenies). [`entity-descriptor.md`](entity-descriptor.md) §4.5.2 (per-instance sets), §4.9 (which SEDs the client creates), §9.2 (an entity's property lookup, its draw rule and `Render_Draw`). [`quests.md`](quests.md) §7.9 (scene records, class 1468), §8.2–§8.3 (quest-requirement entry rule, status codes), §14 (world-event records, classes 1815 and 1887). [`interaction.md`](interaction.md) §3.4.4 (mouse-over eligibility, `QuestState_PhasedOut`, `ClientIsSelectableOverride`), §5 (controller entities: relays, tripwires, world-property boxes, generators). [`instance-scripts.md`](instance-scripts.md) (per-instance script tables and member overrides). [`script-table.md`](script-table.md) §5 (node kinds: 3 EntityVFX, 27 GameCallback, 41 MeshFX, 58 Appearance, 59 Particle, 1 Decal), [`script-runtime.md`](script-runtime.md) §6–§7 (Switch, When, absent properties), §18 (GameCallback). [`wlib-bytecode.md`](wlib-bytecode.md) §6.4, §10 (functions started by native code, game callbacks). [`map-data.md`](map-data.md) §4.8.6 (world-event states of map notes). [`misc-records.md`](misc-records.md) §13.5–§13.7 (census-only entries this spec completes). [`property-system.md`](property-system.md) §2 (dictionary), §4–§5 (values, sets), §8 (DID maps). [`instances.md`](instances.md) (private-encounter templates that name property-response maps). [`economy.md`](economy.md) (world-event filters). |

---

<a id="s0"></a>
## 0. Purpose and terms

Placed and spawned objects in the landblocks carry property groups that no other spec decodes.
They decide whether an object is drawn for *this* player (quest-state phasing), swap an
object's particles, decals, held props, appearance and idle animation by quest state, and wire
the placed trigger logic that a server evaluates (counters, timers, world-event generators,
custom conditions, skirmish bastions and control points, scene teleports, instance exits).

This spec gives an implementer:

- the dictionary facts and full censuses of these groups over every property set of the
  install (§2, §3, §4.8, §5.6, §6, §7);
- the client's **phasing-box mechanism**, the rule that decides which placed or spawned objects
  a player sees from his quest journal (§4, BIN and WLB);
- the client's **quest-state script tables**, the per-entity mechanism that swaps visuals by
  quest state (§5, data and WLB);
- the client's world-event condition test, which several specs reference (§6.7, WLB);
- layouts and references of the server-side logic objects: property-response maps and
  callbacks (§8) and the version-migration objects (§9);
- the reference graph (§10), invariants (§11), synthetic worked examples (§12), an explorer view
  (§13) and the open questions (§14).

| Term | Meaning |
|---|---|
| **instance set** | The per-instance property set of a landblock-info record (`landblock.md` §4.8), keyed by a 64-bit instance ID. "SED instance set": its instance ID is also the instance ID of a static entity descriptor (SED) of the same record. "Spawn-only instance set": no SED has its instance ID; the object exists only when a server creates it. |
| **phasing box** | An entity whose physics object is the descriptor that the UNIQUEDB DID map names `PhasingBox` (§4.1). It holds a quest check list and decides, by the player's quest state, whether the entities linked to it are shown. |
| **target** | An entity linked to a phasing box by a link of type `Phasing` (§4.2). |
| **phased out** | A target whose box decided "not shown": not drawn, not selectable, `QuestState_PhasedOut` true (§4.6). |
| **quest journal** | The player's quest state as `quests.md` §8.2 uses it: which quests are underway (at which objective), completed or failed. A server supplies it; the sandbox supplies its own (§4.9). |
| **entry rule** | The evaluation of one quest-requirement entry (operator, quest, status code) of `quests.md` §8.2, with its bypassable and disabled steps. |

Property, enum-label, class-library and DID-map names in this spec are identifiers stored in
the user's data (the property dictionary 0x34000000, enum mappers, the class library
0x56000000, DID maps). They are quoted for interoperability: an implementation looks them up by
name at run time and never hard-codes the PIDs, keys or DIDs of this install unless a section
says that a number is an algorithm constant (decision 0012).

---

<a id="s1"></a>
## 1. Provenance

| Source | Description |
|---|---|
| OBS | Observed in the user's install (Steam), 2026-10-09, read-only, data files unchanged since 2026-10-04. Throwaway probes in the dirty-room scratch area, using the project's decoders as tools, over: every one of the 49,636 landblock-info records of `client_cell_{1,2,3,4,5,14}.dat` and all their property sets (3,253,862 SED sets, 3,768,840 instance sets, 1,324,469 link sets, 104,271 cell sets, 49,636 landblock sets) and all 1,324,469 links; all 383,332 0x79 sets, all 1,616 0x78 sets and all 40,643 0x47 descriptors (with their template chains where named); all 7,524 0x07 script tables and 28,992 0x0C scriptlets that Tessera's decoder reads (2 records of the two prefixes it does not decode were not searched); the five quest-state tables of §5 dumped node by node and their EntityVFX and GameCallback node bytes read; the property dictionary, every enum mapper named here, the UNIQUEDB, WEENIECONTENT and root DID maps; the root class of every 0x70 record and the class library's ancestor lists. |
| WLB | The class library (record 0x56000000; its SHA-256 equals that of the copy disassembled) disassembled in the dirty room on 2026-10-09 with the rules of [`wlib-bytecode.md`](wlib-bytecode.md) (Accepted): class 287 function 206 (slot 291, the check-list evaluator), class 287 slot 48 (the entry rule of `quests.md` §8.2; only its return convention was read again), class 3172 slot 85 (a game callback), class 3376 slot 3 and its four callers (class 277 slot 13, class 279 slots 1 and 2, class 258 slot 54), classes 1815 and 1887 slot 87 (world-event comparators) and class 1820 slot 5 (world-event condition reader); and a search of the whole code for 32-bit pushes of every property ID of the families in §3. |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, file dated 2026-10-04; the SHA-256 recorded in the analysed project equals that of the installed file), analysed headless on 2026-10-09 in a private copy of the shared Ghidra project (decompiler and disassembler, address ranges only; the copy was deleted after use; nothing copied into this spec). **Questions:** (1) who sets `QuestState_PhasedOut` and by which rule the client decides from the player's quest state whether a placed or spawned object is drawn; (2) which properties that rule reads, from where, and what an absent value means; (3) which events make the client evaluate the rule again; (4) how the client recognises the entity that carries the rule. **Why M1–M3 were not enough:** the data hold the check lists, flags and links but not the rule that combines them, the defaults of absent values, the link orientation used, the identification of a box or the update triggers; the class library never sets `QuestState_PhasedOut` (WLB search); no published source describes the mechanism (M2); observing the client (M3) would need many quest states on one character and could not separate the box, link and default rules. **Read:** the executable's immediate references to the property IDs of the `QuestState_`, `PhasingBox` and `Phasing_` groups; the phasing manager's constructor (identification key), its handlers for entity creation, entity removal, quest change and full refresh, its box test, its target-update and state-setting routines; the property getter's use (only far enough to see that it is the entity lookup of `entity-descriptor.md` §9.2 without dictionary defaults); the default-value helper; the native call into the class library (function ID). No network, login, launcher or anti-tamper code. |
| DICT | The property dictionary and enum mappers of the install (types, defaults, mapper labels), read with the project's decoders on 2026-10-09. |

| Row | Fact | Source | Mark |
|---|---|---|---|
| WL1 | No cell set, landblock set or link set carries any property of the groups in §3; they occur only in SED sets, instance sets, 0x79/0x78 sets and two 0x47 descriptors (§2) | OBS (all sets) | Confirmed 2026-10-09 |
| WL2 | Types, defaults and mappers of the group properties (§3) | DICT | Confirmed 2026-10-09 |
| WL3 | 7,943 instance sets carry `QuestState_*`; 7,040 carry `QuestState_QuestCheck_List` (box path), 741 a top-level `QuestState_QuestID` (script path), 1 both, 163 neither; the full value census (§4.8, §5.6) | OBS (all instance sets) | Confirmed 2026-10-09 |
| WL4 | For each of the 1,063 SEDs whose instance set has `QuestState_*`, the SED's own set holds the same `QuestState_*` and `PhasingBox_*` values | OBS | Confirmed 2026-10-09 |
| WL5 | A phasing box is recognised by its physics object: the DID that the UNIQUEDB DID map gives for key 0x10000046 (label `PhasingBox`; 0x470063F2 in this install) (§4.1) | BIN (question 4); OBS (all 1,001 SEDs placing 0x470063F2 have a check list; the one template whose `PhysObj` is 0x470063F2 is labelled `PhasingBox`) | BIN; data Confirmed |
| WL6 | Phasing links: link type 0x10000019 (`Phasing`); in all 23,161 links with two non-zero ends, end 2 is a box's instance ID and end 1 the target's; boxes have no links of other types; 434 links have one end 0 (§4.2) | OBS (all links) | Confirmed 2026-10-09 |
| WL7 | The box test (§4.3): not-ready gives false; absent list gives false; absent operator means And; the list result is inverted when `QuestState_True_Draw` is false or absent | BIN (questions 1, 2) | BIN |
| WL8 | The list evaluation: And, Or, Xor over the entry rule; any other operator false; no local player false (§4.4) | WLB (class 287 function 206; called by the executable by function ID 287 × 65,536 + 206) | Confirmed (WLB) |
| WL9 | The manager's events and state updates (§4.5) | BIN (question 3) | BIN |
| WL10 | Applying a state sets `Render_Draw`, `ClientIsSelectableOverride` and `QuestState_PhasedOut`; nothing else in the data or class library sets `QuestState_PhasedOut` (§4.6) | BIN; WLB (search) | BIN |
| WL11 | Readers of `QuestState_PhasedOut` (§4.6) | WLB; OBS (script tables) | Confirmed 2026-10-09 |
| WL12 | A box's own `Render_Draw` and `ClientIsSelectableOverride` follow the local player's `CanSeeVisibilityMask` value-2 bit (§4.7) | BIN | BIN |
| WL13 | Quest-state script tables 0x070014AF, 0x070014B0, 0x070014B1, 0x070014BB, 0x070014CE: their Startup graphs and `QuestState_True` / `QuestState_False` scripts (§5.2–§5.4) | OBS (node by node) | Confirmed 2026-10-09 |
| WL14 | Their GameCallback nodes name package 0x70026F0D (class 3172), whose slot-85 body returns "the entry passes" for (`Usage_QuestID`, `Usage_QuestStatus`, `Usage_Operator`) of the call properties (§5.3) | OBS (node bytes); WLB | Confirmed (WLB) |
| WL15 | Users of those tables: 235 templates (0x79 sets) and two 0x47 descriptors that no SED places (§5.1) | OBS | Confirmed 2026-10-09 |
| WL16 | Census of counter boxes, logic-box timers, logic-box members, world-event generators, custom conditions, event lists, skirmish values (§6) | OBS | Confirmed 2026-10-09 |
| WL17 | World-event condition test: range check, operators, boolean events (§6.7) | WLB (classes 1815, 1887 slot 87; class 1820 slot 5) | Confirmed (WLB) |
| WL18 | Census of `Theater_TeleportationSceneID`, `Travel_LeavePrivateInstanceLayerArray`, `HousingSystem_IsNeighborhoodWaypoint`, `PathMap_*` (§7) | OBS | Confirmed 2026-10-09 |
| WL19 | Property-response objects: 419 in six classes plus 4 templates, layouts and references (§8) | OBS | Confirmed 2026-10-09 |
| WL20 | Version objects: master list (class 1623), subtree 1621 with 158 objects, 156 of them listed, 2 always-run, 123 with `Version_*` data (§9) | OBS | Confirmed 2026-10-09 |
| WL21 | Behaviour of counters, timers, generators, custom conditions, event lists, property responses and version objects is not in the client's class library or executable as a rule (beyond §6.7) | WLB (search); BIN (search of immediate PIDs; the few hits checked are error codes that equal a PID by chance) | Confirmed for this build; behaviour server-side (OPEN §14) |
| WL22 | Empty-journal evaluation counts (§4.9) | Tessera computation over OBS data with the rules of §4.3–§4.4 | Computed 2026-10-09 |

---

<a id="s2"></a>
## 2. Where the properties live (Confirmed, WL1, WL4)

| Location | Sets | Sets with any group of §3 | Notes |
|---|---|---|---|
| Instance sets, spawn-only | 3,768,840 instance sets in all | 23,675 | All counters, timers, logic-box members, generators, custom conditions, path-map volumes, property-response maps and skirmish values are here. |
| Instance sets of SEDs | (same total) | 3,599 | Quest state (1,063), theater (926), travel exits (887), housing waypoints (892), world-event conditions of usage permissions (48). |
| SED own sets | 3,253,862 | 3,599 | The same SEDs as the row above, with the same values (WL4). An entity built from a SED uses its instance set and ignores the SED's own set (`entity-descriptor.md` §9.2), so the duplicate never changes a result. |
| Link sets, cell sets, landblock sets | 1,324,469 / 104,271 / 49,636 | 0 | |
| 0x79 sets of 0x70 templates | 383,332 | see §3.12 | Template values (for example the `PhasingBox` template, §4.7). |
| 0x47 descriptors | 40,643 | 2 (`PathMap_*` defaults) | Plus 2 descriptors that name a quest-state table (§5.1). |

The SED rows are on SEDs of type 7 Physics (3,491), 9 Light (107; housing-waypoint, theater and travel values only) and 16 TeleportDestination (1; a theater scene) (`entity-descriptor.md` §3).

---

<a id="s3"></a>
## 3. Dictionary facts (Confirmed, WL2)

Types are those of `property-system.md` §4. "Default" is the dictionary default (absent when
the dictionary has none). Whether a reader uses the default is stated where a rule reads the
property (§4.3, §5.5); the box test does **not** use it.

<a id="s3-1"></a>
### 3.1 Quest check (box path)

| Property | Type | Default | Notes |
|---|---|---|---|
| `QuestState_QuestCheck_List` | Array of `QuestState_QuestCheck_Struct` | — | The box's quest checks |
| `QuestState_QuestCheck_Struct` | Struct {`QuestState_QuestID`, `QuestState_QuestStatus`, `QuestState_QuestOperator`} | — | One check (an entry of `quests.md` §8.2) |
| `QuestState_QuestCheck_Operator` | Enum, mapper 0x23000046 | 11 | How the checks combine: 11 `And`, 3 `Or`, 12 `Xor` are the values the client acts on (§4.4). The mapper is a general operator mapper (it also labels 0 `Invalid`, 1 `Difference`, 2 `Divide`, 4 `Random`, 5 `Set`, 6 `Subtract`, 7 `Add`, 8 `Multiply`, 10 `ArrayAdd`). |
| `QuestState_QuestID`, `…_QuestID_2` | DID | — | A quest record (class 1398 in all non-null values) |
| `QuestState_QuestStatus`, `…_QuestStatus_2` | Enum, mapper 0x23000017 | 0 | Status code of `quests.md` §8.3 |
| `QuestState_QuestOperator`, `…_QuestOperator_2` | Enum, mapper 0x23000039 | 0 | 0 `Undef`, 1 ≥, 2 ≠, 3 =, 4 <, 5 ≤, 6 > (`quests.md` §8.2) |
| `QuestState_True_Draw` | Bool | true | Box: polarity of the result (§4.3). Script path: drawn after a passing test (§5.4). |
| `QuestState_False_Draw` | Bool | false | Script path only: drawn after a failing test (§5.4). The box test never reads it. |
| `QuestState_PhasedOut` | Bool | false | Run-time state set by the client (§4.6); stored in no set of this install |
| `PhasingBox_UpdateOnlyOnCreation` | Bool | false | §4.5 step B3 |
| `PhasingBox_DelayRefreshOfLinkedEntities` | Bool | false | §4.5 event E3 |
| `PhasingBoxOverride` | Bool | false | Stored nowhere; not read by the traced path |
| `Phasing_ForcePhasingRefresh_OnEnterWorld` | Bool | false | On a target: §4.5 event E1 step 2. Stored in one 0x79 template (0x7902EBDE), in no instance set. |
| `Phasing_DebugString`, `Phasing_Debug_ShowingLinks` | String, Bool | —, false | Stored nowhere |
| `QuestState_Worldbuilder_Active` | Bool | false | Stored nowhere |

<a id="s3-2"></a>
### 3.2 Quest-state visuals (script path, §5)

| Property | Type | Default | Read by (§5.4) |
|---|---|---|---|
| `QuestState_Test` | Enum, mapper 0x23000329: 0 `Undef`, 1 `True`, 2 `False` | 0 | Startup graphs: 1 forces the passing branch, 2 the failing one |
| `QuestState_NPC` | Bool | false | Idle selection in three avatar animation tables (§5.7) |
| `QuestState_True_Particle`, `…_False_Particle` | DID (0x39 particle system in all values) | null | Particle node |
| `QuestState_True_DecalMaterial`, `…_False_DecalMaterial` | DID (0x31 surface record in all values) | null | Decal node |
| `QuestState_True_DecalSize`, `…_False_DecalSize` | Waveform | constant 1.0 | Decal node size |
| `QuestState_True_Cosmetic1_Item`, `…Cosmetic2_Item` (and `False_`) | DID (0x47 in all values) | null | MeshFX node: the held prop |
| `QuestState_True_Cosmetic1_HoldingLocation`, `…Cosmetic2_…` (and `False_`) | Enum, mapper 0x23000068 (base 0x230000FE, holding locations) | 0 | MeshFX node: where the prop is held (values seen 0, 1 `RHand`, 2 `LHand`, 0x1000000E) |
| `QuestState_True_IdleAnimation`, `…_False_IdleAnimation` | DID (0x0C scriptlet in all non-null values) | — | Scriptlet node |
| `QuestState_True_Selectable`, `…_False_Selectable` | Bool | true / false | A SetProperty node under a zero-probability parent: never runs in this install (§5.4) |
| `QuestState_True_Apr1…4_AprFile`, `…_Key`, `…_Modifier` (and `False_`) | DID (prefix 0x20), Enum (mapper 0x2300008A), Float | —, 0, — | Appearance nodes |

<a id="s3-3"></a>
### 3.3 Counter boxes

| Property | Type | Default |
|---|---|---|
| `CounterBox_Active` | Bool | true |
| `CounterBox_MaxCount` | Int | 1 |
| `CounterBox_CurrentCount` | Int | 0 |
| `CounterBox_IncrementBy`, `CounterBox_DecrementBy` | Int | 1, 1 |
| `CounterBox_Condition1` … `CounterBox_Condition5` | Int | 0 |
| `CounterBox_ResetOnMaxCount`, `CounterBox_DeactivateOnMaxCount` | Bool | false, false |

<a id="s3-4"></a>
### 3.4 Logic-box timers and logic-box members

| Property | Type | Default |
|---|---|---|
| `LogicBoxTimer_Active` | Bool | true |
| `LogicBoxTimer_Looping` | Bool | false |
| `LogicBoxTimer_MinTimer`, `LogicBoxTimer_MaxTimer` | Float (seconds presumably; OPEN §14) | 0.0, 0.0 |
| `LogicBox@Entity.m_eCondition1` … `m_eCondition6`, `LogicBox@Entity.m_eCurrentState` | Bitfield32, mapper 0x2300015F: bits 1 `A`, 2 `B`, 3 `C` | — |
| `LogicBox@Entity.m_eResetConditions` | Bitfield32, mapper 0x23000160: `Condition1` … `Condition6` | — |

The `LogicBox@Entity.*` names are member overrides (`instance-scripts.md` §5.1).

<a id="s3-5"></a>
### 3.5 World-event wiring

| Property | Type | Default | Notes |
|---|---|---|---|
| `WorldEventGenerator_ResponseMap_Array` | Array of `WorldEventGenerator_ResponseMapping` | — | |
| `WorldEventGenerator_ResponseMapping` | Struct {`WorldEventGenerator_WorldEvent` DID, `WorldEventGenerator_ResponseCallback` DID} | — | §6.4 |
| `WorldEvent_CustomConditionArray` | Array of `WorldEvent_CustomCondition` | — | §6.5 |
| `WorldEvent_CustomCondition` | Struct {`WorldEvent_CustomConditionLink`, and one of `WorldEvent_Condition`, `WorldEvent_AllConditionList`, `WorldEvent_AnyConditionList`} | — | |
| `WorldEvent_CustomConditionLink` | Int | 0; dictionary minimum 1, maximum 6 | |
| `WorldEvent_Condition` | Struct {`WorldEvent_WorldEvent`, `WorldEvent_Operator`, `WorldEvent_ConditionValue`, `WorldEvent_Condition_WorldEventToCompareWith`, `WorldEvent_Condition_UsageString`} | — | `quests.md` §14.2; test in §6.7 |
| `WorldEvent_ConditionValue` | Int | −1 | |
| `WorldEvent_Operator` | Enum, mapper 0x23000039 | 0 | As `QuestState_QuestOperator` |
| `WorldEvent_AllConditionList`, `WorldEvent_AnyConditionList` | Array of `WorldEvent_Condition` | — | |
| `WorldEvent_` *X* `EventList` for *X* in `Activate`, `Deactivate`, `Open`, `Close`, `Lock`, `Unlock`, `Use`, `Reset`, `Trigger`, `Toggle`, `LightFuse`, `CancelFuse`, `CancelCountdown`, `PurgeDeactivate`, `PurgeReset`, `Enable`, `Disable` | Array of `WorldEvent_WorldEvent` | — | §6.6 |
| `WorldEvent_FireOnInitialize` | Bool | false | §6.6 |
| `WorldEvent_ResponseType` | Enum, mapper 0x23000291 | — | 1 `Close`, 2 `Deactivate`, 3 `Open`, 4 `CancelFuse`, 5 `Lock`, 6 `Use`, 7 `Reset`, 8 `Activate`, 9 `Unlock`, 10 `PurgeDeactivate`, 11 `Trigger`, 12 `PurgeReset`, 13 `Toggle`, 14 `CancelCountdown`, 16 `LightFuse`, 19 `Enable`, 20 `Disable`; also the type of the response-type fields of §8.2. Stored nowhere in a set; `WorldEvent_ActionList` (which pairs a list PID with a response type) is stored nowhere either. |
| `WorldEvent_StateToTestValue` | Int | −1 | Read by the condition reader (§6.7); stored in no instance set |

<a id="s3-6"></a>
### 3.6 Skirmish bastions and control points (instance-set members)

| Property | Type | Default |
|---|---|---|
| `Skirmish_Bastion_WorldEvent_Boolean`, `Skirmish_Bastion_WorldEvent_Integer` | DID | — |
| `Skirmish_Bastion_Counterattack_1…3_Delay` | Float | — |
| `Skirmish_Bastion_Counterattack_1…3_Probability` | Int | 100 |
| `Skirmish_ControlPoint_Banner_Camp_Number` | Int | — |
| `Skirmish_ControlPoint_FinalCP`, `Skirmish_ControlPoint_Banner_Flippable` | Bool | — |
| `Skirmish_ControlPoint_Banner_FriendlyApr`, `…_EnemyApr` | Float | — |

Other `Skirmish_` members stored in instance sets are listed with their counts in §6.8.

<a id="s3-7"></a>
### 3.7 Travel and scene hooks

| Property | Type | Default | Notes |
|---|---|---|---|
| `Theater_TeleportationSceneID` | DID | — | Scene record, class 1468 (`quests.md` §7.9) |
| `Travel_LeavePrivateInstanceLayerArray` | Array of `Entity_ContentLayer` (Enum, the content-layer mapper of `entity-descriptor.md` §4.5) | — | |
| `Travel_LeavePrivateInstance` | Bool | true | Stored in no instance set |
| `HousingSystem_IsNeighborhoodWaypoint` | Bool | — | |
| `PathMap_BlockingBoxMin`, `PathMap_BlockingBoxMax` | Vector3 | (−0.25, −1.0, 0.0), (0.25, 1.0, 1.0) | Units of the landblock frame (`coordinates.md`) presumably |
| `PathMap_BlockingCylinder_LowPoint`, `…_Height`, `…_Radius` | Vector3, Float, Float | (0, 0, 0), 1.0, 0.5 | |

<a id="s3-8"></a>
### 3.8 Property responses

| Property | Type | Notes |
|---|---|---|
| `PropertyResponse_EntityPropertyResponseMap`, `PropertyResponse_AreaPropertyResponseMap`, `PropertyResponse_ZonePropertyResponseMap` | DID | A response map (class 2192) |
| `PropertyResponse_WorldEventResponse_List` | Array of `PropertyResponse_WorldEventResponse_Entry` {`PropertyResponse_WorldEvent` DID, `PropertyResponse_Callback` DID} | |
| `PropertyResponseCallback_OnTrueResponseType`, `…_OnFalseResponseType` | Enum, mapper 0x23000291 (§3.5) | |
| `PropertyResponseCallback_AIHint` | Bitfield (any width), mapper 0x2300005C | |
| `PropertyResponseCallback_Drama` | String | |
| `PropertyResponseCallback_AppliedEffect_Array` | Array of {`…_Floor` Int, `…_Ceiling` Int, `…_AppliedEffect_List` (array of effect DIDs)} | |
| `PropertyResponseCallback_UseTargetLevelForSpellcraft` | Bool | |
| `PropertyResponse_Filter_Array` | Array of `EntityFilter_*` structs and `PropertyResponse_Filter_Alignment` (Enum 0x2300000D) | |
| `PropertyResponse_CalculateProperty_Array` | Array of {`…_SourceMod` Int, `…_Target` PID, `…_Min` Int, `…_Max` Int} | |
| `PropertyResponse_CalculateProperty_EntityType`, `PropertyResponse_MapProperty_EntityType` | Enum, mapper 0x230001AB (entity types) | |
| `PropertyResponse_MapProperty_Array` | Array of {`…_Source_Int` Int or `…_Source_Enum` (struct holding one property), `…_Target` (struct holding one property)} | The two struct fields allow any child (dictionary flag 0x40) |

<a id="s3-9"></a>
### 3.9 Version objects

677 dictionary names begin with `Version_`; the ones stored are listed per class in §9.2.

---

<a id="s4"></a>
## 4. Quest-state phasing by phasing boxes (BIN, WLB, OBS)

<a id="s4-1"></a>
### 4.1 Phasing boxes (WL5)

The client recognises a phasing box by its physics object: at start-up the phasing manager looks
up key **0x10000046** in the UNIQUEDB DID map (root DID-map key 0xF; label `PhasingBox`) and
treats every entity whose physics object is that DID as a box. In this install the entry names
0x470063F2, a descriptor of kind 7 whose own set holds no property of §3. An implementation
takes the DID from the map at run time (the key is the algorithm constant; the DID is data).

Boxes in this install:

- **1,001 Physics SEDs** place 0x470063F2. The client creates their entities itself
  (`entity-descriptor.md` §4.9). All 1,001 instance sets carry a check list.
- **6,039 spawn-only instance sets** carry a check list. They become boxes when a server creates
  their entity with that physics object; the template labelled `PhasingBox` (0x7002CD2B, class
  3369, the only one) has `PhysObj` = 0x470063F2 (§4.7).
- Every instance set with a check list is one of these 7,040 (no other entity carries one).
  They lie in 2,737 landblocks and name 2,421 distinct quests.

<a id="s4-2"></a>
### 4.2 Phasing links (WL6)

A box names its targets with landblock links (`landblock.md` §4.6) of link type **0x10000019**
(label `Phasing` in the link-type mapper). In this install:

| Links of type 0x10000019 | 23,376 |
|---|---|
| Both ends non-zero | 22,942: end 2 is a box in all, end 1 is the target |
| End 2 is a box and end 1 is 0 | 219 (the target lies in another landblock; block pair present) |
| End 1 non-zero and end 2 is 0 | 215 (the box lies in another landblock; block pair present) |
| End 1 is a box | 0 |
| Links of other types touching a box | 0 |
| Link property sets | all empty; `link_id` is not unique (only 4,906 of the IDs occur once) |

Targets: 22,926 distinct instance IDs; 22,921 have one box, 5 have two, three or eleven. Of
the 22,942 complete links, 5,669 name a Physics SED as target (the client creates it, §4.9),
17,273 a spawn-only instance set. 7,032 of the 7,040 boxes have at least one link (up to 245);
the median is one.

The client takes the target of a box from the links that the landblock holds for the box's
instance ID (BIN); with the orientation above this is end 1 for end 2 = box.

> **OPEN:** how the client joins the two halves of a cross-landblock link (one end 0 here,
> the other end in the neighbouring landblock named by the block pair); `landblock.md` §4.6
> leaves the block-pair numbers OPEN. Until settled, Tessera joins a half with end 1 = 0 to the
> half in the named landblock whose end 1 is non-zero and whose `link_id` is equal (Tessera
> rule; not checked against the client).

<a id="s4-3"></a>
### 4.3 The box test (BIN, WL7)

The test gives one boolean *B* for a box entity. All property reads use the entity property
lookup of `entity-descriptor.md` §9.2 (per-instance set, then the object chain) **without**
dictionary defaults.

1. If the manager has not yet received the player's quest state (the "ready" flag of §4.5,
   initially false), *B* is false.
2. If the box has no `QuestState_QuestCheck_List`, *B* is false (step 5 is not applied).
3. *op* = the box's `QuestState_QuestCheck_Operator`; when absent, *op* = 11 (`And`).
4. *C* = the list evaluation of §4.4 with *op*.
5. *t* = the box's `QuestState_True_Draw`; when absent, *t* = **false** (the dictionary default
   true is not used here). *B* = *C* when *t* is true, *B* = not *C* when *t* is false.

So a box with `True_Draw` true shows its targets while its checks pass; one with `True_Draw`
false or absent shows them while its checks fail. (In all 7,040 check-list sets that store
`QuestState_False_Draw`, it equals the negation of the effective *t* of step 5; the box test
does not read it.)

<a id="s4-4"></a>
### 4.4 Evaluating a check list (WLB, WL8)

The executable calls class-library function **287 × 65,536 + 206** (class 287, function 206;
`wlib-bytecode.md` §6.4) with the list and *op*. The function:

1. Takes the local player object; with none, the result is false.
2. Visits the elements in stored order. For each it reads `QuestState_QuestID` (0 when
   absent), `QuestState_QuestStatus` (0 when absent) and `QuestState_QuestOperator` (0 when
   absent) and applies the entry rule of `quests.md` §8.2 to (operator, quest, status) for the
   player.
3. Combines by *op*:
   - 11 `And`: false at the first failing element; true when all pass (an empty list gives true);
   - 3 `Or`: true at the first passing element; false when none passes (an empty list gives
     false);
   - 12 `Xor`: false at the second passing element; at the end true exactly when one element
     passed;
   - any other value: false.

The entry rule is the class-library body that `quests.md` §8.2 describes (class 287 slot 48).
It returns 0 for "passes" and a non-zero message code for "fails"; the list function and the
callback of §5.3 test it that way.

<a id="s4-5"></a>
### 4.5 The phasing manager: events and state (BIN, WL9)

The client keeps one phasing manager. Its state:

- **ready**: false at start; set by event E4;
- the **registered boxes** (entities currently present);
- the set **V** of registered boxes whose last test was true;
- a **target map** from target instance ID to box instance ID (the first box to register a
  target keeps it), and per box the **target list** (instance IDs);
- a **quest map** from quest DID to the boxes that depend on it.

**Applying a state** *v* to a target instance ID *T* means: for every entity that the client's
entity registry returns for *T* (one entity for an ordinary instance ID), set the entity's
run-time properties `Render_Draw` = *v*, `ClientIsSelectableOverride` = *v* and
`QuestState_PhasedOut` = not *v*, and pass "hidden = not *v*" to the entity's attached
children (OPEN §14). After a batch of changes the manager asks the scene to refresh.

**E1. An entity is created** (BIN; the handler of the client's general entity-creation notification, so it sees spawned entities and, by all appearances, the entities the client creates for SEDs; see the OPEN at the end of this section).

1. If its physics object is the box DID (§4.1), it is registered as a box (steps B1–B6).
2. Else, if its instance ID is in the target map: when the entity's
   `Phasing_ForcePhasingRefresh_OnEnterWorld` is true, its box is re-evaluated first (E3); then
   the state "box in V" is applied to the entity.
3. Else, if any landblock link of the entity has link type 0x10000019, the state *false* is
   applied (a target whose box is not present yet starts phased out).
4. Else nothing happens.

Registering box *X*:

- **B1.** *X*'s own `Render_Draw` and `ClientIsSelectableOverride` are set as §4.7 says.
- **B2.** *X* joins the registered boxes.
- **B3.** Unless *X*'s `PhasingBox_UpdateOnlyOnCreation` is true, *X* is entered in the quest map
  under every quest DID named by a `QuestState_QuestID` (of DID type) inside a struct element of
  its check list, once per distinct quest.
- **B4.** The test of §4.3 runs; when true, *X* joins V.
- **B5.** For every landblock link of *X*: its target *T* is added to *X*'s target list and to
  the target map (unless *T* is already mapped), and the state *B* of step B4 is applied to *T*.
- **B6.** The scene is refreshed.

**E2. The player's state of quest *Q* changes** (BIN; the manager is told the quest's DID). Every
box registered under *Q* in the quest map is re-evaluated (E3).

**E3. Re-evaluating box *X*** (BIN). If *X*'s entity is not present, nothing happens. Otherwise
the test runs; when its result differs from *X*'s membership in V, V is updated and, unless
*X*'s `PhasingBox_DelayRefreshOfLinkedEntities` is true, the new state is applied to every
instance ID in *X*'s target list and the scene is refreshed. (With the flag set, targets keep
their state until E1 or E4 reaches them.)

**E4. The player's quest state arrives** (BIN; the notification that also sets ready). Ready
becomes true; V is cleared; the test runs for every registered box that is present and fills V;
then for every target in the target map the state "its box in V" is applied; the scene is
refreshed. Neither flag of §3.1 suppresses this pass.

**E5. A box entity is removed** (BIN). Its target-map entries, target list, V membership,
registration and quest-map entries are dropped. Its targets keep their last state. Removing a
non-box entity changes nothing.

Consequences:

- A box with `UpdateOnlyOnCreation` true is evaluated only when it is created and on E4, not on
  quest changes.
- The spawn-only boxes created from the `PhasingBox` template inherit `UpdateOnlyOnCreation`
  = true unless their instance set says false (§4.7, §4.8).
- Each box is tested against the local player only: phasing is per player.

> **OPEN:** whether the entities the client creates itself for Physics SEDs (`entity-descriptor.md`
> §9.3) reach E1 through the same notification as spawned entities (expected; the creation path
> of streamed SED entities was not followed into the notification).

> **OPEN:** which client event delivers E4 (the first full quest-state message, presumably,
> and perhaps later resynchronisations) and which delivers E2. Both are fed by the network
> layer, which decision 0011 excludes; Tessera's sandbox and server send them as §4.9 says.

<a id="s4-6"></a>
### 4.6 What a phased-out target is (BIN, WLB, OBS; WL10, WL11)

- `Render_Draw` false: the entity is not drawn (`entity-descriptor.md` §9.2). The run-time
  value set here wins over stored values.
- `ClientIsSelectableOverride` false: the entity is not selectable (`interaction.md` §3.4.4 step
  1.4).
- `QuestState_PhasedOut` true, read by:
  - the mouse-over controller (class 277 slot 13; `interaction.md` §3.4.4 step 1.2): no
    mouse-over unless the player's `CanSeeVisibilityMask` has the value-2 bit;
  - class 279 slots 1 and 2: a phased-out entity plays no drama sound;
  - class 258 slot 54: a phased-out speaker produces no speech text item;
  - Switch nodes in six script tables (0x07001469, 0x070014B1, 0x07001561, 0x07001693,
    0x070017C5, 0x07001A46; for 0x07001A46 see `instance-scripts.md` §6.2);
  - all three class-library readers go through class 3376 slot 3, which reads the property
    with the default false.

Nothing else sets `QuestState_PhasedOut`: no class-library code pushes its PID for a write, no
script node sets it, no set stores it (WL10).

> **OPEN:** whether a phased-out target still collides (no collision property is set by the
> traced path; `collision.md` does not consult `Render_Draw`), and what the call that passes
> "hidden" to the entity's attached children does (it walks up to 20 attached entries; their
> kind was not traced).

<a id="s4-7"></a>
### 4.7 The box itself (BIN, OBS; WL12)

At registration (B1) the box's `Render_Draw` and `ClientIsSelectableOverride` become true
exactly when the local player object is known and its `CanSeeVisibilityMask` (Enum) has the bit
of value 2 set (the `Hidden` value, `interaction.md` §5.3); otherwise false. Normal players
therefore never see boxes. The manager also watches the local player's `CanSeeVisibilityMask`.

The `PhasingBox` template 0x7002CD2B stores `PhysObj` 0x470063F2, `Entity_CreationType` 8,
`PhasingBox_UpdateOnlyOnCreation` true, `Render_Opacity` 0.5, `IsSelectable` false and the
three ethereality bitfields (`Physics_EtherealityType`, `Physics_EtherealToType`,
`Physics_PlacementEtherealToType`) = 1.

> **OPEN:** what the manager does when the local player's `CanSeeVisibilityMask` changes after
> registration (re-applying the B1 rule to all boxes is likely; not traced).

<a id="s4-8"></a>
### 4.8 Census of the box path (Confirmed, WL3)

7,040 sets with a check list (6,039 spawn-only, 1,001 on box SEDs).

| Field | Values (count) |
|---|---|
| List length | 1 (4,677), 2 (2,182), 3 (120), 4 (29), 5 (6), 6 (20), 7 (1), 9 (4), 15 (1); never empty |
| Elements | 9,725 structs, each with all three fields |
| `QuestState_QuestOperator` in elements | 1 ≥ (2,819), 2 ≠ (711), 3 = (3,937), 4 < (190), 5 ≤ (2,028), 6 > (40); never 0 |
| `QuestState_QuestStatus` in elements | 0x30000000 completed (1,765); 0x10000000 + *n* underway at objective *n*: *n* = 0 (1,803), 1 (1,157), 2 (1,177), 3 (884), 4 (686), 5 (589), 6 (479), 7 (347), 8 (218), 9 (167), 10 (129), 11 (102), 12 (70), 13 (38), 14 (38), 15 (22), 16 (16), 17 (17), 18 (8), 19 (4), 20 (7), 22 (1), 30 (1); no other raw status |
| `QuestState_QuestID` in elements | 9,725 DIDs, all of class 1398; 2,421 distinct |
| `QuestState_QuestCheck_Operator` | absent (6,238), 11 `And` (556), 3 `Or` (246); `Xor` unused |
| `QuestState_True_Draw` | true (6,576), false (431), absent (33) |
| `QuestState_False_Draw` | false (5,228), true (463), absent (1,349) |
| `PhasingBox_UpdateOnlyOnCreation` | absent (1,224), false (5,724), true (92) |
| `PhasingBox_DelayRefreshOfLinkedEntities` | absent (1,173), false (5,708), true (159) |
| Other properties of these sets | `Render_AdjustableScale`, `Physics_AdjustableScale` (all), `Entity_ContentLayers` (3,266 spawn-only, 65 SED) |

All `PhasingBox_*` values stored anywhere are in these 7,040 sets.

<a id="s4-9"></a>
### 4.9 Tessera rules (Decisions D1–D3)

The original client never runs without a server. Tessera does, so it needs rules the original
does not have:

- **D1 (journal).** The walker and the sandbox backend send E4 once the player's world entry
  completes, with the sandbox's journal (empty unless the sandbox has quest state), and E2 for
  each later change. With an empty journal the entry rule of `quests.md` §8.2 passes for
  "≠ completed", "≠ failed" and "≠, < or ≤ underway" entries and fails for all others.
- **D2 (spawn-only boxes offline).** Without a server, no spawn-only box would ever exist, and
  every target of such a box would stay phased out (E1 step 3). Offline, Tessera treats every
  spawn-only instance set with a check list as a box that is present while its landblock is
  loaded, with physics object the box DID and no template (so `UpdateOnlyOnCreation` is false
  unless stored). With a server, the server's entities replace this rule.
- **D3 (debug view).** The explorer and a client debug switch may show boxes and links as if the
  player's `CanSeeVisibilityMask` had the value-2 bit (§4.7).

Consequence of D1 and D2 on this install (WL22, Tessera computation): with an empty journal
1,199 boxes test true and 5,841 false; of the 5,669 Physics-SED targets (complete links) 2,164
are shown and 3,505 phased out; of the 17,273 spawn-only targets 3,488 are shown and 13,785
phased out.

---

<a id="s5"></a>
## 5. Quest-state script tables (OBS, WLB)

A second, independent mechanism works through script tables on the entity itself: on creation
the entity tests one or two quest entries stored as top-level `QuestState_*` properties and then
runs a script that swaps its visuals.

<a id="s5-1"></a>
### 5.1 Tables and their users (WL15)

| Table | Startup delay | Test | `QuestState_True` / `QuestState_False` scripts do | Users |
|---|---|---|---|---|
| 0x070014AF | 0.2 s | one entry, only when `QuestState_QuestID` ≠ null | cosmetics, idle, appearances, draw (§5.4) | 204 templates, 2 0x47 descriptors (0x470053CB, 0x470053CD, as additional table) |
| 0x070014BB | 0.2 s | two entries, both must pass | as 0x070014AF | 23 templates |
| 0x070014CE | 1.0 s | one entry | opacity, then sets `QuestState_Test` | 6 templates |
| 0x070014B0 | 1.0 s | one entry | particle | 1 template |
| 0x070014B1 | 1.0 s | one entry, only while not phased out | decal | 1 template |

The 235 templates are of classes 1724 (134), 1723 (89) and 796 (12). No SED places 0x470053CB
or 0x470053CD, and no landblock set names one of the tables, so in this install the client meets
this mechanism only on server-created entities whose template names one of the tables. Script
keys: 5 `Startup`, 0x10001CF6 `QuestState_True`, 0x10001CF7 `QuestState_False`, 0x10000524
`Viewport_Idle` (labels of mapper 0x23000097).

<a id="s5-2"></a>
### 5.2 Startup graphs (WL13)

All five run their test under a Timer whose port fires after the delay of §5.1. "Run True" and
"Run False" are Run nodes (persistent) of script 0x10001CF6 and 0x10001CF7 in the same table;
"GC(a, b, c)" is a GameCallback node whose call properties `Usage_QuestID`, `Usage_QuestStatus`,
`Usage_Operator` are driven from the entity's properties *a*, *b*, *c* (§5.3), with value ports
true and false.

| Table | Graph under the timer |
|---|---|
| 0x070014AF | When `QuestState_QuestID` ≠ null → Switch (persistent) on `QuestState_Test`: 0 → GC(`QuestState_QuestID`, `…Status`, `…Operator`): true → Run True, false → Run False; 1 → Run True; 2 → Run False. Beside it, unrelated to the test: a MeshFX node and a When on `CanSeeVisibilityMask` = 3 with a second MeshFX node. |
| 0x070014BB | Switch (persistent) on `QuestState_Test`: 0 → GC(`…QuestID`, `…Status`, `…Operator`): false → Run False; true → GC(`…QuestID_2`, `…Status_2`, `…Operator_2`): true → Run True, false → Run False; 1 → Run True; 2 → Run False. The same two unrelated nodes as 0x070014AF. |
| 0x070014CE | GC(`…QuestID`, `…Status`, `…Operator`): true → Run True, false → Run False. |
| 0x070014B0 | Switch (persistent) on `QuestState_Test`: 2 → Run False; 1 → Run True; 0 → GC(…): true → Run True, false → Run False. |
| 0x070014B1 | Switch (persistent) on `QuestState_Test`: 2 → Run False; 1 → Run True; 0 → Switch (persistent) on `QuestState_PhasedOut`: false → GC(…): true → Run True, false → Run False (true: nothing). |

Script 0x10000524 `Viewport_Idle` (tables 0x070014AF and 0x070014BB): an opacity EntityVFX node
(0x070014AF: combiner `Add`, opacity 1.0; 0x070014BB: `Replace`, 1.0) and, in 0x070014BB, a
SetProperty `QuestState_Test` = 1. It is the variant shown in UI viewports.

> **OPEN (implementer, 2026-10-09):** a reader that applies §5 without running the script
> tables must know which of the five graph shapes a table has, and this section names the
> tables only by DID. How is the shape recognised from the table's own data (for example by
> the node kinds and watched properties under `Startup`)? **Tessera's choice:** the library
> exposes the five shapes (`QuestStateGraph`) and the caller chooses one; the explorer shows the
> test and both branches without choosing.

<a id="s5-3"></a>
### 5.3 The quest-entry callback (OBS, WLB; WL14)

Every GameCallback node of the five tables names package **0x70026F0D** (a game-callback object
of class 3172; its `GameCallback_ReturnValue` names `GCParam_BoolRetVal`), has the default value
`GCParam_BoolRetVal` = false, and three call properties `Usage_QuestID`, `Usage_QuestStatus`,
`Usage_Operator` stored as 0, which the node's driven pairs replace with the entity's values
(`script-table.md` §4.2; a driven pair whose source is absent keeps the stored 0,
`instance-scripts.md` §5.3).

The callback's slot-85 body (`wlib-bytecode.md` §10) returns `GCParam_BoolRetVal`:

1. false when there is no local player object;
2. otherwise true exactly when the entry rule (`quests.md` §8.2) passes for (`Usage_Operator`,
   `Usage_QuestID`, `Usage_QuestStatus`) of the call properties, each 0 when absent.

Which value port the node then fires follows `script-runtime.md` §18 (the port whose value
equals the result; OPEN-11 there).

<a id="s5-4"></a>
### 5.4 The `QuestState_True` and `QuestState_False` scripts (WL13)

Below, *P* is `True` in script 0x10001CF6 and `False` in 0x10001CF7; the numbers (1)–(5) only
label the groups of nodes (their run order follows `script-runtime.md`). A When "≠ null" tests the named property against the null DID (operator 3 `NotEqual`,
`script-runtime.md` §7).

| Table | Effect of script *P* |
|---|---|
| 0x070014AF, 0x070014BB | (1) Two MeshFX nodes whose prop and holding location are driven from `QuestState_P_Cosmetic1_Item` / `…_HoldingLocation` and `…Cosmetic2_…`. (2) A SetProperty `ClientIsSelectableOverride` driven from `QuestState_P_Selectable`, under a parent of probability 0 (it never runs). (3) When `QuestState_P_IdleAnimation` ≠ null → a Scriptlet node whose file is that DID. (4) For *k* = 4, 3, 2, 1: When `QuestState_P_Apr`*k*`_AprFile` ≠ null → an Appearance node driven from `…_AprFile`, `…_Key`, `…_Modifier`. (5) A Timer of 2.0 s, then a Switch on `QuestState_P_Draw`: true → opacity EntityVFX "shown", SetProperty `ClientIsSelectableOverride` = true, `UI_FloatyText_HideFloatingName` = false; false → opacity EntityVFX "hidden", `ClientIsSelectableOverride` = false, `UI_FloatyText_HideFloatingName` = true. |
| 0x070014CE | An opacity EntityVFX ("shown" in True, "hidden" in False) and a SetProperty `QuestState_Test` = 1 (True) or 2 (False). |
| 0x070014B0 | When `QuestState_P_Particle` ≠ null → a Particle node whose particle system is that DID. |
| 0x070014B1 | When `QuestState_P_DecalMaterial` ≠ null → a Decal node with `DecalScript_Material` from that property and `DecalScript_Size` from `QuestState_P_DecalSize`. |

The opacity nodes are EntityVFX nodes of type 3 `OpacityModify` (persistent, ramp in 0.25 s,
sustain 1.0 s, ramp out 0.25 s, local timing). "Shown": combiner `Replace` or `Multiply` with a
constant opacity 1.0; "hidden": combiner `Multiply` with a constant 0.0. The exact pairs per
node:

| Table | True script, Draw true / false | False script, Draw true / false |
|---|---|---|
| 0x070014AF | Replace 1.0 / Multiply 0.0 | Replace 1.0 / Multiply 0.0 |
| 0x070014BB | Multiply 1.0 / Multiply 0.0 | Multiply 1.0 / Multiply 0.0 |
| 0x070014CE | Multiply 1.0 (always) | Multiply 0.0 (always) |

This is how the client swaps particles, decals, props, appearances and idle animations by quest
state. The script path does not touch `Render_Draw` or `QuestState_PhasedOut`; it hides through
opacity and selectability.

<a id="s5-5"></a>
### 5.5 Absent values on the script path

Switch and When nodes read an absent property as its dictionary default (`script-runtime.md`
§6.2). Hence on this path an absent `QuestState_True_Draw` means **true** (shown after a pass)
and an absent `QuestState_False_Draw` means **false** (hidden after a failure); an absent
`QuestState_Test` means 0 (run the test). This differs from the box test (§4.3 step 5).

> **OPEN:** whether these tables are run again when the player's quest state changes. The
> Startup graph runs once at creation; its Switch nodes re-fire on changes of
> `QuestState_Test` (and, in 0x070014B1, `QuestState_PhasedOut`), which a server may send; no
> traced path re-runs the callback on a quest change by itself.

<a id="s5-6"></a>
### 5.6 Census of the script path (Confirmed, WL3)

741 instance sets carry a top-level `QuestState_QuestID` (679 spawn-only; 62 on Physics SEDs,
whose objects 0x47004E6E (36), 0x4700545B (23), 0x4700573B (2), 0x47004E71 (1) reach none of the
five tables, so these 62 have no effect through this mechanism). One spawn-only set also has a
check list.

| Property (top level) | Values (count) |
|---|---|
| `QuestState_QuestID` | quest DID of class 1398 (671; 114 distinct, 71 of them also in check lists), null (70) |
| `QuestState_QuestOperator` | 0 (231), 1 (55), 2 (26), 3 (579), 4 (6), 5 (5), 6 (1); 903 in all, 162 of them in sets without a quest ID or list |
| `QuestState_QuestStatus` | 0 (69), 0x10000000 (59), 0x10000001 (12), 0x10000002 (48), 0x10000005 (9), 0x30000000 (544) |
| `QuestState_QuestID_2`, `…Status_2`, `…Operator_2` | 24, 24, 26 (operators 0, 2, 3, 5; statuses 0x10000001, 0x10000002, 0x30000000) |
| `QuestState_Test` | 0 (124), 1 (26) |
| `QuestState_True_Draw` / `…_False_Draw` | 101 / 106 stored |
| `QuestState_True_Particle` / `…_False_Particle` | 58 / 5 |
| `QuestState_True_DecalMaterial`, `…_DecalSize` / `False_` | 104, 100 / 211, 211 |
| `QuestState_True_Cosmetic1_Item`, `…_HoldingLocation` / `False_` | 18, 18 / 2, 2; `Cosmetic2`: 2 items, 4 locations (True only) |
| `QuestState_True_IdleAnimation` / `False_` | 5 / 5 (one null each) |
| `QuestState_True_Selectable` / `False_` | 71 / 79 |
| `QuestState_NPC` | 132 (true 44) |
| `QuestState_True_Apr1_Key` | 1 (0); no set stores an `AprFile` |

163 instance sets carry `QuestState_*` but neither a top-level quest ID nor a check list: 159 hold only `QuestState_QuestOperator` = 0, 2 also `…_QuestOperator_2` = 0, 1 also `QuestState_True_Draw` false, and 1 holds operator 3, status 0x10000000 and a `QuestState_True_Particle`. They have no effect through §4 or §5.

One 0x79 template (0x79037DEA) stores script-path values itself (`QuestID`, status
0x10000004, operator 1, both draw flags true, two cosmetic items).

<a id="s5-7"></a>
### 5.7 `QuestState_NPC`

Three avatar animation tables (0x0700003B, 0x070014E8, 0x07002F5F) choose a combat idle with a
Switch on `QuestState_NPC`: true selects an empty case (no idle scriptlet), the default case plays
the idle scriptlet. Seven scriptlets mention the property too (OBS; not traced further).

---

<a id="s6"></a>
## 6. Logic boxes and placed triggers (OBS; behaviour server-side)

All sets in this section are spawn-only instance sets unless a row says otherwise. Their
entities exist only when a server creates them. Apart from §6.7, the client's class library and
executable contain no rule for these properties (WL21); a server evaluates them, mostly through
the script graphs named by the sets' `Entity_InstanceScriptTable` (`instance-scripts.md`) and
through 0x07 tables that read and write `CounterBox_CurrentCount` and the other members.

<a id="s6-1"></a>
### 6.1 Counter boxes

1,836 sets carry `CounterBox_*`; 1,830 also carry an instance script table and 1,405
`Entity_ContentLayers`; two also carry a world-property box with custom conditions.

| Property | Count | Values |
|---|---|---|
| `CounterBox_MaxCount` | 1,823 | 1 (43), 2 (477), 3 (595), 4 (218), 5 (73), 6 (115), 7 (39), 8 (64), 9 (10), 10 (37), 12 (40), 20 (32), 24 (12), others 11–100 (68) |
| `CounterBox_Active` | 609 | true 538, false 71 |
| `CounterBox_DeactivateOnMaxCount` | 659 | true 442, false 217 |
| `CounterBox_ResetOnMaxCount` | 486 | true 263, false 223; both flags true in 87 sets |
| `CounterBox_IncrementBy` | 213 | 1 in all |
| `CounterBox_DecrementBy` | 30 | 1 (29), 0 (1) |
| `CounterBox_Condition1` … `5` | 477, 202, 147, 82, 53 | Integers 0–35 (one 995). Present sets: none (1,351), {1} (276), {1,2} (54), {1,2,3} (65), {1,2,3,4} (36), {1–5} (46), {2} (1), {5} (7). In 1,822 sets every stored condition ≤ the effective max count (14 exceed it); stored conditions are in ascending order in all but 15 sets. Commonest: Condition*k* = *k* (1: 161, 2: 127, 3: 94, 4: 63, 5: 43). |
| `CounterBox_CurrentCount` | 0 | Run-time only (written by script graphs) |

> **OPEN:** the counter semantics: when the count changes, what reaching `MaxCount` does with
> the two flags, and what a condition value selects (by the data, condition *k* is a count at
> which output *k* fires). A server-side rule; not in the client.

<a id="s6-2"></a>
### 6.2 Logic-box timers

247 sets (246 with an instance script table, 187 with content layers).

| Property | Count | Values |
|---|---|---|
| `LogicBoxTimer_MaxTimer` | 247 | 1–7,200 (commonest 25 (37), 15 (32), 30 (32), 10 (22), 60 (21), 45 (16), 600 (15)); one −5 |
| `LogicBoxTimer_MinTimer` | 245 | 1–7,200 (commonest 5 (36), 30 (31), 20 (26), 25 (26), 60 (19), 45 (15), 15 (14), 400 (14)); one −5 |
| `LogicBoxTimer_Looping` | 210 | true 166, false 44 |
| `LogicBoxTimer_Active` | 170 | true 125, false 45 |

Min ≤ max in 245 sets (equal in 125), min > max in 2.

> **OPEN:** the timer rule (a random delay between min and max, presumably, repeated when
> looping) and its unit. Server-side.

<a id="s6-3"></a>
### 6.3 Logic-box members

50 sets carry `LogicBox@Entity.m_eCondition1` (bits 0x3 in 43, 0x7 in 5, 0x1 and 0x2 once each),
3 `…m_eCondition2` (0x0, 0x0, 0x2), 2 `…m_eCondition3` (0x0, 0x3), 1 `…m_eResetConditions`
(0x1); all 50 have an instance script table, 13 `Entity_MonitorScripts`. The members are
mentioned in 2–10 0x07 tables each (OBS; `instance-scripts.md` §5.2 counts the readers of the
family).

<a id="s6-4"></a>
### 6.4 World-event generators

5,030 sets carry `WorldEventGenerator_ResponseMap_Array` (22 empty, 2,075 with one mapping, 2,933
with two; 7,941 mappings). In every mapping the world event is an integer event (class 1887;
2 distinct) and the callback a property-response callback of class 2517 (4,928) or 2518 (3,013)
(§8.2). The sets are generators: 4,751 carry content layers, 3,019 an instance script table, 1,763
`Generator_Skirmish_Multiple_Wave`, 753 `Generator_ProfileDID`, 898 `AI_LinkedCombat`.
Two templates (classes 2516 here, 0x7901946B and 0x79057353) store mappings too (`interaction.md`
§5 lists the generator class).

Reading of the data: when the named integer event changes, the callback recomputes generator
properties from it: class 2517 computes `Generator_MinLevel` / `Generator_MaxLevel` within 1–200,
class 2518 maps `Skirmish_GroupSize` values to `Generator_ProfileDID` values (§8.2).

> **OPEN:** the exact computation of a class-2517 callback (target = event value +
> `SourceMod`, clamped to [`Min`, `Max`], is the obvious reading) and when it runs.
> Server-side.

<a id="s6-5"></a>
### 6.5 Custom conditions of world-property boxes

3,637 sets carry `WorldEvent_CustomConditionArray`; each also carries `WorldPropertyBox_WorldEvent`
(of the 10,867 world-property boxes, `interaction.md` §5). 64 arrays are empty; the other 3,573
hold 1–6 entries (836 one, 616 two, 1,616 three, 145 four, 151 five, 209 six; 9,505 entries).

| Field of an entry | Census |
|---|---|
| `WorldEvent_CustomConditionLink` | 1 (3,453), 2 (2,749), 3 (2,110), 4 (526), 5 (458), 6 (209). Within an array: unique in 3,626 arrays, ascending in 3,458, exactly 1…*n* in 3,452. |
| Condition form | `WorldEvent_Condition` (9,234), `WorldEvent_AllConditionList` (219: one condition 179, two 40), `WorldEvent_AnyConditionList` (52: one 22, two 30); exactly one form per entry |
| Conditions (all 10,672 `WorldEvent_Condition` structs of instance sets, including those of usage permissions and map notes) | event class 1887 in 9,896, 1815 in 776; operator 3 in 9,739 (1: 389, 2: 417, 4: 64, 5: 40, 6: 23); values −1 … 1,750,000 (0: 1,622, 1: 3,319, 2: 2,678, 3: 1,048, …); `WorldEvent_Condition_WorldEventToCompareWith` (class 1887) in 203; `WorldEvent_Condition_UsageString` in 258 |

`WorldPropertyBox_IgnorePropertyAdded` is stored in 41 world-property boxes (true 17).

> **OPEN:** how a world-property box uses its custom conditions: which output a link number
> selects (by the data, link *k* is output *k* of the box), when the conditions are evaluated
> (on each change of a named event, presumably) and what the box's own `WorldPropertyBox_WorldEvent`
> receives. Server-side; the client's class library has no reader of the custom-condition
> properties.

<a id="s6-6"></a>
### 6.6 Event lists of controllers

1,712 instance sets carry one or more world-event lists (198 generators, 56 usable objects, 7
containers, 1,451 others; 1,002 of the others have an instance script table, 274
`AI_LinkedCombat`). Each list holds boolean world events (class 1815) except 4 entries of
`ActivateEventList` (class 1887).

| List | Sets | Entries | Lengths |
|---|---|---|---|
| `WorldEvent_ActivateEventList` | 762 | 777 | 0 (49), 1 (688), 2 (16), 4–7 (9) |
| `WorldEvent_DeactivateEventList` | 494 | 726 | 0 (13), 1 (245), 2 (228), 3–4 (8) |
| `WorldEvent_PurgeDeactivateEventList` | 330 | 434 | 0–3 |
| `WorldEvent_CancelFuseEventList` | 276 | 311 | 0–3 |
| `WorldEvent_LightFuseEventList` | 101 | 104 | 0–3 |
| `WorldEvent_PurgeResetEventList` | 79 | 70 | 0–1 |
| `WorldEvent_CloseEventList` | 67 | 112 | 0–3 |
| `WorldEvent_OpenEventList` | 46 | 49 | 0–2 |
| `WorldEvent_UnlockEventList` | 25 | 34 | 1–2 |
| `WorldEvent_TriggerEventList` | 18 | 16 | 0, 1, 3 |
| `WorldEvent_ResetEventList` | 8 | 15 | 1–2 |
| `WorldEvent_LockEventList` | 4 | 4 | 1 |
| `WorldEvent_FireOnInitialize` | 333 | — | true 330, false 3 |

The 48 SED instance sets with `WorldEvent_*` properties hold only conditions inside
`Usage_WorldEvent_AllConditionList` (usage permissions, `interaction.md`), not event lists.

> **OPEN:** whether a list names the events the controller *sets* when it performs the action
> (activate, open, light the fuse, …) or the events it *responds to* with that action; the list
> names match the response-type labels of §3.5 one to one. Server-side.

<a id="s6-7"></a>
### 6.7 The world-event condition test (WLB, WL17)

The client's class library tests a `WorldEvent_Condition` (map notes, usage permissions, quest
events; `map-data.md` §4.8.6, `quests.md` §14.2) like this:

1. The struct must be a `WorldEvent_Condition` or a `WorldEvent_Broadcast_ConditionStruct`;
   otherwise false.
2. *E* = the world event named by `WorldEvent_WorldEvent`; unknown → false.
3. When `WorldEvent_StateToTestValue` is stored, the event's state with that index is tested
   instead (no instance set stores it; OPEN on its meaning).
4. *op* = `WorldEvent_Operator` (0 when absent).
5. The target value *c*: when `WorldEvent_Condition_WorldEventToCompareWith` names an event, its
   current value (false when unknown or unreadable); otherwise `WorldEvent_ConditionValue`
   (−1 when absent).
6. Integer events (class 1887): false when *c* is below the event's minimum or above its maximum
   (`WorldEvent_MinIntValue`, `WorldEvent_MaxIntValue`); false when the current value *v* cannot
   be read; then by *op*: 3 *v* = *c*, 2 *v* ≠ *c*, 1 *v* ≥ *c*, 4 *v* < *c*, 5 *v* ≤ *c*,
   6 *v* > *c*, any other false.
7. Boolean events (class 1815): false unless *c* is 0 or 1; false when the current value cannot
   be read; then 3: *v* = *c*, 2: *v* ≠ *c*; any other operator false.

`WorldEvent_AllConditionList` holds when all its conditions hold, `WorldEvent_AnyConditionList`
when any holds (`map-data.md` §4.8.6, `quests.md` §14.1). A server that evaluates custom
conditions (§6.5) should use the same test.

> **OPEN (implementer, 2026-10-09):** step 6 compares *c* with the event's minimum and maximum;
> what an integer event without `WorldEvent_MinIntValue` or `WorldEvent_MaxIntValue` does is
> not stated. **Tessera's choice:** an absent bound does not restrict.

<a id="s6-8"></a>
### 6.8 Skirmish bastions, control points and encounter members

766 sets carry `Skirmish_*` members (635 with content layers, 434 with an instance script table,
134 with `Render_LODClass`).

| Property | Sets | Values |
|---|---|---|
| `Skirmish_Bastion_WorldEvent_Boolean` | 416 | class 1815 (9 distinct) |
| `Skirmish_Bastion_WorldEvent_Integer` | 204 | class 1887 (203; 9 distinct), null (1) |
| `Skirmish_Bastion_Counterattack_1_Delay` / `_2_` | 74 / 3 | 2–30 (5 in 44) / 0, 10, 30 |
| `Skirmish_Bastion_Counterattack_1_Probability` / `_2_` | 7 / 2 | 100 (6), 50 / 100, 75 |
| `Skirmish_ControlPoint_Banner_Camp_Number` | 154 | 0–7 |
| `Skirmish_ControlPoint_FinalCP` | 81 | true 47, false 34 |
| `Skirmish_ControlPoint_Banner_EnemyApr` | 1 | 0.02 |
| `Skirmish_SkirmishPointCalculator_ScalingInstace` | 331 | (name as stored) |
| `Skirmish_SkirmishPointCalculator_ControlPoint1…7_Value`, `…Defender1…3_Value`, `…Optional_Value` | 33, 29, 27, 19, 12, 7, 2; 12, 7, 2; 1 | |
| `Skirmish_EncounterManager_Activate_Count`, `…_Activate_Text`, `…_Complete_Text`, `…_Trigger_Count`, `…_Trigger_Text` | 67, 64, 64, 18, 18 | |
| `Skirmish_EncounterManager_ze_skirmish_controlpoint`, `…_optional`, `…_int_generic`, `…_int_generic_value` | 31, 64, 4, 4 | |
| `Skirmish_Optional_Total`, `Skirmish_Defender_WorldEvent_Integer`, `Skirmish_Survival_*` (4 names) | 17, 7, 1 each | |

Four templates store `Skirmish_ControlPoint_Banner_FriendlyApr` 0.2 and `…EnemyApr` 0. The
run-time skirmish rules belong to the instances subsystem (`instances.md`) and the server.

<a id="s6-9"></a>
### 6.9 Property-response maps on instance sets

138 spawn-only instance sets carry `PropertyResponse_EntityPropertyResponseMap`, all naming a
response map of class 2192 (§8.1). They are NPC-like spawns (110 with `AI_KungFu_StartAsNPC`, 102
with `Drama_DramaList`, 130 with an instance script table).

---

<a id="s7"></a>
## 7. Instance travel and scene hooks (OBS, WL18)

<a id="s7-1"></a>
### 7.1 `Theater_TeleportationSceneID`

1,615 instance sets (926 on Physics SEDs, 689 spawn-only): a scene of class 1468 in 1,592 (108
distinct), null in 21, one DID of class 2651 and one of class 691. 1,583 of the sets carry
`PortalDestination`, 929 `Usage_LayerArray`, 231 `HousingSystem_IsNeighborhoodWaypoint`, 105
`Travel_LeavePrivateInstanceLayerArray`: they are portals, doors and travel points. One template
(0x790064EC) stores a scene as well.

> **OPEN:** when the client plays the scene (on the server's teleport through the object,
> presumably) and how a non-scene DID is treated. The client's class library has no reader of
> the property (its one apparent reference is a message code equal to the PID); the playing of
> scenes belongs to the network-driven travel path.

<a id="s7-2"></a>
### 7.2 `Travel_LeavePrivateInstanceLayerArray`

1,484 instance sets (887 on Physics SEDs, 597 spawn-only); arrays of 0 (53), 1 (1,353), 2 (59), 3
(13) or 4 (6) content layers, 1,534 entries: `All` (2) in 233, layer keys 0x10000003 … 0x100000D3
in the others. 1,466 of the sets carry `PortalDestination` and 1,459 `Usage_LayerArray`.

Reading of the data: using such a portal from inside a private instance on one of the listed
layers takes the player out of the private instance. `Travel_LeavePrivateInstance` (default true)
is stored nowhere.

> **OPEN:** the exact rule (which layer of the player is compared, and what `All` means here).
> Server-side.

<a id="s7-3"></a>
### 7.3 `HousingSystem_IsNeighborhoodWaypoint`

1,834 instance sets (892 on Physics SEDs, 942 spawn-only): true in 1,828, false in 6. 1,781 carry
`PortalDestination`, 1,568 `Usage_LayerArray`, 428 `Usage_HousingPermission_Requirement`, 231
`Theater_TeleportationSceneID`. They mark housing-neighbourhood travel points.

<a id="s7-4"></a>
### 7.4 Path-map blocking volumes

608 spawn-only sets: 602 with a box (`PathMap_BlockingBoxMax` 601, `…Min` 598; an absent corner
takes the dictionary default), 6 with a cylinder, none with both; 12 carry `Entity_CreationType`.

| Field | Census |
|---|---|
| Box corners | min ≤ max on every axis in all 602; centred on the origin in *x* and *y* in 586; minimum *z* = 0 in 531. Commonest maxima (50, 50, 100) 174, (25, 25, 50) 117, (20, 20, 100) 77. |
| Cylinders | radius 1.25, height 12, low point (0, 0, −3) in 4; (2.5, 12, (0, 0, −6)) and (8, 10, (0, 0, −5)) once each |

Two 0x47 descriptors (0x47001986 cylinder, 0x47002271 box) store the dictionary defaults;
three 0x78 sets store `PathMap_IsWater` true. These volumes block the server's path maps for AI movement; the client's
collision does not use them (`collision.md` does not read them).

---

<a id="s8"></a>
## 8. Property-response objects (OBS, WL19)

These complete the census of `misc-records.md` §13.5. 419 objects in six root classes, all
descendants of class 2183 except the map class 2192; plus 4 creature templates.

<a id="s8-1"></a>
### 8.1 Response maps (class 2192, 120 objects)

Each holds `PropertyResponse_WorldEventResponse_List`: entries {`PropertyResponse_WorldEvent`
(boolean event, class 1815), `PropertyResponse_Callback` (an object of class 2193, 2225 or 2353)}.
Typical maps pair each event with two callbacks (for example 22 entries: 11 events × class 2193
and class 2225). Maps are named by `Area_PropertyResponseMapDID` (areas),
`PrivateEncounterTemplate_PropertyResponseMapDID` (`instances.md`), the instance sets of §6.9 and 4
templates of class 1723 (`PropertyResponse_EntityPropertyResponseMap`).

<a id="s8-2"></a>
### 8.2 Callbacks

| Class (ancestors) | Objects | Properties |
|---|---|---|
| 2193 (2197 > 2183) | 21 | `PropertyResponseCallback_OnTrueResponseType` (8 `Activate` in the samples), `…_OnFalseResponseType` (2 `Deactivate`), `…_AIHint` |
| 2225 (2197 > 2183) | 11 | `…_OnTrueResponseType`, `…_Drama` (a drama name) |
| 2353 (2352 > 2183) | 237 | `…_AppliedEffect_Array` (entries with optional `Floor` / `Ceiling` bounds and effect lists), optionally `…_UseTargetLevelForSpellcraft`, `PropertyResponse_Filter_Array` |
| 2517 (2352 > 2183) | 9 | `PropertyResponse_CalculateProperty_Array` (in the samples two entries: targets `Generator_MaxLevel` and `Generator_MinLevel`, min 1, max 200, source mod 0), `…_CalculateProperty_EntityType` |
| 2518 (2352 > 2183) | 21 | `PropertyResponse_MapProperty_EntityType` (65,537 in the samples), `PropertyResponse_MapProperty_Array` (mappings from a `Skirmish_GroupSize` source value to a `Generator_ProfileDID` target) |

Callbacks of classes 2517 and 2518 are named by world-event generators (§6.4); 2193, 2225 and
2353 by response maps. The response types use the mapper of §3.5.

> **OPEN:** the response semantics (when a callback fires, what "on true / on false" refers to,
> how floors and ceilings select an effect entry). Server-side; the class library reads none of
> these properties (`misc-records.md` §13.5).

---

<a id="s9"></a>
## 9. Version-migration objects (OBS, WL20)

These complete the census of `misc-records.md` §13.7.

<a id="s9-1"></a>
### 9.1 The master list and the subtree

- Object **0x7000021D** (class 1623) holds `Version_Number` (the current content update; 4,920
  here), `Version_AlwaysRunOnPlayers` (2 DIDs: 0x70008DFD, 0x70010220) and
  `Version_VersionNumberDataList`: 93 entries {`Version_Number` (update numbers 151 … 4,920,
  strictly ascending), a list of 1–5 object DIDs (156 in all, distinct)}; the list is
  `Version_SingletonList` in 92 entries and `AccountVersion_SingletonList` in 1.
- **Subtree 1621** (descendants of class 1621: 1620 and its subclasses, and 3741 > 1621) holds
  158 objects. 156 are listed by the master list; the other 2 are the always-run objects. 123
  carry `Version_*` data (§9.2); 35 objects of 34 classes (31 classes under 1620, 3 under
  3741) have an empty property set, so their classes' code is all there is.

Reading of the data: when a character (or account) last saw an update older than an entry's
number, the server runs that entry's objects on it once; the two always-run objects run every
time. Server-side (OPEN §14).

> **OPEN (implementer, 2026-10-09):** the value type of `Version_Number` is not stated.
> **Tessera's choice:** read it as an enum key or an integer, whichever is stored.

<a id="s9-2"></a>
### 9.2 Classes with data

| Class | Objects | Stored properties |
|---|---|---|
| 2084, 3169, 3189, 3428, 4037 (under 3167 > 1620) | 8, 14, 3, 3, 3 | `Version_QuestList` (quests of class 1398; 1–532 entries) |
| 2088 (under 3167) | 4 | `Version_QuestList`, `Version_QuestFactionDID` (class 1889), `Version_QuestRepTier` (1–3) |
| 1790 | 1 (always-run) | `Version_POItem_PartnerDataList` (2 partners; item lists of 341 and 84 entries), `Version_POItem_MinTimeBetweenReclaim` |
| 2245 | 1 (always-run) | `Version_TitleBestow_PartnerDataList` (2 partners; 190 and 4 entries) |
| 2351 | 1 | `Version_MonsterPlay_MonsterLevel` (160) |
| 2438 | 1 | `Version_LegacyEffectList` (24 effects, class 734) |
| 2455 | 1 | `Version_IARuneArray` (item level → rune, class 796), `Version_IABadReforgeTable` (class 2204) |
| 4146 | 1 | `Version_IARuneConversionArray` (10 rune conversions) |
| 2542, 3114 | 1, 14 | `Version_CharacterClassTypeList` (class keys) |
| 3673 | 22 | `Version_ResetTraitConfiguration_Array` (keys 1, 3), optionally `Version_CharacterClassTypeList` |
| 3289 | 6 | `Version_UpdatePlayerTraits` {`Version_ActionType` 2, class list, `Version_TraitList` (classes 1477, 1478, 3438)} |
| 3878 | 11 | `Version_UntrainSkills_DataArray` (per class key, skill lists) |
| 3857 | 1 | `Version_RenameQuests_DataArray` (per class key, 22 rename records) |
| 3226 | 2 | `Version_AdvancementLineData_List` (class 2390) |
| 3244 | 1 | `Version_ItemPropertiesStruct` (from / to `Item_EquipmentCategory`, class list), `Version_OnlyEquippedItems`, `Version_OnlyBoundItems` |
| 2588 | 1 | `Version_OnlyWieldableItems`, `Version_OnlyBoundItems`, `Version_OnlyCraftedItems` |
| 3146 | 1 | `Version_UnslotAllRelics` |
| 3253 | 1 | `Version_LevelResponseStruct` {`Version_MinLevel` 65, `Version_IsMonsterPlayer` false, an effect list} |
| 3433 | 1 | `Version_SkillToRefresh` (class 827) |
| 3639 | 1 | `Version_Trait_To_Characteristic_List` |
| 2748 | 1 | `Version_SkirmishTraitList` (89 traits, class 2525) |
| 2662 | 1 | `Version_GOP_UpdateArray` (old property → new property with value replacements; 3 entries) |
| 2735 | 1 | `Version_FloatyInformation_Bitfield` |
| 2767 | 5 | `Version_Property_PropertyUpdateList` (a property ID and the value to set) |
| 3728 | 9 | `Version_AddNewChatTextType_TypeToAdd`, `…_AddToTabsWithType` (`chat-window.md` §4.7) |
| 3742 (under 3741) | 2 | `Version_ProgressionDID` (class 1389) |

The client applies one of these itself: the chat-type additions (class 3728, `chat-window.md`
§4.7). For the others the client has no reader in its class library (WL21).

---

<a id="s10"></a>
## 10. References between records (Confirmed)

| From | Property | To |
|---|---|---|
| Box instance set | `QuestState_QuestCheck_List[]/QuestState_QuestID` | quest (class 1398) |
| Box entity | physics object | 0x47 descriptor named by UNIQUEDB key 0x10000046 |
| Phasing link (type 0x10000019) | end 2 / end 1 | box instance ID / target instance ID |
| Spawned entity's template | `Entity_ClassScriptTable`, `Entity_AdditionalScriptTables` | quest-state tables 0x070014AF, …B0, …B1, …BB, …CE |
| Quest-state table | GameCallback package | 0x70026F0D (class 3172) |
| Entity (script path) | `QuestState_QuestID` (`_2`) | quest (class 1398) |
| Entity (script path) | `QuestState_P_Particle` / `…_DecalMaterial` / `…_Cosmetic*_Item` / `…_IdleAnimation` | 0x39 / 0x31 / 0x47 / 0x0C records |
| Generator | `WorldEventGenerator_ResponseMapping` | integer event (1887); callback (2517, 2518) |
| World-property box | `WorldPropertyBox_WorldEvent`; custom-condition events | events (1815, 1887) |
| Controller | `WorldEvent_*EventList[]` | boolean events (1815; 4 integer) |
| Bastion | `Skirmish_Bastion_WorldEvent_Boolean` / `_Integer` | events 1815 / 1887 |
| Portal | `Theater_TeleportationSceneID` | scene (1468) |
| Spawn, area, private-encounter template, creature template | `…PropertyResponseMap…` | response map (2192) |
| Response map | `PropertyResponse_WorldEvent` / `PropertyResponse_Callback` | event 1815 / callbacks 2193, 2225, 2353 |
| Master version object | `Version_SingletonList`, `Version_AlwaysRunOnPlayers` | objects of subtree 1621 |

---

<a id="s11"></a>
## 11. Invariants

Data invariants hold for every record of this install (Confirmed 2026-10-09); a reader must not
fail when they do not hold elsewhere.

- **WL-I1.** No link, cell or landblock set holds a property of §3.
- **WL-I2.** Every SED whose physics object is the box DID (§4.1) has an instance set with a
  `QuestState_QuestCheck_List`.
- **WL-I3.** Every element of a check list is a `QuestState_QuestCheck_Struct` with all three
  fields; its quest DID is a quest record; its operator is 1–6; its status has raw status 0x1 or
  0x3.
- **WL-I4.** In a link of type 0x10000019 the box is never end 1; when both ends are non-zero,
  end 2 is the instance ID of a set with a check list.
- **WL-I5.** In every set with a check list that stores `QuestState_False_Draw`, it equals
  the negation of the stored `QuestState_True_Draw`, or is true when `True_Draw` is absent.
- **WL-I6.** No set stores `QuestState_PhasedOut`.
- **WL-I7.** Each SED with `QuestState_*` in its instance set has identical `QuestState_*` and
  `PhasingBox_*` values in its own set.
- **WL-I8.** Every `WorldEvent_CustomCondition` has a link in 1–6 and exactly one condition form.
- **WL-I9.** Every path-map box has min ≤ max on each axis (after defaults).
- **WL-I10.** Every object listed by `Version_SingletonList` or `Version_AlwaysRunOnPlayers` is
  in subtree 1621, and every object of subtree 1621 is listed by one of them.

Behaviour invariants (for tests of an implementation with synthetic entities):

- **WL-B1.** Before E4, every box tests false and every target is phased out.
- **WL-B2.** A box with no check list tests false whatever its `True_Draw`.
- **WL-B3.** For a box with one passing check: `True_Draw` true → targets shown; false or absent →
  phased out. With one failing check the opposite.
- **WL-B4.** After E2 for a quest the box does not name, no state changes.
- **WL-B5.** A box with `UpdateOnlyOnCreation` true does not change on E2; it changes on E4.
- **WL-B6.** A box with `DelayRefreshOfLinkedEntities` true changes V on E2 but not its targets
  until E4 or the target's next creation.
- **WL-B7.** A phased-out entity has `Render_Draw` false, `ClientIsSelectableOverride` false and
  `QuestState_PhasedOut` true; a shown one the opposite.
- **WL-B8.** On the script path, an entity with `QuestState_Test` = 2 runs only the False script,
  whatever its quest.

---

<a id="s12"></a>
## 12. Synthetic worked examples

<a id="s12-1"></a>
### 12.1 A box

A synthetic box *X* (physics object = the box DID) with:

- `QuestState_QuestCheck_List` = [(quest Q, status 0x10000002, operator 5 ≤),
  (quest R, status 0x30000000, operator 2 ≠)];
- no `QuestState_QuestCheck_Operator` (so `And`);
- `QuestState_True_Draw` = true;

and one phasing link (type 0x10000019, end 2 = *X*, end 1 = target *T*). The player holds Q
underway at objective 1 and has not completed R.

1. Before quest state arrives: *X* tests false; *T*, created first, has a phasing link and no
   mapped box → phased out (E1 step 3). *X* created: B4 gives false, B5 applies false to *T*.
2. E4: entry 1: Q underway at 1, 1 ≤ 2 → passes; entry 2: R not completed, operator 2 → passes;
   `And` → *C* true; `True_Draw` true → *B* true. *T* is shown: `Render_Draw` true,
   `ClientIsSelectableOverride` true, `QuestState_PhasedOut` false.
3. The player advances Q to objective 3: E2(Q) → entry 1 fails (3 ≤ 2 false) → *C* false → *B*
   false → *T* phased out (unless *X* has `DelayRefreshOfLinkedEntities` true).
4. Same box with `True_Draw` absent: every result above is inverted (*T* shown in step 3, phased
   out in step 2). With `QuestState_QuestCheck_Operator` = 3 (`Or`) in step 3, entry 2 still
   passes and *T* stays shown.

<a id="s12-2"></a>
### 12.2 The script path

A synthetic spawned NPC whose template names table 0x070014AF, with `QuestState_QuestID` = Q,
`QuestState_QuestStatus` = 0x30000000, `QuestState_QuestOperator` = 3, `QuestState_True_Draw`
absent, `QuestState_False_Draw` absent, `QuestState_False_Particle` = some 0x39 DID (ignored:
0x070014AF has no particle node).

- Player has completed Q: 0.2 s after creation the callback returns true → script
  `QuestState_True` runs; 2.0 s later `True_Draw` reads its default true → opacity `Replace` 1.0,
  selectable, floating name shown.
- Player has not completed Q: the False script runs; `False_Draw` reads false → opacity
  `Multiply` 0.0, not selectable, floating name hidden.
- With `QuestState_Test` = 1 stored: the True script runs without the callback.

<a id="s12-3"></a>
### 12.3 A world-event condition

Integer event *E* with minimum 0, maximum 10 and current value 4. Condition {*E*, operator 1 ≥,
value 3} holds; {*E*, 6 >, 4} does not; {*E*, 3 =, 12} is false by the range check (12 > 10);
{*E*, 0, 4} is false (operator 0). Boolean event *F* with value true: {*F*, 3, 1} holds,
{*F*, 2, 1} does not, {*F*, 1, 1} is false (only 2 and 3 apply), {*F*, 3, 2} is false.

---

<a id="s13"></a>
## 13. Explorer: gating and wiring view of a landblock

A landblock view should list (all from data, nothing hard-coded):

1. **Boxes**: each instance set with a check list (SED or spawn-only), its checks as
   "quest *name* operator status" with the status decoded (`quests.md` §8.3), the combining
   operator (default `And`), `True_Draw` (absent shown as "false (default of the box test)"),
   the two `PhasingBox_` flags, and its targets (end 1 of its phasing links, cross-landblock
   halves marked).
2. **A simulated journal**: a list of quests with status; the view evaluates every box with
   §4.3–§4.4 and marks each target shown or phased out (the D1 empty journal by default).
3. **Script-path entities**: instance sets with a top-level quest ID, their test, `QuestState_Test`
   and the visuals of each branch (§5.4).
4. **Logic wiring**: counters, timers, generators (event → callback), world-property boxes
   (link → condition), event lists, bastion and control-point values, theater scenes, instance
   exits and housing waypoints, path-map volumes drawn as boxes and cylinders.

---

<a id="s14"></a>
## 14. Open questions (summary)

1. §4.2: joining cross-landblock link halves (Tessera rule meanwhile).
2. §4.5: which client events deliver E2 and E4 (network side, excluded by decision 0011); whether
   client-created SED entities reach E1 through the same notification.
3. §4.6: collision of phased-out targets; the call that passes "hidden" to attached children.
4. §4.7: the reaction to a later change of the player's `CanSeeVisibilityMask`.
5. §5.5: whether quest-state tables re-run on quest changes.
6. §6.1–§6.6, §7.1–§7.2, §8.2, §9.1: server-side semantics of counters, timers, generator
   callbacks, custom conditions, event lists, scene teleports, instance exits, property responses
   and version migrations (the client does not contain them; a Tessera server needs its own
   rules, to be specified in `test-server.md` when implemented).
7. §6.7: the meaning of `WorldEvent_StateToTestValue`.
8. §3.4: the unit of `LogicBoxTimer_*` values (seconds presumably).

---

<a id="s15"></a>
## 15. Change log

| Date | Sections | Change | Author |
|---|---|---|---|
| 2026-10-09 | all | First version (Review). Box phasing (BIN, WLB), quest-state script tables (data, WLB), world-event condition test (WLB), censuses of all groups over all sets, property-response and version objects, reference graph, invariants, examples, explorer view. | Spec writer (dirty room) |
