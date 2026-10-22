<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# NPC dramas: drama lists, named dramas, actions, triggers and scripted fights

| | |
|---|---|
| **Status** | Accepted (2026-10-09, re-review of the same-day revision; first Accepted 2026-10-09, then Revised 2026-10-09 to answer the two implementer OPENs: §7 full labels and new §7.1, §11 item 4 preview rules as Decision T7 items) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (dirty room, a different session from the writer) |
| **Review record** | 2026-10-09, spec reviewer (fresh session): **pass**, §5.3 checklist met, 7 groups of facts spot-checked independently (dictionary, enum mappers, 0x07000B0A, all 0x79 sets, BIN byte checks, §3.5 layout, internal sums), all match; non-blocking notes in the review log. **Re-review 2026-10-09** (spec reviewer, independent session, not the writer) of the revision (§7, §7.1, §11 item 4, provenance D27, change log): **pass**, §5.3 checklist met. Spot check, read-only, of the property dictionary 0x34000000: all 17 labels of §7.1 and their PIDs, types, defaults and child lists match; the dictionary has exactly these 17 labels containing `KungFu` and no `AI_KungFu_HealthRatio`; the three state switches are children of `Drama_NamedDrama`. The preview rules are Tessera Decisions and touch no closed gate. Non-blocking: §13.4 still writes the short forms `Transition_Drama` and `HealthRatio` (§7.1 resolves them); row D27 stands before D26 in the provenance table. |
| **Implements into** | A gameplay-data layer on `crates/props` (typed view of the drama property tree, §3–§5, and its census commands); `apps/client` and the offline walker (ambient playback of idle and enter-world dramas, §10.2); `apps/explorer` (drama viewer per entity, §11); `crates/net` sandbox backend and any later server (drama runner and triggers, §6, §7, §10.3) |
| **Companion specs** | [`property-system.md`](property-system.md) §4 (value encodings: struct §4.10, array §4.11, StringInfo §4.8, bitfields §4.4 and §7.3), §5 (property sets), §6.2 (the +0x09000000 rule), §7.1 (enum labels). [`entity-descriptor.md`](entity-descriptor.md) §4.5.2 (per-instance property sets, lookup precedence E26). [`landblock.md`](landblock.md) §4.6 (links and link types), §4.8 (the weenie list). [`creatures.md`](creatures.md) §2 (templates, classes 1723 and 1724), §6.2–§6.3 (spawned-object instance sets, Generator and Patrol links), §6.9 (patrol paths). [`quests.md`](quests.md) §5.3–§5.4 (event fields and types), §6.2 (listen lists), §7.8 (barks). [`instance-scripts.md`](instance-scripts.md) §3 (`Entity_InstanceScriptTable`), §3.5 (link targets), §4 (how instance scripts run). [`script-table.md`](script-table.md) §5.2 (Run node, script keys of enum 0x23000097), §8.6 (starting idle). [`script-runtime.md`](script-runtime.md) §5 (driven properties), §12 (script starts), §13 (WhenRunning). [`interaction.md`](interaction.md) §3.4.4–§3.4.5 (`Drama_Performing` in mouse-over rules). [`sound.md`](sound.md) §4–§6 (sound infos, channels). [`combat-data.md`](combat-data.md) (effect records, root class 698). [`chat-window.md`](chat-window.md) (chat types). [`string-table.md`](string-table.md) §9 (resolving a StringInfo). |

---

<a id="purpose"></a>
## Purpose

A **drama** is a named, scripted sequence that an NPC or creature performs: saying lines in
chat or over its head, playing emotes and sounds, waiting, starting or changing a patrol,
signalling other objects, directing other actors, applying effects, turning hostile or
friendly (a scripted fight), or removing itself. Dramas carry the ambient life of towns and
camps and the scenes that quests start.

This spec lets an implementer:

1. read every drama list in the install: where lists are stored and which one applies to an
   entity (§2), the property tree of a list, a named drama and an action (§3), and the meaning
   and value census of every action facet (§4) and drama-level flag (§5);
2. find **what starts** a named drama: quest events, script-link signals, conventional signal
   names, broadcasts, directors, scripted-fight transitions, starting idles and property
   responses (§6);
3. read the **scripted-fight** ("KungFu") data: the NPC-to-monster transitions and their
   health thresholds (§7), and the starting-idle properties that dramas interrupt (§8);
4. know what the **client itself** does with dramas, as far as the client's class library,
   its shared script tables and its executable show (§9);
5. play dramas back in Tessera (offline ambient playback, a server-side runner) under the
   Tessera Decisions of §10, and show them in the explorer (§11).

The original server runs dramas; the data hold the drama definitions but not the runner. Every
behaviour that only the server shows is marked as a hypothesis or OPEN, and the rules Tessera
uses in its place are marked **Decision**.

Everything here is read at run time from the user's install. Property IDs (PIDs), enum keys,
class indices, DIDs and counts are **observations** of one install (§1); code finds properties
by their dictionary label (`property-system.md` §2), enum keys by their mapper label, and
records by the rules given, and must not hard-code the observed values.

---

<a id="s1"></a>
## 1. Provenance

How sources are marked:

- **Confirmed**: observed in the user's install with throwaway probes in the dirty-room scratch
  area, over **every** record or value of the kind named (not a sample), on the date given.
- **WLB**: read from the class library's bytecode (record 0x56000000 in
  `client_gamelogic.dat`), disassembled in the dirty room with the rules of
  [`wlib-bytecode.md`](wlib-bytecode.md) (an Accepted spec). This is data of the install
  (method M1), not the executable; functions are named by owning class and virtual slot of this
  install; behaviour is restated in words.
- **BIN**: read from the client executable under method M4 (decision 0011).
- **Inferred**: a conclusion of this spec writer from Confirmed evidence, used only as a
  qualifier inside a row for the part it names.
- **Decision**: a Tessera choice where neither the data nor the client settle the matter; it is
  not a claim about the original.

| Key | Source |
|---|---|
| OBS | User install (Steam), read in place and read-only on 2026-10-08 and 2026-10-09; data files dated 2026-10-04, unchanged. Throwaway probes in the dirty-room scratch area used the project's own decoders as tools (read-only) and Python over their output. Scope: the property dictionary (0x34000000; all 27,546 definitions, every definition whose label contains `Drama`, `KungFu`, `StartingIdle`, `ReturnsToStartingIdle` or `PropertyResponseCallback` printed with type, flags, default and child list); all 49,636 landblock-info records of regions 1–5 and 14 (`client_cell_15.dat` holds no landblock-info record) with every embedded set: 3,253,862 static-entity sets, 3,768,840 weenie-list instance sets, 1,324,469 link sets with their link names and ends, 49,636 landblock sets, 104,271 cell sets; all 384,948 property-set records 0x78 and 0x79 of `client_gamelogic.dat` (the owning 0x70 root class read for every 0x79); enum mappers 0x23000097 (with base 0x230000F8), 0x23000110, 0x23000113, 0x23000114, 0x23000157, 0x23000158, 0x2300005C, 0x230002CF, 0x2300034D (with base 0x2300034E), 0x23000017, 0x2300004F, 0x23000291, 0x230002A8 (with base 0x230002A9); all 7,524 script tables (prefix 0x07) for their script keys; the script tables 0x07000001, 0x07000097 and 0x07000B0A and every record that names them; every DID that a drama value names (effect records, scriptlets, sound infos) and every (table, token) pair of a drama StringInfo, looked up in `client_local_English.dat`. |
| WLB | The class library (record 0x56000000, chunk type 1), read 2026-10-09: the bodies of classes 1039 and 1742 (slots 1–5, which share bodies), class 1270 slot 2, class 287 slot 31, class 279 slot 1, and a search of the whole code block for 32-bit pushes (`push.imm32`, type 2) of every PID named in this spec, each hit attributed to its owning body and class. Immediates below 0x10001000 coincide with enum keys of other mappers and with user-interface identifiers; a hit was used only when the body passes the value to a property read: here the slot-33 function shared by class 89 and its descendants, which takes an entity, a PID and a default, stores the default in a local, lets the entity's virtual slot 28 fill the local from the property and returns it; all 272 of its call sites that push an immediate PID push a Boolean property. |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, file dated 2026-10-04, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-09 in the dirty room with a command-line disassembler on the read-only executable (no analysis database), by address range only. **Questions:** (B1) does native client code read any drama action or dialog property (the dialog type and text list, the emote, the waits, the action sequence), i.e. is any part of drama playback implemented in the executable; (B2) does the executable carry any drama-related identifier string. **Why M1–M3 were not enough:** the data and the class library show which scripts the client runs, but not whether native code reads drama properties directly; no published source covers dramas (a web search on the property labels found nothing); the running client (M3) shows the result of a drama, not which side computes it. **Read:** a scan of the code section for every move of a 32-bit immediate into a register where the immediate equals a dictionary PID of 0x10000100 or more (12,789 sites), with the call that follows each one, and the instruction context of every site whose immediate is a drama PID (they pass the value as an element ID to user-interface lookups or compare it with the ID field of a user-interface event); the first instructions of the called functions, enough to tell typed property getters (they call the property-table lookup and test the returned type code) from user-interface lookups (they walk a child list comparing an element ID, or instantiate a layout by key); a byte search of the whole file for the drama PIDs and for the text `drama` (8-bit and 16-bit). Nothing from network, login, launcher or anti-tamper code was read. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| D1 | The 77 dictionary definitions labelled `Drama_*`: type, enum mapper, flags, defaults, child lists (§3.1) | OBS (dictionary) | Confirmed 2026-10-08 |
| D2 | Where drama lists are stored, per context and class; the 101 static-entity copies equal their instance sets; the spawned-object link pattern (§2) | OBS (all landblock sets, all 0x79 sets) | Confirmed 2026-10-08 |
| D3 | Tree of a drama list: three element kinds and their fields; sequence, action and name statistics (§3.2–§3.5) | OBS (all 27,804 lists) | Confirmed 2026-10-08 |
| D4 | Action facets: field set, per-field counts, combinations (§3.4, §4.9) | OBS (all 100,382 named-drama sequence actions, 535 idle actions, 3,724 instant actions) | Confirmed 2026-10-08 |
| D5 | Value census of every action field (§4) | OBS (every value) | Confirmed 2026-10-08 |
| D6 | Dialog type, say-channel, sound-channel, detector-slot, actor and script-link mapper labels (§4.2, §4.4, §4.6) | OBS (mappers named in OBS) | Confirmed 2026-10-08 |
| D7 | 57,415 of the 57,455 drama StringInfos resolve; 35 are (0, 0); 5 name a token missing from an existing table; none is a literal (§4.2) | OBS (every StringInfo, every lookup) | Confirmed 2026-10-08 |
| D8 | Emote fields hold script keys (enum 0x23000097); 1,104 of the 1,138 distinct non-zero keys are defined by at least one script table (§4.3) | OBS (all 7,524 tables) | Confirmed 2026-10-08 |
| D9 | Patrol names in drama actions equal the `LinkName` of a patrol link that starts at the same entity (§4.5) | OBS (every value; links of the same record) | Confirmed 2026-10-08 |
| D10 | Script-link actions: the port number comes from the mapper **label**, not the key; the entity's instance script table defines the matching output script (§4.6.1) | OBS (16,241 actions; instance script tables; link names) | Confirmed 2026-10-09 |
| D11 | Broadcast names resolve to dramas of other entities of the same landblock; the property-to-match census; `AI_AcceptsBroadcasts` on templates (§4.6.2) | OBS (5,095 broadcasts; every 0x79 set) | Confirmed 2026-10-09 |
| D12 | Detector-slot actions match the slot names on the entity's AIRemoteDetector links (§4.6.3) | OBS (1,009 actions) | Confirmed 2026-10-08 |
| D13 | Director actions: actor bit *i* ↔ mapper key *i* + 1 ↔ label `Actor<NN>` ↔ the Script link named for cast slot `<NN>`; actors' drama and patrol names resolve on the actor (§4.6.4) | OBS (10,330 actions) | Confirmed 2026-10-09 |
| D14 | Effect, scriptlet and sound references all exist; effects all descend from the effect root class (§4.7, §4.4) | OBS (476 effect DIDs, 104 scriptlets, 5,220 sound infos) | Confirmed 2026-10-08 |
| D15 | AI-hint fields: two any-width bitfields of mapper 0x2300005C; set ⊆ mask in 674 of 679 (§4.8) | OBS | Confirmed 2026-10-08 |
| D16 | Named-drama flags and their values; idle and instant kinds (§5) | OBS | Confirmed 2026-10-08 |
| D17 | Trigger census: which references name each drama (§6.1) | OBS | Confirmed 2026-10-09 |
| D18 | Quest events name dramas held by the entity whose role key equals the event's role key (§6.2) | OBS (all 5,314 drama-naming event entries) | Confirmed 2026-10-08 |
| D19 | Incoming script-link input `Custom<N>` ↔ a drama named `ScriptLink_Custom<N>` on the receiving entity (§6.3) | OBS (17,331 inputs) | Confirmed 2026-10-09; that the input **runs** that drama is Inferred |
| D20 | Conventional signal names and their case variants; names are matched without regard to letter case (§6.4) | OBS | Name census Confirmed 2026-10-08; case-insensitive matching Inferred |
| D21 | Property-response chain to drama callbacks (§6.7) | OBS (17 maps, 40 callbacks) | Confirmed 2026-10-09 |
| D22 | Scripted-fight transitions, their health ratios and flags; transition names resolve in the entity's own list (§7) | OBS | Confirmed 2026-10-08 |
| D23 | Starting-idle properties and their relation to the idle-stop action (§8) | OBS | Confirmed 2026-10-08 |
| D24 | The shared NPC script table 0x07000B0A defines the client-side drama scripts (emote, scriptlet, sound, sanity marker); it reaches every mob and NPC template through 0x07000097 and 0x07000001 (§9.1) | OBS (all 0x07 tables, all 0x78/0x79 sets) | Confirmed 2026-10-09 |
| D25 | The class library reads `Drama_Performing` and `Drama_CanInteractDuringDrama` only (functions of classes 1039/1742), uses them in the quest-interaction check and the mouse-over rules, and starts the drama sound script for barks (§9.2–§9.4) | WLB | Confirmed (bytecode) 2026-10-09 |
| D27 | Full labels, PIDs, types and defaults of the scripted-fight transition fields and of the drama state switches (§7.1) | OBS (property dictionary 0x34000000, re-read 2026-10-09) | Confirmed 2026-10-09 |
| D26 | The executable reads no drama action or dialog property and contains no `drama` text; every immediate equal to an action-property PID is a user-interface identifier; it reads `AI_KungFu_IsNPC` through the typed property getters (§9.5) | BIN (B1, B2) | BIN |
| T1–T7 | Tessera's playback timing, dialog presentation, name matching, list precedence, offline scope, server runner and explorer viewer (§10, §11) | — | Decision |

---

<a id="s2"></a>
## 2. Where drama data live

<a id="s2-1"></a>
### 2.1 Contexts (Confirmed D2)

The root property is `Drama_DramaList` (an array, §3.2). It occurs in these property sets:

| Context | Sets with a list | Notes |
|---|---|---|
| Landblock weenie list, instance sets without a static entity (`landblock.md` §4.8, `creatures.md` §6.2) | 27,169 | Spawned objects: 27,166 of them are the source of a Generator link (type 8) |
| Landblock weenie list, instance sets of a static entity (`entity-descriptor.md` §4.5.2) | 101 | Static entity types: Physics 98, Weenie 2, GeneratorPoint 1 |
| The same 101 static entities' own property sets | 101 | Each list is **identical** (whole value) to the one in the entity's instance set |
| 0x79 sets of 0x70 records | 433 | Root class 1723 (mob templates, `WeenieType` 65615) 384; 1724 (NPC templates, 131151) 41; 796 (items, 129) 4; 5785 (a template class below the same root as 1723 and 1724, 131151) 4 |

No 0x78 set, link set, landblock set or cell set holds a drama property. In all, 73 of the 77
`Drama_*` labels are stored (727,589 stored values with nesting); `Drama_Comment`,
`Drama_ActionComment`, `Drama_Performing` and `Drama_Target` are defined but never stored.
`Drama_Performing` is a run-time state (§9.2).

Outside a list, two class-1723 templates hold a stray top-level `Drama_Action` (with a
`Drama_Emote`); nothing else names it. Readers keep it and ignore it.

<a id="s2-2"></a>
### 2.2 Which list applies to an entity

- **Static entity.** The entity-descriptor lookup applies (`entity-descriptor.md` §4.5.2, E26):
  the instance set replaces the static entity's own set. In this install the two lists are equal
  wherever both exist, so the result does not depend on the rule.
- **Spawned object.** Its instance set carries no template DID (`creatures.md` §6.2), so the
  template whose list it might override cannot be found from the data. The 433 template lists
  apply to objects created from those templates.
- **Decision T4 (list precedence):** an array property is one value: the effective
  `Drama_DramaList` of an entity is the first one found in the entity's lookup order (instance
  set, then template, then the template's parents), and lists are **never merged**. An empty
  list (325 lists: 303 instance sets, 22 static-entity sets) is a valid value that hides any list
  further down the lookup order.

---

<a id="s3"></a>
## 3. The property tree

All values use the encodings of `property-system.md` §4; a struct is a table-form set
(§4.10), an array a list-form set whose elements each carry their own PID (§4.11). PIDs below are
observations, given to make examples checkable; code looks them up by label.

<a id="s3-1"></a>
### 3.1 Dictionary definitions (Confirmed D1)

| Label | PID | Type | Enum mapper / DB type | Flags, default | Children |
|---|---|---|---|---|---|
| `Drama_DramaList` | 0x10000858 | Array | — | — | `Drama_NamedDrama_Idle`, `Drama_NamedDrama_Instant`, `Drama_NamedDrama` |
| `Drama_NamedDrama` | 0x10000CBF | Struct | — | — | `Drama_Comment`, `Drama_ActionSequence`, `Drama_KungFu_BecomeNPC_DoNotEndCombat`, `Drama_Interruptible`, `Drama_KungFu_BecomeNPC`, `Drama_Name`, `Drama_KungFu_BecomeMonster`, `Drama_WaitForDrama` |
| `Drama_NamedDrama_Instant` | 0x100001F2 | Struct | — | — | `Drama_Comment`, `Drama_Action_Instant`, `Drama_Name` |
| `Drama_NamedDrama_Idle` | 0x100001F1 | Struct | — | — | `Drama_Comment`, `Drama_Name`, `Drama_ActionSequence_Idle` |
| `Drama_Name` | 0x100003C6 | String | — | flag 0x1 | — |
| `Drama_ActionSequence` | 0x10000025 | Array | — | flag 0x1 | `Drama_Action` |
| `Drama_ActionSequence_Idle` | 0x1000108A | Array | — | flag 0x1 | `Drama_Action_Idle` |
| `Drama_Action` | 0x1000069D | Struct | — | — | 29 fields (§3.4) |
| `Drama_Action_Instant` | 0x100004C1 | Struct | — | flag 0x1 | 19 fields (§3.4) |
| `Drama_Action_Idle` | 0x100005A9 | Struct | — | — | 8 fields (§3.4) |
| `Drama_Interruptible` | 0x10000027 | Bool | — | default false | — |
| `Drama_WaitForDrama` | 0x10000EC0 | Float | — | — | — |
| `Drama_KungFu_BecomeMonster` | 0x10000EBF | Bool | — | default false | — |
| `Drama_KungFu_BecomeNPC` | 0x10001464 | Bool | — | default false | — |
| `Drama_KungFu_BecomeNPC_DoNotEndCombat` | 0x100027AE | Bool | — | default **true** | — |
| `Drama_Comment`, `Drama_ActionComment` | 0x10001371, 0x10002B19 | String | — | flag 0x200 | — (never stored) |
| `Drama_Wait_Before`, `Drama_Wait_After`, `Drama_Wait_Variance` | 0x1000085A, 0x100004C4, 0x1000136F | Float | — | default 0.0 | — |
| `Drama_Wait_Emote` | 0x10001463 | Bool | — | default false | — |
| `Drama_Dialog` | 0x1000125C | Struct | — | — | `Drama_Dialog_SayChannel`, `Drama_Dialog_TextList`, `Drama_Dialog_Type` |
| `Drama_Dialog_Type` | 0x10000CC1 | Enum | 0x23000158 | flag 0x1 | — |
| `Drama_Dialog_SayChannel` | 0x10000BCE | Enum | 0x23000113 | — | — |
| `Drama_Dialog_TextList` | 0x1000069E | Array | — | flag 0x1 | `Drama_Dialog_Text` |
| `Drama_Dialog_Text` | 0x100004C2 | StringInfo | — | — | — |
| `Drama_Emote`, `Drama_Emote_Stop` | 0x10000FB2, 0x10000CC5 | Enum | 0x23000097 (script keys) | — | — |
| `Drama_Sound` | 0x10001AAC | Struct | — | — | `Drama_SoundChannel`, `Drama_SoundMinRandomDistance`, `Drama_SoundMaxRandomDistance`, `Drama_InstanceSound`, `Drama_SoundID` |
| `Drama_SoundID` | 0x10001AAE | DID | DB type 0x12 (sound info, prefix 0x2A) | flag 0x1 | — |
| `Drama_SoundChannel` | 0x10001AB4 | Enum | 0x2300034D | default 4 | — |
| `Drama_SoundMinRandomDistance`, `Drama_SoundMaxRandomDistance` | 0x10002386, 0x10002388 | Float | — | default 0.0, minimum 0.0, maximum 250.0 | — |
| `Drama_InstanceSound` | 0x1000228B | Bool | — | default false | — |
| `Drama_PatrolStart`, `Drama_PatrolLowPriStart` | 0x10000026, 0x10002B1B | String | — | — | — |
| `Drama_PatrolChangeSpeed` | 0x1000076D | Struct | — | — | `Drama_PatrolChangeSpeed_Name` (String, flag 0x1), `Drama_PatrolChangeSpeed_Speed` (Float, flag 0x1) |
| `Drama_ScriptLink` | 0x10000CC4 | Enum | 0x23000157 | — | — |
| `Drama_BroadcastRadius` | 0x10000A34 | Struct | — | — | `Drama_BroadcastRadius_PropertyToMatch` (PID), `Drama_BroadcastRadius_Name` (String), `Drama_BroadcastRadius_Range` (Float); all flag 0x1 |
| `Drama_RemoteDetector_Enable`, `Drama_RemoteDetector_Disable` | 0x10000859, 0x1000069C | Enum | 0x23000114 | default 0 | — |
| `Drama_DirectActors` | 0x10002AC3 | Struct | — | — | `Drama_DirectActors_PatrolStart`, `…_PatrolLowPriStart`, `…_Emote`, `…_Actors`, `…_Dialog`, `…_PerformDrama` |
| `Drama_DirectActors_Actors` | 0x10002AC2 | Bitfield32 | 0x230002CF | flag 0x1 | — |
| `Drama_DirectActors_Dialog` | 0x10002AC4 | Struct | — | — | `…_Dialog_Type` (enum 0x23000158), `…_Dialog_TextList` (array of `…_Dialog_Text`, StringInfo), `…_Dialog_SayChannel` (enum 0x23000113) |
| `Drama_DirectActors_Emote` | 0x10002AC1 | Enum | 0x23000097 | — | — |
| `Drama_DirectActors_PerformDrama`, `…_PatrolStart`, `…_PatrolLowPriStart` | 0x10002AC5, 0x10002B1A, 0x10002B1C | String | — | — | — |
| `Drama_AddEffect` | 0x10001BE5 | Struct | — | — | `Drama_AddEffectID` (DID, DB type 0x45 game object, flag 0x1), `Drama_AddEffect_Spellcraft` (Float, flag 0x1) |
| `Drama_RemoveEffectID` | 0x10001BE2 | DID | DB type 0x45 | — | — |
| `Drama_Scriptlet` | 0x10001179 | DID | DB type 0x15 (scriptlet, prefix 0x0C) | — | — |
| `Drama_ReplicateScriptServerToClient` | 0x1000761D | Bool | — | — | — |
| `Drama_AIHints` | 0x10001BE4 | Struct | — | — | `Drama_SetHints`, `Drama_HintMask` (both any-width bitfields, mapper 0x2300005C, flag 0x1) |
| `Drama_InstanceBillboard` | 0x1000228C | StringInfo | — | — | — |
| `Drama_Debug_Shout` | 0x10000CC0 | String | — | — | — |
| `Drama_DestroySelf`, `Drama_DestroySelf_FadeOut` | 0x1000069F, 0x100065D5 | Bool | — | — | — |
| `Drama_StopStartingIdle` | 0x100004C3 | Bool | — | default false | — |
| `Drama_SetIdleDrama` | 0x100001F3 | String | — | — | — |
| `Drama_ClearIdleDrama` | 0x10001462 | Bool | — | default false | — |
| `Drama_StartingIdle` | 0x100003C5 | String | — | — | — (top level of an instance set, §5.3) |
| `Drama_CanInteractDuringDrama` | 0x1000388F | Bool | — | — | — (top level of a template, §9.2) |
| `Drama_Performing` | 0x1000076E | Bool | — | — | — (run-time state, §9.2) |
| `Drama_Target` | 0x10001370 | Instance ID | — | — | — (never stored) |

The meaning of the dictionary flag bits is `property-system.md` §2.3's business; nothing in this
spec depends on them beyond the defaults listed.

<a id="s3-2"></a>
### 3.2 Drama list and named dramas (Confirmed D3)

A `Drama_DramaList` is an array of **named dramas** of three element kinds, mixed freely and
kept in stored order:

| Element kind | Elements | Fields present (count) |
|---|---|---|
| `Drama_NamedDrama` | 57,843 | `Drama_Name` (all), `Drama_ActionSequence` (all), `Drama_KungFu_BecomeMonster` (3,533), `Drama_KungFu_BecomeNPC` (1,367), `Drama_Interruptible` (1,079), `Drama_WaitForDrama` (503), `Drama_KungFu_BecomeNPC_DoNotEndCombat` (196) |
| `Drama_NamedDrama_Instant` | 3,724 | `Drama_Name` and `Drama_Action_Instant`, both in all |
| `Drama_NamedDrama_Idle` | 321 | `Drama_Name` and `Drama_ActionSequence_Idle`, both in all |

Per context: instance sets 27,270 lists with 56,837 / 3,560 / 317 elements of the three kinds;
static-entity sets 101 lists with 79 / 1 / 0; templates 433 lists with 927 / 163 / 4 (class 796
holds only idle dramas).

- **List length** 0–55 elements: 0 (325 lists), 1 (15,050), 2 (6,006), 3 (2,363), 4 (1,309),
  5–10 (2,247), 11–55 (504). No null element occurs.
- **Name.** `Drama_Name` is a Latin-1 string of 0–64 bytes; 61 names are empty (all in instance
  sets). Within one list no two names are equal, compared exactly or without regard to letter
  case (27,804 of 27,804 lists). Names are designer identifiers, not player-visible text; 16,118
  distinct names (letter case ignored) occur. Names repeat freely across entities: a name
  identifies a drama **only together with the entity** that holds it.
- Every element kind has exactly the fields its definition lists; `Drama_Comment` never occurs.

<a id="s3-3"></a>
### 3.3 Sequences (Confirmed D3)

- `Drama_ActionSequence`: 0–55 `Drama_Action` elements. Lengths: 0 (885 sequences: 860 in
  instance sets, 25 in templates), 1 (36,971), 2 (11,489), 3 (4,286), 4–10 (3,761),
  11–55 (451).
- `Drama_ActionSequence_Idle`: 1–25 `Drama_Action_Idle` elements (535 in 321 sequences).
- `Drama_Action_Instant` is a single action struct, not a sequence (3,724; 7 are empty).
- 194 sequence actions are empty structs.

<a id="s3-4"></a>
### 3.4 The action struct: facets (Confirmed D4)

An action is a struct whose fields are independent **facets**: an action does whatever all of
its present facets say. There is no action-kind tag. The three action structs allow these
fields (✓) and hold them this many times:

| Field (facet) | `Drama_Action` | `…_Instant` | `…_Idle` | Section |
|---|---|---|---|---|
| `Drama_Wait_Before` | ✓ 5,623 | — | ✓ 28 | §4.1 |
| `Drama_Wait_After` | ✓ 34,169 | — | ✓ 477 | §4.1 |
| `Drama_Wait_Variance` | ✓ 435 | — | ✓ 98 | §4.1 |
| `Drama_Wait_Emote` | ✓ 3,419 | — | ✓ 74 | §4.1 |
| `Drama_Dialog` | ✓ 29,147 | ✓ 1,470 | ✓ 312 | §4.2 |
| `Drama_Emote` | ✓ 31,323 | — | ✓ 264 | §4.3 |
| `Drama_Emote_Stop` | ✓ 1,177 | — | ✓ 0 | §4.3 |
| `Drama_Sound` | ✓ 6,642 | ✓ 181 | — | §4.4 |
| `Drama_PatrolStart` | ✓ 10,712 | — | — | §4.5 |
| `Drama_PatrolLowPriStart` | ✓ 2,693 | — | — | §4.5 |
| `Drama_PatrolChangeSpeed` | ✓ 2,780 | ✓ 38 | — | §4.5 |
| `Drama_ScriptLink` | ✓ 15,374 | ✓ 899 | — | §4.6.1 |
| `Drama_BroadcastRadius` | ✓ 4,964 | ✓ 131 | — | §4.6.2 |
| `Drama_RemoteDetector_Enable` / `…_Disable` | ✓ 515 / 194 | ✓ 266 / 34 | — | §4.6.3 |
| `Drama_DirectActors` | ✓ 10,330 | — | — | §4.6.4 |
| `Drama_AddEffect` | ✓ 1,802 | ✓ 384 | — | §4.7 |
| `Drama_RemoveEffectID` | ✓ 489 | ✓ 48 | — | §4.7 |
| `Drama_Scriptlet` | ✓ 1,085 | ✓ 273 | — | §4.7 |
| `Drama_ReplicateScriptServerToClient` | ✓ 32 | ✓ 0 | — | §4.7 |
| `Drama_AIHints` | ✓ 389 | ✓ 290 | — | §4.8 |
| `Drama_StopStartingIdle` | ✓ 3,119 | ✓ 24 | — | §4.8 |
| `Drama_SetIdleDrama` | ✓ 60 | ✓ 38 | — | §4.8 |
| `Drama_ClearIdleDrama` | ✓ 144 | ✓ 10 | — | §4.8 |
| `Drama_DestroySelf` | ✓ 3,374 | ✓ 179 | — | §4.8 |
| `Drama_DestroySelf_FadeOut` | ✓ 2,272 | ✓ 28 | — | §4.8 |
| `Drama_InstanceBillboard` | ✓ 2,593 | ✓ 159 | — | §4.8 |
| `Drama_Debug_Shout` | ✓ 227 | ✓ 110 | ✓ 0 | §4.8 |
| `Drama_ActionComment` | ✓ 0 | — | — | (never stored) |

(The `Drama_Action` column counts the sequence actions of named dramas; idle-sequence actions are
in the third column. A field with ✓ 0 is allowed but not stored.)

The instant action allows neither waits nor emotes nor patrol starts: an instant drama does its
facets at once (§5.2). The idle action allows only waits, dialog, emote, emote stop and debug
text, and holds only waits, dialog and emotes (§5.3).

<a id="s3-5"></a>
### 3.5 Worked byte layout (synthetic)

A sequence element with two facets, `Drama_Wait_After` = 3.0 and `Drama_Emote` = 0x10000123 (a
synthetic script key). The element is a list-form entry (PID, then the struct value); the struct
is a table-form set whose fields are in bucket order for its hint (`records.md` §1.3.1). With
hint 0 the bucket count is 11: 0x100004C4 mod 11 = 2 and 0x10000FB2 mod 11 = 6, so the wait
comes first.

| Offset | Size | Bytes | Meaning |
|---|---|---|---|
| 0 | 4 | `9D 06 00 10` | element PID `Drama_Action` |
| 4 | 1 | `00` | struct TSize hint 0 |
| 5 | 1 | `02` | field count 2 (VLE) |
| 6 | 4 | `C4 04 00 10` | key = PID `Drama_Wait_After` |
| 10 | 4 | `C4 04 00 10` | PID `Drama_Wait_After` |
| 14 | 4 | `00 00 40 40` | f32 3.0 (seconds, §4.1) |
| 18 | 4 | `B2 0F 00 10` | key = PID `Drama_Emote` |
| 22 | 4 | `B2 0F 00 10` | PID `Drama_Emote` |
| 26 | 4 | `23 01 00 10` | u32 enum key 0x10000123 (script key, §4.3) |

Total 30 bytes. Drama structs in this install have at most 8 fields, so their hint is 0
(`property-system.md` §4.10).

---

<a id="s4"></a>
## 4. Action facets

Counts are over all contexts (instance sets, static-entity sets, templates) and over all three
action structs unless a row says otherwise.

<a id="s4-1"></a>
### 4.1 Timing (Confirmed D5; meaning hypothesis)

| Field | Values | Range | Most common |
|---|---|---|---|
| `Drama_Wait_Before` (f32) | 5,651 | 0–300 | 2 (1,011), 3 (867), 1 (820), 4 (618), 5 (452); median 3; zero in 8 |
| `Drama_Wait_After` (f32) | 34,646 | 0–10¹² | 5 (4,947), 3 (4,415), 4 (4,278), 2 (3,374), 1 (3,226), 6 (2,969), 7 (2,333); median 4; zero in 13; 11 values ≥ 600 and 6 values ≥ 9,999 |
| `Drama_Wait_Variance` (f32) | 533 | 0.2–150 | 1 (187), 2 (91), 0.5 (65), 0.25 (55); median 1 |
| `Drama_Wait_Emote` (bool) | 3,493 | true 3,461, false 32 | — |

Co-occurrence: `Drama_Wait_Variance` stands with `Drama_Wait_After` in 278 actions (44 of them
also with `Drama_Wait_Before`), with `Drama_Wait_Before` alone in 251, and with neither in 4.
`Drama_Wait_Emote` stands with `Drama_Emote` in 3,409 actions and without it in 84. Actions
holding only `Drama_Wait_Before` and one other facet are common (`DestroySelf` 452, `PatrolStart`
412, `Emote` 607, `Dialog` 571).

**Hypothesis (this spec writer, from the labels and the co-occurrences; no source states it):**
the unit is seconds; an action first waits `Wait_Before`, then performs its facets, then waits
`Wait_After`; `Wait_Variance` widens the wait by a random amount; with `Wait_Emote` true the
action also waits until the emote it started has finished (the client's emote script runs that
emote persistently exactly then, §9.1); waits of 9,999 s or more mean "until something else stops
the drama".

> **OPEN (timing):** The original's order of the waits and facets, the distribution and sign of
> the variance and which wait it applies to, and what ends an emote wait. The runner is
> server-side: of these fields the client reads only `Drama_Wait_Emote`, in its emote script
> (§9.1), and neither the class library nor the executable reads any of them (§9.5). An owner
> observation (M3) of one ambient NPC with a known drama could time the gaps. Tessera uses
> Decision T1 (§10.1).

<a id="s4-2"></a>
### 4.2 Dialog (Confirmed D5, D6, D7)

`Drama_Dialog` is a struct {`Drama_Dialog_Type`, `Drama_Dialog_TextList`, `Drama_Dialog_SayChannel`}.
30,929 dialog structs (sequence 29,147, idle 312, instant 1,470; instance sets hold 30,053).
`Drama_DirectActors_Dialog` has the same shape for directed actors (§4.6.4).

**Dialog type** (enum mapper 0x23000158, keys 0–10, no base). Labels are quoted from the data as
interoperability names.

| Key | Label | `Drama_Dialog` | `…DirectActors_Dialog` |
|---|---|---|---|
| 0 | `Undef` | 153 | 3 |
| 1 | `Spew` | 103 | 0 |
| 2 | `ColorShout` | 23,165 (instance sets 22,427) | 8,558 |
| 3 | `ColorSpew` | 210 | 2 |
| 4 | `ColorSay` | 2,408 | 79 |
| 5 | `Say` | 89 | 0 |
| 6 | `Shout` | 41 | 12 |
| 7 | `InstanceSay` | 106 | 1 |
| 8 | `InstanceSpew` | 2,207 | 0 |
| 9 | `InstanceShout` | 2,386 | 275 |
| 10 | `ColorOnly` | 61 | 55 |

The type is present in every dialog struct (flag 0x1 field).

**Say channel** (enum mapper 0x23000113: 0 `Undef`, 1 `Dueling`, 2 `Channel1`, 3 `Admin`):
present in 17,488 of 30,929 dialogs — 2 in 16,782, 0 in 678, 1 in 23, 3 in 5 — and in all 8,985
director dialogs (2 in 8,589, 0 in 382, 3 in 12, 1 in 2). Absent in 13,441 dialogs (for type 2:
7,858 absent, 14,800 with channel 2).

**Text list.** 0–20 `Drama_Dialog_Text` elements: 1 in 27,479 lists, 2–14 in 3,446, 20 in 1,
0 in 3 (director dialogs: 1 in 8,917, 2–12 in 68). Every element is a StringInfo **reference**
(no literal occurs): of the 57,455 StringInfos in dramas (dialog texts 45,614, director texts
9,089, billboards 2,752), 57,415 resolve to a (table, token) pair in the English string
archive; 35 are the null reference (table 0, token 0: "no text", `property-system.md` §4.8);
5 name a token absent from an existing table (a lookup miss, `string-table.md`).

> **OPEN (dialog presentation):** How the original shows a line of each type: which chat type
> (`chat-window.md`; the chat-type mapper 0x230002A8 has `Say` 5, `Emote` 7, `Quest` 21,
> `Narration` 22 among others but nothing named after shout or spew), whether `Color*` types add
> overhead speech text, what the `Instance*` types reach, what the say channel selects, and which
> line of a multi-line list is spoken (one at random, in turn, or all). The client does not read
> these fields (§9.5); the server turns them into the messages the client shows, which falls
> under the official-server gate for traffic (decision 0004). **Hypothesis** (labels
> only): `Say`/`Shout`/`Spew` are local chat at increasing reach, `Color*` adds the overhead
> speech display that barks use (`quests.md` §7.8), `ColorOnly` shows the overhead text without
> chat, `Instance*` reaches every player of the instance. Owner observation (M3) of NPCs whose
> dramas use types 2, 4, 8, 9 and 10 could settle the presentation. Tessera uses Decision T2.

<a id="s4-3"></a>
### 4.3 Emote and emote stop (Confirmed D5, D8)

`Drama_Emote`, `Drama_Emote_Stop` and `Drama_DirectActors_Emote` are keys of enum mapper
0x23000097 (base 0x230000F8): the **script keys** of `script-table.md` (the key space of
`RunScript_ID`, of every script table's script keys and of `Emote_ScriptID` in emote records,
`quests.md` §12.2). Every value is labelled by that mapper.

| Field | Values | Distinct | Key 0 | Defined in some script table | Not defined in any table |
|---|---|---|---|---|---|
| `Drama_Emote` | 31,587 | 1,081 | 31 | 30,448 | 1,108 |
| `Drama_Emote_Stop` | 1,177 | 195 | 1 | 1,160 | 16 |
| `Drama_DirectActors_Emote` | 4,879 | 292 | 69 | 4,776 | 34 |

Over all three fields 1,138 distinct non-zero keys occur; 1,104 are the key of a script in at
least one of the 7,524 script tables; 34 are not (one of them alone accounts for 588
`Drama_Emote` values). Whether a key is defined **for a given entity** depends on the entity's
table set (`script-table.md` §8.1, `instance-scripts.md` §4.1) and is not checked here.

`Drama_Emote_Stop` stands alone in 889 actions and with `Drama_Emote` in 288. Key 0 means "no
emote".

How the client plays an emote is §9.1: the emote facet is a script start on the NPC.

<a id="s4-4"></a>
### 4.4 Sound (Confirmed D5, D6, D14)

`Drama_Sound` is a struct; 6,823 occur (sequence 6,642, instant 181). Field sets: {`SoundID`,
`InstanceSound`} 4,340; {`SoundID`} 1,692; {`SoundID`, `SoundChannel`, `InstanceSound`} 497;
{`SoundID`, `SoundChannel`} 269; the rest (25) add the random-distance pair or one of it.

- `Drama_SoundID`: 5,220 distinct sound infos (prefix 0x2A, `sound.md` §4), all present in the
  install.
- `Drama_SoundChannel` (787 values; mapper 0x2300034D with base 0x2300034E: 0 `Default`,
  1 `Combat`, 2 `Music`, 3 `UI`, 4 `VO`, 5 `QuantizedPlayerMusic`, 6 `PlayerMusic`, 7 `Ambient`,
  8 `SFX`): 4 in 752, 8 in 28, 7 in 4, 3 in 2, 0 in 1. Absent means the dictionary default 4
  (`VO`).
- `Drama_InstanceSound`: 4,848 values, true in 4,844.
- `Drama_SoundMinRandomDistance` (21 values, 0–30) and `…MaxRandomDistance` (25 values, 0–100);
  the dictionary bounds both to 0–250.

How the client plays a drama sound is §9.1.

> **OPEN (sound fields):** What `Drama_InstanceSound` and the random-distance pair change. The
> client's drama sound script drives only the sound and the channel (§9.1).

<a id="s4-5"></a>
### 4.5 Patrol facets (Confirmed D5, D9)

| Field | Values | Matches the `LinkName` of a patrol link (type 2, 1 or 11) whose source is the same entity |
|---|---|---|
| `Drama_PatrolStart` (String) | 10,712 (templates 5) | 10,309 of the 10,707 in instance sets (type 2: 8,764; 1: 1,363; 11: 180; both 1 and 2: 2) |
| `Drama_PatrolLowPriStart` (String) | 2,693 | 2,505 |
| `Drama_PatrolChangeSpeed_Name` (in the struct `Drama_PatrolChangeSpeed`) | 2,818 | 2,744 |
| `Drama_DirectActors_PatrolStart`, `…_PatrolLowPriStart` | 615, 62 | on the actor entity, §4.6.4 |

Patrol links, their `LinkName` and `PatrolLink_Speed`: `creatures.md` §6.3 and §6.9. The source of a
patrol link is its `end_id_2` (`instance-scripts.md` §3.5 settles the ends for Script links;
`creatures.md` §6.3 uses the same reading for patrol links). Of the 398 unmatched
`Drama_PatrolStart` values, 251 belong to entities with no outgoing patrol link in the same record
(58 of them have links that cross landblocks), 68 to entities whose patrol links carry no
`LinkName`, and 79 to entities whose patrol links carry other names.

`Drama_PatrolChangeSpeed_Speed` (f32): 2,818 values, 0–40; 1.0 in 2,518, 0.25 in 78, 1.5 in 44,
0.8 in 28, 2.0 in 22, 0 in 10.

**Hypothesis:** `PatrolStart` starts the patrol along the named link, `PatrolLowPriStart` starts
it at a lower priority (so that combat or another drama can override it), and `PatrolChangeSpeed`
sets a speed factor (1 = normal) for the named patrol, like `PatrolLink_Speed`.

> **OPEN (patrol facets):** Priority semantics of the low-priority start, the unit of the speed
> and what happens when the name matches no link. Server-side; not traced.

<a id="s4-6"></a>
### 4.6 Signals to other objects

<a id="s4-6-1"></a>
#### 4.6.1 Script link (Confirmed D10)

`Drama_ScriptLink` is a key of enum mapper 0x23000157 (21 keys: 0 `Undef` and 20 labels
`Custom1`–`Custom20`). 16,273 values (instance sets 16,245, templates 27, static entities 1); 4 are
0.

**Rule:** the facet's **port number** *N* is the number in the key's **label**, not the key. The
labels are not in key order: keys 1–5 and 10–19 carry `Custom` + the same number, but key 6 is
`Custom7`, 7 is `Custom8`, 8 is `Custom9`, 9 is `Custom6`, and key 22 is `Custom20` (keys 20 and 21
do not exist). Read the label at run time and parse its number. Evidence: where key and label
differ, the entity has an outgoing Script link for the label's number in 978 actions and for the
key's number only in 17.

**What the facet does (Confirmed structure):** it starts, on the entity itself, the script whose
key is labelled `ScriptLinks@Drama::OnCustom<N>` (a key of mapper 0x23000097; for example `OnCustom1`
is 0x1000095B in this install). An entity wires that script in its instance script table
(`Entity_InstanceScriptTable`, `instance-scripts.md` §3): a Run node whose Link target names a Script
link (type 12) and starts a script key on the link's other end. In instance sets:

- 14,913 of the 16,241 non-zero facets have an outgoing Script link whose name begins with the label
  of `ScriptLinks@Drama::OnCustom<N>` followed by `_`;
- 14,940 have an instance script table that defines the key `ScriptLinks@Drama::OnCustom<N>`;
  784 belong to entities without an instance script table (or with an empty one), 517 to entities
  whose table lacks that key (the facet then does nothing that the data show).

Script-link names are the two port labels joined by `_`, optionally followed by `_<n>` to make the
name unique (`<source script-key label>_<target script-key label>[_<n>]`). Typical targets of drama
outputs are relays, generators (`Activation@Generator::Trigger`, `…::Activate`,
`…::PurgeDeactivate`), Boolean and integer event boxes, hotspots, and other dramas
(`Signals.ScriptLinks@Drama::Custom<M>`, §6.3). How a Link target resolves is
`instance-scripts.md` §3.5 (Q4 there).

<a id="s4-6-2"></a>
#### 4.6.2 Broadcast (Confirmed D11)

`Drama_BroadcastRadius` = {`PropertyToMatch` (PID), `Name` (String), `Range` (f32)}; 5,095 values.

- `PropertyToMatch`: `AI_AcceptsBroadcasts` 5,022; `State@Entity.m_bState_1` 28;
  `Generator_OwnerID` 16; `Entity_CreationType` 11; the PID labelled `Invalid` 10;
  `Aggro@NPC::CurrentTarget` 7; `State@Entity.m_bState_2` 1. `AI_AcceptsBroadcasts` (Bool, no
  default) is stored, always true, on all 20,161 mob templates (class 1723), 16,599 of the 16,605
  NPC templates (1724), 16 class-4026 and 6 class-5785 templates, and on no instance set of an
  entity that a broadcast names.
- `Range`: 0–300, median 60; 80 (1,477), 100 (711), 50 (440), 40 (434), 30 (368).
- `Name` (instance sets, 5,058 values; landblock-local resolution, letter case ignored): a drama of
  that name is held by **another** entity of the same landblock record in 4,138; only by the
  broadcasting entity itself in 82; only in other landblocks in 723; nowhere in 115.

**Hypothesis:** the facet asks every entity within `Range` (metres) of the speaker whose
`PropertyToMatch` property qualifies (true, for the Boolean case) to perform its drama named
`Name`.

> **OPEN (broadcast):** The matching rule for each property type (for `Generator_OwnerID`
> presumably "spawned by the same generator", for `Entity_CreationType` and the `Invalid` PID
> unknown), whether the speaker itself is included, and how range is measured. Server-side.

<a id="s4-6-3"></a>
#### 4.6.3 Remote detectors (Confirmed D12)

`Drama_RemoteDetector_Enable` (781) and `…_Disable` (228) are keys of mapper 0x23000114: 0 `Undef`,
1 `AI_RemoteDetector_2`, 2 `AI_RemoteDetector_3`, 3 `AI_RemoteDetector_4`, 4 `AI_RemoteDetector_All`,
5 `AI_RemoteDetector_1`. Enable: 5 in 686, 4 in 55, 1 in 37, 2 in 3. Disable: 5 in 205, 4 in 13,
1 in 10. The same mapper labels `AI_RemoteDetector_SlotName` on AIRemoteDetector links (type 19,
`landblock.md` §4.6; with `AI_RemoteDetector_Activated`). Of the 1,001 facets in instance sets,
766 name the slot of one of the entity's outgoing type-19 links, 14 more say `All` for an entity
that has such links, 126 belong to entities without type-19 links, and 95 to entities whose
type-19 links carry no slot name or other slots.

**Hypothesis:** the facet switches the remote detection volume of that slot (or of all slots) on or
off; `AI_RemoteDetector_Activated` on the link is its initial state.

> **OPEN (detector facets):** The effect of the facet, and its target when the entity has no link of
> that slot. Server-side.

<a id="s4-6-4"></a>
#### 4.6.4 Directing actors (Confirmed D13)

`Drama_DirectActors` (10,330 values, instance sets only, 633 sets) makes the speaker a **director**
of up to six **actors**. Fields: `…_Actors` (all), `…_Dialog` (8,985), `…_Emote` (4,879),
`…_PatrolStart` (615), `…_PerformDrama` (190), `…_PatrolLowPriStart` (62).

**Actor selection.** `Drama_DirectActors_Actors` is a 32-bit bitfield labelled by mapper 0x230002CF
(keys 1–6). By the bit rule of `property-system.md` §7.3, bit *i* is key *i* + 1; the keys' labels
are not in order: key 1 `Actor05`, 2 `Actor06`, 3 `Actor01`, 4 `Actor02`, 5 `Actor03`, 6 `Actor04`.
So bit 2 (value 4) selects actor 01, bit 3 actor 02, bit 4 actor 03, bit 5 actor 04, bit 0 actor
05 and bit 1 actor 06. Observed masks: 4 (4,404), 8 (3,030), 16 (1,265), 32 (755), 1 (504), 2 (206),
other masks with up to six bits 164, and 0 (no actor) in 2.

**Casting.** Actor *NN* of a director is the far end (`end_id_1`) of each outgoing Script link of
the director whose name begins with the label `Scriptlinks@Drama.Director::CastActor<NN>`; the
link's target port is `Scriptlinks@Drama.Actor::CastAsActor`. In 10,102 of the 10,330 facets every
selected actor has at least one such link (628 directors cast actor 01, 557 actor 02, 358 actor 03,
228 actor 04, 137 actor 05, 67 actor 06); 228 facets select an actor that is not cast.

**What actors are told.** `…_Dialog` (§4.2 shape) and `…_Emote` (§4.3) are spoken and played by
the actors; `…_PerformDrama` names a drama **on the actor** (found in the actor's list for 351 of
380 cast actors that have a list; 25 more actors have no list in their instance set);
`…_PatrolStart` and `…_PatrolLowPriStart` name a patrol link **of the actor** (627 of 674 resolvable
cases match). The facets apply to every selected actor.

---

<a id="s4-7"></a>
### 4.7 Effects and scripts (Confirmed D14)

- `Drama_AddEffect` = {`Drama_AddEffectID`, `Drama_AddEffect_Spellcraft`}: 2,186 values.
  `Drama_RemoveEffectID`: 537. The 476 distinct effect DIDs all name 0x70 records whose root
  class descends from the effect root class (698 here; `combat-data.md`). Spellcraft (f32): −1 in
  610, 0 in 1,060, 1 in 127, and 0.08–160 in the rest (`combat-data.md` C49: a spellcraft of −1
  keeps the effect's own).
- `Drama_Scriptlet`: 1,358 values naming 104 distinct scriptlets (prefix 0x0C), all present.
- `Drama_ReplicateScriptServerToClient` (true in all 32) stands with `Drama_Scriptlet` in 31 of
  its 32 actions; 1,327 scriptlet facets lack it.

**Hypothesis (Inferred from the flag's label and from §9.1):** a scriptlet facet runs on the server;
with the replicate flag it is also started on clients through the client drama script
`Drama@NPC::DramaServerToClientScriptlet`.

> **OPEN (scriptlet facets):** Where a scriptlet without the replicate flag runs and what it can do
> there; the target of `AddEffect` and `RemoveEffectID` (presumably the speaker). Server-side.

<a id="s4-8"></a>
### 4.8 State, idle and removal facets (Confirmed D5, D15)

| Field | Values | Notes |
|---|---|---|
| `Drama_AIHints` {`Drama_SetHints`, `Drama_HintMask`} | 679 | Any-width bitfields labelled by mapper 0x2300005C (176 labels; the AI hint keys of `creatures.md` §5.1), bit *i* ↔ key *i* + 1. In all 679 the bit count equals the highest set bit + 1 (0 for an empty field). `SetHints` ⊆ `HintMask` in 674 (equal in 483, a proper part in 119, empty in 72); 5 set a bit outside the mask. Most common masks: bit 15 (key 16) 145, bit 16 70. |
| `Drama_StopStartingIdle` | 3,143 | true 3,086, false 57 (§8) |
| `Drama_SetIdleDrama` | 98 | Names a drama of the same list: an idle drama in 91, another named drama in 1, none in 6 (§5.3) |
| `Drama_ClearIdleDrama` | 154 | true in all |
| `Drama_DestroySelf` | 3,553 | true 3,545, false 8 |
| `Drama_DestroySelf_FadeOut` | 2,300 | true 2,296, false 4; stands alone in 1,942 actions |
| `Drama_InstanceBillboard` | 2,752 | StringInfo (§4.2 resolution) |
| `Drama_Debug_Shout` | 337 | A plain string (not a StringInfo); designer debug text |

**Hypotheses (labels only):** `AIHints` sets the masked AI hint bits to the values in `SetHints`
(the hints that condition AI actions, `creatures.md` §5.1); `DestroySelf` removes the entity,
`DestroySelf_FadeOut` removes it after fading it out; `InstanceBillboard` shows the text as
on-screen billboard text to the players of the instance; `Debug_Shout` is never shown to players.

> **OPEN (state facets):** The hint semantics (including the 5 bits outside the mask), what a false
> `StopStartingIdle`, `DestroySelf` or `DestroySelf_FadeOut` means, and how a billboard is shown.
> Server-side.

<a id="s4-9"></a>
### 4.9 Facet combinations (Confirmed D4)

Sequence actions hold 0–8 facets: 0 (194), 1 (47,566), 2 (35,496), 3 (12,540), 4 (4,072), 5 (474),
6 (34), 7 (5), 8 (1); 889 distinct facet sets occur. The ten most common: {`Emote`} 10,731;
{`ScriptLink`} 8,719; {`DirectActors`, `Wait_After`} 6,933; {`Dialog`} 6,615; {`PatrolStart`} 5,984;
{`Dialog`, `Wait_After`} 5,240; {`Emote`, `Wait_After`} 4,999; {`Dialog`, `Emote`, `Wait_After`}
3,118; {`DestroySelf`} 2,744; {`BroadcastRadius`} 2,230. Instant actions: {`Dialog`} 899,
{`ScriptLink`} 611, {`RemoteDetector_Enable`} 240, {`Scriptlet`} 232, {`AIHints`} 219, {`AddEffect`}
211. Idle actions: {`Dialog`, `Wait_After`} 175, {`Emote`, `Wait_After`} 117, {`Dialog`,
`Wait_After`, `Wait_Variance`} 59, {`Emote`, `Wait_Emote`} 38.

---

<a id="s5"></a>
## 5. Named-drama kinds and flags (Confirmed D16)

<a id="s5-1"></a>
### 5.1 `Drama_NamedDrama`

| Flag | Values | Dictionary default |
|---|---|---|
| `Drama_Interruptible` | 1,079: true 1,021, false 58 | false |
| `Drama_WaitForDrama` (f32) | 503: 0–10¹²; 999,999 (102), 9,999 (60), 9,999,999 (46), 99,999 (44), 3 (37), 10 (31), 10⁸ (22), 999 (22), 600 (19); median 9,999 | — |
| `Drama_KungFu_BecomeMonster` | 3,533: true 3,525 | false |
| `Drama_KungFu_BecomeNPC` | 1,367: true 1,358 | false |
| `Drama_KungFu_BecomeNPC_DoNotEndCombat` | 196: false 131, true 65 | **true** |

**Hypotheses:** `Interruptible` lets another drama or event cut this one short; `WaitForDrama` is a
time limit in seconds within which something waiting on this drama (a quest event with
`QuestEvent_WaitForDrama`, §6.2) gets its result, the huge values meaning "no limit". The KungFu
flags are §7.

> **OPEN (drama flags):** The original meaning of `Interruptible` (default not interruptible) and
> of `WaitForDrama`. Server-side.

<a id="s5-2"></a>
### 5.2 `Drama_NamedDrama_Instant`

One action without waits, emotes or patrol starts (§3.4): its facets take effect together and
the drama ends at once. 3,724 occur; 224 of them are enter-world signals and 98 combat-begin
signals (§6.4).

<a id="s5-3"></a>
### 5.3 `Drama_NamedDrama_Idle` and the starting idle drama

An idle drama holds only dialog, emote and wait facets. It is selected by name:

- `Drama_StartingIdle` (String) sits at the **top level** of 400 instance sets (382 of them hold a
  list): it names an idle drama of the same list in 208, a `Drama_NamedDrama` in 3, and nothing in
  the list in 171 (18 sets have no list at all).
- `Drama_SetIdleDrama` facets (98) name an idle drama of the same list in 91.
- `Drama_ClearIdleDrama` facets (154) clear it.

Of the 317 idle dramas of instance sets, 208 are named by a starting idle, 88 by a set-idle facet, 6
by quest events and 18 by nothing (one drama can be named by several).

**Hypothesis:** the current idle drama repeats while the NPC has nothing else to do; a performed
named drama interrupts it and the idle resumes afterwards; the script keys labelled
`Signals@NPC::IdleDramaEnd` and `ScriptLinks@NPC::OnIdleDramaEnd` (mapper 0x23000097; defined in no
script table) are the related events.

> **OPEN (idle loop):** Whether and how often the original repeats an idle drama, and what
> interrupts and resumes it. Tessera uses Decisions T1 and T5.

---

<a id="s6"></a>
## 6. What starts a named drama

<a id="s6-1"></a>
### 6.1 Census of references (Confirmed D17)

For each of the 60,714 named dramas of instance sets, the references that name it (letter case
ignored; one drama can have several; columns are named-drama, instant and idle elements):

| Starter | §  | `NamedDrama` | `…_Instant` | `…_Idle` |
|---|---|---|---|---|
| A `QuestEvent_DramaName` value anywhere equals the name | 6.2 | 24,594 | 1,247 | 6 |
| An incoming Script link input `Custom<N>` and the name `ScriptLink_Custom<N>` | 6.3 | 16,015 | 1,079 | 0 |
| Another entity's broadcast names it | 4.6.2 | 7,123 | 162 | 0 |
| The enter-world signal name | 6.4 | 5,839 | 224 | 1 |
| A `PatrolPoint…` name | 6.4 | 2,098 | 66 | 0 |
| A `ScriptLink_Custom<N>` name without an incoming link for *N* | 6.3 | 1,905 | 167 | 6 |
| A scripted-fight health transition of the entity | 7 | 1,473 | 512 | 0 |
| Other signal names | 6.4 | 1,133 | 165 | 0 |
| A `QuestEvent_KungFuDramaName` value | 6.2, 7 | 676 | 5 | 0 |
| A scripted-fight quest transition of the entity | 7 | 514 | 0 | 0 |
| A `Quest_ObjectiveVolumeString` or `QuestEvent_EventVolumeString` value | 6.2 | 460 | 5 | 0 |
| A director's `…_PerformDrama` on this actor | 4.6.4 | 300 | 44 | 0 |
| A `QuestEvent_TappedDramaName` value | 6.2 | 113 | 6 | 0 |
| `Drama_StartingIdle` / `Drama_SetIdleDrama` of the entity | 5.3 | 3 / 1 | 0 | 208 / 88 |
| A property-response drama callback | 6.7 | 24 | 7 | 0 |
| **None of the above** | — | 11,481 | 1,016 | 18 |

The "quest" rows match names globally and so over-count where generic names repeat; §6.2 gives the
entity-bound resolution.

<a id="s6-2"></a>
### 6.2 Quest events (Confirmed D18)

Quest event entries (`quests.md` §5.3) name dramas with `QuestEvent_DramaName` (4,688 entries),
`QuestEvent_KungFuDramaName` (535) and `QuestEvent_TappedDramaName` (91); event types as in
`quests.md` §5.4 (DramaName: type 11 talk 2,661, 5 use 1,282, 19 254, 24 emote 247, 13 138, 29 54,
1 52; KungFu name: 27 245, 19 244, 29 46; tapped name: 19 69, 29 20, 27 2). Resolution, letter case
ignored:

| Event has | DramaName entries | Drama found |
|---|---|---|
| `QuestEvent_RoleConstraint` *R* | 2,079 | on an instance set whose `QuestDispenser_RoleConstraint` equals *R*: 1,988; only elsewhere: 17; nowhere: 74 |
| `QuestEvent_NPCTalk` (a template DID) and no role key | 2,207 | in that template's own list: 122; on some instance set: 2,001; nowhere: 84 |
| neither | 402 | somewhere: 383; nowhere: 19 |

KungFu names (all with a role key): 530 of 535 resolve on the role-key instance; tapped names (all
with a role key): 84 of 91.

**Rule (Confirmed structure):** a quest event that names a drama addresses the entity carrying the
event's role key (`QuestDispenser_RoleConstraint` = `QuestEvent_RoleConstraint`); that entity's list
holds the drama. `QuestEvent_RunDramaOnAdvance` (1,103 entries, default false) marks events whose
drama runs when the event advances; `QuestEvent_WaitForDrama` (895) makes the event listen to the
event types labelled `DramaComplete` (17) and `DramaFailed` (12) of mapper 0x2300004F (`quests.md`
§6.2 listen lists); `QuestEvent_DramaProgressOverride` (535) is the journal text while waiting.

> **OPEN (quest-started dramas):** For events with only `QuestEvent_NPCTalk`, which instance
> performs the drama (the instance sets carry no template DID, `creatures.md` §6.2); when a drama
> counts as failed; and what `QuestEvent_RunDramaOnAdvance` false means for events that still name
> a drama (the drama then presumably runs when the event is first offered or tapped). Server-side.

<a id="s6-3"></a>
### 6.3 Script-link inputs (Confirmed D19; meaning Inferred)

A Script link whose name ends with the label `Signals.ScriptLinks@Drama::Custom<N>` (optionally
followed by `_<n>`) delivers, at its target entity, the script key of that label (a key of mapper
0x23000097, `Custom1`–`Custom20`). In instance sets, 10,807 entities receive such inputs, 17,331
(entity, *N*) pairs in all; for 17,094 of them the entity's list holds a drama named
`ScriptLink_Custom<N>` (letter case ignored; observed spellings `Scriptlink_Custom<N>` 13,730 names,
`ScriptLink_Custom<N>` 4,521, `scriptlink_custom<N>` 1,016, and a few others). Senders are relays,
tripwires, random boxes, generators (`OnCleared`), NPC death and combat-end signals, usage signals,
hotspots and other dramas (§4.6.1).

**Reading (Inferred):** when the input `Custom<N>` arrives at an NPC, the server performs the NPC's
drama named `ScriptLink_Custom<N>`. The 20 input keys are defined in 8 script tables, none of them
the shared NPC tables of §9.1, so the reaction is server code. This is the main way level designers
chain dramas to world objects.

<a id="s6-4"></a>
### 6.4 Conventional signal names (Confirmed D20 census; matching Inferred)

Many drama names follow conventions that name engine events rather than designer scenes. Counts are
named dramas over all contexts, letter case ignored:

| Name form (interoperability identifiers, quoted from the data) | Dramas | Case variants seen |
|---|---|---|
| `Signal_EnterWorld` | 6,177 | `signal_enterworld` 3,850, `Signal_Enterworld` 1,452, `Signal_EnterWorld` 771, `Signal_enterworld` 69 |
| `Signal_CombatBegin` | 629 | 4 |
| `Signal_CombatEnd` | 262 | 2 |
| `Signal_Generator_Unsummon` | 133 | 1 |
| `Signal_CombatIntro` | 121 | 3 |
| `Signal_OnDeath` | 65 | 2 |
| `Signal_EnterCombat` | 55 | 2 |
| `Signal_Stage<N>` | 31 | 2 |
| `Signal_` + a festival or world-event word | 9 each | 1 |
| `PatrolPoint_<N>`, `PatrolPoint_Start` and other `PatrolPoint…` (instance sets) | 2,164 | many |
| Other `Signal…` names (24 forms: misspellings and one-offs) | 51 | — |

**Reading (Inferred):** the server performs an entity's drama named after an event when the event
happens: entering the world (spawn), combat begin, intro, end, death, a generator unsummoning its
objects, and, judging by the `PatrolPoint…` names, reaching patrol points. Because the same
convention appears in several spellings with hundreds of uses each (for example both
`signal_enterworld` and `Signal_EnterWorld`), the original most likely compares drama names
**without regard to letter case**; misspelt names (`Signal_EnterWordl`) would then never run.

> **OPEN (signal names):** The full set of engine events and their names, how `PatrolPoint_<N>`
> relates to patrol points (no patrol-point property names a drama), the meaning of
> `Signal_Stage<N>`, and whether matching is ASCII case folding or something else. Server-side;
> owner observation (M3) of a spawning camp could confirm the enter-world rule. Tessera uses
> Decision T3.

<a id="s6-5"></a>
### 6.5 Broadcasts and directors

A broadcast (§4.6.2) performs a named drama on qualifying entities in range; a director's
`…_PerformDrama` (§4.6.4) performs a named drama on its cast actors.

<a id="s6-6"></a>
### 6.6 Scripted-fight transitions

§7.

<a id="s6-7"></a>
### 6.7 Property responses (Confirmed D21)

102 drama-bearing instance sets carry `PropertyResponse_EntityPropertyResponseMap`, naming one of
17 response maps (root class 2192). A map holds `PropertyResponse_WorldEventResponse_List`: entries
{`PropertyResponse_WorldEvent` (a world-event record, `quests.md` §14), `PropertyResponse_Callback`
(a callback record)}. Callback classes: 2225 (11 records: `PropertyResponseCallback_Drama` (String)
and `PropertyResponseCallback_OnTrueResponseType` (mapper 0x23000291, key 8 `Activate`)), 2193 (21:
an AI-hint callback, `PropertyResponseCallback_AIHint`) and 2353 (8: applied-effect callbacks). Over
the 637 (entity, drama-callback) pairs the named drama is in the entity's own list in 30 and absent
in 607.

**Reading (Inferred):** when the world event becomes true the server performs the callback's
drama on the entity (response type `Activate`).

> **OPEN (property responses):** Who performs the 607 callback dramas that the entity does not hold
> (perhaps every entity in the area or zone, through the area and zone map properties that exist in
> the dictionary). Server-side.

<a id="s6-8"></a>
### 6.8 Unreferenced dramas

12,515 named dramas of instance sets are named by none of the starters above. Their names are mostly
designer verbs and stages (patrol pickups and arrivals, tier changes, triggers, endings). They may be
started by references this spec has not found, or by server code through names this install does
not show.

> **OPEN (unreferenced dramas):** What starts these 12,515 dramas. Server-side; a Tessera runner
> leaves them unstarted until a reference is found.

---

<a id="s7"></a>
## 7. Scripted fights ("KungFu") (Confirmed D22)

The data call a staged fight in which an NPC turns into a combatant (and possibly back) **KungFu**;
quest event type 27 is labelled `KungFu` (`quests.md` §5.4). The entity-level properties:

| Property | Type, default | Where (sets) | Values |
|---|---|---|---|
| `AI_KungFu_StartAsNPC` | Bool, false | instance 11,680; templates 1,330 (mostly class 1723); static-entity sets 37 | instance: true 10,000, false 1,680 |
| `AI_KungFu_Repeatable` | Bool, **true** | instance 5,346; templates 1,849; static-entity sets 22 | instance: false 4,901, true 445; templates false in all but 1 |
| `AI_KungFu_EnableCombat` | Bool, false | instance 865 | true 307 |
| `AI_KungFu_IsNPC` | Bool, false | never stored | run-time state, read by the client (§9.3, §9.5) |
| `AI_KungFu_TransitionList` | Array of `AI_KungFu_DramaTransition` and `AI_KungFu_QuestTransition` | instance 1,725 (1,689 with a list); templates 22 | 2,731 + 36 transitions |

**Transitions.**

| Element | Fields | Count | Name in own list |
|---|---|---|---|
| `AI_KungFu_DramaTransition` | `AI_KungFu_Transition_Drama` (String, all), `AI_KungFu_Transition_HealthRatio` (f32, all), `AI_KungFu_ShouldEndCombat` (246: true 94), `AI_KungFu_Transition_AlwaysActive` (43: true 21), `AI_KungFu_Transition_SuppressHealthClamp` (6: true) | instance 2,206, templates 36 | 2,195 of 2,206; 36 of 36 |
| `AI_KungFu_QuestTransition` | `AI_KungFu_Transition_Drama` (all), `AI_KungFu_Transition_HealthRatio` (all), `AI_KungFu_Transition_AlwaysActive` (2: false) | instance 525 | 520 of 525 |

Health ratios: drama transitions 0–33, all but 2 within 0–1; quest transitions 0.01–0.99; over all
2,767 transitions the most common are 0.5 (365), 0.25 (300), 0.1 (278), 0.2 (248), 0.75 (203).

Named dramas carry the state switch: `Drama_KungFu_BecomeMonster` (true in 3,525) and
`Drama_KungFu_BecomeNPC` (true in 1,358), with `Drama_KungFu_BecomeNPC_DoNotEndCombat` (default
true). 254 of the
2,767 transitions name a drama that becomes monster or NPC; most transition dramas only speak or
emote. Quest events of types 27, 19 and 29 name dramas with `QuestEvent_KungFuDramaName` (530 of
535 found on the role-key entity, §6.2) and switch the fight with `QuestEvent_RunKungFu` (48
entries, default true).

**Hypothesis (labels and structure):** an entity with `AI_KungFu_StartAsNPC` appears as a
non-hostile NPC; a drama with `BecomeMonster` turns it into a hostile combatant, one with `BecomeNPC`
turns it back (ending combat only when `DoNotEndCombat` is false; the default, true, keeps combat
going); a
drama transition runs its drama once when the entity's health falls to `HealthRatio` × maximum (or
below), `AlwaysActive` keeping it armed outside a quest, `ShouldEndCombat` ending combat, and
`SuppressHealthClamp` letting health fall past the threshold; a quest transition does the same only
while the related quest event is active; `Repeatable` false means the fight can happen only once per
spawn; `EnableCombat` lets the entity fight while still an NPC. `AI_KungFu_IsNPC` is the run-time state.

> **OPEN (scripted fights):** The original transition evaluation (threshold comparison, arming,
> repetition), what the health clamp is, the 2 ratios above 1, and the effect of `EnableCombat`.
> Server-side; no client code reads these properties except `AI_KungFu_IsNPC` (§9.5).

<a id="s7-1"></a>
### 7.1 Full labels of the scripted-fight fields (Confirmed D27)

Every label of this section, as the property dictionary defines it (interoperability
identifiers; PIDs and defaults are this install's and are read at run time). No other label
of the dictionary contains `KungFu`; in particular there is no `AI_KungFu_HealthRatio`.

| Label | PID | Type | Dictionary default | Child of |
|---|---|---|---|---|
| `AI_KungFu_StartAsNPC` | 0x100009D3 | Bool | false | entity sets |
| `AI_KungFu_Repeatable` | 0x10000A9B | Bool | true | entity sets |
| `AI_KungFu_EnableCombat` | 0x100011EC | Bool | false | entity sets |
| `AI_KungFu_IsNPC` | 0x100014EA | Bool | false | run-time state |
| `AI_KungFu_TransitionList` | 0x100000A0 | Array | — | entity sets; elements are the two structs below |
| `AI_KungFu_DramaTransition` | 0x1000036A | Struct | — | `AI_KungFu_TransitionList` |
| `AI_KungFu_QuestTransition` | 0x100013FC | Struct | — | `AI_KungFu_TransitionList` |
| `AI_KungFu_Transition_Drama` | 0x100013FD | String | — | both transition structs |
| `AI_KungFu_Transition_HealthRatio` | 0x10000538 | Float | — | both transition structs |
| `AI_KungFu_Transition_AlwaysActive` | 0x10003E43 | Bool | false | both transition structs |
| `AI_KungFu_ShouldEndCombat` | 0x1000071A | Bool | false | `AI_KungFu_DramaTransition` only |
| `AI_KungFu_Transition_SuppressHealthClamp` | 0x100061E1 | Bool | false | `AI_KungFu_DramaTransition` only |
| `Drama_KungFu_BecomeMonster` | 0x10000EBF | Bool | false | `Drama_NamedDrama` |
| `Drama_KungFu_BecomeNPC` | 0x10001464 | Bool | false | `Drama_NamedDrama` |
| `Drama_KungFu_BecomeNPC_DoNotEndCombat` | 0x100027AE | Bool | true | `Drama_NamedDrama` |
| `QuestEvent_KungFuDramaName` | 0x10000F74 | String | — | quest events (§6.2) |
| `QuestEvent_RunKungFu` | 0x10001508 | Bool | true | quest events (§6.2) |

"Child of" for the two transition structs is the dictionary's child list: the quest transition
lists only `…Transition_Drama`, `…Transition_AlwaysActive` and `…Transition_HealthRatio`. In
§10.3 item 5, `HealthRatio` is `AI_KungFu_Transition_HealthRatio` and `DoNotEndCombat` is
`Drama_KungFu_BecomeNPC_DoNotEndCombat`.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-09):** the full labels are in the table
> above. The health ratio is `AI_KungFu_Transition_HealthRatio`; a reader needs to resolve only
> that label.

---

<a id="s8"></a>
## 8. Starting idles (Confirmed D23)

NPC-like entities carry a **starting idle** animation that dramas interrupt (`script-table.md` §8.6
gives the animation path; `appearance.md` §7 and `walker.md` use it for placed figures):

| Property | Type, default | Instance sets | Static-entity sets | Templates |
|---|---|---|---|---|
| `NPC_StartingIdleAnimation` | Enum 0x23000110 (296 labels) | 114,251 | 51,185 | 16 |
| `NPC_PlayingStartingIdle` | Bool, false | 111,312 | 47,997 | 25 |
| `NPC_ReturnsToStartingIdle` | Bool, false | 13,431 | 6,321 | 10 |
| `NPC_StartingIdle_AnimStartOffset` | f32, 0.0 | 156 | 119 | — |

Among the 27,270 drama-bearing instance sets, 2,828 hold a `Drama_StopStartingIdle` facet; 2,641 of
those also carry `NPC_StartingIdleAnimation` (with `NPC_ReturnsToStartingIdle` stored in 293).

**Hypothesis:** a stop-starting-idle facet ends the starting idle animation for the rest of the
drama (so emotes play from a neutral stance), and `NPC_ReturnsToStartingIdle` true brings it back when
the drama ends.

> **OPEN (starting idle):** Whether the original ends the starting idle by setting
> `NPC_PlayingStartingIdle` (the property the shared fidget driver of `script-table.md` §8.6 tests),
> and when it resumes it. Server-side.

---

<a id="s9"></a>
## 9. What the client does with dramas

<a id="s9-1"></a>
### 9.1 Client drama scripts in the shared NPC table (Confirmed D24)

Script table 0x07000B0A (kind 2) is included by table 0x07000097, which is included by table
0x07000001; 0x07000001 is the `ScriptTable_DBScriptTable` of 20,152 of the 20,161 mob templates
(class 1723), 16,569 of the 16,605 NPC templates (1724) and a few others (classes 4026, 2097, 796,
5785). It defines four scripts (keys of mapper 0x23000097; labels quoted as interoperability names):

| Key (observed) | Label | Content (node kinds of `script-table.md` §5) |
|---|---|---|
| 0x10003486 | `Drama@NPC::DramaServerToClientScript` | A Switch on `Drama_Wait_Emote`: for true, a Run node with `Persistent` = 1; for false, a Run node with `Persistent` = 0. In both the run's script key is **driven by `Drama_Emote`** |
| 0x10003485 | `Drama@NPC::DramaServerToClientScriptlet` | A Scriptlet node whose file is **driven by `Drama_Scriptlet`** |
| 0x10000E4A | `Drama@NPC::PlaySound` | A WhenRunning node watching 22 voice-over keys (labels `VO@NPC::Bestowal`, `VO@NPC::Generic`, `VO@NPC::Vendor`, `VO@NPC::Custom1`–`Custom20`): its not-running port holds a Sound node (channel 4, moves with the entity) whose sound and channel are **driven by `Drama_SoundID` and `Drama_SoundChannel`**; its running port holds nothing |
| 0x100011A5 | `Drama@NPC::FailedDramaSanityCheck` | Visual-effect nodes (a marker for a failed check) |

So the client side of an emote, a replicated scriptlet and a drama sound is a **script start on the
NPC with the drama's properties as parameters**: the driven fields read `Drama_Emote`,
`Drama_Wait_Emote`, `Drama_Scriptlet`, `Drama_SoundID` and `Drama_SoundChannel` from the start's
property context (`script-runtime.md` §5). A drama sound does not play while one of the NPC's
voice-over scripts runs (`script-runtime.md` §13 port rule). The key labelled
`Drama@NPC::PerformDramaAction` and the key `Drama@NPC.Director::DirectActors` are defined in no
script table.

What starts these scripts in the original is the server (the start reaches the client over the
network, outside this spec). Dialog lines have no client script: they arrive as text (§4.2 OPEN).

<a id="s9-2"></a>
### 9.2 Drama state in the class library (WLB; Confirmed D25)

Classes 1039 and 1742 share these bodies (slot numbers of this install):

- **Slot 1, "performing":** the receiver's `Drama_Performing` read as a Boolean with default false.
- **Slot 2:** the negation of slot 1.
- **Slot 4, "may interact":** true when slot 1 is false; otherwise the receiver's
  `Drama_CanInteractDuringDrama` with default false.
- Slot 3 builds the usage text of an administrator command; class 1742's slot 5 hands an
  administrator request to perform a named drama on an entity to native code (not followed).

`Drama_Performing` is never stored: it is set at run time on an entity while it performs a drama.
`Drama_CanInteractDuringDrama` is stored on 4 mob templates (class 1723), true in all.

Callers: slot 1 is used by the mouse-over rule of the usage-action classes (1551, 1554, 1555, 1556 and
1587 here; `interaction.md` §3.4.4–§3.4.5 already states these rules); slot 2 by class 295/461 slot 94;
slot 4 by class 287 slot 31 (§9.3).

<a id="s9-3"></a>
### 9.3 The quest-interaction check (WLB)

Class 287 slot 31 (called from class 1753 slots 79 and 155, the quest-journal class of
`interaction.md`'s quest tests) decides whether a quest interaction with an entity *E* may proceed.
Its drama part: when *E* is performing a drama (`Drama_Performing` true) and its
`Drama_CanInteractDuringDrama` is not true, the check fails. The same function also reads
`AI_Usage_CanNotBeUsed`, `Quest_InteractableWhileDead`, `Combat_Violent` and
`Death_CorpseIsUsable`, and, when a class test that was not followed holds, makes an entity whose
`AI_KungFu_IsNPC` is not true pass through the harmful-interaction test of class 434; those parts
belong to `interaction.md`'s quest tests (§14 note).

<a id="s9-4"></a>
### 9.4 Barks share the drama sound script (WLB)

The colour-text bark code of `quests.md` §7.8 (class 279 slot 1) plays a bark line's sound by building
a property set with `Drama_SoundID` = the line's sound and `Drama_SoundChannel` = 4 and starting the
script key 0x10000E4A (`Drama@NPC::PlaySound`, §9.1) on the NPC with it (the native function that
starts it is unnamed; that it starts the script with that set is Inferred from the arguments and from
§9.1). This confirms how the drama sound script takes its parameters.

<a id="s9-5"></a>
### 9.5 What the executable does not do (BIN; D26)

- No native code reads a drama action or dialog property. Every 32-bit immediate in the code equal to
  the PID of `Drama_Dialog`, `Drama_Dialog_Text`, `Drama_Dialog_TextList`,
  `Drama_DirectActors_Dialog_Text`, `Drama_DirectActors_Dialog_TextList`, `Drama_Wait_After`,
  `Drama_StopStartingIdle`, `Drama_DestroySelf`, `Drama_PatrolChangeSpeed_Name`, `Drama_InstanceSound`,
  `Drama_RemoteDetector_Disable` or `Drama_BroadcastRadius_Name` is an argument of a user-interface
  child-element lookup or a layout instantiation (the same identifier range as property IDs), and the
  data-section hits of `Drama_Name`, `Drama_StartingIdle`, `Drama_Dialog_Text` and
  `Drama_Action_Instant` lie in tables of unrelated identifiers. `Drama_Dialog_Type`,
  `Drama_Wait_Before` and `Drama_Performing` occur nowhere in the file, and the one byte match of
  `Drama_Emote` lies inside an unrelated instruction. The code sites of
  `Drama_NamedDrama_Idle`, `Drama_NamedDrama_Instant` and `Drama_SetIdleDrama` (consecutive small
  values) are user-interface lookups, a registration of handlers for a run of consecutive small
  identifiers, and a type test against the same identifiers; none passes the value to a property
  getter. The PIDs below 0x10000100 (`Drama_ActionSequence`, `Drama_PatrolStart`,
  `Drama_Interruptible`) equal many small identifiers of other kinds and were not classified site
  by site.
- The file contains no text `drama` in 8-bit or 16-bit characters.
- `AI_KungFu_IsNPC` is passed to the property getters (functions that take a PID and a value slot and
  are called with properties of every type; one of them tests the result for the Boolean type code) at
  10 sites (not followed).

Together with §9.1–§9.4: the client has no drama interpreter. It runs the client drama scripts it is
told to run, displays the lines it is sent, and reads `Drama_Performing`,
`Drama_CanInteractDuringDrama` and `AI_KungFu_IsNPC` for interaction rules.

> **OPEN (KungFu state on the client):** what the native reads of `AI_KungFu_IsNPC` decide
> (presumably name colour, targeting or the attack cursor). Not traced.

---

<a id="s10"></a>
## 10. Playback in Tessera (Decisions)

Everything in this section is Tessera's own rule, chosen where the original is not known (the OPENs
of §4–§8). It is not a claim about the original.

<a id="s10-1"></a>
### 10.1 Decision T1: running one named drama

To **perform** drama *D* on entity *E* (with a random source owned by the runner):

1. Set `Drama_Performing` = true on *E*. If *D* is instant, apply its action's facets (step 3.2) and
   go to step 5.
2. Take the actions of *D*'s sequence in stored order. An empty sequence ends at once.
3. For each action:
   1. Wait `Wait_Before` seconds (0 when absent).
   2. Apply the facets present, in this order: `StopStartingIdle`, `Emote_Stop`, `Emote`, `Dialog`,
      `Sound`, `DirectActors`, `PatrolChangeSpeed`, `PatrolStart`, `PatrolLowPriStart`, `ScriptLink`,
      `BroadcastRadius`, `RemoteDetector_Enable`, `RemoteDetector_Disable`, `AIHints`, `AddEffect`,
      `RemoveEffectID`, `Scriptlet`, `SetIdleDrama`, `ClearIdleDrama`, `InstanceBillboard`,
      `DestroySelf_FadeOut`, `DestroySelf`. `Debug_Shout` is ignored. A facet whose reference does not
      resolve (§4) does nothing and is logged once.
   3. Wait `Wait_After` seconds (0 when absent). When `Wait_Variance` *v* is present, add an
      amount drawn uniformly at random from [0, *v*] seconds to `Wait_After` when that field is present,
      otherwise to `Wait_Before` of this action (applied before step 3.2 in that case).
   4. When `Wait_Emote` is true and the action started an emote, also wait until that emote's script
      has ended, at most 30 seconds.
   5. A wait of 9,999 seconds or more is "until stopped": the drama stays at this point until it is
      interrupted or the entity is removed.
4. After the last action, the drama ends.
5. Set `Drama_Performing` = false on *E*. If *E* has `NPC_ReturnsToStartingIdle` true, resume its
   starting idle; resume its idle drama (T5).

A second perform request on an entity that is performing *D* replaces *D* when *D* has
`Drama_Interruptible` true, and is queued after *D* otherwise (queue length 4; further requests are
dropped and logged). A perform request whose name is not in *E*'s list is ignored and logged.

<a id="s10-2"></a>
### 10.2 Decision T2 and T5: what the offline client shows

The offline walker and client have no server. They run only the client-visible part:

- **T5 (scope):** on entity creation the client performs the entity's drama named
  `Signal_EnterWorld` (T3 matching) if present, then starts the idle drama named by
  `Drama_StartingIdle`; idle dramas repeat with a pause of 5 seconds between passes while no other
  drama runs. Dramas named by quests, links, broadcasts, transitions and the other starters of §6 run
  only when a server or sandbox backend (§10.3) asks for them. Offline, the facets that need server
  state (`PatrolStart`, `PatrolLowPriStart`, `PatrolChangeSpeed`, `ScriptLink`, `BroadcastRadius`,
  `RemoteDetector_*`, `DirectActors` other than its dialog and emote, `AIHints`, `AddEffect`,
  `RemoveEffectID`, KungFu flags) do nothing; `DestroySelf` and `DestroySelf_FadeOut` hide the entity
  until its landblock reloads.
- **Emote:** start the script key of `Drama_Emote` on the entity through its table set
  (`script-runtime.md` §12), persistent when `Wait_Emote` is true — what the original's client drama
  script does (§9.1). `Emote_Stop` stops that key on the entity.
- **Sound:** start the entity's `Drama@NPC::PlaySound` script with `Drama_SoundID` and
  `Drama_SoundChannel` as parameters (§9.1).
- **Scriptlet:** when `Drama_ReplicateScriptServerToClient` is true, start
  `Drama@NPC::DramaServerToClientScriptlet` with `Drama_Scriptlet`; otherwise do nothing offline.
- **T2 (dialog):** pick one line of the text list uniformly at random; resolve it (`string-table.md`
  §9); skip a null or unresolved reference. Show it as overhead speech text over the speaker for 10
  seconds (the bark duration of `quests.md` §7.8) for the types whose label begins with `Color`
  (2, 3, 4 and 10); and log it to the chat window as chat type 5 (`Say`), prefixed by the speaker's
  name, for every type except 10. The other types (0, 1, 5–9) are logged to chat only. The say channel
  is ignored. Director dialogs and emotes are applied to each cast actor as if the actor performed
  them.
- `InstanceBillboard`: show the resolved text centred on screen for 5 seconds.

<a id="s10-3"></a>
### 10.3 Decision T6: the server or sandbox runner

A server backend runs T1 for every entity with a list and starts dramas from:

1. quest events (§6.2: the role-key entity), with `DramaComplete` or `DramaFailed` reported to waiting
   events when a drama ends normally or is interrupted or the entity is removed before it ends; a
   `Drama_WaitForDrama` limit reports `DramaFailed` when exceeded;
2. Script-link inputs `Custom<N>` (§6.3: drama `ScriptLink_Custom<N>`), and output facets as
   `instance-scripts.md` wires them (§4.6.1);
3. the signal names of §6.4 for spawn (`Signal_EnterWorld`), combat begin, intro, end, death and
   generator unsummon;
4. broadcasts (§4.6.2, with `AI_AcceptsBroadcasts` true as the qualifying test and the speaker
   excluded) and directors (§4.6.4);
5. KungFu transitions (§7): a drama transition fires once when health falls to or below
   `HealthRatio` × maximum, re-armed on respawn; `BecomeMonster` sets `AI_KungFu_IsNPC` false and makes
   the entity hostile, `BecomeNPC` sets it true and ends combat unless `DoNotEndCombat` is true;
6. starting and set idles (§5.3, T5).

<a id="s10-4"></a>
### 10.4 Decision T3: name matching

Drama names are compared after ASCII case folding (letters A–Z equal a–z); no other normalisation.
Lookups are within the effective list of the addressed entity (T4).

---

<a id="s11"></a>
## 11. Explorer drama viewer (Decision T7)

For a selected entity (instance set, static entity or template) the explorer shows:

1. the effective list (T4) with, per named drama: kind, name, flags (§5), action count, the
   starters that reference it (§6.1 rows, resolved for this entity), and a warning mark for names
   that fail T3 matching against every starter;
2. per action: its facets in the T1 order, with timings, dialog type label, say channel label,
   resolved lines (all lines of a list), emote and stop keys with their labels and whether the
   entity's table set defines them, sound info DID and channel label, patrol names with the matched
   link, script-link label and the wired targets, broadcast range and match, director's cast with
   actor entities, effect and scriptlet DIDs;
3. the entity's scripted-fight properties and transitions (§7) and starting-idle properties (§8);
4. a "play" button that runs T1 under T2/T5 rules in the character viewer, with two preview
   rules (Decision T7, 2026-10-09; they affect only the preview, not T1–T6):
   1. **Figure.** The dramas play on the figure the character view shows. An instance set of a
      server-placed object names no template (§2.2), so for such a selection the shown figure
      stands in for the actor, and every emote and stop key is resolved through **that
      figure's** script tables (`script-runtime.md` §12.2). The viewer says so next to the
      button.
   2. **Emote clip.** An emote key plays one clip: among the clips that a run of the key would
      start on the figure (§4.3, `script-runtime.md` §12), the one with the largest product of
      blend weight and chance, held clips excluded; ties go to the first in table order. A key
      that would start no clip on the figure is logged and plays nothing; the drama continues
      with its next facet.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-09):** both choices of the
> implementer are adopted as the two preview rules of item 4 (Tessera Decisions; the original
> has no drama preview).

---

<a id="s12"></a>
## 12. Invariants

Checked on this install (tests on real data skip when `TESSERA_GAME_DIR` is unset). Counts are
observations; tests should assert the rules and report the counts.

1. Every `Drama_DramaList` element is one of the three named-drama kinds and has `Drama_Name` and
   its sequence or instant action; within a list no two names are equal after ASCII case folding.
2. Every element of `Drama_ActionSequence` is a `Drama_Action`, of `Drama_ActionSequence_Idle` a
   `Drama_Action_Idle`; every field of an action struct is one its definition lists (§3.4); no null
   entry occurs anywhere in a drama tree.
3. Every `Drama_Dialog` and `Drama_DirectActors_Dialog` has a dialog type with a key in 0–10.
4. Every drama StringInfo is a table reference (no literal); all but 40 resolve; the 35 null ones
   are (0, 0).
5. Every non-zero `Drama_Emote`, `Drama_Emote_Stop` and `Drama_DirectActors_Emote` is labelled by
   mapper 0x23000097.
6. Every `Drama_AddEffectID` and `Drama_RemoveEffectID` names a 0x70 record whose root class
   descends from the effect root; every `Drama_Scriptlet` names an existing 0x0C record; every
   `Drama_SoundID` names an existing 0x2A record.
7. Every non-zero `Drama_ScriptLink` key has a label of the form `Custom<N>` with 1 ≤ *N* ≤ 20;
   the label numbers of keys 6, 7, 8, 9 and 22 are 7, 8, 9, 6 and 20.
8. `Drama_DirectActors_Actors` has no bit above bit 5.
9. In each `Drama_AIHints`, the bit count of each field equals its highest set bit + 1 (0 when no bit
   is set).
10. Each static entity whose own set holds a drama list has an instance set holding an identical
    list.
11. At least 95 % of `Drama_PatrolStart` values in instance sets equal the `LinkName` of an outgoing
    patrol link of the same entity (observed 10,309 / 10,707).

---

<a id="s13"></a>
## 13. Worked synthetic examples

<a id="s13-1"></a>
### 13.1 A greeting with a pause (T1, T2)

Instance set of NPC *E*: `Drama_DramaList` = [`Drama_NamedDrama` {`Drama_Name` = "Signal_EnterWorld",
`Drama_ActionSequence` = [ A1 {`Dialog` {`Type` = 4, `TextList` = [ref (0x25000001, 7)]}, `Emote` =
*K*, `Wait_After` = 3.0}, A2 {`Wait_Before` = 1.0, `Emote` = *K2*, `Wait_Emote` = true} ]},
`Drama_NamedDrama_Idle` {`Drama_Name` = "idle_a", `Drama_ActionSequence_Idle` = [ {`Emote` = *K3*,
`Wait_After` = 6.0} ]}], `Drama_StartingIdle` = "idle_a".

Offline, on creation (T5): *E* performs "Signal_EnterWorld" (matched case-insensitively):
t = 0 s emote *K* starts (non-persistent) and the line (type 4 `ColorSay`) appears over *E* for 10 s
and in chat as `Say`; t = 3 s A2 waits 1 s; t = 4 s emote *K2* starts persistently; the drama waits
until *K2*'s script ends (at most 30 s); then `Drama_Performing` returns to false and the idle drama
"idle_a" starts: emote *K3*, 6 s, a 5 s pause, again.

<a id="s13-2"></a>
### 13.2 A script-link output (§4.6.1)

An action {`Drama_ScriptLink` = key 9}. The mapper labels key 9 `Custom6`, so the action starts
`ScriptLinks@Drama::OnCustom6` on the entity (not `OnCustom9`). The entity's instance script table
defines that key with a Run node targeting the link named
"`ScriptLinks@Drama::OnCustom6_Activation@Generator::Trigger`"; the server starts the generator
trigger script on the link's `end_id_1`.

<a id="s13-3"></a>
### 13.3 Selecting actors (§4.6.4)

`Drama_DirectActors_Actors` = 0x0000000C: bits 2 and 3, keys 3 and 4, labels `Actor01` and
`Actor02`. The director's outgoing Script links whose names begin with
`Scriptlinks@Drama.Director::CastActor01` and `…CastActor02` lead to the two actors; each speaks the
director dialog and plays the director emote of the action.

<a id="s13-4"></a>
### 13.4 A health transition (§7, T6)

`AI_KungFu_StartAsNPC` = true, `AI_KungFu_TransitionList` = [`AI_KungFu_DramaTransition`
{`Transition_Drama` = "turn", `HealthRatio` = 0.5}], and a named drama "turn" with
`Drama_KungFu_BecomeMonster` = true and one dialog action. Under T6, when the entity's health first
reaches 50 % or less, the runner performs "turn": the line is spoken and the entity becomes hostile
(`AI_KungFu_IsNPC` false).

---

<a id="s14"></a>
## 14. Notes for other specs (not edited here)

- `quests.md` §5.3: `QuestEvent_DramaName` and the KungFu and tapped names resolve on the role-key
  entity (§6.2 here); the volume-string OPEN there now has the trigger census of §6.1.
- `creatures.md` §6.2 counts `Drama_DramaList` (27,169) on instance sets without a static entity;
  §2.1 here gives all contexts.
- `interaction.md` §3.4.5 (quest test A): class 287 slot 31 of this install reads, besides the drama
  test of §9.3, `AI_Usage_CanNotBeUsed`, `Quest_InteractableWhileDead`, `Combat_Violent`,
  `AI_KungFu_IsNPC` and `Death_CorpseIsUsable` (§9.3); its full rule is not stated here.
- `sound.md` §5: `Drama_SoundID` drives the sound of `Drama@NPC::PlaySound` (§9.1), which barks also use.

---

<a id="s15"></a>
## 15. Open questions (summary)

| Section | Question |
|---|---|
| §4.1 | Wait order, variance and emote-wait semantics |
| §4.2 | Presentation of each dialog type and say channel; choice among several lines |
| §4.4 | Meaning of `Drama_InstanceSound` and the random-distance pair |
| §4.5 | Low-priority patrol starts; speed unit |
| §4.6.2 | Broadcast matching rule per property type; speaker inclusion; range measure |
| §4.8 | AI-hint semantics; false values of stop-idle and destroy facets; billboard display |
| §5.1 | `Drama_Interruptible`, `Drama_WaitForDrama` |
| §5.3 | Idle-drama repetition and resumption |
| §6.2 | The performing instance for `QuestEvent_NPCTalk` events; failure conditions; `RunDramaOnAdvance` false |
| §6.4 | Engine events and their names; `PatrolPoint_<N>`; the exact case rule |
| §6.7 | Who performs callback dramas the entity does not hold |
| §6.8 | Starters of the 12,515 unreferenced dramas |
| §4.6.3, §4.7 | Effect of detector facets; where scriptlets run; effect targets |
| §8 | How the starting idle is ended and resumed |
| §7 | Transition evaluation, health clamp, ratios above 1, `EnableCombat` |
| §9.5 | What the client's reads of `AI_KungFu_IsNPC` decide |

All but the last are server behaviour: no client code holds them (§9). They can be narrowed by owner
observation (M3) of the original; the traffic side stays behind the gate of decision 0004.

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (status Review). Rows D1–D26 and Decisions T1–T7. |
| 2026-10-09 | Revision (spec writer) answering the two implementer OPENs of 2026-10-09: new §7.1 with the full labels, PIDs, types, defaults and child lists of every scripted-fight field and drama state switch (D27, from the dictionary); the elided labels of §7 spelled in full; §11 item 4 gains the two preview rules (figure and emote clip) as Decision T7 items. Status Revised. |
