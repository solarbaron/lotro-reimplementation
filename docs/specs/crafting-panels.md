<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Crafting panels: the crafting panel and recipe page, the crafting bar and results, crafting trainers, gathering nodes and facilities

| | |
|---|---|
| **Status** | **Accepted** (review of the 2026-10-23 revision (§3, §12, §15.4; CP23) by spec-review:spec-ui-scaling-p6:1 (independent session), 2026-10-23: passed). It was revised (2026-10-23: the OPENs of §3, §12 and §15.4 answered; independent review passed). Accepted at review 1, 2026-10-16 |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Independent spec reviewer (dirty-room session; wrote no part of this spec or its notes in `economy.md` and `interaction.md`), 2026-10-16 |
| **Review record** | **Review 2026-10-23 of the 2026-10-23 revision (§3, §12, §15.4; CP23)** (spec-review:spec-ui-scaling-p6:1 (independent session), wrote no part of it): **passed**; status Revised → Accepted. Checked: checked against CLEAN_ROOM.md §5.3, every item, on the uncommitted worktree diff against main, with the contamination screen clean: every added statement is a functional fact, a marked Tessera rule or Decision, or an OPEN; no code, pseudo-code or listing-shaped text; no copied prose or non-interoperability identifiers; no game text, assets, keys, hostnames or traffic; provenance rows present and marked; no byte layout added; the answered behaviour is testable; unknowns stay OPEN or owner observations; nothing outside decision 0034 §2. Relative links and anchors resolve. Spot checks: `keymap.md` §8.3 carries `toggle_crafting` (T) and `toggle_crafting_1` … `3` (unbound) in `global`. First version for decision 0024 P6 (client half only): the crafting half of exit criterion 1 ("every … recipe record renders or is categorised", §10), the crafting part of criterion 2 (the "craft" leg of the progression scenario, §6, §13) and criterion 3 (golden images, §11, §15). Notes (no fact changed) added the same day to `economy.md` §15 and `interaction.md` (notes section). **Review 1, 2026-10-16** (independent reviewer, dirty-room session that wrote no part of this spec, of its P6 notes in `economy.md`/`interaction.md`, or of decision 0037; whole spec plus those notes and the §13 feed forms, as of branch `p6` HEAD, `git diff e9ed7bb..HEAD`): **passed**; status Review → Accepted. Checked against CLEAN_ROOM.md §5.3, every item: every statement is a functional fact (layout, element, widget type, rect, enum key, count, state, condition or sequence) or a clearly marked **Tessera rule** / **OPEN**; no code or pseudo-code, and the step lists of §4.5, §4.7, §5.6, §5.7 and §10.1 are plain-word conditions, not listing-shaped; no copied prose, comments or non-interoperability identifiers (the naming note scopes every quoted label as the install's own, read by label at run time); no game text (texts named by string table and token only, e.g. 0x04718A25; records by DID), no assets, keys, hostnames, endpoints, certificates, account data or captures; every fact has a provenance row CP1–CP23 with a mark (OBS = Confirmed over every record/element of the kind; BIN = decision 0011 disassembly only; BC = class-library code; Design = Tessera rule) and no BIN/BC fact is mislabelled Confirmed; the layout tables carry element, base/widget, rect and role, with units from `ui-layout.md` (design pixels), seconds (durations) and `interaction.md` (use distances); invariants CI1–CI9 are testable; unknowns are `> **OPEN:**`, never guessed; the BIN row confines M4 to crafting and trainer UI and states nothing was followed into network, login, launcher, anti-tamper or patcher code, and no anti-cheat/anti-tamper/code-signing fact or key value appears. **Decision 0024 §2/§4 (client half only; values scripted, never derived):** §0, the Tessera rules of §4–§9 and CI8 confine the client to presentation (layout, grouping, order, search, held-vs-needed counts from its own inventory model, and the displayed critical chance of EC §11.5.1, a client-side display formula in scope under §2 principle 5); professions, tiers, experience, learned recipes, cooldowns, tool bits, effects and every craft outcome come from the feed; §13's forms match the "P6 progression" revision of `scenario-feed.md` (§6.6, §8.3, §9, §18.P6) line for line. **Server content rules (`server-content.md`):** §0 and §13 keep `tessera-server` data-free and rule-free (`tessera-gamedata` is client/tools only; the test server never sends these forms). **Spot checks (M1, read-only, and provenance):** the install at the task path is readable in place; the BIN row's client binary `x64/lotroclient64.exe` has SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee, matching the provenance row exactly (version 4906.0071.1794.4035); the companion-spec anchors this spec relies on resolve and agree — `economy.md` §11.5/§11.5.1 (critical chance), `interaction.md` §3.9.1 (`Channeling_BarredActions` bit `Crafting` = 8, and the barred-action census matching the added note), `combat-ui.md` §7 (cast/induction/channel bars), `item-tooltips.md` §6.20/§8.3 (compare type 4 `Recipe`), `npc-windows.md` §5.3 (`ActionPickerUI_ActionEntryArray`, `NPCActionPickerType`); the internal counts of §4.5, §10.1 and CP21 are mutually consistent (1,339 one-use, 2,262 cooldown, 2,414 multi-output; 7,782 `Shown` + 9 `ResultImageMissing` = 7,791). The OBS census counts (7,791 recipes, 335 gathering nodes, 26 facility hotspots, 32 trainer NPCs, etc.) rest on the repository's own decoders over every record and were not independently re-derived here (a full property scan would need a heavy build that would collide with the other agents building in this worktree); they are evidence, not constants (§10.4, decision 0037 §1 item 5), so a later install differing is a warning, not a failure. **Non-blocking (for a later revision; no implemented behaviour need change):** (1) CI6 as worded contradicts §5.6/§5.7 for a cooldown recipe whose cooldown runs (M = 0, entry shows 1, Make enabled when ingredients are held, so Make sends 1 > M); the §14 OPEN already states the behaviour `tessera-gamedata` follows (send min(entry, M) when M > 0, the entry only when M = 0), so restating CI6 as "1 … M for Make when M > 0, and 1 when M = 0 with ingredients held" would make invariant and behaviour agree without changing code. (2) §10.1's implementer OPEN adds the `Unreadable` and `TooManyTiers` categories and the 64-tier/64-output work bounds (all 0 on this install); these are supersets that keep CI9 and the decision 0037 §1 census meaning, and the spec writer should fold the writer's confirmation into the §10.1 table in a later revision. Neither item blocks acceptance or alters the code on branch `p6`. The many remaining OPENs are server-side unknowns (owner observation) or implementer questions each carrying a stated Tessera-rule fallback; per §5.3 they do not block acceptance. |
| **Implements into** | `crates/gamedata` (`tessera-gamedata`: a crafting model over `tessera_formats::gameplay::economy`: recipe rows, recipe pages, trainer rows, node and facility views, and the census of §10; module name chosen by the implementer), `crates/ui` (`tessera-ui` game widgets: the crafting panel with its profession pages and recipe page, the three trainer windows, the fallback windows of §11; additive to the `game` module index), `apps/client` (`tessera-client`: wiring to the feed events, the commands of §13, the census run, `--open-panel` names), `crates/net` (only through the additive feed forms that §13 proposes for the `scenario-feed.md` revision labelled "P6 progression"; this spec changes no feed text), `crates/input` (the toggle of §12, through `keymap.md`) |
| **Companion specs** | [`economy.md`](economy.md) (**EC**: §5 recipes, professions, vocations, guilds, control and directory records; §11.5 critical chance, experience value, cooldown durations), [`interaction.md`](interaction.md) (**IA**: §3.3–§3.4 cursor, range and mouse-over, §3.8 inductions, §3.9 channelling states, §4 hotspots), [`npc-windows.md`](npc-windows.md) (**NW**: §3 using an NPC, §5.3 the choice window and its action entries, §8 loot window), [`scenario-feed.md`](scenario-feed.md) (**SF**: §5.8 property values, §6.2 `cast`, `cooldown`, `effect`, §6.6 progression and crafting, §9 events and commands), [`combat-ui.md`](combat-ui.md) (**CU**: §7 cast bar, TR25–TR28), [`item-tooltips.md`](item-tooltips.md) (**IT**: §4 rows, §6.20, §8.3 compare type 4), [`ui-layout.md`](ui-layout.md) (**UL**: layouts, `UILAYOUT`, §9 building panels, §17 opening), [`ui-widgets.md`](ui-widgets.md) (**UW**: §4 Meter, §8 ListBox, §9 Tree), [`ui-behaviour.md`](ui-behaviour.md) (**UB**: states, auto-fade, window manager), [`keymap.md`](keymap.md) (**KM**: §12.6 control codes), [`wlib-bytecode.md`](wlib-bytecode.md) (the class library's instruction set), [`property-system.md`](property-system.md) (§7.3 bitfield labels), [`string-table.md`](string-table.md) (StringInfo, arguments), [`entity-descriptor.md`](entity-descriptor.md) (§4.9 creation types) |

> **Naming note (interoperability).** Layout labels (`craft`, `craft_profession`,
> `vocation_trainer`, `profession_trainer`, `craftguild_trainer`, `examination`), element labels
> (`Craft_Window`, `Craft_Profession_Recipe_LB`, …), widget-type labels, state labels
> (`RecipeGroup_Active`, `requirement_notmet`, …), input-action labels (`ToggleCraftingPanel`,
> …), property names and enum labels are the install's own identifiers, read from the user's
> data (the property dictionary 0x34000000, the enum mappers, the DID maps). They are quoted
> only so that an implementation can find the data by them. Numeric IDs, DIDs, string-table
> tokens and class-library function IDs printed next to them are this install's values
> (2026-10-16) and must be looked up by label at run time. No game text appears here: texts are
> named by string table and token, and records by DID.

---

<a id="s0"></a>
## 0. Purpose

Decision 0024 P6 adds progression and crafting panels. This spec covers the crafting half (the
experience bar, levels, traits and skill training are the progression spec's). From the
install's UI layouts and records (M1), the class library's code (M1, data) and, where those
stop, the offline client (M4, decision 0011), it gives:

- the crafting panel: its layout and element roles, the profession tabs, the tier rows with
  proficiency and mastery meters, the recipe tree (grouping, order, row templates, counts,
  search), the recipe page (result, requirements, ingredients held and needed, optional
  ingredients and the critical result, cooldown, quantity and the make controls) and what a
  craft request carries (§3–§5);
- the crafting bar and how a craft result is presented (§6);
- the recipe and recipe-scroll tooltip rows (§7);
- the crafting trainers reached from the NPC choice window (§8);
- gathering nodes and crafting facilities in the world (§9);
- a census that puts every recipe, profession, gathering node and facility record of the
  install in one category, shown or a reason (§10);
- Tessera fallback windows (§11), keys (§12), the scenario-feed forms the panels need as an
  additive proposal (§13), invariants, examples, open questions and owner comparison points.

**Client half only** (decision 0024 §2 principle 4; `server-content.md` §2): the original
takes the character's professions, tiers, proficiency and mastery experience, learned
recipes, cooldowns, tool and facility state and every craft outcome from the server and from
class-library code it runs. Tessera runs none of those rules. Professions, tiers, experience,
learned recipes, cooldowns, effects, the tool bits and craft results come from the scenario
feed (or a server); `tessera-server` never runs them and links no data crate. The client
computes only presentation: layout, text, grouping and order for display, search, the
held-versus-needed counts of ingredients from its own inventory model, and the critical chance
the panel shows (EC §11.5.1, a display formula). Where the original's choice is game code that
was not traced, a **Tessera rule** stands in and is marked as such.

**Out of scope:** store and upsell parts (the store buttons, the profession-slot upsell tab and
page, the craft-experience acceleration button, the ingredient-pack purchase button, the
cooldown reset paid with store currency), item inscription after crafting (§5.9), item
advancement and repair windows (the repair, reinforcement and supplement texts of the
crafting string table belong to them), the hobby trainer (fishing; `HobbyTrainer` action) and
the progression panels.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (Steam, client build 4906.0071.1794.4035; archives unchanged since 2026-10-04, file times re-checked 2026-10-16), read in place and read-only with throwaway probes in the dirty-room scratch area built on the repository's own decoders. UI layouts decoded per `ui-layout.md` §4 with every property value: `craft` (`UILAYOUT` key 0x10000024, record 0x22000064), `craft_profession` (0x1000002E, 0x22000065), the recipe-page layout 0x22000081 and the ingredient layouts 0x22000082 and 0x22000083 (no `UILAYOUT` key), the dynamic-panel layout 0x2200090F, `vocation_trainer` (0x1000005A, 0x2200002B), `craftguild_trainer` (0x100000B0, 0x220006F9), `profession_trainer` (0x1000011D, 0x2200090C), `npc_actionpicker` (0x1000006E, 0x22000026), `examination` (0x22000000). The DID maps `UILAYOUT`, `STRINGTABLE`, `EMAPPER`, `WEENIECONTENT`; the enum mappers named in the text; the key-map record 0x1D000000 and the input-action mapper; the string table `UI_Crafting` (key 0x10000044, 0x250001D9, all 187 entries read for their argument IDs), `Examination` (0x10000053, 0x250001AF), `CraftSystem` (0x1000005A, 0x25000006). Every object record of `client_gamelogic.dat` with its 0x79 set, and every 0x78 set, searched for the crafting properties; **all** 7,791 recipes, 10 professions, 7 vocations, 7 guilds, 6,055 recipe scrolls, 32 crafting-trainer NPCs, 1,720 container templates (335 gathering nodes), 26 facility hotspots, 13 facility effects and the 16 induction records named here were decoded completely. The recipe object forms were read for their name string (all 7,791). |
| **BC** | The class library's code (record 0x56000000), data in the user's install (method M1), disassembled on 2026-10-14 and 2026-10-16 with a throwaway probe over the repository's `tessera-wvm` (`wlib-bytecode.md`, Accepted): the recipe-description function (class 263 function 104 here, EC §11.5) and the getters it calls; the tool and facility tests it calls; class 263 functions 0x01070006 and 0x01070049 (trainer texts) and their callees. Read only to see which record values the panel receives; no message text was read. |
| **BIN** | **M4 under the standing authorisation of decision 0011.** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee, Ghidra 12.1.3 headless on a private copy of the shared analysis project (`~/tessera-re/work/p6craft`, deleted after use), 2026-10-16. Functions were found by instruction search for the element IDs of the layouts above, the `UI_Crafting` tokens and the input-action IDs, and read by function. **Questions:** (c1) how the crafting panel builds its profession tabs and its recipe tree: which recipes, grouping, order, row templates, row texts, counts and states, search; (c2) what the tier rows show and when; (c3) what the recipe page shows for a recipe and which values drive it (result, requirements, ingredients, optional ingredients, critical chance, cooldown, quantity); (c4) what the make controls check and what the craft request carries; (c5) what the three trainer windows show and send. **Why M1–M3 were not enough:** the layouts give the elements and the class library hands the panel a recipe description, but the panel's own construction, grouping, counting, enabling and request code is native; M3 (watching the original) shows one character's panel at a time and cannot separate rules from data or show the request fields. Only crafting and trainer user-interface code was read; the request functions were identified by their argument lists only, nothing was followed into network, login, launcher, anti-tamper or patcher code. |
| **CMP** | The companion specs in the header (Accepted unless noted); their facts are cited with their own provenance. |
| **Design** | Tessera rules, marked **Tessera rule**: what Tessera does where the original's behaviour is server state, class-library game code not specified, store-only, or outside P6. Not claims about the original. |

Marks: **Confirmed** = checked over every record or element of the kind (OBS), or a BIN fact
the data agree with in every case; **BIN** = rests on the disassembly only; **BC** = rests on
the class library's code.

| # | Fact | Sources | Mark |
|---|---|---|---|
| CP1 | Window catalogue: layouts, keys, roots, widget types, scaling groups, rects, states; the profession page, recipe page, ingredient and tier templates; the trainer windows (§3, §4.2, §5.1, §8.3) | OBS | Confirmed 2026-10-16 |
| CP2 | Default binding: `ToggleCraftingPanel` (input action 0x10000040 here) on keyboard T (DirectInput code 0x14) in key map `Main`, no modifier, activation 0x03; `ToggleCraftingProf1Panel`, `…Prof2Panel`, `…Prof3Panel` (0x100001BF, 0x100001BE, 0x100001C0) have no default binding (§12) | OBS (key-map record 0x1D000000; codes per KM §12.6) | Confirmed 2026-10-16 |
| CP3 | Profession tabs: one tab and page per profession of the player, labelled with the profession's name; the panel title text; the no-profession field; the "train more" tab and page when the player has fewer professions than slots; the store tab otherwise (§4.3) | BIN (c1); OBS (tokens) | BIN |
| CP4 | Recipe tree: hierarchy tier → category → recipe; a recipe appears once under each of its tiers; tier rows and category rows created on first use; the row templates and their choice order (cooldown, one-use, multiple-output, plain); row texts (name, name with makeable count); the pack count; the `RecipeGroup_Active` state of rows and its propagation to category and tier rows (§4.4, §4.5) | BIN (c1); OBS (templates, tokens) | BIN |
| CP5 | Tree order: a comparator by tier on the top level and a case-insensitive comparator by row text on the second level; none on the recipe level (§4.4) | BIN (c1) | BIN (sort routine and its trigger not traced) |
| CP6 | Makeable count: the minimum over the main ingredients of held ÷ needed (integer division), 0 when any is 0; 1 for one-use recipes and for cooldown recipes that are ready; 0 for cooldown recipes waiting; pack count = held packs ÷ pack quantity (§4.5) | BIN (c1) | BIN |
| CP7 | Search: the typed text and each row's text are lowercased; the text is split at spaces and a row passes when every word occurs in its text; category rows are never filtered themselves, parents with all children filtered are hidden by the tree's `auto_filter_parent_with_filtered_children` (§4.6) | BIN (c1); OBS (tree properties) | BIN |
| CP8 | Tier rows: tier name from the `CraftTier` mapper's display text; proficiency meter on the tier after the last completed proficiency tier, proficiency icon on completed tiers; mastery meter on the tier after the last completed mastery tier (only on proficient tiers), mastery icon on mastered tiers; meter fill = current ÷ cap; meter text and tooltips (§4.7) | BIN (c2); OBS (templates, tokens) | BIN |
| CP9 | Recipe page: element roles, what fills each element, the states of the tool and facility lines, the ingredient slots and their held/needed text, the pack mode, the optional-ingredient slots and their checkbox rules, the mastery overlay, the critical result display, the critical chance (EC §11.5.1), the cooldown line and timer, the one-use label, the multiple-output selector, the experience line (§5) | BIN (c3); BC (description getters); OBS | BIN |
| CP10 | The class library's recipe description carries: result and critical result items and quantities, critical chance, experience key and value, facility effect and its name, tool name, one-use flag, cooldown key and duration, category, the two field icons, the level-munge flag, ingredients and optional ingredients with held counts (§5.2) | BC | BC |
| CP11 | Tool test: every bit of the profession's `CraftProfession_RequiredTool` is set in the player's `Inventory_CraftToolCategory`; facility test: the recipe names no `CraftRecipe_RequiredFacilityEffect`, or the player has an active effect of that DID (§5.3) | BC | BC |
| CP12 | Make enabled: no craft running, both tests met, every main ingredient (or the pack) and every checked optional ingredient held in the needed number, and the player's current channelling state (if any) does not have the `Crafting` bit of `Channeling_BarredActions` (§5.7) | BIN (c4); IA §3.9.1 (bit label) | BIN |
| CP13 | Quantity control: clamps to 1 … makeable count (0 when nothing is makeable), the arrow buttons step by one within that range, Make All sets min(makeable count, 999) and crafts; the entry holds up to 3 digits (§5.6) | BIN (c4); OBS (`UICore_Text_max_chars` 3) | BIN |
| CP14 | The craft request carries: the recipe, the selected tier, the selected output index, one flag per optional-ingredient slot (or none when no slot is checked), the quantity and the pack flag; Make greys both make buttons and, with the hide check box checked, sends the window to state `temp_hide` (§5.6) | BIN (c4) | BIN |
| CP15 | Trainer windows: the title field shows the NPC's `Name`; the text field shows the profession's or vocation's learn or change text (class-library choice) or the guild's text; the profession trainer's replace selector lists the player's professions and is shown only when the player has no free profession slot; the vocation train button asks for confirmation when the player already has a vocation (§8.3) | BIN (c5); BC (text choice) | BIN |
| CP16 | Choice-window action entries for crafting trainers (`ProfessionTrainer` 38, `VocationTrainer` 5, `CraftGuildTrainer` 24): one row template per offered record, the pass-through properties `CraftUI_ProfessionDID`, `CraftUI_VocationDID`, `CraftUI_CraftGuildDID` and the script properties (§8.1) | OBS | Confirmed 2026-10-16 |
| CP17 | Trainer NPC census: 21 NPCs offer all 10 professions, 10 offer all 7 guilds, 1 offers all 7 vocations (§8.2) | OBS | Confirmed 2026-10-16 |
| CP18 | Gathering nodes: 335 container templates with profession, tool and tier requirements, resource type, a gathering induction per profession, use distance 3.5, the container usage action (mouse-over type `Container_Open`), an experience progression driven by the node's tier (§9.1) | OBS | Confirmed 2026-10-16 |
| CP19 | Facilities: 13 facility effects named by 7,437 recipes; 26 hotspot templates and 62 items carry a facility effect in their effect lists (§9.2) | OBS | Confirmed 2026-10-16 |
| CP20 | Recipe census (§10.1): every recipe has a profession, a tier with a display text, a category with a display text, a name (the object form's name equals `CraftRecipe_Name` when present, else the result item's `Name`, in all 7,791), named ingredients (≤ 6), optional ingredients (≤ 2) and named multiple outputs; 9 recipes have a non-item result and critical result without any image | OBS | Confirmed 2026-10-16 |
| CP21 | Recipe scrolls: all 6,055 name a recipe whose profession and tier equal the scroll's `Usage_RequiredCraftProfession` and `Usage_RequiredCraftTier`; 1,339 name one-use recipes, 2,238 cooldown recipes; the scroll layout holds the one-use and cooldown rows (§7) | OBS | Confirmed 2026-10-16 |
| CP22 | `CraftRecipe_ExecutionTime` equals the `Induction_Duration` of the recipe's `CraftRecipe_InductionAction` in 6,732 recipes and differs in 1,059 (§6.1) | OBS | Confirmed 2026-10-16 |
| CP23 | Every rule marked **Tessera rule** | Design | Decision |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Panel** | The crafting panel, root `Craft_Window` (§3). |
| **Profession page** | One instance of `Craft_Profession_Window` inside the panel's dynamic panel, one per profession tab (§4.3). |
| **Recipe page** | `Craft_RecipePage`, an instance of `Craft_Recipe_Window` (§5). |
| **Recipe tree** | The tree `Craft_Profession_Recipe_LB` of a profession page (§4.4). |
| **Tier row**, **category row**, **recipe row** | The three node kinds of the recipe tree. |
| **Selected tier** | The tier of the selected recipe row (a recipe appears once per tier it belongs to). |
| **Held count** | How many units of an item the player holds (§5.4). |
| **Makeable count** | The number of crafts the held main ingredients allow (§4.5). |
| **Pack** | The recipe's `CraftRecipe_IngredientPack`, an item that replaces the main ingredients (`CraftRecipe_IngredientPackQuantity` per craft). |
| **Proficiency tier**, **mastery tier** | The tier whose proficiency or mastery experience the character is currently earning; tiers below it are completed (§4.7). |
| **Gathering node** | A container template with `Usage_RequiredCraftProfession` (§9.1). |
| **Facility** | An effect named by `CraftRecipe_RequiredFacilityEffect`; a player near a facility hotspot holds it (§9.2). |
| **Feed window** | A window of the scenario feed (SF §5.6), identified by `WindowId`. |

---

<a id="s3"></a>
## 3. Window catalogue (Confirmed, CP1)

Opening follows UL §17.1: find the layout by its `UILAYOUT` label, the root by its
`UIElementID` label.

| Window | Layout (key, record) | Root (widget type) | Scaling group, origin | Design rect | Other root properties |
|---|---|---|---|---|---|
| Crafting panel | `craft` (0x10000024, 0x22000064) | `Craft_Window` (`UIScalerPanel`) | `Panels` (31), (293, 285) | (7, 123) 800 × 580 | saves location; activatable, activates on show; blocks clicks; auto-fade 0.1; object mode `Stretch`; notify on resize; states `normal` (fade 0 → 1 over 0.5 s), `temp_hide` (fade 1 → 0 over 0.5 s, then state `temp_hidden`), `temp_hidden` (hidden) |
| Profession page | `craft_profession` (0x1000002E, 0x22000065) | `Craft_Profession_Window` (`Field`), placed through `Craft_ProfPageTemplate` | — | (13, 174) 378 × 523; as a page (6, 27) 375 × 522 | |
| Recipe page | layout 0x22000081 (no `UILAYOUT` key; reached as the base of `Craft_RecipePage`) | `Craft_Recipe_Window` (`Field`) | — | as `Craft_RecipePage`: (381, 31) 415 × 522 | erases its background |
| Vocation trainer | `vocation_trainer` (0x1000005A, 0x2200002B) | `VocationTrainer_Window` (`UIScaler`) | `Panels`, (174, 187) | (260, 118) 349 × 375 | saves location; activatable; auto-fade 0.1 |
| Profession trainer | `profession_trainer` (0x1000011D, 0x2200090C) | `ProfessionTrainer_Window` (`UIScaler`) | `Panels`, (174, 187) | (260, 118) 349 × 375 | as above; no skin ID |
| Craft-guild trainer | `craftguild_trainer` (0x100000B0, 0x220006F9) | `CraftGuildTrainer_Window` (`UIScaler`) | `Panels`, (174, 187) | (260, 118) 349 × 375 | as above |

The panel's dynamic panel `Craft_ProfessionDynamicPanel` (at (3, 24) 380 × 550, edges 5/5/5/5)
is based on `DynamicPanel_Template` of layout 0x2200090F: a horizontal tab list
`DynamicPanel_Tab_ListBox` (16, 10) 420 × 29 with a horizontal scroll bar under it, and the page
parent `DynamicPanel_Page_Parent`; minimum size 400 × 400.

**Tessera rule (`--open-panel` names; revised 2026-10-23):** `crafting`, `crafting-recipe`
(the panel with a recipe selected), `vocation-trainer`, `profession-trainer`,
`craftguild-trainer` open the windows above for inspection, once the player is in the world,
filled from the session's current state (the professions, recipes and vocation the feed or
server has given); with no such state they are empty. `crafting-recipe` selects the first
recipe row of the first tab when there is one. A trainer opened by name belongs to no feed
window: its train button sends nothing and its cancel button only closes it. The fixed
synthetic content of §15.4 is the fixture of the in-process tests and of the tier-1 goldens
(§11), not something an `--open-panel` name shows.

> **Was OPEN:** (implementer, `tessera-client`, 2026-10-23) the names are routed now
> (`progression::hub::P6Hub`): each opens its window once the player is in the world, filled
> from the session's current state, not from the fixed content of §15.4 (a literal recipe has
> no definition outside the in-process fixture, §15.4 OPEN), so with no feed state the windows
> are empty; `crafting-recipe` selects the first recipe row of the first tab when there is one.
> A trainer opened by name has no feed window: its train and cancel buttons send nothing and
> cancel only closes it. The spec could say whether the names should show §15.4's content.
>
> **Answered (spec writer, 2026-10-23), Tessera rule:** no; the names show the session's
> state as built, and the rule above now says so. A window that should show §15.4's content
> is reached by running a scenario that scripts it (the committed `crafting` scenario), so the
> client needs no second source of recipe definitions.

---

<a id="s4"></a>
## 4. The crafting panel

<a id="s4-1"></a>
### 4.1 Opening

`ToggleCraftingPanel` opens and closes the panel (CP2, §12). The close button
`Craft_CloseButton` closes it; the drag bar `MoveBar` (800 × 24) moves it. When the panel is
opened with the profession toggles of §12, Tessera shows the tab of that number (**Tessera
rule**; the original's handling of these toggles was not traced).

<a id="s4-2"></a>
### 4.2 Element tree and roles (CP1)

Panel `Craft_Window` (coordinates relative to it):

| Element | Base / widget | Rect | Role |
|---|---|---|---|
| `CraftWindow_BaseBox` | `Box_Silver` | (0, 18) 800 × 560 | background box |
| `CraftWindow_Background` | Field | (351, 37) 445 × 450 | background of the right half |
| `Craft_ProfessionDynamicPanel` | §3 | (3, 24) 380 × 550 | profession tabs and pages (§4.3) |
| `Craft_RecipePage` | recipe page | (381, 31) 415 × 522 | the selected recipe (§5) |
| `Craft_HideOnExecuteButton`, `Craft_HideOnExecuteText` | `Button_Checkbox_02`; text `fieldlabelsmall` | (386, 557) 10 × 10; (398, 553) 141 × 20 | "hide when crafting" check box (§5.6); label token 0x0C099D44 of table 0x250001BB, tooltip `UI_Crafting` token 0x0DF2CD34 |
| `Craft_NoProfessionField` → `Craft_NoProfession_Text_Label` | `Box_Silver`, blocks clicks; text `fieldlabellarge`, centred, margins 75 | (0, 18) 800 × 560; (150, 186) 500 × 151 | covers the panel when the player has no profession; text token 0x075E9A24 |
| `Crafting_Base_Box_TitleBar` → `Craft_Title` | `Base_Box_Titlebar`; `titlebarfont`, centred | (275, −11) 250 × 45; (15, 15) 223 × 20 | title (§4.3) |
| `MoveBar` | Dragbar | (0, 0) 800 × 24 | move |
| `Craft_CloseButton` | `Button_X` | (776, 24) 16 × 16 | close |

Templates of the `craft` layout (top level, not drawn by themselves): `Craft_ProfTabTemplate`
(a `Tab_Tier1_Left` tab, text margins 10, fits its text), `Craft_ProfPageTemplate` (based on
`Craft_Profession_Window`), `Craft_TrainProfTabTemplate` and `Craft_TrainProfPageTemplate`
(text `Craft_TrainProfPPage_Text_Label`, token 0x0A4CB654, centred), `Craft_UpsellTabTemplate`
(store).

Profession page `Craft_Profession_Window`:

| Element | Base / widget | Rect | Role |
|---|---|---|---|
| `Craft_Profession_RecipeField` | `Box_01_Empty` | (0, 25) 370 × 498 | holds the parts below (rects relative to it) |
| `Craft_TreeBox` | `Box_01` | (5, 27) 354 × 450 | frame of the tree |
| `Craft_Profession_Recipe_LB` | Tree | (5, 27) 354 × 450 | the recipe tree (§4.4): one column, vertical; selected-item state `highlight`, normal-item state `normal`, click selects; tree icon `Craft_Profession_TreeIcon` of this layout; indent 8; scroll bar `Craft_Profession_Recipe_SB`; `auto_prune` true; `auto_filter_parent_with_filtered_children` true; expand and collapse elements named by `UI_Craft_Profession_ExpandElement` / `…CollapseElement` |
| `Craft_Profession_Recipe_SB` | `Scrollbar2` | (358, 27) 10 × 450 | |
| `Craft_Profession_Expand_Button`, `Craft_Profession_Collapse_Button` | `ExpandAll_Button`, `CollapseAll_Button` | (4, 6), (23, 6) 16 × 16 | expand all, collapse all |
| `Craft_Profession_SearchText` | text `fieldlabelsmall` | (51, 4) 62 × 20 | label, token 0x0145AAA4 |
| `Craft_Profession_SearchEntry` (+ `_BG`) | text `verdana14`, editable, at most 80 characters | (101, 4) 142 × 20 | search entry (§4.6); tooltip token 0x0B412F50 |
| `WebStoreGetRecipesButton` | store button | (243, 0) 124 × 26 | store (out of scope) |

Row templates of the `craft_profession` layout (§4.5): `Craft_Profession_RecipeTemplate`,
`Craft_Profession_RecipeTemplate_OneShot`, `Craft_Profession_RecipeTemplate_MultiOutput`,
`Craft_Profession_RecipeTemplate_Cooldown` (Buttons, 350 or 400 × 20, rollover; states
`normal`, `highlight`, `…_rollover`, `…_pressed`, `ghosted`; highlight image 0x41005F9E), each
with the text `Craft_Profession_RecipeTemplateText` (`verdana16_crafting`, left, margins 6/10,
fits its text; state `RecipeGroup_Active` sets font colour 0xFF29DF46) and the pack field
`CraftProfession_RecipeTemplate_AvailablePacks` (at x 300, 53 × 16: an open-parenthesis text,
the pack icon 0x4110CB1B 16 × 16 and a count text, colour 0xFF1FA935, state
`RecipeGroup_Active` 0xFF29DF46). The one-use, multiple-output and cooldown templates add a
16 × 16 icon at (2, 2) and move the text to x 20: one-use icon 0x4101E103, multiple-output icon
0x410FFE56, cooldown icon `Craft_Profession_RecipeTemplateIcon` 0x410E7595 (state
`RecipeGroup_CooldownReady`: image 0x410E7596 and another tooltip); the cooldown template also
has the timer `Craft_Profession_RecipeTemplateTimer` (Timer, at x 260, 120 × 16, right-aligned,
colour 0xFFFFF600, font 0x42000008, outlined; minimum units day, hour (`MinMinutes` 60), month).
Tier row template `CraftProfession_TierTemplate` (340 × 20): the text
`CraftProfession_TierTemplateText` (150 × 20, `fieldvaluesmall`, colour 0xFFE2FB02) and a
horizontal list `CraftProfession_TierTemplate_LB` (at x 150, 190 × 20, one row, up to 4 columns)
for the icons and meters of §4.7. Meter templates `CraftProfession_ProfMeterTemplate` and
`CraftProfession_MasteryMeterTemplate` (149 × 20): meter `CraftProfession_Meter` (at (2, 0)
121 × 9, child-meter image 0x410000E2 for proficiency, 0x41007DF8 for mastery, fills from the
left, background images 0x41007E2A / 0x41007E2C) over `CraftProfession_Meter_Bg` (0x41007E2B /
0x41007E2D) and the centred text `CraftProfession_MeterText`. Icon templates
`CraftProfession_ProfIconTemplate` (0x41007E28) and `CraftProfession_MasteryIconTemplate`
(0x41007E29), 32 × 16, with tooltips. `Craft_Profession_TreeIcon` has states `open`
(0x41007E26) and `closed` (0x41007E27). `Craft_Profession_AdvancementTemplate`,
`Craft_Profession_AppearanceTreeIcon`, `CraftProfession_SpacerIcon`,
`CraftProfession_BuyNowButton` and `Craft_Profession_Upsell_Window` are not used by the code
read (the last two are store parts).

<a id="s4-3"></a>
### 4.3 Profession tabs (BIN, CP3)

When the panel is built:

1. The title `Craft_Title` gets `UI_Crafting` token 0x04718A25.
2. With no profession: `Craft_NoProfessionField` is shown and no tab is made.
3. Otherwise `Craft_NoProfessionField` is hidden and, for each profession the player has, in the
   order the player's profession list gives, one tab from `Craft_ProfTabTemplate` and one page
   from `Craft_ProfPageTemplate` are added to the dynamic panel; the tab's text is the
   profession's name.
4. When the player has fewer professions than profession slots, a tab from
   `Craft_TrainProfTabTemplate` with the page `Craft_TrainProfPageTemplate` follows (its page
   text tells the player to visit a trainer). Otherwise, when the player could buy more slots, a
   store tab follows (out of scope).

Switching tabs shows that profession's page; the recipe page then shows the selected recipe of
that page, or nothing (§4.8).

> **OPEN:** (implementer, `crates/ui`, 2026-10-16) rule 4 names no text or image for the
> "train more" tab itself, only for its page. On the owner's install the tab made from
> `Craft_TrainProfTabTemplate` has no text part and is small (26 × 11) next to the
> profession tabs. `tessera-ui` puts Tessera's tab label into a text part that is empty, and
> shows the label as the tab's tooltip when the tab has no text part. The spec should name
> the tab's label or image.

> **Tessera rule (tabs from the feed):** the player's professions are those the feed has
> reported with `ProfessionChanged` (SF §6.6), in the order of their first report. The
> "train more" tab is shown when the feed has given a slot count (§13, `vocation … slots=N`)
> larger than the number of professions; never the store tab.

<a id="s4-4"></a>
### 4.4 The recipe tree (BIN, CP4, CP5)

**Which recipes.** The tree lists only the recipes the character knows, for that page's
profession (the original takes the list from the profession state; a newly learned recipe adds
its rows). There is no "show all recipes" mode.

> **Tessera rule:** a recipe is listed on the page of its `CraftRecipe_Profession` when the feed
> has reported it with `RecipeLearned` and has not forgotten it (§13, `forget-recipe`); a learned
> recipe of a profession the player does not have is kept and listed once that profession is
> reported. The tree is rebuilt when the set changes.

**Hierarchy.** For each listed recipe and for each of its tiers (`CraftRecipe_Tier`, or each
element of `CraftRecipe_TierArray`; every array in this install holds one tier):

1. a **tier row** for that tier, at the top level, made on first use (one per tier present);
2. a **category row** for (tier, `CraftRecipe_UICategory`) under the tier row, made on first use
   (the same category under two tiers gives two rows);
3. the **recipe row** under the category row.

Each row carries for later use the recipe DID (`CraftUI_RecipeDID`, recipe rows), the tier
(`CraftUI_Tier`, tier and recipe rows), the category (`CraftUI_Category`, category rows) and its
text as sort key (`UI_Util_SortName`, category and recipe rows).

**Texts.**

- Tier row: `UI_Crafting` token 0x076C10C4 with argument 0x05697E45 = the tier's display text
  (mapper `CraftTier`, `EMAPPER` key 0x10000042 here, 0x23000063; display texts exist for keys
  1–16).
- Category row: a row from `Craft_Profession_RecipeTemplate` whose text is token 0x076C10C4 with
  argument 0x05697E45 = the category's display text (mapper `CraftUICategory`, key 0x10000070,
  0x23000065; 516 display texts, every key used by a recipe has one); its pack field is hidden.
- Recipe row: the recipe name; with a makeable count *n* > 0 the text is token 0x09B25265 with
  arguments 0x05697E45 = name and 0x00484A34 = *n*; with *n* = 0 token 0x076C10C4 (name only).
  The **recipe name** is the name string of the recipe's object form, which equals
  `CraftRecipe_Name` when the recipe has one (3,092) and the result item's `Name` otherwise
  (4,699) (CP20).
- Pack count: when the pack count *p* (§4.5) is above 0 the pack field is shown and its count
  text is token 0x06D688A4 with argument 0x00484A34 = *p*; otherwise it is hidden.

**Order (CP5).** The tree has a comparator for its top level and one for its second level:

- top level (tier rows): by `CraftUI_Tier`, ascending;
- second level (category rows): by `UI_Util_SortName`, compared case-insensitively, ascending;
- recipe rows: no comparator; they stay in the order they were added.

The tree's sort routine and when it runs were not traced (UW §9 has no sorting).

> **Tessera rule:** each tier row's children and the top level are kept sorted by these keys
> with ties in insertion order; recipe rows are in the order the feed reported the recipes
> (`RecipeLearned` order). Category text comparison folds case by Unicode simple lowercase
> mapping.

> **Tessera rule (expansion):** when a page is built every tier and category row is expanded;
> the expand-all and collapse-all buttons expand or collapse every node; the page keeps its
> expansion while the panel is open. (The original's initial expansion was not traced; owner
> check O186.)

<a id="s4-5"></a>
### 4.5 Recipe rows: template, counts and states (BIN, CP4, CP6)

**Template.** The first that applies:

1. the recipe has a cooldown (its description's cooldown duration is above 0; EC §11.5.3):
   `Craft_Profession_RecipeTemplate_Cooldown`;
2. it is a one-use recipe (`CraftRecipe_OneTimeRecipe` true): `…_OneShot`;
3. it has outputs to choose from (`CraftRecipe_MultiOutputArray`): `…_MultiOutput`;
4. otherwise `Craft_Profession_RecipeTemplate`.

In this install 2,262 recipes have a cooldown, 1,339 are one-use (none of them with a cooldown),
2,414 have multiple outputs (751 of them one-use, 397 with a cooldown).

**Held count.** For an item DID, the number of units the player holds (§5.4).

**Makeable count *n*.**

1. Start with no limit. For each main ingredient (`CraftRecipe_IngredientList` entry), take
   held ÷ needed (`CraftRecipe_IngredientQuantity`), integer division. If any gives 0, *n* = 0.
   Otherwise *n* is the smallest of them.
2. If *n* > 0: a one-use recipe gives *n* = 1. A cooldown recipe gives *n* = 1 when its cooldown
   is not running and *n* = 0 while it runs. Other recipes keep *n*.

**Pack count *p*.** When the recipe has a pack: held packs ÷ `CraftRecipe_IngredientPackQuantity`
(integer division); otherwise 0.

**Craftable.** A recipe is craftable when *n* > 0 or *p* > 0. A craftable recipe's row text takes
state `RecipeGroup_Active`; so does the text of every category row and tier row with at least
one craftable recipe below it; the others take the default state. Counts and states are
recomputed when the player's holdings of any ingredient or pack item change, and when a
cooldown starts or ends.

**Cooldown rows.** While the recipe's cooldown runs: the row shows the plain name (no count),
the pack field is hidden, the timer shows the remaining time and the icon is in its default
state (tooltip token 0x0CD2DB37). When it is not running: the timer is hidden, the icon takes
state `RecipeGroup_CooldownReady` and the row is filled like a plain row.

> **Tessera rule:** a recipe's cooldown runs from the feed's `CooldownStarted` with the recipe's
> ref to its `CooldownEnded` (SF §6.2 `cooldown`); the timer counts down to the end the feed
> scheduled.

> **OPEN:** (implementer, `apps/client`, 2026-10-26) the text of the remaining-time timer (the
> row's timer and `Craft_Recipe_CooldownTimer`, §5.2) is not given: its format, how a fraction
> of a second is rounded, what it shows once the scheduled end has passed while no
> `CooldownEnded` has arrived, and how often it is updated. `tessera-client` shows the
> remaining seconds (the scheduled end, taken as the `CooldownStarted` arrival time on the
> client's combat clock plus its `duration_ms`, less the current time) as the duration text of
> `item-tooltips.md` §6.18, as the §5.2 cooldown line does (largest two non-zero units, whole
> seconds rounded to nearest, `0s` from the scheduled end until `CooldownEnded`). When only
> that text changes it is set on the timer elements in place; the panel is refilled only when
> the set of running recipe cooldowns (or the holdings) changes, so a countdown neither
> rebuilds the recipe tree nor renders the recipe page again. The spec should state the timer's format and clock source,
> and whether the Timer widget counts down by itself.

<a id="s4-6"></a>
### 4.6 Search, expand and collapse (BIN, CP7)

Typing in `Craft_Profession_SearchEntry` re-filters the tree. A row passes when it has no sort
key (tier rows) or is a category row; a recipe row passes when every word of the typed text
occurs in its text. Both texts are lowercased first, the typed text is split at the space
character (empty words are skipped), and the row text is the full text of §4.4 (with its count).
An empty search passes every row. Parents whose children are all filtered out are hidden
(`auto_filter_parent_with_filtered_children`).

> **Tessera rule:** lowercasing uses Unicode simple lowercase mapping; the count part of the
> text takes part as in the original.

<a id="s4-7"></a>
### 4.7 Tier rows: meters and icons (BIN, CP8)

For tier *t* of profession *P*, with *a* = the last completed proficiency tier and *b* = the last
completed mastery tier (*b* ≤ *a*), the row's list gets, in this order:

1. If *t* ≤ *a*: the proficiency icon (`CraftProfession_ProfIconTemplate`, tooltip token
   0x058F0DF0). Then:
   - if *t* ≤ *b*: the mastery icon (`CraftProfession_MasteryIconTemplate`, tooltip 0x0BE0BED0);
   - else if *t* = *b* + 1: a mastery meter (`CraftProfession_MasteryMeterTemplate`) with the
     current mastery experience and the mastery cap, tooltip token 0x01A74BC0;
   - else nothing more.
2. Else if *t* = *a* + 1 and the proficiency cap is not 0: a proficiency meter
   (`CraftProfession_ProfMeterTemplate`) with the current proficiency experience and the cap,
   tooltip token 0x038E5010. (The original then adds a store button when the next tier needs
   a purchase; out of scope.)
3. Else nothing.

**Meters.** The meter's position (`UICore_Meter_position`) is current ÷ cap; its text
`CraftProfession_MeterText` is token 0x0C453864 with arguments 0x08A76A74 = current and
0x00005168 = cap; the template's tooltip is the token named above with the same two arguments.

> **Tessera rule (values from the feed):** `ProfessionChanged { tier, proficiency,
> proficiency_max, mastery, mastery_max }` (SF §9.2) gives the proficiency tier *T* and the
> meters. *a* = *T* − 1, or *T* when proficiency ≥ proficiency_max > 0. The mastery tier is the
> event's `mastery_tier` when the feed gives it (§13), else *T*; *b* is derived from it and the
> mastery values the same way. The position is clamped to 0 … 1; a cap of 0 shows position 0.
> The client never derives experience or caps from `CraftProfession_ProficiencyXPCap` /
> `MasteryXPCap` (EC §5.2); scenarios usually script those values.

<a id="s4-8"></a>
### 4.8 Selection

Selecting a recipe row shows that recipe and the row's tier on the recipe page (§5); selecting a
tier or category row, or no row, empties the recipe page. Switching tabs shows the selection of
the new page.

> **Tessera rule:** when a page is first shown no row is selected; after a rebuild the row of
> the same (recipe, tier) is re-selected when it still exists.

---

<a id="s5"></a>
## 5. The recipe page

<a id="s5-1"></a>
### 5.1 Element roles (CP1)

Coordinates relative to `Craft_Recipe_Window` (415 × 545). Five background boxes
(`quest_dialog_blue_bg`, 413 wide) mark its bands: output (0, 0) height 93, requirements (0, 94)
52, ingredients (0, 147) 197, optional (0, 345) 126, make (0, 472) 51.

| Element | Base / widget | Rect | Role |
|---|---|---|---|
| `Craft_Recipe_Item` | `ItemDisplay_ShortTemplate` of `itemdisplay` | (10, 10) 32 × 32 | result item and quantity |
| `Craft_Recipe_NonItem_Result` | Field | (10, 10) 32 × 32 | image of a non-item result |
| `Craft_Recipe_NameText` | `fieldlabelxlarge_ColorNameText`, left, centred vertically, truncated with automatic tooltip | (50, 5) 261 × 42 | name (§5.2) |
| `Craft_MultiOutput_Field` → `Craft_MultiOutput_Label`, `Craft_MultiOutput_PageCtrl` | Field; text (token 0x0820C094); `Silver_PagCtrl_Template` | (312, 6) 99 × 41 | output selector (§5.2) |
| `Craft_Recipe_EarnedPointsText` | `verdana14`, colour 0xFFF7E4A9, markup | (10, 58) 250 × 18 | experience line (§5.2) |
| `Craft_Recipe_OneUseOnly` | `verdana14`, right, hidden | (254, 56) 153 × 18 | one-use label, token 0x0CDC69A9 |
| `Craft_Recipe_CooldownDuration` | `verdana14`, hidden | (10, 75) 235 × 18 | cooldown length line |
| `Craft_Recipe_CooldownTimer` | Timer, colour 0xFFB80000, font 0x42000001, wrap token 0x02221134 | (240, 73) 169 × 20 | remaining cooldown |
| `Craft_Recipe_Cooldown_MithrilReset_Button` | store-currency reset | (302, 48) 107 × 24 | out of scope (hidden in Tessera) |
| `CraftRecipe_Panel` | Panel | (0, 94) 415 × 380 | holds the requirement and ingredient parts below (rects relative to it) |
| `CraftRecipe_Seal_Proficiency` | image 0x4110D071, tooltip 0x07067819 | (4, 55) 40 × 40 | seal at the ingredients |
| `CraftRecipe_Requirements` | Field | (0, 0) 416 × 249 | |
| `Craft_Recipe_RequiredToolText`, `Craft_Recipe_RequiredFacilityText` | `verdana14`, centred, colour 0xFF59BF0E | (3, 8), (3, 28) 410 × 18 | tool and facility lines (§5.3) |
| `CraftRecipe_MainIngredients` → `Craft_Recipe_IngredientLabel`, `Craft_Recipe_Ingredient1` … `6` | Field; text (token 0x081DD13C); `CraftIngredient` | (1, 50) 415 × 199; slots at (29, 62), (212, 62), (29, 108), (212, 108), (29, 153), (212, 153), 175 × 36 | main ingredients (§5.4) |
| `CraftRecipe_IngredientPacks` → label, `Craft_IngredientPack_Binding`, `CraftRecipe_IngredientPack_Display` | Field; texts; `CraftIngredient` 300 × 36 at (29, 62) | (1, 50) 415 × 199 | pack mode (§5.4) |
| `Craft_Recipe_UseIngredientPack` → `Craft_Recipe_Checkbox_TextLabel` | `Checkbox_WithText` | (54, 92) 350 × 16 | pack check box (§5.4) |
| `CraftRecipe_MasterPage` | Field | (1, 245) 414 × 134 | optional ingredients and critical result (§5.5) |
| `CraftRecipe_MasterOverlay` → label | Field, blocks clicks, hidden, dark fill 0xDD000000; text token 0x0C3A798D | (1, 250) 414 × 128 | covers the master page when the tier is not mastered |
| `CraftRecipe_Seal_Mastery` (in the master page) | image 0x4110D070, tooltip 0x0DC3D469 | (5, 5) 40 × 40 | |
| `Craft_Recipe_OptionalText`, `Craft_Recipe_OptionalIngredient_PageCtrl` | text (token 0x07C56B6C); `Silver_PagCtrl_Template` | (49, 7) 88 × 25; (141, 11) 92 × 20 | optional-ingredient label and pager |
| `Craft_Recipe_OptionalIngredient1` … `5` | `CraftIngredient_Optional`, all at the same rect | (49, 32) 360 × 36 | one optional slot each (§5.5) |
| `Craft_Recipe_CriticalField` → `Craft_Recipe_CriticalLabel`, `Craft_Recipe_CriticalChanceText` | texts (label token 0x0821D32C) | (235, 16) 175 × 25 | critical chance (§5.5) |
| `CraftRecipe_CriticalOutput_Label` | text, token 0x01C72FD4 | (9, 69) 183 × 20 | |
| `Craft_Recipe_CriticalItem` | `ItemDisplay_MediumTemplate` | (9, 92) 343 × 37 | critical result item and quantity |
| `Craft_Recipe_NonItem_CritResult`, `Craft_Recipe_CritName` | Field; `fieldlabelxlarge_ColorNameText` | (10, 94) 32 × 32; (42, 92) 311 × 37 | non-item critical result: image and name |
| `CraftRecipe_DisableInscription_Button` (+ label) | `Button_GOP_Checkbox_02` on `GOP_Craft_DisableInscription` | (235, 74) 10 × 10 | inscription option (§5.9) |
| `Craft_Recipe_Make_Decrease`, `EntryField` → `Craft_Recipe_NumberOfTimesEntry`, `Craft_Recipe_Make_Increase`, `EntryField_Label` | arrows; editable text, one line, centred, max 3 characters | (198, 495), (217, 495) 21 × 20, (238, 495); label (198, 475) | quantity (§5.6) |
| `Craft_Recipe_ExecuteButton`, `Craft_Recipe_MakeAllButton` | `TextButton` (tokens 0x00ED39FC, 0x029B9A4C) | (262, 493), (337, 493) 72 × 20 | make, make all (§5.6) |
| `Craft_BuyNow_CraftXP`, `Craft_BuyNow_Ingredients` | store buttons, hidden | | out of scope |

Ingredient template `CraftIngredient` (layout 0x22000083, 160 × 36; states `normal`,
`highlight` image 0x41007E2E): `CraftIngredientItem` (an `ItemDisplay_ShortTemplate`, (2, 2)
32 × 32) and `Craft_Ingredient_Info` (`verdana14`, (36, 2) 124 × 32, margins 2, truncated with
automatic tooltip). Optional template `CraftIngredient_Optional` (layout 0x22000082, 360 × 36):
`CraftIngredient_Optional_Item` (a `CraftIngredient`, 172 × 36), the check box
`CraftIngredient_OptionalButton` (`Button_Checkbox_03`, boolean, (272, 1) 16 × 16, states
`normal`, `highlight`, `ghosted`), its label (token 0x03D7CB85) and the bonus text
`CraftIngredient_Optional_CritText` (`fieldlabelmedium`, (318, 1) 42 × 18).

<a id="s5-2"></a>
### 5.2 Header: name, result, experience, cooldown, outputs (BIN, BC; CP9, CP10)

The class library describes the recipe to the panel (BC, CP10) with, among others: the result
item and `CraftRecipe_ResultItemQuantity` (default 1), the critical result item and
`CraftRecipe_CriticalResultItemQuantity` (default 1), `CraftRecipe_CriticalSuccessChance`, the
experience value (EC §11.5.2), the facility effect and its name, the tool name, the one-use flag,
the cooldown duration (EC §11.5.3), the category, `CraftRecipe_Field_ResultIcon` and
`CraftRecipe_Field_CritResultIcon`, `CraftRecipe_LevelMungeOutput`, and the ingredient,
optional-ingredient and pack lists with held counts.

- **Name.** `Craft_Recipe_NameText` shows the recipe name (§4.4); for a multiple-output recipe
  with a chosen output that is an item, that item's name. Its state is chosen from the shown
  result item (by a helper not traced; by inference the item's quality colour state).
- **Result.** When the result is an item record: `Craft_Recipe_Item` shows it with its quantity
  and `Craft_Recipe_NonItem_Result` is hidden. Otherwise (79 recipes name a container template,
  EC §1) the item display is emptied and `Craft_Recipe_NonItem_Result` shows the image
  `CraftRecipe_Field_ResultIcon` (draw mode 4), hidden when the recipe has none.
- **One-use.** `Craft_Recipe_OneUseOnly` is shown exactly for one-use recipes.
- **Experience line.** When the description carries an experience-award reference (a DID the
  panel tests for null; not identified, by inference the recipe's
  `CraftRecipe_XPAward_Progression`, which every recipe has), `Craft_Recipe_EarnedPointsText`
  shows token 0x068E3B74 with arguments 0x00058DA2 = the display text of the selected tier
  (mapper `CraftTier`) and 0x04967744 = the experience value. When a craft-experience
  multiplier applies to the player the original uses token 0x00B9A23C (a coloured variant) and
  multiplies the value (store acceleration; out of scope).
- **Cooldown.** With a cooldown duration above 0: `Craft_Recipe_CooldownDuration` is shown with
  token 0x0896F69E, argument 0x00058E15 = the duration formatted as a time span (whole seconds);
  otherwise hidden. `Craft_Recipe_CooldownTimer` shows the remaining time while the recipe's
  cooldown runs.
- **Outputs.** `Craft_MultiOutput_Field` is shown exactly for recipes with
  `CraftRecipe_MultiOutputArray`; its page control's page count is the number of output choices
  and its current page the chosen output (BIN; how the choices map to the array entries was not
  traced).

> **Tessera rule (outputs):** choice 0 is the recipe's own `CraftRecipe_ResultItem` /
> `CraftRecipe_CriticalResultItem`; choice *k* (1 … *m*) is the *k*-th `CraftRecipe_MultiOutputEntry`
> (its `CraftRecipe_ResultItem` and `CraftRecipe_CriticalResultItem`, the latter absent when 0).
> The page control has *m* + 1 pages; page *k* + 1 shows choice *k*; the request carries *k*
> (§5.6). Owner check O184.

> **Tessera rule (experience line):** always the plain token 0x068E3B74 with the value of
> EC §11.5.2; the line is hidden when the recipe's `CraftRecipe_XPReward` key has no entry.

> **OPEN:** (implementer, 2026-10-16) the time-span format of the cooldown line (and of the
> scroll row of §7) is not given beyond "whole seconds". `tessera-gamedata` uses the duration
> text of `item-tooltips.md` §6.18 (Tessera rule there: the largest two non-zero units, for
> example `1d 4h`).

<a id="s5-3"></a>
### 5.3 Requirements: tool and facility (BIN, BC; CP9, CP11)

- **Tool line** `Craft_Recipe_RequiredToolText`: token 0x07272CFC with argument 0x05B9DC5C =
  the tool name, the display text of the key whose bit is set in the profession's
  `CraftProfession_RequiredTool` (mapper `CraftToolCategory`, `EMAPPER` key 0x10000009 here,
  0x23000062; bit *i* is key *i* + 1, `property-system.md` §7.3), or `CraftSystem` token
  0x0041FE75 when no bit is set. State `normal` when the tool test passes, else
  `requirement_notmet`.
- **Facility line** `Craft_Recipe_RequiredFacilityText`: token 0x093916C9 with argument
  0x057E0A39 = the facility effect's name (by inference its `Name`; every facility effect has
  one), or `CraftSystem` token 0x0041FE75 when the recipe names none (354 recipes). State `normal`
  when the facility test passes, else `requirement_notmet`.

**Tests (BC).**

- Tool test: passes when the profession's `CraftProfession_RequiredTool` is 0, or when every bit
  of it is set in the player's `Inventory_CraftToolCategory`.
- Facility test: passes when the recipe has no `CraftRecipe_RequiredFacilityEffect`, or when one
  of the player's active effects has that DID.

The guild requirement (`CraftRecipe_RequiredCraftGuild`, 2,184 recipes) is not part of the
description and no element shows it (BC, BIN: no read found).

> **Tessera rule (inputs):** the tool bits are the player's `Inventory_CraftToolCategory` as the
> feed set it with `stat player @Inventory_CraftToolCategory=…` (SF §5.8, §6.1; a bitfield, labels
> of `CraftToolCategory`); none set means no tool. The player's active effects are those of the
> feed's `EffectApplied`/`EffectRemoved` (SF §6.2 `effect`). The client never derives the tool
> bits from equipped items.

<a id="s5-4"></a>
### 5.4 Ingredients and packs (BIN; CP9)

The page has six main-ingredient slots. Slot *i* (1 … 6) shows the *i*-th entry of
`CraftRecipe_IngredientList` (1–6 entries in this install, so every list fits); slots beyond the
list are hidden. A shown slot:

- `CraftIngredientItem` shows the ingredient item with the **needed** quantity
  (`CraftRecipe_IngredientQuantity`), and is drawn in its not-enough look (the item display's
  unusable flag) when held < needed;
- `Craft_Ingredient_Info` shows token 0x0E86F6CF with arguments 0x00484A34 = held,
  0x0059485C = needed, 0x00052615 = the ingredient's name.

**Held count.** The class library adds two inventory counts of the item (BC; which two
containers was not traced).

> **Tessera rule:** held = the summed quantity of items with that DID in the player's backpack
> and bags of the inventory model (P1), not counting the vault, shared storage or equipped
> items.

**Packs.** For a recipe with a pack (4,520): the check box `Craft_Recipe_UseIngredientPack` and
its label (token 0x064C67CB, argument 0x069C3CF4 = the pack's name) are shown, and the packs
label shows token 0x08775AC3 with the same argument; for other recipes both are hidden. While
the box is checked, `CraftRecipe_IngredientPacks` replaces `CraftRecipe_MainIngredients`, its
display shows the pack with held and needed (`CraftRecipe_IngredientPackQuantity`) as above,
and the quantity rules of §5.6 use the pack count instead of the makeable count. The binding
warning `Craft_IngredientPack_Binding` (token 0x060016A7) is part of that field.

> **Tessera rule:** the pack check box starts unchecked for each recipe shown.

> **OPEN:** (implementer, `crates/ui`, 2026-10-16) the packs label of
> `CraftRecipe_IngredientPacks` (token 0x08775AC3, §5.1 "label") has no element name.
> `tessera-ui` fills the first Text below the field that is neither
> `Craft_IngredientPack_Binding` nor part of `CraftRecipe_IngredientPack_Display`. The spec
> should name the element.

> **OPEN:** (implementer, `crates/ui`, 2026-10-16) "the item display's unusable flag" is
> not named as a property or state of `ItemDisplay_ShortTemplate`. `tessera-ui` switches
> `CraftIngredientItem` to state `ghosted` when it defines one (install layouts) and the
> fallback slot draws a red overlay; the spec should name the flag or state.

<a id="s5-5"></a>
### 5.5 Optional ingredients and the critical result (BIN; CP9)

Shown only for recipes with a critical result (`CraftRecipe_CriticalResultItem` non-zero;
4,461 recipes; every recipe with optional ingredients has one):

- `CraftRecipe_MasterPage` is shown; for other recipes the master page and the overlay are
  hidden.
- `CraftRecipe_MasterOverlay` is shown when the character has not mastered the selected tier
  (that is, the tier is above *b* of §4.7), hiding the optional slots behind it.
- **Optional slots.** For each `CraftRecipe_OptionalIngredientList` entry *j* (0, 1 or 2 in this
  install; at most 5 slots): the slot's ingredient part is filled as in §5.4; its bonus text is
  token 0x03E3EE73 with argument 0x04A67A64 = round(100 × `CraftRecipe_IngredientCritBonus`)
  (rounding as EC §11.5.1 step 4). Its check box is cleared and ghosted unless the slot's
  ingredient is held in the needed number and the tier is mastered; otherwise it is enabled
  (and leaves `ghosted` for `normal`). Unused slots are hidden and cleared.
- **Pager.** All slots share one rect; `Craft_Recipe_OptionalIngredient_PageCtrl` is shown when
  the recipe has optional ingredients, with one page per optional ingredient; the current page
  is clamped to the count.

  > **Tessera rule:** only the slot of the current page is visible (slot *k* on page *k*); the
  > others keep their check-box state. (The original's per-slot visibility argument was not
  > recovered.)

- **Critical result.** When it is an item: `Craft_Recipe_CriticalItem` shows it with
  `CraftRecipe_CriticalResultItemQuantity`, the name text and non-item image are hidden.
  Otherwise the critical name text shows the critical result's name and the non-item image
  shows `CraftRecipe_Field_CritResultIcon` (hidden when none).
- **Critical chance** `Craft_Recipe_CriticalChanceText`: token 0x0E36DFA5 with argument
  0x04A67A64 = the percentage of EC §11.5.1: recipe chance + the character's modifier + the
  bonus of every **checked** optional slot (visible or not), clamped to 0 … 1, × 100, rounded.

> **Tessera rule (modifier):** the character's modifier is the value of the float property
> named by the profession's `CraftProfession_CriticalChanceAddModifier_PropertyName` on the
> player as the feed set it with `stat` (SF §6.1); 0 when never set.

<a id="s5-6"></a>
### 5.6 Quantity, Make and Make All (BIN; CP13, CP14)

Let *M* be the makeable count (§4.5), or the pack count when the pack box is checked, and
*m*₀ = 1 when *M* > 0, else 0.

- **Shown quantity.** When a recipe is shown (and no craft runs) the entry is set to the value
  it reads (below), or to 1 when both that value and *M* are 0.
- **Reading the entry.** An empty entry reads as *m*₀; otherwise leading `0` characters are
  dropped and the rest is read as an unsigned decimal number (a text that does not start with
  a digit reads 0).
- **Typing.** After each change: a value above *M* is replaced by *M*; a value below *m*₀ by
  *m*₀. The entry takes at most 3 characters.
- **Arrows.** Increase: when the value is below *M*, add 1; above *M*, set *M*. Decrease: when
  the value is above *m*₀, subtract 1; below, set *m*₀.
- **Make** (`Craft_Recipe_ExecuteButton`): when Make is enabled (§5.7), send the craft request
  with the entry's value as quantity (nothing is sent for 0).
- **Make All** (`Craft_Recipe_MakeAllButton`): when Make is enabled, set the entry to
  min(*M*, 999) and send as Make does.

> **OPEN:** (implementer, `crates/ui`, 2026-10-16) the arrow rule does not say what happens
> at 999 when *M* is larger (pack counts reach 1,200, §15.3), while the entry takes at most
> 3 characters. `tessera-ui` steps the increase arrow with min(*M*, 999), as Make All caps,
> and never sets the entry to a number above 999.

**The request (CP14).** It carries the recipe DID, the selected tier, the chosen output
index (§5.2), a list with one flag per optional slot (1 = checked; the list is empty when no
slot is checked), the quantity and whether the pack box is checked. Sending marks the panel
busy with that quantity, puts both make buttons into state `ghosted` and, when the hide check
box (`Craft_HideOnExecuteButton`) is in its checked state, puts `Craft_Window` into state
`temp_hide` (it fades out, then hides).

> **Tessera rule (command):** Make sends `Command::Craft { recipe, quantity }` (SF §9.4) with
> the additive fields of §13 (`tier`, `output`, `optional`, `pack`). The panel stays busy until
> the feed has reported as many `CraftResult` events for that recipe as the quantity sent, or a
> cast of kind `craft` of the player ends other than `completed` (`CastEnded`), or the panel is
> closed. Ending busy restores the make buttons' states (§5.7) and returns a hidden panel to
> state `normal`. Owner check O183.

<a id="s5-7"></a>
### 5.7 When Make is enabled (BIN; CP12)

Make and Make All are enabled (state `normal`, else `ghosted`) exactly when:

1. no craft request of this panel is busy;
2. the tool test and the facility test pass (§5.3);
3. with the pack box unchecked, every main ingredient is held in the needed number; with it
   checked, the pack is;
4. every checked optional slot's ingredient is held in the needed number;
5. the player is in no channelling state, or the player's current channelling state record
   (`Channeling_CurrentChannelingDID`) does not have the `Crafting` bit (mask 0x8) of
   `Channeling_BarredActions` (IA §3.9.1).

The makeable count does not take part, so a cooldown recipe whose cooldown runs can still be
requested when its ingredients are held (the server refuses).

> **Tessera rule:** the current channelling state is the record of the player's active cast of
> kind `channel` (SF §6.2 `cast … kind=channel` with a channelling-record ref); none when the
> player has no such cast.

<a id="s5-8"></a>
### 5.8 Hiding and closing

What the original does when the panel is closed during a busy craft was not traced.
**Tessera rule:** closing sends nothing (the craft goes on) and ends the busy state of §5.6 for
the panel only.

<a id="s5-9"></a>
### 5.9 Inscription (out of scope)

After some crafts the original prompts the crafter to inscribe the item (a pending prompt
blocks the panel's input; the option `GOP_Craft_DisableInscription` turns the prompt off).
**Tessera rule:** no prompt in P6; the check box is drawn and toggles a local option only.

---

<a id="s6"></a>
## 6. The crafting bar and results

<a id="s6-1"></a>
### 6.1 In the original

A craft is an induction: each recipe names one with `CraftRecipe_InductionAction` (12 distinct
records: one 7-s induction per profession, used by most recipes, and three of 25, 45 and 60 s
used by one recipe each). The induction bar is the one of IA §3.8.4 (title `Induction_TitleString`,
interruptible flag; all 12 are interruptible, may not be used while moving and may be used in
combat). `CraftRecipe_ExecutionTime` equals the named induction's `Induction_Duration` in 6,732
recipes and differs in 1,059 (values 1–30 s; CP22); which of the two the server uses is not
known (EC §11.5.3 OPEN). The client receives the bar's start and end from the server. Result
messages are server text; the success, failure and interruption texts of `UI_Crafting` (tokens
0x086FE5D3, 0x0EAA4585, 0x0092F604) are not referenced by the native code read (BIN negative
search, all 187 tokens).

<a id="s6-2"></a>
### 6.2 Tessera presentation (Tessera rules)

- **Bar.** `cast player REF DURATION kind=craft` (SF §6.6) shows the cast bar of CU §7 with
  kind `Craft`. When REF is a recipe, the bar's label is the `Induction_TitleString` of the
  recipe's `CraftRecipe_InductionAction`, else the recipe name; interruptible = that
  induction's `Induction_Interruptable`. Other refs follow CU TR25.
- **Result line.** On `CraftResult { recipe, outcome, item }` the chat window posts one system
  line: for `Success`, `UI_Crafting` token 0x086FE5D3; for `Failed`, token 0x0EAA4585; for
  `Critical`, Tessera's own short text "Critical success" (Tessera wording, owner check O185);
  each followed by the item's name and, when its quantity is above 1, its quantity, when
  `item` is given, else by the recipe name.
- **Panel.** A result refreshes the counts (the feed's item events change holdings) and counts
  toward the busy state (§5.6). A one-use recipe stays listed until the feed forgets it (§13).
- **Experience.** Meters change only through `ProfessionChanged`.

---

<a id="s7"></a>
## 7. Recipe and recipe-scroll tooltips (OBS, CP21)

Recipe scrolls (6,055 items of class 805 here) use `Recipe_ExamineLayout` of layout
`examination`: its auto-resizing list holds the rows `NameIcon_Row_Base`, `ItemBinding_Base`,
`UI_WebStore_Base`, `ItemUniqueness_Base`, `Indestructibility_Base`, `QuestID_Base`,
`RecipeOneShot_Base`, `RecipeCooldown_Base`, a separator, `UsageReqsDisplay_Base`, a
separator, `Description_Base`, a separator, `Value_Base`, a separator, `QuestGuide_Base`
(IT §4 for the generic rows). Their compare type is 4 (`Recipe`): the result item's tooltip is
attached (IT §8.3).

The two recipe rows (both 146 × 18, font 0x42000001, colour 0xFF2AB5B8, outlined, fit to text):

| Row | Widget type | Text property | Text |
|---|---|---|---|
| `RecipeOneShot_Base` | `ExaminationRecipeOneShot` | `UI_Examination_Tooltip_RecipeOneShotText` | `Examination` token 0x0005C674 |
| `RecipeCooldown_Base` | `ExaminationRecipeCooldown` | `UI_Examination_Tooltip_RecipeCooldownText` | `Examination` token 0x02B7199E, argument 0x00058E15 (a duration) |

> **Tessera rule (replaces the empty rows of IT §6.20 for these two):** for an item with
> `RecipeItem_Recipe`, `RecipeOneShot_Base` shows its text when that recipe is one-use, and
> `RecipeCooldown_Base` shows its text with the recipe's cooldown duration (EC §11.5.3, formatted
> as in §5.2) when it has one; otherwise each row is empty and removed (IT §5). (The widgets'
> own code was not traced; owner check O187.) The requirement rows (`UsageReqsDisplay_Base`)
> show the scroll's `Usage_RequiredCraftProfession`, `Usage_RequiredCraftTier` and (149 scrolls)
> `Usage_RequiredCraftMastery` per IT.

Items carrying `Effect_Crafting_RecipeList` (71 items of class 3663 here) teach several recipes;
their tooltips are IT's and get no recipe rows.

---

<a id="s8"></a>
## 8. Crafting trainers (NPC actions)

<a id="s8-1"></a>
### 8.1 The choice-window entries (OBS, CP16)

`ActionPickerUI_ActionEntryArray` of `RootActionPickerField` (NW §5.3) has three crafting
entries. Each has an `ActionPickerUI_ActionDataIDTemplateArray`: one row template per record,
keyed by `ActionPickerUI_ActionDataID`:

| Action (`NPCActionPickerType` key) | Templates | Pass-through property | Script property |
|---|---|---|---|
| `ProfessionTrainer` (38) | 10, one per profession (`ActionPicker…ProfessionTemplate`, based on `ActionPickerProfessionTemplate`, 375 × 50, an icon per profession) | `CraftUI_ProfessionDID` | `NPC_ActionPicker_ProfessionTrainerScript` |
| `VocationTrainer` (5) | 7, one per vocation (`ActionPicker…VocationTemplate`, based on `ActionPickerVocationTemplate`, 375 × 50, with the description line `ActionPickerVocationTemplateDescriptionText` at (40, 32)) | `CraftUI_VocationDID` | `NPC_ActionPicker_VocationTrainerScript` |
| `CraftGuildTrainer` (24) | 7, one per guild (`ActionPicker…CraftGuildTemplate`, based on `ActionPickerMiscTemplate`, 375 × 30) | `CraftUI_CraftGuildDID` | `NPC_ActionPicker_CraftGuildTrainerScript` |

The 24 data DIDs are exactly the 10 professions, 7 vocations and 7 guilds of EC §5.4. The
templates' own text entries are placeholders (a design string of table 0x25033E35 and one fixed
description of `UI_NPC_ActionPicker`); the original fills the rows from the records at run time
(not traced).

> **Tessera rule (rows):** a crafting-trainer option (§13, `Text::ActionFor`) uses the template
> whose `ActionPickerUI_ActionDataID` is the option's record; its text is the profession's
> `CraftProfession_Name`, the vocation's `CraftVocation_Name` (with the description line = the
> names of its three `CraftVocation_ProfessionArray` professions joined by ", ") or the name of
> the guild's `CraftGuild_Faction` record. Choosing the row sends `ChooseOption` (NW §5.3).

<a id="s8-2"></a>
### 8.2 Trainer NPCs (OBS, CP17)

32 NPC templates (class 1724 here) carry crafting offers: 21 have
`Craft_OfferedProfessionArray` with all 10 professions, 10 have `Craft_OfferedCraftGuildArray`
with all 7 guilds, 1 has `Craft_OfferedVocationArray` with all 7 vocations. No NPC has two kinds.

<a id="s8-3"></a>
### 8.3 Trainer windows (BIN, BC; CP15)

All three share one shape (rects relative to the root, 349 × 375): background `Vendor_BaseBox_BG`
(`Box_Silver`, (0, 16) 345 × 357), title bar (`Base_Box_Titlebar`, (47, −11) 250 × 42) with the
window label (token 0x0AC90BEC of table 0x250001BB; the guild window token 0x02BD861C), close
button (323, 22), drag bar (15, 2) 306 × 24, the **name text** `…_NameText` (`fieldvaluelarge`,
(18, 37) 301 × 35, colour 0xFFECFF00, centred, outlined), the **display text** `…_DisplayText`
(`NarrativeFont`, (12, 71) 317 × 252, 198 high in the profession trainer, top-aligned, scroll bar
`Scrollbar_Trainer` (329, 68) 10 × 252), the train button `…_TrainButton` ((51, 337) 158 × 20)
and the cancel button `…_CancelButton` ((222, 338) 79 × 20). The profession trainer adds
`ProfessionTrainer_LoseProfessionLabel` (Text, (55, 281) 239 × 25, token 0x0158CCB5) and the
menu `ProfessionTrainer_ProfessionSelector` (`Menu_01`, (53, 302) 239 × 25).

When shown for (NPC entity, record):

1. The name text shows `UI_Crafting` token 0x0274D145 (guild window: 0x08168815) with argument
   0x00005343 = the NPC's `Name`.
2. The display text shows, from the class library:
   - vocation trainer: the vocation's `CraftVocation_ChangeString` when the player already has a
     vocation, else its `CraftVocation_LearnString`;
   - profession trainer: the profession's `CraftProfession_LearnString` or
     `CraftProfession_ChangeString` (chosen by a test on the player's `Craft_ProfessionArray`),
     with argument 0x06AD2BBE = the profession's `CraftProfession_Description`;
   - guild trainer: a text of the guild (not traced further; the guild record's only text is
     `CraftGuild_LearnString`).
3. Profession trainer only: when the player's professions fill all profession slots, the label
   and the selector are shown, the selector lists the player's professions by name (first
   entry selected); otherwise both are hidden.

Buttons: cancel and close hide the window. Train: the vocation trainer asks for confirmation
first when the player already has a vocation (a confirmation box; not traced further), else
sends a train request with (NPC, vocation); the guild trainer sends (NPC, guild) and hides; the
profession trainer sends a request with the selected profession to replace when the selector
is shown, else a plain request.

> **Tessera rule (from the feed):** the windows open only on the feed's `CraftTrainerShown`
> (§13) and close on its `WindowClosed` or the buttons. The vocation text uses ChangeString when
> the feed has reported a vocation (§13 `vocation`), else LearnString. The profession text uses
> ChangeString when the selector is shown, else LearnString. The guild text is
> `CraftGuild_LearnString`. The selector is shown when the feed's slot count is known and the
> player has that many professions. Train sends `TrainCraft { window, replace }` (§13); a
> vocation train with a vocation already reported first shows a two-button confirmation using
> `UI_Crafting` token 0x07CA397E and the train and cancel labels (Tessera's choice of text;
> owner check O189).

> **OPEN:** (implementer, `crates/ui`, 2026-10-16) two facts the windows need: (1) how
> the replace selector `ProfessionTrainer_ProfessionSelector` (`Menu_01`) is driven (its
> popup items and the selection it reports); `tessera-ui` moves the selection to the next
> profession on each click and shows its name (Tessera rule). (2) Where Tessera's
> confirmation sits: the install's trainer layouts have no element for it, so `tessera-ui`
> adds a code-made 289 × 130 field at (30, 110) of the root with the text and the two
> buttons. The labels `…_NameText`, `…_DisplayText`, `…_TrainButton` and `…_CancelButton`
> resolve with the root's prefix (`VocationTrainer_`, `ProfessionTrainer_`,
> `CraftGuildTrainer_`) on the owner's install; the spec could spell them out. (3) The close
> button at (323, 22) has no label in this section; `tessera-ui` reads `<prefix>_CloseButton`
> (`VocationTrainer_CloseButton`, `ProfessionTrainer_CloseButton`,
> `CraftGuildTrainer_CloseButton`), which resolves to that button in all three layouts on the
> owner's install (2026-10-16), with Tessera's own close button as the fallback.

---

<a id="s9"></a>
## 9. Gathering nodes and crafting facilities in the world

<a id="s9-1"></a>
### 9.1 Gathering nodes (OBS, CP18)

Of the 1,720 container templates (class 794 here; `WeenieType` `GameplayContainer`) that carry
`Craft_Resource_Type`, **335** are gathering nodes: they hold `Usage_RequiredCraftProfession`,
`Usage_RequiredCraftTool`, `Usage_RequiredCraftTier`, `Usage_UserBehaviorName`
(`Emote_Use_Container`) and `Container_OnOpen_XPAward_Progression`. The other 1,385 are chests
(`Craft_Resource_Type` 1 `Treasure` or 0).

| `Craft_Resource_Type` (mapper 0x23000124) | Profession | Tool bit (key) | Induction | Nodes |
|---|---|---|---|---|
| 4 `Crop` | 0x70003920 | 0x20 (`FarmingTools` 6) | 0x700039B4 | 183 |
| 3 `Wood` | 0x70003921 | 0x200 (`ForestersAxe` 10) | 0x70003955 | 54 |
| 2 `Mine` | 0x70003922 | 0x8 (`MiningPick` 4) | 0x7000721F | 53 |
| 5 `Ruin` | 0x70001A62 | 0x2 (`ScholarsGlass` 2) | 0x70001A69 | 37 |
| 5 `Ruin` | 0x70003921 | 0x200 | 0x70003955 | 4 |
| 5 `Ruin` | 0x70003922 | 0x8 | 0x7000721F | 4 |

Every node also has: `Usage_RequiredCraftTier` 1–16 (every tier occurs for each of the four
professions), `Usage_MinUsageDist` 3.5, `Usage_Action` = `UsageAction_Container` (mouse-over
type 16 `Container_Open`), `CSM_ActionHandler` the chest menu handler, `IsSelectable` true,
`Selection_Script` `ui_selected_fx`, `Usage_ReserveUntilUseCompletes` and
`Usage_BarShapeshiftedCharacters` true, `Usage_AllowedWhileMounted` true in 151,
`Entity_CreationType` 8 (created only by a server; placed through generator profiles,
`GeneratorProfile_WSLEntity`, 1,298 references), `Name` and `Description`, a `PhysObj` (76
distinct 0x47 descriptors, all present; 173 nodes share one), `Render_AdjustableScale` 0.5 in
180 and 0.6 in 1, and loot tables (EC §4.13). `Container_OnOpen_XPAward_Progression` is one
float progression (150 entries, 20 rising by 5) driven by `Usage_RequiredCraftTier`
(`…_DrivingProperty`); what the server credits is server state.

The four gathering inductions: `Induction_Duration` 7 s, interruptible, not while moving,
allowed in combat, the plain progress bar element; their titles are of four different tables.

**What the client shows (CMP).** The node is drawn from its `PhysObj` like any entity
(`entity-descriptor.md`), with its scale. Hover: the cursor and out-of-range cursor of
mouse-over type 16 (IA §3.3, §3.4.2, §3.4.6), with `Mouseover_Range` = 3.5 (IA §3.4.2); the
container rule of IA §3.4.5 applies (nodes store no `Loot_Permissions_Rule`). Use: the input of
NW §3.1 and the range test of IA §3.4.1 with `Usage_MinUsageDist` 3.5. The client evaluates
none of the requirement properties.

> **Tessera rule (gathering from the feed):** a node appears as a feed actor whose body is the
> node template; using it sends `UseEntity` (NW §3.1). A scenario answers with `cast player
> INDUCTION 7s kind=induction` (the node's `Induction_ActionDID`), then a loot window (`loot`,
> NW §8) with the gathered items, `profession` for the experience, and `despawn` of the node;
> a refusal is a chat line. The bar's label is the induction's title (CU TR25). The client adds
> nothing of its own.

<a id="s9-2"></a>
### 9.2 Crafting facilities (OBS, CP19)

13 effect records are facilities, all named by recipes' `CraftRecipe_RequiredFacilityEffect`
(7,437 recipes; per facility 2,954, 1,953, 850, 667, 453, 208, 207, 50, 45, 34, 14, 1, 1); every
one has a `Name`. They are applied by:

- 26 hotspot templates (class 1170; IA §4) whose `EffectGenerator_HotspotEffectList` names a
  facility effect (42 elements): `Entity_CreationType` 8, `Usage_MinUsageDist` 2; 15 are
  selectable and carry map-note type bit 44 (IA §4.4), 11 are not; three `PhysObj` descriptors;
- 61 general items (class 796; 136 effect elements) and one item of class 802 (11 elements)
  whose effect lists name one (portable or decoration facilities; `items.md`).

The client shows a facility only through the recipe page's facility line (§5.3). A facility
hotspot is drawn as IA §4.6 says.

> **Tessera rule:** standing at a facility is the player holding the facility effect: a scenario
> sends `effect player FACILITY` when the player is near a facility actor and `effect-end` when
> leaving; the client does not test areas.

---

<a id="s10"></a>
## 10. Census: what the panels can show

The census runs offline over the install with no feed and puts every record in exactly one
category per family; categories other than `Shown` are reasons, not failures. Counts are this
install's (2026-10-16).

<a id="s10-1"></a>
### 10.1 Recipes (7,791 records)

**Record set.** Every object record whose root class is the recipe class (class 1024 here),
found without class indices by its characteristic property `CraftRecipe_Profession` (EC §3.4);
every one has a property set.

**Procedure.** For each recipe, build what the panel would show for it learned at each of its
tiers: the tree rows of §4.4 (tier text, category text, recipe name, template), the recipe page
of §5 (result display or image, ingredient slots with names, optional slots, critical display,
outputs, tool and facility lines, cooldown line, experience line), and the scroll rows of §7
when a scroll names it. Then take the first category that applies:

| Category | Rule | Count |
|---|---|---|
| `ProfessionMissing` | `CraftRecipe_Profession` does not name a profession record | 0 |
| `TierTextMissing` | a tier has no `CraftTier` display text | 0 |
| `CategoryTextMissing` | the category has no `CraftUICategory` display text | 0 |
| `NameMissing` | the recipe name is empty | 0 |
| `IngredientMissing` | an ingredient, pack, optional ingredient or output names no item with a `Name` | 0 |
| `TooManySlots` | more than 6 main or 5 optional ingredients | 0 |
| `ResultImageMissing` | the result (or critical result) is not an item and the recipe has no field icon for it | 9 (all farmer recipes; the same 9 for both) |
| `Shown` | everything above resolved | 7,782 |

> **OPEN:** (implementer, P6 review fix, 2026-10-16) two Tessera categories the table lacks:
> `Unreadable`, first in order, for a recipe record that does not decode as a recipe (the
> other families of §10.2 have one; 0 on the owner's install), and `TooManyTiers`, before
> `TooManySlots`, for a recipe with more than 64 distinct tiers (a Tessera bound on the
> work per recipe; 0 here, the install has 16 tiers in all). `tessera-gamedata` also drops
> repeated tier keys of one recipe, so a recipe has one row per distinct tier (CI2). The census
> builds the output pages of the first 64 output choices only (the feed's `craft` request
> names output 0 … 64), once per recipe; every output's name is still checked by
> `IngredientMissing`. The spec writer should confirm or replace these.

Further counts (not categories): 3 recipes are named by no other record (EC §5.5); 79 have a
non-item result (70 with a field icon); 1,339 one-use, 2,262 cooldown, 2,414 multiple-output,
4,461 with a critical result, 4,012 with optional ingredients, 4,520 with a pack, 2,184 with a
guild requirement. Per profession: 1,417; 1,306; 1,294; 935; 857; 731; 644; 231; 199; 177.
Per tier (1–16): 530, 399, 495, 523, 562, 1,091, 692, 701, 921, 256, 269, 272, 355, 194, 349,
182. 491 distinct categories (100 of them used by more than one profession).

<a id="s10-2"></a>
### 10.2 Professions, vocations, guilds, trainers

10 professions × 16 tier entries (every tier has a `CraftTier` display text; every profession
has a name), 7 vocations (3 professions each), 7 guilds (each naming a profession and a faction
with a name), 32 trainer NPCs (§8.2): all `Shown`.

<a id="s10-3"></a>
### 10.3 Gathering nodes and facilities

335 gathering nodes (all with name, descriptor, induction and the four requirement properties:
`Shown`); 13 facility effects with names (`Shown`); 26 facility hotspots (`Shown` as IA §4.6).

> **OPEN:** (implementer, 2026-10-16) which four requirement properties §10.3 means. §9.1 lists
> five properties every node holds (`Usage_RequiredCraftProfession`, `Usage_RequiredCraftTool`,
> `Usage_RequiredCraftTier`, `Usage_UserBehaviorName`, `Container_OnOpen_XPAward_Progression`).
> `tessera-gamedata` takes a node as `Shown` when it has a rendering `Name`, a `PhysObj`, an
> `Induction_ActionDID` and the profession, tool and tier requirements, and gives the reasons
> `NameMissing`, `DescriptorMissing`, `InductionMissing`, `RequirementMissing` otherwise (all
> 335 `Shown` on the owner's install). The node set is the creature-family container
> templates (`creatures.md` §2.2) that hold `Usage_RequiredCraftProfession`.

<a id="s10-4"></a>
### 10.4 Recipe scrolls

6,055 scrolls, each naming a recipe (CP21): `Shown`.

> **Tessera rule:** the real-data test of criterion 1 runs this census, asserts that every
> record lands in a category, and reports the counts; a count that differs from this table is a
> warning (data change), not a failure.

---

<a id="s11"></a>
## 11. Tessera fallback windows (Tessera rules)

Without an install, or when a layout or element of §3 is missing, Tessera builds the windows
from fixed widgets in the style of decision 0031's fallbacks:

- **Panel:** an 800 × 580 window titled from Tessera's own text, a tab row with one tab per
  profession (profession ref text when no name resolves), a tree on the left (tier rows "Tier
  *n*", category rows by number, recipe rows by ref) and on the right a page with a name line,
  six ingredient lines "held/needed name", an optional-ingredient line with a check box, the
  critical chance, the tool and facility lines, a quantity field with arrows and two buttons.
  Colours: craftable rows green, `requirement_notmet` lines red, ghosted buttons grey.
- **Trainer:** a 349 × 375 window with the NPC name, the display text (or the ref), an optional
  selector, train and cancel.
- The rules of §4–§8 apply unchanged; only the drawing differs. Missing images draw as empty
  boxes.

Tier-1 goldens (committed, synthetic, decision 0031) use these fallbacks with the scenario of
§15.4; tier-2 goldens (local, real data) use the install's layouts.

---

<a id="s12"></a>
## 12. Keys (OBS, CP2)

| Input action | Default (map `Main`) | Effect |
|---|---|---|
| `ToggleCraftingPanel` (0x10000040 here) | keyboard T, no modifier, activation 0x03 | open or close the panel (§4.1) |
| `ToggleCraftingProf1Panel`, `…Prof2Panel`, `…Prof3Panel` | none | open the panel at tab 1, 2, 3 (Tessera rule, §4.1) |

> **Was OPEN (for `keymap.md`, owned by the progression spec writer in P6):** add these four rows to
> the keymap defaults (`ToggleCraftingPanel` on T) and check for a clash with walker keys
> (keymap.md §8.2 notes that earlier defaults took walker keys). This spec does not change
> `keymap.md`.
>
> **Answered (spec writer, 2026-10-23):** `keymap.md` §8.3 (revision of 2026-10-23,
> Accepted) carries the four rows: `toggle_crafting` (`ToggleCraftingPanel`, T) and
> `toggle_crafting_1` … `toggle_crafting_3` (unbound), all in the `global` context; the walker
> keys were moved off the keymap defaults by the owner's decision of 2026-10-16.

---

<a id="s13"></a>
## 13. Scenario-feed forms (proposal for the revision labelled "P6 progression")

The existing forms (SF §6.6, §6.2, §6.1) cover most of the panel: `profession` /
`ProfessionChanged`, `recipe` / `RecipeLearned`, `craft-result` / `CraftResult`, `cast …
kind=craft`, `cooldown` (recipe cooldowns), `effect` (facilities), `stat` (tool bits, critical
modifier), item events (holdings), `Command::Craft { recipe, quantity }`. The additions below
carry values the client must not derive (decision 0024 §2 principle 4); all are optional and
additive, so scenarios and logs without them stay valid. The `scenario-feed.md` writer
reconciles names and the revision number at merge.

| Form | Addition | Default | Used by |
|---|---|---|---|
| `profession` step, `ProfessionChanged` | `mastery-tier=N` → `mastery_tier: Option<u16>` | none (= `tier`) | §4.7 |
| `forget-recipe REF` step, `RecipeForgotten { what }` (new) | removes a learned recipe (one-use recipes after use) | — | §4.4, §6.2 |
| `vocation (REF\|none) [slots=N]` step, `VocationChanged { vocation: Option<Ref>, slots: Option<u8> }` (new) | the player's vocation and profession-slot count | none | §4.3, §8.3 |
| `craft-trainer (profession\|vocation\|guild) REF [npc=ENTITY]` step, `CraftTrainerShown { window, kind, what, npc }` (new window kind `craft-trainer`) | opens a trainer window | — | §8.3 |
| `TrainCraft { window, replace: Option<Ref> }` command (new), input form `train-craft [replace=REF]` | the train button | — | §8.3 |
| `Command::Craft` | `tier: Option<u16>`, `output: u16` (0 = own result), `optional: u8` (bit *j* = slot *j* checked), `pack: bool`; input form `craft REF [qty=Q] [tier=N] [output=K] [optional=MASK] [pack]` | none, 0, 0, false | §5.6 |
| choice-window option text | `Text::ActionFor { action, what: Ref }` (an action entry with a data record), form `action=ACTION:REF` | — | §8.1 |

Protocol: none of these needs a protocol minor; the test server never sends them (decision
0024, client half only).

---

<a id="s14"></a>
## 14. Invariants

| # | Invariant |
|---|---|
| CI1 | A recipe row exists only for a learned recipe of a profession the player has (§4.4). |
| CI2 | Each tier has at most one tier row per page, each (tier, category) at most one category row; a recipe has one row per tier it belongs to. |
| CI3 | The makeable count is 0 when any main ingredient is held below its quantity, and at most 1 for one-use and cooldown recipes (§4.5). |
| CI4 | A category or tier row is in `RecipeGroup_Active` exactly when at least one recipe row below it is craftable. |
| CI5 | Make is never enabled while a craft request of the panel is busy (§5.7). |
| CI6 | The quantity sent is 1 … min(*M*, 999) for Make All and 1 … *M* for Make; nothing is sent for 0 (§5.6). |
| CI7 | The shown critical chance is a whole number 0 … 100 (§5.5). |
| CI8 | The client sends no craft, train or gather request the user did not trigger, and derives no experience, tier, cooldown, recipe or outcome (§0). |
| CI9 | Every record of §10 lands in exactly one category. |

> **OPEN:** (implementer, 2026-10-16) CI6 and §5.7 disagree for a cooldown recipe whose
> cooldown runs: *M* = 0 (§4.5), the entry shows 1 (§5.6 "Shown quantity"), and Make is
> enabled when the ingredients are held (§5.7, "the makeable count does not take part"), so
> Make sends 1 > *M*. `tessera-gamedata` follows §5.6 and §5.7 (Make sends the entry's value,
> 1 … 999; Make All sends min(*M*, 999), nothing for 0); CI6 would need "1 … *M* for Make
> unless the cooldown runs". A second case (review fix, 2026-10-16): *M* drops after the value
> was typed (ingredients traded away while one craft's worth is still held, so Make stays
> enabled), and §5.6 clamps only on typing and on the arrows, so the entry can hold more
> than *M*. `tessera-gamedata` sends min(entry, *M*) whenever *M* > 0 (CI6) and the entry's
> value only when *M* = 0; the shown entry is not changed (§5.6 "Shown quantity"). The spec
> should say whether the entry is re-clamped when *M* changes.

---

<a id="s15"></a>
## 15. Worked synthetic examples

All values are synthetic.

<a id="s15-1"></a>
### 15.1 Makeable count

Recipe R: ingredients A ×2, B ×5; not one-use, no cooldown. Held A 7, B 11: 7 ÷ 2 = 3, 11 ÷ 5 = 2
→ *n* = 2; row text "name (2)" form (token 0x09B25265), state `RecipeGroup_Active`. Held B 4 →
*n* = 0, text name only, state default. Same holdings, R one-use → *n* = 1. R with a running
cooldown → *n* = 0 even with ingredients held.

<a id="s15-2"></a>
### 15.2 Tier rows

Feed: `profession P tier=3 proficiency=120/400 mastery=60/800 mastery-tier=2`. *a* = 2 (120 <
400), *b* = 1. Rows: tier 1: proficiency icon, mastery icon; tier 2: proficiency icon, mastery
meter 60/800 (position 0.075); tier 3: proficiency meter 120/400 (position 0.3); tier 4 (if any
recipe): nothing.

<a id="s15-3"></a>
### 15.3 Quantity

*M* = 4. Entry empty → reads 1. Type "9" → 4. Type "007" → reads 7 → 4. Decrease from 1 → stays
1. *M* = 0: entry shows 1 (rule of §5.6, shown quantity), Make is disabled by §5.7 rule 3.
Make All with *M* = 1,200 (pack mode) → 999.

<a id="s15-4"></a>
### 15.4 Progression scenario leg (sketch for the P6 scenario)

In words (exact step spelling per SF): declare a profession ref *P*, a recipe ref *R* of *P*
at tier 1 with one ingredient *A* ×2, and two result items. At 0 s: `profession P tier=1
proficiency=0/200 mastery=0/400`, `recipe R`, `stat player
@Inventory_CraftToolCategory=<the profession's tool label>`, give four *A*. Input at 1 s:
`craft R qty=2`. A trigger `on craft R` answers: `cast player R 7s kind=craft`; at +7 s
`craft-result R success item=…` and a second cast; at +14 s `craft-result R critical item=…`,
`remove` the used *A* items and `profession P tier=1 proficiency=40/200 mastery=0/400`.

Expected: the panel lists *R* under tier 1 and its category with count 2 and the
`RecipeGroup_Active` state; Make sends quantity 2; two bars labelled by *R*'s induction title;
two result lines (success, critical); the busy state ends after the second result; the
proficiency meter shows 40/200 at position 0.2; *R*'s count drops to 0 and the active states
clear.

> **Was OPEN:** (implementer, 2026-10-16) a literal recipe has no ingredients or results in the
> feed: `recipe` carries only `profession=`, `tier=` and `category=` (SF §6.6), so with
> `--no-data` the client cannot know that *R* needs *A* ×2 and the expected count 2 cannot
> follow from the feed. `tessera-gamedata` lets the caller register a literal recipe
> definition (ingredients, pack, optional ingredients, results, one-use, cooldown) under the
> literal's text (`CraftingPresenter::define`); an undefined literal is listed by its ref with
> no ingredients (makeable count 1, Tessera bound). Either the feed gains a literal-recipe
> form or this example names where the definition comes from (for example a fixture of the
> client's in-process tests).
>
> **Answered (spec writer, 2026-10-23), Tessera rule:** the definition of a literal recipe
> (ingredients, pack, optional ingredients, results, one-use flag, cooldown) comes from the
> fixture of the in-process tests and goldens that run this example; the P6 feed gains no
> literal-recipe form. An undefined literal is listed by its ref with no ingredients and a
> makeable count of 1, as built. With the install present, the example's *R* can be a real
> recipe record, and then every value comes from it (§4–§5).

<a id="s15-5"></a>
### 15.5 Critical chance

Recipe chance 0.05, modifier 0.02, optional slots 0.25 (checked) and 0.35 (not checked):
0.05 + 0.02 + 0.25 = 0.32 → 32 %. Both checked: 0.67 → 67 %. A bonus of 1.0 checked → clamped
1.0 → 100 %.

---

<a id="s16"></a>
## 16. Open questions (summary)

1. §4.3: the order of the player's profession list in the original (server).
2. §4.4: the tree's sort routine and when it runs; initial expansion (O186).
3. §5.2: how multiple-output choices map to `CraftRecipe_MultiOutputArray` (Tessera rule; O184);
   the helper that picks the name colour state.
4. §5.4: which two containers the class library counts for held items.
5. §5.5: the per-slot visibility of the optional slots (Tessera rule).
6. §5.6: when the original ends the busy state and restores a hidden panel (Tessera rule; O183).
7. §6.1: whether the server times a craft by `CraftRecipe_ExecutionTime` or by the induction
   (EC §11.5.3); the result lines (server text; O185).
8. §7: the recipe rows' own widget code (Tessera rule; O187).
9. §8.1, §8.3: how the original fills the trainer rows and the guild trainer text; the vocation
   confirmation (O188, O189).
10. §12: keymap rows (answered 2026-10-23 by `keymap.md` §8.3).
11. §10.3: which four requirement properties a gathering node needs (implementer).
12. §14 CI6 against §5.7 for a running cooldown (implementer).
13. §15.4: where a literal recipe's ingredients and results come from (implementer; answered 2026-10-23).
14. §5.2: the cooldown time-span format (implementer).
15. §5.4: the item display's unusable flag (implementer, `crates/ui`).
16. §8.3: the replace selector's menu, the place of Tessera's confirmation, the elided trainer labels and the close button's label (implementer, `crates/ui`).
17. §4.3 rule 4: the "train more" tab's label or image (implementer, `crates/ui`).
18. §5.4: the element of the packs label (implementer, `crates/ui`).
19. §5.6: the increase arrow at 999 with *M* above 999 (implementer, `crates/ui`).

---

<a id="s17"></a>
## 17. Proposed owner observations

Numbers O180–O191 are chosen to stay clear of the progression spec's rows (from O160); they are
reconciled at merge.

| Row | Observation |
|---|---|
| O180 | Press T: the crafting panel opens; screenshot with each profession tab; note the tab order against the vocation's profession order. |
| O181 | In one profession page: are tiers listed lowest first or highest first? Are categories alphabetical? Are recipes within a category in a fixed order (note three neighbours)? |
| O182 | Note a recipe whose row shows a count and one without; does the count equal what the held ingredients allow? Do tier and category rows turn green with a craftable recipe below? |
| O183 | Craft 3 of a recipe with "hide when crafting" checked: when does the panel reappear (after each craft, after the last)? Can Make be pressed during the batch? |
| O184 | A multiple-output recipe: how many pages does the selector show, and is the first page the recipe's own result? |
| O185 | The chat lines after a success, a critical success and an interrupted craft (screenshot). |
| O186 | When the panel opens fresh: which tiers and categories are expanded? |
| O187 | Hover a one-use recipe scroll and a cooldown recipe scroll: screenshot the tooltip rows. |
| O188 | Talk to a profession trainer and a guild trainer: screenshot the choice rows and the trainer window texts. |
| O189 | At a vocation trainer with a vocation already chosen: the confirmation after Train (screenshot; do not confirm). |
| O190 | Gather a node: the cursor at and out of range, the bar's label and length, and what opens afterwards (loot window or direct to bags). |
| O191 | Stand next to a forge and away from it with a recipe needing it: the facility line's colour in both cases. |

---

<a id="s18"></a>
## 18. Notes for other specs

- `economy.md` §15: the recipe name rule of the object form (CP20), the presentation counts of
  §10.1, the class library's description getters (CP10) and the execution-time comparison
  (CP22) (note added there; no fact changed).
- `interaction.md`: the panel's use of `Channeling_BarredActions` bit `Crafting` (§5.7), the 335
  gathering nodes among the excluded class 794 (§9.1) and the 26 facility hotspots (§9.2) (note
  added there).
- `scenario-feed.md`: the additive forms of §13 (revision labelled "P6 progression").
- `combat-ui.md` TR25: the label rule for a craft cast with a recipe ref (§6.2).
- `item-tooltips.md` §6.20: the two recipe rows get the Tessera rule of §7.
- `npc-windows.md` §5.3: the crafting-trainer rows of §8.1 replace the generic row for actions
  38, 5 and 24.
- `keymap.md`: the rows of §12 (owned by the progression spec writer in P6).
- `lua-plugin-api.md` §15.6: the plugin `ProfessionInfo` and `Recipe` views, when specified,
  read the same feed state as §4 (professions, tiers, experience, learned recipes).
- `docs/checklists/owner-observations.md`: rows O180–O191 of §17 (to be added at merge).

---

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-16 | First version (spec writer, dirty room; status Review). OBS census over all crafting records, layouts and the crafting string table; BC reads of the recipe description and trainer texts; BIN (decision 0011) questions c1–c5 on the crafting panel and trainer windows. |
| 2026-10-16 | Implementer OPENs from `crates/gamedata` (no fact changed): §5.2 cooldown time-span format, §10.3 node requirement properties, §14 CI6 against §5.7, §15.4 literal recipe definitions. The census of §10 reproduced every count of §10.1–§10.4 on the owner's install, and the §7 scroll rows (1,339 one-use, 2,238 cooldown) CP21. |
| 2026-10-16 | Implementer OPENs from `crates/ui` (no fact changed): §5.4 the item display's unusable flag; §8.3 the replace selector, the confirmation's place, the elided trainer labels (all resolve on the owner's install). |
| 2026-10-16 | Implementer OPENs from the `crates/ui` review fixes (no fact changed): §4.3 the "train more" tab's label, §5.4 the packs label's element, §5.6 the increase arrow at 999, §8.3 the close button's label. |
| 2026-10-16 | Independent review passed; status Review → Accepted (spec reviewer; record in the header). CLEAN_ROOM.md §5.3 met on every item; decision 0024 §2/§4 and `server-content.md` §2 held; BIN client-binary SHA-256 and the companion-spec cross-references spot-checked (M1). No fact changed. Two non-blocking notes carried for a later revision (CI6 vs §5.7 wording; the §10.1 `Unreadable`/`TooManyTiers` additions), neither altering the code on branch `p6`. |
| 2026-10-23 | Spec writer (dirty room); status Accepted → Revised, awaiting an independent review. **§3:** the `--open-panel` rule now shows the session's current state (empty without one; `crafting-recipe` selects the first row; a trainer opened by name sends nothing), and §15.4's content belongs to the in-process fixture (implementer OPEN answered). **§12:** the keymap rows exist in `keymap.md` §8.3 (OPEN answered). **§15.4:** literal-recipe definitions come from the test fixture; no feed form (implementer OPEN answered). Tessera rules only (CP23); no fact of the install changed. The other implementer OPENs of 2026-10-16 (§4.3, §5.2, §5.4, §5.6, §8.3, §10, §10.3, §14) are not touched by this revision. |
| 2026-10-23 | Header only: independent review by spec-review:spec-ui-scaling-p6:1 (independent session) of the 2026-10-23 revision (§3, §12, §15.4; CP23): passed; status Revised → Accepted (record in the header). No fact or rule changed. |
