<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Combat data: skills, effects, traits, progressions and their tables

| | |
|---|---|
| **Status** | Accepted (2026-10-07) |
| **Last updated** | 2026-10-07 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room sessions, AI-assisted), not the author: first review, re-review and acceptance review, all 2026-10-07 |
| **Review record** | Acceptance review 2026-10-07 (third review session, not the author): **passed**, Accepted. Full record moved to the review log (decision 0018). |
| **Implements into** | `crates/props` (progression evaluation and modifier-entry reading, §4, §5); a future gameplay-data crate (skills, effects, traits, class tables); `apps/explorer` (skill, effect and trait cards, §15); `apps/dattool` |
| **Companion specs** | `property-system.md` (property sets, value types, DID maps, enum mappers), `wstate.md` (object records 0x70, their bundles), `wlib.md` (class library, parent map, method tables), `records.md` §7 and §8 (enum mapper and DID map layouts), `coordinates.md` (world units) |

---

<a id="purpose"></a>
## Purpose

Combat in the original game is driven by a group of game-object records in
`client_gamelogic.dat`: **skills**, **effects**, **traits**, **progressions** (level-to-value
curves), per-class **advancement tables**, and a handful of control and lookup objects. They
are ordinary object records (prefix 0x70, `wstate.md`) with their property sets (prefix 0x79,
`property-system.md` §6.2). Nothing in this spec needs a new byte decoder: every record decodes
with the existing property-set and object-state decoders. What this spec adds is the
**meaning**: which records belong to which family, which properties carry the gameplay values,
how records reference each other, how a progression turns a level into a number, and how a
stat-modifier entry changes a property.

With it an implementer can:

- classify every combat object and show skill, effect and trait cards (§2, §15);
- evaluate a progression at an index (§4);
- read a stat-modifier list and know which property it changes, how, and by how much (§5);
- read effect, skill and trait definitions, class tables, rating-curve tables and the DPS and
  value lookup tables (§6–§10);
- follow the reference graph between these records (§12).

What the data does **not** contain is the server's combat logic: the formulas that combine these
inputs into hit chances, damage and healing (§13). The spec says where that boundary lies.

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam, client data current on 2026-10-06), 2026-10-06 and 2026-10-07, with throwaway probes in the dirty scratch area built on the project's own decoders. **Every census in this spec covers all records of the family named**, not a sample: all 383,332 object records were classified, and all records of each family were decoded on both halves (0x70 and 0x79). Counts are observations of this install and must be read at run time. |
| LC | LotroCompanion `lotro-tools` and `lotro-core` source (third-party extraction tools; read for facts only, no code or prose copied): which properties their extractors read per family, their interpretation of a few operators and formulas. Marked **Reported** unless the data confirms it. |
| DS | `research/notes/data-semantics.md` §5.1 and §12 (class-index list and property naming conventions). |
| DEC | Tessera's own choices where the data, the client and published sources say nothing. Each is marked **Decision** below and is not a claim about the original game. |
| BIN | **M4 under decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, `lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-07 with a headless disassembler/decompiler in the dirty room, by address range only. **Questions:** (b1) does native client code read the progression, modifier-entry, skill or effect properties by property ID; (b2) does native code call the script functions of the progression, effect, skill or trait classes by a constant function ID (`wlib.md` §6.4); (b3) where is the script machine's bytecode dispatch, so that the client-side progression and modifier functions could be read. **Why M1–M3 were not enough:** the out-of-range behaviour of progressions and the order in which modifiers combine are not stored in any record; no published source states them with authority; no running client is available to this session (M3). **Results:** (b1) no: 10 property IDs of §4 and §5 and the 7 attribute hashes of §4.7 were searched as instruction immediates; the attribute hashes have no hit, and every property-ID hit is an argument of a child-element lookup in user-interface code, a separate identifier space that shares the numeric range; no defined string in the executable contains `Progression`, `Mod_` or `Combat_Control`; (b2) no: none of the 986 function IDs of classes 484, 486, 698, 827, 1385, 1386, 1389, 1479, 2402, 2519, 3810, 3916–3918 and 3920 occurs as an immediate; (b3) not located in this pass (the library loader and its chunk dispatcher were found; the interpreter loop was not). Nothing from network, login, launcher or anti-tamper code was read. |

Marks: **Confirmed** = observed in the user's install over every record of the kind named (not a
sample), on the date given; **Reported** = published third-party material only; **BIN only** =
read from the client executable under M4 (decision 0011); **Decision** = a Tessera choice (DEC),
not a fact about the original.

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| C1 | Family membership by ancestry in the class library's parent map, and the observed class indices, counts and sizes (§2.2) | OBS (all 383,332 object records); DS | Confirmed 2026-10-06 |
| C2 | Every family record has a property set at DID + 0x09000000 and decodes completely on both halves; no family record has a partial bundle (§3.1) | OBS | Confirmed 2026-10-07 |
| C3 | The 0x70 half is a compiled form of the 0x79 data (progression arrays, modifier entries, attack hooks, effect lists); cross-checks of §3.2, §4.7, §5.8 | OBS | Confirmed 2026-10-07 |
| C4 | Property-reference fields (type 10) name actor properties to read or write; patterns `_Override`, `_ModifierList`, `_Mod_Array` of PIDs, `_AdditiveArray`, `_PropertyName` (§3.3) | OBS (types and holders); LC (meaning) | Layout Confirmed; meaning Reported |
| C5 | Progression kinds, their properties and lengths (§4.1–§4.6) | OBS (all 7,558 + 39 records) | Confirmed 2026-10-07 |
| C6 | `Progression_MinimumIndexValue` absent ⇒ 0 (the dictionary default), and the compiled form stores 0 (§4.2) | OBS (6 of 6 float progressions without it) | Confirmed. Disagrees with LC, which assumes 1 (§16) |
| C7 | Linear-interpolating keys are strictly increasing; the compiled form stores one segment per adjacent pair with the slope (y₁ − y₀) / (x₁ − x₀) computed in single precision, bit-exact; the compiled minimum index equals the first key (§4.4) | OBS (1,814 of 1,814 records, 30,100 segments) | Confirmed 2026-10-07 |
| C8 | Array index rule: element *k* (0-based) belongs to index MinimumIndexValue + *k* (§4.3) | OBS (consistency of minimum index and compiled form); LC | Layout Confirmed; use as an index Reported |
| C9 | Behaviour outside the covered index range (§4.8) | LC (extrapolate along the last segment above; undefined below) | Reported only; **OPEN** (Tessera's rule is D1) |
| C10 | Modifier-entry layout and its three kinds: empty, inline value, progression-driven; never both value and progression (§5.1, §5.2) | OBS (all 372,834 entries in all 116,435 `Mod_Array` values) | Confirmed 2026-10-07 |
| C11 | Operator enum 0x23000046 and which operators occur with which value types (§5.3) | OBS (mapper and all entries); LC (operator meanings) | Keys and co-occurrence Confirmed; arithmetic Reported |
| C12 | Level ranges: entries for one property within one array have disjoint ranges (stored order not guaranteed; 100 groups are not ascending as stored), are never mixed with unranged entries for the same property, and occur only in non-combat holders (§5.5) | OBS (all 14,798 ranged groups, all 19,533 ranged entries; re-measured 2026-10-07) | Confirmed 2026-10-07; which level is compared Reported (LC) |
| C13 | Effect classes, their distinctive properties and counts (§6.1) | OBS (all 32,971 effects, 87 classes) | Confirmed 2026-10-07; role names are this spec's descriptions |
| C14 | Duration and application-probability property census, and the duration groups by stored value (§6.2) | OBS (all 32,971 effects; re-measured 2026-10-07) | Confirmed 2026-10-07; semantics Reported (LC) |
| C15 | Vital-change amount pattern (constant, progression, random, variable, variance, modifier lists) and its co-occurrence census (§6.4) | OBS (all 1,103 instant-vital and 2,438 vital-over-time effects); LC (meaning) | Layout Confirmed; meaning Reported |
| C16 | Equivalence-class data in the effect control object (§6.6) | OBS | Layout Confirmed; meaning **OPEN** |
| C17 | Skill property census; 56 properties present in every skill (§7) | OBS (all 13,247 skills) | Confirmed 2026-10-07 |
| C18 | Attack hooks: one compiled object per `Skill_AttackHookInfo` in every skill (§7.7) | OBS (13,733 of 13,733) | Confirmed |
| C19 | Trait classes, trait trees (class 3486), trait-cell key structure (§8) | OBS | Confirmed; row/column meaning **OPEN** |
| C20 | Class advancement tables (class 853), derived-stat formula lists, monster-play tables, advancement lines (§9) | OBS | Layout Confirmed; formula reading Reported |
| C21 | Combat control object, DPS and value lookup tables, thresholds, battle statistics, DID maps COMBATEFFECTS and BATTLESTATS (§10) | OBS; LC (curve and table formulas) | Layout Confirmed; formulas Reported |
| C22 | Enum mappers used by these families (§11) | OBS | Confirmed |
| C23 | Reference graph census; no family record names a missing object record (§12) | OBS (every DID value in every 0x78 and 0x79 set) | Confirmed 2026-10-07 |
| C24 | The client library contains bytecode bodies for the progression, effect, skill, trait and modifier classes, and its bytecode contains the IDs of `Mod_Op`, `Mod_Modified`, `Mod_Progression`, `Mod_ModifierList`, `Mod_ProgressionFloor`, `Combat_DPS_LUT`, `Item_ValueLookupTable` and others as 32-bit literals (§13.2) | OBS (method tables of `wlib.md` §6.4; byte search of the bytecode chunk) | Confirmed |
| C25 | Native client code neither reads these properties by ID nor calls these script classes by constant function ID (§14) | BIN (b1, b2) | BIN only |
| C26 | Third-party formulas for rating percentages, weapon damage and skill hook damage (§13.3) | LC | Reported only |
| C27 | Who uses progressions: holder, property path, reference count and target classes (§4.9); all 50 references to class 3810 sit in modifier entries of class-3438 traits (§4.5) | OBS (every DID value of every 0x78 and 0x79 set) | Confirmed 2026-10-07 |
| C28 | Effect-reference structs, their fields, holders and counts; every occurrence of `EffectGenerator_EffectID` by enclosing struct and holder, its 59 zero values (one class-1932 record) and its targets (§6.3); placeholder `Skill_Effect` counts (§3.2, §7.8) | OBS (every 0x78 and 0x79 set; `EffectGenerator_EffectID` occurrences re-measured 2026-10-07) | Confirmed 2026-10-07 |
| C29 | Class-specific effect properties: presence, value ranges and holders of `Mod_Array` per effect class (§6.5) | OBS (all records of the classes named) | Presence and ranges Confirmed 2026-10-07; meanings are this spec's descriptions from the labels |
| C30 | Skill timing properties; induction objects (class 1195) and channelling objects (class 961), their counts and duration properties (§7.9) | OBS (all 13,247 skills, all 1,779 class-1195 and 401 class-961 records); LC (induction duration as cast time) | Layout and counts Confirmed 2026-10-07; "cast time" Reported |
| C31 | Trait control object (class 1486): one record, reached through the WEENIECONTENT map; its progression references and list lengths (§8.6) | OBS | Confirmed 2026-10-07 |
| C32 | Equivalence-class data counts and value histogram (§6.6); DPS and value table lengths and holders (§10.2) | OBS (the one class-1666 record; all 8 class-1660 and all 99 class-1688 records; every 0x78 and 0x79 set for holders) | Confirmed 2026-10-07 |
| C33 | Enum-mapper labels of §11 | OBS (EMAPPER DID map 0x28000001 and each mapper) | Confirmed 2026-10-07 |
| C34 | No integer-typed value equals the DID of an object record, so combat references are stored only as DIDs (§3.4) | OBS (every int, 64-bit int, 32- and 64-bit bitfield and enum value in all 1,616 0x78 and 383,332 0x79 sets, compared with the DIDs of all 383,332 object records; re-measured 2026-10-07) | Confirmed 2026-10-07 |
| C35 | Modifier lists: 1,946 non-empty entries with at least one non-zero PID, lists of 1 or 2 PIDs, `Mod_ModifierOp` counts 668 / 108 / 887 / 283 for keys 5–8 (§5.4); how listed values combine with the entry | OBS (all 372,834 entries); LC (combination) | Counts Confirmed 2026-10-07; combination Reported |
| C36 | Other modifier-like arrays and their progression-reference counts, 18 and 10 (§5.7) | OBS (every DID value of every 0x78 and 0x79 set, as C27) | Confirmed 2026-10-07 |
| C37 | Declared functions and functions with bodies of classes 698 (282, 123) and 827 (477, 166) in the client library; the server-side library DID is in no archive (§13.1) | OBS (method tables of the client library, decoded as `wlib.md` §6.4 describes; re-measured 2026-10-07); `wlib.md` §2 (missing server-side library) | Counts Confirmed 2026-10-07; the missing library as `wlib.md` §2 states it |
| D1 | Out-of-range progression index: clamp to the covered range and log (§4.8, §4.10) | DEC | Decision |
| D2 | Sandbox combination order of modifier entries (§5.3), used by the §5.9 example | DEC | Decision |
| D3 | Value at a linear key, including the last key: the key's stored value (§4.4) | DEC | Decision |
| D4 | Progression of progressions evaluated without an inner index gives none; chain depth limit 4 (§4.10) | DEC | Decision |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Combat object** | An object record (0x70 DID) whose root class belongs to one of the families of §2. |
| **Family** | A group of classes with a common ancestor in the class library's parent map (`wlib.md` §9): effects, traits, progressions, skills, advancement tables and so on. |
| **Property half / compiled half** | The 0x79 property set of an object (readable, keyed by property IDs) and its 0x70 object state (script objects keyed by attribute hashes). §3.2. |
| **Property reference** | A value of type 10 (property ID, `property-system.md` §7.4). In combat data it names a property **of an actor** (the caster, the target, the item's wearer) whose current value is read or written at run time. 0 means "none". |
| **Progression** | An object that maps an integer index (a level, a rank, a tier) to a value. §4. |
| **Modifier entry** | One `Mod_Entry` struct in a `Mod_Array`: "change property *P* by operator *op* with this value or progression". §5. |
| **Spellcraft** | A float stored next to effect references (`*_Spellcraft`, `Skill_EffectSpellcraft`). −1 means "not given". Reported (LC, community usage): the index at which the referenced effect's progressions are evaluated; when not given, the applier's level. Unsettled, see the callout below. |
| **Placeholder entry** | A struct or reference present only because the record stores a full default set: a `Skill_EffectData` with `Skill_Effect` = 0, a `Mod_Entry` with `Mod_Modified` = 0. Readers skip them. |

> **OPEN:** (§18 Q9) What spellcraft means. The reading above (the index at which the
> referenced effect's progressions are evaluated, the applier's level when −1) is Reported only;
> no record and no BIN result states it. A bytecode reading of the effect classes (§13.2) or M3
> tooltip observation would settle it. Until then a viewer evaluates at a user-chosen index (§15).

Units used below: durations and recovery times are **seconds**; ranges, radii and distances are
**world units** (metres by convention, `coordinates.md`); probabilities and chances are
fractions 0–1 unless the property's range says otherwise; arc and heading angles are degrees
(Reported, from their ranges 0–180 and −1–180).

---

<a id="s2"></a>
## 2. Families

<a id="s2-1"></a>
### 2.1 Classification rule

A combat object is classified by its **root class** (`wstate.md` §3.1) and the root class's
ancestors in the parent map (`wlib.md` §9):

1. Read the root class *c* at payload offset 4 of the 0x70 record.
2. Walk *c* and its ancestors. The first rule of the table below that matches gives the
   family. Rules are tested in table order.
3. Classes that match no rule are not combat objects (items, creatures, quests, ...).

Class indices are build-specific (`wlib.md` §1). The indices below are those of this install;
LC's builds from 2023-11 to 2025-05 used the same indices for the classes they name (`wlib.md`
W23, Reported). A reader should keep the indices in one table that a new build can replace, and
may cross-check with the family's signature properties (last column), which are in the data:
every record of the family has them (OBS).

| Order | Family | Rule | Signature properties (present in every record) |
|---|---|---|---|
| 1 | Effect | 698 is *c* or an ancestor | `Effect_Name`, `Effect_Definition_Description`, `Effect_Applied_Description` |
| 2 | Trait | 1479 is *c* or an ancestor | `Trait_Name`, `Trait_Nature`, `Trait_Category`, `Trait_Tier`, `Trait_Cost_To_Slot` |
| 3 | Progression | 1386 is *c* or an ancestor | one of the array properties of §4.1 |
| 4 | Skill | *c* = 827 | `Skill_Name`, `Skill_SkillType`, `Skill_TargetSelectionMode` (and 53 more, §7.1) |
| 5 | Monster-play table / advancement line | *c* = 1932, or 2389 is an ancestor | `AdvTable_ArmorDefense_Points_CalcType` (1932) or `AdvLine_EntryList` (2390, 2399) |
| 6 | Class advancement table | *c* = 853 | `AdvTable_LevelEntryList`, `AdvTable_DerivedStat_Configuration` |
| 7 | Threshold policy | 3530 is an ancestor | `Threshold_PercentageValue_Policy` |
| 8 | Battle statistic | 3584 is an ancestor | `BattleStat_Name`, `BattleStat_Scope` |

Related objects outside these subtrees that this spec also describes: trait trees (class 3486,
§8.5), sparse DID progressions (class 3800, §8.5), the combat control object (class 1659,
§10.1), the effect control object (class 1666, §6.6), the trait control object (class 1486,
§8.6), DPS tables (class 1660) and item value tables (class 1688) (§10.2).

<a id="s2-2"></a>
### 2.2 Census (Confirmed, OBS)

| Family | Classes with records | Records | 0x70 bytes | 0x79 bytes |
|---|---|---|---|---|
| Skill | 1 (827) | 13,247 | 30,685,392 | 36,116,542 |
| Effect | 87 | 32,971 | 17,182,958 | 15,028,659 |
| Progression | 6 | 7,558 | 20,337,190 | 10,009,116 |
| Trait | 9 | 3,963 | 2,171,653 | 1,607,523 |
| Monster-play table (1932) and advancement lines (2390, 2399) | 3 | 141 (67 + 59 + 15) | 80,743 | 117,867 |
| Class advancement table (853) | 1 | 12 | 312,716 | 906,720 |
| Threshold policy (3531) | 1 | 6 | 6,488 | 1,602 |
| Battle statistic (3590, 3622) | 2 | 13 (10 + 3) | 1,274 | 1,595 |

Effect classes with records, by count: 734 (14,475), 755 (2,438), 719 (1,547), 708 (1,245),
767 (1,189), 725 (1,103), 718 (1,076), 763 (767), 2459 (722), 713 (666), 749 (630), 709 (617),
715 (549), 3866 (526), 5724 (470), 3686 (461), 2762 (453), 724 (429), 736 (356), 776 (328),
737 (310), 769 (239), 722 (236), 745 (222), 2063 (163), 3842 (132), and 61 classes with fewer
than 125 records each (§6.1 lists all of them with their roles). Their direct parents are 698,
705, 711, 730, 734, 753, 713, 715, 786, 2258, 2751 and 3941; 698, 705, 711, 730, 753, 786, 2258
and 3941 have no records of their own.

Trait classes: 1477 (1,773), 3438 (946), 3509 (767), 2525 (244), 1483 (125), 3564 (45),
1494 (37), 1478 (25), 3712 (1). Ancestry: 1477, 1478 and 3509 derive from 1479; 1483 and 1494
from 1477; 2525, 3564 and 3712 from 1494; 3438 from 3712.

Progression classes: 1389 (4,303), 3918 (1,814), 1385 (1,338), 2402 (46), 3810 (45),
2519 (12). 1385 and 3918 derive from 3920, 3810 from 2519; 1386 and 3920 have no records.

---

<a id="s3"></a>
## 3. The common record model

<a id="s3-1"></a>
### 3.1 Two halves (Confirmed)

Every combat object has both halves: the 0x70 object state and the 0x79 property set
(`property-system.md` §6.2). All records of all families decode completely with the decoders of
`wstate.md` and `property-system.md` (no partial bundle, no leftover bytes).

**Read the property half.** It carries every value this spec describes, under the property
labels of the master dictionary, which are interoperability identifiers. The tables below give
the PIDs observed in this install for orientation only; an implementation looks properties up
**by label** through the dictionary at run time.

<a id="s3-2"></a>
### 3.2 The compiled half (Confirmed for the parts named)

The object state holds the same data in a form the script machine uses: script objects whose
attributes are named by hashes (`wlib.md` §8.2). Cross-checks over all records:

- Progressions: the compiled array equals the property array element by element in all
  7,558 records, and linear-interpolating progressions add precomputed segment slopes (§4.7).
- Modifier entries: each non-empty `Mod_Entry` becomes one script object, of class 1339
  (inline value) or 1342 (progression-driven), both derived from 1340 (§5.8).
- Skills: each `Skill_AttackHookInfo` becomes one object of class 822 (13,733 of 13,733, every
  skill); arc, box and sphere detection volumes become objects of classes 819 (1,121, equal to
  the skills with `Skill_AEDetectionVolume_ArcRadius`), 820 (132) and 821 (1,882, equal to the
  skills with `Skill_AEDetectionVolume_SphereRadius`), all derived from 818.
- Effect references in skills compile to objects of class 828 (17,033 objects: effect DID,
  spellcraft, duration, implement usage). Placeholder references (`Skill_Effect` = 0: 97,268 in
  skills, C28) are not compiled, and the per-record counts do not match the property half one to one.

An implementation never needs the compiled half to obtain a value. It is useful only to a
script interpreter, which this spec does not cover.

<a id="s3-3"></a>
### 3.3 Property references (layout Confirmed, meaning Reported)

Many fields hold a property ID (type 10) rather than a value. Five naming patterns occur:

| Pattern | Value | Meaning (Reported, LC; consistent with the names and holders) |
|---|---|---|
| `X_Override` (e.g. `Skill_UserEffectList_Override`) | one PID | If non-zero, the actor's current value of that property replaces list or value *X*. |
| `X_ModifierList`, `X_Mod_Array` of `Skill_Property_Mod`, `X_ModifierArray` | array of PIDs | The current values of these actor properties are added to *X* (an additive bonus). Zero PIDs are padding. |
| `X_AdditiveArray` of `Skill_AdditiveEffectArray_Entry` | array of PIDs | The named actor properties hold further effect lists that are appended to list *X*. |
| `*_PropertyName`, `*_Property`, `*_ValueProperty` | one PID | The property that is read, written or represented (for example the property a progression stands for, §4.2). |
| `X_AdditiveModProp` | one PID | An actor property whose value is added to *X* (for example to an application probability). |

Readers display such a field by the property's label and resolve it only when an actor exists.

<a id="s3-4"></a>
### 3.4 Defaults and placeholders (Confirmed)

- Skills store an almost complete property set: 56 properties occur in every skill (§7.1),
  most with default values (0, −1, false or an empty list).
- A DID value 0 means "none" in every reference property (`property-system.md` §4.9). All
  references between combat objects are of type 20; none is stored as an integer (C34: no
  integer, 64-bit integer, bitfield or enum value in any 0x78 or 0x79 set equals the DID of an
  existing object record, combat or not).
- `Mod_Modified` = 0 marks an empty modifier entry (§5.2); `Skill_Effect` = 0 an empty effect
  reference. Both are skipped.

---

<a id="s4"></a>
## 4. Progressions

This section is self-contained: it can be moved to its own spec (`progressions.md`) without
change, and that is recommended, because items, skills, effects, traits, class tables and the
combat control object all point into it (§4.9).

<a id="s4-1"></a>
### 4.1 Kinds (Confirmed, all records)

| Class | Kind (this spec's name) | Records | Value array property (PID) | Element |
|---|---|---|---|---|
| 1389 | Property array | 4,303 | `PropertyProgression_Array` (0x10001082) | One property per element; all elements of a record carry the **same** PID, which is the property the progression produces |
| 1385 | Float array | 1,338 | `FloatProgression_Array` (0x10000947) | `FloatProgression_FloatValue` (f32) |
| 3918 | Linear interpolating | 1,814 | `LinearInterpolatingProgression_Array` (0x10005E5B) | Struct `LinearInterpolatingProgression_ControlPoint` with `LinearInterpolatingProgression_Key` (int) and `LinearInterpolatingProgression_Value` (f32) |
| 2402 | Text array | 46 | `StringInfoProgression_Array` (0x10002898) | `StringInfoProgression_Value` (StringInfo) |
| 2519 | DID array | 12 | `DataIDProgression_Array` (0x10002BFB) | `DataIDProgression_DataIDValue` (DID) |
| 3810 | Progression of progressions | 45 | `DataIDProgression_Array` | as 2519; each DID names another progression |

Element types of the property arrays (1389), by record: f32 3,939; int 319; DID 22; 32-bit
bitfield 20; enum 3. No progression array contains a null element.

Observed lengths (records):

| Kind | Lengths |
|---|---|
| Property array | 200 (3,174), 160 (585), 80 (289), 25 (106), 6 (69), 301 (27), 159 (18), 16 (13), 599 (11), 258 (9), 20 (1), 45 (1) |
| Float array | 200 (392), 500 (282), 80 (249), 150 (193), 160 (125), 100 (42), 120 (8), 15 (7), 2 (6), 19 (6), 5 (5), 8 (5), 10 (5), 101 (4), 16 (2), 301 (2), 401 (2), 700 (2), and 7, 20, 36, 45, 250 once each |
| Linear interpolating (points) | 19 (913), 20 (453), 22 (211), 2 (112), 4 (59), 5 (46), 3 (13), 21 (5), 14 (1), 17 (1) |
| Text array | 150 (40), 11 (4), 7 (1), 12 (1) |
| DID array (2519) | 200 (12) |
| Progression of progressions | 5 (19), 7 (15), 10 (9), 6 (2) |

> **OPEN:** (implementation census, 2026-10-07) The float-array row sums to 1,342 records, not
> the 1,338 float arrays of this section. The implementation's census
> (`crates/formats/tests/gameplay_combat_real_data.rs`) reproduces every other count of the row
> and observes length 8 in 1 record, not 5, which gives 1,338. Is "8 (5)" a typo for "8 (1)"?

<a id="s4-2"></a>
### 4.2 Common fields

| Property | PID | Type | Present in | Meaning |
|---|---|---|---|---|
| `Progression_MinimumIndexValue` | 0x100002C8 | int | 5,738 records: every kind except linear interpolating, and 1,332 of 1,338 float arrays | Index of the first array element. Values: 1 (5,721), 0 (9), −9 (6), −100 (2). **Absent ⇒ 0**, the dictionary default; the compiled form of the 6 float arrays without it stores 0 (C6). |
| `FloatProgression_ValueProperty` | 0x10000766 | PID | 698 float arrays | The property the curve stands for (§3.3). Informational. |
| `LinearInterpolatingProgression_ValueProperty` | 0x10005E5D | PID | all 1,814 linear progressions | As above. |
| `ProgressionProgression_InnerRepresentedPropertyName` | 0x100058D8 | PID | all 45 progressions of progressions | The property the inner progressions produce. |
| `Progression_Type` | 0x10001F0D | enum, mapper 0x2300028A (95 keys) | 5,029 records (3,207 property arrays, all 1,814 linear, 8 float) | A curve-family label used by item advancement tables. Not needed to evaluate. |
| `Progression_PointTier` | 0x10001F0A | enum, mapper 0x23000289 (10 keys) | same 5,029 records | A tier label, as above. |

<a id="s4-3"></a>
### 4.3 Array progressions (property, float, text, DID)

Element *k* (counting from 0) is the value at index *m* + *k*, where *m* is the minimum index
(§4.2). The covered range is *m* … *m* + *n* − 1 for *n* elements.

- **Property array:** the value is the element's value, of the element's own type. Because all
  elements share one PID, the progression also tells which property it produces.
- **Float array:** the value is the f32 element.
- **Text array:** the value is a StringInfo, resolved as in `records.md` §4.
- **DID array (2519):** the value is a DID. Observed targets: 1,283 effects (class 734), 1,000
  float progressions, 117 zeros (none).

<a id="s4-4"></a>
### 4.4 Linear-interpolating progressions (Confirmed)

The value array holds control points (*xᵢ*, *yᵢ*), *xᵢ* an int key and *yᵢ* an f32.

Facts over all 1,814 records:

- Keys are **strictly increasing** in every record. Keys lie in 0–999. The first key is 1 in
  1,807 records, 0 in 6 and 115 in 1. The last key lies between 5 and 999.
- The record has no `Progression_MinimumIndexValue`. The compiled form's minimum-index
  attribute equals the **first key** in all 1,814 records.
- The values are monotonic (all non-decreasing or all non-increasing) in 1,787 records and not
  monotonic in 27.
- The compiled form holds one **segment** per adjacent pair (*n* − 1 segments for *n* points,
  30,100 in total). Each segment stores its two points and a slope *s*. In every segment *s* is
  bit-exactly the single-precision result of (*y₁* − *y₀*) / f32(*x₁* − *x₀*): first the f32
  difference of the values, then the integer difference of the keys converted to f32, then
  one f32 division.

**Evaluation inside the range** (*x₀* ≤ *x* ≤ *x_last*):

1. If *x* equals a key *xⱼ* (any key, the first and the last included), the value is that
   key's stored value *yⱼ* (Decision D3).
2. Otherwise find the segment with *xᵢ* < *x* < *xᵢ₊₁*; exactly one exists because the keys
   strictly increase. The value is *yᵢ* + *s* × f32(*x* − *xᵢ*), computed in f32, with *s* the
   stored or recomputed slope.

Step 1 agrees with step 2's form at every inner key (the segment that starts at the key gives
*yᵢ* + *s* × 0 = *yᵢ* exactly). At the last key no segment starts; the last segment, which ends
there, would give *yₙ₋₂* + *s* × (*xₙ₋₁* − *xₙ₋₂*), which can differ from *yₙ₋₁* in the last bit
of f32. D3 picks the stored value so that the result at a key never depends on rounding.

> **OPEN:** (§18 Q2) The original's exact f32 evaluation inside a segment: whether it computes
> *yᵢ* + *s* × (*x* − *xᵢ*) with the stored slope (the form the stored slopes suggest), or
> interpolates the two points directly, and what it returns at the last key. The form above
> follows from the stored slopes but is not Confirmed: the evaluation code is bytecode (§13.2)
> and native code does not evaluate progressions (C25).

<a id="s4-5"></a>
### 4.5 Progressions of progressions (class 3810)

The DID array names 5 to 10 inner progressions (302 elements: targets 287 linear, 10 float,
5 property arrays; never 0). `Progression_MinimumIndexValue` is 1 in all 45 records. Evaluation
takes **two** indices: the **outer** index *t* selects the element as in §4.3 (element *k* =
*t* − *m*), and that element's progression is then evaluated at the **inner** index (§4.10).
Reported (LC): the outer index is a tier counted from 1 and the inner index a level.

Holders (C27): exactly 50 `Mod_Progression` values point at class 3810, and all 50 are
modifier entries of point-based traits (class 3438, §8.1). No item, effect or skill references
a progression of progressions in this install.

> **OPEN:** (§18 Q3) Which actor value supplies the outer index and which the inner index. The
> holders are ranked trait-tree traits, so the trait's rank and the character's level are
> candidates, but nothing in the data or BIN says so. A property
> `Mod_ProgressionProgressionInnerIndex` exists in the dictionary and in the client library's
> bytecode (§13.2) but in no record. Until settled, an evaluator takes both indices from its
> caller and returns none when the inner index is missing (§4.10, Decision D4).

<a id="s4-6"></a>
### 4.6 Sparse DID progressions (class 3800, outside subtree 1386)

39 records. `SparseDIDProgression_Array` holds structs of `SparseDIDProgressionEntry_Key`
(f32, 5–120) and `SparseDIDProgressionEntry_DID` (all 270 DIDs are traits of class 3438). Used
only by trait-tree specialisations (§8.5). Reported (LC): at a given value, the entry with the
largest key not above it applies.

> **OPEN:** (implementation census, 2026-10-07) "All 270 DIDs are traits of class 3438" is not
> reproduced. Reading `SparseDIDProgressionEntry_DID` of every `SparseDIDProgression_Array` struct
> of the 39 records, the implementation finds 270 values, all naming traits, but of classes 1477
> (191) and 1483 (79), none of class 3438. Which field or which records does the statement
> describe? The implementation checks only "names a trait" until this is settled.

> **OPEN:** (§18 Q4) The lookup rule of sparse DID progressions: whether the entry with the
> largest key not above the value applies (LC's reading, Reported), what is returned below the
> first key, and what value is compared with the f32 keys (a rank, a point total, a level). The
> data stores only the key-DID pairs.

<a id="s4-7"></a>
### 4.7 Compiled form (Confirmed)

| Class | Attribute hash | Type | Meaning |
|---|---|---|---|
| 1386 (all progressions) | 0x08D9B2F5 | int | Minimum index; equals the property, or 0 when the property is absent, or the first key (3918) |
| 1389, 2402 | 0x06C10D43 | reference | Reference array (native class 176); each element is a native property object (class 39, 1389) or StringInfo object (class 199, 2402) equal to the property element |
| 1385, 2519, 3810 | 0x06C10EB3 | reference | Native u32 array (class 17): the f32 bit patterns (1385) or the DIDs (2519, 3810), equal to the property elements |
| 3918 | 0x02132869 | reference | Reference array of segments (class 3917) |
| 3917 (segment) | 0x0AA12EE4, 0x05BA3155, 0x0CF2D1F4 | reference, float, reference | Start point, slope, end point; consecutive segments share the point object |
| 3916 (point) | 0x0735E1C9, 0x05BC8485 | int, float | Key, value |

<a id="s4-8"></a>
### 4.8 Indices outside the range

> **OPEN:** What the original returns for an index below the first index or above the last.
> Nothing in the data shows it, and the native client does not evaluate progressions (C25); the
> client library's bytecode for classes 1385, 1389, 3918 and their base 1386 has 6, 5, 8 and 9
> function bodies of its own (`wlib.md` §6.4) that a bytecode reader could settle. LC clamps
> nothing and extrapolates linear progressions above the last key along the last segment, and
> returns no value below the first key (Reported). Until settled, Tessera clamps the index to
> the covered range for every kind and logs the clamp (Decision D1, not a fact): an index below
> the first index gives the first element or first key's value, an index above the last gives
> the last element or last key's value.

<a id="s4-9"></a>
### 4.9 Who uses progressions (Confirmed, every DID value of every 0x78 and 0x79 set)

All references are DID properties. "References" counts the non-zero values that name a
progression; the path is the enclosing struct and the field. Largest users (C27):

| Holder | Property (path) | References | Target classes |
|---|---|---|---|
| Non-combat objects (mostly items) | `Mod_Entry/Mod_Progression` | 292,436 | 3918 (289,921), 1389 (2,515) |
| Non-combat objects | `Item_RepairCostPerPointProgression` | 87,020 | 1385 (all) |
| Non-combat objects | `Item_Armor_Value_Lookup_Table` | 60,916 | 3918 (all) |
| Non-combat objects | `ItemMunging_ItemLevelOverrideProgression` | 20,798 | 1389 (all) |
| Skills | `Skill_AttackHookInfo/Skill_AttackHook_HookDamageMax_Progression` | 6,365 non-zero of 13,733 | 3918 (6,152), 1385 (213) |
| Skills | `Skill_VitalInfo/Skill_Vital_Points_Progression` | 4,513 non-zero of 52,236 | 3918 (4,509), 1385 (4) |
| Effects | `Mod_Entry/Mod_Progression` | 5,468 non-zero of 17,918 | 1389 (3,444), 3918 (1,607), 1385 (417) |
| Effects | `Effect_VitalOverTime_ChangePerIntervalProgression` | 2,403 | 3918 (2,085), 1385 (318) |
| Effects | `Effect_VitalOverTime_InitialChangeProgression` | 1,680 | 3918 (1,538), 1385 (142) |
| Effects | `Effect_InstantVital_InitialChangeProgression` | 812 | 3918 (479), 1385 (333) |
| Traits | `Mod_Entry/Mod_Progression` | 1,239 | 1389 (776), 3918 (399), 3810 (50), 1385 (14) |
| Traits | `Trait_Rank_Overlay_Progression` | 1,089 non-zero of 1,260 | 1389 (all) |
| Traits | `Trait_PointBasedTrait_PointCostProgression` | 946 | 1385 (all) |
| Combat control | `Combat_Control_CalcControlInfoStruct/Combat_Control_Hard_Cap`, `_Rating`, `_Target_Cap` | 30 each | 1385 (all) |

Totals of references to progressions by holder family: non-combat objects 475,922; skills
11,013; effects 10,672; traits 3,594; progressions 1,302; class advancement tables 24;
monster-play tables and advancement lines 8.

All `Mod_Progression` targets over the whole install: 3918 (291,927), 1389 (6,735),
1385 (431), 3810 (50).

Two item properties that read like progressions are not: `Combat_DPS_LUT` points at DPS tables
(class 1660) and `Item_ValueLookupTable` at value tables (class 1688) (§10.2).

<a id="s4-10"></a>
### 4.10 Evaluation algorithm (in words)

Inputs: a progression DID, an integer index *x* (the outer index for class 3810), and an
optional integer **inner index** *u*, used only by progressions of progressions (§4.5).
Output: a value or "none".

1. Load the property set of the DID. If it is missing, return none. If it has a linear array
   (`LinearInterpolatingProgression_Array`), go to step 6.
2. Find the value array of §4.1. If none is present, return none.
3. Let *m* be `Progression_MinimumIndexValue`, or 0 when absent, and *n* the element count.
   If *n* = 0, return none.
4. Let *k* = *x* − *m*. If *k* < 0 or *k* ≥ *n*, apply §4.8 (Decision D1: replace *k* by 0 or
   by *n* − 1 and log the clamp).
5. If the record is not a progression of progressions (class 3810), return element *k*.
   Otherwise: if no inner index *u* was given, return none (Decision D4). If element *k* is 0
   or does not name a progression, return none. Else evaluate element *k*'s progression with
   this same algorithm at index *u* and **no** inner index, and return its result. Done.
6. Linear, with *n* points (*x₀*, *y₀*) … (*xₙ₋₁*, *yₙ₋₁*) in stored order: if *n* = 0, return
   none. If the keys do not strictly increase (they do in every record, P3), the record is
   malformed: return none and log it. If *x* < *x₀* or *x* > *xₙ₋₁*, apply §4.8 (D1:
   replace *x* by *x₀* or *xₙ₋₁* and log the clamp). If *x* equals a key, return that key's
   value (§4.4 step 1, D3). Otherwise find the segment with *xᵢ* < *x* < *xᵢ₊₁* and return
   *yᵢ* + *s* × f32(*x* − *xᵢ*), computed in f32 (§4.4 step 2). A record with one point returns
   that point's value for every *x* after the clamp.

Limits for hostile data: refuse arrays longer than the record allows (the decoder already
bounds them). Because step 5 passes no inner index on, a chain of progressions of progressions
ends after one level with none; a reader that extends this must still stop after 4 levels and
report a cycle (Decision D4; observed depth is 1).

<a id="s4-11"></a>
### 4.11 Invariants

- **P1** Every progression record has exactly one value array of §4.1 (7,558 of 7,558).
- **P2** All elements of a property-array progression share one PID (4,303 of 4,303).
- **P3** Linear keys strictly increase (1,814 of 1,814).
- **P4** Compiled slope = f32((*y₁* − *y₀*) / f32(*x₁* − *x₀*)), bit-exact (30,100 of 30,100).
- **P5** Compiled minimum index: property value, else 0, else (linear) first key.
- **P6** Progression-of-progression elements are never 0 and always name a progression
  (302 of 302).
- **P7** Every reference to a progression of progressions is a `Mod_Progression` in a
  class-3438 trait (50 of 50).

<a id="s4-12"></a>
### 4.12 Worked synthetic examples

- Float array, minimum index 1, elements (0.5, 0.75, 1.0): index 1 → 0.5, index 3 → 1.0,
  index 4 → out of range (§4.8); with D1 it is clamped to 3 → 1.0.
- Float array without a minimum index, elements (2.0, 4.0): index 0 → 2.0, index 1 → 4.0.
- Linear, points (1, 10.0), (11, 30.0), (21, 30.0): slopes f32(20.0 / 10.0) = 2.0 and 0.0;
  index 6 → 10.0 + 2.0 × 5 = 20.0; index 11 → 30.0 (stored value of the key); index 15 → 30.0;
  index 21 (the last key) → 30.0, the stored value (D3); index 25 → clamped to 21 → 30.0 (D1).
- Progression of progressions *P*, minimum index 1, elements (*A*, *B*), where *A* is a float
  array with minimum index 1 and elements (10.0, 20.0, 30.0), and *B* is linear with points
  (1, 100.0), (3, 300.0) (slope f32(200.0 / 2.0) = 100.0):
  - outer 2, inner 2: *k* = 1 selects *B*; *B* at 2 is not a key, segment (1, 3):
    100.0 + 100.0 × 1 = 200.0;
  - outer 1, inner 3: *k* = 0 selects *A*; *A* at 3 is element 2 = 30.0;
  - outer 3, inner 1: *k* = 2 is out of range, clamped to 1 (D1), selects *B*; *B* at 1 is a
    key → 100.0;
  - outer 1 with no inner index: none (D4).

---

<a id="s5"></a>
## 5. Stat modifiers (`Mod_Array`)

<a id="s5-1"></a>
### 5.1 Layout (Confirmed)

`Mod_Array` (PID 0x100008C6, array; its only allowed child is `Mod_Entry`) holds structs
`Mod_Entry` (0x10000D39). Fields of a `Mod_Entry`:

| Field | PID | Type | Meaning |
|---|---|---|---|
| `Mod_Modified` | 0x10000525 | PID | The property that is changed. 0 = empty entry. |
| `Mod_Op` | 0x1000016A | enum, mapper 0x23000046 | Operator (§5.3) |
| *the modified property itself* | = `Mod_Modified` | that property's type | The **inline value**: a field whose PID equals `Mod_Modified`, typed by that property's definition |
| `Mod_Progression` | 0x100011D8 | DID | A progression that supplies the value instead (0 = none) |
| `Mod_ProgressionFloor`, `Mod_ProgressionCeiling` | 0x10003F8C, 0x10003F8D | int (default −1) | Level range of the entry (§5.5) |
| `Mod_ModifierList` | 0x100013DC | array of `Mod_ModifierProperty` (PID) | Actor properties that modify the value (§5.4) |
| `Mod_ModifierOp` | 0x10000B65 | enum, mapper 0x23000046 | How the modifier list combines (§5.4) |
| `Mod_DescriptionOverride` | 0x1000034C | StringInfo | Tooltip text replacing the generated line |
| `Mod_DescriptionOverrideProgression`, `Mod_NextRankDescriptionOverride`, `Mod_NextRankDescriptionOverrideProgression` | | DID / StringInfo / DID | Further tooltip overrides (51, 147 and 20 entries) |

<a id="s5-2"></a>
### 5.2 Kinds of entry (Confirmed, all entries)

Census over every `Mod_Array` in every 0x78 and 0x79 set: 116,435 arrays (16,947 in effects,
2,262 in traits, 97,226 in other objects, mostly items), 372,834 entries.

| Kind | Rule | Count |
|---|---|---|
| Empty | `Mod_Modified` = 0 | 12,449, all in effects. Each has `Mod_Op` 0, `Mod_Progression` 0, `Mod_ModifierOp` 0 and a modifier list of two zero PIDs. |
| Field-less | no fields at all | 947, all in non-combat objects |
| Inline value | `Mod_Modified` ≠ 0 and the value field present; `Mod_Progression` 0 or absent | 60,295 |
| Progression-driven | `Mod_Modified` ≠ 0 and `Mod_Progression` ≠ 0; **no** value field | 299,143 |

No entry has both a non-zero `Mod_Progression` and a value field, and every non-empty entry has
one of them. In 12 entries a struct carries one more field with an unrelated PID (anomalies;
ignore them). The value field's type always matches the modified property's declared type.

Readers skip empty and field-less entries.

> **OPEN:** (implementation census, 2026-10-07) The 116,435 arrays and 372,834 entries of this
> section are reproduced only when `Mod_Array` values nested inside other values are counted too
> (3,933 arrays below top level, none in effects or traits); top-level arrays alone give 112,502.
> Please state that the census covers `Mod_Array` at any depth, and whether a reader should apply
> nested arrays (they sit inside item structs whose meaning is `items.md`'s, not this spec's).

<a id="s5-3"></a>
### 5.3 Operators (keys Confirmed, arithmetic Reported)

Mapper 0x23000046 has 12 keys. The labels are interoperability identifiers; a reader resolves
the operator by label at run time and falls back to the key numbers below.

| Key | Label | Entries (inline / progression) | Value types seen | Meaning (Reported, LC; LC handles only 5–8) |
|---|---|---|---|---|
| 0 | `Invalid` | empty entries only | | none |
| 1 | `Difference` | 0 | | not used |
| 2 | `Divide` | 0 | | not used |
| 3 | `Or` | 2,227 / 20 | bool, 32/64-bit and any-width bitfields | Bitwise or of the value into the property |
| 4 | `Random` | 0 | | not used |
| 5 | `Set` | 8,365 / 28 | every type, including DIDs, enums and arrays | Replace the property's value |
| 6 | `Subtract` | 3,762 / 357 | float, int | Subtract the value |
| 7 | `Add` | 34,821 / 298,697 | float, int | Add the value |
| 8 | `Multiply` | 10,939 / 41 | float, int | Multiply by the value |
| 10 | `ArrayAdd` | 181 / 0 | arrays | Append the array's elements |
| 11 | `And` | 0 | | not used |
| 12 | `Xor` | 0 | | not used |

> **OPEN:** The order in which several entries (from several effects, items and traits)
> touching one property are combined, and whether `Set` wins over arithmetic. This is server
> logic (§13.1). Tessera's sandbox uses (Decision D2, not a fact): start from the property's
> base value, apply `Set` entries (last applied wins), then `Add`/`Subtract`, then `Multiply`,
> then bitwise operators, then `ArrayAdd`.

<a id="s5-4"></a>
### 5.4 Modifier lists

(C35.) 1,946 non-empty entries carry a modifier list with at least one non-zero PID (lists hold 1 or
2 PIDs). `Mod_ModifierOp` is then 5, 6, 7 or 8 (668, 108, 887, 283 entries). Reported (LC):
the current values of the listed actor properties are combined with the entry's value by
`Mod_ModifierOp` before the entry is applied (key 7 adds them). Zero PIDs are padding.

<a id="s5-5"></a>
### 5.5 Level ranges (Confirmed structure)

A **ranged group** is the set of non-empty entries of one array that share one `Mod_Modified`,
where at least one entry stores `Mod_ProgressionFloor` or `Mod_ProgressionCeiling`. All
14,798 ranged groups (19,533 ranged entries) of the install were checked (C12, re-measured
2026-10-07):

- **Holders:** every ranged group is in a `Mod_Array` of a non-combat object (items and the
  like). No effect, trait, skill or 0x78 set has a ranged entry.
- **Ends present:** 19,334 entries store both ends, 132 only the floor, 67 only the ceiling.
  No entry stores −1 (the dictionary default of both). Floors are 1–300, ceilings 9–1000;
  floor ≤ ceiling in every entry that stores both.
- **Group sizes:** 1 entry (10,548 groups), 2 (3,789), 3 (437), 4 (24).
- **Disjoint:** in every group the ranges [floor, ceiling] are pairwise disjoint.
- **Stored order is not guaranteed.** 14,698 groups store their ranges in ascending order; 100
  do not: 77 store [10, 19], [51, 399], [20, 50]; 1 stores [51, 399], [20, 50]; 22 store
  [70, 1000], [61, 69]. Readers must not depend on the stored order; sort by floor when an order
  is needed.
- **Touching once sorted:** after sorting by floor, consecutive ranges touch (ceiling + 1 =
  next floor) in all but 29 groups; the 29 all hold [10, 19] and [51, 399], leaving 20–50
  uncovered.
- A property with ranged entries never also has an unranged entry in the same array.
- A ceiling of 1000 is the common open end (13,751 entries).

So an entry applies when the governing level *L* satisfies floor ≤ *L* ≤ ceiling, both ends
inclusive, a missing end (absent, or −1 in other data) being open. At most one entry of a group
applies at any *L*, because the ranges are disjoint; in a gap none applies. Reported (LC): *L*
is the level of the item or character that owns the array.

> **OPEN:** (§18 Q5) Which level governs a ranged entry in an effect's or trait's modifier
> array (the applier's level, the target's level, or the effect's spellcraft). This install has
> no such entry, so the question matters only for other builds. A bytecode reading of the
> modifier classes (§13.2) would settle it.

<a id="s5-6"></a>
### 5.6 Evaluation of one entry (in words)

1. Skip the entry if `Mod_Modified` = 0 or the struct is empty.
2. If a level range is present, skip the entry when the governing level is outside it.
3. Value: the inline field if present; otherwise evaluate `Mod_Progression` (§4.10) at the
   governing level (Reported) or the spellcraft index (§1); for a progression of progressions
   (class 3810) the caller supplies the outer and the inner index (§4.5, Q3). If the
   progression gives none, skip the entry.
4. If a modifier list with non-zero PIDs is present, combine the actor's values of those
   properties with the value by `Mod_ModifierOp` (Reported).
5. Apply the value to the modified property with `Mod_Op` (§5.3).

<a id="s5-7"></a>
### 5.7 Other modifier-like arrays

Same idea, different labels (C36): `Runic_Mod_Entry` with `Runic_Mod_Progression` (18 values),
and `Item_QualityCombatPropertyModData/Item_PropertyModProgression` (10). They are not covered
further here.

<a id="s5-8"></a>
### 5.8 Compiled form (Confirmed)

Each inline entry compiles to an object of class 1339 (7 attributes) holding a native property
object (class 39) with the value; each progression-driven entry to an object of class 1342
(9 attributes); both derive from 1340 (6 attributes). Class 1341 (no attributes, 48 function
bodies) belongs to the same group. The per-record counts of 1339 and 1342 objects equal the
counts of inline and progression-driven entries in 383,291 of the 383,332 object records. In
the other 41 (40 non-combat objects and effect 0x70025EEE) the inline counts differ; in that
effect one inline entry (a `Set` of 0.0) has no compiled object. Progression-driven counts
differ in one record only.

<a id="s5-9"></a>
### 5.9 Invariants and worked example

- **M1** Every non-empty entry has exactly one of: value field, non-zero progression.
- **M2** The value field's type equals the declared type of `Mod_Modified`.
- **M3** Ranged groups are disjoint (14,798 of 14,798) and never mixed with unranged entries;
  once sorted by floor, consecutive ranges touch in all but 29 groups. The stored order is not
  an invariant (100 groups are not ascending).
- **M4** Bitwise operator 3 occurs only with bool and bitfield values; operators 6–8 only with
  int and float; operator 10 only with arrays.

Synthetic example: a property `P` (float, PID 0x10009999) with base value 1.0 and an array of
two entries: {`Mod_Modified` = P, `Mod_Op` = 7, P = 0.25} and {`Mod_Modified` = P,
`Mod_Op` = 8, P = 2.0}. With the sandbox order of §5.3 (Decision D2; the original's order is
OPEN, Q6) the result is (1.0 + 0.25) × 2.0 = 2.5.

---

<a id="s6"></a>
## 6. Effects

<a id="s6-1"></a>
### 6.1 Classes and roles (Confirmed, all 32,971)

The role column is this spec's description, derived from the properties that occur in at least
half of the class's records and rarely elsewhere ("distinctive properties", OBS).

| Class | Parent | Records | Role | Distinctive properties (prefix shown once) |
|---|---|---|---|---|
| 734 | 698 | 14,475 | Property modification | `Mod_Array` (all) |
| 755 | 711 | 2,438 | Vital change over time | `Effect_VitalOverTime_*` (vital type, interval FX, critical multipliers, partial-interval rule) |
| 719 | 730 | 1,547 | Summon an object | `Effect_Genesis_*` (summoned object, placement type and offset, duration) |
| 708 | 698 | 1,245 | Apply effects periodically | `Effect_ApplyOverTime_Applied_Effect_Array`, `_Initial_Applied_Effect_Array` |
| 767 | 705 | 1,189 | Conditional combination | `Effect_Combo_*` (effects present, effect to add if present / not present, remove old) |
| 725 | 711 | 1,103 | Instant vital change | `Effect_InstantVital_*`, `Effect_BaseVital_VitalType`, `Effect_DamageType` |
| 718 | 698 | 1,076 | Flag (presence only) | no own properties beyond the common ones |
| 763 | 705 | 767 | Summon a pet | `Effect_AIPet_*` |
| 2459 | 734 | 722 | Mount | `Mount_*`, `Mounted_*` |
| 713 | 734 | 666 | Count-down | `EffectGenerator_Countdown_ExpireEffectList`, pulse count |
| 749 | 705 | 630 | Travel | `Effect_Travel_PrivateEncounter`, `EffectGenerator_EffectDataList` |
| 709 | 698 | 617 | Aura | `Effect_Aura_*` (applied effects, type, radius, spellcraft override), `Aura_ShouldAffectCaster` |
| 715 | 705 | 549 | Dispel by effect | `Effect_Dispel_ByEffectList`, `Effect_Dispel_DispelCasters` |
| 3866 | 705 | 526 | Tiered | `Effect_TierUp_EffectList`, `_FinalEffect` |
| 5724 | 698 | 470 | Grant a bird type | `Effect_GrantBird_BirdType` |
| 3686 | 2258 | 461 | Skill proc | `Effect_SkillProc_*`, `Effect_MinTimeBetweenProcs`, level offsets |
| 2762 | 705 | 453 | Area | `Effect_Area_*` (applied effects, max range, max targets, who is affected) |
| 724 | 705 | 429 | Fellowship-wide | `Effect_InstantFellowship_*` |
| 736 | 753 | 356 | Reactive vital | `Effect_ReactiveVital_*` |
| 776 | 705 | 328 | Loot discovery | `Effect_Lootgen_*` |
| 737 | 705 | 310 | Recall | `Effect_Recall_*` |
| 769 | 705 | 239 | Induce a combat state | `Effect_InduceCombatState_*` |
| 722 | 698 | 236 | Grant a trait | `Effect_GrantTrait_Trait_To_Grant` |
| 745 | 705 | 222 | Instantaneous, no own data | |
| 2063 | 705 | 163 | Random choice | `Effect_RandomEffect_Array` |
| 3842 | 734 | 132 | Kill proc | `Effect_KillProc_*` |
| 720 | 705 | 123 | Grant an emote | `Effect_EmoteToGrant` |
| 756 | 698 | 123 | Vital transfer | `Effect_VitalTransfer_*` |
| 735 | 698 | 119 | Protection | `Effect_Protection_*` |
| 721 | 698 | 98 | Grant a skill | `Effect_GrantSkill_Skill_To_Grant` |
| 2080 | 705 | 97 | Send mail | `Effect_SendMail_*` |
| 3218 | 734 | 91 | Vital threshold | `Effect_VitalThreshold_*` |
| 728 | 698 | 90 | Knock away | `Effect_KnockAway_*` |
| 3184 | 705 | 88 | Cooldown change | `CooldownEffect_*` |
| 3222 | 713 | 88 | Absorb bubble | `Effect_Bubble_*` |
| 731 | 705 | 86 | Pip change | `Effect_Pip_*` |
| 714 | 705 | 81 | Dispel by resist category | `Effect_DispelByResist_*` |
| 766 | 705 | 65 | Leave a channelling state | `Effect_ExitChannelingState_StateToExit` |
| 739 | 753 | 57 | Redirect vital change | `Effect_RedirectVital_*` |
| 746 | 698 | 55 | Reset skills | `Effect_Skill_Reset_*` |
| 2281, 2282 | 705, 698 | 39, 10 | Threat change | `Effect_Threat_AbsoluteMultiplier_*` |
| 727 | 705 | 31 | Tutorial | `Effect_InvokeTutorial_Type` |
| 780 | 734 | 29 | Stealth | `Effect_Stealth_CancelledByMovement` |
| 2559 | 698 | 21 | Fear movement | `Effect_Fear_*` |
| 784 | 786 | 19 | Tracking | `Effect_Tracking_*` |
| 3833 | 734 | 18 | Generator propagation | `Effect_GeneratorPropagation_EffectList` |
| 3609 | 705 | 16 | Enter a channelling state | `Effect_EnterChannelingState_StateToEnter` |
| 744, 2441 | 705, 734 | 15, 2 | Revive | `Effect_Revive_*` |
| 2607 | 715 | 14 | Dispel (variant) | as 715 |
| 3124 | 698 | 14 | Persistent combination | `Effect_PersistentCombo_*` |
| 761 | 705 | 13 | Effect data list | `EffectGenerator_EffectDataList` |
| 1865 | 705 | 12 | Threat transfer | `Effect_AITransferThreat_*` |
| 2544 | 698 | 10 | Population control | `Effect_PopulationControl_*` |
| 3839 | 705 | 9 | Distant-battle damage | `Effect_DistantBattleDamage_*` |
| 768, 726, 770, 779, 2751, 2796, 3099, 3667 | various | 18, 2, 1, 2, 2, 1, 1, 1 | Instantaneous, no own data | |
| 706, 3173, 747, 762, 764, 723, 748, 752, 778, 2676, 758, 716, 1787, 2027, 2046, 2231, 2233, 2259, 2269, 2477, 3690, 3912, 3943 | various | 6 or fewer each | Account property, store alert, notify message, force attack, run away / call for help, impulse, vital transform, toggle skill, timed flag, pet force attack, experience bonus, and single-record specials | named after their `Effect_*` prefixes |

<a id="s6-2"></a>
### 6.2 Properties common to effects (Confirmed census)

| Group | Property | PID | Type | Records | Notes |
|---|---|---|---|---|---|
| Text | `Effect_Name`, `Effect_Definition_Description`, `Effect_Applied_Description` | 0x100011E7, 0x100015B8, 0x100015B9 | StringInfo | 32,971 each | |
| Icon | `Effect_Icon` | 0x1000110B | DID (0x41) | 22,804 | |
| Polarity | `Effect_Harmful`, `Effect_Debuff` | 0x100007F0, 0x100029D2 | bool | 32,965, 32,952 | |
| Application | `Effect_ConstantApplicationProbability` | 0x10000F42 | f32 0–1 | 32,622 | Reported: chance that the effect lands; absent ⇒ 1 |
| | `Effect_ApplicationProbabilityVariance` | 0x100009CA | f32 0–1 | 25,901 | Reported: random spread around the chance |
| | `Effect_ApplicationProbability_AdditiveModProp` | 0x10001E05 | PID | 22,358 (275 non-zero) | §3.3 |
| | `Effect_ApplicationProbabilityProgression` | 0x10000533 | DID | 14,144 (1 non-zero) | |
| Resist | `Effect_Resist_Category` | 0x10000E28 | bitfield, mapper 0x23000064 | 27,131 | |
| | `Effect_Resist_Level`, `Effect_Resist_Chance_Modifier`, `Effect_Resist_Chance_Modifier_Property_Array` | | int −2–150 (mostly 0), f32 −10–0.2, array of 3 PIDs | 26,491, 25,929, 11,548 | |
| Duration | `Effect_Duration_ConstantInterval` | 0x10000E39 | f32 seconds, 0.5–2,677,000 | 10,114 | |
| | `Effect_Duration_ConstantPulseCount` | 0x1000017F | int 1–43,200 | 3,206 | |
| | `Effect_Duration_Permanent` | 0x10001109 | bool | 14,219 (always true) | |
| | `Effect_Instantaneous` | 0x1000025C | bool, no dictionary default | 4,317 (3,594 true, 723 false) | A stored false means "not instantaneous", the same as absent |
| | `Effect_Duration_ProgressionPulseCount`, `Effect_Duration_ProgressionInterval` | 0x100014E1, 0x100013F1 | DID (progression) | 23, 1 | Pulse count or interval taken from a progression (Reported reading of the labels) |
| | `Effect_Duration_ExpiresInRealTime`, `Effect_Duration_CombatOnly` | | bool | 14,379, 18,372 | |
| | `Effect_Duration_ConstantInterval_ModifierList` | | PID array | 486 | §3.3 |
| Stacking | `Effect_EquivalenceClass` | 0x100009CC | enum, mapper 0x23000005 (3,081 keys) | 22,638 (12,478 are 0) | §6.6 |
| | `Effect_ClassPriority` | 0x100012EF | int 1–99,999 | 22,676 | |
| | `Effect_Cap` | 0x1000210A | int 1–100 | 2,709 | |
| Removal | `Effect_RemoveOnDefeat`, `_RemoveOnAwaken`, `_RemoveOnPulseResist`, `_RemovalOnlyInCombat`, `_IsCurable`, `_ForceRemoveOnLogout`, `_RemoveOnLeaveInstance` | | bool | 22,663, 22,663, 19,400, 17,861, 17,573, 1,872, 4,279 | |
| Client | `Effect_SentToClient`, `Effect_UIVisible`, `Effect_ShowRemovalTooltip`, `Effect_FX` (enum, mapper 0x23000097) | | | 26,650, 26,720, 17,864, 16,187 | |
| Critical | `Effect_Critical_Chance` | 0x100007DF | f32 −1–3 | 3,612 | |
| Tooltip | `UI_Examination_Tooltip_0Click_Layout`/`_Element`, `_1Click_*` | | DID (0x22), enum | 28,400 each | Layout of the examination tooltip |

Duration groups (each record falls in exactly one, OBS, all 32,971 effects). The groups go by
**stored value**, not by presence: "Interval" and "Pulse count" mean the property is present
(every stored value is positive: 0.5–2,677,000 s and 1–43,200); "Permanent" means
`Effect_Duration_Permanent` is present, which always means true (E2); "Instantaneous" means
`Effect_Instantaneous` is stored **true**. A stored false is treated like an absent flag.

| Interval | Pulse count | Permanent | Instantaneous | Records | Of which `Effect_Instantaneous` stored false |
|---|---|---|---|---|---|
| – | – | – | – | 6,702 | 0 |
| – | – | – | yes | 3,594 | 0 |
| – | – | yes | – | 12,561 | 722 (all 722 are mounts, class 2459) |
| yes | – | – | – | 5,250 | 1 (class 3943) |
| yes | yes | – | – | 3,206 | 0 |
| yes | – | yes | – | 1,658 | 0 |

No record has a true `Effect_Instantaneous` together with an interval, a pulse count or a
permanent flag. Every record that stores it as false also has another duration source (the
permanent flag or an interval), which is consistent with reading false as "not instantaneous".
The 23 records with `Effect_Duration_ProgressionPulseCount` are in the "interval only" row; the
one record with `Effect_Duration_ProgressionInterval` is in the first row (no constant duration
property).

> **OPEN:** Whether `Effect_Duration_ConstantInterval` is the whole duration or the time
> between pulses when a pulse count is present, and how "permanent" combines with an interval
> (1,658 records have both). LC reads the interval as the duration and the pulse count
> separately (Reported). Server logic.

<a id="s6-3"></a>
### 6.3 Effect references

Effects are referenced through small structs (C28; counts are struct occurrences in every 0x78
and 0x79 set, by holder family):

| Struct | Fields (number of structs that carry them) | Holders |
|---|---|---|
| `EffectGenerator_EffectStruct` (54,832) | `EffectGenerator_EffectID` (DID, all); `EffectGenerator_EffectSpellcraft` (f32, −1 = not given; 54,648); `EffectGenerator_EffectDataList` (9,710); `EffectGenerator_EffectSpellcraftFloor` and `_SpellcraftCeiling` (2,876) | non-combat objects 47,531, effects 6,604, traits 697 |
| `Skill_EffectData` (115,783) | `Skill_Effect` (DID, all); `Skill_EffectSpellcraft` (f32; 112,455); `Skill_EffectDuration` (f32, −1 = the effect's own; 112,455); `Skill_EffectImplementUsage` (enum, mapper 0x230002F9; 115,767) | skills 112,455 (§7.8), effects 3,328 |
| `Skill_Toggle_Effect_Data` (1,862) | `Skill_Toggle_Effect`, `Skill_Toggle_Effect_Spellcraft` (both in all) | skills 1,862 |
| `Effect_MonsterStartupEffect_Struct` (12,373) | `Effect_StartupEffectID` (all); `Effect_StartupEffectSpellcraft` (687); `Effect_StartupEffectDamageQualifier` (7) | non-combat objects 11,832, effects 532, traits 9 |
| `Effect_WSLEffect` (bare DID, 4,954) | | effects 4,954 (combo and dispel lists) |

None of the 54,832 `EffectGenerator_EffectID` values inside `EffectGenerator_EffectStruct`
is 0. The same property also occurs outside that struct, so over every 0x78 and 0x79 set it
occurs 55,825 times: 54,832 inside `EffectGenerator_EffectStruct`, 741 inside
`EffectGenerator_OnRemoval_Effect` (effects 740, non-combat objects 1), 244 inside
`Threshold_State_EffectStruct` (non-combat objects 160, class-1932 monster-play tables 60,
threshold policies 24) and 8 inside `CombatState_StateData_OnEnterStateEffectStruct`
(non-combat objects 8). Of these 55,825 values, 59 are 0, and all 59 are in
`Threshold_State_EffectStruct` structs of a single class-1932 record (§9.2); 0 means "none"
(§3.4) and is skipped. Every non-zero value names an effect. The targets cover 66 of the 87
effect classes, the same 66 whether only the struct or all 55,825 occurrences are counted (none
names a record of classes 706, 716, 748, 752, 761, 762, 779, 1787, 2027, 2231, 2259, 2441, 2477, 2544, 2676, 3099, 3690, 3833, 3839, 3912
or 3943).

> **OPEN:** (implementation census, 2026-10-07) The 4,954 bare `Effect_WSLEffect` values in
> effects include 2,649 zero values (2,305 non-zero). Are the zeros placeholders in the sense of
> §3.4 (skipped by readers), as the implementation assumes?

<a id="s6-4"></a>
### 6.4 Vital-change amounts (layout Confirmed, meaning Reported)

Vital effects describe an amount with a family of properties sharing a stem *S* (for example
`Effect_InstantVital_InitialChange`, `Effect_VitalOverTime_InitialChange`,
`Effect_VitalOverTime_ChangePerInterval`):

| Property | Type | Meaning (Reported, LC) |
|---|---|---|
| *S*`Constant` | f32 | Fixed amount |
| *S*`Progression` | DID | Amount from a progression at the spellcraft or level index |
| *S*`Random` | struct `Effect_RandomValueMin`, `_Max` | Uniform random amount |
| *S*`Variable` | struct `Effect_VariableMin`, `_Max`, `_SpellcraftMin`, `_SpellcraftMax` | Amount interpolated by spellcraft |
| *S*`Variance` | f32 0.05–1 | Random spread applied to the amount |
| *S*`_ModifierList`, `…Additive…_ModifierList` | PID arrays | §3.3 |
| `…_Critical_Multiplier`, `…_Critical_Multiplier_ModList` | f32 1–2, PIDs | Multiplier on a critical result |
| `Effect_BaseVital_VitalType` / `Effect_VitalOverTime_VitalType` | enum, mapper 0x23000003 | Which vital: 1 morale (health), 2 power, 3 and 4 mount vitals |
| `Effect_InstantVital_Multiplicative` | bool | The amount is a fraction of the vital, not points |
| `Effect_DamageType` | enum, mapper 0x23000028 | Damage type (keys are bit values) |

Co-occurrence in instant-vital effects (class 725, 1,103): progression only 529, progression and
variance 259, constant only 226, variable only 52, constant and progression 19, constant and
variable 5, progression and variable 4, constant and variance 3, random 1, progression and random
1, none 4. In vital-over-time effects (class 755, 2,438): initial and per-interval progressions
1,651, per-interval progression only 716, initial and per-interval constants 32, initial
progression with per-interval constant and progression 29, and 10 others.

> **OPEN:** Which source wins when both a constant and a progression are present (19 + 29 + 2
> records). LC prefers the constant (Reported).

> **OPEN:** (implementation census, 2026-10-07) The class-755 co-occurrence counts (1,651, 716,
> 32, 29, 10) are reproduced only when *S*`Variance` is left out of the grouping (for class 725
> it is counted). Please state the grouping rule. Also, the labels behind
> "*S*`_ModifierList`, `…Additive…_ModifierList`" and "`…_Critical_Multiplier`,
> `…_Critical_Multiplier_ModList`" are not spelled out, so the implementation does not read
> them; please list the full labels per stem.

<a id="s6-5"></a>
### 6.5 Class-specific data that carries values

Counts are records of the class that carry the property, out of the class's records (C29).

| Class (records) | Values | Properties |
|---|---|---|
| Any of 14 classes | stat changes | `Mod_Array` (§5), 16,947 records: 734 (14,475 of 14,475), 708 (1,245 of 1,245), 2459 (719 of 722), 713 (190 of 666), 736 (125 of 356), 3686 (62), 3222 (49), 3218 (40), 780 (29), 3842 (5), 752 (4), 748 (2), 764 (1), 3690 (1). All holder classes except 708 are 734 or descend from it; not every record of a descendant has one. |
| 709 (617) | aura | `Effect_Aura_Type` (617; mapper 0x23000128, 3 keys; value 1 in 596, 2 in 21), `Effect_Aura_Radius` (602; 1.3–120), `Effect_Aura_Applied_Effect_Array` (617; 734 `Effect_Aura_Applied_Effect_Data` structs with an effect DID and a spellcraft −1–130), `Effect_Aura_Spellcraft_Override` (596; −1–60), `Aura_ShouldAffectCaster` (617), `Effect_Aura_AffectsPlayers` (405), `Effect_Aura_AffectsMonsters` (304) and four more `Effect_Aura_Affects*` flags (5–42) |
| 2762 (453) | area | `Effect_Area_MaxRange` (453; 1–120), `Effect_Area_MaxTargets` (452; 1–50), `Effect_Area_Applied_Effect_Array` (453; 578 `Effect_Area_Applied_Effect_Data` structs), seven `Effect_Area_Affects*` flags (9–294), `Effect_Area_ShouldAffectCaster` (279), `Effect_Area_MaxTargets_AdditiveModifiers` (110) |
| 708 (1,245) | periodic application | `Effect_ApplyOverTime_Applied_Effect_Array` (1,245; 1,365 `Effect_ApplyOverTime_Applied_Effect_Data` structs), `Effect_ApplyOverTime_Initial_Applied_Effect_Array` (819; 1,070 structs of the same kind) |
| 3686 (461) | skill proc | `Effect_SkillProc_SkillTypes` (434; 64-bit bitfield, mapper 0x230003A4), `Effect_SkillProc_RequiredCombatResult` (317; 32-bit bitfield, mapper 0x230003A8), `Effect_SkillProc_ProcProbability` (259; 0–0.8), `Effect_MinTimeBetweenProcs` (269; 0.25–180 s), `EffectGenerator_SkillProc_UserEffectList` (380), `EffectGenerator_SkillProc_TargetEffectList` (92) |
| 3222 (88) | absorb | `Effect_Bubble_VitalType` (88; mapper 0x23000003), `Effect_Bubble_Value` (42; 200–20,000,000), `Effect_Bubble_Value_Progression` (8), `Effect_Bubble_Percentage` (38; 0.05–0.55), `Effect_BubbleValue_ModifierList` (88) |
| 731 (86) | pips | `Effect_Pip_Type` (86; mapper 0x23000071), `Effect_Pip_AdjustmentAmount` (84; −20–1,000), `Effect_Pip_AdjustmentType` (84), `Effect_Pip_Reset` (2) |
| 767 (1,189) | combinations | `Effect_Combo_EffectPresentList`, `_EffectClassPresentList`, `_RemoveOldEffectIfPresent`, `_RemoveAllOldEffectsIfPresent`, `_CasterOnly` (all 1,189); `_EffectToAddIfPresent` (979), `_EffectToAddIfNotPresent` (747), `_EffectToExamine` (189), `_EffectToGiveBackIfPresent` (173), `_EffectToGiveBackIfNotPresent` (76) |
| 3124 (14) | persistent combinations | `Effect_Combo_EffectPresentList`, `_EffectClassPresentList`, `_CasterOnly` (all 14); `Effect_PersistentCombo_EffectsToAddIfAbsent_Array` (10), `Effect_PersistentCombo_EffectsToAddIfPresent_Array` (9) |
| 3866 (526) | tiers | `Effect_TierUp_EffectList` (526; 3,141 `EffectGenerator_EffectStruct`), `Effect_TierUp_FinalEffect` (462; an array of one `EffectGenerator_EffectStruct` each), `Effect_TierUp_ShowInExamination` (416) |

> **OPEN:** (implementation) The fields of `Effect_Aura_Applied_Effect_Data`,
> `Effect_Area_Applied_Effect_Data` and `Effect_ApplyOverTime_Applied_Effect_Data` ("an effect DID
> and a spellcraft") are not named, so the implementation's effect-reference reader (§6.3) does
> not recognise these structs. Which labels hold the effect DID and the spellcraft?

<a id="s6-6"></a>
### 6.6 Equivalence classes and stacking

`Effect_EquivalenceClass` names one of 3,081 classes (mapper 0x23000005, keys 0–3,270). The
effect control object (class 1666, one record, 0x70000225 here, named in the WEENIECONTENT map
0x2800000E of `wstate.md` §2) holds
`Effect_EqvClassData_Array` with 728 structs `Effect_EqvClassData_Struct`:

| Field | Type | Present | Values |
|---|---|---|---|
| `Effect_EquivalenceClass` | enum | 728 | the class described |
| `Effect_EqvClassData_MaxDepth` | int | 459 | 1–201. Most common values: 24 (280 structs), 3 (55), 2 (36), 4 (29), 12 (14), 1 (11), 5 (10); the other 24 structs hold 6, 8, 10, 20, 25, 51, 55 or 201 |
| `Effect_EqvClassData_MaxExpressedEffs` | int | 144 | 1–10; 1 in 102 structs |
| `Effect_EqvClassData_IsPerCaster` | bool | 293 | true 210, false 83 |
| `Effect_EqvClassData_SorterOverride` | DID | 43 | a non-combat object (sorter) |
| `Effect_EqvClassData_DurationModifier_Source`, `_Target` | PID | 4 each | |

The object also holds `Effect_Control_DefaultSorter` (DID), `Effect_Control_SpellcraftCapProgression`
(DID) and `Effect_Control_ResistCategoryToUIState_Array` (11 entries).

> **OPEN:** The stacking rule. The names suggest: at most *MaxDepth* effects of one class held
> at once, at most *MaxExpressedEffs* of them active, ordered by a sorter, counted per caster
> when *IsPerCaster*; `Effect_ClassPriority` and `Effect_Cap` take part. None of this is stated
> by the data or a source with authority. Server logic.

<a id="s6-7"></a>
### 6.7 The COMBATEFFECTS map

DID map 0x2800002E (root label COMBATEFFECTS, `property-system.md` §8.1): 27 entries in two
groups, key 0 the null sentinel, the other 26 naming effects of classes 734 (10) and 718 (16).
No property enumerates its keys; code looks entries up by label. Labels are identifiers and are
read at run time.

---

<a id="s7"></a>
## 7. Skills

<a id="s7-1"></a>
### 7.1 Overview (Confirmed)

All 13,247 skills have root class 827 (no subclasses). 56 properties occur in every skill, so
a skill card can rely on them; a further 42 occur in more than 5,000.

<a id="s7-2"></a>
### 7.2 Identity and display

`Skill_Name`, `Skill_Desc` (StringInfo); `Skill_Icon`, `Skill_LargeIcon`, `Skill_SmallIcon`
(DID 0x41); `Skill_SkillType` (64-bit bitfield, mapper 0x230003A4, 65 keys);
`Skill_Category` (5,025, mapper 0x2300001A); `Skill_DisplaySkillType` (1,123, any-width
bitfield); `Skill_Harmful` (bool); `Skill_IsMonsterSkill` (8,046);
`UI_Examination_Tooltip_*` (5,201).

<a id="s7-3"></a>
### 7.3 Costs

`Skill_VitalCostList` and `Skill_Toggle_VitalCostPerSecondList` (12,625 each) are arrays of
`Skill_VitalInfo` structs:

| Field | Type | Meaning |
|---|---|---|
| `Skill_Vital_Type` | enum, mapper 0x23000003 | Vital paid |
| `Skill_Vital_Points` | f32 (0–420; 0–50 per second) | Flat cost |
| `Skill_Vital_Points_Progression` | DID | Cost from a progression (4,513 non-zero) |
| `Skill_Vital_Percent` | f32 0–0.95 | Cost as a fraction of the maximum (Reported) |
| `Skill_Vital_Consumes_All` | bool | Spends the whole vital |
| `Skill_Vital_Mod_Array` of `Skill_Property_Mod` | PIDs | §3.3 |

The 12,625 cost lists hold 26,117 `Skill_VitalInfo` entries in total. `Skill_VitalMaxList` (4,642) has the same
struct with −1 values. `Skill_Executor_RequiredVital_Array` (5,025) lists vital preconditions
(`_Type`, `_Operator` with mapper 0x23000039, `_Value`).

<a id="s7-4"></a>
### 7.4 Ranges and area

| Property | Type | Present | Range |
|---|---|---|---|
| `Skill_MinRange`, `Skill_MaxRange` | f32 world units | all | −1–80, −1–300 (−1 = none) |
| `Skill_MaxRange_ModifierArray` | PIDs | 914 | |
| `Skill_AreaEffectMaxTargets` | int | all | −1–32 |
| `Skill_AreaEffectDetectionAnchor` | enum, mapper 0x23000054 (5 keys) | all | |
| `Skill_AEDetectionVolume_SphereRadius` | f32 | 1,882 | 1.5–360 |
| `Skill_AEDetectionVolume_ArcRadius`, `_ArcDegrees` | f32 | 1,121 | 3–120, 20–180 |
| `Skill_AEDetectionVolume_BoxLength`, `_BoxWidth` | f32 | 132 | 5–80, 0.7–12 |
| `Skill_AEDetectionVolume_HeadingOffset` | f32 degrees | 60 | −180–330 |
| `Skill_PositionalHeading`, `Skill_PositionalSpread` | int degrees | all | −1–180, 0–130 |
| `Skill_Target_PositionalHeading`, `_PositionalSpread` | f32 degrees | 8,490 | |

A skill has at most one detection volume (sphere 1,882, arc 1,121, box 132; never two),
matching the compiled objects of §3.2.

<a id="s7-5"></a>
### 7.5 Target selection and preconditions

In every skill: `Skill_TargetSelectionMode` (enum, mapper 0x23000053, 6 keys);
`Skill_ValidTargetPlayer`, `_ValidTargetPet`, `_ValidTargetNPC`, `_ValidTargetMonster` (bool);
`Skill_RequiresFacing`, `Skill_UsableWhileMoving`, `Skill_RequiredFellow`,
`Skill_RequiredDefeated`, `Skill_MustBeStealthed` (with `_Override`), `Skill_Allowed_When_Violent`,
`Skill_Allowed_When_Not_Violent` (each with an `_Override` PID), `Skill_AllowedWhileSwimming`;
implement requirements `Skill_AllowedRHImplement`, `Skill_AllowedLHImplement`,
`Skill_AllowedRangedImplement` (enums). Frequent: `Skill_RequiredTargetEquivClass` and
`Skill_BarringTargetEquivClass` (mapper 0x23000005, with `_MustBeSource`),
`Skill_RequiredEffectList` (1,542) and `Skill_BarringEffectList` (3,638) of `Skill_Effect`
DIDs, `Skill_RequiredTrait` (815, trait DID), `Skill_TargetMustBeInCombat`,
`Skill_TargetMustNotBeInCombat`, `Skill_Target_RestrictedCombatStates` (bitfield),
`Skill_AllowedWhileMounted` (5,164), `Skill_MustBeGrounded` (1,847).

<a id="s7-6"></a>
### 7.6 Hit rules: miss, evade, parry, block, resist, line of sight

| Property | Present | Type and range |
|---|---|---|
| `Skill_CanMiss`, `Skill_MissChanceModifier` | all | bool; f32 −0.15–0.25 |
| `Skill_MovementMissChanceModifier` | 5,006 | f32 0–1 |
| `Skill_CanBeEvaded`, `Skill_EvadeChanceModifier`, `Skill_EvadePointsModifier`, `_Progression`, `_Mod_Array` | all (array 2,056) | bool; f32 −0.2–5; f32 (all 0); DID; PIDs |
| `Skill_CanBeParried`, `Skill_ParryChanceModifier`, `Skill_ParryPointsModifier*` | 13,228 | as above |
| `Skill_CanBeBlocked`, `Skill_BlockChanceModifier`, `Skill_BlockPointsModifier*` | 13,228 | as above (chance −0.2–50) |
| `Skill_Resist_Category` (bitfield, mapper 0x23000064), `Skill_Resist_Chance_Modifier`, `Skill_Resist_Points_Modifier*` | 5,006 | |
| `Skill_RequiredLineOfSight`, `Skill_RequiredAreaEffectLineOfSight` | all | bool |
| `Skill_BreaksSourceStealthOnUse`, `Skill_BreaksTargetStealthOnUse`, `Skill_Threat` | all | bool |

The data gives switches and modifiers. The base chances they modify come from the rating
curves (§10.1) and the actors' state; the combination is server logic (§13.1).

<a id="s7-7"></a>
### 7.7 Attack hooks

`Skill_AttackHookList` (13,243 skills; 13,733 structs `Skill_AttackHookInfo`;
`Skill_AttackHook_AnimationHookIndex` 1–4). Fields that carry values:

| Field | Type | Present / range |
|---|---|---|
| `Skill_AttackHook_HookDamageMax` | f32 | all; 0–10,000 |
| `Skill_AttackHook_HookDamageMax_Progression` | DID | all (6,365 non-zero) |
| `Skill_AttackHook_HookDamageMaxVariance` | f32 | all; 0–0.5 |
| `Skill_AttackHook_HookDamageModifier` | f32 | all; 0–1,500 |
| `Skill_AttackHook_DPSAddMod_Progression`, `_DamageAddContributionMultiplier`, `_ImplementContributionMultiplier` | DID, f32 0–60, f32 0–60 | 4,837 |
| `Skill_AttackHook_DamageType` | enum, mapper 0x23000028 | all |
| `Skill_AttackHook_DamageTypeOverride` | PID | 13,108 |
| `Skill_AttackHook_DamageQualifier` | enum, mapper 0x23000029 (4 keys) | all |
| `Skill_AttackHook_ChanceCriticalHit` | f32 −0.5–100 | all |
| `Skill_AttackHook_CriticalDamageMultiplier`, `_SuperCriticalDamageMultiplier` | f32 1–6, 0.5–6 | all, 13,714 |
| `Skill_AttackHook_PositionalDamageMultiplier` | f32 −1–4 | all |
| `Skill_AttackHook_MitigationModifier` | f32 −1–0.3 | all |
| `Skill_AttackHook_AdditionalThreatValue`, `_Progression`, `_AdditionalThreatType` | f32 0–50,000, DID, enum | 13,714 |
| `Skill_AttackHook_VsMounted_Damage_Multiplier`, `_VsUnMounted_Damage_Multiplier` | f32 | 455 |
| `Skill_AttackHook_UsesPrimaryImplement`, `_UsesSecondaryImplement`, `_UsesRangedImplement`, `_UsesNatural`, `_UsesTactical` | bool | all (tactical 13,347) |
| `Skill_CanCriticalHit` | bool | all |
| `…_Mod_Array` of `Skill_Property_Mod` for each numeric field | PIDs | §3.3 |
| `Skill_AttackHook_TargetEffectList`, `_CriticalTargetEffectList`, `_SuperCriticalTargetEffectList`, `_PositionalTargetEffectList` | arrays of `Skill_EffectData` | all, with `_Override` and `_AdditiveArray` companions |

<a id="s7-8"></a>
### 7.8 Linked effects

| List | Applied to (Reported, LC) | Present |
|---|---|---|
| `Skill_UserEffectList` (+ `_Override`, `_AdditiveArray`) | the user | 2,739 (override 4,786, additive 885) |
| `Skill_CriticalEffectList` (+ `Override`, `_AdditiveArray`) | the user on a critical | 159 (4,403, 280) |
| `Skill_Toggle_Effect_List` | the target while toggled | 1,588 |
| `Skill_Toggle_User_Effect_List` | the user while toggled | 43 |
| hook lists of §7.7 | the target on hit, critical, devastating hit, positional hit | |

`Skill_Effect` in skills (C28): inside the 112,455 `Skill_EffectData` structs, 15,187 non-zero
references and 97,268 placeholders (0); further non-zero values as bare list elements in
`Skill_BarringEffectList` (5,687), `Skill_RequiredEffectList` (1,562),
`Skill_ConsumedEffectList` (119) and `Skill_PreSkillBarringEffectList` (6). The 3,328
`Skill_EffectData` structs held by effects add 779 non-zero values and 2,549 placeholders. Toggle
effects point mainly at classes 734, 2459, 718 and 708.

<a id="s7-9"></a>
### 7.9 Timing

Counts are skills that carry the property, out of 13,247 (C30).

| Property | Type | Present | Values |
|---|---|---|---|
| `Skill_ActionDurationContribution` | f32 seconds | all | 0–10 |
| `Skill_SkillRecoveryTime` | f32 seconds | all | 0–864,000 (cooldown, Reported) |
| `Skill_SkillRecoveryTime_Mod_Array` | PIDs (§3.3) | 1,124 | never empty |
| `Skill_RecoveryChannel` | enum, mapper 0x2300006C (321 keys, 0–374) | 1,247 | 2–374; a shared-cooldown channel (Reported) |
| `Skill_IgnoresResetTime` | bool | 4,642 | true 1,995 |
| `Skill_Immediate` | bool | 4,642 | true 146 |
| `Skill_InductionAction` | DID | 3,494 | every value names an induction object (class 1195) |
| `Skill_Toggle_ChannelingState` | DID | 171 | every value names a channelling object (class 961) |
| `Skill_Toggle_ChannelingDurationOverride` | f32 seconds | 45 | 0.04–9,999,999 |
| `Skill_Toggle_Hook_Number` | int | 1,578 | always 1; marks a toggle skill (Reported) |

**Induction objects (class 1195)**: 1,779 records, ancestry 486 (no combat family of §2).
Every record has `Induction_Duration` (f32 seconds, 0.1–60), read by LC as the cast time
(Reported), and `Induction_Interruptable`, `Induction_AllowedWhileMoving`,
`Induction_AllowedInCombat` (bool). Optional: `Induction_Duration_AdditiveModifierList` (54) and
`Induction_Duration_MultiplierModifierList` (509), PID lists (§3.3).

**Channelling objects (class 961)**: 401 records, ancestry 486. Every record has
`Channeling_Duration` (f32 seconds, 1–99,999) and `Channeling_Duration_Multiplier_PropertyName`
(PID, §3.3). Their other properties (barred actions, abandon rules, scripts) belong to a later
class-mechanics spec.

<a id="s7-10"></a>
### 7.10 Pips, gambits and combinations

Pips: `Skill_Pip_AffectedType` (710; mapper 0x23000071), `Skill_Pip_Change` (−100–50),
`Skill_Pip_RequiredMinValue`/`MaxValue`, `Skill_Pip_Toward_Home` (0–25),
`Skill_Toggle_PipChangePerInterval`, `Skill_Toggle_SecondsPerPipChange`, with modifier arrays.
Combinations: `Skill_Combo_ComboList` (509) of `Skill_Combo_ComboListEntry` (combo skill DID,
`Skill_Combo_ComboMode`, and a per-class combo-state bitfield), `Skill_Combo_StateProperty`
(PID). Gambits (`Skill_AppliedGambit`, `Skill_RequiredGambit`, `Skill_RemovedGambits`) pack
icon codes four bits at a time (Reported, LC). Details are left to a later class-mechanics spec.

---

<a id="s8"></a>
## 8. Traits

<a id="s8-1"></a>
### 8.1 Classes (Confirmed)

| Class | Records | Role (this spec) | Distinctive properties |
|---|---|---|---|
| 1477 | 1,773 | General trait | `Trait_Cosmetic`, `Trait_Exclusion_Group` (mapper 0x23000150), `Trait_Priority` |
| 3438 | 946 | Point-based trait (trait-tree cell) | `Trait_PointBasedTrait_PointCostProgression` (float progression), `Trait_Prerequisite_CharacterLevel`, `_MountLevel`, `Trait_PointBasedTrait_Only_One_Tree_Active`, rank properties |
| 3509 | 767 | Appearance trait | `Trait_Appearance_*` |
| 2525 | 244 | Skirmish trait | `Skirmish_TraitCostProgression`, `Skirmish_TraitEffect_PetMods` |
| 1483 | 125 | Trait granting skills | `Trait_Skill_Array` |
| 3564 | 45 | Ranked trait | rank properties only |
| 1494 | 37 | Virtue | `Trait_Virtue_*`, `SpellcraftCalculator` |
| 1478 | 25 | Granted trait | `Trait_Granted_Category` |
| 3712 | 1 | Ranked trait with rank effects | `Trait_EffectSkill_AtRank*` |

<a id="s8-2"></a>
### 8.2 Common properties

In all 3,963: `Trait_Name`, `Trait_Description`, `Trait_Tooltip` (StringInfo), `Trait_Nature`
(mapper 0x23000057, 34 keys), `Trait_Category` (mapper 0x2300001A), `Trait_Tier` (int),
`Trait_Cost_To_Slot` (int). Frequent: `Trait_Icon` (3,950), `Trait_Minimum_Level` (3,196),
`Mod_Array` (2,262; §5), `Trait_SuppressionToggles` (1,794), `Trait_Groups` (1,273),
`EffectGenerator_TraitEffectList` (555).

<a id="s8-3"></a>
### 8.3 Ranks

`Trait_Virtue_Maximum_Rank` (1,273), `Trait_Virtue_Rank_PropertyName` (1,271; the actor
property that stores the rank), `Trait_Rank_Overlay_Progression` (1,260; property-array
progression of icons, 1,089 non-zero), `Trait_Static_Icon_Overlay`, `Trait_First_Purchasable_Rank`.
Rank-dependent grants: `Trait_EffectSkill_AtRankEffects_Array` (233) and
`Trait_EffectSkill_AtRankSkillsAcquired_Array` (275) with `_Rank` fields.

<a id="s8-4"></a>
### 8.4 Prerequisites

`Trait_Prerequisites_Array` (145), `Trait_Prerequisites_Logic` (206) with `AllOf`, `OneOf` and
`NoneOf` lists of `Trait_Prerequisite` DIDs, `Trait_Prerequisite_TraitRankStructArray` (77).

<a id="s8-5"></a>
### 8.5 Trait trees (class 3486)

17 records, one per tree. `Trait_TraitTree_TreeType` (mapper 0x23000369, 18 keys) names the
tree. `Trait_TraitTree_TraitArray` holds 1,216 structs `Trait_TraitTree_TraitStruct`:

| Field | Type | Meaning |
|---|---|---|
| `Trait_TraitTree_Trait` | DID | A class-3438 trait (all 1,216) |
| `Trait_TraitTree_Branch` | enum, mapper 0x230003A1 (57 keys) | Branch of the tree (1,201 present) |
| `Trait_TraitTree_TraitLocation` | enum, mapper 0x2300036E | Cell (below) |
| `Trait_TraitTree_TraitDependencyArray` | structs of `Trait_TraitTree_TraitDependency` (cell) and `_TraitDependencyRank` (1–5) | 511 dependencies |

**Cell keys** (mapper 0x2300036E, 106 keys): 0, then *a* × 65,536 + *b* for *a* = 1–8 and
*b* = 1–12, then 0x10000000 + *n* for *n* = 0–8. The key's label in the mapper is the two
numbers joined by an underscore (an interoperability fact; a viewer can lay cells out from the
key alone).

> **OPEN:** Which of *a* and *b* is the row and which the column, and what the 0x10000000 group
> is. A tree view needs it; LC passes the labels through without interpreting them.

Further tree fields: `Trait_TraitTree_SpecializationsArray` (13 trees, 39 entries: a branch and
`Trait_TraitTree_SpecializationProgression`, a class-3800 sparse DID progression of traits,
§4.6), `Trait_TraitTree_BranchDescriptionArray`, `Trait_TraitTree_RowToRequiredBranchRankProgression`
(12, float progressions), `Trait_TraitTree_RowToRequiredTreeRanksProgression` (1).

<a id="s8-6"></a>
### 8.6 Trait control object (class 1486)

One record of class 1486 (0x7000025B in this install), named in the WEENIECONTENT DID map
(0x2800000E, `wstate.md` §2); a reader finds it through that map, not by a fixed DID (C31).
Values relevant to combat:

- six progression DIDs, each naming a progression: `Trait_Control_LevelToTotalClassTraitPointsProgression`,
  `Trait_Control_ClassTraitPointsToRefundCostProgression`,
  `Trait_Control_TraitRank_To_Level_Equivalent`, `Trait_Control_LevelScaling_LevelToVirtueRank_Min`
  and `_Max`, `Trait_Control_VirtueTierToItemLevelProgression`;
- arrays (element counts in this install): `Trait_Control_PointBasedTraitNature_Array` (15),
  `Trait_Control_TraitGroup_To_Slot` (7), `Trait_Control_Group_Channels` (19),
  `Trait_Control_TraitStateToUIStateArray` (6), `Trait_Control_RaceArray` (8),
  `Trait_Control_Nature_To_Earned_Message_Array` (17);
- trait DID lists of `Trait_Control_TraitDID` elements, every element naming a trait:
  `Trait_Control_FreepTraits` (22), `Trait_Control_CreepTraits` (14),
  `Trait_Control_SkirmishTraits` (236);
- two virtue experience caps (int): `Trait_Control_VirtueXP_Bonus_SoftCap` and
  `Trait_Control_VirtueXP_Bonus_HardCap`;
- `Trait_Control_VirtueTierToExperience_AdvancementTable` (DID).

---

<a id="s9"></a>
## 9. Class and advancement tables

<a id="s9-1"></a>
### 9.1 Class advancement tables (class 853)

12 records, one per playable class, listed by the level-table directory (class 855,
`AdvTable_LevelTableList/AdvTable_LevelTable`). Contents (all 12 unless noted):

| Property | Shape | Meaning |
|---|---|---|
| `AdvTable_Class` | enum | The class |
| `AdvTable_ClassInfo` | struct of names, description, icons | Display |
| `AdvTable_LevelEntryList` | 160 `AdvTable_LevelEntry` per record (levels 1–160) | Per level: `AdvTable_BaseVitalEntryList` (4 entries, `AdvTable_VitalType` keys 1–4 with `AdvTable_BaseVitalValue`) and `AdvTable_MaxStatEntryList` (7 entries, `AdvTable_StatType` keys 1–7 of mapper 0x2300003E with `AdvTable_StatValue`) |
| `AdvTable_DerivedStat_Configuration` | 37 `AdvTable_DerivedStat_Formula` per record | For each derived stat (`AdvTable_DerivedStat`, keys 1–37 of mapper 0x23000332), a list of 7 pairs (`AdvTable_DerivedStat_Formula_Stat`, keys 1–7 of 0x2300003E; `AdvTable_DerivedStat_Formula_Value`, f32 0–4.5) |
| `AdvTable_AvailableSkillEntryList`, `AdvTable_AvailableMountSkillEntryList` | 551 and 36 entries over the 12 records | Skill DID, level, prerequisite skill, rank, training cost, shortcut index |
| `AdvTable_ClassCharacteristic_List` | 297 entries over the 12 records | Trait DID (`AdvTable_Trait_WC`) and level |
| `Trait_SlotSchedule_Nature_Array` | 132 entries over the 12 records | Trait nature and a slot-schedule DID |
| `PipConfig_Pip_List` | 1 or 2 `Pip_Type` | The class's pip resources |
| `AdvTable_*Control*` floats | 29 properties | Per-class coefficients (for example might-to-melee offence, agility-to-critical) |
| `AdvTable_ArmorDefense_Points_CalcType` and two siblings | enums, mapper 0x23000331 | Which rating curve the class uses for armour (§10.1) |
| `Combat_DamageType_Base_Mitigation_Multiplier_*` | f32 | Base mitigation per damage type |
| `XP_EarnedPointsToLevel_Table`, `AdvTable_DPPerLevelProgression`, `AdvTable_BaseCombatEventTable` and others | DIDs | Experience and event tables |

Reported reading of the derived-stat formulas (consistent with the layout): the derived stat
equals the sum over the 7 pairs of coefficient × the actor's primary stat of that key.

> **OPEN:** (§18 Q13) Whether the derived-stat formula is complete: whether further terms
> (level, the class's `AdvTable_*Control*` coefficients, item bonuses) are added, and in which
> order. This is server logic; M3 observation of the character panel could test the reading
> above.

> **OPEN:** (implementation) The field labels of `AdvTable_AvailableSkillEntryList` and
> `AdvTable_AvailableMountSkillEntryList` entries ("skill DID, level, prerequisite skill, rank,
> training cost, shortcut index"), of `MonsterPlay_SkillList` and `MonsterPlay_TraitList` entries
> (§9.2) and of `Trait_Skill_Array` elements (§8.1) are not given. The implementation offers every
> DID inside these lists and nothing else; the labels would let it type the entries.

<a id="s9-2"></a>
### 9.2 Monster-play tables (class 1932)

67 records: `MonsterPlay_MonsterInfo` (names, class, subspecies, icons),
`MonsterPlay_MonsterLevel` (1–160), `MonsterPlay_SkillList` (525 entries: skill, cost, minimum
level, minimum rank), `MonsterPlay_TraitList` (687), `MonsterPlay_VirtueList` (84), default
melee and ranged skills, partial-mitigation bases and armour calc types as in §9.1.

<a id="s9-3"></a>
### 9.3 Advancement lines (classes 2390, 2399)

74 records holding `AdvLine_EntryList` of `AdvLine_Entry/AdvLine_DataID`: class 2390 lines
name skills (120 entries), class 2399 lines name traits (154 entries). The advancement-line
directory (`AdvLine_Directory_LineList`) lists them. Order within a line is the stored order.

---

<a id="s10"></a>
## 10. Combat control, lookup tables, thresholds and battle statistics

<a id="s10-1"></a>
### 10.1 Combat control object (class 1659)

One record (0x70000235 here; WEENIECONTENT label). Its gameplay tables:

| Property | Shape | Meaning |
|---|---|---|
| `Combat_Control_CalcControlInfoArray` | 30 `Combat_Control_CalcControlInfoStruct` | One per key 1–30 of mapper 0x23000331 (`Combat_Control_CalculationType`): three float-progression DIDs, `Combat_Control_Rating`, `Combat_Control_Target_Cap`, `Combat_Control_Hard_Cap`, all indexed by level |
| `Combat_Control_DamageTypeData_List` | 15 entries | Per damage type: PIDs of the actor properties holding mitigation percentage, immunity, armour points (class, stat, modifier, total) and additional base mitigation; `Combat_Control_MasterDamageType` groups types into two master types |
| `Combat_Control_MasterDamageTypeData_List` | 2 entries | The same PIDs for the two master types |
| `Combat_Control_DamageCriticalMapList` | 3 entries | Per damage qualifier: PIDs of the critical chance and critical points properties |
| `Combat_Control_WeaponStrikeModifierList` | 14 entries | Per equipment category (64-bit bitfield): PID of the weapon damage multiplier property, and for one category critical-multiplier PIDs |
| `Combat_Control_SlayerDPSModList` | 4 entries | Per quality: a DPS-modifier progression |
| `Combat_Control_ValidPlayerAttackResultArray` | 4 entries | Attack results (mapper 0x2300002A) |
| Scalars | | `Combat_Control_MaxCombatLevel` (int), duel timings (s), induction interruption cooldown (s), mounted conversion factors |
| `Combat_Control_*EquationLevelAdjustment_Progression` | 4 DIDs | Level-adjustment progressions |

Reported rating curve (LC, unverified): for rating *R* at level *L*, the percentage is
TargetCap(*L*) / (1 + Rating(*L*) / *R*), capped at HardCap(*L*), where the three are the
progressions of the entry.

> **OPEN:** (§18 Q12) Whether this is the original's rating-curve formula, including what
> happens at *R* = 0 and how the hard cap applies. It is Reported only (LC); the client
> library's bytecode (§13.2) holds the tooltip version, and the server's version is not in the
> data.

<a id="s10-2"></a>
### 10.2 DPS and value lookup tables

Counts cover all records of both classes and every 0x78 and 0x79 set for the holders (C32).
Each record holds exactly the two properties named, nothing else (no minimum-index property).

| Class | Records | Properties | Held by |
|---|---|---|---|
| 1660 | 8 | `Combat_BaseDPSArray` of `Combat_BaseDPS` (f32, 1–1,844.64): **701 elements in each of the 8 records** (5,608 in total). `Combat_QualityModArray` of 5 `Combat_QualityModData` in each record (`Combat_Quality` keys 1–5, `Combat_DPSMod` 1–1.12) | `Combat_DPS_LUT`: 23,204 values, every one naming a class-1660 record (none is 0) |
| 1688 | 99 | `Item_BaseValArray` of `Item_BaseValStruct/Item_BaseVal` (f32, 0.1–1,425,000): **601 elements in each of the 99 records** (59,499 in total). `Item_QualityArray` of 5 `Item_QualityStruct` in each record (`Item_Quality` keys 1–5, `Item_QualityModVal` 0.5–10) | `Item_ValueLookupTable`: 150,622 values, 145,518 naming a class-1688 record and 5,104 equal to 0 (none) |

In every record of both classes the five quality structs are stored in key order 5, 4, 2, 3,
1. A reader looks a quality up by its key, never by position.

Reported (LC): value at item level *L* and quality *q* = base array element *L* (1-based,
first element at level 1) × the quality factor of *q*; for weapons, maximum hit damage =
2 × DPS / (2 − `Combat_DamageVariance`) × weapon speed, and minimum = maximum ×
(1 − variance).

<a id="s10-3"></a>
### 10.3 Threshold policies (class 3531)

6 records, each a `Threshold_PercentageValue_Policy` struct: `Threshold_Policy_CurrentStateIndexProperty`
(PID; all name one health-threshold index property) and `Threshold_Policy_StateList` of 2–10
`Threshold_EffectScript_State` structs (`Threshold_State_MinValue_Float`,
`_MaxValue_Float`, 0.1–0.9; `Threshold_State_EffectList` of effect DIDs). Referenced by
`Vital_HealthThresholdPolicySingleton` (22 objects) and from 2 modifier entries. Reported
reading: while the vital fraction lies in a state's band, that state's effects apply.

<a id="s10-4"></a>
### 10.4 Battle statistics (classes 3590, 3622)

13 records: `BattleStat_Name`, `BattleStat_Description` (StringInfo), `BattleStat_Scope`
(mapper 0x23000391; all 3), `BattleStat_UpdatePolicy` (mapper 0x23000392; 1, 2 or 4),
`BattleStat_ReplicateToClients` (9 true), `BattleStat_ForwardUpdatesToStat` (4; DID of another
battle statistic), column overrides for a UI table. DID map 0x28000031 (BATTLESTATS) has 14
entries in two groups: the null sentinel and the 13 records (10 of class 3590, 3 of 3622).

---

<a id="s11"></a>
## 11. Enum mappers (Confirmed)

Read keys and labels at run time; only counts are given. "Users" counts dictionary properties
whose definition names the mapper.

| DID | EMAPPER label | Keys | Users | Used by |
|---|---|---|---|---|
| 0x23000046 | PropertyOp | 12 | 6 | `Mod_Op`, `Mod_ModifierOp` (§5.3) |
| 0x23000331 | CombatCalculationType | 31 (0–30) | 10 | rating curves, armour calc types |
| 0x23000332 | DerivedStatType | 38 (0–37) | 1 | `AdvTable_DerivedStat` |
| 0x2300003E | StatType | 8 (0–7) | 6 | primary stats |
| 0x230003A8 | CombatResult | 16 | 1 | `Effect_SkillProc_RequiredCombatResult` |
| 0x2300002A | CombatResultType | 16 | 6 | attack results, strike data |
| 0x23000045 | EffectCategory | 15 | 2 | examination category of effects |
| 0x23000005 | EffectEquivalenceClasses | 3,081 (0–3,270) | 10 | §6.6, skill target classes |
| 0x230003A4 | SkillType | 65 (0–64) | 2 | `Skill_SkillType`, proc filters |
| 0x2300036E | TraitCell | 106 | 3 | §8.5 |
| 0x23000369 | PointBasedTraitTree | 18 (keys up to 47) | 4 | tree type |
| 0x230003A1 | PointBasedTraitTreeBranch | 57 (keys up to 65) | 9 | branch |
| 0x23000057 | TraitNature | 34 | 12 | |
| 0x23000064 | ResistCategory | 16 | 12 | effect and skill resist |
| 0x23000028 | DamageType | 19 (bit values up to 65,407) | 22 | |
| 0x23000029 | DamageQualifier | 4 | 7 | |
| 0x23000003 | VitalType | 6 | 23 | |
| 0x2300028A | — | 95 | 5 | `Progression_Type` |
| 0x23000289 | — | 10 | 4 | `Progression_PointTier` |
| 0x23000053, 0x23000054 | TargetSelectionType, AreaEffectAnchorType | 6, 5 | 1 each | §7.4, §7.5 |
| 0x23000071 | PipType | 26 (keys up to 29) | 7 | pips |
| 0x23000391, 0x23000392, 0x23000393 | BattleStatScopeType, BattleStatUpdatePolicyType, BattleStatGeneratingEventType | 4, 5, 2 | 1 each | §10.4 (`BattleStat_Scope`, `BattleStat_UpdatePolicy`, `BattleStat_StatEvent`) |

The label column is the mapper's label in the EMAPPER DID map (0x28000001), checked for every
row on 2026-10-07 (C33); labels are interoperability identifiers and are read at run time. "—"
means the mapper has no entry in the EMAPPER map (it is reached only through the properties that
name it).

---

<a id="s12"></a>
## 12. Reference graph (Confirmed)

Census of every DID value in every 0x78 and 0x79 set whose target is a combat object (by holder
family → target family):

| Holder → target | References | Main properties |
|---|---|---|
| other → progression | 475,922 | `Mod_Progression`, item repair and armour tables, item-level overrides, recipe experience |
| other → effect | 81,429 | `EffectGenerator_EffectID`, creature start-up effects, required facility effects, usage restrictions |
| other → skill | 29,780 | AI action chains, conjunction roles, granted skills |
| skill → effect | 24,423 | `Skill_EffectData/Skill_Effect`, barring, toggle and required effect lists |
| effect → effect | 17,864 | generators, periodic applications, dispel and combo lists |
| other → trait | 13,679 | usage requirements, granted traits, trait trees |
| skill → progression | 11,013 | hook damage, vital costs |
| effect → progression | 10,672 | modifier entries, vital amounts |
| trait → progression | 3,594 | modifier entries, rank overlays, point costs |
| skill → skill | 1,887 | combo skills, mount conversion analogues |
| progression → effect | 1,443 | DID arrays, a property array of effect DIDs |
| trait → effect | 1,360 | trait effect lists |
| progression → progression | 1,302 | progressions of progressions, DID arrays |
| trait → skill | 1,199 | bundled and acquired skills |
| class tables → skills and traits | 925 + 734 + 598 + 297 | §9 |
| skill → trait | 815 | `Skill_RequiredTrait` |
| effect → skill | 564 | cooldown and reset lists, granted skills |
| effect → trait | 236 | granted traits |
| threshold → effect | 24 | §10.3 |

No combat-family record holds a DID of a missing object record (0 dangling references; the 703
dangling object references of the install all sit in quest records). Readers still treat a
missing target as "none".

The compiled halves add DID items (`wstate.md` §5.1): effects 1,943, monster-play and advancement
lines 82, class tables 108; skills, traits and progressions have none.

---

<a id="s13"></a>
## 13. What the data does not contain

<a id="s13-1"></a>
### 13.1 The server's combat logic

The WLIB DID map's second entry, which by its position is the server-side class library, points
at a DID that no archive contains (`wlib.md` §2). The client library defines the same classes
but only some of their functions have bodies: for example the effect base class 698 declares
282 functions of which 123 have bodies, the skill class 827 declares 477 with 166 bodies
(C37; method tables read as `wlib.md` §6.4 describes). The data therefore holds the **inputs** of combat (this spec) but not:

- the order and arithmetic in which hit, miss, evade, parry, block, resist and critical results
  are rolled;
- how skill hook damage, effect amounts and costs are scaled and combined;
- how modifier entries from several sources are combined (§5.3);
- stacking and removal rules (§6.6) and duration semantics (§6.2);
- threat.

A server implementation (out of scope before M8; decision 0004) has to define these itself.

<a id="s13-2"></a>
### 13.2 What the client library's bytecode may contain

The client library holds bytecode bodies for the progression classes (1386: 9 own bodies;
1385: 6; 1389: 5; 3918: 8; 2402: 7; 2519: 7; 3810: 3; segments 3917: 6), for the compiled
modifier classes 1339–1342, and for many effect, skill and trait functions. A byte search of
the bytecode chunk (`wlib.md` §4.1) finds the property IDs of `Mod_Op`, `Mod_ModifierOp`,
`Mod_Modified`, `Mod_Progression`, `Mod_ProgressionFloor`, `Mod_ProgressionCeiling`,
`Mod_ModifierList`, `Mod_Array`, `Combat_DPS_LUT` (8 times), `Combat_BaseDPS`,
`Combat_QualityModData`, `Item_ValueLookupTable`, `Item_Armor_Value_Lookup_Table`,
`Progression_Type`, `Progression_PointTier`, `FloatProgression_FloatValue`,
`LinearInterpolatingProgression_ValueProperty` and `Mod_ProgressionProgressionInnerIndex` as
32-bit literals; in all, 5,993 of the 27,546 property IDs occur. So the client evaluates at
least modifier entries, DPS tables and progressions in script, most likely for examination
tooltips. (A 4-byte literal can match by chance, about once in 2,000 IDs for a block of this
size; the IDs named here occur 1 to 18 times.)

> **OPEN:** (needs a bytecode spec) The client-side evaluation of progressions (in and out of
> range), modifier entries and the DPS and value tables is in these bodies. Reading them needs
> the script machine's instruction set, which `wlib.md` §4.1 leaves out of scope and which the
> BIN pass of this spec did not locate (§14). A future pass should start from the library
> loader (found) and the function-call routine named in `wlib.md`'s BIN row.

<a id="s13-3"></a>
### 13.3 Third-party formulas (Reported only)

LC reconstructs some tooltip numbers. They are recorded here as hypotheses for a later BIN or
M3 check, not as facts:

- rating curves (§10.1);
- weapon damage from the DPS table (§10.2);
- skill hook maximum damage at character level *L*: (`HookDamageMax` + HookDamageMax
  progression(*L*) + Σ max-damage modifier properties + action duration × (DPS-add progression(*L*)
  + Σ DPS modifier properties) × `DamageAddContributionMultiplier`) × damage qualifier ×
  (`HookDamageModifier` + Σ modifier properties); minimum = maximum × (1 − `HookDamageMaxVariance`).
  The damage qualifier itself comes from a rating curve and percentage properties of the actor.

> **OPEN:** (§18 Q12) Whether any of these three formulas is the original's, for tooltips
> (client bytecode, §13.2) or for the server's results (not in the data). In particular: the
> index base of the DPS and value arrays (whether element 0 is level 1, as LC assumes, given
> 701 and 601 elements and no minimum-index property), and the order of the sums and products in
> the hook-damage formula.

---

<a id="s14"></a>
## 14. Client behaviour settled by BIN

Only negative results (C25):

- The native client does not read the searched properties by ID and does not contain the
  searched labels as strings. The hits for the 10 searched property IDs (`Mod_Op`,
  `Mod_Progression`, `Mod_Array`, `Mod_Modified`, `Mod_Entry`, four progression array IDs and
  the minimum index) are all child-element lookups in user-interface code, where the same
  numbers are element identifiers; the 7 attribute hashes of §4.7 do not occur.
- No native code calls a function of the progression, effect, skill or trait classes by a
  constant function ID.

Consequence for Tessera: progression and modifier evaluation is entirely Tessera's own code,
specified by §4.10 and §5.6, with the OPEN points marked there.

---

<a id="s15"></a>
## 15. Explorer cards

Suggested contents. All values come from the property half; references are shown as links that
open the target through normal routing.

| Card | Header | Body |
|---|---|---|
| **Skill** | `Skill_Name`, `Skill_Icon`, family/class index, DID | `Skill_Desc`; costs (§7.3) with progressions evaluated at a chosen level; range min/max, area volume and max targets; target selection mode and valid target kinds; recovery time, action duration, induction and channelling; hit switches (can miss/evade/parry/block, line of sight); attack hooks as a sub-table (damage max, progression value at the chosen level, variance, modifier, damage type, qualifier, critical multipliers, effect lists); linked effect lists with links; required trait and effects |
| **Effect** | `Effect_Name`, `Effect_Icon`, class and role (§6.1), DID | definition and applied descriptions; harmful/debuff; duration group (§6.2) with interval and pulse count; application probability and variance; resist category; equivalence class with its control data (§6.6), class priority and cap; class-specific values (§6.5); `Mod_Array` as a table: modified property label, operator label, value or progression value at the chosen index, level range, modifier list, description override |
| **Trait** | `Trait_Name`, `Trait_Icon`, class and role (§8.1), DID | description, nature, category, tier, minimum level, cost to slot; ranks and maximum rank; prerequisites; `Mod_Array` table; trait effect lists, granted skills; for point-based traits the tree, branch and cell (§8.5) and the point-cost progression |
| **Progression** | class and kind (§4.1), DID | minimum index, represented property; a value table and a small plot over the covered range; for linear kinds the control points and slopes; the list of holders is optional (expensive) |

A level or spellcraft selector (default 1) drives every progression shown on a card. Values the
spec marks OPEN (out-of-range indices of §4.8, a progression of progressions without an inner
index, stacking) are shown as "not evaluated" rather than guessed; a card does not apply the
D1 clamp, so that the viewer never shows a Tessera choice as data.

---

<a id="s16"></a>
## 16. Disagreements with other sources

| Topic | Other source says | This spec | Basis |
|---|---|---|---|
| Missing `Progression_MinimumIndexValue` | LC: 1 | 0 | dictionary default and compiled form (C6) |
| Effect classes with property modifications | DS: 713, 734, 748, 763 are "effects", 718 "flag effect" | 713, 748 derive from 734 and carry `Mod_Array`; 763 is a pet summon (§6.1) | OBS |

---

<a id="s17"></a>
## 17. Invariants (summary, all Confirmed on this install)

| ID | Invariant |
|---|---|
| F1 | Every object classified as combat by §2.1 has a 0x79 partner and both halves decode completely. |
| F2 | Every family record carries its signature properties (§2.1). |
| P1–P7 | §4.11 |
| M1–M4 | §5.9 |
| E1 | Every effect falls into exactly one duration group of §6.2, with the groups taken by stored value (`Effect_Instantaneous` true; a stored false counts as absent). |
| E2 | `Effect_Duration_Permanent`, when present, is true. |
| S1 | 56 properties occur in every skill; every skill with attack hooks has one compiled class-822 object per hook. |
| S2 | A skill has at most one of the sphere, arc and box detection volumes. |
| T1 | Every `Trait_TraitTree_Trait` names a class-3438 trait. |
| T2 | Trait-cell keys have the form of §8.5. |
| G1 | No combat-family record names a missing object record. |

---

<a id="s18"></a>
## 18. Open questions (summary)

| # | Question | Where | Settles it |
|---|---|---|---|
| Q1 | Out-of-range progression indices | §4.8 | client library bytecode (needs a bytecode spec) |
| Q2 | Exact f32 evaluation inside a linear segment and at the last key | §4.4 | as Q1 |
| Q3 | Which actor values supply the outer and inner index of a progression of progressions (all holders are class-3438 traits) | §4.5 | as Q1; or M3 tooltips |
| Q4 | Lookup rule of sparse DID progressions | §4.6 | as Q1 |
| Q5 | Which level governs modifier level ranges in effects and traits (none in this install) | §5.5 | as Q1 |
| Q6 | Combination order of modifier entries; `Set` precedence | §5.3 | server logic; M3 observation |
| Q7 | Duration semantics (interval vs. pulses; permanent with interval) | §6.2 | server logic; M3 |
| Q8 | Constant vs. progression precedence in vital amounts | §6.4 | as Q1 (tooltips) |
| Q9 | Meaning of spellcraft | §1 | as Q1 |
| Q10 | Stacking rules of equivalence classes | §6.6 | server logic |
| Q11 | Row/column of trait-cell keys; the 0x10000000 group | §8.5 | M3 (trait panel) or UI layout data |
| Q12 | Rating-curve and damage formulas; index base of DPS and value arrays | §10.1, §13.3 | as Q1 for tooltip values; server logic otherwise |
| Q13 | Derived-stat formula completeness | §9.1 | server logic; M3 character panel |

---

<a id="implementer-notes"></a>
## Implementer notes

Engineering choices of the implementation (`crates/formats/src/gameplay/combat.rs`) where this
spec is silent. They are not facts about the original; a spec writer may confirm or replace them.

> **OPEN:** (implementation choice) A progression value array whose stored count includes null
> elements is rejected as malformed, because element positions are indices (§4.3) and a null
> element's position is not kept by the decoder. None occurs in the observed install. Elements of
> the wrong type (a float array holding a non-float, a control point without
> `LinearInterpolatingProgression_Key` or `_Value`) are rejected the same way, rather than
> replaced by the dictionary default.

> **OPEN:** (implementation choice) Whether a DID array is a progression of progressions is
> decided by the record's class (§4.1), not by `ProgressionProgression_InnerRepresentedPropertyName`.
> Index arithmetic is done in 64 bits (128 for *x* − *m*), so no index overflows. The D1 clamp is
> logged at debug level.

> **OPEN:** (implementation choice) A `Mod_Entry` without `Mod_Modified` is treated like
> `Mod_Modified` = 0 (empty). An entry with both an inline value and a progression, or with
> neither, is kept and marked (M1 says neither occurs) rather than rejected.

> **OPEN:** (implementation choice) The D2 sandbox combines only values of the base value's own
> type (M2); a value of another type is skipped and logged. Integer arithmetic saturates. Bitwise
> operators on any-width bitfields work byte by byte at the base's width. `ArrayAdd` appends the
> elements and adds the counts. Operators with no Reported meaning (`Invalid`, `Difference`,
> `Divide`, `Random`) are skipped and logged.

> **OPEN:** (implementation choice) Effect references (§6.3) are recognised by their ID field
> wherever a struct carries one (`EffectGenerator_EffectID`, `Skill_Effect`,
> `Skill_Toggle_Effect`, `Effect_StartupEffectID`, tried in this order), so the same struct kinds
> are found under any holder; a DID entry with one of these labels or `Effect_WSLEffect` outside
> such a struct is a bare reference. Zero values are kept and marked as placeholders.

> **OPEN:** (implementation choice) The typed views require the signature properties of their
> family (§2.1) and report a missing one as an error; every other property is optional. The DPS
> and value tables (§10.2) offer base values by position only, because the index base is OPEN
> (Q12), and quality factors by key.

---

<a id="s19"></a>
## 19. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Families, census, progressions, modifier entries, effects, skills, traits, class and control tables, enum mappers, reference graph, BIN results. |
| 2026-10-07 | Revision after review (status stays Review). Every disputed count re-measured over all records. (1) §5.5, M3, C12: ranges are disjoint, but the stored order is not guaranteed (100 groups not ascending, patterns given); readers sort by floor; added holders (all ranged groups are in non-combat objects), ends present, floor and ceiling ranges, group sizes. (2) §6.2, E1, C14: duration groups now go by stored value; the 722 + 1 records with `Effect_Instantaneous` false moved to the "permanent" and "interval" rows (12,561 and 5,250); stored false means not instantaneous; added the two duration-progression properties. (3) §4.4, §4.10, §4.12: the algorithm takes an outer and an optional inner index; class 3810 without an inner index gives none (D4); value at any key, including the last, is the stored value (D3); worked examples for both. (4) Q2, Q3, Q4, Q5, Q9, Q12 and Q13 are now `> **OPEN:**` callouts (§1, §4.4, §4.5, §4.6, §5.5, §9.1, §10.1, §13.3). (5) Provenance: legend of marks; rows C27–C33 for §4.9, §6.3, §6.5, §7.9, §8.6, §6.6/§10.2 and §11; Decision rows D1 (clamp, §4.8), D2 (sandbox order, §5.3, now including `ArrayAdd` last), D3, D4. (6) §10.2: all 8 class-1660 records have 701 elements, all 99 class-1688 records 601; quality structs stored in key order 5, 4, 2, 3, 1; `Item_ValueLookupTable` totals 150,622 values. (7) §6.6 `MaxDepth` histogram spelled out; §11 labels checked against the EMAPPER map (two mappers have none). Other corrections found while re-measuring: §6.3 `EffectGenerator_EffectID` targets 66 of 87 effect classes, not every class; struct counts, optional fields and holders completed (`Skill_EffectData` also in effects); §3.2 and §7.8 placeholder count is 97,268 in skills (the former 99,817 included 2,549 in effects); §6.5 `Mod_Array` holders by class (708 is the only holder outside 734's subtree), class 3124 properties corrected, `Effect_SkillProc_RequiredCombatResult` is a bitfield; §4.9 target classes filled in, all 50 class-3810 references are in class-3438 traits (P7); §7.9 counts and the induction (1,779) and channelling (401) objects; §8.6 list lengths and lookup through the WEENIECONTENT map. |
| 2026-10-07 | Second revision after re-review (status stays Review; text only, no section renumbered). (R1) §6.3: the `EffectGenerator_EffectID` sentence now states its population: 0 of the 54,832 struct values are 0; over all 55,825 occurrences (by enclosing struct and holder) 59 are 0, all in `Threshold_State_EffectStruct` structs of one class-1932 record; targets 66 effect classes either way; re-measured over every 0x78 and 0x79 set, and C28 widened. (R2) Provenance rows added for facts that had none: C34 (§3.4 integer census, re-measured over every integer-typed value), C35 (§5.4 modifier lists, with the LC reading marked Reported), C36 (§5.7), C37 (§13.1 function and body counts, re-measured); the sections now cite them. |
