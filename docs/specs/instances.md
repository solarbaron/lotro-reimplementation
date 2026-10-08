<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Instanced and group content: private encounters, skirmishes, world join, big and distant battles, session play, fellowship maker, phasing boxes

| | |
|---|---|
| **Status** | **Accepted** (2026-10-07) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review session (spec reviewer, not the author session) |
| **Review record** | 2026-10-07: first review and re-review (separate sessions) requested changes; both rounds answered by the writer (§19). Second re-review of the second revision, 2026-10-07 (a session separate from the writer and the earlier reviewers): **passed**, Accepted. Details in the dirty-room review log (decision 0018). |
| **Implements into** | `crates/formats` / `crates/props` (typed views of the records below, read from their property sets), `crates/wstate` (the object-state forms of §13, optional), `apps/explorer` (instance cards, §14.4), `apps/client` (the instance-finder lists of §7 and §15.1, the skirmish statistics panel of §6.4 and §15.1, the distant-battle unit display of §8.4 and §15.1); a later server or sandbox backend (`crates/net`) consumes §15.2 |
| **Companion specs** | [`property-system.md`](property-system.md) §3–§5 (type codes, value encodings, property sets), §6.2 (the +0x09000000 rule), §7 (enum and bitfield labels), §8 (DID maps); [`wstate.md`](wstate.md) §3–§5 (object-state records, root class at offset 4, native codecs); [`wlib.md`](wlib.md) (class indices, ancestry); [`quests.md`](quests.md) §7.1 (roles, `QuestDispenser_PrivateEncounterTemplate`), §7.9 (scene records, class 1468), §8.1–§8.3 (permission struct and quest requirements), §9.6 (medals), §14 (world events, world-event lists); [`map-data.md`](map-data.md) §3 (named places, place-key lookup), §10 (properties that name places), §11 (ripcords); [`economy.md`](economy.md) §4.7, §4.10, §4.13 (species and skirmish loot); [`creatures.md`](creatures.md) §2.1 (class 4026, distant-battle unit groups); [`sound.md`](sound.md) (sound-info records, prefix 0x2A); [`misc-records.md`](misc-records.md) §13.5 (property-response maps) |

---

<a id="purpose"></a>
## Purpose

The game data holds the rules for instanced and group content:

- **private encounters** (instances): templates with their landblocks, content layer, start,
  exit and success positions, timers, capacity, quests, permissions, loot and award tables;
  the control record with lock sizes, lock reset days and entry filters;
- **skirmishes**: skirmish templates (difficulty tiers, group sizes, level scaling), match
  parameters for loot look-up, the skirmish control record (point multipliers, group tiers,
  trait gates, statistics definitions), and the skill-trainer trait lists;
- **world join** (the instance finder): the list of offered templates, featured instances,
  instance groups, seasonal and capped categories, tickets;
- **big battles and distant battles**: the big-battle control record (medals, quest points,
  quest-to-encounter map), distant-battle scripts (waves, objectives, weighted unit lists),
  distant-battle units (cosmetic formations of many entities), the formation control record
  and battle statistics;
- **session play**, the **fellowship maker** (group finder) and **phasing boxes**.

This spec gives an implementer:

1. the census of every record class of the family and of the dictionary definitions (§2);
2. the layout of every class as property-set field tables with the presence, value ranges
   and DID targets found in **all** records of the install (§4–§12);
3. the object-state forms and the correspondences confirmed between the two forms (§13),
   including two that hold data the property form does not (the entry-filter windows of
   §5.2 and the fellowship-maker index of §10.2);
4. the reference graph in and out of the family and an algorithm for the explorer's instance
   cards (§14);
5. what the client itself does with these records (§15.1, BIN) and a precise list of what
   only a server decides (§15.2, `> **OPEN:**`).

Records are identified by DID and by **root class** (the class index at offset 4 of the 0x70
record, `wstate.md` §3.1). Fields are identified by their **property label** from the master
property dictionary (`property-system.md` §2). Labels, enum-mapper labels and WEENIECONTENT
labels are needed for interoperability: the client and every tool look records and
properties up by them. PIDs, class indices and DIDs quoted here are observations of this
install and must be read at run time, never hard-coded. No game text appears here.

---

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install with throwaway probes in the dirty-room
  scratch directory, over **every** record of the class concerned.
- **Reported**: comes only from published third-party material.
- **BIN**: read from the client executable under method M4 (decision 0011).
- **Derived**: a rule found by this spec writer by comparing data, then checked over all
  records (the check is Confirmed).

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam, client data current) on 2026-10-07, read-only. Probes used the project's own decoders (`tessera-dat`, `tessera-props`, `tessera-wstate`, `tessera-formats`, from the commit current that day) as tools. Covered: the root class of all 383,343 object records of `client_gamelogic.dat` (383,332 with prefix 0x70, 11 singletons with prefix 0x76); every property set with prefix 0x79 (383,332) and 0x78 (1,616), all decoding with no failure; the object state of every record of every class of §2.1 (all decode, none partial); the property dictionary (0x34000000, 27,546 definitions); the class library (0x56000000); the WEENIECONTENT DID map (0x2800000E, 316 entries); the EMAPPER map (0x28000001); every enum mapper named in §3.3; the content-layer DID map 0x28000012; all 49,636 landblock-info records of the cell archives (static entities, links, instance property sets). **Revision probes** (same install, same day, after the first review): the census sums of §2.1; the flag byte of all 4,628 class-161 positions of the templates; the attribute order of the world-join object form; the order and kind bits of all 557 fellowship-maker entries against the templates; the top-level property counts of all 96 class-3725 records; the DB-object-type code and target of every DID value in the records of §2.1; the mapper or DID map of every field of §3.3; the holders of every characteristic property of §3.4 over all 0x79 and 0x78 sets; the labelled templates, manager entities and trait lists of §4.12, §5.3 and §6.2; all record pairs of class 2727. **Second revision probes** (same install, same day, after the re-review): every `IntroVO` value of every 0x70 record's property set; the stored group order of the world-join control against its 164-element object-form array and against the template order of the 557 fellowship-maker entries; the key set and bitsets of the fellowship-maker flag-bit map; the key sets of all 735 enum mappers of the install's archives. |
| LC | LotroCompanion `lotro-tools` (no licence file; all rights reserved): the private-encounter and instance-tree loaders. Read for facts only (which class is a template, which properties name the instance-finder lists). No code or prose copied. |
| BIN | **M4 (decision 0011).** Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (verified 2026-10-07), read in the dirty room on 2026-10-07 with a headless, read-only copy of the shared Ghidra project. **Questions:** (1) which properties of this family the client reads at all; (2) how the client chooses the featured instance, the level-scaling level and the template of an instance group; (3) how the skirmish statistics panel computes its derived values; (4) how the client builds the cosmetic display of a distant-battle unit; (5) what the phasing-box flags do on the client. **Method:** a whole-program search for the PIDs of all 1,322 dictionary properties of the family prefixes (§2.2) as immediate operands, then decompilation of the functions that hold property look-ups (a PID passed as the second argument of a property-read call). **Why M1–M3 were not enough:** the data cannot show which fields the client consumes or how it combines them; no published source covers these questions; a running client shows only the result on screen. Only user-interface, data-loading and entity-display code was read. Network messages, login, launcher and anti-tamper code were not read; the world-join request properties of §7.4 were deliberately left alone because they belong to client–server messages (decision 0004). |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| I1 | Census of the family: classes, record counts (1,833 in all; 1,821 in the 17 rule-bearing classes), sizes; every record has a decodable property set and object state (§2.1) | OBS | Confirmed (sums re-checked by the revision probe) |
| I2 | Dictionary definitions per prefix, and how many are stored in any record (§2.2) | OBS | Confirmed |
| I3 | WEENIECONTENT labels of the family and their records (§2.3) | OBS | Confirmed |
| I4 | Family properties carried by records of other classes and by landblock content (§2.4, §14.3) | OBS | Confirmed |
| I5 | Templates are root classes 1368 and 2651 (2651 a subclass of 1368) (§4) | OBS; LC (class roles) | Confirmed |
| I6 | Template field tables (§4.2–§4.11) | OBS | Confirmed (layout and counts); meanings marked per row |
| I7 | Landblock-list encoding: (region, 8 × landblock X, 8 × landblock Y); the object form stores landblock = ⌊value ÷ 8⌋ (§4.3) | OBS; LC (division by 8) | Confirmed (1,511 of 1,511 templates, 4,628 values). Derived by this spec writer. |
| I8 | Template content layer is a key of DID map 0x28000012 and appears in a listed landblock's static content for 1,475 of 1,487 templates with landblocks (§4.4) | OBS | Confirmed |
| I9 | Template kinds of class 2651 (missions, world-join encounters, others) (§4.1) | OBS | Confirmed. Derived by this spec writer. |
| I10 | Private-encounter control: lockable list = templates with `LockSize`; lock reset days are a day-of-week bitfield (§5.1) | OBS | Confirmed |
| I11 | Entry filters: window and limit live only in the object form (§5.2) | OBS | Confirmed (layout); Derived (meaning of the numbers is OPEN) |
| I12 | Match-parameter bands, including which level bands overlap and that no two records share a group band and a tier band with overlapping level bands (§6.1) | OBS | Confirmed (183 records, all 16,653 pairs) |
| I13 | Skirmish control layout (§6.3) and statistic naming (§6.4) | OBS | Confirmed |
| I14 | World-join control: available list, groups, featured list, seasonal and capped categories; object form = ordered list + index map, grouped templates by ascending group label (§7.1, §13.3) | OBS; LC (meaning of the lists) | Confirmed (layout and object form; label-ascending order 164 of 164 positions, stored group order 156 of 164); Reported (meaning) |
| I15 | Big-battle control, distant-battle scripts, distant-battle units and formation control (§8) | OBS | Confirmed |
| I16 | Distant-battle unit: composition = units repeated by offset count; offsets lie inside the formation dimensions; dimension width ≤ the width class's actual width (§8.4) | OBS | Confirmed (96 of 96). Derived by this spec writer. |
| I17 | Session-play control, fellowship-maker control, phasing box, dialog singletons (§9–§12) | OBS | Confirmed |
| I18 | Fellowship-maker object form: 557 candidate entries, their order, flag word bits (kind bits disjoint in this install), per-flag bitsets, encounter → size → tier → entry map (§10.2) | OBS | Confirmed (557 of 557 entries, 107 of 107 templates in the stated order, 20 of 20 bitsets, key set of the flag-bit map 20 of 20; the rule behind the key set is OPEN). Derived by this spec writer. |
| I19 | Object-form correspondences of templates and controls, including the flag bytes of the template landblock positions and the stored attribute order of the world-join control and the label-ascending order of its grouped templates (§13) | OBS | Confirmed (all records of each class; 4,628 of 4,628 positions; 164 of 164 finder positions). Derived by this spec writer. |
| I20 | Reference graph (§14.1–§14.3) | OBS | Confirmed |
| I21 | DID targets: declared DB-object-type codes of the DID fields, the fields that are not type 0x45, and that every non-zero 0x45 value names an existing 0x70 record (§3.2) | OBS | Confirmed (all DID values of the records of §2.1; 0 dangling; `IntroVO` 942 values, 908 non-zero, all prefix 0x2A) |
| I22 | Enum sources: the mapper or DID map named by each field's definition, and the number of names of each (§3.3) | OBS | Confirmed (every row of §3.3) |
| I23 | Characteristic properties and their holder classes (§3.4) | OBS | Confirmed (all 0x79 and 0x78 sets) |
| I24 | Labelled templates `NewbieCinematicEncounter` and `PublicAreaDefaultEncounter` (§4.12) | OBS | Confirmed |
| I25 | Manager entities: ancestry, property-set size, object form (§5.3) | OBS | Confirmed (3 of 3) |
| I26 | Skill-trainer trait lists: sizes, targets, object form, NPC holders (§6.2) | OBS | Confirmed (3 of 3 lists, 25 NPCs) |
| B1 | Featured instance = featured list entry number *n*, *n* from a world property (§15.1) | BIN | BIN only |
| B2 | Level-scaling level and its world-property override (§15.1) | BIN | BIN only |
| B3 | Instance group + group size → template (§15.1) | BIN | BIN only |
| B4 | Derived skirmish statistics and best-time aggregation (§15.1) | BIN | BIN only; the stored data it reads is Confirmed |
| B5 | Distant-battle unit display: fields read, defaults, removal list, bounding box (§15.1) | BIN | BIN only |
| B6 | Phasing-box flags on the client (§15.1) | BIN | BIN only |
| B7 | 253 of the 330 stored family properties are never named in the client, among them the template timers, landblock lists, content layer, lock sizes and reset days, distant-battle script fields and formation parameters (§15.1) | BIN (negative search) | BIN only; see the caveat in §15.1 |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Object record** | A record with DID prefix 0x70 (`wstate.md` §2). Its root class (offset 4) says what kind of record it is. Every record of this spec has prefix 0x70. |
| **Property form** | The record's property set at DID + 0x09000000 (prefix 0x79, `property-system.md` §6.2). Present and decodable for every record of this spec; treated as authoritative where it is not empty. |
| **Object form** | The script-object graph in the 0x70 record's bundle (`wstate.md` §4). For most classes a compiled copy of part of the property form; for some classes (§5.2, §8.7, §10.2, §12) the only place the data lives. |
| **Template** | A private-encounter template: a record of root class 1368 or 2651. One template describes one instance that the server can create. |
| **Instance** | A run-time copy of a template's world (its landblocks, under its content layer) created by the server for one player, a group or the public. Run-time state is not in the data. |
| **Place key** | A string naming a position in the named-places table (`map-data.md` §3). Templates name their start, exit and success positions by place key. |
| **Landblock** | A 192 m square of terrain addressed by region, X and Y (`landblock.md`). |
| **Content layer** | A key of the DID map 0x28000012 (label `ContentLayer`). Static entities of a landblock carry a list of layers they belong to (`Entity_ContentLayers`, same map); an instance shows the entities of its layer (§4.4). |
| **Group size** | A key of the enum mapper labelled `SkirmishGroupSize`: 1 solo, 2 duo, 3 three players, 6 six, 12 twelve, 24 twenty-four (the key is the player count). |
| **Difficulty tier** | A key of the enum mapper labelled `SkirmishDifficultyTier`: 1 … 10. |
| **Absent field** | A property not stored. Its value is the dictionary default of the property (`property-system.md` §2.2) when one exists; otherwise "none". This spec lists the defaults that matter. |

Implementations identify a record's kind by its characteristic property (§3.4) or its root
class, find the singletons by their WEENIECONTENT labels (§2.3), read PIDs from the
dictionary by label, and never hard-code DIDs.

---

<a id="s2"></a>
## 2. Census (Confirmed, all records)

<a id="s2-1"></a>
### 2.1 Record classes (1,833 records in 29 root classes)

"0x70 bytes" and "0x79 bytes" are the total decompressed sizes of the object records and of
their property sets. Every record below has a property set (some empty) that decodes with no
leftover bytes, and an object state that decodes completely.

| Root class | Records | 0x70 bytes | 0x79 bytes | Role (§) |
|---|---|---|---|---|
| 1368 | 959 | 641,099 | 317,319 | Private-encounter template (§4) |
| 2651 | 552 | 506,245 | 539,505 | Skirmish encounter template, subclass of 1368 (§4) |
| 2727 | 183 | 6,588 | 14,274 | Skirmish match parameters (§6.1) |
| 3725 | 96 | 14,396 | 765,590 | Distant-battle unit (§8.4) |
| 3808 | 14 | 1,166 | 12,576 | Distant-battle script (§8.3) |
| 2669 | 3 | 725 | 1,150 | Skirmish skill-trainer trait list (§6.2) |
| 2573 | 2 | 144 | 100 | Entry filter, first kind (§5.2) |
| 2576 | 2 | 126 | 76 | Entry filter, second kind (§5.2) |
| 3894 | 2 | 72 | 60 | World-join ticket (§7.2) |
| 1366 | 1 | 429 | 776 | Private-encounter control (§5.1) |
| 2515 | 1 | 11,268 | 14,078 | Skirmish control (§6.3) |
| 2606 | 1 | 2,092 | 1,742 | World-join control (§7.1) |
| 3787 | 1 | 1,877 | 7,176 | Big-battle control (§8.1) |
| 3745 | 1 | 113 | 240 | Distant-battle unit formation control (§8.5) |
| 1925 | 1 | 753 | 2,708 | Session-play control (§9) |
| 3276 | 1 | 25,539 | 374 | Fellowship-maker control (§10) |
| 3369 | 1 | 109 | 432 | Phasing box (§11) |
| 3423 | 1 | 36 | 6 (empty) | Session-play level directory (§9) |
| 3576 | 1 | 312 | 6 (empty) | Battle-statistic control (§8.7) |
| 1367, 2619, 3136 | 1 each | 63 each | 509, 495, 507 | Server-side manager entities (§5.3) |
| 1766, 1769, 1772, 2753, 2757, 2759, 3060 | 1 each | 36 each | 6 each (empty) | Dialog callback singletons of this family (§12) |
| **Total** | **1,833** | **1,213,530** | **1,679,741** | |

Also part of the family but counted elsewhere or shared:

| Root class | Records | Role |
|---|---|---|
| 3590, 3622 | 10, 3 | Battle statistics (§8.7); 98 bytes each |
| 3399 | 6 | A generic DID-set class; every record has a WEENIECONTENT label; one is the `BigBattles` set (§8.2) |
| 4026 | 16 | Distant-battle unit groups (`creatures.md` §2.1); they carry the unit properties of §8.4 |
| 3317, 3331 | 1 each | Fellowship-maker and fellowship-vote dialogs (§12) |
| 2728, 2729 | 8,326, 7 | Skirmish loot look-ups (`economy.md` §4.10) |

The 1,821 records of the 17 classes 1368, 2651, 2727, 3725, 3745, 3808, 2669, 2573, 2576,
3894, 1366, 2515, 2606, 3787, 1925, 3276 and 3369 hold 1,212,741 bytes of 0x70 records and
1,678,176 bytes of property sets.

<a id="s2-2"></a>
### 2.2 Dictionary definitions (Confirmed)

Definitions are grouped by the label part before the first underscore. "Stored" counts the
definitions that occur as a top-level or nested property in at least one property set of the
install (0x79, 0x78, landblock content); the others are run-time state that only a server or
a running client fills, or are referenced only as PID values (for example the 228 statistic
properties of §6.4).

| Prefix | Definitions | Stored | Notes |
|---|---|---|---|
| `SkirmishStats` | 249 | 7 | 228 per-statistic properties (§6.4) and their helpers |
| `Skirmish` | 228 | 81 | template fields (§4.10), skill trainer (§6.2), control-point and soldier properties of placed entities (§14.3) |
| `BigBattle` | 194 | 41 | mostly entity state (`BigBattle_Interactive_*` on war machines, commander and vanguard state) |
| `PrivateEncounterTemplate` | 89 | 81 | §4 |
| `PrivateEncounter` | 81 | 32 | control (§5.1), entry filters (§5.2), properties of other records (§2.4) |
| `DistantBattleUnit` | 65 | 46 | §8.4, §8.5 |
| `PEMessageSystem` | 50 | 4 | instance-script messaging; four `PointsPer…` properties on creatures |
| `SkirmishControl` | 49 | 42 | §6.3 |
| `Instance` | 44 | 12 | per-instance boss-mechanic state (`Instance_<instance>_…`) |
| `BigBattles` | 38 | 0 | user-interface element identifiers |
| `BigBattleControl` | 30 | 30 | §8.1 |
| `WorldJoin` | 25 | 8 | §4.10, §7 |
| `DistantBattle` | 21 | 19 | §8.3 |
| `FellowshipMakerUI` | 20 | 0 | user interface |
| `FellowshipMakerControl` | 17 | 15 | §10 |
| `BattleStat` | 10 | 6 | §8.7 |
| `WorldJoinControl`, `SessionPlay`, `FellowshipMaker`, `SkirmishMatchParams` | 9, 9, 8, 6 | 9, 8, 1, 6 | §7.1, §9, §2.4, §6.1 |
| `SkirmishTreasureLookup`, `SkirmishCustomLootLookupTable` | 6, 2 | 6, 2 | `economy.md` §4.10 |
| `Phasing`, `PhasingBox`, `PhasingBoxOverride`, `WorldJoinTicket` | 3, 2, 1, 2 | 1, 2, 0, 2 | §11, §7.2 |

`PEDesc_*` (55 definitions) is the particle-emitter family (`particles.md`), not this one,
despite the prefix.

<a id="s2-3"></a>
### 2.3 WEENIECONTENT labels (Confirmed)

The WEENIECONTENT DID map (`property-system.md` §8, 0x2800000E here) names these records
(DIDs of this install):

| Label | DID here | Class | Content |
|---|---|---|---|
| `PrivateEncounterControl` | 0x70000231 | 1366 | §5.1 |
| `PrivateEncounterServer` | 0x70000215 | 1367 | §5.3 |
| `SkirmishControl` | 0x70019DCD | 2515 | §6.3 |
| `WorldJoinControl` | 0x7001B7CE | 2606 | §7.1 |
| `WorldJoinManager` | 0x7001BA6C | 2619 | §5.3 |
| `BigBattlesControl` | 0x7003672A | 3787 | §8.1 |
| `BigBattles` | 0x7003672B | 3399 | §8.2 |
| `BattleStatControl` | 0x70030B94 | 3576 | §8.7 |
| `DistantBattleUnitFormationControl` | 0x7003583A | 3745 | §8.5 |
| `SessionPlayControl` | 0x7000A8BD | 1925 | §9 |
| `SessionPlayLevelDirectory` | 0x7002D24F | 3423 | §9 |
| `FellowshipMakerControl` | 0x7002AAC9 | 3276 | §10 |
| `FellowshipMakerManager` | 0x700255FC | 3136 | §5.3 |
| `NewbieCinematicEncounter` | 0x70032B6D | 1368 | a template (§4.12) |
| `PublicAreaDefaultEncounter` | 0x70022741 | 1368 | a template (§4.12) |
| `Dialog_PrivateEncounter`, `Dialog_LeavePrivateEncounter`, `Dialog_AbandonPrivateEncounter`, `Dialog_FinishSessionQuest` | 0x7000023A, 0x70000245, 0x70023343, 0x70000239 | 1772, 1769, 3060, 1766 | §12 |
| `Dialog_WJQueueDialogCallback`, `Dialog_WJRequestDialogCallback`, `Dialog_WJDequeueDialogCallback` | 0x7001FF70, 0x700200E6, 0x700204A0 | 2753, 2757, 2759 | §12 |
| `Dialog_FellowshipMakerConfirmationDialogCallback`, `Dialog_FellowshipVoteDialogCallback` | 0x7002BDB3, 0x7002BFD6 | 3317, 3331 | §12 |
| `Skirmish_SoldierSkill`, `Skirmish_SoldierAssistSkill`, `Skirmish_SkillFlagEffect` | 0x7001AA28, 0x7001C4C8, 0x7002CE16 | 827, 827, 718 | skill and effect records (`combat-data.md`) |

The phasing-box record (class 3369, §11) has no WEENIECONTENT label; it is the only record of
its class.

<a id="s2-4"></a>
### 2.4 Family properties on records of other classes (Confirmed, all property sets)

| Property | Holder classes (records) | Meaning (§) |
|---|---|---|
| `PrivateEncounter_RaidChestLock_PullCountBeforeLock`, `…_PullCountBeforeSoftLock`, `…_ResetDays`, `…_SoftLockEnabled` | 794 containers (1,385 each) | Per-chest loot locks (§15.2) |
| `PrivateEncounter_DestroyItemOnExit` | 796 (123), 803 (11), 798 (1) items | Item removed when the holder leaves the instance |
| `PrivateEncounter_LinkedTemplate` | 815 (3); landblock content (§14.3) | Entry point (door, portal) → template |
| `PrivateEncounter_TravelArray` / `…[]/PrivateEncounter_Template` | 1724 NPCs (57 records, 455 entries) | Instances an NPC can send a player to |
| `PrivateEncounter_CharGenDirect` | 1930 monster-play sessions (44) | Template entered directly at session start |
| `Skirmish_SkillTrainer_Skirmish` | 1724 (25) | → class 2669 trait list (§6.2) |
| `Skirmish_TraitManager_Skirmish`, `SkirmishControl_…_RequiredTrait_Override`, `Skirmish_SkillTrainer_OverrideDefault_…` | 1724 (24, 1, 1, 1) | Skirmish-trait NPC configuration |
| `Skirmish_Soldier_IsASoldierPet`, `Skirmish_Defender_ShowGroundVitals`, `Skirmish_Lieutenant_Announce_Arrival` | 1723 (56, 1, 1) | Skirmish creature flags |
| `Skirmish_TraitCostProgression`, `Skirmish_TraitEffect_PetMods` | 2525 skirmish traits (244 each) | `combat-data.md` |
| `Skirmish_PetType` | 763 (7) | |
| `BigBattle_Commander_*`, `PEMessageSystem_PointsPer…` and one instance-specific `Instance_…` property (each with a `_Cached` twin) | 1723, 1724, 1653, 1654, 4026, 5785 and the three referee classes (all 36,929) | Default state values present on every creature template (`creatures.md`) |
| `BigBattle_*` player state, `FellowshipMaker_DisallowUse`, three instance-specific `Instance_…` properties | 1653, 1654 (69 each) | Avatar-like templates (`creatures.md` §2.1) |
| `DistantBattleUnit_*` (8 properties) | 4026 (16), 2097 (5) | §8.4 |
| `Phasing_ForcePhasingRefresh_OnEnterWorld` | 1724 (1) | §11 |
| one instance-specific `Instance_…` property | 796 (10) | Instance-specific item state |

---

<a id="s3"></a>
## 3. Conventions

<a id="s3-1"></a>
### 3.1 Field tables

Each record class is given as a table of property paths. `A[]` means "each element of array
A"; `S/F` means "field F of struct S". Columns:

- **Type**: the property type code (`property-system.md` §3): 1 string, 6 vector (three
  floats), 8 enum, 9 float, 10 PID, 11 struct, 12 array, 13 StringInfo, 14 64-bit bitfield,
  15 int, 18 32-bit bitfield, 20 DID, 21 boolean.
- **Present**: in how many records (top level) or entries (inside arrays) the field is
  stored. For templates the column gives class 1368 / class 2651.
- **Observed**: value range and notable distributions; for DIDs, the root classes of the
  targets with counts ("0" counts DID values of 0).

Encodings are those of `property-system.md` §4. Field order inside a struct carries no
meaning; element order inside an array is the stored order and is kept. Every boolean of the
family that is stored is stored `true`, unless a row says otherwise; absent means `false`.

<a id="s3-2"></a>
### 3.2 DID targets (Confirmed, all DID values of the records of §2.1)

DID fields are declared with game-object type code 0x45 (`property-system.md` §9), except
these, which hold DIDs of other archives:

| Field | Declared type code | Targets |
|---|---|---|
| `PrivateEncounterTemplate_IntroVO` | 0x12 | sound-info records, prefix 0x2A, in the sound archive (942 values: 908 non-zero, 34 are 0) |
| `DistantBattleUnit_CosmeticEntity`, `PhysObj` | 0x39 | entity descriptors, prefix 0x47 (376 and 100 values) |
| `Appearance_AprFile` | 0x4E | prefix 0x20 (84 values) |
| `ScriptTable_DBScriptTable` | 0x2A | script tables, prefix 0x07 (7 values) |

Every non-zero 0x45 value in every field of this spec (38,545 values with type 0x45 in all)
names an existing 0x70 record; none dangles.

<a id="s3-3"></a>
### 3.3 Enum sources (Confirmed)

Enum and bitfield fields name their mapper (or DID map) in the property definition
(`property-system.md` §7). Labels in the last column are EMAPPER labels, read at run time.

| Field(s) | Source DID | Names | Label | Keys used |
|---|---|---|---|---|
| `…_ContentLayer`, `Entity_ContentLayer` | DID map 0x28000012 | 1,661 entries | `ContentLayer` | §4.4 |
| `Skirmish_GroupSize` and every other group-size field | 0x230002DA | 7 (0, 1, 2, 3, 6, 12, 24) | `SkirmishGroupSize` | §1 |
| `Skirmish_DifficultyTier` and every other tier field | 0x230002DC | 11 (0–10) | `SkirmishDifficultyTier` | 1–10 |
| `WorldJoin_EncounterType` | 0x23000309 | 4: 0 none, 3 skirmish, 4 classic instance, 6 team | — | 3, 4, 6 |
| `WorldJoin_EncounterCategory`, `…_Capped_…`, `…_Seasonal_…` | 0x23000350 | 37 (0–39) | `WJEncounterCategory` | 36 keys |
| `WorldJoinControl_Instance_Group_Label` | 0x230003D1 | 9 (0–8) | `WJInstanceGroup` | 1–8 |
| `PrivateEncounterTemplate_EncounterType` | 0x2300042E | 2: 0 none, 1 mission | — | 1 |
| `PrivateEncounterTemplate_LockSize`, `PrivateEncounter_Control_LockSize` | 0x23000141 | 6: 0 none, 1–5 lock kinds (§5.1) | — | 1, 3, 4, 5 (templates); 1–5 (control) |
| `PrivateEncounter_Control_LockResetDays` (bitfield 18) | 0x230003AE | 8: 0 none, 1 Sunday … 7 Saturday | `DayOfWeek` | §5.1 |
| `PrivateEncounterTemplate_PaperItemAwardType` | 0x23000340 | 37 (0–41) | `PEAwardType` | 33 keys (§4.9) |
| `PrivateEncounterTemplate_TeamNumber` | 0x23000378 | 3 (0–2) | `PETeamNumber` | 1, 2 |
| `PrivateEncounterTemplate_SkyLocalNameOverride` | 0x230000E9 | 647 | `SkyLocalName` | §4.8 |
| `PrivateEncounterTemplate_DisallowedMapNoteType` (bitfield 14) | 0x230000D7 | 47 | `MapNoteType` | one bit (§4.8) |
| `PrivateEncounterTemplate_RequiredRaceEntry` | 0x2300000B | 123 | `SpeciesType` | 4 keys |
| `SkirmishControl_SkirmishStat`, `Skirmish_*_Stat`, `SkirmishControl_BlockedStat` | 0x230002F4 | 25 (0–24) | `SkirmishStat` | §6.4 |
| `Mod_Op` (in `Skirmish_Stat_Op`) | 0x23000046 | 12 | `PropertyOp` | 2 divide, 7 add (§15.1 B4) |
| `SkirmishControl_Skirmish_TraitNature` | 0x23000057 | 34 | `TraitNature` | 8–11 |
| `BigBattleControl_Medal` | 0x230003BB | 6: 0 none, 1 … 5 (lowest to highest medal) | `QuestMedal` | 1–5 |
| `BigBattleControl_BigBattleWaveLocation`, `DistantBattle_AllowedFlank` | 0x230003AC | 8: 0 none, 1 left flank, 2 centre, 3 right flank, 4–6 prototype positions, 7 full | `DistantBattleLocation` | 1, 2, 3, 7 |
| `BigBattleControl_Quest_Color` | 0x230003D2 | 12 (0–11) | — | 1–11 (`quests.md` §4.2) |
| `DistantBattleUnit_Formation_Width` | 0x230003B6 | 8 (0–7) | — | 1–7 (§8.5) |
| `DistantBattleUnit_LaneType` | 0x230003CC | 3: 0 none, 1 siege, 2 spawn | `DistantBattleLaneType` | 1; default 2 |
| `DistantBattleUnit_CaptureQualityOverride` | 0x230003F4 | 6: 0 low, 1 medium, 2 high, 3 very high, 4 count, 0xFFFFFFFF automatic | — | 2, 3; default 0xFFFFFFFF |
| `BattleStat_Scope` | 0x23000391 | 4: 0 none, 1 team, 2 global, 3 player | `BattleStatScopeType` | 3 |
| `BattleStat_UpdatePolicy` | 0x23000392 | 5: 0 none, 1 set, 2 add, 3 minimum, 4 maximum | `BattleStatUpdatePolicyType` | 1, 2, 4 |
| `…_Medal_Alert`, `PrivateEncounter_EntryFilter_Alert` | 0x23000033 | 65 | `Alert` | |
| `…_RejectErrorType`, `…_MaxRank_Error`, `Required_BillingTokens_Error` | 0x230000BF | 1,673 | `ErrorType` | |

<a id="s3-4"></a>
### 3.4 Identifying a record's kind (Confirmed)

Each kind has a characteristic top-level property that occurs in the property sets of that
kind and of no other root class (checked over all 0x79 and 0x78 sets):

| Characteristic property | Root class (records) |
|---|---|
| `PrivateEncounterTemplate_ContentLayer` (and `…_Name`) | 1368 (959), 2651 (552) |
| `Skirmish_Template_TierMin_ConfigerationArray` | 2651 (552) — tells 2651 from 1368 |
| `SkirmishMatchParams_Level_Min` | 2727 (183) |
| `Skirmish_SkillTrainer_AllowedTraits` | 2669 (3) |
| `PrivateEncounter_Control_LockArray` | 1366 (1) |
| `PrivateEncounter_EntryFilter_RejectErrorType` | 2573 (2), 2576 (2); 2573 alone also holds `…_Alert` |
| `SkirmishControl_SkirmishTraitGates` | 2515 (1) |
| `WorldJoinControl_InstanceGroups` | 2606 (1) |
| `WorldJoinTicket_MaxEncounters_RipCord` | 3894 (2) |
| `BigBattleControl_Medal_Info_Hash` | 3787 (1) |
| `DistantBattle_Wave_List` | 3808 (14) |
| `DistantBattleUnit_Activator_Respawn_Time` | 3725 (96) |
| `DistantBattleUnit_FormationControl_Width_Lookup` | 3745 (1) |
| `SessionPlay_AllowedSlotCount_List` | 1925 (1) |
| `FellowshipMakerControl_ValidEncounterSizeArray` | 3276 (1) |
| `PhasingBox_UpdateOnlyOnCreation` on a record (not on placed content) | 3369 (1) |
| `BattleStat_Name` | 3590 (10), 3622 (3) |

Classes with an empty property set (3399, 3423, 3576, the dialogs) are identified by root
class or WEENIECONTENT label only.

---

<a id="s4"></a>
## 4. Private-encounter templates (classes 1368 and 2651)

Class 2651's ancestry starts 2651 > 1368 > 486 (`wlib.md`); its records hold the 1368 fields
plus the skirmish and world-join fields of §4.10.

<a id="s4-1"></a>
### 4.1 Template kinds (Confirmed, Derived)

| Kind | How to recognise it | Records |
|---|---|---|
| Ordinary instance | class 1368 | 959 (3 of them also carry skirmish fields and `WorldJoin_EncounterType` 6, "team") |
| Mission | class 2651 with `PrivateEncounterTemplate_EncounterType` = 1 | 384; all have `MaxPlayers` 2, a one-entry tier-minimum list, no world-join fields |
| World-join encounter | class 2651 with `WorldJoin_EncounterCategory` | 166; all have a ten-entry tier-minimum list, `LengthDefault`, `LengthRipcord`, `PaperItemAwardArray` and both permission blobs; `WorldJoin_EncounterType` 3 (22), 4 (143), absent (1); 14 of them name a distant-battle script (big battles) |
| Other skirmish template | class 2651 with neither | 2 (three-entry tier-minimum lists); one carries `WorldJoin_EncounterType` 4 without a category (so §4.10 counts 144 records with key 4: 143 world-join encounters and this one), the other has no encounter type |

<a id="s4-2"></a>
### 4.2 Identity, capacity and timers

| Path | Type | Present (1368 / 2651) | Observed | Meaning |
|---|---|---|---|---|
| `PrivateEncounterTemplate_Name` | 13 | all / all | | Display name |
| `PrivateEncounterTemplate_Description` | 13 | all / all | | Display description |
| `PrivateEncounterTemplate_MaxPlayers` | 15 | 920 / 551 | 1368: 1 (389), 2 (98), 3 (101), 4, 6 (250), 12, 18, 20, 24, 25 (48), 36, 50, 100, 200, 250; 2651: 1, 2 (392), 3, 6, 12, 24 | Player capacity of one instance |
| `PrivateEncounterTemplate_PublicInstance` | 21 | 72 / 0 | | Instance shared by all who enter (Reported, LC) |
| `PrivateEncounterTemplate_PublicInstance_MaxPlayerWorldProp` | 10 | 73 / 0 | PIDs of 73 world properties (`World_PublicInstanceMaxPlayer_…`, `World_PopTrackMaxPlayer_…`) | World property that caps a public instance's population (§15.2) |
| `PrivateEncounterTemplate_LockSize` | 8 | 35 / 8 | 1 (2), 3 (10), 4 (7), 5 (24) | Lock kind (§5.1) |
| `PrivateEncounterTemplate_TimeDormantLimit` | 9 | 908 / 550 | 0.01–3,600 / 10–3,600 (120 in 381, 900 in 139 of 2651) | Timer, presumably seconds (§15.2 OPEN) |
| `PrivateEncounterTemplate_TimeGraceLimit` | 9 | 876 / 549 | 0.01–600 / 10–600 | Timer, presumably seconds; not always ≤ the dormant limit (199 of 1,422 exceed it) |
| `PrivateEncounterTemplate_LinkdeadTimeoutOverride` | 9 | 879 / 0 | 300 (873), 10 (6) | Replaces `PrivateEncounter_DefaultLinkdeadTimeout` (§5.1) |
| `PrivateEncounterTemplate_FailureTimeoutOverride` | 9 | 238 / 0 | 1–300 (5 in 170) | |
| `PrivateEncounterTemplate_LengthDefault` | 15 | 0 / 166 | −1 (21), 5–180 | Nominal length; −1 = none (OPEN) |
| `PrivateEncounterTemplate_LengthRipcord` | 20 | 0 / 166 | ripcord 2801 (129), 0 (37) | Ripcord of `map-data.md` §11 |
| `PrivateEncounterTemplate_CanSelfRevive` | 21 | 60 / 0 | | |
| `PrivateEncounterTemplate_MountedCombatDisallowed` / `…_MountedCombatAllowed` | 21 | 2 / 0, 0 / 2 | | |
| `PrivateEncounterTemplate_IsCinematicEncounter`, `…_DynamicAreaInstance` | 21 | 1 / 0 each | | |
| `PrivateEncounterTemplate_WhiteList` | 20 | 17 / 0 | class 3049 (17) | |
| `Induction_ActionDID` | 20 | 48 / 0 | class 1195 | An induction (`combat-data.md`); its use is OPEN (§15.2) |
| `PrivateEncounter_ShowQuestGuide_Override` | 21 | 12 / 0 | | |

<a id="s4-3"></a>
### 4.3 Places and landblocks

| Path | Type | Present (1368 / 2651) | Observed |
|---|---|---|---|
| `PrivateEncounterTemplate_StartingPosition` | 1 | 958 / 552 | Place key (`map-data.md` §10: all resolve) |
| `PrivateEncounterTemplate_SuccessTeleportLocation` | 1 | 850 / 398 | Place key |
| `PrivateEncounterTemplate_SuccessTeleportLocationArray[]` / `…_SuccessTeleportLocationStruct` | 12 / 11 | 3 / 0 (4 entries each) | `…_RequiredRaceEntry` (enum, 4 keys), `…_SuccessTeleportLocation` (place key), `…_SuccessTeleportSceneDID` (scene 1468; 4 entries in 1 record) |
| `PrivateEncounterTemplate_InstanceMilestone` | 1 | 181 / 549 | Place key |
| `PrivateEncounterTemplate_ExternalInstanceMilestone` | 1 | 50 / 0 | Place key |
| `PrivateEncounterTemplate_DefaultExitLocation` | 1 | 15 / 0 | Place key |
| `PrivateEncounterTemplate_ExitLocationIfEnteredThroughDoor` | 1 | 2 / 0 | Place key |
| `PrivateEncounterTemplate_DynamicTelepadInfo` | 11 | 1 / 43 | `…_DynamicTelepad_Array` (1–6 place keys; 108 in all) and `…_DynamicTelepad_ZoneEvent` (world event, class 1887) |
| `PrivateEncounterTemplate_ContainedLandBlockArray[]` / `…_ContainedLandBlock` | 12 / 6 | 935 / 552 records; 1–46 / 1–40 entries; 2,730 / 1,898 entries | Landblock list (encoding below) |
| `PrivateEncounterTemplate_ShouldLoadAllLandblocks` | 21 | 7 / 14 | |

**Landblock list encoding (Confirmed, Derived).** Each `…_ContainedLandBlock` is a vector of
three floats holding integers: (region, *X*, *Y*). The landblock is region, ⌊*X* ÷ 8⌋,
⌊*Y* ÷ 8⌋. In 4,410 of the 4,628 values both *X* and *Y* are multiples of 8; in 218 one or
both are not, and the floor rule still applies: the object form of every one of the 1,511
templates stores exactly these landblocks, in stored order (§13.1). Regions observed: 1
(1,288 values), 2 (1,669), 3 (446), 4 (478), 5 (645), 6 (4), 14 (80), 15 (18). 25 templates
list a landblock twice. 24 templates (all class 1368) have no list.

The place-key strings name positions; their resolution is `map-data.md` §3.4. The object
form stores every place-key field already resolved to a position (§13.1).

<a id="s4-4"></a>
### 4.4 Content layer

`PrivateEncounterTemplate_ContentLayer` (enum, present in every template) is a key of the DID
map 0x28000012 (label `ContentLayer`, 1,661 entries; every template's key is present). Keys
are mostly unique: 950 distinct among the 959 class-1368 templates and 544 among the 552
class-2651 templates.

Static content of landblocks carries `Entity_ContentLayers` (array of keys of the same map).
Over all 49,636 landblock-info records, 15,554 landblocks hold layered content with 1,486
distinct keys. For a template with a landblock list, its key occurs in the layered content
of at least one listed landblock in 1,475 of 1,487 templates; 5 use a key that occurs only in
other landblocks and 7 a key that occurs nowhere. Of the 24 templates without a list, 22 use
a key that occurs somewhere.

Model (Derived from the correlation above; not BIN): an instance's active content layer is
its template's key, and the content-layer test of `entity-descriptor.md` §4.5.1 is applied
with that layer, so the instance shows the static entities whose layer list names it (or
`All`, or that have no list). The client learns the active layer from the server.

> **OPEN:** Whether the client also keeps layer 1 (`InstanceZero`) active inside an
> instance, and how the 7 templates whose key occurs in no landblock content are drawn.

> **OPEN:** Each DID-map entry's value is a 0x78 property set (0x7800001C for 1,626 of the
> 1,660 non-sentinel keys, 12 other sets for the rest). What those sets contribute to an
> instance (properties applied to the layer?) is not established.

<a id="s4-5"></a>
### 4.5 Instance properties

`PrivateEncounterTemplate_InstanceProperties` (struct; 777 / 552 records) holds property
values that apply to the instance as a whole. Fields observed:

| Field | Type | Present (1368 / 2651) | Observed |
|---|---|---|---|
| `PrivateEncounter_LoadPermittedLandBlocksOnly` | 21 | 762 / 552 | true |
| `ze_ai_runhome_train` | 21 | 20 / 153 | true |
| `ze_antiexploit_effect` | 15 | 20 / 151 | 1 |
| `ze_skirmish_player_groupsizechoice` | 15 | 0 / 94 | 2, 3, 6, 12, 24 |
| `ze_skirmish_group_size` | 15 | 0 / 2 | 6, 12 |
| `EnginePerformance_DialBackExpensiveFeatures`, `Volume_IsNonMigrateableBlock` | 21 | 0 / 14 each | true (the 14 big-battle templates) |
| `Combat_UsePvPCombatMechanics` | 21 | 2 / 0 | true |
| `Skirmish_ActivateSkirmishTraits` | 21 | 1 / 0 | true |
| `ze_solo_epic` | 15 | 1 / 0 | 1 |

The struct is open (any property may appear). Model (by its name; not confirmed): a server copies its fields onto the instance (§15.2).

<a id="s4-6"></a>
### 4.6 Quests

| Path | Type | Present (1368 / 2651) | Observed |
|---|---|---|---|
| `PrivateEncounterTemplate_InstanceQuest` | 20 | 802 / 410 | quest 1398 |
| `PrivateEncounterTemplate_TracksInstanceQuestCompletion` | 21 | 286 / 1 | 8 of the 287 have no instance quest |
| `PrivateEncounterTemplate_QuestToBestow_Array[]` | 12 / 20 | 41 / 161 records; 1–3 / 1–8 entries (69 / 546) | quest 1398 |
| `PrivateEncounterTemplate_RandomBestowalQuests_Array[]` / `…_RandomQuest_Struct` | 12 / 11 | 4 / 15 records, one struct each | `…_NumQuestsToBestow` (1 or 2), `…_QuestToBestow_Array` (2–9 quests; 20 / 53 in all) |
| `PrivateEncounterTemplate_QuestTrackerFilter_Array[]` | 12 / 20 | 4 / 0 | quest 1398 |
| `PrivateEncounterTemplate_AssociatedQuest_List[]` | 12 / 20 | 0 / 2 (one empty) | quest 1398 |
| `PrivateEncounterTemplate_EpicQuestInfo` | 11 | 0 / 5 | `…_EpicQuest` (1398), `…_EpicQuestObjective` (1–5) |
| `PrivateEncounterTemplate_FeaturedInstance_Cap_Quest`, `…_NonCap_Quest` | 20 | 0 / 46 each | quest 1398 |
| `PrivateEncounterTemplate_EventControlDID` | 20 | 753 / 552 | world-event list 2161 (`quests.md` §14) |
| `PrivateEncounterTemplate_MemberCountZoneEvent` | 20 | 24 / 0 | world event 1887 |
| `PrivateEncounterTemplate_PropertyResponseMapDID` | 20 | 55 / 551 | property-response map 2192 (`misc-records.md` §13.5) |
| `QuestDispenser_ActiveQuests` | 12 | 0 / 1 (empty) | |
| `Quest_IsDelvingEligible` | 21 | 0 / 383 | true 200, **false 183** |

<a id="s4-7"></a>
### 4.7 Permission blobs

`DefaultPermissionBlobStruct` (68 / 169 records) and
`PrivateEncounterTemplate_CreationPermissionBlobStruct` (53 / 166) have the layout of the
permission struct of `quests.md` §8.1: `Usage_QuestRequirements` (one entry),
`Usage_QuestRequirements_OrList` (three entries in all 166 class-2651 uses),
`Usage_WorldEvent_AllConditionList` (one `WorldEvent_Condition`), `Usage_RequiredClassList`
(21 records of 1368), `Usage_SessionPlayerCanUse` (43 / 0 and 39 / 0 records, true),
`Usage_MonsterPlayerCanUse` (0 / 3). Requirements are evaluated as in `quests.md` §8.2; a
requirement entry with quest DID 0, operator 0 and status 0 (147 of the 166 single
requirements and 378 of the 498 or-list entries of class 2651) looks like an unused slot.

> **OPEN:** Whether an all-zero requirement entry counts as satisfied (so that an or-list of
> three all-zero entries passes) or is skipped; `quests.md` §8.2 does not cover quest DID 0. The two blobs are equal in 1,467 of 1,511 templates.

Model (by the labels; not confirmed): the creation blob gates who may **create** an instance;
the default blob gates who may **enter** one (§15.2 OPEN).

<a id="s4-8"></a>
### 4.8 Scenes, sound, sky and map

| Path | Type | Present (1368 / 2651) | Observed |
|---|---|---|---|
| `PrivateEncounterTemplate_TeleportationSceneID` | 20 | 568 / 552 | scene 1468 (564 / 254), 0 (4 / 298) |
| `PrivateEncounterTemplate_SuccessTeleportSceneDID` | 20 | 268 / 150 | scene 1468 |
| `PrivateEncounterTemplate_IntroVO` | 20 | 765 / 177 | sound-info records, prefix 0x2A (765 / 143), 0 (0 / 34); 942 values, 908 of them non-zero references (`sound.md`) |
| `PrivateEncounterTemplate_SkyLocalNameOverride` | 8 | 337 / 550 | 121 / 82 distinct keys; key 0 in 282 class-2651 records |
| `PrivateEncounterTemplate_DisallowedMapNoteType` | 14 | 84 / 142 | one bit (value 0x1000000) in 83 / 142; 0 in 1 |
| `PrivateEncounterTemplate_ShouldIgnoreWEBroadcasts` | 21 | 13 / 0 | |

<a id="s4-9"></a>
### 4.9 Loot and awards

| Path | Type | Present (1368 / 2651) | Observed |
|---|---|---|---|
| `PrivateEncounterTemplate_SpeciesLevelLootTable` | 20 | 21 / 12 | species table 2001 (`economy.md` §4.7) |
| `PrivateEncounterTemplate_TreasureLevel` | 15 | 0 / 11 | 82–130 |
| `PrivateEncounterTemplate_DefeatInfoOverride` | 20 | 0 / 26 | class 1035 |
| `PrivateEncounterTemplate_RandomEffectsArray[]` | 12 / 20 | 0 / 1 (3 entries) | effects 734 |
| `PrivateEncounterTemplate_PaperItemAwardArray[]` / `…_PaperItemAwardEntry` | 12 / 11 | 0 / 166 records (0–24 entries, 1,015 in all; 21 records empty) | below |
| `PrivateEncounterTemplate_PaperItemAwardEntry` (single, not in an array) | 11 | 1 / 0 | same fields |
| `…Entry/PrivateEncounterTemplate_PaperItemAwardType` | 8 | every entry | 33 distinct keys 2–41 (mapper `PEAwardType`: boss kills by tier, lieutenants, defenders by percentage, quest, encounter) |
| `…Entry/PrivateEncounterTemplate_RatioArray[]` / `…_RatioEntry` | 12 / 11 | every entry; 1–3 elements (1,823 in all) | |
| `…RatioEntry/PrivateEncounterTemplate_AwardCurrency` | 20 | every element | items 796 (currency items) |
| `…RatioEntry/PrivateEncounterTemplate_AwardRatio` | 9 | every element | 0.01–2.2 |

Model (by the labels; not confirmed): when the instance event named by the award type happens,
each participant receives `AwardRatio` × (a base amount) of each listed currency item (§15.2
OPEN for the base amount and rounding).

<a id="s4-10"></a>
### 4.10 Skirmish and world-join fields

Present in class 2651; the "1368" column counts the 3 team templates of §4.1.

| Path | Type | Present (1368 / 2651) | Observed |
|---|---|---|---|
| `Skirmish_Template_DifficultyTierArray[]` / `Skirmish_DifficultyTier` | 12 / 8 | 3 / 552 | always 1 … *n* in order; over the 552 class-2651 records *n* = 1 (433), 2 (45), 3 (43), 5 (31); *n* = 1 in the 3 team templates |
| `Skirmish_Template_GroupSizeArray[]` / `Skirmish_GroupSize` | 12 / 8 | 3 / 552 | 1–5 sizes; ascending in 550 of 552 |
| `Skirmish_Template_GroupSize_Override` | 8 | 0 / 392 | 2 |
| `Skirmish_Template_TierMin_ConfigerationArray[]` / `Skirmish_Template_Configuration` | 12 / 11 | 0 / 552; 1 (384), 3 (2) or 10 (166) entries | entry *k* has `Skirmish_DifficultyTier` = *k* (1 … *n*) and `Skirmish_GroupSize` 0 (1,744), 3 (153) or 6 (153) |
| `Skirmish_Template_Valid_Configurations[]` / `Skirmish_Template_Configuration` | 12 / 11 | 0 / 1 (2 entries) | (tier 1, size 12), (tier 2, size 24) |
| `Skirmish_Template_MinLevelScale` / `…_MaxLevelScale` | 15 | 1 / 552 each | min 10–160, max 32–160; min ≤ max in all |
| `Skirmish_Template_LevelScalingLevel` | 15 | 0 / 14 | 100 (§15.1 B2) |
| `Skirmish_Template_LevelScale_Override_PropertyName` | 10 | 0 / 12 | one world property |
| `Skirmish_Template_MaxLevelScale_Override_PropertyName`, `…_MinLevelScale_Override_PropertyName` | 10 | 0 / 12, 0 / 4 | world properties |
| `Skirmish_Template_DifficultyTierGate_PropertyName` | 10 | 0 / 552 | 0 in 506; else a world property per template |
| `Skirmish_Template_DifficultyRestrictionLevel` / `…_DifficultyRestrictionTier` | 15 / 8 | 0 / 166 each | (0, 0) in 120; levels 120–160 with tier 2 (44) or 3 (2) |
| `Skirmish_RestrictionDescription` | 13 | 0 / 95 | |
| `Skirmish_Template_IsTutorial`, `…_Highest_Time_Is_Best_Time` | 21 | 0 / 1 each | |
| `Skirmish_Template_Block_Stat_Tracking` | 21 | 0 / 2 | |
| `WorldJoin_EncounterType` | 8 | 3 (key 6) / 166 (3: 22, 4: 144) | key 4: 143 world-join encounters and one other template (§4.1) |
| `WorldJoin_EncounterCategory` | 8 | 0 / 166 | 36 distinct keys 0–39 |
| `WorldJoin_EncounterTicket` | 20 | 0 / 14 | ticket 3894 |
| `PrivateEncounterTemplate_EncounterType` | 8 | 0 / 384 | 1 (mission) |
| `PrivateEncounterTemplate_DistantBattleScript` | 20 | 0 / 14 | distant-battle script 3808, one template per script |
| `WebStoreAccountItem_DataID` | 20 | 0 / 58 | store items 2532 (56), 2784 (2) |
| `Required_BillingTokens[]` / `Required_Token_PropertyName`, `Required_BillingTokens_Error` | 12 / 10, 8 | 0 / 5 | one account property; one error key |

For every class-2651 template with `MaxPlayers`, `MaxPlayers` equals the largest entry of
`Skirmish_Template_GroupSizeArray` (551 of 551).

<a id="s4-11"></a>
### 4.11 Teams

Three class-1368 templates (the team templates) carry `PrivateEncounterTemplate_Number_Of_Permitted_Teams`
(2) and `PrivateEncounterTemplate_TeamConfig_Array` (two `…_TeamConfig_Struct` entries:
`…_TeamNumber` 1 and 2, `…_TeamName` (StringInfo), `…_StartingPosition` and
`…_InstanceMilestone` (place keys)).

<a id="s4-12"></a>
### 4.12 Labelled templates

`NewbieCinematicEncounter` names a class-1368 template (the only one with
`…_IsCinematicEncounter`). `PublicAreaDefaultEncounter` names a class-1368 template with
content layer 1 (`InstanceZero`, the shared world), `…_DynamicAreaInstance`, `MaxPlayers` 250,
dormant and grace limits of 300 and a `…_PublicInstance_MaxPlayerWorldProp`, but without
`…_PublicInstance` (the 73rd holder of that PID field of §4.2).

> **OPEN:** How a server uses the public-area default (presumably for dynamically created
> copies of shared areas, given its layer and flag).

---

<a id="s5"></a>
## 5. Private-encounter control, entry filters and managers

<a id="s5-1"></a>
### 5.1 Control record (class 1366, label `PrivateEncounterControl`)

| Path | Type | Observed |
|---|---|---|
| `PrivateEncounter_DefaultEncounterTimeout` | 9 | 900 |
| `PrivateEncounter_DefaultLinkdeadTimeout` | 9 | 300 |
| `PrivateEncounter_LeaveInstanceInduction` | 20 | induction 1195 |
| `PrivateEncounter_Control_LockArray[]` / `…_LockStruct` | 12 / 11 | 5 entries, one per lock kind 1–5 |
| `…LockStruct/PrivateEncounter_Control_LockSize` | 8 | 1 … 5, each once |
| `…LockStruct/PrivateEncounter_Control_LockResetDays` | 18 | day-of-week bitfield (below) |
| `PrivateEncounter_Control_LockableEncounterArray[]` | 12 / 20 | 43 templates (1368: 35, 2651: 8) |
| `PrivateEncounter_Control_RestrictMaxPlayersExclusionArray[]` | 12 / 20 | 4 templates (all public, `MaxPlayers` 1) |
| `PrivateEncounter_Control_PvMP_Challenge_Encounter_Array[]` | 12 / 20 | 1 template |
| `PrivateEncounter_EntryFilters[]` / `PrivateEncounter_EntryFilter` | 12 / 20 | 4 filters (2573: 2, 2576: 2) |
| `PrivateEncounter_EntryFilters_IgnoredEncounters[]` | 12 / 20 | 12 templates (all 1368, all `MaxPlayers` 1) |

**Lockable list (Confirmed).** The set of `…_LockableEncounterArray` equals the set of
templates that carry `PrivateEncounterTemplate_LockSize` (43 = 43).

**Reset days (Confirmed layout).** `LockResetDays` is a 32-bit bitfield labelled by the
`DayOfWeek` mapper: key *k* (1 Sunday … 7 Saturday) is bit *k* − 1. Observed: lock kind 5 →
all seven days; kinds 2 and 4 → three days; kind 3 → two days; kind 1 → one day. A lock
resets on each listed day (§15.2 OPEN for the time of day and time zone).

<a id="s5-2"></a>
### 5.2 Entry filters (classes 2573 and 2576)

Both descend from class 2571. Property form:

| Path | Type | 2573 | 2576 |
|---|---|---|---|
| `PrivateEncounter_EntryFilter_RejectMessage` | 13 | both | both |
| `PrivateEncounter_EntryFilter_RejectErrorType` | 8 | both (two keys) | both |
| `PrivateEncounter_EntryFilter_Alert` | 8 | both (keys 34, 35) | — |

The numbers that make a filter work live only in the **object form** (Confirmed): the root
object of a 2573 record has four integer attributes, a 2576 record three:

| Record | Attribute values in stored order |
|---|---|
| 2573, first | 3,600; an enum key; an enum key; 15 |
| 2573, second | 86,400; an enum key; an enum key; 100 |
| 2576, first | 3,600; an enum key; 0 |
| 2576, second | 86,400; an enum key; 0 |

3,600 and 86,400 are one hour and one day in seconds; 15 and 100 are counts.

> **OPEN:** The model "at most *N* instance entries per window" (15 per hour, 100 per day
> for 2573) fits the numbers but is not confirmed; the role of the two enum keys (they are
> not the property form's error types) and of class 2576 (no count) is unknown. Attribute
> names are hashes only (`wstate.md` §4.2).

<a id="s5-3"></a>
### 5.3 Manager entities (classes 1367, 2619, 3136)

`PrivateEncounterServer` (1367), `WorldJoinManager` (2619) and `FellowshipMakerManager`
(3136) are entity templates (ancestry 1367 > 1733 > 787 > 1726 > …; 2619 and 3136 have class 788
between themselves and 1733), each with a generic entity property set (36 top-level
properties: physics, usage action, selection, creation type 8) and an object form whose root
holds three null references. They carry no rules of this family.

> **OPEN:** What the three manager entities are for (by their labels, the server-side objects
> that host the private-encounter, world-join and fellowship-maker managers).

---

<a id="s6"></a>
## 6. Skirmishes

<a id="s6-1"></a>
### 6.1 Match parameters (class 2727)

| Path | Type | Present | Observed |
|---|---|---|---|
| `SkirmishMatchParams_Level_Min` / `…_Level_Max` | 15 | all 183 | 17 level bands: 1–29, 30–49, 50–59, 60–64, 60–65, 65, 66–74, 75, 76–84, 85, 86–94, 95–99, 100, 101–104, 105–114, 115–159, 160 |
| `SkirmishMatchParams_GroupSize_Min` / `…_Max` | 8 | all | (1, 3), (6, 6), (12, 12): 61 each |
| `SkirmishMatchParams_DifficultyTier_Min` / `…_Max` | 8 | all | (1, 1), (2, 2), (3, 3): 45 each; (1, 3): 48 |

All 183 records are distinct. Each level band appears with every group band and every tier
band (12 records), except the bands 60–64 and 65 (3 records each, tier band (1, 3) only) and
60–65 (9 records, the single-tier bands). The object form is empty (class 2727 has no
attributes). The records are referenced only by the skirmish treasure look-ups
(`economy.md` §4.10): 8,326 look-ups use all 183 records with 63 encounters (3,272 distinct
encounter–parameter pairs).

**Band overlaps (Confirmed, all 16,653 record pairs).** The level bands are disjoint except
60–64, 60–65 and 65, which overlap. They never meet in the same records: 60–64 and 65 occur
only with tier band (1, 3), and 60–65 only with the single-tier bands (1, 1), (2, 2) and
(3, 3). Taken together with the tier band, therefore, no two records share a group band and a
tier band and have overlapping level bands (0 pairs). The tier bands themselves overlap,
because (1, 3) contains each single tier: 144 record pairs overlap in level, group size and
tier at once.

Meaning (Reported, `economy.md` §4.10): a look-up applies to a skirmish run whose level, group
size and tier fall inside the bands. Inclusive bounds are Derived from the band edges: the
disjoint level bands abut without a gap (29 / 30, 49 / 50, 59 / 60, 65 / 66, …).

> **OPEN:** Which record applies when a run of tier 1, 2 or 3 falls inside both a single-tier
> record and a (1, 3) record of the same level and group band (both, or one by a rule the data
> does not show). This is server behaviour (§15.2).

<a id="s6-2"></a>
### 6.2 Skill-trainer trait lists (class 2669)

`Skirmish_SkillTrainer_AllowedTraits[]` / `Skirmish_SkillTrainer_TraitDID`: skirmish traits
(class 2525), 1, 47 and 89 entries (137 in all). NPCs name a list with
`Skirmish_SkillTrainer_Skirmish` (25 NPCs). The object form holds the same DIDs as one set.

<a id="s6-3"></a>
### 6.3 Skirmish control (class 2515, label `SkirmishControl`)

| Path | Type | Observed | Meaning |
|---|---|---|---|
| `SkirmishControl_DifficultyMultiplier_Array[]` / `…_DifficultyToMultiplier` {`…_DifficultyTier`, `…_SkirmishPoint_Multiplier`} | 12 / 11 | 3 entries, tiers 1–3, multipliers 1–1.25 | Skirmish-point multiplier per tier |
| `SkirmishControl_GroupSizeMultiplier_Array[]` / `…_GroupSizeToMultiplier` {`…_GroupSize`, `…_SkirmishPoint_Multiplier`} | 12 / 11 | 6 entries, one per group size, 1–2.5 | Skirmish-point multiplier per size |
| `SkirmishControl_GroupTier_Array[]` / `…_GroupSize_To_GroupTier` {`…_GroupSize_StructVal`, `…_GroupTier_StructVal`} | 12 / 11 | 6 entries: 1→1, 2→3, 3→3, 6→6, 12→12, 24→12 | Group size → size band used for scaling |
| `SkirmishControl_ZoneEvents` {`…_ZoneEvent_Difficulty`, `…_GroupSize`, `…_GroupTier`, `…_Level`} | 11 | world events 1887 | World events that carry the run's tier, size, size band and level |
| `SkirmishControl_SkirmishPointToken` | 20 | item 796 | Skirmish-point currency item |
| `SkirmishControl_SkirmishPanel_RequiredTrait`, `…_SkirmishTrainer_RequiredTrait`, `…_SkirmishBard_RequiredTrait`, `…_WorldJoin_RequiredTrait`, `…_WorldJoin_TutorialCompletedTrait` | 20 | traits 1477 | Traits that unlock the panel, trainer, bard, world join and mark the tutorial done |
| `SkirmishControl_SkirmishTraitGates[]` / `…_SkirmishTraitGate` {`…_MaxRank`, `…_MaxRank_WebstoreItem`} | 12 / 11 | 6 gates, ranks 15–50, store items 2532 | Trait-rank caps unlocked by store items |
| `SkirmishControl_SkirmishTraitGate_DEFAULT_MaxRank`, `…_Error`, `…_WebstoreItem` | 15, 8, 20 | 7, an error key, 0 | Cap without a gate item |
| `SkirmishControl_SkirmishTraitPools[]`, `…_LandscapeSkirmishTraitPools[]` / `…_Skirmish_TraitNature` | 12 / 8 | keys 8–11 / 8, 9, 11 | Trait natures available in skirmishes / on the landscape |
| `SkirmishControl_SkirmishGating_DEFAULT_MaxSkirmishesPerWeek` | 15 | −1 | No weekly limit |
| `SkirmishControl_BlockedStats_List[]` / `…_BlockedStat` | 12 / 8 | 6 statistic keys (the five point sources and points from boss kills) | Loaded by the statistics panel; use OPEN (§15.1 B4) |
| `SkirmishControl_SkirmishGRSEnum_PropertyList[]` / `…_GRSEnum_PropertyName` | 12 / 10 | 19 PIDs (`GRS_…`) | One property per statistic |
| `SkirmishControl_SkirmishStat_To_GRSEnum_PropertyName_Hash[]` / `…_Struct` {`…_SkirmishStat`, `…_GRSEnum_PropertyName`} | 12 / 11 | 19 entries | Statistic key → its `GRS_…` property |
| `SkirmishStats_StatProperty_Conversion_Hash[]` / `…_HashEntry` {`…_FullPropertyName`, `…_SkirmishStat`, `…_SkirmishGroupSize`, `…_OnLevel`} | 12 / 11 | 228 entries = 19 statistics × 6 sizes × 2 | (statistic, size, on-level) ↔ the statistic property of §6.4 |
| `SkirmishStats_CONSTANT_FASTEST_POSSIBLE_SKIRMISH_TIME` | 9 | 0.5 | §15.1 B4 |
| `Skirmish_Stats[]` / `Skirmish_Stat` | 12 / 11 | 5 derived statistics | below and §15.1 B4 |

`Skirmish_Stat` fields: `Skirmish_DerivedStat` (key 17–21), `Skirmish_Primary_Stat` (a
statistic key), `Skirmish_Stat_Operations[]` / `Skirmish_Stat_Op` {`Skirmish_Secondary_Stat`,
`Mod_Op`} (1–6 operations; `Mod_Op` 7 in 10 and 2 in 4), `Skirmish_Stat_Scale` (int, default
1; 3,600 in three statistics). The five derived statistics are points per hour, total points,
completion ratio, optional objectives per hour and lieutenants per hour; the hourly ones
divide by the played time (stored in seconds) and scale by 3,600.

<a id="s6-4"></a>
### 6.4 Statistic keys and properties

Mapper `SkirmishStat` (0x230002F4) has keys 1–16 (stored statistics: points by source 1–5,
kills and deaths 6–8, defenders saved and lost 9–10, control-point flips 11, optional
objectives 12, played time 13, attempts 14, completions 15, best time 16), 17–21 (derived,
§6.3) and 22–24 (boss kills, boss resets, points from boss kills).

Each stored statistic has one dictionary property per group size and level class, named
`SkirmishStats_<statistic>_<size>_<OnLevel|OffLevel>` (sizes `Solo`, `Duo`, `3man`, `6man`,
`12man`, `24man`): 19 × 6 × 2 = 228 properties. Their type is int, except `Best_Time` and
`Skirmish_Played_Time` (float, seconds). The authoritative map from (statistic, size,
on-level) to PID is the conversion hash of §6.3, not the naming pattern. None of the 228 is
stored in the data; they are per-character state.

---

<a id="s7"></a>
## 7. World join (instance finder)

<a id="s7-1"></a>
### 7.1 World-join control (class 2606, label `WorldJoinControl`)

| Path | Type | Observed | Meaning (Reported, LC, for the lists; by label otherwise; BIN where marked) |
|---|---|---|---|
| `WorldJoin_AvailablePETemplates_Array[]` / `WorldJoin_AvailablePETemplate` | 12 / 20 | 150 templates (2651: 149, 1368: 1), no duplicates | Templates offered by the finder |
| `WorldJoinControl_InstanceGroups[]` / `WorldJoinControl_Instance_Grouping` | 12 / 11 | 8 groups | Grouped instances (§15.1 B3) |
| `…Grouping/WorldJoinControl_Instance_Group_Label` | 8 | keys 1–8, each once; the groups are **not** stored in label order (this install: 3, 1, 4, 5, 2, 6, 7, 8) | Group key |
| `…Grouping/WorldJoinControl_Grouped_Instance_List[]` / `…_Grouped_Instance` | 12 / 20 | 1–2 templates (14 in all) | The 14 big-battle templates; none of them is in the available list |
| `WorldJoinControl_FeaturedInstance_Array[]` / `WorldJoin_AvailablePETemplate` | 12 / 20 | 6 templates, all in the available list | Featured rotation (§15.1 B1) |
| `WorldJoinControl_FeaturedInstance_WorldEvent` | 20 | world event 1887 | |
| `WorldJoinControl_DefaultTicket` | 20 | ticket 3894 | Ticket for templates without `WorldJoin_EncounterTicket` |
| `WorldJoinControl_MinLevel` | 15 | 10 | Minimum character level |
| `WorldJoin_Seasonal_EncounterCategory_Array[]` / `…_Entry` | 12 / 8 | 5 category keys | Seasonal categories |
| `WorldJoin_Capped_EncounterCategory` | 8 | 1 category key | The "capped" category (§10.2 bit 0x2) |

The available list and the grouped lists together name 164 templates. The 166 templates that
carry `WorldJoin_EncounterCategory` are these 164 except the class-1368 one, plus 3 that no
list names.

The object form stores the finder order (§13.3), which takes the groups by ascending label,
not in their stored order.

<a id="s7-2"></a>
### 7.2 World-join tickets (class 3894)

Two records, each with `WorldJoinTicket_MaxEncounters_RipCord` and
`WorldJoinTicket_MinTimeBetweenLaunches_RipCord` (ripcords 2801, `map-data.md` §11). One is
the default ticket; 14 templates name the other. The object form is empty.

<a id="s7-3"></a>
### 7.3 Categories and types

`WorldJoin_EncounterType` distinguishes skirmishes (3), classic instances (4) and team
encounters (6). `WorldJoin_EncounterCategory` keys (36 used) group templates for the finder;
their names are display data of mapper 0x23000350 and are read at run time.

<a id="s7-4"></a>
### 7.4 Run-time world-join properties (dictionary only)

The dictionary defines `WorldJoin_Encounter`, `…_DifficultyTier`, `…_GroupSize`, `…_Level`,
`…_IsPublic`, `…_GroupNumber`, `…_PreFellowshipID`, `…_Encounter_PEEffect`,
`…_Encounter_PlayerIID`, `…_Encounter_PlayerToPEEffectHash_Array`, `…_InvitationCreateTime`,
`…_InvitationType`, `…_RequesterIID`, `…_Encounter_SortIndex` and `…_Encounter_SortType`.
None is stored in the data.

> **OPEN:** These look like the fields of a join request, a queue entry and an invitation.
> How they travel between client and server is protocol and outside this spec (decision
> 0004); the queue, request and dequeue dialogs of §12 are the client's side of it.

---

<a id="s8"></a>
## 8. Big battles and distant battles

<a id="s8-1"></a>
### 8.1 Big-battle control (class 3787, label `BigBattlesControl`)

| Path | Type | Observed |
|---|---|---|
| `BigBattleControl_InitialTraitPoints` | 15 | 4 |
| `BigBattleControl_Traits_RefundCostByLevel_Progression` | 20 | progression 1385 |
| `BigBattleControl_Rewards_FirstTimeMedal_Bonus` | 15 | 5 |
| `BigBattleControl_Medal_Info_Hash[]` / `…_Medal_Info_Struct` | 12 / 11 | 5 entries, medals 5, 4, 3, 2, 1 |
| `…/BigBattleControl_Medal` | 8 | medal key |
| `…/BigBattleControl_TraitPointsToGrant` | 15 | 4, 3, 2, 1, 0 for medals 5 … 1 |
| `…/BigBattleControl_CashOut_PointsRequired` | 15 | 20 (medals 2–5), −1 (medal 1) |
| `…/BigBattleControl_CashOut_CurrentPoints` | 10 | a `BigBattle_CashOutBar_…` PID (medals 2–5), 0 |
| `…/BigBattleControl_Medal_BonusProp` | 10 | a `BigBattle_BonusMult_…` PID (medals 2–5) |
| `…/BigBattleControl_FTT_ForCashOut_Loot` | 20 | filtered treasure list 3666 (medals 2–5) |
| `…/BigBattleControl_Medal_Alert` | 8 | alert keys 70–73 (medals 2–5) |
| `…/BigBattleControl_RewardForwarding_GOP`, `…_RewardForwarding_Value` | 10, 15 | medals 2 and 3 only; value 1 |
| `BigBattleControl_Quest_RewardValue_Hash[]` / `…_Struct` {`BigBattleControl_Quest`, `…_QuestPointValue`} | 12 / 11 | 107 quests, points 1–12 |
| `BigBattleControl_Quest_To_Encounter_Hash[]` / `…_Pair` {`BigBattleControl_Quest`, `…_Encounter`} | 12 / 11 | 99 quests → the 14 big-battle templates |
| `BigBattleControl_QuestColorToMedalElement_Array[]` / `…_Struct` {`…_Quest_Color`, `…_Medal_Element`} | 12 / 11 | 11 colours 1–11 → user-interface element keys |
| `BigBattleControl_LocationToZoneEvent_Array[]` / `…_Struct` {`…_BigBattleWaveLocation`, `…_LocationZoneEvent`} | 12 / 11 | flanks 1–3 → world events 1815 |
| `BigBattleControl_Quest_RelatedEntityHighlightScript` | 8 | an FX key |

Medal thresholds of quests are in the quest control record (`quests.md` §9.6).

<a id="s8-2"></a>
### 8.2 The `BigBattles` set (class 3399)

Class 3399's object form holds one attribute, a set of DIDs; its property set is empty. The
record labelled `BigBattles` holds the 14 big-battle templates (the same set as the grouped
lists of §7.1 and the object form of §8.1). The five other 3399 records are WEENIECONTENT sets
of other subsystems: `WebStore` (store items), `MountAppearanceTraits` (767 traits of class
3509), `LoginRewards` (2 records of class 3731), `StorePropertyManipulator` (2 records of class
3924) and `LootGenControl_RewardOnceTGPs` (empty; `economy.md` §14).

<a id="s8-3"></a>
### 8.3 Distant-battle scripts (class 3808)

Each of the 14 scripts is named by exactly one template (`…_DistantBattleScript`).

| Path | Type | Observed |
|---|---|---|
| `DistantBattle_ZoneEvent_Required` | 20 | world event 1815 |
| `DistantBattle_ZoneEvent_Progress` | 20 | world event 1887 |
| `DistantBattle_Wave_List[]` / `DistantBattle_Wave` | 12 / 11 | 2–4 waves (41 in all) |
| `…Wave/DistantBattle_AllowedFlank_List[]` / `DistantBattle_AllowedFlank` | 12 / 8 | 1–3 flank keys (1, 2, 3, 7) |
| `…Wave/DistantBattle_DBU_Count` | 15 | 1–28 units |
| `…Wave/DistantBattle_DBU_Siege_Count` | 15 | 0–8 (0 in 31) |
| `…Wave/DistantBattle_WaveRequirements` | 11 | `…_DBUs_Remaining` (0–3), `…_DelayAfterConditionsMet` (0–90, seconds presumably), `…_Wave_RequiredZoneEvent` (world event 1815; 6 waves) |
| `…Wave/DistantBattle_ZoneEvent_FullClear` | 20 | world event 1887 (10 waves) |
| `…Wave/DistantBattle_NoSecondaryQuest` | 21 | 6 waves |
| `DistantBattle_Objective_List[]` / `DistantBattle_Objective` | 12 / 11 | 4–9 objectives (85 in all) |
| `…Objective/DistantBattle_Objective_Quest` | 20 | quest 1398 |
| `…Objective/DistantBattle_Objective_ZoneEvent` | 20 | world event 1815 |
| `…Objective/DistantBattle_AllowedFlank` | 8 | 1, 2, 3 |
| `…Objective/DistantBattle_DBU_Count`, `…_Wave_Count` | 15 | 2–14 units, 1–3 waves |
| `DistantBattleUnit_WeightedList[]` / `DistantBattleUnit_WeightingPair` {`DistantBattleUnit_Entry`, `…_Weight`} | 12 / 11 | 3–6 pairs; units 3725; weights 1–8 |
| `DistantBattleUnit_Siege_WeightedList[]` / same pair | 12 / 11 | 0 (10 scripts) or 1 pair; weight 1 |

The object form holds the objective quests, in objective order, as one array.

<a id="s8-4"></a>
### 8.4 Distant-battle units (class 3725)

A distant-battle unit is an entity that the client draws as a block of many cosmetic figures
(soldiers, siege engines) standing in formation. 1,335 static entities of the landblocks
place a 3725 record (§14.3). Each record has 53 to 56 top-level properties (53 in 21 records,
54 in 16, 55 in 47, 56 in 12). 52 properties occur in every record: the unit fields marked
"all" below and generic entity fields (`PhysObj`, physics ethereality and detection types,
`Usage_Action`, `Usage_MinUsageDist`, `WeenieType`, `Entity_CreationType` 6 or 7, `Name`,
selection flags, and default state values). The rest are the fields of the table below whose
present count is not "all", and three optional entity fields: `Render_NeverSelectable`
(14 records), `Entity_ClassScriptTable` (3) and `Entity_AdditionalScriptTables` (1). Unit
fields:

| Path | Type | Present | Observed |
|---|---|---|---|
| `DistantBattleUnit_EntityComposition[]` / `DistantBattleUnit_EntityStruct` | 12 / 11 | all; 1–6 groups (376 in all) | One group per kind of figure |
| `…Struct/DistantBattleUnit_Formation_Unit` | 20 | every group | creature templates 1723 (359), NPCs 1724 (8), items 796 (9); distinct within a record |
| `…Struct/DistantBattleUnit_CosmeticEntity` | 20 | every group | entity descriptor 0x47 |
| `…Struct/DistantBattleUnit_Entity_Offset_Array[]` / `DistantBattleUnit_Entity_Offset` | 12 / 6 | every group; 1–3,066 offsets (26,581 in all) | (lateral, 0, depth) in metres (below) |
| `…Struct/DistantBattleUnit_VisualRadius` | 9 | every group | 0.86–9.99 |
| `…Struct/DistantBattleUnit_Formation_Jitter` | 9 | every group | 0–10 |
| `…Struct/Appearance_ClassList[]` / `Appearance_KeyInfo` {`Appearance_Key`, `Appearance_AprFile` (0x20), `Appearance_Modifier` 0–1} | 12 / 11 | 26 groups in 18 records; 1–7 entries | Appearance variation (`appearance.md`) |
| `DistantBattleUnit_Formation[]` / `DistantBattleUnit_FormationGroup` | 12 / 11 | all; same units as the composition, possibly in another order | Layout parameters (below) |
| `…Group/DistantBattleUnit_Formation_Unit`, `…_Formation_Jitter` | 20, 9 | every group | equal, unit by unit, to the composition's |
| `…Group/DistantBattleUnit_Formation_Priority` | 15 | every group | 1–6 |
| `…Group/DistantBattleUnit_Formation_LateralDepth`, `…_PosteriorDepth` | 15 | every group | 0–2, 0–3 |
| `…Group/DistantBattleUnit_Formation_LateralWidth_Ratio` | 9 | every group | 0.1–1 |
| `…Group/DistantBattleUnit_Formation_LayoutCalculator` | 20 | every group | classes 3757 (267), 3856 (109) |
| `DistantBattleUnit_Formation_Dimensions` | 6 | all | (minimum lateral, maximum lateral, depth); symmetric (min = −max) in 58 of 96 |
| `DistantBattleUnit_Formation_Width` | 8 | all | width class 1–7 (§8.5) |
| `DistantBattleUnit_Composition[]` / `DistantBattleUnit_SpawnUnit` | 12 / 20 | all; 1–3,066 (26,581 in all) | creature, NPC or item templates |
| `DistantBattleUnit_SpawnRate`, `…_SpawnRate_Variance` | 9 | all | 6–25, 0–5 |
| `DistantBattleUnit_Min_Activator_Count`, `…_Max_Activator_Count` | 15 | all | 1–6, 1–10 |
| `DistantBattleUnit_Activator_Respawn_Time` | 9 | all | 0–30 |
| `DistantBattleUnit_StartFollowDistance`, `…_StopFollowDistance` | 9 | all | 12–15, 5–15 |
| `DistantBattleUnit_Missile_RequiredArcherUnit`, `…_CurrentCount` | 20, 15 | 53 | creature 1723; 2–526 |
| `DistantBattleUnit_FireMissileScript` | 8 | 56 | FX key (two values) |
| `BigBattle_Interactive_Ammunition_Shell` | 20 | all | class 3826 (56), 0 (40) |
| `BigBattle_Interactive_Aim_Vertical`, `…_Impulse_Max`, `…_Impulse_Max_Base` | 15, 9, 9 | all | 0–75; 0–120 |
| `DistantBattleUnit_GroundClampEntities` | 21 | 22 | **true 9, false 13** (default true, §15.1 B5) |
| `DistantBattleUnit_CaptureQualityOverride` | 8 | 4 | 2, 3 (default automatic) |
| `DistantBattleUnit_LaneType` | 8 | 3 | 1 (siege); default 2 |
| `Render_FarViewDistance` | 9 | 33 | 500, 800, 4,000 |
| `DBUSpawnEffectArray`, `DBUSpawnEffectArray_Forwarded` | 12 | all | empty |

**Composition rule (Confirmed, Derived).** `DistantBattleUnit_Composition` holds, as a
multiset, each group's `Formation_Unit` repeated once per offset of that group (96 of 96);
the order matches the formation order in 23 records and differs in 73. Offsets: the second
component is 0 in all 26,581; the first lies within the dimensions' lateral range and the
third within [0, depth] (96 of 96).

The `DistantBattleUnit_Formation` parameters and the layout calculators are the inputs from
which the stored offsets were made; the client does not read them (§15.1 B5, B7).

> **OPEN:** The frame of the offsets (which world axis is "lateral" and which is "depth",
> and how the unit's heading rotates them), the meaning of `Formation_Jitter` and whether a
> server must use `Composition` (the spawnable creature for each figure) when the unit
> fights. Class 3826 (ammunition shells) and the layout-calculator classes 3757 and 3856 are
> not specified anywhere.

<a id="s8-5"></a>
### 8.5 Formation control (class 3745, label `DistantBattleUnitFormationControl`)

`DistantBattleUnit_FormationControl_SpacingBetweenGroups` (2) and
`DistantBattleUnit_FormationControl_Width_Lookup[]` (7 structs {`DistantBattleUnit_Formation_Width`,
`DistantBattleUnit_FormationControl_ActualWidth`}): each width class 1–7 has an actual width
(5–160 m). Invariant (Confirmed, 96 of 96): a unit's dimension width (max − min) is at most
the actual width of its width class. The object form is one map width class → float bit
pattern of the actual width.

<a id="s8-6"></a>
### 8.6 Distant-battle unit groups (class 4026)

The 16 records of class 4026 (`creatures.md` §2.1) carry the composition, entity-composition,
formation, dimensions and width fields of §8.4 (and `CaptureQualityOverride` in 7,
`GroundClampEntities` in 10) together with a full creature property set. No record of this
family names a 4026 record; landblock content does not place any either.

<a id="s8-7"></a>
### 8.7 Battle statistics (classes 3576, 3590, 3622)

The control record (3576, label `BattleStatControl`) has an empty property set; its object
form is an array of 9 statistic DIDs (6 of class 3590, 3 of class 3622). The statistics
themselves (3590: 10 records, 3622: 3; both descend from 3584):

| Path | Type | Observed |
|---|---|---|
| `BattleStat_Name`, `BattleStat_Description` | 13 | all |
| `UI_Table_ColumnDesc_NameOverride`, `…_TooltipOverride` | 13 | all |
| `BattleStat_Scope` | 8 | 3 (player) in all |
| `BattleStat_UpdatePolicy` | 8 | 1 set (4), 2 add (8), 4 maximum (1) |
| `BattleStat_ReplicateToClients` | 21 | true 9, false 4 |
| `BattleStat_ForwardUpdatesToStat` | 20 | another 3590 (4) |

---

<a id="s9"></a>
## 9. Session play (classes 1925 and 3423)

`SessionPlayControl` (1925):

| Path | Type | Observed |
|---|---|---|
| `SessionPlay_AllowedSlotCount_List[]` / `…_Entry` {`SessionPlay_DID`, `SessionPlay_AllowedSlotCount`} | 12 / 11 | 87 entries; sessions of class 1930; counts 0–500 (250 in 39) |
| `SessionPlay_SessionCrashedMessage`, `…_SessionFailedToStartMessage`, `…_SessionResumptionMessage`, `…_UnableToResumeMessage` | 13 | one each |

The object form is one map session DID → count. `SessionPlayLevelDirectory` (3423) is empty in
both forms. Session records (class 1930, monster-play and session characters) are not
specified here; 44 of them name a template with `PrivateEncounter_CharGenDirect`.

> **OPEN:** What the allowed slot count limits (concurrent players of a session, or
> characters per account), and what the empty level directory stood for.

---

<a id="s10"></a>
## 10. Fellowship maker (class 3276, label `FellowshipMakerControl`)

<a id="s10-1"></a>
### 10.1 Property form

| Path | Type | Observed |
|---|---|---|
| `FellowshipMakerControl_MinLevel` | 15 | 10 |
| `FellowshipMakerControl_ValidEncounterSizeArray[]` / `…_EncounterSize` | 12 / 8 | 2, 3, 6 |
| `FellowshipMakerControl_MatchTimeWorldPropertyBySizeArray[]` / `…_Entry` {`…_EncounterSize` (1–3), `…_MatchTimeWorldPropertyName` (PID)} | 12 / 11 | 3 world properties (`World_FellowshipMaker_TimeBeforeMatching…`) |
| `FellowshipMakerControl_BonusEffectThreshold_Array[]` / `…_Entry` {`…_BonusEffect` (effect 734), `…_BonusEffect_Threshold` (0, 0.25, 0.5, 0.75, 1)} | 12 / 11 | 5 entries |
| `FellowshipMakerControl_RandomBonusEffect` | 20 | 0 |
| `FellowshipMakerControl_DeserterEffect`, `…_OnFMTeleportEffect` | 20 | effects 734 |
| `FellowshipMakerControl_ExcludedEncounterArray[]` / `…_EncounterDID` | 12 / 20 | one template |

`MatchTimeWorldPropertyBySize` pairs group-size keys 1, 2 and 3 with the world properties for
solo, duo and trio matching time (by their labels), while the valid sizes are 2, 3 and 6.

> **OPEN:** How the match-time keys 1–3 relate to the valid sizes 2, 3, 6 (the object form
> keeps two maps between the order numbers 1–3 and other enum keys, §10.2 item 5).

<a id="s10-2"></a>
### 10.2 Object form: the matching index (Confirmed, Derived)

The 25 KB object form is an index the server built from the templates. Its parts are numbered
below by topic, not by stored position; the table after the list gives the stored order of the
root's attributes (attribute names are hashes, so position and native class identify them):

1. **Candidate list**: an array of 557 entries, each an object of class 3141 with two
   integers: a **flag word** and a template DID (107 distinct class-2651 templates).
   **Order (Confirmed, 557 of 557 entries):** the entries of one template are contiguous;
   templates follow the finder order of the world-join control's object form (§13.3: the
   available list in stored order, then the grouped instances by ascending group label; the
   107 templates occur at strictly increasing positions of that array, 13 of them grouped big
   battles); within a template, the
   entries run over its `Skirmish_Template_GroupSizeArray` in stored order, skipping sizes that
   are not valid sizes, and for each size over its `Skirmish_Template_DifficultyTierArray` in
   stored order (size first, then tier; 107 of 107 templates, including the 2 whose stored
   sizes are not ascending).
2. **Flag word bits** (557 of 557 entries agree with the template data):

   | Bits | Meaning |
   |---|---|
   | 0x1 | the template is a skirmish (`WorldJoin_EncounterType` 3) |
   | 0x2 | the template's category is the capped category (§7.1) |
   | 0x4 | none of the other four kind conditions (a classic instance) |
   | 0x80000 | the template is a big battle (names a distant-battle script) |
   | 0x100000 | the template's category is seasonal |
   | 0x20, 0x40, 0x80 | group size 2, 3, 6 (the valid sizes, in order) |
   | 0x80 << *t* (0x100 … 0x1000) | difficulty tier *t* = 1 … 5 |

   Every entry has exactly one of the five kind bits, one size bit and one tier bit. For each
   template the entries are exactly the products of its tiers and those of its group sizes
   that are valid sizes (107 of 107).

   **Kind bits are disjoint in this install.** No template meets two of the kind conditions:
   of the 166 templates with `WorldJoin_EncounterCategory`, 22 are skirmishes, 5 capped,
   7 seasonal, 14 big battles and 118 none of these (0 in two kinds); over the 557 entries the
   kind bits are 0x1 (171), 0x2 (24), 0x4 (330), 0x80000 (13) and 0x100000 (19). Bit 0x4
   marks a template that meets none of the other four conditions.

   > **OPEN:** Which kind bit a template gets when it meets several conditions (for example a
   > seasonal skirmish or a capped big battle). The data has no such template, so the
   > precedence cannot be observed; a writer that meets one should warn.

   > **OPEN:** Which finder templates get entries. 107 of the 164 finder templates have
   > entries. Of the other 57, 33 have no valid size and 1 is the fellowship maker's excluded
   > template, but 23 (22 of class 2651, 1 of class 1368) have a valid size, are not excluded
   > and still have none; the data examined does not show what sets them apart.
3. **Per-flag bitsets**: a map from a flag bit to a bitset over the 557 entries; bit *i* is
   set exactly when entry *i*'s flag word has that flag (20 of 20). A bitset stores only up to
   its last set bit. **Key set (Confirmed):** exactly 20 keys, every single bit from 0x1 to
   0x100000 (bit positions 0–20) except 0x4000 (bit 14). The 13 bits that occur in some
   entry's flag word (0x1, 0x2, 0x4, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0x800, 0x1000,
   0x80000, 0x100000) map to non-empty bitsets; the other 7 keys (0x8, 0x10, 0x2000, 0x8000,
   0x10000, 0x20000, 0x40000) map to an empty bitset (bit count 0), not to a null reference.
   No key outside that range and no multi-bit key occurs.

   > **OPEN:** The rule that chooses the key set. It is not "every bit up to the highest used
   > bit" (0x4000 is skipped), and it does not follow the tier formula of item 2 (0x80 << 7 =
   > 0x4000 would be tier 7, while 0x40000 would be tier 11). No enum mapper of the install
   > has these keys (all 735 mappers checked). A writer that rebuilds this map should emit
   > exactly the 20 keys above, with an empty bitset for a key no entry uses, until the rule
   > is known.
4. **Encounter map**: template DID → (size key 2, 3 or 6 → (tier → entry index)), 121
   templates (14 with an empty map); every index points at an entry with that template, size
   bit and tier bit (557 of 557).
5. The valid sizes (2, 3, 6), the excluded template set, the five bonus-effect thresholds
   (class-3358 objects: effect, threshold), and two maps between size order (1–3) and enum keys.
6. Four more bitsets over the 557 entries (389, 399, 507 and 13 bits set) and two arrays of 7
   references to bitsets (null at indices 0–2, respectively 0–1).

Root attributes in stored order (17; Confirmed):

| Position | Value | Part |
|---|---|---|
| 0 | bitset (class 57), 389 bits set | item 6 |
| 1 | integer 6 | OPEN |
| 2 | reference array (class 176) of 5 class-3358 objects | item 5, bonus thresholds |
| 3 | reference map (class 35), flag bit → bitset, 20 keys (item 3 gives the key set) | item 3 |
| 4 | integer array (class 17) 2, 3, 6 | item 5, valid sizes |
| 5 | reference array, 7 elements, null at 0–2 | item 6 |
| 6 | set (class 18), one template | item 5, excluded templates |
| 7 | map (class 11), enum key → size order 1–3 | item 5 |
| 8 | reference array, 7 elements, null at 0–1 | item 6 |
| 9 | map, size order 1–3 → enum key | item 5 |
| 10 | bitset, 399 bits set | item 6 |
| 11 | integer 2 | OPEN |
| 12 | reference map, template → size map, 121 keys | item 4 |
| 13 | reference array of the 557 class-3141 entries | item 1 |
| 14 | bitset, 507 bits set | item 6 |
| 15 | null reference | OPEN |
| 16 | bitset, 13 bits set | item 6 |

> **OPEN:** The meaning of the four bitsets of item 6, of the two 7-element arrays, of the
> root's integer attributes (values 6 and 2) and of its null attribute. Given the set of
> templates that get entries (OPEN in item 2), a server can rebuild items 1–4 from the
> templates with the order and bit rules above; whether it must honour item 6 is unknown.

---

<a id="s11"></a>
## 11. Phasing box (class 3369)

The single class-3369 record is an entity template (ancestry 3372 > 1733 > 787) with a
generic entity property set, `Name`, `Render_Opacity` 0.5, creation type 8, and
`PhasingBox_UpdateOnlyOnCreation` true. Placed content uses the phasing flags directly:
`PhasingBox_DelayRefreshOfLinkedEntities` on 5,867 instance property sets and 539 static
entities, `PhasingBox_UpdateOnlyOnCreation` on 5,816 and 529 (§14.3); one NPC carries
`Phasing_ForcePhasingRefresh_OnEnterWorld`. Client behaviour: §15.1 B6.

---

<a id="s12"></a>
## 12. Dialog singletons

Classes 1766, 1769, 1772, 2753, 2757, 2759, 3060, 3317 and 3331 have one record each (36
bytes, empty property set, a root object without attributes; all descend from 1728). Their
only content is their class and their WEENIECONTENT label (§2.3), which names a confirmation
dialog of this family (enter or leave an instance, abandon an instance, finish a session
quest, world-join queue, request and dequeue, fellowship-maker confirmation and vote). No
dialog text is stored in them.

> **OPEN:** How the client uses these records (the labels suggest that a dialog's callback
> object is created from the record's class); not examined under BIN.

---

<a id="s13"></a>
## 13. Object-state forms (Confirmed)

All records of every class of §2.1 decode completely. Correspondences confirmed over all
records of each class:

<a id="s13-1"></a>
### 13.1 Templates (1368, 2651)

15–66 items per record (1368) and 22–88 (2651). Confirmed parts:

- One class-161 position per `…_ContainedLandBlock`, in stored order: the landblock rule of
  §4.3 (1,511 of 1,511). Each has region, landblock and cell present, cell 0, and no
  instance, offset or orientation. Its flag byte follows the rules of `property-system.md`
  §4.6: 0xCB (203) when the region is 1 (1,288 positions) and 0x8B (139) otherwise (3,340);
  bit 0x40 is the only difference (4,628 of 4,628).
- Class-160 positions: the resolved place-key fields (0–8 per record).
- A root integer attribute equal to `…_LengthDefault` (0 when absent; 1,511 of 1,511).
- (2651) Root attributes equal to `Skirmish_Template_GroupSizeArray` and
  `…_DifficultyTierArray` as integer arrays in stored order, and a map tier → group size
  equal to the tier-minimum list (552 of 552).
- A class-3324 object whose second attribute equals `…_InstanceQuest` (0 when absent;
  1,511 of 1,511; the 3 team templates have three such objects), with a StringInfo and
  positions; class-1602 wrappers around class-166 property sets for the permission blobs
  (`quests.md` §6.1 uses the same wrapper).

<a id="s13-2"></a>
### 13.2 Controls

| Class | Object form |
|---|---|
| 1366 | lockable list (array), entry filters (array), ignored encounters (set), PvMP list (array), map lock kind → reset-days bits |
| 2573, 2576 | the integers of §5.2 |
| 2515 | 257 items: 16 maps, 231 integer arrays, 4 reference maps and five class-39 single properties (a compiled form of §6.3; item-level correspondence not established) |
| 2669, 3399 | one set of DIDs |
| 2727, 3894, 3423, dialogs | no attributes |
| 1925 | map session → slot count |
| 3745 | map width class → actual width (float bits) |
| 3787 | map medal → class-3872 object (9 integers per medal), map quest → points (107), array of the 99 quest DIDs of the quest-to-encounter hash, set of the 14 big-battle templates |
| 3808 | array of the objective quests |
| 3725 | the set of formation units (as an array, any order), a second DID array (OPEN), three null references |
| 3576 | array of battle-statistic DIDs (stored as DID items) |
| 3590, 3622 | name and description StringInfos |
| 3276 | §10.2 |

<a id="s13-3"></a>
### 13.3 World-join control (2606)

Three root attributes, listed here in their stored order (they are told apart by native
class as well, `wstate.md` §5.3):

1. a map (class 11) template → position in the array of item 3 (0–163; 164 of 164);
2. a set (class 18) of the 5 seasonal category keys;
3. an integer array (class 17) of 164 template DIDs: the available list in stored order,
   followed by the grouped instances taken group by group in **ascending order of
   `WorldJoinControl_Instance_Group_Label`** (1, 2, … 8), each group's
   `…_Grouped_Instance_List` in its stored order (164 of 164 positions). The groups' stored
   order in the property form is a different one (§7.1); following it would misplace
   positions 150–157 of this install.

The position is the finder's sort index.

---

<a id="s14"></a>
## 14. Reference graph

<a id="s14-1"></a>
### 14.1 Outbound edges (Confirmed)

| From | Field | To (class) |
|---|---|---|
| template | `InstanceQuest`, quest arrays, `EpicQuest`, featured quests | quest 1398 |
| template | `EventControlDID` | world-event list 2161 |
| template | `MemberCountZoneEvent`, `DynamicTelepad_ZoneEvent` | world event 1887 |
| template | `PropertyResponseMapDID` | 2192 |
| template | `TeleportationSceneID`, `SuccessTeleportSceneDID` | scene 1468 |
| template | `IntroVO` | sound info (0x2A) |
| template | `SpeciesLevelLootTable` | 2001 |
| template | `AwardCurrency`, `WebStoreAccountItem_DataID` | items 796; store 2532, 2784 |
| template | `DistantBattleScript`, `EncounterTicket`, `LengthRipcord`, `WhiteList`, `DefeatInfoOverride`, `Induction_ActionDID`, `RandomEffect` | 3808, 3894, 2801, 3049, 1035, 1195, 734 |
| template | permission blobs | quests 1398, world events 1815 / 1887 |
| 1366 | lists | templates, filters 2573 / 2576, induction 1195 |
| 2606 | lists, `DefaultTicket`, featured world event | templates, 3894, 1887 |
| 2515 | traits, token, gate items, zone events | 1477, 796, 2532, 1887 |
| 3787 | quests, encounters, loot, progression, zone events | 1398, 2651, 3666, 1385, 1815 |
| 3808 | units, quests, events | 3725, 1398, 1815 / 1887 |
| 3725 | units, shells, entity descriptors | 1723 / 1724 / 796, 3826, 0x47 |
| 3276 | effects, excluded template | 734, 2651 |
| 1925 | sessions | 1930 |
| 2669 | traits | 2525 |
| 3894 | ripcords | 2801 |

<a id="s14-2"></a>
### 14.2 Inbound edges (Confirmed, all 0x79 and 0x78 sets)

| To | From (field) | Count |
|---|---|---|
| templates 1368 | quest roles `QuestDispenser_PrivateEncounterTemplate` (`Quest_RoleArray` 1,422; objective NPC roles 731; global roles 449) | 2,602 |
| templates 2651 | the same roles (808, 400, 392) | 1,600 |
| templates 2651 | `QuestDispenser_OpenInstanceUI_PrivateEncounterTemplate` (role array 27, objective roles 27) | 54 |
| templates 2651 | `SkirmishTreasureLookup_EncounterDID` (2728) | 8,326 (63 templates) |
| templates 1368 | `PrivateEncounter_TravelArray[]/PrivateEncounter_Template` (NPCs 1724) | 455 |
| templates 1368 / 2651 | `Effect_Travel_PrivateEncounter` (travel effects 749) | 196 / 85 |
| templates 1368 | `Area_AreaInstanceStruct/Area_DefaultInstanceTemplate` (areas 690) | 44 |
| templates 1368 | `PrivateEncounter_CharGenDirect` (1930) | 44 |
| templates 1368 | `RaceTable_StartingEncounter`, `RaceTable_BTS_StartingEncounter` (860) | 8 + 8 |
| templates 1368 | `AdvTable_AdvancedCharacterStart_StartingEncounter` (3335) | 9 |
| templates 1368 / 2651 | `PrivateEncounter_LinkedTemplate` (815) | 2 / 1 |
| templates | control lists of 1366, 2606, 3787, 3276 | §5.1, §7.1, §8.1, §10.1 |
| 2727 | `SkirmishTreasureLookup_SkirmishMatchParametersDID` | 8,326 |
| 2669 | `Skirmish_SkillTrainer_Skirmish` (1724) | 25 |
| 3725 | distant-battle scripts (64 + 4) | 68 (60 distinct units) |
| 3808, 3894 | templates | 14, 14 + 1 |

Of the 1,511 templates, 1,463 have at least one inbound reference from a property set.

<a id="s14-3"></a>
### 14.3 Landblock content (Confirmed, all 49,636 landblock-info records)

| Kind of content | Property | Count |
|---|---|---|
| static entities | `PrivateEncounter_LinkedTemplate` → 1368 / 2651 | 317 / 145 |
| instance property sets | `PrivateEncounter_LinkedTemplate` → 1368 / 2651 | 454 / 268 |
| instance property sets | `PrivateEncounter_SetLock`, `…_SetLockOnDeath` | 134, 43 |
| static entities | placing a distant-battle unit (object = a 3725 record) | 1,335 |
| static entities and instance sets | `DistantBattleUnit_Rotation`, `…_CreationLocation`, `…_MissileTargetWeight`, `DistantBattle_AllowedFlank` | 608 / 822, 80 / 80, 28 / 28, 124 / 124 |
| instance property sets | `Skirmish_*` encounter-manager, control-point, bastion, survival and point-calculator properties | 36 labels, 1–416 each |
| instance property sets | `BigBattle_Interactive_*`, `BigBattle_Distant_FX` | 11 labels |
| static entities and instance sets | phasing flags (§11) | |

<a id="s14-4"></a>
### 14.4 Instance cards for the explorer (algorithm; Tessera rule)

To build the card of a template *T*:

1. Read *T*'s property set. Kind (§4.1), name, description, capacity, timers, level range and
   tiers come from §4.2 and §4.10.
2. **Map**: list the landblocks of §4.3 (deduplicated); link each to the terrain view. Resolve
   the place keys of §4.3 through `map-data.md` §3.4 and show them as pins.
3. **Quests**: the quests of §4.6 ("in this instance"); plus every quest whose role names *T*
   (§14.2, "starts or opens this instance"), found with a reverse index over role fields.
4. **Loot**: the species table (§4.9); for class 2651 the skirmish look-ups that name *T*
   (`economy.md` §4.10) grouped by match-parameter band (§6.1); the award currencies.
5. **Entry points**: landblock content and records with `PrivateEncounter_LinkedTemplate`,
   travel arrays and travel effects naming *T* (§14.2–§14.3).
6. **Finder**: whether *T* is available, featured, grouped (with its group key), seasonal or
   capped (§7.1), and its sort index (§13.3); for big battles the distant-battle script and its
   units (§8.3–§8.4).

Build all reverse indices once by walking every property set; never assume a fixed DID.

---

<a id="s15"></a>
## 15. Behaviour

<a id="s15-1"></a>
### 15.1 What the client does (BIN)

**B1. Featured instance.** The client reads `WorldJoinControl_FeaturedInstance_Array` from the
world-join control and the integer world property `WE_Featured_Instance_Driver`. If the
property is absent, not an integer, or below 1, there is no featured instance. Otherwise the
featured template is element *n* − 1 of the array (*n* the property's value); if that element
does not exist or is not a DID, there is none.

**B2. Level-scaling level.** For a template, the client reads
`Skirmish_Template_LevelScalingLevel` (an integer; absent or not an integer → 0). If the value
is below 1 it is the result. Otherwise, if the world property
`WE_skirmish_template_levelscalinglevel_override` exists and is an integer, its value is the
result; else the template's value is.

**B3. Template of an instance group.** Given a group key and a group size, the client finds
the group with that `WorldJoinControl_Instance_Group_Label` and walks its
`…_Grouped_Instance_List` in stored order; the result is the first template whose
`Skirmish_Template_GroupSizeArray` contains the size; if none does, there is no template.

**B4. Skirmish statistics panel.** For a derived statistic (`Skirmish_Stats` entry):

1. Start with the value of the primary statistic.
2. For each operation in stored order, take the value of the secondary statistic and apply
   `Mod_Op`: 2 divide (if the divisor is 0 the running value becomes 0 and the next operation
   follows), 6 subtract, 7 add, 8 multiply; any other key leaves the value unchanged.
3. If `Skirmish_Stat_Scale` is not 1, multiply by it.

Before step 2 the client chooses the arithmetic: floating point when the primary statistic or
any secondary statistic is a float statistic or any operation is a divide; integer
arithmetic otherwise (step 3 then multiplies integers). When the panel combines a statistic over several group sizes or both
level classes, it adds the values, except for best time (key 16): a time at or below
`SkirmishStats_CONSTANT_FASTEST_POSSIBLE_SKIRMISH_TIME` counts as "no time"; among real
times the smallest wins, or the largest when the template has
`Skirmish_Template_Highest_Time_Is_Best_Time`; a real time always beats "no time".
The panel also loads `SkirmishControl_BlockedStats_List`.

> **OPEN:** What the panel does with the blocked statistics (hide them, or exclude them from
> totals).

**B5. Distant-battle unit display.** For an entity that carries
`DistantBattleUnit_EntityComposition`, the client:

1. reads every group's `DistantBattleUnit_CosmeticEntity`, `DistantBattleUnit_VisualRadius`,
   `Appearance_ClassList` and `DistantBattleUnit_Entity_Offset_Array`, and creates one
   cosmetic figure of the entity descriptor per offset, with the appearance keys applied;
2. reads `DistantBattleUnit_CaptureQualityOverride` (absent → automatic, 0xFFFFFFFF) and
   passes it with the figures;
3. reads `DistantBattleUnit_GroundClampEntities` (absent → **true**) and sets the figures'
   ground clamping; it re-applies the flag whenever the property changes;
4. watches `DistantBattleUnit_Composition_Removed_List`: each element is a struct with
   `DistantBattleUnit_Formation_Unit`, `DistantBattleUnit_Removal_Count` (absent → 1),
   `DistantBattleUnit_Removal_ScriptKey` and `…_Removal_ScriptParameters`; for each element it
   removes that many figures of the group whose `Formation_Unit` matches and plays the removal
   script on them. When the list grows, only the new elements are applied;
5. builds a box from `DistantBattleUnit_Formation_Dimensions` (*a*, *b*, *c*) and the unit's
   position (*x*, *y*, *z*): half-width *h* = (*b* − *a*) × 0.5; the box spans *x* − *h* … *x* + *h*,
   *y* … *y* + *c* and *z* … *z* + 1. (When the property is absent the box is the point.)

The client reads none of `DistantBattleUnit_Formation`, its parameters, `Composition`,
`Formation_Width`, the formation control record, spawn rates or activator counts (B7).

> **OPEN:** Which figures are removed first, how a figure is placed from its offset (frame,
> heading, jitter, ground clamping), what the box of step 5 is used for, and which messages
> fill the removal list (protocol, decision 0004).

**B6. Phasing boxes.** The client keeps a phasing manager that tracks boxes (entities with the
phasing-box flags) and the entities linked to each box. When a box's evaluated state
changes, the manager refreshes the linked entities at once, unless the box has
`PhasingBox_DelayRefreshOfLinkedEntities`. A box with `PhasingBox_UpdateOnlyOnCreation` is
evaluated when it is created and is not registered for later updates. When an entity that
carries `Phasing_ForcePhasingRefresh_OnEnterWorld` enters the world, the manager runs a
refresh.

> **OPEN:** What a box's state depends on (its volume, a world event, a quest), how entities
> are linked to a box, and what a delayed refresh waits for.

**B7. Properties the client does not name.** Of the 330 family properties stored in the records
of §2.1, 253 never occur as an immediate operand anywhere in the program. They include the
template timers and `MaxPlayers`, the landblock list, the content layer, the instance-quest,
bestowal and world-event fields, the award tables, `LockSize`, `LockResetDays` and the lockable list, the
entry-filter messages and error keys, every `DistantBattle_*` script field, the formation
parameters, spawn and activator fields of distant-battle units, the session-play list and the
fellowship-maker property form. The 77 with hits include the fields of B1–B6 and the
instance-finder and statistics user interfaces; the others (among them
`PrivateEncounter_Control_LockStruct`, `SkirmishMatchParams_Level_Min`, the success-teleport
fields, `PrivateEncounterTemplate_DisallowedMapNoteType`, `…_TeamName`) were not examined:
many hits are user-interface element identifiers or struct offsets that share the value.
Caveat: code that reaches a property through a computed or data-driven PID does not show up
in such a search, and object-form data (§13) cannot be searched this way. With that caveat,
entering, scaling, locking and ending instances are server behaviour; the client receives
their results.

<a id="s15-2"></a>
### 15.2 What only a server decides (OPEN)

A server or offline sandbox that runs this content needs answers the data does not give:

> **OPEN (entry):** the order of the checks when a player asks to enter (creation blob,
> default blob, lock, entry filter, ticket ripcords, minimum level, billing tokens, group
> size and tier gates of §4.10); which error key is sent for each failure.

> **OPEN (instances):** when an instance is created, shared (public instances and
> `MaxPlayerWorldProp`), and destroyed; the meaning and units of `TimeDormantLimit`,
> `TimeGraceLimit`, `FailureTimeoutOverride`, `DefaultEncounterTimeout` and `LengthDefault`;
> what `LoadPermittedLandBlocksOnly` and `ShouldLoadAllLandblocks` change.

> **OPEN (scaling):** how the level-scale minimum and maximum, the override world properties
> and the level-scaling level combine into the level of creatures; how group size and tier
> scale them (the group tiers and zone events of §6.3 are the inputs).

> **OPEN (locks):** what each lock kind 1–5 locks (per template, per chest with the
> `RaidChestLock` properties of §2.4), the reset time of day and time zone, and what the soft
> lock does.

> **OPEN (rewards):** the base amounts behind `AwardRatio`, the point formula that the
> skirmish-point multipliers of §6.3 apply to, and how big-battle medals turn into trait
> points, cash-out loot and forwarded rewards (§8.1).

> **OPEN (distant battles):** how waves start (requirements, delays), how units are drawn from
> the weighted lists and placed on flanks, and how objectives complete.

> **OPEN (matching):** how the fellowship maker uses its index and match-time world
> properties, and how bonus thresholds pick an effect.

---

<a id="s16"></a>
## 16. Invariants

Each is Confirmed on every record of this install. A reader should check them on synthetic
fixtures and may warn (not fail) when real data breaks one.

| # | Invariant |
|---|---|
| N-I1 | Every record of §2.1 has a property set at DID + 0x09000000 that decodes completely, and an object state that decodes completely. |
| N-I2 | Every non-zero 0x45 DID in the fields of §4–§12 names an existing record of the classes listed for that field. |
| N-I3 | Every template has a name, description and content-layer key; the key is present in DID map 0x28000012. |
| N-I4 | Every `…_ContainedLandBlock` vector holds three non-negative integers; the landblock is (region, ⌊X ÷ 8⌋, ⌊Y ÷ 8⌋) with both indices below 256. |
| N-I5 | Class 2651: `DifficultyTierArray` is 1 … *n*; `MinLevelScale` ≤ `MaxLevelScale`; the tier-minimum list has entry *k* with tier *k*; `MaxPlayers` (when present) = the largest group size. |
| N-I6 | The lockable list of the control record equals the set of templates with `LockSize`. |
| N-I7 | Every featured template is in the available list; no grouped template is. |
| N-I8 | The world-join object form lists the available templates in stored order, then the grouped ones group by group in ascending label order (each group's list in stored order), and maps each to its position. |
| N-I9 | Each distant-battle script is named by exactly one template; each big-battle template is in the `BigBattles` set. |
| N-I10 | Distant-battle unit: composition multiset = each group's unit repeated per offset; the units of `Formation` and `EntityComposition` are the same set, with equal jitter per unit; offsets have second component 0 and lie inside the dimensions; dimension width ≤ the actual width of the width class. |
| N-I11 | Fellowship-maker index: every entry's flag word has one kind bit, one size bit and one tier bit; entries are in the order of §10.2 item 1 (finder order of templates as in N-I8, then sizes, then tiers, each in stored order); the per-flag map has exactly the 20 keys of §10.2 item 3; each per-flag bitset marks exactly the entries with that flag; every encounter-map index points at a matching entry. |
| N-I12 | The skirmish conversion hash has one entry per (statistic, size, on-level) for 19 statistics, 6 sizes and both level classes. |

---

<a id="s17"></a>
## 17. Worked synthetic examples

All DIDs below are invented (prefix 0x70F0…) and belong to no install.

<a id="s17-1"></a>
### 17.1 Landblocks of a template

A synthetic template has `…_ContainedLandBlockArray` = [(2, 984, 904), (2, 979, 911)]. The
landblocks are (2, 123, 113) and (2, 122, 113): 979 ÷ 8 = 122.4 → 122 and 911 ÷ 8 = 113.9 →
113. The object form holds two class-161 positions with region 2 and those landblocks, in
that order.

<a id="s17-2"></a>
### 17.2 Derived statistic

A synthetic `Skirmish_Stat`: primary statistic 1 (points from control points, int), operations
[(add, statistic 2), (divide, statistic 13)], scale 3,600. With 120 and 30 points and 5,400.0
seconds played: 120 + 30 = 150; 150 ÷ 5,400.0 = 0.02777…; × 3,600 = 100 points per hour. With
0 seconds played the result is 0.

<a id="s17-3"></a>
### 17.3 Best time over sizes

Best times (seconds) for one skirmish: solo 0.0 (never completed), duo 812.5, six 790.0; the
fastest-possible constant is 0.5; the template does not have
`Skirmish_Template_Highest_Time_Is_Best_Time`. The combined best time is 790.0. If the
template had the flag, it would be 812.5.

<a id="s17-4"></a>
### 17.4 Instance group

Group key 3 lists templates 0x70F00100 (group sizes 6, 12) and 0x70F00101 (sizes 12, 24).
Asking for size 12 gives 0x70F00100 (first in stored order); size 24 gives 0x70F00101;
size 3 gives none.

<a id="s17-5"></a>
### 17.5 Fellowship-maker entries

A template 0x70F00200 with tiers [1, 2], group sizes [1, 2, 6], `WorldJoin_EncounterType` 4,
not capped, not seasonal, no distant-battle script, gives four entries, in this order (§10.2
item 1: sizes in stored order, then tiers): 0x4 | 0x20 | 0x100 = 0x124 (size 2, tier 1),
0x4 | 0x20 | 0x200 = 0x224 (size 2, tier 2), 0x4 | 0x80 | 0x100 = 0x184 (size 6, tier 1) and
0x4 | 0x80 | 0x200 = 0x284 (size 6, tier 2). Size 1 is not a valid fellowship-maker size. If
the template's first entry has index *i*, its encounter map is {2: {1: *i*, 2: *i* + 1},
6: {1: *i* + 2, 2: *i* + 3}}.

<a id="s17-6"></a>
### 17.6 Distant-battle unit removal

A unit has two groups: unit 0x70F00300 with 40 offsets and unit 0x70F00301 with 10. The
removal list becomes [(0x70F00300, count absent, script *k*)]: the client removes 1 figure of
the first group and plays script *k*. The list then grows by [(0x70F00301, 4, script *k*)]: the
client removes 4 figures of the second group; the first element is not applied again.

---

<a id="s18"></a>
## 18. Notes for other specs

- `economy.md` §14 asks what the record named by `LootGenControl_RewardOnceTGPs` (class 3399)
  is for. Class 3399 is a generic DID-set class whose records are all WEENIECONTENT sets
  (§8.2); that record's set is empty in this install, so it lists no treasure group profile.
- `creatures.md` §2.1 lists class 4026 as distant-battle unit groups; §8.4 and §8.6 give the
  unit fields they carry.
- `map-data.md` §10 lists the place-key fields of templates; §4.3 adds the landblock-list
  encoding and §13.1 the resolved positions in the object form.
- `quests.md` §7.1 counts 4,202 template roles; §14.2 splits them by class and role list.

---

<a id="changelog"></a>
## 19. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Census of the family (1,833 records in 29 root classes, corrected from the 1,834 first stated; 1,322 dictionary definitions); property-form layouts of all classes; landblock-list encoding; content-layer link; entry-filter numbers and the fellowship-maker index from the object forms; reference graph and instance-card algorithm; BIN rows B1–B7 (featured instance, level-scaling level, instance groups, skirmish statistics, distant-battle unit display, phasing boxes, negative search). |
| 2026-10-07 | Revision after the first review (status stays Review). §2.1: record total 1,833 (was 1,834), 17-class subtotal 1,821 (was 1,822); per-class rows and byte totals unchanged. §3.2: table of the non-0x45 DID fields with their type codes, `ScriptTable_DBScriptTable` added. §4.1: the second "other" template carries `WorldJoin_EncounterType` 4 without a category. §4.10: tier-array distribution scoped to class 2651, team templates added; key-4 count explained. §5.3: 36 top-level properties (was 34), exact ancestry. §6.1: band-overlap paragraph replaces "adjacent bands share no level"; new OPEN on overlapping tier bands. §8.4: 53–56 top-level properties (was 60), with the 52 common and the optional ones. §10.2: entry order stated (Confirmed); stored order of the 17 root attributes; kind bits disjoint in this install, precedence OPEN; new OPEN on which finder templates get entries. §13.1: flag bytes 0xCB / 0x8B (was "203" for all). §13.3: attributes listed in stored order (map, set, array). §16 N-I11 includes the order. §17.5 follows the stated order. Provenance: OBS revision probes; rows I1, I12, I18, I19 extended; new rows I21–I26 for §3.2, §3.3, §3.4, §4.12, §5.3, §6.2. |
| 2026-10-07 | Second revision, after the re-review (status stays Review). §13.3 item 3, §10.2 item 1, §7.1, N-I8, N-I11: the grouped templates of the finder order follow ascending `WorldJoinControl_Instance_Group_Label`, each group's list in stored order, not the stored group order (was "in group order"). §3.2, §4.8: `IntroVO` has 942 values, 908 non-zero and 34 zero (was "908 values"). §10.2 item 3: the flag-bit map has exactly the 20 single-bit keys 0x1 … 0x100000 except 0x4000, 7 of them with empty bitsets (was "20 bits, 0x1 … 0x100000"); new OPEN on the rule behind the key set. Provenance: second-round OBS probes; rows I14, I18, I19, I21 extended. |
| 2026-10-07 | Second re-review passed; status Accepted (reviewer). No text change besides the header. |
