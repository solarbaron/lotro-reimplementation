<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Item tooltips: examination layouts, row content, colours, comparison, placement

| | |
|---|---|
| **Status** | Revised (P2 revision 2026-10-10 of §6.16, §6.20 and §8.3, with rows IT23–IT24, Q10 and Q21; awaiting independent review; the rest was Accepted 2026-10-09 after re-review, see the review record) |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted), not the author |
| **Review record** | First review, 2026-10-09 (independent review session that wrote no part of this spec; whole spec, new file): **changes requested**, status stays Review. Checked against CLEAN_ROOM.md §5.3: no code or pseudo-code (§5, §6.4, §8.2 are plain steps); no copied prose; no game text (format strings are cited by token and variable ID only); labels and property names are data identifiers needed for interoperability; BIN use is within decision 0011 (offline client UI, no network or login code). Spot checks, read-only against the install-derived dumps: item census of §3.1–§3.2 and §8.3 (151,230 items; 149,695 hover pairs; 1,535 without, all class 796; 148,979 one-click; 150,507 equal pairs; 107,479 compare pairs, 20 zero; layouts 149,684 / 11; all 25 hover-element counts; compare kinds 101,400 / 6,055 / 1,557 / 1,262 / 378 / 20 / 40,558) match; the six name colours and the 32 states of `ColorNameText` (§6.2) match; row-template colours, formats, sizes, margins and states of `Name_Base`, `ArmorValue_Base`, `ItemUniqueness_Base`, `ItemDecay`, `Usage_Cooldown`, `Description_Base`, `ExaminationItemLevel_Base`, `Value_Base`, `Separator_Short_Base` match; all cited `Examination` and `Utils` tokens exist with the stated variable IDs; IV4 holds; worked examples §14.1, §14.4–§14.6 recomputed; binding combinations agree with `items.md` §5.4. **Blocking:** (1) §2 "Tooltip element", §4.1, IT4, IV2, IV3 are stated for every `*_ExamineLayout` element of the examination layout, but that layout has 79 such top-level elements; the stated structure (base `Tooltip_New`, one `ExaminationListBox` child with autofill, entry templates naming top-level elements of the same layout) holds for the 30 elements item records name, not for all 79 (other bases occur, one element fails IV2, one entry template names an element of another layout); and the "330 in one world-event layout" width of §4.1 belongs to an element no item names. Restate the scope (the elements named by item records) or list the exceptions, and make IV2/IV3 testable as scoped. (2) §6.7 step 3: the `Utils` pair format 0x040CF382 has two variables (0x5425, 0x59BF); the spec does not say which receives the level text and which the marker. State it, or add an OPEN with a Tessera rule. (3) §6.4 step 7: the bitfield row cites "the client's list separator (§6.14 OPEN)", but the §6.14 OPEN does not cover a separator; and the "other → the value's generic text" row is undefined. Give the fact or add an OPEN with a Tessera rule for each. (4) Provenance: §6.3 and §6.12–§6.18 state class-library facts (provider classes 1067, 1058, 1059, 1065, 1055, 1092, 1062, 1091, 2808 and the quest-ID provider; row-data properties such as `Examination_DPS`, `Examination_IsGhosted`; source properties such as `Usage_CooldownDuration`, `ItemDecay_Duration`, `Item_EquipmentCategory`), yet IT18 marks those sections Design only, and classes 1062, 1067 and the quest-ID class are missing from the BC source list. Add a BC row (Confirmed, bytecode) for the traced parts, extend the BC list, and keep IT18 for the Tessera rules. **Non-blocking notes:** §2 "for all but 11 items" should read "all but 11 of the 149,695 items with a hover pair"; §6.11 "32-bit product" should state signedness and overflow; §9 "Probable" is outside the provenance vocabulary (cite or mark OPEN); IV7 presumes a currency with `Currency_Value` 1, state that precondition; the OBS row dates the item dump 2026-10-07, check it against `items.md` OBS3. Review fixes 2026-10-09 (spec writer, dirty room): status stays Review (the spec was never Accepted) until a separate session re-reviews it. (1) Scope restated to the 31 elements item records name (30 in the examination layout, one in 0x220008BE): §2, §4.1, IT4, IV2–IV4; the other 49 `*_ExamineLayout` elements are listed as out of scope with their exceptions; the widths of §4.1 corrected (24 named list boxes carry `max_width` 229, seven carry none; new OPEN with a Tessera rule); row IT19. (2) §6.7 step 3: variable 0x5425 receives the level text, 0x59BF the marker (BC, class-library helper), row IT20. (3) §6.4 step 7: the bitfield join (`Utils` token 0x06756762, ascending bit order, failure on an unlabelled bit) and the "other" types (variable left unset, so the row is empty) are now stated from BIN, row IT21; the any-width bit order is an OPEN with a Tessera rule. (4) New row IT22 (BC, Confirmed) for the class-library parts of §6.3 and §6.12–§6.18, BC source list extended by classes 1062, 1067, 1074; IT18 keeps only the Tessera rules; §6.18 names `Quest_DataID` and the row data of the cooldown and decay providers. Non-blocking notes applied: §2 wording, §6.11 signed 32-bit wrapping product, §9 "Probable" replaced by an OPEN with a Tessera rule, IV7 precondition; the OBS item-dump date (2026-10-07) agrees with `items.md` OBS3. Re-review 2026-10-09 (separate dirty-room session, neither the author nor the first reviewer; whole spec, new file, with the review fixes): **passed**, Accepted. Checklist §5.3 met: every statement is a functional fact or a marked Tessera rule; no code or pseudo-code (§5, §6.4, §6.7, §8.2 are plain steps, not listing-shaped); no copied prose; no game text (format strings by token and variable ID only; labels and property names are data identifiers needed for interoperability); every fact has a provenance row (IT1–IT22) with a mark; every unknown is an OPEN with a Tessera rule (Q1–Q20); BIN within decision 0011 (offline client UI only). All four blocking items of the first review are fixed. Spot checks, read-only, with an independent probe over the layout dumps and all 151,230 item records: 31 distinct non-zero (layout, element) pairs named by item records (30 in 0x22000000, `KinshipCharter_ExamineLayout` in 0x220008BE); 79 `*_ExamineLayout` top-level elements in the examination layout (bases `Tooltip_New` 66, `Tooltip_Bare` 11, `Tooltip` 1, none 1); IV2 31 of 31; IV3 784 of 784 (783 in the examination layout, one in 0x220008BE); entry-template counts 1–55; `max_width` 229 in 24 named list boxes and absent in exactly the seven listed (designed 229 and 304, at (5, 5)); the 49 other elements: one with two children, list-box widths 229, 231, 330, 335, 390, 2048 and one absent; IV4 seven of seven; the bitfield generic rows reachable from the 31 elements are exactly the three named, with the stated widths (types 14 and 18); `Utils` 0x040CF382 and 0x06756762 each carry variables 0x5425, 0x59BF, the yes/no tokens none, and the cited `Examination` item-level and quest-ID tokens the stated variables; IT20 against the class-3796 body and the pair helper it calls (first text argument to 0x5425, marker to 0x59BF, an empty side yields the other); IT21 join against the BIN listing of the list-join helper (both non-empty: pair format; one empty: the other); IT22 properties of classes 1058, 1059, 1062, 1065, 1067, 1074, 1091, 2808 as stated. **Non-blocking notes:** IV4 and IV5 say "Confirmed" without a date; §6.7 could say that the provider tests the examined object against class 801 before anything else (the "every item" scope); §6.4 bitfield text "no bit is set" differs from the debug display of `property-system.md` E3 (key-0 name), which the spec already separates, a cross-reference in E3 would help readers. |
| **Implements into** | `crates/gamedata` (`tessera-gamedata`: the tooltip model of an item record, §3–§8, and the row values of §6), `crates/ui` (the examination tooltip panel: row list assembly §5, placement §9, pinned tooltips §10), `apps/client` (hover over item slots in bags, the paperdoll and plugin item controls), `apps/explorer` (tooltip preview on the item card), the real-data test of decision 0024 P1 exit criterion 2 (§12) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (layout records, elements, states, inheritance, the `UILAYOUT` DID map), [`ui-behaviour.md`](ui-behaviour.md) §6 (generic tooltips; this spec answers the "item and examination tooltips" part of its §6.6 OPEN), [`items.md`](items.md) (item properties, value, armour, DPS, damage, binding, sets), [`icon-layers.md`](icon-layers.md) (the composed icon), [`string-table.md`](string-table.md) (StringInfo and format-string variables), [`property-system.md`](property-system.md) (value types, enum mappers, bit ↔ key), [`records.md`](records.md) §8 (DID maps), [`economy.md`](economy.md) §7 (currencies), [`combat-data.md`](combat-data.md) (effects behind stat lines), [`wlib-bytecode.md`](wlib-bytecode.md) (the class library's code), [`scenario-feed.md`](scenario-feed.md) (Accepted on branch `p0`, 2026-10-09; how a scenario supplies items, §11), [`character-panels.md`](character-panels.md) (bags and paperdoll that show these tooltips), [`lua-plugin-api.md`](lua-plugin-api.md) §15 (`ItemInfo`) |

---

<a id="s0"></a>
## 0. Purpose

When the pointer rests on an item (in a bag, on the paperdoll, on a quickslot, in a vendor
or loot list, on a chat link), the original client shows an **examination tooltip**: a
framed list of rows (name and icon, binding, item level, armour or damage, stat lines,
durability, requirements, set members, description, worth, …). The rows, their order, fonts,
colours, format strings and most texts come from data; the client's code decides which rows
have content, fills them and assembles the list. This spec lets an implementer:

- find the tooltip layout of every item record (§3);
- build the row list from the layout data (§4) and assemble it as the client does (§5);
- fill each row the item records need, with its source property, format string, colour and
  visibility rule (§6, §7);
- show comparison tooltips for equipped items (§8);
- place, hide and pin tooltips (§9, §10);
- drive all of it from the scenario feed in the sandbox (§11) and test it against every item
  record of an install (§12).

Values given for this install (DIDs, element and state IDs, colours, counts) are
**observations**. Implementations read layouts, element labels, state labels, string tables
and properties at run time and never hard-code them; the numbers here are for tests and for
recognising the data.

Out of scope: tooltips of skills, effects, traits and quests (decision 0024 P2, P3), the
legendary-item rows (§6.20), and the server's rules that decide per-instance values
(binding, crafter, item-level scaling of loot). Where a row needs a value only a server
supplies, §11 says what Tessera shows.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (Steam, client build 4906.0071.1794.4035), read-only, in place. The archives are unchanged since 2026-10-04 (file times re-checked 2026-10-09), so the dumps made from them stand for the install: all 302 UI layout records (prefix 0x22, decoded per `ui-layout.md` §4 with zero leftover bytes, dump of 2026-10-06, re-read with a throwaway probe that decodes property values on 2026-10-09); the property dictionary and the enum mappers (`UIElementID` mapper 0x230000D1 with its base, the state mapper, `UIElementType`); the DID map 0x28000004 (named string tables); every item record (all 151,230, dump of 2026-10-07, `items.md` OBS3) for the census of §3. String tables 0x250001AF (`Examination` in 0x28000004), 0x250001E6 (`Utils`) and 0x250001BD were read for **tokens and variable IDs only**; no text is reproduced here. |
| **BC** | The class library's code (record 0x56000000), read 2026-10-09 with the repository's `tessera-dattool bytecode` disassembler (`wlib-bytecode.md`, Accepted) and a throwaway probe over `tessera-wstate` for class method tables (function ID rule, `wlib-bytecode.md` §6.4). Data in the user's install (method M1). Classes read (indices of this install): the examination-element base class 1097 and the item-relevant children 1055 (allowed slots), 1058 (DPS), 1059 (damage), 1060 (durability), 1062 (effect lists), 1065 (equipment category), 1066 (generic property), 1067 (icon), 1068 (indestructible), 1073 (name), 1074 (quest ID), 1090 (unique), 1091 (cooldown), 1092 (usage requirements), 1093 (value), 2808 (item decay), 3707 (armour), 3796 (item level), with the text-pair helper the item-level provider calls (re-read 2026-10-09 for the review fixes); the binding class 2779 and its child 2781; the equipment slot query of class 372; the money breakdown of class 404; the sellability test (`items.md` §5.3, F38). |
| **BIN** | **M4 under the standing authorisation of decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`), headless decompiler on a private copy of the shared analysis project (`~/tessera-re/work/specs13-p1`, deleted after use), 2026-10-09, single functions found by instruction search for property and element IDs. **Questions:** (a) which layout and element a tooltip uses for which purpose; (b) how the row list is assembled (removal of rows, separators, widths); (c) how the name row chooses its colour; (d) how a generic property row turns a value into text; (e) how the binding, durability, armour, item-level and worth rows are filled; (f) when comparison tooltips appear and for which equipped items; (g) where a tooltip is placed; (h) pinned tooltips. **Why M1–M3 were not enough:** the layouts and the class library show which rows exist and what data each row receives, but not what the native row widgets do with it (visibility, formatting, colour, assembly, placement); no published source describes the examination system; the running client is not available to this session (owner rows §15 cover the visual checks). Read: the tooltip assembler, the row-list assembly routine, the layout and element selector (two variants, by record and by instance), the comparison routine, the pinned-tooltip toggle, the hover handler that places tooltips, the type registry of the element factory (only to pair the examination widget types with their classes), the fill routines of the name, generic-property, binding, item-level, armour, durability and worth widgets, the bit-to-key list builders and the list-join helper the generic-property widget calls (second pass 2026-10-09, private copy `~/tessera-re/work/specs13-itt`, deleted after use), the name-colour table loader, the quality-to-state natives, the money widget. Nothing from network, login, launcher or anti-tamper code. |
| **Design** | Tessera rules, marked **Tessera rule**: what Tessera does where the original's behaviour is server data, untraced, or outside P1. Not claims about the original. |

| # | Fact | Source | Mark |
|---|---|---|---|
| IT1 | Item records name their tooltip by six properties: layout and element for the hover (`…_0Click_…`), the one-click (`…_1Click_…`) and the comparison (`…_Compare_…`) tooltip (§3.1) | OBS (all 151,230 items) | Confirmed 2026-10-09 |
| IT2 | 149,695 items have a hover layout and element, and every pair names a top-level element of that layout; 1,535 items (all class 796) have none (§3.2) | OBS | Confirmed 2026-10-09 |
| IT3 | The client selects the layout and element by tooltip purpose: comparison → `…_Compare_…`, hover and pinned → `…_0Click_…`, one-click → `…_1Click_…`; an item without the pair gets no tooltip; a hovered UI element's override layout and element take precedence (§3.3) | BIN (a) | BIN |
| IT4 | Each tooltip element an item record names is a frame based on `Tooltip_New` with one child list box of type `ExaminationListBox` whose entry templates, in order, are the rows; the list instantiates every template once (§4.1) | OBS (all 31 elements named by any of the three pairs of any item record: 30 in the examination layout, one in 0x220008BE) | Confirmed 2026-10-09 |
| IT5 | Row templates: widget type, format token, source property, base colour, state colours, justification and size (§4.2) | OBS | Confirmed 2026-10-09 |
| IT6 | Row-list assembly: empty rows removed, emptied nested lists removed, list shrunk to the widest row, consecutive separators collapsed, trailing separators removed, wide rows stretched (§5) | BIN (b) | BIN |
| IT7 | The name row's colour is the colour of a state of the `ColorNameText` element of the `text` layout; for an item the state follows `Item_Quality` (1 Legendary, 2 Rare, 3 Incomparable, 4 Uncommon, 5 Common; any other value the object state) (§6.2) | BIN (c); OBS (state colours, quality mapper) | BIN + Confirmed data |
| IT8 | A name too tall for its row is re-laid with the next font of the row's font list until it fits (§6.2) | BIN (c) | BIN |
| IT9 | Generic property rows show the value of the row's `UI_Examination_Tooltip_GeneroProperty` from the examined object; formatting by value type; empty when absent, when the value equals the dictionary default and `UI_Examination_Tooltip_DisplayDefault` is not true, or when the formatted text is empty (§6.4) | BIN (d); BC (class 1066) | BIN + Confirmed (bytecode) |
| IT10 | Binding text: bound-to name, else bound, else one of six "bind on" texts chosen by `Inventory_BindOnRaid` / `_BindOnEquip` / `_BindOnAcquire` and `Inventory_BindToAccount` (§6.5) | BC (class 2781 function 6, class 2779); BIN (e) | Confirmed (bytecode) + BIN |
| IT11 | Unique, indestructible, item level, armour, durability, worth rows: sources and visibility (§6.6–§6.11) | BC (classes 1090, 1068, 3796, 3707, 1060, 1093); BIN (e) | Confirmed (bytecode) + BIN |
| IT12 | Worth is shown only for sellable items, as `Item_Value` × max(1, quantity), split into currencies greedily by `Currency_Value` in directory order; zero coins are shown only by money displays configured for it (§6.11) | BC (classes 1093, 404); BIN (e) | Confirmed (bytecode) + BIN |
| IT13 | Comparison: only when the option `GOP_EnableCrossCompareItems` is on, the item is not equipped and its compare kind is Equipment; the compared items are those equipped in the slots of the item's `Inventory_CompatibleSlot` (secondary weapon added when a primary or secondary weapon slot is named) (§8) | BIN (f); BC (class 372 function 11) | BIN + Confirmed (bytecode) |
| IT14 | Placement: beside the pointer, on the side of the screen's larger half, clamped to the screen (§9) | BIN (g) | BIN |
| IT15 | Pinned tooltips: toggled per (instance, record), at most 20 at once, layout `PinnedTooltip` (§10) | BIN (h); OBS | BIN + Confirmed data |
| IT16 | `UI_Examination_Tooltip_Compare_Type` (mapper names `Equipment`, `Recipe`, …) is read by native code, not by the class library (§8.3; corrects the scope of `items.md` F44) | BIN (f); `items.md` F44 | BIN |
| IT17 | Census of compare kinds over all items (§8.3) | OBS | Confirmed 2026-10-09 |
| IT18 | The Tessera rules that stand in for row behaviours not traced (ghosted icon, damage and DPS variables, `cantuse` test, allowed-slot list, requirement lines, effect and stat lines, set lines, duration format; §6.3, §6.12–§6.18); the class-library facts of those sections are IT22 | Design | Decision |
| IT19 | Of the 79 `*_ExamineLayout` top-level elements of the examination layout, item records name 30; the other 49 serve other tooltips and differ (bases `Tooltip_Bare`, `Tooltip` or none, a second child, a foreign entry template, other widths). List-box widths of the 31 named elements: `max_width` 229 in 24, absent in seven (design widths 229 and 304) (§4.1) | OBS (all 79 elements, all items) | Confirmed 2026-10-09 |
| IT20 | The item-level text and the pending-bonus marker are joined by the `Utils` pair format with variable 0x5425 = the level text and 0x59BF = the marker; when either is empty the other is used alone (§6.7) | BC (class 3796 and the helper it calls); OBS (the format has exactly these two variables, 0x5425 placed first) | Confirmed (bytecode) 2026-10-09 |
| IT21 | Generic rows: bitfield values are listed as the display texts of the set bits' keys in ascending bit order (key = bit index + 1), joined pairwise by the `Utils` list format 0x06756762 (variables 0x5425 = text so far, 0x59BF = next text); a set bit without a display text, or no set bit, leaves the variable unset. Value types without a rule of §6.4 leave the variable unset, so the format does not render and the row is empty (§6.4) | BIN (d), second pass; `string-table.md` G25 | BIN |
| IT22 | Class-library parts of §6.3 and §6.12–§6.18: the provider class of each row type and the properties its code reads and writes (icon 1067; DPS 1058, damage 1059; equipment category 1065; allowed slots 1055; requirements 1092; effect lists 1062; cooldown 1091; item decay 2808; quest ID 1074) | BC (each class's own method bodies, property IDs as literal operands) | Confirmed (bytecode) 2026-10-09 |
| IT23 | Effect and stat lines of the item effect lists: their composition, texts and states are the effect lines of `skill-tooltips.md` §9 (definition mode, at the item's level), and the lists' sources are `skill-tooltips.md` §9.7 (§6.16) | `skill-tooltips.md` ST10, ST13 (BC + BIN) | as cited |
| IT24 | Compare kinds 7 (`SkillGrantingItem`) and 5 (`TraitItem`): the item record's `Item_GrantedSkill` or `Item_GrantedTrait` names the record whose **hover** tooltip is attached; no option gates them; 1,262 of 1,262 kind-7 items name a skill with a hover pair (§8.3). Skill-item rows show the skill of `Usage_SkillToExecute` (§6.20) | `skill-tooltips.md` ST17, ST18 (BIN, OBS) | BIN + Confirmed data |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Examination tooltip** | The tooltip panel built from an item's examination layout (this spec), as opposed to the plain text tooltip of `ui-behaviour.md` §6.3. |
| **Examination layout** | The UI layout record an item names (here 0x22000000 for all but 11 of the 149,695 items with a hover pair, `UILAYOUT` label `examination`; the 11 name 0x220008BE). |
| **Tooltip element** | A top-level element of that layout **named by an item record** (hover, one-click or comparison pair); every such label ends in `_ExamineLayout`. The examination layout holds further `*_ExamineLayout` elements for other tooltips (skills, traits, socket panels, …); they are not tooltip elements in this spec's sense (§4.1). |
| **Row template** | One entry template of the tooltip element's list box: an element of the examination layout (§4.2). |
| **Row** | One instance of a row template in a live tooltip. |
| **Widget type** | The row template's `UIElementType` label (for example `ExaminationName`); it selects the client's behaviour for the row. |
| **Format string** | A StringInfo of the row (`UI_Examination_Tooltip_DisplayString` and its kin) whose text holds substitution variables (`string-table.md`). The row fills the variables with values. |
| **Variable ID** | The numeric ID of a substitution variable in a format string, for example 0x5A6195 for the generic value. Variable IDs are data of the string table; they are given here so an implementation knows which variable receives which value. |
| **Viewer** | The local character, whose level, class and race some rows compare with the item. |
| **Instance** | A live item with per-instance properties (`items.md` §6.4 examination list) as opposed to an item **record** (a DID only). Tessera's sandbox has records only (§11). |

---

<a id="s3"></a>
## 3. Choosing the tooltip

<a id="s3-1"></a>
### 3.1 The item properties (Confirmed, IT1)

| Property | Type | Items | Use |
|---|---|---|---|
| `UI_Examination_Tooltip_0Click_Layout`, `…_0Click_Element` | DID → 0x22; enum (element label) | 149,695 each | Hover and pinned tooltips |
| `UI_Examination_Tooltip_1Click_Layout`, `…_1Click_Element` | as above | 148,979 each | One-click tooltip (§3.3) |
| `UI_Examination_Tooltip_Compare_Layout`, `…_Compare_Element` | as above | 107,479 each (20 of them 0 and 0) | The item's tooltip when it is shown as the **equipped** side of a comparison (§8) |
| `UI_Examination_Tooltip_Compare_Type` | enum (mapper with names `SkillTrait`, `Equipment`, `Virtue`, `Recipe`, `TraitItem`, `SkillGrantingItem`, `MountGrantingItem`, `PointBasedTrait`) | 110,672 | Which comparison the item's hover tooltip makes (§8) |

The hover pair and the one-click pair are equal in 150,507 of 151,230 item records.

<a id="s3-2"></a>
### 3.2 Census of hover elements (Confirmed, IT2)

Layout 0x22000000 in 149,684 items, 0x220008BE (`UILAYOUT` label
`examination_item_kinshipcharter`) in 11. Every (layout, element) pair names a top-level
element of its layout (149,695 of 149,695 hover pairs, 148,979 of 148,979 one-click pairs,
107,459 of 107,459 non-zero comparison pairs).

| Hover element (label) | Items | Item classes (`items.md` §2.1) |
|---|---|---|
| `Armor_ExamineLayout` | 44,948 | 795: 41,772; 797: 2,902; 796: 274 |
| `Jewelry_ExamineLayout` | 20,176 | 802: 19,684; 796: 492 |
| `Weapon_ExamineLayout` | 18,903 | 799 |
| `Item_ExamineLayout` | 18,655 | 796: 14,570; 2814: 3,634; 798: 301; 3663: 71; 803: 40; 802: 36; others 3 |
| `Gem_ExamineLayout` | 15,445 | 796 |
| `QuestItem_ExamineLayout` | 11,152 | 796: 11,147; 2097: 5 |
| `Recipe_ExamineLayout` | 6,055 | 805 |
| `DecorationItem_ExamineLayout` | 5,047 | 796 |
| `IA_Weapon_ExamineLayout` | 4,301 | 799 |
| `Instrument_ExamineLayout` | 2,124 | 796: 2,090; 795: 34 |
| `SkillItem_ExamineLayout` | 801 | 796 |
| `IA_Item_ExamineLayout` | 544 | 796: 469; 795: 75 |
| `DeconstructableItem_ExamineLayout` | 527 | 796 |
| `Door_ExamineLayout` | 416 | 1722: 254; 2097: 161; 796: 1 |
| `Object_ExamineLayout` | 243 | 815: 228; 796: 15 |
| `Dye_ExamineLayout` | 79 | 796 |
| `Name_ExamineLayout` | 61 | 796 |
| `IA_MountedItem_ExamineLayout`, `ArmorCompare_ExamineLayout` | 57 each | 796 |
| `WorldEventProgress_ExamineLayout` | 51 | 796 |
| `GameplayContainer_ExamineLayout` | 20 | 796 |
| `Milestone_ExamineLayout` | 18 | 804 |
| `KinshipCharter_ExamineLayout` (layout 0x220008BE) | 11 | 796 |
| `Money_ExamineLayout` | 3 | 796 |
| `SummonPlayerItem_ExamineLayout` | 1 | 796 |
| none | 1,535 | 796 |

<a id="s3-3"></a>
### 3.3 Selection rule (BIN, IT3)

1. If the UI element under the pointer carries both `UI_Examination_Tooltip_Override_Layout`
   and `UI_Examination_Tooltip_Override_Element` with non-zero values, those name the tooltip
   (the character sheet's stat rows use this, `character-panels.md` §3.4). Otherwise:
2. The purpose picks the property pair: **comparison** → `…_Compare_Layout` and
   `…_Compare_Element`; **hover** and **pinned** → `…_0Click_…`; **one-click** →
   `…_1Click_…`. For a live instance the client reads the pair from the instance's
   properties, for a record from the record's property set (two variants of one routine).
3. If either value is missing the tooltip is not shown. An item without the pair has **no
   tooltip at all** (1,535 records here).
4. The tooltip element is created from (layout, element) like a panel (`ui-layout.md` §9.1–§9.2)
   and marked as an examination tooltip; the examined instance ID
   (`UI_Examination_Tooltip_IID`) or record (`UI_Examination_Tooltip_DID`) is stored on it.
   Properties of the request (for example the override compare kind) are copied onto it.

> **OPEN:** what triggers a **one-click** tooltip. Every traced call site of the assembler
> passes the hover purpose; none passes the one-click purpose as a constant. The pair differs
> from the hover pair in only 723 records. **Tessera rule:** one-click requests use the
> hover pair.

---

<a id="s4"></a>
## 4. Structure (Confirmed, IT4, IT5)

<a id="s4-1"></a>
### 4.1 Tooltip element and list box

**Scope.** The statements of this section hold for the **tooltip elements**: the elements that
any of the three pairs of any item record names (§3.1). Here they are 31: 30 top-level
elements of the examination layout and `KinshipCharter_ExamineLayout` of 0x220008BE. An
implementation finds them by walking the item records, not by the label suffix.

Every tooltip element is based on `Tooltip_New` of
layout 0x22000001 (a frame whose background is `Tooltip_New_Background`, `ui-layout.md` §7)
and holds one child, `Examination_AutoResizeListbox`, of widget type `ExaminationListBox`,
at (5, 5) inside the frame:

| Property of the list box | Value here | Meaning |
|---|---|---|
| `UICore_ListBox_entry_templates` | 1–55 entries (layout, element) | The row templates, in display order (top to bottom) |
| `UICore_ListBox_entry_templates_autofill` | true | One row per entry template, created when the tooltip is built |
| `UICore_Element_max_width` | 229 in 24 of the 31 list boxes; **absent** in seven: `Weapon_ExamineLayout` and `WeaponCompare_ExamineLayout` (list box designed 229 wide) and the five `IA_*` elements (designed 304 wide) | Rows wider than this wrap or are cut by their own rules |
| `UICore_Element_min_width` | 20 | |
| `UICore_Element_notify_on_resize` | true | The frame follows the list's size |

The frame is the list box's size plus its 5-pixel inset on each side (`ui-layout.md` §8:
edges of the list box are (0, 0, 0, 0); the frame is resized by the client to wrap it). The
designed frame widths are 239 (list box 229) and 314 (list box 304).

**Entry templates.** The entry templates of the 30 tooltip elements of the examination layout
name top-level elements of that same layout (771 templates). Those of
`KinshipCharter_ExamineLayout` name 12 top-level elements of the examination layout and one
(`KinshipCharter_GuildHistory_Base`) of its own layout 0x220008BE. An implementation resolves
each entry template by the (layout, element) pair it states, never by assuming the
tooltip element's own layout.

> **OPEN:** the wrap width of a list box without `UICore_Element_max_width` (the seven list
> boxes above). The limit pass of `ui-behaviour.md` §9.2.1 applies a maximum only when the
> property is set, so the original does not cap them by that property; what bounds their text
> was not traced. **Tessera rule:** such a list box uses its designed width (229 or 304 here)
> as its maximum width for §5 step 3 and §7.

**Other `*_ExamineLayout` elements (out of scope).** The examination layout holds 79
top-level elements whose label ends in `_ExamineLayout`; 49 are not named by any item
record and serve other tooltips (skills, traits, effects, socket panels, social lists and
others; decision 0024 P2, P3). They do not all share the structure above: 11 are based on
`Tooltip_Bare`, one on `Tooltip`, one has no base element and its list box sits at (0, 0);
one has a second child (a decoration field) besides its list box; one has an entry template
naming an element of another layout; their list boxes' `max_width` values include 231, 330,
335, 390 and 2048, and one list box has none. Specs for those tooltips state their own structure.

Example, the order of `Armor_ExamineLayout` (52 rows; labels of row templates):
`NameIcon_Row_Base`, `NeedsReforge_Base`, `ItemAdvancement_AdvanceableItem_Base`,
`ItemAdvancement_XP_Row_Base`, `ItemAdvancement_LegendaryPoints_Base`, `Inscription_Base`,
`CraftedBy_Base`, `ItemBinding_Row_Base`, `UI_WebStore_Base`, `ExaminationItemLevel_Base`,
`Indestructibility_Base`, `QuestID_Base`, `ClothingColor_Base`, `ImplementType_Base`,
`ExaminationLevelRangeRestriction_Base`, separator, `ArmorValue_Base`, `EffectDisplay_Base`,
separator, `ItemSocketList_Base`, separator, `UsageEffectDisplay_Base`, eleven skill-item
rows, the two summon-cost rows, `Usage_Cooldown`, `ItemDecay`, separator,
`Durability_Row_Base`, `UsageReqsDisplay_Base`, separator, `GameSetMemberListDisplayBase`,
`GameSetBonusListDisplayBase`, separator, `Description_Base`, separator, `Value_Base`,
separator, `QuestGuide_Base`, `Disenchantment_Base`. Implementations read the list from the
layout; this example is for tests.

<a id="s4-2"></a>
### 4.2 Row templates used by item tooltips

All are top-level elements of the examination layout. "Format" is the row's format string as
(token, table); every format of this table lives in the `Examination` string table
(0x250001AF here, reached through key 0x10000053 of the named-string-table DID map). Colours
are the row's `UICore_Text_font_colors` first entry (ARGB) and its states' overrides; the
markup colour for `<rgb>` tags inside a format is the format's own. Fonts are the rows'
`UICore_Text_fonts` (0x42 records, `fonts.md`), most rows one font, the name row five.
"Rule" points to the content rule.

| Row template | Widget type | Source (rule) | Format token | Base colour | State colours | Just. | Size |
|---|---|---|---|---|---|---|---|
| `NameIcon_Row_Base` | `AutoResizeListBox`, horizontal | `Icon_Base` then `Name_Base` (§4.3) | — | — | — | — | 180 × 48 |
| `Icon_Base` | `Field` holding an `ExaminationIcon` 32 × 32 at (4, 8) | §6.3 | — | — | `ghosted` overlay | — | 40 × 48 |
| `Name_Base`, `Name` | `ExaminationName` | §6.2 | 0x05C26945 (one variable 0x52615) | 0xFFC3B585 | from the colour table (§6.2) | left (`Name` centred) | 180 × 48 (`Name` 200 × 22) |
| `ItemBinding_Row_Base` | `ExaminationRow`: left `ItemBinding_Element`, right `ItemUniqueness_Element`; stretch wide | §4.3 | — | — | — | — | 200 × 18 |
| `ItemBinding_Base` | `ExaminationItemBinding` | §6.5 | (texts of §6.5) | 0xFFE5E5E5 | — | left | 146 × 18 |
| `ItemUniqueness_Base` | `ExaminationItemUniqueness` | §6.6 | `UI_Examination_Tooltip_ItemUniqueText` 0x07872D65 | 0xFF666666 | — | right | 220 × 18 |
| `UniqueEquipped_Base` | `ExaminationGeneroProperty` | §6.4, `Inventory_UniqueEquipped` | 0x0FAD18F4 | 0xFF26B27F | — | left | 250 × 18 |
| `ExaminationItemLevel_Base` | `ExaminationItemLevel` | §6.7 | 0x0D5EEEEC (variable 0x50AA9C) | 0xFFE5E5E5 | — | left | 220 × 18 |
| `Indestructibility_Base` | `ExaminationIndestructibility` | §6.8 | `UI_Examination_Tooltip_ItemIndestructibleText` 0x09293789 | 0xFFC3B585 | — | left | 146 × 18 |
| `ArmorValue_Base` | `ExaminationArmor` | §6.9, `Item_Armor_Value` | 0x03BDEBE5 (0x5A6195) | 0xFFCCCC7F | `enchanted` 0xFFFF7F00 | left | 220 × 18 |
| `Durability_Row_Base` | `ExaminationRow`: left `Durability_Element` (base `Durability_Base`), right `DurabilityRating_Element` (base `DurabilityRating_Base`); stretch wide | §6.10 | — | — | — | — | 200 × 18 |
| `Durability_Base` | `ExaminationDurability` | §6.10 | 0x024FC349 (variables 0xF1E3399 current, 0x4015BA9 maximum) | 0xFFE5E5E5 | `broken` 0xFFFF0000, `almostBroken` 0xFFFF7F00 | left | 146 × 18 |
| `DurabilityRating_Base` | `ExaminationGeneroProperty` | §6.4, `Item_DurabilityEnum` | 0x0CC76D07 | 0xFFE5E5E5 | `enchanted` | right (left margin 24) | 220 × 18 |
| `Value_Base` | `ExaminationValue` | §6.11 | (money display, §6.11) | 0xFFE5E5E5 | — | left | 204 × 22 |
| `Damage_Base` | `ExaminationDamage` | §6.12 | 0x01894215 (three variables 0x2864455, 0x48615B5, 0x5BE1855) | 0xFFE5E5E5 | `enchanted` | left | 146 × 18 |
| `DPS_Base` | `ExaminationDPS` | §6.12 | 0x075C1BA3 (variable 0x4953) | 0xFFE5E5E5 | `enchanted` | left | 146 × 18 |
| `ImplementType_Base` | `ExaminationEquipmentCategory` | §6.13 | 0x003CD7E5 (0x689C009) | 0xFFE5E5E5 | `cantuse` 0xFFFF0000 | left | 220 × 18 |
| `AllowedSlots_Base` | `ExaminationAllowedSlots` (list of `AllowedSlotsElement`) | §6.14 | — | 0xFFE5E5E5 | — | left | 220 × 22 |
| `Implement_And_Allowed_Base` | `ExaminationRow`: `ImplementType_Element`, `AllowedSlots_Element` | §6.13, §6.14 | — | — | — | — | 200 × 18 |
| `UsageReqsDisplay_Base`, `…_Horizontal_Base` | `ExaminationUsageReqs` (entries `UsageReqsListElement`) | §6.15 | 0x08649EBC | entries 0xFFE5E5E5 | entries `cantuse` 0xFFFF0000 | left | 220 × 1 |
| `ExaminationLevelRangeRestriction_Base` | `UIExaminationLevelRangeRestriction` | §6.15 | 0x0D29A46E (0x51DE, 0x5168) | 0xFFE5E5E5 | — | left | 220 × 23 |
| `EffectDisplay_Base`, `TargetEffectDisplay_Base`, `UsageEffectDisplay_Base` (`*_Wide` 304 wide) | `ExaminationEffectList` (entries `EffectListElement`) | §6.16 | usage list: header 0x0F345375 | — | — | — | 220 × 0 |
| `WeaponSlayerMod_*`, `WeaponPropertyMod_*` | `ExaminationWeaponSlayerMod`, `ExaminationWeaponPropertyMod` | §6.16 | 0x07B64534 | 0xFFB200B2 | `enchanted` | centred (`_Small` left) | 304 × 15 (229 × 18) |
| `GameSetMemberListDisplayBase`, `GameSetBonusListDisplayBase` | `ExaminationGameSetMemberList`, `…BonusList` | §6.17 | entry and header templates 0x0687F7C5, 0x040D6175, 0x083A3695 | states `gameset_active`, `gameset_inactive` | — | — | 220 × 0 |
| `Usage_Cooldown` | `ExaminationUsageCooldown` | §6.18 | 0x0973E0EE | 0xFF009900 | — | left | 146 × 18 |
| `ItemDecay` | `ExaminationItemDecay` | §6.18 | 0x08DCED69 (0x58E15) | 0xFFFF0000 | — | left | 146 × 18 |
| `QuestID_Base` | `ExaminationQuestID` | §6.18 | 0x0A4BF4B4 (0xA897485) | 0xFFE5E5E5 | — | left | 220 × 22 |
| `Description_Base` | `ExaminationGeneroProperty` | §6.4, `Description` | 0x051F92AE | 0xFFCCCC7F | `enchanted` | left (right margin 6) | 220 × 18 |
| `CraftedBy_Base` | `ExaminationGeneroProperty` | §6.4, `CrafterName` | 0x0086A0E9 | 0xFF26B27F | `enchanted` | left | 220 × 18 |
| `Inscription_Base` | `ExaminationGeneroProperty` | §6.4, `Craft_ItemInscription` | 0x051902AE | 0xFF26B27F | `enchanted` | left | 220 × 18 |
| `ItemConsumable_Base` | `ExaminationGeneroProperty` | §6.4, `Usage_DestroyOnUse` | 0x0407880D | 0xFF666666 | — | left | 170 × 18 |
| `ItemBarter_Base`, `ItemReputation_Base` | `ExaminationGeneroProperty` | §6.4, `Barter_IsBarterItem`, `Reputation_IsReputationItem` | 0x0F08AD5D, 0x0D6C64BD | 0xFFE5E5E5 | — | left | 170 × 18 |
| `UI_WebStore_Base` | `ExaminationGeneroProperty` | §6.4, `Item_IsPurchasedWebStoreItem` | 0x03F996E5 | 0xFFD8DCE7 | — | left | 250 × 18 |
| `Item_DestroyIfYouLeavePE_Base` | `ExaminationGeneroProperty` | §6.4, `PrivateEncounter_DestroyItemOnExit` | 0x067F2C74 | 0xFFFF0000 | — | left | 250 × 18 |
| `ItemDecorationCategory_Base` and the two decoration-placement rows | `ExaminationGeneroProperty` | §6.4, `Item_Decoration_Category`, `Item_Decoration_NotAllowedInKinshipHouse`, `Item_Decoration_OnlyAllowedForHouseOwner` | 0x0AE88BB9, 0x04212105, 0x07F8F852 | 0xFFE5E5E5 | — | left | 170 × 18 |
| `UI_PackageItem_ContainsMoreItems`, `UI_PackageItem_MaxContentsLevel` | `ExaminationGeneroProperty` | §6.4, `PackageItem_NumGeneratedItems`, `PackageItem_CachedMungeLevel` | 0x00A9EEE5, 0x008EB04C | 0xFFD8DCE7 | — | left | 250 × 18 |
| `EnhancmentRuneLevel_Base` | `ExaminationGeneroProperty` | §6.4, `Item_Socket_GemMaxLevel` | 0x070EB400 | 0xFF00E500 | — | left | 220 × 22 |
| `SummonedName_Base` | `ExaminationGeneroProperty` | §6.4, `Summon_ToSummonName` | 0x023E1195 | 0xFF00E500 | — | left | 220 × 22 |
| `CurrentlyEquipped_Label` | `Text` | first row of every `*Compare_ExamineLayout` (§8) | `UICore_Text_entry` 0x0E96BE74 | 0xFFFF7F19 | — | left | 220 × 18 |
| `Separator_Base`, `Separator_Short_Base` | `Field` with `UI_Examination_Tooltip_Separator` true | §4.4 | — | — | — | — | 170 × 10, 170 × 8 |

Every text row above is outlined (`UICore_Text_outline` true), vertically centred, with left
margin 4 (unless stated), `UICore_Text_tag_font_colors` 0xFFCC9933 for markup tags without
their own colour (the worth row 0xFFE5E5E5), and `UICore_Text_fit_to_text` set so the row's
height follows its text (`fonts.md` §7). The `enchanted` state (0xFFFF7F00) and `pending`
state exist on most rows; which rows the client puts into `enchanted` was not traced
(OPEN, §6.1).

The skill-item, legendary (`IA_*`, `ItemAdvancement_*`, `NeedsReforge_Base`), socket, runic,
lockbox, recipe, summoning, world-event and quest-guide rows also occur in item layouts; §6.20
says what Tessera does with them in P1.

<a id="s4-3"></a>
### 4.3 Composite rows

- **Name and icon** (`NameIcon_Row_Base`): a horizontal auto-resizing list of the icon field
  (40 × 48) and the name row; the list grows sideways to fit both.
- **Two-column rows** (`ExaminationRow`): `UI_Examination_Tooltip_Row_Left_Child` and
  `…_Right_Child` name two child elements, laid out on one line, left child left-justified and
  right child right-justified. The row is filled by filling both children; it is empty only
  when both are empty. With `UI_Examination_Tooltip_Stretch_Wide` true the row is stretched to
  the list's width after assembly (§5 step 6), which puts the right child at the right edge.

<a id="s4-4"></a>
### 4.4 Separators

A separator is an empty field (no image, no text) of height 10 (`Separator_Base`) or 8
(`Separator_Short_Base`): it is vertical space between groups of rows. Separators are never
"empty" in the sense of §5 step 1 (their height is not zero); §5 steps 4–5 remove the
redundant ones.

---

<a id="s5"></a>
## 5. Assembling the row list (BIN, IT6)

After every row has been filled (§6), the client processes the list box in this order:

1. **Remove empty rows.** For each row, in order: a two-column row first lays out its two
   children. A row whose width or height is 0 is removed. A row with
   `UI_Examination_Tooltip_RemoveWhenHidden` true is removed when it is hidden. A row that is
   itself a list (effect lists, set lists, requirement lists) is processed recursively by
   these same rules and is removed when it ends with no entries. Filling a row with no content
   clears its text, which makes its height 0, so "empty" means "nothing to show".
2. Note the widest remaining row.
3. **Shrink:** if the list box is wider than its widest row, it is made that wide (never wider
   than its `max_width`, §4.1).
4. **Collapse separators:** walking from the top, a separator that directly follows another
   separator is removed. A separator at the very top is kept.
5. **Trim:** separators at the end of the list are removed, last first, until the last row is
   not a separator (or the list is empty).
6. **Stretch:** every remaining row with `UI_Examination_Tooltip_Stretch_Wide` true is resized
   to the list box's width.
7. The list box and its frame are laid out again at the resulting size.

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) the size a text row takes after filling.
> §4.2 says `UICore_Text_fit_to_text` makes the row's height follow its text; whether the
> row's width follows the text too (which decides the widest row of step 2) is not stated.
> As implemented: a text row is as wide and as high as its measured text (margins
> included, wrapped at the list's width limit of §4.1); separators keep their designed
> size, so on this install they bound the list's width from below (170). Please state the
> width rule.

A tooltip whose rows all wait for data (the client counts pending rows in
`UI_Examination_Tooltip_PendingElements`) is shown only when the data arrives
(`UI_Examination_Tooltip_NotifyWhenAssembled`). **Tessera rule:** all row data is available
when the tooltip is built (§11), so nothing is pending.

Worked example: §14.1.

---

<a id="s6"></a>
## 6. Row content

<a id="s6-1"></a>
### 6.1 Common model (BIN, BC)

For every row, the client's class library holds a provider class (one per examination widget
type; base class 1097 here, with 96 children) that builds a small property set for the row
from the examined object, and the native widget turns that set into text, state and colour.
The provider runs on the **examined object** (instance or record) and may consult the
**viewer**. In what follows, "the item" is the examined object; "the record" its property set;
an instance adds its own properties (`items.md` §6.4).

Fields of the row data that recur:

| Row data property | Meaning |
|---|---|
| `Examination_StringInfo` | A ready text (StringInfo) the widget shows as is |
| `Examination_StateID` | A state label the widget switches to (its colour override, if any, applies) |
| `Examination_Color` | An explicit colour for the text (overrides the state colour) |
| row-specific values | For example `Examination_CurrentDurability`, `Examination_Value` (named per rule below) |

A row whose provider writes nothing is empty and is removed by §5.

> **OPEN:** which conditions put a row into its `enchanted` state (orange). The state exists
> on most value rows; its provider-side trigger (probably an effect raising the value, by its
> name) was not traced. **Tessera rule:** never in P1.

<a id="s6-2"></a>
### 6.2 Name (BIN, BC, IT7, IT8)

1. **Text.** The item's display name: its `Name` StringInfo rendered per
   `string-table.md` §9 (the provider returns the object's name; for an item record that is
   `Name`). The row's format (one variable, 0x52615) receives it.
2. **State and colour.** The provider asks the client for the object's **name state**. For an
   item that is the **quality state**:

   | `Item_Quality` key (mapper `ItemQualities`) | Mapper name | State label of the colour table | Colour here (ARGB) |
   |---|---|---|---|
   | 1 | `Legendary` | `ItemQuality_Legendary_ColorName` | 0xFFF1AB0B |
   | 2 | `Rare` | `ItemQuality_Rare_ColorName` | 0xFFC200C7 |
   | 3 | `Incomparable` | `ItemQuality_Incomparable_ColorName` | 0xFF00FDFF |
   | 4 | `Uncommon` | `ItemQuality_Uncommon_ColorName` | 0xFFFFFF00 |
   | 5 | `Common` | `ItemQuality_Common_ColorName` | 0xFFFFFFFF |
   | 0, absent, other | — | `Object_ColorName` | 0xFFFFFFFF |

   The **colour table** is data: the client reads, once, every state of the element
   `ColorNameText` of the `text` layout (`UILAYOUT` label `text`, 0x22000002 here) and maps each
   state ID to the first colour of that state's `UICore_Text_font_colors` (32 states here,
   including the item-quality, consider-level and player-relation colours). The row's text
   colour is the table colour of its state ID; a state without a table entry gives the
   client's default text colour. An `Examination_Color` in the row data overrides it (the item
   provider sets none).
3. **Fit.** The row has five fonts (`UICore_Text_fonts`). If the laid-out name needs more
   height than the row allows (48), the row re-lays the text with each font of the list in
   order, from the first, and stops at the first that fits; if none fits it keeps the last.

The mapping of step 2 is a native function of the client that other code paths also call
(their uses were not traced). `icon-layers.md` §4's OPEN on quality colouring is
answered by this: quality colours the **name**; the icon's background tile is the only
quality cue of the icon.

<a id="s6-3"></a>
### 6.3 Icon (BC class 1067, IT22; Tessera rule IT18)

The `ExaminationIcon` (32 × 32) shows the composed item icon (`icon-layers.md` §3). Its
provider (class 1067) writes `Examination_Icon`, `Examination_IconOverlay` and
`Examination_IsItem` into the row data, and its code also names `Examination_IsGhosted`; when true the field's `ghosted`
state draws a darkening overlay (layout data: a 0x41 image with draw mode 1 and a colour
item 0xBF000000).

> **OPEN:** when an item icon is ghosted in a tooltip. **Tessera rule:** never in P1.

<a id="s6-4"></a>
### 6.4 Generic property rows (BIN, BC, IT9)

Widget type `ExaminationGeneroProperty` (and `ExaminationSkillGeneroProperty`, same rule).
The row names a property P in `UI_Examination_Tooltip_GeneroProperty`.

Provider (class 1066):

1. Look P up on the examined object (an instance first, then its record). If found, put it
   into the row data.
2. If not found: when the row has `UI_Examination_Tooltip_GeneroPropertyServerData` true, the
   row waits for the server's data (pending, §5); otherwise the row is empty.
3. If the row has `UI_Examination_Tooltip_StyleGrabber`, a state chosen from that value is
   added (none of the item rows has it).

Widget:

1. Set the row's state from `Examination_StateID` if present.
2. Read value V of P from the row data. Absent → empty.
3. If the row has no format string and V is not a DID → empty.
4. Unless the row's `UI_Examination_Tooltip_DisplayDefault` reads **true** (an absent value
   reads false; no item row sets it true, 9 rows of other tooltips do): V equal to P's
   dictionary default (`property-system.md` §2.2; only for properties that have one) →
   empty; for a StringInfo V, an empty resolved text → empty.
5. If the row has `UI_Examination_Tooltip_ComparisonProperty` Q and the row itself holds a
   value for Q: that value must equal V, else empty (no item row uses this).
6. If V is a DID: the row shows that DID as an image sized to the image, and no text.
7. Otherwise the format's variable 0x5A6195 receives V formatted by type:

   | Value type (`property-system.md` §3) | Text |
   |---|---|
   | string | the string |
   | enum | the display text of the key in P's mapper (`property-system.md` §7.1) |
   | float | with `UI_Examination_Tooltip_DisplayAsDuration` true: a duration text (§6.18); else the number with `UI_Examination_Tooltip_FloatPrecision` decimals (default 1) |
   | int | duration text when `DisplayAsDuration` is true, else the decimal integer |
   | int64 | decimal integer |
   | StringInfo | the resolved text |
   | boolean | the `Utils` table's yes or no text (tokens 0x0F3E6915 for true, 0x03918965 for false, table key 0x1000004A) |
   | bitfield (32, 64, any width) | the list text of the set bits (below, IT21) |
   | DID | handled by step 6 |
   | any other type (string token, waveform, timestamp, tri-state, vector3, instance ID, property ID, struct, array, colour, position) | none: the variable is **left unset**. A format that has variable 0x5A6195 then does not render (`string-table.md` §7.1, G25: a slot without a value), the text is empty and the row is empty by step 9 (IT21) |

   **List text of a bitfield** (32- and 64-bit forms traced): walk the bits from the lowest
   to the highest; for each set bit at index *i* (0-based) take key *i* + 1 and its display
   text in P's mapper (key rule of `property-system.md` §7.3; the space-separated listing
   there is the client's debug display, not this row). If any set bit's key has no display
   text, or no bit is set, the variable is left unset (empty row as above). Otherwise the texts are
   joined in that order: the first text alone, then for each further text the `Utils` list
   format (token 0x06756762, table key 0x1000004A) with variable 0x5425 = the text so far and
   0x59BF = the next text. If one of the two is empty the other is taken alone. Item rows
   that show bitfields this way (all row templates reachable from the 31 tooltip elements):
   `ItemDecorationCategory_Base` (`Item_Decoration_Category`, 64-bit) and
   `UsageTargetType_IAWeaponTitles`, `UsageTargetType_IAClassItemTitles`
   (`Usage_ValidTargetTypes`, 32-bit).

   > **OPEN:** the bit order of the any-width bitfield form (type 22), which goes through a
   > different list builder that was not read. **Tessera rule:** the same order and join as
   > the 32- and 64-bit forms.

8. If the format has no variable 0x5A6195 the format text is used as is (flag rows such as
   the barter, reputation and consumable rows show their fixed text when the flag is true).
9. An empty result text → empty row; otherwise the row shows the text.

<a id="s6-5"></a>
### 6.5 Binding (BC class 2781 function 6, class 2779; BIN; IT10)

The binding row's text is the **first** of these that applies (all texts are tokens of the
`Examination` table, key 0x10000053; each carries its own markup colour):

| # | Condition | Token |
|---|---|---|
| 1 | The row data has `UI_Examination_Tooltip_BoundToCharacterName` (a string; the server's) | 0x00462E92 with variable 0x5506DA2 = that name |
| 2 | The instance's `Inventory_BoundToID` ≠ 0 and resolves to a known character name | 0x00462E92 with that name |
| 3 | `Inventory_BoundToID` ≠ 0, `Inventory_BindToAccount` true | 0x02D47F24 |
| 4 | `Inventory_BoundToID` ≠ 0, otherwise | 0x0D454A14 |
| 5 | `Inventory_BindOnRaid` true, account | 0x0051F4F4 |
| 6 | `Inventory_BindOnRaid` true, otherwise | 0x0E97C994 |
| 7 | `Inventory_BindOnEquip` true, account | 0x0FF88704 |
| 8 | `Inventory_BindOnEquip` true, otherwise | 0x09686EE0 |
| 9 | `Inventory_BindOnAcquire` true, account | 0x09F18224 |
| 10 | `Inventory_BindOnAcquire` true, otherwise | 0x034E69E5 |
| 11 | `Inventory_BindToAccount` true | 0x02D47F24 |
| 12 | none | no text (row empty) |

"Account" means `Inventory_BindToAccount` true. Absent booleans read false; an absent
`Inventory_BoundToID` reads 0 (`items.md` §5.4). For a record (no instance), rows 1–4 never
apply. The binding combinations of `items.md` §5.4 therefore give: (true, true, –) → 9;
(true, –, –) → 10; (false, –, true) → 8; (false, true, true) → 7; none → 12.

<a id="s6-6"></a>
### 6.6 Uniqueness (BC class 1090)

`Inventory_Unique` true → the row shows its `UI_Examination_Tooltip_ItemUniqueText`
(right-justified, grey). Otherwise empty. (`UniqueEquipped_Base`, in gem layouts, is a
generic row on `Inventory_UniqueEquipped`, §6.4.)

<a id="s6-7"></a>
### 6.7 Item level (BC class 3796)

Shown for every item (objects of the item base class's ancestor, `items.md` §2.2).

1. *L* = the item's level: for an instance the level-scaling result of `items.md` §5.2; for a
   record `Item_Level` (absent: 0).
2. If the item is a live instance held by a character (its owner), a class-library helper
   that was not traced computes an effective level *E* for that owner:
   if *E* < *L* the text is token 0x0D69E17E with variable 0x9E7DC9C = *E* and 0x50AA9C = *L*;
   if *E* > *L* token 0x07AD68B0 with the same variables. Otherwise token 0x0D5EEEEC with
   0x50AA9C = *L*.
3. If the item has `Item_Level_Bonus_Pending` true, the marker text (token 0x054000F3 of
   the `Examination` table) is joined to the text of step 2 by the `Utils` table's pair
   format 0x040CF382 (table key 0x1000004A): variable **0x5425 = the level text** of step 2,
   variable **0x59BF = the marker**. If the level text is empty the marker is shown alone; if
   the marker is empty the level text alone (IT20).

> **OPEN:** the viewer-effective level *E* of step 2 (a helper of the item-level provider
> that compares the item with the viewer; not traced). **Tessera rule:** *E* = *L* (step 2
> never applies to records).

<a id="s6-8"></a>
### 6.8 Indestructible (BC class 1068)

`Inventory_Indestructible` true → the row shows its
`UI_Examination_Tooltip_ItemIndestructibleText`. Otherwise empty.

<a id="s6-9"></a>
### 6.9 Armour (BC class 3707; BIN)

The armour value *A*: for a record, `Item_Armor_Value` (absent 0; equal to the armour
progression at the item level for every item that has both, `items.md` §9.5, F14); for an instance, the
item's armour accessor (the scaled value of `items.md` §9.5) and, when the instance has an
owner, the owner-adjusted value of a class-library helper (not traced). The widget shows the
row's format with variable 0x5A6195 = *A* as an integer; *A* equal to the property's default
(0) → empty.

<a id="s6-10"></a>
### 6.10 Durability (BC class 1060; BIN)

Left child (`Durability_Base`):

1. *cur* = `Item_CurStructurePoints` (default −1), *max* = `Item_MaxStructurePoints`
   (default −1), *ws* = `Item_WearState` (default 0).
2. If *max* is absent or ≤ 0 → empty.
3. Text: format with variable 0xF1E3399 = *cur* and 0x4015BA9 = *max*, integers.
4. State: *ws* = 3 (`Broken`) → `broken` (red); *ws* = 1 (`Damaged`) → `almostBroken`
   (orange); otherwise the default colour.

Right child (`DurabilityRating_Base`): a generic row (§6.4) on `Item_DurabilityEnum`,
right-justified: the display text of the sturdiness key.

<a id="s6-11"></a>
### 6.11 Worth (BC classes 1093, 404; BIN; IT12)

Provider:

1. *v* = `Item_Value` (absent or 0 → empty row).
2. If the item is not sellable by the client library's vendor rules (`items.md` §5.3: forced
   not sellable, sale value 0, quest item, indestructible, barter item, legendary item,
   purchased store item) → empty row.
3. *q* = max(1, the instance's `Inventory_Quantity`; 1 for a record). The row value is
   *v* × *q*, a signed 32-bit product that wraps on overflow (two's complement,
   `wlib-bytecode.md` integer rule); a wrapped result below 1 makes the row empty (widget
   rule below).

Widget: a value below 1 → empty. Otherwise the row creates the money template
`MoneyDisplay_ItemWorthTemplate` of the `moneydisplay` layout (a "worth" label, token
0x09607F15 of the `Examination` table, then the coin list `MoneyDisplay_ItemWorthDisplay`,
left-justified coins) and gives it the amount.

**Breakdown into coins** (class library, class 404 function 2): walk the currency
directory's `Currency_CurrencyList` in stored order (`economy.md` §7: here gold, silver,
copper with `Currency_Value` 100,000, 100, 1); for each currency the count is
amount div `Currency_Value` (integer division); when the count is above 0 it is recorded
and amount becomes amount mod `Currency_Value`. Currencies with count 0 are not recorded.

**Coins:** the coin list holds at most three coin elements (max columns 3, one row), each a
count text (right-justified, formatted as a decimal integer through the format of key
0x10000065 token 0x07480C44, variable 0x5242A9) and the coin's icon state (`Gold`, `Silver`,
`Copper`). A coin with count 0 is hidden unless the money display's "show zero coins" option
is set; when the display also hides leading zeros, a zero coin is shown only after a larger
non-zero coin.

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) the parts of
> `MoneyDisplay_ItemWorthTemplate` and of one coin of `MoneyDisplay_ItemWorthDisplay` (which
> element holds the label, which a coin's count text, which takes the coin's icon state) are
> not named, so the worth row cannot be filled through the template by label. As
> implemented: the row shows code-made parts on one line, the label text, then per coin its
> count text and an icon image the client supplies (16 × 16 until it is loaded), 2 pixels
> apart and centred vertically. Please name the template's parts.

> **Answered (note, 2026-10-10, [`npc-windows.md`](npc-windows.md) §9.2, OBS; no fact of this
> spec changed):** `MoneyDisplay_ItemWorthTemplate` (215 × 24) has two parts: the label
> `MoneyDisplay_WorthText` (the token 0x09607F15 text above, fits its text, left margin 4) and
> the coin list `MoneyDisplay_ItemWorthDisplay` at (49, 2), based on
> `MoneyDisplay_ExaminationTemplate`, which names its coin element by
> `UIComponent_MoneyDisplay_CoinDisplay_ElementID` = `MoneyDisplay_ExaminationCoinTemplate`
> (59 × 22): the count text `MoneyDisplay_Text` (37 × 16 at (0, 1), right-justified) and the icon
> `MoneyDisplay_Icon` (32 × 22, in `MoneyDisplay_Icon_Holder` at (29, 0)), whose states `Gold`,
> `Silver`, `Copper` show the images 0x41007E7B, 0x41007E7C, 0x41007E7D here. The display
> builds exactly three coin elements (BIN, `npc-windows.md` §9.2). **Order caveat:** "stored
> order" in the breakdown above is the order of the currency directory's **object form**
> (gold, silver, copper); its property set stores silver, gold, copper, so a reader of the
> property set sorts by `Currency_Value`, largest first (`npc-windows.md` §9.3).

> **OPEN:** the default of the two money-display options for the worth row (whether a zero
> silver coin shows between gold and copper). **Tessera rule:** zero coins are hidden.
> Example: §14.4.

<a id="s6-12"></a>
### 6.12 Damage and DPS (BC classes 1058, 1059, IT22; Tessera rule IT18)

The providers compute the values of `items.md` §9.3–§9.4 (DPS from the DPS table at the
item's level and quality, damage from DPS, speed and variance, damage type from
`Combat_DamageType`) and write `Examination_DPS` (class 1058), `Examination_Damage`,
`Examination_DamageVariance`, `Examination_DamageType` (class 1059). The code of both names
`Combat_DPS_LUT`, `Combat_WeaponSpeed`, `Combat_DamageType`, `Item_EquipmentCategory` and
`Item_Quality` as sources (IT22); the formulas are those of `items.md`.

> **OPEN:** which damage value fills each of the three variables of the damage format
> (minimum, maximum, damage-type text) and the decimals of the DPS variable. **Tessera rule:**
> minimum and maximum per the reading of `items.md` §9.4 (marked Reported there), rounded to
> integers; damage type = display text of `Combat_DamageType`; DPS with one decimal.

> **OPEN:** (implementer, 2026-10-10) the Tessera rule above gives the three values but not
> which variable receives which. `crates/gamedata` (`tooltip`) fills them in the order the
> table of §4.2 lists them: 0x2864455 = minimum, 0x48615B5 = maximum, 0x5BE1855 = damage-type
> text (empty text when the type key has no display text). Confirm or give the order.

<a id="s6-13"></a>
### 6.13 Equipment category (BC class 1065, IT22; Tessera rule IT18)

The provider reads `Item_EquipmentCategory` and writes `Examination_EquipmentCategory` and
`Examination_StateID`; the row shows the display text of the set category key (variable
0x689C009). The `cantuse` state (red) marks a category the viewer
cannot use.

> **OPEN:** the viewer test behind `cantuse`. **Tessera rule:** never red in P1.

<a id="s6-14"></a>
### 6.14 Allowed slots (BC class 1055, IT22; Tessera rule IT18)

The provider lists the slots of `Inventory_CompatibleSlot`; each becomes an
`AllowedSlotsElement` text.

> **OPEN:** which slot bits are listed and their texts. **Tessera rule:** one entry per set
> bit of `Inventory_CompatibleSlot` other than `Backpack`, `Overflow` and `Equipment`, with the
> display text of the slot key (mapper `ContainerSlot`), in ascending key order.

<a id="s6-15"></a>
### 6.15 Requirements (BC class 1092, IT22; Tessera rule IT18)

The provider asks the item's `Usage_Permission` object (`items.md` §5.10) to describe its
requirements for the viewer and writes them as an array (`Examination_UsageInfo_Array` of
`Examination_UsageInfo_Struct`, each with `Examination_StringInfo` and `Examination_StateID`);
each requirement becomes one `UsageReqsListElement` line, in
state `cantuse` (red) when the viewer does not meet it. The level-range row
(`ExaminationLevelRangeRestriction_Base`) shows a minimum and maximum level (variables 0x51DE,
0x5168) for items with a level window.

> **OPEN:** the wording and order of requirement lines (they come from the permission
> classes' code, not traced). **Tessera rule:** lines for minimum level (`Usage_MinLevel`),
> maximum level (`Usage_MaxLevel`), required classes and races (`items.md` §5.10), in that
> order, with Tessera's own wording; red when the viewer's scripted level, class or race
> (§11) fails the line.

> **OPEN:** (implementer, 2026-10-10) how a list row names its line element. §4.2 gives the
> base and state colours of the requirement lines on the entries (`UsageReqsListElement`),
> and §6.17 the `gameset_*` states of the set lines, but not the property that links the row
> to that element. The set and effect list rows carry `UICore_ListBox_entry_templates`; the
> requirements row carries none (on this install it has a struct property
> `UI_Usage_List_Template` that no spec describes). `crates/gamedata` (`tooltip`) takes the
> line colours from the first entry template whose element has state colours, else the first
> with a base colour; for a row without entry templates, from the element §4.2 names for its
> widget type (`UsageReqsListElement`, `EffectListElement`, `AllowedSlotsElement`), looked
> up by label in the row's layout; then from the row's own colours. Please state the link.

<a id="s6-16"></a>
### 6.16 Effect and stat lines (BC class 1062, IT22; Tessera rule IT18)

The effect lists (`EffectDisplay_*`: effects on the wearer; `TargetEffectDisplay_*`: on the
target; `UsageEffectDisplay_*`: on use, with a header line) are filled by the effect-list
provider (class 1062) from the item's effect lists and stat modifiers (`items.md` §5.8–§5.9)
into an `Examination_EffectInfo_Array` of `Examination_EffectInfo_Struct` entries, one
`EffectListElement` per effect, each with the effect's own examination text
(`combat-data.md`). This is the same machinery as effect tooltips (decision 0024 P2).

> **Answered (2026-10-10, P2 revision, IT23; was OPEN (P2)):** the lines of these lists are
> the effect lines of `skill-tooltips.md` §9: for each effect of the list, in definition mode
> at the item's level (`skill-tooltips.md` §9.7, §9.8), its definition description, one
> modifier line per `Mod_Array` entry (§9.4 there: the operator's template with the
> property's `PropertyMetaData_Name` and the value text, or the entry's
> `Mod_DescriptionOverride`), the chance line and the combat-only and duration lines; each line
> in state `buffed` or `debuffed` (`duration` for the last two) of the `EffectListElement`
> colours. The item's own `Mod_Array` is shown through the template effect
> `PropertyModEffect` (§9.7 there), so its entries are the item's stat lines. The rows filter
> the item's effects by their `UI_Examination_Tooltip_EffectCategory` (0x10 wearer, 0x20
> target, 0x08 usage). The P1 Tessera rule (`Effect_Name` per effect, dictionary names for
> stats) is superseded.

> **OPEN:** (implementer, 2026-10-10) engineering choices where the Tessera rule is silent,
> for review: (1) no source of a property's *display name* is given; the implementation shows
> the dictionary name of `Mod_Modified`, then the value (at most two decimals). (2) The target
> and usage lists show one line per effect of the item's target and usage effect lists
> (`Effect_Name`); the usage list's header is the row's format, shown when the list has lines.
> (3) The weapon slayer and property-mod rows (`WeaponSlayerMod_*`, `WeaponPropertyMod_*`,
> §4.2) have no content rule here; they are built empty like the rows of §6.20.
>
> *Answer (2026-10-10, P2 revision):* (1) the display name is the modified property's
> `PropertyMetaData_Name` (`misc-records.md` §13.1); a property without a metadata object gets
> no line; values are formatted by `skill-tooltips.md` §9.4 step 8. (2) Target and usage lists
> follow `skill-tooltips.md` §9.7 (categories 0x20 and 0x08); the header rule is
> `skill-tooltips.md` §9.6 (its OPEN keeps the implementation's rule). (3) Still open: the
> slayer and property-mod rows stay empty in P2.

<a id="s6-17"></a>
### 6.17 Item sets (Tessera rule IT18)

The member list shows the set's name and its members (template 0x0687F7C5 per member) and the
bonus list the tiers (header 0x083A3695 with the count, template 0x040D6175 per bonus); each
line is in state `gameset_active` or `gameset_inactive` depending on whether the viewer wears
that member or has reached that count (`items.md` §7.1 rules). Tessera computes "worn" from the
equipment container (§11).

> **OPEN:** the exact line composition. **Tessera rule:** member lines with the members'
> names, bonus lines with the count and the bonus effect names, active per `items.md` §7.1.

> **OPEN:** (implementer, 2026-10-10) `items.md` §7.1 leaves open whether tiers activate
> cumulatively. The implementation marks the bonus line of count *c* `gameset_active` when the
> worn member count is at least *c* and the level gate of `items.md` §7.1 passes (cumulative
> reading); bonus lines list the tier's effect names, then its stat lines (§6.16 rule), after
> "*c*:". Confirm or replace.

> **OPEN:** (implementer, 2026-10-10) the set level of `items.md` §7.1 ("Set level for count
> *c*") needs a request's "given level" and the character's `Apparent_Level`. The
> implementation evaluates each tier's stat lines at that set level with these readings:
> the given level is the examined item's level; the character's level is the viewer's
> scripted level (§11), else the examined item's level; the active members of the average
> rule are the viewer's equipped members (the examined item too when it is equipped), each
> with its item level; the single-precision average is truncated to an integer level;
> member-less sets use the given level (the viewer carries no count property). Confirm or
> replace.

<a id="s6-18"></a>
### 6.18 Cooldown, decay, quest, flags (BC classes 1091, 2808, 1074, IT22; Tessera rule IT18)

- `Usage_Cooldown` (class 1091): the item's `Usage_CooldownDuration` (seconds through
  `CooldownControl`, `items.md` §9.8) as a duration text; empty without a cooldown. The
  provider copies `Usage_CooldownDuration`, `Usage_CooldownChannel` and the item's DID
  (`UI_Examination_Tooltip_DID`) into the row data; another of its methods writes
  `ExaminationElement_UsageCooldown_Total`, `…_Remaining` and `Examination_StateID`.
- `ItemDecay` (class 2808): the item's `ItemDecay_Duration` (seconds through
  `ItemDecayControl`) as a duration text, red; empty without decay. The provider writes
  `ExaminationElement_ItemDecay_Total` from `ItemDecay_Duration`; its fill method also writes,
  under conditions that were not traced, `ExaminationElement_ItemDecay_Remaining` and the
  flags `ItemDecay_Cancelled` and `Inventory_BindOnRaid`.
- `QuestID_Base` (class 1074): the quest named by the item's `Quest_DataID` (variable
  0xA897485 = the quest's name, in a format of the `Examination` table), written as
  `Examination_StringInfo`; empty without `Quest_DataID`. The format is token 0x0A4BF4B4, or
  0x0C5E3954 when two tests of the examined object (a native test with class index 801 and a
  virtual method of the object, neither traced) select it. **Tessera rule:** 0x0A4BF4B4.
- Flag rows: §6.4.

> **OPEN:** the client's duration text format. **Tessera rule:** largest two non-zero units
> of days, hours, minutes, seconds, with Tessera's own unit wording.

> **OPEN:** (implementer, 2026-10-10) the variables of the cooldown format (0x0973E0EE) are
> not given, and §4.2 gives 0x58E15 for the decay format only. The implementation renders the
> row's format with the duration text in both 0x58E15 and 0x5A6195; when the format does not
> render (a slot without a value, G25) the duration text is shown alone, and when it renders
> without the duration the duration is appended. Please state the cooldown format's variable.

<a id="s6-19"></a>
### 6.19 Description

A generic row (§6.4) on `Description`: empty when the item has no description text. Its format wraps the text in markup (`string-table.md` §4.7).

<a id="s6-20"></a>
### 6.20 Rows Tessera leaves empty in P1 (Tessera rule)

Legendary rows (`IA_*`, `ItemAdvancement_*`, `NeedsReforge_Base`, `Item_UpgradeTier_Base`),
socket rows (`ItemSocketList_Base`, `ExaminationItemSocketType_Base`), runic rows, lockbox,
summoning, decoration-house, world-event, quest-guide, disenchantment and recipe one-shot and
cooldown rows. They are built as empty rows, so §5 removes them; the real-data test counts
them (§12).

**Skill-item rows (P2 revision 2026-10-10, IT24).** The 13 rows with
`UI_Examination_Tooltip_For_SkillItem` (14 in the instrument layouts) are no longer left empty:
they show the rows of the skill named by the item's `Usage_SkillToExecute`, by the rules of
`skill-tooltips.md` §6.1 and §7 (with that spec's Tessera rules: damage and class-mechanics
rows empty in P2), and are empty when the item names no skill (`skill-tooltips.md` §12.3:
1,194 items name one).

> **OPEN:** (implementer, 2026-10-10) how the rows of this list are recognised, and one
> conflict. The implementation recognises them by widget type: every examination widget type
> without a rule in §6.2–§6.19 (for example the legacy, runic, socket-list, upgrade-tier,
> quest-guide, world-event, lockbox, summoning-cost, recipe and skill-item types) gives an
> empty row; their entry templates are line templates of those widgets and are not built as
> nested rows. Generic rows (§6.4) are left empty here only for the `ItemAdvancement_*` labels
> and the two decoration-house rows (`ItemDecorationNotAllowedInKinshipHouse_Base`,
> `ItemDecorationOnlyAllowedForHouseOwner_Base`), which §4.2 otherwise lists as generic rows.
> **Conflict:** `IA_Label_Base` (a field holding the icon and the `ExaminationName` row of the
> five `IA_*` layouts, 4,902 items) matches `IA_*` but is the only name row of those tooltips;
> leaving it empty would contradict §12 (`Rendered` = at least the name row). The
> implementation fills it (icon and name). Please confirm or exempt it explicitly.

---

<a id="s7"></a>
## 7. Text rules

- Formats are rendered per `string-table.md` (variables by ID, plural forms, markup). Rows
  with `UICore_Text_parse_markup_tags` true interpret `<rgb>` tags; the tag colour without its
  own value is the row's tag colour.
- Numbers are decimal integers without grouping unless a format says otherwise.
- Texts wrap within the list box's `max_width` minus the row's margins (`fonts.md` §7).

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) the wrap width of a row inside the
> horizontal name-and-icon list (`NameIcon_Row_Base`, §4.3). The name follows the 40-pixel
> icon field, so wrapping it at the whole `max_width` makes the row 40 pixels wider than the
> list may be (§5 step 3 never widens past `max_width`). As implemented: each entry of a
> horizontal list wraps within what is left of the list's width limit after the entries
> before it (229 − 40 = 189 for the name), and a text part placed at an offset inside its
> row wraps within the limit less that offset. Please give the rule.

---

<a id="s8"></a>
## 8. Comparison tooltips (BIN, BC, IT13, IT16, IT17)

<a id="s8-1"></a>
### 8.1 When

When a hover tooltip is assembled, the client reads the compare kind: the request's
`UI_Examination_Tooltip_Override_Compare_Type` if the hovered element has one, else the
item's `UI_Examination_Tooltip_Compare_Type`. For kind **Equipment** (2):

1. The viewer's option `GOP_EnableCrossCompareItems` must be true; otherwise no comparison.
2. If the examined item is an instance whose `Container_Slot` has the `Equipment` bit (it is
   equipped) → no comparison.
3. *S* = the item's `Inventory_CompatibleSlot`. If *S* = 0 → no comparison.
4. The compared items are the viewer's equipped items found by the slot query (§8.2); for each
   one, a tooltip is built from **that** item's `…_Compare_…` pair (purpose comparison, §3.3)
   and attached to the hover tooltip. Every comparison layout starts with
   `CurrentlyEquipped_Label`.

<a id="s8-2"></a>
### 8.2 Slot query (BC class 372 function 11)

1. If *S* lacks the `Equipment` bit (bit 29, mask 0x20000000) → no items.
2. *M* = *S* AND 0x0FFFFFFF. If *M* has either weapon bit (mask 0x30000: `Weapon_Primary`
   0x10000, `Weapon_Secondary` 0x20000), add 0x20000.
3. For each single bit *b* = 0x1, 0x2, 0x4, … below 0x2000000 (bits 0–24), in ascending
   order: if *b* is in *M* and an item is equipped in slot *b*, add it (once).

So an armour piece compares with the piece in its slot, a ring with both rings, a one-handed
weapon with both hands. Example: §14.5.

<a id="s8-3"></a>
### 8.3 Other kinds and census

| Kind (key, name) | Items | What the client does (BIN) |
|---|---|---|
| 2 `Equipment` | 101,400 | §8.1 |
| 4 `Recipe` | 6,055 | The recipe item's result: from `RecipeItem_Recipe` → `CraftRecipe_ResultItem` (and its quantity), a tooltip of the result item is attached |
| 5 `TraitItem` | 1,557 | The hover tooltip of the trait named by `Item_GrantedTrait` is attached (`skill-tooltips.md` §12.4; trait tooltips are P6) (IT24) |
| 7 `SkillGrantingItem` | 1,262 | The **hover** tooltip of the skill named by the item record's `Item_GrantedSkill` is attached (`skill-tooltips.md` §12.4; all 1,262 name a skill with a hover pair; no option gate) (IT24) |
| 8 `MountGrantingItem` | 378 | The granted mount skill's tooltip (P5) |
| 0 | 20 | none |
| absent | 40,558 | none |

The class library does not read `UI_Examination_Tooltip_Compare_Type` (`items.md` F44); the
native assembler does.

> **OPEN:** where the attached tooltips are placed relative to the hover tooltip. **Tessera
> rule:** side by side, in the order of §8.2, on the side of the hover tooltip that faces the
> screen's centre, tops aligned, the group clamped to the screen as one box.

> **OPEN:** the default of `GOP_EnableCrossCompareItems` (an options-panel check box; its
> dictionary default is not the client's start-up value, `appearance.md` §3.3 OPEN on option
> storage). **Tessera rule:** on; Tessera settings key `ui.compare_equipped_items`.

---

<a id="s9"></a>
## 9. Showing, placing, hiding (BIN, IT14)

- **When.** An item tooltip appears through the hover rule of `ui-behaviour.md` §6.2 (the
  same delay); an item slot's hover handler requests the examination tooltip instead of a text
  tooltip. Hovered world objects use the same delay (BIN).

  > **OPEN:** the code path by which an item slot requests the examination tooltip rather
  > than a text tooltip was not traced (the item-slot templates of `character-panels.md` §6
  > name a tooltip layout but no tooltip text, which is consistent with it). **Tessera
  > rule:** every item slot requests the examination tooltip of its item.
- **Placement** (pointer at (*px*, *py*), cursor size *cw* × *ch*, tooltip size *w* × *h*,
  screen *SW* × *SH*):
  1. *x* = *px* + *cw* when *px* / *SW* < 0.5, else *px* − *w*;
  2. *y* = *py* + *ch* when *py* / *SH* < 0.5, else *py* − *h*;
  3. a negative *x* or *y* becomes 0;
  4. *x* = min(*x*, *SW* − *w*), *y* = min(*y*, *SH* − *h*).

  The tooltip does not follow the pointer afterwards. Example: §14.6.
- **Anchored mode.** A client mode places the tooltip at a fixed anchor element's position
  (clamped the same way) instead of at the pointer.

  > **OPEN:** which option turns the anchored mode on. **Tessera rule:** not offered.

- **Hiding:** as `ui-behaviour.md` §6.6.

---

<a id="s10"></a>
## 10. Pinned tooltips (BIN, OBS, IT15)

A pinned tooltip is a copy of the hover tooltip in its own small window: the examination
layout's `PinnedTooltip` element (`UIScaler` of scaling group `Tooltips`, 33; a background,
a list box holding the tooltip, a drag bar and a close button), offset by
(`UI_Examination_Tooltip_PinnedOffset_X`, `…_Y`) = (18, 18).

- Pinning is a toggle keyed by (instance ID, record DID): pinning an item that is already
  pinned closes its pinned tooltip.
- At most **20** pinned tooltips exist at once; a further request is refused with a UI message.
- A pinned tooltip uses the hover pair (`…_0Click_…`).

> **OPEN:** the input that pins a tooltip (probably the `TOOLTIP_DETACH` command,
> `keymap.md`, by its name). **Tessera rule:** `TOOLTIP_DETACH` while an item tooltip shows
> pins it.

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) three details of the pinned window:
> (1) the point the offset (18, 18) is taken from; as implemented, the showing hover
> tooltip's position, else the §9 position at the pointer; (2) which element closes the
> window (a close button exists, but no property this spec gives names it); as implemented,
> every Button of the `PinnedTooltip` window outside the tooltip closes it; (3) the window's
> size; as implemented, it grows by what the tooltip needs beyond its list box's designed
> size. Please confirm or give the rules.

> **OPEN:** (implementer, `crates/ui`, 2026-10-10) two more details of the pinned window:
> (4) which layout's `PinnedTooltip` a pin opens for `KinshipCharter_ExamineLayout`: §2
> calls 0x220008BE "the examination layout" of those 11 items, but that layout has no
> `PinnedTooltip`; as implemented, the window always comes from the layout of `UILAYOUT`
> label `examination` (0x22000000 here), which the client gives the controller; (5) the
> list box holding the tooltip: as implemented, it is grown to at least the tooltip's size
> (the window by the same amount), since the painter clips children to it. Please confirm
> or give the rules.

---

<a id="s11"></a>
## 11. The scenario feed and the sandbox

Tessera's sandbox knows item **records**, not server instances. The scenario feed
(`scenario-feed.md` §5.5, §6.1, §9.2) drives tooltips as follows.

| Feed input | Tooltip effect |
|---|---|
| `ItemAdded { what: Ref::Record(did), quantity, at }` | The slot shows the record; its tooltip is the record's (§3–§6). `quantity` is the instance quantity of §6.11 step 3. |
| `ItemAdded { what: Ref::Literal(text) }` | **Tessera rule:** a minimal tooltip: the frame with one name row showing the literal text in the `Object_ColorName` colour, no icon; no other rows. |
| `ItemQuantity` | Updates *q* (§6.11). |
| `ItemCondition { current, max }` | Replaces *cur* and *max* of §6.10 for that item; *ws* = `Broken` when *current* = 0, `Damaged` when *current* × 4 ≤ *max* (**Tessera rule**; the original's wear-state thresholds are server data), else the record's `Item_WearState`. |
| `ItemMoved` to the `Equipment` container | The item counts as equipped: it gets no comparison (§8.1 step 2), it is found by the slot query (§8.2), and it counts as worn for set lines (§6.17). |
| `CharacterInfo`, `LevelChanged` of the player | The viewer's class, species and level for §6.15. |
| (none) | No instance has `Inventory_BoundToID`, `CrafterName`, `Craft_ItemInscription`, level scaling or a server binding name, so those rows are empty (§6.4 step 2: no row is pending). |

Hovering a slot of a bag, the paperdoll or a vendor or loot list (`character-panels.md`,
P3) shows the tooltip; the walker needs no other input. The same model serves a server
backend later: an instance's properties then fill the rows of §6 that records cannot.

> **OPEN:** (implementer, `apps/client/src/character/tooltip.rs`, 2026-10-10) The literal
> item's minimal tooltip (Tessera rule above) needs a tooltip element and a name-row template,
> which no item names. As implemented: the element `Item_ExamineLayout` of the layout with
> `UILAYOUT` label `examination`, and the first entry template of its list box whose row is
> an `ExaminationName`; without them a literal item shows no tooltip. Also as implemented in
> the client's conversion of the model for the panel: texts lose their style spans (plain
> text), and the worth row's coins carry their count texts but no coin image (the coin
> element of §6.11 is not named, see the OPEN there). Confirm or give the element.

> **Answered in part (note, 2026-10-10, `npc-windows.md` §9.2):** the coin element and its
> images are named in the answer under §6.11; the coin image is the state image of
> `MoneyDisplay_Icon` (`Gold`, `Silver`, `Copper`). The literal item's tooltip element stays
> as implemented.

> **Answered (2026-10-10, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md) D5):** `scenario-feed.md` §6.1 `item-property $item
> PROPERTY=VALUE` emits `ItemProperty { item, property, value }` (§9.2 there; values
> scripted, kinds and limits in §5.8 there). The rows of §6.5 (1–4) and the crafted rows read
> such a reported property as an instance property (§6.4 provider step 1); without the step
> the table row "(none)" above still holds. The note as raised:
>
> **OPEN (for `scenario-feed.md`, non-blocking):** a step to give an item instance selected
> properties (binding target, crafter name, inscription) so tooltip rows 1–4 of §6.5 and the
> crafted rows can be shown in scenarios. Values would be scripted (decision 0024 §4).

---

<a id="s12"></a>
## 12. Real-data test (decision 0024 P1 exit criterion 2)

For every item record of the install at `TESSERA_GAME_DIR` (`items.md` §2.4), build the hover
tooltip model (§3–§6, with the Tessera rules) and lay it out headless. Each record ends in
exactly one category:

| Category | Meaning | This install |
|---|---|---|
| `Rendered` | A tooltip with at least the name row | 149,695 expected |
| `NoTooltip` | No hover pair (§3.3 step 3): the original shows none | 1,535 |
| `LayoutUnreadable` | The named layout record is missing or fails to decode | 0 expected |
| `ElementMissing` | The element is not a top-level element of the layout | 0 |
| `RowFailure` | A row's rule failed on malformed data (the row is left empty; the tooltip still renders and is counted here as well) | 0 expected |

The test also reports, per row template, how many tooltips had it non-empty, and the count
of rows left empty by §6.20. A failure is a panic, an error escaping the categories, or a
`Rendered` count that differs from the number of records with a valid hover pair.

> **OPEN:** (implementer, 2026-10-10) "at least the name row" does not hold for 61 of the
> 149,695 records on this install: `WorldEventProgress_ExamineLayout` (51 items) has no name
> row among its entry templates, and 10 items of `QuestItem_ExamineLayout` have a `Name` whose
> text renders empty (the name row is then empty by §6.2). The implementation counts every
> record whose tooltip element is built as `Rendered` (149,695, as expected) and reports the
> tooltips without a shown name separately; the real-data test fails only when a layout has a
> name row and the item's name renders but the row is empty. Restate the category or say what
> those tooltips show.

> **OPEN:** (implementer, 2026-10-10) which lookup misses count as `RowFailure`. On this
> install about 71,000 item records have a `Description` naming a token missing from its
> string table, and 860 tooltips list effects whose `Effect_Name` names a missing token.
> The implementation treats a lookup miss in a text the examined content names
> (description and other generic StringInfo values, effect, set and member names) like an
> empty text (§6.4 step 4: the row or line is left out) and does not count it; a miss in a
> row's own format or a fixed table token (worth label, coin count, binding, item level),
> and a record that cannot be read, is a `RowFailure`, as are row templates that cannot be
> read fully (a cycle of entry templates, more than the implementation's node bound).
> Confirm, or say what the client shows for such texts.

---

<a id="s13"></a>
## 13. Invariants

| ID | Invariant | Status |
|---|---|---|
| IV1 | Every item record with both hover properties names a top-level element of the named layout (149,695 of 149,695) | Confirmed 2026-10-09 |
| IV2 | Every tooltip element (§4.1: an element named by any of the three pairs of any item record; 31 here) is based on `Tooltip_New` and has exactly one child, a list box of type `ExaminationListBox` with autofill true. Test: collect the distinct non-zero (layout, element) pairs of all item records and check each; the other `*_ExamineLayout` elements are not tested | Confirmed 2026-10-09 (31 of 31) |
| IV3 | Every entry template of those list boxes names a top-level element of the layout the template itself states (784 of 784 here: 783 in the examination layout, one in 0x220008BE) | Confirmed 2026-10-09 |
| IV4 | Every tooltip element whose label ends in `Compare_ExamineLayout` (seven here) has `CurrentlyEquipped_Label` as its first entry | Confirmed |
| IV5 | `ColorNameText` of the `text` layout has a state for each of the five item-quality state labels and for `Object_ColorName` | Confirmed |
| IV6 | After assembly (§5) no two separators are adjacent and the last row is not a separator | Design check (synthetic) |
| IV7 | Provided the currency list holds a currency with `Currency_Value` 1 (copper here), the coin breakdown of §6.11 reconstructs the amount: Σ count × `Currency_Value` = amount, for every amount 1 … 2³¹ − 1. Without such a currency the remainder below the smallest value is not shown, and the invariant does not apply | Design check |
| IV8 | Binding rule §6.5 yields exactly one text or none for each of the 2⁴ combinations of the four flags (bound ID 0) | Design check |

---

<a id="s14"></a>
## 14. Worked synthetic examples

<a id="s14-1"></a>
### 14.1 Assembly

Rows in template order (heights after filling): name 48, binding 18, separator 10, armour 0
(empty), separator 10, separator 10, durability 18, separator 10, description 0 (empty),
separator 10, worth 22, separator 10. Step 1 removes armour and description. Step 4 removes
the second and third of the three adjacent separators and the separator that now follows
another one before worth. Step 5 removes the final separator. Result: name, binding,
separator, durability, separator, worth.

<a id="s14-2"></a>
### 14.2 Binding

Record with `Inventory_BindOnAcquire` true, `Inventory_BindToAccount` true, no other flags:
rule 9. Same record as an instance with `Inventory_BoundToID` ≠ 0 and no known name: rule 3.

<a id="s14-3"></a>
### 14.3 Name colour

`Item_Quality` = 3 → state `ItemQuality_Incomparable_ColorName` → this install's colour
0xFF00FDFF. `Item_Quality` = 0 → `Object_ColorName`.

<a id="s14-4"></a>
### 14.4 Worth

`Item_Value` 123,405, quantity 2 → amount 246,810. Currencies (100,000, 100, 1): gold 2,
remainder 46,810; silver 468, remainder 10; copper 10. Coins shown: 2, 468, 10. Amount
200,005: gold 2, silver 0 (not recorded), copper 5; with the Tessera rule the silver coin is
hidden.

<a id="s14-5"></a>
### 14.5 Slot query

`Inventory_CompatibleSlot` = 0x20010000 (`Equipment` + `Weapon_Primary`): *M* = 0x10000, plus
0x20000 → slots 0x10000 and 0x20000: the main-hand and off-hand items, in that order.
0x20001800 (`Equipment` + `Ring1` + `Ring2`) → both rings.

<a id="s14-6"></a>
### 14.6 Placement

Screen 1920 × 1080, cursor 32 × 32, tooltip 240 × 300, pointer (1500, 200): *px*/*SW* =
0.78 → *x* = 1260; *py*/*SH* = 0.19 → *y* = 232. Pointer (100, 1000): *x* = 132, *y* = 700.

---

<a id="s15"></a>
## 15. Open questions (summary) and owner checks

| # | Question | Section | Settles it |
|---|---|---|---|
| Q1 | One-click tooltip trigger | §3.3 | BIN (callers) or owner |
| Q2 | `enchanted` state trigger | §6.1 | BC (providers) |
| Q3 | Ghosted icon | §6.3 | owner |
| Q4 | Viewer-effective item level | §6.7 | BC |
| Q5 | Zero coins in the worth row | §6.11 | owner (O80) |
| Q6 | Damage variables, DPS decimals | §6.12 | owner (O80) or BIN |
| Q7 | `cantuse` test of the category row | §6.13 | BC |
| Q8 | Allowed-slot list | §6.14 | owner (O80) |
| Q9 | Requirement lines | §6.15 | BC (permission classes) |
| Q10 | Effect and stat lines | §6.16 | Answered 2026-10-10 by `skill-tooltips.md` §9 (IT23) |
| Q11 | Set lines | §6.17 | owner (O80) |
| Q12 | Duration format | §6.18 | BIN or owner |
| Q13 | Comparison placement; option default | §8.3 | owner (O81) |
| Q14 | Anchored mode option | §9 | BIN |
| Q15 | Pin input | §10 | owner (O81) |
| Q16 | Instance properties in scenarios | §11 | `scenario-feed.md` owner; answered by decision 0025 D5 |
| Q17 | Wrap width of list boxes without `max_width` | §4.1 | BIN |
| Q18 | Bit order of any-width bitfields in generic rows | §6.4 | BIN |
| Q19 | Quest-ID format choice | §6.18 | BC |
| Q20 | How item slots request the examination tooltip | §9 | BIN |
| Q21 | Weapon slayer and property-mod rows | §6.16 | BC (classes 1096, 2490) |

Owner checks: `docs/checklists/owner-observations.md` O80 (tooltip rows of ten items) and
O81 (comparison and pinning).

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-09 | First version (Review), for decision 0024 P1. Provenance OBS (all layouts, all items), BC (examination providers and helpers of the class library), BIN (assembler, list assembly, name colour, generic rows, binding, worth, armour, durability, comparison, placement, pinning). |
| 2026-10-09 | Review fixes (status stays Review). §2, §4.1, IT4, IV2–IV4 scoped to the 31 tooltip elements item records name; the other 49 `*_ExamineLayout` elements listed as out of scope; list-box widths corrected and an OPEN with a Tessera rule for list boxes without `max_width` (IT19). §6.7 step 3: variables of the pair format (IT20). §6.4 step 7: bitfield list text and join, the "other" types leave the variable unset (IT21), OPEN for the any-width bit order. New row IT22 (BC) for §6.3, §6.12–§6.18, BC list extended (1062, 1067, 1074), IT18 limited to Tessera rules; §6.18 names `Quest_DataID` and the quest-format choice. §6.11 signed wrapping product; §9 OPEN replaces "Probable"; IV7 precondition; §15 Q17–Q20. |
| 2026-10-10 | Pointers only (no fact changed): the §11 OPEN on instance properties in scenarios and row Q16 point to their answer in decision 0025 (`scenario-feed.md` revision 4, `item-property`). |
| 2026-10-10 | Implementer OPENs (no fact changed): §6.12 damage variables, §6.15 line-element link of list rows, §6.16 stat names and other effect lists, §6.17 cumulative tiers and set-level readings, §6.18 cooldown variable, §6.20 recognition and `IA_Label_Base`, §12 unnamed tooltips and which lookup misses are row failures. |
| 2026-10-10 | Implementer OPENs from `crates/ui` (no fact changed): §5 text-row width, §6.11 parts of the worth template, §10 pin offset origin, close button and window size. |
| 2026-10-10 | Notes only (no fact changed), for P3: §6.11 implementer OPEN answered (parts of the worth template, coin element and coin images, `npc-windows.md` §9.2) with the currency-order caveat (object form versus property set, `npc-windows.md` §9.3); §11 coin-image part of the implementer OPEN answered.
| 2026-10-10 | P2 revision (spec writer, dirty room; status Revised, awaiting independent review): §6.16 effect and stat lines answered by `skill-tooltips.md` §9 (IT23; the P1 Tessera rule superseded; implementer notes (1) and (2) answered, (3) stays open as Q21); §6.20 skill-item rows filled from `Usage_SkillToExecute` (IT24); §8.3 compare kinds 7 and 5 (IT24); §15 Q10 answered, Q21 added. No other fact changed.
|
