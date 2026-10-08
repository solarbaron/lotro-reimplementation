<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Game-code choices of the native panels: minimap and world-map images, quickslot slots, hidden-by-code elements, drag-box frames

| | |
|---|---|
| **Status** | Accepted (2026-10-06: accepted after revision for decisions 0014/0015) |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | **Review of Revision 3 and its fixes (decisions 0014 and 0015): Claude review session (spec-writer agent), 2026-10-06: passed; accepted after revision for decisions 0014/0015; status moved to Accepted.** Every `CLEAN_ROOM.md` §5.3 item holds for the changed material (header, DEC tag, G39–G41, §8.2 step 3, §8.6.3, §8.7.1, §8.7.2 and "How P1 splits into surfaces", §13.7, §14, §15 items 7, 8 and 24, §16, GP15–GP17). It states functional facts and Tessera rules only, with no code, listing structure or addresses. No prose is copied. `EscapeKey`, `menu` and the element and property labels are interoperability identifiers covered by the naming note. There is no game text, key, hostname or traffic. Every new fact has a row with a mark, the Tessera rules cite DEC with the decision and its section, the invariants are testable, the new unknown (§16) has an in-section `> **OPEN:**` callout listed in §15 item 24, and no closed gate is touched (§16 is local client behaviour, not the network). Agreement with the decisions. Decision 0014: §16 follows D2 (Escape gives `menu`, which carries `EscapeKey`) and Consequences "Escape with a UI attached" (a declined Escape never quits, closes the active panel or opens the game menu; quit is kept only without a UI; the help-line note follows Cost). The cited UB §5.1, §5.3–§5.5 and `chat-window.md` §6.2 say what §16 and §13.7 use, and §13.7 steps 1–5 follow the two rules. Decision 0015: §8.6.3 follows §2.3 and Decision 1.7 (separate entry, screen result fixed, surface filter, 40 × 40 square, own opacity), and §8.7.1 follows Decision 1.6 and the round-minimap test. The §8.7.2 surface split applies the owner rule of Decision 1.1 and the coverage and clip rules of Decision 1.7 to the data. It contradicts the decision only where the decision misstates P1 (the marker as the only nested entry, P1 checks 2 and 4). The resulting behaviour (clip by the root only, a fixed marker region, no damage while turning) is the decision's, and §14 records the correction for the decision's next revision. Spot checks: independent read-only probes on the install, 2026-10-06. All 302 layouts were re-read from a fresh directory index and decode to exactly the earlier data, and every check matches. (1) G39: `GamePlay_Radar`, `MinimapPlayerMarker` and `Minimap_Frame` carry object mode 1 in P1 (0x22000012). (2) G40: `MinimapViewport` has no `DrawAfterChildren`. Its children in drawing order are the two map elements (z 2), the fog (z 1), `RadarQuestObjective` and the marker (z 0, mode 1), then `Minimap_Frame` and seven more at z −1, all in mode 1. The map elements and their descendants have no mode. `RadarMapImage_Template` is 200 × 200 with no mode and no children. The radar's children before the viewport lie at rows ≥ 194, and its child after it (`StablemastersButton`) has mode 1. There are 18 non-root elements with a non-zero mode, as listed. (3) G41: the root is 224 × 280 and `GamePlay_Radar` is at (0, 33, 224, 260). Every radar child and every viewport descendant except the map elements and fog ends by radar row 240 (≤ 247). The rim spans root columns −7 to 230 and rows 0 to 237. (4) G1/GP1: 71 of 71 `HiddenDragBox` instances have the three children and the four state properties. (5) G20/GP4: 0x41007F4B is 170 × 170, format 21, RGB 0 everywhere, alpha 0 at the corners and 255 at the centre, drawn with draw mode 2. (6) G25/G27/GP3: one `MapWindow` (1024 × 768), 227 nodes, depth histogram 1/6/72/108/35/5, 227 distinct active elements with root `WorldMap`, regions as stated, scale 0.0173–3.2875, 17 nodes with a negative pixel offset, and 541 area DIDs, each in exactly one node (214 nodes list areas). Non-blocking notes for a later revision: (a) G41 and the §8.7.2 Clip bullet say nothing the radar draws lies below root row 280. Literally this covers radar children and viewport descendants. The four border pieces of the area-text `TutorialHighlighter` (grandchildren of the radar) reach radar row 264 (root row 297) in layout values. They are drawn only inside that hidden, surface-owning highlighter, so the result is unchanged, but the sentence could say so. (b) Until decision 0015 is revised, its Decision 1.7 "only named nested entry" sentence, the last sentence of its §2.3 marker bullet and its P1 checks 2 and 4 conflict with §8.7.2 here. The maintainer should amend the record so that test authors see one rule. (c) Carried over: row G38 sits before G37, and the change-log rows are not in date order. (d) §16 leaves out how the client is quit while a UI is attached. M5 does not depend on it. Earlier records follow. Third review (re-review of Revision 2): Claude review session (spec-writer agent), review 2026-10-06: **passed**, status moved to Accepted. The second review's blocking item is fixed. An independent census of all 302 layouts finds 46 `HideWhenField`s, 19 with a comparison: 14 compare an int (PS type 15), 2 an enum (type 8, operators 3 and 2, both in layout 0x22000849) and 3 a boolean (type 21, operator 3 once and 2 twice). Six distinct properties are compared; five carry dictionary flag 0x800 with a default of 0 or false, and one has no default. G7, GP13 and the §6.2 census are exact. BIN cross-check of G38 under decision 0011 (q4): the dispatcher maps operators 1–6 to six routines and gives false for 0 and anything above 6. The ≠ and = routines compare the stored 32-bit values for codes 5, 8, 10, 15, 18, 20 and 21, use a float compare for code 9 (NaN makes = false and ≠ true), and pass every other code to the type's own routine. The four ordering routines compare codes 8 and 15 as signed 32-bit and code 9 as float (false on NaN), pass codes 1–4, 6, 7, 11–14, 16, 17, 19 and 22 to the type's own routine, and give false for the rest. The data value is the left operand. When the player lacks *P*, the hide test resets *B* to the property's default. §6.2, GP14 and §13.6 agree with all of this. The §15 items 15 and 20–23 and the new in-section OPEN callouts (§4.3, §8.7.2, §9.2, §10.1) are present and carry Tessera rules. Spot checks with independent read-only probes on the install, 2026-10-06, all matching: (1) G1/GP1: 71 of 71 `HiddenDragBox` have the three children and the four state properties; 42 `TutorialHighlighter`, 23 `QuickslotBar`. (2) G7/GP13: as above. (3) G20/GP4: 0x41007F4B is 170 × 170, format 21, RGB 0 everywhere, alpha 0 at the four corners and 255 at the centre. (4) G25/G27/G36: one `MapWindow` (1024 × 768), 227 nodes with depth histogram 1/6/72/108/35/5, 227 distinct active elements, the regions as stated, 541 area DIDs each in exactly one node (214 nodes list areas); 773 child-map buttons, 770 to another node, none to their own node, and 3 to top-level elements of the `map` layout that are no node. (5) G33 census: the game types of P1, P3, P4, P5, P8, P9, P11, P13 and P14 match §3 and §11 (for example P3 `PipDisplay` 6 and `TutorialHighlighter` 23, P5 two `BigBattlesQuestMedalIcon`, P8 one `RaidLock` and one `ChestLock`, P13 `PedigreeDisplay`). Every §5.3 item passes. Non-blocking notes for a later revision: a compared property with no dictionary definition (type 0) is not covered by the §6.2 table, but no data field has one. In the client, ordering gives false for it and equality uses the value's own routine. The §6.2 tri-state row says "32-bit", but PS §4.3 stores a tri-state as one byte; only equality matters there. Row G38 sits before G37, and the Revision 2 change-log row sits above Revision 1. Revision 2 (spec writer, 2026-10-06): the second review's blocking item is fixed. G7 and the §6.2 census now read 14 int, 2 enum, 3 boolean (own recount over all layouts and the property dictionary, 2026-10-06). The comparison routines were re-read under decision 0011 (new row G38), and §6.2 now states per value type how operators 2 and 3 and the four ordering operators compare, and what *B* is when the player lacks *P*. Enums and booleans are no longer covered by a Tessera rule. The §6.2 OPEN and §15 item 15 are narrowed to the types that use their own routine; no data field compares such a type. New invariants GP13 and GP14 and a §13.6 example. New in-section OPEN callouts for the four Probable statements (§4.3, §8.7.2, §9.2, §10.1; §15 items 20–23). Resubmitted for review. Second review (re-review of Revision 1): Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (one blocking item). Every item of the first review is fixed. The in-section OPEN callouts in §6.2, §8.5.3, §8.6.1, §8.7.3, §8.7.4, §8.7.5 and §10.2 are present, precise, carry Tessera rules where needed and are listed in §15 items 9 and 15–19. The §9.2 count now reads 196/199, and an independent recount gives 199 and 196. §10.3 now cites LPA §15 and A.1, and LPA §15's table lists the shortcut-type mapper. GP10 is restricted. G34–G37 give the Probable statements their own rows (§4.3, §8.7.2, §9.2 and §10.1 now cite them). **Blocking (§5.3 "each fact ... Confirmed"; G7 as extended in Revision 1, §6.2):** "16 compare an int property and 3 a boolean" is inaccurate as worded. An independent census of the 19 `HideWhenField`s with a comparison looked up the property-dictionary type of each compared property: 14 compare an int (PS type 15), 2 compare an **enum** (PS type 8, whose data names the character-class mapper; the two `StatElement_…` fields of layout 0x22000849, operators 3 and 2) and 3 compare a boolean (type 21; operator 3 once and 2 twice, as stated). As written, §6.2 says only that ints compare as signed 32-bit, and the new Tessera rule says "any other type gives false (never hides)". Read together, they make the two enum comparisons never hide. That is probably not the original's behaviour, and the census hides the case. Fix: reword G7 and the §6.2 census to "14 int, 2 enum, 3 boolean"; extend the §6.2 OPEN and its Tessera rule to enums (for example, compare enum keys as 32-bit integers) or state explicitly that enums never hide; and update §15 item 15. The enum fields are outside P1–P15, so M5 panel behaviour is unaffected. Even so, the Confirmed row must be exact. Spot checks: independent read-only re-runs on the install, 2026-10-06, all match except the G7 split above. (1) G1/GP1: 71 of 71 `HiddenDragBox`; 46 `HideWhenField`, 42 `TutorialHighlighter`, 23 `QuickslotBar`. (2) G7: 19 of 46 have a comparison, with operators and values as stated. (3) G20/GP4/§8.1: 0x41007F4B is 170 × 170, format 21, black, corners 0, centre 255, draw mode 2; all §8.1 rects; template 200 × 200. (4) G25/GP3: 227 nodes, 227 distinct active elements (all 1024 × 768 Fields, 226 with an Image item), root `WorldMap`, regions and scale range as stated, 17 negative pixel offsets; 773 buttons, 770 to nodes, 199 nodes with a button to another node, 196 with a non-child target. (5) G27: 541 area DIDs, each in exactly one node. `UI_Map_FogOfWar` is type 21. Every other §5.3 item still passes. Status kept at Review. First review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (two blocking items, three minor). The spec is close to implementable for M5; the §8.4.1 range rule, the §8.6.1 transform and every §13 worked example recompute correctly by hand. Blocking 1 (§5.3 "unknowns are marked `> **OPEN:**`"; same rule as the other M5 reviews, every unknown needs an in-section callout listed in §15): §8.7.4 (the coordinate-label rule; G23 says "rule OPEN") and §8.7.5 (the `MapZoom` event; G24 says "event meaning OPEN") are listed in §15 item 9 but have no callout in their sections. Other unknowns are stated inline with no callout and no §15 entry: §8.7.3, the two unidentified internal flags that force `Unknown_Time`; §10.2, the unidentified per-bar counter that keeps empty slots shown; §6.2, comparison of value types other than int and float ("not traced"); §8.5.3, the condition that enables `MapFogOfWar` (Probable: `UI_Map_FogOfWar` true; the existing callout covers only drawing); §8.6.1, what the radar does when no tile item has a usable `UI_Map_GameMap` (for example the empty range at *X* = 254 of §8.4.1). Add a callout for each and list it in §15. Blocking 2 (§5.3 "each fact ... Confirmed"; G25, §9.2): "196 of 227 nodes have buttons to other nodes" is inaccurate as written. An independent count over the 227 active elements finds 199 nodes with at least one `UI_Map_Child_Map` button whose target is another node's active element, and 196 nodes with a button whose target is not one of their tree children. Say "196 of 227 nodes have buttons to nodes that are not their tree children (199 to any other node)". Minor 1: §10.3 cites "KM §12 shortcut types", but `keymap.md` §12 does not define shortcut types (no "shortcut" occurs in it). Correct the reference or mark the shortcut-type enumeration OPEN. Minor 2: GP10 says "for any position"; restrict it to positions where §8.6.1 finds a georeference (not the §8.4.1 empty range). Minor 3: "Probable" statements without a provenance row of their own (§4.3 tooltip text, §8.7.2 the `Mood` gauge, §9.2 click navigation, §10.1 step 3 label text) should either cite a G row with mark Probable or carry an OPEN. Every other §5.3 item passes: only functional facts; the algorithms are in plain words with no code or listing structure (decision 0012 numeric constants only); no copied prose; identifiers are data labels used for interoperability, as the naming note says; no game text, keys or hostnames; BIN rows name questions q1–q6 under decision 0011 and exclude network, login and anti-tamper code; invariants GP1–GP12 are testable; no closed gate. Spot checks with independent read-only probes on the install, 2026-10-06, all matching: (1) G1/GP1: 71 of 71 instantiated `HiddenDragBox` elements over all 302 layouts have the three frame children and the four state properties naming `hidden`, `visible`, `dragging`, `visible_mouseover`; the `ToggleHiddenDragBoxes` label resolves to 0x1000007B in the InputActions mapper. (2) G7, §6.3: 46 `HideWhenField` instances; operators 6 (7), 2 (5), 5 (4), 3 (3); P4 `PlayerLevel_HWField` operator 2 and `PlayerScaledLevel_HWField` operator 3 with `LevelScaling_ScaledLevel` = 0, P5 `OpponentLevel_HWField` operator 2, P9 `Quest_Task_Filter` `HideWhenMonsterPlayer` true; also 42 `TutorialHighlighter`, 23 `QuickslotBar`. (3) G19/GP2: all 383,332 0x79 records scanned; 1,303 dungeons, each with a 0x78 `Dungeon_MapData` holding `UI_Map_GameMap` with an existing image; all JPEG (500), size histogram, scale counts (3.0 642, 1.5 219, 6.0 107, 5.0 93, 4.0 55, 2.0 39) and fog 670 true / 633 false as stated; `UI_Map_FogOfWar` is property type 21 (the LB §6.2 correction holds). (4) G20/GP4 and §8.1: 0x41007F4B is 170 × 170, format 21, every pixel RGB black, corners alpha 0, centre 255, drawn with draw mode 2; every §8.1 rectangle and `RadarMapImage_Template` 200 × 200 match. (5) G25/G27/GP3: 227 nodes, depth histogram 1/6/72/108/35/5, root active element `WorldMap`, 227 distinct active elements (all 1024 × 768 Fields, Image item matching in 226, the exception as stated), field-presence counts, regions, scale range, 17 nodes with negative pixel offsets (type 15, signed), 541 area DIDs each in exactly one node; 773 child-map buttons, 770 naming another node. BIN cross-check of G11: the range routine's clamps (0, 0xFE, the difference-2 test, 2 and 0xFC) agree with §8.4.1 and §13.2. Earlier records follow. Revision 1 (spec writer, 2026-10-06): every item of the first review addressed (in-section OPEN callouts in §6.2, §8.5.3, §8.6.1, §8.7.3, §8.7.4, §8.7.5 and §10.2, listed in §15; §9.2 count reworded after an own recount; §10.3 reference corrected; GP10 restricted; provenance rows G34–G37 for the Probable statements); resubmitted for review. First submission (spec writer, 2026-10-06): submitted for review. Answers the M5 completeness critique item A9 and the `ui-layout.md` §17.5 OPEN items (1) and (4) for the panels of `ui-layout.md` §17. |
| **Implements into** | `crates/ui` (`tessera-ui`: the fallback widgets of the game widget types named here), `apps/client` (`tessera-client`: the native panels of ROADMAP M5 exit criterion 2 and the data they show), `crates/world` (minimap and world-map image lookup, map pixel frames) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (**UL**: element trees, states §9.4, drawing §9.3 and §9.6, draw modes §9.6.1, the panel catalogue §17), [`ui-behaviour.md`](ui-behaviour.md) (**UB**: input routing, drag), [`landblock.md`](landblock.md) §6 (**LB**: landblock-properties records and minimap tiles), [`coordinates.md`](coordinates.md) §7, §10, §11.4 (**CO**: cell values, map pixel frame, compass heading), [`wstate.md`](wstate.md) §11 (**WS**: areas and dungeons), [`property-system.md`](property-system.md) (**PS**: property sets and value types), [`keymap.md`](keymap.md) §12 (**KM**: input actions), [`lua-plugin-api.md`](lua-plugin-api.md) Appendix A.1 and §15 (**LPA**: shortcut-type values and their data enum mapper), [`walker.md`](walker.md) §6 (the walker's own minimap) |

> **Naming note (interoperability).** Widget-type labels (`Radar`, `HiddenDragBox`, …), element-ID
> labels (`MinimapViewport`, …), state labels (`Mood_0`, …), UI-message labels
> (`Element_drag_global_begin`, …), the input-action label `ToggleHiddenDragBoxes` and property
> names (`UI_Map_…`, `UIComponent_…`, `UI_HideWhenField_…`, `HiddenDragBox_…`, …) are identifiers
> read from the user's own data (enum mappers, the property dictionary 0x34000000 and DID maps,
> UL §3.3, PS §2). They are used only to find records, elements, states and properties at run
> time. Numeric IDs printed next to them are this install's values (2026-10-06) and must be looked
> up by label at run time, not hard-coded. No game text is reproduced here.

---

<a id="s0"></a>
## 0. Purpose

`ui-layout.md` §17 lets an implementer build the native panels of the walker from layout records.
Some of what those panels show is not in the layout data but is chosen by the original client's
game code: which images the minimap and the world map draw and where, how many shortcut slots a
quickslot bar creates and when empty ones are visible, which elements code hides in normal play,
and which state the movable drag-box frames are in. This spec states those choices as functional
facts so that Tessera's fallback widgets (UL §9.2 note, UL §17.1 step 4) draw what the original
draws, and says what Tessera does where the original's rule depends on server data that the
sandbox does not have.

It covers the panels of UL §17.2 (P1–P15). The facts per panel:

| Panel (UL §17.2) | What this spec adds |
|---|---|
| P1 Minimap | §8 (images, placement, marker, rim, labels), §4 (drag-box frame), §5 (tutorial highlights) |
| P3 Toolbar | §10 (quickslot slots), §7 (pips, page changer), §5 (23 tutorial highlights) |
| P4 Player vitals | §6 (level fields), §4 (drag-box frame) |
| P5 Target vitals | §6, §4, §5 |
| P9 Quest log | §6 (task filter) |
| P10 World map | §9 (map tree, image choice, georeference) |
| P11 Quest tracker | §4 |
| P14 Extra shortcut bar | §10, §4 |
| All | §3 (summary of hidden-by-code rules), §11 (the remaining game types and Tessera's fallback), §16 (Escape with the game UI attached, Tessera rule) |

The chat window (P2) is out of scope (critique item A8). Engine widget classes (Meter, ListBox,
Tree, Scrollbar, tabs) are out of scope (critique item A2); where a panel here depends on one,
the dependency is named.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | Observed in the user's install (Steam, data current on 2026-10-06) with throwaway read-only probes in the dirty-room scratch area: all 302 UI layout records (prefix 0x22) decoded with full property values and every top-level element of every layout resolved by UL §7 (property replacement by PID); all 383,332 property-set records of prefix 0x79 scanned for dungeon records (`Dungeon_Name`) and every `Dungeon_MapData` target (prefix 0x78) decoded; the headers of every render surface named here; the alpha channels of the minimap viewport, rim and marker surfaces; the enum mappers `UIElementMessage`, `UIStateID`, `UIElementID`, `UIElementType` and `InputActions`. |
| BIN | The client executable of the user's install, `x64/lotroclient64.exe`, PE file version 4906.0071.1794.4035 (`lotro_live`, change list 711794, compiled 2026-09-27), read in the dirty room under the standing authorisation of [decision 0011](../decisions/0011-standing-m4-authorisation.md) (method M4) on 2026-10-06. Only the routines named in the questions and their direct callees were read, by address range. **Questions:** (q1) what the game widget types `Radar` and `MapFogOfWar` do when created and when the player moves: which children they look up, which records they load, which images they set and with which draw mode, where they place them; (q2) how the minimap rim and mood, time-of-day and coordinate elements are driven; (q3) what `HiddenDragBox` does at creation, on its toggle and on mouse messages, and how it passes states to children; (q4) whether `TutorialHighlighter`, `HideWhenField`, `PipDisplay` and `ToolbarQuickslotPageChanger` hide themselves, and the full `HideWhenField` rule including its comparison operators; (q5) how `QuickslotBar` builds its slots, how many, from which template, and when empty slots are visible; (q6) how the world-map window reads its map tree and how the dungeon map is chosen for the minimap and the world map. **Why M1–M3 were not enough:** the layout data name the elements, states and images but not which code shows or hides them, which records feed the map images or how they are placed; the original client cannot be observed in this session and no owner screenshots exist yet. The network protocol, login, keys, launcher and anti-tamper code were not read (decision 0004). |
| LC | LotroCompanion `lotro-tools` (map loaders and map finder), read for facts only: how a parchment-map tree is read from the `map` layout and that an area is found through `UI_Map_AreaDIDs_Array`. Marked **Reported** where only LC supports a fact. |
| UL, UB, LB, CO, WS, PS, KM, LPA | The companion specs listed in the header. |
| DEC | Accepted Tessera decisions [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge, **IB**) and [0015](../decisions/0015-ui-rendering.md) (UI rendering, **UR**). They fix **Tessera's** behaviour, not facts about the original; statements taken from them are marked "Tessera rule" with the decision and its section. |

Marks: **Confirmed** = checked over every record or element of the kind (OBS), or a BIN fact the
data agree with in every case; **BIN** = rests on the disassembly only; **Probable** = supported
by BIN or data but not proven in every case; **Reported** = third-party only.

| # | Fact | Sources | Mark |
|---|---|---|---|
| G1 | `HiddenDragBox`: children found by ID, the shown, dragging and mouse-over flags, the state choice from the four state properties, children hidden while not shown (§4.2) | BIN (q3); OBS (71 of 71 instantiated drag boxes have all three children and the four state properties naming `hidden`, `visible`, `dragging`, `visible_mouseover`) | Confirmed |
| G2 | The frame is reset (not shown) at creation and toggled by the input action labelled `ToggleHiddenDragBoxes` (§4.3) | BIN (q3: creation routine, game-event handler); OBS (the action ID resolves to that label in the `InputActions` mapper) | BIN |
| G3 | Mouse press, mouse release and mouse-over messages set the dragging and mouse-over flags (§4.4) | BIN (q3: message handler); OBS (`UIElementMessage` labels of the message keys) | BIN |
| G4 | A drag box passes each state it enters to its direct children whose `HiddenDragBoxChild_WantsHiddenDrabBoxStateChanges` is true (§4.5) | BIN (q3: the state-switch override) | BIN |
| G5 | `TutorialHighlighter` hides itself after creation, takes state `normal`, and is shown (state `highlight`) only when a tutorial-highlight event names its `UIComponent_TutorialHighlighter_Type` (§5) | BIN (q4: post-creation routine, activation handler); OBS (42 instances, 25 with a type) | BIN |
| G6 | `HideWhenField` visibility rule, its condition list, evaluation order and the six comparison operators (§6) | BIN (q4: the hide test, the comparison dispatcher and its six routines, the visibility override, the creation and property-change callers) | BIN |
| G7 | `HideWhenField` instances in the panels and their conditions (§6.3); value types and dictionary defaults of the compared properties (§6.2) | OBS (46 instances over all layouts; P4 2, P5 1, P9 1. 19 have a comparison. By the property-dictionary type of the compared property: 14 int (PS type 15), 2 enum (PS type 8, whose data names the character-class enum mapper; both in layout 0x22000849, operators 3 and 2), 3 boolean (PS type 21; operator 3 once, 2 twice), none of any other type. Six distinct properties are compared: five have a dictionary default, which is 0 or false, and one has none. The 2 enum and 3 boolean fields are outside P1–P15) | Confirmed |
| G8 | `PipDisplay` and `ToolbarQuickslotPageChanger` hide themselves in their constructors (§7) | BIN (q4) | BIN |
| G9 | Radar child roles, the viewport centre, the marker's centring, children hidden at creation (§8.1–§8.2) | BIN (q1: the radar's post-creation routine); OBS (the radar layout) | BIN |
| G10 | Indoor/outdoor test and which map elements are shown in each case (§8.3) | BIN (q1); CO §7 (cell values) | BIN |
| G11 | Outdoors: 3 × 3 landblock tiles around the player's landblock, the edge clamps, loading the landblock-properties records, rebuild only when region, block or content layer change (§8.4) | BIN (q1: the range routine, the request routine, the load callback) | BIN |
| G12 | Tile image choice by content layer, including the layer-1 fallback (§8.4.3) | BIN (q1); OBS (LB §6.2 census) | BIN |
| G13 | Tiles are list items created from `RadarMapImage_Template` (200 × 200), north row first, west column first; images set with draw mode 5 (§8.4.4) | BIN (q1); OBS (template size, ListBox properties 3 × 3, horizontal) | BIN |
| G14 | World point → map pixel: truncation toward zero of (offset × scale), y measured upward from the pixel offset (§8.6.1) | BIN (q1: the map transform); CO §10 (same formulas) | BIN |
| G15 | The map element is moved so that the player's pixel lies on the viewport centre; the marker stays centred (§8.6.2) | BIN (q1) | BIN |
| G16 | The marker image is rotated by the player's heading (§8.6.3) | BIN (q1: heading read, −90° offset and sign); OBS (the marker art points east) | Probable (screen direction) |
| G17 | Indoors: the map image and georeference come from the `UI_Map_GameMap` struct of a DBPROPERTIES record; the image element is resized to the image; fog of war is shown only indoors (§8.5) | BIN (q1, q6) | BIN |
| G18 | The indoor map record is the current dungeon's `Dungeon_MapData` (§8.5.1) | OBS (1,303 of 1,303 dungeons have `Dungeon_MapData` naming a 0x78 set with `UI_Map_GameMap`); BIN (q6: the client reads the player's `Dungeon_DID` when indoors and asks a game-script function for the (dungeon, map-data) pair) | Probable |
| G19 | Dungeon map census: image sizes, scales, fog flags (§8.5.2) | OBS | Confirmed |
| G20 | Viewport disc: a 170 × 170 A8R8G8B8 surface, black, alpha 0 outside a soft-edged circle, drawn with draw mode 2 (§8.7.1) | OBS | Confirmed |
| G21 | Minimap rim: state chosen from `Mood_Level` by a fixed table, clamped to −10…10 (§8.7.2) | BIN (q2: property watcher, mood-to-state table); OBS (state labels) | BIN |
| G22 | Time-of-day element: state from the world property `World_IsNight` (§8.7.3) | BIN (q2) | BIN |
| G23 | Coordinate and area labels are filled and shown by game-script results; the coordinate label is hidden when there is no result (§8.7.4) | BIN (q2) | BIN (rule OPEN) |
| G24 | The `MapZoom` button broadcasts a game event and does not change the radar's scale (§8.7.5) | BIN (q1: message handler) | BIN; event meaning OPEN |
| G25 | World-map tree in the `MapWindow` element's `UI_Map_AreaData`: node fields, counts, depth, active elements, images (§9.1–§9.2) | OBS (227 nodes) | Confirmed |
| G26 | The world-map window reads `UI_Map_AreaData` when it is created and builds its node tree from it (§9.1) | BIN (q6) | BIN |
| G27 | Each area DID is listed by exactly one node (§9.4) | OBS (541 areas) | Confirmed |
| G28 | The map shown first is the node that lists the player's area (§9.4) | LC; OBS (G27) | Reported |
| G29 | The world-map panel also loads a dungeon map through the same (dungeon, map-data) pair (§9.4) | BIN (q6) | BIN |
| G30 | `QuickslotBar` builds 12 slots from `QuickslotButtonTemplate` of the `quickslots` layout, sets `UI_Shortcut_Index` = shortcut offset + slot index, adds them to its list box (§10.1) | BIN (q5); OBS (template, list-box properties 1 × 12) | BIN |
| G31 | Empty slots are hidden, and shown between `Element_drag_global_begin` and `Element_drag_global_end` (§10.2) | BIN (q5); OBS (message labels) | BIN |
| G32 | Slot content templates per shortcut type; an empty shortcut uses `QuickslotEmptyTemplate` (§10.3) | BIN (q5: content dispatcher, the empty-content class) | BIN |
| G33 | Census of the remaining game types in the panels and their Tessera fallback (§11) | OBS; BIN (constructors of each type checked for a self-hide) | Confirmed (census); BIN (self-hide) |
| G34 | The drag bar's tooltip text is built from the `HiddenDragBox_Title` StringInfo (§4.3) | BIN (q3: creation routine passes the title to the drag bar's tooltip setter; the tooltip itself was not drawn or observed) | Probable |
| G35 | The `Mood` gauge (`MoodDisplay`) is driven by the same `Mood_Level` value as the rim (§8.7.2) | OBS (its states use the same 21 labels `Mood_n10` … `Mood_p10`); BIN (`MoodDisplay` code not read) | Probable |
| G36 | Clicking a `UI_Map_Child_Map` button shows the node whose active element it names (§9.2) | OBS (770 of 773 buttons name another node's active element and none names its own; the other 3 name top-level elements of the `map` layout that are no node's active element; G25, recounted 2026-10-06); LC (map loaders read the same property as a link) | Probable |
| G38 | Per-type comparison of the hide test: the type that selects the branch, 32-bit and float compares, types for which the ordering operators give false, types that use their own comparison routine; *B*'s value when the player lacks *P* (§6.2) | BIN (q4, re-read 2026-10-06: the equality routine that operators 2 and 3 share, the four ordering routines, and the routine that resets a value to its property's default); OBS (G7: only types 8, 15 and 21 occur in the data, with operators 2 and 3 for 8 and 21) | BIN |
| G37 | A quickslot's `QuickslotLabelText` shows the key bound to the slot's shortcut index (§10.1 step 3) | BIN (q5: the text comes from a key-binding lookup by shortcut index); the text format was not traced | Probable |
| G39 | In the resolved P1 tree, `GamePlay_Radar`, `MinimapPlayerMarker` and `Minimap_Frame` carry `UICore_Element_ObjectMode` 1 (Stretch) in the layout data, so the marker already has the mode that §8.2 step 3 sets (§8.2, §8.7.2) | OBS (UL U46, every element of the instantiated P1, 2026-10-06) | Confirmed |
| G40 | In the resolved P1 tree: `MinimapViewport` has no `DrawAfterChildren`; its children in drawing order (z-level descending, then read order, UL §9.3) are `MinimapMapImage` and `RadarMapImage` (z 2), `MinimapFogOfWarDisplay` (z 1), `RadarQuestObjective` and `MinimapPlayerMarker` (z 0, mode 1), then `Minimap_Frame` and seven more children at z −1, all in mode 1; the three map elements and their descendants have no object mode; `RadarMapImage_Template` has no object mode and no children; the children of `GamePlay_Radar` in mode 1 drawn before `MinimapViewport` (z ≥ 0 against its −1) lie at rows ≥ 194 of the radar, the viewport at rows 0–169; every child of `GamePlay_Radar` after `MinimapViewport` has mode 1 (§8.7.2) | OBS (2026-10-06: every element of the instantiated P1 and the template, resolved by UL §7, read-only probes) | Confirmed |
| G41 | In the resolved P1 tree, `GamePlay_Radar` is a direct child of the root `GamePlay_RadarBox` (224 × 280) with rect (0, 33, 224, 260): it sits at x = 0, y = 33 of the root, so its box reaches 13 rows past the root's bottom edge; every child of `GamePlay_Radar` and every descendant of `MinimapViewport` lies above radar row 247 (root row 280) except the map elements and the fog, which `MinimapViewport` clips (§8.7.2 "Clip") | OBS (2026-10-06: the instantiated P1 tree resolved by UL §7, read-only probe; one record, so all records) | Confirmed |

---

<a id="s2"></a>
## 2. Conventions

- **Local player.** "The player's property *P*" is the value of property *P* on the local player's
  object. The sandbox provides only what `tessera-net`'s sandbox backend models; every other
  property counts as **absent**.
- **Visible, hidden.** The element's visible flag (UL §9.3 step 1). "Hidden by code" means the
  original's game code calls the element's set-visible operation with false; a hidden element and
  its subtree are not drawn and not hit-tested (UL §9.3).
- **State switch.** "Switches to *S*" means UL §9.4 with state *S*; if the element's resolved
  states table does not define *S*, nothing changes (UL §9.7 last paragraph applies to every
  element).
- **Draw modes** are those of UL §9.6.1.
- **Elements, states and properties are found by label** in the user's data (naming note). Child
  elements are found by element ID anywhere below the widget (depth-first), as UL §9.2 creates
  them.
- **Tessera rule** marks a choice of this project where the original's rule depends on data the
  sandbox lacks or was not traced; it is not a claim about the original.

---

<a id="s3"></a>
## 3. Elements that game code hides or shows (summary)

| Game type (UL §17.2) | Original, in normal play (no tutorial, no edit mode, no drag) | Rule | Tessera in the sandbox |
|---|---|---|---|
| `HiddenDragBox` frame children (`HiddenDragBox_DragBox`, `HiddenDragBox_DragBar`, `HiddenDragBox_TitleText`) | Hidden; root in state `hidden` | §4 | Same; shown while Tessera's toggle (the `ToggleHiddenDragBoxes` action) is on |
| `TutorialHighlighter` (P1 2, P3 23, P5 1) | Hidden; state `normal` | §5 | Hidden always (no tutorials) |
| `HideWhenField` (P4 2, P5 1, P9 1) | Depends on the player's properties | §6 | Evaluated with absent properties; results in §6.3 |
| `PipDisplay` (P3 6) | Hidden at construction; shown by code (rule OPEN) | §7 | Hidden |
| `ToolbarQuickslotPageChanger` (P3 1) | Hidden at construction; shown by code (rule OPEN) | §7 | Shown (Tessera rule, §7) |
| `Radar` children `RadarQuestObjective`, `AreaText_QuestHighlight` | Hidden at creation | §8.2 | Hidden |
| `Radar` map elements | One of the two map displays shown, by indoor/outdoor | §8.3 | Same |
| `CoordinateText_Label` (P1) | Shown only when game script returns a text | §8.7.4 | Tessera setting, default shown |
| Empty quickslot buttons (P3, P14) | Hidden; shown while any drag is in progress | §10.2 | Same |

Elements hidden by `UICore_Element_hide` in the layout (UL §17.4 P6) are a layout fact, not
listed here.

---

<a id="s4"></a>
## 4. Drag-box frames (`HiddenDragBox`) — answers UL §17.5 item (1)

The roots of P1, P4, P5, P11 and P14 are of game type `HiddenDragBox`. Every instance in the data
(71 instantiated, G1) has three children: `HiddenDragBox_DragBox` (a Field with frame pieces),
`HiddenDragBox_DragBar` (an engine Dragbar) and `HiddenDragBox_TitleText` (a Text), and four
properties that name states: `HiddenDragBox_HiddenState` (= `hidden`),
`HiddenDragBox_VisibleState` (= `visible`), `HiddenDragBox_DraggingState` (= `dragging`) and
`HiddenDragBox_VisibleMouseoverState` (= `visible_mouseover`).

<a id="s4-1"></a>
### 4.1 Flags (BIN, G1)

A drag box keeps three booleans:

| Flag | Meaning | Initial |
|---|---|---|
| *shown* | the frame is shown (edit mode for this box) | false |
| *dragging* | a mouse button went down on the box and is still held | false |
| *over* | the pointer is over the drag bar | false |

<a id="s4-2"></a>
### 4.2 Refresh (BIN, G1)

Whenever a flag changes, the drag box:

1. Sets the visible flag of each of its three children to *shown*.
2. Chooses a state property: if not *shown*, `HiddenDragBox_HiddenState`; else if *dragging*,
   `HiddenDragBox_DraggingState`; else if *over*, `HiddenDragBox_VisibleMouseoverState`; else
   `HiddenDragBox_VisibleState`.
3. If the drag box has that property, switches itself to the state the property names (and §4.5
   passes it on).

**Reset** sets all three flags to false and refreshes.

<a id="s4-3"></a>
### 4.3 Creation and the toggle (BIN, G2)

1. After the tree is created (UL §9.2 post-creation), the drag box finds its three children,
   copies the StringInfo `HiddenDragBox_Title` into the title text, builds the drag bar's tooltip
   text from it (Probable, G34), resizes the title and drag bar, and **resets** (§4.2). One exception:
   when the drag box has `UIComponent_UIScaler_Target_Element` and no element with that ID exists
   below it, the reset is skipped (no instance in the 15 panels hits this case).
2. Each time the game raises the input action labelled `ToggleHiddenDragBoxes` (in the
   `InputActions` mapper, KM §12; ID 0x1000007B here), every drag box toggles its own flag: if
   *shown*, it resets; otherwise it sets *shown* and refreshes. Since all drag boxes start reset,
   they switch together.

So in normal play the frame, the drag bar and the title are **hidden** and the root is in state
`hidden`; the edit mode is the toggle. This settles UL §17.5 item (1); Tessera's
earlier choice agrees.

> **OPEN:** the drag bar's tooltip (G34, Probable): the creation routine passes the resolved
> `HiddenDragBox_Title` text to the drag bar's tooltip setter, but whether the tooltip shows that
> text unchanged or composes it with other text, and when it appears, were not observed.
> **Tessera rule** until settled: the drag bar's tooltip is the resolved title text, shown only
> while the frame is shown (§4.2).

<a id="s4-4"></a>
### 4.4 Mouse (BIN, G3)

Only while the children are visible can the pointer reach them (UL §9.3):

| UI message (`UIElementMessage` label) | Condition | Effect |
|---|---|---|
| `Element_mouse_over_top` | from `HiddenDragBox_DragBar` | *over* = (the message's value is 1); refresh |
| `Element_mouse_press` | from the drag box itself | *dragging* = true; refresh |
| `Element_mouse_release` | from the drag box itself | *dragging* = false; refresh |
| `Element_size_changed` | from the drag box's scaler target | the box resizes to the target's size plus a height property and refits its title (not needed for drawing) |

Moving the box is the engine Dragbar's job (UB).

<a id="s4-5"></a>
### 4.5 State passed to children (BIN, G4)

When a drag box switches state (by §4.2 or any other way), it also switches each **direct child**
whose `HiddenDragBoxChild_WantsHiddenDrabBoxStateChanges` is true to the same state (UL §9.4;
skipped where the child does not define it). In the data the three frame children carry the
property, and so do some content children (for example P14's `CtrlBar`, which defines no drag-box
state, so nothing happens there). The layout flag byte of the roots is 0, so this is the only way
the frame children receive `visible`, `dragging` or `visible_mouseover`.

> **OPEN:** whether a drag box passes the state further down than its direct children; the routine
> read walks one child list. No panel here needs more (the frame pieces of `HiddenDragBox_DragBox`
> receive states through that element's own flag byte, UL §5.4).

---

<a id="s5"></a>
## 5. Tutorial highlights (`TutorialHighlighter`) (BIN, G5)

1. After creation the element reads `UIComponent_TutorialHighlighter_Type` (0 when absent), marks
   itself inactive, **hides itself** and switches to `normal`.
2. When the game raises a tutorial-highlight event whose type equals the element's type (and a
   tutorial condition holds), the element becomes active, shows itself and switches to
   `highlight` (whose media switch to the pulsing `HighlightLoop` state, UL §6.8). When the event
   ends or names another type, it becomes inactive, hides and switches to `normal`.

Census (G5): 42 instantiated highlighters in all layouts, 25 with a type, none with
`UICore_Element_hide`. In normal play with tutorials finished, every highlighter is hidden.

**Tessera rule:** highlighters stay hidden (the sandbox has no tutorials). Their borders must not
be drawn by a fallback Field.

---

<a id="s6"></a>
## 6. Conditional fields (`HideWhenField`) (BIN, G6)

<a id="s6-1"></a>
### 6.1 Visibility

A `HideWhenField` is visible exactly when the **hide test** (§6.2) is false:

- after creation it sets its visible flag to "not hidden";
- whenever one of its condition properties (§6.2) changes on the element or the relevant player
  property changes, it sets it again;
- when anything asks it to show itself, it shows only if the hide test is false (asking it to hide
  always hides).

<a id="s6-2"></a>
### 6.2 Hide test

The conditions are tested in this order; the first that applies decides (**hide**), and if none
applies the field is **not hidden**. A boolean condition property applies only when present and
true.

| # | Property on the element | Hides when |
|---|---|---|
| 1 | `UI_HideWhenField_HideWhenWebstoreDisabled` | true and the store is disabled for the account |
| 2 | `UI_HideWhenField_HideWhenRipCord` (a value) | present and the account test for that value fails |
| 3 | `UI_HideWhenField_HideWhenRipCordEnabled` (a value) | present and the account test for that value passes |
| 4 | `UI_HideWhenField_HideWhenNonAdmin` | true and the account is not an administrator |
| 5 | `UI_HideWhenField_HideWhenNonVIP` | true and the account is not a subscriber |
| 6 | `UI_HideWhenField_HideWhenVIP` | true and the account is a subscriber |
| — | (no local player) | the test stops here: **not hidden** |
| 7 | `UI_HideWhenField_HideWhenMonsterPlayer` | true and the player is a monster player |
| 8 | `UI_HideWhenField_HideWhenSessionPlayer` | true and the player is in a session-play character |
| 9 | `UI_HideWhenField_HideWhenPvPPlayer` | true and the player is in PvP |
| 10 | `UI_HideWhenField_HideWhenPlayer` | true and a player-kind test of the client (not identified) fails |
| 11 | `UI_HideWhenField_HideWhenPropertyIsTrue` (a property ID) | the player's boolean property of that ID is present and true |
| 12 | `UI_HideWhenField_InvalidDataID` (a property ID) | the player's DID property of that ID is absent or the invalid DID |
| 13 | `UI_HideWhenField_PropertyCompare_Value` and `…_Operator` | the comparison below is true |
| 14 | `UI_HideWhenField_HideWhenNotOnWarsteed` (only tested when 13 is absent) | true and the player is not on a war-steed |

> **OPEN (implementation, M5 clean room):** row 12's "invalid DID" value is not stated.
> `tessera-ui` (`game::hidewhen`) treats DID 0 as invalid.

**Comparison (row 13).** The value property is an array whose **first element** is a property
*P* with a value *A* (the data value). The operator (enum, default 0) selects a test of *A*
against *B*, the player's value of *P*. If the player lacks *P*, *B* is *P*'s default from the
property dictionary (PS §2.2, flag 0x800) when it has one, else the zero value of *P*'s type (0
for int, enum, float, property ID, 32-bit bitfield and DID reference; false for a boolean; 255,
"undefined", for a tri-state; an empty value of its own kind for the other codes) (G38). The field hides when the test is true:

| Operator | Test | Operator | Test |
|---|---|---|---|
| 1 | *A* ≥ *B* | 4 | *A* < *B* |
| 2 | *A* ≠ *B* | 5 | *A* ≤ *B* |
| 3 | *A* = *B* | 6 | *A* > *B* |
| 0 or > 6 | false (never hides) | | |

How *A* and *B* are compared depends on *P*'s type code in the property dictionary (PS §3) (BIN,
G38):

| *P*'s type (PS §3 code) | Operators 2 and 3 (≠, =) | Operators 1, 4, 5, 6 (≥, <, ≤, >) |
|---|---|---|
| int (15), enum value (8) | the two 32-bit values are equal or not | signed 32-bit comparison; an enum compares its key as a signed 32-bit integer |
| float (9) | float comparison; if either value is NaN, = is false and ≠ is true | float comparison; false if either value is NaN |
| boolean (21), tri-state (5), property ID (10), 32-bit bitfield (18), DID reference (20) | the two stored 32-bit values are equal or not (a boolean is 0 for false, 1 for true, PS §4.5) | **false** (never hides) |
| any other code (1–4, 6, 7, 11–14, 16, 17, 19, 22) | the value type's own comparison (not traced) | the value type's own comparison (not traced) |

Observed operators: 6 (7 fields), 2 (5), 5 (4), 3 (3). Of the 19 fields with a comparison (G7),
14 compare an int property, 2 an enum property (operators 3 and 2) and 3 a boolean property
(operator 3 once, 2 twice); none compares a float or a type of the last table row. The 2 enum and
3 boolean fields are outside the 15 panels. Every compared property's dictionary default is 0 or
false, or absent (G7), so *B* is 0 or false whenever the player lacks *P*.

> **OPEN:** how the original compares values whose type has its own comparison routine (codes 1–4,
> 6, 7, 11–14, 16, 17, 19 and 22, the last row of the table above). That routine was not traced.
> No `HideWhenField` in the data compares such a type (G7). **Tessera rule** until settled: every
> operator gives false (never hides) for these types.

> **OPEN:** the exact account and player tests behind rows 1–10 and 14 (they read account
> entitlements and player state that only the server sets). None of the 15 panels uses them except
> row 7 (P9), for which the sandbox answer is "not a monster player".

<a id="s6-3"></a>
### 6.3 The fields in the panels (Confirmed data, G7) and the sandbox result

| Panel | Element | Condition | Sandbox (*B* absent → 0, normal player) |
|---|---|---|---|
| P4 | `PlayerLevel_HWField` | operator 2, value `LevelScaling_ScaledLevel` = 0 | 0 ≠ 0 is false → **visible** |
| P4 | `PlayerScaledLevel_HWField` | operator 3, value `LevelScaling_ScaledLevel` = 0 | 0 = 0 is true → **hidden** |
| P5 | `OpponentLevel_HWField` | operator 2, value `LevelScaling_ScaledLevel` = 0 | **visible** (the test reads the local player, not the target) |
| P9 | `Quest_Task_Filter` | `UI_HideWhenField_HideWhenMonsterPlayer` true | **visible** |

So the plain level field shows and the scaled-level field is hidden unless the player is level-scaled;
a fallback Field that draws both would overlay two level numbers.

---

<a id="s7"></a>
## 7. Pip displays and the page changer (BIN, G8)

- **`PipDisplay`** (P3: six instances placed on top of each other, one per class resource, each
  with `UIComponent_PipDisplay_Type` and a `UIComponent_PipDisplay_List` of {state, value} pairs)
  hides itself in its constructor. Five of the six have no default state and no element media, so
  they would draw nothing even if visible; `Gameplay_AtunementUI` has default state `atune10`.
- **`ToolbarQuickslotPageChanger`** (P3) hides itself in its constructor.

> **OPEN:** which game code shows a `PipDisplay` (Probable: the one whose type matches the local
> player's class resource) and the `ToolbarQuickslotPageChanger` (Probable: the quickslot page
> logic), and what the page indicator text shows.

**Tessera rules:** pip displays stay hidden (UL §17.4 P3 agrees). The page changer is shown, with
the page indicator showing Tessera's current page number (1), because the original's main toolbar
shows page arrows in normal play (to be confirmed by the owner's screenshot, checklist P3-A).

> **OPEN (implementation, M5 clean room):** which element below `ToolbarQuickslotPageChanger` is
> the page indicator; no label is given. `tessera-ui` (`game::simple`) sets the page number on
> the first Text element below the changer in depth-first order.

---

<a id="s8"></a>
## 8. The minimap (`Radar`) — answers UL §17.5 item (4) for P1

<a id="s8-1"></a>
### 8.1 Elements the radar uses

P1's `GamePlay_Radar` is based on `MinimapWindow` of the `radar` layout. The radar finds these
descendants by ID (G9); positions are layout values (UL §8):

| Element | Type | Rect (x, y, w, h) | Role |
|---|---|---|---|
| `MinimapViewport` | Field | (27, 0, 170, 170) | Clip window; draws the disc (§8.7.1); its centre is the player's place |
| ↳ `RadarMapImage` | ListBox | (0, 0, 600, 600) | Outdoor tile grid (§8.4); `UICore_ListBox_horizontal` true, 3 rows × 3 columns |
| ↳ `MinimapMapImage` | Field | (0, −1, 2048, 1536) | Indoor map image (§8.5) |
| ↳ `MinimapFogOfWarDisplay` › `MinimapFogOfWarParent` › `MinimapFogOfWarField` | Field › Field › `MapFogOfWar` | 2048 × 1536 | Indoor fog of war (§8.5.3) |
| ↳ `MinimapPlayerMarker` | Field | (0, 3, 28, 28) | Player marker (§8.6.3) |
| ↳ `RadarQuestObjective` | Field | (0, 0, 30, 30) | Quest objective blip; hidden at creation |
| ↳ `Minimap_Frame` | Panel | (−34, −33, 237, 237) | The rim (§8.7.2) |
| ↳ `TimeOfDay`, `Mood`, `MapZoom`, `PerkBuy_DestinyPoints`, housing buttons, one highlighter | various | | §8.7, §11 |
| `AreaText_Label`, `CoordinateText_Label` | Text | (27, 200, 170, 20), (12, 220, 200, 20) | §8.7.4 |
| `AreaText_QuestHighlight` | Field (animation) | (0, 194, 224, 32) | Hidden at creation |

`UI_Radar_RadarRadius` (81 here) is read by the radar; it bounds note blips (§8.8).

<a id="s8-2"></a>
### 8.2 Creation (BIN, G9)

After the tree is created, the radar:

1. Takes the viewport centre *c* = (*W* div 2, *H* div 2) of `MinimapViewport`'s size, with div
   truncating toward zero: (85, 85) for 170 × 170.
2. Finds `MinimapMapImage`, `RadarMapImage`, `MinimapPlayerMarker`, the two labels and the fog
   elements; if any is missing it stops (the minimap shows no map).
3. Sets the marker's `UICore_Element_ObjectMode` to 1 (UL §8.4 object modes; the layout data
   already give it 1, G39) and places the marker
   so that its centre is on *c*: position (*c*.x − *w* div 2, *c*.y − *h* div 2) = (71, 71) for a
   28 × 28 marker.
4. Shows `MinimapFogOfWarDisplay` (it is hidden again outdoors, §8.3) and hides
   `RadarQuestObjective` and `AreaText_QuestHighlight`.

<a id="s8-3"></a>
### 8.3 Indoors or outdoors (BIN, G10)

Each time the player's position is updated, and the position is valid (CO §7: region ≠ 0,
X ≠ 0xFF, Y ≠ 0xFF, cell ≠ 0xFFFF):

- **interior cell** (cell neither 0 nor 0xFFFF): show `MinimapMapImage` and
  `MinimapFogOfWarDisplay`, hide `RadarMapImage`;
- **outdoors** (cell 0): hide `MinimapMapImage` and `MinimapFogOfWarDisplay`, show
  `RadarMapImage`, then update the tile grid (§8.4).

Then the radar places the map (§8.6.2), rotates the marker (§8.6.3) and updates notes (§8.8). With
an invalid position nothing changes.

<a id="s8-4"></a>
### 8.4 Outdoors: the tile grid (BIN, G11–G13)

<a id="s8-4-1"></a>
#### 8.4.1 Which landblocks

For the player's landblock (*X*, *Y*) in region *R*:

1. *x*<sub>min</sub> = *X* − 1 if *X* − 1 > 0, else 0; *x*<sub>max</sub> = *X* + 1 if *X* + 1 < 254
   (0xFE), else 254. The same for *y* with *Y*.
2. If *x*<sub>max</sub> − *x*<sub>min</sub> ≠ 2: if *x*<sub>min</sub> = 0, set *x*<sub>max</sub> = 2;
   otherwise, if *x*<sub>max</sub> = 254, set *x*<sub>max</sub> = 252. The same for *y*.
   (For *X* = 254 this leaves an empty range, 253…252; the client behaves so. No landblock
   with X or Y of 253 or 254 is known to carry a minimap tile; Tessera may treat that case as
   "no tiles".)
3. The grid covers *x*<sub>min</sub>…*x*<sub>max</sub> × *y*<sub>min</sub>…*y*<sub>max</sub>, normally the
   3 × 3 blocks centred on the player's block.

The grid is rebuilt only when the region, *X*, *Y* or the content layer (§8.4.3) differ from the
last build.

<a id="s8-4-2"></a>
#### 8.4.2 Which records

For each block of the grid the radar requests the **landblock-properties record** 0x804RXXYY
(LB §6; DB type LANDBLOCKPROPERTIES) and builds the grid when all requested records have arrived.
From each record it uses the `UI_Map_GameMap` struct (LB §6.2). A block without a record or
without `UI_Map_GameMap` gives a tile with no image.

<a id="s8-4-3"></a>
#### 8.4.3 Which image (content layer)

Let *L* = the player's `Volume_ContentLayer` (enum; 0 when absent). For each block:

1. If *L* = 0: the image is `UI_Map_MapImage` of `UI_Map_GameMap`.
2. Otherwise walk `UI_Map_MapImageArray` in stored order. The first element whose
   `UI_Map_ContentLayer` equals *L* gives the image. If none matches, the **last** element whose
   `UI_Map_ContentLayer` is 1 gives the image. If neither exists, use `UI_Map_MapImage`.

<a id="s8-4-4"></a>
#### 8.4.4 Placing the tiles

1. The grid's items are made in **row-major order, north row first, west column first**: for *Y*
   from *y*<sub>max</sub> down to *y*<sub>min</sub>, and for each, *X* from *x*<sub>min</sub> up to
   *x*<sub>max</sub>. Item *i* reuses the list box's existing item *i*, or is created from the
   element `RadarMapImage_Template` (a 200 × 200 Field) of the `radar` layout.
2. Each item gets its block's image with **draw mode 5** (UL §9.6.1, Mix colour: keeps the
   destination alpha), replacing the item's media; a block with no image clears the item's image.
   The block's `UI_Map_GameMap` struct is also applied to the item as properties.

> **OPEN (implementation, M5 clean room):** what "applied to the item as properties" means
> (the struct set as one property `UI_Map_GameMap` on the item, or each of its fields set as a
> property of the item), and whether anything reads them. `tessera-ui` (`game::radar`) keeps
> the decoded struct in the radar and sets no item property; nothing it draws depends on it.
3. In the list box, item *i* sits at column *i* mod 3 and row *i* div 3, each 200 × 200 pixels:
   item (*col*, *row*) covers list pixels [200·*col*, 200·*col* + 200) × [200·*row*, 200·*row* + 200).

> **OPEN (cross-spec):** the ListBox engine widget's arrangement is not specified (critique item
> A2). Step 3 is what the radar's own pixel transform (§8.6.1) assumes, so Tessera arranges the
> tile items that way for this list box regardless.

<a id="s8-5"></a>
### 8.5 Indoors: the dungeon map (BIN, G17–G19)

<a id="s8-5-1"></a>
#### 8.5.1 Which map

Game code passes the radar a map-data DID; the radar loads that DBPROPERTIES record (a 0x78
property set, PS §6) and reads its `UI_Map_GameMap` struct. The client obtains the pair (dungeon,
map data) from a game-script function, after reading the player's `Dungeon_DID` when the player's
position is a valid interior position.

Data agreement (G18): every dungeon record (1,303, WS §11.1) has `Dungeon_MapData`, a 0x78 DID
whose set holds `UI_Map_GameMap`; the landblock-info cells name their dungeon by `Dungeon_DID`
(LB §4, WS §11.4).

**Tessera rule (Probable original rule):** indoors, take the cell's `Dungeon_DID`, read the
dungeon's `Dungeon_MapData` and use its `UI_Map_GameMap`. If the dungeon has none, show no map.

> **OPEN (implementation, M5 clean room):** reading the records may take frames. While the
> lookup for an interior position is pending, `tessera-ui` (`game::radar`) keeps the indoor
> map shown only when the last answered lookup was for the same landblock; otherwise it
> clears `MinimapMapImage`'s image (and leaves the map where it is) until the lookup answers.
> Leaving an interior clears the image as well.

> **OPEN:** the game-script function's rule (for example whether it walks
> `Dungeon_ParentDungeon` or checks quest state) is not traced (script code).

<a id="s8-5-2"></a>
#### 8.5.2 The image

1. The radar sets `MinimapMapImage`'s image to `UI_Map_MapImage` with **draw mode 5** and resizes
   the element to the image's pixel size.
2. The georeference (§8.6.1) is the struct's `UI_Map_BlockOffsetX`, `UI_Map_BlockOffsetY`,
   `UI_Map_PixelOffsetX`, `UI_Map_PixelOffsetY` and `UI_Map_Scale`.

Census of the 1,303 dungeon maps (G19): every image is a JPEG (format 500); sizes 800 × 600 (965),
1600 × 1200 (295), 2400 × 1800 (24), 3200 × 2400 (6), 1600 × 2400 (4), 3200 × 3600 (3),
800 × 1800 (3), and one each of 1200 × 900, 2400 × 3000 and 4800 × 4800. Scales 3.0 (642),
1.5 (219), 6.0 (107), 5.0 (93), 4.0 (55), 2.0 (39), others below 25 each. `UI_Map_FogOfWar` is
true in 670 and false in 633.

<a id="s8-5-3"></a>
#### 8.5.3 Fog of war

The radar enables its `MapFogOfWar` element, sized to the image, only for a map whose fog data
apply (Probable: `UI_Map_FogOfWar` true); otherwise it disables it. The fog shows which parts of
the dungeon the player has explored (player data).

**Tessera rule:** no fog (the map is drawn uncovered); `MinimapFogOfWarField` draws nothing.

> **OPEN:** the condition under which the radar enables `MapFogOfWar`. The enable and disable
> calls were read, but the value they test was not traced (Probable: `UI_Map_FogOfWar` of the
> map's `UI_Map_GameMap`, true in 670 of the 1,303 dungeon maps and false in every
> landblock-properties record, §14). The Tessera rule above (no fog) does not depend on it.

> **OPEN:** how `MapFogOfWar` draws (cell size, colour from `UI_Map_FogOfWar_Color`, the explored
> set).

<a id="s8-6"></a>
### 8.6 Placement

<a id="s8-6-1"></a>
#### 8.6.1 World point to map pixel (BIN, G14)

For a map with block offsets (*bx*, *by*), pixel offsets (*px*<sub>0</sub>, *py*<sub>0</sub>) and
scale *s* (pixels per world unit), and a world point with global coordinates (*G*<sub>x</sub>,
*G*<sub>y</sub>) (CO §10):

- *dx* = *G*<sub>x</sub> − 160·*bx*, *dy* = *G*<sub>y</sub> − 160·*by*, in 32-bit floats;
- *px* = *px*<sub>0</sub> + trunc(*dx* · *s*), *py* = *py*<sub>0</sub> − trunc(*dy* · *s*), where
  trunc rounds toward zero (so a small negative product gives 0).

This is CO §10's formula with the truncation made explicit.

For the **outdoor grid** the radar takes the georeference from one tile item: it tries item 6 (row
2, column 0), then item 4, then the other items in index order, and uses the first that has a
`UI_Map_GameMap` with block offsets and a scale. It uses, in place
of the tile's stored pixel offsets, that item's position in the list: *px*<sub>0</sub> =
200·*col*, *py*<sub>0</sub> = 200·(*row* + 1), with (*bx*, *by*) the tile's block. Because every
tile has pixel offsets (0, 200) and scale 1.25 (LB §6.2), this equals placing each tile's own
pixel frame at its list position; the result is list-box pixels.

> **OPEN:** what the radar does outdoors when no tile item yields a georeference: the grid is empty
> (the range at *X* = 254 or *Y* = 254 of §8.4.1 and §13.2) or no block of the grid has a
> `UI_Map_GameMap` with block offsets and a scale. Not traced: whether `RadarMapImage` keeps its
> last position, whether list-box items of the previous build stay, and whether the marker is
> still drawn. **Tessera rule** until settled: remove every item of `RadarMapImage` (no tiles are
> drawn), leave its position unchanged, and keep the marker centred (§8.2); GP10 does not apply.

For the **indoor map** the result is pixels of `MinimapMapImage`.

<a id="s8-6-2"></a>
#### 8.6.2 Scrolling (BIN, G15)

With (*px*, *py*) the player's map pixel and *c* the viewport centre (§8.2), the radar moves the
shown map element (`RadarMapImage` outdoors, `MinimapMapImage` and `MinimapFogOfWarParent`
indoors) to position (*c*.x − *px*, *c*.y − *py*) inside `MinimapViewport`. The map is never
scaled or rotated: north is up, and one map pixel is one screen pixel at UI scale 1.

<a id="s8-6-3"></a>
#### 8.6.3 The player marker (BIN, Probable direction, G16)

The marker image (28 × 28; its arrow points to the right, east, in the stored image) is drawn
rotated about its centre so that the arrow points along the player's compass heading *h* (CO
§11.4: clockwise from north): the client rotates by (90° − *h*) in its own rotation sense, which
with an east-pointing arrow gives a clockwise screen rotation of (*h* − 90°).

> **OPEN:** confirm the screen direction with an owner screenshot (marker of a character facing
> north must point up); and the resampling the client uses for a rotated image (UL §9.6 draws
> images unrotated only).

**Tessera rule** (DEC: UR §2.3, Decision 1.7; answers Tessera's side of the resampling OPEN):
the marker owns a surface (step 3 of §8.2), so it is composed as a separate entry in P1's
drawing order, not blended into the radar's canvas. Its rotation is a rotation of that entry
about its centre, set so that the screen result is the one above (an east-pointing arrow turns
clockwise on screen by *h* − 90°), whatever sign convention the plugin API's rotation turns
out to have. The rotated marker is resampled with Tessera's surface filter (bilinear on
premultiplied texels by default). Its coverage region in the radar's canvas is the square
around the circle through its corners, 40 × 40 for the 28 × 28 marker, the same at every
heading, so turning causes no repaint. It takes its own opacity only, so a Fade on P1's root
does not fade it (UL §5.9 opacity OPEN).

<a id="s8-7"></a>
### 8.7 Rim, disc, labels and buttons

<a id="s8-7-1"></a>
#### 8.7.1 The disc (OBS, G20)

`MinimapViewport`'s own image is a 170 × 170 A8R8G8B8 surface (0x41007F4B here): black, alpha
255 inside a circle of radius about 85 pixels around the centre with a soft edge, alpha 0 outside.
It is drawn with **draw mode 2** (Replace, UL §9.6.2), which writes its alpha into the panel
surface; the map tiles and the indoor map are drawn after it with draw mode 5, which keeps that
alpha. The minimap is round only because the panel surface's alpha is used when the surface is
composed onto the frame (UL §9.6.1 OPEN). A Tessera implementation that composes panels with
their alpha gets the round map; one that ignores alpha shows a square. Tessera's composition
always uses the surface alpha (UL §9.6.1 Tessera rule, DEC: UR Decision 1.6 and its §8
round-minimap test).

<a id="s8-7-2"></a>
#### 8.7.2 The rim (BIN, G21)

`Minimap_Frame` has no default state and no own media; its art is in 21 states `Mood_n10` …
`Mood_0` … `Mood_p10`. Whenever the player's `Mood_Level` (int) changes, the radar switches the
rim to:

| `Mood_Level` *m* | State |
|---|---|
| *m* < −10 | `Mood_n10` |
| −10 ≤ *m* ≤ −1 | `Mood_n`\|*m*\| |
| 0 | `Mood_0` |
| 1 ≤ *m* ≤ 10 | `Mood_p`*m* |
| *m* > 10 | `Mood_p10` |

`Mood_0`'s image is the plain rim (238 × 238 here). The `Mood` element (a `MoodDisplay`, default
state `Mood_0`) shows a small gauge with the same state labels (Probable, G35: driven by the same
value).

**Tessera rule:** the sandbox has no mood; *m* = 0, so the rim is in state `Mood_0` and `Mood` in
its default state.

> **OPEN:** what drives the `Mood` gauge (G35, Probable). Its states use the same 21 labels as the
> rim, but the `MoodDisplay` code was not read, so neither the value it reads nor its mapping to
> states is known. In the sandbox both readings give `Mood_0`, the gauge's default state, so M5 does
> not depend on this.

> **OPEN:** `Minimap_Frame` lies partly outside `MinimapViewport` (rect (−34, −33, 237, 237) in a
> 170 × 170 parent), and 6,428 of the rim image's 13,373 pixels with alpha above 8 fall outside the
> viewport box. Under UL §9.3 step 6 (children clipped to the parent) most of the rim's outer
> ring would be cut off. Either the client does not clip here (UL §9.3 `clip_to_parent` OPEN) or
> the original shows a clipped rim. Settle with an owner screenshot (checklist P1-A). Tessera
> draws the rim unclipped by the viewport (clipped only by the radar root) until then.
> Data that bear on this (G39): `Minimap_Frame` carries object mode 1 in the layout, so by
> LPA §13.19 it owns a surface of its own. The clip of UL §9.3 step 6 is traced for drawing
> into one surface; whether the viewport's box clips an element with its own surface depends
> on how the original composes such surfaces (UL §9.3 nested-surface OPEN, LPA §13.18
> OPEN (1)). Tessera composes it as a nested entry
> clipped only by its enclosing root panel (DEC: UR Decision 1.7), which gives the same
> result as the Tessera rule above.

<a id="s8-7-2-composition"></a>
**How P1 splits into surfaces in Tessera** (DEC: UR Decision 1.1 and 1.7; data G40, UL §17.2).
Decision 0015 names the marker as P1's only nested entry and checks rim pixels "in the root
canvas" (its P1 check 2). The data give P1 more surfaces than that, so test authors should use
the following instead:

- **Surfaces.** 18 non-root elements of P1 have a non-zero object mode (UL §17.2), so each owns
  a surface in Tessera when shown. `GamePlay_Radar` itself is in mode 1: it is a nested entry
  of the root's surface, and the rim `Minimap_Frame`, the marker, `RadarQuestObjective`,
  `TimeOfDay`, `Mood`, the five buttons, the two labels, `AreaText_QuestHighlight` and the
  two tutorial highlighters are nested entries of the **radar's** surface, one level further
  down. The drag bar and title of the drag box are nested entries of the root's surface. So
  the disc, the tile grid, the indoor map and the fog are painted into the radar's canvas (the
  "panel surface" of §8.7.1 is, in Tessera, the radar's surface), and the rim's pixels are in
  the rim's own canvas, not in the root canvas.
- **Drawing order (G40).** In `MinimapViewport`'s drawing order (UL §9.3; it has no
  `UICore_Element_DrawAfterChildren`) its own disc image comes first, then `MinimapMapImage`
  and `RadarMapImage` (z-level 2), then `MinimapFogOfWarDisplay` (z-level 1), and only then
  the first children that own a surface (`RadarQuestObjective` and the marker at z-level 0,
  the rim and the rest at −1). The tile items made from `RadarMapImage_Template` have no object
  mode and no children. In `GamePlay_Radar`'s drawing order the children that own a surface
  and come before `MinimapViewport` (the quest highlight, the area-text tutorial highlighter
  and the two labels) all lie at radar rows 194 and below, outside the viewport's rows 0–169.
  Every element drawn after the rim owns a surface. So under the split of UR Decision 1.7 no
  pixel of the disc, the tiles, the indoor map or the fog falls inside the coverage region of
  an earlier nested entry: all of them go to the radar's canvas below, where draw mode 5 finds
  the disc's alpha (§8.7.1). The rim's coverage region contains the whole disc, but nothing of
  the radar's canvas is drawn after the rim, so its canvas above stays empty.
- **Clip.** `GamePlay_Radar` is not a root panel, so the nearest enclosing window of the rim,
  the marker and the other nested entries of the radar is P1's root (UR Decision 1.7, "Clip"),
  as the Tessera rule above says. `GamePlay_Radar` sits at x = 0, y = 33 of the root
  `GamePlay_RadarBox` (224 × 280; rect (0, 33, 224, 260), G41) and `MinimapViewport` at
  (27, 0) of the radar. So in layout values at the design size the rim's box runs from
  7 pixels left of the root's box (0 + 27 − 34 = −7) to 6 pixels past its right edge
  (−7 + 237 = 230 against a width of 224), and from root row 0 (33 + 0 − 33) to row 237 of
  280; the root's clip cuts those 13 columns and no row. The radar's own box reaches root
  row 293, but nothing the radar draws lies below root row 280 (G41), so that cut removes no
  pixel. These offsets are this install's layout values; read them from the data.
- **Test.** Instead of decision 0015's P1 check 2: in a painter and composition test on a
  synthetic P1-shaped tree, the composed result shows rim pixels outside the 170 × 170 viewport
  box and none outside the root's box, and the radar's canvas holds the disc and the map
  pixels with the disc's alpha (GP17).
  Decision 0015's P1 check 4 is corrected in one clause: the marker and the rim are both
  nested entries of the radar's surface, the rim paints into its own canvas, and everything
  after the marker in the radar's drawing order owns a surface (G40), so the marker's
  **canvas above is empty** at every heading; it holds no rim pixels. A test that follows
  check 4 asserts that the canvas above is empty at every angle, not that it "holds the same
  rim pixels". The rest of check 4 is unchanged: the marker's coverage region is the same
  40 × 40 square at every heading, and turning through 360° in 1° steps gives no damage after
  the first rotation.

<a id="s8-7-3"></a>
#### 8.7.3 Time of day (BIN, G22)

The `TimeOfDay` element (`TimeOfDayDisplay`, default state `Unknown_Time`) switches to `Night_Time`
when the world property `World_IsNight` is present and true, to `Day_Time` when it is present and
false, and to `Unknown_Time` when it is absent, when there is no local player, or while one of two
internal flags of the element (not identified) is set.

> **OPEN:** what the two internal flags of `TimeOfDayDisplay` that force `Unknown_Time` are, and
> which game code sets and clears them (the routine that reads them was traced; the writers were
> not). **Tessera rule** until settled: both are clear, so the state follows `World_IsNight` alone.

**Tessera rule:** set `World_IsNight` from the walker's game clock with Tessera's own day/night
boundary (`environment.md` keeps the original's night rule OPEN); until then `Day_Time` between
the clock's dawn and dusk and `Night_Time` otherwise.

<a id="s8-7-4"></a>
#### 8.7.4 Area and coordinate labels (BIN, G23)

Both texts are produced by game code: the coordinate label's text and visibility come from a
game-script function (when it returns no text the label is hidden; Probable: it returns text when
the player's option `GOP_DisplayCoordinatesOnRadar` is on); the area label's text is set from the
player's area when the area or neighbourhood changes (Probable).

**Tessera rules:** `AreaText_Label` shows the area name of WS §11.2 for the player's block;
`CoordinateText_Label` shows the map coordinates of CO §8.3 and is visible while Tessera's own
setting (default on) is on.

> **OPEN:** the coordinate-label rule (G23): which game-script function produces the text, the
> condition under which it returns text (Probable: the option `GOP_DisplayCoordinatesOnRadar` is
> on) and the text's format; and the event that updates the area label (Probable: a change of the
> player's area or neighbourhood). The script code was not traced. The Tessera rules above apply
> until then.

<a id="s8-7-5"></a>
#### 8.7.5 Buttons (BIN, G24)

`MapZoom` broadcasts a game event when clicked (Probable: open the world map, P10); the radar has
no zoom of its own. `StablemastersButton` opens a panel page by key. `PerkBuy_DestinyPoints` has a
`UICore_Button_click_action`. These buttons are visible as the layout leaves them.

**Tessera rule:** `MapZoom` opens P10.

> **OPEN:** the meaning of the game event `MapZoom` broadcasts (G24): which code handles it and
> what it does (Probable: open or toggle the world map, P10). The radar's own handler only
> broadcasts it and changes no scale.

<a id="s8-8"></a>
### 8.8 Notes and blips

The radar also places note icons (quest objectives, vendors, other players) from game data and
server data, bounded by `UI_Radar_RadarRadius` and filtered by `UI_Map_MapNoteDisplayType`.

> **OPEN:** note placement and the blip art (not traced). Tessera shows no notes.

<a id="s8-9"></a>
### 8.9 Summary for the sandbox

Outdoors: 3 × 3 minimap tiles of LB §6.3 chosen by §8.4.3 with Tessera's content layer (the
walker's, `walker.md` §6), scrolled by §8.6, inside the round disc, under the rim in state
`Mood_0`, marker centred and rotated, area and coordinate labels filled. Indoors: the dungeon map
of §8.5, uncovered. The walker's own debug minimap (`walker.md` §6, ±320 m) is a different thing
and is not this panel.

---

<a id="s9"></a>
## 9. The world map (P10) — answers UL §17.5 item (4) for P10

<a id="s9-1"></a>
### 9.1 The map tree (Confirmed data, G25; BIN, G26)

The `map` layout's top-level element `MapWindow` (1024 × 768, edges (5, 5, 5, 5)) carries the
struct property `UI_Map_AreaData`: the **root node** of a tree of map nodes. The client's map
window reads it when created and builds its node tree from it. Each node is a struct with:

| Property | Type (PS §3) | Present in | Meaning |
|---|---|---|---|
| `UI_Map_ActiveElement` | enum (element ID) | 227 of 227 | The top-level element of the `map` layout that shows this map |
| `UI_Map_MenuName` | StringInfo | 227 | The map's name in the map menu |
| `UI_Map_MapImage` | DID (0x41) | 227 | The map image |
| `UI_Map_BlockOffsetX`, `UI_Map_BlockOffsetY` | int | 227 | Georeference block (§8.6.1) |
| `UI_Map_PixelOffsetX`, `UI_Map_PixelOffsetY` | int (signed) | 227 | Georeference pixel; negative in 17 nodes |
| `UI_Map_Scale` | float | 227 | Pixels per world unit, 0.0173 to 3.2875 |
| `UI_Map_PixelWidth`, `UI_Map_PixelHeight` | int | 227 | 1024 × 768 in every node |
| `UI_Map_RegionID` | enum (REGION) | 226 | Region number (absent on the root) |
| `UI_Map_AreaDIDs_Array` | array of `UI_Map_AreaDID` | 214 | Area DIDs (WS §11) this map covers |
| `UI_Map_AreaData_Array` | array of node structs | 64 | Child nodes |
| `UI_Map_MapNoteFilterProperty`, `UI_Map_IncludeCommonMapNoteTypes`, `UI_Map_MapNoteDisplayType` | | 227, 219, 98 | Note filtering (§8.8) |
| `UI_Map_QuestGuideBound_*`, `UI_Map_QuestGuideDisabled`, `UI_Map_HideFilterButtons`, `UI_Map_PreventZoomOut` | | 9, 7, 7, 1 | Quest guide and menu options |

Shape: 227 nodes, depth 0–5 (1, 6, 72, 108, 35, 5 nodes per depth); the root's active element is
`WorldMap`. Node regions: 2 (74), 1 (47), 3 (40), 4 (38), 5 (25), 14 (2).

<a id="s9-2"></a>
### 9.2 Active elements and child-map buttons (Confirmed data, G25)

- Every node's active element is a distinct top-level **Field** of the `map` layout, 1024 × 768,
  whose Image item names the node's `UI_Map_MapImage` in 226 of 227 nodes; one node's element
  (`CityMap_1_Archet`) has no Image item. Every map image is a 1024 × 768 JPEG except one of
  1024 × 766.
- The active elements' children include Buttons with `UI_Map_Child_Map` (an element ID): 773 such
  buttons in the instantiated active elements, 770 naming another node's active element. Clicking
  one navigates to that node (Probable, G36). The targets are not limited to the node's children in
  the tree: 196 of 227 nodes have buttons to nodes that are not their tree children (199 to any
  other node).

**Tessera rule:** to show node *N*, draw *N*'s `UI_Map_MapImage` as the image of *N*'s active
element (this also covers the one element without an Image item).

> **OPEN (implementation, M5 clean room):** the draw mode for that image is not stated.
> `tessera-ui` (`game::worldmap`) uses the draw mode of the element's own Image item, or draw
> mode 2 (Normal) when it has none.

> **OPEN:** what a click on a `UI_Map_Child_Map` button does (G36, Probable: show the node whose
> active element the button names; the world-map window's message handler was not read for this),
> and what the other 3 of the 773 buttons do: each names a top-level element of the `map` layout
> that exists but is no node's active element (OBS 2026-10-06; no button names its own node).
> **Tessera rule** until settled: a click shows the named node; a button whose target is no node's
> active element does nothing.

<a id="s9-3"></a>
### 9.3 Georeference

The node's offsets and scale map world points to pixels of the 1024 × 768 map by §8.6.1. The player
marker is placed at the player's pixel; notes likewise (§8.8, OPEN).

<a id="s9-4"></a>
### 9.4 Which map is shown

- **Data (Confirmed, G27):** each of the 541 area DIDs that occur in `UI_Map_AreaDIDs_Array` is
  listed by exactly one node.
- **Reported (LC, G28):** the map for an outdoor position is the node listing the area of the
  position's landblock (`Area_DID` of the block map, LB §5, WS §11.2).
- **BIN (G29):** for an interior position the world-map panel obtains the same (dungeon, map-data)
  pair as the radar (§8.5.1) and shows the dungeon map from `Dungeon_MapData`.

**Tessera rule:** outdoors, show the node listing the player's area; if none, the node with the
player's region (`UI_Map_RegionID`) at the smallest depth; if none, the root. Indoors, show the
dungeon map of §8.5.1 in the same window (image and georeference from its `UI_Map_GameMap`).

> **OPEN:** the original's choice when the area is in no node, whether it prefers a deeper node
> that contains the position by its pixel bounds, and how it draws a dungeon map in the
> 1024 × 768 window (dungeon images are larger, §8.5.2; scrolling or scaling not traced).

> **OPEN (implementation, M5 clean room):** until the item above is settled, `tessera-ui`
> (`game::worldmap`) draws the dungeon map on an element of its own (no layout entry), placed
> like a node's active element (the first child of `MapBackground`), at (0, 0), unscaled,
> resized to the image's pixel size, with draw mode 2 (Normal); there is no scrolling. When
> the dungeon has no map, or while its lookup is pending, it shows the outdoor choice above;
> a pending lookup is retried on each position update and replaces the node when it answers.

<a id="s9-5"></a>
### 9.5 Composing the panel (Tessera rule)

The P10 root (`MapPanel`) holds only the frame, title, resize bars and buttons; `MapWindow` and
the active elements are separate top-level elements of the same layout. How the original attaches
them was not traced.

**Tessera rule:** create `MapWindow` as a child of `MapPanelPadding` (rect (5, 26, 1024, 768) in
the root, the same size as `MapWindow`), and the shown node's active element as the first child of
`MapBackground` (so that the quest indicators and the coordinate display are drawn over it). Draw
the player marker with `MinimapPlayerMarker`'s image (§8.6.3) at the player's pixel.

> **OPEN:** the original's attachment points, marker art and z-order on the world map; settle with
> owner screenshots (checklist P10-A).

> **OPEN (implementation, M5 clean room):** `tessera-ui` (`game::worldmap`) draws the marker
> on an element of its own (no layout entry) with the image, draw mode and size of the first
> attached minimap's `MinimapPlayerMarker` (no marker without one). It is the **last** child
> of `MapBackground` (over the map and its overlays), centred on the player's pixel of the
> shown map (offset by that map element's position), turned by the heading as in §8.6.3
> (object mode 1), and hidden while the shown map has no georeference.

---

<a id="s10"></a>
## 10. Quickslot bars (P3, P14)

<a id="s10-1"></a>
### 10.1 Building the slots (BIN, G30)

A `QuickslotBar` (P3 `Toolbar_Quickslot`, P14 `CtrlBar`; 23 instances over all layouts) names its
list box by `UIComponent_Quickslot_Listbox` (an element ID; `QuickslotListBox` here: a ListBox
420 × 35 with `UICore_ListBox_horizontal` true, 1 row, 12 columns). When it (re)builds:

1. It removes every item of the list box.
2. For slot index *i* = 0 … 11 (always **12** slots):
   1. creates the element `QuickslotButtonTemplate` of the `quickslots` layout (a 35 × 35 Button
      with the children `QuickslotLabelText` and `QuickslotOverlay`), with no parent;
   2. sets its `UI_Shortcut_Index` to `UIComponent_Quickslot_ShortcutOffset` + *i* (offset 0 for
      the main bar, 12 for P14's bar);
   3. sets `QuickslotLabelText`'s text from the key-binding table for that shortcut index (Probable,
      G37: the key bound to the slot's command), or clears it when there is none;
   4. creates the slot's content element for the shortcut in that slot (§10.3) and adds it to the
      button;
   5. inserts the button into the list box at index *i*;
   6. hides the button if the slot's shortcut is empty (§10.2).

The button's own art is only `QuickslotOverlay`'s rollover, pressed and highlight images (its
`normal` state has no image); the label is white with an outline.

> **OPEN:** the text of `QuickslotLabelText` (G37, Probable). The client takes it from a
> key-binding lookup by shortcut index, but how a binding becomes text (key names, modifier
> notation, abbreviation, which of several bindings) was not traced. **Tessera rule** until
> settled: the label shows Tessera's own short name for the first key bound to the slot's
> action (`keymap.md`), or is empty.

> **OPEN (cross-spec):** the list box's item arrangement (critique A2). Tessera places button *i*
> at (35·*i*, 0) inside a horizontal 1 × 12 list box, which fills its 420-pixel width exactly;
> vertical bars (P14's orientation toggle, `UIComponent_Quickslot_HorizontalOption`) are not
> needed for M5.

<a id="s10-2"></a>
### 10.2 Empty slots (BIN, G31)

- A slot whose shortcut is empty is **hidden** after a rebuild.
- When the bar receives the UI message `Element_drag_global_begin` (any drag starts anywhere,
  UB), it shows every empty slot; on `Element_drag_global_end` it hides them again (unless a
  per-bar counter that this session did not identify is non-zero).

So with no shortcuts, a bar draws **no slots** in normal play. P14's background elements
(`Quickslots_BG_Horiz`) carry no image, so an empty extra bar draws nothing at all outside the edit
mode of §4.

**Tessera rules:** the sandbox's slots are empty unless Tessera binds its own actions to them;
empty slots follow the rule above (shown during a drag), with the counter below taken as zero.

> **OPEN:** the per-bar counter that, when non-zero, keeps empty slots shown at
> `Element_drag_global_end`: what it counts and which messages raise and lower it (the test was
> read in the message handler; the writers were not identified). **Tessera rule** until settled:
> the counter is always zero, so empty slots hide at every drag end.

<a id="s10-3"></a>
### 10.3 Slot content (BIN, G32)

The content element depends on the shortcut's type (the shortcut-type values are those of the
data enum mapper of shortcut types, LPA §15 table and Appendix A.1; that the client's content
dispatcher uses the same values is Probable, see the OPEN below): an empty shortcut gets
`QuickslotEmptyTemplate` (a 35 × 35 Field with one image, draw mode 4); other types get their own
templates of the `quickslots` layout (`QuickslotAliasTemplate`, `QuickslotEmoteTemplate`,
`QuickslotHobbyTemplate`, `QuickslotPetOneShotTemplate`, `QuickslotPetAssistTemplate`,
`SkillQuickslotDisplayTemplate`, and item displays). The empty template's image is what an empty
slot shows while visible (§10.2).

> **OPEN:** the enumeration of shortcut types the content dispatcher switches on (Probable: the
> shortcut-type mapper of LPA §15), the mapping of each type to its template, and how skill and
> item icons are drawn into them (needs item and skill data; outside the sandbox).

---

<a id="s11"></a>
## 11. The other game types in the panels (G33)

None of these types hides the element in its constructor or its post-creation routine (BIN,
checked for a set-visible call in both); their content beyond the layout's images and text is game code (UL §17.5 item
(5)). Tessera's fallback widget draws the layout's images and text and the listed sandbox content.

| Type | Panels | What game code adds | Tessera |
|---|---|---|---|
| `UIScaler`, `UIScalerPanel` | P3, P6–P10, P12, P13 | UI scaling of the subtree (UB §9) | UB §9 |
| `MoodDisplay` | P1 | State from the mood value | Default state (§8.7.2) |
| `TimeOfDayDisplay` | P1 | §8.7.3 | §8.7.3 |
| `MapFogOfWar` | P1 | §8.5.3 | Draws nothing |
| `HousingPanelButton`, `HousingDecorationButton` | P1 | Housing shortcuts | Hidden (Tessera rule: no housing) |
| `LevelProgressMeter` | P3 | Experience fill (engine Meter, A2) | Fixed sandbox value |
| `AutoAttackButton` | P3 | Combat toggle | Layout default state |
| `TargetViewport` | P4, P5 | 3D portrait | Optional portrait (UL §17.4) |
| `PropertyDisplay`, `RankInsigniaDisplay`, `ClassDisplay`, `MultiVitalDisplay`, `EffectDisplayList` | P4, P5 | Values, icons and effect lists from the character | Name, level, full meters; no effects |
| Two medal icons (`BigBattlesQuestMedalIcon`) | P5 | Medal art | Hidden (Tessera rule) |
| `QuestTreeDisplay`, `QuestDisplay` | P9 | Quest lists | Empty |
| `QuestTracker` | P11 | Tracked quests | Empty |
| `PedigreeDisplay` | P13 | Lineage values | Sandbox profile or empty |
| `RaidLock`, `ChestLock` (one each) | P8 | Raid and chest lock state | Hidden (Tessera rule) |

> **OPEN:** the drawing of each of these types beyond images and text (UL §17.5 item (5)), and
> whether the original hides the housing buttons, medal icons and lock icons in normal play.

---

<a id="s12"></a>
## 12. Invariants

Data invariants (checkable against the install; values are this install's):

- **GP1** Every instantiated `HiddenDragBox` has the three children of §4 and the four state
  properties naming `hidden`, `visible`, `dragging`, `visible_mouseover` (71 of 71).
- **GP2** Every dungeon record has `Dungeon_MapData` naming a 0x78 property set with
  `UI_Map_GameMap` whose `UI_Map_MapImage` exists (1,303 of 1,303).
- **GP3** The `MapWindow` element's `UI_Map_AreaData` tree has 227 nodes; every node's
  `UI_Map_ActiveElement` is a distinct top-level element of the `map` layout; every area DID in any
  `UI_Map_AreaDIDs_Array` occurs in exactly one node (541 areas).
- **GP4** `MinimapViewport`'s image has alpha 0 at its four corner pixels and 255 at its centre.
- **GP5** `RadarMapImage_Template` is 200 × 200 and `RadarMapImage` is 600 × 600 with 3 rows and 3
  columns.
- **GP6** `QuickslotButtonTemplate` is 35 × 35 and each main-bar list box is 12 × 35 = 420 pixels
  wide.

Behaviour invariants (for Tessera's implementation and synthetic fixtures):

- **GP7** In normal play (toggle off) no `HiddenDragBox_DragBox`, `HiddenDragBox_DragBar` or
  `HiddenDragBox_TitleText` is drawn, and each drag-box root is in state `hidden`.
- **GP8** No `TutorialHighlighter` and no `PipDisplay` is drawn in the sandbox.
- **GP9** For the P4 fields with an absent `LevelScaling_ScaledLevel`: `PlayerLevel_HWField`
  visible, `PlayerScaledLevel_HWField` hidden.
- **GP10** Outdoors, the player's map pixel after scrolling lands on the viewport centre (85, 85)
  exactly, for any position for which §8.6.1 finds a georeference (not the empty range of §8.4.1,
  and not a grid in which no tile has a usable `UI_Map_GameMap`).
- **GP11** `Mood_Level` *m* maps to the state of §8.7.2; the mapping is monotone and saturates at
  ±10.
- **GP12** An empty quickslot bar draws no slot buttons except between a drag begin and its end.
- **GP13** (data invariant) Every `HideWhenField` that has `UI_HideWhenField_PropertyCompare_Value`
  compares a property of PS type 15, 8 or 21 (19 of 19 here: 14, 2, 3), and each compared
  property's dictionary default is absent, 0 or false.
- **GP14** In the hide test, a boolean or enum *P* with operator 2 or 3 compares the stored 32-bit
  values; with operator 1, 4, 5 or 6 a boolean never hides and an enum compares its keys as signed
  32-bit integers (§6.2).
- **GP15** With a game UI attached, no press of Escape quits the client, whatever is focused or
  active (§16).
- **GP16** With a game UI attached, no on-demand panel shown and P6 hidden, a declined Escape
  shows P6 as the active panel; the next declined Escape hides it again. With an on-demand
  panel active, a declined Escape hides that panel and leaves every HUD panel (P1–P5, P11,
  P14) and every plugin window as it was (§16).
- **GP17** In P1 as Tessera composes it (§8.7.2, "How P1 splits into surfaces"), every pixel of
  `MinimapViewport`'s disc and of the visible map element (tile grid or indoor map) is in
  `GamePlay_Radar`'s canvas below, none in a canvas above; a tile pixel drawn over a disc
  pixel of alpha 0 stays transparent, and one over alpha 255 keeps alpha 255 (draw mode 5).
  The canvases above of the rim and of `MinimapPlayerMarker` are empty, the marker's at every
  heading.

---

<a id="s13"></a>
## 13. Worked examples (synthetic)

<a id="s13-1"></a>
### 13.1 Outdoor minimap placement

Player in region 1 at landblock (100, 150), block-local (50, 80), outdoors.

1. Range (§8.4.1): *x* 99…101, *y* 149…151.
2. Items (§8.4.4): row 0 is *Y* 151, row 1 is *Y* 150, row 2 is *Y* 149; column 0 is *X* 99. The
   player's block (100, 150) is item 4 (column 1, row 1).
3. Transform (§8.6.1) from the item at row 2, column 0 (block (99, 149)): *px*<sub>0</sub> = 0,
   *py*<sub>0</sub> = 600, *s* = 1.25. *G* = (16,050, 24,080); *dx* = 16,050 − 15,840 = 210,
   *dy* = 24,080 − 23,840 = 240. *px* = 0 + trunc(262.5) = 262; *py* = 600 − trunc(300.0) = 300.
   (Check with the player's own tile: (50 · 1.25 + 200, 200 − 80 · 1.25 + 200) = (262.5, 300).)
4. Scrolling (§8.6.2): `RadarMapImage` moves to (85 − 262, 85 − 300) = (−177, −215) inside the
   viewport. The marker stays at (71, 71).

<a id="s13-2"></a>
### 13.2 Grid at the world edge

*X* = 0: *x*<sub>min</sub> = 0, *x*<sub>max</sub> = 1, difference 1, *x*<sub>min</sub> = 0 → *x*<sub>max</sub> = 2:
columns 0, 1, 2; the player is in column 0. *X* = 253: 252…254. *X* = 254: *x*<sub>min</sub> = 253,
*x*<sub>max</sub> = 254, difference 1, *x*<sub>min</sub> ≠ 0 and *x*<sub>max</sub> = 254 → *x*<sub>max</sub> = 252:
empty range.

<a id="s13-3"></a>
### 13.3 Content layer

A block with base image *I*<sub>0</sub> and `UI_Map_MapImageArray` = [{layer 1, *I*<sub>1</sub>},
{layer 0x10000123, *I*<sub>2</sub>}]: *L* = 0 → *I*<sub>0</sub>; *L* = 0x10000123 → *I*<sub>2</sub>;
*L* = 1 → *I*<sub>1</sub>; *L* = 0x10000999 → *I*<sub>1</sub> (layer-1 fallback). With the array
[{layer 0x10000123, *I*<sub>2</sub>}] and *L* = 0x10000999 → *I*<sub>0</sub>.

<a id="s13-4"></a>
### 13.4 Indoor placement

Map with *bx* = 10, *by* = 20, *px*<sub>0</sub> = 400, *py*<sub>0</sub> = 300, *s* = 3.0, image
800 × 600. Player at *G* = (1,610.5, 3,205.25): *dx* = 10.5 → trunc(31.5) = 31 → *px* = 431;
*dy* = 5.25 → trunc(15.75) = 15 → *py* = 285. `MinimapMapImage` (resized to 800 × 600) moves to
(85 − 431, 85 − 285) = (−346, −200). With *dx* = −0.2: trunc(−0.6) = 0, so *px* = 400.

<a id="s13-5"></a>
### 13.5 Drag box

Toggle off: children hidden, state `hidden`. Toggle on: children shown, state `visible`. Pointer
enters the drag bar (`Element_mouse_over_top` value 1): `visible_mouseover`. Mouse press on the
box: `dragging` (dragging wins over mouse-over). Release: back to `visible_mouseover`. Toggle off:
reset, `hidden`, children hidden. Each switch is passed to `HiddenDragBox_DragBox`,
`HiddenDragBox_DragBar` and `HiddenDragBox_TitleText` (§4.5).

<a id="s13-6"></a>
### 13.6 Hide test

A field with operator 6 and value [*P* = 5]: hides when 5 > *B*. Player value 3 → hidden; 5 →
visible; *P* absent → *B* = 0 → hidden. Operator 0 → never hides.

*P* a boolean, value [*P* = true]: operator 3 hides when the player's *P* is true; *P* absent with
a dictionary default of false → *B* = false → visible. Operator 2 hides when the player's *P* is
false or absent. Operator 6 never hides, whatever *B* is.

*P* an enum, value [*P* = key 7]: operator 3 hides when the player's key is 7; operator 2 hides for
any other key, including the default 0 when *P* is absent; operator 6 hides when the player's key,
read as a signed 32-bit integer, is less than 7.

<a id="s13-7"></a>
### 13.7 Escape (§16)

Synthetic sequence; game UI attached, HUD shown, nothing else open, nothing focused.

1. Escape: there is no keyboard target, so `EscapeKey` reaches game code. No on-demand panel is
   active and P6 is hidden, so rule 2 shows P6 and makes it the active panel.
2. Escape: P6 is the keyboard target and does not consume `EscapeKey` (assumed for this
   example); P6 is an active on-demand panel, so rule 1 hides it.
3. P9 is opened and becomes active; a text field in it without `lose_focus_on_escape` gets
   focus. Escape: the field declines it (UB §5.4); P9 is the active panel, so rule 1 hides P9.
4. The chat window's entry line gets focus and Escape is pressed: the chat window consumes it
   (`chat-window.md` §6.2), so this rule does not run.
5. Escape again: the entry no longer has focus and the chat window does not handle it; the
   active panel is the chat window, a HUD panel, so rule 1 does not apply, and P6 is hidden, so
   rule 2 shows P6.

At no step does the client quit (GP15).

---

<a id="s14"></a>
## 14. Corrections to, and agreements with, other documents

| Document | Statement | This spec |
|---|---|---|
| UL §17.5 item (1) | Drag-box state in normal play OPEN; Tessera choice: frame hidden, shown in an edit mode | **Answered** (§4): hidden in normal play; the edit mode is the `ToggleHiddenDragBoxes` input action; states as §4.2. The Tessera choice agrees |
| UL §17.5 item (4) | Minimap and world-map images OPEN | **Answered** for the minimap (§8) and partly for the world map (§9; first-map choice Reported, attachment a Tessera rule) |
| UL §17.4 P1 | "Minimap image: OPEN … Fog of war, mood, housing: not provided; those children are hidden by Tessera" | Draw the tiles (§8.4) or dungeon map (§8.5). Do **not** hide the rim: `Minimap_Frame` carries the rim art and is driven by mood; use state `Mood_0` (§8.7.2). Mood gauge: default state. Fog: draws nothing |
| UL §17.4 P10 | "Map choice: OPEN … Tessera shows the panel without a map image" | Show the node of §9.4 |
| UL §17.4 P3 | "Slots: Tessera's own actions or empty" | Empty slots are hidden (§10.2), not drawn as empty frames |
| `m5-visual.md` P14-A | "Bar frame and empty slot frames as the bar lays them out" | With no shortcuts and the toggle off, P14 draws nothing; its frame appears only in the edit mode (§4, §10.2). The checklist row should judge the edit-mode frame, or slots during a drag |
| `m5-visual.md` P1-A | "judge the rim, buttons and frame only" | The map image is now specified (§8); the rim's clipping is OPEN (§8.7.2) |
| LB §6.2 | `UI_Map_FogOfWar` listed as int, always 0 | Its type is boolean (PS type 21); false in every landblock record (same value). True in 670 dungeon maps (§8.5.2) |
| `walker.md` §5 | "Escape quits" | Holds only without a game UI. With a UI attached, a declined Escape follows §16 and never quits (decision 0014, Consequences "Escape with a UI attached") |
| Decision 0015, Decision 1.7 "Clip" (last paragraph) and P1 checks 2 and 4 under §8 | The P1 marker is the only named nested entry of the M5 panels; a painter test finds rim pixels outside the viewport box "in the root canvas" (check 2); while turning, "the canvas above holds the same rim pixels at every angle" (check 4) | **Correction (data, G40, G41, UL §17.2):** P1 has 18 non-root surfaces at two levels; the rim owns a surface and its pixels are in its own canvas; the map and disc are in `GamePlay_Radar`'s canvas. The clip result is unchanged (root only). Check 4: the marker's canvas above is **empty** at every heading (the rim and everything after the marker in the radar's drawing order own surfaces); the 40 × 40 coverage region and the absence of damage while turning still hold. Test as in §8.7.2 "How P1 splits into surfaces". For the decision's next revision (the record is not edited here) |
| `walker.md` §6 | Walker minimap: ±320 m, tiles of resident blocks | Agreement on the tiles and the layer; the native P1 panel shows a fixed 3 × 3 grid at 1.25 px per unit and is not the walker's debug minimap |

---

<a id="s15"></a>
## 15. Open questions (summary)

1. §4.5: state propagation beyond direct children.
2. §6.2: account and player tests behind the `HideWhenField` flags other than the comparison.
3. §7: when game code shows a `PipDisplay` and the page changer; the page indicator text.
4. §8.4.4, §10.1: ListBox arrangement (critique A2).
5. §8.5.1: the game-script rule that picks the dungeon map.
6. §8.5.3: fog-of-war drawing.
7. §8.6.3: marker direction on screen and rotation resampling (Tessera rule from decision 0015).
8. §8.7.2: whether the rim is clipped to the viewport (the rim owns a surface by its layout mode, G39; Tessera rule agrees with decision 0015).
9. §8.7.4: the coordinate-label rule; §8.7.5: the `MapZoom` event.
10. §8.8: note blips.
11. §9.4: world-map choice outside listed areas; dungeon maps in the world-map window.
12. §9.5: attachment of `MapWindow` and the active element; world-map marker.
13. §10.3: the shortcut-type enumeration of the content dispatcher; shortcut type → content template; icons.
14. §11: drawing of the remaining game types; whether housing, medal and lock icons are hidden.
15. §6.2: comparison of the value types that use their own comparison routine (codes 1–4, 6, 7,
    11–14, 16, 17, 19, 22); no field in the data compares one. Int, enum, float, boolean,
    tri-state, property-ID, 32-bit-bitfield and DID comparisons are settled (G38).
16. §8.5.3: the condition that enables `MapFogOfWar`.
17. §8.6.1: radar behaviour when no tile item yields a georeference (empty range, no `UI_Map_GameMap`).
18. §8.7.3: the two internal flags of `TimeOfDayDisplay` that force `Unknown_Time`.
19. §10.2: the per-bar counter that keeps empty slots shown at a drag end.
20. §4.3: the drag bar's tooltip text and when it appears (G34).
21. §8.7.2: what drives the `Mood` gauge (G35).
22. §9.2: the effect of a click on a child-map button, and the 3 buttons whose target is no node's active element (G36).
23. §10.1: how a key binding becomes the `QuickslotLabelText` text (G37).
24. §16: what the original's game code does with an `EscapeKey` that the UI declined (Tessera rule from decision 0014).

---

<a id="s16"></a>
## 16. Escape with the game UI attached (Tessera rule)

With a game UI attached, Escape gives the action `menu`, which carries the command `EscapeKey`
(DEC: IB D2). The UI is offered it first (UB §5.3): a focused text field with
`lose_focus_on_escape` gives up focus and consumes it (UB §5.4), an engine confirmation dialog
that is the active panel closes with its "no" result (UB §5.5), and the chat window, when its
entry has focus, releases that focus and clears the entry (`chat-window.md` §6.2). A command that no keyboard target consumes goes on
to game code and to the plugins' key broadcast (UB §5.3 step 3, LPA §13.13). This happens
often: only 19 elements set `lose_focus_on_escape` while 103 are editable (UB §5.4), a focused
plugin text box does not consume Escape under the Tessera rule of LPA §13.13, and an active
panel that does not handle `EscapeKey` declines it.

> **OPEN:** what the original's game code does with an `EscapeKey` that reaches it: which panels
> it closes and in what order, whether it opens the main menu, and whether HUD panels or plugin
> windows react. Not traced (game code). Owner observation: in the original, press Escape with
> (a) nothing open, (b) the main menu open, (c) the quest log open and active, (d) a plugin
> window active.

**Tessera rule** (DEC: IB, Consequences "Escape with a UI attached"; it never quits). When a UI
is attached, the client treats the **press** of `menu` that the UI did not consume as game
code's handling of `EscapeKey`; its release and its auto-repeats do nothing. The **on-demand
panels** are P6, P7, P8, P9, P10, P12, P13 and P15 of UL §17.2; the HUD panels P1–P5, P11 and
P14 and all plugin windows are never hidden by this rule.

1. If the active panel (UB §5.1) is an on-demand panel, hide it. Done.
2. Otherwise, if P6 (main menu) is hidden, show it (UL §17.1) and make it the active panel;
   if P6 is shown, hide it.

Without a UI attached (the walker of M3 and M4) `menu` keeps its meaning of `walker.md` §5
(quit). How the client is quit while a UI is attached is outside this rule; the walker's help
line must not label Escape "quit" then (IB, Cost).

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First version (critique item A9): drag-box frames (answers UL §17.5 (1)), tutorial highlighters, `HideWhenField` rule with operators, pip displays and page changer, the minimap's outdoor tile grid, content-layer choice, indoor dungeon map, placement, disc, rim mood states, time of day, labels; the world-map tree, georeference and choice (answers UL §17.5 (4) in part); quickslot slots, empty-slot rule and content templates; the remaining game types; invariants GP1–GP12; examples §13.1–§13.6; corrections to UL §17.4, the M5 checklist and LB §6.2. OBS and BIN (client 4906.0071.1794.4035). |
| 2026-10-06 | Revision 2 after the second review. G7 corrected after an own recount over all layouts and the property dictionary: the 19 compared properties are 14 int, 2 enum and 3 boolean (it read 16 int and 3 boolean), and their dictionary defaults are recorded. New row G38 (BIN, q4 re-read): §6.2 now has a table of how each value type compares under operators 2 and 3 and under the ordering operators (enums and ints signed 32-bit; booleans and other 32-bit types equality only, ordering never hides; NaN handling for floats). It also states *B* when the player lacks *P* (dictionary default, else the type's zero value). The §6.2 OPEN and its Tessera rule now cover only the types with their own comparison routine; §15 item 15 updated. New invariants GP13 (data) and GP14, and §13.6 boolean and enum examples. New in-section OPEN callouts with Tessera rules for the Probable statements G34 (§4.3), G35 (§8.7.2), G36 (§9.2) and G37 (§10.1); §15 items 20–23. G36 now records what the 3 buttons that name no node point to. The §6.3 sandbox results are unchanged. Section numbers unchanged. |
| 2026-10-06 | Revision 1 after the first review. Added in-section OPEN callouts to §6.2 (other value types, with a Tessera rule for booleans and the G7 census of compared types), §8.5.3 (fog enable condition), §8.6.1 (no georeference, with a Tessera rule), §8.7.3 (the two `Unknown_Time` flags), §8.7.4 (coordinate-label rule), §8.7.5 (`MapZoom` event) and §10.2 (per-bar counter); §15 items 15–19 and item 13 extended. §9.2: node count corrected to "196 of 227 nodes have buttons to nodes that are not their tree children (199 to any other node)" (own recount 2026-10-06). §10.3: the shortcut-type reference now points to LPA, not KM §12. GP10 restricted to positions with a georeference. New provenance rows G34–G37 (Probable) for §4.3, §8.7.2, §9.2 and §10.1 step 3; G7 extended. Section numbers unchanged. |
| 2026-10-06 | **Revision 3: applies the accepted decisions 0014 (UI input bridge) and 0015 (UI rendering)** (status Accepted → Revised, review pending). No fact about the original is changed. New provenance tag DEC. **New §16** (Tessera rule from decision 0014, Consequences "Escape with a UI attached"): game code's reaction to a declined `EscapeKey` with a UI attached (hide the active on-demand panel, else toggle the main menu P6; never quit), with an OPEN on the original's behaviour and an owner observation; §0 table, invariants GP15 and GP16, example §13.7, §14 row for `walker.md` §5, §15 item 24. **§8.6.3:** Tessera rule from decision 0015 §2.3 and Decision 1.7 next to the OPEN (the marker is a separate entry rotated about its centre to the stated screen result, resampled with the surface filter, a fixed 40 × 40 coverage square, own opacity only). **§8.7.1:** the composition uses the surface alpha by the UL §9.6.1 Tessera rule (decision 0015), replacing the reference to an earlier Tessera choice. **§8.7.2:** OPEN extended with the data fact G39 (`Minimap_Frame` has object mode 1, so it owns a surface) and the agreement of decision 0015's nested-surface clip with the rim rule. **§8.2 step 3:** the layout already gives the marker mode 1 (new row G39, OBS 2026-10-06 via UL U46). §15 items 7 and 8 updated. Section numbers unchanged; §16 and §13.7 are new. |
| 2026-10-06 | **Fixes after the review of Revision 3** (status stays Revised, review pending). **§16 rule 1:** the active panel is defined in UB §5.1 (activation), not UB §4.6; citation corrected. **§8.7.2:** new paragraph "How P1 splits into surfaces" (DEC: UR Decision 1.1, 1.7): `GamePlay_Radar` and 17 other non-root P1 elements own surfaces, so the disc and map are painted into the radar's canvas and the rim into its own; from the data (new row G40, OBS 2026-10-06, Confirmed) the disc and every map element come before every surface-owning element in drawing order, so none of their pixels goes to a canvas above and draw mode 5 keeps finding the disc's alpha; the rim's box extends 7 and 6 pixels past the root, which the root clip cuts; a replacement for decision 0015's P1 check 2. New invariant GP17; §14 row on decision 0015's "only nested entry" sentence and P1 check 2 (for the decision's next revision). No fact about the original is changed. |
| 2026-10-06 | **Final-pass fixes** (status stays Revised, review pending). No fact about the original is changed. **§8.7.2 "How P1 splits into surfaces":** the Test bullet now also corrects decision 0015's P1 check 4 (the marker's canvas above is empty at every heading and holds no rim pixels; the 40 × 40 coverage region and no damage while turning still hold); the Clip bullet states `GamePlay_Radar`'s offset (0, 33) in the root, the rim's root rows, and that the root's bottom cut removes no pixel. New provenance row G41 (Confirmed 2026-10-06, read-only probe of the resolved P1 tree). GP17 states that the rim's and the marker's canvases above are empty. §14 decision-0015 row extended to check 4. |
