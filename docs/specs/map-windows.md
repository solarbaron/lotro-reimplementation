<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Map windows: the world-map panel, map navigation, overlays on the world map and the minimap, and place lookup

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review by spec-review:spec-ui-r4:3 (independent session), which wrote no part of the round-4 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (round-4 revision of 2026-10-23: the implementer OPENs of §4.5 answered with row MW36 and BIN question (q10); §6.9 says which widget `FilterMenu` is; implemented behaviour changes in §4.5 and §6.9; awaiting independent review; see the change log). Before: **Accepted** (round-3 revision of 2026-10-23: independent review 2026-10-23 by spec-review:spec-ui-r3:1 (independent session): passed; see the review record. The revision: four implementer OPENs of §4.6 and §6.3 answered, MW15 completed; see the change log). Before: **Accepted** (2026-10-23, independent review 2026-10-23 by spec-review:spec-gameplay:2 (independent session), a session that wrote no part of the 2026-10-23 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (2026-10-23, revision 1: the implementer OPENs of §4.6, §6.3, §6.8, §6.9 (b), §7.2, §7.3 and §8 answered with rows MW30–MW35; new §8.1 (the fallback map window, which makes the tier-1 world-map golden normative); implemented behaviour changes in §4.6 (the player's dungeon map indoors) and may change in §6.3 (the install's tooltip and coordinate entries may now be rendered); awaiting independent review). Before: Accepted (independent review 2026-10-13: CLEAN_ROOM.md §5.3 passed, every item) |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Spec reviewer (dirty room, AI-assisted, Clean-Room-Role: review), a session that wrote no part of this spec, of the P5 notes in `game-panels.md` and `map-data.md`, or of the P5 code, 2026-10-13 |
| **Review record** | **Review 2026-10-23 of the round-4 revision** (spec-review:spec-ui-r4:3 (independent session), which wrote no part of it): **passed**; status Revised → Accepted. Round-4 revision with the fixes after review 2: §4.5 opening and placement (MW36, BIN (q10) within decision 0011, not re-traced; decision-0012 constants 3 units and 0.2 s listed), MW8, §3.3, §6.9 Widget bullet and OPEN (the Selecting rule is outside the callout again), scenario-feed carry-over note. Non-blocking: §4.5 says `boolean_button` comes "from its base `AvailableMapMenu`"; it is set on that element's own base `DropDownButton`. CLEAN_ROOM.md §5.3 met on the changed text: functional facts and labelled Tessera rules only; no code or pseudo-code; no copied prose; no game text (strings cited by token only), keys, hostnames or traffic; no disassembly in prose; new facts carry provenance rows and marks; unknowns are OPEN callouts with Tessera rules; nothing outside decision 0034 §2; contamination screen clean. Relative links and anchors of the ten round-4 specs resolve. Spot checks (own read-only probes, 2026-10-23): `EnablePortraits=True` in `[Render]` of both preference files; `FilterMenu` based on `MapNoteFilterMenu` (based on `Menu_02` of 0x22000080, `auto_close_on_selection` false, `text_item` `ContextMenuBool` of 0x2200007B, `selection_display` 0); `MapMenu` based on `AvailableMapMenu`, based on `DropDownButton` of 0x22000004 with `boolean_button` true; `Menu_01` of 0x22000080 a Menu without `auto_close_on_selection`; the finder's `…_TierMenu` (popup `Menu_01_PopupField`, items `Menu_01_PopupItem`, own selection display, no `auto_close_on_selection`) and `…_SizeMenu` (based on `Menu_01`). Before: **Review 2026-10-23 of the four implementer OPENs of §4.6 and §6.3, MW15 completed** (spec-review:spec-ui-r3:1 (independent session), Clean-Room-Role: review; wrote no part of the revision or of the code): **passed**; status Revised → Accepted. Checked: checked against CLEAN_ROOM.md §5.3, every item, on the uncommitted worktree diff against main; contamination screen clean (no code, pseudo-code or listing-shaped text; no disassembly prose; no game text, keys, hostnames or traffic; hex values are data IDs read from the install; nothing outside decision 0034 §2); answers are functional facts with provenance or labelled Tessera rules, unknowns are OPEN; relative links and anchors of the six package specs resolve (own check). Spot checks: the §6.3 coordinate answer agrees with `coordinates.md` §8.3 (token 0x08D46694, arguments 0x539D and 0x48E2); the saved q5 listing re-read for MW15 is kept in the dirty-room analysis work area outside the repository, and no new decompiler session was used (BIN row). Non-blocking note: the nested OPEN for `scenario-feed.md` in §4.6 exists only in this spec; `scenario-feed.md` has no matching callout yet, so the change-log words "filed there" mean filed here for that spec's owner to carry over. Earlier: **Review 2026-10-23** of the 2026-10-23 revision (spec-review:spec-gameplay:2 (independent session)): **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; MW30 hashes reproduced (`records.md` §1.5: `ns` 0x753, `ew` 0x6C7); MW32 labels reproduced by an own M1 probe of DID map 0x28000002 (`GameplayOptionDefaults` key 0x68, `Preferences_Gameplay_Option_Defaults` key 0x10000012); §8.1 rects and z-levels agree with the Accepted §3.2–§3.3 tables. Earlier: First version for decision 0024 P5 (client half only): exit criterion 1 ("the map panel renders every map region with scenario markers") and the "place lookup" theme of decision 0024 §5 P5. **Independent review 2026-10-13 (spec reviewer, dirty room, AI-assisted, not the author): passed; Accepted.** Checked against `docs/CLEAN_ROOM.md` §5.3: every statement is a functional fact (element IDs, rects, masks, counts, behaviours, math); no code, pseudo-code or decompiler-listing structure (the §3.4 inverse transform is a math formula; §4.2/§4.7 steps are plain-word behaviour); no copied prose or comments; all quoted identifiers are the install's own data labels, declared as interoperability names in the naming note; no game text (texts named by string table and token only), assets, keys, hostnames, endpoints or official-traffic dumps; every fact has a provenance row (MW1–MW29) marked OBS/BIN/Confirmed/Probable per the scheme at the top of §1, consistent with decisions 0011 and 0012; byte/rect tables give units (map pixels at design size, world units, z-levels); invariants MWI1–MWI10 are testable; unknowns are `> **OPEN:**` with explicit Tessera rules rather than guesses; decision-0034 scope item holds (offline map UI only; the BIN row records that the network protocol, login, keys, launcher and anti-tamper code were not read, decision 0004). Spot checks (reviewer's own read-only probes of the owner's install and client, 2026-10-13): **M1** — every cited record resolves in place (`map` layout 0x22000041, `ContextMenu` 0x2200007B and `radar` 0x2200003A in `client_local_English.dat`; `GameplayOptionDefaults` 0x78000006, `RegionControl` 0x7001819C and both icon tables 0x78000001/0x78000002 in `client_gamelogic.dat`; the two string tables in `client_local_English.dat`), and the `MapNoteType` mapper 0x230000D7 payload begins with its own DID then `0x230001A4`, exactly the base the spec states. **BIN** (decision 0011; read-only byte scan of the private client copy `lotroclient64.exe`, file time 2026-10-04): the version strings `4906` and change list `711794` match the provenance (PE 4906.0071.1794.4035); the pure-BIN immediates appear as the spec requires — the read AND-mask 0x9B7C7FFFFEFF0002 (MW12) twice, the 28-bit common mask 0x1B7C76E75CF00000 (MW3) once, the tooltip DIDs 0x0B0C5A53 (MW15), 0x053B8BB3 (MW19), 0x0B8B58E0 and 0x0DDC06F5 (MW22/MW15) present — while values the spec marks OBS (the near/radar filter defaults from `GameplayOptionDefaults`, the radar display mask from the radar layout) are correctly **not** code immediates, and the code references layout **keys** (0x10000041, 0x1000003A) rather than record DIDs, matching the naming note's key/record distinction. Census (§5.1–§5.4) and decision 0036 §1 are mutually consistent and the counts add up internally (node regions 74+47+40+38+25+2+1 = 227; filter-menu item buckets 127+88+4+1 nodes + 7 hidden = 227; R-node 227 + R-dungeon 1,298 = 1,525; 1,303 dungeons − 5 shared sets = 1,298; C5 = 2 so 1,523 marked; 540 area + 1 non-area = 541 node DIDs). Overlay/marker rules (§6) and the scenario-feed `map-region`/`markers` dependency (`scenario-feed.md` §8.3, §18.P5 T-D6/T-D7) agree. Non-blocking notes (not conditions of acceptance): (1) §6.2's back-to-front table mixes z across different parents (e.g. the quest-guide parent at z −5 versus the player marker at z −500), so the raw z column is comparable only within a parent; the quest-guide pieces are not drawn in P5 (§6.5), so this does not affect any P5 behaviour. (2) The §8 OPEN (no fallback map window defined) blocks the tier-1 world-map golden of decision 0036 §2 until the spec writer adds the rule; the implementer's provisional note covers the interim and is not a clean-room defect. These are logged as OPENs/notes in the spec, not fixes required for Accepted. |
| **Implements into** | `crates/ui` (`tessera-ui` `game::worldmap`, `game::radar`, `game::map`: the map window's parts, navigation, the dungeon-map presentation, note, marker and filter-menu elements), `crates/gamedata` (the map-region census of §5 and the place-lookup index of §7.3), `crates/settings` (`tessera-settings`: the three filter values and the presentation switch of §6.9 and §3.1), `apps/client` (`tessera-client`: map panel wiring, feed markers, the `findplace` command, the census run), `apps/explorer` (optional: the census sheet of §5.4) |
| **Companion specs** | [`game-panels.md`](game-panels.md) (**GP**: §8 minimap, §9 world map), [`map-data.md`](map-data.md) (**MD**: §3 places, §4 map notes, §4.8 selection, filters and icon tables), [`quest-journal.md`](quest-journal.md) (**QJ**: §8.3 quest markers), [`scenario-feed.md`](scenario-feed.md) (**SF**: §6.5 `marker`, `MarkerAdded`, `MarkerRemoved`), [`coordinates.md`](coordinates.md) (**CO**: §4 positions, §8 map coordinates, §9 `/loc`, §10 map pixel frame), [`wstate.md`](wstate.md) (**WS**: §11 areas, territories, regions, dungeons, `Region_Map_List`), [`ui-layout.md`](ui-layout.md) (**UL**: element trees, §9.3 drawing order, §5.9 object modes), [`ui-behaviour.md`](ui-behaviour.md) (**UB**: input routing, pointer commands), [`keymap.md`](keymap.md) (**KM**: input actions), [`lua-plugin-api.md`](lua-plugin-api.md) (**LPA**: §13.18–§13.19 stretch modes), [`chat-window.md`](chat-window.md) (**CW**: §8 command names, §10 Tessera's registry), [`walker.md`](walker.md) §6 (`--place` index), [`string-table.md`](string-table.md) (**ST**) |

> **Naming note (interoperability).** Layout labels (`map`, `ContextMenu`), element labels
> (`MapWindow`, `MapBackground`, `FilterMenu`, …), widget-type labels, enum labels (`Waypoint`,
> `Fellow`, …), input-action labels (`ToggleMapPanel`, `ToggleMapPanelFullscreen`, `SelectLeft`,
> …), option and property names (`GOP_Map_MapNoteFilter`, `UI_Map_…`, …) and DID-map labels
> (`UI_Maps`, `WeenieClientMsgs`, `RegionControl`) are the install's own identifiers, read from
> the user's data (enum mappers, the property dictionary 0x34000000, DID maps). They are quoted
> only so that an implementation can find the data by them. Numeric IDs and DIDs printed next to
> them are this install's values (2026-10-11) and must be looked up by label at run time. No game
> text appears here: texts are named by string table and token.

---

<a id="s0"></a>
## 0. Purpose

`game-panels.md` §9 gives the world-map tree, the node images and their georeference, and
`map-data.md` §4.8 gives how the radar and the map window select notes from the map-note table.
What the P5 milestone still needs is the map window as the original presents it: how a player
moves through the maps, which map appears for a place, how dungeon maps are drawn, what is drawn
over a map and in which order, the note-type filter controls and their defaults, and what the
original offers to find a place. This spec adds those facts, defines a countable "every map
region" for the census of P5 exit criterion 1, gives Tessera's rules for the presentation of
scenario-feed markers and for place lookup, and proposes corrections where the earlier Tessera
rules differ from what the client does.

| Part | Section |
|---|---|
| The parts of the map window, their element IDs, roles and fill rules | §3 |
| Navigation: building, showing, child maps, going up, the map menu, the player's map, dungeon maps, display scale | §4 |
| "Every map region": definition, counts, categories of what cannot be shown, census procedure | §5 |
| Overlays on the world map and the minimap, their order and look; note-type filters and defaults | §6 |
| Place lookup: what the original offers; Tessera's lookup | §7 |
| Tessera rules where the data and the client stop | §8 |
| Invariants, examples, corrections, open questions, proposed owner observations | §9–§13 |

Out of scope: travel and instance-finder windows (other P5 specs), mounted gaits, the quest
guide's objective shading (QJ §14 Q7; only its place in the drawing order is given here),
fog-of-war drawing (GP §8.5.3 OPEN).

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | The user's install (Steam, archives unchanged since 2026-10-04, file times re-checked 2026-10-11), read in place and read-only with throwaway probes in the dirty-room scratch area: the UI layout `map` (UILAYOUT key 0x10000041, record 0x22000041) with every top-level element resolved by UL §7 (1,803 element instances), the `ContextMenu` layout (UILAYOUT key 4, 0x2200007B), the `radar` layout (0x2200003A); the whole node tree of `UI_Map_AreaData` (227 nodes) with every field; every stand-alone property set (prefix 0x78, 1,618) and every object property set (prefix 0x79, 383,332) searched for `Dungeon_MapData`, `Dungeon_MiniMapData`, `Area_MapData` and `Dungeon_Name`; the headers of all 227 node images and all 1,300 dungeon-map images; `GameplayOptionDefaults` (0x78000006); `RegionControl` (WEENIECONTENT key 0x100000E9, 0x7001819C); the two note icon tables (0x78000001, 0x78000002); the enum mappers `MapNoteType` (EMAPPER key 0x72, 0x230000D7 with base 0x230001A4: names and display-text entries), `InputActions` (0x230000AF), `UIElementMessage` (0x230001AC), `UIElementID` (0x230000D1); the string tables `UI_Maps` (STRINGTABLE key 0x1000003E, 0x250001E1) and `WeenieClientMsgs` (key 0x10000039, 0x250001C0) for **tokens and argument IDs only**; the `Commands` table (0x250001AB) for command names (read to classify, not reproduced). |
| BIN | The client executable `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, change list 711794, compiled 2026-09-27; file time 2026-10-04, unchanged), read in the dirty room on 2026-10-11 (the saved q5 listing was re-read on 2026-10-23 for the round-3 answer of §6.3, with no new decompiler session) under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md) (method M4) in a private copy of the auto-analysed project. Only the map-window class (its post-creation, node-tree, show-node, message, menu, filter, note, marker, coordinate and presentation routines), the radar's note routines and filter commands, and their direct callees were read, by address range. **Questions:** (q1) how the map window builds its node tree and shows a node; (q2) which inputs move between maps and whether any zoom exists besides the tree; (q3) which map the window chooses for the player and when; (q4) how a dungeon map is drawn in the window; (q5) how notes are created, filtered, labelled and ordered on the world map, how the filter menu is built and where its state is kept; (q6) how the player marker is placed, and how a world point in another region maps to a map pixel; (q7) what the coordinate display shows; (q8) how the radar places note blips, uses `UI_Radar_RadarRadius` and changes its filter; (q9) how the windowed and full-screen presentations differ. **Why M1–M3 were not enough:** the layouts name the elements and the data hold the masks and options, but which code reads them, the defaults' use, the navigation inputs, the stretch of dungeon maps and the region rule leave no trace in the data; no published source describes them; the original cannot be observed in this session and the owner's P10 screenshots (checklist P10-A) are not yet taken. The network protocol, login, keys, launcher and anti-tamper code were not read (decision 0004). **Round-4 revision (2026-10-23):** method M4 under decision 0011, the same build (SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee, hashed 2026-10-23), Ghidra 12.1.3 headless on a private read-only copy of the shared project (deleted after use), plus the saved map-window listing of 2026-10-11; read: BIN (q10) only. **Question:** (q10) where the `MapMenu` context menu opens, which element its submenus use, which region entries get a submenu, and what "a short time" of the reopen rule is. **Why M1–M3 were not enough:** the layouts give the menu element but not the placement, the timing or the submenu rules, and the original cannot be observed in this session. |
| CMP | The companion specs in the header (Accepted unless noted); their facts are cited with their own provenance. |
| DEC | Accepted Tessera decisions: [0020](../decisions/0020-chat-command-routing.md) (one command dispatcher), [0024](../decisions/0024-parity-track.md) (parity track, client half only). Statements taken from them are marked **Tessera rule**. |

Marks: **Confirmed** = checked over every record or element of the kind (OBS), or a BIN fact the
data agree with in every case; **BIN** = rests on the disassembly only; **Probable** = supported
but not proven in every case.

| # | Fact | Sources | Mark |
|---|---|---|---|
| MW1 | After its tree is created, the map window finds by element ID `MapField`, `MapMenuScaleParent`, `FilterMenu`, `MapMenu`, `MapQuestObjectiveIndicatorsParent` (hidden at once), `StablemasterButton`, `CoordinateDisplay`, `QuestGuideCheckbox` and `QuestGuideOptionField`, reads `UI_Map_AreaData` and builds its node tree, registers areas (MD43), and reads the `UI_Map_QuestObjectiveIndicator_*` corner and tile images (§3.2) | BIN (q1); OBS (element IDs and types) | BIN |
| MW2 | Two presentations: full screen (`MapWindow` shown, `MapPanel` hidden) and windowed (`MapPanel` shown, `MapField` moved under `MapPanelPadding`); the player option `GOP_MapInFullscreenMode` chooses on show; the collapse and expand buttons and the input action `ToggleMapPanelFullscreen` switch and store it (§3.1) | BIN (q9); OBS (option default true in 0x78000006; button click actions resolve to `ToggleMapPanel` and `ToggleMapPanelFullscreen` in `InputActions`) | BIN |
| MW3 | Per node the window reads image, scale, region, block and pixel offsets, `UI_Map_FogOfWar`, `UI_Map_HideFilterButtons`, pixel width and height (from the image's size when 0), `UI_Map_MapNoteDisplayType` (ORed with the 28-bit common mask 0x1B7C76E75CF00000 when `UI_Map_IncludeCommonMapNoteTypes` is true), `UI_Map_MapNoteFilterProperty`, `UI_Map_ActiveElement` (required), `UI_Map_MenuName` (required), `UI_Map_AreaDIDs_Array`, the four quest-guide bounds (default: the pixel frame), `UI_Map_QuestGuideDisabled`, `UI_Map_PreventZoomOut`, and keeps its parent's active element (§4.1) | BIN (q1); OBS (227 nodes) | BIN |
| MW4 | Showing a node instantiates its active element as a child of `MapBackground`, sets the node's `UI_Map_MapImage` on it with draw mode 2, and rebuilds notes, menus and marker; showing the node already shown does nothing (except the dungeon map); an element that is no node's active element is shown without node data (§4.2) | BIN (q1) | BIN |
| MW5 | A left click or double left click (`SelectLeft`, `SelectDblLeft`) on any element inside the window that carries `UI_Map_Child_Map` (an element ID) shows that element's node; choosing an item of the map menu does the same (§4.3, §4.5) | BIN (q2); OBS (773 such elements in the 227 active elements: 770 name a node, 3 name a top-level element that is no node) | BIN |
| MW6 | A right click or double right click (`SelectRight`, `SelectDblRight`) on the window shows the parent node; at a node without parent (the root, a dungeon map) the window shows the player's outdoor map when the dungeon map is shown and the player is in a dungeon, else it makes a call not traced (§4.4) | BIN (q2) | BIN |
| MW7 | The window's message handler has no mouse-wheel, drag-to-pan or keyboard-navigation case; the only key it handles is `ToggleMapPanelFullscreen`. Maps are never zoomed or scrolled inside a node (§4.8) | BIN (q2: every case of the handler) | BIN |
| MW8 | The map menu (`MapMenu`, a toggle button that opens a context menu) lists the 6 entries of `RegionControl`'s `Region_Map_List` (WS §11.3) as submenus, each holding the nodes whose `UI_Map_RegionID` equals the entry's region, labelled by `UI_Map_MenuName`, sorted by label; the menus (root and submenus) use `ContextMenuElement_MapMenu` (§4.5, MW36) | BIN (q1); OBS (6 entries; node regions 2: 74, 1: 47, 3: 40, 4: 38, 5: 25, 14: 2, none: 1) | Probable (submenu order; collation) |
| MW9 | The player's map: from the player's (dungeon, map-data) pair, a dungeon shows `DungeonMap`; otherwise the area of the player's position is looked up in the area registration (MD43) and that node is shown, `WorldMap` when the area is unlisted; with an invalid position nothing changes (§4.6) | BIN (q3) | BIN |
| MW10 | A dungeon map is shown in the top-level element `DungeonMap`: its image is set on `DungeonMap_ImageField` (800 × 600 at (112, 84)) after giving the field object mode 3 (CanvasResize) at the image's pixel size, then object mode 1 (Stretch) at the field's own size, so the image and everything attached to it is scaled to 800 × 600; `DungeonMap_OverlayField` frames it; notes and the marker are children of the image field (§4.7) | BIN (q4); OBS (rects; 1,298 images of 11 sizes) | BIN |
| MW11 | Notes of the map-note table enter the window only when their `type` shares a bit with the shown node's display mask (MW3), or always on a dungeon map, and (outdoors) when the node of the note's area is the shown node or a descendant (MD §4.8.4); each is an instance of `MapNoteTemplate` attached to the shown map element (§6.3) | BIN (q5) | BIN |
| MW12 | A created note is visible exactly when its `type` shares a bit with the **filter value**: the 64-bit player option named by the shown node's `UI_Map_MapNoteFilterProperty` (`GOP_Map_MapNoteFilter` when the node names none), ANDed with 0x9B7C7FFFFEFF0002 when read; for `type` = 1 (link) the test is filter bit 1 (§6.3, §6.9) | BIN (q5); OBS (170 nodes name `GOP_Map_MapNoteFilter`, 57 `GOP_Map_Distant_MapNoteFilter`) | BIN |
| MW13 | `FilterMenu` is hidden on `WorldMap` and on nodes with `UI_Map_HideFilterButtons`; otherwise it holds two items (all, none) and one check item per bit of the node's display mask (all 64 bits on a dungeon map) except bits 0, 1, 38, 45, 46 and 48, labelled by the `MapNoteType` display text of that bit's key, sorted by label, each an instance of `ContextMenuBool` with the type's menu image and a check state = filter bit; selecting toggles the bit (all / none set or clear every listed bit) and stores the filter option on the player (§6.9) | BIN (q5); OBS (all 49 labelled keys have a display text; item counts per node) | BIN; icon field Probable |
| MW14 | Defaults in `GameplayOptionDefaults` (0x78000006): `GOP_Map_MapNoteFilter` 0x1B7C77E7FEFF0000, `GOP_Map_Distant_MapNoteFilter` 0x000444048AE90000, `GOP_Radar_MapNoteFilter` 0x9FFE7FFFFFFF0003, `GOP_DisplayCoordinatesOnMap` true, `GOP_QuestGuide_Map` true, `GOP_QuestGuide_Radar` true, `GOP_MapInFullscreenMode` true, `GOP_ShowFellowshipPetsOnRadar` true; the three filters are bitfields of the `MapNoteType` mapper | OBS | Confirmed |
| MW15 | A world-map note's tooltip text is the `UI_Maps` entry 0x0B0C5A53 (arguments 0x0CE92D54 = the note's text, 0x050F3B05 and 0x050F4F05 = the two map-coordinate texts of the note's position, CO §8); for notes whose type has bit 21, 22, 23 or 50 (the stable-master types) the first argument is the entry 0x0DDC06F5 built from the note's text (0x04D94125) and a second text the client obtains for the note's entity (0x09325495) when that lookup succeeds; when the type has none of those bits, or the lookup fails, the first argument is the note's text itself (§6.3) | BIN (q5); OBS (tokens and argument IDs exist in 0x250001E1) | BIN |
| MW16 | Notes the client registers at run time (not from the table) are created with the same template on the shown map, visible under the same filter test; a note of type value 0x8000000 (`FellowshipLeader`) is drawn 2 z-levels and one of type 0x400000000 (`Fellow`) 1 z-level in front of the template's z (§6.4) | BIN (q5) | BIN (their source not traced) |
| MW17 | The player marker is an instance of `MapPlayerMarker` attached to the shown map element (the image field on a dungeon map) unless that element carries `UI_Map_HidePlayerMapnote` true; it is centred on the player's pixel and hidden whenever its box would leave the node's pixel frame; it is turned by the player's heading like the radar's (§6.6) | BIN (q6); OBS (7 active elements carry the flag: `WorldMap` and the 6 territory maps) | BIN |
| MW18 | World point → map pixel on the window uses GP §8.6.1 with the node's region, or the point's region when the node has none; a point whose region or instance differs from that region gets a fixed far-off offset, so it never maps into a frame (§6.1) | BIN (q6) | BIN |
| MW19 | `CoordinateDisplay` shows, while the pointer moves over the map, the map coordinates (CO §8) of the pointed pixel converted back to a world position, in the `WeenieClientMsgs` entry 0x053B8BB3 (arguments 0x753 and 0x6C7, the two coordinate texts); it is visible while `GOP_DisplayCoordinatesOnMap` is true and the node does not hide its filter buttons (§3.4) | BIN (q7); OBS (token, arguments) | BIN |
| MW20 | `UI_Map_HideFilterButtons` hides `FilterMenu`, `MapMenu`, `StablemasterButton` and `CoordinateDisplay` (§3.4) | BIN (q1, q7) | BIN |
| MW21 | `StablemasterButton` opens another panel's page by key and hides the map window; its visibility depends on a game test on the WEENIECONTENT record labelled `TraitNoviceCharacteristic` (§3.4) | BIN (q1) | BIN (test not traced) |
| MW22 | The radar creates its note elements from the `map` layout's `MapNoteTemplate` with object mode 1; `UI_Radar_RadarRadius` is in map pixels: the world radius is the value divided by the radar's georeference scale; a note nearer than that radius is drawn centred on its pixel with its on-screen image, a farther one with its off-screen image on the circle of that radius around the viewport centre, in the direction of the note; both tooltips are the `UI_Maps` entry 0x0B8B58E0 (arguments 0x0DA44875 = the note's text, 0x00E7E725 = the horizontal distance) (§6.8) | BIN (q8); OBS (tokens, argument IDs, radius 81) | BIN |
| MW23 | The radar's filter value is the option `GOP_Radar_MapNoteFilter`; the radar's own `UI_Map_MapNoteDisplayType` is 0x9FFE7FEFDFF80003; its menu commands set the filter to only bit 45, set it to the radar's display mask, or toggle one type's bits (toggling bit 1 toggles bits 0 and 1), then store the option (§6.8) | BIN (q8); OBS (radar layout value) | BIN |
| MW24 | Census of map regions (§5.2): 227 nodes, every active element in the layout, every image a JPEG present in the install (226 of 1,024 × 768, 1 of 1,024 × 766); 1,303 dungeons, all with `Dungeon_MapData` naming one of 1,298 distinct sets (5 sets named by 2 dungeons), all holding `UI_Map_GameMap` with an image present (1,300 map sets in all: these 1,298, 1 named only by a `Dungeon_MiniMapData`, 1 named by the one area with a non-zero `Area_MapData`); frames against the valid block grid as §5.3 | OBS | Confirmed |
| MW25 | The `map` layout holds no editable text element (0 of 1,803 instances set `UICore_Text_editable`); the install's `Commands` table (261 entries) has no command whose names suggest a place search; the location-related names are `loc` and `location` | OBS | Confirmed |
| MW26 | No instruction of the executable has the element IDs of `ZoomOutMapInstructions`, `MapNoteFilterMenu`, `RealScrollMapDisplay` or `RealMapFogOfWarDisplay` as an operand (§3.3) | BIN (operand scan of every instruction) | BIN |
| MW27 | The map window shows a note's tooltip on mouse-over only when the note's pixel is not under an unexplored cell of an active fog of war (§6.3) | BIN (q5) | Probable |
| MW28 | An "open the map" request names the window's element and an optional node; without a node it selects the player's map (MW9), then shows the window (§4.6) | BIN (q3: the handler; its sender, likely the minimap's `MapZoom` event (GP G24), was not traced) | Probable |
| MW29 | Of the 541 DIDs listed by nodes, 540 are area records and 1 (listed by the node of `area_rohan_west_helms_deep`) names a non-area object record; 135 of the 675 area records are listed by no node | OBS | Confirmed |
| MW30 | The argument IDs of `CoordinateDisplay`'s entry are the name hashes (`records.md` §1.5) of `ns` (0x753) and `ew` (0x6C7), in that order in the entry; the names of 0x050F3B05 and 0x050F4F05 of the note tooltip entry were not recovered (no candidate of a coordinate vocabulary hashes to them); that entry places them, in that order, after the note's text (§6.3) | OBS (`WeenieClientMsgs` 0x250001C0 and `UI_Maps` entries: argument IDs and order only; hashes computed, 2026-10-23) | Confirmed 2026-10-23 |
| MW31 | The off-screen images of both icon tables are 36 distinct 30 × 30 surfaces; 35 point to the right edge (east, +x of the image) with the point centred vertically, one (0x410F34EE) is a round marker narrowing downwards; none points another way (§6.8) | OBS (`UI_Map_MapNoteOffscreenImageStruct` of 0x78000001 and 0x78000002, the images decoded, 2026-10-23) | Confirmed 2026-10-23 |
| MW32 | `GameplayOptionDefaults` is the label of key 0x68 of the `UNIQUEDB` DID map (root key 0xF; 0x28000002 here), which names the property set (0x78000006 here); the same set is also listed in the `DBPC` map (key 0x8) and under the `UNIQUEDB` label `Preferences_Gameplay_Option_Defaults` (key 0x10000012); no `WEENIECONTENT` entry names it (§6.9) | OBS (root and second-level DID maps, 2026-10-23) | Confirmed 2026-10-23 |
| MW33 | The 22 nodes with a null `UI_Map_MenuName` (table 0, token 0) are all leaves (no child nodes), each lists 1–5 areas, in regions 2 (8 nodes), 3 (13) and 5 (1), at depths 2–4 (§7.2) | OBS (the map tree of the `map` layout, 2026-10-23) | Confirmed 2026-10-23 |
| MW34 | Interior cells name their dungeon by `Dungeon_DID` in the cell's property set of the landblock-info record (`landblock.md` §4.4; 15,603 cells name 175 dungeons, `sound.md` §10.8.2); every dungeon record has `Dungeon_MapData` (§4.6) | OBS (through `wstate.md` §11.4, `game-panels.md` G18, `sound.md` §10.8.2) | Confirmed (cited) |
| MW35 | The fallback map window of §8.1 (Tessera rule) | This spec | Decision |
| MW36 | `MapMenu` (a Button with `boolean_button`, based on `AvailableMapMenu`) opens its context menu when it is toggled on, at the window's next update, with a fixed position: the button's screen left edge and three units below its last row (no position when either value would be negative); the root menu and every submenu use `ContextMenuElement_MapMenu`; a submenu exists only for a region that the map tree's nodes name and that has a `Region_Map_List` entry; when the menu closes by deactivation, a toggle of the button within 0.2 s is ignored and the button's highlight is cleared once 0.2 s have passed, while a close with a selection clears it at once; `FilterMenu` is an engine Menu (§6.9) (§4.5). Numeric constants under decision 0012: the 3-unit offset and the 0.2 s guard | BIN (q10); OBS (`map` layout resolved, 2026-10-23) | BIN; submenu set Probable |

---

<a id="s2"></a>
## 2. Terms

- **Node**: one struct of the map tree in `MapWindow`'s `UI_Map_AreaData` (GP §9.1). It is
  shown through its **active element**, a top-level Field of the `map` layout.
- **Dungeon map**: a stand-alone property set (prefix 0x78) holding `UI_Map_GameMap` (MD §7,
  GP §8.5.2), named by a dungeon's `Dungeon_MapData`.
- **Shown map**: the node or dungeon map the window currently shows; **shown map element**: the
  node's active element, or `DungeonMap_ImageField` for a dungeon map.
- **Map pixel**: a pixel of the shown map's pixel frame: `UI_Map_PixelWidth` ×
  `UI_Map_PixelHeight` of a node (1,024 × 768 here), the image's size for a dungeon map. GP
  §8.6.1 converts world points to map pixels.
- **Display scale**: the ratio between the shown map element's on-screen size and its pixel
  frame (§4.8). All positions below are in map pixels unless stated.
- **Display mask** of a node: its `UI_Map_MapNoteDisplayType`, ORed with the common mask when
  `UI_Map_IncludeCommonMapNoteTypes` is true (MW3); all 64 bits for a dungeon map.
- **Filter value**: a 64-bit player option that holds the user's note-type choices (§6.9).
- **Feed marker**: a marker of the scenario feed (`MarkerAdded`, SF §6.5).
- Bit *i* of a note type is the `MapNoteType` key *i* + 1 (MD §6.3). Masks are written in hex.

---

<a id="s3"></a>
## 3. The parts of the map window

<a id="s3-1"></a>
### 3.1 Two presentations (BIN, MW2)

The map window has two presentations; both show the same `MapField` subtree.

| Presentation | Shown root | Where `MapField` is attached | Visible button |
|---|---|---|---|
| Full screen | `MapWindow` (top-level element of `map`, 1,024 × 768) | `MapWindow` (its own child) | `ToggleMapFullscreenCollapse` |
| Windowed | `MapPanel` (P10 root, a `UIScalerPanel`, 1,038 × 802, minimum 651 × 512, resize aspect 1.2715, saves location and size) | `MapPanelPadding` (rect (5, 26, 1,024, 768)) | `ToggleMapFullscreenExpand` |

- When the window is shown, the client reads the player option `GOP_MapInFullscreenMode`
  (default **true**, MW14) and uses that presentation.
- Clicking the visible one of the two buttons (their click action is
  `ToggleMapPanelFullscreen`) or the key bound to that action while the window has the
  keyboard switches presentation and stores the option. The other button of each pair
  (`Toggle_Map_Panel`, click action `ToggleMapPanel`) closes the window.
- How the full-screen `MapWindow` is sized to the screen was not traced (its UI scaler group
  is the same, 66, as `MapPanel`'s).

> **OPEN:** the screen placement and scale of `MapWindow` in full-screen presentation.
> **Tessera rule:** centre it on the screen, scaled uniformly by the largest factor that fits
> the screen (never above the UI scale × 2), keeping its 4 : 3 frame; the display scale of §4.8
> follows.

<a id="s3-2"></a>
### 3.2 `MapWindow` and `MapField` (OBS, rects at design size; roles BIN MW1)

Lower z-levels are in front (UL §9.3). "Om" is `UICore_Element_ObjectMode` (UL §5.9).

| Element | Type | Rect (x, y, w, h) | z | Om | Role and fill rule |
|---|---|---|---|---|---|
| `MapWindow` | Field | (0, 0, 1,024, 768) | −1,500 | 0 | Full-screen root; carries `UI_Map_AreaData` (GP §9.1) and the quest-objective indicator properties (corner images NE/NW/SE/SW, a tile list, alpha 0.65) |
| ↳ `MapField` | Field | (0, 0, 1,024, 768) | −1,500 | | The part that moves between presentations (§3.1) |
| ↳↳ `MapPadding` | Field | (0, 0, 1,024, 768) | −1,400 | 2 | Background art (Tile); behind the map |
| ↳↳ `MapBackground` | Field | (0, 0, 1,024, 768) | −1,500 | 1 | Parent of the shown node's active element (§4.2); blocks clicks |
| ↳↳↳ *shown active element* | Field | (0, 0, 1,024, 768) | layout | | The node's map (§4.2) |
| ↳↳↳ `MapQuestObjectiveIndicatorsParent` | Field | (0, 0, 1,024, 768) | −5 | 1 | Quest-guide shading (§6.5); hidden at creation |
| ↳↳↳ `CoordinateDisplay` | Text | (52, 730, 450, 25) | −10 | 1 | Pointer coordinates (§3.4) |
| ↳↳↳ `QuestGuideOptionField` › `QuestGuideCheckbox`, `QuestGuideOptionText` | Field › Button, Text | (38, 714, 188, 20) | −1,500 | | Quest-guide switch (§3.4) |
| ↳↳ `MapMenuScaleParent` | Field | (52, 18, 800, 25) | −1,500 | 1 | Holds the three controls; scaled with the UI scale |
| ↳↳↳ `FilterMenu` | Menu | (0, 0, 150, 25) | −1,500 | 1 | Note-type filter (§6.9); popup list from layout 0x22000080; does not display its selection; static label `FilterMenu_Label` |
| ↳↳↳ `MapMenu` | Button (toggle) | (151, 0, 150, 25) | −1,500 | 1 | Opens the map menu (§4.5); static label `ShowMap_Label` |
| ↳↳↳ `StablemasterButton` | Button | (302, 0, 200, 23) | −1,500 | | §3.4 |
| ↳ `MapDecoration` | Field | (−73, 0, 1,168, 768) | −1,600 | 1 | Frame art in front of the map |
| ↳ `MapButtonsParent` | UIScaler | (985, 2, 39, 16) | −3,000 | 1 | `ToggleMapFullscreenCollapse` (0, 0, 16, 16), `Toggle_Map_Panel` (20, 0, 16, 16) |

`MapPanel` (the windowed root) holds the frame art (`Panel_BaseBox_BG`), `MapPanelPadding`
(with the title bar `Panel_TitleBar` › `Panel_Label_Title`, `ToggleMapFullscreenExpand`,
`Toggle_Map_Panel` and a resize grip image), eight `ResizeBar`s and the `MoveBar`. Its title is
the panel's own `UI_StringInfo` by the generic panel rule (UL); the map code does not set it
(Probable: no reference found).

<a id="s3-3"></a>
### 3.3 Other top-level elements of `map` (OBS; use BIN)

| Element | Rect / size | Use |
|---|---|---|
| `MapPlayerMarker` | 28 × 28, z −500, Om 1, image 0x41007F4C | The player marker (§6.6) |
| `DungeonMap` | (0, 0, 1,024, 768), z −500 | Dungeon-map view (§4.7): `DungeonMap_ImageField` (112, 84, 800, 600, Om 1) › `MapFogOfWarDisplay` › `MapFogOfWarField` (a `MapFogOfWar`); `DungeonMap_OverlayField` (0, 0, 1,024, 768, Om 1, frame image 0x4100813A); `Toggle_FOW` (a close button, click action `ToggleMapPanel`) |
| `MapNoteTemplate` | 30 × 30 Field, z −1, default state `normal`, child `MTButton` (30 × 30 Button, tooltips on, mouse-visible alpha threshold 128, tooltip layout `ToolTip_02`) | One note on the world map and on the radar (§6.3, §6.8) |
| `MapQuestLandblockObjectiveTemplate`, `MapQuestRadiusObjectiveTemplate`, `MapQuestStaticObjectiveTemplate` | 50 × 50 (z −400), 50 × 50 (z −401), 32 × 32 (z −10,000) | Quest-guide shading pieces (§6.5) |
| `QuestGuideQuestListHolder` | UIScaler | Quest-guide list (QJ; hidden at creation) |
| `ZoomOutMapInstructions`, `MapNoteFilterMenu`, `RealScrollMapDisplay`, `RealMapFogOfWarDisplay` | | **Not used** by the client's code (MW26); Tessera never creates them as elements of their own (`MapNoteFilterMenu` is still resolved by UL §7 as the layout base of `FilterMenu`, §6.9) |
| 227 active elements, and 202 other buttons and fields | | Nodes (§4) and their child-map elements |

<a id="s3-4"></a>
### 3.4 Fill and visibility rules of the controls (BIN, MW13, MW19–MW21)

Evaluated each time a map is shown (§4.2), in this order:

1. If the shown node has `UI_Map_HideFilterButtons` true (7 nodes: `WorldMap` and the 6
   territory maps), hide `FilterMenu`, `MapMenu`, `StablemasterButton` and `CoordinateDisplay`.
2. Otherwise show `FilterMenu` (but §6.9: hidden on `WorldMap`, which step 1 already covers
   here) and `MapMenu`; show `StablemasterButton` only when the client's game test of MW21
   passes; show `CoordinateDisplay` while `GOP_DisplayCoordinatesOnMap` is true.
3. `QuestGuideOptionField` is shown when a client-wide quest-guide test (not traced) passes,
   the quest guide is allowed on the shown node (not `UI_Map_QuestGuideDisabled`, which the
   same 7 nodes carry) and the private-encounter override
   `PrivateEncounter_ShowQuestGuide_Override`, when present, allows it; `QuestGuideCheckbox`
   shows `GOP_QuestGuide_Map`, and clicking it stores the new value.

**`CoordinateDisplay` text** (MW19): on every pointer move over the map, the pointer's offset in
the shown map element is converted to map pixels (× pixel frame ÷ on-screen size, truncated),
then back to a world point by the inverse of GP §8.6.1 (*G*<sub>x</sub> = 160·*bx* + (*px* −
*px*<sub>0</sub>) / *s*, *G*<sub>y</sub> = 160·*by* + (*py*<sub>0</sub> − *py*) / *s*, in the
node's region), and that point's two map-coordinate texts (CO §8) fill the `WeenieClientMsgs`
entry of MW19. When the conversion fails (scale 0) the text is left unchanged.

**`StablemasterButton`**: a click opens another panel at its stable-masters page (the client
names the panel and page by key) and hides the map window.

**Tessera rules:** `StablemasterButton` is hidden (Tessera has no panel for that page; the
game test is not traced). `QuestGuideOptionField` is hidden until the quest guide is
implemented (QJ §14 Q7). `CoordinateDisplay` follows step 2 with Tessera's setting
`map_coordinates` (default from `GOP_DisplayCoordinatesOnMap` read at run time) and is
rendered with Tessera's text rules (ST §9).

---

<a id="s4"></a>
## 4. Navigation

<a id="s4-1"></a>
### 4.1 Building the tree (BIN, MW3; data GP §9.1)

The tree is read once, when the window is created, depth-first from the root, children in array
order. A node whose active element or menu name is missing is not added (none here). Each node
keeps the fields of MW3 and the active element of its parent (0 for the root). Areas are then
registered as MD §4.8.4 step 3 says (MD43): first registration wins, unlisted areas map to the
root.

<a id="s4-2"></a>
### 4.2 Showing a node (BIN, MW4)

To show the map whose active element is *E*:

1. If *E* is already shown and is not `DungeonMap`, stop.
2. Instantiate *E* as a child of `MapBackground` (the previous shown element, its notes and its
   marker are removed).
3. If *E* is no node's active element: show *E* as the layout draws it, with no note, marker,
   menu or filter update, and stop (the client logs an error). Three child-map elements lead
   here (MW5; §4.3).
4. For a node, set its `UI_Map_MapImage` as *E*'s image with **draw mode 2**. For
   `DungeonMap` see §4.7.
5. Read the filter value (§6.9), rebuild `FilterMenu`, create the notes (§6.3, §6.4), apply
   §3.4, create the player marker (§6.6), and refresh the quest-guide pieces.

<a id="s4-3"></a>
### 4.3 Child maps (BIN, MW5)

A left click or double left click on any element below the window whose properties hold
`UI_Map_Child_Map` shows the element that property names (§4.2). The elements are the 773
child-map buttons and fields of the active elements (GP §9.2); 770 name a node. The other 3 name
the top-level fields `Moria_area_moria_balrogs_lair`, `Region_Rohan_East` and
`Region_Rohan_West`, which are no node's active element; showing them follows §4.2 step 3.
Clicking the player marker while an outdoor node is shown and the player is in a dungeon shows
the dungeon map (§4.7).

**Tessera rule** (replaces GP §9.2's "does nothing", §11): follow the client, including §4.2
step 3 for the three non-node targets.

<a id="s4-4"></a>
### 4.4 Going up (BIN, MW6)

A right click or double right click on the window (any element below its root) goes up:

1. If the shown node has a parent, show the parent.
2. Else, if a dungeon map is shown and the player is in a dungeon, show the player's outdoor
   map (MW9 without the dungeon step: the node of the player's area, `WorldMap` when
   unlisted).
3. Else (the root is shown, or no player or note manager) the client makes a call whose effect
   was not traced.

When the shown node has `UI_Map_PreventZoomOut` (1 node here, `CityMap_Winter_Festival`), the
client makes that same call first and then still continues with step 1.

Each successful step plays a UI sound (not identified).

> **OPEN:** what the call of steps 3 and of `UI_Map_PreventZoomOut` does (it reaches an
> interface of the window whose first operation was not identified; Probable: it re-selects the
> player's map as MW28 does). Owner observation O141 (§13).
> **Tessera rule:** step 3 does nothing; on a node with `UI_Map_PreventZoomOut` a right click
> does nothing (the property's name; the traced continuation to the parent is recorded as
> OPEN rather than copied).

<a id="s4-5"></a>
### 4.5 The map menu (BIN, Probable order, MW8)

`MapMenu` is a toggle button; a click opens a context menu (a second click within a short time
after it closed does not reopen it). The menu is built from `RegionControl` (WEENIECONTENT
label `RegionControl`; WS §11.3):

1. One submenu per entry of `Region_Map_List` ({`Region_ID`, `Region_Name`}), labelled by
   `Region_Name` (6 entries here, stored order regions 1, 2, 3, 4, 14, 5); an entry under
   which no node is listed gets none (rule 3 of "Opening and placement" below; here every
   entry has nodes, MW8).
2. In each submenu one item per node whose `UI_Map_RegionID` equals the entry's region,
   labelled by the rendered `UI_Map_MenuName`, sorted by label. The root (no region) is in no
   submenu.
3. Choosing an item shows its node (§4.3).

The menus (root and submenus) are instances of `ContextMenuElement_MapMenu` (layout
`ContextMenu`; rule 2 below; corrected in the round-4 revision from "items").

**Tessera rule** for what the client leaves untraced: submenus in `Region_Map_List` order,
items sorted by their rendered labels in ascending order of Unicode scalar values after
simple lower-casing, ties by depth-first tree order.

**Opening and placement (BIN, MW36; round-4 revision).** `MapMenu` is a Button with
`boolean_button` (from its base `AvailableMapMenu`). When it is toggled on, the window builds
and opens the menu at its next update; toggled off, it closes an open context menu (Probable).

1. The menu is given a fixed position (`ui-widgets.md` §19.2): its top-left corner at the
   button's screen left edge and three units below the button's last row, so it opens just
   below the button; when either value would be negative no position is given and the menu
   opens at the pointer (§19.3 there). The menu is then kept on the screen by §19.3.
2. The root menu and every submenu use the element `ContextMenuElement_MapMenu` of layout
   `ContextMenu` (the window sets it on each).
3. A submenu exists for a region only when nodes of the map tree name that region and
   `Region_Map_List` has an entry for it (Probable); an entry under which no node is listed
   gets no submenu, and nodes of a region without an entry are in no submenu.
4. **Reopen guard.** When the menu closes because the player activated something outside it
   (`ui-widgets.md` §19.4 step 4), the window notes the time. A toggle of the button within
   0.2 s after that is ignored (so the click that closed the menu by its press does not open it
   again), and the button's highlight is cleared once 0.2 s have passed. When the menu closes
   with a selection, the button's highlight is cleared at once.

> **OPEN (implementer, 2026-10-23):** (`tessera-ui` `game::mapwindow`) this section still
> described Tessera's retired code-made popup (columns, a second popup beside the row). The
> client now opens `MapMenu` and `FilterMenu` as `ui-widgets.md` §19 context menus: element
> `ContextMenuElement_MapMenu` when the install's layout resolves, one submenu per region
> entry, `FilterMenu` check items that keep the tree open after a selection (§6.9). As a
> Tessera choice each menu's top-left corner is at its button's bottom-left corner (not at the
> pointer, §19.3's default), and a click on the button within 0.3 s of the menu closing does
> not reopen it. The press of that click deactivates the menu, so it closes before the button's
> click arrives. Whether the request's element also applies to submenus, and what a
> `Region_Map_List` entry with no named node shows, is not stated: Tessera uses the request's
> element for every menu of the tree, and an entry without named nodes is a disabled submenu
> item. Please confirm or replace this placement and the 0.3 s guard.
>
> **Answered (spec writer, 2026-10-23, round-4 revision; MW36):** replaced by the four rules
> above: the menu's top-left is three units below the button's last row (not at its
> bottom-left corner); the guard is 0.2 s, started by a close through deactivation; every
> menu of the tree uses `ContextMenuElement_MapMenu` (confirmed); an entry without nodes gets
> no submenu item at all (not a disabled one). `FilterMenu` is not a context menu: §6.9.
> **Implemented behaviour changes:** the menu's position (two units lower), the guard's length,
> the empty-region item, and `FilterMenu` (§6.9).

> **Was OPEN:** (implementer, 2026-10-11, `tessera-ui` `game::mapwindow`, `game::popup`)
> `tessera-ui` has no context-menu widget that game code fills, so `MapMenu` and `FilterMenu`
> open a code-made popup below their button: the map menu as one list of the
> `Region_Map_List` entries, each opening a second popup beside its row with that region's
> node items (revised 2026-10-11: one list of all 211 items did not fit a 768-pixel screen),
> the filter menu as one list of command and check rows with the type's menu image. A popup
> opens in front of every root panel (so in front of `MapWindow` at z −1,500), and one
> taller than the screen is laid out in columns. Nodes whose `UI_Map_MenuName` is null
> (§7.2 OPEN) get no item. Please confirm this presentation, or give the `ContextMenu`
> layout's submenu structure as a rule.
>
> **Answered (spec writer, 2026-10-23, round-4 revision):** superseded: `ui-widgets.md` §19
> gives the context menu, and the rules above give `MapMenu`'s use of it; `FilterMenu` is the
> Menu of §6.9.

<a id="s4-6"></a>
### 4.6 Which map is shown (BIN, MW9, MW28)

**The player's map** is chosen by these steps:

1. Get the player's (dungeon, map-data) pair (the client asks a game-script function; GP
   §8.5.1). If its dungeon differs from the last one recorded, record it and its map data;
   with no dungeon, show the node of the player's area (step 3).
2. If a dungeon is recorded and the shown map is not that dungeon's map, show `DungeonMap` with
   the recorded map data (§4.7).
3. Otherwise, if the player's position is invalid (CO §4), change nothing. Else find the area
   of the position (by the client's game logic; the landblock's `Area_DID`, LB §5, WS §11.2),
   look it up in the area registration (MD §4.8.4) and show that node; **`WorldMap`** when the
   area is registered to no node.

**When:** the dungeon step runs when the player's dungeon changes; the whole selection runs on
an "open the map" request without a named node (MW28). Showing or hiding the window by itself
(`ToggleMapPanel`) keeps the shown map. Navigation (§4.3–§4.5) never changes the recorded
dungeon.

There is no rule by region or by pixel bounds: an area in no node shows the root (MD43; 135
area records are listed by no node, MW29).

**Tessera rules:** the player's dungeon is the sandbox's `Dungeon_CurrentDungeon` of the player
when it has one, else the dungeon whose cell contains the position (WS §11.4), else none; its
map data is that dungeon's `Dungeon_MapData` (GP G18). The selection runs when the window is
first shown in a session, on `MapZoom` (GP §8.7.5), and when the player's dungeon changes.

**The player's dungeon in the client half (Tessera rule, revision 1, 2026-10-23; MW34).** The
client half takes the player's dungeon from these sources, the first that gives one:

1. a `Dungeon_CurrentDungeon` the session reports for the player (a scenario may set it as a
   property of the player; the sandbox reports none by itself);
2. when the player's position is in an interior cell, that cell's `Dungeon_DID`, read from
   the cell's property set in the landblock-info record of the player's landblock (the same
   lookup the minimap makes for its indoor map, `game-panels.md` §8.5.1);
3. none: the player is outdoors, or the cell names no dungeon.

The cell number the walker reports is enough for step 2: the client half reads the
landblock-info record itself (asynchronously, as the minimap does; until it has loaded, the
shown map stays as it is). The dungeon's map data is its `Dungeon_MapData` (MW34).

> **Answered (spec writer, 2026-10-23; revision 1):** the input is the cell's `Dungeon_DID`
> (step 2 above); the session need not carry `Dungeon_CurrentDungeon`. **Implemented
> behaviour changes:** for a player in an interior cell whose cell names a dungeon, the map
> window's player's map becomes that dungeon's `DungeonMap` (§4.6 step 2) instead of the
> outdoor node. The implementer's text is kept as the record.
>
> (Was **OPEN:** implementer, 2026-10-11, `apps/client` `map::ui`.) The client half does not
> yet know the player's dungeon: the session carries no `Dungeon_CurrentDungeon` for the player,
> and the walker's minimap input gives the cell number but not the cell's dungeon. The map
> window therefore shows the player's outdoor map (the node of the area of the player's
> landblock, read from the install's block maps) and never `DungeonMap` for the player;
> dungeon maps are shown by place lookup (§7.3) and by the census (§5.4). The area of an
> interior cell's landblock is used for a player inside. The rule stands; the input is the
> gap.

> **OPEN (implementer, 2026-10-23):** (`apps/client`) source 1 above, a
> `Dungeon_CurrentDungeon` the session reports for the player, is not wired: the scenario feed
> and the net layer have no player-property form that carries it, so only source 2 (the
> interior cell's `Dungeon_DID`) is used. Which feed or protocol form should carry it?
>
> **Answered (spec writer, 2026-10-23):** none exists yet, and none is needed for the rule to
> work. Source 2 gives the dungeon for every interior cell that names one (MW34), which is the
> case the original's dungeon map covers. A feed form that sets `Dungeon_CurrentDungeon` on
> the player belongs to `scenario-feed.md`, not to this spec:
>
> > **OPEN (for `scenario-feed.md`, non-blocking):** a player-property step that sets
> > `Dungeon_CurrentDungeon`, so a scenario can put the player in a dungeon without placing
> > them in a cell. Until it exists, source 1 is never available and the client half uses
> > source 2.
> > (Carried over to [`scenario-feed.md` §16](scenario-feed.md#s16) in the round-4 revision.)
>
> An official-server session (M8) is out of scope here.

> **OPEN (implementer, 2026-10-23; engineering choice):** a dungeon that is not among the
> install's dungeon records, has no `Dungeon_MapData`, or whose map data names no map region
> counts as no dungeon, and the outdoor map is shown. On this install every dungeon resolves.
>
> **Answered (spec writer, 2026-10-23):** confirmed as a **Tessera rule**: in each of those
> cases step 1 above records no dungeon and the selection goes on to step 3 (the node of the
> player's area, or `WorldMap`). How the original treats such a dungeon was not traced; none
> occurs in this install.

<a id="s4-7"></a>
### 4.7 Dungeon maps (BIN, MW10)

To show a dungeon map with map data *M* (a `UI_Map_GameMap`):

1. Instantiate `DungeonMap` under `MapBackground`.
2. Give `DungeonMap_ImageField` object mode 3 (CanvasResize) and resize it to *M*'s image size;
   set the image with draw mode 2; then give it object mode 1 (Stretch) and resize it back to
   its layout size, 800 × 600. The canvas, holding the image and every child attached to the
   field, is scaled to 800 × 600 with no regard to aspect ratio (LPA §13.18).
3. The pixel frame is the image's size; notes and the player marker are attached to the image
   field in image pixels and so scale with it.
4. When *M* has `UI_Map_FogOfWar` true, the fog elements are sized to the image and the explored
   set is applied (GP §8.5.3, OPEN); otherwise they are hidden.
5. `DungeonMap_OverlayField` (the frame image) is drawn in front, over the whole 1,024 × 768.

All 1,298 dungeon images (MW24) are JPEGs of 11 sizes; 1,285 are 4 : 3 (800 × 600: 962;
1,600 × 1,200: 294; 2,400 × 1,800: 24; 3,200 × 2,400: 6; 1,200 × 900: 1) and are scaled
uniformly; 13 are not (1,600 × 2,400: 4; 800 × 1,800: 3; 3,200 × 3,600: 3; 1,200 × 1,400: 1;
4,800 × 4,800: 1; 2,400 × 3,000: 1) and are distorted by step 2 in the original.

**Tessera rule:** draw exactly as steps 1–5 (distortion included); the fog is hidden (GP
§8.5.3 Tessera rule).

<a id="s4-8"></a>
### 4.8 Display scale; no zoom (BIN, MW7)

`MapBackground` and `DungeonMap_ImageField` have object mode 1 (Stretch): their content is laid
out in map pixels and scaled to their on-screen size, which follows the panel's size (resizable
in windowed presentation, with a fixed aspect) and the UI scale. Positions of notes, markers and
the pointer convert by that scale (§3.4). Within a node the original has **no** zoom, pan or
scroll: the only "zoom" is the tree (§4.3–§4.4).

---

<a id="s5"></a>
## 5. "Every map region"

<a id="s5-1"></a>
### 5.1 Definition (Tessera rule from the client's two kinds of shown map, MW4, MW10)

A **map region** is one thing the map window can show with its own georeference:

- **R-node**: every node of the map tree (§4.1), identified by its active element ID; and
- **R-dungeon**: every distinct map-data set named by a `Dungeon_MapData` of a dungeon record
  (WS §11.4), identified by the set's DID.

Not map regions: the 3 non-node child-map targets (no georeference, §4.3); the map set named
only by a `Dungeon_MiniMapData` (radar only); the map set named by `Area_MapData` (no traced
path shows it in the map window); minimap tiles (GP §8.4).

<a id="s5-2"></a>
### 5.2 Counts in this install (OBS, Confirmed, MW24)

| Kind | Count | Notes |
|---|---|---|
| R-node | 227 | depth 0–5 (1, 6, 72, 108, 35, 5); 226 with a region |
| R-dungeon | 1,298 | named by 1,303 dungeons; 5 sets named by 2 dungeons each |
| Total | **1,525** | |
| Excluded map sets | 2 | 0x780012C4 (mini-map only), 0x780000A3 (`Area_MapData` of area 0x70003D6F) |

<a id="s5-3"></a>
### 5.3 What cannot be shown, by category (OBS)

| Category | Meaning | Count here |
|---|---|---|
| C1 no element | the node's active element is not a top-level element of `map` | 0 |
| C2 no image | the image DID names no record | 0 |
| C3 bad image | the image record does not decode | 0 (all 1,525 are JPEG) |
| C4 no georeference | scale magnitude ≤ 0.0002, or a georeference field missing | 0 |
| C5 frame outside the world | no pixel of the frame maps to a valid position (blocks 0–254, CO §4) | 2: `WorldMap` (its frame maps to *G*<sub>y</sub> from −92,488 to −48,142) and 0x78000419 (*G*<sub>x</sub> from −534 to −1) |
| C6 frame partly outside | some pixels map outside the valid blocks | 3 nodes (`TerritoryMap_Mordor`, `area_shelobs_lair`, `area_umbar_catacombs_the_neaths`) and 106 dungeon maps |

Regions of C1–C4 cannot be drawn; C5 regions are drawn but no world point can be marked on them
(the root's player marker is hidden by data anyway, MW17); C6 regions are drawn and marked
normally.

<a id="s5-4"></a>
### 5.4 Census procedure (Tessera rule, for P5 exit criterion 1)

For each map region, in order (R-nodes depth-first in tree order, then R-dungeons by ascending
DID):

1. Show it (§4.2 or §4.7) in the full-screen presentation at UI scale 1 (display scale 1 for
   nodes; 800 ÷ *W* by 600 ÷ *H* for dungeon maps).
2. **Test point**: intersect the frame with the valid block rectangle (global coordinates 0 ≤
   *G* < 255 · 160 on both axes); take the centre of that intersection; convert it to a
   `Location` (region = the node's region; for a node without one or a dungeon map, the region
   of the dungeon's position when known (WS §11.4), else 1 — the pixel does not depend on it,
   §6.1; *z* = 0).
3. Emit one feed marker there (§6.7), with label "census" and no icon.
4. Pass when: the map's image is drawn into the shown element; the marker's icon is drawn
   centred within 1 map pixel of the forward transform (GP §8.6.1) of the test point; and no
   error is logged. C5 regions pass without step 2–3 and are reported as C5.
5. Report the counts of each category and of failures with their region IDs. On this install
   the expected result is 1,525 drawn, 1,523 marked, C5 = 2, no failure.

The census reads every image and is meant for real-data tests and the explorer sheet; it runs
headless (scenario feed driver) and needs no GPU beyond the painter.

---

<a id="s6"></a>
## 6. Overlays

<a id="s6-1"></a>
### 6.1 World point to map pixel on the world map (BIN, MW18)

GP §8.6.1's transform, with these rules for the map's origin position (block (*bx*, *by*) of
the georeference):

- its region is the node's `UI_Map_RegionID`; when the node has none (the root, every dungeon
  map), the point's own region;
- when the point's region, or its instance, differs from the origin's, the client uses a fixed
  offset of −2,000 units on each axis instead of the block difference, which puts the pixel far
  outside every frame here. So **points of another region are never drawn** on a node with a
  region.

<a id="s6-2"></a>
### 6.2 Drawing order on the world map (OBS z-levels, BIN attachment)

From back to front (UL §9.3: lower z in front; equal z by read order):

| # | Layer | Parent | z |
|---|---|---|---|
| 1 | `MapPadding` art | `MapField` | −1,400 |
| 2 | The shown map image (the active element's image; on a dungeon map the image field) | `MapBackground` | layout |
| 3 | Child-map buttons and fields of the active element | active element | layout |
| 4 | Table and run-time notes (§6.3, §6.4) | shown map element | −1 + relative z |
| 5 | The player marker (§6.6) | shown map element | −500 |
| 6 | Quest-guide shading (§6.5) | `MapQuestObjectiveIndicatorsParent` | −5 (its pieces −400, −401, −10,000) |
| 7 | `CoordinateDisplay` | `MapBackground` | −10 |
| 8 | Quest-guide option, controls | `MapBackground`, `MapMenuScaleParent` | −1,500 |
| 9 | `MapDecoration` frame art | `MapWindow` / panel | −1,600 |
| 10 | Full-screen and close buttons | `MapButtonsParent` | −3,000 |

On a dungeon map, `DungeonMap_OverlayField` is drawn in front of the image field (same parent,
later read order).

<a id="s6-3"></a>
### 6.3 Table notes on the world map (BIN, MW11, MW12, MW15, MW27; MD §4.8)

1. **Selection**: MD §4.8.1–§4.8.2 (queries, discovery, content layer), then MW11: a note is
   **created** only when `type` & display mask ≠ 0 (any type on a dungeon map) and, outdoors,
   the node of its area is the shown node or a descendant. On nodes whose display mask is 0
   (`WorldMap`, `RegionMap_16_Dunland`) no table note is created; on the 6 territory maps only
   bits 27, 34 and 46 (the fellowship types) are in the mask.
2. **Element**: an instance of `MapNoteTemplate` attached to the shown map element; `MTButton`
   carries the note's three images (normal, selected, highlight) from the map window's icon
   table (MD §4.8.5) as its states; the note's z is the template's plus the entry's
   `UI_Map_MapNoteRelativeZLevel` (Probable). It is centred on the note's map pixel (§6.1).
3. **Visibility**: visible exactly when `type` & filter value ≠ 0 (MW12; link rows: filter bit
   1, but bit 0 is never created on outdoor nodes and link rows have no icon). World-event
   states (MD §4.8.6) are applied after.
4. **Tooltip** (`ToolTip_02` layout): the text of MW15: the note's text and the two map
   coordinates of its position; for the stable-master types the composite first argument.
   It is shown on mouse-over, except over unexplored fog (MW27).

**Argument order (revision 1, 2026-10-23; MW30).** `CoordinateDisplay`'s entry (MW19) takes
the north–south text (latitude, the `S`/`N` part of CO §8) in 0x753 and the east–west text
(longitude) in 0x6C7: the two IDs are the name hashes of `ns` and `ew`, and the entry writes
them in that order. **Tessera rule** for the note tooltip (MW15), whose two argument names were
not recovered: 0x050F3B05 takes the latitude text and 0x050F4F05 the longitude text, the order
in which the entry writes them and in which CO §8 and `CoordinateDisplay` give coordinates. The
client may now render both entries from the install (with the coordinate texts of CO §8) and
falls back to the Tessera wording below only when an entry is missing.

> **Answered (spec writer, 2026-10-23; revision 1):** 0x753 = latitude, 0x6C7 = longitude
> (data); 0x050F3B05 = latitude, 0x050F4F05 = longitude (Tessera rule). **Implemented
> behaviour may change:** the tooltip and `CoordinateDisplay` can render the install's entries
> instead of the Tessera wording. The implementer's text is kept as the record.
>
> (Was **OPEN:** implementer, 2026-10-11, `apps/client` `map::ui`.) MW15 names the arguments
> 0x050F3B05 and 0x050F4F05 as "the two map-coordinate texts" without saying which holds the
> latitude and which the longitude (and MW19 likewise for 0x753 and 0x6C7). Until that is
> stated the client builds a note's tooltip, and `CoordinateDisplay`'s text, in Tessera's
> wording (the note's text, then the coordinates "12.3S, 45.6W" of CO §8 on a second line,
> as feed markers do, §6.7 item 6) instead of rendering the `UI_Maps` and
> `WeenieClientMsgs` entries. Please state the argument order.

> **OPEN (implementer, 2026-10-23):** (`apps/client`, MW15) for the stable-master types (bits
> 21, 22, 23, 50) the tooltip's first argument is the note's plain text, not the 0x0DDC06F5
> composite: the client half has no lookup for the note entity's second text (0x09325495).
> Does the original use the composite with an empty second text, or the plain text, when that
> lookup fails?
>
> **Answered (spec writer, 2026-10-23; MW15, BIN q5):** the plain text. The original builds
> the composite only when the note's type has one of the four bits **and** the lookup of the
> second text succeeds; otherwise the first argument is the note's own text, unchanged. A
> client half that cannot look the second text up therefore matches the original's
> failed-lookup case. The implementation conforms.

> **OPEN (implementer, 2026-10-23):** (`apps/client`) each coordinate text is the English form
> of CO §8.3 (`4.0S`); the per-coordinate `Position` template 0x08D46694 is not rendered.
> Exact for English only.
>
> **Answered (spec writer, 2026-10-23):** the rule is CO §8.3: each coordinate string is the
> `Position` template (string table under STRINGTABLE key 0xC, token 0x08D46694; argument
> 0x539D the number with one decimal place, argument 0x48E2 the letter as a nested StringInfo
> naming one of the four letter tokens of the same table), rendered by `string-table.md` §9,
> and the pair is then 0x053B8BB3 of `WeenieClientMsgs` (above). **Tessera rule:** the English
> form of CO §8.3 is the fallback only when that table or token is missing or does not
> render. With an English install the two give the same text, so this is a conformance gap
> for other locales, not a visible change here.

<a id="s6-4"></a>
### 6.4 Run-time notes (BIN, MW16)

Besides the table, the client keeps notes it registers at run time (the icon tables have types
such as `Fellow`, `FellowshipLeader`, `Selection`, `Pet`, `Enemy`, `Warband`, `Waypoint`,
`TrackedAnimal`, `SharedTracking`, `Admin`; their source is server and game state, not traced).
They use the same template, the same filter test, the same icon tables and the note's own text
as tooltip; `FellowshipLeader` notes are drawn 2 and `Fellow` notes 1 z-level in front of the
template's z. Other players who are not fellows have no note type and are not drawn.

**Tessera rule** (not needed for exit criterion 1): members of the player's fellowship
(`FellowshipChanged`, SF §6.4) whose entity is known and placed (`EntityMoved`) are drawn as
`Fellow` notes (the leader as `FellowshipLeader`), labelled by name, subject to the filter. No
other run-time note is drawn in P5.

<a id="s6-5"></a>
### 6.5 Quest-guide shading (OPEN)

The window shows the quest guide's objective areas through `MapQuestObjectiveIndicatorsParent`
and the three templates of §3.3, with the corner and tile images and alpha 0.65 of `MapWindow`
(QJ §8.3, `quests.md` §16). Their geometry was not traced for this spec.

> **OPEN:** how objective areas become landblock tiles, radius circles and static icons, and
> their placement (QJ §14 Q7). **Tessera rule:** not drawn in P5; `MapQuestObjectiveIndicatorsParent`
> stays hidden.

<a id="s6-6"></a>
### 6.6 The player marker (BIN, MW17)

- **Element**: an instance of `MapPlayerMarker` (image 0x41007F4C here, 28 × 28), attached to
  the shown map element (the image field on a dungeon map). It is not created when that
  element carries `UI_Map_HidePlayerMapnote` true (`WorldMap` and the 6 territory maps).
- **Place**: centred on the player's map pixel (§6.1); hidden whenever its box would extend
  past the pixel frame on any side (*x* − *w*/2 < 0, *x* + *w*/2 > width, likewise for *y*).
- **Heading**: turned by the player's heading as the radar marker (GP §8.6.3 and its Tessera
  rule).
- **Updates**: on player movement the marker is re-placed; the fog of war is updated from the
  same position.

<a id="s6-7"></a>
### 6.7 Feed markers (Tessera rules; SF §6.5, QJ §8.3)

The client has no counterpart for a marker that a scenario or test server adds; these rules give
it the look of a run-time note. They apply to every `MarkerAdded`; QJ §8.3 already covers the
quest case and is kept, with the defaults below for what it leaves open.

1. **Kind**: a marker with `quest` is a quest marker (QJ §8.3); one without is a plain marker.
2. **Icon**: `icon` when given (any image DID; drawn at its own size). Otherwise the normal,
   selected and highlight images of the view's icon table entry for type bit 1 (`Quest`) for a
   quest marker, and for type bit 47 (`Waypoint`) for a plain marker (0x41005E62 here in both
   tables). With its own `icon`, the marker keeps that image on mouse-over (QJ §8.3 OPEN (3),
   confirmed as a Tessera rule).
3. **Element and order**: an instance of `MapNoteTemplate` attached to the shown map element,
   centred on the marker's pixel, z −1, after every table and run-time note in read order (so in
   front of them, behind the player marker).
4. **Filters**: feed markers are **not** subject to the display mask, the filter value or the
   discovery and content-layer tests (they are server-placed, and the default filters do not
   include bit 47).
5. **Placement**: drawn when its `location` maps into the shown map's pixel frame by §6.1 (so not
   on a node of another region; on `WorldMap` never, C5). On the radar: §6.8.
6. **Tooltip**: on the world map the marker's `label` and the two map-coordinate texts of its
   `location` (CO §8), in Tessera's own wording (the original's template is game text); on the
   radar the `label` and the horizontal distance as §6.8.
7. **Updates**: a later `MarkerAdded` with the same `marker` replaces it; `MarkerRemoved`
   removes it; markers persist across map changes and are re-placed whenever a map is shown.

<a id="s6-8"></a>
### 6.8 Overlays on the minimap (answers GP §8.8; BIN, MW22, MW23; MD §4.7, §4.8.3)

- **Notes**: selected as MD §4.8.1–§4.8.3 (radar query, link rows not drawn, the radar's type
  filter, ranged notes) with the radar's icon table. Each is an instance of the `map` layout's
  `MapNoteTemplate` with object mode 1.
- **Radius**: `UI_Radar_RadarRadius` (*R* = 81 here) is in **map pixels**. The radar's world
  radius is *R* ÷ *s* with *s* its georeference scale (64.8 units outdoors at *s* = 1.25).
- **Inside the radius** (horizontal distance from the radar's recorded position < *R* ÷ *s*):
  the note's on-screen images, centred on its map pixel.
- **Outside** (and every ranged note, MD §4.7 step 4): the off-screen images (14 types of each
  table have them), placed on the circle of radius *R* pixels around the viewport centre in the
  horizontal direction from the player to the note, and turned to point that way (Probable).
  A note whose entry has no off-screen image is not drawn there.
- **Tooltip**: the `UI_Maps` entry of MW22 with the note's text and the horizontal distance.
- **Filter**: the option `GOP_Radar_MapNoteFilter` (default 0x9FFE7FFFFFFF0003), minus the
  private encounter's disallowed types (MD §4.8.3); the radar's own display mask is
  0x9FFE7FEFDFF80003. Its menu offers: only bit 45; reset to the display mask; per-type toggles
  (toggling bit 1 toggles bits 0 and 1). The menu that sends these commands was not traced.
- **Feed markers** (Tessera rule, extends QJ §8.3): quest and plain markers alike are drawn
  inside the radius only, with the icon of §6.7 item 2 from the radar's table; outside nothing
  (the `Quest` and `Waypoint` entries have no off-screen image); markers of another region are
  not drawn (§6.1 applies through the radar's own origin).
- **Order**: notes and feed markers behind `MinimapPlayerMarker` (Tessera rule; QJ §8.3 OPEN
  (2) confirmed).

> **Answered (spec writer, 2026-10-23; revision 1; MW31):** (a) confirmed by the data: 35 of
> the 36 off-screen images point east (to the image's right edge) when unturned, so turning
> them by the angle of the direction from the player to the note points them at the note; the
> one round image (0x410F34EE) looks the same at every angle except its downward point, which
> it shares with the turning rule. The turn itself stays Probable (the client's rotation was
> not traced; owner row O143). (b) Acceptable as a Tessera rule: drawing the unturned notes
> inside the radius in the viewport's canvas shows the same pixels as giving each its own
> surface; only the turned rim notes need one (`render.md` §6.10). No implemented behaviour
> changes. The implementer's text is kept as the record.
>
> (Was **OPEN:** implementer, 2026-10-11, `tessera-ui` `game::mapmarks`.) Two readings were
> needed. (a) The off-screen image's own direction is not stated; the implementation assumes it
> points east unturned, as `MinimapPlayerMarker` does (GP §8.6.3), and turns it by the angle of
> the direction from the player to the note (Probable item above). (b) MW22 gives every radar
> note object mode 1; the implementation gives it only to the notes on the rim (the ones that
> turn, and so need a surface of their own, `render.md` §6.10) and draws the unturned notes
> inside the radius in the viewport's canvas, which shows the same pixels. Please confirm (a)
> (owner row O143) and whether (b) is acceptable.

<a id="s6-9"></a>
### 6.9 Note-type filters: controls and defaults (BIN, MW12–MW14)

| View | Filter value (option) | Which nodes | Default here | Control |
|---|---|---|---|---|
| Map window, near maps | `GOP_Map_MapNoteFilter` | the 170 nodes naming it (and any node naming none) | 0x1B7C77E7FEFF0000 (36 bits) | `FilterMenu` |
| Map window, distant maps | `GOP_Map_Distant_MapNoteFilter` | the 57 nodes naming it | 0x000444048AE90000 (12 bits) | `FilterMenu` |
| Radar | `GOP_Radar_MapNoteFilter` | — | 0x9FFE7FFFFFFF0003 | the radar's menu (§6.8) |

- **Reading**: when a map is shown, the window reads the option its node names, ANDs it with
  0x9B7C7FFFFEFF0002, and uses the result until the next show. Both map defaults are already
  inside that mask, and the common mask has no bit outside the near default.
- **`FilterMenu` items** (MW13): first two command items, *all* and *none* (`UI_Maps` entries
  0x09F086CC and 0x0EBA8F2C), then one check item per listed bit, labelled by the
  `MapNoteType` display text of key *bit* + 1, sorted by label (collation not traced), each a
  `ContextMenuBool` (icon field `ContextMenuEntryIcon` with the type's
  `UI_Map_MapNoteMenuImage`, draw mode 4; text `ContextMenuText`; check `ContextMenuBoolFlag`
  shown when the bit is set). Listed bits: those of the display mask except 0 (`Link`), 1
  (`Quest`), 38 (`NoIcon`), 45 (`MapNoteOffscreen`), 46 (`FellowOffscreen`) and 48 (`Admin`).
  Item counts here: 25 on 127 nodes, 26 on 88, 27 on 4, 0 on `RegionMap_16_Dunland` (menu with
  only *all* and *none*); hidden on `WorldMap` and the 6 territory maps.
- **Widget** (round-4 revision, `ui-widgets.md` W33): `FilterMenu` is an engine Menu
  (`ui-widgets.md` §11; based on `MapNoteFilterMenu` of the `map` layout), not a context menu.
  Its popup is `Menu_02_PopupField` of layout 0x22000080 with the one-column list
  `Menu_02_PopupListBox` and a vertical scroll bar; its entries are made from `ContextMenuBool`
  of layout `ContextMenu` (its `text_item`); a click on the menu opens the popup below it
  (`ui-widgets.md` §11.3), a second click or a deactivation closes it.
- **Selecting**: *all* sets and *none* clears every listed bit; a type item toggles its bit;
  the menu stays open (`UICore_Menu_auto_close_on_selection` false, a property only the Menu
  class reads). When the value changes, the window stores it in the node's option and updates
  every note's visibility.

> **OPEN (round-4 revision):** a Menu reports no selection for a click on the entry that is
> already selected (`ui-widgets.md` §11.4), so a second click on the same type item would not
> toggle it back unless the window clears the list's selection; whether it does was not traced.
> **Tessera rule:** every click on an item of `FilterMenu` acts (the list keeps no selection
> after an item has acted).

- **Tessera rules:** the three values are kept in Tessera's settings file (per user, not per
  character), initialised from `GameplayOptionDefaults` read at run time (never hard-coded);
  sorting of item labels as §4.5's Tessera rule.

> (Was **OPEN:** implementer, 2026-10-11, `apps/client` `map::data`, `map::hub`; both gaps answered below.) Two gaps the
> client half met. (a) `tessera-settings` has no keys for the three filter values,
> `map_fullscreen`, `map_coordinates` and `map_discovery` yet (ROADMAP P5 deliverables), so the
> client keeps them in memory for the run, initialised as this section says; a change made
> in the filter menu or with the presentation buttons is lost at exit. (b) This spec gives
> `GameplayOptionDefaults` by label and by this install's DID only; the client looks the
> label up as a WEENIECONTENT label, then as a `UNIQUEDB` path entry, and falls back to every
> bit of the read mask (and both switches on) when neither names a set. Please give the
> lookup rule (DID map and label) of `GameplayOptionDefaults`.
>
> Update (P5 exit verifier, 2026-10-12): gap (a) is closed. `tessera-settings` `MapPrefs` keeps
> the six values in the `[Map]` section of Tessera's settings file (unset keys take the
> install's defaults); the client lays them over the defaults and writes a filter or
> presentation change back off the frame. Gap (b) stays open.
>
> **Answered (spec writer, 2026-10-23; revision 1; MW32):** gap (b): `GameplayOptionDefaults`
> is found as the label of the root DID map's `UNIQUEDB` map (root key 0xF), entry key 0x68,
> as `chat-window.md` and `fonts.md` already read it; look it up there by label at run time.
> The `WEENIECONTENT` attempt finds nothing in this install and may be dropped. When no set is
> found, the implementer's fallback (every bit of the read mask, both presentation switches
> on) is the Tessera rule. No implemented behaviour changes on this install.

---

<a id="s7"></a>
## 7. Place lookup

<a id="s7-1"></a>
### 7.1 What the original offers (OBS, BIN; MW8, MW19, MW25)

The original has **no place search**: the map layout has no text entry (MW25), the map window
handles no typed text, and no command of the install's `Commands` table looks up a place by name.
A player finds a place by:

1. **Browsing**: child-map buttons (§4.3), going up (§4.4) and the map menu by region and map
   name (§4.5).
2. **The player's map**: §4.6, and the minimap's `MapZoom` button (GP §8.7.5).
3. **Coordinates**: `CoordinateDisplay` shows the map coordinates under the pointer (§3.4);
   each note's tooltip shows its coordinates (§6.3); `/loc` prints the player's position
   (CO §9).

So the original matches no user text against the places table (whose keys are internal
identifiers, MD §14.2), area names or dungeon names.

<a id="s7-2"></a>
### 7.2 Names a lookup can use (data; OBS through the companion specs)

| Source | Text | Gives |
|---|---|---|
| Map nodes | rendered `UI_Map_MenuName` (227) | a node |
| Areas, territories, regions | rendered `Area_Name` (WS §11.2) | the area's node (MD43), its landblocks (`walker.md` §6 index) |
| Dungeons | rendered `Dungeon_Name` (WS §11.4) | the dungeon map, the dungeon's position when known |
| Places table | keys (9,025, MD §3) | a position and zone |
| Coordinates | map coordinates text (CO §8) or a pasted `/loc` line (CO §9.8) | a position |

> **Answered (spec writer, 2026-10-23; revision 1; MW33):** the 22 nodes are leaves that list
> areas (regions 2, 3 and 5). **Tessera rule** (confirmed): they are kept as nodes (shown by
> navigation, the player's map and place lookup through their areas), get no map-menu item
> (an item without a label could not be told apart), have no name in the name index and an
> empty `map-region` resolved name; a selector reaches them by element ID. What the original's
> menu does with them was not traced. No implemented behaviour changes. The implementer's text
> is kept as the record.
>
> (Was **OPEN:** implementer, 2026-10-11, `tessera-gamedata` `map::PlaceIndex`.) On this
> install 22 of the 227 nodes store a null `UI_Map_MenuName` (string table 0, token 0; for example the
> nodes of active elements 0x10003C53, 0x10003CCF, 0x10003D02), so 205 node names render, not
> 227; their map-menu items (§4.5) and their `map-region` resolved names (`scenario-feed.md`
> §8.3) are empty. Which label the original shows for such a node (an empty item, or none) is
> not stated. The implementation keeps the nodes (§4.1: present, not missing) and leaves them
> out of the name index.

<a id="s7-3"></a>
### 7.3 Tessera's place lookup (Tessera rule, decision 0024 §5 P5)

**Command.** A client built-in command `findplace TEXT` of the one dispatcher (decision 0020,
CW §10 item 3), registered under that Tessera name only (no install name is taken; `loc` and
`location` stay unregistered). Without TEXT it removes the lookup marker.

**Matching**, in this order; the first step with a result wins:

1. TEXT parses as map coordinates (CO §8.3 inverse, region = the shown map's region, else the
   player's) or as a `/loc` line (CO §9.8): one result, that position.
2. TEXT equals a place key ignoring case (MD §3.4): one result.
3. Names of §7.2 (nodes, areas, territories, regions, dungeons), normalised and ranked as the
   walker's `--place` index (`walker.md` §6: lower-cased, Latin-1 folded, alphanumeric words;
   exact, then prefix at a word boundary, then whole-word substring; ties: the shown map's
   region, the more specific kind (dungeon, area, territory, region, node), then the larger
   place, then the lower DID or element ID).

**Result.** At most 10 results are listed as one chat line each (Tessera's own information line
type and wording: kind, name, map coordinates when a position exists). The first result is then
shown:

- a node: show it;
- an area, territory or region: show the node of the area (or of the territory's or region's
  first listed area in tree order), with the lookup marker at the place's centre (`walker.md`
  §6 target);
- a dungeon: show its dungeon map, with the marker at the dungeon's position when known;
- a position or place key: show the player's-map choice of §4.6 step 3 for that position (or
  the dungeon map of its dungeon), with the marker at the position.

The **lookup marker** is a plain feed marker (§6.7) with id outside the feed's range (Tessera
owns it), label = the result's name or the coordinates text, no icon; one exists at a time.
No result: one line "no place found" (Tessera wording), nothing else changes. Lookup reads the
same index as `--place` and never writes to the install.

> **Answered (spec writer, 2026-10-23; revision 1):** (a) a **per-record** target is meant:
> the lookup's names are per record (§7.2), so the shown node, the marker and the tie rule's
> "larger place" (that record's landblock count) all belong to the chosen record, as built;
> `walker.md` §6's merged places stay the `--place` rule. (b) Confirmed: without the game UI
> `findplace` posts its result lines only (one error line before the facts are loaded) and
> shows nothing. No implemented behaviour changes. The implementer's text is kept as the
> record.
>
> (Was **OPEN:** implementer, 2026-10-11, `apps/client` `map::data`, `map::lookup`.) Two
> readings.
> (a) `walker.md` §6 merges every area record of one name in a region into one place, but the
> names here are per record (several area records share a name and region on this install),
> so the shown node and a by-name marker can belong to different records. The client applies
> the §6 target rule (centre of the covered landblock nearest the centroid) to the landblocks
> of the chosen record itself (an area's own blocks; a territory's or region's the blocks of
> its areas), from the same `Area_DID` block maps `--place` reads, and fills the tie rule's
> "larger place" with that landblock count. Is a per-record target meant, or should the
> lookup show the node of the merged place? (b) Without the game UI (no `--ui`, or headless)
> no map window exists: `findplace` posts its result lines only (once the facts are loaded;
> without them one error line), and shows nothing.

---

<a id="s8"></a>
## 8. Tessera rules where the data and the client stop

| Topic | Client behaviour | Tessera rule |
|---|---|---|
| Discovery keys (MD §4.8.2) | asks game logic (server state) | Setting `map_discovery = all \| none`, default `all`: every key counts as discovered |
| World-event map-note states (MD §4.8.6) | evaluated against world properties | Evaluate against the world properties the session supplies; with none, no condition holds (those 663 rows are hidden) |
| Content layer *L* (MD §4.8.2) | the player's `Volume_ContentLayer` | The walker's content layer (`walker.md` §1 `--content-layers`, first value) |
| Filter storage | player options (server-saved) | Tessera settings (§6.9) |
| Presentation | `GOP_MapInFullscreenMode` | Tessera setting `map_fullscreen`, default from `GameplayOptionDefaults` (§3.1) |
| Stable-master button, quest guide | game tests, quest state | Hidden (§3.4, §6.5) |
| Fog of war | explored cells (server or client state) | Hidden (GP §8.5.3) |
| Player's dungeon | game-script pair (GP §8.5.1) | §4.6 Tessera rules |
| Run-time notes | server-registered | Fellows only (§6.4) |
| Feed markers | none | §6.7 |
| Place search | none | §7.3 |

<a id="s8-1"></a>
### 8.1 The fallback map window (Tessera rule, revision 1, 2026-10-23; MW35)

For runs without an install and for the committed tier-1 goldens of decision 0036 §2, the map
window is built from code-made elements that carry the element IDs of §3.2–§3.3 (from the
client's vocabulary; never read from an install in this case) at the rects and z-levels of the
§3.2 table:

1. **`MapWindow`** (0, 0, 1,024, 768), z −1,500, holding `MapField`, `MapDecoration`
   (−73, 0, 1,168, 768; z −1,600; Tessera's own or CC0 frame art, drawn with alpha) and
   `MapButtonsParent` (985, 2, 39, 16; z −3,000) with `ToggleMapFullscreenCollapse` and
   `Toggle_Map_Panel` at (0, 0) and (20, 0), 16 × 16.
2. **`MapField`** holds `MapPadding` (z −1,400, the background art), `MapBackground` and
   `MapMenuScaleParent` (52, 18, 800, 25) with `FilterMenu` (0, 0, 150, 25) and `MapMenu`
   (151, 0, 150, 25), labelled in Tessera's own short words, and `StablemasterButton`
   hidden.
3. **`MapBackground`** holds `MapQuestObjectiveIndicatorsParent` (hidden), `CoordinateDisplay`
   (52, 730, 450, 25; z −10) and `QuestGuideOptionField` (hidden). It has **no object mode**
   (it is drawn in `MapWindow`'s canvas at the design size), unlike the install's (mode 1),
   so that a golden can capture it.
4. **`MapPanel`** (the windowed root) is a frame of 1,038 × 802 with a title bar (its title in
   Tessera's words), `ToggleMapFullscreenExpand` and `Toggle_Map_Panel`, and
   `MapPanelPadding` at (5, 26); no resize bars.
5. **Nodes**: a node's active element is a plain field of the node's pixel frame without
   child-map buttons; `DungeonMap` (z −500) holds `DungeonMap_ImageField` (112, 84, 800, 600,
   Stretch) and `DungeonMap_OverlayField` (0, 0, 1,024, 768) with Tessera's frame art;
   `MapPlayerMarker` is 28 × 28, z −500, Stretch, with Tessera's marker art. Notes use
   `MapNoteTemplate`'s size (30 × 30).
6. **Data**: without an install there is no map tree, no georeference and no icon table; the
   window shows its frame, the empty map background and the controls. A tree, its images, a
   georeference and options given by a caller (the tier-1 golden's synthetic one-node tree)
   are test data, not part of the fallback.

The controllers of §4–§6 attach to this tree exactly as to the install's. The committed tier-1
golden `p5_map_window` captures this window with a synthetic tree and is normative from this
revision on (no longer provisional).

> **Answered (spec writer, 2026-10-23; revision 1):** the rule is §8.1; it adopts the
> provisional tree of the implementer note below without change, including the
> `MapBackground` object-mode deviation. No implemented behaviour changes. The texts below are
> kept as the record.
>
> (Was **OPEN:** implementer, 2026-10-11, P5 planning.) No fallback map window is defined for
> runs without an install. Decision 0036 §2 asks for a committed tier-1 golden of "Tessera's map
> window with a generated map image, notes and feed markers … and no install", and the travel
> and instance windows each have a Tessera fallback window, but this spec gives the map window
> none: no code-made element tree (`MapWindow`, the map field, the map background with the
> element IDs of §3), no synthetic map tree and no literal georeference. A Tessera rule is
> needed: which elements a code-made map window holds and with which IDs, a one-node synthetic
> tree (active element, image reference, pixel frame, offsets and scale given literally), and
> which controls of §3.4 it shows. Until then the tier-1 world-map golden cannot be made.
>
> **Implementer note (2026-10-11, `tessera-ui` `game::mapfallback`):** pending that rule,
> `tessera-ui` builds a provisional code-made map window from §3.2's table: `MapWindow` with
> `MapField` (`MapPadding`, `MapBackground` with `MapQuestObjectiveIndicatorsParent` hidden,
> `CoordinateDisplay` and `QuestGuideOptionField` hidden, `MapMenuScaleParent` with
> `FilterMenu`, `MapMenu` and `StablemasterButton` hidden), `MapDecoration` and
> `MapButtonsParent` at their rects and z-levels; `MapPanel` reduced to its frame, title,
> `ToggleMapFullscreenExpand`, `Toggle_Map_Panel` and `MapPanelPadding` at (5, 26); a node's
> active element as a plain field of its pixel frame (no child-map buttons); `DungeonMap` and
> `MapPlayerMarker` at §3.3's rects. One deviation: the fallback `MapBackground` has no object
> mode (it is drawn in `MapWindow`'s canvas at the design size), so the committed tier-1 golden
> `p5_map_window` can capture it. The synthetic tree is test data (literal georeference and
> options), not part of the fallback. The golden stands until the rule asked for above exists.

---

<a id="s9"></a>
## 9. Invariants

Checked on this install unless marked; readers report violations as warnings.

| ID | Invariant |
|---|---|
| MWI1 | Every node's active element is a top-level element of `map` and is the active element of exactly one node. |
| MWI2 | Every node image and every dungeon-map image names a decodable surface record. |
| MWI3 | Every `Dungeon_MapData` names a set holding `UI_Map_GameMap`. |
| MWI4 | Every node with `UI_Map_HidePlayerMapnote` also has `UI_Map_HideFilterButtons` and `UI_Map_QuestGuideDisabled` (the same 7 nodes). |
| MWI5 | Both map filter defaults ⊆ 0x9B7C7FFFFEFF0002; the common mask ⊆ the near default. |
| MWI6 | Every key of the `MapNoteType` chain has a display text (49 of 49), so every listed filter bit has a label. |
| MWI7 | Every region of `Region_Map_List` is the region of at least one node, and every node region is in the list. |
| MWI8 | (Behavioural) The shown map never changes on window show or hide alone; a node is shown only by §4.2. |
| MWI9 | (Behavioural) A feed marker is drawn on a map only when its location maps into that map's frame (§6.1, §6.7). |
| MWI10 | (Behavioural) The census of §5.4 visits each of the 1,525 regions exactly once. |

---

<a id="s10"></a>
## 10. Worked examples (synthetic)

All DIDs, IDs and values are invented.

<a id="s10-1"></a>
### 10.1 Showing a node and its notes

Node *N*: region 1, block (100, 100), pixel offsets (0, 768), scale 0.5, pixel frame 1,024 ×
768, display mask 0x0000_0400_0000_0000 (bit 42) with `UI_Map_IncludeCommonMapNoteTypes`
false, filter option the near map filter with value 0x0000_0400_0000_0000. Notes in *N*'s area:
row A type bit 42 at global (16,100, 16,050); row B type bit 37 at (16,200, 16,100).

- A: created (bit 42 in the mask), visible (bit 42 in the filter); pixel *px* = 0 + trunc((16,100
  − 16,000) × 0.5) = 50, *py* = 768 − trunc((16,050 − 16,000) × 0.5) = 743. Its 30 × 30 note is
  centred there.
- B: not created (bit 37 not in the mask). `FilterMenu` lists *all*, *none* and one item (bit
  42), checked.

<a id="s10-2"></a>
### 10.2 Another region

Same *N*. A feed marker with location region 2 at (16,100, 16,050): §6.1 replaces the block
difference by −2,000 on each axis, and the offsets inside the block still add: *dx* = −2,000 +
100 = −1,900, *px* = trunc(−1,900 × 0.5) = −950: outside the frame, not drawn. On a dungeon map
(no region) the same marker takes region 2 as the origin's region and is drawn if it maps
inside.

<a id="s10-3"></a>
### 10.3 Dungeon map stretch

Map data: image 1,600 × 2,400, block (10, 20), pixel offsets (0, 2,400), scale 2. The image
field gets a 1,600 × 2,400 canvas, then is stretched to 800 × 600: horizontal factor 0.5,
vertical 0.25. The player at global (1,700, 3,300) is at map pixel (200, 2,400 − 200) = (200,
2,200), on screen at (112 + 100, 84 + 550) = (212, 634) of `DungeonMap`.

<a id="s10-4"></a>
### 10.4 Going up

Shown: node *C* (parent *P*). Right click → *P*. Right click on the root → nothing (§4.4
Tessera rule). Dungeon map shown, player in that dungeon, right click → the node of the
player's area (or the root).

<a id="s10-5"></a>
### 10.5 Filter menu toggle

Near filter 0x0000_0400_0000_0000; node mask bits {37, 42}. Items: *all*, *none*, then the
two type items sorted by label. Toggling bit 37 gives 0x0000_0420_0000_0000, stored in
Tessera's settings; notes of bit 37 become visible. *none* gives 0.

<a id="s10-6"></a>
### 10.6 Place lookup

`findplace 12.3S, 45.6W` with region 1 shown: step 1 parses it; the position's area *A* is
listed by node *M*; *M* is shown with the lookup marker at the position; one chat line. Then
`findplace` alone removes the marker.

---

<a id="s11"></a>
## 11. Corrections to, and agreements with, other documents

Proposed here; the Accepted texts are not edited except for the additive notes listed in the
change log.

| Document | Statement | This spec |
|---|---|---|
| GP §9.2 Tessera rule and OPEN (draw mode) | Node image with the element's own draw mode or 2 | **Correction (BIN, MW4):** the client always sets the node image with draw mode 2 |
| GP §9.2 OPEN and Tessera rule (child-map buttons) | Click shows the named node (Probable); 3 non-node targets do nothing | **Answered (BIN, MW5):** confirmed for left and double-left clicks on any element with `UI_Map_Child_Map`; the 3 targets are shown without node data (§4.2 step 3); Tessera rule changed accordingly (§4.3) |
| GP §9.4 Tessera rule (map choice) | Area's node, else the node with the player's region at the smallest depth, else the root | **Correction (BIN, MW9, MD43):** the original shows `WorldMap` for an unlisted area; there is no region or pixel-bounds fallback. G28 (Reported) becomes BIN |
| GP §9.4 OPEN (dungeon map in the window) and its implementation OPEN | Dungeon map on an own element at (0, 0), unscaled | **Answered (BIN, MW10):** `DungeonMap`, image stretched into the 800 × 600 field at (112, 84), frame overlay (§4.7) |
| GP §9.5 Tessera rule (attachment) | `MapWindow` as a child of `MapPanelPadding` | **Correction (BIN, MW2):** `MapField` (not `MapWindow`) is moved under `MapPanelPadding` in the windowed presentation; full screen shows `MapWindow` itself; the default is full screen. Active elements are children of `MapBackground` (agreement) |
| GP §9.5 OPEN (marker) and implementation OPEN | Marker art, z-order; marker on an own element as `MapBackground`'s last child | **Answered (BIN, MW17):** an instance of `MapPlayerMarker` (its own art, not the minimap's) attached to the shown map element, z −500, hidden by `UI_Map_HidePlayerMapnote` and at the frame edge (§6.6) |
| GP §8.8 OPEN (notes and blips) | Not traced | **Answered** (§6.8): template, radius in pixels, on/off-screen images, tooltip, filter option and commands |
| GP §8.7.5 OPEN (`MapZoom`) | Probable: opens the world map | Supported (Probable, MW28): the window's open request selects the player's map |
| MD §4.8.4 step 2 | "kept only when its `type` shares a bit with the window's note-type filter" | **Refinement (BIN, MW11, MW12):** two masks apply: creation by the node's display mask (all on dungeon maps), visibility by the filter option of the node |
| MD §4.8.6 OPEN (a) | Initial filters and controls | **Answered** (§6.9, MW13, MW14) |
| QJ §8.3 implementer OPEN (1) | Unit of `UI_Radar_RadarRadius` | **Answered (BIN, MW22):** map pixels; world radius = *R* ÷ scale; `tessera-ui`'s reading is correct |
| QJ §8.3 implementer OPEN (2) | Markers above the map, below the player marker | Agreement on the world map (z −1 vs −500, §6.2) and on the radar (Tessera rule, §6.8) |
| QJ §8.3 implementer OPEN (3) | Own icon kept on mouse-over | Confirmed as a Tessera rule (§6.7 item 2) |
| QJ §8.3 implementer OPEN (region, `apps/client`) | Markers of another region than the **player's** hidden | **Correction (BIN, MW18):** the original compares with the **shown map's** region; on maps without one (the root, dungeon maps) any region is placed; on the radar the radar's origin region (the player's) applies |
| `walker.md` §6 `--place` | Name index and ranking | Agreement: §7.3 reuses it |
| CW §10 item 3 | Only listed install names are registered | Agreement: `findplace` is a Tessera name; `loc` stays unregistered (a `/loc` built-in would need its own revision of CW) |

---

<a id="s12"></a>
## 12. Open questions (summary)

1. §3.1: screen placement and scale of `MapWindow` in full-screen presentation (Tessera rule
   given).
2. §4.4: what the extra call on `UI_Map_PreventZoomOut` and at the top of the tree does (O141).
3. §4.5: submenu order and label collation of the map menu (Tessera rule given).
4. §6.3: whether a note's z adds `UI_Map_MapNoteRelativeZLevel` (Probable).
5. §6.4: the source of run-time notes (server and game state).
6. §6.5: quest-guide shading geometry (QJ §14 Q7).
7. §6.8: the radar's filter menu (which element sends the commands); the off-screen icon's
   rotation (Probable).
8. §3.4: the `StablemasterButton` game test (MW21).
9. §6.9: collation of the filter-menu labels.
10. §8: answered 2026-10-23 (§8.1, the fallback map window).
11. §4.5, §6.9: the popup presentation of the map and filter menus (implementer OPEN,
    2026-10-11).
12. §6.8: answered 2026-10-23 (the images point east, MW31; rim-only surfaces accepted).
13. §6.3: the names of the note tooltip's two coordinate arguments (Tessera rule given,
    2026-10-23).

---

<a id="s13"></a>
## 13. Proposed owner observations

For `docs/checklists/owner-observations.md` (numbers from O140, reconciled at merge):

| Row | Observation in the original client |
|---|---|
| O140 | Open the map (M) on a new character: full screen or windowed? Screenshot of the controls row and of `FilterMenu` opened (item order, check marks). |
| O141 | On the winter-festival city map, right-click: does the map go up to its parent, re-centre on the player, or nothing? Same on the world map. |
| O142 | Open a dungeon map whose image is not 4 : 3 (any tall cave map): is it squeezed into the frame? |
| O143 | Radar: a vendor note just outside the radar's circle: drawn at the rim, turned towards it? Its tooltip shows a distance? |
| O144 | The map menu: submenu order and the order of map names in one submenu. |
| O145 | The three child-map buttons of §4.3 that lead to maps without notes: what appears? |
| O146 | Hover a note on a dungeon map under unexplored fog: tooltip or none? |

---

<a id="changelog"></a>
## Change log

| Date | Change | By |
|---|---|---|
| 2026-10-11 | First version (status Review) for decision 0024 P5: the map window's parts and presentations, navigation, dungeon-map presentation, the map-region census definition and counts, overlay order and look on the world map and the minimap, filter controls and defaults, feed-marker rules, place lookup. OBS and BIN (client 4906.0071.1794.4035, questions q1–q9). Additive notes pointing here added to `game-panels.md` §8.8, §9.2, §9.4, §9.5 and `map-data.md` §4.8.6, §15 item 1. | Spec writer (dirty room) |
| 2026-10-13 | Independent review against CLEAN_ROOM.md §5.3: **passed**; status Review → Accepted. No fact changed (review-only commit). Record in the header; spot checks were the reviewer's own read-only M1 probes of the install and a BIN byte scan of the private client copy (decision 0011). Two non-blocking notes recorded in the header (§6.2 cross-parent z; §8 fallback-window OPEN still blocks decision 0036 §2's tier-1 world-map golden). | Spec reviewer (dirty room) |
| 2026-10-23 | Revision 1 (spec writer, dirty room; own M1 probes of the install: string-table argument IDs and name hashes, the 36 off-screen images, the DID maps, the map tree): §4.6 the client half's dungeon input (cell `Dungeon_DID`), §6.3 argument order (MW30, data for `CoordinateDisplay`, Tessera rule for the note tooltip), §6.8 (a) and (b) (MW31), §6.9 (b) `GameplayOptionDefaults` by `UNIQUEDB` label (MW32), §7.2 null menu names (MW33), §7.3 per-record targets, new §8.1 fallback map window (MW35; the tier-1 golden `p5_map_window` is normative), §12 items 10, 12, 13; rows MW30–MW35. Status Accepted → Revised, awaiting independent review. **Implemented behaviour that changes:** §4.6 (indoors, the player's dungeon map); §6.3 may render the install's entries. | Spec writer (dirty room) |
| 2026-10-23 | Independent review (spec-review:spec-gameplay:2 (independent session)) of the 2026-10-23 revision: **passed**, CLEAN_ROOM.md §5.3 every item; status Revised → Accepted. Header status and review record only; no other text changed; no implemented behaviour changes beyond those the revision row states. | Spec reviewer (dirty room) |
| 2026-10-23 | Round-3 revision (spec writer, dirty room; status Accepted → Revised, awaiting independent review; no new decompiler session: the saved q5 listing of 2026-10-11 re-read). Section numbers unchanged. **MW15:** completed: with none of the stable-master bits, or when the entity lookup fails, the first argument is the note's own text. **§4.6:** two implementer OPENs answered (source 1 needs a `scenario-feed.md` form, filed there as a non-blocking OPEN, source 2 suffices; an unresolvable dungeon counts as none, Tessera rule). **§6.3:** two implementer OPENs answered (the plain text on a failed lookup is the original's; coordinate texts render through the `Position` template of CO §8.3, the English form only as fallback). No implemented behaviour changes except rendering the coordinate template for non-English installs. |
| 2026-10-23 | Independent review (spec-review:spec-ui-r3:1 (independent session)) of the round-3 revision: **passed**, CLEAN_ROOM.md §5.3 every item; status Revised → Accepted. Header status and review record only; no other text changed. | Spec reviewer (dirty room) |
| 2026-10-23 | Round-4 revision (spec writer, dirty room; status Accepted → Revised, awaiting independent review; BIN question (q10) in a new private decompiler copy plus the saved listing). Section numbers unchanged. New row MW36. **§4.5:** opening and placement of `MapMenu`'s context menu (fixed position three units below the button), `ContextMenuElement_MapMenu` on every menu of the tree, submenus only for regions with nodes, the 0.2 s reopen guard after a close by deactivation; the 2026-10-11 OPEN marked superseded and the 2026-10-23 implementer OPEN answered. **§6.9:** `FilterMenu` is an engine Menu with a popup of layout 0x22000080. **Implemented behaviour changes:** the map menu's position, the guard's length, regions without nodes get no item, and `FilterMenu` uses the Menu popup instead of a kept-open context menu. | Spec writer (dirty room) |
| 2026-10-23 | Independent review (spec-review:spec-ui-r4:2 (independent session)) of the round-4 revision: **changes requested**; status stays Revised. Blocking: (1) §6.9: the sentence "When the value changes, the window stores it in the node's option and updates every note's visibility", part of the "Selecting" rule, now ends the new round-4 OPEN callout (a lazy continuation of the quote), so a rule stands inside an OPEN; put it back in the "Selecting" bullet, after the callout's own text is closed. (2) MW8 still says the map menu's *items* are `ContextMenuElement_MapMenu` instances, which §4.5 corrects to "the menus (root and submenus)"; bring MW8 in line with §4.5 and MW36. Spot checks passed (own read-only probes): `FilterMenu` based on `MapNoteFilterMenu`, itself based on `Menu_02` of 0x22000080, with `auto_close_on_selection` false, `text_item` `ContextMenuBool` of 0x2200007B and `selection_display` 0; `MapMenu` based on `AvailableMapMenu`, based on a Button with `boolean_button`; `Menu_02_PopupField` 150 × 200 with `max_height` 200 and its list at (5, 5), 130 × 190, with a vertical scroll bar. BIN (q10) was not re-traced. Non-blocking: §3.3 says Tessera never creates `MapNoteFilterMenu`, while §6.9 now builds `FilterMenu` on it as a layout base; a later revision may say that the base is resolved by UL §7, not created. Header status unchanged; no other text changed. | Spec reviewer (dirty room) |
| 2026-10-23 | Round-4 fixes after the second review (spec writer, dirty room; status stays Revised, awaiting independent review). Section numbers unchanged; no new facts. **§6.9:** the sentence on storing the value and updating note visibility is back in the "Selecting" bullet, and the round-4 OPEN callout holds only its own text. **MW8:** the menus (root and submenus), not the items, use `ContextMenuElement_MapMenu`, as §4.5 and MW36 say. **§3.3** (non-blocking note): `MapNoteFilterMenu` is resolved as `FilterMenu`'s layout base, not created as an element of its own. No implemented behaviour changes. | Spec writer (dirty room) |
| 2026-10-23 | Independent review (spec-review:spec-ui-r4:3 (independent session)) of the round-4 revision: **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; status Revised → Accepted. Header status and review record only; no other text changed. | Spec reviewer (dirty room) |
