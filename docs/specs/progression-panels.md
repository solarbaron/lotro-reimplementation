<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Progression panels: the experience bar, the trait panel and the skill-training window

| | |
|---|---|
| **Status** | **Accepted** (review of the 2026-10-23 revision (§1.4, §2.3, §4.1, §6; PP17, PP18) by spec-review:spec-ui-scaling-p6:1 (independent session), 2026-10-23: passed). It was revised (2026-10-23: OPENs of §1.4, §2.3, §4.1 and §6 answered, row PP17 added; independent review passed). Accepted at the independent review of 2026-10-16 |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate dirty-room session, AI-assisted, Clean-Room-Role: review; wrote no part of this spec or the P6 code) |
| **Review record** | **Review 2026-10-23 of the 2026-10-23 revision (§1.4, §2.3, §4.1, §6; PP17, PP18)** (spec-review:spec-ui-scaling-p6:1 (independent session), wrote no part of it): **passed**; status Revised → Accepted. Checked: checked against CLEAN_ROOM.md §5.3, every item, on the uncommitted worktree diff against main, with the contamination screen clean: every added statement is a functional fact, a marked Tessera rule or Decision, or an OPEN; no code, pseudo-code or listing-shaped text; no copied prose or non-interoperability identifiers; no game text, assets, keys, hostnames or traffic; provenance rows present and marked; no byte layout added; the answered behaviour is testable; unknowns stay OPEN or owner observations; nothing outside decision 0034 §2. Relative links and anchors resolve. Spot checks: `keymap.md` §8.3 binds `toggle_traits` to J and `toggle_skills` to K; `chat-window.md` CW55 gives mask 0x8, type 4 `Standard`; §4.1 leaves the original's recipe-trainer window to the owner (§10). First version, 2026-10-16, for [decision 0024](../decisions/0024-parity-track.md) P6 (client half only) and [decision 0037](../decisions/0037-p6-trait-tree-and-recipe-census.md). **Independent review 2026-10-16 (spec reviewer, a session that wrote no part of it): passed; status Review → Accepted.** Checked against CLEAN_ROOM.md §5.3, decision 0024 §2 and §4 (client half only; values scripted, never derived; server data-free) and `server-content.md` §2: every statement is a functional fact (layout element roles and rects, enum/key forms, feed shapes, census rules) or a marked Tessera **Decision** (level-up presentation, fallbacks, gestures); no code or pseudo-code; element, property and enum-mapper names are interoperability identifiers (numeric IDs in the data, as in `combat-data.md`/`ui-layout.md`); no game text, assets, keys, hostnames or traffic (§2.3, §3.5 state so); provenance PP1–PP16 marked Confirmed/Reported/Decision; invariants PPI1–PPI11 are concrete; unknowns are `> **OPEN:**`, not guessed; no M4 pass; the progression model lives in `tessera-gamedata` (client/tools only, never linked by `tessera-server`) and the client derives no level, spends no point and decides no training (§1.2, §1.3, §2.2, §3.7, §4.4, PPI8, PPI9). **Own read-only probes of the install (M1, 2026-10-16):** a dat-only scan of every object record of `client_gamelogic.dat` (383,332 records) by root class at payload offset 4 — the census method of §5.1, no hard-coded DID — found **exactly 17** class-3486 records, confirming "17 trees" and the "every trait tree" definition; the layout records 0x22000033, 0x22000091, 0x22000030, 0x22000003 and 0x220008A4 exist and are UI layouts. The sub-counts of §5.3 (1,216 placements, 914 traits, 55 branch keys, 511 dependencies, 97 shared cell keys in 4 trees) are identical to the Accepted `combat-data.md` §8.5 (17 records, 1,216 placements, 511 dependencies) and its 2026-10-16 P6 implementer census, and were not re-derived here to avoid duplicating reviewed work. The feed events, commands and `TrainerFlags` bits cited (§2.2, §3.7, §4.3, §4.4) match `scenario-feed.md` §6.6/§9.1/§9.2/§9.4 (P6 progression). **Non-blocking review notes:** (a) the human-readable element names are symbolic renderings of numeric element IDs (the layout payloads carry no name strings — verified); the structure, rects and four-state lists (PPI5, PPI6) are OBS from the data, the names are interoperability identifiers, as the provenance note intends; (b) the available `dattool` layout decoder is incomplete for these records, so PPI5/PPI6 and the location-key row/column orientation of §3.2 (answering `combat-data.md` §8.5 Q11) were corroborated by record existence and internal consistency with the enum-label two-number form, but not re-decoded independently. **Scope note (keymap):** on branch p6 `keymap.md` was given only an implementer `> **OPEN:**` (no `ToggleTraitPanel`/`ToggleSkillPanel` rows; status stays Accepted P2), consistent with §1.4 here; a genuine keymap spec-writer revision adding those two actions (read from the install's key-map data) is still owed and is a keymap fact, not a progression fact — `keymap.md`'s status is unchanged by this review. |
| **Implements into** | `crates/gamedata` (`progression` module: the level/experience model, the trait-tree model with its cell placement and census, the skill-training offer model); `crates/ui` (`game::progression`: the experience bar, the trait panel and the skill-training window game widgets, and their fallback widgets); `apps/client` (`progression` module: opening the panels, their commands to the session, the level-up presentation, `--open-panel` names); `crates/plugins` (the progression plugin views only, through `lua-plugin-api.md` §15.6) |
| **Companion specs** | [`combat-data.md`](combat-data.md) §8 (trait classes, trait trees class 3486, trait control), §7 (skills); [`player-character.md`](player-character.md) §9 (experience tables and level controls), §13 (trait configuration, class-to-nature, group-to-nature); [`game-panels.md`](game-panels.md) §11, P3 toolbar (`LevelProgressMeter`); [`npc-windows.md`](npc-windows.md) §5.3 (NPC action picker), §9.2 (money display and coin split); [`interaction.md`](interaction.md) §10.2 (level-up actions), §12 (player trigger responses); [`icon-layers.md`](icon-layers.md) (composed trait and skill icons); [`skill-tooltips.md`](skill-tooltips.md) (skill tooltips); [`item-tooltips.md`](item-tooltips.md) §6.11 (worth); [`ui-layout.md`](ui-layout.md) (`UILAYOUT`, §9 building panels, §17 opening), [`ui-behaviour.md`](ui-behaviour.md) (window manager, tooltips, input routing), [`ui-widgets.md`](ui-widgets.md) (list boxes, trees, scroll bars, meters), [`string-table.md`](string-table.md) (rendering, markup, variables); [`keymap.md`](keymap.md) §8, §12.14.2 (panel-toggle actions); [`scenario-feed.md`](scenario-feed.md) §6.6, §9.2, §9.4 (the "P6 progression" revision); [`lua-plugin-api.md`](lua-plugin-api.md) §15.6; [`crafting-panels.md`](crafting-panels.md) (the sibling P6 spec: crafting panel, crafting trainers, recipe census) |

---

<a id="purpose"></a>
## Purpose

Decision 0024 P6 needs the **client half** of character progression: the experience bar and
level readout on the main toolbar, the trait panel (trait trees, equipped-trait slots, points),
and the skill-training window reached from an NPC trainer. Every value the panels show arrives
from the scenario feed (and, with M8, from an official server); the client half computes only
presentation (layout, text, icon composition, coin splitting, cell placement, state selection).
It never derives a level from experience, spends a trait point itself, or decides what the
player may train (decision 0024 §2 principle 4; `scenario-feed.md` §6.6 "Meaning for the client
half").

With this spec an implementer can:

- draw the toolbar experience meter and level readout, and react to `ExperienceChanged` and
  `LevelChanged`, including the local level-up presentation (§2);
- build the trait panel: map a class's trait trees to tabs, place point-based traits in cells
  by their location key, show ranks, prerequisites, points and slot state, draw the trait
  tooltip, and issue `SlotTrait` / `SetTraitRank` / `ChooseTraitBranch` (§3);
- build the skill-training window reached from an NPC action, show rows with cost and state,
  and issue `TrainSkill` (§4);
- run the trait-tree census that decision 0037 §1 item 1 requires for P6 exit criterion 1 (§5);
- fall back to Tessera's own windows when a layout record is missing (§6).

Crafting (the crafting panel, recipe pages, crafting trainers, gathering and facilities) is the
sibling spec `crafting-panels.md`; this spec covers the non-crafting progression panels only.

<a id="provenance"></a>
## Provenance

| Key | Source |
|---|---|
| **OBS** | Observed in the user's install (Steam, data current on 2026-10-06; install unchanged on 2026-10-16), read-only, 2026-10-16, with throwaway probes in the dirty scratch area built on a snapshot of the project's own decoders. The UI layout records named here (`toolbar` 0x22000033, `trait` 0x22000091, `skilltrainer` 0x22000030, and the tooltip frame 0x22000003) were decoded in full with property values and element trees resolved. The trait-tree census covers **every** object record of the trait-tree class of `combat-data.md` §8.5 (class 3486) and **every** placement of every tree, plus the class and `Trait_Name`/cost presence of every placed trait, found at run time by root class (prefix 0x70 payload offset 4), never by a hard-coded DID. Counts are observations of this install and must be read at run time. |
| **Reported** | Facts carried from Accepted or Review companion specs (cited inline) and, where noted, third-party extraction tools (LotroCompanion `lotro-tools`/`lotro-core`, read for facts only; no code or prose copied): which properties their trait and skill extractors read. Not re-observed here beyond the spot checks below. |
| **DEC** | Tessera's own choices where neither the data nor a companion spec settles a presentation question (mainly the level-up presentation and the fallback windows). Each is marked **Decision** and is not a claim about the original. |

No row of this spec rests on method M4 (client-executable disassembly): this pass used only the
data files of the install (M1) and published facts (M2). Where the original's behaviour lives
only in client code (the meter fill, the state selection, the XP tooltip text, the level-up
fanfare), the spec says so in an OPEN callout and gives a Tessera presentation as a Decision,
rather than guess the original.

Marks: **Confirmed** = observed over every record of the kind named, on 2026-10-16;
**Reported** = companion spec or published material only; **Decision** = a Tessera choice.

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| PP1 | Toolbar level-meter element tree, rects, meter states and overlay meters (§2.1) | OBS (layout 0x22000033) | Confirmed 2026-10-16 |
| PP2 | `ExperienceChanged`, `LevelChanged` feed shapes and what they change (§2.2) | Reported (`scenario-feed.md` §6.6, §9.2) | Reported |
| PP3 | Data-driven level effects: items that raise the level cap (level-up actions) and the player-trigger effects applied at listed levels (§2.3) | Reported (`interaction.md` §10.2, §12) | Reported |
| PP4 | The client's XP-bar tooltip text, the meter-state selection rule and the original's level-up fanfare live in client code, not the data | OBS (the layout carries the tooltip frame and four meter states but no tooltip text and no state rule) | Confirmed 2026-10-16 (that they are **not** in the data); the rules themselves **OPEN** |
| PP5 | Trait-panel element tree: the All/Equipped sub-panel with its nature slot categories, and the point-based trait-tree grid, floater row, points field and cell template (§3.1–§3.4) | OBS (layout 0x22000091) | Confirmed 2026-10-16 |
| PP6 | The trait-cell location key is *row* × 65,536 + *column* (row = *a* 1–8, column = *b* 1–12), or 0x10000000 + *n* for floater *n*, or 0 for unplaced; the grid cell element for each key and its position come from `UI_TraitTree_TraitBaseLocationArray` on `PointBasedTraitTree_TreeField` (§3.2). This answers `combat-data.md` §8.5 OPEN Q11 | OBS (layout 0x22000091; the location enum labels join the two numbers with an underscore, as `combat-data.md` §8.5 states for the data mapper) | Confirmed 2026-10-16 |
| PP7 | Trait-cell states (`normal`, `highlight`, `ghosted`, `ghosted_highlight`) and the coloured trim and ghost overlay per state; the rank-overlay text element and static-icon overlay; dependency arrows between cells (§3.3) | OBS (cell template `PointBasedTraitTree_TraitBase`) | Confirmed 2026-10-16 |
| PP8 | Trait trees, cells and branches as data (class 3486): 17 trees, 1,216 placements naming class-3438 traits, cell-key and branch mappers, dependencies (§3.2, §5) | Reported (`combat-data.md` §8.5) | Reported |
| PP9 | Trait-tree census on this install: 17/17 trees and 1,216/1,216 placements `Shown`; 914 distinct traits (all class 3438, all with `Trait_Name` and `Trait_PointBasedTrait_PointCostProgression`); 55 distinct branch keys; 99 distinct location keys (92 grid, 7 floaters); 511 dependencies, all resolving within branch; 97 cell keys shared by more than one placement in 4 of the 17 trees; no placement outside the key forms of PP6 (§5.3) | OBS (every class-3486 record and every placement) | Confirmed 2026-10-16 |
| PP10 | Equipped-trait slot categories (Common, Race, Class, Legendary, Tribe, and Skirmish Role/Skill/Training/Personal), which are slots by trait nature; trait natures and the class→nature, group→nature and configuration models (§3.6) | OBS (layout 0x22000091 slot categories); Reported (`player-character.md` §13, `combat-data.md` §8.2) | Confirmed 2026-10-16 (slot categories) + Reported (natures/config) |
| PP11 | `TraitPoints`, `TraitSlotted`, `TraitRank`, `TraitTreeChanged` feed shapes and the `SlotTrait`/`SetTraitRank`/`ChooseTraitBranch` commands (§3.7) | Reported (`scenario-feed.md` §6.6, §9.2, §9.4) | Reported |
| PP12 | Skill-training window element tree: can/cannot-use fields, Active/Passive/Trait/Gambit and Skirmish tabs each a list `Tree`, the train button, the mithril buy-now button, the player-money display, the "trainable only" checkbox (§4.2) | OBS (layout 0x22000030) | Confirmed 2026-10-16 |
| PP13 | The skill-training window is reached from an NPC action-picker entry; the money display and coin split are those of `npc-windows.md` §9.2 (§4.1, §4.5) | Reported (`npc-windows.md` §5.3, §9.2) | Reported |
| PP14 | `TrainerShown` (with `offers`, entry `coin`/`level`/`rank`/`flags`), `UntrainedSkills`, `SkillLearned`/`SkillForgotten`, and the `TrainSkill` command (§4.3, §4.4) | Reported (`scenario-feed.md` §6.6, §9.1, §9.2, §9.4) | Reported |
| PP15 | Panel-toggle actions for the trait and skill panels are the keymap's (`keymap.md` §8, §12.14.2); the install's toolbar names the actions `ToggleTraitPanel` and `ToggleSkillPanel` (§1.4) | OBS (layout 0x22000033 tutorial action map); Reported (`keymap.md`) | Confirmed 2026-10-16 (the names) + Reported (bindings) |
| PP16 | Plugin progression views (`LocalPlayer:GetTrainedSkills()`, `GetUntrainedSkills()`, and the trait/skill views of §15.6) (§7) | Reported (`lua-plugin-api.md` §15.6, §16) | Reported |
| PP17 | Default keys of the trait and skill panels: `keymap.md` §8.3 binds `toggle_traits` (`ToggleTraitPanel`) to J and `toggle_skills` (`ToggleSkillPanel`) to K from the install's key-map data (§1.4) | Reported (`keymap.md` §8.3, Accepted 2026-10-23) | Reported |
| PP18 | Tessera rules of 2026-10-23: the window for a `recipes` offer (§4.1), when the experience-bar fallback is drawn (§6), the chat mask of the level-up notice (§2.3) | This spec, answering implementer OPENs | Decision |

Spot checks against the install, 2026-10-16 (OBS): the trait-tree census numbers of PP9 were
re-derived from every class-3486 record; the trait-family counts of `combat-data.md` §8.1 (1,773
+ 946 + 767 + 244 + 125 + 45 + 37 + 25 + 1 = 3,963) were reproduced; `UI_TraitTree_TraitLocation`
labels `1_1` … `5_9` and `F_1` … `F_8` were read from the layout and match the data mapper's
two-number form of `combat-data.md` §8.5.

---

<a id="s1"></a>
## 1. Concepts, scope and conventions

<a id="s1-1"></a>
### 1.1 Terms

| Term | Meaning |
|---|---|
| **Experience bar** | The toolbar meter and text showing progress toward the next level (`LevelProgressMeter`, `game-panels.md` P3). |
| **Trait tree** | A class-3486 record (`combat-data.md` §8.5): one per tree type, holding placements of point-based traits in cells, grouped into branches. |
| **Placement** | One entry of a tree's `Trait_TraitTree_TraitArray`: a trait DID, a branch key, a cell (location) key and a dependency list. One trait DID can sit in more than one placement. |
| **Cell** | A fixed position in the panel's grid, identified by a location key (row, column) or a floater index, into which a placement's trait icon is drawn. |
| **Point-based trait** | A class-3438 trait with a point-cost progression, the kind a tree places (`combat-data.md` §8.1). |
| **Rank** | How far a trait is advanced; shown as an overlay number on the cell and fed as `TraitSlotted.rank` / `TraitRank.rank`. |
| **Trait nature** | The category of a trait (class, racial, legendary, virtue, …); drives which equipped slot it goes in (`combat-data.md` §8.2 `Trait_Nature`; `player-character.md` §13.3–§13.4). |
| **Slot kind** | An equipped-trait slot category in the panel (Common, Race, Class, Legendary, Tribe, Skirmish …), one per trait nature (§3.6). |
| **Trainer window** | The skill-training window (`skilltrainer` layout) opened from an NPC trainer action. |

All layout DIDs, element IDs, property names and enum-mapper numbers quoted here are the
install's own and are **interoperability identifiers**: read them at run time, do not compile
them in. Positions are in the layout's own 1024-wide design pixels before UI scaling
(`ui-behaviour.md` UI scaling; `ui-scaling.md`). Colours are 0xAARRGGBB as the layout stores
them.

<a id="s1-2"></a>
### 1.2 What the feed drives

The panels are driven by the "P6 progression" events of `scenario-feed.md` §6.6/§9.2:
`ExperienceChanged`, `LevelChanged` (with `quiet`), `SkillLearned`/`SkillForgotten`,
`UntrainedSkills`, `TrainerShown` (with `offers` and entry `rank`/`flags`), `TraitPoints` (with
`tree`), `TraitSlotted`, `TraitRank`, `TraitTreeChanged`. The panels issue the commands
`TrainSkill`, `SlotTrait`, `SetTraitRank` and `ChooseTraitBranch` (§9.4). The feed computes no
level, spends no point and decides nothing the player may train; a click issues a command and
the view changes only when the answering event arrives.

<a id="s1-3"></a>
### 1.3 What the client half computes

Layout and painting; text rendering and markup (`string-table.md`); icon composition
(`icon-layers.md`); coin splitting and the money display (`npc-windows.md` §9.2); placement of a
trait into its cell by the location key (§3.2); the cell's visual state from the feed's slot,
rank and point state and the placement's prerequisites (§3.6); the level-up presentation (§2.3);
and the census of §5. Nothing else.

<a id="s1-4"></a>
### 1.4 Opening the panels

The trait panel and the skill-training window are toggled by the keymap's panel-toggle actions
(`keymap.md` §8, §12.14.2), like the other P-track panels. The install's toolbar carries a
tutorial action map that names the toggle actions `ToggleTraitPanel` and `ToggleSkillPanel`
(and, for the level-up tutorial, `ToggleJournalPanel`); the research-reported default keys are
**J** for traits and **K** for skills. `apps/client` also opens each by an `--open-panel` name
(`traits`, `skill-trainer`; the exact names are the implementer's, like the other panels).

> **Was OPEN:** (P6) `keymap.md` does not yet carry rows for `ToggleTraitPanel` and
> `ToggleSkillPanel` (its §8.2 P6 note defines only the crafting toggles and says the trait and
> skill-panel actions "await this spec's rows"). keymap.md is the authority for action names and
> default bindings; a keymap revision should add these two actions (buttons, `global` context,
> exact policy) and their default keys, read from the install's key-map data, and this spec will
> cite the row. Until then `apps/client` may bind them provisionally; this is a keymap fact, not
> a progression fact.
>
> **Answered (spec writer, 2026-10-23):** `keymap.md` §8.3 (revision of 2026-10-23,
> Accepted) carries the rows: `toggle_traits` is `ToggleTraitPanel`, default J, and
> `toggle_skills` is `ToggleSkillPanel`, default K, both in the `global` context (PP17). The
> panels open and close with these actions; the provisional bindings are retired.

---

<a id="s2"></a>
## 2. The experience bar and level display

<a id="s2-1"></a>
### 2.1 The toolbar level meter (OBS, PP1)

On the main toolbar (`game-panels.md` P3, layout `toolbar` 0x22000033) the element `LevelMeter`
(a `Field`, rect 1, 181, size 1024 × 30, anchored to all four edges) holds a horizontal
experience meter spanning the toolbar's width:

| Element | Type | Rect (x, y, w, h) | Role |
|---|---|---|---|
| `LevelMeter` | Field | 1, 181, 1024, 30 | Container; tooltips on, generic tooltip frame (layout 0x22000003), tooltip ID `ToolTip_01` |
| `LevelMeterFill` | `LevelProgressMeter` | 5, 7, 1015, 11 | The experience fill; fills from the **Left**, alphablend, `move_fill` off |
| `LevelMeter_VIPRestMeter` | Meter | 0, 0, 1015, 11 (z 1) | Rested-experience overlay (VIP), drawn after children |
| `LevelMeter_PurchasedRestMeter` | Meter | 0, 0, 1015, 11 (z 2) | Rested-experience overlay (purchased), drawn after children |
| `ToolbarFieldMain_LevelMeter_Glass_Overlay` | Field | 5, 7, 1015, 11 | Glass highlight over the fill |
| `LevelMeterText` | text (`fieldlabelmedium`) | 0, 5, 1020, 15 | The text drawn over the bar |
| `ToolbarFieldMain_LevelMeter_{Left,Right,Mid_Left,Mid_Right}_Cap` | Field | at 0/1009/397/608, y 187, ~16 × 13 | End and middle caps of the bar frame |
| `ToolbarButton_{LevelUpXP,Experience,RestXP}_TutorialHighlight` | tutorial highlighter | overlay | Tutorial pointers for the level-up, experience and rested-XP hints |

`LevelMeterFill` carries **four states** chosen by the client at run time, each setting the
meter's fill image: `normal`, `bonus` (a rested or other experience bonus is active),
`xp_suppressed` (experience is not being gained, e.g. at the level cap or when suppressed) and
`xp_suppressed_with_bonus`. The two rested meters overlay the portion of the bar covered by
rested experience.

The fill fraction, the state choice, the text in `LevelMeterText` and the tooltip content are
**not** in the layout (the layout carries only the fill element, the four state images, the
tooltip **frame** and the tutorial highlighters). They are set by client code from the
player's current/needed experience and level, and from the rested-experience amount.

> **OPEN:** (PP4) The exact rule for the fill fraction (current ÷ needed, clamped), the choice
> among the four meter states, the text shown in `LevelMeterText` (the level number, a
> percentage, or "current / needed"), and the tooltip's content and layout are client code, not
> in the data, and were not read under M4 in this pass. Tessera's presentation is the Decision
> of §2.2. M3 observation of the running client or an M4 pass would settle the original's.

<a id="s2-2"></a>
### 2.2 What `ExperienceChanged` and `LevelChanged` change (PP2; Tessera presentation = Decision)

`ExperienceChanged { current, needed }` (both u64, `scenario-feed.md` §9.2) sets the player's
current and needed experience. `LevelChanged { entity, level, quiet }` sets the entity's level;
for the local player it sets the level the panels show. The feed never computes a level from
experience (`scenario-feed.md` §6.6).

**Tessera presentation (Decision):** on `ExperienceChanged` for the local player, the experience
bar updates its fill to `current ÷ needed` (clamped to 0 … 1; `needed` = 0 draws an empty bar)
and its text to the level and the ratio; it uses `normal` unless a later feed revision marks a
bonus or a suppressed state. The rested-experience overlays are drawn from a rested amount only
when the feed carries one (none in the P6 feed yet; the overlays stay empty). The bar's tooltip
is Tessera's own (the level, current/needed experience, and the rested amount when present),
with no copied game string.

> **OPEN:** (P6) The P6 feed has no form for rested experience, an experience bonus, a suppressed
> state, or the experience-bar tooltip values beyond current/needed. A later `scenario-feed.md`
> revision should add them (a `rest`, a `bonus` flag and a `suppressed` flag on
> `ExperienceChanged`) so the bar can reach its `bonus`, `xp_suppressed` and
> `xp_suppressed_with_bonus` states and fill its rested overlays from data rather than Tessera
> defaults.

<a id="s2-3"></a>
### 2.3 The level-up presentation (local)

`scenario-feed.md` §6.6 defines a **level-up** as a `LevelChanged` for the local player that
**raises** the level the client knows and is **not** `quiet`. A `quiet` report, a report that
does not raise the level, and the first level of a session set the level silently.

The game data carries two level-related pieces, both already specified:

- **Level-cap unlock items** (`interaction.md` §10.2, "level-up actions"): 8 usage-action
  records, named by 9 items, with `Usage_LevelupUsageAction_LevelToAdvance` (50, 95, 105, 120,
  130, 140, 150, 160) and, in 7, a list of quests to complete. These are the items that let a
  character raise its **level cap**; they are not the per-level presentation and the panels do
  not draw them. (Reported, PP3.)
- **Level-triggered effects** (`interaction.md` §12, `PlayerTriggerResponseSet`): at levels 15,
  30, 39, 45, 58 and 65, an effect is applied to players of listed classes. In the client half
  this is a server/feed decision surfaced as `EffectApplied` (`scenario-feed.md` §9.2); the
  panels and the world view show the effect as any other, through `combat-visuals.md`. (Reported,
  PP3.)

**Tessera presentation on a level-up (Decision):** when a non-`quiet` `LevelChanged` raises the
local player's level, the client half:

1. posts a notice line to the chat/notice area (the P4 `notice` channel of `chat-window.md`)
   naming the new level, in Tessera's own words (no copied game string);
2. plays Tessera's own level-up cue through `tessera-audio` (a short, bundled or synthesised
   sound; decision 0022 covers the bundled asset), if audio is on;
3. does not itself apply any effect: a level-triggered effect reaches the client only as a feed
   `EffectApplied` (above).

The original client's exact level-up presentation (its fanfare sound DID, its screen effect and
its chat line) lives in client code and was not read under M4 in this pass.

> **OPEN:** (P6) The original's level-up fanfare (sound, visual effect) and its exact chat/notice
> wording are client code, not data. Tessera uses its own cue and wording. M3 observation or an
> M4 pass would let a later revision describe the original; the Decision above stands until then.

> **OPEN:** (P6) The feed does not distinguish a level-up that crosses a `LevelToAdvance` cap
> (unlocking a level-cap item, `interaction.md` §10.2) from an ordinary level-up. If the client
> should show anything special at a cap, a feed form is needed; none exists, so Tessera shows the
> ordinary presentation.

> **OPEN:** (P6, implementer note) Beyond the chat notice and audio cue above, Tessera's
> implementation also shows a short transient on-screen **level-up banner** naming the new level
> (a pure presentation choice; `tessera-ui`'s `progression::xpbar::LevelUpBanner`, captured by the
> decision 0037 §2 `p6_levelup_banner` golden). The spec does not require it; a later revision may
> describe the original's on-screen presentation (if any) and either adopt or drop the banner. Its
> appearance at a time is a pure function of the start time, so a golden freezes it deterministically.

> **Was OPEN:** (implementer, `tessera-client`, 2026-10-23) item 1 is built: the notice line is
> posted as a derived chat line with the `Standard` mask (0x8, type 4), the mask of the P4 friend
> and channel notices of `chat-window.md` (CW53, CW55), since "the P4 `notice` channel" names no
> mask of its own; confirm the mask. Item 2 is **not built**: Tessera has no bundled or synthesised
> level-up cue yet, and the P6 glue has no path to the client's audio engine. The level-up runs
> silently until a cue (decision 0022 for a bundled asset, or a synthesised one) and its
> playback from the P6 glue are chosen.
>
> **Answered (spec writer, 2026-10-23), Decision (PP18):** the mask is confirmed: the
> level-up notice is a derived chat line with the `Standard` mask (0x8, chat type 4), like the
> P4 friend and channel notices (`chat-window.md` CW53, CW55). Item 2 stays part of the
> Decision; no spec fact is missing for it. Until a cue exists the level-up is silent, which
> is the expected interim behaviour, not a defect of item 1.

---

<a id="s3"></a>
## 3. The trait panel

The trait panel is the layout `trait` (0x22000091), a 1024 × 768 record. It holds two distinct
presentations in one layout:

- an **All / Equipped** sub-panel (`MPTraitsPage` → `TraitsPanel`, with the `AllTraits_Page` and
  `EquippedTraits_Page` tabs) that lists traits by nature and holds the equipped-trait **slots**
  (§3.6);
- the **point-based trait-tree** presentation (`PointBasedTraitTree_*`) that draws a class's
  trait trees as cell grids (§3.1–§3.4).

The point-based tree presentation is the one that matches the class-3486 data model (cells,
branches, ranks, points) and is the focus of P6 criterion 1; the equipped-slot sub-panel is
driven by `TraitSlotted` (§3.6–§3.7).

<a id="s3-1"></a>
### 3.1 Trees, tabs and pages (OBS, PP5)

A trait tree is drawn in a grid display. The layout holds three grid templates, chosen by the
kind of tree:

| Grid element | Columns × rows (cells) | Use |
|---|---|---|
| `PointBasedTraitTree_GridDisplay` (with `_LeftBranch`, `_CenterBranch`, `_RightBranch`, each 185 wide) | three branch columns | the common point-based tree layout (`PointBased_TraitTree`, 53 cells incl. 8 floaters) |
| `Class_PointBasedTraitTree_GridDisplay` | 12 × 7 (up to 84) + 8 floaters | the class tree layout (`Class_PointBased_TraitTree`, 92 cells) |
| `BigBattles_PointBasedTraitTree_GridDisplay` | 9 × 8 (72) | the big-battle tree layout (`BigBattles_PointBased_TraitTree`, 72 cells) |

The grid lives in a scrolling list box (`PointBasedTraitTree_GridListBox`, one column, auto-fill
entry template naming `PointBasedTraitTree_GridDisplay`) with a scroll bar
(`PointBasedTraitTree_Scrollbar`). Above the grid is a floater row
(`PointBasedTraitTree_FloaterDisplay`, 8 cells) for the tree's specialisation ("floating")
traits. Below the grid is the points field (§3.4).

The newer **specialisation** UI (`traittreeui` layout 0x220008a4) presents the same class-3486
data as three specialisation columns with right-side tabs `TraitSet_1_Tab` … `TraitSet_7_Tab`;
Tessera builds the point-based grid presentation of this section (it maps directly to the
class-3486 cell/branch model) and may offer the specialisation view later.

**Mapping trees to tabs.** Which trees a class may use, and the order of the tabs, is class data
(`player-character.md` §13.4 maps each class to its trait natures and branch keys;
`combat-data.md` §8.5 tree records carry `Trait_TraitTree_TreeType`). The feed's
`TraitTreeChanged { tree, branch }` names the active tree and its chosen branch; `TraitPoints …
tree=` carries the points of a named tree (`scenario-feed.md` §6.6). The panel shows one tab per
tree the feed names (or per tree of the player's class, when the client can resolve the class
from `CharacterInfo.class`), with the active tree selected.

> **OPEN:** (P6) The exact assignment of a class's trees to tabs (which tree is tab 1, whether
> all 17 trees or only the player's class's trees appear, and how a tree's branches map to the
> three branch columns of the common grid vs. the 12-column class grid) is class/server-driven.
> The feed names the active tree and branch but not the full tab set. Tessera shows a tab per
> tree the feed references and places each placement by its location key (§3.2); a later feed or
> `player-character.md`/`combat-data.md` revision should state the full class→tree→tab rule.

<a id="s3-2"></a>
### 3.2 Cells, branches and location keys (OBS, PP6; data Reported, PP8)

Each grid cell is an instance of the cell template `PointBasedTraitTree_TraitBase` (§3.3),
placed at a fixed position. The element `PointBasedTraitTree_TreeField` carries
`UI_TraitTree_TraitBaseLocationArray`, a map from a **location key** to the cell element (and
hence its position) for that key. The location keys are the same two-number form as the data's
`Trait_TraitTree_TraitLocation` mapper (`combat-data.md` §8.5):

- **Grid cells**: key = *row* × 65,536 + *column*, with *row* (*a*) = 1 … 8 and *column* (*b*) =
  1 … 12. Cell element `PointBasedTraitTree_Cell_a_b` sits at a fixed (x, y): increasing *a*
  moves **down** (row), increasing *b* moves **right** (column). (For the common grid the
  observed cells are `Cell_1_1` at (20, 12), `Cell_2_1` at (20, 88), `Cell_1_2` at (80, 12), each
  about 40 × 50; the class and big-battle grids use 60-pixel column and row steps.)
- **Floater cells**: key = 0x10000000 + *n* (*n* = 1 … 8), element `PointBasedTraitTree_Cell_Float_n`
  in the floater row above the grid.
- **Unplaced**: key 0 means the placement has no cell and is not drawn in the grid.

This resolves the OPEN of `combat-data.md` §8.5 (which of *a*, *b* is row/column, and what the
0x10000000 group is): *a* = row, *b* = column, 0x10000000 + *n* = floater *n*. (See §11 note.)

**Branches.** A placement's `Trait_TraitTree_Branch` (mapper 0x230003A1) groups cells into
branches (the Left/Center/Right branch columns of the common grid, or the wider class grid's
column bands). The branch is a grouping key, not a position; the position comes from the location
key. 55 branch keys occur across the install's trees (PP9).

**Dependencies.** A placement's `Trait_TraitTree_TraitDependencyArray` lists cells it depends on,
each with a `_TraitDependencyRank` (1 … 5 in the data). In the panel these are drawn as the cell
template's directional **arrows** (`PointBasedTraitTree_RightArrow`, `_LeftArrow`, `_DownArrow`,
hidden by default) linking a cell to the cells it depends on, and they gate the cell's available
state (§3.6). A dependency names a cell **within the dependent placement's branch** (Tessera
rule, from `combat-data.md` §8.5: the first placement of the same branch at that cell; when that
branch places nothing there, the tree's only placement at that cell, if exactly one; otherwise
the dependency is unresolved and the cell is treated as having no met prerequisite). A stored
rank outside 1 … 65,535 is clamped to that range (0 and negatives read as 1, never "always met").

<a id="s3-3"></a>
### 3.3 The trait cell (OBS, PP7)

The cell template `PointBasedTraitTree_TraitBase` is a `PointBasedTraitDisplay`, 40 × 50, with:

| Property / child | Role |
|---|---|
| `UI_TraitDisplay_Icon_Element` → `TraitIcon` | the trait's icon (`Trait_Icon`, composed per `icon-layers.md`) |
| `UI_TraitDisplay_Static_Icon_Overlay_Element` → `TraitStaticIconOverlay` | a static overlay (`Trait_Static_Icon_Overlay`) |
| `UI_TraitDisplay_Rank_Overlay_Element` → `TraitRankOverlay` | a `fieldlabelsmall` text at the cell's bottom showing the rank (e.g. "3/5") |
| `UI_TraitDisplay_Is_Draggable` = false | the cell is not drag-dragged in this grid |
| `TraitBackground`, `TraitBackgroundTrim` | the cell frame and a coloured trim per state (below) |
| `TraitGhostOverlay` | a dimming/ghost overlay drawn in the `ghosted` state |
| four states `normal`, `highlight`, `ghosted`, `ghosted_highlight` | the cell's visual state (§3.6) |

The `TraitBackgroundTrim` border colour is set per state in the layout: `normal` green
(0xFF00FF00), `highlight` blue (0xFF0000FF), `ghosted` black (0xFF000000),
`ghosted_highlight` yellow (0xFFFFFF00). The `TraitGhostOverlay` draws a black dim in the
`ghosted` state and nothing in the others. (These are the layout's own state media; the client
chooses the state, §3.6.)

<a id="s3-4"></a>
### 3.4 The points display (OBS, PP5)

Below the grid, `PointBasedTraitTree_PointsField` (rect 175, 448, 274 × 26) holds the point
counters, all `fieldlabelsmall` text centred:

| Element | Role |
|---|---|
| `PointBasedTraitTree_SpentLabel` | the "spent" caption (string entry, table 0x250001E7) |
| `PointBasedTraitTree_SpentPointsLabel` | the spent-points **value** (white; has a tooltip) |
| `PointBasedTraitTree_PointsLabel` | a separator/caption (blue 0xFF4773F7) |
| `PointBasedTraitTree_PointsRemainingLabel` | the remaining-points **value** (white; has a tooltip) |
| `PointBasedTraitTree_AvailableLabel` | the "available" caption |

The feed fills these from `TraitPoints { available, total, tree }` (`scenario-feed.md` §9.2):
`total` is the points the tree (or the class) grants, `available` the points not yet spent, and
`total − available` the points spent. `tree` chooses which tree's points are shown (absent: the
class's). The captions' text comes from the install's string table; the values are the feed's.

<a id="s3-5"></a>
### 3.5 The trait tooltip

A cell's tooltip uses the generic tooltip frame (the install's tooltip layout, named by the
cell's `UICore_Element_tooltip_layout`, 0x22000003) and shows the trait's name
(`Trait_Name`), its description/tooltip (`Trait_Tooltip`, a StringInfo; `combat-data.md` §8.2),
its tier and cost-to-slot, and its rank state. The text is resolved and marked up per
`string-table.md`; no game string is reproduced in this spec. Where the trait grants a skill or
an effect at a rank (`combat-data.md` §8.3), those are shown as the skill/effect tooltips of
`skill-tooltips.md`.

<a id="s3-6"></a>
### 3.6 Cell states and equipped-trait slots (OBS, PP7, PP10)

**Cell state.** The client chooses a cell's state from the feed's rank/point state and the
placement's prerequisites:

| State | When | Trim |
|---|---|---|
| `normal` | the trait is **available** (prerequisites met, points remain, not at max rank) and not under the pointer | green |
| `highlight` | available and under the pointer | blue |
| `ghosted` | **locked** (a prerequisite not met, or no points) or **unavailable** | black, dim overlay |
| `ghosted_highlight` | locked and under the pointer | yellow |

A placement at **max rank** (rank = the trait's maximum) is shown at full rank in
`TraitRankOverlay` and drawn `normal` (fully invested), not `ghosted`. An **empty** cell (a grid
position with no placement) draws nothing. A placement whose prerequisites are unmet draws
`ghosted` with its dependency arrows; the client does not itself decide "requirements not met" at
the player's level — it uses only the feed's state (the tree's `TraitRank`/`TraitPoints` and the
placement's dependencies), never a derived level test (Tessera rule, `combat-data.md` §8.5).

**Equipped-trait slots.** The All/Equipped sub-panel holds the equipped-trait slots by nature,
in these categories (OBS, layout 0x22000091): **Common**, **Race**, **Class**, **Legendary**,
**Tribe**, and the Skirmish categories **Role**, **Skill**, **Training**, **Personal**. Each is a
slot field holding one or more trait-display slots of the same template family as §3.3. A slot is
one of the trait-configuration save slots (`player-character.md` §13.2): the class, the two
mounted, the big-battle and the virtue configurations each define their natures and slot counts,
and `Trait_Nature` (`combat-data.md` §8.2, mapper 0x23000057) / the group→nature and class→nature
maps (`player-character.md` §13.3–§13.4) decide which nature goes in which category. An empty slot
draws its empty frame; a filled slot draws the slotted trait's icon and rank.

> **OPEN:** (P6) The precise mapping of a trait's nature to a slot category and the number of
> slots per category depend on the active trait configuration (`player-character.md` §13.2) and
> the character's unlocked slots, which are server/feed state. The feed's `TraitSlotted { slot,
> what, rank }` names a slot **index** and a trait; it does not name the category. Tessera places
> a slotted trait in the category of its nature and uses the slot index within that category; a
> later feed revision could carry the category to remove the inference.

<a id="s3-7"></a>
### 3.7 What the feed changes, and the commands (PP11)

| Event | Effect on the panel |
|---|---|
| `TraitPoints { available, total, tree }` | fills the points field (§3.4) for the tree (or class) |
| `TraitSlotted { slot, what, rank }` | sets equipped slot *slot* to trait *what* at *rank* (`what` none clears it) |
| `TraitRank { tree, what, rank }` | sets the rank shown on the cell of trait *what* in *tree* |
| `TraitTreeChanged { tree, branch }` | sets the active tree tab and its chosen branch (`tree` none: no active tree) |

The client half **spends nothing itself**. A drag of a trait onto an equipped slot, or a click
on an equipped slot to clear it, sends `SlotTrait { slot, what }` (`scenario-feed.md` §9.4);
the slot changes only when the answering `TraitSlotted` arrives. A click on a tree cell to
increase (or a modified click to decrease) a trait's rank sends `SetTraitRank { tree, what, rank
}`; the cell changes only on the answering `TraitRank`. Choosing a tree's branch (e.g. a
specialisation) sends `ChooseTraitBranch { tree, branch }`; the view changes only on the
answering `TraitTreeChanged`. (`scenario-feed.md` §6.6 "Meaning for the client half".)

> **OPEN:** (P6) The gesture that issues `SlotTrait` (drag from the All list to an equipped slot,
> or a context action) and the gesture that issues `SetTraitRank` (left-click to add a point,
> right-click to remove, or +/− buttons) are UI design and are not in the data. Tessera chooses:
> left-click a cell adds one rank (`SetTraitRank` with rank + 1), a modified click removes one;
> drag from the All/Equipped list to a slot issues `SlotTrait`. A later revision may refine this
> once owner comparison (§10) shows the original's gestures.

---

<a id="s4"></a>
## 4. The skill-training window

<a id="s4-1"></a>
### 4.1 Reaching it (PP13)

The skill-training window opens from an **NPC trainer action**: the NPC action picker
(`npc-windows.md` §5.3) offers a trainer entry (an `ActionPickerUI_Action` key of the
`NPCActionPickerType` mapper, e.g. a trainer action), and choosing it makes the feed send
`TrainerShown` (`scenario-feed.md` §6.6). `TrainerShown.offers` tells which rows the window
draws: `skills`, `traits` or `recipes`; for `skills` and `traits` the skill-training window of
§4.2 opens. **Tessera rule (revision of 2026-10-23, PP18):** for `recipes` the skill-training
window of §4.2 opens too, with one row per offered recipe (icon, name, level and coin cost
from the entry), and Train answers with `TrainSkill { window, entry }` as for skills. The
crafting trainers of `crafting-panels.md` §8.3 are reached only by `CraftTrainerShown`: they
show one profession, vocation or guild, have no rows and answer with `TrainCraft`.

> **Was OPEN:** (implementer, `tessera-client`, 2026-10-23) a `TrainerShown` with `offers=recipes`
> carries rows (entries with a cost, level and `TrainSkill` answers), while the crafting-trainer
> windows of `crafting-panels.md` §8.3 have no rows: they show one profession, vocation or guild
> and answer with `TrainCraft` (and are opened by `CraftTrainerShown`). The client therefore shows
> a `recipes` offer in the skill-training window of §4.2 (rows of recipes, Train sends
> `TrainSkill { window, entry }`, as the committed `crafting` scenario expects). Which window the
> original shows for a recipe trainer, and whether §4.1 meant this, is for the spec writer.
>
> **Answered (spec writer, 2026-10-23):** the client's reading is the rule now (above). The
> earlier sentence that sent a `recipes` offer to the crafting trainer could not work: those
> windows have no rows and the offer's entries answer with `TrainSkill`. Which window the
> original uses for an NPC that teaches recipes was not observed; that stays an owner
> observation (§10).

<a id="s4-2"></a>
### 4.2 Layout (OBS, PP12)

The window is the layout `skilltrainer` (0x22000030): `SkillTrainer_Window` (a `UIScaler`, 407 ×
498) with a title bar (`SkillTrainer_TitleBar` → `SkillTrainer_WindowLabel`), a move bar
(`SkillTrainer_MoveBar`) and a close button (`SkillTrainer_CloseButton`). Two mutually exclusive
root fields:

- `SkillTrainer_CannotUseSkillTrainer` with a reason text (shown when the player may not use this
  trainer);
- `SkillTrainer_CanUseSkillTrainer` with the trainer content:

| Element | Role |
|---|---|
| Tabs `SkillTrainer_{Active,Passive,Trait,Gambit}Tab` and the Skirmish tabs `SkillTrainer_Skirmish{Role,Skill,Training,Personal}_Tab` | switch the page |
| Pages `SkillTrainer_{Active,Passive,Trait,Gambit}_Page` and the Skirmish pages, each a list `Tree` (`SkillTrainer_*_Tree`) with a scroll bar | the rows |
| `SkillTrainer_Expand_All_Button`, `SkillTrainer_Collapse_All_Button` | expand/collapse the tree |
| `SkillTrainer_ShowTrainableSkillsOnlyButton` / `_Text` | a checkbox filtering to trainable rows |
| `SkillTrainer_TrainButton` (a `TextButton`) | train the selected row |
| `SkillTrainer_BuyNowButton` / `_Text` | buy with mithril (a store path; Tessera hides it, §6) |
| `SkillTrainer_PlayerMoney` (based on `MoneyDisplay_PlayerMoney_YourMoneyTemplate`) | the player's coin (`npc-windows.md` §9.2) |
| `SkillTrainer_PlayerPVPPoints`, `SkillTrainer_PlayerSkirmishPoints` | alternate currencies for the PvP/skirmish pages |

<a id="s4-3"></a>
### 4.3 Rows and states (PP14)

The trainer rows come from `TrainerShown.entries` (`scenario-feed.md` §9.1/§9.2): each entry has
an `entry` index, a `what` (the skill or trait DID or a literal ref), a `coin` cost, a required
`level`, an optional `rank`, and `flags` (bit 0 already trained, bit 1 requirements not met). A
row shows:

| Column | Source |
|---|---|
| Icon | the skill's/trait's icon (`Skill_SmallIcon` / `Trait_Icon`, composed per `icon-layers.md`) |
| Name | the skill/trait name (`string-table.md`) |
| Level (and rank) | the entry's required `level`, and `rank` when present (shown with the level) |
| Cost | the entry's `coin`, drawn as the money display's coin split (§4.5) |

States: a row with `flags` bit 0 (already trained) is **ghosted**; a row with bit 1 (requirements
not met) is ghosted and non-selectable; an **affordable, available** row is normal and selectable;
a **selected** row is highlighted. The "show trainable only" checkbox hides the ghosted rows. The
train button is enabled only when a selectable, affordable row is selected.

The window also shows the **untrained-skills** list where the panel is player state rather than a
trainer window: `UntrainedSkills { entries }` (`scenario-feed.md` §9.2) is the whole list of
skills the player can still train, each with `coin`/`level`/`rank`; the skill-training view shows
its latest list (and the plugin view `LocalPlayer:GetUntrainedSkills()`, §7).

<a id="s4-4"></a>
### 4.4 The train button, `TrainSkill`, and what changes (PP14)

Clicking `SkillTrainer_TrainButton` with a selectable, affordable row selected sends
`TrainSkill { window, entry }` (`scenario-feed.md` §9.4), carrying the open trainer window's ID
and the selected row's `entry` index (not the skill DID; the server resolves the entry). The
client spends no coin and trains nothing itself. The window changes only on the answering events:

- `SkillLearned { what }` adds the skill to the player's trained skills (and the plugin
  `LocalPlayer:GetTrainedSkills()` list), removes it from `UntrainedSkills`, and updates the
  quickslot/skill presentations of `combat-ui.md`/`skill-tooltips.md`;
- `SkillForgotten { what }` removes it again;
- a fresh `TrainerShown` / `UntrainedSkills` re-draws the rows (the trained row now ghosts).

<a id="s4-5"></a>
### 4.5 Cost and the money display (PP13)

The entry's `coin` cost is drawn with the P3 money display and coin split of `npc-windows.md`
§9.2 (three coin elements — copper, silver, gold — hidden until filled; worth and cost labels).
`SkillTrainer_PlayerMoney` shows the player's coin from the latest `WalletChanged { Coin }`
(`scenario-feed.md` §9.2), and a row whose cost exceeds it is drawn in the `TooExpensive` state
(red) and is not affordable. PvP and skirmish pages use their own currencies (shown by
`SkillTrainer_PlayerPVPPoints` / `_PlayerSkirmishPoints`); the P6 feed drives only the coin path,
so Tessera shows the coin cost and leaves the alternate-currency rows for a later revision.

> **OPEN:** (P6) The P6 feed's `TrainerShown`/`UntrainedSkills` entries carry only a coin cost
> (and level/rank); the alternate currencies (PvP points, skirmish marks, mithril) are not in the
> feed. A later revision should add a currency to the entry so the skirmish and PvP trainer pages
> can show their costs; until then those pages show the coin cost only.

---

<a id="s5"></a>
## 5. The trait-tree census (criterion 1)

Decision 0037 §1 item 1 sets P6 exit criterion 1 for trait trees: every trait-tree record
renders or is categorised, with counts recorded as evidence. This section defines the census the
decision holds open for.

<a id="s5-1"></a>
### 5.1 The record set (OBS, PP8, PP9)

The census record is the **trait-tree record**: every object record of the trait-tree class of
`combat-data.md` §8.5 (class 3486 on this install), found at run time by root class (prefix 0x70,
root class at payload offset 4), never by a hard-coded DID. Each tree is visited once. Its
**placements** — the entries of `Trait_TraitTree_TraitArray` — are parts of the tree, not records
of their own: each (tree, placement index) is visited exactly once, with its tree, and the
placements decide the tree's category (decision 0037 §1 item 1). A trait DID that no tree places
is not part of this census (other trait classes are counted by their own panels, not here).

<a id="s5-2"></a>
### 5.2 Categories

Each placement is put in the **first** category that applies; the tree takes the first category
of its placements (`Empty` when it has none):

| Category | A placement is this when |
|---|---|
| `TraitMissing` | its `Trait_TraitTree_Trait` is not a trait-family record (`combat-data.md` §8.1) |
| `NameMissing` | the trait's `Trait_Name` does not resolve to text |
| `CellMissing` | its `Trait_TraitTree_TraitLocation` is absent, 0, or a key of no form in §3.2 (not a grid cell *a*×65,536+*b* with *a* 1–8, *b* 1–12, and not a floater 0x10000000+*n* with *n* 1–8) |
| `TooManyCells` | the placement is past the first 4,096 of its tree (the page's bound), or the tree has more than 4,096 placements |
| `DependencyMissing` | a `Trait_TraitTree_TraitDependency` names a cell that no placement of the same branch occupies and that is not the tree's single placement at that cell (§3.2) |
| `Shown` | none of the above: the cell and the trait's tooltip are built |

The tree is **rendered** (`Shown`) when the panel builds its page with every cell, rank and
prerequisite above; a placement is **rendered** when its cell and its trait's tooltip are built
(decision 0037 §1 item 1). A category other than `Shown` is a **reason**, not a failure; a
failure is only a placement whose building panics, logs an error, or leaves a required part
unbuilt without a category that names the reason (decision 0037 §1 item 4; invariant PPI7).

A dependency without `Trait_TraitTree_TraitDependencyRank` is read as rank 1; a stored rank
outside 1 … 65,535 is clamped to that range (§3.2).

<a id="s5-3"></a>
### 5.3 Counts on this install (OBS, PP9, Confirmed 2026-10-16)

Observed over every class-3486 record and every placement:

| Quantity | Count |
|---|---|
| Trait-tree records | **17** |
| Distinct tree types (`Trait_TraitTree_TreeType`) | **17** (keys 1, 2, 3, 13, 35–47) |
| Placements | **1,216** |
| Placements `Shown` | **1,216** (every tree and every placement `Shown`; 0 in any other category) |
| Trees `Shown` | **17** of 17 |
| Distinct placed traits | **914** (all class 3438, all with `Trait_Name` and `Trait_PointBasedTrait_PointCostProgression`) |
| Distinct branch keys | **55** |
| Distinct location keys | **99** (92 grid forms, *a* 1–8, *b* 1–12; 7 floaters, *n* 1–7; floater 8 defined but unused) |
| Zero / bad-form location keys | **0** |
| Dependencies | **511** (all resolve within their branch; 0 ranks outside 1–65,535) |
| Cell keys shared by more than one placement | **97**, in **4** of the 17 trees |

These are evidence for this install, not constants: a later install may differ, and a count that
differs from this spec is a **warning** (data change), not a failure (decision 0037 §1 item 5).
The census asserts the rules of §5.2, not the numbers.

---

<a id="s6"></a>
## 6. Tessera fallback windows

When a layout record named here is missing (or fails to decode), Tessera draws its own window
with the same information, as the other P-track panels do (`game-panels.md` §11 fallback;
`npc-windows.md` fallback widgets). The fallbacks are:

- **Experience bar fallback:** a horizontal bar filled to `current ÷ needed` with the level and
  ratio as text, in Tessera's own frame (no toolbar art). Used when `toolbar` or `LevelMeter`
  is missing.

  > **Was OPEN:** (implementer, `tessera-client`, 2026-10-23) "missing" read as: the HUD's toolbar is
  > open but has no `LevelProgressMeter` element; with the HUD off (`--no-ui-hud`) no bar is
  > shown unless asked for by name (`--open-panel experience`). Confirm or say whether the
  > fallback bar should show whenever the toolbar is not open.
  >
  > **Answered (spec writer, 2026-10-23), Decision (PP18):** confirmed. "Missing" means
  > missing from the install: the toolbar layout or its `LevelProgressMeter` element is absent
  > or does not decode while the HUD's toolbar is open; then the fallback bar takes its place.
  > A toolbar that is closed, or a HUD turned off (`--no-ui-hud`), is the user's choice and
  > shows no bar; `--open-panel experience` opens the bar (the install's, or the fallback when
  > the install lacks it) on its own.
- **Trait panel fallback:** tabs for the trees the feed names; each tree a simple grid of cells
  placed by the location key of §3.2 (row × column, floaters in a top row), each cell an icon
  with a rank number and a state border (green available, grey locked, blue slotted); a points
  line (spent / available); a met and an unmet prerequisite shown by arrows or a locked border.
- **Trainer window fallback:** a list of rows (icon, name, level, coin cost), ghosting the
  already-trained and the requirements-not-met rows, with a selection and a Train button; no
  mithril buy-now button and no alternate-currency rows.

The fallbacks are what the tier-1 (committed, synthetic) goldens of decision 0037 §2 capture.
Tessera **hides** the store paths in every case: the `SkillTrainer_BuyNowButton` (mithril) is not
drawn, and no window offers a purchase (Tessera has no store; consistent with `game-panels.md`'s
hiding of housing/medal/lock elements as a Tessera rule).

---

<a id="s7"></a>
## 7. Plugin views

The progression plugin views are `lua-plugin-api.md` §15.6 (Review): `LocalPlayer:GetTrainedSkills()`
returns the player's trained-skill list (one `SkillList` per apartment, §16) and
`GetUntrainedSkills()` returns the untrained list fed by `UntrainedSkills` (§4.3). The trait and
trait-tree plugin views, if any, are defined there; this spec adds no plugin API. The views read
the same models the panels do; they never issue the progression commands (those are the panels'
and the built-in dispatcher's).

---

<a id="s8"></a>
## 8. Invariants

Data invariants (checkable against the install; values are this install's):

- **PPI1** Every class-3486 record has `Trait_TraitTree_TreeType` and a `Trait_TraitTree_TraitArray`;
  across 17 trees there are 1,216 placements (§5.3).
- **PPI2** Every placement's `Trait_TraitTree_Trait` is a class-3438 record with `Trait_Name` and
  `Trait_PointBasedTrait_PointCostProgression` (914 distinct traits, §5.3).
- **PPI3** Every placement's `Trait_TraitTree_TraitLocation` is 0, a grid key (*a* 1–8, *b* 1–12)
  or a floater key (0x10000000 + *n*, *n* 1–8); 0 of 1,216 are outside these forms.
- **PPI4** Every dependency resolves to a cell within its placement's branch (or to the tree's
  single placement at that cell); 511 of 511 resolve (§3.2, §5.3).
- **PPI5** The toolbar `LevelMeter` has the child `LevelProgressMeter` `LevelMeterFill` with
  exactly the four states `normal`, `bonus`, `xp_suppressed`, `xp_suppressed_with_bonus`.
- **PPI6** The cell template `PointBasedTraitTree_TraitBase` has exactly the four states `normal`,
  `highlight`, `ghosted`, `ghosted_highlight`, and names a rank-overlay, icon and static-overlay
  element.

Behaviour invariants (for Tessera's implementation and synthetic fixtures):

- **PPI7** The census visits every trait-tree record once and every placement once; every
  placement lands in exactly one category; no placement is a failure (decision 0037 §1 item 4).
- **PPI8** The experience bar's fill is `clamp(current ÷ needed, 0, 1)` and is 0 when `needed` =
  0; a `LevelChanged` that does not raise the local player's level, or is `quiet`, runs no
  level-up presentation (§2.2–§2.3).
- **PPI9** No panel spends a point, trains a skill, or slots a trait on its own: the view changes
  only on the answering event after the command (`SlotTrait`/`SetTraitRank`/`ChooseTraitBranch`/
  `TrainSkill`) (§3.7, §4.4).
- **PPI10** A trainer row with `flags` bit 0 or bit 1 is non-selectable and the Train button is
  disabled for it; a row whose coin cost exceeds the player's coin is `TooExpensive` and not
  affordable (§4.3, §4.5).
- **PPI11** No window draws a mithril buy-now button or any purchase (Tessera rule, §6).

---

<a id="s9"></a>
## 9. Worked synthetic examples

All refs below are literals; no install is needed (the examples drive the fallback windows).

<a id="s9-1"></a>
### 9.1 Experience and a level-up

Feed: `xp 1500/2000` → `ExperienceChanged { current: 1500, needed: 2000 }`. The bar fills to
0.75 and shows level and "1500 / 2000". Then `level 12` (the player was 11) → `LevelChanged {
entity: player, level: 12, quiet: no }`: the level rises, so the client posts its level-up notice
line, plays its cue, and sets the bar's level to 12. A following `xp 0/2600` refills the bar to
0. A `level 12 quiet` (no change) runs no presentation.

<a id="s9-2"></a>
### 9.2 A trait tree

Feed for a tree ref `T1`: `trait-points 3/10 tree=T1` → the points field shows spent 7, available
3, total 10. `trait-rank T1 Trait_A 2` places `Trait_A` in its cell at rank 2 (overlay "2"); if
`Trait_A`'s location key is 0x00020003 the cell is row 2, column 3. `trait-tree T1 branch=2`
selects tab T1 and branch 2. A left-click on `Trait_A`'s cell sends `SetTraitRank { tree: T1,
what: Trait_A, rank: 3 }`; the cell shows rank 2 until the answering `trait-rank T1 Trait_A 3`
arrives. A trait `Trait_B` whose dependency (cell of `Trait_A`, rank 3) is unmet draws `ghosted`
with a down-arrow from `Trait_A`'s cell.

<a id="s9-3"></a>
### 9.3 A skill trainer

Feed: `trainer skills=(Skill_X,Skill_Y) coin=50,0 level=10,12 trained=(2) offers=skills` →
`TrainerShown` with entry 1 (`Skill_X`, coin 50, level 10) and entry 2 (`Skill_Y`, coin 0, level
12, flags bit 0 trained). The window shows row 1 selectable with a 50-copper cost split, row 2
ghosted (trained). Selecting row 1 and clicking Train sends `TrainSkill { window, entry: 1 }`.
The answer `learn Skill_X` → `SkillLearned { what: Skill_X }` adds `Skill_X` to the trained list
and removes it from `UntrainedSkills`; a fresh `trainer` now ghosts row 1.

---

<a id="s10"></a>
## 10. Owner comparison points (for O160+)

To be turned into owner rows O160+ by a clean task (do not edit `owner-observations.md` here).
Each asks the owner to compare Tessera with the original client on the owner's install:

- **O160** The toolbar experience bar: fill fraction, the text shown (level / percentage /
  current-over-needed), the rested-experience overlays, and the bar's tooltip content (§2.1,
  §2.2, PP4).
- **O161** The level-up presentation: the chat/notice wording, the fanfare sound and any screen
  effect at an ordinary level-up, and anything special when a level-cap item unlocks (§2.3).
- **O162** The trait panel: tab set and order for the player's class, the grid layout of each
  tree (branches, cell positions, floaters), and whether Tessera places each trait in the right
  cell (§3.1, §3.2).
- **O163** Trait-cell states and colours (available, locked, slotted, max rank), the rank overlay
  text and the dependency arrows (§3.3, §3.6).
- **O164** The points display wording and values (spent / available / total) and the trait
  tooltip content (§3.4, §3.5).
- **O165** The gesture to add/remove a rank and to slot/unslot an equipped trait, and the equipped
  slot categories shown for the class (§3.6, §3.7).
- **O166** The skill-training window: reaching it from the NPC action, the tab set, row content
  (icon, name, level, cost), the ghosting of trained and unmet rows, the "trainable only"
  checkbox, and the Train button (§4); and which window an NPC that teaches recipes opens
  (§4.1, Tessera rule of 2026-10-23: the skill-training window with recipe rows).
- **O167** The coin cost split and the player-money display in the trainer window, and whether
  Tessera correctly hides the mithril buy-now path (§4.5, §6).

---

<a id="s11"></a>
## 11. Notes for other specs

- `combat-data.md` §8.5 OPEN Q11 (which of *a*, *b* is the row/column of a trait-cell key, and
  what the 0x10000000 group is) is **answered** by §3.2 / PP6: *a* = row, *b* = column, 0x10000000
  + *n* = floater *n*, read from the trait panel's `UI_TraitTree_TraitBaseLocationArray`. A later
  `combat-data.md` revision may cite this section; this spec does not edit it.
- `keymap.md` §8.3 now carries the rows for `ToggleTraitPanel` (`toggle_traits`, J) and
  `ToggleSkillPanel` (`toggle_skills`, K); the §1.4 OPEN is answered (2026-10-23).
- `scenario-feed.md` §6.6: this spec's OPENs ask for feed forms for rested/bonus/suppressed
  experience (§2.2), a trainer-entry currency (§4.5) and, optionally, a trait-slot category
  (§3.6). They do not block P6; they would let the panels show more from data than from Tessera
  defaults.

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-16 | Independent review (spec reviewer, Clean-Room-Role: review; wrote no part of the spec or the P6 code): passed against CLEAN_ROOM.md §5.3, decision 0024 §2/§4 and `server-content.md` §2; status Review → Accepted. Own M1 probe confirmed the "every trait tree" census (17 class-3486 records, found by root class over all 383,332 object records) and the existence of the five UI-layout records; sub-counts match the Accepted `combat-data.md` §8.5; feed shapes match `scenario-feed.md` P6. Two non-blocking notes (element names are symbolic IDs; layout-internal invariants not re-decoded) and a keymap scope note recorded in the header review record. No text changed besides the header and this row. |
| 2026-10-16 | First version (Review). Rewritten from dirty-room observation after the original P6 spec was lost. Covers the toolbar experience bar and level readout with the local level-up presentation (§2); the trait panel — trees-to-tabs, the cell grid with branches, location keys (row × column, floaters), ranks, points, the trait tooltip, cell states, equipped-trait slots and the `SlotTrait`/`SetTraitRank`/`ChooseTraitBranch` commands (§3); the skill-training window reached from an NPC action, its rows, states, cost and the `TrainSkill` command (§4); the trait-tree census for decision 0037 criterion 1 with counts on this install (§5); the fallback windows (§6); plugin views (§7); invariants (§8); worked examples (§9); owner comparison points for O160+ (§10). Facts are OBS (install layouts 0x22000033, 0x22000091, 0x22000030 and the class-3486 census, all re-observed read-only 2026-10-16) and Reported (companion specs), with Tessera Decisions for the level-up presentation and fallbacks; no M4 pass this session. Answers `combat-data.md` §8.5 OPEN Q11 (§3.2, §11). |
| 2026-10-23 | Spec writer (dirty room); status Accepted → Revised, awaiting an independent review. New rows PP17 (keymap §8.3 default keys J and K, Reported) and PP18 (Decision). **§1.4:** the keymap OPEN is answered by `keymap.md` §8.3. **§2.3:** the notice mask `Standard` confirmed; the missing cue is interim. **§4.1:** a `recipes` offer opens the skill-training window with recipe rows answered by `TrainSkill` (behaviour as built; the earlier sentence naming the crafting trainer is replaced). **§6:** the experience-bar fallback is drawn only when the install lacks the meter, not when the toolbar or HUD is off. |
| 2026-10-23 | Header only: independent review by spec-review:spec-ui-scaling-p6:1 (independent session) of the 2026-10-23 revision (§1.4, §2.3, §4.1, §6; PP17, PP18): passed; status Revised → Accepted (record in the header). No fact or rule changed. |
