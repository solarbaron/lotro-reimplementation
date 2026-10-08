<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Quests, deeds and dialogue: quest records, objectives and events, roles, rewards, deeds, world events and the quest-target index

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate sessions from the author; re-review by a third session that neither wrote the spec nor made the first review) |
| **Review record** | Re-review 2026-10-07 (a session that neither wrote the spec nor made the first review): **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/props` / `crates/formats` (typed views over the property sets of §4–§15; the map-note record of §17 and its fuzz target), `crates/wstate` (the compiled quest object of §6 and the quest-target index of §16 on top of the existing native codecs), `apps/dattool` / `apps/explorer` (quest, deed, NPC-dialogue, title, emote and quest-map views), `crates/net` sandbox backend and any later server work (§5.7, §7, §8, §9 give the data a quest engine runs on) |
| **Companion specs** | [`property-system.md`](property-system.md) §2–§5 (dictionary, value encodings, property sets), §6.2 (the +0x09000000 rule), §7 (enum, bitfield, PID and DID display), §8 (DID maps, WEENIECONTENT). [`wstate.md`](wstate.md) §3–§8 (object-state records, bundle, native codecs incl. classes 427 and 2479 in §5.3–§5.4), §11 (areas, territories, dungeons). [`records.md`](records.md) §1.3 (TSize), §4 (StringInfo), §7 (enum mappers), §8 (DID maps). [`string-table.md`](string-table.md) §9 (resolving a StringInfo). [`sound.md`](sound.md) §4 (sound-info records, prefix 0x2A). [`coordinates.md`](coordinates.md) (positions, landblocks). |

---

<a id="purpose"></a>
## Purpose

Quests and deeds are the main gameplay content of the data. This spec lets an implementer:

- find every record of the quest family by its identifying properties and read its
  property set (§2, §4);
- read a quest's objectives, conditions and events, and number its events exactly as the
  client's compiled data does (§5), so that a server can track objectives and a client
  can show progress;
- read the compiled quest object in the 0x70 record (event table, objective index, loot
  index, effect index, completion comments) (§6);
- read who gives, advances and completes a quest and with which dialogue lines and
  voice-over (§7), including the NPC-side role lists, NPC comment records, ambient
  "colour text" lines and scene records;
- evaluate the data side of prerequisites (§8) and reward tiers, treasure and reputation
  (§9);
- read deed-specific data, deed directories and the deed UI tabs (§10), quest arcs (§11),
  titles and emotes (§12), factions (§13) and world events (§14);
- use the quest-target index for quest maps and objective tracking (§16) and the
  dungeon quest map notes (§17).

Text is never stored inline in this family: every user-visible string is a StringInfo
that refers to a string table (prefix 0x25) and is resolved per `string-table.md` §9.
This spec refers to records by DID and to fields by their **property label** (the name
from the property dictionary, `property-system.md` §2), which is the interoperability
identifier; labels are read at run time from the user's dictionary and are not
reproduced as code constants. Class indices and record DIDs given below are
**observations of this install** and must be discovered at run time (§2.2).

---

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam; client data stamped 2026-10-04) on 2026-10-06 and 2026-10-07 with throwaway probes in the dirty-room scratch area, built read-only on the repository's decoders (`tessera-dat`, `tessera-formats`, `tessera-props`, `tessera-wstate`). Unless a row says otherwise, a count covers **every** record of the kind: all 20,807 quest/deed records (0x70 object state and 0x79 property set), all records of every other class named in §2.1, every 0x78 and 0x79 property set of `client_gamelogic.dat` for the reference census of §3, the single quest-target object 0x70019558, and the map-note record 0x0E400000 of `client_general.dat` (5,638 notes, decoded to the last byte). |
| LC | LotroCompanion `lotro-tools` source (quest, deed, objective, role, requirement, reward, loot, world-event and quest-map-note loaders), read for facts only; no code, comments or identifiers were copied beyond property labels, which are interoperability identifiers. |
| DS | `research/notes/data-semantics.md` §4.7, §5, §12 (dirty-room notes). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, file dated 2026-10-04), analysed headless in a private copy of the shared Ghidra project on 2026-10-07 (address ranges only; nothing copied). **Questions:** (1) how the client derives the level it shows for a quest when the quest names an override property; (2) which properties of this family the executable reads directly by PID (to separate client-side behaviour from behaviour that lives in script code or on the server); (3) whether the client itself evaluates quest-requirement operators, chooses species-specific dialogue lines or computes tier rewards. **Why M1–M3 were not enough:** the data gives the three properties involved in (1) but not their precedence; (2) and (3) are properties of the code. No network, login, launcher or anti-tamper code was read. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| F1 | Record kinds of the family, their identifying properties, counts and sizes (§2.1) | OBS (all records); DS | Confirmed 2026-10-07 |
| F2 | Well-known singleton records through WEENIECONTENT labels (§2.3) | OBS (DID map 0x2800000E decoded) | Confirmed 2026-10-07 |
| F3 | Enum mappers used by the family and their entry counts (§2.4) | OBS (registry `data` field of every property named, mappers decoded) | Confirmed 2026-10-07 |
| F4 | Outgoing and incoming reference census (§3) | OBS (every 0x78/0x79 set walked) | Confirmed 2026-10-07 |
| Q1 | Quest vs deed split by `Quest_IsAccomplishment`; deed-only and quest-only properties (§4.1) | LC; OBS (20,807 records) | Confirmed 2026-10-07 |
| Q2 | Top-level quest properties: types, presence counts, value ranges, dictionary defaults (§4.2) | OBS; LC (meanings of several) | Confirmed (layout, counts); meanings Reported where marked |
| Q3 | Effective challenge level: hide flag, override property read from the player, then stored level (§4.3) | BIN; OBS (properties and values) | Confirmed (BIN + OBS) |
| O1 | Objective / condition / event nesting (§5.1–§5.3), including the shapes of conditions and the composition of compound events (§5.2) | LC; OBS (82,280 event entries; all 56,035 objective conditions, 2,199 quest-level conditions and 6,154 compounds, re-measured 2026-10-07 for the review) | Confirmed 2026-10-07 |
| O2 | Event types, their characteristic fields and counts (§5.4) | OBS (field census per type); LC (meanings of several types) | Confirmed (fields, counts); meanings Reported where marked |
| O3 | Monster selector fields and the classification index they match (§5.5, §16.3) | OBS (every selector; index tables checked against every agent) | Confirmed 2026-10-07 |
| O4 | Event numbering: EventID, ObjectiveID, ConditionID, order (§5.7), with the compound walk of step 1.2 (every direct entry once, every entry array in order) and empty conditions numbered | OBS (all 20,807 compiled quests match the rule exactly; re-run 2026-10-07 after review) | Confirmed 2026-10-07. Derived by this spec writer. |
| C1 | Compiled quest object: root attributes and what they index (§6.1) | OBS (all 20,807); LC (event table) | Confirmed 2026-10-07; meaning of one attribute OPEN |
| C2 | Compiled event objects: added properties, sibling lists (members per §5.2, ascending order), listen lists (§6.2) | OBS (all 82,280 events; sibling lists compared unsorted in all 20,807 quests) | Confirmed 2026-10-07. Derived by this spec writer. |
| C3 | Effect-application objects: all fifteen attributes, the double bit pattern (§6.3) | OBS (all 958 class-700 objects of all 20,807 quests) | Confirmed 2026-10-07 |
| R1 | Role structure, action enum, flattened `Quest_RoleArray` and its relation to the source roles (§7.1–§7.3); `Quest_ObjectiveIndex` has no dictionary default and its presence per container and action (§7.1) | LC; OBS (dictionary; all 95,886 role structs) | Confirmed 2026-10-07. The objective assignment of action-5 roles is derived by this spec writer; their run-time limits are OPEN. |
| R2 | NPC-side role lists and their agreement with quest roles (§7.4) | OBS (43,406 entries) | Confirmed 2026-10-07 |
| R3 | NPC comment, comment-list, colour-text and scene records; compiled scene native (§7.7–§7.9) | OBS (all records) | Confirmed 2026-10-07 |
| P1 | Usage-permission struct fields used by quests; quest-requirement entries, operator and status encoding (§8) | LC; OBS | Layout and encoding Confirmed; evaluation rule Reported (LC) |
| W1 | Reward tier properties, reward-level records, default reward-level table (§9.1–§9.3) | LC; OBS (160 reward records, all quests) | Layout Confirmed; choice of table when none is named Reported (LC) |
| W2 | Quest treasure, reputation, medal, paper-item, loot-table and given-item data (§9.4–§9.7) | LC; OBS | Confirmed 2026-10-07 |
| D1 | Deed fields, UI tabs, directories, version list (§10) | LC; OBS | Confirmed 2026-10-07; several meanings OPEN |
| A1 | Quest arcs and the arc directory (§11) | LC; OBS | Confirmed 2026-10-07 |
| T1 | Titles, emotes and the emote directory (§12) | OBS | Confirmed 2026-10-07 |
| T2 | Factions, account modifiers, reputation control (§13) | OBS | Confirmed 2026-10-07 |
| E1 | World events: boolean and integer kinds, condition lists, control lists (§14) | LC; OBS | Confirmed 2026-10-07 |
| I1 | Quest item directories (§15) | OBS | Confirmed 2026-10-07 |
| X1 | Quest-target index sections and their meaning (§16); section-4 entry census split by matched, unmatched and missing-quest objectives (§16.3) | OBS (all entries; section 4 re-measured over all 208,859 entries on 2026-10-07 for the review); LC (role as a location table) | Confirmed 2026-10-07; meaning of several fields OPEN |
| M1 | Quest map-note record 0x0E400000 layout (§17) | LC (outline); OBS (5,638 notes, exact) | Confirmed 2026-10-07 |
| B1 | Properties the executable reads directly, and properties it never names (§18) | BIN | BIN only |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Quest record** | A game object whose property set carries `Quest_Name`, `Quest_ObjectiveArray` and `Quest_RoleArray`. Quests and deeds share it (§4.1). |
| **Deed** | A quest record with `Quest_IsAccomplishment` = true ("accomplishment"). |
| **Source form** | The quest as authored, in the 0x79 property set: objectives, conditions, event entries, roles. |
| **Compiled form** | The 0x70 object-state record of the same quest: one script object per event, plus index containers (§6). It repeats the source facts and adds numbering. |
| **Objective** | One step of a quest (`Quest_CompletionObjective`), identified by `Quest_ObjectiveIndex` (1 … *n*). |
| **Condition** | One element of an objective's condition array: a completion condition or a failure condition (§5.2). |
| **Event entry** | A `QuestEvent_Entry` struct: something that happens in the world and counts towards a condition, typed by `QuestEvent_ID` (§5.4). |
| **Role** | A `Quest_Role` struct binding an NPC (or a role key) to an action in a quest and to dialogue lines (§7). |
| **Reward tier** | A small enum value on the quest (for example `Quest_ExpTier`) that selects an amount from a reward-level record (§9). |
| **Treasure** | A quest-treasure record (`Quest_QuestTreasureDID`) listing fixed and selectable rewards (§9.4). |
| **Status code** | A 32-bit quest status with the raw status in the top four bits and an objective number below (§8.3). |

---

<a id="s2"></a>
## 2. Record family

<a id="s2-1"></a>
### 2.1 Kinds (Confirmed)

Every kind below is a 0x70 object-state record with its property set at DID + 0x09000000
(`property-system.md` §6.2). The **identifying properties** decide the kind; the root
class (offset 4 of the 0x70 record, `wstate.md` §3.1) is a fast hint for this install.
The byte columns are the summed payload sizes of the kind's 0x70 and 0x79 records.

| Kind | Identifying properties (0x79 top level) | Root class here | Records | 0x70 bytes | 0x79 bytes |
|---|---|---|---|---|---|
| Quest or deed | `Quest_Name`, `Quest_ObjectiveArray`, `Quest_RoleArray` (all three in every record) | 1398 | 20,807 (15,400 quests, 5,407 deeds) | 29,740,057 | 23,800,043 |
| Quest treasure | only `QuestTreasure_*` properties (1 – 3 per record, never empty) | 1406 | 9,479 | 3,280,772 | 782,644 |
| Reward-level table | `QuestReward_*EntryList` | 1404 | 160 | 182,400 | 492,480 |
| Quest arc | `QuestArc_Name`, `QuestArc_Quest_Array` | 2119 | 344 | 26,652 | 25,784 |
| Quest-arc directory | `QuestArcDirectory_QuestArc_Array` | 2120 | 1 | 12,770 | 2,770 |
| Accomplishment directory | `Accomplishment_List` | 838 | 29 | 1,517,565 | 57,650 |
| Accomplishment version list | `Accomplishment_Version_List` | 4241 | 1 | 57 | 8,160 |
| Quest-item directory | `Quest_QuestItemDirectory_Array` (1 of the 21 sets is empty; it is still reached through the region directory) | 2315 | 21 | 756 | 7,956 |
| Quest-item region directory | `Quest_QuestItemRegionDirectory_Array` | 2316 | 1 | 36 | 678 |
| Title | `Title_String`, `Title_Category` | 1470 | 3,091 | 111,276 | 290,554 |
| Emote | `Emote_CommandString`, `Emote_ScriptID` | 1049 | 273 | 9,828 | 56,409 |
| Emote directory | `Emote_AvailableList` | 1050 | 1 | 1,185 | 2,214 |
| Faction | `Reputation_Faction_Name`, `Reputation_Faction_AdvancementTable` | 1889 | 114 | 4,104 | 17,202 |
| Reputation account modifier | `Reputation_AccountModifier_ModifierList` | 5739 | 69 | 6,189 | 15,801 |
| Reputation control | `Reputation_FactionTable` | 1891 | 1 | 3,067 | 1,680 |
| World event, boolean | `WorldEvent_WorldPropertyName`, `WorldEvent_StartActive`, no `WorldEvent_MaxIntValue` | 1815 | 1,574 | 92,866 | 369,995 |
| World event, integer | `WorldEvent_WorldPropertyName`, `WorldEvent_MaxIntValue` | 1887 | 699 | 25,164 | 120,867 |
| World-event list | `WorldEvent_WorldEventList` with `WorldEvent_Control_*` | 2161 (42), 1812 (1) | 43 | 37,832 | 72,350 |
| NPC comment | `NPCComment_Content`, `NPCComment_Title` | 2246 | 859 | 142,052 | 875,321 |
| NPC comment list | `NPCComment_CommentArray` | 2247 | 223 | 16,785 | 11,270 |
| Colour-text list | `NPC_ColorTextArray` and/or `NPC_ReputationColorTextArray` | 978 | 1,206 | 43,416 | 89,096 |
| Scene | `Scene_KeyframeArray`, `Scene_Name` | 1468 | 1,172 | 402,963 | 241,299 |
| Quest-target index | none (6-byte empty set); root object holds a class-2479 native | 2461 | 1 | 29,944,338 | 6 |
| Quest control | `QuestControl_RewardLevelDIDArray` | 1683 | 1 | 2,778 | 8,238 |
| Accomplishment control | `Accomplishment_UITabElements_Array` | 1788 | 1 | 4,240 | 10,900 |
| NPC colour-text control | `NPC_TextDuration`, `Quest_ColorTextInterval` | 1678 | 1 | 36 | 42 |

In the class library, classes 1815 and 1887 derive from 1816, 2161 from 2147 and 5739 from
5736; the other kinds with more than one record derive directly from 486 (chain
486 → 89 → 485 → 484, `wlib.md` §9). Every 0x70 record decodes exactly with `wstate.md`
(§9 there) and every 0x79 set with `property-system.md` (zero failures in this census).

> **OPEN:** Whether further kinds of the family exist that carry none of the identifying
> properties listed here.

> **OPEN:** (implementation census, 2026-10-07) The identifying properties of the emote kind
> are not unique to it in this install: `Emote_CommandString` with `Emote_ScriptID` is
> carried by 275 property sets, the 273 emotes (root class 1049) and 2 records of root class
> 3982. §12.2 offers a data-driven separation (the emote directory lists exactly the 273
> emotes; the 2 others are not listed), but it needs the directory, not the set alone. Is
> "listed in `Emote_AvailableList`" the intended rule, or does a property of the set itself
> separate emotes from these records? The quest-item directory whose set is empty is not
> identified by any property (20 of 21 are); it is only reached through the region
> directory (§15).

<a id="s2-2"></a>
### 2.2 Discovery at run time

A reader identifies kinds by the properties of §2.1 and may cache the root class of the
first record it identifies as a hint, in the way of `wstate.md` §11.1. The root classes
above must not be hard-coded. Data-driven discovery:

- **quest class** = root class of any `QuestDispenser_Quest` target in an NPC record (§7.4);
- **treasure class** = root class of any `Quest_QuestTreasureDID` target;
- **reward-level class** = root class of any element of `QuestControl_RewardLevelDIDArray`;
- the singletons of §2.3 by their WEENIECONTENT labels.

<a id="s2-3"></a>
### 2.3 Well-known records (Confirmed)

These are named by the WEENIECONTENT DID map (0x2800000E, `property-system.md` §8.1). A
reader looks the label up and must not hard-code the DID.

| WEENIECONTENT label | DID here | Kind | Holds |
|---|---|---|---|
| `QuestControl` | 0x70000262 | quest control | Default reward-level table per level (§9.2), medal thresholds, challenge-level ranges, quest-share range, limits (§9.8) |
| `QuestArcDirectory` | 0x7000E36F | arc directory | All 344 arcs (§11) |
| `AccomplishmentDirectory` | 0x70000255 | accomplishment directory | The main deed list, 3,252 entries (§10.2) |
| `AutoBestowQuestDirectory` | 0x700614C4 | accomplishment directory | 1,284 quests and deeds granted automatically (§10.2) |
| `AccomplishmentVersionControl` | 0x70063E21 | version list | Deeds per game version (§10.3) |
| `AccomplishmentControl` | 0x70008CDB | accomplishment control | Deed UI tabs, groups, acceleration settings (§10.4) |
| `QuestEventTargetLocationControl` | 0x70019558 | quest-target index | §16 |
| `QuestItemRegionDirectory` | 0x70015A08 | quest-item region directory | §15 |
| `QuestCategoryLimiterControl` | 0x7002465E | limiter | A map of three `Quest_Category` values to objects (meaning OPEN, §9.8) |
| `EmoteDirectory` | 0x70000216 | emote directory | §12.2 |
| `ReputationControl` | 0x7000A452 | reputation control | §13.3 |
| `WorldEventControl` | 0x7000981F | world-event list | 1,509 world events (§14.3) |
| `NPCColorControl` | 0x7000020C | colour-text control | §7.8 |
| `MonsterDivisionControl` | 0x70006CC2 | division control | `MonsterDivision_Control_DivisionToRegionMap` (§5.5) |
| `VersionAccountTitleBestow` | 0x70010220 | version title grants | Titles granted per account billing group (§12.1) |

The label `WorldEventServer` names an entity record (0x7000981E) with no world-event
properties; it is not part of this family.

<a id="s2-4"></a>
### 2.4 Enum mappers (Confirmed)

The mapper of each enum or bitfield property is the `data` DID of its dictionary
definition (`property-system.md` §7.1). Entry counts are those of the mapper record
itself (base chains not followed).

| Mapper | Label in the EMAPPER map (0x28000001) | Entries | Used by (examples) |
|---|---|---|---|
| 0x23000019 | QuestCategory | 478 | `Quest_Category` (424 distinct values on 15,399 quests), `QuestEvent_QuestCompleteCategory` |
| 0x2300004F | QuestEventType | 45 | `QuestEvent_ID` (38 values used), the listen lists of §6.2 |
| 0x23000017 | QuestStatus | 43 | `Usage_QuestStatus`, `QuestEvent_Status`, `QuestEvent_Session_Status` (§8.3) |
| 0x2300001D | QuestRoleAction | 14 | `QuestDispenser_Action` (§7.2) |
| 0x2300001C | AccomplishmentUITab | 86 | `Accomplishment_UITab` (77 values used) |
| 0x2300001B | AccomplishmentCategory | 26 | `Accomplishment_Category`, `QuestEvent_AccomplishmentCompleteCategory` |
| 0x2300010A | — | 5 | `Accomplishment_UIFilter` |
| 0x2300031F | DeedAccelerationCategory | 4 | `Accomplishment_AccelerationCategory` |
| 0x230003C0 | QuestScriptCallbackKey | 190 | `QuestEvent_ScriptCallback_Key` (154 values used) |
| 0x23000039 | Util | 7 | `Usage_Operator`, `QuestEvent_WorldEvent_Operator`, `WorldEvent_Operator` (§8.2) |
| 0x23000106 | — | 8 | `Quest_Scope`, `QuestEvent_QuestCompleteScope` |
| 0x2300001E | UIQuestScope | 11 | no quest property in this install |
| 0x23000107 | — | 18 | every `*Tier` reward property and `QuestReward_Tier` (§9.1) |
| 0x23000108 | — | 3 | `Quest_SessionPointsFailureTier`, `QuestReward_FailureTier` |
| 0x230003AD, 0x230003AE | —, DayOfWeek | 4, 8 | `Quest_LockType`, `Quest_Lock_ResetDays` (§4.2) |
| 0x23000018 | QuestUpdateType | 18 | `QuestEffect_ApplyRole` (§9.7) |
| 0x230001F3 | ReputationTier | 13 | faction tiers (§8.4, §13) |
| 0x2300000A, 0x2300000B, 0x2300000C, 0x2300000D, 0x2300000E, 0x23000061 | GenusType, Species, SubspeciesType, AlignmentType, CharacterClassType, MonsterDivision | 18, 123, 768, 6, 212, 592 | the monster selector of §5.5 |
| 0x2300002A | CombatResultType | 16 | `QuestEvent_Skill_AttackResult` |
| 0x23000097 | FX | 11,373 | `QuestDispenser_StartScriptID`, `QuestDispenser_VOScriptID`, `NPCComment_VOScriptID`, `Emote_ScriptID` |
| 0x230003BA, 0x230003BB | —, QuestMedal | 4, 6 | `Quest_Points_Start`, `Quest_Medal_LootArray_Medal` (§9.6) |
| 0x2300007A, 0x23000143 | TitleCategory, — | 26, 26 | §12.1 |
| 0x230000C4 | BillingGroup | 892 | `QuestTreasure_BillingToken` |
| 0x230003D2 | — | 12 | `Quest_BigBattleColor` |
| 0x230000B9 | UISceneEntity | 74 | `Emote_PreviewCameraOverride` |
| 0x230002A8 | ChatTextType | 39 | `WorldEvent_BroadcastChannel` |
| 0x23000060 | DropFrequency | 57 | `Quest_DropFrequency` |
| 0x23000495 | DeedUIGroup | 12 | `Accomplishment_DeedUIGroup` |
| DID map 0x28000012 | CONTENT_LAYER | — | `Entity_ContentLayer` (§17) |

"—" means the mapper has no label in the EMAPPER map; it is reached only through the
dictionary.

Enum values are always shown by number in this spec; their labels are read from the
mappers at run time.

---

<a id="s3"></a>
## 3. Reference graph

<a id="s3-1"></a>
### 3.1 Outgoing references of a quest record (Confirmed)

The 20,807 quest property sets hold 218,590 references to 0x70 records, 20,369 to sound
infos (0x2A) and 406 to render surfaces (0x41), and no other DID prefix.

DID-valued properties of the 20,807 quest property sets (all nesting levels). "Kind" is
the kind of §2.1 or another family; "missing" counts targets that do not exist in the
install, which a reader must tolerate.

| Property (path, abbreviated) | Occurrences | Target kind (count) |
|---|---|---|
| `Quest_QuestTreasureDID` | 19,431 | treasure (all) |
| `Quest_RewardLevelDID` | 7,156 | reward-level table 7,154; a quest record 2 (invalid; §9.2) |
| `Quest_NextQuest` | 3,376 | quest 3,375; missing 1 |
| `Quest_PositiveFaction`/`Quest_NegativeFaction` → `Quest_FactionDID` | 10,084 | faction 10,082; treasure 1 (invalid); missing 1 |
| `Quest_QuestsToComplete` → `Quest_DataID` | 1,358 | quest |
| `Quest_GiveItemArray` → `Quest_ItemDID` (quest level and event level) | 2,486 | item records |
| `Quest_LootTable` → `Quest_LootItemDID` / `Quest_LootMonsterDID` | 2,046 / 940 | items / monsters (14 missing) |
| `Quest_Medal_LootArray_TreasureDID` | 520 | treasure |
| `Quest_PaperItemAwards` → `Quest_PaperItemDID` | 250 | item (2 distinct) |
| `Quest_EffectArray` / `QuestEvent_ApplyEffectArray` → `Quest_DataID` | 756 / 202 | effect records |
| Role `QuestDispenser_NPC` (all role arrays) | 66,576 | NPC 61,891; monster 3,094; item 927; class-5785 objects 156; effect 2; missing 506 |
| Role `QuestDispenser_VOSoundID` | 20,369 | sound info (prefix 0x2A, `sound.md` §4); 9,562 distinct |
| Role `QuestDispenser_PrivateEncounterTemplate`, `…_OpenInstanceUI_PrivateEncounterTemplate` | 4,202 / 54 | private-encounter templates |
| Role `QuestDispenser_SessionDID` / `QuestDispenser_TeleportationSceneID` / `QuestDispenser_MissionTwistIcon` | 180 / 28 / 406 | session records / scenes / surfaces (0x41) |
| Event `QuestEvent_NPCTalk` | 21,508 | NPC 20,177; monster 1,249; other 5; missing 77 |
| Event `QuestEvent_ItemDID` | 18,840 | item records |
| Event `QuestEvent_QuestComplete` / `QuestEvent_BestowedQuestID` | 9,879 / 198 | quest (28 missing) |
| Event `QuestEvent_MonsterDID` | 4,038 | monster (17 missing) |
| Event `QuestEvent_LandmarkDID` (incl. monster selectors) | 2,705 + 729 | landmark records (class 1210 here) |
| Event `QuestEvent_WorldEvent` | 4,367 | world event |
| Event `QuestEvent_EmoteDID` / `QuestEvent_EmoteTargetDID` | 1,116 / 563 | emote / NPC or monster |
| Event `QuestEvent_FactionDID` | 685 | faction |
| Event `QuestEvent_SkillDID`, `QuestEvent_RecipeDID`, `QuestEvent_HobbyDID`, `QuestEvent_ChannelingState`, `QuestEvent_Session_DID`, `QuestEvent_InductionActionOverride`, `QuestEvent_TerritoryDID`, `QuestEvent_LandmarkRegion`, `QuestEvent_Detect` | 406, 128, 88, 539, 117, 47, 75, 20, 140 | skill, recipe, hobby, channelling state, session, induction action, territory, territory, NPC/monster |
| Monster selector `Quest_MonsterRegion` | 5,374 | territory (class 692, `wstate.md` §11.1) |
| `Usage_QuestID` (requirements) | 13,665 | quest 13,659; treasure 6 (invalid) |
| `WorldEvent_WorldEvent` (usage conditions) | 4,197 | world event |
| `Usage_RequiredFaction_DataID`, `Usage_RequiredTrait`, `Usage_RequiredEffect`, `Usage_RequiredCraftProfession`, `Usage_RequiredHobby` | 384, 155, 31, 142, 20 | faction, trait, effect, craft profession, hobby |
| `Accomplishment_RequiredTrait` | 599 | trait (2 distinct) |
| `WebStoreAccountItem_DataID` | 6,976 | web-store item records |

String-table references: every StringInfo in the family is a table reference (no literal
and no argument block occurs in any of the 297,771 StringInfos of the quest property sets,
nor in the other kinds of §2.1).

<a id="s3-2"></a>
### 3.2 Incoming references (Confirmed)

Who points at the family (property sets of all classes; "class" is the source record's
root class here, for orientation only).

| Target | Property | Source (count) |
|---|---|---|
| Quest | `QuestDispenser_Quest` (NPC-side role lists, §7.4) | NPCs 40,461; monsters 2,232; items 633; class 5785 80; quests 6 |
| Quest | `Usage_QuestID` | quests 13,659; NPC comments 573; barter profiles 513; private-encounter templates 293; items, travel routes and others 665 |
| Quest | `QuestEvent_QuestComplete`, `Quest_NextQuest`, `Quest_DataID`, `QuestEvent_BestowedQuestID` | quests 9,851 / 3,375 / 1,358 / 198; items 1,023 (`Quest_DataID`) |
| Quest | `Accomplishment_File` | accomplishment directories 6,455; version list 939 |
| Quest | `Quest_InterestedQuest_Entry` | landmarks 2,833; skills 2,112; territories 22 |
| Quest | `QuestArc_Quest_Entry` | arcs 1,589 |
| Quest | others (`PrivateEncounterTemplate_*Quest*`, `Version_*Quest*`, `Area_PublicAreaQuest`, `MovieLibrary_QuestDID`, `Quest_RelatedQuest`, `BigBattleControl_Quest`, …) | < 1,000 each |
| Treasure | `Quest_QuestTreasureDID`, `Quest_Medal_LootArray_TreasureDID`, `Collection_TreasureDID` | quests 19,431 + 520; collections 29 |
| Reward-level table | `Quest_RewardLevelDID` | quests 7,154; quest control 160 |
| Title | `QuestTreasure_Title` | treasure 2,509; also craft professions 320, version grants 237, nationalities 28, kinship themes 36, items 27, hobbies 10, pedigrees 8 |
| Emote | `Emote_ID` (emote directory 273), `QuestEvent_EmoteDID` 1,116, `Effect_EmoteToGrant` 123, `Usage_BarringEmote` 96, `QuestTreasure_Emote` 20 | |
| Faction | `Quest_FactionDID` 10,082; `Usage_RequiredFaction_DataID` 5,581 (quests, items, barter, NPCs, travel); `QuestEvent_FactionDID` 685; `Reputation_PositiveFaction`/`…Negative…` on monsters 782; `Reputation_Faction` 489 | |
| World event | `WorldEvent_WorldEvent` (conditions and lists) 16,193; `QuestEvent_WorldEvent` 4,367; `PropertyResponse_WorldEvent` 2,034; zone-event properties of private encounters, skirmishes and battles | |
| NPC comment | `NPCComment_DIDComment` (comment lists) 907 | |
| NPC comment list | `NPCComment_DIDCommentArray` on NPCs 404, monsters 20, items 3 (a single DID despite the label) | |
| Colour-text list | `NPC_ColorTextFile` on NPCs 284 | |
| Scene | `Area_SceneID` 4,293; private-encounter teleport scenes 1,240; effect generators 394; quest roles 28; travel, monster play, neighbourhoods and theatre 83 | |
| Quest arc | `QuestArcDirectory_QuestArc_Entry` 344 | |
| Accomplishment directory | `MonsterPlay_AccomplishmentDirectory` 28, `AdvTable_AccomplishmentDirectory` (class tables) 12, `RaceTable_AccomplishmentDirectory` 8; WEENIECONTENT for the main and auto-bestow directories | |
| World-event list (2161) | `PrivateEncounterTemplate_EventControlDID` 1,305 | |
| Account modifier | `Reputation_AccountModifier` (factions) 69 | |
| Quest-item directory | `Quest_QuestItemDirectory` (region directory) 22 | |

---

<a id="s4"></a>
## 4. Quest and deed records

<a id="s4-1"></a>
### 4.1 Quest or deed (Confirmed)

A record is a **deed** when `Quest_IsAccomplishment` is present and true (5,407 records;
the property is never stored false). All other quest records are **quests** (15,400).

- Properties found only on deeds: `Accomplishment_UITab`, `Accomplishment_Category` and
  `Accomplishment_MinLevelToStart` (all 5,407), `Accomplishment_UIFilter` (4,765),
  `Accomplishment_ShowProgressBar` (2,472), `Accomplishment_AccelerationCategory` (1,920),
  `Accomplishment_RequiredTrait` (599), `Quest_IsHidden` (549), `Quest_IsSessionAccomplishment`
  (45), `Accomplishment_CanBeAccelerated` (41), `Accomplishment_MinLevelToStart_FloatToCap` (10).
- No deed has `Quest_Category`; every deed has `Quest_QuestTreasureDID`; 82 deeds have a
  non-empty `Quest_RoleArray`.

<a id="s4-2"></a>
### 4.2 Top-level properties (Confirmed counts)

All properties found at the top level of the 20,807 sets, grouped by purpose. "Recs" is
the number of records that store the property. "Default" is the dictionary default
(`property-system.md` §2.2), which applies when the property is absent. Types are the
dictionary types. Meanings marked (LC) are Reported; the others follow from the data.

**Identity and text**

| Property | Type | Recs | Values / meaning |
|---|---|---|---|
| `Quest_Name` | StringInfo | 20,807 | Title |
| `Quest_Description` | StringInfo | 20,398 | Journal text |
| `Quest_TotalWordCount` | Int | 4,616 | −22 … 3,261 (meaning OPEN; negative values occur) |
| `Quest_MonsterPlay_RecapTextSuccess` | StringInfo | 17 | Recap text for monster-play quests |

**Classification and level**

| Property | Type | Recs | Values / meaning |
|---|---|---|---|
| `Quest_Category` | Enum (0x23000019) | 15,399 | Journal category; 424 distinct values |
| `Quest_Scope` | Enum (0x23000106) | 3,087 | 1: 572, 2: 64, 3: 1,693, 5: 12, 9: 54, 10: 692 |
| `Quest_ChallengeLevel` | Int | 19,979 | 1 … 160 (all 160 values occur) |
| `Quest_ChallengeLevelOverrideProperty` | PID | 2,243 | Two PIDs: the character-level property (1,615) and a skirmish-level property (628); §4.3 |
| `Quest_IgnoreDefaultChallengeLevel` | Bool, default false | 3,283 | Always true when stored |
| `Quest_HideChallengeLevel` | Bool, default false | 252 | Always true; §4.3 |
| `Quest_IsInstanceQuest`, `Quest_ContainsInstanceQuest`, `Quest_IsSessionQuest`, `Quest_IsMonsterPlayQuest`, `Quest_IsRaidQuest`, `Quest_ShowRaidInJournal`, `Quest_IsFellowshipRecommended`, `Quest_IsSmallFellowshipRecommended`, `Quest_IsPEBestowedQuest`, `Quest_IsDelvingEligible` | Bool, default false | 1,544, 502, 87, 638, 2,660, 503, 880, 660, 1,532, 201 | Group-size and play-mode flags (LC: journal size label; auto-bestowed by an encounter) |
| `Quest_BigBattleColor` | Enum (0x230003D2) | 105 | 1 … 11 |

**Repetition, locks and timers**

| Property | Type | Recs | Values / meaning |
|---|---|---|---|
| `Quest_MaxTimesCompletable` | Int, default 1 | 4,501 | −1 (4,471; unlimited, LC), −2 (1), 2, 3, 5 |
| `Quest_LockType` | Enum (0x230003AD) | 2,113 | 1: 31, 2: 1,915, 3: 167 (LC: 1 bi-weekly, 2 daily, 3 weekly reset) |
| `Quest_Lock_ResetDays` | Bitfield32 (0x230003AE) | 75 | 1, 16, 17, 82; bit *i* names weekday key *i* + 1 (`property-system.md` §7.3) |
| `Quest_IsDailyGlobalReset` | Bool | 1,053 | Always true |
| `Quest_SecondsUntilRetry`, `Quest_AbandonRetryTimer`, `Quest_FailureRetryTimer` | Int (seconds) | 353, 712, 227 | 3 … 7,776,000; −1 … 2,592,000; −1 … 86,400 |
| `Quest_TimeLimit` | Float (seconds) | 129 | 25 … 28,800 |
| `Quest_PayToRepeatNow_Cost` | Int | 1,021 | −1, 0, 1, 3, 5 (930), 10 |
| `Quest_CanBeCancelled` | Bool, default true | 2,426 | Always stored false |
| `Quest_IsShareable` | Bool, default true | 9,859 | Always stored false |
| `Quest_AutoShare`, `Quest_AutoAcceptAutoBestow`, `Quest_NextQuest_AutoBestow`, `Quest_CanBestowInsideInstance`, `Quest_AlwaysShowBestowRings`, `Quest_DisplayFirstAvailableQuest`, `Quest_RequiresNonTrivialCompletion`, `Quest_ForceMonitorUpdateOnCompletion` | Bool, default false | 61, 87, 39, 379, 780, 47, 340, 6 | Always true when stored |
| `Quest_IsCompletionTracked` | Bool, default true | 4,103 | Always stored false |
| `Quest_IncludesTrackedEvents` | Bool | 107 | True exactly in the 107 records with `Quest_TrackedConditionsArray` |
| `Quest_Points_Start` | Enum (0x230003BA) | 107 | 1, 2, 3: start value of the point score (§9.6) |

**Chaining**

| Property | Type | Recs | Values / meaning |
|---|---|---|---|
| `Quest_NextQuest` | DID | 3,376 | Follow-up quest |
| `Quest_PrecludedQuest` | DID | 3 | A quest that this one excludes (one value is 0) |
| `Quest_QuestsToComplete` | Array of `Quest_DataID` | 119 | 1 … 64 quests (LC: level-up quests that complete these) |

**Objectives, roles and dialogue** — §5, §7. `Quest_ObjectiveArray` (all; 1 … 32
objectives), `Quest_RoleArray` (all; 0 … 59 roles), `Quest_BestowalRoles` (13,092),
`Quest_GlobalRoles` (839), `Quest_CompletionCommentArray` (761),
`Quest_FailureConditionArray` (1,557), `Quest_TrackedConditionsArray` (107).

**Rewards** — §9. `Quest_QuestTreasureDID`, `Quest_RewardLevelDID`, the tier properties of
§9.1, `Quest_PositiveFaction`, `Quest_NegativeFaction`, `Quest_PosRepToGive` (185),
`Quest_GoldToGive` (6), `Quest_ExpToGive` (2), `Quest_GloryToGive` (12),
`Quest_ClassTraitPointReward` (LC; not present in this install), `Quest_Medal_LootArray`
(104), `Quest_PaperItemAwards` (250), `Quest_NeverAwardsMithril` (3),
`Quest_NeverGivesItemExp` (24), `Quest_AllowsPaperItemResetOnAbandon` (79),
`Quest_AllowsPaperItemResetOnFailure` (451), `Quest_Reward_CraftingProfession` /
`…_CraftingTier` / `…_CraftingMasteryTier` (10 each), `RewardTrack_XPToAdd` and
`RewardTrack_ExperienceType` (108 each).

**Items, loot and effects** — §9.7. `Quest_GiveItemArray` (679), `Quest_LootTable`
(1,087), `Quest_EffectArray` (643).

**Prerequisites** — §8. `DefaultPermissionBlobStruct` (17,531), and at top level
`Usage_MinLevel` (163, always 84), `Usage_RequiredClassList` (6), `Usage_RequiredFaction`
(1), `Usage_RequiresExpansionPack1_Legacy` (497), `Usage_RequiresExpansionPack2_Legacy`
(292).

**Other** — `WebStoreAccountItem_DataID` (6,976; the store item that unlocks the content),
`QuestDispenser_ActiveQuests` (12; a role list on a quest record, §7.4),
`Quest_CompletionScriptID` (1).

> **OPEN:** Whether top-level `Usage_*` properties (outside `DefaultPermissionBlobStruct`)
> are evaluated like the same properties inside it. They occur in 959 records, mostly
> `Usage_MinLevel` = 84 together with a struct that has its own `Usage_MinLevel`.

<a id="s4-3"></a>
### 4.3 Effective challenge level (Confirmed (BIN + OBS))

The level the client shows for a quest is computed from three properties, in this order
(BIN):

1. If `Quest_HideChallengeLevel` is stored and true, the level is **0** (not shown).
2. If `Quest_ChallengeLevelOverrideProperty` is present and non-zero, the client reads
   the property it names **from the local player's object**. If that read succeeds, the
   level is the player's value (observed override targets: the character level and a
   skirmish level).
3. Otherwise, if `Quest_ChallengeLevel` is present (type Int), the level is its value.
4. Otherwise the level is 0.

`Quest_IgnoreDefaultChallengeLevel` plays no part in this computation (BIN). It is stored
in 3,283 records; 2,243 of them also have an override property.

> **OPEN:** What `Quest_IgnoreDefaultChallengeLevel` changes (LC treats it together with
> the override property when choosing a reward table; §9.2).

---

<a id="s5"></a>
## 5. Objectives, conditions and events (source form)

<a id="s5-1"></a>
### 5.1 Nesting (Confirmed)

| Level | Container | Element(s) | Count (all records) |
|---|---|---|---|
| 1 | `Quest_ObjectiveArray` (array) | `Quest_CompletionObjective` (struct) | 50,784 objectives |
| 2 | objective struct | `Quest_ObjectiveIndex` (Int, always), `Quest_CompletionConditionArray` (array, always), `Quest_ObjectiveDescription` (46,854), `Quest_ObjectiveProgressOverride` (2,084), `Quest_ObjectiveBillboardOverride` (659), `Quest_ObjectiveLoreOverride` (37), `Quest_NPCObjRoles` (7,611 arrays, §7), `Quest_ObjectiveVolumeString` (459) | |
| 3 | `Quest_CompletionConditionArray` | `Quest_CompletionCondition` (54,819) or `Quest_FailureCondition` (1,216), both arrays | |
| 4 | a condition array | `QuestEvent_Entry` (struct; 65,272 in completion conditions, 1,250 in failure conditions), `QuestEvent_CompoundEvent` (array; 6,154), `Quest_CompletionConditionCount` (Int; 196), `Quest_Condition_NeverFinish` (Bool; 2,102: 1,815 true, 287 false) | |
| 5 | `QuestEvent_CompoundEvent` | 1 … 6 direct `QuestEvent_Entry` structs (7,332 in all), 0 … 2 `QuestEvent_Entry_Array` arrays of `QuestEvent_Entry` (5,175 arrays, 33 of them empty; 6,086 entries), `QuestEvent_CompoundProgressOverride` (StringInfo; 3,162). Composition of the 6,154 compounds in §5.2. | |

Quest level, outside objectives:

- `Quest_FailureConditionArray`: array of `Quest_FailureCondition` arrays of
  `QuestEvent_Entry` (1,997 entries in 1,557 quests). The whole quest fails when one
  holds (LC).
- `Quest_TrackedConditionsArray`: array of `Quest_TrackedCondition` arrays of
  `QuestEvent_Entry` (343 entries in 107 quests; event types 22, 32 and 58 only, all
  with `QuestEvent_Points`). They feed the point score of §9.6.

Total event entries: 78,690 in completion conditions + 1,250 in objective failure
conditions + 1,997 quest-level failure entries + 343 tracked = **82,280**, which equals the
number of compiled events (§6).

Objective indices: in every record the indices form the set 1 … *n* without repeats; in
2,465 records the array stores them out of order (for example 2 then 1). Order matters
for numbering (§5.7).

<a id="s5-2"></a>
### 5.2 Conditions

- An objective is complete when its completion conditions are met; a failure condition
  fails the objective (LC: the quest). The combination rule across several completion
  conditions of one objective is not stated in the data.
- A condition array holds any number of direct event entries (`QuestEvent_Entry`) and
  compound events (`QuestEvent_CompoundEvent`), in a stored order that matters for
  numbering (§5.7), plus the optional count and never-finish elements below. Shapes of the
  54,819 objective completion conditions: one direct entry 43,946; one compound 5,270;
  2 … 83 direct entries and no compound 5,284; 2 … 12 compounds and no direct entry 174;
  direct entries and compounds mixed 121; no entry at all 24 (23 empty, 1 holding only
  `Quest_Condition_NeverFinish`). Compound events occur only in objective completion
  conditions; objective failure conditions, quest-level failure conditions and tracked
  conditions hold direct entries only (1 … 9 per condition).
- A **compound event** groups entries that belong together; the compiled form links its
  members to each other (§6.2). Its elements, in stored order, are direct `QuestEvent_Entry`
  structs, `QuestEvent_Entry_Array` arrays and at most one
  `QuestEvent_CompoundProgressOverride`. Composition of the 6,154 compounds:

  | Direct entries | Entry arrays | Compounds |
  |---|---|---|
  | 1 | 1 | 5,105 |
  | 1 | 2 (the second one empty in all 33) | 33 |
  | 2 | 0 | 892 |
  | 3 | 0 | 88 |
  | 3 | 1 | 4 |
  | 4 | 0 | 27 |
  | 5 | 0 | 4 |
  | 6 | 0 | 1 |

  Every compound starts with a direct entry; no entry array is stored before the first
  direct entry, and no direct entry is stored after an entry array. The progress override
  may stand anywhere among the elements.
- The **members** of a compound event are all the entries it contributes under §5.7
  step 1.2: each direct entry once, and every entry of each of its entry arrays. Every
  compound has 2 … 12 members (5,463 have exactly 2).
- `Quest_CompletionConditionCount` (1 … 15) appears in a condition next to several
  entries: it is the number of those entries that must be done. It is compiled into the
  required-counts attribute of §6.1 under the key ObjectiveID × 65,536 + ConditionID.
- `Quest_Condition_NeverFinish` marks a condition that never completes by itself.

> **OPEN:** The exact completion rule when an objective has several completion
> conditions without a count (all of them, or any). 2,921 objectives have more than one
> completion condition (2,458 two, 177 three, 286 four or more).

<a id="s5-3"></a>
### 5.3 Event entry fields common to all types (Confirmed)

| Property | Type | Occurrences (of 82,280) | Meaning |
|---|---|---|---|
| `QuestEvent_ID` | Enum (0x2300004F) | all | Event type (§5.4) |
| `QuestEvent_EventOrder` | Int | 78,690 (all completion entries) | 0 … 82; order of the entry inside its objective; 0 for 50,782 |
| `QuestEvent_Number` | Int, default 1 | 38,204 | How many times the event must happen (−1 … 999,999) |
| `QuestEvent_ProgressOverride`, `QuestEvent_BillboardProgressOverride`, `QuestEvent_TappedProgressOverride`, `QuestEvent_FailedBillboardText`, `QuestEvent_DramaProgressOverride`, `QuestEvent_LeaveDetectionOverride`, `Accomplishment_LoreInfo` | StringInfo | 57,607, 28,204, 821, 557, 535, 220, 10,262 | Journal and on-screen texts |
| `QuestEvent_ShowProgressText`, `QuestEvent_ShowBillboardText` | Bool, default true | 18,685, 58,841 | Whether the texts are shown |
| `QuestEvent_RoleConstraint` | String | 11,693 | A role key that names which world object counts (9,300 distinct; also the key of the role-key index, §16.2) |
| `QuestEvent_Locations` | Array of `QuestEvent_Location` | 4,123 arrays, 5,435 locations | §5.6 |
| `QuestEvent_LocationsAreOverrides`, `QuestEvent_Locations_ForceFullLandblock`, `QuestEvent_ForceCheckContentLayer` | Bool | 5,516, 610, 77 | |
| `Quest_Role` | Struct | 21,456 | Dialogue bound to this event (§7) |
| `Quest_GiveItemArray` with `QuestEvent_GiveItemsOnAdvance` | Array / Bool | 1,703 / 838 | Items given when the event advances |
| `QuestEvent_ApplyEffectArray` | Array of `Quest_Effect` | 192 arrays, 202 effects | §9.7 |
| `QuestEvent_DailyMaximumIncrements` | Int | 477 | Daily cap on increments (−1 … 500) |
| `QuestEvent_DramaName`, `QuestEvent_WaitForDrama`, `QuestEvent_RunDramaOnAdvance`, `QuestEvent_KungFuDramaName`, `QuestEvent_TappedDramaName`, `QuestEvent_RunKungFu` (default true) | String / Bool | 4,688, 895, 1,103, 535, 91, 48 | Scripted sequences around the event (keys of server-side scripts) |
| `QuestEvent_HideRadarIcon`, `QuestEvent_DisableEntityExamination`, `QuestEvent_CanTeleportToObjective` (default true), `QuestEvent_AllowQuickslot` (default true), `QuestEvent_IsFellowUseShared`, `QuestEvent_IsRemote`, `QuestEvent_DestroyOnQuestAdvance`, `QuestEvent_FailQuestWhenEventFails`, `QuestEvent_ShouldShowEscortVitals` (default true) | Bool | 166, 66, 3,028, 935, 428, 1,130, 341, 299, 112 | |
| `QuestEvent_EventVolumeString` | String | 20 | (meaning OPEN) |

<a id="s5-4"></a>
### 5.4 Event types (Confirmed fields and counts)

`QuestEvent_ID` values used, the number of entries of each type (all 82,280 entries),
the type-specific fields (present in most entries of the type), the class of the
compiled event object here (§6.2), and the meaning. Meanings follow from the fields;
those marked (LC) are Reported.

| ID | Entries | Characteristic fields | Compiled class | Meaning |
|---|---|---|---|---|
| 1 | 7,024 | `QuestEvent_RoleConstraint` (6,949), `QuestEvent_Detect`, `QuestEvent_Locations` | 1420 | Enter a detection volume |
| 2 | 83 | `QuestEvent_RoleConstraint` | 1429 | Leave a detection volume |
| 4 | 82 | monster selector, `QuestEvent_TerritoryDID` | 1432 | A monster-play character dies (LC) |
| 5 | 1,714 | `QuestEvent_NPCTalk` (947), `QuestEvent_DramaName` (1,282) | 1434 | Use an NPC or object |
| 6 | 17 | `QuestEvent_SkillQuestFlags`, `QuestEvent_Skill_AttackResultArray` | 1439 | A skill is applied (LC) |
| 7 | 7,343 | `QuestEvent_ItemDID` (7,038), `QuestEvent_HasInventoryItem` | 1426 | Use an item |
| 9 | 781 | `QuestEvent_RoleConstraint`, `QuestEvent_LeaveDetectionOverride` | 1417 | Stay inside a detection volume |
| 10 | 2,493 | `QuestEvent_ItemDID` (all), `QuestEvent_DestroyInventoryItems` | 1422 | Hand an item over (item used on another inventory) |
| 11 | 23,700 | `QuestEvent_NPCTalk` (20,549), `QuestEvent_DramaName` | 1433 | Talk to an NPC |
| 13 | 326 | `QuestEvent_ChannelingRoleConstraint`, `QuestEvent_ChannelingState`, `QuestEvent_ItemDID` | 1414 | Carry or channel something to a target |
| 14 | 630 | `QuestEvent_TimeLimit` (seconds, 1 … 86,400) | 1441 | A timer expires |
| 16 | 221 | `QuestEvent_ItemDID` (all), `Quest_Role` | 1425 | Talk through an item |
| 18 | 153 | `QuestEvent_PlayerLevel` (2 … 150) | 1430 | Reach a level |
| 19 | 315 | `QuestEvent_RoleConstraint`, `QuestEvent_KungFuDramaName`, `QuestEvent_DramaName` | 1415 | Clear a camp / defend a target (LC) |
| 20 | 213 | `QuestEvent_ChannelingState` | 1882 | A channelling fails |
| 21 | 2,351 | `QuestEvent_LandmarkDID` (2,332) | 1428 | Discover a landmark |
| 22 | 7,006 | `QuestEvent_MonsterDID` (3,835) or `QuestEvent_MonsterGenus_Array` (3,177), `QuestEvent_MonsterGenus_ExclusionArray` | 1431 | Defeat monsters |
| 24 | 1,116 | `QuestEvent_EmoteDID` (all), `QuestEvent_EmoteTargetDID` (563) | 1419 | Perform an emote (at a target) |
| 25 | 189 | monster selector, `QuestEvent_TerritoryDID` | 1436 | The player dies (LC) |
| 26 | 778 | `QuestEvent_SkillDID` or `QuestEvent_SkillQuestFlags`, `QuestEvent_Skill_AttackResultArray`, `QuestEvent_DailyMaximumIncrements` | 1440 | Use a skill with given combat results |
| 27 | 280 | `QuestEvent_RoleConstraint`, `QuestEvent_KungFuDramaName` | 1427 | A scripted fight (LC) |
| 29 | 114 | `QuestEvent_EscortPatrolName`, `QuestEvent_RoleConstraint` | 1421 | Escort (LC) |
| 30 | 90 | — | 1438 | The player character dies |
| 31 | 8,735 | `QuestEvent_ItemDID` (all), `QuestEvent_DestroyInventoryItems` (6,464), `QuestEvent_RequireUniqueItems` | 1424 | Hold items in the inventory |
| 32 | 10,348 | `QuestEvent_QuestComplete` (9,879) or `QuestEvent_QuestCompleteCategory` / `QuestEvent_AccomplishmentCompleteCategory`, `QuestEvent_QuestCompleteScope`, `QuestEvent_QuestComplete_SuppressQuestCountUpdate` | 1437 | Complete a quest or deed (or *N* of a category) |
| 33 | 128 | `QuestEvent_RecipeDID` | 1416 | Execute a crafting recipe |
| 34 | 4,366 | `QuestEvent_WorldEvent`, `QuestEvent_WorldEvent_Operator`, `QuestEvent_WorldEvent_Value` | 1859 | A world-event condition holds (§14) |
| 37 | 117 | `QuestEvent_Session_DID`, `QuestEvent_Session_Status` | 1905 | A session finishes with a status |
| 38 | 5 | `QuestEvent_ResourceSets`, `QuestEvent_NPCTalk` | 2163 | A resource set is reached (LC) |
| 39 | 88 | `QuestEvent_HobbyDID`, `QuestEvent_ItemDID` | 2226 | A hobby item (fishing) is obtained |
| 40 | 10 | `QuestEvent_IA_EventType`, `QuestEvent_IA_RunicType`, `QuestEvent_IA_ItemLevel` | 2265 | An item-advancement event |
| 43 | 14 | — | 2472 | Dismount |
| 45 | 685 | `QuestEvent_FactionDID`, `QuestEvent_ReputationTier` (1 … 10) | 2589 | Reach a reputation tier |
| 46 | 16 | — | 2643 | Being teleported |
| 56 | 197 | `QuestEvent_MonsterDID` (173) or a monster selector | 3610 | A monster enters the player's area of interest (LC) |
| 57 | 47 | `QuestEvent_InductionActionOverride`, `QuestEvent_CorpseUsage_*`, monster fields | 3681 | Use a corpse |
| 58 | 343 | `QuestEvent_ScriptCallback_Key` (2 … 219), `QuestEvent_Points` | 3804 | A script callback fires (key from mapper 0x230003C0) |
| 59 | 162 | `QuestEvent_BestowedQuestID` | 3865 | Another quest is bestowed |

Mapper 0x2300004F defines seven further types that no entry uses (0, 8, 12, 15, 17, 23 and
28); all but 0 appear in listen lists (§6.2).

Field details:

- `QuestEvent_Skill_AttackResultArray` holds `QuestEvent_Skill_AttackResult` enums
  (mapper 0x2300002A; 9, 10 and 11 dominate); `QuestEvent_SkillQuestFlags` is a 64-bit
  mask (62 distinct values).
- `QuestEvent_QuestCompleteScope` uses the quest-scope mapper (values 0, 1, 3, 10).
- `QuestEvent_WorldEvent_Operator` uses the operator mapper of §8.2 (3 = equal in 4,152 of
  4,367); `QuestEvent_WorldEvent_Value` is 0 … 200.
- `QuestEvent_Status` and `QuestEvent_Session_Status` are status codes (§8.3): failed
  (0x20000000, 17 + 17) or completed (0x30000000, 100).

<a id="s5-5"></a>
### 5.5 Monster selector (Confirmed)

`QuestEvent_MonsterGenus_Array`, `QuestEvent_MonsterGenus_ExclusionArray`,
`Quest_LootMonsterGenus_Array` and `Quest_LootMonsterGenus_ExclusionArray` hold
`Quest_MonsterGenus_Struct` elements (4,702 + 305 in events, 6,304 + 20 in loot tables).
Each field is optional; a monster matches an element when it matches every field present
(LC; the index of §16.3 supports exactly these fields).

| Field | Type | Present (all 11,331 selectors) | Matches the monster's |
|---|---|---|---|
| `Quest_MonsterGenus` | Bitfield32 (0x2300000A) | 5,221 | genus (`Agent_Genus` / classification genus); one bit, except 0 (771) and one value with three bits |
| `Quest_MonsterSpecies` | Enum (0x2300000B) | 4,630 | species |
| `Quest_MonsterSubspecies` | Enum (0x2300000C) | 966 | `Agent_Subspecies` |
| `Quest_MonsterAlignment` | Enum (0x2300000D) | 8 | alignment |
| `Quest_MonsterClass` | Enum (0x2300000E) | 556 | `Agent_Class` |
| `Quest_MonsterDivision` | Enum (0x23000061) | 4,352 | the monster's own `Quest_MonsterDivision` property |
| `Quest_MonsterRegion` | DID (territory) | 5,374 | the territory the division belongs to (`MonsterDivision_Control_DivisionToRegionMap`) |
| `QuestEvent_LandmarkDID` | DID (landmark) | 729 | being near that landmark (meaning OPEN) |

An exclusion array removes matching monsters from the inclusion set (LC).

> **OPEN:** How the client or server tests `QuestEvent_LandmarkDID` inside a selector
> (distance, or the landmark's area).

<a id="s5-6"></a>
### 5.6 Locations (Confirmed)

`QuestEvent_Location` structs: `QuestEvent_Location_Position` (Position,
`property-system.md` §4.6; all 5,435; flags 0x9B, 0xBB, 0xDB, 0xFB, 0x1B, 0x3B, 0x5B, 0x7B,
0x8B: region and landblock always, offset in all but the 3 of flag 0x8B), `QuestEvent_Location_Radius` (Float, 520;
0 … 350), `QuestEvent_Location_EntireLandblock` (Bool, 1,045; always true),
`QuestEvent_Location_RoleConstraint` (String, 5). These are author-given map points; the
quest-target index of §16 carries the generated ones.

<a id="s5-7"></a>
### 5.7 Event numbering (Confirmed, all 20,807 quests)

The compiled form gives every event entry three numbers. They are reproduced exactly by
the following rule, in all 20,807 records:

1. Walk `Quest_ObjectiveArray` **in stored order** (not in `Quest_ObjectiveIndex` order).
   For each objective:
   1. Take its **completion** conditions in stored order, then its **failure** conditions
      in stored order. Number them 1, 2, … within the objective: this is the
      **ConditionID**. Every condition takes a number, including a condition that holds
      no entry (24 completion conditions; in 12 objectives such a condition comes before a
      non-empty one, and skipping it would shift every later ConditionID of the objective).
   2. Inside a condition, take its elements in stored order:
      - a direct `QuestEvent_Entry` contributes itself, once;
      - a `QuestEvent_CompoundEvent` contributes its own elements in their stored order:
        each direct `QuestEvent_Entry` once, and each `QuestEvent_Entry_Array` all of its
        entries in their stored order (an empty array contributes nothing);
      - every other element (`Quest_CompletionConditionCount`,
        `Quest_Condition_NeverFinish`, `QuestEvent_CompoundProgressOverride`) contributes
        nothing.

      In this install no direct entry of a compound follows one of its entry arrays
      (§5.2), so "stored order" and "direct entries first, then the arrays" give the same
      result; a reader uses stored order.
   3. Each entry gets ObjectiveID = the objective's `Quest_ObjectiveIndex`.
2. Then walk `Quest_FailureConditionArray`: each `Quest_FailureCondition` is condition 1, 2,
   … with ObjectiveID **0**.
3. Then walk `Quest_TrackedConditionsArray` the same way, again with ObjectiveID 0 and
   ConditionIDs from 1.
4. **EventID** = the entry's position in this walk, starting at 1. EventIDs of a quest are
   exactly 1 … *N*.

Facts: the quest-level failure and tracked conditions both use ObjectiveID 0 and may
share ConditionIDs; `QuestEvent_IsFailureEvent` (§6.2) distinguishes them. Entries of
failure conditions have no `QuestEvent_EventOrder`; the compiled order value is then −1.

---

<a id="s6"></a>
## 6. The compiled quest object (0x70 record)

The 0x70 record of a quest is an object-state record (`wstate.md`). Its root object (the
quest class) has eleven attributes. Attribute names are not in the data; they are
identified by their name hash (`records.md` §1.5) and by content.

<a id="s6-1"></a>
### 6.1 Root attributes (Confirmed, all 20,807)

| Hash | Type | Target | Content |
|---|---|---|---|
| 0x19552D8 | reference | class 35 (map u32 → reference) | **Event table**: key = EventID → the event object (§6.2). 82,280 entries; key = the event's `QuestEvent_EventID` in every entry. |
| 0xE73DC88 | reference | class 37 (multimap u32 → reference) | **Objective index**: key = ObjectiveID → each event object of that objective (82,280 entries) |
| 0xDE16EF4 | reference | class 11 (map u32 → u32) | Required counts: key = ObjectiveID × 65,536 + ConditionID → `Quest_CompletionConditionCount` (196 entries) |
| 0x58579D3 | reference | class 11 | Medal treasure: key = `Quest_Medal_LootArray_Medal` → treasure DID (520 entries in 104 quests) |
| 0xDBA5B63 | reference | class 11 | Paper items: key = `Quest_PaperItemDID` → `Quest_PaperItemTier` (250 entries) |
| 0xE169424 | reference | class 35 | Loot index: key = item DID → class-182 list of loot-entry objects (class 1401 here; 2,046 objects, one per `Quest_LootTable` entry). A loot-entry object holds the entry's property set and a class-17 list of `Quest_LootItemProbability` values. |
| 0x63428B3 | reference or null | class 35 | Effect index: key = `QuestEffect_ApplyRole` → list of effect-application objects (643 quests, 686 keys) |
| 0xD4E96F3 | reference | class 176 | Completion comments (§7.3): one object (class 3910 here) per `Quest_CompletionCommentStruct` (994) |
| 0xFF40B92 | reference | script object (class 1602 here) | Usage permissions: one attribute holding a class-166 property set of `Usage_*` values (present in every quest, also when the 0x79 set has no `DefaultPermissionBlobStruct`). It is derived, not copied: it holds `Usage_AllowedWhileMounted` in all 20,807, `Usage_MonsterPlayerCanUse` in exactly the 638 monster-play quests, and `Usage_MinLevel` in 14,107 (the source struct has it in 4,268) |
| 0x79DD9A3 | reference | class 17 | Always empty |
| 0x16D4503 | int | — | 0 (15,090) or 1 (5,717) |

> **OPEN:** How the compiled permission set is derived from the source struct, the
> top-level `Usage_*` properties and the quest's flags (for example where its extra
> `Usage_MinLevel` values come from).

> **OPEN:** The meaning of root attribute 0x16D4503. It does not coincide with any single
> stored property (best partial matches: `Quest_NextQuest`, `Quest_Scope` = 3).

<a id="s6-2"></a>
### 6.2 Event objects (Confirmed, all 82,280)

There is one script object per event entry. Its class depends only on `QuestEvent_ID`
(the "compiled class" column of §5.4). All event classes share these attributes:

| Hash | Type | Content |
|---|---|---|
| 0x58F0A6E | reference → class 166 | The event's property set: the source entry's properties **plus** `QuestEvent_QuestDID` (the quest's own DID), `QuestEvent_ObjectiveID`, `QuestEvent_ConditionID`, `QuestEvent_EventID` (§5.7), `QuestEvent_DailyMaximumIncrements` (0 when the source has none), `QuestEvent_IsFailureEvent` (true on entries of failure conditions) and `Quest_Condition_NeverFinish` (true on the 2,218 events of conditions whose own flag is true and on all 343 tracked events; never stored false) |
| 0x6EB4612 | int | `QuestEvent_EventOrder`, or −1 when the source has none |
| 0xB5713A3 | reference → class 25 | **Sibling list**: the EventIDs of the other members of the same compound event (members as defined in §5.2, collected by §5.7 step 1.2), in ascending EventID order; empty for an entry that is not in a compound. Since every compound has at least two members, the list of a member is never empty. Holds, order included, in all 20,807 quests. |
| 0xC18ECB4 | reference → class 25 | **Listen list**: the event types this event reacts to (below) |
| 0xAE3F773 | reference or null → class 182 | The event's locations: one object per `QuestEvent_Location` (class 2507 here: a class-160 position and a float radius, −1 when the source has none) |
| 0x00C6983 | reference or null → class 182 | Effects applied when the event advances: effect-application objects |
| 0x2583094 | int | 0 in all |

Monster-defeat events (type 22) add two lists of **compiled selectors** (class 1402 here,
eight integer attributes: genus, species, subspecies, alignment, class, division,
territory DID, landmark DID; 0 = field absent): hash 0x30B9204 for inclusion (4,108
selectors) and 0x26B9204 for exclusion (301), plus two integer attributes (0x511C1DB,
0x5700552) holding DIDs (meaning OPEN).

**Listen lists.** The first element is normally the event's own type. Observed lists (all
events):

| Type | List | Events |
|---|---|---|
| any type *t* without the cases below | (*t*) | 77,085 |
| 1, 5, 11, 24 with `QuestEvent_WaitForDrama` = true and a `QuestEvent_DramaName` | (*t*, 17, 12) | 665 |
| 9 | (1, 21, 2) | 781 |
| 10 | (10, 31, 7) | 2,493 |
| 13 | (1, 7, 28, 15, 20) | 326 |
| 16 | (7, 16) | 221 |
| 19 | (19, [17, 12,] *x*, 23, 22, 28, 8) with *x* = 1 or 11 | 315 |
| 27 | (27, 11, 22, 28, 8) | 280 |
| 29 | (29, [17, 12,] *x*, 22, 28, 23, 8) with *x* = 1 or 11 | 114 |

The bracketed pair appears exactly when the drama rule of the second row holds; it was
also checked for types 19 and 29.

> **OPEN:** What decides *x* = 1 (detection) or 11 (NPC talk) in the lists of types 19 and
> 29 (14 + 2 and 16 + 3 events have 1).

<a id="s6-3"></a>
### 6.3 Effect-application objects (Confirmed)

958 objects (756 from `Quest_EffectArray`, 202 from `QuestEvent_ApplyEffectArray`; the
202 are exactly the items of the events' effect lists, attribute 0x00C6983 of §6.2),
class 700 here. Every one of the 958 has the same fifteen attributes (value encodings per
`wstate.md` §5.2):

| Hash | Type | Content (all 958) |
|---|---|---|
| 0x8CF9154 | int | The effect DID |
| 0x2712104 | float | Spellcraft level, `QuestEffect_Spellcraft`; −1 when the source has none (49); range −1 … 160 |
| 0x35D2793 | int | 512 |
| 0x7CB357C | int | 1 |
| 0x6B28A64, 0x832D143, 0x8BBFE54, 0x99E38B5, 0xCF31763 | int | 0 |
| 0xC1FF21D | 64-bit int | 0 |
| 0x03EAE14, 0x9FC3165 | double | Bit pattern 0xC7EFFFFFE0000000 (IEEE 754 binary64), that is −3.4028234663852886 × 10^38: the most negative finite **32-bit** float widened to 64 bits. It is neither the most negative finite double nor the bit pattern of the minimum signed 64-bit integer. Stored as `wstate.md` §5.2 gives for doubles (low 32-bit word first). |
| 0x7359352 | reference → class 699 | An object whose property set holds `Effect_QuestDID` and optional effect-generator data |
| 0x53151DF | reference → class 1626 | An object with five 64-bit integer attributes, all 0 |
| 0xF9D4CF4 | reference | Null |

---

<a id="s7"></a>
## 7. Roles and dialogue

<a id="s7-1"></a>
### 7.1 The role struct (Confirmed)

`Quest_Role` structs appear in `Quest_RoleArray`, `Quest_BestowalRoles`,
`Quest_GlobalRoles`, `Quest_NPCObjRoles` (per objective) and inside event entries.
Fields (95,886 non-empty role structs in all quests, plus 166 empty `Quest_Role` structs in event entries; 62,425 of them have an action, 66,576 an NPC):

| Property | Type | Meaning |
|---|---|---|
| `QuestDispenser_Action` | Enum (0x2300001D) | What the role does (§7.2) |
| `QuestDispenser_NPC` | DID | The NPC, monster or object that plays the role (absent when the role is bound by key) |
| `QuestDispenser_RoleName` | String | The role key inside the quest (ASCII; an action word plus an ordinal; 280 distinct). It is the join key with the NPC-side lists of §7.4 and the map notes of §17. |
| `QuestDispenser_RoleConstraint` | String | A role key that names the world object dynamically (2,196 distinct) |
| `Quest_ObjectiveIndex` | Int, **no dictionary default** | Objective the role belongs to. Stored only in the roles of `Quest_RoleArray`, never in the source role arrays. There it is ≥ 1 for actions 1–4, 7, 9, 10, 13, 14 (36,455 roles), stored as 0 for action 6 (bestowal, 14,929) and **absent** for action 5 (959). A reader keeps "absent" distinct from 0 (see below). |
| `QuestDispenser_RoleSuccessText` | StringInfo | The dialogue line |
| `QuestDispenser_RoleFailureText` | StringInfo | The line when the role's check fails (for example a missing item) |
| `QuestDispenser_TextArray` | Array of `QuestDispenser_TextStruct` | Variant lines: each struct has `QuestDispenser_Species` (Enum 0x2300000B, the player's species) and a success (and rarely a failure) text |
| `QuestDispenser_VOSoundID` | DID (0x2A) | Voice-over sound info for the line |
| `QuestDispenser_VOScriptID`, `QuestDispenser_StartScriptID` | Enum (0x23000097) | Script keys run with the voice-over or when the role starts |
| `QuestDispenser_SendVOToServer` | Bool | |
| `QuestDispenser_IsRemote` | Bool | The role can be played without being near the NPC |
| `QuestDispenser_DetectRadius` | Float, default 5.0 | 1 … 40 (detect roles) |
| `QuestDispenser_PrivateEncounterTemplate`, `QuestDispenser_OpenInstanceUI_PrivateEncounterTemplate`, `QuestDispenser_PortalDestination` (String), `QuestDispenser_TeleportationSceneID`, `QuestDispenser_SessionDID`, `QuestDispenser_OpenUIMenu_UIElementID`, `QuestDispenser_OpenUIMenu_SubPanel_UIElementID` | | Targets of teleport, instance, session and UI roles |
| `QuestDispenser_MissionTwistName`, `QuestDispenser_MissionTwistDescription`, `QuestDispenser_MissionTwistIcon` | StringInfo / DID | Mission variants |

**Objective of a role.** `Quest_ObjectiveIndex` has no default in the property dictionary,
so a reader must not substitute 0 when it is missing: 0 means "the bestowal stage" (action
6), while a missing value means "not bound to an objective". The 95,886 non-empty role
structs of all quests store it as follows:

| Container | Roles | `Quest_ObjectiveIndex` |
|---|---|---|
| `Quest_RoleArray`, action 6 | 14,929 | 0 |
| `Quest_RoleArray`, action 5 | 959 | absent |
| `Quest_RoleArray`, every other action | 36,455 | 1 … *n* |
| `Quest_BestowalRoles` | 12,171 | absent (the flattened form gives 0, §7.3) |
| `Quest_GlobalRoles` (all action 5) | 841 | absent |
| `Quest_NPCObjRoles` | 9,241 | absent (the enclosing objective gives it, §7.3) |
| `Quest_Role` of an event entry | 21,290 (plus 166 empty structs, not counted here; §5.3 counts all 21,456) | absent (the enclosing objective gives it, §7.3) |

**Action-5 roles** (leave an instance) belong to the quest as a whole. 841 of the 959 are
the flattened form of a quest-level `Quest_GlobalRoles` entry (837 quests with one, 2
with two; same fields apart from the role name); the other 118, one per quest, have no
stored source (part of the §7.3 OPEN). Neither form names an objective, all 959 carry
`QuestDispenser_PrivateEncounterTemplate`, and 554 entries of the NPC-side lists of §7.4
point at them. An implementer therefore assigns an action-5 role to **no objective**: it
is kept with an absent objective and offered for the whole time the quest is held,
whatever the current objective. This reading is derived by this spec writer from the
storage; no source states the run-time rule.

> **OPEN:** Whether a server limits an action-5 role to some objectives (for example to
> those whose events lie inside the private encounter it leaves). The executable does not
> name `Quest_RoleArray` (§18), so the rule is not in the client.

> **OPEN:** How a variant line of `QuestDispenser_TextArray` is chosen (by the player's
> species, presumably) and whether the struct's own success text is then shown at all.
> The executable never names `QuestDispenser_Species` or the text-array property (§18),
> so the choice is made by script code or by the server.

<a id="s7-2"></a>
### 7.2 Role actions (Confirmed)

`QuestDispenser_Action` values in `Quest_RoleArray` (52,343 roles):

| Value | Roles | Meaning |
|---|---|---|
| 1 | 10,258 | A comment line spoken by an NPC during an objective (no advance) |
| 2 | 24,354 | Talk to an NPC; advances the objective |
| 3 | 904 | Teleport |
| 4 | 221 | Talk through an item |
| 5 | 959 | Leave an instance |
| 6 | 14,929 | Bestow (offer) the quest |
| 7 | 192 | Detection |
| 9 | 111 | Session |
| 10 | 5 | Wait for a resource set |
| 13 | 27 | Open an instance-selection UI |
| 14 | 383 | Mission teleport |

Mapper values 0, 8 and 12 (undefined, failure, open UI menu) do not occur in any role
struct.

Invariant (every role of `Quest_RoleArray`): action 6 ⇔ `Quest_ObjectiveIndex` stored with
value 0; action 5 ⇔ `Quest_ObjectiveIndex` absent; every other action ⇔ a stored value
≥ 1. It holds only when an absent value is kept distinct from 0 (§7.1).

<a id="s7-3"></a>
### 7.3 The flattened role list and completion comments (Confirmed)

`Quest_RoleArray` is the quest's complete role list, the one the NPC-side lists of §7.4
point into. It is derived from the source roles: the bestowal roles (`Quest_BestowalRoles`,
which store only NPC and text) become action-6 roles of objective 0; each objective's
`Quest_NPCObjRoles` and the `Quest_Role` of each event become roles of that objective with
the action of their kind; `Quest_GlobalRoles` (always action 5, with a private-encounter
template) are included as well. Every role gets a role name.

In 17,247 of the 20,807 records the multiset of (objective, success-text token) of
`Quest_RoleArray` equals that of the source roles exactly; in 2,645 the flattened list has
more entries, in 81 fewer, and in 834 the same number with differences.

> **OPEN:** The rule for the remaining 3,560 records (extra roles in the flattened list,
> for example action-1 roles without an NPC). A reader must use `Quest_RoleArray` as the
> authority and treat the source role arrays as authoring detail.

**Completion comments.** `Quest_CompletionCommentArray` (761 quests, 994 structs): each
`Quest_CompletionCommentStruct` has `QuestDispenser_NPCArray` (NPC DIDs, 1,004; and in 20
structs `QuestDispenser_RoleConstraint` strings instead) and `QuestDispenser_TextArray`
(1,757 `QuestDispenser_RoleSuccessText` values): lines those NPCs say once the quest is done
(LC). In the compiled form each struct becomes one object with a class-176 list of
StringInfo items, a class-18 set of the NPC DIDs and a class-134 set (22 objects, always
empty).

<a id="s7-4"></a>
### 7.4 NPC-side role lists (Confirmed)

NPC, monster, item and class-5785 records carry `QuestDispenser_ActiveQuests` (10,829
records; 0 … 334 entries; 43,405 `QuestDispenser_QuestRole` structs with
`QuestDispenser_Quest` (DID) and `QuestDispenser_RoleName`). One more struct sits at the top
level of one record.

- 43,401 of these 43,406 entries name a quest whose `Quest_RoleArray` has a role with the
  same role name **and** the same `QuestDispenser_NPC`; the other 5 match neither.
- Of the quest roles that name an NPC, the share listed on that NPC is: action 6 13,375 of
  13,621; action 2 20,289 of 20,386; action 1 7,527 of 9,928; actions 3, 4, 5, 7, 9, 10,
  13, 14 almost all.
- 6,163 quest roles name no NPC (they are bound by role key or by script).

> **OPEN:** (implementation census, 2026-10-07) The counts above are those of the four
> holder kinds named (by root class here: NPCs 1724 40,461 entries, monsters 1723 2,232,
> items 796 633, class 5785 80; together 43,406, of which 43,401 match). Reading every
> property set finds one more `QuestDispenser_QuestRole` entry, in the
> `QuestDispenser_ActiveQuests` array of one record of root class 763, and it matches (role
> name and holder as `QuestDispenser_NPC`), giving 43,407 and 43,402. What kind of record is
> that holder, and should §7.4 (and §3.2, which lists the same four sources) include it?

A client uses these lists to know which quests an NPC takes part in (for example to show
an offer marker); a server uses the quest side.

<a id="s7-5"></a>
### 7.5 Voice-over (Confirmed)

`QuestDispenser_VOSoundID` (20,369 values on roles; 9,562 distinct) is a sound-info DID
(`sound.md` §4). `QuestDispenser_VOScriptID` (24,338) is a script key; one value covers
20,449 of them. `QuestControl` gives the default script keys `Quest_DefaultBestowScriptID`
and `Quest_DefaultCommentVOScriptID`.

<a id="s7-6"></a>
### 7.6 Where the dialogue of a quest comes from (summary)

| Moment | Data |
|---|---|
| Offer | Action-6 role of `Quest_RoleArray` with the NPC's DID; line `QuestDispenser_RoleSuccessText` (or a species variant) |
| During an objective | Action-1 roles of the objective (comments), action-2 roles (talk to advance; `QuestDispenser_RoleFailureText` when the event's check fails) |
| On completion | `Quest_CompletionCommentArray` lines for the listed NPCs |
| Barks | Colour-text lists (§7.8) |
| Free-standing comments | NPC comment records (§7.7) |

<a id="s7-7"></a>
### 7.7 NPC comment records (Confirmed)

**Comment** (kind 2246; 859 records): `NPCComment_Title`, `NPCComment_WindowLabel`,
`NPCComment_Content`, `NPCComment_CloseButtonText` (StringInfo, all), `NPCComment_VOScriptID`
(Enum, 0x23000097), and `DefaultPermissionBlobStruct` with a **fixed shape** in all 859:
exactly 2 `Usage_QuestRequirements` entries, 5 `Usage_QuestRequirements_OrList` entries, 3
`Usage_WorldEvent_AllConditionList` entries, 3 `Usage_WorldEvent_AnyConditionList`
entries, 3 `Usage_RequiredRaces` entries and 4 `Usage_RequiredClassList` entries, plus
every scalar usage field. Unused slots hold zeros: operator 0, quest DID 0, status 0, world
event 0, race 0, class 0 (1,287 of 1,718 AND entries, 4,153 of 4,295 OR entries).

Reader rule: an entry whose operator is 0 or whose target DID is 0 is an empty slot and
is skipped.

**Comment list** (kind 2247; 223 records): `NPCComment_CommentArray`, an array of
`NPCComment_DIDComment` DIDs (907; all comment records). NPCs (404), monsters (20) and
items (3) name one list each through `NPCComment_DIDCommentArray`.

> **OPEN:** Which comment of a list is shown when several pass their requirements (first,
> random, all).

<a id="s7-8"></a>
### 7.8 Colour text (barks) (Confirmed)

Kind 978 (1,206 records): `NPC_ColorTextArray` (1,203 records; 1 … 52 `NPC_ColorText`
StringInfos) and `NPC_ReputationColorTextArray` (3 records; structs with `NPC_ColorText`,
`NPC_ColorText_MinReputationTier`, `NPC_ColorText_MaxReputationTier`, tiers of mapper
0x230001F3, 0 = no bound). NPCs name a list through `NPC_ColorTextFile` (384 values; 284 to
an existing list, 100 are 0).

Timing comes from `NPCColorControl` (§2.3): `Quest_ColorTextDetectionRadius` (10.0, metres),
`NPC_TextDuration` (10.0, seconds) and `Quest_ColorTextInterval` (5; unit OPEN).

> **OPEN:** How a line is picked (random or in order) and how radius, duration and
> interval drive it.

<a id="s7-9"></a>
### 7.9 Scenes (Confirmed)

Kind 1468 (1,172 records): `Scene_Name` (StringInfo) and `Scene_KeyframeArray` (1 … 6
`Keyframe_Struct`, 7,027 in all). A keyframe has `Keyframe_Subtitle` (StringInfo, all),
`Keyframe_VideoTrack` (0x41 surface DID, 1,105), `Keyframe_MovieTrack` (DID of a
movie-library record, class 1982 here, 65), `Keyframe_Duration` (Float, 1) and
`Keyframe_ShowRandomImageFromUIL` (Bool, 1).

The 0x70 record holds the same scene as one class-427 native (`wstate.md` §5.4,
"scene info") whose fields are, in all 1,172 records and all 7,027 entries:

| Native field (`wstate.md` §5.4) | Meaning |
|---|---|
| title | `Scene_Name` |
| owner | The scene's own DID |
| entry_count | Number of keyframes |
| entry index | Keyframe number 1 … *k* |
| first f32 | `Keyframe_Duration`, or −1 when absent (7,026 of 7,027) |
| second f32 | 0 except in the 1,105 entries with a video track: 8.03 … 10.51 (meaning OPEN; a length in seconds fits) |
| first u32 | 0 in all |
| StringInfo | `Keyframe_Subtitle` |
| two u32 | `Keyframe_MovieTrack` DID (or 0), then a kind: 12 with a movie track (65), 9 with a video track (1,105), 8 otherwise (5,857) |
| bool8 | `Keyframe_ShowRandomImageFromUIL` |

The video-track DID is not repeated in the native; a reader takes it from the 0x79 set.

---

<a id="s8"></a>
## 8. Prerequisites and usage requirements

<a id="s8-1"></a>
### 8.1 Permission struct (Confirmed)

`DefaultPermissionBlobStruct` (17,531 quests) holds `Usage_*` fields. In quest records:

| Field | Recs | Values |
|---|---|---|
| `Usage_MinLevel` (default 1), `Usage_MaxLevel` | 4,268, 457 | −1 … 160; −1 … 159 (−1 = no bound) |
| `Usage_MinLevel_FloatToCap` | 202 | true |
| `Usage_QuestRequirements` | 9,191 | 1 … 25 entries (§8.2) |
| `Usage_QuestRequirements_OrList` | 222 | 1 … 13 entries |
| `Usage_WorldEvent_AllConditionList`, `Usage_WorldEvent_AnyConditionList` | 2,408, 278 | `WorldEvent_Condition` structs (§14.2) |
| `Usage_RequiredClassList` (`Usage_ClassType`), `Usage_RequiredRaces` (`Usage_RequiredRaceEntry`) | 271, 170 | class and race enums |
| `Usage_RequiredFaction` (`…_DataID`, `…_Tier`) | 467 | faction DID and tier 1 … 12 |
| `Usage_RequiredTrait`, `Usage_RequiredTrait_OrList`, `Usage_BarringTrait`, `Usage_RequiredEffect` | 81, 62, 7, 31 | trait or effect DIDs |
| `Usage_RequiredCraftProfession`, `Usage_RequiredCraftProficiency`, `Usage_RequiredCraftProficiencyLocked`, `Usage_RequiredHobby` | 142, 63, 8, 20 | |
| `Usage_RequiredNeighborRelationship` | 36 | relationship DID and level 1 … 9 |
| `Usage_MinGloryRank`, `Usage_SessionPlayerCanUse`, `Usage_RequiresSubscriber`, `Usage_BarrAdvancedStartCharacters`, `Usage_RequiredAccountToken`, `Usage_BarredAccountPropertyArray` | 11, 64, 5, 29, 7, 6 | |

The general semantics of `Usage_*` fields belong to the usage/permission subsystem and are
not specified here.

<a id="s8-2"></a>
### 8.2 Quest requirements (layout Confirmed; evaluation Reported)

A `Usage_QuestRequirement` struct has `Usage_Operator` (Enum 0x23000039), `Usage_QuestID`
(DID) and `Usage_QuestStatus` (status code, §8.3; missing in 1 of 13,003 entries).

Operator values: 1 ≥, 2 ≠, 3 =, 4 <, 5 ≤, 6 >, 0 undefined. Counts in quest AND lists: 1:
1,226; 2: 2,963; 3: 8,805; 4: 2; 6: 7. In OR lists: 1: 14; 2: 7; 3: 641.

Evaluation (Reported, LC): every entry of `Usage_QuestRequirements` must hold, and at least
one entry of `Usage_QuestRequirements_OrList` must hold when that list is present; an entry
holds when the player's status of the named quest compares to the stored status with the
operator.

> **OPEN:** How a status comparison is done: numerically on the 32-bit code (so that
> "≥ underway at objective 3" also accepts completed and failed), or by a rule that
> knows the raw status. The executable names `Usage_QuestStatus` only in UI code and never
> names `Usage_Operator` (§18), so the evaluation lives in script code or on the server.

<a id="s8-3"></a>
### 8.3 Status codes (Confirmed)

Mapper 0x23000017 (43 entries) defines 32-bit status codes:

| Bits 28–31 | Meaning | Bits 0–27 |
|---|---|---|
| 0x1 | Underway | Objective number (0 = any; 1 … 35 named in the mapper) |
| 0x2 | Failed | 0 |
| 0x3 | Completed | 0 |
| 0x4 | Abandoned | 0 |
| 0x5 | Tapped (LC: a quest whose credit was taken by another player) | 0 |

The mapper also names the masks 0xF0000000 (raw status) and 0x0FFFFFFF (objective number).
Values used in quest requirements: 0x10000000 … 0x10000016 and 0x30000000 (completed:
8,200 of 13,002 AND entries).

<a id="s8-4"></a>
### 8.4 Faction tiers (Confirmed)

`Usage_RequiredFaction_Tier`, `QuestEvent_ReputationTier`, `NPC_ColorText_*ReputationTier`
and the faction tier fields of §13 use mapper 0x230001F3: tiers 1 … 12, 0 undefined.

---

<a id="s9"></a>
## 9. Rewards

<a id="s9-1"></a>
### 9.1 Tier properties (Confirmed)

A tier property selects an amount from a reward-level record (§9.2). All tier properties
use mapper 0x23000107; its keys 1 … 6 are the ordinary tiers, and 11, 16, 400 … 403,
500 … 502, 1,000 and 2,000 are special tiers (labels in the mapper).

| Tier property (quest) | Recs | Values | List in the reward-level record | Amount property |
|---|---|---|---|---|
| `Quest_ExpTier` | 12,541 | 1 … 6 | `QuestReward_ExpEntryList` | `QuestReward_ExpAmount` |
| `Quest_GoldTier` | 11,550 | 1 … 6 | `QuestReward_MoneyEntryList` | `QuestReward_MoneyAmount` |
| `Quest_ItemExpTier` | 9,674 | 1 … 6, 11, 16, 2,000 | `QuestReward_ItemExpEntryList` | `QuestReward_ExpAmount` |
| `Quest_MountExpTier` | 5,502 | 0 … 6, 1,000, 2,000 | `QuestReward_MountExpEntryList` | `QuestReward_ExpAmount` |
| `Quest_CraftExpTier` (with `Quest_CraftProfessionDID`) | 16 | 6 | `QuestReward_CraftExpEntryList` | `QuestReward_ExpAmount` |
| `Quest_GloryTier` | 331 | 1 … 6, 400 | `QuestReward_GloryEntryList` | `QuestReward_GloryAmount` |
| `Quest_TurbinePointTier` | 2,574 | 1 … 6, 400, 500 … 502 | `QuestReward_TurbinePointEntryList` | `QuestReward_TurbinePointAmount` |
| `Quest_MithrilCoinsTier` | 1,952 | 2 … 6 | `QuestReward_MithrilCoinEntryList` | `QuestReward_MithrilCoinAmount` |
| `Quest_SessionPointsTier` | 788 | 0, 2,000 | `QuestReward_SessionPointsEntryList` | `QuestReward_SessionPointsAmount` |
| `Quest_SessionPointsFailureTier` (mapper 0x23000108) | 5 | 0, 1 | `QuestReward_SessionFailurePointsEntryList` (key `QuestReward_FailureTier`) | `QuestReward_SessionFailurePointsAmount` |
| `Quest_PositiveFaction` / `Quest_NegativeFaction` → `Quest_RepTier` | 9,860 / 53 | 1 … 6, 1,000, 2,000 | `QuestReward_RepEntryList` | `QuestReward_RepAmount` |
| `QuestTreasure_Virtue_XP_Tier` (on the treasure, §9.4) | 9,429 | 0 … 6, 11, 400 … 403, 1,000 | `QuestReward_VirtueExpEntryList` | `QuestReward_ExpAmount` |
| `Quest_PaperItemAwards` → `Quest_PaperItemTier` | 250 | 2, 3, 4, 6 | `QuestReward_PaperItemsEntryList` | `QuestReward_PaperItems` (Float) |

Each list element is a struct with `QuestReward_Tier` (or `QuestReward_FailureTier`) and the
amount. Lookup: the amount of the element whose tier equals the quest's tier value; a tier
value without an element gives no reward of that kind (for example key 2,000 in
`Quest_ItemExpTier`, which no list contains). Tier 0 means none.

The reward-level record also has `QuestReward_Accomplishment_ExpEntryList`,
`…_Accomplishment_MoneyEntryList` and `…_Accomplishment_RepEntryList` (tiers 2 … 6, and
1,000 for reputation).

> **OPEN:** Which deed properties select the `QuestReward_Accomplishment_*` lists (deeds
> carry `Quest_ExpTier` on only 330 records but most carry `Quest_RewardLevelDID`,
> `Quest_TurbinePointTier` and `Quest_PositiveFaction`), and whether a deed's tiers read the
> ordinary or the accomplishment lists.

> **OPEN:** The unit of `QuestReward_MoneyAmount` and `Quest_GoldToGive` (copper is
> likely) and the meaning of the float `QuestReward_PaperItems` (0.1 … 0.5).

<a id="s9-2"></a>
### 9.2 Reward-level records and the default table (Confirmed layout; choice Reported)

Every reward-level record (160) has the fifteen lists of §9.1, each with the same tiers in
every record (for example `QuestReward_ExpEntryList`: tiers 1 … 6; `QuestReward_ItemExpEntryList`:
1 … 6, 11, 16; `QuestReward_VirtueExpEntryList`: 1 … 6, 11, 400 … 403, 1,000).

`QuestControl_RewardLevelDIDArray` (in `QuestControl`) has exactly 160 elements, all
distinct, and they are exactly the 160 reward-level records. Element *i* (1-based) is the
table for level *i*: amounts grow with *i* in every list checked (tier-4 experience is
monotonic over all 160).

Choosing the table for a quest:

1. If the quest has `Quest_RewardLevelDID`, use that record (7,154 quests; two quests name
   a quest record instead, which a reader treats as "no table"). Compared with the quest's
   `Quest_ChallengeLevel`, the named table is the one of the same level in 6,584 quests,
   a higher level in 305 and a lower in 250.
2. Otherwise (Reported, LC) use element *L* of `QuestControl_RewardLevelDIDArray`, where *L*
   is the quest's level (§4.3), and the highest level when *L* is 0.

> **OPEN:** Step 2 for quests whose level comes from an override property (a server would
> use the player's level), and for levels above 160.

<a id="s9-3"></a>
### 9.3 Fixed amounts (Confirmed)

`Quest_ExpToGive` (2), `Quest_GoldToGive` (6), `Quest_GloryToGive` (12) and
`Quest_PosRepToGive` (185; 2,500 … 280,000) give fixed amounts. LC reports that a fixed
amount replaces the tier amount of the same kind; `Quest_NegRepToGive` (LC) does not occur
in this install.

<a id="s9-4"></a>
### 9.4 Quest treasure (Confirmed)

Treasure records (9,479; all properties at the top level):

| Property | Recs | Elements | Meaning |
|---|---|---|---|
| `QuestTreasure_FixedItemArray` | 5,904 | 1 … 33 `QuestTreasure_ItemStruct` (9,081) | Items always given |
| `QuestTreasure_SelectableItemArray` | 1,606 | 2 … 12 structs (5,456) | Items of which the player chooses one (LC) |
| `QuestTreasure_ItemStruct` | | `QuestTreasure_Item` (item DID) and `QuestTreasure_ItemQuantity` (Int; present in all 14,537; 1 … 50,000) | |
| `QuestTreasure_FixedTitleArray`, `QuestTreasure_SelectableTitleArray` | 2,469, 13 | `QuestTreasure_Title` DIDs (2,471; 38) | Titles (§12.1) |
| `QuestTreasure_FixedEmoteArray`, `QuestTreasure_SelectableEmoteArray` | 10, 5 | `QuestTreasure_Emote` DIDs | Emotes (§12.2) |
| `QuestTreasure_FixedTraitArray` | 286 | `QuestTreasure_Trait` DIDs (301) | Traits |
| `QuestTreasure_FixedVirtueArray` | 565 | `QuestTreasure_Virtue_Struct`: `QuestTreasure_Virtue` (PID of the virtue's rank property; 64 distinct) and `QuestTreasure_Virtue_Increment` (1 … 35) | Virtue ranks |
| `QuestTreasure_Virtue_XP_Tier` | 9,429 | tier (§9.1) | Virtue experience |
| `QuestTreasure_FixedBillingTokenArray` | 122 | `QuestTreasure_BillingToken` enums (mapper 0x230000C4; 124 distinct) | Account unlocks |

`QuestTreasure_FixedRunicArray` / `…SelectableRunicArray` (LC: relics) do not occur in this
install. 50 treasure records have no `QuestTreasure_Virtue_XP_Tier`.

<a id="s9-5"></a>
### 9.5 Reputation (Confirmed)

`Quest_PositiveFaction` (10,057) and `Quest_NegativeFaction` (104) are structs with
`Quest_FactionDID` (faction DID; absent in 35 + 42) and `Quest_RepTier` (tier, §9.1; absent in
197 + 51). The amount is the `QuestReward_RepAmount` of the tier, or `Quest_PosRepToGive`
when present; the negative struct subtracts (LC).

<a id="s9-6"></a>
### 9.6 Medals and points (Confirmed)

`Quest_Medal_LootArray` (104 quests; always 5 entries): `Quest_Medal_LootArray_Medal` (1 … 5,
mapper 0x230003BB) and `Quest_Medal_LootArray_TreasureDID`. `QuestControl_MedalThresholds`
gives point thresholds per medal (`Quest_Points`, `Quest_Medal`; 150, 400, 650 and 951 for
medals 2, 3, 4 and 5 here) and `QuestControl_Points_Start_Mapping` the start score per
`Quest_Points_Start` value (0, 400 and 1,000 here). Tracked events (§5.1) add or subtract
`QuestEvent_Points` (−500 … 400).

> **OPEN:** Medal 1 has no threshold entry (presumably the medal below 150 points).

<a id="s9-7"></a>
### 9.7 Items, loot and effects (Confirmed)

- `Quest_GiveItemArray` (679 quests): `Quest_ItemStruct` with `Quest_ItemDID` and optional
  `Quest_ItemQuantity` (2 … 12; 1 when absent) — items handed over at bestowal (LC). Inside
  events the same array is given when the event advances (`QuestEvent_GiveItemsOnAdvance`).
- `Quest_LootTable` (1,087 quests; 1 … 18 `Quest_LootItemStruct`): `Quest_LootItemDID`,
  `Quest_LootItemProbability` (1 … 200) or `Quest_LootItemProbabilityArray`, and the monsters
  that drop it: `Quest_LootMonsterDID` or monster selectors (§5.5). Extra drops active while
  the quest is held (LC).
- `Quest_EffectArray` (643 quests; 756 `Quest_Effect`): `Quest_DataID` (effect DID),
  `QuestEffect_ApplyRole` (2: 596, 4: 93, 7: 67; mapper 0x23000018: applied on completion,
  on bestowal, on failure), `QuestEffect_Spellcraft` (41; −1 … 160).

<a id="s9-8"></a>
### 9.8 Quest control values (Confirmed)

`QuestControl` also holds (values observed here, to be read at run time):
`Quest_MaxActiveQuestsFromSubscription` (5), `Quest_PEBestowalLimit` (5),
`Quest_FellowshipShareQuestRange` (30.0), `Quest_PlayerLock_ValidateDistance` (80.0),
`Quest_PlayerLock_ValidateInterval` (600), `QuestControl_CancelledPendingQuestTimeout` (120.0),
`QuestSurvey_Frequency` (0.0), `Quest_MinChallengeLevelArray` (structs of
`Quest_MinChallengeLevel`, `Quest_MaxChallengeLevel`, `Quest_DefaultLevelDifference`),
`QuestControl_CategoryToChallengeLevelOverrideProperty_Hash` (8 category → override PID
pairs), `QuestControl_LockingAutoBestowEvents`, and
`Quest_TeleToObjective_PaperItemServiceToCategories_Array`.

> **OPEN:** The units of the distances (metres are likely) and the role of
> `Quest_MinChallengeLevelArray` and of the limiter record `QuestCategoryLimiterControl`.

---

<a id="s10"></a>
## 10. Deeds

<a id="s10-1"></a>
### 10.1 Deed fields (Confirmed)

| Property | Type | Values |
|---|---|---|
| `Accomplishment_UITab` | Enum (0x2300001C) | 1 … 91, 77 distinct: the deed-log tab |
| `Accomplishment_Category` | Enum (0x2300001B) | 19 distinct of 1 … 41: deed type (exploration, slayer, lore, reputation, event, class- and race-specific groups) |
| `Accomplishment_UIFilter` | Enum (0x2300010A) | 1 … 4 (4,765 deeds) |
| `Accomplishment_MinLevelToStart` | Int, default 1 | 1 … 160 |
| `Accomplishment_ShowProgressBar` | Bool | true (2,472) |
| `Accomplishment_AccelerationCategory` | Enum (0x2300031F) | 1 (1,458), 2 (462) |
| `Accomplishment_CanBeAccelerated` | Bool, default true | stored false (41) |
| `Accomplishment_RequiredTrait` | DID | trait (2 distinct) |
| `Quest_IsHidden` | Bool | true (549) |

Deeds use the same objectives, events and rewards as quests (§5, §9); 1,940 deeds carry
`Quest_MithrilCoinsTier` and 2,555 `Quest_TurbinePointTier`.

<a id="s10-2"></a>
### 10.2 Directories (Confirmed)

An accomplishment directory's property set has `Accomplishment_List`, an array whose
elements are either `Accomplishment_File` (one quest DID) or `Accomplishment_Set` (an array of
2 … 30 `Accomplishment_File` DIDs; 686 sets with 1,946 members in all directories). Over
the 29 directories the lists name 6,455 DIDs, all quest records.

| Directory | Reached through | Entries |
|---|---|---|
| Main deed directory | WEENIECONTENT `AccomplishmentDirectory` | 3,252 |
| Auto-bestow directory | WEENIECONTENT `AutoBestowQuestDirectory` | 1,284 quests and deeds granted without an NPC |
| Per class | `AdvTable_AccomplishmentDirectory` of the class tables | |
| Per race | `RaceTable_AccomplishmentDirectory` of the race tables | |
| Monster play | `MonsterPlay_AccomplishmentDirectory` | |

LC uses the class and race directories to attach a class or race requirement to their
deeds.

The 0x70 record of a directory (class 838 here) is a compiled index with eight root
attributes: a map from each listed DID to a list of 1 … 83 objects (class 1418 here; each
holds the DID and its position 1 … *n* in the list, other fields 0 or null; 0x0B48813); a
plain list and a set of all listed DIDs (0x3010353, 0x4E8A613); a map from each member of a
set to its set (class 1860: a list of the set's DIDs; 0x163DD43); a map DID → list (138
entries; 0x3AD9FD3); a map from small keys to objects of classes 2733, 2734, 3626, 3867,
5608 and 5719 holding further DID lists (0x41F1613); and two empty containers.

> **OPEN:** What the lists of class-1418 objects (one per deed, 1 … 83 objects) and the
> keyed objects of attribute 0x41F1613 stand for. The object count does not equal the
> deed's number of objectives (3,373 of 11,696) or events (3,346).

<a id="s10-3"></a>
### 10.3 Version list (Confirmed)

`AccomplishmentVersionControl`: `Accomplishment_Version_List` of 21
`Accomplishment_Version_Entry` structs, each with `Accomplishment_Version_Number` (Enum;
3,400 … 4,900) and an `Accomplishment_List` (0 … 227 deeds; 939 in all).

> **OPEN:** What the version entries are used for (retroactive deed grants on a content
> update is a guess).

<a id="s10-4"></a>
### 10.4 Accomplishment control (Confirmed)

`AccomplishmentControl` holds `Accomplishment_UITabElements_Array` (81 entries mapping an
`Accomplishment_UITab` to UI element IDs for the normal and the monster-play deed log),
`Accomplishment_DeedUIGroups_Array` (10 groups of UI tabs, `Accomplishment_DeedUIGroup` from
mapper 0x23000495), `Accomplishment_Tree_IconArray` (24 entries: an
`Accomplishment_Tree_AccomplishmentCategory` with underway and completed images, 0x41),
acceleration settings (`Accomplishment_DeedAccelerationPropertyMap_Array`,
`Accomplishment_DeedAcceleration_MithrilPurchase_Effects`) and tooltip layout fields.

---

<a id="s11"></a>
## 11. Quest arcs (Confirmed)

Arc records (344): `QuestArc_Name` (StringInfo) and `QuestArc_Quest_Array` (0 … 29
`QuestArc_Quest_Entry` quest DIDs; 1,589 in all, all distinct: a quest belongs to at most
one arc). The arc directory lists all arcs in `QuestArcDirectory_QuestArc_Array`; its 0x70
record holds a class-11 map from each of the 1,589 quests to its arc's DID (the inverse
index).

---

<a id="s12"></a>
## 12. Titles and emotes

<a id="s12-1"></a>
### 12.1 Titles (Confirmed)

Title records (3,091), all six properties in every record: `Title_String` and
`Title_Description` (StringInfo), `Title_Category` (Enum 0x2300007A; 23 values used),
`Title_Exclusion_Group` (Enum 0x23000143; 0 in 2,747; 1 … 25 otherwise: titles of one
non-zero group exclude each other, presumably), `Title_Priority` (Int 1 … 16) and
`Title_Icon` (0x41; 13 distinct). Titles are granted by treasure (§9.4), craft
professions, version grants (`VersionAccountTitleBestow`: per partner and billing group a
title list and a stamp property), nationalities, kinship themes, hobbies, pedigrees and
item uses (§3.2).

<a id="s12-2"></a>
### 12.2 Emotes (Confirmed)

Emote records (273): `Emote_CommandString`, `Emote_Description`, `Emote_SourceText`,
`Emote_Message_FirstPerson`, `Emote_Message_FirstPersonWithTarget`,
`Emote_Message_ThirdPerson`, `Emote_Message_ThirdPersonWithTarget` (StringInfo, all),
`Emote_IconImage` (0x41), `Emote_ScriptID` and `Emote_Mounted_ScriptID` (Enum 0x23000097; 0
for the mounted script in 152), `Emote_AllowedWhileMounted`, `Emote_MustBeGranted` (Bool,
all; 144 must be granted), `Emote_HideInEmotesPage` (25), `Emote_MTXExclusive` (40),
`Emote_PreviewCameraOverride` (38; mapper 0x230000B9) and `Emote_WebStoreItem` (64).

The emote directory lists all 273 in `Emote_AvailableList` (`Emote_ID`) and names one
`Emote_AFK_Emote`.

---

<a id="s13"></a>
## 13. Reputation (Confirmed)

<a id="s13-1"></a>
### 13.1 Factions

Faction records (114): `Reputation_Faction_Name`, `Reputation_Faction_Description`
(StringInfo); `Reputation_Faction_AdvancementTable` (DID of a progression record, class 849
here, 13 distinct); `Reputation_Faction_TierNameProgression` (DID, class 2402 here, 6
distinct); `Reputation_Faction_CurrentTier_PropertyName`,
`Reputation_Faction_EarnedReputation_PropertyName`, `Reputation_Faction_GlobalCap_PropertyName`
(PIDs, each distinct per faction: the player properties that hold the faction's state);
`Reputation_LowestTier` (1 in all), `Reputation_HighestTier` (3 … 12),
`Reputation_Faction_DefaultTier` (1 … 7), `Reputation_Faction_LowestMonetizedTier` (7
records), `Reputation_Faction_DisableAcceleration` (2), `Reputation_AccountModifier` (69) and
`WebStoreAccountItem_DataID` (7).

<a id="s13-2"></a>
### 13.2 Account modifiers

Modifier records (69): `AccountModifier_Name`, `AccountModifier_Description`,
`AccountModifier_IsVIPExclusive` (true), `Reputation_AccountModifier_HighWater_PropertyName`
(PID) and `Reputation_AccountModifier_ModifierList` (5 structs of
`Reputation_AccountModifier_Tier` and `Reputation_AccountModifier_FloatMod`, 0 … 0.35).

<a id="s13-3"></a>
### 13.3 Reputation control

`ReputationControl`: `Reputation_FactionTable` (113 faction DIDs; one faction record is not
listed), `Reputation_GroupModifierTable` (DID), `Reputation_BarterReward_Array` (23 tier →
amount pairs), `Reputation_BonusAwardedForMithril`, `ReputationControl_BonusRep_SoftCap` and
`…_HardCap` (Int64).

---

<a id="s14"></a>
## 14. World events

<a id="s14-1"></a>
### 14.1 Kinds (Confirmed)

A world event is a named piece of shared world state held in the world property named by
`WorldEvent_WorldPropertyName` (a PID; 1,530 distinct over 1,574 boolean events and 672
over 699 integer events).

| Property | Boolean (1,574) | Integer (699) | Meaning |
|---|---|---|---|
| `WorldEvent_DescriptionString`, `WorldEvent_ProgressString` | all | all | Texts |
| `WorldEvent_StartActive` | all (true 1,376) | all (true 664) | |
| `WorldEvent_DefaultBoolValue` | 954 | — | Initial value |
| `WorldEvent_DefaultIntValue`, `WorldEvent_MinIntValue`, `WorldEvent_MaxIntValue` | — | 555, 608, 699 | Initial value and range (−10,000 … 1,750,000) |
| `WorldEvent_AllConditionList`, `WorldEvent_AnyConditionList` | 610, 9 | — | Derived value: true when all / any conditions hold |
| `WorldEvent_CountConditionList` | — | 91 | Derived value: the number of conditions that hold (presumably) |
| `WorldEvent_DecayRateMinutes`, `WorldEvent_DecayStartValue`, `WorldEvent_DecayEndValue`, `WorldEvent_DecayStepValue` | 202, 63, 139, — | 55, 47, 30, 55 | Decay over time |
| `WorldEvent_ScheduledEffectList` | 1,022 (always 3 entries, all DID 0 and spellcraft −1) | — | |
| `WorldEvent_EndTimeProperty` | 1,022 (0 in 994) | — | PID of an end time |
| `WorldEvent_BroadcastChannel` | 84 | 32 | Bitfield64 (mapper 0x230002A8) |
| `WorldEvent_CheckpointWithRegistry` | 93 (false) | 4 (false) | |
| `WorldEvent_AutoIncrement_ResetToMinFromMax` | — | 38 | |
| `WorldEvent_MultiEventCode_DriverEventInfo` (`…_NumberOfChoices`, `…_NumberOfDrivenEvents`), `WorldEvent_MultiEventCode_DrivenEventInfo` (`…_DriverWorldEvent`, `…_IndexOfThisEvent`) | — | 14, 53 | One integer event drives 3 … 5 others (choice among 5) |

Both kinds also carry the same examination-tooltip layout fields.

<a id="s14-2"></a>
### 14.2 Conditions (Confirmed)

A `WorldEvent_Condition` struct (in world events, in usage permissions and elsewhere) has
`WorldEvent_WorldEvent` (DID), `WorldEvent_Operator` (operator of §8.2) and
`WorldEvent_ConditionValue` (Int; for a boolean event 0 or 1), and rarely
`WorldEvent_Condition_WorldEventToCompareWith` (compare with another integer event instead
of a constant; 134 in quests). Quest events of type 34 carry the same three fields under
`QuestEvent_WorldEvent*` names.

<a id="s14-3"></a>
### 14.3 Lists (Confirmed)

`WorldEventControl` lists 1,509 world events; the 42 other list records (named by
`PrivateEncounterTemplate_EventControlDID`) list 1 … 414 events for an instance. Each list
has `WorldEvent_Control_DecayEpsilon` and `WorldEvent_Control_ScheduleEpsilon` (300 in all).

> **OPEN:** The units of the epsilons and how decay and schedule are applied; whether the
> per-instance lists scope world events to an instance.

---

<a id="s15"></a>
## 15. Quest-item directories (Confirmed)

`QuestItemRegionDirectory` lists 22 `Quest_QuestItemRegionDirectory_Entry` structs, each a
territory (`Quest_MonsterRegion`) and a quest-item directory (`Quest_QuestItemDirectory`).
A directory (20 of 21 records non-empty) lists `Quest_QuestItemDirectory_Entry` structs: a
quest item (`Quest_ItemDID`), the monsters that drop it (`Agent_Species`,
`Agent_Subspecies`, 0 = any; `Quest_MonsterDivision`) and `Quest_DropFrequency` (mapper
0x23000060; 3, 4, 5, 9, 10, 23).

---

<a id="s16"></a>
## 16. Quest-target index (record `QuestEventTargetLocationControl`)

The single record named by the WEENIECONTENT label `QuestEventTargetLocationControl`
(0x70019558 here, 29,944,338 bytes, 57,284 items) has an empty property set and a root object
with three reference attributes. All of it decodes with `wstate.md` (class 2479 in §5.4,
classes 35 and 138 in §5.3).

<a id="s16-1"></a>
### 16.1 Root attributes (Confirmed)

| Hash | Target | Content |
|---|---|---|
| 0xFD28452 | class 2479 | Location tables (§16.3) |
| 0x8067B38 | class 35, 38,858 entries | **Entity → quests**: key = the DID of an NPC (8,939), monster (16,072), item (11,614 + 262 of other item classes), landmark (1,822), other objects (7) or a missing record (142); value → a class-17 list of quest DIDs (1 … 317; 229,071 in all; 237 missing) |
| 0x1D773D8 | class 138, 9,211 entries | **Role key → quests**: key → a class-225 string (a role key as used in `QuestEvent_RoleConstraint` and `QuestDispenser_RoleConstraint`); value → a class-17 list of quest DIDs (10,492; 68 missing) |

Every list item of both maps is a separate class-17 item (48,069 lists; 9,211 strings).

<a id="s16-2"></a>
### 16.2 Use

- Entity → quests answers "in which quests does this NPC, monster, item or landmark take
  part", for quest markers and for a server routing a world event to the quests that care.
- Role key → quests does the same for objects bound by role key.

<a id="s16-3"></a>
### 16.3 Location tables (class 2479) (Confirmed)

Field layout: `wstate.md` §5.4. Meanings:

1. **Section 1, base locations**: 50,115 DIDs (NPCs 16,516, monsters 17,064, items 9,750,
   landmarks 2,690, and others) → 0 … *p* positions (29,328 DIDs have none; 87,341
   positions). Each position has a u32 that is 0 in all and a byte that is 0 (28,782) or 1
   (58,559). Regions 1 … 6, 14 and 15 occur.
2. **Section 2, classification index**: seven tables, in this order, from a classification
   value to the agents that have it (checked against every agent's own properties: 33,805
   of 33,805 existing agents in tables 0 … 4, 17,265 of 17,265 monsters in table 5):

   | Table | Key | Entries |
   |---|---|---|
   | 0 | genus bit value (`Agent_Genus`) | 17 keys |
   | 1 | species | 105 keys |
   | 2 | subspecies (`Agent_Subspecies`) | 671 keys |
   | 3 | alignment | 3 keys |
   | 4 | class (`Agent_Class`) | 15 keys |
   | 5 | monster division (`Quest_MonsterDivision`) | 517 keys, monsters only |
   | 6 | territory DID | 58 keys, monsters only |

   These are exactly the fields of the monster selector (§5.5).
3. **Section 3, layered locations**: 34,083 DIDs → 395,473 positions that exist only in
   some content layers; each carries its content-layer IDs (1 for 336,868 positions, up to
   5; 581 with none). No position of section 1 recurs here: section 1 holds the positions
   that need no content layer.
4. **Section 4, objective targets**: 20,012 quest DIDs (19,841 present in the install, 171
   not present) → objective index (1 … 32; 49,521 objectives, 49,232 of them under present
   quests and 289 under missing ones) → list of target groups → entries. There are 208,859
   entries in all. An entry is a DID (0 in 32,540), a position, a radius (0 in 195,427;
   otherwise from 1 upward), a role-key string (set in 28,354), a filter list, and a second
   string (set in 44,499).

   The counts below split all 208,859 entries; none is left out:

   | Group | Objectives | Entries | DID 0 | DID ≠ 0 |
   |---|---|---|---|---|
   | Matched objectives of present quests (below) | 48,311 | 200,867 | 28,798 | 172,069 |
   | Other objectives of present quests | 921 | 7,733 | 3,658 | 4,075 |
   | Objectives of the 171 missing quests | 289 | 259 | 84 | 175 |
   | **All** | **49,521** | **208,859** | **32,540** | **176,319** |

   - **Matched objectives.** In 48,311 of the 49,232 objectives of present quests the
     number of target groups equals the number of compiled events of that objective
     (§6.1 objective index), and group *k* belongs to the objective's *k*-th event in
     EventID order (§5.7). Of the 172,069 entries with a non-zero DID in these objectives,
     160,249 name a DID stored in that event's property set (at any depth). The other
     11,820 are all in item events or talk events:
     - 9,968 (types 31, 7 and 10) name a DID listed as `Quest_LootMonsterDID` in the
       quest's own `Quest_LootTable` (§9.7), that is a monster that drops one of the
       quest's loot items; 16 of these DIDs have no record in the install;
     - 1,842 (types 31, 7 and 10) name a landmark (class 1210 here);
     - 4 (type 31) name a DID with no record in the install;
     - 6 (type 11) name an NPC or object other than the one the event names.

     The 28,798 DID-0 entries of these objectives are counted separately and take no part
     in this comparison.
   - **Other objectives.** The remaining 921 objectives of present quests have a different
     number of groups: 919 fewer groups than events (for example one group for two
     events, 556 objectives), 2 more groups than events. Their grouping is OPEN (below).
   - **Role-key string.** Full census: each of the 28,282 role-key strings in entries of
     present quests equals a role key stored somewhere in that quest's property set, as
     the value of `QuestEvent_RoleConstraint`, `QuestDispenser_RoleConstraint`,
     `QuestEvent_ChannelingRoleConstraint` or `QuestEvent_Location_RoleConstraint`. The
     other 72 belong to entries of the 171 missing quests and cannot be checked.
   - **Second string.** Of the 44,499 set, 44,495 are in entries of present quests; 21 of
     those equal a role key of the quest (as above), 44,474 do not. The other 4 are under
     missing quests.
   - A **filter** is a flag byte with one u32 per set bit, bits in ascending order: 0x01
     genus, 0x02 species, 0x04 subspecies, 0x08 alignment, 0x10 class, 0x20 monster
     division, 0x40 territory DID (all 5,345 values are territories). Bit 0x80 is never set.
   - Event types with entries: 1, 2, 4, 5, 7, 9, 10, 11, 13, 16, 19, 21, 22, 24, 25, 27,
     29, 31, 34 (4 of 2,943), 38, 39, 56, 57. Types 6, 14, 18, 26, 30, 32, 33, 37, 45, 58
     and 59 have none.

A quest map shows, for the player's current objective, the section-4 entries of each of
its events: the entry position, and for an entry with a DID, the positions of that DID in
sections 1 and 3 (filtered by the active content layers).

> **OPEN:** The meaning of the byte (0/1) on section-1 and section-3 positions, of the
> u32 that is always 0, and of the second string of a section-4 entry (LC: a key for
> "point done" bookkeeping). How the 921 objectives whose target groups do not match the
> event count are grouped (919 with fewer groups than events, for example three monster
> events with one group; 2 with more groups than events). What bit 0x80 of a filter would carry (the client reads a u32 for it, BIN per
> `wstate.md` §5.4; the landmark field of the selector is the remaining candidate).

---

<a id="s17"></a>
## 17. Quest map notes (record 0x0E400000)

<a id="s17-1"></a>
### 17.1 Layout (Confirmed, 5,638 notes, exact to the last byte)

Record 0x0E400000 of `client_general.dat` (1,037,249 bytes). All integers little-endian.

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | did | u32 | 0x0E400000 |
| 2 | count *n* | u32 | Number of notes (5,638) |
| 3 | notes | note × *n* | |

Note:

| Order | Field | Type | Meaning |
|---|---|---|---|
| 1 | position | Position (`property-system.md` §4.6) | Where the note sits. Flags 0x1B, 0x3B, 0x5B or 0x7B in all: region, landblock, cell and offset always present, so every note is inside a cell (interior) |
| 2 | area | u32 DID | Area (354) or dungeon (36) record, or 0 (5,248) |
| 3 | dungeon | u32 DID | The dungeon whose map shows the note (all 5,638 are dungeon records) |
| 4 | target | u32 DID | The noted object: NPC 2,936, object kind 1170 1,718, item 321, monster 317, landmark 301, other 45 |
| 5 | layers | property, list form | A u32 PID followed by its value (`property-system.md` §5.2 list form). In all notes the PID is that of `Entity_ContentLayers` (an array of `Entity_ContentLayer` enums from DID map 0x28000012): 0 elements in 3,099 notes, 1 in 2,520, 2, 4 or 5 in the rest. A PID of 0 would mean no value (4 bytes). |
| 6 | info | embedded property set (TSize + table entries, `property-system.md` §5.1) | 1 or 2 properties: `QuestDispenser_RoleConstraint` (3,363 notes) and/or `QuestDispenser_ActiveQuests` (2,453 notes; 16,124 `QuestDispenser_QuestRole` structs of quest DID and role name) |

The role names in a note are role names of the quest's `Quest_RoleArray` (§7.3).

> **OPEN:** (implementation census, 2026-10-07) 3 of the 16,124 `QuestDispenser_QuestRole`
> structs in note infos have no `QuestDispenser_RoleName` (a quest DID only); the other
> 16,121 name a role of a present quest's `Quest_RoleArray`. What does a note role without a
> role name stand for (every role of the quest, or none)?

<a id="s17-2"></a>
### 17.2 Use

A dungeon map shows the notes whose dungeon is the map's dungeon and whose content layers
are active; a note is relevant to the player when one of its quest roles is the current
step of one of the player's quests (bestow roles for offers, talk roles for objectives).

> **OPEN:** How a note whose info has only a role key (3,185 notes without
> `QuestDispenser_ActiveQuests`) is tied to quests: through the role-key index of §16.1 is
> the likely route.

---

<a id="s18"></a>
## 18. Client behaviour and where it lives (BIN)

- §4.3 is the only quest computation of this spec that was found in the executable.
- The executable names by immediate PID, among the properties of this family: `Quest_Name`
  (35 places), `QuestDispenser_RoleSuccessText` (18), `Quest_IsAccomplishment` (14),
  `Title_String` (13), `Accomplishment_UITab` (12), `Accomplishment_Category` (5),
  `Emote_CommandString` (4), `WorldEvent_WorldPropertyName` (4), `Usage_QuestStatus` (7),
  `Quest_ChallengeLevel`, `Quest_ChallengeLevelOverrideProperty`, `Quest_HideChallengeLevel`,
  `Quest_QuestTreasureDID`, `QuestTreasure_FixedItemArray`, `QuestTreasure_SelectableItemArray`,
  `Quest_Category`, `Quest_NextQuest`, `QuestDispenser_ActiveQuests`, `QuestEvent_ConditionID`,
  `Scene_KeyframeArray`, `QuestDispenser_RoleName`, `Usage_QuestRequirements`,
  `Usage_QuestRequirements_OrList`, `Usage_QuestID` (1 – 7 places each).
- It names **none** of: `QuestEvent_ID`, `QuestEvent_EventID`, `QuestEvent_ObjectiveID`,
  `QuestEvent_QuestDID`, `Quest_ObjectiveArray`, `Quest_ObjectiveDescription`,
  `QuestEvent_ProgressOverride`, `QuestEvent_NPCTalk`, `QuestEvent_Locations`,
  `Quest_RoleArray`, `Quest_BestowalRoles`, `QuestDispenser_NPC`, `QuestDispenser_TextArray`,
  `QuestDispenser_Species`, `QuestDispenser_VOSoundID`, `Usage_Operator`, the `QuestReward_*`
  lists and tiers, `QuestArc_*`, `NPCComment_*`, `NPC_ColorTextArray`, `NPC_ColorTextFile`,
  `Keyframe_Subtitle`.
- The quest-requirement loader reads both requirement lists only to collect the quest DIDs
  they mention (so that it can re-check when those quests change); the comparison itself is
  not in the executable.

Consequence for implementers: event matching, requirement evaluation, species variants and
reward amounts are behaviour of the class library's script code or of the server. This spec
gives the data those rules run on; the rules are OPEN where stated.

> **OPEN:** The class library's script methods of the quest classes (`wlib.md`) were not
> analysed. They may settle §5.2, §7.1, §8.2 and §9.2 step 2.

---

<a id="s19"></a>
## 19. Invariants

Checked on this install (all records). A reader may warn on a violation but must not fail.

| # | Invariant |
|---|---|
| V1 | Every quest record has `Quest_Name`, `Quest_ObjectiveArray` (1 … 32 objectives) and `Quest_RoleArray`; `Quest_IsAccomplishment`, when stored, is true. |
| V2 | The `Quest_ObjectiveIndex` values of a record are 1 … *n* without repeats. |
| V3 | The number of compiled events (§6.1 event table) equals the number of event entries of §5.1 for every quest; EventIDs are 1 … *N*. |
| V4 | Numbering per §5.7 reproduces `QuestEvent_EventID`, `QuestEvent_ObjectiveID`, `QuestEvent_ConditionID`, `QuestEvent_ID` and `QuestEvent_IsFailureEvent` of every compiled event. |
| V5 | The sibling list of a compiled event equals, in ascending order, the EventIDs of the other members (§5.2) of its compound event, and is empty for an entry outside a compound. |
| V6 | In `Quest_RoleArray`, action 6 ⇔ `Quest_ObjectiveIndex` stored as 0, and action 5 ⇔ `Quest_ObjectiveIndex` absent. The check treats an absent value as distinct from 0 (the property has no dictionary default, §7.1); with absent read as 0 the invariant fails for the 959 action-5 roles. |
| V7 | `QuestControl_RewardLevelDIDArray` has one distinct element per reward-level record; its tier-4 experience amounts never decrease with the index. |
| V8 | Every StringInfo of the family is a table reference without arguments. |
| V9 | Every quest-treasure item struct has `QuestTreasure_ItemQuantity`. |
| V10 | Every map note decodes per §17.1 and the record ends exactly after note *n*. |
| V11 | In section 2 of §16.3, every listed agent that exists has the key's value in the field of that table. |
| V12 | Every arc's quests are distinct across all arcs. |
| V13 | NPC comment records have the fixed requirement shape of §7.7. |
| V14 | Every scene's class-427 native has one entry per keyframe and repeats subtitle and flag (§7.9). |

> **OPEN:** (implementer choices where the spec is silent, `crates/formats/src/gameplay/quests.rs`;
> to be confirmed or replaced) (1) An objective without `Quest_ObjectiveIndex` makes the
> strict §5.7 numbering of its quest fail (`MissingProperty`) rather than guessing an
> ObjectiveID; a lenient variant leaves such an objective out of the walk (its entries take
> no EventID, so later EventIDs shift) and reports its stored position, so that the rest of
> the quest is still numbered. Which ObjectiveID, if any, the compiled form would give such
> entries is not stated. (2)
> Enum-typed fields (tiers, actions, operators, statuses) are also accepted when the
> dictionary types them as a non-negative Int. (3) §9.2 step 2 gives no table for a level
> below 0 or above the length of `QuestControl_RewardLevelDIDArray`, and for a null element.
> (4) The listen list of types 19 and 29 (§6.2) is not derived (the choice of 1 or 11 is
> OPEN). (5) The map-note decoder allocates for at most one note per 19 bytes of payload
> (position flag byte, three DIDs, a null layers PID, an empty info set). (6) In
> `Quest_FailureConditionArray` and `Quest_TrackedConditionsArray` only elements with the
> PID of `Quest_FailureCondition` or `Quest_TrackedCondition` (§5.1) are conditions; any other
> element is skipped and takes no ConditionID, as in an objective's condition array.

---

<a id="s20"></a>
## 20. Worked examples (synthetic)

<a id="s20-1"></a>
### 20.1 Event numbering

A synthetic quest stores its objectives in the order index 2, then index 1:

- objective 2: completion condition A = one compound event (one direct entry of type 11,
  then a `QuestEvent_Entry_Array` with one entry of type 10); failure condition B = one
  entry of type 30;
- objective 1: completion condition C = one entry of type 22;
- `Quest_FailureConditionArray` with one condition D = one entry of type 32;
- no tracked conditions.

Numbering (§5.7): EventID 1 = A's direct entry (ObjectiveID 2, ConditionID 1, type 11); 2 = A's
second entry (2, 1, type 10); 3 = B (2, 2, type 30, failure); 4 = C (1, 1, type 22); 5 = D
(0, 1, type 32, failure). Sibling lists: event 1 → (2), event 2 → (1), others empty.
Listen lists: event 1 → (11), or (11, 17, 12) if it waits for a drama it names; event 2 →
(10, 31, 7); event 3 → (30); event 4 → (22); event 5 → (32). Objective index of §6.1: key 2
→ events 1, 2, 3; key 1 → event 4; key 0 → event 5. Order values: 1 and 2 take their
`QuestEvent_EventOrder`; 3 and 5 are −1.

A second synthetic quest has one objective (index 1) and stores, in this order:

- completion condition A: a direct entry of type 21, then a compound event X whose
  elements are a direct entry of type 22, a second direct entry of type 22 and a
  `QuestEvent_CompoundProgressOverride`;
- completion condition B: holds only `Quest_Condition_NeverFinish` (no entry);
- completion condition C: a compound event Y whose elements are a direct entry of type 11,
  a `QuestEvent_Entry_Array` with one entry of type 7, and an empty
  `QuestEvent_Entry_Array`.

Numbering (§5.7): EventID 1 = A's direct entry (ObjectiveID 1, ConditionID 1, type 21);
2 = X's first direct entry (1, 1, type 22); 3 = X's second direct entry (1, 1, type 22);
B takes ConditionID 2 and contributes no event; 4 = Y's direct entry (1, 3, type 11);
5 = the entry of Y's first array (1, 3, type 7); Y's empty array and X's progress override
contribute nothing. Members (§5.2): X = {2, 3}, Y = {4, 5}. Sibling lists: event 1 → ()
(not in a compound), event 2 → (3), event 3 → (2), event 4 → (5), event 5 → (4). Listen
lists: (21), (22), (22), (11), (7). Objective index of §6.1: key 1 → events 1 … 5.

<a id="s20-2"></a>
### 20.2 Reward lookup

A synthetic quest has `Quest_ChallengeLevel` 12, no `Quest_RewardLevelDID`, `Quest_ExpTier`
4. A synthetic `QuestControl_RewardLevelDIDArray` has element 12 = record R, and R's
`QuestReward_ExpEntryList` holds (tier 4, amount 900). The table is R (§9.2 step 2, Reported)
and the experience reward is 900. With `Quest_RewardLevelDID` = S, S is used regardless of
the level. With `Quest_ExpTier` 2,000 and no element of tier 2,000, no experience is given.

<a id="s20-3"></a>
### 20.3 Status codes

"Completed" is 0x30000000. "Underway, at objective 3" is 0x10000003. A synthetic
requirement (operator 3, quest Q, status 0x30000000) asks for Q completed. The objective
number of a code is code AND 0x0FFFFFFF; its raw status is code AND 0xF0000000.

<a id="s20-4"></a>
### 20.4 A map note (bytes)

A synthetic note, after the record header (`00 00 40 0E`, then the count):

```
1B 02 10 20 05 00 00 00 20 41 00 00 A0 41 00 00 00 00   position: flags 0x1B, region 2,
                                                        landblock (0x10, 0x20), cell 5,
                                                        offset (10.0, 20.0, 0.0)
00 00 00 00                                             area: none
34 12 00 70                                             dungeon 0x70001234
78 56 00 70                                             target 0x70005678
CE 04 00 00 00 00 00 00                                 layers: PID 0x4CE, 0 elements
00 01                                                   info: TSize hint 0, 1 entry
70 12 00 10 70 12 00 10 03 6B 65 79                     key = PID 0x10001270, PID echo,
                                                        string of length 3
```

The PIDs in this example are those of this install's dictionary and are shown only to
make the bytes concrete; a reader takes the PID from the data and its type from the
dictionary.

---

<a id="s21"></a>
## 21. Open questions (summary)

| Section | Question |
|---|---|
| §2.1 | Further family kinds without the listed identifying properties |
| §4.2 | Top-level `Usage_*` properties outside the permission struct |
| §4.3 | Effect of `Quest_IgnoreDefaultChallengeLevel` |
| §5.2 | Completion rule for several completion conditions without a count |
| §5.5 | Landmark field of a monster selector |
| §6.1 | Root attribute 0x16D4503; derivation of the compiled permission set |
| §6.2 | Choice of 1 or 11 in the listen lists of types 19 and 29; DID attributes of type-22 events |
| §7.1 | Objective limits of action-5 roles; choice of species variant lines |
| §7.3 | Derivation of the flattened role list in 3,560 records |
| §7.7, §7.8 | Choice among NPC comments; bark selection and timing units |
| §7.9 | Second float of a scene entry |
| §8.2 | Status comparison rule |
| §9.1 | Deed reward lists; money unit; paper-item float |
| §9.2 | Default table for override levels and levels above 160 |
| §9.6 | Medal 1 (no threshold entry) |
| §9.8 | Units and role of several quest-control values; category limiter |
| §10.2, §10.3 | Compiled deed-directory objects; version list use |
| §14.3 | World-event decay and schedule epsilons; per-instance lists |
| §16.3 | Location byte and word; second string; unmatched objectives; filter bit 0x80 |
| §17.2 | Notes with only a role key |
| §18 | Script methods of the quest classes |

---

<a id="s22"></a>
## 22. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). |
| 2026-10-07 | Repair round after review (status stays Review). (1) §5.1, §5.2, §5.7 step 1.2, §6.2, V5, §20.1: a compound event holds 1 … 6 direct entries and 0 … 2 entry arrays (composition table added; 1,012 compounds have 2 … 6 direct entries and no array, and 4 more have three direct entries and one array, together the review's 1,016); the walk takes every direct entry once and every entry array in stored order; "member of a compound" defined by that walk; sibling lists are in ascending EventID order; empty conditions still take a ConditionID; condition shapes counted; second worked example with two direct entries. (2) §7.1, §7.2, V6: `Quest_ObjectiveIndex` has no dictionary default; per-container presence table; action 5 has it absent, which must stay distinct from 0; action-5 roles are assigned to no objective (new OPEN on server-side limits). (3) §5.2: count key written in words. (4) §16.3 section 4: full split of the 208,859 entries (matched, unmatched, missing quests; DID 0 and non-zero); the earlier 189,047 counted DID-0 entries as matches and is replaced by 160,249 of 172,069 non-zero DIDs; the 11,820 others are loot monsters, landmarks, missing records and 6 talk targets, not selector agents as stated before; role-key strings are a full census (28,282 present-quest strings all match; 72 under missing quests); unmatched objectives are 919 with fewer and 2 with more groups. (5) §21: §9.6 lists medal 1 only. (6) §6.3: the two doubles are bit pattern 0xC7EFFFFFE0000000 (most negative finite 32-bit float widened), not a 64-bit minimum; all fifteen attributes of the effect-application objects tabulated; provenance row C3 added; rows O1, O4, C2, R1, X1 updated. |
