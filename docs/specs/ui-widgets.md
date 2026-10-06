<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI engine widget classes: GroupBox, Meter, Scrollable, Scrollbar, ListBox, Tree, Panel, Menu and Viewport

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | Review 2026-10-06, Claude review session (spec-writer agent): **changes requested; status stays Review.** Three blocking items, four minor. **Blocking 1** (§5.3 "unknowns are marked OPEN"; implementability of P8-S): the social panel P8 resolves to 2 GroupBox (type 1) instances and 4 Menus. §13 row P8-S names neither. No section says what a GroupBox does: §16 row 1 lists GroupBox, ColorPicker and Browser as "remain open", but there is no in-section `> **OPEN:**` callout, and §17 does not list them. Add an OPEN callout with a Tessera rule (for example: a GroupBox behaves as the base element) for GroupBox, ColorPicker (6 instances) and Browser (9), add it to §17, and name the GroupBoxes and the closed Menus (§11) in row P8-S. **Blocking 2** (§5.3 "invariants concrete enough to test"; §7.3 against WI12): steps 1 and 2 do not say whether "if *S* is false, stop here" applies only when *H* was changed in that step, or always. WI12's case "(false, true) + `normal` gives (false, false)" holds only under the first reading. The second reading stops at step 2 and gives (false, true). State the scope explicitly (and check it against the BIN trace); tab switching (§10.3) and list selection (§8.4) depend on it. **Blocking 3** (§5.3 "each fact has a provenance entry"): the §13 panel facts have no provenance row: the P3 child meters of `LevelProgressMeter`, the P4/P5 directions and draw modes, P7's 11 tabs and the Graphics page opening, the 16 × 16 slider thumb, P8's default pages, and P12's composition. Rows W11 and W18 are BIN-only, but they also carry OBS census facts: §7.2 (1,514 arrow buttons with 0.5 s / 0.1 s) and §11.4 (195 menus, `auto_close_on_selection` false on 14, …). Add an OBS row (Confirmed, 2026-10-06) covering them. **Minor:** (a) §10.5: "all 286 tab and page IDs" counts page entries. There are 286 entries, so 572 IDs (286 tabs + 286 pages), all of which are found as descendants. Reword. (b) §6.6 uses *s* for the thumb's start, which clashes with the scroll offset *s* of §5. Define it as the thumb's position on the bar's axis. (c) §5.7 says the pop path of `ScrollableControls_Mouse` "was not traced step by step" but has no OPEN callout and no §17 entry. (d) §9.3 step 1: make it clear whether `indent_amount` is also added in the "otherwise" branch (parent's entry *x* plus indent part *x*). Say that the width rule (depth × `indent_amount`, with depth 1 for top-level nodes) deliberately reserves an indent that placement does not apply to top-level entries. Also, in §8.3's vertical wrap, say whether "the column's width" is the cell width under `ignore_cell_alignment`. **Passes:** the facts are functional. The algorithms are prose with numeric constants allowed under decision 0012, with no code or listing structure and no copied prose. Labels are interop identifiers under the naming note; the §13 page names match element-ID labels. There is no game text, key, hostname or traffic. Only offline UI code was read, so no closed gate applies. The BIN and Probable marks follow the Accepted UL and UB specs. Every worked example in §15 recomputes by hand: 15.1 geometry, 15.2 frames 30/58/59, 15.3 track (10, 10, 0, 437), *L* 427, thumb 154, *a* 10/146/283, line step 30, *q* 0.03911, drag 0.18315 → (183, −1) → *sy* 140, 15.4 stops 37 / 0.37374 and 0.625, 15.5 *Cw* 420. WI3 and WI5 follow from the formulas. **Spot checks** were made read-only on the install, 2026-10-06, with the reviewer's own resolver and own dictionary reader (not the writer's pickles). All of them match: (1) 83,161 resolved instances; Meter 882, ListBox 582, Panel 105, Tree 116, Scrollbar 792, Menu 195, Viewport 1, `TargetViewport` 35; types 3, 6, 13 and 19 are absent (W2, §3.3). (2) §4.8: modes child 379, frame 402, alpha-test 4, none 97, with no combination; directions 311/51/17; draw modes 226/153; `move_fill` 19/271/89; `offset` set on none; smooth movement 17 at 0.3 s, 24 without a duration, 1 false; frame arrays 399 × 60 + 3 × 30 = 24,030 frames (WI14 count), all with DID prefix 0x41. (3) §6.10, W21 and WI13: 792 of 792 bars have a 0x16 child and none has a fill part; 152 are horizontal; `proportional` 765/11/16; `min_widget_size` 20 on 776; `hide_when_disabled` 780/11; slider mode 27; stops 100 with position 0.5 on 8; 776 have `smooth_movement` with no duration; buttons 757/16/19. (4) §7.2: all 1,514 direct-child arrow buttons are Buttons with `hot_button` true, 0.5 s and 0.1 s. (5) §2.2 and §12 dictionary defaults from an independent parse of 0x34000000 that ends exactly at the record end: every listed value matches, including the Viewport minimums and maximums. The only other UICore Meter/Scrollbar/Scrollable/ListBox/Tree/Panel/Menu/Button default is `UICore_Button_click_action`, which this spec does not use. (6) §10.5: 64 of 105 panels have a page list (1–18 pages); `page_open` is set on 1 page in 60 panels, 0 in 2 and 2 in 2. §13 P3, P4 and P5 meter directions, draw modes and `move_fill` match. **Revision 1 (spec writer, 2026-10-06): submitted for review.** Every blocking and minor item above is addressed; see the change log. **Re-review 2026-10-06, Claude review session (spec-writer agent), a different session from the author: passed; status moves to Accepted.** **Blocking 1 resolved:** §3.4 specifies GroupBox behaviour (selection change, completion-pass default, clicks and the stop answer) with census W25; §3.5 carries an in-section OPEN with a Tessera rule (base-element behaviour) for ColorPicker and Browser, listed as §17 item 12; §13 P8-S names the four Panels and their default pages, the GroupBoxes `FriendsHeader` and `IgnoreHeader`, and the four closed Menus. **Blocking 2 resolved:** §7.3 now states that the "*S* false" stop applies only when that step changed `highlighted` (W26). WI12 and §15.7 recompute under that rule: (false, true) + `normal` gives (false, false); (true, true) + `normal` gives (false, true); (true, false) + `ghosted` gives (true, true) with *S* false and (false, true) with *S* true. §15.8 also follows from it. **Blocking 3 resolved:** W22 covers the §13 panel composition and W23 the censuses of §7.2, §8.3, §10.5 and §11.4, both OBS and Confirmed; W11 and W18 point to W23. **Minor items (a)–(d) resolved:** §10.5 gives 286 entries and 572 IDs; §6.6 uses *a* and *len*; §5.7 has an OPEN for the pop path (§17 item 13); §9.3 adds the indent in every case and explains the width reserve, and §8.3 says the cell is used only by the horizontal steps. §15.9 and §15.10 recompute by hand: widths 124 and 122; entries at 16 and 24; track (16, 16, 306, 0), *L* 290, *a* 153, *px* 76, value 0.27839. WI17 holds for depth 3 and for parents without an icon. The new text is prose with numeric constants (decision 0012). Its element-ID labels are interoperability identifiers under the naming note; it contains no game text. **Spot checks** of the new Confirmed facts, made read-only on 2026-10-06 with the reviewer's own tree builder and descendant lookup (not the writer's probes), all match: (1) W25: 83,161 instances; GroupBox 26, ColorPicker 6, Browser 9; `default_button` set on 20, each naming a Button found by the descendant lookup; `allow_button_click_on_selected_button` true on 6 and absent on 20; `selected_button` set on none; 25 GroupBoxes hold 2–12 descendant Buttons and 1 holds none. (2) W23 Menus: 195, all with `popup_layout`; 194 with a rollover state; `auto_close_on_selection` false on 14 and absent on 181; `open_down` set on 3; `open_center` true on 2; `default_selected_item` and `default_no_selection_text` set on none. (3) W23 ListBox flags: `ignore_cell_alignment` 1 (false), `fill_in_reverse` 12 (true 8), `flipped_layout` 1 (false), `wrap_indentation_amount` 1 (6). (4) W22 P8: 4 Panels; `Social_Panel` has 5 entries and only the grouping entry opens; `GroupingPanel`, `FriendsPagePanel` and `RaidLocksPanel` each open the stated page; `FriendsHeader` holds 7 Buttons and `IgnoreHeader` 2, both defaulting to the member column with allow true; the four named Menus are present, and `WhoClassMenu` and `LFQuestClassMenu` have selection display 0. (5) W22 P7 and P12: `OptionsTabPanel` has 11 entries and only the Graphics entry has `page_open`; there are 11 Trees with `indent_amount` 8, `max_columns` 1 and a vertical bar. P12 has 1 Tree, 1 ListBox, 3 Scrollbars, 1 Menu and no Panel, and its Menu has `open_down` set, `auto_close_on_selection` false and selection display 0. **Non-blocking note for the next revision:** in WI12's last sentence, "whatever the second flag is" should read "whatever `supports_ghosted_highlight` is". The pair's second flag is `ghosted`, and the case (false, true) + `normal` is given explicitly earlier in the same invariant. |
| **Implements into** | `crates/ui` (engine widget classes on top of the element tree of `ui-layout.md` §9), `apps/client` (native panels P3, P4, P5, P7, P8, P12 of `ui-layout.md` §17) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (UL: record grammar §4, properties §5.9, inheritance §7, layout §8, creation, states, drawing, Button states §9). [`ui-behaviour.md`](ui-behaviour.md) (UB: commands, capture, presses and releases, wheel delivery, messages). [`property-system.md`](property-system.md) (PS: dictionary defaults §2.2, value types §4). [`records.md`](records.md) (REC: enum mappers §7). [`fonts.md`](fonts.md) (FO: text in buttons and entries). [`keymap.md`](keymap.md) (KM §12: input-map keys such as `ScrollableControls_Mouse`). [`lua-plugin-api.md`](lua-plugin-api.md) (LPA: plugin controls that wrap these classes). Checklist [`m5-visual.md`](../checklists/m5-visual.md) rows P3-A, P4-A, P5-A, P7-A, P8-S, P12-A. |

**Naming note.** Property names (`UICore_…`), element-ID labels (`widget`, `meter_fill`, …),
state labels (`normal`, `highlight`, …), UI-message labels (`Scrollbar_new_position`, …),
element-type labels and input-command names are identifiers read from the user's own install
(the property dictionary and the enum mappers, UL §3.3). They are used here only to find the
values at run time and are interoperability identifiers. The numeric keys next to them are this
install's values; implementations look them up by label.

---

<a id="s0"></a>
## 0. Purpose

UL §9.2 step 3 creates every element through a widget factory chosen by the element's type,
and UL §5.9 leaves open what the engine widget classes do with their own properties. This spec
closes that gap for the classes that the M5 native panels need:

- **Meter** (§4): morale, power and experience bars, frame-by-frame gauges;
- **Scrollable** (§5), the shared base of lists and text: content size, scroll offsets and the
  link to scroll bars;
- **Scrollbar** (§6): track, thumb geometry, value mapping, stops, dragging, paging and the
  wheel;
- **GroupBox** (§3.4): one selected Button among several (radio sets, column headers); the
  ColorPicker and Browser classes are OPEN with a Tessera rule (§3.5);
- **Button auto-repeat and state requests** (§7), which scroll bars, tabs and list items rely on;
- **ListBox** (§8): entries from templates, the grid layout, selection;
- **Tree** (§9): nodes, indentation, expand and collapse icons;
- **Panel** (§10): pages and tabs;
- **Menu** (§11): the drop-down button and its popup list;
- **Viewport** (§12): the 3D portrait element (properties only; its drawing is OPEN).

For each class it gives the properties the class reads (by PID and label), its parts (child
elements it finds or creates by element ID), its state machine, how it lays out its parts and
how they draw, the UI messages it sends and receives, and a census. §13 maps the facts to the
checklist rows.

All counts are observations of one install on one date. Tests assert the invariants (§14), not
the counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | The owner's Steam install, read in place and read-only on 2026-10-06 with throwaway probes in the dirty-room scratch area: all 302 UI layout records (prefix 0x22) decoded with the full property values by the grammar of UL §4; every element resolved by UL §7 and every top-level element instantiated with all inherited children (83,161 element instances, the same total as UL §7.3); the property dictionary 0x34000000 decoded with its default, minimum and maximum values (PS §2.2); the enum mappers `UIElementType` (base 0x230001BC), `UIElementID` (base 0x2300013B), `UIStateID` (base 0x2300017B), `UIElementMessage` (base 0x230001AC), the meter-direction mapper 0x2300017C and the camera mapper 0x230000B9; four arrow-button render surfaces and five meter images converted to images to check orientation and size. **Revision 1 (same day, same install):** the six native panels of §13 instantiated again and walked element by element (types, sizes, properties, states, page lists); censuses of every resolved GroupBox, ColorPicker, Browser and Menu instance, of every arrow button that is a direct child of a scroll bar, of every Panel page entry, and of the ListBox fill flags. |
| BIN | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`), disassembled with capstone in the dirty room on 2026-10-06, address ranges only. **Questions:** (a) which widget class the factory creates for each engine element type, and how the classes derive from each other; (b) for Meter, Scrollable, Scrollbar, ListBox, Tree, Panel and Menu: which properties each class reads, what each one does, which parts it creates or finds by element ID, and how it lays them out (the classes' property-change handlers, post-creation routines, resize routines, frame-update routines and layout routines); (c) the scroll-bar value mapping, stops, dragging, paging and wheel handling, and how a scrollable widget keeps its scroll bars in step (the classes' message handlers); (d) the Button's auto-repeat ("hot button"), its click on release and its translation of state requests; (e) the ListBox grid layout, entry creation from templates and selection; (f) the Panel's page and tab switching. Numeric constants are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). Diagnostic strings in the executable name the meter routines; they are not reproduced. **Why M1–M3 were not enough:** the layout data gives property values and labels but no behaviour; no published source describes the engine's widget classes (the plugin API documents only the plugin surface); the original client cannot be observed running in this session, and observation could not give the arithmetic, rounding or ordering rules anyway. Only offline UI code was read; nothing about network, login, keys, launcher, patcher or anti-tamper. No address, listing or symbol appears in this spec. **Revision 1 questions (same build, read 2026-10-06):** (g) what the GroupBox class does (its factory, constructor, the methods it replaces relative to the base element, its message handler, property-change handler and its step in the pass that runs after a tree is created) and which methods the ColorPicker class replaces; (h) the exact scope of the early stops in the Button's state-request translation (§7.3); (i) whether the tree placement adds the indent amount when the parent has no icon (§9.3), and which sizes the ListBox placement uses under `ignore_cell_alignment` (§8.3); (j) the order of the descendant lookup (§2.3); (k) what the pass that runs after a tree is created does for scrollables, Trees and Buttons (§5.2, §9.1); (l) how a scroll bar treats arrow buttons that are not its descendants (§6.2, re-read). |
| UL, UB, PS, REC, FO, LPA, KM | The companion specs named in the header; facts taken from them keep their marks. |

Marks: **Confirmed** = observed in the install over every record or instance of the kind, or a
BIN fact the data agrees with; **BIN** = rests on the disassembly only; **Probable** = supported
by BIN or data with one step not traced; **OPEN** = unknown, with a callout in the section.

| # | Fact | Sources | Mark |
|---|---|---|---|
| W1 | Factory per engine type; class derivation (§3.1, §3.2) | BIN (factory registration, class constructors and their tables of methods) | BIN |
| W2 | Engine types Scrollable (3), ComboBox (6), Frame (13) and Flash (19) have no factory, so elements of these types are not created (§3.1) | BIN; OBS (no element uses them, UL §11) | Confirmed |
| W3 | How a widget reads a property: effective value, else dictionary default, else zero; "set on the element" rules (§2.2) | BIN (the property getters); OBS (dictionary defaults) | Confirmed (defaults), BIN (rule) |
| W4 | Meter modes, child-meter parts and their geometry per direction (§4.2–§4.3) | BIN (meter post-creation, part creation, part update, resize); OBS (census §4.8; image sizes) | BIN; census Confirmed |
| W5 | Frame meter frame choice and draw mode (§4.4) | BIN (frame update) | BIN |
| W6 | Alpha-test meter (§4.5) | BIN (partial) | Probable; detail OPEN |
| W7 | Meter position, goal, smooth movement and messages (§4.6) | BIN (property-change handler, frame update) | BIN |
| W8 | Scrollable content size, offsets, scroll-to clamp, scroll-bar update, line and page steps, scroll-bar message handling, wheel, and the `ScrollableControls_Mouse` push on hover (§5) | BIN (scrollable resize, property-change handler, message handler, list-box step routine, scroll-to routine) | BIN |
| W9 | Scrollbar properties, parts, track, thumb geometry, value mapping, stops (§6.1–§6.5) | BIN (scroll-bar property-change handler, layout routine, track routine, deferred button attach, stop conversions); OBS (census, arrow-button art) | BIN; census Confirmed |
| W10 | Scrollbar interaction: arrow buttons, track paging, thumb drag with the 100-pixel rule, touched-position mode, wheel, smooth movement, disabled and ghosted (§6.6–§6.9) | BIN (scroll-bar message handler, frame update, wheel routine) | BIN |
| W11 | Button auto-repeat timing, click on release, boolean toggle, state-request translation (§7.1, §7.3) | BIN (button press, release, frame update and state-request routines); OBS (dictionary defaults) | BIN (behaviour); the §7.2 census is W23 |
| W12 | ListBox properties and flags (§8.1) | BIN (property-change handler); OBS (census) | BIN; census Confirmed |
| W13 | Entries created from templates; autofill (§8.2) | BIN (entry creation routines, property-change handler) | BIN |
| W14 | ListBox grid layout: rows and columns, cell sizes, placement order, reverse and flipped fills, wrap indentation, lazy layout before drawing (§8.3) | BIN (layout routine, drawing hook, laid-out list) | BIN |
| W15 | ListBox selection and its state changes and messages (§8.4) | BIN (selection routine, message handler) | BIN |
| W16 | Tree nodes, indentation, icon parts and their states, expand on selection (§9) | BIN (tree width and position routines, expand routine, tree selection routine); OBS (icon templates use states `open` and `closed`) | BIN; data agreement Confirmed |
| W17 | Panel pages and tabs, open-page rule, tab highlight, page visibility, messages (§10) | BIN (page set-up, open-page routine, message handler); OBS (census) | BIN; census Confirmed |
| W18 | Menu popup creation, list and default item, open and close, placement, selection display (§11.1–§11.4) | BIN (menu post-creation, message handler, selection display update) | BIN (behaviour); the §11.4 census is W23 |
| W19 | Viewport properties and defaults (§12) | OBS (census, dictionary defaults) | Confirmed; drawing OPEN |
| W20 | Decrement button placed at the start of the track and increment button at the end; in data the decrement button carries the art that points toward the start | BIN (track routine); OBS (the two arrow surfaces of the common scroll-bar template decoded: the decrement button's surface points up, the increment button's points down) | Confirmed (data agreement) |
| W21 | Empty scroll-bar fill parts: no scroll bar in data has them (§6.2) | OBS (792 of 792 instances) | Confirmed |
| W22 | Composition of the six native panels of §13: P3 (the `LevelProgressMeter` element's child-meter properties and its four states that set `child_image`, its two child meters of the full fill size with direction Left, Alphablend and `move_fill` false, the glass overlay Field, the quickslot ListBox with `horizontal`, `max_rows` 1, `max_columns` 12); P4 and P5 (every meter's direction and draw mode, the five `vitalmeter_*` states, the two `TargetViewport` portraits); P7 (one Panel with 11 page entries of which only the Graphics entry has `page_open` true; 11 Trees with `indent_amount` 8 and `max_columns` 1; 11 vertical, proportional scroll bars with `min_widget_size` 20; the slider template: a horizontal slider-mode Scrollbar, not proportional, with a 16 × 16 `widget` and two 16 × 16 arrow Buttons that are its siblings; the check-box template: a Button with `boolean_button` and `supports_ghosted_highlight` true); P8 (4 Panels and their default pages, 2 GroupBoxes, 4 Menus, by element-ID label); P12 (1 Tree, 1 ListBox, 3 Scrollbars, 1 Menu, no Panel; the two info tabs are Buttons) | OBS (each panel's top element instantiated with all inherited children and walked) | Confirmed, 2026-10-06 |
| W23 | Censuses: 1,514 arrow buttons that are direct children of scroll bars, all Buttons with `hot_button` true, first interval 0.5 s and interval 0.1 s (§7.2); 195 Menus, all with a popup layout, 194 with a rollover state, `auto_close_on_selection` false on 14, `open_down` on 3, `open_center` on 2, `default_selected_item` and `default_no_selection_text` on none (§11.4); 286 Panel page entries whose 286 tab IDs and 286 page IDs are all found as descendants (§10.5); `ignore_cell_alignment` set on 1 ListBox (false), `fill_in_reverse` set on 12 (true on 8), `flipped_layout` set on 1 (false), `wrap_indentation_amount` set on 1 (6) (§8.3) | OBS (every resolved instance) | Confirmed, 2026-10-06 |
| W24 | GroupBox: plain-element class; selected-button property change, default button applied in the tree-completion pass, click handling with the stop answer (§3.4) | BIN (factory, constructor, table of methods compared with the base element's, message handler, property-change handler, tree-completion step); OBS (every `default_button` names a Button found by the descendant lookup) | BIN; data agreement Confirmed |
| W25 | Census of GroupBox (26), ColorPicker (6) and Browser (9) instances and their properties; ColorPicker replaces, among others, the base element's message handler, property-change handler and image-layer drawing (§3.4, §3.5) | OBS; BIN (table of methods) | Confirmed (census); behaviour of ColorPicker and Browser OPEN |
| W26 | Scope of the early stops of the state-request translation: a stop applies only when that step changed `highlighted` (§7.3, WI12) | BIN (state-request routine, re-traced branch by branch) | BIN |
| W27 | Tree placement adds `indent_amount` in both branches; a top-level node's width reserves an indent that placement does not apply (§9.3). ListBox placement under `ignore_cell_alignment`: only the horizontal fill uses the entry's own size (§8.3). Descendant lookup order (§2.3) | BIN (tree position and width routines, list layout routine, lookup routine) | BIN |
| W28 | Tree-completion pass: a scrollable attaches itself to its scroll bars as a listener, sets its mouse-visible flag and runs §5.2 for both axes; a Tree then sets `click_select` and `layout_only_visible` true; a Button chooses its state again (§5.1, §5.2, §7.3, §9.1). Re-read: a scroll bar adopts arrow buttons that are not its descendants on its next frame update, and the track counts a button only when the mutual nearest-lookup test holds (§6.2, §6.3); a menu with selection display 0 updates no display (§11.4) | BIN (the pass routine of the base element, the scrollable, the Tree and the Button; the top-level creation routine that runs the pass; the scroll bar's frame update, button attach and track routine; the menu's selection display update) | BIN |

---

<a id="s2"></a>
## 2. Conventions

<a id="s2-1"></a>
### 2.1 Terms

| Term | Meaning |
|---|---|
| **Part** | A child element that a widget class finds or creates by a fixed or property-named element ID and then positions, sizes, shows or hides itself. |
| **Run-time property** | A property set by code at run time; it overrides the state and own properties (UL §9.4 step 5) and is reported to the class like any change. |
| **Box**, **W**, **H** | The element's current inclusive box (UL §8.1); *W* and *H* are its current width and height in pixels. |
| **Move**, **resize** | Setting an element's position (relative to its parent) or size at run time. A resize lays the element's children out again (UL §8.4). |
| **Show**, **hide** | Setting the element's visibility (UB §11.2). |
| **Send message *m*** | Deliver UI message *m* (`UIElementMessage`, §2.4) to the element's own handler and listeners (UB §12.1). |
| **Frame update** | A per-frame callback an element registers for itself while an animation or a repeat runs, and removes when it ends. |
| **now** | The UI clock in seconds (64-bit float). |
| trunc(*x*) | *x* rounded toward zero to an integer. |
| half(*n*) | *n* ÷ 2 rounded toward zero (integer). |

<a id="s2-2"></a>
### 2.2 Reading a property (BIN, W3)

When a class reads a property it takes, in order:

1. the element's **effective** value (UL §9.4 step 5: run-time, then current state, then own);
2. otherwise the **dictionary default** of the PID (PS §2.2), if the definition has one;
3. otherwise zero, false or "none" for the type.

Some rules apply only when the property is **set on the element**, meaning step 1 found a
value; a dictionary default does not count. Each such rule says so. The dictionary defaults
that matter here (OBS, 2026-10-06; read them from the install):

| PID | Label | Default |
|---|---|---|
| 0x06DF | `UICore_Scrollbar_widget_size` | 1.0 |
| 0x0170 | `UICore_Scrollable_keep_in_bounds` | true |
| 0x00BE | `UICore_ListBox_item_normal_state` | 0x0B (`normal`) |
| 0x0120 | `UICore_ListBox_item_selected_state` | 0x07 (`highlight`) |
| 0x01B9 | `UICore_ListBox_horizontal` | false |
| 0x0343 | `UICore_ListBox_layout_only_visible` | false |
| 0x0759 | `UICore_Tree_indent_elements_for_icon` | true |
| 0x047B | `UICore_Menu_auto_close_on_selection` | true |
| 0x00C8 | `UICore_Button_hot_click_interval` | 0.05 s |
| 0x05AE | `UICore_Button_hot_click_first_interval` | 0.2 s |
| 0x03CB | `UICore_Meter_notify_on_new_position` | false |
| 0x0519 | `UICore_Scrollbar_horizontal_allow_mouse_wheel` | false |

Every other property named in §4–§11 has no dictionary default, so it reads as zero or false
when absent.

<a id="s2-3"></a>
### 2.3 Finding parts by element ID (BIN)

Three lookups are used:

- **direct child**: a child of the element with the given ID;
- **descendant**: the first element with the ID in the element's subtree, searched in this
  order (BIN, W27): first the element's direct children in child-list order (UL §9.3), then,
  for each child in child-list order, that child's subtree by the same rule;
- **nearest**: first the element's subtree, then the element itself, then, walking up one
  ancestor at a time, that ancestor's other children and their subtrees, then the ancestor
  itself; the walk stops at an ancestor that code marked as a lookup boundary (not traced) or
  at the root.

Element IDs are not unique (UL §5.2), so each rule below says which lookup it uses.

> **OPEN:** which code marks an element as a lookup boundary for the nearest search. No layout
> property was seen to set it. Tessera rule: no boundaries (the walk ends at the root panel).

<a id="s2-4"></a>
### 2.4 UI messages and commands used here

Keys from the `UIElementMessage` base mapper and the input commands of UB §3.1 (labels are
interoperability identifiers; take the keys from the install):

| Key | Message label | | Key | Message label |
|---|---|---|---|---|
| 0x01 | `Scrollable_scrollablesize_changed` | | 0x29 | `Button_hot_click` |
| 0x04 | `ListBox_same_selection` | | 0x2C | `Element_mouse_scroll` |
| 0x0A | `Element_mouse_over_top` | | 0x32 | `Element_mouse_press` |
| 0x0C | `Menu_closed` | | 0x33 | `Scrollbar_line_increment` |
| 0x0E | `Element_mouse_release` | | 0x34 | `Scrollbar_new_position` |
| 0x12 | `Meter_new_position` | | 0x35 | `Button_click` |
| 0x15 | `Menu_opened` | | 0x36 | `Meter_goal_position_reached` |
| 0x18 | `Element_mouse_tap` | | 0x37 | `Panel_NewPage` |
| 0x1E | `Element_mouse_click` | | 0x3A | `Scrollbar_page_increment` |
| 0x22 | `Scrollbar_page_decrement` | | 0x3B | `Element_set_show_state` |
| 0x25 | `Element_mouse_move` | | 0x3C | `ListBox_new_selection` |
| | | | 0x3D | `Element_activated` |
| | | | 0x3E | `Scrollbar_line_decrement` |

Commands: `SelectLeft` 0x56, `SelectDblLeft` 0x33, `ScrollUp` 0x82, `ScrollDown` 0x10 (UB §3.1).
States: `normal` 0x0B, `highlight` 0x07, `ghosted` 0x12, `ghosted_highlight` 0x13,
`ghosted_rollover` 0x14, `ghosted_pressed` 0x15, `open` 0x06, `closed` 0x09 (UL §3.3, §9.7).

---

<a id="s3"></a>
## 3. Classes and factories

<a id="s3-1"></a>
### 3.1 Factory per engine type (BIN, W1, W2)

The UI manager keeps one factory per element type (UL §9.2 step 3). The engine registers these
for the engine types (keys of the `UIElementType` base mapper):

| Type | Label | Class created | | Type | Label | Class created |
|---|---|---|---|---|---|---|
| 1 | GroupBox | GroupBox | | 11 | Viewport | Viewport (§12) |
| 2 | ColorPicker | ColorPicker | | 12 | Browser | Browser (registered by the browser module) |
| 4 | Menu | Menu (§11) | | 14 | Field | Field: the base element's behaviour, with no drawing, layout or input rule of its own |
| 5 | Scrollbar | Scrollbar (§6) | | 15 | Dragbar | Dragbar (UB §8.1) |
| 7 | ListBox | ListBox (§8) | | 16 | Text | Text (FO) |
| 8 | Panel | Panel (§10) | | 17 | Meter | Meter (§4) |
| 9 | Button | Button (UL §9.7, §7) | | 18 | ResizeBar | ResizeBar (UB §8.2) |
| 10 | Tree | Tree (§9) | | | | |

Types 3 (Scrollable), 6 (ComboBox), 13 (Frame) and 19 (Flash) have no factory: an element of
these types is not created (UL §9.2 step 3). No layout uses them (UL §11).

<a id="s3-2"></a>
### 3.2 Derivation (BIN, W1)

Each class adds to the behaviour of the class it derives from; a rule given for a base class
holds for every derived class unless the derived class's section replaces it.

| Class | Derives from |
|---|---|
| Scrollable (§5) | plain element (no factory of its own) |
| Text | Scrollable |
| Button | Text |
| Menu | Button |
| Scrollbar | Button |
| ListBox | Scrollable |
| Tree | ListBox |
| Meter, Panel, Viewport | plain element |
| GroupBox (§3.4), ColorPicker (§3.5) | plain element (BIN, W24, W25) |
| Browser (§3.5) | not traced (registered by the browser module) |

So a Scrollbar and a Menu have the Button state choice (UL §9.7), the auto-repeat and the state
requests of §7, and Text's ability to draw a label. A Tree lays out and selects like a ListBox.

<a id="s3-3"></a>
### 3.3 Census (Confirmed, OBS, 2026-10-06)

| Type | Stored typed elements (UL §11) | Resolved instances (UL §7.3, 83,161 total) |
|---|---|---|
| Meter | 88 | 882 |
| ListBox | 193 | 582 |
| Panel | 73 | 105 |
| Tree | 59 | 116 |
| Scrollbar | 13 | 792 |
| Menu | 4 | 195 |
| Viewport | 1 | 1 (and 35 instances of the game type `TargetViewport`, §12) |

Resolved instances count every use of a template; per-property counts below are counts of
resolved instances.

<a id="s3-4"></a>
### 3.4 GroupBox (type 1) (BIN, W24)

A GroupBox keeps one of the Buttons inside it selected, like a set of radio buttons or sort
column headers. It derives from the plain element and adds the rules below.

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x02EE | `UICore_GroupBox_selected_button` | Enum (element ID) | The selected element; set by the GroupBox itself (run-time) or by code. Set on no element in data |
| 0x0248 | `UICore_GroupBox_default_button` | Enum (element ID) | Selected when the tree is complete (below) |
| 0x01B4 | `UICore_GroupBox_allow_button_click_on_selected_button` | Bool | Whether a click on the already-selected element continues past the GroupBox |

The GroupBox remembers the **recorded selection**, an element ID that is 0 (none) at creation.

**Selection change.** When the effective `selected_button` changes to *v*:

1. If the recorded selection is not 0, find it by the **descendant** lookup (§2.3); if found,
   request state `normal` (0x0B) on it (§7.3 for a Button; UL §9.4 otherwise).
2. Record *v*. Find *v* by the descendant lookup; if found, request state `highlight` (0x07) on
   it.
3. Then handle the change as the base element does.

**Tree completion.** When a top-level element and its whole tree have been created (UL §9.2
steps 1–8, by either form of UL §9.1 "Create element", so also for entries created from
templates and for menu popups), the client runs a **completion pass** on the top-level element
once. The base element passes it to each of its children in child-list order, so it reaches
every element of the tree. A GroupBox first passes it to its children; then, if
`default_button` is not 0, it sets the run-time `selected_button` to that ID, which runs the
selection change above. (What other classes do in this pass: §5.1 for scrollables, §9.1 for
Trees, §7.3 for Buttons; Panels, Meters, Viewports and ColorPickers only pass it on, BIN,
W28.)

**Clicks.** On `Button_click` (0x35) from any source (its own, or one bubbling up from a
descendant, UB §12.1), with *e* the source's element ID:

- If *e* equals the effective `selected_button`: request `highlight` on the source again (so a
  `boolean_button` whose release has just switched `highlighted` off is highlighted again,
  §7.1 step 3). Then, if
  `allow_button_click_on_selected_button` is false, answer **stop** (UB §12.1: the click goes no
  further, so ancestors and game code do not see it); if it is true, handle the message as the
  base element does.
- Otherwise set the run-time `selected_button` to *e* (the selection change above moves
  `highlight` from the old element to the source) and handle the message as the base element
  does.

The source need not be a direct child and need not be a Button; any element that posts
`Button_click` and has an element ID takes part. Of the classes in this spec only Buttons and
the classes derived from them post it (§7.1).

Census (OBS, W25, resolved instances): 26 GroupBoxes. `default_button` is set on 20, and in
all 20 it names a Button that the descendant lookup finds; `allow_button_click_on_selected_button`
is true on 6 and absent on 20; `selected_button` is set on none. 25 GroupBoxes hold 2 to 12
Buttons (some also hold Fields, Text elements or a game-type element); 1 holds only Text
elements.

<a id="s3-5"></a>
### 3.5 ColorPicker (type 2) and Browser (type 12)

A ColorPicker derives from the plain element and replaces, among other methods, the base
element's message handler, property-change handler and image-layer drawing (BIN, W25). A Browser is created by a factory that
the browser module registers; its class was not traced. Census (OBS, W25): 6 ColorPicker
instances (the colour choices of the appearance and dye layouts, and one in a chat-option entry
template of the options layout, which P7 creates only if game code adds that entry); 9 Browser
instances (web, store and help windows), none in a §13 panel. The only class-specific
properties set in data are `UICore_ColorPicker_current_selection` (on 1 ColorPicker) and the
`UICore_Browser_…` cursor, pop-up and zoom settings.

> **OPEN:** what a ColorPicker does (its messages, properties and drawing) and what a Browser
> does. Neither is needed to draw the §13 panels. Tessera rule: both behave as the base
> element: they draw their own images (UL §9.6) and their children and add no input, layout or
> drawing rule; a Browser shows no web content.

---

<a id="s4"></a>
## 4. Meter (type 17)

<a id="s4-1"></a>
### 4.1 Properties

| PID | Label | Type | Read when | Effect |
|---|---|---|---|---|
| 0x0342 | `UICore_Meter_child_meter` | Bool | post-creation | Child-meter mode (§4.3) |
| 0x05FF | `UICore_Meter_frame_meter` | Bool | post-creation | Frame-meter mode (§4.4) |
| 0x0AD5 | `UICore_Meter_alphatest_meter` | Bool | post-creation | Alpha-test mode (§4.5) |
| 0x02F3 | `UICore_Meter_position` | Float | on change, on update | Fill fraction, nominally 0–1 (§4.6). Not clamped by the meter |
| 0x0020 | `UICore_Meter_goal_position` | Float | on change | Target of a smooth movement (§4.6) |
| 0x024D | `UICore_Meter_smooth_movement` | Bool | when the goal changes | Animate toward the goal (§4.6) |
| 0x001F | `UICore_Meter_smooth_movement_duration` | Float, s | when the goal changes | Duration of the animation |
| 0x03CB | `UICore_Meter_notify_on_new_position` | Bool | on change | Send 0x12 when the position changes (§4.6) |
| 0x06E2 | `UICore_Meter_child_direction` | Enum 0x2300017C: 1 Bottom, 2 Top, 3 Right, 4 Left | part creation, update | Edge the fill grows from (§4.3) |
| 0x011F | `UICore_Meter_child_image` | DID (0x41) | part creation, on change | Fill image (§4.3) |
| 0x05FE | `UICore_Meter_child_drawmode` | Enum `DrawModeType` | part creation, on change | Draw mode of the fill image (UL §9.6.1) |
| 0x05A7 | `UICore_Meter_move_fill` | Bool | update | The image slides with the fill instead of being uncovered (§4.3) |
| 0x024C | `UICore_Meter_offset` | Int, px | update | Start inset of the fill (§4.3); set on no element in this install |
| 0x0422 | `UICore_Meter_frame_array` | Array of `UICore_Meter_frame` (0x0473, DID) | update | Frames of a frame meter (§4.4) |
| 0x0341 | `UICore_Meter_draw_mode` | Enum `DrawModeType` | update | Draw mode of a frame (§4.4); Normal (2) when absent |

`UICore_Meter_child_image_size_to_owner` (0x051B, set on 1 element, false) and a stand-alone
`UICore_Meter_frame` have no reader in the UI code (BIN search for their PIDs found none).

<a id="s4-2"></a>
### 4.2 Modes and post-creation (BIN, W4)

At post-creation (UL §9.2 step 8, after the base element's own steps) the meter reads its three
mode properties once. A meter may have any combination; in data each meter has at most one
(§4.8):

1. **Child meter** (`child_meter` true): create the fill parts (§4.3).
2. **Alpha-test meter** (`alphatest_meter` true): give the meter its own drawing surface (§4.5).
3. **Frame meter** (`frame_meter` true): no set-up; frames are chosen on update (§4.4).

A meter with none of the three draws only its own images (UL §9.6) and children; its position
still sends messages (§4.6).

The meter **updates** its parts (§4.3–§4.5, each for its own mode) whenever its position changes
(§4.6) and whenever it is resized (after the resize itself).

<a id="s4-3"></a>
### 4.3 Child meter: parts and geometry (BIN, W4)

**Parts.** The meter creates two parts once, at post-creation, and only if all three of
`child_direction` ≠ 0, `child_image` set to a valid DID and `child_drawmode` ≠ 0 hold (otherwise
it logs a diagnostic and creates nothing):

| Part | Element ID (label) | Type | Parent | Initial box | Notes |
|---|---|---|---|---|---|
| fill | 0x26 (`meter_fill`) | Field | the meter | (0, 0), *W* × *H* of the meter | `UICore_Element_mouse_invisible` set true |
| fill image | 0x23 (`meter_fill_image`) | Field | the fill | (0, 0), *W* × *H* | base image = `child_image` with `child_drawmode` (UL §6.2, with skin substitution UL §9.8); `mouse_invisible` true |

The fill clips the fill image (UL §9.3), and the fill image draws its surface at native size from
its top-left corner, tiled (UL §9.6). In data the fill images are wider than the meters (for
example 175 × 9 images in 164 × 9 meters), so one copy is drawn and clipped.

When `child_image` or `child_drawmode` changes later (for example through a state such as
`vitalmeter_red`, which sets `child_image`; 990 state uses), the meter sets the fill image's base
image again with both current values, provided the fill image exists, the draw mode is not 0 and
the image is valid.

**Update.** With *p* = `UICore_Meter_position`, *o* = `UICore_Meter_offset` (0 when absent),
*m* = `move_fill`, and *W*, *H* the meter's current size, the meter sets the fill's position
(*fx*, *fy*) and size (*fw*, *fh*) and the fill image's position (*ix*, *iy*) and size (*iw*, *ih*),
all relative to their parents. Start from *fx* = *fy* = *ix* = *iy* = 0, *fw* = *iw* = *W*,
*fh* = *ih* = *H*, then by direction:

| Direction | *m* true (image slides with the fill) | *m* false (image fixed, fill uncovers it) |
|---|---|---|
| 4 Left (grows rightward from the left edge) | *fx* = trunc(0 − (1 − *p*)·*W*) | *fx* = *o*; *fw* = trunc(*W*·*p*) − *o*; *iw* = *fw* |
| 3 Right (grows leftward from the right edge) | *fx* = trunc((1 − *p*)·*W*) | *fw* = trunc(*W*·*p*); *fx* = *W* − *fw*; *ix* = *o* − *fx*; *iw* = *W* − *o* |
| 2 Top (grows downward from the top edge) | *fy* = trunc(0 − (1 − *p*)·*H*) | *fy* = *o*; *fh* = trunc(*H*·*p*) − *o*; *ih* = *fh* |
| 1 Bottom (grows upward from the bottom edge) | *fy* = trunc((1 − *p*)·*H*) | *fh* = trunc(*H*·*p*); *fy* = *H* − *fh*; *iy* = *o* − *fy*; *iw* = *W* − *o* (the width, not the height, as the client does) |
| any other | nothing changes (full fill) | same |

All products are 32-bit float products of the integer size converted to float and *p*; each
result is truncated toward zero. The fill is then resized and moved, and the fill image resized
and moved, in that order.

So, with *m* false and *o* = 0, the image stays where it is relative to the meter and the fill
acts as a window over it whose length is trunc(size × *p*), anchored at the edge the direction
names; with *m* true the window always has the meter's full size and slides out of the meter by
(1 − *p*) of its size, carrying the image with it.

> **OPEN:** what the client does with a negative or zero fill size, which the formulas give for
> *p* < 0 or for *o* larger than trunc(size × *p*) (no element sets *o*; no stored position is
> outside 0–1). Tessera rule: treat a size below 1 as 0 (nothing drawn) and keep the computed
> position.

<a id="s4-4"></a>
### 4.4 Frame meter (BIN, W5)

On update the frame meter takes *n* = the number of entries of `UICore_Meter_frame_array`.

1. If the array is absent or empty, log a diagnostic and stop.
2. Frame index *i* = trunc(float(*n* − 1)·*p* + 0.5) (32-bit float).
3. If *i* equals the index shown last (none at creation), stop.
4. Remember *i*. If entry *i* exists and holds a valid DID, set the meter's **own base image**
   (UL §6.2, with skin substitution) to that DID with the draw mode `UICore_Meter_draw_mode`
   (Normal, 2, when absent). Otherwise log a diagnostic; the image does not change.

The frame replaces the base layer of the meter itself (UL §9.6); it is drawn like any element
image.

<a id="s4-5"></a>
### 4.5 Alpha-test meter (Probable, W6)

At post-creation the meter is given its own drawing surface (the flag that makes code create a
surface for a non-root element, UL §6.6 OPEN). On every update the meter passes 1 − *p* to that
surface as a threshold.

> **OPEN:** the exact effect of the threshold (which pixels of the meter's image the surface
> drops for a given value, and whether the comparison is "less than" or "less or equal"). 4
> elements use this mode; none is in the §17 panels of UL. Tessera rule: draw the meter's image
> and drop the pixels whose alpha ÷ 255 is below 1 − *p*.

<a id="s4-6"></a>
### 4.6 Position, goal and smooth movement (BIN, W7)

**Position change.** When the effective `UICore_Meter_position` changes to *v* (from data, a
state, game code or a running animation):

1. Update the parts (§4.3–§4.5).
2. If |*v* − *last*| > 0.0002 (*last* starts at 0): set *last* = *v*, and if
   `notify_on_new_position` is true send `Meter_new_position` (0x12).
3. If |*v* − *goal*| ≤ 0.0002, where *goal* = `UICore_Meter_goal_position` (0 when absent),
   send `Meter_goal_position_reached` (0x36). (This also fires for a meter at 0 with no goal.)

**Goal change.** When the effective `UICore_Meter_goal_position` changes to *g*:

- If `smooth_movement` is false: set the run-time position to *g* (which runs the steps above).
- If it is true:
  1. If an animation is running, stop it (and remove its frame update).
  2. Let *d* = `smooth_movement_duration`. If *d* = 0, stop here: **the goal is ignored**. In
     this install 24 meters have `smooth_movement` true and no duration, so a goal has no effect
     on them.
  3. Otherwise record *start* = the current position, *t₀* = now, *t₁* = now + *d*, and register a
     frame update.
- **Each frame** while running: *t* = (now − *t₀*) ÷ (*t₁* − *t₀*) computed in 64-bit and
  converted to 32-bit float; if *t₁* − *t₀* ≤ 0, *t* = 0. If *t* ≥ 1: stop the animation and use
  *t* = 1. Set the run-time position to *start* + (*g* − *start*)·*t* (32-bit float).

<a id="s4-7"></a>
### 4.7 Drawing

The meter has no drawing routine of its own: its own images (and a frame meter's frame) are
its base layer (UL §9.6), and a child meter's fill is drawn as the fill image part clipped by the
fill part (UL §9.3).

<a id="s4-8"></a>
### 4.8 Census (Confirmed, OBS, 2026-10-06)

| Quantity | Value |
|---|---|
| Modes (resolved instances) | child 379, frame 402, alpha-test 4, none 97; no instance combines two |
| Child direction | 4 (Left) 311, 3 (Right) 51, 2 (Top) 17 |
| Child draw mode | Normal (2) 226, Alphablend (4) 153 |
| `move_fill` | true 19, false 271, absent 89 |
| Frame arrays | 60 frames (399), 30 frames (3); frame draw mode Multiply (3) 395, Screen (7) 4, Alphablend (4) 3 |
| Smooth movement | true with duration 0.3 s: 17; true without duration: 24; false: 1 |
| Meters with layout children | 22 (21 with one child, 1 with three) |
| `UICore_Meter_offset` | 0 instances |

---

<a id="s5"></a>
## 5. Scrollable (base of Text, ListBox, Tree) (BIN, W8)

<a id="s5-1"></a>
### 5.1 State and properties

A scrollable keeps a **content size** (*Cw*, *Ch*, pixels; 0 × 0 at creation) and **scroll
offsets** (*sx*, *sy*, pixels; 0 at creation). Its derived class sets the content size (a
ListBox in §8.3). Content is drawn shifted by (−*sx*, −*sy*) and clipped to the scrollable's box
(UL §9.3).

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x01B7 | `UICore_Scrollable_vertical_scrollbar` | Enum (element ID) | Its vertical scroll bar, found by the **nearest** lookup (§2.3); it must be a Scrollbar whose own nearest element with the scrollable's ID is the scrollable |
| 0x051A | `UICore_Scrollable_horizontal_scrollbar` | Enum (element ID) | Its horizontal scroll bar, same rule |
| 0x0170 | `UICore_Scrollable_keep_in_bounds` | Bool, default true | Clamp offsets (§5.3) |
| 0x0561 | `UICore_Scrollable_preserve_percentage_on_resize` | Bool | Keep the scroll fraction when the view or content changes (§5.2) |
| 0x071F | `UICore_Scrollable_notify_on_size_change` | Bool | Send 0x01 when the content size changes (§8.3) |

When a scroll-bar ID property changes, the scrollable first runs §5.2 for the scroll bar it had
(if any), then records the new ID; the new bar is first updated on the next trigger of §5.2.

**Completion pass** (§3.4; BIN, W28). When the pass reaches a scrollable (after it has passed
it on to its children), the scrollable, for each of its two scroll-bar IDs that is not 0:
finds the bar by the nearest lookup with the mutual check of the table
above (the found element must be a Scrollbar); if it is found, attaches itself to the bar as a
**listener** (UB §12.1 step 2.2, so that the bar's messages reach the scrollable although the
bar is usually not its descendant, §5.5) and sets its own mouse-visible flag (UB §4.3). Then it
runs §5.2 for both axes.

<a id="s5-2"></a>
### 5.2 Updating a scroll bar from the scrollable

For one axis (horizontal or vertical), with *C* = the content size and *V* = the scrollable's
own size on that axis, and *s* the offset on that axis:

1. Find the axis's scroll bar (§5.1). If there is none, stop.
2. If *C* ≤ *V*: use *C* = *V*, set *s* = 0, and mark the bar **disabled**. Otherwise it is
   enabled.
3. If *s* + *V* > *C*, set *s* = *C* − *V*.
4. Widget fraction *f* = 1.0 if *C* = 0, otherwise min(*V* ÷ *C*, 1.0) (32-bit float).
5. Read the bar's current `UICore_Scrollbar_position` *q*; if disabled, *q* = 0.
6. On the bar, with `UICore_Scrollbar_disallow_updating` held true for the duration (so the bar
   does not lay itself out in between): set `UICore_Scrollbar_horizontal` to the axis, set
   `UICore_Scrollbar_disabled` to the disabled mark, set `UICore_Scrollbar_widget_size` to *f*.
7. If `preserve_percentage_on_resize` is true: set the bar's position to *q* again, set
   *s* = trunc((*C* − *V*)·*q*) with the stored content size *C*, set the **other** axis's
   offset to 0 (the client does this), and move the content (§5.4).
   Otherwise **synchronise** the bar from the offset: *q* = 0 if |*C* − *V*| ≤ 0.0002, else
   *s* ÷ (*C* − *V*) (32-bit float); set the bar's position to *q* and its horizontal flag with
   updating disallowed, then lay the bar out (§6.4).

**Triggers.** The scrollable runs this for both axes in the completion pass (§5.1), when its
own size changes (then it re-applies §5.3 to the current offsets) and when its content size
changes (§8.3).

So a scrollable whose size and content never change after creation (for example a list that
stays empty, content 0 × 0) still runs §5.2 once, in the completion pass: *C* ≤ *V*, the bar is
disabled, and with `hide_when_disabled` true (780 of 792 bars) it is hidden (§6.4 step 6). (This
answers the OPEN of the first draft; BIN, W28.)

<a id="s5-3"></a>
### 5.3 Scrolling to an offset

To scroll to (*x*, *y*) (from game code, from a scroll-bar message, or from a forced call that
skips the clamp):

1. Unless forced, and if `keep_in_bounds` is true: *x* = min(*x*, *Cw* − *W*), then *x* = 0 if
   *x* ≤ 0; *y* likewise with *Ch* − *H*.
2. If (*x*, *y*) equals (*sx*, *sy*), stop.
3. Store the offsets, synchronise both scroll bars from them (§5.2 step 7, second branch), and
   move the content (§5.4).

<a id="s5-4"></a>
### 5.4 Moving the content

A ListBox (and Tree) marks its layout dirty and asks for a redraw of its box; the entries are
placed again before the next draw (§8.3). (Text's line scrolling is outside this spec.)

<a id="s5-5"></a>
### 5.5 Messages from the scroll bars

A scrollable acts on messages whose sender is one of its two scroll bars (by the stored ID, and
the sender must be that bar):

| Message | Action |
|---|---|
| `Scrollbar_line_increment` (0x33), `Scrollbar_line_decrement` (0x3E), `Scrollbar_page_increment` (0x3A), `Scrollbar_page_decrement` (0x22) | Ignored while a condition reported by the input layer holds (one query of the input layer, not identified; Tessera: never). Otherwise compute the step for the bar's axis (§5.6), positive for increments and negative for decrements, add it to that axis's offset, and scroll there (§5.3) |
| `Scrollbar_new_position` (0x34) | *q* = the bar's position; on the bar's axis set the offset to trunc((*C* − *V*)·*q*) without the clamp, keep the other offset, and move the content (§5.4) |

The message counts as handled.

> **OPEN:** the input-layer condition under which line and page messages are ignored (one query
> of the input layer whose meaning was not identified). Tessera rule: never ignore them.

<a id="s5-6"></a>
### 5.6 Line and page steps (ListBox and Tree)

With *V* the view size on the axis and *n* the number of columns (horizontal) or rows
(vertical) of the current layout (§8.3):

- **page**: *V*;
- **line**: 0 if *n* = 0, otherwise min(*C* ÷ *n* rounded toward zero, *V*) (the average cell
  size, at most one view).

(Text's scrolling belongs to the Text class and is outside this spec.)

<a id="s5-7"></a>
### 5.7 Wheel and hover (Probable for delivery, BIN for the action)

- **Wheel.** On `Element_mouse_scroll` (0x2C) with `ScrollUp` or `ScrollDown`, a scrollable with
  a vertical scroll bar passes the command to that bar's wheel rule (§6.8) if the bar exists,
  is visible, is not in state `ghosted` and is not disabled; the message is then handled. The
  scrollable does not check who sent the message, so wheel messages raised by its descendants
  (entries under the pointer, UB §4.7) reach it and scroll it (Probable: the delivery path to
  ancestors was not traced). The horizontal bar never takes the wheel from the scrollable.
- **Hover.** On `Element_mouse_over_top` (0x0A) with value 1 from the scrollable or any
  descendant, the scrollable pushes the input map with key 0x2, `ScrollableControls_Mouse`
  (KM §12, the map that binds the wheel to `ScrollUp` and `ScrollDown`), for the hovered element
  with priority 0 (UB §5.2 stack rules), and registers itself so that the map is popped when the
  pointer leaves that element. So the wheel commands exist only while the pointer is over a
  scrollable or one of its descendants (BIN for the push; the pop path is OPEN below).

> **OPEN:** the exact path that pops the `ScrollableControls_Mouse` entry a scrollable pushed for
> a hovered descendant: which event triggers the pop (the descendant's own
> `Element_mouse_over_top` with value 0, or a manager-side record of the hovered element), and
> whether moving between two descendants of the same scrollable pops and pushes again or keeps
> one entry. Tessera rule: when the pointer leaves the element the entry was pushed for, pop that
> entry; when it enters another element of the same scrollable's subtree, push again by the rule
> above (UB §5.2 handles duplicates). The net effect, wheel commands only while the pointer is
> over the scrollable's subtree or a scroll bar (§6.9), is the same under either reading.

---

<a id="s6"></a>
## 6. Scrollbar (type 5)

<a id="s6-1"></a>
### 6.1 Properties (BIN, W9)

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x006B | `UICore_Scrollbar_horizontal` | Bool | Axis; lays the bar out again |
| 0x006A | `UICore_Scrollbar_proportional` | Bool | Thumb length follows `widget_size` (§6.4); lays out |
| 0x06DF | `UICore_Scrollbar_widget_size` | Float, default 1.0 | Fraction of the track the thumb covers when proportional; lays out |
| 0x02F0 | `UICore_Scrollbar_min_widget_size` | Int, px | Minimum thumb length; lays out |
| 0x06E0 | `UICore_Scrollbar_position` | Float 0–1 | Value (§6.5); snapped, clamped, then lays out |
| 0x0421 | `UICore_Scrollbar_goal_position` | Float | Target of a smooth movement (§6.9) |
| 0x05A6 | `UICore_Scrollbar_smooth_movement` | Bool | Animate toward the goal |
| 0x001E | `UICore_Scrollbar_smooth_movement_duration` | Float, s | Animation length; set on no element |
| 0x011E | `UICore_Scrollbar_increment_button` | Enum (element ID) | Increment button part (§6.2) |
| 0x02F2 | `UICore_Scrollbar_decrement_button` | Enum (element ID) | Decrement button part |
| 0x0694 | `UICore_Scrollbar_disabled` | Bool | Disabled (§6.9); lays out |
| 0x01B6 | `UICore_Scrollbar_hide_when_disabled` | Bool | Hide the bar while disabled; lays out |
| 0x0560 | `UICore_Scrollbar_ghost_when_disabled` | Bool | Ghost the bar while disabled; lays out |
| 0x0340 | `UICore_Scrollbar_disallow_updating` | Bool | While true the bar does not lay itself out |
| 0x024B | `UICore_Scrollbar_num_stop_locations` | Int | Number of stops *N* (§6.5); only when set on the element |
| 0x02F1 | `UICore_Scrollbar_stop_locations` | Bool | Recorded with the stop flag; no further reader found; set on no element |
| 0x02A0 | `UICore_Scrollbar_current_stop` | Int | Current stop index, written by the bar |
| 0x03C9 | `UICore_Scrollbar_stops_dont_include_endpoints` | Bool | Stops at cell centres instead of ends (§6.5); also changes the travel (§6.4) |
| 0x03CA | `UICore_Scrollbar_move_to_touched_position` | Bool | Slider mode (§6.7) |
| 0x04C6 | `UICore_Scrollbar_page_interval` | Float, s | Copied to the bar's own `UICore_Button_hot_click_interval` |
| 0x06E1 | `UICore_Scrollbar_page_first_interval` | Float, s | Copied to the bar's own `UICore_Button_hot_click_first_interval` |
| 0x0519 | `UICore_Scrollbar_horizontal_allow_mouse_wheel` | Bool, default false | A horizontal bar takes the wheel only with this (§6.8) |

<a id="s6-2"></a>
### 6.2 Parts (BIN, W9, W20, W21)

| Part | How found | Placed by the bar |
|---|---|---|
| **thumb** | direct child with element ID 0x16 (`widget`); found on first layout | moved and resized (§6.4) |
| **decrement button** | descendant with the `decrement_button` ID; if not found, the bar retries on its next frame update among the descendants of its own parent and then **re-parents** the button into itself | moved to (0, 0); made an auto-repeat button (§7.1) |
| **increment button** | the same with `increment_button` | moved to (*W* − *bw*, *H* − *bh*) (its own size *bw* × *bh*); made an auto-repeat button |
| **fill parts** | direct children with IDs 0x1D (`scrollbar_left_fill`), 0x20 (`scrollbar_right_fill`), 0x03 (`scrollbar_top_fill`), 0x09 (`scrollbar_bottom_fill`) | sized to reach from the bar's edges to the thumb's centre (§6.4 step 5) |

When a button part is made an auto-repeat button, the bar sets on the button
`UICore_Button_hot_button` true and, only if they are not set on the button,
`UICore_Button_hot_click_first_interval` = 0.5 and `UICore_Button_hot_click_interval` = 0.125.
A track button needs no image of its own; in data the bar's track art is drawn by Field
children with stretch edges.

Census (resolved instances): every Scrollbar has a `widget` child (792 of 792); 757 name both
buttons and have them as direct children, 16 name buttons elsewhere (for example the options
slider template, whose two 16 × 16 arrow Buttons are siblings of the bar: on the bar's first
frame update they become its children at its two ends, BIN re-read W28, §13 P7), 19 name none; no Scrollbar
has a fill part (W21). In the common vertical template the decrement button carries the
up-pointing arrow and the increment button the down-pointing one (W20), which agrees with the
placement.

<a id="s6-3"></a>
### 6.3 Track (BIN, W9)

The **track** is the box (*tl*, *tt*, *tr*, *tb*) inside the bar, computed before every layout:

1. Start with (0, 0, *W*, *H*). Note that *tr* and *tb* start at the width and height, not at
   *W* − 1 and *H* − 1.
2. If the decrement button exists as a part of this bar (found by the **nearest** lookup from the
   bar, a Button, and the bar is the nearest element with the bar's own ID seen from the
   button; BIN, W28): *tl* += its width, *tt* += its height, and move it to (0, 0).
3. If the increment button exists as a part of this bar (same test): *tr* −= its width, *tb* −= its height, and move it to
   (*tr*, *tb*) (after the subtraction).

Both adjustments apply on both axes whatever the bar's orientation; only the axis of the bar
matters for the thumb. Track length *L* = *tb* − *tt* (vertical) or *tr* − *tl* (horizontal).

<a id="s6-4"></a>
### 6.4 Layout and thumb geometry (BIN, W9)

The bar lays itself out whenever one of the "lays out" properties of §6.1 changes, when its
position is set, and when it is resized. Steps:

1. If `disallow_updating` is true, stop.
2. *q* = `UICore_Scrollbar_position` (0 when absent).
3. If `num_stop_locations` is set on the bar and is not 0, and `current_stop` is **not** set on
   the bar: set the position from the user value *q* (§6.5, which picks the stop and sends
   `Scrollbar_new_position`) and stop; that call lays the bar out again.
4. Compute the track (§6.3). If the thumb exists:
   1. **Size.** Let (*tw*, *th*) be the thumb's current size. If `proportional` is true:
      *k* = trunc(float(*L*)·`widget_size` + 0.5), the product in 32-bit float and the
      addition in 64-bit; *k* = max(*k*, `min_widget_size`); on the bar's axis the thumb's
      length becomes *k* (*tw* for horizontal, *th* for vertical); resize the thumb to
      (*tw*, *th*) (its own children follow by their edges, UL §8.4).
   2. **Travel.** *v* = *L* − (thumb length on the axis), unless
      `stops_dont_include_endpoints` is true, in which case *v* = *L*.
   3. **Position.** *a* = trunc(float(*v*)·*q*) + (*tl* or *tt* on the axis). If
      `stops_dont_include_endpoints` is true, subtract half(thumb length). Move the thumb to
      (*a*, 0) for a horizontal bar or (0, *a*) for a vertical one (the thumb's other coordinate
      is always 0).
5. **Fill parts.** Let *cx* = *x* + half(*tw*) and *cy* = *y* + half(*th*) be the thumb's
   centre, with (*x*, *y*) the position just set. Each fill part that exists is shown unless the
   bar is disabled, and is placed: left fill at (0, 0) size (*cx*, *H*); right fill at (*cx*, 0)
   size (*W* − *cx*, *H*); top fill at (0, 0) size (*W*, *cy*); bottom fill at (0, *cy*) size
   (*W*, *H* − *cy*).
6. **Visibility.** Show the bar unless it is disabled **and** `hide_when_disabled` is true.
7. **Ghosting.** If `ghost_when_disabled` is true, request state `ghosted` (0x12) when disabled
   and `normal` (0x0B) otherwise (§7.3).

Step 4 runs even when the bar is disabled; the thumb's size then follows `widget_size` 1.0 or
whatever the scrollable last set.

<a id="s6-5"></a>
### 6.5 Value mapping and stops (BIN, W9)

**Position property change.** When the effective position changes to *q*:

1. If the stop rule applies (`num_stop_locations` set and non-zero), *q* is snapped: *q* =
   stop-to-value(value-to-stop(*q*)).
2. Clamp to [0, 1].
3. If the result differs from the requested value by more than 0.0002, set the run-time
   position to the result (which re-enters this rule); otherwise lay the bar out (§6.4).

**Value to stop** (*N* = `num_stop_locations`, only when set and non-zero; otherwise "no stop"):
if `stops_dont_include_endpoints`: *i* = trunc(*N*·*q*); otherwise, if *N* > 1:
*i* = trunc(float(*N* − 1)·*q* + 0.5) (the addition in 64-bit), else *i* = 0.

**Stop to value**: if the stop rule does not apply, −1 (none). Clamp *i* to [0, *N* − 1]. If
`stops_dont_include_endpoints`: (*i* + 0.5) ÷ *N*; otherwise, if *N* > 1: *i* ÷ (*N* − 1), else 0.5.

**Setting the position from the user** (dragging, clicking with the slider mode, §6.6–§6.7):
clamp the value to [0, 1]. If the stop rule applies, **select stop** *i* = value-to-stop(value);
otherwise set the run-time position to the value and send `Scrollbar_new_position` (0x34) with
the arguments (trunc(value × 1000), −1).

**Select stop** *i*: *q* = stop-to-value(*i*); if *q* < 0 stop. Clamp *i* to [0, *N* − 1]; set the
run-time `current_stop` = *i* and the run-time position = *q*; send `Scrollbar_new_position`
with (trunc(*q* × 1000), *i*).

The bar never scrolls content itself; its owner reacts to the messages (§5.5).

<a id="s6-6"></a>
### 6.6 Arrow buttons and track paging (BIN, W10)

- **Arrow buttons.** On `Button_hot_click` (0x29, §7.1) from the increment button: step +1;
  from the decrement button: step −1. If `current_stop` is set on the bar: select stop
  (current + step). Otherwise send `Scrollbar_line_increment` (0x33) for +1 or
  `Scrollbar_line_decrement` (0x3E) for −1, with the step as argument.
- **Track paging** (only when `move_to_touched_position` is false; the bar is then itself an
  auto-repeat button, §6.2 rule applied to the bar). On `Button_hot_click` from the bar itself,
  take the pointer position relative to the bar; on the bar's axis compare it with the thumb's
  span [*a*, *a* + *len*], where *a* is the thumb's position on the bar's axis (§6.4 step 4.3)
  and *len* its length on that axis: beyond the end, send `Scrollbar_page_increment` (0x3A);
  before the start, send `Scrollbar_page_decrement` (0x22); inside (ends included), nothing. The message
  repeats with the auto-repeat (§7.1), so holding the button pages until the thumb reaches the
  pointer.

<a id="s6-7"></a>
### 6.7 Dragging the thumb and the slider mode (BIN, W10)

A **drag source** is the thumb, or the bar itself when `move_to_touched_position` is true. In
the slider mode the bar turns off the thumb's mouse-button flag (UL §9.2 step 5) so that clicks
on the thumb reach the bar, and turns off its own `hot_button`.

1. **Press** (`Element_mouse_press` from a drag source while it holds `SelectLeft` or
   `SelectDblLeft`): in the slider mode first jump to the pointer (step 4). Record the press
   point in bar coordinates (a child's message position plus the child's position) and the
   current position as the **drag start**, and mark dragging.
2. **Move** (`Element_mouse_move` from a drag source holding the left command), with the point
   (*x*, *y*) in bar coordinates:
   - If the coordinate across the axis (|*x*| for a vertical bar, |*y*| for a horizontal one)
     is more than 100: if dragging, set the position from the user to the drag-start value
     (the thumb snaps back) and clear dragging. Nothing else.
   - Otherwise, in the slider mode jump to the pointer (step 4) and mark dragging. Else let
     *v* = travel on the axis (track length minus the thumb length, or the track length with
     `stops_dont_include_endpoints`); if *v* = 0, set the position from the user to 0;
     otherwise to drag-start + (pointer − press point on the axis) ÷ *v* (32-bit float). Mark
     dragging.
3. **Release** (`Element_mouse_release` of `SelectLeft` or `SelectDblLeft` from a drag source):
   clear dragging.
4. **Jump to the pointer** (slider mode): with the track and thumb of §6.3–§6.4, *px* = *x* − *tl*,
   *py* = *y* − *tt*; unless `stops_dont_include_endpoints`, subtract half the thumb's width and
   height and clamp *px* to [0, (*tr* − *tl* − thumb width) − 1] and *py* likewise; let *d* = the
   travel on the axis minus 1; the value is *px* ÷ *d* (or *py* ÷ *d*) if *d* > 1, else 0; set
   the position from the user to it.

<a id="s6-8"></a>
### 6.8 Wheel (BIN, W10)

A bar acts on `ScrollUp` and `ScrollDown` (from its own `Element_mouse_scroll` or passed by its
scrollable, §5.7) unless it is horizontal and `horizontal_allow_mouse_wheel` is false. If the
bar has no position property set on it, nothing happens (the wheel still counts as handled).
Otherwise:

- with stops (`current_stop` set): step = +1 for `ScrollDown` and −1 for `ScrollUp` on a vertical
  bar, the opposite on a horizontal bar; select stop (current + step);
- without: send `Scrollbar_line_increment` for `ScrollDown`, `Scrollbar_line_decrement` for
  `ScrollUp` (both orientations).

So one wheel step scrolls a list by one line step (§5.6).

<a id="s6-9"></a>
### 6.9 Hover, smooth movement, disabled (BIN, W10)

- **Hover.** On `Element_mouse_over_top` with value 1 the bar pushes `ScrollableControls_Mouse`
  (key 0x2) for itself with priority 0; with value 0 it pops it. So the wheel works over a bar
  as over its list.
- **Goal.** When `goal_position` changes: if `smooth_movement` is true and
  `smooth_movement_duration` is set on the bar with |duration| > 0.0002, animate from the
  current position to the goal over the duration exactly as the meter does (§4.6, without the
  goal message); if `smooth_movement` is true without a duration, the goal is ignored (776 bars
  have `smooth_movement` true and none has a duration, so for them a goal has no effect); if
  `smooth_movement` is false, set the position to the goal.
- **Disabled** bars still lay out (§6.4) and still react to input; only visibility and the
  ghosted state change (§6.4 steps 6–7).

<a id="s6-10"></a>
### 6.10 Census (Confirmed, OBS, 2026-10-06; resolved instances)

| Quantity | Value |
|---|---|
| Orientation | vertical 640, horizontal 152 |
| `proportional` | true 765, false 11, absent 16 |
| `min_widget_size` | 20 on 776 |
| `hide_when_disabled` | true 780, false 11 |
| `move_to_touched_position` | true on 27 (sliders) |
| Stops | `num_stop_locations` 100 on 8 (each with `position` 0.5); none uses `stops_dont_include_endpoints` |
| `disabled` | true on 12 |
| `ghost_when_disabled` | false on 2 |
| `horizontal_allow_mouse_wheel` | true on 2 |
| Thumb `widget` | a Button in the templates; its frame pieces are Field children with stretch edges and per-state images |

---

<a id="s7"></a>
## 7. Button additions used by these classes (BIN, W11)

UL §9.7 gives the Button's state choice. These rules complete it.

<a id="s7-1"></a>
### 7.1 Auto-repeat ("hot button")

| PID | Label | Type, default |
|---|---|---|
| 0x0071 | `UICore_Button_hot_button` | Bool |
| 0x05AE | `UICore_Button_hot_click_first_interval` | Float s, default 0.2 |
| 0x00C8 | `UICore_Button_hot_click_interval` | Float s, default 0.05 |
| 0x0072 | `UICore_Button_broadcast_when_ghosted` | Bool |
| 0x0B15 | `UICore_Button_click_when_ghosted` | Bool |
| 0x0026 | `UICore_Button_boolean_button` | Bool |

1. **Press** of `SelectLeft`, or of `SelectDblLeft` unless a per-element flag set by code
   excludes double clicks (not traced), marks the button pressed (two further flags that code
   sets can suppress the press; not traced). Then, if the button is not ghosted or `broadcast_when_ghosted` is
   true, and `hot_button` is true: send `Button_hot_click` (0x29) with the command at once,
   start repeating, register a frame update, and set *next* = now + `hot_click_first_interval`.
2. **Each frame** while repeating: if the pointer is over the button (the flag set by
   `Element_mouse_over_top`): when now ≥ *next*, send `Button_hot_click` (argument 0) and add
   `hot_click_interval` to *next*. If the pointer is not over it: when now > *next*, set
   *next* = now (so it fires at once when the pointer returns).
3. **Release** of the pressing command: if the pointer is over the button and (the button is
   not ghosted or `click_when_ghosted` is true): a `boolean_button` toggles
   `UICore_Button_highlighted` (this is how check boxes switch on and off); and, if the button
   was not repeating, a click happens. Clear pressed; stop repeating (remove the frame update);
   choose the state again (UL §9.7); if a click happened send `Button_click` (0x35) with
   `SelectLeft`.

So an auto-repeat button never sends `Button_click`; it sends `Button_hot_click` on the press
and then every interval while held over it.

> **OPEN:** the per-element flags that exclude `SelectDblLeft` or suppress a press (set by code,
> not by layout properties). Tessera rule: accept `SelectLeft` and `SelectDblLeft` and suppress
> nothing.

<a id="s7-2"></a>
### 7.2 Census

All 1,514 arrow buttons that are direct children of scroll bars are Buttons that set
`hot_button` true with first interval 0.5 s and interval 0.1 s (OBS, W23), so the bar's
fallback values of §6.2 are never used for them. The check box of the options templates is a
Button with `boolean_button` and `supports_ghosted_highlight` true (OBS, W22).

<a id="s7-3"></a>
### 7.3 State requests

When code asks a Button (and so a Menu or Scrollbar) to enter state *s* (a tab switch §10, a list
selection §8.4, ghosting §6.4), the Button translates the request into its two flags instead of
switching state directly. With *H* = `UICore_Button_highlighted` and *S* =
`UICore_Button_supports_ghosted_highlight`:

1. If *s* is `highlight` (0x07) or `ghosted_highlight` (0x13) and *H* is false: set *H* true.
   Then, if *S* is false, stop; if *S* is true, go to step 3 (step 2 is skipped).
2. Otherwise, if *s* is `normal` (0x0B), or *s* is `ghosted` (0x12) and *S* is true: if *H* is
   true, set it false, and then, if *S* is false, stop. If *H* was already false, or *S* is true,
   go on to step 3.
3. Let *g* = (*s* is `ghosted`, `ghosted_highlight`, `ghosted_rollover` or `ghosted_pressed`).
   If *g* differs from `UICore_Button_ghosted`, set `ghosted` to *g* and stop.
4. If step 1 or step 2 changed *H*, stop. Otherwise (nothing changed in steps 1–3) switch to
   state *s* directly (UL §9.4).

**Scope of the stops** (BIN, W26). The stop "if *S* is false" in steps 1 and 2 applies only when
that same step changed *H*. A request that leaves *H* unchanged always reaches step 3, whatever
*S* is; so `normal` on a Button that is ghosted but not highlighted clears `ghosted`, and
`ghosted` on a Button with *S* false does not touch *H* (step 2 does not apply) and sets
`ghosted`. Worked cases: §15.7; invariant WI12.

Every flag change makes the Button choose its state again by UL §9.7, so a "selected" tab or
list entry shows `highlight` and, under the pointer, `highlight_rollover`. In the completion
pass (§3.4) a Button (and so a Menu or Scrollbar) first does the scrollable steps of §5.1, then
chooses its state again by UL §9.7 (BIN, W28).

---

<a id="s8"></a>
## 8. ListBox (type 7)

<a id="s8-1"></a>
### 8.1 Properties (BIN, W12)

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x01B9 | `UICore_ListBox_horizontal` | Bool, default false | Fill rows first (true) or columns first (false) (§8.3); marks the layout dirty |
| 0x0202 | `UICore_ListBox_max_columns` | Int | Column limit (§8.3); dirty |
| 0x024E | `UICore_ListBox_max_rows` | Int | Row limit; dirty |
| 0x0343 | `UICore_ListBox_layout_only_visible` | Bool | Lay out only visible entries; dirty |
| 0x0423 | `UICore_ListBox_fill_in_reverse` | Bool | Fill from the far side (§8.3) |
| 0x0B22 | `UICore_ListBox_flipped_layout` | Bool | Fill from the bottom (§8.3) |
| 0x0B23 | `UICore_ListBox_wrap_indentation_amount` | Int, px | Extra indent per wrapped row or column (§8.3) |
| 0x0B24 | `UICore_ListBox_ignore_cell_alignment` | Bool | Advance by the entry's own size instead of the cell size |
| 0x06E3 | `UICore_ListBox_click_select` | Bool | A left press selects the entry under the pointer (§8.4) |
| 0x0600 | `UICore_ListBox_selected_item_state_change` | Bool | Selection changes entry states (§8.4) |
| 0x00BE | `UICore_ListBox_item_normal_state` | Enum state, default `normal` | State requested for a deselected entry |
| 0x0120 | `UICore_ListBox_item_selected_state` | Enum state, default `highlight` | State requested for the selected entry |
| 0x04C8 | `UICore_ListBox_drag_select` | Bool | Pressing captures the pointer for drag selection (§8.4) |
| 0x0562 | `UICore_ListBox_drag_rollover` | Bool | Recorded; behaviour not traced |
| 0x03CC, 0x00BD | `UICore_ListBox_notify_on_add`, `…_notify_on_remove` | Bool | Recorded; the add and remove messages were not traced |
| 0x051C | `UICore_ListBox_entry_templates` | Array of `UICore_ListBox_entry_template` (0x05A8) | Entry templates (§8.2) |
| 0x0171 | `UICore_ListBox_entry_templates_autofill` | Bool | Create one entry per template when the templates are set (§8.2) |

Each `UICore_ListBox_entry_template` is a struct of `UICore_ListBox_entry_template_layout`
(0x01B8, layout DID) and `UICore_ListBox_entry_template_element` (0x04C7, element ID).

<a id="s8-2"></a>
### 8.2 Entries and templates (BIN, W13)

The entries of a list box are its child elements in an **entry list** (in insertion order). Game
code adds them; the engine helps in one way: **add an entry from template *i*** takes entry *i*
of `entry_templates`, reads its layout DID and element ID (both must be present and of the right
types), creates that top-level element by the DID form of UL §9.1 "Create element" with the list
box as parent, and appends it to the entry list (a Tree inserts it as a node, §9.2). If the list
box refuses it, the element is destroyed.

When `entry_templates` changes and `entry_templates_autofill` is true, the list box adds one
entry from each template, in array order (39 list boxes have autofill true).

So the number of entries, and which template each uses, is game code except for autofill
lists. For the sandbox panels the lists start empty (UL §17.4); Tessera adds its own entries with
the panel's templates.

<a id="s8-3"></a>
### 8.3 Layout (BIN, W14)

The list box lays its entries out lazily: any change that affects layout (§8.1 "dirty", a
scroll, a shown or hidden entry when `layout_only_visible` is true, an added or removed entry)
marks the layout dirty and asks for a redraw (for added and removed entries: Probable, the add
and remove routines were not read); the layout runs at the start of the list box's
next draw (before its images and children, UL §9.3 step 5), and only when dirty.

1. **Laid-out list.** The entries to place are all entries, or, when `layout_only_visible` is
   true, only the visible ones, in entry-list order. If the current selection is not among
   them, the selection is cleared (§8.4, with the "new selection" message).
2. **Rows and columns.** Let *n* = the number of entries to place, *mc* = `max_columns`, *mr* =
   `max_rows` (0 when absent).
   - Horizontal: if *mc* < 0: columns = *n*, rows = 1 if *n* > 0 else 0. Otherwise columns = *n*
     if *mc* = 0 else min(*mc*, *n*); rows = 0 if columns = 0, else ⌈*n* ÷ columns⌉; and if
     `max_rows` is set on the element, rows = min(rows, *mr*).
   - Vertical: the same with rows and columns swapped (rows from *mr*, then columns =
     ⌈*n* ÷ rows⌉, capped by `max_columns` when set on the element).
3. **Cell sizes.** Every column width and row height starts at 0. Walk the entries with a
   (column, row) cursor starting at (0, 0); while the cursor is inside the grid, the column's
   width becomes the maximum of the entry widths in it and the row's height the maximum of the
   entry heights in it (a Tree adds indentation to the width, §9.3). Advance the cursor:
   horizontally, next column, and after the last column the first column of the next row;
   vertically, next row, and after the last row the first row of the next column. Entries
   beyond the grid are not placed and keep their old positions.
4. **Content size.** *Cw* = the sum of the column widths, *Ch* = the sum of the row heights. If
   either changed, store them, update both scroll bars (§5.2), and if `notify_on_size_change`
   is true send `Scrollable_scrollablesize_changed` (0x01) with the old sizes.
5. **Placement.** Start at *x* = −*sx* and *y* = −*sy*. If `fill_in_reverse` is true, add
   *W* − (width of column 0) to *x*. If exactly one of `fill_in_reverse` and `flipped_layout` is
   true, add *H* − (height of row 0) to *y*. Let the **cell** of an entry be its column width
   and row height, or, with `ignore_cell_alignment`, the entry's own width and height; the cell
   is used only by the horizontal steps below (BIN, W27). Walk the
   entries again with the (column, row) cursor. For each entry inside the grid whose parent is
   the list box, move it to (*x*, *y*) (a Tree places its parts, §9.3), then advance:
   - **Horizontal**, not at the last column: *x* −= cell width if `fill_in_reverse`, else
     *x* += cell width; next column.
   - **Horizontal**, at the last column: the wrap indent *wi* (0 at the start) grows by
     `wrap_indentation_amount`; *x* = *W* − *wi* − *sx* − cell width if `fill_in_reverse`, else
     *x* = *wi* − *sx*; *y* moves down by the cell height when (`flipped_layout` XOR
     `fill_in_reverse`) is false and up otherwise; first column of the next row.
   - **Vertical**, not at the last row: *y* −= the row's height (from the row heights of step
     3, not the cell) if `fill_in_reverse`, else *y* += it; next row.
   - **Vertical**, at the last row: *wi* grows by `wrap_indentation_amount`; *y* = *H* − *sy* −
     (height of that row) − *wi* if `fill_in_reverse`, else *y* = *wi* − *sy*; *x* moves right
     by the column's width when (`flipped_layout` XOR `fill_in_reverse`) is false and left
     otherwise; first row of the next column. "Height of that row" and "the column's width"
     are the row heights and column widths of step 3, also under `ignore_cell_alignment`.

   So `ignore_cell_alignment` changes only horizontal lists; a vertical list always advances by
   its row heights and column widths. In data `ignore_cell_alignment` is set on one horizontal
   list box, to false (W23).

   Within a row (horizontal) or a column (vertical) only `fill_in_reverse` chooses the
   direction, while the starting corner and the step between rows or columns use both flags as
   given; implementations reproduce these rules as stated, including the combinations that
   step away from the starting corner.

Entries are drawn as ordinary children (UL §9.3), so the list box's box clips them; scrolling
moves them by changing *sx*, *sy*.

<a id="s8-4"></a>
### 8.4 Selection (BIN, W15)

The list box has at most one **selected entry**.

**Select entry *e*** (with or without notification):

1. If *e* is already selected: with notification send `ListBox_same_selection` (0x04) with *e*;
   stop.
2. Find *e*'s index in the laid-out list; if it is not there, *e* = none and the index is −1.
3. Remember the old selection and make *e* the selection.
4. If `selected_item_state_change` is true: request `item_normal_state` on the old entry and
   `item_selected_state` on the new one (§7.3 for Button entries; UL §9.4 otherwise).
5. With notification send `ListBox_new_selection` (0x3C) with the index and *e*.

**Click selection.** On its own `Element_mouse_press` with `SelectLeft` (presses on entries reach
it as the entries' messages pass up, §5.7), if `click_select` is true: select the entry under
the pointer, with notification. Then, if `drag_select` is true, a drag selection is not already
running and the input layer reports the left command still held, capture the pointer to the list
box and start a drag selection.

> **OPEN:** (1) the rule that picks "the entry under the pointer" (Tessera: the first laid-out
> entry whose box contains the point); (2) drag selection while captured, `drag_rollover`, and
> the add and remove notifications. None is needed to draw the §17 panels.

<a id="s8-5"></a>
### 8.5 Census (Confirmed, OBS, 2026-10-06; resolved instances)

| Quantity | Value |
|---|---|
| `max_columns` | 1 on 399 of 453 set; also 12 (24), 5, 6, 4, 2 |
| `horizontal` | false 155, true 104 |
| `click_select` | true 260, false 113 |
| `selected_item_state_change` | true 257, false 107 |
| `item_selected_state` | `highlight` 142; a game state on 7 |
| Vertical scroll bar named | 403 of 582 |
| Templates | `entry_templates` on 305; a single `entry_template` on 12 |

---

<a id="s9"></a>
## 9. Tree (type 10)

<a id="s9-1"></a>
### 9.1 Properties (BIN, W16)

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x00B9 | `UICore_Tree_indent_amount` | Int, px | Indent per level (§9.3) |
| 0x055D, 0x016D | `UICore_Tree_icon_layout`, `UICore_Tree_icon_ID` | DID, element ID | Template of the expand/collapse icon part of a node |
| 0x0A6F | `UICore_Tree_indentelement_ID` | Element ID | Optional indent part inside an entry (§9.3) |
| 0x0759 | `UICore_Tree_indent_elements_for_icon` | Bool, default true | The entry starts after the icon |
| 0x0643 | `UICore_Tree_auto_expand` | Bool | A new node starts expanded when true, collapsed when false (§9.2) |
| 0x041F, 0x0731 | `UICore_Tree_auto_prune`, `UICore_Tree_auto_filter_parent_with_filtered_children` | Bool | Read by the pruning and filtering routines (OPEN below) |

A Tree also reads every ListBox property (§8.1); in data trees use `max_columns` 1 (74 of 91
set) and a vertical scroll bar (100 of 116).

**Completion pass** (§3.4; BIN, W28). After the scrollable steps (§5.1), a Tree sets its
run-time `UICore_ListBox_click_select` and `UICore_ListBox_layout_only_visible` to true. So
every Tree selects on a left press and lays out only visible entries (collapsed nodes' entries
are hidden, §9.2, so they take no row), whatever its layout data says.

> **OPEN:** what pruning (`auto_prune`, true on 66) and filtering
> (`auto_filter_parent_with_filtered_children`, true on 72) do; both act when game code removes
> or filters entries. Not needed for drawing. Tessera rule: no pruning; a filtered-out node hides
> its entry only.

<a id="s9-2"></a>
### 9.2 Nodes and parts

Entries of a tree are **nodes** under an invisible root node. Each node may have: its **entry**
(the element added from a template, §8.2), an **icon** part (created from `icon_layout` and
`icon_ID`; in data a 16 × 16 Field whose states `open` and `closed` set its image), and an
**indent** part (found in the entry by `indentelement_ID`). A node is **expanded** or
collapsed; the root is always expanded.

**Creating a node** (when an entry is added): the node records its entry and its parent node,
starts expanded if `auto_expand` is true and collapsed otherwise, and takes as its indent part
the entry's direct child with the ID `indentelement_ID` (when set and found).

**Expanding or collapsing** a node through the tree: when expanding, first expand its
ancestors (up to, not including, the root) the same way; then record the flag on the node;
request state `open` (0x06) on its icon when expanded and `closed` (0x09) otherwise; show the
icon exactly when the node's entry is visible; then, for each child node, show the child's entry
and icon when the node is expanded and visible and hide them otherwise, applying the same rule
down the child's subtree (a filtered-out node stays hidden; filtering is OPEN, §9.1). Game code
can expand or collapse every node at once (the same steps from the root, applied to every
level).

<a id="s9-3"></a>
### 9.3 Indentation and placement

**Depth** of a node: the number of steps from it up to the root (a top-level node has depth 1).

**Width** used by the list layout (§8.3 step 3) for a node's entry: depth × `indent_amount` +
(icon width, if the node has an icon) + (entry width).

**Placement** at (*x*, *y*) from §8.3 step 5, for a node that is not the root and has an entry:

1. Start with *ex* = *x*. If the node's parent is not the root: *ex* = `indent_amount` + *b*,
   where *b* is the parent's icon *x* if the parent has an entry and an icon; the parent's entry
   *x* plus the parent's indent part *x* (the indent part counted only if it exists) if the
   parent has an entry and no icon; and 0 if the parent has no entry (BIN, W27).
   `indent_amount` is added in every one of these cases. Positions are relative to each
   element's parent (UL §8.1).
2. If the node has an icon: move it to (*ex*, *y*); if `indent_elements_for_icon` is true,
   *ex* += the icon's width.
3. If the node has an indent part: move the entry to (*x*, *y*) and the indent part to
   (*ex* − *x*, its own *y*) inside the entry. Otherwise move the entry to (*ex*, *y*).

So top-level entries start at the column's *x*, and each child level starts `indent_amount`
pixels right of its parent's icon (or of its parent's entry, plus its indent part, when the
parent has no icon).

The width rule counts a top-level node at depth 1, so its width includes one `indent_amount`
that placement does not apply (step 1 leaves a top-level entry at *x*). This is how the client
works (BIN, W27); implementations keep it, because the extra width enters the column width
and so the content size and the horizontal scroll range (§8.3 steps 3–4).

<a id="s9-4"></a>
### 9.4 Selection

A Tree selects like a ListBox (§8.4), with one addition: when the tree's toggle-on-select
switch is on (it is on for every tree when created; no engine code turning it off was found)
and the selected entry belongs to a node, the node is toggled first (a node with children flips
between expanded and collapsed; a node without children is set expanded) and the tree's
laid-out list is rebuilt. This happens on every selection, including a click on the entry that
is already selected (before the "same selection" check).

---

<a id="s10"></a>
## 10. Panel (type 8)

<a id="s10-1"></a>
### 10.1 Properties (BIN, W17)

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x0649 | `UICore_Panel_pages` | Array of `UICore_Panel_page_data` (0x0021) | The page list; every change rebuilds the page map (§10.2) |
| 0x0250 | `UICore_Panel_tab_element` | Element ID (in a page-data struct) | The tab of the page |
| 0x02A1 | `UICore_Panel_page_element` | Element ID (in a page-data struct) | The page |
| 0x06E7 | `UICore_Panel_page_open` | Bool (in a page-data struct) | Open this page at set-up |

<a id="s10-2"></a>
### 10.2 Set-up

When `UICore_Panel_pages` is set (at creation and on every change):

1. Clear the tab→page and page→tab maps and the current page and tab (none).
2. For each entry of the array in order whose element is a page-data struct holding both a tab
   and a page ID: if the tab ID is already in the tab→page map, skip the entry. Otherwise add
   tab→page; if the page ID is already in the page→tab map, skip the rest of the entry;
   otherwise add page→tab. If the tab ID differs from the page ID: if `page_open` is true,
   remember (page, tab) as the
   **page to open** (a later entry with `page_open` true replaces an earlier one); find the tab
   as a descendant of the panel and turn on its mouse-button flag (UL §9.2 step 5), so that tabs
   are clickable without any listener.
3. Open the remembered page (§10.3). If none was remembered, nothing is opened and every page
   keeps its layout visibility.

<a id="s10-3"></a>
### 10.3 Opening a page

To open (page *P*, tab *T*): if *P* is the current page or *T* is the current tab, stop. Make them
current. Then for every (tab, page) pair of the map: find the page and the tab as descendants of
the panel; the pair is **selected** when it is (*T*, *P*).

- Show the page if selected, hide it otherwise.
- Unless the tab is in state `ghosted`, request state `highlight` (0x07) on a selected tab and
  `normal` (0x0B) on the others. For Button tabs this sets or clears `UICore_Button_highlighted`
  (§7.3), so the selected tab shows `highlight` and `highlight_rollover`; tab Buttons in data
  pass their states to their frame-piece children (state flag bit 0, UL §9.4), so the tab art
  changes with them.

Finally send `Panel_NewPage` (0x37).

<a id="s10-4"></a>
### 10.4 Switching by the user

On `Element_mouse_click` (0x1E) or `Element_activated` (0x3D) from any descendant of the panel
whose element ID is a tab in the map (and the page maps back to it), open that tab's page
(§10.3).

<a id="s10-5"></a>
### 10.5 Census (Confirmed, OBS, 2026-10-06; resolved instances)

105 panels: 64 with a page list (1 to 18 pages; 2, 3 and 4 most common), 41 without. Of the 64:
60 open exactly one page at set-up, 2 none, 2 two (the later one wins). The page lists hold 286
entries in all; each names a tab and a page, and all 572 of these IDs (286 tab IDs and 286 page
IDs) are found as descendants of their panel (W23).

---

<a id="s11"></a>
## 11. Menu (type 4) (BIN, W18)

<a id="s11-1"></a>
### 11.1 Properties

| PID | Label | Type | Effect |
|---|---|---|---|
| 0x0205, 0x0383 | `UICore_Menu_popup_layout`, `UICore_Menu_popup` | DID, element ID | The popup template; without a layout the menu's own layout is used |
| 0x02A5 | `UICore_Menu_listbox` | Element ID | The list box inside the popup (a ListBox, found as a descendant of the popup) |
| 0x06EA, 0x0563 | `UICore_Menu_text_item_layout`, `UICore_Menu_text_item` | DID, element ID | Template of a text entry |
| 0x04CC | `UICore_Menu_selection_display` | Element ID | Text part of the menu that shows the selection |
| 0x0829 | `UICore_Menu_default_no_selection_text` | StringInfo | Text shown with no selection |
| 0x0607 | `UICore_Menu_default_selected_item` | Element ID | Entry selected at set-up |
| 0x0606, 0x06EB | `UICore_Menu_open_up`, `UICore_Menu_open_center` | Bool | Popup placement (§11.3) |
| 0x047B | `UICore_Menu_auto_close_on_selection` | Bool, default true | Close after a selection |

`UICore_Menu_open_down` (0x02F9, 3 instances) has no reader in the UI code; opening downward is
the default.

<a id="s11-2"></a>
### 11.2 Set-up (post-creation)

1. Create the popup: the top-level element `popup` of `popup_layout` (or of the menu's own
   layout), created as its own root-level element, hidden, with `UICore_Element_activatable`
   true.
2. Find the list box in the popup. Select the entry whose element ID is
   `default_selected_item`, or no entry when it is absent or not found (and update the
   selection display, §11.4). Set `notify_on_size_change` true on the list box.
3. Give the popup a minimum width (`UICore_Element_min_width`) equal to the menu's width and set
   its `UICore_Element_blocks_clicks` true.
4. Run the Button's post-creation. Record the popup's extra width over the menu and its extra
   height over the list box (used to size the popup to its list; not traced further).

<a id="s11-3"></a>
### 11.3 Opening and closing

On `Element_mouse_click` (0x1E) from the menu or one of its children:

- **Open** popup: hide it, mark closed, clear `UICore_Button_highlighted`, send `Menu_closed`
  (0x0C).
- **Closed**, the list box has entries and the popup exists: *x* = the menu's screen *x*; with
  `open_center`, *x* −= half(popup width − menu width). *y* = the menu's screen *y* minus the
  popup's height with `open_up`, otherwise the menu's screen *y* + its height − 1 (its last
  row). Move the popup to (*x*, *y*),
  show it, bring it forward, mark open, set `UICore_Button_highlighted`, send `Menu_opened`
  (0x15).

`Element_mouse_over_top` sets `highlighted` to the open mark, so an open menu stays
highlighted.

<a id="s11-4"></a>
### 11.4 Selection display

After a selection (or at set-up), the menu finds `selection_display` as a descendant: it must be
Text. If the list box has a selected entry, the display takes the text of the entry's Text part;
otherwise it shows `default_no_selection_text` if set, and is cleared otherwise. When
`selection_display` is 0 (absent, or set to the ID 0) or names no Text descendant, no display is
updated (BIN, re-read with W28). With `auto_close_on_selection` the popup closes.

> **OPEN:** who creates the popup's text entries from `text_item_layout` and `text_item` (game
> code is assumed: no engine caller was found), and how the popup is sized to its list. Not
> needed for the closed menus of the §17 panels.

Census (OBS, W23): 195 instances, all with a popup layout; 194 have a rollover state;
`auto_close_on_selection` false on 14 (absent, so true by default, on 181); `open_down` set on 3;
`open_center` true on 2; `default_selected_item` and `default_no_selection_text` set on none, so
at set-up every menu has no selection and its selection display is cleared.

---

<a id="s12"></a>
## 12. Viewport (type 11) and `TargetViewport` (W19)

A Viewport renders a 3D scene into its box. The engine type occurs once; the portraits of the
vitals panels (UL §17 P4, P5) are the game type `TargetViewport` (35 instances), which carries
the same properties.

| PID | Label | Type | Dictionary default | Data |
|---|---|---|---|---|
| 0x03D1 | `UICore_Viewport_uiscene` | DID (0x21, UI scene) | — | set on 35 of the 36 instances, 9 distinct scenes |
| 0x02FA | `UICore_Viewport_camera` | Enum (camera mapper 0x230000B9) | — | set on 35 of 36, 6 distinct values (one on 28) |
| 0x0074 | `UICore_Viewport_AmbientLight` | Colour | 0xFF000000 | |
| 0x0073 | `UICore_Viewport_FakeShadowColor` | Colour | 0xFF7F7F7F | |
| 0x0651 | `UICore_Viewport_UseShadows` | Bool | false | |
| 0x0AFF | `UICore_Viewport_UseParticles` | Bool | false | |
| 0x0384 | `UICore_Viewport_UseFog` | Bool | false | |
| 0x069A, 0x034D | `UICore_Viewport_FogNearDistance`, `…FogFarDistance` | Float | 100, 500 | |
| 0x06EC | `UICore_Viewport_FogColor` | Colour | 0xFFFFFFFF | |
| 0x01BD | `UICore_Viewport_BloomIntensity` | Float 0–1 | 0.3 | 0.05 in data |
| 0x069B | `UICore_Viewport_BloomCompensationAmount` | Float 0–1 | 1.0 | |
| 0x01BE | `UICore_Viewport_DeshadeStrength` | Float 0–1 | 0.0 | 0.08 in data |
| 0x03D0 | `UICore_Viewport_RimLightingIntensity` | Float 0–32 | 0.0 | 1.0 in data |
| 0x042B | `UICore_Viewport_SkyGlowAmount` | Float 0–1 | 0.0 | |
| 0x0176, 0x00C9 | `UICore_Viewport_RenderToTextureWidth`, `…Height` | Int 1–256 | 256 | 62 on 2 |
| 0x0652 | `UICore_Viewport_RenderToTextureBackgroundColor` | Colour | 0xFF000000 | |
| 0x0520 | `UICore_Viewport_RenderToTextureIndex` | Int 0–1 | −1 | |
| 0x0B93 | `UICore_Viewport_RenderInParent` | Bool | false | |

> **OPEN:** how a Viewport draws (scene loading, camera placement, lighting, the render-to-
> texture path and how the result is composed into the panel surface), and what
> `TargetViewport` adds (the target's figure). This is rendering and game code, outside the
> traced widget code. Tessera rule (UL §17.4): the portrait is optional for M5; draw the M4
> figure into the element box if available, otherwise draw nothing there. The checklist judges
> the panel frame around it.

---

<a id="s13"></a>
## 13. What the checklist rows need

| Row | Widgets involved | Build from |
|---|---|---|
| **P3-A** (toolbar) | The experience fill `LevelMeterFill` is the game type `LevelProgressMeter` (1015 × 11). It carries child-meter properties (`child_meter` true, direction Left, Alphablend, `move_fill` false) and four states (`normal`, which is its default state, `bonus`, `xp_suppressed`, `xp_suppressed_with_bonus`), each of which sets `child_image`. Its two children are engine child meters of the same size (direction Left, Alphablend, `move_fill` false: the rest-experience overlays). Siblings: the glass overlay, a plain Field of the fill's size, and a Text. The quickslot ListBox (`horizontal` true, `max_rows` 1, `max_columns` 12; the game type `QuickslotBar` fills it). (OBS, W22) | §4.3, §4.6 (sandbox position: a fixed value set by Tessera), §8.3 (12 columns in one row: entries beyond 12 are not placed); OPEN below on the game type |
| **P4-A** (player vitals) | Ten child meters (OBS, W22). Current morale, current power, dread and rage: direction Left, Normal draw mode. Temporary morale and temporary power: direction Left, Alphablend, and five `vitalmeter_*` states that set `child_image`. The light, buff and shadow meters: Left; the debuff meter: Right; all four Alphablend. The `TargetViewport` portrait. | §4.3 (with *p* = 1 the fill covers the whole meter: the checklist's "meters full"); temporary meters keep their layout position 0 (no fill) unless the sandbox sets one; §12 |
| **P5-A** (target vitals) | Six child meters (OBS, W22). Morale, temporary morale, rage, power and temporary power: direction **Right** (they fill from the right edge); dread: direction Left. Current meters use Normal, temporary meters Alphablend; every meter except dread has the five `vitalmeter_*` states. The `TargetViewport` portrait. | §4.3 directions 3 and 4; §12 |
| **P7-A** (options) | The options tab Panel `OptionsTabPanel` with 11 page entries; only the Graphics entry has `page_open` true, so the Graphics page opens at set-up and its tab is highlighted (§10.2–§10.3). Each page holds a Tree (`indent_amount` 8, `max_columns` 1; empty until entries are added, §8.2) with a vertical, proportional scroll bar (`min_widget_size` 20, decrement button at the top, increment at the bottom). Entry templates of the same layout: the slider, a horizontal Scrollbar in slider mode, not proportional, so its 16 × 16 `widget` keeps its size, whose two 16 × 16 arrow Buttons are siblings in the template and become the bar's children at its two ends on its first frame update (§6.2, §6.3); the check box, a Button with `boolean_button` and `supports_ghosted_highlight` true. (OBS, W22) | §10, §9, §6.2–§6.5, §6.7, §7.1, §7.3 |
| **P8-S** (social) | Four Panels (OBS, W22). The social tab Panel `Social_Panel` (5 entries): the grouping page opens at set-up, so its tab is `highlight` and the other four are `normal`. Nested panels open their own default pages: `GroupingPanel` the who page, `FriendsPagePanel` the friends page, `RaidLocksPanel` the chest-lock page. Two GroupBoxes (§3.4) holding column-header Buttons: `FriendsHeader` (7 Buttons, `default_button` the member column) and `IgnoreHeader` (2 Buttons, the member column), both with `allow_button_click_on_selected_button` true; each highlights its default button in the completion pass. Four Menus (§11), all closed at set-up with no selection: `WhoClassMenu` and `WhoDisplay_DisplayTypeMenu` on the who page, `LFQuestMenu` and `LFQuestClassMenu` on the hidden quest page; `WhoClassMenu` and `LFQuestClassMenu` name selection display 0 (§11.4). Tabs switch by click. | §10.2–§10.4, §7.3, §3.4, §11.2–§11.4 |
| **P12-A** (plugin manager) | One Tree with a vertical scroll bar; the about pane's description Text with a scroll bar; the options pane's ListBox (`max_columns` 1) with a scroll bar; one Menu in the about footer (`open_down` set, `auto_close_on_selection` false, selection display 0), closed at set-up; no Panel: the about and options tabs are plain Buttons. (OBS, W22) | §9, §8, §6, §11 |

> **OPEN (P3):** which class the game type `LevelProgressMeter` is created with (game code
> registers its factory; whether it derives from Meter was not traced). If it is a Meter, its
> `normal` state's `child_image` gives it fill parts at post-creation (§4.3) and it draws the
> experience fill; if it is not, it draws nothing of its own. Tessera rule: build it as an engine
> Meter (§4), so the experience fill draws from its default state's image.

> **OPEN (P12):** which info tab the plugin manager highlights and which pane it shows when it
> opens (the two tabs are Buttons switched by game code; both panes are visible in the layout).
> Tessera rule until the owner's reference settles it: the about tab highlighted (§7.3 request
> `highlight`), the options pane hidden.

---

<a id="s14"></a>
## 14. Invariants

- **WI1** For every resolved Meter with `child_meter` true, `child_direction` ∈ {1, 2, 3, 4},
  `child_image` names an existing 0x41 surface and `child_drawmode` ≠ 0 (379 of 379; so every
  child meter in data creates its parts).
- **WI2** (child meter, *m* false, *o* = 0, 0 ≤ *p* ≤ 1) The fill lies inside the meter:
  0 ≤ *fx*, *fx* + *fw* ≤ *W*, and *fw* = trunc(*W*·*p*) for directions 3 and 4; for direction 4
  *fx* = 0 and for direction 3 *fx* + *fw* = *W*. Likewise vertically.
- **WI3** (child meter, *m* false) The fill image's position relative to the meter, *fx* + *ix*,
  is *o* for directions 3 and 4 whatever *p* is (the image never moves).
- **WI4** (frame meter) The frame index is in [0, *n* − 1] for every *p* in [0, 1].
- **WI5** (scroll bar) Value-to-stop and stop-to-value are inverse on stops: for *N* ≥ 2 and
  every *i* in [0, *N* − 1], value-to-stop(stop-to-value(*i*)) = *i*, in both endpoint modes.
- **WI6** (scroll bar, proportional, `min_widget_size` ≤ *L*, 0 ≤ *q* ≤ 1, endpoints included)
  The thumb lies in the track: *tt* ≤ *a* and *a* + length ≤ *tb* on a vertical bar (*tl*, *tr*
  on a horizontal one); *a* = *tt* at *q* = 0 and *a* + length = *tb* at *q* = 1.
- **WI7** (scrollable) After §5.2 with *C* > *V*, 0 < *f* < 1; with *C* ≤ *V*, *f* = 1, the offset
  is 0 and the bar is disabled.
- **WI8** (scroll-to with `keep_in_bounds`) 0 ≤ *sx* ≤ max(0, *Cw* − *W*) and likewise for *sy*.
- **WI9** (list layout) rows × columns ≥ *n* unless a set `max_rows` (horizontal) or
  `max_columns` (vertical) cap applies; the content size equals the sum of the cell sizes.
- **WI10** (list layout, vertical, one column, no reverse, no flip, no scroll) Entry *k* is placed
  at *y* = the sum of the heights of entries 0 … *k* − 1, *x* = 0.
- **WI11** (panel) After set-up with exactly one `page_open` entry, exactly one page of the map
  is visible and exactly one tab Button has `UICore_Button_highlighted` true (unless a tab was
  ghosted).
- **WI12** (state requests, §7.3; pairs are (`highlighted`, `ghosted`)). With
  `supports_ghosted_highlight` false: from (false, false), `highlight` gives (true, false); from
  (true, false), `normal` gives (false, false); from (false, true), `normal` gives (false, false)
  (step 2 changes nothing, so step 3 clears `ghosted`); from (true, true), `normal` gives
  (false, true) (step 2 changed *H* and stops); from (false, true), `highlight` gives
  (true, true); from (true, false), `ghosted` gives (true, true). With
  `supports_ghosted_highlight` true: from (true, false), `ghosted` gives (false, true); from
  (false, true), `highlight` gives (true, false). From (false, false), `normal` changes no flag
  and switches the state directly (step 4), whatever the second flag is.
- **WI15** (GroupBox, §3.4) After the completion pass of a GroupBox whose `default_button` *D*
  is found and whose Buttons are neither ghosted nor `supports_ghosted_highlight`, *D* has
  `highlighted` true. After any sequence of clicks on its Buttons (none ghosted, none
  `boolean_button`), exactly the Button named by the effective `selected_button` has
  `highlighted` true among the Buttons the GroupBox has selected so far.
- **WI16** (completion pass, §5.2) A list box that has no entries after its tree is created and
  names a vertical scroll bar with `hide_when_disabled` true leaves that bar disabled and hidden.
- **WI17** (tree, §9.3; `indent_amount` ≥ 0, every node has an entry, no node has an indent
  part, one column) A node at depth *k* has its entry placed at *x* = *x₀* + (*k* − 1) ×
  `indent_amount` + (its icon width if it has an icon and `indent_elements_for_icon` is true),
  where *x₀* is the column's start (−*sx*); so the entry's right edge is at most *x₀* + the width
  the layout used for it (*k* × `indent_amount` + icon width + entry width), with at least
  `indent_amount` to spare.
- **WI13** Every Scrollbar instance has a direct child with element ID 0x16 (792 of 792), and no
  instance has a child with ID 0x03, 0x09, 0x1D or 0x20.
- **WI14** Every child-meter image (379 on meters, 990 in states) and every frame of every frame
  meter (24,030) is a 0x41 render surface that exists in the install.

---

<a id="s15"></a>
## 15. Worked synthetic examples (values invented)

<a id="s15-1"></a>
### 15.1 Child meter

Meter 164 × 9, *o* = 0.

| Direction, *m* | *p* | Fill (*fx*, *fy*, *fw*, *fh*) | Image in fill (*ix*, *iy*, *iw*, *ih*) | Visible part of the image (meter coordinates) |
|---|---|---|---|---|
| 4, false | 0.75 | (0, 0, 123, 9) | (0, 0, 123, 9) | columns 0–122 of the image at meter *x* 0–122 |
| 3, false | 0.75 | (41, 0, 123, 9) | (−41, 0, 164, 9) | image columns 41–163 at meter *x* 41–163 |
| 4, true | 0.75 | (−41, 0, 164, 9) | (0, 0, 164, 9) | image columns 41–163 at meter *x* 0–122 |
| 4, false | 1.0 | (0, 0, 164, 9) | (0, 0, 164, 9) | the full width |

trunc(164 × 0.75) = 123; trunc(0 − 0.25 × 164) = −41.

<a id="s15-2"></a>
### 15.2 Frame meter

60 frames: *p* = 0.5 → trunc(59 × 0.5 + 0.5) = 30; *p* = 0.991 → trunc(58.469 + 0.5) = 58;
*p* = 1 → 59.

<a id="s15-3"></a>
### 15.3 Vertical scroll bar and its list

Bar 10 × 447 with a 10 × 10 decrement and a 10 × 10 increment button, proportional,
`min_widget_size` 20. List box 419 × 433, one column, 40 entries of 30 px: *Ch* = 1200.

1. §5.2: *C* = 1200 > *V* = 433, enabled; *f* = 433 ÷ 1200 = 0.36083.
2. Track: (10, 10, 0, 437) on the vertical axis (decrement button at (0, 0), increment button
   at (0, 437)); *L* = 437 − 10 = 427.
3. Thumb length *k* = trunc(427 × 0.36083 + 0.5) = trunc(154.58) = 154; max(154, 20) = 154.
4. Travel *v* = 427 − 154 = 273. At *q* = 0: *a* = 10. At *q* = 0.5: *a* = trunc(136.5) + 10 = 146.
   At *q* = 1: *a* = 283, and 283 + 154 = 437 = *tb*.
5. Line step = min(1200 ÷ 40, 433) = 30; page step = 433. One `ScrollDown` sends
   `Scrollbar_line_increment`; the list scrolls to *sy* = 30 (clamped to [0, 767]) and
   synchronises the bar to *q* = 30 ÷ 767 = 0.03911.
6. Dragging the thumb 50 px down from *q* = 0: 0 + 50 ÷ 273 = 0.18315; the bar sets that
   position and sends `Scrollbar_new_position` with (183, −1); the list sets
   *sy* = trunc(767 × 0.18315) = 140.

<a id="s15-4"></a>
### 15.4 Stops

*N* = 100, endpoints included: *q* = 0.37 → *i* = trunc(99 × 0.37 + 0.5) = trunc(37.13) = 37 →
*q* = 37 ÷ 99 = 0.37374. *N* = 4 with `stops_dont_include_endpoints`: *q* = 0.6 → *i* =
trunc(2.4) = 2 → *q* = 2.5 ÷ 4 = 0.625, and the thumb's centre sits at the value.

<a id="s15-5"></a>
### 15.5 Horizontal list with a row cap

14 entries of 35 × 35, `horizontal` true, `max_columns` 12, `max_rows` 1 (set): columns = 12,
rows = min(⌈14 ÷ 12⌉, 1) = 1. Entries 0–11 are placed at *x* = 0, 35, …, 385, *y* = 0; entries 12
and 13 are outside the grid and not placed. *Cw* = 420, *Ch* = 35.

<a id="s15-6"></a>
### 15.6 Tabs

A panel with pages [(tab *A*, page *a*, open false), (tab *B*, page *b*, open true), (tab *C*,
page *c*, open true)]: set-up opens (*c*, *C*): page *c* shown, *a* and *b* hidden, tab *C*
highlighted, *A* and *B* normal, `Panel_NewPage` sent. A click on *A* opens (*a*, *A*). A second
click on *A* does nothing (*A* is current).

<a id="s15-7"></a>
### 15.7 State requests

Pairs are (`highlighted`, `ghosted`); *S* = `supports_ghosted_highlight`.

| *S* | Start | Request | Steps taken (§7.3) | Result |
|---|---|---|---|---|
| false | (false, true) | `normal` | step 2: *H* already false, no stop; step 3: *g* false ≠ true, clear `ghosted`, stop | (false, false) |
| false | (true, true) | `normal` | step 2: clear *H*; *S* false, stop | (false, true) |
| false | (false, true) | `highlight` | step 1: set *H*; *S* false, stop | (true, true) |
| true | (false, true) | `highlight` | step 1: set *H*; *S* true, go to step 3: *g* false ≠ true, clear `ghosted`, stop | (true, false) |
| true | (true, false) | `ghosted` | step 2: clear *H*, go on; step 3: *g* true, set `ghosted`, stop | (false, true) |
| false | (false, false) | `normal` | nothing changes in steps 1–3; step 4: switch to `normal` directly | (false, false) |

<a id="s15-8"></a>
### 15.8 GroupBox

A GroupBox with Buttons *A*, *B*, *C* (none ghosted, *S* false), `default_button` = *B*.

1. Completion pass: run-time `selected_button` = *B*; the recorded selection is 0, so nothing is
   requested on an old element; `highlight` on *B*: *B* = (true, false). Recorded: *B*.
2. Click on *C* (`Button_click` from *C* bubbles to the GroupBox): *C* ≠ *B*, so
   `selected_button` = *C*: `normal` on *B* → (false, false); `highlight` on *C* → (true, false).
   The message continues to the GroupBox's ancestors.
3. Click on *C* again with `allow_button_click_on_selected_button` false: `highlight` on *C*
   (no flag changes, state switched to `highlight` directly), then the GroupBox answers stop:
   no ancestor or game-code handler sees this click. With the property true, the click
   continues.

<a id="s15-9"></a>
### 15.9 Tree indentation

`indent_amount` 8, icons 16 × 16, `indent_elements_for_icon` true, no indent parts, *sx* = 0.
Top-level node *N* (depth 1) with entry width 100; its child *M* (depth 2) with entry width 90.

- Widths for the layout: *N* 1 × 8 + 16 + 100 = 124; *M* 2 × 8 + 16 + 90 = 122. Column width 124.
- Placement of *N* at *x* = 0: icon at 0, entry at 16 (right edge 116, 8 short of 124).
- Placement of *M* at *x* = 0: *ex* = 8 + *N*'s icon *x* 0 = 8; icon at 8, entry at 24 (right
  edge 114).

<a id="s15-10"></a>
### 15.10 Options slider track

The options slider (§13 P7): bar 322 × 16, `widget` 16 × 16, two 16 × 16 arrow Buttons adopted
on the first frame update. Track (§6.3): (16, 16, 306, 0); horizontal *L* = 306 − 16 = 290. Not
proportional, so the thumb stays 16 wide; travel *v* = 290 − 16 = 274. At *q* = 0.5:
*a* = trunc(274 × 0.5) + 16 = 153. Jump to the pointer at bar *x* = 100 (§6.7 step 4):
*px* = 100 − 16 − 8 = 76, inside [0, 273]; *d* = 274 − 1 = 273; value 76 ÷ 273 = 0.27839.

---

<a id="s16"></a>
## 16. Corrections to, and agreements with, other specs (not edited here)

| Target | Statement there | Finding (this spec) | Basis |
|---|---|---|---|
| `ui-layout.md` §5.9 OPEN, §16 item 6 | Behaviour of the remaining engine-level properties and of each widget class not traced | Answered for GroupBox (§3.4), Meter, Scrollable, Scrollbar, ListBox, Tree, Panel, Menu (§4–§11) and for the Button additions (§7); ColorPicker and Browser are OPEN with a Tessera rule (§3.5); Viewport drawing (§12) and Dragbar and ResizeBar beyond UB remain outside this spec | BIN |
| `ui-layout.md` §9.2 step 3 | "The engine types used by the layouts … have engine widgets" | **Agreement**, with the table of §3.1; types 3, 6, 13 and 19 have no factory | BIN |
| `ui-layout.md` §6.6 OPEN (which non-root elements own a surface) | — | One more case: alpha-test meters ask for their own surface at post-creation (§4.5) | BIN |
| `ui-behaviour.md` §4.7 OPEN | Whether an unhandled scroll message reaches a scrollable ancestor; how far one wheel step scrolls | A scrollable acts on wheel messages without checking the sender (so descendants' wheel messages reach it, Probable) and turns one wheel step into one line step: for lists, content size ÷ rows, at most one view (§5.6, §5.7, §6.8) | BIN |
| `ui-behaviour.md` §4.3 OPEN | Per-type mouse-visible rules | Not answered here. Related facts: a Panel turns on the mouse-button flag of its tabs (§10.2); a slider-mode scroll bar turns off its thumb's flag (§6.7); meter parts are `mouse_invisible` (§4.3) | BIN |
| `ui-behaviour.md` §5.2, `keymap.md` §12 (and the input-bridge gap of the M5 critique, item A1) | Which code pushes `ScrollableControls_Mouse` | Scrollables and scroll bars push it (key 0x2, priority 0) for the hovered element while the pointer is over them (§5.7, §6.9); Tessera's input bridge should make its wheel actions active the same way | BIN |
| `ui-layout.md` §9.2 | Creation ends with post-creation (step 8) | One more step for a top-level element: after its whole tree is created, a completion pass runs over the tree (§3.4); scrollables, Trees, Buttons and GroupBoxes act in it (§5.1, §9.1, §7.3, §3.4) | BIN |
| `ui-behaviour.md` §4.3 (mouse-visible writers) | Type rule, listener registration, direct code | In the completion pass a scrollable that finds a scroll bar sets its own mouse-visible flag (§5.1); this is a direct-code write | BIN |
| `ui-layout.md` §17.4 P4, P5 | Meters full in the sandbox | With *p* = 1 a child meter's fill covers the meter for every direction (§15.1) | BIN |

---

<a id="s17"></a>
## 17. Open questions (summary)

Every open item has an in-section `> **OPEN:**` callout (item 3 is closed and kept for
numbering).

1. Negative or zero child-meter fill sizes (§4.3).
2. The exact effect of the alpha-test threshold (§4.5).
3. *Closed in revision 1:* the completion pass runs §5.2 once for every scrollable, so an empty
   list's bar is disabled and, with `hide_when_disabled`, hidden (§5.1, §5.2).
4. The entry-under-pointer rule, drag selection, `drag_rollover` and the add and remove
   notifications of list boxes (§8.4).
5. Menu text-entry creation and popup sizing (§11.4).
6. Viewport and `TargetViewport` drawing (§12).
7. The plugin manager's initial info tab (§13).
8. The lookup boundary of the nearest-element search (§2.3).
9. The input-layer condition that suppresses line and page messages (§5.5).
10. The Button flags that exclude double clicks or suppress presses (§7.1).
11. Tree pruning and filtering (§9.1).
12. What the ColorPicker and Browser classes do (§3.5).
13. The exact pop path of the `ScrollableControls_Mouse` entry a scrollable pushes for a hovered
    descendant (§5.7).
14. Which class the game type `LevelProgressMeter` is created with (§13, P3).

Outside this spec: the Text class's own scrolling (Text is a Scrollable, §3.2, but its line
layout belongs to FO). Unused in data and so not pursued: any reader of
`UICore_Scrollbar_stop_locations` beyond recording it (§6.1).

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First complete draft (status Review), answering the M5 completeness critique items A2 and C1: factory and class table (§3); Meter child, frame and alpha-test modes with exact fill geometry per direction, position, goal and smooth movement (§4); Scrollable offsets, scroll-bar update, scroll-to clamp, line and page steps, wheel and hover (§5); Scrollbar parts, track, thumb geometry, value mapping, stops, arrow buttons, paging, dragging with the 100-pixel rule, slider mode, wheel, disabled and ghosted (§6); Button auto-repeat, click on release, boolean toggle and state-request translation (§7); ListBox templates, grid layout and selection (§8); Tree nodes, indentation, icons and expand (§9); Panel pages and tabs (§10); Menu popup (§11); Viewport properties (§12); checklist mapping (§13); the `ScrollableControls_Mouse` push on hover (§5.7, §6.9); invariants WI1–WI14 and worked examples. OBS + BIN, client 4906.0071.1794.4035. |
| 2026-10-06 | **Revision 1** (answers the review of the same day; status stays Review). **Blocking 1:** new §3.4 GroupBox (selection change, completion-pass default, clicks with the stop answer, census) and §3.5 ColorPicker and Browser with an OPEN and a Tessera rule (§17 item 12); §3.2 rows; §13 P8-S names the four Panels, the two GroupBoxes and the four Menus; §16 row 1. **Blocking 2:** §7.3 states that the stops apply only when that step changed `highlighted` (BIN re-trace, W26); WI12 extended; §15.7. **Blocking 3:** provenance rows W22 (panel composition, OBS) and W23 (censuses of §7.2, §8.3, §10.5, §11.4, OBS); W11 and W18 point to W23. **Minor:** (a) §10.5 counts 286 entries and 572 IDs; (b) §6.6 uses *a* and *len*; (c) §5.7 OPEN on the pop path (§17 item 13); (d) §9.3 step 1 (indent added in every case, parent without entry), the width reserve explained, §8.3 cell use under `ignore_cell_alignment` (vertical lists unaffected). **Also found (BIN, same build):** the completion pass (§3.4, §5.1, §9.1, §7.3; W28), which closes the §5.2 OPEN (§17 item 3) and attaches scrollables to their bars as listeners; Trees force `click_select` and `layout_only_visible` true; descendant lookup order (§2.3); the track's mutual test for arrow buttons (§6.3); selection display 0 (§11.4). **Corrected:** §13 P7 sliders do have arrow buttons (adopted siblings, §6.2); §13 P3 has no cap pieces, and the `LevelProgressMeter` element's own properties and states are given, with an OPEN on its class (§17 item 14); §11.4 census (194 of 195 menus have a rollover state). New W24–W28, WI15–WI17, §15.8–§15.10, §16 rows for UL §9.2 and UB §4.3. |
