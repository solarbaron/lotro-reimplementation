<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# NPC windows: using an NPC, dialogue, quest offers, vendors, loot and the wallet

| | |
|---|---|
| **Status** | Accepted (revision 2 of 2026-10-11, passed review 2 on 2026-10-11) |
| **Last updated** | 2026-10-11 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer, an independent session that wrote no part of this spec (review 1, 2026-10-11, changes requested; review 2, 2026-10-11, passed, by a session that wrote no part of the spec or its revisions) |
| **Review record** | First version of 2026-10-10 for decision 0024 P3 (client half only). **Review 1, 2026-10-11** (text of commit b62d0ae, with its notes in `item-tooltips.md`, `economy.md`, `interaction.md`, `string-table.md`): **changes requested**. Checked (CLEAN_ROOM.md §5.3): functional facts only; no code or pseudo-code (the input, buying, quantity and loot rules are plain-word steps); no copied prose; identifiers are install labels quoted for interoperability; no game text (texts named by table and token only), keys, hostnames or traffic; every fact group has a provenance row with source and date, BIN rows name decision 0011, questions (a)–(e) and why M1–M3 were not enough; Tessera choices are marked; nothing under a closed gate. Spot checks, own read-only probes of the install: §4 catalogue (8 of 8 labels, keys, layouts, roots, widget types, design rects); §9.2 money templates, coin icon states and images, worth and cost labels; §7.1 tab rectangles and page bases; §6.1/§5.2 close button, text pane and intro text; §7.5 element set; §8.1 Loot All button and its label token; §9.4 wallet items (167 + 443, 577 small icons, all category counts, mapper 16 names / 14 texts); §9.3 property-set currency order; §11 counts (quest offers, wallet, 10,579 vendor items, 859 comments) agree with the implementation's own census. Own M4 spot checks (decision 0011, private copy deleted after use): NW18 (three coin elements, `Copper`, `Silver`, `Gold`, each hidden, front insertion by option; count format as stated), NW13 (label choice by whether any listed entry is visible), NW15 (half preset constant 0.5; clamp and write-back of the set-quantity step; OK rules). The notes in the four other specs match this spec and, for the `item-tooltips.md` worth template, the install; NW20 and NW21 counts were not re-derived. **Blocking:** (1) §7.5 rule 4 and §13.2 contradict rule 1, and the binary settles it: every character typed into the entry re-reads its text; a non-empty text is read as an unsigned number (a text that is not a number reads as 0) and set as the quantity, so it is clamped to 1 … *M* and the entry is rewritten at once (typing 80 with *M* = 50 leaves 50, and OK then buys 50); an empty entry leaves the quantity unchanged, and OK on an empty entry does nothing. Revise rule 4, rule 5's "above *M*" case and §13.2; this also answers the §7.5 implementer OPEN. Implemented behaviour changes: `tessera-ui` keeps the typed text unclamped and treats an empty entry at OK like 0. (2) §6.3 places `QuestRewards_VirtueExpField` with the fixed virtues (step 2) while §10.3 lists `VirtueXp` among the step-1 amount rows; state one position (implementer OPEN of §10.3; `tessera-gamedata` may have to move the row). (3) §5.3: the two implementer OPENs were inserted inside the numbered Tessera rule, so rules 4 and 5 now read as part of OPEN callouts; move the OPENs after rule 5 (no behaviour change). **Non-blocking:** answer the remaining implementer OPENs (§3.1, §5.2, §5.3, §5.5, §7.5 opening quantity, §7.6, §7.7, §8.3 both, §9.4, §11 both); §8.3 with a coin-only `LootShown` the Tessera rule closes in the frame of the Loot All, before the feed can answer (decide); §7.5 could add that the set-quantity step also puts one dialog element (by inference the OK button) in state `ghosted` while the entry is empty or the cost exceeds the player's coin, `normal` otherwise; the header date and change log did not record the OPENs added on 2026-10-11. Full record in the review log (decision 0018). **Review 2, 2026-10-11** (revision 2, commit badecaf, with the notes of b62d0ae in `item-tooltips.md`, `economy.md`, `interaction.md`, `string-table.md`): **passed**; status Revised → Accepted. Checked (CLEAN_ROOM.md §5.3): every statement is a functional fact; §7.5 rules 1–8 and the number reading are plain-word conditions and steps, not code or a listing's structure; no copied prose; identifiers are install labels quoted for interoperability; no game text, keys, hostnames or traffic (texts by table and token only; the binary is named by a hash prefix); every fact group has a provenance row (the revision-2 BIN pass names decision 0011, its questions (c′), (d′) and why M1–M3 were not enough); layouts complete with units; invariants NWI1–NWI10 testable; unknowns are OPEN; nothing under a closed gate. Review 1's three blocking items are resolved: §7.5 rule 4 and §13.2 agree with rules 1 and 5; §6.3 step 2, §10.3 and §13.4 give the virtue experience row one position; §5.3 rules 1–5 form one list with the OPENs after it. Spot checks, own read-only probes of the install (M1): message key 0x28 is `Text_character_input` in the base mapper 0x230001AC of `UIElementMessage` (0x230000D0 here); state keys 0x10000013 `TooExpensive`, 0x12 `ghosted`, 0x0B `normal` in `UIStateID`; key 0xA2 is `AcceptInput` in mapper 0x23000176; the quantity dialog's OK button, entry (at most 5 characters) and cost display; `QuestRewards_VirtueExpField` with `…_BG`, `…_VirtueExpIcon` (32 × 32 at (12, 4)) and `…_VirtueExpRewardText` (319 × 33 at (45, 4)); NW21 re-derived over all 3,449 barter profiles (58,556 cost structs, 58,554 with an item, 44,856 naming one of the 610 wallet items, 423 distinct wallet items, 3,081 distinct cost items): agrees. Own M4 spot checks (decision 0011, private copy deleted after use), all agree with revision 2: NW15 (each character input to the entry re-reads it, a non-empty text is read and set with clamp and rewrite, an empty one only re-chooses the OK state; the reading skips leading `0` characters and then applies a general unsigned conversion; OK: empty does nothing, 0 → 1 and above *M* → clamped, both stay open, `TooExpensive` cost does nothing, else buy, remember vendor and quantity, close; the OK-state routine acts on the element of `VendorBuyStack_OkButton`, `ghosted` when the entry is empty or the player's coin is below the cost, else `normal`; opening while open closes first, same vendor reopens with the remembered quantity capped at *M*, another vendor forgets it, closing keeps it; Enter is `AcceptInput` on a key press while the dialog's root has focus), NW17 (the fixed-entry step calls the item, relic, trait, title and emote routines and then the virtue row, which reads only `QuestTreasure_Virtue_XP_Tier`; no instruction refers to the property ID of `QuestTreasure_FixedVirtueArray`). **Non-blocking (for a later revision):** (a) §7.5 last paragraph: the money-display option that makes the display set `TooExpensive` itself is switched on by the quantity dialog in code when it is built, on the coin list inside its cost display, not by a layout property; no behaviour change (the Tessera rule keeps the choice in the window). (b) §7.5 rule 1: in the original the cost and the OK state are refreshed only when the client's cost for that quantity can be computed; harmless for Tessera. (c) NW15 and NW17 carry OBS label facts under a BIN mark; the labels are now confirmed (this review). (d) The implementer OPENs of §3.1, §5.2, §5.3 (two), §5.5, §7.6, §7.7, §8.3 (two), §9.4 (two) and §11 (two) and the exit-verifier OPENs remain open; they do not block acceptance. **Implemented behaviour that revision 2 changes** (code on branch p3 predates it): `tessera-ui` quantity dialog (typed text clamped and rewritten at once, empty entry kept and OK on it ignored, OK ignored while the cost is `TooExpensive`, OK button `ghosted`/`normal`, opening quantity remembered per vendor); `tessera-gamedata` offer rows (rows for `QuestTreasure_FixedVirtueArray` entries are no longer shown; the virtue experience row follows the fixed emote entries). |
| **Implements into** | `crates/gamedata` (`npc` module: dialogue page text, the conversation and choice models, quest-offer content and reward rows, vendor, barter and loot entries with price display, wallet entries and the coin split, the census categories of §11), `crates/ui` (game widgets and fallback widgets of the window types named here: action list, vendor entry, quantity dialog, coin list, reward rows, wallet tree), `apps/client` (NPC use input, the windows, their commands to the session, `--open-panel` names), `crates/plugins` (the wallet display only, through the wallet model of §9; the plugin API itself is `lua-plugin-api.md` §15) |
| **Companion specs** | [`interaction.md`](interaction.md) §3.4 (use range, cursor, mouse-over), [`ui-layout.md`](ui-layout.md) (layouts, `UILAYOUT`, §9 building panels, §17 opening), [`ui-behaviour.md`](ui-behaviour.md) (input routing, window manager, tooltips), [`ui-widgets.md`](ui-widgets.md) (scroll bars, list boxes), [`game-panels.md`](game-panels.md) (`HideWhenField`), [`string-table.md`](string-table.md) (rendering, markup, display cut, variables), [`quests.md`](quests.md) (§4 records, §7 roles and comments, §9 rewards), [`economy.md`](economy.md) (§6 vendors and barter, §7 currency), [`items.md`](items.md) (item properties, sellability §5.3), [`icon-layers.md`](icon-layers.md) (composed icons), [`item-tooltips.md`](item-tooltips.md) (examination tooltips, §6.11 worth), [`character-panels.md`](character-panels.md) (§5 item slots, bags), [`keymap.md`](keymap.md) (§12 key maps, K42–K46), [`scenario-feed.md`](scenario-feed.md) (revision 4 Accepted; revision 5, the P3 revision of §10 here), [`lua-plugin-api.md`](lua-plugin-api.md) §15 (wallet views) |

> **Naming note (interoperability).** Layout labels (`questdispenser`, `vendor`, …), element
> labels (`RootQuestField`, `Vendor_BuyButton`, …), state labels (`Gold`, `TooExpensive`, …),
> widget-type labels, input-action names (`USE`, `AutoLootAll`, …), property names and enum
> labels are the install's own identifiers, quoted because an implementation must find the
> data by them. No game text appears in this spec: texts are named by string table and token.

---

<a id="s0"></a>
## 0. Purpose

Decision 0024 P3 needs the client half of NPC interaction: using an NPC, the windows an NPC
opens (conversation, quest offer, vendor, barter, loot) and the wallet. This spec gives, from
the install's UI layouts and records (M1) and, where the data stop, from the offline client
(M4, decision 0011):

- which input uses an entity and what the use needs from P2's picking and selection (§3);
- for each window: its layout, root and elements, open and close rules, how each element is
  filled from the install or from the scenario feed, and the commands its controls send
  (§4–§9);
- the scenario-feed forms the windows need (§10; a small additive revision of
  `scenario-feed.md`);
- how a real-data test categorises records the windows cannot show (§11).

**Client half only** (decision 0024 §2.4, `server-content.md` §2): the client runs no vendor,
loot, price, quest or dialogue rule. Which window opens, its entries, prices, stock,
quantities, reward amounts and quest states come from the scenario feed or a server. The
client computes only presentation (layout, text rendering, coin splitting, sorting for
display). Where the original client's own choice is game code that was not traced, a
**Tessera rule** stands in and is marked as such.

All values (DIDs, element and state IDs, positions, counts) are observations of one install.
Implementations find layouts by `UILAYOUT` label, elements by `UIElementID` label, records by
WEENIECONTENT label or by the family rules of the specs named, and read every value at run
time.

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (Steam, client build 4906.0071.1794.4035), read-only, in place; archives unchanged since 2026-10-04 (file times re-checked 2026-10-10). All 302 UI layout records (dump of 2026-10-06 decoded per `ui-layout.md` §4, re-read with a throwaway value-decoding probe on 2026-10-10; layout 0x22000047 does not decode with that probe and holds none of the elements named here); the `UILAYOUT` DID map (0x28000003 here) and the DID maps `STRINGTABLE`, `WEENIECONTENT`, `EMAPPER`; the enum mappers named in the text; the key-map records 0x1D000000 and 0x1D000001 and the mappers 0x230000AF, 0x23000176, 0x230000B0 (`keymap.md` §12); with throwaway probes over the repository's own decoders (read-only): the property sets of all 20,807 quest records, all 3,449 barter profiles, the 400 vendor inventories and their 10,579 distinct items, the wallet control record and its 610 items, all 859 NPC comment records, the currency directory in both forms (property set and object record); the argument IDs (not the text) of the 57,534 string-table entries named by role-line properties of quest records and of the 859 comment contents. |
| **BC** | The class library's code (record 0x56000000), data in the user's install (method M1), disassembled on 2026-10-10 with the repository's `tessera-dattool bytecode` (`wlib-bytecode.md`, Accepted) and a throwaway method-table probe over `tessera-wstate`. Read (class indices of this install): class 470 functions 5, 29, 44 (the player commands the executable calls, §3); the bodies behind slot 644, slot 486 and slots 137, 237, 269, 344, 426, 522 of the local player's class (1730 here, chain 512 … 787 … 484); class 295 functions 26 and 81 (the use dispatcher, read only to see what it is handed); class 404 function 2 (coin breakdown, already in `item-tooltips.md` §6.11); class 3126 function 16 (the player-variable filler of §5.5) with the helpers it calls. No message text was read. |
| **BIN** | **M4 under the standing authorisation of decision 0011.** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, SHA-256 prefix d22363eb9aff0fb5), headless decompiler on a private copy of the shared analysis project (`~/tessera-re/work/p3npc`, deleted after use), 2026-10-10; functions found by instruction search for element IDs, input-action IDs, state IDs and property IDs. **Questions:** (a) which input uses a world entity, and what is used; (b) what the loot window's buttons do and when it closes; (c) how the vendor window buys, sells and buys back, and what the quantity dialog does; (d) the order of the rows of the quest-offer reward list and how many objectives it shows; (e) how the money display builds its coin elements. **Why M1–M3 were not enough:** the layouts name the elements and the key maps the bindings, but which command a click on a world entity or a list entry produces, the buy modes, the clamps of the quantity dialog, the reward-row order and the close-on-empty rule are native UI code that no record states; no published source describes them; the running client is not available to this session (owner checks §14 cover the visible results). **Read:** the game-view command handlers (two variants) and the world-click dispatcher behind the full-screen click field of layout `selection`; the use, select and loot wrappers they call (only to read the class-library function IDs they invoke); the loot window's click handler, its Loot All, close and list-change routines and the Loot All label routine; the vendor buy-page, sell-page and buy-back-page click handlers, the buy routine, its mode selector, the toggle handler of the `VendorBuyUI` input map, the sell and sell-all routines (to their class-library calls), the quantity dialog's constructor, click handler, set-quantity and confirm routines and one float constant; the quest reward list's outer fill routine, its static-row routine and the five entry-kind routines (to the treasure properties they read), the objective-list routine; the money display's constructor and coin-element builder. **Revision 2 pass** (2026-10-11, same build, private copy `~/tessera-re/work/p3nw-r1`, deleted after use), for review 1's questions (c′) what the quantity dialog does with each character typed into its entry, with an empty entry at OK, at opening and with the OK button's state, and (d′) where the virtue experience row stands: **read** the dialog's message handler again (its character-input branch), its set-quantity, confirm and open routines, the text-to-number helper, the OK-state routine, the money display's amount refresh and the player-coin query it calls (to the class-library call), and the reward list's fixed-entry routine with its six callees (to the property IDs they read); one instruction search of the whole executable for the property ID of `QuestTreasure_FixedVirtueArray` (no hit). Why M1–M3 were not enough: as above; the layout names the entry and the OK button but no record states the input handling. **Nothing from network, login, launcher or anti-tamper code was read** (decision 0004): every path above was followed only up to the class-library call or the UI element it drives. |
| **Design** | Tessera rules, marked **Tessera rule**: what Tessera does where the original's behaviour is server data, untraced game code, or outside P3. Not claims about the original. |

| # | Fact | Source | Mark |
|---|---|---|---|
| NW1 | Window catalogue: layouts, `UILAYOUT` keys, roots, widget types, scaling groups, origins, design rects, size limits (§4) | OBS | Confirmed 2026-10-10 |
| NW2 | Conversation window elements, button fields, text-pane properties (§5.2, §6.1) | OBS | Confirmed 2026-10-10 |
| NW3 | Choice window (action picker) elements, templates and the 32-entry `ActionPickerUI_ActionEntryArray` with its conditions, texts and list templates; mapper `NPCActionPickerType` has 34 names and no display text (§5.3) | OBS | Confirmed 2026-10-10 |
| NW4 | NPC comment window elements; all 859 comment records hold `NPCComment_Title`, `NPCComment_WindowLabel`, `NPCComment_Content` and `NPCComment_CloseButtonText`; no comment content uses an argument (§5.4) | OBS | Confirmed 2026-10-10 |
| NW5 | Vendor window, its four pages, the entry templates, the stack overlay states, the quantity dialog (§7) | OBS | Confirmed 2026-10-10 |
| NW6 | Barter window and entry template; the list's entry template names layout 0x22000045, which holds no element of that ID, while the barter layout holds it at top level (§7.7) | OBS | Confirmed 2026-10-10 |
| NW7 | Loot window elements, background states, list display type (§8) | OBS | Confirmed 2026-10-10 |
| NW8 | Money display templates, coin template parts, coin images and options (§9.2) | OBS | Confirmed 2026-10-10 |
| NW9 | Wallet window elements; the wallet control record lists 610 items (167 + 443); every one has `PaperItem_Category`, `Name` and `Icon_Layer_ImageDID`; 577 have `PaperItem_SmallIcon`; category counts; the category mapper has 16 names and 14 display texts (§9.4) | OBS | Confirmed 2026-10-10 |
| NW10 | Default bindings: `USE` on keyboard U, `TogglePaperItemPanel` on B, `ToggleQuestPanel` on L (all `Main`, no modifier, activation 0x03); `VendorBuyUI`: `ToggleStackDisplay` on both Shift keys, `ToggleQuantityDisplay` on both Alt keys, `ToggleFillStackDisplay` on Tab, `SelectRight` on Shift + right button (0x06) (§3.1, §7.4) | OBS (key-map records; codes per `keymap.md` §12.6) | Confirmed 2026-10-10 |
| NW11 | `USE` makes the client call class-library function 0x01D60005 on the local player; that function uses the entity of the player's `Entity_Selection`, and with no selection shows a message instead (§3.1) | BIN (a); BC | BIN + Confirmed (bytecode) 2026-10-10 |
| NW12 | World clicks: the clicked entity must pass the selectable test; it becomes the selection; a left double-click then uses it unless it is the player; a right click or `AutoLootAll` on a non-player entity uses it, `AutoLootAll` setting `Usage_DoMetaUsage` first (§3.1) | BIN (a); BC | BIN 2026-10-10 |
| NW13 | Loot window: Loot All and Enter take everything, or close when the list is empty; the button's label follows the list; after Loot All the window closes when the list empties (§8.3) | BIN (b) | BIN 2026-10-10 |
| NW14 | Vendor buying: right click and left double-click on a buy entry, or the Buy button, buy the selected entry; four buy modes by held toggle; items with a stack size below 2 always buy one (§7.4) | BIN (c) | BIN 2026-10-10 |
| NW15 | Quantity dialog: presets, arrows, clamp to 1 … stack size, OK and Enter, cancel; the half preset multiplies by 0.5 (§7.5). Revision 2: every character input re-reads the entry and sets it (clamp and rewrite), an empty entry keeps the quantity and makes OK do nothing, the number reading, the `TooExpensive` check at OK, the OK button's `ghosted`/`normal` choice, the opening quantity (*M*, or the last purchase's quantity for the same vendor), the money display's own coin comparison; message key 0x28 is `Text_character_input` in the install's `UIElementMessage` mapper (base 0x230001AC) and state key 0x10000013 is `TooExpensive` in `UIStateID` | BIN (c), (c′); OBS (mapper labels) | BIN 2026-10-10, revised 2026-10-11 |
| NW16 | Selling and buying back: entry clicks, Sell, Sell All (every listed item that is not locked), the show-locked check box; buy-back entry clicks and button (§7.6, §7.8) | BIN (c) | BIN 2026-10-10 |
| NW17 | Quest-offer reward list: row order, kinds, headers; the objective list shows one objective entry (§6.2, §6.3). Revision 2: the virtue experience row is the last row of the fixed-entry step, conditioned on the treasure's `QuestTreasure_Virtue_XP_Tier`; no list routine reads `QuestTreasure_FixedVirtueArray` | BIN (d), (d′); OBS (property and element labels of the IDs) | BIN 2026-10-10, revised 2026-10-11 |
| NW18 | The money display builds exactly three coin elements with icon states `Copper`, `Silver`, `Gold`, hidden until filled (§9.2) | BIN (e) | BIN 2026-10-10 |
| NW19 | Currency directory: the object form lists gold, silver, copper; the property set stores silver, gold, copper (§9.3) | OBS (both forms); BC (class 404 walks the object form) | Confirmed 2026-10-10 |
| NW20 | Player-variable filler: 20 argument IDs and the player value each receives (§5.5); role-line census of argument IDs (§5.5, §11) | BC (class 3126 function 16 and its helpers); OBS (argument IDs) | Confirmed (bytecode, census) 2026-10-10 |
| NW21 | Barter costs: 44,856 of the 58,554 cost structs with an item name one of the 610 wallet items (423 distinct wallet items; 3,081 distinct cost items in all) (§9.5) | OBS | Confirmed 2026-10-10 |
| NW22 | All 10,579 distinct items of the 400 vendor inventories have `Name`, a non-null icon layer and `Item_Value`; 2,780 are stackable (§11) | OBS | Confirmed 2026-10-10 |
| NW23 | Quest census for the offer window (§11) | OBS | Confirmed 2026-10-10 |
| NW24 | Every rule marked **Tessera rule** | Design | Decision |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Feed window** | A window of the scenario feed (`scenario-feed.md` §5.6): `dialogue`, `quest-offer`, `vendor`, `loot` (and the kinds of other milestones), identified by `WindowId`. |
| **Native window** | A root panel built from an install layout (`ui-layout.md` §9.1, §17.1). One feed window is shown by one native window instance. |
| **Conversation window** | The native window `RootQuestField` of layout `questdispenser` (§5.2, §6). |
| **Choice window** | The native window `RootActionPickerField` of layout `npc_actionpicker` (§5.3). |
| **Window entity** | The entity a feed window belongs to: the dialogue speaker, the quest giver, the vendor or the loot source (each optional in the feed). |
| **Selection** | The local player's `Entity_Selection`: the selected entity. Picking and the selection model are P2's; this spec only reads them. |
| **Wallet item** | An item listed in the wallet control record (`PaperItemControl`, §9.4). |
| **Coin amount** | An amount in units of the smallest denomination (`Currency_Value` 1, `economy.md` §7). |
| **Unit price** | A feed entry's `Price` (`scenario-feed.md` §9.1) for one unit. **Tessera rule**: vendor and buy-back prices are per unit; the client multiplies by the quantity only for display (§7.5). |

---

<a id="s3"></a>
## 3. Using an NPC (client side)

<a id="s3-1"></a>
### 3.1 Which input uses an entity (BIN, BC; NW10–NW12)

**Keyboard.** The input action `USE` (InputActions key 0x10000085 here; default binding:
keyboard U in key map `Main`, no modifier, activation 0x03) makes the client call
class-library function 0x01D60005 (class 470 function 5 here) on the local player. That
function reads the player's `Entity_Selection` (property 0x34F): with no selection it shows
a system message (StringInfo of the `Usage` table, key 0x10000037 of `STRINGTABLE`, token
0x0A34ED42) and does nothing else; otherwise it hands the selected entity to the class
library's use dispatcher (class 295 here; it reads the entity's usage action, `interaction.md`
§3.4.4, and makes further checks that were not traced). Both game-view command handlers (the
normal and an alternative game view) treat `USE` the same way.

**Mouse.** Clicks on the world reach the full-screen field `ItemSelectionField` of layout
`selection` (0x22000013). On a mouse press or tap message carrying one of the commands
`TapLeft`, `SelectLeft`, `SelectDblLeft`, `SelectRight` or `AutoLootAll`, the client takes
the entity under the pointer (P2's picking; when the clicked object is not an entity the
client knows but another object, the entity named by that object's `MeshFXScript_SourceID` is
taken) and:

1. With no entity under the pointer: `SelectRight`, `SelectDblLeft` and `AutoLootAll` do
   nothing; `TapLeft` and `SelectLeft` clear the selection.
2. The entity must pass a test of the mouse-over controller (class-library function
   0x01150004, class 277 here; by inference the selectability of `interaction.md` §3.4.4
   step 1); otherwise nothing happens.
3. The entity becomes the selection (class-library function 0x01D60001); when the client
   resolves the entity to another one first (a lookup that was not traced; by inference a
   parent of `interaction.md` §3.4.2), that one is selected.
4. `SelectDblLeft`: unless the selected entity is the local player, the client then uses the
   selection exactly as `USE` does.
5. `SelectRight` and `AutoLootAll`: when the entity's `WeenieType` is not a player type (its
   five low bits are not all set), the client uses the selection as `USE` does, unless it is
   the local player; for `AutoLootAll` it first sets the player's `Usage_DoMetaUsage` to
   true. For player entities the right click leads to player-to-player handling (dueling and
   alignment checks, a menu), which belongs to P4. A check that can stop a right click on an
   entity other than the current selection was not traced.

Default mouse bindings (`keymap.md` K42, K46): `SelectLeft` left button, `SelectDblLeft` left
double-click, `TapLeft` left tap, `SelectRight` right button (tap), `AutoLootAll` Shift +
right button.

> **OPEN:** (Q1) the texts and conditions of the no-selection and out-of-range messages and the
> dispatcher's further checks (class 295 function 81 was not read in full); owner check O110
> for the mouse inputs.

> **Tessera rule (P3 client):** `USE`, a left double-click and a right click on a non-player
> entity use the selection as above. Using means: the client checks the use range of
> `interaction.md` §3.4.1 (gap distance at most *R*); in range it sends
> `Command::Npc(UseEntity { entity })` (`scenario-feed.md` §9.4); out of range it posts a
> system chat line and sends nothing. With no selection it posts a system line. The texts of
> these lines are the `Usage` table entries the class library would show; until their tokens
> are traced (§14 Q1) Tessera posts its own short texts. `AutoLootAll` additionally marks the
> next loot window from that entity for Loot All (§8.3).

> **OPEN:** (implementer, `apps/client`, 2026-10-10) two client facts the section leaves to
> P2 are implemented provisionally: (1) until P2's picking supplies setup-based cylinders
> (§3.4.1 I6a), both cylinders of the range test are Decision D13's (radius 0.05 m, height
> 0.1 m, no offset) and *R* comes from the body record's `Usage_MinUsageDist` (D14
> otherwise); (2) without P2's selection the client offers a Tessera input action "use
> nearest NPC in range" on keyboard U (the install's `USE` default, NW10): the nearest
> entity that is not a player and whose gap distance is within its *R* (ties to the lower
> entity ID) is used through the same entry point. Please confirm or replace when the
> selection exists.

<a id="s3-2"></a>
### 3.2 What use needs from picking and selection (P2)

| Need | From |
|---|---|
| The entity under the pointer, skipping the local player and its shapeshift entity | P2 picking, `interaction.md` §3.4.2 hover rule |
| The selection (`Entity_Selection`), set and cleared by the clicks of §3.1 | P2 selection model |
| The selectable test | `interaction.md` §3.4.4 step 1 |
| Cursor and out-of-range cursor while hovering | `interaction.md` §3.4.2, §3.4.6 (P2 draws it; `Mouseover_Range` from the hovered entity) |
| The use range *R* and the gap distance | `interaction.md` §3.4.1 |

The client never decides what a use does: the scenario feed answers `UseEntity` with a
trigger (`scenario-feed.md` §7.5 `use ENTITY`), a server with its own events.

<a id="s3-3"></a>
### 3.3 What the windows show after a use

Every NPC window opens only on a feed or server event (`DialogueShown`, `QuestOffered`,
`VendorShown`, `LootShown`); a use never opens a window by itself. The NPC service rule of
`interaction.md` §3.4.5 decides only the cursor.

---

<a id="s4"></a>
## 4. Window catalogue (Confirmed, NW1)

Opening follows `ui-layout.md` §17.1: find the layout by its `UILAYOUT` label, the root by
its `UIElementID` label.

| Native window | `UILAYOUT` label (key here) | Layout here | Root (widget type) | Scaling group | Origin | Design position, size | Other root properties |
|---|---|---|---|---|---|---|---|
| Conversation | `questdispenser` (0x1000004F) | 0x22000025 | `RootQuestField` (`UIScalerPanel`) | `QuestBestow` (58) | (334, 235) | (12, 270), 669 × 470 | min 669 × 470; saves location and size; activatable, activates on show |
| Choice | `npc_actionpicker` (0x1000006E) | 0x22000026 | `RootActionPickerField`, based on `Panel_Template` of layout 0x22000846 (`UIScalerPanel`) | `QuestBestow` (58) | (206, 150) | (15, 120), 412 × 472 | min 200 × 350, max 1024 × 768; saves location and size |
| NPC comment | `npccomment` (0x100000AC) | 0x220006F5 | `NPCComment_Window` (`Field`) | — | — | (10, 118), 484 × 488 | saves location; auto-fade 0.1 |
| Vendor | `vendor` (0x10000010) | 0x2200001E | `Vendor_Window` (`UIScaler`) | `Panels` (31) | (206, 244) | (10, 116), 412 × 488 | saves location; auto-fade 0.1; object mode Stretch |
| Quantity dialog | `vendor_buy_stack` (0x100000BA) | 0x220007BB | `VendorBuyStackField` (`UIScaler`) | `Panels` (31) | (147, 22) | (308, 300), 295 × 45 | min 256 × 164; z −500 |
| Barter | `barter` (0x10000084) | 0x22000191 | `Barter_Window` (`UIScaler`) | `Barter` (53) | (323, 448) | (10, 116), 646 × 496 | `UI_Barter_PageSize` 20; saves location; auto-fade 0.1 |
| Loot | `externalcontainer` (0x10000003) | 0x2200001B | `ExternalContainerField` (`UIScaler`) | `Panels` (31) | (132, 109) | (374, 231), 265 × 219 | object mode CanvasResize; saves location; auto-fade 0.1 |
| Wallet | `paperitem` (0x100000CE) | 0x22000840 | `PaperItemPanel_Window_V2` (`UIScaler`) | `Wallet` (38) | (225, 249) | (151, 118), 449 × 499 | min 449 × 255, max 449 × 700; saves location and size |

Shared parts used by these windows: the money templates of layout `moneydisplay` (§9.2), the
item displays of layout `itemdisplay` (`character-panels.md` §5), the vendor pages of layouts
0x22000092 (buy-back), 0x22000093 (buy) and 0x22000094 (sell), which no `UILAYOUT` label names
(they are reached as bases), the list templates of layout `vendor_listbox`, the vendor entry
templates of layout `vendor_item`, the quest lists of layouts `questobjectives` (0x22000842)
and `questrewardslist` (0x22000844) with the reward row bases of layout 0x22000720.

**Tessera rule (`--open-panel` names):** `conversation`, `choice`, `npc-comment`, `vendor`,
`vendor-quantity`, `barter`, `loot`, `wallet` open the windows above empty, for golden images
and inspection.

**Tessera rule (instances):** each feed window is shown by its own native-window instance;
when a dialogue page and a quest offer use the conversation window at the same time, each
has its own instance and the later one is drawn above.

> **OPEN:** (P3 exit verifier, 2026-10-11) the quantity dialog's root `VendorBuyStackField`,
> laid out by its edges from the layout's design size (`ui-layout.md` §8.4) on a 1600 × 900
> screen, comes out 294 × −87 in Tessera; its children are placed for a height of about 164.
> Whether the original applies the root's minimum (256 × 164) at opening is not stated.
> Tessera rule until settled: after opening, a root below its minimum size (or empty) is
> resized through the element resize of `ui-behaviour.md` §9.2.1 (limit pass, UB47) and kept
> on screen. The barter window's root likewise lands at x = −107 on that screen (design
> position (10, 116), origin (323, 448)); how the original places it is not stated either.

> **OPEN:** (implementer, `tessera-ui` and `apps/client`, 2026-10-11) Tessera rule until the
> original's placement is known: every root of this catalogue, once opened (and again after
> a replaced dialogue instance is centred on the old one, §5.1), is clamped into the screen
> the way the move step of `ui-behaviour.md` §8.1 step 3 clamps a dragged panel: its
> displayed box, left edge then right edge, top edge then bottom edge (a root larger than
> the screen ends with its right and bottom edges on the screen's); its size is kept. The
> barter window then opens at x = 0 on a 1,600-pixel screen. Saved locations are not read
> by these windows yet.

---

<a id="s5"></a>
## 5. Dialogue

<a id="s5-1"></a>
### 5.1 Which native window shows a dialogue page (Tessera rule)

The original's choice of window for an NPC conversation is server-driven and was not traced.
> **OPEN:** (Q2) which native window the original uses for which conversation; owner check
> O111.

**Tessera rule:** for each `DialogueShown { window, speaker, text, options }`:

1. `text` is `Text::Comment(did)` and `options` is empty → the **NPC comment window** (§5.4).
2. `options` is empty → the **conversation window** with only the Continue button (§5.2).
3. Otherwise → the **choice window** (§5.3).

A later `DialogueShown` for the same `WindowId` replaces the content; when the rule picks a
different native window, the old instance is hidden and the new one shown at the old one's
position (centre to centre). `WindowClosed` hides it.

<a id="s5-2"></a>
### 5.2 Conversation window: a text page (NW2)

Parts of `RootQuestField` (positions in the root):

| Element | Data | Role for a text page |
|---|---|---|
| `RootQuest_BaseBox_TitleBar` → `QuestDispenser_Label` | title bar text, centred, truncated with an automatic tooltip | the speaker's name (empty when there is no speaker) |
| `MoveBar` | drag bar with a tooltip | moves the window |
| `QuestCloseButton` | `Button_X` at (645, 20) | sends `CloseWindow` |
| `Quest_QuickInfoField` | level, group-type and repeatable fields (§6.1) | hidden |
| `RootQuest_TextPane` | 321 × 365 at (15, 60); holds `QuestDispenserIntroText` (NarrativeFont, colour 0xFFC0C0C0, top margin 15, markup on, always scrolls to the top, a font list of seven fonts) and `QuestText_Scrollbar` | the page text |
| `RootQuest_Mission_TextPane`, `RootQuest_MissionTwistPane` | mission texts | hidden |
| `RootQuest_ObjectivesRewardsField` | objective and reward list (§6.2) | hidden |
| `RootQuest_ButtonsField` | holds the button fields, each based on a top-level `…Reference` element of the same layout: `AcceptDeclineField`, `NowLaterField`, `TeleportField`, `ContinueField`, `CommentField`, `OpenUIMenuField`, `OpenInstanceUIField`; the filter toggles; the font-size arrows `Quest_Bestow_IncreaseFontSize` / `…DecreaseFontSize` with the sample `Quest_Bestow_FontExample` | only `ContinueField` (`ContinueButton`) is shown; Continue sends `CloseWindow` |
| `ResizeBarBottom`, `ResizeBarRight`, `ResizeBarBottomRight` | resize bars | resize (`ui-behaviour.md` §8.2) |

The layout text entries of the text elements (table 0x25033E35) are design-time placeholders;
the client always replaces them.

> **Tessera rule (font size):** the two arrows step through the text pane's font list (seven
> fonts, `UICore_Text_fonts`), one step per click, clamped at both ends; the choice applies to
> `QuestDispenserIntroText`, `QuestDispenserMissionText` and the sample, and is kept in
> Tessera's settings.

> **OPEN:** (implementer, `apps/client`, 2026-10-11) `tessera-settings` has no key for this
> choice yet, so the client keeps it for the session only (every dialogue window opened later
> in the run uses it; a restart forgets it). Needs a settings key owned by `crates/settings`.

<a id="s5-3"></a>
### 5.3 Choice window: text with options (NW3)

`RootActionPickerField` takes from `Panel_Template` (layout 0x22000846) the background, the
title bar with `Panel_Label_Title`, the close button `Panel_Button_Close`, the drag bar and
eight resize bars. Its own content is `ActionPickerField` with the list `ActionPickerListBox`
(380 × 385 at (7, 18), vertical scroll bar `ActionPickerListBox_Scrollbar`).

Templates of the layout used for the list rows:

| Template | Widget | Content |
|---|---|---|
| `ActionPickerGreetingTemplateText` | text, NarrativeFont, 375 wide, wraps, bottom margin 4 | a greeting line |
| `ActionPickerIntroText` | text, NarrativeFont, colour 0xFFC0C0C0 | an intro line |
| Section headers `ActionPicker…Header` (quests available, current, pending, resettable, unlockable, NPC comments, quest comments, other actions, private encounters) | fieldlabelmedium text, colour 0xFFFFF600, each with its own text of table 0x25033EC7 | section titles |
| `ActionPickerMiscTemplate` | `Button`, 375 × 33, text left margin 38, colour 0xFFF5DF92, outline, truncated with tooltip, state `requirement_notmet` (colour 0xFFBE2525); child `ActionPickerTemplate_Icon` 26 × 24 at (7, 6) | an action row; the vendor, barter, banker, trainer … templates are based on it and only change the icon |
| `ActionPickerQuestTemplate` and the five quest templates based on it (available, current, failed, unlockable, session), `ActionPickerPendingQuestTemplate`, `ActionPickerResettableQuestTemplate` | `Button` rows with icon, text, timers | quest rows |
| `ActionPickerNPCCommentTemplate` | `Button` 375 × 30 with a bullet `LotroBullet_Quest` | comment rows |

The root carries `ActionPickerUI_ActionEntryArray`, 32 entries here. Each entry names an
action (`ActionPickerUI_Action`, a key of the mapper `NPCActionPickerType`, for example
`Vendor` 2, `Barter` 14, `Quest` 9, `NPCComment` 23), the row template
(`ActionPickerUI_ListEntryTemplate`, or per-kind template arrays for quests, crafting and
trainers), the row text (`ActionPickerUI_EntryTextProperty`: a property of the NPC that may
hold its own text, for example `NPC_ActionPicker_VendorString`; else
`ActionPickerUI_DefaultTextEntry`, a StringInfo of table `UI_NPC_ActionPicker` 0x250001E4
here), a condition on the NPC (`ActionPickerUI_NPCMustContainProperty` or
`ActionPickerUI_NPCPropertyMustBeTrue`), the class-library script property to run and the
property handed to it. How the original assembles the list from these entries for an NPC is
server-driven (which actions the NPC offers) and was not traced; the entries are loaded into a
table keyed by action when the window is built (BIN).

> **OPEN:** (Q4) how the original fills the choice list from the action entries (sections,
> headers, order, `requirement_notmet`); it depends on what the server sends. Owner check O111.

> **Tessera rule (choice page):** for `DialogueShown` with options:
>
> 1. `Panel_Label_Title` shows the speaker's name (empty without a speaker).
> 2. The first list row is an instance of `ActionPickerGreetingTemplateText` showing the page
>    text (wrapping, markup per §5.5).
> 3. One row per option, in ascending option number, each an instance of
>    `ActionPickerMiscTemplate` with the option text. When the option's text is
>    `Text::Action(a)` (scenario-feed revision 5, §10.2) the row uses the action entry whose
>    `ActionPickerUI_Action` is *a*: its `ActionPickerUI_ListEntryTemplate` (the icon), and
>    as text the speaker record's value of the entry's `ActionPickerUI_EntryTextProperty` when
>    the speaker's body record has one, else the entry's `ActionPickerUI_DefaultTextEntry`.
>    An entry without a single template (quests, crafting, trainers) uses
>    `ActionPickerMiscTemplate`.
> 4. Option numbers are not drawn. Clicking a row (`SelectLeft` release on it, or `Enter`
>    while it has focus) sends `ChooseOption { window, option }`. No row is in state
>    `requirement_notmet`.
> 5. More rows than fit scroll with the list's scroll bar (`ui-widgets.md` §5); the window
>    keeps its size.

> **OPEN:** (implementer, `apps/client`, 2026-10-10) an action entry with neither a value
> of its `ActionPickerUI_EntryTextProperty` on the speaker's record nor an
> `ActionPickerUI_DefaultTextEntry` (on this install the `Quest` action) gives no text:
> the client shows the row with an empty text and the census counts `TextMissing`. Please
> state the row's text for such actions (for example the quest's name).

> **OPEN:** (implementer, `apps/client`, 2026-10-11) rule 4's `Enter` on a focused row is not
> wired: `tessera-ui`'s list rows take no keyboard focus, so no row can have it. Enter on the
> choice window goes to the rest of the UI as before. Which row has focus when the window opens
> (none, or the first option)?

> **OPEN:** (implementer, `tessera-ui`, 2026-10-11) the table gives `ActionPickerMiscTemplate`
> as "truncated with tooltip" but not whether its text wraps; on this install a long option
> wrapped to a second line that the 33-pixel row clipped. Tessera rule until settled: an
> option row's text element gets `UICore_Text_one_line` true, so a long option is one line
> cut with the template's truncation suffix and its automatic tooltip (`fonts.md` §7.6,
> `ui-behaviour.md` §10.2). The greeting row keeps wrapping.

<a id="s5-4"></a>
### 5.4 NPC comment window (NW4)

| Element | Data | Fill (**Tessera rule** by element and property names) |
|---|---|---|
| `NPCComment_TitleTextField` | titlebarfont text at (10, 29) | `NPCComment_Title` of the comment record |
| `NPCComment_TitleBar` → `NPCComment_WindowLabel` | title bar text | `NPCComment_WindowLabel` |
| `NPCCommentList_Container` → `NPCComment_ContentText` | NarrativeFont text 452 × 375 at (10, 51), markup on, top-justified, scroll bar `NPCComment_ContentScrollbar` | `NPCComment_Content` |
| `NPCComment_CloseTextButton` | text button 130 × 20 at (201, 453) | text `NPCComment_CloseButtonText`; sends `CloseWindow` |
| `NPCComment_CloseButton` | `Button_X` | sends `CloseWindow` |
| `NPCComment_MoveBar` | drag bar | moves the window |

All 859 comment records hold the four texts (NW4).

<a id="s5-5"></a>
### 5.5 Rendering dialogue text

Every text of these windows is rendered by `string-table.md`: StringInfo resolution (§9),
display mode with the display cut (§7.6), escapes (§7.7), and markup per element (§7.8): the
elements with `UICore_Text_parse_markup_tags` true (`QuestDispenserIntroText`,
`QuestDispenserMissionText`, `QuestDispenserTwistDescriptionText`, `NPCComment_ContentText`,
`QuestObjectives_Objective`) interpret the §4.7 markup; the action rows and titles apply only
the `<<>` literal and link steps.

**Player variables (BC, NW20).** The class library has a filler (class 3126 function 16 here)
that walks the argument IDs of a StringInfo and sets, for each ID it knows, a value taken from
the local player; IDs it does not know are left unset. Its 20 IDs:

| Argument ID | Value set |
|---|---|
| 0x00052615, 0x03EC29B5, 0x05506DA2, 0x0DA47675 | the player's `Name` as text |
| 0x03EC68D5 | the species name: `Character_SpeciesNameOverride` when set, else the display text of `Agent_Species` |
| 0x00480683, 0x0EB8DCB3 | the display text of `Agent_Class` |
| 0x00056575 | `Agent_Species` as an enum argument (value with its mapper, so grammar tags can select) |
| 0x000057A8, 0x0A3EC778 | `Sex` as an enum argument |
| 0x0050AA9C | the player's level as an integer (class 3697 function 2) |
| 0x08A72645 | the surname (`Surname`) when the option `GOP_ShowSurnameTitle` is on; otherwise a default message of the class library (not read) |
| 0x0005662B | a rank title composed with the player's name when the option `GOP_ShowRankTitle` is on; otherwise the same default message |
| 0x047968D1, 0x047968D2, 0x047CE071, 0x047CE072, 0x0543E294, 0x05466A34, 0x05854A75 | the player's title of category 2, 3, 4, 5, 6, 7 and 1 respectively; the default message when the player has none |

Which callers run the filler for dialogue lines was not traced (it is called from four bodies
of classes 1405, 1470 and 1477 here). Census (NW20): of the 57,534 string-table entries named
by role-line properties of quest records, 15,993 use arguments; the IDs used are 0x03EC29B5
(14,575 entries), 0x0EB8DCB3 (881), 0x03EC68D5 (761), 0x05506DA2 (115), 0x0059485C (3),
0x053A16A2 (3), 0x0A3724C5 (2), 0x00005343 (1), 0x00052615 (1), 0x00480683 (1). All but four
IDs (9 uses) are in the filler's table. No NPC comment content uses an argument.

> **OPEN:** (Q12) which callers apply the filler to dialogue lines, and the values of the four
> argument IDs outside its table.

> **OPEN:** (implementer, 2026-10-10) the table gives 0x00056575 (species) and 0x000057A8,
> 0x0A3EC778 (sex) as **enum arguments** ("value with its mapper, so grammar tags can
> select"), but `string-table.md` defines no enum value kind for slot values (`tessera-text`
> fills slots with text, integers, floats and nested messages). `tessera-gamedata` supplies the
> enum's display text as plain text for these three IDs, so grammar selectors keyed on the
> mapper's tags do not see them. Please state how an enum argument renders (its display
> StringInfo as a nested message, a tag group, or another form).

> **Tessera rule (variables):** before rendering a dialogue line (page text, option text,
> role line, objective, quest name) the client supplies the values of the table above from
> the local character (the entered character's name; species, sex and class from the latest
> `CharacterInfo`; level from the latest `LevelChanged` or `CharacterInfo`; surname, rank and
> titles empty, since the sandbox has none). A line that still lacks a value renders by
> `string-table.md` §8.3 client mode (render stops with reason 6), and the element shows an
> empty text; the census of §11 counts it.

<a id="s5-6"></a>
### 5.6 Where dialogue text comes from in the install

A scenario names install text through the feed's `Text` forms (`scenario-feed.md` §5.2), and
the client resolves them as follows (`quests.md` §7):

| `Text` | Resolved to |
|---|---|
| `Name(did)` | the record's display name (`Quest_Name` for a quest, `Name` for an item or NPC, by family) |
| `Description(did)` | the record's description (`Quest_Description`, `Description`) |
| `QuestRole { quest, role, failure }` | the success or failure line of that role of `Quest_RoleArray`, with the species-variant rule of `quests.md` §7.1 |
| `Comment(did)` | `NPCComment_Content` of the comment record (and §5.1 picks the comment window) |
| `Objective { quest, objective }` | `Quest_ObjectiveDescription` of the objective with that `Quest_ObjectiveIndex` |
| `Action(a)` (revision 5) | the action entry's text of §5.3 rule 3 |
| `Literal(s)` | the string, rendered as a literal (no grammar), with markup per element |

Where NPCs' lines live (`quests.md` §7.6): offers are action-6 roles; talk and comment lines
during objectives are action-2 and action-1 roles; completion comments are in
`Quest_CompletionCommentArray`; free-standing comments are comment records reached through an
NPC's `NPCComment_DIDCommentArray` (`quests.md` §7.7). Selectors (`scenario-feed.md` §8.3)
reach quests (`quest`) and comments (`npc-comment`); `role($q[i],N)`, `comment($c[i])` and
`objective($q[i],N)` pick the line.

<a id="s5-7"></a>
### 5.7 Closing

| Event or input | Effect |
|---|---|
| `WindowClosed { window }` | hide the instance |
| Close button, Continue, the comment's close button | send `CloseWindow { window }` |
| `EscapeKey` while the window has focus | send `CloseWindow` (window manager, `ui-behaviour.md` §8) |
| Window entity despawned (`EntityDespawned`) | **Tessera rule:** send `CloseWindow` once |
| The player moves away | **Tessera rule:** when the gap distance to the window entity exceeds the use range *R* of `interaction.md` §3.4.1, send `CloseWindow` once (the original's rule is OPEN, §14 Q3) |

The client never hides a feed window before `WindowClosed` arrives.

> **OPEN:** (Q3) whether and at which distance the original closes NPC windows when the player
> walks away; owner check O111.

---

<a id="s6"></a>
## 6. Quest-offer window

<a id="s6-1"></a>
### 6.1 Layout and header (NW2)

`QuestOffered { window, quest, giver }` opens an instance of the conversation window
(`RootQuestField`) with:

| Element | Fill (**Tessera rule** where marked) |
|---|---|
| `QuestDispenser_Label` | `Quest_Name` (for `Ref::Literal`, the literal) |
| `Quest_QuickInfoField`: `Quest_LevelLabel` (fixed text) and `Quest_LevelText` | the effective challenge level of `quests.md` §4.3 as an integer; the level field is hidden when `Quest_HideChallengeLevel` is true (**Tessera rule**) |
| `Quest_SoloField`, `Quest_SmallFellowshipField` (icon 0x41008127), `Quest_FellowshipField` (icon 0x41008125), `Quest_RaidField` (icon 0x41008126) with `Quest_GroupTypeLabel` | **Tessera rule:** exactly one shown: raid when `Quest_IsRaidQuest`, else fellowship when `Quest_IsFellowshipRecommended`, else small fellowship when `Quest_IsSmallFellowshipRecommended`, else solo |
| `RepeatableImageField` (icon 0x4100812A, tooltip) | **Tessera rule:** shown when `Quest_MaxTimesCompletable` (default 1) is not 1 |
| `QuestDispenserIntroText` | the offer line (§6.4) |
| `RootQuest_ObjectivesRewardsField` → `Quest_ObjectivesRewardsList` (list box with scroll bar `Quest_ObjectivesRewardsScrollbar`) | the objective block, then the reward block (§6.2, §6.3) |
| `RootQuest_ButtonsField` | only `AcceptDeclineField` shown: `QuestAcceptButton` and `QuestDeclineButton`; its store buttons (`QuestIncreaseSlotsButton`, `QuestBuyNowButton`) hidden |

<a id="s6-2"></a>
### 6.2 Objective block (BIN, NW17)

The block is an instance of `Quest_ObjectivesField` (based on `QuestObjectives_Root`, widget
`QuestObjectives`, an auto-resizing list). The original adds one objective entry: either the
objective text alone, followed by the quest-token row `QuestObjectives_TokenDisplay` (an item
display) when the quest has one, or the header `QuestObjectives_Header` (text of table
`UI_Quest` 0x250001E3) followed by one entry `QuestObjectives_Objective` (NarrativeFont, colour
0xFFFFF600, markup on, wraps, bullet image 0x41007E02). Which objective text the class library
hands it was not traced.

> **OPEN:** (Q5) which objective text the class library hands the objective list; owner check
> O112.

> **Tessera rule:** the header, then one entry with `Quest_ObjectiveDescription` of the
> objective whose `Quest_ObjectiveIndex` is lowest; no entry (and no header) when that
> objective has no description or the quest is a literal ref. No token row.

> **OPEN:** (implementer, `tessera-ui`, 2026-10-11) how the auto-resizing objective list sizes
> its entry is not stated; created from the template, `QuestObjectives_Objective` kept its
> design width, wider than `Quest_ObjectivesRewardsList`, and its text ran past the pane's
> right edge. Tessera rule until settled (as `quest-journal.md` §4.7 sizes the details pane's
> elements): the entry is resized to the list's width (a text inside it to the list's width
> less its left offset), wraps (`UICore_Text_one_line` false), and its height follows its
> wrapped text (the row grows by the text's change of height).

<a id="s6-3"></a>
### 6.3 Reward block (BIN, NW17)

The block is an instance of `Quest_RewardsField` (based on `QuestRewards_Root`, widget
`QuestRewardsList`, holding the list `QuestRewards_List`). The original fills it in this order
(each row only when it has something to show):

1. **Amount rows**, from a reward-information structure the class library returns for the
   quest (class-library function 0x011F00C1): the money row `QuestRewards_MoneyRewardText`
   (a money display, §9.2), the PVP currency row `QuestRewards_PVPField` (a wallet-item
   display of the item `UI_PaperItemEntry_ItemDID`), the reputation rows
   (`QuestRewards_ReputationField`, state `reward_positivereward` for the positive faction
   with icon 0x410D705B, then `reward_negativereward` with icon 0x410D705C), glory
   (`QuestRewards_GloryField`), item experience (`QuestRewards_ItemExpField`), craft
   experience (`QuestRewards_CraftExpField`), mount experience (`QuestRewards_MountExpField`),
   wallet-item rows (`QuestRewards_PaperItemField`, one per wallet item), class trait points
   (`QuestRewards_ClassTraitPointsField`), then one of the store-point and mithril rows; then
   billing-token rows (`QuestRewards_BillingTokenField`).
2. **Fixed reward entries** (`QuestRewards_StaticRewardField`, based on `QuestReward_Base` of
   layout 0x22000720), by kind in this order: items (`QuestTreasure_FixedItemArray`),
   relics, traits (`QuestTreasure_FixedTraitArray`), titles
   (`QuestTreasure_FixedTitleArray`), emotes (`QuestTreasure_FixedEmoteArray`); then, as the
   last row of this step, the **virtue experience row** `QuestRewards_VirtueExpField`, added
   when the quest's treasure record has a non-zero `QuestTreasure_Virtue_XP_Tier`. That row
   is not a `QuestReward_Base` entry: it has the shape of the amount rows of step 1
   (`QuestRewards_VirtueExpField_BG`, `QuestRewards_VirtueExpIcon`,
   `QuestRewards_VirtueExpRewardText`), and its text is a formatted string with the amount
   the class library reports for the quest. The list fill reads no
   `QuestTreasure_FixedVirtueArray`: the five entry-kind routines read only the item, relic, trait, title and emote arrays, and the virtue row reads
   only the tier (revision 2, BIN 2026-10-11).
3. **Selectable rewards**, when the quest has any: the header
   `QuestRewards_SelectableRewardsHeader`, then entries (`QuestRewards_SelectableRewardField`,
   based on `QuestReward_Selectable`) by kind in the order items, relics, traits, titles,
   emotes (the `…Selectable…Array` properties).
4. When any row was added, the header `QuestRewards_Header` (table `UI_Quest`) is put at the
   top of the block.

Every reward row of kinds 1 shares the shape `…_BG` (`box_quest_reward`), a 32 × 32 icon at
about (12, 4) and a text 319 × 33 at (45, 4); entries of kinds 2–3 use the parts of
`QuestReward_Base`: `QuestReward_Item` (a medium item display), `QuestReward_Trait` (a
`TraitDisplay` with icon, name and overlays), `QuestReward_Image` with `QuestReward_Text`, and
`QuestReward_BillingToken`.

> **OPEN:** (Q5) the texts of title, emote and virtue experience rows and the amounts'
> formats; whether the original shows fixed virtue-rank rewards anywhere else in the offer;
> owner check O112.

> **Tessera rule (fill):**
>
> - Entries of steps 2–3 come from the quest's treasure record (`Quest_QuestTreasureDID`,
>   `quests.md` §9.4): items as item displays with the composed icon and a quantity overlay
>   for `QuestTreasure_ItemQuantity` > 1, and the item tooltip on hover; traits as trait
>   displays (icon and name of the trait record, `icon-layers.md` §5.3); titles and emotes as
>   image and text (`Title_String`, `Emote_CommandString`; no image). Entries of
>   `QuestTreasure_FixedVirtueArray` are **not** shown (as in the original's list, step 2).
>   Relics never occur here (`quests.md` §9.4).
> - The virtue experience row of step 2 is shown only with a `VirtueXp` amount the feed
>   supplies (§10.3), as the last row of step 2: after the fixed emote entries, before the
>   selectable rewards. Its text element `QuestRewards_VirtueExpRewardText` shows the amount
>   as a decimal integer. The row does not depend on the treasure record's
>   `QuestTreasure_Virtue_XP_Tier` (the tier is a reward-level key, server data).
> - Amount rows of step 1 are shown only with amounts the feed supplies (`QuestRewards`,
>   revision 5, §10.3): the client never derives an amount from reward-level tables
>   (`quests.md` §9.1–§9.2 are server data). A reputation row names the faction of the
>   feed entry (`Ref`), with the faction's name text and the amount.
> - Texts of amount rows: the row's own text element shows the amount as a decimal integer;
>   the money row shows coins (§9.2).

> **OPEN:** (P3 exit verifier, 2026-10-11) `QuestReward_Base` on this install has a sixth
> part, `QuestReward_Runic` (a 32 × 32 display), besides the five named above; and which parts
> an entry of each kind shows is not stated. Every part except the background carries the
> layout's sample text. Tessera rule until settled: an item entry shows only
> `QuestReward_Item`; a trait entry only `QuestReward_Trait` (its name in the trait display's
> text); title and emote entries `QuestReward_Image` and `QuestReward_Text`;
> `QuestReward_BillingToken` and `QuestReward_Runic` stay hidden.

<a id="s6-4"></a>
### 6.4 The offer line (Tessera rule)

The text pane shows, in this order of preference:

1. the success line (`quests.md` §7.1 variant rule) of the action-6 role of `Quest_RoleArray`
   whose `QuestDispenser_NPC` is the giver's body record (`Body::Object(did)` of the giver's
   `EntityCreated`);
2. else the first action-6 role in stored order that has a line;
3. else `Quest_Description`;
4. else nothing (the pane stays empty).

For `Ref::Literal` quests the pane shows the literal.

<a id="s6-5"></a>
### 6.5 Selectable rewards, accept and decline

| Control | Command |
|---|---|
| `QuestAcceptButton` | `AnswerQuest { window, accept: true }` |
| `QuestDeclineButton` | `AnswerQuest { window, accept: false }` |
| `QuestCloseButton`, `EscapeKey` | `CloseWindow { window }` |
| A selectable reward row | **Tessera rule:** highlighted locally (list selection state `highlight`); nothing is sent (`AnswerQuest` carries no choice, §14 Q6) |

The generic answers (`scenario-feed.md` §7.6) close the window after accept or decline.

> **OPEN:** (Q6) whether the original requires a selectable reward before Accept and how the
> choice reaches the server; owner check O112. A feed revision adds a choice field if needed.

---

<a id="s7"></a>
## 7. Vendor window

<a id="s7-1"></a>
### 7.1 Layout (NW5)

`Vendor_Window` holds: backgrounds (`Vendor_BaseBox_BG`, two bars with images 0x41102B68 and
0x41102B69), the title bar with `Vendor_WindowLabel` (table `UI`), the drag bar, the close
button `Vendor_CloseButton`, `Vendor_FindItemsButton`, the player's money
`Vendor_PlayerMoney` (based on `MoneyDisplay_PlayerMoney_YourMoneyTemplate`, §9.2), and the
panel `Vendor_Panel` with four pages and tabs, in the panel's order:

| Page | Tab (text of table `Vendor` 0x2500001D) | Page layout (base) | Open at start |
|---|---|---|---|
| `Vendor_SellPage` | `Vendor_SellTab` | `VendorSellWindow` of 0x22000094 | no |
| `Vendor_BuyPage` | `Vendor_BuyTab` | `VendorBuyWindow` of 0x22000093 | **yes** |
| `Vendor_RepairPage` | `Vendor_RepairTab` | `RepairWindow` of `repair` | no |
| `Vendor_BuybackPage` | `Vendor_BuybackTab` | `VendorBuybackWindow` of 0x22000092 | no |

Tabs from left to right: Buy (6, 10), Sell (100, 10), Buy-back (194, 10), Repair (288, 10),
each 94 × 23.

**Lists.** The buy, sell and buy-back lists are instances of `VendorListBoxTemplate`
(`vendor_listbox`): a `Tree` list `VendorListBox`, horizontal, two columns, click selection,
items in state `normal` and, when selected, `highlight`, with scroll bar `VendorScrollbar`.

**Buy page** (`VendorBuyWindow`): `Vendor_BuyListField` (384 × 300 at (0, 30)); sort and
filter labels and menus (`Vendor_BuySortList`, `Vendor_BuyFilterList`); the stack hint
`Vendor_BuyStackDescriptionLabel` (centred text of table `Vendor`); the buttons
`Vendor_BuyTryOnButton` (dressing room) and `Vendor_BuyButton`.

**Sell page** (`VendorSellWindow`): the check box `Vendor_SellShowLockedButton` with its text;
`Vendor_SellInventoryField` (384 × 329 at (0, 30)); the filter menu `Vendor_SellFilterList`;
`Vendor_SellButton`, `Vendor_SellAllButton`; the value label `Vendor_SellValueLabel` and
`Vendor_SellValue` (based on `MoneyDisplay_MoneyShowAllTemplate`).

**Buy-back page** (`VendorBuybackWindow`): `Vendor_BuybackListField` (384 × 357 at (0, 2)) and
`Vendor_BuybackButton` (with a tooltip).

<a id="s7-2"></a>
### 7.2 Entry rows (NW5)

Buy and buy-back entries are instances of `VendorItem` (`vendor_item`), based on
`VendorItemDisplay_Template` (`itemdisplay`, widget `VendorItemDisplay`, 183 × 64):

| Part | Data | Fill (**Tessera rule** by name) |
|---|---|---|
| background | states `normal` (image 0x41005F87), `highlight`, `normal_rollover`, `highlight_rollover` (0x41005F8B) | selected and hovered states |
| `VendorItemDisplay_Item` | `ItemDisplay_IconTemplate` 32 × 32 at (8, 7) | the composed icon (`icon-layers.md` §3) with the overlays of `character-panels.md` §5 (quantity overlay for an entry quantity above 1) |
| `ItemDisplay_Name` | buttonfont text 131 × 30 at (42, 8), left, top, left margin 4, truncated with a tooltip | the item's name; for `Ref::Literal`, the literal |
| `VendorItemDisplay_Money` | `MoneyDisplay_MoneyTemplate` 160 × 22 at (17, 41), tooltips off | the unit price in coins (§9.2); state `TooExpensive` (red count texts) when the coin price exceeds the player's last `WalletChanged { Coin }` amount |
| `VendorItem_StackOverlay` | 20 × 20 at (154, 23), states `vendoritem_stackable_overlay_on` (0x4101E1AB), `vendoritem_quantity_overlay_on` (0x410FB9BD), `vendoritem_fillstackable_overlay_on` (0x410FB9BE), `vendoritem_overlay_off` | the buy mode of §7.4 while its toggle is held, for items whose `Inventory_MaxQuantity` is 2 or more; off otherwise |

Sell entries are instances of `VendorSellItem`, based on `VendorSellItemDisplay_Template`
(widget `VendorSellItemDisplay`): the same icon, name (111 wide), money display and the lock
button `VendorSellItemDisplay_Locked` (20 × 20, tooltip).

Hovering the icon or name shows the item's examination tooltip (`item-tooltips.md` §3, §9).

> **Tessera rule:** the name uses the quality colour of `item-tooltips.md` §6.2; the stock of
> `VendorShown` entries, when present, is shown in the quantity overlay of the icon; stock 0
> ghosts the Buy button while that entry is selected. Entries are listed in feed entry order,
> two per row, left then right.

<a id="s7-3"></a>
### 7.3 Prices

A feed `Price { coin, items }` is shown as:

- `coin` in the entry's money display (§9.2);
- item costs (barter): **Tessera rule:** a `VendorShown` in which any entry has an item cost
  opens the **barter window** (§7.7) instead of the vendor window; there each entry shows its
  item costs in the cost slots and, when `coin` is above 0, the coin price as one more cost
  (§7.7).

`VendorItemDisplay_AltCurrency_Quantity` (a top-level text template of `itemdisplay`, size
0 × 0) is not used by P3.

<a id="s7-4"></a>
### 7.4 Buying (BIN, NW14; NW10)

On the buy page:

- a right click (`SelectRight`) on an entry selects it and buys it;
- a left double-click (`SelectDblLeft`) on an entry buys the selected entry;
- `SelectLeft` on an entry selects it;
- `Vendor_BuyButton` buys the selected entry;
- `Vendor_BuyTryOnButton` previews the selected entry in the dressing room.

**Buy modes.** While the vendor window is active, the input map `VendorBuyUI` binds three
toggles; holding one sets a mode, releasing clears it (several can be held; the first in this
list wins): `ToggleQuantityDisplay` (Alt) → quantity mode; `ToggleStackDisplay` (Shift) →
stack mode; `ToggleFillStackDisplay` (Tab) → fill mode; none → single mode. A change of the
held toggles refreshes the overlays and the stack description label. Buying the selected
entry then:

1. when the item's stack size (`Inventory_MaxQuantity`) is below 2: buys one;
2. quantity mode: opens the quantity dialog (§7.5) with the stack size as its maximum;
3. stack mode: buys one full stack (quantity = stack size);
4. fill mode: buys enough to fill the player's partial stacks (the quantity is decided
   outside the window);
5. single mode: buys one.

> **OPEN:** (Q13) the quantity of fill mode; owner check O113.

> **Tessera rule (commands):** every purchase sends `Buy { window, entry, quantity }`: 1 for
> cases 1 and 5, the stack size for case 3, the dialog's quantity for case 2. Case 4 sends
> the quantity that fills the backpack's partial stacks of that record, computed from the
> inventory model (`character-panels.md` §4; stack size minus quantity, summed over the
> record's backpack stacks), or 1 when there is none. The client changes nothing until the
> feed's events arrive.

<a id="s7-5"></a>
### 7.5 Quantity dialog (BIN, NW15)

`VendorBuyStackField` holds: the drag bar, the text `HowManyText` (table `UI`), the entry
`VendorBuyStack_EntryText` (editable, centred, at most 5 characters) in `EntryField`, the
arrows `VendorBuyStack_Amount_Decrease` and `…_Increase`, the preset buttons
`VendorBuyStack_Amount_Full`, `…_Half`, `…_Zero`, the cost `VendorBuyStack_CostDisplay` (based
on `MoneyDisplay_ItemCostTemplate`: a label and a coin list), the item
`VendorBuyStack_ItemDisplay` (medium item display), `VendorBuyStack_OkButton` and the close
button `VendorBuyStack_CancelButton`.

Rules (*M* = the maximum the dialog was opened with, the stack size). The original keeps the
quantity only as the entry's text; "the entry's value" below is that text read as a number
(rule 4).

1. **Setting a quantity** *q*: *q* is clamped to 1 … *M*, the entry's text is replaced by the
   clamped value as a decimal integer, the cost display shows the price for that quantity, and
   the OK button's state is chosen again (rule 7).
2. Full sets *M*; Half sets *M* × 0.5 truncated (then clamped, so at least 1); the third
   preset sets 1.
3. The arrows set the entry's value minus or plus 1 (rule 1 clamps the result); an empty
   entry counts as 1.
4. **Typing.** Every character input to the entry (UI message `Text_character_input`, 0x28,
   deletions included) re-reads the entry's whole text after the change:
   - a non-empty text is read as a number and set as the quantity (rule 1), so it is clamped
     and the entry is rewritten at once: with *M* = 50, typing 8 leaves 8, typing a 0 after
     it gives 80, which is set as 50 and the entry shows 50;
   - an empty text leaves the quantity (and the cost shown) unchanged; only the OK button's
     state is chosen again (rule 7).

   Reading a text as a number: leading `0` characters are skipped, then the value is the
   unsigned decimal number formed by the leading digits that follow; a text that does not
   start with a digit reads as 0 (so it is set as 1). With at most 5 characters no value
   overflows. After any input the entry therefore holds either nothing or a decimal integer
   1 … *M*. (In the original, a text that starts with white space or a sign goes through a
   general unsigned conversion: white space is skipped, a `0x` or `0` prefix after it selects
   base 16 or 8, a `-` negates modulo 2^32. **Tessera rule:** such a text reads as 0.)
5. **OK**, or `AcceptInput` (Enter) while the dialog has focus:
   1. an empty entry: nothing happens; the dialog stays open;
   2. a value of 0 is set to 1, a value above *M* is set (clamped); the dialog stays open in
      both cases (rule 4 keeps the entry from holding either, so this happens only if the
      entry was filled some other way);
   3. while the cost display (its money display) is in state `TooExpensive`: nothing happens;
      the dialog stays open;
   4. otherwise the purchase is made with the entry's value, the dialog remembers the vendor
      and that quantity (rule 8), and closes.

   A `ghosted` OK button sends no click (`ui-widgets.md` §7.1 step 3, unless the layout
   sets `UICore_Button_click_when_ghosted`), but Enter reaches these steps in any OK state.
6. The close button closes the dialog without buying.
7. **OK button state:** after every set (rule 1) and after an input that leaves the entry
   empty, `VendorBuyStack_OkButton` is put in state `ghosted` (0x12) when the entry is empty
   or the player's coin is below the cost shown, and in state `normal` (0x0B) otherwise.
8. **Opening.** Opening the dialog while it is open first closes the open one without buying.
   The dialog opens with quantity *M* (set by rule 1), except when it is opened for the same
   vendor as the last purchase made through it: then with that purchase's quantity, at most
   *M*. Opening it for another vendor forgets the remembered quantity; closing the dialog
   does not.

The money display of the cost puts its count texts in state `TooExpensive` when the player's
coin is below the amount it shows, and `normal` otherwise (BIN, revision 2: the money display
does this itself when an option of the display is on; which layout property sets that option
was not read).

> **Tessera rule:** the cost display shows the entry's unit coin price × quantity (and item
> costs × quantity in its tooltip); the purchase sends `Buy { window, entry, quantity }`. The
> dialog closes when its vendor window closes. "The player's coin" in rules 5 and 7 and in the
> `TooExpensive` state is the amount of the last `WalletChanged { Coin }`; before any, there is
> no comparison (the OK button is `ghosted` only for an empty entry, and the cost display
> stays `normal`). "The same vendor" in rule 8 is the vendor window's entity when the feed
> names one, else its `WindowId`; the remembered quantity lasts for the client session and is
> not saved. After a rewrite (rule 4) the caret stands after the last character.

> **OPEN:** (implementer, `tessera-ui`, 2026-10-11) Tessera's text editing posts
> `Text_character_input` only for inserted characters; deletions, cuts and pastes post
> Tessera's own text-changed notification, which the dialog reads as well (rule 4, "deletions
> included"). Both reach the dialog as game-code events after the frame's input step, so when
> several edits land in one frame (a paste, or typing faster than the frame rate) the dialog
> re-reads the entry once with all of them applied: `0` then `5` in one frame give 5, where a
> re-read per character gives 15. Tessera choice until settled: one re-read per batch of
> edits.

<a id="s7-6"></a>
### 7.6 Selling (BIN, NW16)

On the sell page a right click or left double-click on an entry selects it and sells it;
`Vendor_SellButton` sells the selected entry; `Vendor_SellAllButton` sells every listed item
that is not locked; the check box toggles a player option (showing locked items) and
refreshes the list; a change of the filter menu refreshes the list. A sale names whole items
(the item instances), not quantities.

> **OPEN:** (Q7) which items the original lists for sale, the effect of dropping an item on the
> window, the sale price and Sell All's scope; owner check O113.

> **Tessera rule (sell list and commands):**
>
> 1. The sell list shows the backpack items (`character-panels.md` §4.3, slot order) whose
>    record is sellable by `items.md` §5.3 (the client library's sellability test that the
>    worth row uses, `item-tooltips.md` §6.11 step 2). No item is locked in the sandbox;
>    the check box and the lock buttons have no effect; the filter menu has no items.
> 2. Each entry's money display shows the item's worth (`item-tooltips.md` §6.11: value ×
>    quantity, coins as §9.2), labelled by `Vendor_SellValueLabel` for the selected entry in
>    `Vendor_SellValue`. This is a display of worth, not the price the vendor pays
>    (`economy.md` §11.6 OPEN); the money the player receives arrives as `WalletChanged`.
> 3. Selling an entry sends `Sell { window, item, quantity: the item's quantity }`; Sell All
>    sends one `Sell` per listed item, in list order.
> 4. Dropping a backpack item on the vendor window (any page) sells it like the Sell button
>    (`ui-behaviour.md` §7).

> **OPEN:** (implementer, `apps/client`, 2026-10-10) rule 1's sellability test reads the
> record only: "the sale value is 0" is taken as `Item_Value` absent or 0, without an
> instance's wear-state factor (§6.11), and `ItemAdvancement_Item` counts when present.
> Without an install every backpack item is listed with worth 0 (no sale data to test).
> Rule 4 (dropping a bag item on the window) is not wired yet. Please confirm.

> **OPEN:** (implementer, `tessera-ui`, 2026-10-11) rule 2 does not say what an entry's money
> display shows for a worth of 0 (a literal item, which has no record, or an item without
> sale data): `MoneyDisplay_MoneyTemplate` hides every zero coin, so the entry showed no value
> at all. Tessera rule until settled: a literal item's worth is 0, and an entry with worth 0
> shows the copper coin with count 0 (the §9.2 rule for an amount of 0 with `ForceShowAll`
> and `SuppressLeadingZeroes`); any other worth uses the template's own options.

<a id="s7-7"></a>
### 7.7 Barter window (NW6)

`Barter_Window` holds: the title bar with `Barter_WindowLabel`, the profile label and menu
(`BarterProfile_label`, `BarterProfile_Menu`), column labels `Barter_RewardLabel` and
`Barter_ItemsLabel`, the list `BarterList_Listbox` (one column, click selection, scroll bar
`BarterListbox_Scrollbar`), the page control `BarterList_PagCtrl` (20 entries per page,
`UI_Barter_PageSize`), the buy button `Barter_BuyButton` and a hidden store button.

Each entry is `BarterEntryTemplate` (608 × 65, based on `Button_TitleListEntry`): the reward
`BarterReward` with `BarterRewardItem` (medium item display, 362 × 32) or
`BarterDisplay_ReputationRewardField` (icon 0x410D705B and two text lines); three cost slots
`BarterItem1` … `BarterItem3` (32 × 32 short item displays at x 388, 467, 545) each with an
info text `BarterItem1_Info` … (centred, bottom-justified, truncated).

The list's entry template names layout 0x22000045 (`accomplishment`), which has no element
`BarterEntryTemplate`; the barter layout has it at top level (NW6).

> **OPEN:** (Q8) how the original builds barter entries (given the dangling entry template),
> shows coin costs and formats the info text; owner check O114.

> **Tessera rule:**
>
> - Entry template: `BarterEntryTemplate` of the barter layout.
> - Reward: the entry's `what`; cost slots: the price's item costs in order, then, when
>   `coin` is above 0, one coin slot drawn as a money display (§9.2) in place of icon and
>   info; costs beyond three slots are not drawn (listed in the reward's tooltip).
> - Info text of a cost slot: the required quantity as a decimal integer; when the player
>   holds fewer (a wallet item's last reported amount, or the sum of backpack quantities of
>   that record), the text uses the colour of the money display's `TooExpensive` state.
> - The profile menu shows one entry (the feed's list) and is ghosted; pages hold 20 entries
>   in entry order.
> - `Barter_BuyButton`, a right click or a left double-click on an entry send
>   `Buy { window, entry, quantity: 1 }`.

> **OPEN:** (P3 exit verifier, 2026-10-11) `BarterReward` holds both alternatives at once
> (`BarterRewardItem` and `BarterDisplay_ReputationRewardField`, whose two text lines carry the
> layout's sample text); which one the original hides is not stated. Tessera rule: every feed
> reward is an item, so the reputation field is hidden.

> **OPEN:** (implementer, `tessera-ui`, 2026-10-10) the parts and messages of the page control
> `BarterList_PagCtrl` (a game widget type with three children here) are not stated: which
> child turns to the previous or next page, whether it shows a page number, and what it
> sends. Until they are, `tessera-ui` hides the control's own children and creates two
> Tessera buttons (previous, next) inside it, ghosted on the first and the last page.

<a id="s7-8"></a>
### 7.8 Buy-back and the command table (NW16)

On the buy-back page a right click or left double-click on an entry selects it and buys it
back; `Vendor_BuybackButton` buys back the selected entry. Buy-back entries come from the
feed's `BuybackShown` (revision 5, §10.4) and look like buy entries (§7.2).

| Control | Command |
|---|---|
| Buy entry: right click; left double-click; `Vendor_BuyButton`; quantity dialog OK | `Buy { window, entry, quantity }` (§7.4, §7.5) |
| Sell entry: right click; left double-click; `Vendor_SellButton`; drop from a bag | `Sell { window, item, quantity }` (§7.6) |
| `Vendor_SellAllButton` | one `Sell` per listed item |
| Buy-back entry: right click; left double-click; `Vendor_BuybackButton` | `BuyBack { window, entry }` (revision 5) |
| `Vendor_CloseButton`, `EscapeKey` | `CloseWindow { window }` |
| Tabs | switch pages locally |

> **Tessera rule (hidden in P3):** the Repair tab and page, `Vendor_FindItemsButton` and
> `Vendor_BuyTryOnButton` are hidden; the sort and filter menus are ghosted and the lists keep
> feed order. The buy-back tab is shown only after a `BuybackShown` for the window.

---

<a id="s8"></a>
## 8. Loot window

<a id="s8-1"></a>
### 8.1 Layout (NW7)

`ExternalContainerField` holds: `BaseBox_ExternalContainer` (background), the list
`ExternalContainer` (based on `ContainerDisplay_01` of layout 0x2200008F: the list box
`ContainerListBox`, a container element, one column, `UI_Item_DisplayType` `Medium`, scroll bar
`ContainerScrollbar`; 246 × 154 at (9, 37)), the close button `ExternalContainerCloseButton`,
`ExternalContainerBackground` (states `container_chest`, `container_corpse`), the title bar with
`External_Title` (a fixed text of table `UI`), the drag bar, the button
`ExternalContainerLootAllButton` (102 × 20 at (81, 189)) and the timer
`ExternalContainerTimer`. The layout has two variants: `ExternalContainerField_Headless` (same
parts, the button `ExternalContainerButton` with another text) and
`ExternalContainerFieldLarge` (six columns, no button), not used by P3.

> **OPEN:** (Q9) how the original shows money in loot and which input takes a single entry;
> owner check O115.

<a id="s8-2"></a>
### 8.2 Rows and the coin line (Tessera rule)

- One row per untaken entry of `LootShown`, in entry order: a medium item display
  (`ItemDisplay_MediumTemplate`: icon 32 × 32 with the quantity overlay for quantities above
  1, name 165 × 32), with the item tooltip on hover.
- The layout has no coin element. When `coin` is above 0, the first row is a coin row: a
  money display (`MoneyDisplay_MoneyTemplate`, §9.2) showing the coin amount. It is not an
  entry: it cannot be taken alone.
- Background state: `container_corpse` when the source is an entity of kind creature,
  non-player or player; `container_chest` otherwise (object or no source).
- The timer is hidden.

> **OPEN:** (implementer, `tessera-ui`, 2026-10-11) whether the medium item display's name
> (165 × 32) wraps is not stated; on this install long names wrapped to two lines that filled
> the row. Tessera rule until settled: a loot row's name element gets `UICore_Text_one_line`
> true, so a long name is one line, cut with the element's truncation suffix and tooltip when
> it has `UICore_Text_truncate_text_to_fit` (`fonts.md` §7.6), else clipped at its right edge.

<a id="s8-3"></a>
### 8.3 Taking loot (BIN, NW13)

The original:

1. `ExternalContainerLootAllButton` and `AcceptInput` (Enter) while the window is active do
   **Loot All**: when the list has no entry, the window closes; otherwise the client asks
   the class library to take everything from the container and remembers that Loot All is
   pending.
2. Whenever the container's contents change, the client relabels the button: with at least
   one listed entry, the layout's text (table `UI` token 0x0A5A16DC); with none, the text of
   table `UI` token 0x02E15B29. When the list is empty and Loot All is pending, the window
   closes.
3. The close button closes the window (after calling a class-library function with the
   container, by inference to stop looting).

> **Tessera rule (commands):**
>
> | Input | Command |
> |---|---|
> | Loot All button, Enter | `TakeLoot { window, entry: None }` while an untaken entry exists or `coin` is above 0 and no Loot All was sent yet; otherwise `CloseWindow` |
> | Right click or left double-click on an entry row; dragging a row into a bag | `TakeLoot { window, entry: Some(n) }` (a drop's target slot is not sent) |
> | Close button, `EscapeKey` | `CloseWindow` |
>
> After a Loot All has been sent, the client sends `CloseWindow` once at the end of the first
> frame in which every entry has been taken (`LootTaken` received for each). The button's
> label follows rule 2 with the coin row not counted. A `LootShown` opened from an entity used
> with `AutoLootAll` (§3.1) sends Loot All at once. A `LootTaken` for the last entry without
> a pending Loot All leaves the window open with the relabelled button (rule 2).

> **OPEN:** (implementer, `apps/client`, 2026-10-11) "dragging a row into a bag": the client
> cannot tell a bag slot from other drop targets of another window, so it reads a drag from a
> loot row that ends on any element outside the loot window as a drag into a bag (Tessera
> reading); a drop on the loot window itself, or on no element, takes nothing.

> **OPEN:** (implementer, `tessera-ui`, 2026-10-10) a `LootShown` with coin and no entries:
> Loot All sends `TakeLoot { entry: None }`, and since every entry is then "taken" the client
> sends `CloseWindow` at the end of the same frame, before the feed can answer. `tessera-ui`
> does this as written. Should the close wait for the coin (a `LootTaken` or `WalletChanged`
> after the Loot All), or for the next frame?

---

<a id="s9"></a>
## 9. Wallet and coins

<a id="s9-1"></a>
### 9.1 Where money is shown

| Place | Element | Template (§9.2) |
|---|---|---|
| Vendor window | `Vendor_PlayerMoney` | `MoneyDisplay_PlayerMoney_YourMoneyTemplate` (a label of table `Vendor` and the coin list `MoneyDisplay_YourMoney_AllMoney`, based on `MoneyDisplay_PlayerMoneyShowAllTemplate`) |
| Wallet window | `PaperItemPanel_MoneyEntry` → `MoneyDisplay` | `MoneyDisplay_PlayerMoneyWalletTemplate` |
| Vendor entries, quantity dialog, reward rows, sell value | `VendorItemDisplay_Money`, `VendorBuyStack_CostDisplay`, `QuestRewards_MoneyRewardText`, `Vendor_SellValue` | `MoneyDisplay_MoneyTemplate`, `MoneyDisplay_ItemCostTemplate`, `MoneyDisplay_MoneyShowAllTemplate` |
| Item tooltips | worth row | `MoneyDisplay_ItemWorthTemplate` (`item-tooltips.md` §6.11) |

The bag windows (`containerdisplay`, `inventory`) hold no money element. The character
panel's `Player_Wallet_Button` (click action `TogglePaperItemPanel`) opens the wallet window.

<a id="s9-2"></a>
### 9.2 Money display and coin images (Confirmed NW8; BIN NW18)

A money display is a horizontal list (one row, at most three columns, only visible items laid
out) of **coin elements**. The display names its coin element by
`UIComponent_MoneyDisplay_CoinDisplay_ElementID` and `…_CoinDisplay_Layout` (layout
`moneydisplay`). On construction it creates exactly three coin elements, with icon states
`Copper`, `Silver` and `Gold`, in that order, each hidden; it appends each one, or, when
`UIComponent_MoneyDisplay_LeftJustifiedCoins` is true, inserts each one at the front. With the
list filled in reverse (`UICore_ListBox_fill_in_reverse` true) or left-justified, the coins
therefore read gold, silver, copper from left to right (derived from the two list options).

Coin elements:

| Template | Count text `MoneyDisplay_Text` | Icon `MoneyDisplay_Icon` (in `MoneyDisplay_Icon_Holder`) | States of the element |
|---|---|---|---|
| `MoneyDisplay_CoinTemplate` (53 × 22) | fieldlabelsmall, 31 × 16 at (0, 1), right, right margin 3; colour 0xFFF5DF93 (`normal`), 0xFFFF0000 (`TooExpensive`) | 32 × 22, holder at (23, 0) | `normal`, `TooExpensive` |
| `MoneyDisplay_WalletCoinTemplate` (53 × 22) | as above, colour 0xFFFFFFFF (`normal`) | holder at (23, −1) | as above |
| `MoneyDisplay_ExaminationCoinTemplate` (59 × 22) | 37 × 16, font 0x42000001, colour 0xFFE5E5E5, right margin 4 | holder at (29, 0) | as above |

The icon's states choose the coin image and its tooltip (StringInfo of table `UI`):

| Icon state | Image | Tooltip token |
|---|---|---|
| `Gold` | 0x41007E7B | 0x0135BA5C |
| `Silver` | 0x41007E7C | 0x01E4A5BC |
| `Copper` | 0x41007E7D | 0x0BE07C7C |

The count text is set through the format of table `UI` token 0x07480C44 (variable 0x5242A9,
the count as an integer), as `item-tooltips.md` §6.11 states.

Display templates (all `MoneyDisplay` or `PlayerMoneyDisplay` widgets, 160 × 22):

| Template | Coin element | Options |
|---|---|---|
| `MoneyDisplay_MoneyTemplate` | `MoneyDisplay_CoinTemplate` | — (states `normal`, `TooExpensive` and button states) |
| `MoneyDisplay_MoneyShowAllTemplate` | `MoneyDisplay_CoinTemplate` | `UIComponent_MoneyDisplay_ForceShowAll`, `…_SuppressLeadingZeroes` |
| `MoneyDisplay_MoneyWalletShowAllTemplate` | `MoneyDisplay_WalletCoinTemplate` | as above |
| `MoneyDisplay_ExaminationTemplate` | `MoneyDisplay_ExaminationCoinTemplate` | based on `MoneyDisplay_MoneyTemplate` |
| `MoneyDisplay_PlayerMoneyTemplate` | `MoneyDisplay_CoinTemplate` | — (shows the player's money) |
| `MoneyDisplay_PlayerMoneyShowAllTemplate` | `MoneyDisplay_CoinTemplate` | `ForceShowAll` |
| `MoneyDisplay_PlayerMoneyWalletTemplate` | `MoneyDisplay_WalletCoinTemplate` | `ForceShowAll` |

`MoneyDisplay_ItemWorthTemplate` (215 × 24) is the worth label `MoneyDisplay_WorthText` (text of
table `Examination` token 0x09607F15, colour 0xFFE5E5E5, font 0x42000001, outline, fits its
text) and the coin list `MoneyDisplay_ItemWorthDisplay` (based on
`MoneyDisplay_ExaminationTemplate`, at (49, 2), not filled in reverse, left-justified coins).
`MoneyDisplay_ItemCostTemplate` has the same two parts with another label token (0x0A976764)
and colour 0xFFC3B585, its coin list based on `MoneyDisplay_MoneyTemplate`.

> **OPEN:** (Q11) which currency fills which coin element (the display's fill routine was not
> read); owner check O117.

> **Tessera rule (coins):** a coin amount is split by §9.3; the largest-value currency's count
> goes to the `Gold` element, the next to `Silver`, the value-1 currency's to `Copper`. A coin
> with count 0 is hidden, except: with `ForceShowAll` every coin is shown; with
> `SuppressLeadingZeroes` as well, a zero coin is shown only after a larger non-zero coin
> (`item-tooltips.md` §6.11). An amount of 0 with `ForceShowAll` shows only the copper coin
> with 0 when `SuppressLeadingZeroes` is set, and all three otherwise. `TooExpensive` is set
> by the window (§7.2, §7.5), never by the display (the original's display can set it itself
> when an option is on, §7.5; Tessera keeps the choice in the window, with the same result).

<a id="s9-3"></a>
### 9.3 Splitting an amount (BC; NW19)

The split is `item-tooltips.md` §6.11 (class 404 function 2): walk the currency directory's
list, take for each currency amount div `Currency_Value`, keep the remainder. The class library
walks the **object form** of the directory (record `CurrencyDirectory`, its compiled list of
three references), which is in the order gold, silver, copper (`Currency_Value` 100,000, 100,
1 here). The **property set** of the same record stores `Currency_CurrencyList` in the order
silver, gold, copper.

> **Tessera rule:** a reader of the property set sorts the currencies by `Currency_Value`,
> largest first, before splitting (equal to the object form's order here). A currency with a
> value below 1 is ignored.

<a id="s9-4"></a>
### 9.4 Wallet window (NW9)

`PaperItemPanel_Window_V2` holds: backgrounds (`PaperItemPanel_BaseBox_BG`, the
`Box_Wallet_Background` frame with corner images), the close button, the drag bar, the title
bar with `PaperItemPanel_WindowLabel` (table `UI_PaperItem` 0x2501BDF6), the resize bar
`ResizeBarBottom`, the list area `PaperItemPanel_Barter_Player` (based on
`PaperItemPanel_Barter_Base`), the top menu `PaperItemPanel_TopMenu` (a `HideWhenField`
hidden for monster players), the destiny-perks button field (hidden for session players) and
store elements (`PaperItemPanel_MithrilUpsellButton`, `PaperItemPanel_CurrencyUpsell`, the
flyout `PaperItemPanel_MTXFlyout`).

`PaperItemPanel_Barter_Base` holds the expand and collapse buttons, a search label and entry
(`PaperItemPanel_SearchEntry`, at most 80 characters), and the page `PaperItemPanel_Page` with
the tree `PaperItemListBox` (one column, auto expand, prunes empty parents, indent 8, tree icon
`PaperItem_TreeIcon` with states `open` 0x411202B5 and `closed` 0x411202B6, scroll bar
`PaperItemListBox_Scrollbar`).

Row templates (top level of the layout):

| Template | Parts |
|---|---|
| `PaperItem_TreeCategory` | a category title, fieldlabelmedium, colour 0xFFF5DF92 |
| `PaperItem_EntryTemplate` (based on `PaperItem_Entry_Base`: background `Box_Wallet_Barter_ListItem`, examination override layout 0x22000000 element `PaperItem_Entry_ExamineLayout`) | `PaperItemEntry_Icon` 16 × 16 at (346, 1); `PaperItemEntry_Name` 312 × 16 at (23, 2), white, outlined, state `AccountItem` (0xFF18E618); `PaperItemEntry_Total` 103 × 16 at (242, 2), right, states `ItemCapped` (0xFFF20C1A) and `ItemGrandfathered` (0xFFE9AF15) |
| `PaperItemPanel_MoneyEntry` | background, the label `MoneyDisplay_Label` (table `UI_PaperItem`) and `MoneyDisplay` (`MoneyDisplay_PlayerMoneyWalletTemplate`, sound effects on) |
| `PaperItemPanel_CoinOverflowEntry` | label `OverflowEntry_Label` and `PaperItemPanel_OverflowDisplay` (`MoneyDisplay_MoneyWalletShowAllTemplate`) |
| `PaperItemEntry_Tooltip` (`PaperItemInfo`) | entry tooltip rows: name and icon, description, cap state, account-shared text |

**Wallet items.** The record `PaperItemControl` (WEENIECONTENT label) lists the wallet items in
`PaperItemControl_Free_PaperItemArray` (167 here) and `PaperItemControl_PaperItemArray` (443),
each entry a `PaperItemControl_Item` with `PaperItemControl_BaseCap` (`PaperItemControl_CapDefault`)
and `PaperItemControl_TreatBaseCapAsHardCap`. All 610 items carry `PaperItem_Category`
(mapper `PaperItemCategory`, 0x23000316 here; 16 keys, 0–6 and 8–16, with display texts for
14 of them: not for `Undef` 0 and `Hidden` 5), `Name` and `Icon_Layer_ImageDID`; 577 carry `PaperItem_SmallIcon`
(a 0x41 image). Category counts here: Instances 206, Taxidermy 86, Crafting 81, Landscape 77,
Festival 67, Skirmish 33, Misc 31, Coin 11, IA 8, PvMP 7, Capped_Currency 3.

Opening: `TogglePaperItemPanel` (default B, NW10) and `Player_Wallet_Button`.

> **OPEN:** (Q10) the original's category and entry order, the total's format and the cap
> state's condition; owner check O116.

> **Tessera rule (fill):**
>
> 1. The money row (`PaperItemPanel_MoneyEntry`) is first, showing the last
>    `WalletChanged { Coin }` amount (0 before any). The coin overflow row is hidden.
> 2. Then one category row per category that has a listed entry, in ascending category key,
>    titled with the category's display text (its label when it has none); under each, its
>    entries sorted by rendered name (case-insensitive), then by DID.
> 3. An entry is listed for every `Currency::Item(ref)` whose last `WalletChanged` amount is
>    above 0. A record ref takes its category, name and icon from the item: icon
>    `PaperItem_SmallIcon`, else the composed icon scaled to 16 × 16; a literal ref goes
>    under the category `Misc` with the literal as name and no icon.
> 4. `PaperItemEntry_Total` shows the amount as a decimal integer; state `ItemCapped` when
>    the amount is at or above the item's `PaperItemControl_CapDefault`. `AccountItem` and
>    `ItemGrandfathered` are never set.
> 5. The search entry keeps the entries whose name contains its text (case-insensitive) and
>    the categories that still have entries; expand and collapse act on all categories.
> 6. Store elements, the upsell flyout and the destiny-perks button are hidden.

> **OPEN:** (implementer, `apps/client`, 2026-10-10) rule 3 does not say where a record
> ref that is not a wallet item and has no `PaperItem_Category` goes (a barter token of the
> feed can be any item); `tessera-gamedata` lists it under a category row without a title.
> `PaperItemEntry_Icon` takes one image: for an entry without `PaperItem_SmallIcon` the
> client shows the item's `Icon_Layer_ImageDID` (not the composed layers) at 16 × 16.

> **OPEN:** (implementer, `tessera-gamedata`, 2026-10-11) Tessera rule for the question above
> until settled: a record ref without `PaperItem_Category` is listed under the category
> `Misc`, with the literal refs (rule 3), so no category row is without a title. A record
> that has a category keeps it, wallet item or not.

> **OPEN:** (P3 exit verifier, 2026-10-11) on this install `PaperItemEntry_Total` is of the
> game type `QuantityDisplay`, which no spec gives an engine class; built as the fallback Field
> it drew no text, so rule 4's amount was invisible. Tessera rule until settled:
> `QuantityDisplay` is created as a Text. The destiny-perks button field of rule 6 is the
> element `PaperItemPanel_HidePerksButtonField` (label read from this install's layout).

<a id="s9-5"></a>
### 9.5 Barter currencies

Barter costs (`economy.md` §6.1) name ordinary items: of the 58,554 cost structs with an item,
44,856 name one of the 610 wallet items (423 distinct wallet items; 3,081 distinct cost items
in all, NW21). A barter currency is therefore usually a wallet item: its held amount is the
wallet's (`WalletChanged { Item }`, `scenario-feed.md` §5.5); other cost items are counted in
the backpack. The wallet lists them by category (§9.4); the barter window shows them in its
cost slots (§7.7). The three coin currencies (`economy.md` §7) are never wallet items.

<a id="s9-6"></a>
### 9.6 The wallet model for plugins

The plugin wallet views (`lua-plugin-api.md` §15) read the same model as §9.4: the coin amount
and the list of (wallet item ref, amount) from the latest `WalletChanged` per currency. This
spec adds nothing to the plugin API.

---

<a id="s10"></a>
## 10. The scenario feed

<a id="s10-1"></a>
### 10.1 Events and commands per window

| Feed event (`scenario-feed.md` §9.2) | Window effect |
|---|---|
| `DialogueShown` | §5.1–§5.4 |
| `QuestOffered` | conversation window as quest offer (§6) |
| `QuestRewards` (revision 5) | amount rows of the open offer (§6.3) |
| `VendorShown` | vendor or barter window (§7.3) |
| `BuybackShown` (revision 5) | buy-back page (§7.8) |
| `LootShown`, `LootTaken` | loot window rows (§8) |
| `WindowClosed` | hides the window (§5.7) |
| `WalletChanged` | money displays and wallet (§9) |
| `ItemAdded`, `ItemRemoved`, `ItemQuantity` | the sell list (§7.6), held amounts (§7.7) |
| `CharacterInfo`, `LevelChanged` | dialogue variables (§5.5) |

Commands: `UseEntity` (§3.1), `ChooseOption` (§5.3), `CloseWindow` (every window),
`AnswerQuest` (§6.5), `Buy`, `Sell` (§7), `BuyBack` (revision 5, §7.8), `TakeLoot` (§8.3).

<a id="s10-2"></a>
### 10.2 What revision 5 adds, and why

The existing events cover dialogue pages with install text (`Text::QuestRole`,
`Text::Comment`, `Text::Name`, `Text::Objective`), vendor buy lists, loot and the wallet
(currency refs name records, so wallet entries have names and icons). The sell list comes from
the inventory model. Three gaps need additive forms (`scenario-feed.md` revision 5, the P3
revision):

1. **Action labels** (`Text::Action`, text function `action(@Label)`): choice rows with the
   install's own action texts (§5.3), so a scenario can show a vendor or barter option with
   install text and no literal.
2. **Reward amounts** (`QuestRewards` event, `rewards` step): amounts the client must not
   derive (§6.3).
3. **Buy-back** (`BuybackShown` event, `buyback` step, `BuyBack` command): the buy-back page's
   list and its command (§7.8).

Revision 5 is in `scenario-feed.md` §4.2, §5.2, §6.3, §7.5, §7.7, §8.6, §9.1–§9.5, §10.3,
§11.1, §14.5 (rows N-H1 … N-H11), §14.N1, §14.N2 and §18.P3.

<a id="s10-3"></a>
### 10.3 Reward rows from `QuestRewards`

| `RewardKind` | Row (§6.3 step 1) | Amount shown |
|---|---|---|
| `Coin` | `QuestRewards_MoneyRewardText` | coins (§9.2) |
| `Reputation` (`what`: the faction) | `QuestRewards_ReputationField`, positive or negative state by the sign | the absolute amount, with the faction's name |
| `Glory`, `ItemXp`, `CraftXp`, `MountXp`, `TraitPoints` | the glory, item-, craft-, mount-experience and class-trait-point rows | decimal integer |
| `Wallet` (`what`: the wallet item) | `QuestRewards_PaperItemField` | decimal integer with the item's name |
| `VirtueXp` | `QuestRewards_VirtueExpField`: **not** a step-1 row; the last row of §6.3 step 2 (after the fixed emote entries, before the selectable rewards) | decimal integer |

Rows of step 1 follow the order of §6.3 step 1, not the order of the event's entries; several
`Reputation` or `Wallet` entries keep their relative order. The `VirtueXp` row is placed by
§6.3 step 2 (revision 2 answers the implementer OPEN of 2026-10-10 on its position).

> **OPEN:** (implementer, `tessera-gamedata` and `tessera-ui`, 2026-10-11) the `rewards` step
> writes `virtue-xp` at most once, but the `QuestRewards` event does not forbid several
> `VirtueXp` entries. Tessera choice: only the first is shown (NWI10, at most one row).

---

<a id="s11"></a>
## 11. Census: what the windows cannot show

A real-data test opens each window over records of the install and puts every record (or
entry) in exactly one category. Counts are this install's (NW22, NW23).

**Dialogue lines** (every role line of every quest record, every comment record):

| Category | Meaning | Here |
|---|---|---|
| `Shown` | renders a non-empty text | — (measured by the test) |
| `TextMissing` | the StringInfo lookup fails (`string-table.md` §9.3) | measured |
| `VariableUnsupplied` | an argument ID outside the table of §5.5 | at most 9 role-line entries (4 IDs) |
| `Empty` | renders to an empty text | measured |

**Quest offers** (every quest record), one primary category each:

| Category | Meaning | Here |
|---|---|---|
| `Offered` | an offer line by §6.4 rule 1 or 2 | 13,278 |
| `DescriptionOnly` | no bestow line; rule 3 shows `Quest_Description` | 2,030 |
| `NoText` | neither (rule 4: empty pane) | 92 |
| `Deed` | `Quest_IsAccomplishment` true: not offered | 5,407 |
| `RecordUnreadable` | the record fails to decode | 0 |

Counted besides (flags, not categories): `ObjectiveWithoutText` (the lowest objective has no
description, §6.2: 227 quests), `NoTreasure` (no `Quest_QuestTreasureDID`, so no fixed or
selectable rows: 1,376 quests), `SeveralBestowRoles` (834 quests; rule 1 decides by giver).
Every bestow role of the install has a line.

**Vendor, barter and loot entries** (every item a list can show):

| Category | Meaning | Here (vendor inventories) |
|---|---|---|
| `Shown` | name and icon | 10,579 of 10,579 distinct items |
| `NoIcon` | no non-null `Icon_Layer_*` (`has-icon=no`) | 0 |
| `NoName` | no `Name`, or it renders empty | 0 |
| `Literal` | a literal ref (placeholder icon, literal name) | — |
| `RecordUnreadable` | not an item record or fails to decode | 0 |

> **OPEN:** (implementer, 2026-10-10) three rules for "has an icon" disagree at the edges:
> `NoIcon` above says "no non-null `Icon_Layer_*`", the selector `has-icon`
> (`scenario-feed.md` §8.3) says "any `Icon_Layer_*` present", and `icon-layers.md` §3.1
> composes no icon without an Image or a Background property. An item with a non-null Image
> but no Background is `Shown` by the first rule and draws nothing by §3.1; one whose layers
> are all present but null has an icon by the second. `tessera-gamedata` counts `NoIcon`
> when no icon is drawn: §3.1 gives none, or every layer is null (all 10,579 vendor items
> are `Shown` either way here). Please state the census rule.

> **OPEN:** (implementer, 2026-10-10) "every role line of every quest record" is read as the
> success and failure lines of `Quest_RoleArray` roles and their species variants **and** the
> lines of `Quest_CompletionCommentArray` (§5.6; NW20's 57,534 entries match that sum
> closely, while the roles alone are about 1,750 short). Please confirm.

**Wallet entries** (the 610 wallet items): `Shown` (610), `NoSmallIcon` (33: the composed
icon is scaled), `NoCategoryText` (items whose category has no display text: 0 here).

**Windows themselves:** `LayoutMissing` (the `UILAYOUT` label or the root is absent) and
`ElementMissing` (an element this spec names is absent) are failures; this install has none.

---

<a id="s12"></a>
## 12. Invariants

| ID | Invariant | Status |
|---|---|---|
| NWI1 | Each window of §4 resolves: its `UILAYOUT` label exists and the root is a top-level element of the layout | Confirmed 2026-10-10 (8 of 8) |
| NWI2 | Every `ActionPickerUI_ActionEntryArray` entry has an `ActionPickerUI_Action` and either a list template or a template array (32 of 32) | Confirmed 2026-10-10 |
| NWI3 | The coin icon element has exactly the three states `Gold`, `Silver`, `Copper` in each coin template (3 of 3 templates) | Confirmed 2026-10-10 |
| NWI4 | The currency directory's object form and property set name the same three currencies; sorting the property set's list by `Currency_Value` descending gives the object form's order | Confirmed 2026-10-10 |
| NWI5 | Every wallet item has `PaperItem_Category`, `Name` and an icon layer (610 of 610) | Confirmed 2026-10-10 |
| NWI6 | The split of §9.3 reconstructs every amount 0 … 10^15 (Σ count × value = amount) when a currency of value 1 exists | Design check |
| NWI7 | A window instance is hidden only on `WindowClosed` (§5.7) | Design check (scenario) |
| NWI8 | The vendor quantity dialog never sends a quantity outside 1 … *M* (§7.5) | Design check |
| NWI9 | After any input to the quantity dialog's entry, its text is empty or a decimal integer 1 … *M* without leading zeros (§7.5 rule 4); OK on an empty entry sends nothing | Design check |
| NWI10 | No `QuestTreasure_FixedVirtueArray` entry becomes a reward row; at most one `QuestRewards_VirtueExpField` row, never before a fixed entry of the same offer (§6.3) | Design check |

---

<a id="s13"></a>
## 13. Worked synthetic examples

<a id="s13-1"></a>
### 13.1 Coins

Currencies stored in a property set as (100, 100,000, 1). Sorted: 100,000, 100, 1. Amount
1,250: gold 0, silver 12, copper 50 → `Gold` hidden, `Silver` 12, `Copper` 50. With
`ForceShowAll` and `SuppressLeadingZeroes`: gold hidden (leading zero), silver 12, copper 50.
Amount 100,005 with the same options: gold 1, silver 0 (shown, after a larger non-zero coin),
copper 5.

<a id="s13-2"></a>
### 13.2 Buy modes

An item with `Inventory_MaxQuantity` 50. Shift held, right click on its entry → `Buy { entry,
quantity: 50 }`. Alt held → the quantity dialog with *M* = 50 opens showing 50 (no earlier
purchase from this vendor through the dialog). Half → 25. Deleting both characters → the
entry is empty, the quantity and cost stay at 25, OK is `ghosted`; OK or Enter now does
nothing. Typing 8 → 8; typing 0 → 80 is set as 50 at once, the entry shows 50; OK →
`Buy { …, quantity: 50 }`, the dialog closes. Opening the dialog again for an item of the same
vendor with *M* = 20 → it shows 20 (50 remembered, at most *M*); for an item of another vendor
→ it shows that item's *M*. Typing `x` into an empty entry → 0 → set as 1. With
`WalletChanged { Coin }` 400 and a unit price of 10: quantity 50 costs 500 → OK `ghosted`, the
cost display `TooExpensive`, Enter does nothing; Half → 25, cost 250 → OK `normal`. An item
with `Inventory_MaxQuantity` 1 and Shift held → quantity 1. Tab held, backpack stacks of that
record with quantities 47 and 10 → 3 + 40 = 43.

<a id="s13-3"></a>
### 13.3 Loot All

`LootShown` with entries 1, 2 and coin 30: rows coin, 1, 2; the button has its layout text.
Loot All → `TakeLoot { entry: None }`. The generic answer delivers `LootTaken` 1, `ItemAdded`,
`LootTaken` 2, `ItemAdded` in one tick; at the end of that frame every entry is taken and Loot
All is pending → `CloseWindow`; the next tick's `WindowClosed` hides the window. Without Loot
All, taking entries 1 and 2 one by one leaves the window open with the coin row and the
button relabelled.

<a id="s13-4"></a>
### 13.4 Reward rows

A quest with a treasure record holding two fixed items and one title, and two selectable items;
the feed sends `QuestRewards` with entries (`ItemXp` 500), (`Coin` 1,250), (`Reputation`
faction F, 300). Block: `QuestRewards_Header`, money (silver 12, copper 50), reputation (F,
positive), item experience 500, item 1, item 2, title, `QuestRewards_SelectableRewardsHeader`,
selectable item 1, selectable item 2. With one more feed entry (`VirtueXp` 300) and a treasure
record that also holds two `QuestTreasure_FixedVirtueArray` entries: the virtue experience row
(300) comes after the title and before `QuestRewards_SelectableRewardsHeader`; the two virtue
entries add no row.

<a id="s13-5"></a>
### 13.5 Window choice

`DialogueShown { text: Comment(c), options: [] }` → comment window. `DialogueShown { text:
Literal("…"), options: [] }` → conversation window with Continue. `DialogueShown { text:
QuestRole{…}, options: [1, 2] }` → choice window with the greeting row and two action rows in
order 1, 2.

---

<a id="s14"></a>
## 14. Open questions (summary) and owner checks

> **OPEN:** the owner rows below are numbered O110–O117 provisionally; the P3 close-out adds
> them to `docs/checklists/owner-observations.md` (another P3 spec may take numbers from O110
> as well; the close-out renumbers if they collide).

| # | Question | Section | Settles it |
|---|---|---|---|
| Q1 | Texts and conditions of the out-of-range and no-selection messages; which mouse inputs use an entity (confirm right click, double-click) | §3.1 | BIN (class 295 function 81) or owner (O110) |
| Q2 | Which native window shows which NPC conversation (comment, conversation, choice) | §5.1 | owner (O111) |
| Q3 | Whether windows close when the player walks away, and at what distance | §5.7 | owner (O111) |
| Q4 | How the choice list is assembled (sections, headers, order) | §5.3 | server-driven; owner (O111) |
| Q5 | Which objective the offer shows; virtue experience, title and emote row texts; whether fixed virtue ranks appear elsewhere | §6.2, §6.3 | BC (class 287 function 193 and its callers) or owner (O112) |
| Q6 | Whether a selectable reward must be chosen before Accept, and how the choice is sent | §6.5 | owner (O112); feed revision if needed |
| Q7 | Selling: drag onto the window, the sell price, Sell All's scope (locked, junk) | §7.6 | owner (O113); `economy.md` §11.6 |
| Q8 | Barter window: coin costs, info text format, mixed vendors | §7.7 | owner (O114) |
| Q9 | Loot: how money shows, the input that takes one entry | §8.2, §8.3 | owner (O115) |
| Q10 | Wallet: category and entry order, total format, cap state | §9.4 | owner (O116) |
| Q11 | Which currency fills which coin element (by value rank or by fixed records) | §9.2 | BIN (money display fill) or owner (O117) |
| Q12 | Which callers apply the player-variable filler to dialogue lines; the four unsupplied IDs | §5.5 | BC |
| Q13 | Fill mode's quantity (§7.4 case 4) | §7.4 | owner (O113) |

Owner checks (provisional numbers): O110 NPC use inputs; O111 dialogue windows and closing;
O112 quest-offer window; O113 vendor buying and selling; O114 barter window; O115 loot window;
O116 wallet; O117 coin display.

---

<a id="s15"></a>
## 15. Notes for other specs

- `interaction.md` §3.4: which input uses an entity is §3.1 here (note added there).
- `item-tooltips.md` §6.11 and §11: the parts of the worth template and the coin images are
  §9.2 here; the currency order caveat is §9.3 (notes added there).
- `economy.md` §7: the directory's two forms differ in order (§9.3); barter currencies are
  mostly wallet items (§9.5) (notes added there).
- `string-table.md` §8.2, §9.5: the class library supplies 20 player variables (§5.5) (note
  added there).
- `scenario-feed.md`: revision 5 (§10.2).
- `lua-plugin-api.md` §15: the wallet model is §9.6.

<a id="changelog"></a>
## Change log

| Date | Change |
|---|---|
| 2026-10-10 | First version (Review), for decision 0024 P3: NPC use input (BIN), window catalogue, dialogue windows and text rendering with the player-variable filler (BC), quest-offer window with reward order (BIN), vendor, quantity, sell, buy-back and barter windows (BIN for the controls), loot window (BIN), money display and coin images (BIN), currency order caveat, wallet window and wallet items, scenario-feed revision 5 summary, census categories, invariants, examples. |
| 2026-10-11 | Header only: review 1 (independent spec reviewer): changes requested, three blocking items (§7.5 rule 4 and §13.2, §6.3/§10.3 virtue-experience row, §5.3 rule list); status stays Review. No content changed. |
| 2026-10-11 | Recorded late (review 1 note): OPEN callouts added on 2026-10-11 by the P3 implementer and the P3 exit verifier in §4 (quantity-dialog and barter root placement), §5.2 (settings key), §5.3 (Enter on a focused row), §6.3 (`QuestReward_Runic`, parts per kind), §7.7 (`BarterReward`), §8.3 (dragging a row into a bag), §9.4 (`PaperItemEntry_Total`). |
| 2026-10-11 | **Revision 2** (status Revised), answering review 1. **Behaviour changes:** §7.5 rewritten from a second BIN pass: every character typed re-reads the entry and sets the quantity (clamped, entry rewritten at once); an empty entry keeps the quantity and OK or Enter on it does nothing; OK does nothing while the cost display is `TooExpensive`; new rules 7 (OK `ghosted` while the entry is empty or the cost exceeds the player's coin) and 8 (opening quantity *M*, or the last purchase's quantity for the same vendor); number reading stated; Tessera rule for coin before any `WalletChanged` and for "same vendor"; the §7.5 implementer OPEN is answered and removed. §6.3 step 2: the virtue experience row is the last row of the step (after emotes), conditioned in the original on the treasure's virtue-experience tier; fixed virtue-array entries are not shown (the original's list never reads that array); §10.3 moves `VirtueXp` out of the step-1 rows and its implementer OPEN is answered and removed. §13.2 and §13.4 examples revised; invariants NWI9, NWI10 added. **No behaviour change:** §5.3 implementer OPENs moved after rule 5 so rules 1–5 form one list; §9.2 note on the display's own `TooExpensive`; Q5 widened; the §6.3 exit-verifier OPEN's Tessera rule no longer names virtue entries; provenance BIN revision-2 pass, NW15 and NW17 revised. |
| 2026-10-11 | Header only: review 2 (independent spec reviewer, wrote no part of the spec): passed; status Revised → Accepted. No content changed. Non-blocking notes and the implemented behaviour revision 2 changes are in the review record. |
