<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Quest journal: the quest log, the quest tracker, objective text and counters, quest messages and quest markers

| | |
|---|---|
| **Status** | Review (awaiting independent review) |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Pending: an independent spec-review session that wrote no part of this spec |
| **Review record** | None yet. First version of 2026-10-10 for decision 0024 P3 (client half only), ROADMAP P3 exit criterion 1. |
| **Implements into** | `crates/gamedata` (`quest` module: log entries, categories and order, title and level, display texts, objective text and counter values, reward rows shown, tracker entries, ring and marker choices, the per-record census of §9), `crates/ui` (game widgets and fallback widgets of the types named here: quest tree, quest display, quest objectives list, quest tracker, history window, map and radar quest notes), `apps/client` (the quest log and tracker panels, their commands to the session, `--open-panel` names, quest lines in chat, rings over NPCs in the walker, quest markers on the minimap and the world map) |
| **Companion specs** | [`quests.md`](quests.md) (§2–§7 records, objectives, roles; §4.3 effective level; §9 rewards; §10 deeds; §11 arcs), [`npc-windows.md`](npc-windows.md) (§5.5 player variables, §6.2–§6.3 objective and reward blocks), [`scenario-feed.md`](scenario-feed.md) (revision 4 Accepted, revision 5 by the P3 NPC-window writer: §5.2, §6.3, §6.5, §7.6, §9.2, §9.4), [`string-table.md`](string-table.md) (§3.2 argument IDs, §7 rendering, §8.3 substitution, §9 StringInfo), [`ui-layout.md`](ui-layout.md) (§17 panels P9 and P11), [`game-panels.md`](game-panels.md) (§4 drag boxes, §6 `HideWhenField`, §8 radar, §9 world map), [`map-data.md`](map-data.md) (§4.8 note icon tables), [`chat-window.md`](chat-window.md) (§4.3, §5.1 chat types and lines), [`game-controls.md`](game-controls.md) (§7.5 consideration tables), [`script-table.md`](script-table.md) (§8.2 script-key lookup), [`particle-runtime.md`](particle-runtime.md) (effects started by script keys), [`property-system.md`](property-system.md) (§7 enum display texts) |

> **Naming note (interoperability).** Layout labels (`quest`, `questtracker`, `questdisplay`,
> …), element labels (`QuestTree`, `QuestTracker_QuestTextLine2`, …), state labels
> (`quest_solo`, `ConEven_ColorName`, …), widget-type labels, string-table labels (`UI_Quest`,
> `Quest`, `PlayerActionHandler`), script-key labels of the FX mapper (`Quest_Available`, …),
> property names and enum labels are the install's own identifiers, quoted because an
> implementation must find the data by them. No game text appears in this spec: texts are named
> by string table and token, argument IDs by their hash value.

---

<a id="s0"></a>
## 0. Purpose

ROADMAP P3 (decision 0024 §5) asks for the quest log, the quest tracker with objective
counters, quest messages in chat and quest markers on NPCs and on the maps, with exit
criterion 1: *log and tracker show every quest record from a scenario without failure*. This
spec gives, from the install's UI layouts, records and class library (M1) and, where those
stop, from the offline client (M4, decision 0011):

- the quest log window: layout, which quests it lists, grouping and order, entry rows, header
  counts, filters, the details pane and its sections, controls and commands (§4);
- where every objective text and counter comes from, and how counters are put into texts (§5);
- the quest tracker: layout, which quests, entry content, height budget, completion, controls
  (§6);
- the chat lines for a new quest, an objective update, a completion and a failure (§7);
- the rings over NPCs and the quest markers on the minimap and the world map (§8);
- a census of every quest record for the criterion (§9) and the place of deeds (§10).

**Client half only** (decision 0024 §2.4, LEGAL.md §11.1 rule 5). Quest states, objective
counts, tracking and NPC rings come from the scenario feed (`QuestState`, `QuestObjective`,
`QuestTracked`, `MarkerAdded`, `PropertyChanged`, `scenario-feed.md` §9.2) or a server. The
client derives no quest state: it never decides that an objective advanced, that a quest is
eligible or complete, or which reward is due. It computes only presentation from the states it
is given and the install's data (text, order, layout). Where the original client's choice is
game code that was not traced, or depends on server state, a **Tessera rule** stands in and is
marked as such.

All values (DIDs, element and state IDs, tokens, class and slot numbers, counts) are
observations of one install. Implementations find layouts by `UILAYOUT` label, elements by
`UIElementID` label, string tables by `STRINGTABLE` label, properties by name and script keys
by label, and read every value at run time.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (Steam, client build 4906.0071.1794.4035), read-only, in place; archives unchanged since 2026-10-04 (file times re-checked 2026-10-10). All 302 UI layout records (dump of 2026-10-06 decoded per `ui-layout.md` §4, values re-read with a throwaway decoding probe on 2026-10-10); the DID maps `UILAYOUT`, `STRINGTABLE` (0x28000004 here), `EMAPPER`; the enum mappers named in the text; the gameplay-option defaults set (`GameplayOptionDefaults`, 0x78000006 here); the radar and map note icon tables (0x78000002, 0x78000001); the script tables (all 7,526 records of prefix 0x07, 7,524 decoded); with throwaway probes over the repository's own decoders (read-only): every one of the 20,807 quest records (names, categories and their enum display texts, levels, descriptions, every objective, every completion-condition event entry, progress and billboard overrides and the argument IDs of every text they name), the string tables `UI_Quest`, `Quest` and `PlayerActionHandler` (tokens, piece counts and argument IDs; the text was read only to locate tokens and is not reproduced). |
| **BC** | The class library's code (record 0x56000000), data in the user's install (method M1), disassembled on 2026-10-10 with the repository's `tessera-wvm` decoder (`wlib-bytecode.md`, Accepted) and a throwaway method-table probe over `tessera-wstate`. Read (class indices and virtual slots of this install): the quest journal class 1753 slots 26, 61, 62, 79, 82, 84, 85, 91, 97, 107, 124, 145; the event-progress class 1418 slots 9, 20, 39, 68, 74; the event base class 1423 slots 37, 40, 50, 51, 59, 73, 83, 94, 96, 97, 107, and slot 73/96 of each compiled event class of `quests.md` §5.4; the quest class 1398 slots 92, 105, 124, 125, 131, 133, 136, 142, 144, 193, 199; the journal-base class 1405 slots 9, 18, 20, 41, 101, 102, 104, 105; the local player's class 287 functions 1, 4–8, 16, 18–21, 23, 29, 92, 121, 258 (the functions the executable calls, below); class 260 function 2 and class 1695 slots 85–86 (difficulty state). No message text was read beyond locating tokens. |
| **BIN** | **M4 under the standing authorisation of decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, SHA-256 prefix d22363eb9aff0fb5), headless decompiler on a private copy of the shared analysis project (`~/tessera-re/work/p3qj`, deleted after use), 2026-10-10; functions found by instruction search for element IDs, property IDs and string tokens as immediate operands. **Questions:** (a) which quests the quest-tree widget lists, how it groups and orders them, what an entry row shows; (b) what the quest-display widget shows and in which order, and which class-library functions supply its texts; (c) what the quest-tracker widget shows per entry, how it limits the height, which template it takes, and what its button and context menu do; (d) what the log's header labels, track and abandon buttons show and when they are ghosted; (e) which class-library function IDs the native script-machine calls of these widgets name. **Why M1–M3 were not enough:** the layouts name the elements and templates but not which code fills them, the grouping and sort keys, the height rule or the button states; the class library holds the journal's text rules but not the native widgets; no published source describes them; the running client is not available to this session (owner checks §13 cover the visible results). **Read:** the quest panel's constructor and update routines (header counts, task limiter, shown-count label, abandon and track button states), the quest-tree widget's constructor, its add-quest, title, scope-bit, rebuild, filter and status-change routines and its two comparators, the quest-display widget's build routine and its header, bestowal, description, objective and objective-list routines, the objective-list widget's fill routine, the quest-tracker widget's entry builder, entry update and context-menu routines, the twenty wrappers that call class-library functions of class 287 (only their function IDs), and the registration table of one native package (to name two native IDs). **Nothing from network, login, launcher or anti-tamper code was read** (decision 0004). |
| **Design** | Tessera rules, marked **Tessera rule**: what Tessera does where the original's behaviour is server state, untraced game code, or outside P3. Not claims about the original. |

| # | Fact | Source | Mark |
|---|---|---|---|
| QJ1 | Quest log layout, elements, templates, filter buttons and their scope bits, the task limiter struct (§4.1, §4.5, §4.6) | OBS | Confirmed 2026-10-10 |
| QJ2 | The tree lists either the underway quests or the completed quests (the Show Completed box switches); deeds are never listed; a quest leaves the list when it stops being underway (§4.2) | BIN (a); BC (class 287 function 6) | BIN 2026-10-10 |
| QJ3 | Categories by `Quest_Category`, titled by the category's enum display text with a fallback token; categories ordered by title, quests by effective level then by name, both without case (§4.3) | BIN (a) | BIN 2026-10-10 |
| QJ4 | Entry row: title format with and without level, difficulty state, scope bits and the icons they show, timer (§4.4) | BIN (a); OBS (templates) | BIN 2026-10-10 |
| QJ5 | Filter: a quest row is shown when its scope bits share a bit with the selected filter buttons, and its title matches the name filter (§4.5) | BIN (a) | BIN 2026-10-10 |
| QJ6 | Header counts: the quest count text, the shown-count label and the task-limiter text with their tokens and argument IDs (§4.6) | BIN (d); OBS (tokens) | BIN 2026-10-10 |
| QJ7 | Quest display: section order, the header (title, arc, category, group icon), objective block, objective text, background, repeat note, rewards (§4.7) | BIN (b); BC (class 287 functions 5, 7, 8, 16, 19, 92, 258) | BIN + Confirmed (bytecode) 2026-10-10 |
| QJ8 | Abandon and track button states and labels; the abandon confirmation texts (§4.8) | BIN (d); BC (class 287 function 1, class 1405 slots 9, 41, 101–105) | BIN + Confirmed (bytecode) 2026-10-10 |
| QJ9 | Objective progress text: completed and failed words, objective-level override with objective counters, event lines per condition joined by line breaks and an alternative separator (§5.2) | BC (class 1753 slots 84, 97, 124; class 1398 slot 105) | Confirmed (bytecode) 2026-10-10 |
| QJ10 | Event line: show flag, override, drama and tapped variants, per-type default text, counters, the deed counter suffix (§5.3) | BC (class 1418 slot 20, class 1423 slots 37, 40, 50, 51, 59, 73, 83, 94, 96, 97, 107) | Confirmed (bytecode) 2026-10-10 |
| QJ11 | Counter argument IDs and what fills them (§5.4) | BC (class 1418 slot 9, class 1753 slots 26, 61, 62, 124; classes 1433 and 1859 slot 96); OBS (argument IDs of every quest text) | Confirmed 2026-10-10 |
| QJ12 | Census of the argument IDs of every name, description, objective, progress and billboard text of the 15,400 quests: all are counters, the two named values, or player variables of `npc-windows.md` §5.5 (§5.4, §9) | OBS | Confirmed 2026-10-10 |
| QJ13 | Tracker layout, templates, height properties, gameplay-option defaults (§6.1, §6.2) | OBS | Confirmed 2026-10-10 |
| QJ14 | Tracker capacity: a quest cannot be added when the unlocked and locked tracked lists hold 10 together (§6.2) | BC (class 1405 slots 9, 41, 101, 102) | Confirmed (bytecode) 2026-10-10 |
| QJ15 | Tracker entry: template choice, name line, objective text, height budget per entry, ellipsis, timer, ready icon, lock and focus markers, points bar (§6.3) | BIN (c) | BIN 2026-10-10 |
| QJ16 | Tracker button and context menu: items, their tokens and what they do (§6.6) | BIN (c); OBS (tokens) | BIN 2026-10-10 |
| QJ17 | Quest state messages: new quest, new deed, completed, failed; hidden deeds silent; quest chat type (§7.1) | BC (class 1753 slots 82, 137; class 1398 slots 92, 124, 131); chat type mask from `chat-window.md` §4.1 | Confirmed (bytecode) 2026-10-10 |
| QJ18 | Objective update messages from billboard texts (§7.2) | BC (class 1753 slot 91, class 1418 slots 39, 68, 74, class 1423 slots 94, 107, 50) | Confirmed (bytecode) 2026-10-10 |
| QJ19 | Rings over NPCs: the journal picks one quest per NPC from its role list, runs a ring script key on the NPC and stores the quest in `Quest_CurrentBestowRingDID`; the key by ring kind, scope, tier and monster play (§8.1) | BC (class 1753 slots 79, 85, 107, 145, the ring-key routine) | Confirmed (bytecode) 2026-10-10; the kind tests partly traced |
| QJ20 | The 39 ring script keys are defined in script tables 0x0700000D (39 keys) and 0x070017E7 (35 keys), reached from 807 tables by includes; `Quest_Available` runs a scriptlet with a particle system (§8.1) | OBS (all 7,524 decoded script tables) | Confirmed 2026-10-10 |
| QJ21 | Quest note images of the radar and map icon tables for the types `Quest`, `QuestSpecial`, `QuestFailer`; `RadarQuestObjective`; the map's quest-objective indicator properties (§8.3) | OBS | Confirmed 2026-10-10 |
| QJ22 | Census of all 20,807 quest records for criterion 1 (§9) | OBS | Confirmed 2026-10-10 |
| QJ23 | Difficulty state of a quest title from the quest level and the player's level through the consideration control's quest tables (§4.4) | BC (class 260 function 2, class 1695 slots 85–86); `game-controls.md` §7.5 | Confirmed (bytecode, partial) 2026-10-10 |
| QJ24 | Every rule marked **Tessera rule** | Design | Decision |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Quest** | A quest record with `Quest_IsAccomplishment` absent or false (`quests.md` §4.1). |
| **Deed** | A quest record with `Quest_IsAccomplishment` true. |
| **Feed state** | The latest `QuestState` the session reported for a quest (`Active`, `Completed`, `Failed`, `Removed`), or none. `Active` is the original's raw status underway (0x1), `Completed` completed (0x3), `Failed` failed (0x2) (`quests.md` §8.3). |
| **Objective report** | The latest `QuestObjective { objective, count, total, done }` for one (quest, `Quest_ObjectiveIndex`) pair. |
| **Tracked** | The latest `QuestTracked` flag for a quest (false when none). |
| **Current objective** | The objective whose texts the log and the tracker show (§5.1). |
| **Effective level** | `quests.md` §4.3, with the player values of §4.4. |
| **Scope bits** | A 32-bit mask over the `UIQuestScope` mapper (0x2300001E here: bit *k* − 1 for key *k*), computed per quest for filtering and icons (§4.4). |
| **Ring** | The script effect the client runs on an NPC to mark its quest role (§8.1). |
| **Counter IDs** | The string-table argument IDs of §5.4 that receive counts. |

---

<a id="s3"></a>
## 3. What the journal consumes and sends

| Feed event (`scenario-feed.md` §9.2) | Effect here |
|---|---|
| `QuestState { quest, state }` | log membership (§4.2), tracker line text (§6.4), chat line (§7.1), ring (§8.2) |
| `QuestObjective { quest, objective, count, total, done }` | current objective (§5.1), counters (§5.4), chat line (§7.2), ready icon (§6.4) |
| `QuestTracked { quest, tracked }` | tracker membership (§6.2), the log's track button (§4.8) |
| `MarkerAdded { …, quest: Some(_) }`, `MarkerRemoved` | quest markers on the radar and the world map (§8.3) |
| `PropertyChanged { entity, property, value }` | the ring of an NPC (`Quest_CurrentBestowRingDID`, §8.2); player values for levels and limits (§4.4, §4.6) |
| `LevelChanged`, `CharacterInfo` | the player's level (titles, difficulty states, §4.4) and player variables (`npc-windows.md` §5.5) |

| Control | Command (`scenario-feed.md` §9.4) |
|---|---|
| Track button, tracker menu Remove / Remove All | `TrackQuest { quest, tracked }` (§4.8, §6.6) |
| Abandon button, after confirmation | `AbandonQuest { quest }` (§4.8) |

The generic answer to `TrackQuest` is `QuestTracked` (`scenario-feed.md` §7.6); `AbandonQuest`
has none, so a scenario answers it with a `quest … removed` step in an `on` trigger.

**Client state kept** (Tessera rule): per quest, the feed state, the objective reports by
objective index and the tracked flag, in the order first reported; per entity, the reported
properties (`scenario-feed.md` §9.5 bound). `Removed` drops a quest's objective reports and
its tracked flag. Literal quest refs (`Ref::Literal`) are kept and shown with their literal as
the name, no category text (the fallback of §4.3), no level and no objectives.

> **OPEN:** (implementer, 2026-10-10) only `Removed` drops a quest's objective reports. When a
> repeatable quest goes from `Completed` or `Failed` back to `Active`, the reports of the
> earlier run stay, so (§5.1) the repeat starts at the objective after the highest done one
> and the ready icon (§6.4) may show at once until new reports arrive. `tessera-gamedata`
> keeps them, as written. Should a transition to `Active` from `Completed` or `Failed` clear
> the reports (or must a scenario send fresh `objective` steps)?

---

<a id="s4"></a>
## 4. Quest log window (P9)

<a id="s4-1"></a>
### 4.1 Layout and elements (Confirmed, QJ1)

Layout `quest` (`UILAYOUT` key 0x10000047 → 0x22000043 here); root `QuestPanel` (widget
`UIScaler`), design rect (9, 118) 770 × 550, minimum 770 × 550, maximum height 800, saves
location and size, auto-fade 0.1, object mode Stretch, scaling group `Panels`
(`ui-layout.md` §17.2 P9). Opened by the input action `ToggleQuestPanel` (key L by default,
`npc-windows.md` NW10). Elements (positions in their parents):

| Element (widget) | Rect | Role |
|---|---|---|
| `QuestPanel_Top_Header` | (5, 19) 761 × 88 | header art (image 0x41102349) |
| `BaseBox_TitleBar` → `QuestPanelLabel` | | title text (`UI` table token 0x048AC0CC) |
| `QuestPanelMoveBar` (`Dragbar`), `QuestPanelCloseButton`, `Quest_Panel_Resize_Bar` (`ResizeBar`, bottom) | | move, close, resize |
| `Quest_Journal_Page` (`Field`) | (−1, 26) 768 × 519 | holds the rest |
| `QuestTree` (`QuestTreeDisplay`) with `QuestTree_SB` | (11, 99) 321 × 384 | the quest list (§4.2–§4.5): vertical list, one column, click selects (`highlight` / `normal` states), tree indent 8, auto-prune, parents hidden when every child is filtered, tree icons from layout 0x22000065 (`Craft_Profession_TreeIcon`) |
| `QuestTree_ExpandAll_Button`, `QuestTree_CollapseAll_Button` | (12, 79), (32, 79) 16 × 16 | expand or collapse every category |
| `Quest_Shown_Label` | (20, 79) 312 × 20 | shown-count text (§4.6) |
| `SelectedQuestDisplay` (`QuestDisplay`) with `SelectedQuestDisplay_SB` | (344, 80) 410 × 401 | details pane (§4.7); `UI_QuestDisplay_DisplayType` 4 (`CurrentObjective`), `UI_QuestDisplay_ShowProgress` true |
| `QuestTrackerButton`, `QuestShareButton`, `QuestAbandonButton`, `QuestHistoryButton`, `QuestFindAFellowshipButton` | row at y 491 | §4.8 |
| `Quest_IncreaseFontSize`, `Quest_DecreaseFontSize`, `Quest_FontExample` | (35, 486) | font size of the details pane (seven fonts listed on `Quest_FontExample`) |
| `Quest_ShowTrackerField`: `Quest_ShowTrackerCheckBox`, `Quest_Tracker_QuestObjectiveMode`, `Quest_Tracker_FellowshipStatusMode` and labels | (548, −6) | tracker options (§6.2) |
| `Quest_CountText` with `QuestCount_Label`, `QuestSlot_MithrilButton` | (416, 11) | quest count (§4.6); store button |
| `Quest_Filters` (`Field`, `UI_QuestJournal_FilterElementArray`) | (10, −5) 356 × 78 | filter buttons (§4.5), `Quest_ShowCompleted` box, name filter `Quest_Filter_Text` |
| `Quest_Task_Filter` (`HideWhenField`, hides for monster players, `game-panels.md` §6.3) | (322, 24) | task limiter: `Quest_Task_Count_Label`, `Quest_Task_Count_Text`, two store buttons (§4.6) |
| `QuestHistory` | | the history window (§4.9), hidden at creation |

Templates of the same layout (top-level elements, created per row by the tree):
`QuestTreeCategoryTitle` (text, 300 × 16, one line, truncated with tooltip, fonts 0x42000007 and
0x4200000C) for a category row; `QuestTreeQuestTitle` (button `button_quest_entry`, 313 × 36)
for a quest row, with children `ConColorField` (36 × 36, nine `Con…_ColorName` states with
images), `TitleText` (238 × 18), the scope icons `EpicImageField` (0x41008124),
`FellowshipImageField` (0x41008125), `RaidImageField` (0x41008126), `SmallFellowshipImageField`
(0x41008127), `TaskImageField` (0x41008128), `InstanceImageField` (0x41008129),
`RepeatableImageField` (0x4100812A) (10 × 10 each, all with tooltips of table `UI_Quest`),
`QuestTimeRemaining` (`Timer`), `TrackerButton` (states `highlight` 0x410E0B81, `normal`
0x410E4045), `ReadyToCompleteField` and `QuestCompletedField` (image 0x410E3F4A), the fellow
share images `Fellow1ImageField` … `Fellow5ImageField` on `FellowImageField_Background`, and
`QuestIsLFFQuest`. The tree finds these children through the struct
`UI_QuestTree_QuestTitleElement` on `QuestTree` (one property per role) and the category
template through `UI_QuestTree_QuestCategoryElement`.

**Tessera rule (store and group controls):** `QuestSlot_MithrilButton`, the task limiter's
two store buttons (`Tasks_Reset_BuyNowButton_Wrapper`,
`Tasks_Increase_Daily_Limit_BuyNowButton_Wrapper`), `QuestShareButton`,
`QuestFindAFellowshipButton`, `QuestIsLFFQuest`, the fellow share images and
`Quest_Tracker_FellowshipStatusMode` are hidden or ghosted in P3 (no store, no fellowship).
`--open-panel quest-log` opens the window (with the scenario's quests, or empty).

<a id="s4-2"></a>
### 4.2 Which quests are listed (BIN, QJ2)

The tree is in one of two modes, chosen by the `Quest_ShowCompleted` box (unchecked at
creation, Tessera rule): **active** lists every quest with raw status underway; **completed**
lists every quest with raw status completed. A rebuild clears the tree and asks the class
library (class 287 function 6) for the quests of that status. When a quest's status changes,
the tree adds it if the new status is underway and removes it otherwise (in active mode).
Every quest record with `Quest_IsAccomplishment` true is skipped before any row is made, so
**deeds are never in the quest log** (§10).

**Tessera rule (from the feed):** active mode lists the quests whose feed state is `Active`;
completed mode those whose feed state is `Completed`. `Failed` and `Removed` quests are in
neither list (as in the original, a failed quest is not underway and not completed). Deeds
(record with `Quest_IsAccomplishment` true) are skipped in both. Literal refs are listed by
their feed state like records.

<a id="s4-3"></a>
### 4.3 Grouping and order (BIN, QJ3)

1. **Category rows.** Each quest row sits under a category row keyed by the quest's
   `Quest_Category` value (absent: key 0). A category row is created from
   `QuestTreeCategoryTitle` the first time a quest of that key is added; its text is the key's
   **display text** in the enum mapper that the `EMAPPER` entry labelled `QuestCategory` names
   (key 0x1000004B → 0x23000019 here, the mapper of `Quest_Category`; `property-system.md`
   §7.1, display texts looked up along the base chain). When the key has no display text, the text is the `Quest` table token
   0x0F59FEF6 (a fixed fallback).
2. **Order of categories:** by their row text, compared without case (wide-character
   case-insensitive comparison), ascending.
3. **Order of quests within a category:** by effective level ascending (§4.4), then by the
   quest's name (`Quest_Name` text) compared without case, ascending.
4. There is no further grouping: arcs (`quests.md` §11) and regions do not group rows; the
   arc name appears only in the details pane (§4.7).

**Tessera rules:** the case-insensitive comparison uses the simple Unicode lower-case mapping
of each character and then code-point order; equal keys keep the order in which the quests
were first reported. A display text that fails to resolve (`string-table.md` §9.3) is treated
like an absent one (the fallback token). At creation every category row is expanded.
`ExpandAll`/`CollapseAll` expand or collapse every category row. Selection: a click selects
a quest row (state `highlight`); when nothing is selected (first open, or the selected quest
left the list) the first quest row in display order is selected; a category row cannot be
selected.

<a id="s4-4"></a>
### 4.4 The entry row (BIN, QJ4, QJ23)

For each quest row (template `QuestTreeQuestTitle`):

- **Level.** The effective level *L* of `quests.md` §4.3 (`Quest_HideChallengeLevel`, then the
  override property read from the local player, then `Quest_ChallengeLevel`, else 0). The row
  stores it as `UI_QuestTree_QuestLevel` for sorting.
- **Title** (`TitleText`). When *L* ≥ 1: the format string of the element's
  `UI_QuestTree_TitleLabelElementFormatString` (`Quest` table token 0x0108CAB5 here) with
  argument 0x0050AA9C = *L* (integer) and argument 0x0058E905 = the quest's name; when *L* < 1:
  `Quest` table token 0x0B355705 with argument 0x0058E905 = the name. The lower-cased title is
  kept as the row's filter key (§4.5).
- **Difficulty state.** The state set on `TitleText` and on `ConColorField` is the result of
  class 260 function 2 for *L* (`TitleText` defines no states, so only `ConColorField`, whose
  nine `Con…_ColorName` states carry images, changes; `ui-layout.md` §9.7): it reads the `ConsiderationControl` object (class 1695 here),
  takes a table from its quest-table list (`ConsiderationControl_QuestTableList`,
  `game-controls.md` §7.5) by level, and a rank by the difference between the quest level and
  the player's level (with *L* = 0 the player's own level is used, so the difference is 0);
  `ConsiderationControl_RankToUIState` turns the rank into a `Con…_ColorName` state.
- **Scope bits and icons** (one bit per `UIQuestScope` key; icon shown exactly when its bit is
  set by the rule):

  | Bit (key) | Set when | Icon shown |
  |---|---|---|
  | 0x40 (`Epic`) | `Quest_Scope` = 3 | `EpicImageField` |
  | 0x80 (`Raid`) | `Quest_ShowRaidInJournal` true | `RaidImageField` |
  | 0x02 (`Fellowship`) | `Quest_IsFellowshipRecommended` true | `FellowshipImageField` |
  | 0x04 (`SmallFellowship`) | not fellowship, `Quest_IsSmallFellowshipRecommended` true | `SmallFellowshipImageField` |
  | 0x20 (`Solo`) | neither of the two above and not raid | — |
  | 0x08 (`Task`) | `Quest_Scope` = 1 | `TaskImageField` |
  | 0x100 (`Instance`) | `Quest_IsInstanceQuest` or `Quest_ContainsInstanceQuest` true | `InstanceImageField` |
  | 0x01 (`Repeatable`) | `Quest_MaxTimesCompletable` stored, not 1, and `Quest_Scope` ≠ 4 | `RepeatableImageField` |
  | 0x200 (`Shared`) | a fellow shares the quest (fellowship state) | fellow images |
  | 0x10 (`Normal`) | no other bit set | — |

- **Timer** (`QuestTimeRemaining`): shown when the class library reports a remaining time
  above 0 (class 287 function 67); hidden otherwise.
- **Ready and completed icons:** `ReadyToCompleteField` and `QuestCompletedField` (same image,
  tooltips `UI_Quest` 0x07F8730E and 0x07B236EE) mark a quest that is ready to complete and a
  completed quest (by their tooltips; the routine that shows them was not read).

**Tessera rules:** *L* uses the player's values as reported: for an override property the
player's value from `PropertyChanged` of the player entity for that property, and for
`Advancement_Level` (the character-level property, 1,615 quests here) also the level of the
latest `LevelChanged` or `CharacterInfo` of the player; an unknown value counts as a failed read
(next step of `quests.md` §4.3). The difficulty rank follows the data-consistent reading of
`game-controls.md` §7.5 with the quest-table list (list element by the player's level, table
element *d* + 15 for *d* = *L* − player level, clamped to 0–30); with no known player level the
state is `ConEven_ColorName`. Bit 0x200 is never set (no fellowship in P3). The timer is hidden
(the feed has no remaining-time form; OPEN, §14 Q8). The ready icon follows §6.4; the completed
icon is shown on every row in completed mode.

<a id="s4-5"></a>
### 4.5 Filters (Confirmed data QJ1; BIN QJ5)

`Quest_Filters` lists thirteen elements in `UI_QuestJournal_FilterElementArray`: twelve toggle
buttons (`button_toggle`, 36 × 33, icon child with `normal`/`highlight` state images) each
carrying `UI_QuestJournal_Filter_QuestScope` (a `UIQuestScope` mask) and, for six of them,
`UI_QuestJournal_Filter_Alignment` (`Good` 1 or `Evil` 3), plus `Quest_ShowCompleted`
(check box, mask 0):

| Button | Mask (key) | Alignment |
|---|---|---|
| `Quest_Filter_Solo` | 0x20 (6) | — |
| `Quest_Filter_SmallFellowship`, `…_MP` | 0x04 (3) | Good, Evil |
| `Quest_Filter_Fellowship`, `…_MP` | 0x02 (2) | Good, Evil |
| `Quest_Filter_Instance` | 0x100 (9) | — |
| `Quest_Filter_Epic` | 0x40 (7) | — |
| `Quest_Filter_Raid` | 0x80 (8) | — |
| `Quest_Filter_Repeatable` | 0x01 (1) | — |
| `Quest_Filter_Shared`, `…_MP` | 0x200 (10) | Good, Evil |
| `Quest_Filter_Tasks` | 0x08 (4) | — |

The panel shows a button with an alignment only when it equals the player's alignment (the
`…_MP` buttons stand in the same places for monster players). A filter button's pressed state
mirrors whether its mask is in the tree's filter mask; a click toggles it. A quest row is
**shown** when (its scope bits AND the filter mask) ≠ 0 and its lower-cased title contains the
name filter text (`Quest_Filter_Text`, editable, one line); otherwise it is hidden, and a
category whose rows are all hidden is hidden (`UICore_Tree_auto_filter_parent_with_filtered_children`).
The bit `Normal` (0x10) has no button.

**Tessera rules:** the player's alignment is `Good` (1), so the six `…_MP` buttons are hidden.
The filter mask at creation has every bit set (0x3FF), so every row is shown, and the `Normal`
bit stays set whatever the buttons do. The name filter compares lower-cased texts by
substring; an empty filter matches every row.

<a id="s4-6"></a>
### 4.6 Header counts (BIN, QJ6)

| Element | Text (table, token) | Arguments |
|---|---|---|
| `Quest_CountText` | `UI_Quest` 0x0E18AE34 (two integers with a separator) | 0x08A76A74 = number of quests underway (class 287 function 20); 0x00005168 = the player's quest limit (class 287 function 21: the player's maximum-quest properties `Quest_MaxActiveQuests`, `Quest_BaseMaxActiveQuests` and the additional-slot properties, server state) |
| `Quest_Shown_Label` | `UI_Quest` 0x0843160C | 0x00484A34 = rows shown after filtering; 0x0059485C = rows in the tree; 0x00059E45 = `UI_Quest` 0x0A6E23FC (active mode) or 0x04E88B4C (completed mode) |
| `Quest_Task_Count_Text` | `UI_Quest` 0x0E18AE34 | per entry of `UI_QuestJournal_QuestCategoryLimiter_Array` on `QuestPanel` (one entry here: category key 112 `Task`, text element `Quest_Task_Count_Text`): 0x08A76A74 = quests taken in that category, 0x00005168 = the category limit, both player state of the limiter (`quests.md` §9.8); the count reads 0 once the player's `QuestCategoryLimiter_ResetTime` has passed |

The quest-count element is refreshed when the quest list changes; the task text when the
limiter state changes; the shown label on every rebuild and filter change.

**Tessera rules:** quests underway = the number of non-deed quests with feed state `Active`.
The limit is the player's `Quest_MaxActiveQuests` from `PropertyChanged` when reported; without
it `Quest_CountText` shows the count alone as a decimal integer. The task count is the number of
non-deed `Active` quests whose `Quest_Category` is the limiter's category, and the task limit
is the player's value of the property named by that category's
`QuestCategoryLimiterControl_PropertyName` when reported, else its
`QuestCategoryLimiterControl_DefaultLimit` (`quests.md` §9.8). The shown label counts quest
rows (not category rows).

> **OPEN:** (implementer, 2026-10-10) the task text needs three install values this spec does
> not say how to read: (1) the field names of the entries of
> `UI_QuestJournal_QuestCategoryLimiter_Array` (the category key and the text element);
> (2) the limiter's `QuestCategoryLimiterControl_PropertyName` and `…_DefaultLimit` per
> category, which `quests.md` §9.8 places in the class-library root object (class 3087, a
> class-35 map) rather than in a property set, so the record readers of `tessera-gamedata`
> cannot reach them. `tessera-gamedata` provides `TaskLimiter` (count and limit from the
> three values) and `quests_underway`; the caller supplies the values until the spec names
> the fields and a reader of the root object exists.

<a id="s4-7"></a>
### 4.7 Details pane (BIN, QJ7)

`SelectedQuestDisplay` (widget `QuestDisplay`) shows the selected quest. It builds a vertical
list of elements created from the top-level templates of layout `questdisplay`
(`UILAYOUT` key 0x10000054 → 0x22000067 here), each sized to the pane's width. For an
**underway quest** (display type 4) the list is, in this order:

1. **Header** (when `UI_QuestDisplay_ShowTitle`, default true): `QuestDisplay_QuestHeaderField`
   (base `Box_QuestJournal_Header`, 410 × 92) with
   - `QuestDisplay_QuestHeaderTitle`: the title as in §4.4 (format of
     `UI_QuestDisplay_TitleLabelElementFormatString`, the same token, or the name alone);
   - `QuestDisplay_QuestHeaderArcName`: the quest's arc name from the class library (class 287
     function 92; the arc is the `QuestArc_Name` of the arc that lists the quest,
     `quests.md` §11); when there is none, the element is hidden and
     `QuestDisplay_QuestHeaderCategoryName` moves up to the mean of the two elements' tops;
   - `QuestDisplay_QuestHeaderCategoryName`: the category text of §4.3 (display text only; no
     fallback token is set here);
   - `QuestDisplay_QuestHeaderIcon` state: `quest_raid` when `Quest_ShowRaidInJournal`, else
     `quest_fellowship` when `Quest_IsFellowshipRecommended`, else `quest_smallfellowship` when
     `Quest_IsSmallFellowshipRecommended`, else `quest_solo`; the `…_mp` state of the same name
     for monster players;
   - `QuestDisplay_QuestHeaderXPTier`: hidden in normal play (shown only for an
     administrative display mode, with `UI_Quest` 0x0B2B7502 and `Quest_ExpTier`).
2. **Objective block**: an instance of `QuestDisplay_ObjectivesField` (base
   `QuestObjectives_Root` of layout `questobjectives`) holding one `QuestObjectives_Objective`
   entry (bullet image 0x41007E02, markup on, wraps) whose text is the **progress text** of the
   current objective (§5.2, class 287 function 5), followed by one item row
   (`QuestObjectives_TokenDisplay` with a medium item display) per quest-token item the class
   library lists for the quest (class 287 function 121). No header row in this mode.
3. **Objective text**: `QuestDisplay_QuestObjectiveDescription` (NarrativeFont, grey, markup on)
   with the current objective's `Quest_ObjectiveDescription` (class 287 function 7 → class 1398
   slot 133); left out when the objective has none.
4. **Background** (when `UI_QuestDisplay_ShowDescription`, default true, and the quest has a
   description): `QuestDisplay_QuestDescriptionLabel` (`UI_Quest` 0x099EECDC) then
   `QuestDisplay_QuestDescription` with `Quest_Description` (class 287 function 8 → class 1398
   slot 125).
5. **Repeat note** (when `UI_QuestDisplay_ShowDescription` is true and
   `Quest_MaxTimesCompletable` is stored):
   `QuestDisplay_QuestRepeatableDescription` with `UI_Quest` 0x05DD1B74 when the player has
   completed the quest before (class 287 function 258), else 0x00B22074.
6. **Rewards** (when `UI_QuestDisplay_ShowRewards`, default true): an instance of
   `QuestDisplay_RewardsField` (base `QuestRewards_Root`), filled like the quest-offer window's
   reward block (`npc-windows.md` §6.3).
7. **Filter row**: `QuestDisplay_QuestFilterListbox` with the per-character and per-account
   filter toggles.

For a **completed quest** (display type 4 and status completed) and in the history window
(display type 2), the order is: header; when `UI_QuestDisplay_ShowBestowalDialog` (history) or
completed: `QuestDisplay_QuestBestowLabel` (`UI_Quest` 0x08B189FC) and
`QuestDisplay_QuestBestowText` with the quest's bestowal dialogue (class 287 function 4);
background (4); then, for every objective *i* from 1 to the last (completed) or to the current
one (history), `QuestDisplay_QuestObjectiveLabel` with `UI_Quest` 0x0B86343C and argument
0x053A16A2 = *i*, followed by that objective's `Quest_ObjectiveDescription`; then rewards and
the filter row. Display type 3 (`NoObjectives`) leaves the objective parts out.

**Tessera rules:** the progress text is §5.2 with the feed's counters (§5.4). The quest-token
item rows are left out (the class library's list is player state; OPEN, §14 Q9). The arc name
is the `QuestArc_Name` of the arc whose `QuestArc_Quest_Array` lists the quest (`quests.md`
§11: a quest is in at most one arc). Monster-play states are never used. The administrative
tier line is hidden. The repeat note uses 0x00B22074 ("never completed") unless the feed has
reported the quest `Completed` earlier in the session, then 0x05DD1B74. The reward block shows
the treasure-record rows of `npc-windows.md` §6.3 steps 2–3 and no amount rows (the feed's
`QuestRewards` is bound to the quest-offer window; OPEN, §14 Q10). The filter row is hidden. The
bestowal dialogue of a completed quest is the line of `npc-windows.md` §6.4 rules 2–3 (no
giver). A literal quest shows the header with its literal and nothing else. The font-size
arrows step through the seven fonts of `Quest_FontExample` like `npc-windows.md` §5.2's rule
for the conversation pane.

> **OPEN:** (P3 exit verifier, 2026-10-11) the texts of this pane (objective text, background
> description) are not stated to wrap or to grow with their text. With the template's text
> properties as they are, Tessera draws a long `Quest_Description` on one line cut at the pane
> width (local golden of the quest log, 2026-10-11). Whether the original wraps these texts and
> sizes each element to its text is an owner check (O113).

> **OPEN:** (implementer, 2026-10-10) for an underway quest whose current objective gives no
> line under §5.2 (no override, no shown event line), the §5.2 Tessera rule makes the
> objective's `Quest_ObjectiveDescription` the progress text (item 2), and item 3 then shows
> the same description again: the pane shows it twice (1,493 objectives here, minus those
> without a description). `tessera-gamedata` follows the text as written. Is item 3 left out
> when the progress text came from the description?

<a id="s4-8"></a>
### 4.8 Controls and commands (BIN, QJ8)

| Control | Original | Tessera |
|---|---|---|
| `QuestTrackerButton` (label from base `RealQuestTrackerButton`) | Label `UI_Quest` 0x0C275572 with tooltip 0x006D3754 when the selected quest is not tracked, 0x081F3F12 with tooltip 0x01EB6284 when it is. Ghosted when no quest is selected, the quest is completed, a private-encounter quest owns the tracker (`Quest_TrackerPEQuestDID` set), the quest is an instance quest (`Quest_IsInstanceQuest`, tracked or not), or it is not tracked and the tracker is full (§6.2). A click toggles tracking (class 287 function 29 reports the state). | Same labels; tracked = the feed's tracked flag; ghosted when nothing is selected, the selected quest's feed state is `Completed`, `Quest_IsInstanceQuest` is true, or it is not tracked and 10 quests are tracked. A click sends `TrackQuest { quest, tracked: !tracked }`. |
| `QuestAbandonButton` (label from base `RealQuestAbandonButton`) | Ghosted when no quest is selected, the quest is completed, or `Quest_CanBeCancelled` is false (default true; 2,426 quest records store false). A click asks for confirmation (class 287 function 1): `Quest` token 0x092EECC4 when `Quest_IsInstanceQuest`, else 0x0522E51E when `Quest_IsSessionQuest`, else 0x0EF53D54 when the player is inside an instance, else 0x0A912DDE; argument 0x00569A84 = the quest's name. Confirming sends the abandon request. | Same ghosting from the feed state and the record; the confirmation dialog (`ui-behaviour.md` dialog boxes) uses the same token choice with "inside an instance" false; OK sends `AbandonQuest { quest }`, cancel sends nothing. |
| `QuestShareButton`, `QuestFindAFellowshipButton` | Fellowship share and group finder | Ghosted (Tessera rule of §4.1) |
| `QuestHistoryButton` | Opens the history window for the selected quest (§4.9) | Same |
| `QuestTree_ExpandAll_Button`, `…CollapseAll_Button` | Expand / collapse | Same |
| `Quest_ShowCompleted` | Switches active / completed mode (§4.2) | Same |
| `Quest_ShowTrackerCheckBox` | `GOP_ShowQuestTracker` (default true): tracker panel visible | Same, as a Tessera setting |
| `Quest_Tracker_QuestObjectiveMode` / `…FellowshipStatusMode` | `GOP_QuestTracker_IsPartyStatusMode` (default false) | Objective mode only |
| `QuestPanelCloseButton`, `EscapeKey` | Close | Same |

The labels of the five buttons come from their bases' `UICore_Text_entry` in the layout
(`RealQuestShareButton` and `RealQuestAbandonButton` carry table 0x25000023 tokens); the
track button's label is replaced by code as above.

> **OPEN:** (implementer, `apps/client`, 2026-10-11) `tessera-ui` has no dialog box widget
> yet, so the client confirms an abandon by a second click on `QuestAbandonButton` for the
> same quest: the first click posts the confirmation text of the token choice above as a
> notice line, the second sends `AbandonQuest` (cancel = selecting another quest or closing
> the log). The history button (§4.9) shows the selected quest in the details pane in history
> order instead of opening `QuestHistoryField`. Both are stand-ins until the widgets exist;
> please confirm or name the dialog box layout the confirmation should use.

> **OPEN:** (implementer, `apps/client`, 2026-10-11) `Quest_ShowTrackerCheckBox`'s Tessera
> setting is not saved yet: the client keeps the tracker's visibility while it runs (on at
> start, restored on the check box whenever the log opens) but `tessera-settings` has no
> entry for it, so every start shows the tracker again. The font index of the font-size
> arrows (§4.7) is likewise not saved. Both need a settings entry (owner of
> `tessera-settings`), saved off the main thread (ARCH §21).

<a id="s4-9"></a>
### 4.9 History window

Layout `questhistory` (key 0x1000006C → 0x22000044 here), root `QuestHistoryField`
(widget `QuestHistory`, 448 × 475) with a `QuestDisplay` list (`QuestHistory_ListBox`,
display type 2 `PastObjectives`, `UI_QuestDisplay_ShowBestowalDialog` true,
`UI_QuestDisplay_ShowRewards` false), scroll bar, close buttons and title (`UI` token
0x06C3490C). It shows the selected quest per §4.7 (history order). **Tessera rule:** the
objectives listed are 1 … current objective (§5.1).

---

<a id="s5"></a>
## 5. Objective text and counters

<a id="s5-1"></a>
### 5.1 The current objective (Tessera rule)

The original takes the current objective from the player's quest record (class 287 function
16). From the feed, for a quest with feed state `Active`:

1. if some objective report has `done` false, the current objective is the **highest** such
   `Quest_ObjectiveIndex`;
2. else, if there are reports (all done), it is the lowest objective index of the record above
   the highest reported one; when there is none above, the highest reported one;
3. else (no report), the lowest objective index of the record (`quests.md` §5.1: 1 … *n*).

An index the record does not have is ignored with a warning. For `Completed` and `Failed`
quests no current objective is needed (§5.2 step 1).

<a id="s5-2"></a>
### 5.2 Progress text of an objective (BC, QJ9)

The journal (class 1753 slot 84) builds one text for a quest:

1. raw status completed → `Quest` token 0x0FEF4204; failed → `Quest` token 0x08285964 (one
   word each, no argument);
2. otherwise, for the current objective *o* (slot 97):
   1. if *o* has `Quest_ObjectiveProgressOverride`: that StringInfo with the **objective
      counters** (§5.4) and nothing else from the events;
   2. otherwise, for each **completion condition** of *o* in stored order, the **event lines**
      of its events (§5.3) in walk order (`quests.md` §5.7 step 1.2, compound members
      included), joined by `Quest` token 0x0A3193D5 (a line break between two arguments
      0x0896E2F1 and 0x0896E2F2); a compound event with `QuestEvent_CompoundProgressOverride`
      contributes that one line (with the event counters of its members, §5.4) instead of its
      members' lines; events the class library marks as completed or not to be shown add no
      line;
   3. the per-condition texts are joined by `Quest` token 0x0FBBD982 (two arguments with an
      alternative marker between them on a line of its own): the conditions of an objective
      are alternatives (`quests.md` §5.2);
   4. failure conditions add no line.
3. The text is put into the element as one string; each line break starts a new bulleted line
   in `QuestObjectives_Objective` and in `QuestTracker_QuestTextLine2` (both have a bullet
   image).

**Tessera rules:** the completed and failed words follow the feed state. Events are never
marked completed individually (the feed has per-objective counts only), so every shown event
line of the current objective is listed. When step 2 gives no line at all (no override and
no event with a shown line), the text is the objective's `Quest_ObjectiveDescription`; when
that is absent too, `UI_Quest` token 0x0B86343C with argument 0x053A16A2 = the objective index.

<a id="s5-3"></a>
### 5.3 The event line (BC, QJ10)

For one event (class 1418 slot 20 over the compiled event, class 1423 and its subclasses):

1. No line when `QuestEvent_ShowProgressText` is false (dictionary default true).
2. The base text, first that applies:
   1. status underway and `QuestEvent_DramaProgressOverride` present: that text;
   2. a "tapped" state (status 0x5…): `QuestEvent_TappedProgressOverride` when present,
      else as 3;
   3. `QuestEvent_ProgressOverride` when present;
   4. otherwise the event class's **default text**: every compiled event class except the
      base overrides one function (slot 73) that builds a text from `Quest` table templates
      and the event's fields (for example the names of the target records); the base gives
      none. A subclass may then add its own arguments (slot 96): type 11 sets argument
      0x00005343 to the name of the `QuestEvent_NPCTalk` record; type 34 sets 0x005A6195 to
      the current value of the integer world event (and a second value).
3. The event counters are set (§5.4).
4. A second line from the event class (slot 97; the base gives none, landmark events add one)
   is joined with `Quest` token 0x0A3193D5.
5. **Deed counter:** when the caller asks for it, the quest is a deed and the event's required
   number (`QuestEvent_Number`) is above 1, and the base text does not use **both** 0x053A16A2
   and 0x0059485C, the text becomes `Quest` token 0x0F6EF2D4 with 0x0896E2F1 = the text and the
   two counters (the text followed by the count and the total).

**Tessera rules:** steps 2.1 and 2.2 do not apply (no drama or tapped state in the feed). Step
2.4 is not implemented in P3: an event without `QuestEvent_ProgressOverride` gives no line
(OPEN, §14 Q1; census §9: 2,979 shown events here). Step 4 gives no line. Step 5 applies to
deeds shown in the tracker (§6), with the counters of §5.4.

<a id="s5-4"></a>
### 5.4 Counter arguments (BC, QJ11; census QJ12)

| Argument ID | Set by (original) | Value (original) | Tessera value from the feed |
|---|---|---|---|
| 0x053A16A2 | class 1418 slot 9 | the event's progress count | the objective report's `count` (0 without report) |
| 0x0059485C | class 1418 slot 9 | the event's required number (`QuestEvent_Number`) | the report's `total`; without report the event's `QuestEvent_Number` (default 1) |
| 0x06A30283 | class 1418 slot 9 | required − count | `total` − `count`, not below 0 |
| 0x000C09A3 | class 1753 slots 26/61–62, 124 | the objective's summed counts (each event capped at its required number; completed objectives count as full) | the report's `count` (0 without report) |
| 0x063E25B3 | as above | the objective's summed required numbers | the report's `total`; without report the sum of `QuestEvent_Number` over the objective's completion-condition events |
| 0x005A6195 | type-34 events (class 1859 slot 96) | the world event's integer value | the report's `count` |
| 0x00005343 | type-11 events (class 1433 slot 96) | the name of the `QuestEvent_NPCTalk` record | the same name (display name of the record) |

Event counters (the first three) fill event lines (§5.3) and compound overrides; objective
counters fill `Quest_ObjectiveProgressOverride` and `Quest_ObjectiveBillboardOverride`. Player
variables (the 20 IDs of `npc-windows.md` §5.5) are filled in every text of this spec by the
same Tessera rule as there.

**Census (QJ12).** Over the 15,400 quests, the arguments used are: names none; descriptions
0x03EC29B5 and 0x0EB8DCB3 (14 texts each); objective descriptions 0x03EC68D5 (13) and
0x0EB8DCB3 (4); objective progress overrides 0x000C09A3 (580), 0x063E25B3 (553) and two player
variables; event progress overrides 0x053A16A2 (8,052), 0x0059485C (7,746), 0x005A6195 (86),
0x00005343 (4); compound overrides the two event counters; billboard overrides the same sets.
**Every argument of every such text is in the table above or in the player-variable table.**
Many progress texts write the total as a literal number and use only 0x053A16A2.

> **OPEN:** (Q2, feed gap) the feed reports one count and total per **objective**, while the
> original counts per **event**. An objective with several counted events shows the same
> numbers on every line (Tessera rule above). A per-event form (for example an optional event
> index in `QuestObjective`) would be an additive feed revision; owner check O119.

---

<a id="s6"></a>
## 6. Quest tracker (P11)

<a id="s6-1"></a>
### 6.1 Layout (Confirmed, QJ13)

Root `GamePlay_QuestTrackerBox` of layout `gameplayactivatables` (a `HiddenDragBox`,
`game-panels.md` §4), design rect (670, 223) 340 × 550, edges (4, 5, 5, 4), title
`HiddenDragBox_Title` = `UI` token 0x0CFB777C, scaling group `QuestTracker`, object mode
CanvasResize, saves location (`ui-layout.md` §17.2 P11). Its child `GamePlay_QuestTracker`
(widget `QuestTracker`, (0, 20) 340 × 530, object mode Stretch,
`UI_QuestTracker_LockOption` = `GOP_LockQuickslots`) holds one entry per tracked quest,
created from templates of layout `questtracker` (key 0x1000005B → 0x22000028 here):

| Template | Use |
|---|---|
| `QuestTracker_QuestTemplate` (339 × 85, `UI_QuestTracker_TotalMaxObjectives` 20, `UI_QuestTracker_SizePerObjective` 12, states `highlight`/`normal`) | a tracked quest |
| `QuestTracker_QuickslotQuestTemplate` | a tracked quest that has a quest-item shortcut (an item display at the left) |
| `QuestTracker_PEQuestTemplate`, `QuestTracker_PEQuickslotQuestTemplate` | the private-encounter quest |

Children of `QuestTracker_QuestTemplate`:

| Child | Rect | Content |
|---|---|---|
| `QuestTracker_QuestTextLine1` (text `activequest_title`, one line, truncated) | (75, 1) 223 × 15 | quest name; colour 0xFFFFF600 (`normal`), 0xFFF3A20A (`highlight`) |
| `QuestTracker_QuestObjectiveMode` → `QuestTracker_QuestTextLine2` (text `activequest_body`, fit to text, markup on, bullet 0x41007E8A) | (73, 15) → (0, 0) 239 × 48 | progress text (§6.3); white (`normal`), 0xFFFFF600 (`highlight`) |
| `QuestTracker_Time_Ellipsis_Line` → `QuestTracker_QuestTimeLeft` (`Timer`) | below line 2 | remaining time |
| `QuestTracker_QuestPoints_Bar` (`QuestPointsBar`) with meter, medal and percentage label | hidden | point-score quests (`Quest_IncludesTrackedEvents`, `quests.md` §9.6) |
| `QuestTracker_ReadyToCompleteField` (image 0x41007F89, 16 × 16) | (38, 1) | ready to complete (§6.4) |
| `QuestTracker_LockedIndicator` (0x41007F8A) | hidden | locked entry |
| `QuestTracker_FocusIndicator` (states `override`, `focused`) | hidden | quest-guide focus |
| `QuestTracker_TeleToObjectiveButton` | hidden | travel to objective (store) |
| `QuestTracker_Button` (base `Button_QuestTracker`, 30 × 30, context menu on) | (306, −4) | open log / menu (§6.6) |
| `QuestTracker_QuestTextEllipsis` (text 12 × 12, `UI` token 0x037DDBC3) | (315, 27) | shown when line 2 is cut |
| `QuestTracker_PartyStatusMode` with five `QuestTracker_FellowField` | | fellowship status mode |
| `QuestTracker_Button_TutorialHighlight` | | tutorial (hidden, `game-panels.md` §5) |

<a id="s6-2"></a>
### 6.2 Which quests (QJ13, QJ14)

The tracker shows the player's tracked quests: the private-encounter quest
(`Quest_TrackerPEQuestDID`), the locked list (`Quest_TrackerArrayLocked`) and the tracked list
(`Quest_TrackerArray`), all player properties of quest DIDs. **Capacity:** a quest cannot be
tracked when the two lists hold 10 quests together (class 1405 slot 9: count ≥ 10). Deeds can
be tracked like quests.

Gameplay options (defaults in `GameplayOptionDefaults`): `GOP_ShowQuestTracker` true (panel
shown), `GOP_AutoFillQuestTracker` true, `GOP_AutoAddDeedToTracker` false,
`GOP_AutoAddTaskToTracker` true, `GOP_QuestTracker_IsPartyStatusMode` false,
`GOP_UIScale_QuestTracker` 1.0. The class library does not name the three auto-add options;
the executable names them only in one routine that handles many gameplay options (not read).
Who adds a newly accepted quest to the tracker was not found.

**Tessera rules:** the tracker shows the quests (and deeds) whose tracked flag is true, at most
10: a `QuestTracked` true for an 11th is kept but not shown until another entry leaves, with a
warning. The client never tracks a quest on its own; a scenario (or server) that wants the
original's automatic tracking sends `QuestTracked` after `QuestState` active (OPEN, §14 Q3).
`Removed` untracks. No private-encounter or locked entries in P3.

<a id="s6-3"></a>
### 6.3 Entry content and height budget (BIN, QJ15)

For each tracked quest:

1. **Template:** `QuestTracker_QuickslotQuestTemplate` when the quest has a quest-item
   shortcut, else `QuestTracker_QuestTemplate`. **Tessera rule:** always the plain template.
2. **Line 1:** the quest's `Quest_Name` (no level); the element is switched to the
   difficulty state of the effective level (§4.4), which it does not define, so its colour
   stays that of `normal` or `highlight` (`ui-layout.md` §9.7).
3. **Line 2:** the progress text of §5.2 (class 287 function 5).
4. **Height budget:** with *n* tracked entries, the line-2 height is limited to
   ⌊`UI_QuestTracker_TotalMaxObjectives` ÷ *n*⌋ × `UI_QuestTracker_SizePerObjective` pixels
   (⌊20 ÷ *n*⌋ × 12 here: 240 for one entry, 120 for two, 72 for three, 24 for ten); when the fitted text is taller, line 2 is cut to that height and
   `QuestTracker_QuestTextEllipsis` is shown; otherwise it is hidden.
5. **Timer:** shown under line 2 when the quest is underway and its remaining time is above 0.
6. **Ready icon** (§6.4), lock marker when the entry is locked (`UI_QuestTracker_QuestLocked`),
   focus marker when the quest is the quest-guide focus; the travel button only when a tracker
   flag and a class-library test allow it (not traced).
7. **Points bar:** for quests with `Quest_IncludesTrackedEvents`, the medal bar below line 2
   and a percentage label (`UI_Quest` 0x073BB615, argument 0x04A67A64).
8. The entry's height is that of its visible children; entries are stacked in the list.

**Tessera rules:** steps 5–7 are hidden in P3 (no timer, lock, focus, travel or point-score
form in the feed). The deed counter of §5.3 step 5 applies to tracked deeds.

<a id="s6-4"></a>
### 6.4 How completion is drawn

- A completed quest's line 2 is the completed word, a failed quest's the failed word (§5.2
  step 1); the entry stays until the quest leaves the tracker.
- `QuestTracker_ReadyToCompleteField` is shown, for a quest (never a deed), when the class
  library reports it ready to complete (class 287 function 23: the current objective is the
  last one and the quest has more than one objective, with further tests not traced).
- Counters show the count and total of §5.4; an objective report with `done` true moves the
  current objective (§5.1).

**Tessera rule (ready):** shown for an `Active` quest (not a deed) whose current objective
(§5.1) is the record's highest objective index, when the record has two or more objectives.

<a id="s6-5"></a>
### 6.5 Order (Tessera rule)

The original lists entries in the order of the player's tracker lists (not traced in detail:
OPEN, §14 Q4). **Tessera rule:** entries in the order in which their `QuestTracked` true first
arrived, earliest at the top.

<a id="s6-6"></a>
### 6.6 Controls (BIN, QJ16)

- **Click** on `QuestTracker_Button`: opens the quest log at that quest (the deed log for a
  deed). Tooltip `UI` token 0x09DBBD94.
- **Right click** opens a context menu of `PlayerActionHandler` table items: "show in log"
  (0x02D768CC; deeds 0x0D4EDD34), the status-mode switch (0x0FEDF8A5 / 0x0F8A6EF0 /
  0x04552A65), lock or unlock (0x03D71BEC / 0x04F67FCC), find others (0x0A2D422C), quest-guide
  focus (0x07ADCC1C / 0x006610CC), **remove** (0x0ADC8C8C), **remove all** (0x0F81D16C), track
  nearby quests (0x0E16800C), highlight related objects (0x0CF6645C).

**Tessera rules:** the menu offers only "show in log", "remove" and "remove all". Remove sends
`TrackQuest { quest, tracked: false }`; remove all sends it for every tracked quest in display
order. Show in log opens the quest log with the quest selected (switching to the mode of its
feed state); for a deed it does nothing (no deed log in P3).

> **OPEN:** (implementer, `apps/client`, 2026-10-11) `tessera-ui` has no popup menu widget
> yet: a right click on `QuestTracker_Button` opens the menu state for the quest and posts the
> three item labels as a notice line; the choice is carried out through the client's menu
> entry point (tests drive it). The popup's layout (which element or template shows the
> items) is not named here; please name it when the menu widget is specified.

---

<a id="s7"></a>
## 7. Quest messages (chat)

<a id="s7-1"></a>
### 7.1 State messages (BC, QJ17)

On a status change the journal (class 1753 slot 82) builds one line and posts it (class 1753
slot 137) to the chat with the 64-bit type mask 0x100000, chat type 21 `Quest` of the
`ChatTextType` mapper (`chat-window.md` §4.1, bit *k* − 1 for type *k*):

| New status | Message (`Quest` table token) | Argument |
|---|---|---|
| underway, quest | 0x04131F74 | 0x00569A84 = `Quest_Name` |
| underway, deed with `Quest_IsHidden` false or absent | 0x04755C44 | same |
| underway, deed with `Quest_IsHidden` true | no message | |
| completed | 0x0FF5E454 | same |
| failed | 0x028F5AC4 | same |
| other | no message | |

**Tessera rule:** the client posts these lines on `QuestState` `Active` (from any other state
or none), `Completed` and `Failed`; `Removed` posts nothing (the original's removal line is
server text; a scenario uses `system`). A repeated report of the same state posts nothing.
Lines are added to the chat log by `chat-window.md` §5.1 with type 21, which the default first
tab shows (`chat-window.md` §3.3).

<a id="s7-2"></a>
### 7.2 Objective update messages (BC, QJ18)

When an event's progress changes, the journal (class 1753 slot 91) posts, on the same chat
type:

1. if the current objective has `Quest_ObjectiveBillboardOverride` (class 1398 slot 105):
   that StringInfo with the objective counters (§5.4);
2. else, if the event has a billboard text: `QuestEvent_BillboardProgressOverride` (or the
   event class's default) with the event counters, when `QuestEvent_ShowBillboardText` is true
   (default true).

**Tessera rule:** on each `QuestObjective` that changes `count` or `done` for the current or an
earlier objective, post rule 1 for that objective, else rule 2 for the **first** event of the
objective's completion conditions (walk order) that has a billboard override and a true show
flag; nothing otherwise. Census: 21,645 completion-condition events here have a shown billboard
override.

> **OPEN:** (Q5) whether these lines also appear on screen (centre-screen text) in the
> original, and in which element; the posting routine also calls a native of package 345 that
> was not identified. Owner check O121.

<a id="s7-3"></a>
### 7.3 Messages that are server text

The `Quest` table also holds lines the class library and the executable never name (journal
updated, quest removed, time expired, experience earned, sharing results). They come from the
server. **Tessera rule:** the client posts none of them; a scenario posts them with `system`.

---

<a id="s8"></a>
## 8. Quest markers

<a id="s8-1"></a>
### 8.1 Rings over NPCs (BC QJ19, OBS QJ20)

The original marks NPCs with a quest role by running a **ring script** on the NPC entity:

1. For each NPC that the client knows, the journal (class 1753 slots 79, 107, 145) walks the
   NPC's role list (`QuestDispenser_ActiveQuests`, `quests.md` §7.4) and, for each quest, gives
   the role a **ring kind** and a priority from the player's state for that quest (underway,
   completed, eligible, level, required expansions or store items, session play). The highest
   priority wins (the first one on a tie).
2. When a kind wins, the journal stops the NPC's previous ring script (slot 85), starts the
   script key of the kind (table below) on the NPC, and sets the NPC's
   `Quest_CurrentBestowRingDID` (a DID reference to a game object) to the winning quest; when
   none wins, it stops the ring and removes the property.
3. The script key comes from the FX mapper (0x23000097 here, the script-key mapper of
   `script-table.md` §2.1) and is looked up in the NPC's script tables (`script-table.md`
   §8.2). The ring's shape and placement over the figure are the script's own nodes: for
   example `Quest_Available` runs a scriptlet whose particle system (prefix 0x39) is placed at
   one of the figure's attach points with a far fade between 40 and 60 world units. All 39 keys are
   defined in script table 0x0700000D (and 35 of them in 0x070017E7), which 807 script tables
   reach through includes.

**Key by ring kind** (class 1753, the ring-key routine). Variant rules: **E** = `Quest_Scope`
3 (epic) gives the `…_Epic` key; **H** = `Quest_ExpTier` above 4 gives the `…_Hard` key;
scope 2 gives `Quest_Crafting`, 5 `Quest_Item_Advancement`, 9 `…_Allegiance`, 10 `…_Mission`;
**MP** = the `MP_…` key of the same name for a monster player.

| Ring kinds | Keys (labels of the FX mapper) |
|---|---|
| 4, 13 (available) | `Quest_Available`, `Quest_Available_Hard`, `Quest_Available_Epic`, `Quest_Available_Epic_Hard`, `Quest_Crafting`, `Quest_Item_Advancement`, `Quest_Available_Allegiance`, `Quest_Available_Mission`; MP variants |
| 1, 5, 14, 19, 29 (advance) | `Quest_Advance`, `Quest_Advance_Hard`, `Quest_Advance_Epic`, `Quest_Advance_Epic_Hard`, `Quest_Crafting`, `Quest_Item_Advancement`, `Quest_Advance_Allegiance`, `Quest_Advance_Mission`; or `Quest_InProgress` when an entity test of the NPC holds; MP variants |
| 7 | `Quest_InProgress` (only when that entity test holds) |
| 3 (future) | `Quest_InFuture`, `Quest_InFuture_Epic` |
| 6 (below level) | `Quest_BelowLevel` |
| 9 | `Quest_Timedquest` |
| 10 | `Quest_SessionPlay` |
| 8 | `Quest_Comment` |
| 16, 18 | `QuestRing_NeedExpansion` |
| 20 | `QuestRing_NeedWebstoreItem` or `QuestRing_NeedExpansion` |

The gameplay option `GOP_OverheadQuestIcons` (default true) exists; where it gates the rings
was not found.

<a id="s8-2"></a>
### 8.2 Rings from the feed (Tessera rule)

The ring kind depends on eligibility and player state, which the client does not derive. The
existing feed form `stat NPC @Quest_CurrentBestowRingDID=$quest[i]` (a `PropertyChanged` with a
DID value, `scenario-feed.md` §6.1) names the quest whose ring an NPC shows; the client chooses
the key from that quest and its feed state:

1. no `Quest_CurrentBestowRingDID` on the entity, value none or a DID that is not a quest
   record: no ring;
2. the quest's feed state is none: an **available** key (variants E, H and the scopes of §8.1;
   never MP);
3. state `Active`: an **advance** key (same variants) when the entity's body record
   (`EntityCreated.class`) is the `QuestDispenser_NPC` of a `Quest_RoleArray` role with action
   2 (`quests.md` §7.2) whose `Quest_ObjectiveIndex` is the current objective (§5.1);
   otherwise `Quest_InProgress`;
4. `Completed`, `Failed`, `Removed`: no ring.

Whenever the property or the quest's state changes, the previous ring script is stopped
(`particle-runtime.md` stop and linger rules) and the new one started; the script runs only
while the entity is spawned. Rings are shown when the Tessera setting for overhead quest icons
is on (default on).

> **OPEN:** (implementer, `apps/client`, 2026-10-11) the walker does not yet start script
> keys on entities, so the client computes the ring key of rules 1–4 and stands in for the
> ring script with a small world-anchored glyph above the NPC (2.4 m above its position;
> available, advance, in progress, other). The anchor height and the glyphs are Tessera
> choices until the script's own nodes run on the figure (`particle-runtime.md`). The
> overhead quest icons setting is the client's `--quest-icons on|off` (default on) for
> now: `tessera-settings` has no entry for it yet, so it is not saved between runs.

> **OPEN:** (Q6, feed gap) the feed has no form that names a ring **kind**: below-level,
> future, timed, session, comment and the expansion and store rings cannot be scripted. An
> additive form (for example an enum property whose value is an FX-mapper key) would cover
> them; the `npc-windows.md` writer owns `scenario-feed.md` for P3. Owner check O122 (which
> ring the original shows in the cases of rules 2–3).

<a id="s8-3"></a>
### 8.3 Quest markers on the minimap and the world map (Confirmed data, QJ21)

The radar and the map window draw map notes with the images of their icon tables
(`map-data.md` §4.8.5). The quest note types and their images (both tables the same):

| Note type (key, bit value) | Normal / selected / highlight image | Menu image |
|---|---|---|
| `Quest` (2, 0x2) | 0x41005E4E / 0x41005E50 / 0x41005E51 | 0x41005E4F |
| `QuestSpecial` (17, 0x10000) | map: 0x412536DA / 0x412536E1 / 0x412536E4, off-screen 0x412536D5 / 0x412536FD / 0x412536EE | 0x412536E5 |
| `QuestFailer` (36, 0x800000000) | 0x41005E84 / 0x41005E86 / 0x41005E87 | 0x41005E85 |

The radar also has `RadarQuestObjective` (based on `MapNoteTemplate` of layout `map`, 30 × 30,
image 0x410F8A54, hidden at creation, `game-panels.md` §8.2), and the map window carries the
quest-objective indicator properties `UI_Map_QuestObjectiveIndicator_*` (corner and tile
images, alpha 0.65) used to shade objective areas from the quest-target index
(`quests.md` §16).

**Tessera rule (feed markers):** a `MarkerAdded` with `quest` present is a quest marker. It is
drawn on the radar and on the world map at its `location` (`game-panels.md` §8.6.1 transform
with the shown map's georeference), with the image `icon` when given, else the `Quest`
type's normal image of that view's icon table; the `label` text is its tooltip; on the radar
it is drawn only inside `UI_Radar_RadarRadius` of the player, on the world map only when the
location maps into the shown map's 1024 × 768 frame. Mouse-over switches to the highlight image.
`MarkerRemoved` removes it. Markers without `quest` belong to P5. Objective-area shading,
`RadarQuestObjective` and the dungeon quest notes (`map-data.md` §5) are not drawn in P3 (OPEN,
§14 Q7).

> **OPEN:** (implementer, `tessera-ui`, 2026-10-10) (1) the unit of `UI_Radar_RadarRadius`
> (81 here) is not stated; `tessera-ui` reads it as pixels of the shown map from the player's
> pixel (the viewport is 170 pixels wide), not world units. (2) The drawing order of quest
> markers and the player marker is not stated; `tessera-ui` draws the markers above the map
> and below the player marker, on the radar as children of `MinimapViewport`, on the world
> map as children of `MapBackground`. (3) With a marker's own `icon`, mouse-over keeps that
> icon (the table's highlight image is used only for markers without one). Please confirm.

> **OPEN:** (implementer, `apps/client`, 2026-10-11) a feed marker's `location` is in its
> region's frame, and the transform of `game-panels.md` §8.6.1 uses the shown map's
> georeference of the player's region. The client draws only the markers whose `location`
> region equals the player's region and hides the others (they reappear when the player
> enters that region). Please confirm, or state how a marker of another region is shown.

---

<a id="s9"></a>
## 9. Census for exit criterion 1 (Confirmed, QJ22)

A real-data test puts every quest record in exactly one category for the log and the tracker,
using the rules of §4–§6 with no feed counts. Counts of this install:

| Category | Meaning | Records |
|---|---|---|
| `Shown` | a non-deed record that decodes, whose name renders non-empty | 15,400 |
| `Deed` | `Quest_IsAccomplishment` true: excluded from the log (§10); the scenario selector uses `deed=no` | 5,407 |
| `NoName` | `Quest_Name` absent, failing (`string-table.md` §9.3) or empty: not showable | 0 |
| `Unreadable` | the record fails to decode | 0 |

Flags counted besides (none prevents showing; each has a rule above):

| Flag | Rule | Quests |
|---|---|---|
| `CategoryFallback` | no `Quest_Category` (1) or no resolving display text (1 key, 1 quest): fallback token (§4.3) | 2 |
| `LevelHidden` / `LevelFromPlayer` / `NoLevel` | §4.4: hidden 164; override property 2,168 (`Advancement_Level` and a skirmish level); neither override nor a stored level ≥ 1: 440; stored level 12,628 | — |
| `NoDescription` | `Quest_Description` absent (160), empty (175) or failing (5): no background section | 340 |
| `ObjectiveWithoutText` | an objective without (391) or with a failing (1) `Quest_ObjectiveDescription` | 327 |
| `ObjectiveWithoutLines` | an objective for which §5.2 step 2 gives no line under the Tessera rules (4,209 of 43,319 objectives; 103 of them also without description, which show the objective-number line) | 3,467 |
| `UnsuppliedArgument` | a shown text uses an argument outside §5.4 and the player variables | 0 |

Objectives of the 15,400 quests: 43,319; 2,054 have an objective-level progress override. In
the other 41,265 objectives, the 57,068 completion-condition events give 44,702 event lines;
182 have an override but `QuestEvent_ShowProgressText` false; 9,205 have neither override nor
show flag; 2,979 have no override and a true show flag (the original's default text, §5.3 step
2.4; OPEN Q1). Lines per objective: 0 for 4,209, 1 for 34,616, 2 for 2,999, 3 for 854, more
for 641 (up to 53 objectives with 10 or more). The shown-category count: 424 distinct
categories, of which 423 have a resolving display text.

> **OPEN:** (implementer, 2026-10-10) the `ObjectiveWithoutLines` count above (4,209
> objectives, 3,467 quests) and the event figures (57,068 events, 44,702 lines) count each
> compound member's own line and never a compound's `QuestEvent_CompoundProgressOverride`. §5.2
> step 2.2 makes a compound with that override contribute its one line instead of its members'
> lines (3,143 compounds here have one). The `tessera-gamedata` census follows §5.2 and finds
> 1,493 objectives in 1,238 quests without a line on this install; counting members as the
> table does reproduces 57,068 / 44,702 / 4,209 exactly. Please confirm §5.2 step 2.2 and
> restate the census row (or state that the census deliberately ignores compound overrides).

**What "without failure" means (Tessera rule):** for every `Shown` record, the log row, the
details pane and the tracker entry are created, every text of §4–§6 renders without a
`string-table.md` failure other than those counted as flags, and no element named here is
missing from the layouts (`LayoutMissing` and `ElementMissing` are failures, as in
`npc-windows.md` §11; this install has none).

---

<a id="s10"></a>
## 10. Deeds

The quest-tree widget skips deeds (§4.2): they belong to a separate **deed log** (the
accomplishment panel of layout `accomplishment` and its tabs, `quests.md` §10.4), which is not
part of P3. Deeds can be tracked (§6.2) and produce chat lines (§7.1).

> **OPEN:** (Q11, later milestone) the deed log window: tabs from
> `Accomplishment_UITabElements_Array`, deed groups, progress bars and the deed tracker rules.
> Until then the selector attribute `deed=no` (`scenario-feed.md` §8.3) chooses quests for the
> log, and a scenario that tracks a deed shows it only in the tracker.

---

<a id="s11"></a>
## 11. Invariants

Checked on this install (all records). A reader may warn on a violation but must not fail.

| # | Invariant |
|---|---|
| QI1 | Every quest record (non-deed) has a `Quest_Name` that renders non-empty (§9). |
| QI2 | Every argument ID used by a name, description, objective, progress or billboard text of a quest is a counter ID of §5.4 or a player variable of `npc-windows.md` §5.5 (QJ12). |
| QI3 | Every `Quest_Category` value used by a quest but one has a display text in the category mapper that resolves (§9). |
| QI4 | The layouts `quest`, `questtracker`, `questdisplay`, `questobjectives`, `questhistory` and `gameplayactivatables` hold every element this spec names, and the filter array names 13 elements of `Quest_Filters`. |
| QI5 | `QuestTracker_QuestTemplate` has `UI_QuestTracker_TotalMaxObjectives` 20 and `UI_QuestTracker_SizePerObjective` 12 (values read at run time). |
| QI6 | The 39 ring keys of §8.1 have labels in the FX mapper and definitions in at least one script table. |
| QI7 | The radar and map icon tables each have an entry for note type `Quest` (bit value 2). |

---

<a id="s12"></a>
## 12. Worked examples (synthetic)

<a id="s12-1"></a>
### 12.1 Log order

Three synthetic active quests: A (category key 5 with display text "Beta", level 12, name
"Wolves"), B (category 5, level 12, name "apples"), C (category 7 with display text "alpha",
level 3, name "Cart"). Rows: category "alpha" → C; category "Beta" → B ("apples" before
"Wolves" without case), A. Titles: the format token with 12 and "apples", 12 and "Wolves",
3 and "Cart". `Quest_Shown_Label` arguments 3, 3 and the active-mode token. With the `Task`
filter button alone pressed (mask 0x08 plus the always-set `Normal` bit), a quest whose scope
bits are only 0x20 (`Solo`) is hidden and the label's first argument drops.

<a id="s12-2"></a>
### 12.2 Progress text and counters

A synthetic quest's objective 2 has two completion conditions: X = one event of type 22 with
`QuestEvent_Number` 6 and a progress override "Defeat bats (${0x053A16A2}/${0x0059485C})"
(synthetic); Y = one event of type 11 with override "Talk to ${0x00005343}" and target record R
named "Orla". The feed sends `QuestObjective { objective: 2, count: 4, total: 6, done: false }`.
Current objective 2 (§5.1 rule 1). Progress text: line "Defeat bats (4/6)", then the
alternative line (token 0x0FBBD982), then "Talk to Orla". In the tracker with three tracked
quests line 2 is limited to ⌊20 ÷ 3⌋ × 12 = 72 pixels.

<a id="s12-3"></a>
### 12.3 Deed suffix

A synthetic deed objective has one event with `QuestEvent_Number` 10 and override "Explore the
hills" (no arguments). Tracked, with report count 3, total 10: line 2 is the text with token
0x0F6EF2D4: the override text followed by the count 3 and the total 10, in the token's form.

<a id="s12-4"></a>
### 12.4 Ring

NPC entity E (body record N) and quest Q (`Quest_Scope` absent, `Quest_ExpTier` 2). The
scenario runs `stat $e @Quest_CurrentBestowRingDID=$q[0]`: no feed state → `Quest_Available`.
After `quest $q[0] active` and `objective $q[0] 1 1/1 done`, the current objective is 2; Q's
`Quest_RoleArray` has an action-2 role with NPC N and objective index 2 → `Quest_Advance`. After
`quest $q[0] completed` the ring stops.

<a id="s12-5"></a>
### 12.5 Messages

`quest $q[0] active` posts token 0x04131F74 with Q's name on chat type 21; a following
`objective $q[0] 1 2/5` posts the objective's billboard override (if any) with 2 and 5;
`quest $q[0] completed` posts 0x0FF5E454.

---

<a id="s13"></a>
## 13. Owner checks (provisional numbers O118–O124)

> **OPEN:** the owner rows are numbered provisionally after `npc-windows.md` (O110–O117); the
> P3 close-out adds them to `docs/checklists/owner-observations.md`.

| Row | Check in the original client |
|---|---|
| O118 | Quest log: category order, quest order within a category, title format, which filter buttons are pressed at first open, whether categories start expanded, the count texts |
| O119 | Details pane: section order (progress lines before or after the objective text), how an objective with two counted events shows its numbers, the text of an event without its own progress text |
| O120 | Tracker: order of entries, the cut with the ellipsis for several tracked quests, whether accepting a quest tracks it, what remains after completing a quest |
| O121 | Chat and screen: the lines for a new quest, an objective update, a completion; whether the update also appears on screen |
| O122 | Rings over NPCs: which ring an NPC shows for an available quest, for the quest's next talk step and for a quest underway with another NPC |
| O123 | Minimap and world map: the icon of a quest objective or giver and its tooltip |
| O124 | Abandon: the confirmation and what the log shows after abandoning |

---

<a id="s14"></a>
## 14. Open questions (summary)

| # | Question | Section | Suggested method |
|---|---|---|---|
| Q1 | The per-type default text of an event without `QuestEvent_ProgressOverride` (slot 73 of each compiled event class; 2,979 shown events here) | §5.3 | BC (read the 38 slot-73 bodies and their `Quest` tokens); owner O119 |
| Q2 | Per-event counts in the feed | §5.4 | feed revision; owner O119 |
| Q3 | Who adds a new quest to the tracker (the three auto-add options) | §6.2 | BIN (option consumers); owner O120 |
| Q4 | The tracker's entry order | §6.5 | BIN; owner O120 |
| Q5 | On-screen display of quest messages | §7.2 | BIN (package 345 natives); owner O121 |
| Q6 | A feed form for ring kinds | §8.2 | feed revision; owner O122 |
| Q7 | Objective-area shading, `RadarQuestObjective` and dungeon quest notes | §8.3 | later milestone (P5) |
| Q8 | A feed form for remaining time (timers) | §4.4, §6.3 | feed revision |
| Q9 | Quest-token item rows of the objective block | §4.7 | BC (class 287 function 121) |
| Q10 | Reward amounts in the log's details pane | §4.7 | feed revision |
| Q11 | The deed log | §10 | later milestone |

---

<a id="s15"></a>
## 15. Notes for other specs

- `quests.md` §18: the journal's text rules of §5 (class library) and the ring choice of §8.1;
  deeds are never in the quest log (§4.2).
- `game-panels.md` §8.8 and §9.3: quest markers from the feed (§8.3).
- `chat-window.md` §4.7: quest lines use chat type 21 (§7.1).
- `game-controls.md` §7.5 Q16 (partly): the quest-table list is used for quest titles (§4.4).

---

<a id="change-log"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-10 | First version (Review), for decision 0024 P3. |
