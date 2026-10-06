<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# UI runtime behaviour: input routing, focus, capture, tooltips, drag and drop, window management, UI scale and UI text

| | |
|---|---|
| **Status** | Accepted |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (dirty room), AI-assisted |
| **Reviewer** | Claude review session (spec-writer agent) |
| **Review record** | First review: Claude review session (spec-writer agent), review 2026-10-06: **changes requested** (three blocking items, four minor). Blocking 1 (§5.3 "each fact ... marked Confirmed"; UB19, UB20, §6.3 last sentence): the claim that each tooltip variant's "only child is a Text element" is false. All 4 top-level elements of the tooltip layout have **two** children: a Field (z-level 1, at (0, 0), 512 × 512, edges (5, 5, 5, 5)) behind the Text element (z-level 0). Everything else in that sentence matches. Fix the sentence, and say in §6.4 step 2 that the Field follows the panel by its edges as the Text element does. Blocking 2 (§5.3 "invariants concrete enough to test"; UB-I9 against §8.2 step 4): "min limits hold when set" does not follow from the algorithm. Steps 4.4 (aspect ratio) and 4.6 (screen caps) run after the minimums of step 4.3. Example: the §13.5 setup with `min_height` 240 gives *H* = trunc(350 ÷ 1.5) = 233 < 240. A correct implementation would fail the invariant. Restrict UB-I9 to what holds (for example: min limits hold when no aspect ratio is set and the caps do not apply), or change §8.2 if the BIN trace says otherwise. Blocking 3 (§5.3 "unknowns are marked OPEN"; §8.1 step 3, UB-I8): the move clamp lists four inequalities but no order. A target wider or taller than its parent (a root panel larger than a small window) cannot meet all four, so the result is undefined and UB-I8 cannot hold. State the order, as §9.2 step 5 does, or add an OPEN and limit UB-I8 to targets that fit inside their parent. Minor (not blocking): (a) §6.2 says nothing about elements whose hover handler shows no tooltip (`tooltips_on` false, no text, `TooltipEnable` false). Say whether the handler, and `Element_mouse_hover`, run again on every frame after the delay or once per hover, or add an OPEN. (b) §5.2 step 1: say whether giving focus to *F* pops the active panel's map. "Push again" on clearing focus otherwise reads as a second push. (c) §3.3: the Windows default values (500 ms, 4 × 4) have no provenance row. (d) §16 row 2 ("through a chain of subtractions, not an immediate"), the §5.6 OPEN ("no comparison with these IDs ... jump table") and the §3.3 OPEN ("tests a further flag before the comparison") describe machine-code structure, not behaviour. Rephrase them as plain unknowns. Every other §5.3 item passes. The algorithms are prose with numeric constants allowed under decision 0012. Nothing has code or listing structure, and no prose is copied. There is no game text, key, hostname or traffic. The example "level 5" is invented, the template text is not reproduced, and the property, enum, command, map and preference names are interop identifiers covered by the naming note. Every fact has a row UB1–UB39 with a mark (the BIN and Probable marks follow the Accepted `ui-layout.md`). Every §17 item has an in-section callout. No network, login or server material is involved, so no closed gate applies. The spec is implementable apart from the three items above. I recomputed every §13 example by hand and all of them match: 13.1 timing, 13.2 hit test, 13.3 placement 1704 / 1032 and size 216 × 18, 13.4 threshold, 13.5 resize 350 × 233, 13.6 order *Y*, *X*, *Z*, 13.7 scale (70, 80). They also agree with UL §9.3 sibling order and KM §12.5.7 bits. Spot checks, read-only on the install 2026-10-06, with independent probes. (1) UB39 / §15 over all 302 layout records: `HoverDelay` 243 (242 × 0.1, 1 × 300); `AutoFade` 105 (101 × 0.1, 4 × 0.5); `dragable` 10; `mouse_invisible` 27 (16 true); alpha threshold 3 × 128; resize-bar borders 30 / 39 / 3 / 3; `activatable` 370 (349 true) + 1 state; `blocks_clicks` 292 (247) + 1; `auto_tooltip_truncated_text` 271 (246); `KeepObjectWhenHidden` 35; `save_loc` 194 (192) + 1; aspect ratio 1 ≈ 1.2715; `tooltip_layout` 2,091 (2,076 / 0 in 3) + 41 states; `InputMapID` and `disallow_drag_in` on none; Dragbar 135, ResizeBar 52: all match. (2) UB-I1: 2,084 blocks set both properties, with 7 distinct pairs. 5 resolve to a Text descendant through `tooltip_text_ID`, (0, 0) occurs in 3 blocks, and one pair in 2 blocks names a missing element: matches. (3) UB31 / UB-I2: of the 1,616 0x78 sets, only one holds `UIScaleDefinition_Array`. It decodes to the end with 56 entries, all Min 0.4, Max 4.0 and 73 stops. Groups 60 and 61 carry the two preference keys and the other 54 carry a property; none has both or neither. `UpdateOnRelease` is 1 in 3 entries: matches. (4) UB35 / UB-I3: the REC §1.5 hash of the three identifiers gives 0x0AA521D0, 0x00058AD4 and 0x094418B0. `STRINGTABLE` maps `UI_Helper` with key 0x0F to a table with exactly one entry, with that token, both argument IDs and three pieces of lengths 0, 2 and 0: matches. (5) UB17 / UB31, owner's `UserPreferences.ini`: `TooltipEnable=True`, `TooltipDelay=0.25`, and all three `UIScale_*` = 1.00: matches. (6) UB20 tooltip layout: four 30 × 30 top-level elements at z-level −1500 with edges 0. Each Text child is at (2, 2), 26 × 26, with edges 5, left and right margins 6, top margin 0, `max_width` 508 and `max_height` 508 (808 in one variant): matches, except for the second child (Blocking 1). **Revision 1 by the spec writer, 2026-10-06**: every item above is addressed; see the change log. **Re-review: Claude review session (spec-writer agent), 2026-10-06: passed, Accepted.** All three blocking items are fixed. Blocking 1: §6.3, §6.4 step 2, UB19 and UB20 now describe both children. A fresh read-only check of all 4 variants confirms them: the Field is at (0, 0), 512 × 512, z-level 1, edges 5, 5, 5, 5; the Text element is at (2, 2), 26 × 26, z-level 0, edges 5, with margins 6 / 6 / 0 and `max_width` 508. Blocking 2: §8.2 now says the step 4.3 limits are not final. UB-I9 is restricted to the case with no aspect ratio and no cap change. I checked that this restricted form follows from steps 4.1–4.3, including the case where a minimum is above the maximum. The extended §13.5 recomputes to 350 × 233. Blocking 3: the §8.1 step 3 clamp order is stated. The rewritten UB-I8 follows from it on both branches, and §13.10 recomputes to x = −200 and y = 200. Minor items: (a) §6.2 steps 2–5 and UB-I13 are consistent. (b) §5.2 step 1 and UB-I14 are consistent, and §13.9 recomputes stack by stack. (c) UB40 is Reported with sources. (d) the three OPEN and §16 phrasings are now plain unknowns. The §5.3 checklist passes on every item. Further spot checks against the install (read-only, 2026-10-06), all matching: `min_width` / `min_height` / `max_width` / `max_height` 230 / 153 / 218 / 124; `Resizebar_Target_Element` 3; `IgnoreResizeHeightCap` 1 and `IgnoreResizeWidthCap` 0; `activate_on_show` 136 (134) + 1 state; `focus_on_show` 11; `UICoreDragbar_Ancestor_UIElementID` 4 (one label); `tooltips_on` 1,827 (1,787) + 42; `container` 200 (196); `save_size` 41 (34); `editable` / `selectable` 128 / 138; `hide` 769 (740) + 165 states (42 / 123); `tooltip_text_ID` 173 (98 zero); `UICore_Panel_tab_element` 233 occurrences. Non-blocking notes for a later revision: §14 lists UB-I13 and UB-I14 between UB-I9 and UB-I10. §6.1 says a change to `TooltipDelay` "runs the timer check again", but by §6.2 step 2 only a `TooltipEnable` change clears the hover-handled flag, so a delay change while the flag is set has no visible effect. Saying so would remove the ambiguity. |
| **Implements into** | `crates/ui` (`tessera-ui`: pointer routing, hover, capture, activation and z-order, keyboard focus and key routing, tooltips, drag and drop, move and resize, auto-fade, UI scale, text truncation), `crates/input` (activation timing of mouse commands, §3.3), `crates/settings` (tooltip and scale preferences, §6.1, §9.1), `apps/client` (native panels, M5) |
| **Companion specs** | [`ui-layout.md`](ui-layout.md) (layout records, element tree, run-time layout §8, sibling order and hit-test order §9.3, states §9.4, media §9.5, button state choice §9.7; called **UL** below), [`keymap.md`](keymap.md) §12 (commands, input maps, activations; **KM**), [`fonts.md`](fonts.md) §7–§9 (text layout and drawing; **FO**), [`string-table.md`](string-table.md) §4.7, §7.8, §9 (markup, StringInfo; **ST**), [`skin-definition.md`](skin-definition.md) §8 (panel overrides, scaler origin; **SK**), [`lua-plugin-api.md`](lua-plugin-api.md) §9, §13 (plugin controls; **LPA**), [`install-layout.md`](install-layout.md) §8 (settings folder, `UserPreferences.ini`; **IL**), [`property-system.md`](property-system.md) (property sets; **PS**), [`records.md`](records.md) §1.5 (name hash), §7–§8 (enum mappers, DID maps; **REC**) |

> **Naming note (interoperability).** Property names (`UICore_…`, `UICoreDragbar_…`,
> `UIComponent_UIScaler_…`, `UIScaleDefinition_…`, `GOP_UIScale_…`), enum labels (UI element
> messages such as `Element_mouse_press`, state labels such as `normal_activated`, element
> labels such as `DialogConfirmationYesButton`, scaling-group labels), input-command names
> (`SelectLeft`, `FocusNext`, …), input-map names (`DialogBoxes`, `EditControls`, …),
> preference keys (`Misc.TooltipDelay`, `UI.UIScale_Global`, …), the DID-map label `UI_Helper`,
> the folder names `ui/layouts`, the extension `.layout`, the `%r` token, and the names
> `ID_Truncated_Tooltip`, `TEXT` and `TOOLTIP` (whose REC §1.5 name hashes key a string-table
> entry and its arguments, §10.2) are identifiers that the user's
> data, preference file and key map use. They appear here only because a compatible client
> cannot work without them; they are read from the install or the user's files at run time
> wherever possible. The interoperability rationale is the *Google v. Oracle* row of
> `docs/LEGAL.md` §4.1 and `docs/ARCHITECTURE.md` §15/§18; `docs/LEGAL.md` §10 is cited for
> the naming rule only. No client message text, game text or third-party prose is reproduced.

---

<a id="s0"></a>
## 0. Purpose

`ui-layout.md` describes what a native panel **is**: records, element trees, states, media and
the layout arithmetic. This spec describes what the original client's UI **does** at run time
with those trees, so that Tessera's `tessera-ui` behaves the same way:

- how input reaches the UI: the input layer's commands, mouse buttons, click and double-click
  timing (§3);
- pointer routing: hit testing, hover, mouse capture, clicks, the wheel, the cursor (§4);
- activation, z-order among panels, keyboard focus, key routing, text-entry keys, dialog keys
  and keyboard navigation (§5);
- tooltips: enabling, delay, content, panel, size, placement and hiding (§6);
- drag and drop of items (§7);
- the window manager: moving, resizing, clamping, saved positions and the files involved (§8);
- UI scale and resolution changes (§9);
- text in the UI: truncation with a suffix and the automatic tooltip of truncated text (§10);
- visibility transitions (auto-fade) (§11) and the UI message vocabulary (§12).

Behaviour that individual game panels implement in game code (what the bag window does with a
dropped item, which panel opens on which key) is out of scope; this spec stops where game code
takes over and says so.

All counts are observations of one install on one date; implementations read data at run time
and tests assert the invariants (§14), not the counts.

---

<a id="s1"></a>
## 1. Provenance

| Tag | Source |
|---|---|
| OBS | The owner's Steam install, read in place and read-only on 2026-10-06 with throwaway probes in the dirty-room scratch area: all 302 UI layout records (prefix 0x22) decoded with the grammar of UL §4 (property census, tooltip layout structure); all 1,616 stand-alone property sets (prefix 0x78) of `client_gamelogic.dat` searched for UI-scale definitions (one holds them) and decoded; the named-string-table DID map (REC §8, label `STRINGTABLE`) and the table it names for key 0x0F; the enum mappers `UIElementMessage` (base mapper 0x230001AC, 77 labels), `UIStateID` (base 0x2300017B), `UIElementID` (base 0x2300013B), `UIElementType` (base 0x230001BC), the ObjectMode and ClampGameViewEdge mappers; the owner's settings folder in the Proton prefix (`UserPreferences.ini`, `lotro.keymap`, `PluginData/`; no `ui/` folder exists). |
| BIN | Method M4 under [decision 0011](../decisions/0011-standing-m4-authorisation.md). Client `x64/lotroclient64.exe`, file version 4906.0071.1794.4035 (`lotro_live`, SHA-256 prefix d22363eb9aff0fb5), disassembled with capstone and pefile in the dirty room on 2026-10-06, function by function (address ranges only). **Questions:** (1) how input commands reach UI elements, which commands are mouse buttons, and how a release, click, double click, tap and wheel step are delivered; (2) the hit test, the hover rule, mouse capture and the cursor choice; (3) activation, z-order among panels, keyboard focus, key routing and the text-entry and dialog keys; (4) the tooltip timer, content, panel construction, sizing and placement; (5) drag-and-drop start, icon, targets and drop; (6) moving and resizing panels with drag bars and resize bars, clamping, and what happens to saved positions; (7) how UI scale and screen size changes are applied; (8) text truncation with a suffix and the truncated-text tooltip; (9) auto-fade; (10) the click timing of the input layer. Numeric constants (thresholds, factors) are functional facts under [decision 0012](../decisions/0012-numeric-constants.md). **Why M1–M3 were not enough:** the data gives property values and labels but no behaviour; no published source describes the engine's UI runtime (the plugin API documentation describes only the plugin surface, LPA); the owner's client cannot be observed running in this session (M3), and observation could not give thresholds, ordering or tie-break rules anyway. Only offline UI, input-command and text code was read; nothing about network, login, keys, launcher, patcher or anti-tamper. No address, listing or symbol appears in this spec. |
| UL | [`ui-layout.md`](ui-layout.md) (Accepted): element tree, sibling list order, hit-test order, states, run-time layout, Button state choice. Facts taken from there keep its marks. |
| KM | [`keymap.md`](keymap.md) §12: command names and IDs (InputActions mapper chain), input maps, activation words, the default bindings. |
| FO, ST, SK, LPA, IL, PS, REC | The companion specs named in the header. |

Marks: **Confirmed** = observed in the install (all records of the kind) or a BIN fact that the
data agrees with; **BIN** = rests on the disassembly only; **Probable** = supported by BIN or
data but with a step not traced; **Reported** = third-party only; **OPEN** = unknown, with a
callout in the section.

| # | Fact | Sources | Mark |
|---|---|---|---|
| UB1 | The UI receives input as **commands** from the input layer (KM §12), each with a press or a release; nine engine commands form the **mouse-button set** (§3.1); other commands are keys (§5.3) | BIN | BIN |
| UB2 | Activation timing of the input layer: tap, double click and "nearby" thresholds come from the operating system's double-click time and double-click rectangle (§3.3) | BIN | BIN |
| UB3 | Point-in-element test: inclusive box, hit mask, alpha threshold with scale correction (§4.1) | BIN; OBS (3 elements set the alpha threshold, all 128) | BIN |
| UB4 | Hit-test walk: children front to back, depth first; an element catches the pointer only when it is **mouse-visible** or **blocks clicks**; hidden, fading and drag-icon elements are skipped (§4.2) | BIN; UL §9.3 | BIN |
| UB5 | Mouse-visible rule of the base element and the widgets traced (§4.3) | BIN; OBS (property census §15) | BIN; per-widget rules OPEN |
| UB6 | Hovered element, enter/leave messages and the cursor choice (§4.4, §4.8) | BIN | BIN |
| UB7 | Mouse capture: begins on a press over a mouse-visible element, ends when every pressed button command is released; only the captured element can be hovered (§4.5) | BIN | BIN |
| UB8 | Press, release, click, double-click and context-menu messages (§4.6) | BIN | BIN |
| UB9 | Wheel delivery (§4.7) | BIN; KM (default bindings) | BIN |
| UB10 | In the input layer's pointer-lock mode, and after the pointer left the window, the UI performs no hit testing and hovers nothing (§4.9) | BIN | BIN |
| UB11 | Activation of root panels, the activation list, bring-to-front within the z-level group, state `normal_activated` (§5.1) | BIN; UL §9.3 | BIN |
| UB12 | Keyboard focus, input-map stacking with priorities 0 (active panel) and 50 (focused element), the ancestor chain pushed one priority lower per level, the stack order, duplicate and removal rules, the active panel's push when focus is cleared, state `normal_focussed` (§5.2) | BIN (set-focus routine, the element's push and pop, the input layer's map-stack insert and remove; revision 1) | BIN |
| UB13 | Key routing and the key messages; unconsumed keys go to game code (§5.3) | BIN | BIN |
| UB14 | Text-entry command table (§5.4) | BIN; KM (EditControls, CopyAndPasteControls bindings) | BIN |
| UB15 | Confirmation-dialog keys (§5.5) | BIN; OBS (element labels) | BIN |
| UB16 | Keyboard navigation bindings (§5.6) | KM (default bindings, OBS) | Confirmed (bindings); handler OPEN |
| UB17 | Tooltip preferences `Misc.TooltipEnable` (default true) and `Misc.TooltipDelay` (default 0.25 s); per-element `UICore_Element_HoverDelay` (§6.1) | BIN; OBS (owner file: True, 0.25) | Confirmed |
| UB18 | Tooltip timer: restarts on every pointer move; not while a button is captured; the hover-handled flag, the handler's result, repetition every frame when the result is false, the hover-ended notification (§6.2) | BIN (timer, base hover handler, set-hovered routine, the two reset routines; revision 1) | BIN |
| UB19 | Tooltip text sources, tooltip panel from `tooltip_layout` + `tooltip_ID`, text element from `tooltip_text_ID`; each variant of the default tooltip layout has a background Field and a Text element (§6.3) | BIN; OBS (census, tooltip layout structure, all 4 variants; revision 1 re-checked 2026-10-06) | Confirmed |
| UB20 | Tooltip size = measured text + panel chrome; wrap width = `max_width` minus margins; the Text element and the background Field follow the panel by their edges; scaled by the panel's scale (§6.4) | BIN; OBS (default tooltip panel 30×30 with a 512×512 Field at z-level 1 and a 26×26 Text child at z-level 0, both edges 5; max width 508, margins 6) | Confirmed |
| UB21 | Tooltip placement: pointer + cursor image size, clamped into the screen (§6.5) | BIN | BIN |
| UB22 | Tooltip hiding; no timeout in this build (§6.6) | BIN | BIN |
| UB23 | Drag start: left button, distance ≥ 4 px (squared distance ≥ 16), nearest `dragable` ancestor-or-self (§7.1) | BIN; OBS (10 elements set `dragable`) | BIN |
| UB24 | Drag icon: provided by the source or created by its container as a copy of the source; re-parented to the screen root, excluded from hit tests, sized by `AllowStretchOnDrag` and scale, keeps the grab offset (§7.1, §7.2) | BIN | BIN |
| UB25 | Drop targets: `container` and not `disallow_drag_in`, plus the element's accept check; drag-over feedback states (§7.3) | BIN; OBS | BIN |
| UB26 | Drop, cancel, `disallow_drag_out`, `DragAcceptedState`, messages; keys swallowed during a drag (§7.4) | BIN | BIN |
| UB27 | Moving: drag bar moves the ancestor named by `UICoreDragbar_Ancestor_UIElementID` (else its parent); the moved element is clamped into its parent, near side first and far side second on each axis (§8.1) | BIN (move step; clamp order re-traced in revision 1); OBS (135 Dragbar elements, 4 name an ancestor) | BIN |
| UB28 | Resizing: resize-bar border flags → handle 1–8; size limits, aspect ratio, screen caps (§8.2) | BIN; OBS (52 ResizeBar elements; border census) | BIN |
| UB29 | Saved positions: `save_loc`, `save_size`, `save_visibility` register an element; the end of a move or resize notifies game code; applying a saved entry follows UL §9.9; the storage is not local (§8.4) | BIN; UL §9.9; OBS (no layout files in the settings folder) | BIN; storage OPEN |
| UB30 | `ui/layouts` file-name resolution and the `%r` token (§8.5) | BIN | BIN; file content OPEN |
| UB31 | UI-scale preferences, the `UIScaleDefinition_Array` (56 entries, range 0.4–4.0 in 73 stops) and scaling groups (§9.1) | OBS (one property set); BIN (preference binding) | Confirmed |
| UB32 | Panel scale = group option × global scale; size = cached unscaled size × scale, limited to the screen; scaling origin kept fixed; clamped into the screen (§9.2) | BIN (game UI code) | BIN |
| UB33 | Cursor scale: `UI.UIScale_Cursor` outside (0, 4] resets to 1; cursor drawn at ⌊32 × scale⌋ px (§9.3) | BIN | BIN |
| UB34 | Screen size change: the screen root is resized to the new render size (1024 × 768 when no device) and root panels are laid out again (§9.4) | BIN; UL §8.3–§8.5 | BIN |
| UB35 | Truncation with `UICore_Text_truncate_text_to_fit` and the truncated-text tooltip through the `UI_Helper` table token 0x0AA521D0 (§10) | BIN; OBS (token and its two arguments; property census) | Confirmed (token), BIN (rules) |
| UB36 | Message delivery: element handler, then listeners; messages raised during delivery are queued (§12.1) | BIN | BIN |
| UB37 | Auto-fade on show and hide (§11.1) | BIN; OBS (105 elements set `UICore_Element_AutoFade`) | BIN |
| UB38 | Effects of showing and hiding an element: activation list, `activate_on_show`, `focus_on_show`, visibility broadcast, surface release (§11.2) | BIN | BIN |
| UB39 | Census of the behaviour properties (§15) | OBS | Confirmed |
| UB40 | Platform defaults quoted in §3.3: double-click time 500 ms, double-click rectangle 4 × 4 pixels. These are operating-system values, not values of the client, and Tessera reads the live values from the platform | Microsoft Learn, `SetDoubleClickTime` reference (the system default double-click time is 500 ms); Microsoft Windows 2000 registry reference, entries `DoubleClickWidth` and `DoubleClickHeight` (default 4 pixels each) | Reported |

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Screen root** | The single element that holds every root panel (UL §8.5). Its size is the render size. |
| **Root panel** | A top-level panel attached to the screen root (UL §9.1), and also the tooltip panel and the drag icon. Activation, saved positions and scaling act on root panels. |
| **Displayed** | Visible, with every ancestor up to the screen root visible. |
| **Command** | One input command of the InputActions chain (KM §12.11), delivered with a press or a release; analog sources deliver a value. |
| **Mouse-button set** | The nine engine commands the UI treats as mouse buttons (§3.1). |
| **Mouse-visible** | Per-element flag: the element can become the hovered element and receive pointer input (§4.3). |
| **Blocks clicks** | Per-element flag from `UICore_Element_blocks_clicks`: the element stops the hit test (the pointer does not reach what is behind it) but does not become hovered unless also mouse-visible. |
| **Hit element** | The element returned by the hit test (§4.2). |
| **Hovered element** | The mouse-visible element under the pointer, or the captured element (§4.4). |
| **Captured element** | The element that receives pointer input while a button is held (§4.5). |
| **Pressed set** | Per element: the mouse-button commands pressed on it and not yet released (§4.6). |
| **Active panel** | The root panel that was activated last and is still displayed (§5.1). |
| **Focus element** | The element with keyboard focus (§5.2), typically a text entry. |
| **Input map** | A section of the key map (KM §12.5.4). An element names one with `UICore_Element_InputMapID`; the engine also uses `DialogBoxes`, `EditControls`, `CopyAndPasteControls`, `ScrollableControls(_Mouse)`. |
| **UI message** | An event an element sends to itself and its listeners, identified by a `UIElementMessage` value (§12). |
| **Listener** | Game code, a plugin or a widget registered to receive an element's messages. |

---

<a id="s3"></a>
## 3. How input reaches the UI

<a id="s3-1"></a>
### 3.1 Commands, not keys (BIN, UB1)

The input layer turns devices into **commands** through the active input maps (KM §12.8,
§12.12). Each delivery carries the command ID, whether it is a press or a release, a value
(1.0 for a digital press; the source value for analog sources), and the pointer position in
screen pixels (origin top-left of the window's client area).

The input layer turns devices into commands; the UI manager sorts them into two kinds:

1. **Mouse-button commands.** The manager keeps a set of command IDs it treats as mouse
   buttons. At start it holds the nine engine commands below; game code adds four game
   commands when the game UI starts: `DressingRoom`, `AutoLootAll`, `ItemLinkToChat` and
   `MultiUseItem` (modifier-plus-click commands of the default key map, KM §12.9).

   | Command (KM name) | ID | | Command | ID |
   |---|---|---|---|---|
   | `SelectLeft` | 0x56 | | `SelectDblMid` | 0x93 |
   | `SelectRight` | 0x4C | | `TapLeft` | 0x32 |
   | `SelectMid` | 0xA4 | | `TapRight` | 0x73 |
   | `SelectDblLeft` | 0x33 | | `TapMid` | 0x6E |
   | `SelectDblRight` | 0x2E | | | |

   These are routed by pointer position (§4).
2. **All other commands** are routed to the keyboard target (§5.3). The wheel commands
   `ScrollUp` (0x82), `ScrollDown` (0x10) and `Scroll` (0xAB) are an exception: they are
   routed by pointer position (§4.7).

The IDs are InputActions keys (KM §12.11); implementations take them from the mapper chain
by name.

The UI manager receives every command delivery of type "command" from the input layer.
Commands it does not consume are passed on to game code through the manager's notifications
(§5.3).

<a id="s3-2"></a>
### 3.2 Default bindings that feed the UI (Confirmed, KM)

In the default key map (KM §12.9–§12.10, input map `Main`), `SelectLeft` is bound to mouse
button 0 with activation `Click` (press and release), `SelectDblLeft` and `SelectDblRight` to
buttons 0 and 1 with `MouseDblClick`, `TapLeft` to button 0 with `Tap`, and `SelectRight` to
button 1 with `Tap`. The wheel is bound to `ScrollUp`/`ScrollDown` in
`ScrollableControls_Mouse`. Tessera's importer (KM §12.14) keeps these names.

<a id="s3-3"></a>
### 3.3 Click, tap and double-click timing in the input layer (BIN, UB2)

The activation bits of KM §12.5.7 are computed per control as follows. *T* is the operating
system's double-click time in milliseconds (read once at start); *W* × *H* is the operating
system's double-click rectangle in pixels (read once at start); half(*n*) = *n* ÷ 2 rounded
toward zero.

1. **Down** (0x01) on every press, **Up** (0x02) on every release.
2. **Tap** (0x04): on a release, when at most half(*T*) ms have passed since the press of the
   same control.
3. **DblClickDown** (0x08): on a press, when at most *T* ms have passed since the previous
   press of the same control.
4. **NearbyDown** (0x20): as DblClickDown, and in addition, with (*x0*, *y0*) the previous
   press position and (*x*, *y*) the current pointer position:
   *x0* − half(*W*) ≤ *x* < *x0* + half(*W*) and *y0* − half(*H*) ≤ *y* < *y0* + half(*H*)
   (the operating system's point-in-rectangle rule: left and top inclusive, right and bottom
   exclusive).
5. **DblClickUp** (0x10) and **NearbyUp** (0x40): on the release that follows a press that had
   DblClickDown (respectively NearbyDown).

Tessera takes *T*, *W* and *H* from the platform at run time. For reference, the Windows
defaults are 500 ms and 4 × 4 pixels (UB40, Reported); they are operating-system values and
must not be hard-coded in place of the platform's.

> **OPEN:** whether a third press within *T* of the second press again counts as DblClickDown,
> and whether any condition other than the time limit of step 3 can withhold DblClickDown.

---

<a id="s4"></a>
## 4. Pointer routing

<a id="s4-1"></a>
### 4.1 Point-in-element test (BIN, UB3)

A point (*x*, *y*) in the parent's coordinates is inside an element when all of these hold:

1. **Box:** left ≤ *x* ≤ right and top ≤ *y* ≤ bottom, with the element's inclusive box
   (UL §8.1) in parent coordinates.
2. **Hit mask:** if the element has a hit mask (media kind 7, UL §6.9), the mask must not
   exclude the point, tested at (*x* − left, *y* − top).
3. **Alpha threshold:** if `UICore_Element_mouse_visible_alpha_threshold` is set on the element
   (any value; default threshold 0) and the element has a base image (UL §9.6), the image's
   alpha at the point must pass the threshold. The point is first made element-relative; when
   either of the element's scale factors differs from 1 by more than 0.0002, each coordinate is
   divided by its scale factor and truncated toward zero.

> **OPEN:** the exact pass rules of the hit mask (which mask pixels exclude) and of the alpha
> test (alpha ≥ threshold or > threshold; the tiled-image case of UL §9.6). Census: the
> threshold property is set on 3 elements, all with value 128; no hit-mask media item occurs
> in the data (UL §6.1).

<a id="s4-2"></a>
### 4.2 Hit test (BIN, UB4)

The hit test starts at the screen root with the pointer position and returns the deepest
element that catches the pointer, or nothing.

For an element *E* and a point relative to *E*'s parent:

1. If *E* is not visible, or is in an automatic fade (§11.1), or is the icon of a drag in
   progress (§7.2), *E* catches nothing: return "not caught".
2. Walk *E*'s children in **hit-test order**: from the end of the sibling list to its start
   (lowest z-level first, and among equal z-levels the highest read order first; UL §9.3).
   For each child whose point-in-element test (§4.1) passes, run this procedure on the child
   with the point made relative to *E*. If it returns a caught element, return it at once.
3. Otherwise, if *E* is mouse-visible (§4.3) or blocks clicks, return *E* as caught.
4. Otherwise return "not caught".

So a child that contains the point but catches nothing lets the search continue with the
next sibling behind it, and finally with *E* itself. The point-in-element test of *E* itself
is made by the caller (step 2 of *E*'s parent); the screen root is tested against the screen.

<a id="s4-3"></a>
### 4.3 Mouse-visible (BIN, UB5)

Every element starts **not** mouse-visible. Code can set the flag explicitly; once code has
set it explicitly, it is kept. Otherwise the base element recomputes it whenever a relevant
property changes, and it is true when any of these holds:

- `UICore_Element_context_menu` is true;
- `UICore_Element_tooltips_on` is true and the element has tooltip text (a run-time tooltip
  string set by code, or a non-empty resolved `UICore_Element_tooltip_entry`; §6.3);
- the element accepts drops: `UICore_Element_container` is true and
  `UICore_Element_disallow_drag_in` is not true (§7.3).

Widget classes override the rule (code set explicitly still wins): the drag-bar and
resize-bar widgets are always mouse-visible; many other widget classes are always
mouse-visible, a few never, and some add their own conditions (one engine widget that picks a
value from the pointer position is mouse-visible by the base rule or whenever its state is not
`ghosted`, 0x12).

`UICore_Element_blocks_clicks` sets the "blocks clicks" flag directly (§2).

> **OPEN:** the per-type rules: which engine widget types (Button, ListBox, Scrollbar, Text
> with `UICore_Text_editable` or `UICore_Text_selectable`, Menu, Tree, …) are mouse-visible.
> Also unknown: which code reads `UICore_Element_mouse_invisible` (PID 0xC3, set on 27
> elements, 16 true): no reader was found in the engine's property dispatch. Tessera rule
> until answered: Button, Dragbar, ResizeBar, Scrollbar, ListBox, Tree, Menu, ComboBox,
> ColorPicker, editable or selectable Text and every game-type element are mouse-visible; an
> element with `mouse_invisible` = true is not.

> **OPEN:** the relation to UL §9.2 step 5 and §9.3 (M5 revision of `ui-layout.md`), which
> describe a "mouse-button flag" that the client turns on when game code registers a listener
> for one of the four mouse-button messages of an element ID, plus "a second flag that widget
> code can set". The hit test reads exactly two per-element flags: the mouse-visible flag and the
> blocks-clicks flag of this section. So the UL flags are these two, but which UL flag is
> which, and whether registering a listener sets the mouse-visible flag "explicitly" in the sense
> above, was not established. Tessera rule until answered: registering a game-side listener for a
> mouse-button message makes the element mouse-visible.

<a id="s4-4"></a>
### 4.4 Hover (BIN, UB6)

After every pointer move, button press and button release the manager runs the hit test
(§4.2) and updates the hover state:

1. The **hit element** is remembered (also when it only blocks clicks).
2. The **candidate** is: when an element is captured (§4.5), the captured element if it is the
   hit element, otherwise nothing; when nothing is captured, the hit element if it is
   mouse-visible, otherwise nothing.
3. If the candidate differs from the current hovered element:
   - Every element on the ancestor chain of the old hovered element that is not on the chain
     of the new one gets "pointer left" (message `Element_mouse_over`, value 0); every element
     on the new chain that was not on the old one gets "pointer entered" (`Element_mouse_over`,
     value 1).
   - The old hovered element loses its tooltip (§6.6) and gets `Element_mouse_over_top` with
     value 0; the new one gets `Element_mouse_over_top` with value 1.
   - The cursor is chosen again (§4.8).
4. If an item drag is in progress, the drop target is updated (§7.3).

A Button uses "pointer over it" (*O*) and its pressed set (*P*) for its state choice (UL §9.7).

> **OPEN (Probable):** whether the chain of step 3 includes the hovered element itself and
> where it stops (screen root or root panel). The chain comparison was traced; its bounds were
> not.

<a id="s4-5"></a>
### 4.5 Mouse capture (BIN, UB7)

1. **Begin.** On the press of a mouse-button command, if nothing is captured and an element is
   hovered, that element is **captured** and the command is put in the capture's command set.
   If an element is already captured and its command set is not empty (the capture began with
   a press), the command is added to the set. A capture begun by code (drag icon, §7.2) has an
   empty set, and presses do not add to it.
2. While an element is captured:
   - pointer moves are delivered to it, not to the hovered element (§4.6);
   - no other element can become hovered (§4.4 step 2), so other elements get no enter/leave;
   - no tooltip is shown (§6.2).
3. **End.** On the release of a command that is in the capture's command set, the command is
   removed; when the set becomes empty, capture ends, the hover timer restarts (§6.2) and
   hover is updated. A capture begun by code ends only when code (or the drag end, §7.4) ends
   it.
4. **Switching capture** to another element (drag start, §7.2, and move or resize, §8): every
   command in the old element's pressed set is first released on it (its release handler runs,
   §4.6), so the old element leaves its pressed state.

<a id="s4-6"></a>
### 4.6 Presses, releases, clicks and double clicks (BIN, UB8)

Let *target* be the captured element after step 1 of §4.5 (or the hovered element when no
capture began). Coordinates passed to elements are relative to the element.

**Press** of a mouse-button command *c*:

1. If *c* is `SelectLeft` and no item drag is in progress: any leftover drag state is cleared
   and the press position is recorded (for §7.1).
2. Capture begins (§4.5). If no drag is in progress and *c* is `SelectLeft`, the hovered element
   becomes the **drag candidate** (§7.1).
3. **Activation** (only when nothing was captured before this press): let *R* be the nearest
   root panel at or above the hit element. If *R* is the active panel, nothing changes. Otherwise, if *R* exists and its `UICore_Element_activatable`
   is true, *R* is activated (§5.1); if not (no *R*, or *R* not activatable), the active panel
   is deactivated and no panel is active.
4. If *c* is already in *target*'s pressed set, *target* receives a **release** of *c* (below);
   otherwise *target* receives a **press**: *c* is added to its pressed set and it sends
   `Element_mouse_press` with *c* and the position.

**Release** of a mouse-button command *c*:

1. If *c* ends capture (§4.5 step 3), capture ends first.
2. If *c* is `SelectLeft` and an item drag is in progress, the drop happens (§7.4). The drag
   candidate is cleared.
3. If *c* is in *target*'s pressed set: it is removed, and *target* sends
   `Element_mouse_release` with *c* and the position. Then, if *target* is the hovered element
   at that moment (the pointer is over it; §4.4):
   - the command is mapped to its single-click form: `SelectDblLeft` → `SelectLeft`,
     `SelectDblRight` → `SelectRight`, `SelectDblMid` → `SelectMid`; others unchanged;
   - if *c* was a double-click command and *target* accepts double clicks, it sends
     `Element_mouse_dbl_click` with *c*; otherwise it sends `Element_mouse_click` with the
     single-click form.

**Context menu:** when an element with `UICore_Element_context_menu` = true receives its own
`Element_mouse_press` with `SelectRight`, it sends `Element_open_context_menu` with
`SelectRight` (the context-menu widget, 3 elements with `UICore_ContextMenu_open_delay` 0.5 s,
belongs to the widget layer).

**Pointer moves** go to the captured element, or, when nothing is captured and no item drag
runs, to the hovered element. The base element sends `Element_mouse_move` with the position
only while it has a pressed command or when code has asked it for move messages; a target
that is being moved or resized follows the pointer (§8.1, §8.2).

> **OPEN:** which element types accept double clicks (a per-element flag set by code, not by a
> layout property); what `TapLeft`/`TapRight`/`TapMid` produce (they are in the mouse-button set
> and reach the pressed-set logic like any button command; no element message specific to
> taps, `Element_mouse_tap`, was seen being sent by the engine); commands whose binding the
> input layer classifies as a special kind (an internal class value 2) take a separate
> element entry that was not traced.

<a id="s4-7"></a>
### 4.7 Wheel (BIN, UB9)

`ScrollUp`, `ScrollDown` and `Scroll` are delivered to the captured element, or, when nothing
is captured, to the **hit element** (which may be an element that only blocks clicks). The
base element sends `Element_mouse_scroll` with the command and the position. Scrollable
widgets act on it; the default bindings put the wheel in `ScrollableControls_Mouse` (§3.2).

> **OPEN:** whether an unhandled scroll message reaches the scrollable ancestor (bubbling) or
> scrollable widgets find their content by other means; how far one wheel step scrolls.

<a id="s4-8"></a>
### 4.8 Cursor (BIN, UB6)

The cursor shown is, in this order of preference:

1. the cursor of the captured element, if it has one;
2. the cursor of the hovered element, if it has one;
3. an override cursor set by game code, if any;
4. the default cursor.

An element's cursor is set by a Cursor media item (UL §6.5: image and hot spot) and removed by
one with DID 0. The cursor image is drawn scaled (§9.3).

<a id="s4-9"></a>
### 4.9 Pointer locked or outside the window (BIN, UB10)

1. **Pointer lock.** The input layer has a pointer-lock mode (used for mouse-look): while it is
   on, the reported pointer position is the position frozen when the mode began, and the UI
   does not hit-test: nothing is hovered and button commands reach no element. Hit testing
   resumes with the next input event after the mode ends.
2. **Pointer left the window.** When the operating system reports that the pointer left the
   game window, nothing is hovered (tooltips go away, §6.6) and the UI stops hit-testing until
   an input event arrives whose position lies inside the screen root again.

---

<a id="s5"></a>
## 5. Activation, z-order, keyboard focus and keys

<a id="s5-1"></a>
### 5.1 Activation and z-order (BIN, UB11)

1. **What is activated:** only root panels. Activating any other element activates the
   nearest root panel above it.
2. **When:** a click activates the root panel under the pointer only if its
   `UICore_Element_activatable` is true (§4.6 step 3). Activation requested by code or by
   `activate_on_show` (§11.2) needs only that the panel is displayed and not already active; if
   the panel is not activatable, the request also sets its `UICore_Element_activatable` to
   true.
3. **Activating** panel *P* that is not active:
   - the previous active panel is **deactivated**: it pops its input map, the focus element (if
     any) loses focus (§5.2), and it sends `Element_deactivated`;
   - *P* becomes the active panel and pushes its input map (priority 0, §5.2);
   - *P* moves to the end of the **activation list** (most recent last);
   - *P* is **brought to front** among its siblings (below);
   - *P* sends `Element_activated`; if *P* has a designated focus element, that element gets
     keyboard focus (§5.2).
4. **Bring to front:** within its parent's sibling list (UL §9.3) the element moves to the
   front-most position **among siblings with the same z-level**: it is re-inserted after the
   last sibling whose z-level is greater than or equal to its own. It never moves in front of
   a sibling with a lower z-level. (So z-level groups keep their stacking; activation orders
   panels inside a group.)
5. **States:** an element receiving `Element_activated` while in state 0 or `normal` switches to
   `normal_activated`; on `Element_deactivated` it returns from `normal_activated` to `normal`
   (each switch only if the element defines the state, UL §9.4).
6. **Hiding the active panel** (§11.2) deactivates it and activates the most recently activated
   panel in the activation list that is still displayed.

Census: `UICore_Element_activatable` is set on 370 elements (349 true), `activate_on_show` on
136 (134 true).

<a id="s5-2"></a>
### 5.2 Keyboard focus and input maps (BIN, UB12)

1. There is at most one **focus element**. Giving focus to *F* when *F* is not already the
   focus element:
   - the old focus element (if any) pops its input maps (step 2) and sends `Element_set_focus`
     with value 0;
   - if *F* is an element, *F* pushes its input maps with **priority 50** (step 2) and sends
     `Element_set_focus` with value 1. Giving focus does **not** itself pop the active panel's
     entries; only the old focus element's pop can remove them, when the active panel is that
     element or one of its ancestors (§13.9);
   - if *F* is "nothing" (focus is cleared), the active panel, if any, pushes its input maps
     with priority 0. This restores the entries that the old focus element's pop removed when
     the active panel is that element or one of its ancestors (step 2: a pop removes an
     ancestor's entries at every priority). An entry that is still on the stack is not added a
     second time.
2. **Input maps and the map stack.** An element's own map is `UICore_Element_InputMapID` (an
   InputMap enum value, KM §12.5.4); the property is optional.
   - **Push** with priority *p*: first the element's parent pushes with priority *p* − 1 (and so
     on up to the screen root, one lower per level); then, if the element has its own map, the
     entry (map, element, *p*) is added to the input layer's map stack.
   - **The stack** is ordered by priority, highest first. A new entry is placed before every
     existing entry whose priority is equal or lower, so among equal priorities the newest comes
     first. Adding an entry equal to an existing one in map, element and priority changes
     nothing.
   - **Pop**: first the element's parent pops (and so on up to the screen root); then every entry
     with the element's map and the element is removed, whatever its priority.
   - The input layer consults the stack from the start: a map with a higher priority is consulted
     before a lower one.
3. **States:** on `Element_set_focus` 1, an element in state 0, `normal_activated` or `normal`
   switches to `normal_focussed`; on value 0 it returns from `normal_focussed` to
   `normal_activated` if its root panel is active, else to `normal`.
4. Focus is lost when the focus element is hidden, removed, deactivated with its panel, or when
   its own key handling releases it (§5.4).
5. `UICore_Element_focus_on_show` (11 elements): when the element is shown it takes focus,
   unless the current focus element refuses to give it up (§11.2).

> **OPEN:** which input maps engine widgets use when they do not set `InputMapID` (the property
> is set on no layout element; the text widget's editing commands come from `EditControls` and
> `CopyAndPasteControls`, KM §12.5.4, but where those maps are pushed was not traced); which
> elements a root panel designates as its focus element; the "refuse to give up focus" rule;
> whether an editable Text element takes focus when pressed (an engine "take focus" routine
> exists and is reached from widget code that was not traced). Tessera rule until answered:
> pressing an editable Text element gives it focus.

<a id="s5-3"></a>
### 5.3 Key routing (BIN, UB13)

For a command that is not routed by the pointer (§3.1):

1. The **keyboard target** is the focus element, or, when there is none, the active panel.
2. On a **press** the target's key handler runs; the base element sends `Element_key_press`
   with the command and the delivery's value × 1000, truncated (1000 for a digital press). On
   a **release** the base element sends `Element_key_release` with the command.
3. If the target reports the command as **consumed**, it stops there. Otherwise, or when there
   is no target, the command goes on to game code (for example movement, camera and panel
   toggles).
4. While an item drag whose icon was created by the source's container is in progress
   (§7.1 step 5, §7.4), key commands sent to UI elements are reported as consumed without any
   key message.

A message counts as consumed when the element's own handler or one of its listeners consumes
it (§12.1). Plugin windows receive actions through this path (LPA §9.3, §14.4).

<a id="s5-4"></a>
### 5.4 Text-entry keys (BIN, UB14)

A Text element handles these commands when it is the keyboard target. Text properties
(`UICore_Text_…`) gate them as stated; any other command is not consumed.

| Command | Effect |
|---|---|
| `CursorCharLeft`, `CursorCharRight` | Move the caret one character |
| `CursorWordLeft`, `CursorWordRight` | Move the caret one word |
| `CursorPreviousLine`, `CursorNextLine` | Move the caret one line |
| `CursorPreviousPage`, `CursorNextPage` | Move the caret one page |
| `CursorStartOfLine`, `CursorEndOfLine` | Caret to the line start or end |
| `CursorStartOfDocument`, `CursorEndOfDocument` | Caret to the text start or end |
| `CopyText` | Copy the selection to the clipboard |
| `CutText` | Only when `editable` and `selectable`: copy the selection and delete it |
| `PasteText` | Insert the clipboard text (the widget's paste routine; its own conditions were not traced) |
| `DeleteKey` | Only when `editable`: delete the selection if `selectable` and a selection exists, else the character after the caret |
| `BackspaceKey` | Only when `editable`: delete the selection if `selectable` and a selection exists, else (caret not at the start) the character before the caret |
| `EscapeKey` | When `lose_focus_on_escape` is true: give up focus, consumed; else not consumed (it reaches game code) |
| `AcceptInput` | When `lose_focus_on_acceptinput` is true: give up focus, consumed; else not consumed |

Typed characters are not commands; they reach text entry by a separate path (Probable: the
`Text_character_input` message, value 40, exists for this purpose) and are limited by
`UICore_Text_max_chars`.

Default bindings (KM, `EditControls`): arrows, Page Up/Down, Home/End (with Ctrl for word and
document moves), Return and numeric Enter for `AcceptInput`, Escape, Delete, Backspace;
`CopyAndPasteControls`: Ctrl+C/X/V (with an alternative meta key).

Census: `editable` 128 elements (103 true), `selectable` 138 (126 true),
`lose_focus_on_escape` 19 (all true), `lose_focus_on_acceptinput` 56 (all true), `max_chars`
84.

> **OPEN:** the character path (window character messages to the focus element), caret-movement
> details (word boundaries, the selection model and its mouse gestures, what a "page" is),
> character filtering, IME handling, and the effect of `max_chars` on paste.

<a id="s5-5"></a>
### 5.5 Confirmation-dialog keys (BIN, UB15)

A dialog built by the engine's dialog framework (UL §9.1) that is the active panel, and that
does not set `UICore_Dialog_ignorekeyboardinput`, reacts to its own `Element_key_press`:

- `AcceptKey` or `AcceptInput` → the dialog closes with the result of its "yes" button
  (engine element label `DialogConfirmationYesButton`, 0x0F);
- `EscapeKey` → it closes with the result of its "no" button (`DialogConfirmationNoButton`,
  0x21).

A `Button_click` from a child with either of these element IDs closes it the same way. The
matching key releases are consumed.

> **OPEN:** the result handling of the other engine dialogs (message box with
> `DialogMessageOkButton`, text input with `DialogTextInputDoneButton` and
> `DialogTextInputQuitButton`).

<a id="s5-6"></a>
### 5.6 Keyboard navigation (Confirmed bindings; behaviour OPEN, UB16)

The default key map's `DialogBoxes` input map binds Tab to `FocusNext`, Shift+Tab to
`FocusPrevious`, the arrow keys to `FocusMoveLeft/Right/Up/Down` and Escape to `EscapeKey`
(KM §12.9). Panels that push this map receive these commands as keys (§5.3).

> **OPEN:** which part of the client handles `FocusNext`, `FocusPrevious` and `FocusMove…`: no
> handler was found in the engine UI code that was read, so it probably belongs to a widget
> class or to game code. Unknown: the tab order (read order? geometry?), which elements are
> focusable, and whether `UICore_Panel_tab_element` (233 elements) takes part. Tessera rule
> until answered: `FocusNext`/`FocusPrevious` cycle through the displayed, focusable elements
> of the active panel in read order (UL §5.5), depth first; `FocusMove…` picks the nearest
> focusable element in that direction by box centre.

---

<a id="s6"></a>
## 6. Tooltips

<a id="s6-1"></a>
### 6.1 Preferences and per-element delay (Confirmed, UB17)

| Preference (`UserPreferences.ini`, IL §8.3) | Type | Default | Meaning |
|---|---|---|---|
| `[Misc] TooltipEnable` | bool | true | Hover tooltips are shown at all |
| `[Misc] TooltipDelay` | float, seconds | 0.25 | Time the pointer must rest before a tooltip appears |

The owner's file holds `TooltipEnable=True` and `TooltipDelay=0.25`. Changing
`TooltipEnable` hides any tooltip at once; changing either preference runs the timer check of
§6.2 again at once.

An element's own `UICore_Element_HoverDelay` (float, seconds), when present (found through the
element's effective properties, UL §9.4), replaces `TooltipDelay` for that element. Census:
243 elements set it, 242 to 0.1 and one to 300.

<a id="s6-2"></a>
### 6.2 Timer (BIN, UB18)

1. Every pointer move restarts the hover timer (stores "now"). Ending a capture also restarts
   it.
2. The UI manager keeps one **hover-handled** flag, initially clear. It is cleared when the
   hovered element changes (§4.4), when `TooltipEnable` changes (§6.1) and when game code asks
   for the tooltip to be hidden. Whenever it is cleared while set, the element hovered at that
   moment first gets a **hover-ended** notification (a widget hook; the base element does
   nothing with it). A pointer move does not clear the flag.
3. Each frame, when the flag is clear, nothing is captured, the pointer has not left the window
   (§4.9) and an element is hovered: if now > timer start + delay (§6.1), any tooltip is hidden,
   the hovered element's hover handler runs with the pointer position, and the flag is set to
   the handler's **result** (handled or not).
4. The base hover handler:
   1. When `UICore_Element_tooltips_on` is true and `TooltipEnable` is true, it tries to show
      the element's tooltip (§6.3–§6.5); its result is then "handled", even when no tooltip
      appears (empty text, no `tooltip_ID`, no layout, no Text element). Otherwise its result is
      "not handled".
   2. If game code has marked the element for hover messages, it then sends
      `Element_mouse_hover` with the element-relative position, and its result is instead
      whether that message was consumed (§12.1); the result of step 4.1 is then not used.
5. **Consequences.**
   - After a "handled" result nothing more happens for this hover: the timer does not fire
     again until the flag is cleared (step 2), and a pointer move restarts the timer but neither
     hides the tooltip nor clears the flag.
   - After a "not handled" result the flag stays clear, so step 3 runs again on **every frame**
     while the pointer rests: any tooltip is hidden and the handler (and, for a marked element,
     `Element_mouse_hover`) runs once per frame, until the pointer moves (which restarts the
     delay) or the hovered element changes. This is the case for an unmarked element with
     `tooltips_on` false or with `TooltipEnable` false, and for a marked element whose
     `Element_mouse_hover` nobody consumes. For a marked element that also has `tooltips_on`
     true, its tooltip is then hidden and shown again on every frame.

> **OPEN:** which widget classes replace the base hover handler of step 4, and what the
> hover-ended notification of step 2 does in the widget classes that use it.

<a id="s6-3"></a>
### 6.3 Content and panel (Confirmed, UB19)

1. **Text:** the run-time tooltip string of the element if code has set a non-empty one (for
   example the truncated-text tooltip of §10.2 or a plugin), otherwise the resolved StringInfo
   of `UICore_Element_tooltip_entry` (ST §9). If the text is empty, no tooltip is shown.
2. **Tooltip element ID:** `UICore_Element_tooltip_ID` (an element label). If the element does
   not have it, no tooltip is shown.
3. **Tooltip layout:** `UICore_Element_tooltip_layout` of the element; if absent, the value
   from the global defaults of the property; if still absent, (Probable) the layout the
   element itself was created from. If none, no tooltip is shown.
4. **Panel:** any tooltip showing is hidden; then the tooltip panel is created from (layout,
   tooltip element ID) like a root panel (UL §9.1–§9.2).
5. **Text element:** the panel's `UICore_Element_tooltip_text_ID` names a descendant Text
   element (searched in the panel's subtree); its text is set to the tooltip text. If the
   panel has no such property or the element is missing or not a Text, no tooltip is shown.
   Markup in the text is interpreted only if that Text element enables it (ST §4.7; one of the
   four tooltip variants in this install does).

Census: `tooltips_on` on 1,827 elements (1,787 true) and 42 states; `tooltip_entry` on 1,780
elements and 60 states; `tooltip_ID` on 2,044 elements (2,020 name the same element label;
two blocks name an element that their tooltip layout does not contain, UB-I1);
`tooltip_layout` on 2,091 elements (2,076 name the same layout, which holds four tooltip
variants). Each variant is a 30 × 30 panel (edges 0, 0, 0, 0; z-level −1500) with two
children:

- a **Field** at (0, 0), 512 × 512, z-level 1, edges (5, 5, 5, 5): the background, behind the
  text. It is larger than the panel at design size and is drawn clipped to the panel's box
  (UL §9.3);
- a **Text** element at (2, 2), 26 × 26, z-level 0, edges (5, 5, 5, 5), left and right text
  margins 6, top margin 0, `max_width` 508 and `max_height` 508 or 808, named by the panel's
  `tooltip_text_ID`.

<a id="s6-4"></a>
### 6.4 Size (Confirmed, UB20)

1. The Text element measures its text (FO §7) with a wrap width of its `UICore_Element_max_width`
   (or the screen width when unset) minus its left and right text margins. The measured
   extent is the laid-out text plus the left and right margins (width) and plus the top and
   bottom margins (height).
2. The panel's new size is its current size plus (measured extent − the Text element's current
   size), separately for width and height: the panel keeps its design chrome around the text.
   When the panel is laid out at the new size (step 3), its children follow by their edge
   modes (UL §5.8, §8.4): in the default tooltip layout both the Text element and the
   background Field have edges (5, 5, 5, 5), so each keeps its design distances from the panel's
   four sides and grows or shrinks by the same amount as the panel.
3. The panel is laid out at that size, then drawn scaled by its scale (§9.2): its object mode is
   set to CanvasResize while the size is set, then to Stretch with the size multiplied by the
   scale (each axis truncated toward zero).

> **OPEN:** how `max_height` limits a tall tooltip (it is not used by the measuring step that
> was traced).

<a id="s6-5"></a>
### 6.5 Placement (BIN, UB21)

With the pointer at (*px*, *py*), the cursor image's displayed size *cw* × *ch* (§9.3), the
tooltip's displayed size *w* × *h* and the screen size *SW* × *SH*:

1. *x* = *px* + *cw*; *y* = *py* + *ch*;
2. *x* = max(*x*, 0); *y* = max(*y*, 0);
3. *x* = min(*x*, *SW* − *w*); *y* = min(*y*, *SH* − *h*) (applied after step 2, so a tooltip
   wider or taller than the screen ends up at a negative position);
4. the panel becomes a root panel, is attached to the screen root if it is not already, and
   is moved to (*x*, *y*).

The tooltip does not follow the pointer afterwards. (Its z-level −1500 puts it in front of
ordinary panels, UL §9.3.)

<a id="s6-6"></a>
### 6.6 Hiding (BIN, UB22)

The tooltip panel is hidden and destroyed when the hovered element changes, when another
tooltip is shown, when `TooltipEnable` changes, or when game code hides it. If the panel's
`UICore_Element_BroadcastOnTooltipStop` is true it first sends `Element_tooltip_stopped`.

A show request can carry a display time, but in this build the lifetime check never fires, so
tooltips have **no timeout**.

> **OPEN:** the tooltip-detach feature (input command `TOOLTIP_DETACH`) and the item and
> examination tooltips built by game code; both are game code.

---

<a id="s7"></a>
## 7. Drag and drop of items

<a id="s7-1"></a>
### 7.1 Starting a drag (BIN, UB23)

1. The **drag candidate** is the hovered element at the press of `SelectLeft` (§4.6).
2. On each pointer move while a candidate exists and no drag is running: the left button must
   still be held, and the recorded press position must lie inside the screen. Let *dx*, *dy*
   be the pointer's offset from the press position. Nothing happens while *dx*² + *dy*² < 16
   (the pointer has moved less than 4 pixels).
3. Otherwise a drag attempt starts: any leftover drag state is cleared and the
   drag-in-progress state is set.
   The **source** is the candidate if it is `UICore_Element_dragable`, else the nearest
   ancestor that is. If none is, the candidate sends `Element_not_dragable` with the press
   position and nothing more happens (the drag state stays until the button is released).
4. The source fills a **drag description**: the payload (game code) and, optionally, an
   **icon** element. The base element provides nothing; game widgets (inventory slots,
   quick slots, plugin controls) may.
5. If the source provided no icon, the source's **container** (its parent) is asked:
   - a container with `UICore_Element_disallow_drag_out` = true refuses, and nothing more
     happens;
   - otherwise the base container creates the icon as a **new element built from the source's
     descriptor** (a visual copy of the source) and starts swallowing key commands until the
     drag ends (§7.4 step 4).
6. If there is still no icon, or the icon is the source element itself, nothing more happens.

Census: `dragable` on 10 elements, `disallow_drag_out` on 2 (both false).

<a id="s7-2"></a>
### 7.2 The drag icon (BIN, UB24)

With the icon from §7.1:

1. The icon is marked as a drag icon (excluded from hit tests, §4.2), given its own drawing
   surface, made not mouse-visible, made a root panel and re-parented to the screen root.
2. **Size:** when the icon was created by the container (§7.1 step 5), the source has
   `UICore_Element_AllowStretchOnDrag` (default true) and the source's effective object mode
   is Stretch (its own `UICore_Element_ObjectMode`, default CanvasResize, when it has its own
   surface; else derived from its root panel), the icon is set to Stretch and given the
   source's size. Otherwise the icon keeps its own size multiplied by the scale of the
   source's root panel (each axis truncated toward zero).
3. **Position:** the icon is placed so that the point grabbed on the source stays under the
   pointer: icon position = pointer − (press position − source's screen position). It then
   follows the pointer with the same offset on every move.
4. The icon takes the z-level of the screen root's front-most child and is inserted in front,
   is captured (§4.5), the hover is updated (§4.4), and `Element_drag_global_begin` (value 1)
   is sent for the icon.

<a id="s7-3"></a>
### 7.3 Drop targets and feedback (BIN, UB25)

On every hover update during a drag:

1. The **drop target** is found from the hit element upward: an element accepts if
   `UICore_Element_container` is true, `UICore_Element_disallow_drag_in` is not true, and its
   accept check (set by game code; default accept) agrees for the payload; otherwise its
   parent is asked, up to the screen root.
2. When the drop target changes, the old one gets `Element_drag_over` with value 0 and the new
   one with value 1.
3. **Feedback states:** an element with `UICore_Element_container` and
   `UICore_Element_drag_rollover` both true reacts to `Element_drag_over` 1 by remembering its
   current state and switching to `drag_rollover_accept` (if it accepts the payload and is not
   `disallow_drag_in`) or `drag_rollover_reject`; on value 0 it returns to the remembered
   state.

Census: `container` on 200 elements (196 true); `drag_rollover` on 8 (all true);
`disallow_drag_in` on none.

<a id="s7-4"></a>
### 7.4 Drop, end and cancel (BIN, UB26)

1. **Drop:** on the release of `SelectLeft` during a drag, capture is released, the hover is
   set to the hit element, and the drop target (if any) handles the drop: it switches to its
   `UICore_Element_DragAcceptedState` if set, else back to its remembered state, sends
   `Element_stop_drag_item` with the description, and reports "accepted" when it is still the
   drop target found for the icon (§7.3 step 1) and the description is marked acceptable. With
   no drop target the drop is not accepted.
2. **Source end:** the source's container sends `Element_drag_item_has_been_removed` when the
   drop was accepted, else `Element_stop_drag_item`, and stops swallowing keys.
3. **Clean-up:** an icon created for the drag is destroyed, capture is released, the hover
   timer restarts, `Element_drag_global_end` is sent, and the drag state is cleared.
4. **Keys during a drag:** from the creation of the icon by the container (§7.1 step 5) until
   the source end, key commands sent to UI elements are reported consumed without key
   messages (§5.3 step 4).

> **OPEN:** whether Escape or a right click cancels a drag; the payload types and the
> acceptance rules of game widgets (the per-element accept check of §7.3 step 1 is set by
> game code).

---

<a id="s8"></a>
## 8. Window manager

<a id="s8-1"></a>
### 8.1 Moving panels (BIN, UB27)

Panels are moved by **drag-bar** elements (engine type Dragbar; UL §3.3).

1. **Target:** the ancestor named by the drag bar's `UICoreDragbar_Ancestor_UIElementID`
   (searched up the parent chain), else the drag bar's parent.
2. **Begin:** when the drag bar receives `Element_mouse_press` with `SelectLeft` and the target
   is neither moving nor resizing: the target records the pointer position and its own screen
   position and enters **moving**; the drag bar switches to state `normal_pressed`. (The press
   also captured the drag bar, §4.5.)
3. **Move:** on each `Element_mouse_move` of the drag bar while the target is moving, the
   target's new screen position (*x*, *y*) is its recorded position plus the pointer's offset
   from the recorded pointer position. It is then clamped into its parent, in screen
   coordinates with inclusive boxes, with *w* × *h* the target's size and the parent's box
   (*pl*, *pt*, *pr*, *pb*), in this order:
   1. *x* = max(*x*, *pl*) (left edge not left of the parent's);
   2. *x* = min(*x*, *pr* − *w* + 1) (right edge not right of the parent's);
   3. *y* = max(*y*, *pt*);
   4. *y* = min(*y*, *pb* − *h* + 1).

   On each axis the second clamp wins. So a target that fits inside its parent stays inside it,
   and a target wider (taller) than its parent ends with its right (bottom) edge on the
   parent's right (bottom) edge and its left (top) edge outside the parent. The target keeps
   its size. For a root panel the parent is the screen root, so a panel no larger than the
   screen cannot be dragged off screen.
4. **End:** on `Element_mouse_release` with `SelectLeft` the drag bar releases the target, returns
   to state `normal`, and the target leaves moving; if the target has `UICore_Element_save_loc`,
   game code is notified (§8.4).

`UICore_Element_constrain_move_to_parent` is read when a move step runs but its value does not
change the result in this build (the clamp of step 3 is always applied).

Census: 135 Dragbar elements; 4 elements name an ancestor (all the same label).

<a id="s8-2"></a>
### 8.2 Resizing panels (BIN, UB28)

Panels are resized by **resize-bar** elements (engine type ResizeBar).

1. **Handle:** from the resize bar's border flags `UICore_Resizebar_border_right` (R),
   `_left` (L), `_top` (T) and `_bottom` (B):

   | Flags | Handle | Meaning |
   |---|---|---|
   | R and T | 3 | top-right corner |
   | R and B (not T) | 5 | bottom-right corner |
   | R only | 4 | right edge |
   | L and T (not R) | 1 | top-left corner |
   | L and B (not R, T) | 7 | bottom-left corner |
   | L only | 8 | left edge |
   | T only (not R, L) | 2 | top edge |
   | B only (not R, L, T) | 6 | bottom edge |
   | none | — | no resize |

2. **Target:** the element named by `UICore_Resizebar_Target_Element`, else the resize bar's
   parent.
3. **Begin** (`Element_mouse_press` with `SelectLeft`, target not moving or resizing): the
   target records the pointer position, its own width *W0* and height *H0* and the handle, and
   enters **resizing**; the resize bar switches to `normal_pressed`.
4. **Step** (each pointer move while resizing), with *dx*, *dy* the pointer's offset from the
   recorded position:
   1. Raw size: handles 1, 7, 8: *W* = *W0* − *dx*; handles 3, 4, 5: *W* = *W0* + *dx*;
      handles 1, 2, 3: *H* = *H0* − *dy*; handles 5, 6, 7: *H* = *H0* + *dy*; the other axis
      keeps its recorded value.
   2. The **driving axis** is width for handles 1, 4, 5, 8 and height for 2, 3, 6, 7.
   3. If `UICore_Element_max_width` is set and *W* exceeds it, *W* = max_width and width
      drives. If `UICore_Element_min_width` is set and *W* is below it, *W* = min_width and
      width drives. Then the same for height with `max_height` and `min_height` (height then
      drives). Minimums are applied after maximums, so they win.
   4. If `UICore_Element_resize_aspect_ratio` is set and not 0: when width drives,
      *H* = trunc(*W* ÷ ratio); else *W* = trunc(*H* × ratio) (32-bit floats).
   5. The anchored side stays: handles 1, 7, 8 keep the right edge (left = right − *W* + 1);
      handles 3, 4, 5 keep the left edge (right = left + *W* − 1); handles 1, 2, 3 keep the
      bottom (top = bottom − *H* + 1); handles 5, 6, 7 keep the top (bottom = top + *H* − 1).
   6. **Screen caps** (in the target's parent coordinates, compared with the screen size
      *SW* × *SH*): left = max(left, 0); unless `UICore_Element_IgnoreResizeWidthCap`,
      right = min(right, *SW* − 1); top = max(top, 0); unless
      `UICore_Element_IgnoreResizeHeightCap`, bottom = min(bottom, *SH* − 1).
   7. The target is moved to (left, top) and sized (right − left + 1, bottom − top + 1); its
      children are laid out again (UL §8.4).

   The size limits of step 4.3 are **not final**: the aspect-ratio step 4.4 and the screen caps
   of step 4.6 run after them, and either can give a size below a minimum or above a maximum
   (§13.5). Only when no aspect ratio is set and step 4.6 changes nothing does the final size
   respect the limits (with a minimum winning over a smaller maximum).
5. **End** (`Element_mouse_release` with `SelectLeft`): the resize bar returns to `normal`; the
   target leaves resizing; if it has `UICore_Element_save_size`, game code is notified (§8.4).

With `UICore_Element_resize_line` set together with another code-set flag, an outline element
is shown during the resize instead of resizing live.

Census: 52 ResizeBar elements; border flags right 30, bottom 39, left 3, top 3 (all true);
`Resizebar_Target_Element` 3; `min_width` 230 elements, `min_height` 153, `max_width` 218,
`max_height` 124, `resize_aspect_ratio` 1 (≈ 1.2715), `IgnoreResizeHeightCap` 1,
`IgnoreResizeWidthCap` none.

> **OPEN:** the outline mode of `UICore_Element_resize_line` (no layout element sets it) and
> whether the cap of step 6 uses the screen size also for targets that are not root panels
> (it does in the code traced; such targets are rare). `UICore_Element_force_match_parent_scale`
> (3 elements) and `UICore_Element_clip_to_parent` (1) were not traced.

<a id="s8-3"></a>
### 8.3 Keeping panels on screen (BIN)

Three mechanisms keep panels visible: the move clamp of §8.1 step 3, the resize caps of §8.2
step 4.6, and the keep-inside-parent flag of UL §5.4/§8.4 step 5. When the screen size changes
(§9.4) root panels are laid out again from their design size (UL §8.3), which re-applies their
edge modes; edge mode 0 keeps a panel's current position (UL §8.4 step 4), so a panel moved by
the user can end up partly outside a smaller screen.

> **OPEN:** whether game code clamps saved panel positions into the screen after a resolution
> change, and `UICore_Element_ClampGameViewEdgeToObject` (enum None, Bottom, Top, Right, Left;
> set on no layout element): it is handed to the element when set, and its effect (probably
> shrinking the 3D view so that a docked panel does not cover it) was not traced.

<a id="s8-4"></a>
### 8.4 Saved positions, sizes and visibility (BIN; storage OPEN, UB29)

1. `UICore_Element_save_loc`, `UICore_Element_save_size` and `UICore_Element_save_visibility`
   are per-element flags. An element with at least one of them true is registered with the UI
   manager's list of saved elements; when all three become false it is removed.
2. The end of a move of an element with `save_loc` (§8.1 step 4) and the end of a resize of an
   element with `save_size` (§8.2 step 5) raise the UI manager's "layout changed" notification
   to game code.
3. Saved entries are applied to an element when game code asks the UI manager to do so, by the
   steps of UL §9.9 (normalized screen corners converted to pixels; visibility, position and
   size applied as the three flags select; then `Element_saved_location_applied`). (An earlier
   draft of this spec, following an earlier reading of UL §9.2 step 5, said saved data was
   consulted at creation; UL has since identified that table as the message-listener registry.)
4. The engine UI code writes no file for this, and the owner's settings folder holds no
   panel-position file (OBS). Persisting the values is left to game code.

Census: `save_loc` on 194 elements (192 true), `save_size` 41 (34 true), `save_visibility` 1.

> **OPEN:** where game code stores saved panel data (the per-character options are not local
> files on this machine; they may be held by the server, which is gated by decision 0004), and
> when game code applies the store (UL §9.9 OPEN items 1 and 3). Tessera
> stores its own saved panel data in Tessera's settings (a Tessera decision, `crates/settings`),
> keyed by root-panel element label and layout DID.

<a id="s8-5"></a>
### 8.5 UI layout files (`ui/layouts`, BIN, UB30)

The client resolves the name of a **UI layout file** (a function exposed to the client's
internal script layer) as follows:

1. If the name has no drive part, it is placed in the folder `ui/layouts` below the settings
   folder (IL §8.1).
2. If it has no extension, `.layout` is appended.
3. The token `%r` in the result is replaced by the current screen size written as
   *width*`_`*height* in decimal (for example `1920_1080`). A `%` followed by another
   character is left to the expansion rules of the client's generic token expander.

The owner's settings folder has no `ui` folder, so no such file was observed.

> **OPEN:** which commands save and load a `.layout` file. UL §9.9 gives the XML form of the
> saved-location store (`Layout` and `Element` nodes with `ID`, `X`, `Y`, `Width`, `Height`) and
> ties the folder and extension to it; no such file exists on the owner's machine, so the file
> content is not observed, and the identifier forms are open there (UL §9.9 OPEN item 2). The client
> also exports and imports a gameplay-options XML document with sections named `Options`,
> `Aliases`, `ChatColors` and `Layouts`; its file location is unknown. IL §8.2 should list
> `ui/layouts/` once the content is confirmed (§16).

---

<a id="s9"></a>
## 9. UI scale and resolution

<a id="s9-1"></a>
### 9.1 Preferences and scale definitions (Confirmed, UB31)

| Preference (`[UI]`, IL §8.3) | Default | Meaning |
|---|---|---|
| `UIScale_Global` | 1.0 | Multiplies the scale of every scalable panel in game |
| `UIScale_Pregame` | 1.0 | Scale of the character-selection (pre-game) screens |
| `UIScale_Cursor` | 1.0 | Scale of the mouse cursor (§9.3) |

The owner's file holds 1.00 for all three.

The allowed values and the per-panel options come from data: one stand-alone property set of
`client_gamelogic.dat` holds `UIScaleDefinition_Array`, an array of structs with the fields:

| Field | Type | Meaning |
|---|---|---|
| `UIScaleDefinition_Group` | Enum (UIScalingGroupType, LPA Appendix A.7) | The scaling group the entry controls |
| `UIScaleDefinition_Preference` | String | The preference key that stores the value (`UI.UIScale_Global`, `UI.UIScale_Pregame`), or absent |
| `UIScaleDefinition_Property` | PropertyId | The gameplay-option property that stores the value (a `GOP_UIScale_…` property), or absent |
| `UIScaleDefinition_Min`, `UIScaleDefinition_Max` | Float | Range |
| `UIScaleDefinition_NumberStops` | Int | Number of positions of the options slider |
| `UIScaleDefinition_UpdateOnRelease` | Bool | Apply when the slider is released (1) or while it moves (0) |

Observed: 56 entries; every one has Min 0.4, Max 4.0 and 73 stops (a step of 0.05); groups 60
(global) and 61 (pre-game) use preferences, the other 54 use `GOP_UIScale_…` gameplay options;
`UpdateOnRelease` is 1 in 3 entries. Find the set by searching the property sets for
`UIScaleDefinition_Array` at run time; do not hard-code its DID.

<a id="s9-2"></a>
### 9.2 Panel scale (BIN, game UI code, UB32)

A scalable root panel belongs to a scaling group (`UIComponent_UIScaler_Group`) and has a
scaling origin (`UIComponent_UIScaler_Origin_X`, `_Y`, in unscaled panel pixels, default 0;
SK §8.4 sets them from a skin).

1. **Scale** *s* = the group's option value × `UIScale_Global` when the panel takes part in the
   global scale (if the global value is 0 the panel keeps its current scale); otherwise the
   group's option value alone. (Probable: pre-game panels use the pre-game entry, group 61,
   `UIScale_Pregame`, the same way.)
2. **Fit:** with the panel's unscaled size *CW* × *CH* (cached in
   `UIComponent_UIScaler_Cached_Width`/`_Height`) and the screen *SW* × *SH*,
   *s* = min(*s*, *SW* ÷ *CW*, *SH* ÷ *CH*) (32-bit floats), so a panel is never scaled larger
   than the screen.
3. **Size:** width = trunc(*CW* × *s*), height = trunc(*CH* × *s*). The panel's `min_width`,
   `max_width`, `min_height` and `max_height` are scaled by *s* (truncated) for this step and
   bound the new size (minimum, then maximum), then restored.
4. **Origin kept fixed:** with *s0* the previous scale (`UIComponent_UIScaler_Cached_Scale`,
   default 1) and origin (*ox*, *oy*): new left = left + trunc(*ox* × *s0*) − trunc(*ox* × *s*),
   new top = top + trunc(*oy* × *s0*) − trunc(*oy* × *s*).
5. **Clamp:** left = min(left, *SW* − width), then left = max(left, 0); the same for top with
   *SH* − height.
6. The panel gets the new size and position; *s* is stored as the cached scale.

For a root panel in object mode Stretch (`UICore_Element_ObjectMode`; 301 layout elements set
Stretch, UL §5.9) the children are not re-laid out (UL §8.4 step 1): the panel's surface keeps
its unscaled canvas and is drawn stretched onto the scaled box. A panel in another object mode
is simply resized, and its children follow their edge modes (UL §8.4).

> **OPEN (Probable):** the drawing side of step 6 (Stretch surfaces drawn scaled) is inferred
> from the tooltip and drag-icon code, which explicitly lay out in CanvasResize and then switch
> to Stretch at the scaled size (§6.4, §7.2); the compositing code was not traced (UL §8.4
> OPEN on object modes). Also unknown: the plugin-side combination of `Window:SetScale` with
> the group scale (LPA §13.3 OPEN).

<a id="s9-3"></a>
### 9.3 Cursor scale (BIN, UB33)

If `UIScale_Cursor` is ≤ 0 or > 4.0 it is reset to 1.0. The cursor image is redrawn at
trunc(32 × `UIScale_Cursor`) pixels square (Probable: the hot spot of UL §6.5 is scaled with
it; the scale is passed to the routine that installs the cursor). The displayed cursor size
is the offset used for tooltips (§6.5).

<a id="s9-4"></a>
### 9.4 Screen size changes (BIN, UB34)

The UI registers for the render device's size-change notification. On a change:

1. the screen root is resized to the new render size (1024 × 768 when no render device
   exists);
2. because the root's size changed, every root panel is laid out again from its layout's design
   size to the new screen size (UL §8.3, §8.4);
3. game code receives a "screen resized" notification from the UI manager, and the cursor is
   set again.

> **OPEN:** what game code does on the notification (for example re-applying §9.2, which
> depends on the screen size, or clamping saved positions, §8.3).

This answers the OPEN of UL §8.5 (§16).

---

<a id="s10"></a>
## 10. Text in the UI

Text drawing, wrapping, alignment, outline and font choice are specified in FO §7–§9; markup in
ST §4.7–§4.8 and §7.8. This section adds the UI-level rules.

<a id="s10-1"></a>
### 10.1 Truncation with a suffix (BIN, UB35)

`UICore_Text_truncate_text_to_fit` is a **StringInfo**: the text of the suffix drawn where text
is cut (resolved from the string table at run time; an empty or zero StringInfo disables
truncation).

1. When the element's text is laid out and the property resolves to non-empty text, the suffix
   is laid out once with the element's font and its width *sw* is kept.
2. **Overflow test:** the measured text extent including the text margins (§6.4 step 1) is
   compared with the element's box: for single-line text (a text element that does not wrap)
   the width with the element's width, for multi-line text the height with the element's
   height. Not larger: no cut.
3. **Last visible line:** for multi-line text, the last line whose bottom (the sum of the line
   heights from the first line) is within the element's height minus the top and bottom
   margins minus 1; single-line text uses line 0.
4. **Cut position:** on that line, the character index at which the text of the line fits into
   the element's width minus the left and right margins minus *sw*.
5. **Drawing:** text is drawn up to the cut position; the suffix is drawn there instead of the
   rest, and nothing after it.

Census: the property is set on 324 elements and 1 state; 318 values name one of two entries
of one string table, 1 names a third entry, 5 are (0, 0).

> **OPEN:** the fit rule of step 4 (whole glyphs only; whether trailing spaces are kept; how
> kerning at the cut is handled) and how alignment (FO §7.4) treats the shortened line. This
> also answers part of FO Q13 (§16).

<a id="s10-2"></a>
### 10.2 Tooltip of truncated text (Confirmed token; BIN rules)

When a Text element with `UICore_Text_auto_tooltip_truncated_text` = true is cut by §10.1:

1. If the Text element has its own tooltip text (a tooltip string kept by the text widget;
   Probable: its resolved `UICore_Element_tooltip_entry`), its run-time tooltip (§6.3 step 1)
   becomes the rendering of the **truncated-tooltip template** with the argument `TEXT` = the
   element's full text and `TOOLTIP` = that tooltip text; otherwise the run-time tooltip
   becomes the full text.
2. `UICore_Element_tooltips_on` is switched on.

When such an element is not cut: if it has its own tooltip text, that becomes the run-time
tooltip and tooltips are on; otherwise the run-time tooltip is cleared and tooltips are off.

The template is the string-table entry with token **0x0AA521D0** in the named string table
`UI_Helper` (root map label `STRINGTABLE`, REC §8; key 0x0F in this install). Its argument IDs
are **0x00058AD4** (`TEXT`) and **0x094418B0** (`TOOLTIP`); token and argument IDs are the
REC §1.5 name hashes of the identifiers `ID_Truncated_Tooltip`, `TEXT` and `TOOLTIP`. In this
install that table holds exactly this one entry, with three pieces: the first argument, a
two-character separator and the second argument (OBS).

Census: `auto_tooltip_truncated_text` on 271 elements (246 true).

---

<a id="s11"></a>
## 11. Visibility transitions

<a id="s11-1"></a>
### 11.1 Auto-fade (BIN, UB37)

When code shows or hides an element that has `UICore_Element_AutoFade` (float, seconds) and its
own drawing surface:

1. **Fade in** on show, **fade out** on hide. The element sends `Element_auto_fade_start` and
   records the start time and its current opacity (`UICore_Element_ObjectOpacity`, UL §5.9).
2. Each frame: *t* = (now − start) ÷ AutoFade, clamped to [0, 1] (*t* = 1 if |AutoFade| ≤
   0.0002); opacity = start opacity + (target − start opacity) × *t*, target 1 for fade in and
   0 for fade out. The opacity is applied to the element and to each child with its own
   surface.
3. At *t* = 1 the fade ends (`Element_auto_fade_stop`); a fade out then hides the element for
   real. While a fade runs the element is skipped by the hit test (§4.2).

Census: 105 elements set `AutoFade` (101 with 0.1 s, 4 with 0.5 s).

<a id="s11-2"></a>
### 11.2 Showing and hiding (BIN, UB38)

**Hiding** an element: it leaves the activation list; if it has keyboard focus, focus is
released; if it is the active panel it is deactivated and the most recently activated panel
that is still displayed is activated (§5.1); an element with its own drawing surface releases
the surface unless `UICore_Element_KeepObjectWhenHidden` is true (35 elements).

**Showing** an element: an element with its own drawing surface recreates it; if
`UICore_Element_activate_on_show`
is true it is activated, and if `UICore_Element_focus_on_show` is true it takes keyboard focus,
in both cases unless the current focus element refuses to give up focus.

In both cases, an element with `UICore_Element_enable_visibility_broadcast` (1 element) sends
`Element_is_visible_changed` with the new visibility.

`UICore_Element_hide` (769 elements, 165 states) sets the visibility (hide = true → not
visible); a state that sets it shows or hides the element when the state is entered (UL §9.4).

---

<a id="s12"></a>
## 12. UI messages

<a id="s12-1"></a>
### 12.1 Delivery (BIN, UB36)

An element sends a message to itself: its own handler runs first (widgets react, for example
the dialog keys of §5.5), then the element's registered listeners, and the message counts as
consumed if any of them consumed it. A message raised while another message is being delivered
is queued and delivered, in order, after the current one.

<a id="s12-2"></a>
### 12.2 Messages this spec uses

The values are keys of the engine-level `UIElementMessage` mapper (base mapper; labels are read
at run time, UL §3.3).

| Value | Label | Sent when | Argument |
|---|---|---|---|
| 6 | `Element_drag_global_end` | drag clean-up (§7.4) | — |
| 7 | `Element_drag_over` | drop target changes (§7.3) | 1 entered, 0 left |
| 9 | `Element_set_focus` | focus gained or lost (§5.2) | 1 gained, 0 lost |
| 10 | `Element_mouse_over_top` | becomes or stops being the hovered element (§4.4) | 1 / 0 |
| 14 | `Element_mouse_release` | button command released on the element (§4.6) | command, position |
| 15 | `Element_stop_drag_item` | drop on the target; sent by the source's container when the drop was not accepted (§7.4) | description |
| 20 | `Element_drag_item_has_been_removed` | sent by the source's container after an accepted drop (§7.4) | description |
| 25 | `Element_mouse_hover` | hover time reached, for marked elements (§6.2) | position |
| 26 | `Element_mouse_over` | pointer entered or left the element's chain (§4.4) | 1 / 0 |
| 29 | `Element_mouse_dbl_click` | double-click release (§4.6) | command |
| 30 | `Element_mouse_click` | click release (§4.6) | single-click command |
| 32 | `Element_not_dragable` | drag attempt without a dragable source (§7.1) | press position |
| 36 / 42 | `Element_auto_fade_stop` / `_start` | auto-fade (§11.1) | — |
| 37 | `Element_mouse_move` | pointer moves over or while captured by the element, if it has a pressed command or asked for moves (§4.6) | position |
| 40 | `Text_character_input` | typed character (§5.4; Probable) | character |
| 43 | `Element_open_context_menu` | right press with `context_menu` (§4.6) | command |
| 44 | `Element_mouse_scroll` | wheel (§4.7) | command, position |
| 45 | `Element_key_release` | key command released (§5.3) | command |
| 46 | `Element_saved_location_applied` | saved entry applied (§8.4, UL §9.9 step 5) | — |
| 47 | `Element_deactivated` | panel deactivated (§5.1) | — |
| 48 | `Element_drag_global_begin` | drag icon created (§7.2) | 1 |
| 50 | `Element_mouse_press` | button command pressed on the element (§4.6) | command, position |
| 53 | `Button_click` | a Button is clicked (UL §9.7); the dialog framework reacts to it (§5.5) | — |
| 61 | `Element_activated` | panel activated (§5.1) | — |
| 63 | `Element_key_press` | key command pressed (§5.3) | command, value |
| 68 | `Element_tooltip_stopped` | tooltip panel hidden with `BroadcastOnTooltipStop` (§6.6) | — |
| 69 | `Element_is_visible_changed` | visibility changed with broadcast on (§11.2) | visibility |

Positions are relative to the receiving element. The plugin events of LPA §9 (`MouseDown`,
`MouseClick`, `DragStart`, …) are the plugin-side view of these messages.

---

<a id="s13"></a>
## 13. Worked synthetic examples

All values are invented for illustration.

<a id="s13-1"></a>
### 13.1 Click timing (§3.3)

*T* = 500 ms, *W* = *H* = 4. Press at 0 ms at (100, 100): Down. Release at 200 ms: Up and Tap
(200 ≤ 250). Press at 400 ms at (101, 99): Down, DblClickDown (400 ≤ 500) and NearbyDown
(98 ≤ 101 < 102 and 98 ≤ 99 < 102). Release at 700 ms: Up, DblClickUp, NearbyUp (no Tap:
300 > 250). Had the second press been at (101, 102), it would be DblClickDown without
NearbyDown (102 is not < 102).

<a id="s13-2"></a>
### 13.2 Hit test and hover (§4.2–§4.4)

Panel *P* (mouse-visible: no; blocks clicks: yes) with children *A* (z 0, read order 1, a Button,
mouse-visible) and *B* (z −5, read order 2, a Field without tooltip, not mouse-visible), both
containing the pointer. Hit-test order is *B* (lower z) then *A*. *B* catches nothing, so *A* is
tested next and caught: hit element *A*, hovered *A*. If *A* were hidden, the hit element would
be *P* (it blocks clicks) and nothing would be hovered.

<a id="s13-3"></a>
### 13.3 Tooltip placement (§6.4, §6.5)

Screen 1920 × 1080, cursor image 32 × 32, pointer at (1850, 1000). The Text element measures
200 × 14 plus margins 6 + 6 → extent 212 × 14; the 30 × 30 panel with a 26 × 26 text child
becomes 216 × 18; scale 1. *x* = min(max(1882, 0), 1920 − 216) = 1704;
*y* = min(max(1032, 0), 1080 − 18) = 1032.

<a id="s13-4"></a>
### 13.4 Drag threshold (§7.1)

Press at (100, 100). Move to (103, 102): 9 + 4 = 13 < 16, no drag. Move to (102, 104):
4 + 16 = 20 ≥ 16, the drag attempt starts.

<a id="s13-5"></a>
### 13.5 Resize (§8.2)

Target box (100, 50, 399, 249), so *W0* = 300, *H0* = 200; handle 5 (bottom-right); recorded
pointer (390, 240); `min_width` 200, `max_width` 350, aspect ratio 1.5; screen 1920 × 1080.
Pointer at (450, 300): *dx* = *dy* = 60; raw *W* = 360, *H* = 260; width drives (handle 5);
*W* = 350 (max); *H* = trunc(350 ÷ 1.5) = 233; left and top stay: right = 100 + 349 = 449,
bottom = 50 + 232 = 282; caps hold. Result: position (100, 50), size 350 × 233.

The same with `min_height` 240 added: step 4.3 leaves *H* = 260 (not below 240, so height does
not drive), width still drives, and step 4.4 again gives *H* = 233. The result is 350 × 233,
below the minimum height: the aspect ratio wins over the limits (§8.2, UB-I9).

<a id="s13-6"></a>
### 13.6 Activation and z-order (§5.1)

Screen-root children in list order (UL §9.3): *X* (z 0), *Y* (z 0), *Z* (z −10). Clicking *X*
activates it: it is re-inserted after the last sibling with z ≥ 0, giving *Y*, *X*, *Z*. *X* is now
drawn in front of *Y* but still behind *Z*.

<a id="s13-7"></a>
### 13.7 Panel scale (§9.2)

Unscaled 300 × 200 at (100, 100), origin (150, 100), previous scale 1, group option 1.2,
global 1.0, screen 1920 × 1080. *s* = 1.2 (fit limit min(6.4, 5.4) does not apply); size
360 × 240; left = 100 + 150 − 180 = 70, top = 100 + 100 − 120 = 80; clamps hold.

<a id="s13-8"></a>
### 13.8 Truncated-text tooltip (§10.2)

A Text element shows a long name cut with the suffix, has `auto_tooltip_truncated_text` = true
and the explicit tooltip "level 5" (invented): its run-time tooltip is the `UI_Helper`
template rendered with `TEXT` = the full name and `TOOLTIP` = "level 5" (full name, separator,
"level 5"). Without an explicit tooltip it is the full name alone.

<a id="s13-9"></a>
### 13.9 Input maps and focus (§5.2)

Root panel *P* (map *MP*) is a child of the screen root *S* (no map); *P*'s child *F* has map
*MF*. Activating *P* pushes *P* with priority 0: *S* would push with −1 but has no map, so the
stack is (*MP*, *P*, 0). Giving focus to *F* pushes *F* with 50: first *P* with 49, then *S*
with 48 (nothing), then *F*: the stack is (*MF*, *F*, 50), (*MP*, *P*, 49), (*MP*, *P*, 0).
Clearing focus: *F* pops; *P*'s pop removes both of its entries (49 and 0), then *F*'s entry is
removed, leaving the stack empty. The active panel *P* then pushes with priority 0, and the
stack is (*MP*, *P*, 0) again.

<a id="s13-10"></a>
### 13.10 Move clamp of a panel larger than the screen (§8.1)

Screen root 800 × 600, box (0, 0, 799, 599). A root panel 1000 × 400 is dragged so that its
unclamped position is (100, 300). Horizontally: *x* = max(100, 0) = 100, then
*x* = min(100, 799 − 1000 + 1) = −200, so the panel spans −200 to 799 and its right edge is on
the screen's right edge. Vertically: *y* = max(300, 0) = 300, then
*y* = min(300, 599 − 400 + 1) = 200, so the panel spans 200 to 599.

---

<a id="s14"></a>
## 14. Invariants

Marked (OBS) where checkable against the install; the rest are behavioural test cases for
`crates/ui` with synthetic trees.

- **UB-I1** (OBS) Every non-zero (`tooltip_layout`, `tooltip_ID`) pair set together in one
  property block either names a top-level element that has (directly or through its base
  chain, UL §7) a `UICore_Element_tooltip_text_ID` naming a descendant whose resolved type is
  Text, or names an element missing from that layout. In this install: 7 distinct pairs in
  2,084 blocks; 5 resolve fully; (0, 0) in 3 blocks; one pair in 2 blocks names a missing
  element (no tooltip is shown for those two, §6.3).
- **UB-I2** (OBS) `UIScaleDefinition_Array` exists in exactly one property set; each entry has
  exactly one of `UIScaleDefinition_Preference` and `UIScaleDefinition_Property`, and
  Min < Max.
- **UB-I3** (OBS) The `UI_Helper` named table contains token 0x0AA521D0 with argument IDs
  0x00058AD4 and 0x094418B0.
- **UB-I4** A hit test never returns a hidden element, a fading element or the drag icon.
- **UB-I5** While an element is captured, the hovered element is the captured element or
  nothing.
- **UB-I6** A click message is sent only for a command that was pressed on the same element
  and released while the pointer was over it.
- **UB-I7** Bringing to front never changes the relative order of siblings with different
  z-levels.
- **UB-I8** After a move step, on each axis: if the moved element is no wider (no taller) than
  its parent, its screen box lies inside the parent's on that axis; otherwise its right
  (bottom) edge equals the parent's right (bottom) edge (§8.1, §13.10).
- **UB-I9** After a resize step, left ≥ 0 and top ≥ 0, and right ≤ *SW* − 1 and
  bottom ≤ *SH* − 1 unless the corresponding Ignore flag is set. When no aspect ratio is set
  and the screen caps of §8.2 step 4.6 changed nothing, the final width and height also are at
  least the minimums that are set, and at most the maximums that are set and are not smaller
  than the matching minimum. (With an aspect ratio the limits may be broken, §13.5.)
- **UB-I13** While the pointer rests on one element (no pointer move, no hover change, no change
  of `TooltipEnable`), the base hover handler (§6.2 step 4) of an element that game code has not
  marked for hover messages runs at most once if `tooltips_on` and `TooltipEnable` are both true, and once per
  frame after the delay otherwise (§6.2).
- **UB-I14** Giving focus to an element never removes an entry of the active panel from the map
  stack unless the active panel is the old focus element or one of its ancestors; after focus is
  cleared, the active panel's own entry (map, panel, 0) is on the stack whenever the panel has
  a map (§5.2, §13.9).
- **UB-I10** A tooltip's position satisfies *x* ≤ *SW* − *w* and *y* ≤ *SH* − *h*; it is ≥ 0 when
  the tooltip fits on the screen.
- **UB-I11** No drag starts while the pointer is within a squared distance below 16 of the
  press position.
- **UB-I12** There is at most one active panel and at most one focus element; the focus
  element's input map is above the active panel's.

---

<a id="s15"></a>
## 15. Census of behaviour properties (Confirmed, OBS, 2026-10-06)

Counts of property values stored on element blocks and state blocks of all 302 layout records,
before inheritance (UL §7). Values are the property values as stored.

| Property | Elements | States | Values |
|---|---|---|---|
| `UICore_Element_tooltips_on` | 1,827 | 42 | true 1,787, false 40 |
| `UICore_Element_tooltip_entry` | 1,780 | 60 | 46 are (0, 0) |
| `UICore_Element_tooltip_ID` | 2,044 | 41 | one label 2,020 |
| `UICore_Element_tooltip_layout` | 2,091 | 41 | one layout 2,076; 0 in 3 |
| `UICore_Element_tooltip_text_ID` | 173 | 0 | 0 in 98 |
| `UICore_Element_HoverDelay` | 243 | 0 | 0.1 (242), 300 (1) |
| `UICore_Element_activatable` | 370 | 1 | true 349 |
| `UICore_Element_activate_on_show` | 136 | 1 | true 134 |
| `UICore_Element_focus_on_show` | 11 | 0 | all true |
| `UICore_Element_blocks_clicks` | 292 | 1 | true 247 |
| `UICore_Element_mouse_invisible` | 27 | 0 | true 16 |
| `UICore_Element_mouse_visible_alpha_threshold` | 3 | 0 | 128 |
| `UICore_Element_context_menu` | 55 | 0 | true 52 |
| `UICore_Element_dragable` | 10 | 0 | all true |
| `UICore_Element_container` | 200 | 0 | true 196 |
| `UICore_Element_drag_rollover` | 8 | 0 | all true |
| `UICore_Element_disallow_drag_out` | 2 | 0 | both false |
| `UICore_Element_AllowStretchOnDrag` | 2 | 0 | 1 true, 1 false |
| `UICore_Element_save_loc` / `save_size` / `save_visibility` | 194 / 41 / 1 | 1 / 1 / 0 | true 192 / 34 / 1 |
| `UICore_Element_min_width` / `min_height` | 230 / 153 | 0 | |
| `UICore_Element_max_width` / `max_height` | 218 / 124 | 0 | |
| `UICore_Element_resize_aspect_ratio` | 1 | 0 | ≈ 1.2715 |
| `UICore_Element_IgnoreResizeHeightCap` | 1 | 0 | true |
| `UICore_Resizebar_border_right` / `_bottom` / `_left` / `_top` | 30 / 39 / 3 / 3 | 0 | all true |
| `UICore_Resizebar_Target_Element` | 3 | 0 | |
| `UICoreDragbar_Ancestor_UIElementID` | 4 | 0 | one label |
| `UICore_Element_AutoFade` | 105 | 0 | 0.1 (101), 0.5 (4) |
| `UICore_Element_KeepObjectWhenHidden` | 35 | 0 | all true |
| `UICore_Element_enable_visibility_broadcast` | 1 | 0 | true |
| `UICore_Element_hide` | 769 | 165 | elements true 740; states true 42, false 123 |
| `UICore_Text_truncate_text_to_fit` | 324 | 1 | two entries (318), empty 5 |
| `UICore_Text_auto_tooltip_truncated_text` | 271 | 0 | true 246 |
| `UICore_Text_editable` / `selectable` | 128 / 138 | 4 / 4 | true 103 / 126 |
| `UICore_Text_lose_focus_on_escape` / `_acceptinput` | 19 / 56 | 0 | all true |
| `UICore_Text_max_chars` | 84 | 0 | |
| `UICore_Panel_tab_element` | 233 | 0 | |
| Not set on any layout element | | | `UICore_Element_InputMapID`, `_disallow_drag_in`, `_DragAcceptedState`, `_resize_line`, `_IgnoreResizeWidthCap`, `_constrain_move_to_parent`, `_ClampGameViewEdgeToObject`, `_BroadcastOnTooltipStop`, `_LuaOverrideID` |

Engine widget counts (UL §11): Dragbar 135, ResizeBar 52.

---

<a id="s16"></a>
## 16. Corrections and additions to other specs (not edited here)

| Target | Statement there | Finding (this spec) | Basis |
|---|---|---|---|
| `ui-layout.md` §8.5 OPEN (screen size change) and §16 item 12 | Which code reacts to a screen size change is unknown | The UI registers for the render device's size-change notification; the handler resizes the screen root to the new render size (1024 × 768 without a device), which lays out every root panel again (UL §8.4), and notifies game code (§9.4). Modal-dialog containers, being screen-root children with edges (5, 5, 5, 5), follow. | BIN |
| `ui-layout.md` §9.6 OPEN and U30 (`UICore_Element_DrawAfterChildren`) | The code that reads PID 0x3CD, if any, was not found (earlier version of UL) | The client's handling of element property changes does handle PID 0x3CD: it stores the value as a per-element flag and then refreshes the element. So the client honours the property. (The M5 revision of UL, §9.3 and §9.6, has since specified its drawing effect; this row only agrees with it.) | BIN |
| `ui-layout.md` §5.9 (engine properties named only) | Behaviour of the remaining engine-level properties OPEN | Behaviour now specified here for the tooltip, hover, activation, focus, mouse, drag, save, size-limit, resize-bar, drag-bar, auto-fade, visibility and text-truncation properties (§4–§11, census §15). | BIN, OBS |
| `keymap.md` §12.5.7 and §12.19 (timings OPEN) | "The double-click and tap timings and the 'close' distance are OPEN" | Tap: release within half the OS double-click time; DblClick: press within the OS double-click time of the previous press of the same control; Nearby: also inside the half-open rectangle of ± half the OS double-click rectangle around the previous press position (§3.3). | BIN |
| `keymap.md` §12.12 OPEN (map order) | When each input map is active | The UI pushes the active panel's input map with priority 0 and the focus element's with priority 50, each element's ancestors one priority lower per level; the stack is ordered by priority, newest first among equals, identical pushes are ignored and a pop removes an element's map at every priority (§5.2). The full input-layer order is still open there. | BIN |
| `fonts.md` §7.4 OPEN (Q13) | Extents and line handling with `UICore_Text_truncate_text_to_fit` | Truncation rule in §10.1 (overflow test, last visible line, cut position, suffix drawing) and the truncated-text tooltip in §10.2; the fit rule at the cut stays OPEN. | BIN |
| `install-layout.md` §8.2 | Settings-folder contents list `ui/skins/…` | Add `ui/layouts/<name>.layout` (UI layout files, §8.5; content OPEN). | BIN |
| `lua-plugin-api.md` §13.1 (focus) | Tessera rule: a text box takes focus when clicked | **Agreement, with a caveat.** The engine's click handling (§4.6 step 3) activates the root panel but does not itself move keyboard focus; focus moves through §5.2 (code, `focus_on_show`, a panel's designated focus element). Whether the engine's editable Text widget takes focus on its own press message was not traced (§5.2 OPEN), so the Tessera rule stands. | BIN |

---

<a id="s17"></a>
## 17. Open questions (summary)

Every item has an in-section `> **OPEN:**` callout.

1. Triple presses within the double-click time (§3.3).
2. Hit-mask and alpha-threshold pass rules (§4.1).
3. Per-widget mouse-visible rules; the reader of `UICore_Element_mouse_invisible`; how the
   mouse-button flag of UL §9.2 step 5 maps onto the two flags of the hit test (§4.3).
4. Bounds of the enter/leave chain (§4.4).
5. Which elements accept double clicks; tap commands; the special binding class (§4.6).
6. Wheel bubbling and scroll step (§4.7).
7. Input maps of engine widgets; a panel's designated focus element; the focus-refusal rule
   (§5.2).
8. Character path, caret, selection, IME and `max_chars` details of text entry (§5.4).
9. Result handling of the other engine dialogs (§5.5).
10. Keyboard navigation handler and tab order (§5.6).
11. `max_height` of tooltips (§6.4).
12. Tooltip detach and game tooltips (§6.6).
13. Drag cancel and payload rules (§7.4).
14. `resize_line` outline mode; caps for non-root targets; `force_match_parent_scale`,
    `clip_to_parent` (§8.2).
15. Clamping saved positions after a resolution change; `ClampGameViewEdgeToObject` (§8.3).
16. Storage of saved panel data and when game code applies it (§8.4).
17. Commands that save and load `.layout` files; the identifier forms of UL §9.9; the location
    of the options XML (§8.5).
18. Drawing of scaled Stretch panels; plugin window scale (§9.2).
19. What game code does on the screen-resized notification (§9.4).
20. Fit rule at the truncation cut (§10.1).
21. Widget classes that replace the hover handler; the hover-ended notification (§6.2).

---

## Change log

| Date | Change |
|---|---|
| 2026-10-06 | First complete draft (status Review): input commands and click timing (§3), hit test, mouse-visible rule, hover, capture, press/release/click/double-click, wheel, cursor and mouse-look (§4), activation and z-order, keyboard focus and input-map priorities, key routing, text-entry and dialog keys, navigation bindings (§5), tooltips (§6), drag and drop (§7), moving, resizing, clamping, saved positions and `ui/layouts` files (§8), UI-scale definitions, panel and cursor scale, screen size changes (§9), truncation and the truncated-text tooltip (§10), auto-fade and show/hide effects (§11), message table (§12), examples, invariants, census, corrections to `ui-layout.md`, `keymap.md`, `fonts.md`, `install-layout.md` and `lua-plugin-api.md`. BIN on client 4906.0071.1794.4035; OBS on the owner's install. |
| 2026-10-06 | **Revision 1** (answers the first review; status stays Review). **Corrected:** §6.3 census, UB19, UB20: each tooltip variant has two children, a background Field (0, 0, 512 × 512, z-level 1, edges 5) and the Text element (z-level 0), re-checked on all 4 variants; §6.4 step 2: both children follow the panel by their edges. UB-I9 restricted to the case without aspect ratio and without a cap change; §8.2 now says the limits of step 4.3 are not final; §13.5 extended with `min_height` 240 (350 × 233). §8.4 step 3 and §8.5 OPEN follow the M5 revision of UL (§9.2 step 5 is the listener registry; saved locations are UL §9.9); §12.2 row 46 updated. **Added (BIN, same build, re-traced):** §8.1 step 3 clamp order (near side, then far side, per axis; the far side wins), UB-I8 rewritten, §13.10; §5.2 steps 1–2 input-map stack rules (ancestor pushes at one priority lower per level, ordering, duplicate and removal rules, the active panel's push on clearing focus restores entries removed by the old focus element's pop), UB12, UB-I14, §13.9; §6.2 hover-handled flag, the handler's result, repetition on every frame when not handled, hover-ended notification, UB18, UB-I13, new OPEN (§17 item 21); §4.3 OPEN on the relation to UL's mouse-button flag (§17 item 3). UB40 (Reported) for the Windows defaults of §3.3. **Rephrased** as plain unknowns: §3.3 OPEN, §5.6 OPEN, §16 row 2. |
