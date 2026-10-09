<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Player character data: avatar templates, character creation, names, levels and class resources

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room sessions, AI-assisted, not the author) |
| **Review record** | 2026-10-07: changes requested. 2026-10-07 re-review: passed (Accepted). 2026-10-08 revision (§3, §8.4, §8.5): review pending. 2026-10-08 review of the revision (spec reviewer, not the author): changes requested (one Confirmed fact of §8.5 contradicted on re-check); status stays Revised. 2026-10-09 revision (§8.2, §8.5, §19.6, P31): review pending. 2026-10-09 re-review (spec reviewer, not the author or an earlier reviewer): **passed**, Accepted. Details in the review log (decision 0018). |
| **Implements into** | `crates/props` / `crates/formats` (typed views of the records below, read from their property sets); `crates/wstate` (the meaning of native class 3053, §8.4); a later server or sandbox backend (`crates/net`: character creation, default property set, name generation, level and pip rules, §4.6, §8.5, §9, §10, §17); `apps/client` (the local player's default property set, §4.6); `apps/explorer` (race, nationality, class and avatar-template cards, §5–§7) |
| **Companion specs** | [`property-system.md`](property-system.md) §2–§5 (dictionary, value encodings, property sets), §6.2 (the +0x09000000 rule), §7 (enum, bitfield and PID display), §8 (DID maps); [`wstate.md`](wstate.md) §3–§5 (object-state records, the root class at offset 4, native codecs, in particular classes 11, 17, 18, 35, 39, 104 and 3053); [`wlib.md`](wlib.md) (class indices, ancestry, function IDs §6.4); [`records.md`](records.md) §7–§8 (enum mappers, DID maps); [`misc-records.md`](misc-records.md) §9 (name-filter table), §14 (WEENIECONTENT); [`creatures.md`](creatures.md) §2–§3 (template subtree, mirror properties); [`appearance.md`](appearance.md) §2–§3 (playable bodies, character-creation controls); [`skeleton-anim.md`](skeleton-anim.md) §2.2 (race-table walk); [`combat-data.md`](combat-data.md) §4 (progressions), §8.6 (trait control), §9.1 (class advancement tables), §10.3 (threshold policies); [`economy.md`](economy.md) §8 (store helpers); [`quests.md`](quests.md) §13.1 (factions); [`string-table.md`](string-table.md) §9 (resolving a StringInfo) |

---

<a id="purpose"></a>
## Purpose

The game data contains the records a server needs to create a player object with the
original's defaults, and the records the character-creation and character-sheet screens
read: the player-like avatar templates, the race and nationality tables, the class tables'
creation fields, the random-name generator, the experience tables and level controls, the
class resources ("pips"), and a set of account, bank, wardrobe, guild and trait-configuration
singletons. No spec before this one gave them field tables.

Nothing here needs a new byte decoder: every record decodes with the property-set decoder
(`property-system.md` §5) and the object-state decoder (`wstate.md`). This spec adds the
**meaning**, a **census over every record**, the **references between records**, and client
behaviour where the client executable settles it (rows marked BIN).

With it an implementer can:

- find every record of the group without hard-coded DIDs (§2);
- build a new character's default property set from the right avatar template (§4);
- validate a (race, sex, nationality, class) choice and show the creation screens' lists (§5–§7);
- reproduce the original's phonetic joining of name syllables and generate names (§8);
- look up levels from experience points and read the level controls (§9);
- show and drive class resources (§10);
- read the account, bank, wardrobe, guild, pedigree and allegiance controls (§11–§12).

The data does not contain the server's rules for most of these (how rested experience
accrues, how a pip drifts, how a guild leader is replaced). Where a rule is missing the spec
says so in an OPEN callout and, where Tessera's sandbox needs a rule anyway, gives a
**Decision** that is not a claim about the original.

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam, data current on 2026-10-07), 2026-10-07, with throwaway probes in the dirty scratch area built on a snapshot of the project's own decoders. **Every census covers all records of the classes named**: all 383,332 object records (prefix 0x70) were classified by root class; every record of every class in §2.1 was decoded on both halves (0x70 and 0x79); reference censuses cover every DID value in every 0x79 and 0x78 property set of `client_gamelogic.dat` plus every DID map under the root DID map. Counts are observations of this install and must be read at run time. |
| LC | LotroCompanion `lotro-tools` and `lotro-core` source (third-party extraction tools, read for facts only; no code or prose copied): which properties their race, nationality, pip, starting-gear and experience-table extractors read, and how they index an experience table. Marked **Reported** unless the data confirms it. |
| BIN | **M4 under decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, `lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-07 with a headless disassembler/decompiler in the dirty room (a copy of the shared analysis project, deleted afterwards), by address range only. **Questions:** (b1) does native code read the property IDs of the race, nationality, name-generation, name-rule, pip and control records of this spec (instruction-immediate search for 77 property IDs, two of which, 0xA9 and 0x630, are too small to search meaningfully; string search); (b2) how the client builds and applies a phonetic-shift table (trimming, case, which entry wins, what an unresolved replacement means); (b3) how the client computes a pip's effective maximum. **Why M1–M3 were not enough:** the shift tables contain overlapping entries stored in no consistent order (20 of 233 overlapping pairs are stored general-first), 526 replacement strings name string-table tokens that do not exist, and one match string carries trailing white space, so the data alone cannot say which entry wins or what a missing string means; the pip records name a "maximum modifier" property without saying how it combines; no published source states either (M2); no running client is available to this session (M3). **Results:** (b1) the IDs occur as immediates in the shift-table loader (§8.4), the pip routines (§10.4) and some user-interface routines that were not analysed (character-creation stage and action, race key, gender list, selection entity and cameras, nationality list, account-type restriction entries, outfit minimum level, pip texts and threshold index); the syllable, category, option and race-to-category property IDs occur nowhere except two of them in user-interface code; no string in the executable contains the five-letter placeholder of §8.2; (b2) answered in §8.4; (b3) answered in §10.4. The generator's own procedure (choosing options and syllables) runs in the class library's script bytecode and was not read in this pass (it was read on 2026-10-08, WLB). Nothing from network, login, launcher or anti-tamper code was read. |
| WLB | **Class-library script code** of the install, read on 2026-10-08: the bytecode chunk of record 0x56000000 (`client_gamelogic.dat`) disassembled in the dirty room with the rules of [`wlib-bytecode.md`](wlib-bytecode.md) §2–§6. This is data of the install (method M1), not the executable. Read: the bodies of class 3044 (slots 0, 50, 84–86, 89, 90, 94–96), class 3047 (slots 0–2) and class 3048 (slots 0–2, 4–7) of this install, and a sweep of all 18,323 bodies for native calls of class 3053. Behaviour is restated in words; nothing of the code is reproduced. |
| BIN2 | **M4 under decision 0011, second pass.** Same client build 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-08 in a private copy of the shared analysis project (headless decompiler, deleted after use) and with an import-table and call-site scan of the read-only executable, by address range only. Natives were located with the registration pairing of `wlib-bytecode.md`'s BIN row. **Questions:** (b4) what the native functions called by the generator's script code (WLB) do: the weighted pick and its reset, the integer random range, the string case changes, substring and comparison, and the arguments of the shift application; (b5) how the client's character-creation code uses the generator (attempts and checks); (b6) how the shift rule compares and counts characters (string type, comparison routine, locale). **Why M1–M3 were not enough:** the script code passes values to these natives by function ID only; their behaviour, the calling user-interface code and the locale exist only in the executable; no published source describes them (M2); no running client is available to this session (M3). **Results:** §8.4 (text model) and §8.5. The name check that the creation screen calls was identified as a call only; its rules were not read. Nothing from network, login, launcher or anti-tamper code was read. |
| DEC | Tessera's own choices where neither data, client nor published sources say anything. Each is marked **Decision** and is not a claim about the original. |

Marks: **Confirmed** = observed in the user's install over every record of the kind named, on
the date given; **Reported** = published third-party material only; **BIN only** = read from
the client executable under M4; **Decision** = a Tessera choice.

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| P1 | The 33 root classes of §2.1 hold 224 records (784,978 bytes in 0x70, 1,560,674 bytes in 0x79); the further classes of §2.1 hold 81 more; every record decodes completely on both halves, with the native codec of `wstate.md` §5.4 for class 3053 (§2.1, §3) | OBS | Confirmed 2026-10-07 |
| P2 | WEENIECONTENT labels and the records they name (§2.2) | OBS (DID map 0x2800000E) | Confirmed 2026-10-07 |
| P3 | Avatar templates: 69 + 69 records in 69 pairs; who references each (race tables 14 pairs, monster-play tables 54 pairs, PvP control 1 pair); every record has exactly one kind of referrer (§4.1) | OBS | Confirmed 2026-10-07 |
| P4 | Pair differences between classes 1653 and 1654 (§4.2) | OBS (all 69 pairs) | Confirmed 2026-10-07 |
| P5 | Property-set composition: 355 mirror names identical in all 138 sets, 47,886 mirror values equal to their base and 1,104 with no base; class-constant and varying counts (§4.3) | OBS | Confirmed 2026-10-07 |
| P6 | The 14 race avatars of class 1654 differ in exactly six properties; their class-constant defaults (§4.4) | OBS | Confirmed 2026-10-07 |
| P7 | Avatar `Agent_Species` equals the race table's `RaceTable_Race` and `Sex` equals `RaceTable_GenderType`, except the second gender entry of one race (§4.4, §5.3) | OBS (all 30 avatar references) | Confirmed 2026-10-07 |
| P8 | Race-table layout and census; gender struct layout; the race directory's compiled half lists the 14 player and 14 admin avatars (§5) | OBS | Confirmed 2026-10-07 |
| P9 | Nationality-table layout and census; every nationality named by a race exists; one nationality is named by no race; naming-guideline sexes equal the race's genders (§6) | OBS | Confirmed 2026-10-07 |
| P10 | Class-table creation fields: starting inventory, advanced-start kits and tiers, class priority, class-to-trait map, pip configuration consistency (§7) | OBS | Confirmed 2026-10-07 |
| P11 | LC reads starting gear only where the entry starts equipped and its quantity is 1, and reads race traits, regeneration and the player avatar from these records (§7.1, §4.4) | LC | Reported |
| P12 | Name generator layout and census: 15 (race, sex) → category entries equal to the race tables' 15 gender entries; 21 categories, 64 options, 56 syllable tables (8,797 strings), 13 shift tables (1,289 entries); each syllable-table and shift-table definition holds exactly its key property (`RandomNameGeneration_SyllableTable_Key`, `RandomNameGeneration_PhoneticShiftsTable_Key`) and its list; one shift-table key used but undefined (§8.2) | OBS | Confirmed 2026-10-07 |
| P13 | Syllable strings: 1–7 letters, ASCII letters only, no empty string; the first table of every option holds capitalised strings and every later table lower-case strings (§8.2) | OBS (all 8,797 strings resolved through the string tables) | Confirmed 2026-10-07 |
| P14 | Shift tables: the client trims white space (U+0020, U+000A, U+000D, U+0009, U+000C) from both ends of the end and start matches, keeps an unresolvable replacement as the empty string, groups entries by match length, and tries longer end matches first, then longer start matches, comparing without regard to case; it applies at most one entry per join and replaces the matched end and start (§8.4) | BIN (b2); OBS (the 13 compiled class-3053 objects in the 0x70 half equal the 13 source tables under exactly these rules, 1,289 of 1,289 entries) | Confirmed (BIN + OBS) |
| P15 | Meaning of the fields of native class 3053 (§8.4) | BIN (b2); OBS (as P14) | Confirmed (BIN + OBS) |
| P16 | Name-rule control: four rule keys; the name-filter table holds rule sets for three of them, exactly the generator's three valid-name rules (§8.1) | OBS | Confirmed 2026-10-07 |
| P17 | Experience tables (class 849): data only in the 0x70 half, a non-decreasing list of 64-bit integers with element 0 = 0; who uses them (§9.2) | OBS (all 29 records) | Confirmed 2026-10-07 |
| P18 | Element *k* of an experience table is the total experience needed to be at level *k* (§9.2) | LC | Reported |
| P19 | XP globals, group modifier tables, rested-experience control, level-scaling, capped-growth and progression-gating controls: layouts (§9.1, §9.3–§9.7) | OBS | Layout Confirmed 2026-10-07; meanings are readings of the labels (OPEN where stated) |
| P20 | Pip records and control: layout, census, threshold bands (§10.1–§10.3) | OBS (all 25 pips, all 18 threshold policies) | Confirmed 2026-10-07 |
| P21 | Pip effective maximum (§10.4) | BIN (b3) | BIN only |
| P22 | Account, bank, wardrobe, outfit, colour controls: layouts and censuses (§11) | OBS | Layout Confirmed 2026-10-07; meanings are readings of the labels |
| P23 | Guild control, kinship themes, pedigree, allegiances: layouts, censuses, consistency (§12) | OBS | Confirmed 2026-10-07 |
| P24 | Trait-configuration control, configurations, group-to-nature and class-to-trait maps (§13) | OBS | Confirmed 2026-10-07 |
| P25 | Character and account state properties: defined in the dictionary but held by no record (§15) | OBS (every 0x79 and 0x78 set) | Confirmed 2026-10-07 |
| P26 | Reference graph of §16 | OBS | Confirmed 2026-10-07 |
| P27 | Avatar-template compiled halves: one import of DB type 69, 35 (1654) or 36 (1653) items, no link section and no available-property section, in all 138 (§4.7) | OBS (all 138 records) | Confirmed 2026-10-07 |
| P28 | Level-table directory and stat directory: property halves; compiled halves (class key → class table with 12 imports; property ID → stat object and key 1–7 → stat object with 7 imports); stat objects' properties; the two stat objects with empty property halves and an empty u32 array (§7.5, §7.6) | OBS (every record of classes 855, 1454, 1453, 1455, 1456, 1459, 1460, 3434, 3435) | Confirmed 2026-10-07 |
| P29 | Manager entities: class subtree, `WeenieType` key 1, `Entity_CreationType` key 8, `PhysObj`, `SeeMeVisibilityMask`, the ten repository version ints and two further version ints of 2407; compiled half one script object holding three null references (0xFFFFFFFF), no imports (§14) | OBS (both records) | Confirmed 2026-10-07 |
| P30 | Lists and value types in the records of this spec: no null entries; 111 list properties (100 distinct paths) over the 318 records of the classes of §2.1, 853 and 1486, every list with one element PID; the longest list has 463 elements (§3.1) | OBS (every record, 2026-10-08) | Confirmed 2026-10-08 |
| P31 | Generator procedure: category map, weighted option draw, uniform syllable draw with case-insensitive repeat rejection (at most 101 draws per part, counted separately for each part), joins with the option's shift tables tried in order until one applies, case normalisation, failure yields the empty string (§8.5) | WLB; BIN2 (b4) | Confirmed (WLB + BIN) 2026-10-08; draw limit re-checked 2026-10-09 |
| P32 | Creation screen: generate and check, at most 100 attempts (§8.5) | BIN2 (b5) | BIN only |
| P33 | Text model of the shift rule and of the generator's case changes: UTF-16 code units; the C runtime's case-insensitive comparison and case conversion under the locale the client sets at start-up (§8.4) | BIN2 (b6) | BIN only |
| P34 | The generator's compiled half equals its property half: per category the options in stored order, each with its syllable-table keys and shift-table keys in stored order, and a weighted table whose element *i* holds option *i*'s weight (21 of 21 categories) (§8.5) | OBS | Confirmed 2026-10-08 |
| P35 | Every shift-table text that resolves is ASCII (all four fields of all 1,289 entries; 526 replacement tokens absent, §8.2) (§8.4) | OBS (the resolved texts of the P12 census) | Confirmed 2026-10-08 |
| P36 | Native function 1 of class 3053 is called by exactly one script body (class 3048, slot 5 here); native function 3 by none (§8.4) | WLB (all 18,323 bodies) | Confirmed 2026-10-08 |
| D1 | Building a new character's default property set (§4.6) | DEC | Decision |
| D2 | The sandbox's random generator and name check inside the client's generation procedure; weights below 1 and empty totals (§8.5) | DEC | Decision |
| D3 | Class display order by class priority (§7.3) | DEC | Decision |
| D4 | Sandbox pip drift (§10.5) | DEC | Decision |
| D5 | List cap and type mismatches in Tessera's typed views (§3) | DEC | Decision |
| D6 | Case folding and counting beyond ASCII in the shift rule and the generator (§8.4) | DEC | Decision |

---

<a id="s1"></a>
## 1. Concepts and conventions

| Term | Meaning |
|---|---|
| **Object record** | A record with prefix 0x70 (`wstate.md`). Its **root class** is the u32 at payload offset 4. |
| **Property half** | The object's property set at DID + 0x09000000 (`property-system.md` §6.2). Every field table in this spec describes the property half unless it says otherwise. |
| **Compiled half** | The object-state record (0x70) itself. For the records of this spec it is a compiled copy of the property half (maps keyed by enum keys, arrays of DIDs); a reader takes values from the property half. Exceptions: the experience tables (§9.2) and the group modifier tables (§9.3) have an empty property half and keep their data only in the compiled half. |
| **Avatar template** | An object of class 1653 or 1654 (§4): a complete property set for a player-like entity. |
| **Race avatar** | An avatar template named by a race table's gender entry (§5.3). |
| **Control** | A singleton record named by a WEENIECONTENT label (§2.2). |
| **Name** | Property names, enum labels and DID-map labels quoted here are the install's own and are interoperability identifiers; enum labels are quoted only where a rule depends on them. Read them from the install; do not compile them in. One dictionary name is spelled `TraitConfiguraiton_TraitNature` (sic); a reader must use the spelling the dictionary has. |

Table conventions (as `economy.md` §3.1): *Type* is the dictionary type of the property
(`int`, `int64`, `f32`, `bool`, `enum`, `bf32`/`bf64`/`bfany` bitfields, `did`, `pid`,
`sinfo` = StringInfo, `str` = Latin-1 string, `struct`, `array`). *Present* is the number of
records (of the class named in the heading) that hold the property. Where a value is called
an observation ("here: 160") it is this install's value and is read at run time. Enum mapper
DIDs are those the dictionary names in this install; read the mapper the dictionary names.
Texts (`sinfo`, `str`) are game text and are never reproduced.

In the *Content* columns, a description of purpose ("store item that unlocks the class") is a
reading of the property's name, not a confirmed fact; counts, ranges, types, keys and targets
are Confirmed.

Units: the data rarely states units. Where a property name suggests one (seconds, days) the
spec says "by its name"; such readings are not facts and carry an OPEN where it matters.

---

<a id="s2"></a>
## 2. Census and how to find the records

<a id="s2-1"></a>
### 2.1 Classes (Confirmed, P1)

Every class below has the ancestry 486 → 89 → 485 → 484 (`wlib.md` §9) unless the last column
says otherwise. Class indices are build-specific (`wlib.md` §1); find records by the rules of
§2.3, not by these numbers.

**The task group (33 classes, 224 records).**

| Class | Records | 0x70 bytes | 0x79 bytes | What | Section |
|---|---|---|---|---|---|
| 1653 | 69 | 246,537 | 631,757 | Avatar template, administrator twin (ancestry 1730 → 512 → 1731 → 516 → 1729 → … → 1732, `creatures.md` §2.1) | §4 |
| 1654 | 69 | 244,536 | 629,760 | Avatar template, player (ancestry 1731 → 516 → 1729 → … → 1732) | §4 |
| 861 | 1 | 491 | 82 | Race directory | §5.1 |
| 860 | 8 | 11,910 | 9,155 | Race table (one per playable species) | §5.2 |
| 858 | 1 | 942 | 250 | Nationality directory | §6.1 |
| 857 | 29 | 7,548 | 8,888 | Nationality table | §6.2 |
| 3046 | 1 | 153 | 378 | Class priority control | §7.3 |
| 2194 | 1 | 196 | 90 | Name-rule control | §8.1 |
| 3044 | 1 | 253,776 | 259,546 | Random-name generator | §8.2 |
| 862 | 1 | 36 | 66 | XP globals | §9.1 |
| 1693 | 1 | 36 | 54 | Rested-experience control | §9.4 |
| 3802 | 1 | 8,624 | 102 | Level-scaling control | §9.5 |
| 3193 | 1 | 324 | 240 | Capped-growth control | §9.6 |
| 2296 | 1 | 36 | 42 | Progression gating | §9.7 |
| 2545 | 1 | 36 | 482 | Character-slot control | §11.1 |
| 2774 | 1 | 957 | 828 | Account-type restrictions | §11.2 |
| 2797 | 1 | 36 | 1,788 | Account bank repository (ancestry 2798 → 486) | §11.3 |
| 2799 | 1 | 36 | 66 | Character bank repository (ancestry 2798 → 486) | §11.3 |
| 4190 | 1 | 611 | 452 | Shared housing storage (ancestry 3738 → 2798 → 486) | §11.3 |
| 4191 | 1 | 57 | 54 | Standard housing storage (ancestry 3738 → 2798 → 486) | §11.3 |
| 2800 | 3 | 108 | 1,836 | Bank upgrade schedules | §11.3 |
| 2581 | 1 | 36 | 18 | Bank repository control | §11.3 |
| 2085 | 1 | 663 | 1,282 | Outfit control | §11.5 |
| 3607 | 1 | 443 | 515 | Colour-pack control | §11.6 |
| 3627 | 1 | 657 | 2,268 | Appearance colour control | §11.7 |
| 1165 | 1 | 344 | 990 | Guild control | §12.1 |
| 370 | 9 | 3,537 | 3,546 | Kinship theme | §12.2 |
| 375 | 1 | 121 | 294 | Pedigree control | §12.3 |
| 4105 | 1 | 501 | 492 | Allegiance control | §12.4 |
| 4106 | 7 | 252 | 3,010 | Allegiance | §12.4 |
| 3580 | 1 | 152 | 82 | Trait-configuration control | §13.1 |
| 3572 | 5 | 1,223 | 1,620 | Trait configuration | §13.2 |
| 2407 | 1 | 63 | 641 | Repository manager (an entity, ancestry 788 → 1733 → … → 484) | §14 |
| **Total** | **224** | **784,978** | **1,560,674** | | |

**Further classes this spec describes (19 classes, 81 records).**

| Class | Records | 0x70 bytes | 0x79 bytes | What | Section |
|---|---|---|---|---|---|
| 1681 | 1 | 320 | 266 | Pip control | §10.1 |
| 1380 | 25 | 43,122 | 12,544 | Pip (class resource) | §10.2 |
| 855 | 1 | 499 | 114 | Level-table directory | §7.5 |
| 849 | 29 | 10,431 | 174 | Experience table | §9.2 |
| 852 | 3 | 747 | 18 | Group modifier table | §9.3 |
| 3322 | 1 | 148 | 142 | Advanced-start control | §7.2 |
| 3335 | 9 | 9,330 | 17,172 | Advanced-start kit | §7.2 |
| 3675 | 1 | 2,367 | 1,098 | Class-to-trait control | §13.4 |
| 3587 | 1 | 209 | 588 | Trait group to trait nature | §13.3 |
| 1454 | 1 | 654 | 74 | Stat directory | §7.6 |
| 1453, 1455, 1456, 1459, 1460, 3434, 3435 | 1 each | 139 / 139 / 139 / 139 / 139 / 59 / 59 | 214 / 190 / 214 / 190 / 190 / 6 / 6 | Stat objects (ancestry 1458 → 1452 → 486) | §7.6 |
| 2323 | 1 | 63 | 507 | Character-sheet manager (an entity, ancestry 788 → …) | §14 |
| 2639 | 1 | 2,946 | 3,066 | Wardrobe control | §11.4 |

Class 853 (12 class advancement tables) and class 1486 (trait control) are specified in
`combat-data.md` §9.1 and §8.6; §7 adds only their character-creation fields.

<a id="s2-2"></a>
### 2.2 WEENIECONTENT labels (Confirmed, P2)

The WEENIECONTENT DID map (0x2800000E, `misc-records.md` §14) names these singletons. DIDs
are observations.

| Label | DID here | Class | Section |
|---|---|---|---|
| `RaceTableDirectory` | 0x7000020F | 861 | §5.1 |
| `NationalityTableDirectory` | 0x70000210 | 858 | §6.1 |
| `LevelTableDirectory` | 0x7000020E | 855 | §7.5 |
| `StatDirectory` | 0x70000229 | 1454 | §7.6 |
| `ChargenClassPriorityControl` | 0x70022DC2 | 3046 | §7.3 |
| `AdvancedCharacterStartControl` | 0x7002BF09 | 3322 | §7.2 |
| `NameRulePropertyControl` | 0x7000F4D4 | 2194 | §8.1 |
| `RandomNameGenerationDirectory` | 0x70022DBA | 3044 | §8.2 |
| `XPGlobals` | 0x7000022A | 862 | §9.1 |
| `XPBonusControl` | 0x70000227 | 1693 | §9.4 |
| `LevelScalingControl` | 0x700368A3 | 3802 | §9.5 |
| `CappedGrowthPropertyControl` | 0x70027A6B | 3193 | §9.6 |
| `ProgressionGatingControl` | 0x700145F8 | 2296 | §9.7 |
| `PipControl` | 0x7000021F | 1681 | §10.1 |
| `CharacterSlotControl` | 0x7001AAF6 | 2545 | §11.1 |
| `AccountTypeRestrictionsControl` | 0x70021372 | 2774 | §11.2 |
| `Account` | 0x70021DEE | 2797 | §11.3 |
| `Character` | 0x70021DEF | 2799 | §11.3 |
| `SharedHousing` | 0x70057499 | 4190 | §11.3 |
| `StandardHousing` | 0x7005749A | 4191 | §11.3 |
| `BankRepositoryControl` | 0x7001B1BD | 2581 | §11.3 |
| `WardrobeControl` | 0x7001BE4C | 2639 | §11.4 |
| `OutfitControl` | 0x7000D66E | 2085 | §11.5 |
| `ColorPackControl` | 0x70031874 | 3607 | §11.6 |
| `AppearanceColorControl` | 0x700321CE | 3627 | §11.7 |
| `GuildControl` | 0x7000025F | 1165 | §12.1 |
| `PedigreeControl` | 0x7000026D | 375 | §12.3 |
| `AllegianceControl` | 0x7004A21F | 4105 | §12.4 |
| `TraitConfigurationControl` | 0x70030E03 | 3580 | §13.1 |
| `TraitGroupToTraitNature` | 0x70030DDA | 3587 | §13.3 |
| `ClassToTraitControl` | 0x700332A5 | 3675 | §13.4 |
| `TraitControl` | 0x7000025B | 1486 | `combat-data.md` §8.6 |
| `RepositoryManager` | 0x700187A9 | 2407 | §14 |
| `CharacterSheetManager` | 0x70015EE2 | 2323 | §14 |

<a id="s2-3"></a>
### 2.3 Finding the records (rule)

1. Resolve each control through its WEENIECONTENT label (§2.2).
2. Follow the references of §16 from the controls to the other records: race tables from the
   race directory, nationality tables from the nationality directory, avatar templates from
   the race tables (and from the monster-play tables and the PvP control), pips from the pip
   control, configurations from the trait-configuration control, allegiances from the
   allegiance control, bank upgrade schedules from the bank repositories, kinship themes from
   the guild control and the kinship charter items, advanced-start kits and experience tables
   from the class tables of the level-table directory.
3. Every record of the group is reachable this way (P26); no class index needs to be known.

---

<a id="s3"></a>
## 3. The common record model (Confirmed, P1)

- Every record of §2.1 has a property half, and both halves decode completely with the
  existing decoders (the name generator's compiled half needs the class-3053 codec of
  `wstate.md` §5.4).
- The compiled half of a singleton with no lists is 36 bytes: one script object of the root
  class with no attribute values.
- The compiled halves of list-holding controls hold maps keyed by enum keys or property IDs
  (codec 11 or 35), arrays of DIDs (codec 17) and per-entry property values (codec 39); they
  repeat the property half in another shape. Cross-checks are given where they help (§5.5,
  §8.4); otherwise a reader ignores the compiled half.
- Directory records whose lists name other object records (classes 855, 858, 861, 1454) carry
  an import list of DB type 69 entries for the named records (`wstate.md` §3.2).

<a id="s3-1"></a>
### 3.1 List sizes and value types (Confirmed P30; Decision D5)

This answers the implementer note of 2026-10-07.

- **What the client does.** The client's property-set reader bounds a list only by the bytes
  left in the record (`property-system.md` §4.11, §5.3); it has no count limit of its own.
- **What the data holds.** Over all 318 records of the classes of §2.1 and of classes 853
  and 1486 there are no null entries (none occurs in any record, `property-system.md` §5.2),
  and the longest list has 463 elements (a syllable
  table, §8.2); the next longest have 236 (trait control) and 200 (a shift table).
- **Types.** A value always has the type that the dictionary gives its PID
  (`property-system.md` §5.2), and in each of the 111 list properties of these records (counted per class and path) all
  elements carry one and the same PID. A typed view can therefore meet a type other than the
  one in this spec's tables only when another build's dictionary changes a definition, or
  in a hostile record. The client has no general rule for that case; the one reader of this
  spec read under BIN that meets a foreign type treats it as absent (§10.4, an actor's
  value, not a record).
- **Decision D5 (Tessera):** a typed view of these records rejects a list of more than
  65,536 elements and reports a present property whose dictionary type differs from the
  type in this spec's tables as an error. Both are Tessera choices, not client behaviour.
  The cap is more than 100 times the longest observed list.

---

<a id="s4"></a>
## 4. Avatar templates (classes 1653 and 1654)

This section answers the OPEN of `creatures.md` §2.1 ("what classes 1653 and 1654 are used
for").

<a id="s4-1"></a>
### 4.1 Kinds and referrers (Confirmed, P3)

Every avatar template is referenced from exactly one kind of place:

| Referrer | Property | Class-1654 records | Class-1653 records |
|---|---|---|---|
| Race table gender entry (§5.3) | `RaceTable_Gender_PlayerAvatar` / `RaceTable_Gender_AdminAvatar` | 14 (15 references) | 14 (15 references) |
| Monster-play table (class 1932, `combat-data.md` §9.2) | `MonsterPlay_MonsterAvatar` / `MonsterPlay_MonsterAdminAvatar` | 54 (67 references) | 54 (67 references) |
| PvP control (class 2708) | `PVPControl_Lobby_PlayerAvatar` / `PVPControl_Lobby_AdminAvatar` | 1 | 1 |
| **Total** | | **69** | **69** |

Each referrer names one record of each class; together they form 69 **pairs** (player,
administrator). No other 0x79 or 0x78 property set and no DID map names an avatar template;
`creatures.md` §6.10 found no generator profile that reaches them.

<a id="s4-2"></a>
### 4.2 The pair difference (Confirmed, P4)

Over all 69 pairs the administrator record (1653) and the player record (1654) differ only in:

| Property | Player (1654) | Administrator (1653) |
|---|---|---|
| `WeenieType` | key 0x1F (`Player`) | key 0x3F (`Admin`) |
| `Name`, `Inscription_Text` | a placeholder text | another placeholder text |
| `Usage_Permission` | a usage-permission object of class 1612 | one of class 1600 |
| `BaseRotTime` (f32) | 45.0 in all 69 | absent |
| `GOP_ProfanityFilterEnabled`, `Death_NeverSayDie`, `CloakingAppearanceID`, `Stance_CurrentStance` | absent | present in all 69 |
| `Admin_PlayerTestMode`, `Render_AdjustableScale` | absent | present in 1 and 2 |
| `Combat_Partial{Block,Evade,Parry}_Mitigation_Mod` and their `_Cached` mirrors | | differ in 57 pairs |
| `Physics_EtherealityType` / `Physics_EtherealToType` | | differ in 36 / 4 pairs |
| `Entity_AdditionalScriptTables` | | differ in 2 pairs |

A player character is built from the **player** record (1654). The administrator twin is a
staff variant; Tessera reads it only for completeness.

<a id="s4-3"></a>
### 4.3 Composition of the property set (Confirmed, P5)

| | 1653 | 1654 |
|---|---|---|
| Top-level properties per set | 804–811 | 801–807 |
| Distinct names over the class | 817 | 812 |
| Mirror properties (pattern rule of `creatures.md` §3.2.1) | 355 | 355 |
| Class-constant (same value in all 69) | 425 | 423 |
| Varying | 37 | 34 |

- The 355 mirror names are the same in all 138 sets. Of the 48,990 mirror values, 47,886
  equal their base and 1,104 (8 names × 138 sets) have no base in the set; none differs. A
  store treats them as `creatures.md` §3.2.1 says (recompute from the base; a mirror never
  stands for its base).
- The varying properties of class 1654 are: `Agent_Alignment`,
  `Agent_ChainProtectionEnabled`, `Agent_Classification`, `Agent_ClassificationFilter`,
  `Agent_Genus`, `Agent_ShouldCheckDiminishingReturns`, `Agent_Species`,
  `Appearance_ClassList`, `Attitude_Table`, `Character_SpeciesNameOverride`,
  `Combat_CanBlock`, `Combat_CanParry`, the three partial-mitigation modifiers,
  `Entity_AdditionalScriptTables`, `Entity_PlayerPhysicsScript`,
  `Health_UpdateThresholdPercent`, `Power_UpdateThresholdPercent`,
  `Movement_Effect_SpeedMultiplier`, `PVP_Hero_AwardedRankXP_Assist`,
  `PVP_Hero_AwardedRankXP_KillingBlow`, `PVP_IsPVPLobbyPlayer`, `PhysObj`, the three
  `Physics_*Ethereal*` bitfields, `ScriptTable_StartupScriptNames`,
  `Script_UseBossHitAnimPriority`, `Sex` and the four `Vital_*BaseRegen` floats. Class 1653
  adds `Admin_PlayerTestMode`, `Death_NeverSayDie` and `Render_AdjustableScale`.
- No avatar template carries a class (`AdvTable_Class` or any class key): the class comes
  from the class table chosen at creation (§7).

<a id="s4-4"></a>
### 4.4 Race avatars: the player defaults (Confirmed, P6, P7)

The 14 race avatars of class 1654 hold 801 top-level properties each (802 in one) and differ
from each other in exactly six properties:

| Property | Type | Meaning |
|---|---|---|
| `Agent_Species` | enum (mapper 0x2300000B, `SpeciesType`) | Equals the race table's `RaceTable_Race` |
| `Sex` | enum (mapper 0x23000010) | Equals the gender entry's `RaceTable_GenderType`, except the second gender entry of the race with two entries naming one avatar (§5.3) |
| `Agent_Genus` | bf32 (mapper 0x2300000A) | One bit set |
| `Agent_Classification` | did | A classification object (class 973, `creatures.md`) |
| `PhysObj` | did (0x47) | The avatar entity of `appearance.md` §2.1 |
| `Character_SpeciesNameOverride` | sinfo | Present in 1 of the 14 |

Everything else is identical in all 14 (441 class-constant names plus the 355 mirrors).
Selected class-constant values, all observations:

| Property | Value here | Remark |
|---|---|---|
| `Advancement_Level` | 1 | Level of a new character |
| `Vital_HealthPeaceBaseRegen`, `Vital_PowerPeaceBaseRegen` | 5.0 | Out-of-combat regeneration (unit OPEN) |
| `Vital_HealthCombatBaseRegen`, `Vital_PowerCombatBaseRegen` | 0.0 | In-combat regeneration |
| `Health_MaxLevel`, `Power_MaxLevel` | 0.0 | The base vitals per level are in the class table's level entries (`combat-data.md` §9.1) |
| `Combat_CanBlock`, `Combat_CanParry` | false | Not granted by the template (how a class grants them is not in these records) |
| `Agent_Alignment` | key 1 | Mapper 0x2300000D |
| `Agent_ClassificationFilter` | key 5 | Mapper 0x2300000F; label `Player` |
| `Entity_CreationType` | key 7 | Mapper 0x23000174 |
| `Inventory_MaxBackpackSlots`, `Inventory_MaxEquipmentSlots`, `Inventory_MaxOverflowSlots` | 75, 21, 20 | |
| `Inscription_MaxLength` | 400 | |
| `Usage_MinUsageDist` | 2.0 | World units |
| `BaseRotTime` | 45.0 | |
| `Movement_Effect_SpeedMultiplier`, `Movement_AccelerationMultiplier`, `Movement_TurnSpeedMultiplier`, `Movement_Effect_MountSpeedMultiplier` | 1.0 | No base speed is stored here |
| `LevelScaling_ScaledLevel` | 0 | See `game-panels.md` P4 |
| `Attitude_Table`, `CSM_ActionHandler`, `Usage_Action`, `Usage_Permission`, `Usage_TakePermission` | DIDs of object records | Same record in all 14 |
| `GameEvent_CombatEventTable`, `GameEvent_ConjunctionEventTable` | 0x0C000000 | `GameEvent_StealthEventTable` names another 0x0C record |
| `Entity_AdditionalScriptTables` | one entry, script table 0x07000000 | |

LC's race extractor reads the four regeneration floats from these records (P11, Reported).

<a id="s4-5"></a>
### 4.5 Monster-play and PvP avatars (Confirmed, P3)

The 54 monster-play pairs vary in 32 of the properties of §4.3 (species, genus,
classification, alignment 1/2/3, `Agent_ClassificationFilter` keys 1 or 5, attitude table,
appearance lists in 5, block and parry flags, partial mitigation 0 or 0.001, ethereality bits,
and in one record regeneration 50.0 and a startup script). They are used by the monster-play
tables of `combat-data.md` §9.2 and are not character-creation inputs. The PvP pair has `Sex`
key 0 and `PVP_IsPVPLobbyPlayer`.

<a id="s4-6"></a>
### 4.6 Building a new character's default property set (Decision D1)

The original builds the character on its server; the data shows the inputs but not the
procedure. Tessera's sandbox and the walker's local player do this:

1. Find the race table whose `RaceTable_Race` is the chosen species (§5) and in it the gender
   entry whose `RaceTable_GenderType` is the chosen sex.
2. Take the property set of its `RaceTable_Gender_PlayerAvatar` (class 1654) as the base.
   Drop mirror properties; recompute them on demand (`creatures.md` §3.2.1).
3. Set `Sex` to the chosen sex (this repairs the one gender entry whose avatar has the other
   sex, §5.3).
4. Add the class: from the class table of the chosen class (§7, `combat-data.md` §9.1) take the
   level-1 entry of `AdvTable_LevelEntryList` for base vitals and maximum stats, and the pip
   configuration (§10).
5. Add the nationality key and the choices of the appearance screen (`appearance.md` §3.4);
   these are character state (§15), not template data.
6. Give the starting inventory of §7.1.

> **OPEN:** Which further properties the original server writes at creation (current vitals,
> money, home location, starting position, titles), and in which order class and race
> values combine. Not in the data; M3 observation of a new character's property dump would
> settle the client-visible part.

<a id="s4-7"></a>
### 4.7 The compiled half (Confirmed, P27)

Each avatar template's 0x70 record has one import (DB type 69) and 35 (1654) or 36 (1653)
items, and no link section (`creatures.md` §3.1). Nothing in it is needed.

---

<a id="s5"></a>
## 5. Race tables (classes 861 and 860)

`skeleton-anim.md` §2.2 and `appearance.md` §2.1 use the gender-to-avatar walk. This section
gives the full layout.

<a id="s5-1"></a>
### 5.1 Race directory (class 861, Confirmed, P8)

| Property | Type | Content |
|---|---|---|
| `RaceTable_RaceTableList` | array of `RaceTable_RaceTable` (did) | 8 race tables, one per species, no repeats |

The list order is the directory's order (here: species keys 0x51, 0x17, 0x41, 0x49, 0x72,
0x75, 0x78, 0x7D).

<a id="s5-2"></a>
### 5.2 Race table (class 860, Confirmed, P8)

8 records, 18 or 19 properties each.

| Property | Type | Present | Content |
|---|---|---|---|
| `RaceTable_Race` | enum (0x2300000B) | 8 | The species key; unique |
| `RaceTable_GenderList` | array of `RaceTable_GenderStruct` | 8 | 2 entries in 7 records, 1 in 1 (§5.3) |
| `RaceTable_ClassList` | array of `RaceTable_Class` (enum 0x2300000E) | 8 | Allowed classes: 1, 8, 8, 9, 10, 11, 11, 11 entries (69 in all); every key has a class table (§7) |
| `RaceTable_NationalityList` | array of `RaceTable_Nationality` (enum 0x23000011) | 8 | Allowed nationalities: 1, 1, 1, 3, 5, 5, 5, 7 entries; 28 in all, each named by exactly one race |
| `AdvTable_RaceCharacteristic_List` | array of `AdvTable_Trait_Entry` | 8 | 7–11 entries (66 in all): `AdvTable_Trait_WC` (did, a trait of class 1477 (60), 1478 (3) or 1483 (3)), `AdvTable_Trait_Level` (int, 1–116), `AdvTable_Trait_Rank` (int, 0 in all) |
| `Travel_PlayerHomeLocationArray` | array of `Travel_PlayerHomeLocation` (did, class 1499) | 8 | 3 entries (4 in one record); travel destinations (`map-data.md` §8.3) |
| `RaceTable_StartingLocation`, `RaceTable_InitialHomeTelepad` | str | 8, 8 | Strings naming a start location and the first home travel point (counted in `map-data.md`); not reproduced |
| `RaceTable_StartingEncounter`, `RaceTable_BTS_StartingEncounter` | did (class 1368) | 8, 8 | Introduction encounters (encounter templates; class 1368 appears in `economy.md` §4.13) |
| `RaceTable_AllowSkipTutorial` | bool | 8 | true in 4 |
| `RaceTable_IsTallRace` | bool | 8 | true in 4 (`appearance.md` §2.1) |
| `RaceTable_AccomplishmentDirectory` | did (class 838) | 8 | Race deed directory (`quests.md`) |
| `RaceTable_UIScene` | did (0x21) | 8 | Creation-screen scene (`misc-records.md` §4) |
| `RaceTable_Chargen_Camera`, `RaceTable_Character_Select_Camera` | enum (0x230000B9) | 8, 8 | Camera keys of the creation and selection screens |
| `RaceTable_Description`, `RaceTable_MonsterPlayExaminationName` | sinfo | 8, 8 | Texts |
| `RaceTable_CharGen_ClassSelect_WebstoreItem` | did (class 2532) | 2 | Store item that unlocks the race (store records, `economy.md` §8) |

The dictionary defines 38 `RaceTable_` names; 37 occur. `RaceTable_StartingLocationList`
occurs in no record.

<a id="s5-3"></a>
### 5.3 Gender entry (`RaceTable_GenderStruct`, Confirmed, P7, P8)

15 entries, each with all 15 fields:

| Property | Type | Content |
|---|---|---|
| `RaceTable_GenderType` | enum (0x23000010) | 0x1000 (8 entries) or 0x2000 (7) |
| `RaceTable_Gender_PlayerAvatar` | did (class 1654) | The race avatar (§4.4) |
| `RaceTable_Gender_AdminAvatar` | did (class 1653) | Its administrator twin (§4.2) |
| `RaceTable_GenderNullAppearance` | did (0x20) | The empty appearance (`appearance.md` §2.1) |
| `RaceTable_RaceSelect_Entity` | did (0x47) | Selection-screen entity; differs from the avatar's `PhysObj` in all 15 |
| `RaceTable_RaceSelect_Icon`, `_LargeIcon`, `_SmallIcon`, `RaceTable_RaceSelect_Background` | did (0x41) | Images |
| `RaceTable_RaceSelect_Movie_DID` | did (class 1982) | Movie-library entry (`misc-records.md` §13.4) |
| `RaceTable_Chargen_Zoom_Camera`, `RaceTable_Character_Select_Zoom_Camera` | enum (0x230000B9) | Zoom camera keys (equal in each entry) |
| `RaceTable_Gender_Name`, `RaceTable_Gender_Desc`, `RaceTable_Gender_Race_Name` | sinfo | Texts |

The 15 entries name 14 distinct player avatars and 14 distinct administrator avatars: the
species with key 0x78 has two entries (0x1000 and 0x2000) that both name one avatar pair whose
`Sex` is 0x1000. Every other avatar's `Agent_Species` equals the race and its `Sex` equals the
entry's gender. The species with key 0x49 has one entry (0x1000 only).

<a id="s5-4"></a>
### 5.4 Validity of a creation choice (rule)

A (species, sex, nationality, class) choice is valid when:

1. a race table has `RaceTable_Race` = species and a gender entry with
   `RaceTable_GenderType` = sex;
2. the nationality key is in that race table's `RaceTable_NationalityList`;
3. the class key is in that race table's `RaceTable_ClassList`.

Store items, account tokens and access rights may hide a choice in the original
(`RaceTable_CharGen_ClassSelect_WebstoreItem`, `AdvTable_CharGen_ClassSelect_Required_Tokens`,
§7.4); a sandbox without accounts ignores them.

<a id="s5-5"></a>
### 5.5 Compiled half

The race directory's compiled half holds a map from species key to a per-race object and two
sets of u32 DIDs: one equals the set of the 14 player avatars, the other the set of the 14
administrator avatars (P8). It imports the 8 race tables. Readers use the property half.

---

<a id="s6"></a>
## 6. Nationality tables (classes 858 and 857)

<a id="s6-1"></a>
### 6.1 Nationality directory (class 858, Confirmed, P9)

| Property | Type | Content |
|---|---|---|
| `NationalityTable_NationalityTableList` | array of `NationalityTable_NationalityTable` (did) | 29 nationality tables, no repeats |

<a id="s6-2"></a>
### 6.2 Nationality table (class 857, Confirmed, P9)

29 records, 6–8 properties each.

| Property | Type | Present | Content |
|---|---|---|---|
| `NationalityTable_Nationality` | enum (0x23000011) | 29 | The nationality key; unique |
| `NationalityTable_Name`, `NationalityTable_Desc` | sinfo | 29, 29 | Texts |
| `NationalityTable_Icon` | did (0x41) | 29 | |
| `NationalityTable_Naming_Guideline_Array` | array of `NationalityTable_Naming_Guideline_Struct` | 29 | 1 or 2 structs (53 in all): `NationalityTable_Sex` (enum 0x23000010), `NationalityTable_Naming_Guideline` (sinfo, advice text for the name screen) |
| `NationalityTable_Title` | did (class 1470, a title) | 28 | Title for the nationality |
| `Reputation_FactionTable` | array of `Reputation_Faction` (did, class 1889) | 28 | 4 factions each (`quests.md` §13.1) |
| `AppearanceUI_DyeMapControlOverrides` | array of `AppearanceUI_DyeMapControlOverride` | 29 | 3 structs (2 in one record): `AppearanceUI_ControlPropertyName` (pid: `AC_Color_Hair`, `AC_Color_Skin`, `AC_Color_Eyes`) and `AppearanceUI_DyeMapImage` (did, 0x41) |

Rules and observations:

- Every nationality key named by a race table has a record; key 0x1 has a record but is named
  by no race (it is the one without title and factions and with two overrides).
- In every record the set of `NationalityTable_Sex` values equals the set of genders of the
  race that lists the nationality.
- A dye-map override replaces, for the creation screen, the palette image of the avatar's
  `AppearanceUI_DyeMapControl` whose `AppearanceUI_ControlPropertyName` is the same
  (`appearance.md` §3.4): the nationality restricts hair, skin and eye colours. That the override
  and the control name the same property is a data fact; the effect is a reading of the names.

> **OPEN (Q14):** What a dye-map override does on the creation screen: whether it replaces the
> control's palette image, restricts the choices to a subset of it, or only changes the default
> colour. Not settled by the data; M3 observation of the creation screen would settle it.

---

<a id="s7"></a>
## 7. Class data used at creation

The 12 class advancement tables (class 853) are specified in `combat-data.md` §9.1. This
section adds the fields that character creation and the character sheet read.

<a id="s7-1"></a>
### 7.1 Starting inventory (Confirmed, P10; LC reading P11)

`AdvTable_StartingInventory_List` in every class table: 4–22 entries (88 in all), each an
`AdvTable_StartingInventory_Entry`:

| Property | Type | Content |
|---|---|---|
| `AdvTable_StartingInventory_Item` | did | An item (classes 795 (67), 799 (17), 797 (2), 798 (2); `items.md`) |
| `AdvTable_StartingInventory_Quantity` | int | 1 in all 88 |
| `AdvTable_StartingInventory_StartsEquipped` | bool | true in 85 |
| `AdvTable_StartingInventory_RequiredRace` | enum (0x2300000B) | 0 (any race) in 70; a species key in 18 (three entries for each of six species) |
| `AdvTable_StartingInventory_ContainerSlot` | bf32 (0x230000EE) | Only in advanced-start kits (§7.2) |

Reading of the names (used by Tessera's sandbox, not confirmed for the original): a new
character of species *s* receives every entry whose required race is 0 or *s*; an entry that
starts equipped goes to its equipment slot (from the item's own slot data, `items.md`), the
others to the backpack. LC's extractor keeps only entries that start equipped with quantity 1
(Reported).

<a id="s7-2"></a>
### 7.2 Advanced start (classes 3322 and 3335, Confirmed, P10)

Advanced-start control (class 3322):

| Property | Type | Content |
|---|---|---|
| `AdvTable_AdvancedCharacterStart_TierStruct` | struct of `AdvTable_AdvancedCharacterStart_TierEntry` | One entry: `AdvTable_AdvancedCharacterStart_Tier` (enum 0x23000348, key 8), `AdvTable_Level` (int, here 20), `AdvTable_AdvancedCharacterStart_Tier_BillingTokenArray` (10 billing-token keys, mapper 0x230000C4) |

Each class table has `AdvTable_AdvancedCharacterStart_AdvancedTierCASI_List` with one entry
(`AdvTable_AdvancedCharacterStart_Tier` key 8, `AdvTable_AdvancedCharacterStart_CharacterStartInfo`
→ a kit). The 12 class tables name 9 distinct kits (class 3335, 9 records):

| Property | Type | Content |
|---|---|---|
| `AdvTable_StartingInventory_List` | array (as §7.1) | 25–29 entries (239 in all); `RequiredRace` 0 in all; quantity 1, 5 or 10; `StartsEquipped` true in 157; `AdvTable_StartingInventory_ContainerSlot` 0 (95) or one bit (16 distinct single bits; a kit uses 15 or 16 of them, two kits one bit twice) |
| `Travel_PlayerHomeLocationArray` | array of did (class 1499) | 18 each |
| `AdvTable_AdvancedCharacterStart_StartingEncounter` | did (class 1368) | |
| `AdvTable_AdvancedCharacterStart_InitialHomeTelepad` | str | |

An advanced start creates the character at the tier's level with the kit's inventory
instead of §7.1 (reading of the names).

> **OPEN (Q3):** The advanced-start procedure: whether the kit replaces or adds to the starting
> inventory of §7.1, how the tier's level is granted (experience, traits, skills), how the
> billing tokens gate the choice, and how the kit's container slots are used. Server logic; not
> in the data.

<a id="s7-3"></a>
### 7.3 Class priority (class 3046, Confirmed, P10)

| Property | Type | Content |
|---|---|---|
| `CharGen_ClassPriority_Array` | array of `CharGen_ClassPriority_Entry` | 12 entries: `CharGen_ClassPriority_ClassType` (enum 0x2300000E), `CharGen_ClassPriority_PriorityValue` (int 1–12, unique) |

The 12 class keys are exactly the classes of the 12 class tables.

**Decision D3:** Tessera lists classes on the creation screen in ascending priority value.

> **OPEN (Q2):** Whether the original uses `CharGen_ClassPriority_PriorityValue` for the display
> order, for the default selection, or for something else, and whether 1 is first or last. M3
> observation of the creation screen would settle it.

<a id="s7-4"></a>
### 7.4 Other creation fields of the class table (Confirmed)

| Property | Type | Present | Content |
|---|---|---|---|
| `AdvTable_CharGenBackground` | did (0x41) | 12 | Creation-screen image |
| `AdvTable_CharGen_ClassSelect_Movie_DID` | did (class 1982) | 12 | Movie-library entry |
| `AdvTable_CharGen_ClassSelect_Required_Tokens` | pid | 4 | Names a world property holding the account tokens that unlock the class (two expansion-access names occur) |
| `AdvTable_CharGen_ClassSelect_WebstoreItem` | did (class 2532) | 2 | Store item that unlocks the class |
| `AdvTable_ClassOutfitAllowedSlots` | bf32 (0x230000EE) | 12 | Equipment slots an outfit may cover (two values occur) |
| `AdvTable_StatDescriptionArray` | array of `AdvTable_StatDescriptionEntry` | 12 | 7 entries: `AdvTable_StatDescription_Stat` (enum 0x2300003E, keys 1–7) and a text |
| `AdvTable_InnateDefaultSkill` | did (skill) | 12 | 0 in one |
| `AdvTable_PerkInventory` | did (class 1796) | 12 | Perk vendor (`economy.md` §6.4) |
| `AdvTable_PipType` | enum (0x23000071) | 6 | The class's own pip (§10) |
| `PipConfig_Pip_List` | array of `Pip_Type` | 12 | 1 entry (6 tables) or 2 (6 tables); see §10.6 |
| `AdvTable_FellowshipMaker_AvailableRoles` | bf32 (0x2300033E) | 12 | Roles offered by the group finder |
| `XP_EarnedPointsToLevel_Table` | did (class 849) | 12 | The experience table (§9.2); one record for all 12 |

<a id="s7-5"></a>
### 7.5 Level-table directory (class 855, Confirmed, P28)

| Property | Type | Content |
|---|---|---|
| `AdvTable_LevelTableList` | array of `AdvTable_LevelTable` (did, class 853) | The 12 class tables |

Its compiled half maps class key → class table and imports the 12 tables.

<a id="s7-6"></a>
### 7.6 Stat directory (class 1454) and stat objects (Confirmed, P28)

| Property | Type | Content |
|---|---|---|
| `Stat_StatList` | array of `Stat_StatEntry` (did) | 7 stat objects of classes 1456, 1453, 1459, 1460, 1455, 3434, 3435 (all derive from 1452 through 1458) |

| Stat object property | Type | Present | Content |
|---|---|---|---|
| `Stat_PropertyName` | pid | 2 | The actor property holding the stat (here two primary stats) |
| `Stat_Type` | enum | 2 | The stat key |
| `WebStore_FilteringAttribute`, `WebStore_Stat_Trait_List` | enum; array of 20 trait DIDs (class 1477) | 5 | Store filtering data (`economy.md` §8) |

Two stat objects (3434, 3435) have empty property halves and an empty u32 array in the compiled
half. The compiled half of the directory maps a property ID to each stat object and a key
1–7 to each. The stat directory is a store and character-sheet helper; nothing in character
creation needs it.

---

<a id="s8"></a>
## 8. Names

<a id="s8-1"></a>
### 8.1 Name-rule control (class 2194, Confirmed, P16)

| Property | Type | Content |
|---|---|---|
| `NameRule_NameRuleArray` | array of `NameRule_NameRuleEntry` | 4 entries, each `NameRule_NameRule` (enum, mapper 0x230000B2, labelled `NameRules` in EMAPPER): keys 1, 2, 0x10000003, 0x10000006 |

- The name-filter table 0x0E200000 (`misc-records.md` §9) holds rule sets for keys 1, 2 and
  0x10000003, the same three keys as the generator's `RandomNameGeneration_ValidNameRules`
  (§8.2). Key 0x10000006 has no rule set.
- The compiled half holds, per rule key, an object of class 2177 with three integers: the
  rule key, 22 and 0 in all four. The dictionary defines `NameRule_PlayerNameLength` and
  `NameRule_GuildNameLength` (ints), which occur in no property set.

> **OPEN:** Whether 22 is a maximum player-name length and 0 a guild-name length (the class-2177
> attributes are unnamed hashes), and how a rule set of the name-filter table is applied
> (`misc-records.md` §9 OPEN). Name validation is server-side in the original.

<a id="s8-2"></a>
### 8.2 Random-name generator (class 3044, Confirmed, P12, P13)

One record (0x70022DBA here), five properties. All 13,953 texts are StringInfo references into
69 string tables (`string-table.md` §9); none is a literal.

**Race to category.** `RandomNameGeneration_RaceToCategoriesArray`: 15 entries of
`RandomNameGeneration_RaceToCategories_Entry`:

| Property | Type | Content |
|---|---|---|
| `NameGenerationCategory_Race` | enum (0x2300000B) | Species key |
| `NameGenerationCategory_Gender` | enum (0x23000010) | Sex key |
| `RandomNameGeneration_CategoryType` | enum (0x23000333, `RandomNameType`) | The category |

The 15 (species, sex) pairs are exactly the 15 gender entries of the race tables (§5.3). The
two entries of species 0x78 name one category.

**Categories.** `RandomNameGenerationRules`: 21 `NameGenerationCategory` structs:

| Property | Type | Content |
|---|---|---|
| `RandomNameGeneration_CategoryType` | enum (0x23000333) | Unique; 14 are reached from the race map above, 7 are not (their labels name monster-play classes) |
| `NameGenerationCategory_Options` | array of `NameGenerationCategory_Option` | 1 option (2 categories), 2 (15), 4 (2), 12 (2); 64 in all |

Option:

| Property | Type | Content |
|---|---|---|
| `NameGenerationCategory_Option_Weight` | int | 1–100; the sums per category are 5 (2 categories), 10 (13), 99 (1), 100 (4) and 103 (1), so weights are relative |
| `NameGenerationCategory_Option_SyllableTables` | array of `NameGenerationCategory_Option_SyllableTable` (enum 0x23000313) | 2 tables (31 options), 3 (29), 4 (4); the parts of the name in order |
| `NameGenerationCategory_Option_PhoneticShiftsTables` | array of `NameGenerationCategory_Option_PhoneticShiftsTable` (enum 0x23000314) | Exactly 1 entry in all 64 |

**Syllable tables.** `RandomNameGeneration_SyllableTableDefinitions`: 56
`RandomNameGeneration_SyllableTable_Definition` structs. Each struct holds exactly two
properties:

| Property | Type | Content |
|---|---|---|
| `RandomNameGeneration_SyllableTable_Key` | enum (0x23000313, `SyllableTableKey`) | The table's key; unique over the 56 |
| `RandomNameGeneration_SyllableTable_StringsList` | array of `RandomNameGeneration_SyllableTable_String` (sinfo) | 14–463 strings, 8,797 in all |

An option's `NameGenerationCategory_Option_SyllableTable` value is looked up as a
`RandomNameGeneration_SyllableTable_Key`. Every key used by an option
is defined and every definition is used. Every string resolves to 1–7 ASCII letters (no
empty string, no other character). The first table of every option holds strings with an
upper-case first letter, every later table strings with a lower-case first letter; so a
generated name is capitalised without any case change. Eleven strings occur twice within their
table (8 tables; a duplicate makes a string more likely under a uniform draw).

**Shift tables.** `RandomNameGeneration_PhoneticShiftsTableDefinitions`: 13
`RandomNameGeneration_PhoneticShiftsTable_Definition` structs. Each struct holds exactly two
properties:

| Property | Type | Content |
|---|---|---|
| `RandomNameGeneration_PhoneticShiftsTable_Key` | enum (0x23000314) | The table's key; keys 1–13, unique |
| `RandomNameGeneration_PhoneticShiftsTable_StructList` | array of `RandomNameGeneration_PhoneticShiftsTable_Struct` | 30–200 entries, 1,289 in all |

An option's `NameGenerationCategory_Option_PhoneticShiftsTable` value is looked up as a
`RandomNameGeneration_PhoneticShiftsTable_Key`. Each entry has four sinfo fields:

| Property | Meaning |
|---|---|
| `RandomNameGeneration_PhoneticShiftsTable_LeftString_EndMatch` | *E*: what the left part must end with (1–3 letters; one value carries a trailing space) |
| `RandomNameGeneration_PhoneticShiftsTable_RightString_StartMatch` | *S*: what the right part must start with (1–5 letters) |
| `RandomNameGeneration_PhoneticShiftsTable_LeftString_Replacement` | *L′*: replaces *E* (empty in 42; the token is absent from its string table in 220) |
| `RandomNameGeneration_PhoneticShiftsTable_RightString_Replacement` | *R′*: replaces *S* (empty in 67; absent in 306) |

Within a table no two entries have the same (*E*, *S*), comparing without regard to case. Two
options name shift-table key 14, which has no definition (they get no shifts, §8.4 step 1).
Three start matches are a five-letter upper-case placeholder word; no syllable starts with
it, so those entries never match (the client compares it literally, §8.4).

**Valid-name rules.** `RandomNameGeneration_ValidNameRules`: 3 entries
`RandomNameGeneration_ValidNameRule` (enum 0x230000B2): keys 1, 2, 0x10000003 (§8.1).

**Compiled half.** 9,135 items: per race a sex-to-category map, per category a class-3047 object
(its options as class-3048 objects and a weighted table of native class 175, §8.5), the syllable strings (codec 199 StringInfo items), 13 compiled shift
tables (native class 3053, §8.4) and the set of valid rules.

<a id="s8-3"></a>
### 8.3 Nationality naming guidelines

`NationalityTable_Naming_Guideline_Array` (§6.2) holds, per sex, an advice text shown on the
name screen. It does not drive the generator; the generator is chosen by (species, sex) only.

<a id="s8-4"></a>
### 8.4 Phonetic shifts: the client's rule (Confirmed (BIN + OBS), P14, P15)

The client builds one **compiled shift table** per shift-table definition (the generator's
compiled half stores the 13 results) and applies it at the join between two parts of a name. This answers the field-meaning OPEN of native class
3053 in `wstate.md` §5.4.

**The compiled table.** For every entry of the definition:

- *E*, *S*, *L′* and *R′* are resolved to text. A replacement whose token cannot be resolved
  (absent from its string table) counts as the empty string. (An unresolvable *E* or *S* makes
  the client report a failed load; it does not happen in this install.)
- Every character of the set U+0020, U+000A, U+000D, U+0009, U+000C is removed from both ends
  of *E* and of *S*. Replacements are kept as stored.
- The **left length** of an entry is the number of characters of its trimmed *E* plus one; its
  **right length** is the number of characters of its trimmed *S* plus one.

Native class 3053 stores the table grouped this way: outer key = left length; under each outer
key the distinct *E* texts (texts equal without regard to case form one group), each with an
inner table keyed by right length whose elements are the triples (*S*, *L′*, *R′*); `list_a` =
the distinct left lengths and `list_b` = the distinct right lengths of the whole table, each in
descending order (`wstate.md` §5.4 gives the byte layout). All 13 compiled tables of the install
equal their source definitions under these rules (1,289 of 1,289 entries; P14).

**Applying a table** to a left part *A* and a right part *B* (two texts; both may change):

1. If the option names no defined shift table, nothing changes.
2. An entry **matches** when *A* ends with its *E* and *B* starts with its *S*, both compared
   without regard to case. (An entry whose *E* is longer than *A*, or whose *S* is longer than
   *B*, does not match.)
3. Of the matching entries, the one with the longest *E* is chosen; among matching entries with
   that *E*, the one with the longest *S*. (In this install no two entries of a table have equal
   *E* and *S* without regard to case, so the choice is unique; the client otherwise takes the
   first in its grouping order.)
4. If an entry is chosen: *A* becomes *A* without its last |*E*| characters, followed by *L′*;
   *B* becomes *R′* followed by *B* without its first |*S*| characters. At most one entry is
   applied per join. The client reports whether an entry was applied.
5. If no entry matches, *A* and *B* are unchanged.

So a longer end match beats a shorter one even when the shorter one has a longer start match,
and stored order never decides. Replacements are inserted with their stored case.

<a id="s8-4-1"></a>
**Text model** (answers the implementer note of 2026-10-07; P33, P35, D6).

- The client holds these texts as UTF-16 strings. Every length of this section (|*E*|, |*S*|,
  the left and right lengths, "the last |*E*| characters") counts **UTF-16 code units**.
- "Without regard to case" is the C runtime's case-insensitive wide-string comparison,
  code unit by code unit. Its folding depends on the C locale, which the client sets for all
  categories at start-up from its language setting (one of four named Windows language
  locales; the C locale is not used). In such a locale letters beyond ASCII fold by the
  runtime's tables. The case changes of §8.5 step 5 use the same runtime and locale.
- Every text of the generator in this install is ASCII: the syllables (P13) and every shift
  text that resolves (P35). ASCII-only folding therefore gives the client's results on this
  install.
- **Decision D6 (Tessera):** count UTF-16 code units, and fold (or change the case of) each
  code unit by its simple one-to-one Unicode case mapping; a code unit with no one-to-one
  mapping, a surrogate included, stays as it is. On ASCII text this equals the client;
  beyond ASCII it approximates the Windows runtime and is not a claim about it.

In the client this is native function 1 of class 3053 (function ID 0x0BED0001, `wlib.md`
§6.4); the table is built by native function 3 (0x0BED0003) (BIN). Function 1 is called by
exactly one script function of the class library, the option's join step of §8.5; function 3
is called by no script function of this install, because the generator's compiled half
already holds the 13 built tables (WLB, P36).

<a id="s8-5"></a>
### 8.5 Generating a name (Confirmed (WLB + BIN2), P31–P34; Decision D2)

This section answers Q5 and the implementer note of 2026-10-07 on Decision D2.

The generator runs in the class library's script code (class 3044 and the classes of its
compiled half: 3047 for a category, 3048 for an option) and calls native functions of the
client. It reads the generator's **compiled half** (§8.2). In this install the compiled half
equals the property half for all 21 categories (P34), so a reader takes the values from the
property half.

**The client's procedure** for one name, given (species, sex) (P31):

1. **Category.** Look up (species, sex) in the race-to-category entries. Without an entry,
   generation fails.
2. **Option.** Draw one option of the category by weight. With *W* the sum of the category's
   weights, option *i* is chosen with probability *wᵢ* ÷ *W*; an option of weight 0 is never
   chosen. (The client draws an integer *r* uniformly from 1 to *W* inclusive and walks the
   weighted tree of `economy.md` §9.2; which option a given *r* selects follows the tree,
   not the stored order. The draw restores the table before each pick, so options are drawn
   independently for every name.) A category without options fails.
3. **Parts.** For each syllable-table key of the option, in stored order, draw one string:
   an index uniformly from 0 to *n* − 1 inclusive, *n* being the table's number of strings
   (a duplicate string counts twice), and resolve it to text. A drawn text that equals,
   without regard to case (§8.4), a part already drawn for this name is rejected and drawn
   again. At most **101 draws** are made for one part (the first draw and up to 100 redraws;
   the limit is counted separately for each part); if all 101 are rejected, or the key has no
   table, or the table is empty, generation fails.

   > **OPEN:** (implementer note, 2026-10-09) What the client does when a drawn syllable
   > string does not resolve to text (every syllable of this install resolves, PC9). Tessera
   > leaves unresolvable strings out of the table when it resolves the generator's texts, so
   > *n* counts only resolvable strings and such a string is never drawn (an engineering
   > choice; it cannot change a result on this install).
4. **Join.** Start with an empty name *A*. For each part *B* in order: apply the option's
   shift tables to (*A*, *B*) by §8.4, trying the option's
   `NameGenerationCategory_Option_PhoneticShiftsTable` keys in stored order and stopping at
   the first table that applies an entry (a key with no definition acts as a table without
   entries); then append *B* to *A*. So the shift sits between the name built so far and the
   next part, and at most one entry is applied per join. The join is made for the first part
   too, with *A* empty. By §8.4 step 2 an entry can then match only if its trimmed *E* is
   empty; every trimmed *E* of this install has 1–3 letters (§8.2), so here the first part is
   never shifted.

   > **OPEN:** an entry whose *E* is empty after trimming does not occur in this install, so
   > the client's result for it (whether it matches every *A*, the empty one included) was not
   > observed. Tessera applies §8.4 as stated: such an entry matches every *A*.
5. **Case.** Change the whole name to lower case, then its first character to upper case.
6. A failed generation yields the empty string.

**The creation screen** (BIN only, P32): the random-name action generates a name, passes a
lower-cased copy to the client's name check together with a rule selector held by the
screen, and repeats until a name passes, making **at most 100 attempts** in all (the first
and 99 repeats). If none passes it reports failure and leaves the last generated name. The
name check's rules were not read (Q4). The generator's `RandomNameGeneration_ValidNameRules`
are not read by the generation code (they are read by user-interface code that was not
traced). A second entry point takes the category from a game object instead of (species,
sex); it was not traced further.

**Decision D2 (Tessera):** the sandbox follows the client's procedure above with these
substitutions, which are Tessera choices:

1. Random numbers come from Tessera's own uniform generator (the client's generator is not
   specified here); draws that are uniform over the stated ranges satisfy the procedure.
2. The sandbox's own name check (§8.6) takes the place of the client's, with the same limit of
   100 attempts in all.
3. An option weight below 1 (none observed) is never drawn; a category whose weights sum to
   0 fails generation (the client's behaviour there was not established).

<a id="s8-6"></a>
### 8.6 Validating a typed name

The data holds the rule sets (`misc-records.md` §9) but not their semantics (OPEN there).
Tessera's sandbox applies its own rules. The original rejects names server-side; its
creation screen also runs a client-side name check on generated names (§8.5, BIN only),
whose rules were not read (Q4).

---

<a id="s9"></a>
## 9. Experience, levels and level controls

<a id="s9-1"></a>
### 9.1 XP globals (class 862, Confirmed)

| Property | Type | Here | Reading of the name |
|---|---|---|---|
| `XPGlobals_AbsoluteMaxLevel` | int | 160 | Highest character level |
| `XPGlobals_AbsoluteMaxItemLevel` | int | 999 | Highest item level |
| `XPGlobals_GroupXPModifierTable` | did (class 852) | | Group experience multipliers (§9.3) |
| `XPGlobals_EffectiveContribution_MaxLevelDistance` | int | 10 | Level distance limit for kill contribution |
| `XPGlobals_Contribution_AllowedLevelDifference` | int | 5 | Allowed level difference for contribution |

`XPGlobals_AbsoluteMaxLevel` equals the number of level entries in every class table
(`combat-data.md` §9.1: 160) and is below the length of the class experience table (§9.2).

<a id="s9-2"></a>
### 9.2 Experience tables (class 849, Confirmed P17; index rule Reported P18)

29 records. The property half is empty (6 bytes). The compiled half has two items: the root
object (one reference attribute) and a `array_u64` (codec 104, `wstate.md` §5.3) of 11–201
elements.

- In all 29 the first element is 0 and the elements never decrease; in 20 the second element is
  also 0.
- Holders: `XP_EarnedPointsToLevel_Table` (12 class tables, all naming the one 201-element
  record), `Reputation_Faction_AdvancementTable` (114 factions), `Allegiance_Advancement_Progression`
  and `Allegiance_Advancement_Progressions_Array` (14, §12.4), `Trait_Control_VirtueTierToExperience_AdvancementTable`
  (1), `GloryControl_*` (2), `ItemAdvancement_*` (4), `PVPControl_*_EarnedRankXP_Table` (2),
  `Mounted_Control_SteedXP_To_Level_Table` (1).

**Index rule (Reported, LC):** element *k* is the total experience needed to be at level (or
rank) *k*; element 0 is unused. A character with total experience *x* is at the highest level
*k* ≥ 1 whose element is ≤ *x*. Tessera uses this rule; it fits the data (element 1 is 0 in the
class table, so a new character is level 1).

> **OPEN:** The rule is not confirmed by the client. For tables whose element 1 is not 0 (the
> allegiance and some reputation tables) it implies rank 0 at zero points; whether the
> original counts ranks from 0 there is unknown.

<a id="s9-3"></a>
### 9.3 Group modifier tables (class 852, Confirmed)

3 records with empty property halves. The compiled half's second item is a `map_u32` (codec
11) of 24 entries: key 1–24 → a 32-bit float (bit pattern stored as u32). Holders:
`XPGlobals_GroupXPModifierTable`, `Reputation_GroupModifierTable` (class 1891),
`GloryControl_GloryModifierTable` (class 1670). In the XP table the factor grows from 1.0 at
key 1 and stays constant from key 6 on.

> **OPEN:** The key is read as the number of group members (Reported only by the holder names);
> whether the factor multiplies each member's share or the total is server logic.

<a id="s9-4"></a>
### 9.4 Rested-experience control (class 1693, Confirmed layout)

| Property | Type | Here |
|---|---|---|
| `XPBonusControl_DailyGlobalResetTime` | int | 10800 |
| `XPBonusControl_DailyRestPercentage` | f32 | 30.0 |
| `XPBonusControl_MaxRestPercentage` | f32 | 30.0 |
| `XPBonusControl_Multiplier` | f32 | 2.0 |

> **OPEN:** Units and rule (seconds after midnight in which time zone; percentage of what; when
> the multiplier applies). Server logic.

<a id="s9-5"></a>
### 9.5 Level-scaling control (class 3802, Confirmed layout)

| Property | Type | Content |
|---|---|---|
| `LevelScalingControl_ItemLevel_Progression_Default` | did (class 1389, a progression) | Default level → item level progression (`combat-data.md` §4) |
| `LevelScalingControl_Quality_To_ItemLevelProgression_Hash` | array of `LevelScalingControl_Quality_ItemLevelProgression_Pair` | 2 pairs: `LevelScalingControl_ItemQuality` (enum 0x23000067) and `LevelScalingControl_ItemLevel_Progression` (did, class 1389) |
| `LevelScalingControl_ScalingEffects` | did (class 2519, a progression) | |

The compiled half (8,624 bytes) holds the three progressions expanded to u32 arrays; readers
evaluate the progressions instead (`combat-data.md` §4).

<a id="s9-6"></a>
### 9.6 Capped-growth control (class 3193, Confirmed layout)

`CappedGrowthPropertyControl_Configuration`: 3 `CappedGrowthPropertyControl_PropertyInfo`
structs, each with:

| Property | Type | Content |
|---|---|---|
| `CappedGrowthPropertyControl_SoftCap` | struct | One property: the capped property itself with its soft cap value |
| `CappedGrowthPropertyControl_HardCap` | struct | The same property with its hard cap value (above the soft cap in all 3) |
| `CappedGrowthPropertyControl_SoftCap_Error`, `CappedGrowthPropertyControl_HardCap_Error` | enum (0x230000BF, error keys) | Messages when a cap is reached |

The property inside the two structs is the property that is capped (its PID is the struct
field's PID). Two of the three entries cap test properties; one caps a banked-time property of
the skirmish system.

<a id="s9-7"></a>
### 9.7 Progression gating (class 2296, Confirmed layout, P19)

| Property | Type | Here |
|---|---|---|
| `ProgressionGating_MonsterPlayAccessService` | enum (0x230003A6) | A travel service key |
| `XP1_LevelBar`, `XP2_LevelBar` | int | 50, 60 |

> **OPEN:** How the level bars gate progression (by name: level caps tied to expansion access).

---

<a id="s10"></a>
## 10. Class resources (pips)

`combat-data.md` covers the effects and skills that change pips and the class tables'
`PipConfig_Pip_List`. This section gives the pip records.

<a id="s10-1"></a>
### 10.1 Pip control (class 1681, Confirmed P20)

| Property | Type | Content |
|---|---|---|
| `PipControl_Directory` | array of `PipControl_Entry` (did, class 1380) | All 25 pips, no repeats |
| `UI_Examination_Tooltip_0Click_Element`, `UI_Examination_Tooltip_1Click_Element` | enum | Tooltip layout element |
| `UI_Examination_Tooltip_0Click_Layout`, `UI_Examination_Tooltip_1Click_Layout` | did (0x22) | 0x22000000 |

The compiled half maps `Pip_Type` key → pip DID.

<a id="s10-2"></a>
### 10.2 Pip (class 1380, Confirmed P20)

25 records, 8–13 properties each.

| Property | Type | Present | Content |
|---|---|---|---|
| `Pip_Type` | enum (0x23000071, `PipType`) | 25 | Unique; the key used by `PipConfig_Pip_List`, skills and effects |
| `Pip_ValueProperty` | pid | 25 | The actor property that holds the current value; unique per pip |
| `Pip_Min` | int | 25 | 0 (24) or 1; dictionary default −1 |
| `Pip_Max` | int | 25 | 1–1,000 |
| `Pip_Home` | int | 25 | 0 (18), 3, 5, 10, 20 (3), 25; Min ≤ Home ≤ Max in all 25 |
| `Pip_Name`, `Pip_Description` | sinfo | 25 | Texts |
| `Pip_ErrorType` | enum (0x230000BF) | 25 | Message when a skill's pip requirement is not met |
| `Pip_Adjustment_OutOfCombat`, `Pip_Adjustment_InCombat`, `Pip_Adjustment_WhileMoving`, `Pip_Adjustment_StandingStill` | struct | 19, 4, 2, 1 | Timed changes (§10.5) |
| `Threshold_Pip_IntegerValue_Policy` | struct | 18 | Effects per value band (§10.3) |
| `Pip_MaxModifier` | pid | 3 | Actor property that lowers the maximum (§10.4) |
| `Pip_Home_ModProperty`, `Pip_Cashout_Conversion_ModProperty` | pid | 1, 1 | Actor properties that modify home and cash-out |
| `Pip_LastValidSelectionProperty` | pid | 2 | Actor property; with `Pip_ResetOnSelectedEnemyChange` (bool, 2, true) the value resets when the selected enemy changes (reading of the names) |
| `Pip_Examination_Min_Icon`, `_Max_Icon`, `_Home_Icon` | did (0x41) | 2 each | Icons for the examination panel |

Adjustment struct:

| Property | Type | Content |
|---|---|---|
| `Pip_Adjustment_Amount` | int | 1 or 3 (out of combat; absent in 4 of 19, dictionary default 0), 1 or 3 (in combat), −50 or 1 (moving), 100 (standing still) |
| `Pip_Adjustment_Interval` | f32 | 0.5–10; seconds by its name; absent in 3 of the 19 out-of-combat structs (default 0.0) |
| `Pip_Adjustment_NextTriggerTimestamp` | pid | Actor property holding the time of the next change |
| `Pip_Adjustment_AmountModifier`, `Pip_Adjustment_IntervalModifier`, `Pip_Adjustment_InitialDelayModifier`, `Pip_Adjustment_EnabledFlag`, `Pip_Adjustment_AlwaysRequeueFlag` | pid | Actor properties that modify the change (1 each) |

<a id="s10-3"></a>
### 10.3 Threshold bands (Confirmed P20)

`Threshold_Pip_IntegerValue_Policy` holds `Threshold_Policy_CurrentStateIndexProperty` (pid,
unique per pip) and `Threshold_Policy_StateList`: 1–19 `Threshold_EffectScript_State` structs
with `Threshold_State_MinValue_Integer`, `Threshold_State_MaxValue_Integer` (int; absent in
some states, dictionary default 0 for both), `Threshold_State_EffectList` (0–3
`Threshold_State_EffectStruct`, each an `EffectGenerator_EffectID`) and, in one pip,
`Threshold_State_ScriptData` (lists of script IDs to run or stop on entering or leaving).

With absent bounds read as 0, in all 18 policies the states are in ascending order, each band
is [min, max] with min ≤ max, consecutive bands touch (min = previous max + 1), the first band
starts at 0 or 1, and the last band's max is ≥ `Pip_Max`. Reading (Reported by
`combat-data.md` §10.3 for the float form): the state whose band holds the current value is
the current state; its index is written to the state-index property and its effects apply.

> **OPEN (Q15):** Whether the original writes the current state's index to
> `Threshold_Policy_CurrentStateIndexProperty` at all (the write is a reading of the property's
> name), and if so whether the index counts from 0 or from 1 in stored order, and what is
> written when no band holds the value. Server logic; M3 observation of an actor's state-index
> value while its pip changes would settle the base.

<a id="s10-4"></a>
### 10.4 Effective maximum (BIN only, P21)

The client computes a pip's effective maximum for an actor as follows:

1. *M* = `Pip_Max` (−1 if absent).
2. If `Pip_MaxModifier` is absent or 0, the result is *M*.
3. Otherwise read the actor's value *v* of the property it names; if the actor has no such
   value, or the value is not an `int`, *v* = 0.
4. The result is min(*M*, *M* + *v*), raised to `Pip_Min` if it is below (`Pip_Min` read as −1
   if absent).

So the modifier can only lower the maximum. The minimum is `Pip_Min` as stored.

<a id="s10-5"></a>
### 10.5 Timed changes (OPEN; Decision D4)

> **OPEN:** How adjustments run: whether "out of combat" changes move the value toward
> `Pip_Home` (amounts are positive in all records, so a direction rule must exist), whether the
> interval restarts on each change, and how the modifier properties combine. Server logic; the
> client only shows the value.

**Decision D4 (sandbox):** every *Interval* seconds while the struct's condition holds (out of
combat, in combat, moving, standing still), move the value by *Amount* toward `Pip_Home` for
the out-of-combat struct and by the signed *Amount* for the others, clamped to [`Pip_Min`,
effective maximum].

<a id="s10-6"></a>
### 10.6 Which pips a class has (Confirmed P10)

`PipConfig_Pip_List` lists 1 or 2 `Pip_Type` keys per class table. All 12 lists hold one key
common to all classes; the 6 tables with `AdvTable_PipType` also hold that type, the other 6
hold only the common key. Every key named by a class or monster-play table has a pip record.

---

<a id="s11"></a>
## 11. Account, bank, wardrobe, outfit and colour controls

These records configure account-level services. A sandbox without accounts reads them for
the limits they state; meanings are readings of the names unless marked.

<a id="s11-1"></a>
### 11.1 Character slots (class 2545)

| Property | Type | Content |
|---|---|---|
| `CharacterSlot_AdditionalSlots_XP1` | int | 2 |
| `CharacterSlot_Additional_BillingToken_Array` | array of `CharacterSlot_BillingToken` (enum 0x230000C4) | 17 billing tokens that add slots |
| `CharacterSlot_WebStore_BillingToken_Array` | array of the same | 38 store tokens |

<a id="s11-2"></a>
### 11.2 Account-type restrictions (class 2774)

`AccountTypeRestrictions_AllEntriesByTier`: 5 `AccountTypeRestrictions_Entry` structs, one per
`AccountTypeRestrictions_AccountType` (enum 0x23000307, `BillingAccountType`; keys 1, 3, 5, 6,
7), each with all of:

| Property | Type | Content (range over the 5) |
|---|---|---|
| `AccountTypeRestrictions_NumCharacterSlots` | int | 2–12 |
| `AccountTypeRestrictions_Currency_Cap` | int | One value in all 5 |
| `AccountTypeRestrictions_MinLevelForGlobalChat` | int | 1–20 |
| `AccountTypeRestrictions_MinPlayedTimeForGlobalChat_WorldProperty` | pid | One of three world properties (the minimum played time lives in world state) |
| `AccountTypeRestrictions_Mail_MaxSendTargetsInInterval`, `AccountTypeRestrictions_Mail_SendTargetLimitInterval` | int | 1–20; 30–600 |
| `AccountTypeRestrictions_Chat_MaxTellTargetsInInterval`, `AccountTypeRestrictions_Chat_ConsecutiveTellTargetInterval` | int | 1–20; 30–300 |
| `AccountTypeRestrictions_Chat_RequestRateThreshold`, `_RequestRateIncrement`, `_RequestRateDecayValue` | f32 | 1.0 or 4.0; 1.0; 1.0 |
| `AccountTypeRestrictions_Chat_RequestRateDecayInterval` | int | 1–4 |

> **OPEN:** Units of the interval fields (seconds by their size) and the chat-rate algorithm.

<a id="s11-3"></a>
### 11.3 Bank repositories (classes 2797, 2799, 4190, 4191, 2800, 2581)

Common fields of classes 2797, 2799, 4190 and 4191:

| Property | Type | Content |
|---|---|---|
| `BankRepository_BaseCapacity` | int | Slots at the start: 0 (account), 30 (character), 0 (shared housing), 500 (housing) |
| `BankRepository_MaxCapacity` | int | 420, 360, 255, 500 |
| `Bank_PolicyProhibitionProperty` | pid | An item property that forbids storage here (one per repository) |
| `Bank_PolicyProhibitionError` | enum (0x230000BF) | Message |

Specific fields:

| Class | Property | Content |
|---|---|---|
| 2797 | `BankRepository_WebStore_BillingToken_Array` | 41 `BankRepository_WebStore_BillingToken_Struct`: `BankRepository_WebStore_BillingToken` (enum 0x230000C4), `BankRepository_WebStore_BankType` (enum 0x230002E7), `BankRepository_WebStore_NumSlots` (int) |
| 2799 | `BankRepository_UpgradeProgression` | did → class 2800 |
| 4190 | `BankRepository_HousingUpgradeArray` | 3 `BankRepository_HousingUpgradeEntry`: `BankRepository_UpgradeProgression` (did → class 2800) and `BankRepository_HousingUpgradeTypeArray` (14, 14 and 9 house-type keys of enum 0x23000257) |
| 2581 | `BankRepositoryControl_MaxChestNameLength` | int, 25 |

Upgrade schedule (class 2800, 3 records): `BankRepository_UpgradeProgression_Array` of 23, 13
and 17 `BankRepository_Upgrade_Struct`: `BankRepository_Upgrade_RequiredCapacity` (int; the
capacity before the step), `BankRepository_Upgrade_IncreasedCapacity` (int; slots added),
`BankRepository_Upgrade_Cost` (int; present only in the first 6, 4 and 6 steps). In all three
the increase is 15 in every step and each step's required capacity is the previous step's
required capacity plus its increase. The character bank's schedule runs from required capacity
30 (its base capacity) to 360 (its maximum); the two housing schedules run from 0 to 180 and
from 0 to 240. The first two housing entries use the 13-step schedule, the third the 17-step
one, whose last step ends at 255, the shared-housing maximum.

> **OPEN:** Whether "required capacity" is the capacity before the step (then the character
> bank's last step would pass its maximum, so the maximum must clamp) or the capacity reached;
> the cost of steps without `BankRepository_Upgrade_Cost` (free, or the last stated cost); the
> currency unit.

<a id="s11-4"></a>
### 11.4 Wardrobe control (class 2639)

| Property | Type | Content |
|---|---|---|
| `Wardrobe_ItemClothingColor_List` | array of `Wardrobe_ItemClothingColorExt_Bit` (bf64, mapper 0x230003F2) | 53 colours, each value with exactly one bit set |
| `Wardrobe_WebStore_AccountItem_Array` | array of `Wardrobe_WebStore_AccountItem_Struct` | 80: `Wardrobe_WebStore_NumSlots` (int), `Wardrobe_WebStore_AccountItemDID` (did, class 2532) |

The other 18 `Wardrobe_` names of the dictionary are character state (§15).

<a id="s11-5"></a>
### 11.5 Outfit control (class 2085)

| Property | Type | Content |
|---|---|---|
| `Outfit_MinLevel` | int | 1 |
| `OutfitControl_OutfitTypeVisibilityMap` | array of `OutfitControl_OutfitTypeVisibility` | 23: `OutfitControl_OutfitType` (enum 0x23000273, keys 0–22) → `OutfitControl_OutfitVisibilityProperty` (pid: the character property holding that outfit's slot-visibility bits) |
| `OutfitControl_ContainerSlotToCosmeticProperty_Map` | array of `OutfitControl_ContainerSlotToCosmeticProperty_Entry` | 6: `OutfitControl_ContainerSlot` (bf32, 0x230000EE, one bit) → `OutfitControl_CosmeticProperty` (pid) for the weapon slots |
| `OutfitControl_ContainerSlotToOOCVisibility_Map` | array of `OutfitControl_ContainerSlotToOOCVisibility_Entry` | 3: weapon slot → pid of an out-of-combat hide flag |
| `OutfitControl_CosmeticallyCompatibleEquipmentCategories_Array` | array of `Item_EquipmentCategory` (bf64, 0x2300004C) | 7 groups of equipment categories that may stand in for each other cosmetically |
| `WebStore_WebStoreAccountItemArray` | array of did (class 2532) | 20 store items |

<a id="s11-6"></a>
### 11.6 Colour packs (class 3607)

`ColorPack_Control_Packs_To_Colors_Array`: 11 `ColorPack_Control_Pack_To_Color_Stuct` (sic)
structs: `ColorPack_Control_ColorPack` (enum 0x23000395, keys 1–11), `ColorPack_Control_Colors`
(bfany over mapper 0x23000398: the colours the pack unlocks), `ColorPack_Control_ColorPack_WebstoreDataID`
(did, class 2541).

<a id="s11-7"></a>
### 11.7 Appearance colour control (class 3627)

`AppearanceColorControl_ColorMod_Array`: 75 `AppearanceColorControl_ColorMod_Struct`:
`AppearanceColorControl_Color` (enum 0x23000398; keys 0, 2–17 and 19–76, unique) and
`AppearanceColorControl_Mod_Value` (f32, 0.0–0.8). Mapper 0x23000398 is also the mapper of
the colour-pack bitfield, so the 75 keys are the dye colours.

> **OPEN:** What the modifier value does (an ordering or a hue offset). Not used by any record
> that this spec reads.

---

<a id="s12"></a>
## 12. Social controls

<a id="s12-1"></a>
### 12.1 Guild control (class 1165, Confirmed P23)

| Property | Type | Here |
|---|---|---|
| `Guild_MaxMembers` | int | 1000 |
| `Guild_GuildNameMaxLength`, `Guild_RankNameMaxLength`, `Guild_MotDMaxLength` | int | 32, 32, 250 |
| `Guild_MaxGuildCalendarEvents` | int | 20 |
| `Guild_RecruitmentTimeOut` | f32 | 30.0 |
| `Guild_DaysLeaderIsOfflineForSuccessorUsurp`, `…ForOfficerUsurp`, `…ForMemberUsurp` | int | 35, 45, 55 (days by name) |
| `Guild_LeaderRankName`, `Guild_NewMemberRankName` | sinfo | Texts |
| `Guild_DefaultTheme` | did (class 370) | A kinship theme (§12.2) |
| `Guild_RecreateGuildCharterDID`, `Guild_RecreateMPGuildCharterDID` | did (class 796, items) | Charter items |
| `Guild_RankList` | array of `Guild_RankEntry` | 4: `Guild_RankID` (int; stored order 1, 3, 2, 4), `Guild_RankLevel` (int 1, 5, 10, 20), `Guild_RankName` (sinfo), `Guild_RankPermissions` (bf32, mapper 0x2300015D) |
| `Kinship_LifespanRankArray` | array of `Kinship_LifespanRankStruct` | 10: `Kinship_LifespanRank` (enum 0x2300007B, keys 1–10), `Kinship_LifespanRank_RealTimeToGainRank` (int, 0–360, increasing), `Kinship_LifespanRank_MaxMembers` (int, 1000–1500, non-decreasing), `Kinship_LifespanRank_Permissions` (bf32, in 7), `Kinship_LifespanRank_MonsterPermissions` (bf32, in 1) |
| `Kinship_LifespanEpsilon` | int | 1800 |

Reading of the names: a guild ("kinship") gains lifespan ranks with real time, which raise the
member limit and unlock permissions.

> **OPEN (Q11):** The units of `Kinship_LifespanRank_RealTimeToGainRank`,
> `Kinship_LifespanEpsilon` and `Guild_RecruitmentTimeOut`, and what the epsilon is compared
> with. Neither the names nor the values settle them. Server logic; not in the data.

<a id="s12-2"></a>
### 12.2 Kinship themes (class 370, Confirmed P23)

9 records:

| Property | Type | Present | Content |
|---|---|---|---|
| `KinshipTheme_Name`, `KinshipTheme_Tooltip`, `KinshipTheme_Founder_Name`, `KinshipTheme_Founder_Description` | sinfo | 9 | Texts |
| `KinshipTheme_Icon` | did (0x41) | 9 | |
| `KinshipTheme_Founder_Title` | did (class 1470) | 9 | Title of the founder |
| `KinshipTheme_RankList` | array of `KinshipTheme_RankEntry` | 9 | 4 entries: `KinshipTheme_Rank_RankID` (int; stored order 1, 3, 2, 4, the same IDs and order as `Guild_RankList`), `KinshipTheme_Rank_Name`, `KinshipTheme_Rank_Description` (sinfo), `KinshipTheme_Rank_Title` (did, class 1470; absent for rank ID 4) |
| `KinshipTheme_IsMonsterPlay` | bool | 4 | true in all 4 |

A theme renames the guild's ranks. Referrers: `Guild_DefaultTheme` (1) and
`KinshipCharterItem_Theme` on 9 charter items (class 796), one per theme.

<a id="s12-3"></a>
### 12.3 Pedigree (class 375, Confirmed)

| Property | Type | Here |
|---|---|---|
| `Pedigree_Surname_LevelRestriction` | int | 15 |
| `Pedigree_Surname_Fee`, `Pedigree_RelationshipTermination_Fee` | int | 1000, 1000 |
| `Pedigree_TitleArray` | array of `Pedigree_TitleEntry` | 8: `Pedigree_Relationship` (enum 0x2300007C, keys 1–8, unique) → `Pedigree_Title` (did, class 1470) |

<a id="s12-4"></a>
### 12.4 Allegiances (classes 4105 and 4106, Confirmed P23)

Allegiance control (class 4105):

| Property | Type | Content |
|---|---|---|
| `Allegiance_Control_UIVisibilityLevelRequirement` | int | 20 |
| `Allegiance_Type_Array` | array of `Allegiance_Type_Entry` (did, class 4106) | 7 |
| `Allegiance_Advancement_Progressions_Array` | array of did (class 849) | 7 entries, all the same record |
| `Allegiance_Group_Table` | array of `Allegiance_Group_Entry` | 3: `Allegiance_Group_Name` (enum 0x23000436, keys 1–3), `Allegiance_Group_MinLevelToStart` (int), four images (`Allegiance_Group_Image_Normal`, `_Highlighted`, `_Ghosted`, `_Normal_Mouseover`, did 0x41), `Allegiance_Group_Array` (did list of the group's allegiances: 4, 1, 2) |

Allegiance (class 4106, 7 records, 14 properties each): `Allegiance_Name`,
`Allegiance_Description` (sinfo), `Allegiance_Groups` (enum 0x23000436), `Allegiance_MinimumLeveltoStart`
(int), `Allegiance_Advancement_Progression` (did, class 849), `Allegiance_DeedList` (30
`Allegiance_Deed_Entry` DIDs, class 1398 deeds; 210 distinct over the 7), `Allegiance_TravelSkill`
(did, a skill), `Allegiance_Image` and six `Allegiance_Tab_*_Icon` (did 0x41).

Consistency (all 7): the type array equals the union of the group arrays; each allegiance's
`Allegiance_Groups` and `Allegiance_MinimumLeveltoStart` equal its group's name and
`Allegiance_Group_MinLevelToStart`. Two allegiances name a different experience table from the
control's array, but the two tables have identical contents.

---

<a id="s13"></a>
## 13. Trait configuration

<a id="s13-1"></a>
### 13.1 Trait-configuration control (class 3580, Confirmed P24)

| Property | Type | Content |
|---|---|---|
| `TraitConfiguration_TraitConfigurationList` | array of `TraitConfiguration_TraitConfigurationListEntry` (did, class 3572) | 5 |
| `TraitConfiguration_CooldownChannel` | enum (0x2300006C) | Cooldown channel of a configuration swap |
| `TraitConfiguration_CooldownDuration` | enum (0x2300006D) | Its duration key |

<a id="s13-2"></a>
### 13.2 Trait configuration (class 3572, Confirmed P24)

5 records, one per `TraitConfiguration_Type` (enum 0x2300038C, keys 1–5).

| Property | Type | Content |
|---|---|---|
| `TraitConfiguration_Struct` | struct | All of: `TraitConfiguration_Type`; `TraitConfiguration_TraitNature_Array` (`TraitConfiguraiton_TraitNature` (sic) keys of mapper 0x23000057; 13 natures in the class configuration, 1 in the others); `TraitConfiguration_SaveSlot_Array` (7 `TraitConfiguration_SaveSlot`: `TraitConfiguration_SavedSlot_Index` 0–6, `TraitConfiguration_SavedSlot_RequiredBillingToken` (enum 0x230000C4; 0 or a token), `TraitConfiguration_SavedSlot_CanMithrilPurchase` (bool, in 10)); `TraitConfiguration_CurrentActiveSlot_PropertyName` (pid of the character property holding the active slot) |
| `TraitConfiguration_QuickslotPage` | enum (0x23000376) | In 2 records |

Slots 0 and 1 never need a token or a purchase. In the two mounted configurations (types 1,
2) slots 2–6 each need a distinct billing token; in the class and big-battle configurations
(types 3, 4) slots 2–6 carry `CanMithrilPurchase` = true; in the virtue configuration (type 5)
no slot has either.

<a id="s13-3"></a>
### 13.3 Trait group to trait nature (class 3587)

`Trait_Control_Group_To_Nature_Array`: 19 `Trait_Control_Group_To_Nature`: `Trait_Group` (enum
0x230002EE, keys 1–19, unique) → `Trait_Nature` (enum 0x23000057).

<a id="s13-4"></a>
### 13.4 Class to trait nature (class 3675)

`Trait_Control_ClassToTraitNature_Array`: 12 `Trait_Control_ClassToTraitNature_Struct`:
`Trait_Control_Class` (enum 0x2300000E; the 12 classes), `Trait_Nature` (the class's trait
nature) and `Trait_Control_Branches_Array` (6 `Trait_Control_Branch` keys of mapper 0x230003A1
each; 11 of the 12 classes share two keys, 0xC and 0x11; the other four of each class are its
own, and one class has six keys of its own). The compiled half maps each branch key to the
classes that have it.

---

<a id="s14"></a>
## 14. Manager entities (classes 2407 and 2323, Confirmed P29)

`RepositoryManager` (2407) and `CharacterSheetManager` (2323) are entities (class subtree 788,
`WeenieType` key 1, `Entity_CreationType` key 8, a `PhysObj` 0x47 record, hidden by
`SeeMeVisibilityMask`). Their property sets hold the generic entity defaults; 2407 adds ten
`*Repository_VersionNumber` ints (guild, guild member, calendar, calendar reply, event,
history, trait configuration, quick slots, custom slot names, paper items) and
`NeighborhoodPersistentNPC_RepositoryVersion`, `PVP_Leaderboard_TableVersion`. Their compiled
halves hold three null references. They are server-side bookkeeping objects; a client needs
nothing from them.

---

<a id="s15"></a>
## 15. Character and account state (Confirmed P25)

The dictionary defines many properties that belong to a live character or account and occur
in no record. A server keeps them in its own store; the data gives only their names, types and
enum mappers (`property-system.md` §2).

| Prefix | Defined | Occurring in any 0x79 or 0x78 set |
|---|---|---|
| `Character_` | 35 | 2: `Character_AppearanceInfo` (96 times, in the preset sets of `appearance.md` §3.4) and `Character_SpeciesNameOverride` (2 avatar templates) |
| `Account_` | 50 | 0 |
| `CharGen_` | 19 | 4 (the class-priority names, §7.3) |
| `Chargen_` | 1 | 1 (`Chargen_AvatarPresetsArray`, 8 preset sets) |
| `Wardrobe_` | 24 | 6 (§11.4) |
| `Outfit_` | 27 | 1 (`Outfit_MinLevel`); the 22 per-outfit visibility properties are named by §11.5 |
| `Pip_` | 113 | 41 |
| `TraitConfiguration` | 37 | 17 |

Examples of state the creation screens write (by name): `CharGen_Name`, `CharGen_Stage`
(enum 0x23000132), `CharGen_Action` (enum 0x23000133), `Character_SkinColor`,
`Character_HairColor`, `Character_EyeColor`, `Character_WarPaintColor` (colour type 16),
`Character_HasNotEnteredWorldBefore` (bool), `Character_PreviousRace`, `Character_PreviousGender`.

> **OPEN:** Which of these the client reads from the server's object description and which
> it only sends. That belongs to the protocol (gated, decision 0004).

---

<a id="s16"></a>
## 16. Reference graph (Confirmed P26)

All DID values of every 0x79 and 0x78 set of `client_gamelogic.dat` and every DID map were
scanned. No 32-bit integer value equals the DID of a record of this group. Every record of §2.1
is the target of at least one reference (all 29 experience tables included). References into
the group:

| Target | Referrer | Count |
|---|---|---|
| Race directory (861) | WEENIECONTENT | 1 |
| Race tables (860) | `RaceTable_RaceTableList` (861) | 8 |
| Avatar templates 1654 / 1653 | race gender entries; `MonsterPlay_MonsterAvatar` / `…AdminAvatar` (1932); `PVPControl_Lobby_PlayerAvatar` / `…AdminAvatar` (2708) | 15 + 67 + 1 each |
| Nationality directory (858) | WEENIECONTENT | 1 |
| Nationality tables (857) | `NationalityTable_NationalityTableList` | 29 |
| Pips (1380) | `PipControl_Directory` | 25 |
| Trait configurations (3572) | `TraitConfiguration_TraitConfigurationList` | 5 |
| Allegiances (4106) | `Allegiance_Type_Array`, `Allegiance_Group_Array` | 7 + 7 |
| Kinship themes (370) | `Guild_DefaultTheme` (1165); `KinshipCharterItem_Theme` (796) | 1 + 9 |
| Bank upgrade schedules (2800) | `BankRepository_UpgradeProgression` (2799; 4190 in housing entries) | 1 + 3 |
| Advanced-start kits (3335) | `AdvTable_AdvancedCharacterStart_CharacterStartInfo` (853) | 12 |
| Experience tables (849) | §9.2 | 150 |
| Group modifier tables (852) | §9.3 | 3 |
| Stat objects | `Stat_StatList` (1454) | 7 |
| Every control of §2.2 | its WEENIECONTENT label | 1 each |

References out of the group (beyond those in the field tables): traits (1477, 1478, 1483),
titles (1470), factions (1889), deeds (1398), travel destinations (1499), encounters (1368),
movies (1982), store items (2532, 2541), items (795–799, 802, 2814), progressions (1389, 2519),
effects (from pip thresholds: classes 734, 718, 767, 708, 715), images (0x41), entities (0x47),
appearances (0x20), UI scenes (0x21), layouts (0x22), script tables (0x07), event tables (0x0C).

---

<a id="s17"></a>
## 17. Sandbox use (algorithm)

A sandbox server that creates a character does, with Decisions D1–D4:

1. Read the controls of §2.2.
2. Offer species and sexes from the race tables (§5), nationalities from the race's list
   (§6), classes from the race's list in priority order (§7.3). Check a submitted choice with
   §5.4.
3. Offer a random name with §8.5 (shifts by §8.4); check a typed name with the sandbox's rules.
4. Build the property set with §4.6, add the starting inventory (§7.1) and the class's pips
   (§10.6) at their home values.
5. Use the class experience table (§9.2) and `XPGlobals_AbsoluteMaxLevel` (§9.1) for levels.
6. Take account limits (slots, bank capacities) from §11 with the sandbox's own account tier.

---

<a id="s18"></a>
## 18. Invariants

Each is Confirmed on all records of the kind in this install (P-rows) unless marked BIN.

- **PC1** Every record of §2.1 decodes completely on both halves.
- **PC2** Avatar templates come in 69 pairs; each record has exactly one kind of referrer
  (§4.1); within a pair only the properties of §4.2 differ.
- **PC3** The 355 mirror names are identical in all 138 avatar sets; no mirror differs from a
  base that is present.
- **PC4** The 14 race avatars differ only in the six properties of §4.4.
- **PC5** For every race gender entry, both avatars have `Agent_Species` = `RaceTable_Race`;
  their `Sex` equals `RaceTable_GenderType` in 14 of 15 entries (the exception is §5.3).
- **PC6** `RaceTable_Race` values are unique; every `RaceTable_Class` key is the
  `AdvTable_Class` of a class table; every `RaceTable_Nationality` key is the
  `NationalityTable_Nationality` of exactly one record, and no nationality is listed by two
  races.
- **PC7** For each nationality listed by a race, the set of `NationalityTable_Sex` values equals
  the race's set of genders.
- **PC8** The generator's 15 (species, sex) pairs equal the race tables' 15 gender entries;
  every category they name has rules; every syllable-table key used is defined.
- **PC9** Every syllable string is 1–7 ASCII letters; first-position tables are capitalised,
  later ones lower-case.
- **PC10** Within a shift table, (*E*, *S*) pairs are unique without regard to case after
  trimming.
- **PC11** Each class-3053 object equals its source definition under the compiled-table rules
  of §8.4, and its two length lists are strictly descending (BIN + OBS).
- **PC12** Experience tables start with 0 and never decrease.
- **PC13** Pip types and pip value properties are unique; Min ≤ Home ≤ Max; threshold bands are
  ascending, touching, start at 0 or 1 and reach `Pip_Max` (absent bounds read as 0).
- **PC14** Every `Pip_Type` named by a class or monster-play table has a pip record; the pip
  directory lists every pip once.
- **PC15** In each bank upgrade schedule, each required capacity is the previous one plus the
  previous step's increase.
- **PC16** Allegiance control and allegiance records agree (§12.4).
- **PC17** (BIN) A phonetic-shift application changes at most one end of *A* and one start of
  *B*, and only when both match.
- **PC18** The generator's compiled half equals its property half (P34): per category the
  options, their syllable-table and shift-table keys in stored order, and the weights.
- **PC19** Every syllable string and every resolvable shift text is ASCII (P13, P35).
- **PC20** (WLB + BIN) A generated name is empty or has an upper-case first character and
  lower-case further characters (§8.5 step 5), and no two of its parts, as drawn before the
  shifts, are equal without regard to case (step 3).

> **OPEN:** (implementer note, `crates/wstate/src/character`, 2026-10-09) PC18 cannot be
> checked element by element: the spec does not say which attributes of a class-3047
> (category) and a class-3048 (option) object hold the options, the syllable-table keys,
> the shift-table keys and the weighted table, nor how a category object is tied to its
> category key. The real-data census therefore checks a weaker form: the compiled half holds
> as many class-3047 objects as categories (21) and class-3048 objects as options (64), and
> its 21 weighted tables (native class 175) carry, as a multiset, the categories' weight
> sequences in option order. Which attributes hold these fields?

---

<a id="s19"></a>
## 19. Worked synthetic examples

These values are made up for tests; they are not taken from the install.

<a id="s19-1"></a>
### 19.1 Phonetic shift

Shift table (stored order):

| # | *E* (stored) | *S* | *L′* | *R′* |
|---|---|---|---|---|
| 1 | `a` | `e` | `a` | (empty) |
| 2 | `ia` | `e` | `i` | `ye` |
| 3 | `n ` (trailing space) | `d` | `n` | `nd` |
| 4 | `ia` | `el` | (token absent) | `al` |

Compiled: left lengths (descending) 3, 2; right lengths 3, 2. Group "ia" (left length 3) holds
#2 (right length 2) and #4 (right length 3, *L′* = empty); group "a" holds #1; group "n" (left
length 2, trimmed) holds #3.

- *A* = `Kalia`, *B* = `ero`: left length 3 first; group "ia" matches the end of *A*. Right
  length 3: #4 needs `el`, *B* starts `er`: no. Right length 2: #2 needs `e`: yes. *A* loses its
  last 2 characters and gains `i` → `Kali`; *B* loses its first character and gains `ye` →
  `yero`. Joined: `Kaliyero`.
- *A* = `KALIA`, *B* = `Elm`: case is ignored; group "ia" matches; right length 3: #4 (`el`)
  matches. *A* → `KAL` (the absent *L′* is empty), *B* → `al` + `m` = `alm`. Joined: `KALalm`.
- *A* = `Toran`, *B* = `dor`: left length 3: `an` is not `ia`. Left length 2: group "a" does not
  match (`n` ≠ `a`); group "n" (stored `n `, trimmed) matches. Its only triple, #3 (right
  length 2, `d`), matches. *A* → `Tora` + `n` = `Toran`; *B* → `nd` + `or` = `ndor`. Joined:
  `Toranndor`.
- *A* = `Bo`, *B* = `ra`: nothing matches; both unchanged; joined `Bora`.

<a id="s19-2"></a>
### 19.2 Pip effective maximum

`Pip_Max` = 20, `Pip_Min` = 0, `Pip_MaxModifier` names property *P*:

| Actor's *P* | Result |
|---|---|
| absent | 20 |
| int −4 | 16 |
| int +5 | 20 |
| int −30 | 0 (raised to `Pip_Min`) |
| a float | 20 (not an int, *v* = 0) |

<a id="s19-3"></a>
### 19.3 Experience lookup (Reported rule)

Table [0, 0, 100, 250, 450]: 0 points → level 1; 99 → 1; 100 → 2; 260 → 3; 10,000 → 4 (the
last level of the table).

<a id="s19-4"></a>
### 19.4 Creation check

Race table: species 0x10, genders {0x1000, 0x2000}, classes {0x20, 0x21}, nationalities {0x3}.
Choice (0x10, 0x2000, 0x3, 0x21) is valid; (0x10, 0x2000, 0x4, 0x21) fails rule 2; (0x10,
0x2000, 0x3, 0x22) fails rule 3; (0x11, …) fails rule 1.

<a id="s19-5"></a>
### 19.5 Threshold band

States [(absent, absent), (1, 3), (4, 7), (8, 8)] with `Pip_Max` 8 read as bands [0, 0], [1, 3],
[4, 7], [8, 8]: value 5 lies in the third state (index 2 counting from 0, 3 counting from 1).

> **OPEN (Q15):** Which of the two indices the state-index property holds is not settled (§10.3);
> a test must not assume either.

<a id="s19-6"></a>
### 19.6 Generating a name

Category with options (weights 3, 0, 2): option 1 is drawn with probability 3/5, option 2
never, option 3 with probability 2/5. Option 3 names syllable tables T1 = [`Dor`, `Bel`] and
T2 = [`dor`, `in`] and shift tables [key 9 (undefined), the table of §19.1].

- Part 1: `Dor` is drawn from T1. Join with the empty name: nothing applies; *A* = `Dor`.
- Part 2: `dor` is drawn from T2 and rejected (equal to `Dor` without regard to case); `in` is
  drawn and accepted. Join: key 9 has no table and applies nothing; the §19.1 table has no
  entry with *E* matching the end of `Dor`; *A* = `Dorin`.
- Case step: `dorin` → `Dorin`.

With parts `Kalia` and `ero` the second table applies entry #2 of §19.1, giving
`Kaliyero`; with parts `KAL` and `in` (no match) the case step gives `Kalin`. A part table
whose every string equals an earlier part makes 101 rejected draws, and the name is empty.

---

<a id="s20"></a>
## 20. Open questions (summary)

| # | Question | Where | Settles it |
|---|---|---|---|
| Q1 | Properties the server writes at creation beyond the template | §4.6 | M3 (property dump of a new character) |
| Q2 | Meaning of the class priority (display order or default) | §7.3 | M3 |
| Q3 | Advanced-start procedure | §7.2 | server logic |
| Q4 | Name-rule numbers (22, 0) and name-filter semantics | §8.1 | bytecode or server |
| Q5 | Generator procedure (option draw, syllable draw, join scope, retries) | §8.5 | **Answered 2026-10-08** (WLB + BIN2). Left open: the client's random generator and its name check (Q4) |
| Q6 | Experience index rule for tables whose element 1 is not 0 | §9.2 | client panels (M3) |
| Q7 | Group modifier key and use | §9.3 | server logic |
| Q8 | Rested-experience units and rule | §9.4 | server logic |
| Q9 | Level bars | §9.7 | server logic |
| Q10 | Pip timed changes | §10.5 | server logic; M3 |
| Q11 | Units of account, guild and kinship intervals; bank steps without cost | §11.2, §11.3, §12.1 | server logic |
| Q12 | Appearance colour modifier | §11.7 | M3 |
| Q13 | Which state properties the client reads | §15 | protocol (gated) |
| Q14 | Nationality dye-map override effect | §6.2 | M3 (creation screen) |
| Q15 | Whether the threshold state index is written, its base (0 or 1) and its value outside every band | §10.3, §19.5 | server logic; M3 |
| Q16 | The client's result for a shift entry whose *E* is empty after trimming (none in this install) | §8.5 step 4 | a future install holding such an entry; M3 |

---

<a id="s21"></a>
## 21. Notes for other specs

- `creatures.md` §2.1 OPEN (classes 1653, 1654): answered by §4.
- `wstate.md` §5.4 (native class 3053, field meanings OPEN): answered by §8.4.
- `appearance.md` §2.1 calls `RaceTable_Gender_AdminAvatar` "another avatar object": it is the
  administrator twin of §4.2.

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Census of the 33-class group (224 records) and 19 further classes (81 records) over all records; avatar templates (answers `creatures.md` §2.1 OPEN); race and nationality tables; class creation fields; name-rule control and random-name generator with the client's phonetic-shift rule (BIN + OBS; answers the field meanings of native class 3053); experience tables and level controls; pips with the effective-maximum rule (BIN); account, bank, wardrobe, outfit, colour, guild, kinship, pedigree, allegiance and trait-configuration controls; state-property census; reference graph; invariants PC1–PC17; synthetic examples. |
| 2026-10-07 | Answers the first review (status stays Review). §8.2: the definition structs' key properties `RandomNameGeneration_SyllableTable_Key` and `RandomNameGeneration_PhoneticShiftsTable_Key` named, each definition holds exactly key and list (P12 widened). OPEN callouts at the place of use for Q2 (§7.3), Q3 (§7.2), Q11 (§12.1) and Q14 (§6.2); new Q15 (threshold state index written or not, base, value outside every band) with callouts in §10.3 and §19.5 and a §20 row. Provenance: P19 covers §9.7; new P27 (§4.7), P28 (§7.5, §7.6), P29 (§14), each re-checked over every record of its classes. No fact changed. |
| 2026-10-07 | Re-review passed; status Accepted. No content change. |
| 2026-10-08 | Revision (status Revised; Accepted text changed), answering the implementer notes of 2026-10-07 and Q5. §3: client list bound and value types stated, new §3.1 text with census P30 and Decision D5 (cap 65,536, type mismatch is an error). §8.4: new text model (UTF-16 code units; the runtime's case-insensitive comparison under the start-up locale; P33, P35) with Decision D6 replacing the implementer's scalar-value and full-lower-case choice; the statement that native function 3 is called from script code is corrected (P36). §8.5 rewritten: the client's generation procedure (P31, P34) replaces most of Decision D2; changes against the old D2: shift tables tried in order until one applies (not only the first), repeated parts rejected (100 draws per part), case normalisation, failure yields the empty string, at most 100 attempts in all (not 21). §8.2 compiled-half sentence corrected (3047 = category, 3048 = option). §8.6 notes the client-side check. New provenance keys WLB and BIN2, rows P30–P36, D5, D6; D2 narrowed. Invariants PC18–PC20; example §19.6; §20 Q5 answered. |
| 2026-10-09 | Revision (status stays Revised), answering the review of 2026-10-08. §8.5 step 3, P31 and §19.6: a part gets at most **101** draws (the first and up to 100 redraws, counted per part), not 100; re-checked on the part-draw body with the operand order of `wlib-bytecode.md` §5.10 (compare-and-branch). §8.2: option-weight sums corrected to 5 (2 categories), 10 (13), 99 (1), 100 (4), 103 (1), 21 in all. §8.5 step 4: the join is made for the first part too; it never applies there only because no trimmed *E* is empty in this install; OPEN added for an empty trimmed *E* (§20 Q16). |
| 2026-10-09 | Re-review of the 2026-10-09 revision passed; status Accepted. No content change. |
