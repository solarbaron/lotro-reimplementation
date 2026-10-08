<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Combat data: skills, effects, traits, progressions and their tables

| | |
|---|---|
| **Status** | Revised (OPEN sweep of 2026-10-09, §19 last row; waiting for review). Previously Accepted (revision of 2026-10-08 and its repair, accepted on re-review 2026-10-08). Implementers re-check code against the sections changed on 2026-10-08 (§19, last two rows). |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room sessions, AI-assisted), not the author: first review, re-review and acceptance review, all 2026-10-07; review and re-review of the 2026-10-08 revision, 2026-10-08 |
| **Review record** | Acceptance review 2026-10-07 (third review session, not the author): **passed**, Accepted. Revision of 2026-10-08 (OPEN closure, §19): reviewed 2026-10-08 (separate review session, not the author): **changes requested** (§6.2 steps 3–4, §5.3 64-bit types, §10.2 weapon duration; 5 spot checks reproduce). Fixes by the author 2026-10-08 (§19). Re-review 2026-10-08 (separate review session, not the author): **passed**, Accepted. Full records in the review log (decision 0018). OPEN sweep 2026-10-09 (spec writer, dirty room): normative text of §4.1, §4.6, §5.2, §6.4, §6.5, §8.1, §9.1, §9.2 changed (rows C53–C58, D6); status Revised until a separate session reviews it. |
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
| BC | The client library's bytecode (record 0x56000000, chunk type 1), read on 2026-10-08 in the dirty room with a throwaway disassembler that follows `wlib-bytecode.md` (Accepted), by class and body offset. This is data in the user's install (method M1). Bodies read: the progression classes 1386, 1385, 1389, 2402, 2519, 3810, 3918, 3920 and the segment and point classes 3916, 3917; the index helper class 1388; the modifier classes 1338–1342; the effect base class 698 (duration, interval, pulse-count and spellcraft functions); the property-modification effect family (class 734 and the classes that share its bodies), the applied-effect record class 700 and the override holder class 732; the instant-vital effect class 725; the point-based trait functions of class 3438 and the ranked-trait functions they call; the combat control class 1659; the DPS and value table classes 1660 and 1688; the weapon-data functions of class 256; class 3800. Read on the same day for the fixes of the 2026-10-08 review: the vital-over-time amount functions of class 755; the represented-property functions of classes 2402, 2519 and 3810; the weapon-data function of class 256 in full, with the weapon timing classes 1692 and 1691, the weapon variance class 2618, the damage function of class 1659 and the modified-value function of class 1046 it calls; the duration, interval, pulse-count and permanence functions of class 698 again. Plus byte and immediate searches of the whole block for the property IDs named in §4.6, §9.1 and §10.1. Facts from this source describe what the **client's** library computes; the server-side library is not in the archives (§13.1). |
| DEC | Tessera's own choices where the data, the client and published sources say nothing. Each is marked **Decision** below and is not a claim about the original game. |
| OBS3 | Observed in the user's install on 2026-10-09 (OPEN sweep, spec writer). The archives are unchanged since 2026-10-04 (file times), so a dump of **every** 0x70 record with its 0x79 property set and of all 0x78 sets, made on 2026-10-07 with the repository's own decoders, stands for the install; it was walked at every nesting depth, the property dictionary (0x34000000) was read for types and defaults, and DID targets were classified by the root class of every 0x70 record. Counts cover all records of the kind named. |
| BIN | **M4 under decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, `lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-07 with a headless disassembler/decompiler in the dirty room, by address range only. **Questions:** (b1) does native client code read the progression, modifier-entry, skill or effect properties by property ID; (b2) does native code call the script functions of the progression, effect, skill or trait classes by a constant function ID (`wlib.md` §6.4); (b3) where is the script machine's bytecode dispatch, so that the client-side progression and modifier functions could be read. **Why M1–M3 were not enough:** the out-of-range behaviour of progressions and the order in which modifiers combine are not stored in any record; no published source states them with authority; no running client is available to this session (M3). **Results:** (b1) no: 10 property IDs of §4 and §5 and the 7 attribute hashes of §4.7 were searched as instruction immediates; the attribute hashes have no hit, and every property-ID hit is an argument of a child-element lookup in user-interface code, a separate identifier space that shares the numeric range; no defined string in the executable contains `Progression`, `Mod_` or `Combat_Control`; (b2) no: none of the 986 function IDs of classes 484, 486, 698, 827, 1385, 1386, 1389, 1479, 2402, 2519, 3810, 3916–3918 and 3920 occurs as an immediate; (b3) not located in this pass (the library loader and its chunk dispatcher were found; the interpreter loop was not). Nothing from network, login, launcher or anti-tamper code was read. **Second pass, 2026-10-08** (same build and executable, a disassembler on the read-only executable only, no analysis database), by address range only, to establish what the native functions called by the BC bodies do. **Questions:** (b4) what the native functions used by the progression, modifier, duration and combat-table bodies return (integer and float minimum and maximum, clamp, array element access and count, the property arithmetic used by the modifier operators, the float-to-property conversion, the map insertion used by modifier lists, the two level readers); (b5) whether array access checks the index. **Why M1–M3 were not enough:** the bytecode passes values to these natives by function ID only (`wlib-bytecode.md` §7); what they compute is in the executable alone, no published source describes them, and no running client is available to this session. **Results** (registered entry points located with the native-registration pairing of `wlib-bytecode.md`'s BIN row): package 122 functions 24 and 25 are signed 32-bit minimum and maximum, functions 22 and 23 are single-precision minimum and maximum (SSE `minss`/`maxss`); package 2574 function 4 clamps a float to [low, high]; package 17 function 4 and package 176 function 3 read an array element **without** a bounds check, functions 12 and 11 return the element count, package 176 function 10 tests for an empty array; package 3176 function 32 stores a float into a property object, as f32 for float properties, truncated toward zero for int and int64 properties, and not at all for other types; package 39 functions 65, 66, 67, 68, 69 and 78 implement add, subtract, multiply, divide, set and or on property objects (§5.3); package 35 function 1 inserts into a reference map and does **not** replace an existing key (returns 0); package 3703 functions 1 and 2 read the actor's `Advancement_Level` and `Apparent_Level` (1 when absent or not an int). Nothing from network, login, launcher or anti-tamper code was read. **Third pass, 2026-10-08** (same build and executable, a command-line disassembler on the read-only executable, no analysis database, by address range only). **Question:** (b6) what the set and or natives of package 39 (functions 69 and 78) and the add, subtract, multiply and divide natives (functions 65–68) do for the value types whose values are held in a separate value object (int64, 64-bit bitfield, any-width bitfield), which the second pass had not followed. **Why M1–M3 were not enough:** as for b4; the data holds `Or` entries on 64-bit and any-width bitfields, and the per-type code is in the executable alone. **Results** (the value-object kind of each property type taken from the client's value factory, a switch on the type code): set copies the source value into the property for every type, without a type-specific step; or works on bool and 32-bit bitfield values directly and, for the 64-bit and any-width bitfields, through the value object's own or operation (64-bit or; any-width or after widening the shorter operand with zero bits); the int64 value object implements add, subtract, multiply and divide in 64 bits (wrapping, low 64 bits, signed division truncated toward zero) and does nothing for or; every other type is left unchanged by or (§5.3). Nothing from network, login, launcher or anti-tamper code was read. |

Marks: **Confirmed** = observed in the user's install over every record of the kind named (not a
sample), on the date given; **Confirmed (bytecode)** = read from the client library's code in the
install (BC), which is data, with every observed record agreeing where a census applies;
**Confirmed (bytecode + BIN)** = as before, where the meaning of a native function the code calls
comes from BIN; **Reported** = published third-party material only; **BIN only** = read from the
client executable under M4 (decision 0011); **Decision** = a Tessera choice (DEC), not a fact about
the original.

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
| C9 | Behaviour outside the covered index range (§4.8) | LC (extrapolate along the last segment above; undefined below); now settled by C38 and C39 | Reported; superseded 2026-10-08 by C38, C39 (LC agrees above the range, not below) |
| C10 | Modifier-entry layout and its three kinds: empty, inline value, progression-driven; never both value and progression (§5.1, §5.2) | OBS (all 372,834 entries in all 116,435 `Mod_Array` values) | Confirmed 2026-10-07 |
| C11 | Operator enum 0x23000046 and which operators occur with which value types (§5.3) | OBS (mapper and all entries); LC (operator meanings); C41 (arithmetic) | Keys and co-occurrence Confirmed; arithmetic Confirmed (bytecode + BIN) 2026-10-08 |
| C12 | Level ranges: entries for one property within one array have disjoint ranges (stored order not guaranteed; 100 groups are not ascending as stored), are never mixed with unranged entries for the same property, and occur only in non-combat holders (§5.5) | OBS (all 14,798 ranged groups, all 19,533 ranged entries; re-measured 2026-10-07) | Confirmed 2026-10-07; which level is compared: C42 (client rule), LC for items (Reported) |
| C13 | Effect classes, their distinctive properties and counts (§6.1) | OBS (all 32,971 effects, 87 classes) | Confirmed 2026-10-07; role names are this spec's descriptions |
| C14 | Duration and application-probability property census, and the duration groups by stored value (§6.2) | OBS (all 32,971 effects; re-measured 2026-10-07) | Confirmed 2026-10-07; duration semantics C45 (client library), the rest Reported (LC) |
| C15 | Vital-change amount pattern (constant, progression, random, variable, variance, modifier lists) and its co-occurrence census (§6.4) | OBS (all 1,103 instant-vital and 2,438 vital-over-time effects); LC (meaning); C46 (source selection) | Layout Confirmed; source selection Confirmed 2026-10-08 (C46); other meaning Reported |
| C16 | Equivalence-class data in the effect control object (§6.6) | OBS | Layout Confirmed; meaning **OPEN** |
| C17 | Skill property census; 56 properties present in every skill (§7) | OBS (all 13,247 skills) | Confirmed 2026-10-07 |
| C18 | Attack hooks: one compiled object per `Skill_AttackHookInfo` in every skill (§7.7) | OBS (13,733 of 13,733) | Confirmed |
| C19 | Trait classes, trait trees (class 3486), trait-cell key structure (§8) | OBS | Confirmed; row/column meaning **OPEN** |
| C20 | Class advancement tables (class 853), derived-stat formula lists, monster-play tables, advancement lines (§9) | OBS | Layout Confirmed; formula reading Reported |
| C21 | Combat control object, DPS and value lookup tables, thresholds, battle statistics, DID maps COMBATEFFECTS and BATTLESTATS (§10) | OBS; LC (curve and table formulas); C47, C48 (client formulas) | Layout Confirmed; curve and table formulas Confirmed (bytecode) 2026-10-08 |
| C22 | Enum mappers used by these families (§11) | OBS | Confirmed |
| C23 | Reference graph census; no family record names a missing object record (§12) | OBS (every DID value in every 0x78 and 0x79 set) | Confirmed 2026-10-07 |
| C24 | The client library contains bytecode bodies for the progression, effect, skill, trait and modifier classes, and its bytecode contains the IDs of `Mod_Op`, `Mod_Modified`, `Mod_Progression`, `Mod_ModifierList`, `Mod_ProgressionFloor`, `Combat_DPS_LUT`, `Item_ValueLookupTable` and others as 32-bit literals (§13.2) | OBS (method tables of `wlib.md` §6.4; byte search of the bytecode chunk) | Confirmed |
| C25 | Native client code neither reads these properties by ID nor calls these script classes by constant function ID (§14) | BIN (b1, b2) | BIN only |
| C26 | Third-party formulas for rating percentages, weapon damage and skill hook damage (§13.3) | LC | Reported; rating and weapon formulas superseded by C47, C48, C52; hook damage still Reported only |
| C27 | Who uses progressions: holder, property path, reference count and target classes (§4.9); all 50 references to class 3810 sit in modifier entries of class-3438 traits (§4.5) | OBS (every DID value of every 0x78 and 0x79 set) | Confirmed 2026-10-07 |
| C28 | Effect-reference structs, their fields, holders and counts; every occurrence of `EffectGenerator_EffectID` by enclosing struct and holder, its 59 zero values (one class-1932 record) and its targets (§6.3); placeholder `Skill_Effect` counts (§3.2, §7.8) | OBS (every 0x78 and 0x79 set; `EffectGenerator_EffectID` occurrences re-measured 2026-10-07) | Confirmed 2026-10-07 |
| C29 | Class-specific effect properties: presence, value ranges and holders of `Mod_Array` per effect class (§6.5) | OBS (all records of the classes named) | Presence and ranges Confirmed 2026-10-07; meanings are this spec's descriptions from the labels |
| C30 | Skill timing properties; induction objects (class 1195) and channelling objects (class 961), their counts and duration properties (§7.9) | OBS (all 13,247 skills, all 1,779 class-1195 and 401 class-961 records); LC (induction duration as cast time) | Layout and counts Confirmed 2026-10-07; "cast time" Reported |
| C31 | Trait control object (class 1486): one record, reached through the WEENIECONTENT map; its progression references and list lengths (§8.6) | OBS | Confirmed 2026-10-07 |
| C32 | Equivalence-class data counts and value histogram (§6.6); DPS and value table lengths and holders (§10.2) | OBS (the one class-1666 record; all 8 class-1660 and all 99 class-1688 records; every 0x78 and 0x79 set for holders) | Confirmed 2026-10-07 |
| C33 | Enum-mapper labels of §11 | OBS (EMAPPER DID map 0x28000001 and each mapper) | Confirmed 2026-10-07 |
| C34 | No integer-typed value equals the DID of an object record, so combat references are stored only as DIDs (§3.4) | OBS (every int, 64-bit int, 32- and 64-bit bitfield and enum value in all 1,616 0x78 and 383,332 0x79 sets, compared with the DIDs of all 383,332 object records; re-measured 2026-10-07) | Confirmed 2026-10-07 |
| C35 | Modifier lists: 1,946 non-empty entries with at least one non-zero PID, lists of 1 or 2 PIDs, `Mod_ModifierOp` counts 668 / 108 / 887 / 283 for keys 5–8 (§5.4); how listed values combine with the entry | OBS (all 372,834 entries); LC (combination) | Counts Confirmed 2026-10-07; combination Confirmed (bytecode + BIN) 2026-10-08 (C44) |
| C36 | Other modifier-like arrays and their progression-reference counts, 18 and 10 (§5.7) | OBS (every DID value of every 0x78 and 0x79 set, as C27) | Confirmed 2026-10-07 |
| C37 | Declared functions and functions with bodies of classes 698 (282, 123) and 827 (477, 166) in the client library; the server-side library DID is in no archive (§13.1) | OBS (method tables of the client library, decoded as `wlib.md` §6.4 describes; re-measured 2026-10-07); `wlib.md` §2 (missing server-side library) | Counts Confirmed 2026-10-07; the missing library as `wlib.md` §2 states it |
| D1 | Out-of-range progression index: clamp to the covered range and log (§4.8, §4.10) | DEC | Decision; superseded 2026-10-08 by C38 (arrays clamp, as D1 chose) and C39 (linear progressions extrapolate) |
| D2 | Sandbox combination order of modifier entries (§5.3), used by the §5.9 example | DEC | Decision |
| D3 | Value at a linear key, including the last key: the key's stored value (§4.4) | DEC | Decision; superseded 2026-10-08 by C39 (the last key is computed from the last segment) |
| D4 | Progression of progressions evaluated without an inner index gives none; chain depth limit 4 (§4.10) | DEC | Decision; superseded 2026-10-08 by C43 |
| C38 | Array progressions (property, float, text, DID, progression of progressions) clamp the index to [minimum index, minimum index + count − 1] with signed comparisons before reading the element; the element read itself has no bounds check (§4.3, §4.8) | BC (classes 1386, 1388, 1385, 1389, 2402, 2519); BIN (b4, b5: integer minimum and maximum, unchecked element read) | Confirmed (bytecode + BIN) 2026-10-08 |
| C39 | Linear progressions: segment search with half-open segments, the f32 formula and its operation order, extrapolation along the first segment below the first key and along the last segment from the last key on (the last key included), 0.0 when there is no segment; storing the result in an int or int64 property truncates toward zero (§4.4, §4.8, §4.10) | BC (classes 3918, 3917, 3916); BIN (b4: float-to-property store) | Confirmed (bytecode + BIN) 2026-10-08 |
| C40 | Modifier-entry load checks and the valid operators per value type, as the client library applies them; the property each progression kind represents, text and DID arrays included; census: all 60,295 inline and all 299,143 progression-driven entries pass them, and every progression-driven entry's progression represents exactly the modified property (§5.2, §5.3, §5.6, §5.9) | BC (classes 1340–1342 load and verify functions; represented-property functions of classes 1385, 1389, 2402, 2519, 3810, 3918); OBS (all 372,834 entries, every progression they name) | Confirmed (bytecode) 2026-10-08 |
| C41 | Operator arithmetic and the identity value of each operator (§5.3), for every value type the load check allows: float, int and int64 arithmetic; `Or` on bool, 32-bit, 64-bit and any-width bitfields; `Set` on every type; `ArrayAdd` on arrays | BC (class 1341 operator switch, identity function); BIN (b4: property arithmetic natives; b6: set, or and the int64 arithmetic for the separately held value types) | Confirmed (bytecode + BIN) 2026-10-08 |
| C42 | Level ranges compare the entry's evaluation level, truncated toward zero, inclusive at both ends; an entry outside its range yields its operator's identity value; the load check rejects a stored floor or ceiling ≤ 0 and floor ≥ ceiling; no entry in the data violates it, and every ranged entry is progression-driven (§5.5) | BC (class 1342); OBS (all 19,533 ranged entries) | Confirmed (bytecode) 2026-10-08 |
| C43 | Progression of progressions inside a modifier entry: outer index = the entry's evaluation level, inner index = the integer `Mod_ProgressionProgressionInnerIndex` of the evaluation's property set, or outer 1 and inner = the level when that property is absent; point-based traits write the character's `Advancement_Level` into that property of their applied-effect records; the compiled applied-effect records of the 946 class-3438 traits store spellcraft −1 (1,169 records) or 0 (85), none of the 46 traits holding the 50 references has `Trait_PointBasedTrait_LevelBasedSpellcraft`, and in all 761 traits with compiled modifier entries those entries are reachable from the applied-effect record's override holder (class 732) (§4.5) | BC (classes 1342, 3438, 700, 732, 734); BIN (b4: level readers); OBS (all 946 class-3438 traits) | Confirmed (bytecode + BIN) 2026-10-08 |
| C44 | Modifier lists: combination order and operand, the property set they read, and the client's refusal of a modifier array in which two applying entries change one property; census: 79 arrays hold two unranged entries for one property, all in class-1346 objects, none in an effect or trait (§5.4) | BC (classes 1340, 1341); BIN (b4: map insertion keeps the first key); OBS (all 116,435 arrays) | Confirmed (bytecode + BIN) 2026-10-08 |
| C45 | Effect duration as the client library computes it: override, permanent, interval × pulse count, interval and pulse-count sources and modifier sums, interval 0 without a source, minimum interval 0.5 s, equivalence-class duration factors clamped to [0.1, 5.0], minimum duration 0.5 s; the separate "is permanent" query (§6.2, §6.6) | BC (class 698, class 1046 duration-modifier function, class 1666 lookups); BIN (b4: float maximum, clamp) | Confirmed (bytecode + BIN) 2026-10-08 |
| C46 | Vital amounts: exactly one source is used, selected by flag bits compiled into the object state; the bit of each source for the instant amount (725) and for the initial and per-interval amounts (755), and the order in which class 755 tests them; where several sources are stored the compiled flags select the progression over a constant, a variable or a random amount, and the variable amount over a constant; which class-755 bits occur (§6.4) | BC (class 725 amount function; class 755 initial and per-interval amount functions); OBS (compiled flags of all 1,103 class-725 and 2,438 class-755 records) | Confirmed (bytecode) 2026-10-08 |
| C47 | Rating curve of the combat control object: formula, operation order, guards; its three compiled arrays hold 31 elements indexed by calculation type (element 0 empty) and match `Combat_Control_Rating`, `_Target_Cap`, `_Hard_Cap` in 30 of 30 structs (§10.1) | BC (class 1659); OBS (object 0x70000235 both halves); BIN (b4: float minimum) | Confirmed (bytecode + BIN) 2026-10-08 |
| C48 | DPS table and item value table lookups (element 0 = level 1, range handling, quality factor) and the item value formula (§10.2) | BC (classes 1660, 1688, 1659, 256) | Confirmed (bytecode) 2026-10-08 |
| C49 | Spellcraft in the client library: effect progressions (amounts, interval, pulse count, modifier entries) are evaluated at the applied effect's spellcraft truncated toward zero; a spellcraft argument other than −1 replaces the stored one (§1) | BC (classes 698, 700, 725, 734, 828) | Confirmed (bytecode) 2026-10-08 |
| C50 | No bytecode body names class 3800, and none of the property IDs of sparse DID progressions (`SparseDIDProgression*`), trait trees (`Trait_TraitTree_Trait*`, `_Specialization*`), derived-stat formulas (`AdvTable_DerivedStat*`) or the rating-curve structs (`Combat_Control_CalcControlInfo*`, `_CalculationType`, `_Rating`, `_Target_Cap`, `_Hard_Cap`) occurs anywhere in the bytecode block (§4.6, §8.5, §9.1, §10.1) | OBS (byte and immediate search of the whole block) | Confirmed 2026-10-08 |
| C51 | The property-half source of the pulse-count modifier list is `Effect_PulseCount_AdditiveModifiers`: in all 32,971 effects the compiled list is present exactly when the property is (277 effects: classes 755, 708, 734, 2282 with 251, 13, 12, 1) and holds the same non-zero PIDs in the same order; no other record holds the property; the values are summed as integers (§6.2) | OBS (all 32,971 effects, both halves; every 0x78 and 0x79 set); BC (class 698 pulse-count functions) | Confirmed 2026-10-08 |
| C52 | Weapon data procedure of the client library: the checks, the action duration and animation modifier from the weapon timing tables (classes 1692, 1691) by equipment category and `Combat_WeaponSpeed`, the variance from the weapon variance table (class 2618) by category mask, the `Combat_Damage_Mod` addition for objects whose `WeenieType` has bits 0x01 and 0x80, and the maximum-damage formula; census: every weapon's category is a single bit and a key of the timing map, its speed a key of that table, and its category lies in exactly one variance mask, whose value equals the stored variance; every weapon's `WeenieType` has both bits; no 0x79 set holds `Combat_Damage_Mod` or names it in `Mod_Modified` (§10.2) | BC (class 256 weapon function and the bodies it calls in classes 256, 1659, 1691, 1692, 2618, 1046, 492); OBS (all 23,204 weapons, the class-1692, 24 class-1691 and one class-2618 records, every 0x79 set) | Confirmed (bytecode) 2026-10-08 |
| D5 | A weapon category that lies in several variance masks takes the first matching entry in stored order (§10.2.1) | DEC | Decision (no weapon in this install has more than one match) |
| C53 | Float-array lengths: length 8 occurs in 1 record (the row of §4.1 said 5); the row sums to the 1,338 float arrays | OBS3 (all 1,338 class-1385 records) | Confirmed 2026-10-09 |
| C54 | The 270 `SparseDIDProgressionEntry_DID` values name traits of classes 1477 (191) and 1483 (79), none of class 3438 (corrects §4.6) | OBS3 (all 39 class-3800 records) | Confirmed 2026-10-09 |
| C55 | `Mod_Array` occurs at top level in 112,502 sets and nested in 3,933 arrays: 3,929 inside `Set_ActiveCountData` (item-set tiers), 2 inside `SPM_PropertyMod` and 2 inside `Usage_ModPropertyUsageData_ModProperties` (the two property-modifying tokens of `items.md` §2.1) | OBS3 | Confirmed 2026-10-09 |
| C56 | `Effect_WSLEffect` = 0 occurs 2,649 times: 2,648 in `Effect_Combo_EffectPresentList`, 1 in `Effect_Dispel_ByEffectList`; none in `Effect_Protection_ByEffectList` | OBS3 | Confirmed 2026-10-09 |
| C57 | Vital-amount co-occurrence counted with the variance properties left out (both classes), the variance counts separately, and the full labels of the modifier-list and critical-multiplier properties per stem (§6.4) | OBS3 (all 1,103 class-725 and 2,438 class-755 records) | Confirmed 2026-10-09 |
| C58 | Field labels of `Effect_Aura_Applied_Effect_Data`, `Effect_Area_Applied_Effect_Data`, `Effect_ApplyOverTime_Applied_Effect_Data` (§6.5); of `AdvTable_AvailableSkillEntry` (§9.1), of the three monster-play entry structs (§9.2) and of `Trait_Skill_Array` (§8.1) | OBS3 (dictionary; every occurrence) | Confirmed 2026-10-09 |
| D6 | Implementer choices of the implementer-notes section confirmed as Tessera decisions: null or mistyped progression elements reject the progression; a `Mod_Entry` with both an inline value and a progression uses the progression, one with neither is skipped; effect references recognised by their ID field under any holder, zero references kept as placeholders; typed views require the family's signature properties | DEC | Decision 2026-10-09 (none of these shapes occurs in this install) |

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
| **Spellcraft** | A float stored next to effect references (`*_Spellcraft`, `Skill_EffectSpellcraft`). −1 means "not given". In the client library (C49) it is the **evaluation level** of an applied effect: the effect's amount, interval and pulse-count progressions and its modifier entries are evaluated at the applied effect's spellcraft truncated toward zero, and a spellcraft passed with a request that is not −1 replaces the stored one. Reported (LC, community usage): when no spellcraft is given anywhere, the applier's level is used. See the callout below. |
| **Placeholder entry** | A struct or reference present only because the record stores a full default set: a `Skill_EffectData` with `Skill_Effect` = 0, a `Mod_Entry` with `Mod_Modified` = 0. Readers skip them. |

> **OPEN:** (§18 Q9) Where an applied effect's spellcraft comes from when every source says
> "not given" (−1): the applier's level (LC's reading, Reported) or another value. The applied-effect
> records compiled into the data store −1 (1,169 trait records) or 0 (85), so the value is supplied
> when the effect is applied, by code that is not in the client library's effect classes read here
> (§13.2). Related: for ranked traits that are not level-based, the client derives the trait's
> spellcraft from the character's rank in the trait through ranked-trait functions (class 1494
> family) that this pass did not read. M3 tooltip observation at several character levels, or a
> reading of those functions, would settle both. Until then a viewer evaluates at a user-chosen index
> (§15).

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
Two of these patterns are now Confirmed for the client library: `Mod_ModifierList` (combined with
the entry's value by `Mod_ModifierOp`, §5.4, C44) and `Effect_Duration_ConstantInterval_ModifierList`
(its values are added to the interval, §6.2, C45).

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
| Float array | 200 (392), 500 (282), 80 (249), 150 (193), 160 (125), 100 (42), 120 (8), 15 (7), 2 (6), 19 (6), 5 (5), 10 (5), 101 (4), 16 (2), 301 (2), 401 (2), 700 (2), and 7, 8, 20, 36, 45, 250 once each (C53) |
| Linear interpolating (points) | 19 (913), 20 (453), 22 (211), 2 (112), 4 (59), 5 (46), 3 (13), 21 (5), 14 (1), 17 (1) |
| Text array | 150 (40), 11 (4), 7 (1), 12 (1) |
| DID array (2519) | 200 (12) |
| Progression of progressions | 5 (19), 7 (15), 10 (9), 6 (2) |

> **Answered (2026-10-09; was OPEN, implementation census 2026-10-07):** The float-array row sums to 1,342 records, not
> the 1,338 float arrays of this section. The implementation's census
> (`crates/formats/tests/gameplay_combat_real_data.rs`) reproduces every other count of the row
> and observes length 8 in 1 record, not 5, which gives 1,338. Is "8 (5)" a typo for "8 (1)"?
>
> *Answer:* yes. Length 8 occurs in one record (C53); the row is corrected and sums to 1,338.

<a id="s4-2"></a>
### 4.2 Common fields

| Property | PID | Type | Present in | Meaning |
|---|---|---|---|---|
| `Progression_MinimumIndexValue` | 0x100002C8 | int | 5,738 records: every kind except linear interpolating, and 1,332 of 1,338 float arrays | Index of the first array element. Values: 1 (5,721), 0 (9), −9 (6), −100 (2). **Absent ⇒ 0**, the dictionary default; the compiled form of the 6 float arrays without it stores 0 (C6). |
| `FloatProgression_ValueProperty` | 0x10000766 | PID | 698 float arrays | The property the curve represents (§3.3); absent ⇒ `FloatProgression_FloatValue`. Not needed to evaluate the curve, but the client's load check of a modifier entry compares it with the modified property (§5.6 step 1, C40). |
| `LinearInterpolatingProgression_ValueProperty` | 0x10005E5D | PID | all 1,814 linear progressions | As above (without a fallback); also decides how the value is stored into a property (§4.4). |
| `ProgressionProgression_InnerRepresentedPropertyName` | 0x100058D8 | PID | all 45 progressions of progressions | The property the inner progressions produce. |
| `Progression_Type` | 0x10001F0D | enum, mapper 0x2300028A (95 keys) | 5,029 records (3,207 property arrays, all 1,814 linear, 8 float) | A curve-family label used by item advancement tables. Not needed to evaluate. |
| `Progression_PointTier` | 0x10001F0A | enum, mapper 0x23000289 (10 keys) | same 5,029 records | A tier label, as above. |

<a id="s4-3"></a>
### 4.3 Array progressions (property, float, text, DID)

Element *k* (counting from 0) is the value at index *m* + *k*, where *m* is the minimum index
(§4.2). The covered range is *m* … *m* + *n* − 1 for *n* elements. An index outside the range is
clamped to it first (§4.8, C38).

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

**Evaluation** (Confirmed, bytecode + BIN, C39). The client library evaluates a linear
progression at an integer index *x* as follows; *x* is not clamped first.

1. **Segment choice.** Segment *i* (from point *i* to point *i* + 1) **contains** *x* when
   *xᵢ* ≤ *x* < *xᵢ₊₁* (half-open: the end key belongs to the next segment). The segment used is
   the one that contains *x*; if none does, it is the **first** segment when *x* < *x₀* and the
   **last** segment when *x* ≥ *xₙ₋₁*. (The original finds it with a binary search over the
   segments, which gives exactly this result because the keys strictly increase, P3; with keys
   that do not increase its result is not defined, and Tessera treats such a record as malformed,
   §4.10.)
2. **Value.** With the chosen segment's start point (*xᵢ*, *yᵢ*) and stored slope *s*: compute
   *d* = *x* − *xᵢ* as a 32-bit integer (wrapping), convert *d* to f32 (round to nearest), multiply
   by *s* in f32, then add *yᵢ* in f32. The result is *yᵢ* + *s* × f32(*x* − *xᵢ*).
3. **No segment** (fewer than two points): the value is 0.0.

Consequences, all following from steps 1–2:

- Inside the range, every *x* with *x₀* ≤ *x* < *x_last* is found in the segment that contains it;
  at an inner key *xⱼ* the result is *yⱼ* + *s* × 0.0, which is *yⱼ*.
- At the **last key** no segment contains *x*; the search ends on the last segment, so the value
  is *yₙ₋₂* + *s* × f32(*xₙ₋₁* − *xₙ₋₂*), which can differ from the stored *yₙ₋₁* in the last bit
  of f32. The stored last value is not returned.
- Outside the range see §4.8.

When the progression's value is stored into a property object (the form in which modifier entries
use it, §5.6), the represented property's type decides: a float property receives the f32; an
int or int64 property receives the f32 truncated toward zero; other types are not set (BIN). In
this install 1,439 linear progressions represent float properties and 375 represent int
properties (OBS, C39).

<a id="s4-5"></a>
### 4.5 Progressions of progressions (class 3810)

The DID array names 5 to 10 inner progressions (302 elements: targets 287 linear, 10 float,
5 property arrays; never 0). `Progression_MinimumIndexValue` is 1 in all 45 records. Evaluated on
its own, a progression of progressions behaves like a DID array (§4.3): the index selects an
element DID, with the clamp of §4.8. The **two-index** evaluation exists only inside a modifier
entry (§5.6, C43): the **outer** index *t* selects the element as in §4.3 (element *k* = *t* − *m*,
clamped), and that element's progression is then evaluated at the **inner** index *u*:

- *t* is the entry's evaluation level (the level the entry is evaluated at, truncated toward zero:
  for an applied effect its spellcraft, §1) and *u* is the integer property
  `Mod_ProgressionProgressionInnerIndex` of the property set that accompanies the evaluation
  (§5.6), when that property is present;
- when it is absent, *t* = 1 and *u* = the evaluation level.

The inner progression is evaluated with its own rules (§4.3, §4.4); if it is itself a progression
of progressions, it yields a DID (no further nesting). If the selected element does not load, the
entry fails (§5.6).

Point-based traits supply the inner index: when the client builds a class-3438 trait's applied-effect
records for a character (the trait's `Mod_Array` travels in those records as an override modifier
list: in all 761 class-3438 traits with compiled modifier entries they hang under the record's
override holder, class 732, OBS), it writes the character's `Advancement_Level` (1 when absent) into
`Mod_ProgressionProgressionInnerIndex` of each record's property set (C43). So for these traits the
inner index is the character's level and the outer index is the trait's spellcraft. Reported (LC):
the outer index is a tier counted from 1 and the inner index a level, which agrees.

Holders (C27): exactly 50 `Mod_Progression` values point at class 3810, and all 50 are
modifier entries of point-based traits (class 3438, §8.1). No item, effect or skill references
a progression of progressions in this install.

`Mod_ProgressionProgressionInnerIndex` is in the dictionary and in the client library's bytecode
but in no record: it is written at run time only. How a non-level-based trait's spellcraft follows
from its rank is part of the §1 callout (Q9).

<a id="s4-6"></a>
### 4.6 Sparse DID progressions (class 3800, outside subtree 1386)

39 records. `SparseDIDProgression_Array` holds structs of `SparseDIDProgressionEntry_Key`
(f32, 5–120) and `SparseDIDProgressionEntry_DID` (all 270 DIDs are traits: 191 of class 1477 and
79 of class 1483, none of class 3438; C54, correcting the earlier text). Used
only by trait-tree specialisations (§8.5). Reported (LC): at a given value, the entry with the
largest key not above it applies.

> **Answered (2026-10-09; was OPEN, implementation census 2026-10-07):** "All 270 DIDs are traits of class 3438" is not
> reproduced. Reading `SparseDIDProgressionEntry_DID` of every `SparseDIDProgression_Array` struct
> of the 39 records, the implementation finds 270 values, all naming traits, but of classes 1477
> (191) and 1483 (79), none of class 3438. Which field or which records does the statement
> describe? The implementation checks only "names a trait" until this is settled.
>
> *Answer:* the statement was wrong; the implementation's count is right (C54). §4.6 now gives
> classes 1477 and 1483. "Names a trait" is the invariant to check.

> **OPEN:** (§18 Q4) The lookup rule of sparse DID progressions: whether the entry with the
> largest key not above the value applies (LC's reading, Reported), what is returned below the
> first key, and what value is compared with the f32 keys (a rank, a point total, a level). The
> data stores only the key-DID pairs, and the client library holds no lookup code for them: class
> 3800 has no functions of its own besides its initializers (which create a native map object for
> the compiled pairs), no bytecode names class 3800, and none of its property IDs occurs in the
> bytecode (C50). The lookup is therefore in native or server code. M3 observation of the trait
> panel's specialisation choice at several point totals would settle the rule.

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

Confirmed (bytecode + BIN, C38, C39). The client library treats the kinds differently:

- **Array kinds** (property, float, text, DID arrays and progressions of progressions): the index
  is clamped to the covered range before the element is read. With minimum index *m* and *n*
  elements, the index becomes max(*m*, min(*m* + *n* − 1, *x*)), signed 32-bit comparisons; an
  index below the range gives the first element, above it the last. (With *n* = 0 the clamp
  yields *m* − 1 and the original then reads outside the array, unchecked; no record has an empty
  array, P1. Tessera returns none, §4.10.)
- **Linear progressions**: no clamp. Below the first key the value is extrapolated along the
  **first** segment, from the last key on along the **last** segment, with the formula of §4.4
  step 2 (for example, with points (1, 10.0) and (11, 30.0), index 0 gives 8.0 and index 15 gives
  38.0).

LC agrees for linear progressions above the range and returns no value below it (Reported, §16).
Decision D1 is superseded: its clamp is the original's rule for array kinds, not for linear
progressions.

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

Inputs: a progression DID and a signed 32-bit integer index *x*. A caller that holds a float
level converts it first by truncation toward zero (the original's conversion gives −2³¹ for NaN or
out-of-range values). Output: a value or "none". The rules are the original's (C38, C39) except
where a step says "Tessera".

1. Load the property set of the DID. If it is missing, return none. If it has a linear array
   (`LinearInterpolatingProgression_Array`), go to step 6.
2. Find the value array of §4.1. If none is present, return none.
3. Let *m* be `Progression_MinimumIndexValue`, or 0 when absent, and *n* the element count.
   If *n* = 0, return none (Tessera; the original would read outside the array, §4.8).
4. Clamp: replace *x* by max(*m*, min(*m* + *n* − 1, *x*)) and let *k* = *x* − *m* (§4.8).
5. Return element *k*: a property value (1389), an f32 (1385), a StringInfo (2402) or a DID (2519
   and 3810). For a progression of progressions the two-index evaluation of §4.5 is performed by
   the modifier-entry evaluation (§5.6), which calls this algorithm twice: once here for the outer
   element DID, once for the inner progression at the inner index. Done.
6. Linear, with *n* points (*x₀*, *y₀*) … (*xₙ₋₁*, *yₙ₋₁*) in stored order: if *n* < 2, return 0.0
   (the original's value without segments, §4.4 step 3). If the keys do not strictly increase
   (they do in every record, P3), the record is malformed: return none and log it (Tessera).
   Otherwise choose the segment by §4.4 step 1 (the containing segment, else the first below *x₀*,
   else the last from *xₙ₋₁* on) and return *yᵢ* + *s* × f32(*x* − *xᵢ*) computed as in §4.4
   step 2, with *s* the stored or recomputed slope (P4).

Limits for hostile data: refuse arrays longer than the record allows (the decoder already
bounds them). Nesting cannot recur: the modifier-entry evaluation evaluates an inner progression
with steps 1–6 only, so an inner progression of progressions yields a DID and stops (§4.5).

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
- **P8** Every linear progression has at least two points (1,814 of 1,814, §4.1 lengths), so
  every one has at least one segment.

<a id="s4-12"></a>
### 4.12 Worked synthetic examples

- Float array, minimum index 1, elements (0.5, 0.75, 1.0): index 1 → 0.5, index 3 → 1.0,
  index 4 → clamped to 3 → 1.0, index −5 → clamped to 1 → 0.5 (§4.8).
- Float array without a minimum index, elements (2.0, 4.0): index 0 → 2.0, index 1 → 4.0.
- Linear, points (1, 10.0), (11, 30.0), (21, 30.0): slopes f32(20.0 / 10.0) = 2.0 and 0.0;
  index 6 → segment 0 → 10.0 + 2.0 × 5 = 20.0; index 11 → segment 1 (half-open) → 30.0 + 0.0 × 0
  = 30.0; index 21 (the last key) → last segment → 30.0 + 0.0 × 10 = 30.0; index 25 → last
  segment → 30.0; index 0 → first segment → 10.0 + 2.0 × (−1) = 8.0 (§4.8).
- Linear, points (1, 0.1), (3, 0.7): slope f32(0.6 / 2.0); index 3 (the last key) is computed as
  0.1 + slope × 2.0 in f32, which a reader must not replace by the stored 0.7 (the two can differ
  in the last bit).
- Progression of progressions *P*, minimum index 1, elements (*A*, *B*), where *A* is a float
  array with minimum index 1 and elements (10.0, 20.0, 30.0), and *B* is linear with points
  (1, 100.0), (3, 300.0) (slope f32(200.0 / 2.0) = 100.0), used by a modifier entry (§5.6):
  - evaluation level 2, inner-index property 2: outer 2 selects *B*; *B* at 2 is in segment 0:
    100.0 + 100.0 × 1 = 200.0;
  - evaluation level 1, inner-index property 3: outer 1 selects *A*; *A* at 3 is element 2 = 30.0;
  - evaluation level 3, inner-index property 1: outer 3 is clamped to 2 and selects *B*; *B* at 1
    → 100.0;
  - evaluation level 3, no inner-index property: outer 1 selects *A*, inner 3 → 30.0;
  - evaluated on its own at index 2 (§4.10 step 5): the DID of *B*.

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

Census over every `Mod_Array` in every 0x78 and 0x79 set, **at any nesting depth**: 116,435
arrays (16,947 in effects, 2,262 in traits, 97,226 in other objects, mostly items), 372,834
entries. Of the arrays, 112,502 are top-level properties and 3,933 are nested (C55): 3,929 in
the `Set_ActiveCountData` of item-set tiers (`items.md` §7.1), 2 in `SPM_PropertyMod` and 2 in
`Usage_ModPropertyUsageData_ModProperties` (the property-modifying tokens, `items.md` §2.1). No
nested array occurs in an effect or a trait. A nested array has the layout and evaluation of this
section; when it is applied is the holder's rule (for set tiers, while the tier is active,
`items.md` §7.1; for the tokens, not specified).

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

> **Answered (2026-10-09; was OPEN, implementation census 2026-10-07):** The 116,435 arrays and 372,834 entries of this
> section are reproduced only when `Mod_Array` values nested inside other values are counted too
> (3,933 arrays below top level, none in effects or traits); top-level arrays alone give 112,502.
> Please state that the census covers `Mod_Array` at any depth, and whether a reader should apply
> nested arrays (they sit inside item structs whose meaning is `items.md`'s, not this spec's).
>
> *Answer:* §5.2 now states that the census covers every depth and where the 3,933 nested
> arrays sit (C55). A reader evaluates a nested array like any other; whether and when it
> applies belongs to the holder (`items.md` §7.1 for set tiers).

<a id="s5-3"></a>
### 5.3 Operators (keys Confirmed, arithmetic Confirmed for the client library)

Mapper 0x23000046 has 12 keys. The labels are interoperability identifiers; a reader resolves
the operator by label at run time and falls back to the key numbers below. The "Effect" column is
what the client library does when it applies a value *v* to a property *P* (*P* := *P* op *v*;
C41); LC's reading (Reported, keys 5–8 only) agrees.

| Key | Label | Entries (inline / progression) | Value types seen | Effect | Identity value |
|---|---|---|---|---|---|
| 0 | `Invalid` | empty entries only | | entry rejected (§5.6) | |
| 1 | `Difference` | 0 | | not handled | none |
| 2 | `Divide` | 0 | | *P* ÷ *v* (float: f32 division; int and int64: signed, truncated toward zero; no zero check) | 1 |
| 3 | `Or` | 2,227 / 20 | bool, 32/64-bit and any-width bitfields | bitwise or: bool true when either is true; 32-bit and 64-bit bitfields or their 32 or 64 bits; an any-width bitfield first widens the shorter operand with zero bits to the longer one's width, so the result has the larger width | 0 |
| 4 | `Random` | 0 | | not handled | none |
| 5 | `Set` | 8,365 / 28 | only types the client allows for it (below) | *P* becomes a copy of *v*, for every type (no type-specific step) | none (an error) |
| 6 | `Subtract` | 3,762 / 357 | float, int | *P* − *v* (f32; 32-bit wrapping for int; 64-bit wrapping for int64) | 0 |
| 7 | `Add` | 34,821 / 298,697 | float, int | *P* + *v* (f32; 32-bit wrapping for int; 64-bit wrapping for int64) | 0 |
| 8 | `Multiply` | 10,939 / 41 | float, int | *P* × *v* (f32; the low 32 bits for int; the low 64 bits for int64) | 1 |
| 10 | `ArrayAdd` | 181 / 0 | arrays | each element of the array *v* is appended to *P*; nothing happens when *v* is not an array | none (an error) |
| 11 | `And` | 0 | | not handled | none |
| 12 | `Xor` | 0 | | not handled | none |

For float, int, bool and 32-bit bitfield properties the arithmetic reads *v*'s stored 32 bits as
the type of *P* (the two types always agree in the data, M2). For int64 properties and for 64-bit
and any-width bitfields the client holds the value in a separate value object and calls that
object's own operation (BIN b6): int64 implements `Add`, `Subtract`, `Multiply` and `Divide`
(signed, truncated toward zero) in 64 bits as the table says and leaves *P* unchanged for `Or`;
the two bitfield kinds implement `Or` as the table says. An operator applied to a type the table
does not list for it leaves *P* unchanged (the load check below rejects such entries first). No
entry with key 6, 7 or 8 modifies an int64 property in this install, so the int64 arithmetic is
read from the code only (C41).

**Valid operators per value type** (the client's load check, C40; every entry in the data passes):

| Value type of the modified property (`property-system.md` §3) | Operators allowed |
|---|---|
| float (9), int (15), int64 (19) | `Set`, `Subtract`, `Add`, `Multiply` |
| array (12) | `Set`, `ArrayAdd` |
| enum (8), DID (20) | `Set` |
| 64-bit bitfield (14), 32-bit bitfield (18), boolean (21), any-width bitfield (22) | `Or`, `Set` |
| every other type | none (the entry is rejected) |

`Divide` is handled by the arithmetic but allowed for no type. The **identity value** is what an
entry yields when it does not apply at the evaluation level (§5.5): applying it leaves *P*
unchanged. It is created in the modified property's type.

> **OPEN:** (§18 Q6) The order in which several entries (from several effects, items and traits)
> touching one property are combined, and whether `Set` wins over arithmetic. The client library
> turns each entry into an (operator, value) pair (§5.6) and applies one pair as in the table above,
> but the code that collects the pairs of all active sources and orders them is not in the client
> library's effect, trait or modifier classes read here (§13.2); it is native or server code
> (§13.1). M3 observation (a stat panel with one additive and one multiplicative bonus active)
> would settle the order. Tessera's sandbox uses (Decision D2, not a fact): start from the
> property's base value, apply `Set` entries (last applied wins), then `Add`/`Subtract`, then
> `Multiply`, then bitwise operators, then `ArrayAdd`.

<a id="s5-4"></a>
### 5.4 Modifier lists

(C35.) 1,946 non-empty entries carry a modifier list with at least one non-zero PID (lists hold 1 or
2 PIDs). `Mod_ModifierOp` is then 5, 6, 7 or 8 (668, 108, 887, 283 entries). Zero PIDs are
padding: the client keeps only the non-zero PIDs, in stored order, and treats a list without any
as absent. `Mod_ModifierOp` is non-zero exactly when a non-zero PID is present (the client rejects
the entry otherwise; every entry passes, C40), and must be an operator allowed for the modified
property's type (§5.3).

Combination (Confirmed, bytecode + BIN, C44): after the entry's value *v* is obtained (§5.6), for
each listed PID in order whose property is present in the property set that accompanies the
evaluation, *v* := *v* op *a*, where op is `Mod_ModifierOp` and *a* that property's value (§5.3
semantics, *v* in the role of *P*). Absent properties are skipped; without a property set the list
is ignored. So with key 7 the listed values are added to *v*; with `Set` the last present listed
property replaces it. LC's reading (Reported) agrees for key 7. For an applied effect the property
set is the one its applied-effect record holds (§4.5); which actor's properties the server puts
there is not in the client library.

One more client rule (C44): when the client evaluates an effect's `Mod_Array` it keeps at most one
(operator, value) pair per modified property; if two entries that apply at the evaluation level
modify the same property, the second is refused and the client drops the **whole** array's result.
No effect or trait array has two such entries (79 arrays with two unranged entries for one property
exist, all in class-1346 objects, which this rule does not concern).

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
applies at any *L*, because the ranges are disjoint; in a gap none applies.

Client rule (Confirmed, bytecode, C42):

- Only progression-driven entries have a range: the client's inline-entry object has no floor or
  ceiling, and every ranged entry in the data is progression-driven.
- *L* is the entry's **evaluation level** truncated toward zero: the same integer that indexes the
  entry's progression (§5.6). For an effect's or trait's array it is the applied effect's
  spellcraft (§1); for items LC reports the item's level (Reported).
- The range is tested before the progression is loaded. Outside it the entry yields the identity
  value of its operator (§5.3), which leaves the property unchanged (a `Set` or `ArrayAdd` entry
  outside its range is logged as an error by the client; no such ranged entry exists).
- At load the client rejects an entry whose stored floor is ≤ 0, whose stored ceiling is ≤ 0, or
  whose floor is not below its ceiling (floor = ceiling is rejected). No entry in the data is
  rejected (C42); floors are 1–300 and ceilings 9–1000.

<a id="s5-6"></a>
### 5.6 Evaluation of one entry (in words)

Confirmed (bytecode + BIN) as the client library's rule (C40–C44). Inputs: the entry, an
**evaluation level** (a float: for an applied effect its spellcraft, §1) and an optional
**property set** that accompanies the evaluation (for an applied effect, the set its record holds).

1. **Checks** (made by the client's load and verification functions for entries, before any
   evaluation): skip the entry if `Mod_Modified` = 0 or the
   struct is empty; reject it if `Mod_Op` is 0 or not allowed for the modified property's type,
   if `Mod_ModifierOp` and the modifier list disagree (§5.4), if a stored floor or ceiling fails
   the checks of §5.5, if a progression-driven entry's progression does not load, or if that
   progression does not **represent** the modified property. A progression represents: for a
   property array, the PID of its elements; for a float array, `FloatProgression_ValueProperty`,
   or `FloatProgression_FloatValue` when absent; for a linear progression,
   `LinearInterpolatingProgression_ValueProperty`; for a text array (2402),
   `StringInfoProgression_ValueProperty`, or `StringInfoProgression_Value` when absent; for a DID
   array (2519), `DataIDProgression_ValueProperty`, or `DataIDProgression_DataIDValue` when absent;
   for a progression of progressions, `ProgressionProgression_InnerRepresentedPropertyName` (the
   client tests for class 3810 first, so the DID-array rule does not apply to it). No modifier
   entry in this install names a text or DID array (the `Mod_Progression` targets of §4.9 are
   classes 3918, 1389, 1385 and 3810 only). Under the rules above, an entry naming a text array
   would always be rejected, because no operator is allowed for StringInfo properties (§5.3), and
   one naming a DID array passes only with `Set` on a DID property. Every entry in the data
   passes (C40).
2. **Level**: *L* = the evaluation level truncated toward zero.
3. **Range** (progression-driven entries only): if *L* is outside the stored range, the result is
   the operator's identity value (§5.3, §5.5); stop.
4. **Value**: the inline field, or `Mod_Progression` evaluated at *L* (§4.10). For a progression of
   progressions use the two indices of §4.5: outer *L* and inner = the property set's
   `Mod_ProgressionProgressionInnerIndex`, or outer 1 and inner *L* when that property is absent.
   If a progression does not load, the entry fails (and with it the whole array's result in the
   effect path, §5.4).
5. **Modifier list**: combine the property set's values of the listed properties into the value
   with `Mod_ModifierOp` (§5.4).
6. The result is the pair (`Mod_Op`, value). Applying it to a property *P* is *P* := *P* op value
   (§5.3); the order across entries and sources is OPEN (Q6).

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
- **M5** Every non-empty entry passes the client's checks of §5.6 step 1 (60,295 inline and
  299,143 progression-driven entries); in particular every progression-driven entry's progression
  represents exactly `Mod_Modified` (299,143 of 299,143), every stored floor and ceiling is > 0,
  and floor < ceiling wherever both are stored (C40, C42).
- **M6** Every ranged entry is progression-driven (19,533 of 19,533).
- **M7** No effect or trait `Mod_Array` has two entries for one property that can apply at the
  same level (C44).

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
| | `Effect_Duration_ConstantInterval_ModifierList` | 0x10000712 | PID array | 486 | §3.3 |
| | `Effect_PulseCount_AdditiveModifiers` | 0x100009CB | PID array | 277 (classes 755: 251, 708: 13, 734: 12, 2282: 1) | Pulse-count modifier list (§3.3, C51) |
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

**Duration as the client library computes it** (Confirmed, bytecode + BIN, C45). Inputs: the
effect, its spellcraft (§1) and a property source supplied with the request (may be absent). The
duration override, the modifier-list values and the equivalence-class factor values below are all
read from that one source.

1. **Override.** If the source holds `EffectGenerator_EffectData_BaseDurationOverride` (a field of
   the effect data of §6.3), the duration is that value, whatever its sign (0 and negative values
   included); go to step 5.
2. **Permanent.** If `Effect_Duration_Permanent` is true, the duration is unbounded (the client
   uses the largest finite f32, 3.4028235 × 10³⁸ s); stop. So the 1,658 records with both a
   permanent flag and an interval are permanent; their interval does not set the duration.
3. **Interval** *I*:
   - If `Effect_Duration_ConstantInterval` is greater than 0, *I* starts as that value.
   - Otherwise, if `Effect_Duration_ProgressionInterval` is a non-zero DID, *I* starts as that
     progression's value at the truncated spellcraft.
   - Otherwise there is no interval source: *I* = 0.0 at once. The modifier list below is not
     added and the 0.5 s minimum is not applied.
   When a source exists, add the current values of the properties in the interval's modifier list
   (`Effect_Duration_ConstantInterval_ModifierList`, §3.3; its compiled form is present in exactly
   the 486 effects that store the property, with the same PIDs in the same order) that the property
   source holds, as f32, and raise *I* to at least 0.5 s.
4. **Pulses.** If the effect has a pulse count (`Effect_Duration_ConstantPulseCount` > 0, or a
   non-zero `Effect_Duration_ProgressionPulseCount`), the duration is *I* × f32(*N*), where *N* is
   the constant, or the progression's integer value at the truncated spellcraft, plus the integer
   values of the properties in the pulse-count modifier list `Effect_PulseCount_AdditiveModifiers`
   that the property source holds, raised to at least 1. (The compiled half holds that list in
   exactly the 277 effects that store the property, with the same PIDs in the same order, C51.)
   Otherwise the duration is *I*. So `Effect_Duration_ConstantInterval` is the time **between
   pulses** when a pulse count is present (3,206 records) and the whole duration otherwise. With
   no interval source *I* is 0, so the duration is 0 here, pulses or not.
5. **Equivalence-class factors.** If the effect's `Effect_EquivalenceClass` is not 0 and a property
   source is given: for each of `Effect_EqvClassData_DurationModifier_Source` and `_Target` that the
   class's data names (§6.6), multiply the duration by clamp(1 + *v*, 0.1, 5.0), *v* being the
   source's float value of the named property (0 when absent); then raise the duration to at least
   0.5 s. This last step applies whenever both conditions hold, even when the class's data names
   neither property, and even to a duration of 0 from step 3 or a non-positive override from step
   1: such an effect gets 0.5 s. Without an equivalence class, or without a property source, step 5
   changes nothing, and an effect without any duration source keeps 0.

A separate client query asks whether an effect **is permanent** (this is not the duration): it
answers no when the property source holds `EffectGenerator_EffectData_BaseDurationOverride`
greater than 0, and otherwise returns `Effect_Duration_Permanent`. The duration computation above
uses a present override of any sign before it looks at the permanent flag.

All arithmetic is f32. Whether the server uses the same rule is not in the data (§13.1); LC reads
the interval as the duration and the pulse count separately (Reported), which agrees for effects
without pulses.

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

> **Answered (2026-10-09; was OPEN, implementation census 2026-10-07):** The 4,954 bare `Effect_WSLEffect` values in
> effects include 2,649 zero values (2,305 non-zero). Are the zeros placeholders in the sense of
> §3.4 (skipped by readers), as the implementation assumes?
>
> *Answer:* yes. A DID value 0 means "none" in every reference property (§3.4,
> `property-system.md` §4.9), and `Effect_WSLEffect` is a DID reference (dictionary type DID,
> default 0). The zeros sit in `Effect_Combo_EffectPresentList` (2,648) and
> `Effect_Dispel_ByEffectList` (1); a reader skips them (C56).

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

**Grouping rule of these counts (C57).** The class-725 counts above treat *S*`Variance` as a
member of the group; the class-755 counts leave it out. Counted the same way for both classes,
with the variance **left out** (the variance is a spread, not a source):

| Class | Sources stored (variance left out) | Records |
|---|---|---|
| 725 | progression only | 788 |
| 725 | constant only | 229 |
| 725 | variable only | 52 |
| 725 | constant and progression | 19 |
| 725 | constant and variable | 5 |
| 725 | progression and variable | 4 |
| 725 | none | 4 |
| 725 | random only; progression and random | 1 each |
| 755 | initial and per-interval progressions | 1,651 |
| 755 | per-interval progression only | 716 |
| 755 | initial and per-interval constants | 32 |
| 755 | initial progression, per-interval constant and progression | 29 |
| 755 | per-interval progression and variable | 5 |
| 755 | per-interval constant and progression | 2 |
| 755 | per-interval constant only | 2 |
| 755 | per-interval random only | 1 |

Variance present: class 725 `Effect_InstantVital_InitialChangeVariance` in 262 records (259 with a
progression, 3 with a constant); class 755 `Effect_VitalOverTime_InitialChangeVariance` in 143 and
`Effect_VitalOverTime_ChangePerIntervalVariance` in 281.

**Full labels per stem (C57)** (all arrays hold `Effect_ModifierPropertyList_Entry` PIDs; all
multipliers are f32 with dictionary default 1.0):

| Stem | Modifier list | Additive modifier list | Critical multiplier | Its modifier list |
|---|---|---|---|---|
| `Effect_InstantVital_InitialChange` (725) | `Effect_InstantVital_InitialChange_ModifierList` (526) | `Effect_InstantVital_Additive_InitialChange_ModifierList` (22) | `Effect_InstantVital_Critical_Multiplier` (1,103) | `Effect_InstantVital_CriticalMultiplierModList` (304) |
| `Effect_VitalOverTime_InitialChange` (755) | `Effect_VitalOverTime_InitialChange_ModifierList` (1,117) | `Effect_VitalOverTime_AdditiveInitialChange_ModifierList` (1) | `Effect_VitalOverTime_InitialChange_Critical_Multiplier` (2,438) | `Effect_VitalOverTime_InitialChange_Critical_Multiplier_ModList` (185) |
| `Effect_VitalOverTime_ChangePerInterval` (755) | `Effect_VitalOverTime_ChangePerInterval_ModifierList` (2,140) | `Effect_VitalOverTime_AdditiveChangePerInterval_ModifierList` (12) | `Effect_VitalOverTime_ChangePerInterval_Critical_Multiplier` (2,438) | `Effect_VitalOverTime_ChangePerInterval_Critical_Multiplier_ModList` (357) |

The labels do not follow one pattern (note the placement of `Additive` and the two critical
forms of class 725), so a reader looks each one up by its full label.

**Which source is used** (Confirmed, C46). The client uses exactly **one** source per amount. The
choice is not made from the property half at run time: it is stored in the compiled half, as flag
bits of the effect's flags attribute (class 698, attribute hash 0x0C868373). For the instant-vital
amount (class 725) the client's amount function reads bit 0x10 as "constant", 0x20 as "random",
0x40 as "variable" and 0x80 as "progression" (evaluated at the truncated spellcraft), and at most
one of these bits is set in every record. Where a record stores several sources, the flags select:

| Sources stored | Records | Source used |
|---|---|---|
| constant and progression (class 725) | 19 | progression |
| per-interval constant and progression, with an initial progression (class 755) | 29 | progression (per-interval) |
| per-interval constant and progression (class 755) | 2 | progression |
| progression and variable (725: 4; 755 per-interval: 5) | 9 | progression |
| progression and random (725) | 1 | progression |
| constant and variable (725) | 5 | variable |

A reader takes the source from the compiled flags when it has the compiled half; from the property
half alone the rule is: progression, else variable, else random, else constant (random and constant
never occur together). The 4 class-725 records without any source have none of the bits set and
an amount of 0. LC prefers the constant (Reported), which disagrees (§16).

For class 755 the client's two amount functions (C46) read the same attribute:

| Amount | Constant | Random | Variable | Progression | No bit of the group set |
|---|---|---|---|---|---|
| Initial change | 0x10 | 0x20 | 0x40 | 0x80 | amount 0 |
| Change per interval | 0x100 | 0x200 | 0x400 | 0x800 | amount 0 |

Each function tests its four bits in the order of the table (constant, random, variable,
progression) and uses the first that is set; in every class-755 record at most one bit of each
group is set, so the order never decides (OBS, all 2,438). Bits observed in this install: 0x10,
0x80, 0x100, 0x200 and 0x800. The bits 0x20, 0x40 and 0x400 are set in no record: no class-755
record stores an initial random or variable amount, and the 5 records that store both a
per-interval progression and a per-interval variable amount have only 0x800 (the progression). The
lower bits 0x1, 0x2 and 0x4 of the same attribute are set in records too; the amount functions do
not test them.

> **Answered (2026-10-09; was OPEN, implementation census 2026-10-07):** The class-755 co-occurrence counts (1,651, 716,
> 32, 29, 10) are reproduced only when *S*`Variance` is left out of the grouping (for class 725
> it is counted). Please state the grouping rule. Also, the labels behind
> "*S*`_ModifierList`, `…Additive…_ModifierList`" and "`…_Critical_Multiplier`,
> `…_Critical_Multiplier_ModList`" are not spelled out, so the implementation does not read
> them; please list the full labels per stem.
>
> *Answer:* the two paragraphs after the co-occurrence counts state the grouping rule, give both
> classes counted without the variance, and list the full labels per stem (C57).

<a id="s6-5"></a>
### 6.5 Class-specific data that carries values

Counts are records of the class that carry the property, out of the class's records (C29).

| Class (records) | Values | Properties |
|---|---|---|
| Any of 14 classes | stat changes | `Mod_Array` (§5), 16,947 records: 734 (14,475 of 14,475), 708 (1,245 of 1,245), 2459 (719 of 722), 713 (190 of 666), 736 (125 of 356), 3686 (62), 3222 (49), 3218 (40), 780 (29), 3842 (5), 752 (4), 748 (2), 764 (1), 3690 (1). All holder classes except 708 are 734 or descend from it; not every record of a descendant has one. |
| 709 (617) | aura | `Effect_Aura_Type` (617; mapper 0x23000128, 3 keys; value 1 in 596, 2 in 21), `Effect_Aura_Radius` (602; 1.3–120), `Effect_Aura_Applied_Effect_Array` (617; 734 `Effect_Aura_Applied_Effect_Data` structs, each `Effect_Aura_Applied_Effect` (DID → effect) and `Effect_Aura_Applied_Effect_Spellcraft` (f32, −1 = not given; −1–130)), `Effect_Aura_Spellcraft_Override` (596; −1–60), `Aura_ShouldAffectCaster` (617), `Effect_Aura_AffectsPlayers` (405), `Effect_Aura_AffectsMonsters` (304) and four more `Effect_Aura_Affects*` flags (5–42) |
| 2762 (453) | area | `Effect_Area_MaxRange` (453; 1–120), `Effect_Area_MaxTargets` (452; 1–50), `Effect_Area_Applied_Effect_Array` (453; 578 `Effect_Area_Applied_Effect_Data` structs, each `Effect_Area_Applied_Effect` (DID) and `Effect_Area_Applied_Effect_Spellcraft` (f32, default −1)), seven `Effect_Area_Affects*` flags (9–294), `Effect_Area_ShouldAffectCaster` (279), `Effect_Area_MaxTargets_AdditiveModifiers` (110) |
| 708 (1,245) | periodic application | `Effect_ApplyOverTime_Applied_Effect_Array` (1,245; 1,365 `Effect_ApplyOverTime_Applied_Effect_Data` structs, each `Effect_ApplyOverTime_Applied_Effect` (DID) and `Effect_ApplyOverTime_Applied_Effect_Spellcraft` (f32, default −1)), `Effect_ApplyOverTime_Initial_Applied_Effect_Array` (819; 1,070 structs of the same kind). The same struct also fills the two `Effect_PersistentCombo_EffectsToAdd*_Array` properties of class 3124 (20 structs) |
| 3686 (461) | skill proc | `Effect_SkillProc_SkillTypes` (434; 64-bit bitfield, mapper 0x230003A4), `Effect_SkillProc_RequiredCombatResult` (317; 32-bit bitfield, mapper 0x230003A8), `Effect_SkillProc_ProcProbability` (259; 0–0.8), `Effect_MinTimeBetweenProcs` (269; 0.25–180 s), `EffectGenerator_SkillProc_UserEffectList` (380), `EffectGenerator_SkillProc_TargetEffectList` (92) |
| 3222 (88) | absorb | `Effect_Bubble_VitalType` (88; mapper 0x23000003), `Effect_Bubble_Value` (42; 200–20,000,000), `Effect_Bubble_Value_Progression` (8), `Effect_Bubble_Percentage` (38; 0.05–0.55), `Effect_BubbleValue_ModifierList` (88) |
| 731 (86) | pips | `Effect_Pip_Type` (86; mapper 0x23000071), `Effect_Pip_AdjustmentAmount` (84; −20–1,000), `Effect_Pip_AdjustmentType` (84), `Effect_Pip_Reset` (2) |
| 767 (1,189) | combinations | `Effect_Combo_EffectPresentList`, `_EffectClassPresentList`, `_RemoveOldEffectIfPresent`, `_RemoveAllOldEffectsIfPresent`, `_CasterOnly` (all 1,189); `_EffectToAddIfPresent` (979), `_EffectToAddIfNotPresent` (747), `_EffectToExamine` (189), `_EffectToGiveBackIfPresent` (173), `_EffectToGiveBackIfNotPresent` (76) |
| 3124 (14) | persistent combinations | `Effect_Combo_EffectPresentList`, `_EffectClassPresentList`, `_CasterOnly` (all 14); `Effect_PersistentCombo_EffectsToAddIfAbsent_Array` (10), `Effect_PersistentCombo_EffectsToAddIfPresent_Array` (9) |
| 3866 (526) | tiers | `Effect_TierUp_EffectList` (526; 3,141 `EffectGenerator_EffectStruct`), `Effect_TierUp_FinalEffect` (462; an array of one `EffectGenerator_EffectStruct` each), `Effect_TierUp_ShowInExamination` (416) |

> **Answered (2026-10-09; was OPEN, implementation):** The fields of `Effect_Aura_Applied_Effect_Data`,
> `Effect_Area_Applied_Effect_Data` and `Effect_ApplyOverTime_Applied_Effect_Data` ("an effect DID
> and a spellcraft") are not named, so the implementation's effect-reference reader (§6.3) does
> not recognise these structs. Which labels hold the effect DID and the spellcraft?
>
> *Answer:* the struct name without `_Data` holds the effect DID, and that name with
> `_Spellcraft` the spellcraft (f32, dictionary default −1, "not given"); every struct holds
> both (C58). The table above names them. A reader adds `Effect_Aura_Applied_Effect`,
> `Effect_Area_Applied_Effect` and `Effect_ApplyOverTime_Applied_Effect` to the ID fields of
> §6.3.

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

`Effect_EqvClassData_DurationModifier_Source` and `_Target` name float properties whose values
scale the duration of every effect of the class by clamp(1 + *v*, 0.1, 5.0) each (§6.2 step 5,
C45).

> **OPEN:** (§18 Q10) The stacking rule. The names suggest: at most *MaxDepth* effects of one class
> held at once, at most *MaxExpressedEffs* of them active, ordered by a sorter, counted per caster
> when *IsPerCaster*; `Effect_ClassPriority` and `Effect_Cap` take part. None of this is stated
> by the data or a source with authority. The client library's effect control class keeps the
> per-class data in compiled lookup maps (looked up by equivalence class, with defaults 0 or 1 for
> a class without data), but this pass did not identify which map holds which field nor find code
> that enforces a limit; applying effects is server logic (§13.1). M3 observation (applying more
> effects of one class than *MaxDepth* allows, and watching which stay) would settle it.

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
| 1483 | 125 | Trait granting skills | `Trait_Skill_Array` (an array of bare `Trait_Skill` DIDs, 135 in all; C58) |
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

> **OPEN:** (§18 Q11) Which of *a* and *b* is the row and which the column, and what the
> 0x10000000 group is. A tree view needs it; LC passes the labels through without interpreting
> them, and the client library's bytecode never names the trait-tree properties (C50), so the
> layout is not computed by script code read in this pass. M3 observation of the trait panel (which
> cell a known trait occupies), or a reading of the panel's layout data, would settle it.

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
| `AdvTable_AvailableSkillEntryList`, `AdvTable_AvailableMountSkillEntryList` | 551 and 36 entries over the 12 records | Both hold `AdvTable_AvailableSkillEntry` structs with six fields, all present in all 587: `AdvTable_Skill` (DID), `AdvTable_Level` (int), `AdvTable_PrerequisiteSkill` (DID), `AdvTable_Rank` (int), `AdvTable_TrainingCost` (int), `AdvTable_AdvancedCharacterShortcutIndex` (int) (C58) |
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
> order. None of the `AdvTable_DerivedStat*` property IDs occurs in the client library's bytecode
> (C50); whether script code reads the formula through the class-853 compiled attributes was not
> checked. This is server logic; M3 observation of the character panel (a primary stat changed by a
> known amount) could test the reading above.

> **Answered (2026-10-09; was OPEN, implementation):** The field labels of `AdvTable_AvailableSkillEntryList` and
> `AdvTable_AvailableMountSkillEntryList` entries ("skill DID, level, prerequisite skill, rank,
> training cost, shortcut index"), of `MonsterPlay_SkillList` and `MonsterPlay_TraitList` entries
> (§9.2) and of `Trait_Skill_Array` elements (§8.1) are not given. The implementation offers every
> DID inside these lists and nothing else; the labels would let it type the entries.
>
> *Answer:* §9.1, §9.2 and §8.1 now give every field label and type (C58).

<a id="s9-2"></a>
### 9.2 Monster-play tables (class 1932)

67 records: `MonsterPlay_MonsterInfo` (names, class, subspecies, icons),
`MonsterPlay_MonsterLevel` (1–160), `MonsterPlay_SkillList` (66 records, 525 `MonsterPlay_SkillEntry`
structs: `MonsterPlay_Skill` DID, `MonsterPlay_SkillCost`, `MonsterPlay_SkillMinLevel`,
`MonsterPlay_SkillMinRank` ints), `MonsterPlay_TraitList` (7 records, 687 `MonsterPlay_TraitEntry`
structs: `MonsterPlay_Trait` DID, `MonsterPlay_TraitMinLevel` and `MonsterPlay_TraitMinRank` ints,
`MonsterPlay_TraitTrainable` bool), `MonsterPlay_VirtueList` (7 records, 84
`MonsterPlay_VirtueEntry` structs: `MonsterPlay_Trait` DID, `MonsterPlay_VirtueStartRank` and
`MonsterPlay_VirtueMaxRank` ints); every field is present in every entry (C58); default
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

**Rating curve** (Confirmed, bytecode + BIN, C47). The compiled half of the control object holds
three arrays of 31 progression DIDs, indexed by calculation type (element 0 empty, elements 1–30 =
the structs' `Combat_Control_Rating`, `Combat_Control_Target_Cap` and `Combat_Control_Hard_Cap`,
30 of 30 matching). The client converts a rating *R* (f32) into a fraction for calculation type
*c* at level *L* (integer) with an additive cap bonus *b* (f32, 0 unless a caller supplies one):

1. If *R* ≤ 0, or *c* = 0, the result is 0. If *L* < 1, the client logs an error and the result is 0.
2. Evaluate the three float progressions of type *c* at *L* (§4.10, so *L* is clamped to each
   curve's range): *Rt* = Rating(*L*), *T* = TargetCap(*L*), *H* = HardCap(*L*) + *b*.
3. The result is min(*H*, *T* × (*R* / (*R* + *Rt*))), computed in f32 in that order: *R* + *Rt*,
   then the division, then the product with *T*, then the minimum.

Some callers then subtract a value from the result and raise it to at least 0. LC's form
TargetCap / (1 + Rating / *R*), capped at HardCap, is the same function in exact arithmetic but
rounds differently in f32 (§16). Whether the server uses the same code is not in the data (§13.1).

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

**Lookups** (Confirmed, bytecode, C48). Element 0 of both base arrays belongs to level 1:

- **DPS table** (class 1660) at item level *L* and quality *q*: if the table has no quality struct
  for *q*, or *L* = 0, or *L* is greater than the element count (compared unsigned, so a negative
  *L* also fails), the client logs an error and returns −1.0. Otherwise the result is
  `Combat_BaseDPS`[*L* − 1] × `Combat_DPSMod`(*q*), one f32 product.
- **Value table** (class 1688) at item level *L* and quality *q*: if *L* is greater than the
  element count, the client logs an error and uses the last element; *L* ≤ 0 is not checked (the
  original reads outside the array; Tessera treats it as no value). The quality factor is
  `Item_QualityModVal`(*q*), 0.0 when the table has no struct for *q*. The item's value is
  `Item_BaseVal`[*L* − 1] × factor + 0.5, computed in f32 and truncated toward zero to an integer.
- **Weapon damage**: see §10.2.1.

<a id="s10-2-1"></a>
#### 10.2.1 Weapon data (Confirmed, bytecode, C52)

Two lookup tables take part besides the DPS table:

| Table | Records | Property half | Compiled half |
|---|---|---|---|
| Weapon timing | one class-1692 record (0x70000254 here) and 24 class-1691 records | `Item_WeaponActionDurationTable`: 24 DIDs of class-1691 records; each of those holds `Item_EquipmentCategory` (64-bit bitfield, one bit) and `Item_WeaponActionDurationDataArray` of 6 structs {`Item_WeaponSpeed` (enum 0x23000066, keys 1–6), `Item_BaseActionDuration` (f32, seconds), `Item_BaseAnimDurationMultiplierMod` (f32)}; layout as `items.md` §6.3 gives it | class 1692, attribute 0x01D52CB5: a map from category value to class-1691 DID (24 pairs, each the DID whose `Item_EquipmentCategory` is the key); class 1691, attribute 0x0DF47C71: a reference map from speed key (1–6) to an object with the duration and the animation modifier |
| Weapon variance | one class-2618 record (0x7001BA69 here) | `Item_WeaponVarianceTable` of 3 structs {`Item_WeaponVarianceValue` (f32), `Item_WeaponVarianceEquipmentCategory` (64-bit bitfield, several bits: a category mask)} | attribute 0x0329CE35: a map from mask to the variance's f32 bits (the same 3 pairs) |

The client library's weapon-data function takes a weapon, a level *G* passed in by its caller (the
item-level computation of `items.md` §5.2) and an output property set. It runs these steps; "fail"
means it logs an error and writes nothing further:

1. If the weapon's `ItemAdvancement_Imbued` is true, it does nothing and reports success.
2. The object must be a weapon; `Item_EquipmentCategory` must be non-zero, `Combat_WeaponSpeed`
   non-zero; otherwise fail.
3. **DPS level** *L*: if the weapon has `ItemAdvancement_Item` true and its
   `ItemAdvancement_CombatPropertyModLevel` is 0 or absent, *L* = *G* − 3 when *G* > 3 and *L* = *G*
   otherwise, and the client writes *L* into the output as `ItemAdvancement_CombatDPSLevel`.
   Otherwise *L* = *G* + the level offset of the weapon's `Item_DistributionType` (`items.md`
   §6.4). If *L* ≤ 0, fail. `Item_Quality` must be non-zero; otherwise fail.
4. **Action duration *D*** and animation modifier: find the timing table whose category equals the
   weapon's `Item_EquipmentCategory` (the class-1692 map), and in it the entry whose speed key
   equals `Combat_WeaponSpeed`; its `Item_BaseActionDuration` is *D*, its
   `Item_BaseAnimDurationMultiplierMod` the modifier. No table or no entry: fail.
5. `Combat_DPS_LUT` must be non-zero; otherwise fail.
6. **Variance *v***: the entry of the variance table whose mask contains every bit of the weapon's
   category (mask AND category = category). No such entry: fail. If *v* < 0 or *v* ≥ 1, fail.
7. Write *v* as `Combat_DamageVariance`, *D* as `Item_BaseActionDuration` and the modifier as
   `Item_BaseAnimDurationMultiplierMod` into the output.
8. **DPS**: the DPS table lookup above at *L* and `Item_Quality`. If the result is ≤ 0, fail. If
   the weapon's `WeenieType` has both bits 0x01 and 0x80 set (`items.md` §2.3: the item marker,
   true for all 23,204 weapons here), add the weapon's `Combat_Damage_Mod` (0.0 when absent), read
   as the value after the weapon's own static effect list is applied to it (`items.md` §3.2, F32).
   No 0x79 set in this install holds `Combat_Damage_Mod` or names it as `Mod_Modified`, so the
   addition is 0 for every stored weapon.
9. **Maximum damage**: with *D*′ the weapon record's own `Item_BaseActionDuration` (−1.0 when
   absent) and *v*′ its own `Combat_DamageVariance` (0.0 when absent), damage =
   (2.0 × *D*′ × DPS) / (2.0 − *v*′), computed in f32 in that order: 2.0 × *D*′, then × DPS, then
   2.0 − *v*′, then the division. If damage ≤ 0, or the DPS of step 8 (after the addition) is
   ≤ 0, fail.
10. Write damage as `Combat_Damage` and DPS as `Combat_BaseDPS` into the output.

Step 9 reads the duration and variance from the weapon record, not the values written in step 7.
In this install they are the same: every weapon's stored `Item_BaseActionDuration` equals its
timing-table entry (`items.md` §9.7, 23,204 of 23,204) and its stored `Combat_DamageVariance`
equals the variance-table value (C52, 23,204 of 23,204). Census (C52): every weapon's category is a
single bit and a key of the timing map, every weapon's speed is a key of its table, and every
weapon's category lies in exactly one variance mask.

> **OPEN:** (§18 Q15) Which entry the client takes when a category lies in several variance masks:
> the function walks the compiled map and takes the first match, and the order of that walk is the
> native map's, which this pass did not read. No weapon in this install has more than one match, so
> the data does not decide. Tessera takes the first match in the property half's stored order
> (Decision D5).

LC's formula (maximum = 2 × DPS / (2 − variance) × weapon speed) agrees, with the weapon speed being
the action duration; LC's minimum (maximum × (1 − variance)) is not computed by this function
(Reported).

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
- stacking and removal rules (§6.6), and whether the server computes durations as the client
  library does (§6.2);
- threat.

A server implementation (out of scope before M8; decision 0004) has to define these itself.

<a id="s13-2"></a>
### 13.2 What the client library's bytecode contains

The client library holds bytecode bodies for the progression classes (1386: 9 own bodies;
1385: 6; 1389: 5; 3918: 8; 2402: 7; 2519: 7; 3810: 3; segments 3917: 6), for the compiled
modifier classes 1339–1342, and for many effect, skill and trait functions. A byte search of
the bytecode chunk (`wlib.md` §4.1) finds the property IDs of `Mod_Op`, `Mod_ModifierOp`,
`Mod_Modified`, `Mod_Progression`, `Mod_ProgressionFloor`, `Mod_ProgressionCeiling`,
`Mod_ModifierList`, `Mod_Array`, `Combat_DPS_LUT` (8 times), `Combat_BaseDPS`,
`Combat_QualityModData`, `Item_ValueLookupTable`, `Item_Armor_Value_Lookup_Table`,
`Progression_Type`, `Progression_PointTier`, `FloatProgression_FloatValue`,
`LinearInterpolatingProgression_ValueProperty` and `Mod_ProgressionProgressionInnerIndex` as
32-bit literals; in all, 5,993 of the 27,546 property IDs occur. (A 4-byte literal can match by
chance, about once in 2,000 IDs for a block of this size; the IDs named here occur 1 to 18
times.)

With `wlib-bytecode.md` these bodies were read on 2026-10-08 (BC, and BIN for the natives they
call). What they settle, by section:

| Behaviour | Section | Row |
|---|---|---|
| Array progressions clamp the index; linear progressions choose a segment and extrapolate; f32 order; the last key | §4.3, §4.4, §4.8, §4.10 | C38, C39 |
| Progression of progressions in a modifier entry: outer and inner index; point-based traits supply the inner index | §4.5 | C43 |
| Modifier-entry checks, valid operators per type, operator arithmetic and identity values | §5.3, §5.6 | C40, C41 |
| Level ranges: which level, inclusive ends, identity outside | §5.5 | C42 |
| Modifier lists: order, operand, refusal of two entries for one property | §5.4 | C44 |
| Effect duration: override, permanent, interval × pulses, minimums, equivalence-class factors | §6.2, §6.6 | C45 |
| Vital amounts: one source, selected by compiled flags | §6.4 | C46 |
| Rating curve | §10.1 | C47 |
| DPS and value table lookups, weapon maximum damage, item value | §10.2 | C48 |
| Spellcraft as the evaluation level | §1 | C49 |

The bodies compute these values for the client's own purposes (examination tooltips and local
displays among them); which client feature calls each function was not traced. The server-side
library is not in the archives (§13.1), so these are facts about the client library, not proofs of
the server's arithmetic.

<a id="s13-3"></a>
### 13.3 Third-party formulas (Reported only)

LC reconstructs some tooltip numbers. Its rating curve and weapon damage agree with the client
library (§10.1, §10.2, §10.2.1, C47, C48, C52). One formula remains a hypothesis:

- skill hook maximum damage at character level *L*: (`HookDamageMax` + HookDamageMax
  progression(*L*) + Σ max-damage modifier properties + action duration × (DPS-add progression(*L*)
  + Σ DPS modifier properties) × `DamageAddContributionMultiplier`) × damage qualifier ×
  (`HookDamageModifier` + Σ modifier properties); minimum = maximum × (1 − `HookDamageMaxVariance`).
  The damage qualifier itself comes from a rating curve and percentage properties of the actor.

> **OPEN:** (§18 Q12) Whether the hook-damage formula above is the original's, and the order of
> its sums and products in f32. The client library has bodies that read
> `Skill_AttackHook_DPSAddMod_Progression` (13 sites, mostly class 364) and
> `Skill_AttackHook_HookDamageMax` (class 293 family); reading them with `wlib-bytecode.md`, as
> was done for §10, would settle the tooltip form. The server's form is not in the data (§13.1).

---

<a id="s14"></a>
## 14. Client behaviour settled by BIN

Negative results of the first pass (C25):

- The native client does not read the searched properties by ID and does not contain the
  searched labels as strings. The hits for the 10 searched property IDs (`Mod_Op`,
  `Mod_Progression`, `Mod_Array`, `Mod_Modified`, `Mod_Entry`, four progression array IDs and
  the minimum index) are all child-element lookups in user-interface code, where the same
  numbers are element identifiers; the 7 attribute hashes of §4.7 do not occur.
- No native code calls a function of the progression, effect, skill or trait classes by a
  constant function ID.

Positive results of the second pass (2026-10-08, b4–b5) concern only the native helpers that the
client library's bytecode calls (BIN row): integer and float minimum, maximum and clamp, unchecked
array element access, element counts, the float-to-property store, the property arithmetic of the
modifier operators, the map insertion that keeps the first key, and the two level readers. They
are used in §4.4, §4.8, §5.3, §5.4, §6.2 and §10.1. The third pass (2026-10-08, b6) followed the
set, or and arithmetic operations into the value objects of int64, 64-bit and any-width bitfield
properties (§5.3).

Consequence for Tessera: native client code does not evaluate progressions or modifiers; the
client's evaluation is script code in the client library (§13.2), and §4.10 and §5.6 describe it in
words. Tessera implements those rules itself; it does not need to run the bytecode.

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

A level or spellcraft selector (default 1) drives every progression shown on a card, evaluated
with §4.10 (out-of-range indices follow the original's rules of §4.8; a card marks such values as
"outside the stored range"). A progression of progressions is shown with an inner-index selector
as well (§4.5). Values the spec marks OPEN (stacking, the combination order of several sources)
are shown as "not evaluated" rather than guessed, so that the viewer never shows a Tessera choice
as data.

---

<a id="s16"></a>
## 16. Disagreements with other sources

| Topic | Other source says | This spec | Basis |
|---|---|---|---|
| Missing `Progression_MinimumIndexValue` | LC: 1 | 0 | dictionary default and compiled form (C6) |
| Effect classes with property modifications | DS: 713, 734, 748, 763 are "effects", 718 "flag effect" | 713, 748 derive from 734 and carry `Mod_Array`; 763 is a pet summon (§6.1) | OBS |
| Linear progression below the first key | LC: no value | extrapolated along the first segment (§4.8) | C39 |
| Linear progression at the last key | (Tessera's earlier Decision D3: the stored value) | computed from the last segment (§4.4) | C39 |
| Vital amount with both a constant and a progression | LC: the constant | the progression (§6.4) | C46 |
| Rating curve evaluation order | LC: TargetCap / (1 + Rating / *R*), then the cap | *T* × (*R* / (*R* + *Rt*)), then the minimum with the cap (same in exact arithmetic, different f32 rounding) (§10.1) | C47 |

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
| P8 | §4.11 |
| M5–M7 | §5.9 |
| R1 | The combat control object's three compiled rating-curve arrays have 31 elements each, element 0 empty, and element *c* equals the struct of calculation type *c* (30 of 30, §10.1). |
| V1 | In every class-725 record at most one of the amount-source flag bits 0x10, 0x20, 0x40, 0x80 is set, and where several sources are stored the selection of §6.4 holds (1,103 of 1,103). |
| V2 | In every class-755 record at most one of the bits 0x10, 0x20, 0x40, 0x80 and at most one of 0x100, 0x200, 0x400, 0x800 is set; 0x20, 0x40 and 0x400 are set in none (2,438 of 2,438, §6.4). |
| E3 | In every effect the compiled interval and pulse-count modifier lists exist exactly when `Effect_Duration_ConstantInterval_ModifierList` (486) and `Effect_PulseCount_AdditiveModifiers` (277) are stored, and hold their non-zero PIDs in stored order (32,971 of 32,971, §6.2). |
| W1 | Every weapon's `Item_EquipmentCategory` is one bit and a key of the timing map, its `Combat_WeaponSpeed` a key of that table, and its category lies in exactly one variance mask whose value equals its stored `Combat_DamageVariance` (23,204 of 23,204, §10.2.1). |

---

<a id="s18"></a>
## 18. Open questions (summary)

| # | Question | Where | Settles it |
|---|---|---|---|
| Q1 | Out-of-range progression indices | §4.8 | **Closed** 2026-10-08 (C38, C39) |
| Q2 | Exact f32 evaluation inside a linear segment and at the last key | §4.4 | **Closed** 2026-10-08 (C39) |
| Q3 | Which actor values supply the outer and inner index of a progression of progressions | §4.5 | **Closed** 2026-10-08 (C43); the trait-spellcraft part moved to Q9 |
| Q4 | Lookup rule of sparse DID progressions | §4.6 | open: no client-library code (C50); M3 trait panel |
| Q5 | Which level governs modifier level ranges in effects and traits | §5.5 | **Closed** 2026-10-08 (C42) |
| Q6 | Combination order of modifier entries from several sources; `Set` precedence | §5.3 | open: native or server code; M3 observation |
| Q7 | Duration semantics (interval vs. pulses; permanent with interval) | §6.2 | **Closed** 2026-10-08 for the client library (C45) |
| Q8 | Constant vs. progression precedence in vital amounts | §6.4 | **Closed** 2026-10-08 (C46) |
| Q9 | Origin of an applied effect's spellcraft when none is given; rank-based trait spellcraft | §1 | open: M3 tooltips, or a reading of the ranked-trait functions |
| Q10 | Stacking rules of equivalence classes | §6.6 | open: server logic; M3 observation |
| Q11 | Row/column of trait-cell keys; the 0x10000000 group | §8.5 | open: M3 (trait panel) or UI layout data |
| Q12 | Rating-curve and damage formulas; index base of DPS and value arrays | §10.1, §10.2, §13.3 | rating curve, table index base, weapon damage **closed** 2026-10-08 (C47, C48, C52); hook-damage formula open (§13.3) |
| Q13 | Derived-stat formula completeness | §9.1 | open: server logic; M3 character panel |
| Q14 | Property-half source of the pulse-count modifier list | §6.2 | **Closed** 2026-10-08 (C51: `Effect_PulseCount_AdditiveModifiers`) |
| Q15 | Variance entry chosen when a weapon category lies in several masks | §10.2.1 | open: native map order (BIN); no case in the data; Tessera uses D5 |

---

<a id="implementer-notes"></a>
## Implementer notes

Engineering choices of the implementation (`crates/formats/src/gameplay/combat.rs`) where this
spec is silent. They are not facts about the original; a spec writer may confirm or replace them.

> **Answered (2026-10-09; was OPEN, implementation choice):** A progression value array whose stored count includes null
> elements is rejected as malformed, because element positions are indices (§4.3) and a null
> element's position is not kept by the decoder. None occurs in the observed install. Elements of
> the wrong type (a float array holding a non-float, a control point without
> `LinearInterpolatingProgression_Key` or `_Value`) are rejected the same way, rather than
> replaced by the dictionary default.
>
> *Answer:* confirmed as Decision D6. No progression array of this install holds a null element
> (§4.1), every control point holds both fields, and every element has the array's type, so the
> choice changes no result here.

> **Answered (2026-10-09; was OPEN, implementation choice):** Whether a DID array is a progression of progressions is
> decided by the record's class (§4.1), not by `ProgressionProgression_InnerRepresentedPropertyName`.
> Index arithmetic is done in 64 bits (128 for *x* − *m*), so no index overflows. The D1 clamp is
> logged at debug level.
>
> **Answered in part (2026-10-08, merge):** §4.8 (C38, C39) now gives the client library's rule
> for out-of-range indices: array kinds clamp (as D1 chose), linear progressions extrapolate
> along the first or last segment; D1 is superseded. The class test of a progression of
> progressions is in §5.6 step 1 (the client tests for class 3810 first).
>
> *Answer (remainder):* nothing is left open. The class test agrees with the client (§5.6 step
> 1); 64-bit index arithmetic and debug logging are implementation details the spec does not
> constrain; D1 is superseded by §4.8.

> **Answered (2026-10-09; was OPEN, implementation choice):** A `Mod_Entry` without `Mod_Modified` is treated like
> `Mod_Modified` = 0 (empty). An entry with both an inline value and a progression, or with
> neither, is kept and marked (M1 says neither occurs) rather than rejected.
>
> **Answered in part (2026-10-08, merge):** §5.6 step 1 gives the client's checks: an entry
> with `Mod_Modified` = 0 or an empty struct is skipped; the reject conditions are listed
> there. An entry with both an inline value and a progression, or with neither, is not
> addressed and stays open.
>
> *Answer (remainder, Decision D6):* an entry with both a non-zero `Mod_Progression` and an
> inline value is evaluated from the progression (it is progression-driven by the rule of §5.2);
> an entry with neither produces no pair and is skipped, with a log line. Neither shape occurs
> in this install (§5.2), so the client's handling of them was not traced; D6 is Tessera's.

> **Answered (2026-10-09; was OPEN, implementation choice):** The D2 sandbox combines only values of the base value's own
> type (M2); a value of another type is skipped and logged. Integer arithmetic saturates. Bitwise
> operators on any-width bitfields work byte by byte at the base's width. `ArrayAdd` appends the
> elements and adds the counts. Operators with no Reported meaning (`Invalid`, `Difference`,
> `Divide`, `Random`) are skipped and logged.
>
> **Answered in part (2026-10-08, merge):** §5.3 now gives the client library's arithmetic per
> operator and type (C41), which differs from these choices: int and int64 `Add`, `Subtract` and
> `Multiply` wrap rather than saturate; `Or` on an any-width bitfield first widens the shorter
> operand with zero bits, so the result has the larger width; `Divide` is handled but allowed for
> no type by the load check (§5.3 valid-operator table), `Difference`, `Random`, `And` and `Xor`
> are not handled; an operator applied to a type the table does not list leaves the property
> unchanged. The combination order across entries and sources (D2) stays OPEN (§18 Q6).
>
> *Answer (remainder):* nothing is left in this note: the per-operator arithmetic is §5.3, which
> the implementation follows instead of these choices, and the combination order is tracked as
> Q6 alone (§5.3 callout).

> **Answered (2026-10-09; was OPEN, implementation choice):** Effect references (§6.3) are recognised by their ID field
> wherever a struct carries one (`EffectGenerator_EffectID`, `Skill_Effect`,
> `Skill_Toggle_Effect`, `Effect_StartupEffectID`, tried in this order), so the same struct kinds
> are found under any holder; a DID entry with one of these labels or `Effect_WSLEffect` outside
> such a struct is a bare reference. Zero values are kept and marked as placeholders.
>
> *Answer:* confirmed as Decision D6, with the three applied-effect ID fields of §6.5
> (`Effect_Aura_Applied_Effect`, `Effect_Area_Applied_Effect`, `Effect_ApplyOverTime_Applied_Effect`)
> added to the list. Zero values mean "none" (§3.4, C56).

> **Answered (2026-10-09; was OPEN, implementation choice):** The typed views require the signature properties of their
> family (§2.1) and report a missing one as an error; every other property is optional. The DPS
> and value tables (§10.2) offer base values by position only, because the index base is OPEN
> (Q12), and quality factors by key.
>
> **Answered in part (2026-10-08, merge):** the index base is closed (§18 Q12): §10.2
> "Lookups" (C48) states that element 0 of both base arrays belongs to level 1, with the
> out-of-range behaviour of each table. The signature-property choice stays open.
>
> *Answer (remainder, Decision D6):* confirmed. The signatures of §2.1 are present in every
> record of their family in this install (§2.1), so a missing one marks a malformed or foreign
> record.

---

<a id="s19"></a>
## 19. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Families, census, progressions, modifier entries, effects, skills, traits, class and control tables, enum mappers, reference graph, BIN results. |
| 2026-10-07 | Revision after review (status stays Review). Every disputed count re-measured over all records. (1) §5.5, M3, C12: ranges are disjoint, but the stored order is not guaranteed (100 groups not ascending, patterns given); readers sort by floor; added holders (all ranged groups are in non-combat objects), ends present, floor and ceiling ranges, group sizes. (2) §6.2, E1, C14: duration groups now go by stored value; the 722 + 1 records with `Effect_Instantaneous` false moved to the "permanent" and "interval" rows (12,561 and 5,250); stored false means not instantaneous; added the two duration-progression properties. (3) §4.4, §4.10, §4.12: the algorithm takes an outer and an optional inner index; class 3810 without an inner index gives none (D4); value at any key, including the last, is the stored value (D3); worked examples for both. (4) Q2, Q3, Q4, Q5, Q9, Q12 and Q13 are now `> **OPEN:**` callouts (§1, §4.4, §4.5, §4.6, §5.5, §9.1, §10.1, §13.3). (5) Provenance: legend of marks; rows C27–C33 for §4.9, §6.3, §6.5, §7.9, §8.6, §6.6/§10.2 and §11; Decision rows D1 (clamp, §4.8), D2 (sandbox order, §5.3, now including `ArrayAdd` last), D3, D4. (6) §10.2: all 8 class-1660 records have 701 elements, all 99 class-1688 records 601; quality structs stored in key order 5, 4, 2, 3, 1; `Item_ValueLookupTable` totals 150,622 values. (7) §6.6 `MaxDepth` histogram spelled out; §11 labels checked against the EMAPPER map (two mappers have none). Other corrections found while re-measuring: §6.3 `EffectGenerator_EffectID` targets 66 of 87 effect classes, not every class; struct counts, optional fields and holders completed (`Skill_EffectData` also in effects); §3.2 and §7.8 placeholder count is 97,268 in skills (the former 99,817 included 2,549 in effects); §6.5 `Mod_Array` holders by class (708 is the only holder outside 734's subtree), class 3124 properties corrected, `Effect_SkillProc_RequiredCombatResult` is a bitfield; §4.9 target classes filled in, all 50 class-3810 references are in class-3438 traits (P7); §7.9 counts and the induction (1,779) and channelling (401) objects; §8.6 list lengths and lookup through the WEENIECONTENT map. |
| 2026-10-07 | Second revision after re-review (status stays Review; text only, no section renumbered). (R1) §6.3: the `EffectGenerator_EffectID` sentence now states its population: 0 of the 54,832 struct values are 0; over all 55,825 occurrences (by enclosing struct and holder) 59 are 0, all in `Threshold_State_EffectStruct` structs of one class-1932 record; targets 66 effect classes either way; re-measured over every 0x78 and 0x79 set, and C28 widened. (R2) Provenance rows added for facts that had none: C34 (§3.4 integer census, re-measured over every integer-typed value), C35 (§5.4 modifier lists, with the LC reading marked Reported), C36 (§5.7), C37 (§13.1 function and body counts, re-measured); the sections now cite them. |
| 2026-10-08 | Revision (status Revised; awaiting review): OPEN closure from the client library's bytecode (new source BC, read with `wlib-bytecode.md`) and a second BIN pass on the natives it calls; every census re-run over all records. Closed: Q1 and Q2 (§4.4, §4.8, §4.10: array kinds clamp, linear progressions choose a half-open segment and extrapolate, the last key is computed from the last segment; Decisions D1 and D3 superseded), Q3 (§4.5: outer = evaluation level, inner = `Mod_ProgressionProgressionInnerIndex`, which point-based traits set to `Advancement_Level`; D4 superseded), Q5 (§5.5), Q7 (§6.2, client duration rule), Q8 (§6.4, compiled source flags), Q12 except hook damage (§10.1 rating curve, §10.2 lookups and formulas), and the §13.2 bytecode callout. Added: §5.3 arithmetic, identity values and valid operators per type; §5.4 combination and duplicate rule; §5.6 rewritten as the client's rule; §6.6 duration factors; §14 second-pass results; §16 four rows; invariants P8, M5–M7, R1, V1; rows C38–C50; C9, C11, C12, C14, C15, C21, C26, C35 and D1, D3, D4 updated. Remaining OPEN, made precise: Q4, Q6, Q9 (now also the rank-based trait spellcraft), Q10, Q11, Q12 (hook damage), Q13. |
| 2026-10-08 | Fixes for the review of 2026-10-08 (status stays Revised; no section renumbered; new subsection §10.2.1). (1) §6.2 step 3: without an interval source the interval is 0.0 at once, with no modifier list and no 0.5 s minimum; step 5 states that it raises any duration, 0 included, to 0.5 s when an equivalence class and a property source are present. (2) §6.2 step 4: the pulse-count modifier list's property-half source identified as `Effect_PulseCount_AdditiveModifiers` (C51, all 32,971 effects; property row added; Q14 recorded and closed). (3) §5.3: a third BIN pass (b6) read set, or and the int64 arithmetic in the value objects of int64, 64-bit and any-width bitfields; the table and the paragraph after it now give those cases; C41 states its scope. (4) §10.2: weapon damage moved to §10.2.1 and specified step by step from the class-256 function: the timing and variance tables (property and compiled halves), the action duration from category and `Combat_WeaponSpeed`, the DPS level, the `Combat_Damage_Mod` rule (`WeenieType` bits 0x01 and 0x80), the formula and its inputs (C52, census over all 23,204 weapons); C48 narrowed; open question Q15 and Decision D5 for several matching variance masks. (5) §4.2: `FloatProgression_ValueProperty` is used by the load check. (6) §5.6 step 1: represented property of text and DID arrays; no modifier entry names them (C40). (7) §6.4: class-755 bit table read from the code, with the bits never set (C46, V2). (8) §6.2: the override remark replaced by the separate "is permanent" query (C45). (9) Change log in date order. Invariants V2, E3, W1 added; BC and BIN rows extended. |
| 2026-10-08 | Re-review of the fixes (separate review session, not the author): passed; status moved to Accepted. No text change besides the header. |
| 2026-10-08 | Merge of the decoder branch (spec writer, dirty room). The implementer OPEN notes and census differences of 2026-10-07 are kept; those that the Accepted text of the 2026-10-08 revision already answers are marked with a pointer: Implementer notes: out-of-range indices (§4.8), skipped entries (§5.6), operator arithmetic (§5.3) and the table index base (§10.2, Q12), each answered in part. No fact changed; status unchanged. |
| 2026-10-09 | OPEN sweep (spec writer, dirty room; status Revised, waiting for review). New provenance key OBS3 (every set walked at every depth) and rows C53–C58, D6. Corrections: §4.1 float-array length 8 occurs once (C53); §4.6 sparse DID targets are traits of classes 1477 and 1483, not 3438 (C54). Added: §5.2 census depth and where nested `Mod_Array`s sit (C55); §6.3 `Effect_WSLEffect` zeros (C56); §6.4 grouping rule, counts without variance for both classes, full labels per stem (C57); §6.5, §8.1, §9.1, §9.2 field labels (C58). Implementer notes: all six closed (D6 confirms the choices, or a pointer to §4.8, §5.3, §5.6). Still OPEN: Q4, Q6, Q9, Q10, Q11, Q12 (hook damage), Q13, Q15 (server logic, native code or owner observation). |
