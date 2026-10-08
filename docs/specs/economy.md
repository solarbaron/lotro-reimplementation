<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Economy tables: loot generation, crafting recipes, vendors, barter and currency

| | |
|---|---|
| **Status** | Revised (OPEN sweep of 2026-10-09, change log last row; waiting for review). Previously Accepted |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Claude review sessions (spec-reviewer, not the author session): first review, second review (re-review) and third review (acceptance), all 2026-10-07; fourth review (2026-10-08 revision) 2026-10-08 |
| **Review record** | Third review 2026-10-07 (a session different from the author and both earlier reviewers): **passed**, Accepted. Full record moved to the review log (decision 0018). Fourth review 2026-10-08 of the 2026-10-08 revision (Claude review session, separate from the writer's): **passed**, Accepted. Details in the review log (decision 0018). OPEN sweep 2026-10-09 (spec writer, dirty room): normative text of §4.5, §4.8, §4.9 and §6.2 changed (rows E1–E3); status Revised until a separate session reviews it. |
| **Implements into** | `crates/formats` / `crates/props` (typed views of the records below, read from their property sets), `crates/wstate` (the object-state mirror of §9, optional), `apps/explorer` and `apps/dattool` (the cross-links of §10: "drops from", "sold by", "bartered for", "crafted by", "used in"); a later server or sandbox backend (`crates/net`) consumes §11 |
| **Companion specs** | [`property-system.md`](property-system.md) §3–§5 (type codes, value encodings, property sets), §6.2 (the +0x09000000 rule), §7 (enum and bitfield labels), §8 (DID maps); [`wstate.md`](wstate.md) §3–§5 (object-state records, the root class at offset 4, native codecs, in particular classes 11, 17, 25, 35, 39, 117, 166, 175, 176, 182, 199 and 1942 of §5.3–§5.4); [`wlib.md`](wlib.md) (class indices, ancestry, attribute lists); [`records.md`](records.md) §7–§8 (enum mappers, DID maps). This spec starts with decoded property sets and decoded object-state records. |

---

<a id="purpose"></a>
## Purpose

The game data ships the tables that a game server runs its economy from:

- **loot generation**: trophy lists, treasure group profiles with weighted item tables,
  treasure lists, weighted and filtered treasure tables, level-based tables per creature
  species, skirmish and world-event lookups, hobby (fishing) reward profiles, runic tables,
  and the control records that turn a drop-frequency label into a probability;
- **crafting**: recipes (ingredients, optional ingredients, result, critical result,
  profession, tier, experience reward), professions with their tiers, vocations, guilds,
  the control and directory records, and two DID maps;
- **vendors and barter**: barter profiles (cost items to reward), vendor inventories,
  the vendor properties of NPC records, perk vendors;
- **currency**: the denominations and their control record;
- **store** records (low priority, §8).

This spec gives an implementer:

1. the layout of every record class of the family, as property-set field tables with the
   presence, value range and DID targets found in **all** records of the install (§4–§8);
2. the object-state mirror that the same records carry in their 0x70 record, with the
   correspondence to the property form (§9), including two native codecs that `wstate.md`
   left partly open (the weighted table, class 175, and the barter entry, class 1942);
3. the DID graph between these records, items, creatures, NPCs and containers, and the
   algorithms that build the explorer's cross-links from it (§10);
4. what the client itself does with these records (§11.1, BIN), what the class library's
   code computes from them (vendor purchase price, recipe experience, cooldowns, the
   critical chance shown while crafting; §11.5–§11.7, WL), and a precise list of what only a
   server decides (§11.2–§11.6, `> **OPEN:**`).

Records are identified by DID and by **root class** (the class index at offset 4 of the
0x70 record, `wstate.md` §3.1). Fields are identified by their **property label** from the
master property dictionary (`property-system.md` §2). Labels are needed for
interoperability: the client and every tool look properties up by them, and the PIDs
behind them must be read from the dictionary at run time, never hard-coded. No game text
appears here.

---

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: decoded on the user's own install with throwaway probes in the dirty-room
  scratch directory, over **every** record of the class concerned. The probes used the
  project's own decoders read-only (property sets, object states, DID maps, enum mappers).
- **Reported**: comes only from published third-party material; not checked, or not
  checkable, against data.
- **BIN**: read from the client executable under method M4 (decision 0011).

| Key | Source |
|---|---|
| OBS | Observed in the user's install on 2026-10-06 and 2026-10-07 (Steam install, client data current on those dates). Every object record of `client_gamelogic.dat` had its root class read: 383,343 in all, of which 383,332 have prefix 0x70 and 11 are singletons with prefix 0x76 (`wstate.md` §2); all 383,343 object states decode. Every record of the classes in §2 had its property set (DID + 0x09000000) and its object state decoded completely. Inbound references and characteristic properties were counted over **all** property sets: the 383,332 object property sets (prefix 0x79, one per 0x70 record; the 0x76 singletons have none) and the 1,616 stand-alone 0x78 sets. Re-measured over all records on 2026-10-07 for the review revision. DID maps 0x28000032, 0x28000033 and 0x2800000E and the enum mappers named in §3.3 were decoded. |
| LC | LotroCompanion `lotro-tools` (no licence file; all rights reserved) and `lotro-core`: the loot, generic-mob-loot, custom-instance-loot, recipe, crafting, trade, hobby and perk loaders. Read for facts (which property means what) only. |
| DS | `research/notes/data-semantics.md` §4.7 and §5.2 (DID-map examples, root-class list). |
| BIN | **M4 (decision 0011).** Client `x64/lotroclient64.exe`, build 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5, the build of `wstate.md`), read in the dirty room on 2026-10-07 with a headless Ghidra copy of the shared project. **Questions:** (1) does the client itself read the loot, recipe, barter and vendor tables, and if so which properties; (2) what the item-examination code does with a recipe. **Method:** a search of the whole program for the PIDs of all 520 properties of this family as immediate operands, then reading the functions that held hits. **Why M1–M3 were not enough:** the data cannot show which properties the client consumes or how; no published source covers it; a running client (M3) shows only the result on screen, not which fields drive it. Only examination-tooltip and user-interface construction code was read; nothing from network, login, launcher or anti-tamper code. |
| OBS3 | Observed in the user's install on 2026-10-09 (OPEN sweep, spec writer), read-only; archives unchanged since 2026-10-04 (file times). A dump of **every** 0x70 record with its 0x79 set and of all 0x78 sets, made on 2026-10-07 with the repository's own decoders, walked at every nesting depth, plus the property dictionary for types and defaults. |
| OBS2 | Observed in the user's install on 2026-10-08 with fresh throwaway probes, each over **every** record concerned: all 1,007 class-3307 records (both forms); all 1,439 NPCs with `Vendor_InventoryList` and all 400 inventories; all 202 barter profiles with `Barter_ItemMungeLevelOverride`, their 2,336 reward structs and the reward structs of all other profiles; the CooldownControl record (both forms); all six class-3399 records; the keys of every `set_u64` (105) and `refmap_u64` (117) object of all 383,343 object records. |
| WL | The code of the class library (record 0x56000000, chunk 1), which is **data** in the user's install (method M1), disassembled in the dirty room on 2026-10-08 with the rules of [`wlib-bytecode.md`](wlib-bytecode.md) §2–§6: the method tables of every class of §2 and the bodies named in the fact rows below, located through the method tables (`wlib.md` §6.4) and through the property-ID immediates of `wlib-bytecode.md` §11.2 (each such immediate was checked by reading the call that consumes it; immediates that only coincide with a PID were discarded). "Class *c* function *s*" below means the body in slot *s* of class *c*'s method table. |
| BIN2 | **M4 (decision 0011).** Same build as BIN (SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-08 with a headless Ghidra copy of the shared project. **Question (3):** how the crafting panel turns the critical-chance values that the class library hands to it (WL, §11.5) into the chance it shows. **Method:** the native functions 289/31, 289/33 and 289/45 that the class library calls were resolved through the client's native-function registration; the fields they store were followed to the one panel function that reads them, and its two floating-point constants were read. **Why M1–M3 were not enough:** the data holds the inputs but not the rule that combines them; M3 shows only a rounded percentage for the few recipes and ingredients one character can use, which cannot separate addition from other rules or show a cap. Only crafting-panel user-interface code was read; nothing from network, login, launcher or anti-tamper code. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| C1 | Census of the family: classes, record counts, sizes (§2); characteristic properties (§3.4) | OBS | Confirmed |
| C2 | Every record of the family has a property set at DID + 0x09000000, and every one decodes (§2) | OBS | Confirmed (33,380 + 7,817 + 3,851 + 5 records) |
| L1 | Trophy-list layout (§4.1) | OBS; LC (meaning of the fields) | Confirmed (layout); Reported (meaning) |
| L2 | Treasure-group-profile, treasure-list, weighted-table layouts (§4.2–§4.4) | OBS; LC | Confirmed (layout); Reported (meaning) |
| L3 | Filtered-treasure-list and entity-filter layout (§4.5) | OBS; LC | Confirmed (layout); Reported (filter meaning) |
| L4 | Level, species, PVP, world-event, runic, skirmish and hobby tables (§4.6–§4.11) | OBS; LC | Confirmed (layout); Reported (meaning) |
| L5 | Loot control singletons and drop-frequency resolution (§4.12) | OBS; LC | Confirmed (layout, coverage); Reported (probability meaning) |
| L6 | Where loot tables are attached (§4.13) | OBS | Confirmed |
| R1 | Recipe layout, the two recipe variants and their co-occurrence rules (§5.1) | OBS; LC | Confirmed (layout); Reported (meaning) |
| R2 | Profession, vocation, guild, control and directory layouts; VOCATIONS and PROFESSIONS DID maps (§5.2–§5.5) | OBS; LC | Confirmed |
| V1 | Barter layout and list shapes (§6.1) | OBS; LC | Confirmed |
| V2 | Look-up-table quantities: the object form stores the progression value at the reward item's level (§6.2) | OBS | Confirmed (5,565 of 5,565). Derived by this spec writer. |
| V3 | Vendor inventory and NPC vendor properties; Vendor_ItemList is the union of the inventories (§6.3) | OBS; LC | Confirmed (1,439 of 1,439 NPCs) |
| V4 | Perk vendor, currency and store records (§6.4, §7, §8) | OBS; LC | Confirmed (layout) |
| W1 | Object-state mirror of each class (§9.1–§9.4) | OBS | Confirmed (all records of each class). Derived by this spec writer. |
| W2 | The third word of a weighted-table element (class 175) is the subtree sum of an implicit binary heap; the first word is the element's index (§9.2) | OBS | Confirmed (8,131 of 8,131 tables for the sum; for the index, 7,441 of 7,441 tables of the classes of this spec and 7,625 of the 8,131 tables in all). Derived by this spec writer. |
| W3 | Field meanings of the 49-byte barter entry (class 1942) (§9.3) | OBS | Confirmed (42,214 of 42,214). Derived by this spec writer. |
| W4 | Key packing of the species and PVP level maps (class 117) (§9.5) | OBS | Confirmed (264 of 264 and 15 of 15) |
| G1 | DID graph and acyclicity of the loot graph (§10) | OBS | Confirmed |
| B1 | Examination tooltip, compare type 4: the client reads the recipe's result item and result quantity (§11.1) | BIN | BIN only |
| B2 | No client code was found that reads the loot, barter or vendor tables as data (§11.1) | BIN (negative search) | BIN only; see the caveat in §11.1 |
| W5 | The order of the class-3308 array of a weighted treasure table is the removal order of a binary min-heap filled in property order (§9.4) | OBS2 | Confirmed (1,007 of 1,007). Derived by this spec writer. |
| V5 | The order of `Vendor_ItemList` is the iteration order of a growing hash set filled in first-occurrence order (§6.3) | OBS2 | Confirmed (1,439 of 1,439). Derived by this spec writer. |
| V6 | Purchase price computed by the class library's vendor code (§11.6) | WL | Confirmed in the class library (all its code paths read); not compared with prices in the running client |
| R3 | Recipe cooldown keys resolve through the CooldownControl duration map (§11.5) | OBS2; WL | Confirmed (one record; its 63 object-form entries equal its 63 property entries; the 3 keys used by recipes are present). Unit: derived (see §11.5) |
| R4 | The experience value of a recipe as the class library computes it (§11.5) | WL; OBS (keys covered, §5.4) | Confirmed in the class library |
| R5 | The critical-success chance the crafting panel shows (§11.5) | WL (inputs); BIN2 (combination, clamp, rounding) | Inputs Confirmed in the class library; combination BIN only |
| L7 | The class library declares the economy classes' own functions but ships no body for the loot-table, barter-cost and money functions (§11.7) | WL | Confirmed (every class of §2) |
| L8 | Class 3399 is a set of DIDs; the record named by `LootGenControl_RewardOnceTGPs` is empty in both forms (§14) | OBS2; WL | Confirmed (6 of 6 records) |
| W6 | The class library's four-field 64-bit key packing (field widths and the bits 0–3 field) (§9.5) | WL; OBS2 (low-nibble census) | Confirmed in the class library; its use for the loot maps is not shown (§14) |
| V7 | Barter profiles with `Barter_ItemMungeLevelOverride` mostly reward scaling items (§6.2) | OBS2 | Confirmed as a count; meaning OPEN |
| E1 | `WorldEvent_ConditionValue` is an int (type 15, dictionary default −1) wherever it occurs; `WorldEvent_Operator` an enum (type 8) of mapper 0x23000039; observed condition values over every set: most often 0 (10,289), 1 (4,364), 2 (430) and 3 (385), in all −24 … 70,000 (102 distinct values; the loot holders of §4.5 and §4.8 use 0 … 3 only) (§4.5, §4.8) | OBS3 | Confirmed 2026-10-09 |
| E2 | Every one of the 1,200 runic level-table entries stores `RunicLevelLootTable_Level` (type 15, dictionary default 1), equal to its position + 1 (§4.9) | OBS3 | Confirmed 2026-10-09 |
| E3 | No property set holds `Usage_MinLevel` both at top level and inside `DefaultPermissionBlobStruct` (105,841 top level only, 5,167 inside the struct only) (§6.2) | OBS3 (every 0x79 set) | Confirmed 2026-10-09 |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Object record** | A record with DID prefix 0x70 (`wstate.md` §2). Its root class (offset 4) says which kind of economy record it is. Every record of this spec has prefix 0x70; none of the 11 singletons with prefix 0x76 has a root class of this spec. |
| **Property form** | The record's property set at DID + 0x09000000 (prefix 0x79, `property-system.md` §6.2). It is complete for every class of this spec and is the form this spec treats as authoritative. |
| **Object form** | The script-object graph in the 0x70 record's bundle (`wstate.md` §4). For most classes it mirrors part of the property form in a compiled shape (§9); for some it is empty. |
| **Item record** | An object record whose root class belongs to the item family. In this install the economy tables point at items of root classes 795, 796, 797, 798, 799, 802, 803, 805, 2814 and 4178 (the item spec owns their meaning). Recipes also name records of root class 794 as results (79 result items, 78 critical results, 26 multi-output DIDs), the class that also holds the loot properties of containers (§4.13); a cross-link treats them like items. |
| **Entry** | One element of an array property, usually a struct. Entries are kept in stored order (`property-system.md` §4.11). |
| **Weight** | A non-negative integer that sets the relative chance of an entry in a pick-one table. |
| **Drop frequency** | An enum key (of the mapper named by the definition of `LootGen_TrophyList_DropFrequency`) that a control table turns into a probability (§4.12). |
| **Absent field** | A struct field or top-level property that is not stored. Its value is the **dictionary default** of the property (`property-system.md` §2.2) when the definition has one; this spec lists the defaults that matter. |

Class indices, PIDs and DIDs quoted below are observations of this install. Implementations
identify a record's kind by its characteristic property (§3.4) or its root class, find the
singletons by the WEENIECONTENT labels of §4.12, §5.4 and §7, read the PIDs from the
dictionary by label, and never hard-code DIDs.

---

<a id="s2"></a>
## 2. Census (Confirmed, all records)

"0x70 bytes" and "0x79 bytes" are the total decompressed sizes of the object records and
of their property sets. All records of every class below have a property set, and all of
them decode with no leftover bytes.

<a id="s2-1"></a>
### 2.1 Loot (20 classes, 33,380 records)

| Root class | Records | 0x70 bytes | 0x79 bytes | Role (§) |
|---|---|---|---|---|
| 1237 | 15,224 | 3,964,104 | 3,463,141 | Trophy list (§4.1) |
| 2728 | 8,326 | 299,736 | 649,428 | Skirmish treasure lookup (§4.10) |
| 1235 | 6,418 | 4,609,876 | 3,802,320 | Treasure group profile (§4.2) |
| 3307 | 1,007 | 248,090 | 148,176 | Weighted treasure table (§4.4) |
| 3666 | 927 | 1,910,959 | 1,010,286 | Filtered treasure list (§4.5) |
| 1236 | 894 | 32,184 | 85,062 | Treasure list (§4.3) |
| 1917 | 179 | 30,576 | 18,825 | Barter trophy list, a subclass of 1237 (§4.1) |
| 2153 | 127 | 117,422 | 210,504 | Hobby reward profile (§4.11) |
| 3669 | 121 | 169,608 | 815,298 | Level-based loot table (§4.6) |
| 1918 | 47 | 8,250 | 4,476 | Reputation trophy list, a subclass of 1237 (§4.1) |
| 2239 | 34 | 1,224 | 18,600 | Runic treasure group (§4.9) |
| 2001 | 23 | 7,405 | 14,670 | Species level loot table (§4.7) |
| 2372 | 16 | 3,572 | 2,028 | Treasure group profile, second class (§4.2) |
| 2227 | 15 | 19,170 | 50,670 | Runic level loot table (§4.9) |
| 2127 | 11 | 10,419 | 52,998 | World-event level loot table (§4.8) |
| 2729 | 7 | 252 | 66,734 | Skirmish custom loot lookup table (§4.10) |
| 1673 | 1 | 2,068 | 7,332 | Loot control (§4.12) |
| 1674 | 1 | 3,287 | 62 | Loot directory control (§4.12) |
| 2128 | 1 | 847 | 852 | World-event loot table directory (§4.8) |
| 3671 | 1 | 461 | 648 | PVP level loot table (§4.7) |

Class 2229 (169 records, runic list, §4.9) belongs to the item-advancement family by its
properties but is reached only through runic loot tables; it is documented here and not
counted above.

<a id="s2-2"></a>
### 2.2 Crafting (6 classes, 7,817 records)

| Root class | Records | 0x70 bytes | 0x79 bytes | Role (§) |
|---|---|---|---|---|
| 1024 | 7,791 | 2,596,076 | 3,947,253 | Recipe (§5.1) |
| 1023 | 10 | 28,902 | 41,934 | Profession (§5.2) |
| 1026 | 7 | 917 | 1,162 | Vocation (§5.3) |
| 2278 | 7 | 252 | 518 | Craft guild (§5.3) |
| 1665 | 1 | 687 | 1,512 | Craft control (§5.4) |
| 1021 | 1 | 661 | 234 | Craft directory (§5.4) |

<a id="s2-3"></a>
### 2.3 Vendors, currency and store

| Root class | Records | 0x70 bytes | 0x79 bytes | Role (§) |
|---|---|---|---|---|
| 1943 | 3,449 | 3,479,120 | 3,864,370 | Barter profile (§6.1) |
| 1619 | 400 | 79,656 | 119,228 | Vendor inventory (§6.3) |
| 1796 | 2 | 406 | 612 | Perk vendor (§6.4) |
| 1028 | 3 | 441 | 270 | Currency denomination (§7) |
| 1029 | 1 | 163 | 42 | Currency directory (§7) |
| 1996 | 1 | 36 | 30 | Currency control (§7) |
| 2541, 2532, 3054, 3052 | 589, 477, 429, 270 | | | Store items (§8) |
| 2534, 3109, 1455, 1456, 1459, 1460 | 1 each | | | Store directory and store helpers (§8) |

A family census taken before this spec counted 1,903 "store" records; that figure
included the 10 profession records (class 1023, which carry two store properties) and two
classes that are not store records (1930, monster-play sessions, 95; 4224, reward tracks,
27).

---

<a id="s3"></a>
## 3. Conventions

<a id="s3-1"></a>
### 3.1 Field tables

Each record class is given as a table of property paths. A path names a top-level property;
`A[]` means "each element of array A"; `S/F` means "field F of struct S". Columns:

- **Type**: the property type code (`property-system.md` §3): 8 enum, 9 float, 10 PID,
  11 struct, 12 array, 13 StringInfo, 15 int, 18 32-bit bitfield, 20 DID, 21 boolean.
- **Present**: in how many records (top level) or how many entries (inside arrays) the
  field is stored, out of all.
- **Observed**: value range and notable distributions; for DIDs, the root classes of the
  targets with counts. "0" counts DID values of 0 (none).
- **Default**: the dictionary default when the field is absent (only where one exists and
  matters).

The byte encoding of every value is that of `property-system.md` §4; nothing in this
family needs another encoding. Field order inside a struct is the hashed order of
`property-system.md` §4.10 and carries no meaning; element order inside an array is the
stored order and is kept.

<a id="s3-2"></a>
### 3.2 DID targets

Every DID-valued field of this family is declared with the game-object type code 0x45
(`property-system.md` §9), except the icon fields noted as render surfaces (code 0x05).
Every non-zero 0x45 value of every field in this spec points at an existing 0x70 record
(no dangling target in any record). The 26 icon references point at existing 0x41
surfaces.

<a id="s3-3"></a>
### 3.3 Enum sources

Enum fields name their mapper in the property definition (`property-system.md` §7.1).
Observed mappers, with their EMAPPER label where it exists (labels are read at run time):

| Field(s) | Mapper DID | Names / display texts | Label |
|---|---|---|---|
| `LootGen_TrophyList_DropFrequency`, `LootGenControl_DropFrequency_Label`, `RunicList_DropFrequency` | 0x23000060 | 57 / 0 (keys 0–61) | |
| `LootGenControl_MoneyDropModifier_Label` | 0x23000070 | 35 / 34 (keys 0–48) | |
| `CraftRecipe_Tier`, `CraftProfession_Tier`, `CraftGuild_RequiredCraftTier` | 0x23000063 | 18 / 16 (keys 0–17) | CraftTier |
| `CraftRecipe_UICategory` | 0x23000065 | 517 / 516 (keys 0–716) | CraftUICategory |
| `CraftRecipe_XPReward`, `CraftControl_XPRewardEnum` | 0x23000145 | 16 / 0 (keys 0–16) | CraftXPReward |
| `CraftProfession_RequiredTool` (bitfield) | 0x23000062 | 14 / 13 (keys 0–16) | CraftToolCategory |
| `Barter_ReputationRewardTier` | 0x2300026F | 24 / 0 | BarterReputationRewardTier |
| `CraftRecipe_CooldownDuration` | 0x2300006D | 64 / 0 | |
| `CraftRecipe_Animation` | 0x23000097 | 11,373 / 0 (keys are large values 0x10000002–0x10003645) | |
| `Barter_SoundCategory` | 0x2300013C | 67 / 0 | |
| `CraftProfession_ProficiencyTutorialHint` | 0x23000127 | 383 / 0 | |
| `CraftProfession_RecipeWebStoreCategory` | 0x230002E2 | 85 / 0 | |
| `HobbyRewardProfile_TimeAvailable` | 0x23000498 | 3 / 0 | |
| `CraftGuild_QuestCategory` | 0x23000019 | 478 / 477 | |

<a id="s3-4"></a>
### 3.4 Identifying a record's kind (Confirmed)

Each kind has a **characteristic property**: a top-level property that occurs in the
property sets of that kind and of no other root class. Checked over every property set of
the install: the 383,332 object property sets of the 0x70 records and the 1,616 stand-alone
0x78 sets (none of which holds any of these properties); the 11 singletons with prefix 0x76
have no property set.
A reader that does not want to depend on class indices may classify by it.

| Characteristic property | Root classes holding it (records) |
|---|---|
| `LootGen_TrophyList` | 1237 (15,224), 1917 (179), 1918 (47) |
| `TreasureGroupProfile_ItemTable` | 1235 (6,418), 2372 (16) |
| `LootGen_TreasureList` | 1236 (894) |
| `LootGen_WeightedTreasureTable` | 3307 (1,007) |
| `LootGen_FilteredTreasureListArray` | 3666 (927) |
| `LevelBasedLootTable_LootTable` | 3669 (121) |
| `SpeciesLevelLootTable` | 2001 (23) |
| `PVPLevelLootTable` | 3671 (1) |
| `WorldEventLevelLootTable_LootTable` / `WorldEventLootTableDirectory_Array` | 2127 (11) / 2128 (1) |
| `RunicLevelLootTable_LootTable` / `RunicList_Array` / `RunicTreasureGroup_Array` | 2227 (15) / 2229 (169) / 2239 (34) |
| `SkirmishTreasureLookup_TrophyListTemplateDID` / `SkirmishCustomLootLookupTable_Array` | 2728 (8,326) / 2729 (7) |
| `HobbyRewardProfile_ItemArray` | 2153 (127) |
| `LootGenControl_DropFrequencyTable` / `LootGenControl_SpeciesLevelLootTableArray` | 1673 (1) / 1674 (1) |
| `CraftRecipe_ResultItem` | 1024 (7,791) |
| `CraftProfession_TierArray` / `CraftVocation_ProfessionArray` / `CraftGuild_Profession` | 1023 (10) / 1026 (7) / 2278 (7) |
| `CraftControl_XPRewardArray` / `CraftDirectory_ProfessionArray` | 1665 (1) / 1021 (1) |
| `Barter_ItemListArray` | 1943 (3,449) |
| `VendorInventory_Items` | 1619 (393; the 7 empty inventories have no property and are known only by class) |
| `PerkVendor_InventoryList` | 1796 (2) |
| `Currency_Value` / `Currency_CurrencyList` / `Currency_Control_GarnishmentPercentage` | 1028 (3) / 1029 (1) / 1996 (1) |

To tell 1917 and 1918 from 1237, or 2372 from 1235, use the root class or the property
that references the list (§4.13).

---

<a id="s4"></a>
## 4. Loot tables

<a id="s4-1"></a>
### 4.1 Trophy list (classes 1237, 1917, 1918)

A trophy list is a list of independent drop candidates. Classes 1917 and 1918 are
subclasses of 1237 (their ancestry starts 1237 > 486, `wlib.md`) with the same layout; they
are the targets of the barter and reputation trophy-list properties (§4.13).

| Path | Type | Present | Observed | Default |
|---|---|---|---|---|
| `LootGen_TrophyList` | 12 | all (15,224 / 179 / 47 records) | 1–34 entries (1237), 1–12 (1917), 1–6 (1918); 77,188 / 436 / 121 entries | |
| `LootGen_TrophyList[]` element `LootGen_TrophyList_Entry` | 11 | all entries | | |
| `…_Entry/LootGen_TrophyList_ItemOrProfile` | 20 | all entries | 1237: items 55,074 (796: 41,762; 795: 5,658; 802: 3,088; 2814: 2,173; 799: 1,727; 797: 299; 805: 273; 4178: 62; 798: 20; 803: 12), treasure group profiles 1235: 14,546, treasure lists 1236: 7,568. 1917: items 796 (415), profiles 2372 (21). 1918: items 796 (121). Never 0. | 0 |
| `…_Entry/LootGen_TrophyList_DropFrequency` | 8 | all entries | 1237: 44 distinct keys in 2–61; 1917: 18 distinct in 2–56; 1918: 12 distinct in 2–59 | 0 |
| `…_Entry/LootGen_TrophyList_ItemQuantity` | 15 | 72,004 of 77,188 (1237); 121 of 436 (1917); none (1918) | 1–10,000; 1 in 69,237 of the 72,004 | **1** |
| `…_Entry/LootGen_TrophyList_GroupDrop` | 21 | 937 entries in 577 records (1237); 119 in 52 (1917) | true 170 + 73 | false |
| `Description` | 13 | 26 records (1237 only) | | |
| `Icon_Layer_ImageOverrideDID` | 20 (surface) | the same 26 records | 0x41 surfaces | |
| `Inventory_MaxQuantity` | 15 | 13 records (1237 only) | 1 | |

Meaning (Reported, LC): each entry is a separate candidate that drops with the probability
its drop-frequency key has in the loot control table (§4.12). The target is either an item,
given `ItemQuantity` times, or a treasure group profile or treasure list, from which one
item is then picked (§4.2, §4.3). `GroupDrop` marks entries that are generated for the
group rather than per looter. The roll itself is server behaviour (§11.2).

<a id="s4-2"></a>
### 4.2 Treasure group profile (classes 1235, 2372)

A pick-one table of items with weights.

| Path | Type | Present | Observed | Default |
|---|---|---|---|---|
| `TreasureGroupProfile_ItemTable` | 12 | all (6,418 / 16) | 1–1,967 entries (1235); 2–7 (2372); 116,706 / 58 entries | |
| `…[]` element `TreasureGroupProfile_ItemTable_Entry` | 11 | all entries | | |
| `…_Entry/TreasureGroupProfile_ItemTable_Item` | 20 | all entries | 1235: 795: 35,653; 802: 28,448; 796: 25,101; 799: 18,780; 2814: 3,433; 805: 2,742; 797: 2,496; 798: 50; 803: 3. 2372: 796: 51, 805: 7. Never 0. | 0 |
| `…_Entry/TreasureGroupProfile_ItemTable_Weight` | 15 | all entries | 1–470,000 (1235); 1–50 (2372). Never 0. | 0 |
| `…_Entry/TreasureGroupProfile_ItemTable_Quantity` | 15 | 15,468 entries in 990 records (1235 only) | 1–6,000 | **1** |

Meaning (Reported, LC): one entry is chosen with probability weight ÷ (sum of weights);
the chosen item is given `Quantity` times.

<a id="s4-3"></a>
### 4.3 Treasure list (class 1236)

A pick-one table of treasure group profiles.

| Path | Type | Present | Observed | Default |
|---|---|---|---|---|
| `LootGen_TreasureList` | 12 | all 894 | 1–11 entries; 2,299 entries | |
| `…[]` element `LootGen_TreasureList_Entry` | 11 | all | | |
| `…_Entry/LootGen_TreasureList_GroupProfile` | 20 | all | 1235 in all 2,299 | 0 |
| `…_Entry/LootGen_TreasureList_Weight` | 15 | all | 1–2,000 | 0 |

The object form of this class is empty (§9.1).

<a id="s4-4"></a>
### 4.4 Weighted treasure table (class 3307)

A pick-one table of trophy lists.

| Path | Type | Present | Observed | Default |
|---|---|---|---|---|
| `LootGen_WeightedTreasureTable` | 12 | all 1,007 | 1–20 entries; 4,335 entries | |
| `…[]` element `LootGen_WeightedTreasureTable_Entry` | 11 | all | | |
| `…_Entry/LootGen_WeightedTreasureTable_TrophyList` | 20 | all | 1237 in all 4,335 | 0 |
| `…_Entry/LootGen_WeightedTreasureTable_TrophyList_Weight` | 15 | all | 1–300 | 0 |

<a id="s4-5"></a>
### 4.5 Filtered treasure list (class 3666) and entity filters

An ordered list of (filter, table) entries.

| Path | Type | Present | Observed |
|---|---|---|---|
| `LootGen_FilteredTreasureListArray` | 12 | all 927 | 1–179 entries; 11,618 entries |
| `…[]` element `LootGen_FilteredTreasureListArray_Entry` | 11 | all | |
| `…_Entry/LootGen_FilteredTrophyTable_EntryDID` | 20 | all | trophy lists 1237: 10,302; weighted tables 3307: 1,316. Never 0. |
| `…_Entry/EntityFilter_Array` | 12 | all | 1–5 filters; elements of three kinds, below |

Filter kinds (elements of `EntityFilter_Array`):

| Element | Type | Count | Content |
|---|---|---|---|
| `EntityFilter_PropertySet` | 12 | 18,525 (7,356 of them empty) | A list of property values; every element carries the PID of the property it tests and an allowed value. Observed tested properties: `Agent_Class` (17,051 values, 19 distinct class keys), `Agent_Species` (18), `ze_skirmish_difficulty` (104), `WE_Player_Level_Cap` (108), `WE_Lootbox_bonus_embers_motes` (8). All elements of one non-empty filter carry the same PID (11,169 of 11,169). |
| `EntityFilter_PropertyRange` | 11 | 7,426 | Struct of `EntityFilter_PropertyRange_Min` and `EntityFilter_PropertyRange_Max`, each a one-element array whose element carries the tested property and the bound. Tested: `Advancement_Level` (5,070), `ze_skirmish_level` (2,284), `ze_skirmish_difficulty` (72). Minimum and maximum name the same property and min ≤ max in all 7,426. Maxima of 999 and 1,000 occur, above every level the data uses. |
| `EntityFilter_WorldEvent` | 11 | 326 | Struct holding one `WorldEvent_Condition` (fields `WorldEvent_WorldEvent` (type 20) → world-event record of class 1815 or 1887, `WorldEvent_Operator` (type 8, mapper 0x23000039) key 3 in all, `WorldEvent_ConditionValue` (type 15, int, dictionary default −1) 0 or 1; E1). |

The definition of `EntityFilter_Array` allows more element kinds (weenie type, player-pet
flag, alignment, class); none occurs in these records.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The type codes of the `WorldEvent_Condition` fields are
> not given here or in §4.8: `WorldEvent_Operator` is described as a key (read as an enum,
> type 8), but the type of `WorldEvent_ConditionValue` (values 0–3) is not stated. Which type
> code does it have? Until answered, `crates/formats` reads it from an int, an enum or a
> boolean.
>
> *Answer (E1):* type 15 (int; dictionary default −1) wherever the label occurs; every stored
> value is an int. The tables of §4.5 and §4.8 now give the type codes of all three fields.

Meaning (Reported, LC): an entry applies to a looter for whom every filter of the entry
holds; a property-set filter holds when the looter's value of the tested property equals
one of the listed values; a range filter holds when the value lies between the bounds; a
world-event filter holds when the condition holds. See §11.4 for what stays open.

<a id="s4-6"></a>
### 4.6 Level-based loot table (class 3669)

| Path | Type | Present | Observed | Default |
|---|---|---|---|---|
| `LevelBasedLootTable_LootTable` | 12 | all 121 | exactly 160 entries in every record (19,360) | |
| `…[]` element `LevelBasedLootTable_LootTable_Entry` | 11 | all | | |
| `…_Entry/LevelBasedLootTable_Level` | 15 | all | entry *i* (from 0) has level *i* + 1 in all 121 records | 1 |
| `…_Entry/LevelBasedLootTable_TrophyList` | 20 | all | 1237: 16,185; 0: 3,175 | 0 |
| `…_Entry/LevelBasedLootTable_TreasureProfile` | 20 | all | treasure lists 1236: 3,825; 0: 15,535 | 0 |

Meaning (Reported, LC): for a creature of level *L* the entry with level *L* gives the trophy
list and the treasure list that apply; 0 means none.

<a id="s4-7"></a>
### 4.7 Species level loot table (class 2001) and PVP level loot table (class 3671)

| Path | Type | Present | Observed |
|---|---|---|---|
| `SpeciesLevelLootTable` (class 2001) | 12 | all 23 | 2–59 entries; 264 entries |
| `…[]` element `SpeciesLevelLootTable_Entry` | 11 | all | |
| `…_Entry/Agent_Species` | 8 | all | 74 distinct keys in 1–123 |
| `…_Entry/Agent_Subspecies` | 8 | all | 0 in 261; three other keys once each |
| `…_Entry/MonsterLevel_ExaminationModStatType` | 8 | all | 0: 190, 7: 52, 4: 21, 1: 1 |
| `…_Entry/LevelBasedLootTable_LevelTable` | 20 | all | 3669 in all 264 |
| `PVPLevelLootTable` (class 3671) | 12 | 1 record | 15 entries |
| `…_Entry/Agent_Species`, `…/Agent_Class`, `…/LevelBasedLootTable_LevelTable` | 8, 8, 20 | all | 15 distinct species; class 0 in all; 3669 in all |

Meaning (Reported, LC): the table chooses the level-based loot table of a creature from its
species, subspecies and examination type (the creature's difficulty category). Within one
record the (species, subspecies, examination type) triples are unique (the object form keys
them, §9.5).

<a id="s4-8"></a>
### 4.8 World-event level loot tables (classes 2127, 2128)

| Path | Type | Present | Observed |
|---|---|---|---|
| `WorldEventLevelLootTable_LootTable` (class 2127) | 12 | all 11 | 160 entries each (1,760) |
| `…_Entry/WorldEventLevelLootTable_Level` | 15 | all | entry *i* (from 0) has level *i* + 1 in all 11 |
| `…_Entry/WorldEventLevelLootTable_TrophyListTemplate` | 20 | all | 1237: 1,063; 1917: 160; 0: 537 |
| `WorldEventLootTableDirectory_Array` (class 2128) | 12 | 1 record | 11 entries |
| `…_Entry/WorldEventLootTableDirectory_LevelTable` | 20 | all | 2127 in all 11 (each of the 11 tables once) |
| `…_Entry/WorldEvent_AllConditionList[]/WorldEvent_Condition` | 11 | 12 conditions | `WorldEvent_WorldEvent` (20) → 1887 (9) or 1815 (3); `WorldEvent_Operator` (8) key 3; `WorldEvent_ConditionValue` (15) 1–3 |

The directory is the WEENIECONTENT label `WorldEventLootTableDirectory` (§4.12). Meaning
(Reported): the first directory entry whose conditions all hold selects a level table.

<a id="s4-9"></a>
### 4.9 Runic loot (classes 2227, 2229, 2239)

| Path | Type | Present | Observed | Default |
|---|---|---|---|---|
| `RunicLevelLootTable_LootTable` (2227) | 12 | all 15 | 80 entries each (1,200) | |
| `…_Entry/RunicLevelLootTable_Level` | 15 | all | entry *i* (from 0) stores level *i* + 1 in all 1,200 (E2) | 1 |
| `…_Entry/RunicLevelLootTable_TrophyList` | 20 | all | 1237 in all 1,200 | 0 |
| `…_Entry/RunicLevelLootTable_RunicTrophyList` | 20 | all | runic lists 2229: 1,050; 0: 150 | 0 |
| `RunicList_Array` (2229) | 12 | all 169 | 1–11 entries; 842 | |
| `…_Entry/RunicList_RunicOrTreasureGroup` | 20 | all | relics 2219: 469; runic treasure groups 2239: 373 | 0 |
| `…_Entry/RunicList_DropFrequency` | 8 | all | 7 distinct keys in 2–23 (same mapper as §4.1) | 0 |
| `RunicTreasureGroup_Array` (2239) | 12 | all 34 | 10–70 entries; 450 | |
| `…_Entry/RunicTreasureGroup_Runic` | 20 | all | relics 2219 | |
| `…_Entry/RunicTreasureGroup_Weight` | 15 | all | 1–5 | 0 |
| `RunicTreasureGroup_PullCount_Array` (2239) | 12 | all 34 | exactly 4 entries each | |
| `…_Entry/RunicTreasureGroup_PullCount` | 15 | all | the four entries hold counts 1, 2, 3, 4 in that order in all 34 | 0 |
| `…_Entry/RunicTreasureGroup_Weight` | 15 | all | 0–100 (0 in 61 of 136) | 0 |

Meaning (Reported, LC): a runic list is a trophy list for relics; a runic treasure group
first picks a pull count by weight, then picks that many relics by weight.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The table above names no level field for the entries of
> `RunicLevelLootTable_LootTable`, unlike §4.6 (`LevelBasedLootTable_Level`) and §4.8
> (`WorldEventLevelLootTable_Level`). Is a level stored in each entry, and under which label?
> Until answered, `crates/formats` derives the level from the position (entry *i* has level
> *i* + 1) and checks E-I5 for this table by entry count only.
>
> *Answer (E2):* yes, `RunicLevelLootTable_Level` (type 15, dictionary default 1) in every
> entry, equal to the position + 1 in all 1,200; the table above now lists it, and E-I5 can be
> checked against the stored level.

<a id="s4-10"></a>
### 4.10 Skirmish lookups (classes 2728, 2729)

| Path | Type | Present | Observed |
|---|---|---|---|
| `SkirmishTreasureLookup_EncounterDID` (2728) | 20 | all 8,326 | private-encounter templates of class 2651 |
| `SkirmishTreasureLookup_SkirmishMatchParametersDID` | 20 | all | match-parameter records of class 2727 |
| `SkirmishTreasureLookup_TrophyListTemplateDID` | 20 | all | 1237 in all |
| `SkirmishTreasureLookup_TreasureListTemplateDID` | 20 | all | 0 in all |
| `SkirmishTreasureLookup_BarterTrophyListDID` | 20 | all | 0 in all |
| `SkirmishTreasureLookup_ReputationTrophyListDID` | 20 | all | 0 in all |
| `SkirmishCustomLootLookupTable_Array` (2729) | 12 | all 7 | 620 entries (5 records), 2,607 and 2,619; 8,326 |
| `…[]` element `SkirmishCustomLootLookupTable_Entry` | 20 | all | 2728; every 2728 record is listed exactly once over the 7 tables |

The match-parameter record (class 2727) carries `SkirmishMatchParams_Level_Min/Max`,
`_GroupSize_Min/Max` and `_DifficultyTier_Min/Max` (Reported, LC; their layout belongs to
the instance spec). Meaning (Reported): for a skirmish of a given encounter, level, group
size and difficulty, the lookup whose parameters match supplies the trophy list.

<a id="s4-11"></a>
### 4.11 Hobby reward profile (class 2153)

| Path | Type | Present | Observed |
|---|---|---|---|
| `HobbyRewardProfile_AdvancementProgression` | 20 | all 127 | progression records of class 1385 |
| `HobbyRewardProfile_ItemArray` | 12 | all | 1–74 entries; 2,689 |
| `…[]` element `HobbyRewardProfile_ItemStruct` | 11 | all | |
| `…/HobbyRewardProfile_Item` | 20 | all | 796: 2,688; 2814: 1 |
| `…/HobbyRewardProfile_Weight` | 15 | all | 1–100 |
| `…/HobbyRewardProfile_MinProficiency` | 15 | all | 1–175 |
| `…/HobbyRewardProfile_MaxProficiency` | 15 | all | 250 or 400 |
| `…/HobbyRewardProfile_MetaScore` | 15 | 2,562 entries in 75 records | 0–3 |
| `…/HobbyRewardProfile_TimeAvailable` | 8 | the same 2,562 | key 0 in all |

The dictionary gives these fields no default. Meaning (Reported, LC): an entry is eligible
while the player's hobby proficiency lies in [min, max]; one eligible entry is picked by
weight.

<a id="s4-12"></a>
### 4.12 Loot control singletons and drop-frequency resolution

Both are reached through WEENIECONTENT labels (DID map 0x2800000E,
`property-system.md` §8.1): **LootGenControl** (class 1673) and **LootGenDirectoryControl**
(class 1674). The label **LootGenControl_RewardOnceTGPs** names a record of class 3399 with
an empty property set in this install; its object form holds one empty `set_u32` (§14).

| Path (class 1673) | Type | Observed |
|---|---|---|
| `LootGenControl_DropFrequencyTable` | 12 | 56 entries |
| `…_Entry/LootGenControl_DropFrequency_Label` | 8 | 56 distinct keys in 2–61 (mapper 0x23000060) |
| `…_Entry/LootGenControl_DropFrequency_Percentage` | 9 | 0.0001–1.0; a **fraction**, not a percentage, despite the label |
| `LootGenControl_MoneyDropTable` | 12 | 160 entries |
| `…_Entry/LootGenControl_MoneyDropTable_DropAmountMaximum` | 15 | 10–10,590, non-decreasing over the 160 entries |
| `…_Entry/LootGenControl_MoneyDropTable_DropAmountVariance` | 15 | 5–2,120 |
| `LootGenControl_MoneyDropModifierTable` | 12 | 27 entries |
| `…_Entry/LootGenControl_MoneyDropModifier_Label` | 8 | 27 distinct keys in 1–40 (mapper 0x23000070) |
| `…_Entry/LootGenControl_MoneyDropModifier_Multiplier` | 9 | 0.8–10.0 |

| Path (class 1674) | Type | Observed |
|---|---|---|
| `LootGenControl_SpeciesLevelLootTableArray` | 12 | 4 elements `LootGenControl_SpeciesLevelLootTable` → class 2001 |
| `LootGenControl_PVPLevelLootTable` | 20 | the one class-3671 record |

**Drop-frequency resolution.** To turn the drop-frequency key *k* of a trophy-list or
runic-list entry into a probability, find the entry of `LootGenControl_DropFrequencyTable`
whose label is *k* and take its fraction. Confirmed: all 47 keys used by trophy and runic
lists have an entry (no key is missing). The fractions are data: read them at run time.

<a id="s4-13"></a>
### 4.13 Where loot tables are attached (Confirmed)

Properties of other records that point into the loot family. Counts are non-zero
references over all property sets (the 383,332 object property sets and the 1,616 0x78
sets; no 0x78 set holds any of them). The figure after each holder class is the number of
references from that property in records of that class; the References column is the row
total.

| Target class | Referencing property (root class of the holder) | References |
|---|---|---|
| 1237 trophy list | `LootGen_TrophyList_Override` (mobs 1723: 1,949; containers 794: 1,192); `SoftLock_LootGen_TrophyList_Override` (794: 108); `PackageItem_TrophyListTemplate` (package items 2814: 3,390); `Item_Disenchant_Trophy_List` (items 795: 600, 802: 246, 799: 126, 797: 16, 796: 4); `Runic_Deconstruction_TrophyListTemplate` (relics 2219: 706); `RunicCombinationControl_Success_TrophyList` (2301: 9); the loot tables of §4.4–§4.10: weighted tables (3307: 4,335), filtered lists (3666: 10,302), level tables (3669: 16,185), world-event level tables (2127: 1,063), runic level tables (2227: 1,200), skirmish lookups (2728: 8,326) | 49,757 |
| 1917 barter trophy list | `LootGen_BarterTrophyList` (1723: 333; 794: 107); `SoftLock_LootGen_BarterTrophyList` (794: 2); `PackageItem_TrophyListTemplate` (2814: 1); world-event tables (160) | 603 |
| 1918 reputation trophy list | `LootGen_ReputationTrophyList` (1723: 1,088; 794: 1) | 1,089 |
| 1236 treasure list | `LootGen_TreasureList_Override` (794: 610; 1723: 45); `SoftLock_LootGen_TreasureList_Override` (794: 58); `Effect_Lootgen_DiscoveryTreasureListTemplate` (effects 776: 327; inside `PropertyProgression_Array` of 1389: 160); `AffectEntity_GrantTrophyCommand_TrophyDID` (2426: 5); trophy lists (7,568); level tables (3,825) | 12,598 |
| 1235 treasure group profile | trophy lists (14,546), treasure lists (2,299) | 16,845 |
| 3666 filtered list | `LootGen_FilteredTrophyTable` (794: 937; 1723: 157), `…2` (794: 957), `…3` (794: 275), `…4` (794: 99); `SoftLock_…` variants (794: 202, 123, 25); `PackageItem_FilteredTrophyTableTemplate` (2814: 275); `BigBattleControl_FTT_ForCashOut_Loot` (3787: 4) | 3,054 |
| 3307 weighted table | filtered lists (1,316); `PackageItem_Freep_WeightedTreasureTableID` (2814: 53), `PackageItem_Creep_WeightedTreasureTableID` (2814: 4); `LoginReward_TreasureTable` (3731: 6) | 1,379 |
| 2001 species table | `LootGenControl_SpeciesLevelLootTable` (1674: 4); `PrivateEncounterTemplate_SpeciesLevelLootTable` (1368: 21; 2651: 12) | 37 |
| 2729 skirmish lookup table | `LootGen_CustomSkirmishLootLookupTable` (794: 85); `SoftLock_…` (794: 14) | 99 |
| 2153 hobby profile | `Hobby_TreasureProfileList[]/…/Hobby_TreasureProfile` (2131: 120); `Mod_Array[]/…/Hobby_Fishing_RewardProfileOverride` (effects 734: 52) | 172 |
| 2227 runic level table | `ItemAdvancement_ItemLevelInfo_RunicLootTable` (2321: 453) | 453 |
| 2229 runic list | runic level tables (1,050); `ItemAdvancement_RunicLootListOverride` (796: 136); `RunicCombinationControl_SuccessList` / `_FailureList` (2301: 9 + 9) | 1,204 |

Mob records (1723) also hold `LootGen_GeneratesTrophies` (boolean, all 20,161; true in
12,514; default true). Containers (794) hold `LootGen_AssignLootRightsOnStartup` (1,385,
true in 779).

---

<a id="s5"></a>
## 5. Crafting

<a id="s5-1"></a>
### 5.1 Recipe (class 1024)

| Path | Type | Present (of 7,791) | Observed | Default |
|---|---|---|---|---|
| `CraftRecipe_Profession` | 20 | all | profession 1023 | |
| `CraftRecipe_Tier` | 8 | 7,682 | keys 1–16 (CraftTier) | |
| `CraftRecipe_TierArray` | 12 | 109 | one element `CraftRecipe_Tier`, key 10 in all | |
| `CraftRecipe_IngredientList` | 12 | all | 1–6 entries; 20,545 | |
| `…[]` element `CraftRecipe_IngredientEntry` | 11 | all | | |
| `…/CraftRecipe_Ingredient` | 20 | all | 796: 20,335; 795: 122; 802: 82; 799: 4; 797: 2 | |
| `…/CraftRecipe_IngredientQuantity` | 15 | all | 1–1,500 | |
| `…/CraftRecipe_IngredientIsConsumed` | 21 | all | true except 9 | true |
| `CraftRecipe_OptionalIngredientList` | 12 | 4,012 | 1–2 entries (2 in 4,005); 8,017; same element as above | |
| `…/CraftRecipe_IngredientCritBonus` | 9 | all optional entries | 0.25, 0.35, 0.45, 0.8 or 1.0 | 0.0 |
| `…/CraftRecipe_Ingredient`, `…Quantity`, `…IsConsumed` | | all optional entries | items 796; quantity 1 (7,927), 25 (79), 2 (11); consumed in all | |
| `CraftRecipe_IngredientPack` | 20 | 4,520 | items 796 | |
| `CraftRecipe_IngredientPackQuantity` | 15 | all | 1–15 | |
| `CraftRecipe_ResultItem` | 20 | all | never 0; 796: 3,607; 799: 1,926; 795: 1,435; 802: 413; 797: 171; 2814: 157; root class 794: 79 (see §1); 803: 3 | |
| `CraftRecipe_ResultItemQuantity` | 15 | 843 | 2–50 | **1** |
| `CraftRecipe_CriticalResultItem` | 20 | all | non-zero in 4,461; 0 in 3,330 | |
| `CraftRecipe_CriticalResultItemQuantity` | 15 | 4,067 | 0–100 (0 in 2,530) | **1** |
| `CraftRecipe_CriticalSuccessChance` | 9 | 4,461 | 0.05 (4,196), 0.01 (258), 0.1 (7): a fraction | 0.0 |
| `CraftRecipe_MultiOutputArray` | 12 | 2,414 | 1–9 entries; 4,772 | |
| `…[]` element `CraftRecipe_MultiOutputEntry` | 11 | all | fields `CraftRecipe_Ingredient` (an item), `CraftRecipe_ResultItem`, `CraftRecipe_CriticalResultItem` (0 in 974) | |
| `CraftRecipe_XPReward` | 8 | all | 14 distinct keys in 1–16 (CraftXPReward) | |
| `CraftRecipe_XPAward_Progression` | 20 | all | progression 1385 | |
| `CraftRecipe_XPAward_Progression_DrivingProperty` | 10 | all | one of 2 PIDs (7,682 / 109) | |
| `CraftRecipe_XPAward_Driver` | 15 | 109 | 10 | |
| `CraftRecipe_LevelMungeOutput` | 21 | 109 | false 68, true 41 | false |
| `CraftRecipe_UICategory` | 8 | all | 491 distinct keys in 2–716 (CraftUICategory) | |
| `CraftRecipe_ExecutionTime` | 9 | all | 1–30; 7.0 in 6,732 | 0.0 |
| `CraftRecipe_CooldownDuration` | 8 | 2,262 | keys 35, 36, 38 | 0 |
| `CraftRecipe_Cooldown_MithrilResetCost` | 15 | 7,682 | 0–5 (0 in 7,527) | 0 |
| `CraftRecipe_OneTimeRecipe` | 21 | 1,339 | true in all | false |
| `CraftRecipe_RequiredCraftGuild` | 20 | all | guild 2278 in 2,184; 0 in 5,607 | |
| `CraftRecipe_RequiredFacilityEffect` | 20 | 7,437 | effect records of class 718 | |
| `CraftRecipe_InductionAction` | 20 | all | induction records of class 1195 | |
| `CraftRecipe_Animation` | 8 | all | 11 distinct keys | |
| `CraftRecipe_Name` | 13 | 3,092 | | |
| `CraftRecipe_NameItemOnNormalSuccess` | 21 | all | true in 1,348 | false |
| `CraftRecipe_NameItemOnCriticalSuccess` | 21 | 2,014 | true in all | false |
| `CraftRecipe_Field_ResultIcon`, `CraftRecipe_Field_CritResultIcon` | 20 (surface) | 101 each | 0x41 surfaces (0 in 30) | |
| `EffectGenerator_CraftingCriticalSuccessEffectList` | 12 | all | exactly 2 elements `EffectGenerator_EffectStruct`: `EffectGenerator_EffectID` → one effect of class 745 and one of class 715; `EffectGenerator_EffectSpellcraft` −1.0 in all | |
| `Item_iLevel_Crafting_Offset` | 15 | 2 | 0, 2 | |

Co-occurrence rules (Confirmed, all 7,791):

1. **Two variants.** 7,682 recipes have `CraftRecipe_Tier` and `CraftRecipe_Cooldown_MithrilResetCost`
   and none of the next three; 109 recipes have `CraftRecipe_TierArray`,
   `CraftRecipe_XPAward_Driver` and `CraftRecipe_LevelMungeOutput` and neither of the first
   two. Every recipe is of exactly one variant.
2. `CraftRecipe_CriticalSuccessChance` is present exactly when `CraftRecipe_CriticalResultItem`
   is non-zero (4,461 recipes).
3. `CraftRecipe_CriticalResultItemQuantity` occurs both with and without a critical result
   item (1,535 with, 2,532 without).
4. `CraftRecipe_ResultItem` is never 0.

Meaning (Reported, LC): the ingredients are consumed (unless `IsConsumed` is false) to make
`ResultItemQuantity` of the result item, or `CriticalResultItemQuantity` of the critical
result item on a critical success. Optional ingredients raise the critical chance by their
`CritBonus`. A multi-output entry replaces the result and critical result when its
`Ingredient` is the ingredient the player supplied (a recipe with several possible inputs).
`XPReward` is a key into the craft control's experience table (§5.4). What the client
computes from these is §11.1 and §11.5.1–§11.5.3; the server rules still open are in §11.5.

<a id="s5-2"></a>
### 5.2 Profession (class 1023)

| Path | Type | Present (of 10) | Observed |
|---|---|---|---|
| `CraftProfession_TierArray` | 12 | all | exactly 16 entries each (160) |
| `…[]` element `CraftProfession_TierEntry` | 11 | all | |
| `…/CraftProfession_Tier` | 8 | all | keys 1–16, each once per profession |
| `…/CraftProfession_RecipeArray` | 12 | all | 2–33 elements `CraftProfession_Recipe` → recipes 1024 (1,733 in total) |
| `…/CraftProfession_ProficiencyXPCap`, `…/CraftProfession_MasteryXPCap` | 15 | all | 200–840 and 400–1,680 |
| `…/CraftProfession_TierLevelCapGate` | 15 | all | 16 distinct levels in 1–160; the same value for a given tier in all 10 professions, rising with the tier |
| `…/CraftProfession_ProficiencyTitle`, `…/CraftProfession_MasteryTitle` | 20 | all | title records of class 1470 |
| `…/CraftProfession_CompletedProficiencyLevelCharacteristic`, `…/CraftProfession_CompletedMasteryLevelCharacteristic` | 20 | all | trait records of class 1477 |
| `…/CraftProfession_TierOpeningCharacteristic` | 20 | 150 (absent in tier 1) | class 1477 |
| `…/CraftProfession_AutoCompleteProficiency`, `…/CraftProfession_AutoCompleteMastery` | 21 | all | true in all |
| `…/CraftProfession_ProficiencyTutorialHint` | 8 | all | 160 distinct keys |
| `CraftProfession_Item` | 20 | all | an item of class 796 |
| `CraftProfession_RequiredTool` | 18 | all | exactly one bit set in every record; two professions share the same bit (CraftToolCategory; bit *i* = key *i* + 1, `property-system.md` §7.3) |
| `CraftProfession_Characteristic` | 20 | all | class 1483 in 4; 0 in 6 |
| `CraftProfession_*_PropertyName` (12 fields: Enabled, ProficiencyLevel, ProficiencyXP, MasteryLevel, MasteryXP, CompletedProficiencyLevel, CompletedMasteryLevel, CriticalChanceAddModifier, StartTime, ExtraRecipeArray, OpenLevel, TierOpenLevel) | 10 | all | each a PID; different in every profession. They name the per-character properties that hold the profession's state. |
| `CraftProfession_AllowsXPAcceleration` | 21 | all | true in 7 |
| `CraftProfession_RecipeWebStoreCategory`, `WebStore_FilteringAttribute`, `WebStoreAccountItem_DataID` | 8, 8, 20 | all | store keys; the DID is 0 in all |
| `CraftProfession_Name`, `…_Description`, `…_LearnString`, `…_ChangeString` | 13 | all | |

`CraftProfession_OpenLevel_PropertyName` and `CraftProfession_TierOpenLevel_PropertyName`
hold PIDs from a separate, later range than the other ten.

Only 1,733 of the 7,791 recipes are listed in a profession's tier arrays; most of the
others are taught by recipe items (§5.5).

<a id="s5-3"></a>
### 5.3 Vocation (class 1026) and craft guild (class 2278)

| Path | Type | Present | Observed |
|---|---|---|---|
| `CraftVocation_ProfessionArray` (1026) | 12 | all 7 | exactly 3 elements `CraftVocation_Profession` → 1023 |
| `CraftVocation_ItemArray` | 12 | all 7 | exactly 4 elements `CraftVocation_Item` → items 796 (21) and 798 (7) |
| `CraftVocation_Name`, `…_Description`, `…_LearnString`, `…_ChangeString` | 13 | all | |
| `CraftGuild_Profession` (2278) | 20 | all 7 | 1023; seven different professions (one guild per profession) |
| `CraftGuild_Faction` | 20 | all | reputation-faction records of class 1889 |
| `CraftGuild_RequiredCraftTier` | 8 | all | key 2 in all |
| `CraftGuild_QuestCategory` | 8 | all | seven distinct keys |
| `CraftGuild_LearnString` | 13 | all | |

The vocation definition also allows `CraftVocation_AdvancedStartCraftmansTier_Array`; it
does not occur.

<a id="s5-4"></a>
### 5.4 Craft control (class 1665), craft directory (class 1021) and the DID maps

WEENIECONTENT labels: **CraftControl** → the class-1665 record, **CraftDirectory** → the
class-1021 record.

| Path (1665) | Type | Observed |
|---|---|---|
| `CraftControl_XPRewardArray` | 12 | 15 entries `CraftControl_XPRewardEntry` |
| `…/CraftControl_XPRewardEnum` | 8 | 15 distinct keys in 1–16 (key 13 absent) |
| `…/CraftControl_XPRewardValue` | 15 | 0–120 |
| `CraftControl_TierToMungeLevelArray` | 12 | 16 entries: `CraftProfession_Tier` (1–16, each once) and `Usage_MinLevel` (16 distinct levels in 10–160) |
| `CraftControl_ChangeVocation_RemoveQuestsArray` | 12 | 10 entries: `CraftControl_Profession` (→ 1023, each profession once) and `CraftControl_RemoveQuestsArray` (0–4 elements `CraftControl_QuestToRemove` → quests 1398) |

| Path (1021) | Type | Observed |
|---|---|---|
| `CraftDirectory_VocationArray` | 12 | 7 elements → the 7 vocations |
| `CraftDirectory_ProfessionArray` | 12 | 10 elements → the 10 professions |
| `CraftDirectory_CraftGuildArray` | 12 | 7 elements → the 7 guilds |

Confirmed: every `CraftRecipe_XPReward` key used by a recipe has an entry in
`CraftControl_XPRewardArray`.

**DID maps** (layout `records.md` §8; reached from the root map by the labels VOCATIONS and
PROFESSIONS, `property-system.md` §8.1):

| Map | Entries | Keys | Targets |
|---|---|---|---|
| 0x28000032 VOCATIONS | 8 | 0 (sentinel, DID 0) and 1–7 | the 7 vocation records, one per key |
| 0x28000033 PROFESSIONS | 11 | 0 (sentinel) and 1–10 | the 10 profession records, one per key |

The map labels are identifiers read at run time. The keys give each vocation and
profession a small number for enum-style use.

<a id="s5-5"></a>
### 5.5 Where recipes and professions are referenced (Confirmed)

| Target | Referencing property (root class of the holder) | References |
|---|---|---|
| Recipe 1024 | `RecipeItem_Recipe` (recipe items 805: 6,055; the same items also carry `Usage_BarringCraftRecipe` and an `Effect_Crafting_Recipe` usage effect, 6,055 references each); `CraftProfession_RecipeArray` (1,733); `Effect_Crafting_RecipeList[]/Effect_Crafting_Recipe` (items 3663: 1,370); `QuestEvent_RecipeDID` in quest objectives (1398: 128) | 7,788 distinct recipes are referenced; 3 are not |
| Profession 1023 | `CraftRecipe_Profession` (7,791); `Usage_RequiredCraftProfession` (805: 6,055; 794: 335; 1398 permission struct: 142; 3663: 71; 796: 26); `CraftTrinket_Profession` (796: 1,248); `Craft_OfferedProfessionArray` (NPC trainers 1724: 210 in 21 NPCs); `Quest_CraftProfessionDID` (15); `Quest_Reward_CraftingProfession` (10); `Effect_Crafting_Profession` (71) | |
| Guild 2278 | `CraftRecipe_RequiredCraftGuild` (2,184); `Craft_OfferedCraftGuildArray` (1724: 70 in 10 NPCs) | |
| Vocation 1026 | `Craft_OfferedVocationArray` (1724: 7 in 1 NPC) | |

---

<a id="s6"></a>
## 6. Vendors and barter

<a id="s6-1"></a>
### 6.1 Barter profile (class 1943)

| Path | Type | Present (of 3,449) | Observed | Default |
|---|---|---|---|---|
| `Barter_ItemListArray` | 12 | all | 1–40 lists; 42,214 lists | |
| `…[]` element `Barter_ItemList` | 12 | all | 2–4 elements (shapes below) | |
| `…[][]` element `Barter_ItemStruct` (a cost) | 11 | 58,556 | | |
| `…/Barter_Item` | 20 | 58,554 | 796: 54,827; 795: 3,589; 802: 123; 799: 8; 803: 4; 797: 3 | |
| `…/Barter_ItemQuantity` | 15 | 52,991 | 1–10,000 | (none) |
| `…/Barter_ItemQuantity_LookupTable` | 20 | 5,565 (342 profiles) | progressions 1385; `Barter_ItemQuantity` is absent wherever this is present | 0 |
| `…[][]` element `Barter_RewardItemStruct` | 11 | 42,207 | | |
| `…/Barter_Item` | 20 | all | 796: 20,818; 795: 13,583; 802: 3,742; 805: 1,524; 799: 1,307; 2814: 838; 797: 354; 4178: 21; 803: 20 | |
| `…/Barter_ItemQuantity` | 15 | all | 1–5,000 (1 in 41,424) | |
| `…/Barter_ItemUsesMinUsageLevelForCost` | 21 | 1,649 | true in all | false |
| `…[][]` element `Barter_RewardReputationStruct` | 11 | 7 (2 profiles) | `Barter_ReputationFaction` → faction 1889; `Barter_ReputationRewardTier` keys 1–5 | |
| `…[][]` element `Barter_SoundCategory` | 8 | 3 | key 45 | 0 |
| `Barter_Profile_Name` | 13 | 3,161 | | |
| `Barter_ItemMungeLevelOverride` | 15 | 202 | 18–160 | **−1** |
| `DefaultPermissionBlobStruct` | 11 | all | usage requirements of the profile: `Usage_AllowedWhileMounted` (3,445), `Usage_WorldEvent_AllConditionList` (2,703), `Usage_RequiredFaction` (536: faction 1889 and a tier key 1–10), `Usage_QuestRequirements` (513: quest 1398, status, operator), `Usage_MonsterPlayerCanUse` (206), `Usage_RequiredClassList` (18), `Usage_MinLevel` (13), `Usage_RequiredEffect` (2) | |

**List shapes** (Confirmed, all 42,214): every `Barter_ItemList` is 1–3 cost structs,
then exactly one reward, then at most one sound category, in that stored order:

| Shape | Lists |
|---|---|
| cost, reward item | 27,676 |
| cost, cost, reward item | 12,720 |
| cost, cost, cost, reward item | 1,811 |
| cost, reputation reward | 4 |
| cost, reputation reward, sound category | 3 |

Two cost structs have no `Barter_Item` and quantity 1; treat their item as DID 0 (the object form stores 0 there too).

Meaning (Reported, LC): each list is one trade: give all cost items in the stated
quantities, receive the reward item (or the reputation reward). A cost with a look-up table
has a level-dependent quantity (§6.2). The profile's usage requirements gate the whole
profile.

<a id="s6-2"></a>
### 6.2 Look-up-table quantities (Confirmed in the object form)

A cost whose quantity comes from `Barter_ItemQuantity_LookupTable` has no stored quantity
in the property form. The object form (§9.3) stores the resolved quantity. In all 5,565
such costs the stored quantity equals the following:

1. Take the list's reward item (`Barter_RewardItemStruct/Barter_Item`).
2. Choose its level: if the reward struct has `Barter_ItemUsesMinUsageLevelForCost` = true,
   the reward item's `Usage_MinLevel` (a top-level property, or the field of its
   `DefaultPermissionBlobStruct`); otherwise the reward item's `Item_Level`.
3. Read the progression record's property set: `Progression_MinimumIndexValue` *m* and the
   value array (`FloatProgression_Array`, elements `FloatProgression_FloatValue`).
4. Index *i* = level − *m*, clamped to 0 at the low end and to the last element at the high
   end (220 of the 5,565 costs clamp at the high end). The quantity is element *i*. In all
   5,565 cases the element is a whole number, so no rounding rule is exercised.

Counts: rule 2 with the minimum-usage level: 2,446 costs; with `Item_Level`: 3,119.
The progression format itself belongs to the progression spec.

> **Answered (2026-10-08, merge; was OPEN, implementer 2026-10-07):** Step 3 gives no type
> codes, and no progression spec exists yet: what are the types of
> `Progression_MinimumIndexValue` and of the elements of `FloatProgression_Array` (bare
> `FloatProgression_FloatValue` floats, or structs holding that field)? Until answered,
> `crates/formats` reads the minimum index from a 32- or 64-bit int and each element from a
> float or an int, bare or inside a struct.
>
> *Answer:* the progression format is `combat-data.md` §4, which is self-contained: §4.1 (row
> 1385, float array) gives the elements of `FloatProgression_Array` as bare
> `FloatProgression_FloatValue` values (f32), and §4.2 gives `Progression_MinimumIndexValue` as
> an int (absent ⇒ 0); §4.3 gives the index rule.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** Step 2 names two places for `Usage_MinLevel` (top
> level, or inside `DefaultPermissionBlobStruct`) but no precedence when a reward item has
> both with different values. `crates/formats` takes the top-level property first; with that
> order all 5,565 look-up costs of the install match the object form, so the data does not
> contradict it. A value that is not a whole number gives no quantity (no rounding rule).
>
> *Answer:* no property set of this install holds both (E3), so the order never decides here.
> **Decision (Tessera):** the top-level property first, as implemented; an element that is not
> a whole number gives no quantity and is reported (every element used here is whole, step 4).

Profiles with the override (V7, OBS2): its value is one of 22 levels from 18 to 160 (160 in
35 profiles, 100 in 33, 95 in 24). Of the 2,336 reward structs in the 202 profiles, 2,065
name an item that has `ItemMunging_MinMungeLevel` and 2,192 an item that has
`ItemMunging_ItemLevelOverrideProgression`; in all other profiles 12,050 of 39,871 rewards
have `ItemMunging_MinMungeLevel`. The barter class (1943) has no code that resolves costs or
rewards (§11.7).

> **OPEN:** What `Barter_ItemMungeLevelOverride` does. It does not change the level of step 2:
> none of the 5,565 look-up costs needs it to match, and 70 of them lie in profiles that have
> it. The counts above point to the level at which a scaling reward item is generated, but
> neither the data nor the class library states this. Only the server applies it; an owner
> observation can settle it: in the original client, buy the reward of a profile that has the
> override and compare the received item's level with the override value.

<a id="s6-3"></a>
### 6.3 Vendor inventory (class 1619) and the vendor properties of NPCs

| Path (1619) | Type | Present (of 400) | Observed |
|---|---|---|---|
| `VendorInventory_Items` | 12 | 393 | 1–656 elements `VendorInventory_Item`; 14,014 |
| `…[]` `VendorInventory_Item` | 20 | all | 796: 5,605; 805: 4,078; 795: 2,407; 799: 1,030; 2814: 473; 802: 246; 797: 174; 798: 1 |

Seven inventories have no item array (empty inventories).

Vendor properties of NPC records (root class 1724; mobs have none except barter):

| Path | Type | NPCs | Observed | Default |
|---|---|---|---|---|
| `Vendor_InventoryList` | 12 | 1,439 | 1–8 elements `Vendor_Inventory` → 1619; 1,763 | |
| `Vendor_ItemList` | 12 | 1,439 | 0–655 elements `Vendor_Item` (items); 91,274 | |
| `Vendor_SellModifier` | 9 | 1,439 | 1.0000001–80.0; 4.0 in 1,338 | −1.0 |
| `Vendor_SellsWearableItems` | 21 | 1,439 | true in 716 | false |
| `Vendor_BuysItems` | 21 | 1,310 | true in all | false |
| `Vendor_DiscountList` | 12 | 1,305 | 1–4 elements `Vendor_Discount` (type 10: a PID each); 2,262 | |
| `Barter_ProfileArray` | 12 | 1,084 (and 9 mobs) | 1–72 elements `Barter_Profile` → 1943; 8,843 (+ 9) | |
| `Barter_Profile_UseTabs` | 21 | 16,583 (and 9 mobs) | true in 834 (+ 9) | false |
| `Craft_OfferedProfessionArray`, `Craft_OfferedCraftGuildArray`, `Craft_OfferedVocationArray` | 12 | 21, 10, 1 | trainers (§5.5) | |

**Vendor_ItemList is derived** (Confirmed, 1,439 of 1,439 NPCs): its elements are exactly
the distinct items of all inventories named by the NPC's `Vendor_InventoryList`, each once.
Its length equals the number of distinct items.

<a id="s6-3-1"></a>
**Order of `Vendor_ItemList`** (V5; Confirmed, 1,439 of 1,439 NPCs). The list is in the
iteration order of a hash set of DIDs that grows while it is filled. The bucket counts are
those of `records.md` §1.3 (23, 47, 89, 191, 383, … for hints 1, 2, 3, 4, 5, …). To
reproduce it:

1. Make the **insertion sequence**: walk the inventories in `Vendor_InventoryList` order and
   each inventory's `VendorInventory_Items` in stored order; keep each item DID at its first
   occurrence and drop later repeats.
2. Start with *B* = 23 buckets, all empty, and a count of 0. Each bucket holds an ordered
   chain.
3. For each DID of the insertion sequence:
   1. If count + 1 is greater than 2 × *B*, grow first: take the next bucket count *B*′ of the
      table, make *B*′ empty buckets, then visit the old buckets from 0 to *B* − 1 and each
      chain from front to back, and append every DID to the **back** of chain (DID mod *B*′).
      Then set *B* = *B*′.
   2. Put the DID at the **front** of chain (DID mod *B*), with the DID taken as an unsigned
      32-bit number. Add 1 to the count.
4. The list is the buckets from 0 to *B* − 1, each chain from front to back.

Consequences: the final *B* is the smallest table value of at least 23 with 2 × *B* ≥ the
list length (lengths 0–46 give 23, 47–94 give 47, 95–178 give 89, and so on); the DIDs taken
modulo the final *B* are non-decreasing along the list. 538 of the 1,439 lists are longer
than 46 and exercise step 3.1; 148 lists happen to be in ascending DID order as well. The
class library reads `Vendor_ItemList` into an array in stored order (class 462 function 24, WL);
which order the vendor window shows is not established here, so a display may equally use
the inventories, which are authoritative.

Barter profiles are also named by `MithrilCoinsControl_DefaultBarterProfileArray` (class
3648, label MithrilCoinsControl: 5 references).

<a id="s6-4"></a>
### 6.4 Perk vendor (class 1796)

| Path | Type | Present | Observed |
|---|---|---|---|
| `PerkVendor_InventoryList` | 12 | both records | 27 and 45 elements `PerkVendor_PerkDID` → perk records of class 1377 |

Referenced by `AdvTable_PerkInventory` (class 853: 12) and `MonsterPlay_PerkInventory`
(class 1932: 7).

---

<a id="s7"></a>
## 7. Currency (classes 1028, 1029, 1996)

WEENIECONTENT labels: **CurrencyDirectory** → class 1029; **CurrencyControl** → class 1996;
**GoldCurrency**, **SilverCurrency**, **CopperCurrency** → the three class-1028 records.

| Path | Type | Present | Observed |
|---|---|---|---|
| `Currency_CurrencyList` (1029) | 12 | 1 record | 3 elements `Currency_Currency` → the three 1028 records |
| `Currency_Value` (1028) | 15 | all 3 | 1, 100 and 100,000: the value of one unit in units of the smallest denomination |
| `Currency_ObjectDID` (1028) | 20 | all 3 | an item of class 796 (the coin object) |
| `Currency_Name`, `Currency_Name_Singular`, `Currency_Name_Plural` (1028) | 13 | all 3 | |
| `Currency_Control_GarnishmentPercentage` (1996) | 9 | 1 record | 0.25 (a fraction) |
| `Currency_Control_GoldCapWebstoreItem` (1996) | 20 | 1 record | a store record of class 2784 |

Values are data; read them at run time. The unit of the money-drop amounts of §4.12 is not
known (OPEN in §11.6).

---

<a id="s8"></a>
## 8. Store records (low priority)

| Class | Records | Item link |
|---|---|---|
| 2541 | 589 | `WebStoreInventoryItem_DataID` → items (796: 546; 2814: 37; 795: 3; 802, 803, 3924: 1 each) |
| 3054 | 429 | `WebStoreInventoryItem_DataID` → items 796 |
| 3052 | 270 | `WebStoreInventoryItem_DataID` → items 796 |
| 2532 | 477 | `WebStoreInventoryItem_DataID` (47) → 2814; `WebStoreAccountItem_BundleItemArray` (139 records, 285 elements `WebStoreAccountItem_DataID` → other 2532 records) |
| 2534 | 1 | store directory: `WebStore_Directory_Category_Array` (82 entries) and single links to 2532, 2541, 2776, 2784, 3109 records |
| 1455, 1456, 1459, 1460 | 1 each | `WebStore_Stat_Trait_List` (20 traits of class 1477 each) |

All store items carry `WebStoreItem_SKU`, `WebStoreItem_Name`, button tooltips and (2541,
3052, 3054) `WebStoreItem_ShortName`. Classes 3052, 3054 and 2532 derive from 2541 > 2533
(`wlib.md` ancestry). A store item is a catalogue entry only: prices and purchase logic are
not in the data. The explorer may show "offered in the store" for items reached through
`WebStoreInventoryItem_DataID`.

---

<a id="s9"></a>
## 9. Object-state mirror (Confirmed)

<a id="s9-1"></a>
### 9.1 Overview

The 0x70 record of each class holds a script object of the root class (`wstate.md` §5).
Its attributes are in the library's order for the class (`wstate.md` §4.2); "attribute *n*"
below counts from 1 in that order. A reader of the economy needs only the property form;
this section lets a `tessera-wstate` user interpret the object form and cross-check it.

| Root class | Root attributes | Mirrors |
|---|---|---|
| 1236, 2728, 2729, 2229, 2239, 2278, 1996 | none (the bundle is the bare root) | nothing; the property form is the only copy |
| 2128 | one `refmultimap_u32` (37) | not analysed |
| 1237, 1917, 1918 | 1: reference to a `refarray` (182) of class-1233 objects | trophy entries, same order (§9.4) |
| 1235, 2372 | 1: `refarray` (176) of class-2810 objects; 2: weighted table (175) | item table, same order (§9.4, §9.2) |
| 3307 | 1: `refarray` (176) of class-3308 objects; 2: weighted table (175) | weighted table, **sorted by ascending weight** (§9.4) |
| 3666 | 1: `refarray` (176) of class-3665 objects | filtered entries, same order (§9.4) |
| 3669 | 1: `map_u32` (11) level → trophy list; 2: `map_u32` (11) level → treasure list | the non-zero entries only, as maps |
| 2227 | two `map_u32` (11) | level → trophy list (80 entries) and level → runic list (the 70 non-zero entries); equal to the property tables in all 15 |
| 2127 | one `map_u32` (11) | level → trophy-list template, non-zero entries only (0, 141 or 160 entries); equal in all 11 |
| 2001 | 1: `refmap_u64` (117) whose values are DID items | key (§9.5) → level table |
| 3671 | as 2001 | key (§9.5) → level table |
| 1674 | 1: an embedded class-2001 object whose map is the **union** of the four species tables of §4.12 (135 keys; no key occurs in two tables); 2: a DID item naming the PVP table | |
| 1673 | 1: `array_u32` (17), the 160 money-drop variances in order; 2: `array_u32` (17), the 160 maxima in order; 3: `map_u32` (11), drop-frequency label → IEEE-754 bits of the fraction; 4: `map_u32` (11), money-modifier label → bits of the multiplier | all four equal the property tables |
| 2153 | 1: integer = the advancement-progression DID; 2: `refarray` (182) of class-2154 objects | hobby entries, same order |
| 1943 | 1: `refarray` (182) of class-1942 barter entries; 2: a class-1602 object holding a `property_set` (166) | barter lists in order (§9.3); the profile's usage requirements |
| 1619 | 1: `array_u32` (25) | `VendorInventory_Items`, same order (all 400, including the 7 empty ones) |
| 1796 | 1: `array_u32` (25) | `PerkVendor_InventoryList` |
| 1029 | 1: `refarray` (182) of 3 | the currency list |
| 1028 | three StringInfo (199), one integer (the value), one integer (the coin item DID) | |
| 1024 | six references (below) | parts of the recipe |
| 1023 | twelve `map_u32` (11) and one `refmap_u32` (35) | tier → value maps (below) |
| 1026 | `array_u32` (25) of 4, `refmap_u32` (35, empty), `array_u32` (25) of 3 | items, (unused), professions |
| 1665 | 1: `map_u32` (11), tier → `Usage_MinLevel`; 2: `map_u32` (11), XP key → value; 3: `refmap_u32` (35) of 10 keys, profession → quests | 1 and 2 equal the property tables |
| 1021 | four `map_u32` (11) and three `array_u32` (25) | directory contents |

**Recipe (1024)** root attributes, all references (null 0xFFFFFFFF when the part is absent):

| Attribute | Content | Mirrors |
|---|---|---|
| 1 | `refarray` (182) of class-1022 objects (empty in 3,779) | `CraftRecipe_OptionalIngredientList` in order (7,791 of 7,791) |
| 2 | StringInfo (199) | present in all 7,791, also in the 4,699 recipes without `CraftRecipe_Name` |
| 3 | `refarray` (182) of class-1022 objects | `CraftRecipe_IngredientList` in order (7,791 of 7,791) |
| 4 | one class-1022 object, or null (3,271) | the ingredient pack: item = `CraftRecipe_IngredientPack`, quantity = `CraftRecipe_IngredientPackQuantity`; crit bonus 0 and consumed 1 in all 4,520 |
| 5 | `array_u32` (17) | the tier list: [`CraftRecipe_Tier`] or the `CraftRecipe_TierArray` keys (7,791 of 7,791) |
| 6 | `refarray` (182) of class-2635 objects, or null (5,377) | `CraftRecipe_MultiOutputArray` in order (2,414 of 2,414) |

Class 1022 (ingredient) has four attributes: crit bonus (float), consumed (integer 0/1),
quantity, item DID. Class 2635 (multi-output) has three: critical result, ingredient,
result, which is the reverse of the order in which this spec lists the property fields.

**Profession (1023)** root attributes 1–13 map tier keys 1–16 to: 1 tier-opening
characteristic (0 for tier 1); 2 completed-mastery characteristic; 3 mastery XP cap;
4 completed-proficiency characteristic; 5 a value that is 0 for every tier of every
profession (see the OPEN in §14); 6 and 8 the two auto-complete flags (both 1 everywhere, so
which is which cannot be told); 7 proficiency title; 9 tier level-cap gate; 10 tutorial-hint
key; 11 proficiency XP cap; 12 (`refmap_u32`, 35) tier → an `array_u32` (25) equal to that
tier's `CraftProfession_RecipeArray`; 13 mastery title. (Confirmed, 10 of 10.)

<a id="s9-2"></a>
### 9.2 Weighted table (native class 175)

Layout (`wstate.md` §5.3): a counted list of *n* elements of three u32, then a u32 that is
0. This spec fixes the meaning of the element words:

| Offset in element | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 | **Index**: the element's own position *i* (0-based). In the loot tables it is also the index of the matching element of the sibling `refarray`. |
| 4 | 4 | u32 | **Weight** *w*ᵢ of element *i* |
| 8 | 4 | u32 | **Subtree sum** *s*ᵢ: *w*ᵢ + *s*₂ᵢ₊₁ + *s*₂ᵢ₊₂, where a missing child counts 0 |

The elements form an implicit binary tree in array order (the children of *i* are 2*i* + 1
and 2*i* + 2). The root's sum *s*₀ is the total weight of the table. This structure allows
a weighted pick in logarithmic time.

Confirmed over every object record (0x70 and 0x76): the install has **8,131** class-175
tables, all in 0x70 records, by root class 1235 (6,418), 2372 (16) and 3307 (1,007) of this
spec and 2063 (163), 3044 (21) and 3138 (506) elsewhere. The subtree-sum rule holds in all
8,131. The index rule holds in all 7,441 tables of 1235, 2372 and 3307, and also in the 184
of 2063 and 3044 (7,625 in all), but in none of the 506 tables of class 3138, where the first
word carries another value. The trailing u32 is 0 in all.

For 1235 and 2372 the weights are the property-form weights in stored order (6,434 of
6,434). For 3307 they are the weights of the sibling array, which is sorted (§9.4).

<a id="s9-3"></a>
### 9.3 Barter entry (native class 1942)

The 49-byte entry of `wstate.md` §5.4, one per `Barter_ItemList`, in list order (42,214 of
42,214):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | DID | The barter profile that contains the entry (all) |
| 4 | 4 | DID | Reward item (`Barter_RewardItemStruct/Barter_Item`); 0 when the reward is reputation |
| 8 | 4 | u32 | Reward quantity (`…/Barter_ItemQuantity`); 0 when the reward is reputation |
| 12 | 1 | bool8 | `Barter_ItemUsesMinUsageLevelForCost` of the reward (0 when absent) |
| 13 | 4 | DID | Reputation faction (`Barter_RewardReputationStruct/Barter_ReputationFaction`); 0 otherwise |
| 17 | 4 | u32 | Reputation reward tier key; 0 otherwise |
| 21 | 8 × 3 | (DID, u32) × 3 | Cost 1–3 in list order: item and quantity; unused slots are (0, 0). For a look-up cost the quantity is the resolved value of §6.2. |
| 45 | 4 | u32 | `Barter_SoundCategory` key; 0 when absent |

A barter list never has more than three costs, which the fixed layout requires.

<a id="s9-4"></a>
### 9.4 Element classes of the loot and hobby mirrors

| Class | Attributes (library order) | Correspondence |
|---|---|---|
| 1233 (trophy entry) | group drop (0/1), drop-frequency key, quantity, item-or-profile DID, a fifth integer | `GroupDrop`, `DropFrequency`, `ItemQuantity` (with the default 1 filled in), `ItemOrProfile`; the fifth is 0 in all 77,745 |
| 2810 (item-table entry) | quantity, item DID | `Quantity` (default 1 filled in), `Item` |
| 3308 (weighted-table entry) | trophy-list DID, weight | `TrophyList`, `TrophyList_Weight` |
| 3665 (filtered entry) | entry DID, reference to a class-3033 object | `LootGen_FilteredTrophyTable_EntryDID` (11,618 of 11,618); 3033 holds a `refarray` (176) of filters |
| 3664 (property-set filter) | PID, reference to a `refarray` (176) of `property` (39) items | one per **non-empty** `EntityFilter_PropertySet`; empty property-set filters are dropped (the filter counts then agree in all 11,618 entries) |
| 3739 (range filter) | reference to the minimum, reference to the maximum (each a `property`, 39) | `EntityFilter_PropertyRange` |
| 1898, 4141, 199 | world-event filter objects | `EntityFilter_WorldEvent` (326) |
| 2154 (hobby entry) | min proficiency, weight, time-available key, meta score, max proficiency, item DID | the six `HobbyRewardProfile_*` fields; absent meta score and time key appear as 0 |

<a id="s9-4-1"></a>
**Order of 3307 entries** (W5; Confirmed, 1,007 of 1,007). The `refarray` holds the same
(trophy list, weight) pairs as the property array, by ascending weight, in the order in which
a binary min-heap on the weight gives them up:

1. Start with an empty array *H* (positions from 0; the parent of position *p* > 0 is
   ⌊(*p* − 1) / 2⌋, the children of *i* are 2*i* + 1 and 2*i* + 2).
2. For each pair in property-array order: put it at the end of *H*; then, while it is not at
   position 0 and its parent's weight is **strictly greater** than its weight, move the
   parent down into its position and continue from the parent's position; place the pair
   where this stops.
3. Until *H* is empty: output *H*[0]; remove the last element of *H*; if *H* is not empty,
   put that element at position 0 and sift it down: at position *i*, pick *i*; if the left
   child exists and the picked element's weight is strictly greater than the left child's,
   pick the left child; then, if the right child exists and the picked element's weight is
   strictly greater than the right child's, pick the right child; if the pick is *i*, stop,
   otherwise swap the two and continue at the picked position.

The output order is the `refarray` order, and the class-175 table of the same record holds
the weights in that order (§9.2). For reference: 448 records have no equal weights, and among
the others a stable sort by weight happens to give the same order in 518 and a stable sort of
the reversed array in 12; only the rule above matches all 1,007. Worked example: §13.6.

<a id="s9-5"></a>
### 9.5 Keys of the species and PVP maps (class 117)

| Map | Key (u64) | Confirmed |
|---|---|---|
| Species table (2001, and the union in 1674) | (`MonsterLevel_ExaminationModStatType` × 2³²) + (`Agent_Species` × 2²⁰) + (`Agent_Subspecies` × 2⁴) | 264 of 264 entries (3 of them with a non-zero subspecies) |
| PVP table (3671) | `Agent_Species` × 2¹⁶ | 15 of 15; `Agent_Class` is 0 in all, so its position is unknown (OPEN in §14) |

Bits 0–3 of the species key are 0 in all observed keys.

<a id="s9-5-1"></a>
**The class library's four-field key** (W6). The class library has one function that packs
four numbers into a 64-bit key with the same bit positions as the species key (class 976
function 10, WL): field 1 × 2³² + field 2 × 2²⁰ + field 3 × 2⁴ + field 4, each field taken as
an unsigned 32-bit number. It refuses (logs an error and returns the key 0) when field 4 is
greater than 15, field 3 greater than 65,535 or field 2 greater than 4,095, so the layout is:
bits 32–63 field 1, bits 20–31 field 2 (12 bits), bits 4–19 field 3 (16 bits), bits 0–3
field 4 (4 bits). Its one caller is the membership test of class 975 (function 2), which
looks keys up in a `set_u64` (class 105). When field 2 is non-zero, that test replaces
field 1 by the value that the `map_u32` of the class-1658 record (35 entries, every value a
single bit from 1 to 8,192) gives for field 2. It then tries, in this order and stopping at
the first key in the set: (field 1, 2, 3, 4) as given; field 4 set to 0; field 3 set to 0;
fields 3 and 4 set to 0; fields 2 and 3 set to 0; fields 2, 3 and 4 set to 0; fields 1, 2 and 3
set to 0. OBS2: among all `set_u64` keys of the install (549 keys, all in records of root class 884),
bits 0–3 take the values 0–3, 6 and 7. The loot species tables (2001, the union in 1674) use
the same bit positions with `MonsterLevel_ExaminationModStatType` as field 1, `Agent_Species`
as field 2 and `Agent_Subspecies` as field 3 (264 of 264 and 135 of 135, field 4 = 0 in all);
the code that builds their lookup keys is not in the client (§11.7).

> **Answered (2026-10-08, merge; was OPEN, implementer 2026-10-07):** The widths of the
> packed fields are not stated. With the multipliers above, a species key of 2¹² or more
> spills into the examination-type field (bit 32 and up), and a subspecies key of 2¹⁶ or more
> collides with the species field (bit 20 and up). What are the field widths (and is the
> examination type limited to 32 bits)? Until answered, `crates/formats` reports no key for an
> entry outside those ranges (all observed keys are inside them), and the PVP key only while
> `Agent_Class` is 0.
>
> *Answer:* §9.5.1 (W6): bits 32–63 hold field 1 (the examination type, an unsigned 32-bit
> number), bits 20–31 field 2 (species, 12 bits), bits 4–19 field 3 (subspecies, 16 bits),
> bits 0–3 field 4; the class library's packing refuses values above those widths and returns
> the key 0. That the loot species tables are keyed by that same function is not shown (W6,
> §14). The position of `Agent_Class` in the PVP key stays OPEN in §14.

---

<a id="s10"></a>
## 10. DID graph and cross-links

<a id="s10-1"></a>
### 10.1 Edges of the loot graph (Confirmed)

Class-level edges (an edge A → B means some field of an A record points at a B record):

| From | To |
|---|---|
| 1674 | 2001, 3671 |
| 2001, 3671 | 3669 |
| 2128 | 2127 |
| 2127 | 1237, 1917 |
| 2729 | 2728 |
| 2728 | 1237 (and encounter 2651, match parameters 2727) |
| 3666 | 1237, 3307 |
| 3307 | 1237 |
| 3669 | 1237, 1236 |
| 2227 | 1237, 2229 |
| 2229 | relic 2219, 2239 |
| 2239 | relic 2219 |
| 1237 | items, 1235, 1236 |
| 1917 | items, 2372 |
| 1918 | items |
| 1236 | 1235 |
| 1235, 2372 | items |
| 2153 | items (and progression 1385) |

Among the loot-table classes no edge leads back to a class above it, so the graph of the
loot tables alone is **acyclic**. The longest path from an attaching record (§4.13) to an
item has 6 edges (container → 3666 → 3307 → 1237 → 1236 → 1235 → item, or 1674 → 2001 →
3669 → 1237 → 1236 → 1235 → item). The graph **with items is not acyclic**: package items
(2814) and disenchantable items attach trophy lists (§4.13) and are themselves loot
targets, so a walk must stop at item records and keep a visited set.

<a id="s10-2"></a>
### 10.2 Building the cross-links (algorithm)

All lookups use the property form. Build the indexes once per install; they are bounded by
the counts of §2 and §4.13.

**Drops from** (item → sources):

1. For every loot record, record the forward edges of §10.1 (one per non-zero DID field).
2. Invert them into a reverse index: target DID → list of (source DID, field label).
3. Add the attaching edges of §4.13 (mobs, containers, package items, disenchanting items,
   relics, effects, encounters, item-advancement records) to the same reverse index.
4. For an item, walk the reverse index breadth-first from the item, keeping a visited set,
   and stop at records that are not loot tables (§2.1 classes and 2229). Those records are
   the sources. Keep the path for display (for example "via treasure group X and trophy list
   Y"). A depth limit of 8 is safe for this install (§10.1).
5. Species tables (2001, 3671) and the world-event and skirmish lookups are sources of a
   different kind: show them as "creatures of species S at level L", "world event", or
   "skirmish encounter E" rather than as records.

**Sold by** (item → NPCs): for every NPC with `Vendor_InventoryList`, for every inventory,
for every `VendorInventory_Item`, add (item → NPC, inventory). `Vendor_ItemList` gives the
same set (§6.3) and may be used as a check.

**Bartered for** (item → profiles and NPCs): reward items of `Barter_RewardItemStruct`
give "obtainable from profile P"; NPCs (and the 9 mobs) with `Barter_ProfileArray` give the
traders. Cost items give the reverse link "used as barter currency in P".

**Crafted by** (item → recipes): `CraftRecipe_ResultItem`, `CraftRecipe_CriticalResultItem`
(when non-zero) and the `ResultItem` and `CriticalResultItem` of every multi-output entry.
Each recipe adds its `CraftRecipe_Profession` and tier (§5.1). "Learned from": items with
`RecipeItem_Recipe` = the recipe; professions whose tier `RecipeArray` lists it.

**Used in** (item → recipes): `CraftRecipe_Ingredient` of the required and optional lists,
the multi-output `Ingredient`, and `CraftRecipe_IngredientPack`.

**Offered in the store** (item → store records): `WebStoreInventoryItem_DataID` (§8).

<a id="s10-3"></a>
### 10.3 Reference volumes into items (Confirmed)

Counts of non-zero references (a few recipe results are class-794 records, §1).

| Path | References |
|---|---|
| `TreasureGroupProfile_ItemTable[]/…_Item` | 116,764 |
| `Vendor_ItemList[]` (NPCs) | 91,274 |
| `Barter_ItemStruct/Barter_Item` | 58,554 |
| `LootGen_TrophyList[]/…_ItemOrProfile` (items only) | 55,610 |
| `Barter_RewardItemStruct/Barter_Item` | 42,207 |
| `CraftRecipe_IngredientList[]/…_Ingredient` | 20,545 |
| `VendorInventory_Items[]` | 14,014 |
| `CraftRecipe_OptionalIngredientList[]/…_Ingredient` | 8,017 |
| `CraftRecipe_ResultItem` | 7,791 |
| `CraftRecipe_MultiOutputArray[]` (three DIDs per entry, results and ingredient) | 13,342 |
| `CraftRecipe_IngredientPack` | 4,520 |
| `CraftRecipe_CriticalResultItem` | 4,461 |
| `HobbyRewardProfile_ItemArray[]/…_Item` | 2,689 |

---

<a id="s11"></a>
## 11. Behaviour

<a id="s11-1"></a>
### 11.1 What the client does (BIN)

**B1. Recipe preview in item tooltips.** When the client builds the comparison part of an
item's examination tooltip and the item's compare type (`UI_Examination_Tooltip_Override_Compare_Type`
of the tooltip request, else the item's `UI_Examination_Tooltip_Compare_Type`) is **4**, it:

1. reads the item's `RecipeItem_Recipe`; stops if it is 0;
2. loads that recipe's property set (DID + 0x09000000) and reads `CraftRecipe_ResultItem`
   and `CraftRecipe_ResultItemQuantity`, using 1 when the quantity is absent; stops if the
   result item is 0;
3. builds a second tooltip for the result item, passing an `Inventory_Quantity` value equal
   to that quantity.

It does not read the critical result for this preview.

**B2. No client-side evaluation of the economy tables was found.** A search of the whole
program for the PIDs of all 520 properties of this family as immediate operands found no
code that reads the trophy lists, treasure tables, level tables, barter lists, vendor
inventories or the loot control table as data. The hits were user-interface element
identifiers that share the same numeric range, the generic property-registration code, and
B1. Caveat: code that reaches a property through a computed or data-driven PID would not
show up in such a search. With that caveat, drop rolls, barter cost evaluation and vendor
stock are server behaviour: the client receives their results.

The search covered the executable only. The class library, which the client also runs, is
data and reads some properties of this family through PID constants in its code: recipe and
profession fields, `Vendor_SellModifier`, `Vendor_DiscountList` and `Vendor_ItemList`
(§6.3.1, §11.5, §11.6.1). It holds no loot-table, barter-cost or money code (§11.7).

<a id="s11-2"></a>
### 11.2 Trophy-list roll (server; Reported model)

The model of LC, which an offline sandbox may use:

1. For every entry of the list, independently, draw a uniform number in [0, 1) and compare
   it with the entry's drop-frequency fraction (§4.12); the entry drops when the number is
   below the fraction.
2. A dropped entry whose target is an item yields `ItemQuantity` of it.
3. A dropped entry whose target is a treasure group profile (1235, 2372) yields one weighted
   pick from it (§11.3); a treasure list (1236) first picks a profile by weight, then an
   item from it.

What the class library does with a trophy list (WL): it can present one as an examined
object. Class 1237 function 91 returns the list's `Description` and function 92 its
`Icon_Layer_ImageOverrideDID` (0 when absent), and the examination function of class 1073
(function 4) uses the description as the examination text when the examined object is a
trophy list. So the description that 26 trophy lists carry is examination text; where the
icon is used is not shown. The class library has no code that rolls a list (§11.7).

> **OPEN:** Whether the server rolls each entry independently (the LC model), how
> `GroupDrop` entries are distributed in a fellowship (one roll for the group, one item for
> one member, or one per member), what `Inventory_MaxQuantity` on trophy lists does, and how
> often a creature rolls its trophy list. Only the server decides: neither the executable
> (B2) nor the class library (L7) holds this logic.

<a id="s11-3"></a>
### 11.3 Weighted pick (server; data-backed structure)

A weighted pick from a table with weights *w*₀ … *w*ₙ₋₁ chooses entry *i* with probability
*w*ᵢ ÷ Σ*w*. The object form stores a subtree-sum tree for this (§9.2), which a server can
descend in logarithmic time. An implementation that only needs the distribution may use a
cumulative-sum search over the property-form weights instead; both give the same
probabilities.

> **OPEN:** The server's random-number generator, its range (integer in [0, Σ*w*) or
> real), and the descent order in the subtree-sum tree (node first, then left subtree, then
> right, or another order). These decide which entry a given random number selects, not the
> probabilities. Without server code, an offline backend should treat any correct weighted
> pick as equivalent. The classes that hold these tables (1235, 2372, 1236, 3307) have no
> pick code in the class library (L7), so the client cannot show the descent order.

> **OPEN:** How many picks a runic treasure group makes: the LC model picks a pull count by
> the weights of `RunicTreasureGroup_PullCount_Array`, then that many relics, but whether
> the relic picks are with or without replacement is not known. Classes 2227, 2229 and 2239
> have no functions of their own in the class library (L7).

<a id="s11-4"></a>
### 11.4 Level and filter selection (server)

> **OPEN:** Which level indexes a level-based table (3669, 2127, 2227): the creature's
> level, the looter's level, or a clamped mix; what happens above the last level (160 or
> 80). The tables cover every level from 1 to their size, so a clamp to the last entry is
> the safe sandbox choice.

> **OPEN:** For filtered lists (3666): whether all entries whose filters hold apply, or only
> the first; whether range bounds are inclusive at both ends (the data uses pairs such as
> 10–19, 20–29, which suggests inclusive); which entity a filter tests (the looter or the
> container's owner). The data shows that the lists are ordered and that the most common
> filters are class and level bands.

> **OPEN:** How a creature's species table is chosen when several attach (the four tables of
> the directory control, private-encounter overrides) and which wins on a key conflict. The
> union in the 1674 object form has no conflicts in this install, so the data cannot say.
> The lookup functions of 2001, 3671, 3669, 3666 and 1674 are declared without bodies in the
> class library (L7). Whether a species-table lookup retries with fields set to 0 the way
> the classification filter of §9.5.1 does is not known; the 264 keys use no subspecies
> except in 3 entries.

<a id="s11-5"></a>
### 11.5 Crafting (server)

<a id="s11-5-1"></a>
**Critical-success chance shown while crafting** (R5). The class library's
recipe-description function (class 263 function 104, WL) hands the crafting panel three
kinds of input: the recipe's `CraftRecipe_CriticalSuccessChance` (0.0 when absent); the
character's value of the float property whose PID is the recipe profession's
`CraftProfession_CriticalChanceAddModifier_PropertyName` (0.0 when the character has none;
class 263 function 6); and, for each optional ingredient in list order, its crit bonus (the
class-1022 attribute that mirrors `CraftRecipe_IngredientCritBonus`, §9.1). The panel
(BIN2) then computes, in single precision:

1. *c* = recipe chance + profession modifier;
2. for each optional-ingredient slot that is in its selected state in the panel, add that
   ingredient's crit bonus to *c* (slots that are not selected add nothing);
3. clamp *c* to the range 0.0 … 1.0;
4. show round(100 × *c*) as a whole percentage, rounding halves away from zero (add 0.5,
   or subtract it for a negative value, then truncate).

So bonuses **add**, the sum is **capped at 1** (a crit bonus of 1.0, 4,079 optional
entries, therefore shows 100 %), and the profession modifier is a further addend. These
constants are BIN facts (decision 0012): cap 1.0, scale 100.0, rounding offset 0.5.

> **OPEN:** Whether the server rolls a craft's critical success against the same value the
> panel shows (step 3, before the rounding of step 4). The client only displays it. An owner
> observation can test it: in the original client, craft a recipe whose shown chance is 100 %
> and check that every craft is critical, and craft one with a low shown chance often enough
> to compare the rate.

<a id="s11-5-2"></a>
**Experience value of a recipe** (R4). The same recipe-description function looks up the
recipe's `CraftRecipe_XPReward` key in the experience map of the CraftControl record
(class 1665 function 85, WL; attribute 2 of §9.1, equal to `CraftControl_XPRewardArray`) and
passes the value on as the recipe's experience; a key without an entry gives 0. Every key used
by a recipe has an entry (§5.4).

> **OPEN:** How the server credits that value: how it is split between proficiency and
> mastery, how `CraftRecipe_XPAward_Progression` and its driving property scale it, and what
> the 109 tier-array recipes (`XPAward_Driver` 10, `LevelMungeOutput`) change. No class-library
> code reads `CraftRecipe_XPAward_Progression`, `…_DrivingProperty` or `…_Driver`.

<a id="s11-5-3"></a>
**Cooldown durations** (R3). WEENIECONTENT label **CooldownControl** → the record of class
1664. Its property `CooldownControl_DurationMapList` holds entries
`CooldownControl_DurationMapListEntry` with `CooldownControl_DurationType` (an enum key of
the same mapper as `CraftRecipe_CooldownDuration`) and `CooldownControl_DurationValue`
(float); its object form holds the same pairs as a `map_u32` from key to the float's bits
(63 entries in both forms, equal). The class library resolves a recipe's cooldown by looking
its `CraftRecipe_CooldownDuration` key up in that map, giving −1.0 when the key has no entry
(class 1664 function 85 via class 1024 function 111), and passes the result to the crafting
panel. The three keys used by recipes have entries: 35 → 64,800, 36 → 237,600,
38 → 583,200 (values of this install; read them at run time). Unit: **seconds** (derived:
the map's values include 60, 3,600, 86,400, 604,800 and 2,592,000, a minute, an hour, a day,
a week and 30 days in seconds, and every value is a whole number).

> **OPEN:** Units and effect of `CraftRecipe_ExecutionTime` (1–30, 7.0 in 6,732 recipes;
> presumably seconds of craft animation). No class-library code reads it or
> `Craft_ExecutionTime_Override`, and the recipe description handed to the panel does not
> carry it. An owner observation can settle the unit: time the progress bar of a craft in the
> original client for recipes with different values.

<a id="s11-6"></a>
### 11.6 Vendors, barter and money (server)

<a id="s11-6-1"></a>
**Purchase price** (V6). The class library's vendor code computes the price a character pays
a vendor for *q* units of an item (class 462 function 19, with the discount of function 33
and the discount list of function 13; WL). All arithmetic is single-precision floating
point.

1. *s* = the vendor's `Vendor_SellModifier` (−1.0 when absent). If *s* ≤ 1.0 the code logs an
   error and goes on.
2. Discount factor *d*: start at 1.0. Read the vendor's `Vendor_DiscountList`; if it is
   absent, *d* = 1.0. Otherwise, for each element in order, subtract from *d* the buyer's
   value of the float property whose PID the element holds (0.0 when the buyer has none).
   If *d* < 0.0, set *d* = 0.0. If any element cannot be read or holds PID 0, the code logs an
   error and the whole list is ignored (*d* = 1.0).
3. *m* = *s* × *d*; if *m* < 1.0, set *m* = 1.0.
4. If the item is not an item object, the purchase fails (price 0, result "no").
5. *v* = the item's `Item_Value` (an integer, 0 when absent), taken as unsigned.
6. price = truncate(*v* × *q* × *m* + 0.5), the products formed left to right; the purchase
   is allowed when the price is greater than 0.

So a vendor never sells below the item's value (*m* ≥ 1), and discounts lower the multiplier
`Vendor_SellModifier` but not below 1. With the data of this install every `Vendor_SellModifier`
is greater than 1.0 (§6.3). Worked example: §13.7.

> **OPEN:** The price a vendor pays when it buys an item from a character (selling to a
> vendor). No class-library code computes it: the class library hands the sale to code
> outside this spec. An owner observation can settle it: sell items of known `Item_Value`
> to a vendor in the original client and compare the money received with `Item_Value` ×
> quantity. The item value properties belong to the item spec.

> **OPEN:** Money drops: how the server indexes `LootGenControl_MoneyDropTable` (160 entries;
> whether by level is not shown by the data), how the variance applies (± around the maximum,
> or below it), and how the money-drop-modifier labels are attached to creatures. No property
> set of this install (none of the 383,332 object property sets and none of the 1,616 0x78
> sets) carries a property whose label starts with `LootGen_Money`, so the attachment is not
> in the shipped data. The loot control class (1673) declares 12 functions of its own and
> ships a body for none of them (L7), so the client holds no money-drop code either.

> **OPEN:** The unit of the `LootGenControl_MoneyDropTable` amounts
> (`…_DropAmountMaximum`, `…_DropAmountVariance`). Neither the data nor any source consulted
> states it: no property links the table to a currency record of §7, and the amounts are bare
> integers. Whether they count the smallest denomination (the `Currency_Value` 1 record) or
> another unit is unknown. An implementation must not assume a unit until this is answered.
> An owner observation can settle it: note the money dropped by creatures of one level in the
> original client and compare it with the table's entry for that level.

<a id="s11-7"></a>
### 11.7 What the class library contains for these classes (WL)

The class library (`wlib.md`, `wlib-bytecode.md`) declares, for each class of §2, the
functions it adds to its parent's method table. For the loot, barter-cost and money logic
the client's library ships the declarations but **no bodies** (method-table entries
0xFFFFFFFF, `wlib.md` §6.4). Census of the functions each class adds beyond its parent
(Confirmed, every class of §2):

| Classes | Added functions | With a body |
|---|---|---|
| 1235, 1236, 3666, 3669, 2001, 3671, 2127, 2128, 2227, 1673, 1674, 3399, 1917, 1918, 1619 | 10, 3, 7, 3, 4, 3, 1, 4, 3, 12, 1, 4, 1, 1, 4 | none |
| 2372, 2229, 2239, 2728, 2729 | none | — |
| 1237 | 9 | 3: a class check (function 87), the description and the icon of §11.2 (91, 92) |
| 3307 | 9 | 1: a class check (function 85) |
| 2153 | 6 | 2: a class check (86) and a filter (89) that collects, in entry order, the item DIDs of the entries whose `MinProficiency` ≤ a given proficiency ≤ `MaxProficiency` and whose meta score ≤ a given value |
| 1943 | 13 | 6: accessors and checks; none resolves costs, look-up tables or rewards |
| 1024, 1023, 1026, 2278, 1665, 1021, 1664 | 42, 41, 16, 7, 6, 18, 2 | 26, 23, 7, 5, 3, 3, 2: accessors and the look-ups of §11.5 |
| 1796, 1028, 1029, 1996 | 3, 6, 4, 3 | 2, 4, 3, 2: accessors |

Every class of §2 also has its own initializer (function 0) that creates the empty
containers of §9.1. The vendor code of §11.6.1 belongs to class 462, not to a class of §2.
Consequences: drop rolls, weighted picks, level and filter selection, species-table
precedence, runic pulls, barter-cost evaluation and money drops cannot be read from the
client; they stay OPEN in §11.2–§11.6.

---

<a id="s12"></a>
## 12. Invariants

Each is Confirmed on every record of this install unless noted. A reader should check them
on synthetic fixtures and may warn (not fail) when real data breaks one.

| # | Invariant |
|---|---|
| E-I1 | Every record of the classes of §2 has a property set at DID + 0x09000000 that decodes completely. |
| E-I2 | Every non-zero DID in the fields of §4–§8 names an existing record of one of the target classes listed for that field. |
| E-I3 | Every `LootGen_TrophyList_Entry` has `ItemOrProfile` (non-zero) and `DropFrequency`; every drop-frequency key used has an entry in `LootGenControl_DropFrequencyTable`. |
| E-I4 | Every weight of a treasure group profile is ≥ 1; every treasure list, weighted table and hobby profile has at least one entry with weight ≥ 1. |
| E-I5 | A level-based table and a world-event level table have exactly 160 entries whose levels are 1 … 160 in order; a runic level table has 80 with levels 1 … 80 in order. |
| E-I6 | Within one species table, the key of §9.5 is unique. |
| E-I7 | A recipe is of exactly one variant (§5.1 rule 1); `CraftRecipe_CriticalSuccessChance` is present exactly when the critical result item is non-zero; `CraftRecipe_ResultItem` is non-zero. |
| E-I8 | A profession has 16 tier entries with tier keys 1 … 16, each once. |
| E-I9 | Every `Barter_ItemList` matches one of the shapes of §6.1 (1–3 costs, one reward, optional sound category). |
| E-I10 | A barter cost has either `Barter_ItemQuantity` or `Barter_ItemQuantity_LookupTable`, never both. |
| E-I11 | For every NPC, the item set of `Vendor_ItemList` equals the union of the items of its inventories, without duplicates. |
| E-I12 | Class-175 tables: element *i* has third word = weight + third words of elements 2*i* + 1 and 2*i* + 2 (if present); the trailing word is 0. |
| E-I13 | Barter entries (1942): one per barter list, in order, with the field correspondence of §9.3. |
| E-I14 | The graph of the loot-table classes (§10.1, items excluded) is acyclic; a breadth-first walk from any loot table reaches items within 6 edges. |
| E-I15 | VOCATIONS and PROFESSIONS map keys 1 … *n* to the vocation and profession records one-to-one, with the sentinel key 0 → DID 0. |
| E-I16 | For every weighted treasure table (3307), the class-3308 `refarray` equals the property array reordered by the heap rule of §9.4.1. |
| E-I17 | For every NPC, `Vendor_ItemList` equals the list that the hash-set rule of §6.3.1 builds from its inventories. |
| E-I18 | Every `CraftRecipe_CooldownDuration` key used by a recipe has an entry in the CooldownControl duration map (§11.5.3), and the map's object form equals its property list. |

---

<a id="s13"></a>
## 13. Worked synthetic examples

All DIDs below are invented (prefix 0x70F0…) and belong to no install.

<a id="s13-1"></a>
### 13.1 Trophy list with defaults

A synthetic trophy-list property set:

| Path | Value |
|---|---|
| `LootGen_TrophyList[0]/…_ItemOrProfile` | 0x70F00010 (an item) |
| `LootGen_TrophyList[0]/…_DropFrequency` | key 9 |
| `LootGen_TrophyList[1]/…_ItemOrProfile` | 0x70F00020 (a treasure group profile) |
| `LootGen_TrophyList[1]/…_DropFrequency` | key 2 |
| `LootGen_TrophyList[1]/…_GroupDrop` | true |

With a synthetic control table where key 9 → 0.7 and key 2 → 0.1, a reader reports: entry 0,
item 0x70F00010, quantity 1 (default), fraction 0.7, not a group drop; entry 1, profile
0x70F00020, quantity 1, fraction 0.1, group drop. The object mirror holds two class-1233
objects with attribute values (0, 9, 1, 0x70F00010, 0) and (1, 2, 1, 0x70F00020, 0).

<a id="s13-2"></a>
### 13.2 Subtree-sum table

Weights 2, 5, 1, 4 (four elements) give:

| *i* | Index word | Weight | Children | Sum |
|---|---|---|---|---|
| 3 | 3 | 4 | none | 4 |
| 2 | 2 | 1 | none | 1 |
| 1 | 1 | 5 | 3 | 5 + 4 = 9 |
| 0 | 0 | 2 | 1, 2 | 2 + 9 + 1 = 12 |

The stored class-175 body is the count 4, then (0, 2, 12), (1, 5, 9), (2, 1, 1), (3, 4, 4),
then 0. The total weight is 12; entry 1 has probability 5/12.

<a id="s13-3"></a>
### 13.3 Barter entry

A list with two costs (0x70F00031 × 3; 0x70F00032 × 10) and a reward of 2 × 0x70F00040,
no sound category, inside profile 0x70F00030, encodes as the 49-byte entry:

| Offset | Value |
|---|---|
| 0 | 0x70F00030 |
| 4 | 0x70F00040 |
| 8 | 2 |
| 12 | 0 |
| 13 | 0 |
| 17 | 0 |
| 21 | 0x70F00031, 3 |
| 29 | 0x70F00032, 10 |
| 37 | 0, 0 |
| 45 | 0 |

All integers little-endian. The first eight bytes are `30 00 F0 70 40 00 F0 70`.

<a id="s13-4"></a>
### 13.4 Look-up cost

A cost with `Barter_ItemQuantity_LookupTable` = a progression with minimum index 1 and
values 3, 3, 4, 5, 6; the reward item has `Item_Level` 3 and the reward struct has no
`UsesMinUsageLevelForCost`. Index = 3 − 1 = 2, quantity 4. With level 9 the index 8 is past
the end and clamps to the last element, quantity 6.

<a id="s13-5"></a>
### 13.5 Species key

Species key 35, subspecies key 5, examination type key 7: 7 × 2³² + 35 × 2²⁰ + 5 × 2⁴ =
0x0000000702300050.

<a id="s13-6"></a>
### 13.6 Order of a weighted treasure table's object form

Property array (trophy list, weight): (A, 5), (B, 3), (C, 5), (D, 3), (E, 3). Filling the
heap of §9.4.1 gives, after each step: [A]; [B, A]; [B, A, C]; [B, D, C, A] (D rises above
A, then stops below B because 3 is not greater than 3); [B, D, C, A, E]. Removing: B, leaving
[E, D, C, A] (E stays at the top: D's weight is not smaller than E's); E, leaving [D, A, C];
D, leaving [C, A]; C; A. The `refarray` order is B, E, D, C, A, and the class-175 table holds
the weights 3, 3, 3, 5, 5. A table whose four weights are all equal, (A, 100), (B, 100),
(C, 100), (D, 100), gives A, D, C, B.

<a id="s13-7"></a>
### 13.7 Purchase price

`Item_Value` 250, quantity 3, `Vendor_SellModifier` 4.0, a discount list of two PIDs for
which the buyer has the values 0.125 and 0.25. *d* = 1.0 − 0.125 − 0.25 = 0.625;
*m* = 4.0 × 0.625 = 2.5; price = truncate(250 × 3 × 2.5 + 0.5) = truncate(1,875.5) = 1,875.
With buyer values 0.5 and 0.5, *d* = 0.0, *m* = 0.0 is raised to 1.0 and the price is 750.

<a id="s13-8"></a>
### 13.8 Order of a vendor item list

Two inventories, [0x70F00010, 0x70F00011, 0x70F00027] and [0x70F00011, 0x70F00028,
0x70F00012]. Insertion sequence: 0x70F00010, 0x70F00011, 0x70F00027, 0x70F00028,
0x70F00012 (the second 0x70F00011 is dropped). Five DIDs stay below 2 × 23, so *B* = 23
throughout. Buckets (DID mod 23): 0x70F00010 → 2, 0x70F00011 → 3, 0x70F00027 → 2,
0x70F00028 → 3, 0x70F00012 → 4. Each new DID goes to the front of its chain: bucket 2 holds
0x70F00027, 0x70F00010; bucket 3 holds 0x70F00028, 0x70F00011; bucket 4 holds 0x70F00012.
`Vendor_ItemList` = 0x70F00027, 0x70F00010, 0x70F00028, 0x70F00011, 0x70F00012.

---

<a id="s14"></a>
## 14. Open questions (summary)

Behaviour that only the server decides (§11; none of it is in the client, §11.7): the
trophy roll, group drops and `Inventory_MaxQuantity` on trophy lists (§11.2); the
random-number generator and the descent order of weighted picks, runic pull counts (§11.3);
level and filter selection and species-table precedence (§11.4); whether the critical roll
uses the chance the panel shows, how experience is credited, and the unit of the execution
time (§11.5); the price a vendor pays for an item, money drops and the unit of the money-drop
amounts (§11.6); the role of `Barter_ItemMungeLevelOverride` (§6.2). Each of these OPENs
names the owner observation that can settle it where one exists.

Data questions:

> **OPEN:** The fifth attribute of class 1233 (0 in all 77,745 trophy entries) and
> attribute 5 of the profession object (a tier map whose values are all 0); probably mirrors
> of properties that no record sets (for example `CraftProfession_TierOpeningQuestDID`).
> What the class library shows (WL): class 1233 only sets the fifth attribute to 0 in its
> initializer and never reads it. Profession attribute 5 has an accessor (class 1023
> function 117), and its one caller (class 263 function 66) reads the entry for tier *t* + 2,
> where *t* is the character's value of the property named by
> `CraftProfession_ProficiencyLevel_PropertyName`, and only when the character's value of the
> property named by `CraftProfession_ProficiencyXP_PropertyName` has reached the
> `ProficiencyXPCap` of tier *t* + 1 (otherwise it gives 0). Which property the attribute
> mirrors is not shown.

> **OPEN:** Which of profession attributes 6 and 8 mirrors `AutoCompleteProficiency` and
> which `AutoCompleteMastery` (both are 1 everywhere). No class-library code reads either
> attribute, and neither name hash occurs as a constant in the executable.

> **OPEN:** The position of `Agent_Class` in the PVP map key (§9.5; 0 in all 15 entries) and
> whether the loot species maps ever use bits 0–3. The class library's own four-field key
> reserves bits 0–3 for a fourth field of at most 15 (§9.5.1), but the code that builds the
> loot maps' lookup keys is not in the client (§11.7), and the PVP key (`Agent_Species` ×
> 2¹⁶) does not follow that layout.

> **OPEN:** What the server does with the members of the set named by
> `LootGenControl_RewardOnceTGPs`, and what the dictionary property
> `TreasureGroupProfile_RewardOnce` (set on no record, read by no class-library code) is
> for. Facts (L8): class 3399 has one attribute, a `set_u32` (class 18) of DIDs, and no
> functions with a body other than its initializer. The install has six class-3399 records;
> five hold sets of 1,767, 767, 14, 2 and 2 DIDs (records of root classes 2532, 2541, 2651,
> 2776, 3052, 3054, 3109, 3509, 3731 and 3924; the store classes of §8 among them), and the
> one named by
> the WEENIECONTENT label `LootGenControl_RewardOnceTGPs` holds an **empty** set and an
> empty property set. With this install an implementation may treat the reward-once set as
> empty; the label suggests treasure group profiles that a character receives only once,
> which only matters if a later install fills the set.

---

<a id="s15"></a>
## 15. Notes for other specs

- `wstate.md` §5.3 lists the third word of a class-175 element as unknown and §14 there
  asks for the field meanings of class 1942. §9.2 and §9.3 of this spec answer both for the
  data in this install (W2, W3). The `wstate.md` owner may adopt them; this spec does not
  change `wstate.md`.
- The item spec owns the item root classes listed in §1 and the item value, level and
  usage properties used in §6.2 and §11.6.
- The progression spec owns the layout of class 1385 used in §6.2 and §4.11.
- The item spec owns `Item_Value`, which the purchase price of §11.6.1 reads, and the
  `ItemMunging_*` properties counted in §6.2.

---

<a id="changelog"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Census of all 33,380 loot, 7,817 crafting, 3,851 vendor and 5 currency records; property-form layouts; object-state mirror with the class-175 subtree-sum and class-1942 field meanings; look-up-table quantity rule; DID graph and cross-link algorithms; BIN rows B1 (recipe preview) and B2 (negative PID search). |
| 2026-10-07 | Review revision (status stays Review). Every disputed count re-measured over all records with a fresh probe. (a) OBS row, §1 and §3.4: the 383,343 object records are 383,332 with prefix 0x70 and 11 singletons with prefix 0x76; property sets exist only for the 0x70 records (383,332 at 0x79) plus 1,616 stand-alone 0x78 sets, and the characteristic-property and inbound counts cover all of them; no 0x76 singleton has a class of this spec. §3.4 counts unchanged (all matched). (b) §7: the "Reported by analogy" unit of the money-drop amounts removed; it is now an OPEN in §11.6 (and listed in §14). §11.6: the absence of `LootGen_Money*` properties now covers every property set, not only creatures and containers. (c) §9.2: Type column added (all three words u32). Also corrected in §9.2 and row W2: the index rule's 7,625 tables are not all of this spec's classes; they are 7,441 (1235: 6,418; 2372: 16; 3307: 1,007) plus 184 of classes 2063 (163) and 3044 (21); per-class table counts added. Cosmetic: §4.13 References column now holds each row's total, and the "plus the loot tables of §4.4–§4.10" clause of the 1237 row lists its six counts; every figure in §4.13 re-measured and unchanged. |
| 2026-10-07 | Second review revision (status stays Review). (d) §14, third data OPEN: the count was wrong as worded. Re-measured over all 1,007 class-3307 records with a fresh probe: 29 (not 27) match neither a stable nor a reverse-stable sort by weight; 2 of the 29 keep equal weights in ascending trophy-list DID order and 2 in descending DID order, leaving 25 that match none of the four orders (the earlier 27 excluded only the ascending-DID order). The breakdown (448 without ties; 518 stable, 12 reverse-stable, 29 neither) is now stated in §9.4. No other change. |
| 2026-10-08 | Revision (status Accepted → Revised): OPEN closure with fresh probes over all records (OBS2), the class library's code (WL, data) and one M4 question (BIN2). Closed: the order of the class-3308 array (§9.4.1, binary-heap rule, 1,007 of 1,007) and the order of `Vendor_ItemList` (§6.3.1, growing hash set, 1,439 of 1,439), removing the §6.3 OPEN and the third data OPEN of §14. Answered and narrowed: critical-success chance shown while crafting (§11.5.1; remaining OPEN: the server's roll); recipe experience value (§11.5.2; remaining: how it is credited); cooldown keys → CooldownControl durations in seconds (§11.5.3; remaining OPEN: execution time only); purchase price from a vendor (§11.6.1; remaining OPEN: the price a vendor pays). Added facts to the remaining OPENs: munge-override counts (§6.2), trophy-list description and icon in examination (§11.2), the class library's four-field key and classification filter (§9.5.1), profession attribute 5 accessor, class 3399 as a DID set with the reward-once set empty (§14). New §11.7 (functions declared without bodies), invariants E-I16–E-I18, examples §13.6–§13.8, provenance OBS2, WL, BIN2 and rows W5, V5, V6, R3, R4, R5, L7, L8, W6, V7. Owner observations that can settle the remaining behavioural OPENs are named in each. |
| 2026-10-08 | Merge of the decoder branch (spec writer, dirty room). The implementer OPEN notes and census differences of 2026-10-07 are kept; those that the Accepted text of the 2026-10-08 revision already answers are marked with a pointer: §6.2 (progression types: answered by `combat-data.md` §4.1–§4.3) and §9.5 (packed field widths: answered by §9.5.1); the munge-override OPEN of the base text is replaced by the Accepted §6.2 text. No fact changed; status unchanged. |
| 2026-10-09 | OPEN sweep (spec writer, dirty room; status Revised, waiting for review). New provenance key OBS3 and rows E1–E3. Answered the three implementer OPENs: §4.5/§4.8 type codes of the world-event condition fields (E1), §4.9 the stored `RunicLevelLootTable_Level` (E2), §6.2 `Usage_MinLevel` never stored in both places, top level first by decision (E3). Still OPEN (server behaviour or owner observation): the munge-level override, loot rolls, random generator and descent order, runic pulls, level index, filtered lists, species tables, critical crafts, craft XP, execution time, sale price, money drops and their unit, profession attributes, PVP key, reward-once set. |
