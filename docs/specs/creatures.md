<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Creatures, NPCs and spawning: templates, AI tables and the generator chain

| | |
|---|---|
| **Status** | Accepted (OPEN-closing revision of 2026-10-08, §15; narrow re-review of 2026-10-08 passed) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate session, not the author; second and third reviews by further separate sessions, none of them the author) |
| **Review record** | Third review 2026-10-07 (separate session, not the author): **passed**, Accepted. Full record moved to the review log (decision 0018). Revision of 2026-10-08: reviewed 2026-10-08 (separate session, not the author): **not passed**, changes requested (four items). Narrow re-review 2026-10-08 (separate session, not the author): **passed**, Accepted. Details in the review log. |
| **Implements into** | A gameplay-data layer on `crates/props` and `crates/wstate` (template store, authored-subset extraction, AI and generator decoders; crate placement is the implementer's choice); `crates/world` (generator, spawn-point and patrol-path index per landblock); `crates/net` sandbox backend (spawning, §6.8); `apps/explorer` (creature card and spawn overlay, §10); `apps/dattool` (census commands) |
| **Companion specs** | [`wstate.md`](wstate.md) (object records 0x70, bundles, link and available-property sections §6–§7); [`property-system.md`](property-system.md) (property sets, the +0x09000000 rule §6.2, enum and bitfield labels §7, DID maps §8); [`wlib.md`](wlib.md) (class library, ancestry); [`landblock.md`](landblock.md) §4.6 (links), §4.8 (weenie list), §5 (block maps); [`entity-descriptor.md`](entity-descriptor.md) §4 (static entities, instance property sets §4.5.2); [`setup.md`](setup.md) §4 (0x47 entity descriptors); [`appearance.md`](appearance.md) §7 (how NPC and creature figures are dressed); [`environment.md`](environment.md) §4 (region tables; the encounter table is decoded here, §7); [`coordinates.md`](coordinates.md) (map coordinates for overlays); [`string-table.md`](string-table.md) §9 (rendering `Name` and other StringInfo values) |

---

<a id="purpose"></a>
## Purpose

A server with mobs and NPCs, and the explorer's creature views, need four things from the
data files:

1. the **creature templates**: which object records are mobs, NPCs and spawnable containers,
   how to identify them without hard-coding class indices, and which of their roughly 400
   properties per record carry authored gameplay values (identity, classification,
   appearance, AI, loot references, quest roles) and which are derived copies or values that
   are the same in every record of the class (§2–§4);
2. the **AI records** the templates point at: action tables, the probability table, attitude
   and classification records (with the rule by which an attitude table judges another
   entity, §5.3.1), colour-event tables, formations, conjunctions and puppet packages (§5);
3. the **generator chain** from a landblock's generator instance set through its links to
   spawn points and patrol paths, and through a generator profile to the templates it can
   create (§6);
4. the **encounter table** layout (prefix 0x14, §7), the enum mappers involved (§8) and the
   whole DID graph (§9).

It also states what the data do **not** contain (mob level, vitals, stat scaling, respawn
timings, the templates of most placed spawns, §4.8, §6.10), so that a server knows what it has
to supply itself, and it defines the explorer's creature card and spawn overlay (§10).

Everything here is read at run time from the user's install. Class indices, DIDs and counts
are **observations** of one install (§1); code must find records by the rules of §2.2 and
must not hard-code the observed values.

---

<a id="provenance"></a>
## 1. Provenance

How sources are marked:

- **Confirmed**: observed in the user's install with throwaway probes in the dirty-room
  scratch area, over **every** record of the kind named (not a sample), on the date given.
- **Reported**: from published third-party material only.
- **BIN**: read from the client executable under method M4 (decision 0011).
- **Decision**: a Tessera choice where the data and the client say nothing; it is not a
  claim about the original.
- **Inferred**: used only as a qualifier inside a Confirmed or BIN row (C27, C30) for one
  named part of the fact that was not observed directly but concluded by this spec writer
  from observed evidence (how code uses a native function, or two numberings with the same
  values). The rest of the row keeps its mark; nothing in the spec depends on the inferred
  part beyond what the confirmed part already gives.

| Key | Source |
|---|---|
| OBS | User install (Steam), observed 2026-10-06 and 2026-10-07, read-only. Probes decoded with the project's own decoders (read-only use) and Python, over: all 383,332 object records with prefix 0x70 and their 0x79 property sets (root classes read at offset 4, `wstate.md` §3; the other 11 object records have prefix 0x76, are singletons without a property set, `wstate.md` §2, and none is a template), the class library (0x56000000), the property dictionary (0x34000000), every enum mapper named here, the WEENIECONTENT DID map, all 49,636 landblock-info records (`client_cell_*.dat`: 3,253,862 static entities, 1,324,469 links, 3,768,840 instance property sets), every 0x47 entity descriptor named by a static entity, and the five 0x14 records. |
| LC | LotroCompanion `lotro-tools` (read for facts only): its mob, NPC, classification, action-table and generator loaders. Used for: entries with `GeneratorProfile_Weight` or `GeneratorProfile_Probability` ≤ 0 are skipped; action-table level bounds ≤ 0 mean "no bound"; the `AIActionProbability` table name; the existence of species-level loot tables. No code or names beyond interoperability identifiers taken. |
| DS | `research/notes/data-semantics.md` §5.2 (class 1723 mob, 1724 NPC), §6.3 (link types; generators resolve through `GeneratorProfile_Profile`). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`), analysed headless in the dirty room on 2026-10-07. **Question:** creature templates carry no level and no vitals (§4.8); which properties does the client itself read to present a creature, and with what default when the level is absent? **Why M1–M3 were not enough:** the data show only that the level is absent; what the client shows in its place is behaviour; no published source documents it; an in-game observation (M3) would show one creature at a time and could not tell a default from a server-sent value. **Read:** a search of the code for the 32-bit property IDs of §4–§6 (most matches turned out to be UI element identifiers in the same number range, not property reads, and were discarded), and one routine that fills a creature information panel from an entity's property table, with its property lookups. Nothing from network, login, launcher or anti-tamper code was read. |
| OBS2 | User install (Steam), observed 2026-10-08, read-only, data files unchanged since 2026-10-04. Throwaway probes in the dirty-room scratch area, using the project's decoders as tools, over: the object form (0x70 bundle) of all 38,651 templates, all 4,168 generator profiles, all 124 attitude records, the one classification-filter-list record (class 977) and the WEENIECONTENT entry `ClassificationControl`; every 0x79 property set of every 0x70 record, searched for DIDs of classes 1653 and 1654; the link-type enum mapper (0x230000BE, base 0x230001A9) and mapper 0x23000112; the `gameplayactivatables` UI layout record (0x22000012, searched for two element IDs); and the bytecode of the class library (0x56000000), disassembled with the rules of `wlib-bytecode.md` (an Accepted spec): the bodies of classes 884, 974, 975 and 976, class 1658's lookup functions, the class-1196 accessors and fill function, the class-434 function that calls class 884 for the harmful-interaction check (a second caller in class 434 was located, not read), and a search of the whole code for 32-bit pushes of the property IDs named in §5.3.1, §5 and §6. |
| BIN2 | **M4 (decision 0011), second use, 2026-10-08.** Same client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, file unchanged since 2026-10-04), headless decompiler and disassembler on a copy of the shared analysis project (deleted after use), single addresses and functions only. **Questions:** (1) which window owns the creature information panel of C9; (2) whether the executable reads `ForwardSource_Movement_SpeedMultiplier` or the property IDs of the AI action tables, attitude filter lists, colour-event tables, generator profiles, `Generator_ProfileDID` or `PatrolLink_Speed` directly; (3) where the client reads the 0x14 encounter records. **Why M1–M3 were not enough:** the data hold the panel's layout but not which code fills it; whether code reads a property is not visible in the data; no published source covers either; the running client (M3) shows the panel only when a skill opens it and shows nothing of property reads. **Read:** a byte search of the executable for those 32-bit IDs and the instructions or table entries around each of the 18 occurrences; the panel routine's caller and that caller's caller (a window method, not followed further); for (3), the cache-size setting and the DID-range registration of prefix 0x14 (the reader itself was not reached). Nothing from network, login, launcher or anti-tamper code was read. |
| OBS3 | User install (Steam), observed 2026-10-08 (review fixes), read-only, data files unchanged since 2026-10-04. Fresh throwaway probes in the dirty-room scratch area, using the project's decoders as tools, over: every 0x79 property set of every one of the 383,332 0x70 records, searched for DIDs of classes 1653 and 1654 and counted per avatar record (references and distinct referrer records); the object form of all 4,168 generator profiles walked in parallel with their property sets, every attribute of every node compared with its property-set counterpart; all 549 filter exceptions against the species-to-genus table; all 351 stored `ClassificationFilter_Filter_Default` values; the EMAPPER entry of mapper 0x230000BE. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| C1 | Creature subtree: 38,651 object records in ten root classes under one library class; per-class counts and sizes (§2.1) | OBS (all 383,332 records with prefix 0x70) | Confirmed 2026-10-06; recounted 2026-10-07 |
| C2 | `WeenieType` labels identify mob templates (`Monster`, one class), NPC templates (`RealNPC`, `NPC`) and containers (`GameplayContainer`) (§2.2) | OBS (all 0x70 sets) | Confirmed 2026-10-07 |
| C3 | Template object record: one root object, one DID item naming the attitude table, one import of it; link schema and instance-property list (§3.1, §3.3, §3.4) | OBS (38,651) | Confirmed 2026-10-07 |
| C4 | Mirror properties: six name patterns whose value equals the base property in every record where the base is present (one exception); the eight base-less mirror names, seven of whose bases have no dictionary definition, and the values of the base-less mirrors (§3.2.1) | OBS (38,651 sets; 6,508,079 mirror occurrences; 185,058 base-less occurrences; all 27,546 dictionary definitions) | Confirmed 2026-10-07 |
| C5 | Class-constant and varying properties per class; authored subset sizes (§3.2) | OBS (20,161 + 16,605 + the small classes) | Confirmed 2026-10-07 |
| C6 | Link schema direction 0 is the link's source end (`end_id_2`), 1 its target end (`end_id_1`) (§3.3, §6.3) | OBS (schemas of the creature and generator classes against 1,324,469 landblock links); derived by this spec writer | Confirmed 2026-10-07 (consistent on every link whose ends can be typed) |
| C7 | Template property census and DID targets (§4.1–§4.7) | OBS (all records of each class) | Confirmed 2026-10-07 |
| C8 | No level, no stat scaling, no per-template vitals in the templates; the level-range records are empty (§4.8) | OBS (every property set; 24 level-range records) | Confirmed 2026-10-07 |
| C9 | The client's creature information panel reads name, `Advancement_Level` (1 when absent), genus, species, cached health and power maxima, examination rank, alignment, state immunities and conjunction immunity, and derives three immunity indicators from bit masks (§4.8) | BIN | BIN only |
| C9a | That panel is the analyse-skill panel: element `AnalyzeSkillComponent` inside the top-level element `GamePlay_AnalyzeSkillPanel` of the `gameplayactivatables` layout (§4.8) | BIN2 (the panel routine's callers); OBS2 (element labels from mapper 0x230000D1; both IDs present in layout 0x22000012) | BIN (code path); Confirmed (labels, layout) |
| C10 | Agent species, genus and alignment equal the referenced classification record's (§5.4) | OBS (36,929 of 36,929) | Confirmed 2026-10-07 |
| C11 | Action tables, attitude, classification, colour-event, formation, conjunction and puppet records: layouts and censuses (§5) | OBS (all records of each class) | Confirmed 2026-10-07 |
| C12 | `AIAction_Probability` codes are converted to percentages by a table reached through WEENIECONTENT (§5.2) | LC (table name); OBS (table found and decoded) | Confirmed 2026-10-07 |
| C13 | Generator instance sets, their links and the GeneratorPoint position-set rule (§6.2–§6.4) | OBS (all landblocks) | Confirmed 2026-10-07 |
| C14 | 0x47 `kind` equals the static-entity type of every static entity that names the record (§6.4) | OBS (3,210,410 static entities) | Confirmed 2026-10-07 |
| C15 | Generator-profile grammar (§6.5) | OBS (all 4,168 profiles) | Confirmed 2026-10-07 |
| C16 | Profile object form: the role of every bundle class, the attribute name hash of every node field, and the object tree equals the property-set tree in every profile (§6.6) | OBS (all 4,168, counts); OBS2 (all 4,168 compared node by node, 4,168 equal); OBS3 (hashes: each attribute matched its role in every node of its class, 4,168 profiles, 54,850 script objects) | Confirmed 2026-10-08 |
| C17 | Entries whose `GeneratorProfile_Weight` or `GeneratorProfile_Probability` is 0 or less are skipped (§6.8) | LC | Reported |
| C17a | The sandbox's choice, placement, repeat, respawn and AI-action rules, including the reading of weight as a relative weight and of probability as per mille (§6.8, Decision D3). That reading is a hypothesis of this spec writer, kept under the §6.8 OPEN; it is not a fact about the original. (D3's attitude step follows C27 and is not a hypothesis.) | Tessera | Decision |
| C18 | Patrol links and path graph (§6.9) | OBS (all landblocks) | Confirmed 2026-10-07 |
| C19 | Encounter table layout (§7) | OBS (5 of 5 records decoded to the last byte) | Confirmed 2026-10-07; meaning OPEN |
| C20 | Enum mappers and their entry counts (§8) | OBS | Confirmed 2026-10-07 |
| C21 | Tessera rules D1 (template store and its lookup, including the rule that a base is never answered from a mirror, §3.2.1, §3.2.2), D2 (instance-property list, §3.4), D4 (unscoped entries, §6.7), D5 and D6 (tool views, §10), D7 (colour-event extra event, §5.5), D8 (classification values of a sandbox entity, §5.3.1) | Tessera | Decision |
| C22 | Classes 1653 and 1654 are the administrator and player forms of the 69 avatar templates; each of the 138 records is named by exactly one **kind** of referrer (race-table gender entries, monster-play records or the PvP-control record), by one to seven referrer records of that kind; multiplicities per record as in §2.1 | OBS2 (every 0x79 set of every 0x70 record searched; 15 + 67 + 1 references per class, 69 of 69 records of each class reached); OBS3 (per-record counts); `player-character.md` §4 | Confirmed 2026-10-08 |
| C23 | Reference attributes of the template root object: per class, which are non-null and what they point at (§3.1) | OBS2 (all 38,651 root objects) | Confirmed 2026-10-08 |
| C24 | Link type 13 is the key labelled `PathMapTest` of the link-type enumeration (§3.3) | OBS2 (mapper 0x230000BE with base 0x230001A9); `wstate.md` §6 | Confirmed 2026-10-08 |
| C25 | The executable holds no direct reference to the property IDs of `ForwardSource_Movement_SpeedMultiplier`, `AI_ActionTable_Array`, `AIActionTable_Actions`, `Attitude_AggressiveFilterList`, `Attitude_FriendlyFilterList`, `AI_ColorEventTable`, `AIColorEventTable_Events`, `AIColorEventTable_Event`, `GeneratorProfile_Profile`, `Generator_ProfileDID` or `PatrolLink_Speed`: of these 11 IDs, 7 do not occur anywhere in the file, and the 18 occurrences of the other 4 are UI element IDs: 12 in UI code (child-element lookups and handler tables) and 6 in data tables of element IDs; each of the four numbers is also a labelled key of the UI element-ID enumeration (mapper 0x230000D1) (§3.2.1, §5, §6) | BIN2; OBS2 (labels) | BIN only (Confirmed for the labels) |
| C26 | The class library's code reads none of these properties: `AI_ActionTable_Array` and its element fields, `AIActionTable_Actions` and the `AIAction_*` fields of §5.1, `AI_ColorEventTable`, `AIColorEventTable_Events`, `AIColorEventTable_Event` and its fields, the `GeneratorProfile_*` properties present in this install (§6.5), `PatrolLink_Speed`. Every 32-bit push of one of their IDs (none for most) is a result code, a comparison or switch constant, a DID-map key or a formatting argument of an unrelated function (§5.1, §5.5, §6.8, §6.9) | OBS2 (all such pushes in the bytecode, each read in context) | Confirmed 2026-10-08 |
| C27 | Attitude evaluation: the class library evaluates an attitude table against another entity's five classification values by the rule of §5.3.1 (aggressive list first, then friendly; per filter type one filter; `Default`; seven exception keys) | OBS2 (bytecode of classes 884, 974, 975, 976, 1658, 1196, 434) | Confirmed 2026-10-08 for the code path; the meaning of the native map and set operations it calls is Inferred (from their use and the data) |
| C28 | Object form of attitude and filter-list records: per list a map from filter type to one filter; per filter a `Default` integer and a set of 64-bit exception keys built by the §5.3.1 key rule; it equals the property set in every list (§5.3.1) | OBS2 (all 124 attitude records and the class-977 record: 233 lists, 652 filters, 549 exceptions; no genus conflict) | Confirmed 2026-10-08 |
| C29 | The WEENIECONTENT entry `ClassificationControl` (class 1658) lists 116 classification records and carries, in object form, a species-to-genus map that equals their `Classification_Species` and `Classification_Genus` values (§5.3.1) | OBS2 (116 of 116) | Confirmed 2026-10-08 |
| C30 | Attitude results 1, 2 and 3 carry the labels `Neutral`, `Friendly` and `Aggressive` of mapper 0x23000112 (§5.3.1) | OBS2 (the mapper; the codes returned by class 884's code) | Confirmed for the labels; that the code uses this mapper's numbering is Inferred (same values) |

---

<a id="s1a"></a>
## 1a. Terms

| Term | Meaning |
|---|---|
| **Template** | An object record (prefix 0x70) whose root class lies in the creature subtree (§2.1), with its property set at DID + 0x09000000. A template is not an instance: placed creatures are created from it at run time. |
| **Mob template** | A template whose `WeenieType` is labelled `Monster`. |
| **NPC template** | A template whose `WeenieType` is labelled `RealNPC` or `NPC`. |
| **Container template** | A template whose `WeenieType` is labelled `GameplayContainer` (chests and resource nodes). |
| **Instance set** | One entry of a landblock-info weenie list: an instance ID and an embedded property set (`landblock.md` §4.8; `entity-descriptor.md` §4.5.2). |
| **Generator** | An instance set that is the source of Generator_PositionSet links (type 7) or the target of Generator links (type 8); 34,296 of them hold `Generator_ProfileDID` (§6.2). |
| **Spawn point** | A static entity of type GeneratorPoint (3). |
| **Position set** | A value of enum mapper 0x23000047 that groups spawn points; carried by the spawn point's 0x47 marker (§6.4). |
| **Generator profile** | An object record whose property set holds `GeneratorProfile_Profile` (§6.5). |
| **Mirror property** | A property whose name follows one of the patterns of §3.2.1 and whose value copies another property of the same set. |
| **Authored subset** | The properties of a template that are neither mirrors nor class-constant (§3.2.2). |

Property names in this spec are the labels of the install's property dictionary, quoted as
interoperability identifiers. Enum labels are read from the install's enum mappers and are
quoted only where a rule depends on them.

---

<a id="s2"></a>
## 2. Template records

<a id="s2-1"></a>
### 2.1 The creature subtree (Confirmed census, all 383,332 object records with prefix 0x70)

Every template's root class (`wstate.md` §3.1) descends from one library class, index 1732 in
this install; the library gives the ancestry (`wlib.md`). Ten root classes occur below it:

| Root class | Records | 0x70 bytes | 0x79 bytes | Top-level properties per set | `WeenieType` | Kind |
|---|---|---|---|---|---|---|
| 1723 | 20,161 | 14,184,781 | 115,556,507 | 440–467 (median 443) | `Monster` (all) | Mob template |
| 1724 | 16,605 | 9,074,632 | 85,831,261 | 391–435 (median 420) | `RealNPC` 16,599, `NPC` 6 | NPC template |
| 794 | 1,722 | 661,188 | 1,656,210 | 40–87 (median 80) | `GameplayContainer` (all) | Container template |
| 1653 | 69 | 246,537 | 631,757 | 804–811 | `Admin` | Avatar templates, administrator form (C22) |
| 1654 | 69 | 244,536 | 629,760 | 801–807 | `Player` | Avatar templates, player form (C22) |
| 4026 | 16 | 12,420 | 112,324 | 411–414 | `RealNPC` | Distant-battle unit groups (`DistantBattleUnit_*` properties) |
| 5785 | 6 | 3,294 | 55,257 | 424–434 | `RealNPC` | Neighbourhood NPCs (`NPC_Neighbor_*` properties) |
| 1725, 3074, 3075 | 1 each | 143 each | 4,464 / 4,444 / 4,484 | 385–387 | `Referee` | Placeholder referee objects |
| **Total** | **38,651** | | **204,486,468** | | | |

Ancestry (nearest first): 1723 and 1724 → 514 → 1729 → 510 → 790 → 1732; 794 → 1732;
1653 → 1730 → 512 → 1731 → 516 → 1729; 1654 → 1731 → 516 → 1729; 4026 → 514; 5785 → 5783 →
514; 1725 → 518 → 514; 3074 → 3070 → 518; 3075 → 3071 → 518. Above 1732 the chain is 695 →
1733 → 787 → 1726 → 488 → 223 → 89 → 485 → 484.

**Classes 1653 and 1654 (C22).** They are the avatar templates of `player-character.md` §4:
69 pairs (administrator form 1653, player form 1654). Every one of the 138 records is named by
exactly one **kind** of referrer (one of the three rows below), and each referring property
pair names one record of each class. A record can be named by several referrers of its kind:

| Referrer | Properties (player form / administrator form) | Records of each class |
|---|---|---|
| Race-table gender entry (class 860) | `RaceTable_Gender_PlayerAvatar` / `RaceTable_Gender_AdminAvatar` | 14 (15 references) |
| Monster-play record (class 1932) | `MonsterPlay_MonsterAvatar` / `MonsterPlay_MonsterAdminAvatar` | 54 (67 references) |
| PvP-control record (class 2708) | `PVPControl_Lobby_PlayerAvatar` / `PVPControl_Lobby_AdminAvatar` | 1 |

Multiplicity per avatar record (the same in both classes; counts below are per class, then
over all 138):

| Kind | Named by 1 referrer record | by 2 | by 3 | by 7 |
|---|---|---|---|---|
| Race-table gender entry | 14 (one of them by two gender entries of the same class-860 record: 15 references) | 0 | 0 | 0 |
| Monster-play record | 48 | 3 | 2 | 1 (48 + 6 + 6 + 7 = 67 references) |
| PvP-control record | 1 | 0 | 0 | 0 |
| **All 138 records** | **126** | **6** | **4** | **2** |

So 12 of the 138 records are named by more than one referrer record, and 2 more by two
references inside one referrer record. A reader must not assume that an avatar record has a
unique referrer.

No other property set names them, and no generator profile reaches them (§6.10). They belong to
player characters and monster-play characters, not to creatures; `player-character.md` §4
specifies them and how a character is built from them.

<a id="s2-2"></a>
### 2.2 Finding templates without hard-coding (rule)

Class indices are build-specific (`wstate.md` §11.1). A reader finds templates as follows:

1. Decode each 0x70 record's root class (offset 4) and its property set (DID + 0x09000000).
2. Read `WeenieType` (enum mapper 0x230000C9 with base 0x230001AB in this install; read the
   mapper the dictionary names). Label `Monster` marks a mob template, `RealNPC` or `NPC` an
   NPC template, `GameplayContainer` a container template.
3. The creature subtree root is the nearest common ancestor of the root classes found in
   step 2; every record whose root class descends from it is a template (this adds the
   player-like, referee and other classes of §2.1).

In this install `Monster` occurs only on class 1723, `RealNPC` on 1724, 4026 and 5785, `NPC` on
1724 (6 records) and `GameplayContainer` only on 794 (C2).

---

<a id="s3"></a>
## 3. Template layout

<a id="s3-1"></a>
### 3.1 The object record (0x70) (Confirmed, 38,651 records)

The record follows `wstate.md` §3 exactly. Contents observed in templates:

| Part | Mob, NPC and most other templates | Containers (794) |
|---|---|---|
| Imports | One entry: DB-object type 69, the attitude table's DID (§5.3), flags 16 (36,929 records). | None |
| Bundle | Item 0: the root script object (class = root class; eight reference attributes in class 1723, the first referencing item 1). Item 1: a DID item holding the attitude table's DID. 13 NPC records hold 7, 9 or 11 items (native containers and helper objects). | 1 item (1,656 records) or 6 (66) |
| Link section | The link schema of §3.3 (absent in 147 records: all of 1653, 1654, 1725, 3074 and 3075, and 6 of 1724) | §3.3 (absent in 2 records) |
| Available-property section | The instance-property list of §3.4 | §3.4 |

In all 36,929 records with an import, the import, the DID item and the property
`Attitude_Table` name the same DID. A reader takes every gameplay value from the property set.

**Root-object references (C23, all 38,651 root objects).** Attributes are identified by
their name hashes (`wlib.md` §8.2), quoted as interoperability identifiers:

| Root class | Reference attributes | Non-null |
|---|---|---|
| 1723, 1724, 5785, 1725, 3074, 3075 | 8 | 0x021101A5 → the DID item (the attitude table) in every record; 0x04343517 → an embedded class-701 object in 13 class-1724 records; the other six null in every record |
| 4026 | 10 | 0x021101A5 → the DID item; 0x09C27143 and 0x0B268289 → native class-17 objects; all three in all 16 records; the other seven null |
| 794 | 3 | 0x04343517 → an embedded class-701 object in 66 records; the other two null in every record |
| 1653, 1654 | 72, 70 | 21 (1653) and 20 (1654) attributes non-null in all 69 records each, one of them the DID item, the others embedded objects of the avatar machinery (`player-character.md` §4); the rest null in every record |

The 13 + 66 records with a class-701 object are the records whose bundle holds more than
the root object and the DID item (§3.1 table above).

<a id="s3-2"></a>
### 3.2 The property set (0x79) and its authored subset

A template's property set is an ordinary top-level property set (`property-system.md` §5).
It is large because it carries, besides the values the content author chose, **derived
copies** and **class-wide values** written into every record.

<a id="s3-2-1"></a>
#### 3.2.1 Mirror properties (Confirmed, C4)

A property is a **mirror** when its name matches one of the patterns below; its **base** is
the property named by removing the pattern's affix.

| Pattern | Base | Occurrences in the 38,651 sets | Equal to base | Base absent | Different |
|---|---|---|---|---|---|
| `Forward_Cached_`*X* | *X* | 1,338,192 | 1,338,192 | 0 | 0 |
| *X*`_Cached` | *X* | 3,848,272 | 3,737,347 | 110,925 | 0 |
| `ForwardSource_`*X* | *X* | 723,173 | 649,039 | 74,133 | 1 |
| *X*`_ForwardSource` | *X* | 443,838 | 443,838 | 0 | 0 |
| *X*`_Shadow` | *X* | 77,302 | 77,302 | 0 | 0 |
| *X*`_Base` | *X* | 77,302 | 77,302 | 0 | 0 |

- The base is absent for eight names only: `Conjunction_TableOverride_Cached`,
  `Combat_TacticalMastery_Class_Unified_Cached`, `Combat_PhysicalMastery_Class_Unified_Cached`,
  `ForwardSource_Movement_MountSpeedMultiplier` (36,929 sets each),
  `ForwardSource_Movement_SpeedMultiplier` (36,928), and three hunter and loremaster names
  (138 each). The single difference is in the only template that holds
  `Movement_SpeedMultiplier` at all (an NPC template, class 1724): its
  `ForwardSource_Movement_SpeedMultiplier` is 1.0 while the base is 2.0. A store must
  therefore keep that mirror (Decision D1 below).
- For seven of the eight base-less mirror names the base is not a property at all: its name
  has no definition among the 27,546 in the property dictionary (`property-system.md` §2),
  so no lookup can name it. Each of these seven mirrors holds one and the same value in every
  set that has it. The 138 sets with the hunter names hold both base-less mirrors of one base
  (`ForwardSource_` and `_Cached`), always with equal values.
- Only `ForwardSource_Movement_SpeedMultiplier` has a base with a dictionary definition
  (`Movement_SpeedMultiplier`, which has a dictionary default). In the 36,928 sets that hold
  the mirror without the base, the mirror is 1.0 in 36,925 and another value in 3 (two sets
  above 1.0, one below). The dictionary default, like every value here, is read from the
  install at run time.
- Mirrors are 40.2 % of all top-level properties of the 38,651 sets (6,508,079 of
  16,193,558). Every set of classes 1723, 1724, 4026, 5785 and the three referee classes
  carries exactly 175 mirrors (the same 175 names), every set of 1653 and 1654 carries 355, and
  every container set (794) 12.

A reader treats a mirror as derived: it keeps the base and recomputes the mirror on demand.
A mirror never stands for its base: when a set holds a mirror but not its base, a lookup of
the base answers as if no mirror were there (Decision D1, the last step of the lookup in §3.2.2).
Reason: the one template that holds both `Movement_SpeedMultiplier` and its `ForwardSource_`
mirror shows that the two can differ, so the mirror is not a reliable copy of the base.

> **OPEN:** Whether the original server treats a base-less
> `ForwardSource_Movement_SpeedMultiplier` (the three sets where it is not 1.0) as the
> creature's speed factor. The data cannot show it, and neither the client nor its class
> library refers to the mirror directly: the executable holds its property ID only as a UI
> element ID (C25), and the class library's code pushes the number once, as a DID-map key,
> never to read a property. (A generic mechanism that handles `ForwardSource_` properties by
> name or by dictionary data would not show in that search; none was looked for.) The answer
> most likely lies with the server, which no M1–M4 method reaches. Under D1 the base answers its dictionary default and the
> mirror keeps its own value.

> **OPEN:** (implementer note, `crates/formats/src/gameplay/creatures.rs`) Which pattern
> applies when a name matches two of the six (for example `ForwardSource_`*X*`_Cached`) is
> not stated. Tessera takes the first in table order, so such a name is a `_Cached` mirror of
> `ForwardSource_`*X*, and a lookup may step through two mirrors (at most 8 steps). Confirm or
> give the rule.

<a id="s3-2-2"></a>
#### 3.2.2 Class-constant and varying properties (Confirmed, C5)

For each root class, a non-mirror property is **class-constant** when it is present in every
record of the class with one and the same value; otherwise it is **varying**. The varying
properties form the **authored subset**.

| Class | Distinct PIDs | Mirrors | Class-constant | Varying | Varying per record (min / median / mean / max) |
|---|---|---|---|---|---|
| 1723 (mobs) | 610 | 175 | 200 | 235 | 65 / 68 / 69.2 / 92 |
| 1724 (NPCs) | 545 | 175 | 190 | 180 | 26 / 55 / 55.7 / 70 |
| 794 (containers) | 115 | 12 | 16 | 87 | 12 / 52 / 48.1 / 59 |
| 1653 | 817 | 355 | 425 | 37 | 24 / 24 / 24.3 / 31 |
| 1654 | 812 | 355 | 423 | 34 | 23 / 23 / 23.2 / 29 |
| 4026 | 414 | 175 | 230 | 9 | 6 / 7 / 7.7 / 9 |
| 5785 | 435 | 175 | 235 | 25 | 14 / 21 / 19.5 / 24 |

So a median mob set of 443 properties holds 175 mirrors, 200 class-constants and 68
authored values.

**Rule for a template store (Decision D1).** Store per class the class-constant properties
once; store per template only the varying properties present in its set. A mirror present
in *T*'s set is **derived** (not stored) only when its base is also present in *T*'s set and
the two values are equal. Every other mirror present in the set, that is one whose base is
absent or whose value differs from the base's, is stored for *T* like a varying property
(in this install: the 185,058 base-absent occurrences and the one differing occurrence of
§3.2.1). A lookup of property *P* on template *T* answers, in order: *T*'s own stored value
(varying property or stored mirror); the class-constant value; for a mirror that *T*'s set
holds but the store derived, the base's value by the same lookup; otherwise absent (the
dictionary default applies as in `property-system.md` §2.2). These four steps are the whole
lookup: in particular, a base that *T*'s set does not hold and that is not class-constant
answers absent even when *T* stores a mirror of it; the lookup never derives a base from a
mirror (in this install this concerns `Movement_SpeedMultiplier` in 36,928 templates, §3.2.1,
and §12.1 shows the case). The store records, per
template, which derived mirrors its set holds, so that a mirror the set does not hold is
reported absent and not computed. With these rules the store gives back every template's
set exactly (CR13). The classification is
recomputed from the install when the store is built, never hard-coded, because a content
update can turn a constant into a varying property.

<a id="s3-3"></a>
### 3.3 Link schema (wstate.md §6) for templates (Confirmed, C3, C6)

The object record's link section lists the link kinds an instance of the template may take
part in. Each entry is (direction, link type, limit, flag, PIDs carried by such links,
terminator byte); one row of the table below is one entry. The terminator byte is 0 in every
type-13 entry and 3 in every other entry (`wstate.md` §6). Observed in class 1723 (20,161
records):

| Direction | Link type (`landblock.md` §4.6) | Limit | Flag | PIDs | Records |
|---|---|---|---|---|---|
| 0 | 12 Script | 250 | 0 | none | all |
| 1 | 12 Script | 250 | 0 | none | all |
| 0 | 8 Generator | 1 (19,517), 20 (639), 10 (4), 5 (1) | 1 with limits 1 and 10, 0 with 20 and 5 | none | all |
| 0 | 2 Patrol | 100 | 0 | `LinkName`, `PatrolLink_Speed` | all |
| 0 | 1 PatrolOnce | 100 | 0 | `LinkName`, `PatrolLink_Speed` | all |
| 0 | 19 AIRemoteDetector | 4 (20,156), 99 (5) | 0 | three PIDs | all |
| 1 | 13 | 99 | 0 | none | all (terminator 0) |
| 0 | 13 | 99 | 0 | none | all (terminator 0) |
| 1 | 0x10000019 Phasing | 1 | 0 | none | 1,217 |
| 0 | 11 FlyingPatrol | 100 | 0 | `LinkName`, `PatrolLink_Speed` | 111 |

Class 1724: 16,095 records carry {0/8 limit 1, 1/12 limit 99, 0/12 limit 99, 0/2 and 0/1
limit 100, 1/0x10000019 limit 1}; 486 carry subsets of it, 14 only 0/8 with flag 1, 4 are
variants (script limits 25 or 250, one without a Generator entry, one with a FlyingPatrol
entry), and 6 have no section. Containers (794): 0/8 limit 1 (1,698 records) or 20 (22);
0/12 and 1/12 limit 250 in 1,385; 1/0x10000019 limit 1 in 3 of those; 335 records hold the
0/8 entry alone and 2 have no section. Class 4026 (16 records) carries exactly the
eight entries that every class-1723 record has, with the 0/8 entry at limit 1, flag 1 and the
type-19 entry at limit 4; class 5785 (6)
exactly the 16,095-record set of 1724. Type-13 entries occur only in classes 1723 and 4026,
always as the pair of rows above (direction 1 stored before direction 0). The generator class (1113, 50 records) carries {1/8 limit 250, 0/7 limit 100, 0/2
limit 100} in 30 records, the same with limits 0 for types 8 and 2 in 4, and no section in 16.

**Direction.** On every landblock link whose ends can be typed, an object whose schema has
direction 0 for a link type is that link's **source** end (`end_id_2`, the third u64 of the
link) and direction 1 its **target** end (`end_id_1`, the second u64): generators are the
source of type-7 links and the target of type-8 links, creatures and spawned objects the
source of type-8 and type-2 links (§6.3). The limit is the most links of that type the object
may have at that end. Landblock instance sets that are sources of type-8 links have 1 such
link (207,697 sources), 2 (10,289) or up to 20; the template of such a source is not in the
data (§6.10), so the limit cannot be checked against its class.

**Link type 13 (C24).** Code 13 is the key labelled `PathMapTest` of the link-type
enumeration (enum mapper 0x230000BE with base mapper 0x230001A9; the label is quoted as an
interoperability name). No landblock link uses it, so a server or tool that builds links
from the landblocks never meets it; a byte-exact writer keeps the two type-13 entries.

> **OPEN:** The meaning of `flag` (set only on Generator entries in templates: limits 1 and
> 10 in mobs, 14 NPC records). `wstate.md` §6 finds that the offline client only stores the
> flag and that no method of the object record reads the link section after loading, so the
> meaning belongs to the original server or authoring tool; no M1–M4 method reaches it. Not
> needed to read or place templates.

<a id="s3-4"></a>
### 3.4 Instance-property list (wstate.md §7) for templates (Confirmed)

38,488 templates carry the available-property list; 163 do not (all of 1653, 1654, 1725,
3074, 3075, 20 of 1724 and 2 of 794). Lengths: 46 (18,583 records), 34 (16,077), 33 (1,637),
49 (1,336), 29 (335), 23 (234), 42 (171), 50 (58), 35 (27), 47 (22), 53 (4), and 10, 51, 54, 55
once each. The union over the subtree is 123 PIDs, for example `Name`, `AI_WanderingRange`,
`AI_PhysicallyWanders`, `AI_DefaultPatrolName`, `AI_PerceptionRadius`, `NPC_StartingIdleAnimation`,
`QuestDispenser_RoleConstraint`, `Drama_DramaList`, `Generator_OwnerID`. Many of them are
the properties that landblock instance sets give spawned objects (§6.2). In 38,487 of the 38,488
templates some listed PIDs are present in the template's own set; in one, none.

Over all 199,295 object records with the list, 544 distinct PIDs are listed. The 514,987
instance sets without a static entity use 523 distinct top-level PIDs; 251 of them are listed
by some class, and those carry 85 % of the property occurrences. Generic `Render_*`,
`Physics_*` and `Entity_*` properties are set per instance without being listed.

> **OPEN:** Whether the original server or authoring tool enforces the list (a whitelist of
> per-instance properties) or treats it as advisory. The offline client does not consult it:
> no method of the object record reads the section after loading (`wstate.md` §6 "Client",
> §7). Tessera treats it as advisory: instance properties are applied whether listed or not
> (Decision D2).

---

<a id="s4"></a>
## 4. Template properties by role (Confirmed census, C7)

Counts are records of class 1723 (mobs) / 1724 (NPCs) that contain the property; "all" means
every record. DID targets are given as prefix, or as "0x70 class *n*" for object records.
Null DIDs (0) mean "none". Mirrors (§3.2.1) are not listed.

<a id="s4-1"></a>
### 4.1 Identity and classification

| Property | Type | Mobs / NPCs | Values |
|---|---|---|---|
| `Name` | StringInfo | all / all | String-table reference (`string-table.md` §9); 20,096 distinct in mobs |
| `WeenieType` | enum 0x230000C9 | all / all | §2.2 |
| `Agent_Species` | enum 0x2300000B (123 keys) | all / all | 109 distinct in mobs; NPCs: one placeholder value in 16,603 |
| `Agent_Subspecies` | enum 0x2300000C (768 keys) | all / all | 675 distinct in mobs; NPCs: one placeholder value in all |
| `Agent_ShowSubspecies` | boolean | all / all | true in 4,122 mobs |
| `Agent_Genus` | bitfield32, mapper 0x2300000A (18 keys) | all / all | Exactly **one** bit set in all 36,929 sets of mobs, NPCs, 1653, 1654, 4026, 5785 and the referees; 17 different bits occur |
| `Agent_Alignment` | enum 0x2300000D (6 keys) | all / all | Mobs: key 3 14,649, key 2 3,453, key 1 2,059. NPCs: key 2 in 16,603 |
| `Agent_Class` | enum 0x2300000E (212 keys) | all / all | 16 distinct in mobs; NPCs: one placeholder value in 16,604 |
| `Agent_Classification` | DID → 0x70 class 973 | all / all | §5.4; 115 distinct in mobs, 3 in NPCs |
| `Agent_ClassificationFilter` | enum 0x2300000F (7 keys) | all / 16,599 | Mobs: `Monster` 17,856, `Victim` 1,466, `DelvingMonster` 740, `Guard` 99. NPCs: `Undef` 16,502 |
| `Quest_MonsterDivision` | enum 0x23000061 (592 keys) | all / — | 521 distinct; groups mobs for quest objectives |
| `MonsterLevel_ExaminationModStatType` | enum 0x23000070 (35 keys) | — / 16,599 | Examination rank; **absent in every mob template**; 11 distinct in NPCs |
| `Entity_CreationType` | enum 0x23000174 | all / 16,599 | Mobs: key 7 in 19,984, key 8 in 177 |
| `Agent_NotAlive_Name` | StringInfo | 4 / 2 | Name shown when dead |

<a id="s4-2"></a>
### 4.2 Presentation

| Property | Type | Mobs / NPCs | Values |
|---|---|---|---|
| `PhysObj` | DID → 0x47 | all / all | 658 / 517 distinct; resolve with `setup.md` §3.1 |
| `Appearance_ClassList` | array of `Appearance_KeyInfo` {`Appearance_Key`, `Appearance_AprFile` → 0x20, `Appearance_Modifier`} | 13,510 / 10,104 | 1–11 entries; `appearance.md` §7 |
| `Appearance_RandomAppearanceType` | enum 0x2300031D (440 keys) | all / 16,502 | Non-zero in 3,909 mobs |
| `Render_AdjustableScale`, `Physics_AdjustableScale` | vector | 13,120 / 16,584–16,590 | Uniform scales; 126 distinct in mobs. The two differ in 47 templates (render larger, for example 1.5 against 1.3) and only one is present in 6 |
| `Inventory_Cosmetic_LoadoutSet` | array of DID → 0x70 class 1198 | 7,699 / — | 2–10 loadouts |
| `Inventory_Cosmetic_Slot`*n*`_Entity`, `_HoldingLocation`, `_Scale`, `_Behavior`, offsets | DID → 0x47, enums, vector, floats | slot 1: 2,546, slot 2: 1,448, slot 3: 14 | Held props |
| `Material_TorsoMaterial`, `_BootMaterial`, `_OffHandMaterial`, `_WeaponMaterial` | enum 0x230000F3 (54 keys) | all / — | Impact-sound materials |
| `Combat_AnimationSet` | enum 0x2300010C (45 keys) | all / 22 | Non-zero in 3,894 mobs |
| `Combat_FXSet` | enum | 19,990 / — | |
| `Snd_Monster_Type`, `VO@NPC.m_eDemeanor`, `VO@NPC.m_eRace` | int, enum 0x23000162, enum | 118, all, 565 / 4, 16,583, 5,382 | Voice and creature sound selection |
| `AC_Color_*`, `AC_Texture_*`, `AC_BoneScale_Test` | | ≤ 9 / ≤ 123 | Character-creation style values on humanoid NPCs |

<a id="s4-3"></a>
### 4.3 AI

| Property | Type | Mobs / NPCs | Values |
|---|---|---|---|
| `AI_ActionTable_Array` | array of `AI_ActionTable_Array_Element` {`AI_ActionTable` DID, `AI_ActionTable_MinLevel` int, `AI_ActionTable_MaxLevel` int} | all / — | 10–16 elements in every mob (206,050 in all). `AI_ActionTable`: → 0x70 class 534 in 50,890, null in 155,158, → class 884 in 2 (an authoring error; ignore such entries). Min level −1 in 197,911 elements (else 1–65); max level −1 in 205,925 (else 0–100). −1 means "no bound" (Reported, LC, which treats every value ≤ 0 so; one max level is 0). §5.1 |
| `Attitude_Table` | DID → 0x70 class 884 | all / all | 118 / 6 distinct; §5.3 |
| `AI_PerceptionRadius` | float, world units (metres, `coordinates.md`) | all / 16,599 | Mobs 0–120 (7 in 13,325); NPCs always 0 |
| `AI_MovementType` | enum 0x23000021 | all / — | `ConstrainedToHome` 19,288, `FreeRange` 409, `Stationary` 296, `TurnOnly` 168 |
| `AI_CallForHelp_Radius` | float | all / — | 0–60 (7 in 19,495) |
| `AI_CallForHelp_RespondTypes` | bitfield32, mapper 0x23000023 | all / — | 13 distinct |
| `AI_CallForHelpFilter` | DID → 0x70 class 977 | 48 non-null / — | A classification-filter list (§5.3) |
| `AI_Hints` | any-width bitfield, mapper 0x2300005C (176 keys) | all / — | 92 distinct; matched by action requirements (§5.1) |
| `AI_ColorEventTable` | DID → 0x70 class 539 | 1,810 non-null / — | §5.5 |
| `Formation_Group_DataID`, `Formation_Layout_Type` | DID, enum 0x2300036D | 19,990 / 1 | DID → class 3477 in 272, → class 1723 in 2 (error), null otherwise; §5.6 |
| `AIPuppet_TargetEvalPackage`, `AIPuppet_TargetFlags`, `AI_IsPuppetNPC` | DID → class 3761, bitfield 0x230003B8, boolean | 352 non-null, 536, 540 / — | §5.8 |
| `AI_KungFu_StartAsNPC`, `AI_KungFu_Repeatable`, `AI_KungFu_TransitionList` | boolean, boolean, array | 1,323, 1,848, 22 / 7, 1, — | Start peaceful and turn hostile on a trigger |
| `AI_PhysicallyWanders`, `AI_WanderingRange`, `AI_WanderingProb` | boolean, float, float | 114, 37, 11 / 23, 19, — | Rare on templates; normally set per instance (§6.2) |
| `GameEvent_CombatEventTable`, `GameEvent_ConjunctionEventTable` | DID → 0x0C | 589, 55 non-null / — | Scriptlets |

<a id="s4-4"></a>
### 4.4 Combat flags

| Property | Mobs / NPCs | Values |
|---|---|---|
| `Combat_CanBlock`, `Combat_CanParry`, `Combat_CanEvade` | all / — | true 20,003, false 158 (the same records for all three) |
| `CombatState_CurrentStateImmunities` | all / — | bitfield32, mapper 0x23000050; non-zero in 361 |
| `Death_NeverSayDie` | all / — | true in 5 |
| `Combat_NotAttackable` | 21 / 16,598 | |
| `Health_Starting_Level` | 14 / — | Fraction 0.01–0.75 |

<a id="s4-5"></a>
### 4.5 Loot references (targets decoded in the loot specification)

| Property | Mobs | Target |
|---|---|---|
| `LootGen_TrophyList_Override` | 1,949 non-null of 20,161 | 0x70 class 1237 |
| `LootGen_TreasureList_Override` | 45 non-null | 0x70 class 1236 |
| `LootGen_BarterTrophyList` | 333 non-null | 0x70 class 1917 |
| `LootGen_ReputationTrophyList` | 1,088 non-null | 0x70 class 1918 |
| `LootGen_FilteredTrophyTable` | 157 non-null of 790 present | 0x70 class 3666 |
| `LootGen_GeneratesTrophies` | all (true 12,514) | boolean |
| `Loot_IsRemoteLootable`, `Tapping_OpenTapping` | 19,990 each (true 19,134) | booleans |

Mobs without an override receive species-level loot (Reported, LC: a table keyed by species,
subspecies and examination rank). Containers carry `LootGen_TreasureList_Override` (271
distinct), `LootGen_TrophyList_Override` (782 distinct) and soft-lock variants.

<a id="s4-6"></a>
### 4.6 Quest, faction and service roles

| Property | Mobs / NPCs | Values |
|---|---|---|
| `QuestDispenser_ActiveQuests` | 664 / 9,553 | Array of `QuestDispenser_QuestRole` {`QuestDispenser_Quest` → 0x70 class 1398, `QuestDispenser_RoleName` string}. Non-empty in 573 / 8,703; 2,232 / 40,460 roles; up to 334 per NPC |
| `QuestDispenser_RoleConstraint` | 3 / 45 | String naming a role; also set per instance (14,894 instance sets) |
| `Quest_RelatedQuest` | 536 (373 non-null) / — | → class 1398 |
| `Reputation_PositiveFaction`, `Reputation_NegativeFaction` | 745, 39 / — | → class 1889 (2 negative → 1346) |
| `NPCComment_DIDCommentArray` | 20 / 404 | → class 2247 |
| `OccupationTitle`, `MapNote_Type`, `MapNote_Enabled`, `MapNote_StringInfo` | — / 16,584 | NPC title and map-note role (`MapNote_Type` bitfield64, 23 distinct) |
| `Vendor_*`, `Barter_ProfileArray`, `Repair_CostModifier` | — / 1,439, 1,084, 1,296 | Vendor services |
| `TravelWebWC`, `TravelPad_Location`, `Travel_*` | — / 437 | Stable-masters |
| `SkillTrainer_Class`, `Usage_RequiredClassList` | — / 266, 267 | Trainers |
| `NPC_Is_Banker`, `NPC_Is_AccountBanker`, `NPC_Is_WardrobeBanker`, `NPC_Is_Auctioneer`, `NPC_Is_Notary`, `NPC_Is_Trait_Manager` | — / 28, 26, 26, 24, 9, 31 | Service flags |
| `Induction_ActionDID` | 5 / 16,583 (1,153 non-null) | → class 1195 |

<a id="s4-7"></a>
### 4.7 Effects, scripts and use

| Property | Mobs / NPCs | Values |
|---|---|---|
| `Effect_MonsterStartupEffect_Array` | 5,832 / 299 | Array (0–204 entries) of `Effect_MonsterStartupEffect_Struct` {`Effect_StartupEffectID` → effect classes 708, 709, 713, 718, 725, 734, 2063, 3686, 3842 and others; optional `Effect_StartupEffectSpellcraft` float 0.3–160; optional `Effect_StartupEffectDamageQualifier`} |
| `Effect_MonsterInitEffect_Array` | 557 / — | Same element struct |
| `Entity_AdditionalScriptTables` | all / 16,591 | Array of 1–9 `ScriptTable_DBScriptTable` → 0x07 or null (`script-table.md`) |
| `Usage_Action` | all / all | → 0x70 classes 1577, 1578 (19,804 mobs), 1580 (15,295 NPCs), 1589 and others |
| `CSM_ActionHandler` | all / 16,599 | → class 1538 (20,160 mobs) or 1539 |

<a id="s4-8"></a>
### 4.8 What templates do not contain (Confirmed C8; BIN C9)

- **No level.** `Advancement_Level` is absent from every mob and NPC template; it occurs on
  1,385 containers (values 0–160) and as 1 on classes 1653 and 1654. No template carries
  `MonsterLevel_*` stat tables.
- **No per-template vitals or mitigations.** `Health_MaxLevel` is 10 in every mob,
  `Power_MaxLevel` 0; the partial mitigation and resistance-type properties are
  class-constant (0) or mirrors.
- **No mob rank.** `MonsterLevel_ExaminationModStatType` is absent from every mob.
- **The level tables are not shipped.** The WEENIECONTENT entry `MonsterLevelRangeDirectory`
  names a record whose `MonsterLevelRange_LevelRangeList` holds 24 DIDs (root class 1347);
  all 24 property sets are empty and their bundles hold only the root object. The entry
  `MonsterLevelDirectory` names a record with an empty property set.

The client's own creature information panel (BIN) fills its fields from the entity's
properties and refreshes them when they change:

| Field | Property | When absent or of another type |
|---|---|---|
| Name | `Name` | |
| Level | `Advancement_Level` (int) | shown as **1** |
| Genus | `Agent_Genus` (label of the set bit) | a fixed "none" text when 0 |
| Species | `Agent_Species` (label) | a fixed "none" text when 0 |
| Health maximum | `Health_MaxLevel_Cached` (float) | 0 |
| Power maximum | `Power_MaxLevel_Cached` (float) | 0 |
| Rank | `MonsterLevel_ExaminationModStatType` (label) | a fixed "none" text when 0 |
| Alignment | `Agent_Alignment` (label) | a fixed "none" text when 0 |
| Immunity indicator 1 | `CombatState_CurrentStateImmunities` | set when bits 2 and 14 are both set |
| Immunity indicator 2 | same, and `Conjunction_Immune` | set when bit 8 is set; otherwise the value of `Conjunction_Immune` |
| Immunity indicator 3 | same | set when bit 9 is set |

Bit *i* of the immunity bitfield is labelled by key *i* + 1 of mapper 0x23000050. Level, rank
and vital maxima are therefore values the server sets on the instance; the templates do not
hold them.

**The window (C9a).** The panel is the **analyse-skill panel**: the routine fills a child of
the element `AnalyzeSkillComponent` (element ID 0x10001442), which lies inside the top-level
element `GamePlay_AnalyzeSkillPanel` (0x1000143F) of the layout under `UILAYOUT` key
0x10000051 (`gameplayactivatables`, record 0x22000012 in this install; `ui-layout.md` §3.2,
§9.1). A window method creates that element through the create-by-key entry of `ui-layout.md`
§9.1 when it first needs it, finds the component, fills it by the table above and shows the
window. Element names are quoted from the data (mapper 0x230000D1) as interoperability names.
The fill routine and its wrapper have no other caller, so no other window
uses this table. What makes the client call that window method was not followed (it may be
driven by server messages, which are excluded). The explorer card (§10.1) does not depend
on it.

---

<a id="s5"></a>
## 5. AI records (Confirmed census, C11)

All are object records with property sets; their bundles hold only the root object except
where stated.

<a id="s5-1"></a>
### 5.1 Action table (root class 534, 3,033 records)

One top-level property, `AIActionTable_Actions`: an array of 1–12 `AIActionTable_Action`
structs (14,300 in all):

| Field | Type | Present | Values |
|---|---|---|---|
| `AIAction_Chain` | array of `AIAction_ChainEntry` {`AIAction_Skill` DID, optional `AIAction_Recovery` float} | all | 1–5 entries (15,023 in all); every skill is 0x70 class 827; recovery 0–7 in 206 entries |
| `AIAction_Probability` | enum 0x23000024 (10 keys) | all | Converted by §5.2 |
| `AIAction_Cooldown` | float (time; unit OPEN) | 13,384 | −1 (725), 0.1–99,999 |
| `AIAction_InitialCooldown` | float (time; unit OPEN) | 2,896 | 1–720 |
| `AIAction_TargetCooldown` | float (time; unit OPEN) | 2,287 | −1 (202), 1–60,000 |
| `AIAction_CooldownChannel` | enum 0x2300006C (321 keys) | 1,305 | Shared cooldown group |
| `AIAction_RequiredHints`, `AIAction_DisallowedHints` | any-width bitfield, mapper 0x2300005C | 3,502, 4,463 | Conditions on the actor's hints |
| `AIAction_TargetRequiredHints`, `AIAction_TargetDisallowedHints` | same | 2,157, 1,971 | Conditions on the target's hints |
| `AIAction_CustomTarget` | enum 0x23000118 (35 keys) | 3,503 | Target selector |
| `AIAction_ColorEventType` | enum 0x23000117 (25 keys) | 2,337 | Colour event raised with the action (§5.5) |

A template's `AI_ActionTable_Array` names up to 16 tables, each with an optional level range.

> **OPEN:** How the original server chooses among the tables and actions (all eligible tables
> merged or the first matching one; probability as a per-decision chance or as a weight; the
> role of −1 cooldowns; the time unit of the cooldowns and recovery, seconds being likely).
> The data hold the tables but no selection rule; neither the executable (C25) nor the class
> library's code (C26) reads the action-table properties, so the client does not run mob AI
> and no M1–M4 method reaches the rule. Decision D3 (§6.8) gives Tessera's sandbox rule.

<a id="s5-2"></a>
### 5.2 Probability table (Confirmed C12)

The WEENIECONTENT DID map (`property-system.md` §8.1) labels one entry `AIActionProbability`
(0x70000267, root class 1655). Its property `AIActionProbability_Array` holds nine
`AIActionProbability_Struct` {`AIActionProbability_Type` enum 0x23000024,
`AIActionProbability_Value` float}. The value is a percentage; the key labelled `Zero` maps to
0 and the key labelled `Infinite` maps to −1. Key 0 (`Undef`) has no entry. A reader builds the
mapping from the install and must not hard-code it.

<a id="s5-3"></a>
### 5.3 Attitude table (884, 124 records) and classification filters (977)

Top level: `Attitude_FriendlyFilterList` (118 records) and `Attitude_AggressiveFilterList` (114);
110 have both and 2 have an empty property set. Each list is an array of 1–5 `ClassificationFilter_Filter`:

| Field | Type | Present | Meaning (labels from mapper 0x2300000F) |
|---|---|---|---|
| `ClassificationFilter_Filter_Type` | enum 0x2300000F | all | Which group of entities the filter selects (`MonsterPlayer`, `Monster`, `Victim`, `Guard`, `Player`, `DelvingMonster`), compared with the other entity's `Agent_ClassificationFilter` |
| `ClassificationFilter_Filter_Default` | boolean | 278 aggressive / 72 friendly filters | Stored value true in all 350 (and in the one class-977 filter) |
| `ClassificationFilter_FilterExceptionList` | array of `ClassificationFilter_FilterException` {optional `…_Alignment`, `…_Genus` (bitfield), `…_Species`, `…_Subspecies`} | 122 / 193 filters | 1–10 exceptions |

The one record of class 977 has the same filter-list shape under `ClassificationFilter_FilterList`.
The bundle of an attitude record holds the root object plus native containers (classes 35,
105) and helper objects (974, 975), 1,890 items over the 124 records.

No list names the same `ClassificationFilter_Filter_Type` twice (all 233 lists: 114
aggressive, 118 friendly, 1 in class 977). Filters by shape (C28):

| List | `Default` true, no exceptions | `Default` true, exceptions | `Default` absent, exceptions | `Default` absent, no exceptions |
|---|---|---|---|---|
| Aggressive (365 filters) | 176 | 102 | 20 | 67 |
| Friendly (286 filters) | 62 | 10 | 183 | 31 |
| Class 977 (1 filter) | 1 | 0 | 0 | 0 |

Exceptions (549) by the fields they hold: species only 246; alignment only 113; genus only
112; genus and alignment 31; species and subspecies 28; genus and species 14; species and
alignment 5. No exception holds a subspecies without a species, and none holds no field.

<a id="s5-3-1"></a>
#### 5.3.1 How an attitude table judges another entity (Confirmed C27–C30)

The class library (record 0x56000000) carries the rule as script code; it was read with
`wlib-bytecode.md`. The code calls native functions for three steps (find the filter for a
type in a map, test whether a key set is empty, test whether it contains a key); those
meanings are inferred from how the code uses them (C27); the map and sets they work on are
confirmed against the data (CR17). Two entities are involved: the **judge**, whose attitude table *T* is used,
and the **other** entity *B*, described by five values: filter type *f* (mapper 0x2300000F),
genus *g* (the genus bitfield value, mapper 0x2300000A), species *s*, subspecies *u* and
alignment *a*. The result is one of three codes, numbered like keys 1–3 of mapper 0x23000112:
1 `Neutral`, 2 `Friendly`, 3 `Aggressive`.

**Table.** In this order:

1. If *f* is 0, the result is Neutral.
2. If *T* has an aggressive list and that list **selects** *B* (below), the result is
   Aggressive. The friendly list is not consulted.
3. Otherwise, if *T* has a friendly list that selects *B*, the result is Friendly.
4. Otherwise the result is Neutral. (A table with an empty property set is therefore Neutral
   towards everyone.)

**List.** A list selects *B* when it has a filter whose `ClassificationFilter_Filter_Type`
equals *f* and that filter selects *B*. A list without a filter for *f* does not select *B*.

**Filter.** Let *D* be the filter's `ClassificationFilter_Filter_Default`: its stored value
when present (true in all 351 present values of this install), and **false when absent**. The filter selects *B* exactly when the following gives true:

1. **Genus from species.** If *s* is not 0, look *s* up in the species-to-genus table
   (below). If the table has no genus for *s*, the answer is *D*. If *g* is not 0 and differs
   from the table's genus, the answer is *D*. Otherwise *g* becomes the table's genus.
2. **No exceptions.** If the filter has no exceptions, the answer is *D*.
3. **Exceptions.** Form the seven keys (rule below) of (genus, species, subspecies,
   alignment) = (*g*, *s*, *u*, *a*), (*g*, *s*, *u*, 0), (*g*, *s*, 0, *a*), (*g*, *s*, 0, 0),
   (*g*, 0, 0, *a*), (*g*, 0, 0, 0) and (0, 0, 0, *a*). If any of them equals the key of one of
   the filter's exceptions, the answer is **not** *D*. Otherwise it is *D*.

So an exception reverses the filter's default for the entities it describes: with `Default`
true the filter selects every entity of type *f* except the excepted ones; with `Default`
absent it selects only the excepted ones; with `Default` absent and no exceptions it selects
no one (67 aggressive and 31 friendly filters in this install).

**Key.** A key is a 64-bit unsigned value: bits 32–63 hold the genus bitfield value, bits
20–31 the species, bits 4–19 the subspecies and bits 0–3 the alignment. If the species is
above 4,095, the subspecies above 65,535 or the alignment above 15, the key is 0 instead (the
library also logs an error); no stored key is 0. An exception's key is built from its
optional `…_Genus`, `…_Species`, `…_Subspecies` and `…_Alignment` (absent = 0), after the
same genus-from-species step: when the exception has a species, its genus is the table's
genus for that species. In this install every exception that holds both a genus and a
species (14) holds the species' genus, so the step changes only exceptions without a genus.

**Exception species without a genus.** If an exception's species has no entry in the
species-to-genus table, no genus can be taken from the table. This happens for **0** of the
549 exceptions in this install (all 293 exceptions that hold a species name a species of the
table). Such an exception can never change a result, whatever genus its key carries: an
entity *B* whose species is the same has no table genus either, so step 1 above answers *D*
before the exceptions are consulted, and every key formed for any other *B* holds a different
species (another species, or 0). A reader may therefore drop such an exception, or keep it
with its own `…_Genus` (absent = 0); both give the same results. (The argument needs the
exception's key to be non-zero; the key rule gives 0 only for out-of-range values, and no
exception of this install yields one, since all 549 built keys equal the stored non-zero keys,
C28.) Which of the two the original's stored object form would hold is not known and not
needed.

**Species-to-genus table (C29).** The WEENIECONTENT DID map (`property-system.md` §8.1)
labels one entry `ClassificationControl` (root class 1658 in this install). Its property
`Classification_Control_List` lists classification records (class 973, §5.4; 116 of the 124).
The table maps each listed record's `Classification_Species` to its `Classification_Genus`
(116 entries, one species each). A reader builds it from the install and must not hard-code it.

**Object form (C28).** The class library's code reads the attitude record's object form,
which a reader of the property set does not need. Attribute name hashes are quoted as
interoperability identifiers:

| Object | Attribute | Holds |
|---|---|---|
| Root object (class 884) | 0x015B4D65 | Reference to the aggressive list (class 974), null without one |
| Root object (class 884) | 0x08B3DC39 | Reference to the friendly list (class 974), null without one |
| Class-977 root object | 0x07C90C42 | Reference to its one list (class 974) |
| List (class 974) | 0x0D14BE23 | Reference to a native class-35 map: filter type → reference to a filter (class 975) |
| Filter (class 975) | 0x0ABBBF54 | Integer: 1 when `…_Filter_Default` is true, 0 when it is absent |
| Filter (class 975) | 0x04E11F23 | Reference to a native class-105 set of 64-bit exception keys (empty when the filter has none) |

In every one of the 233 lists the object form equals the property set under the key rule
above.

**Where the values come from.** In the class library the judge's table is the record named
by its `Interaction_Permission_AttitudeTable`, and *B*'s five values are its
`Interaction_Permission_ClassificationFilter`, `…_Genus`, `…_Species`, `…_SubSpecies` and
`…_Alignment`. The library's harmful-interaction check evaluates both directions: the
actor's table against the target, and the target's table against the actor.

**Decision D8 (Tessera sandbox).** No template holds an `Interaction_Permission_*` property,
and no script of the class library writes one. The sandbox gives every entity the values of
its own `Attitude_Table`, `Agent_ClassificationFilter`, `Agent_Genus`, `Agent_Species`,
`Agent_Subspecies` and `Agent_Alignment` in their place.

> **OPEN:** How the original fills an entity's `Interaction_Permission_*` values (from its own
> `Agent_*` values and `Attitude_Table`, from a master's for pets, or otherwise). They are set
> at run time and not by script code; D8 is Tessera's choice. Also not read: how the library's
> harmful-interaction check combines the two directional results with its other inputs
> (player-versus-player state, teams, duels, passivity, invulnerability). Neither is needed
> to evaluate a table.

> **OPEN:** (implementer note) The exception fields are named here only as `…_Alignment`,
> `…_Genus`, `…_Species` and `…_Subspecies`, and §6.5 names the repeat and placement fields
> only as `…_RepeatMinTimes`, `…_RepeatMaxTimes`, `…_ExactPlacementX` (`Y`, `Z`),
> `…_RandomPlacementMinDist` and `…MaxDist`. Tessera resolves each as the one dictionary label
> that starts with `ClassificationFilter` (respectively `GeneratorProfile`) and ends with the
> given suffix, and treats the field as absent when no label or several match; on the
> 2026-10-07 install every one resolved to exactly one label. Please give the full labels.

<a id="s5-4"></a>
### 5.4 Classification (973, 124 records)

Exactly three properties in every record: `Classification_Species` (mapper 0x2300000B),
`Classification_Genus` (bitfield, mapper 0x2300000A, one bit set in all 124) and
`Classification_Alignment` (mapper 0x2300000D). For every template that names a
classification (36,929), its own `Agent_Species`, `Agent_Genus` and `Agent_Alignment` equal the
classification's three values (C10). A reader may therefore take either; a disagreement is a
data error to report, not a value to choose.

<a id="s5-5"></a>
### 5.5 Colour-event table (539, 883 records)

`AIColorEventTable_Events`: an array of `AIColorEventTable_Event`. Every one of the 883
records holds exactly 22 events in it, one for each of the keys 1–22 of mapper 0x23000117
(19,426 array events in all). One record **also** holds a single `AIColorEventTable_Event`
as a second top-level property, next to its full array: its type is one of the 22 already in
that array, and its content differs from the array event of the same type. All counts in
this section are of the **19,426 array events only**; counting the top-level event gives
19,427 events, 97,138 data entries and 1,390 sounds (it adds 8 data entries, each with a
sound, and its `_Probability` is 1.0, `_SoundChannel` 4 and `_SayType` present).

Event fields: `AIColorEventTable_Event_Type` (enum 0x23000117), `AIColorEventTable_Event_Probability`
(float 0.01–2), `AIColorEventTable_Event_SayType` (enum 0x23000119, absent in 1,391 events),
`AIColorEventTable_Event_SoundChannel` (int, 4 in all), `AIColorEventTable_Event_Data` (array
of `AIColorEventTable_Event_DataEntry` {`AIColorEventTable_Event_String` StringInfo, optional
`AIColorEventTable_Event_SoundID` → 0x2A}; 97,130 entries, 1,382 with a sound). These are the
barks a creature says or plays when an event of that type happens.

**Decision D7.** A reader keeps a top-level `AIColorEventTable_Event` as one more event of the
table, in addition to the array; it neither drops it nor lets it replace the array event of
the same type. The explorer lists it separately, marked "top-level".

> **OPEN:** Whether the original server uses the top-level event, the array event of the same
> type, or both. One record in this install; the data cannot decide, and neither the
> executable (C25) nor the class library's code (C26) reads colour-event tables, so no M1–M4
> method reaches the answer.

<a id="s5-6"></a>
### 5.6 Formation (3477, 212 records)

One property, `Formation_Group`: a struct of `Formation_Follower` DIDs (685 in all; → class
1723 in 668, 1724 in 17; up to 10 per record). A template's `Formation_Group_DataID` names the
group it leads and `Formation_Layout_Type` (enum 0x2300036D) the arrangement.

> **OPEN:** (implementer note) `Formation_Group` is called a struct, but a struct holds each
> PID at most once (`property-system.md` §4.10) while a formation has up to 10 followers.
> Tessera collects every `Formation_Follower` DID at any depth under `Formation_Group` (685 on
> the install, as above). Please state the nesting.

<a id="s5-7"></a>
### 5.7 Conjunction (1001, 237 records)

Eight properties each: `Conjunction_Name`, `Conjunction_Description` (StringInfo),
`Conjunction_Category` (enum 0x23000077), `Conjunction_Flags` (bitfield32),
`Conjunction_Icon_Small`/`_Medium`/`_Large` (→ 0x41), `Conjunction_RoleList` (6
`Conjunction_Role` entries in every record; below them 9,954
`Conjunction_Role_ContributionToActionMap` entries, each with `Conjunction_Role_ActionSkill`,
`Conjunction_PvP_Role_ActionSkill` and `Conjunction_Role_ContributionSkill`, → class 827 in
2,016 entries and null in 7,938). The WEENIECONTENT entry `ConjunctionTable` (class 1006) holds
a list of 156 DIDs (`Conjunction_MonsterPlay_ConjunctionList`). Templates reach them
only through `GameEvent_ConjunctionEventTable` scriptlets; `Conjunction_TableOverride` is null in
every template.

> **OPEN:** (implementer note) Where the `Conjunction_Role_ContributionToActionMap` entries sit
> below a `Conjunction_Role` ("below them") is not stated. Tessera takes every struct with
> that PID at any depth under a role element (9,954 on the install, as above). Please state
> the nesting.

<a id="s5-8"></a>
### 5.8 Puppet target package (3761, 82 records)

One property, `AIPuppet_TargetRequestQueue_Type` (enum 0x230003D5: keys 1 and 2; 72 records
key 2, 10 key 1).

---

<a id="s6"></a>
## 6. Spawning: from a landblock to creature templates

<a id="s6-1"></a>
### 6.1 Overview

The chain, in data-flow order:

| Step | From | Through | To |
|---|---|---|---|
| 1 | Generator instance set (no position, §6.2) | `Generator_ProfileDID` | Generator profile (§6.5) |
| 2 | Generator instance set | type-7 links, generator as source | GeneratorPoint static entities: position and rotation (§6.4) |
| 3 | GeneratorPoint | its 0x47 marker's `Generator_PositionSetID` | Position set of the point (§6.4) |
| 4 | Generator profile | `GeneratorProfile_WSLEntity`, `GeneratorProfile_ProfileDefinition` | Templates and sub-profiles, filtered by position set (§6.7) |
| 5 | Spawned-object instance set (per-instance properties; no template DID) | type-8 link, object as source | Its generator (§6.3) |
| 6 | Spawned object or GeneratorPoint | type-2, -1 or -11 link | First PatrolPoint, then type-16 links to the next points (§6.9) |

<a id="s6-2"></a>
### 6.2 Generator instance sets (Confirmed C13)

A landblock's weenie list (`landblock.md` §4.8) holds 3,768,840 instance sets; 514,987 belong
to instance IDs that no static entity places (`entity-descriptor.md` §4.5.2). They have **no
position and no template DID**; they are linked to the world only through links.

- 34,296 instance sets hold `Generator_ProfileDID` (→ 0x70 class 1132 in all; 1,413 distinct
  profiles): 34,275 without a static entity, 20 belonging to GeneratorPoint entities and 1 to a
  PatrolPoint.
- Other generator properties on instance sets: `Generator_PositionSetLimitArray` (19,306 sets;
  array of `Generator_PositionSetLimitRecord` {`Generator_PositionSetID`,
  `Generator_PositionSetLimit` int}; 28,884 records, limits 1–35, most often 2 (12,368), 3
  (7,367), 1 (3,868), 10 (3,117)), `Generator_Skirmish_Multiple_Wave` (5,175), `Generator_Skirmish_Wave_Activated_Delay`
  (513), `Generator_PurgeOnDeactivation` (517), `Generator_PlaySoundOnActivate` (459),
  `Generator_ForceEntityExactPlacement` (385), `Generator_DeactivateOnPELock` (364),
  `Generator_IsScaling` (10).
- The other instance sets without a static entity (spawned objects and helpers; the sources of
  type-8 links are among them) carry per-instance overrides such as `AI_WanderingRange` (24,748 sets, 0–200 m), `AI_PhysicallyWanders` (24,460),
  `AI_WanderingProb` (9,987), `AI_PerceptionRadius` (10,705), `NPC_StartingIdleAnimation`
  (63,066), `QuestDispenser_RoleConstraint` (14,894), `Name` (5,924), `Appearance_InstanceList`
  (30,258), `Drama_DramaList` (27,169), scale, and `Entity_ContentLayers` (161,759).

<a id="s6-3"></a>
### 6.3 Links used by spawning (Confirmed C6, C13)

Link layout and type codes: `landblock.md` §4.6. Ends: `end_id_1` is the **target**, `end_id_2`
the **source** (§3.3). The link's `name` is an editor label; Tessera does not depend on it.

| Type | Source → target (counts, all landblocks) | Properties on the link |
|---|---|---|
| 7 Generator_PositionSet | generator with profile → GeneratorPoint 418,053; generator without profile → GeneratorPoint 894; generator → GeneratorPoint whose own instance set is a generator 20; cross-landblock half (source in the other landblock) 22 | none |
| 8 Generator | spawned-object instance set → generator without profile 222,721; → generator with profile 4,948; cross-landblock halves 4,477 + 4,480; PatrolPoint → PatrolPoint 302; others 300 | none |
| 2 Patrol | instance set without a static entity → PatrolPoint 22,548; GeneratorPoint → PatrolPoint 4,225; cross-landblock halves 983 + 943; other 18 | `LinkName` (15,144), `PatrolLink_Speed` (7,082; 0–50, 1 in 5,856) |
| 1 PatrolOnce | instance set → PatrolPoint 5,843; GeneratorPoint → PatrolPoint 531; others 349 | `LinkName` (2,096), `PatrolLink_Speed` (2,466; 0–4.5) |
| 11 FlyingPatrol | instance set → PatrolPoint 234; others 17 | same |
| 16 Path | PatrolPoint → PatrolPoint 241,081; cross-landblock halves 12,564 + 12,549 | none observed |

- A generator has 0 type-7 links (721 of the 34,296), 1–4 (5,449), 5–19 (21,024) or 20 or more
  (7,102). 272 instance sets without a profile are sources of type-7 links.
- 1,206 generators with a profile are also targets of type-8 links: they spawn their profile's
  templates **and** individually placed objects.
- Cross-landblock links are stored as two halves, one in each landblock, each with the far
  end set to 0 and the block-pair fields filled; pairing the halves is `landblock.md` §4.6's
  OPEN.
- Link ids (field 2): kind byte 4, region 0, landblock bytes 0xFF/0xFF (1,318,390 links) or
  0/0 (6,079), then a serial. They are **not unique** across landblocks (43,499 distinct
  values); a reader treats them as landblock-local.

<a id="s6-4"></a>
### 6.4 Spawn points and position sets (Confirmed C13, C14)

Every GeneratorPoint static entity (415,238) names a 0x47 marker (239 distinct). Every one of
the 239 markers carries `Generator_PositionSetID` (enum 0x23000047, 360 keys) in its own
property set. A spawn point's position set is, in order of precedence:

1. `Generator_PositionSetID` in the point's instance set, else in its own static-entity set
   (74 points carry it in both; in all 74 it equals the marker's value);
2. otherwise the marker's value (followed through parent 0x47 records if the marker lacks it;
   never needed in this install).

The point's frame (position, rotation) is the static entity's (`entity-descriptor.md` §4.4);
its own set may also carry `AI_PhysicallyWanders` (279,723; true 266,256),
`AI_WanderingRange` (266,499; 0–90 m, 30 in 110,431), `AI_WanderingProb` (5,366) and
`GeneratorProfile_WBOnly_PerceptionRadius` (153), which apply to what spawns there.

The 0x47 record's `kind` field (`setup.md` §4.3) equals the static-entity type of every static
entity that names it: 3 for GeneratorPoint markers, 23 PatrolPoint, 7 Physics, 2 Decal,
9 Light, 19 SegmentNode, 31 LBIOnly (3,210,410 of 3,210,410).

Consistency (C13): among the 34,240 generators whose profile tree (with nested profiles)
contains position-set entries, every point's position set is named in that tree for 33,922; `Generator_PositionSetLimitRecord` names a position set present among the
generator's points in 28,877 of 28,924 records and one named by its profile in 28,700.

<a id="s6-5"></a>
### 6.5 Generator profile property set (root class 1132, 4,168 records; Confirmed C15)

Exactly one top-level property, `GeneratorProfile_Profile`, an array. Structs below are
property sets (field order in storage is bucket order and carries no meaning). Arrays keep
their element order.

**`GeneratorProfile_Profile` (array).** Elements, in any mix:

| Element | Count | Notes |
|---|---|---|
| `GeneratorProfile_RegenType` (enum 0x23000048, 22 keys) | 3,621 | Always element 0 when present; at most one |
| `GeneratorProfile_Entry` (struct) | 75 in 20 profiles | An unconditional spawn entry |
| `GeneratorProfile_OneOf` (array) | 3,328 in 2,736 profiles | A choice |
| `GeneratorProfile_PositionSetEntry` (struct) | 6,010 in 1,759 profiles | Entries scoped to one position set |

**`GeneratorProfile_Entry` (struct).** Exactly one target, plus modifiers:

| Field | Type | Where it occurs |
|---|---|---|
| `GeneratorProfile_WSLEntity` | DID → template (or other object) | 23,270 entries |
| `GeneratorProfile_ProfileDefinition` | DID → another profile (1132) | 3,352 entries |
| `GeneratorProfile_Weight` | int | Every entry inside a `OneOf` (1–1,000; 1 most often); never elsewhere |
| `GeneratorProfile_Probability` | int, per mille (dictionary default 1000) | Every entry directly inside a `PositionSetEntry` (420; values 1, 300, 400, 500, 600, 800, 1000) and 1 top-level entry |
| `GeneratorProfile_Repeat` | struct {`…_RepeatMinTimes`, `…_RepeatMaxTimes`} ints (dictionary default 1) | 8 top-level entries |
| `GeneratorProfile_ExactPlacement` | struct {`…_ExactPlacementX`, `…Y`, `…Z`} floats, world units (X ±2.5, Y −9.5–0, Z 0 or 0.05) | 66 top-level entries |
| `GeneratorProfile_RandomPlacement` | struct {`…_RandomPlacementMinDist`, `…MaxDist`} floats, world units (min 0–10, max 3–40) | 8 top-level entries |
| `GeneratorProfile_Properties` | array of property values (PID-tagged elements) | 1,540 entries: `AI_PerceptionRadius` (20 or 40), `AI_KungFu_StartAsNPC`, two skirmish flags |

**`GeneratorProfile_OneOf` (array).** At most one leading modifier element, then the choices:

| Element | Count |
|---|---|
| `GeneratorProfile_Repeat` (struct) | 16 directly in a profile, and in **every** `OneOf` inside a `PositionSetEntry` that is itself inside a `OneOf` (1,952) |
| `GeneratorProfile_ExactPlacement` (struct) | 26 |
| `GeneratorProfile_RandomPlacement` (struct) | 35 |
| `GeneratorProfile_Entry` (struct, with `Weight`) | 10,305 in `OneOf`s directly in a profile; 10,232 in the `OneOf`s of the 1,952 `PositionSetEntry` structs that are `OneOf` elements; 5,590 in the `OneOf`s of the 5,590 `PositionSetEntry` structs directly in a profile |
| `GeneratorProfile_PositionSetEntry` (struct, with `Weight` = 1) | 1,952 |

The 3,328 `OneOf`s directly in a profile have one of two shapes (Confirmed, all records):

- **Choice of entries** (2,440): `Entry` elements only, after an optional modifier at element
  0 (`Repeat` 16, all with minimum = maximum = 1; `ExactPlacement` 26; `RandomPlacement` 35;
  none in 2,363).
- **Choice of position-set entries** (888): `PositionSetEntry` elements only (1 in 172
  `OneOf`s, 2 in 368, 3 in 348; 1,952 in all), with **no** modifier element and no `Entry`.
  In every one of the 888, all its `PositionSetEntry` elements name the **same** position set,
  and each has `Weight` 1. Each such `PositionSetEntry` holds a `OneOf` whose element 0 is a
  `Repeat` with minimum = maximum (1 in 900, 2 in 756, 3 in 64, 4 in 204, 5 in 24, 6 in 4),
  followed by `Entry` elements, each with a `WSLEntity` and a `Weight` (and
  `GeneratorProfile_Properties` in 1,260).

The `OneOf`s of the 5,590 `PositionSetEntry` structs directly in a profile hold exactly one
`Entry` each and no modifier. No `OneOf` holds another `OneOf` directly.

**`GeneratorProfile_PositionSetEntry` (struct).** `GeneratorProfile_PositionSetID` (enum
0x23000047) and either a single `GeneratorProfile_Entry` (420, each with `Probability` and a
`ProfileDefinition`) or a `GeneratorProfile_OneOf` (5,590 + 1,952); `GeneratorProfile_Weight`
when it is itself a choice.

Further facts: `RepeatMinTimes` ≤ `RepeatMaxTimes` in all 1,976 repeat structs (equal in 1,968);
`MinDist` ≤ `MaxDist` in all 43 random placements; 2,409 profiles contain no
`PositionSetEntry` of their own (many are used only as sub-profiles). The dictionary defines further `GeneratorProfile_*` properties
(`…_Switch`, `…_Progressive*`, `…_ConditionedProfile`, `…_Condition`, `…_Generator_MinLevel`,
`…_Generator_MaxLevel`, `…_Quality`, `…_RegenerationDelay*`, `…_RandomPlacementHeading`,
`…_ExactPlacementHeading`, `…_RegenScalarDID`) that occur in **no** record of this install; a
reader keeps them if met but needs no rule for them.

<a id="s6-6"></a>
### 6.6 Profile object form (0x70 bundle; Confirmed C16)

A profile's object record holds the same tree as script and native objects. Over the 4,168
records every bundle class has one role, and in **every** record the object tree equals the
property-set tree of §6.5 node for node: targets, weights, probabilities, position sets,
repeats, placements and per-entry properties (C16, compared over all 4,168).

| Bundle class | Items | Role | Property-set counterpart |
|---|---|---|---|
| 1132 | 4,168 | Root object; one reference to the top node | The record |
| 1130 | 4,168 | Top node: the regeneration type (integer; 0 when the profile has no `GeneratorProfile_RegenType`) and a reference to its child list | `GeneratorProfile_Profile` and its `RegenType` element |
| native 176 | 15,038 (4,168 + 10,870) | Child list: references to the child nodes in stored order (the `RegenType` element is not a child) | The elements of the profile array or of a `OneOf` |
| 1136 | 10,870 | Choice node: a child list, a weight list and the choice's placement modifier | `GeneratorProfile_OneOf` |
| native 17 | 10,870 | Weight list: one unsigned integer per child of the choice node, in child order (lengths 1–40) | The `Weight` of each `Entry` or `PositionSetEntry` in the `OneOf` |
| 1131 | 23,270 | Entry node: the target DID (stored as an integer), probability, placement and a reference to its property set | `Entry` with `WSLEntity` |
| 1143 | 3,352 | The same with a profile DID as target | `Entry` with `ProfileDefinition` |
| native 166 | 23,550 (23,270 + 280) | Embedded property set: the entry's `GeneratorProfile_Properties` (non-empty in 1,540, empty otherwise); 280 of the 3,352 profile-target entries have one | `GeneratorProfile_Properties` |
| 1137 | 7,962 | Position-set node: the position-set ID (integer) and one child (an entry, a choice or a repeat node) | `GeneratorProfile_PositionSetEntry` |
| 1140 | 1,060 | Repeat node: minimum and maximum count (integers) and one child | `GeneratorProfile_Repeat`, **only when it is not minimum = maximum = 1** |
| native 72 | 92 | Exact placement as a frame: X, Y, Z, then a rotation that is (1, 0, 0, 0) in all 92 (`wstate.md` §5.4) | `GeneratorProfile_ExactPlacement` (66 on entries, 26 on choices) |

Further facts:

- **Repeats.** A repeat of 1 to 1 has no node: none of the 16 repeats directly in a profile's
  `OneOf` (all 1 to 1) and none of the 900 inner repeats of 1 to 1 has one. The 8 entry-level
  repeats wrap their entry node, and the other 1,052 inner repeats (2 to 6) wrap their choice
  node under the position-set node: 8 + 1,052 = 1,060.
- **Common node fields.** Every node class (1130, 1131, 1136, 1137, 1140, 1143) has a
  reference to its parent node (null on the top node), the regeneration-type integer (0
  on every node below the top node), a probability integer (1000 unless
  the entry has a `GeneratorProfile_Probability`; 7 values in profile-target entries, 1000
  in every other node), a random-placement minimum and maximum (0 and 0 unless the node
  carries a `GeneratorProfile_RandomPlacement`; set on 8 entry nodes and 35 choice nodes), a
  reference to an exact-placement frame (null unless placed) and a reference to an embedded
  property set (non-null only on entry nodes). Five further fields hold the same value in
  every node of every record (two integers 0, two floats −1.0 and one float −50.1); none
  corresponds to a property present in this install.

<a id="s6-6-1"></a>
#### 6.6.1 Attribute name hashes of the profile classes (Confirmed C16)

Attributes of a script object are identified by their name hash: a script-object body
(`wstate.md` §5.2) holds one value per attribute in the order of the class's attribute list
(`wlib.md` §8.4), which gives each attribute's name hash and type. The hashes below are quoted
as interoperability identifiers. Each was matched to its role over every node of its class in all
4,168 profiles (OBS3): scalar fields against the property-set value of the node's counterpart
(with the stated value when the property is absent), reference fields against the item they
resolve to.

**Fields common to the six node classes** (1130, 1131, 1136, 1137, 1140, 1143; the same hash in
each class):

| Hash | Type | Field | Value |
|---|---|---|---|
| 0x06688D64 | reference | Parent node | Null on the top node (1130); otherwise the node whose child list or child reference names this node |
| 0x054A92F9 | integer | Regeneration type | On the top node the `GeneratorProfile_RegenType` key, 0 when the profile has none (560 top nodes hold 0); 0 on every other node |
| 0x04E9C549 | integer | Probability, per mille as stored | The entry's `GeneratorProfile_Probability`, 1000 when absent; 1000 on every node that is not an entry node |
| 0x0E886945 | float | Random-placement minimum distance (world units) | `…_RandomPlacementMinDist` of the node's counterpart, 0 when it has no random placement |
| 0x06686935 | float | Random-placement maximum distance (world units) | `…_RandomPlacementMaxDist`, 0 when none |
| 0x0B13FE35 | reference | Exact-placement frame (native 72) | Null unless the counterpart has a `GeneratorProfile_ExactPlacement` (66 entry nodes of class 1131, 26 choice nodes) |
| 0x045679F3 | reference | Embedded property set (native 166) | Non-null on every 1131 node (23,270) and on 280 of the 3,352 1143 nodes; null on all other nodes |
| 0x08C1928C | integer | Constant | 0 in every node |
| 0x006192FC | integer | Constant | 0 in every node |
| 0x010A77F5 | float | Constant | −1.0 in every node |
| 0x010C1F15 | float | Constant | −1.0 in every node |
| 0x0D0DF187 | float | Constant | −50.1 (as a 32-bit float) in every node |

**Fields of one class:**

| Class | Hash | Type | Field |
|---|---|---|---|
| 1132 (root) | 0x0ACD5401 | reference | The top node (class 1130); the root has no other attribute |
| 1130 (top) | 0x018C7B59 | reference | Child list (native 176) |
| 1136 (choice) | 0x018C7B59 | reference | Child list (native 176) |
| 1136 (choice) | 0x0DBFACD3 | reference | Weight list (native 17), one weight per child in child order |
| 1131 (entry) | 0x0B788484 | integer | Target DID (`GeneratorProfile_WSLEntity`) |
| 1143 (entry) | 0x04615C54 | integer | Target profile DID (`GeneratorProfile_ProfileDefinition`) |
| 1137 (position set) | 0x0679EEF4 | integer | Position-set ID (`GeneratorProfile_PositionSetID`) |
| 1137 (position set) | 0x068EDBC4 | reference | Its one child (an entry, choice or repeat node) |
| 1140 (repeat) | 0x0FF0F883 | integer | Repeat minimum (`…_RepeatMinTimes`) |
| 1140 (repeat) | 0x0FF08063 | integer | Repeat maximum (`…_RepeatMaxTimes`) |
| 1140 (repeat) | 0x0D4E3D11 | reference | Its one child (an entry node or a choice node) |

Where a placement is stored: an entry's own `RandomPlacement` or `ExactPlacement` sits on its
entry node, a `OneOf`'s modifier sits on its choice node, and a repeat node's placement fields
are 0 and its frame reference null (the placement, if any, stays on the node it wraps). The
probability of a repeat or position-set node is 1000 even when its child entry has another
value.

Tessera reads profiles from the **property set**, which is complete; the object form is
not needed. A tool that shows the object form can label its items with this table.

<a id="s6-7"></a>
### 6.7 Static resolution: which templates can appear at a spawn point (algorithm)

Used by tools (spawn overlay, §10.2) and by the sandbox. Input: a generator instance set *G*
with profile *P*, and one of its spawn points *S* with position set *s* (§6.4).

1. Start with an empty result list. Walk *P*'s `GeneratorProfile_Profile` array.
2. For a top-level `Entry`: add its target (step 5).
3. For a `OneOf`: for each `Entry` element add its target, carrying the element's weight;
   for each `PositionSetEntry` element apply step 4. An element skipped in step 4 takes no
   part in the `OneOf`'s weights (D3, §6.8). A modifier at element 0 is carried with every
   target the `OneOf` adds.
4. For a `PositionSetEntry`: if its `GeneratorProfile_PositionSetID` differs from *s*, skip
   it. Otherwise add the target of its `Entry` (carrying the probability) or walk its `OneOf`
   as in step 3.
5. Adding a target: a `WSLEntity` DID is added as a candidate; a `ProfileDefinition` DID is
   expanded by walking that profile from step 1 **with the same *s***.
6. Guard: stop expanding a profile already on the current expansion path, and stop at depth 8.
   In this install no cycle exists and the deepest nesting is 4 (§6.10).

Entries outside any `PositionSetEntry` apply to every point of the generator (Decision D4;
profile trees that mix scoped and unscoped entries occur for 33,671 generators). A
sub-profile reached from inside a `PositionSetEntry` stays scoped to that position set
(step 5). The result lists the
templates that **can** appear, with their weights and probabilities; which one appears is a
run-time choice (§6.8).

> **OPEN:** (implementer note) Which weight and probability a candidate carries when several
> lie on its path is not stated. Tessera reports the weight of the nearest enclosing `OneOf`
> choice, the nearest `GeneratorProfile_Probability`, and the innermost placement and repeat;
> a template reached on several paths is listed once per path; entries removed by C17 are
> left out. A walk visits at most 65,536 profile elements (protection against hostile
> profile graphs). Confirm or give the rule.

<a id="s6-8"></a>
### 6.8 Run-time semantics (Reported; Decisions for the sandbox)

The data hold no rule for choosing, and the client does not spawn creatures. The one
published tool that reads profiles (LC) skips entries whose weight or probability is 0 or
less (Reported, C17); it states no other rule. Everything else in this section is Tessera's
own sandbox rule (Decision D3, C17a), not a claim about the original.

**Decision D3 (Tessera sandbox backend, not client behaviour):**

1. A generator becomes active when its landblock loads and its content-layer test passes
   (`entity-descriptor.md` §4.5.1).
2. For each spawn point *S* with position set *s* (§6.4), evaluate the generator's profile
   once and spawn every object the evaluation produces. Entries with a weight or probability
   of 0 or less are removed before anything else (C17). Evaluation rules:
   1. **Profile.** Evaluate the elements of `GeneratorProfile_Profile` in stored order and
      collect what each produces. `GeneratorProfile_RegenType` produces nothing (step 6).
   2. **`Entry`.** If it has `GeneratorProfile_Probability` *p*, test once with chance
      *p* / 1000; on failure the entry produces nothing. Otherwise (or on success) produce its
      target *k* times, where *k* is 1, or, when the entry has its own `GeneratorProfile_Repeat`,
      a whole number chosen uniformly between `RepeatMinTimes` and `RepeatMaxTimes` inclusive.
      Producing a `WSLEntity` target makes one object; producing a `ProfileDefinition` target
      evaluates that profile from rule 2.1 with the same *s* and takes all it produces.
   3. **`PositionSetEntry`.** If its `GeneratorProfile_PositionSetID` differs from *s*, it
      produces nothing. Otherwise it produces what its `Entry` (rule 2.2) or its `OneOf` (rule
      2.4) produces.
   4. **`OneOf`.** Set aside a modifier at element 0 (`Repeat`, `ExactPlacement` or
      `RandomPlacement`); the other elements are the choices. A choice is **eligible** when it
      is an `Entry`, or a `PositionSetEntry` whose position set equals *s*. Choices that are
      not eligible are removed **before** the draw and do not count in the weight sum. If no
      choice is eligible, the `OneOf` produces nothing. Otherwise **one draw** picks one
      eligible choice with chance (its `GeneratorProfile_Weight`) / (sum of the weights of the
      eligible choices) and produces what that choice produces (rules 2.2 and 2.3). A
      `Repeat` modifier repeats the **whole draw**: the number of draws is a whole number
      chosen uniformly between its bounds, each draw is independent (the same choice may be
      picked again), and the `OneOf` produces the union of all draws. A placement modifier
      applies to **every object** the `OneOf` produces, through all draws and nested
      profiles, except an object whose own `Entry` carries a placement: the innermost
      placement wins.
   5. **Limits.** Nesting is guarded as in §6.7 step 6; at most 256 objects are produced per
      spawn point per evaluation, and further objects are dropped with a warning. (In this
      install the most one profile can produce at one point, taking every repeat at its
      maximum, is 81, reached by 2 profiles; every other profile and position set gives 15 or
      fewer.)

   In this install every `OneOf` that holds `PositionSetEntry` choices holds nothing else and
   names a single position set (§6.5), so at a point of that set one of its 1–3 choices is
   drawn with equal chance, and at any other point it produces nothing; whether non-matching
   choices count in the weight sum only matters for data that mix position sets in one
   `OneOf`. Each chosen `PositionSetEntry` then makes 1–6 draws from its own `OneOf`
   (its leading `Repeat`), all at the same point.
3. Place each produced object at the spawn point's frame, displaced by its exact placement
   (X, Y, Z in the point's frame) or by a uniformly random point of its placement annulus
   (between `RandomPlacementMinDist` and `RandomPlacementMaxDist` from the point) on the
   ground; without a placement, at the point itself.
4. Respect `Generator_PositionSetLimit`: at most that many points of the named position set
   are occupied at once.
5. Apply, in order, the template's properties, the entry's `GeneratorProfile_Properties`, and
   the instance properties of the spawn point; the later value wins.
6. Respawn after a delay chosen by `GeneratorProfile_RegenType` from a Tessera table; the
   install holds only the labels.
7. Level, examination rank and vital maxima come from Tessera's own configuration (§4.8).
8. For AI (§5.1): consider every action table whose level range contains the level; an action
   is eligible when its hint conditions hold and its cooldowns have run out; pick among eligible
   actions with probability proportional to the §5.2 percentage, treating −1 as "always
   preferred". Attitude: an entity's attitude towards another is its attitude table
   evaluated against the other by the rule of §5.3.1 (aggressive list first, so an
   aggressive match wins over a friendly one), with the classification values of Decision
   D8; the sandbox treats the entity as hostile to the other when the result is
   `Aggressive`. (The §5.3.1 rule is the original's, C27; only the use of the result is
   Tessera's.)

> **OPEN:** The original server's choice, placement, repeat, respawn and AI-action rules,
> including the frame an exact placement is expressed in (the spawn point's or the world's
> axes; the object form stores it as a frame with an identity rotation, §6.6, which does not
> decide this). They are not in the data, the executable has no direct reference to
> `GeneratorProfile_Profile` or `Generator_ProfileDID` (C25) and the class library's code holds no reader of them (C26), so
> no M1–M4 method reaches them; D3 may differ from them. **Hypothesis** (this
> spec writer's reading of the property names, the dictionary default of
> `GeneratorProfile_Probability`, 1000, and the observed ranges; not confirmed by any source):
> `Weight` is a relative weight among the choices of a `OneOf`; `Probability` is a chance in
> per mille that an entry spawns (observed values 1–1000); `Repeat` is how many times an entry
> or a choice is produced; `ExactPlacement` is an offset from the spawn point;
> `RandomPlacement` an annulus around it. D3 adopts this hypothesis for the sandbox only.

> **OPEN:** (implementer note: engineering choices of `evaluate_profile` where D3 is silent;
> not claims about the original) (a) A `OneOf` choice without `GeneratorProfile_Weight` counts
> with weight 1. (b) Absent repeat bounds take the dictionary default of `…_RepeatMinTimes`
> when it has one, else 1; bounds out of order are swapped and negative bounds count as 0.
> (c) A probability of 1000 or more succeeds without a random draw, and a `OneOf` with a single
> eligible choice takes it without a draw. (d) An entry holding both placements uses the exact
> one; an entry naming both a `WSLEntity` and a `ProfileDefinition` produces the object only;
> a `PositionSetEntry` holding both an `Entry` and a `OneOf` produces both; a `PositionSetEntry`
> choice with a weight of 0 or less is removed like an entry (C17). The first three shapes are
> reported as grammar deviations; none of the four occurs in the install. (e) Besides the
> 256-object cap, one evaluation visits at most 65,536 profile elements. Confirm or replace.

<a id="s6-9"></a>
### 6.9 Patrol paths (Confirmed C18)

PatrolPoint static entities (268,896; 8 distinct 0x47 markers, all of `kind` 23) form a
directed graph through type-16 links (source = earlier point, target = next point). Out-degree
is 1 for 247,413 points, 0 for 18,926 and 2–16 for the rest; in-degree is mostly 1. A patrol
starts at the target of a type-2, -1 or -11 link (§6.3). Following the unique next point from
the start, the observed chains:

| Start link | Ends without next | Returns to the start | Enters a loop elsewhere | Leaves the landblock |
|---|---|---|---|---|
| Patrol (2) | 15,047 | 6,895 | 278 | 5,554 |
| PatrolOnce (1) | 3,713 | 1,230 | 68 | 1,592 |
| FlyingPatrol (11) | 173 | 11 | 0 | 59 |

Chains with branching points were followed through the last-stored next link only; a
consumer that needs branches follows all of them. `PatrolLink_Speed` on the start link takes
values 0–50, 1 in most links.

> **OPEN:** How a patrol chooses among several next points, and whether PatrolOnce stops at
> the end while Patrol reverses or restarts. Not in the data and not traced in the client. An owner observation of the original (M3) could show the end
> behaviour of one patrol of each kind.

> **OPEN:** The meaning and unit of `PatrolLink_Speed`. Hypothesis of this spec writer, from
> the label only: a speed factor for the patrolling creature, 1 meaning its normal speed. Not
> confirmed: the executable holds its property ID only as a UI element ID (C25) and the class
> library's code does not read it (C26). An owner observation of the original (M3) on a
> patrol whose start link carries a value other than 1 could settle it. An implementation must
> not depend on the hypothesis without a revision of this spec.

<a id="s6-10"></a>
### 6.10 What the spawn data can and cannot reconstruct (Confirmed)

- Profile nesting depth (through `ProfileDefinition`): 0 for 625 generators, 1 for 22,808,
  2 for 9,856, 3 for 997, 4 for 10; no cycles.
- Templates reachable from the 1,413 profiles named by landblock generators and the 2,323
  profiles they reach through `ProfileDefinition`: 4,742 distinct mob templates (of 20,161),
  **17** NPC templates (of 16,605), 318 containers, 64 items (class 796) and 4 hotspots (1170).
- The objects attached to generators through type-8 links (222,721 links from instance sets
  without a template DID, §6.3) cannot be resolved to templates from the client data; only 17
  NPC templates are reachable through profiles at all. A server built on the client data can
  place the generator-profile population, but needs its own source for the type-8 objects and
  for most NPCs.

---

<a id="s7"></a>
## 7. Encounter table (prefix 0x14, ENCOUNTER_DESC; Confirmed C19)

Five records in `client_general.dat` (0x14000000, …01, …02, …07, …0A; slot 3 of the region
descriptors, `environment.md` §4). All decode to the last byte with this layout:

| Offset / order | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | Own DID |
| 4 | 4 | u32 | 0 in all |
| 8 | 4 | u32 | 6 in all: the block-map key the table is indexed by (`landblock.md` §5.6, L19) |
| 12 | 4 | u32 | *n*, number of zone entries (16 in 0x14000000, 1 in the others) |
| 16 | var | zone × *n* | below |

Zone entry:

| Order | Size | Type | Meaning |
|---|---|---|---|
| 1 | 4 | u32 | Zone key; equals the entry's index (0 … *n* − 1) in all |
| 2 | var | Pascal string (VLE length, Latin-1; `records.md` §1.4) | Editor label, 4–12 bytes |
| 3 | 4 | u32 | *m*, number of slots (1, 5, 7, 12 or 13) |
| 4 | 21 × *m* | slot | below |

Slot (21 bytes):

| Offset | Size | Type | Observed |
|---|---|---|---|
| 0 | 4 | u32 | Slot index, 0 … *m* − 1 |
| 4 | 4 | u32 | 0 in all 70 slots |
| 8 | 4 | f32 | 1.0 in all |
| 12 | 4 | u32 | 1 in all |
| 16 | 4 | u32 | 0 in all |
| 20 | 1 | u8 | 0 in all |

The block-map key-6 values observed in region 1 (0, 1, 4, 6, 12–15; `landblock.md` §5.6) are
all zone keys of 0x14000000. Every slot carries the same values, so the table holds no
per-zone data in this install.

> **OPEN:** The meaning of the slot fields at offsets 4–20 and of the slot index. With every
> slot identical, the data cannot decide. The client registers the DID range
> 0x14000000–0x14FFFFFF with its record cache (BIN2), but the reader of the record was not
> reached; a further M4 pass from that registration to the type's serializer could name the
> fields. Not needed: Tessera's world and sandbox ignore the table.

---

<a id="s8"></a>
## 8. Enum mappers (Confirmed C20)

Mapper DIDs are those the property dictionary names for each property in this install; a
reader takes them from the dictionary (`property-system.md` §7), not from this table.
Bitfield bit *i* is labelled by key *i* + 1 (`property-system.md` §7).

| Property (examples) | Mapper | Keys | Notes |
|---|---|---|---|
| `Agent_Species`, `Classification_Species`, filter-exception species | 0x2300000B | 123 | |
| `Agent_Subspecies` | 0x2300000C | 768 | |
| `Agent_Genus`, `Classification_Genus` (bitfield32) | 0x2300000A | 18 | One bit per creature |
| `Agent_Alignment`, `Classification_Alignment` | 0x2300000D | 6 | |
| `Agent_Class` | 0x2300000E | 212 | Player and creature classes |
| `Agent_ClassificationFilter`, `ClassificationFilter_Filter_Type` | 0x2300000F | 7 | |
| Attitude results of §5.3.1 (the mapper of `GCParam_HotspotAttitude`) | 0x23000112 | keys 0–3 labelled | 1 `Neutral`, 2 `Friendly`, 3 `Aggressive` |
| Link types (EMAPPER label `EntityLinkType`, as in `wstate.md`) | 0x230000BE → base 0x230001A9 | | Key 13 `PathMapTest` (§3.3) |
| `Quest_MonsterDivision` | 0x23000061 | 592 | |
| `MonsterLevel_ExaminationModStatType` | 0x23000070 | 35 | Examination rank |
| `AI_MovementType` | 0x23000021 | 6 | |
| `AI_MovementBehavior` | 0x2300036B | 7 | Class-constant in mobs |
| `AI_CallForHelp_Type`, `AI_CallForHelp_RespondTypes` (bitfield32) | 0x23000023 | 9 | |
| `AI_LinkedCombat` | 0x2300011B | 3 | Instance sets (4,755) |
| `AI_Hints`, `AIAction_*Hints` (any-width bitfields) | 0x2300005C | 176 | |
| `AIAction_Probability`, `AIActionProbability_Type` | 0x23000024 | 10 | §5.2 |
| `AIAction_CustomTarget` | 0x23000118 | 35 | |
| `AIAction_CooldownChannel` | 0x2300006C | 321 | |
| `AIAction_ColorEventType`, `AIColorEventTable_Event_Type` | 0x23000117 | 25 | |
| `AIColorEventTable_Event_SayType` | 0x23000119 | 11 | |
| `Conjunction_Category` | 0x23000077 | 18 | |
| `Formation_Layout_Type` | 0x2300036D | 10 | |
| `AIPuppet_TargetRequestQueue_Type` | 0x230003D5 | 3 | |
| `AIPuppet_TargetFlags` (bitfield32) | 0x230003B8 | 22 | |
| `GeneratorProfile_PositionSetID`, `Generator_PositionSetID` | 0x23000047 | 360 | |
| `GeneratorProfile_RegenType`, `Generator_RegenType` | 0x23000048 | 22 | Labels only; no delays in the data |
| `Entity_CreationType` | 0x23000174 | 10 | |
| `LifeState` | 0x23000123 | 6 | Class-constant `Alive` in templates |
| `WeenieType` | 0x230000C9 → base 0x230001AB | 38 + 19 | §2.2 |
| `Appearance_RandomAppearanceType` | 0x2300031D | 440 | |
| `CombatState_CurrentStateImmunities` (bitfield32) | 0x23000050 | 25 | |
| `Combat_AnimationSet` | 0x2300010C | 45 | |
| `Material_*` | 0x230000F3 | 54 | |
| `VO@NPC.m_eDemeanor` | 0x23000162 | 22 | |

---

<a id="s9"></a>
## 9. DID graph (Confirmed)

| From | Property | To | Count (non-null) |
|---|---|---|---|
| landblock instance set (generator) | `Generator_ProfileDID` | profile, class 1132 | 34,296 |
| profile 1132 | `GeneratorProfile_ProfileDefinition` | profile 1132 | 3,352 entries |
| profile 1132 | `GeneratorProfile_WSLEntity` | classes 1723, 1724, 794, 796, 1170 | 23,270 entries |
| GeneratorPoint static entity | `object` | 0x47 marker (`Generator_PositionSetID`) | 415,238 |
| template | `Agent_Classification` | classification 973 | 36,929 |
| template | `Attitude_Table` (and the bundle's DID item) | attitude 884 | 36,929 |
| mob | `AI_ActionTable_Array` / `AI_ActionTable` | action table 534 | 50,890 |
| action table 534 | `AIAction_Skill` | skill 827 | 15,023 |
| WEENIECONTENT `AIActionProbability` | | probability table 1655 | 1 |
| WEENIECONTENT `ClassificationControl` (class 1658) | `Classification_Control_List` | classification 973 | 116 |
| mob | `AI_ColorEventTable` | colour events 539 | 1,810 |
| colour events 539 | `AIColorEventTable_Event_SoundID` | sound 0x2A | 1,382 (array events; 1,390 with the top-level event, §5.5) |
| mob | `Formation_Group_DataID` | formation 3477 | 272 |
| formation 3477 | `Formation_Follower` | 1723 / 1724 | 668 / 17 |
| mob | `AIPuppet_TargetEvalPackage` | puppet 3761 | 352 |
| mob | `AI_CallForHelpFilter` | filter list 977 | 48 |
| conjunction 1001 | role skills | skill 827 | 2,016 × 3 |
| template | `PhysObj` | 0x47 | 36,766 (mobs + NPCs) |
| template | `Appearance_ClassList` / `Appearance_AprFile` | 0x20 | 27,936 (mobs) |
| template | `Inventory_Cosmetic_Loadout` | loadout 1198 | 59,410 (mobs) |
| template | `Entity_AdditionalScriptTables` | 0x07 | 76,397 (mobs) |
| template | `GameEvent_*EventTable` | 0x0C | 644 (mobs) |
| template | `Effect_Monster*Effect_Array` / `Effect_StartupEffectID` | effects | 11,517 (mobs) |
| template | `LootGen_*` | loot classes 1236, 1237, 1917, 1918, 3666 | §4.5 |
| template | `QuestDispenser_Quest`, `Quest_RelatedQuest` | quest 1398 | 42,692 + 373 |
| template | `Reputation_*Faction` | faction 1889 (2 → 1346) | 784 |
| template | `Usage_Action`, `CSM_ActionHandler`, `Induction_ActionDID` | 1577–1589, 1538–1539, 1195 | all |

---

<a id="s10"></a>
## 10. Tool views (Decisions D5, D6)

<a id="s10-1"></a>
### 10.1 Creature card (explorer)

For a template DID the card shows, in this order (Decision D5):

1. Title: `Name` rendered as text (`string-table.md` §9); DID; root class; `WeenieType` label.
2. Classification: species, subspecies (only when `Agent_ShowSubspecies` is true), genus
   (the label of the one set bit), alignment, class, classification filter, monster division;
   for NPCs the examination rank; a warning if the classification record disagrees (§5.4).
3. Model: `PhysObj` and appearance list with a 3D preview (`appearance.md` §7), scales,
   held props.
4. AI: movement type, perception and call-for-help radii, hints (labels), action tables with
   level ranges, each expandable to its actions (skill DIDs, probability as percentage from
   §5.2, cooldowns), attitude summary, colour-event table, formation.
5. Loot references (§4.5), quest roles (§4.6) with quest names, factions, services.
6. Spawns: the generators whose profiles reach this template (§6.7, reverse index), with
   landblock and point count; "not placed by any profile" when there are none (§6.10).
7. A "derived" section listing mirror and class-constant property counts, collapsed by
   default, and a "level, rank and vitals are set by the server" note (§4.8).

<a id="s10-2"></a>
### 10.2 Spawn overlay (explorer world map)

Decision D6, per visible landblock:

1. Draw every GeneratorPoint at its map position (`coordinates.md`), coloured by its
   position set; PatrolPoint chains as polylines along type-16 links, with the start link's
   type shown by dash style.
2. For each generator with a profile, draw thin lines from the generator's centroid (the mean
   of its points) to its points; the generator itself has no position.
3. Hovering a point lists the candidate templates of §6.7 with weights or probabilities;
   clicking one opens its card.
4. Points and generators whose content layers fail for the chosen layer
   (`entity-descriptor.md` §4.5.1) are hidden unless "all layers" is on.
5. A generator that is the target of type-8 links gets a "server-defined spawns: *n*" badge
   at its points (or, without points, is listed in the landblock's side panel); no template
   is shown for those objects.

---

<a id="s11"></a>
## 11. Invariants (Confirmed on this install; tests on real data skip when `TESSERA_GAME_DIR` is unset)

- **CR1** Every 0x70 record whose `WeenieType` is `Monster`, `RealNPC`, `NPC` or
  `GameplayContainer` has a root class descending from one common library class; 38,651 records
  descend from it.
- **CR2** For every template with `Agent_Classification`, species, genus and alignment equal
  the classification record's.
- **CR3** `Agent_Genus` has exactly one bit set in every template that has it.
- **CR4** Every mirror equals its base when the base is present, except at most one
  `ForwardSource_Movement_SpeedMultiplier`.
- **CR5** Every mob and NPC object record imports exactly the DID of its `Attitude_Table`, and
  its bundle's DID item is that DID.
- **CR6** Every non-null `Generator_ProfileDID` names a record of the profile class; every
  `GeneratorProfile_ProfileDefinition` names a profile; profile expansion terminates (no cycle).
- **CR7** Every type-7 link's target is a GeneratorPoint of the same landblock (418,989 of
  418,989); its source is an instance set of the same landblock, or 0 in a cross-landblock
  half (22).
- **CR8** Every GeneratorPoint marker carries `Generator_PositionSetID`; the 0x47 `kind` equals
  the static-entity type for every static entity naming a 0x47 record.
- **CR9** In every `OneOf`, a modifier struct (repeat or placement) occurs at most once and
  only as element 0; every `Entry` in a `OneOf` has `GeneratorProfile_Weight` ≥ 1.
- **CR10** `RepeatMinTimes` ≤ `RepeatMaxTimes`; `RandomPlacementMinDist` ≤ `…MaxDist`.
- **CR11** Every 0x14 record decodes to its last byte with §7; zone keys equal their indices.
- **CR12** No mob or NPC template carries `Advancement_Level`.
- **CR13** A template store built by D1 (§3.2.2) gives back, for every template, exactly the
  properties and values of its stored set (38,651 of 38,651), including the one mirror that
  differs from its base. A lookup of `Movement_SpeedMultiplier` on any of the 36,928 templates
  whose set holds only its `ForwardSource_` mirror answers absent (dictionary default), never
  the mirror's value.
- **CR14** Every `OneOf` directly in a profile holds either `Entry` choices only (after an
  optional modifier at element 0) or `PositionSetEntry` choices only (no modifier); in the
  second case all its `PositionSetEntry` elements name one position set, and each of their
  `OneOf`s has a `Repeat` with minimum = maximum at element 0 (§6.5).
- **CR16** For every generator profile, the object form read with the attribute hashes of
  §6.6.1 gives the same tree as the property set under the correspondence of §6.6 (4,168 of 4,168); a class-1140 node exists for exactly the
  repeats that are not 1 to 1 (1,060).
- **CR17** For every attitude record and the class-977 record, each list's object form (map
  from filter type to `Default` integer and exception-key set) equals the list built from the
  property set with the §5.3.1 key rule (233 of 233 lists).
- **CR18** The species-to-genus map of `ClassificationControl` equals, entry for entry, the
  species and genus of the classification records its property set lists (116 of 116).
- **CR19** No filter list names a filter type twice; no filter exception holds a subspecies
  without a species, and none holds no field.
- **CR15** Every colour-event record holds exactly 22 array events, one per key 1–22 of the
  event-type mapper; a top-level `AIColorEventTable_Event`, when present, is kept besides the
  array (§5.5, D7).

---

<a id="s12"></a>
## 12. Worked synthetic examples

All values below are invented for tests; none is game data.

<a id="s12-1"></a>
### 12.1 Authored subset of a three-record class

Class *K* has three templates. Their sets (property: value):

| | T1 | T2 | T3 |
|---|---|---|---|
| `Health_MaxLevel` | 10 | 10 | 10 |
| `AI_PerceptionRadius` | 7 | 12 | 7 |
| `AI_PerceptionRadius_ForwardSource` | 7 | 12 | 7 |
| `Combat_CanBlock` | true | true | true |
| `Combat_CanBlock_Cached` | true | true | true |
| `Quest_RelatedQuest` | absent | 0x70001234 | absent |
| `Movement_SpeedMultiplier` | absent | 2.0 | absent |
| `ForwardSource_Movement_SpeedMultiplier` | 1.0 | 1.0 | 1.5 |

Result: `Health_MaxLevel` and `Combat_CanBlock` are class-constant; the `_ForwardSource`,
`_Cached` and `ForwardSource_` properties are mirrors; `AI_PerceptionRadius`,
`Quest_RelatedQuest` and `Movement_SpeedMultiplier` are varying (the last two because they
are not present in every record). Mirrors: `AI_PerceptionRadius_ForwardSource` and
`Combat_CanBlock_Cached` equal their bases in every record and are derived.
`ForwardSource_Movement_SpeedMultiplier` is stored in all three: in T1 and T3 because its base
is absent, in T2 because it differs from its base (1.0 against 2.0).

The store keeps {`Health_MaxLevel` 10, `Combat_CanBlock` true} for *K*;
{`AI_PerceptionRadius` 7, `ForwardSource_Movement_SpeedMultiplier` 1.0} for T1;
{`AI_PerceptionRadius` 12, `Quest_RelatedQuest` 0x70001234, `Movement_SpeedMultiplier` 2.0,
`ForwardSource_Movement_SpeedMultiplier` 1.0} for T2; and {`AI_PerceptionRadius` 7,
`ForwardSource_Movement_SpeedMultiplier` 1.5} for T3. Lookups on T2:
`AI_PerceptionRadius_ForwardSource` answers 12 (derived from the base);
`ForwardSource_Movement_SpeedMultiplier` answers 1.0 (stored), **not** the base's 2.0;
`Movement_SpeedMultiplier` answers 2.0. On T1 and on T3, `Movement_SpeedMultiplier` answers
absent, so the dictionary default applies: on T3 it is **not** the stored mirror's 1.5, and on
T1 it is not the mirror's 1.0 either, even though the two values may coincide.
`ForwardSource_Movement_SpeedMultiplier` answers 1.0 on T1 and 1.5 on T3 (stored).

<a id="s12-2"></a>
### 12.2 Candidates at a spawn point

Profile P1 (synthetic DIDs): `GeneratorProfile_Profile` = [RegenType 4; OneOf [Entry {WSLEntity
0x70000A01, Weight 3}, Entry {WSLEntity 0x70000A02, Weight 1}]; PositionSetEntry {PositionSetID
12, Entry {ProfileDefinition P2, Probability 500}}]. Profile P2: [Entry {WSLEntity 0x70000A03}].

Generator G names P1 and links to points S1 (position set 12) and S2 (position set 40).

- S2: candidates 0x70000A01 (weight 3) and 0x70000A02 (weight 1); the `PositionSetEntry` is
  skipped (12 ≠ 40).
- S1: the same two plus 0x70000A03 with probability 500 ‰ (through P2).
- Under D3, at S1 the `OneOf` yields 0x70000A01 with chance 3/4, and 0x70000A03 is added with
  chance 1/2.

**Case 2: a `OneOf` of position-set entries, and a leading modifier (D3 rules 2.3 and 2.4).**
Profile P3 = [OneOf [PositionSetEntry {PositionSetID 12, Weight 1, OneOf [Repeat {2, 2},
Entry {WSLEntity 0x70000A04, Weight 1}, Entry {WSLEntity 0x70000A05, Weight 3}]};
PositionSetEntry {PositionSetID 12, Weight 1, OneOf [Repeat {1, 1}, Entry {WSLEntity
0x70000A06, Weight 1}]}; PositionSetEntry {PositionSetID 40, Weight 2, OneOf [Repeat {1, 1},
Entry {WSLEntity 0x70000A07, Weight 1}]}]; OneOf [RandomPlacement {MinDist 2, MaxDist 5},
Entry {WSLEntity 0x70000A08, Weight 1}, Entry {ProfileDefinition P2, Weight 1}]]. (The third
`PositionSetEntry` names another set than the first two; this install has no such `OneOf`,
but a reader must handle it.)

- At S1 (set 12) the eligible choices of the first `OneOf` are the first two
  `PositionSetEntry` elements (weight sum 2; the set-40 element is removed before the draw and
  its weight 2 is not counted). Each is drawn with chance 1/2. The first gives two independent
  draws from its own `OneOf`: each is 0x70000A04 with chance 1/4 and 0x70000A05 with chance 3/4,
  so two 0x70000A05 with chance 9/16, one of each 6/16, two 0x70000A04 1/16. The second gives one
  0x70000A06. All of them stand at S1 itself (no placement applies to this `OneOf`).
- At S2 (set 40) the only eligible choice is the third element, drawn with chance 1 (not 2/4):
  one 0x70000A07.
- At a point of set 99 the first `OneOf` produces nothing.
- At every point the second `OneOf` produces one object: 0x70000A08 (chance 1/2) or, through
  P2, 0x70000A03 (chance 1/2); either is placed at a uniformly random point between 2 and 5
  units from the point, because the leading `RandomPlacement` applies to everything the
  `OneOf` produces, including what P2 produces.
- So at S1 one evaluation produces 2 or 1 objects from the first `OneOf` plus 1 from the
  second: 3 objects with chance 1/2, 2 with chance 1/2.

<a id="s12-3"></a>
### 12.3 A minimal encounter-table record (50 bytes)

DID 0x14000099, one zone with the 4-byte label "Main", one slot:

| Offset | Bytes (hex) | Field |
|---|---|---|
| 0 | 99 00 00 14 | own DID |
| 4 | 00 00 00 00 | 0 |
| 8 | 06 00 00 00 | block-map key 6 |
| 12 | 01 00 00 00 | *n* = 1 |
| 16 | 00 00 00 00 | zone key 0 |
| 20 | 04 4D 61 69 6E | label, length 4 |
| 25 | 01 00 00 00 | *m* = 1 |
| 29 | 00 00 00 00 | slot index 0 |
| 33 | 00 00 00 00 | 0 |
| 37 | 00 00 80 3F | 1.0 |
| 41 | 01 00 00 00 | 1 |
| 45 | 00 00 00 00 | 0 |
| 49 | 00 | 0 |

Total 50 bytes: 16 header bytes, a 4-byte zone key, a 5-byte label (1 length byte and 4
characters), a 4-byte slot count and one 21-byte slot. A 12-byte label instead gives 58
bytes, the size of each of the four one-zone, one-slot records of the install.

<a id="s12-4"></a>
### 12.4 Attitude evaluation (§5.3.1)

Synthetic values. Species-to-genus table: species 40 → genus 0x8; species 41 → genus 0x8.
Attitude table *T*:

- aggressive list: one filter, type 5, `Default` true, one exception {alignment 2};
- friendly list: one filter, type 2, `Default` absent, one exception {species 40}.

Exception keys: {alignment 2} gives 0x0000000000000002; {species 40} first takes the genus
0x8 of species 40 and gives 0x0000000802800000 (genus 0x8 in bits 32–63, species 40 =
0x028 in bits 20–31).

| Other entity (*f*, *g*, *s*, *u*, *a*) | Aggressive list | Friendly list | Result |
|---|---|---|---|
| (5, 0x1, 0, 0, 1) | filter 5; no key matches, so `Default` true | not consulted | Aggressive |
| (5, 0x1, 0, 0, 2) | filter 5; key (0, 0, 0, 2) matches, so not true | no filter for type 5 | Neutral |
| (2, 0x8, 40, 7, 1) | no filter for type 2 | filter 2; genus 0x8 agrees with species 40; key (0x8, 40, 0, 0) matches, so not false | Friendly |
| (2, 0x4, 40, 0, 1) | no filter for type 2 | filter 2; genus 0x4 differs from species 40's genus 0x8, so `Default` (false) | Neutral |
| (2, 0, 41, 0, 1) | no filter for type 2 | filter 2; genus becomes 0x8; no key matches (species 41) | Neutral |
| (0, 0x8, 40, 0, 1) | not consulted (*f* = 0) | not consulted | Neutral |

---

<a id="s13"></a>
## 13. Open questions

Item numbers are kept; closed items say where the answer is.

1. *Closed 2026-10-08:* classes 1653 and 1654 are the avatar templates (§2.1, C22).
2. Link-schema `flag` (§3.3; server or authoring tool only). *Closed 2026-10-08:* link type
   13 is `PathMapTest` (§3.3, C24); the root-object reference census (§3.1, C23).
3. Whether the original server or authoring tool enforces the instance-property list (§3.4;
   the offline client does not consult it).
4. *Closed 2026-10-08:* the creature panel is the analyse-skill panel (§4.8, C9a).
5. Action-table and action selection rules and the time unit of cooldowns (§5.1; server
   only). *Closed 2026-10-08:* attitude evaluation (§5.3.1, C27–C30). New, narrower: how the
   original fills the `Interaction_Permission_*` values and how its harmful-interaction check
   combines the two directional results (§5.3.1).
6. *Closed 2026-10-08:* profile bundle classes (§6.6, C16).
7. The original server's spawn, placement, respawn and AI-action rules, and the frame of
   exact placements (§6.8; server only).
8. Patrol branching and end behaviour, and the meaning of `PatrolLink_Speed` (§6.9; an owner
   observation of the original could settle the end behaviour and the speed).
9. Encounter-table slot fields (§7; a further M4 pass could name them; not needed).
10. Where the templates of type-8-linked spawns come from (§6.10): not in the client data; a
    server must supply them.
11. Which colour event the original server uses when a record holds a top-level event besides
    its array (§5.5; server only).
12. Whether a base-less `ForwardSource_Movement_SpeedMultiplier` acts as the creature's speed
    factor in the original server (§3.2.1; the client does not read it).

---

<a id="s14"></a>
## 14. Notes for other specs (not edited here)

| Spec | Note |
|---|---|
| `entity-descriptor.md` §4.5.2 | `Generator_ProfileDID` occurs in **34,275** instance sets without a static entity (recounted 2026-10-07 over all 514,987), not 33,902; with the 20 GeneratorPoint and 1 PatrolPoint instance sets, 34,296. |
| `landblock.md` §4.6 | `end_id_1` is the link's target and `end_id_2` its source (§3.3 here); link ids are landblock-local serials (§6.3). §4.8 "server-side object templates": they are per-instance property sets without template DIDs (§6.2). |
| `setup.md` §4.3 | `kind` equals the static-entity type of every static entity naming the record (§6.4), answering its OPEN for the observed values. |
| `wstate.md` §6, §7 | Direction 0/1 = source/target end (§3.3); for templates the available list names per-instance properties (§3.4). |
| `environment.md` §4 | The 0x14 encounter-table layout is decoded in §7 here. |

---

<a id="s15"></a>
## 15. Change log

| Date | Sections | Change | Author |
|---|---|---|---|
| 2026-10-07 | all | First version, status Review. Census over all 38,651 templates, all AI records, all 4,168 profiles, all 49,636 landblock-info records and the five encounter tables; one BIN use (creature information panel). | Spec writer (dirty room) |
| 2026-10-07 | header, §1 (OBS, C1, C17, C17a, C21), §2.1, §3.1, §3.2.1, §3.2.2, §3.3, §5.5, §6.5, §6.7, §6.8, §9, §11 (CR13–CR15), §12.1–§12.3, §13 | Review fixes, every count re-measured over all records on 2026-10-07. (1) §12.3 headed 50 bytes, total explained. (2) 383,332 records with prefix 0x70, the other 11 object records are 0x76 singletons. (3) §5.5: the top-level colour event is in addition to the full array; counts are of the 19,426 array events (19,427 with it); Decision D7 and an OPEN added. (4) D1 stores every mirror whose base is absent or differs; §12.1 covers it; CR13. (5) D3 step 2 rewritten as one rule (non-matching position-set choices are removed before the draw; a leading `Repeat` repeats the whole draw; a leading placement applies to everything the `OneOf` produces; 256-object cap); §6.5 census of `OneOf` shapes; §12.2 case 2; CR14. (6) §3.3: type 13 is two entries (direction 1 then 0), terminator 0 per `wstate.md` §6. (7) C17 split into Reported (LC) and Decision (C17a); the weight and per-mille reading moved into the §6.8 OPEN as a hypothesis. Also corrected: §3.1 link section is absent in 147 + 2 records, not 163 (163 is the available-property count); §3.3 container, 4026 and 5785 schemas completed. | Spec writer (dirty room) |
| 2026-10-07 | §3.2.1, §15 | Second pass on the review issues: every disputed count re-measured over all records with fresh probes and found as now stated (383,332 + 11 object records; 19,426 array colour events + 1 top-level, 97,130 + 8 data entries, 1,382 + 8 sounds; 1,952 `PositionSetEntry` choices in 888 `OneOf`s, all single-set; encounter records 1,687 and 4 × 58 bytes, 70 slots; D1 store rebuilds 38,651 of 38,651 sets, storing 185,059 mirrors; §3.3 schema and type-13 pair counts; all §6.5 counts, ranges and the 81-object maximum). §3.2.1: mirror totals and per-class mirror counts stated for every class. No other count in the reviewed sections changed. | Spec writer (dirty room) |
| 2026-10-07 | §1 (C4, C21), §3.2.1, §3.2.2, §6.9, §11 (CR13), §12.1, §13 | Second-review fixes. (A) One rule for base-less mirrors: the §3.2.1 sentence letting a mirror stand for its base is replaced by the D1 lookup rule (a base the set does not hold answers absent, never from a mirror); stated in §3.2.1 and §3.2.2, covered by C21, tested by CR13 and shown in §12.1 (T3 now has a differing synthetic mirror). New facts, measured over all 185,058 base-less occurrences and all 27,546 dictionary definitions: seven of the eight base-less mirror names have no base in the dictionary, each holds a single value, the hunter base has two base-less mirrors with equal values, and only `Movement_SpeedMultiplier` has a definition (mirror 1.0 in 36,925 of 36,928 sets); new OPEN (§3.2.1, §13 item 12). (B) §6.9: the `PatrolLink_Speed` reading moved into an OPEN as a hypothesis; §13 item 8 extended. | Spec writer (dirty room) |
| 2026-10-08 | header, Purpose, §1 (OBS2, BIN2, C9a, C16, C17a, C21–C30), §2.1, §3.1, §3.2.1, §3.3, §3.4, §4.8, §5.1, §5.3, §5.3.1 (new), §5.5, §6.6, §6.8 (D3 step 8), §6.9, §7, §8, §9, §11 (CR16–CR19), §12.4 (new), §13 | OPEN-closing revision; status Revised. **Closed:** §2.1 (classes 1653 and 1654 are the avatar templates, with their referrers); §3.1 (census of the root object's reference attributes); §4.8 (the panel is the analyse-skill panel); §5.3 (the attitude evaluation rule, read from the class library's code and confirmed against the object form of all 233 lists; new §5.3.1 with the species-to-genus table, the 64-bit exception key and Decision D8); §6.6 (role of every profile bundle class; object form equals the property set in all 4,168 profiles); §3.3 in part (link type 13 is `PathMapTest`). **Changed:** D3 step 8 now uses the §5.3.1 rule (aggressive before friendly; the earlier sandbox rule let friendly lists win). **Made precise, still OPEN:** §3.2.1, §3.3 flag, §3.4, §5.1, §5.5, §6.8, §6.9 (two), §7, and a new narrower §5.3.1 item (how the `Interaction_Permission_*` inputs are filled and how the two directions combine): each now says which method was tried and why it cannot settle the question (server-side, or a further M4 pass for §7). Second M4 use recorded as BIN2. | Spec writer (dirty room) |
| 2026-10-08 | header, §1 (marks key: Inferred; OBS3; C16, C22), §2.1, §5.3.1 (Filter, Key), §6.6.1 (new), §8, §11 (CR16) | Fixes for the review of 2026-10-08, every fact re-measured with fresh probes over all records (OBS3). (A) §2.1 and C22: each avatar record is named by exactly one *kind* of referrer, not by exactly one referrer; new multiplicity table (126 records by one referrer record, 6 by two, 4 by three, 2 by seven; one record per class named twice inside one race-table record). (B) §8: the link-type mapper is labelled with its EMAPPER label `EntityLinkType`; the earlier label had no source in the install. (C) New §6.6.1 with the attribute name hash of every field of every profile node class, including the five constant fields, so that CR16 can be tested from the spec. (D) §5.3.1: *D* is the stored value when present (true in all 351); new rule for an exception whose species has no genus in the table (0 of 549 here; it can never change a result, so it may be dropped or kept). Minor: `Inferred` defined in the §1 marks key. Status stays Revised until a narrow re-review. | Spec writer (dirty room) |
| 2026-10-08 | header | Narrow re-review of the fixes for review items A–D (separate session, not the author): passed; status Accepted. No content changed. | Spec reviewer |
