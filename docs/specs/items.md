<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Items: item objects, their property sets, lookup tables and auxiliary records

| | |
|---|---|
| **Status** | Revised (OPEN sweep of 2026-10-09, §16 last row; review 2026-10-09: changes requested, see the review record). Previously Accepted (revision of 2026-10-08: OPEN items closed from the client library and BIN; review fixes 2026-10-08) |
| **Last updated** | 2026-10-09 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted), not the author |
| **Review record** | Re-review 2026-10-07 (separate dirty-room session): **passed**, Accepted. Revision of 2026-10-08: reviewed 2026-10-08 (separate dirty-room session): **changes requested**; §13.11 repair example gives 161, not 162, under §6.3's single-precision arithmetic; status stays Revised. Re-review 2026-10-08 (separate dirty-room session, not the author): **passed**, Accepted. Full records in the review log (decision 0018). OPEN sweep 2026-10-09 (spec writer, dirty room): normative text of §2.1, §2.4, §3.3, §5.7, §6.3, §6.4, §7.2, §7.3, IT12 and IT13 changed (rows F45–F53); status Revised until a separate session reviews it. Review of the OPEN sweep 2026-10-09 (separate dirty-room session, not the author): **changes requested**; status stays Revised. Spot checks over all records with an independent probe on the repository decoders confirmed F45 (eight door labels 254 / 0, the 11 class-3924 labels, the 13 class-2097 labels in 58 records), the 151,224 / 6 result of rule 4, F46 (200 weapons and 41,881 apparel records with a non-null mirror of the list's length, 5 general items without), the F47–F49 dictionary types and mappers, the F49 counts (31 tiers, 602 outputs of weight 1, 3,279 weights 1–65), F50 (1603 and 1547 have one 0x70 and one 0x76 record) and F53's data side (44 = 38 + 6 and 6 PIDs, class 17). Blocking: (1) §6.4 and row F53 cite the examination builder as "class 269 function 1, inherited by 412"; by the function-ID rule of `wlib-bytecode.md` §6 it is class **412** function 1 (the body that calls the class-1202 getter), which class 269 inherits (412 is 269's parent); function 1 of class 269 is a different body that does not read the list. (2) §2.4 rule 4 step 1 calls the key (803) and multi-recipe-scroll (3663) signatures "all and exclusive" in §2.1, but `Key_CanBePlacedOnKeyRing` is in 9 of the 40 keys (the others are placed by step 3), and the 3663 label `Effect_Crafting_RecipeList` sits inside the usage effect data list (all 71, no other item), which §2.1 does not mark and step 1 does not say to search at depth; the stated 151,224 depends on that nested test. State both. Not blocking: row F52 follows F53; the 2026-10-09 change-log row precedes the 2026-10-08 merge row; §6.4 could say the examination data is a `UI_Examination_Tooltip_CharacteristicStruct` and that the list is not walked when the object's property copy is empty. |
| **Implements into** | `crates/wstate` / `crates/props` (item identification and typed accessors), a future gameplay-data crate (value, DPS, armour and durability evaluation; set and legacy tables), `apps/explorer` (item card), `crates/net` sandbox backend (inventory, vendors, loot) |
| **Companion specs** | [`wlib.md`](wlib.md) (class library, ancestor chains, §9 parent map); [`wstate.md`](wstate.md) (object-state records, §3–§8); [`property-system.md`](property-system.md) (dictionary §2, value encodings §4, property sets §5, the +0x09000000 rule §6.2, enum resolution §7, DID maps §8); [`string-table.md`](string-table.md) §9 (rendering StringInfo); [`icon-layers.md`](icon-layers.md) (item icon composition); [`appearance.md`](appearance.md) §4–§5 (worn and held visuals, the only part of items specified before this spec); [`ui-layout.md`](ui-layout.md) (tooltip layouts); [`combat-data.md`](combat-data.md) (progressions §4, `Mod_Array` §5, effects §6); [`economy.md`](economy.md) (loot §4, recipes §5, vendors and barter §6). This spec starts from decoded property sets and decoded object-state records. |

---

<a id="purpose"></a>
## Purpose

Items are the largest gameplay family in the data: 151,230 of the 383,332 game-object records
(prefix 0x70), 39 %. Inventories, loot tables, vendors, barter lists, crafting recipes and quest
rewards all point at them. This spec lets an implementer:

- recognise item records by their class-library ancestry and property signature, without
  hard-coding class indices (§2);
- know what the object-state record and the property set of an item contain, and which
  properties carry information versus which are inherited stamps, mirrored copies or values
  derived from shared tables (§3, §4);
- read the gameplay values a server needs (level, quality, value, stack size, binding,
  durability, weapon damage, armour, stat modifiers, on-use and on-equip effects, sockets,
  legendary data) and understand their encodings (§5);
- read the shared lookup and control tables that those values depend on, and recompute the
  derived values exactly as the data does (§6, §9);
- follow the auxiliary records that hang off items: item sets, relics, legacies and the
  directories that list them (§7);
- navigate the DID graph into and out of items (§8);
- build the explorer's item card (§10).

Item **visuals** (worn and held appearance) stay in `appearance.md`; item **icons** stay in
`icon-layers.md`. Effects, skills, traits, progressions and stat modifiers are specified in
[`combat-data.md`](combat-data.md); loot tables, recipes, vendors, barter and currency in
[`economy.md`](economy.md); quests and deeds in [`quests.md`](quests.md). This spec describes
only how items point at them. (Those three specs were written in parallel with this one and
are in Review; section references to them are to their 2026-10-07 versions.)

---

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| OBS | Observed in the user's install (Steam, client build 4906.0071.1794.4035) on 2026-10-06 and 2026-10-07 with throwaway probes in the dirty-room scratch area. The probes used the repository's own decoders (`tessera-dat`, `tessera-props`, `tessera-wstate`) as tools. **Every** object record was decoded: all 383,332 records with prefix 0x70 and their 383,332 property sets (0x79), all 1,616 stand-alone property sets (0x78), the property dictionary (0x34000000), the class library (0x56000000), all 735 enum mappers (0x23) and the DID maps 0x2800000E (WEENIECONTENT) and 0x28000001 (EMAPPER). Every count in this spec covers all records of the kind named, unless it says otherwise. |
| OBS-SB | Part of OBS: a count of how often each item property's PID occurs as a 4-byte little-endian constant inside the bytecode chunk (chunk type 1, `wlib.md` §4.1) of the script library. This is a statistical observation about the data file. It says nothing about what the code does with the value. |
| LC | LotroCompanion `lotro-tools` (item, item-set, value-table, DPS-table, weapon-speed and legendary loaders) and `lotro-core` (money and value-table classes), read for facts only. No code, comment or non-interoperability identifier was copied. |
| DS | `research/notes/data-semantics.md` §4 (WEENIECONTENT labels) and §12 (item property naming). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, file version read 2026-10-07), analysed headless in Ghidra on 2026-10-07 from a private copy of the shared project. **Question:** does the client's native code apply the value, DPS, damage, armour or set rules of §9 itself (so that they are client behaviour and not only relations in the data), and does it read item properties by PID? **Method:** a search of all instructions for immediate operands equal to 38 item-related PIDs, a search of the string data for property names, and decompilation of the functions that use the hits. **Why M1–M3 were not enough:** the data shows that stored values agree with the tables (§9), but not who computes them; no published source says; the running client does not show it. **Result:** no immediate operand equals the PIDs of `Item_Value`, `Item_ValueLookupTable`, `Combat_DamageVariance`, `Combat_Damage`, `Item_Armor_Value`, `Item_Armor_Value_Lookup_Table`, `Item_BaseActionDuration`, `LinearInterpolatingProgression_Array`, `Set_ActiveCount` or `Set_MemberList`, and no property name occurs as a string. The other PIDs do occur, but every decompiled use treats the number as a UI-element identifier, which shares the 0x1000xxxx number space. So BIN does **not** settle the question; the rules of §9 are stated as data relations, and who applies them stays OPEN (§14). Nothing from network, login, launcher or anti-tamper code was read. (Superseded for the client side by BC below: the formulas live in the client library's bytecode, not in native code.) |
| BC | The client library's bytecode (record 0x56000000, chunk type 1, `wlib.md` §4.1), read on 2026-10-08 in the dirty room with a throwaway disassembler that follows [`wlib-bytecode.md`](wlib-bytecode.md) (Accepted). This is data in the user's install (method M1). Bodies read, by class: the level-scaling functions (class 1228 and its subclass 3215); the item base functions of class 791 (value, armour, level offset, getters); the weapon functions of class 256 that it calls; the static item effect functions (class 1046); the set classes 1817, 1824, 1827–1829, 1833, 1837, 4006; the socket classes 3952, 3977, 3978, 4214–4216; the wear classes 1208, 369 and the wear control 1672; the binding class 2779; the vendor functions of class 462; the disenchant examination class 4158; the indestructible examination class 1068; the player level-scaling class 3697 and the scaled-level helpers of class 3802; the craft preview function of class 263; the inventory control class 1202. Plus a search of the whole code block for 32-bit literals equal to item PIDs. A literal equal to a PID is not always a property read: validation functions return 32-bit error codes that can coincide with PIDs (for example the literals equal to `ItemWearControl_SkillGateValue` and `ItemWearControl_DurabilityArray` are such return codes); every hit used here was classified by reading its body. Facts from this source describe what the **client's** library computes; the server-side library is not in the archives (`combat-data.md` §13.1). |
| BIN2 | **M4 (decision 0011), second pass.** Same build and executable, 2026-10-08, a private copy of the shared Ghidra project opened headless without analysis, decompiling only registered native functions called by the BC bodies (located with the native-registration pairing of `wlib-bytecode.md`'s BIN row). **Question:** what these natives do: the property inquiries by PID (integer, boolean, float) used by the script accessors, in particular whether an absent property yields the dictionary default; the random-number function; the object test used by the level-scaling code; the ordered container used to average set member levels; the integer clamp. **Why M1–M3 were not enough:** the bytecode passes values to natives by function ID only; their behaviour is in the executable alone; no published source describes them; no running client is available to this session. **Results:** package 169 functions 30, 31 and 33 look the PID up in the object's own property table (after an override table that is empty unless a debug flag is set) and return "not found" otherwise, leaving the caller's default in place; no dictionary-default step was found. Package 174 function 2 returns low + *r* × (high − low) for its two float arguments, *r* from the client's uniform generator (equal arguments return that value). Package 214 function 41 returns bit 0x20000000 of the object's flag word (the same bit selects between the two lookup paths of the inquiry functions). Package 1974 is a binary min-heap of (unsigned 32-bit key, value) pairs: function 2 inserts, 3 reads the smallest key and its value, 4 removes it, 1 tests for empty. Package 2574 function 2 clamps its third argument to [second, first] (signed 32-bit). The integer minimum and maximum (package 122 functions 24, 25) and the `Advancement_Level` / `Apparent_Level` readers (package 3703 functions 1, 2) are as `combat-data.md`'s BIN row (b4) records them. Nothing from network, login, launcher or anti-tamper code was read. |
| OBS3 | Observed in the user's install on 2026-10-09 (OPEN sweep, spec writer). The install's archives are unchanged since 2026-10-04 (file times), so a dump of **every** 0x70 record with its 0x79 property set and of all 0x78 sets, made on 2026-10-07 with the repository's own decoders, stands for the install; it was walked at every nesting depth (struct fields, array elements, arrays inside arrays), and the property dictionary (0x34000000) was read for types, mappers and defaults. Every count marked OBS3 covers all records of the kind named. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| F1 | Item records are the 151,230 object records whose root class descends from class 1734 here; 15 classes have records (§2.1) | OBS (all 383,332 headers + `wlib.md` parent map) | Confirmed 2026-10-07 |
| F2 | Ancestor chain above the item base, and the sibling carry-all classes 4178/4179 (41 records) under the same parent (§2.2) | OBS | Confirmed |
| F3 | `WeenieType` low byte 0x81 marks 151,257 records: 151,216 items and the 41 carry-alls; 14 items have low byte 0x01 (§2.3) | OBS (all 383,332 sets) | Confirmed. Bit structure: F31. |
| F4 | The class indices of the item kinds match those named by LC's current item loader | LC; OBS | Reported (stability), Confirmed (use here) |
| F5 | Object-state shape of item records: root attributes, the per-item scaffold object, links and available-property lists (§3.2) | OBS (151,230 records) | Confirmed layout; scaffold meaning F32 |
| F6 | For all 41,881 records of class 795, the object state holds a map keyed by species + sex whose values equal the `Item_WornAppearanceMapList` entries (§3.3) | OBS | Confirmed |
| F7 | Property-set sizes: 41 to 117 top-level properties, mean 85.5; 527 distinct top-level names (§4.1) | OBS | Confirmed |
| F8 | Sixteen properties are present with the same value in all 195,930 records under ancestor 1733 and in no other record (§4.2) | OBS (all 383,332 sets) | Confirmed |
| F9 | Ten mirrored copies (`Forward_Cached_*`, `ForwardSource_*`, `*_Cached`, `*_Shadow`) and `Item_CurStructurePoints` equal their source in every item (§4.3) | OBS | Confirmed |
| F10 | None of the eight stamps and only two of the twelve copy labels occur in the script bytecode; 322 of the 507 other labels do (§4.6) | OBS-SB | Confirmed (count only) |
| F11 | `Item_Value` = value-table base value at `Item_Level` × quality factor, single-precision product, rounded half up; 145,518 of 145,518 records with a table (145,477 items, 41 carry-alls); null table ⇒ value 0 (5,087 of 5,087) (§9.2) | OBS; LC (formula) | Confirmed |
| F12 | `Combat_BaseDPS` = DPS-table base at (`ItemAdvancement_CombatDPSLevel` if present, else `Item_Level`) × quality factor as a single-precision product, exactly equal in 23,204 of 23,204 (a double-precision product is exact in only 1,115) (§9.3) | OBS; LC | Confirmed |
| F13 | `Combat_Damage` = `Combat_BaseDPS` × `Item_BaseActionDuration` × 2 ÷ (2 − `Combat_DamageVariance`), within 1.2 × 10⁻⁷ relative, 23,204 of 23,204 (§9.4) | OBS; LC (min/max reading) | Confirmed relation; min/max reading Reported |
| F14 | `Item_Armor_Value` = the armour progression evaluated at `Item_Level` (`combat-data.md` §4.10), truncated, 60,880 of 60,880 items that have both (§9.5) | OBS | Confirmed |
| F15 | `Item_MaxStructurePoints` and `Item_DurabilityValue` equal the wear-control tables at their enum keys (87,020 and 90,932 of all) (§9.6) | OBS | Confirmed |
| F16 | Weapon timing values equal the weapon-speed tables at (equipment category, speed) for 23,204 of 23,204 weapons (§9.7) | OBS; LC | Confirmed |
| F17 | Cooldown and decay enums used by items all have rows in the cooldown and decay control maps (§9.8) | OBS | Confirmed |
| F18 | Every non-null DID reference in the item property sets (3,607,800) resolves to an existing record; distribution by prefix and property (§8.1) | OBS (every archive's directory) | Confirmed |
| F19 | Inbound references to items: 129,560 distinct items are named by other records; sources by property (§8.2) | OBS (all 383,332 sets) | Confirmed |
| F20 | Every `Mod_Array` entry carries `Mod_Modified` and `Mod_Op`, and either `Mod_Progression` (289,349 without an unrelated field) or a field whose PID equals `Mod_Modified` (21,136); 12 more carry `Mod_Progression` and one unrelated field, so 289,349 + 12 = the 289,361 `Mod_Progression` entries of §5.8 and §8.1 | OBS (310,497 entries) | Confirmed |
| F21 | The item level governs an item's stat modifiers (progression index and floor/ceiling test) | LC; `combat-data.md` §5.5 | Reported (LC). The client library attaches the modifiers with spellcraft −1 ("not given"), so the level is supplied at application time (F32, §5.8 OPEN) |
| F22 | Effect references: struct layout of the four `EffectGenerator_*` lists and their target classes (§5.9) | OBS | Confirmed layout; spellcraft −1 = not given (`combat-data.md` §1); floor/ceiling: F35 |
| F23 | Behaviour objects named by `Usage_Action`, `Usage_Permission`, `Usage_TakePermission`, `CSM_ActionHandler` (§5.10) | OBS; WEENIECONTENT labels | Confirmed |
| F24 | Item sets: layout of class 1824, directory 0x70009869 (label `GameSetDirectory`), membership through `Set_MemberList` or through `Item_PropertySet` (§7.1) | OBS; LC | Confirmed layout; client-side tier and level rules F41; server activation OPEN |
| F25 | Relic, transmutation, runic-list, legacy, title and imbued-legacy records and how items reach them (§7.2–§7.4) | OBS | Confirmed layout; semantics mostly Reported/OPEN |
| F26 | Control records named in WEENIECONTENT that items depend on (§6) | OBS; DS (labels) | Confirmed |
| F27 | Binding combinations of `Inventory_BindOnAcquire` / `_BindToAccount` / `_BindOnEquip` (§5.4); their reading as binding kinds | OBS; LC; BC (F40) | Combinations Confirmed; binding target Confirmed (bytecode, F40); trigger timing Reported |
| F28 | Money unit of `Item_Value` (copper; 100 copper = 1 silver; 1,000 silver = 1 gold) | LC | Reported |
| F29 | Enum mappers used by item properties and their EMAPPER labels (§11) | OBS | Confirmed |
| F30 | The native client code does not settle §9 (BIN result above) | BIN | BIN only (negative result) |
| F31 | `WeenieType` bit structure: low 16 bits = major type (mapper mask 0xFFFF), higher bits = subtype; major types nest by bit inclusion (§2.3) | OBS (mappers 0x230000C9 and base 0x230001AB; the 195,930 sets that carry the property) | Confirmed 2026-10-08 |
| F32 | The class-701 object of an item is its compiled static effect list: own unranged effects, one `PropertyModEffect` element per `Mod_Array` (spellcraft −1), category default effects (§3.2) | OBS (all 151,230 item records, object state against property set); BC (class 1046) | Confirmed 2026-10-08 |
| F33 | Level-scaling ("munging") rule: which items scale, the two cases, cap, progression, legendary cap, random bonus, crafting preview offset (§5.2) | BC (classes 1228, 3215, 263, 862); BIN2 (random, object flag); BIN (`combat-data.md` b4: minimum); OBS (label combinations, all items) | Confirmed (bytecode + BIN) 2026-10-08 |
| F34 | The client library recomputes value, armour, DPS, damage and timing for a scaled level; value = f32(base × factor + 0.5) truncated; armour index = level + distribution offset (§9.2, §9.5) | BC (classes 791, 1688, 256, 1202); OBS (145,518 values: both rounding forms agree, no value reaches 2²³) | Confirmed (bytecode) 2026-10-08 |
| F35 | Effect structs with a spellcraft floor or ceiling are level-dependent: left out of the static list, added at run time when Item_Level + distribution offset lies in [floor, ceiling] (§5.9) | BC (class 1046); OBS (all 90 such structs absent from every compiled list) | Confirmed (bytecode + data) 2026-10-08 |
| F36 | Socket rules: lock by `Item_Socket_Unlock_ILevel`, type match, level checks per socket kind, gem uniqueness, rune level-up (§5.11) | BC (classes 3952, 3977, 3978, 4214–4216, 791); OBS (all socketables) | Confirmed (bytecode) 2026-10-08 |
| F37 | Repair cost per point and total, sale value with wear-state factor, which items wear; the skill-gate table is not read by the client library; operation order and precision of both (single-precision factors, structure-point difference converted as unsigned, integer sale product converted to single precision), the −1.0 `Repair_CostModifier` default and the −1 structure-point getter defaults (§6.3) | BC (classes 1208, 369, 1672, 791) | Confirmed (bytecode) 2026-10-08 |
| F38 | Vendor rules of the client library: conditions that make an item unsellable, buy-quantity check, stackability (§5.3, §5.4) | BC (classes 462, 368, 791, 2535) | Confirmed (bytecode) 2026-10-08 |
| F39 | `Item_Disenchant_Value` is read only with a component result (0 shown as 1); −1 and 0 occur only without one (§5.3) | BC (class 4158); OBS (all 62,162 items with the label) | Confirmed 2026-10-08 |
| F40 | Binding checks: bindable, bound, binding target (account if `Inventory_BindToAccount`, else character) (§5.4) | BC (class 2779) | Confirmed (bytecode) 2026-10-08 |
| F41 | Set rules of the client library: tier per exact count (count − 2), counts 2 … member count, set level (Set_Level, average of the highest member levels, or the apparent level), `Set_MaxPlayerLevel` gate, member-less sets read two actor properties (§7.1) | BC (classes 1817, 1824, 1828, 1829, 1837, 4006); BIN2 (heap); OBS (all 2,217 item sets) | Confirmed (bytecode + BIN) 2026-10-08 |
| F42 | `Item_LevelScaling_OnLevelForPlayersBelowLevel` in the client's scaled-level function (§5.2) | BC (classes 3697, 3802); BIN2 (clamp); BIN (`combat-data.md` b4: level readers, minimum) | Confirmed (bytecode + BIN) 2026-10-08 |
| F43 | Absent properties: each script read site supplies its own default; 1,045 of 1,104 constant-default sites use the dictionary default; the native lookup does not substitute it (§14) | BC (all accessor call sites); BIN2 | Confirmed (bytecode + BIN) 2026-10-08 |
| F44 | `WeenieType` is not read by the client library; `Item_ImplementType` is compared with the mapper's `Instrument` value; `ExternalAppearanceID`, `Item_MaterialType`, `UI_Examination_Tooltip_Compare_Type`, `Inventory_UseOnAcquire`, `Inventory_QuestItem`, `Quest_IsQuestToken`, `Effect_IsEnchantable`, `Set_LevelRequired`, `Set_MinActiveMembers`, the `Combat_*Stance` labels, the `Item_PILI_*` labels and the legacy roll tables are not read by it (§5, §7.1, §7.3) | BC (literal search, bodies 278/2, 1953/49) | Confirmed (bytecode, absence) 2026-10-08 |
| F45 | Doors (1722) and interactive objects (2097) share `WeenieType` `Door`; eight labels present in all 254 doors and in none of the 167 interactive objects separate them; one class-2097 record has `WeenieType` `Item` and no label that general items lack; the 11 `SPM_*` / `Usage_ModPropertyUsageData_*` / `Usage_PropertyCondition_Array` labels are in both class-3924 records and in no other item (§2.1, §2.4) | OBS3 (all 151,230 items) | Confirmed 2026-10-09 |
| F46 | The 200 weapons with `Item_WornAppearanceMapList` carry the same object-state worn map as class 795 (root attribute 0x0905B9D8 non-null in all 200; equal to the list); the 5 general items with the list have none (§3.3) | OBS3 (object-state root of all 42,086 items that have the list); implementation census 2026-10-07 (map equality, all 200) | Confirmed 2026-10-09 |
| F47 | Value types of the requirement labels of §5.7: `Usage_RequiredRaceEntry` bare enum (0x2300000B); `Usage_RequiredTrait_Struct` fields; `Usage_RequiredVirtueRank` int; craft tier and mastery enums (0x23000063); quest-requirement struct fields and types; the two barring `_Array` forms | OBS3 (dictionary; every occurrence at any depth) | Confirmed 2026-10-09 |
| F48 | Entry labels and value types of the five wear-control arrays (§6.3) and of `ItemDecayControl_DurationMapList` (§6.4) | OBS3 | Confirmed 2026-10-09 |
| F49 | Labels and value types of the records of classes 2134, 2139, 2338, 4015 (§7.3) and 3138 (§7.2); `Runic_Tier` enum (0x230002A0), `ItemAdvancement_Mod_Weight` int | OBS3 (all records of each class) | Confirmed 2026-10-09 |
| F50 | Of the 24 targets of IT13, the root classes of two (1603, 1547 here) also own a 0x76 singleton record (`wstate.md` §3); IT13 and §5.10 count 0x70 records | OBS3; `wstate.md` OBS (the 11 records with prefix 0x76) | Confirmed 2026-10-09 |
| F51 | §2.4 step 1 tie rule and the kind order of rule 4 | Decision (Tessera), not a fact about the original | Decision 2026-10-09 |
| F53 | `InventoryControl` compiles the examination list (38 PIDs) followed by the merge list (6) into one class-17 list that the examination-tooltip builder reads, copying each listed property the object holds; the merge list alone is a second class-17 list that no class-library code reads (§6.4) | OBS3 (the object-state record 0x70000230 decoded with `tessera-wstate`, compared with its property set); BC (class 1202 getter, class 269 function 1) | Confirmed 2026-10-09 (meaning of native package 166 function 10 as a property lookup inferred from its use) |
| F52 | Getters of `Inventory_CanBeContained`, `Item_MithrilCost` and `SpellcraftCalculator` located with their callers; the callers pass the values to natives not specified in `wlib-bytecode.md` §7, so their effect stays OPEN (§14) | BC (re-read 2026-10-09) | Confirmed (location only) 2026-10-09 |

---

<a id="s1"></a>
## 1. Concepts and terms

| Term | Meaning in this spec |
|---|---|
| **Item record** | An object record (prefix 0x70, `wstate.md`) whose root class (offset 4) descends from the **item base class** *I* (§2). Its static properties are the property set at DID + 0x09000000 (prefix 0x79, `property-system.md` §6.2). |
| **Item kind** | The root class of an item record. Class names are not in the data (`wlib.md` §13); this spec uses its own descriptive labels. |
| **Property label** | The dictionary name of a PID (`property-system.md` §2). Labels are interoperability identifiers: implementations look PIDs up by label at run time and never hard-code PID numbers. The PIDs printed here are for test convenience only. |
| **Stamp** | A property present in every record of a class subtree with one value (§4.2, §4.4). |
| **Copy** | A property whose value equals another property of the same set in every record (§4.3). |
| **Derived value** | A stored value that equals a function of other stored values and shared tables (§4.5, §9). |
| **Authored value** | Any property that is none of the above: it carries per-item information (§4.6). |
| **Behaviour object** | A 0x70 record with an empty or near-empty property set, named by an item property, whose root class's script code implements a behaviour (§5.10). |
| **Control record** | A 0x70 record reached through a WEENIECONTENT label (`property-system.md` §8.1) that holds a game-wide table (§6). |

Throughout, "class *n*" is a class index observed in this install. Indices are build-specific
(`wlib.md` §1); §2.4 gives the data-driven way to find them.

---

<a id="s2"></a>
## 2. Identifying item records

<a id="s2-1"></a>
### 2.1 The item kinds (Confirmed, all 151,230 item records)

The item base class here is **1734**. It has 27 descendant classes; 15 of them are root classes
of records, the others (and 1734 itself) have none: 791, 793, 800, 811, 812, 813, 814, 816,
1721, 1735, 1736, 1737.

| Root class | Records | Path below 1734 | `WeenieType` (label: records) | `Item_Class` values | Distinct property labels | Signature (properties present in every record of the class, exclusive to it among item classes, or characteristic) | Descriptive label (this spec's) |
|---|---|---|---|---|---|---|---|
| 796 | 52,753 | direct | Item 52,719; PopupImage 17; Entity 10; Hotspot 4; Money 3 | 87 | 426 | Heterogeneous; 218 labels occur only in this class (sockets for essences and traceries, crafting trinkets, instruments, decorations, hotspots) | General items |
| 795 | 41,881 | 791 | Clothing 41,871; Item 10 | 16 | 136 | `Item_WornAppearanceMapList` (all 41,881), `Item_DurabilityEnum`, `Item_ClothingColorSecondary` (exclusive) | Worn apparel (armour and cosmetic clothing) |
| 799 | 23,204 | 816 → 1737 | Weapon 23,204 | 15 | 157 | `Combat_DPS_LUT`, `Combat_BaseDPS`, `Combat_Damage`, `Combat_DamageVariance`, `Combat_DamageType`, `Combat_WeaponSpeed` (all, exclusive) | Weapons |
| 802 | 19,720 | direct | Jewelry 19,720 | 3 | 126 | `Inventory_DefaultSlot` in a jewellery slot | Jewellery |
| 805 | 6,055 | 796 | Item 6,055 | 134 | 83 | `RecipeItem_Recipe`, `Usage_BarringCraftRecipe` (all, exclusive) | Recipe scrolls |
| 2814 | 3,634 | direct | PackageItem 3,634 | 22 | 120 | `PackageItem_TrophyListTemplate` (all, exclusive) and 18 other `PackageItem_*` / web-store labels | Packages (items with contents) |
| 797 | 2,902 | 813 → 1735 | Shield 2,902 | 1 | 119 | `Item_Class` = the shield key in all | Shields |
| 798 | 301 | 814 → 1736 | Letter 301 | 3 | 74 | `TextItem_Title`, `TextItem_PrefabContentList` (all, exclusive) | Readable text items |
| 1722 | 254 | 793 | Door 254 | 1 | 68 | `Usage_PlayersCanOpen`, `Usage_PlayersCanClose` (exclusive, 13 and 15 records); `CSM_ActionHandler`, `MapNote_Enabled`, `MapNote_Linkable`, `Physics_CollisionReports`, `Physics_EtherealToType`, `Physics_EtherealityType`, `Physics_NoPathmaps`, `Physics_PlacementEtherealToType` (all 254; none in class 2097, F45) | In-world doors |
| 815 | 228 | direct | Waypoint 228 | 1 | 78 | 13 exclusive labels (travel destination, layer lists, tracking) | In-world travel points |
| 2097 | 167 | direct | Door 166; Item 1 | 2 | 94 | No label is present in every record and exclusive. 13 labels occur only here, in 58 of the records between them (`BigBattle_Interactive_Aim_Horizontal_Max`, `…_Aim_Horizontal_Min`, `…_Built`, `…_Burning`, `…_Crank_Max`, `…_Disable_Mod`, `…_Disarm_Mod`, `…_MaxRange`, `…_Upgrade_Damage_Modifier`, `DistantBattleUnit_Activator_Spawn_Transfer_Properties`, `Missile_InitialOffset`, `MultiUsage_ActivityCollection`, `State@Entity.m_iCount`). The 166 `Door` records lack all eight door labels of row 1722 (F45) | In-world interactive objects |
| 3663 | 71 | 796 | Item 71 | 1 | 73 | A usage effect whose data list carries `Effect_Crafting_RecipeList` | Multi-recipe scrolls |
| 803 | 40 | direct | Key 40 | 1 | 79 | `Key_CanBePlacedOnKeyRing` (exclusive) | Keys |
| 804 | 18 | direct | Milestone 18 | 1 | 55 | `Milestone_*`, `Travel_TravelFlags` (exclusive) | In-world milestones |
| 3924 | 2 | 796 | Item 2 | 1 | 82 | `SPM_Max_Error`, `SPM_Min_Error`, `SPM_PropertyMod`, `SPM_SKU`, `SPM_Target`, `SPM_Target_Max`, `SPM_Target_Min`, `Usage_ModPropertyUsageData_MaxValues`, `Usage_ModPropertyUsageData_MinValues`, `Usage_ModPropertyUsageData_ModProperties`, `Usage_PropertyCondition_Array` (all 11 in both records and in no other item, F45) | Property-modifying tokens |

`WeenieType` labels come from enum mapper 0x230000C9 (EMAPPER label `WeenieType`, §11). Four
kinds (1722, 815, 2097, 804) are fixtures placed in the world: they have `Item_Class` =
`NonInventory`, no `Item_Value` and no `Item_Level` (§5.2).

Byte totals (inflated payloads): 0x70 records 164,216,619 bytes (largest 2,856), 0x79 sets
205,932,378 bytes (largest 2,655).

<a id="s2-2"></a>
### 2.2 Ancestor chain (Confirmed)

The ancestor chain of the item base class, from the class upwards, is
1734 → 801 → 1733 → 787 → 1726 → 488 → 223 → 89 → 485 → 484.

- **801** has exactly two record-bearing subtrees: 1734 (items) and **4178** (39 records) with
  its child **4179** (2 records). The 41 records of 4178/4179 carry the same item signature
  (`Item_Class`, `Item_Value`, `Item_ValueLookupTable`, `Item_Level`, `Inventory_MaxQuantity`,
  `Inventory_CompatibleSlot`, icon layers) and, per LC, are carry-all containers
  (`WeenieType` label `BagOfSingletons`; their sets hold `BoS_*` properties naming the items
  they accept). A server should treat them as items; the item rules of this spec apply to them
  unchanged (they are included in F11). They are **not** in the 151,230 count.
- **1733** is the root of every placeable world entity: its subtree holds 195,930 records
  (items, creatures, NPCs, containers, world objects). The entity stamps of §4.2 come from it.
- Class **794** (1,722 records, under 1732 → 695 → 1733) and classes 1989, 1113, 1210, 1724
  carry `Item_Class` in 2,585 records but are not items: they are world containers, resource
  nodes and creatures. Do not use `Item_Class` alone to recognise items.

<a id="s2-3"></a>
### 2.3 Property signature (Confirmed)

Over all 383,332 property sets:

| Test | Items (151,230) | Other records |
|---|---|---|
| Has `Item_Class` | 151,230 | 2,585 (§2.2) |
| Has `Icon_Layer_BackgroundDID` | 151,230 | 747 (relics 706, carry-alls 41) |
| Has `Item_Value` | 150,564 | 41 (carry-alls) |
| Has `Item_ValueLookupTable` | 150,564 | 58 |
| Has `Inventory_MaxQuantity` | 148,895 | 54 |
| `WeenieType` low byte = 0x81 | 151,216 | 41 (carry-alls) |
| `WeenieType` low byte = 0x01 | 14 (10 `Entity`, 4 `Hotspot`, all class 796) | 6,007 |

**Bit structure (Confirmed, F31; settles the former OPEN).** The mapper's base list
(0x230001AB) has an entry `MajorTypeMask` = 0xFFFF. The low 16 bits of a value are its **major
type**; the bits above select a **subtype** within that major type (for example `Weapon` =
0x20081 = 2 × 65,536 + 0x81, `Clothing` = 3 × 65,536 + 0x81, `Jewelry` = 32 × 65,536 + 0x81).
The major types nest by bit inclusion:

| Major type (value) | Contains the bits of |
|---|---|
| `Entity` (0x1) | — |
| `Container` (0x3) | `Entity` |
| `Inventory` (0x7) | `Container` |
| `Agent` (0xF) | `Inventory` |
| `Player` (0x1F) | `Agent` |
| `Admin` (0x3F) | `Player` |
| `NPC` (0x4F) | `Agent` |
| `Item` (0x81) | `Entity` |
| `GameplayContainer` (0x103) | `Container` |

Over the 195,930 sets that carry `WeenieType`, the low 16 bits of every value are one of these
nine major types except in one record (0x200032, `AIRemoteDetector`, low bits 0x32). The low
byte 0x81 of the table above is therefore the `Item` major type, and the 14 items with low byte
0x01 have major type `Entity`. The client library's bytecode never reads `WeenieType` (F44);
how native code tests it was not traced. An implementation compares the major type (value AND
the mask read from the mapper) or the label, never the full value.

<a id="s2-4"></a>
### 2.4 Rule for implementers (normative)

1. Find the classes at run time, without hard-coding 1734 or 801:
   - take every DID listed by vendor, barter and treasure-group tables (`Vendor_Item` elements
     of `Vendor_ItemList`, `Barter_Item`, `TreasureGroupProfile_ItemTable_Item`; §8.2) and read
     each record's root class;
   - let *P* be the deepest class that is an ancestor-or-self of all of those root classes
     (`wlib.md` §9 gives the parent map). Here *P* = 801: all 308,799 references fall in its
     subtree (`Vendor_Item` 91,274, all from class 1724; `Barter_Item` 100,761, all from class
     1943; `TreasureGroupProfile_ItemTable_Item` 116,764, of which 116,706 from class 1235 and
     58 from class 2372). No reference is null or names a missing record;
   - let *I* be the child of *P* whose subtree holds the most of those references. Here
     *I* = 1734 with 308,778 (all vendor and treasure references and 100,740 barter
     references); the other 21 are barter references that name carry-alls (4178).
     A reference whose root class is *P* itself counts for no child (none occurs: class 801
     has no records). **Decision (Tessera, F51):** if two or more children share the largest
     count (not observed), take the one whose subtree holds the record named by the
     WEENIECONTENT label `BankFillItem`; if none of them does, take the lowest class index,
     and log the tie;
   - a reader may instead use 1734 and 801 as fast hints after checking that the record named
     by the WEENIECONTENT label `BankFillItem` (0x70000EF3 here, class 799) descends from 1734.
2. A record is an **item** if its root class is *I* or descends from *I*.
3. A record is **item-like** (treated as an item by inventory code) if its root class descends
   from *P*. This adds the carry-alls of §2.2.
4. The **kind** is the root class. Code that needs a kind uses the signatures of §2.1 or the
   `WeenieType` label, never the class index alone. **Decision (Tessera, F51)**, the order in
   which a reader without class indices decides the kind of an item from its property set:
   1. the signatures marked "all" and "exclusive" in §2.1: weapon (799), recipe scroll (805),
      multi-recipe scroll (3663), package (2814), text item (798), property-modifying token
      (3924: any of its 11 labels), key (803);
   2. `WeenieType` `Door`: an in-world door (1722) when the set has `CSM_ActionHandler` (or any
      of the other seven door labels of §2.1), otherwise an in-world interactive object (2097);
   3. the `WeenieType` label: `Weapon`, `Clothing`, `Jewelry`, `Shield`, `PackageItem`,
      `Letter`, `Key`, `Waypoint`, `Milestone` give the matching kind; other labels give none;
   4. `Item_WornAppearanceMapList` present: worn apparel;
   5. otherwise a general item (796).
   With this order 151,224 of the 151,230 item records of this install get their root class as
   kind (OBS3, every item). The six others: the class-2097 record with `WeenieType` `Item`
   (0x7001B1C1 here) holds no label that the general items lack and reads as a general item
   (F45), and the 5 general items with `Item_WornAppearanceMapList` (§3.3) read as worn
   apparel. A reader that needs the exact kind reads the root class with the class discovery
   of step 1 and `wlib.md` §9.

> **Decision (Tessera):** the explorer and the sandbox server classify items by rule 2–3. A
> record whose root class descends from *I* but whose property set lacks `Item_Class` would be
> reported as malformed; none exists.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** Step 1 does not say what happens when two children of
> *P* hold the same number of references. `crates/formats/src/gameplay/items.rs`
> (`ItemClasses::discover`) picks the lower class index and counts references whose class is
> *P* itself for no child. Please confirm or state a rule.
>
> *Answer:* step 1 now states the rule (Decision F51): references of class *P* count for no
> child; a tie goes to the child that holds `BankFillItem`, else to the lowest index. No tie
> occurs in this install.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** Rule 4 cannot separate two kinds of §2.1. The in-world
> interactive objects (2097) carry `WeenieType` `Door` like the doors (1722), and their
> signature ("13 exclusive labels (`BigBattle_Interactive_*` extras, activity collections)")
> names no label present in every record; the property-modifying tokens (3924) carry `Item`
> like the general items, and their "11 exclusive `SPM_*` / `Usage_ModPropertyUsageData_*`
> labels" are not listed. Please name, for each of the two, labels present in every record
> of the class and absent from the others. Until then `ItemView::kind` reads them as Door
> and General. Engineering choice recorded here: the kind is decided by, in order, the
> signatures that §2.1 marks "all" or "exclusive" (weapon, recipe scroll, multi-recipe
> scroll, package, text item, door, key), then the `WeenieType` label (`Weapon`, `Clothing`,
> `Jewelry`, `Shield`, `PackageItem`, `Letter`, `Door`, `Key`, `Waypoint`, `Milestone`; the
> other labels give no kind), then `Item_WornAppearanceMapList`, else General.
>
> *Answer:* §2.1 rows 1722, 2097 and 3924 now list the labels (F45). Class 3924: 11 labels in
> both records and in no other item. Class 2097 has no label in every record; it is separated
> from the doors by absence: eight labels are in all 254 doors and in none of the 167
> interactive objects. Rule 4 now gives the order (Decision F51, the implementer's order with
> the token and door steps added) and the six records it cannot place.

---

<a id="s3"></a>
## 3. The record pair

<a id="s3-1"></a>
### 3.1 Where the values are (Confirmed)

Each item is two records with the same serial: the object-state record (0x70) and the
property set (0x79). All 151,230 pairs exist and decode exactly. **Every gameplay value in this
spec is in the property set.** The object-state record holds a script-object scaffold whose
attributes are almost all null references, empty containers or zero; its only data that
duplicates a property is the worn-appearance map of §3.3.

<a id="s3-2"></a>
### 3.2 Object-state shape (Confirmed)

Decoded with `wstate.md`. Attributes are named by their name hash (`wlib.md` §8.2).

| Kind | Root attributes (hash: type) | Items in the bundle | Links (`wstate.md` §6) |
|---|---|---|---|
| 796, 802, 797, 805, 798, 815, 2097, 3663, 804, 3924 | 0x035B97AF, 0x04343517, 0x0BCFA314 (references) | 2 (root + scaffold) up to 55 (796), 34 (802), 21 (797); 8 in every 805 and 3663 record | One link of type 8 (limit 1) in most. 13,874 general items add type 13 (limit 99, both directions) and type 12 (limit 250, both directions), and 11,145 more add a type 0x10000019 link as well; recipe scrolls have type 8 and two type-13 links; travel points, doors and interactive objects have a type 0x1000000B link; milestones have none |
| 795, 799 | the three above + 0x0905B9D8 (reference) | 18–38 (795), 3–38 (799) | One link, type 8 |
| 2814 | nine attributes (five integers, four references) | 3–7 | Five links (types 8, 13, 13, 12, 12) |
| 1722 | the three + 0x0516AFF9 | 2 | Six to eight links |
| 803 | the three + 0x0F9CB3A4 | 2–6 | One link |

- Attribute 0x04343517 references an object of **class 701** in all 151,230 records (13
  attributes; most null). In records with 5 or more items, that object leads through a
  `multimap_u32` (class 13) and a `refmap_u32` (class 35) to an object of class 700 and
  further objects of classes 732, 1342, 1626, 1339, 699 and native classes 166 (empty property
  set), 182, 39 and 199.
- Every item record has a non-empty available-property list (`wstate.md` §7) except 182
  general items; the list depends on the kind (one to 66 distinct lists per class) and only
  22 % of its entries are labels that the item's property set has.
- No item record contains a DID item. Imports (`wstate.md` §3.2) occur only in class 795 (7 in
  39,542 records, 8 in 2,339: appearance records, among them the wardrobe records of §3.3) and
  in 200 weapons (7 each: the worn battle gauntlets).

**What the scaffold is (Confirmed, F32; settles the former OPEN).** The class-701 object is the
item's **compiled static effect list**, the same structure `interaction.md` §4.3 specifies for
hotspot templates: a key multimap, an element map and one class-700 element per effect, each
element naming an effect DID (attribute 0x08CF9154) and a spellcraft (0x02712104), optionally
with a class-699 data object. Checked against the property set of all 151,230 items:

- 113,349 items have elements, 37,881 have none; the 37,881 have no effect list and no
  `Mod_Array`.
- Every element is one of: (a) an effect of the item's own four `EffectGenerator_*` lists
  whose struct has no spellcraft floor or ceiling; every such effect of every item is present;
  (b) the effect named by the WEENIECONTENT label `PropertyModEffect` (0x70000253 here, class
  734), exactly once in each of the 92,396 items that have `Mod_Array`, always with spellcraft
  −1 and a class-699 data object; (c) a default effect that `InventoryControl` gives for the
  item's equipment category (§6.4).
- None of the 90 effect structs that carry a floor or ceiling (14 in equipper lists, 76 in
  target lists) is compiled; they are added at run time for the item's level (§5.9).

The client library builds the same list (BC, class 1046): equipper effects, the
`PropertyModEffect` element (spellcraft −1.0) and the category's default equipper effects go
under multimap key 16, target effects and the category's default target effects under key 32
(hotspot effects use key 64, `interaction.md` §4.3); when the item has `Combat_DamageType`, each
element's data receives it as `EffectGenerator_EffectData_ImplementDamageType`. A server can
therefore rebuild the list from the property set and `InventoryControl`; an offline reader may
skip the object-state record of an item.

<a id="s3-3"></a>
### 3.3 Worn-appearance mirror (Confirmed)

In all 41,881 records of class 795, root attribute 0x0905B9D8 references a `refmap_u32`
(class 35). Its keys are **species key | sex key** (for example 23 | 0x1000 = 4,119) and each
value is an object of class 2065 with two integers: the `Item_AppearanceKey` value and the
wardrobe record DID (0x20…). For every one of the 41,881 records this map equals the
`Item_WornAppearanceMapList` of the property set (same keys, same pairs). The 200 weapons
that have the list carry the same mirror, equal to their list in all 200 (F46; corrects the
earlier text, which said they had none); the 5 general items with the list have no mirror
(the attribute is absent or null). `appearance.md` §4 reads the
property list; readers need not read the mirror.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The sentence "The 205 other items with that list (200
> weapons, 5 general items) have no such map" is not reproduced. Running
> `crates/formats/tests/gameplay_items_real_data.rs` (it decodes the object-state record of
> every item that has `Item_WornAppearanceMapList`) finds root attribute 0x0905B9D8 resolving
> to a non-empty `refmap_u32` in all 200 weapons with the list as well, and in each of them
> the map equals the list (keys and the two integers); only the 5 general items have none.
> Is the sentence wrong, or does it describe something other than a non-null reference to a
> non-empty map? Should IT12 cover the weapons too? The test asserts IT12 for every record
> that has a mirror and reports the classes; it does not assert which classes those are.
>
> *Answer:* the sentence was wrong; the implementation's census is right (F46). The paragraph
> above is corrected and IT12 now covers every item whose object state holds the map (the
> 41,881 apparel records and the 200 weapons).

---

<a id="s4"></a>
## 4. What an item's property set contains

<a id="s4-1"></a>
### 4.1 Census (Confirmed)

| Measure | Value |
|---|---|
| Top-level properties per item | min 41, max 117, mean 85.5, median 84 |
| Total top-level values | 12,925,452 |
| Distinct top-level labels | 527 |
| Largest groups by label prefix (labels / values) | `Item` 75 / 2,285,749; `Usage` 103 / 1,953,330; `Inventory` 17 / 1,140,841; `UI` 9 / 924,512; `BigBattle` 15 / 907,497; `MC` 5 / 756,150; `Forward` 5 / 756,150; `Icon` 6 / 549,015; `Entity` 7 / 303,676; `ForwardSource` 3 / 302,462; `Combat` 12 / 238,062 |

Every property is classified below into exactly one of five categories. Per item, on average:
8.0 entity stamps (§4.2), 10.6 copies (§4.3), 3.2 derived values (§4.5), 19.7 class-uniform
values (§4.4) and **44.0 authored values** (§4.6).

| Kind | Mean properties | Stamps | Copies | Derived | Class-uniform | Authored |
|---|---|---|---|---|---|---|
| 796 | 76.1 | 8 | 10.1 | 1.1 | 4 | 52.9 |
| 795 | 89.4 | 8 | 10.9 | 3.9 | 22 | 44.6 |
| 799 | 102.1 | 8 | 11.0 | 7.0 | 34 | 42.2 |
| 802 | 86.5 | 8 | 11.0 | 4.0 | 28 | 35.5 |
| 805 | 76.4 | 8 | 10 | 1 | 40 | 17.4 |
| 2814 | 82.2 | 8 | 10 | 1 | 34 | 29.2 |
| 797 | 92.8 | 8 | 11.0 | 4.0 | 38 | 31.8 |
| 798 | 65.0 | 8 | 10 | 1 | 31 | 15.0 |
| 1722 | 55.2 | 8 | 10 | 0 | 22 | 15.2 |
| 815 | 47.5 | 8 | 10 | 0 | 19 | 10.5 |
| 2097 | 45.0 | 8 | 10 | 0.0 | 9 | 18.0 |
| 3663 | 73 | 8 | 10 | 1 | 46 | 8 |
| 803 | 65.7 | 8 | 10 | 1 | 27 | 19.6 |
| 804 | 54.2 | 8 | 10 | 0 | 27 | 9.2 |
| 3924 | 82 | 8 | 10 | 1 | 55 | 8 |

"Class-uniform" in a small class is partly coincidence (3924 has two records); the column is an
observation, not a schema.

<a id="s4-2"></a>
### 4.2 Entity stamps (Confirmed)

These eight labels, and their eight copies of §4.3, are present in **all 195,930 records under
ancestor 1733** (items, creatures, NPCs and other world entities) with one value each, and in
no record outside that subtree. Each value equals the dictionary default of its label
(`property-system.md` §2.2).

| Label | Type | Value everywhere |
|---|---|---|
| `MC_Trait_Loremaster_BredForWar_AttackSpeed`, `…_BleedDamage`, `…_CritChance`, `…_FlankChance`, `…_Threat` | float | 0.0 |
| `BigBattle_OfficerBannerCanHeal` | boolean | false |
| `Boss_Zone_1` | int | 0 |
| `Skirmish_RoleCombo` | enum | key 40 |

They carry **no item information**. A reader may drop them; a writer producing item data for
Tessera's own server does not need them. None of these sixteen PIDs (eight stamps, eight
copies) occurs in the script bytecode (F10).

Four more labels are nearly the same: `BigBattle_Interactive_Impulse_Min`, `…_Max`,
`…_Min_Base`, `…_Max_Base` are 0.0 in all items except one class-2097 record (10.0 and 40.0).

<a id="s4-3"></a>
### 4.3 Copies (Confirmed)

Each label in the first column equals the label in the second column in **every** item that
has it (all counts are of items):

| Copy | Source | Items |
|---|---|---|
| `Forward_Cached_MC_Trait_Loremaster_BredForWar_*` (5 labels) | the five `MC_Trait_*` stamps | 151,230 each |
| `ForwardSource_Skirmish_RoleCombo` | `Skirmish_RoleCombo` | 151,230 |
| `ForwardSource_Boss_Zone_1` | `Boss_Zone_1` | 151,230 |
| `BigBattle_OfficerBannerCanHeal_Cached` | `BigBattle_OfficerBannerCanHeal` | 151,230 |
| `Usage_Action_Shadow` | `Usage_Action` | 151,230 |
| `Usage_MinUsageDist_Shadow` | `Usage_MinUsageDist` | 151,230 |
| `Item_CurStructurePoints` | `Item_MaxStructurePoints` | 87,020 |

`ForwardSource_Movement_SpeedMultiplier` occurs in two items without its source. Readers keep
copies when round-tripping data and ignore them otherwise; on a live item, the current
structure points start equal to the maximum (§5.5).

<a id="s4-4"></a>
### 4.4 Class-uniform values (Confirmed per class; not a schema)

A property is class-uniform for kind *K* when every record of *K* has it with one value.
Examples that matter to gameplay (each holds in all records of the kind):

| Kind | Label | Value |
|---|---|---|
| 799 (23,204) | `Combat_DamageVariance` | 0.25 |
| 799 | `Combat_WeaponSpeed` | key 1 (`Normal`, mapper 0x23000066) |
| 799 | `Item_BaseActionDuration` | 1.0 |
| 799 | `Item_BaseAnimDurationMultiplierMod` | −0.1 |
| 799 | `Combat_WeaponHarmless` | false |
| 795, 799, 797, 802 | `Inventory_MaxQuantity` | 1 |
| 805 | `Usage_DestroyOnUse` | true |
| 797 | `Item_Class` | the shield key (33) |

These values are game rules expressed as data. **They must be read from each item, not
hard-coded**, because a later client build may change them; for weapons they are also
derived from shared tables (§9.7).

<a id="s4-5"></a>
### 4.5 Derived values (Confirmed relations)

| Stored label | Items | Derived from | § |
|---|---|---|---|
| `Item_Value` | 150,564 | `Item_ValueLookupTable`, `Item_Level`, `Item_Quality` | §9.2 |
| `Combat_BaseDPS` | 23,204 | `Combat_DPS_LUT`, level, `Item_Quality` | §9.3 |
| `Combat_Damage` | 23,204 | `Combat_BaseDPS`, `Item_BaseActionDuration`, `Combat_DamageVariance` | §9.4 |
| `Item_Armor_Value` | 60,880 of 64,322 | `Item_Armor_Value_Lookup_Table`, `Item_Level` | §9.5 |
| `Item_MaxStructurePoints` | 87,020 | `Item_MaxStructurePointsEnum` + wear control | §9.6 |
| `Item_DurabilityValue` | 90,932 | `Item_DurabilityEnum` + wear control | §9.6 |
| `Item_BaseActionDuration`, `Item_BaseAnimDurationMultiplierMod` | 23,204 | equipment category, `Combat_WeaponSpeed` + speed tables | §9.7 |

The stored value and the recomputed value agree in every record. A server may use either; a
Tessera writer that changes an input must recompute the derived value.

The other 3,442 `Item_Armor_Value` values (3,412 of class 795, 30 of class 797) have no lookup
table and are all 0.

<a id="s4-6"></a>
### 4.6 Authored values

Everything else carries per-item information: 507 of the 527 labels occur in at least one item
outside §4.2–§4.3. 322 of those 507 PIDs occur as constants in the script bytecode (OBS-SB), for
example `Item_Level` (18 occurrences), `Combat_BaseDPS` (18), `Item_DurabilityValue` (13),
`Item_Quality` (10), `Item_Value` (9), `Combat_DPS_LUT` (8), `Mod_Array` (5),
`Inventory_MaxQuantity` (4). The sixteen labels of §4.2 occur zero times. This supports §4.2 but is a
count, not a statement of what the scripts do.

The property groups of §5 list the authored labels that matter for gameplay. Labels not listed
(UI tweaks, physics and render flags, housing, hobbies, monster play, web-store flags) are
decoded generically (`property-system.md`) and shown raw by the explorer.

---

<a id="s5"></a>
## 5. Property groups

Columns: **Items** = number of item property sets with the label at top level; **Type** =
property type code name (`property-system.md` §3). "Meaning" is Confirmed only where a relation
in §9 or a census proves it; otherwise it is marked Reported (LC), "label" (the plain reading of
the interoperability label, not confirmed by any observation) or OPEN.

<a id="s5-1"></a>
### 5.1 Identity and display

| Label | Type | Items | Observed values | Meaning |
|---|---|---|---|---|
| `Name` | StringInfo | 151,230 | 134,004 distinct | Item name (render per `string-table.md` §9) |
| `PluralName` | StringInfo | 48,923 | | Plural form for stacks (Reported LC) |
| `Description` | StringInfo | 150,592 | | Description text |
| `Item_Class` | enum (0x23000036) | 151,230 | 260 of the mapper's 279 keys | Item category; the mapper has display texts (278) |
| `Item_Quality` | enum (0x23000067) | 150,564 | keys 0–5: 1,839 / 8,238 / 45,577 / 29,227 / 33,424 / 32,259 | Quality; indexes the quality factors of §9.2–§9.3 (Confirmed) |
| `Item_EquipmentCategory` | bitfield 64 (0x2300004C) | 91,190 | one bit set in 91,154, zero in 36; 39 distinct values | Equipment category; bit *i* = key *i* + 1 (`property-system.md` §7.3) |
| `WeenieType` | enum (0x230000C9) | 151,230 | 15 values (§2.1) | Object behaviour kind |
| `Auction_CategoryType` | enum (0x230001D1) | 109,148 | 46 | Auction category (label) |
| `Item_Client_Sort_Type` | enum (0x23000317) | 28,546 | 7 | Sort group (label); `InventoryControl_SmartSortInfo_Array` lists property IDs per group (§6.4) |
| `Item_MaterialType` | enum (0x230000F3) | 87,982 | 13 | Material (label); not read by the client library (F44); use OPEN (§14) |
| `Item_ImplementType` | enum (0x23000004) | 29,220 | 16 | Kind of wielded implement: the mapper names single-bit kinds (for example `Sword`, `Bow`, `Orb`, `Instrument` = 0x20000) and combination masks; the client library compares the value with the `Instrument` bit (Confirmed, BC, F44) |
| `Icon_Layer_*` | DID → 0x41 | see `icon-layers.md` §2.2 | | Icon layers |
| `PhysObj` | DID → 0x47 | 151,230 | 10,295 distinct | Ground / held look (`appearance.md` §5) |
| `ExternalAppearanceID` | DID → 0x20 | 138,462 | 0x20000004 in 138,428, 0x200020F1 in 34 | Not read by the client library (F44); use OPEN (§14) |
| `Item_WornAppearanceMapList` | array | 42,086 | | Worn look (`appearance.md` §4) |
| `UI_Examination_Tooltip_0Click_Layout`, `…_1Click_Layout`, `…_Compare_Layout` | DID → 0x22 | 149,695 / 148,979 / 107,479 | 0x22000000 in all but 11, 11 and 20 | Tooltip layout record (`ui-layout.md`) |
| `UI_Examination_Tooltip_0Click_Element`, `…_1Click_Element`, `…_Compare_Element` | enum (0x230000D1) | 149,695 / 148,979 / 107,479 | 25 / 22 / 10 | Element of that layout used as the tooltip |
| `UI_Examination_Tooltip_Compare_Type` | enum | 110,672 | 6 | Not read by the client library (F44); use OPEN (§14) |
| `Entity_AdditionalScriptTables` | array of `ScriptTable_DBScriptTable` (DID → 0x07) | 149,924 | 1,163 distinct lists | Script tables attached to the item's entity (`script-table.md`) |

<a id="s5-2"></a>
### 5.2 Level, scaling and distribution

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Item_Level` | int | 150,478 | 1–580, 325 distinct; 1 in 29,639 | Item level; indexes value, DPS and armour tables (Confirmed, §9). Absent in 752 items: all 254 doors, 228 travel points, 166 interactive objects, 18 milestones, 86 general items. Dictionary default 0. |
| `Usage_MinLevel` | int | 100,735 | 2–160 | Minimum character level to use (Reported LC). Equal to `Item_Level` in 40,703 items, lower in 51,520, higher in 8,512. Dictionary default 1. |
| `Usage_MaxLevel` | int | 15,109 | 44 distinct | Maximum character level to use (Reported LC) |
| `Item_DistributionType` | enum (0x2300028C) | 83,031 | Loot 74,544; Quest 3,696; Craft 1,552; Instance 1,261; Raid 746; … (13 keys) | Source category; `InventoryControl_DistributionToLevelOffsetList` maps each key to a level offset (0 for all 15 keys here; §6.4). The client library adds that offset to `Item_Level` for the armour lookup (§9.5) and for the level window of ranged effects (§5.9); absent label = offset 0 (Confirmed, BC, F34) |
| `Item_iLevel_Crafting_Offset` | int | 30,250 | 6 distinct; 0 in 60 % | Levels added for the crafting preview: for a scaling item with an offset > 0 the client library previews the crafted result at `Item_Level` + offset (Confirmed, BC, F33; rule below) |
| `Item_iLevel_Bonus_Limit`, `Item_iLevel_Bonus_Chance` | int, float | 27,983 each | limit 0–5, chance 0–0.5 | Cap and per-step chance of a random item-level bonus (Confirmed, BC, F33; rule below) |
| `ItemMunging_MinMungeLevel`, `ItemMunging_MaxMungeLevel` | int | 29,733 / 8,740 | | Level-scaling window; 1 and 1,000 when absent (Confirmed, BC, F33) |
| `ItemMunging_ItemLevelOverrideProgression` | DID → progression (class 1389) | 20,798 | 54 distinct | Progression that maps the driving level to the item level (Confirmed, BC, F33) |
| `ItemMunging_ItemLevelOverrideProperty` | property ID | 32,486 | `Item_Level` 16,517; `Advancement_Level` 9,204; `ze_skirmish_level` 6,765 | The driving property; a non-zero value makes the item scale (Confirmed, BC, F33) |
| `Item_LevelScaling_OnLevelForPlayersBelowLevel` | int | 126 | 6 distinct | Level cap used when the character is level-scaled (Confirmed, BC, F42; rule below) |
| `SpellcraftCalculator` | DID (class 3177) | 56 | 6 distinct | Read by the client library (classes 3177, 1494); its effect was not traced (OPEN, §14) |

Combinations of the munging labels over all items: none (110,855); only the override property
(9,321); min + progression + property (`ze_skirmish_level` 6,063, `Advancement_Level` 4,798);
min + property (4,595); min + progression (4,539); and nine rarer combinations.

**Level-scaling rule (Confirmed, client library, F33; settles the former OPEN).** The client
library's level-scaling functions compute a scaled ("munged") copy of an item's level-dependent
properties for a requested level. Whether the server uses the same rule is not in the data
(`combat-data.md` §13.1).

*Which items scale.* An item scales when `ItemMunging_ItemLevelOverrideProperty` is present and
not 0, or when `Item_LevelUpgradeTier` is greater than 0 (no item has the latter here). That is
32,486 items. The 7,889 items that carry window or progression labels without an override
property never scale; for them the labels have no effect in the client library.

*Inputs.* The item; a requested level *L*; an addend *A* (0 at every call site in the client
library); an output property set *W* that receives the scaled values. *min* =
`ItemMunging_MinMungeLevel` (1 when absent), *max* = `ItemMunging_MaxMungeLevel` (1,000 when
absent).

*Case 1: the override property is `Item_Level`* (16,517 items; *L* is an item level).

1. Clamp *L* to [*min*, *max*].
2. Apply the random bonus (below).
3. Write the result to *W* as `Item_Level`. If the item has `Usage_MinLevel` greater than 0,
   write that value to *W* as `Usage_MinLevel`. *A* is not used.

*Case 2: any other override property* (`Advancement_Level` 9,204 items, `ze_skirmish_level`
6,765; *L* is the value of that property for the case at hand).

1. If the item carries `Item_Level_Calculated` = true, the level is *L* unchanged: the flag is
   removed from the item, *L* is written to *W* as `Item_Level`, and the rule ends.
2. *x* = the smaller of *L* and the player level cap. The cap is read from the world setting
   `WE_Player_Level_Cap`, or, in a mode the client selects in native code (not traced), from
   `XPGlobals_AbsoluteMaxLevel` of the advancement globals; it is 120 when the value cannot be
   read.
3. If *x* > *max*, *x* = *max*. If then *x* < *min*, the rule fails and *W* is not written.
4. Write *x* to *W* as `Usage_MinLevel`.
5. If the item has `ItemMunging_ItemLevelOverrideProgression`, *x* = that progression evaluated
   at *x* (`combat-data.md` §4). The progression must represent `Item_Level` and yield an
   integer; otherwise *x* is kept (the client logs an error).
6. If *W* already holds `Item_Reforgable` = true (the property is read from the output set, not
   from the item; callers that pass an empty *W* never trigger this step) and the world setting
   `WE_LI2_Legendary_Item_iLevel_Cap` can be read and is less than *x*, *x* = that cap.
7. If the item object has bit 0x20000000 of its native flag word set (BIN2; what kind of object
   that marks was not traced), the bonus is not rolled now: when `Item_iLevel_Bonus_Chance` is
   greater than 0, *W* receives `Item_Level_Bonus_Pending` = true. Otherwise apply the random
   bonus to *x*.
8. *x* = *x* + *A*. Write `Item_Level_Calculated` = true and `Item_Level` = *x* to *W*.

*Random bonus.* Only when the item has both `Item_iLevel_Bonus_Chance` *c* and
`Item_iLevel_Bonus_Limit` *n* with *n* > 0: repeat at most *n* times: draw *r* uniformly between
0 and 1 (BIN2); if *r* ≤ *c*, add 1 to the level and repeat, otherwise stop. The bonus is *k*
levels with probability *c*^*k* × (1 − *c*) for *k* < *n*, and *n* levels with probability
*c*^*n*.

*After the level.* The client library then writes into *W*, at the new level, the item value
(§9.2), the armour value (§9.5) and, for weapons, DPS, damage and timing (`combat-data.md`
§10.2, §9.7); when an item's level changes it removes the level-ranged effects added for the
old level and adds those for the new one (§5.9).

*Callers in the client library.* Loot and instance creation (with *L* from the caller), the
pending-loot list of the inventory, a path that takes *L* from the value of the item's override
property in a supplied property collection, and the crafting preview, which uses *L* =
`Item_Level` + `Item_iLevel_Crafting_Offset` for a scaling item whose offset is greater than 0.

**Scaled characters (Confirmed, client library, F42).** When a character's `Apparent_Level`
differs from its `Advancement_Level` (the character is level-scaled), the client library's
scaled-level function uses, for an item with `Item_LevelScaling_OnLevelForPlayersBelowLevel`
*v* > 0: *y* = the smaller of *v* and `Advancement_Level`; the scaled driving level is *y* +
(`Apparent_Level` − `Advancement_Level`), clamped to [1, player level cap]; the item level
follows from it through the item's override progression when the item has one. Without the
label (or with *v* ≤ 0) the function starts from a level lookup that this spec does not
specify. When the two levels are equal the item is not scaled.

Tessera's offline viewer shows the stored `Item_Level` and the munging fields raw; a viewer that
previews a level runs the rule above with the random bonus disabled.

<a id="s5-3"></a>
### 5.3 Economy

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Item_Value` | int | 150,564 | 4,278 distinct; 0 in 14,873 | Base value in copper coins (unit Reported, F28); derived (§9.2) |
| `Item_ValueLookupTable` | DID → class 1688 | 150,564 | 100 distinct; null in 5,087 | Value table (§6.1) |
| `Barter_IsBarterItem` | boolean | 41,888 | true 5,366 | When true the client library's vendor code treats the item as not sellable to a vendor (Confirmed, BC, F38; list of conditions below) |
| `Item_ForceNotSellable` | boolean | 3,933 | true in all | Not sellable to a vendor (Confirmed, BC, F38) |
| `Item_Disenchant_Value` | int | 62,162 | −1 (25,668), 0 (10,149), positive (26,345) | Quantity shown with the disenchanting component result. The client library reads it only when `Item_Disenchant_Component_Result` is set, and shows 0 as 1 (default 1 when absent). The value is positive in exactly the 26,345 items with a component result; −1 and 0 occur only in items without one (the 992 with a trophy list store −1), where the client library never reads them (Confirmed, BC + OBS, F39) |
| `Item_Disenchant_Component_Result` | DID → item | 26,345 | 45 distinct | An item (class 796 or 2814): the disenchanting result (label) |
| `Item_Disenchant_Trophy_List` | DID → class 1237 (loot list) | 992 | 26 | A loot list: the disenchanting result (label) |
| `Item_MithrilCost` | int | 308 | 8 distinct | Read by a getter of class 3649 in the client library; its use was not traced (OPEN, §14) |
| `Item_WearState` | enum (0x230000FB) | 87,020 | key 2 (`Pristine`) in all | Wear state; the wear control maps it to the sale-value factor (Confirmed, BC, §6.3) |

**Vendor rules of the client library (Confirmed, BC, F38).** The client library treats an item
as not sellable to a vendor when any of these holds: `Item_ForceNotSellable` is true; its sale
value (§6.3) is 0; `Usage_IsQuestItem` is true; `Inventory_Indestructible` is true;
`Barter_IsBarterItem` is true; `ItemAdvancement_Item` is true; `Item_IsPurchasedWebStoreItem` is
true. Whether the server applies the same list is not in the data.

<a id="s5-4"></a>
### 5.4 Inventory, slots and binding

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Inventory_MaxQuantity` | int | 148,895 | 1 (114,461), 50 (18,924), 100, 10, 0 (2,979), 25, 500, 5, … up to 999,999,999; 38 distinct | Stack size limit (Reported LC). In the client library an item is stackable when the value, compared unsigned, is greater than 1, and a vendor purchase quantity *q* is accepted when 1 ≤ *q* ≤ the value; 0 has no special meaning there, so a 0 item is not stackable and no quantity passes the purchase check. 2,964 of the 2,979 zeros are packages (class 2814, `Item_Class` key 27). Absent: the stack check reads 0; the entity accessor reads `InventoryControl_MaxQuantity` instead (Confirmed, BC, F38) |
| `Inventory_Unique` | boolean | 5,067 | true in all | One per inventory (Reported LC) |
| `Inventory_UniqueEquipped` | boolean | 15,417 | true 12,629 | Read by the client library only for socketables: a gem with the flag may be socketed only once per DID and once per uniqueness channel across the character's equipped items (Confirmed, BC, F36; §5.11) |
| `Item_UniquenessChannel` | enum (0x2300043B) | 15,416 | 363 distinct | Group within which gem uniqueness applies; 0 = none (Confirmed, BC, F36; §5.11) |
| `Inventory_CompatibleSlot` | bitfield 32 (0x230000EE) | 148,895 | 27 distinct | Slots the item may occupy (`appearance.md` §4.1, §5.2) |
| `Inventory_DefaultSlot` | bitfield 32 | 128,084 | 24 distinct; `Backpack` 36,894; equipment slot + `Equipment` bit in 91,154 | Slot it goes to by default |
| `Inventory_PrecludedSlot` | bitfield 32 | 128,078 | `Equipment` 81,026; none 40,151; `Weapon_Secondary`+`Equipment` 6,901 | Slots blocked while equipped (Reported LC) |
| `Inventory_CanBeContained` | boolean | 149,447 | true 148,235 | Read by container functions of the client library (classes 368/2804, 1534, 1541); its effect was not traced (OPEN, §14) |
| `Inventory_BindOnAcquire`, `Inventory_BindToAccount`, `Inventory_BindOnEquip` | boolean | 148,891 / 81,821 / 148,295 | table below | Binding: triggers and target (target Confirmed, BC, F40; trigger timing Reported LC) |
| `Inventory_UseOnAcquire` | boolean | 2,230 | true in all | Not read by the client library (F44); server behaviour (OPEN, §14) |
| `Inventory_Indestructible` | boolean | 278 | true in all | Adds the indestructible line to the examination tooltip; the vendor code refuses the item (Confirmed, BC, F38) |
| `Inventory_QuestItem`, `Usage_IsQuestItem`, `Quest_IsQuestToken` | boolean | 743 / 10,995 / 5,112 | | Quest-item flags. `Usage_IsQuestItem` true: the vendor code refuses the item (Confirmed, BC, F38); its other uses and the other two flags are not specified here |
| `Inventory_ParentingLocation`, `Inventory_PaperdollParentingLocation`, `Inventory_ParentingOrientation` | enum | 4,734 / 3,794 / 26,164 | | Held-item placement (`appearance.md` §5.3) |

Binding combinations (acquire, account, equip; "–" = label absent), all 151,230 items:

| BindOnAcquire | BindToAccount | BindOnEquip | Items | Reading (Reported LC) |
|---|---|---|---|---|
| true | true | false or – | 37,971 | Bound to the account when acquired |
| true | false or – | false or – | 33,098 | Bound to the character when acquired |
| false | false or – | true | 46,521 | Bound to the character when equipped |
| false | true | true | 30 | Bound to the account when equipped (target Confirmed, F40) |
| false or – | false or – | false or – | 33,610 | Not bound |

Dictionary defaults of all three are false.

**Binding checks of the client library (Confirmed, BC, F40).** An item is *bindable* when any of
`Inventory_BindOnAcquire`, `Inventory_BindOnEquip` or `Inventory_BindOnRaid` is true. It is
*bound* when `Inventory_BoundToID` (64-bit) is not 0. A bound, bindable item belongs to a
character when `Inventory_BoundToID` equals the character's instance ID if
`Inventory_BindToAccount` is false, or the character's account ID if it is true; the client
refuses a bound item whose ID does not match. So `Inventory_BindToAccount` selects the binding
*target* (account instead of character) for whichever trigger the item has, which gives the
readings of the table above. When the binding happens (on acquiring or on equipping) is set by
the server, which writes `Inventory_BoundToID`; the client library only checks it.

<a id="s5-5"></a>
### 5.5 Durability and wear

| Label | Type | Items | Observed | Meaning |
|---|---|---|---|---|
| `Item_DurabilityEnum` | enum (0x230000FA) | 90,932 | keys 3 (78,417), 4 (8,551), 6 (3,912), 1 (34), 2 (18) | Sturdiness class |
| `Item_DurabilityValue` | float | 90,932 | 0.75 / 0.8 / 1.0 / 0.85 / 0.0 for those keys | Derived (§9.6). The client library treats an item as subject to wear only when this value is present and not 1.0 (the 3,912 items of key 6, `Indestructible`, store 1.0) (Confirmed, BC, F37) |
| `Item_MaxStructurePointsEnum` | enum (0x230000F9) | 87,020 | 48 distinct | Structure-points class |
| `Item_MaxStructurePoints` | int | 87,020 | 12 distinct (28–100) | Derived (§9.6) |
| `Item_CurStructurePoints` | int | 87,020 | = max | Copy (§4.3) |
| `Item_RepairCostPerPointProgression` | DID → class 1385 (float progression) | 87,020 | 8 distinct | Repair cost per structure point, evaluated at `Item_Level` (Confirmed, BC, F37; §6.3) |

Durability labels occur on all of 795, 797, 799, on 19,520 of 802 and on 3,425 of 796.

<a id="s5-6"></a>
### 5.6 Weapons (kind 799; all 23,204 have every label in this table except where noted)

| Label | Type | Observed | Meaning |
|---|---|---|---|
| `Combat_DPS_LUT` | DID → class 1660 | 6 distinct | DPS table (§6.2) |
| `Combat_BaseDPS` | float | 801 distinct | Damage per second; derived (§9.3) |
| `Combat_Damage` | float | 801 distinct | Maximum damage per hit; derived (§9.4) |
| `Combat_DamageVariance` | float | 0.25 in all | Spread: minimum = maximum × (1 − variance) (Reported LC) |
| `Combat_DamageType` | enum (0x23000028) | 9 keys, all powers of two | Damage type |
| `Combat_WeaponSpeed` | enum (0x23000066) | key 1 in all | Speed class; with the equipment category selects §9.7 |
| `Item_BaseActionDuration`, `Item_BaseAnimDurationMultiplierMod` | float | 1.0 and −0.1 in all | Attack duration in seconds (Reported LC) and animation-speed modifier; derived (§9.7) |
| `Combat_SingleWeaponStance`, `Combat_DualWieldStance`, `Combat_WithShieldStance` | enum (0x23000092) | 7 / 7 / 6 | Combat stance per wield mode (label); not read by the client library (F44); use OPEN (§14) |
| `Combat_WeaponSlayerGenus`, `Combat_WeaponSlayerAddMod` | bitfield 32, float | 3,011 items | Bonus against creature genera (Reported LC) |
| `ItemAdvancement_CombatDPSLevel` | int | 3,766 items | Level used for the DPS lookup instead of `Item_Level` (Confirmed, §9.3) |
| `EffectGenerator_TargetEffectList` | array | 3,084 items | Effects applied to the struck target (§5.9) |

`InventoryControl_DefaultEquipperEffectList` and `…_DefaultTargetEffectList` (§6.4) add effects
by equipment category to **every** weapon or shield of that category; they are not in the item.

<a id="s5-7"></a>
### 5.7 Armour

| Label | Type | Items | Meaning |
|---|---|---|---|
| `Item_Armor_Value` | int | 64,322 (classes 795, 797, 796) | Armour; derived where a table exists (§9.5), else 0 |
| `Item_Armor_Value_Lookup_Table` | DID → class 3918 (linear-interpolating progression) | 60,916 | Armour progression; 126 distinct. 36 jewellery items have the table but no armour value; the table gives 0 for them. |

The armour stat may also appear inside `Mod_Array` (§5.8); LC reports three shields with both.

<a id="s5-8"></a>
### 5.8 Stat modifiers (`Mod_Array`)

`Mod_Array` (92,396 items; also on 20,106 non-item records such as effects, traits and relics)
is an array of `Mod_Entry` structs. Census over all 310,497 entries in items:

| Field | Type | Entries | Meaning |
|---|---|---|---|
| `Mod_Modified` | property ID | 310,497 | The modified property (a stat) |
| `Mod_Op` | enum (0x23000046) | 310,497 | Operation: key 7 in 308,948, 5 in 724, 6 in 672, 3 in 108, 8 in 45. The mapper labels the keys (for example 7 = add, 5 = set, 6 = subtract, 8 = multiply, 3 = bitwise or). |
| `Mod_Progression` | DID → progression (class 3918 in 287,637, class 1389 in 1,724) | 289,361 | Value as a function of level |
| *the field whose PID equals `Mod_Modified`* | the type of that property | 21,136 | Constant value |
| `Mod_ProgressionFloor`, `Mod_ProgressionCeiling` | int | 19,466 / 19,401 | Level range for the entry (Reported LC) |
| `Mod_DescriptionOverride` | StringInfo | 3,596 | Tooltip text replacing the stat line (Reported LC) |

Every entry has either `Mod_Progression` or the self-named constant field (12 entries also carry
one further field whose PID is not `Mod_Modified`; `combat-data.md` §5.2 treats these as
anomalies to ignore). Readers find the constant by looking up the PID named in
`Mod_Modified` among the struct's fields. Operators, level ranges (inclusive at both ends) and
the evaluation of one entry are specified in `combat-data.md` §5.3–§5.6.

**How an item's modifiers are attached (Confirmed, F32).** The item's `Mod_Array` is not applied
on its own: its compiled effect list holds one element for the effect named by the WEENIECONTENT
label `PropertyModEffect` (class 734), with the item's modifiers in that element's data and
spellcraft −1 (all 92,396 items with `Mod_Array`). In the client library an effect's modifier
entries are evaluated at the applied effect's spellcraft, and a spellcraft passed with the
application request that is not −1 replaces the stored one (`combat-data.md` §1, §5.5, §5.6);
the order in which modifiers combine is `combat-data.md` §5.3–§5.4. Progressions are evaluated
per `combat-data.md` §4.10; §9.5 confirms that evaluation against the stored armour values.

> **OPEN (F21):** Which spellcraft the server passes when it applies an item's
> `PropertyModEffect` element, and therefore which level indexes the item's modifier
> progressions and level ranges. The data stores −1 ("not given") for every item; the client
> library's static item effect function passes −1.0; the value comes with the server's
> application. LC uses the item level (Reported). An owner observation would settle it: the
> stat panel of a character wearing one item whose modifiers come from a progression, read
> before and after the character gains a level (if the bonus does not change, the item level
> governs).

<a id="s5-9"></a>
### 5.9 Effects and granted abilities

Four arrays name effects. Each element is an `EffectGenerator_EffectStruct`:

| Field | Type | Observed | Meaning |
|---|---|---|---|
| `EffectGenerator_EffectID` | DID → effect record (0x70, `combat-data.md` §6) | every struct | The effect |
| `EffectGenerator_EffectSpellcraft` | float | −1.0 in 25,846 of the 26,659 structs of the first three lists; positive in 812; 0.0 in 1 | Evaluation level of the effect; −1 = not given, the level then comes with the application request (`combat-data.md` §1, C49); copied unchanged into the compiled list (§3.2) |
| `EffectGenerator_EffectSpellcraftFloor`, `…Ceiling` | int | in 2,876 structs; (−1, −1) in 2,786 | Item-level window of a level-dependent effect; −1 = open end (Confirmed, BC + OBS, F35; rule below) |
| `EffectGenerator_EffectDataList` | array | in 9,361 usage structs | Arguments for the effect: `Effect_Crafting_Recipe` (6,055), `Effect_GrantTrait_Trait_To_Grant` (1,385), `Effect_GrantSkill_Skill_To_Grant` (1,263), `Effect_GrantTraitRank_Grant_List` (587), `Effect_Crafting_Profession` (71), `Effect_Crafting_RecipeList` (71) |

| List | Items | Elements | Element count per item | Main target classes |
|---|---|---|---|---|
| `EffectGenerator_UsageEffectList` | 13,480 | 14,599 | 1 (12,631) to 7 | 770 (6,055, recipe learning), 722, 734, 721, 5724, … |
| `EffectGenerator_EquipperEffectList` | 8,568 | 8,925 | 1 (8,267) to 15 | 718 (5,173), 734 (2,772), 736, 3686, 3124 |
| `EffectGenerator_TargetEffectList` | 3,084 (weapons) | 3,135 | 1–2 | 756, 734, 755 |
| `EffectGenerator_HotspotEffectList` | 62 | 137 | 1–4 | 718 |

Reported (LC): usage effects apply when the item is used, equipper effects while it is
equipped, target effects on the target of a weapon hit. Equipper and target lists occur only
on items; usage lists also on 90 non-item records (classes 794, 1724, 3826) and hotspot lists
on 2,246 records of class 1170.

**Level-dependent effects (Confirmed, F35).** A struct with a floor or a ceiling other than −1
(90 structs here: 14 in equipper lists, 76 in target lists, all in scaling items whose override
property is `Advancement_Level`; windows [61, 1000] in 62, [61, 69] in 16, [70, 1000] in 12) is
level-dependent. The client
library leaves it out when it compiles the item's static effects (none of the 90 is in any
compiled list, §3.2) and adds it at run time, for a scaling item, when the item's effective
level lies inside [floor, ceiling], both ends inclusive, a −1 end being open. The effective
level is `Item_Level` plus the distribution level offset of §6.4. Structs whose floor and
ceiling are both −1 are always compiled. When a scaling item's level changes, the client library
removes the level-dependent effects it added (it keeps their effect IDs in the item's
`ItemMunging_LevelMungedEffectIDArray`) and adds those whose window contains the new level.

Related labels: `Item_GrantedSkill` (1,262, → skill class 827), `Item_GrantedTrait` (1,557, →
trait classes 1477, 3509, 2525, 1483), `Usage_SkillToExecute` + `Usage_SkillLevel` (1,194 /
1,098), `Effect_IsEnchantable` (149,611; true 119,174; not read by the client library, use OPEN, §14).

<a id="s5-10"></a>
### 5.10 Usage: behaviour objects and requirements

**Behaviour objects (Confirmed).** Four labels name records whose property sets are small (the
24 permission and handler targets hold at most six properties, mostly menu texts or a
mouse-over type; most are empty) and whose behaviour lives in the script code of their root
class:

| Label | Items | Distinct targets | Most common targets (records: WEENIECONTENT label where one exists) |
|---|---|---|---|
| `Usage_Action` | 151,230 | 99 | 0x7000044D (91,154, no label); 0x70000000 (29,939, `UsageAction_DoNothing`); 0x7003EF28 (15,411); 0x7000D95C (4,805); 0x700225C2 (2,988) |
| `Usage_Permission` | 149,760 | 15 | 0x7000044C (91,154); 0x70000427 (58,024, `UsagePermission_Default`); 0x7000024B (130, `UsagePermission_AlwaysAllow`) |
| `Usage_TakePermission` | 151,188 | 3 | 0x70000441 (98,545); 0x70000426 (42,441); 0x7000024C (10,202, `TakePermission_CantTake`) |
| `CSM_ActionHandler` | 105,092 | 6 | all labelled: `EquippableActionHandler` (87,869), `DefaultActionHandler` (16,114), `ConsumableActionHandler` (553), `TextItemActionHandler` (301), `DoorActionHandler` (253), `NPCActionHandler` (2) |

The targets of `Usage_Permission`, `Usage_TakePermission` and `CSM_ActionHandler`, and 59 of
the 99 `Usage_Action` targets (named by 121,162 items), are each the only record of their root
class (for example class 1567 for 0x7000044D). The other 40 `Usage_Action` targets belong to
classes with several records: class 1556 (2 records, one of them `UsageAction_DoNothing`,
named by 30,029 items), 3904 (30 records, 30 items) and 3859 (8 records, 9 items).
WEENIECONTENT labels are read at run time (`property-system.md` §8.1); unlabelled targets are
identified by DID or class. The same 91,154 items (the equippable ones) use 0x7000044D and
0x7000044C.

> **Decision (Tessera sandbox server):** dispatch "use" by the `Usage_Action` target DID with a
> table of Tessera's own behaviours, and treat unknown targets as "do nothing".

**Requirements and limits** (authored; semantics Reported LC unless marked):

| Label | Type | Items | Notes |
|---|---|---|---|
| `Usage_RequiredClassList` | array of `Usage_ClassType` (enum 0x2300000E) | 20,000 | 41 distinct lists |
| `Usage_RequiredRaces` | array of `Usage_RequiredRaceEntry`, each a bare enum of mapper 0x2300000B (no fields) | 322 | 3,373 entries over all 1,390 holders of any class (1,029 of the arrays inside `DefaultPermissionBlobStruct`) |
| `Usage_RequiredFaction` | struct (`Usage_RequiredFaction_DataID` → class 1889, `Usage_RequiredFaction_Tier`) | 4,569 | |
| `Usage_RequiredTrait`, `Usage_RequiredTrait_Array` | DID / array of `Usage_RequiredTrait_Struct` | 2,577 / 1,024 | Each struct holds `Usage_RequiredTrait` (DID) and `Usage_RequiredVirtueRank` (int, dictionary default −1) in all 1,098 structs; the dictionary also allows `Usage_RequiredVirtueRankMax` (int), which no struct carries |
| `Usage_BarringTrait`, `Usage_BarringSkill`, `Usage_BarringCraftRecipe`, `Usage_BarringEmote` | DID | 1,647 / 1,261 / 6,055 / 96 | Barring condition (label) |
| `Usage_BarringTrait_Array`, `Usage_BarringSkill_Array` | array of bare `Usage_BarringTrait` / `Usage_BarringSkill` DIDs | 252 / 383 (378 general items, 5 packages) | 280 and 788 elements. The dictionary has no `_Array` form of the other two barring labels |
| `Usage_MinGloryRank` | int | 1,084 | |
| `Usage_RequiredCraftProfession` (→ class 1023), `Usage_RequiredCraftTier`, `Usage_RequiredCraftMastery` | DID / enum (mapper 0x23000063) / enum (mapper 0x23000063) | 6,152 / 6,126 / 149 | |
| `Usage_QuestRequirements` | array of `Usage_QuestRequirement` structs: `Usage_Operator` (enum, mapper 0x23000039), `Usage_QuestID` (DID), `Usage_QuestStatus` (enum, mapper 0x23000017) | 397 | 16,232 structs in all holders; one lacks `Usage_QuestStatus` and two lack `Usage_Operator` (the dictionary default applies, §14) |
| `Usage_RestrictedEffect`, `Usage_RequiredEffect` | DID → effect | 5,522 / 236 | |
| `Usage_CooldownDuration` | enum (0x2300006D) | 7,229 | 46 keys; seconds from the cooldown control (§9.8) |
| `Usage_CooldownChannel` | enum (0x2300006C) | 5,432 | Shared-cooldown group (187 keys used) |
| `Usage_EquipCooldownDuration` | enum (0x2300006D) | 85,754 | key 0 in 85,266 |
| `Usage_DestroyOnUse` | boolean | 15,589 | true in all |
| `Usage_AllowedInCombat` | boolean | 16,156 | false in all (dictionary default true) |
| `Usage_AllowedWhileMounted` | boolean | 139,591 | true in all (dictionary default false) |
| `Usage_MinUsageDist` | float | 151,230 | 2.0 in 95.6 % (25 distinct) |
| `Usage_TargetType`, `Usage_ValidTargetTypes` | bitfield 32 | 71,389 / 15,569 | |
| `ItemDecay_Duration` | enum (0x2300030C) | 249 | Lifetime; seconds from the decay control (§9.8) |

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** Field labels and types missing from the requirement
> table: the fields of a `Usage_RequiredRaceEntry` (the implementation returns the raw
> elements); the type of `Usage_RequiredVirtueRank` (read as an int or an enum key); the
> types of `Usage_RequiredCraftTier` and `Usage_RequiredCraftMastery` (the row says
> "DID / enum" for three labels; both are read as enum keys); the types of `Usage_Operator`
> and `Usage_QuestStatus` (read as enum keys, as `map-data.md` §8.4.1 gives them); the
> `_Array` forms of the barring labels (not read). Please state them.
>
> **Answered in part (2026-10-08, merge):** `interaction.md` §3.7.1 gives `Usage_Operator` as
> an enum of mapper 0x23000039 (a label has one dictionary type wherever it occurs). The other
> labels and types asked for here remain open.
>
> *Answer (2026-10-09, F47):* the table above now gives every type: race entries are bare
> enums, the virtue rank is an int, craft tier and mastery are enums of mapper 0x23000063,
> the quest-requirement struct is spelled out, and the two `_Array` barring forms are arrays
> of bare DIDs.

Where a label's value is the same in every item that has it but differs from the dictionary
default (the last three booleans), the **presence** of the label is the information; an
absent label stands for whatever default the reading code supplies. The client library supplies
one per read site, usually the dictionary default (§14, F43); the read sites of these three
labels were not traced, so Tessera applies the dictionary default (§14 decision).

<a id="s5-11"></a>
### 5.11 Sockets, essences and traceries

**Sockets on equipment.** `Item_Socket_Array` (9,938 items) is an array of
`Item_Socket_Array_Entry` structs with exactly two fields (19,953 entries):
`Item_Socket_Type` (bitfield 64, mapper 0x23000432 `ItemSocketType`, one bit set in every entry)
and `Item_Socket_Unlock_ILevel` (int). Sockets per item: 1 (4,835), 2 (3,083), 3 (688),
4 (1,078), 5 (46), 11 (150), 12 (58). Socket types used: `Classic` 16,155, then
`PvP_Essence`, `Essence_Mainstat`, `Legacy_Power`, `Legacy_Craft`, … (22 types).

**Socketables** (15,445 general items, `Item_Class` = `Essence`): `Item_Socket_Type` at top level
(one bit set in 15,405; zero in 40) says which socket accepts them. With it (15,416 items):
`Item_Socket_GemMinLevel` / `Item_Socket_GemMaxLevel` (int), `Item_Socket_LevelupRuneIncrement`,
`Item_Socket_Operation` (DID → class 4216 in 15,376, class 4215 in 40; each the only record of
its class, with one property `Item_Socket_DragOverlay`), `Item_TraceryUpgrade_Value` (1 in all),
`Item_TraceryUpgrade_Component_Result` (10,208 items, each naming a different item of class 796),
`Item_UniquenessChannel` and `Item_PropertySet` (DID → item set, 516 items, §7.1).

The socket control record (§6.4) maps socket types to frame images, socket types to level-up
cost properties, and minimum levels to essence overlays.

**Socket rules of the client library (Confirmed, BC, F36; settles the former OPEN).** Socketing
uses two operation objects named by `Item_Socket_Operation`: class 4216 (15,376 socketables:
place a gem) and class 4215 (the 40 others: raise a socketed gem's level, below). Census of the
socketables with `Item_Socket_Type` (15,416): the 1,710 essences (socket types `Classic` and the
six `*_Essence` / `Essence_*` kinds) have `Item_Socket_GemMinLevel` = `GemMaxLevel` (one level,
for example 182 / 182); the 13,666 traceries (the `Legacy_*` and `Triangle` kinds) have windows
such as 50–125, 130–200, 201–299, …, 550–599; 29 `Classic` items have neither; the 40 class-4215
items have socket type 0, windows such as 50–125 or 449–499 and `Item_Socket_LevelupRuneIncrement`
1–4. (The former text's "500 / 499" was not a common value and is withdrawn.)

- **Lock.** Socket *i* of the host is **locked** while its entry has
  `Item_Socket_Unlock_ILevel` *u* and the host's `Item_Level` is less than *u*; an entry without
  *u* is never locked. A locked socket takes no gem. The client counts unlocked sockets as the
  entries minus the locked ones.
- **Type.** The gem fits socket *i* when its `Item_Socket_Type` and the socket's
  `Item_Socket_Type` share at least one bit (bitwise AND not 0).
- **Checks in order** when a gem is placed: the host and the gem exist; type and lock (above);
  gem uniqueness (below); then the level checks:
  1. If the host has `Item_Socket_RequiredMinLevel` / `…RequiredMaxLevel` (absent defaults 0 and
     −1, both defaults = no window), the gem's `Item_Level` must lie inside it. No item has them
     here.
  2. The character's `Advancement_Level` must be at least the gem's `Usage_MinLevel` (1 when
     absent), and, when the gem's `Usage_MaxLevel` is greater than 0, at most that.
  3. When the target socket's `Item_Socket_Type` is exactly one of the essence kinds (0x1,
     0x20000, 0x40000, 0x80000, 0x200000, 0x400000, 0x800000): the gem's
     `Item_Socket_GemMinLevel` (0 when absent) must not exceed the host's `Item_Level`. For any
     other socket type: when the host's `Usage_MinLevel` is greater than 1, it must be at
     least the gem's `Usage_MinLevel`.
- **Gem level shown.** For a socketed gem the client reports the gem's
  `Item_Socket_GemLevel` as its current level and the gem item's `Item_Socket_GemMaxLevel` as its
  maximum; for a host with `Item_Reforgable` the maximum is capped at the host's `Item_Level`,
  and an empty socket of such a host reports level 45 and the host's level as maximum.
- **Raising a gem (class 4215).** The rune's `Item_Socket_GemMinLevel` must not exceed the host's
  `Item_Level`; the socketed gem's current level must be below the smaller of the host's
  `Item_Level` and the gem's `GemMaxLevel` (when that smaller value is greater than 0) and, when
  the rune's `GemMaxLevel` is greater than 0, below the rune's `GemMaxLevel`. The amount per rune
  is `Item_Socket_LevelupRuneIncrement` (the check multiplies it by the number of runes used; the
  function that applies the raise was not read).
- **Uniqueness.** Only gems with `Inventory_UniqueEquipped` true are checked. Replacing a gem in a
  socket that already holds a gem of the same non-zero `Item_UniquenessChannel` is allowed.
  Otherwise the host must not already contain the same gem DID or a gem of the same non-zero
  channel; and if the host is equipped (its current slot value has the `Equipment` bit
  0x20000000), no equipped item of the character may contain the same
  unique gem DID or a unique gem of the same non-zero channel.

Whether the server enforces the same checks is not in the data.

<a id="s5-12"></a>
### 5.12 Legendary and item-advancement data on items

| Label | Type | Items | Notes |
|---|---|---|---|
| `ItemAdvancement_Item` | boolean | 4,902 | true in all; marks advancing ("legendary") items |
| `ItemAdvancement_ProgressionGroupOverride` | DID → class 2139 | 4,902 | Weighted lists of legacy pools (§7.3) |
| `ItemAdvancement_StaticEffectGroupOverride`, `…Group2Override` | DID → class 2139 | 1,399 / 98 | |
| `ItemAdvancement_CombatDPSLevel` | int | 3,766 | DPS level (§9.3) |
| `ItemAdvancement_CombatPropertyModDID`, `…ModLevel`, `…ModLargeIconDID`, `…ModSmallIconDID`, `Item_RequiredCombatPropertyType` | DID → effect 734 / int / 0x41 / 0x41 / enum | 1,136 each | Non-DPS main legacy (Reported LC). Every advancing item has exactly one of `ItemAdvancement_CombatDPSLevel` (3,766) and this group (1,136) (Confirmed) |
| `ItemAdvancement_CanBeDeconstructed`, `…DeconstructionCostOverride`, `…RelicDeconstructionCostOverride` | boolean / int / int | 5,393 / 527 / 527 | |
| `ItemAdvancement_Title` | DID → class 2338 | 421 (general items) | Title-granting scrolls |
| `ItemAdvancement_RunicToCreate` | DID → class 2219 | 391 | Relic-creating items |
| `ItemAdvancement_RunicLootListOverride` | DID → class 2229 | 136 | |
| `ItemAdvancement_ReplacementLegacy`, `…AllowedLegacyToReplace`, `…LegacyType`, `…NewLegacyToAdd` | DID → effect / DID / enum (0x23000323) / DID | 405 / 405 / 404 / 7 | Legacy-replacement scrolls |
| `Item_MaxLevelUpgrades` | int | 4,905 | 3 in most |
| `Item_Reforgable`, `ItemAdvancement_ReforgeTable` (→ class 2327) | boolean / DID | 448 / 27 | |
| `Item_PILI_*` (10 labels: base weapon item, title, four relic DIDs, legacy entry list → class 4015, XP to apply, two tier counts) | | 181 each | A group of 181 general items that each describe a fully specified advancing weapon. No `Item_PILI_*` PID occurs in the client library's bytecode (F44); their use is server behaviour (OPEN, §14) |

<a id="s5-13"></a>
### 5.13 Kind-specific groups

| Kind | Labels (items) | Notes |
|---|---|---|
| Recipe scrolls (805) | `RecipeItem_Recipe` (6,055 → recipe class 1024), `Usage_BarringCraftRecipe` (6,055, same target), usage effect with `Effect_Crafting_Recipe` | The recipe is named three times (`RecipeItem_Recipe`, `Usage_BarringCraftRecipe`, the `Effect_Crafting_Recipe` argument of the single usage effect); all three agree in all 6,055 |
| Multi-recipe scrolls (3663) | usage effect data `Effect_Crafting_RecipeList`, `Effect_Crafting_Profession` (71 each) | |
| Packages (2814) | `PackageItem_TrophyListTemplate` (3,634; → loot list class 1237 in 3,390, null in 243), `PackageItem_FilteredTrophyTableTemplate` (3,582; → class 3666 in 275), `PackageItem_Freep_WeightedTreasureTableID` / `…Creep_…` (→ class 3307), `PackageItem_PreviewList` (429 items, 1,366 preview item DIDs), `PackageItem_BindAllItemsToAccount` / `…ToCharacter`, `PackageItem_LockBox` and lock IDs, `PackageItem_UseCachedMungeLevel`, `PackageItem_UsePlayerAsContainerForMunging` | Contents come from loot tables (loot family) |
| Text items (798) | `TextItem_Title`, `TextItem_PrefabContentList`, `TextItem_AuthorName` (301 / 301 / 284) | |
| Doors (1722), interactive objects (2097) | `Usage_PlayersCanOpen`, `Usage_PlayersCanClose`, `Door_*` (animation scriptlets 0x0C), `BigBattle_Interactive_*`, `MultiUsage_ActivityCollection` | World fixtures |
| Travel points (815), milestones (804) | `PortalDestination`, `Usage_LayerArray`, `Waypoint_Cost`, `Milestone_TravelLink`, `Travel_TravelFlags` | World fixtures |
| Keys (803) | `Key_CanBePlacedOnKeyRing`, `Usage_KeyID` (enum, 81 items) | |
| Instruments (796) | `Usage_IsMusicalInstrument`, `Item_Music_Instrument_Type` (enum 0x230000FC), `Item_Music_Channeling_State` (→ class 961) (1,077 each) | |
| Crafting tools and trinkets (796) | `Item_CraftToolCategory` (478), `CraftTrinket_Profession` (→ class 1023), `CraftTrinket_Tier` (1,248) | |
| Decorations (796) | `Item_Decoration_Category` (5,047), `Item_Decoration_PropertyHook_Name` / `_Value` (635) | Housing |
| Paper items (796) | `PaperItem_Category` (610), `PaperItem_SmallIcon` (674), `PaperItemControl_CurrencyBonusPropertyArray` (697) | Wallet currencies, listed by the paper-item control (§6.4) |
| Reputation items (796) | `Reputation_IsReputationItem`, `Reputation_Faction` (→ class 1889), `Reputation_ReputationGain` (int64) (268–269) | |
| Mounts (796) | `Mount_SkillToGrantTall`, `Mount_SkillToGrantShort` (377), `Mount_Type` (52) | |
| Hobby items (796) | `Hobby_*` (552) | |

---

<a id="s6"></a>
## 6. Shared lookup and control records

All of these are ordinary 0x70 records with property sets. Control records are found through
their WEENIECONTENT label (`property-system.md` §8.1); the DIDs are this install's and must not
be hard-coded.

<a id="s6-1"></a>
### 6.1 Value tables (class 1688, 99 records)

Each has exactly two properties:

| Label | Type | Content |
|---|---|---|
| `Item_BaseValArray` | array of `Item_BaseValStruct` { `Item_BaseVal`: float } | 601 elements in every table; element *k* (1-based) is the base value at item level *k* |
| `Item_QualityArray` | array of `Item_QualityStruct` { `Item_Quality`: enum, `Item_QualityModVal`: float } | 5 elements, one per quality key 1–5 (every table) |

Referenced by `Item_ValueLookupTable` of 145,477 items (+41 carry-alls).

<a id="s6-2"></a>
### 6.2 DPS tables (class 1660, 8 records)

| Label | Type | Content |
|---|---|---|
| `Combat_BaseDPSArray` | array of `Combat_BaseDPS` (float) | 701 elements; element *k* is the base DPS at level *k* |
| `Combat_QualityModArray` | array of `Combat_QualityModData` { `Combat_Quality`: enum, `Combat_DPSMod`: float } | 5 elements, quality keys 1–5 |

Six of the eight are used by weapons (`Combat_DPS_LUT`); the other two only by advancement
widgets (`ItemAdvancement_AdvanceableWidget_DPSLUT`, class 4024).

<a id="s6-3"></a>
### 6.3 Weapon timing and wear control

- **Weapon timing tables** (class 1691, 24 records), listed by the record labelled
  `WeaponActionDurationTable` (0x70000254 here, class 1692, property
  `Item_WeaponActionDurationTable`, an array of DIDs). Each table has `Item_EquipmentCategory`
  (one category) and `Item_WeaponActionDurationDataArray`: entries { `Item_WeaponSpeed`: enum
  0x23000066, `Item_BaseActionDuration`: float, `Item_BaseAnimDurationMultiplierMod`: float },
  one per speed key 1–6. All 144 entries are (1.0, −0.1) on this install.
- **Wear control** (label `ItemWearControl`, 0x70000224 here, class 1672):

| Label | Entries | Entry struct: key field (enum, mapper) and value field (type) |
|---|---|---|
| `ItemWearControl_MaxStructurePointsArray` | 50 | `ItemWearControl_MaxStructurePointsEntry`: `ItemWearControl_MaxStructurePointsEnum` (0x230000F9, the mapper of `Item_MaxStructurePointsEnum`), `ItemWearControl_MaxStructurePointsValue` (int) |
| `ItemWearControl_DurabilityArray` | 7 | `ItemWearControl_DurabilityEntry`: `ItemWearControl_DurabilityEnum` (0x230000FA, the mapper of `Item_DurabilityEnum`), `ItemWearControl_DurabilityValue` (float) |
| `ItemWearControl_RepairCostQualityModArray` | 5 | `ItemWearControl_RepairCostQualityModEntry`: `ItemWearControl_RepairCostQualityModEnum` (0x23000067, the mapper of `Item_Quality`), `ItemWearControl_RepairCostQualityModValue` (float factor) |
| `ItemWearControl_ItemWearStateValueModArray` | 4 | `ItemWearControl_ItemWearStateValueModEntry`: `ItemWearControl_ItemWearState` (0x230000FB, the mapper of `Item_WearState`), `ItemWearControl_ItemWearStateValueMod` (float factor; 1.0 for the key every item has) |
| `ItemWearControl_SkillGateArray` | 5 | `ItemWearControl_SkillGateEntry`: `ItemWearControl_SkillGateEnum` (0x230000FD), `ItemWearControl_SkillGateValue` (float) |

Every entry of the five arrays holds exactly its two fields (OBS3, F48).

**Roles (Confirmed, client library, F37; settles the former OPEN).** The wear control's
script object holds its tables as maps; the client library reads two of them, recognisable by
their keys:

- **Repair-cost quality factors** (`ItemWearControl_RepairCostQualityModArray`, keyed by
  `Item_Quality`; 0.0 for a missing key). Repair cost per structure point = the item's
  `Item_RepairCostPerPointProgression` evaluated at its `Item_Level` × the factor for its
  quality (0 when the item has no progression). The cost of repairing one item = cost per point
  × the discount factor × the repairer's `Repair_CostModifier` (−1.0 when absent) ×
  (`Item_MaxStructurePoints` − `Item_CurStructurePoints`), in single precision in that order,
  truncated toward zero. Precisely: the first three factors are multiplied left to right in
  single precision; the structure-point difference is a 32-bit integer subtraction (wrapping,
  `wlib-bytecode.md` §4.1), and that difference is converted to single precision **as an
  unsigned** value; the last product is truncated toward zero to a signed 32-bit integer (a
  NaN or out-of-range product gives −2,147,483,648, `wlib-bytecode.md` §5, f32 → i32). Factors
  that are not exact in binary therefore matter: a discount of 0.9 can give one less than the
  exact decimal product (§13.11). The discount factor is 1.0 minus the sum of the character's values
  (float, 0.0 when absent) of the properties named in the repairer's discount list, never below
  0.0. The cost of repairing several items is the sum of the single costs.
  - **No modifier means a negative cost.** The −1.0 default of `Repair_CostModifier` is what
    the client library supplies (Confirmed, BC): a repairer without the property yields a
    negative cost (the same magnitude as with 1.0, sign reversed). This is the observed
    behaviour, not a defect to correct; an implementation that reproduces the client's figure
    keeps it.
  - **Current above maximum.** In the data the two structure-point properties occur together
    and are equal in all 87,020 items (F15), so the difference is 0. The client library does
    not guard the case current > maximum: the 32-bit difference wraps to 2³² − (current −
    maximum), its unsigned conversion is about 4.29 × 10⁹, and unless the other factors make
    the product 0 the truncation is out of range and gives −2,147,483,648 (or a very large
    value when the product still fits). Both getters return −1 when their property is absent,
    so an item with neither property also gives a difference of 0. Whether the server can
    ever send current > maximum is not in the data.
- **Wear-state value factors** (`ItemWearControl_ItemWearStateValueModArray`, keyed by
  `Item_WearState`; 1.0 for a missing key). An item's **sale value** = stack quantity
  (`Inventory_Quantity`, at least 1) × `Item_Value`, times the factor of its wear state when the
  item is subject to wear (§5.5), truncated toward zero; a positive value that truncates to 0 or
  less becomes 1; a value of 0 stays 0. Precision: quantity × `Item_Value` is an integer
  product; when it is 0 the result is 0 at once; otherwise it is converted (signed) to single
  precision, multiplied in single precision by the wear-state factor (1.0 when the item is not
  subject to wear) and truncated toward zero.
- **Skill-gate table** (`ItemWearControl_SkillGateArray`): not read by the client library. (A
  32-bit literal equal to the PID of `ItemWearControl_SkillGateValue` occurs once, as an error
  code returned by a crafting check, not as a property read.)

An item whose `Item_CurStructurePoints` equals `Item_MaxStructurePoints` needs no repair. The
client library looks for equipped items in wear states `Damaged` (key 1) or `Broken` (key 3) to
tell whether any equipped item needs attention. Whether the server uses the same arithmetic is
not in the data.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The entry field labels are abbreviated ("`…_MaxStructurePointsEnum`")
> or not given. The implementation reads `ItemWearControl_MaxStructurePointsEnum`,
> `ItemWearControl_MaxStructurePointsValue`, `ItemWearControl_DurabilityEnum` and
> `ItemWearControl_DurabilityValue` (all four are defined in the dictionary and reproduce
> §9.6 on every item). The entries of the repair-cost, wear-state and skill-gate arrays are
> read by field type (first enum field = key, first float or int field = value), because
> their labels are not given. Please spell out every entry label and value type.
>
> **Answered in part (2026-10-08, merge):** the "Roles" paragraphs above give, for the
> repair-cost array, the key `Item_Quality` and a float factor (0.0 for a missing key); for the
> wear-state array, the key `Item_WearState` and a float factor (1.0 for a missing key); and
> state that the client library does not read the skill-gate array. The entry field labels of
> those three arrays remain open.
>
> *Answer (2026-10-09, F48):* the wear-control table above spells out every entry struct, key
> field (with its mapper) and value field (with its type).

<a id="s6-4"></a>
### 6.4 Other control records

| WEENIECONTENT label | DID here (class) | Item-relevant content |
|---|---|---|
| `InventoryControl` | 0x70000230 (1202) | Slot counts and limits; `InventoryControl_DistributionToLevelOffsetList` (15 entries: distribution key → level offset, all 0 here; added to `Item_Level` for the armour lookup and the window of level-dependent effects, §9.5, §5.9); `…_DefaultEquipperEffectList` (13 equipment categories → effect lists) and `…_DefaultTargetEffectList` (9 categories); `…_EquipmentCategoryCachedSlotList` (3 slots → a cached-category property); `…_SlotToPaperdollLocation_MappingList`; `…_ExamProperty_Array` (38 property IDs: the per-instance properties of an item, for example `Inventory_BoundToID`, `Item_CurStructurePoints`); `…_MergeProperty_Array` (6 property IDs including `Item_Value` and `Item_Level`); `InventoryControl_SmartSortInfo_Array` (8 entries: a sort type and a list of property IDs) |
| `CooldownControl` | 0x70000264 (1664) | `CooldownControl_DurationMapList`: 63 entries (cooldown key → seconds, float) |
| `ItemDecayControl` | 0x70021EB3 (2809) | `ItemDecayControl_DurationMapList`: 20 `ItemDecayControl_DurationMapListEntry` structs, each `ItemDecayControl_DurationType` (enum, mapper 0x2300030C, the mapper of `ItemDecay_Duration`; 20 distinct keys) and `ItemDecayControl_DurationValue` (float, seconds; dictionary default 0.0) (F48) |
| `ItemMungingControl` | 0x70000252 (1671) | Dye colour and pattern lookup lists (`appearance.md` §4.9) |
| `ItemSocketControl` | 0x70040DEE (4005) | Socket images per type, level-up cost property per socket type, essence overlays by level |
| `ItemAdvancementControl` | 0x7000EAA6 (2152) | 42 game-wide legendary settings (§7.3) |
| `GameSetDirectory` | 0x70009869 (1827) | `Set_Directory_GameSetList`: all 2,224 set DIDs (§7.1) |
| `RelicTransmutationDirectory` | 0x70025633 (3139) | 506 transmutation recipes (§7.2) |
| `PaperItemControl` | 0x70024867 (3105) | Paper-item lists (443 + 167 items), version upgrades (84 old/new pairs), services |
| `CosmeticPetDirectory` | 0x7004832E (4091) | Cosmetic companions (§7.4) |
| `WardrobeControl` | 0x7001BE4C (2639) | Wardrobe (appearance family) |

**Examination and merge lists (Confirmed, F53; settles most of the former OPEN).** The
`InventoryControl` object record (0x70000230 here) stores both lists in its compiled half: root
attribute 0x05D1B393 is a native class-17 list of 44 PIDs, the 38 of
`InventoryControl_ExamProperty_Array` followed by the 6 of `InventoryControl_MergeProperty_Array`,
in stored order; root attribute 0x04154F39 is a class-17 list of the 6 merge PIDs alone. The
client library reads the first through a getter of class 1202 (function 33 here), and its one
caller is the function that assembles an item's examination (tooltip) data (class 269 function
1 here, inherited by 412): it creates a set with `UI_Examination_Tooltip_DID` (and
`UI_Examination_Tooltip_AlternateSourceIID` when there is an alternate source), then, for each PID
of the list in order that the examined object's properties hold, appends that property to the
set's `UI_Examination_Tooltip_ArrayHelper` array. So the examination list names the per-instance
values an examination (tooltip) carries, and the merge PIDs are carried too. No class-library
code reads the separate merge list.

> **OPEN:** What the separate merge list (root attribute 0x04154F39) is used for, beyond
> adding its PIDs to the examination list. No class-library function reads it; a native use
> (stack merging is suggested by the label only) was not traced. Nothing in this spec depends on
> it.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The entry layout of `CooldownControl_DurationMapList` and
> `ItemDecayControl_DurationMapList` is not given (labels of the key and seconds fields, and
> the type of the decay seconds). The implementation reads each entry struct by field type:
> first enum field = key, first float (or int) field = seconds. This reproduces §9.8 coverage
> on every item. Please state the labels and types.
>
> **Answered in part (2026-10-08, merge):** `economy.md` §11.5.3 gives the cooldown entries:
> `CooldownControl_DurationMapListEntry` with `CooldownControl_DurationType` (an enum key) and
> `CooldownControl_DurationValue` (float, seconds). The `ItemDecayControl_DurationMapList`
> entry labels and the type of its seconds remain open.
>
> *Answer (2026-10-09, F48):* the `ItemDecayControl` row of the table above gives the entry
> struct: an enum key `ItemDecayControl_DurationType` and a float `ItemDecayControl_DurationValue`.

---

<a id="s7"></a>
## 7. Auxiliary item families (ancestor 486)

These records are not items (their root classes descend from 486 → 89 → 485 → 484, not from
1733) but exist only to serve items.

<a id="s7-1"></a>
### 7.1 Item sets (class 1824, 2,217 records)

Class 1824's parent is 1817, which also roots **class 2078** (7 records): those use the same
labels, but their members are traits (class 1477), not items. Both are listed in the
`GameSetDirectory` record (2,217 + 7 = 2,224 DIDs).

| Label | Type | Records | Observed | Meaning |
|---|---|---|---|---|
| `Set_Name`, `Set_Description` | StringInfo | 2,217 | | |
| `Set_Icon` | DID → 0x41 | 2,217 | one surface in all | |
| `Set_Level` | int | 2,217 | 65 distinct | Set level used for the tiers' effects (Confirmed, BC, F41; rule below) |
| `Set_LevelRequired` | **boolean** | 2,217 | true in all | Not read by the client library (its PID occurs in no bytecode body); LC reads it as an integer, it is a boolean |
| `Set_MaxPlayerLevel` | int | 2,217 | 52 distinct | Maximum character level for the bonuses; −1 when absent = no limit (Confirmed, BC, F41) |
| `ItemGameSet_UseAverageItemLevelForSetLevel` | boolean | 100 | true | The set level is the average level of the highest active members (Confirmed, BC, F41; rule below) |
| `Set_MemberList` | array of `Set_MemberDID` (DID → item) | 2,191 | 2–18 members; 6 in 1,184 sets | Members |
| `Set_ActiveCountDataList` | array of `Set_ActiveCountData` | 2,217 | 1–7 entries | Bonus tiers |
| `Set_MinActiveMembers`, `Set_MaxActiveMembers`, `Set_ActiveCountProperty`, `Set_ActiveLevelProperty` | int / int / property ID / property ID | 26 | min 2 in all; max 4 (22), 5 (1), 8 (3); each set names its own two properties | Sets without a member list (below); `Set_MinActiveMembers` is not read by the client library |

`Set_ActiveCountData` fields: `Set_ActiveCount` (int; the lists of all 2,224 item and trait
sets are strictly ascending), `Mod_Array` (3,929 tiers; §5.8), `Set_EffectDataList` (1,449; an array of
`EffectGenerator_EffectStruct`, §5.9), `Set_StartScriptID` (1,746; enum 0x23000097),
`Set_ShortDescriptionForCount` (21).

**Membership (Confirmed).** 10,607 distinct items are named by the member lists of the item
sets; no item is in two lists; every member is an item. The 26 sets without a member list are named instead by
the `Item_PropertySet` of 516 socketables (22 of the 26 sets, 4 to 44 socketables each); the
other 4 are named by nothing. So **the members of a set are the items in its member list plus
the items whose `Item_PropertySet` names it.** A reverse index (item → set) must be built by
scanning; the member items themselves do not name their set.

Nine sets have two members but tiers up to 5 (`Set_ActiveCount` 2–5).

**Set rules of the client library (Confirmed, BC + BIN2, F41; settles most of the former
OPEN).** Whether the server uses the same rules is not in the data.

- **Count.** For a set with a member list, the count is the number of **distinct** members of
  the set that are active for the character. The client keeps, per set, a map from member to the
  list of item levels of its active copies; an item set accepts a second copy of a member (one
  more level in its list, no change to the count), a trait set refuses it (BIN2: the count is
  the map's key count). For a member-less set, the count is the value of the character's property named by
  `Set_ActiveCountProperty` (0 when absent).
- **Tier for a count *c*.** No tier when *c* < 2, or when *c* is greater than the set's member
  count (member-less sets: greater than `Set_MaxActiveMembers`). Otherwise the client reads
  position *c* − 2 of the set's tier list, which native code fills from
  `Set_ActiveCountDataList`; the fill was not traced, but the examination code (below) asks for
  every *c* and skips positions without a tier, so a position holds the tier whose
  `Set_ActiveCount` equals *c*, or nothing. Consequence: the 27 tiers of the nine two-member sets with counts
  3–5 are never returned; all 26 member-less sets have tiers only up to their maximum.
- **Examination.** The tooltip lists, for each *c* from 2 to the member count (or maximum), the
  tier for *c* if it exists, each with its effects described at the set level for *c*, together
  with the total member count and the current count.
- **Character level gate.** The bonuses apply only if `Set_MaxPlayerLevel` is −1 (absent) or
  the character's `Advancement_Level` (1 when absent) is at most `Set_MaxPlayerLevel`.
- **Set level for count *c*.** If `ItemGameSet_UseAverageItemLevelForSetLevel` is true: for a
  set with a member list, the average of the *c* highest item levels among the active members
  (sum ÷ *c*, single precision; a member with several active copies counts with the level of
  the copy added last); members that the request adds hypothetically count with their
  given level, and missing members up to *c* count with that level too; for a member-less set,
  the value of the character's property named by `Set_ActiveLevelProperty` ÷ the value of the
  count property, in single precision (so the level property is read as the sum of the active
  members' levels); when either the requested count or the count property is 0, the given
  level. Otherwise `Set_Level` (−1 when absent); if that is −1, the character's
  `Apparent_Level`. Every item set stores `Set_Level` here.

> **OPEN:** Whether the server activates the tiers cumulatively (every tier with
> `Set_ActiveCount` ≤ the count) or only the tier whose count equals the current count, and
> whether it ignores tiers above the member count as the client does. The client library only
> looks tiers up per exact count and lists them separately; activation is server behaviour. An
> owner observation would settle it: the character's stats with 3 members of a set equipped whose
> tiers are at counts 2 and 3 (cumulative if both bonuses show).

<a id="s7-2"></a>
### 7.2 Relics and transmutation

- **Relics** (class 2219, 706 records): `Runic_Name`, `Runic_Type` (bitfield, 6 values),
  `Runic_Level`, `Runic_Tier` (enum, mapper 0x230002A0; 31 distinct keys over the 706 relics),
  `Runic_RequiredItemLevel` (423), `Relic_ValidContainerSlots`,
  `Mod_Array` (all), `Runic_Deconstruction_Value` and `…_TrophyListTemplate`, icons and usage
  labels. They carry icon layers and `Usage_*` labels like items but are not in the item
  subtree. Reached from items (`ItemAdvancement_RunicToCreate` 391, `Item_PILI_Runic_Type1–4`
  181 each), from runic lists (469), runic treasure groups (class 2239, 450) and transmutation
  outputs (343).
- **Transmutation recipes** (class 3138, 506 records; F49): `Relic_Transmutation_Recipe_RelicCurrencyCost`
  (int, 1–58,500, all), `Relic_Transmutation_Recipe_TransmutationCategory` (enum, mapper
  0x23000322, all), `Relic_Transmutation_Recipe_Output_List` (all; an array of
  `Relic_Transmutation_Recipe_Output` structs, 602 in all, each `Relic_Transmutation_Recipe_Output_Relic`
  (DID: a relic or an item) and `Relic_Transmutation_Recipe_Output_Weight` (int, 1 in all
  602)), `Relic_Transmutation_Recipe_Can_Produce_Items` (bool, true in the 259 that have it),
  and in 12 recipes `Relic_Transmutation_Recipe_Output_Icon_Override` (DID → 0x41),
  `…_Output_Title_Override` and `…_Output_Tooltip_Override` (StringInfo). Listed by
  `RelicTransmutationDirectory`.
- **Runic lists** (class 2229, 169 records): one property, `RunicList_Array` of
  { `RunicList_DropFrequency`: enum, `RunicList_RunicOrTreasureGroup`: DID → relic or class
  2239 }. Named by runic level loot tables (class 2227, 1,050), by items (136) and by the runic
  combination control. Their use in loot is specified in `economy.md` §4.9.

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The value types of `Runic_Tier` (§7.2) and
> `ItemAdvancement_Mod_Weight` (§7.3) are not given. The implementation accepts an int or an
> enum key for the tier and any numeric value for the weight. Please state them.
>
> *Answer (F49):* `Runic_Tier` is an enum of mapper 0x230002A0 (§7.2);
> `ItemAdvancement_Mod_Weight` is an int, dictionary default 0, observed 1–65 (§7.3).

<a id="s7-3"></a>
### 7.3 Legacies and advancement tables

| Class | Records | Content | Named by |
|---|---|---|---|
| 2134 | 359 | `ItemAdvancement_Effect_Array`: 3,279 `ItemAdvancement_Effect_Entry` structs { `ItemAdvancement_Effect` (DID → effect 734), `ItemAdvancement_Mod_Weight` (int, 1–65) } | 2139 (1,519) |
| 2139 | 224 | `ItemAdvancement_ProgressionListArray`: 1,519 `ItemAdvancement_ProgressionList_Entry` structs { `ItemAdvancement_ProgressionList` (DID → 2134), `ItemAdvancement_ProgressionList_Weight` (int, 0–50) }; `ItemAdvancement_ProgressionPullCountList`: 4 `ItemAdvancement_ProgressionPullCount_Entry` structs per record (896 in all) { `ItemAdvancement_ProgressionPullCount_Amount` (int, 1–4), `ItemAdvancement_ProgressionPullCount_Weight` (int, 0–100) } | items (6,399 via the three override labels), reforge tables (class 2204, 350) |
| 2338 | 421 | Legendary titles: `Name` (StringInfo), `ItemAdvancement_IATitle_Category` (enum, mapper 0x230002C3), `ItemAdvancement_IATitle_Tier` (enum, mapper 0x230002A6; keys 1–3), `ItemAdvancement_Title_SlayerGenusType` (32-bit bitfield, mapper 0x2300000A; 0 in 255 records), `ItemAdvancement_Title_DamageType` (enum, mapper 0x23000028), all in every record; `EffectGenerator_RunicEffectList` (an array of `EffectGenerator_EffectStruct`, §5.9) in 344 and a `Mod_Array` in 105 | items (421 + 181) |
| 4015 | 329 | Imbued legacies, all fields in every record: `ItemAdvancement_ImbuedLegacy_Effect` (DID → effect), `ItemAdvancement_AdvanceableWidget_InitialMaxLevel` (int; 46 in all) and `…_AbsoluteMaxLevel` (int; 83 in all), `ItemAdvancement_AdvanceableWidget_Icon` and `…_SmallIcon` (DID → 0x41), `ItemAdvancement_AdvanceableWidget_Category` (enum, mapper 0x230003EB), `UI_Examination_Tooltip_0Click_Element` and `…_0Click_Layout` (tooltip layout, `ui-layout.md`), `ItemAdvancement_ImbuedLegacy_ClassicLegacyTransform` (struct of `ItemAdvancement_ImbuedLegacy_TierTransform_Array`, an array of `ItemAdvancement_AdvanceableWidget_UnlockedLevels` ints 0–5, 2,303 in all, and `ItemAdvancement_LegacyTypeName_Array`, an array of `ItemAdvancement_PropertyName` PIDs, 338 in all). The same widget labels occur in 17 records of classes 4024, 4025 and 4048 | `Item_PILI_Legacy_Entry_List` (1,267), the advancement control (333) |
| 2152 | 1 | The advancement control (label `ItemAdvancementControl`): 42 settings, including level tables, reforge levels, legacy name lists, relic and XP currencies (items) | WEENIECONTENT |

> **OPEN:** The legendary rules: how legacies are rolled from the class-2139 / class-2134 tables
> (weights, pull counts), imbuement, reforging, and what the 181 `Item_PILI_*` items do. None of
> `ItemAdvancement_ProgressionListArray`, `ItemAdvancement_ProgressionPullCountList`,
> `ItemAdvancement_Mod_Weight` or any `Item_PILI_*` PID occurs in the client library's bytecode
> (F44), so the rolls are server behaviour and cannot be read from the client. (The client
> library does hold legendary examination, imbued-legacy level and reforge-table functions,
> which this spec does not specify.) The tables are fully decodable with `property-system.md`.
> A Tessera server needs its own rule (out of scope before M8, decision 0004).

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** The records of classes 2139, 2338, 4015 (§7.3) and 3138
> (§7.2) are not given typed readers because their labels are abbreviated or missing:
> the field labels and types of the `ItemAdvancement_ProgressionListArray` entries
> ("{ → 2134, weight }") and of the `ItemAdvancement_ProgressionPullCountList` entries
> ("{ amount, weight }"); the full labels written "`…_Tier`" and "`…_DamageType`" (2338),
> "initial and absolute max levels", "icon", "tooltip layout" and "`…_ClassicLegacyTransform`"
> (4015), "`…_TransmutationCategory`", "`…_Output_List`" and the output entry fields (3138).
> They stay reachable through the generic property access. Please spell out every label and
> its type.
>
> *Answer (F49):* the §7.3 table and the §7.2 transmutation paragraph now spell out every
> label, its value type and, for enums, the mapper.

<a id="s7-4"></a>
### 7.4 Cosmetic companions (class 3698, 563 records)

Each has `CosmeticEntity_CosmeticEntity` (DID → the creature shown), `CosmeticEntity_Name_Initial`,
distance, movement type, following speed, movement thresholds and `Item_Quality` (562, all key 5).
They are reached from effects (`Mod_Array` → `CosmeticEntity_CosmeticPet_ForwardSource` in 562
effect records of class 734) and from the `CosmeticPetDirectory`, not from items directly; an
item summons a companion through its usage effect.

---

<a id="s8"></a>
## 8. The DID graph

<a id="s8-1"></a>
### 8.1 Out of items (Confirmed, all 151,230 property sets, nested values included)

3,607,800 non-null DID values; every one names an existing record (F18).

| Target prefix | References | Main properties (references) |
|---|---|---|
| 0x70 | 1,505,975 | `Mod_Progression` (289,361), `Usage_Action` (151,230), `Usage_Action_Shadow` (151,230), `Usage_TakePermission` (151,188), `Item_ValueLookupTable` (145,477), `Usage_Permission` (149,760), `CSM_ActionHandler` (105,092), `Item_RepairCostPerPointProgression` (87,020), `Item_Armor_Value_Lookup_Table` (60,916), `EffectGenerator_EffectID` (26,796), `Item_Disenchant_Component_Result` (26,345), `Combat_DPS_LUT` (23,204), `ItemMunging_ItemLevelOverrideProgression` (20,798), `Induction_ActionDID` (17,344), `Item_Socket_Operation` (15,416), `Item_TraceryUpgrade_Component_Result` (10,208), and about 70 smaller ones (§5) |
| 0x20 | 773,250 | `Item_WornAppearance` (634,655), `ExternalAppearanceID` (138,462), `Appearance_AprFile` (133) |
| 0x41 | 524,175 | icon layers (`icon-layers.md`), `ItemAdvancement_CombatPropertyMod*IconDID`, `PaperItem_SmallIcon`, `Hobby_Reward_Icon*`, `Item_Popup_Image` |
| 0x22 | 406,133 | the three tooltip layouts (§5.1) |
| 0x07 | 235,387 | `ScriptTable_DBScriptTable` inside `Entity_AdditionalScriptTables` (46 more are null) |
| 0x47 | 151,230 | `PhysObj` |
| 0x2A | 11,618 | `Usage_Sound` (11,148), `Hobby_Birding_BirdCall` (470) |
| 0x0C | 32 | `Door_Animation_*` |

Null DIDs are frequent and meaningful as "none" (for example `Induction_ActionDID` 107,732 nulls,
`Icon_Layer_UnderlayDID` 26,092, `Item_ValueLookupTable` 5,087).

Item-to-item references: `Item_Disenchant_Component_Result` (26,345), `Item_TraceryUpgrade_Component_Result`
(10,208), `PackageItem_PreviewItem` (1,366), `Item_PILI_Base_Weapon_WC` (181).

<a id="s8-2"></a>
### 8.2 Into items (Confirmed, all 383,332 property sets)

129,560 distinct items are named by some record. Largest sources (references; source root class):

| Source property path | Source class | References | Target kinds |
|---|---|---|---|
| `TreasureGroupProfile_ItemTable` → `…_Item` | 1235 (treasure groups) | 116,706 (class 1235 only; another 58 come from class 2372, 116,764 in all, §2.4) | 795, 802, 796, 799, 2814 |
| `Barter_ItemListArray` → `Barter_Item` | 1943 (barter lists) | 100,761 | 796, 795, 802, 805, 799 |
| `Vendor_ItemList` | 1724 (NPCs) | 91,274 | 796, 805, 795, 799, 2814 |
| `LootGen_TrophyList` → `…_ItemOrProfile` | 1237 (loot lists) | 55,074 | 796, 795, 802, 2814, 799 |
| `CraftRecipe_IngredientList`, `…_OptionalIngredientList`, `…_ResultItem`, `…_CriticalResultItem`, `…_MultiOutputArray`, `…_IngredientPack` | 1024 (recipes) | 58,493 in all | 796 mostly; results also 795, 799, 802, 797 |
| `VendorInventory_Items` | 1619 | 14,014 | |
| `Quest_ObjectiveArray` (several nested paths), `Quest_LootTable`, `Quest_GiveItemArray`, `Quest_RoleArray`, `Quest_BestowalRoles`, `Quest_PaperItemAwards` | 1398 (quests and deeds) | 24,548 | 796, 798 mostly |
| `Set_MemberList` | 1824 | 10,607 | 795, 802, 799, 796, 797 |
| `QuestTreasure_FixedItemArray`, `…_SelectableItemArray` | 1406 | 14,537 | |
| `HobbyRewardProfile_ItemArray` | 2153 | 2,689 | |
| `RewardTrack_Array` → `…_Reward` | 4224 | 2,466 | 2814 |
| `PrivateEncounterTemplate_PaperItemAwardArray` | 2651 | 1,823 | |
| `WebStoreInventoryItem_DataID` | 2541, 3054, 3052, 2532 | 1,335 | |
| `AdvTable_StartingInventory_List` | 3335, 853 | 327 | |

About 60 further source paths name items fewer than 600 times each (generators, monster play,
mail effects, housing control, guild control, paper-item control, skill material components,
legendary control, starting inventories). The loot, crafting, vendor and barter sources are
specified in `economy.md` §4–§6 (where loot tables are attached: §4.13); the quest sources in
`quests.md` §3.2.

---

<a id="s9"></a>
## 9. Algorithms

All arithmetic below is in IEEE-754 single precision unless a step says otherwise; tables are
read at run time.

<a id="s9-1"></a>
### 9.1 Classify a record

1. Read the root class (object-state offset 4).
2. Walk its ancestors (`wlib.md` §9, stop after the class count). If *I* (§2.4) is met, the
   record is an item; if only the parent of *I* is met, it is item-like.
3. Load the property set (+0x09000000). Kind-specific code keys off §2.1's signatures.

<a id="s9-2"></a>
### 9.2 Item value (Confirmed, F11)

Inputs: `Item_ValueLookupTable` *T*, `Item_Level` *L*, `Item_Quality` *Q*.

1. If *T* is absent or null, the value is 0. (All 5,087 null-table items store 0; the 666 items
   without the label store no value.)
2. Let *b* = the `Item_BaseVal` of element *L* of *T*'s `Item_BaseValArray` (1-based). *L* is
   between 1 and the array length (601) in all items.
3. Let *m* = the `Item_QualityModVal` of the `Item_QualityArray` element whose `Item_Quality`
   is *Q*. Every table has keys 1–5 and every item with a table has one of them.
4. *x* = *b* × *m*, computed as a single-precision product.
5. The value is *x* rounded to the nearest integer, **halves rounded up** (for example 6.5 → 7,
   687.5 → 688; 1,353 records have an exact half and all 1,353 round up).

Single precision matters: if the product is computed in double precision from the same
single-precision inputs and rounded half up, 533 items get a different integer, and the stored
value matches the single-precision result in all 533. Double precision with halves rounded to
even fails in 624 items. (An earlier draft gave 93 here; that count did not reproduce and is
withdrawn.)

**Client library form (Confirmed, BC, F34).** The client library's value function computes
f32(*b* × *m* + 0.5) and truncates it toward zero. This equals step 5 for every value below
2²³ (above it, adding 0.5 in single precision rounds to even); no stored value reaches 2²³, and
both forms reproduce all 145,518 values. It also: writes 0 when the item's quality key is 0 or
the table DID is 0; refuses (writes nothing, logs an error) when the level is 0 or negative,
when the item has no `Item_Quality` or no `Item_ValueLookupTable`, or when the level exceeds the
table's length (the table's own lookup would use its last element, `combat-data.md` §10.2, but
this function refuses first); a quality key the table lacks gives factor 0.0. The level is the
item's level or, for a scaling item, the scaled level of §5.2 (no distribution offset).

<a id="s9-3"></a>
### 9.3 Weapon DPS (Confirmed, F12)

Inputs: `Combat_DPS_LUT` *T*, level *L* = `ItemAdvancement_CombatDPSLevel` if present
(3,766 weapons) else `Item_Level`, quality *Q*.

1. *b* = element *L* (1-based) of *T*'s `Combat_BaseDPSArray`.
2. *m* = `Combat_DPSMod` of the `Combat_QualityModArray` element whose `Combat_Quality` is *Q*.
3. DPS = *b* × *m*, computed as a single-precision product. Stored `Combat_BaseDPS` equals
   this **exactly** in all 23,204 weapons. A double-precision product (not rounded to single
   precision) is exact in only 1,115, so a reader must round the product to single precision
   before comparing or storing.

Using `Item_Level` for the 3,766 weapons with a DPS level gives wrong values in all of them.

<a id="s9-4"></a>
### 9.4 Weapon damage (Confirmed relation, F13)

With DPS *d*, `Item_BaseActionDuration` *t* and `Combat_DamageVariance` *v*:

- stored `Combat_Damage` = *d* × *t* × 2 ÷ (2 − *v*) (within 1.2 × 10⁻⁷ relative, all 23,204);
- Reported (LC): this is the **maximum** damage per hit; the minimum is maximum × (1 − *v*).
  Then the mean of minimum and maximum is *d* × *t*: the damage per attack implied by the DPS.

<a id="s9-5"></a>
### 9.5 Armour value (Confirmed, F14)

Inputs: `Item_Armor_Value_Lookup_Table` *P* (class 3918 record) and `Item_Level` *L*.

1. Read `LinearInterpolatingProgression_Array`: control points
   { `LinearInterpolatingProgression_Key`: int, `LinearInterpolatingProgression_Value`: float },
   keys strictly ascending (1,814 of 1,814 records; 2 to 22 points).
2. Evaluate the progression at *L* as `combat-data.md` §4.4 and §4.10 describe (single
   precision, stored-slope form). Inside the keys this is linear interpolation between the two
   neighbouring points.
3. Truncate toward zero. All 60,880 stored values match, whether step 2 is computed in single
   precision with the slope form or in double precision (the values are non-negative, so
   truncation equals floor; rounding to nearest would fail in 18,324 items).

Among the 60,880 items checked above, the item level lies inside the keys (first key ≤ *L* ≤
last key) in 60,211, above the last key in 669 and below the first key in none. (Counting all
60,916 items that have a table, including the 36 without a stored armour value, gives 60,247
inside and the same 669 above.) In all 669 items above the last key the last segment is
flat, so clamping and extrapolation give the same value and the
data does not settle `combat-data.md` §4.8's OPEN.
Other uses of class-3918 progressions (`Mod_Progression`) are not confirmed to truncate
(§5.8 OPEN).

**Client library form (Confirmed, BC, F34).** The client library's armour function evaluates
the progression at *L* + the distribution level offset of the item's `Item_DistributionType`
(§6.4; 0 for every key here, so step 2 above is unaffected on this install), requires the
progression to represent `Item_Armor_Value`, and stores the result into the integer property,
which truncates toward zero (`combat-data.md` BIN b4). An item without a table keeps its stored
value. For a scaling item *L* is the scaled level of §5.2.

<a id="s9-6"></a>
### 9.6 Durability and structure points (Confirmed, F15)

- `Item_MaxStructurePoints` = the `…_MaxStructurePointsValue` of the wear-control entry whose
  enum key equals `Item_MaxStructurePointsEnum` (87,020 of 87,020).
- `Item_DurabilityValue` = the `…_DurabilityValue` of the entry whose key equals
  `Item_DurabilityEnum` (90,932 of 90,932).
- A new item instance starts with current structure points = maximum (the static copy, §4.3).

<a id="s9-7"></a>
### 9.7 Weapon timing (Confirmed, F16)

Find the timing table (§6.3) whose `Item_EquipmentCategory` equals the weapon's; in it, the
entry whose `Item_WeaponSpeed` equals `Combat_WeaponSpeed`. Its duration and animation modifier
equal the weapon's stored `Item_BaseActionDuration` and `Item_BaseAnimDurationMultiplierMod`
(23,204 of 23,204).

<a id="s9-8"></a>
### 9.8 Cooldown and lifetime in seconds (Confirmed coverage, F17)

Every key of `Usage_CooldownDuration` (7,229 items) and `Usage_EquipCooldownDuration`
(85,754; key 0 = none) used by an item is either 0 or a row of the cooldown control map;
every `ItemDecay_Duration` key used (249) is 0 or a row of the decay control map. The
duration is the row's float, in seconds (unit Reported: the enum labels name the same spans,
for example a key labelled one second maps to 1.0).

<a id="s9-9"></a>
### 9.9 Set membership (Confirmed)

1. Read the `GameSetDirectory` list.
2. For each set of the item-set class (1824 here; not the trait-set sibling), record its
   `Set_MemberList` DIDs.
3. Add every item whose `Item_PropertySet` names the set.
4. The reverse index item → set is a function: no item belongs to two sets.

---

<a id="s10"></a>
## 10. Explorer item card

The item card shows one item record. Rows appear only when their sources exist.

| Row | Source | Notes |
|---|---|---|
| Header | `Name` (StringInfo, `string-table.md` §9); composed icon (`icon-layers.md` §3); DID, root class, kind label (§2.1) | |
| Category | `Item_Class` display text (mapper 0x23000036 display list, `property-system.md` §7.1); `Item_EquipmentCategory` names | |
| Quality | `Item_Quality` display text | No colour is derived (`icon-layers.md` §4) |
| Level | `Item_Level`; `Usage_MinLevel`–`Usage_MaxLevel`; munging fields raw (§5.2) | |
| Binding and limits | §5.4 table reading; `Inventory_Unique`, `Inventory_UniqueEquipped`; stack size `Inventory_MaxQuantity` | |
| Value | `Item_Value` as gold/silver/copper (F28) and the recomputed value (§9.2), flagged if they differ | |
| Durability | `Item_CurStructurePoints` / `Item_MaxStructurePoints`; `Item_DurabilityEnum` display text | |
| Armour | `Item_Armor_Value` (and recomputed, §9.5) | |
| Weapon | DPS (§9.3), damage range (§9.4, marked Reported), `Combat_DamageType` display text, speed | |
| Stats | Each `Mod_Entry`: modified property label, operation label, constant value or progression DID (evaluation marked OPEN, §5.8) | |
| Effects | Each effect struct of the four lists: list name, effect DID and the effect record's `Effect_Name` | `combat-data.md` §6 |
| Sockets | `Item_Socket_Array` types and unlock levels; for socketables, the accepted socket type and set | |
| Set | Set name (via §9.9), tier counts, member count | |
| Legendary | §5.12 labels present | |
| Description | `Description` | |
| Visuals | worn/held preview (`appearance.md` §8) | |
| Links | Every outbound DID of §8.1 (clickable) and the inbound references of §8.2 (a reverse index built once per session) | |
| Raw | Every property, grouped as in §4 (stamps and copies collapsed by default) | |

---

<a id="s11"></a>
## 11. Enum mappers used by item properties (Confirmed)

Labels in the third column are the EMAPPER DID-map labels (`property-system.md` §8.1). Mapper
DIDs are this install's; resolve them through each property definition's `data` field.

| Property | Mapper (here) | EMAPPER label | Names / display texts |
|---|---|---|---|
| `Item_Class` | 0x23000036 | `ItemClass` | 279 / 278 |
| `Item_EquipmentCategory` (bitfield) | 0x2300004C | `EquipmentCategory` | 42 / 41 |
| `Item_Quality` | 0x23000067 | `ItemQualities` | 6 / 5 (key 0 has no text) |
| `Item_Socket_Type` (bitfield) | 0x23000432 | `ItemSocketType` | 25 / 24 |
| `Combat_WeaponSpeed`, `Item_WeaponSpeed` | 0x23000066 | `WeaponSpeedType` | 7 / 0 |
| `Combat_DamageType` | 0x23000028 | `DamageType` | 19 / 15 |
| `WeenieType` | 0x230000C9 (base 0x230001AB) | `WeenieType` | 38 own / 0 |
| `Inventory_CompatibleSlot`, `_DefaultSlot`, `_PrecludedSlot` (bitfields) | 0x230000EE (base 0x23000101) | `ContainerSlot` | 30 own / 21 |
| `Item_DistributionType` | 0x2300028C | `ItemDistributionType` | 16 / 0 |
| `Item_DurabilityEnum` | 0x230000FA | | 8 / 7 |
| `Item_MaxStructurePointsEnum` | 0x230000F9 | | 51 / 0 |
| `Item_WearState` | 0x230000FB | | 5 / 0 |
| `Usage_CooldownDuration` | 0x2300006D | `CooldownDuration` | 64 / 0 |
| `Usage_CooldownChannel` | 0x2300006C | `CooldownChannel` | |
| `Mod_Op` | 0x23000046 | | 12 / 0 |
| `Auction_CategoryType` | 0x230001D1 | | 53 / 52 |
| `Item_Client_Sort_Type` | 0x23000317 | | 9 / 0 |
| `Item_MaterialType` | 0x230000F3 | | 54 / 0 |

The EMAPPER label `ClothingClassType` names mapper 0x23000015 (6 names), but **no property
definition uses it**; it plays no part in items on this install.

---

<a id="s12"></a>
## 12. Invariants (all Confirmed on this install)

| # | Invariant |
|---|---|
| IT1 | Every record whose root class descends from *I* has a property set that decodes, and that set has `Item_Class`, `WeenieType`, `Name`, `PhysObj`, `Usage_Action` and `Icon_Layer_BackgroundDID`. |
| IT2 | The sixteen labels of §4.2 are present, with one value each, in exactly the records under ancestor 1733. |
| IT3 | Each copy of §4.3 equals its source. |
| IT4 | Non-null `Item_ValueLookupTable` ⇒ 1 ≤ `Item_Level` ≤ length of the base array, the table has the item's quality key, and `Item_Value` follows §9.2. Null table ⇒ `Item_Value` = 0. |
| IT5 | Every weapon has `Combat_DPS_LUT`; §9.3 holds exactly (single-precision product equals the stored `Combat_BaseDPS` bit for bit), and §9.4 holds within 3 × 10⁻⁷ relative. |
| IT6 | `Item_Armor_Value` with a lookup table follows §9.5 exactly. |
| IT7 | §9.6 and §9.7 hold exactly. |
| IT8 | Every non-null DID in an item property set names an existing record. |
| IT9 | Every `Mod_Entry` has `Mod_Modified` and `Mod_Op`, and has `Mod_Progression` or a field whose PID equals `Mod_Modified`. |
| IT10 | `Item_Socket_Array` entries have exactly the two fields of §5.11 and one socket-type bit. |
| IT11 | `Set_ActiveCount` values within a set are strictly ascending; no item appears in two `Set_MemberList`s; every member of a class-1824 set is an item. |
| IT12 | For every item whose object state holds the worn map (root attribute 0x0905B9D8 non-null: here all class-795 records and the 200 weapons with the list), the map equals `Item_WornAppearanceMapList` (§3.3). |
| IT13 | The targets of `Usage_Permission`, `Usage_TakePermission` and `CSM_ActionHandler` are each the only 0x70 record of their root class, and their property sets hold at most six properties. A 0x76 singleton record of the same class (`wstate.md` §3; classes 1603 and 1547 here) is not counted (F50). |
| IT14 | In every item's object state, every class-700 element's effect DID is an effect of the item's own `EffectGenerator_*` lists without floor or ceiling, the `PropertyModEffect` effect, or a default effect of the item's equipment category; every own unranged effect has an element; items with `Mod_Array` have exactly one `PropertyModEffect` element, with spellcraft −1 (§3.2). |
| IT15 | No effect struct with a floor or ceiling other than −1 has an element in the item's compiled list (§5.9). |
| IT16 | `Item_Disenchant_Value` > 0 exactly when `Item_Disenchant_Component_Result` is set (§5.3). |
| IT17 | Every socketable of an essence kind has `Item_Socket_GemMinLevel` = `Item_Socket_GemMaxLevel` or neither; every tracery has both with minimum < maximum (§5.11). |
| IT18 | Every `WeenieType` value's low 16 bits are a major type of §2.3, except at most the single `AIRemoteDetector` record. |
| IT19 | In every set without a member list, every `Set_ActiveCount` is at most `Set_MaxActiveMembers` (§7.1). |

> **Answered (2026-10-09; was OPEN, implementer 2026-10-07):** IT13 holds only when "record" means a 0x70 record. Counted
> over all object DIDs (0x70–0x77), the classes of 2 of the 24 targets also hold a 0x76
> singleton record. The real-data test counts 0x70 records. Please say which count IT13 and
> §5.10 mean.
>
> *Answer (F50):* 0x70 records. IT13 now says so; the two classes are 1603 (the target
> labelled `UsagePermission_Default`) and 1547 (`TakePermission_CantTake`), whose 0x76
> singletons have no property set. §5.10's "only record of their root class" means the same.

A real-data test asserts IT1–IT19 and reports counts; it never asserts the counts themselves,
which change with client updates.

---

<a id="s13"></a>
## 13. Worked synthetic examples (for tests)

All numbers are invented for the examples; none is taken from the install.

**13.1 Classification.** A synthetic library has classes 10 (no parent), 20 (parent 10),
25 (parent 20), 30 (parent 25), 31 (parent 25), 40 (parent 30) and 50 (parent 20). Vendor,
barter and treasure lists name 900 records of class 40, 95 of class 31 and 5 of class 50. The
deepest common ancestor-or-self of {40, 31, 50} is *P* = 20. Its children are 25 (995
references in its subtree) and 50 (5), so *I* = 25. A record of class 30 is an item; a record
of class 50 is item-like but not an item; a record of class 10 is neither.

**13.2 Value.** Table: base values 2.0, 4.0, 6.5 at levels 1–3; quality factors key 5 → 1.0,
key 4 → 1.25. An item at level 3, quality 5: 6.5 × 1.0 = 6.5 → **7** (half rounds up). Level
3, quality 4: 6.5 × 1.25 = 8.125 → **8**. Level 2, quality 4: 4.0 × 1.25 = 5.0 → **5**. A null
table → **0**.

**13.3 DPS and damage.** DPS table: level 10 → 20.0; quality key 2 → 1.04. A weapon at item
level 12 with `ItemAdvancement_CombatDPSLevel` 10, quality 2: DPS = 20.0 × 1.04 = **20.8** (stored as the single-precision value nearest 20.8).
With duration 1.0 and variance 0.25: `Combat_Damage` = 20.8 × 1.0 × 2 ÷ 1.75 = **23.771429**;
Reported reading: minimum = 23.771429 × 0.75 = 17.828571; mean = 20.8.

**13.4 Armour.** Control points (1, 10.0), (11, 30.0), (21, 30.0). Level 6 → 20.0 → **20**;
level 7 → 22.0 → **22**; level 4 → 16.0 → **16**; level 25 → **30**. With points (1, 2.0),
(5, 5.0): level 2 → 2.75 → **2**.

**13.5 Mod entry.** A struct { `Mod_Modified` = PID 0x10000123 (a synthetic stat),
`Mod_Op` = 7, PID 0x10000123 = 15 }: a constant +15 (operation label from the mapper). A
struct { `Mod_Modified` = 0x10000123, `Mod_Op` = 7, `Mod_Progression` = 0x70001000 }: value
from that progression (§5.8 OPEN).

**13.6 Binding.** (true, true, false) → account-bound on acquire; (false, absent, true) →
bind on equip; all absent → unbound (§5.4).

**13.7 Set membership.** Set A lists items 1, 2, 3. Set B has no list; items 7 and 8 have
`Item_PropertySet` = B. Members: A = {1, 2, 3}, B = {7, 8}. Reverse index: 1, 2, 3 → A;
7, 8 → B.

**13.8 Level scaling, case 1.** An item with override property `Item_Level`, min 10, max 50,
bonus chance 0.25, limit 2. Requested *L* = 60 → clamped to 50. Random draws 0.1 then 0.7: the
first is ≤ 0.25 (51), the second is not (stop): `Item_Level` 51. With draws 0.2, 0.2: 52 (the
limit stops it). Requested *L* = 5 → 10 before the bonus.

**13.9 Level scaling, case 2.** Override property `Advancement_Level`, min 20, max 100,
progression *P* with *P*(30) = 44, no bonus labels, player level cap 120, empty *W*. *L* = 30:
*x* = 30; `Usage_MinLevel` 30 written; *x* = *P*(30) = 44; `Item_Level` 44 and
`Item_Level_Calculated` true written. *L* = 150: *x* = min(150, 120) = 120 → capped at max 100.
*L* = 10: *x* = 10 < min 20 → the rule fails, nothing written.

**13.10 Set tier and level.** A set lists members A, B, C (three members) and has tiers at counts
2 and 3, `Set_Level` −1, `ItemGameSet_UseAverageItemLevelForSetLevel` true. Active: A at item
level 50, C at 60. Count 2 → the count-2 tier, set level (60 + 50) ÷ 2 = 55.0. A tier lookup for
count 4 returns none (4 > 3 members). With a member-less set whose count property holds 3 and
level property holds 150: count 3, set level 150 ÷ 3 = 50.0.

**13.11 Repair and sale.** Cost-per-point progression gives 2.0 at the item's level, quality
factor 1.5, discount factor 0.75 (one discount property with value 0.25), repairer cost
modifier 1.0, structure 100 / 40: in single precision 2.0 × 1.5 = 3.0, × 0.75 = 2.25, × 1.0 =
2.25, × 60.0 = 135.0 → **135** (every step exact in binary). Same item, repairer without
`Repair_CostModifier` (−1.0): → **−135**. Same item, discount factor 0.9: 3.0 × 0.9 =
2.6999998 in single precision (0.9 is not exact in binary), × 1.0, × 60.0 = 161.99998, truncated
→ **161**, not the 162 of exact decimal arithmetic. Sale: quantity 3, `Item_Value` 7, wear-state factor
0.5, item subject to wear: 21 × 0.5 = 10.5 → **10**; with `Item_Value` 0 → **0**; with
quantity 1, value 1, factor 0.5 → 0.5 truncates to 0 → **1**.

---

<a id="s14"></a>
## 14. Open questions

**Settled on 2026-10-08** (the former callouts of this section; details in the sections named):

- *Who applies the rules of §9.* The client library's bytecode computes them: item value,
  armour, DPS, weapon damage and timing are recomputed by the client library when it scales an
  item (§5.2, §9.2, §9.5, `combat-data.md` §10.2); the native client does not (F30). The
  server-side library is not in the archives, so whether the server computes them the same way
  is not in the data. For Tessera the rules remain data relations: a reader may recompute or
  trust the stored values; they agree everywhere.
- *Dictionary defaults of absent properties* (F43). The client library does not apply
  dictionary defaults as a rule: every script read of a property passes its own default, and the
  native lookup reports an absent property as not found (BIN2). At 1,045 of the 1,104 read sites
  with a constant default that default equals the dictionary default; the 59 others include, for
  items, `Item_Disenchant_Value` (1, dictionary 0), `Item_Socket_GemMinLevel` and
  `Item_Socket_GemMaxLevel` (0 at the socket-system sites, dictionary −1),
  `Item_CurStructurePoints` (−1, no dictionary default), `Inventory_Quantity` (1, none) and
  `Item_TraceryUpgrade_Value` (1, dictionary 0). **Decision (Tessera):** use the default this
  spec gives for a property where it gives one (§5, §6.3, §7.1); otherwise use the dictionary
  default. The same answer applies to `mesh.md` §8.5.
- *F21 parts:* spellcraft −1 means "not given" and is passed through (§5.9); the floor/ceiling
  pair is the item-level window of a level-dependent effect (§5.9, F35); modifier arithmetic and
  combination order are `combat-data.md` §5.3–§5.6. Only the level input remains (below).
- *Level scaling and `Item_iLevel_Crafting_Offset`* (§5.2, F33); `Item_LevelScaling_OnLevelForPlayersBelowLevel` (F42).
- *Set rules of the client:* count, tier per count, tiers above the member count, member-less
  sets, set level, level gate (§7.1, F41). Server activation remains (below).
- *Item flags:* `Inventory_MaxQuantity` = 0 (§5.4); `Item_Disenchant_Value` −1 versus 0 (§5.3,
  F39); `Barter_IsBarterItem` and `Inventory_Indestructible` (§5.3, §5.4, F38); the (false,
  true, true) binding combination (§5.4, F40).
- *Wear tables:* repair-cost and wear-state tables; the skill-gate table is unused by the client
  (§6.3, F37).
- *`WeenieType` bit structure* (§2.3, F31); *object-state scaffold* (§3.2, F32);
  *`Item_ImplementType`* (§5.1, F44).
- *Socket lock, socket kinds and gem level windows* (§5.11, F36).

**Still open:**

> **OPEN (F21):** The spellcraft the server passes when it applies an item's `PropertyModEffect`
> element, and so the level that indexes the item's modifier progressions and level ranges
> (§5.8). Server behaviour; owner observation of the original client settles it (§5.8 gives the
> test).

> **OPEN:** Set bonus activation at the server: cumulative or exact-count tiers, and tiers above
> the member count (§7.1). Owner observation settles it (§7.1 gives the test).

> **OPEN:** Legendary rules (legacy rolls, pull counts, imbuement, reforging) and the
> `Item_PILI_*` group (§5.12, §7.3). Not in the client library; server behaviour.

> **OPEN:** Labels the client library never reads (no 32-bit literal equal to their PID in the
> bytecode), so their use is server behaviour or native code not traced: `Inventory_UseOnAcquire`,
> `Inventory_QuestItem`, `Quest_IsQuestToken`, `Effect_IsEnchantable`, `ExternalAppearanceID`,
> `Item_MaterialType`, `UI_Examination_Tooltip_Compare_Type`, the three `Combat_*Stance`
> labels (F44). Labels it reads without the effect being traced here:
> `Inventory_CanBeContained` (container functions of classes 368/2804, 1534, 1541),
> `Item_MithrilCost` (a getter of class 3649), `SpellcraftCalculator` (classes 3177, 1494). A
> further reading of those bodies (method M1) settles the second group. **Tried (2026-10-09,
> F52):** the reads are getters (`Inventory_CanBeContained`: one body shared by classes
> 368/2804, default false; `Item_MithrilCost`: classes 3649/3650, default 0;
> `SpellcraftCalculator`: classes 3175/3177/3197 and the ranked-trait function of the class-1494
> family). Their callers (classes 2804, 1534, 1541; 3704 and the class-3937 family; 1062 and
> the 1494 family) pass the results to native functions whose behaviour `wlib-bytecode.md` §7
> does not specify, so the effect is still not readable from the library alone; a BIN pass
> on those natives would settle it.

> **OPEN:** Use of the separate merge list of `InventoryControl` (§6.4); the examination list is
> settled (F53).

---

<a id="s15"></a>
## 15. Notes for other specs (not edited here)

- `appearance.md` §4.1: the worn item counts by `WeenieType` (41,871 / 200 / 15) agree with §2.1
  and §3.3 here; the object-state worn map (§3.3) is a mirror and need not be read.
- `wstate.md` §11.1 lists identification by property signature for areas; items follow the same
  principle with the class-ancestry rule of §2.4.
- `icon-layers.md` §2.2 counts 151,977 sets with a background: 151,230 items + 706 relics + 41
  carry-alls (§2.3 here).
- `mesh.md` §8.5 OPEN (dictionary defaults of absent properties) is the question settled in
  §14 (F43): the client library supplies a default per read site; the native lookup does not
  substitute the dictionary default.
- `interaction.md` §4.3 (compiled effect list of hotspot templates) describes the same class-701
  structure that items carry (§3.2); items use multimap keys 16 and 32.
- `combat-data.md` §10.2 states the value and DPS table lookups; §9.2 and §9.5 here add the
  item-level checks, the distribution offset and the scaling path of the client library.

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (Review). Census over all 151,230 item records and all 383,332 property sets; derived-value relations verified against every item; one narrow BIN pass with a negative result. |
| 2026-10-07 | Review fixes, every count re-measured over all records: §9.2 double-precision disagreement is 533 items (half up; 624 half to even), not 93; F12, §9.3 step 3, IT5 and §13.3 state exact equality of the single-precision DPS product (23,204 of 23,204; double precision exact in 1,115); §2.4 gives references per source (91,274 / 100,761 / 116,764 = 308,799; 308,778 under 1734, 21 barter references to carry-alls); §8.2 treasure row says it counts class 1235 only (58 more from class 2372); §9.5 counts the 60,880 checked items (60,211 inside, 669 above, 0 below) and gives the 60,916-item counts separately. Status stays Review. |
| 2026-10-08 | Revision (status Revised). OPEN items closed from the client library's bytecode (new source BC, method M1) and a second BIN pass on natives (BIN2), every data count re-measured over all records: §2.3 `WeenieType` bit structure (F31); §3.2 the class-701 object is the compiled static effect list (F32, IT14); §5.2 level-scaling rule, crafting preview offset, scaled characters (F33, F42); §5.3 vendor rules, `Item_Disenchant_Value` (F38, F39, IT16); §5.4 `Inventory_MaxQuantity` = 0, gem uniqueness, binding target, (false, true, true) row (F38, F40); §5.5 wear subject and repair-cost progression; §5.8 how modifiers are attached, F21 narrowed to the server's spellcraft; §5.9 spellcraft −1 and level-dependent effect windows (F35, IT15); §5.11 socket lock, type and level rules, the withdrawn "500 / 499" example (F36, IT17); §6.3 repair-cost and wear-state tables, skill-gate table unused (F37); §6.4 distribution offset role, exam/merge OPEN made precise; §7.1 client set rules, OPEN narrowed to server activation (F41, IT19); §7.3 legendary OPEN made precise; §9.2 and §9.5 client library forms (F34); §14 rewritten (settled list, five remaining callouts); invariants IT14–IT19; examples 13.8–13.11; provenance keys BC and BIN2, rows F31–F44. |
| 2026-10-08 | Review fixes (status stays Revised): §13.11 repair example now uses factors exact in binary (discount 0.75 → 135), adds the −1.0 modifier case (−135) and the 0.9 case that truncates to 161 in single precision; §6.3 states the precision of the repair cost (single-precision factors, wrapping 32-bit difference converted as unsigned, out-of-range truncation), the current > maximum case and that the negative cost of a repairer without `Repair_CostModifier` is intended behaviour; §6.3 sale value states the integer product converted to single precision; F20 says 289,349 + 12 = 289,361; F37 row widened. |
| 2026-10-09 | OPEN sweep (spec writer, dirty room; status Revised, waiting for review). New provenance key OBS3 (every set of the install walked at every depth; dictionary types) and rows F45–F53. Implementer OPENs answered: §2.4 tie rule and kind order (Decision F51), §2.1 signatures of classes 1722, 2097, 3924 (F45), §3.3 and IT12 weapons carry the worn mirror (F46; the earlier sentence was wrong), §5.7 requirement types (F47), §6.3 wear-control entry labels and §6.4 decay entry labels (F48), §7.2/§7.3 labels and types of classes 2134, 2139, 2338, 4015, 3138, `Runic_Tier`, `ItemAdvancement_Mod_Weight` (F49), IT13 counts 0x70 records (F50). §6.4 examination list: compiled form and its reader found (F53). §14 unread-label callout: the getters and callers were located (F52), effect still OPEN. Still OPEN: F21 spellcraft, server set activation, legendary rules, the unread labels, the separate merge list (the examination list is settled, F53; all others server behaviour or native code). |
| 2026-10-08 | Merge of the decoder branch (spec writer, dirty room). The implementer OPEN notes and census differences of 2026-10-07 are kept; those that the Accepted text of the 2026-10-08 revision already answers are marked with a pointer: §5.10 (`Usage_Operator` type: `interaction.md` §3.7.1), §6.3 (repair-cost and wear-state keys and factors: Roles paragraphs) and §6.4 (cooldown entry labels: `economy.md` §11.5.3), each answered in part; IT13 note kept next to the IT1–IT19 sentence. No fact changed; status unchanged. |
