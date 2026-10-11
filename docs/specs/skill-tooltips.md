<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Skill and effect tooltips: examination layouts, rows, effect lines, timers, icons

| | |
|---|---|
| **Status** | Draft, awaiting independent review (first version 2026-10-10) |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | — (a separate dirty-room session that wrote no part of this spec) |
| **Review record** | — |
| **Implements into** | `crates/gamedata` (`tessera-gamedata`: the tooltip model of a skill or effect record, the effect-line model shared with items, §6–§10, and the census of §14), `crates/ui` (rows of the examination panel, the timer rows, §7, §8, §10), `apps/client` (hover over quickslot skills, skill lists and effect-bar icons, §12), `apps/explorer` (tooltip preview on the skill and effect cards), the real-data test of decision 0024 P2 exit criterion 2 (§14) |
| **Companion specs** | [`item-tooltips.md`](item-tooltips.md) (examination tooltips: selection §3.3, frame and list box §4.1, assembly §5, generic rows §6.4, name colour §6.2, placement §9; its §6.16, §6.20 and §8.3 point here), [`combat-data.md`](combat-data.md) (skills §7, effects §6, modifier entries §5, progressions §4), [`misc-records.md`](misc-records.md) §13.1 (property metadata, percentage display), [`icon-layers.md`](icon-layers.md) (§3 composition, §5.1–§5.3 skill, effect and examination icons), [`string-table.md`](string-table.md) (StringInfo, variables, §9.4 numbers), [`property-system.md`](property-system.md) (types, enum mappers, bit ↔ key), [`ui-layout.md`](ui-layout.md), [`ui-behaviour.md`](ui-behaviour.md) §6 (tooltip timing), [`wlib-bytecode.md`](wlib-bytecode.md) (the class library's code), [`combat-ui.md`](combat-ui.md) (P2: quickslot bar §9, effect bars §8, which request these tooltips), [`scenario-feed.md`](scenario-feed.md) (Accepted, revision 4; §13 here) |

---

<a id="s0"></a>
## 0. Purpose

When the pointer rests on a skill (in a quickslot, a skill list, a skill-granting item) or on
an effect icon (the player's or the target's effect bar), the original client shows an
**examination tooltip** built the same way as an item's (`item-tooltips.md`): a frame with a
list of row templates from the examination layout, each row filled by a provider from the
examined object and assembled by the rules of `item-tooltips.md` §5. This spec gives:

- which layout and element a skill or effect names, and how the tooltip is chosen (§3);
- the rows of the skill and effect tooltips, their templates and formats (§4);
- the content rule of every skill row (§6, §7) and effect row (§8);
- the **effect lines**: the text the client generates for an effect (its description, one line
  per stat modifier, the chance, combat-only and duration lines), shared with the effect and
  stat lines of item tooltips (§9; answers `item-tooltips.md` §6.16);
- the duration and timer texts (§10);
- which icon a skill or effect shows in tooltips, quickslots and effect bars (§11);
- hover behaviour of quickslot skills, effect-bar icons, skill items and skill-granting items
  (§12), and what Tessera's sandbox feeds into all of it (§13);
- a census over every skill and effect record of the install with the categories of the
  real-data test (§14), invariants (§15) and worked examples (§16).

Values given for this install (DIDs, element IDs, tokens, variable IDs, colours, counts) are
**observations**. Implementations read layouts, labels, string tables, enum mappers and
properties at run time; numbers here are for tests and for recognising the data.

Out of scope: trait, virtue and quest tooltips; the class-mechanics rows (pips, gambits,
materials, summon costs) beyond what §7.12 says; the combat formulas behind damage numbers
(`combat-data.md` §13: server logic); the server's choice of which effects an actor holds.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (Steam, client build 4906.0071.1794.4035), read-only, in place; archives unchanged since 2026-10-04. Read on 2026-10-10 with throwaway probes in the dirty scratch area built on the repository's own decoders: the examination layout 0x22000000 (dump of 2026-10-06, `item-tooltips.md` OBS) and the `text` layout 0x22000002; the dump of **every** object record with its property set made on 2026-10-07 (`combat-data.md` OBS3), filtered to all 13,247 skills, all 32,971 effects, all 5,211 property-metadata objects and all 151,230 items; the 80,750 distinct (table, token) pairs those records name for names, descriptions, modifier overrides and metadata names, each looked up in the English string tables (existence and emptiness only); the 0x41 directory (1,408,052 surfaces) for icon existence; the DID maps 0x28000004 (named string tables) and 0x28000001 (EMAPPER); the WEENIECONTENT map; the Examination (0x250001AF) and UI (0x250001BB) tables for **tokens and variable IDs only**. No game text is reproduced here. |
| **OBS2** | Correlations between the compiled half and the property half (2026-10-10, same dump): for each attribute of the skill class's attribute list (`wlib.md` §8), the property whose value equals the attribute's in every skill; for the effect class's flags attribute, which property each tested bit follows. |
| **BC** | The class library's code (record 0x56000000), read 2026-10-10 with a throwaway probe over `tessera-wvm` and `tessera-wstate` (body disassembly, method tables by function ID, `wlib-bytecode.md` §6.4). Data in the user's install (method M1). Classes read (indices of this install): the skill examination base 1079 and its children 1078 (damage), 1080 (skill effect lists), 1081 (skill generic rows), 1082 (materials), 1083 (max targets), 1084 (pips), 1087 (trait), 1088 (vital cost), 1841 (qualifier), 2290–2293 (gambits and pip costs), 2308 (spellcraft modifier), 2386 (max range), 2406 (casting time); the examination classes 1061 (effect description), 1062 (effect lists), 1064 (effect time), 1066 (generic), 1067 (icon), 1073 (name), 1085 (recovery time), 1086 (special class), 2015 (effect resistance); the effect base class 698 functions that build examination lines (slots 92, 104, 124, 127, 128, 131, 137, 138, 148, 149, 203, 210–213, 220, 221, 229, 230, 254, 283, 284), the property-modification class 734 (slots 220, 221, 307, 308, 316, 319, 320), the modifier classes 1340/1341 (functions 9, 12, 13, 20, 46–49 here), the property-metadata control class 1392, class 1046 (functions 44, 45, 56, 64: the item effect list), the skill class 827 accessors named in §7, the skill-display class 378 function 84, the induction class 1195 function 119. |
| **BIN** | **M4 under the standing authorisation of decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`), headless decompiler on a private copy of the shared analysis project (`~/tessera-re/work/p2-stt`, deleted after use), 2026-10-10, single functions located through the element factory's type registry (as `item-tooltips.md` BIN), the native-function registration pairing of `wlib-bytecode.md`'s BIN row, and instruction searches for property IDs as immediates. **Questions:** (a) what the native row widgets of the skill and effect examination types do with their row data (format, variable, state, visibility); (b) how the duration and timer texts are built; (c) how a modifier value and the chance are formatted as numbers; (d) how a bitfield value becomes the resistance text; (e) which skill icon a skill display shows and what it puts on its tooltip request; (f) what the client attaches to a skill-granting item's hover tooltip; (g) whether native code writes the recovery-time and effect-time row data. **Why M1–M3 were not enough:** the layouts and the class library show which rows exist and what data each provider writes, but not what the native widgets and formatting natives do with it; no published source describes the examination system; the running client is not available to this session (owner rows §17 cover the visual checks). Read: the fill routines of the widget types `ExaminationSkillMaxTargets`, `…CastingTime`, `…Damage`, `…VitalCost`, `…Pip`, `…RemovedGambits`, `…SpecialClass`, `…Trait`, `…Materials`, `…MaxRange`, `…Qualifier`, `…SpellcraftMod`, `ExaminationEffectResistance`, the recovery-time and effect-time timer fills; the duration formatter natives (packages 385 and 416), their shared inner formatter, unit and number helpers and two float constants; the timer widget's text routine; the kind-3 number argument native (package 199 function 8) and the StringInfo validity and emptiness natives (functions 25, 26); the bit-to-key list builder; the skill display's icon routine; the compare-kind routine for trait- and skill-granting items with the layout selector it calls. Nothing from network, login, launcher or anti-tamper code. |
| **Design** | Tessera rules, marked **Tessera rule**: what Tessera does where the original's behaviour depends on server state, live actor state, or was not traced. Not claims about the original. |

| # | Fact | Source | Mark |
|---|---|---|---|
| ST1 | 5,201 skills name the hover and one-click pair (layout 0x22000000, element `Skill_ExamineLayout`); the other 8,046 name none and are exactly the skills with `Skill_IsMonsterSkill`; no skill names a comparison pair (§3.1) | OBS (all 13,247 skills) | Confirmed 2026-10-10 |
| ST2 | 28,400 effects name the hover and one-click pair `Effect_ExamineLayout` of 0x22000000; 4,571 name none (§3.1) | OBS (all 32,971 effects) | Confirmed 2026-10-10 |
| ST3 | Both elements are `Tooltip_New` frames with one `ExaminationListBox` (max width 229); `Skill_ExamineLayout` has 38 entry templates, `Effect_ExamineLayout` 4; their row templates, widget types, formats, colours and states (§4) | OBS | Confirmed 2026-10-10 |
| ST4 | Skill row providers derive from class 1079, which finds the skill to examine: the examined record itself, or the item's `Usage_SkillToExecute` when the row has `UI_Examination_Tooltip_For_SkillItem` (§6.1) | BC (class 1079) | Confirmed (bytecode) 2026-10-10 |
| ST5 | Content rules of the skill rows: sources, formats and variables, visibility (§7) | BC (classes of §7); BIN (a) | Confirmed (bytecode) + BIN |
| ST6 | Name state `Skill_ColorName` for skills and `Effect_ColorName` for effects; name text `Skill_Name`, `Effect_Name` (§7.1, §8.1) | BC (class 1073; class 827 and 698 accessors); OBS (`ColorNameText` states) | Confirmed (bytecode) + Confirmed data |
| ST7 | Examination icon: a skill's `Skill_SmallIcon`, an effect's `Effect_Icon`, never an item stack (§11.1) | BC (class 1067; skill attribute ↔ property correlation OBS2: the attribute read is equal to `Skill_SmallIcon` in 13,247 of 13,247 skills and to `Skill_Icon` in 13,008) | Confirmed (bytecode + OBS2) |
| ST8 | Skill displays choose `Skill_Icon`, `Skill_LargeIcon` or `Skill_SmallIcon` by `UI_SkillDisplay_IconSize` (absent: Small); travel-link skills may use the link's icon; the display sets `UI_Examination_Tooltip_DID_Override` to the skill (§11.2, §12.1) | BIN (e); OBS (layouts, mapper 0x2300013A) | BIN + Confirmed data |
| ST9 | Effect rows: resistance, description, time; the resistance and time rows need a live effect named by the request's `UI_Examination_Tooltip_EffectID` and `…_EffectInstanceID` (§8) | BC (classes 2015, 1061, 1064); BIN (a) | Confirmed (bytecode) + BIN |
| ST10 | Effect lines: composition, order, states, the applied/definition modes, the procedural flag, modifier-line templates and value formatting, overrides, the chance, combat-only and duration lines (§9) | BC (classes 698, 734, 1341, 1392, 1062, 1061); BIN (c) | Confirmed (bytecode) + BIN |
| ST11 | A modifier line's state is `debuffed` when the effect is harmful and `buffed` otherwise: the bit the code tests (0x02000000 of the effect's flags attribute) is set in exactly the 9,143 effects with `Effect_Harmful` true (§9.3) | BC; OBS2 (all 32,971 effects) | Confirmed (bytecode + OBS2) |
| ST12 | The "no duration" test of the duration line is bit 0x40000000 of the same attribute; it is set in exactly the 10,295 effects without an interval, pulse count or permanent flag that do not use a progression interval (§9.5) | BC; OBS2 | Confirmed (bytecode + OBS2) |
| ST13 | Item effect lists: equipper effects, the item's own `Mod_Array` wrapped in the WEENIECONTENT effect `PropertyModEffect`, effects by equipment category, target effects; evaluated at the item level (§9.7) | BC (class 1046 functions 44, 56, 64; class 1062) | Confirmed (bytecode) 2026-10-10 |
| ST14 | Duration texts: units of 30 days, days, hours, minutes, seconds; two templates; options for tenths, two units and lower zeros; the timer widget's choices from `UIComponent_Timer_*` (§10) | BIN (b); OBS (template tokens and variables) | BIN + Confirmed data |
| ST15 | Numbers in modifier and chance lines are kind-3 arguments with decimal places 3 and 0, radix 10, flags 2 (§9.4) | BC (literal operands); BIN (c); `string-table.md` §9.4.3 | Confirmed (bytecode) + BIN |
| ST16 | Effect resistance text: the display text of the lowest set category bit in the ResistCategory mapper (EMAPPER key 0x1000000D); spellcraft with no decimals (§8.2) | BIN (a), (d); OBS (map entries) | BIN + Confirmed data |
| ST17 | SkillGrantingItem comparison: the item record's `Item_GrantedSkill` names a skill whose **hover** tooltip is attached; TraitItem the same with `Item_GrantedTrait`; census 1,262 of 1,262 kind-7 items name a skill with a hover pair (§12.4) | BIN (f); OBS (all items) | BIN + Confirmed data |
| ST18 | Skill-item rows: 13 row templates with `UI_Examination_Tooltip_For_SkillItem`, based on the skill rows, in nine item layouts (14 in the two instrument layouts); 1,194 items name a skill in `Usage_SkillToExecute`, all of it with a hover pair (§12.3) | OBS (layouts, all items) | Confirmed 2026-10-10 |
| ST19 | Recovery-time row: a timer showing the total recovery time or the counting remaining time, wrapped in one of two formats (§7.15) | BC (class 1085); BIN (a), (g) | Confirmed (bytecode) + BIN |
| ST20 | Census of names, descriptions, icons and row sources over every skill and effect (§14) | OBS | Confirmed 2026-10-10 |
| ST21 | The Tessera rules (record-based rows, viewer from the scenario feed, procedural-flag default, damage and class-mechanics rows empty in P2, effect-list header, evaluation levels) | Design | Decision |
| ST22 | 29 effect classes replace the line composition of the effect base class; how the 28,400 effects with a tooltip split by the class whose composition they use (§9.2) | BC (method tables of every effect class); OBS (all effects) | Confirmed (bytecode) 2026-10-10 |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Examination tooltip, row, row template, widget type, format string, variable ID** | As `item-tooltips.md` §2. |
| **Provider** | The class-library class that fills one row's data (`item-tooltips.md` §6.1). Skill rows have their own providers (§6); the name, icon, generic and effect rows share the item providers. |
| **Request properties** | Properties the requesting UI element puts on the tooltip (for example the examined DID, an effect's runtime IDs, a cooldown); providers read them before the examined object (`item-tooltips.md` §3.3 step 4). |
| **Record path / live path** | A provider can fill a row from the examined **record** (a DID) or from a **live** object of the session (an actor's own skill, an applied effect). Tessera's sandbox has records only, plus what the scenario feed reports (§13). |
| **Viewer** | The local character (`item-tooltips.md` §2): its level and maximum vitals enter some rows. |
| **Effect lines** | The list of texts an effect contributes to a tooltip (§9). Each line is a StringInfo with a **line state** (§9.3). |
| **Definition mode / applied mode** | Effect lines of an effect as offered by a skill or an item (definition) or as currently held by an actor (applied) (§9.1). |
| **Evaluation level** | The integer at which an effect's progressions and modifier entries are evaluated (`combat-data.md` §1 "spellcraft", §5.6). |

---

<a id="s3"></a>
## 3. Choosing the tooltip

<a id="s3-1"></a>
### 3.1 Properties and census (Confirmed, ST1, ST2)

| Family | Records | With hover and one-click pair | Layout, element | Without | Comparison pair |
|---|---|---|---|---|---|
| Skills (class 827) | 13,247 | 5,201 (hover pair equal to one-click pair in all) | 0x22000000, `Skill_ExamineLayout` | 8,046, all with `Skill_IsMonsterSkill` (and every skill with it lacks the pair) | none |
| Effects (class 698 and descendants) | 32,971 | 28,400 (equal pairs) | 0x22000000, `Effect_ExamineLayout` | 4,571 | none |

No skill or effect has `UI_Examination_Tooltip_Compare_Type`.

<a id="s3-2"></a>
### 3.2 Selection (BIN, `item-tooltips.md` IT3)

The rule of `item-tooltips.md` §3.3 applies unchanged. The purpose codes the client's
record selector uses are: comparison → the `…_Compare_…` pair, **hover** → `…_0Click_…`,
one-click → `…_1Click_…` (BIN (f); any other code gives no pair, and codes other than
these and 1 log an error). A skill or effect without the
pair has **no tooltip**.

The examined DID is the record named by the request (`UI_Examination_Tooltip_DID`, or
`UI_Examination_Tooltip_DID_Override` when the requesting element carries it, §12.1).

<a id="s3-3"></a>
### 3.3 Request properties read by skill and effect rows (BC, BIN)

| Property | Type | Written by | Read by |
|---|---|---|---|
| `UI_Examination_Tooltip_DID_Override` | DID | skill displays (BIN (e)): the shown skill | §3.2 |
| `UI_Examination_Tooltip_EffectID` | int | effect-bar icons (`combat-ui.md` §8.1) | §8.2, §8.3, §8.4 |
| `UI_Examination_Tooltip_EffectInstanceID` | int64 | effect-bar icons: the instance ID of the object holding the effect | §8.2–§8.4 |
| `ExaminationElement_Skill_RecoveryTime_Total`, `…_Remaining` | float seconds | the recovery provider's live path (§7.15) | §7.15 |
| `UI_SkillDisplay_SkillMinLevel` | int | skill-list code (three native writers, not followed) | §7.14 |
| `Advancement_Level` | int | the request, when given | §7.9 (the level of the damage text) |
| `Item_Level` | int | the request, when given, else the item | §9.7 |

---

<a id="s4"></a>
## 4. Structure (Confirmed, ST3)

<a id="s4-1"></a>
### 4.1 Frames

Both elements have the structure of `item-tooltips.md` §4.1: a `Tooltip_New` frame
(239 × 150 designed) with one child `Examination_AutoResizeListbox` (`ExaminationListBox`,
at (5, 5), `max_width` 229, `min_width` 20, autofill true). Assembly (empty rows, separators,
shrinking, stretching) is `item-tooltips.md` §5.

<a id="s4-2"></a>
### 4.2 Row order

`Skill_ExamineLayout` (38 entry templates, top to bottom): `NameIcon_Row_Base`, separator,
`Skill_QualifierRange_Row_Base`, `Skill_MaxTargets_Base`, `Skill_SphereRadius_Base`,
`Skill_ArcRadius_Base`, `Skill_CastingTime_Base`, `Skill_ResistCategory_Base`,
`Skill_SkillType_Base`, separator, `Skill_Description_Base`, separator, `Skill_Damage_Base`,
separator, `EffectDisplay_Base`, `PositionalEffectDisplay_Base`, `UserEffectDisplay_Base`,
separator, `Skill_VitalCost_Base`, `Skill_AppliedGambit_Base`, `Skill_Pip_Base`,
`Skill_RequiredGambit_Base`, `Skill_RemovedGambits_Base`, `Skill_CostPip_Base`,
`Skill_RequiresPip_Base`, `Skill_SpecialClass_Base`, `UserCriticalHitEffectDisplay_Base`,
separator, `TargetCriticalAndSuperCriticalHitEffectDisplay_Base`,
`TargetCriticalHitEffectDisplay_Base`, `TargetSuperCriticalHitEffectDisplay_Base`,
`Skill_Trait_Base`, `Skill_MinLevel_Base`, `Skill_Materials_Base`,
`ItemSummonerMaterialCost_Base`, `ItemSummoneeMaterialCost_Base`, separator,
`Skill_RecoveryTime_Base`.

`Effect_ExamineLayout` (4): `NameIcon_Row_Base`, `Effect_ResistanceCategory_Base`,
`Effect_Description_Base`, `Effect_TimeDisplay_Base`.

Implementations read the lists from the layout; these are for tests.

<a id="s4-3"></a>
### 4.3 Row templates

All are top-level elements of the examination layout. Formats are tokens of the
`Examination` table (key 0x10000053 of the named-string-table map, 0x250001AF here) with the
variable IDs the table lists. Every text row is outlined, vertically centred, left margin 4,
font 0x42000001, `UICore_Text_fit_to_text` = Parent, tag colour 0xFFCC9933 unless stated,
and has the states `enchanted` (0xFFFF7F00) and `pending` unless stated. "Class" is the
provider (§6–§8); "Rule" the section.

| Row template | Widget type | Class | Format (variables) | Base colour, other states | Size | Rule |
|---|---|---|---|---|---|---|
| `NameIcon_Row_Base` (`Icon_Base`, `Name_Base`) | as `item-tooltips.md` §4.2 | 1067, 1073 | `Name_Base` 0x05C26945 (0x52615) | name colour by state (§7.1) | 180 × 48 | §7.1, §11.1 |
| `Skill_QualifierRange_Row_Base` | `ExaminationRow`, left `Skill_Qualifier_Base`, right `Skill_MaxRange_Base` (right-justified, one line), stretch wide | — | — | — | 200 × 18 | §7.2 |
| `Skill_Qualifier_Base` | `ExaminationSkillQualifier` | 1841 | none (the text is shown as is) | 0xFF666666; no states | 229 × 18 | §7.2 |
| `Skill_MaxRange_Base` | `ExaminationSkillMaxRange` | 2386 | 0x0640A225 (0x5A6195) | 0xFF666666, right margin 4 | 229 × 18 | §7.3 |
| `Skill_MaxTargets_Base` | `ExaminationSkillMaxTargets` | 1083 | 0x026E2AF3 (0x5A6195) | 0xFF666666 | 229 × 18 | §7.4 |
| `Skill_SphereRadius_Base`, `Skill_ArcRadius_Base` | `ExaminationSkillGeneroProperty` on `Skill_AEDetectionVolume_SphereRadius`, `…_ArcRadius` | 1081 | 0x02494823 (0x5A6195) | 0xFF666666 | 229 × 18 | §7.5 |
| `Skill_CastingTime_Base` | `ExaminationSkillCastingTime` | 2406 | none | 0xFF666666; no states | 229 × 18 | §7.6 |
| `Skill_ResistCategory_Base` | `ExaminationSkillGeneroProperty` on `Skill_Resist_Category` | 1081 | 0x0AB3AC49 (0x5A6195) | 0xFF666666 | 229 × 18 | §7.7 |
| `Skill_SkillType_Base` | `ExaminationSkillGeneroProperty` on `Skill_DisplaySkillType` | 1081 | 0x07BFA695 (0x5A6195) | 0xFF666666 | 229 × 18 | §7.7 |
| `Skill_Description_Base` | `ExaminationGeneroProperty` on `Skill_Desc`, `DisplayDefault` false, markup on | 1066 | 0x0751907E (0x5A6195) | 0xFFCCCC7F, right margin 6 | 229 × 18 | §7.8 |
| `Skill_Damage_Base` | `ExaminationSkillDamage`, markup off | 1078 | 0x056AAF35 (0x48615B5) | 0xFFC31C1F, right margin 6 | 229 × 18 | §7.9 |
| `EffectDisplay_Base` | `ExaminationEffectList`, target 3 (`Target`), category 0x10; entries `EffectListElement` | 1062 | none | — | 220 × 0 | §9.6 |
| `PositionalEffectDisplay_Base` | as above, target 4, entries `UserEffectListElement` | 1062 | header 0x0A5DBE9C | — | 220 × 0 | §9.6 |
| `UserEffectDisplay_Base` | target 1 (`User`) | 1062 | header 0x0F345375 | — | 220 × 0 | §9.6 |
| `UserCriticalHitEffectDisplay_Base` | target 2 | 1062 | header 0x0753CBF4 | — | 220 × 0 | §9.6 |
| `TargetCriticalAndSuperCriticalHitEffectDisplay_Base`, `TargetCriticalHitEffectDisplay_Base`, `TargetSuperCriticalHitEffectDisplay_Base` | targets 7, 5, 6 | 1062 | headers 0x0F692314, 0x0C30D3D4, 0x04C81704 | — | 220 × 0 | §9.6 |
| `Skill_VitalCost_Base` | `ExaminationSkillVitalCost`, markup on | 1088 | 0x040EC744 (0x8603E64) | 0xFF00E500 | 229 × 18 | §7.10 |
| `Skill_AppliedGambit_Base`, `Skill_RequiredGambit_Base` | composite: a label (states `pending`, `cantuse` red) and a `GambitSlotList` (entries of layout 0x220006F6) | 2290, 2291 | — | 0xFF00E500 | 229 × 18 | §7.12 |
| `Skill_Pip_Base` | `ExaminationSkillPip` | 1084 | 0x098E2BE0 (0x54E0) | 0xFFC3E81F | 229 × 18 | §7.12 |
| `Skill_RemovedGambits_Base` | `ExaminationSkillRemovedGambits` on `Skill_RemovedGambits` | — | 0x0BDDB973 | 0xFF00E500 | 229 × 18 | §7.12 |
| `Skill_CostPip_Base`, `Skill_RequiresPip_Base` | composite: text and 16 × 16 icon fields | 2292, 2293 | 0x01954CA9 (0x484893); 0x0A84CE19, 0x085165E9 (0xA69E3C3) | 0xFFC3E81F | 256 × 16 | §7.12 |
| `Skill_SpecialClass_Base` | `ExaminationSkillSpecialClass` | 1086 | 0x066209B3 (0x480683) | 0xFF009900 | 229 × 18 | §7.11 |
| `Skill_Trait_Base` | `ExaminationSkillTrait` | 1087 | 0x0E276F84 (0x5965E4) | 0xFFC1C1C1; `pending`, `cantuse` 0xFFFF0000 | 229 × 18 | §7.13 |
| `Skill_MinLevel_Base` | `ExaminationGeneroProperty` on `UI_SkillDisplay_SkillMinLevel`, `DisplayDefault` false | 1066 | 0x02CA2A4C (0x5A6195) | 0xFFFF0000 | 229 × 18 | §7.14 |
| `Skill_Materials_Base` | `ExaminationSkillMaterials` | 1082 | 0x03469953 (0x8A68573) | 0xFF009900; `missing` 0xFFFF0000 | 229 × 18 | §7.12 |
| `ItemSummonerMaterialCost_Base`, `ItemSummoneeMaterialCost_Base` | item summon-cost rows | 1951, 1950 | 0x00ECB2C3, 0x00EB22C3 | 0xFF7F007F, tag 0xFFFFFFFF; no states | 146 × 18 | §7.12 |
| `Skill_RecoveryTime_Base` | `ExaminationSkillRecoveryTime` (a timer), `UIComponent_Timer_ShowTwoPlaceFormat` true | 1085 | 0x0EDBB215 (0x58E15) as designed; §7.15 | 0xFF009900 | 229 × 18 | §7.15 |
| `Effect_ResistanceCategory_Base` | `ExaminationEffectResistance` | 2015 | 0x0637D315 (0x689C009, 0x96A2908) | 0xFFC3B51F | 220 × 18 | §8.2 |
| `Effect_Description_Base` | `ExaminationEffectDescription`; entries `EffectListElement` | 1061 | — | — | 220 × 40 | §8.3 |
| `Effect_TimeDisplay_Base` | `ExaminationEffectTimeDisplay` (a timer), `UIComponent_Timer_ShowFullFormat` true | 1064 | none | 0xFF009900; no states | 220 × 40 | §8.4 |

**Line elements** (entries of the effect lists, `Text`, markup on, right margin 6, height
follows the text):

| Element | Base colour | `buffed` | `debuffed` | `duration` | Tag colour |
|---|---|---|---|---|---|
| `EffectListElement` | 0xFFCCCC7F | 0xFF00E500 | 0xFFFF0000 | 0xFF009900 | 0xFF009900 |
| `UserEffectListElement` | 0xFF00E500 | 0xFF99FF00 | 0xFFFF0099 | 0xFF99FF00 | 0xFFCC9933 |

The rows `ExaminationSkillSpellcraftMod` (class 2308, format variable 0x5A6195) and
`ExaminationPassiveEffectList` exist as widget types but no element of these two tooltips uses
them.

> **OPEN:** (implementer, 2026-10-10) the table of the tokens of §7–§9 that are not formats of
> a row (the qualifier, maximum-target, casting-time, vital-cost, special-class, recovery wrap,
> modifier-template, value-text, chance, combat-only and duration-line tokens): this section
> names the `Examination` table for formats only. The implementation reads all of them from
> the `Examination` table (key 0x10000053); the duration templates and number tokens of §10.1
> from the `UI` table as stated there.

---

<a id="s5"></a>
## 5. Assembly

`item-tooltips.md` §5 (and its OPENs) apply unchanged. Note that several skill rows are
empty in most tooltips (§14.3), so the separator rules matter: a skill without damage and
effects shows the name, the first group, the description and the recovery time with single
separators between them (example §16.1).

---

<a id="s6"></a>
## 6. The skill providers

<a id="s6-1"></a>
### 6.1 Which skill (BC, ST4)

Every skill row except the name, icon, description, minimum-level and summon-cost rows is
filled by a child of class 1079. Before filling, the provider finds the **skill** *K*:

1. If the row template has `UI_Examination_Tooltip_For_SkillItem` true, the examined object
   is an item: *K* = the item's `Usage_SkillToExecute` (record path: from the item record's
   property set; live path: from the instance). No such property, or 0 → the row is empty.
2. Otherwise *K* is the examined record (or live skill).

Then the row is filled from *K*. The description row is a plain generic row (`item-tooltips.md`
§6.4) on the examined object, so in an item tooltip it shows the **item's** `Description`
row instead (the skill-item layouts have no `Skill_Description_Base`, §12.3).

<a id="s6-2"></a>
### 6.2 Record and live paths

The providers have a record path (examined DID) and a live path (the viewer's own skill
object, which applies the viewer's modifiers, trait bonuses and cooldowns). Which path the
client takes for which requester was not traced (OPEN, §17 Q1).

> **Tessera rule (ST21).** Every skill row is filled from the **record** *K* and the
> **viewer** of §13 (level, maximum vitals). Actor modifier lists (`Skill_*_Mod_Array`,
> `…_ModifierList`, `…_Override`, `…_AdditiveArray`, `combat-data.md` §3.3) contribute
> nothing; the recovery row uses the feed's cooldown (§7.15).

---

<a id="s7"></a>
## 7. Skill rows

<a id="s7-1"></a>
### 7.1 Name (BC class 1073; BIN, `item-tooltips.md` §6.2)

Text: `Skill_Name` of *K*. State: `Skill_ColorName` (the provider's object state for a skill);
its colour is the table colour of `item-tooltips.md` §6.2 (0xFFC3B585 here). The five-font fit
rule of `item-tooltips.md` §6.2 step 3 applies.

**Travel skills.** When *K* has a non-zero `Skill_TravelLinkToUse` (16 skills here), the
client asks the viewer's travel link for a destination; if one exists the name becomes a
`Travel`-table format (key 0x10000042, token 0x056CDE3C, variable 0x0E302115 = the
destination), and for a link type 3 token 0x06EFEF04.

> **Tessera rule.** Travel skills show `Skill_Name` (the sandbox knows no travel links).

<a id="s7-2"></a>
### 7.2 Qualifier (BC class 1841; BIN (a))

Left child of `Skill_QualifierRange_Row_Base`. The provider builds one StringInfo *Q* and the
widget shows it as is.

1. *L* = the skill's qualifier list (a compiled list of `DamageQualifier` keys; Inferred: the
   distinct non-zero `Skill_AttackHook_DamageQualifier` values of the skill's attack hooks;
   §14.3 gives their census).
2. Let *F* = `Skill_IgnoresResetTime` or `Skill_Immediate` (true when either is), and *I* =
   `Skill_Immediate`.
3. If *L* is empty: *Q* = token 0x0A9A9C99 when *F* and *I*, 0x099F1219 when *F* and not *I*;
   otherwise *Q* is empty.
4. If *L* has entries: *T* = the display text of *L*[0] in the mapper of EMAPPER key 0x1000005E
   (`DamageQualifier`, 0x23000029 here); for each further entry, *T* = token 0x041F53AE with
   0x50E84 = *T* and 0x0ACA64B5 = that entry's display text. Then *Q* = *T*, or, when *F*,
   token 0x00DFAA95 (*I*) or 0x06552994 (not *I*) with 0x50E84 = *T*.
5. An empty *Q* → empty child.

> **OPEN:** the property behind the compiled qualifier list (step 1). **Tessera rule:** as
> stated in step 1, in hook order.

<a id="s7-3"></a>
### 7.3 Maximum range (BC class 2386; BIN (a))

The provider computes the viewer-adjusted maximum range *R* (a class-827 function of the skill
and the viewer). *R* ≤ 0 → empty. Otherwise the row data holds `Examination_MaxRange` = *R* and
the widget fills its format with 0x5A6195 = *R* as a **kind-2 float** with 1 decimal place when
*R* has a fraction and 0 otherwise (`string-table.md` §9.4.2).

> **Tessera rule:** *R* = `Skill_MaxRange` (−1 and 0 → empty).

<a id="s7-4"></a>
### 7.4 Maximum targets (BC class 1083; BIN (a))

*N* = `Skill_AreaEffectMaxTargets`. Absent → the row is empty. If *N* > 1: the row data's
`Examination_StringInfo` = token 0x0B226945 with 0x5A6195 = *N*, and the widget shows the
row's format with 0x5A6195 = that StringInfo. *N* ≤ 1 → empty.

<a id="s7-5"></a>
### 7.5 Sphere and arc radius (BC class 1081 → class 1066)

Generic rows (`item-tooltips.md` §6.4) on *K*: a value equal to the dictionary default (0.0)
is empty; otherwise one decimal (`FloatPrecision` default 1).

<a id="s7-6"></a>
### 7.6 Casting time (BC class 2406; BIN (a), (b))

1. If *K* has an induction (`Skill_InductionAction` ≠ 0, a class-1195 object): *D* = the
   induction duration for the viewer (the object's `Induction_Duration` with its modifier
   lists, times a viewer factor; class-1195 and -1191 functions). Text = the induction
   object's `Induction_SkillTooltipOverride` when it has one (no induction object of this
   install does), else token 0x0875B0DE.
2. Else if *K* is a channelled toggle (toggle, §7.11, with `Skill_Toggle_ChannelingState` ≠ 0):
   *D* = `Skill_Toggle_ChannelingDurationOverride` when > 0, else the channel object's duration
   for the viewer (`Channeling_Duration` × its multiplier property); text = token 0x03081A4E.
3. Else the row is empty.
4. Variable 0x58E15 = the duration text of *D* (§10.1) with tenths on, one unit, the short
   template. The widget shows the StringInfo as is.

> **Tessera rule:** *D* = `Induction_Duration` (step 1) or `Channeling_Duration` (step 2)
> without modifiers.

<a id="s7-7"></a>
### 7.7 Resistance category and skill type (BC class 1081 → 1066; `item-tooltips.md` §6.4)

Generic rows on `Skill_Resist_Category` (32-bit bitfield, mapper 0x23000064) and
`Skill_DisplaySkillType` (any-width bitfield, mapper 0x230003D4): the list text of the set
bits (`item-tooltips.md` §6.4 step 7 and its any-width OPEN); 0 (the default) → empty.

<a id="s7-8"></a>
### 7.8 Description

A generic row on `Skill_Desc` of the examined object (not *K*, §6.1). `Skill_Desc` is a
StringInfo; an entry that renders empty → empty row (`item-tooltips.md` §6.4 step 4).

<a id="s7-9"></a>
### 7.9 Damage (BC classes 1078, 827; BIN (a))

The row data's StringInfo *S* is built by a skill function from the request's
`Advancement_Level` (else the viewer's) and the request properties:

1. Count the attack hooks that carry damage (*h*). *h* = 0 → *S* empty (row empty).
2. When *h* > 1, *S* starts as token 0x04381BC3 with 0x0A1D4733 = *h*.
3. For each damaging hook in order: its damage text *d* (one of three skill functions,
   chosen by whether the hook uses an implement and which; they fill the damage variables
   0x2864455, 0x48615B5, 0x5BE1855 of `item-tooltips.md` §6.12 from the request's
   `ExaminationElement_Skill_*Weapon_Damage*`, `…_DamageType`, `…_ActionDuration`,
   `…_TacticalDPS` properties and the hook's own damage, scaled by a PvP factor) is wrapped as
   token 0x0F616255 with 0x0E0F7997 = *d*; it is joined to *S* by token 0x0B38FBFE with
   0x4D43E1 = *S* and 0x4D43E2 = the wrapped text.
4. The widget shows the row's format with 0x48615B5 = *S*.

> **OPEN:** the three hook-damage functions and the request's weapon properties (they need
> the viewer's weapon and stat state, which the client receives from the server).
> **Tessera rule (P2):** the damage row is built **empty**; the census counts it (§14).

<a id="s7-10"></a>
### 7.10 Vital cost (BC class 1088; BIN (a))

For each vital type *v* = 1, 2, 3, 4 (mapper `VitalType`, EMAPPER key 0x10000025, 0x23000003
here), when *K* has a cost list:

1. cost = round((*M*ᵥ × percentᵥ + pointsᵥ) × factorᵥ), where *M*ᵥ is the viewer's maximum of
   vital *v*, percentᵥ the skill's `Skill_Vital_Percent` for *v*, pointsᵥ its
   `Skill_Vital_Points` (or `Skill_Vital_Points_Progression` at the level) and factorᵥ a
   multiplier from the viewer's modifiers (class-827 functions).
2. cost ≤ 0 → no line for *v*.
3. Line = token 0x078CA1B4 with 0x48484 = cost (integer) and 0x5AE85C = the display text of *v*.
4. Lines are joined with token 0x05BA07CE (0x603E6F1 = text so far, 0x603E6F2 = the new
   line).

Then, when *K* has a per-second cost list (`Skill_Toggle_VitalCostPerSecondList`), the same
for each *v* with the per-second cost and token 0x0D808C44 (same variables). The widget shows
the row's format with 0x8603E64 = the joined text.

> **Tessera rule:** factorᵥ = 1; pointsᵥ evaluated at the viewer's level (`combat-data.md`
> §4.10); *M*ᵥ from the feed's vitals (§13), 0 when unknown; round half away from zero.
> `Skill_Vital_Consumes_All` entries show no cost line.
>
> **OPEN:** the rounding native (package 122 function 30) and `Skill_Vital_Consumes_All`.
>
> **OPEN (implementer):** which of `Skill_Vital_Points` and `Skill_Vital_Points_Progression`
> wins when an entry has both. On this install every entry with a non-null progression also
> stores `Skill_Vital_Points` = 0, so reading the constant first shows no cost line for any of
> them. Tessera evaluates a non-null progression at the viewer's level and falls back to
> `Skill_Vital_Points` only when there is none (or it does not evaluate).

<a id="s7-11"></a>
### 7.11 Special class (BC classes 1086, 827)

If *K* is a toggle skill (its compiled toggle data is present; OBS2: exactly the skills that
store `Skill_Toggle_Hook_Number`): text = token 0x042D933C when `Skill_Toggle_ChannelingState`
≠ 0, else 0x0EA960CC. Otherwise empty. The widget fills 0x480683 of the row's format.

<a id="s7-12"></a>
### 7.12 Class-mechanics rows (P2: empty)

The pip, gambit, pip-cost, required-pip, materials and summon-cost rows have providers that
read the viewer's class state (pips, gambit slots, crafting materials, summoning costs of
items).

> **Tessera rule (P2):** these rows are built **empty** (class mechanics are a later
> milestone, `combat-data.md` §7.10); the census counts them. `Skill_RemovedGambits_Base` names
> `Skill_RemovedGambits` as its generic property but has its own widget type; it is also built
> empty in P2.

<a id="s7-13"></a>
### 7.13 Required trait (BC class 1087; BIN (a))

The provider writes the required trait's text and, when the viewer lacks it, state
`cantuse`; for a trait that is a passive skill it sets `Examination_TraitIsPassiveSkill`, and
the widget then uses token 0x0E102ECC instead of the row's format when the state is
`cantuse`. Otherwise the row's format with 0x5965E4 = the text.

> **Tessera rule:** text = the `Trait_Name` of `Skill_RequiredTrait` (0 → empty); state
> `cantuse` unless the feed reports the trait (never in P2).
>
> **OPEN:** (implementer, 2026-10-10) which traits count as passive skills (`Examination_TraitIsPassiveSkill`,
> token 0x0E102ECC) is not given. The implementation never sets it: the row always uses its
> format.

<a id="s7-14"></a>
### 7.14 Minimum level

A generic row on `UI_SkillDisplay_SkillMinLevel`, a **request** property (no skill record
holds it); empty without it.

> **Tessera rule:** never set in P2.

<a id="s7-15"></a>
### 7.15 Recovery time (BC class 1085; BIN (a), (g); ST19)

Row data: `ExaminationElement_Skill_RecoveryTime_Total` *T* and `…_Remaining` *r* (floats,
seconds). The record path copies both from the request (and leaves the row empty when *T* ≤ 0
or either is missing); the live path computes *T* = the viewer's recovery time of the skill
(`Skill_SkillRecoveryTime` with its modifier list) and *r* = the remaining time of the viewer's
cooldown for it (its recovery channel included).

The widget (a timer): when *T* ≥ 0,

- *r* = 0: wrap string = token 0x0EDBB215, the timer shows *T* (static);
- *r* ≠ 0: wrap string = token 0x085F0B57, the timer counts down from *r*.

The wrap string's variable 0x58E15 receives the timer text (§10.2; this row has
`ShowTwoPlaceFormat` true and no full format).

> **Tessera rule:** *T* = `Skill_SkillRecoveryTime` of *K*; *r* = the remaining time of a
> running feed cooldown of *K* (`CooldownStarted`, `combat-ui.md` TR35), else 0. *T* ≤ 0 →
> empty row.
>
> **OPEN:** (implementer, 2026-10-10) a recovery time below one second: the timer's default
> format (§10.2 step 3, no tenths) shows no unit, so every unit variable is empty. What the
> original shows then is not given. The implementation leaves the row empty; on the observed
> install that is 192 of the 3,394 skills with a recovery time (3,202 rows are shown).

---

<a id="s8"></a>
## 8. Effect rows

<a id="s8-1"></a>
### 8.1 Name and icon (BC classes 1073, 1067)

Text: `Effect_Name`; state `Effect_ColorName` (0xFFC3B585 here). Icon: §11.1.

<a id="s8-2"></a>
### 8.2 Resistance (BC class 2015; BIN (a), (d); ST16)

The provider needs the **live** applied effect: the request's `UI_Examination_Tooltip_EffectID`
(the effect's runtime ID) and `…_EffectInstanceID` (the instance ID of the object holding it;
equal to the viewer's ID → the viewer's effects). No such effect → empty row. Otherwise it
writes `Examination_Effect_ResistanceCategory` = the effect's `Effect_Resist_Category` and
`Examination_Effect_ResistanceSpellcraft` = the applied effect's spellcraft.

The widget: category text *C* = the display text of the key of the **lowest** set bit (key =
bit index + 1) of the category in the mapper of EMAPPER key 0x1000000D (`ResistCategory`,
0x23000064 here); bits without a display text are skipped. No text → **empty row**. Otherwise
the format 0x0637D315 with 0x689C009 = *C* and 0x96A2908 = max(0, spellcraft) as a kind-2
float with 0 decimals.

> **Tessera rule:** the live effect is the feed's effect instance (§13); spellcraft = the
> `EffectApplied` level when the feed gives one, else the viewer's level.

<a id="s8-3"></a>
### 8.3 Description (BC class 1061)

The effect lines of the effect in **applied mode** (§9.1) fill an `Examination_EffectInfo_Array`;
each line becomes one `EffectListElement` with its line state (§9.3).

- With a live effect (request IDs set and found): its lines at its own spellcraft and property
  source.
- Without (both IDs 0): the provider makes a temporary applied effect of the record with
  spellcraft −1 and the viewer as caster, and uses that.
- The record cannot be read → empty.

<a id="s8-4"></a>
### 8.4 Time (BC class 1064; BIN (a), (g))

Needs the live effect (as §8.2); else empty. Row data: `Examination_Effect_StartTime` (the
effect's start), `Examination_Effect_Duration` (its duration) and
`Examination_Effect_NeedsTimer` (true when it has a finite duration). `NeedsTimer` false →
empty row. Otherwise a timer counting down to start + duration, shown with the **full
format** of §10.2 (all units from the largest non-zero one down to seconds). It updates while
shown; there is no wrap string.

> **Tessera rule:** start = the `EffectApplied` arrival, duration = its `duration_ms`; no
> duration → empty row (`combat-ui.md` TR30).

---

<a id="s9"></a>
## 9. Effect lines (BC, BIN; ST10–ST13, ST15)

<a id="s9-1"></a>
### 9.1 Modes and inputs

An effect's line list is built from: the effect record *E*; the mode (**definition**: an
effect a skill or item offers; **applied**: an effect an actor holds); the evaluation level
*L* (§9.8); the applied effect's property source (`combat-data.md` §6.2: may be absent); the
viewer. The list is ordered; each line is (text, state code).

<a id="s9-2"></a>
### 9.2 Composition

**Description line.** The text is `Effect_Applied_Description` in applied mode and
`Effect_Definition_Description` in definition mode. It is added (state code 2) only when it is
a valid StringInfo (literal, or a table entry that exists) whose rendered text is not empty.

**Effects of class 734 and its descendants** (property modification; `combat-data.md` §6.1):

1. If the procedural flag (below) is false, the list is **empty**: no line at all.
2. Add the description line.
3. Add one **modifier line** per entry of the effect's `Mod_Array` (§9.4), in stored order.
4. If the list is still empty, stop (the class-734 test that would continue returns false).
5. Definition mode only: add the **chance line** (§9.5).
6. Add the **extra lines** of the mode (§9.5).

**Effects of every other class:**

1. Add the description line.
2. Applied mode: if the procedural flag is false, stop.
3. Definition mode: if the list is empty, stop; add the chance line.
4. Add the extra lines of the mode.

**Procedural flag** = `Effect_Display_Procedurally_Generated_Examination_Information`
(boolean, dictionary default **true**; stored false in 547 effects, never stored true).

**Classes with their own composition.** The two list functions of the effect base class
(definition and applied mode) are replaced in 29 effect classes, which add class-specific lines
(vital amounts, applied sub-effects, aura and area targets, tiers, combinations, procs, absorb
amounts and others) and may order the lines differently (the instant-vital class, for example,
puts the chance line before the description). By the class whose functions an effect uses, the
28,400 effects with a tooltip split into: class 734's composition 12,731; the base composition
above 2,808; their own composition 12,861 (classes 755: 2,438, 719: 1,547, 708: 1,245, 767:
1,189, 725: 1,103, 763: 767, 709: 617, 715 and its child: 563, 3866: 526, 3686: 461, 2762:
453, 724: 429, 736: 356, 737: 310, 769: 239, 3842: 132, 756: 123, 3218: 91, 3222: 88, 714:
81, 739: 57, 744: 15, 3124: 14, 2544: 10, 748: 4, 2441: 2, 3943: 1). Three further
descendants of 734 (2459, 713, 3833) replace only the continuation test of step 4.

> **OPEN:** the class-specific lines and order of those 29 classes and the continuation test
> of the three descendants. **Tessera rule:** descendants of 734 use the class-734 composition
> with a false continuation test; every other class uses the base composition (description;
> definition mode: the chance line; extra lines). The class-specific lines are not shown in P2.

> **OPEN:** (implementer, 2026-10-10) the Tessera rule needs the effect's root class and the
> class parents (the class library). A data source without them (synthetic tests) uses the
> base composition for every effect; the item's own `Mod_Array` (§9.7 step 2) always uses the
> class-734 composition, since its template effect is of that class.

> **OPEN (ST21):** whether an absent flag reads true. The client library reads it through a
> property native that, per `items.md` BIN2, keeps the caller's default (0) for an absent
> property. Read that way, no property-modification effect would ever show a line, including
> the template effect that carries every item's own stat lines (§9.7); the data stores the
> flag only as false. **Tessera rule:** an absent flag is true (the dictionary default).

<a id="s9-3"></a>
### 9.3 Line states (BC)

| State code | State label | Used for |
|---|---|---|
| 1 | `buffed` | modifier and chance lines of a non-harmful effect |
| 2 | (none: the element's base colour) | the description line |
| 3 | `debuffed` | modifier and chance lines of a harmful effect (`Effect_Harmful` true, ST11) |
| 4 | `duration` | combat-only and duration lines |

Colours: the line element's (§4.3), `EffectListElement` for target lists and the effect
tooltip, `UserEffectListElement` for user lists.

<a id="s9-4"></a>
### 9.4 Modifier lines (BC classes 1341, 1392; BIN (c); `misc-records.md` §13.1)

For one `Mod_Entry` (empty and field-less entries are skipped, `combat-data.md` §5.2):

1. Evaluate the entry at *L* (`combat-data.md` §5.6) with the property source; an entry that
   is not applicable at *L* yields the operator's identity value. No result → no line.
2. *O* = the entry's `Mod_DescriptionOverride`, if stored. (A next-rank request, used by trait
   displays only, takes `Mod_NextRankDescriptionOverride` when *O* is empty.)
3. *P* = `Mod_Modified`. If *P* has **no property-metadata object** (`misc-records.md`
   §13.1), there is no line. *N* = its `PropertyMetaData_Name`; *pct* = its
   `PropertyMetaData_DisplayAsPercentage` (false when absent).
4. If *O* is absent, a metadata field read by the property-metadata control (class 1392
   function 97 here; Inferred: `PropertyMetaData_HideIdentityExaminationMod`, true in 5,210 of
   5,211 objects) is true and the value equals the operator's identity value
   (`combat-data.md` §5.3), there is no line.
5. **Override line.** If *O* is present: the line is *O*, with its variables filled when it
   uses them: 0x5A6195 = the value text (*pct* off) and 0x06D04635 = the value text (*pct* on)
   (step 8). For an array value the override is used as is. The line is added only if *O*
   renders non-empty.
6. **Generated line.** Otherwise only values of the types string, enum, float, StringInfo,
   64-bit bitfield, int, 32-bit bitfield, int64, boolean and any-width bitfield are shown; any
   other type → no line. **Normalise:** for `Add` and `Subtract` with a negative float, int or
   int64 value, show the other operator with the absolute value.
7. **Template** by the (normalised) operator, all with 0x5A6195 = the value text and
   0x744A2C9 = *N*:

   | Operator (`combat-data.md` §5.3) | Token |
   |---|---|
   | 7 `Add` | 0x0E447774 |
   | 6 `Subtract` | 0x067FC704 |
   | 8 `Multiply`, float value, *pct* on | 0x09C7FA07 when *v* ≥ 1, else 0x09C79707 |
   | 8 `Multiply`, otherwise | 0x07DF4F79 |
   | 5 `Set` | 0x0E440914 |
   | 3 `Or` | 0x0DE44862 |
   | 10 `ArrayAdd` | 0x046C9024 |
   | any other | no line |

8. **Value text:** token 0x0CE39455 (*pct* on) or 0x0F5F0564 (*pct* off), with 0x5A6195 =
   - int, int64: the integer;
   - float, *pct* on: *d* × 100 with *d* per `misc-records.md` §13.1 X4, as a kind-3 number
     with 3 decimal places, radix 10, flags 2 (no grouping, trailing fraction zeros dropped,
     `string-table.md` §9.4.3);
   - float, *pct* off: the value as a kind-3 number, radix 10, flags 2, with 3 decimal places
     (0 for properties whose metadata selects an integer display, OPEN below);
   - boolean: the `Utils` yes or no text (`item-tooltips.md` §6.4 table);
   - enum: the display text of the key; StringInfo: the text; string: the string;
   - bitfields: the list text of the set bits (`item-tooltips.md` §6.4).
9. The line gets the state of the effect (§9.3: 3 when harmful, else 1).

> **OPEN:** which metadata condition selects 0 decimal places for float values (a
> property-metadata function, class 1392 function 96 here, read only in part). **Tessera
> rule:** 3 decimal places always.

<a id="s9-5"></a>
### 9.5 Chance and extra lines (BC class 698; BIN (b), (c); ST12)

**Chance line** (definition mode): unless *E* has
`Effect_SuppressApplicationProbabilityExamination` true, the client computes the application
chance *c* (class 698 function 128: constant, progression, variance and additive modifier
properties). Only when 0 < *c* < 1: token 0x05AF40D9 with 0x04A67A64 = *c* × 100 as a kind-3
number with 0 decimal places, radix 10, flags 2; state by §9.3.

> **Tessera rule:** *c* = `Effect_ConstantApplicationProbability` (absent: 1).

**Extra lines**, in this order:

1. **Combat-only line**, when `Effect_Duration_CombatOnly` is true. Definition mode: token
   0x073C4A39 with 0x48A069 = the removal delay, rounded to an integer, where delay = max(0,
   9.0 + the viewer's `Effect_CombatOnlyRemovalDelay_AddMod`). Applied mode: the same when the
   applied effect's caster is the viewer, otherwise token 0x020EF2F3 (no variables). State 4.
2. **Duration line**, unless *E* has no duration source (the compiled "no duration" bit, ST12:
   no interval, no pulse count, no permanent flag and no progression interval) or is
   permanent (`Effect_Duration_Permanent` true). *D* = the duration of `combat-data.md` §6.2
   at *L* with the property source. *D* ≤ 0 → no line. Otherwise token 0x04FE416E with
   0x0A668AAE = the duration text of *D* (§10.1: tenths on, up to two units, short
   template). State 4.

<a id="s9-6"></a>
### 9.6 Effect-list rows (BC class 1062; ST13)

An `ExaminationEffectList` row names `UI_Examination_Tooltip_SkillEffectTarget` (which list)
and `UI_Examination_Tooltip_EffectCategory` (a bit mask of effect categories). Each effect of
the selected list contributes its lines in **definition** mode (§9.2) at its evaluation level
(§9.8); the row's entries are those lines in order, except that a line which carries an
effect DID (`UIComponent_EffectDID`, set only by class-specific lines, §9.2) whose effect passes
a class-698 test (function 264 here; the attribute it reads was not identified) is moved after
all other lines, keeping the order among the moved lines.

**Skill lists** (target → the skill's effect references, `combat-data.md` §7.7–§7.8):

| Target key (label) | Effects |
|---|---|
| 1 `User` | `Skill_UserEffectList` |
| 2 `UserCriticalHit` | `Skill_CriticalEffectList` |
| 3 `Target` | every attack hook's `Skill_AttackHook_TargetEffectList`, hooks in order |
| 4 `TargetPositional` | the hooks' `Skill_AttackHook_PositionalTargetEffectList` |
| 5 `TargetCriticalHit`, 6 `TargetSuperCriticalHit`, 7 `TargetCriticalAndSuperCriticalHit` | built from the hooks' `…_CriticalTargetEffectList` and `…_SuperCriticalTargetEffectList` by a merge function (below) |

> **OPEN:** the merge used for targets 5–7 (a class-698 list merge whose duplicate test was
> not read). **Tessera rule:** 5 = effects in the critical lists that are not in the
> devastating lists; 6 = the reverse; 7 = effects in both; placeholders (`Skill_Effect` = 0)
> are skipped; `_Override` and `_AdditiveArray` companions are ignored (§6.2).

**Header.** The rows with a format (§4.3 "header") show it as a line above the entries.

> **OPEN:** the header behaviour was not traced (the list widget's fill routine). **Tessera
> rule:** the header is shown, in the row's base text style, only when the list has at least
> one line (the rule `crates/gamedata` already applies to item usage lists).

<a id="s9-7"></a>
### 9.7 Item effect lists (BC class 1046 functions 44, 56, 64; class 1062; ST13)

For an item, the effect list provider builds the item's applied-effect list (or uses the one an
instance carries) from, in order:

1. the effects of `EffectGenerator_EquipperEffectList`, category 0x10;
2. the item's own `Mod_Array`, wrapped in the effect named by the WEENIECONTENT label
   `PropertyModEffect` (0x70000253 here: a class-734 effect with no description and no
   procedural-flag property), category 0x10: its modifier lines (§9.4) are the item's **stat
   lines**;
3. effects that the item's equipment category grants (`Item_EquipmentCategory`, class 1202
   functions; category 0x10 and 0x20);
4. the effects of `EffectGenerator_TargetEffectList`, category 0x20.

A row shows the effects whose category is in its `UI_Examination_Tooltip_EffectCategory`
(`EffectDisplay_Base` 0x10, `TargetEffectDisplay_Base` 0x20, `UsageEffectDisplay_Base` 0x08).
The evaluation level is the request's `Item_Level` (else the item's, default 1) plus an offset
the item derives from `Item_DistributionType` (class 801 functions 421, 429).

> **OPEN:** the equipment-category effects (step 3), the source of category 0x08 (usage)
> effects and the level offset. **Tessera rule:** step 3 adds nothing; category 0x08 = the
> effects of `EffectGenerator_UsageEffectList`; offset 0.
>
> **OPEN:** (implementer, 2026-10-10) the place of the category-0x08 effects in the order of
> steps 1–4 is not given. The implementation lists them after step 4. When the
> `PropertyModEffect` label or its record is missing, the item's entries are described with an
> empty template (no description, flag true, not harmful).

<a id="s9-8"></a>
### 9.8 Evaluation level

For an effect reference with a spellcraft other than −1, *L* = that spellcraft truncated
(`combat-data.md` C49); with −1, *L* = the level the list was given: for items the level of
§9.7, for skills the viewer's level (a class-1062 helper of the viewer). A live applied effect
uses its own spellcraft.

> **Tessera rule:** the viewer's level is the feed's player level (§13); the
> `combat-data.md` §1 OPEN (Q9) on −1 is resolved this way for display only.

---

<a id="s10"></a>
## 10. Duration and timer texts (BIN (b); ST14)

<a id="s10-1"></a>
### 10.1 The duration formatter

Input: seconds *s* (a float) and options. Templates are entries of the `UI` table (key
0x10000065 of the named-string-table map, 0x250001BB here): the **short** template
0x0E38E034 and the **long** template 0x08896F94; both have the five unit variables 0x52438D3
(units of 30 days), 0x486E3 (days), 0x4D4A73 (hours), 0x1E3A8F3 (minutes), 0x79842C3
(seconds), each inside a group the template drops when its variable is empty.

Options: *tenths* (show tenths of a second), *two* (allow a second unit), *all* (every unit
after the first), *pad* (two-digit numbers), *zeros* (show a zero unit after a shown one).

1. If *tenths*: *t* = ⌊frac(*s*) × 10.0 + 0.5⌋ (f32); if *t* = 10, *t* = 0 and carry 1 second.
2. *n* = trunc(*s*) + carry; split into *n* div 2,592,000 (30-day units), then days (86,400),
   hours (3,600), minutes (60) and seconds of the remainders.
3. Walk the units from the largest; *k* = units shown so far. A unit is **shown** when (*k* = 0,
   or *all*, or (*two* and *k* < 2)) and (*k* < 2 or *all*) and (its value > 0, or (*k* > 0 and
   *zeros*)). A shown unit's variable = the number text (UI token 0x093DD17F with 0x5A6195 =
   the integer; with *pad* and a value ≤ 9, token 0x0871F54F); a unit not shown gets the empty
   text.
4. Seconds with *tenths* and *t* ≥ 1: when the visibility test of step 3 (without the value
   test) passes, the seconds variable = UI token 0x0FF725B3 with 0x79842C3 = the seconds text
   and 0x58A38D3 = *t*; otherwise empty.

| Caller | *tenths* | *two* | template | other |
|---|---|---|---|---|
| Casting time (§7.6) | on | off | short | — |
| Effect duration line (§9.5) | on | on | short | — |
| Timer, default (§10.2) | off | `ShowTwoPlaceFormat` | short | — |
| Timer, full format (§10.2) | off | — | short | *all* and *zeros* on |

<a id="s10-2"></a>
### 10.2 Timer rows

A timer element formats its current value with its own properties:

1. `UIComponent_Timer_ShowClockFormat` true → a clock format (not used by these rows; not
   traced).
2. Else if `UIComponent_Timer_FormatString` renders non-empty → that string as the template,
   with *all* = `ShowFullFormat`, *two* = `ShowTwoPlaceFormat`, *pad* =
   `AlwaysShowDoubleDigitMinSec`, *zeros* = `AlwaysShowLowerZeros`.
3. Else if `UIComponent_Timer_ShowFullFormat` (default false) is false → the default of
   §10.1 with *two* = `UIComponent_Timer_ShowTwoPlaceFormat` (dictionary default true).
4. Else the full format of §10.1.

Update period without the full format: one day while the value has 30-day units, one hour
while it has days, one minute while it has hours, otherwise every update. With the full
format: every update.

A `UIComponent_Timer_WrapString` (set by §7.15) wraps the text in its variable 0x58E15.

---

<a id="s11"></a>
## 11. Icons (BC, BIN; ST7, ST8)

<a id="s11-1"></a>
### 11.1 Examination icon

The icon provider (class 1067) sets `Examination_Icon` and `Examination_IsItem` = false:

| Examined | Icon | Census (§14) |
|---|---|---|
| Skill | `Skill_SmallIcon` (32 × 32 in every skill, `icon-layers.md` §5.1) | 13,247 of 13,247 name an existing surface |
| Skill with a travel link (§7.1) | the travel destination's icon when the viewer has one, else `Skill_SmallIcon` | 16 skills |
| Effect | `Effect_Icon` | 22,804 name an existing surface; 10,167 have none |

The field composes the icon by `icon-layers.md` §5.3 with this single layer (no static or rank
overlay). An effect without `Effect_Icon` gets an empty icon field (the name row stays).

<a id="s11-2"></a>
### 11.2 Skill displays (quickslots, skill lists)

A skill display chooses by its element's `UI_SkillDisplay_IconSize` (mapper 0x2300013A; the
display reads 3 when absent):

| Key (label) | Icon |
|---|---|
| 2 (`Large`) | `Skill_LargeIcon` |
| 3 (`Small`), absent | `Skill_SmallIcon` |
| any other (1 `Medium`) | `Skill_Icon` |

`Skill_LargeIcon` equals `Skill_Icon` in every skill; 239 skills have a 71 × 69 `Skill_Icon`
and a separate 32 × 32 `Skill_SmallIcon` (`icon-layers.md` §5.1). In this install six display
templates set `Small` (among them `RealQuickslotSkillDisplay`) and one sets `Large`.

**Travel skills:** when the skill has `Skill_TravelLinkToUse` and the element's
`UI_SkillDisplay_RecallSkillsShouldUseAreaIcon` is not false, the display shows the travel
link's area icon when one resolves; otherwise the size rule.

> **Tessera rule:** travel skills use the size rule.

<a id="s11-3"></a>
### 11.3 Effect-bar icons

`Effect_Icon`, drawn as `combat-ui.md` §8.1 says. Effects without one fall back as
`combat-ui.md` §13.5.

---

<a id="s12"></a>
## 12. Hover behaviour

<a id="s12-1"></a>
### 12.1 Quickslot skills and skill lists (BIN (e))

A skill display sets `UI_Examination_Tooltip_DID_Override` = its skill, so hovering it shows
that skill's hover tooltip (§3.2) through the hover rule of `ui-behaviour.md` §6.2, placed by
`item-tooltips.md` §9. A display without a skill shows none.

<a id="s12-2"></a>
### 12.2 Effect-bar icons (live remaining time)

The icon carries `UI_Examination_Tooltip_DID` (the effect record), `UI_Examination_Tooltip_EffectID`
and `…_EffectInstanceID` (`combat-ui.md` §8.1). Its tooltip is the effect's hover tooltip;
the resistance and time rows use the live effect (§8.2, §8.4), and the time row counts down
while the tooltip is shown (§10.2, full format). The tooltip does not follow the pointer and is
not rebuilt while shown; only the timer text changes.

The effect tooltip has **no stack-count row**. The original does not show stacks in it.

> **OPEN (for `combat-ui.md`):** its TR31 says the tooltip shows the stack count when
> `stacks` > 1; this layout has no row for it. **Tessera rule:** no stack count in the
> tooltip; `combat-ui.md` TR31 should be aligned at review.

<a id="s12-3"></a>
### 12.3 Skill items (ST18)

The item layouts `SkillItem`, `Armor`, `ArmorCompare`, `Jewelry`, `JewelryCompare`,
`Weapon`, `WeaponCompare`, `Instrument` and `InstrumentCompare` (`_ExamineLayout`) contain 13
skill rows marked `UI_Examination_Tooltip_For_SkillItem`: `SkillItem_EffectDisplay_Base`
(target 3, header 0x0F345375), `SkillItem_UserEffectDisplay_Base` (target 1, header
0x0F345375), and `SkillItem_` `ResistCategory`, `Damage`, `Qualifier`, `CastingTime`, `Pip`,
`Trait`, `MaxRange`, `MaxTargets`, `SphereRadius`, `ArcRadius`, `Materials` (`_Base`), each
based on the skill row of the same name; the two instrument layouts add
`SkillItem_VitalCost_Base`. They show the rows of the skill named by the
item's `Usage_SkillToExecute` (§6.1); this answers the "skill-item rows" of `item-tooltips.md`
§6.20.

Census: 1,194 items name a skill there (691 `SkillItem`, 226 `Jewelry`, 48 `Instrument`, 110
`Weapon`, 119 `Armor` hover elements), every one a skill with a hover pair; the other 110
`SkillItem` items name none, so their skill rows are empty.

> **OPEN:** (implementer, 2026-10-10) the widget type of `SkillItem_EffectDisplay_Base` and
> `SkillItem_UserEffectDisplay_Base` in the install's item layouts is
> `ExaminationSkillEffectList`, which §4.3 does not list. The implementation treats it as
> `ExaminationEffectList` (the skill's effect lists of §9.6, by the row's
> `UI_Examination_Tooltip_SkillEffectTarget`) and gives it the `EffectListElement` line colours.

<a id="s12-4"></a>
### 12.4 Skill- and trait-granting items (BIN (f); ST17)

When an item's hover tooltip is assembled with compare kind 7 (`SkillGrantingItem`), the
client reads `Item_GrantedSkill` from the item's **record** (an instance: its record) and, when
it is set, builds the **hover** tooltip (purpose hover, §3.2) of that skill and attaches it to
the item's tooltip like a comparison tooltip (`item-tooltips.md` §8.3). Kind 5 (`TraitItem`)
does the same with `Item_GrantedTrait` and the trait's hover tooltip. No option gates either.

Census: 1,262 items have kind 7; all have `Item_GrantedSkill` naming a skill with a hover pair.

---

<a id="s13"></a>
## 13. The scenario feed and the sandbox

| Feed input (`scenario-feed.md`) | Tooltip effect |
|---|---|
| `CharacterInfo`, `LevelChanged` of the player | The viewer's level for progressions (§7.10, §9.8) |
| Player vitals (maxima) | *M*ᵥ of §7.10 |
| `QuickslotChanged { Skill(ref) }` | The slot's skill display and its tooltip (§12.1) |
| `CooldownStarted { what, duration_ms }`, `CooldownEnded` | *r* of §7.15 while the cooldown runs |
| `EffectApplied { entity, effect, what, duration_ms, stacks, source }` | A live effect instance: the icon's EffectID and EffectInstanceID; start, duration, spellcraft (§8.2–§8.4); caster = `source` (§9.5 combat-only line) |
| `EffectRemoved` | The instance ends; an open tooltip of it is hidden with its icon |
| A literal ref (`Ref::Literal`) | **Tessera rule:** a minimal tooltip: the frame with one name row showing the literal in the `Effect_ColorName` or `Skill_ColorName` colour, no icon (as `item-tooltips.md` §11) |
| (none) | No travel links, traits, pips, gambits, weapons or actor modifiers: the rows of §7.1 (travel), §7.9, §7.12, §7.13 (state), §7.14 follow their Tessera rules |

Server backends later supply the same live data; the rows then use it.

> **OPEN:** (implementer, `apps/client`, 2026-10-11) the row "Player vitals (maxima) → *M*ᵥ
> of §7.10" needs the `VitalType` key of the feed's `Health` and `Power` (`Vital::Health`,
> `Vital::Power`); §7.10 names the mapper but not which of its keys 1–4 is morale and which
> is power. Until a spec names them, the client gives the viewer no maximum vitals (*M*ᵥ = 0,
> the "0 when unknown" of the §7.10 Tessera rule), so percentage costs show no line.

---

<a id="s14"></a>
## 14. Real-data test (decision 0024 P2 exit criterion 2)

<a id="s14-1"></a>
### 14.1 Procedure

For every skill record and every effect record of the install at `TESSERA_GAME_DIR`
(`combat-data.md` §2.1 classification), build the hover tooltip model with the Tessera rules
(viewer: level 1, no vitals, no live effect) and lay it out headless; build the examination
icon (§11.1) and, for skills, the quickslot icon (§11.2, `Small`). Each record ends in exactly
one **tooltip category** and one **icon category**.

| Tooltip category | Meaning | Skills | Effects |
|---|---|---|---|
| `Rendered` | The tooltip element was built and its name row shows text | 5,201 | 27,150 |
| `RenderedNoName` | Built, but the name renders empty: `NameMissing` (the name's token is not in its table) or `NameEmpty` (the entry renders empty) | 0 | 1,250 (1,215 missing, 35 empty) |
| `NoTooltip` | No hover pair: the original shows none | 8,046 | 4,571 |
| `LayoutUnreadable` | The layout record is missing or fails to decode | 0 expected | 0 expected |
| `ElementMissing` | The element is not a top-level element of the layout | 0 | 0 |
| `RowFailure` | A row's rule failed on malformed data (the tooltip still renders; counted here as well) | 0 expected | 0 expected |

| Icon category | Meaning | Skills | Effects |
|---|---|---|---|
| `IconRendered` | The icon property names an existing 0x41 surface and it decodes | 13,247 | 22,804 |
| `IconAbsent` | No icon property (or 0) | 0 | 10,167 |
| `IconUnreadable` | The named surface is missing or fails to decode | 0 | 0 |

A failure is a panic, an error escaping the categories, or a count that differs from the
records with a valid hover pair (`Rendered` + `RenderedNoName`). Lookup misses in texts the
record names (names, descriptions, overrides, metadata names) are treated like empty texts and
are reported, not failures (as `item-tooltips.md` §12 implementer note).

<a id="s14-2"></a>
### 14.2 Census of texts (Confirmed, ST20)

| Text | With a tooltip | Without |
|---|---|---|
| `Skill_Name` | 5,201 text | 8,039 text, 7 empty |
| `Skill_Desc` | 4,438 text, 763 empty entries | 8,046 empty literal |
| `Effect_Name` | 27,144 text, 6 literal text, 35 empty, 1,215 missing token | 4,456 text, 115 literal text |
| `Effect_Applied_Description` | 10,592 text, 6 literal text, 17,802 empty literal | 170 text, 111 literal, 4,290 empty |
| `Effect_Definition_Description` | 1,269 text, 27,131 empty literal | 33 text, 4,538 empty |

Of the 80,750 distinct (table, token) pairs named by these records, their modifier overrides
and the metadata names, 69,997 render text, 1,886 are empty entries and 8,867 are missing
tokens.

**Modifier entries in effects with a tooltip** (`combat-data.md` §5.2): 31,109 non-empty
entries, every one naming a property that has a metadata object with a name text (one name
renders empty); `Mod_DescriptionOverride` renders text in 7,008, is empty in 1,313 (145
literals), names a missing token in 7,789, is absent in 14,999. 547 effects store the
procedural flag false; 13,755 effects with a tooltip have at least one non-empty entry.

<a id="s14-3"></a>
### 14.3 Row sources in the 5,201 skill tooltips (informative)

Counts of skills whose record gives the row a source under the rules of §7 (the test reports
the non-empty rows it builds; these figures are for orientation, not asserted):
`Skill_AreaEffectMaxTargets` > 1: 526; sphere radius ≠ 0: 442; arc radius ≠ 0: 169;
induction: 1,482; toggle: 1,456 (channelled 47); resistance category ≠ 0: 359;
`Skill_DisplaySkillType` set: 1,123; description text: 4,438; damaging hooks: 869; user
effects: 1,901; target effects: 2,829; positional: 36; user critical: 154; target critical:
110; target devastating: 116; vital cost: 2,230; required trait: 815; recovery time > 0:
3,394; pips: 710; applied, required and removed gambits: 50, 88, 92. Damage qualifiers of the
hooks: none 2,922, Melee 1,021, Magic 747, Ranged 506, two kinds 5.

Effects with a tooltip (informative): chance line source 0 < chance < 1 in 163 (8 of them
suppressed); combat-only 3,099; duration line source 8,457; resistance category ≠ 0: 3,010.

---

<a id="s15"></a>
## 15. Invariants

| ID | Invariant | Status |
|---|---|---|
| SV1 | Every skill and effect with a hover pair names (0x22000000, `Skill_ExamineLayout` or `Effect_ExamineLayout`), a top-level element based on `Tooltip_New` with one `ExaminationListBox` (5,201 and 28,400) | Confirmed 2026-10-10 |
| SV2 | Every entry template of the two elements names a top-level element of the examination layout (38 + 4) | Confirmed 2026-10-10 |
| SV3 | No skill or effect has a comparison pair or a compare kind | Confirmed |
| SV4 | `Skill_LargeIcon` = `Skill_Icon` in every skill; every skill's `Skill_SmallIcon` names an existing 32 × 32 surface | Confirmed |
| SV5 | Every non-empty `Mod_Entry` of an effect with a tooltip names a property with a metadata object | Confirmed (31,109 of 31,109) |
| SV6 | Every kind-7 item's `Item_GrantedSkill` names a skill with a hover pair (1,262 of 1,262); every `Usage_SkillToExecute` of an item names one (1,194 of 1,194) | Confirmed |
| SV7 | The procedural flag is never stored true | Confirmed (547 false) |
| SV8 | The duration formatter: for every *s* in [0, 10⁷) the shown units of the full format, read back, sum to trunc(*s*) when trunc(*s*) > 0 | Design check (synthetic) |
| SV9 | A modifier line's template has exactly the variables 0x5A6195 and 0x744A2C9 (eight templates) | Confirmed (Examination table) |

---

<a id="s16"></a>
## 16. Worked synthetic examples

<a id="s16-1"></a>
### 16.1 Skill rows

Skill: no damage, no effects, `Skill_Desc` text, `Skill_SkillRecoveryTime` 30.0, no induction,
`Skill_MaxRange` 25.0, hooks with qualifier 1, neither immediate nor ignoring reset. Rows after
filling: name; separator; qualifier-range row (qualifier = display text of key 1; range 25
with 0 decimals); max targets empty; radii empty; casting time empty; resistance and type
empty; separator; description; separator; damage empty; separator; effect lists empty;
separator; vital cost empty; …; separator; recovery (wrap 0x0EDBB215; short template with
only the seconds unit, 30). Assembly (`item-tooltips.md` §5) keeps: name, separator, qualifier-range, separator,
description, separator, recovery.

<a id="s16-2"></a>
### 16.2 Modifier lines

Effect class 734, harmful, flag absent (true), definition description empty, `Mod_Array`:
(a) `Add` int property *P* with metadata name *N*, value −15 → normalised `Subtract` 15 →
token 0x067FC704 with 0x5A6195 = value text (0x0F5F0564 with 15) and 0x744A2C9 = *N*, state
`debuffed`; (b) `Multiply` float *Q* with *pct*, value 0.9 → decrease template 0x09C79707, value
text 0x0CE39455 with 10 (*d* = 0.1, × 100, `string-table.md` flags 2 drop the zeros); (c) an
entry with an override that uses 0x06D04635 and value 0.05 on a *pct* property → the override
with 0x06D04635 = 5. No description, three modifier lines, then the chance line (if 0 < *c* < 1)
and the extra lines.

<a id="s16-3"></a>
### 16.3 Durations

*s* = 95.0, short template: default timer (*two* on): units 1 minute, 35 seconds → both shown.
*s* = 3,725.0: 1 hour, 2 minutes, 5 seconds → hours and minutes (two units). Full format: hours,
minutes, seconds (all from the first non-zero). *s* = 3,600.0 full format: hours 1, minutes 0,
seconds 0 → all three shown (*zeros*). Casting time *s* = 1.55 with tenths: *t* = ⌊0.55 × 10 +
0.5⌋ = 6 → seconds text "1" with tenth 6 through 0x0FF725B3. *s* = 2.96: *t* = ⌊10.1⌋ = 10 → *t* =
0, carry → 3 seconds, no tenths.

> **OPEN:** (implementer, 2026-10-10) the casting-time example and step 1 of §10.1 disagree
> for durations stored in single precision: the stored 1.55 is 1.5499999523, whose fraction
> times 10.0 plus 0.5 is 5.9999995 in f32, so *t* = 5 by step 1, not 6. The implementation
> follows step 1 on the stored value (the example holds for an exact 1.55). Which one the
> original shows is an owner check (OS6).

<a id="s16-4"></a>
### 16.4 Resistance text

Category 0x00000050 (bits 4 and 6 → keys 5 and 7): the lowest set bit is bit 4, key 5; *C* =
its display text. Spellcraft −3.0 → 0.

---

<a id="s17"></a>
## 17. Open questions (summary) and owner checks

| # | Question | Section | Settles it |
|---|---|---|---|
| Q1 | Which provider path (record or live) each requester uses | §6.2 | BIN (assembler callers) |
| Q2 | Property behind the qualifier list | §7.2 | BC (skill compiler data) or OBS2 |
| Q3 | Hook damage text and the request's weapon properties | §7.9 | BC + server data (later milestone) |
| Q4 | Rounding native, consumes-all costs | §7.10 | BIN |
| Q5 | Procedural flag when absent | §9.2 | BIN (property native and object property tables) |
| Q6 | Integer display of float modifier values | §9.4 | BC (class 1392) |
| Q7 | Merge of the critical target lists | §9.6 | BC (class 698 list merge) |
| Q8 | Effect-list header | §9.6 | BIN (list widget fill) |
| Q9 | Equipment-category and usage effects of items, level offset | §9.7 | BC |
| Q10 | Stack count in the effect tooltip (`combat-ui.md` TR31) | §12.2 | owner (OS5) and `combat-ui.md` review |
| Q11 | Class-specific line compositions of 29 effect classes | §9.2 | BC (each class's two list functions) |

Owner checks (to be numbered from O93 in `docs/checklists/owner-observations.md` at
close-out): **OS1** a skill tooltip with damage, effects and a cost (rows, order, colours);
**OS2** an effect tooltip from the effect bar (resistance, lines, remaining time counting down,
its format); **OS3** a stat line with a percentage and a decrease; **OS4** a skill-granting
item's attached skill tooltip; **OS5** whether an effect tooltip shows a stack count; **OS6**
the casting-time and recovery-time texts of a skill with tenths of a second.

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-10 | First version (Draft), for decision 0024 P2. Provenance OBS (layouts, all skills, effects, metadata objects and items, string pairs, icon surfaces, DID maps), OBS2 (compiled-attribute correlations), BC (skill, effect and line providers, effect classes 698 and 734, modifier description, item effect list), BIN (native row widgets, duration and timer formatting, number arguments, resistance text, skill-display icon and tooltip override, compare kinds 5 and 7). Answers `item-tooltips.md` §6.16 OPEN (P2), the skill-item rows of §6.20 and the SkillGrantingItem row of §8.3. |
| 2026-10-10 | Implementer OPENs from `crates/gamedata` (no fact changed): §4.3 token tables, §7.13 passive-skill traits, §7.15 sub-second recovery times, §9.2 compositions without class data, §9.7 place of usage effects and a missing template effect, §12.3 widget type `ExaminationSkillEffectList`, §16.3 single-precision casting time. |
