<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0014 — UI input bridge. UI commands are keymap actions, activation timing lives in the mapper, and the UI map stack becomes a UI band of the context stack

**Status:** Accepted by the owner, 2026-10-06 (approved with its companion record and the
questions in the owner summary answered "yes"). The open review items below are left to the
M5 implementation.

## Owner summary

**The decision in five lines.**
1. UI commands become ordinary `tessera-input` actions that carry their InputActions command
   name and ID, so there is one keymap file and one resolver, and everything is rebindable
   (D1, D2).
2. Bindings gain an activation filter (`on` = press, release, tap, double, double_nearby),
   computed per button in the mapper from the platform's double-click time and rectangle
   (D3, D4).
3. The UI's input-map stack becomes a UI band of owned, prioritised entries above the game
   contexts. The mouse-click actions sit in a pass-through `pointer` context, so the run
   chord and mouse-look keep working (D4–D6).
4. The mapper offers each event to the UI in order through a synchronous sink. The UI's
   verdict decides who owns the hold (the client overrides it only for a right press during
   an item drag), and a declined command goes on to game code unchanged (D7).
5. Input routing and plugin input handlers run in frame-loop step 1, against the elements'
   current geometry. The UI tick (hover, `MouseHover`, the plugins' `Update`), the plugin
   host's deferred work (sliced by whole items; it also delivers the plugin events raised by
   game and session state, such as `Turbine.Chat.Received`) and the collector step run in
   step 7, and paint runs in step 8. The only plugin code outside steps 1 and 7 is the
   unload at client exit, after the last frame (D8).

**Questions the owner must answer.**
1. Do you accept option C (extend `tessera-input`) and the substantial `keymap.md` revision
   it needs (the list near the end), rather than running the original's input-map model?
2. Do you accept the rewording of ARCHITECTURE §20.3 and §18.1, LPA §3.3 and
   `plugin-runtime.md` §10.3 (D8, owners list)? That includes two Tessera rules: plugin
   events raised by game or session state are queued and delivered in step 7, not when the
   state changes (Known difference 21), and the unload at logout runs in step 7. It must be
   accepted together with `0015-ui-rendering.md` and the follow-ups under "Coordination".
3. Do you accept these visible changes? With a UI attached, Escape opens the game menu
   instead of quitting. A quick right re-press over a panel element is a double click and
   does not resume mouse-look. During any item drag a right press neither turns the camera
   nor locks the pointer (partly a Tessera rule, D7). `text_entry` changes: mouse buttons,
   the wheel and mouse-look pass through it while typing, and `text_submit`/`text_cancel`
   are replaced by `open_chat`/`menu` (Consequences).
4. Three in-game observations, none of which blocks M5. In the original, does mouse-look go
   on after a quick right re-press over the world, and does running go on after a quick
   second left press while right is held (Known difference 14)? Does the wheel zoom the
   camera over a plain plugin window (D5 OPEN)? What does a right press do while an item is
   being dragged from a bag or a quick slot: turn the camera, cancel the drag, or nothing
   (Known difference 20)?


## Open review items (2026-10-06)

After five revise-and-review rounds an independent clean-room review still lists the items
below. They are recorded here for the owner and for the M5 implementer; none changes the
chosen option.

**Must fix before acceptance:**

1. D8 'Game-raised plugin events' (lines 812-826) contradicts LPA §9.6 (BIN). LPA §9.6 says a chat line produced while a `Turbine.Chat.Received` dispatch is running (for example by `Shell.WriteLine` in a handler) is not delivered to that apartment's `Received` handlers. LPA §11.1's Tessera rule ('never re-entrantly (§9.6)') and Appendix B keep that rule. D8 queues every such line, including lines raised 'while plugin code runs (in step 1 or 7)', and later delivers it 'to the handlers set at delivery'. The writing apartment therefore does get its own line, one item later. Failure case: a plugin that echoes or logs each received line with `WriteLine` produces a new queued `Received` event per delivery. Since queued items run in the same frame until the 2 ms slice is used, that apartment loops forever: it burns the deferred-work slice every frame, floods the chat and starves real deferred work (`LoadPlugin`, `PluginData` callbacks, the logout unload). Fix: record with each queued chat event the apartment(s) whose `Received` dispatch was running when it was raised, and skip those apartments at delivery. Add this to Known difference 21, the LPA §11.3 owners entry and invariant (y).
2. D8 claims, and owner summary item 5, the ARCHITECTURE §18.1/§20.3 rewording and invariant (y) assert, that no plugin code runs in steps 2-6 or 8-10. The record's own rules do not guarantee this. D8 keeps LPA §9.7 synchronous delivery for UI notifications. Any UI change made by game or session code outside steps 1 and 7 therefore runs plugin handlers at that point. Example: a session event in step 2 makes game code show a native panel with `activate_on_show` (UB §11.2, §5.1). LPA §13.14 shares activation between client panels and plugin windows, so the active plugin window gets `Deactivated` and its focused TextBox gets `FocusLost` synchronously in step 2. A `HideWhenField` hiding the active panel on a player-state change does the same. A window resize, or a scale change handled in a winit callback, re-lays out root panels (UB §9.4), and the resulting `SizeChanged`/`PositionChanged` notifications are not input handlers, which is all D8 step 1 lists. Fix: either require that game- and session-driven UI changes (show/hide, activation, focus, screen resize) are applied in step 7 (for example as deferred items or before the tick), or widen the stated rule and invariant (y) to include them.
3. The chord-exclusion rule contradicts the release-class rule (D7 lines 553-557, D4 lines 321-323, keymap list item 8 lines 1404-1409). D7 says a button whose holds are all UI-owned 'is left out of the held set used to complete chords until it is released'. D4 and item 8 say release-class resolution 'runs only for the release of a button that the mapper holds as down (§7.2's held set)'. §7.2 has one held set: it completes chords, suppresses repeated presses and carries the 64-button cap. Item 8 lists both 'held-set exclusion of UI-consumed buttons' and 'Release-class resolution runs only at the release of a button in the held set'. Read literally, the release of a left press consumed by a panel is a release of a button not held. `tap_left` then never fires, which contradicts D4's own sentence ('`tap_left` in `pointer` still fires and is offered at the end of a click on a panel element'), invariant (o) and the worked example (the `tap_left` pulse at 150 ms). Fix: define the exclusion as a mark on a button that stays held (chord completion ignores marked buttons). The button stays in §7.2's held set for press suppression, the cap and release-class resolution. Say this in D7 and item 8.

**Other improvements:**

- Importer gap (list items 17 and 18): item 17 adds the row `ScrollableControls_Mouse` `ScrollUp`/`ScrollDown` -> `ui.scroll_mouse`. Yet §12.14.5 translates wheel bindings only for zoom commands and drops 'any other wheel binding', and nothing maps a wheel sub-control bound to a button action onto `WheelUp`/`WheelDown` clicks. So every import that includes that section leaves the actions out and warns. Add a translation rule (AxisPositive/AxisNegative bound to a button action -> a `WheelUp`/`WheelDown` click entry).
- D8 places input handled in winit callbacks in step 1 of the following frame, but does not say when the frame clock (`Turbine.Engine.GetGameTime()`, 'constant within one frame', LPA §12) and the UI clock behind the 0.25 s `MouseHover` rule advance. If the clock advances at the start of the redraw, step-1 handlers run in callbacks see the previous frame's value while `Update` sees the new one. Say where the clock steps, for example right after present.
- ARCHITECTURE §21 says there is no blocking I/O on the main thread after start-up. D8 runs the logout unload as a step-7 item, and unloads requested by a user or plugin inside step 1 or 7. Each closes Lua states, which LPA §6.3 step 5 and §10.6 require to write `PluginData.Save` synchronously or flush before closing. Note this as an exception in the owners list (ARCH §21), or say how the flush avoids blocking the frame.
- D6 sends typed characters to the UI when a `text` context sits 'at or above its first `opaque` entry'. It never mentions `opaque_keys`. So `ui.eat_keyboard` (opaque_keys, no text) pushed above a focused field would not stop characters reaching it. Nobody pushes that context yet, but the rule probably should key on `opaque_keys` too.
- Consequences ('Effects on other code') says `StandardActions` 'loses those three fields'. It has four: `text_submit`, `text_cancel`, `text_history_prev` and `text_history_next` (`crates/input/src/defaults.rs`).
- Consequences ('What the owner will notice'): 'PageUp, PageDown, Ctrl+Home and Ctrl+End page its log instead of zooming or resetting the camera'. Today Ctrl+Home does not reset the camera (`reset_camera` is Exact `Home`) and End is unbound, so only the zoom part is a change.
- D7 'During an item drag': when the client consumes `mouselook` (right becomes UI-owned), the chord-exclusion rule also keeps the run chord from completing with that right button after the drag, until right is pressed again. Known difference 20 mentions only the camera; add running.
- Once the UI is attached, D7/D8 tie the pointer lock to winit grab and hide calls made from inside the sink callback. Say explicitly that the sink may call window APIs during `handle_with`. The main thread owns the window, so this is fine, but an implementer should know the sink is not pure.
- Scale-factor change during the pointer lock: D3 resets recorded press positions to unknown, but *F* and the mapper's frozen position stay in the old physical pixels until the lock ends. State whether *F* is rescaled or the lock is ended.

## Context
The native UI and the plugin API take input as **commands**, not keys (`ui-behaviour.md` §3.1,
`lua-plugin-api.md` §13.12–§13.13). Each delivery carries an InputActions command ID, a press
or a release, a value (1.0 for a digital press) and the pointer position. The UI sorts
deliveries by command:

- **Mouse-button commands** are routed by pointer position: capture, `MouseDown`/`MouseUp`/
  `MouseClick`/`MouseDoubleClick`, and panel activation (UB §4.5–§4.6, LPA §13.12.2). There
  are nine engine commands (select, double-select and tap, each left/right/middle). Game code
  adds four modifier-click commands when the game UI starts: `DressingRoom`, `AutoLootAll`,
  `ItemLinkToChat`, `MultiUseItem` (UB §3.1).
- **Wheel commands** (`ScrollUp`, `ScrollDown`) go to the captured or hit element and bubble
  to the nearest usable scroll host (UB §4.7, LPA §13.12.6). They exist only while the pointer
  is over a scrollable or a scroll bar: those elements push the input map
  `ScrollableControls_Mouse` with priority 0 for the hovered element and pop it when the
  pointer leaves (`ui-widgets.md` §5.7, §6.9, BIN). One wheel step is one line step of the
  bar (UW §6.8).
- **Every other command** goes to the keyboard target, which is the focus element or the
  active panel. That covers text editing (`Cursor…`, `BackspaceKey`, `DeleteKey`,
  `Copy/Cut/PasteText`), dialog keys (`AcceptKey`, `AcceptInput`, `EscapeKey`) and
  navigation (`FocusNext`, `FocusPrevious`, `FocusMove…`). If the target does not consume
  the command, it goes on to game code and to the plugins' `KeyDown`/`KeyUp` broadcast with
  the command ID (UB §5.3–§5.6, LPA §13.13, §14.4). Action 0xAB (`Scroll`) is ignored by the
  plugin UI (LPA §13.13).
- The UI's only toggle-type test is "Impulse (type 2) or not", read from the action map at
  run time (LPA §13.12.1, `keymap.md` §12.10). An Impulse action never gives plugin button
  events, and its end is ignored.
- Which command a control produces depends on **activation bits** (Down, Up, Tap,
  DblClickDown/Up, NearbyDown/Up, `keymap.md` §12.5.7). The input layer computes them from
  the platform's double-click time and rectangle (UB §3.3).
- Elements push and pop **input maps** on a **map stack**. The active panel pushes with
  priority 0 and the focus element with 50. Each ancestor pushes one priority lower per
  level. The stack is ordered by priority, newest first among equals; an identical push
  changes nothing; a pop removes the element's map at every priority (UB §5.2, §13.9).
- Plugin events caused by an input are delivered before the input processing that caused
  them continues (LPA §9.7), and input is delivered when the client processes input,
  outside the UI tick (LPA §3.3).

Tessera's input layer (`keymap.md` §2–§10, `crates/input`) has none of this:

- Its contexts have fixed priorities and are switched on and off by code. There is no
  pushing per element and no owner.
- Its built-in map (§8) has no UI context and no UI actions. It only has `text_entry` for a
  chat line, and no client code uses that yet.
- Resolution yields a per-frame **snapshot** (§7.1). The order of events within a frame is
  lost, and so are the pointer positions and modifiers at each edge.
- Bindings cannot say tap, release or double click. The importer drops such bindings
  (§12.14.5).
- Nothing can tell the mapper that the UI consumed a command.
- OS auto-repeat presses are ignored, and auto-repeat for UI actions is OPEN (§11).
- Analog presses of button actions and modifier holds are computed in `update`, not in
  `handle` (`crates/input/src/mapper.rs`). Holds also end outside `handle`: in `activate`,
  `deactivate`, `set_keymap`, run-time registration and `release_all`.

The OPEN of `keymap.md` §12.12 matters here. When the original consults each input map, and in
what order, is still unknown beyond the UI's own pushes.

## Options considered

### 1. Who turns devices into UI commands
| Option | For | Against |
|---|---|---|
| **A. The UI reads raw events itself** (egui style, hard-coded buttons and keys) | Small; no keymap change | UI commands cannot be rebound. Plugins still need command IDs on `KeyDown`, so the keymap is needed anyway. Two places interpret Escape and Enter. Imported UI sections have nowhere to go. It contradicts the command model that UB and LPA are written in |
| **B. Run the original's input-map model** (activation bitmasks, meta keys, best match per map, defaults from the 0x1D records) | Highest fidelity; defaults come from the user's install | It replaces an Accepted spec and working M3 code. Its core, the order in which maps are consulted, is OPEN (§12.12). Input would stop working without install data (tests, explorer). Tessera's own keymap file would become a second system |
| **C. Extend `tessera-input`** (chosen): UI commands become ordinary Tessera actions carrying a command name. Bindings gain an activation filter. UI pushes become entries of a UI band. A synchronous offer lets the UI consume | Keeps one keymap, one file and one resolver. Rebindable. The importer can translate UI sections. Plugins get command IDs from §12.15. Works without install data (D1) | `keymap.md` needs a substantial revision (list below). The difference in map order from the original stays OPEN |

### 2. Where tap, double-click and nearby are computed
| Option | For | Against |
|---|---|---|
| In `tessera-ui` | Close to the elements | Bindings could not select by activation, so game bindings (a right-click tap on the world) and imported tap/double bindings would be impossible. Gestures would be tied to mouse buttons |
| From OS double-click events (winit has none) | Uses the platform's own detector | Not available portably. Tap and nearby would still be needed |
| **In the mapper, per button** (chosen) | One place. Works for every button kind. Bindings choose by activation. Matches UB §3.3, which places it in the input layer | The mapper needs the platform's double-click time and rectangle, which the client must read |

### 3. How keymap contexts relate to the UI map stack
| Option | For | Against |
|---|---|---|
| A separate UI stack outside the mapper, consulted by the client before the mapper | No change to contexts | Two resolvers, and holds split between them. Opaque releases and focus-loss releases cannot see each other |
| Fixed-priority contexts that the UI only switches on and off | Uses today's API | Cannot hold the same map twice (UB §13.9 has (*MP*, *P*, 49) and (*MP*, *P*, 0)). Loses element ownership and the pop-at-every-priority rule |
| **A UI band of owned, prioritised entries inside the context stack** (chosen) | Reproduces UB §5.2 exactly. One resolver, one set of holds | New mapper operations and a band rule |

### 4. What an unconsumed command does
| Option | For | Against |
|---|---|---|
| Resolution continues below the UI context that produced it (re-resolve the press) | Arrow keys in a dialog would still move the character if the dialog declines | Unlike the original, where the **same command** goes on to game code (UB §5.3). A declined Escape in a text field could reach two Escape actions and be offered twice. Needs rules against double offers |
| **The same action goes on to game code** (chosen) | Matches UB §5.3 and LPA §13.13. Simple: the verdict only decides who owns the hold | Built-in UI contexts must bind the same actions that game code reacts to (Escape → `menu`, Enter → `open_chat`). Otherwise a declined key does nothing in the game. Declined `focus_*` arrows do not turn the character (Consequences) |

The pass-through context of D2 and D4 follows the chosen option too: a declined `select_*`
is an ordinary game hold of the same action, which game code may read (D4, difference 9).
What it adds is that the declined press also continues to lower contexts, so game bindings
of the same button (the run chord, `mouselook`) fire as well. The chosen option also changes
what Escape does in the client (Consequences, "Escape with a UI attached").

### 5. How the UI's verdict reaches the mapper
| Option | For | Against |
|---|---|---|
| Deferred: queue events, the UI runs after `handle`, verdicts are fed back | No callback | Game holds would have to be provisional and rolled back. Chord completion and opaque releases depend on them before the next raw event. Plugin handlers may change what the next event hits (LPA §9.7), so the UI cannot run them later in one batch either |
| **Synchronous: the mapper calls a sink trait during `handle_with`** (chosen) | Exact order, as in the original (UB §12.1 queueing stays inside the UI) | The sink runs plugin code at frame-loop step 1, so the frame loop and the UI budget must say so (D8). Stack changes made there must be queued (D5) |

### 6. How the pointer actions coexist with game bindings on the same buttons
The game band binds `MouseLeft+MouseRight` (run) and `MouseRight` (`mouselook`); the UI needs
`MouseLeft` as `select_left`. Under §7.2 the first context with a match takes the press.

| Option | For | Against |
|---|---|---|
| A normal context above the game band | Simple | Holding right and then pressing left gives `select_left`, so the run chord never completes; a user binding on plain `MouseLeft` in a game context is hidden. The walker regresses |
| A normal context, active only while a UI is attached | The walker without UI is unchanged | The run chord still breaks with a UI attached |
| Put the run chord in the pointer context | The chord fires with `select_left` (§7.2 fires every match in one context) | Mixes game and UI actions in one context; a user who rebinds run must know to edit `pointer`. Other game bindings on mouse buttons are still hidden |
| **A pass-through context** (chosen): all its matches fire and are offered, `double` and `double_nearby` included; the press continues to lower contexts unless the UI consumed one of their Starts | The run chord, `mouselook` and game mouse bindings work in either press order, and also on a quick second press of the same button. A click on a panel still does not reach the world, unless a game-band context has an exact binding of that click with its modifiers (for example a user's `Shift+MouseLeft`): that binding takes the press in §7.2's first pass, before `pointer`'s Subset `select_left` matches in the second (D2, as §12.12's exact-mask preference would). Without a sink it hides nothing | One more context flag. While right is held first, `select_left` fires together with run (Known difference 9). A quick second press over the world fires both the double-select action and the game binding, where §12.12 may pick the double-select alone (Known difference 14, inferred) |
| A pass-through context whose `double`/`double_nearby` matches always consume (revision 1) | Follows §12.12's largest-activation rule, if both bindings sit in one map | Hides `mouselook` on a quick right re-press, and run on a quick second left press, whenever a UI is attached; during mouse-look the frozen position makes every re-press "nearby". The premise is not established (Known difference 14) |

### 7. How the wheel over the UI is kept from zooming the camera
| Option | For | Against |
|---|---|---|
| A hit-test callback (`claims_pointer`) that withholds wheel motion over any element | Simple to state | Not the original's rule: a Tessera hit-test rule that disagrees with the map pushes of UW §5.7. A second path beside the band |
| **The UW §5.7 push** (chosen): `ScrollableControls_Mouse` becomes the UI-band context `ui.scroll_mouse`; a UI-band context that binds a wheel click also takes that event's wheel motion | Follows the BIN fact of UW §5.7 and §6.9; no callback; captures need no special case, because the entry follows hover as UB §4.4 defines it | Over a non-scrollable native panel the wheel zooms the camera, as the original's map pushes imply. So does a capture whose pointer has left the captured element, for example a scroll-bar thumb drag that leaves the thumb: hover is then nothing (UB §4.4 step 2), the bar pops the entry (UW §6.9) and the wheel zooms until the pointer is back over the thumb. Which plugin controls push is OPEN (D5) |

## Decision
`tessera-input` produces the UI's commands. `tessera-ui` routes them. The client wires the two
together. Neither crate depends on the other: `tessera-input` defines a sink trait and its own
event type, and the client copies each event into `tessera-ui`'s own command type. No new
sibling allowance is needed (ARCH §3). Everything runs on the main thread (ARCH §21).

**D1. Actions carry command names.**
- Each event carries a **command name** (an InputActions name) and, when it resolves, the
  command **ID**. The name comes from the §12.15 rules: the entry's own imported command
  first, then the (context, action) row of the link table (§12.14.2 and §12.15, extended
  by items 17 and 20 of the list below).
- The client resolves names to IDs at run time from the mapper chain (`keymap.md` §12.11).
  Without that data (synthetic `tessera-ui` tests, a missing record) the ID is "none".
- An event **with a name** is offered to the UI, with or without an ID. The UI routes it by
  the class of the name: one of the nine mouse-button names, a wheel name, or anything else
  as a key command. The `MouseButton` field of plugin button events comes from the ID table
  of LPA §13.12.1, or from the name (Left for `SelectLeft`, `SelectDblLeft`, `TapLeft`, and so
  on) when there is no ID. Only the plugin `KeyDown`/`KeyUp` broadcast needs the ID; an event
  without an ID skips it.
- An event **without a name** (for example the `fly_*` actions, which have no row) is not
  offered. It goes straight to game code, and plugins cannot see it.
- The **value** offered is the absolute entry value: 1.0 for button actions and plain
  entries (UB §3.1, "1.0 for a digital press"). The sign of an axis entry already chose the
  command (§12.15 rule 2), so it is not repeated in the value.

**D2. New built-in contexts and actions** (Tessera names; defaults in the list for
`keymap.md` below):
- `pointer`: game band, priority 40, **pass-through** (D4). It holds the nine pointer actions
  `select_left/right/middle`, `select_double_left/right/middle` and `tap_left/right/middle`,
  linked to `SelectLeft`, …, `TapMid`. These are the original's `Main` mouse-click rows
  (UB §3.2). The actions have the **Subset** modifier policy (§2), so a Shift-, Ctrl- or
  Alt-click still gives `select_left`; an exact binding of the same click, in `pointer` or
  elsewhere, wins in §7.2's first pass (D4, "Pass-through and the two passes", says why
  this also holds inside `pointer`). An exact binding in a game-band context below
  `pointer` therefore takes that modifier-click even over a panel, and the UI never sees
  it (Options 6).
- `pointer` is not activated by the walk and fly mode helpers. The client activates it when
  it attaches a UI.
- UI band contexts, which the UI pushes (D5):
  - `ui.dialog` (`DialogBoxes`): `focus_next`, `focus_prev`, `focus_left/right/up/down`, and
    `menu` on Escape.
  - `text_entry` (`EditControls`): the existing context, which no client code uses yet, moved
    into the UI band and given the caret actions.
  - `ui.clipboard` (`CopyAndPasteControls`).
  - `ui.scroll_mouse` (`ScrollableControls_Mouse`): `scroll_up`/`scroll_down` on
    `WheelUp`/`WheelDown`, linked to `ScrollUp`/`ScrollDown`.
  - `ui.scroll` (`ScrollableControls`): empty by default.
  - `ui.eat_keyboard` (`EatKeyboardInput`): no bindings, opaque to keys.
  - `ui.chat_log` (no InputMap; a Tessera context): `text_page_up/down` and
    `text_doc_start/end`, so the chat window can page its log while it is the keyboard
    target and its entry has no focus (`chat-window.md` §6.2).
- In `text_entry`, Escape is bound to `menu` and Enter/NumpadEnter to `open_chat`. These are
  the actions game code already handles for `EscapeKey`/`AcceptInput`, so a field that
  declines them behaves like the original, where the command goes on to game code.
  `text_submit` and `text_cancel` are retired. `text_history_prev/next` become
  `text_line_up/down` (`CursorPreviousLine`/`CursorNextLine`); history is the chat widget's
  reading of those commands. Saved overrides of the old names are migrated (list item 5).
- **Modifier policies of the new UI actions** (§2):
  - **Subset**: the caret actions (`text_char_*`, `text_word_*`, `text_line_*`,
    `text_page_*`, `text_line_start/end`, `text_doc_start/end`), `text_backspace`,
    `text_delete` and `scroll_up`/`scroll_down` (so Shift+wheel over a list still scrolls;
    its motion is withheld from the camera in either case, D7). The original matches a
    binding whose mask is a subset of the held meta keys (§12.12), so it delivers these
    commands with extra modifiers held, and the UI reads the modifiers from the offered
    event. The chat window needs this: it gives
    `CursorPreviousLine`/`CursorNextLine` another meaning with Shift or Ctrl held
    (`chat-window.md` §6.4). So Shift+Left gives `text_char_left` with Shift, and
    Shift+Up and Ctrl+Up give `text_line_up`. Because §7.2 fires every match of one
    context, the word and document moves also get Ctrl+Shift default bindings (list item
    13), which match exactly, so that Ctrl+Shift+Left gives `text_word_left` alone in the first pass instead of both
    `text_char_left` and `text_word_left` in the second (Known difference 19).
  - **Exact** (the button default, stated here on purpose): the focus actions and the
    clipboard actions; `menu` and `open_chat` keep their existing Exact policy. Subset would
    make Ctrl+Shift+Insert both copy (`Ctrl+Insert`) and paste (`Shift+Insert`). The cost
    is that Ctrl+Tab or Ctrl+Shift+C give no command (Known difference 19).
- A context belongs to one band, fixed at registration (built-in and run-time registrations
  alike, D5 "Code access to the band"). Inside the band, the entry's priority orders a
  UI-band context (D5). Its registered `priority` is used only when code activates it with
  `activate`. `text_entry` keeps 100, and the other UI-band built-ins register 0.

**D3. Activation filter on bindings.**
- A binding entry may carry `on`:

  | `on` | Meaning |
  |---|---|
  | `press` (default) | Today's behaviour: a hold from press to release |
  | `release` | A pulse at every release |
  | `tap` | A pulse at a release that comes at most half(*T*) after the press |
  | `double` | A hold from a press that comes at most *T* after the previous press of the same button, until its release |
  | `double_nearby` | As `double`, and the pointer lies inside the half-open rectangle of ± half(*W*) × half(*H*) around the previous press position |

  A *pulse* is a hold that starts and ends at once, as wheel clicks are today. These are the
  rules of UB §3.3.
- *T*, *W* and *H* come from the platform. The client reads them: the double-click time and
  rectangle on Windows, the desktop settings on Linux (an X settings or desktop double-click
  time and distance; a distance *d* gives *W* = *H* = 2*d*), and the system interval on macOS.
- When the platform gives nothing, the client uses Tessera's settings `[Input]`
  `double_click_ms` (default 500) and `double_click_px` (default 4). `double_click_px` is the
  **size of the rectangle**, not a distance: *W* = *H* = `double_click_px` (so the default
  accepts ± 2 px, like the Windows default 4 × 4 of UB §3.3). It is in **logical** pixels:
  the client multiplies it by the window's scale factor and rounds to the nearest integer,
  so on a display with scale factor 2 the default becomes *W* = *H* = 8 physical pixels
  (± 4). Tessera's settings can also override the platform values. A plain Wayland
  compositor such as Hyprland (the reference machine, decision 0010) normally publishes no
  double-click values, so there the fallback applies.
- **Pixel space.** Pointer positions in events, press positions and *W*, *H* are physical
  pixels of the window's client area, origin top-left (winit's physical position; UB §3.1).
  A platform value given in logical pixels, and `double_click_px`, are multiplied by the
  window's scale factor. `tessera-ui` converts positions into its own layout space (UB §9).
- The mapper clamps *T* into 100–5000 ms and *W*, *H* (after scaling) into 2–64 px. The
  lower bound is 2 because half(1) = 0 would make the half-open rectangle empty, so that no
  press would ever be nearby; with 2 it is 2 × 2 pixels (*x0* − 1 ≤ *x* < *x0* + 1, and the
  same for *y*), so a press at the previous position is nearby.
- **Scale-factor changes.** The client reads the platform values at start-up and again when
  the window's scale factor changes at run time (winit's scale-factor event). It then
  recomputes *W* and *H* (scaling, rounding, clamping as above) and hands them to the
  mapper, which also sets every recorded press position to "unknown": positions recorded
  in the old physical pixels are not compared with new ones. So the first press after a
  scale change is never nearby. *T* is unaffected.
- Tracking is per button, for keys, mouse buttons and pad buttons alike. It uses the last
  press time and position (bounded to the 64 most recent buttons). The press position is the
  last cursor position, or "unknown", which never counts as nearby.
- **Which presses are recorded.** Only a press that §7.2 resolves updates the button's last
  press time and position. A press that §7.2 ignores does not: an OS auto-repeat press, a
  press of a button that is already held, and a press made while 64 buttons are held. So
  holding a key with OS repeat never makes its next real press a double press, and the
  third-press rule below counts only real presses.
- Tessera's reading of the UB §3.3 OPEN: a third press within *T* of the second is again a
  double press. Every press is compared with the previous one.
- A non-default `on` is allowed only on a single-stroke binding with one button plus
  modifiers, of a **button** action. It is not allowed on wheel clicks, chords, sequences,
  analog entries, or any entry of an axis action, where a zero-length pulse would add
  nothing to `value`. Where each case is caught:
  - **From the entry alone** (a sequence, a stroke with more than one button, a wheel
    click, an analog entry): at **read time**, like any other invalid entry of §6.3. The
    lenient reader drops the entry with a warning, the strict reader fails, and
    `Keymap::set` from code refuses it with an error, as it refuses the entries the strict
    reader rejects.
  - **Only against the registry** (a non-`press` `on` on an entry of an **axis** action, which
    the file alone cannot tell): when the keymap is compiled against the registry, the
    entry is disabled with a warning, like the prefix bindings of §5.2. `Keymap::set`
    accepts it, because the keymap has no registry.

**D4. Each activation class is resolved at its own event.**
- *Press-class* bindings (`press`, `double`, `double_nearby`) are resolved at the press by
  §7.2. Only press-class bindings decide which context consumes the press.
- *Release-class* bindings (`release`, `tap`) are resolved at the release, separately, with
  §7.2's two passes (exact, then subset). The first context in resolution order at that
  moment with a matching release-class binding fires. Modifiers are those held at the
  release, and the opaque rules apply.
- **Which releases resolve.** Release-class resolution runs only for the release of a
  button that the mapper holds as down (§7.2's held set). The release of a button that is
  not held changes nothing, as today: no hold ends and no `release` or `tap` binding fires.
  The winit adapter relies on this for the synthetic releases it forwards (§10). Holds that
  end without a physical release fire no release-class binding: focus loss and
  `release_all` clear the held set and end every hold, but resolve no `release` or `tap`
  binding (the button was not seen to be released, and a pulse at focus loss would act on
  a window the user has left). The same holds for holds ended by `activate`, `deactivate`,
  `set_keymap`, run-time registration and band pushes and pops: the buttons stay held, and
  their later physical release resolves release-class bindings as usual.
- So `select_right` (tap) in `pointer` and `mouselook` (press) in the game band both work on
  one right click: the hold ends first, then the pulse fires.
- **Release after a UI-owned press.** When the press of a button produced **at least one
  hold, and all of its holds were UI-owned** (D7), release-class resolution at its release
  considers only UI-band contexts and pass-through contexts; every other game-band context
  is skipped. A press that produced no hold at all (for example a key that has only a
  `release` binding, or none) does not qualify, so its release resolves against every
  context as usual and the imported release and tap bindings of list item 18 fire.
  Ownership is judged on the holds as they were made; a press whose holds were all dropped
  (D7, "Applying band requests") counts as having produced none. So `tap_left` in
  `pointer` still fires and is offered at the end of a click on a panel element, but a
  user's game-band `on = "release"` or `"tap"` binding of `MouseLeft` does not fire when
  left is released after dragging a panel, and a game-band release binding of Enter does
  not fire after a text field consumed the Enter press. This is the release-side
  counterpart of D7's chord rule. A release-class pulse of a pass-through context that the
  UI declines therefore continues to no game-band context at such a release.
- **Pass-through contexts.** A context flag, `pass_through`. When the first context with a
  match is pass-through:
  - its matching complete bindings fire and are offered at once, in the order below, whatever
    their `on` (a `double` or `double_nearby` match is no exception);
  - if the UI consumed one of their Starts, the press (or release) is consumed there;
  - otherwise resolution continues with the next context, in the same pass, as if the
    pass-through context had not matched. Its holds stay.
- **No sequences in a pass-through context.** A multi-stroke binding (for example a user's
  `select_left = "G G"` in `pointer`) is disabled with a warning when the keymap is compiled.
  So a pass-through context never has an incomplete match and never starts a pending
  sequence (§7.2 step 3), which would stop a press that should continue.
- **Pass-through and the two passes.** Each (context, binding entry) pair fires and is offered
  at most once per raw event. If nothing consumed the press in pass 1 (a pass-through context
  that fired there does not count), pass 2 runs as §7.2 says, with one rule for
  pass-through contexts: **a pass-through context that fired anything in pass 1 is skipped
  in pass 2**. It counts as having no match there, and resolution goes on below it. A
  pass-through context that fired nothing in pass 1 fires and offers its pass-2 matches as
  usual. This is how a normal context behaves, where a pass-1 match ends the resolution and
  its subset-only bindings never fire, so the exact pass prefers exact bindings inside
  `pointer` too. Example: with a modifier-click action bound to exact `Shift+MouseLeft` in
  `pointer` (difference 8), a Shift-click over the world fires that action in pass 1
  (declined, so the press continues); in pass 2 `pointer` is skipped, so `select_left` does
  not fire, and lower contexts get their subset matches. Without that exact binding,
  `pointer` fires nothing in pass 1 and `select_left` fires in pass 2 (invariant (m)). The
  rule also means that a `select_left` that fired in pass 1 never sends a second Start to
  the UI, which LPA §13.12.2 step 3 would turn into a release (`MouseUp`, possibly
  `MouseClick`). If pass 1 consumed the press (the UI through a pass-through binding, or a
  lower context), pass 2 does not run, as today. The same rules hold for the two passes of
  release-class resolution.
- **Consequences of the rule.** Without a sink nothing is consumed, so a pass-through context
  never hides a lower binding, whatever its `on` values. With a sink, it hides a lower binding
  only when the UI consumed one of its Starts, which takes a hit element or a capture (D7)
  and therefore never happens while the pointer is locked (UB §4.9). So a second right press
  within *T* over the world fires `select_double_right` (declined) and then `mouselook`; a
  quick second left press while right is held fires `select_left` and `select_double_left`
  (declined, the pointer is locked) and then the run chord. Over a panel element the same
  second right press is consumed by `select_double_right` and does not start `mouselook`
  (Known difference 14).
- Order of the events one raw event produces. The keys, most significant first, are the
  pass, the context, the activation class and the registration order:
  - At a press: the bindings that fire, pass 1 before pass 2; within a pass, context by
    context in resolution order (so a pass-through context's events come before those of
    the contexts below it); within one context, `press` before `double` before
    `double_nearby`, then action registration order.
  - At a release: the ends of the holds this release ends, in the order those holds began,
    then the release-class pulses, by the same keys (pass, context, then `release` before
    `tap`, then registration order). Each pulse is a start immediately followed by its
    end.
- This gives the sequence of LPA §13.12.2's worked example.

**D5. The UI band.**
- The context stack holds a **UI band** of entries (context, owner, priority), with owner an
  opaque `u64` (the element) and priority an `i32`. The band is consulted before every
  game-band context.
- Inside the band, entries are ordered by priority, highest first. A new entry goes before
  every entry of equal or lower priority. Pushing an identical (context, owner, priority) is
  a no-op and does not move the entry, unlike game-band reactivation.
- A pop by (context, owner) removes that pair's entries at every priority.
- A context listed several times is consulted once, at its first entry.
- The UI performs the ancestor walk of UB §5.2 itself and asks the client for each push and
  pop. The client maps the InputMap name to a context by the table below. A map without a
  context is ignored, with a debug log.
- At most 256 entries; a push beyond that is refused and logged (warning). The refusal is not
  reported back to the sink or to `tessera-ui`, whose record of its own pushes then holds
  one entry the band lacks. That is harmless: the missing entry's context is simply not
  consulted for that owner, a later pop of the pair removes whatever entries of it exist
  (possibly none, which changes nothing), and no hold can belong to an entry that was never
  made. The UI does not retry the push. Normal use needs about a dozen entries (one focus
  chain, one active panel, one hover push).
- When a context leaves the stack completely, its holds are released (§3), and a pending
  sequence (§7.2 step 1) of that context is dropped, as `deactivate` does today.
- **Band pushes and the pending sequence.** Today `activate` always drops a pending
  sequence. In the band the rule is narrower, because hover pushes of `ui.scroll_mouse`
  come and go with every pointer movement: a push drops the pending sequence only when it
  adds the band's **first** `opaque_keys` entry (D6) and the sequence's context is consulted
  after that entry (a game-band context, or a UI-band context below the new entry). Every
  other push, and an identical push, leaves the pending sequence alone. Without this rule a
  game-band sequence started before a field gains focus outside an offer (for example by a
  plugin's `Control:Focus()` call in an `Update` handler, LPA §13.13) could be completed by
  a key typed into the field, because §7.2 step 1 tries the pending sequence before any
  context and so bypasses `opaque_keys`. The same holds inside an offer: the push is applied
  when the offer returns (D7), and the sequence is dropped then. On a game-band context,
  `activate` keeps today's rule.
- **Code access to the band.** These rules fix what today's context API does with the two
  bands:
  - `register_context` takes the band. Built-in contexts have the bands of D2. A context
    registered on a running mapper (a plugin's) is in the game band unless its definition
    asks for the UI band. The band never changes.
  - Band push and pop name a context and an owner. A push or pop that names a game-band
    context is refused with a debug log and changes nothing.
  - Owner 0 is reserved for code; `tessera-ui` never uses it for an element. On a UI-band
    context, `activate(c)` is the push (*c*, 0, *c*'s registered priority). Activating it
    again is that identical push, so it does nothing and does not move the entry (unlike
    the game band). `deactivate(c)` is the pop (*c*, 0); entries of other owners stay.
    `is_active(c)` is true while *c* has any entry. `set_active(c, on)` compares *on* with
    whether the entry (*c*, 0) exists. On a game-band context the four operations keep
    today's meaning, and the walk and fly mode helpers touch game-band contexts only.
  - `handle` (no sink, D7) resolves against the band as it stands, whoever pushed the
    entries: `activate`, direct push calls, or offers made by an earlier `handle_with`.
  - So today's code, which activates `text_entry` with `activate` (`StandardActions`, the
    `input_runtime` fuzz target), keeps working. What `text_entry` does once it is active
    changes, as Consequences ("Effects on other code") and invariant (e2) state.
- **Queued stack changes.** A push or pop requested during an offer is queued. The mapper
  applies the queue as soon as that offer returns, and then offers at once the Ends that
  this causes (D7, "Applying band requests"); this holds for every offer, whichever step of
  D7 makes it. The context order a raw event is resolved against is fixed when its
  resolution starts (D7 step 3), also for a pass-through continuation, so a change made
  during that raw event's own offers takes effect from the next raw event; a pop still ends
  the popped context's holds at once (§3), and a hold of that raw event whose Start was not
  offered yet is dropped (D7). Borrowing enforces this: the sink cannot reach the mapper.
- **The request channel.** With each offer the mapper passes the sink a mutable **band
  request list**, owned by the mapper and empty at the start of the offer; the sink appends
  push (context, owner, priority) and pop (context, owner) requests to it in the order the UI
  made them. When the offer returns, the mapper applies the list in order and clears it,
  before it offers anything else. So the sink never borrows the mapper. On the UI side, `tessera-ui` records each push and pop
  of its ancestor walk (InputMap name, element, priority) in an outbox. During an offer the
  client, as the sink, translates the outbox into the request list by the table below.
  Outside an offer the mapper is not borrowed. This covers the step-7 UI tick (D8): a hover
  change in its hover refresh, a focus or visibility change made by a plugin `Update`
  handler, and changes made by the plugin host's deferred work after the tick. At the end
  of step 7 the client drains the outbox and calls the mapper's push and pop operations
  directly. If anything was recorded after that, the client drains the outbox again at the
  end of step 8. Holds those calls end go to the end queue (D7), which is offered at the
  next frame's first `handle_with` or `offer_pending`. So the band that the next frame's
  step 1 resolves against already reflects this frame's tick.
- **Who pushes.** UB §5.2 says no layout element sets `InputMapID`, so most pushes are
  Tessera rules until the OPENs are answered:

  | InputMap (§12.5.4) | Tessera context | Pushed by |
  |---|---|---|
  | `Main` | game band (`global`, `camera`, `movement`, `fly`, `pointer`) | never pushed |
  | `DialogBoxes` | `ui.dialog` | a dialog of the engine dialog framework (UB §5.5, UL §9.1) while it is the active panel, priority 0 (Tessera rule; UB §5.6 OPEN) |
  | `EditControls` | `text_entry` | a focused editable Text element or plugin `TextBox`, priority 50 (Tessera rule; UB §5.2 OPEN) |
  | `CopyAndPasteControls` | `ui.clipboard` | the same element, right after `EditControls` (Tessera rule; UB §5.2 OPEN). Order inside the band does not matter (D6) |
  | `ScrollableControls_Mouse` | `ui.scroll_mouse` | a scrollable while the hovered element is the scrollable or one of its descendants, for that hovered element, priority 0 (UW §5.7, BIN; pop path by the UW §5.7 Tessera rule); a scroll bar while it is hovered, for itself, priority 0 (UW §6.9, BIN). Plugin controls: every hovered plugin control pushes it (Tessera rule, OPEN below) |
  | `ScrollableControls` | `ui.scroll` | nobody yet |
  | `EatKeyboardInput` | `ui.eat_keyboard` (Tessera reading of the name) | nobody yet |
  | none (Tessera context) | `ui.chat_log` | the chat window, for itself, priority 0, while it is the active panel (Tessera rule; `chat-window.md` §6.2 says what the window does with these commands without entry focus, not which map supplies them). The outbox records it under the Tessera name `ui.chat_log` in place of an InputMap name |
  | `IgnoreRemainingMaps`, `DebugConsole`, `StringTokenDebugger` | none | — |
  | `SelectionUI`, `MusicUI`, `MapPanel`, `VendorBuyUI`, `PVPGameplayUI` | none until those panels exist | — |

  > **OPEN:** whether plugin controls push `ScrollableControls_Mouse`, and which classes. The
  > plugin `Label` family derives from `ScrollableControl` (LPA §13.4), but the engine class
  > behind each plugin control is not specified. Tessera pushes for every hovered plugin
  > control, so every `MouseWheel` handler (21 surveyed plugin distributions use one,
  > `plugin-api-usage.md`) fires; the cost is that the wheel does not zoom the camera over a
  > plugin window. Owner observation: does the wheel zoom the camera over a plain plugin
  > window?

**D6. Keyboard-only opacity, decided at the band boundary.**
- A new context flag, `opaque_keys`. It covers keyboard keys and pad buttons; mouse buttons,
  the wheel and analog axes always pass.
- Inside the UI band, `opaque_keys` does **not** end the pass. A key press is tried against
  every UI-band context in band order, so a focused field's `text_entry` and `ui.clipboard`
  and a dialog's `ui.dialog` below it all see it, whatever their push order.
- At the end of the band: if any UI-band entry is `opaque_keys`, a key or pad press that no
  UI-band context took **ends the pass** there, as an `opaque` context does (§7.2 step 2).
  It reaches neither `pointer` nor the game band in that pass. It is not consumed, so if
  pass 1 ends this way, pass 2 runs and tries the UI band again with subset matches, and
  ends at the band end again if nothing there takes it. Only after both passes is the
  press swallowed. Invariant (q) depends on this: Shift+Left matches no exact binding in
  pass 1, and `text_char_left` (Subset) fires in pass 2.
- The full `opaque` flag keeps its meaning in both bands: it ends the pass at that context.
  No built-in UI context uses it.
- **Typed characters** (§7.6) go to the UI when the UI band holds a context with `text` at or
  above its first `opaque` entry. Otherwise the game-band rule of §7.6 applies. Under
  `handle`, which has no sink, the characters this rule would send to the UI go to the
  snapshot's `text`, as they do today while `text_entry` is first.
- Pushing the first `opaque_keys` entry (the band goes from none to one) releases the
  `pointer` and game-band holds whose last stroke has a key or a pad button. Further
  `opaque_keys` entries release nothing more. That includes the same context under another
  owner or priority: `text_entry` activated by code with `activate` (owner 0, priority 100)
  and pushed by a focused field (priority 50) can coexist, and the context is consulted
  once, at its first entry (D5). The release never touches the holds of UI-band contexts,
  wherever they sit relative to the new entry (for example `ui.chat_log` below a field's
  `text_entry`): they keep their holds, which end by the ordinary rules. While an
  `opaque_keys` entry is in the band, game-band modifier holds (§7.4) are not evaluated.
- `text_entry` gets `opaque_keys` and `text` in place of `opaque`. `ui.eat_keyboard` gets
  `opaque_keys`.
- So with a text field focused: W types instead of moving; Ctrl+C reaches `ui.clipboard`;
  Tab reaches `ui.dialog` below the field; a click elsewhere and the run chord still work,
  in either press order (D4).

**D7. The offer protocol.**
- `InputMapper::handle_with(event, now, sink)` replaces `handle` when a UI is attached.
  `handle` offers nothing. It resolves against the band as it stands (D5, "Code access to
  the band"), a pass-through context consumes nothing under it (D4), typed characters go to
  the snapshot (D6), and the Ends of holds that an earlier `handle_with` offered go to the
  end queue (holds it makes itself are never offered, below). With no UI-band entry, the snapshot of every action that exists today is
  today's (invariant (e)). An active `text_entry` behaves as Consequences ("Effects on other
  code") says.
- **What is offered**, in arrival order:
  - action **Start**, **End** and **Repeat** events of **digital holds** made by resolution
    (§7.2, D4), each with the action, the command name and ID (D1), the value (D1), the
    source button, the pointer position, the modifiers, the timestamp and the sequence
    number of the raw event that produced it (Ends from the end queue carry the number of
    the call that offers them);
  - typed characters, by the rule of D6;
  - pointer moved (coalesced, below), pointer left and focus lost.
- **Not offered:** modifier holds (§7.4), analog presses of button actions (§7.5), analog and
  relative contributions to axis actions. They are computed in `update` from levels, not
  from events, and have no place in the event order. Plugins do not see their commands.
- The sink returns **Consumed** or **Not consumed** for every offer. The mapper uses only
  the verdict of a Start. The verdicts of an End and of a Repeat are ignored: a Repeat
  changes no hold, no ownership and no snapshot value. Inside the UI the same result decides
  whether the repeat goes on to the plugins' `KeyDown` broadcast (below).
- A consumed Start makes the hold **UI-owned**. The game snapshot does not show it. A button
  whose press produced **at least one hold, and all of them consumed** (UI-owned), is left
  out of the held set used to complete chords until it is released, so pressing both
  buttons over a panel does not run. A press that produced no hold stays in that set, as
  today.
- A Start that is not consumed makes a normal hold for game code. This is the same action,
  with no re-resolution (except the pass-through continuation of D4).
- **Every offered Start gets exactly one offered End.** Holds can end outside `handle_with`:
  `activate`, `deactivate`, `set_keymap`, run-time registration, `release_all`, and band
  pushes and pops made outside an offer (D5). Their Ends are put on an **end queue**. The
  queue is offered at the start of the next `handle_with` and by
  `InputMapper::offer_pending(sink)`, which the client calls
  once per frame after the last raw event and before `update` (with the one exception of a
  deferred cursor-left, "How the position is frozen" item 1 and D8 step 1). Ends that happen inside
  `handle_with` or `offer_pending` (queued band changes applied after an offer) are offered
  at once, after the offer that caused them (below).
- **Only offered Starts get Ends.** Each hold records whether its Start was offered. A hold
  made under `handle` (no sink) was never offered: its End is never offered and never put
  on the end queue, whichever call ends it (`handle_with`, `offer_pending` or a direct
  operation), and its OS repeats give no Repeat offers. Such a hold stays a game hold.
- **Applying band requests.** After every offer (an End from the end queue, the move, an
  event of the raw event, or an End offered under this rule) the mapper applies that
  offer's request list in order (D5) and then offers, in the order those holds began, the
  Ends of the offered holds that the applied requests ended. Each of these Ends is again an
  offer whose requests are applied the same way before the next event is offered. This
  ends, because each such End belongs to a hold that already ended and no hold is made
  outside resolution. A request can also end a hold of the raw event being resolved whose
  Start has **not been offered yet** (several bindings fired in one context and an earlier
  offer popped that context or pushed the first `opaque_keys` entry). Such a hold is
  **dropped**: neither its Start nor its End is offered, it never becomes UI-owned or a
  game hold, and it adds no press or release edge to the snapshot. A release-class pulse
  of that raw event whose context has left the stack before its offer is dropped the same
  way. So no End is offered before its Start, and no Start is offered for a hold that has
  already ended.
- **Repeat.** An OS auto-repeat press of a held key produces a Repeat for each of that key's
  offered holds that is repeatable. Repeatability is decided **per hold, by the command
  its Start carried** (D1), not per action, because one action can carry several commands:
  an imported entry keeps its own command, and the §12.15 rows of one action differ by
  context and by sign. With action-map data, a hold is repeatable when its command ID has
  toggle type 3 (AutoRepeat) in table A (`keymap.md` §12.10, D9); a hold whose command has
  no ID, or an ID not in the table, has type 0 and does not repeat. The client hands the
  mapper the set of command IDs of type 3 at start-up. Without action-map data, the
  action's built-in `repeat` flag (list item 13) decides. A Repeat never counts as a
  snapshot edge. The UI routes it like a
  Start: `Element_key_press` again and, if not consumed, `KeyDown` again (LPA §13.12.2 already
  expects a repeated start). Auto-repeat presses still never start or resolve holds.
- **Pointer moves are coalesced.** A cursor-position event only records the position and
  marks a move pending; it offers nothing. With a sink attached, a pending move is offered,
  at the last recorded position and with the timestamp of the last cursor-position event,
  **before the mapper
  resolves any other raw event** that can offer or change what is offered: every key, mouse
  button, gamepad button, wheel, typed-character, cursor-left and focus-lost event. This does
  not depend on whether that event will offer anything. It cannot, because whether an event
  offers anything depends on the band, and the band depends on the hover change that the
  move causes (UW §5.7): when the pointer moves onto a list and the wheel turns in the same
  frame, the move must be offered first so that the list's `ui.scroll_mouse` push is applied
  before the wheel event is resolved. Raw mouse motion, gamepad axis positions and the window
  system's modifier report do not flush a pending move. `offer_pending` also offers it. So a
  button, key or wheel event always follows a move to its own position, and a high-rate mouse
  costs at most one hit test and one `MouseMove` per other raw event plus one per frame.
- **Steps of one `handle_with` call.** In every step, each offer's band requests are applied
  when it returns, and the Ends they cause are offered at once ("Applying band requests"
  above), before the next offer.
  1. Offer the end queue, in order.
  2. If the raw event is one of the flushing kinds above and a move is pending, offer the
     move.
  3. Resolve the raw event against the band as it stands now, which includes every request
     applied in steps 1 and 2; this context order is fixed for the whole raw event (D5).
     Offer its events in the order of D4 as resolution produces them: a pass-through
     context's events are offered as soon as it fires, and resolution continues below it
     only after their verdicts (D4). The wheel-motion decision below is made here.
- **`offer_pending`** is steps 1 and 2 without a raw event: it offers the end queue and a
  pending move, with the same request handling.
- **Wheel motion.** A raw wheel event's `WheelX`/`WheelY` motion skips `pointer` and the game
  band when a UI-band context binds a wheel click of that axis (`WheelLeft`/`WheelRight` for
  X, `WheelUp`/`WheelDown` for Y). Its wheel clicks are resolved and offered as usual. With
  the defaults this means: over a scrollable or scroll bar (`ui.scroll_mouse` pushed) the
  wheel scrolls and does not zoom; elsewhere, including over a plain native panel, the
  wheel zooms the camera.
  - **Timing.** Today §7.5 routes accumulated relative motion at `update`. The new rule
    keeps that for routing, but decides per raw event whether the motion is **withheld**:
    when the wheel event is resolved (step 3), against the band at that moment. Withheld
    motion skips `pointer` and the game band when `update` routes it; motion that is not
    withheld is routed at `update` by §7.5 as today. So a `ui.scroll_mouse` entry pushed or
    popped between a wheel event and `update` does not change where that event's motion
    goes, and with no UI-band entry nothing is withheld (invariant (e)).
  - **Under `handle`.** The same per-event decision is made against the band as it stands,
    whoever pushed its entries (D5). So with `ui.scroll_mouse` active under `handle` the
    wheel's motion does not change `zoom`, and its `scroll_up`/`scroll_down` clicks appear
    in the snapshot as ordinary game holds (nothing is offered).
  - **Captures** get no special case. During a capture the hovered element is the captured
    element while the pointer is over it, and nothing otherwise (UB §4.4 step 2; UB §4.5
    only keeps *other* elements from becoming hovered). So when the pointer leaves the
    captured element, it gets `Element_mouse_over_top` with value 0, a scroll bar pops
    `ui.scroll_mouse` (UW §6.9) and a scrollable's entry is popped by the UW §5.7 Tessera
    rule; when the pointer comes back, the entry is pushed again. During a thumb drag in
    which the pointer leaves the captured thumb (for example off the bar), the wheel
    therefore zooms the camera until the pointer is back over the thumb.
- **Typed characters** given to the sink are not added to the snapshot's `text`, so nothing
  is typed twice. Characters that the D6 rule does not send to the UI go to the snapshot as
  today.
- The UI's consumption rules stay in `tessera-ui`:
  - A key command is consumed when the target or a listener consumes it (UB §5.3).
  - A mouse-button command is consumed when the press had a hit element or a capture
    (UB §4.2, §4.5). This is Tessera's rule, so that clicks on a panel background with
    `blocks_clicks` do not fall through to the world.
  - A wheel command is consumed when a scroll host stopped its message (UB §4.7).
  - Toggle types come from the client (D9). An Impulse action's End is ignored (LPA §13.13).
  - The sink's verdict is the UI's, with one exception made by the client: the `mouselook`
    Start during an item drag (below).
- **Pointer lock** (UB §4.9) is the client's state. The client, as the sink, sees the
  `mouselook` Starts and Ends in order.
  - **On, after the verdict.** `mouselook` (`CameraInstantMouseSteer`) is a key command, so
    the UI routes it to the keyboard target, which needs no hit test (UB §5.3). The client
    routes a `mouselook` Start first. If the UI consumed it, nothing turns on. If the UI did
    **not** consume it, the client checks for an item drag (next bullet); with none, it
    turns the lock on: it tells `tessera-ui` that the lock is on, with the frozen position
    *F*, hides and grabs the cursor (below), and then returns Not consumed. A consumed
    `mouselook` Start, whoever consumed it, turns nothing on and is an ordinary UI-owned
    hold, so the camera does not turn.
  - **During an item drag.** "An item drag" here means that a drag icon exists, from its
    capture (UB §7.2 step 4) to the clean-up (UB §7.4 step 3); `tessera-ui` tells the
    client. The UI itself consumes a `mouselook` Start in one case only: the icon was
    created by the source's container (UB §7.1 step 5, §7.4 step 4) **and** there is a
    keyboard target, a focus element or an active panel (UB §5.3 steps 1 and 4: only key
    commands sent to UI elements are swallowed). It declines the Start in the other drags:
    when a game widget supplied its own icon (UB §7.1 step 4 names inventory slots, quick
    slots and plugin controls), and when there is no keyboard target (UB §5.3 step 3), which
    is the case after the drag's left press on a root panel that is not activatable, or on
    no root panel (UB §4.6 step 3 then leaves no panel active). Read literally, the specs
    would then turn the lock on mid-drag: UB §4.9 hit-tests nothing and hovers nothing, so
    the drop target of UB §7.3 is lost and the left release finds no hit element to drop
    on (UB §7.4 step 1). **Tessera rule** (UB §7.4 OPEN: whether a right click cancels a
    drag): when the UI declines a `mouselook` Start while a drag icon exists, the client
    does not turn the lock on and returns **Consumed** itself. The UI's routing of that
    Start is unchanged: the keyboard target's key handler ran if there was one, and plugins
    got `KeyDown` (UB §5.3 step 3, LPA §13.13 step 3), as the command went on to game code;
    the client, as game code, ignores it. The hold is UI-owned, so the camera does not
    turn, and the run chord cannot complete either, because the left press that began the drag
    produced a UI-owned hold and no other (the chord rule above). So in every item drag a right press neither turns the camera nor
    locks the pointer, the cursor stays visible and hit-tested, and the drop at the left
    release reaches its target. A consumed Start is never re-examined: if the drag ends
    while right is still held, the camera stays still until right is pressed again (Known
    difference 20).
  - **Off, after the last End.** The lock stays on while at least one `mouselook` hold for
    which it was turned on exists. The client routes each `mouselook` End. After the End of
    the last such hold, it turns the lock off (tells `tessera-ui`, then releases the grab as
    below). The End of a hold that did not turn the lock on (a consumed Start) changes
    nothing.
  - Each offered event counts as "the next input event" of UB §4.9. `mouselook` starts on
    every right press that no higher context takes and that neither the UI nor (during an
    item drag) the client consumes, also over a panel. During the lock the UI hit-tests nothing and nothing is hovered, so a
    scrollable's `ui.scroll_mouse` entry is popped and the wheel zooms during mouse-look.
  - **Mouse-button commands during the lock** (Tessera rule; question for UB §4.9 in the
    owners list). A Start of a mouse-button command reaches no element and is declined,
    even while a capture exists (UB §4.9). The End of a command that is in the capturing
    control's pressed set is still applied: LPA §13.12.2's release steps 1, 2 and 4 run, so
    the capturing control gets `MouseUp` and the capture ends when its set empties. Step 3's
    refresh finds no hover target during the lock, so no `MouseClick` or `MouseDoubleClick`
    follows. Example: left held on a panel element (captured), right pressed (`mouselook`,
    lock on), left released: `MouseUp` to that element and the capture ends. So no capture
    or pressed set outlives its buttons.
- **How the position is frozen.** Today the walker neither grabs nor hides the cursor
  (`apps/client` has no cursor grab), and offered events carry the mapper's last cursor
  position, which an unlocked OS cursor keeps changing. With a UI attached the client does
  both of the following, so the frozen report of UB §4.9 holds on every platform:
  1. **Frozen mapper position (always).** *F* is the position carried by the `mouselook`
     Start, which is the press position (the pending move was flushed before the press).
     While the lock is on, the client feeds the mapper **no cursor-position and no
     cursor-left events**. A cursor-left would clear the mapper's position (KM §7.1; today's
     mapper sets it to unknown), so later offers, the `select_right` pulse included, would
     carry "unknown" instead of *F*. It can arrive during the lock on Wayland, where pointer
     focus is separate from keyboard focus. Raw mouse motion still reaches the mapper for
     mouse-look, and focus-lost events are fed as usual. So the mapper's position stays *F*.
     Every event offered
     during the lock, and every event of the call whose End turns the lock off, carries
     *F*. No pointer move becomes pending, and D3's nearby test compares presses with *F*.
     This holds whichever call offers the turning-off End: `handle_with` of a release or of
     focus lost, or `handle_with` and `offer_pending` offering an End from the end queue
     (after `set_keymap`, `deactivate`, a band pop or `release_all`). No cursor position is
     fed when the lock ends. The next cursor-position event the OS reports gives the real
     position, which is *F* when the warp below worked or the grab was `Locked`. One
     withheld report is not dropped: if the last pointer report the client withheld during
     the lock was a cursor-left, the client feeds that cursor-left as the next raw event
     right after the call that turned the lock off returns (before `update` when that call
     was `offer_pending`, which is the one exception to "`offer_pending` comes after the last
     raw event" in D8 step 1). It is offered as "pointer left" after the turning-off End and its
     `select_right` pulse, which still carry *F*, and the UI then hovers nothing until a
     position inside the window arrives (UB §4.9 step 2). Every offered event
     also carries the sequence number of the raw event that produced it (the number of the
     offering call for Ends from the end queue), so the client can tell which raw event an
     offered event belongs to.
  2. **OS lock (where available).** When it turns the lock on, the client hides the cursor
     and asks winit for `CursorGrabMode::Locked` (available on Wayland, including Hyprland,
     and on macOS). Where winit refuses it (X11, Windows), the client asks for `Confined`.
     Mouse-look keeps reading raw mouse motion, as today. When it turns the lock off, it
     puts the cursor back at *F* where the platform allows (`set_cursor_position`; on
     Wayland only while the grab is still `Locked`, so before releasing it), releases the
     grab and shows the cursor. (With the software cursor in use, `ui_cursor = software` of
     `0015-ui-rendering.md` §2.5, "hides" means the software cursor is not drawn and
     "shows" means it is drawn again; the OS cursor stays hidden throughout that mode.) If the warp is refused (for example after focus loss, or on
     a platform that forbids it), the cursor reappears where the OS has it. The UI has seen
     *F* until then, and the next OS cursor-position event moves it to the real position
     (the recorded difference of `0015-ui-rendering.md` §2.5).

  Without a UI attached the client may keep today's ungrabbed mouse-look.
- So a quick right click on a panel still works: at the release the `mouselook` End comes
  first (D4), the lock ends, and then the `select_right` pulse is hit-tested at *F*, the press
  position. A long right drag gives no tap, so turning the camera never opens a context menu.

**D8. Frame loop and budget.**
The steps are those of ARCHITECTURE §20.3, reworded as the owners list says.

- **Step 1, input.** The offers, the UI's input routing and the plugin input handlers
  (`MouseDown`, `MouseUp`, `MouseClick`, `MouseDoubleClick`, `MouseMove`, `MouseEnter`,
  `MouseLeave`, `MouseWheel`, `KeyDown`, `KeyUp`, `FocusGained`, `FocusLost` and the
  notifications they cause) run here. The client calls `handle_with` for each raw event,
  then `offer_pending`, then, only when that `offer_pending` call offered the End that
  turned the pointer lock off and a cursor-left was withheld during the lock, `handle_with`
  for that cursor-left (D7, "How the position is frozen" item 1), and then `update`. This
  is what LPA §3.3 and §9.7 require: input is delivered when the client processes input,
  outside the UI tick, and the events an input causes are delivered before input
  processing continues.
  - **Where step 1 happens in the event loop.** The client feeds raw events to the mapper
    from winit's `window_event` and `device_event` callbacks as they arrive, as the walker
    does today (`apps/client/src/app.rs` calls the mapper there, before
    `RedrawRequested`). Each raw event's timestamp is taken on arrival, in that callback,
    and is the one D3's timing and the offers use. The work done in those callbacks (the
    mapper's resolution, the offers, the UI's routing and the plugin input handlers they
    run) belongs to step 1 of the **following** frame, the one that the next
    `RedrawRequested` draws, and counts toward that frame's U1 and plugin time. That
    frame's redraw begins with `offer_pending` (and the deferred cursor-left, if any) and
  `update`.
- **Step 7, the UI tick and the plugin host**, in the order of LPA §3.3 (P48, P51):
  1. if the pointer needs re-evaluating, the hit test and the hover refresh (LPA §13.11),
     with their `MouseLeave`/`MouseEnter` and the hover pushes and pops of
     `ui.scroll_mouse` (D5);
  2. the hover check, which may deliver `MouseHover` (LPA §13.12.5);
  3. the plugins' `Update` broadcast (LPA §9.4). Plugin timers are `Update` handlers that
     read `Turbine.Engine.GetGameTime()`; the API has no other timer (LPA §9.4, §12);
  4. the rest of the UI's per-frame work, except layout and paint, which are step 8.

  Then, in this order:
  5. the plugin host's deferred work (queued `LoadPlugin` requests, asynchronous
     `PluginData.Load` handlers, plugin-data writes and their `onDone` callbacks, the
     **game-raised plugin events** and the **logout unload**, both below). This
     follows the Tessera order of LPA §3.3 OPEN: input, then the UI tick, then the deferred
     work, then rendering. **It is sliced by whole items** (Tessera rule): the host runs
     the queued items in queue order and starts no further item once the deferred work of
     this frame has taken 2 ms (a Tessera constant, measured as plugin time); the rest
     waits for the next frame's step 7. At least one item runs in each frame, and an item
     is never interrupted (each of its host calls has its own instruction budget). Because
     the queue order is kept, load order (LPA §6.2) and per-key `PluginData` order
     (LPA §10.6) are unchanged; the specs only say "later" and "between frames";
  6. the collector step: `lua_gc` with `LUA_GCSTEP` and 20 in every apartment, once per
     tick, whatever the deferred work left queued (`plugin-runtime.md` §9.2, whose Tessera
     choice places it "after the tick's event dispatch and deferred plugin-host work,
     before rendering"). It is plugin time;
  7. last, the client drains the band-request outbox into the mapper (D5).
- **Game-raised plugin events** (Tessera rule). Some plugin events are raised by game or
  session state, not by input or by the tick: `Turbine.Chat.Received` (LPA §11.3) for every
  chat line the client shows, whether it comes from a session event in step 2 or is one of
  Tessera's own system notices (which the client posts at the start of its redraw), and the `…Changed` events of the gameplay views (LPA §15), which the session
  layer feeds. The code that raises such an event (session code, the chat window's line
  intake, the gameplay views) never calls Lua. It appends the event, with its argument
  values taken at that moment, to the plugin host's deferred-work queue, and the host
  delivers it in step 7 item 5, in queue order, one queued event being one item, to the
  handlers set at delivery (an apartment closed in between gets nothing). The
  native side (the chat window's own log, for example) is not delayed; only the plugin
  delivery is. A getter called in such a handler returns the value at delivery, which may
  already include a later change made in the same frame (Known difference 21). An event of
  this kind raised while plugin code runs (in step 1 or 7) is queued the same way, so it
  never re-enters Lua from inside a handler; events that are UI notifications keep the
  delivery of LPA §9.7.
- **Unload at logout and at client exit** (LPA §6.3, `plugin-runtime.md` §8.3). An unload
  that a plugin or the user asks for (`PluginManager.UnloadScriptState`, `/plugins unload`,
  the manager's unload action) runs where the request is made: in a plugin call, or in the
  step-1 routing of the chat line or click that asked for it, so in step 1 or 7. The
  **logout** unload (every apartment) is queued by the session code that reports the logout
  and runs in step 7 item 5 as one item per apartment, in queue order and sliced like the
  other items, so writes queued before it keep their order. The **client-exit** unload is the one
  exception to the steps: when the event loop exits (winit's exit callback), after the
  last frame has been presented and before the renderer and window are destroyed, the host
  unloads every apartment on the main thread (the `Unload` handlers, then the state close
  with its synchronous `PluginData.Save`, LPA §10.6). Every handler call there keeps its
  instruction budget. No frame follows, so the frame-time rule does not apply to it.
- **"Between frames".** LPA §3.3 and `plugin-runtime.md` §10.3 say that all plugin code runs
  on the main thread "between frames" (LPA citing ARCHITECTURE §18.1). This record reads
  that as: on the main thread, in frame-loop step 1 (input handlers) and step 7 (`Update`,
  deferred work including game-raised events and the logout unload, collector step), with
  one exception, the unload at client exit after the last frame; never during steps 2–6 or
  8 onward (session, simulation, layout, paint and rendering), and never on another
  thread. The owners list asks for that wording.
- **Step 8, layout and paint.** Layout, painting and the composition list, as
  `0015-ui-rendering.md` describes them. Its layout changes no box that a hit test
  reads (below).
- **What the order means.** `MouseHover` comes before `Update` in a frame, as in LPA §3.3.
  An `Update` handler that moves or shows a control under a resting pointer does not get a
  `MouseEnter` in the same frame. It gets one at the next hit test, which is the next input
  event of step 1 or the next frame's step-7 refresh (if that change marks the pointer for
  re-evaluation, which is `tessera-ui`'s rule, LPA §3.3). This is the original's order.
  Input events, which belong to step 1, always come before that frame's tick.
- **Geometry used for hit tests.** The hit tests of step 1 and the hover refresh of step 7
  use every element's **current** geometry: its position, size, visibility and z-order as
  they stand when the hit test runs. A change applies to the element tree at once,
  whoever makes it: a plugin handler's `SetPosition`, `SetSize`, `SetVisible` or z-order
  call (LPA §9.7: "the size itself changes at once"; LPA §13.11 tests the controls'
  rectangles), a drag-bar move or a resize of a native panel (UB §8.1, §8.2), and the
  re-layout of the children of an element whose size changed, which UL §8.4 runs
  "whenever that element's size changes". `tessera-ui` may defer that re-layout for speed
  only as long as no hit test reads an element it would move: before a hit test, every
  pending re-layout is completed. Step 8 paints from the same geometry and changes no box
  that a hit test reads. So an event's `X`/`Y` are relative to the control's current
  position (LPA §13.12.3), and the common plugin pattern of dragging a window with
  `MouseMove` and `SetPosition` works with several moves in one frame: the second
  coalesced move (one flushed before a key or wheel event, then the `offer_pending` move)
  is hit-tested against the box the first one set, so the capturing control gets no
  spurious `MouseLeave` or `MouseEnter`, and its `X`/`Y` are relative to where it now is.
  What is drawn may lag by a frame or more (the companion's paint jobs), which is the
  companion's recorded difference "hit testing during a transition".
- **Budget.** U1 of `0015-ui-rendering.md` §6 (its performance budget; steady state,
  outside the content-event frames that its U4 judges) counts Tessera's own work inside the
  offers (hit tests, routing, native element handling) and in the step-7 tick. Time inside
  plugin Lua calls is measured separately as plugin time. That covers the input handlers of
  step 1 (wherever in the event loop they run, above), and the `Update` broadcast, the
  deferred work (game-raised events and the logout unload included) and the collector step
  of step 7. The client-exit unload is plugin time too, outside any frame. Plugin time is not part of U1 or of the U4 paint slice, but it counts toward the
  frame-time rule (no frame over 50 ms). Each plugin handler call is one host call with its
  own instruction budget (500 million instructions, `plugin-runtime.md` §10.1, LPA §3.3).
  Input handler calls are never deferred, dropped or cut by a per-frame slice, because
  their order and the next event's hit test depend on them. The pointer-move coalescing of
  D7 bounds how many there are. The `Update` broadcast is not cut either, because LPA §9.4
  calls every subscriber in every tick, and neither is the collector step, which
  `plugin-runtime.md` §9.2 requires once per tick. Only the deferred work is sliced (step 7,
  item 5), because nothing orders it against the frame. The scripted U1 run adds a 1000 Hz pointer sweep to
  measure this.

**D9. Toggle types for the UI.**
- The client reads table A of the default action map (DID map ACTIONMAP, key 1;
  `keymap.md` §12.2, §12.10) at run time and gives `tessera-ui` a table from command ID to
  toggle type. Tessera's reading of "the action map of the input context in force"
  (LPA §13.12.1) is this map: the original's key-map code uses key 1 only (KM §12.2, BIN).
- An ID not in the table, or an event without an ID, has type 0, which the UI treats as
  Momentary (LPA §13.12.1). Without action-map data every type is 0. Then a `tap_*` pulse
  gives plugin button events that the original would not give; this affects only runs
  without an install, where no plugins run.
- The same table decides which holds repeat: a hold repeats when the command ID its Start
  carried has type 3 (D7, "Repeat"). The actions' built-in `repeat` flags are used only
  without action-map data.

## Consequences
- **Effects on other code.** Game code reads one snapshot as before; UI-owned holds are simply
  absent from it. The walker, tests and tools that call `handle` see no change for any
  existing action; `pointer` is inactive unless the client activates it, and even when it is
  active, `handle` consumes nothing in it (there is no sink, and a pass-through context
  consumes only on a UI verdict, D4, `double` matches included), so it hides no lower
  binding. With a sink, `pointer` hides a lower binding only when the UI consumed the press
  (next entry). The one existing context whose behaviour changes is `text_entry`, which no
  client code activates today. Under `handle`, after `activate(text_entry)`:
  - keys that it does not bind are still swallowed (`opaque_keys`), typed characters still
    reach the snapshot, and game-band modifier holds are still not evaluated;
  - mouse buttons, the wheel and analog axes now pass it (`opaque_keys` instead of
    `opaque`), so `mouselook`, the run chord, `zoom` and `look_*` work while typing;
  - activating it releases only the game-band holds whose last stroke has a key or a pad
    button (D6), where today it releases every hold below it;
  - Enter and NumpadEnter give `open_chat`, Escape gives `menu`, and Up/Down give
    `text_line_up/down`, in place of the retired `text_submit`, `text_cancel` and
    `text_history_prev/next` (D2). `StandardActions` loses those three fields and gains
    the new ones.

  Invariant (e2) pins this. `tessera-ui` gets an ordered, positioned
  command stream with names and IDs and decides consumption as UB and LPA describe. Plugins
  get `KeyDown`/`KeyUp` with the IDs of §12.15. Imported UI sections and tap/double bindings,
  dropped today, get a Tessera form.
- **What the owner will notice in the walker with a UI attached** (the normal client):
  - Over the world, nothing changes for `mouselook`, run and the other mouse bindings,
    including releasing right and quickly pressing it again to keep turning the camera, and
    a quick second left press while right is held (D4, invariant (g)).
  - Over a panel element (the pointer frozen there at the first right press, D7), a quick
    right re-press within *T* is a double right click on that element: the UI consumes it
    and `mouselook` does not start. A further press within *T* is again a double press
    (D3's third-press rule), so a camera turn that began over a panel can be resumed only by
    a press more than *T* (500 ms by default) after the previous one. This is the one
    place where `pointer` hides a game binding on a quick re-press (Known difference 14).
  - Left pressed first over a panel element never starts the run chord: the left button is
    UI-owned, so right pressed afterwards gives `mouselook` only (D7). Right first, then
    left, runs anywhere, because the locked pointer hit-tests nothing.
  - During mouse-look the cursor is hidden and locked (D7, "How the position is frozen").
  - During any item drag a right press neither turns the camera nor locks the pointer, and
    the drop still lands where the cursor is. The UI swallows the press itself only when the
    container made the drag icon and a panel is active or an element has focus (UB §5.3
    step 4). In every other drag (from an inventory slot, a quick slot or a plugin control,
    or with no active panel) it is a Tessera rule: the client swallows it, because the
    specs would otherwise lock the pointer mid-drag and lose the drop target (D7, Known
    difference 20). What the original does there is an owner observation.
  - Mouse wheel during a drag of a scroll-bar thumb: once the pointer leaves the thumb, the
    wheel zooms the camera instead of scrolling, until the pointer is back over it (D7,
    "Captures"). This follows from UB §4.4 and UW §6.9.
  - While the chat window is the active panel, PageUp, PageDown, Ctrl+Home and Ctrl+End
    page its log (`ui.chat_log`, D5) instead of zooming or resetting the camera.
- **Escape with a UI attached.** Today the walker turns `menu` into quit
  (`ControlEvents::quit` in `apps/client/src/walker/controls.rs`). Under Option 4 every
  declined Escape reaches `menu`: a focused native editable field without
  `lose_focus_on_escape` (only 19 elements set it, against 103 with `editable` true, so at
  least 84 editable fields lack it; UB §5.4 census), a focused plugin `TextBox` (Escape is not
  an editing action under LPA §13.13's Tessera rule), an active panel that does not handle
  `EscapeKey`, and Escape with no keyboard target at all. The last bullet of the worked
  example would quit the client. So when a UI is attached, the client's reaction to `menu`
  must become game code's handling of `EscapeKey` (for example: close the active panel, or,
  with none, open the game menu), never an immediate quit. The exact game-code behaviour is
  not specified in the cited specs; it belongs to the M5 client and `game-panels.md`. Without
  a UI the walker may keep quit on `menu`.
- **While `ui.dialog` is pushed**, the arrow keys produce `focus_*`. If the dialog declines
  them they go on to game code unchanged (Option 4), and game code does not react to
  `focus_*`, so the arrows do not turn the character. This is the original's behaviour as
  far as UB §5.3 and §5.6 describe it. With the D5 rule only engine dialogs push
  `DialogBoxes`, so this happens only while such a dialog is active.
- **Known differences from the original:**
  1. Order of consultation. Tessera consults the UI band and then the game band, and the
     first context with a press-class match consumes the press (except pass-through). The
     original's order across maps, and the effect of `IgnoreRemainingMaps`, are OPEN
     (§12.12).
  2. Inside one context, every matching complete binding fires (§7.2 step 3). §12.12 picks
     one best binding per map and event. Whether the original fires one binding, or one per
     activation bit (the LPA example needs both select and double-select at the second
     press), is OPEN.
  3. Tessera's exact and subset policies replace the best-match rule (§12.14.6).
  4. Keyboard opacity of `text_entry` is Tessera's mechanism. How the original keeps typed
     keys from reaching movement is unknown.
  5. Withholding wheel motion follows the UW §5.7 push, with Tessera's reading that the
     first map that binds the wheel takes it (§12.12 OPEN). Plugin controls push by a
     Tessera rule (D5 OPEN).
  6. Analog sources, analog presses of button actions and modifier holds are not offered to
     the UI, so plugins never see their commands.
  7. Pads produce no UI repeat (no OS repeat), and pad UI navigation has no defaults. Key
     repeat uses the operating system's delay and rate; the original's repeat timing is
     unknown (LPA §13.12.2 OPEN).
  8. The modifier-click commands `DressingRoom`, `AutoLootAll`, `ItemLinkToChat` and
     `MultiUseItem` (UB §3.1) have no Tessera action yet. Until they do, those clicks give
     `select_left`/`select_right` (Subset policy). When the game panels add them, they belong
     in `pointer` with exact modifiers, where §7.2's exact pass prefers them.
  9. With right held first, a left press fires both `select_left` (pass-through) and the run
     chord. If both bindings sat in one map and the original picked one binding per map,
     §12.12 would give run alone (more mask bits); like difference 14, this is inferred,
     not established. The UI ignores the
     `select_left` because the pointer is locked; game code that later reacts to
     `select_left` (target selection) must ignore it while `mouselook` is held.
  10. `Scroll` (0xAB, UB §3.1) is never produced, because analog and relative axes are never
      offered. The plugin UI ignores it (LPA §13.13) and scroll hosts do not handle it
      (UB §4.7), so no known behaviour depends on it.
  11. The double-click time and rectangle come from Tessera's settings where the platform
      has none (D3), which includes the reference machine.
  12. Without action-map data, `tap_*` pulses give plugin button events (D9).
  13. IME composition is still not covered (§11 OPEN, UB §5.4 OPEN).
  14. **Quick second press of a mouse button** (inferred, not established). A second press
      within *T* (and nearby, which during mouse-look is always so, since the position is
      frozen) fires the double-select action in `pointer` and, unless the UI consumes it, the
      game binding below (`mouselook` on right, the run chord on left with right held). If
      the original consulted `SelectDblRight` (command 0x2E, UB §3.1; its default binding
      has the `MouseDblClick` activation, 0x60) and `CameraInstantMouseSteer` in the
      same input map and picked one binding per map (§12.12), it would take the double-select
      alone and not steer, even over the world. None of the cited specs establishes that:
      §12.14.2 places `CameraInstantMouseSteer` in `Main`, but no cited spec gives the
      activation of its default binding, and both map order and one binding versus all are
      OPEN (differences 1 and 2). The run case also depends on how the original binds
      run (a meta-key mask with the right button, §12.14.4 rule 2, is one possibility).
      Tessera therefore does not claim that a double-select swallows the press "as in the
      original" (revision 1 did). It consumes only on a UI verdict, so the walker keeps
      today's behaviour over the world; over a panel element a quick right re-press is a
      double click on it and does not start `mouselook`. **Owner observation:** in the
      original, release the right button during mouse-look and press it again at once
      without moving the mouse, over the world: does the camera keep turning? Same with a
      quick second left press while right is held: does running continue? If the answer is
      no, the rule can be switched to "double-press matches consume" (Options 6, last row),
      at the cost of that walker effect.
  15. **Pointer moves are coalesced** (D7). Plugins get at most one `MouseMove` per other raw
      event plus one per frame, instead of one per OS pointer movement, so a handler that
      counts moves or traces the path sees fewer, coarser steps. LPA §13.12.5's 0.25 s
      hover rule measures on the UI's frame clock from the time the UI notes a movement
      (LPA §13.12.3); with coalescing that is the time the move is offered (at the latest in
      the same frame's step 1), not the time of the last OS movement, although the offered
      move carries the latter's timestamp. So `MouseHover` may start up to one frame later.
      No known behaviour depends on either.
  16. **Mouse-button Ends during the pointer lock** reach the capturing control (D7, Tessera
      rule). UB §4.9 says only that button commands reach no element during the lock. Read
      literally, a capture made before the lock would then outlive its button.
  17. **Paging the chat log without entry focus** uses the Tessera context `ui.chat_log`
      (D5). Which input map supplies those commands in the original is not specified.
  18. **A focused plugin `TextBox` misses keys that no UI-band context binds.** LPA §13.13
      step 2 has a focused TextBox fire `KeyDown` (and `KeyUp`) for every key action it is
      offered. With `text_entry` pushed (`opaque_keys`, D6), a key that no UI-band context
      binds is swallowed before it becomes an action, so it is never offered: the TextBox
      gets no `KeyDown` for it, and neither does the plugins' broadcast. Examples: Tab
      outside an engine dialog (`ui.dialog` is not pushed), the F-keys, and the quick-slot
      keys (which have no Tessera action yet anyway, "Still OPEN"). How the original keeps
      typed keys from reaching movement is unknown (difference 4), so whether it offers
      such keys to the TextBox is unknown too.
  19. **Modifier policies of the UI actions** (D2). The caret, delete, backspace and scroll
      actions are Subset, as the original's subset matching suggests (§12.12). Where two
      Subset bindings of one context both match and neither matches exactly, both fire
      (KM §7.2 step 3), where §12.12 picks the one with more mask bits: Ctrl+Alt+Left gives
      both `text_char_left` and `text_word_left`. The exact Ctrl+Shift defaults of the word
      and document moves cover the common case. The focus and clipboard actions are Exact,
      so Ctrl+Tab and Ctrl+Shift+C give no command (while a field has focus they are
      swallowed by `opaque_keys`), where §12.12 would give `FocusNext` and `CopyText`.
  20. **Right press during an item drag** (Tessera rule, D7). UB §5.3 step 4 swallows the
      `mouselook` Start only for a container-made icon with a keyboard target. In every
      other drag the client consumes it, so the camera does not turn and the pointer does
      not lock. Read literally, the specs would lock the pointer and lose the drop target
      (UB §4.9, §7.3, §7.4 step 1). Whether a right click cancels a drag in the original is
      UB §7.4 OPEN. A consumed Start is not re-examined when the drag ends, so a right
      button held across the drop does not start turning the camera until it is pressed
      again. **Owner observation:** in the original, drag an item from a bag and from a
      quick slot, press right during the drag and release left over another slot: does the
      camera turn, is the drag cancelled, and does the item land?
  21. **Game-raised plugin events are queued to step 7** (Tessera rule, D8).
      `Turbine.Chat.Received` and the gameplay views' `…Changed` events reach plugins in
      step 7 of the frame in which they were raised (or a later frame, when the deferred
      work's 2 ms slice is used up), not at the moment the state changed. LPA §15 says only
      that an `XChanged` event "fires after the value changed", which still holds. Two
      changes of one value in one frame give two events in order, but a getter called in
      the first handler already returns the second value. When the original delivers these
      events is not stated in the cited specs (LPA §3.3 OPEN covers only the deferred work);
      no surveyed behaviour is known to depend on it.
- **Still OPEN, carried over:** which panels push `DialogBoxes`/`EditControls`/
  `CopyAndPasteControls` (UB §5.2, §5.6; D5 gives Tessera rules); which plugin controls push
  `ScrollableControls_Mouse` (D5); the pop path of that entry (UW §5.7); what
  `IgnoreRemainingMaps` does; mapping the game-panel maps once those panels exist; importing
  commands that have no Tessera action (quick slots and the like) so that plugins see their
  IDs on `KeyDown`. That last item would need generic command actions, for example a
  `commands` context. It is not decided here. New with revision 2: the owner observation of
  difference 14 (quick re-press during mouse-look in the original). New with revision 3:
  mouse-button Ends during the lock (difference 16) and the map behind the chat log's paging
  commands (difference 17). New with revision 4: a right press during an item drag
  (difference 20, owner observation). New with revision 5: when the original delivers
  game-raised plugin events (difference 21).
- **Owners of other specs and records.** For their owners, not edited here:
  - LPA §13.12.2: its two "Tessera" choices for start and end order and for taps follow D4
    (D4's order, `press` before `double` before `double_nearby` and then registration
    order, replaces "the order of the bindings in the effective key map"); its rule for
    types 3–5 (one start at the press, one end at the release) agrees with D7, and D4's
    once-per-binding rule keeps a second Start of the same action from turning into a
    release at its step 3.
  - LPA §13.12.3 and §13.12.5: pointer-move coalescing and its effect on `MouseMove` and the
    hover clock (difference 15).
  - LPA §13.13 OPEN on text-box consumption: D6 and D7. Its step 2 (a focused TextBox fires
    `KeyDown` for every key action it is offered) is narrowed by `opaque_keys`
    (difference 18).
  - LPA §3.3 and §9.4: agreement. D8 keeps the tick order (hover refresh, `MouseHover`,
    `Update` broadcast, the rest) and the Tessera order "input, then the UI tick, then the
    plugin host's deferred work, then rendering". LPA §3.3's "All plugin code runs on the
    main (UI) thread, between frames (Tessera architecture §18.1)" should read "between
    frames" as D8 does: on the main thread in frame-loop steps 1 and 7 (step 7 including
    the game-raised plugin events and the logout unload, which are deferred work), except
    the unload at client exit, which runs after the last frame (LPA §6.3); never during the
    session, simulation, layout, paint or rendering steps. Its list of deferred work gains
    the game-raised events (`Turbine.Chat.Received`, the gameplay views' `…Changed` events,
    §11.3 and §15; Known difference 21) and the logout unload. Its "Tessera may do the same
    or rely on the default collector" is already superseded by `plugin-runtime.md` §9.2,
    which D8 follows.
  - LPA §6.3 and `plugin-runtime.md` §8.3: agreement on the unload steps; D8 adds when they
    run (in the plugin call or step-1 routing that asked, in step 7 at logout, after the
    last frame at client exit).
  - LPA §9.7 and §13.11: agreement. Hit tests read the current geometry (D8), so a
    handler's `SetSize` or `SetPosition` affects the next event's hit test, as "the size
    itself changes at once" implies.
  - LPA §11.3 and §15: game-raised events reach plugins in step 7 (D8, Known difference
    21). A question for its spec writer: is anything known about when the original
    delivers them?
  - `plugin-runtime.md` §10.3 ("All plugin code runs on the client's main thread, between
    frames"): the same reading of "between frames" (D8), with the same client-exit
    exception. §9.2: D8 places the per-tick
    collector step in step 7, after the deferred work and before the outbox drain, which is
    its Tessera choice ("after the tick's event dispatch and deferred plugin-host work,
    before rendering"); agreement.
  - UB §3.3: the platform values reach the mapper through the client (D3).
  - UB §4.6: Tessera's consumption rule for mouse-button commands (D7).
  - UB §4.9: the lock state reaches the UI through the client, and only for a `mouselook`
    Start that neither the UI nor, during an item drag, the client consumed (D7). A question for its spec writer: are the Ends of
    mouse-button commands in a capture's pressed set delivered during the lock? D7 says yes
    (difference 16).
  - UB §5.3 step 4 and §7.1 step 5: a `mouselook` Start is consumed by the UI only during a
    drag whose icon the container created, and only when there is a keyboard target. No
    change is needed there; the interaction is recorded here (D7).
  - UB §7.4 OPEN (whether a right click cancels a drag): D7's Tessera rule, in which the
    client consumes the `mouselook` Start during every other item drag, so the drag goes on
    and no lock starts (difference 20). A question for its spec writer.
  - `walker.md` §5: "Escape quits" holds only without a UI attached. With a UI, `menu`
    becomes game code's handling of `EscapeKey` (Consequences, "Escape with a UI attached").
  - `chat-window.md` §6.2: the commands that page the log without entry focus come from
    `ui.chat_log`, which the chat window pushes while it is the active panel (D5,
    difference 17). Without that push, PageUp and PageDown would reach `zoom`.
  - UB §5.2 and §5.6 OPENs: the Tessera push rules of D5.
  - UW §16, row on `ScrollableControls_Mouse`: agreement; D5 and D7 make the wheel actions
    active the same way (scrollable and scroll-bar pushes at priority 0, UW §5.7 and §6.9;
    one wheel step is one line step, UW §5.6 and §6.8).
  - `chat-window.md` §10 item 5: its mapping `text_submit` = `AcceptInput`, `text_cancel` =
    `EscapeKey`, `text_history_prev/next` = line up/down becomes `open_chat`, `menu` and
    `text_line_up/down` (D2, list item 5).
  - `game-panels.md` and the M5 client: game code's handling of `EscapeKey` (Consequences,
    "Escape with a UI attached").
  - ARCHITECTURE §16: the pipeline gains the UI band and the sink. The sibling rules are
    unchanged, because `input` and `ui` stay independent.
  - ARCHITECTURE §20.3 (D8):
    - Step 1 becomes "Pump window and input events; the mapper resolves them and offers
      them to the UI in order (UI input routing and plugin input handlers run here, against
      the elements' current geometry; events that arrive between redraws are handled in
      winit's callbacks on arrival and belong to this step of the next frame); offer
      pending Ends; publish the action snapshot".
    - Step 7 becomes "UI tick (LPA §3.3): hover refresh, `MouseHover`, the plugins'
      `Update` broadcast (plugin timers are `Update` handlers), the rest of the UI's
      per-frame work; then the plugin host's deferred work, sliced by whole items (it
      includes the plugin events raised by game and session state and the logout unload);
      then the collector step in every apartment (`plugin-runtime.md` §9.2); then apply the
      UI's band changes to the mapper". Only the deferred work is sliced (D8, budget).
    - Step 8 becomes "UI: layout, paint damaged rectangles within the UI slice, build the
      composition list". The paint wording is `0015-ui-rendering.md`'s; see
      "Coordination". "Layout" there changes no box that a hit test reads (D8, "Geometry
      used for hit tests").
    - A sentence after the list: "At client exit, after the last frame, the plugin host
      unloads every apartment (LPA §6.3); this is the only plugin code outside steps 1
      and 7".
  - ARCHITECTURE §18.1: "all plugin code runs on the main thread in a per-frame time slice"
    needs the exceptions of D8. All plugin code runs on the main thread in frame-loop steps
    1 and 7, except the unload of every apartment at client exit, which runs after the last
    frame (this is what LPA §3.3 and `plugin-runtime.md` §10.3 call "between frames").
    Plugin events raised by game or session state are queued and delivered in step 7.
    Input handlers (step 1), the `Update` broadcast and the collector step (step 7) are
    bounded by the per-call instruction budget, are not cut by a slice, and are measured as
    plugin time. The deferred work of step 7, which includes the game-raised events and the
    logout unload, is sliced by whole items (2 ms per frame, at least one item).
  - `0015-ui-rendering.md`: the required follow-ups are under "Coordination" below.
- **Cost.** The `keymap.md` revision below, then mapper work: entries, the band, the code
  access rules of D5 (`activate` on UI-band contexts, owner 0, refused pushes of game-band
  contexts, the pending-sequence drop on the first `opaque_keys` push), ownership, gesture
  tracking (with the reset of press positions on a scale change), release-class resolution
  (only for held buttons), pass-through (with the sequence check), the sink, the end queue,
  move coalescing, repeat per hold (the set of type-3 command IDs). The client glue
  includes the platform timing reader (again on a scale change), the action-map toggle
  table, the item-drag check on the `mouselook` Start, the band-request outbox (drained in
  offers and after steps 7 and 8), the lock bridge (lock after the verdict, withheld cursor
  positions and cursor-left events, the deferred cursor-left, cursor grab and hiding, the
  warp back) and the new reaction to `menu`. The walker's HUD help line (`help_text` in
  `apps/client/src/walker/controls.rs`) labels `menu` "quit"; with a UI attached Escape no
  longer quits, so the M5 client glue changes that label (for example to "menu"), keeping
  "quit" only without a UI. `tessera-ui` keeps element geometry current for hit tests
  (D8). The plugin host gains the game-raised event queue in its deferred work, the logout
  unload in step 7 and the unload at client exit (D8). `StandardActions` and every caller
  of the retired `text_entry` actions change (D2). Fuzz targets:
  - `input_keymap` must cover the `on` field (valid and invalid values, `on` on a sequence,
    a chord, a wheel click and an axis action, through the lenient and strict readers and
    `Keymap::set`) and the migration of the old `text_entry`
    names;
  - `input_runtime` (`fuzz/fuzz_targets/input_runtime.rs`, which already activates and
    deactivates `text_entry`) must also drive band pushes and pops with fuzzed contexts
    (game-band ones included), owners (0 included) and priorities, up to and past the
    256-entry limit; `handle_with` with a scripted sink that returns fuzzed verdicts and
    appends fuzzed band requests; `offer_pending`; interleaved `handle` and `handle_with`;
    and keymaps whose entries carry fuzzed `on` values, including several bindings of one
    button in one UI-band context, so that an offer's requests pop holds that are not yet
    offered; releases of buttons that are not held, focus loss and `release_all` with
    release-class bindings present; and run-time changes of *T*, *W* and *H*. It checks invariants (c) (one End per offered Start) and (p) (no End before
    its Start, no End of a never-offered hold) on every run.

## Coordination with `0015-ui-rendering.md`

The two records are meant to be accepted together. This record does not edit the companion.
Its owner must make the changes below before the two are accepted, or the session that
accepts them makes them.

**Items 1–6 (from revision 3): applied.** The companion's revision 4 made them: its Context
("Input timing", "Pointer lock"), §2.4, §2.5, §6 row U1 and its Consequences now follow D7
and D8 as revision 3 left them, and its "Coordination with IB" says that this record needs
no change. The fourth review checked item 4 against the companion's Context, §2.5, §6 and
Consequences and found it correct and complete; the companion's U4 (b) wording ("plugin
handler time excluded (IB D8)") already agrees. In short, the six items were:
1. Context, "Input timing": the tick in step 7, layout and paint in step 8.
2. Consequences, ARCHITECTURE §20.3 step 8: "UI: layout, paint damaged rectangles within the
   UI slice, build the composition list", the companion's own wording.
3. §6 row U1: Tessera's own work in the step-1 offers, the step-7 tick and step 8; plugin Lua
   time excluded.
4. Pointer lock (Context and §2.5): on only after the UI declined the `mouselook` Start, off
   after the End of the last hold that turned it on; no cursor-position events during the
   lock.
5. §2.4: the step-1 hit tests and the step-7 hover refresh use the layout of the last
   completed step 8. (Superseded by item 10: they now use the current geometry.)
6. The companion's own coordination items 1 and 4: settled.

**Items 7–9 (from revision 4): applied.** The companion's revision 5 made them, and its
"Coordination with IB" item 6 asks this record to say so. In short:
7. Item drag and the lock-on point (D7, "During an item drag"): the companion's owner
   summary, Context "Pointer lock" and §2.5 now say that the lock turns on only when the UI
   declined the `mouselook` Start and no item drag is in progress, and that during every
   item drag the Start is consumed, by the UI or by the client.
8. Cursor-left withheld during the lock (D7, "How the position is frozen", item 1): §2.5
   ("In short", items 2 and 3) and its "Agreement with IB" withhold cursor-left events too
   and feed a withheld cursor-left right after the turning-off call.
9. The end of step 7 and plugin time (D8): the companion's Context, its ARCHITECTURE §20.3
   step-7 restatement and §6 row U1 name the slicing of the deferred work and the collector
   step.

**The companion's own follow-ups 6 and 7: done here.** Item 6 is the paragraph above. Item
7 (the OS cursor under `ui_cursor = software`) is the parenthesis added to D7 "How the
position is frozen" item 2. The companion can mark both settled.

**Items 10–12 (new with revision 5).** Revision 5 changes two things that the companion
restates: which geometry the hit tests use, and what step 7 contains.

10. **Hit tests use the current geometry** (D8, "Geometry used for hit tests"). Every place
    where the companion says that the step-1 hit tests and the step-7 hover refresh use
    "the layout of the last completed step 8" or "the previous frame's layout" changes:
    - Context, "Input timing": "against the previous frame's layout (IB D8, from LPA §3.3
      and §9.7)" becomes "against the elements' current geometry (IB D8, from LPA §3.3,
      §9.7 and §13.11)".
    - Decision 1.5, "Hit testing during a transition": the parenthesis "(…; IB D8: the
      step-1 hit tests and the step-7 hover refresh use the layout of the last completed
      step 8)" becomes "(…; IB D8: the step-1 hit tests and the step-7 hover refresh use the
      elements' current geometry, which a handler's `SetSize` or `SetPosition`, a panel
      move or resize, and the re-layout of UL §8.4 change at once)". The rest of that
      bullet (a click near a moving edge can hit a box that is not drawn yet; a recorded
      difference bounded by U4) still holds and becomes slightly more frequent, since the
      box can now move within a frame.
    - §2.4, the bullet "Hit tests in frame-loop step 1, and the hover refresh of the step-7
      tick, use the layout of the last completed step 8 (IB D8, and IB 'Coordination' item
      5)" becomes "… use every element's current geometry: position, size, visibility and
      z-order as they stand when the hit test runs; step 8 changes no box that a hit test
      reads (IB D8, and IB 'Coordination' item 10)". §2.4 item 3 (rounding under a scale)
      is unaffected: the displayed box it compares is the current one.
    - Consequences, the ARCHITECTURE §20.3 step-1 restatement: "against the previous
      frame's layout" becomes "against the elements' current geometry". The same entry
      and the companion's Context ("step 8 lays out, paints and builds the composition
      list") may keep "layout" for step 8, read as in the owners list of this record:
      step 8 changes no box that a hit test reads.
    - "Coordination with IB" item 5 is superseded by this item.
11. **What step 7 contains** (D8, "Game-raised plugin events" and "Unload at logout and at
    client exit").
    - Consequences, the ARCHITECTURE §20.3 step-7 restatement, and Context "Input timing":
      "then the plugin host's deferred work (sliced by whole items, 2 ms per frame)" gains
      "(it includes the plugin events raised by game and session state, such as
      `Turbine.Chat.Received`, and the logout unload)". The same entry adds the sentence
      that the owners list gives after the §20.3 list: at client exit, after the last
      frame, the host unloads every apartment, the only plugin code outside steps 1 and 7.
    - §6 row U1 needs no change: game-raised events are plugin time inside "the plugin
      host's deferred work", which U1 already excludes.
    - Consequences, ARCHITECTURE §20.3 step 1: events that arrive between redraws are
      handled in winit's callbacks on arrival and belong to step 1 of the next frame (D8);
      the companion's U1 sweep, which injects raw `CursorMoved` events, counts them there.
12. **Standing request, the other way round.** If the companion changes when step 8 lays
    out, or lets step 8 move a box that a hit test reads, D8 "Geometry used for hit tests"
    changes with it.

Nothing else in the companion depends on what revisions 4 and 5 changed. The pass-through
rule, the release-class rules (after a UI-owned press, only for held buttons, none at focus
loss), the drop rules for holds not yet offered and for pending sequences, the modifier
policies, per-hold repeat, the read-time check of `on`, the scale-factor reset, the
wheel-motion timing and the capture reading of the wheel concern the mapper and
`tessera-ui` only. The capture reading does not touch the companion, which does not restate
the wheel rule for captures.

## Changes `keymap.md` must gain (for its spec owner)
1. **§0 and the header.** The sentence "nothing in them is taken from another program's key
   map" becomes: nothing in §0–§11 is taken from another program, **except** the parts that
   reproduce the original's behaviour for compatibility, each listed with its provenance in
   §1.1:
   - the default bindings of `pointer` and the UI contexts (§8), and their command names
     (§2, from KM §12.14.2 and §12.15);
   - the activation timing rules of the new §7.x (the five activations, *T*, *W*, *H*, the
     half-time tap, the half-open nearby rectangle, the third-press reading), from UB §3.3
     (BIN);
   - the UI-band ordering rules of §3 (priority order, newest first among equals, the
     identical-push no-op, the pop at every priority), which reproduce UB §5.2 (BIN);
   - the push rule of `ScrollableControls_Mouse` (§8.x), from UW §5.7 and §6.9 (BIN);
   - the repeat rule of §7.y (a hold repeats when its command has toggle type 3), from
     KM §12.10;
   - "the same command goes on to game code" (§7.y, Option 4), from UB §5.3 and LPA §13.13.
   Everything else new in §0–§11 (the two bands as a mechanism, pass-through, `opaque_keys`,
   the offer protocol, the end queue, move coalescing, the drop rules, the modifier
   policies, the clamps, the settings fallbacks and the Tessera-choice defaults below) is a
   Tessera design decision and stays implementer-authored. The author-role line names the
   spec-writer role for the listed parts (the §8 rows, the new §7.x, the band-ordering rules
   of §3, the push rule of §8.x and the repeat rule of §7.y), not only for the §8 rows.
2. **§1.1 provenance** (spec-writer role for the rows taken from the original):
   - the UI-band design: this record (Decision); the band-ordering rules within it: UB §5.2
     (BIN there; UB §13.9 is the worked example);
   - UB §3.3 timing rules: BIN there;
   - the `pointer` defaults: UB §3.2 (Confirmed there, KM);
   - the `ui.scroll_mouse` defaults and its push rule: UB §3.2, UW §5.7 and §6.9 (BIN);
   - the `ui.dialog` defaults: UB §5.6 (Confirmed bindings, KM §12.9);
   - the `text_entry` caret, delete, Escape and Enter defaults and the `ui.clipboard`
     Ctrl+C/X/V defaults: UB §5.4 (BIN there). The exact key for each caret command beyond
     UB §5.4's list is Tessera's reading of that list;
   - `Ctrl+Insert`, `Shift+Delete` and `Shift+Insert` for copy, cut and paste: **Tessera
     choice** (a common platform convention). UB §5.4 says only "an alternative meta key";
   - the exact `Ctrl+Shift` defaults of the word and document moves (`Ctrl+Shift+Left/Right`,
     `Ctrl+Shift+Home/End`): **Tessera choice**, made so that one binding fires under §7.2
     (D2, Known difference 19); they are not the original's bindings;
   - the `ui.chat_log` context and its defaults: **Tessera choice** (the `text_entry` keys;
     no InputMap behind it, Known difference 17);
   - the repeat rule and the built-in `repeat` flags: KM §11 and §12.10 (toggle type 3),
     with the data deciding per hold by command when present (D7).
3. **§2 actions**: optional command name and `repeat` flag (the fallback without action-map
   data; with data, repeat is decided per hold by its command, D7); how an event's command
   name and ID are chosen (D1); the modifier policies of the new actions (D2): Subset for the pointer
   actions, the caret actions, `text_backspace`, `text_delete` and `scroll_up/down`; Exact
   for the focus and clipboard actions.
4. **§3 contexts**: the two bands and the band of each context; UI-band entries (context,
   owner, priority) with the ordering, no-op, pop-at-every-priority and once-per-context
   rules (D5); the 256-entry limit, with a refused push not reported back (D5); queued stack
   changes; the `pass_through` flag (D4); the `opaque_keys` flag with the band-boundary rule,
   its release rule (only when the band goes from no `opaque_keys` entry to one; holds of
   UI-band contexts are never released by it) and its modifier-hold rule (D6); the
   band-end rule that a key press no UI-band context took **ends the pass** (it is not
   consumed, so pass 2 still tries the UI band, D6); the pending-sequence rules (a pop that
   removes a context drops its pending sequence; the push of the first `opaque_keys` entry
   drops a pending sequence of a context consulted after it; other pushes keep it, D5).
   The code access rules of D5: the band is fixed at registration, also for contexts
   registered on a running mapper; a push or pop naming a game-band context is refused;
   owner 0 is reserved for code; `activate`, `deactivate`, `set_active` and `is_active` on
   a UI-band context are the push and pop of (context, 0) at its registered priority; and
   `handle` resolves against the band as it stands. A pass-through context holds no
   sequences: multi-stroke bindings in it are disabled at compile time with a warning (D4).
5. **§5.3 binding entries, §6.1, §6.3**: field `on` with the five values (D3), allowed only on
   single-stroke, single-button entries, not on sequences, chords, wheel clicks or analog
   (`axis`) entries, written only when not `press`. The schema example shows `on`. These
   cases are detectable from the entry alone, so they are caught at **read time**: the
   lenient reader drops such an entry (and an unknown `on` value) with a warning, the
   strict reader fails, and `Keymap::set` refuses it with an error. Deterministic writing
   keeps `on`. A non-`press` `on` on an entry of an **axis action** can only be detected
   against the registry, so `Keymap::set` and both readers accept it and it is disabled
   with a warning when the keymap is compiled (D3). D3 and this item state the same split. **Migration:**
   on reading, a `text_entry` override of `text_submit` moves to `open_chat`, `text_cancel`
   to `menu`, and `text_history_prev`/`text_history_next` to `text_line_up`/`text_line_down`,
   unless the file already has the new pair; each move is a warning, and saving writes the
   new names. Without this, §6.1's "unknown names are kept" would keep the old overrides but
   silently stop applying them.
6. **§7.1**: raw events stay; the snapshot excludes UI-owned holds; new `handle_with` and
   `offer_pending` paths.
7. **New §7.x activation timing**: per-button tracking, *T*/*W*/*H* source and pixel space,
   `double_click_px` as the rectangle size in logical pixels (scaled, then clamped), clamps
   (*T* 100–5000 ms, *W* and *H* 2–64 px),
   third-press rule, unknown pointer never nearby; only presses that §7.2 resolves are
   recorded (not OS repeats, presses of held buttons or presses beyond 64 held buttons);
   new *W*, *H* on a scale-factor change, with every recorded press position reset to
   unknown (D3).
8. **§7.2/§7.3**: press-class versus release-class resolution (both with the two passes),
   pulses, event order within one raw event, pass-through (D4): a pass-through context
   consumes only on a UI verdict, whatever its `on` values; each (context, binding entry)
   pair fires and is offered at most once per raw event, so a binding that fired in pass 1
   is neither fired nor offered again in pass 2; a pass-through context that fired anything
   in pass 1 is skipped in pass 2 (it counts as not matching there), so the exact pass
   prefers exact bindings inside it too; event order keyed by pass, then context, then
   activation class, then registration order; held-set exclusion of UI-consumed buttons
   (D7), and its release-side counterpart: at the release of a button whose press produced
   at least one hold, all of them UI-owned, release-class resolution considers only
   UI-band and pass-through contexts (D4); a press with no hold qualifies for neither.
   Release-class resolution runs only at the release of a button in the held set; the
   release of a button that is not held changes nothing, and focus loss, `release_all` and
   the other operations that end holds fire no `release` or `tap` binding (D4). The
   sentence "Auto-repeat presses … are ignored" becomes: auto-repeat presses never start or
   resolve holds; they produce Repeat offers for repeatable holds (D7).
9. **§7.4**: game-band modifier holds are not evaluated while an `opaque_keys` entry is in the
   UI band (D6).
10. **§7.5**: wheel motion skips `pointer` and the game band when a UI-band context binds a
    wheel click of that axis (D7). **Timing change:** whether a wheel event's motion is
    withheld is decided per raw event, when the event is resolved, against the band at that
    moment, under `handle` and `handle_with` alike; routing of motion that is not withheld
    stays at `update`, as today. So a push or pop between the wheel event and `update` does
    not move that event's motion, and under `handle` an active `ui.scroll_mouse` withholds
    the motion and puts its `scroll_up`/`scroll_down` clicks in the snapshot. With no
    UI-band entry nothing is withheld, so today's snapshot is unchanged (invariant (e)).
11. **§7.6**: typed characters go to the sink by the D6 rule and are then not in the
    snapshot's `text`.
12. **New §7.y UI offers**: what is and is not offered, verdicts, hold ownership, the end
    queue, Repeat, the raw-event sequence number on offered events, move coalescing with the
    flush before every key, button, wheel, typed-character, cursor-left and focus-lost
    event (whatever it will offer), the three steps of one call and the two of
    `offer_pending`, focus loss (D7); `offer_pending` once per frame after the last raw
    event, except that a cursor-left withheld during the pointer lock is fed after the
    `offer_pending` call that ended the lock (D7, D8 step 1). Repeat is decided per hold by
    the command its Start carried. Only a Start's verdict is used; the verdicts of an End
    and a Repeat are ignored. Only offered Starts get Ends: a hold made under `handle` never
    has its End or a Repeat offered. A hold or pulse of the raw event being resolved that an
    applied request ends before its Start was offered is dropped (no Start, no End, no
    snapshot edge). The
    **band request channel** (D5): the sink trait's offer method receives, besides the
    event, a mutable band request list owned by the mapper, to which the sink appends push
    (context, owner, priority) and pop (context, owner) requests; the mapper applies the list
    in order when the offer returns, after every offer and in every step, and offers at
    once the Ends this causes, each again with its own list. Outside
    `handle_with` and `offer_pending` the mapper's push and pop operations are called
    directly, and their Ends go to the end queue.
13. **§8**: contexts `pointer` (40, pass-through, not in the mode helpers), `ui.dialog`,
    `ui.clipboard`, `ui.scroll_mouse`, `ui.scroll`, `ui.eat_keyboard`, `ui.chat_log` (UI
    band, registered priority 0); `text_entry` moved to
    the UI band with `opaque_keys` + `text`; retire `text_submit`, `text_cancel`; rename
    `text_history_prev/next` to `text_line_up/down`; new actions and defaults:

    | Context | Action (command) | Default |
    |---|---|---|
    | pointer | `select_left` (`SelectLeft`) | `MouseLeft` |
    | pointer | `select_double_left` (`SelectDblLeft`) | `MouseLeft`, `on = "double_nearby"` |
    | pointer | `tap_left` (`TapLeft`) | `MouseLeft`, `on = "tap"` |
    | pointer | `select_right` (`SelectRight`) | `MouseRight`, `on = "tap"` |
    | pointer | `select_double_right` (`SelectDblRight`) | `MouseRight`, `on = "double_nearby"` |
    | pointer | `select_middle`, `select_double_middle`, `tap_right`, `tap_middle` (`SelectMid`, `SelectDblMid`, `TapRight`, `TapMid`) | unbound |
    | ui.scroll_mouse | `scroll_up` / `scroll_down` (`ScrollUp` / `ScrollDown`) | `WheelUp` / `WheelDown` |
    | ui.dialog | `focus_next` / `focus_prev` (`FocusNext` / `FocusPrevious`) | `Tab` / `Shift+Tab` |
    | ui.dialog | `focus_left/right/up/down` (`FocusMoveLeft/Right/Up/Down`) | arrow keys |
    | ui.dialog | `menu` (`EscapeKey`) | `Escape` |
    | text_entry | `menu` (`EscapeKey`), `open_chat` (`AcceptInput`) | `Escape`; `Enter`, `NumpadEnter` |
    | text_entry | `text_char_left/right` (`CursorCharLeft/Right`), `text_word_left/right` (`CursorWordLeft/Right`) | `Left`/`Right`; `Ctrl+Left`/`Ctrl+Right` and `Ctrl+Shift+Left`/`Ctrl+Shift+Right` |
    | text_entry | `text_line_up/down` (`CursorPreviousLine/NextLine`), `text_page_up/down` (`CursorPreviousPage/NextPage`) | `Up`/`Down`; `PageUp`/`PageDown` |
    | text_entry | `text_line_start/end` (`CursorStartOfLine/EndOfLine`), `text_doc_start/end` (`CursorStartOfDocument/EndOfDocument`) | `Home`/`End`; `Ctrl+Home`/`Ctrl+End` and `Ctrl+Shift+Home`/`Ctrl+Shift+End` |
    | text_entry | `text_backspace` (`BackspaceKey`), `text_delete` (`DeleteKey`) | `Backspace`; `Delete` |
    | ui.clipboard | `text_copy` / `text_cut` / `text_paste` (`CopyText` / `CutText` / `PasteText`) | `Ctrl+C`, `Ctrl+Insert` / `Ctrl+X`, `Shift+Delete` / `Ctrl+V`, `Shift+Insert` (the Insert/Delete forms are a Tessera choice, item 2) |
    | ui.chat_log | `text_page_up/down`, `text_doc_start/end` (the same commands as in `text_entry`) | `PageUp`/`PageDown`; `Ctrl+Home`/`Ctrl+End` and `Ctrl+Shift+Home`/`Ctrl+Shift+End` (Tessera choice, the `text_entry` keys) |

    The caret, delete, backspace and scroll actions have the Subset policy, so the plain
    defaults also fire with extra modifiers (Shift+Left gives `text_char_left`, Shift+Up
    and Ctrl+Up give `text_line_up`, as `chat-window.md` §6.4 needs). The exact Ctrl+Shift
    rows exist so that Ctrl+Shift+Left fires `text_word_left` alone in §7.2's first pass
    (D2, difference 19). The focus and clipboard actions are Exact. The caret, delete,
    backspace and focus-move actions have `repeat` set as the fallback without data; with
    data the action map decides, per hold by the hold's command (D7, D9). This answers the auto-repeat part of the §11
    OPEN.
14. **New §8.x**: the InputMap → context table of D5, with the push rules, including the
    Tessera row for `ui.chat_log`.
15. **§10 winit adapter**: OS repeat presses are passed through with the repeat flag; cursor
    positions are physical pixels (D3); platform double-click values are read by the client,
    not the adapter.
16. **§11**: auto-repeat OPEN resolved (item 13); IME stays OPEN; new OPENs: map consultation
    order versus D5, one binding versus all matching (Consequences, difference 2), plugin
    pushes of `ScrollableControls_Mouse`, unmapped game-panel maps, command actions for
    untranslated commands, the four modifier-click commands, and whether a double-select
    binding swallows a quick re-press of a game binding in the original (difference 14,
    owner observation), mouse-button Ends during the pointer lock (difference 16), the
    map behind the chat log's paging commands (difference 17), and a right press during an
    item drag (difference 20, owner observation).
17. **§12.14.2**: three existing rows are **retargeted**, not kept beside new ones (the old
    targets are retired actions): `EditControls` `AcceptInput` → (`text_entry`,
    `text_submit`) becomes → (`text_entry`, `open_chat`); `EditControls` `EscapeKey` →
    (`text_entry`, `text_cancel`) becomes → (`text_entry`, `menu`); `EditControls`
    `CursorPreviousLine`/`CursorNextLine` → `text_history_prev`/`text_history_next` become →
    `text_line_up`/`text_line_down`. The `EditControls` `BackspaceKey` row stays. After the
    revision no §12.14.2 row names a retired action. New rows:
    - `Main` `SelectLeft` … `TapMid` → `pointer`.
    - `ScrollableControls_Mouse` `ScrollUp`/`ScrollDown` → `ui.scroll_mouse`.
    - `DialogBoxes` `FocusNext`, `FocusPrevious`, `FocusMove…` → `ui.dialog`.
    - `DialogBoxes` `EscapeKey` → (`ui.dialog`, `menu`).
    - `EditControls` caret commands other than the two line moves above, and `DeleteKey`
      → `text_entry`.
    - **Copied to `ui.chat_log`** (Tessera rule): an imported `EditControls` binding of
      `CursorPreviousPage`, `CursorNextPage`, `CursorStartOfDocument` or
      `CursorEndOfDocument` is written to (`text_entry`, the action) **and** to
      (`ui.chat_log`, the same action), keeping its own command. So a user's imported
      PageUp/PageDown override changes both, and the chat log pages with the same keys
      as the entry line. The import report lists the copy once, as part of the
      `EditControls` binding. `ui.chat_log` has no InputMap, so no other section feeds
      it.
    - `CopyAndPasteControls` → `ui.clipboard`.
18. **§12.14.5**: activations translate instead of being dropped:

    | Activation | Tessera `on` |
    |---|---|
    | has Down (0x01) | `press` |
    | otherwise DblClickDown (0x08) | `double` |
    | otherwise NearbyDown (0x20) | `double_nearby` |
    | otherwise Up (0x02, also 0x06) | `release` |
    | otherwise Tap (0x04) | `tap` |
    | DblClickUp-only or NearbyUp-only | dropped |

    A non-`press` result is kept only when the translated stroke is one button plus
    modifiers (D3). When §12.14.4 rule 2 turns a meta-key bit into a chord button (for
    example bit 8 into `MouseRight`, or a lone `LShift`) on a binding that translates to
    `release`, `tap`, `double` or `double_nearby`, the stroke has two buttons and the
    binding is **dropped**, with the reason "activation on a chord" in the import report
    (§12.14.1). A `press` binding with such a chord button is translated as today. A
    non-`press` result whose target is an **axis** action (for example a release binding of
    a movement command) is dropped with the reason "activation on an axis action" (D3).
    This is a change for existing **button** actions: their release and tap bindings (for
    example the 0x06 bindings) used to be dropped and are now imported with `on`, so the
    import report lists fewer drops and the imported key map can fire those actions at a
    release.
19. **§12.14.6**: the new known differences 8, 9, 14 and 19 of this record.
20. **§12.15**: the reporting table gains the rows of item 13, including (`ui.chat_log`,
    `text_page_up/down`, `text_doc_start/end`) → `CursorPreviousPage`/`NextPage`,
    `CursorStartOfDocument`/`EndOfDocument`. That context has no InputMap of its own; it
    is fed only by the copy rule of item 17. The existing rows for `text_submit`, `text_cancel` and
    `text_history_prev/next` are **replaced**, not kept: (`text_entry`, `open_chat`) →
    `AcceptInput`, (`text_entry`, `menu`) → `EscapeKey` and (`text_entry`,
    `text_line_up/down`) → `CursorPreviousLine`/`CursorNextLine`. No row names a retired
    action afterwards.
21. **Invariants and examples**:
    - (a) The UB §13.1 timing sequence through the mapper gives exactly those pulses and
      holds.
    - (b) The UB §13.9 push/pop sequence gives that stack.
    - (c) A consumed Start never shows in the snapshot; its End is offered exactly once,
      also when the hold ends in `deactivate`, `set_keymap` or `release_all`.
    - (d) The LPA §13.12.2 double-click order is produced.
    - (e) With no UI-band entry, `handle` without a sink gives today's snapshot for every
      existing action, with `pointer` inactive and with it active, for any press timing:
      also for a second right press within *T* at the same position (`mouselook` starts
      again) and a second left press within *T* while right is held (`move_forward` +1
      again). This holds because a pass-through context consumes only on a UI verdict (D4),
      and assumes the 256-hold limit of §7.3 is not reached.
    - (e2) Under `handle`, after `activate(text_entry)`: `is_active(text_entry)` is true and
      the band holds (`text_entry`, 0, 100); activating it again changes nothing; `W` puts
      `w` into the snapshot's `text` and does not move; Enter gives `open_chat`, Escape
      gives `menu`; `MouseRight` gives `mouselook` and the wheel changes `zoom`; a held
      `MouseRight` survives the activation, a held `W` is released. After
      `deactivate(text_entry)` the band is empty and (e) applies again. A band push naming
      `movement` changes nothing.
    - (e3) A multi-stroke binding in `pointer`, and a non-`press` `on` on an entry of an
      axis action, are disabled with a warning when the keymap is compiled.
    - (f) A declined Escape in `text_entry` presses `menu` for game code.
    - (g) With a UI attached and nothing under the pointer, the run chord works in both press
      orders and `mouselook` survives releasing left (today's
      `mouselook_survives_releasing_left_in_either_order`); a second right press within *T*
      at the same position offers `select_double_right` (declined) and starts `mouselook`;
      a second left press within *T* while right is held offers `select_left` and
      `select_double_left` (declined, pointer locked) and runs again.
    - (g2) With a UI attached and a hit element under the pointer, a second right press
      within *T* at the same position offers `select_double_right`, which is consumed, and
      `mouselook` does not start.
    - (h) Shift+`MouseLeft` gives `select_left`.
    - (h2) Client glue, pointer lock: with a UI that consumes every key command (a drag
      whose icon the container created, with an active panel), a right press offers a
      consumed `mouselook` Start, the lock stays off, and the next cursor-position event is
      fed and offered as a move. With no drag and a UI that declines it, the lock turns on
      after the verdict, no cursor-position or cursor-left event reaches the mapper until
      the End of the last such `mouselook` hold, and the `select_right` pulse of a quick
      click carries the press position.
    - (h4) Client glue, the other item drags: with a drag icon supplied by a plugin control,
      or a container-made icon and no keyboard target, a right press is routed by the UI
      and declined (plugins get `KeyDown` for `CameraInstantMouseSteer`), the client
      returns Consumed, the lock stays off, the snapshot shows no `mouselook`, and the left
      release drops on the element under the real pointer position.
    - (h5) Client glue, cursor-left during the lock: cursor-position and cursor-left events
      arriving during the lock are withheld; the turning-off End and the `select_right`
      pulse carry *F*; if the last withheld report was a cursor-left, a "pointer left" offer
      follows them, and nothing is hovered until a position inside the window arrives.
    - (h3) Client glue, capture across the lock: left pressed on a hit element, right
      pressed (lock on), left released: the element gets `MouseUp` and no `MouseClick`, and
      the capture has ended.
    - (i) With `text_entry` and `ui.clipboard` pushed in either order, `W` types, `Ctrl+C`
      gives `text_copy`, and `Tab` reaches `ui.dialog` below them.
    - (j) Many `CursorMoved` events followed by a press give one pointer-moved offer at the
      last position, then the press.
    - (k) A wheel notch with `ui.scroll_mouse` pushed gives `scroll_up` and no `zoom`
      delta; without it, `zoom` changes. The same holds under `handle` with
      `ui.scroll_mouse` activated by code (`scroll_up` in the snapshot, no `zoom` delta). A
      notch resolved while the entry is pushed, followed by its pop before `update`, still
      gives no `zoom` delta, and the reverse order still changes `zoom`.
    - (k2) Capture and the wheel: with the thumb of a scroll bar captured (left held on it)
      and the pointer moved off the thumb, the bar has popped `ui.scroll_mouse` and a wheel
      notch changes `zoom`; with the pointer back on the thumb, it gives `scroll_up` again.
    - (l) With `ui.scroll_mouse` not pushed, a cursor-position event onto a list followed, with
      no `offer_pending` in between, by a wheel notch gives: the move offer (during which the
      sink requests the `ui.scroll_mouse` push), then `scroll_up`, and no `zoom` delta. The
      same holds when the event after the move is a key press that offers nothing: the move
      is still offered before it.
    - (m) Shift held, `MouseLeft` pressed with `pointer` active and no exact binding anywhere:
      `select_left` Start is offered exactly once (pass 2), and with no modifier held it is
      offered exactly once (pass 1), also when no lower context matches.
    - (m2) With a test action bound to exact `Shift+MouseLeft` in `pointer`, a declined
      Shift-click fires and offers that action (pass 1) and not `select_left`; a lower
      context's subset binding of `MouseLeft` still fires in pass 2.
    - (n) A band push requested by the sink during an offer of step 3 is not consulted for
      the rest of that raw event (pass-through continuation included) and is consulted for
      the next raw event; a push requested during the step-2 move offer, or during an End
      offered in step 1, is consulted for the raw event of that call.
    - (o) A release-class binding in a game-band context that is not pass-through (a test
      action bound to `MouseLeft` with `on = "release"` in `movement`) fires at a release
      over the world, and does not fire at the release of a left press that the UI
      consumed; `tap_left` in `pointer` is offered at both releases.
    - (p) Ends: no End is offered before its Start, and none for a hold made under
      `handle`. With two bindings of one key in one UI-band context whose first offer pops
      that context, the second hold is dropped: its Start and End are never offered and
      the snapshot shows no edge for it.
    - (q) Modifier policies: in `text_entry`, Shift+Left gives `text_char_left`;
      Ctrl+Shift+Left gives `text_word_left` alone; Shift+Up and Ctrl+Up give
      `text_line_up`; Ctrl+Tab in `ui.dialog` gives nothing.
    - (r) Clamp: `double_click_px` 1 at scale factor 1 gives *W* = *H* = 2, and a second
      press at the same position within *T* fires `double_nearby` bindings, one pixel to the
      right does not.
    - (s) Releases that resolve nothing: the release of a button that is not held changes
      nothing (no hold ends, no `release` or `tap` binding fires). With `MouseLeft` held and
      a `tap` binding on it, focus loss (or `release_all`) ends the holds and fires no
      `tap`; the button's later physical release is then a release of a button not held.
    - (t) No-hold presses: a key with no press-class binding and a game-band `on =
      "release"` binding fires that binding at its release, also with a UI attached and a
      UI-band context pushed that does not bind the key and is not `opaque_keys`.
    - (u) Pending sequence: with a game-band binding `G G` and its first `G` pressed,
      pushing `text_entry` (the first `opaque_keys` entry) outside an offer drops the
      sequence; a second `G` then types `g` and fires nothing. A hover push of
      `ui.scroll_mouse` between the two presses does not drop it.
    - (v) Repeat per hold: with action-map data, an OS repeat of a key held by two holds of
      one action, one carrying a command of toggle type 3 and one of type 0, gives a Repeat
      for the first hold only. Without data, the action's built-in `repeat` flag decides.
    - (w) Press tracking: OS repeat presses do not update the last press time, so a key held
      with repeat and then released and pressed again more than *T* after its real press is
      not a double press. After a scale-factor change, a press at the position of the
      previous press within *T* fires `double` but not `double_nearby` bindings.
    - (x) Client glue, current geometry: a plugin control captured by a left press whose
      `MouseMove` handler calls `SetPosition` to follow the pointer gets, for two coalesced
      moves in one frame (one flushed before a key event, one by `offer_pending`), two
      `MouseMove` events with `X`/`Y` relative to its position after the first, and no
      `MouseLeave` or `MouseEnter`.
    - (y) Plugin code placement: a chat line posted in step 2, and a system notice posted at
      the start of the redraw, reach `Turbine.Chat.Received` in that frame's step 7 (or a
      later one, when the slice is used up), after the `Update` broadcast; no plugin code
      runs in steps 2–6 or 8–10. At client exit, `Unload` handlers run after the last frame
      and a `PluginData.Save` they make is written before the process ends.

## Worked example (synthetic)
*T* = 500 ms, *W* = *H* = 4 (`double_click_px` 4 at scale factor 1), defaults above, `pointer`
active, nothing focused, pointer over a plugin button *B*.

- Left press at 0 ms: `select_left` Start, offered and consumed (hit), so it is UI-owned and
  the press stops in `pointer`. Release at 150 ms: `select_left` End, then the `tap_left`
  pulse (150 ≤ 250). Press at 300 ms at the same spot: `select_left` Start, then
  `select_double_left` Start (`double_nearby`). Release at 600 ms: the two Ends in that order,
  with no tap (300 > 250). The game snapshot shows none of it, and *B* receives the event
  sequence of LPA §13.12.2 (the `TapLeft` pulse is Impulse in the action map, so it gives no
  button events, D9).
- The same clicks over the world are declined. The first press continues past `pointer` and
  finds nothing below it. The game sees `select_left` pressed and released twice,
  `tap_left` once (at 150 ms), and `select_double_left` once.
- Right held over the world, then left pressed: `select_left` fires and is declined (the
  pointer is locked), the press continues, and the run chord in `movement` fires. Releasing
  left ends run; `mouselook` stays held.
- Right turned the camera over the world from 0 ms to 800 ms (lock on, position frozen at
  *F*); right pressed again at 900 ms: within *T* of the press at 0 ms? No (900 > 500), so
  only `mouselook` starts. Released at 1000 ms (a tap: the `select_right` pulse at *F* hits
  nothing and is declined) and pressed again at 1200 ms: 300 ms after the press at 900 ms
  and, the position being *F*, nearby, so `select_double_right` fires first and is offered; the
  lock is off and nothing is hit, so it is declined, the press continues, and `mouselook`
  starts again (D4).
- A quick right click on *B*: the `mouselook` Start goes to the keyboard target (the active
  panel, if any, since nothing is focused), which does not handle it, so it is not
  consumed, and only then does the lock turn on at *F*. Release at 100 ms: `mouselook` End (the last
  locking hold, so the lock turns off), then the `select_right` pulse at *F*, the press
  position, which the mapper kept because it was fed no cursor positions (D7), so
  *B* gets `MouseDown`, `MouseUp`, `MouseClick` with `Button` 2. A second right press on *B* at
  300 ms is within *T* and nearby: `select_double_right` is consumed by *B*, and `mouselook`
  does not start (Known difference 14).
- An item drag is in progress (left held, the drag icon follows the pointer) and right is
  pressed. Two cases (D7, "During an item drag"):
  - The source's container created the icon (UB §7.1 step 5) and a panel is active: the UI
    reports the `mouselook` Start consumed without a key message (UB §5.3 step 4), so no
    plugin gets `KeyDown`.
  - *B*'s plugin control supplied the icon (UB §7.1 step 4), or no panel is active (nothing
    is focused here, so there is then no keyboard target): the UI declines the Start, and
    plugins that want key events get `KeyDown` with the `CameraInstantMouseSteer` ID. The
    client sees the drag icon and returns Consumed itself (Tessera rule).

  In both cases the hold is UI-owned, the camera does not turn, the lock stays off and the
  cursor stays visible. The drop at the left release is hit-tested at the real pointer
  position and reaches its target. Right's release ends the UI-owned `mouselook` hold
  (`KeyUp` in the second case) and changes no lock.
- Left pressed on *B* (captured, `MouseDown`), then right pressed over it: `mouselook`
  starts and the lock turns on. Left released during mouse-look: *B* gets `MouseUp`, the
  capture ends, and no `MouseClick` follows, because nothing is hovered during the lock
  (D7, difference 16).
- The pointer moves from the world onto a list and, in the same frame, the wheel turns one
  notch. The wheel event first flushes the pending move: the list's scrollable becomes
  hovered and requests the `ui.scroll_mouse` push, which is applied before the wheel event is
  resolved (D7 steps 2 and 3). So `scroll_up` is offered and the list scrolls one line step
  (UW §5.6, §6.8); the camera does not zoom. The same notch over a plain native panel: no
  `ui.scroll_mouse`, so `zoom` changes.
- Escape while a text field without `lose_focus_on_escape` has focus: `menu` from
  `text_entry` is offered. The field declines it, so game code sees `menu` pressed and plugins
  get `KeyDown` with the `EscapeKey` ID. Game code must treat it as `EscapeKey` (close the
  active panel or open the menu), not quit (Consequences).

## Revision 1: answers to the review (2026-10-06)
1. **`pointer` broke the run chord.** Accepted. `pointer` is now pass-through and inactive in
   the mode helpers (D2, D4, Options 6). D6, invariant (e) and the new invariant (g) are
   reworded. A double click still takes the press, which is §12.12's largest-activation rule.
   (Superseded by revision 2, item 1: a pass-through context now consumes only on a UI
   verdict, and the largest-activation claim is withdrawn as inferred.)
2. **`opaque_keys` and the shared band.** Accepted with the reviewer's rule: opacity is
   decided at the band boundary, and text goes to the band when it holds a `text` context
   (D6). D5 now says who pushes `ui.clipboard` and in what order, and why the order does not
   matter.
3. **Events produced by `update`.** Accepted. Modifier holds and analog presses are not
   offered (D7, difference 6). Ends of holds released outside `handle_with` go through an end
   queue, offered at the next `handle_with` or by `offer_pending` (D7).
4. **Architecture and frame loop.** Accepted. D8 places input routing and plugin input
   handlers in step 1, and cites LPA §3.3 and §9.7, which require this. The owners list now
   names ARCH §20.3, ARCH §18.1 and the conflicting sentence in `0015-ui-rendering.md`.
   Hit tests use the previous frame's layout. (Revision 5, item 2, replaces this: hit tests
   use the current geometry.)
5. **Budget.** Accepted. Pointer moves are coalesced (D7) and plugin time is counted
   separately (D8). One part is not taken: plugin handler calls are not deferred to a slice,
   because LPA §9.7 requires their effects before input processing continues.
6. **Modifier-clicks.** Accepted: Subset policy (D2), difference 8.
7. **No IDs, no UI input.** Accepted (D1): events with a name are offered with or without
   an ID. Events with no name at all are still not offered, since the UI has no way to route
   them and the original has no command for them.
8. **Toggle types.** Accepted (D9). The `repeat` flag now follows toggle type 3 when the data
   is present, with the built-in flags as the fallback, and a Repeat produces
   `Element_key_press` and `KeyDown` again (D7).
9. **Provenance.** Accepted: list items 1 and 2, including the Tessera choice of the Insert
   and Delete clipboard keys.
10. **Arrow keys in `ui.dialog`.** Accepted: a Consequences entry and the D5 push rule.
11. **Pointer lock and right clicks.** Accepted (D7, Pointer lock). `claims_pointer` is
    removed rather than repaired: UW §5.7 and §6.9 (new since the review) give the
    original's rule, the hover push of `ScrollableControls_Mouse`, so the band itself decides
    where the wheel goes (D7, Options 7). That removes the ordering question about the
    callback.
12. **Further known differences.** Accepted: differences 10–13 and the pixel space in D3.
13. **Migration and wording.** Accepted: list items 5, 8 and 11.
14. **Worked example.** Accepted: the world case now includes the `tap_left` pulse.

## Revision 2: answers to the second review (2026-10-06)
1. **D4 contradicted itself; invariants (e) and (g).** Accepted, and resolved by changing the
   rule rather than the wording: the review's alternative is taken. A pass-through context
   now consumes only when the UI consumed one of its Starts; `double` and `double_nearby`
   matches are no exception (D4, Options 6, which keeps the revision-1 rule as a rejected
   row). With that, "without a sink nothing is consumed, so a pass-through context never
   hides a lower binding" is true, and so are the Consequences sentence and invariant (e),
   which now names the quick re-press cases. Invariant (g) adds the re-press cases with a
   UI attached, and the new (g2) covers the one case where `pointer` still hides
   `mouselook`: a quick right re-press over a hit element.
2. **The walker regression was not stated for the owner.** Accepted. Under the new rule the
   regression over the world is gone; what remains (a quick right re-press over a panel
   element does not start `mouselook`) is stated in the new Consequences entry "What the
   owner will notice" and in Known difference 14. That difference also withdraws the claim
   that a double-select swallowing the press is "as in the original": §12.12 picks per map,
   no cited spec gives the activation of the default `CameraInstantMouseSteer` binding
   (§12.14.2 gives only its section, `Main`), and map order and one-versus-all are OPEN.
   Difference 9 is marked inferred for the same reason. An owner observation is added, and
   the rejected rule stays available if the observation shows the original swallows.
3. **Circular coalescing rule.** Accepted with the reviewer's rule: with a sink, a pending
   move is offered before every key, button, wheel, typed-character, cursor-left and
   focus-lost raw event, whatever it will offer (D7, five numbered steps; three since
   revision 4, item 7). Typed characters
   are added to the review's list because they are offered too (D6). New invariant (l) and a
   worked-example bullet cover the move-plus-wheel frame.
4. **Pass-through against §7.2's two passes.** Accepted: each (context, binding entry) pair
   fires and is offered at most once per raw event; pass 2 runs when pass 1 consumed nothing
   and fires only new matches in a pass-through context (D4, list item 8, invariant (m)).
5. **Escape quits the client.** Accepted: Consequences entry "Escape with a UI attached",
   worked example, owners list and Cost. The review's "109 of 128 editable elements" is
   restated from the census as it is given (19 elements set `lose_focus_on_escape`, 103
   have `editable` true, so at least 84 editable fields lack it), because the census does
   not say how the two sets overlap.
6. **Wrong citation.** Accepted: D8 and the owners list cite §6 of `0015-ui-rendering.md`.
7. **`double_click_px`.** Accepted: it is the rectangle size (*W* = *H* = value), in logical
   pixels, scaled by the window's scale factor before the clamp (D3, list item 7, worked
   example).
8. **Pointer lock position.** Accepted: the client substitutes the frozen position in every
   event it hands to the UI while the lock is on (always), and also hides the cursor and
   asks for `CursorGrabMode::Locked`, falling back to `Confined`, with a warp back and a
   cursor-position event to the mapper at the end (D7, "How the position is frozen").
   Offered events carry the raw-event sequence number so the client can apply the
   substitution to the rest of the releasing event. (Revision 3, item 5, replaces the
   substitution and the feed at *F* by withholding cursor positions from the mapper during
   the lock.)
9. **Importer activations on chords.** Accepted: list item 18 drops such bindings with the
   reason "activation on a chord".
10. **How the sink requests a push.** Accepted: a mapper-owned band request list passed to
    the sink with each offer, plus a `tessera-ui` outbox that the client drains into it, or
    into direct mapper calls outside offers (D5, list item 12). D5 also now says that the
    context order of a raw event is fixed at its resolution, including a pass-through
    continuation (invariant (n)); the revision-1 wording suggested otherwise.
11. **Coalescing as a known difference.** Accepted: Known difference 15 and an owners-list
    entry for LPA §13.12.3 and §13.12.5.
12. **Check against the new and updated specs.** UW §5.7, §6.8 and §6.9 agree with D5 and
    D7: the D5 push row now separates the scrollable's push (for the hovered element in its
    subtree) from the scroll bar's (for itself), both at priority 0, and the worked example
    cites UW §5.6 for the line step. UB §4.9's frozen report is now met on every platform
    (item 8). LPA §13.12.2's Tessera order "order of the bindings in the effective key map"
    is listed for its owner as replaced by D4's order. `chat-window.md` §10 item 5 still
    names the retired `text_entry` actions and is added to the owners list.

## Revision 3: answers to the third review (2026-10-06)
1. **Frame order contradicted LPA (MUST FIX).** Accepted, and fixed rather than recorded as
   a difference. D8 now runs the UI tick in step 7 in LPA §3.3's order: hover refresh,
   `MouseHover`, the plugins' `Update` broadcast (plugin timers are `Update` handlers,
   LPA §9.4), then the rest. The plugin host's deferred work follows, and then the
   band-request outbox is drained. Layout and paint are step 8. D5's request channel names
   the step-7 sources of out-of-offer pushes, and drains after step 7 and again after
   step 8. The owners entry for LPA §3.3 and §9.4 is now a true agreement. ARCH §20.3 and
   §18.1 are reworded to match, and so is the `Update` slice: LPA §9.4 calls every
   subscriber in every tick. The companion record's restatements are listed under
   "Coordination".
2. **The lock turned on before the UI's verdict (MUST FIX).** Accepted. The lock turns on
   only after the UI declined a `mouselook` Start, and turns off after the End of the last
   hold that turned it on. A consumed Start, such as during an item drag (UB §5.3 step 4),
   changes nothing (D7). This changes nothing else, because key routing needs no hit test.
   Worked example (two new bullets), invariant (h2) and the owners list (UB §4.9,
   UB §5.3) follow. (Revision 4, item 1, narrows the UB §5.3 step 4 case and adds a Tessera
   rule for the other drags.)
3. **Band rules for today's API (SHOULD FIX).** Accepted. D5 "Code access to the band"
   defines `activate`/`deactivate`/`set_active`/`is_active` on a UI-band context (push and
   pop of owner 0 at the registered priority), refuses band pushes of game-band contexts,
   gives run-time registrations a band, and makes `handle` resolve against the band as it
   stands. D7 no longer says "no band". The claim that `handle` keeps today's behaviour is
   narrowed to "no UI-band entry" (invariant (e)). What an activated `text_entry` now does
   is listed under Consequences ("Effects on other code") and pinned by invariant (e2).
   List item 4 carries the rules. Under Cost, `input_runtime` must cover band push and pop,
   `handle_with`, `offer_pending` and `on`, and `input_keymap` must cover `on` and the
   migration.
4. **Pass-through and sequences.** Accepted: sequences in a pass-through context are
   disabled when the keymap is compiled (D4, list item 4, invariant (e3)).
5. **The cursor-position feed at the end of the lock.** Accepted, and simplified. While the
   lock is on, the client feeds the mapper no cursor-position events, so the mapper's
   position stays *F* whichever call offers the End (`handle_with` of a release or of focus
   lost, or an End from the end queue). Nothing is fed at the end, so a refused warp needs
   no special case: the next OS cursor-position event gives the real position. This
   replaces revision 2's position substitution and its feed at *F* (D7). (Revision 4,
   item 4, also withholds cursor-left events and feeds a withheld cursor-left after the
   lock ends.)
6. **A capture across the lock.** Accepted as a Tessera rule with a question for UB §4.9.
   Ends of commands in the capture's pressed set are still applied (`MouseUp`, the capture
   ends), with no click. Starts reach no element (D7, difference 16, invariant (h3)).
7. **Owners list gaps.** Accepted. `walker.md` §5 ("Escape quits") and `chat-window.md` §6.2
   are added. For §6.2 the record also adds the Tessera context `ui.chat_log`, pushed by the
   chat window while it is the active panel, so PageUp and PageDown page the log instead of
   zooming (D2, D5, list items 13, 14 and 20, difference 17).
8. **`on` on axis actions; the Repeat verdict.** Accepted. A non-`press` `on` is allowed only
   on entries of button actions. On an axis action it is disabled at compile time (D3, list
   item 5), and the importer drops such bindings with a reason (list item 18). Item 18 now
   states that release and tap bindings of existing button actions are imported where they
   used to be dropped. The mapper ignores the verdict of a Repeat; inside the UI it decides
   only whether the repeat reaches `KeyDown` (D7, list item 12).
9. **Consistency with the companion's revision 3.** Checked against its "Coordination with
   IB". Its item 1 (stale owners-list entry) is done. Its item 2 (step-8 wording) is adopted,
   with the tick moved to step 7. Its item 3 (§2.5 cites D7) is answered under
   "Coordination", item 4, because revision 3 changes the lock-on point and the frozen
   position mechanism. Its item 4 (U1's scope) is stated in D8.

## Revision 4: answers to the fourth review (2026-10-06)
1. **Item-drag claim misread UB (MUST FIX).** Accepted. UB §5.3 step 4 swallows key commands
   only from the container's creation of the icon (UB §7.1 step 5, §7.4 step 4) and only
   when they are sent to a UI element, so not for icons that game widgets supply (UB §7.1
   step 4) and not when there is no keyboard target (UB §5.3 steps 1 and 3, §4.6 step 3).
   D7 now states that narrow case and adds a **Tessera rule** for every other drag: when
   the UI declines a `mouselook` Start while a drag icon exists, the client returns
   Consumed itself and turns no lock on, because the literal reading would lock the pointer
   mid-drag and lose the drop target (UB §4.9, §7.3, §7.4 step 1). UB §7.4's OPEN (does a
   right click cancel a drag) is cited, and Known difference 20 asks for an owner
   observation. The worked example now has two cases, the owner-facing consequence says
   which part is Tessera's, and the owners list (UB §4.9, UB §5.3 and §7.1, new UB §7.4),
   invariants (h2) and new (h4), the owner summary and Coordination item 7 follow.
2. **Capture claim contradicted UB §4.4 and UW §6.9 (MUST FIX).** Accepted. Options 7 and
   D7 no longer say the entry survives a capture. D7 "Captures" states the spec reading:
   when the pointer leaves the captured element, hover becomes nothing, the entry is popped
   and the wheel zooms until the pointer is back. A new owner-facing consequence and
   invariant (k2) pin it, so no implementer special-cases captures.
3. **Pass-through pass 2 contradicted the exact-preference claims (MUST FIX).** Accepted with
   the reviewer's first option: in pass 2 a pass-through context that fired anything in
   pass 1 is skipped (D4). Invariant (m) still holds, because without an exact binding
   `pointer` fires nothing in pass 1. D2's and difference 8's claims are now true; D2
   points to the rule, list item 8 carries it and new invariant (m2) pins the Shift-click
   example.
4. **Cursor-left during the lock.** Accepted: cursor-left events are withheld during the lock
   too. The last withheld report, if it was a cursor-left, is fed right after the call that
   turned the lock off, so the turning-off End and its `select_right` pulse still carry *F*
   and the UI then hovers nothing until the pointer is back (D7 item 1, invariant (h5),
   Coordination item 8).
5. **Ends of holds started under `handle`.** Accepted: only offered Starts get Ends; such a
   hold's End and Repeats are never offered or queued (D7, list item 12, invariant (p)).
6. **Queued pops of holds not yet offered.** Accepted with the reviewer's rule: a hold or
   pulse of the raw event being resolved that an applied request ends before its Start was
   offered is dropped, with no Start, no End and no snapshot edge (D7 "Applying band
   requests", D5, list item 12, invariant (p), fuzz target).
7. **Batched versus per-offer application of band requests.** Accepted: one rule everywhere.
   Each offer's requests are applied when it returns and the Ends they cause are offered at
   once, in every step. The D7 step list is now three steps (end queue, move, resolve and
   offer), with `offer_pending` as the first two; D5 and list item 12 say the same, and
   invariant (n) names the steps.
8. **Release-class resolution after a UI-owned press.** Accepted: at the release of a button
   whose press produced only UI-owned holds, release-class resolution considers only
   UI-band and pass-through contexts (D4), the release-side counterpart of the chord rule
   (list item 8, invariant (o)).
9. **Wheel-motion timing.** Accepted: the decision to withhold is made per raw event at
   resolution, under `handle` and `handle_with` alike, and routing of motion that is not
   withheld stays at `update`. So invariant (e) is unchanged, and `handle` with an active
   `ui.scroll_mouse` withholds the motion and puts the scroll clicks in the snapshot (D7,
   list item 10, invariant (k)).
10. **D3 clamp.** Accepted: *W* and *H* are clamped to 2–64, and D3 says why (list item 7,
    invariant (r)).
11. **Modifier policies.** Accepted with a decision: Subset for the caret, delete, backspace
    and scroll actions (KM §12.12, CW §6.4), exact Ctrl+Shift defaults for the word and
    document moves so that one binding fires, and Exact for the focus and clipboard
    actions. The remaining gaps are Known difference 19 (D2, list items 3 and 13, invariant
    (q)).
12. **Focused plugin TextBox and `opaque_keys`.** Accepted: Known difference 18, and the LPA
    §13.13 owners entry mentions it.
13. **Difference 14's command ID.** Fixed: `SelectDblRight` is 0x2E; 0x60 is the
    `MouseDblClick` activation of its default binding.
14. **Options 4 note.** Reworded: a declined `select_*` is an ordinary game hold; what
    pass-through adds is that the press also continues to lower contexts.
15. **Event order keys.** Stated: pass, then context, then activation class, then
    registration order (D4, list item 8).
16. **List items 17 and 20.** Both now say that the three existing `EditControls` rows of
    §12.14.2 and the matching §12.15 rows are retargeted or replaced, not kept beside the
    new ones.
17. **"Between frames" and the collector step.** Accepted. The owners list adds LPA §3.3 and
    `plugin-runtime.md` §10.3 next to ARCHITECTURE §18.1, with one reading of "between
    frames" (D8). D8 step 7 places the per-tick `lua_gc(LUA_GCSTEP, 20)` after the deferred
    work and before the outbox drain, which is the Tessera choice of `plugin-runtime.md`
    §9.2, and decides that the deferred work is sliced by whole items (2 ms per frame, at
    least one item, order kept). ARCHITECTURE §20.3 and §18.1 wording and Coordination
    item 9 follow.
18. **256-entry limit.** Stated: the refusal is not reported back, and a later pop of the
    pair is harmless (D5).
19. **`text_entry` at two entries.** Stated: only the first `opaque_keys` entry releases
    holds, and it never releases the holds of UI-band contexts such as `ui.chat_log` (D6).
20. **Coordination item 4.** Confirmed by the review. The section "Coordination" now marks
    items 1–6 as applied by the companion's revision 4 and adds items 7–9 for what this
    revision changes in the companion (§2.5, its Context and Consequences, row U1).

## Revision 5: answers to the fifth review (2026-10-06)
1. **"Between frames" contradicted LPA §6.3, and game-raised events had no slot (MUST FIX).**
   Accepted; the rule is both routed and widened. Plugin events raised by game or session
   state (`Turbine.Chat.Received` for session lines and Tessera's own system notices, the
   gameplay views' `…Changed` events) are queued by their source, which never calls Lua,
   and delivered in step 7 as deferred work (D8, "Game-raised plugin events", Known
   difference 21, invariant (y)). The logout unload runs in step 7 as deferred work; an
   unload asked for by a plugin or the user runs in the step-1 or step-7 call that asked;
   the client-exit unload, with its synchronous `PluginData.Save`, is the one named
   exception, after the last frame (D8, "Unload at logout and at client exit"). The
   rewording of LPA §3.3, `plugin-runtime.md` §10.3 and ARCHITECTURE §18.1 and §20.3 now
   states that exception, so it is true; LPA §6.3, §11.3 and §15 get owners entries.
2. **Hit tests against the previous step-8 layout contradicted LPA §9.7 and §13.11 (MUST
   FIX).** Accepted with the reviewer's first option, widened to every element: hit tests
   read the current geometry, which handlers, panel moves and resizes, and the UL §8.4
   re-layout change at once; step 8 changes no box that a hit test reads (D8, "Geometry
   used for hit tests"). `MouseMove` `X`/`Y` are relative to the control's current
   position, so the drag-a-window pattern sees no spurious leave or enter (invariant (x)).
   This restores Option 5's argument as written. The companion's restatements (Context,
   Decision 1.5, §2.4, Consequences) are Coordination item 10.
3. **keymap.md provenance contradicted itself (MUST FIX).** Accepted. List item 1's
   exception now covers every part of §0–§11 that reproduces the original: the §8 default
   rows and command names, the §7.x activation timing (UB §3.3), the §3 band-ordering
   rules (UB §5.2), the §8.x push rule (UW §5.7, §6.9) and the §7.y repeat rule (KM
   §12.10), plus "the same command goes on". The author-role note names the spec-writer
   role for all of them, and lists what stays a Tessera design decision.
4. **"Only UI-owned holds" included presses with no hold.** Fixed in D4 and D7 and list
   item 8: "at least one hold, and all of them UI-owned"; a press with no hold qualifies for
   neither rule (invariant (t)).
5. **Release-class resolution and unheld buttons, focus loss.** Stated (D4, "Which releases
   resolve"; list item 8; invariant (s)): it runs only at the release of a held button; a
   release of a button that is not held changes nothing, as the winit adapter relies on;
   focus loss, `release_all` and the other hold-ending operations fire no `release` or
   `tap` binding.
6. **Band pushes and the pending sequence.** Stated (D5): the push of the first
   `opaque_keys` entry drops a pending sequence of a context consulted after it; a pop
   drops the popped context's sequence; other pushes, hover pushes included, keep it
   (list item 4, invariant (u)).
7. **"Swallowed" at the band end.** Reworded (D6): such a press ends the pass, like
   `opaque`, and is not consumed, so pass 2 tries the UI band again with subset matches;
   invariant (q) depends on this (list item 4).
8. **The deferred cursor-left versus "`offer_pending` after the last raw event".** D8 step 1
   now lists the exception, and D7 and list item 12 point to it.
9. **Press tracking details.** Stated (D3): only presses that §7.2 resolves are recorded, not
   OS repeats, presses of held buttons or presses beyond 64 held; on a run-time
   scale-factor change the client recomputes *W* and *H* and the mapper resets every
   recorded press position to unknown (list item 7, invariant (w)).
10. **Whose toggle type decides repeat.** Per hold, by the command its Start carried (D7,
    D9, list items 3, 12 and 13, invariant (v)); the actions' `repeat` flags are only the
    fallback without action-map data.
11. **When an invalid `on` is caught.** One split in D3 and list item 5: sequences, chords,
    wheel clicks and analog entries at read time (lenient drop, strict failure,
    `Keymap::set` refuses); `on` on an axis action only at compile time, which
    `Keymap::set` cannot check.
12. **Options 6's "a click on a panel does not reach the world".** Qualified there and in D2:
    an exact game-band binding of a modifier-click takes it in pass 1, even over a panel.
13. **Provenance of the Ctrl+Shift defaults.** List item 2 marks them, and the `ui.chat_log`
    defaults, as Tessera choices.
14. **`ui.chat_log` and imported paging overrides.** List item 17 copies an imported
    `EditControls` binding of the four paging commands into `ui.chat_log` (Tessera rule);
    item 20 refers to it.
15. **Input handled in winit callbacks.** D8 step 1: such events are handled on arrival,
    their timestamps are taken on arrival, and the work counts toward step 1 (U1 and plugin
    time) of the following frame.
16. **The walker's help line.** Cost now names the `menu` "quit" label of `help_text`, which
    the M5 client glue changes when a UI is attached.
17. **Coordination items 7–9.** Marked applied by the companion's revision 5, together with
    the companion's own follow-ups 6 and 7 (the latter is the `ui_cursor = software`
    parenthesis in D7 item 2). New items 10–12 cover what this revision changes in the
    companion.
