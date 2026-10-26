<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI engine widget classes: GroupBox, Meter, Scrollable, Scrollbar, ListBox, Tree, Panel, Menu and Viewport; engine dialog boxes and context menus

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review by spec-review:spec-r8:2 (independent session): passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (revision 8, 2026-10-23, spec writer, package spec-r8: the round-7 implementer closing note of §18.4 filed and answered; item 2 of the revision-7 band answer replaced (game-band release-class bindings fire at the release of a key pressed before the modal entry, `keymap.md` §7.3.1 round 8); review note N3 answered by a pointer in revision-5 answer item 3; no new research, no fact about the original changed; section numbers and anchors unchanged; awaiting an independent review; see the change log). Before: **Accepted** (revision 7, 2026-10-23: independent review by spec-review:spec-ui-r7:1 (independent session), which wrote no part of it: passed; see the review record). Before: **Revised** (revision 7, 2026-10-23, spec writer: the two round-6 implementer OPENs of §18.4 filed and answered as Tessera rules (the modal entry made at once by a hold-keeping push, KM §3.3; focus-cycle and editing keys outside the pairing rule, with a precision under a modal dialog); new anchors `s18-4-band`, `s18-4-edit`; §17 item 29; no new research, no fact about the original changed; awaiting an independent review; see the change log). Before: **Accepted** (revision 6, 2026-10-23: independent review by spec-review:spec-ui-r6:1 (independent session), which wrote no part of it: passed; see the review record). Before: **Revised** (revision 6, 2026-10-23, spec writer: the non-blocking notes N1–N3 of the revision-5 review answered in §18.4 by pointers only (KM §7.2 and §7.8.6 for repeats, the 256 and 64 tracking bounds next to "no release arrives without its press", pointers from the table's release rows and the revision-4 `EscapeKey` sentence to the pairing rule; new anchors `s18-4-pairing`, `s18-4-bounds`); no new research, no fact or rule changed; awaiting an independent review; see the change log). Before: **Accepted** (revision 5, 2026-10-23: independent review by spec-review:spec-ui-r5:1 (independent session), which wrote no part of it: passed; see the review record). Before: **Revised** (revision 5, 2026-10-23, spec writer: the round-4 implementer OPENs of §12.4 and §18.4 filed and answered as Tessera rules (holds from before a modal dialog, the plugin key broadcast, press and release pairing), §17 item 28; no new research, no fact about the original changed; awaiting an independent review; see the change log). Before: **Accepted** (2026-10-23, independent review by spec-review:spec-ui-r4:3 (independent session), which wrote no part of the round-4 revision: passed CLEAN_ROOM.md §5.3, every item; contamination screen clean; see the review record). Before: **Revised** (revision 4, 2026-10-23: the round-3 implementer OPENs of §12, §18 and §19 filed and answered; new §11.3 deactivation rule, new §12.5, provenance rows W33 and W34, BIN revision 4; review round 1 fixes of §2.4 and §11.3; awaiting independent review; see the change log). Before: **Accepted** (2026-10-23: revision 3 passed an independent review after round 5; see the review record). Revision 3 of 2026-10-23: §12 viewport and `TargetViewport` drawing (§12.1–§12.4), new §18 engine dialog boxes and §19 context menus, provenance rows W30–W32, BIN revision 3; round-4 wording fixes of §18.2, §19.3, §19.4 and W29; round-5 wording fixes of §19.2, §18.6, W30 and the BIN revision-3 cell. Before: **Accepted** (2026-10-14: revision 2 passed an independent review; see the review record). Revision 2 (2026-10-14): §6.4 implementer OPEN answered, new provenance row W29, §6.10 census row, §17 item 18; no behaviour already implemented changes. Before: Accepted (2026-10-06: accepted after revision for decisions 0014/0015) |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent); revision 6: spec-review:spec-ui-r6:1 (independent session), which wrote no part of it, 2026-10-23: passed, Accepted; revision 5: spec-review:spec-ui-r5:1 (independent session), which wrote no part of it, 2026-10-23: passed, Accepted; revision 4: spec-review:spec-ui-r4:3 (independent session), which wrote no part of it, 2026-10-23, after the round-2 fixes: passed, Accepted; spec-review:spec-ui-r4:2 (independent session), which wrote no part of it, 2026-10-23, after the round-1 fixes: changes requested (§18.2 cross-owner order; see the change log), status stays Revised; spec-review:spec-ui-r4:1 (independent session), which wrote no part of it, 2026-10-23: changes requested (§2.4 message table, §11.3 wording; see the change log), status stays Revised; revision 3: spec-review:spec-ui-input:2 (independent session), which wrote no part of it (2026-10-23); revision 2: an independent dirty-room review session that wrote no part of it (2026-10-14); revision 7: spec-review:spec-ui-r7:1 (independent session), which wrote no part of it, 2026-10-23: passed, Accepted; package spec-r8 revision: spec-review:spec-r8:2 (independent session), which wrote no part of it, 2026-10-23: passed, Accepted |
| **Review record** | **Review 2026-10-23** of the package spec-r8 changes (uncommitted diff) by spec-review:spec-r8:2 (independent session), a session that wrote no part of them: **passed**, CLEAN_ROOM.md §5.3 every item on the changed text; contamination screen clean (no code, pseudo-code, disassembly prose, copied prose, game text, keys or hostnames); relative links and anchors of the seven owned files resolve; every answer is labelled a Tessera rule or Decision with no client fact added. Checked: the revision-5 answer item 3 pointer (N3), the replaced revision-7 band answer item 2 and the closing-note answer agree with `keymap.md` §7.3.1 and §7.4 round 8. Earlier: **Review 2026-10-23 of revision 7** (spec-review:spec-ui-r7:1 (independent session), Clean-Room-Role: review; wrote no part of the revision or of the code): **passed**; status Revised → Accepted. Checked against CLEAN_ROOM.md §5.3, every item, on the uncommitted worktree diff against HEAD; contamination screen clean (no code, pseudo-code or listing-shaped text, no disassembly prose, no game text, keys, hostnames or traffic, nothing outside decision 0034 §2); every added statement is a labelled Tessera rule or a pointer to Accepted text, no new observation of the original claimed; all four round-6 implementer OPENs of the package report (keymap §3.3 / ui-widgets §18.4 band push, focus-cycle and editing keys, S6 faces in use, LPA §14.2.6 round-5 answer 2) are filed and answered; relative links and anchors of the five package specs resolve (own check). Cross-checks: the band answer agrees with `keymap.md` §3.3 hold-keeping push (anchor `s3-3-keep`), with revision-5 answer item 1 (holds from before end at their own release; the entry taking the focus still ends them) and with KM §3.5 at the pop; the editing-key answer agrees with LPA §13.13 (start and end routed on their own; broadcast only when not consumed), with the revision-4 modal rule's exceptions and with revision-5 answer item 2 and client host and UI core (b); §17 item 29 names the untraced question. Non-blocking: (N3) revision-5 answer item 3, second bullet, says a release whose press went past every dialog reaches its receiver; under a modal dialog the band end now keeps game-band release-class bindings from firing at that release (revision-7 band answer item 2); a pointer from item 3 to that item would help readers. Before: **Review 2026-10-23 of revision 6** (spec-review:spec-ui-r6:1 (independent session), Clean-Room-Role: review; wrote no part of the revision or of the code): **passed**; status Revised → Accepted. Checked against CLEAN_ROOM.md §5.3, every item, on the uncommitted worktree diff against HEAD; contamination screen clean (pointers and wording only; no code, pseudo-code or listing-shaped text, no disassembly prose, no game text, keys, hostnames or traffic, nothing outside decision 0034 §2); no fact about the original and no Tessera rule changed; the notes N1–N3 of the revision-5 review are answered; relative links and anchors (new `s18-4-pairing`, `s18-4-bounds`) of the three package specs resolve (own check). Cross-checks: KM §7.2 "Presses that are not resolved" says an OS auto-repeat press never starts, resolves or ends a hold and KM §7.8.6 gives its Repeat offers (N1); the 256 and 64 bounds next to "no release arrives without its press" are those of the precisions bullet, and 256 is KM §7.3's hold limit (N2); the new paragraph after the §18.4 table and the revision-4 parenthesis agree with the pairing rule of item 3 (a dialog consumes a release exactly when it consumed that key's press), and the table rows stay labelled as the original's behaviour (N3). No blocking or non-blocking items. Before: **Review 2026-10-23 of revision 5** (spec-review:spec-ui-r5:1 (independent session), Clean-Room-Role: review; wrote no part of the revision or of the code): **passed**; status Revised → Accepted. Checked against CLEAN_ROOM.md §5.3, every item, on the uncommitted worktree diff against HEAD; contamination screen clean (no code, pseudo-code or listing-shaped text, no disassembly prose, no game text, keys, hostnames or traffic, nothing outside decision 0034 §2); every added statement is a labelled Tessera rule or a pointer to Accepted text, no new observation of the original claimed; every round-4 implementer OPEN of the package's report for this spec is filed and answered; relative links and anchors of the four package specs resolve (own check). Cross-checks: the §18.4 answers agree with the modal rule of W30, the revision-4 Tessera rule (§17 item 26), LPA P48 and §13.13 (broadcast only for key actions the focus control or active window did not consume), KM §3.3 "Release on the first `opaque_keys` entry" and KM §7.3 (256 holds; holds end at the physical release); the text-input `EscapeKey` precision agrees with UB §5.4 (`lose_focus_on_escape`) and states its override; "0 in layouts" for `UICore_Dialog_modal` is the UL §5.9 row; the §12.4 answer agrees with §12.1 step 1. The departure from the original's release consumption is labelled a Tessera rule and recorded as a known difference (§17 item 28). Non-blocking: (N1) §18.4 round-4 answer item 1 cites KM §7.3 for "game code ignores repeats for holds"; that rule is KM §7.2 ("Presses that are not resolved") and §7.8.6. (N2) item 3's "no release arrives without its press, in any receiver" holds only inside the 256 and 64 tracking bounds; the precisions bullet of the second answer already names the unpaired release when a bound forgets an entry, so the two read together. (N3) the §18.4 table row "release of `AcceptKey` or `EscapeKey` … consumed" and the revision-4 sentence on the `EscapeKey` release of a message carry no pointer to the pairing rule that replaces them for Tessera; a pointer would help readers who stop at the table. Earlier: **Review 2026-10-23 of the round-4 revision** (spec-review:spec-ui-r4:3 (independent session), which wrote no part of it): **passed**; status Revised → Accepted. Revision 4 with the round-1 and round-2 fixes: §2.4 keys, §11.3 deactivation OPEN and Tessera rules, §11.4, §12.1–§12.3 answers, new §12.5, §17 items 25–27, §18.2 client-wide order (round-2 fix reproduces steps 2–4), §18.4, §19.3, §19.4, W33, W34, BIN revision 4 (question (q) within decision 0011: build, SHA-256, tool, question and why M1–M3 were not enough stated; not re-traced). Non-blocking: (1) the parenthesis closing the §18.2 engineering-choices callout says item (c) is answered in §19.4; the answer is in §19.3; (2) the Status cell lists the round-1 fixes but not the round-2 fix of §18.2. CLEAN_ROOM.md §5.3 met on the changed text: functional facts and labelled Tessera rules only; no code or pseudo-code; no copied prose; no game text (strings cited by token only), keys, hostnames or traffic; no disassembly in prose; new facts carry provenance rows and marks; unknowns are OPEN callouts with Tessera rules; nothing outside decision 0034 §2; contamination screen clean. Relative links and anchors of the ten round-4 specs resolve. Spot checks (own read-only probes, 2026-10-23): `EnablePortraits=True` in `[Render]` of both preference files; `FilterMenu` based on `MapNoteFilterMenu` (based on `Menu_02` of 0x22000080, `auto_close_on_selection` false, `text_item` `ContextMenuBool` of 0x2200007B, `selection_display` 0); `MapMenu` based on `AvailableMapMenu`, based on `DropDownButton` of 0x22000004 with `boolean_button` true; `Menu_01` of 0x22000080 a Menu without `auto_close_on_selection`; the finder's `…_TierMenu` (popup `Menu_01_PopupField`, items `Menu_01_PopupItem`, own selection display, no `auto_close_on_selection`) and `…_SizeMenu` (based on `Menu_01`). Before: **Review of revision 3 after round 5, 2026-10-23** (spec-review:spec-ui-input:2 (independent session), which wrote no part of it): **passed**; status Revised → Accepted. CLEAN_ROOM.md §5.3 met on the changed text: functional facts and labelled Tessera rules only; no code or pseudo-code, no copied prose, no game text, keys or hostnames; no disassembly in prose (§18.2, §19.2–§19.4, W29 and §18.6 state observable results; the BIN revision-3 cell names questions (n)–(p) only); BIN (n)–(p) within decision 0011 (offline UI code, build, full SHA-256, tool, questions and why M1–M3 were not enough stated; scope confirmed by the orchestrator); decision-0012 constants listed in W30 and W31; every unknown is an OPEN with a Tessera rule (§17 items 19–24). Spot checks against the install: layouts 0x2200007C and 0x2200007B, scene 0x21000004 and texture lists 0x40000E4B, 0x40000E4C present; relative links and anchors resolve. Non-blocking: none. **Review of revision 3 after round 4, 2026-10-23** (spec-review:spec-ui-input:1 (independent session), which wrote no part of it): **changes requested**; status stays Revised. The round-4 fixes of §18.2, §19.3, §19.4 and W29 hold, and the layout IDs, boxes and mapper names of §18.1 and §19.1 agree with the install. Blocking: §19.2 still words an item of another kind as stopping "the build", which describes construction and leaves open whether the menu is shown in part or not at all; state the observable result. Non-blocking: cut the BIN revision-3 parenthetical list of code kinds that were read to "BIN (n)–(p)", as G42 now does. **Review of revision 3, 2026-10-23** (spec-review:spec-ui-input:2): **changes requested** (B1: the fixed 10 pixels of §19.3 missing from W31), answered by the review fixes; a later review asked that §18.2 and §19.3 state observable results only and that review-record text move to the review log; answered by round 4 (change log). Status stays Revised. **Review of revision 2, 2026-10-14** (commit 7c545cd on branch `specfix`): **passed, Accepted**; its spot checks of W29 agreed with the text. Earlier: Review of the decision-0014/0015 revision, 2026-10-06: **passed**, Accepted. Full records moved to the review log (decision 0018). |
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
- **Viewport** (§12): the 3D portrait element: properties, what a `TargetViewport` shows, its
  camera, its render target and opacity mask (§12.1–§12.4; parts of the drawing stay OPEN);
- **Engine dialog boxes** (§18, revision 3): the wait, confirmation, message and text-input
  dialogs, their layout, parts, keys, results, timeouts and queue;
- **Context menus** (§19, revision 3): the `ContextMenu` layout, item kinds, submenus, sizing,
  placement, hover delay, selection and closing.

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
| BIN | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`), disassembled with capstone in the dirty room on 2026-10-06, address ranges only. **Questions:** (a) which widget class the factory creates for each engine element type, and how the classes derive from each other; (b) for Meter, Scrollable, Scrollbar, ListBox, Tree, Panel and Menu: which properties each class reads, what each one does, which parts it creates or finds by element ID, and how it lays them out (the classes' property-change handlers, post-creation routines, resize routines, frame-update routines and layout routines); (c) the scroll-bar value mapping, stops, dragging, paging and wheel handling, and how a scrollable widget keeps its scroll bars in step (the classes' message handlers); (d) the Button's auto-repeat ("hot button"), its click on release and its translation of state requests; (e) the ListBox grid layout, entry creation from templates and selection; (f) the Panel's page and tab switching. Numeric constants are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). Diagnostic strings in the executable name the meter routines; they are not reproduced. **Why M1–M3 were not enough:** the layout data gives property values and labels but no behaviour; no published source describes the engine's widget classes (the plugin API documents only the plugin surface); the original client cannot be observed running in this session, and observation could not give the arithmetic, rounding or ordering rules anyway. Only offline UI code was read; nothing about network, login, keys, launcher, patcher or anti-tamper. No address, listing or symbol appears in this spec. **Revision 1 questions (same build, read 2026-10-06):** (g) what the GroupBox class does (its factory, constructor, the methods it replaces relative to the base element, its message handler, property-change handler and its step in the pass that runs after a tree is created) and which methods the ColorPicker class replaces; (h) the exact scope of the early stops in the Button's state-request translation (§7.3); (i) whether the tree placement adds the indent amount when the parent has no icon (§9.3), and which sizes the ListBox placement uses under `ignore_cell_alignment` (§8.3); (j) the order of the descendant lookup (§2.3); (k) what the pass that runs after a tree is created does for scrollables, Trees and Buttons (§5.2, §9.1); (l) how a scroll bar treats arrow buttons that are not its descendants (§6.2, re-read). **Revision 3 (2026-10-23):** method M4 under decision 0011, client `x64/lotroclient64.exe` 4906.0071.1794.4035, SHA-256 d22363eb9aff0fb599fd04b3efa9a1769d08da8af9d8f9b73121c12bfd7e21ee (hashed 2026-10-23), Ghidra 12.1.3 headless on a private read-only copy of the shared analysis project (deleted after use); functions found by instruction search for the property IDs and element IDs named below. **Questions:** (n) how the engine dialog framework chooses the element of each dialog type, which parts it fills from which description properties, which inputs close a dialog with which result, how timeouts, modality and several pending dialogs are handled (§18); (o) how a context menu is built from its items (templates, sizes, scale, placement), and how hovering, the open delay, clicks and deactivation act (§19); (p) which figure and camera a `TargetViewport` uses, what a target's viewport description changes, what the portrait preference gates, and which render-to-texture and opacity-mask properties affect the drawn portrait (§12.1–§12.4). **Why M1–M3 were not enough:** the layouts give the dialog, menu and portrait elements and their properties, and the dictionary names the description properties, but neither says which part receives which property, which input gives which result, how menus are sized and placed, or how a portrait finds its target and camera; no published source describes these engine parts; the original client cannot be observed running in this session, and observation could not show the queue, the timeout arithmetic or the result properties anyway. Only offline UI code was read: BIN (n)–(p); nothing about network, login, keys, launcher, patcher or anti-tamper. No address, listing or internal name appears in this spec. **Revision 4 (2026-10-23):** method M4 under decision 0011, the same build and SHA-256 (hashed again 2026-10-23), Ghidra 12.1.3 headless on a private read-only copy of the shared analysis project (deleted after use); an immediate-operand scan of the whole executable for the property ID of `UICore_Menu_auto_close_on_selection`, and the one routine it found. **Question:** (q) which UI classes read `UICore_Menu_auto_close_on_selection`, and what a Menu does with its popup on the messages it receives from it (§11.3, §19.4). **Why M1–M3 were not enough:** the layouts say which elements carry the property, not which kind of menu obeys it, and the original cannot be observed in this session. Only offline UI code was read: BIN (q). |
| UL, UB, PS, REC, FO, LPA, KM | The companion specs named in the header; facts taken from them keep their marks. |
| DEC | Accepted Tessera decisions [0014](../decisions/0014-ui-input-bridge.md) (UI input bridge, **IB**) and [0015](../decisions/0015-ui-rendering.md) (UI rendering, **UR**). They fix **Tessera's** behaviour, not facts about the original; statements taken from them are marked "Tessera rule" with the decision and its section. |

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
| W29 | Scroll-bar visibility: §6.4 step 6 (and the fill parts of step 5) set the same visibility that `UICore_Element_hide` sets; the bar has no separate shown state, so its next layout overrides an earlier hide (§6.4). The fellowship member row's set-up writes `FellowTemplateVolumeControl`'s visibility after setting its position (§6.4). Census: 4 of the 792 resolved Scrollbar instances are hidden by the layout (own block or default state), all with `hide_when_disabled` true and none disabled (§6.10) | BIN, revision 2 (2026-10-14): method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md), client `x64/lotroclient64.exe` 4906.0071.1794.4035, SHA-256 prefix d22363eb9aff0fb5 (verified 2026-10-14), Ghidra 12.1.3 headless on a private read-only copy of the shared project (deleted after use); **question (m):** does the bar's layout leave a bar hidden that the layout data or game code hid; read: BIN (m); **why M1–M3 were not enough:** the data say only which bars are hidden, not whether the class's layout overrides it, and the original client cannot be observed in this session. OBS (2026-10-14, read-only: the 302 layout records of the 2026-10-06 dump, the archives unchanged since 2026-10-04 and five records re-read from the install byte-identical; every top-level element instantiated, 83,161 instances as in UL §7.3) | BIN; census Confirmed 2026-10-14 |
| W30 | Engine dialog framework: the four engine dialog types and their elements in layout `Dialog`, the parts each type fills from which description property, the countdown and its pause rule (numeric constants recorded under decision 0012: the countdown's string variable ID 0x7B035 set to the remaining whole seconds; the 0.0002 s duration magnitude at or below which a confirmation gets no element; the 1.0 s gap between dialog updates beyond which the end time is moved later; the 2.0 s repeat of the range check), the result properties written for each input, keys while active, modal key swallowing, one dialog at a time with a queue, `topmost` and `cannotbeoverridden`, `StartInactive`, the range-check repeat (§18) | BIN (n); OBS (layout `Dialog` 0x2200007C resolved element by element, 2026-10-23; the dictionary's `UICore_Dialog_*` properties with their types and defaults; the `UICore_Dialog_type` mapper names); class library (record 0x56000000, read 2026-10-23 with the repository's `tessera-dattool bytecode`, M1): the quest log's abandon request and the description properties it sets (§18.6) | BIN; layout data Confirmed 2026-10-23 |
| W31 | Context menus: the three menu elements and four item templates of layout `ContextMenu`, item kinds, item sizing, common width, the 0.8 × screen-height list cap (numeric constants recorded under decision 0012: the 0.8 factor of the list's maximum height, §19.3 "Common width"; the fixed 10 pixels added to an item's text width, §19.3 "Item size"), the scale-down to fit, placement of root menus and submenus, check marks, hover states, the open delay, the messages sent on selection, on close and on deactivation (§19) | BIN (o); OBS (layout `ContextMenu` 0x2200007B resolved, 2026-10-23; the UI-message and state mappers) | BIN; layout data Confirmed 2026-10-23 |
| W32 | `TargetViewport`: the figure's entity, the portrait preference gate, the per-target `UIViewport_Array` description matched by `UIViewport_Type`, its image, scene-entity, lighting-scene and starting-idle members, the camera chosen from `UI_TargetViewport_CameraPropertyName` else `Camera_1`; which viewport properties (render-to-texture size, background, opacity mask, lighting values) affect the drawn portrait and take effect when changed, the mask applied to the rendered scene (§12.1–§12.4) | BIN (p); OBS (the two portrait elements, scene 0x21000004 element by element, and a census of `UIViewport_Array`, `UIViewport_Type`, `UIViewport_Image` and `Character_PortraitCameraName` over every 0x47, 0x78 and 0x79 record, 2026-10-23, 0 decode failures) | BIN; data Confirmed 2026-10-23 |
| W33 | Only the Menu class (§11) reads `UICore_Menu_auto_close_on_selection`; the context menus of §19 do not, so a context-menu tree always closes on a selection (§19.4 step 3). A Menu closes its open popup when the popup is deactivated (§11.3). A list-box selection of the entry already selected sends no `Menu_new_selection` but still closes the popup when `auto_close_on_selection` holds (§11.4). The map window's `FilterMenu` and the finder's size and tier menus are Menu elements with popups of layout 0x22000080 (§11.4) | BIN (q): one reader of the property ID in the executable (an immediate-operand scan; a property ID passed in another form would not be found); OBS (the `map` and `fellowshipmaker` layouts resolved, 2026-10-23; the keys of `Menu_new_selection` 0x1C and `Element_deactivated` 0x2F in §2.4 read from the install's `UIElementMessage` base mapper 0x230001AC, 2026-10-23) | BIN; data Confirmed 2026-10-23 |
| W34 | Both `UserPreferences.ini` files on the machine (the Steam prefix and the third-party launcher's prefix, `install-layout.md` §8) hold `EnablePortraits` in `[Render]`, value `True` in both; both portrait opacity masks (0x40000E4B, 0x40000E4C) are texture lists of five DXT3 levels from 64 × 64 down to 4 × 4, the 64 × 64 level stored in the high-resolution archive (§12.1, §12.3) | OBS (2026-10-23, read-only: the two files' `[Render]` sections; the two lists and their ten surface headers) | Confirmed 2026-10-23 (this machine); that `EnablePortraits` is the preference of §12.1 step 2 is Probable (name only) |

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

> **OPEN (implementer, 2026-10-26, follow-up `ui-host-settle`):** the client reads the install's
> dictionary at start-up and follows the order above: a PID whose definition stores a default
> takes the install's value; a PID the install defines **without** a stored default has none
> (step 3), even where the table above lists one; the table's value is used only for a PID
> the dictionary does not define. On this install every table PID above is defined with a
> stored default, so nothing changes there. One conflict remains: the install's definition of
> `UICore_Element_alphablendmod` (0x06E5) stores no default, so step 3 would give 0, while
> `ui-layout.md` §9.6 gives the property "default 1" and `lua-plugin-api.md` §13.17 says a
> plugin background's factor "stays 1.0". Engineering choice until answered: a default that
> another spec states as a rule of the property (`alphablendmod` 1, `ui-layout.md` §9.6;
> `UICore_Element_AllowStretchOnDrag` true, `ui-behaviour.md` §7.2) is kept whatever the
> dictionary stores; step 3 applies to every other PID. Question: is the "default 1" of §9.6
> the reading the client applies when the property is absent (so step 3 does not apply to it),
> or should `ui-layout.md` §9.6 and LPA §13.17 follow step 3?

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
| 0x0A | `Element_mouse_over_top` | | 0x2F | `Element_deactivated` |
| 0x0C | `Menu_closed` | | 0x32 | `Element_mouse_press` |
| 0x0E | `Element_mouse_release` | | 0x33 | `Scrollbar_line_increment` |
| 0x12 | `Meter_new_position` | | 0x34 | `Scrollbar_new_position` |
| 0x15 | `Menu_opened` | | 0x35 | `Button_click` |
| 0x18 | `Element_mouse_tap` | | 0x36 | `Meter_goal_position_reached` |
| 0x1C | `Menu_new_selection` | | 0x37 | `Panel_NewPage` |
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

Whether this flag is the "flagged by code" of UL §8.3 (screen-level reference boxes) and
UL §8.4 step 1 is not known; see the UL §8.4 scope OPEN. Tessera rule there: it is not, so an
alpha-test meter owns a surface but is laid out like any other non-root element. No M5 panel
holds an alpha-test meter.

> **OPEN:** the exact effect of the threshold (which pixels of the meter's image the surface
> drops for a given value, and whether the comparison is "less than" or "less or equal"). 4
> elements use this mode; none is in the §17 panels of UL. Tessera rule: draw the meter's image
> and drop the pixels whose alpha ÷ 255 is below 1 − *p*.

Tessera rule for where the threshold acts (DEC: UR Decision 1.1, 1.4 and 1.6): the meter owns
its own surface, into which its image is painted as for any element; the threshold 1 − *p* is
a value of that surface's composition entry and is applied when the surface is composed. A
position change therefore changes only the composition entry and repaints nothing. The alpha
compared is the surface pixel's own alpha, as in the rule above, before the entry's opacity is
applied. This order **departs** from decision 0015 §8 as written: its "UI pipeline" row multiplies
the texel by the entry's opacity first and applies the threshold to the result. This spec
follows instead the decision's open review item "Alpha-threshold order" (decision 0015, Open
review items, Other improvements), which asks for the threshold on the unfaded texel alpha:
the meter's fill must not change when only its opacity changes (for example during a fade).
The difference concerns no M5 panel (none of the 4 alpha-test meters is in the UL §17
panels), and the decision's §8 row should be aligned when the record is next revised.

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

> **OPEN:** (clean-room implementer, 2026-10-07) whether the push of this section and of §6.9
> runs the ancestor walk of UB §5.2 step 2 (the hovered element's ancestors pushing their own
> maps one priority lower each) or adds the single entry. No layout element sets
> `UICore_Element_InputMapID`, so the two differ only for maps given by code. `tessera-ui`
> adds the single entry (map, hovered element, 0) and pops only that pair, so a hover never
> leaves or removes entries of its ancestors.

**Tessera input mapping** (DEC: IB D5, D7). The pushes of this section and of §6.9 reach
Tessera's input mapper as entries of the Tessera context `ui.scroll_mouse` (owner = the
element the map was pushed for, priority 0), which binds `WheelUp`/`WheelDown` to the actions
carrying `ScrollUp`/`ScrollDown`. While such an entry exists, a wheel event's motion does not
reach the camera, so the wheel scrolls and does not zoom; over an element that pushes nothing
(for example a plain native panel) the wheel zooms the camera. Two consequences that follow
from the rules here and in UB §4.4, stated so that no implementation special-cases them:
- **Captures.** During a capture the hovered element is the captured element while the pointer
  is over it, and nothing otherwise (UB §4.4 step 2). When the pointer leaves a captured
  element, the hover value 0 pops the entry (§6.9 for a bar; the Tessera pop rule above for a
  scrollable), so during a thumb drag that leaves the thumb the wheel zooms the camera until
  the pointer is back over it.
- **Pointer lock.** While the pointer is locked for mouse-look nothing is hovered
  (`ui-behaviour.md` §4.9), so the entry is popped and the wheel zooms.

> **OPEN:** whether plugin controls push `ScrollableControls_Mouse`, and which classes (the
> engine class behind each plugin control is not specified, `lua-plugin-api.md` §13.4).
> **Tessera rule** (DEC: IB D5, "Who pushes"): every hovered plugin control pushes it for
> itself at priority 0, so a plugin's wheel handler always fires; the cost is that the wheel
> does not zoom the camera over a plugin window. Owner observation (IB owner summary,
> question 4): does the wheel zoom the camera over a plain plugin window in the original?

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

**Visibility is one flag (BIN, W29).** Step 6, and the showing of fill parts in step 5, set
the element's one visibility (UB §11.2), the same visibility a change of
`UICore_Element_hide` sets (true hides, false shows; UB §11.2 last paragraph). The bar keeps
no shown flag of its own. Consequences:

- A bar hidden by the layout data, by a state or by game code is shown again by its next
  layout (any trigger at the top of this section: a "lays out" property, a position change,
  a resize) unless at that moment it is disabled **and** `hide_when_disabled` is true.
- A bar hidden by step 6 is shown again by a later `UICore_Element_hide` false (or a state
  that sets it), until its next layout hides it again.
- The original's game code that keeps a bar hidden writes the visibility **after** the bar's
  last layout: the fellowship member row (`social-panels.md` §4.2) sets the volume control's
  position first and then shows or hides it from its voice flag (BIN, W29).

In the data, 4 of the 792 resolved Scrollbar instances are hidden by the layout (§6.10), all
with `hide_when_disabled` true and none disabled; two of them are `FellowTemplateVolumeControl`.

> **Answered (2026-10-14; was an implementer OPEN of 2026-10-11, from `social-panels.md`
> TR14):** step 6 does **not** leave such a bar hidden: there is one visibility, and step 6
> writes it (above). Tessera's step 6 as built (show unless disabled and `hide_when_disabled`)
> is right, and so is keeping a bar hidden by disabling it with `hide_when_disabled` true
> (`social-panels.md` TR14, §9.1 item (2)). No implemented behaviour changes.

> **OPEN:** whether, in the original, a layout of a bar runs during creation **after** its
> post-creation reports `UICore_Element_hide` (UL §9.2 step 8: the order in which the
> effective properties are reported, and whether attaching the bar to its parent lays it out
> again), that is, whether a bar hidden only by the layout data is visible once its tree is
> created. Not traced. **Tessera rule:** no special case; steps 1–7 apply in Tessera's own
> report order, so such a bar may be visible after creation. Code that must keep a bar hidden
> sets it disabled with `hide_when_disabled` true, or hides it after its last layout.

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
  as over its list. (Tessera's mapping of this push, and what it means during a capture: §5.7.)
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
| `UICore_Element_hide` true (own block or default state; W29, 2026-10-14) | 4: `FellowTemplateVolumeControl` (2), `Webstore_URL_Scrollbar` (2); each with `hide_when_disabled` true, none disabled |
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

> **OPEN:** what happens to a node's icon part and child nodes when its entry is removed from
> the tree, destroyed or moved to another parent (not traced). Tessera rule (implementation,
> M5): the removed node's icon part is destroyed with it; its child nodes move up to the
> nearest ancestor node that remains (top-level when none does) and are shown exactly when
> that new parent is expanded and visible (a top-level node is shown); adding an entry that
> already has a node adds no second node or icon.

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

- **Deactivated** (revision 4, BIN (q), W33): when the open popup reports `Element_deactivated`
  (another element became the active one, for example after a click elsewhere), the menu closes
  the popup as above.

> **OPEN:** whether the original keeps the popup open on deactivation in some situation, and if
> so in which. Not observed; the closing rule above is the only behaviour stated.
>
> **Tessera rule** (revision 4, review round 1): a deactivated popup always closes; there is no
> case in which it stays open (§17 item 27).

**Tessera rule** (revision 4): a click on the menu while its popup is open closes the popup
(the first bullet), even though the press of that click has already deactivated the popup: a
popup closed by deactivation within the same input event as the click is not opened again by
that click.

<a id="s11-4"></a>
### 11.4 Selection display

After a selection (or at set-up), the menu finds `selection_display` as a descendant: it must be
Text. If the list box has a selected entry, the display takes the text of the entry's Text part;
otherwise it shows `default_no_selection_text` if set, and is cleared otherwise. When
`selection_display` is 0 (absent, or set to the ID 0) or names no Text descendant, no display is
updated (BIN, re-read with W28). With `auto_close_on_selection` the popup closes. A new
selection in the list sends `Menu_new_selection` with the selected entry; a click on the entry
that is already selected sends nothing, but still closes the popup when
`auto_close_on_selection` holds (revision 4, BIN (q), W33).

> **OPEN:** who creates the popup's text entries from `text_item_layout` and `text_item` (game
> code is assumed: no engine caller was found), and how the popup is sized to its list. Not
> needed for the closed menus of the §17 panels.
>
> **Answered in part (spec writer, 2026-10-23, revision 4; W33):** game code fills the popup's
> list. Two such Menus are specified elsewhere: the map window's `FilterMenu` (an engine Menu
> based on `MapNoteFilterMenu` of the `map` layout: popup `Menu_02_PopupField` of layout
> 0x22000080, a 150 × 200 field with `UICore_Element_max_height` 200 holding the one-column
> list `Menu_02_PopupListBox` at (5, 5), 130 × 190, with a vertical scroll bar; `text_item` =
> `ContextMenuBool` of layout `ContextMenu`; `auto_close_on_selection` false;
> `selection_display` 0; filled by the map window, `map-windows.md` §6.9) and the finder's size
> and tier menus (`FellowshipMakerLaunchNow_SizeMenu`, based on `Menu_01` of layout 0x22000080,
> and `FellowshipMakerLaunchNow_TierMenu`, of the game type `FellowshipMakerTierMenu` with the
> Menu properties: popup `Menu_01_PopupField`, entries `Menu_01_PopupItem`, each with a
> selection display; filled by the finder, `travel-windows.md` §6.4). How the popup is sized to
> its list stays OPEN. **Tessera rule:** the popup keeps its layout width (at least the menu's
> width, §11.2 step 3); its height is the list's content height plus the popup's extra height
> over its list (§11.2 step 4), at most the popup's `UICore_Element_max_height` when set; a
> longer list scrolls with the popup's scroll bar. Entries are laid out at the list's width.

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
>
> **Answered in part (spec writer, 2026-10-23, revision 3; W32):** what a `TargetViewport`
> shows, which camera it uses and what a target's own viewport description changes are §12.1;
> the portrait scene is §12.2; the render target, background and opacity mask are §12.3; what
> Tessera draws while no figure is drawn is §12.4. Scene loading and camera selection by name
> are `misc-records.md` §4.2 (U4). Still OPEN (§12.3): the arithmetic of the opacity-mask pass,
> the meaning of `UICore_Viewport_RenderToTextureIndex`, and how the bloom, deshade,
> rim-lighting and fake-shadow values act (the `character-screens.md` §2.1.1 OPEN).

Tessera rule for composing a portrait (DEC: UR Decision 1.7, "External content"; it answers
only Tessera's side of the OPEN above): the figure is rendered into an offscreen target of the
size of §12.3 step 1, in a linear 8-bit RGBA format (never sRGB), holding premultiplied
colour; what each drawn frame leaves in it is §12.3 (the background under the scene, then the
opacity mask; revision 4: the renderer may clear the target to transparent black and put the
background under the drawn scene afterwards, `render.md` §6.10 "Alpha"). The target is
composed as a separate entry in the panel's drawing order, onto the element's displayed box,
so the panel art shows where the mask leaves the target transparent. The
`UICore_Viewport_RenderToTexture…` size, background and opacity-mask properties are used as
§12.3 says (revision 3); the index is recorded only. A Browser shows no web content in M5
(§3.5).

> **OPEN:** a viewport element that also has a non-zero object mode, and so owns a surface of
> its own (UL §8.4 OPEN (scope), `lua-plugin-api.md` §13.19): how the original orders the 3D
> scene against the element's own media and its children. Both M5 portraits are such elements
> (UL §17.2, §17.4, U47): `PlayerPortrait` (P4) has mode 3, no media and no children;
> `OpponentPortrait` (P5) has mode 1, one Image item (draw mode 2) and a child `Target_Image`
> in mode 1. Decision 0015 composes external content as a separate entry but does not say
> where it sits when its element owns a surface. Not traced (viewport rendering is outside the
> traced widget code).

Tessera rule for a surface-owning viewport (a Tessera choice that applies UR Decision 1.7; the
decision does not state this case): the element's own surface holds what the element paints
on the CPU (its media, by the usual rules, and the content of its descendants that own no
surface). The external content is a **direct nested entry of that surface**, placed in the
surface's drawing order at the point where the element's own content ends and its children
begin, with the element's box as its coverage region. So the composition order is: the
element's canvas below (its own image, for `OpponentPortrait` drawn with Replace), then the
3D scene, then the element's children in their order (for `OpponentPortrait`, `Target_Image` as
its own nested entry), with anything the element's surface itself draws after that point in
its canvas above. For `PlayerPortrait` the canvas below is empty, so only the scene shows. A
viewport element without a surface of its own keeps the rule above (its scene is a nested
entry of the nearest surface above it, at the same place in that surface's drawing order).

<a id="s12-1"></a>
### 12.1 What a `TargetViewport` shows (BIN, W32; data OBS)

The two M5 portraits (`PlayerPortrait` of P4, `OpponentPortrait` of P5) are `TargetViewport`
elements whose data is (OBS, both portraits unless stated): `UICore_Viewport_uiscene` =
0x21000004; `UICore_Viewport_camera` = the UISceneEntity key labelled `Camera_1`
(0x1000001E here); `UICore_Viewport_RenderToTextureWidth` and `…Height` 62;
`…RenderToTextureBackgroundColor` opaque black; `…RenderToTextureIndex` 0 (P4) and 1 (P5);
`…RenderToTextureOpacityMask` a texture list (prefix 0x40, `texture-list.md`), 0x40000E4B (P4)
and 0x40000E4C (P5); `UICore_Viewport_RenderInParent` true; `UICore_Element_KeepObjectWhenHidden`
true; `UI_TargetViewport_CameraPropertyName` = the property `Character_PortraitCameraName`;
`UI_TargetViewport_DisableObjectCreationFromPreference` true; `UIViewport_Type` `Self` (P4) and
`Opponent` (P5) (mapper names: `Paperdoll`, `Opponent`, `Self`, `AppearanceMod`,
`MountedPaperdoll`, `EmotesPaperdoll`); P4 alone has `UI_TargetViewport_UsePlayerAsTarget` true.

1. **Which entity.** With `UI_TargetViewport_UsePlayerAsTarget` true the viewport shows the
   player's own character; otherwise the entity its panel shows (for P5 the current selection,
   `combat-ui.md` §5). When that entity changes, the old figure is removed and a new one is
   built; with no entity, or one the client does not know, no figure is shown.
2. **The portrait option.** When `UI_TargetViewport_DisableObjectCreationFromPreference` is
   true, no figure is built while the client's portrait option (a user graphics preference) is
   off; changing the option rebuilds or removes the figure.
3. **The target's viewport description.** The entity's properties may hold `UIViewport_Array`,
   a list of `UIViewport_Struct` values. The viewport uses the first struct whose
   `UIViewport_Type` equals its own `UIViewport_Type`, and none when its own type is 0. From
   that struct:
   - `UIViewport_Image` (a DID), when set: the viewport is given that image in place of a 3-D
     figure (how the image is drawn was not traced; OPEN, §12.3);
   - `UIViewport_EntityArray`: each `UIViewport_EntityStruct` names a scene element by the text
     of its `Name` (the element names of `misc-records.md` §4.2, descriptor bit 0x100); when the
     scene has an element of that name, the struct's `UIViewport_EntityProperties` are applied to
     it and `UIViewport_EntityPosition` replaces its placement. In data these entries are cameras
     with `CameraFOV` and `CameraFarDistance` and a position: per-target framing;
   - `UIViewport_LightingScene` (a DID): a record whose own `UIViewport_EntityArray` is applied
     the same way;
   - `UIViewport_StartingIdle` (an animation key): the figure starts with that animation;
   - `UIViewport_UISceneOverride` (a DID; 14 values in data) is not read by this step (OPEN).
4. **Camera.** On every update the viewport sets its own `UICore_Viewport_camera`: to the value
   of the entity's property named by `UI_TargetViewport_CameraPropertyName` when the entity has
   that property as an enumeration value, otherwise to the key labelled `Camera_1`. The camera
   rule of `misc-records.md` §4.2 U4 then picks the scene element.

Census (OBS, every 0x47, 0x78 and 0x79 record, 0 decode failures): 1,535 records hold
`UIViewport_Array`; the `UIViewport_Type` values inside them occur 1,014 times as `Opponent`,
592 `Self`, 54 `MountedPaperdoll`, 49 `Paperdoll`, 37 `AppearanceMod`, 23 `EmotesPaperdoll` and
25 times as 0; 41 records hold a `UIViewport_Image`. No record stores
`Character_PortraitCameraName`, so with install data alone both portraits use `Camera_1`.

> **OPEN (implementer, 2026-10-23):** (`apps/client`, portraits) Tessera's settings have no
> portrait graphics option, so step 2 treats the option as always on. Should a settings key
> (and an Options entry) be specified for it?
>
> **Answered (spec writer, 2026-10-23, revision 4; W34):** no Tessera key. The user's
> preferences file holds the original's option as `EnablePortraits` in its `[Render]` section
> (both files on this machine: `True`; that this key is the preference step 2 tests is
> Probable, by its name). **Tessera rule:** the portrait option is read from that key where the
> client already reads `[Render]` of `UserPreferences.ini` (windowed runs, off the frame thread,
> the file found as `far-view.md` FD21 says): the value `False` (compared without regard to
> case) turns it off; any other value, an absent key or no readable file leaves it on.
> Headless runs and tests keep it on. Tessera writes nothing to that file. An Options entry is
> not specified here; when the Options panel offers one, it changes the option for the run.
> **Implemented behaviour changes** only for a user whose file says `False`.

> **OPEN (implementer, 2026-10-23):** (`apps/client`, portraits) step 3 (the target's own
> `UIViewport_Array` description: per-target cameras, an image instead of a figure, the
> starting idle, the lighting scene) is not applied. Step 4 is only partly applied: the camera
> key is read and a change redraws the portrait, but nothing uses it for framing.
>
> **Answered (spec writer, 2026-10-23, revision 4):** the rules of steps 3 and 4 stand; these
> are gaps of the build, recorded here so they are not lost, and the §12.2 stand-in covers them
> meanwhile. Order of work (Tessera rule): step 4 with the scene's camera (§12.2) first, then
> the `UIViewport_EntityArray` camera entries of step 3 (they carry the per-target framing of
> the 1,606 `Opponent` and `Self` descriptions of the census), then the starting idle (§12.5),
> then images.

> **OPEN (implementer, 2026-10-26):** (`apps/client/src/portrait/scene.rs`; steps 2–4
> built) facts the build needed and the spec does not state; the choices below are Tessera
> readings until answered:
> (a) **Which records are "the entity's properties"** for steps 3 and 4. As implemented: the
> placed figure's instance property set, then the property set of its game object (the
> +0x09000000 rule), then the own property set of its visual entity (the 0x47 its `PhysObj`
> names, or the 0x47 body itself); the first set that holds the property wins. A player's
> body is the race table's `RaceTable_Gender_PlayerAvatar` object (`appearance.md` §2.1).
> Template chains and the 0x47 `extra_sets` are not searched. With this reading every
> entity the real-data test shows (the player as P4 and P5, another entity as P5) has a
> description.
> (b) **The `Name` of a `UIViewport_EntityStruct`** has the dictionary type StringInfo on
> this install. As implemented: a StringInfo with literal text (or a plain string) gives
> the name; one that refers to a string table gives none (an element name is not localised
> text). The entry applies to the **first** element, in stored order, whose descriptor name
> (bit 0x100) equals it exactly (case-sensitive), as U4 takes the first match.
> (c) **`UIViewport_EntityPosition` and `UIViewport_EntityProperties`.** As implemented: the
> position is a position value (its offset and its orientation, each only when present) or
> a vector (position only), in the scene's frame, replacing the element's placement part by
> part; a placement outside the descriptor limits is not applied. The properties are the
> entries of the value (struct fields or array elements), each replacing the element's
> property of the same PID or joining its set; at most 64 entries per array and 64
> properties per entry (engineering bounds). An entry may name any element, the figure's
> marker element of the Tessera rule below too: the figure then stands where the entry puts
> it (the marker is looked for after the entries are applied).
> (d) **`UIViewport_LightingScene`**: which record types it names. As implemented: a
> property-set record (0x78–0x7F), a game object (its set) or a 0x47 entity (its own set);
> its `UIViewport_EntityArray` entries are applied after the description's own.
> (e) **An image description** (`UIViewport_Image`): with no rule for drawing the image
> (§12.3 OPEN), the portrait shows no figure and is hidden as §12.4 says.
> (f) Step 4 for every `TargetViewport`: when the entity lacks the camera property (or the
> element has no `UI_TargetViewport_CameraPropertyName`) the key labelled `Camera_1` is
> used, whatever key the element stores (a stored key of 0 included); the element's own
> key only when that label does not resolve.
> (g) **What counts as "the target's own camera entry"** of the §12.2 answer (scene camera
> as it stands, else the stand-in). As implemented: an entry that names the camera element
> U4 picks and writes at least one property or replaces its placement. An entry with a name
> alone, or whose only change is a position outside the descriptor limits (not applied),
> does not count, and the stand-in stays. An entry with properties alone (for example
> `CameraFOV`, no position) counts, so the scene camera is used at its own placement with
> that field of view. Is that the intended reading?

> **OPEN:** where the figure stands in the portrait scene (no marker element was traced for
> `TargetViewport`). **Tessera rule:** the rule of `character-screens.md` §2.1.2 (the first
> Physics element whose setup has the holding location `Holding_Loc_Entity_00`), else the
> origin with identity rotation.

<a id="s12-2"></a>
### 12.2 The portrait scene (OBS)

Scene 0x21000004 holds five elements (`misc-records.md` §4): one Physics element without a
placement (it stands at the origin), one Camera
element named `Camera_1` with `CameraFOV` 60 and `CameraFarDistance` 20, and three Light
elements with `LightDiffuse` colours and constant `LightDiffuseIntensity` values (0.7, 0.7,
0.8). Its fog trailer has the override off. The scene is drawn by the presentation rules of
`character-screens.md` §2.1.1 (Tessera rule there), with the viewport's own ambient and fog
values; values are read from the install at run time.

> **OPEN (implementer, 2026-10-23):** (`apps/client`, portraits) the portrait scene is not
> drawn yet: no `Camera_1` framing, no scene lights, no viewport ambient or fog. The figure is
> drawn by the walker's portrait path with its fixed light, and Tessera's own head-and-shoulders
> camera is applied (field of view 40°; it shows the top 30 % of the joints' height plus 13 %
> head room). Please confirm this as the stand-in until the scene is read through
> `misc-records.md` §4.2 U4.
>
> **Answered (spec writer, 2026-10-23, revision 4):** confirmed as the **Tessera stand-in**
> (an engineering choice, not a client fact), for as long as §12.1 steps 3 and 4 and this
> section are not built. It ends when the scene is drawn: then the camera is the scene element
> chosen by U4 (for `Camera_1` here: its own placement, about 5.8 m from the scene origin and
> 1.9 m above it, `CameraFOV` 60 and `CameraFarDistance` 20, as observed; read at run time),
> replaced by the target's own camera entry when §12.1 step 3 gives one, and the lights are the
> scene's three Light elements. With the scene's own camera several metres away, a figure
> would appear small in a 62-pixel target (an inference from the camera's distance and field
> of view, not observed); the per-target camera entries of §12.1 step 3 are taken to give the
> close framing. **Tessera rule:** for a target whose description gives no camera entry, the
> stand-in camera may stay in use after the scene is drawn.

> **OPEN (implementer, 2026-10-26):** (`apps/client/src/portrait/scene.rs`) the scene's
> light curves: the portrait is a still picture (§12.5 stand-in), so a Light element whose
> `LightDiffuseIntensity` is not constant is evaluated at time 0, and a light driven by the
> day cycle uses its day variant (the creation viewport's rule, `character-screens.md`
> §2.1.1). The ambient input is `UICore_Viewport_AmbientLight` as stored values (the
> portraits leave it at the dictionary default, opaque black), the hemispheric input zero;
> the viewport's own fog (`UseFog`, near, far, colour) applies unless the scene's
> `fog_override` replaces it (U3). With black ambient the figure is lit by the three scene
> lights alone; please confirm that this is the intended look (no other ambient term in a
> portrait).

<a id="s12-3"></a>
### 12.3 Render target, background and opacity mask (BIN, W32; Tessera rules)

The drawn portrait depends on these viewport properties: the render-to-texture width and
height, background colour and index, the opacity mask, and the ambient, fake-shadow colour,
bloom, deshade, rim-lighting, fog, sky-glow, shadow and particle values; a change to any of
them takes effect on the next drawn frame (BIN). The opacity mask is a texture named by its DID
and is applied to the rendered scene, after the scene is drawn (BIN). Tessera:

1. The scene is rendered into a target of `RenderToTextureWidth` × `…Height` pixels (62 × 62
   for both portraits; the layout box is also 62 × 62), cleared to
   `RenderToTextureBackgroundColor` (opaque black for both).
2. **Tessera rule (OPEN on the arithmetic):** with an opacity mask, every pixel of the target
   (colour premultiplied) is multiplied by the mask texture's alpha at the same relative
   position (the mask stretched over the target); a mask without an alpha channel uses its red
   channel. So the black background shows only where the mask is opaque.
3. The masked target is composed by the rules of §12 above (external content, nested entry of
   the element's surface); `RenderInParent` true is that rule. The target is sampled with
   linear filtering when its size differs from the displayed box.
4. `RenderToTextureIndex` is recorded and not used: every Tessera viewport has its own target
   (OPEN: the original's use of the index; in data the two portraits use different values).

> **OPEN (implementer, 2026-10-23):** (`tessera-render`, portraits) `render.md` §6.10 "Alpha"
> says a portrait target is cleared to (0, 0, 0, 0) in each frame it is drawn; step 1 says it is
> cleared to `RenderToTextureBackgroundColor`. Tessera clears to transparent, draws the scene,
> then puts the background under it (C + (1 − A) · bg). For opaque, see-through and additive
> (destination factor one) draws this gives the same result as clearing to the background
> first. Please reconcile the two rows.
>
> **Answered (spec writer, 2026-10-23, revision 4):** step 1 states the result, `render.md`
> §6.10 "Alpha" the means, and both now say so. The **result** a drawn frame must leave is that
> of a target cleared to the background colour and then drawn: for every pixel, the scene's
> premultiplied colour *C* and coverage *A* over the background *bg*, *C* + (1 − *A*) · *bg*,
> before the mask of step 2. Clearing to transparent and putting the background under the
> scene afterwards is an allowed way to get it (Tessera rule); it is exact for opaque,
> source-over and additive draws. A draw whose result depends on the destination colour in
> another way (a multiply, min or max blend) would differ (whether any figure part draws with
> one was not checked); a target that gets such a draw is cleared to the background first.

> **OPEN (implementer, 2026-10-23):** (`apps/client`, portraits) which surface formats count
> as "a mask without an alpha channel" (step 2)? Tessera's choice: R8G8B8, JPEG and DXT1 take
> the mask from red; A8R8G8B8, A8, DXT3 and DXT5 use alpha. From a texture list, Tessera uses
> the first level that decodes and is at most 1024 per side.
>
> **Answered (spec writer, 2026-10-23, revision 4; W34):** confirmed as the **Tessera rule**
> (the original's choice was not traced): the formats of `records.md` §5.2 whose texels carry an
> alpha value (A8R8G8B8, A8, DXT3, DXT5) give the mask from alpha; R8G8B8, embedded JPEG and
> DXT1 (whose one-bit alpha is ignored) give it from red; the level is the first of the list
> that decodes and is at most 1024 per side. On this install both masks are DXT3 lists
> (64 × 64 down to 4 × 4, the largest level in the high-resolution archive), so both use alpha;
> without the high-resolution archive the 32 × 32 level is the first that decodes.

> **OPEN (implementer, 2026-10-23; engineering choices):** when `RenderToTextureWidth` or
> `…Height` is set neither on the element nor in the dictionary, the layout-box side is used;
> sizes are clamped to 1–256; viewport targets are capped at 2048 per side; at most 16 masks are
> kept; the content IDs of the two portraits are Tessera's own values for P4 and P5.
>
> **Answered (spec writer, 2026-10-23, revision 4):** confirmed as **Tessera rules**. The
> clamp is the dictionary's range of the two properties (1–256, §12 table); with an install
> the dictionary gives the default 256, so the layout-box fallback applies only without one.
> The cap, the mask count and the content IDs are engineering limits with no client fact
> behind them.

<a id="s12-4"></a>
### 12.4 While no figure is drawn (Tessera rule)

This answers the `combat-ui.md` §5.1 implementer OPEN of 2026-10-14 for Tessera's side: while
a `TargetViewport` draws no figure (no entity, the portrait option off, a figure still loading,
or no 3-D scene drawing in the client yet), the element and its children (for P5
`Target_Image`) are hidden, so the frame art shows in their box. That is the implemented
reading, now a Tessera rule. The original's behaviour in that case (the masked black
background, and the hidden `…_COVER_LOWRES` elements with their state lists) was not traced.

> **OPEN (implementer, 2026-10-23), round 4:** (`apps/client`, portraits) the walker draws a
> portrait for any entity that has a built character figure. An entity the walker draws only
> through its object layer (no character figure) shows no portrait, which is treated as the
> "nothing to draw" case of this section. §12.1 step 3 per-target framing is still not built
> (the head-and-shoulders stand-in camera of §12.2).
>
> **Answered (spec writer, 2026-10-23, revision 5):** confirmed as a **Tessera rule** for now.
> In the original any entity the client knows gets a figure (§12.1 step 1); Tessera's
> object-layer-only entity counts as "a figure still loading" of this section until the
> portrait can build its figure from the entity's own visual (the setup the object layer
> draws), which is a gap of the build, not a rule. The step-3 framing gap and its order of
> work stay as §12.1 says.

<a id="s12-5"></a>
### 12.5 Animation and redraw (revision 4; Probable; Tessera rules)

> **OPEN (implementer, 2026-10-23):** (`apps/client`, portraits; also `combat-ui.md` §5.1)
> Tessera's portrait is a still picture, drawn again only when the entity, figure, camera,
> size, background or mask changes (frame-cost rule), so the idle animation does not play in
> it. Does the original animate the portrait figure every frame?
>
> **Answered (spec writer, 2026-10-23, revision 4):** the original's portrait figure is a live,
> animated figure (Probable): a target's description can name the animation it starts with
> (`UIViewport_StartingIdle`, §12.1 step 3, BIN), which only matters for a figure that plays
> animations; how often the original renders the target was not traced (OPEN, §17 item 25).
> The rules below replace the still picture once they are built (**implemented behaviour
> changes** then); until then the still picture is the Tessera stand-in.

**Tessera rules.**

1. The portrait figure plays an idle: the target's `UIViewport_StartingIdle` when its
   description names one (§12.1 step 3), else the figure's own standing idle (the clip the
   walker's figure path resolves for standing still). It plays no other animation of the
   entity in the world (no walking, combat or emote clips), so the portrait does not follow the
   entity's motion.
2. While the portrait is shown, its target is drawn again whenever the figure's pose changes,
   at most once per client frame and at least 15 times a second when the client's frame rate
   allows it; a client may choose any rate in between to save frame time. While the portrait
   is hidden (§12.4, or its panel hidden), the target is not drawn and the clip does not need to
   advance.
3. A change of entity, figure, camera, size, background or mask draws the target in the next
   frame, whatever the rate of rule 2.

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
| `ui-layout.md` §6.6 OPEN (which non-root elements own a surface) | — | One more case: alpha-test meters ask for their own surface at post-creation (§4.5). Whether that flag also makes the meter a "flagged" element for UL §8.3 and §8.4 step 1 is not known; UL §8.4 scope OPEN records it with the Tessera choice that it does not | BIN |
| `ui-behaviour.md` §4.7 OPEN | Whether an unhandled scroll message reaches a scrollable ancestor; how far one wheel step scrolls | A scrollable acts on wheel messages without checking the sender (so descendants' wheel messages reach it, Probable) and turns one wheel step into one line step: for lists, content size ÷ rows, at most one view (§5.6, §5.7, §6.8) | BIN |
| `ui-behaviour.md` §4.3 OPEN | Per-type mouse-visible rules | Not answered here. Related facts: a Panel turns on the mouse-button flag of its tabs (§10.2); a slider-mode scroll bar turns off its thumb's flag (§6.7); meter parts are `mouse_invisible` (§4.3) | BIN |
| `ui-behaviour.md` §5.2, `keymap.md` §12 (and the input-bridge gap of the M5 critique, item A1) | Which code pushes `ScrollableControls_Mouse` | Scrollables and scroll bars push it (key 0x2, priority 0) for the hovered element while the pointer is over them (§5.7, §6.9); Tessera's input bridge should make its wheel actions active the same way. **Agreement:** decision 0014 (D5, D7) does so through the UI-band context `ui.scroll_mouse`, one wheel step being one line step (§5.6, §6.8); its Tessera rules for plugin controls and captures are in §5.7 | BIN; DEC |
| `ui-layout.md` §6.6 OPEN (which non-root elements own a surface) and decision 0015 (Decision 1.1) | — | **Agreement:** decision 0015 gives alpha-test meters their own surface, as §4.5 says, and applies the threshold at composition (§4.5) | BIN; DEC |
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
6. Viewport and `TargetViewport` drawing (§12; Tessera's composition of a portrait from decision 0015). *Answered in part in revision 3* (§12.1–§12.4); what remains is items 19–21 and 23.
7. The plugin manager's initial info tab (§13).
8. The lookup boundary of the nearest-element search (§2.3).
9. The input-layer condition that suppresses line and page messages (§5.5).
10. The Button flags that exclude double clicks or suppress presses (§7.1).
11. Tree pruning and filtering (§9.1).
12. What the ColorPicker and Browser classes do (§3.5).
13. The exact pop path of the `ScrollableControls_Mouse` entry a scrollable pushes for a hovered
    descendant (§5.7).
14. Which class the game type `LevelProgressMeter` is created with (§13, P3).
15. Whether plugin controls push `ScrollableControls_Mouse`, and which classes (§5.7; Tessera rule from decision 0014: every hovered plugin control pushes it).
16. How a viewport that owns a surface of its own orders its 3D scene against its own media
    and its children (§12; Tessera choice: the scene is a nested entry of the element's own
    surface between its own content and its children).
17. Whether the hover push of `ScrollableControls_Mouse` runs the ancestor walk of UB §5.2
    (§5.7; added by the clean-room implementation, 2026-10-07; Tessera: the single entry).
18. Whether a scroll bar hidden only by the layout data is laid out again (and so shown) during
    creation, after its hide is reported (§6.4; added 2026-10-14 when the implementer OPEN of
    2026-10-11 on step 6 was answered; Tessera: no special case).
19. The opacity-mask arithmetic and the meaning of `UICore_Viewport_RenderToTextureIndex`
    (§12.3; Tessera: the mask's alpha multiplies the target, every viewport has its own target).
20. Where the figure stands in the portrait scene, how a target's `UIViewport_Image` is drawn and
    who reads `UIViewport_UISceneOverride` (§12.1; Tessera: the `character-screens.md` §2.1.2
    marker rule, else the origin).
21. How the bloom, deshade, rim-lighting and fake-shadow values act (§12, `character-screens.md`
    §2.1.1 OPEN).
22. What a confirmation that gets no element does, and what the dialog range check does
    (§18.3; Tessera: an immediate timeout; no range check offline).
23. What the original shows in a portrait while no figure is drawn (§12.4; Tessera: the element
    and its children hidden).
24. Whether the original shows part of a context menu whose item list holds an item of an
    invalid kind, or none of it (§19.2; Tessera: the menu is not shown).
25. How often the original renders a portrait target (§12.5; Tessera: on pose changes, at most
    once per frame and at least 15 times a second).
26. Whether a modal dialog's key consumption depends on the dialog being the active panel, and
    whether it covers keys the UI itself binds (§18.4; Tessera: neither condition; it covers
    them).
27. Whether the original keeps a Menu's popup open on deactivation in some situation (§11.3;
    Tessera: it always closes).
28. Whether the original's modal key consumption also keeps the plugin key broadcast from
    running (§18.4; derived from LPA P48 and W30, not traced as one path; Tessera: no
    broadcast for what the modal rule consumes), and what an earlier receiver of a key sees
    when a confirmation consumes its release (§18.4; Tessera: presses and releases are
    paired, a known difference).
29. Whether the original routes the press and release of a dialog's focus-cycle keys and of
    its entry's editing keys like any other key action (§18.4, revision 7; Tessera: they stay
    outside the pairing and take LPA §13.13's routing, and under a modal dialog what the
    keyboard target does not consume is consumed).

Outside this spec: the Text class's own scrolling (Text is a Scrollable, §3.2, but its line
layout belongs to FO). Unused in data and so not pursued: any reader of
`UICore_Scrollbar_stop_locations` beyond recording it (§6.1).

---

<a id="s18"></a>
## 18. Engine dialog boxes (revision 3; BIN, W30; layout data OBS)

Game code asks the engine for a dialog by handing it a **description**: a property set that
holds `UICore_Dialog_type` and the properties below. This section is what the engine does with
it. `ui-behaviour.md` §5.5 (keys) and UL §9.1 (modal container, overrides) point here.

<a id="s18-1"></a>
### 18.1 Types and elements

`UICore_Dialog_type` (enum; mapper names): `Wait` 1, `Confirmation` 2, `Message` 3, `TextInput` 4
are built by the engine. The engine does not build the other values (`Awaken`,
`AvailableCharSelectDialog`, `BuyNowMessageDialog`, `RenameChar`, `SuppressableConfirmation`,
`ConversionConfirmation`, keys of 0x10000000 and above); this spec does not specify them.

The engine type chooses a top-level element of the layout with `UILAYOUT` label `Dialog`
(key 0x2, 0x2200007C here); `UICore_Dialog_UILayout_Override` and
`UICore_Dialog_UIElement_Override` replace the layout and the element (UL §9.1):

| Type | Element (ID here) | Root (design units) | Parts the engine uses (element ID here) |
|---|---|---|---|
| `Wait` | `DialogWaitField` (0x0C) | `UIScaler`, group 55, origin (256, 100), 512 × 200 | `DialogWaitText` (0x0A) |
| `Confirmation` | `DialogConfirmationField` (0x17) | `UIScaler`, group 55, origin (256, 91), 512 × 183 | `DialogConfirmationQuestionText` (0x14), `DialogConfirmationYesButton` (0x0F), `DialogConfirmationNoButton` (0x21) |
| `Message` | `DialogMessageField` (0x06) | `UIScaler`, group 55, origin (256, 100), 512 × 200 | `DialogMessageStatementText` (0x28), `DialogMessageOkButton` (0x27) |
| `TextInput` | `DialogTextInputField` (0x25) | `UIScaler`, group 55, origin (256, 90), 512 × 180 | `DialogTitleText` (0x33), `DialogTextInputQuestionText` (0x07), `DialogIconField` (0x31), `DialogIconText` (0x32), `DialogTextInputTextEntry` (0x1C), `DialogTextInputDoneButton` (0x04), `DialogTextInputQuitButton` (0x0E) |

Parts are found by element ID below the dialog element (§2.3); a part the element lacks is
skipped. In `DialogTextInputField` the title and icon parts do not exist (they occur in the
game variants `AbandonHouseDialogTextInputField` and `AbandonPremiumHouseDialogTextInputField`
of the same layout).

The confirmation element (OBS): a field `DialogConfirmationVisibleField` at (0, −45), 512 × 183,
holding the frame `DialogConfirmationBoxField` (based on `Box_Silver_BlueGradient`), the question
text (0, 7) 500 × 142 (markup parsed, centred both ways), and `DialogConfirmationButtonField`
(0, 156) 508 × 20 with the yes button at (142, 0) and the no button at (261, 0), both
106 × 20 and based on `TextButton`; only the yes button has `UICore_Element_activate_on_show`.
The text-input element: the frame `DialogTextInputBoxField` 512 × 180 holding the question
(13, 25) 489 × 68, an entry frame (10, 96) 492 × 24 (based on `Box_01`) with the entry
(2, 2) 488 × 20 (editable, selectable, one line, `UICore_Text_max_chars` 100), and a button
field (0, 151) 512 × 20 with the done button (184, 0) and the quit button (264, 0), both 64 × 20.
The message element: the frame holds an outer text field (10, 22) 493 × 121 with the statement
text, and the OK button (192, 170) 96 × 20. Button labels not set by the description are the
layout's own (`UICore_Text_entry`). The same layout also holds the game variants
`DialogConfirmationField_WithSuppressionCheckbox`, `DialogConfirmationYesNoReversedField` (no
button left, yes button right), `SuperWarning_DialogConfirmationField`, `Awaken_DialogMessageField`,
`BuyNowMessageDialogField` and `ChargenDialogWaitField`.

<a id="s18-2"></a>
### 18.2 One dialog at a time

Exactly one engine dialog
is shown at a time; the others wait in a first-in, first-out queue:

1. With no dialog shown, the request is shown at once.
2. Otherwise, a request whose description does not have `UICore_Dialog_topmost` true is added
   at the end of the queue.
3. A `topmost` request while a dialog is shown: if the shown dialog's description has
   `UICore_Dialog_cannotbeoverridden` true, the request is put at the **front** of the queue (it
   is shown next). Otherwise the shown dialog is taken down without a result, its description is
   put back at the front of the queue, and the request is shown at once.
4. When a dialog closes (§18.5), the request at the front of the queue is shown.

What a shown dialog is: the dialog element of §18.1 for its type (or its override), with every
part filled from the description by §18.3; a modal dialog lies inside its modal container
(UL §9.1). From the moment it appears, the dialog is the active panel and lies in front of
the other panels (UB §8), unless `UICore_Dialog_StartInactive` (default false) is true: then
the element that was active before stays active and the dialog is not moved to the front.

> **OPEN (implementer, 2026-10-23; Tessera rule proposed):** (`tessera-ui` dialogs) one dialog
> at a time across owners works through a run-time mark property on the shown dialog. A
> `topmost` request takes another owner's dialog down by marking it, and that owner puts it
> back at the front of its own queue. First-in, first-out holds within each owner's queue;
> across owners the first owner to update after a close shows its next request.
>
> **Answered (spec writer, 2026-10-23, revision 4):** the mechanism (a mark on the shown
> dialog, a queue per owner) is Tessera's to choose; the observable rules are steps 1–4 above,
> client-wide. One part of the proposal departs from step 4 and is replaced. **Tessera rule:**
> however the requests are kept, they are shown in the order of one client-wide queue that
> follows steps 2–4 exactly: when a dialog closes, the request shown next is chosen first among
> the requests that step 3 put at the front (a `topmost` request held back by a
> `cannotbeoverridden` dialog, or a dialog taken down by a `topmost` request), whatever their
> owner, the one put at the front most recently first; only when none of those waits is the
> next request the one that step 2 added at the end earliest, whatever its owner. It is not
> the request of whichever owner updates first. Within one owner this order is the one the
> owner's own queue gives. **Implemented behaviour changes** only when two owners have
> requests waiting at once.

> **OPEN (implementer, 2026-10-23; engineering choices):** (a) a dialog the UI core hid without
> its result reaching the owner is shown again at the owner's next update; (b) a full queue
> refuses a `topmost` request that would push the owner's shown dialog back, and a dialog taken
> down by another owner and put back into a full queue ends the newest waiting request as
> dismissed; (d) the code-made dialog centres its question, statement and wait texts both ways
> and centres its button labels. (Item (c) concerns menus and is answered in §19.4.)
>
> **Answered (spec writer, 2026-10-23, revision 4):** (a) and (b) are confirmed as **Tessera
> rules**. The original's queue has no known length; Tessera's queue is bounded (hostile-input
> rule), and a request ended by (b) gets no result, as a dialog taken down without one
> (§18.5). (d) is confirmed as a Tessera rule for the code-made dialog (the fallback when the
> install's `Dialog` layout does not resolve); the install's elements keep their own
> justification (§18.1).

<a id="s18-3"></a>
### 18.3 Filling the parts

Each text below is a StringInfo of the description, rendered (`string-table.md`) and set as the
part's text.

| Type | Description property → part |
|---|---|
| `Wait` | `UICore_Dialog_wait_text` → wait text |
| `Confirmation` | `…confirmation_questiontext` → question; `…confirmation_yesbuttontext` → yes button; `…confirmation_nobuttontext` → no button; `…confirmation_duration` (seconds) → countdown |
| `Message` | `…message_statementtext` → statement; `…message_okbuttontext` → OK button; `…message_duration` (seconds) → countdown |
| `TextInput` | `…title_text` → title; `…textinput_questiontext` → question; `…icon_field` (an image DID) → the icon field's image (draw mode 4); `…icon_text` → icon text; `…textinput_donebuttontext` → done button; `…textinput_cancelbuttontext` → quit button; `…textinput_displaycancelbutton` (default true) → quit button shown or hidden; `…textinput_response` → the entry's initial text; `…textinput_activateandtakefocus` true → the dialog is activated and the entry takes the keyboard focus; `…textinput_maxchars` > 0 → the entry's `UICore_Text_max_chars` |

**Countdown.** A confirmation or message with a duration *d* closes by itself *d* seconds after
it is shown (§18.5). While it counts, its question (statement) text is rendered again each time
the whole number of remaining seconds changes, with the variable 0x7B035 set to the remaining
seconds truncated toward zero, so a question that uses that variable shows a countdown. If more
than 1.0 s passes between two updates of the dialog, the end time is moved later by that gap
(the countdown does not run while the client stalls). A confirmation whose duration has a
magnitude of at most 0.0002 s gets no element.

> **OPEN:** what follows for a confirmation that gets no element (it is not shown; whether it
> times out at once was not traced). **Tessera rule:** it closes at once with the timeout
> result of §18.5.

**Range check.** With `UICore_Dialog_Range_Check_Entity` (an instance ID) and
`UICore_Dialog_Range_Check_Distance` > 0 the dialog registers a range check that repeats every
2.0 s.

> **OPEN:** what the range check does when the entity is out of range. **Tessera rule:** no
> range check offline.

<a id="s18-4"></a>
### 18.4 Keys and clicks

A confirmation or message dialog reacts to keys only while its element is the active panel and
its description does not have `UICore_Dialog_ignorekeyboardinput` true (`ui-behaviour.md`
§5.5); a text-input dialog reacts to the commands its entry reports. Commands are the key
map's (KM §12): `AcceptKey`, `AcceptInput`, `EscapeKey`.

| Type | Input | Result (§18.5) |
|---|---|---|
| `Confirmation` | click on the yes button, or `AcceptKey` or `AcceptInput` pressed | yes |
| `Confirmation` | click on the no button, or `EscapeKey` pressed | no |
| `Confirmation` | release of `AcceptKey` or `EscapeKey` | consumed, no result |
| `Message` | click on the OK button, or `AcceptKey`, `AcceptInput` or `EscapeKey` pressed | OK |
| `Message` | release of `AcceptKey` | consumed, no result |
| `TextInput` | click on the done button, or `AcceptInput` pressed in the entry | done |
| `TextInput` | click on the quit button, or `EscapeKey` pressed in the entry, while the quit button is shown | quit |
| `Wait` | none | the dialog has no buttons and handles no keys; game code removes it |

**Releases in Tessera (revision 6, pointer only).** The two "release of … consumed, no result"
rows above are the original's behaviour (BIN) and stay the fact. Tessera does not follow
them: a dialog consumes a release exactly when it consumed that key's press, and a release
whose press went past every dialog goes past it too, by the pairing rule of item 3 of the
round-4 answer below ([§18.4, pairing rule](#s18-4-pairing); a Tessera rule, §17 item 28),
within the tracking bounds named there ([§18.4, bounds](#s18-4-bounds)).

A **modal** dialog (`UICore_Dialog_modal` true) also consumes every other key press and the
release of every key that was pressed while it was shown, so no key reaches game code below it.

> **OPEN (implementer, 2026-10-23):** (`tessera-ui` focus) the UI core's dialog key handler
> closes every marked dialog with yes on `AcceptKey` and `AcceptInput`, with no on `EscapeKey`,
> and consumes the releases of all three. So a text-input dialog gives done on `AcceptKey` too
> (the table names `AcceptInput` in the entry), and a message dialog consumes the `EscapeKey`
> release (the table names only the `AcceptKey` release). `EscapeKey` with the quit button
> hidden gives no result; Tessera shows the dialog again. A change of the core's handler would
> make these exact.
>
> **Answered (spec writer, 2026-10-23, revision 4):** the table above is the rule (BIN) and
> stands; the three differences are gaps of the build to be closed in the core's handler:
> a text-input dialog gives done only for `AcceptInput` reported by its entry (an `AcceptKey`
> press elsewhere gives nothing), and a message dialog consumes only the `AcceptKey` release,
> so an `EscapeKey` release after the dialog closed reaches the next handler (the original's
> behaviour; for Tessera this release sentence is **replaced** by the pairing rule of
> [§18.4, pairing rule](#s18-4-pairing), under which that release is consumed). `EscapeKey` in a
> text-input dialog whose quit button is hidden gives no result and the dialog stays shown;
> whether the key is consumed then was not traced (**Tessera rule:** it is consumed). Showing
> the dialog again after the core hid it is an allowed way to keep it shown, provided no frame
> shows the screen without it.

> **OPEN (implementer, 2026-10-23):** (`tessera-ui`, client host) the modal key rule is not
> built: the UI core's dialog key handler takes only `AcceptKey`, `AcceptInput`, `EscapeKey` and
> the focus-cycle keys, and the host does not ask whether a modal dialog is shown before it
> passes keys to game code. No current request sets `modal`. Does the consumption depend on the
> dialog being the active panel, and does it cover keys the UI core itself binds (chat entry,
> panel toggles)?
>
> **Answered (spec writer, 2026-10-23, revision 4):** neither was traced (§17 item 26).
> **Tessera rule:** while any modal engine dialog is shown, it consumes every key press and
> the release of every key pressed while it was shown, whether or not it is the active panel,
> before the UI's own bindings (chat entry, panel toggles, the game menu) and before game code;
> the exceptions are the dialog's own commands of the table above and, for a text-input dialog,
> the keys its entry takes (text, cursor, focus cycle). The rule is needed only once a request
> sets `modal` (none does in the client today, so nothing changes until then).

<a id="s18-4-pairs"></a>
> **OPEN (implementer, 2026-10-23), round 4, three questions on the rules above:** (1) when a
> modal dialog appears, do game-band key holds from before it end at once, with their End
> reaching game code (as when a text entry takes the keyboard, KM §3.3), or do they stay held
> until their own release? (2) While a modal dialog is the keyboard target, does a key press
> the dialog does not consume still raise the UI's key broadcast, so plugins get `KeyDown`;
> do plugin key events count as "game code below it"? (3) The table consumes the release of
> `AcceptKey` on a shown confirmation with no condition, so a key pressed before the
> confirmation appeared reaches its receiver as a press and never as a release (a stuck key).
>
> **Answered (spec writer, 2026-10-23, revision 5):**
>
> 1. **Holds from before stay held until their own release.** The modal rule above consumes
>    only the releases of keys pressed *while* the dialog is shown (BIN, W30); the release of
>    a key pressed before it is not consumed, so it reaches game code as in the original, and
>    movement that key started stops at that release. Appearing does not end holds. The
>    Tessera reading that the dialog ends game-band holds when it appears is **not** taken.
>    Two things still end them by other rules: the dialog's entry taking the keyboard focus (a
>    text-input dialog with `…textinput_activateandtakefocus`, or a click into its entry) ends
>    game-band key holds as any focused text field does (KM §3.3, "Release on the first
>    `opaque_keys` entry"), and focus loss of the window ends every hold (KM §7.3). OS repeats
>    of such a key while the dialog is shown are presses and are consumed (game code ignores
>    repeats for holds: an OS auto-repeat press never starts, resolves or ends a hold, KM §7.2
>    "Presses that are not resolved", and it gives only Repeat offers, KM §7.8.6; revision 6
>    corrects the earlier pointer to KM §7.3).
> 2. **No key broadcast for what the modal dialog consumes.** Plugin key events count as game
>    code below the dialog. The plugin `KeyDown`/`KeyUp` broadcast runs only for a key action
>    that the focus control or active window did not consume (LPA §13.13, P48, BIN), and a
>    modal dialog consumes every key press while it is shown (W30), so no press made while a
>    modal dialog is shown reaches the broadcast, the dialog's own commands and its entry's
>    keys included (they are consumed by the dialog or the entry). This is derived from the
>    two BIN facts, not traced as one path (§17 item 28). **Tessera rule:** the broadcast is
>    skipped for every press and release the modal rule consumes, whether or not the dialog
>    is the active panel (as item 26 of §17 already says for the consumption itself). A
>    release that goes past the dialog (item 3) raises `KeyUp` as usual.
> 3. <a id="s18-4-pairing"></a>**Presses and releases go to the same receiver (Tessera rule; a known difference).** The
>    original consumes the release of `AcceptKey` or `EscapeKey` while a confirmation (and of
>    `AcceptKey` while a message) is the active panel, whatever received the press (the table,
>    BIN); with a key held across the dialog's appearance the earlier receiver then sees no
>    release. Tessera pairs the two halves instead:
>    - a release is consumed by a dialog exactly when the press of that key was consumed by a
>      dialog: it gave a result, or the modal rule took it. That release is consumed even when
>      the dialog is gone by then (the press closed it), so game code and the broadcast see
>      neither half of the key;
>    - a release whose press went past every dialog (it was made before the dialog appeared,
>      or while the dialog did not react to keys) goes past the dialog too, even while a
>      confirmation, message or modal dialog is shown: its receiver, the plugin `KeyUp`
>      broadcast included, gets the release. Under a modal dialog that receiver includes the
>      game band's release-class bindings of the key map: the client host's modal entry is a
>      hold-keeping push, and such a release goes past its band end (revision 8; the band
>      answer item 2 below and [KM §7.3.1](keymap.md#s7-3-1)).
>
>    The pairing covers every key a dialog takes, `AcceptInput` and the `EscapeKey` of a
>    message included, so it **replaces** the sentence of the revision-4 answer above that an
>    `EscapeKey` release after a message closed reaches the next handler (that is the
>    original's behaviour, BIN, and stays the fact; Tessera consumes that release too).
>    The difference from the original shows only for a key held while a dialog appears or
>    closes; no key is left pressed, and no release arrives without its press, in any
>    receiver, as long as the pairing still tracks the key: the client host tracks at most
>    256 consumed holds and the UI core at most 64 held releases, and when a bound forgets an
>    entry that key's later release goes past as an unpaired release (revision 6 pointer; the
>    precisions of the next answer, [§18.4, bounds](#s18-4-bounds)). Holds of the key map
>    itself never stick:
>    they end at the physical release whoever consumes it (KM §7.3 step 1).
>
> **Implemented behaviour changes:** a release of `AcceptKey` or `EscapeKey` whose press came
> before the confirmation is no longer consumed (the regression test that checks the
> release after the dialog has closed stays valid; a test that the release goes on while the
> confirmation is still shown is now correct too).

> **OPEN (implementer, 2026-10-23), round 4, Tessera rules as built (two code packages):**
> (client host) (a) only key holds pressed while a modal dialog is shown are consumed: their
> press, repeats and release, the release even after the dialog closed; at most 256 such holds
> are tracked, the oldest forgotten; keys held from before keep reaching game code; (b) under a
> modal dialog typed text goes only to the UI's focus, never to the chat window's entry or game
> code; (c) the client's own raw keys wait too (the walker's world keys, the NPC windows'
> Escape and Enter, the map and finder keys). (UI core) (a) the release of a key whose press gave
> a confirmation or message result is consumed though the dialog is gone; (b) while a modal
> dialog is shown, a press whose keyboard target lies outside it is not delivered to that
> target, and a held release goes only to a target inside a modal dialog that is still shown;
> (c) an OS repeat of a key that went down before the modal dialog appeared is consumed, its
> release is not held back; (d) held releases are matched by command name, at most 64 at a
> time, and a new press with no modal dialog shown forgets the held one; (e) a dialog whose
> element lies in its modal container (UL §9.1) is modal for keys whatever its description
> says, and a layout panel opened with `UICore_Dialog_modal` is a modal, confirmation-keyed
> dialog; (f) `EscapeKey` in a text-input entry with the quit button hidden gives no result and
> goes on as an ordinary key of the entry (lose focus on Escape applies).
>
> **Answered (spec writer, 2026-10-23, revision 5):** every item is an engineering choice
> with no client fact behind it beyond the table, the modal rule (W30) and the answer above;
> all are confirmed as **Tessera rules**, with these precisions:
>
> - <a id="s18-4-bounds"></a>Client host (a), UI core (a), (c) and (d) are the pairing rule of item 3 above, built in
>   two places; both bounds are confirmed (256 matches the key map's hold limit, KM §7.3; 64
>   is enough for the UI core's command names). When a bound forgets an entry, that key's
>   later release goes past as an unpaired release; nothing else changes.
> - Client host (b) and UI core (b): while a modal dialog is shown, key presses and typed
>   characters reach only a keyboard target inside that dialog (its entry); a focus outside
>   it, the chat window's entry included, gets nothing until the dialog closes. With no
>   target inside the dialog, typed characters are dropped.
> - Client host (c): confirmed; the client's raw keys are UI bindings or game code in the
>   sense of the modal rule.
> - UI core (e): confirmed. No layout in this install sets `UICore_Dialog_modal` (UL §5.9,
>   "0 in layouts"), so the case has no install data; the element lying in a modal container
>   is the stronger test and decides.
> - UI core (f): confirmed, completing the revision-4 rule ("it is consumed"): the entry
>   handles `EscapeKey` as its own key (it gives up the focus when its
>   `UICore_Text_lose_focus_on_escape` is true, UB §5.4), and the key reaches nothing outside
>   the dialog in either case (no game menu, no broadcast; this overrides the "not consumed"
>   branch of UB §5.4 inside a text-input dialog); the dialog stays shown without a result.

<a id="s18-4-band"></a>
> **OPEN (implementer, 2026-10-23), round 6 (also KM §3.3):** KM §3.3 says the first
> `opaque_keys` entry in the UI band ends every game-band key hold, so the host's modal entry
> of the `text_entry` context cannot be pushed while a hold from before the dialog is live
> without breaking item 1 of the revision-5 answer above. As built, the push waits until no
> game hold of a key or pad button from before the dialog is live; in the meantime every
> offered press under the dialog is still consumed. Gap: in that interval a binding without a
> command name (for example `toggle_mount` on Ctrl+M) can reach game code under a modal
> dialog. Proposed: either a Tessera rule that accepts the interval, or a `tessera-input`
> band push that blocks keys without ending game holds.
>
> **Answered (spec writer, 2026-10-23, revision 7):** the second option is the **Tessera
> rule**; the interval is not accepted. An engineering choice with no client fact behind it
> beyond the modal rule (W30) and item 1 above.
>
> 1. When a modal dialog is shown and the band holds no modal entry yet, the client host
>    makes its modal entry of `text_entry` at once, as a **hold-keeping push**
>    ([KM §3.3, hold-keeping push](keymap.md#s3-3-keep)). From that push on, a key or pad-button
>    press that no UI-band context takes reaches no game-band binding, with or without a
>    command name, so `toggle_mount` on Ctrl+M cannot fire under the dialog.
> 2. The push ends no hold: game-band holds from before the dialog stay held until their own
>    release, and their End then reaches game code, as item 1 above says. **Revision 8:** at
>    that release the game band's release-class bindings fire as they would without the
>    dialog, by the pairing rule of item 3 of the revision-5 answer (the press went past every
>    dialog, so the release does too; [KM §7.3.1](keymap.md#s7-3-1), round-8 answer). This
>    replaces the revision-7 sentence that they do not fire. The release of a key pressed
>    under the dialog still fires no game-band binding. Modifier holds from before the dialog
>    stay too, until their own release ([KM §7.4](keymap.md#s7-4), carried modifiers).
> 3. The dialog's entry taking the keyboard focus is still an ordinary push of `text_entry` by
>    the focused element; it is the first ordinary `opaque_keys` entry and ends the game-band
>    key holds then, as item 1 above says (KM §3.3).
> 4. The host pops its modal entry when no modal dialog is shown any more. Nothing ends at
>    the pop (the context keeps the entries of other owners, if any; KM §3.5 otherwise, which
>    ends only holds of `text_entry` itself).
> 5. The wait for the holds from before to end is no longer needed and is removed.
>
> **Implemented behaviour changes:** the modal entry is made when the dialog is shown, not
> after the holds from before have ended; until the change is built, the interval remains a
> known gap.

> **OPEN (implementer, 2026-10-23), round 7 (closing note):** the client host's earlier
> Tessera limit (the modal band push waited for holds from before, and a commandless press
> forced the push early, which ended those holds before their release) is gone. The push is now
> made at once with the keeping variant, so holds from before stay until their own release and
> commandless bindings such as `toggle_mount` never fire under a modal dialog.
>
> **Answered (spec writer, 2026-10-23, revision 8):** noted; the "Implemented behaviour
> changes" of the answer above are built and the interval is no longer a known gap. Two
> precisions from the round-8 answers of `keymap.md`: a release-class binding of the game band
> fires at the release of a key pressed **before** the dialog (item 2 above as revised), and so
> the host's consumption of offered pulse Starts under a modal dialog must not take those
> pulses; and a modifier hold from before stays until its release (KM §7.4). "Commandless
> bindings never fire under a modal dialog" holds for presses made while the dialog is shown.
> **Implemented behaviour changes:** only the pulse exemption of the host, if it consumes
> every offered pulse Start today.

<a id="s18-4-edit"></a>
> **OPEN (implementer, 2026-10-23), round 6, pairing rule:** built so that every press a
> dialog takes holds back its release. That covers results of all four kinds (text-input
> done and quit included) and `EscapeKey` in a text-input entry whose quit button is hidden
> (consumed in every case, per UI core (f)). A dialog's focus-cycle keys and the entry's
> editing keys are not dialog results, so their releases go to the keyboard target and, if
> not handled there, to the broadcast. Confirm that the focus-cycle and editing keys stay
> outside the pairing.
>
> **Answered (spec writer, 2026-10-23, revision 7):** confirmed as a **Tessera rule**, with
> one precision for modal dialogs. Not traced in the original; an engineering choice.
>
> 1. **Outside the dialog pairing.** The pairing rule of
>    [§18.4, pairing rule](#s18-4-pairing) covers the presses a dialog takes as dialog
>    keys: its results and the modal rule's consumption, and `EscapeKey` under UI core (f).
>    A dialog's focus-cycle keys and its entry's editing keys (text, cursor, deletion,
>    selection, clipboard) are not among them. Their press and their release each take the
>    keyboard routing of LPA §13.13: the keyboard target first, then, if it does not consume
>    the action, the plugin broadcast.
> 2. **Precision under a modal dialog.** Such a key pressed while a modal dialog is shown is
>    a key pressed while it was shown: whatever of its press or release the keyboard target
>    inside the dialog does not consume is consumed by the modal rule, so it reaches neither
>    the broadcast (item 2 of the revision-5 answer) nor game code. The branch "if not
>    handled there, to the broadcast" applies only while no modal dialog is shown (a
>    confirmation or message that is not modal), and to keys pressed before a modal dialog
>    appeared (item 3 of that answer).
> 3. **Unpaired key-ups accepted there.** Under a dialog that is not modal, a focus-cycle key
>    whose press the dialog's focus handling consumed (no `KeyDown`) can give a broadcast
>    `KeyUp` at its release, because LPA §13.13 routes the end of a key action on its own.
>    That is accepted. The sentence of item 3 of the revision-5 answer that no release
>    arrives without its press is about the keys the pairing covers.
>
> **Implemented behaviour changes:** only if a focus-cycle or editing key release under a
> modal dialog reaches the broadcast today.

<a id="s18-5"></a>
### 18.5 Results and closing

On a result the dialog writes the result properties below into its description. When
`UICore_Dialog_WSLCallback` is set, that class-library callback receives the description with
the result properties already in it. After a result the dialog's element is gone, and the next
queued request is shown (§18.2 step 4).

| Result | Properties written |
|---|---|
| Confirmation yes / no | `UICore_Dialog_confirmation_response` = true / false |
| Confirmation timeout | `…confirmation_response` = false and `…confirmation_timedout` = true |
| Message OK | `UICore_Dialog_message_response` = true |
| Message timeout | `…message_response` = false |
| Text input done | `UICore_Dialog_textinput_response` = the entry's text |
| Text input quit | `…textinput_response` = an empty text |

A dialog taken down by a `topmost` request (§18.2 step 3) writes no result and is shown again
later from the queue.

<a id="s18-6"></a>
### 18.6 Uses named by other specs

- **Quest abandon** (`quest-journal.md` §4.8): the quest log's abandon request (class library,
  M1) is a description with `UICore_Dialog_type` = `Confirmation`, the chosen question as
  `UICore_Dialog_confirmation_questiontext`, a class-library callback of the quest log as
  `UICore_Dialog_WSLCallback` and the quest's `Quest_DataID`, and no override: the dialog is `DialogConfirmationField` of layout
  `Dialog`. The callback receives `…confirmation_response`; `quest-journal.md` §4.8 says what
  confirming does.
- **Friend and ignore add, note** (`social-panels.md` §3.5, §3.6): `TextInput` dialogs
  (`DialogTextInputField`); the note's current text is `…textinput_response`; done gives the
  entered text, quit gives an empty text, which adds or changes nothing.

**Tessera rule** for the client: an engine dialog is shown for a request of the feed or of the
client's own panels by the rules above, with `DialogBoxes` pushed while it is active (UL §9.1,
IB D5); the description's result reaches the requesting panel instead of a class-library
callback.

<a id="s19"></a>
## 19. Context menus (revision 3; BIN, W31; layout data OBS)

A context menu is built by game code from a list of items and opened over the screen; the
engine draws it from layout `ContextMenu` (0x2200007B here). `ui-behaviour.md` §4.6 (the
`Element_open_context_menu` message on a right press), the menus of `social-panels.md` and
`map-windows.md` §4.5 use it (the finder's menus of `travel-windows.md` and the map window's
`FilterMenu` are Menus, §11; revision 4).

<a id="s19-1"></a>
### 19.1 Layout

| Element (ID here) | Use | Design box and parts |
|---|---|---|
| `ContextMenuElement` (0x12) | menu without a title | `UIScaler`, group 55, object mode 3, `UICore_ContextMenu_open_delay` 0.5; 30 × 30; `Background` (based on `Box_01`), `ContextMenuList` (0x11) a ListBox at (2, 2) 26 × 26 with one column and the vertical scroll bar `ContextMenuList_Scrollbar`, `NormalBackground` |
| `ContextMenuWithTitleElement` (0x22) | menu with a title | as above, plus `ContextMenuTitle` (0x1A) at (0, 0) 30 × 16 holding `ContextMenuTitleText` (0x1F); the list at (2, 16) 28 × 14 |
| `ContextMenuElement_MapMenu` | the map window's menus (`map-windows.md` §4.5) | as `ContextMenuElement` with a `Menu_Brown_Background` frame, origin (0, 0), no object mode |
| `ContextMenuEntry` (0x0B) | plain item | 28 × 16; icon `ContextMenuEntryIcon` (0x10) (4, 0) 16 × 16; text `ContextMenuText` (0x19) (22, 2), font of `menu_font`, max 400 × 400; states `normal`, `normal_rollover`, `ghosted` |
| `ContextMenuBool` (0x18) | check item | 44 × 18; icon and text as above; check mark `ContextMenuBoolFlag` (0x02) (26, 0) 16 × 16 at the right edge (edges 4, 0, 5, 0); same states |
| `ContextMenuSub` (0x05) | submenu item | 44 × 18; icon and text as above; arrow `ContextMenuOpen` (0x08) (26, 0) 16 × 16 at the right edge; same states |

<a id="s19-2"></a>
### 19.2 The item list

A menu is given an ordered list of items and optionally a title, a fixed position and an element
ID that replaces the menu element. Each item has: a kind (1 check item, 2 submenu, 3 plain
item), a label text, an optional icon (image DID), an enabled flag,
for check items a checked flag and a flag whether the check mark shows, an item ID and value
reported on selection, a flag that selection reports a close instead of a selection, and for
submenus the submenu's own item list. Other kind values are not valid menu items.

> **OPEN:** whether the original shows the items before an item of another kind or shows no
> menu at all was not traced. **Tessera rule:** a menu (or submenu) whose list holds an item of
> another kind is not shown, and the request has no other effect.

<a id="s19-3"></a>
### 19.3 What a menu looks like and where it opens

A **menu tree** is a root menu together with all its submenus, at any depth.

- **Menu element.** The given override, else `ContextMenuWithTitleElement` when the title is not
  empty, else `ContextMenuElement`. A menu blocks clicks (`UICore_Element_blocks_clicks` true)
  and lies in front of the item that opened it (a root menu in front of the screen's panels).
  Its `UICore_ContextMenu_lvl` is its depth: 0 for a root menu, the parent menu's depth + 1 for
  a submenu. With a title, the title text is shown and the title field is as wide as the text
  plus the template's margins.
- **Item templates.** Kind 1 uses `ContextMenuBool`, kind 2 `ContextMenuSub`, kind 3
  `ContextMenuEntry`. An item shows its label in its text part and, when it has an icon, that
  image in its icon part (draw mode 4). Its `UICore_ContextMenu_lvl` is its menu's depth and
  its `UICore_ContextMenu_index` its position in the item list. An enabled item is in state
  `normal`, a disabled one in `ghosted`.
- **Item size.** Each item's width is its text width + 10 pixels + (the template's item width −
  its text part width), and its height is the text height + (the template's item height − its
  text part height) (W31).
- **Common width.** Every item of a menu has the width of the menu's widest item, and at least
  the title's width. The list is that wide and as high as the sum of its item heights, but no
  higher than 0.8 × the screen height (W31; beyond that the list scrolls); the menu element is
  the list's size plus the margins around the list in the template.
- **Scale to fit.** A menu larger than the screen is scaled down uniformly by the smaller of
  screen width ÷ menu width and screen height ÷ menu height; a menu is never scaled above its
  own scale.
- **Placement.** A given position is the menu's top-left corner (one variant gives its
  bottom-left corner, so the menu opens upward). A submenu opens at the right edge of its parent
  item with its top level with the item, or to the left of the parent item when it would leave
  the screen on the right. Otherwise the menu's top-left corner is at the pointer. In every case
  the menu is then moved the least distance that puts it wholly on the screen.
- **Check marks.** A check item's mark is visible or hidden by the item's show flag, and is in
  state `normal_activated` when the item is checked, else `ghosted`. The template defines no
  `normal_activated` state, so a checked mark shows its own block (UL §9.4 step 1).
- **Submenus.** A submenu is not visible until it is opened (§19.4).
- **Closing.** The whole menu tree closes on a selection (§19.4 step 3) and on any activation
  outside the menu tree (§19.4 step 4).

> **OPEN (implementer, 2026-10-23; engineering choices):** (`tessera-ui` context menus) "text
> width" is read as the shaper's content width, which includes the text part's margins. The
> upward variant puts the menu's bottom edge just above the given *y*, as §11.3 `open_up` does.
> Submenus are built when they open, so a submenu holding an invalid kind is not shown at that
> moment. A submenu opens on hover only when its item is enabled. Items take a Tessera element
> ID so that registering their mouse messages does not make other layouts' elements with the
> template IDs mouse-visible.
>
> **Answered (spec writer, 2026-10-23, revision 4):** all five are confirmed as **Tessera
> rules** (none was traced in the original): text width as the content width with the text
> part's margins; the upward variant's bottom edge one unit above the given *y*; a submenu
> whose list holds an invalid kind is not shown when it would open (§19.2's rule, applied at
> opening); a disabled submenu item opens nothing, by hover or by click (§19.4 step 3 already
> ignores clicks on it); items carry a Tessera element ID. A menu measured before its font has
> loaded is built again at the same place once its text can be measured, and a root built
> again closes its open submenu (implementer item (c) of §18.2): also confirmed as Tessera
> rules.

<a id="s19-4"></a>
### 19.4 Hover, selection and closing

1. **Hover.** When the pointer enters an item (`Element_mouse_over_top`), an enabled item takes
   `normal_rollover` (a disabled one `ghosted`), and the open delay of step 2 starts counting;
   when it leaves, the item returns to `normal` (or `ghosted`).
2. **Open delay.** When the pointer has rested on the same item for longer than the menu's
   `UICore_ContextMenu_open_delay` (0.5 s), a submenu item opens its submenu; any other item
   closes this menu's open submenu.
3. **Click** (`Element_mouse_click`) on an enabled item: an open submenu of another item is
   closed first; a submenu item opens its submenu, or closes it when it is open; any other item
   is **selected**: for a menu that a UI element opened, its listener receives
   `Element_select_context_menu` with the item's ID and the item (or
   `Element_close_context_menu` with the ID and value for an item flagged to report a close);
   a check item's mark flips between shown and hidden; for a menu opened on a game object the
   item's command goes to game code. Then the whole menu tree closes. A click on a disabled item
   does nothing.
4. **Deactivation.** When the player activates any element outside the menu tree (for example
   by clicking elsewhere), the whole menu tree closes and the listener receives
   `Element_close_deactivated_context_menu`.

**Tessera rule** for the client (revised in revision 4): the row menus of `social-panels.md`
§3.5, §3.6 and §4.5 and the map window's `MapMenu` (one submenu per region entry,
`map-windows.md` §4.5) are context menus built by §19.2–§19.4 from their specs' item lists.
The map window's `FilterMenu` and the finder's size and tier menus are **not** context menus:
they are Menu elements (§11) whose popups the map window and the finder fill (§11.4, W33).

> **OPEN (implementer, 2026-10-23):** (`tessera-ui` context menus, `game::mapwindow`) step 3
> closes the whole tree after a selection, but `map-windows.md` §6.9 says `FilterMenu` stays
> open (`UICore_Menu_auto_close_on_selection` false). Tessera: a request flag that keeps the
> tree open, set only for `FilterMenu`; a selection there flips the mark and the tree stays
> open. Please say which rule `FilterMenu` follows.
>
> **Answered (spec writer, 2026-10-23, revision 4; W33):** `FilterMenu` follows §11, not this
> section. In the install it is an engine Menu (based on `MapNoteFilterMenu` of the `map`
> layout) with a popup of layout 0x22000080 and `ContextMenuBool` entries from layout
> `ContextMenu`; `auto_close_on_selection` false is a Menu property, which only the Menu class
> reads, so its popup stays open after a selection (§11.4) and closes on a second click of the
> menu, or when it is deactivated (§11.3). The engine context menu has no keep-open setting:
> every context-menu tree closes on a selection (step 3). **Implemented behaviour changes:**
> `FilterMenu` opens its popup below the menu by §11.3 (the popup's top-left at the menu's
> screen *x* and its last row) with the popup's art and scroll bar, and the keep-open request
> flag of the context menu is retired. The same holds for the finder's size and tier menus
> (`travel-windows.md` §6.8 rule 7 (e)), which close after a selection
> (`auto_close_on_selection` is not set on them, so it is true).

> **OPEN (implementer, 2026-10-23):** (`map-windows.md` §4.5, `travel-windows.md` §6.8 rule 7
> (e)) neither the map menus nor the finder's size and tier menus have a position stated.
> Tessera: a fixed top-left position at the bottom-left corner of the menu's button. The quest
> tracker menu opens at the pointer.
>
> **Answered (spec writer, 2026-10-23, revision 4):** the map window's `MapMenu` gives its
> context menu a fixed position, its button's left edge and three units below the button's
> last row, and guards against reopening for 0.2 s (`map-windows.md` §4.5, MW36). The finder's
> menus and `FilterMenu` are Menus, placed by §11.3 (above). A context menu opened without a
> position (the quest tracker menu, the social row menus) opens at the pointer (§19.3).

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First complete draft (status Review), answering the M5 completeness critique items A2 and C1: factory and class table (§3); Meter child, frame and alpha-test modes with exact fill geometry per direction, position, goal and smooth movement (§4); Scrollable offsets, scroll-bar update, scroll-to clamp, line and page steps, wheel and hover (§5); Scrollbar parts, track, thumb geometry, value mapping, stops, arrow buttons, paging, dragging with the 100-pixel rule, slider mode, wheel, disabled and ghosted (§6); Button auto-repeat, click on release, boolean toggle and state-request translation (§7); ListBox templates, grid layout and selection (§8); Tree nodes, indentation, icons and expand (§9); Panel pages and tabs (§10); Menu popup (§11); Viewport properties (§12); checklist mapping (§13); the `ScrollableControls_Mouse` push on hover (§5.7, §6.9); invariants WI1–WI14 and worked examples. OBS + BIN, client 4906.0071.1794.4035. |
| 2026-10-06 | **Revision 1** (answers the review of the same day; status stays Review). **Blocking 1:** new §3.4 GroupBox (selection change, completion-pass default, clicks with the stop answer, census) and §3.5 ColorPicker and Browser with an OPEN and a Tessera rule (§17 item 12); §3.2 rows; §13 P8-S names the four Panels, the two GroupBoxes and the four Menus; §16 row 1. **Blocking 2:** §7.3 states that the stops apply only when that step changed `highlighted` (BIN re-trace, W26); WI12 extended; §15.7. **Blocking 3:** provenance rows W22 (panel composition, OBS) and W23 (censuses of §7.2, §8.3, §10.5, §11.4, OBS); W11 and W18 point to W23. **Minor:** (a) §10.5 counts 286 entries and 572 IDs; (b) §6.6 uses *a* and *len*; (c) §5.7 OPEN on the pop path (§17 item 13); (d) §9.3 step 1 (indent added in every case, parent without entry), the width reserve explained, §8.3 cell use under `ignore_cell_alignment` (vertical lists unaffected). **Also found (BIN, same build):** the completion pass (§3.4, §5.1, §9.1, §7.3; W28), which closes the §5.2 OPEN (§17 item 3) and attaches scrollables to their bars as listeners; Trees force `click_select` and `layout_only_visible` true; descendant lookup order (§2.3); the track's mutual test for arrow buttons (§6.3); selection display 0 (§11.4). **Corrected:** §13 P7 sliders do have arrow buttons (adopted siblings, §6.2); §13 P3 has no cap pieces, and the `LevelProgressMeter` element's own properties and states are given, with an OPEN on its class (§17 item 14); §11.4 census (194 of 195 menus have a rollover state). New W24–W28, WI15–WI17, §15.8–§15.10, §16 rows for UL §9.2 and UB §4.3. |
| 2026-10-06 | **Revision applying the accepted decisions 0014 (UI input bridge) and 0015 (UI rendering)** (status Accepted → Revised, review pending). No fact about the original is changed. New provenance tag DEC. §4.5: Tessera rule for where the alpha threshold acts (the meter's own surface, applied at composition, compared with the pixel's own alpha before opacity; a position change repaints nothing). §5.7: Tessera's mapping of the `ScrollableControls_Mouse` pushes to the context `ui.scroll_mouse` and the wheel-motion rule, with the capture and pointer-lock consequences; new OPEN callout with Tessera rule on plugin controls (every hovered plugin control pushes), §17 item 15. §6.9: pointer to §5.7. §12: Tessera rule for composing a portrait as external content (offscreen target at the layout-box size, linear RGBA, premultiplied, separate entry). §16: the `ScrollableControls_Mouse` row records the agreement with decision 0014; new agreement row for alpha-test meter surfaces. §17 item 6 updated. Section numbers unchanged. |
| 2026-10-06 | **Fixes after the review of the decision-0014/0015 revision** (status stays Revised, review pending). §4.5: the Tessera rule now states that applying the alpha threshold before the entry's opacity departs from decision 0015 §8 ("UI pipeline" row) as written and follows the decision's open review item "Alpha-threshold order"; no M5 panel is affected. §12: new OPEN and Tessera rule for a viewport element that also owns a surface (both M5 portraits: `PlayerPortrait` mode 3, `OpponentPortrait` mode 1 with an own image and a mode-1 child; data from `ui-layout.md` U47): the external content is a direct nested entry of the element's own surface, between its own content and its children. §17 item 16 added. No fact about the original is changed. |
| 2026-10-06 | **Final-pass fixes** (status stays Revised, review pending). No fact about the original is changed. §4.5: one note that it is not known whether the meter's surface flag is the "flagged by code" of UL §8.3 and §8.4 step 1, pointing to the UL §8.4 scope OPEN and its Tessera choice (it is not; no M5 panel affected). §16 row on UL §6.6 extended to match. |
| 2026-10-14 | **Revision 2** (spec writer, dirty room; status Accepted → Revised, awaiting independent review). Answers the §6.4 implementer OPEN of 2026-10-11 (from `social-panels.md` TR14): new provenance row W29 (BIN question (m) on build 4906.0071.1794.4035, Ghidra 12.1.3 headless on a private copy; OBS census); §6.4 states that step 6 and the fill parts of step 5 write the element's one visibility, the one `UICore_Element_hide` also writes, so a bar's next layout overrides an earlier hide unless it is disabled with `hide_when_disabled`; the original's fellowship row writes the volume control's visibility after its position. The OPEN is closed; a narrower OPEN (whether a layout runs during creation after the hide is reported) is added with a Tessera rule (no special case), §17 item 18. §6.10 gains the census row of layout-hidden bars (4 of 792). **Implemented behaviour:** unchanged (Tessera's step 6 already shows the bar; the fellowship rows' disabled-with-`hide_when_disabled` choice is confirmed). |
| 2026-10-14 | **Review of revision 2** (spec reviewer, independent session): **passed; status Revised → Accepted.** Spot checks and one non-blocking note in the header's review record; no other text changed; implemented behaviour unchanged. |
| 2026-10-23 | **Revision 3** (spec writer, dirty room; status Accepted → Revised, awaiting independent review). BIN revision 3 (questions (n)–(p), build 4906.0071.1794.4035, full SHA-256, Ghidra 12.1.3 headless on a private copy) and new rows W30–W32. **§12:** the drawing OPEN answered in part; new §12.1 (what a `TargetViewport` shows: entity, portrait option, the target's `UIViewport_Array` description, camera from `UI_TargetViewport_CameraPropertyName` else `Camera_1`, census), §12.2 (the portrait scene 0x21000004), §12.3 (render target, background, opacity mask as Tessera rules), §12.4 (Tessera rule while no figure is drawn, answering the `combat-ui.md` §5.1 implementer OPEN for Tessera's side). **New §18** (engine dialog boxes: types and elements of layout `Dialog`, parts, countdown, keys, results, queue, the quest-abandon and social uses) and **§19** (context menus: layout `ContextMenu`, item kinds, building, placement, hover, open delay, selection, deactivation). §0 and the title name the new sections; §17 item 6 updated, items 19–23 added. No section number reused. **Implemented behaviour:** none changes; §12.4 records the implemented portrait hiding as a Tessera rule; §18 and §19 describe widgets Tessera has not built yet. |
| 2026-10-23 | **Revision 3, screen fixes** (spec writer, dirty room; status stays Revised, awaiting independent review). Wording only, no fact changes: §12.3 first paragraph states which viewport properties affect the drawn portrait and that the mask is applied to the rendered scene, instead of internal structure; §18.1 says only that the engine does not build the other `UICore_Dialog_type` values; §18.5 keeps only the observable order (result properties present when the class-library callback receives the description, element gone, next queued request shown) and drops the listener step; BIN question (p) and row W32 reworded to match. Row W30 now lists the decision-0012 numeric constants of §18.3 (string variable 0x7B035, 0.0002 s, 1.0 s, 2.0 s). For the reviewer: confirm that the M4 passes (n)–(p) are within this package's scope. |
| 2026-10-23 | **Revision 3, review fixes** (spec writer, dirty room; status stays Revised, awaiting independent review). (B1) Row W31 now lists the decision-0012 numeric constants of §19.3: the 0.8 factor of the list's maximum height and the fixed 10 pixels of the item width (step 4 now names the unit). Non-blocking (a): the §18.2 dialog-number sentence is dropped, since nothing in this spec observes the number. No fact or behaviour changes. |
| 2026-10-23 | **Revision 3, round 4** (spec writer, dirty room; status stays Revised, awaiting a new independent review). Wording only, no fact changes: §18.2's showing paragraph states what a shown dialog is and which element is active and in front; §19.3 is restated as observable results (renamed "What a menu looks like and where it opens": the menu tree, menu element, item templates and states, item size with the W31 constant, common width with the 0.8 cap, scale to fit, placement and clamping, check marks, submenu visibility, when the tree closes) and no longer lists build steps; §19.4 steps 1 and 4 no longer name internal bookkeeping (deactivation is any activation outside the menu tree); W31's references follow §19.3's new paragraph names; W29 and the §6.4 "Visibility is one flag" paragraph state the visibility fact without naming routines, W29 cites BIN (m); §18.6 states the quest-abandon description, not how it is built; the header's review records are reduced to summaries (decision 0018). |
| 2026-10-23 | Independent review of revision 3 after round 4 (spec-review:spec-ui-input:1, independent session): changes requested (§19.2 wording; header review record). Status stays Revised. No content change. |
| 2026-10-23 | **Revision 3, round 5** (spec writer, dirty room; status stays Revised, awaiting a new independent review). §19.2: an item of another kind is no longer worded as stopping a build; other kind values are not valid items, and a new OPEN records that whether the original shows part of such a menu or none was not traced, with the Tessera rule that such a menu is not shown (§17 item 24). §18.6 and W30: the quest abandon callback is named as a class-library callback of the quest log, not a routine or handler. BIN revision-3 cell: the list of code kinds read is cut to "BIN (n)–(p)". No other fact changes. |
| 2026-10-23 | Independent review of revision 3 after round 5 (spec-review:spec-ui-input:2 (independent session)): passed; status Revised → Accepted. No content change (header only). |
| 2026-10-23 | **Revision 4** (spec writer, dirty room; status Accepted → Revised, awaiting independent review). Round-3 implementer OPENs filed and answered; section numbers unchanged, one new subsection. BIN revision 4 (question (q), same build and SHA-256, Ghidra 12.1.3 headless on a private copy) and rows W33 (which classes read `auto_close_on_selection`, Menu deactivation, the Menus of the map window and the finder) and W34 (the `EnablePortraits` preference, the portrait masks' format). §11.3: the popup closes when deactivated (BIN), Tessera rule for the click on an open menu. §11.4: the OPEN answered in part (the map window's and the finder's Menus), Tessera rule for popup sizing. §12: the composition paragraph points to §12.3 for the target's content and size (the clear-colour conflict with `render.md` §6.10 reconciled). §12.1: portrait option read from `EnablePortraits` (Tessera rule); the step 3–4 gaps recorded with an order of work. §12.2: the head-and-shoulders stand-in camera confirmed until the scene is drawn. §12.3: result-versus-means of the background clear; mask formats and the engineering limits confirmed. New §12.5: the portrait figure animates (Probable) with Tessera redraw rules. §17 items 25–26. §18.2: client-wide order across owners (Tessera rule; implemented behaviour changes when two owners wait); engineering choices (a), (b), (d) confirmed. §18.4: the key-table gaps of the build named; modal scope as a Tessera rule. §19.3: five engineering choices and the menu rebuild confirmed. §19.4: the client rule corrected (`FilterMenu` and the finder's menus are §11 Menus; implemented behaviour changes), `FilterMenu`'s keep-open and the menu placement answered. |
| 2026-10-23 | Independent review of revision 4 (spec-review:spec-ui-r4:1 (independent session)): changes requested. (1) §2.4: the message names `Menu_new_selection` (§11.4, W33) and `Element_deactivated` (§11.3) have no row with their keys in the message table; the spec writer adds both with the keys from the install's `UIElementMessage` mapper. (2) §11.3 "Deactivated": the Probable clause about a condition "seen" under which the popup stays open reads as a description of a code branch; restate it as a behavioural unknown with an OPEN callout (and a §17 item). Status stays Revised. No content change (header and this row only). |
| 2026-10-23 | **Revision 4, review round 1 fixes** (spec writer, dirty room; status stays Revised, awaiting a new independent review). (1) §2.4: rows 0x1C `Menu_new_selection` and 0x2F `Element_deactivated` added to the message table, keys read from the install's `UIElementMessage` base mapper 0x230001AC (OBS, 2026-10-23; recorded in W33); the existing key 0x15 `Menu_opened` was re-checked against the same mapper. (2) §11.3 "Deactivated": the Probable clause is removed; whether the original ever keeps the popup open on deactivation is now an OPEN callout with the Tessera rule that a deactivated popup always closes, and §17 item 27. No other fact changes; section numbers unchanged. |
| 2026-10-23 | Independent review of revision 4 after the round-1 fixes (spec-review:spec-ui-r4:2 (independent session), which wrote no part of it): **changes requested**; status stays Revised. The round-1 fixes hold (§2.4 keys 0x1C `Menu_new_selection` and 0x2F `Element_deactivated` agree with the install's `UIElementMessage` mapper; §11.3 is a behavioural OPEN with §17 item 27). Blocking: §18.2, answer to the cross-owner OPEN: the Tessera rule says steps 1–4 hold client-wide, but its order ("the request that has waited longest among all owners' queues", with a request put back by step 3 counting only as the oldest of its owner's) can show another owner's older request before a dialog that step 3 put back at the front of the queue, so it departs from step 3 when two or more owners wait; "waited longest" is also undefined for a put-back request. State an order that reproduces steps 3 and 4 client-wide (every request step 3 puts at the front is shown before any other waiting request, the latest put-back first), or mark the departure as a Tessera rule that replaces step 3 across owners. Spot checks passed (own read-only probes): layout 0x22000080 (`Menu_01`, `Menu_02` and their popups, lists and scroll bars), the finder's `…_SizeMenu` (based on `Menu_01`) and `…_TierMenu` (type `FellowshipMakerTierMenu`, popup `Menu_01_PopupField`, items `Menu_01_PopupItem`, its own selection display), scene 0x21000004 (`Camera_1` about 5.8 m from the origin and 1.9 m up, `CameraFOV` 60, `CameraFarDistance` 20, three Light elements), the mask lists 0x40000E4B and 0x40000E4C (five DXT3 levels, 64 × 64 to 4 × 4, the largest in the high-resolution archive), `EnablePortraits=True` in `[Render]` of both preference files, and 1,014 + 592 = 1,606 `Opponent` and `Self` descriptions. BIN (q) was not re-traced. No other text changed. |
| 2026-10-23 | Revision 4, round-2 fixes (spec writer, dirty room; status stays Revised, awaiting independent review). Section numbers unchanged; no new facts. **§18.2**, answer to the cross-owner OPEN: the Tessera rule now states one client-wide order that reproduces steps 2–4: every request that step 3 put at the front is shown before any other waiting request, whatever its owner, the most recently put-back first; then the requests step 2 added at the end, earliest first. The undefined "waited longest" wording is gone. **Implemented behaviour changes** only with two or more owners waiting at once. |
| 2026-10-23 | Independent review (spec-review:spec-ui-r4:3 (independent session)) of the round-4 revision: **passed**, CLEAN_ROOM.md §5.3 every item; contamination screen clean; status Revised → Accepted. Header status and review record only; no other text changed. |
| 2026-10-23 | **Revision 5** (spec writer, dirty room; status Accepted → Revised, awaiting an independent review). No new research; no fact about the original changed; section numbers unchanged (new anchor `s18-4-pairs`). Round-4 implementer OPENs filed and answered: **§18.4** (1) game-band holds from before a modal dialog stay held until their own release (from the W30 modal rule; the dialog's entry taking the focus still ends them by KM §3.3); (2) no plugin key broadcast for what the modal rule consumes (derived from LPA P48 and W30; Tessera rule); (3) presses and releases of a key go to the same receiver (Tessera rule, a known difference from the original's unconditional release consumption: a release whose press went past every dialog goes past it too, a release whose press a dialog consumed is consumed even after the dialog closed, which replaces the revision-4 sentence on the `EscapeKey` release of a message); the two code packages' built choices (client host (a)–(c), UI core (a)–(f)) confirmed as Tessera rules with precisions (typed text only to a target inside the modal dialog; `EscapeKey` in a text-input entry without quit button consumed in every case). **§12.4:** an entity drawn only through the object layer shows no portrait for now (Tessera rule; a gap of the build). §17 item 28 added. **Implemented behaviour changes:** a release of `AcceptKey` or `EscapeKey` whose press came before a confirmation is no longer consumed. |
| 2026-10-23 | Independent review (spec-review:spec-ui-r5:1 (independent session)) of revision 5: **passed**; status Revised → Accepted; three non-blocking notes in the review record. Header only (status, reviewer and review record); no fact, rule or section changed. |
| 2026-10-23 | **Revision 6** (spec writer, dirty room; status Accepted → Revised, awaiting an independent review). Answers the three non-blocking notes of the revision-5 review in §18.4; pointers and wording only, no new research, no fact about the original and no Tessera rule changed; section numbers unchanged (new anchors `s18-4-pairing` on item 3 of the round-4 answer, `s18-4-bounds` on its precisions bullet). **N1:** item 1 now cites KM §7.2 ("Presses that are not resolved") and KM §7.8.6 for "repeats are ignored for holds", not KM §7.3. **N2:** item 3's "no release arrives without its press" is bounded by the client host's 256 consumed holds and the UI core's 64 held releases, with a pointer to the precisions bullet that says an entry a bound forgets gives an unpaired release. **N3:** a paragraph after the §18.4 table and a parenthesis in the revision-4 answer point from the original's release rows and from the `EscapeKey`-release sentence to the pairing rule that replaces them for Tessera. **Implemented behaviour changes:** none. |
| 2026-10-23 | Independent review (spec-review:spec-ui-r6:1 (independent session)) of revision 6: **passed**; status Revised → Accepted. Header only (status, reviewer and review record); no fact, rule or section changed. |
| 2026-10-23 | **Revision 7** (spec writer, dirty room; status Accepted → Revised, awaiting an independent review). No new research; no fact about the original changed; section numbers unchanged (new anchors `s18-4-band`, `s18-4-edit`). Round-6 implementer OPENs filed and answered in **§18.4** as Tessera rules: (1) the client host makes its modal entry of `text_entry` at once when a modal dialog is shown, by the new hold-keeping band push of KM §3.3 (anchor `s3-3-keep`), so no key reaches game code under the dialog and holds from before still end only at their own release; the wait for those holds is removed; the dialog's entry taking the focus still ends game-band key holds; (2) a dialog's focus-cycle keys and its entry's editing keys stay outside the pairing rule and take LPA §13.13's routing; under a modal dialog what the keyboard target does not consume is consumed (no broadcast); an unpaired `KeyUp` under a non-modal dialog is accepted. **§17:** item 29. **Implemented behaviour changes:** the modal entry is pushed at once (needs the `crates/input` push kind); a focus-cycle or editing key release under a modal dialog no longer reaches the broadcast, if it does today. |
| 2026-10-23 | Independent review (spec-review:spec-ui-r7:1 (independent session)) of revision 7: **passed**; status Revised → Accepted. Header only (status, reviewer and review record); no fact, rule or section changed. |
| 2026-10-23 | **Revision 8** (spec writer, dirty room, package spec-r8; status Accepted → Revised, awaiting an independent review). No new research; no fact about the original changed; section numbers and anchors unchanged. §18.4: the round-7 implementer closing note filed and answered (the at-once hold-keeping modal entry is built); item 2 of the revision-7 band answer replaced (game-band release-class bindings fire at the release of a key pressed before the modal entry, following the pairing rule and `keymap.md` §7.3.1 round 8; modifier holds from before stay, KM §7.4); revision-5 answer item 3 gains the pointer asked for by review note N3. **Implemented behaviour changes:** the client host must let through offered pulse Starts fired at the release of a key pressed before a modal dialog; the two mapper changes of the same rules are listed in the round-8 row of `keymap.md`'s change log. |
| 2026-10-23 | Independent review (spec-review:spec-r8:2 (independent session)) of revision 8: **passed**; status Revised → Accepted. Header only (status, reviewer and review record); no fact, rule or section changed. |
