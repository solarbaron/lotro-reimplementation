<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Travel and instance-finder windows: the stable-master travel list, started travel and arrival, the instance finder, its queue and invitation displays

| | |
|---|---|
| **Status** | **Accepted** (independent re-review 2026-10-13) |
| **Last updated** | 2026-10-13 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Independent spec reviewer (dirty-room session, AI-assisted), who wrote no part of this spec or of its P5 notes in `instances.md` and `map-data.md` |
| **Review record** | First version for decision 0024 P5 (client half only), exit criterion 3 ("travel and instance-finder scenarios render"). Independent review 2026-10-13 against CLEAN_ROOM.md §5.3: **changes requested (one blocking)**; status stays Review. The §5.3 contamination items all pass (every statement is a functional fact; no code or pseudo-code and nothing following a decompiler-listing structure; no copied prose or non-interoperability identifiers; no game text, assets, keys, hostnames, endpoints, account data or official-traffic dumps; nothing outside LEGAL.md §9.3 — the BIN row records that the network side of the travel and join requests was not followed, decision 0004, and only offline UI code was read). Byte/element layouts carry units (design rects); invariants TI1–TI8 are testable; genuinely unknown facts are OPEN, not guessed. Provenance header spot-checked by own read-only probe of the install (M1, 2026-10-13): client version resource reads build 4906.0071.1794.4035, binary SHA-256 prefix d22363eb9aff0fb5 and archive dates 2026-10-04 all match the OBS/BIN rows. **Blocking:** §8.1's census is marked Confirmed (TW21) but its `Unreferenced` cell (1, 0x70005F75) contradicts the section's own definition — 0x70005F75 is named by `TravelVersioning` (`TravelVersioning_DestinationPairArray`, the destination's `TravelVersioning_SwiftDestination`), so by the `OtherReference` definition it is `OtherReference`, not `Unreferenced`; the merged implementation's census already produces `OtherReference` 5 / `Unreferenced` 0, so the table also disagrees with the code it describes. Reconcile §8.1 (correct the counts to 5 / 0, or narrow the `OtherReference` definition) and close the inline §8.1 OPEN before Accepted; no change to implemented behaviour (the code follows the definitions). **Non-blocking:** (a) §9's `JoinInstance` `size`/`tier` are typed `u8` while the reconciled `scenario-feed.md` "P5 travel" revision types them `u32` (SF World/`JoinInstance`); §9 defers types and names to that revision at merge, so align §9 to it. (b) The spec's own mark scheme (BIN, BC, Probable, Decision) is broader than §5.1's Confirmed/Reported pair; this follows established project practice (`map-data.md`, `instances.md`) and is noted only so a reader treats BIN/BC facts as resting on M4 / class-library data, not as "observed in real data". The P5 notes in `instances.md` §15.1/§18 and `map-data.md` §8.8/§16 add no fact and are faithful cross-references to this spec (checked). **Re-review 2026-10-13 (a separate independent dirty-room session, AI-assisted, that wrote no part of this spec, of its P5 notes, or of the earlier review): passed, Accepted.** The blocking item is resolved by revision 7bbade9: §8.1 places 0x70005F75 in `OtherReference` (named as a `TravelVersioning_SwiftDestination` of a `TravelVersioning_DestinationPairArray` pair, and by no station route or home), widens the `OtherReference` definition to state that a pair's swift destination counts, closes the inline OPEN, and gives `OtherReference` 5 / `Unreferenced` 0 — self-consistent with the section's own definitions and with the merged census, which already produces 5 / 0. Confirmed by an own read-only M1 probe of the install (2026-10-13, `TESSERA_GAME_DIR`): the destination census reports Listed 740 (swift 389, slow 351), HomeOnly 22 (3 / 19), UnsoldRouteOnly 1 (0x7000EB70), OtherReference 5 (0x70005F75 among them), Unreferenced 0, RecordUnreadable 0, total 768; station rows 421 stations / 5,199 rows (1–40 per station), swift 3,079, slow 2,120, free 29, own home 12, with level 4,673 (below 2: 0), quest req 186, faction req 41, repeated 0, bad route / destination 0; routes 5,008 with 3 unsold (0x700094F9, 0x70009507, 0x7000EB72); names-not-table 0 — every §8.1 and §8.2-adjacent count reproduces. The §5.3 contamination checklist passes on a full read (functional facts only; no code, pseudo-code or decompiler-listing structure; no copied prose or non-interoperability identifiers; texts named by table and token, no game text, keys, hostnames, endpoints, account data or official-traffic dumps; the BIN work stays within decision 0011's offline-client scope and leaves the request network side unfollowed, decision 0004); provenance marks present; byte/element layouts carry units; TI1–TI8 testable; unknowns are OPEN. `instances.md` and `map-data.md` are both Accepted and their P5 notes are notes-only. The two earlier non-blocking items stand, neither a §5.3 violation and neither altering implemented behaviour: (a) §9 still types `JoinInstance`/`InstanceEntry` `size`/`tier` as `u8`, while the now-**Accepted** `scenario-feed.md` "P5 travel" revision types them `u32` (`size: Option<u32>`, `tier: Option<u32>`); §9 is an explicit proposal that defers types and names to that revision, and the code follows the Accepted `scenario-feed.md` (u32), so aligning §9 to `u32` is advisory editorial cleanup only, behaviour-neutral; (b) the BIN/BC/Probable/Decision mark scheme is broader than §5.1's Confirmed/Reported pair, following established project practice (`map-data.md`, `instances.md`). **Status → Accepted.** |
| **Implements into** | `crates/gamedata` (`tessera-gamedata`: the travel-row and finder-row models of §4.10 and §6.8, the census of §8), `crates/ui` (`tessera-ui` game widgets: the travel window, the end-travel box, the loading-screen presentation, the instance-finder page, the invitation window, the instance-travel window; module names chosen by the implementer, additive to the `game` module index), `apps/client` (`tessera-client`: wiring to the feed events, the commands of §9, the census run), `crates/net` (only through the additive feed forms that §9 proposes for the `scenario-feed.md` revision labelled "P5 travel"; this spec changes no feed text) |
| **Companion specs** | [`map-data.md`](map-data.md) (**MD**: §8 travel network, §8.8 the travel window's script functions), [`instances.md`](instances.md) (**IN**: §4 templates, §7 world join, §10 fellowship maker, §12 dialog singletons, §15.1 B1–B3, B8), [`scenario-feed.md`](scenario-feed.md) (**SF**: §5.6 windows, §6.5 map, travel, mounts and instances, §9 events and commands), [`interaction.md`](interaction.md) (**IA**: §3.4.5 NPC service rule, types 9 `NPC_Travel` and 21 `NPC_PrivateEncounterTravel`), [`npc-windows.md`](npc-windows.md) (**NW**: §3 using an NPC, §9.2 money displays), [`ui-layout.md`](ui-layout.md) (**UL**: layouts, `UILAYOUT`, §9 building panels, §17 opening), [`ui-widgets.md`](ui-widgets.md) (**UW**: §8 ListBox, §9 Tree, §11 Menu), [`ui-behaviour.md`](ui-behaviour.md) (**UB**: input routing, window manager), [`keymap.md`](keymap.md) (**KM**: §12 key maps), [`wlib-bytecode.md`](wlib-bytecode.md) (**WB**: §6.4 starting a function by ID), [`string-table.md`](string-table.md) (**ST**), [`chat-window.md`](chat-window.md) (**CW**: chat types, §10 Tessera's registry), [`map-windows.md`](map-windows.md) (**MW**, status Review: the map window and radar; out of scope here), [`mounts.md`](mounts.md) (mounted gaits; out of scope here) |

> **Naming note (interoperability).** Layout labels (`travel`, `fellowshipmaker`, `worldjoin`,
> `pe_travel`, `alert`, `gameplayactivatables`, `loadscreen`), element labels (`Travel_Window`,
> `FellowshipMakerLaunchNow_JoinButton`, …), widget-type labels, state labels
> (`Travel_DestinationDiscovered`, …), input-action labels (`ToggleWorldJoin`, …), property names
> and enum labels are the install's own identifiers, read from the user's data (the property
> dictionary 0x34000000, the enum mappers, the DID maps). They are quoted only so that an
> implementation can find the data by them. Numeric IDs, DIDs, string-table tokens and
> class-library function IDs printed next to them are this install's values (2026-10-11) and
> must be looked up by label at run time. No game text appears here: texts are named by string
> table and token.

---

<a id="s0"></a>
## 0. Purpose

P5 exit criterion 3 needs the travel window and the instance finder to render scenario data
as the original client presents them. This spec gives, from the install's UI layouts and
records (M1), the class library's code (M1) and, where the data stop, the offline client (M4,
decision 0011):

- the travel window a stable master opens: its element tree, how rows are listed, grouped,
  marked, searched and selected, what the selection shows (cost, free, minimum level, status,
  territory, area, swift), the go controls and closing (§4);
- how a started travel is presented (the end-travel box, the loading screen) and what
  arrival means for the client half (§5);
- the instance finder: which window the original uses, its entry tree (kind and category
  headers, instance groups, the featured instance), the size, tier and level controls, the
  join control, the queue and confirmation displays and the invitation window (§6);
- the chat and alert lines either window produces (§7);
- a census that puts every travel destination and every private-encounter template in one
  category, shown or a reason (§8);
- the scenario-feed forms the windows need, as an additive proposal (§9).

**Client half only** (decision 0024 §2, principle 4; LEGAL.md §11.1 rule 5): the original takes
the travel list, each row's cost, level, flags and usability, and every finder eligibility
test from class-library code it runs, and the outcome of a travel or join request from the
server. Tessera runs none of these rules: rows, prices, eligibility, queue states and arrival
come from the scenario feed (or a server), never from the install. The client computes only
presentation (layout, text rendering, grouping for display, search). Where the original's
choice is game code that was not traced, a **Tessera rule** stands in and is marked as such.

**Out of scope:** the "looking for instance" board on the right half of the finder page
(listings of other players; social, §6.7), the fellowship maker's group-finder pages (the
simple and advanced pages, matching; `instances.md` §10), the skirmish statistics panel
(`instances.md` §15.1 B4), the store and paper-item purchase paths (§4.8), mounted gaits
(`mounts.md`), the map window and radar (`map-windows.md`).

---

<a id="s1"></a>
## 1. Provenance

| Key | Source |
|---|---|
| **OBS** | The user's install (Steam, client build 4906.0071.1794.4035; archives unchanged since 2026-10-04, file times re-checked 2026-10-11), read in place and read-only with throwaway probes in the dirty-room scratch area. UI layouts decoded per `ui-layout.md` §4 with every property value: `travel` (UILAYOUT key 0x1000004E, record 0x2200002A), `fellowshipmaker` (0x100000D8, 0x2200084C), `worldjoin` (0x100000C1, 0x220007C2), `pe_travel` (0x10000015, 0x22000027), `alert` (0x1000006F, 0x2200001D), `gameplayactivatables` (0x10000051, 0x22000012), the template layout 0x2200008A (no `UILAYOUT` key), `loadscreen` (0x10000011, 0x2200000B), `radar` (0x1000003C, 0x2200003A); all 302 layout records searched for the element IDs named here. The DID maps 0x28000001 (`EMAPPER`), 0x28000003 (`UILAYOUT`), 0x28000004 (`STRINGTABLE`); the enum mappers named in the text; the key-map records 0x1D000000 and 0x1D000001 (`keymap.md` §12); with a probe over the repository's own decoders (tessera-props, main at 8e9670d): **every** object property set (prefix 0x79, 383,332) searched for the travel and instancing properties, giving all 421 travel-web records, 768 destinations, 5,008 routes, 437 NPCs with `TravelWebWC`, 56 holders of `PrivateEncounter_TravelArray`, all 1,511 private-encounter templates and the world-join control. |
| **BC** | The class library's code (record 0x56000000), data in the user's install (method M1), disassembled on 2026-10-11 with a throwaway probe over the repository's `tessera-wvm` (`wlib-bytecode.md`, Accepted): function 0x01CC0003 and the body it calls (the destination-info builder), 0x01CC0009 (end travel), 0x010C0001 (error display). Read only to see what the client receives and which values are computed; no message text was read. |
| **BIN** | **M4 under the standing authorisation of decision 0011.** Client `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (SHA-256 prefix d22363eb9aff0fb5), headless decompiler on a private copy of the shared analysis project (`~/tessera-re/work/p5tw`, deleted after use), 2026-10-11; functions found by instruction search for element IDs, input-action IDs and property IDs, and read by address range. **Questions:** (q1) how the travel window is built, which code supplies its rows and per-row values, and how rows are grouped, marked, ghosted, searched and selected; (q2) what the selection display and the go buttons show and do, and when the window closes; (q3) what the client shows during a ride and how it is ended; (q4) which window `ToggleWorldJoin` and `ToggleFellowshipMakerUI` open; (q5) how the instance-finder page builds its tree (headers, groups, featured instance) and fills its size, tier and level controls; (q6) what the join control checks and sends, and how the queue, confirmation and invitation displays follow the player's state; (q7) what the instance-travel window shows and does. **Why M1–M3 were not enough:** the layouts name the elements and the records hold the travel graph and the templates, but which code reads them, which values are computed, the grouping, ordering, ghosting and state rules leave no trace in the data; no published source describes these windows; the original cannot be observed in this session. Only offline UI code was read; the travel and join requests end in class-library calls whose network side was not followed (decision 0004). No address, listing or decompiler output appears in this spec. |
| **CMP** | The companion specs in the header (Accepted unless noted); their facts are cited with their own provenance. |
| **Design** | Tessera rules, marked **Tessera rule**: what Tessera does where the original's behaviour is server state, class-library game code not specified, or outside P5. Not claims about the original. |

Marks: **Confirmed** = checked over every record or element of the kind (OBS), or a BIN fact
the data agree with in every case; **BIN** = rests on the disassembly only; **BC** = rests on
the class library's code; **Probable** = supported but not proven in every case.

| # | Fact | Sources | Mark |
|---|---|---|---|
| TW1 | Window catalogue: layouts, keys, roots, widget types, scaling groups, rects, the row and header templates of the travel tree, the finder page parts, the invitation, instance-travel, alert, end-travel and loading-screen elements (§3, §4.2, §5.1, §6.2, §6.5, §7.2) | OBS | Confirmed 2026-10-11 |
| TW2 | The travel window is shown for an entity (the stable master); its title is `Travel_NameText` filled with a text of table `Travel` (key 0x10000042, 0x25000027), token 0x0269CC25, whose variable 0x5343 receives the entity's `Name` (§4.1) | BIN (q1); OBS (table, element) | BIN |
| TW3 | The travel window's rows, per-row values and tests all come from class-library functions started by ID: 0x01CC0001 (destination list for the entity), 0x01CC0026 and 0x01CC0006 (listing tests), 0x01CC0003 (destination info, a native object of class 3680 here), 0x01CC002E (route test), 0x01CC0007, 0x01CC0008, 0x01CC001B (usability tests), 0x01CC0005 (request test), 0x01CC0004 (travel request), 0x01CC0028 (paper-item path); the player tests 0x07B90011 and 0x018500E4 (§4.3) | BIN (q1, q2); BC (bodies exist) | BIN |
| TW4 | Destination info fields and their use: two DIDs (the route is the second), three texts (name, territory, area), cost, minimum level, a paper-item service key, and the flags swift, listed, requirements met (§4.3) | BIN (q1, q2: every read of the copied object) | BIN |
| TW5 | Row rules: which destinations get a row, the unknown-destination and failed-requirements icons, the swift icon, the search text, the ghosted state and overlay, re-selection after a rebuild (§4.4) | BIN (q1) | BIN |
| TW6 | Rows are grouped under one header per territory text, created at first use; nothing is sorted by the window; the tree inserts in call order (§4.5) | BIN (q1); CMP (UW §9: insertion appends) | BIN |
| TW7 | Search: rows whose search text does not contain the typed text are filtered; headers follow their children (§4.6) | BIN (q1); OBS (`UICore_Tree_auto_filter_parent_with_filtered_children` true) | BIN |
| TW8 | Selection display: free versus money cost, the minimum-level text and its fixed text below 2, the three route-status states, territory, area and swift label (§4.7) | BIN (q2); OBS (state labels, tokens) | BIN |
| TW9 | Go button: label and ghosting by the tests of TW3, the shapeshift and no-player reasons; a click or a double-click on a normal row requests travel and closes the window; paper-item and buy-now buttons share one position (§4.8, §4.9) | BIN (q2) | BIN |
| TW10 | The cost the class library hands the window is a single-precision product of a stored amount and a multiplier, plus 0.5, converted to an integer (§4.3) | BC (body of 0x01CC0003's callee) | BC; operands Probable |
| TW11 | Closing: the close button, a travel request, the source entity going away, and further game notifications (§4.9) | BIN (q2) | BIN (two notifications not identified) |
| TW12 | Instance-travel window: title = template name, intro text from the source entity and template, teleport-now sends a request and closes, teleport-later and close only close; showing it hides the invitation window (§4.11) | BIN (q7); OBS | BIN |
| TW13 | End-travel box: shown exactly while the local player's `Travel_IsTraveling` is true and its `Travel_Allow_Dismount` is not false (absent counts as true); its button starts class-library function 0x01CC0009 (§5.1) | BIN (q3); BC | BIN |
| TW14 | Default bindings: `ToggleWorldJoin` on Ctrl + J, `ToggleSkirmishPanel` on Ctrl + K, `ToggleMapPanel` on M (map `Main`, activation 0x03); `ToggleFellowshipMakerUI`, `ToggleFellowshipMakerSimple`, `ToggleFellowshipMakerAdvanced` have no default binding (§6.1) | OBS (key-map records; codes per KM §12.6, mask bit 2 = Ctrl) | Confirmed 2026-10-11 |
| TW15 | `ToggleFellowshipMakerUI` toggles `FellowshipMaker_Window`; `ToggleWorldJoin` opens it at page `FellowshipMaker_LaunchNowPage`; no code names `WorldJoin_Window` (§6.1) | BIN (q4: the panel registration table; instruction search for the element ID: no hit) | BIN |
| TW16 | Finder tree: kind headers from the entry's flag word masked with 0x18000F (8 when no kind bit), category headers from the template's `WorldJoin_EncounterCategory` (a fixed text for 0), one row per template or per instance group, row text = `PrivateEncounterTemplate_Name`; header and group texts from the display texts of mappers `FellowshipMakerConfiguration`, `WJEncounterCategory`, `WJInstanceGroup` (§6.3) | BIN (q5); OBS (EMAPPER keys 0x100000F5, 0x10000111, 0x1000014C) | BIN |
| TW17 | Join control: resolves the template (group and size by IN B3), builds a request object with level, tier and size where the template uses them, checks it with class-library function 0x0A2B0005 and sends it with 0x0A2B0004; failures are shown by 0x010C0001 (§6.4) | BIN (q6); BC | BIN |
| TW18 | Queue and confirmation displays follow the local player's `FellowshipMaker_PlayerState` (mapper `FellowshipMakerUI_PlayerState`, keys 1–7); the window also refreshes on `FellowshipMaker_DisallowUse`, `Travel_IsTraveling`, `LifeState`, `PrivateEncounter_Template_DataID`, `Combat_Violent`, `Trait_Manager_NPC` (§6.5) | BIN (q6); OBS (mapper) | BIN |
| TW19 | Invitation window: title = template name, intro text, level, tier and size fields (tier and size texts from mappers `SkirmishDifficultyTier`, `SkirmishGroupSize`), skirmish or classic detail label by `WorldJoin_EncounterType`, a count-down timer shown in one invitation state (§6.5) | BIN (q6); OBS | BIN |
| TW20 | Alert fields `WorldJoinTravelField`, `WorldJoinInvitationField`, `WorldJoinQueueField`, `FellowshipMakerWaitingInQueueAlertField` and the alert types 38, 39, 41, 58 of mapper 0x23000033; the last field carries one text per player state (§7.2) | OBS | Confirmed 2026-10-11 |
| TW21 | Travel census (§8.1) | OBS | Confirmed 2026-10-11 |
| TW22 | Template census (§8.2) | OBS | Confirmed 2026-10-11 |
| TW23 | Every rule marked **Tessera rule** | Design | Decision |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Station** | A travel-web record (MD §8.2); its `TravelDestinationRecordArray` lists (route, cost) entries. |
| **Row** | One entry of the travel window's tree: a destination reached by one route. |
| **Header** | A non-selectable tree node grouping rows (the travel window's territory headers; the finder's kind and category headers). |
| **Source entity** | The entity a window belongs to: the stable master of a travel window, the NPC of an instance-travel window. |
| **Template** | A private-encounter template (IN §4). |
| **Finder** | The instance finder: page `FellowshipMaker_LaunchNowPage` of `FellowshipMaker_Window` (§6.1). |
| **Group row** | A finder row standing for an instance group of the world-join control (IN §7.1, B3). |
| **Feed window** | A window of the scenario feed (SF §5.6) of kind `travel` or `instances`, identified by `WindowId`. |
| **Coin amount** | An amount in units of the smallest denomination (`economy.md` §7); shown with the money display of NW §9.2. |

---

<a id="s3"></a>
## 3. Window catalogue (Confirmed, TW1)

| Window | Layout (key, record) | Root element | Widget type | Rect (design) | Notes |
|---|---|---|---|---|---|
| Travel window | `travel` (0x1000004E, 0x2200002A) | `Travel_Window` | UIScaler, group `Panels`, origin (223, 324) | (8, 117) 446 × 649 | `save_loc`, `activate_on_show`, `blocks_clicks`, object mode `Stretch` |
| Fellowship maker (holds the finder) | `fellowshipmaker` (0x100000D8, 0x2200084C) | `FellowshipMaker_Window` | UIScaler | (0, 5) 930 × 580 | panel `FellowshipMaker_Panel` with the pages of §6.2 |
| Invitation window | `worldjoin` (0x100000C1, 0x220007C2) | `RootWJInvitationField` | UIScalerPanel, group `Panels`, origin (182, 289) | (535, 117) 364 × 578 | |
| World-join window (not used, §6.1) | `worldjoin` | `WorldJoin_Window` | Field | (160, 117) 364 × 521 | encounter and skirmish tabs |
| Instance-travel window | `pe_travel` (0x10000015, 0x22000027) | `RootPETravelField` | UIScalerPanel, group `Panels`, origin (206, 237) | (26, 118) 412 × 474 | |
| End-travel box | `gameplayactivatables` (0x10000051, 0x22000012) | `GamePlay_EndTravel_Activatable` (a `HiddenDragBox_Template`, root element, group `Menus`) holding `GamePlay_EndTravel` (template `EndTravel_Template` of record 0x2200008A, widget type `EndTravel`, hidden by default) | | (782, 568) 174 × 91 | |
| Loading screen | `loadscreen` (0x10000011, 0x2200000B) | `LoadScreenMain` | Field | 2560 × 1080 | image `LoadScreenImage`, meter `LoadScreenMeter`, ring `Loading_Anim`, label `LoadingProgressUIElement_Label` |
| Alert fields | `alert` (0x1000006F, 0x2200001D) | §7.2 | | 299 × 48 each | base `OverflowAlert` |

Element labels present in the install's `UIElementID` mapper but in no layout:
`Travel_WindowLabel`, `Travel_GoButton`, `Travel_SelectionStatus`, `Travel_DestinationCost`,
`Travel_StatusText`, `WorldJoin_Tabs`, `WorldJoin_CreateTab`, `WorldJoin_JoinTab`. They are
not built.

---

<a id="s4"></a>
## 4. Travel window

<a id="s4-1"></a>
### 4.1 Opening (BIN, TW2)

The window is shown for one entity, the stable master the player used (an NPC whose use type
is 9 `NPC_Travel`, IA §3.4.5 row 7: it stores `TravelWebWC`). Which message makes the client
show it is game or server traffic and was not read (decision 0004). On show the window keeps
the entity, sets the title (TW2) and builds its rows (§4.3). It is a native panel of the
travel layout; the client creates it once and shows and hides it.

**Tessera rule.** A feed `TravelShown` (SF §9.2) opens or refreshes the window; its `source`
is the source entity (absent: the title shows the fixed text of §4.10 rule 1). Use of an NPC
sends `UseEntity` as for every NPC (NW §3); the scenario answers with `travel-list`.

<a id="s4-2"></a>
### 4.2 Element tree and roles (TW1)

Positions are design rects relative to the parent.

| Element | Type / base | Rect | Role |
|---|---|---|---|
| `Travel_Window` | UIScaler | 446 × 649 | root |
| ↳ `TravelWindow_BaseBox_BG` | `Box_Silver_BlueGradient` | (1, 17) 441 × 603 | frame |
| ↳ `TravelWindow_BG` | Field, image 0x411348A7 | (8, 23) 428 × 590 | background |
| ↳ `Travel_MoveBar` | Dragbar | (7, 2) 428 × 24 | move |
| ↳ `TravelWindow_BaseBox_TitleBar` ↳ `Travel_NameText` | `titlebarfont`, centred | (14, 15) 228 × 20 | title (TW2) |
| ↳ `Travel_CloseButton` | `Button_X` | (419, 25) 16 × 16 | close |
| ↳ `Travel_DestinationListField` | template `TravelListBoxTemplate` | (15, 39) 414 × 340 | list (below) |
| ↳ `Travel_Divider_Horizontal` | Field | (24, 380) 400 × 26 | decoration |
| ↳ `Travel_Stats_BG_Bar` | Field | (15, 402) 414 × 20 | bar behind the three status texts |
| ↳ `Travel_Currency_MinLevel` | `fieldlabelsmall`, centred | (15, 402) 126 × 20 | minimum level (§4.7) |
| ↳ `Travel_RouteStatus` | `fieldlabelsmall`, 3 states | (142, 402) 167 × 20 | route status (§4.7) |
| ↳ `Travel_SwiftTravel` | `fieldlabelsmall`; text token 0x0F4A924C of `Travel` | (312, 402) 117 × 20 | swift label (§4.7) |
| ↳ `Travel_Territory_Field` (`Travel_Territory_Label`, `Travel_Territory`) | Field with two texts | (20, 427) 405 × 20 | territory of the selection |
| ↳ `Travel_Area_Field` (`Travel_Area_Label`, `Travel_Area`) | Field with two texts | (20, 450) 405 × 20 | area of the selection |
| ↳ `Travel_Divider_Horizontal_2`, `Travel_Button_Divider`, two `Travel_Currency_Divider_TravelStats_*` | Fields | | decoration |
| ↳ `Travel_Currency_GoButton` | `Button_General_Use`, text token 0x0EC8FB7F of `Travel` | (13, 493) 202 × 77 | go with coin (§4.8) |
| ↳↳ `Travel_Currency_DestinationCost` | `MoneyDisplay_MoneyTemplate` (NW §9.2), tooltips off | (34, 40) 86 × 24 | cost |
| ↳↳ `Travel_Currency_DestinationCost_Free` | `fieldlabelsmall`, token 0x08FB7675 of `Travel` | (59, 40) 86 × 20 | free |
| ↳↳ `Travel_CurrencyButtonLabel_NoRouteSelected` | text token 0x07F54814 of `Travel` | 202 × 77 | label without selection |
| ↳ `Travel_PaperItem_GoButton` | `Button_General_Use`, token 0x0F506DCF | (230, 494) 202 × 77, flag 1 | go with a paper item (§4.8) |
| ↳↳ `Travel_PaperItem_GoButton_Coin` (hidden), `Travel_PaperItem_Field` (cost container, icon, count text), `Travel_PaperItemButtonLabel_NoRouteSelected` | | | paper-item parts |
| ↳ `TravelWindow_BuyNowButton` | `Button_General_Use_BuyNow`, `WebStoreUI_Category` `WS_MithrilCoin` | (230, 494) 202 × 77 | store (§4.8) |
| ↳ `TravelWindow_PlayerMoney` (+ `_Border`) | `MoneyDisplay_PlayerMoney_YourMoneyTemplate` | (16, 575) 264 × 28 | the player's coin |
| ↳ `TravelWindow_PaperItemBalance` | `PaperItemServiceBalance_Base`, `UI_PaperItem_PaperItemService` = `TravelRoutes` (2) | (343, 574) 87 × 33 | paper-item balance |
| ↳ `Travel_NoDestinationField` ↳ `Travel_NoDestination_Label` | `Box_Silver_Tooltip`; text token 0x04163FDC of `UI` (0x250001BB) | (1, 17) 441 × 603, z −2 | shown while the list is empty (§4.4) |

The list template `TravelListBoxTemplate` → `TravelListBoxTemplate_Field` holds:
`TravelList_Search_Label` (token 0x0145AAA4 of table 0x250001D9), `TravelList_SearchBG`,
`TravelList_SearchText` (editable, one line, at most 80 characters, loses focus on accept),
`Travel_Expand_All_Button` and `Travel_Collapse_All_Button` (ExpandCollapseButtons on
`TravelListBox`), `TravelListBox_BG`, the tree `TravelListBox` (Tree, one column,
`click_select`, selected state `highlight`, normal state `normal`, `auto_expand` true,
`auto_filter_parent_with_filtered_children` true, indent 0, icon template
`FellowshipMaker_Tree_ExpandBox_Tier1` of layout 0x2200084C) and its scroll bar
`TravelScrollbar`.

**Row template** `Travel_ListItem` (Button, 400 × 22, seven states with images: `normal`,
`normal_rollover`, `normal_pressed`, `highlight`, `highlight_rollover`, `highlight_pressed`,
`ghosted`), children in order: `Travel_ListItem_DestinationIcons_UnknownDestination_Icon`
(hidden, mouse-invisible, 400 × 22), `Travel_ListItem_DestinationIcons_FailedRequirements_Icon`
(the same), `Travel_ListItem_DestinationIcons_Field`, `Travel_GhostedOverlay` (hidden,
image 0x41134868), `Travel_ListItem_SwiftTravel` (20 × 20 at (9, 3), image 0x411348B1, tooltip
token 0x0925F034 of `Travel`), `Travel_List_Box` (hidden), `Travel_ListDestinationText`
(base `Travel_ListFont`: `fieldlabelsmall`, colour 0xFFCC9933 in `normal`, 0xFFFFFFFF in
`highlight`; at (31, 4) 367 × 20, truncates with an automatic tooltip).

**Header template** `Travel_ListItem_Header` (`fieldlabellarge`, white, left, top margin 10).

<a id="s4-3"></a>
### 4.3 Where the rows come from (BIN, TW3, TW4, TW10)

The original does not read the station record itself. It starts class-library functions by
ID (WB §6.4) with the source entity as argument (the class library maps the entity to its
station; MD §8.8):

1. **List.** 0x01CC0001 returns the destination DIDs, in an order the window keeps.
2. **Per destination *d*:** test A = 0x01CC0026 (entity, *d*); test D = 0x01CC0006 (*d*). A
   destination with neither gets no row. D is the **discovered** test (§4.7).
3. **Info.** 0x01CC0003 (entity, *d*) returns a success flag and an info object (native class
   3680 here) with the fields below; then 0x01CC002E (route) must hold.
4. A row is created when the info call succeeded **and** its *listed* flag is set.

| Info field | Window use |
|---|---|
| DID 1 | not read by the window |
| route (DID 2) | stored on the row (`UI_Travel_RouteDID`); re-selection, go (§4.8) |
| name (text 1) | row text; search text |
| territory (text 2) | header text (§4.5); `Travel_Territory`; search text |
| area (text 3) | `Travel_Area`; search text |
| cost (int) | §4.7 |
| minimum level (int) | §4.4 icon test, §4.7 text |
| paper-item service key (int) | §4.8; 0 = none |
| swift (flag) | swift icon, swift label, search text |
| listed (flag) | step 4 |
| requirements met (flag) | §4.4 icon test |
| fourth flag | not read by the window |

The destination DID is stored on the row as `UI_Travel_DestinationDID`.

**Cost (BC, TW10).** The info builder sets the cost to the single-precision product of a
stored amount (an attribute of the station's entry, by inference `TravelDestinationCost`) and
a multiplier computed by another class-library body, plus 0.5, converted to a signed integer.
The multiplier's body was not specified; it is where `Travel_DiscountArray` and
`TravelWebSellMultiplier` are read (MD §8.8, MD41). **Tessera does not compute it**: the cost
comes from the feed (§4.10).

**Rebuilds.** The window rebuilds the whole list when it is shown and whenever one of the local
player's watched properties changes (MD39). Before a rebuild it shows
`Travel_NoDestinationField`; after it, the field is hidden when the tree holds any node.

<a id="s4-4"></a>
### 4.4 Rows (BIN, TW5)

For each created row, in list order:

1. **Marks.** With *L* the local player's level (from player function 0x018500E4), the
   failed-requirements icon is shown when *L* < minimum level or *requirements met* is false.
   The unknown-destination icon is shown when test D failed (an undiscovered destination
   listed only because of test A).
2. **Search text** = name, then area, then territory, then (when *swift*) the swift label's
   text, concatenated.
3. **Swift icon** `Travel_ListItem_SwiftTravel` is shown exactly when *swift*.
4. **Text** `Travel_ListDestinationText` = name; the row's height follows the text's height.
5. **Header.** The row goes under the header for its territory (§4.5).
6. **Ghosting.** The row is put in state `ghosted` and `Travel_GhostedOverlay` is shown when
   test A failed **and** any of these holds: the destination or the route is invalid, station
   test 0x01CC0007 fails, destination test 0x01CC0008 fails, route test 0x01CC001B fails —
   except when the row has a paper-item service key and the player test 0x07B90011 fails (then
   the row stays normal).
7. **Re-selection.** A row that is not ghosted and whose route equals the route selected
   before the rebuild is selected again.

After all rows, the tree scrolls to the selected row.

<a id="s4-5"></a>
### 4.5 Grouping and order (BIN, TW6)

Rows are grouped under one header per **territory text**: a header (`Travel_ListItem_Header`,
text = the territory, also its search text) is created the first time a territory occurs and
inserted at the top level; later rows with an equal text go under it. Headers appear in the
order of their first row; rows keep list order inside their header. The window sorts nothing.
Equality is whole-text equality.

<a id="s4-6"></a>
### 4.6 Search, expand and collapse (BIN, TW7)

Text typed in `TravelList_SearchText` (on each character input or key press) becomes the
filter. A row is filtered out when the filter is not empty and the row's search text does not
contain the filter; headers are never tested themselves and are hidden by the tree when all
their rows are filtered (`auto_filter_parent_with_filtered_children`). After filtering the tree
lays out again. The two expand/collapse buttons act on every header (`ui-widgets.md` §9).

> **OPEN:** whether "contains" ignores case. The comparison routine was not read. **Tessera
> rule:** case-insensitive comparison of the lower-cased texts (ASCII and the Unicode simple
> lower-case mapping).

<a id="s4-7"></a>
### 4.7 Selection display (BIN, TW8)

On every selection change, rebuild and show, the window fills:

| Element | No selection | Selection with info *i* |
|---|---|---|
| `Travel_Currency_DestinationCost` | hidden, amount 0 | shown with *i*.cost when cost ≠ 0 (NW §9.2 coin split), else hidden |
| `Travel_Currency_DestinationCost_Free` | hidden | shown when cost = 0 |
| `Travel_Currency_MinLevel` | hidden | text token 0x0682DDAC of `UI` with variable 0x50AA9C = minimum level; when the level is below 2 the variable is the text of token 0x0B8C7995 of `UI` instead |
| `Travel_RouteStatus` | shown, state `Travel_NoRouteSelected` | state `Travel_DestinationDiscovered` when test D held, else `Travel_DestinationUndiscovered` |
| `Travel_Territory_Field`, `Travel_Area_Field` | hidden | shown with territory and area |
| `Travel_SwiftTravel` | hidden | shown exactly when *swift* |

Each state of `Travel_RouteStatus` sets its own text entry (tokens 0x07F54814, 0x0250F084,
0x0320D014 of `Travel`).

<a id="s4-8"></a>
### 4.8 Go controls (BIN, TW9)

**Coin go button** (`Travel_Currency_GoButton`):

- No selection: state `ghosted`, `Travel_CurrencyButtonLabel_NoRouteSelected` shown, button
  text token 0x0481FBC4 of `Travel`.
- Selection: the label is hidden; the button is `ghosted` unless the destination is
  discovered (test D) **and** station test 0x01CC0007, destination test 0x01CC0008 and route
  test 0x01CC001B (with flag 0) hold, in which case it is `normal`. The text token is 0x0740F624
  (undiscovered), 0x0768ADC4 (station test failed), 0x0245D7C4 (destination test failed),
  0x0E2C21E4 (route test failed) or 0x01D0CE24 (usable), all of `Travel`.
- Whatever the above, with no local player or when the player's `Shapeshift_MeshIID` is set,
  the button is `ghosted` and its text is a reason text chosen by key (0x1000034C and
  0x100006CD respectively; the routine that renders the key was not traced).

**Paper-item go button and buy-now button** occupy the same rect (230, 494). The buy-now button
is shown when the store test of the paper-item service (the row's key, else `TravelRoutes` 2)
fails and (test A held or the row has a key and player test 0x07B90011 fails); the paper-item
button is shown otherwise. Their labels and the paper-item cost count (token 0x0A6FE14C,
variable 0x539D) follow tests of the paper-item services. A click on the paper-item button
either opens the store path (class-library 0x01CC0028) or requests travel with the paper flag.

**Tessera rule:** P5 hides `Travel_PaperItem_GoButton`, `TravelWindow_BuyNowButton` and
`TravelWindow_PaperItemBalance` (no store, no paper-item services; decision 0024 scope). The
coin go button follows §4.10.

<a id="s4-9"></a>
### 4.9 Confirmation and closing (BIN, TW9, TW11)

- **Go.** A click on the coin go button, or a double-click on the selected row while it is in
  state `normal`, runs request test 0x01CC0005 (entity, destination, paper flag) and, when it
  holds, the travel request 0x01CC0004 (player, entity, route, paper flag); then the window
  closes. There is no confirmation dialog; the request's answer is not shown by the window.
- **Close.** `Travel_CloseButton` closes it. It also closes when the source entity goes away,
  and on two further game notifications that were not identified. Closing clears the entity
  and the selection.

<a id="s4-10"></a>
### 4.10 Tessera rules: rows from the feed

Tessera builds the same window from feed values; it computes no list, test, price or level.

1. **Title.** `source` present and resolvable to a name: the title of TW2 with that name. Else
   the variable is left unset and the title is the table text with its variable rendered empty
   (ST §9 rules).
2. **Rows.** One row per `TravelShown` entry, in entry order. An entry's `destination` names a
   destination record (MD §8.3) or a route record (MD §8.4.1); for a route the destination is
   its `TravelRoute_Destination`.
3. **Values per row** (feed first, install for display data only):

   | Value | Source |
   |---|---|
   | name | the destination's `TravelDisplayName`; a literal ref's text |
   | cost | the entry's `coin` (never computed) |
   | swift | the destination's `TravelDestinationIsSwiftTravel` |
   | minimum level | the entry's `min_level` (§9); without it, the route's `Usage_MinLevel` when the entry names a route; else 0 |
   | discovered, requirements met, usable | the entry's flags (§9); without them: discovered, met, usable |
   | territory, area | the entry's `area` ref (§9): its name is the area text and its parent territory's name the territory text (`wstate.md` §11); without it both are empty |

4. **Marks and states** follow §4.4 rules 1, 3, 6 and §4.7–§4.8 with the feed flags in place of
   the tests: failed-requirements icon when level > player level or not met; unknown icon when
   not discovered; ghosted when not usable; coin go button `normal` only when discovered and
   usable. The reason texts of §4.8 are not used: a ghosted go button keeps token 0x0740F624
   when not discovered and 0x0481FBC4 otherwise (Tessera rule, no test identity known).
5. **Grouping.** Rows with an empty territory are inserted at the top level, before any header
   (Tessera rule); others follow §4.5.
6. **Go.** The coin go button and the double-click send `ChooseTravel { window, entry }`
   (SF §9.3) and close the window locally only when the feed answers (`WindowClosed` or
   `TravelStarted`). Tessera rule: the client does not close on its own, so a scenario can
   refuse.
7. **Empty list.** `Travel_NoDestinationField` is shown while the window has no row.

<a id="s4-11"></a>
### 4.11 Instance-travel window (BIN, TW12)

An NPC whose use type is 21 `NPC_PrivateEncounterTravel` (IA §3.4.5 row 20: it stores
`PrivateEncounter_TravelArray`) leads to the instance-travel window for **one** template:

| Element | Role |
|---|---|
| `RootPETravelField` (UIScalerPanel) | root; `save_loc`, `activate_on_show`, `blocks_clicks` |
| `RootPETravel_BaseBox_BG`, `RootPETravel_BaseBox_TitleBar` ↳ `PETravelTitle_Label` | frame and title = the template's `PrivateEncounterTemplate_Name` |
| `PETravelDisplayField` ↳ `PETravelDisplayListBox` (ListBox with scroll bar) | holds one `PETravelDispenserIntroText` (NarrativeFont, markup, default token 0x07694813 of table 0x25033E35) filled with an intro text built from the source entity and the template by a routine not traced |
| `PETeleportField` (template `PETeleportFieldReference`) | `PETeleportNowButton` (token 0x02212BCC of 0x25033EC8), `PETeleportLaterButton` (token 0x0576EF5C) |
| `PETravelCloseButton`, `MoveBar` | close, move |

Teleport-now sends a request with the entity and the template (not followed) and closes;
teleport-later and close only close. Showing the window hides `RootWJInvitationField`. Which
template of the array the window shows is decided outside the window.

**Tessera rule:** an `instances` feed window whose proposed `source` (§9) names an entity
holding `PrivateEncounter_TravelArray`, with exactly one entry, is shown as this window;
teleport-now sends `JoinInstance { window, entry: 1 }`. The intro text is the template's
`PrivateEncounterTemplate_Description` (IN §4.2) until the original's routine is specified.

> **OPEN:** (implementer, 2026-10-11, `apps/client` `travel::present`, `travel::hub`) The rule
> cannot be tested for a source whose body record is unknown: a scenario actor
> (`actor $x kind=npc`, SF §5) has no body, nor has any source in a run without an install. The
> client reads "unknown" as holding the array (a single-entry window from such a source is the
> instance-travel window) and shows the finder only when the source's known body lacks it.
> Should an unknown body instead give the finder when an install is present, and how would a
> scenario name a holder then?

---

<a id="s5"></a>
## 5. Started travel and arrival

<a id="s5-1"></a>
### 5.1 In the original (BIN, OBS; TW13)

- A swift destination is reached by a short ride, a hop and a short ride, or a hop to a pad;
  a slow one by a chain of segments (MD §8.4.2). The path, timing and movement are server or
  data-driven behaviour (MD §8.5, §8.6 OPENs).
- While the local player's `Travel_IsTraveling` is true and its `Travel_Allow_Dismount` is not
  false (absent counts as true; both must be booleans, type 21), the client shows the
  end-travel box `GamePlay_EndTravel`: `EndTravel_Box` (`Box_Silver_Tooltip`, 165 × 65),
  `EndTravel_Text` (token 0x00D287E4 of `UI`) and `EndTravel_Button` (token 0x0B961B54 of `UI`).
  It re-tests on every change of either property. A click on the button starts class-library
  function 0x01CC0009 on the local player (it calls one further body and returns; what it
  sends was not followed).
- A change of position across loaded regions shows the loading screen (`loadscreen`): the
  stretched `LoadScreenImage` (default image 0x41007E06), the meter `LoadScreenMeter` (child
  image 0x41007E25, direction Left, no smooth movement), `Loading_Anim` and the label
  `LoadingProgressUIElement_Label` (token 0x07525AE7 of `UI`). When it is shown is the
  teleport path's rule (`walker.md`), not the travel window's.
- The client posts no chat line of its own when travel starts or ends (no text reference in
  the window code; lines come from the server or the class library).

<a id="s5-2"></a>
### 5.2 Tessera presentation of `TravelStarted` (Tessera rules)

`TravelStarted { destination, duration_ms }` (SF §9.2) starts a **travel presentation** that
lasts *duration_ms* or until the next `teleport` (`LocationReport` cause `Teleported`) or
`TravelEnded` (§9), whichever comes first:

1. **Swift destination** (`TravelDestinationIsSwiftTravel` true, or a route ref whose
   destination is swift): the loading screen of §5.1 is shown at once; its meter fills linearly
   from 0 to 1 over the duration; it hides at the end.
2. **Slow destination** (false): the end-travel box is shown for the duration; the walker keeps
   its view; `Travel_IsTraveling` is set on the client's local player model for that time.
3. **Duration 0:** nothing is shown.
4. **End-travel button:** sends `Dismount` (SF §9.3; no new command) and ends the presentation
   only when the feed answers with `TravelEnded` or a teleport.
5. **Arrival:** the client never moves the player for a travel; a scenario places the player
   with `teleport` (SF §6.5). When the duration ends without a teleport, the presentation ends
   where the player is.
6. **Chat line:** none is posted (the original posts none, §5.1); a scenario can post its own
   lines.
7. A `TravelStarted` while a presentation runs replaces it.

> **OPEN:** (implementer, 2026-10-11, `apps/client` `travel::ui`) Rule 1 needs the
> `loadscreen` layout; without an install (and so without its image and meter) Tessera has
> no fallback loading screen, so a swift travel in a run without an install is presentation
> state only (its timing is kept; nothing is drawn). The fallback windows of decision 0031
> cover the travel window, the finder, the instance-travel window, the end-travel box and
> the queue alert only. Please say whether a fallback loading screen is wanted (and its
> parts), or that none is drawn.
>
> (P5 exit verifier, 2026-10-12) With the install, the loading screen shows its image but
> not its meter, label or animation: at a 1600 × 900 screen the layout rules of
> `ui-layout.md` §8 as Tessera applies them shrink `NormalBackground` (edges 5, 5, 5, 5, under
> the aspect-fitted `LoadScreenImage`) to 597 × 588 and give `LoadScreenMeter` and
> `LoadingProgressUIElement_Label` negative sizes (−228 × −166 and −222 × −160) below the
> screen. This depends on the open question of `ui-layout.md` §8 (children of an element that
> owns a Stretch surface); see the note added there.
>
> (implementer, 2026-10-13, `crates/ui`) Pending that decision, the run-time rule now rescues
> an element whose computed box has a non-positive width or height (the Tessera fallback added
> to `ui-layout.md` §8.4 step 6 OPEN): the meter and the label take their design extent inside
> the shrunken `NormalBackground`, so the loading screen now draws its meter and label as well
> as its image. This is a stop-gap for visibility, not the pixel-accurate placement of the
> "other reading"; the owner's choice of reading still stands.

---

<a id="s6"></a>
## 6. Instance finder

<a id="s6-1"></a>
### 6.1 Which window (BIN, OBS; TW14, TW15)

The original's instance finder is a page of the fellowship maker: `ToggleFellowshipMakerUI`
toggles `FellowshipMaker_Window`, and `ToggleWorldJoin` (default Ctrl + J) opens it at
`FellowshipMaker_LaunchNowPage` (its tab `FellowshipMaker_LaunchNowTab`). The `worldjoin`
layout still holds `WorldJoin_Window` with encounter and skirmish tabs (pages `WJCreatePage`
with `WorldJoin_EncounterType` `Classic` 4 and `Skirmish` 3, entry template
`WorldJoin_EntryTemplate`), but no code of this build names its root, so it is never built;
Tessera does not build it either (Tessera rule). The same layout's invitation window is used
(§6.5).

<a id="s6-2"></a>
### 6.2 Window and finder page (TW1)

`FellowshipMaker_Window` (930 × 580): `FellowshipMaker_TitleBar` ↳ `FellowshipMaker_WindowLabel`,
`FellowshipMaker_TutorialHintButton`, `FellowshipMaker_CloseButton`, `FellowshipMaker_MoveBar`,
`ResizeBarBottom`, and `FellowshipMaker_AttributeMap` (PropertyAttributeMapField) ↳
`FellowshipMaker_Panel` (Panel) with the tabs and pages `FellowshipMaker_SimpleTab` /
`FellowshipMaker_JoinPage`, `FellowshipMaker_AdvancedTab` / `FellowshipMaker_AdvancedPage`,
`FellowshipMaker_WaitingTab` / `FellowshipMaker_ConfirmPage`, `FellowshipMaker_LaunchNowTab` /
`FellowshipMaker_LaunchNowPage`.

The finder page `FellowshipMakerLaunchNowPage` (widget `FellowshipMakerLaunchNowPanel`,
930 × 530):

| Element | Type / base | Rect | Role |
|---|---|---|---|
| `FellowshipMakerLaunchNow_Header` ↳ `FellowshipMakerLaunchNowPage_Header_Label` | text token 0x026A8B0C of `UI_FellowshipMaker` (key 0x10000090, 0x2501D901) | (2, 5) 334 × 33 | page heading |
| ↳ `FellowshipMakerLaunchNow_SizeLabel`, `…_SizeMenu` | label token 0x049C28EC; Menu `Menu_01` | (50, 41) 53 × 25 | group size (§6.4) |
| ↳ `FellowshipMakerLaunchNow_TierLabel`, `…_TierMenu` | label token 0x088C340C; widget `FellowshipMakerTierMenu` (popup layout 0x22000080) | (162, 41) 53 × 25 | difficulty tier |
| ↳ `FellowshipMakerLaunchNow_LevelLabel`, `…_LevelField` | label token 0x0BF6303C; editable text, 3 characters, states `normal` / `ghosted` | (284, 41) 47 × 20 | level |
| `FellowshipMaker_Instance_ListBox` | ListBox, entry templates `FellowshipMakerLaunchNowFeaturedInstanceField` then `FellowshipMakerLaunchNow_EncounterTreeField` | (0, 70) 353 × 415 | featured field and tree |
| `FellowshipMakerLaunchNow_JoinButton` ↳ `…_JoinButtonLabel` | `Chargen_Buttons_Triskel`, label token 0x0FAECA7C, tooltip 0x0B2CF854 | (100, 488) 152 × 38 | join (§6.4) |
| `FellowshipMakerLaunchNow_LFInstanceFinder` | Field | (353, 0) 565 × 530 | listings board (§6.7) |

`FellowshipMakerLaunchNowFeaturedInstanceField` (353 × 77):
`FellowshipMakerLaunchNow_FeaturedInstance_EncounterDisplay` (Button, eight states;
`normal` and `ghosted` carry their own tooltip and image) with `FellowshipMaker_EncounterText`
and `FellowshipMaker_EncounterDisplayBase_Checkmark`, `…_FeaturedInstance_Label` (token
0x06C30B6C), `…_FeaturedInstance_ResetButton` (28 × 28).

`FellowshipMakerLaunchNow_EncounterTreeField` (353 × 415): the tree
`FellowshipMakerLaunchNow_EncounterTree` (widget `FellowshipMakerMultiStateTree`, 314 × 377),
expand-all and collapse-all buttons, `…_Search_Label`, `…_SearchBG`, `…_SearchText`, a scroll
bar. Row template `FellowshipMaker_EncounterDisplayBase` (314 × 22; `FellowshipMaker_IndentElement`
↳ `FellowshipMaker_EncounterText`, `…_Checkmark`); header templates
`FellowshipMaker_EncounterDisplayHeader` (level 1) and `FellowshipMaker_EncounterDisplayHeader_Tier2`
(level 2).

<a id="s6-3"></a>
### 6.3 The entry tree (BIN, TW16)

The page builds the tree from a list of finder entries (template plus flag word of mapper
`FellowshipMakerConfiguration`; IN §10.2, B8). For each entry whose template passes
class-library test 0x0A2B002B:

1. **Group or template.** When the template belongs to an instance group (IN §7.1, B3), the
   entry is attached to that group's row, created once per group with the display text of the
   group label in mapper `WJInstanceGroup` (EMAPPER key 0x1000014C; 0x230003D1 here) and the
   group key stored as `FellowshipMakerUI_InstanceGroup`. Otherwise a template row is created
   with text = the template's `PrivateEncounterTemplate_Name`, `FellowshipMakerUI_EncounterDataID`
   = the template and `FellowshipMakerUI_ErrorType` = the entry's error key.
2. **Kind header** (level 1): *k* = the entry's flag word & 0x18000F (the six kind bits
   `Type_Skirmish`, `Type_Instance`, `Type_ScalingInstance`, `Type_NonScalingInstance`,
   `Type_BigBattles`, `Type_Seasonal`); *k* = 8 when no kind bit is set. One header per *k*,
   created at first use with the display text of *k* in mapper `FellowshipMakerConfiguration`
   (EMAPPER key 0x100000F5; 0x23000343 here) and `FellowshipMakerUI_Configuration` = *k*.
3. **Category header** (level 2, under its kind header): the template's
   `WorldJoin_EncounterCategory` *c*; text = the display text of *c* in mapper
   `WJEncounterCategory` (EMAPPER key 0x10000111; 0x23000350 here), or, for *c* = 0 or absent,
   the fixed text token 0x0BA22B42 of `UI_FellowshipMaker`. One header per (*k*, *c*).
4. The row goes under its category header. Order: headers and rows in entry order (no sort).
5. **Error rows.** A row with a non-zero error key, or one a further check marks, is drawn in
   its error look (a state change; the check routine was not traced).
6. **Featured.** The featured template (IN B1) is mirrored in the featured field: its text is the
   featured row's text and its state the row's state; the field is `ghosted` when there is no
   featured template, when the template's `DefaultPermissionBlobStruct` `Usage_MinLevel` is
   above the player's level, or when a further test fails; it shows the selected look while the
   selected row is the featured one. A further test of the featured template (not traced)
   also ghosts the field, and exactly when it holds the reset button
   `…_FeaturedInstance_ResetButton` is shown (its action was not traced). A click on the field
   selects the featured row (clearing the search).
7. **Search.** As §4.6 over the row and header texts.

   > **OPEN:** (implementer, 2026-10-11, `tessera-ui` `game::finder`) read as: a row's
   > search text is its own text followed by its category header's and kind header's texts
   > (as a travel row's search text holds its territory), so typing a header's text keeps
   > that group. Please confirm.

Selection of a row refills the size and tier menus and the level field (§6.4) and the join
button.

<a id="s6-4"></a>
### 6.4 Size, tier, level and join (BIN, TW17)

- **Size menu.** Its items carry `FellowshipMakerUI_GroupSize` (a `SkirmishGroupSize` key); with a
  group row selected, the template is the first of the group's list whose
  `Skirmish_Template_GroupSizeArray` holds the chosen size (IN B3). How the items are filled was
  not traced.
- **Tier menu.** Its items carry `FellowshipMakerUI_DifficultyTier`; on selection the menu is
  refilled from the template's `Skirmish_Template_DifficultyTierArray`, and the world events of
  `SkirmishControl_ZoneEvents` take part in which items are offered (rule not traced; Probable).
- **Level field.** An editable number of at most 3 characters, `ghosted` (not editable) in its
  `ghosted` state; how it is filled was not traced. A selection also sets the listings board's
  level filter (§6.7) to the template's level range, its top raised to the player's level when
  lower.
- **Join button.** No template resolved: `ghosted`, label token 0x07F08470 of
  `UI_FellowshipMaker`. Else the client builds a request (native class 2628 here) with the
  template and, where the template uses that dimension, the level, the tier and the size; it
  checks it with 0x0A2B0005, which returns an error key (0 = none): no error → `normal`, label
  0x028AC480; an error with a text → `ghosted` with that text; otherwise `ghosted` with label
  0x042040B0.
- **Join.** A click (or a double-click on the selected row or on the featured field) re-checks;
  without error it sends the request with 0x0A2B0004; with an error it shows it through
  class-library function 0x010C0001 (§7.1).

<a id="s6-5"></a>
### 6.5 Queue, confirmation and invitation (BIN, OBS; TW18, TW19)

The local player's `FellowshipMaker_PlayerState` (mapper `FellowshipMakerUI_PlayerState`:
1 `WaitingForConfirm`, 2 `WaitingInQueue`, 3 `WaitingForOthersToConfirm`, 4 `WaitingForPEJoin`,
5 `WaitingToConfirmEnterQueue`, 6 `WaitingForOthersToConfirmEnterQueue`, 7 `WaitingInWJQueue`)
drives the confirm page `FellowshipMakerConfirmPage`:

- states 2 and 7: `FellowshipMakerConfirmPage_WaitingField` shown with its timer
  `…_WaitingField_Timer_Timer` counting up from the time in queue (hidden when that time is 0);
- state 1: `FellowshipMakerConfirmPage_ConfirmTimer_Field` in `normal` with a count-down timer;
- other states hide the waiting field; `FellowshipMakerConfirmPage_ConfirmButton` and
  `FellowshipMakerConfirmPage_LeaveQueueButton` (152 × 38 each) are the page's two controls.

The window also refreshes when `FellowshipMaker_DisallowUse`, `Travel_IsTraveling`,
`LifeState`, `PrivateEncounter_Template_DataID`, `Combat_Violent` or `Trait_Manager_NPC`
changes on the local player.

**Invitation window** (`RootWJInvitationField`): title `WJInvitationTitle_Label` = the template's
`PrivateEncounterTemplate_Name` (with a second line from the move bar's tooltip when that is
set), `WJInvitation_BestowedQuestIcon`, the list `WJDisplayInvitationListBox` with
`WJFeaturedInstanceField` and `WJInvitationDisplayField`; in the latter `WJInvitationText`
(intro text built from the player and the template), `WJInvitation_SkirmishLevel_Field` (the
level, or the scaling range), `WJInvitation_SkirmishTier_Field` (display text of the tier in
mapper `SkirmishDifficultyTier`, EMAPPER key 0x100000BC), `WJInvitation_SkirmishSize_Field`
(display text of the size in mapper `SkirmishGroupSize`, key 0x100000BD; a template with
`Skirmish_Template_GroupSize_Override` shows that size), `Skirmish_Details_Label` or
`ClassicInstance_Details_Label` by `WorldJoin_EncounterType`, `WJInvitation_SkirmishPointInfoField`;
the reference field `WJAcceptDeclineFieldReference`: `WJInvitationAcceptButton`,
`WJInvitationDeclineButton`, `WJInvitationCancelButton`, `WJInvitationBuyNowButton` (hidden
when built), and the timer `WJInvitationTimer` with `WJInvitationTimer_Label` and
`WJInvitationTimer_Image`, shown only in one invitation state and counting down the remaining
seconds. When the invitation window is shown and what its buttons send was not followed.

<a id="s6-6"></a>
### 6.6 Inside an instance

No element of any layout is dedicated to leaving an instance or to an instance timer for
finder instances (search over all element labels); the confirmation dialogs of IN §12 (enter
or leave an instance, world-join queue, request and dequeue) are opened by game code not
traced (IN §12 OPEN). The finder only refreshes when `PrivateEncounter_Template_DataID`
changes.

**Tessera rules:**

1. `InstanceChanged { instance: Some(t) }` sets the client's "in instance" state; the finder's
   join button is `ghosted` with label 0x042040B0 while it is set.
2. Leaving: a client built-in command `leaveinstance` (decision 0020 dispatcher, CW §10) sends
   `LeaveInstance`; `InstanceChanged { instance: None }` clears the state. No button is added.
3. No timer is shown inside an instance in P5.

<a id="s6-7"></a>
### 6.7 Listings board (out of scope)

`FellowshipMakerLaunchNow_LFInstanceFinder` holds the "looking for instance" board: level and
class filters, a member list with sortable headers, find, my-instances, add, remove, join and
tell buttons, `LFInstance_Unavailable_Overlay` and an entry count. It lists other players'
listings (`LFG_*` properties); it belongs to the social panels. **Tessera rule:** P5 shows the
board with `LFInstance_Unavailable_Overlay` visible and every button `ghosted`.

<a id="s6-8"></a>
### 6.8 Tessera rules: the finder from the feed

1. **Entries.** One finder entry per `InstanceListShown` entry, in entry order; its `instance`
   names a template (IN §4) or a literal ref. The kind *k* is taken from the template's entries
   in the fellowship-maker index (IN §10.2; its flag word & 0x18000F, 8 when none or when the
   template has no index entry); the category from `WorldJoin_EncounterCategory`.
   > **OPEN:** (implementer, 2026-10-11, `tessera-gamedata` `instance::FinderPresenter`)
   > Two readings were needed. (a) The kind is taken from the matching index **rebuilt** with
   > the rules of IN §10.2 (`MatchingIndex::build` over the finder order), not from the
   > fellowship-maker control's stored object form, which no clean decoder reads yet; the
   > rebuilt index gives 587 entries here, 30 more than the stored 557, because the 23
   > templates of IN §10.2's OPEN get entries (and so a kind) where the stored index has none
   > (kind 8). (b) §6.3 rule 2's "display text of *k*" is read as the display text of the
   > mapper key whose bit is *k* (bit *b* is key *b* + 1, IN §10.2 item 2), so *k* = 8 shows
   > the text of key 4; *k* itself is not a key of that mapper for 0x80000 or 0x100000.
   > Please confirm both, or give the stored index's reading rule.
2. **Groups.** A template that belongs to an instance group is shown through its group row
   (§6.3 rule 1) when the feed lists any template of the group; selecting a size resolves the
   group's template by IN B3; `JoinInstance` still names the feed entry of the
   resolved template when the feed lists it, else the first listed entry of the group.
3. **Featured.** The featured field shows the template the proposed `featured` option names
   (§9), else none (`ghosted`). Tessera never evaluates `WE_Featured_Instance_Driver`.
4. **Menus.** The size menu lists the template's `Skirmish_Template_GroupSizeArray` (for a group
   row, the union over the group's templates in list order), the tier menu its
   `Skirmish_Template_DifficultyTierArray`, each item labelled with the display text of its key
   (mappers `SkirmishGroupSize`, `SkirmishDifficultyTier`); the first item is chosen. The level
   field shows the player's level clamped into `Skirmish_Template_MinLevelScale` …
   `MaxLevelScale` and is editable only when the template has a scaling range (min < max).
   Display data only; nothing is checked.
5. **Join.** `normal` when a row is selected and the client is not in an instance; a click sends
   `JoinInstance { window, entry }` with the proposed size, tier and level options (§9). No
   client-side eligibility test runs.
6. **Queue.** The proposed `InstanceQueueChanged` event (§9) sets the displayed player state; the
   confirm page and the alert field follow §6.5 and §7.2; `LeaveQueue` is sent by
   `FellowshipMakerConfirmPage_LeaveQueueButton`.

> **OPEN:** (implementer, 2026-10-11, `tessera-ui` `game::finder`) Readings the implementation
> needed: (a) while the displayed queue state is not 0 the panel shows
> `FellowshipMaker_ConfirmPage` (its tab when the panel lists one), else
> `FellowshipMaker_LaunchNowPage`; §6.5 says the state "drives the confirm page" but not which
> page is open. (b) A row's error look (§6.3 rule 5, the feed's `locked`) is the row's
> `ghosted` state when it defines one; the fallback row draws its text red. (c) When
> `FellowshipMaker_Instance_ListBox` holds no entries after the layout is built, the finder
> adds one entry from each of its two entry templates (featured field, tree field). (d) A
> template without `Skirmish_Template_MinLevelScale` … `MaxLevelScale` shows the player's level
> in the level field, not editable. (e) The size and tier menus open Tessera's code-made popup
> list (no context-menu widget is filled by game code). Please confirm or correct.
>
> (P5 exit verifier, 2026-10-12) (f) In this install's `fellowshipmaker` layout the panel's
> `FellowshipMaker_ConfirmPage` is an empty field without a base element, while
> `FellowshipMakerConfirmPage` (the waiting field, the timers and the two buttons of §6.5) is a
> top-level element of the same layout; `FellowshipMaker_LaunchNowPage`, by contrast, takes
> `FellowshipMakerLaunchNowPage` as its base. Tessera creates `FellowshipMakerConfirmPage`
> inside the empty page when the finder is built, and shows and hides both pages explicitly
> when the queue state changes. Please say how the original fills the confirm page.

---

<a id="s7"></a>
## 7. Chat and alert lines

<a id="s7-1"></a>
### 7.1 Lines the windows post (BIN, BC)

Neither window posts a chat line itself. Travel-request answers come from outside the window
(§4.9). Finder join errors are shown by class-library function 0x010C0001 with the error key:
for one key it posts a message through a chat routine with the short value 47 (by inference
chat type 47), otherwise it shows a message through a native package (317 here) not specified.

> **OPEN:** the presentation of 0x010C0001 (chat type and text for each error key, and whether
> a dialog is used). It is class-library behaviour (P7).

**Tessera rule:** a finder or travel failure answered by the feed with a `SystemMessage`
(SF §9.2) is shown as the feed sends it; the client adds no line.

<a id="s7-2"></a>
### 7.2 Alert fields (OBS, TW20)

The alert layout holds four fields for these windows, each based on `OverflowAlert`
(299 × 48) with an `UI_Alert_Image` and a tooltip of table 0x25027518:

| Field | Alert type (mapper 0x23000033) | Image | Tooltip token |
|---|---|---|---|
| `WorldJoinTravelField` | 38 `WorldJoinTravel` | 0x410FF4B4 | 0x0FD5D7A0 |
| `WorldJoinInvitationField` | 39 `WorldJoinInvitation` | 0x41102563 | 0x02D75780 |
| `WorldJoinQueueField` | 41 `WorldJoinQueue` | 0x4110322B | 0x01123170 |
| `FellowshipMakerWaitingInQueueAlertField` | 58 `FellowshipMakerWaitingInQueue` | 0x4111CF58 | per state, below |

`FellowshipMakerWaitingInQueueAlertField` carries `FellowshipMakerUI_AlertStateStringArray`:
one text per `FellowshipMakerUI_PlayerState` key 1–7 (tokens 0x004B9FED, 0x01ABDA45,
0x09E23B7D, 0x0431010E, 0x0A43BB45, 0x07F0DB25, 0x089E4D15 of `UI_FellowshipMaker`, in key
order).

**Tessera rule:** while the displayed queue state (§6.8 rule 6) is not 0, the
`FellowshipMakerWaitingInQueueAlertField` alert is shown with the state's text as tooltip; a
click opens the finder window. The other three alerts are not shown in P5.

---

<a id="s8"></a>
## 8. Census: what the windows can show

A real-data test puts every record of the kind in exactly one category. Counts are this
install's (TW21, TW22).

<a id="s8-1"></a>
### 8.1 Travel destinations (768) and station rows

| Category | Meaning | Here |
|---|---|---|
| `Listed` | the destination of at least one route in at least one station's `TravelDestinationRecordArray`: a row of some travel window | 740 (swift 389, slow 351) |
| `HomeOnly` | no station lists a route to it; it is some station's `TravelHomeLocation` | 22 (swift 3, slow 19) |
| `UnsoldRouteOnly` | only routes no station sells lead to it | 1 (0x7000EB70) |
| `OtherReference` | named by no station route and no station home, but named by `TravelVersioning` (its `TravelVersioning_DestinationPairArray`, as either destination of a pair, including the `TravelVersioning_SwiftDestination`), `Travel_PlayerHomeLocationArray` or `TravelHomeLocationsArray` | 5 (0x70005F75, 0x7000D29F, 0x7000D393, 0x700308AB, 0x700616BE) |
| `Unreferenced` | named by no record of the kinds above | 0 |
| `RecordUnreadable` | not a destination or fails to decode | 0 |

All 768 carry `TravelDisplayName` as a table reference and `TravelDestinationIsSwiftTravel`.

0x70005F75 is named only as the `TravelVersioning_SwiftDestination` (element 5 of a
`TravelVersioning_DestinationPairArray` pair) and by no station route or home, so it is
`OtherReference` by the definition above, not `Unreferenced`. This is the only destination in
that pair position with no other reference; every other count of this section is unchanged.

**Station rows** (421 stations, 1–40 rows each, 5,199 rows): every row has a route whose
destination is a destination record; no station lists one destination twice; 3,079 rows lead
to swift destinations and 2,120 to slow ones; 29 rows cost 0 (20 swift, 9 slow; shown as
free); 12 rows lead to the station's own home; 4,673 rows' routes carry `Usage_MinLevel`
(10–150; none below 2), 526 do not (Tessera shows level 0, the fixed text of §4.7); 186 rows'
routes carry quest requirements and 41 a faction requirement (eligibility is feed state).
Routes: 5,008, of which 3 are sold by no station (0x700094F9, 0x70009507, 0x7000EB72).

<a id="s8-2"></a>
### 8.2 Private-encounter templates (1,511)

| Category | Meaning | Here |
|---|---|---|
| `FinderRow` | in `WorldJoin_AvailablePETemplates_Array`: a template row of the finder | 150 (149 world-join encounters, 1 team template); 6 of them featured |
| `FinderGroup` | in an instance group: shown through one of the 8 group rows | 14 |
| `FinderUnlisted` | carries `WorldJoin_EncounterCategory` but is in no finder list | 3 (0x7001BB0F, 0x70024183, 0x700355EA) |
| `InstanceTravel` | named by some `PrivateEncounter_TravelArray` (56 holders, 455 references): the instance-travel window | 440 |
| `Mission` | `PrivateEncounterTemplate_EncounterType` 1: no window list | 384 |
| `NoWindow` | any other template: entered through doors, quests or scripts, no window list | 520 (516 ordinary, 2 team, 2 other skirmish) |
| `RecordUnreadable` | fails to decode | 0 |

No template is in two lists (finder and instance travel are disjoint). Names: 1,510 table
references and 1 literal (the template labelled `PublicAreaDefaultEncounter`, IN §4.12).
Finder templates (164) use 36 non-zero categories and category 0 (9 templates) or none
(1, the team template; shown under the fixed category text); 14 carry
`Skirmish_Template_LevelScalingLevel` 100 (IN B2), 14 a ticket, 56 a store item.

> **OPEN:** (implementer, 2026-10-11, `tessera-gamedata` `instance::template_census`) A scan
> of every object property set finds 57 records that store `PrivateEncounter_TravelArray`; one
> of them stores an empty array. The implementation counts as holders only the 56 with at
> least one template reference (455 references), which matches the table above. Whether a
> record with an empty array is a holder (and whether its window shows an empty instance-travel
> list) is not stated.

**Windows themselves:** `LayoutMissing` and `ElementMissing` (any layout or element named in §3,
§4.2, §5.1, §6.2, §6.5 or §7.2 absent) are failures; this install has none.

---

<a id="s9"></a>
## 9. Scenario-feed forms (proposal for the revision labelled "P5 travel", numbered revision 8 of `scenario-feed.md`)

The existing forms (SF §6.5, §9.2, §9.3) render both windows: `travel-list` / `TravelShown`,
`travel` / `TravelStarted`, `instances` / `InstanceListShown`, `instance` / `InstanceChanged`,
`ChooseTravel`, `JoinInstance`, `LeaveInstance`, `Dismount`. Exit criterion 3 needs nothing
more. The additions below carry values the client must not derive (decision 0024 §2 principle
4); all are optional and additive, so scenarios and logs without them stay valid. The
`scenario-feed.md` writer reconciles names and the revision number at merge.

| Form | Addition | Default | Used by |
|---|---|---|---|
| `travel-list` step, `TravelShown` entry | `min-level=N` → `min_level: Option<u16>`; flags `undiscovered`, `unmet`, `unusable` → `flags: u8` (bit 0 undiscovered, bit 1 requirements not met, bit 2 not usable); `area=REF` → `area: Option<Ref>` (an `area` selector element) | none, 0, none | §4.10 rule 3 |
| `TravelEnded` event (new), step `travel-end` | ends the travel presentation | — | §5.2 |
| `instances` step, `InstanceListShown` | `from=ENTITY` → `source: Option<EntityId>`; `featured=REF` → `featured: Option<Ref>` | none | §4.11, §6.8 rule 3 |
| `JoinInstance` command | `size: u8`, `tier: u8`, `level: u16` (0 = not chosen) | 0 | §6.8 rule 5 |
| `InstanceQueueChanged` event (new), step `instance-queue STATE [REF] [for=DURATION]` | `state: u8` (0 none, 1–7 the `FellowshipMakerUI_PlayerState` keys), `instance: Option<Ref>`, `seconds: Option<u32>` (time in queue, or time left to confirm) | — | §6.5, §7.2 |
| `LeaveQueue` command (new) | — | — | §6.8 rule 6 |

Protocol: none of these needs a new protocol minor for P5; the test server never sends them
(decision 0024, client half only).

---

<a id="s10"></a>
## 10. Invariants

| # | Invariant |
|---|---|
| TI1 | A travel window shows one row per feed entry, in entry order; rows with equal territory text share one header; headers appear in the order of their first row. |
| TI2 | Exactly one of `Travel_Currency_DestinationCost` and `Travel_Currency_DestinationCost_Free` is visible while a row is selected; neither without selection. |
| TI3 | `Travel_RouteStatus` is in `Travel_NoRouteSelected` exactly when no row is selected. |
| TI4 | The coin go button is `normal` only with a selected row that is discovered and usable (and, in the original, when the player is not shapeshifted). |
| TI5 | The client never computes a cost, a level requirement, a usability or an eligibility, never moves the player for a travel and never closes a feed window without a feed answer (§4.10 rule 6). |
| TI6 | Each finder template row sits under exactly one category header, which sits under exactly one kind header; a group row stands for every listed template of its group. |
| TI7 | The census of §8 puts every destination and every template in exactly one category; the counts sum to 768 and 1,511 on this install. |
| TI8 | No text is produced from anything but install string tables, mapper display texts, feed literals and names (no game text in code). |

---

<a id="s11"></a>
## 11. Worked synthetic examples

All DIDs are invented (prefix 0x70F0…).

**11.1 Grouping.** Feed entries (in order): D1 (area A1 in territory T1), D2 (area A2 in T2),
D3 (A3 in T1), D4 (no area). Tree: D4 at the top level; header T1 with D1, D3; header T2 with
D2. Typing a filter that only D3's area text contains hides D1, D2, D4 and header T2.

**11.2 Free and level.** Entry D1 `coin=0 min-level=1`: the free text is shown, the money display
hidden; the level text uses the fixed text (level below 2). Entry D2 `coin=2500 min-level=40`
with the player at level 35: money shows 25 silver (NW §9.3 split), the failed-requirements
icon is on D2's row, the go button follows the flags only (the level mark is display).

**11.3 Travel presentation.** `travel D_swift 4s` → loading screen, meter 0 at 0 s, 0.5 at 2 s,
hidden at 4 s; `teleport` at 3 s ends it at 3 s. `travel D_slow 10s` → end-travel box for 10 s;
clicking its button sends `Dismount`; the box stays until `travel-end` or 10 s.

**11.4 Finder tree.** Templates P1 (index kind bit `Type_Instance`, category 13), P2
(`Type_Instance`, category 0), P3 (`Type_Skirmish`, category 4), P4 and P5 in group 2 with size
arrays (12) and (24). Tree: header *Instance* → header *cat 13* → P1, header *fixed text* → P2;
header *Skirmish* → header *cat 4* → P3; P4/P5's kind and category headers → one group row.
Choosing size 24 on the group row resolves P5.

---

<a id="s12"></a>
## 12. Open questions (summary)

1. §4.3: the tests 0x01CC0026, 0x01CC0006, 0x01CC0007, 0x01CC0008, 0x01CC001B, 0x01CC002E, the
   cost multiplier, and how territory and area texts are chosen (class-library code; P7).
2. §4.6: case handling of the search comparison (Tessera rule given).
3. §4.9: the two unidentified close notifications.
4. §5.1: what 0x01CC0009 sends; when the loading screen appears for a travel teleport.
5. §6.3: test 0x0A2B002B; the error-look check; the featured reset button.
6. §6.4: how the size and tier menus and the level field are filled; the tier gating by zone
   events.
7. §6.5: when the invitation window opens and what its buttons send (traffic, decision 0004).
8. §6.6: how the original lets a player leave a finder instance (IN §12 dialogs).
9. §7.1: the presentation of finder errors.
10. §8.2: whether a record with an empty `PrivateEncounter_TravelArray` is a holder
    (implementer OPEN, 2026-10-11).
11. §6.8: the open page while queued, the error look, the finder list's entries, the level
    field without a scaling range, the menus' popup (implementer OPEN, 2026-10-11).

---

<a id="s13"></a>
## 13. Proposed owner observations

| Row | Observation |
|---|---|
| O140 | Open a stable master's travel list: headers are territory names; rows within a header are not alphabetical (TW6). Screenshot with one header expanded. |
| O141 | Select a free row and a paid row; note what the cost area shows; select an undiscovered row and note the status text and the go button's text. |
| O142 | Type part of an area name into the travel search: do rows of other territories with that area text stay? Does case matter? |
| O143 | Ride a slow route: the end-travel box appears; does a chat line appear at start or arrival? Time the ride against the route's segments. |
| O144 | Take a swift route: is the loading screen shown, and for how long? |
| O145 | Press Ctrl + J: the fellowship-maker window opens at the instance-finder page; screenshot the tree's two header levels and the featured field. |
| O146 | Queue for an instance: screenshot the waiting page, the timer and the alert icon and its tooltip. |
| O147 | Inside an instance from the finder: how does the client offer leaving (dialog, command, menu)? |

---

<a id="s14"></a>
## 14. Notes for other specs

- `map-data.md` §8.8 and §16: the travel window's full set of class-library functions, the info
  object and the cost rounding are in §4.3 here (notes added there, status of this spec).
- `instances.md` §7.3, §15.1, §18: the finder's tree, headers, mappers and request dimensions
  are in §6.3–§6.4 here (notes added there).
- `scenario-feed.md`: the additive forms of §9 (revision 8, labelled "P5 travel").
- `interaction.md` §3.4.5: use types 9 and 21 open the travel and instance-travel windows
  (§4.1, §4.11).
- `chat-window.md` §10: the built-in `leaveinstance` of §6.6 joins Tessera's registry.
- `lua-plugin-api.md` §15.5: no plugin surface is needed for P5's windows.
- `game-panels.md`: the end-travel box (§5.1) is a gameplay activatable of the HUD layer.

---

<a id="changelog"></a>
## Change log

| Date | Change | By |
|---|---|---|
| 2026-10-13 | Independent re-review against CLEAN_ROOM.md §5.3 by a separate session (not the author, not the earlier reviewer): **passed, Accepted**. The blocking §8.1 census item is resolved (0x70005F75 is `OtherReference` as a `TravelVersioning_SwiftDestination`; counts 5 / 0) and confirmed by an own read-only M1 probe of the install (Listed 740, HomeOnly 22, UnsoldRouteOnly 1, OtherReference 5 incl. 0x70005F75, Unreferenced 0, total 768; 421 stations / 5,199 rows; routes 5,008 with 3 unsold — every §8.1 number reproduces). Two non-blocking items stand, both behaviour-neutral: §9's `size`/`tier` are still `u8` while the now-Accepted `scenario-feed.md` "P5 travel" revision types them `u32` (the code follows that revision; aligning §9 is advisory), and the broader mark scheme. Status → Accepted. | Spec reviewer (dirty room) |
| 2026-10-13 | Revision answering the review's blocking item: §8.1 reconciled so that 0x70005F75 is `OtherReference` (named by `TravelVersioning`), not `Unreferenced`; the counts are now `OtherReference` 5 / `Unreferenced` 0, matching both the table's own definition and the merged census output; the `OtherReference` definition is widened to state that a pair's `TravelVersioning_SwiftDestination` counts; the inline §8.1 OPEN is closed and replaced by a resolution note. No fact of any other section changed; TI7's sums (768, 1,511) are unaffected. Behaviour-neutral: the implemented census already follows the definitions. Status → Revised. | Spec writer (dirty room) |
| 2026-10-13 | Independent review against CLEAN_ROOM.md §5.3: changes requested (one blocking — the §8.1 `Unreferenced`/`OtherReference` census contradiction for 0x70005F75, which also disagrees with the merged code's output; behaviour-neutral) and two non-blocking notes (§9 `size`/`tier` type alignment with `scenario-feed.md`; the mark scheme). Status stays Review; details in the Review record. Provenance header spot-checked on the install (M1): build, binary SHA-256 prefix and archive dates match. | Spec reviewer (dirty room) |
| 2026-10-11 | First version (Review): window catalogue; travel window (opening, tree, row source and info object, marks, grouping, search, selection display, go controls, closing, Tessera feed rules); instance-travel window; started travel and arrival (end-travel box, loading screen, Tessera presentation); instance finder (fellowship-maker launch-now page, entry tree, menus, join, queue and confirmation, invitation window, inside an instance, listings board out of scope); chat and alert lines; census of 768 destinations, 5,199 station rows and 1,511 templates; feed proposal for the "P5 travel" revision; invariants, examples, OPENs, owner rows O140–O147. Provenance OBS, BC, BIN (decision 0011, questions q1–q7), rows TW1–TW23. | Spec writer (dirty room) |
